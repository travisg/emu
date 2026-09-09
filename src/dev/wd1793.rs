// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Western Digital WD1793 floppy disk controller (Kaypro II).
//!
//! Port of `dev/wd1793.{h,cpp}`. This is a *read-only* model of the chip:
//! Restore, Seek, Read Sector, Read Address and Force Interrupt do what they
//! say; Write Sector takes its bytes through DRQ like a real one and drops
//! them, so a guest's write loop runs to completion and nothing survives it;
//! the rest complete with an interrupt. The image is opened read-only for
//! the same reason; nothing here can modify it.
//!
//! The image geometry is fixed to the Kaypro II single-sided format the C++
//! assumes: 40 tracks x 10 sectors x 512 bytes, sectors numbered from 0.
//!
//! # Timing, and why it is load-bearing
//!
//! On the Kaypro the chip's INTRQ and DRQ are ORed onto the Z80's NMI, and
//! the rom's transfer loops are `HALT; INI; JR NZ`: sleep until the byte's
//! DRQ, take it, sleep again. NMI is edge-triggered, so that only works if
//! each DRQ rises *after* the loop has reached its HALT. A controller that
//! raised the next DRQ the moment the data register was read would fire
//! the NMI at the `JR`, the handler's RET would land on the `HALT`, and the
//! machine would sleep forever waiting for an edge that already came. So
//! the bytes come at the bit rate: 32 us each at the Kaypro's 250 kbit/s
//! MFM, 80 cycles of its 2.5 MHz -- the real chip's spacing, and what the
//! rom's loop was written against. The completion INTRQ follows the last
//! byte by the two CRC bytes' time, Type I commands complete after their
//! stepping at the command's step rate (at least one byte time, the chip's
//! own overhead), and Force Interrupt's interrupt takes a byte time too.
//! Nothing else is timed: the sector is found the moment it is asked for,
//! with no rotation and no head settle.
//!
//! The cycles arrive through `tick`, from the machine's interrupt poll, and
//! the rate they are issued at through `set_pacing_hz` (the resolved
//! `--throttle`), as for the SIO. `--fast-io` does not reach this device:
//! the byte time is what makes the guest's loop work, and at 80 cycles it
//! is already faster than any real drive.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const TRACKS: u32 = 40;
const SECTORS_PER_TRACK: u32 = 10;
const SECTOR_SIZE: usize = 512;

/// Status reads toggle the index pulse bit every this many reads, so a Type I
/// poll loop sees the disk "spinning".
const INDEX_PULSE_PERIOD: u32 = 64;

/// A byte's time on the disk: 32 us at 250 kbit/s MFM.
const BYTE_MICROS: u64 = 32;
/// The Type I step rates, WD1793 with the 1 MHz clock a 5.25" drive uses,
/// by the command's low two bits (datasheet table): 6, 12, 20 and 30 ms.
const STEP_RATE_MS: [u64; 4] = [6, 12, 20, 30];

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
    /// A Write Sector in progress: the data register takes the bytes.
    writing: bool,

    /// Cycles until DRQ rises for the next byte; 0 when none is due.
    drq_in: u32,
    /// Cycles until the command completes with INTRQ; 0 when none is due.
    intrq_in: u32,
    /// The rate machine cycles are issued at, for the times above.
    pacing_hz: u64,

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
            writing: false,
            drq_in: 0,
            intrq_in: 0,
            pacing_hz: crate::system::kaypro::CLOCK_HZ,
            image: None,
        }
    }

    /// Pace the byte and step times against `pacing_hz` cycles to the
    /// second, the resolved `--throttle`.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        self.pacing_hz = pacing_hz;
    }

    fn byte_cycles(&self) -> u32 {
        (self.pacing_hz * BYTE_MICROS / 1_000_000).max(1) as u32
    }

    fn ms_cycles(&self, ms: u64) -> u32 {
        (self.pacing_hz * ms / 1000) as u32
    }

    /// Advance by `elapsed` machine cycles: a byte becomes due, a command
    /// completes.
    pub fn tick(&mut self, elapsed: u32) {
        if self.drq_in > 0 {
            self.drq_in = self.drq_in.saturating_sub(elapsed);
            if self.drq_in == 0 {
                self.drq = true;
            }
        }
        if self.intrq_in > 0 {
            self.intrq_in = self.intrq_in.saturating_sub(elapsed);
            if self.intrq_in == 0 {
                self.intrq = true;
                self.status &= !STATUS_BUSY;
            }
        }
    }

    /// Complete the command with INTRQ after `cycles`.
    fn complete_in(&mut self, cycles: u32) {
        self.intrq_in = cycles.max(1);
    }

    /// The next byte of a transfer: DRQ after a byte time, or, past the
    /// last one, the completion after the CRC's two.
    fn next_byte(&mut self) {
        self.drq = false;
        if self.sector_index >= self.buffer_count {
            self.writing = false;
            self.complete_in(2 * self.byte_cycles());
        } else {
            self.drq_in = self.byte_cycles();
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

    /// The INTRQ output line. On the Kaypro INTRQ and DRQ are ORed onto
    /// the Z80's NMI (`nmi_line`) rather than appearing in a readable port
    /// -- the system latch at 0x1c reads back what was written, see
    /// `system/kaypro.rs`.
    pub fn interrupt_pending(&self) -> bool {
        self.intrq
    }

    /// The DRQ output line; see `interrupt_pending`.
    pub fn data_ready(&self) -> bool {
        self.drq
    }

    /// What the Kaypro puts on the Z80's NMI pin: INTRQ or DRQ.
    pub fn nmi_line(&self) -> bool {
        self.intrq || self.drq
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
                if self.drq && !self.writing && self.sector_index < self.buffer_count {
                    let val = self.sector_bytes[self.sector_index];
                    self.sector_index += 1;
                    self.next_byte();
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
            3 => {
                self.data = val;
                if self.drq && self.writing && self.sector_index < self.buffer_count {
                    // a Write Sector's byte: taken and dropped
                    self.sector_index += 1;
                    self.next_byte();
                }
            }
            _ => {}
        }
    }

    /// Start a transfer of `count` bytes through DRQ, the first due a byte
    /// time from now.
    fn start_transfer(&mut self, count: usize, writing: bool) {
        self.status = STATUS_BUSY;
        self.sector_index = 0;
        self.buffer_count = count;
        self.writing = writing;
        self.drq = false;
        self.drq_in = self.byte_cycles();
    }

    fn process_command(&mut self) {
        let cmd = self.command & 0xf0;
        // a command write clears INTRQ and supersedes whatever was due
        self.intrq = false;
        self.intrq_in = 0;
        self.drq_in = 0;

        if self.command & 0x80 == 0 {
            // Type I: the stepping takes its time, at the command's step
            // rate, and the chip's own overhead is a byte time at least
            let from = self.track;
            match cmd {
                // restore: seek to track 0
                0x00 => self.track = 0,
                // seek: the target track is in the data register
                0x10 => self.track = self.data,
                // step in/out and friends: complete without moving
                _ => {}
            }
            let steps = from.abs_diff(self.track) as u64;
            let stepping = self.ms_cycles(steps * STEP_RATE_MS[(self.command & 0x03) as usize]);
            self.status = STATUS_BUSY;
            self.complete_in(stepping.max(self.byte_cycles()));
        } else if self.command & 0xe0 == 0x80 {
            // Type II: read sector (0x80..=0x9f)
            if !self.read_sector_from_image() {
                println!(
                    "WD1793: Read Sector failed (track {} sector {}), filling with 0xe5",
                    self.track, self.sector
                );
                self.sector_bytes.fill(0xe5);
            }
            self.start_transfer(SECTOR_SIZE, false);
        } else if self.command & 0xe0 == 0xa0 {
            // Type II: write sector (0xa0..=0xbf), into the void
            self.start_transfer(SECTOR_SIZE, true);
        } else if cmd == 0xc0 {
            // Type III: read address -- hand back a synthetic ID field
            self.sector_bytes[0] = self.track;
            self.sector_bytes[1] = 0; // side 0
            self.sector_bytes[2] = self.sector;
            self.sector_bytes[3] = 2; // 512-byte sectors
            self.sector_bytes[4] = 0; // crc
            self.sector_bytes[5] = 0; // crc
            self.start_transfer(6, false);
        } else if cmd == 0xd0 {
            // Type IV: force interrupt. A zero condition field means terminate
            // with no interrupt; any other raises one, after the overhead.
            self.status = 0;
            self.drq = false;
            self.writing = false;
            if self.command & 0x0f != 0 {
                self.complete_in(self.byte_cycles());
            }
        } else {
            // everything else (read/write track): complete after the overhead
            self.status = STATUS_BUSY;
            self.complete_in(self.byte_cycles());
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

    /// A byte time at the Kaypro's own rate.
    const BYTE: u32 = 80;

    #[test]
    fn no_image_reads_not_ready() {
        let mut fdc = Wd1793::new();
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0);
        // and a read sector fills with formatter filler
        fdc.write(0, 0x80);
        fdc.tick(BYTE);
        assert!(fdc.data_ready());
        assert_eq!(fdc.read(3), 0xe5);
    }

    /// Type I commands complete after their stepping -- 6 ms a track at
    /// the command's slowest-clock rate 00 -- with a byte time as the floor,
    /// and INTRQ clears on a status read.
    #[test]
    fn restore_and_seek_complete_after_the_stepping() {
        let mut fdc = Wd1793::new();
        fdc.write(3, 7);
        fdc.write(0, 0x10); // seek to data register
        assert_eq!(fdc.read(1), 7);
        assert_ne!(fdc.read(0) & STATUS_BUSY, 0);
        fdc.tick(7 * 15_000 - 1);
        assert!(!fdc.interrupt_pending(), "seven tracks at 6 ms");
        fdc.tick(1);
        assert!(fdc.interrupt_pending());
        assert!(fdc.read(0) & (STATUS_TRACK0 | STATUS_BUSY) == 0);
        assert!(!fdc.interrupt_pending(), "reading status clears intrq");
        fdc.write(0, 0x00); // restore
        assert_eq!(fdc.read(1), 0);
        fdc.tick(7 * 15_000);
        assert!(fdc.interrupt_pending());
        assert_ne!(fdc.read(0) & STATUS_TRACK0, 0);
        // a seek that goes nowhere still takes the chip's overhead
        fdc.write(3, 0);
        fdc.write(0, 0x10);
        fdc.tick(BYTE - 1);
        assert!(!fdc.interrupt_pending());
        fdc.tick(1);
        assert!(fdc.interrupt_pending());
    }

    /// The bytes come a byte time apart, each DRQ rising only after the
    /// previous byte was taken -- the edge the Kaypro's HALT loop sleeps
    /// for -- and the completion follows the last by the CRC's two.
    #[test]
    fn read_sector_streams_the_right_bytes_a_byte_time_apart() {
        let p = image_with_pattern("stream");
        let mut fdc = Wd1793::new();
        assert!(fdc.load_image(&p));

        fdc.write(1, 3); // track
        fdc.write(2, 4); // sector
        fdc.write(0, 0x88); // read sector
        assert_ne!(fdc.read(0) & STATUS_BUSY, 0);
        assert!(!fdc.data_ready(), "the first byte takes its time too");
        fdc.tick(BYTE);
        assert!(fdc.data_ready());
        assert_ne!(fdc.read(0) & STATUS_INDEX_OR_DRQ, 0, "drq shows in type II status");

        let expected = (3 * SECTORS_PER_TRACK + 4) as u8;
        for _ in 0..SECTOR_SIZE - 1 {
            assert_eq!(fdc.read(3), expected);
            assert!(!fdc.data_ready(), "taken: drq drops");
            fdc.tick(BYTE - 1);
            assert!(!fdc.data_ready());
            fdc.tick(1);
            assert!(fdc.data_ready());
        }
        assert_eq!(fdc.read(3), expected);
        assert!(!fdc.data_ready());
        assert!(!fdc.interrupt_pending());
        assert_ne!(fdc.read(0) & STATUS_BUSY, 0, "still busy through the crc");
        fdc.tick(2 * BYTE);
        assert!(fdc.interrupt_pending());
        assert_eq!(fdc.read(0) & STATUS_BUSY, 0);

        // past the end the data register reads back the last written value
        fdc.write(3, 0x42);
        assert_eq!(fdc.read(3), 0x42);
        std::fs::remove_file(&p).ok();
    }

    /// A write takes its bytes through DRQ at the same pace and drops
    /// them: the loop runs, the image is untouched.
    #[test]
    fn write_sector_takes_the_bytes_and_drops_them() {
        let p = image_with_pattern("write");
        let mut fdc = Wd1793::new();
        fdc.load_image(&p);
        fdc.write(1, 1);
        fdc.write(2, 1);
        fdc.write(0, 0xa8);
        for _ in 0..SECTOR_SIZE {
            fdc.tick(BYTE);
            assert!(fdc.data_ready());
            fdc.write(3, 0x99);
            assert!(!fdc.data_ready());
        }
        fdc.tick(2 * BYTE);
        assert!(fdc.interrupt_pending());
        assert_eq!(fdc.read(0) & STATUS_BUSY, 0);
        fdc.write(0, 0x88);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), (SECTORS_PER_TRACK + 1) as u8, "unchanged");
        std::fs::remove_file(&p).ok();
    }

    /// The pacing rate rescales the byte time: at a tenth of the clock a
    /// byte is 8 cycles.
    #[test]
    fn the_byte_time_follows_the_pacing_rate() {
        let mut fdc = Wd1793::new();
        fdc.set_pacing_hz(250_000);
        fdc.write(0, 0x80);
        fdc.tick(7);
        assert!(!fdc.data_ready());
        fdc.tick(1);
        assert!(fdc.data_ready());
    }

    #[test]
    fn out_of_range_track_fills_with_filler() {
        let p = image_with_pattern("range");
        let mut fdc = Wd1793::new();
        fdc.load_image(&p);
        fdc.write(1, 40);
        fdc.write(0, 0x80);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), 0xe5);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn read_address_yields_six_bytes() {
        let mut fdc = Wd1793::new();
        fdc.write(1, 5);
        fdc.write(2, 2);
        fdc.write(0, 0xc0);
        let got: Vec<u8> = (0..6)
            .map(|_| {
                fdc.tick(BYTE);
                fdc.read(3)
            })
            .collect();
        assert_eq!(got, vec![5, 0, 2, 2, 0, 0]);
        assert!(!fdc.data_ready());
    }

    /// Force Interrupt with no condition terminates silently; with one it
    /// interrupts after the overhead. Either way it cancels what was due.
    #[test]
    fn force_interrupt_condition_field() {
        let mut fdc = Wd1793::new();
        fdc.write(0, 0x80);
        fdc.write(0, 0xd0);
        fdc.tick(10 * BYTE);
        assert!(!fdc.interrupt_pending());
        assert!(!fdc.data_ready(), "the read was cancelled");
        assert_eq!(fdc.read(0) & STATUS_BUSY, 0);
        fdc.write(0, 0xd8);
        assert!(!fdc.interrupt_pending());
        fdc.tick(BYTE);
        assert!(fdc.interrupt_pending());
        assert!(fdc.nmi_line());
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
