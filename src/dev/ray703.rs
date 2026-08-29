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
//!
//! Output works the same way, and the X-RAY executive listing (drawing 390779,
//! transcribed as `test/703/390779_XRAY_listing.txt` on storage) shows the
//! whole protocol, because it carries the teletype driver with it on its pages
//! titled "INTERRUPT SERVICE AREA FOR TTY AND HSPT DRIVERS":
//!
//! * The setup routine hands the *first* character to the printer directly and
//!   then returns to "WAIT FOR IRS" (card 749). Every character after that is
//!   written from inside the service routine, which the driver reaches only by
//!   being interrupted.
//! * One interrupt line serves both directions. `IRH0`, X-RAY's level 0 stub,
//!   is commented "FOR TY AND HSPT" (card 1360), and the driver's single entry
//!   `TIRS` decides what happened by testing the sign of the operation word in
//!   the caller's file table (`SAM  READ OR WRITE`, card 782) rather than by
//!   asking the hardware.
//!
//! So a `DOT dev,E` has to raise the device's interrupt when the character has
//! been printed. Nothing else advances a 703 driver's output loop, and without
//! it X-RAY hangs in its I/O monitor's wait-for-completion loop forever.
//!
//! "When the character has been printed" is a tenth of a second later, because
//! the console is a Model 33 running at ten characters a second. The teletype
//! is paced to that rate in *machine* time -- it is handed the clock cycles the
//! CPU spent, and counts them -- rather than off a wall clock, which is what
//! makes `--throttle` come out right: throttling paces cycles against real
//! seconds for the whole machine at once, so under `--throttle` the printer
//! runs at ten characters a second of real time, and unthrottled it runs at ten
//! characters per 57,142 emulated cycles, however fast the host gets through
//! them. The guest sees period-correct timing either way, which is the part a
//! program can actually observe.

use crate::console::ConsoleEndpoint;
use crate::cpu::ray703::CLOCK_HZ;
use std::io;
use std::path::Path;

/// DIO device codes (392292, "Individual Devices").
pub const DEV_TTY: u8 = 0xe;
pub const DEV_TAPE_READER: u8 = 0xd;

/// Characters a second for the Model 33 on the console. The driver listing
/// (392292, "Individual Devices") says it outright: "the standard Teletype
/// Model 33/35 (Automatic Send-Receive) runs at up to ten characters per
/// second". That is the 110-baud line with eleven bits to the character --
/// start, eight data, two stop -- which comes to exactly ten.
const TTY_CHARS_PER_SEC: u64 = 10;

/// The same rate as a count of 703 clock cycles, which is the only time base
/// a device on this bus has -- see `Bus::poll_interrupt_lines`. 571429/10
/// truncates to 57,142 cycles, 99.9985 ms at the 1.75 us cycle: a tenth of a
/// second to far better than a 1968 teletype's motor held it.
const TTY_CHAR_CYCLES: u32 = (CLOCK_HZ / TTY_CHARS_PER_SEC) as u32;

/// DIO function codes. The read functions arm a device; the collect functions
/// take the frame it captured.
///
/// The collect code is the read code with bit 2 set, not a constant: X-RAY
/// builds its collecting DIN by exclusive-oring the operation word it was
/// opened with against `X84` (`X'8004'`, card 1521), which clears the sign bit
/// that marks a read and flips exactly that bit of the function nibble. So
/// function 9 collects with D and function B collects with F.
const FN_DISCONNECT: u8 = 0x0;
const FN_READER_START: u8 = 0x8;
const FN_READ: u8 = 0x9;
const FN_READ_KEYBOARD: u8 = 0xb;
const FN_COLLECT: u8 = 0xd;
const FN_COLLECT_KEYBOARD: u8 = 0xf;
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
    /// Set by a read function or by a collecting DIN, cleared by anything
    /// else. An unarmed teletype must not drain the keystroke channel, or
    /// characters typed before the program asks for them would vanish instead
    /// of queueing.
    armed: bool,
    /// A real Model 33 prints what you type; function 9 selects the tape
    /// reader instead and "characters read are not typed".
    echo: bool,
    /// The captured frame, waiting for the program's DIN.
    frame: Option<u8>,
    /// Completion interrupts the printer still owes, one per character it was
    /// handed. A counter and not a flag: with a character taking a tenth of a
    /// second, a second `DOT dev,E` arriving before the first has finished is
    /// reachable in a way it was not when a character completed on the next
    /// instruction, and a lost completion hangs a driver silently. Characters
    /// queue at the line rate rather than overwriting each other; nothing says
    /// what a 703 teletype did with a write on top of a busy printer, and no
    /// real driver issues one, so the choice only decides how a broken driver
    /// fails -- late output rather than a wait that never ends.
    tx_owed: u32,
    /// Cycles left in the character currently being printed. Meaningless
    /// unless `tx_owed` is nonzero.
    tx_remaining: u32,
    /// Cycles banked toward the next character the keyboard may deliver,
    /// saturating at one character time. The saturation is the point: the
    /// budget goes on accumulating while the device is unarmed or holding an
    /// uncollected frame, and without a ceiling a machine that sat in its wait
    /// loop for a second would then take ten queued keystrokes back to back at
    /// no rate at all -- which is exactly the case that matters, a line typed
    /// at a prompt. One character's credit means the first keystroke after an
    /// idle is instant and the second still waits its turn.
    rx_credit: u32,
    /// Cycles per character, normally [`TTY_CHAR_CYCLES`]. Zero -- set by
    /// `set_fast_io`, behind the `--fast-io` flag -- collapses every wait
    /// above to "on the next poll", which is exactly the model this device
    /// had before it was paced: completions still arrive one per poll and
    /// keystrokes still queue behind an uncollected frame, only the time
    /// is gone.
    char_cycles: u32,
}

impl Tty703 {
    pub fn new(console: ConsoleEndpoint, level: u8) -> Self {
        Tty703 {
            console,
            level,
            armed: false,
            echo: false,
            frame: None,
            tx_owed: 0,
            tx_remaining: 0,
            // A machine that has just been started has not kept the operator
            // waiting, so the first character in is free.
            rx_credit: TTY_CHAR_CYCLES,
            char_cycles: TTY_CHAR_CYCLES,
        }
    }

    /// Run at host speed instead of ten characters a second (`--fast-io`).
    pub fn set_fast_io(&mut self) {
        self.char_cycles = 0;
    }

    pub fn dot(&mut self, function: u8, val: u16) {
        match function {
            FN_WRITE => {
                // Strip the high bit the 703 conventionally carries. Bare
                // masking is what the MC6850 does too; nothing here tries to
                // be clever about the NULs and RUBOUTs that pad a tape record.
                self.console.put_char((val & 0x7f) as u8);
                // The character appears on the terminal now and the printer
                // reports it finished a character time later. Printing it at
                // the completion instead would be just as defensible -- a
                // Model 33 is hammering the type box across that tenth of a
                // second either way -- but this way the pacing shows up as the
                // gap *after* each character, so a program that writes one
                // character and stops has already printed it.
                //
                // Only an idle printer starts a fresh character time; a write
                // that lands on a busy one queues behind it, so completions
                // stay a character apart however fast the program writes.
                if self.tx_owed == 0 {
                    self.tx_remaining = self.char_cycles;
                }
                self.tx_owed = self.tx_owed.saturating_add(1);
            }
            FN_READ_KEYBOARD => {
                self.armed = true;
                self.echo = true;
            }
            FN_READ => {
                self.armed = true;
                self.echo = false;
            }
            // Function 0 is the disconnect. X-RAY's M.TDISC2 ("THIS ROUTINE
            // WILL DISCONNECT THE DEVICE BEING USED") builds it by masking the
            // function nibble out of the operation word with `X7F`, which is
            // X'7FF0' (card 1517), and oring the DOT opcode back in. Anything
            // else unrecognised lands here too; there is no other stop code.
            FN_DISCONNECT => self.armed = false,
            _ => self.armed = false,
        }
    }

    pub fn din(&mut self, function: u8) -> u16 {
        match function {
            // A collecting DIN hands over the frame *and asks for the next
            // one*, so a service routine that forgets its DIN simply stops
            // receiving.
            //
            // The asking is not decoration. X-RAY starts a console read by
            // executing this instruction and nothing else: its setup routine
            // builds the collecting DIN into the cell `NSPEC` and executes it
            // under the comment "SELECT THE DEVICE" (card 701), and there is
            // no arming DOT anywhere on the read path. Booting the real
            // executive is what settled it -- with the DIN inert, X-RAY opens
            // the console, selects it, and waits forever for a keystroke the
            // teletype was never told to listen for.
            //
            // Which of the two collect codes arrived also says which half of
            // the Model 33 is being read, so it sets the echo the same way the
            // arming DOTs do.
            FN_COLLECT | FN_COLLECT_KEYBOARD => {
                self.armed = true;
                self.echo = function == FN_COLLECT_KEYBOARD;
                self.frame.take().unwrap_or(0) as u16
            }
            _ => 0,
        }
    }

    /// Advance the device by `elapsed` clock cycles: finish a character the
    /// printer has now had time for, capture at most one keystroke, and report
    /// the interrupt level they should pulse as a bitmask. Nothing is captured
    /// until the program has armed the device, and nothing is captured while a
    /// frame is still uncollected.
    ///
    /// Both directions run at ten characters a second, and each keeps its own
    /// clock. Sharing one would say that printing a character delays the next
    /// keystroke, and neither the manual nor the driver listing says anything
    /// of the sort -- an ASR-33's send and receive halves are separate
    /// distributors on a full duplex line, and the program is what serializes
    /// them by never reading and writing at once.
    ///
    /// Both halves are taken in the same call rather than returning on the
    /// first. The teletype has one interrupt line, so a completion and a
    /// keystroke in the same instruction do merge into a single interrupt --
    /// but that merge belongs in the CPU's per-level latch, where the hardware
    /// does it. Dropping the *capture* would lose a keystroke, which the
    /// hardware does not do.
    pub fn poll(&mut self, elapsed: u32) -> u16 {
        let mut lines = 0;
        if self.tx_owed > 0 {
            self.tx_remaining = self.tx_remaining.saturating_sub(elapsed);
            if self.tx_remaining == 0 {
                self.tx_owed -= 1;
                // Whatever is still queued starts its own character time now.
                self.tx_remaining = if self.tx_owed > 0 { self.char_cycles } else { 0 };
                lines |= 1 << self.level;
            }
        }
        // Credit accrues whether or not anyone is listening -- the operator
        // was typing at a machine that had not asked yet -- but never past one
        // character, so a burst cannot be delivered as a burst.
        self.rx_credit = self.rx_credit.saturating_add(elapsed).min(self.char_cycles);
        if self.armed && self.frame.is_none() && self.rx_credit >= self.char_cycles {
            if let Some(c) = self.console.try_next_char() {
                // The credit is spent only when a character actually arrives,
                // so a device that is armed with nobody typing keeps its
                // standing credit and the next keystroke is not made to wait.
                self.rx_credit = 0;
                // Carriage return and line feed reach the guest as themselves.
                // A Model 33 has a key for each and the software tells them
                // apart: X-RAY's driver opens every record on a line feed
                // (8A) and closes it on a carriage return (8D), so folding one
                // into the other -- which this did, to be kind to a script
                // piped in with newline endings -- makes the executive
                // unreachable. A program that wants to be kind can do the
                // folding itself; the demo does.
                // Function B's echo is the Model 33 printing what its own
                // keyboard sent, not the program writing; it raises no
                // completion, and owing one here would hand the program an
                // interrupt it never asked for. It costs no printer time here
                // either -- the keyboard's own rate already spaces it, and
                // charging it again would let a typist block a program's
                // output, which on a full duplex machine they cannot.
                if self.echo {
                    self.console.put_char(c);
                    if c == 0x0d {
                        self.console.put_char(0x0a);
                    }
                }
                self.frame = Some(c | 0x80);
                lines |= 1 << self.level;
            }
        }
        lines
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

    /// Unlike the teletype's, this DIN does not start anything. `running` is a
    /// motor -- "the RUN/LOAD switch is part of the tape guide mechanism" --
    /// and a read of the frame register does not thread tape.
    pub fn din(&mut self, function: u8) -> u16 {
        match function {
            FN_COLLECT => self.frame.take().unwrap_or(0) as u16,
            _ => 0,
        }
    }

    /// Advance the tape by one frame and report the level to pulse. Running
    /// off the end simply stops interrupting, which is what an operator sees
    /// when the tape runs out of the reader.
    ///
    /// Deliberately *not* paced to the reader's 300 frames a second, though
    /// `elapsed` is here for the day it should be. Two reasons. Nothing
    /// watches a tape go by -- the reader has no output, so its rate decides
    /// only how long a load takes, where the teletype's rate is the whole
    /// character-by-character texture of using the machine. And pacing it
    /// would cost 1,905 cycles a frame -- over three minutes a frame under the
    /// `ray703-panel-ptb --throttle 10` slow-motion demo, which would replace
    /// a tape load one can watch with one that never visibly finishes.
    pub fn poll(&mut self, _elapsed: u32) -> u16 {
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
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0);
        // the keystroke is still queued, not swallowed
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
    }

    #[test]
    fn received_characters_carry_the_high_bit() {
        let mut tty = tty_with_input(b"Az");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'z' as u16);
    }

    /// Carriage return and line feed are different keys on a Model 33 and
    /// different characters to the software that reads it -- X-RAY's record
    /// format opens on one and closes on the other -- so neither is folded
    /// into the other on the way in.
    #[test]
    fn carriage_return_and_line_feed_stay_distinct() {
        let mut tty = tty_with_input(b"\r\n");
        tty.dot(FN_READ, 0);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(tty.din(FN_COLLECT), 0x8d);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(tty.din(FN_COLLECT), 0x8a);
    }

    #[test]
    fn a_frame_is_held_until_it_is_collected() {
        let mut tty = tty_with_input(b"AB");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "the second character waits for the first DIN");
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'B' as u16);
    }

    #[test]
    fn only_the_collect_function_returns_the_frame() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ, 0);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(tty.din(0x0), 0);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
    }

    /// A `DOT dev,0` is how a 703 driver puts a device down between records;
    /// any other unrecognised function does the same.
    #[test]
    fn a_non_read_function_disconnects_the_input() {
        for stop in [FN_DISCONNECT, 0x3] {
            let mut tty = tty_with_input(b"AB");
            tty.dot(FN_READ, 0);
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
            tty.din(FN_COLLECT);
            tty.dot(stop, 0);
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "function {stop:#x} should have disconnected");
        }
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
    }

    /// The completion interrupt is the whole output protocol: a driver writes
    /// one character and does nothing more until the printer says it is done.
    #[test]
    fn a_write_raises_one_completion_interrupt() {
        let (mut tty, _out) = tty_capturing(b"");
        tty.dot(FN_WRITE, 0xc1);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "the printer finished the character");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "and says so exactly once");
    }

    #[test]
    fn every_write_raises_its_own_completion() {
        let (mut tty, _out) = tty_capturing(b"");
        for _ in 0..3 {
            tty.dot(FN_WRITE, 0xc1);
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0);
        }
    }

    /// The completion is a tenth of a second away, not the next instruction:
    /// the printer is a Model 33 at ten characters a second, and the whole
    /// point of pacing it is that the driver's wait loop actually waits.
    #[test]
    fn a_character_takes_a_full_character_time_to_print() {
        let (mut tty, _out) = tty_capturing(b"");
        tty.dot(FN_WRITE, 0xc1);
        // a plausible instruction's worth of cycles, many times over
        for _ in 0..1000 {
            assert_eq!(tty.poll(7), 0, "the printer is still printing");
        }
        assert_eq!(tty.poll(TTY_CHAR_CYCLES - 7000), 1, "and finishes on the cycle it is due");
    }

    /// A driver that writes ahead of the printer gets a completion for every
    /// character, a character time apart -- the counter in `tx_owed` rather
    /// than a flag, which would have dropped one and hung the second wait.
    #[test]
    fn writes_that_overrun_the_printer_each_get_their_completion() {
        let (mut tty, out) = tty_capturing(b"");
        tty.dot(FN_WRITE, 0xc1);
        tty.dot(FN_WRITE, 0xc2);
        assert_eq!(*out.0.lock().unwrap(), b"AB", "both characters went out");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "the first character finished");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES - 1), 0, "the second is still a character behind");
        assert_eq!(tty.poll(1), 1, "and then it too finished");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "with nothing else owed");
    }

    /// The keyboard sends at the same ten characters a second, so a line
    /// arriving all at once from a pipe is handed to the guest a character at
    /// a time, the way a typist -- or the 33's own tape reader -- delivered it.
    #[test]
    fn keystrokes_arrive_no_faster_than_the_keyboard_sends() {
        let mut tty = tty_with_input(b"AB");
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(0), 1, "the first keystroke is not made to wait");
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES - 1), 0, "the second is still on its way");
        assert_eq!(tty.poll(1), 1);
        assert_eq!(tty.din(FN_COLLECT), 0xc2);
    }

    /// The receive credit saturates at one character. Without that ceiling a
    /// program that sat in its wait loop for a second would bank a second's
    /// worth of cycles and then take ten queued keystrokes back to back at no
    /// rate at all -- which is exactly the case that matters, a line typed at
    /// a prompt while the guest was busy. The failure would be silent: the
    /// feature would look like it was working right up to the moment it was
    /// needed.
    #[test]
    fn a_long_idle_does_not_bank_a_burst_of_keystrokes() {
        let mut tty = tty_with_input(b"AB");
        // ten characters' worth of machine time with the device unarmed
        for _ in 0..10 {
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0);
        }
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(0), 1, "one character of standing credit, and no more");
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
        assert_eq!(tty.poll(0), 0, "the rest of the line still arrives at ten a second");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT), 0xc2);
    }

    /// `--fast-io` returns the device to its pre-pacing model: everything
    /// completes on the very next poll with no machine time charged, while
    /// the protocol -- one completion per write, frames held until
    /// collected -- stays exactly as it is paced.
    #[test]
    fn fast_io_charges_no_machine_time() {
        let (mut tty, _out) = tty_capturing(b"");
        tty.set_fast_io();
        tty.dot(FN_WRITE, 0xc1);
        assert_eq!(tty.poll(0), 1, "the completion needs no character time");
        assert_eq!(tty.poll(0), 0, "but still arrives exactly once");

        let mut tty = tty_with_input(b"AB");
        tty.set_fast_io();
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(0), 1);
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
        assert_eq!(tty.poll(0), 1, "the second keystroke does not wait its turn");
        assert_eq!(tty.din(FN_COLLECT), 0xc2);
    }

    /// Waiting on the guest does not cost the operator anything: a device that
    /// is armed with nobody typing keeps its credit, so the first keystroke
    /// after a wait is instant rather than a tenth of a second late.
    #[test]
    fn an_armed_but_silent_keyboard_keeps_its_credit() {
        let (tx, rx) = mpsc::channel();
        let mut tty = Tty703::new(ConsoleEndpoint::new(rx, Box::new(Vec::new())), 0);
        tty.dot(FN_READ, 0);
        for _ in 0..5 {
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "nobody is typing");
        }
        tx.send(b'A').unwrap();
        assert_eq!(tty.poll(0), 1, "and the keystroke is taken the moment it arrives");
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
    }

    /// Writing must not arm the input side -- a completion interrupt is not an
    /// invitation to read the keyboard.
    #[test]
    fn a_write_leaves_the_keyboard_disconnected() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_WRITE, 0xc1);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "the completion, and nothing else");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0);
        tty.dot(FN_READ, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "the keystroke was queued, not swallowed");
        assert_eq!(tty.din(FN_COLLECT), 0xc1);
    }

    /// One line, two causes. The device reports one level either way; merging
    /// them is the CPU's per-level latch's job, and losing the keystroke on
    /// the way is nobody's.
    #[test]
    fn a_completion_and_a_keystroke_share_the_line() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ, 0);
        tty.dot(FN_WRITE, 0xc2);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "one level, however many causes");
        assert_eq!(tty.din(FN_COLLECT), 0xc1, "the keystroke was still captured");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "and the completion is not raised twice");
    }

    /// Function B collects with F, not D -- see the FN_ constants above.
    #[test]
    fn the_keyboard_read_collects_with_its_own_function() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_COLLECT_KEYBOARD), 0xc1);
    }

    /// X-RAY never issues an arming DOT on its read path -- the collecting DIN
    /// is the whole of "SELECT THE DEVICE" -- so a DIN has to start a read on
    /// a teletype nothing else has spoken to.
    #[test]
    fn a_collecting_din_starts_the_read() {
        let mut tty = tty_with_input(b"A");
        assert_eq!(tty.din(FN_COLLECT_KEYBOARD), 0, "nothing captured yet");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1, "but the device is now listening");
        assert_eq!(tty.din(FN_COLLECT_KEYBOARD), 0xc1);
    }

    /// The collect function says which half of the Model 33 is being read, and
    /// only its keyboard prints what it sends.
    #[test]
    fn the_collect_function_sets_the_echo() {
        let (mut tty, out) = tty_capturing(b"AB");
        tty.din(FN_COLLECT_KEYBOARD);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(*out.0.lock().unwrap(), b"A");
        tty.din(FN_COLLECT);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(*out.0.lock().unwrap(), b"A", "function D does not type");
    }

    /// Function B is the Model 33 keyboard, which prints what you type;
    /// function 9 is its tape reader, whose "characters read are not typed".
    #[test]
    fn only_the_keyboard_function_echoes() {
        let (mut tty, out) = tty_capturing(b"A\r");
        tty.dot(FN_READ_KEYBOARD, 0);
        tty.poll(TTY_CHAR_CYCLES);
        // function B's own collect code, which keeps the echo on
        tty.din(FN_COLLECT_KEYBOARD);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(*out.0.lock().unwrap(), b"A\r\n", "a bare CR needs the LF added");

        let (mut tty, out) = tty_capturing(b"A");
        tty.dot(FN_READ, 0);
        tty.poll(TTY_CHAR_CYCLES);
        assert!(out.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_tape_reader_needs_starting() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x01, 0x02]);
        assert_eq!(reader.poll(0), 0);
        reader.dot(FN_READ, 0);
        assert_eq!(reader.poll(0), 1);
        assert_eq!(reader.din(FN_COLLECT), 0x01);
        assert_eq!(reader.poll(0), 1);
        assert_eq!(reader.din(FN_COLLECT), 0x02);
    }

    #[test]
    fn running_off_the_end_of_the_tape_stops_interrupting() {
        let mut reader = TapeReader703::new(3);
        reader.load_tape(vec![0xff]);
        reader.dot(FN_READER_START, 0);
        assert_eq!(reader.poll(0), 1 << 3, "the level is configurable");
        reader.din(FN_COLLECT);
        assert_eq!(reader.poll(0), 0);
        assert_eq!(reader.din(FN_COLLECT), 0);
    }

    #[test]
    fn an_empty_tape_never_interrupts() {
        let mut reader = TapeReader703::new(0);
        reader.dot(FN_READ, 0);
        assert_eq!(reader.poll(0), 0);
    }
}
