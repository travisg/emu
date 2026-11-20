// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Raytheon 703 DIO devices: the console teletype, the high speed paper tape
//! reader and punch, and the line clock.
//!
//! They hang off the 16-bit Direct Input/Output channel rather than the memory
//! space, so none is a `MemoryDevice`: the machine decodes the DIO address
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
//! Both interrupt once per character, and the program collects the character
//! with a DIN inside the service routine -- which is why the 703 is the first
//! machine in this tree that needs a working interrupt path at all. A program
//! that would rather not be interrupted asks the device's status word
//! instead, `DIN dev,0`, whose bit 7 is the interrupt not yet serviced: the
//! Paper Tape I/O System (DN 393954) runs every device that way with no level
//! enabled.
//!
//! The function codes are the 706 User's Manual's (Tables 5-14 and 5-16),
//! which lists them as bits. The 1968 listings set one more, the 8, in every
//! code they use -- `DOT 13,9` where the table has D1, `DIN 13,13` for D5,
//! `DOT 12,14` for C6 -- which on the teletype is the manual's ASCII bit
//! and on the tape devices, which are binary, selects nothing: those take
//! a code with it or without.
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
//! counts that tenth in clock cycles -- it is handed the cycles the CPU spent,
//! which is the only time base a device on this bus has -- and how many cycles
//! the tenth is worth comes from the rate those cycles are being issued at
//! (`Bus::set_device_pacing_hz`, the resolved `--throttle`). So a throttled
//! machine prints ten characters a second of real time at any throttle, slow
//! motion included, and an unpaced one prints ten per 57,142 emulated cycles
//! however fast the host gets through them. The guest sees ten a second of its
//! own time either way, which is the part a program can observe.
//!
//! The 60 Hz line clock at the bottom is paced the same way and for the same
//! reason, sixty a second of wall clock being what a mains tap gives. It is
//! also the one device in this file with no drawing number behind it -- it is
//! invented, and its own doc comment says so at length.
//!
//! The rule those two share is the whole of it: `--throttle` is a knob on the
//! cpu, and a device keeps the rate its real counterpart ran at however fast
//! the cpu is being run. `--fast-io` is the only thing that takes a device off
//! that rate, and it takes it to no rate at all.

use crate::console::ConsoleEndpoint;
use crate::cpu::ray703::CLOCK_HZ;
use std::io;
use std::path::Path;

/// DIO device codes (392292, "Individual Devices").
pub const DEV_TTY: u8 = 0xe;
pub const DEV_TAPE_READER: u8 = 0xd;
pub const DEV_TAPE_PUNCH: u8 = 0xc;
/// ...and the invented line clock's, which no Raytheon document assigns
/// it. The 706 User's Manual gives 2 to the DMA magnetic tape controller
/// (Table 5-1); this machine has none.
pub const DEV_LINE_CLOCK: u8 = 0x2;
/// The teletypes, the console first. The three after it are invented as the
/// line clock is: the console's controller again, strapped to another
/// device code -- the card has jumpers for all sixteen (704 technical
/// manual, table 5-6) -- and sharing its interrupt level, which is the
/// shape the status word (706 UM Table 5-15) and X-RAY's shared-level loop
/// were made for. It is not how Raytheon sold more teletypes: that was the
/// Teletype Multiplexer, one controller at device A with up to sixteen
/// teletypes behind a scanner (706 UM 5-14), which this machine does not
/// have. Table 5-1 of that manual gives every one of the sixteen codes to
/// some peripheral, so these take three whose owners are not here either:
/// the buffered output and input channels' and the digital to analog
/// converter's.
pub const DEV_TERMINALS: [u8; 4] = [DEV_TTY, 0x7, 0x6, 0x4];

/// Characters a second for the Model 33 on the console. The driver listing
/// (392292, "Individual Devices") says it outright: "the standard Teletype
/// Model 33/35 (Automatic Send-Receive) runs at up to ten characters per
/// second". That is the 110-baud line with eleven bits to the character --
/// start, eight data, two stop -- which comes to exactly ten.
const TTY_CHARS_PER_SEC: u64 = 10;

/// The same rate as a count of clock cycles, which is the only time base a
/// device on this bus has -- see `Bus::poll_interrupt_lines`. The divisor is
/// the rate machine time is being paced at (`Tty703::set_pacing_hz`), so ten
/// characters a second stays ten a second of wall clock however fast the cpu
/// is being run; unpaced it is the machine's own clock, where 571429/10
/// truncates to 57,142 cycles, 99.9985 ms at the 1.75 us cycle -- a tenth of a
/// second to far better than a 1968 teletype's motor held it.
fn tty_char_cycles(pacing_hz: u64) -> u32 {
    (pacing_hz / TTY_CHARS_PER_SEC) as u32
}

/// The unpaced figure, the machine running at its own clock rate.
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
/// `DIN dev,0`, "Return Status to Accumulator" (706 UM Table 5-14).
const FN_STATUS: u8 = 0x0;
/// `DIN dev,8`: the character, and nothing selected. Table 5-14 has no such
/// DIN -- every transfer it lists selects the next -- but Raytheon's
/// teletype test (DN 390443 rev B) takes its key with one, having selected
/// the keyboard with `DOT dev,B`, and selects again for the next. The code
/// is the ASCII bit with no device in its low two bits, which as a DOT is a
/// disconnect; that the character comes with it is the test's word alone.
const FN_TAKE: u8 = 0x8;

/// The status word's two bits (706 UM Table 5-15), bit 0 the most
/// significant as everywhere on this machine.
const STATUS_SELECTED: u16 = 0x8000;
const STATUS_IRQ: u16 = 0x0100;

/// The ASR-33 teletype controller: the console's at DIO device 14, and the
/// invented extra terminals' at the other `DEV_TERMINALS`.
///
/// The 703's software world is eight-bit ASCII with the high bit *set* -- the
/// driver documentation spells out CR as 8D, LF as 8A, blank as A0 and RUBOUT
/// as FF. A modern terminal wants seven, so the bit goes on at the keyboard
/// and comes off at the printer. A tape in the teletype's own reader is
/// another matter: its eight channels come through as punched, which is
/// what lets a binary tape load through the console.
pub struct Tty703 {
    console: ConsoleEndpoint,
    /// The tape in the ASR's reader and how far it has run. While there is
    /// tape left the reader is what sends, at the keyboard's ten frames a
    /// second, under either read function -- "data will also be accepted
    /// from the paper tape reader if it is manually turned ON" (706 UM
    /// 5-5.4.4) -- and the keyboard has the line again when it runs out.
    tape: Vec<u8>,
    tape_pos: usize,
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
    /// Cycles per character, [`tty_char_cycles`] of whatever rate machine time
    /// is being paced at. Zero -- set by `set_fast_io`, behind the `--fast-io`
    /// flag -- collapses every wait above to "on the next poll", which is
    /// exactly the model this device had before it was paced: completions still
    /// arrive one per poll and keystrokes still queue behind an uncollected
    /// frame, only the time is gone.
    char_cycles: u32,
    /// `--fast-io`, held separately from the zero it puts in `char_cycles` so
    /// that a pacing rate arriving afterwards cannot quietly put the teletype
    /// back to ten characters a second.
    fast_io: bool,
    /// The status word's bit 7, "interrupt true and has not been serviced by
    /// the transmission of a character": set by every pulse, cleared by the
    /// next transfer or select. It is how one service routine finds which of
    /// several teletypes on a shared level interrupted -- the level itself
    /// carries no device number, and X-RAY's shared-level loop (`SLOOP`,
    /// listing 390779 cards 764-771) asks each device's status in turn.
    irq: bool,
}

impl crate::snapshot::Snapshot for Tty703 {
    fn save(&self, w: &mut crate::snapshot::Writer) {
        let Tty703 {
            console: _session, tape, tape_pos, level: _wiring, armed, echo, frame, tx_owed, tx_remaining,
            rx_credit, char_cycles: _session_pacing, fast_io: _session_flag, irq,
        } = self;
        w.bytes(tape);
        w.u32(*tape_pos as u32);
        w.bool(*armed);
        w.bool(*echo);
        w.opt(frame, |w, f| w.u8(*f));
        w.u32(*tx_owed);
        w.u32(*tx_remaining);
        w.u32(*rx_credit);
        w.bool(*irq);
    }

    fn load(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        self.tape = r.bytes()?.to_vec();
        self.tape_pos = r.u32()? as usize;
        self.armed = r.bool()?;
        self.echo = r.bool()?;
        self.frame = r.opt(|r| r.u8())?;
        self.tx_owed = r.u32()?;
        self.tx_remaining = r.u32()?;
        self.rx_credit = r.u32()?;
        self.irq = r.bool()?;
        Ok(())
    }
}

impl Tty703 {
    pub fn new(console: ConsoleEndpoint, level: u8) -> Self {
        Tty703 {
            console,
            tape: Vec::new(),
            tape_pos: 0,
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
            fast_io: false,
            irq: false,
        }
    }

    /// Plug a line into the teletype: the host end of a terminal that was
    /// built unplugged.
    pub fn set_console(&mut self, console: ConsoleEndpoint) {
        self.console = console;
    }

    /// Put a tape in the ASR's reader and set its lever to START.
    pub fn load_tape(&mut self, frames: Vec<u8>) {
        self.tape = frames;
        self.tape_pos = 0;
    }

    /// Run at host speed instead of ten characters a second (`--fast-io`).
    pub fn set_fast_io(&mut self) {
        self.fast_io = true;
        self.char_cycles = 0;
    }

    /// Pace the character time against `pacing_hz` cycles to the second, so
    /// that ten characters a second is ten of wall clock rather than ten per
    /// 571,429 emulated cycles. `--fast-io` outranks it: a device asked to run
    /// at host speed stays there whatever the cpu is paced to.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        if self.fast_io {
            return;
        }
        self.char_cycles = tty_char_cycles(pacing_hz);
        // The standing credit is one character time, and this is called before
        // the machine runs, so the first keystroke is still free.
        self.rx_credit = self.char_cycles;
    }

    pub fn dot(&mut self, function: u8, val: u16) {
        // every DOT is a transfer or a select, which is what services the
        // interrupt as far as the status word is concerned
        self.irq = false;
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
                self.irq = false;
                self.armed = true;
                self.echo = function == FN_COLLECT_KEYBOARD;
                self.frame.take().unwrap_or(0) as u16
            }
            FN_TAKE => {
                self.irq = false;
                self.armed = false;
                self.frame.take().unwrap_or(0) as u16
            }
            // Selected is a read armed or a character printing. Asking
            // leaves the interrupt bit as it was: the status DIN is how a
            // shared service routine looks before it transfers.
            FN_STATUS => {
                let selected = if self.armed || self.tx_owed > 0 { STATUS_SELECTED } else { 0 };
                selected | if self.irq { STATUS_IRQ } else { 0 }
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
            // The reader's frame as punched, or the keyboard's character
            // with the eighth bit the Model 33 sends marking.
            let next = match self.tape.get(self.tape_pos) {
                Some(&frame) => {
                    self.tape_pos += 1;
                    Some(frame)
                }
                None => self.console.try_next_char().map(|c| c | 0x80),
            };
            if let Some(frame) = next {
                // The credit is spent only when a character actually arrives,
                // so a device that is armed with nobody typing keeps its
                // standing credit and the next keystroke is not made to wait.
                self.rx_credit = 0;
                // Carriage return and line feed reach the guest as themselves.
                // A Model 33 has a key for each and the software tells them
                // apart: X-RAY's driver opens every record on a line feed
                // (8A) and closes it on a carriage return (8D), so a device
                // that folded one into the other would make the executive
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
                    let c = frame & 0x7f;
                    self.console.put_char(c);
                    if c == 0x0d {
                        self.console.put_char(0x0a);
                    }
                }
                self.frame = Some(frame);
                lines |= 1 << self.level;
            }
        }
        if lines != 0 {
            self.irq = true;
        }
        lines
    }
}

/// The paper tape devices' functions (706 UM Table 5-16), less the 8 the
/// 1968 listings carry in every code.
const FN_TAPE_MASK: u8 = 0x7;
/// The reader's two function bits. `DOT 13,1` is read forward; `DIN 13,4`
/// transfers the character and disconnects, and `DIN 13,5` -- PTB's `DIN
/// 13,13` -- transfers it and reads forward. They are taken as bits because
/// the I/O software makes a device's function out of the caller's: "codes 9
/// and B are equivalent for every device except the teletype" (706 UM
/// 5-4.3), and a read with B reaches this reader as `DIN 13,15`, which is
/// how Conversational FORTRAN's library reads a unit assigned to it.
const FN_READ_FORWARD: u8 = 0x1;
const FN_TRANSFER: u8 = 0x4;
/// `DOT 12,2` and `DOT 12,3`, the punch's motor on and off...
const FN_PUNCH_ON: u8 = 0x2;
const FN_PUNCH_OFF: u8 = 0x3;
/// ...and `DOT 12,6`, punch the character, turning the motor on if it is
/// off: the 1968 driver's `DOT 12,14`.
const FN_PUNCH: u8 = 0x6;

/// The tape devices' status word (706 UM Table 5-17). Bits 0 and 1 are the
/// opposite sense from the teletype's bit 0: true is *not* ready.
const TAPE_STATUS_OFF_LINE: u16 = 0x8000;
const TAPE_STATUS_NOT_READY: u16 = 0x4000;

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
    /// Reading forward: set by the read forward function, as a DOT or with
    /// a transfer, and cleared by a disconnect. PTB issues its DOT once and
    /// keeps the tape moving with the DIN that takes each frame.
    running: bool,
    frame: Option<u8>,
    /// The status word's bit 7, as the teletype's: set by the pulse, cleared
    /// by the transfer that answers it or by a DOT.
    irq: bool,
}

/// The tape is in the snapshot, as a disk is: it is what the reader holds.
impl crate::snapshot::Snapshot for TapeReader703 {
    fn save(&self, w: &mut crate::snapshot::Writer) {
        let TapeReader703 { level: _wiring, frames, pos, running, frame, irq } = self;
        w.bytes(frames);
        w.u32(*pos as u32);
        w.bool(*running);
        w.opt(frame, |w, f| w.u8(*f));
        w.bool(*irq);
    }

    fn load(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        self.frames = r.bytes()?.to_vec();
        self.pos = r.u32()? as usize;
        self.running = r.bool()?;
        self.frame = r.opt(|r| r.u8())?;
        self.irq = r.bool()?;
        Ok(())
    }
}

impl TapeReader703 {
    pub fn new(level: u8) -> Self {
        TapeReader703 { level, frames: Vec::new(), pos: 0, running: false, frame: None, irq: false }
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

    /// Thread a tape through the reader, its first frame under the lamp.
    /// The frame of the tape it replaces goes with that tape, and its
    /// interrupt with it; what the program had the reader doing stays as it
    /// was, the operator's hands being no DOT.
    pub fn load_tape(&mut self, frames: Vec<u8>) {
        self.frames = frames;
        self.pos = 0;
        self.frame = None;
        self.irq = false;
    }

    /// Read forward or disconnect. Function 8 starts the reader as 9 does:
    /// the 1968 driver's page for this device reads "Read a Record --
    /// Function codes 8 or 9" (392292), the one code here that Table 5-16's
    /// bits do not account for. Table 5-1 has a `DOT 13,3`, read reverse,
    /// that Table 5-16 and the "unidirectional" reader of 5-6 do not; here
    /// it stops the tape like any other code that is not read forward.
    pub fn dot(&mut self, function: u8, _val: u16) {
        self.irq = false;
        self.running = function & FN_TAPE_MASK == FN_READ_FORWARD || function == FN_READER_START;
    }

    /// The status word, or the frame. A transfer says in the same code
    /// whether the tape is to go on, and going on is all it takes to start:
    /// "continuous data flow is accomplished by executing input instructions
    /// (DIN) which simultaneously specify data transfer and read" (706 UM
    /// 5-6.6), and PTIOS reads a tape with that DIN and no DOT.
    pub fn din(&mut self, function: u8) -> u16 {
        if function == FN_STATUS {
            // Off line is both bits (5-6.4.1); on line, bit 1 is no
            // character in the buffer to transfer.
            let ready = if !self.running {
                TAPE_STATUS_OFF_LINE | TAPE_STATUS_NOT_READY
            } else if self.frame.is_none() {
                TAPE_STATUS_NOT_READY
            } else {
                0
            };
            return ready | if self.irq { STATUS_IRQ } else { 0 };
        }
        if function & FN_TRANSFER == 0 {
            return 0;
        }
        self.running = function & FN_READ_FORWARD != 0;
        self.irq = false;
        self.frame.take().unwrap_or(0) as u16
    }

    /// Advance the tape by one frame and report the level to pulse. Running
    /// off the end simply stops interrupting, which is what an operator sees
    /// when the tape runs out of the reader.
    ///
    /// The one device here that free-runs instead of keeping its real rate,
    /// though `elapsed` is here for the day it should take its 300 frames a
    /// second. Nothing watches a tape go by: the reader has no output, so its
    /// rate decides only how long a load takes, where the teletype's rate is
    /// the whole character-by-character texture of using the machine.
    pub fn poll(&mut self, _elapsed: u32) -> u16 {
        if !self.running || self.frame.is_some() || self.pos >= self.frames.len() {
            return 0;
        }
        self.frame = Some(self.frames[self.pos]);
        self.pos += 1;
        self.irq = true;
        1 << self.level
    }
}

/// Frames a second through the punch (706 UM 5-6.1), and the second its
/// motor takes to come up to speed (5-6.5.2).
const PUNCH_FRAMES_PER_SEC: u64 = 110;

/// High speed paper tape punch, DIO device 12.
///
/// "Each character is eight data bits plus a sprocket hole", and "the
/// controller contains a one-character buffer register" (392292): a `DOT
/// 12,6` hands it a frame, and the interrupt that follows says the frame is
/// punched and the buffer free for the next. The motor is the program's to
/// run -- on with the first frame or by itself, and off only by its own
/// function, a disconnect leaving it turning -- and takes a second to come
/// up to speed, which a frame sent with the motor off waits out.
///
/// What is punched collects here until the operator tears it off
/// (`take_tape`): the tape is the machine's, like a platter, and reaches a
/// file only when someone asks for one.
pub struct TapePunch703 {
    level: u8,
    /// What has been punched since the tape was last torn off.
    tape: Vec<u8>,
    /// On line: set by the motor-on and punch functions, cleared by a
    /// disconnect, which also keeps a frame still being punched from
    /// interrupting ("its interrupt capability is inhibited", 5-6.5.1).
    on_line: bool,
    motor: bool,
    /// Cycles until the motor is up to speed.
    spin_up: u32,
    /// A frame is in the buffer, and the cycles until it is punched once
    /// the motor is up.
    busy: bool,
    remaining: u32,
    irq: bool,
    /// Cycles to a frame and to the motor's second, at the pacing rate;
    /// both zero under `--fast-io`.
    frame_cycles: u32,
    spin_up_cycles: u32,
    fast_io: bool,
}

impl crate::snapshot::Snapshot for TapePunch703 {
    fn save(&self, w: &mut crate::snapshot::Writer) {
        let TapePunch703 {
            level: _wiring, tape, on_line, motor, spin_up, busy, remaining, irq,
            frame_cycles: _session_pacing, spin_up_cycles: _session_spin_up, fast_io: _session_flag,
        } = self;
        w.bytes(tape);
        w.bool(*on_line);
        w.bool(*motor);
        w.u32(*spin_up);
        w.bool(*busy);
        w.u32(*remaining);
        w.bool(*irq);
    }

    fn load(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        self.tape = r.bytes()?.to_vec();
        self.on_line = r.bool()?;
        self.motor = r.bool()?;
        self.spin_up = r.u32()?;
        self.busy = r.bool()?;
        self.remaining = r.u32()?;
        self.irq = r.bool()?;
        Ok(())
    }
}

impl TapePunch703 {
    pub fn new(level: u8) -> Self {
        let mut punch = TapePunch703 {
            level,
            tape: Vec::new(),
            on_line: false,
            motor: false,
            spin_up: 0,
            busy: false,
            remaining: 0,
            irq: false,
            frame_cycles: 0,
            spin_up_cycles: 0,
            fast_io: false,
        };
        punch.set_pacing_hz(CLOCK_HZ);
        punch
    }

    /// Punch at host speed (`--fast-io`).
    pub fn set_fast_io(&mut self) {
        self.fast_io = true;
        self.frame_cycles = 0;
        self.spin_up_cycles = 0;
    }

    /// Keep 110 frames a second, and the motor's second, of wall clock at
    /// whatever rate machine time is paced; `--fast-io` outranks it.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        if self.fast_io {
            return;
        }
        self.frame_cycles = (pacing_hz / PUNCH_FRAMES_PER_SEC) as u32;
        self.spin_up_cycles = pacing_hz as u32;
    }

    /// The tape punched so far.
    pub fn tape(&self) -> &[u8] {
        &self.tape
    }

    /// Tear the tape off: what was punched, the punch left with none.
    pub fn take_tape(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.tape)
    }

    fn start_motor(&mut self) {
        if !self.motor {
            self.motor = true;
            self.spin_up = self.spin_up_cycles;
        }
    }

    pub fn dot(&mut self, function: u8, val: u16) {
        self.irq = false;
        match function & FN_TAPE_MASK {
            FN_PUNCH => {
                self.on_line = true;
                self.start_motor();
                // The frame is on the tape now and reported punched a frame
                // time later, as the teletype prints. One buffer: a frame
                // sent over one still being punched takes its place in the
                // timing and both are on the tape, a thing no driver does.
                self.tape.push(val as u8);
                self.busy = true;
                self.remaining = self.frame_cycles;
            }
            FN_PUNCH_ON => {
                self.on_line = true;
                self.start_motor();
            }
            FN_PUNCH_OFF => self.motor = false,
            _ => self.on_line = false,
        }
    }

    /// The status word; the punch has nothing else to read. Bit 4, tape
    /// supply low, is never true: the reel here does not run out.
    pub fn din(&mut self, function: u8) -> u16 {
        if function != FN_STATUS {
            return 0;
        }
        (if self.on_line { 0 } else { TAPE_STATUS_OFF_LINE })
            | (if self.motor && self.spin_up == 0 { 0 } else { TAPE_STATUS_NOT_READY })
            | (if self.irq { STATUS_IRQ } else { 0 })
    }

    /// Spend `elapsed` cycles bringing the motor up and then punching the
    /// frame in the buffer, and report the level to pulse when it is done.
    pub fn poll(&mut self, elapsed: u32) -> u16 {
        let left = elapsed.saturating_sub(self.spin_up);
        self.spin_up = self.spin_up.saturating_sub(elapsed);
        if !self.busy || !self.motor || self.spin_up > 0 {
            return 0;
        }
        self.remaining = self.remaining.saturating_sub(left);
        if self.remaining > 0 {
            return 0;
        }
        self.busy = false;
        if !self.on_line {
            return 0;
        }
        self.irq = true;
        1 << self.level
    }
}

/// 60 Hz line-frequency clock, DIO device 2.
///
/// INVENTED HARDWARE. Unlike everything else in this file -- and everything
/// else on this machine's DIO channel -- no Raytheon document stands behind
/// this device: no drawing number, no manual section, no driver listing. The
/// tree needed a periodic interrupt source for a preemptive executive
/// (`rex/rex.asm`), and a line clock is the period-plausible shape for
/// one: a transformer tap and a zero-crossing detector pulsing an interrupt
/// level at the mains rate, the instrument the PDP-8 knew as the KW8.
///
/// The programming model is one bit. `DOT 2,1` connects it, and every zero
/// crossing from then on pulses the level; function 0 -- the tree-wide
/// disconnect code -- or any other function disconnects, and disconnecting
/// forgets the phase, so a re-arm always waits one full period. It has no
/// readable register at all: a DIN to device 2 falls through the machine
/// decode's open-bus default and reads zero, exactly as when the device was
/// not installed.
///
/// Sixty a second is sixty a second of wall clock, the mains being the mains,
/// so the period follows the rate machine time is being paced at
/// (`set_pacing_hz`, the resolved `--throttle`) exactly as the teletype's
/// character time does. `--throttle` is a knob on the cpu; the hardware around
/// it keeps its own rate.
///
/// `--fast-io` is the one thing that takes a device off that rate, and it is
/// the one flag this device sits out: a completion is one-shot and something
/// waits for it, so making one instant only makes a machine more responsive,
/// where a periodic interrupt with no period at all never returns. So
/// `Ray703::set_fast_io` does not reach the clock, and a test can run
/// `--fast-io` for an instant teletype over unchanged scheduling slices.
pub struct LineClock703 {
    level: u8,
    armed: bool,
    /// Cycles banked toward the next tick. The post-pulse remainder carries
    /// forward -- unlike the teletype, which starts a fresh character time
    /// per character -- because a clock's whole job is the long-run rate,
    /// and resetting would lose up to an instruction's worth of cycles on
    /// every tick.
    acc: u32,
    /// Cycles to the tick, [`line_clock_period_cycles`] of whatever rate
    /// machine time is being paced at.
    period_cycles: u32,
}

/// The mains rate, which is also 1800 RPM -- the figure the disc uses for half
/// a revolution.
const LINE_HZ: u64 = 60;

/// The mains rate as a count of clock cycles, which is the only time base a
/// device on this bus has -- see `Bus::poll_interrupt_lines`. The divisor is
/// the rate machine time is being paced at (`LineClock703::set_pacing_hz`).
///
/// The floor of one cycle guards the modulus in `poll`, which a pacing rate
/// under 60 Hz would otherwise divide by zero. It guards nothing else: a cpu
/// held that far below its own rate cannot answer sixty ticks a second, and
/// choosing such a throttle is choosing that.
fn line_clock_period_cycles(pacing_hz: u64) -> u32 {
    ((pacing_hz / LINE_HZ) as u32).max(1)
}

/// The unpaced figure, the machine running at its own clock rate: 571,429 / 60
/// truncates to 9,523 cycles, 16.665 ms. The 0.03% truncation error is far
/// inside any mains tolerance.
const LINE_CLOCK_PERIOD_CYCLES: u32 = (CLOCK_HZ / LINE_HZ) as u32;

const FN_LINE_CLOCK_CONNECT: u8 = 0x1;

impl crate::snapshot::Snapshot for LineClock703 {
    fn save(&self, w: &mut crate::snapshot::Writer) {
        let LineClock703 { level: _wiring, armed, acc, period_cycles: _session_pacing } = self;
        w.bool(*armed);
        w.u32(*acc);
    }

    fn load(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        self.armed = r.bool()?;
        self.acc = r.u32()?;
        Ok(())
    }
}

impl LineClock703 {
    pub fn new(level: u8) -> Self {
        LineClock703 { level, armed: false, acc: 0, period_cycles: LINE_CLOCK_PERIOD_CYCLES }
    }

    /// Pace the tick against `pacing_hz` cycles to the second, so that sixty a
    /// second is sixty of wall clock rather than sixty per 571,429 emulated
    /// cycles. Unlike the teletype there is no `--fast-io` to outrank it: the
    /// clock is never asked to run at host speed.
    ///
    /// The banked cycles are left alone -- the modulus in `poll` absorbs an
    /// overhang from the old period, so a rate arriving mid-run costs at most
    /// one early tick, and the rate arrives before the machine runs anyway.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        self.period_cycles = line_clock_period_cycles(pacing_hz);
    }

    pub fn dot(&mut self, function: u8, _val: u16) {
        // Connecting -- or reconnecting -- starts a fresh full period, and
        // anything that is not the connect function disconnects, the
        // teletype's rule.
        self.armed = function == FN_LINE_CLOCK_CONNECT;
        self.acc = 0;
    }

    /// Bank the elapsed cycles and pulse the level once per period. A
    /// disconnected clock banks nothing -- contrast the teletype's standing
    /// keystroke credit, which models an operator the machine kept waiting;
    /// nothing waits behind a clock. Multiple owed periods merge into one
    /// pulse, which the CPU's per-level latch would do anyway.
    pub fn poll(&mut self, elapsed: u32) -> u16 {
        if !self.armed {
            return 0;
        }
        self.acc = self.acc.saturating_add(elapsed);
        if self.acc >= self.period_cycles {
            self.acc %= self.period_cycles;
            return 1 << self.level;
        }
        0
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
    fn a_din_that_is_no_transfer_leaves_the_frame() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ, 0);
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(tty.din(FN_STATUS), STATUS_SELECTED | STATUS_IRQ, "status, not the frame");
        assert_eq!(tty.din(0x3), 0);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'A' as u16);
    }

    /// `DOT dev,B`, a key, `DIN dev,8`: the character once, and no second
    /// one until the keyboard is selected again.
    #[test]
    fn the_take_function_returns_the_frame_and_selects_nothing() {
        let mut tty = tty_with_input(b"AZ");
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_TAKE), 0x80 | b'A' as u16);
        assert_eq!(tty.din(FN_STATUS), 0, "serviced, and nothing selected");
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 0, "the next key waits for a select");
        assert_eq!(tty.din(FN_TAKE), 0);
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_TAKE), 0x80 | b'Z' as u16);
    }

    #[test]
    fn status_says_selected_while_reading_or_printing() {
        let mut tty = tty_with_input(b"");
        assert_eq!(tty.din(FN_STATUS), 0, "nothing selected at power-on");
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.din(FN_STATUS), STATUS_SELECTED);
        tty.dot(FN_DISCONNECT, 0);
        assert_eq!(tty.din(FN_STATUS), 0);
        tty.dot(FN_WRITE, b'A' as u16);
        assert_eq!(tty.din(FN_STATUS), STATUS_SELECTED, "printing");
        tty.poll(TTY_CHAR_CYCLES);
        assert_eq!(tty.din(FN_STATUS), STATUS_IRQ, "printed, and the completion not yet serviced");
    }

    /// Bit 7 stays up until a transfer or a select services it, and asking
    /// for status is neither -- so a routine on a shared level can ask each
    /// device and then serve the ones that want it.
    #[test]
    fn the_interrupt_bit_holds_until_a_transfer_or_select() {
        // a keystroke, serviced by collecting it
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ_KEYBOARD, 0);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_STATUS) & STATUS_IRQ, STATUS_IRQ);
        assert_eq!(tty.din(FN_STATUS) & STATUS_IRQ, STATUS_IRQ, "asking twice changes nothing");
        assert_eq!(tty.din(FN_COLLECT_KEYBOARD), 0xc1);
        assert_eq!(tty.din(FN_STATUS) & STATUS_IRQ, 0);
        // a completion, serviced by the next character, a select or a disconnect
        for service in [FN_WRITE, FN_READ_KEYBOARD, FN_READ, FN_DISCONNECT] {
            let mut tty = tty_with_input(b"");
            tty.dot(FN_WRITE, b'A' as u16);
            assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
            assert_eq!(tty.din(FN_STATUS) & STATUS_IRQ, STATUS_IRQ);
            tty.dot(service, b'B' as u16);
            assert_eq!(tty.din(FN_STATUS) & STATUS_IRQ, 0, "function {service:#x} services it");
        }
    }

    /// A completion and a keystroke in one poll are one pulse and one bit;
    /// collecting the keystroke services both, as one interrupt line would.
    #[test]
    fn a_merged_completion_and_keystroke_are_one_interrupt() {
        let mut tty = tty_with_input(b"A");
        tty.dot(FN_READ_KEYBOARD, 0);
        tty.dot(FN_WRITE, b'X' as u16);
        assert_eq!(tty.poll(TTY_CHAR_CYCLES), 1);
        assert_eq!(tty.din(FN_STATUS), STATUS_SELECTED | STATUS_IRQ);
        assert_eq!(tty.din(FN_COLLECT_KEYBOARD), 0xc1);
        assert_eq!(tty.din(FN_STATUS), STATUS_SELECTED, "still reading; nothing owed");
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

    /// The whole of the pacing feature in one invariant: characters a *wall*
    /// second is `pacing_hz / char_cycles`, which is ten at every rate the cpu
    /// can be held to. Slow motion is what needs it -- a teletype whose
    /// character time stayed 57,142 cycles would take a minute a character at
    /// `--throttle 10000`, and a full ninety-five at `--throttle 10`.
    #[test]
    fn a_paced_teletype_prints_ten_a_second_at_any_rate() {
        // the machine's own clock, a hundredth of it, and the slow-motion demo
        for hz in [CLOCK_HZ, CLOCK_HZ / 100, 10_000, 10] {
            let (mut tty, _out) = tty_capturing(b"");
            tty.set_pacing_hz(hz);
            let expected = (hz / TTY_CHARS_PER_SEC) as u32;
            assert_eq!(tty.char_cycles, expected, "hz={hz}");

            tty.dot(FN_WRITE, 0xc1);
            assert_eq!(tty.poll(expected - 1), 0, "one cycle short of a character, hz={hz}");
            assert_eq!(tty.poll(1), 1, "hz={hz}");
        }
    }

    /// The unpaced device is the machine at its own clock rate, so a run with
    /// no `--throttle` is byte-identical to one from before there was a rate
    /// to pass -- `set_device_pacing_hz` is simply never called.
    #[test]
    fn the_unpaced_character_time_is_the_machines_own_clock() {
        let (mut tty, _out) = tty_capturing(b"");
        assert_eq!(tty.char_cycles, TTY_CHAR_CYCLES);
        tty.set_pacing_hz(CLOCK_HZ);
        assert_eq!(tty.char_cycles, TTY_CHAR_CYCLES, "the same rate changes nothing");
    }

    /// `--fast-io` outranks a pacing rate, whichever order they arrive in.
    /// The factory sets fast-io and `main` sets the rate afterwards, so a
    /// setter that recomputed the character time unconditionally would put an
    /// instant teletype back to ten a second the moment anyone throttled --
    /// and the scripted sessions that pass `--fast-io` would slow to a crawl
    /// under their own hang guards.
    #[test]
    fn fast_io_outranks_a_pacing_rate() {
        let (mut tty, _out) = tty_capturing(b"");
        tty.set_fast_io();
        tty.set_pacing_hz(CLOCK_HZ);
        assert_eq!(tty.char_cycles, 0, "still instant");
        tty.dot(FN_WRITE, 0xc1);
        assert_eq!(tty.poll(0), 1, "the completion needs no character time");
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

    /// A tape in the teletype's reader comes through as punched, all eight
    /// channels, and the keyboard has the line again when it runs out.
    #[test]
    fn the_teletypes_reader_sends_frames_as_punched() {
        let (mut tty, printed) = tty_capturing(b"K");
        tty.set_fast_io();
        tty.load_tape(vec![0x00, 0x7f, 0x8a]);
        tty.dot(FN_READ, 0);
        for frame in [0x00, 0x7f, 0x8a] {
            assert_eq!(tty.poll(0), 1);
            assert_eq!(tty.din(FN_COLLECT), frame);
        }
        tty.poll(0);
        assert_eq!(tty.din(FN_COLLECT), 0x80 | b'K' as u16, "the keyboard, marking");
        assert!(printed.0.lock().unwrap().is_empty(), "function 9 does not print what it reads");
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

    /// Table 5-16's codes, the 1968 listings', and the keyboard read's
    /// that the I/O software sends any device are the same functions.
    #[test]
    fn the_reader_takes_the_tables_codes_and_the_listings() {
        for (forward, transfer) in [(0x1, 0x5), (0x9, 0xd), (0x9, 0xf)] {
            let mut reader = TapeReader703::new(0);
            reader.load_tape(vec![0x11, 0x22]);
            reader.dot(forward, 0);
            assert_eq!(reader.poll(0), 1);
            assert_eq!(reader.din(transfer), 0x11);
            assert_eq!(reader.poll(0), 1, "the transfer read forward");
            assert_eq!(reader.din(transfer), 0x22);
        }
    }

    /// Table 5-1's `DOT 13,3`, read reverse, moves no tape here.
    #[test]
    fn the_reader_does_not_read_in_reverse() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x11]);
        reader.dot(0x3, 0);
        assert_eq!(reader.poll(0), 0);
        assert_eq!(reader.din(FN_STATUS), 0xc000);
    }

    /// Table 5-17: off line is bits 0 and 1, on line with nothing read is
    /// bit 1, and bit 7 is a frame the program has not yet taken.
    #[test]
    fn the_readers_status_says_off_line_not_ready_and_interrupting() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x11, 0x22]);
        assert_eq!(reader.din(FN_STATUS), 0xc000, "never selected");
        reader.dot(FN_READ_FORWARD, 0);
        assert_eq!(reader.din(FN_STATUS), 0x4000, "moving, nothing read yet");
        reader.poll(0);
        assert_eq!(reader.din(FN_STATUS), 0x0100);
        assert_eq!(reader.din(FN_STATUS), 0x0100, "asking twice changes nothing");
        assert_eq!(reader.din(FN_TRANSFER | FN_READ_FORWARD), 0x11);
        assert_eq!(reader.din(FN_STATUS), 0x4000);
        reader.poll(0);
        reader.dot(FN_DISCONNECT, 0);
        assert_eq!(reader.din(FN_STATUS), 0xc000, "5-6.4.1: bits 0 and 1 become true");
    }

    /// `DIN 13,4` takes the frame and stops the tape; `DIN 13,5` with the
    /// reader stopped starts it, the DIN-only read X-RAY's driver makes.
    #[test]
    fn a_transfer_says_whether_the_tape_goes_on() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x11, 0x22, 0x33]);
        reader.dot(FN_READ_FORWARD, 0);
        reader.poll(0);
        assert_eq!(reader.din(FN_TRANSFER), 0x11);
        assert_eq!(reader.poll(0), 0, "stopped");
        assert_eq!(reader.din(FN_STATUS), 0xc000);
        assert_eq!(reader.din(FN_TRANSFER | FN_READ_FORWARD), 0, "nothing in the buffer, and the tape moves");
        assert_eq!(reader.poll(0), 1);
        assert_eq!(reader.din(FN_TRANSFER | FN_READ_FORWARD), 0x22);
    }

    /// A tape changed under a running reader reads from its first frame.
    #[test]
    fn a_new_tape_reads_from_its_start() {
        let mut reader = TapeReader703::new(0);
        reader.load_tape(vec![0x11, 0x22]);
        reader.dot(FN_READ_FORWARD, 0);
        reader.poll(0);
        reader.load_tape(vec![0x77]);
        assert_eq!(reader.din(FN_STATUS), 0x4000, "the old tape's frame went with it, and its interrupt");
        reader.poll(0);
        assert_eq!(reader.din(FN_TRANSFER | FN_READ_FORWARD), 0x77);
    }

    // -- the punch ---------------------------------------------------------

    fn punch_at_host_speed() -> TapePunch703 {
        let mut punch = TapePunch703::new(0);
        punch.set_fast_io();
        punch
    }

    /// A frame is all eight bits of the accumulator's low half, and each
    /// raises one interrupt when it is punched.
    #[test]
    fn the_punch_punches_eight_bits_and_interrupts_per_frame() {
        for function in [FN_PUNCH, 0xe] {
            let mut punch = punch_at_host_speed();
            for frame in [0x00u16, 0x8a, 0x12ff] {
                punch.dot(function, frame);
                assert_eq!(punch.poll(0), 1);
                assert_eq!(punch.poll(0), 0, "one interrupt a frame");
            }
            assert_eq!(punch.tape(), [0x00, 0x8a, 0xff]);
        }
    }

    /// Table 5-17 for the punch: bit 0 off line, bit 1 the motor not up to
    /// speed, bit 7 the interrupt not serviced.
    #[test]
    fn the_punchs_status_follows_the_motor_and_the_select() {
        let mut punch = punch_at_host_speed();
        assert_eq!(punch.din(FN_STATUS), 0xc000, "off line, motor off");
        punch.dot(FN_PUNCH_ON, 0);
        assert_eq!(punch.din(FN_STATUS), 0);
        assert_eq!(punch.poll(0), 0, "5-6.5.2: no interrupt for the motor");
        punch.dot(FN_PUNCH, 0x55);
        punch.poll(0);
        assert_eq!(punch.din(FN_STATUS), 0x0100);
        punch.dot(FN_DISCONNECT, 0);
        assert_eq!(punch.din(FN_STATUS), 0x8000, "5-6.5.1: a disconnect leaves the motor on");
        punch.dot(FN_PUNCH_OFF, 0);
        assert_eq!(punch.din(FN_STATUS), 0xc000);
    }

    /// The motor takes a second to come up, a frame sent with it off waits
    /// that second out, and frames after it are a hundred and ten to the
    /// second.
    #[test]
    fn a_cold_punch_takes_a_second_and_then_110_frames_a_second() {
        let hz = CLOCK_HZ as u32;
        let frame = (CLOCK_HZ / 110) as u32;
        let mut punch = TapePunch703::new(0);
        punch.dot(FN_PUNCH, 0x01);
        assert_eq!(punch.din(FN_STATUS), 0x4000, "not up to speed");
        assert_eq!(punch.poll(hz - 1), 0);
        assert_eq!(punch.poll(1), 0, "up to speed, the frame not yet punched");
        assert_eq!(punch.din(FN_STATUS), 0);
        assert_eq!(punch.poll(frame - 1), 0);
        assert_eq!(punch.poll(1), 1);
        punch.dot(FN_PUNCH, 0x02);
        assert_eq!(punch.poll(frame - 1), 0);
        assert_eq!(punch.poll(1), 1, "no second wait with the motor running");
    }

    /// 5-6.5.1: a disconnected punch does not interrupt, though the frame
    /// it was handed is on the tape.
    #[test]
    fn a_disconnect_keeps_a_frame_from_interrupting() {
        let mut punch = punch_at_host_speed();
        punch.dot(FN_PUNCH, 0x41);
        punch.dot(FN_DISCONNECT, 0);
        assert_eq!(punch.poll(0), 0);
        assert_eq!(punch.din(FN_STATUS) & STATUS_IRQ, 0);
        assert_eq!(punch.take_tape(), [0x41]);
        assert!(punch.tape().is_empty(), "torn off");
    }

    // -- the line clock ----------------------------------------------------

    #[test]
    fn an_unarmed_line_clock_never_pulses() {
        let mut clk = LineClock703::new(2);
        for _ in 0..10 {
            assert_eq!(clk.poll(LINE_CLOCK_PERIOD_CYCLES), 0);
        }
        // disconnecting after a connect leaves it just as silent
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        clk.dot(FN_DISCONNECT, 0);
        assert_eq!(clk.poll(10 * LINE_CLOCK_PERIOD_CYCLES), 0);
    }

    /// The first tick comes one full period after the connect, never sooner:
    /// a disconnected clock banks nothing.
    #[test]
    fn the_first_pulse_comes_exactly_one_period_after_arming() {
        let mut clk = LineClock703::new(5); // the level is configurable
        clk.poll(LINE_CLOCK_PERIOD_CYCLES); // banked nowhere -- still unarmed
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        assert_eq!(clk.poll(LINE_CLOCK_PERIOD_CYCLES - 1), 0);
        assert_eq!(clk.poll(1), 1 << 5);
    }

    /// The accumulator keeps the remainder past each pulse, so the long-run
    /// rate is exact: one hundred periods of cycles fed in chunks that never
    /// divide evenly must yield exactly one hundred pulses. A counter that
    /// restarted from zero at each pulse -- the teletype's per-character
    /// pattern -- would lose most of a chunk every period and drift slow.
    #[test]
    fn the_cadence_carries_the_remainder_and_does_not_drift() {
        let mut clk = LineClock703::new(2);
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        let total = 100 * LINE_CLOCK_PERIOD_CYCLES;
        let mut fed = 0;
        let mut pulses = 0;
        while fed < total {
            let chunk = 13.min(total - fed); // 13 does not divide 9,523
            if clk.poll(chunk) != 0 {
                pulses += 1;
            }
            fed += chunk;
        }
        assert_eq!(pulses, 100);
    }

    #[test]
    fn disconnecting_forgets_the_phase() {
        let mut clk = LineClock703::new(2);
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        clk.poll(LINE_CLOCK_PERIOD_CYCLES - 1);
        clk.dot(FN_DISCONNECT, 0);
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        assert_eq!(clk.poll(LINE_CLOCK_PERIOD_CYCLES - 1), 0, "the banked cycles are gone");
        assert_eq!(clk.poll(1), 1 << 2);

        // any function that is not the connect disconnects
        clk.dot(0x3, 0);
        assert_eq!(clk.poll(10 * LINE_CLOCK_PERIOD_CYCLES), 0);
    }

    /// The whole of the clock's pacing in one invariant, the teletype's test
    /// in the other units: ticks a *wall* second is `pacing_hz /
    /// period_cycles`, which is sixty at every rate the cpu can be held to.
    #[test]
    fn the_tick_is_sixty_a_wall_second_at_every_pacing_rate() {
        for hz in [CLOCK_HZ, CLOCK_HZ / 100, CLOCK_HZ * 4, 10_000, 1_000_000] {
            let mut clk = LineClock703::new(2);
            clk.set_pacing_hz(hz);
            clk.dot(FN_LINE_CLOCK_CONNECT, 0);
            let mut pulses = 0;
            // one wall second of machine time, an instruction's worth at a time
            let mut fed = 0;
            while fed < hz as u32 {
                let chunk = 7.min(hz as u32 - fed);
                if clk.poll(chunk) != 0 {
                    pulses += 1;
                }
                fed += chunk;
            }
            assert_eq!(pulses, LINE_HZ, "{hz} Hz");
        }
    }

    /// The floor keeps the modulus in `poll` from dividing by zero when the
    /// pacing rate is under 60 Hz. One cycle to the tick is as fast as the
    /// device goes; whether a cpu paced that slowly can answer it is the
    /// operator's problem, not the device's.
    #[test]
    fn a_pacing_rate_under_sixty_hertz_floors_at_one_cycle() {
        let mut clk = LineClock703::new(2);
        clk.set_pacing_hz(10);
        clk.dot(FN_LINE_CLOCK_CONNECT, 0);
        assert_eq!(clk.poll(0), 0, "not even the floor ticks on no cycles at all");
        assert_eq!(clk.poll(1), 1 << 2);
        assert_eq!(clk.poll(1), 1 << 2);
    }
}
