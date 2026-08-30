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
}
