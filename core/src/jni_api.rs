//! Binding JNI usati dall'app Android (`com.jacopopisano.xiso2xex.NativeLib`).
//!
//! Android consegna i file scelti dall'utente come file descriptor (Storage
//! Access Framework). Il codice Rust li prende in prestito senza chiuderli:
//! la proprietà resta al `ParcelFileDescriptor` lato Kotlin.

#![cfg(unix)]

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::ManuallyDrop;
use std::os::fd::FromRawFd;

use jni::objects::JClass;
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;

/// Prende in prestito un fd senza chiuderlo quando il `File` esce di scope.
fn borrow_fd(fd: jint) -> ManuallyDrop<File> {
    // SAFETY: l'fd è valido per tutta la durata della chiamata JNI ed è
    // posseduto dal chiamante; ManuallyDrop evita la doppia chiusura.
    ManuallyDrop::new(unsafe { File::from_raw_fd(fd) })
}

fn throw(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new("java/io/IOException", msg);
}

/// Analizza l'ISO e restituisce un JSON con layout, eseguibile e lista file.
#[no_mangle]
pub extern "system" fn Java_com_jacopopisano_xiso2xex_NativeLib_summarize<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    fd: jint,
) -> jstring {
    let file = borrow_fd(fd);
    let result = crate::summarize(&*file)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::to_string(&s).map_err(|e| e.to_string()));
    match result {
        Ok(json) => match env.new_string(json) {
            Ok(s) => s.into_raw(),
            Err(e) => {
                throw(&mut env, &e.to_string());
                std::ptr::null_mut()
            }
        },
        Err(msg) => {
            throw(&mut env, &msg);
            std::ptr::null_mut()
        }
    }
}

/// Copia `length` byte da `offset` dell'ISO in coda al file di output.
/// Restituisce il numero di byte copiati.
#[no_mangle]
pub extern "system" fn Java_com_jacopopisano_xiso2xex_NativeLib_copyRange<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    in_fd: jint,
    offset: jlong,
    length: jlong,
    out_fd: jint,
) -> jlong {
    let input = borrow_fd(in_fd);
    let output = borrow_fd(out_fd);
    match copy_range(&input, offset as u64, length as u64, &output) {
        Ok(n) => n as jlong,
        Err(e) => {
            throw(&mut env, &e.to_string());
            -1
        }
    }
}

fn copy_range(mut input: &File, offset: u64, length: u64, mut output: &File) -> std::io::Result<u64> {
    input.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut remaining = length;
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = input.read(&mut buf[..want])?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "fine dell'ISO raggiunta prima del previsto",
            ));
        }
        output.write_all(&buf[..n])?;
        remaining -= n as u64;
    }
    Ok(length)
}
