// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! MITS Altair 680 (Motorola 6800).
//!
//! Port of `system/altair680.cpp`. The rom is a flat 256-byte binary; the
//! `iHexParseCallback` in the C++ file is dead code that is never wired up.

use crate::bus::{Bus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::mc6850::Mc6850;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/mits680b/mits680b.bin";
const MONITOR_ROM_SIZE: usize = 256;

pub struct Altair680 {
    ram: Memory,
    rom_monitor: Memory,
    rom_vtl: Memory,
    uart: Mc6850,
}

impl Altair680 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint) -> io::Result<Self> {
        let image = rom::load_binary(rom_path)?;
        if image.len() < MONITOR_ROM_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "monitor rom {} is {} bytes, expected at least {}",
                    rom_path.display(),
                    image.len(),
                    MONITOR_ROM_SIZE
                ),
            ));
        }

        let mut rom_monitor = Memory::new(MONITOR_ROM_SIZE);
        rom_monitor.load_at(0, &image[..MONITOR_ROM_SIZE]);

        Ok(Altair680 {
            ram: Memory::new(32 * 1024),
            rom_monitor,
            rom_vtl: Memory::new(768),
            uart: Mc6850::new(console),
        })
    }
}

impl Bus for Altair680 {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.read_byte(addr as u32),
            0xf000..=0xf001 => self.uart.read_byte((addr - 0xf000) as u32),
            0xfc00..=0xfeff => self.rom_vtl.read_byte((addr - 0xfc00) as u32),
            0xff00..=0xffff => self.rom_monitor.read_byte((addr - 0xff00) as u32),
            // unmapped: the C++ decode returns no device and the read yields 0
            _ => 0,
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.write_byte(addr as u32, val),
            0xf000..=0xf001 => self.uart.write_byte((addr - 0xf000) as u32, val),
            // The rom banks are writable through the decode in the C++ too --
            // rom-ness is not enforced there either.
            0xfc00..=0xfeff => self.rom_vtl.write_byte((addr - 0xfc00) as u32, val),
            0xff00..=0xffff => self.rom_monitor.write_byte((addr - 0xff00) as u32, val),
            _ => {}
        }
    }

    /// The banks only: the ACIA pulls a character on any register read.
    fn peek8(&self, addr: u32) -> Option<u8> {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => Some(self.ram.peek(addr as u32)),
            0xfc00..=0xfeff => Some(self.rom_vtl.peek((addr - 0xfc00) as u32)),
            0xff00..=0xffff => Some(self.rom_monitor.peek((addr - 0xff00) as u32)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn build() -> Altair680 {
        let path = std::env::temp_dir().join(format!("emu-altair680-{}.bin", std::process::id()));
        // monitor byte i = i, so reads are recognisable
        let image: Vec<u8> = (0..MONITOR_ROM_SIZE).map(|i| i as u8).collect();
        std::fs::write(&path, &image).unwrap();
        let (_tx, rx) = mpsc::channel();
        let machine = Altair680::new(&path, ConsoleEndpoint::new(rx, Box::new(Vec::new())));
        std::fs::remove_file(&path).ok();
        machine.unwrap()
    }

    /// The debugger's peek covers ram and both roms and declines the ACIA,
    /// whose every register read drains the console queue.
    #[test]
    fn peek_refuses_the_acia_and_reads_the_banks() {
        let mut m = build();
        m.write8(0x0100, 0x42);
        assert_eq!(m.peek8(0x0100), Some(0x42));
        assert_eq!(m.peek8(0xff10), Some(0x10), "monitor rom");
        assert_eq!(m.peek8(0xfc00), Some(0), "vtl rom, empty");
        assert_eq!(m.peek8(0xf000), None, "ACIA status");
        assert_eq!(m.peek8(0xf001), None, "ACIA data");
        assert_eq!(m.peek8(0x8000), None, "unmapped");
        m.poke8(0x0100, 0x99);
        assert_eq!(m.read8(0x0100), 0x99, "poke is the guest's own write");
    }
}
