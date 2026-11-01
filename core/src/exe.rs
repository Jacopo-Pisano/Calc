//! Lettura minima degli header degli eseguibili Xbox 360 (XEX2) e Xbox (XBE).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeInfo {
    /// "XEX2" (Xbox 360) oppure "XBE" (Xbox originale).
    pub format: &'static str,
    /// Title ID in esadecimale, es. `4D5307E6`.
    pub title_id: Option<String>,
    /// Titolo leggibile (solo XBE: gli XEX non lo contengono nell'header).
    pub title_name: Option<String>,
    pub media_id: Option<String>,
    pub version: Option<String>,
    pub disc_number: Option<u8>,
    pub disc_count: Option<u8>,
}

const XEX_EXECUTION_INFO: u32 = 0x0004_0006;

pub fn parse(data: &[u8]) -> Option<ExeInfo> {
    match data.get(..4)? {
        b"XEX2" => parse_xex2(data),
        b"XBEH" => parse_xbe(data),
        _ => None,
    }
}

fn be32(d: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(off..off + 4)?.try_into().ok()?))
}

fn le32(d: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(off..off + 4)?.try_into().ok()?))
}

fn parse_xex2(d: &[u8]) -> Option<ExeInfo> {
    let mut info = ExeInfo {
        format: "XEX2",
        title_id: None,
        title_name: None,
        media_id: None,
        version: None,
        disc_number: None,
        disc_count: None,
    };
    let count = be32(d, 0x14)? as usize;
    for i in 0..count.min(256) {
        let base = 0x18 + i * 8;
        let (Some(key), Some(value)) = (be32(d, base), be32(d, base + 4)) else {
            break;
        };
        if key != XEX_EXECUTION_INFO {
            continue;
        }
        let o = value as usize;
        let Some(ex) = d.get(o..o + 24) else { break };
        let media_id = be32(ex, 0)?;
        let version = be32(ex, 4)?;
        let title_id = be32(ex, 12)?;
        info.media_id = Some(format!("{media_id:08X}"));
        info.version = Some(format!(
            "{}.{}.{}.{}",
            version >> 28,
            (version >> 24) & 0xF,
            (version >> 8) & 0xFFFF,
            version & 0xFF
        ));
        info.title_id = Some(format!("{title_id:08X}"));
        info.disc_number = Some(ex[18]);
        info.disc_count = Some(ex[19]);
    }
    Some(info)
}

fn parse_xbe(d: &[u8]) -> Option<ExeInfo> {
    let base = le32(d, 0x104)?;
    let cert_addr = le32(d, 0x118)?;
    let cert = cert_addr.checked_sub(base)? as usize;
    let title_id = le32(d, cert + 0x8)?;
    let raw_name = d.get(cert + 0xC..cert + 0xC + 80)?;
    let units: Vec<u16> = raw_name
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    let name = String::from_utf16_lossy(&units).trim().to_string();
    Some(ExeInfo {
        format: "XBE",
        title_id: Some(format!("{title_id:08X}")),
        title_name: (!name.is_empty()).then_some(name),
        media_id: None,
        version: None,
        disc_number: None,
        disc_count: None,
    })
}
