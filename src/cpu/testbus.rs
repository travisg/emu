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
//! A minimal bus for the CPU core unit tests.
//!
//! Flat 64K of RAM plus an IO space, so a test can hand-assemble a handful of
//! bytes, run them through a core and assert on the result. The machines in
//! `system` are the real buses; this exists so a core can be exercised without
//! one, and so `cargo test` covers all three cores on a bare checkout -- the
//! trace-diff suites in `tests/` need both a `roms` symlink and a C++ oracle
//! binary, and skip themselves without them.

use super::{Cpu, StepResult};
use crate::bus::Bus;

pub(crate) struct TestBus {
    pub mem: Vec<u8>,
    /// Z80 port space. Mirrored every 256 ports; no core here puts anything
    /// meaningful in the high half of the port address.
    pub ports: [u8; 256],
    /// Every `io_write8` in order, so a test can assert on port traffic.
    pub io_writes: Vec<(u16, u8)>,
    /// 703 DIO space: 16 devices x 16 functions, addressed by the instruction's
    /// low byte.
    pub ports16: [u16; 256],
    /// Every `io_write16` in order, the wide equivalent of `io_writes`.
    pub io16_writes: Vec<(u8, u16)>,
    /// Pending interrupt-level pulses, consumed by the next
    /// `poll_interrupt_lines`. A test sets this to inject a signal.
    pub int_lines: u16,
    /// Running total of the cycle counts handed to `poll_interrupt_lines`, so
    /// a test can check that a core reports the machine time its devices are
    /// paced by.
    pub polled_cycles: u64,
    /// Address to count reads of. Some operations read their operand more than
    /// once, which is invisible in the result but observable on a device
    /// register -- see the 6800 `asr` fallthrough.
    pub watch: Option<u16>,
    pub watch_reads: u32,
}

impl TestBus {
    pub fn new() -> Self {
        TestBus {
            mem: vec![0; 0x10000],
            ports: [0; 256],
            io_writes: Vec::new(),
            ports16: [0; 256],
            io16_writes: Vec::new(),
            int_lines: 0,
            polled_cycles: 0,
            watch: None,
            watch_reads: 0,
        }
    }

    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        let start = addr as usize;
        self.mem[start..start + bytes.len()].copy_from_slice(bytes);
    }

    /// The 6800/6809 reset vector: big-endian at 0xfffe.
    pub fn set_reset_vector(&mut self, addr: u16) {
        self.load(0xfffe, &addr.to_be_bytes());
    }
}

impl Bus for TestBus {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = (addr & 0xffff) as u16;
        if self.watch == Some(addr) {
            self.watch_reads += 1;
        }
        self.mem[addr as usize]
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.mem[(addr & 0xffff) as usize] = val;
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        self.ports[(port & 0xff) as usize]
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        self.io_writes.push((port, val));
        self.ports[(port & 0xff) as usize] = val;
    }

    fn io_read16(&mut self, port: u8) -> u16 {
        self.ports16[port as usize]
    }

    fn io_write16(&mut self, port: u8, val: u16) {
        self.io16_writes.push((port, val));
        self.ports16[port as usize] = val;
    }

    fn poll_interrupt_lines(&mut self, elapsed_cycles: u32) -> u16 {
        self.polled_cycles += elapsed_cycles as u64;
        std::mem::take(&mut self.int_lines)
    }
}

/// Step `n` instructions, requiring every one of them to complete. A core that
/// stops early (bad opcode, halt) fails here rather than silently leaving the
/// registers short of what the test expects.
pub(crate) fn run_steps(cpu: &mut dyn Cpu, bus: &mut TestBus, n: usize) {
    for i in 0..n {
        assert_eq!(cpu.step(bus), StepResult::Ok, "step {i} did not complete");
    }
}
