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
//! fully populated 703: 32K words of core, a console teletype and a high speed
//! paper tape reader on the DIO channel, and a 74601 disc controller whose
//! four drive bays mount whatever `ray703-disc{0..3}.img` files the current
//! directory holds (the Kaypro floppy convention: a file that is not there is
//! a drive that was never installed). Plus one device no Raytheon catalogue
//! offered: the invented 60 Hz line clock (`dev/ray703.rs` says invented at
//! length), silent until a program connects it.
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
use crate::dev::ray703::{LineClock703, TapeReader703, Tty703, DEV_LINE_CLOCK, DEV_TAPE_READER, DEV_TTY};
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

pub struct Ray703 {
    core: Memory,
    tty: Tty703,
    reader: TapeReader703,
    disc: Disc74601,
    line_clock: LineClock703,
}

impl Ray703 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        // The character devices signal level 0, which is what PTB and every
        // driver listing assume. They cannot be confused for each other
        // because a device stays silent until the program arms it, and a
        // program only ever arms the one it is reading from.
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
            tty: Tty703::new(console, 0),
            reader: TapeReader703::new(0),
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
                         or 'ray703-load' (add '-panel' for the front panel window)"
                    ),
                ));
            }
        }

        Ok(sys)
    }

    /// `--fast-io`: run the teletype and the disc at host speed instead of
    /// ten characters a second and half a revolution per transfer. The tape
    /// reader already free-runs.
    pub fn set_fast_io(&mut self) {
        self.tty.set_fast_io();
        self.disc.set_fast_io();
        // The line clock deliberately keeps its 60 Hz period: it is a time
        // base, not an I/O completion, and an "instant" timer is an
        // interrupt storm. This is what lets a test run --fast-io for an
        // instant teletype while scheduling slices stay real machine time.
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

impl Bus for Ray703 {
    fn read8(&mut self, addr: u32) -> u8 {
        self.core.read_byte(addr & 0xffff)
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.core.write_byte(addr & 0xffff, val);
    }

    /// The DIO address is a device nibble and a function nibble (4-2.1).
    /// Reading a device that isn't there yields zero, as the open input bus
    /// would.
    fn io_read16(&mut self, port: u8) -> u16 {
        let (device, function) = (port >> 4, port & 0x0f);
        match device {
            DEV_TTY => self.tty.din(function),
            DEV_TAPE_READER => self.reader.din(function),
            DEV_DISC => self.disc.din(function),
            _ => 0,
        }
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        let (device, function) = (port >> 4, port & 0x0f);
        match device {
            DEV_TTY => self.tty.dot(function, val),
            DEV_TAPE_READER => self.reader.dot(function, val),
            DEV_DISC => self.disc.dot(function, val),
            // no read arm anywhere for the clock: it has no readable
            // register, so its DIN is the open-bus default below on purpose
            DEV_LINE_CLOCK => self.line_clock.dot(function, val),
            _ => {}
        }
    }

    /// Every device gets the same elapsed cycle count, which is how they
    /// share one clock without sharing any state: the teletype spends it
    /// running its character at ten a second, the reader ignores it, the
    /// disc counts down its transfer in flight, and the line clock banks it
    /// toward the next tick. The disc alone is handed the core -- it is the
    /// machine's one DMA device, and this poll is the one place the memory
    /// and the devices meet.
    fn poll_interrupt_lines(&mut self, elapsed_cycles: u32) -> u16 {
        self.tty.poll(elapsed_cycles)
            | self.reader.poll(elapsed_cycles)
            | self.disc.poll(elapsed_cycles, &mut self.core)
            | self.line_clock.poll(elapsed_cycles)
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
}
