// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The MITS 88-DCDD floppy disk controller and its eight-inch drives.
//!
//! Written from MITS's *Altair 8800 Floppy Disk System* manual, its "Altair
//! Disk Controller I/O Information" (printed pages 133-136) and timing
//! diagram; page numbers below are the manual's printed ones. Three ports,
//! octal 010-012 there:
//!
//! - `OUT 08h` selects drive D0-D3 and enables disk control; D7 clears it,
//!   deselecting every drive and disabling the interrupt. A drive with no
//!   disk cannot be enabled, as one with its door open could not (133).
//! - `IN 08h` is the status, every bit true when 0 (134): D0 ENWD, the
//!   controller ready for a write byte; D1 MH, the head may move; D2 HS,
//!   the head loaded and settled; D3 and D4 always 0, which is how software
//!   sees a drive enabled; D5 the bus's INTE, not modelled -- the bus has
//!   no view of the cpu's interrupt enable, so it reads false; D6 track 0;
//!   D7 NRDA, a byte read off the disk and not yet taken. With no drive
//!   enabled every bit reads false.
//! - `OUT 09h` commands, a 1 acting (135): D0 step in, D1 step out, D2 head
//!   load, D3 head unload, D4 and D5 interrupt enable and disable, D6 the
//!   head current switch (which a write on the inner tracks wants, and which
//!   changes nothing here), D7 write enable.
//! - `IN 09h` is the sector position, valid only with the head loaded and
//!   settled (136): D0 sector true, low for the 30 us at the start of each
//!   sector, D1-D5 the sector number, D6 and D7 high. Otherwise -- which
//!   the manual leaves unsaid -- it reads all high, never sector true.
//! - `IN 0Ah` reads the byte under the head and clears NRDA; `OUT 0Ah`
//!   gives the controller the next byte to write and clears ENWD. What a
//!   read returns with no new byte the manual does not say: here, the last
//!   byte it took.
//!
//! **The timing is the disk's, in machine cycles.** The FD-400 turns once
//! in 166.7 ms -- 360 rpm, 32 hard sectors of the timing diagram's 5.2 ms;
//! the manual never states the speed -- and the
//! controller moves a byte every 32 us (134) -- 64 cycles of the 8800's
//! 2 MHz, which is what Altair software is written against: its read loops
//! take two bytes a pass, the second read without asking NRDA, in the
//! window the second byte is there (Mike Douglas, *Read Timing for the
//! Altair 8 inch Floppy*). So nothing here answers a request: where the
//! disk is under the head is the clock's to say, and a port read asks the
//! clock. A sector's first byte is under the head 140 us after sector true
//! (136); a write enabled at sector true asks for its first byte 280 us in
//! (135), takes at most 137 (the first with its sync bit set, as software
//! makes it), and writes the last byte it was given over every slot it
//! was given none, to the end of the sector (136), when the sector reaches
//! the disk. The head is settled 40 ms after a load, and after a step with
//! it loaded (134); after a step the head may step again 10 ms on, for a
//! 1 ms window, or after 31 ms (134), and not for 40 ms after a load or
//! during a write; a step asked for while MH says not to is taken anyway,
//! which the manual leaves open. A drive deselected, or another selected,
//! has its head lifted: clearing disk control drops the head load line,
//! which the manual implies rather than says. A write in flight reaches
//! the disk at its sector's end, or as far as it got when its drive is
//! deselected or its disk taken out; a process ending inside that last
//! millisecond loses it, as a machine switched off would.
//!
//! The times are the machine's at `set_pacing_hz`, so the software has to
//! run at the clock it was written for: an 8800 throttled away from 2 MHz
//! either way cannot read its disk -- slower misses bytes, faster takes
//! the unchecked second byte before it has come, and the boot loader
//! prints its checksum error, `C`, for ever -- as an 8080 at another clock
//! could not. `--no-throttle` is fine, the disk then counting the same
//! machine cycles the cpu does; `--fast-io` leaves the disk alone, as it
//! leaves the Kaypro's floppy: the bytes' time is what the software is
//! built around.
//!
//! The image is the sectors in order, 137 bytes each, 32 to a track and 77
//! tracks -- the raw bytes as they pass under the head, the format of the
//! Altair disk images in circulation (some are padded to a 128-byte
//! multiple, which the drive ignores). What the bytes mean -- the sync and
//! track byte, the payload, the stop byte and checksum -- is the software's,
//! and nothing here reads them. The sector-true interrupt (135) is
//! modelled; which interrupt level it reaches is the machine's.

use crate::console::DiskImage;

/// Drives on the controller. It addresses sixteen (133); Altair CP/M and
/// Disk BASIC use four.
pub const DRIVES: usize = 4;
pub const TRACKS: usize = 77;
pub const SECTORS: usize = 32;
/// A sector as it passes under the head: sync byte to stop byte.
pub const SECTOR_BYTES: usize = 137;
/// A disk's image, its tracks one after another.
pub const IMAGE_BYTES: u64 = (TRACKS * SECTORS * SECTOR_BYTES) as u64;

/// One revolution, 360 rpm, in nanoseconds; a sector is a 32nd of it.
const REVOLUTION_NANOS: u64 = 166_666_667;
/// A byte's time under the head (134).
const BYTE_NANOS: u64 = 32_000;
/// Sector true, at the start of each sector (136).
const SECTOR_TRUE_NANOS: u64 = 30_000;
/// The first byte of a sector is read this long after sector true (136).
const READ_START_NANOS: u64 = 140_000;
/// A write's first byte is asked for this long after sector true (135).
const WRITE_START_NANOS: u64 = 280_000;
/// The head loaded, or stepped while loaded, is settled after this (134).
const HEAD_SETTLE_NANOS: u64 = 40_000_000;
/// MH after a step: false for 10 ms, true for 1 ms, false to 31 ms (134).
const STEP_REPEAT_NANOS: u64 = 10_000_000;
const STEP_REPEAT_WINDOW_NANOS: u64 = 1_000_000;
const STEP_SETTLE_NANOS: u64 = 31_000_000;
/// MH stays false this long after a write ends, while trim erase is on (135).
const TRIM_ERASE_NANOS: u64 = 475_000;

/// Write slots in a sector: the 137 bytes and the 000 after them (135).
const WRITE_SLOTS: u64 = SECTOR_BYTES as u64 + 1;

const ST_ENWD: u8 = 1 << 0;
const ST_MH: u8 = 1 << 1;
const ST_HS: u8 = 1 << 2;
const ST_TRACK0: u8 = 1 << 6;
const ST_NRDA: u8 = 1 << 7;
/// D3 and D4, "not used, = 0" with a drive enabled. D5, the bus's INTE,
/// this side cannot see, and it reads false (1) with the rest.
const ST_UNUSED: u8 = 0b0001_1000;

const CMD_STEP_IN: u8 = 1 << 0;
const CMD_STEP_OUT: u8 = 1 << 1;
const CMD_HEAD_LOAD: u8 = 1 << 2;
const CMD_HEAD_UNLOAD: u8 = 1 << 3;
const CMD_INT_ENABLE: u8 = 1 << 4;
const CMD_INT_DISABLE: u8 = 1 << 5;
const CMD_WRITE: u8 = 1 << 7;

const SELECT_CLEAR: u8 = 1 << 7;

/// One drive: its disk, where its head is, and when it last moved.
#[derive(Default)]
struct Drive {
    image: Option<DiskImage>,
    track: u8,
    loaded: bool,
    /// When the head is settled, loaded or stepped while loaded.
    settled_at: u64,
    /// When the head was last loaded, for MH's 40 ms.
    loaded_at: Option<u64>,
    /// When the head last stepped.
    stepped_at: Option<u64>,
    /// The terminal has been told this disk is read-only.
    told_read_only: bool,
}

/// A write in flight: the sector it is writing, and the byte each slot got.
struct Write {
    drive: usize,
    track: u8,
    /// The sector's number since the clock started: its start and end.
    serial: u64,
    slots: [Option<u8>; SECTOR_BYTES],
}

pub struct Dcdd {
    drives: [Drive; DRIVES],
    /// The drive disk control is enabled for.
    selected: Option<usize>,
    /// Machine cycles since power-on: where every disk is under its head.
    now: u64,
    pacing_hz: u64,
    interrupts: bool,
    /// The last byte read: the sector's serial number and its slot.
    taken: Option<(u64, u64)>,
    /// The data register: the last byte under the head when it was read.
    data: u8,
    write: Option<Write>,
    /// When the last write ended, for MH's trim erase.
    write_ended: Option<u64>,
    /// The sector last read off a disk, so a sector's 137 reads are one.
    cache: Option<(usize, u8, u8, [u8; SECTOR_BYTES])>,
    /// The disks a snapshot load read, until `commit_loaded` puts them in.
    loaded: Option<Vec<Option<DiskImage>>>,
}

impl Dcdd {
    /// The controller with empty drives, at the 8800's 2 MHz.
    pub fn new() -> Self {
        Dcdd {
            drives: Default::default(),
            selected: None,
            now: 0,
            pacing_hz: crate::system::altair8800::CLOCK_HZ,
            interrupts: false,
            taken: None,
            data: 0,
            write: None,
            write_ended: None,
            cache: None,
            loaded: None,
        }
    }

    /// The disk's times against `pacing_hz` cycles to the second.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        self.pacing_hz = pacing_hz.max(1);
    }

    fn cycles(&self, nanos: u64) -> u64 {
        (self.pacing_hz as u128 * nanos as u128 / 1_000_000_000) as u64
    }

    fn sector_cycles(&self) -> u64 {
        self.cycles(REVOLUTION_NANOS / SECTORS as u64).max(1)
    }

    /// Advance the disks by `elapsed` machine cycles. A write reaches the
    /// disk as its sector ends.
    pub fn tick(&mut self, elapsed: u32) {
        self.now += elapsed as u64;
        if let Some(w) = &self.write {
            if self.now >= (w.serial + 1) * self.sector_cycles() {
                self.finish_write();
            }
        }
    }

    /// The sector under the heads: its serial number since power-on and
    /// how far into it the disk has turned, in cycles.
    fn position(&self) -> (u64, u64) {
        let s = self.sector_cycles();
        (self.now / s, self.now % s)
    }

    /// Put `image` in `drive`, or empty it. Opening the door clears disk
    /// control if the drive was enabled (133); a write in flight reaches
    /// the disk first, as far as it got.
    pub fn mount(&mut self, drive: usize, image: Option<DiskImage>) {
        if self.write.as_ref().is_some_and(|w| w.drive == drive) {
            self.finish_write();
        }
        if self.selected == Some(drive) {
            self.clear();
        }
        self.cache = None;
        self.drives[drive].image = image;
        self.drives[drive].told_read_only = false;
    }

    pub fn has_image(&self, drive: usize) -> bool {
        self.drives.get(drive).is_some_and(|d| d.image.is_some())
    }

    /// The disk in `drive`, whole.
    pub fn disk_contents(&self, drive: usize) -> Result<Vec<u8>, String> {
        let image = self.drives.get(drive).and_then(|d| d.image.as_ref()).ok_or("the drive is empty")?;
        image.contents().map_err(|e| format!("{}: {e}", image.path.display()))
    }

    /// Whether the sector-true interrupt is asserted: enabled, and a drive
    /// enabled with its head settled, at sector true (135).
    pub fn irq(&self) -> bool {
        self.interrupts && self.head_ready() && self.position().1 < self.cycles(SECTOR_TRUE_NANOS)
    }

    /// Clear disk control: no drive enabled, its head unloaded, the
    /// interrupt disabled (133, 135).
    fn clear(&mut self) {
        if let Some(d) = self.selected.take() {
            self.drives[d].loaded = false;
        }
        self.interrupts = false;
    }

    fn enabled(&self) -> Option<&Drive> {
        self.selected.map(|d| &self.drives[d])
    }

    fn head_ready(&self) -> bool {
        self.enabled().is_some_and(|d| d.loaded && self.now >= d.settled_at)
    }

    fn move_ok(&self, d: &Drive) -> bool {
        let after = |at: Option<u64>, nanos: u64| at.is_none_or(|t| self.now >= t + self.cycles(nanos));
        let stepped = match d.stepped_at {
            None => true,
            Some(t) => {
                let since = self.now - t;
                since >= self.cycles(STEP_SETTLE_NANOS)
                    || (self.cycles(STEP_REPEAT_NANOS)..self.cycles(STEP_REPEAT_NANOS + STEP_REPEAT_WINDOW_NANOS))
                        .contains(&since)
            }
        };
        stepped
            && after(d.loaded_at, HEAD_SETTLE_NANOS)
            && self.write.is_none()
            && after(self.write_ended, TRIM_ERASE_NANOS)
    }

    /// The read slot under the head: which of the sector's bytes has most
    /// lately come past, if any has.
    fn read_slot(&self) -> Option<(u64, u64)> {
        let (serial, into) = self.position();
        let start = self.cycles(READ_START_NANOS);
        if into < start {
            return None;
        }
        let slot = ((into - start) / self.cycles(BYTE_NANOS).max(1)).min(SECTOR_BYTES as u64 - 1);
        Some((serial, slot))
    }

    /// The write slot the clock is in, if the write has asked for a byte.
    fn write_slot(&self) -> Option<u64> {
        let w = self.write.as_ref()?;
        let s = self.sector_cycles();
        let start = w.serial * s + self.cycles(WRITE_START_NANOS);
        if self.now < start {
            return None;
        }
        Some((self.now - start) / self.cycles(BYTE_NANOS).max(1))
    }

    /// ENWD: the write's slot under the head has no byte yet. The slot
    /// after the 137th takes the 000 that ends a sector (135), which goes
    /// nowhere here.
    fn wants_byte(&self) -> bool {
        match (self.write_slot(), &self.write) {
            (Some(j), Some(w)) if j < WRITE_SLOTS => w.slots.get(j as usize).is_none_or(|b| b.is_none()),
            _ => false,
        }
    }

    fn status(&self) -> u8 {
        let Some(d) = self.enabled() else {
            return 0xff;
        };
        let mut s = !ST_UNUSED;
        if self.wants_byte() {
            s &= !ST_ENWD;
        }
        if self.move_ok(d) {
            s &= !ST_MH;
        }
        if self.head_ready() {
            s &= !ST_HS;
            if self.write.is_none() && self.read_slot().is_some_and(|at| self.taken != Some(at)) {
                s &= !ST_NRDA;
            }
        }
        if d.track == 0 {
            s &= !ST_TRACK0;
        }
        s
    }

    fn sector_register(&self) -> u8 {
        if !self.head_ready() {
            return 0xff;
        }
        let (serial, into) = self.position();
        let truth = if into < self.cycles(SECTOR_TRUE_NANOS) { 0 } else { 1 };
        0b1100_0000 | ((serial % SECTORS as u64) as u8) << 1 | truth
    }

    /// The byte of the sector under the head, read off the disk.
    fn byte_under_head(&mut self, serial: u64, slot: u64) -> u8 {
        let Some(drive) = self.selected else {
            return self.data;
        };
        let track = self.drives[drive].track;
        let sector = (serial % SECTORS as u64) as u8;
        if !matches!(self.cache, Some((d, t, s, _)) if (d, t, s) == (drive, track, sector)) {
            let mut bytes = [0u8; SECTOR_BYTES];
            if let Some(image) = &mut self.drives[drive].image {
                // a short image reads as unformatted: zeros
                let _ = image.read_at(offset(track, sector), &mut bytes);
            }
            self.cache = Some((drive, track, sector, bytes));
        }
        self.cache.as_ref().map_or(0, |c| c.3[slot as usize])
    }

    /// A write's sector onto its disk: the bytes each slot got, and the last
    /// one given over every slot that got none (136).
    fn finish_write(&mut self) {
        let Some(w) = self.write.take() else {
            return;
        };
        self.write_ended = Some(self.now);
        let mut bytes = [0u8; SECTOR_BYTES];
        let mut last = 0u8;
        for (b, slot) in bytes.iter_mut().zip(w.slots) {
            last = slot.unwrap_or(last);
            *b = last;
        }
        let sector = (w.serial % SECTORS as u64) as u8;
        self.cache = None;
        if let Some(image) = &mut self.drives[w.drive].image {
            if image.write_protected {
                return;
            }
            if let Err(e) = image.write_at(offset(w.track, sector), &bytes) {
                eprintln!("88-DCDD: drive {}: write of track {} sector {sector}: {e}", w.drive, w.track);
            }
        }
    }

    pub fn read(&mut self, port: u8) -> u8 {
        match port {
            0 => self.status(),
            1 => self.sector_register(),
            _ => {
                if self.head_ready() && self.write.is_none() {
                    if let Some((serial, slot)) = self.read_slot() {
                        self.data = self.byte_under_head(serial, slot);
                        self.taken = Some((serial, slot));
                    }
                }
                self.data
            }
        }
    }

    pub fn write(&mut self, port: u8, val: u8) {
        match port {
            0 => {
                if self.write.is_some() {
                    self.finish_write();
                }
                // selecting clears disk control first: the drive enabled
                // before has its head lifted, and the new one's comes down
                // only on a load command
                self.clear();
                if val & SELECT_CLEAR == 0 {
                    let d = (val & 0x0f) as usize;
                    if self.has_image(d) {
                        self.selected = Some(d);
                    }
                }
            }
            1 => self.command(val),
            _ => {
                if let Some(j) = self.write_slot() {
                    if let Some(w) = &mut self.write {
                        if let Some(slot) = w.slots.get_mut(j as usize) {
                            *slot = Some(val);
                        }
                    }
                }
            }
        }
    }

    fn command(&mut self, val: u8) {
        if val & CMD_INT_ENABLE != 0 {
            self.interrupts = true;
        }
        if val & CMD_INT_DISABLE != 0 {
            self.interrupts = false;
        }
        let Some(drive) = self.selected else {
            return;
        };
        let now = self.now;
        let settle = self.cycles(HEAD_SETTLE_NANOS);
        let d = &mut self.drives[drive];
        if val & (CMD_STEP_IN | CMD_STEP_OUT) != 0 {
            d.track = if val & CMD_STEP_IN != 0 {
                (d.track + 1).min(TRACKS as u8 - 1)
            } else {
                d.track.saturating_sub(1)
            };
            d.stepped_at = Some(now);
            if d.loaded {
                d.settled_at = now + settle;
            }
        }
        if val & CMD_HEAD_LOAD != 0 && !d.loaded {
            d.loaded = true;
            d.loaded_at = Some(now);
            d.settled_at = now + settle;
        }
        if val & CMD_HEAD_UNLOAD != 0 {
            // a write in flight goes on to its sector's end: the write and
            // trim erase circuits hold the head down until they are done (135)
            d.loaded = false;
        }
        if val & CMD_WRITE != 0 {
            if self.write.is_some() {
                self.finish_write();
            }
            let track = self.drives[drive].track;
            let d = &mut self.drives[drive];
            if d.image.as_ref().is_some_and(|i| i.write_protected) && !d.told_read_only {
                eprintln!("88-DCDD: drive {drive}: the disk is read-only; what is written to it is lost");
                d.told_read_only = true;
            }
            let (serial, _) = self.position();
            self.write = Some(Write { drive, track, serial, slots: [None; SECTOR_BYTES] });
        }
    }

    /// The controller, the write in flight and each drive's disk whole.
    pub fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        let Dcdd {
            drives, selected, now, pacing_hz: _session, interrupts, taken, data, write, write_ended,
            cache: _derived, loaded: _mid_load,
        } = self;
        w.opt(selected, |w, d| w.u8(*d as u8));
        w.u64(*now);
        w.bool(*interrupts);
        w.opt(taken, |w, (s, k)| {
            w.u64(*s);
            w.u64(*k);
        });
        w.u8(*data);
        w.opt(write, |w, wr| {
            w.u8(wr.drive as u8);
            w.u8(wr.track);
            w.u64(wr.serial);
            for slot in &wr.slots {
                w.opt(slot, |w, b| w.u8(*b));
            }
        });
        w.opt(write_ended, |w, t| w.u64(*t));
        for d in drives {
            let Drive { image, track, loaded, settled_at, loaded_at, stepped_at, told_read_only: _session } = d;
            w.u8(*track);
            w.bool(*loaded);
            w.u64(*settled_at);
            w.opt(loaded_at, |w, t| w.u64(*t));
            w.opt(stepped_at, |w, t| w.u64(*t));
            let disk = match image {
                Some(i) => Some((i, i.contents().map_err(|e| format!("{}: {e}", i.path.display()))?)),
                None => None,
            };
            w.opt(&disk, |w, (i, bytes)| {
                w.str(&i.path.display().to_string());
                w.bool(i.write_protected);
                w.bytes(bytes);
            });
        }
        Ok(())
    }

    /// Back from a snapshot. Each drive gets the disk the snapshot held, in
    /// memory and copy-on-write, whatever was in it: a restore never writes
    /// into a file the user owns.
    pub fn load_state(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        let drive = |r: &mut crate::snapshot::Reader| match r.u8()? as usize {
            d if d < DRIVES => Ok(d),
            d => Err(format!("88-DCDD: no drive {d}")),
        };
        self.selected = r.opt(drive)?;
        self.now = r.u64()?;
        self.interrupts = r.bool()?;
        self.taken = r.opt(|r| Ok((r.u64()?, r.u64()?)))?;
        self.data = r.u8()?;
        self.write = r.opt(|r| {
            let drive = drive(r)?;
            let track = r.u8()?;
            let serial = r.u64()?;
            let mut slots = [None; SECTOR_BYTES];
            for slot in &mut slots {
                *slot = r.opt(|r| r.u8())?;
            }
            Ok(Write { drive, track, serial, slots })
        })?;
        self.write_ended = r.opt(|r| r.u64())?;
        let mut disks = Vec::with_capacity(DRIVES);
        for d in &mut self.drives {
            d.track = r.u8()?;
            if d.track as usize >= TRACKS {
                return Err(format!("88-DCDD: a head on track {}", d.track));
            }
            d.loaded = r.bool()?;
            d.settled_at = r.u64()?;
            d.loaded_at = r.opt(|r| r.u64())?;
            d.stepped_at = r.opt(|r| r.u64())?;
            disks.push(r.opt(|r| {
                let path = std::path::PathBuf::from(r.str()?);
                let write_protected = r.bool()?;
                Ok(DiskImage::in_memory(path, r.bytes()?.to_vec(), write_protected))
            })?);
        }
        self.cache = None;
        // the disks wait for `commit_loaded`: the snapshot can still be
        // refused after this section, and the drives must be as they were
        self.loaded = (!r.keeps_disks()).then_some(disks);
        Ok(())
    }

    /// Put in the drives the disks the last load read, if it was not a
    /// rollback's.
    pub fn commit_loaded(&mut self) {
        for (drive, image) in self.loaded.take().into_iter().flatten().enumerate() {
            self.drives[drive].image = image;
        }
        self.cache = None;
    }
}

impl Default for Dcdd {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a sector starts in a disk's image.
fn offset(track: u8, sector: u8) -> u64 {
    ((track as usize * SECTORS + sector as usize) * SECTOR_BYTES) as u64
}

/// Whether an image of `len` bytes is a disk this drive reads: the
/// sectors, and at most a sector's padding after them.
pub fn image_fits(len: u64) -> bool {
    (IMAGE_BYTES..IMAGE_BYTES + SECTOR_BYTES as u64).contains(&len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2 MHz: a byte is 64 cycles, a sector 10,416, a millisecond 2,000.
    const MS: u32 = 2_000;

    fn disk() -> Vec<u8> {
        (0..IMAGE_BYTES as usize).map(|i| (i * 13 + (i >> 9)) as u8).collect()
    }

    fn with_disk() -> Dcdd {
        let mut c = Dcdd::new();
        c.mount(0, Some(DiskImage::in_memory("d.dsk".into(), disk(), false)));
        c
    }

    fn tick(c: &mut Dcdd, cycles: u64) {
        let mut left = cycles;
        while left > 0 {
            let n = left.min(10) as u32;
            c.tick(n);
            left -= n as u64;
        }
    }

    /// On to the start of the next sector.
    fn to_sector_start(c: &mut Dcdd) {
        let s = c.sector_cycles();
        let left = s - c.now % s;
        tick(c, left);
    }

    /// Select drive 0 and load its head, and wait for it to settle.
    fn ready() -> Dcdd {
        let mut c = with_disk();
        c.write(0, 0);
        c.write(1, CMD_HEAD_LOAD);
        tick(&mut c, 40 * MS as u64);
        c
    }

    #[test]
    fn a_drive_is_enabled_only_with_a_disk_in_it() {
        let mut c = Dcdd::new();
        assert_eq!(c.read(0), 0xff, "nothing enabled: every bit false");
        c.write(0, 0);
        assert_eq!(c.read(0), 0xff, "no disk, as with the door open");
        let mut c = with_disk();
        c.write(0, 0);
        let s = c.read(0);
        assert_eq!(s & 0b0001_1000, 0, "D3 and D4 read 0 with a drive enabled");
        assert_eq!(s & ST_TRACK0, 0, "on track 0");
        assert_ne!(s & ST_HS, 0, "the head is up until loaded");
        assert_eq!(c.read(1), 0xff, "no sector position with the head up");
        c.write(0, SELECT_CLEAR);
        assert_eq!(c.read(0), 0xff);
    }

    #[test]
    fn the_head_settles_40ms_after_a_load_and_lifts_on_deselect() {
        let mut c = with_disk();
        c.write(0, 0);
        c.write(1, CMD_HEAD_LOAD);
        tick(&mut c, 39 * MS as u64);
        assert_ne!(c.read(0) & ST_HS, 0);
        assert_ne!(c.read(0) & ST_MH, 0, "no stepping for 40 ms after a load");
        tick(&mut c, MS as u64);
        assert_eq!(c.read(0) & (ST_HS | ST_MH), 0);
        c.write(0, SELECT_CLEAR);
        c.write(0, 0);
        assert_ne!(c.read(0) & ST_HS, 0, "deselected, the head came up");
    }

    #[test]
    fn a_step_may_follow_at_10ms_for_a_millisecond_or_after_31ms() {
        let mut c = ready();
        c.write(1, CMD_STEP_IN);
        assert_ne!(c.read(0) & ST_TRACK0, 0, "off track 0");
        assert_ne!(c.read(0) & ST_HS, 0, "a step unsettles a loaded head");
        let mh = |c: &mut Dcdd| c.read(0) & ST_MH == 0;
        tick(&mut c, 10 * MS as u64 - 10);
        assert!(!mh(&mut c));
        tick(&mut c, 10);
        assert!(mh(&mut c), "the repeat window");
        tick(&mut c, MS as u64);
        assert!(!mh(&mut c));
        tick(&mut c, 20 * MS as u64);
        assert!(mh(&mut c), "settled");
        c.write(1, CMD_STEP_OUT);
        c.write(1, CMD_STEP_OUT);
        assert_eq!(c.read(0) & ST_TRACK0, 0, "back on track 0, and no further out");
    }

    #[test]
    fn the_sector_register_counts_round_with_sector_true_at_each_start() {
        let mut c = ready();
        let sector_cycles = c.sector_cycles();
        assert_eq!(sector_cycles, 10_416);
        // to the start of the next sector
        to_sector_start(&mut c);
        let first = c.read(1);
        assert_eq!(first & 1, 0, "sector true");
        assert_eq!(first & 0xc0, 0xc0);
        tick(&mut c, 60);
        assert_eq!(c.read(1) & 1, 1, "30 us of it");
        tick(&mut c, sector_cycles - 60);
        let next = c.read(1);
        assert_eq!(next & 1, 0, "the next sector's start");
        assert_eq!((next >> 1) & 0x1f, ((first >> 1) + 1) & 0x1f, "the next sector");
    }

    #[test]
    fn a_byte_comes_every_32us_from_140us_into_the_sector_and_a_read_takes_it() {
        let mut c = ready();
        to_sector_start(&mut c);
        let sector = (c.read(1) >> 1) & 0x1f;
        tick(&mut c, 279);
        assert_ne!(c.read(0) & ST_NRDA, 0, "nothing under the head yet");
        tick(&mut c, 1);
        assert_eq!(c.read(0) & ST_NRDA, 0);
        let at = sector as usize * SECTOR_BYTES;
        assert_eq!(c.read(2), disk()[at]);
        assert_ne!(c.read(0) & ST_NRDA, 0, "taken");
        assert_eq!(c.read(2), disk()[at], "the register holds it");
        tick(&mut c, 64);
        assert_eq!(c.read(0) & ST_NRDA, 0);
        assert_eq!(c.read(2), disk()[at + 1]);
    }

    #[test]
    fn a_write_fills_its_slots_repeats_the_last_byte_and_lands_at_the_sectors_end() {
        let mut c = ready();
        let s = c.sector_cycles();
        to_sector_start(&mut c);
        let sector = ((c.read(1) >> 1) & 0x1f) as usize;
        c.write(1, CMD_WRITE);
        assert_ne!(c.read(0) & ST_ENWD, 0, "no byte wanted until 280 us");
        assert_ne!(c.read(0) & ST_NRDA, 0, "nothing read while writing");
        tick(&mut c, 560);
        for b in [0x81u8, 0x22, 0x33] {
            assert_eq!(c.read(0) & ST_ENWD, 0);
            c.write(2, b);
            assert_ne!(c.read(0) & ST_ENWD, 0, "given");
            tick(&mut c, 64);
        }
        assert_ne!(c.read(0) & ST_MH, 0, "no stepping while writing");
        let before = c.disk_contents(0).unwrap();
        assert_eq!(before, disk(), "nothing on the disk until the sector ends");
        tick(&mut c, s);
        let after = c.disk_contents(0).unwrap();
        let mut expect = vec![0x81, 0x22, 0x33];
        expect.resize(SECTOR_BYTES, 0x33);
        assert_eq!(after[sector * SECTOR_BYTES..(sector + 1) * SECTOR_BYTES], expect);
        assert_eq!(after[(sector + 1) * SECTOR_BYTES..], disk()[(sector + 1) * SECTOR_BYTES..]);
    }

    #[test]
    fn a_read_only_disk_keeps_its_sectors() {
        let mut c = Dcdd::new();
        c.mount(0, Some(DiskImage::in_memory("ro.dsk".into(), disk(), true)));
        c.write(0, 0);
        c.write(1, CMD_HEAD_LOAD);
        tick(&mut c, 40 * MS as u64);
        c.write(1, CMD_WRITE);
        tick(&mut c, 600);
        c.write(2, 0x99);
        let s = c.sector_cycles();
        tick(&mut c, s);
        assert_eq!(c.disk_contents(0).unwrap(), disk());
    }

    #[test]
    fn opening_the_enabled_drives_door_clears_disk_control() {
        let mut c = ready();
        c.mount(0, None);
        assert_eq!(c.read(0), 0xff);
        assert!(c.selected.is_none());
    }

    #[test]
    fn the_interrupt_is_sector_true_while_enabled() {
        let mut c = ready();
        let s = c.sector_cycles();
        to_sector_start(&mut c);
        assert!(!c.irq(), "not enabled");
        c.write(1, CMD_INT_ENABLE);
        assert!(c.irq());
        tick(&mut c, 60);
        assert!(!c.irq(), "only for sector true");
        tick(&mut c, s - 60);
        assert!(c.irq());
        c.write(1, CMD_INT_DISABLE);
        assert!(!c.irq());
        c.write(1, CMD_INT_ENABLE);
        c.write(0, SELECT_CLEAR);
        assert!(!c.interrupts, "clearing disk control disables it");
    }

    #[test]
    fn an_image_is_the_sectors_with_at_most_a_sectors_padding() {
        assert_eq!(IMAGE_BYTES, 337_568);
        assert!(image_fits(337_568) && image_fits(337_664));
        assert!(!image_fits(337_567) && !image_fits(337_568 + 137) && !image_fits(204_800));
    }

    /// A snapshot mid-write, loaded into a fresh controller, finishes the
    /// sector as the original does -- and leaves the drives alone until
    /// committed.
    #[test]
    fn a_snapshot_mid_write_finishes_the_same_sector() {
        let mut a = ready();
        let s = a.sector_cycles();
        to_sector_start(&mut a);
        a.write(1, CMD_WRITE);
        tick(&mut a, 560);
        a.write(2, 0x85);
        tick(&mut a, 64);
        a.write(2, 0x44);
        let mut w = crate::snapshot::Writer::new();
        a.save_state(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut b = Dcdd::new();
        let mut r = crate::snapshot::Reader::new(&bytes);
        b.load_state(&mut r).unwrap();
        assert!(!b.has_image(0), "the disk waits for the commit");
        b.commit_loaded();
        for c in [&mut a, &mut b] {
            tick(c, 64);
            c.write(2, 0x55);
            tick(c, s);
        }
        assert_eq!(a.disk_contents(0).unwrap(), b.disk_contents(0).unwrap());
        assert_eq!((a.now, a.read(0), a.read(1)), (b.now, b.read(0), b.read(1)));
    }
}
