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
//! Raytheon 703 DIO devices: the console teletype and the paper tape reader.
//!
//! Both hang off the 16-bit Direct Input/Output channel rather than the memory
//! space, so neither is a `MemoryDevice`: the machine decodes the DIO address
//! and calls these directly, the way `Z80Sio` and `Wd1793` are driven from the
//! z80 machines' port handlers.
//!
//! The device and function codes come from the *Teletype - High Speed Paper
//! Tape I/O Driver* listing (drawing 392292, transcribed alongside the scan),
//! and are confirmed by the PTB bootstrap (drawing 390364), which drives both
//! devices with the same two instructions:
//!
//! ```text
//!     DOT 14,9    start the teleprinter's tape reader
//!     DIN 14,13   collect the frame it just read
//!     DOT 13,9    start the high speed reader
//!     DIN 13,13   collect the frame
//! ```
//!
//! Neither device is polled. Both interrupt once per character, and the
//! program collects the character with a DIN inside the service routine --
//! which is why the 703 is the first machine in this tree that needs a working
//! interrupt path at all.

use crate::console::ConsoleEndpoint;
use std::io;
use std::path::Path;

/// DIO device codes (392292, "Individual Devices").
pub const DEV_TTY: u8 = 0xe;
pub const DEV_TAPE_READER: u8 = 0xd;

/// DIO function codes. The read functions arm a device; function D collects
/// the frame it captured.
const FN_READER_START: u8 = 0x8;
const FN_READ: u8 = 0x9;
const FN_READ_KEYBOARD: u8 = 0xb;
const FN_COLLECT: u8 = 0xd;
const FN_WRITE: u8 = 0xe;

/// Console teletype, DIO device 14.
///
/// The 703's software world is eight-bit ASCII with the high bit *set* -- the
/// driver documentation spells out CR as 8D, LF as 8A, blank as A0 and RUBOUT
/// as FF. A modern terminal wants seven, so the bit goes on at the keyboard
/// and comes off at the printer.
pub struct Tty703 {
    console: ConsoleEndpoint,
    /// Which interrupt level this device signals.
    level: u8,
    /// Set by a read function, cleared by anything else. An unarmed teletype
    /// must not drain the keystroke channel, or characters typed before the
    /// program asks for them would vanish instead of queueing.
    armed: bool,
    /// A real Model 33 prints what you type; function 9 selects the tape
    /// reader instead and "characters read are not typed".
    echo: bool,
    /// The captured frame, waiting for the program's DIN.
    frame: Option<u8>,
}

impl Tty703 {
    pub fn new(console: ConsoleEndpoint, level: u8) -> Self {
        Tty703 { console, level, armed: false, echo: false, frame: None }
    }

    pub fn dot(&mut self, function: u8, val: u16) {
        match function {
            FN_WRITE => {
                // Strip the high bit the 703 conventionally carries. Bare
                // masking is what the MC6850 does too; nothing here tries to
                // be clever about the NULs and RUBOUTs that pad a tape record.
                self.console.put_char((val & 0x7f) as u8);
            }
            FN_READ_KEYBOARD => {
                self.armed = true;
                self.echo = true;
            }
            FN_READ => {
                self.armed = true;
                self.echo = false;
            }
            // Anything else disconnects the input side. The driver does this
            // between records; there is no separate stop function code.
            _ => self.armed = false,
        }
    }

    pub fn din(&mut self, function: u8) -> u16 {
        match function {
            // Taking the frame is also what re-arms the capture, so a service
            // routine that forgets its DIN simply stops receiving.
            FN_COLLECT => self.frame.take().unwrap_or(0) as u16,
            _ => 0,
        }
    }

    /// Capture at most one keystroke and report the interrupt level it should
    /// pulse, as a bitmask. Nothing happens until the program has armed the
    /// device, and nothing happens while a frame is still uncollected.
    pub fn poll(&mut self) -> u16 {
        if !self.armed || self.frame.is_some() {
            return 0;
        }
        let Some(c) = self.console.try_next_char() else {
            return 0;
        };
        // A terminal in raw mode sends CR for Return, but a pipe feeding the
        // emulator a script sends LF; the guest only understands CR.
        let c = if c == 0x0a { 0x0d } else { c };
        if self.echo {
            self.console.put_char(c);
            if c == 0x0d {
                self.console.put_char(0x0a);
            }
        }
        self.frame = Some(c | 0x80);
        1 << self.level
    }
}

/// High speed paper tape reader, DIO device 13.
///
/// Read-only and one frame at a time, which is all the hardware was: "a
/// unidirectional photoelectric reader capable of reading 300 characters per
/// second". A tape is small enough to hold in memory outright -- the whole
/// point of an absolute paper tape is that it fits in the machine it loads.
pub struct TapeReader703 {
    level: u8,
    frames: Vec<u8>,
    pos: usize,
    /// Set by the start function. The real reader free-runs from there: PTB
    /// issues its DOT once and never again.
    running: bool,
    frame: Option<u8>,
}

impl TapeReader703 {
    pub fn new(level: u8) -> Self {
        TapeReader703 { level, frames: Vec::new(), pos: 0, running: false, frame: None }
    }

    /// Mount a tape image: one byte per frame, exactly as punched.
    ///
    /// Unlike the Kaypro's floppy this is a hard error, because a tape is only
    /// ever mounted when the user named one on the command line -- silently
    /// running with no tape would look like a hung machine.
    pub fn load_image(&mut self, path: &Path) -> io::Result<()> {
        let frames = std::fs::read(path)?;
        println!("703: mounted paper tape '{}' ({} frames)", path.display(), frames.len());
        self.load_tape(frames);
        Ok(())
    }

    pub fn load_tape(&mut self, frames: Vec<u8>) {
        self.frames = frames;
        self.pos = 0;
    }

    pub fn dot(&mut self, function: u8, _val: u16) {
        self.running = matches!(function, FN_READER_START | FN_READ);
    }

    pub fn din(&mut self, function: u8) -> u16 {
        match function {
            FN_COLLECT => self.frame.take().unwrap_or(0) as u16,
            _ => 0,
        }
    }

    /// Advance the tape by one frame and report the level to pulse. Running
    /// off the end simply stops interrupting, which is what an operator sees
    /// when the tape runs out of the reader.
    pub fn poll(&mut self) -> u16 {
        if !self.running || self.frame.is_some() || self.pos >= self.frames.len() {
            return 0;
        }
        self.frame = Some(self.frames[self.pos]);
        self.pos += 1;
        1 << self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    /// A serial sink a test can read back, since `ConsoleEndpoint` takes
    /// ownership of the `Write` it is given.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A teletype with `input` queued and its output captured.
    fn tty_capturing(input: &[u8]) -> (Tty703, Captured) {
        let (tx, rx) = mpsc::channel();
        for &c in input {
            tx.send(c).unwrap();
        }
        let sink = Captured::default();
        let tty = Tty703::new(ConsoleEndpoint::new(rx, Box::new(sink.clone())), 0);
        (tty, sink)
    }

    fn tty_with_input(input: &[u8]) -> Tty703 {
        let (tx, rx) = mpsc::channel();
        for &c in input {
            tx.send(c).unwrap();
        }
        Tty703::new(ConsoleEndpoint::new(rx, Box::new(Vec::new())), 0)
    }

    #[test]
    fn an_unarmed_teletype_does_not_touch_the_keyboard() {
        let mut tty = tty_with_input(b"A");
        assert_eq!(tty.poll(), 0);
        // the keystroke is still queued, not swallowed
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(), 1);
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
    }

    #[test]
    fn received_characters_carry_the_high_bit() {
        let mut tty = tty_with_input(b"Az");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
        assert_eq!(tty.poll(), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'z' as u16);
    }

    /// The guest speaks in carriage returns; a script piped into the emulator
    /// speaks in line feeds.
    #[test]
    fn linefeed_arrives_as_a_carriage_return() {
        let mut tty = tty_with_input(b"\n");
        tty.dot(FN_READ, 0);
        tty.poll();
        assert_eq!(tty.din(FN_COLLECT), 0x8d);
    }

    #[test]
    fn a_frame_is_held_until_it_is_collected() {
        let mut tty = tty_with_input(b"AB");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(), 1);
        assert_eq!(tty.poll(), 0, "the second character waits for the first DIN");
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
        assert_eq!(tty.poll(), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'B' as u16);
    }

    #[test]
    fn only_the_collect_function_returns_the_frame() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ, 0);
        tty.poll();
        assert_eq!(tty.din(0x0), 0);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
    }

    #[test]
    fn a_non_read_function_disconnects_the_input() {
        let mut tty = tty_with_input(b"AB");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(), 1);
        tty.din(FN_COLLECT);
        tty.dot(0x3, 0);
        assert_eq!(tty.poll(), 0);
    }

    #[test]
    fn output_drops_the_high_bit() {
        let (mut tty, out) = tty_capturing(b"");
        // 8D is the 703's carriage return and A0 its blank; a terminal wants
        // seven-bit ASCII.
        for c in [0x8du16, 0x8a, 0xc1, 0xa0] {
            tty.dot(FN_WRITE, c);
        }
        assert_eq!(*out.0.lock().unwrap(), b"\r\nA ");
        assert_eq!(tty.poll(), 0, "writing must not arm the input side");
    }

    /// Function B is the Model 33 keyboard, which prints what you type;
    /// function 9 is its tape reader, whose "characters read are not typed".
    #[test]
    fn only_the_keyboard_function_echoes() {
        let (mut tty, out) = tty_capturing(b"A\n");
        tty.dot(FN_READ_KEYBOARD, 0);
        tty.poll();
        tty.din(FN_COLLECT);
        tty.poll();
        assert_eq!(*out.0.lock().unwrap(), b"A\r\n", "a bare CR needs the LF added");

        let (mut tty, out) = tty_capturing(b"A");
        tty.dot(FN_READ, 0);
        tty.poll();
        assert!(out.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_tape_reader_needs_starting() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x01, 0x02]);
        assert_eq!(reader.poll(), 0);
        reader.dot(FN_READ, 0);
        assert_eq!(reader.poll(), 1);
        assert_eq!(reader.din(FN_COLLECT), 0x01);
        assert_eq!(reader.poll(), 1);
        assert_eq!(reader.din(FN_COLLECT), 0x02);
    }

    #[test]
    fn running_off_the_end_of_the_tape_stops_interrupting() {
        let mut reader = TapeReader703::new(3);
        reader.load_tape(vec![0xff]);
        reader.dot(FN_READER_START, 0);
        assert_eq!(reader.poll(), 1 << 3, "the level is configurable");
        reader.din(FN_COLLECT);
        assert_eq!(reader.poll(), 0);
        assert_eq!(reader.din(FN_COLLECT), 0);
    }

    #[test]
    fn an_empty_tape_never_interrupts() {
        let mut reader = TapeReader703::new(0);
        reader.dot(FN_READ, 0);
        assert_eq!(reader.poll(), 0);
    }
}
