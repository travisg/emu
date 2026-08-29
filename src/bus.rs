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
//! The bus a CPU talks to, and the devices hanging off it.
//!
//! This is the half of the C++ `System` class that the CPU actually needs: the
//! address decode and the memory/IO primitives. Construction, ROM loading and
//! metadata live in `system` instead. Splitting them is what breaks the C++
//! ownership cycle (`System` owns `Cpu` by `unique_ptr` while `Cpu` holds a
//! `System&` back-reference): here the CPU holds nothing, and the bus is passed
//! into each `step()`.

/// Byte order for a multi-byte bus access. The CPU is the authority: 6800 and
/// 6809 are big-endian, the z80 is little-endian.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

/// Interrupt lines as seen by the CPU. The lines live in the machine (which
/// knows what's wired to them); how they're serviced lives in the CPU.
///
/// Scaffolding for the ported cores: no C++ core has a working interrupt-
/// injection path (the z80's `RaiseIRQ` is never called, NMI is never read,
/// IM0/IM2 are stubs, and the 6800/6809 exception bitmasks are never set beyond
/// reset), so none of *this* struct is trace-validatable against the oracle.
/// The Raytheon 703 does run interrupts for real, but its 16 prioritized levels
/// don't fit an irq/nmi pair -- see `poll_interrupt_lines`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct IntStatus {
    pub irq: bool,
    pub nmi: bool,
    /// vector supplied by the device, for z80 IM2
    pub vector: u8,
}

impl IntStatus {
    pub const NONE: IntStatus = IntStatus { irq: false, nmi: false, vector: 0 };
}

/// A device mapped into the memory space.
///
/// Mirrors the C++ `MemoryDevice` (`dev/memory.h`). `read_byte` takes `&mut
/// self` because reads genuinely mutate: the MC6850 and uart16550 pull a
/// character from the console and clear pending-rx state on read.
///
/// Note this covers only memory-mapped devices. Port-mapped devices (z80 SIO,
/// WD1793) are concrete structs a machine drives from `io_read8`/`io_write8`;
/// they are not `MemoryDevice` in the C++ either.
pub trait MemoryDevice {
    fn read_byte(&mut self, addr: u32) -> u8;
    fn write_byte(&mut self, addr: u32, val: u8);
}

/// The bus. A machine implements this and does all its own address decoding.
///
/// Address is `u32` so the same trait covers 8/16/24/32-bit address spaces.
/// Data width is expressed by *which* accessor a machine overrides: an
/// 8-bit-data machine implements only `read8`/`write8` and inherits the
/// composed wide accessors below, which generalize the C++
/// `System::MemRead16`.
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn write8(&mut self, addr: u32, val: u8);

    /// Port IO, z80 systems only. Machines without a separate IO space inherit
    /// these no-ops.
    fn io_read8(&mut self, _port: u16) -> u8 {
        0
    }
    fn io_write8(&mut self, _port: u16, _val: u8) {}

    /// 16-bit port IO, for the Raytheon 703's DIO channel: `DIN`/`DOT` move a
    /// whole word between ACR and the addressed device in one operation, so
    /// this is a distinct primitive rather than two `io_read8`s. The port is
    /// the instruction's low byte, a 4-bit device number and a 4-bit function
    /// code. An absent device reads zero, which is what the hardware does.
    fn io_read16(&mut self, _port: u8) -> u16 {
        0
    }
    fn io_write16(&mut self, _port: u8, _val: u16) {}

    fn read16(&mut self, addr: u32, e: Endian) -> u16 {
        let a = self.read8(addr) as u16;
        let b = self.read8(addr.wrapping_add(1)) as u16;
        match e {
            Endian::Big => (a << 8) | b,
            Endian::Little => (b << 8) | a,
        }
    }

    fn write16(&mut self, addr: u32, val: u16, e: Endian) {
        let (first, second) = match e {
            Endian::Big => ((val >> 8) as u8, val as u8),
            Endian::Little => (val as u8, (val >> 8) as u8),
        };
        self.write8(addr, first);
        self.write8(addr.wrapping_add(1), second);
    }

    fn read32(&mut self, addr: u32, e: Endian) -> u32 {
        let a = self.read16(addr, e) as u32;
        let b = self.read16(addr.wrapping_add(2), e) as u32;
        match e {
            Endian::Big => (a << 16) | b,
            Endian::Little => (b << 16) | a,
        }
    }

    fn write32(&mut self, addr: u32, val: u32, e: Endian) {
        let (first, second) = match e {
            Endian::Big => ((val >> 16) as u16, val as u16),
            Endian::Little => (val as u16, (val >> 16) as u16),
        };
        self.write16(addr, first, e);
        self.write16(addr.wrapping_add(2), second, e);
    }

    /// Interrupt lines currently asserted. Default: nothing is wired up.
    fn poll_interrupts(&self) -> IntStatus {
        IntStatus::NONE
    }

    /// Prioritized interrupt lines that have *pulsed* since the last call, one
    /// bit per level (bit n = level n). Take-and-clear: a machine returns each
    /// pulse exactly once, and the core latches it into its own per-level
    /// state.
    ///
    /// Separate from `poll_interrupts` for three reasons. The 703's lines are
    /// momentary signals, not the level-held wires `IntStatus` models; a level
    /// whose signal arrives while it is disabled is dropped on the floor rather
    /// than remembered, so "still asserted?" is not a question the machine can
    /// answer. And this takes `&mut self` because polling genuinely mutates --
    /// the console device drains `ConsoleEndpoint::try_next_char` here.
    ///
    /// `elapsed_cycles` is how many clock cycles the machine ran since the last
    /// call, which is the only time base a device on this bus gets: nothing
    /// here reads a wall clock, so a device that takes time -- a teletype at
    /// ten characters a second -- counts the machine's cycles instead. Virtual
    /// time is the right base rather than an expedient one, because
    /// `--throttle` turns it into real time for the whole machine at once.
    fn poll_interrupt_lines(&mut self, _elapsed_cycles: u32) -> u16 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 64K bus, enough to exercise the composed wide accessors.
    struct TestBus {
        mem: Vec<u8>,
    }

    impl TestBus {
        fn new() -> Self {
            TestBus { mem: vec![0; 0x10000] }
        }
    }

    impl Bus for TestBus {
        fn read8(&mut self, addr: u32) -> u8 {
            self.mem[(addr & 0xffff) as usize]
        }
        fn write8(&mut self, addr: u32, val: u8) {
            self.mem[(addr & 0xffff) as usize] = val;
        }
    }

    #[test]
    fn read16_composes_by_endian() {
        let mut bus = TestBus::new();
        bus.write8(0x100, 0xde);
        bus.write8(0x101, 0xad);
        assert_eq!(bus.read16(0x100, Endian::Big), 0xdead);
        assert_eq!(bus.read16(0x100, Endian::Little), 0xadde);
    }

    #[test]
    fn write16_splits_by_endian() {
        let mut bus = TestBus::new();
        bus.write16(0x200, 0xbeef, Endian::Big);
        assert_eq!((bus.read8(0x200), bus.read8(0x201)), (0xbe, 0xef));
        bus.write16(0x200, 0xbeef, Endian::Little);
        assert_eq!((bus.read8(0x200), bus.read8(0x201)), (0xef, 0xbe));
    }

    #[test]
    fn wide_accesses_round_trip() {
        let mut bus = TestBus::new();
        for e in [Endian::Big, Endian::Little] {
            bus.write16(0x300, 0x1234, e);
            assert_eq!(bus.read16(0x300, e), 0x1234);
            bus.write32(0x400, 0x89abcdef, e);
            assert_eq!(bus.read32(0x400, e), 0x89abcdef);
        }
    }

    #[test]
    fn wide_access_wraps_at_the_top_of_the_space() {
        // the composed accessors must wrap rather than panic
        let mut bus = TestBus::new();
        bus.write16(0xffff, 0x1234, Endian::Big);
        assert_eq!(bus.read8(0xffff), 0x12);
        assert_eq!(bus.read8(0x0000), 0x34);
    }
}
