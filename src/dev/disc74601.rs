// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Raytheon 74601/74602 fixed-head disc, DIO device 1.
//!
//! The mass storage device of the 703/706 line: up to four head-per-track
//! drives on one controller, 64 tracks of 100,000 bits each at 1800 RPM.
//! With the standard sectoring -- 128 sectors of 47 words, the alternative
//! jumperings are not modelled -- that is 385,024 words a drive, reachable
//! with no seek at an average of half a revolution, 16.7 ms.
//!
//! The programming model is the 706 User's Manual section 5-9, "DISC MEMORY",
//! and every function and status bit here cites its 5-9.x. The 703 predates
//! that manual but drives the same controller: the 1969 sales brochure
//! (SP-244D) quotes this drive's 6.4 million raw bits and 16.7 ms for the 703,
//! and X-RAY's disk driver -- on the X-RAY listing's pages titled "NOT
//! ASSEMBLED FOR NON-MASS SYSTEMS", printed even though the transcribed build
//! skips them -- stuffs exactly section 5-9.5's DOT sequence and computes its
//! sector arithmetic with `DXS 47` (card 1055). Where the manual is silent,
//! that driver is the oracle, and each such divergence is commented at its
//! use site.
//!
//! This is the tree's first DMA-style device. Unlike the teletype's
//! word-per-DIN conversation, the controller is told a core address, a disc
//! address and a word count, and then moves the whole transfer itself,
//! interrupting once at completion (5-9.5.4/5). The machine hands `poll` a
//! `&mut Memory` for exactly that reason; the copy happens all at once when
//! the transfer's time expires, because the only thing period software ever
//! observes is the completion interrupt -- X-RAY's `M.DSTAT` loops on the
//! status word and its data lives untouched until the I/O-done routine runs.
//!
//! Timing follows the teletype's model: a rate in machine cycles, not a
//! mechanism. A transfer completes an average access (half a revolution) plus
//! its word time after the starting DOT. A rotational-position model would
//! charge sequential transfers honestly less; nothing in the tree can tell
//! the difference today, so the refinement waits for software that can.

use crate::bus::MemoryDevice;
use crate::cpu::ray703::CLOCK_HZ;
use crate::dev::memory::Memory;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// DIO device code (Table 5-28: every disc function byte is 1x).
pub const DEV_DISC: u8 = 0x1;

/// "Up to four disc drives may be attached to the disc controller" (5-9).
pub const DISC_UNITS: usize = 4;

/// The standard sectoring (5-9.4): "most units use 128 sectors per track and
/// 47 words per sector so that programming may be standardized". 47 * 16 data
/// bits + 13 CRC + 8 gap = 773 bits a sector, 128 of them in a track's
/// 100,000 bits with the remainder left as read-amplifier slack.
const TRACKS: usize = 64;
const SECTORS_PER_TRACK: usize = 128;
const WORDS_PER_SECTOR: usize = 47;

/// 64 * 128 * 47 (5-9): what "up to 385,024 words of auxiliary storage" is.
pub const WORDS_PER_UNIT: usize = TRACKS * SECTORS_PER_TRACK * WORDS_PER_SECTOR;

/// An image file is the drive exactly: big-endian words, the same layout as
/// core and as a `-s ray703` image, so a transfer is a plain byte copy.
pub const IMAGE_BYTES: usize = WORDS_PER_UNIT * 2;

/// DOT functions (Table 5-28; the port byte is device << 4 | function).
const FN_DISCONNECT: u8 = 0x0; // 5-9.5.1
const FN_SET_MEM_ADDR: u8 = 0x1; // 5-9.5.2
const FN_SET_TRACK_SECTOR: u8 = 0x2; // 5-9.5.3
const FN_WRITE: u8 = 0x4; // 5-9.5.4
const FN_READ: u8 = 0x6; // 5-9.5.5
const FN_VERIFY: u8 = 0x7; // 5-9.5.6

/// DIN functions 0-3 return the status of that unit (5-9.7). Function 4 is
/// this emulation's one departure from Table 5-28: it reads back the memory
/// address counter, "W.A.+1" after a completed transfer. The manual only
/// documents that readback for the mag tape (5-10.4.10), but X-RAY's shared
/// DMA completion routine `M.IDDR` issues `DIN dev,4  GET W.A.+1` (card 1331)
/// for *any* mass device whose file table asked for it -- and its disc setup
/// path deliberately zeroes the field that asks (cards 999-1000, "SET WA TO
/// ZERO FOR LAST IR"). A disc that read zero here would hand X-RAY a zero to
/// store over its file table's word-address field on every completion, so the
/// period driver outranks the table's silence.
const FN_READ_MEM_ADDR: u8 = 0x4;

/// Half a revolution at 1800 RPM, the documented average access (5-9.1), as
/// a count of clock cycles. The divisor is the rate machine time is being
/// paced at (`Disc74601::set_pacing_hz`), so the platter turns at 1800 RPM of
/// wall clock whatever the cpu is being run at; unpaced it is the machine's own
/// clock, 571,429 / 60 = 9,523.
const HALF_REV_HZ: u64 = 60;
fn avg_access_cycles(pacing_hz: u64) -> u32 {
    (pacing_hz / HALF_REV_HZ) as u32
}

/// 5-9.1's "data transfer rate of 187,000 words per second" -- 100,000 bits a
/// track, 30 revolutions a second. Unpaced that is 3.05 cycles a word; 3 is as
/// close as integer cycles get.
const WORDS_PER_SEC: u64 = 187_000;
fn cycles_per_word(pacing_hz: u64) -> u32 {
    (pacing_hz / WORDS_PER_SEC) as u32
}

/// The unpaced figures, the machine running at its own clock rate.
const AVG_ACCESS_CYCLES: u32 = (CLOCK_HZ / HALF_REV_HZ) as u32;
const CYCLES_PER_WORD: u32 = (CLOCK_HZ / WORDS_PER_SEC) as u32;

/// Status word, Table 5-29. Bit 0 is the most significant bit, as everywhere
/// on this machine; an all-zero word means ready for a new command.
const ST_NOT_READY: u16 = 0x8000; // bit 0: controller or device not ready
/// Bit 1 is not in Table 5-29 but is real: 5-9.5.4/5 say completion sets
/// "status bit-0 and bit-1" false, and X-RAY's `M.DSTAT` probes exactly those
/// two -- `SAM` for the controller, then `SLL 1` / `SAM` for the device
/// (cards 1065-1068).
const ST_BUSY: u16 = 0x4000; // bit 1: device busy
const ST_PROTECTED: u16 = 0x0004; // bit 13: write command to a protected track (5-9.10.2)
/// Bits 14 and 15 are rate error and rate-or-CRC error (5-9.10.1, 5-9.5.4).
/// Neither is ever raised here -- an image in memory cannot lose the race a
/// rate error is, and has no CRC to fail -- but the constants exist so a
/// status decode reads complete.
#[allow(dead_code)]
const ST_RATE_ERR: u16 = 0x0002; // bit 14
#[allow(dead_code)]
const ST_RATE_OR_CRC: u16 = 0x0001; // bit 15

/// One mounted drive.
struct DiscUnit {
    /// The whole platter, big-endian words.
    data: Vec<u8>,
    /// Write-through target, opened read+write; `None` for a unit mounted by
    /// a test. Written a completed operation at a time, so there is nothing
    /// to flush at exit -- dropping the machine closes it on every exit path.
    file: Option<File>,
    /// The WRITE INHIBIT switch (Table 5-30): the highest protected track, or
    /// `None` for OFF. The knob's positions are 0/1/3/7/15/31/63, but nothing
    /// here needs to enforce that.
    write_inhibit: Option<u8>,
    /// Result bits (13-15) of the unit's last completed command. 5-9.9: "the
    /// disc controller will save the results ... of the previous operation
    /// indefinitely", so they hold until the next command clears them.
    result: u16,
}

enum Op {
    Write,
    Read,
    Verify,
}

/// A transfer in flight, between its starting DOT and its completion.
struct Active {
    unit: usize,
    op: Op,
    words: u16,
    /// Cycles until completion.
    remaining: u32,
}

pub struct Disc74601 {
    /// Which interrupt level completions pulse.
    level: u8,
    /// Cleared by `DOT 1,0`, set back by any other disc DOT. 5-9.5.1 says
    /// only that the disconnected controller's "interrupt capability is
    /// inhibited" -- nothing about aborting a transfer -- so an in-flight
    /// operation still lands its data and only the completion pulse is
    /// swallowed. X-RAY disconnects after completion, never during, so this
    /// choice only decides how odd software fails.
    connected: bool,
    units: [Option<DiscUnit>; DISC_UNITS],
    /// The memory address counter, a 15-bit word address (5-9.5.2: ACR bits
    /// 1-15; bit 0, the sign bit, is unused). Advances with the transfer, so
    /// after completion it reads as W.A.+1 -- see `FN_READ_MEM_ADDR`.
    mem_addr: u16,
    /// Track number counter, ACR bits 0-5 of the Set Track & Sector word
    /// (5-9.5.3) -- the *top* six bits, bit 0 being the MSB.
    track: u16,
    /// First sector register, ACR bits 7-15 (bit 6 unused). Nine bits because
    /// the jumperable maximum is 512 sectors a track; the standard sectoring
    /// counts to 128, so the counter's use wraps it there.
    sector: u16,
    active: Option<Active>,
    /// `--fast-io`: transfers complete on the next poll.
    fast_io: bool,
    /// Access time and transfer rate as cycle counts, from whatever rate
    /// machine time is being paced at (`set_pacing_hz`).
    access_cycles: u32,
    word_cycles: u32,
}

impl Disc74601 {
    pub fn new(level: u8) -> Self {
        Disc74601 {
            level,
            connected: true,
            units: [None, None, None, None],
            mem_addr: 0,
            track: 0,
            sector: 0,
            active: None,
            fast_io: false,
            access_cycles: AVG_ACCESS_CYCLES,
            word_cycles: CYCLES_PER_WORD,
        }
    }

    /// Run transfers at host speed instead of disc speed (`--fast-io`).
    pub fn set_fast_io(&mut self) {
        self.fast_io = true;
    }

    /// Turn the platter at 1800 RPM against a machine paced at `pacing_hz`
    /// cycles to the second; see [`crate::bus::Bus::set_device_pacing_hz`].
    /// `--fast-io` still outranks it, in `dot`.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        self.access_cycles = avg_access_cycles(pacing_hz);
        self.word_cycles = cycles_per_word(pacing_hz);
    }

    /// Mount an image file on a unit, Kaypro-floppy style: a file that simply
    /// is not there is a drive that was never installed and stays silent, so
    /// the registry can probe for all four units on every boot without
    /// nagging. A file that exists but is the wrong size prints and leaves
    /// the unit not-ready -- a half-sized platter is not a thing, and
    /// guessing at padding would invent data the file does not hold.
    pub fn load_image(&mut self, unit: usize, path: &Path) -> bool {
        let mut file = match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
            Err(e) => {
                println!("703: disc unit {unit} image '{}': {e}", path.display());
                return false;
            }
        };
        let mut data = Vec::with_capacity(IMAGE_BYTES);
        if let Err(e) = file.read_to_end(&mut data) {
            println!("703: disc unit {unit} image '{}': {e}", path.display());
            return false;
        }
        if data.len() != IMAGE_BYTES {
            println!(
                "703: disc unit {unit} image '{}' is {} bytes, not {IMAGE_BYTES}; not mounted",
                path.display(),
                data.len()
            );
            return false;
        }
        println!("703: mounted disc unit {unit} '{}' ({WORDS_PER_UNIT} words)", path.display());
        self.units[unit] =
            Some(DiscUnit { data, file: Some(file), write_inhibit: None, result: 0 });
        true
    }

    /// A zeroed drive with no backing file, for tests.
    pub fn mount_blank(&mut self, unit: usize) {
        self.units[unit] =
            Some(DiscUnit { data: vec![0; IMAGE_BYTES], file: None, write_inhibit: None, result: 0 });
    }

    /// Set a unit's WRITE INHIBIT switch: `Some(n)` protects tracks 0..=n,
    /// `None` is OFF (Table 5-30).
    pub fn set_write_inhibit(&mut self, unit: usize, highest_track: Option<u8>) {
        if let Some(u) = &mut self.units[unit] {
            u.write_inhibit = highest_track;
        }
    }

    /// The LOAD button on the controller's operating panel (Table 5-30):
    /// "Loads contents of sector 0, track 0 of disc 0 into core memory
    /// starting at location 0." One sector, 47 words, every address fixed --
    /// the hardware bootstrap of 5-9.10.3, and the disc controller's only
    /// control besides the WRITE INHIBIT knobs.
    ///
    /// No interrupt is raised: the button is an operator control pressed with
    /// the processor stopped, and the manual describes a read into core, not
    /// a command completing. (Invented in the details only -- the manual
    /// never says what the sequencer does about interrupts -- but a boot
    /// sector's words 0-3 are the level 0 interrupt block it is busy
    /// overwriting, so a completion pulse here could dispatch through a
    /// half-written vector.)
    ///
    /// Returns false when no disc 0 is mounted, which is a press of the
    /// button with no drive spinning: nothing happens.
    pub fn press_load(&mut self, core: &mut Memory) -> bool {
        let Some(u) = &self.units[0] else { return false };
        core.load_at(0, &u.data[..WORDS_PER_SECTOR * 2]);
        true
    }

    pub fn dot(&mut self, function: u8, val: u16) {
        if function == FN_DISCONNECT {
            self.connected = false; // 5-9.5.1
            return;
        }
        // Any other disc DOT is software talking to the controller again;
        // there is no explicit reconnect function, so this is it.
        self.connected = true;
        match function {
            FN_SET_MEM_ADDR => self.mem_addr = val & 0x7fff, // 5-9.5.2: bits 1-15
            FN_SET_TRACK_SECTOR => {
                // 5-9.5.3: track in bits 0-5, sector in bits 7-15, bit 6
                // unused -- remember bit 0 is the MSB, so the track is the
                // *top* of the word.
                self.track = val >> 10;
                self.sector = val & 0x1ff;
            }
            FN_WRITE | FN_READ | FN_VERIFY => self.start(function, val),
            // 3, 5, 8-15: no such functions; a DOT to nowhere does nothing,
            // matching the bus's absent-device convention.
            _ => {}
        }
    }

    /// 5-9.5.4/5/6: unit number in ACR bits 0-1, word count in bits 2-15,
    /// and the operation starts.
    fn start(&mut self, function: u8, val: u16) {
        // A command on top of a running transfer is a "not ready" the program
        // was told about (5-9.7: a non-zero status word means not ready to
        // receive a new command); what the hardware does with one anyway is
        // undocumented, so it is simply lost here. X-RAY never issues one --
        // M.DSTAT spins until the status word clears.
        if self.active.is_some() {
            return;
        }
        let unit = (val >> 14) as usize;
        let words = val & 0x3fff;
        let Some(u) = &mut self.units[unit] else {
            // A drive that is not installed is not spinning: the command is
            // never accepted and no completion ever comes. Software that
            // checked status first saw bit 0 and knew better.
            return;
        };
        u.result = 0; // a new command supersedes the held result (5-9.9)

        let op = match function {
            FN_WRITE => Op::Write,
            FN_READ => Op::Read,
            _ => Op::Verify,
        };

        // The WRITE INHIBIT check (5-9.10.2): "the protected track will not
        // be written on and an interrupt will be returned immediately", with
        // status bit 13. The whole span is checked, not just the starting
        // track, though only the invented track wrap below can carry a span
        // from an unprotected track into a protected one.
        if matches!(op, Op::Write) {
            if let Some(limit) = u.write_inhibit {
                let start = self.track as usize * SECTORS_PER_TRACK
                    + self.sector as usize % SECTORS_PER_TRACK;
                let sectors = (words as usize).div_ceil(WORDS_PER_SECTOR);
                let protected = (0..sectors.max(1)).any(|k| {
                    let track = (start + k) / SECTORS_PER_TRACK % TRACKS;
                    track <= limit as usize
                });
                if protected {
                    u.result = ST_PROTECTED;
                    // Zero words: the immediate interrupt with nothing moved.
                    self.active = Some(Active { unit, op, words: 0, remaining: 0 });
                    return;
                }
            }
        }

        let remaining = if self.fast_io {
            0
        } else {
            self.access_cycles + words as u32 * self.word_cycles
        };
        self.active = Some(Active { unit, op, words, remaining });
    }

    pub fn din(&mut self, function: u8) -> u16 {
        match function {
            // 5-9.7: functions 0-3 return the status of that unit.
            0..=3 => self.status(function as usize),
            FN_READ_MEM_ADDR => self.mem_addr, // the divergence; see the constant
            _ => 0,
        }
    }

    fn status(&mut self, unit: usize) -> u16 {
        if self.units[unit].is_none() {
            // A drive that is not there is a drive that is not ready. This is
            // the one observable the controller card's presence changes: an
            // absent *device* reads 0 on this bus, which as a status word
            // would mean "ready".
            return ST_NOT_READY;
        }
        match &self.active {
            // 5-9.5.5 has completion clearing bits 0 and 1, so during the
            // transfer the running unit shows both, and every other unit
            // shows bit 0 alone -- the controller is busy but their spindles
            // are fine. X-RAY's M.DSTAT distinguishes exactly these two.
            Some(a) if a.unit == unit => ST_NOT_READY | ST_BUSY,
            Some(_) => ST_NOT_READY,
            None => self.units[unit].as_ref().unwrap().result,
        }
    }

    /// Advance the transfer in flight by `elapsed` cycles; when its time
    /// expires, do the whole DMA copy against `core` and pulse the level.
    ///
    /// The `&mut Memory` is the DMA channel: the machine's
    /// `poll_interrupt_lines` is the one place its core and its devices meet,
    /// so that is where a device that addresses memory has to run.
    pub fn poll(&mut self, elapsed: u32, core: &mut Memory) -> u16 {
        let Some(a) = &mut self.active else { return 0 };
        a.remaining = a.remaining.saturating_sub(elapsed);
        if a.remaining > 0 {
            return 0;
        }
        let a = self.active.take().unwrap();
        self.complete(&a, core);
        // A disconnected controller still finished the work; only its voice
        // is inhibited (5-9.5.1).
        if self.connected {
            1 << self.level
        } else {
            0
        }
    }

    fn complete(&mut self, a: &Active, core: &mut Memory) {
        let unit = self.units[a.unit].as_mut().unwrap();
        // The platter is one linear run of words, track-major: track t,
        // sector s, word w is word (t*128 + s)*47 + w. That linearity *is*
        // 5-9.4's continuation rule -- a transfer running off the last sector
        // of a track "resumes at the first sector of the next track" because
        // the next word of the array is exactly that. Wrapping modulo the
        // unit at the end of track 63 is invented (the manual stops at the
        // increment), on the grounds that a six-bit track counter has nowhere
        // else to go.
        let start_disc = (self.track as usize * SECTORS_PER_TRACK
            + self.sector as usize % SECTORS_PER_TRACK)
            * WORDS_PER_SECTOR;
        let words = a.words as usize;
        for i in 0..words {
            let d = (start_disc + i) % WORDS_PER_UNIT * 2;
            // The memory address counter is 15 bits of word address, so it
            // wraps at word 0x8000 -- which is byte 0x10000, the same wrap
            // `Memory` itself applies. Byte-at-a-time through the device
            // interface keeps that true; `load_at` would clip instead.
            let c = (self.mem_addr.wrapping_add(i as u16) & 0x7fff) as u32 * 2;
            match a.op {
                Op::Write => {
                    unit.data[d] = core.read_byte(c);
                    unit.data[d + 1] = core.read_byte(c + 1);
                }
                Op::Read => {
                    core.write_byte(c, unit.data[d]);
                    core.write_byte(c + 1, unit.data[d + 1]);
                }
                // 5-9.5.6: "the same operation ... except that no data is
                // transferred into the core memory". The read and its CRC
                // check happen on the disc side, and an image has no CRC to
                // fail, so a verify is timing and register motion only.
                Op::Verify => {}
            }
        }

        if matches!(a.op, Op::Write) {
            Self::write_through(unit, start_disc, words);
        }

        // The counters end where the transfer left them. The address counter
        // advances per word strobed over the DMA channel -- which is why it
        // reads W.A.+1 afterwards, and why a verify, which strobes nothing,
        // leaves it alone. The track and sector registers follow the sectors
        // that passed under the heads, a whole sector per CRC check even when
        // the count stops mid-sector.
        if !matches!(a.op, Op::Verify) {
            self.mem_addr = self.mem_addr.wrapping_add(a.words) & 0x7fff;
        }
        let sectors = words.div_ceil(WORDS_PER_SECTOR);
        let end = (self.track as usize * SECTORS_PER_TRACK
            + self.sector as usize % SECTORS_PER_TRACK
            + sectors)
            % (TRACKS * SECTORS_PER_TRACK);
        self.track = (end / SECTORS_PER_TRACK) as u16;
        self.sector = (end % SECTORS_PER_TRACK) as u16;
    }

    /// Write a completed write's span back to the image file: the touched
    /// bytes only, in one range or two when the end-of-unit wrap splits it.
    fn write_through(unit: &mut DiscUnit, start_word: usize, words: usize) {
        let Some(file) = &mut unit.file else { return };
        let mut spans = Vec::new();
        let start = start_word % WORDS_PER_UNIT;
        let first = words.min(WORDS_PER_UNIT - start);
        spans.push((start * 2, first * 2));
        if first < words {
            spans.push((0, (words - first) * 2));
        }
        for (off, len) in spans {
            let r = file
                .seek(SeekFrom::Start(off as u64))
                .and_then(|_| file.write_all(&unit.data[off..off + len]));
            if let Err(e) = r {
                println!("703: disc write-through failed: {e}");
                return;
            }
        }
    }

    /// A word of the platter, for tests that pin the data layout.
    #[cfg(test)]
    fn disc_word(&self, unit: usize, word: usize) -> u16 {
        let d = &self.units[unit].as_ref().unwrap().data;
        u16::from_be_bytes([d[word * 2], d[word * 2 + 1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL: u8 = 1;

    fn disc() -> Disc74601 {
        let mut d = Disc74601::new(LEVEL);
        d.mount_blank(0);
        d
    }

    fn core() -> Memory {
        Memory::new(64 * 1024)
    }

    /// Fill `n` core words from `word_addr` with a recognizable pattern.
    fn fill(core: &mut Memory, word_addr: u16, n: u16) {
        for i in 0..n {
            let w = (i ^ 0xa5c3).to_be_bytes();
            core.write_byte((word_addr + i) as u32 * 2, w[0]);
            core.write_byte((word_addr + i) as u32 * 2 + 1, w[1]);
        }
    }

    fn word_at(core: &mut Memory, word_addr: u16) -> u16 {
        u16::from_be_bytes([
            core.read_byte(word_addr as u32 * 2),
            core.read_byte(word_addr as u32 * 2 + 1),
        ])
    }

    /// Issue the full 5-9.5 sequence: memory address, track/sector, go.
    fn command(d: &mut Disc74601, op: u8, addr: u16, track: u16, sector: u16, unit: u16, words: u16) {
        d.dot(FN_SET_MEM_ADDR, addr);
        d.dot(FN_SET_TRACK_SECTOR, (track << 10) | sector);
        d.dot(op, (unit << 14) | words);
    }

    /// Poll with a whole transfer's worth of cycles at once.
    fn finish(d: &mut Disc74601, core: &mut Memory) -> u16 {
        d.poll(u32::MAX, core)
    }

    #[test]
    fn an_unmounted_unit_reports_not_ready() {
        let mut d = Disc74601::new(LEVEL);
        assert_eq!(d.din(0), ST_NOT_READY, "nothing mounted");
        d.mount_blank(0);
        assert_eq!(d.din(0), 0, "all-zero status is ready (Table 5-29)");
        assert_eq!(d.din(1), ST_NOT_READY, "the other spindles are still absent");
    }

    #[test]
    fn a_write_moves_core_to_disc_and_interrupts_once() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 10);
        command(&mut d, FN_WRITE, 0x100, 0, 0, 0, 10);
        assert_eq!(finish(&mut d, &mut m), 1 << LEVEL, "one completion pulse");
        assert_eq!(finish(&mut d, &mut m), 0, "and only one");
        for i in 0..10 {
            assert_eq!(d.disc_word(0, i), (i as u16) ^ 0xa5c3, "disc word {i}");
        }
    }

    #[test]
    fn a_read_moves_disc_to_core() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 10);
        command(&mut d, FN_WRITE, 0x100, 5, 9, 0, 10);
        finish(&mut d, &mut m);
        command(&mut d, FN_READ, 0x300, 5, 9, 0, 10);
        assert_eq!(finish(&mut d, &mut m), 1 << LEVEL);
        for i in 0..10u16 {
            assert_eq!(word_at(&mut m, 0x300 + i), i ^ 0xa5c3, "core word {i}");
        }
    }

    #[test]
    fn verify_touches_no_core() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x200, 47);
        // the disc under those sectors is zero, and verify must not care
        command(&mut d, FN_VERIFY, 0x200, 3, 0, 0, 47);
        assert_eq!(finish(&mut d, &mut m), 1 << LEVEL);
        assert_eq!(d.din(0), 0, "clean status");
        for i in 0..47u16 {
            assert_eq!(word_at(&mut m, 0x200 + i), i ^ 0xa5c3, "core word {i} untouched");
        }
    }

    #[test]
    fn busy_bits_are_set_during_a_transfer_and_false_after() {
        let (mut d, mut m) = (disc(), core());
        d.mount_blank(1);
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 5);
        assert_eq!(d.din(0), ST_NOT_READY | ST_BUSY, "the running unit shows bits 0 and 1");
        assert_eq!(d.din(1), ST_NOT_READY, "an idle unit shows the busy controller only");
        finish(&mut d, &mut m);
        assert_eq!(d.din(0), 0, "5-9.5.5: completion sets bits 0 and 1 false");
        assert_eq!(d.din(1), 0);
    }

    #[test]
    fn completion_takes_average_access_plus_word_time() {
        let (mut d, mut m) = (disc(), core());
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 10);
        let total = AVG_ACCESS_CYCLES + 10 * CYCLES_PER_WORD;
        assert_eq!(d.poll(total - 1, &mut m), 0, "one cycle early is not done");
        assert_eq!(d.poll(1, &mut m), 1 << LEVEL, "the exact cycle is");
    }

    /// 5-9.4: a transfer running off the end of a track continues at sector 0
    /// of the next. Written as one 94-word span, read back as the two sectors
    /// the manual says it landed on.
    #[test]
    fn a_transfer_spans_the_end_of_a_track() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 94);
        command(&mut d, FN_WRITE, 0x100, 2, 127, 0, 94);
        finish(&mut d, &mut m);
        command(&mut d, FN_READ, 0x800, 3, 0, 0, 47);
        finish(&mut d, &mut m);
        for i in 0..47u16 {
            assert_eq!(word_at(&mut m, 0x800 + i), (47 + i) ^ 0xa5c3, "track 3 sector 0 word {i}");
        }
    }

    /// Invented behaviour, pinned: past track 63 the transfer wraps to track
    /// 0, because a six-bit track counter has nowhere else to go.
    #[test]
    fn the_track_register_wraps_past_63() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 94);
        command(&mut d, FN_WRITE, 0x100, 63, 127, 0, 94);
        finish(&mut d, &mut m);
        command(&mut d, FN_READ, 0x800, 0, 0, 0, 47);
        finish(&mut d, &mut m);
        for i in 0..47u16 {
            assert_eq!(word_at(&mut m, 0x800 + i), (47 + i) ^ 0xa5c3, "track 0 sector 0 word {i}");
        }
    }

    #[test]
    fn a_write_to_a_protected_track_sets_bit_13_and_writes_nothing() {
        let (mut d, mut m) = (disc(), core());
        d.set_write_inhibit(0, Some(3));
        fill(&mut m, 0x100, 10);
        command(&mut d, FN_WRITE, 0x100, 2, 0, 0, 10);
        // 5-9.10.2: "an interrupt will be returned immediately"
        assert_eq!(d.poll(0, &mut m), 1 << LEVEL);
        assert_eq!(d.din(0), ST_PROTECTED);
        command(&mut d, FN_READ, 0x800, 2, 0, 0, 10);
        finish(&mut d, &mut m);
        for i in 0..10u16 {
            assert_eq!(word_at(&mut m, 0x800 + i), 0, "the platter stayed blank");
        }
    }

    #[test]
    fn error_status_persists_until_the_next_command() {
        let (mut d, mut m) = (disc(), core());
        d.set_write_inhibit(0, Some(0));
        command(&mut d, FN_WRITE, 0x100, 0, 0, 0, 1);
        finish(&mut d, &mut m);
        // 5-9.9: the controller saves the result indefinitely
        assert_eq!(d.din(0), ST_PROTECTED);
        d.poll(1_000_000, &mut m);
        assert_eq!(d.din(0), ST_PROTECTED, "time does not clear it");
        command(&mut d, FN_READ, 0x100, 5, 0, 0, 1);
        finish(&mut d, &mut m);
        assert_eq!(d.din(0), 0, "the next command does");
    }

    #[test]
    fn din_function_4_reads_back_the_word_address_plus_one() {
        let (mut d, mut m) = (disc(), core());
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 10);
        finish(&mut d, &mut m);
        // X-RAY card 1331: "DIN dev,4  GET W.A.+1"
        assert_eq!(d.din(FN_READ_MEM_ADDR), 0x10a);
    }

    #[test]
    fn undefined_functions_read_zero_and_write_nothing() {
        let (mut d, mut m) = (disc(), core());
        assert_eq!(d.din(5), 0);
        assert_eq!(d.din(0xf), 0);
        d.dot(3, 0xffff);
        d.dot(5, 0xffff);
        d.dot(0xe, 0xffff);
        assert_eq!(d.din(0), 0, "still ready; nothing started");
        assert_eq!(finish(&mut d, &mut m), 0, "and nothing completes");
    }

    #[test]
    fn the_memory_address_ignores_bit_0() {
        let mut d = disc();
        // 5-9.5.2: bits 1-15 of the accumulator; bit 0 -- the sign -- is not
        // part of the address. X-RAY's driver relies on this: its DOT dev,1
        // word still carries the sign bit that marked the operation a read.
        d.dot(FN_SET_MEM_ADDR, 0x8000 | 0x0123);
        assert_eq!(d.din(FN_READ_MEM_ADDR), 0x0123);
    }

    #[test]
    fn track_and_sector_use_the_manuals_bit_numbering() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 1);
        // Track 2, sector 3: track lives in bits 0-5 -- the TOP of the word,
        // bit 0 being the MSB -- and sector in bits 7-15 with bit 6 unused.
        command(&mut d, FN_WRITE, 0x100, 2, 3, 0, 1);
        finish(&mut d, &mut m);
        let word = (2 * SECTORS_PER_TRACK + 3) * WORDS_PER_SECTOR;
        assert_eq!(d.disc_word(0, word), 0xa5c3, "landed at the linear sector");
    }

    #[test]
    fn disconnect_inhibits_the_completion_interrupt() {
        let (mut d, mut m) = (disc(), core());
        fill(&mut m, 0x100, 5);
        command(&mut d, FN_WRITE, 0x100, 1, 0, 0, 5);
        d.dot(FN_DISCONNECT, 0);
        // 5-9.5.1 inhibits the interrupt, not the work
        assert_eq!(finish(&mut d, &mut m), 0, "no pulse from a disconnected controller");
        // any disc DOT reconnects, and the data did land
        command(&mut d, FN_READ, 0x800, 1, 0, 0, 5);
        assert_eq!(finish(&mut d, &mut m), 1 << LEVEL);
        for i in 0..5u16 {
            assert_eq!(word_at(&mut m, 0x800 + i), i ^ 0xa5c3);
        }
    }

    #[test]
    fn a_command_to_an_unmounted_unit_never_completes() {
        let (mut d, mut m) = (disc(), core());
        command(&mut d, FN_READ, 0x100, 0, 0, 2, 10);
        assert_eq!(finish(&mut d, &mut m), 0);
        assert_eq!(finish(&mut d, &mut m), 0, "a drive that is not spinning stays silent");
    }

    /// The platter turns at 1800 RPM of wall clock however slowly the cpu is
    /// being run, so a transfer under a slow-motion throttle takes the sixtieth
    /// of a second it takes on the real machine instead of a sixtieth of the
    /// machine's cycles at the operator's leisure.
    #[test]
    fn the_transfer_time_follows_the_pacing_rate() {
        for hz in [CLOCK_HZ, CLOCK_HZ / 100, 10_000] {
            let (mut d, mut m) = (disc(), core());
            d.set_pacing_hz(hz);
            let total = (hz / 60) as u32 + 10 * (hz / 187_000) as u32;
            command(&mut d, FN_READ, 0x100, 0, 0, 0, 10);
            assert_eq!(d.poll(total - 1, &mut m), 0, "hz={hz}");
            assert_eq!(d.poll(1, &mut m), 1 << LEVEL, "hz={hz}");
        }
    }

    /// `--fast-io` outranks a pacing rate, whichever order the two arrive in:
    /// the factory sets fast-io and `main` the rate, so a rate is not a way to
    /// put a disc asked for host speed back on a spinning platter.
    #[test]
    fn fast_io_outranks_a_pacing_rate() {
        let (mut d, mut m) = (disc(), core());
        d.set_fast_io();
        d.set_pacing_hz(CLOCK_HZ);
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 1000);
        assert_eq!(d.poll(0, &mut m), 1 << LEVEL, "still instant");
    }

    #[test]
    fn fast_io_completes_on_the_next_poll() {
        let (mut d, mut m) = (disc(), core());
        d.set_fast_io();
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 1000);
        assert_eq!(d.poll(0, &mut m), 1 << LEVEL, "no cycles need pass");
    }

    /// A path that exists and holds `bytes` (the system tests' pattern).
    fn scratch_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("emu-disc74601-{pid}-{name}"));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn write_through_lands_in_the_image_file() {
        // a blank image the way the e2e script makes one: a sparse file of
        // exactly the platter size
        let path = scratch_file("image", &[]);
        File::create(&path).unwrap().set_len(IMAGE_BYTES as u64).unwrap();
        let (mut d, mut m) = (Disc74601::new(LEVEL), core());
        assert!(d.load_image(0, &path));
        fill(&mut m, 0x100, 2);
        command(&mut d, FN_WRITE, 0x100, 1, 1, 0, 2);
        finish(&mut d, &mut m);
        let bytes = std::fs::read(&path).unwrap();
        let off = (SECTORS_PER_TRACK + 1) * WORDS_PER_SECTOR * 2;
        assert_eq!(&bytes[off..off + 4], &[0xa5, 0xc3, 0xa5, 0xc2], "the file holds the words");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_wrong_size_image_is_rejected() {
        let path = scratch_file("short", &[0u8; 100]);
        let mut d = Disc74601::new(LEVEL);
        assert!(!d.load_image(0, &path));
        assert_eq!(d.din(0), ST_NOT_READY, "the unit stayed unmounted");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_load_button_reads_the_boot_sector_to_word_0() {
        let (mut d, mut m) = (disc(), core());
        // put a recognizable sector 0 on the platter the honest way, through
        // a write command from high core
        fill(&mut m, 0x100, 47);
        command(&mut d, FN_WRITE, 0x100, 0, 0, 0, 47);
        finish(&mut d, &mut m);
        assert!(d.press_load(&mut m));
        for i in 0..47u16 {
            assert_eq!(word_at(&mut m, i), i ^ 0xa5c3, "boot word {i}");
        }
        assert_eq!(word_at(&mut m, 47), 0, "exactly one sector, nothing past it");
        // and the press raised no interrupt line on the next poll
        assert_eq!(d.poll(u32::MAX, &mut m), 0);
    }

    #[test]
    fn the_load_button_does_nothing_with_no_disc_0() {
        let mut d = Disc74601::new(LEVEL);
        let mut m = core();
        m.write_byte(0, 0x42);
        assert!(!d.press_load(&mut m));
        assert_eq!(m.read_byte(0), 0x42, "core untouched");
        d.mount_blank(1);
        assert!(!d.press_load(&mut m), "the button is wired to disc 0 alone");
    }

    #[test]
    fn the_interrupt_level_is_configurable() {
        let mut d = Disc74601::new(3);
        d.mount_blank(0);
        let mut m = core();
        command(&mut d, FN_READ, 0x100, 0, 0, 0, 1);
        assert_eq!(finish(&mut d, &mut m), 1 << 3);
    }
}
