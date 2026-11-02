// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Intel 8080 interpreter core.
//!
//! Not a port, and not a mode of the Z80 core: written from Intel's *8080
//! Microcomputer Systems User's Manual* (September 1975), whose chapter 4
//! instruction set gives each instruction's flags and states. The decode is
//! the x/y/z split of the opcode byte that `z80.rs` uses, since the Z80's
//! unprefixed page is the 8080's map, but no semantics are shared: the two
//! parts disagree at the flag byte, and an exerciser CRCs that byte.
//!
//! Where the 8080 is not a Z80 without prefixes:
//!
//!   - The flag byte is `S Z 0 AC 0 P 1 CY`: bit 1 always reads 1 and bits 3
//!     and 5 always 0, in `PUSH PSW` and after `POP PSW` alike.
//!   - P is the result's parity after every arithmetic and logical
//!     instruction. There is no overflow flag and no N.
//!   - AC on a subtract (`SUB`, `SBB`, `CMP`, `DCR`, the immediates) is the
//!     carry out of bit 3 of the two's-complement add the ALU really does --
//!     set when there is *no* borrow, the inverse of the Z80's H.
//!   - `ANA` sets AC from bit 3 of the OR of its operands; `ORA` and `XRA`
//!     clear it. All three clear CY.
//!   - `DAA` has no N to consult: it always adjusts as after an add.
//!   - `DAD` sets CY alone, the rotates CY alone, `CMA` nothing, and `INR`/
//!     `DCR` everything but CY.
//!   - The holes in the map are aliases on silicon, and run as their
//!     documented twins, states included: `08`..`38` are `NOP`, `CB` is
//!     `JMP`, `D9` is `RET`, and `DD`, `ED` and `FD` are `CALL`.
//!   - `JMP` takes 10 states whether or not a condition holds; a conditional
//!     `CALL` 17 taken and 11 not, a conditional `RET` 11 and 5.
//!
//! Interrupts are the part's: `EI` enables them after the instruction that
//! follows it (manual, `EI`: "the processor's interrupt system is enabled
//! following the execution of the next instruction"), an accepted interrupt
//! clears INTE and executes the instruction the interrupting device puts on
//! the data bus, and `HLT` stops the processor until one comes. There is no
//! NMI, so a `HLT` with INTE clear can never be woken -- only RESET does
//! that -- and the core reports it as `Halted`.

use super::{Addressing, Cpu, Register, StepResult};
use crate::bus::{Bus, Endian};
use std::io::Write;

const F_C: u8 = 1 << 0;
/// Bit 1: no flag, always 1.
const F_ONE: u8 = 1 << 1;
const F_P: u8 = 1 << 2;
const F_AC: u8 = 1 << 4;
const F_Z: u8 = 1 << 6;
const F_S: u8 = 1 << 7;
/// The bits a flag byte can have set; bits 3 and 5 are always 0.
const F_MASK: u8 = F_S | F_Z | F_AC | F_P | F_ONE | F_C;

/// States per opcode (manual, chapter 4), the not-taken count for a
/// conditional `CALL` or `RET`, whose taken surcharge is charged where the
/// condition is tested.
#[rustfmt::skip]
const CYCLES: [u8; 256] = [
//   0   1   2   3   4   5   6   7   8   9   a   b   c   d   e   f
     4, 10,  7,  5,  5,  5,  7,  4,  4, 10,  7,  5,  5,  5,  7,  4, // 0x
     4, 10,  7,  5,  5,  5,  7,  4,  4, 10,  7,  5,  5,  5,  7,  4, // 1x
     4, 10, 16,  5,  5,  5,  7,  4,  4, 10, 16,  5,  5,  5,  7,  4, // 2x
     4, 10, 13,  5, 10, 10, 10,  4,  4, 10, 13,  5,  5,  5,  7,  4, // 3x
     5,  5,  5,  5,  5,  5,  7,  5,  5,  5,  5,  5,  5,  5,  7,  5, // 4x
     5,  5,  5,  5,  5,  5,  7,  5,  5,  5,  5,  5,  5,  5,  7,  5, // 5x
     5,  5,  5,  5,  5,  5,  7,  5,  5,  5,  5,  5,  5,  5,  7,  5, // 6x
     7,  7,  7,  7,  7,  7,  7,  7,  5,  5,  5,  5,  5,  5,  7,  5, // 7x
     4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 8x
     4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 9x
     4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // ax
     4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // bx
     5, 10, 10, 10, 11, 11,  7, 11,  5, 10, 10, 10, 11, 17,  7, 11, // cx
     5, 10, 10, 10, 11, 11,  7, 11,  5, 10, 10, 10, 11, 17,  7, 11, // dx
     5, 10, 10, 18, 11, 11,  7, 11,  5,  5, 10,  4, 11, 17,  7, 11, // ex
     5, 10, 10,  4, 11, 11,  7, 11,  5,  5, 10,  4, 11, 17,  7, 11, // fx
];

/// What a taken conditional `CALL` or `RET` costs over its not-taken count.
const TAKEN_SURCHARGE: u32 = 6;

/// States a sleeping `HLT` charges per step. The halted part runs no
/// machine cycles at all; this is the quantum the run loop and the devices
/// see time pass in while it waits.
const HALT_STATES: u32 = 4;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AluOp {
    Add,
    Adc,
    Sub,
    Sbb,
    Ana,
    Xra,
    Ora,
    Cmp,
}

const ALU_OPS: [AluOp; 8] =
    [AluOp::Add, AluOp::Adc, AluOp::Sub, AluOp::Sbb, AluOp::Ana, AluOp::Xra, AluOp::Ora, AluOp::Cmp];

#[derive(Clone, Default)]
pub struct Cpu8080 {
    a: u8,
    /// Always within `F_MASK` and with `F_ONE` set.
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    /// The interrupt enable flip-flop, the part's INTE pin.
    inte: bool,
    /// Set by `EI`, cleared by the next step's interrupt poll, which it
    /// suppresses: the instruction after `EI` runs before an interrupt can
    /// be accepted, which is what lets a handler end `EI; RET`.
    ei_shadow: bool,
    /// Stopped by `HLT`, with `pc` held on it until an interrupt is
    /// accepted, which returns past it. Held there rather than past it for
    /// the debugger: a breakpoint on the instruction after a `HLT` stops
    /// once, when the interrupt returns, not on every sleeping step.
    halted: bool,
    /// States the last `step()` consumed, for `last_step_cycles`.
    cycles: u32,
}

fn parity(val: u8) -> bool {
    val.count_ones().is_multiple_of(2)
}

impl Cpu8080 {
    pub fn new() -> Self {
        Cpu8080 { f: F_ONE, ..Default::default() }
    }

    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    fn psw(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }
    fn set_bc(&mut self, v: u16) {
        [self.b, self.c] = v.to_be_bytes();
    }
    fn set_de(&mut self, v: u16) {
        [self.d, self.e] = v.to_be_bytes();
    }
    fn set_hl(&mut self, v: u16) {
        [self.h, self.l] = v.to_be_bytes();
    }
    /// The flag byte through its fixed bits, as `POP PSW` loads it.
    fn set_psw(&mut self, v: u16) {
        let [a, f] = v.to_be_bytes();
        self.a = a;
        self.f = (f & F_MASK) | F_ONE;
    }

    /// The pair an `rp` field names: B, D, H, SP.
    fn rp(&self, p: u8) -> u16 {
        match p {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            _ => self.sp,
        }
    }
    fn set_rp(&mut self, p: u8, v: u16) {
        match p {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => self.set_hl(v),
            _ => self.sp = v,
        }
    }

    /// A register field: B C D E H L M A, M the byte at HL.
    fn reg(&mut self, bus: &mut dyn Bus, r: u8) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => bus.read8(self.hl() as u32),
            _ => self.a,
        }
    }
    fn set_reg(&mut self, bus: &mut dyn Bus, r: u8, v: u8) {
        match r {
            0 => self.b = v,
            1 => self.c = v,
            2 => self.d = v,
            3 => self.e = v,
            4 => self.h = v,
            5 => self.l = v,
            6 => bus.write8(self.hl() as u32, v),
            _ => self.a = v,
        }
    }

    fn imm8(&mut self, bus: &mut dyn Bus) -> u8 {
        let v = bus.read8(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        v
    }
    fn imm16(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = self.imm8(bus);
        u16::from_le_bytes([lo, self.imm8(bus)])
    }
    fn read16(&self, bus: &mut dyn Bus, addr: u16) -> u16 {
        let lo = bus.read8(addr as u32);
        u16::from_le_bytes([lo, bus.read8(addr.wrapping_add(1) as u32)])
    }
    fn write16(&self, bus: &mut dyn Bus, addr: u16, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        bus.write8(addr as u32, lo);
        bus.write8(addr.wrapping_add(1) as u32, hi);
    }
    /// High byte first, as the part pushes.
    fn push(&mut self, bus: &mut dyn Bus, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.sp = self.sp.wrapping_sub(1);
        bus.write8(self.sp as u32, hi);
        self.sp = self.sp.wrapping_sub(1);
        bus.write8(self.sp as u32, lo);
    }
    fn pop(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = bus.read8(self.sp as u32);
        self.sp = self.sp.wrapping_add(1);
        let hi = bus.read8(self.sp as u32);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    fn flag(&self, bit: u8) -> bool {
        self.f & bit != 0
    }
    fn set_flag(&mut self, bit: u8, on: bool) {
        if on {
            self.f |= bit;
        } else {
            self.f &= !bit;
        }
    }
    /// S, Z and P from a result.
    fn set_szp(&mut self, v: u8) {
        self.set_flag(F_S, v & 0x80 != 0);
        self.set_flag(F_Z, v == 0);
        self.set_flag(F_P, parity(v));
    }

    /// NZ Z NC C PO PE P M.
    fn cond(&self, cc: u8) -> bool {
        let bit = [F_Z, F_C, F_P, F_S][(cc >> 1) as usize];
        self.flag(bit) == (cc & 1 == 1)
    }

    /// The adder: `a + v + carry`, setting S Z AC P CY from it.
    fn add(&mut self, v: u8, carry: bool) -> u8 {
        let wide = self.a as u16 + v as u16 + carry as u16;
        let r = wide as u8;
        self.set_szp(r);
        self.set_flag(F_C, wide > 0xff);
        self.set_flag(F_AC, (self.a & 0x0f) + (v & 0x0f) + carry as u8 > 0x0f);
        r
    }

    /// A subtract is the adder over the one's complement of the operand
    /// with the borrow inverted into the carry, and CY is the inverted
    /// carry out -- but AC is the adder's carry out of bit 3 uninverted,
    /// which is the 8080's AC on a subtract.
    fn sub(&mut self, v: u8, borrow: bool) -> u8 {
        let r = self.add(!v, !borrow);
        self.f ^= F_C;
        r
    }

    fn alu(&mut self, op: AluOp, v: u8) {
        let carry = self.flag(F_C);
        match op {
            AluOp::Add => self.a = self.add(v, false),
            AluOp::Adc => self.a = self.add(v, carry),
            AluOp::Sub => self.a = self.sub(v, false),
            AluOp::Sbb => self.a = self.sub(v, carry),
            AluOp::Cmp => {
                self.sub(v, false);
            }
            AluOp::Ana | AluOp::Xra | AluOp::Ora => {
                let r = match op {
                    AluOp::Ana => self.a & v,
                    AluOp::Xra => self.a ^ v,
                    _ => self.a | v,
                };
                self.set_flag(F_AC, op == AluOp::Ana && (self.a | v) & 0x08 != 0);
                self.a = r;
                self.set_szp(r);
                self.set_flag(F_C, false);
            }
        }
    }

    /// `INR` and `DCR`: the adder over 1 or 0xff, CY untouched.
    fn inr_dcr(&mut self, v: u8, inr: bool) -> u8 {
        let r = if inr { v.wrapping_add(1) } else { v.wrapping_sub(1) };
        self.set_szp(r);
        self.set_flag(F_AC, if inr { r & 0x0f == 0 } else { r & 0x0f != 0x0f });
        r
    }

    /// Adds 6 to the low digit if it is over 9 or AC is set, then 6 to the
    /// high digit if that is over 9 or CY is set; CY is set by the second
    /// adjustment and never cleared.
    fn daa(&mut self) {
        let mut adjust = 0;
        let mut carry = self.flag(F_C);
        if self.a & 0x0f > 9 || self.flag(F_AC) {
            adjust |= 0x06;
        }
        if self.a >> 4 > 9 || carry || (self.a >> 4 >= 9 && self.a & 0x0f > 9) {
            adjust |= 0x60;
            carry = true;
        }
        self.a = self.add(adjust, false);
        self.set_flag(F_C, carry);
    }

    /// The `HLT` an interrupt or the debugger ends.
    fn wake(&mut self) {
        if self.halted {
            self.halted = false;
            self.pc = self.pc.wrapping_add(1);
        }
    }

    fn exec(&mut self, bus: &mut dyn Bus, op: u8) -> StepResult {
        self.cycles += CYCLES[op as usize] as u32;
        let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
        let (p, q) = (y >> 1, y & 1);
        match (x, z) {
            // NOP, and its seven aliases
            (0, 0) => {}
            (0, 1) => {
                if q == 0 {
                    let v = self.imm16(bus);
                    self.set_rp(p, v);
                } else {
                    // DAD
                    let wide = self.hl() as u32 + self.rp(p) as u32;
                    self.set_hl(wide as u16);
                    self.set_flag(F_C, wide > 0xffff);
                }
            }
            (0, 2) => match (p, q) {
                (0, 0) => bus.write8(self.bc() as u32, self.a),
                (1, 0) => bus.write8(self.de() as u32, self.a),
                (2, 0) => {
                    let addr = self.imm16(bus);
                    self.write16(bus, addr, self.hl());
                }
                (3, 0) => {
                    let addr = self.imm16(bus);
                    bus.write8(addr as u32, self.a);
                }
                (0, _) => self.a = bus.read8(self.bc() as u32),
                (1, _) => self.a = bus.read8(self.de() as u32),
                (2, _) => {
                    let addr = self.imm16(bus);
                    let v = self.read16(bus, addr);
                    self.set_hl(v);
                }
                _ => {
                    let addr = self.imm16(bus);
                    self.a = bus.read8(addr as u32);
                }
            },
            (0, 3) => {
                let v = self.rp(p);
                self.set_rp(p, if q == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) });
            }
            (0, 4) | (0, 5) => {
                let v = self.reg(bus, y);
                let r = self.inr_dcr(v, z == 4);
                self.set_reg(bus, y, r);
            }
            (0, 6) => {
                let v = self.imm8(bus);
                self.set_reg(bus, y, v);
            }
            (0, _) => match y {
                // RLC RRC RAL RAR: CY alone
                0 => {
                    self.a = self.a.rotate_left(1);
                    self.set_flag(F_C, self.a & 1 != 0);
                }
                1 => {
                    self.set_flag(F_C, self.a & 1 != 0);
                    self.a = self.a.rotate_right(1);
                }
                2 => {
                    let out = self.a & 0x80 != 0;
                    self.a = (self.a << 1) | self.flag(F_C) as u8;
                    self.set_flag(F_C, out);
                }
                3 => {
                    let out = self.a & 1 != 0;
                    self.a = (self.a >> 1) | ((self.flag(F_C) as u8) << 7);
                    self.set_flag(F_C, out);
                }
                4 => self.daa(),
                5 => self.a = !self.a,
                6 => self.set_flag(F_C, true),
                _ => self.f ^= F_C,
            },
            (1, 6) if y == 6 => {
                // HLT, held on itself: see `halted`
                self.pc = self.pc.wrapping_sub(1);
                self.halted = true;
                if !self.inte {
                    return StepResult::Halted;
                }
            }
            (1, _) => {
                let v = self.reg(bus, z);
                self.set_reg(bus, y, v);
            }
            (2, _) => {
                let v = self.reg(bus, z);
                self.alu(ALU_OPS[y as usize], v);
            }
            (3, 0) => {
                if self.cond(y) {
                    self.cycles += TAKEN_SURCHARGE;
                    self.pc = self.pop(bus);
                }
            }
            (3, 1) => match (q, p) {
                (0, _) => {
                    let v = self.pop(bus);
                    match p {
                        3 => self.set_psw(v),
                        _ => self.set_rp(p, v),
                    }
                }
                // RET, and D9
                (_, 0) | (_, 1) => self.pc = self.pop(bus),
                (_, 2) => self.pc = self.hl(),
                _ => self.sp = self.hl(),
            },
            (3, 2) => {
                let target = self.imm16(bus);
                if self.cond(y) {
                    self.pc = target;
                }
            }
            (3, 3) => match y {
                // JMP, and CB
                0 | 1 => self.pc = self.imm16(bus),
                2 => {
                    let n = self.imm8(bus);
                    bus.io_write8(u16::from_be_bytes([n, n]), self.a);
                }
                3 => {
                    // the port number is on both halves of the address bus
                    let n = self.imm8(bus);
                    self.a = bus.io_read8(u16::from_be_bytes([n, n]));
                }
                4 => {
                    // XTHL
                    let v = self.read16(bus, self.sp);
                    self.write16(bus, self.sp, self.hl());
                    self.set_hl(v);
                }
                5 => {
                    let de = self.de();
                    self.set_de(self.hl());
                    self.set_hl(de);
                }
                6 => self.inte = false,
                _ => {
                    self.inte = true;
                    self.ei_shadow = true;
                }
            },
            (3, 4) => {
                let target = self.imm16(bus);
                if self.cond(y) {
                    self.cycles += TAKEN_SURCHARGE;
                    self.push(bus, self.pc);
                    self.pc = target;
                }
            }
            (3, 5) => {
                if q == 0 {
                    let v = if p == 3 { self.psw() } else { self.rp(p) };
                    self.push(bus, v);
                } else {
                    // CALL, and DD ED FD
                    let target = self.imm16(bus);
                    self.push(bus, self.pc);
                    self.pc = target;
                }
            }
            (3, 6) => {
                let v = self.imm8(bus);
                self.alu(ALU_OPS[y as usize], v);
            }
            _ => {
                self.push(bus, self.pc);
                self.pc = (y as u16) * 8;
            }
        }
        StepResult::Ok
    }
}

impl Cpu for Cpu8080 {
    /// RESET clears the program counter, INTE and the halt; the rest of the
    /// registers are left as they were (manual, chapter 2).
    fn reset(&mut self, _bus: &mut dyn Bus) {
        self.pc = 0;
        self.inte = false;
        self.ei_shadow = false;
        self.halted = false;
        self.cycles = 0;
    }

    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        let elapsed = std::mem::replace(&mut self.cycles, 0);
        let ints = bus.poll_interrupts(elapsed);
        let shadowed = std::mem::replace(&mut self.ei_shadow, false);
        let op = if ints.irq && self.inte && !shadowed {
            // The interrupting device answers INTA with an instruction, and
            // an `RST` is the only one-byte instruction it is any use for:
            // anything else is taken as the pulled-up bus's `RST 7`. The
            // fetch does not advance PC, so the RST pushes the address of
            // the instruction it interrupted.
            self.wake();
            self.inte = false;
            let byte = bus.interrupt_acknowledge();
            if byte & 0xc7 == 0xc7 {
                byte
            } else {
                0xff
            }
        } else if self.halted {
            if !self.inte {
                return StepResult::Halted;
            }
            self.cycles += HALT_STATES;
            return StepResult::Ok;
        } else {
            self.imm8(bus)
        };
        self.exec(bus, op)
    }

    fn last_step_cycles(&self) -> u32 {
        self.cycles
    }

    fn pc(&self) -> u32 {
        self.pc as u32
    }

    fn disassemble(&self, peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<super::Disassembly> {
        super::i8080_dis::disassemble(peek, addr)
    }

    fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        let Cpu8080 { a, f, b, c, d, e, h, l, sp, pc, inte, ei_shadow, halted, cycles } = self;
        for v in [a, f, b, c, d, e, h, l] {
            w.u8(*v);
        }
        w.u16(*sp);
        w.u16(*pc);
        for v in [inte, ei_shadow, halted] {
            w.bool(*v);
        }
        w.u32(*cycles);
        Ok(())
    }

    fn load_state(&mut self, rd: &mut crate::snapshot::Reader) -> Result<(), String> {
        let s = self;
        for v in [&mut s.a, &mut s.f, &mut s.b, &mut s.c, &mut s.d, &mut s.e, &mut s.h, &mut s.l] {
            *v = rd.u8()?;
        }
        s.f = (s.f & F_MASK) | F_ONE;
        s.sp = rd.u16()?;
        s.pc = rd.u16()?;
        for v in [&mut s.inte, &mut s.ei_shadow, &mut s.halted] {
            *v = rd.bool()?;
        }
        s.cycles = rd.u32()?;
        Ok(())
    }

    /// The trace line's registers first, then the interrupt state.
    fn registers(&self) -> Vec<Register> {
        vec![
            Register::new("PC", self.pc, 16),
            Register::new("PSW", self.psw(), 16),
            Register::new("BC", self.bc(), 16),
            Register::new("DE", self.de(), 16),
            Register::new("HL", self.hl(), 16),
            Register::new("SP", self.sp, 16),
            Register::new("INTE", self.inte as u8, 1),
            Register::new("HALT", self.halted as u8, 1),
            Register::new("EI", self.ei_shadow as u8, 1),
        ]
    }

    fn set_register(&mut self, name: &str, value: u32) -> bool {
        let w = value as u16;
        match name {
            "PC" => self.pc = w,
            "PSW" => self.set_psw(w),
            "BC" => self.set_bc(w),
            "DE" => self.set_de(w),
            "HL" => self.set_hl(w),
            "SP" => self.sp = w,
            "INTE" => self.inte = value & 1 != 0,
            "HALT" => self.halted = value & 1 != 0,
            "EI" => self.ei_shadow = value & 1 != 0,
            _ => return false,
        }
        true
    }

    fn addressing(&self) -> Addressing {
        Addressing { endian: Endian::Little, unit_bytes: 1 }
    }

    fn dump(&self) {
        let fl = |bit, c| if self.flag(bit) { c } else { ' ' };
        println!(
            "psw 0x{:04x} ({}{}{}{}{}) bc 0x{:04x} de 0x{:04x} hl 0x{:04x} sp 0x{:04x} pc 0x{:04x}",
            self.psw(),
            fl(F_S, 's'),
            fl(F_Z, 'z'),
            fl(F_AC, 'a'),
            fl(F_P, 'p'),
            fl(F_C, 'c'),
            self.bc(),
            self.de(),
            self.hl(),
            self.sp,
            self.pc
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            out,
            "PC={:04x} PSW={:04x} BC={:04x} DE={:04x} HL={:04x} SP={:04x}",
            self.pc,
            self.psw(),
            self.bc(),
            self.de(),
            self.hl(),
            self.sp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{check_set_register, registers_as_trace, run_steps, trace_of, TestBus};

    fn boot(prog: &[u8]) -> (Cpu8080, TestBus) {
        let mut bus = TestBus::new();
        bus.load(0x0000, prog);
        let mut cpu = Cpu8080::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    /// One instruction at 0x0100 with A, the flags and B set first.
    fn after(op: &[u8], a: u8, f: u8, b: u8) -> Cpu8080 {
        let mut bus = TestBus::new();
        bus.load(0x0100, op);
        let mut cpu = Cpu8080::new();
        cpu.pc = 0x0100;
        cpu.set_psw(u16::from_be_bytes([a, f]));
        cpu.b = b;
        run_steps(&mut cpu, &mut bus, 1);
        cpu
    }

    #[test]
    fn registers_match_the_trace_line() {
        let (mut cpu, mut bus) = boot(&[0x01, 0x34, 0x12, 0x31, 0x00, 0x80, 0x37]); // lxi b; lxi sp; stc
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(trace_of(&cpu), registers_as_trace(&cpu, 6));
        assert_eq!(trace_of(&cpu), "PC=0007 PSW=0003 BC=1234 DE=0000 HL=0000 SP=8000");
    }

    #[test]
    fn set_register_round_trips() {
        let mut cpu = Cpu8080::new();
        check_set_register(
            &mut cpu,
            &[
                ("PC", 0x1234, 0x1234),
                ("PSW", 0x42ff, 0x42d7),
                ("PSW", 0x4200, 0x4202),
                ("BC", 0x1122, 0x1122),
                ("DE", 0x3344, 0x3344),
                ("HL", 0x5566, 0x5566),
                ("SP", 0x10000 + 0x8000, 0x8000),
                ("INTE", 1, 1),
                ("HALT", 3, 1),
                ("EI", 1, 1),
            ],
        );
    }

    /// Bit 1 of the flag byte reads 1 and bits 3 and 5 read 0, whatever is
    /// popped into it and whatever an instruction computes.
    #[test]
    fn the_flag_byte_has_bit_1_set_and_bits_3_and_5_clear() {
        // lxi h,0xffff; push h; pop psw; push psw; pop b
        let (mut cpu, mut bus) = boot(&[0x21, 0xff, 0xff, 0xe5, 0xf1, 0xf5, 0xc1]);
        cpu.sp = 0x8000;
        run_steps(&mut cpu, &mut bus, 5);
        assert_eq!(cpu.bc(), 0xffd7);
        // xra a: the result's bits 3 and 5 are clear anyway, so add 0x28
        let cpu = after(&[0xc6, 0x28], 0x00, 0, 0);
        assert_eq!(cpu.f & 0x28, 0);
        assert_eq!(cpu.f & F_ONE, F_ONE);
    }

    /// P is parity after an add as after a logical op; there is no overflow.
    #[test]
    fn p_is_parity_even_where_a_z80_reads_overflow() {
        let cpu = after(&[0xc6, 0x01], 0x7f, 0, 0); // adi 1: 0x80, one bit set
        assert_eq!(cpu.a, 0x80);
        assert!(!cpu.flag(F_P), "odd parity, though the add overflowed");
        let cpu = after(&[0xc6, 0x02], 0x01, 0, 0); // 0x03: even
        assert!(cpu.flag(F_P));
    }

    /// For every operand pair and borrow in: SUB, SBB and CMP set AC when
    /// the low digit does not borrow, CY when the byte does.
    #[test]
    fn a_subtract_sets_ac_when_the_low_digit_does_not_borrow() {
        let mut bus = TestBus::new();
        let mut cpu = Cpu8080::new();
        for a in 0..=0xffu8 {
            for v in 0..=0xffu8 {
                for borrow in [false, true] {
                    // sub b, sbb b, cmp b
                    for (op, store, uses_borrow) in [(0x90u8, true, false), (0x98, true, true), (0xb8, false, false)] {
                        bus.mem[0x0100] = op;
                        cpu.pc = 0x0100;
                        cpu.set_psw(u16::from_be_bytes([a, if borrow { F_C } else { 0 }]));
                        cpu.b = v;
                        run_steps(&mut cpu, &mut bus, 1);
                        let bin = (borrow && uses_borrow) as i32;
                        let wide = a as i32 - v as i32 - bin;
                        let ctx = format!("op {op:02x} a={a:02x} v={v:02x} borrow={borrow}");
                        assert_eq!(cpu.a, if store { wide as u8 } else { a }, "{ctx}");
                        assert_eq!(cpu.flag(F_C), wide < 0, "{ctx}: CY");
                        assert_eq!(cpu.flag(F_AC), (a & 0x0f) as i32 - (v & 0x0f) as i32 - bin >= 0, "{ctx}: AC");
                        assert_eq!(cpu.flag(F_Z), wide as u8 == 0, "{ctx}: Z");
                        assert_eq!(cpu.flag(F_S), wide as u8 & 0x80 != 0, "{ctx}: S");
                        assert_eq!(cpu.flag(F_P), parity(wide as u8), "{ctx}: P");
                    }
                }
            }
        }
    }

    #[test]
    fn an_add_sets_ac_on_the_carry_out_of_the_low_digit() {
        let cpu = after(&[0x80], 0x0f, 0, 0x01); // add b
        assert_eq!(cpu.a, 0x10);
        assert!(cpu.flag(F_AC) && !cpu.flag(F_C));
        let cpu = after(&[0x88], 0xf0, F_C, 0x0f); // adc b: 0xf0 + 0x0f + 1
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(F_AC) && cpu.flag(F_C) && cpu.flag(F_Z));
    }

    /// INR sets AC when the low digit wraps to 0; DCR when it does not wrap
    /// from 0 -- the adder over 0xff. Neither touches CY.
    #[test]
    fn inr_and_dcr_set_ac_from_the_adder_and_leave_cy() {
        let cpu = after(&[0x04], 0, F_C, 0x0f); // inr b
        assert_eq!(cpu.b, 0x10);
        assert!(cpu.flag(F_AC) && cpu.flag(F_C));
        let cpu = after(&[0x04], 0, 0, 0x0e);
        assert!(!cpu.flag(F_AC) && !cpu.flag(F_C));
        let cpu = after(&[0x05], 0, 0, 0x10); // dcr b: 0x10 -> 0x0f borrows
        assert_eq!(cpu.b, 0x0f);
        assert!(!cpu.flag(F_AC));
        let cpu = after(&[0x05], 0, 0, 0x11);
        assert!(cpu.flag(F_AC));
        let cpu = after(&[0x05], 0, 0, 0x01);
        assert!(cpu.flag(F_Z) && cpu.flag(F_P) && cpu.flag(F_AC));
    }

    /// ANA's AC is bit 3 of the OR of its operands; ORA and XRA clear AC,
    /// and all three clear CY.
    #[test]
    fn ana_takes_ac_from_bit_3_of_its_operands() {
        let cpu = after(&[0xa0], 0x08, F_C, 0x00); // ana b
        assert_eq!(cpu.a, 0);
        assert!(cpu.flag(F_AC) && !cpu.flag(F_C) && cpu.flag(F_Z));
        let cpu = after(&[0xa0], 0x00, 0, 0x08);
        assert!(cpu.flag(F_AC));
        let cpu = after(&[0xa0], 0xf7, 0, 0xf7);
        assert!(!cpu.flag(F_AC));
        let cpu = after(&[0xb0], 0x08, F_AC | F_C, 0x08); // ora b
        assert!(!cpu.flag(F_AC) && !cpu.flag(F_C));
        let cpu = after(&[0xa8], 0x08, F_AC | F_C, 0x00); // xra b
        assert!(!cpu.flag(F_AC) && !cpu.flag(F_C));
    }

    #[test]
    fn daa_after_an_add_is_the_decimal_sum_for_every_bcd_pair() {
        let bcd = |n: u32| (((n / 10) << 4) | (n % 10)) as u8;
        // adc b; daa
        let mut bus = TestBus::new();
        bus.load(0x0100, &[0x88, 0x27]);
        let mut cpu = Cpu8080::new();
        for x in 0..100 {
            for y in 0..100 {
                for carry in [false, true] {
                    cpu.pc = 0x0100;
                    cpu.set_psw(u16::from_be_bytes([bcd(x), if carry { F_C } else { 0 }]));
                    cpu.b = bcd(y);
                    run_steps(&mut cpu, &mut bus, 2);
                    let sum = x + y + carry as u32;
                    assert_eq!(cpu.a, bcd(sum % 100), "{x} + {y} + {carry}");
                    assert_eq!(cpu.flag(F_C), sum >= 100, "{x} + {y} + {carry}: CY");
                    assert_eq!(cpu.flag(F_Z), sum.is_multiple_of(100), "{x} + {y} + {carry}: Z");
                }
            }
        }
    }

    #[test]
    fn dad_and_the_rotates_set_only_cy() {
        // lxi h,0x8000; lxi b,0x8000; dad b
        let (mut cpu, mut bus) = boot(&[0x21, 0x00, 0x80, 0x01, 0x00, 0x80, 0x09]);
        cpu.f = F_ONE | F_Z | F_S | F_P | F_AC;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.hl(), 0);
        assert_eq!(cpu.f, F_ONE | F_Z | F_S | F_P | F_AC | F_C);
        let cpu = after(&[0x07], 0x80, F_Z, 0); // rlc
        assert_eq!((cpu.a, cpu.f), (0x01, F_ONE | F_Z | F_C));
        let cpu = after(&[0x0f], 0x01, 0, 0); // rrc
        assert_eq!((cpu.a, cpu.f), (0x80, F_ONE | F_C));
        let cpu = after(&[0x17], 0x80, 0, 0); // ral: through the carry
        assert_eq!((cpu.a, cpu.flag(F_C)), (0x00, true));
        let cpu = after(&[0x1f], 0x00, F_C, 0); // rar
        assert_eq!((cpu.a, cpu.flag(F_C)), (0x80, false));
        let cpu = after(&[0x2f], 0x5a, F_C, 0); // cma: no flags
        assert_eq!((cpu.a, cpu.f), (0xa5, F_ONE | F_C));
        let cpu = after(&[0x3f], 0, F_C, 0); // cmc
        assert!(!cpu.flag(F_C));
        let cpu = after(&[0x37], 0, 0, 0); // stc
        assert!(cpu.flag(F_C));
    }

    /// The holes in the opcode map run as the instructions they alias,
    /// at the same cost.
    #[test]
    fn the_undocumented_opcodes_run_as_their_documented_twins() {
        for op in [0x08u8, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38] {
            let cpu = after(&[op], 0x12, 0, 0);
            assert_eq!((cpu.pc, cpu.a, cpu.last_step_cycles()), (0x0101, 0x12, 4), "{op:02x}");
        }
        let cpu = after(&[0xcb, 0x00, 0x20], 0, 0, 0);
        assert_eq!((cpu.pc, cpu.last_step_cycles()), (0x2000, 10), "cb is jmp");
        for op in [0xddu8, 0xed, 0xfd] {
            let mut bus = TestBus::new();
            bus.load(0x0100, &[op, 0x00, 0x20]);
            let mut cpu = Cpu8080::new();
            cpu.pc = 0x0100;
            cpu.sp = 0x8000;
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!((cpu.pc, cpu.sp, cpu.last_step_cycles()), (0x2000, 0x7ffe, 17), "{op:02x} is call");
            assert_eq!(bus.read16(0x7ffe, Endian::Little), 0x0103);
            bus.load(0x2000, &[0xd9]);
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!((cpu.pc, cpu.sp, cpu.last_step_cycles()), (0x0103, 0x8000, 10), "d9 is ret");
        }
    }

    #[test]
    fn cycle_counts_match_the_intel_manual() {
        #[rustfmt::skip]
        let cases: &[(&[u8], u32)] = &[
            (&[0x00], 4),             // nop
            (&[0x41], 5),             // mov b,c
            (&[0x46], 7),             // mov b,m
            (&[0x70], 7),             // mov m,b
            (&[0x06, 0x12], 7),       // mvi b
            (&[0x36, 0x12], 10),      // mvi m
            (&[0x01, 0x34, 0x12], 10),// lxi b
            (&[0x3a, 0x00, 0x20], 13),// lda
            (&[0x32, 0x00, 0x20], 13),// sta
            (&[0x2a, 0x00, 0x20], 16),// lhld
            (&[0x22, 0x00, 0x20], 16),// shld
            (&[0x0a], 7),             // ldax b
            (&[0x02], 7),             // stax b
            (&[0xeb], 4),             // xchg
            (&[0x80], 4),             // add b
            (&[0x86], 7),             // add m
            (&[0xc6, 0x01], 7),       // adi
            (&[0x04], 5),             // inr b
            (&[0x34], 10),            // inr m
            (&[0x03], 5),             // inx b
            (&[0x09], 10),            // dad b
            (&[0x27], 4),             // daa
            (&[0x07], 4),             // rlc
            (&[0xc3, 0x00, 0x20], 10),// jmp
            (&[0xc2, 0x00, 0x20], 10),// jnz, taken
            (&[0xca, 0x00, 0x20], 10),// jz, not taken
            (&[0xcd, 0x00, 0x20], 17),// call
            (&[0xc4, 0x00, 0x20], 17),// cnz, taken
            (&[0xcc, 0x00, 0x20], 11),// cz, not taken
            (&[0xc9], 10),            // ret
            (&[0xc0], 11),            // rnz, taken
            (&[0xc8], 5),             // rz, not taken
            (&[0xc7], 11),            // rst 0
            (&[0xe9], 5),             // pchl
            (&[0xf9], 5),             // sphl
            (&[0xc5], 11),            // push b
            (&[0xc1], 10),            // pop b
            (&[0xe3], 18),            // xthl
            (&[0xd3, 0x10], 10),      // out
            (&[0xdb, 0x10], 10),      // in
            (&[0xfb], 4),             // ei
            (&[0xf3], 4),             // di
        ];
        for &(op, want) in cases {
            let cpu = after(op, 0, 0, 0); // Z clear, so NZ holds and Z does not
            assert_eq!(cpu.last_step_cycles(), want, "{op:02x?}");
        }
    }

    #[test]
    fn xthl_sphl_pchl_and_xchg_move_pairs() {
        // lxi sp,0x8000; lxi h,0x1234; push h; lxi h,0x5678; xthl
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0x21, 0x34, 0x12, 0xe5, 0x21, 0x78, 0x56, 0xe3]);
        run_steps(&mut cpu, &mut bus, 5);
        assert_eq!(cpu.hl(), 0x1234);
        assert_eq!(bus.read16(0x7ffe, Endian::Little), 0x5678);
        assert_eq!(cpu.sp, 0x7ffe);
        // lxi d,0xabcd; xchg; sphl; pchl
        bus.load(0x000b, &[0x11, 0xcd, 0xab, 0xeb, 0xf9, 0xe9]);
        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!((cpu.de(), cpu.hl(), cpu.sp, cpu.pc), (0x1234, 0xabcd, 0xabcd, 0xabcd));
    }

    #[test]
    fn push_psw_stacks_a_over_the_flags() {
        // lxi sp,0x8000; mvi a,0x42; stc; push psw
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0x3e, 0x42, 0x37, 0xf5]);
        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!(bus.mem[0x7fff], 0x42);
        assert_eq!(bus.mem[0x7ffe], F_ONE | F_C);
    }

    /// IN and OUT put the port number on both halves of the address bus.
    #[test]
    fn a_port_number_is_on_both_halves_of_the_address() {
        let (mut cpu, mut bus) = boot(&[0x3e, 0x55, 0xd3, 0x10, 0xdb, 0x11]); // mvi a; out 0x10; in 0x11
        bus.ports[0x11] = 0x99;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.io_writes, vec![(0x1010, 0x55)]);
        assert_eq!(bus.io_reads, vec![0x1111]);
        assert_eq!(cpu.a, 0x99);
    }

    /// Accepted with INTE set, not on the instruction after EI, and INTE is
    /// cleared on the way in.
    #[test]
    fn an_interrupt_waits_for_the_instruction_after_ei() {
        // lxi sp,0x8000; ei; nop; nop
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0xfb, 0x00, 0x00]);
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 2);
        assert!(cpu.inte);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x0005, "the nop after ei runs");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x0038, "rst 7 off the pulled-up bus");
        assert_eq!(bus.read16(0x7ffe, Endian::Little), 0x0005);
        assert!(!cpu.inte, "acceptance clears INTE");
        assert_eq!(cpu.last_step_cycles(), 11);
        assert_eq!(bus.acks, 1);
    }

    #[test]
    fn the_bus_answers_inta_with_an_rst() {
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0xfb, 0x00, 0x00]);
        bus.irq = true;
        bus.vector = 0xd7; // rst 2
        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!(cpu.pc, 0x0010);
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0xfb, 0x00, 0x00]);
        bus.irq = true;
        bus.vector = 0x00; // not an rst
        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!(cpu.pc, 0x0038);
    }

    #[test]
    fn hlt_sleeps_until_an_interrupt_returns_past_it() {
        // lxi sp,0x8000; ei; hlt; mvi a,0x42 -- and rst 7 a plain ret
        let (mut cpu, mut bus) = boot(&[0x31, 0x00, 0x80, 0xfb, 0x76, 0x3e, 0x42]);
        bus.load(0x38, &[0xc9]);
        run_steps(&mut cpu, &mut bus, 3);
        assert!(cpu.halted);
        assert_eq!(cpu.pc, 0x0004, "held on the hlt");
        assert_eq!(cpu.last_step_cycles(), 7);
        bus.watch = Some(0x0004);
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.pc, cpu.last_step_cycles(), bus.watch_reads), (0x0004, HALT_STATES, 0));
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert!(!cpu.halted);
        assert_eq!(cpu.pc, 0x0038);
        assert_eq!(bus.read16(cpu.sp as u32, Endian::Little), 0x0005, "past the hlt");
        bus.irq = false;
        run_steps(&mut cpu, &mut bus, 2); // ret; mvi a
        assert_eq!((cpu.a, cpu.pc), (0x42, 0x0007));
    }

    /// With INTE clear nothing can end a HLT -- the part has no NMI -- so
    /// it reports Halted, and goes on reporting it until a reset.
    #[test]
    fn hlt_with_interrupts_off_is_halted_for_good() {
        let (mut cpu, mut bus) = boot(&[0xf3, 0x76]); // di; hlt
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.step(&mut bus), StepResult::Halted);
        bus.irq = true;
        assert_eq!(cpu.step(&mut bus), StepResult::Halted);
        assert_eq!(cpu.pc, 0x0001);
        cpu.reset(&mut bus);
        assert_eq!((cpu.pc, cpu.halted), (0, false));
    }

    #[test]
    fn conditions_read_the_flags_they_name() {
        // (opcode of jnz..jm, flags, taken)
        let cases = [
            (0xc2, 0, true),
            (0xc2, F_Z, false),
            (0xca, F_Z, true),
            (0xd2, F_C, false),
            (0xda, F_C, true),
            (0xe2, 0, true),
            (0xe2, F_P, false),
            (0xea, F_P, true),
            (0xf2, F_S, false),
            (0xfa, F_S, true),
        ];
        for (op, f, taken) in cases {
            let cpu = after(&[op, 0x00, 0x20], 0, f, 0);
            assert_eq!(cpu.pc == 0x2000, taken, "{op:02x} with f={f:02x}");
        }
    }

    #[test]
    fn a_short_program_sums_a_table() {
        // lxi h,0x0020; mvi b,4; xra a; loop: add m; inx h; dcr b; jnz loop; hlt
        let (mut cpu, mut bus) =
            boot(&[0x21, 0x20, 0x00, 0x06, 0x04, 0xaf, 0x86, 0x23, 0x05, 0xc2, 0x06, 0x00, 0x76]);
        bus.load(0x20, &[1, 2, 3, 0x10]);
        let mut steps = 0;
        while cpu.step(&mut bus) == StepResult::Ok {
            steps += 1;
            assert!(steps < 100);
        }
        assert_eq!((cpu.a, cpu.pc), (0x16, 0x000c));
    }
}
