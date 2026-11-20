// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Raytheon 703 (1967).
//!
//! Not a port of anything -- the C++ tree never had this machine. It is a
//! fully populated 703: 32K words of core, a console teletype and the high
//! speed paper tape reader and punch on the DIO channel, and a 74601 disc
//! controller whose
//! four drive bays mount whatever `ray703-disc{0..3}.img` files the current
//! directory holds (the Kaypro floppy convention: a file that is not there is
//! a drive that was never installed). Plus two things Raytheon did not
//! sell in this form, where its catalogue had a time-of-day clock and a
//! teletype multiplexer: the invented 60 Hz line clock (`dev/ray703.rs` says
//! invented at length), silent until a program connects it, and three more
//! teletypes beside the console, on the console's level -- each an
//! unplugged line until `--serial` gives it a host end, and present either
//! way, so that a snapshot is the same machine whatever was plugged in.
//!
//! The 703 has no ROM at all. An operator keyed a bootstrap in from the front
//! panel and fed the machine an absolute paper tape; there was nothing in core
//! at power-on. There is no front panel here, so the two subsystems stand in
//! for the two halves of that ritual:
//!
//! * `ray703` loads a flat image into core from word 0 and starts there, which
//!   is what a keyed-in program amounts to.
//! * `ray703-ptb` keys in the real thing instead -- the eleven words of the
//!   PTB bootstrap (drawing 390364) -- and treats `-r` as the paper tape to
//!   feed it, so a period absolute tape loads the period way.
//! * `ray703-load` presses the LOAD button on the disc controller (706 UM
//!   Table 5-30): `-r` is reinterpreted as the disc image for unit 0, and the
//!   button's fixed sequence reads its sector 0, track 0 into words 0-46.
//!   The button lives on the controller cabinet, not on the figure 5-1 front
//!   panel this tree draws, so like PTB's preset index register it is an
//!   operator action a subsystem stands in for rather than a switch to click.

use crate::bus::{Bus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::disc74601::{Disc74601, DEV_DISC};
use crate::dev::memory::Memory;
use crate::dev::ray703::{
    LineClock703, TapePunch703, TapeReader703, Tty703, DEV_LINE_CLOCK, DEV_TAPE_PUNCH, DEV_TAPE_READER,
    DEV_TERMINALS,
};
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/703/demo.bin";

/// 32,768 words, the largest core the 703 could be ordered with, as a flat
/// byte space. Word N is bytes 2N and 2N+1, big-endian.
const CORE_BYTES: usize = 64 * 1024;

/// PTB, the paper tape bootstrap, keyed into words 0-A from the front panel.
///
/// This is the teleprinter listing with the two high-speed-reader
/// substitutions applied (word 2 `DOT 13,9`, word 4 `DIN 13,13`), because the
/// tape here is a file rather than something threaded through the console.
///
/// Word 1 does double duty and is the reason it fits in eleven words: the
/// hardware reads it as the level 0 linkage address, where 0x8004 masks to
/// word 4, and the straight-line path executes it once as a harmless LDW.
const PTB: [u16; 11] = [
    0x0020, // 0  ENB 0        enable interrupt 0
    0x8004, // 1  LDW SERV     interrupt service address
    0x03d9, // 2  DOT 13,9     start tape
    0x1003, // 3  JMP $        wait for interrupt
    0x02dd, // 4  DIN 13,13    input frame            <- SERV
    0x0800, // 5  SAZ          non-zero?              <- TEST
    0x0401, // 6  IXS 1        yes, load next frame
    0x0010, // 7  INR 0        no, restore interrupt
    0x0638, // 8  LLB X'38'    change test to STB *0
    0x300a, // 9  STB /TEST
    0x0010, // A  INR 0
];

/// PTB expects the operator to have set the index register to the byte origin
/// of the program minus twelve. Twelve bytes are consumed getting the service
/// routine to rewrite itself, so this makes the first real frame land on the
/// origin. Word 0x100 is the first word above PTB and its interrupt block that
/// is a round number.
///
/// `tools/asm703.py --tape-origin` has to agree with this, since a tape's
/// contents are meaningless at any other address; it defaults to the same
/// number and says so.
const PTB_LOAD_ORIGIN: u16 = 0x0100;

/// How many teletypes, the console included (`DEV_TERMINALS`).
pub const TERMINALS: usize = DEV_TERMINALS.len();

/// The extra terminals as `--serial` names them, one line each in turn.
pub const SERIAL_PORTS: [&str; TERMINALS - 1] =
    ["teletype 1, DIO device 7", "teletype 2, DIO device 6", "teletype 3, DIO device 4"];

/// The snapshot section of each teletype, the console's first.
const TTY_SECTIONS: [&[u8; 4]; TERMINALS] = [b"TTY ", b"TTY1", b"TTY2", b"TTY3"];

pub struct Ray703 {
    core: Memory,
    /// The console first, then the extra terminals in `DEV_TERMINALS` order.
    ttys: [Tty703; TERMINALS],
    reader: TapeReader703,
    punch: TapePunch703,
    disc: Disc74601,
    line_clock: LineClock703,
}

impl Ray703 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        // The character devices signal level 0, which is what PTB and every
        // driver listing assume. A program that arms one at a time knows
        // which interrupted by what it started; one that runs several at
        // once asks each teletype's status word.
        //
        // The disc gets level 1. X-RAY leaves every channel number to the
        // per-installation system description ('DSKI EQU NUMBER, cards
        // 77-81), so there is no canonical assignment to copy; a level of its
        // own above the 10-cps teletype's is the only sane priority for a DMA
        // device (`ready_level` scans 15 down to 0), and it makes this the
        // first machine here with two live interrupt levels.
        //
        // The invented line clock gets level 2, the next free one. Not the
        // character devices' 0: a tick has no collect function, so a shared
        // service routine could not tell a tick from the empty DIN that
        // already means something there.
        let mut sys = Ray703 {
            core: Memory::new(CORE_BYTES),
            ttys: [
                Tty703::new(console, 0),
                Tty703::new(unplugged(), 0),
                Tty703::new(unplugged(), 0),
                Tty703::new(unplugged(), 0),
            ],
            reader: TapeReader703::new(0),
            punch: TapePunch703::new(0),
            disc: Disc74601::new(1),
            line_clock: LineClock703::new(2),
        };

        match subsystem {
            "" => {
                let image = rom::load_binary(rom_path)?;
                if image.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} is empty; there is nothing to run", rom_path.display()),
                    ));
                }
                sys.core.load_at(0, &image);
            }
            "ptb" => {
                for (word, insn) in PTB.iter().enumerate() {
                    sys.core.load_at(word * 2, &insn.to_be_bytes());
                }
                sys.reader.load_image(rom_path)?;
                println!(
                    "703: PTB keyed into words 0-A; the tape will load at word {:#06x}.",
                    PTB_LOAD_ORIGIN
                );
                println!(
                    "703: PTB is a loader and stops there -- an operator would now press \
                     HALT and RESET and key in a start address."
                );
            }
            "load" => {
                // The LOAD button (Table 5-30) with `-r` as the boot disc.
                // Unlike the silent probe for the working directory's images,
                // a disc the user named on the command line has to be there
                // -- the tape reader's rule, for the same reason: pressing
                // LOAD over an empty drive bay looks like a hung machine.
                if !sys.disc.load_image(0, rom_path) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "could not mount '{}' as the boot disc (missing, unreadable, \
                             or not exactly 770,048 bytes)",
                            rom_path.display()
                        ),
                    ));
                }
                sys.disc.press_load(&mut sys.core);
                println!(
                    "703: LOAD pressed; disc 0's sector 0, track 0 is in words 0-46."
                );
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "unknown ray703 subsystem '{other}'; try 'ray703', 'ray703-ptb' \
                         or 'ray703-load' (add '-panel' for the front panel window, \
                         '-tty' for the teletype window)"
                    ),
                ));
            }
        }

        Ok(sys)
    }

    /// `--fast-io`: run the teletype, the punch and the disc at host speed
    /// instead of ten characters a second, a hundred and ten frames and half
    /// a revolution per transfer. The tape reader already free-runs.
    pub fn set_fast_io(&mut self) {
        for tty in &mut self.ttys {
            tty.set_fast_io();
        }
        self.punch.set_fast_io();
        self.disc.set_fast_io();
        // Not the line clock: a periodic interrupt with no period never
        // returns. That is what lets a test run --fast-io for an instant
        // teletype over unchanged scheduling slices.
    }

    /// Give extra terminal `n` (1 to `TERMINALS - 1`) its host end.
    pub fn attach_terminal(&mut self, n: usize, line: ConsoleEndpoint) {
        self.ttys[n].set_console(line);
    }

    /// Wire the panel window's disc rack lamps into the controller.
    pub fn attach_disc_rack(&mut self, rack: crate::console::DiscRackState) {
        self.disc.attach_rack(rack);
    }

    /// Try to mount a disc image on one of the controller's four units.
    /// Policy -- which paths, and that a missing one is fine -- lives in the
    /// factory, the way the registry hands the Kaypro its floppy path.
    pub fn mount_disc(&mut self, unit: usize, path: &Path) -> bool {
        self.disc.load_image(unit, path)
    }

    /// The index register PTB needs preloaded, or `None` for a plain boot.
    ///
    /// The front panel set this by hand; with no panel, the machine hands it
    /// to the factory to poke into the core before it starts.
    pub fn ptb_index(subsystem: &str) -> Option<u16> {
        (subsystem == "ptb").then(|| (PTB_LOAD_ORIGIN * 2).wrapping_sub(12))
    }
}

/// A terminal with nothing on its line: no keys ever arrive and what it
/// prints goes nowhere.
fn unplugged() -> ConsoleEndpoint {
    let (_, rx) = std::sync::mpsc::channel();
    ConsoleEndpoint::new(rx, Box::new(io::sink()))
}

impl Bus for Ray703 {
    fn read8(&mut self, addr: u32) -> u8 {
        self.core.read_byte(addr & 0xffff)
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.core.write_byte(addr & 0xffff, val);
    }

    fn save_disk(&self, unit: usize, path: &Path) -> Result<(), String> {
        let platter = self.disc.platter(unit).ok_or_else(|| format!("no platter on disc unit {unit}"))?;
        crate::snapshot::write_new_file(path, platter)
    }

    /// Reader 0 is the high speed reader, reader 1 the console teletype's
    /// own.
    fn load_tape(&mut self, reader: usize, frames: Vec<u8>) -> Result<(), String> {
        match reader {
            0 => self.reader.load_tape(frames),
            1 => self.ttys[0].load_tape(frames),
            n => return Err(format!("no paper tape reader {n}: 0 is the high speed reader, 1 the teletype's")),
        }
        Ok(())
    }

    fn save_tape(&mut self, path: &Path) -> Result<(), String> {
        crate::snapshot::write_new_file(path, self.punch.tape())?;
        self.punch.take_tape();
        Ok(())
    }

    fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        let Ray703 { core, ttys, reader, punch, disc, line_clock } = self;
        w.section(b"CORE", |w| core.save(w));
        for (tty, tag) in ttys.iter().zip(TTY_SECTIONS) {
            w.section(tag, |w| tty.save(w));
        }
        w.section(b"PTR ", |w| reader.save(w));
        w.section(b"PTP ", |w| punch.save(w));
        w.section(b"CLK ", |w| line_clock.save(w));
        // the disc last: nothing after it can fail once its platters are on
        w.section(b"DISC", |w| disc.save(w));
        Ok(())
    }

    fn load_state(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        r.section(b"CORE", |r| self.core.load(r))?;
        for (tty, tag) in self.ttys.iter_mut().zip(TTY_SECTIONS) {
            r.section(tag, |r| tty.load(r))?;
        }
        r.section(b"PTR ", |r| self.reader.load(r))?;
        r.section(b"PTP ", |r| self.punch.load(r))?;
        r.section(b"CLK ", |r| self.line_clock.load(r))?;
        r.section(b"DISC", |r| self.disc.load(r))
    }

    fn commit_loaded_disks(&mut self) {
        self.disc.commit_loaded();
    }

    /// Every device is on the DIO channel, so all of core is peekable.
    fn peek8(&self, addr: u32) -> Option<u8> {
        Some(self.core.peek(addr & 0xffff))
    }

    /// The DIO address is a device nibble and a function nibble (4-2.1).
    /// Reading a device that isn't there yields zero, as the open input bus
    /// would.
    fn io_read16(&mut self, port: u8) -> u16 {
        let (device, function) = (port >> 4, port & 0x0f);
        if let Some(n) = DEV_TERMINALS.iter().position(|&d| d == device) {
            return self.ttys[n].din(function);
        }
        match device {
            DEV_TAPE_READER => self.reader.din(function),
            DEV_TAPE_PUNCH => self.punch.din(function),
            DEV_DISC => self.disc.din(function),
            _ => 0,
        }
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        let (device, function) = (port >> 4, port & 0x0f);
        if let Some(n) = DEV_TERMINALS.iter().position(|&d| d == device) {
            return self.ttys[n].dot(function, val);
        }
        match device {
            DEV_TAPE_READER => self.reader.dot(function, val),
            DEV_TAPE_PUNCH => self.punch.dot(function, val),
            DEV_DISC => self.disc.dot(function, val),
            // no read arm anywhere for the clock: it has no readable
            // register, so its DIN is the open-bus default below on purpose
            DEV_LINE_CLOCK => self.line_clock.dot(function, val),
            _ => {}
        }
    }

    /// Every device gets the same elapsed cycle count, which is how they
    /// share one clock without sharing any state: the teletypes spend it
    /// running its character at ten a second, the reader ignores it, the
    /// punch and the disc count down what they have in flight, and the line
    /// clock banks it toward the next tick. The disc alone is handed the core -- it is the
    /// machine's one DMA device, and this poll is the one place the memory
    /// and the devices meet.
    fn poll_interrupt_lines(&mut self, elapsed_cycles: u32) -> u16 {
        let ttys = self.ttys.iter_mut().fold(0, |lines, tty| lines | tty.poll(elapsed_cycles));
        ttys | self.reader.poll(elapsed_cycles)
            | self.punch.poll(elapsed_cycles)
            | self.disc.poll(elapsed_cycles, &mut self.core)
            | self.line_clock.poll(elapsed_cycles)
    }

    /// Hand the pacing rate to the devices whose periods are I/O completions:
    /// the teletype's character time, the punch's frame and the disc's
    /// access. The reader takes no time at all -- it free-runs by design.
    ///
    /// The line clock takes it too: the mains does not slow down because the
    /// cpu did. It sits out only `--fast-io`, which is the other axis --
    /// whether a device charges time at all, rather than what a second of it
    /// is worth in cycles.
    fn set_device_pacing_hz(&mut self, hz: u64) {
        for tty in &mut self.ttys {
            tty.set_pacing_hz(hz);
        }
        self.punch.set_pacing_hz(hz);
        self.disc.set_pacing_hz(hz);
        self.line_clock.set_pacing_hz(hz);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Endian;
    use crate::cpu::ray703::Cpu703;
    use crate::cpu::{Cpu, StepResult};
    use std::sync::mpsc;

    fn machine(subsystem: &str, rom: &Path) -> io::Result<Ray703> {
        let (_tx, rx) = mpsc::channel();
        // the sender is dropped, so try_next_char just reports nothing
        Ray703::new(rom, ConsoleEndpoint::new(rx, Box::new(Vec::new())), subsystem)
    }

    /// A path that exists and holds `bytes`. These constructors take a path
    /// rather than bytes, so there is no way to test them without a file.
    fn scratch_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        // The pid keeps two people running the suite on one machine from
        // fighting over the same file.
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("emu-ray703-{pid}-{name}"));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// All of core is peekable -- the devices live on the DIO channel --
    /// and a poke is a plain core write.
    #[test]
    fn peek_reads_the_core_and_poke_writes_it() {
        let rom = scratch_file("peek", &[0x12, 0x34]);
        let mut m = machine("", &rom).unwrap();
        assert_eq!((m.peek8(0), m.peek8(1)), (Some(0x12), Some(0x34)));
        assert_eq!(m.peek8(0xffff), Some(0), "the top of core, empty");
        m.poke8(0x100, 0xab).unwrap();
        assert_eq!(m.read8(0x100), 0xab);
    }

    #[test]
    fn an_unknown_subsystem_is_rejected_by_name() {
        let rom = scratch_file("unknown", &[0x01, 0x00]);
        let Err(err) = machine("frobnitz", &rom) else {
            panic!("an unknown subsystem should not build a machine");
        };
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
        assert!(err.to_string().contains("frobnitz"));
    }

    #[test]
    fn a_missing_image_is_a_hard_error_in_either_mode() {
        let missing = Path::new("/nonexistent/ray703/image");
        assert!(machine("", missing).is_err());
        assert!(machine("ptb", missing).is_err());
    }

    #[test]
    fn an_empty_image_is_rejected() {
        let rom = scratch_file("empty", &[]);
        assert!(machine("", &rom).is_err());
    }

    /// The image is big-endian word pairs loaded from word 0, so a word
    /// instruction fetched by the core sees exactly what was in the file.
    #[test]
    fn an_image_loads_as_big_endian_words() {
        let rom = scratch_file("image", &[0x03, 0xe9, 0x10, 0x03]);
        let mut sys = machine("", &rom).unwrap();
        assert_eq!(sys.read16(0, Endian::Big), 0x03e9);
        assert_eq!(sys.read16(2, Endian::Big), 0x1003);
    }

    #[test]
    fn the_ptb_subsystem_keys_the_bootstrap_and_mounts_the_tape() {
        let tape = scratch_file("tape", &[0x00, 0xff]);
        let mut sys = machine("ptb", &tape).unwrap();
        assert_eq!(sys.read16(0, Endian::Big), 0x0020, "ENB 0");
        assert_eq!(sys.read16(2 * 2, Endian::Big), 0x03d9, "the high speed reader variant");
        assert_eq!(Ray703::ptb_index("ptb"), Some(0x0200 - 12));
        assert_eq!(Ray703::ptb_index(""), None);
    }

    #[test]
    fn the_dio_decode_splits_device_from_function() {
        let rom = scratch_file("dio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        // arming the tape reader (DOT 13,9) must not arm the teletype
        sys.io_write16(0xd9, 0);
        assert_eq!(sys.poll_interrupt_lines(0), 0, "no tape is mounted");
        // and an absent device reads back zero rather than panicking
        assert_eq!(sys.io_read16(0x1d), 0);
    }

    /// `--serial`'s names for the extra teletypes say the codes they answer.
    #[test]
    fn the_serial_ports_name_their_device_codes() {
        for (port, dev) in SERIAL_PORTS.iter().zip(&DEV_TERMINALS[1..]) {
            assert!(port.ends_with(&format!("DIO device {dev:X}")), "{port}");
        }
    }

    /// The punch answers device 12 and the reader device 13, each with its
    /// own status word, and neither is a teletype.
    #[test]
    fn the_punch_and_the_reader_answer_their_own_codes() {
        let rom = scratch_file("tapes-dio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        sys.set_fast_io();
        assert_eq!(sys.io_read16(0xc0), 0xc000, "punch: off line, motor off");
        assert_eq!(sys.io_read16(0xd0), 0xc000, "reader: off line");
        sys.io_write16(0xce, 0x8a); // DOT 12,14
        assert_eq!(sys.poll_interrupt_lines(0), 1, "punched, on level 0");
        assert_eq!(sys.io_read16(0xc0), 0x0100);
        for &dev in &DEV_TERMINALS {
            assert_eq!(sys.io_read16(dev << 4), 0, "no teletype stirred");
        }
    }

    /// The port's `tape` and `savetape`: reader 0 is the high speed reader
    /// and 1 the console's own, and the punched tape goes to a new file
    /// once, torn off.
    #[test]
    fn tapes_go_in_the_readers_and_come_off_the_punch() {
        let rom = scratch_file("tapes", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        sys.set_fast_io();
        sys.load_tape(0, vec![0x11]).unwrap();
        sys.load_tape(1, vec![0x22]).unwrap();
        assert!(sys.load_tape(2, vec![]).is_err());
        sys.io_write16(0xd1, 0); // DOT 13,1
        sys.io_write16(0xe9, 0); // DOT 14,9
        sys.poll_interrupt_lines(0);
        assert_eq!(sys.io_read16(0xd5), 0x11);
        assert_eq!(sys.io_read16(0xed), 0x22, "as punched, no eighth bit added");

        sys.io_write16(0xc6, 0x1ff); // DOT 12,6
        let out = std::env::temp_dir().join(format!("emu-ray703-{}-punched", std::process::id()));
        std::fs::remove_file(&out).ok();
        sys.save_tape(&out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), [0xff]);
        sys.io_write16(0xc6, 0x01);
        assert!(sys.save_tape(&out).is_err(), "never over a file that is there");
        std::fs::remove_file(&out).unwrap();
        sys.save_tape(&out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), [0x01], "the refused tape was still in the punch");
        std::fs::remove_file(&out).ok();
    }

    /// A plugged-in line whose keys are `input`, and what it prints.
    fn line(input: &[u8]) -> (ConsoleEndpoint, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for Shared {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let (tx, rx) = mpsc::channel();
        for &c in input {
            tx.send(c).unwrap();
        }
        let printed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (ConsoleEndpoint::new(rx, Box::new(Shared(printed.clone()))), printed)
    }

    /// Each teletype answers its own device code: what the program prints on
    /// one comes out of that line alone, a key typed on one is that one's to
    /// collect, and only its status word says it interrupted -- all four on
    /// level 0, the console's.
    #[test]
    fn each_teletype_answers_its_own_device_code() {
        let rom = scratch_file("terminals", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        let mut printed = Vec::new();
        for n in 1..TERMINALS {
            let (endpoint, out) = line(if n == 2 { b"K" } else { b"" });
            sys.attach_terminal(n, endpoint);
            printed.push(out);
        }
        sys.set_fast_io();
        let port = |dev: u8, function: u8| (dev << 4) | function;
        // a character on teletype 1, and every keyboard armed
        sys.io_write16(port(DEV_TERMINALS[1], 0xe), 0x80 | b'X' as u16);
        for &dev in &DEV_TERMINALS {
            sys.io_write16(port(dev, 0xb), 0);
        }
        assert_eq!(sys.poll_interrupt_lines(0), 1, "one level for all of them");
        let status: Vec<u16> = DEV_TERMINALS.iter().map(|&d| sys.io_read16(port(d, 0x0))).collect();
        assert_eq!(status, [0x8000, 0x8100, 0x8100, 0x8000], "teletype 1 printed, teletype 2 has a key");
        assert_eq!(sys.io_read16(port(DEV_TERMINALS[2], 0xf)), 0x80 | b'K' as u16);
        assert_eq!(sys.io_read16(port(DEV_TERMINALS[3], 0xf)), 0, "nothing typed there");
        assert_eq!(*printed[0].lock().unwrap(), b"X", "teletype 1's paper");
        assert_eq!(*printed[1].lock().unwrap(), b"K", "teletype 2 echoes its own keyboard");
        assert!(printed[2].lock().unwrap().is_empty());
    }

    /// The invented line clock: DOT 2,1 connects it, the pulse arrives on
    /// its own level 2 exactly one period later, and the decode isolates it
    /// -- no other device stirs, and its read side is the open-bus zero.
    #[test]
    fn a_dot_to_device_2_arms_only_the_line_clock() {
        let rom = scratch_file("clock-dio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        let period = (crate::cpu::ray703::CLOCK_HZ / 60) as u32;
        sys.io_write16(0x21, 0); // DOT 2,1
        assert_eq!(sys.poll_interrupt_lines(period - 1), 0, "one cycle short of a period");
        assert_eq!(sys.poll_interrupt_lines(1), 1 << 2, "the tick, on the clock's own level");
        assert_eq!(sys.io_read16(0x21), 0, "no readable register");
        assert_eq!(sys.io_read16(0x20), 0);
        sys.io_write16(0x20, 0); // DOT 2,0 disconnects
        assert_eq!(sys.poll_interrupt_lines(u32::MAX), 0);
    }

    /// Guards the deliberate gap in `set_fast_io`: the clock keeps its
    /// period while the teletype and the disc go instant.
    #[test]
    fn the_line_clock_ignores_fast_io() {
        let rom = scratch_file("clock-fastio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        sys.set_fast_io();
        sys.io_write16(0x21, 0);
        let period = (crate::cpu::ray703::CLOCK_HZ / 60) as u32;
        assert_eq!(sys.poll_interrupt_lines(0), 0, "nothing here is instant");
        assert_eq!(sys.poll_interrupt_lines(period - 1), 0);
        assert_eq!(sys.poll_interrupt_lines(1), 1 << 2);
    }

    /// The pacing rate reaches every device with a period, the line clock
    /// included. At a hundredth of the machine's speed a teletype character
    /// and a clock tick are both a hundredth of the cycles, which is what
    /// keeps each of them the fraction of a *wall* second it claims to be.
    #[test]
    fn the_pacing_rate_reaches_the_teletype_and_the_clock() {
        let rom = scratch_file("pacing", &[0x01, 0x00]);
        let hz = crate::cpu::ray703::CLOCK_HZ / 100;
        let mut sys = machine("", &rom).unwrap();
        sys.set_device_pacing_hz(hz);

        let char_cycles = (hz / 10) as u32;
        sys.io_write16(0xee, 0xc1); // DOT 14,E -- print a character
        assert_eq!(sys.poll_interrupt_lines(char_cycles - 1), 0, "not printed yet");
        assert_eq!(sys.poll_interrupt_lines(1), 1, "the completion, on level 0");

        let period = (hz / 60) as u32;
        sys.io_write16(0x21, 0); // DOT 2,1 -- connect the clock
        assert_eq!(sys.poll_interrupt_lines(period - 1), 0, "one cycle short of a period");
        assert_eq!(sys.poll_interrupt_lines(1), 1 << 2, "the tick, on level 2");
    }

    /// End to end through the real core: PTB loads an absolute tape out of a
    /// file exactly as it would have off a reel in 1968.
    #[test]
    fn the_bootstrap_loads_a_tape_through_the_machine() {
        let payload: Vec<u8> = (0..16).map(|i| 0x41 + i).collect();
        let mut tape = vec![0u8; 3]; // blank leader
        tape.extend(std::iter::repeat_n(0xffu8, 12)); // the self-modify dance
        tape.extend_from_slice(&payload);

        let path = scratch_file("bootstrap", &tape);
        let mut sys = machine("ptb", &path).unwrap();
        // The factory presets the index and main.rs resets afterwards, so do
        // it in that order here: a reset that clobbered the preset would leave
        // PTB storing frames over itself.
        let mut cpu = Cpu703::new();
        cpu.set_index(Ray703::ptb_index("ptb").unwrap());
        cpu.reset(&mut sys);

        for _ in 0..10000 {
            assert_eq!(cpu.step(&mut sys), StepResult::Ok);
        }

        let origin = (PTB_LOAD_ORIGIN * 2) as u32;
        for (i, b) in payload.iter().enumerate() {
            assert_eq!(sys.read8(origin + i as u32), *b, "payload byte {i}");
        }
    }

    /// The LOAD button's whole fixed sequence (Table 5-30): sector 0, track 0
    /// of disc 0 lands in words 0-46, and only that -- word 47 of the image
    /// is deliberately non-zero to pin the one-sector cutoff.
    #[test]
    fn the_load_subsystem_boots_from_the_disc_image() {
        let mut image = vec![0u8; crate::dev::disc74601::IMAGE_BYTES];
        image[0..2].copy_from_slice(&0x1040u16.to_be_bytes()); // JMP X'40'
        image[92..94].copy_from_slice(&0xbeefu16.to_be_bytes()); // word 46, last loaded
        image[94..96].copy_from_slice(&0xdeadu16.to_be_bytes()); // word 47, sector 1: not loaded
        let path = scratch_file("bootdisc", &image);
        let mut sys = machine("load", &path).unwrap();
        assert_eq!(sys.read16(0, Endian::Big), 0x1040);
        assert_eq!(sys.read16(46 * 2, Endian::Big), 0xbeef);
        assert_eq!(sys.read16(47 * 2, Endian::Big), 0, "LOAD reads one sector exactly");
    }

    #[test]
    fn the_load_subsystem_needs_a_real_disc_image() {
        assert!(machine("load", Path::new("/nonexistent/boot.img")).is_err());
        let short = scratch_file("shortdisc", &[0u8; 512]);
        assert!(machine("load", &short).is_err(), "a wrong-size image is refused");
    }

    #[test]
    fn the_disc_answers_on_dio_device_1() {
        let rom = scratch_file("disc-dio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        // With no image in the working directory the controller is present
        // but the drive is not: DIN 1,0 reads not-ready, where an absent
        // *device* would read zero -- which as a status word means ready.
        assert_eq!(sys.io_read16(0x10), 0x8000);
        // and disc DOTs disturb neither character device
        sys.io_write16(0x11, 0x0100);
        assert_eq!(sys.poll_interrupt_lines(0), 0);
    }

    /// The DMA seam end to end: a transfer commanded over the DIO reaches
    /// core through `poll_interrupt_lines`, and completes on the disc's own
    /// level 1 rather than the character devices' level 0.
    #[test]
    fn a_disc_transfer_reaches_core_through_the_bus() {
        let rom = scratch_file("disc-dma", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        sys.disc.mount_blank(0);
        for i in 0..10u16 {
            sys.write16(0x200 + i as u32 * 2, 0x5000 + i, Endian::Big);
        }
        // write words 0x100.. to track 4 sector 2, then read them back higher
        sys.io_write16(0x11, 0x0100);
        sys.io_write16(0x12, (4 << 10) | 2);
        sys.io_write16(0x14, 10);
        assert_eq!(sys.poll_interrupt_lines(u32::MAX), 1 << 1, "completion pulses level 1");
        sys.io_write16(0x11, 0x0300);
        sys.io_write16(0x12, (4 << 10) | 2);
        sys.io_write16(0x16, 10);
        assert_eq!(sys.poll_interrupt_lines(u32::MAX), 1 << 1);
        for i in 0..10u16 {
            assert_eq!(sys.read16(0x600 + i as u32 * 2, Endian::Big), 0x5000 + i, "word {i}");
        }
    }
    /// A snapshot refused only once all of it has been read -- a byte too
    /// many at its end -- has still changed no drive: the platter is the
    /// file it was, and a write after the rollback reaches it.
    #[test]
    fn a_snapshot_refused_at_its_end_leaves_the_platter_its_file() {
        use crate::snapshot::{load_machine, roll_back, save_machine};
        let rom = scratch_file("refused-rom", &[0x01, 0x00]);
        let platter = scratch_file("refused-disc", &vec![0u8; crate::dev::disc74601::IMAGE_BYTES]);
        let mut sys = machine("", &rom).unwrap();
        assert!(sys.mount_disc(0, &platter));
        let mut cpu = crate::cpu::ray703::Cpu703::new();
        let before = save_machine("ray703", &cpu, &sys).unwrap();
        let mut long = before.clone();
        long.push(0);
        assert!(load_machine(&long, "ray703", &mut cpu, &mut sys).is_err());
        roll_back(&before, "ray703", &mut cpu, &mut sys).unwrap();

        sys.write16(0x200, 0x5a5a, Endian::Big);
        sys.io_write16(0x11, 0x0100);
        sys.io_write16(0x12, 0);
        sys.io_write16(0x14, 1);
        assert_eq!(sys.poll_interrupt_lines(u32::MAX), 1 << 1);
        assert_eq!(std::fs::read(&platter).unwrap()[..2], [0x5a, 0x5a], "the write reached the file");
        std::fs::remove_file(&platter).ok();
    }
}
