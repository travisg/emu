// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Save states: a machine's whole state in a file, and back.
//!
//! The format is this tree's own, little-endian throughout:
//!
//! ```text
//! "EMUSNAP\0"  u32 version  str system  section*
//! section := [tag: 4 bytes] [len: u32] [payload: len bytes]
//! ```
//!
//! `system` is the machine's full name as `-s` gave it, subsystem tokens
//! included, since `rc2014` and `rc2014-cpm` are different machines. The
//! cpu's state is the `CPU ` section and the bus's -- memory and every
//! device -- the `BUS ` section, which nests a section per device. A reader
//! of a section must consume it exactly: a load that reads less or more
//! than its save wrote is a save and a load that disagree, and fails.
//!
//! What is saved is the machine. Memory goes whole, rom included, so a
//! snapshot is a machine in itself. Session settings -- the pacing rate,
//! `--fast-io`, the windows' mirrors, the console and serial endpoints --
//! stay as the running session has them: a load leaves those fields alone.
//! So do keys typed and not yet taken by the guest, which are in the host's
//! queue rather than the machine: a snapshot taken with keys still queued
//! is restored without them, and the queue's next key reaches the restored
//! machine instead.
//! Each `save` destructures its struct without `..`, binding a field it
//! does not save to `_` with the reason, so a field added later and not
//! saved does not compile.
//!
//! Disks follow one rule: a restore never writes into a file the user
//! owns. A floppy or a 703 platter is saved whole and comes back
//! copy-on-write, its session's writes in memory, and the port's `savedisk`
//! writes one out to a new file; the RC2014's card, at 128 MB, is saved as
//! its path and a hash of its contents, and a restore refuses a card that
//! no longer hashes the same.

use crate::bus::Bus;
use crate::cpu::Cpu;

pub const MAGIC: &[u8; 8] = b"EMUSNAP\0";
pub const VERSION: u32 = 1;

/// Builds a snapshot.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A length, then the bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.buf.extend_from_slice(b);
    }

    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    /// A present flag, then the value if there is one.
    pub fn opt<T>(&mut self, v: &Option<T>, mut f: impl FnMut(&mut Self, &T)) {
        self.bool(v.is_some());
        if let Some(v) = v {
            f(self, v);
        }
    }

    /// A tagged section, whose payload `f` writes.
    pub fn section(&mut self, tag: &[u8; 4], f: impl FnOnce(&mut Self)) {
        self.buf.extend_from_slice(tag);
        let at = self.buf.len();
        self.u32(0);
        f(self);
        let len = (self.buf.len() - at - 4) as u32;
        self.buf[at..at + 4].copy_from_slice(&len.to_le_bytes());
    }
}

/// Reads a snapshot back; every read fails rather than run off the end.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// What is being read, for the errors.
    what: String,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0, what: "snapshot".into() }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.data.len() - self.pos < n {
            return Err(format!("{} ends early", self.what));
        }
        let b = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(b)
    }

    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(format!("{}: {v} is not a flag", self.what)),
        }
    }

    pub fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    /// Bytes into `into`, which must be the length they were saved at:
    /// memory that changed size is a different machine.
    pub fn bytes_into(&mut self, into: &mut [u8]) -> Result<(), String> {
        let b = self.bytes()?;
        if b.len() != into.len() {
            return Err(format!("{}: {} bytes where {} were expected", self.what, b.len(), into.len()));
        }
        into.copy_from_slice(b);
        Ok(())
    }

    pub fn str(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| format!("{}: a name that is not text", self.what))
    }

    pub fn opt<T>(&mut self, mut f: impl FnMut(&mut Self) -> Result<T, String>) -> Result<Option<T>, String> {
        if self.bool()? {
            Ok(Some(f(self)?))
        } else {
            Ok(None)
        }
    }

    /// The next section, which must be `tag`, read by `f` to its end.
    pub fn section<T>(&mut self, tag: &[u8; 4], f: impl FnOnce(&mut Reader<'a>) -> Result<T, String>) -> Result<T, String> {
        let name = String::from_utf8_lossy(tag).trim_end().to_string();
        let got = self.take(4)?;
        if got != tag {
            return Err(format!("{}: expected section {name}, found {}", self.what, String::from_utf8_lossy(got)));
        }
        let n = self.u32()? as usize;
        let payload = self.take(n)?;
        let mut inner = Reader { data: payload, pos: 0, what: format!("section {name}") };
        let v = f(&mut inner)?;
        inner.finish()?;
        Ok(v)
    }

    /// Everything read: a save and a load that disagree leave some over.
    pub fn finish(&self) -> Result<(), String> {
        match self.data.len() - self.pos {
            0 => Ok(()),
            n => Err(format!("{} has {n} bytes this load did not read", self.what)),
        }
    }
}

/// A disk too big to embed is saved as a hash of its contents: 64-bit
/// FNV-1a, which tells a changed card from the same one and is all a
/// restore needs to know.
pub fn content_hash(mut r: impl std::io::Read) -> std::io::Result<u64> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Ok(h);
        }
        for &b in &buf[..n] {
            h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// `bytes` into a new file at `path`, refusing one that is there: the
/// file a disk is saved to is never one the user already has.
pub fn write_new_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A piece of the machine that saves and loads its own state.
pub trait Snapshot {
    fn save(&self, w: &mut Writer);
    fn load(&mut self, r: &mut Reader) -> Result<(), String>;
}

/// A whole machine: the header, then the cpu and the bus.
pub fn save_machine(system: &str, cpu: &dyn Cpu, bus: &dyn Bus) -> Result<Vec<u8>, String> {
    let mut w = Writer::new();
    write_header(&mut w, system);
    let mut cpu_result = Ok(());
    w.section(b"CPU ", |w| cpu_result = cpu.save_state(w));
    cpu_result?;
    let mut bus_result = Ok(());
    w.section(b"BUS ", |w| bus_result = bus.save_state(w));
    bus_result?;
    Ok(w.into_bytes())
}

/// Load a whole machine saved by `save_machine` into `cpu` and `bus`,
/// which must be the machine it was saved from. On an error the machine
/// may be part loaded: the caller restores what it had.
pub fn load_machine(data: &[u8], system: &str, cpu: &mut dyn Cpu, bus: &mut dyn Bus) -> Result<(), String> {
    let mut r = Reader::new(data);
    read_header(&mut r, system)?;
    r.section(b"CPU ", |r| cpu.load_state(r))?;
    r.section(b"BUS ", |r| bus.load_state(r))?;
    r.finish()
}

/// The header: magic, version, system.
pub fn write_header(w: &mut Writer, system: &str) {
    w.buf.extend_from_slice(MAGIC);
    w.u32(VERSION);
    w.str(system);
}

/// Check the header against the machine it is being loaded into.
pub fn read_header(r: &mut Reader, system: &str) -> Result<(), String> {
    if r.take(MAGIC.len()).ok() != Some(&MAGIC[..]) {
        return Err("not a snapshot".into());
    }
    match r.u32()? {
        VERSION => {}
        v => return Err(format!("snapshot version {v}; this emulator reads {VERSION}")),
    }
    let saved = r.str()?;
    if saved != system {
        return Err(format!("a snapshot of {saved}, and this machine is {system}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        let mut w = Writer::new();
        write_header(&mut w, "rc2014-cpm");
        w.section(b"TEST", |w| {
            w.u8(1);
            w.bool(true);
            w.u16(0x1234);
            w.u32(0xdeadbeef);
            w.u64(1 << 40);
            w.bytes(&[9, 8, 7]);
            w.str("disks/x.img");
            w.opt(&Some(5u16), |w, v| w.u16(*v));
            w.opt(&None::<u16>, |w, v| w.u16(*v));
        });
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        read_header(&mut r, "rc2014-cpm").unwrap();
        r.section(b"TEST", |r| {
            assert_eq!(r.u8()?, 1);
            assert!(r.bool()?);
            assert_eq!(r.u16()?, 0x1234);
            assert_eq!(r.u32()?, 0xdeadbeef);
            assert_eq!(r.u64()?, 1 << 40);
            assert_eq!(r.bytes()?, &[9, 8, 7]);
            assert_eq!(r.str()?, "disks/x.img");
            assert_eq!(r.opt(|r| r.u16())?, Some(5));
            assert_eq!(r.opt(|r| r.u16())?, None);
            Ok(())
        })
        .unwrap();
        r.finish().unwrap();
    }

    #[test]
    fn a_load_that_disagrees_with_its_save_fails() {
        let mut w = Writer::new();
        w.section(b"TEST", |w| w.u32(7));
        let bytes = w.into_bytes();
        let short = Reader::new(&bytes).section(b"TEST", |r| r.u16());
        assert!(short.unwrap_err().contains("did not read"), "read too little");
        let long = Reader::new(&bytes).section(b"TEST", |r| r.u64());
        assert!(long.unwrap_err().contains("ends early"), "read too much");
        let wrong = Reader::new(&bytes).section(b"CPU ", |r| r.u32());
        assert!(wrong.unwrap_err().contains("expected section CPU"));
    }

    #[test]
    fn the_header_names_the_machine() {
        let mut w = Writer::new();
        write_header(&mut w, "rc2014");
        let bytes = w.into_bytes();
        assert!(read_header(&mut Reader::new(&bytes), "rc2014-cpm").unwrap_err().contains("this machine is rc2014-cpm"));
        assert_eq!(read_header(&mut Reader::new(b"nope"), "rc2014"), Err("not a snapshot".into()));
    }
}
