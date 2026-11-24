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
//! fully populated 703: 32K words of core, and on the DIO channel a console
//! teletype, the Teletype Multiplexer with its sixteen channels, the high
//! speed paper tape reader and punch, the Time-of-Day Clock, its interval
//! jumper giving REX its tick, and a 74601 disc controller whose four drive
//! bays mount whatever `ray703-disc{0..3}.img` files the current directory
//! holds (the Kaypro floppy convention: a file that is not there is a drive
//! that was never installed). A multiplexer channel is an unplugged line
//! until `--serial` gives it a host end, and present either way, so that a
//! snapshot is the same machine whatever was plugged in.
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
    Mux703, TapePunch703, TapeReader703, TodClock703, Tty703, DEV_MUX, DEV_TAPE_PUNCH, DEV_TAPE_READER, DEV_TOD_CLOCK,
    DEV_TTY,
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

/// The multiplexer's channels `--serial` plugs, one line each in turn:
/// teletype n is channel n-1, REX's terminal n.
pub const SERIAL_PORTS: [&str; 3] = [
    "teletype 1, multiplexer channel 0",
    "teletype 2, multiplexer channel 1",
    "teletype 3, multiplexer channel 2",
];

pub struct Ray703 {
    core: Memory,
    /// The console.
    tty: Tty703,
    mux: Mux703,
    reader: TapeReader703,
    punch: TapePunch703,
    disc: Disc74601,
    tod: TodClock703,
}

impl Ray703 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        // The character devices signal level 0, which is what PTB and every
        // driver listing assume, and the multiplexer with them. A program
        // that arms one at a time knows which interrupted by what it
        // started; one that runs several at once asks each one's status.
        //
        // The disc gets level 1. X-RAY leaves every channel number to the
        // per-installation system description ('DSKI EQU NUMBER, cards
        // 77-81), so there is no canonical assignment to copy; a level of its
        // own above the 10-cps teletype's is the only sane priority for a DMA
        // device (`ready_level` scans 15 down to 0), and it makes this the
        // first machine here with two live interrupt levels.
        //
        // The clock's interval interrupt gets level 2, the next free one.
        // Not the character devices' 0: the clock has no status word, so a
        // shared service routine could not tell its interrupt from theirs.
        // It powers up stopped at zero, as the hardware does; the factory
        // stands in for the operator who set and started it
        // (`set_time_of_day`).
        let mut sys = Self::bare(console);

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

    /// The machine at power-on with nothing in core: the devices on their
    /// levels, as `new` documents.
    pub fn bare(console: ConsoleEndpoint) -> Self {
        Ray703 {
            core: Memory::new(CORE_BYTES),
            tty: Tty703::new(console, 0),
            mux: Mux703::new(0, unplugged),
            reader: TapeReader703::new(0),
            punch: TapePunch703::new(0),
            disc: Disc74601::new(1),
            tod: TodClock703::at_power_on(2),
        }
    }

    /// A flat core image from word 0, for a host with no files.
    pub fn with_core_image(image: &[u8], console: ConsoleEndpoint) -> Self {
        let mut sys = Self::bare(console);
        sys.core.load_at(0, image);
        sys
    }

    /// Mount a platter from bytes, kept in memory; false if it is not
    /// exactly one platter.
    pub fn mount_disc_bytes(&mut self, unit: usize, data: Vec<u8>) -> bool {
        self.disc.load_bytes(unit, data)
    }

    /// The controller's LOAD button: unit 0's sector 0, track 0 into words
    /// 0-46. False with nothing on unit 0.
    pub fn press_load(&mut self) -> bool {
        self.disc.press_load(&mut self.core)
    }

    /// `--fast-io`: run the teletype, the punch and the disc at host speed
    /// instead of ten characters a second, a hundred and ten frames and half
    /// a revolution per transfer. Not the tape reader, whose frame time is
    /// what lets a driver disconnect between records (`TapeReader703`).
    pub fn set_fast_io(&mut self) {
        self.tty.set_fast_io();
        self.mux.set_fast_io();
        self.punch.set_fast_io();
        self.disc.set_fast_io();
        // Not the clock: a periodic interrupt with no period never
        // returns. That is what lets a test run --fast-io for an instant
        // teletype over unchanged scheduling slices.
    }

    /// Set the time-of-day clock to `count` tenths of a millisecond since
    /// midnight and start it -- the operator at the clock's panel.
    pub fn set_time_of_day(&mut self, count: u32) {
        self.tod.set_and_start(count);
    }

    /// Give teletype `n` (1 to `SERIAL_PORTS.len()`) its host end: the line
    /// into multiplexer channel `n - 1`.
    pub fn attach_terminal(&mut self, n: usize, line: ConsoleEndpoint) {
        self.mux.attach(n - 1, line);
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
            1 => self.tty.load_tape(frames),
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
        let Ray703 { core, tty, mux, reader, punch, disc, tod } = self;
        w.section(b"CORE", |w| core.save(w));
        w.section(b"TTY ", |w| tty.save(w));
        w.section(b"MUX ", |w| mux.save(w));
        w.section(b"PTR ", |w| reader.save(w));
        w.section(b"PTP ", |w| punch.save(w));
        w.section(b"TOD ", |w| tod.save(w));
        // the disc last: nothing after it can fail once its platters are on
        w.section(b"DISC", |w| disc.save(w));
        Ok(())
    }

    fn load_state(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        r.section(b"CORE", |r| self.core.load(r))?;
        r.section(b"TTY ", |r| self.tty.load(r))?;
        r.section(b"MUX ", |r| self.mux.load(r))?;
        r.section(b"PTR ", |r| self.reader.load(r))?;
        r.section(b"PTP ", |r| self.punch.load(r))?;
        r.section(b"TOD ", |r| self.tod.load(r))?;
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
        match device {
            DEV_TTY => self.tty.din(function),
            DEV_MUX => self.mux.din(function),
            DEV_TAPE_READER => self.reader.din(function),
            DEV_TAPE_PUNCH => self.punch.din(function),
            DEV_DISC => self.disc.din(function),
            DEV_TOD_CLOCK => self.tod.din(function),
            _ => 0,
        }
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        let (device, function) = (port >> 4, port & 0x0f);
        match device {
            DEV_TTY => self.tty.dot(function, val),
            DEV_MUX => self.mux.dot(function, val),
            DEV_TAPE_READER => self.reader.dot(function, val),
            DEV_TAPE_PUNCH => self.punch.dot(function, val),
            DEV_DISC => self.disc.dot(function, val),
            DEV_TOD_CLOCK => self.tod.dot(function, val),
            _ => {}
        }
    }

    /// Every device gets the same elapsed cycle count, which is how they
    /// share one clock without sharing any state: the teletypes spend it
    /// running its character at ten a second, the reader ignores it, the
    /// punch and the disc count down what they have in flight, and the clock
    /// counts it. The disc alone is handed the core -- it is the
    /// machine's one DMA device, and this poll is the one place the memory
    /// and the devices meet.
    fn poll_interrupt_lines(&mut self, elapsed_cycles: u32) -> u16 {
        self.tty.poll(elapsed_cycles)
            | self.mux.poll(elapsed_cycles)
            | self.reader.poll(elapsed_cycles)
            | self.punch.poll(elapsed_cycles)
            | self.disc.poll(elapsed_cycles, &mut self.core)
            | self.tod.poll(elapsed_cycles)
    }

    /// Hand the pacing rate to the devices whose periods are I/O completions:
    /// the teletype's character time, the punch's frame and the disc's
    /// access, and the reader's frame.
    ///
    /// The clock takes it too: its crystal does not slow down because the
    /// cpu did. It sits out only `--fast-io`, which is the other axis --
    /// whether a device charges time at all, rather than what a second of it
    /// is worth in cycles.
    fn set_device_pacing_hz(&mut self, hz: u64) {
        self.tty.set_pacing_hz(hz);
        self.mux.set_pacing_hz(hz);
        self.punch.set_pacing_hz(hz);
        self.reader.set_pacing_hz(hz);
        self.disc.set_pacing_hz(hz);
        self.tod.set_pacing_hz(hz);
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

    /// A path that exists and holds `bytes`: `new` takes a path rather than
    /// bytes, so its tests need a file.
    fn scratch_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        // The pid keeps two people running the suite on one machine from
        // fighting over the same file.
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("emu-ray703-{pid}-{name}"));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_core_image_from_bytes_loads_at_word_0() {
        let (_tx, rx) = mpsc::channel();
        let m = Ray703::with_core_image(&[0x12, 0x34], ConsoleEndpoint::new(rx, Box::new(Vec::new())));
        assert_eq!((m.peek8(0), m.peek8(1), m.peek8(2)), (Some(0x12), Some(0x34), Some(0)));
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

    /// `--serial`'s names say the multiplexer channel each line is.
    #[test]
    fn the_serial_ports_name_their_channels() {
        for (n, port) in SERIAL_PORTS.iter().enumerate() {
            assert_eq!(*port, format!("teletype {}, multiplexer channel {n}", n + 1));
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
        assert_eq!(sys.io_read16(0xe0), 0, "the teletype did not stir");
        assert_eq!(sys.io_read16(0xa0), 0, "nor the multiplexer");
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
        // a reader frame's time; fast-io leaves the reader its 300 a second
        sys.poll_interrupt_lines((crate::cpu::ray703::CLOCK_HZ / 300) as u32);
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

    /// The console teletype and the multiplexer share level 0 and answer
    /// their own codes: a key typed on teletype 2 is multiplexer channel 1's
    /// input word, what the program sends to teletype 1 comes out of that
    /// line alone, and a set prints its own keys. Codes 7, 6 and 4 answer
    /// nothing.
    #[test]
    fn the_terminals_are_the_console_and_the_multiplexer() {
        let rom = scratch_file("terminals", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        let mut printed = Vec::new();
        for n in 1..=SERIAL_PORTS.len() {
            let (endpoint, out) = line(if n == 2 { b"K" } else { b"" });
            sys.attach_terminal(n, endpoint);
            printed.push(out);
        }
        sys.set_fast_io();
        for ch in 0..3 {
            sys.io_write16(0xa1, ch); // DOT A,1: listen
        }
        assert_eq!(sys.poll_interrupt_lines(0), 1, "the key, on level 0");
        assert_eq!(sys.io_read16(0xa0), 0x8001, "stopped, a channel selected");
        assert_eq!(sys.io_read16(0xa1), 0x8100 | 0x80 | b'K' as u16, "channel 1's key");
        assert_eq!(sys.io_read16(0xa0), 0x0001, "moved on");

        sys.io_write16(0xa2, 0); // DOT A,2: channel 0 for output
        assert_eq!(sys.poll_interrupt_lines(0), 1);
        assert_eq!(sys.io_read16(0xa1), 0x0000, "channel 0 ready for a character");
        sys.io_write16(0xa3, 0x80 | b'X' as u16);
        assert_eq!(*printed[0].lock().unwrap(), b"X", "teletype 1's paper");
        assert_eq!(*printed[1].lock().unwrap(), b"K", "teletype 2 prints its own key");
        assert!(printed[2].lock().unwrap().is_empty());
        assert_eq!(sys.io_read16(0xe0), 0, "the console did not stir");
        for dev in [7u8, 6, 4] {
            assert_eq!(sys.io_read16(dev << 4), 0, "nothing at {dev}");
        }
    }

    /// The clock answers at B, powers up stopped at zero, and the factory's
    /// setting starts it; its interval interrupt is level 2, and code 2,
    /// which is the DMA magnetic tape's, answers nothing.
    #[test]
    fn the_clock_is_device_b_and_interrupts_on_level_2() {
        let rom = scratch_file("clock-dio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        let interval = (crate::cpu::ray703::CLOCK_HZ / 50) as u32;
        assert_eq!(sys.poll_interrupt_lines(10 * interval), 0, "stopped at power-on");
        assert_eq!(sys.io_read16(0xb2), 0);
        sys.set_time_of_day(10); // a millisecond past midnight
        assert_eq!(sys.io_read16(0xb2), 0x0001);
        assert_eq!(sys.poll_interrupt_lines(interval * 19 / 20 - 1), 0);
        assert_eq!(sys.poll_interrupt_lines(4), 1 << 2, "at 20 ms on the clock");
        sys.io_write16(0xb4, 0); // DOT B,4 -- stop
        assert_eq!(sys.poll_interrupt_lines(10 * interval), 0);
        sys.io_write16(0x21, 0);
        assert_eq!(sys.io_read16(0x20), 0, "nothing at code 2");
        assert_eq!(sys.poll_interrupt_lines(10 * interval), 0);
    }

    /// Guards the deliberate gap in `set_fast_io`: the clock keeps its
    /// interval while the teletype and the disc go instant.
    #[test]
    fn the_clock_ignores_fast_io() {
        let rom = scratch_file("clock-fastio", &[0x01, 0x00]);
        let mut sys = machine("", &rom).unwrap();
        sys.set_fast_io();
        sys.set_time_of_day(0);
        let interval = (crate::cpu::ray703::CLOCK_HZ / 50) as u32;
        assert_eq!(sys.poll_interrupt_lines(0), 0, "nothing here is instant");
        assert_eq!(sys.poll_interrupt_lines(interval - 2), 0);
        assert_eq!(sys.poll_interrupt_lines(4), 1 << 2);
    }

    /// The pacing rate reaches every device with a period, the clock
    /// included. At a hundredth of the machine's speed a teletype character
    /// and a clock interval are both a hundredth of the cycles, which is what
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

        let interval = (hz / 50) as u32;
        sys.set_time_of_day(0);
        assert_eq!(sys.poll_interrupt_lines(interval - 1), 0, "one cycle short of 20 ms");
        assert_eq!(sys.poll_interrupt_lines(2), 1 << 2, "the interval, on level 2");
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

        // 31 frames at 300 a second is some 59,000 cycles, nearly all of
        // them spent a step at a time in PTB's wait.
        for _ in 0..100000 {
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
