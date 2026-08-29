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
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

pub mod panel703;
pub mod sdl;
pub mod terminal;

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
    /// The 703's lights-and-switches front panel.
    Panel703 { title: &'static str, panel: PanelState },
}
