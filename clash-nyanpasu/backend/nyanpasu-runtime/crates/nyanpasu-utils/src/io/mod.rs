// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Byte-oriented IO helpers.

#[cfg(feature = "atomic_fs")]
pub mod atomic_fs;

use std::convert::Infallible;
use std::io::BufRead;
use std::result::Result as StdResult;
/// Read all bytes until a newline (the `0xA` byte) or a carriage return (`\r`) is reached, and append them to the provided buffer.
///
/// Adapted from <https://doc.rust-lang.org/std/io/trait.BufRead.html#method.read_line>.
pub fn read_line<R: BufRead + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,

                Err(e) => return Err(e),
            };
            match memchr::memchr(b'\n', available) {
                Some(i) => {
                    let end = i + 1;
                    buf.extend_from_slice(&available[..end]);
                    (true, end)
                }
                None => match memchr::memchr(b'\r', available) {
                    Some(i) => {
                        let end = i + 1;
                        buf.extend_from_slice(&available[..end]);
                        (true, end)
                    }
                    None => {
                        buf.extend_from_slice(available);
                        (false, available.len())
                    }
                },
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

/// Unwrap an infallible result.
pub fn unwrap_infallible<T>(result: StdResult<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => match err {},
    }
}
