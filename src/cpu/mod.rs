// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! CPU cores.

use crate::bus::{Bus, Endian};
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

/// One register as the debugger sees it: its name, its value, and how wide
/// it is -- a flag is one bit, the 703's EXR five, a Z80 pair sixteen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Register {
    pub name: &'static str,
    pub value: u32,
    pub bits: u8,
}

impl Register {
    pub fn new(name: &'static str, value: impl Into<u32>, bits: u8) -> Self {
        Register { name, value: value.into(), bits }
    }

    /// The value in the width the trace uses: hex digits to cover `bits`,
    /// a bare `0`/`1` for a flag.
    pub fn hex(&self) -> String {
        format!("{:0width$x}", self.value, width = (self.bits as usize).div_ceil(4))
    }
}

/// How the debugger presents this core's memory: the byte order of a 16-bit
/// word, and how many bus bytes one unit of the core's own address space
/// spans -- two on the word-addressed 703, one everywhere else.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Addressing {
    pub endian: Endian,
    pub unit_bytes: u32,
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

    /// Clock cycles consumed by the most recent `step()`.
    ///
    /// 0 means this core does not count cycles, which renders throttling
    /// inert (the run loop warns once and runs uncapped). Cores that do
    /// count keep the tally internal and override this; nothing about it
    /// may influence `trace_line()`, for the reason given there -- a traced
    /// run and an untraced one must execute identically.
    fn last_step_cycles(&self) -> u32 {
        0
    }

    /// Apply one front panel data-entry actuation (a register bit toggle,
    /// a clear, an ENTER/DISPLAY memory access). Only a core with a
    /// physical panel overrides this; the run-state switches (RUN, HALT,
    /// SINGLE COMMAND, RESET) are the run loop's business and never
    /// arrive here.
    fn panel_command(&mut self, _bus: &mut dyn Bus, _cmd: &crate::console::PanelCommand) {}

    /// The address of the next instruction, in the core's own units (words
    /// on the 703). The debugger's breakpoints compare against this.
    fn pc(&self) -> u32 {
        0
    }

    /// Every register the debugger can see, the trace line's first. None of
    /// this may touch the bus: the debugger reads registers on a running
    /// machine, and a bus access it makes is one the guest didn't.
    fn registers(&self) -> Vec<Register> {
        Vec::new()
    }

    /// Set a register by its `registers()` name (upper case); the value is
    /// masked to the register's width. False means no such register.
    fn set_register(&mut self, _name: &str, _value: u32) -> bool {
        false
    }

    /// How the debugger should read this core's memory. The default is the
    /// byte-addressed big-endian shape of the Motorola parts.
    fn addressing(&self) -> Addressing {
        Addressing { endian: Endian::Big, unit_bytes: 1 }
    }

    /// Human-readable register dump, for debugging.
    fn dump(&self);

    /// One line of trace state for the instruction *about to* execute.
    ///
    /// Must log PC plus register state and **not** the opcode: peeking the
    /// opcode would consume a byte whenever PC sits on a device register, so a
    /// traced run would diverge from an untraced one.
    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()>;
}
