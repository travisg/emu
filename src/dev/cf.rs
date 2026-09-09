// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! A CompactFlash card in True IDE mode on an eight-bit bus: the RC2014's
//! compact flash module, at ports `0x10`-`0x17`.
//!
//! Written from the ATA task-file register set the card presents (CF+ and
//! CompactFlash Specification, chapter 6), which is what Grant Searle's CP/M
//! monitor and CBIOS drive: features, sector count, the four LBA bytes,
//! command/status, and the data register. The module wires only the low
//! eight data lines, so a guest has to put the card in 8-bit mode (`SET
//! FEATURES` with `0x01`) before a transfer means anything; until it does,
//! each data access moves a sixteen-bit word of which only the low byte is
//! seen, as on the real module.
//!
//! Commands: `READ SECTORS` (`0x20`/`0x21`) and `WRITE SECTORS`
//! (`0x30`/`0x31`), with the sector count the task file says and zero
//! meaning 256; `IDENTIFY DEVICE` (`0xec`); `SET FEATURES` (`0xef`),
//! accepted whatever the feature; `FLUSH CACHE`, `INITIALIZE DEVICE
//! PARAMETERS` and `READ VERIFY`, which have nothing to do. Anything else
//! aborts. The card is never busy: a command completes as it is written,
//! and the data is there on the next read, which is what the monitor's
//! loader relies on -- it waits on BSY and never looks at DRQ.
//!
//! The image is an ordinary file, any size that is whole sectors, opened
//! read/write; writes go through, as the 703's discs do. A sector past its
//! end is an ID-not-found error. With no image the card is not there, and
//! every register reads as the floating bus, `0xff` -- which leaves the
//! monitor's loader waiting on a BSY that never clears, as a missing card
//! does.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const SECTOR_SIZE: usize = 512;

// status register bits
const STATUS_DRDY: u8 = 1 << 6;
const STATUS_DSC: u8 = 1 << 4;
const STATUS_DRQ: u8 = 1 << 3;
const STATUS_ERR: u8 = 1 << 0;

// error register bits
const ERROR_ABORT: u8 = 1 << 2;
const ERROR_ID_NOT_FOUND: u8 = 1 << 4;

const FEATURE_8BIT: u8 = 0x01;

pub struct CompactFlash {
    image: Option<File>,
    /// Sectors in the image.
    sectors: u64,
    error: u8,
    count: u8,
    lba: [u8; 4],
    status: u8,
    eight_bit: bool,

    buffer: [u8; SECTOR_SIZE],
    /// Next byte of `buffer` to hand out or take in.
    index: usize,
    /// Sectors still to move in the current command, this one included.
    remaining: u32,
    /// The current command is a write: the data register takes bytes.
    writing: bool,
    /// The sector the current transfer is at.
    current: u64,
}

impl Default for CompactFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactFlash {
    pub fn new() -> Self {
        CompactFlash {
            image: None,
            sectors: 0,
            error: 0,
            count: 0,
            lba: [0; 4],
            status: STATUS_DRDY | STATUS_DSC,
            eight_bit: false,
            buffer: [0; SECTOR_SIZE],
            index: 0,
            remaining: 0,
            writing: false,
            current: 0,
        }
    }

    /// Insert the card: the image at `path`, read/write. A file that is not
    /// there is a slot with no card in it, reported rather than fatal.
    pub fn mount(&mut self, path: &Path) -> bool {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => {
                let len = f.metadata().map(|m| m.len()).unwrap_or(0);
                self.sectors = len / SECTOR_SIZE as u64;
                println!("CF: mounted '{}', {} sectors", path.display(), self.sectors);
                self.image = Some(f);
                true
            }
            Err(e) => {
                println!("CF: no card: '{}': {e}", path.display());
                self.image = None;
                false
            }
        }
    }

    pub fn is_mounted(&self) -> bool {
        self.image.is_some()
    }

    fn lba(&self) -> u64 {
        (self.lba[0] as u64)
            | (self.lba[1] as u64) << 8
            | (self.lba[2] as u64) << 16
            | ((self.lba[3] & 0x0f) as u64) << 24
    }

    fn fail(&mut self, error: u8) {
        self.error = error;
        self.status = STATUS_DRDY | STATUS_DSC | STATUS_ERR;
        self.remaining = 0;
    }

    fn done(&mut self) {
        self.error = 0;
        self.status = STATUS_DRDY | STATUS_DSC;
        self.remaining = 0;
    }

    /// Fill the buffer from sector `current`; false past the image's end.
    fn load(&mut self) -> bool {
        let Some(image) = self.image.as_mut() else { return false };
        if self.current >= self.sectors {
            return false;
        }
        image
            .seek(SeekFrom::Start(self.current * SECTOR_SIZE as u64))
            .and_then(|_| image.read_exact(&mut self.buffer))
            .is_ok()
    }

    /// Write the buffer to sector `current`; false past the image's end.
    fn store(&mut self) -> bool {
        let Some(image) = self.image.as_mut() else { return false };
        if self.current >= self.sectors {
            return false;
        }
        image
            .seek(SeekFrom::Start(self.current * SECTOR_SIZE as u64))
            .and_then(|_| image.write_all(&self.buffer))
            .and_then(|_| image.flush())
            .is_ok()
    }

    /// Start moving `count` sectors from the task file's LBA.
    fn start(&mut self, writing: bool) {
        self.current = self.lba();
        self.remaining = if self.count == 0 { 256 } else { self.count as u32 };
        self.writing = writing;
        self.index = 0;
        if !writing && !self.load() {
            self.fail(ERROR_ID_NOT_FOUND);
            return;
        }
        if writing && self.current + self.remaining as u64 > self.sectors {
            self.fail(ERROR_ID_NOT_FOUND);
            return;
        }
        self.error = 0;
        self.status = STATUS_DRDY | STATUS_DSC | STATUS_DRQ;
    }

    /// The end of a sector's worth of data: on to the next, or done.
    fn next_sector(&mut self) {
        if self.writing && !self.store() {
            self.fail(ERROR_ABORT);
            return;
        }
        self.remaining -= 1;
        self.current += 1;
        self.index = 0;
        if self.remaining == 0 {
            self.done();
        } else if !self.writing && !self.load() {
            self.fail(ERROR_ID_NOT_FOUND);
        }
    }

    /// `IDENTIFY DEVICE`: the 256-word block, with the fields a driver
    /// reads -- the LBA capacity and a model name.
    fn identify(&mut self) {
        let mut words = [0u16; 256];
        words[0] = 0x848a; // CompactFlash signature
        words[49] = 1 << 9; // LBA supported
        words[60] = self.sectors as u16;
        words[61] = (self.sectors >> 16) as u16;
        let model = b"emu compact flash                       ";
        for (i, pair) in model.chunks(2).enumerate() {
            words[27 + i] = ((pair[0] as u16) << 8) | pair[1] as u16;
        }
        for (i, w) in words.iter().enumerate() {
            self.buffer[2 * i] = *w as u8;
            self.buffer[2 * i + 1] = (*w >> 8) as u8;
        }
        self.remaining = 1;
        self.writing = false;
        self.index = 0;
        self.error = 0;
        self.status = STATUS_DRDY | STATUS_DSC | STATUS_DRQ;
    }

    fn command(&mut self, cmd: u8) {
        match cmd {
            0x20 | 0x21 => self.start(false),
            0x30 | 0x31 => self.start(true),
            0xec => self.identify(),
            0xef => {
                // SET FEATURES: 8-bit transfers is the one that matters
                // here; the rest (write cache, and so on) have no effect
                if self.error == FEATURE_8BIT {
                    self.eight_bit = true;
                }
                self.done();
            }
            // flush cache, initialize device parameters, read verify
            0xe7 | 0x91 | 0x40 | 0x41 => self.done(),
            _ => self.fail(ERROR_ABORT),
        }
    }

    /// Register read: 0 data, 1 error, 2 sector count, 3-6 LBA, 7 status.
    pub fn read(&mut self, reg: u8) -> u8 {
        if self.image.is_none() {
            return 0xff;
        }
        match reg {
            0 => {
                if self.status & STATUS_DRQ == 0 || self.writing {
                    return 0;
                }
                let val = self.buffer[self.index];
                // a sixteen-bit access the module only shows half of
                self.index += if self.eight_bit { 1 } else { 2 };
                if self.index >= SECTOR_SIZE {
                    self.next_sector();
                }
                val
            }
            1 => self.error,
            2 => self.count,
            3..=6 => self.lba[(reg - 3) as usize],
            _ => self.status,
        }
    }

    /// Register write: 0 data, 1 features, 2 sector count, 3-6 LBA, 7
    /// command.
    pub fn write(&mut self, reg: u8, val: u8) {
        if self.image.is_none() {
            return;
        }
        match reg {
            0 => {
                if self.status & STATUS_DRQ == 0 || !self.writing {
                    return;
                }
                self.buffer[self.index] = val;
                if !self.eight_bit {
                    self.buffer[self.index + 1] = 0;
                }
                self.index += if self.eight_bit { 1 } else { 2 };
                if self.index >= SECTOR_SIZE {
                    self.next_sector();
                }
            }
            // the features register shares its address with error; the
            // value waits there for SET FEATURES
            1 => self.error = val,
            2 => self.count = val,
            3..=6 => self.lba[(reg - 3) as usize] = val,
            _ => self.command(val),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(name: &str, sectors: u64) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("emu-cf-test-{}-{name}.img", std::process::id()));
        let mut f = File::create(&p).unwrap();
        // every sector filled with its own index
        for i in 0..sectors {
            f.write_all(&[i as u8; SECTOR_SIZE]).unwrap();
        }
        p
    }

    /// The monitor's setup: 8-bit mode, then a read of sector `lba`.
    fn read_sector(cf: &mut CompactFlash, lba: u32) -> Vec<u8> {
        cf.write(1, FEATURE_8BIT);
        cf.write(7, 0xef);
        cf.write(3, lba as u8);
        cf.write(4, (lba >> 8) as u8);
        cf.write(5, (lba >> 16) as u8);
        cf.write(6, 0xe0 | (lba >> 24) as u8);
        cf.write(2, 1);
        cf.write(7, 0x20);
        assert_ne!(cf.read(7) & STATUS_DRQ, 0);
        (0..SECTOR_SIZE).map(|_| cf.read(0)).collect()
    }

    #[test]
    fn no_card_reads_as_the_floating_bus() {
        let mut cf = CompactFlash::new();
        assert_eq!(cf.read(7), 0xff);
        cf.write(7, 0x20);
        assert_eq!(cf.read(0), 0xff);
    }

    #[test]
    fn a_sector_reads_back_and_the_transfer_ends() {
        let p = image("read", 4);
        let mut cf = CompactFlash::new();
        assert!(cf.mount(&p));
        assert_eq!(cf.read(7), STATUS_DRDY | STATUS_DSC, "ready, idle");
        let got = read_sector(&mut cf, 2);
        assert!(got.iter().all(|&b| b == 2));
        assert_eq!(cf.read(7) & (STATUS_DRQ | STATUS_ERR), 0, "done");
        assert_eq!(cf.read(0), 0, "nothing more to give");
        std::fs::remove_file(&p).ok();
    }

    /// A multi-sector read runs on from one sector to the next, and zero
    /// in the count means 256.
    #[test]
    fn a_multi_sector_read_runs_on() {
        let p = image("multi", 3);
        let mut cf = CompactFlash::new();
        cf.mount(&p);
        cf.write(1, FEATURE_8BIT);
        cf.write(7, 0xef);
        cf.write(3, 1);
        cf.write(4, 0);
        cf.write(5, 0);
        cf.write(6, 0xe0);
        cf.write(2, 2);
        cf.write(7, 0x20);
        let got: Vec<u8> = (0..2 * SECTOR_SIZE).map(|_| cf.read(0)).collect();
        assert!(got[..SECTOR_SIZE].iter().all(|&b| b == 1));
        assert!(got[SECTOR_SIZE..].iter().all(|&b| b == 2));
        assert_eq!(cf.read(7) & STATUS_DRQ, 0);
        cf.write(2, 0);
        cf.write(7, 0x20);
        assert_eq!(cf.remaining, 256);
        std::fs::remove_file(&p).ok();
    }

    /// Writes go through to the file.
    #[test]
    fn a_write_lands_in_the_image() {
        let p = image("write", 4);
        let mut cf = CompactFlash::new();
        cf.mount(&p);
        cf.write(1, FEATURE_8BIT);
        cf.write(7, 0xef);
        cf.write(3, 3);
        cf.write(4, 0);
        cf.write(5, 0);
        cf.write(6, 0xe0);
        cf.write(2, 1);
        cf.write(7, 0x30);
        assert_ne!(cf.read(7) & STATUS_DRQ, 0);
        for i in 0..SECTOR_SIZE {
            cf.write(0, i as u8);
        }
        assert_eq!(cf.read(7) & (STATUS_DRQ | STATUS_ERR), 0);
        let mut back = vec![0u8; SECTOR_SIZE];
        let mut f = File::open(&p).unwrap();
        f.seek(SeekFrom::Start(3 * SECTOR_SIZE as u64)).unwrap();
        f.read_exact(&mut back).unwrap();
        assert!(back.iter().enumerate().all(|(i, &b)| b == i as u8));
        assert!(read_sector(&mut cf, 3).iter().enumerate().all(|(i, &b)| b == i as u8));
        std::fs::remove_file(&p).ok();
    }

    /// Past the end of the image is ID not found; an unknown command
    /// aborts.
    #[test]
    fn errors_show_in_status_and_error() {
        let p = image("err", 2);
        let mut cf = CompactFlash::new();
        cf.mount(&p);
        cf.write(1, FEATURE_8BIT);
        cf.write(7, 0xef);
        cf.write(3, 2);
        cf.write(2, 1);
        cf.write(7, 0x20);
        assert_ne!(cf.read(7) & STATUS_ERR, 0);
        assert_eq!(cf.read(7) & STATUS_DRQ, 0);
        assert_eq!(cf.read(1), ERROR_ID_NOT_FOUND);
        cf.write(7, 0x99);
        assert_eq!(cf.read(1), ERROR_ABORT);
        std::fs::remove_file(&p).ok();
    }

    /// Until SET FEATURES puts the card in 8-bit mode, the eight-bit module
    /// sees only the low byte of every word.
    #[test]
    fn sixteen_bit_mode_shows_every_other_byte() {
        let p = image("wide", 1);
        let mut cf = CompactFlash::new();
        cf.mount(&p);
        cf.write(7, 0xec); // identify: a 16-bit word per access
        let got: Vec<u8> = (0..SECTOR_SIZE / 2).map(|_| cf.read(0)).collect();
        assert_eq!(got[0], 0x8a, "word 0's low byte");
        assert_eq!(got[49], 0x00, "word 49's low byte; its high byte has the LBA bit");
        assert_eq!(cf.read(7) & STATUS_DRQ, 0, "256 accesses drained the sector");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn identify_names_the_capacity() {
        let p = image("ident", 7);
        let mut cf = CompactFlash::new();
        cf.mount(&p);
        cf.write(1, FEATURE_8BIT);
        cf.write(7, 0xef);
        cf.write(7, 0xec);
        let got: Vec<u8> = (0..SECTOR_SIZE).map(|_| cf.read(0)).collect();
        assert_eq!(u16::from_le_bytes([got[120], got[121]]), 7, "word 60");
        assert_eq!(&got[54..58], b"me u", "the model, in ATA's swapped pairs");
        std::fs::remove_file(&p).ok();
    }
}
