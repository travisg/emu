// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The console, split across the thread boundary.
//!
//! The C++ `Console` is a single object touched by both threads, with the
//! machine registering a callback that the console thread invokes directly
//! into device state (which is how the RC2014 SIO race, `753dd4b`, happened).
//!
//! Here it splits in two:
//!
//! - [`ConsoleFrontend`] runs on the main thread, owns the terminal, and
//!   *sends* keystrokes.
//! - [`ConsoleEndpoint`] lives inside the machine on the CPU thread and
//!   *receives* them.
//!
//! Nothing is shared, so the equivalent race is unrepresentable rather than
//! merely avoided.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

pub mod panel703;
pub mod paper;
pub mod ray703;
pub mod sdl;
pub mod terminal;
pub mod tty703;
pub mod tty33_font;

pub use paper::Paper;

/// The CPU-thread half of the console: keyboard in, serial out.
pub struct ConsoleEndpoint {
    rx: Receiver<u8>,
    out: Box<dyn Write + Send>,
}

impl ConsoleEndpoint {
    pub fn new(rx: Receiver<u8>, out: Box<dyn Write + Send>) -> Self {
        ConsoleEndpoint { rx, out }
    }

    /// Next queued keystroke, if any. Never blocks -- a UART poll must not
    /// stall the CPU.
    pub fn try_next_char(&mut self) -> Option<u8> {
        match self.rx.try_recv() {
            Ok(c) => Some(c),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Emit a byte of serial output. Flushed immediately, as the C++
    /// `Console::Putchar` does, so guest output isn't held in a buffer while
    /// the guest waits for a response to it.
    pub fn put_char(&mut self, c: u8) {
        let _ = self.out.write_all(&[c]);
        let _ = self.out.flush();
    }

    /// Replace the output sink. The endpoint is built in `main` -- before
    /// the factory has parsed the subsystem tokens -- with stdout as its
    /// sink; a factory that routes the serial output somewhere else (the
    /// 703's teletype window) swaps its sink in here before the machine
    /// consumes the endpoint.
    pub fn set_output(&mut self, out: Box<dyn Write + Send>) {
        self.out = out;
    }
}

/// The main-thread half. `run` blocks until the user asks to quit or the CPU
/// side sets the shutdown flag.
pub trait ConsoleFrontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>);
}

/// A character-mode framebuffer shared between the CPU thread (which writes
/// it through the bus) and a display frontend on the main thread (which reads
/// it when the dirty flag is set).
///
/// This is the Rust shape of the C++ `mVideoLock` + `mNeedsRefresh` fix in
/// `043e199`: the mutex is the only way to reach the bytes, so the race that
/// fix closed is unrepresentable here rather than merely avoided.
#[derive(Clone)]
pub struct VideoBuffer {
    ram: Arc<Mutex<Vec<u8>>>,
    dirty: Arc<AtomicBool>,
}

impl VideoBuffer {
    pub fn new(size: usize) -> Self {
        VideoBuffer {
            ram: Arc::new(Mutex::new(vec![0; size])),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn size(&self) -> usize {
        self.ram.lock().unwrap().len()
    }

    /// CPU-side read through the bus.
    pub fn read(&self, offset: usize) -> u8 {
        let ram = self.ram.lock().unwrap();
        ram[offset % ram.len()]
    }

    /// CPU-side write through the bus. Marks the frame dirty.
    pub fn write(&self, offset: usize, val: u8) {
        {
            let mut ram = self.ram.lock().unwrap();
            let i = offset % ram.len();
            ram[i] = val;
        }
        self.dirty.store(true, Ordering::Release);
    }

    /// Frontend-side: run `f` over the whole framebuffer under the lock.
    pub fn with_ram<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        let ram = self.ram.lock().unwrap();
        f(&ram)
    }

    /// Frontend-side: consume the dirty flag. True if a redraw is due.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
}

/// The six positions of the 703 panel's DISPLAY SELECTOR rotary (manual
/// 5-2), in the order they sit around the knob in figure 5-1. The knob is
/// frontend-local state: the core publishes all six sources every step and
/// the frontend picks one to light, which is why the manual can say the
/// selector "can be changed while the program is running".
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Selector {
    /// Machine status: EXR in indicators 0-4, ADFNEG 5, ADFEQL 6, overflow
    /// 7, local/global 8, the sequence register in 12-15.
    Ms,
    /// Instruction register, indicators 0-7.
    In,
    /// Memory address register.
    Ma,
    /// Memory buffer register.
    Mb,
    /// Index register.
    Ix,
    /// Accumulator.
    Ac,
}

/// The 703 front panel's wiring into the machine (manual section 5): the
/// lamp state the core publishes after every step, and the four SENSE
/// toggles the frontend flips and the sense skips read back.
///
/// The same shape as [`VideoBuffer`] -- a `Clone` handle over shared cells,
/// written on the CPU thread, read at frame rate on the main thread -- but
/// plain atomics instead of a mutex: every cell is an independent sampled
/// value with nothing to keep consistent against anything else, so all
/// accesses are `Relaxed` and the core never takes a lock. The frontend
/// only reads through the accessors below; a later lamp model (averaging
/// how long each bit spent lit, the way an incandescent bulb does) replaces
/// the point-sampled cells behind the same API.
#[derive(Clone, Default)]
pub struct PanelState(Arc<PanelInner>);

#[derive(Default)]
struct PanelInner {
    /// PROGRAM COUNTER row, always live (5-1). 15 bits.
    pcr: AtomicU16,
    acr: AtomicU16,
    ixr: AtomicU16,
    /// Last byte address strobed on the bus, instruction fetches included.
    mar: AtomicU16,
    /// Last word through the memory buffer.
    mbr: AtomicU16,
    /// Opcode byte of the last instruction fetched (5-2 puts the
    /// instruction register in indicators 0-7).
    inr: AtomicU8,
    /// The MS position's whole word, packed by the core.
    msw: AtomicU16,
    /// SENSE toggles, bit n = switch n, set = up = true. Skips take the
    /// *false* position (5-4), so all-clear preserves the no-panel
    /// behaviour of every sense skip skipping.
    sense: AtomicU8,
    /// Whether the machine sits halted at the panel, published by the run
    /// loop (the only place that knows -- HLT and bad opcodes halt there
    /// too, not just the switch). Drives the HALT indicator's red lens.
    halted: AtomicBool,

    /// Lamp on-time accumulators, for rendering the lamps as incandescent
    /// bulbs rather than point samples: for each lamp source (PC row plus
    /// the six selector positions, indexed by [`PanelState::accumulate`]'s
    /// convention) and each indicator, the number of clock cycles that bit
    /// has spent set; and the total cycles accumulated. All monotonic --
    /// the frontend diffs snapshots rather than resetting anything, so
    /// there is no read-modify-write race across the thread boundary.
    on: [[AtomicU64; 16]; 7],
    on_cycles: AtomicU64,
}

/// One frame's view of a lamp source: the per-indicator on-cycle counters
/// and the total cycles, both monotonic since power-on. Duty over an
/// interval is the ratio of the deltas of two snapshots.
#[derive(Copy, Clone, Default)]
pub struct LampSnapshot {
    pub bits: [u64; 16],
    pub cycles: u64,
}

impl PanelState {
    pub fn new() -> Self {
        Self::default()
    }

    // -- the CPU-thread side, called by the core ---------------------------

    pub fn set_registers(&self, pcr: u16, acr: u16, ixr: u16, msw: u16) {
        self.0.pcr.store(pcr, Ordering::Relaxed);
        self.0.acr.store(acr, Ordering::Relaxed);
        self.0.ixr.store(ixr, Ordering::Relaxed);
        self.0.msw.store(msw, Ordering::Relaxed);
    }

    pub fn set_memory(&self, mar: u16, mbr: u16) {
        self.0.mar.store(mar, Ordering::Relaxed);
        self.0.mbr.store(mbr, Ordering::Relaxed);
    }

    pub fn set_instruction(&self, inr: u8) {
        self.0.inr.store(inr, Ordering::Relaxed);
    }

    /// The position of SENSE toggle `n` (0-3): true = up.
    pub fn sense(&self, n: u8) -> bool {
        self.0.sense.load(Ordering::Relaxed) & (1 << n) != 0
    }

    /// Flip one bit of the memory buffer by indicator number (0 = MSB).
    /// The MBR lives here rather than in the core: the program's memory
    /// traffic and the operator's keying share it, exactly as they shared
    /// the real register. Only the CPU thread calls this.
    pub fn toggle_mbr_bit(&self, indicator: u8) {
        self.0.mbr.fetch_xor(0x8000 >> (indicator & 15), Ordering::Relaxed);
    }

    /// CPU-thread side: replace the memory buffer (the display CLEAR, and
    /// DISPLAY's read-back).
    pub fn set_mbr(&self, v: u16) {
        self.0.mbr.store(v, Ordering::Relaxed);
    }

    /// CPU-thread side, called by the `Emulator` on every run-state change:
    /// the machine is sitting halted at the panel. Not derivable frontend-
    /// side from the cycle accumulators -- under a slow `--throttle` a
    /// *running* machine has whole frames with no cycles in them.
    pub fn set_halted(&self, halted: bool) {
        self.0.halted.store(halted, Ordering::Relaxed);
    }

    /// Charge `cycles` of on-time to every currently lit bit of every lamp
    /// source, reading the point-sample cells the core just published.
    /// Called once per executed step, *never* from a switch actuation --
    /// advancing the cycle total while halted would break the frontend's
    /// halted detection (delta cycles == 0 over a frame).
    ///
    /// Source indices: 0 = the PC row, 1.. = the six selector positions in
    /// `Selector` order. Cost is one relaxed fetch_add per *set* bit per
    /// step; if that ever matters it can batch in core-local counters and
    /// flush every N steps behind this same signature.
    pub fn accumulate(&self, cycles: u32) {
        let c = cycles as u64;
        if c == 0 {
            return;
        }
        let sources = [
            self.program_counter(),
            self.selected(Selector::Ms),
            self.selected(Selector::In),
            self.selected(Selector::Ma),
            self.selected(Selector::Mb),
            self.selected(Selector::Ix),
            self.selected(Selector::Ac),
        ];
        for (src, &value) in sources.iter().enumerate() {
            let mut bits = value;
            while bits != 0 {
                // indicator number: bit 0 is the MSB, as everywhere here
                let i = bits.leading_zeros() as usize;
                self.0.on[src][i].fetch_add(c, Ordering::Relaxed);
                bits &= !(0x8000 >> i);
            }
        }
        self.0.on_cycles.fetch_add(c, Ordering::Relaxed);
    }

    fn snapshot_source(&self, src: usize) -> LampSnapshot {
        let mut s = LampSnapshot { bits: [0; 16], cycles: self.0.on_cycles.load(Ordering::Relaxed) };
        for (i, ctr) in self.0.on[src].iter().enumerate() {
            s.bits[i] = ctr.load(Ordering::Relaxed);
        }
        s
    }

    /// Frontend-side: the PC row's accumulators.
    pub fn snapshot_pc(&self) -> LampSnapshot {
        self.snapshot_source(0)
    }

    /// Frontend-side: one selector position's accumulators.
    pub fn snapshot(&self, sel: Selector) -> LampSnapshot {
        self.snapshot_source(1 + sel as usize)
    }

    // -- the frontend side -------------------------------------------------

    pub fn program_counter(&self) -> u16 {
        self.0.pcr.load(Ordering::Relaxed)
    }

    /// The word behind the SELECTED DISPLAY row for one selector position.
    pub fn selected(&self, sel: Selector) -> u16 {
        match sel {
            Selector::Ms => self.0.msw.load(Ordering::Relaxed),
            Selector::In => (self.0.inr.load(Ordering::Relaxed) as u16) << 8,
            Selector::Ma => self.0.mar.load(Ordering::Relaxed),
            Selector::Mb => self.0.mbr.load(Ordering::Relaxed),
            Selector::Ix => self.0.ixr.load(Ordering::Relaxed),
            Selector::Ac => self.0.acr.load(Ordering::Relaxed),
        }
    }

    pub fn toggle_sense(&self, n: u8) {
        self.0.sense.fetch_xor(1 << n, Ordering::Relaxed);
    }

    /// Frontend-side: is the machine halted at the panel? Lights the HALT
    /// indicator's red lens.
    pub fn halted(&self) -> bool {
        self.0.halted.load(Ordering::Relaxed)
    }
}

/// Units in the 74601 disc rack. `dev/disc74601.rs` pins its `DISC_UNITS`
/// to this at compile time.
pub const RACK_UNITS: usize = 4;

/// The disc rack's lamps: ONLINE, READ and WRITE for each of the four
/// 74601 units, drawn below the console in the panel window. Same shape as
/// [`PanelState`] -- a `Clone` handle over relaxed atomics, written by the
/// disc controller on the CPU thread, read at frame rate -- and the
/// activity counters are monotonic on-cycle accumulators for the same
/// reason: the frontend diffs snapshots, so there is no read-modify-write
/// race across the thread boundary.
///
/// The op counters exist because cycle duty alone cannot show a `--fast-io`
/// transfer: it completes with zero cycles charged, inside a single poll.
/// A completion count that moved this frame is the frontend's cue to wink
/// the lamp anyway.
#[derive(Clone, Default)]
pub struct DiscRackState(Arc<DiscRackInner>);

#[derive(Default)]
struct DiscRackInner {
    /// A platter is mounted and spinning.
    online: [AtomicBool; RACK_UNITS],
    /// Cycles each unit has spent with a read (or verify -- the heads are
    /// reading) or write in flight. Monotonic.
    read_on: [AtomicU64; RACK_UNITS],
    write_on: [AtomicU64; RACK_UNITS],
    /// Completed operations per unit. Monotonic.
    read_ops: [AtomicU64; RACK_UNITS],
    write_ops: [AtomicU64; RACK_UNITS],
    /// Total cycles seen by the controller: the duty denominator.
    cycles: AtomicU64,
}

/// One frame's view of the rack. Duty over an interval is the ratio of the
/// deltas of two snapshots, exactly as with [`LampSnapshot`].
#[derive(Copy, Clone, Default)]
pub struct RackSnapshot {
    pub online: [bool; RACK_UNITS],
    pub read_on: [u64; RACK_UNITS],
    pub write_on: [u64; RACK_UNITS],
    pub read_ops: [u64; RACK_UNITS],
    pub write_ops: [u64; RACK_UNITS],
    pub cycles: u64,
}

impl DiscRackState {
    pub fn new() -> Self {
        Self::default()
    }

    // -- the CPU-thread side, called by the disc controller ----------------

    pub fn set_online(&self, unit: usize, online: bool) {
        self.0.online[unit].store(online, Ordering::Relaxed);
    }

    /// Advance the duty denominator: called on every controller poll,
    /// transfer in flight or not, so an idle lamp's duty decays toward zero
    /// instead of freezing at its last value.
    pub fn advance(&self, cycles: u32) {
        if cycles != 0 {
            self.0.cycles.fetch_add(cycles as u64, Ordering::Relaxed);
        }
    }

    /// Charge in-flight time to a unit's READ or WRITE lamp.
    pub fn charge(&self, unit: usize, write: bool, cycles: u32) {
        if cycles == 0 {
            return;
        }
        let ctr = if write { &self.0.write_on[unit] } else { &self.0.read_on[unit] };
        ctr.fetch_add(cycles as u64, Ordering::Relaxed);
    }

    /// Count a completed operation -- the `--fast-io` wink source.
    pub fn complete(&self, unit: usize, write: bool) {
        let ctr = if write { &self.0.write_ops[unit] } else { &self.0.read_ops[unit] };
        ctr.fetch_add(1, Ordering::Relaxed);
    }

    // -- the frontend side -------------------------------------------------

    pub fn snapshot(&self) -> RackSnapshot {
        let mut s = RackSnapshot { cycles: self.0.cycles.load(Ordering::Relaxed), ..Default::default() };
        for i in 0..RACK_UNITS {
            s.online[i] = self.0.online[i].load(Ordering::Relaxed);
            s.read_on[i] = self.0.read_on[i].load(Ordering::Relaxed);
            s.write_on[i] = self.0.write_on[i].load(Ordering::Relaxed);
            s.read_ops[i] = self.0.read_ops[i].load(Ordering::Relaxed);
            s.write_ops[i] = self.0.write_ops[i].load(Ordering::Relaxed);
        }
        s
    }
}

/// One actuation of a front panel control, sent from the panel window to
/// the run loop on the CPU thread. The run-state switches (`Run`, `Halt`,
/// `SingleCommand`, `Reset`) are handled by the `Emulator` itself; the
/// data-entry commands are forwarded to the core's `panel_command`.
///
/// Bit arguments carry the manual's *indicator number*: 0-15 counted from
/// the left, indicator 0 the most significant bit, exactly as the lamps
/// are drawn. The core translates to its own shift arithmetic.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PanelCommand {
    /// RUN "initiates a program process" (5-4).
    Run,
    /// HALT "halts the program being executed at the completion of the
    /// current instruction" (5-4).
    Halt,
    /// SINGLE COMMAND: "each actuation of the switch executes one
    /// instruction ... then halts" (5-3).
    SingleCommand,
    /// RESET, "the master reset switch" (5-3).
    Reset,
    /// One PROGRAM COUNTER switch-indicator, "always active for both entry
    /// and display" (5-1) -- honoured while running, too.
    TogglePcBit(u8),
    /// The PROGRAM COUNTER CLEAR switch (5-1).
    ClearPc,
    /// One SELECTED DISPLAY switch-indicator. Entry reaches only the MB,
    /// IX and AC positions; the rest are display-only (5-2).
    ToggleSelectedBit(Selector, u8),
    /// The display CLEAR switch, which "clears the register associated
    /// with DISPLAY SELECTOR positions MB, IX, or AC" (5-3).
    ClearSelected(Selector),
    /// ENTER: the MBR into memory at the PCR, which then increments (5-3).
    Enter,
    /// DISPLAY: memory at the PCR into the MBR, PCR increments (5-3).
    Display,
}

/// What a machine hands the frontend when it has something to show. Built
/// by the machine factory alongside the bus; the variant tells the main
/// thread which frontend to build, so frontend selection stays out of the
/// system names.
pub enum Display {
    /// A character-mode screen rendered by [`sdl::SdlFrontend`].
    CharCell {
        title: &'static str,
        video: VideoBuffer,
        /// The character generator rom, 8 bytes per glyph, 256 glyphs.
        font_rom: Vec<u8>,
    },
    /// The 703's windows, rendered together by one
    /// [`ray703::Ray703Frontend`]: SDL allows one context and one event
    /// pump per process, so however many windows the subsystem tokens ask
    /// for must share a frontend, and the composition lives in this variant
    /// rather than in `main`. At least one part is `Some` -- a machine with
    /// no window returns no `Display` at all.
    Ray703 { title: &'static str, panel: Option<PanelDisplay>, tty: Option<TtyDisplay> },
}

/// The teletype window's share of [`Display::Ray703`]: the paper the
/// machine's serial output prints onto (the factory has already made it
/// the `ConsoleEndpoint`'s sink).
pub struct TtyDisplay {
    pub paper: Paper,
}

/// The front panel window's share of [`Display::Ray703`]: the lamp state
/// the core publishes, the switch actuations back to the run loop, and the
/// disc rack lamps drawn below the console.
pub struct PanelDisplay {
    pub panel: PanelState,
    /// Switch actuations to the run loop on the CPU thread.
    pub control: Sender<PanelCommand>,
    pub rack: DiscRackState,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The factory can retarget the serial output sink after `main` has
    /// built the endpoint around stdout -- this is the seam the teletype
    /// window hangs off.
    #[test]
    fn set_output_redirects_serial_output() {
        let (_tx, rx) = std::sync::mpsc::channel();
        let mut ep = ConsoleEndpoint::new(rx, Box::new(std::io::sink()));
        let paper = Paper::new();
        ep.set_output(Box::new(paper.clone()));
        ep.put_char(b'A');
        paper.with_lines(|lines, _| assert_eq!(lines[0][0], b'A'));
    }

    /// Rack counters are monotonic and diff to a duty, the LampSnapshot
    /// convention.
    #[test]
    fn rack_accumulators_yield_duty_and_ops_are_monotonic() {
        let r = DiscRackState::new();
        let before = r.snapshot();
        r.advance(100);
        r.charge(1, false, 25);
        r.charge(1, true, 10);
        r.complete(1, false);
        let after = r.snapshot();
        assert_eq!(after.cycles - before.cycles, 100);
        assert_eq!(after.read_on[1] - before.read_on[1], 25);
        assert_eq!(after.write_on[1] - before.write_on[1], 10);
        assert_eq!(after.read_ops[1] - before.read_ops[1], 1);
        assert_eq!(after.read_on[0], 0, "other units untouched");
        r.advance(0);
        assert_eq!(r.snapshot().cycles, after.cycles, "zero cycles accrue nothing");
    }

    #[test]
    fn accumulate_charges_only_set_bits() {
        let p = PanelState::new();
        p.set_registers(0x8001, 0xffff, 0x0000, 0x0000);
        p.accumulate(5);
        let pc = p.snapshot_pc();
        assert_eq!(pc.cycles, 5);
        assert_eq!(pc.bits[0], 5, "indicator 0 is the MSB");
        assert_eq!(pc.bits[15], 5);
        assert_eq!(pc.bits[1], 0);
        let ac = p.snapshot(Selector::Ac);
        assert!(ac.bits.iter().all(|&b| b == 5));
        let ix = p.snapshot(Selector::Ix);
        assert!(ix.bits.iter().all(|&b| b == 0));
    }

    #[test]
    fn accumulate_halves_for_alternating_values() {
        let p = PanelState::new();
        for i in 0..10 {
            p.set_registers(0, if i % 2 == 0 { 0x8000 } else { 0x0001 }, 0, 0);
            p.accumulate(3);
        }
        let ac = p.snapshot(Selector::Ac);
        assert_eq!(ac.cycles, 30);
        assert_eq!(ac.bits[0], 15);
        assert_eq!(ac.bits[15], 15);
        assert_eq!(ac.bits[7], 0);
    }

    /// Zero cycles accrue nothing, and the IN position charges the opcode
    /// byte at indicators 0-7 exactly as it displays there.
    #[test]
    fn accumulate_places_the_instruction_register_at_the_top() {
        let p = PanelState::new();
        p.set_instruction(0x81);
        p.accumulate(0);
        assert_eq!(p.snapshot(Selector::In).cycles, 0);
        p.accumulate(2);
        let s = p.snapshot(Selector::In);
        assert_eq!(s.bits[0], 2);
        assert_eq!(s.bits[7], 2);
        assert_eq!(s.bits[8], 0);
    }
}
