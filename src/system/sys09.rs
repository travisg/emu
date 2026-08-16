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
//! System09 (Motorola 6809).
//!
//! Port of `system/system09.cpp`. The only system that actually loads an Intel
//! HEX rom -- the other three have vestigial ihex scaffolding but read flat
//! binaries.

use crate::bus::{Bus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::mc6850::Mc6850;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/6809/BASIC.HEX";

/// Which peripheral layout the subsystem selects.
#[derive(Copy, Clone, PartialEq, Eq)]
enum MemoryLayout {
    /// MC6850 at 0xa000
    Standard,
    /// uart16550 at 0x8000 -- not yet ported
    Obc,
}

pub struct System09 {
    ram: Memory,
    rom: Memory,
    uart: Mc6850,
    layout: MemoryLayout,
}

impl System09 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        if subsystem == "obc" {
            // the uart16550 the C++ uses for this layout is a later phase
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "the 6809-obc subsystem needs the uart16550, which is not ported yet",
            ));
        }

        let mut sys = System09 {
            ram: Memory::new(32 * 1024),
            rom: Memory::new(16 * 1024),
            uart: Mc6850::new(console),
            layout: MemoryLayout::Standard,
        };

        // The hex records are written through the address decode, exactly as
        // the C++ callback does -- so a record aimed at ram lands in ram.
        let mut writes: Vec<(u32, Vec<u8>)> = Vec::new();
        rom::load_ihex(rom_path, |addr, bytes| writes.push((addr, bytes.to_vec())))?;
        for (addr, bytes) in writes {
            for (i, b) in bytes.iter().enumerate() {
                sys.write8(addr.wrapping_add(i as u32), *b);
            }
        }

        Ok(sys)
    }

    /// The address decode, shared by reads and writes.
    fn device_at(&mut self, addr: u16) -> Option<(&mut dyn MemoryDevice, u32)> {
        match addr {
            0x0000..=0x7fff => Some((&mut self.ram, addr as u32)),
            0x8000..=0x87ff if self.layout == MemoryLayout::Obc => {
                Some((&mut self.uart, (addr - 0x8000) as u32))
            }
            0xa000..=0xa7ff if self.layout == MemoryLayout::Standard => {
                Some((&mut self.uart, (addr - 0xa000) as u32))
            }
            0xc000..=0xffff => Some((&mut self.rom, (addr - 0xc000) as u32)),
            _ => None,
        }
    }
}

impl Bus for System09 {
    fn read8(&mut self, addr: u32) -> u8 {
        match self.device_at((addr & 0xffff) as u16) {
            Some((dev, a)) => dev.read_byte(a),
            None => 0,
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        if let Some((dev, a)) = self.device_at((addr & 0xffff) as u16) {
            dev.write_byte(a, val);
        }
    }
}
