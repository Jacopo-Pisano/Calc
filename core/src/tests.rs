use super::*;
use std::io::Cursor;

const S: usize = SECTOR_SIZE as usize;

/// Costruisce una tabella di directory; `children[i] = (left, right)` sono
/// gli indici dei figli nell'albero binario.
fn dir_table(entries: &[(&str, u32, u32, u8)], children: &[(Option<usize>, Option<usize>)]) -> Vec<u8> {
    let mut offsets = Vec::new();
    let mut off = 0;
    for (name, ..) in entries {
        offsets.push(off);
        off += (14 + name.len() + 3) & !3;
    }
    let mut t = vec![0xFFu8; S];
    for (i, (name, sector, size, attr)) in entries.iter().enumerate() {
        let o = offsets[i];
        let l = children[i].0.map_or(0, |c| offsets[c] / 4) as u16;
        let r = children[i].1.map_or(0, |c| offsets[c] / 4) as u16;
        t[o..o + 2].copy_from_slice(&l.to_le_bytes());
        t[o + 2..o + 4].copy_from_slice(&r.to_le_bytes());
        t[o + 4..o + 8].copy_from_slice(&sector.to_le_bytes());
        t[o + 8..o + 12].copy_from_slice(&size.to_le_bytes());
        t[o + 12] = *attr;
        t[o + 13] = name.len() as u8;
        t[o + 14..o + 14 + name.len()].copy_from_slice(name.as_bytes());
    }
    t
}

fn xex_header() -> Vec<u8> {
    let mut h = vec![0u8; 0x100];
    h[..4].copy_from_slice(b"XEX2");
    h[0x14..0x18].copy_from_slice(&1u32.to_be_bytes());
    h[0x18..0x1C].copy_from_slice(&0x0004_0006u32.to_be_bytes());
    h[0x1C..0x20].copy_from_slice(&0x80u32.to_be_bytes());
    let ex = 0x80;
    h[ex..ex + 4].copy_from_slice(&0xA1B2_C3D4u32.to_be_bytes()); // media id
    h[ex + 4..ex + 8].copy_from_slice(&0x2000_1234u32.to_be_bytes()); // versione 2.0.18.52
    h[ex + 12..ex + 16].copy_from_slice(&0x4D53_07E6u32.to_be_bytes()); // title id
    h[ex + 18] = 1;
    h[ex + 19] = 2;
    h
}

fn a_bin() -> Vec<u8> {
    (0..5000u32).map(|i| (i % 251) as u8).collect()
}

fn build_iso(partition: usize) -> Vec<u8> {
    let p = |sector: usize| partition + sector * S;
    let mut iso = vec![0u8; p(40)];

    // Descrittore del volume.
    let vd = p(32);
    iso[vd..vd + 20].copy_from_slice(MAGIC);
    iso[vd + 20..vd + 24].copy_from_slice(&33u32.to_le_bytes());
    iso[vd + 24..vd + 28].copy_from_slice(&(S as u32).to_le_bytes());
    iso[vd + 0x7EC..vd + 0x800].copy_from_slice(&MAGIC[..20]);

    let xex = xex_header();
    // Radice: "media" è la radice dell'albero, "default.xex" a sinistra,
    // "zz.txt" a destra.
    let root = dir_table(
        &[
            ("media", 34, S as u32, ATTR_DIRECTORY),
            ("default.xex", 35, xex.len() as u32, 0x80),
            ("zz.txt", 39, 3, 0x80),
        ],
        &[(Some(1), Some(2)), (None, None), (None, None)],
    );
    iso[p(33)..p(34)].copy_from_slice(&root);
    let media = dir_table(&[("a.bin", 36, 5000, 0x80)], &[(None, None)]);
    iso[p(34)..p(35)].copy_from_slice(&media);
    iso[p(35)..p(35) + xex.len()].copy_from_slice(&xex);
    let a = a_bin();
    iso[p(36)..p(36) + a.len()].copy_from_slice(&a);
    iso[p(39)..p(39) + 3].copy_from_slice(b"ciao"[..3].as_ref());
    iso
}

#[test]
fn lists_entries_rebuilt() {
    let iso = build_iso(0);
    let mut img = XisoImage::open(Cursor::new(iso)).unwrap();
    assert_eq!(img.layout(), Layout::Rebuilt);
    let paths: Vec<_> = img.entries().unwrap().into_iter().map(|e| (e.path, e.size, e.is_dir)).collect();
    assert_eq!(
        paths,
        vec![
            ("default.xex".to_string(), 0x100, false),
            ("media".to_string(), 0, true),
            ("media/a.bin".to_string(), 5000, false),
            ("zz.txt".to_string(), 3, false),
        ]
    );
}

#[test]
fn detects_xgd3_partition_and_xex_info() {
    let iso = build_iso(Layout::Xgd3.partition_offset() as usize);
    let s = summarize(Cursor::new(iso)).unwrap();
    assert_eq!(s.layout, Layout::Xgd3);
    assert_eq!(s.file_count, 3);
    assert_eq!(s.total_size, 0x100 + 5000 + 3);
    let exe = s.executable.unwrap();
    assert_eq!(exe.format, "XEX2");
    assert_eq!(exe.title_id.as_deref(), Some("4D5307E6"));
    assert_eq!(exe.media_id.as_deref(), Some("A1B2C3D4"));
    assert_eq!(exe.version.as_deref(), Some("2.0.18.52"));
    assert_eq!((exe.disc_number, exe.disc_count), (Some(1), Some(2)));
}

#[test]
fn extracts_files() {
    let iso = build_iso(0);
    let dir = std::env::temp_dir().join(format!("xiso-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let mut img = XisoImage::open(Cursor::new(iso)).unwrap();
    let mut last = (0, 0);
    img.extract_all(&dir, |_, done, total| last = (done, total)).unwrap();
    assert_eq!(last, (0x100 + 5000 + 3, 0x100 + 5000 + 3));
    assert_eq!(fs::read(dir.join("default.xex")).unwrap(), xex_header());
    assert_eq!(fs::read(dir.join("media/a.bin")).unwrap(), a_bin());
    assert_eq!(fs::read(dir.join("zz.txt")).unwrap(), b"cia");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rejects_non_xbox_iso() {
    let data = vec![0u8; 200 * 1024];
    assert!(matches!(XisoImage::open(Cursor::new(data)), Err(Error::NotXdvdfs)));
}

#[test]
fn rejects_path_traversal() {
    let mut iso = build_iso(0);
    let root = dir_table(&[("..", 35, 10, 0x80)], &[(None, None)]);
    iso[33 * S..34 * S].copy_from_slice(&root);
    let mut img = XisoImage::open(Cursor::new(iso)).unwrap();
    assert!(matches!(img.entries(), Err(Error::Corrupt(_))));
}

#[test]
fn rejects_directory_cycle() {
    let mut iso = build_iso(0);
    // "media" punta di nuovo alla radice.
    let root = dir_table(&[("media", 33, S as u32, ATTR_DIRECTORY)], &[(None, None)]);
    iso[33 * S..34 * S].copy_from_slice(&root);
    let mut img = XisoImage::open(Cursor::new(iso)).unwrap();
    assert!(matches!(img.entries(), Err(Error::Corrupt(_))));
}

#[test]
fn parses_xbe() {
    let mut d = vec![0u8; 0x400];
    d[..4].copy_from_slice(b"XBEH");
    d[0x104..0x108].copy_from_slice(&0x10000u32.to_le_bytes());
    d[0x118..0x11C].copy_from_slice(&0x10200u32.to_le_bytes());
    d[0x208..0x20C].copy_from_slice(&0x4D53_0004u32.to_le_bytes());
    for (i, u) in "Gioco".encode_utf16().enumerate() {
        d[0x20C + i * 2..0x20C + i * 2 + 2].copy_from_slice(&u.to_le_bytes());
    }
    let info = exe::parse(&d).unwrap();
    assert_eq!(info.format, "XBE");
    assert_eq!(info.title_id.as_deref(), Some("4D530004"));
    assert_eq!(info.title_name.as_deref(), Some("Gioco"));
}
