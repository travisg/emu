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
//! Western Digital WD1793 floppy disk controller (Kaypro II).
//!
//! Port of `dev/wd1793.{h,cpp}`. This is a *read-only* model of the chip: the
//! C++ command decoder handles Restore, Seek, Read Sector, Read Address and
//! Force Interrupt, and treats every other command (including Write Sector) as
//! "complete immediately". The image is opened read-only for the same reason;
//! nothing here can modify it.
//!
//! The image geometry is fixed to the Kaypro II single-sided format the C++
//! assumes: 40 tracks x 10 sectors x 512 bytes, sectors numbered from 0.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TRACKS: u32 = 40;
const SECTORS_PER_TRACK: u32 = 10;
const SECTOR_SIZE: usize = 512;

/// Status reads toggle the index pulse bit every this many reads, so a Type I
/// poll loop sees the disk "spinning".
const INDEX_PULSE_PERIOD: u32 = 64;

// status register bits
const STATUS_BUSY: u8 = 1 << 0;
const STATUS_INDEX_OR_DRQ: u8 = 1 << 1;
const STATUS_TRACK0: u8 = 1 << 2;
const STATUS_NOT_READY: u8 = 1 << 7;

pub struct Wd1793 {
    status: u8,
    track: u8,
    sector: u8,
    data: u8,
    command: u8,

    intrq: bool,
    drq: bool,
    index_pulse: bool,
    /// The C++ keeps this counter in a function-local `static`; there is only
    /// ever one controller so a field is equivalent.
    index_counter: u32,
    selected: bool,

    sector_index: usize,
    buffer_count: usize,
    sector_bytes: [u8; SECTOR_SIZE],

    image: Option<File>,
}

impl Default for Wd1793 {
    fn default() -> Self {
        Self::new()
    }
}

impl Wd1793 {
    pub fn new() -> Self {
        Wd1793 {
            status: 0,
            track: 0,
            sector: 0,
            data: 0,
            command: 0,
            intrq: false,
            drq: false,
            index_pulse: false,
            index_counter: 0,
            selected: true,
            sector_index: 0,
            buffer_count: 0,
            sector_bytes: [0; SECTOR_SIZE],
            image: None,
        }
    }

    /// Attach a disk image. A missing image is not fatal -- as in the C++,
    /// the controller just reports Not Ready and reads come back as `0xe5`
    /// filler -- so this reports rather than errors.
    pub fn load_image(&mut self, path: &Path) -> bool {
        match File::open(path) {
            Ok(f) => {
                println!("WD1793: loaded image '{}'", path.display());
                self.image = Some(f);
                true
            }
            Err(e) => {
                println!("WD1793: failed to open image '{}': {e}", path.display());
                self.image = None;
                false
            }
        }
    }

    pub fn has_image(&self) -> bool {
        self.image.is_some()
    }

    /// INTRQ line, as the machine's status latch reads it back.
    pub fn interrupt_pending(&self) -> bool {
        self.intrq
    }

    /// DRQ line, as the machine's status latch reads it back.
    pub fn data_ready(&self) -> bool {
        self.drq
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Register read: 0 status, 1 track, 2 sector, 3 data.
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg {
            0 => {
                let mut val = self.status;

                // not ready if there's no disk or the drive isn't selected
                if self.image.is_none() || !self.selected {
                    val |= STATUS_NOT_READY;
                }

                if self.command & 0x80 == 0 {
                    // Type I (or IV) context: bit 1 is index pulse, bit 2 is
                    // track 0
                    if self.track == 0 {
                        val |= STATUS_TRACK0;
                    }
                    self.index_counter += 1;
                    if self.index_counter > INDEX_PULSE_PERIOD {
                        self.index_pulse = !self.index_pulse;
                        self.index_counter = 0;
                    }
                    if self.index_pulse {
                        val |= STATUS_INDEX_OR_DRQ;
                    }
                } else if self.drq {
                    // Type II/III context: bit 1 is DRQ
                    val |= STATUS_INDEX_OR_DRQ;
                }

                // reading status clears the interrupt
                self.intrq = false;
                val
            }
            1 => self.track,
            2 => self.sector,
            3 => {
                if self.drq && self.sector_index < self.buffer_count {
                    let val = self.sector_bytes[self.sector_index];
                    self.sector_index += 1;
                    if self.sector_index >= self.buffer_count {
                        // buffer drained: operation complete
                        self.drq = false;
                        self.intrq = true;
                        self.status = 0;
                    }
                    val
                } else {
                    self.data
                }
            }
            _ => 0,
        }
    }

    /// Register write: 0 command, 1 track, 2 sector, 3 data.
    pub fn write(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.command = val;
                self.process_command();
            }
            1 => self.track = val,
            2 => self.sector = val,
            3 => self.data = val,
            _ => {}
        }
    }

    fn process_command(&mut self) {
        let cmd = self.command & 0xf0;

        if self.command & 0x80 == 0 {
            // Type I
            match cmd {
                0x00 => {
                    // restore: seek to track 0
                    self.track = 0;
                    self.status = STATUS_TRACK0;
                    self.intrq = true;
                }
                0x10 => {
                    // seek: the target track is in the data register
                    self.track = self.data;
                    self.status = 0;
                    self.intrq = true;
                }
                _ => {
                    // step in/out and friends: complete immediately
                    self.intrq = true;
                }
            }
        } else if self.command & 0xe0 == 0x80 {
            // Type II: read sector (0x80..=0x9f)
            self.status = STATUS_BUSY;
            self.intrq = false;
            self.sector_index = 0;
            self.buffer_count = SECTOR_SIZE;

            if !self.read_sector_from_image() {
                println!(
                    "WD1793: Read Sector failed (track {} sector {}), filling with 0xe5",
                    self.track, self.sector
                );
                self.sector_bytes.fill(0xe5);
            }
            self.drq = true;
        } else if cmd == 0xc0 {
            // Type III: read address -- hand back a synthetic ID field
            self.status = STATUS_BUSY;
            self.intrq = false;
            self.sector_index = 0;
            self.buffer_count = 6;
            self.sector_bytes[0] = self.track;
            self.sector_bytes[1] = 0; // side 0
            self.sector_bytes[2] = self.sector;
            self.sector_bytes[3] = 2; // 512-byte sectors
            self.sector_bytes[4] = 0; // crc
            self.sector_bytes[5] = 0; // crc
            self.drq = true;
        } else if cmd == 0xd0 {
            // Type IV: force interrupt. A zero condition field means terminate
            // with no interrupt.
            self.status = 0;
            self.intrq = self.command & 0x0f != 0;
        } else {
            // everything else, including write sector: complete immediately
            self.status = 0;
            self.intrq = true;
        }
    }

    /// Fill the sector buffer from the image at the current track/sector.
    /// Sectors are numbered from 0 (Kaypro II convention).
    fn read_sector_from_image(&mut self) -> bool {
        let Some(image) = self.image.as_mut() else {
            return false;
        };
        let track = self.track as u32;
        if track >= TRACKS {
            return false;
        }
        let sector = self.sector as u32 % SECTORS_PER_TRACK;
        let offset = (track * SECTORS_PER_TRACK + sector) as u64 * SECTOR_SIZE as u64;
        if image.seek(SeekFrom::Start(offset)).is_err() {
            return false;
        }
        image.read_exact(&mut self.sector_bytes).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn image_with_pattern(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("emu-wd1793-test-{}-{name}.img", std::process::id()));
        let mut f = File::create(&p).unwrap();
        // every sector filled with its own linear index, so reads are checkable
        let mut buf = vec![0u8; (TRACKS * SECTORS_PER_TRACK) as usize * SECTOR_SIZE];
        for (i, chunk) in buf.chunks_mut(SECTOR_SIZE).enumerate() {
            chunk.fill(i as u8);
        }
        f.write_all(&buf).unwrap();
        p
    }

    #[test]
    fn no_image_reads_not_ready() {
        let mut fdc = Wd1793::new();
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0);
        // and a read sector fills with formatter filler
        fdc.write(0, 0x80);
        assert!(fdc.data_ready());
        assert_eq!(fdc.read(3), 0xe5);
    }

    #[test]
    fn restore_and_seek() {
        let mut fdc = Wd1793::new();
        fdc.write(3, 7);
        fdc.write(0, 0x10); // seek to data register
        assert_eq!(fdc.read(1), 7);
        assert!(fdc.interrupt_pending());
        assert!(fdc.read(0) & STATUS_TRACK0 == 0);
        assert!(!fdc.interrupt_pending(), "reading status clears intrq");
        fdc.write(0, 0x00); // restore
        assert_eq!(fdc.read(1), 0);
        assert_ne!(fdc.read(0) & STATUS_TRACK0, 0);
    }

    #[test]
    fn read_sector_streams_the_right_bytes_and_completes() {
        let p = image_with_pattern("stream");
        let mut fdc = Wd1793::new();
        assert!(fdc.load_image(&p));

        fdc.write(1, 3); // track
        fdc.write(2, 4); // sector
        fdc.write(0, 0x88); // read sector
        assert_ne!(fdc.read(0) & STATUS_BUSY, 0);
        assert_ne!(fdc.read(0) & STATUS_INDEX_OR_DRQ, 0, "drq shows in type II status");

        let expected = (3 * SECTORS_PER_TRACK + 4) as u8;
        for _ in 0..SECTOR_SIZE - 1 {
            assert_eq!(fdc.read(3), expected);
            assert!(fdc.data_ready());
        }
        assert_eq!(fdc.read(3), expected);
        assert!(!fdc.data_ready());
        assert!(fdc.interrupt_pending());
        assert_eq!(fdc.read(0) & STATUS_BUSY, 0);

        // past the end the data register reads back the last written value
        fdc.write(3, 0x42);
        assert_eq!(fdc.read(3), 0x42);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn out_of_range_track_fills_with_filler() {
        let p = image_with_pattern("range");
        let mut fdc = Wd1793::new();
        fdc.load_image(&p);
        fdc.write(1, 40);
        fdc.write(0, 0x80);
        assert_eq!(fdc.read(3), 0xe5);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn read_address_yields_six_bytes() {
        let mut fdc = Wd1793::new();
        fdc.write(1, 5);
        fdc.write(2, 2);
        fdc.write(0, 0xc0);
        let got: Vec<u8> = (0..6).map(|_| fdc.read(3)).collect();
        assert_eq!(got, vec![5, 0, 2, 2, 0, 0]);
        assert!(!fdc.data_ready());
    }

    #[test]
    fn force_interrupt_condition_field() {
        let mut fdc = Wd1793::new();
        fdc.write(0, 0xd0);
        assert!(!fdc.interrupt_pending());
        fdc.write(0, 0xd8);
        assert!(fdc.interrupt_pending());
    }

    #[test]
    fn index_pulse_toggles_under_polling() {
        let mut fdc = Wd1793::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..(INDEX_PULSE_PERIOD * 4) {
            seen.insert(fdc.read(0) & STATUS_INDEX_OR_DRQ);
        }
        assert_eq!(seen.len(), 2, "index pulse never toggled");
    }
}
