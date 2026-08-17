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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

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

/// What a machine hands the frontend when it has a screen. Built by the
/// machine factory alongside the bus; the main thread turns it into an
/// [`sdl::SdlFrontend`].
pub struct Display {
    pub title: &'static str,
    pub video: VideoBuffer,
    /// The character generator rom, 8 bytes per glyph, 256 glyphs.
    pub font_rom: Vec<u8>,
}
