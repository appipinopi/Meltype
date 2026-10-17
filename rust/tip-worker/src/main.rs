// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Private framed stdio transport. Each process owns one independent TIP connection.
use std::{
    ffi::{CStr, CString},
    io::{self, Read, Write},
};
fn serve(input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    let connection = meltype_native::tip::meltype_tip_create();
    if connection.is_null() {
        return Err(io::Error::other("cannot create TIP connection"));
    }
    let result = (|| loop {
        let mut size = [0; 4];
        match input.read_exact(&mut size) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            result => result?,
        }
        let size = u32::from_le_bytes(size) as usize;
        if size > 65536 {
            return Err(io::Error::other("request exceeds 64 KiB"));
        }
        let mut request = vec![0; size];
        input.read_exact(&mut request)?;
        let request = CString::new(request).map_err(io::Error::other)?;
        let raw = unsafe { meltype_native::tip::meltype_tip_request(connection, request.as_ptr()) };
        let response = if raw.is_null() {
            Vec::new()
        } else {
            let response = unsafe { CStr::from_ptr(raw).to_bytes().to_vec() };
            unsafe {
                libc::free(raw.cast());
            }
            response
        };
        if response.len() > 1024 * 1024 {
            return Err(io::Error::other("response exceeds 1 MiB"));
        }
        output.write_all(&(response.len() as u32).to_le_bytes())?;
        output.write_all(&response)?;
        output.flush()?;
    })();
    meltype_native::tip::meltype_tip_destroy(connection);
    result
}
fn main() {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--stdio-tip-worker"] {
        std::process::exit(2);
    }
    if serve(&mut io::stdin().lock(), &mut io::stdout().lock()).is_err() {
        std::process::exit(1);
    }
}
