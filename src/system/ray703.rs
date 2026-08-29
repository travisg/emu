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
//! Raytheon 703 (1967).
//!
//! Not a port of anything -- the C++ tree never had this machine. It is a
//! fully populated 703: 32K words of core, a console teletype and a high speed
//! paper tape reader on the DIO channel.
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

use crate::bus::{Bus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::memory::Memory;
use crate::dev::ray703::{TapeReader703, Tty703, DEV_TAPE_READER, DEV_TTY};
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
/// `test/asm703.py --tape-origin` has to agree with this, since a tape's
/// contents are meaningless at any other address; it defaults to the same
/// number and says so.
const PTB_LOAD_ORIGIN: u16 = 0x0100;

pub struct Ray703 {
    core: Memory,
    tty: Tty703,
    reader: TapeReader703,
}

impl Ray703 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        // Both devices signal level 0, which is what PTB and every driver
        // listing assume. They cannot be confused for each other because a
        // device stays silent until the program arms it, and a program only
        // ever arms the one it is reading from.
        let mut sys = Ray703 {
            core: Memory::new(CORE_BYTES),
            tty: Tty703::new(console, 0),
            reader: TapeReader703::new(0),
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
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "unknown ray703 subsystem '{other}'; try 'ray703' or 'ray703-ptb' \
                         (add '-panel' for the front panel window)"
                    ),
                ));
            }
        }

        Ok(sys)
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
            _ => 0,
        }
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        let (device, function) = (port >> 4, port & 0x0f);
        match device {
            DEV_TTY => self.tty.dot(function, val),
            DEV_TAPE_READER => self.reader.dot(function, val),
            _ => {}
        }
    }

    fn poll_interrupt_lines(&mut self) -> u16 {
        self.tty.poll() | self.reader.poll()
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
        assert_eq!(sys.poll_interrupt_lines(), 0, "no tape is mounted");
        // and an absent device reads back zero rather than panicking
        assert_eq!(sys.io_read16(0x1d), 0);
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
}
