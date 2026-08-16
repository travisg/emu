// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! ROM image loading.
//!
//! Two distinct paths, matching the C++:
//!
//! - **Flat binary** -- the common case. Altair680, RC2014 and Kaypro all load
//!   this way, straight into a `Memory` buffer.
//! - **Intel HEX** -- used by system09 only. Despite appearances the other
//!   three systems don't use it: altair680 and kaypro have an
//!   `iHexParseCallback` that is never wired up (kaypro's isn't even defined)
//!   and rc2014 has a stray `#include "ihex.h"` with no parser.

use std::fs;
use std::io;
use std::path::Path;

/// Read a flat binary ROM image, as the C++ `fopen`/`fread` does.
pub fn load_binary(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

/// Parse an Intel HEX file, handing each data record to `write` as
/// `(address, bytes)`.
///
/// Replaces the C++ `iHexParseCallback(ptr, address, len)` shape, so a machine
/// keeps its existing "write into the decoded device" logic. Extended
/// linear/segment address records accumulate into the high bits of the
/// address, exactly as libihex does.
pub fn load_ihex<F>(path: &Path, mut write: F) -> io::Result<()>
where
    F: FnMut(u32, &[u8]),
{
    let text = fs::read_to_string(path)?;
    let mut base: u32 = 0;

    for record in ihex::Reader::new(&text) {
        let record = record
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        match record {
            ihex::Record::Data { offset, value } => {
                write(base.wrapping_add(offset as u32), &value);
            }
            ihex::Record::ExtendedLinearAddress(a) => {
                base = (a as u32) << 16;
            }
            ihex::Record::ExtendedSegmentAddress(a) => {
                base = (a as u32) << 4;
            }
            ihex::Record::EndOfFile => break,
            // start-address records carry an entry point, which none of these
            // machines use -- they all start from a reset vector
            ihex::Record::StartLinearAddress(_)
            | ihex::Record::StartSegmentAddress { .. } => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("emu-rom-test-{}-{}", std::process::id(), name));
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn parses_data_records() {
        // two bytes (0x01 0x02) at address 0x0100, then EOF
        let p = temp_file("data.hex", ":020100000102FA\n:00000001FF\n");
        let mut got = Vec::new();
        load_ihex(&p, |addr, bytes| got.push((addr, bytes.to_vec()))).unwrap();
        fs::remove_file(&p).ok();
        assert_eq!(got, vec![(0x0100, vec![0x01, 0x02])]);
    }

    #[test]
    fn extended_linear_address_offsets_subsequent_records() {
        let p = temp_file(
            "ela.hex",
            ":02000004FFFFFC\n:020100000102FA\n:00000001FF\n",
        );
        let mut got = Vec::new();
        load_ihex(&p, |addr, bytes| got.push((addr, bytes.to_vec()))).unwrap();
        fs::remove_file(&p).ok();
        assert_eq!(got, vec![(0xffff_0100, vec![0x01, 0x02])]);
    }

    #[test]
    fn rejects_a_corrupt_checksum() {
        let p = temp_file("bad.hex", ":020100000102FF\n:00000001FF\n");
        let r = load_ihex(&p, |_, _| {});
        fs::remove_file(&p).ok();
        assert!(r.is_err(), "a bad checksum must not parse silently");
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        assert!(load_binary(Path::new("/nonexistent/rom.bin")).is_err());
        assert!(load_ihex(Path::new("/nonexistent/rom.hex"), |_, _| {}).is_err());
    }
}
