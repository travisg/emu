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
//! CPU cores.

use crate::bus::Bus;
use std::io::Write;

pub mod m6800;
pub mod m6809;
pub mod ray703;
pub mod z80;

#[cfg(test)]
mod testbus;

/// Why a `step()` stopped, mapping onto the C++ cores' `Run()` return codes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StepResult {
    /// instruction executed, keep going
    Ok,
    /// executed a halt/wait instruction
    Halted,
    /// hit an opcode the core doesn't implement
    BadOpcode,
    /// branch-to-self with interrupts off: nothing can ever change
    InfiniteLoop,
}

/// An interpreter core.
///
/// Unlike the C++ `Cpu`, which has no single-instruction entry point at all
/// (each core's `Run()` *is* the whole cycle-limited loop, with the per-
/// instruction body inlined into it), this trait factors out exactly one
/// instruction. The loop, cycle limit and shutdown check live in `Emulator`.
pub trait Cpu {
    fn reset(&mut self, bus: &mut dyn Bus);

    /// Execute exactly one instruction.
    fn step(&mut self, bus: &mut dyn Bus) -> StepResult;

    /// Human-readable register dump, for debugging.
    fn dump(&self);

    /// One line of golden-trace state for the instruction *about to* execute.
    ///
    /// Must match the C++ `--trace` format byte for byte -- see
    /// `trace_oracle.h`. In particular it must log PC plus register state and
    /// **not** the opcode: peeking the opcode would consume a byte whenever PC
    /// sits on a device register, so a traced run would diverge from an
    /// untraced one.
    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()>;
}
