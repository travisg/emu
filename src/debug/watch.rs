// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Write watchpoints: a `Bus` the run loop steps the cpu through while any
//! watch is set, which notes the first write into a watched range and
//! otherwise is the machine's bus.
//!
//! Every method is forwarded by hand, the trait's defaults included: a
//! default left standing here would replace the machine's own -- its
//! interrupt poll, its port decode -- only while a watch is set, which no
//! test without one would ever see. The wide accessors are forwarded whole
//! and checked over their span, since a machine may implement them itself.
//!
//! What the cpu writes is what is watched. A device that writes memory on
//! its own -- the 703's disc, the one DMA device -- does so inside its
//! machine and never crosses this bus.

use crate::bus::{Bus, Endian, IntStatus};
use crate::console::DiskImage;

/// A watched range of bus bytes, `len` from `addr`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Watch {
    pub addr: u32,
    pub len: u32,
}

impl Watch {
    fn covers(&self, addr: u32, len: u32) -> bool {
        addr < self.addr.saturating_add(self.len) && self.addr < addr.saturating_add(len)
    }
}

/// The first watched write of a step: where, and the byte that landed at
/// the first watched address it touched.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WatchHit {
    pub addr: u32,
    pub val: u8,
}

pub struct WatchBus<'a> {
    pub inner: &'a mut dyn Bus,
    pub watches: &'a [Watch],
    pub hit: Option<WatchHit>,
}

impl WatchBus<'_> {
    /// Note a write of `bytes` from `addr`, if it is the step's first into
    /// a watch.
    fn wrote(&mut self, addr: u32, bytes: &[u8]) {
        if self.hit.is_some() {
            return;
        }
        for (i, &val) in bytes.iter().enumerate() {
            let a = addr.wrapping_add(i as u32);
            if self.watches.iter().any(|w| w.covers(a, 1)) {
                self.hit = Some(WatchHit { addr: a, val });
                return;
            }
        }
    }
}

fn split16(val: u16, e: Endian) -> [u8; 2] {
    match e {
        Endian::Big => val.to_be_bytes(),
        Endian::Little => val.to_le_bytes(),
    }
}

impl Bus for WatchBus<'_> {
    fn read8(&mut self, addr: u32) -> u8 {
        self.inner.read8(addr)
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.inner.write8(addr, val);
        self.wrote(addr, &[val]);
    }

    fn peek8(&self, addr: u32) -> Option<u8> {
        self.inner.peek8(addr)
    }

    fn poke8(&mut self, addr: u32, val: u8) -> Result<(), String> {
        self.inner.poke8(addr, val)
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        self.inner.io_read8(port)
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        self.inner.io_write8(port, val)
    }

    fn io_read16(&mut self, port: u8) -> u16 {
        self.inner.io_read16(port)
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        self.inner.io_write16(port, val)
    }

    fn read16(&mut self, addr: u32, e: Endian) -> u16 {
        self.inner.read16(addr, e)
    }

    fn write16(&mut self, addr: u32, val: u16, e: Endian) {
        self.inner.write16(addr, val, e);
        self.wrote(addr, &split16(val, e));
    }

    fn read32(&mut self, addr: u32, e: Endian) -> u32 {
        self.inner.read32(addr, e)
    }

    fn write32(&mut self, addr: u32, val: u32, e: Endian) {
        self.inner.write32(addr, val, e);
        let bytes = match e {
            Endian::Big => val.to_be_bytes(),
            Endian::Little => val.to_le_bytes(),
        };
        self.wrote(addr, &bytes);
    }

    fn poll_interrupts(&mut self, elapsed_cycles: u32) -> IntStatus {
        self.inner.poll_interrupts(elapsed_cycles)
    }

    fn interrupt_acknowledge(&mut self) -> u8 {
        self.inner.interrupt_acknowledge()
    }

    fn interrupt_return(&mut self) {
        self.inner.interrupt_return()
    }

    fn poll_interrupt_lines(&mut self, elapsed_cycles: u32) -> u16 {
        self.inner.poll_interrupt_lines(elapsed_cycles)
    }

    fn set_device_pacing_hz(&mut self, hz: u64) {
        self.inner.set_device_pacing_hz(hz)
    }

    fn change_disk(&mut self, drive: usize, image: Option<DiskImage>) -> Result<(), String> {
        self.inner.change_disk(drive, image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bus that overrides everything, each method leaving a mark, so a
    /// method the wrapper failed to forward shows as a missing mark.
    #[derive(Default)]
    struct Marks {
        mem: Vec<u8>,
        marks: Vec<&'static str>,
    }

    impl Bus for Marks {
        fn read8(&mut self, addr: u32) -> u8 {
            self.marks.push("read8");
            self.mem[addr as usize]
        }
        fn write8(&mut self, addr: u32, val: u8) {
            self.marks.push("write8");
            self.mem[addr as usize] = val;
        }
        fn peek8(&self, addr: u32) -> Option<u8> {
            self.mem.get(addr as usize).copied()
        }
        fn poke8(&mut self, _addr: u32, _val: u8) -> Result<(), String> {
            self.marks.push("poke8");
            Ok(())
        }
        fn io_read8(&mut self, _port: u16) -> u8 {
            self.marks.push("io_read8");
            0x5a
        }
        fn io_write8(&mut self, _port: u16, _val: u8) {
            self.marks.push("io_write8");
        }
        fn io_read16(&mut self, _port: u8) -> u16 {
            self.marks.push("io_read16");
            0x1234
        }
        fn io_write16(&mut self, _port: u8, _val: u16) {
            self.marks.push("io_write16");
        }
        fn read16(&mut self, _addr: u32, _e: Endian) -> u16 {
            self.marks.push("read16");
            0xbeef
        }
        fn write16(&mut self, addr: u32, val: u16, e: Endian) {
            self.marks.push("write16");
            let [a, b] = split16(val, e);
            self.mem[addr as usize] = a;
            self.mem[addr as usize + 1] = b;
        }
        fn read32(&mut self, _addr: u32, _e: Endian) -> u32 {
            self.marks.push("read32");
            0
        }
        fn write32(&mut self, _addr: u32, _val: u32, _e: Endian) {
            self.marks.push("write32");
        }
        fn poll_interrupts(&mut self, _elapsed: u32) -> IntStatus {
            self.marks.push("poll_interrupts");
            IntStatus { irq: true, ..IntStatus::NONE }
        }
        fn interrupt_acknowledge(&mut self) -> u8 {
            self.marks.push("interrupt_acknowledge");
            0x42
        }
        fn interrupt_return(&mut self) {
            self.marks.push("interrupt_return");
        }
        fn poll_interrupt_lines(&mut self, _elapsed: u32) -> u16 {
            self.marks.push("poll_interrupt_lines");
            0x8000
        }
        fn set_device_pacing_hz(&mut self, _hz: u64) {
            self.marks.push("set_device_pacing_hz");
        }
        fn change_disk(&mut self, _drive: usize, _image: Option<DiskImage>) -> Result<(), String> {
            self.marks.push("change_disk");
            Ok(())
        }
    }

    #[test]
    fn every_method_reaches_the_machine() {
        let mut inner = Marks { mem: vec![0; 16], ..Default::default() };
        let watches = [Watch { addr: 0x100, len: 1 }];
        let mut w = WatchBus { inner: &mut inner, watches: &watches, hit: None };
        w.read8(0);
        w.write8(0, 1);
        assert_eq!(w.peek8(0), Some(1));
        w.poke8(0, 0).unwrap();
        assert_eq!(w.io_read8(0), 0x5a);
        w.io_write8(0, 0);
        assert_eq!(w.io_read16(0), 0x1234);
        w.io_write16(0, 0);
        assert_eq!(w.read16(0, Endian::Big), 0xbeef);
        w.write16(0, 0, Endian::Big);
        w.read32(0, Endian::Big);
        w.write32(0, 0, Endian::Big);
        assert!(w.poll_interrupts(1).irq);
        assert_eq!(w.interrupt_acknowledge(), 0x42);
        w.interrupt_return();
        assert_eq!(w.poll_interrupt_lines(1), 0x8000);
        w.set_device_pacing_hz(1);
        w.change_disk(0, None).unwrap();
        assert_eq!(w.hit, None);
        assert_eq!(
            inner.marks,
            [
                "read8", "write8", "poke8", "io_read8", "io_write8", "io_read16", "io_write16", "read16", "write16",
                "read32", "write32", "poll_interrupts", "interrupt_acknowledge", "interrupt_return",
                "poll_interrupt_lines", "set_device_pacing_hz", "change_disk",
            ]
        );
    }

    #[test]
    fn the_first_write_into_a_watch_is_the_hit() {
        let mut inner = Marks { mem: vec![0; 16], ..Default::default() };
        let watches = [Watch { addr: 4, len: 2 }];
        let mut w = WatchBus { inner: &mut inner, watches: &watches, hit: None };
        w.write8(3, 9);
        assert_eq!(w.hit, None, "next to it");
        w.write16(3, 0x1122, Endian::Big);
        assert_eq!(w.hit, Some(WatchHit { addr: 4, val: 0x22 }), "a word that straddles the start");
        w.write8(5, 7);
        assert_eq!(w.hit, Some(WatchHit { addr: 4, val: 0x22 }), "the first one stays");
        assert_eq!(inner.mem[5], 7, "and every write still lands");
    }
}
