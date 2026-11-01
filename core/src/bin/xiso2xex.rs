//! CLI desktop: `xiso2xex <gioco.iso> [cartella_output]`
//!
//! Estrae il contenuto di un'ISO Xbox 360 (default.xex + file del gioco) in
//! una cartella pronta da copiare su una console con dashboard homebrew.

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xiso::XisoImage;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(iso) = args.first().map(PathBuf::from) else {
        eprintln!("uso: xiso2xex <gioco.iso> [cartella_output]");
        return ExitCode::from(2);
    };
    let dest = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| iso.with_extension(""));

    match run(&iso, &dest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nerrore: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(iso: &Path, dest: &Path) -> Result<(), xiso::Error> {
    let mut img = XisoImage::open(BufReader::with_capacity(1 << 20, File::open(iso)?))?;
    println!("Formato: {}", img.layout().label());
    let entries = img.entries()?;
    match img.executable_info(&entries) {
        Some(exe) => println!(
            "Eseguibile: {} — Title ID {} {}",
            exe.format,
            exe.title_id.as_deref().unwrap_or("?"),
            exe.title_name.as_deref().unwrap_or("")
        ),
        None => println!("Attenzione: default.xex non trovato nella radice"),
    }
    println!("Estrazione in {}", dest.display());
    let mut last_pct = u64::MAX;
    img.extract_all(dest, |_, done, total| {
        let pct = (done * 100).checked_div(total).unwrap_or(100);
        if pct != last_pct {
            last_pct = pct;
            print!("\r{pct:3}%");
            let _ = std::io::stdout().flush();
        }
    })?;
    println!("\nFatto: {} file.", entries.iter().filter(|e| !e.is_dir).count());
    Ok(())
}
