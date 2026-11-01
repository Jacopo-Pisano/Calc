//! Lettore del filesystem XDVDFS usato dai dischi Xbox e Xbox 360.
//!
//! Un'immagine ISO di un gioco Xbox 360 contiene una partizione XDVDFS con
//! dentro `default.xex` (l'eseguibile) e tutti i file del gioco. Le dashboard
//! homebrew (Aurora, FreeStyle Dash, XeXMenu) avviano direttamente
//! `default.xex` da una cartella, quindi "convertire un ISO in XEX" vuol dire
//! estrarre l'intero contenuto della partizione in una cartella.
//!
//! Questo crate:
//! * trova la partizione di gioco (XGD1, XGD2, XGD3 o ISO già "ricostruita");
//! * elenca tutti i file con offset e dimensione;
//! * legge l'header di `default.xex` / `default.xbe` per mostrare Title ID e
//!   informazioni sul disco;
//! * copia i file in una cartella di destinazione.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use serde::Serialize;

pub mod exe;
#[cfg(feature = "jni")]
mod jni_api;

pub use exe::ExeInfo;

/// Dimensione di un settore XDVDFS.
pub const SECTOR_SIZE: u64 = 2048;
/// Il descrittore del volume si trova al settore 32 della partizione.
const VOLUME_DESCRIPTOR_SECTOR: u64 = 32;
const MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";

const ATTR_DIRECTORY: u8 = 0x10;
/// Limite di sicurezza contro immagini corrotte con directory cicliche.
const MAX_DEPTH: usize = 64;
/// Limite di sicurezza sulla dimensione di una singola tabella di directory.
const MAX_DIR_TABLE: u32 = 64 * 1024 * 1024;

/// Tipo di disco riconosciuto, in base all'offset della partizione di gioco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Layout {
    /// ISO "ricostruita" / XSF: la partizione inizia all'offset 0.
    Rebuilt,
    /// Disco Xbox 360 XGD2.
    Xgd2,
    /// Disco Xbox 360 XGD3.
    Xgd3,
    /// Disco Xbox originale (XGD1, dump redump completo).
    Xgd1,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::Rebuilt, Layout::Xgd2, Layout::Xgd3, Layout::Xgd1];

    pub fn partition_offset(self) -> u64 {
        match self {
            Layout::Rebuilt => 0,
            Layout::Xgd2 => 0x0FD9_0000,
            Layout::Xgd3 => 0x0208_0000,
            Layout::Xgd1 => 0x1830_0000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Layout::Rebuilt => "XISO ricostruita",
            Layout::Xgd2 => "Xbox 360 XGD2",
            Layout::Xgd3 => "Xbox 360 XGD3",
            Layout::Xgd1 => "Xbox originale XGD1",
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// Nessuna partizione XDVDFS trovata: non è un'ISO Xbox / Xbox 360.
    NotXdvdfs,
    /// Struttura delle directory non valida.
    Corrupt(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "errore di I/O: {e}"),
            Error::NotXdvdfs => write!(
                f,
                "il file non è un'ISO Xbox/Xbox 360 (firma MICROSOFT*XBOX*MEDIA non trovata)"
            ),
            Error::Corrupt(m) => write!(f, "immagine corrotta: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Un file o una cartella dentro l'ISO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Percorso relativo con `/` come separatore, es. `media/audio.xma`.
    pub path: String,
    /// Offset assoluto (in byte) dei dati nel file ISO.
    pub offset: u64,
    pub size: u64,
    pub is_dir: bool,
}

/// Un'immagine XDVDFS aperta.
pub struct XisoImage<R> {
    reader: R,
    layout: Layout,
    root_sector: u32,
    root_size: u32,
}

impl<R: Read + Seek> XisoImage<R> {
    /// Cerca la partizione di gioco provando tutti gli offset noti.
    pub fn open(mut reader: R) -> Result<Self> {
        for layout in Layout::ALL {
            let pos = layout.partition_offset() + VOLUME_DESCRIPTOR_SECTOR * SECTOR_SIZE;
            let mut buf = [0u8; 28];
            if reader.seek(SeekFrom::Start(pos)).is_err() || read_full(&mut reader, &mut buf)? < 28 {
                continue;
            }
            if &buf[..20] != MAGIC {
                continue;
            }
            let root_sector = u32::from_le_bytes(buf[20..24].try_into().unwrap());
            let root_size = u32::from_le_bytes(buf[24..28].try_into().unwrap());
            return Ok(Self { reader, layout, root_sector, root_size });
        }
        Err(Error::NotXdvdfs)
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn partition_offset(&self) -> u64 {
        self.layout.partition_offset()
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Elenca ricorsivamente tutti i file e le cartelle, ordinati per percorso.
    pub fn entries(&mut self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        let mut visited = HashSet::new();
        self.walk_dir(self.root_sector, self.root_size, "", 0, &mut visited, &mut out)?;
        out.sort_by_key(|e| e.path.to_lowercase());
        Ok(out)
    }

    fn sector_offset(&self, sector: u32) -> u64 {
        self.partition_offset() + sector as u64 * SECTOR_SIZE
    }

    fn walk_dir(
        &mut self,
        sector: u32,
        size: u32,
        prefix: &str,
        depth: usize,
        visited: &mut HashSet<u32>,
        out: &mut Vec<Entry>,
    ) -> Result<()> {
        if size == 0 {
            return Ok(());
        }
        if depth > MAX_DEPTH {
            return Err(Error::Corrupt("directory annidate troppo in profondità".into()));
        }
        if size > MAX_DIR_TABLE {
            return Err(Error::Corrupt(format!("tabella di directory troppo grande ({size} byte)")));
        }
        if !visited.insert(sector) {
            return Err(Error::Corrupt(format!("directory ciclica al settore {sector}")));
        }

        let mut table = vec![0u8; size as usize];
        self.reader.seek(SeekFrom::Start(self.sector_offset(sector)))?;
        let n = read_full(&mut self.reader, &mut table)?;
        table.truncate(n);

        // Le voci formano un albero binario: ogni voce punta al sottoalbero
        // sinistro e destro tramite offset in parole da 4 byte.
        let mut stack = vec![0usize];
        let mut seen_offsets = HashSet::new();
        let mut subdirs = Vec::new();
        while let Some(off) = stack.pop() {
            if !seen_offsets.insert(off) {
                continue;
            }
            let Some(raw) = parse_dirent(&table, off) else {
                continue;
            };
            if raw.left != 0 && raw.left != 0xFFFF {
                stack.push(raw.left as usize * 4);
            }
            if raw.right != 0 && raw.right != 0xFFFF {
                stack.push(raw.right as usize * 4);
            }
            let name = sanitize_name(&raw.name)
                .ok_or_else(|| Error::Corrupt(format!("nome di file non valido: {:?}", raw.name)))?;
            let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            let is_dir = raw.attributes & ATTR_DIRECTORY != 0;
            out.push(Entry {
                path: path.clone(),
                offset: self.sector_offset(raw.sector),
                size: if is_dir { 0 } else { raw.size as u64 },
                is_dir,
            });
            if is_dir {
                subdirs.push((raw.sector, raw.size, path));
            }
        }
        for (s, sz, path) in subdirs {
            self.walk_dir(s, sz, &path, depth + 1, visited, out)?;
        }
        Ok(())
    }

    /// Legge al massimo `max` byte dall'inizio di un file.
    pub fn read_head(&mut self, entry: &Entry, max: usize) -> Result<Vec<u8>> {
        let len = (entry.size as usize).min(max);
        let mut buf = vec![0u8; len];
        self.reader.seek(SeekFrom::Start(entry.offset))?;
        let n = read_full(&mut self.reader, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// Cerca `default.xex` (Xbox 360) o `default.xbe` (Xbox) nella radice e
    /// ne legge le informazioni principali.
    pub fn executable_info(&mut self, entries: &[Entry]) -> Option<ExeInfo> {
        let exe = entries.iter().find(|e| {
            !e.is_dir
                && (e.path.eq_ignore_ascii_case("default.xex") || e.path.eq_ignore_ascii_case("default.xbe"))
        })?;
        let head = self.read_head(exe, 1024 * 1024).ok()?;
        exe::parse(&head)
    }

    /// Copia i dati di un file nello stream `out`, chiamando `progress` con il
    /// numero di byte copiati finora. Se `progress` restituisce `false` la
    /// copia viene interrotta.
    pub fn copy_entry<W: Write>(
        &mut self,
        entry: &Entry,
        out: &mut W,
        mut progress: impl FnMut(u64) -> bool,
    ) -> Result<u64> {
        self.reader.seek(SeekFrom::Start(entry.offset))?;
        let mut remaining = entry.size;
        let mut buf = vec![0u8; 1024 * 1024];
        let mut done = 0u64;
        while remaining > 0 {
            let chunk = remaining.min(buf.len() as u64) as usize;
            let n = read_full(&mut self.reader, &mut buf[..chunk])?;
            if n == 0 {
                return Err(Error::Corrupt(format!("{} termina oltre la fine dell'ISO", entry.path)));
            }
            out.write_all(&buf[..n])?;
            remaining -= n as u64;
            done += n as u64;
            if !progress(done) {
                return Err(Error::Io(io::Error::new(io::ErrorKind::Interrupted, "annullato")));
            }
        }
        Ok(done)
    }

    /// Estrae tutto il contenuto dell'ISO in `dest` (usato dalla CLI desktop).
    pub fn extract_all(
        &mut self,
        dest: &Path,
        mut progress: impl FnMut(&Entry, u64, u64),
    ) -> Result<()> {
        let entries = self.entries()?;
        let total: u64 = entries.iter().map(|e| e.size).sum();
        let mut done = 0u64;
        fs::create_dir_all(dest)?;
        for e in &entries {
            let target = dest.join(&e.path);
            if e.is_dir {
                fs::create_dir_all(&target)?;
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut f = io::BufWriter::new(fs::File::create(&target)?);
            let base = done;
            self.copy_entry(e, &mut f, |n| {
                progress(e, base + n, total);
                true
            })?;
            f.flush()?;
            done += e.size;
        }
        Ok(())
    }
}

struct RawDirent {
    left: u16,
    right: u16,
    sector: u32,
    size: u32,
    attributes: u8,
    name: Vec<u8>,
}

fn parse_dirent(table: &[u8], off: usize) -> Option<RawDirent> {
    let hdr = table.get(off..off + 14)?;
    if hdr[0] == 0xFF && hdr[1] == 0xFF {
        return None; // riempimento di fine settore
    }
    let name_len = hdr[13] as usize;
    if name_len == 0 {
        return None;
    }
    let name = table.get(off + 14..off + 14 + name_len)?.to_vec();
    Some(RawDirent {
        left: u16::from_le_bytes([hdr[0], hdr[1]]),
        right: u16::from_le_bytes([hdr[2], hdr[3]]),
        sector: u32::from_le_bytes(hdr[4..8].try_into().unwrap()),
        size: u32::from_le_bytes(hdr[8..12].try_into().unwrap()),
        attributes: hdr[12],
        name,
    })
}

/// Rende un nome sicuro da usare come componente di percorso.
fn sanitize_name(raw: &[u8]) -> Option<String> {
    // I nomi XDVDFS sono ASCII/Latin-1.
    let name: String = raw.iter().map(|&b| b as char).collect();
    if name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
        return None;
    }
    Some(name)
}

/// Come `read_exact`, ma restituisce il numero di byte letti invece di
/// fallire alla fine del file.
fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// Riepilogo serializzabile mostrato dall'app Android.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub layout: Layout,
    pub layout_label: &'static str,
    pub partition_offset: u64,
    pub total_size: u64,
    pub file_count: usize,
    pub executable: Option<ExeInfo>,
    pub entries: Vec<Entry>,
}

/// Apre un'ISO e ne restituisce il riepilogo completo.
pub fn summarize<R: Read + Seek>(reader: R) -> Result<Summary> {
    let mut img = XisoImage::open(reader)?;
    let entries = img.entries()?;
    let executable = img.executable_info(&entries);
    Ok(Summary {
        layout: img.layout(),
        layout_label: img.layout().label(),
        partition_offset: img.partition_offset(),
        total_size: entries.iter().map(|e| e.size).sum(),
        file_count: entries.iter().filter(|e| !e.is_dir).count(),
        executable,
        entries,
    })
}

#[cfg(test)]
mod tests;
