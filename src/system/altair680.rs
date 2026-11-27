// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! MITS Altair 680b (Motorola 6800).
//!
//! The machine MITS's documents describe, from a port of
//! `system/altair680.cpp`: 32K of RAM at 0, the ACIA at `0xf000`, the
//! board's hardware programmable bits at `0xf002`, VTL-2 in the
//! three-PROM socket at `0xfc00` and the 256-byte PROM monitor at
//! `0xff00`. The proms are read-only.
//!
//! - **The straps** (Operator's Manual I.C): jumpers read back at `0xf002`.
//!   The monitor's reset code reads bit 7 as "no terminal", and jumps to 0
//!   instead of starting the monitor, and bit 2 as one stop bit rather than
//!   two (`MON680.LST` 0215-0219). The `noterm` subsystem sets bit 7: the
//!   machine is programmed from its front panel, and RESET then RUN starts
//!   whatever was deposited at 0 (Operator's Manual V.8). Otherwise the
//!   jumpers are the manual's first example: a terminal, two stop bits, no
//!   Baudot.
//! - **VTL-2** is `roms/mits680b/vtl2.bin` when that is there, an empty
//!   socket when not; `J FC00` at the monitor starts it, and its manual's
//!   `*=1024*32` and `&=264` set its memory and empty its program.
//! - **`basic`** stands in for the operator loading Altair 680 BASIC off
//!   paper tape: `L` at the monitor's prompt, the tape through the reader
//!   on the terminal's line, and `J0000` once it is in. Those keys and the
//!   tape's bytes reach the ACIA as one stream, ahead of anything typed and
//!   as fast as the guest takes them; the keyboard has the line once the
//!   stream has run out. `-r` is the tape, in the monitor's S1 format; the
//!   monitor is always the default one.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::mc6850::Mc6850;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/mits680b/mits680b.bin";
/// VTL-2 for the socket at `0xfc00`, read if it is there.
pub const VTL_ROM: &str = "roms/mits680b/vtl2.bin";
/// The BASIC tape `basic` reads when `-r` names no other.
pub const BASIC_TAPE: &str = "roms/mits680b/basic680.s19";
const MONITOR_ROM_SIZE: usize = 256;
const VTL_ROM_SIZE: usize = 768;

/// `0xf002`'s bit 7, jumper 5 high: no terminal.
pub const STRAP_NO_TERMINAL: u8 = 0x80;

/// What the operator types around the tape: the monitor's load command
/// before it, and the jump to BASIC's cold start after.
const LOAD_COMMAND: &[u8] = b"L";
const START_BASIC: &[u8] = b"J0000";

/// The reader's stream: its bytes and how far it has been read.
struct Tape {
    bytes: Vec<u8>,
    pos: usize,
}

pub struct Altair680 {
    ram: Memory,
    rom_monitor: Memory,
    /// The VTL-2 socket, empty without its proms.
    rom_vtl: Option<Memory>,
    uart: Mc6850,
    straps: u8,
    tape: Option<Tape>,
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl Altair680 {
    /// `subsystem` is the machine's own tokens, the windows taken out:
    /// "", "basic" or "noterm".
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        let (straps, basic) = match subsystem {
            "" => (0, false),
            "noterm" => (STRAP_NO_TERMINAL, false),
            "basic" => (0, true),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "unknown altair680 subsystem '{other}'; try 'altair680', 'altair680-basic' \
                         or 'altair680-noterm'"
                    ),
                ))
            }
        };

        // with `basic`, -r is the tape; given no -r, main hands over the
        // monitor's own path, and the tape is the default one
        let (monitor_path, tape_path) = match basic {
            false => (rom_path, None),
            true if rom_path == Path::new(DEFAULT_ROM) => (Path::new(DEFAULT_ROM), Some(Path::new(BASIC_TAPE))),
            true => (Path::new(DEFAULT_ROM), Some(rom_path)),
        };
        let monitor = rom::load_binary(monitor_path)?;
        // an empty socket when the proms are not there
        let vtl = std::fs::read(VTL_ROM).ok();
        let tape = tape_path.map(rom::load_binary).transpose()?;
        Self::from_images(&monitor, vtl.as_deref(), tape, straps, console).map_err(invalid)
    }

    /// The machine from its images: the monitor prom's (the first 256
    /// bytes), VTL-2's if its socket is filled, and for `basic` the tape,
    /// which the operator's `L` and `J0000` go around.
    fn from_images(
        monitor: &[u8],
        vtl: Option<&[u8]>,
        tape: Option<Vec<u8>>,
        straps: u8,
        console: ConsoleEndpoint,
    ) -> Result<Self, String> {
        if monitor.len() < MONITOR_ROM_SIZE {
            return Err(format!("the monitor rom is {} bytes, expected at least {MONITOR_ROM_SIZE}", monitor.len()));
        }
        let mut rom_monitor = Memory::new(MONITOR_ROM_SIZE);
        rom_monitor.load_at(0, &monitor[..MONITOR_ROM_SIZE]);

        let rom_vtl = match vtl {
            Some(vtl) if vtl.len() == VTL_ROM_SIZE => {
                let mut m = Memory::new(VTL_ROM_SIZE);
                m.load_at(0, vtl);
                Some(m)
            }
            Some(vtl) => return Err(format!("{VTL_ROM} is {} bytes, expected {VTL_ROM_SIZE}", vtl.len())),
            None => None,
        };

        let tape = tape.map(|t| {
            let mut bytes = LOAD_COMMAND.to_vec();
            bytes.extend(t);
            bytes.extend(START_BASIC);
            Tape { bytes, pos: 0 }
        });

        Ok(Altair680 { ram: Memory::new(32 * 1024), rom_monitor, rom_vtl, uart: Mc6850::new(console), straps, tape })
    }

    /// The ACIA's next character: off the tape while there is any, then
    /// from the keyboard.
    fn poll_console(&mut self) {
        if let Some(tape) = &mut self.tape {
            if tape.pos < tape.bytes.len() {
                if self.uart.can_receive() {
                    self.uart.receive(tape.bytes[tape.pos]);
                    tape.pos += 1;
                }
                return;
            }
        }
        self.uart.poll();
    }
}

impl Bus for Altair680 {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.read_byte(addr as u32),
            0xf000..=0xf001 => self.uart.read_byte((addr - 0xf000) as u32),
            0xf002 => self.straps,
            0xfc00..=0xfeff => self.rom_vtl.as_mut().map_or(0, |m| m.read_byte((addr - 0xfc00) as u32)),
            0xff00..=0xffff => self.rom_monitor.read_byte((addr - 0xff00) as u32),
            // unmapped: nothing answers, and the read yields 0
            _ => 0,
        }
    }

    /// The proms and the straps take no writes.
    fn write8(&mut self, addr: u32, val: u8) {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.write_byte(addr as u32, val),
            0xf000..=0xf001 => self.uart.write_byte((addr - 0xf000) as u32, val),
            _ => {}
        }
    }

    /// A poke the machine would drop is refused: into a prom, the straps,
    /// or where nothing is decoded.
    fn poke8(&mut self, addr: u32, val: u8) -> Result<(), String> {
        let a = (addr & 0xffff) as u16;
        match a {
            0x0000..=0x7fff | 0xf000..=0xf001 => {
                self.write8(addr, val);
                Ok(())
            }
            0xfc00..=0xfeff if self.rom_vtl.is_some() => Err(format!("rom at {a:#06x}")),
            0xff00..=0xffff => Err(format!("rom at {a:#06x}")),
            0xf002 => Err(format!("no memory at {a:#06x}: the straps are jumpers")),
            _ => Err(format!("no memory at {a:#06x}")),
        }
    }

    /// The ACIA takes its next character here, and drives IRQ: "The IRQ
    /// output is tied directly to the MPU's IRQ input" (680b Theory of
    /// Operation, sheet 2). The monitor enables the receive interrupt but
    /// never clears I, so the line only reaches a program that unmasks it,
    /// through the IRQ vector's pointer to 0x0100 in RAM.
    fn poll_interrupts(&mut self, _elapsed_cycles: u32) -> IntStatus {
        self.poll_console();
        IntStatus { irq: self.uart.irq(), ..IntStatus::NONE }
    }

    /// The tape is saved whole with how far it has been read, so a restore
    /// carries on mid-load with the tape it was taken with.
    fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        let Altair680 { ram, rom_monitor, rom_vtl, uart, straps: _fixed_by_the_subsystem, tape } = self;
        w.section(b"RAM ", |w| ram.save(w));
        w.section(b"MON ", |w| rom_monitor.save(w));
        w.section(b"VTL ", |w| w.opt(rom_vtl, |w, m| m.save(w)));
        w.section(b"ACIA", |w| uart.save(w));
        w.section(b"TAPE", |w| {
            w.opt(tape, |w, t| {
                w.bytes(&t.bytes);
                w.u32(t.pos as u32);
            })
        });
        Ok(())
    }

    fn load_state(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        r.section(b"RAM ", |r| self.ram.load(r))?;
        r.section(b"MON ", |r| self.rom_monitor.load(r))?;
        self.rom_vtl = r.section(b"VTL ", |r| {
            r.opt(|r| {
                let mut m = Memory::new(VTL_ROM_SIZE);
                m.load(r)?;
                Ok(m)
            })
        })?;
        r.section(b"ACIA", |r| self.uart.load(r))?;
        self.tape = r.section(b"TAPE", |r| {
            r.opt(|r| {
                let bytes = r.bytes()?.to_vec();
                let pos = r.u32()? as usize;
                if pos > bytes.len() {
                    return Err(format!("the tape is read to {pos} of {}", bytes.len()));
                }
                Ok(Tape { bytes, pos })
            })
        })?;
        Ok(())
    }

    fn peek8(&self, addr: u32) -> Option<u8> {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => Some(self.ram.peek(addr as u32)),
            0xf002 => Some(self.straps),
            0xfc00..=0xfeff => self.rom_vtl.as_ref().map(|m| m.peek((addr - 0xfc00) as u32)),
            0xff00..=0xffff => Some(self.rom_monitor.peek((addr - 0xff00) as u32)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::m6800::Cpu6800;
    use crate::cpu::Cpu;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    /// Output the console wrote, shared with the test.
    #[derive(Clone, Default)]
    struct Out(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Out {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn console() -> ConsoleEndpoint {
        let (_tx, rx) = mpsc::channel();
        ConsoleEndpoint::new(rx, Box::new(Vec::new()))
    }

    /// Monitor byte i = i, so reads are recognisable; VTL-2's socket as
    /// given, `0xa5` throughout when filled.
    fn build_with(vtl: bool, straps: u8) -> Altair680 {
        let monitor: Vec<u8> = (0..MONITOR_ROM_SIZE).map(|i| i as u8).collect();
        let vtl = vtl.then(|| vec![0xa5; VTL_ROM_SIZE]);
        Altair680::from_images(&monitor, vtl.as_deref(), None, straps, console()).unwrap()
    }

    fn build() -> Altair680 {
        build_with(false, 0)
    }

    /// A debugger write above the ram, between the ACIA and the proms, or
    /// into a prom says so; one to the ram or to the ACIA goes through.
    #[test]
    fn a_poke_the_machine_would_drop_is_refused() {
        let mut m = build();
        for hole in [0x8000, 0xefff, 0xf003, 0xfbff] {
            assert!(m.poke8(hole, 0x55).unwrap_err().contains("no memory at"), "{hole:#06x}");
        }
        assert!(m.poke8(0xf002, 0x55).unwrap_err().contains("straps"));
        assert!(m.poke8(0xff10, 0x55).unwrap_err().contains("rom at 0xff10"));
        m.poke8(0x7fff, 0x55).unwrap();
        assert_eq!(m.read8(0x7fff), 0x55);
        m.poke8(0xf000, 0x03).unwrap();
    }

    /// The monitor is a prom: a guest's write lands nowhere.
    #[test]
    fn the_proms_take_no_writes() {
        let mut m = build();
        m.write8(0xff10, 0x99);
        assert_eq!(m.read8(0xff10), 0x10);
        m.write8(0xf002, 0x99);
        assert_eq!(m.read8(0xf002), 0);
    }

    /// The debugger's peek covers ram, the proms and the straps and
    /// declines the ACIA, whose data register gives up its byte when read.
    #[test]
    fn peek_refuses_the_acia_and_reads_the_banks() {
        let mut m = build();
        m.write8(0x0100, 0x42);
        assert_eq!(m.peek8(0x0100), Some(0x42));
        assert_eq!(m.peek8(0xff10), Some(0x10), "monitor rom");
        assert_eq!(m.peek8(0xf000), None, "ACIA status");
        assert_eq!(m.peek8(0xf001), None, "ACIA data");
        assert_eq!(m.peek8(0xf002), Some(0), "straps");
        assert_eq!(m.peek8(0x8000), None, "unmapped");
        m.poke8(0x0100, 0x99).unwrap();
        assert_eq!(m.read8(0x0100), 0x99, "poke is the guest's own write");
    }

    /// The straps read back at 0xf002: the plain machine's, a terminal
    /// and two stop bits, and `noterm`'s bit 7, which the monitor's reset
    /// takes as no terminal. The subsystem is checked before any file is
    /// read.
    #[test]
    fn the_straps_read_back() {
        assert_eq!(build().read8(0xf002), 0);
        assert_eq!(build_with(false, STRAP_NO_TERMINAL).read8(0xf002), STRAP_NO_TERMINAL);
        let err = Altair680::new(Path::new("nowhere"), console(), "panel").err().unwrap();
        assert!(err.to_string().contains("unknown altair680 subsystem 'panel'"));
    }

    /// VTL-2's socket filled is a prom like the monitor; empty, nothing is
    /// decoded there.
    #[test]
    fn the_vtl_socket_is_a_prom_or_empty() {
        let mut filled = build_with(true, 0);
        assert_eq!((filled.read8(0xfc00), filled.peek8(0xfeff)), (0xa5, Some(0xa5)));
        filled.write8(0xfc10, 0x11);
        assert_eq!(filled.read8(0xfc10), 0xa5, "a prom takes no writes");
        assert!(filled.poke8(0xfc10, 0x11).unwrap_err().contains("rom at 0xfc10"));
        let mut empty = build();
        assert_eq!((empty.read8(0xfc00), empty.peek8(0xfc00)), (0, None));
        assert!(empty.poke8(0xfc00, 0x11).unwrap_err().contains("no memory at 0xfc00"));
        let short = Altair680::from_images(&[0; MONITOR_ROM_SIZE], Some(&[0; 10]), None, 0, console());
        assert!(short.err().unwrap().contains("is 10 bytes"));
    }

    /// The monitor's own load and jump, run against a tape: the stream is
    /// `L`, the tape and `J0000`, taken only as the ACIA empties, so none
    /// of it is lost to the master reset the monitor starts with; the
    /// records land, and the program they load runs. The monitor here is
    /// the one in `MON680.LST`, keyed in from its object column.
    #[test]
    fn basic_loads_a_tape_through_the_monitor_and_runs_it() {
        // a program at 0 that prints "OK" and waits
        let prog = [0xc6, b'O', 0xf7, 0xf0, 0x01, 0xc6, b'K', 0xf7, 0xf0, 0x01, 0x20, 0xfe];
        let mut rec = vec![(prog.len() + 3) as u8, 0x00, 0x00];
        rec.extend_from_slice(&prog);
        let sum = rec.iter().fold(0u8, |s, b| s.wrapping_add(*b));
        rec.push(!sum);
        let hex: String = rec.iter().map(|b| format!("{b:02X}")).collect();
        let tape = format!("S0 leader\r\nS1{hex}\r\nS9\r\n");
        let out = Out::default();
        let (_tx, rx) = mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(out.clone()));
        let mut m = Altair680::from_images(&MON680, None, Some(tape.into_bytes()), 0, console).unwrap();
        let mut cpu = Cpu6800::new();
        cpu.reset(&mut m);
        for _ in 0..200_000 {
            if cpu.step(&mut m) != crate::cpu::StepResult::Ok {
                break;
            }
        }
        let out = String::from_utf8_lossy(&out.0.lock().unwrap()).into_owned();
        assert!(out.ends_with("\r\n.J 0000OK"), "{out:?}");
        assert!(out.starts_with("\r\n.L S0"), "the load command, then the tape echoed: {out:?}");
        assert_eq!(m.tape.as_ref().map(|t| t.pos), Some(m.tape.as_ref().unwrap().bytes.len()));
    }

    /// The ACIA version of the 680b PROM monitor, `MON680.LST`'s object
    /// code, 0xff00-0xffff.
    const MON680: [u8; 256] = [
        0x8d, 0x22, 0x24, 0xfc, 0xc6, 0x7f, 0xd1, 0xf3, 0xf4, 0xf0, 0x01, 0x24, 0x74, 0x39, 0x01, 0x8d, //
        0xef, 0xc0, 0x30, 0x2b, 0x3c, 0xc1, 0x09, 0x2f, 0x0a, 0xc1, 0x11, 0x2b, 0x34, 0xc1, 0x16, 0x2e, //
        0x30, 0xc0, 0x07, 0x39, 0xf6, 0xf0, 0x00, 0x57, 0x39, 0x8d, 0xd5, 0xc0, 0x53, 0x26, 0xfa, 0x8d, //
        0xcf, 0xc1, 0x39, 0x27, 0x1c, 0xc1, 0x31, 0x26, 0xf0, 0x4f, 0x8d, 0x17, 0xc0, 0x02, 0xd7, 0xf9, //
        0x8d, 0x20, 0x8d, 0x0f, 0x7a, 0x00, 0xf9, 0x27, 0x05, 0xe7, 0x00, 0x08, 0x20, 0xf4, 0x4c, 0x27, //
        0xd8, 0x20, 0x58, 0x8d, 0xba, 0x58, 0x58, 0x58, 0x58, 0x1b, 0xd7, 0xf8, 0x8d, 0xb1, 0x1b, 0xdb, //
        0xf8, 0x39, 0x8d, 0xef, 0xd7, 0xfa, 0x8d, 0xeb, 0xd7, 0xfb, 0xde, 0xfa, 0x39, 0x16, 0x54, 0x54, //
        0x54, 0x54, 0x8d, 0x01, 0x16, 0xc4, 0x0f, 0xcb, 0x30, 0xc1, 0x39, 0x23, 0x04, 0xcb, 0x07, 0x01, //
        0x01, 0x8c, 0xc6, 0x20, 0x37, 0x8d, 0x9d, 0x57, 0x24, 0xfb, 0x33, 0xf7, 0xf0, 0x01, 0x39, 0xde, //
        0xfa, 0x08, 0xdf, 0xfa, 0x96, 0xfa, 0x8d, 0xd5, 0x96, 0xfb, 0x8d, 0xd1, 0x8c, 0x8d, 0xc3, 0x8d, //
        0xe1, 0xa6, 0x00, 0x8d, 0xc8, 0x8d, 0xdb, 0x8d, 0xaa, 0xe7, 0x00, 0x9e, 0xf6, 0xc6, 0x0d, 0x8d, //
        0xd0, 0xc6, 0x0a, 0x8d, 0xcc, 0xc6, 0x2e, 0x8d, 0xc8, 0xbd, 0xff, 0x00, 0x17, 0x8d, 0xc3, 0x81, //
        0x4c, 0x27, 0x8c, 0x81, 0x4a, 0x26, 0x04, 0x8d, 0x99, 0x6e, 0x00, 0x81, 0x4d, 0x27, 0xce, 0x81, //
        0x4e, 0x27, 0xbc, 0x81, 0x50, 0x26, 0xd4, 0x3b, 0x8e, 0x00, 0xf3, 0xc6, 0x03, 0x37, 0x37, 0xf7, //
        0xf0, 0x00, 0xf6, 0xf0, 0x02, 0x2b, 0x19, 0xc4, 0x04, 0xca, 0xd1, 0xf7, 0xf0, 0x00, 0x9f, 0xf6, //
        0x9f, 0xfa, 0xd6, 0xf2, 0x2b, 0x0a, 0x20, 0xb3, 0x01, 0x00, 0xff, 0xee, 0x01, 0x04, 0xff, 0xd8, //
    ];
}
