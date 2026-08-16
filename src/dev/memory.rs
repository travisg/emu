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
//! A flat bank of memory.

use crate::bus::MemoryDevice;

/// A flat RAM/ROM bank.
///
/// ROM-ness is not a property of the bank: as in the C++, it's enforced by the
/// machine's address decode simply declining to route writes there.
///
/// Unlike the C++ `Memory`, accesses are masked to the bank size rather than
/// being unchecked. The C++ does no bounds checking at all, which is how the
/// RC2014 decode bug (`1e7005d`) turned into a heap overflow instead of a
/// visible wrap.
pub struct Memory {
    mem: Box<[u8]>,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Memory { mem: vec![0; size].into_boxed_slice() }
    }

    pub fn size(&self) -> usize {
        self.mem.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.mem
    }

    /// Bulk load, for flat-binary ROM images that bypass the byte-at-a-time
    /// device interface (as the C++ `fread` into `GetPtr()` does).
    pub fn load_at(&mut self, offset: usize, data: &[u8]) {
        let end = (offset + data.len()).min(self.mem.len());
        if offset < end {
            let n = end - offset;
            self.mem[offset..end].copy_from_slice(&data[..n]);
        }
    }

    fn index(&self, addr: u32) -> usize {
        (addr as usize) % self.mem.len()
    }
}

impl MemoryDevice for Memory {
    fn read_byte(&mut self, addr: u32) -> u8 {
        let i = self.index(addr);
        self.mem[i]
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        let i = self.index(addr);
        self.mem[i] = val;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_bytes() {
        let mut m = Memory::new(0x100);
        m.write_byte(0x10, 0xaa);
        assert_eq!(m.read_byte(0x10), 0xaa);
        assert_eq!(m.size(), 0x100);
    }

    #[test]
    fn starts_zeroed() {
        let mut m = Memory::new(0x40);
        assert!((0..0x40).all(|a| m.read_byte(a) == 0));
    }

    #[test]
    fn accesses_past_the_end_wrap_instead_of_overflowing() {
        // the C++ equivalent of this was an out-of-bounds heap access
        let mut m = Memory::new(0x100);
        m.write_byte(0x00, 0x42);
        assert_eq!(m.read_byte(0x100), 0x42);
        m.write_byte(0x101, 0x99);
        assert_eq!(m.read_byte(0x01), 0x99);
    }

    #[test]
    fn load_at_copies_and_clips() {
        let mut m = Memory::new(0x10);
        m.load_at(0x8, &[1, 2, 3, 4]);
        assert_eq!(m.read_byte(0x8), 1);
        assert_eq!(m.read_byte(0xb), 4);

        // an image longer than the remaining space is clipped, not a panic
        m.load_at(0xe, &[9, 9, 9, 9, 9, 9]);
        assert_eq!(m.read_byte(0xe), 9);
        assert_eq!(m.read_byte(0xf), 9);
    }
}
