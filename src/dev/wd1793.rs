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
//! Port of `dev/wd1793.{h,cpp}`. Restore, Seek, Read Sector, Write Sector,
//! Read Address and Force Interrupt do what they say; the rest complete with
//! an interrupt. A written sector goes through to the image when its last
//! byte is in -- to the file, or to the in-memory copy of a disk mounted
//! copy-on-write ([`DiskImage`]). A write-protected disk -- a file that
//! could only be opened read-only -- shows Write Protect in the status
//! register, and a Write Sector to it ends at once with that bit set and
//! nothing transferred, as on the chip. On the Kaypro that is a hang, as
//! it was on the machine: the rom's write loop sleeps in `HALT` for each
//! byte's DRQ, the refusal's INTRQ wakes only the first, and the line then
//! stays high with no edge to come.
//!
//! The image geometry is fixed to the Kaypro II single-sided format the C++
//! assumes: 40 tracks x 10 sectors x 512 bytes, sectors numbered from 0.
//!
//! Two drives hang off the chip, A and B, and the machine's latch selects
//! one (or neither: not ready). Either may be empty, and the window's drive
//! doors swap images at run time through `mount`. The chip has one track
//! register between them, as the real one does -- the BIOS re-seeks after a
//! drive change. What the bay window shows -- the image in each slot, and
//! the drive's front light -- is published through an attached
//! [`DriveBayState`]. The light is the drive's select line gated by the
//! motor: the Tandon lights it from DRIVE SELECT, but the Kaypro's BIOS
//! leaves the last drive selected between accesses and only drops the
//! motor bit, and on the machine the light goes out with the motors.
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

use crate::console::{DiskImage, DriveBayState, DRIVE_BAYS};

use std::path::Path;

const TRACKS: u32 = 40;
const SECTORS_PER_TRACK: u32 = 10;
const SECTOR_SIZE: usize = 512;
/// The size of an image in the fixed geometry, which is what a file has
/// to be to go in a drive.
pub const IMAGE_SIZE: u64 = TRACKS as u64 * SECTORS_PER_TRACK as u64 * SECTOR_SIZE as u64;

/// Drives on the chip. The bay window draws exactly this many.
pub const DRIVES: usize = DRIVE_BAYS;

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
const STATUS_RECORD_NOT_FOUND: u8 = 1 << 4;
const STATUS_WRITE_FAULT: u8 = 1 << 5;
const STATUS_WRITE_PROTECT: u8 = 1 << 6;
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
    /// The drive the machine's latch has selected, if any.
    selected: Option<usize>,
    /// The latch's motor bit: with it off the selected drive's light is
    /// out.
    motor: bool,

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

    /// The disk in each drive, with the path it came from for the bay's
    /// label.
    images: [Option<DiskImage>; DRIVES],
    /// The bay window's lamps and labels, when there is a window.
    bay: Option<DriveBayState>,
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
            selected: Some(0),
            motor: true,
            sector_index: 0,
            buffer_count: 0,
            sector_bytes: [0; SECTOR_SIZE],
            writing: false,
            drq_in: 0,
            intrq_in: 0,
            pacing_hz: crate::system::kaypro::CLOCK_HZ,
            images: [None, None],
            bay: None,
        }
    }

    /// Attach the bay window's state. Syncs the labels from whatever is
    /// already mounted, so it does not matter whether the images went in
    /// first.
    pub fn attach_bay(&mut self, bay: DriveBayState) {
        for (drive, image) in self.images.iter().enumerate() {
            bay.set_mounted(drive, image.as_ref().map(|i| i.path.clone()));
        }
        self.bay = Some(bay);
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
        if let Some(bay) = &self.bay {
            bay.advance(elapsed);
            if let (Some(drive), true) = (self.selected, self.motor) {
                bay.charge(drive, elapsed);
            }
        }
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
            if self.writing {
                if let Err(bit) = self.write_sector_to_image() {
                    println!("WD1793: Write Sector failed (track {} sector {})", self.track, self.sector);
                    self.status |= bit;
                }
            }
            self.writing = false;
            self.complete_in(2 * self.byte_cycles());
        } else {
            self.drq_in = self.byte_cycles();
        }
    }

    /// Put a disk image in `drive` at startup, copy-on-write if
    /// `copy_on_write`. A missing image is not fatal -- as in the C++, the
    /// drive just reports Not Ready and reads come back as `0xe5` filler --
    /// so this reports rather than errors.
    pub fn load_image(&mut self, drive: usize, path: &Path, copy_on_write: bool) -> bool {
        let path = path.to_path_buf();
        let image = if copy_on_write { DiskImage::open_copy_on_write(path.clone()) } else { DiskImage::open(path.clone()) };
        match image {
            Ok(image) => {
                println!("WD1793: drive {}: loaded image '{}'{}", drive_letter(drive), path.display(), image_note(&image));
                self.mount(drive, Some(image));
                true
            }
            Err(e) => {
                println!("WD1793: drive {}: failed to open image '{}': {e}", drive_letter(drive), path.display());
                self.mount(drive, None);
                false
            }
        }
    }

    /// Change the disk in `drive`: `image` goes in, or `None` leaves it
    /// empty. A transfer in flight keeps the sector it already read; the
    /// next one finds whatever is in the slot now, as with a real disk
    /// pulled mid-run.
    pub fn mount(&mut self, drive: usize, image: Option<DiskImage>) {
        if let Some(bay) = &self.bay {
            bay.set_mounted(drive, image.as_ref().map(|i| i.path.clone()));
        }
        self.images[drive] = image;
    }

    pub fn has_image(&self, drive: usize) -> bool {
        self.images[drive].is_some()
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

    /// The machine's drive select: which drive's select line is asserted,
    /// or none, and whether the motors are on. The selected drive's light
    /// is lit for as long as both hold.
    pub fn select(&mut self, drive: Option<usize>, motor: bool) {
        self.selected = drive;
        self.motor = motor;
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Register read: 0 status, 1 track, 2 sector, 3 data.
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg {
            0 => {
                let mut val = self.status;

                // not ready if no drive is selected, the selected one is
                // empty, or the motors are off (READY needs the spindle)
                if !self.motor || !self.selected.is_some_and(|d| self.images[d].is_some()) {
                    val |= STATUS_NOT_READY;
                }

                if self.command & 0x80 == 0 {
                    // Type I (or IV) context: bit 1 is index pulse, bit 2 is
                    // track 0
                    if self.track == 0 {
                        val |= STATUS_TRACK0;
                    }
                    if self.selected_image().is_some_and(|i| i.write_protected) {
                        val |= STATUS_WRITE_PROTECT;
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
                    // a Write Sector's byte, into the buffer the last one
                    // flushes
                    self.sector_bytes[self.sector_index] = val;
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
            // Type II: write sector (0xa0..=0xbf). A protected disk ends
            // the command before the first DRQ.
            if self.selected_image().is_some_and(|i| i.write_protected) {
                self.status = STATUS_BUSY | STATUS_WRITE_PROTECT;
                self.complete_in(self.byte_cycles());
            } else {
                self.start_transfer(SECTOR_SIZE, true);
            }
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

    fn selected_image(&self) -> Option<&DiskImage> {
        self.selected.and_then(|d| self.images[d].as_ref())
    }

    /// Where the current track/sector starts in an image, if the track is
    /// on the disk. Sectors are numbered from 0 (Kaypro II convention).
    fn sector_offset(&self) -> Option<u64> {
        let track = self.track as u32;
        if track >= TRACKS {
            return None;
        }
        let sector = self.sector as u32 % SECTORS_PER_TRACK;
        Some((track * SECTORS_PER_TRACK + sector) as u64 * SECTOR_SIZE as u64)
    }

    /// Fill the sector buffer from the image at the current track/sector.
    fn read_sector_from_image(&mut self) -> bool {
        let Some(offset) = self.sector_offset() else {
            return false;
        };
        let Some(image) = self.selected.and_then(|d| self.images[d].as_mut()) else {
            return false;
        };
        image.read_at(offset, &mut self.sector_bytes).is_ok()
    }

    /// Write the sector buffer through to the image at the current
    /// track/sector. Err is the status bit that says why not: no such
    /// sector (no disk, or a track off its end), or the file refused it.
    fn write_sector_to_image(&mut self) -> Result<(), u8> {
        let offset = self.sector_offset().ok_or(STATUS_RECORD_NOT_FOUND)?;
        let image = self.selected.and_then(|d| self.images[d].as_mut()).ok_or(STATUS_RECORD_NOT_FOUND)?;
        image.write_at(offset, &self.sector_bytes).map_err(|e| match e.kind() {
            std::io::ErrorKind::UnexpectedEof => STATUS_RECORD_NOT_FOUND,
            _ => STATUS_WRITE_FAULT,
        })
    }
}

/// What the startup and insert messages say about how a disk is mounted.
pub fn image_note(image: &DiskImage) -> &'static str {
    if image.is_copy_on_write() {
        " (copy-on-write: writes are kept in memory, not in the file)"
    } else if image.write_protected {
        " (write-protected)"
    } else {
        ""
    }
}

/// The drive's letter, as CP/M and the bezel name it.
pub fn drive_letter(drive: usize) -> char {
    (b'A' + drive as u8) as char
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
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
        assert!(fdc.load_image(0, &p, false));

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

    /// Write sector `sector` of `track` with `byte`, a byte time apart as
    /// the data register asks, and wait out the completion.
    fn write_sector(fdc: &mut Wd1793, track: u8, sector: u8, byte: u8) {
        fdc.write(1, track);
        fdc.write(2, sector);
        fdc.write(0, 0xa8);
        for _ in 0..SECTOR_SIZE {
            fdc.tick(BYTE);
            assert!(fdc.data_ready());
            fdc.write(3, byte);
            assert!(!fdc.data_ready());
        }
        assert!(!fdc.interrupt_pending());
        fdc.tick(2 * BYTE);
        assert!(fdc.interrupt_pending());
    }

    /// A write takes its bytes through DRQ at the same pace as a read and
    /// lands in the image file: a read gives them back, the file holds
    /// them, and the neighbouring sectors are untouched.
    #[test]
    fn write_sector_goes_through_to_the_image() {
        let p = image_with_pattern("write");
        let mut fdc = Wd1793::new();
        fdc.load_image(0, &p, false);
        assert_eq!(fdc.read(0) & STATUS_WRITE_PROTECT, 0);
        write_sector(&mut fdc, 1, 1, 0x99);
        assert_eq!(fdc.read(0) & (STATUS_BUSY | STATUS_RECORD_NOT_FOUND | STATUS_WRITE_FAULT), 0);
        fdc.write(0, 0x88);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), 0x99, "read back");
        let on_disk = std::fs::read(&p).unwrap();
        let at = (SECTORS_PER_TRACK + 1) as usize * SECTOR_SIZE;
        assert!(on_disk[at..at + SECTOR_SIZE].iter().all(|&b| b == 0x99), "in the file");
        assert_eq!(on_disk[at - 1], SECTORS_PER_TRACK as u8, "the sector before is untouched");
        assert_eq!(on_disk[at + SECTOR_SIZE], (SECTORS_PER_TRACK + 2) as u8, "and the one after");
        std::fs::remove_file(&p).ok();
    }

    /// A file that cannot be opened for writing goes in write-protected,
    /// which shows in Type I status; a Write Sector to it ends after the
    /// overhead with that bit set, no DRQ ever raised and the file
    /// untouched.
    #[test]
    fn a_read_only_file_is_a_write_protected_disk() {
        let p = image_with_pattern("protect");
        let mut perms = std::fs::metadata(&p).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&p, perms).unwrap();
        let mut fdc = Wd1793::new();
        assert!(fdc.load_image(0, &p, false), "mounted all the same");
        assert_ne!(fdc.read(0) & STATUS_WRITE_PROTECT, 0, "type I status shows the tab");
        fdc.write(1, 1);
        fdc.write(2, 1);
        fdc.write(0, 0xa8);
        fdc.tick(BYTE);
        assert!(!fdc.data_ready(), "no byte is asked for");
        assert!(fdc.interrupt_pending());
        let status = fdc.read(0);
        assert_ne!(status & STATUS_WRITE_PROTECT, 0);
        assert_eq!(status & STATUS_BUSY, 0);
        let on_disk = std::fs::read(&p).unwrap();
        let at = (SECTORS_PER_TRACK + 1) as usize * SECTOR_SIZE;
        assert_eq!(on_disk[at], (SECTORS_PER_TRACK + 1) as u8);
        std::fs::remove_file(&p).ok();
    }

    /// A disk mounted copy-on-write takes writes like any other and gives
    /// them back on a read, but the file never sees them.
    #[test]
    fn a_copy_on_write_disk_keeps_its_writes_out_of_the_file() {
        let p = image_with_pattern("cow");
        let before = std::fs::read(&p).unwrap();
        let mut fdc = Wd1793::new();
        fdc.load_image(0, &p, true);
        assert_eq!(fdc.read(0) & STATUS_WRITE_PROTECT, 0, "not protected: writable");
        write_sector(&mut fdc, 2, 5, 0x77);
        assert_eq!(fdc.read(0) & (STATUS_RECORD_NOT_FOUND | STATUS_WRITE_FAULT), 0);
        fdc.write(0, 0x88);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), 0x77, "the session sees its write");
        assert_eq!(std::fs::read(&p).unwrap(), before, "the file does not");
        write_sector(&mut fdc, TRACKS as u8, 0, 0x55);
        assert_ne!(fdc.read(0) & STATUS_RECORD_NOT_FOUND, 0, "and the copy has the disk's bounds");
        std::fs::remove_file(&p).ok();
    }

    /// A write to a track off the end of the disk, or with no disk in the
    /// drive, takes its bytes and ends with Record Not Found.
    #[test]
    fn a_write_with_nowhere_to_go_is_record_not_found() {
        let p = image_with_pattern("rnf");
        let mut fdc = Wd1793::new();
        fdc.load_image(0, &p, false);
        write_sector(&mut fdc, TRACKS as u8, 0, 0x55);
        assert_ne!(fdc.read(0) & STATUS_RECORD_NOT_FOUND, 0, "off the end");
        assert_eq!(std::fs::metadata(&p).unwrap().len(), IMAGE_SIZE, "the file did not grow");
        fdc.mount(0, None);
        write_sector(&mut fdc, 0, 0, 0x55);
        assert_ne!(fdc.read(0) & STATUS_RECORD_NOT_FOUND, 0, "no disk");
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
        fdc.load_image(0, &p, false);
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

    /// The latch's select picks the slot a read comes from, an empty
    /// slot or no selection reads not ready, and a mount at run time is
    /// what the next read finds. The one track register is shared.
    #[test]
    fn two_drives_read_their_own_images_and_swap_at_run_time() {
        let p = image_with_pattern("two");
        let mut fdc = Wd1793::new();
        fdc.select(None, true);
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0, "nothing selected");
        fdc.select(Some(1), true);
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0, "B is empty");
        fdc.mount(1, Some(DiskImage::open(p.clone()).unwrap()));
        assert!(fdc.has_image(1));
        assert!(!fdc.has_image(0));
        assert_eq!(fdc.read(0) & STATUS_NOT_READY, 0);
        fdc.write(1, 2);
        fdc.write(2, 3);
        fdc.write(0, 0x88);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), (2 * SECTORS_PER_TRACK + 3) as u8);
        // A, still empty, reads filler off the same track register
        fdc.write(0, 0xd0);
        fdc.select(Some(0), true);
        assert_eq!(fdc.read(1), 2, "one track register for both drives");
        fdc.write(0, 0x88);
        fdc.tick(BYTE);
        assert_eq!(fdc.read(3), 0xe5);
        // ejecting B empties it again
        fdc.write(0, 0xd0);
        fdc.mount(1, None);
        fdc.select(Some(1), true);
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0);
        std::fs::remove_file(&p).ok();
    }

    /// The bay's light counters follow the select line while the motor is
    /// on, and its labels the mounts -- including one made before the bay
    /// was attached.
    #[test]
    fn the_bay_sees_the_select_line_and_the_mounted_names() {
        let p = image_with_pattern("bay");
        let mut fdc = Wd1793::new();
        fdc.load_image(0, &p, false);
        let bay = DriveBayState::new();
        fdc.attach_bay(bay.clone());
        assert_eq!(bay.mounted(0), Some(p.clone()), "attach syncs an earlier mount");
        assert_eq!(bay.mounted(1), None);
        fdc.select(Some(1), true);
        fdc.tick(30);
        fdc.select(None, true);
        fdc.tick(50);
        fdc.select(Some(1), false);
        fdc.tick(20);
        let s = bay.snapshot();
        assert_eq!(s.cycles, 100);
        assert_eq!(s.select_on, [0, 30], "selected with the motor off is dark");
        assert_ne!(fdc.read(0) & STATUS_NOT_READY, 0, "and, motor off, not ready");
        fdc.mount(0, None);
        assert_eq!(bay.mounted(0), None);
        std::fs::remove_file(&p).ok();
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
