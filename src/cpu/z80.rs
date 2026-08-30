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
//! Zilog Z80 interpreter core.
//!
//! Port of `cpu/cpuz80.cpp`, preserved bug-for-bug.
//!
//! # Why this core is shaped differently from the 6800/6809 ones
//!
//! Those two are a 256-entry `OpDecode` table plus a handful of shared
//! operation handlers, because their opcode maps really are a cross product of
//! (operation x addressing mode x target register). The Z80's is not: the
//! DD/FD prefix changes what an opcode *means* per-opcode, and whether an
//! instruction "consumes" the prefix decides whether the run continues at all
//! (see [`CpuZ80::step`]). A 256-entry table would end up one bespoke entry per
//! opcode -- a switch statement wearing a table costume.
//!
//! What does factor cleanly is the *operation*, once the operand is in hand.
//! So the decode is the standard `x/y/z/p/q` bit split of the opcode, and the
//! semantics live in small op-kind enums shared by every encoding that reaches
//! them:
//!
//!   - [`AluOp`] -- one implementation of the eight 8-bit ALU operations,
//!     shared by the register forms (`0x80..=0xbf`) and the immediate forms
//!     (`0xc6`, `0xce`, ... `0xfe`). The C++ writes those flag expressions out
//!     twice; they are identical, checked expression by expression.
//!   - [`RotOp`] -- the eight CB-page rotates/shifts, which the C++ writes out
//!     as eight near-identical blocks.
//!   - [`CpuZ80::block_in`] / [`block_out`](CpuZ80::block_out) /
//!     [`block_cp`](CpuZ80::block_cp) -- the ED-page block operations,
//!     parameterized by direction and whether they repeat.
//!
//! Operand *fetch* deliberately stays at the call site rather than moving into
//! the shared handlers, because that is exactly where the prefix rules differ.
//!
//! # Faithfulness
//!
//! The base page and the CB page are complete (every opcode value decodes), but
//! **the ED page is full of holes** and must stay that way: `LDI`, `LDD` and
//! `LDDR` are absent (only `LDIR` exists), `NEG` is only `0x44` and not its
//! seven aliases, `RETN` is absent, and `IM` is missing its `0x76`/`0x7e`
//! encodings. Each falls through to `BadOpcode`, which ends the run. Leaving
//! them out is a decision carried over from the C++, not an oversight.
//!
//! Other deliberate quirks, each marked at its use site: `LD r, (IX+d)` adds
//! `d` unsigned while every other indexed form sign-extends; most CB shifts
//! ignore an active DD/FD prefix and then abort; `HALT` is a `NOP`; `RETI` is a
//! plain `RET`; `RLC`/`RES`/`SET` have the undocumented register writeback.

use super::{Cpu, StepResult};
use crate::bus::{Bus, Endian};
use std::io::Write;

// Flag bits, matching the C++ `Flag` enum. Bits 3 and 5 (F3/F5) exist on real
// silicon but the C++ never writes them, so neither do we.
const F_C: u8 = 1 << 0;
const F_N: u8 = 1 << 1;
const F_PV: u8 = 1 << 2;
const F_H: u8 = 1 << 4;
const F_Z: u8 = 1 << 6;
const F_S: u8 = 1 << 7;

/// The eight 8-bit ALU operations, indexed by the opcode's `y` field.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum AluOp {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
}

const ALU_OPS: [AluOp; 8] = [
    AluOp::Add,
    AluOp::Adc,
    AluOp::Sub,
    AluOp::Sbc,
    AluOp::And,
    AluOp::Xor,
    AluOp::Or,
    AluOp::Cp,
];

/// The eight CB-page rotates and shifts, indexed by the opcode's `y` field.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum RotOp {
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    /// undocumented
    Sll,
    Srl,
}

const ROT_OPS: [RotOp; 8] = [
    RotOp::Rlc,
    RotOp::Rrc,
    RotOp::Rl,
    RotOp::Rr,
    RotOp::Sla,
    RotOp::Sra,
    RotOp::Sll,
    RotOp::Srl,
];

/// Which DD/FD prefix an instruction actually made use of.
///
/// The C++ carries two `consume_mPrefix*` locals and errors out at the end of
/// the instruction if a prefix was set but never consumed. Both can be set at
/// once (`DD FD 21 ...` sets both prefixes, and only one of them can be
/// consumed), so this stays two independent flags rather than an enum.
#[derive(Copy, Clone, Default)]
struct PrefixUse {
    dd: bool,
    fd: bool,
}

#[derive(Clone)]
pub struct CpuZ80 {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,

    a_alt: u8,
    f_alt: u8,
    b_alt: u8,
    c_alt: u8,
    d_alt: u8,
    e_alt: u8,
    h_alt: u8,
    l_alt: u8,

    pc: u16,
    sp: u16,
    ix: u16,
    iy: u16,

    im: u8,
    iff1: bool,
    iff2: bool,
    i: u8,
    r: u8,

    /// Active DD/FD prefix. Struct state rather than locals because
    /// [`read_r`](CpuZ80::read_r) and [`write_r`](CpuZ80::write_r) consult it
    /// to remap `H`/`L` onto the halves of IX/IY.
    prefix_dd: bool,
    prefix_fd: bool,
}

impl Default for CpuZ80 {
    fn default() -> Self {
        // The C++ `mRegs = {}` leaves the in-class initializers alone, so `im`
        // starts at 1 while everything else zeroes. It only matters on the
        // interrupt path, which is dead code, but keep it faithful.
        CpuZ80 {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_alt: 0,
            f_alt: 0,
            b_alt: 0,
            c_alt: 0,
            d_alt: 0,
            e_alt: 0,
            h_alt: 0,
            l_alt: 0,
            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,
            im: 1,
            iff1: false,
            iff2: false,
            i: 0,
            r: 0,
            prefix_dd: false,
            prefix_fd: false,
        }
    }
}

/// Even parity: true when the number of set bits is even, as the C++
/// `calc_parity` returns.
fn parity(val: u8) -> bool {
    val.count_ones().is_multiple_of(2)
}

impl CpuZ80 {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- register pairs ----

    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | self.f as u16
    }
    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }
    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }
    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }
    fn af_alt(&self) -> u16 {
        ((self.a_alt as u16) << 8) | self.f_alt as u16
    }

    fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = val as u8;
    }
    fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }
    fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }
    fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }
    fn set_af_alt(&mut self, val: u16) {
        self.a_alt = (val >> 8) as u8;
        self.f_alt = val as u8;
    }

    /// The `dd` register pair encoding: BC, DE, HL, SP.
    fn read_dd(&self, dd: u8) -> u16 {
        match dd {
            0b00 => self.bc(),
            0b01 => self.de(),
            0b10 => self.hl(),
            _ => self.sp,
        }
    }

    fn write_dd(&mut self, dd: u8, val: u16) {
        match dd {
            0b00 => self.set_bc(val),
            0b01 => self.set_de(val),
            0b10 => self.set_hl(val),
            _ => self.sp = val,
        }
    }

    /// The `qq` register pair encoding: BC, DE, HL, AF.
    fn read_qq(&self, qq: u8) -> u16 {
        match qq {
            0b00 => self.bc(),
            0b01 => self.de(),
            0b10 => self.hl(),
            _ => self.af(),
        }
    }

    fn write_qq(&mut self, qq: u8, val: u16) {
        match qq {
            0b00 => self.set_bc(val),
            0b01 => self.set_de(val),
            0b10 => self.set_hl(val),
            _ => self.set_af(val),
        }
    }

    // ---- 8-bit register file ----

    /// The `r` encoding. Under a DD/FD prefix, `H` and `L` are remapped onto
    /// the halves of IX/IY -- which is what makes the prefix flags struct state
    /// rather than locals. Note this happens whether or not the instruction
    /// goes on to *consume* the prefix: the undocumented `ADD A, IXh` reads the
    /// remapped half and only then aborts the run at end-of-instruction.
    fn read_r(&self, r: u8) -> u8 {
        if self.prefix_dd {
            match r {
                0b100 => return (self.ix >> 8) as u8,
                0b101 => return self.ix as u8,
                _ => {}
            }
        } else if self.prefix_fd {
            match r {
                0b100 => return (self.iy >> 8) as u8,
                0b101 => return self.iy as u8,
                _ => {}
            }
        }

        match r {
            0b000 => self.b,
            0b001 => self.c,
            0b010 => self.d,
            0b011 => self.e,
            0b100 => self.h,
            0b101 => self.l,
            0b111 => self.a,
            // 0b110 is the (HL) hole; every caller either guards it or routes
            // through read_r_or_hl. The C++ asserts here.
            _ => unreachable!("read_r called with the (HL) encoding"),
        }
    }

    fn write_r(&mut self, r: u8, val: u8) {
        if self.prefix_dd {
            match r {
                0b100 => {
                    self.ix = (self.ix & 0x00ff) | ((val as u16) << 8);
                    return;
                }
                0b101 => {
                    self.ix = (self.ix & 0xff00) | val as u16;
                    return;
                }
                _ => {}
            }
        } else if self.prefix_fd {
            match r {
                0b100 => {
                    self.iy = (self.iy & 0x00ff) | ((val as u16) << 8);
                    return;
                }
                0b101 => {
                    self.iy = (self.iy & 0xff00) | val as u16;
                    return;
                }
                _ => {}
            }
        }

        match r {
            0b000 => self.b = val,
            0b001 => self.c = val,
            0b010 => self.d = val,
            0b011 => self.e = val,
            0b100 => self.h = val,
            0b101 => self.l = val,
            0b111 => self.a = val,
            _ => unreachable!("write_r called with the (HL) encoding"),
        }
    }

    /// For encodings where the missing register slot means `(HL)`.
    fn read_r_or_hl(&mut self, bus: &mut dyn Bus, r: u8) -> u8 {
        if r == 0b110 {
            let addr = self.hl();
            self.mem_read(bus, addr)
        } else {
            self.read_r(r)
        }
    }

    fn write_r_or_hl(&mut self, bus: &mut dyn Bus, r: u8, val: u8) {
        if r == 0b110 {
            let addr = self.hl();
            self.mem_write(bus, addr, val);
        } else {
            self.write_r(r, val);
        }
    }

    // ---- bus and stack ----

    /// Addresses wrap at 64K. The C++ lets `temp16 + 1` promote to `int` and
    /// hands 0x10000 to the decode, which masks it for the decode but not for
    /// the offset -- an out-of-bounds read of the rom bank. That is UB, not
    /// behaviour worth reproducing; wrapping is the sane reading.
    fn mem_read(&self, bus: &mut dyn Bus, addr: u16) -> u8 {
        bus.read8(addr as u32)
    }

    fn mem_write(&self, bus: &mut dyn Bus, addr: u16, val: u8) {
        bus.write8(addr as u32, val);
    }

    fn read_n(&mut self, bus: &mut dyn Bus) -> u8 {
        let val = bus.read8(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn read_nn(&mut self, bus: &mut dyn Bus) -> u16 {
        let val = bus.read16(self.pc as u32, Endian::Little);
        self.pc = self.pc.wrapping_add(2);
        val
    }

    /// The signed displacement of an indexed operand.
    fn read_d(&mut self, bus: &mut dyn Bus) -> i8 {
        self.read_n(bus) as i8
    }

    /// `(IX+d)` / `(IY+d)`, picked by whichever prefix is active.
    fn indexed_addr(&mut self, bus: &mut dyn Bus, use_ix: bool) -> u16 {
        let base = if use_ix { self.ix } else { self.iy };
        let d = self.read_d(bus);
        base.wrapping_add(d as u16)
    }

    fn push8(&mut self, bus: &mut dyn Bus, val: u8) {
        self.sp = self.sp.wrapping_sub(1);
        self.mem_write(bus, self.sp, val);
    }

    fn push16(&mut self, bus: &mut dyn Bus, val: u16) {
        self.push8(bus, (val >> 8) as u8);
        self.push8(bus, val as u8);
    }

    fn pop8(&mut self, bus: &mut dyn Bus) -> u8 {
        let val = self.mem_read(bus, self.sp);
        self.sp = self.sp.wrapping_add(1);
        val
    }

    fn pop16(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = self.pop8(bus) as u16;
        let hi = self.pop8(bus) as u16;
        (hi << 8) | lo
    }

    // ---- flags ----

    fn set_flag(&mut self, bit: u8, on: bool) {
        if on {
            self.f |= bit;
        } else {
            self.f &= !bit;
        }
    }

    fn flag(&self, bit: u8) -> bool {
        (self.f & bit) != 0
    }

    fn carry(&self) -> u8 {
        u8::from(self.flag(F_C))
    }

    fn set_sz(&mut self, val: u8) {
        self.set_flag(F_S, (val & 0x80) != 0);
        self.set_flag(F_Z, val == 0);
    }

    /// The logical-operation flag set: S, Z and parity from the result, H/N/C
    /// cleared. `AND` re-sets H afterwards.
    fn set_logic_flags(&mut self, val: u8) {
        self.set_sz(val);
        self.set_flag(F_PV, parity(val));
        self.set_flag(F_H, false);
        self.set_flag(F_N, false);
        self.set_flag(F_C, false);
    }

    /// The eight branch conditions, in encoding order.
    fn test_cond(&self, cond: u8) -> bool {
        match cond {
            0 => !self.flag(F_Z),
            1 => self.flag(F_Z),
            2 => !self.flag(F_C),
            3 => self.flag(F_C),
            4 => !self.flag(F_PV),
            5 => self.flag(F_PV),
            6 => !self.flag(F_S),
            _ => self.flag(F_S),
        }
    }

    // ---- shared operations ----

    /// One 8-bit ALU operation against the accumulator.
    ///
    /// Shared by the register/`(HL)`/indexed forms and the immediate forms; the
    /// C++ spells those out separately but every flag expression matches.
    fn alu(&mut self, op: AluOp, val: u8) {
        let a = self.a;
        match op {
            AluOp::Add => {
                let res = a.wrapping_add(val);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (a & 0xf) + (val & 0xf) > 0xf);
                self.set_flag(F_PV, ((a ^ res) & (val ^ res) & 0x80) != 0);
                self.set_flag(F_N, false);
                self.set_flag(F_C, a as u16 + val as u16 > 0xff);
                self.a = res;
            }
            AluOp::Adc => {
                let c = self.carry();
                let res = a.wrapping_add(val).wrapping_add(c);
                self.set_flag(F_C, a as u16 + val as u16 + c as u16 > 0xff);
                self.set_flag(F_N, false);
                self.set_flag(F_PV, ((a ^ res) & (val ^ res) & 0x80) != 0);
                self.set_flag(F_H, (a ^ res ^ val) & 0x10 != 0);
                self.set_sz(res);
                self.a = res;
            }
            AluOp::Sub | AluOp::Cp => {
                let res = a.wrapping_sub(val);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (a & 0xf) < (val & 0xf));
                self.set_flag(F_PV, ((a ^ val) & (a ^ res) & 0x80) != 0);
                self.set_flag(F_N, true);
                self.set_flag(F_C, a < val);
                // CP is SUB without the writeback
                if op == AluOp::Sub {
                    self.a = res;
                }
            }
            AluOp::Sbc => {
                let c = self.carry();
                let res = a.wrapping_sub(val).wrapping_sub(c);
                self.set_flag(F_C, (a as i32) < val as i32 + c as i32);
                self.set_flag(F_N, true);
                self.set_flag(F_PV, ((a ^ val) & (a ^ res) & 0x80) != 0);
                self.set_flag(F_H, (a ^ res ^ val) & 0x10 != 0);
                self.set_sz(res);
                self.a = res;
            }
            AluOp::And => {
                self.a = a & val;
                let res = self.a;
                self.set_logic_flags(res);
                // AND is the one logical op that leaves H set
                self.set_flag(F_H, true);
            }
            AluOp::Xor => {
                self.a = a ^ val;
                let res = self.a;
                self.set_logic_flags(res);
            }
            AluOp::Or => {
                self.a = a | val;
                let res = self.a;
                self.set_logic_flags(res);
            }
        }
    }

    /// One CB-page rotate/shift. Returns the result; sets every flag.
    fn rot(&mut self, op: RotOp, val: u8) -> u8 {
        let (res, carry) = match op {
            RotOp::Rlc => (val.rotate_left(1), val & 0x80 != 0),
            RotOp::Rrc => (val.rotate_right(1), val & 0x01 != 0),
            RotOp::Rl => ((val << 1) | self.carry(), val & 0x80 != 0),
            RotOp::Rr => ((val >> 1) | (self.carry() << 7), val & 0x01 != 0),
            RotOp::Sla => (val << 1, val & 0x80 != 0),
            RotOp::Sra => ((val >> 1) | (val & 0x80), val & 0x01 != 0),
            RotOp::Sll => ((val << 1) | 0x01, val & 0x80 != 0),
            RotOp::Srl => (val >> 1, val & 0x01 != 0),
        };

        self.set_flag(F_C, carry);
        self.set_flag(F_H, false);
        self.set_flag(F_N, false);
        self.set_flag(F_PV, parity(res));
        self.set_sz(res);
        res
    }

    /// The operand of an 8-bit ALU instruction.
    ///
    /// Fetch stays out of [`alu`](CpuZ80::alu) because this is where the prefix
    /// rules live: only the `(HL)` slot has an indexed form that consumes the
    /// prefix. `DD 84` (the undocumented `ADD A, IXh`) takes the last branch,
    /// reads the remapped register, and then ends the run at the
    /// end-of-instruction prefix check -- exactly as the C++ does.
    fn alu_operand(&mut self, bus: &mut dyn Bus, r: u8, used: &mut PrefixUse) -> u8 {
        if self.prefix_dd && r == 0b110 {
            let addr = self.indexed_addr(bus, true);
            used.dd = true;
            self.mem_read(bus, addr)
        } else if self.prefix_fd && r == 0b110 {
            let addr = self.indexed_addr(bus, false);
            used.fd = true;
            self.mem_read(bus, addr)
        } else {
            self.read_r_or_hl(bus, r)
        }
    }

    /// `INC r` and `DEC r`, which differ only in the direction and three flags.
    fn inc_dec_r(&mut self, bus: &mut dyn Bus, r: u8, used: &mut PrefixUse, inc: bool) {
        let bump = |v: u8| if inc { v.wrapping_add(1) } else { v.wrapping_sub(1) };

        let (old, new) = if self.prefix_dd && r == 0b110 {
            let addr = self.indexed_addr(bus, true);
            let old = self.mem_read(bus, addr);
            self.mem_write(bus, addr, bump(old));
            used.dd = true;
            (old, bump(old))
        } else if self.prefix_fd && r == 0b110 {
            let addr = self.indexed_addr(bus, false);
            let old = self.mem_read(bus, addr);
            self.mem_write(bus, addr, bump(old));
            used.fd = true;
            (old, bump(old))
        } else {
            let old = self.read_r_or_hl(bus, r);
            self.write_r_or_hl(bus, r, bump(old));
            (old, bump(old))
        };

        self.set_flag(F_PV, old == if inc { 0x7f } else { 0x80 });
        self.set_sz(new);
        self.set_flag(F_N, !inc);
        self.set_flag(F_H, if inc { (old & 0x0f) == 0x0f } else { (old & 0x0f) == 0 });
    }

    /// The unprefixed opcode page.
    ///
    /// Decoded by the standard bit split: `x` = bits 7-6, `y` = bits 5-3,
    /// `z` = bits 2-0, with `p` = `y >> 1` and `q` = `y & 1` for the encodings
    /// that use `y` as a register-pair plus a direction bit.
    ///
    /// This page is complete -- every one of the 256 values decodes to
    /// something, so there is no `BadOpcode` path here. `0xcb`, `0xdd`, `0xed`
    /// and `0xfd` never arrive: `step` peels them off first.
    fn exec_base(&mut self, bus: &mut dyn Bus, op: u8, used: &mut PrefixUse) -> StepResult {
        let x = op >> 6;
        let y = (op >> 3) & 0b111;
        let z = op & 0b111;
        let p = y >> 1;
        let q = y & 1;

        match (x, z) {
            (0, 0) => match y {
                0 => {} // NOP
                1 => {
                    // EX AF, AF'
                    let cur = self.af();
                    let alt = self.af_alt();
                    self.set_af(alt);
                    self.set_af_alt(cur);
                }
                2 => {
                    // DJNZ e
                    let rel = self.read_d(bus);
                    self.b = self.b.wrapping_sub(1);
                    if self.b != 0 {
                        self.pc = self.pc.wrapping_add(rel as u16);
                    }
                }
                3 => {
                    // JR e
                    let rel = self.read_d(bus);
                    self.pc = self.pc.wrapping_add(rel as u16);
                }
                // JR cc, e -- NZ, Z, NC, C, which are conditions 0..3
                _ => {
                    let rel = self.read_d(bus);
                    if self.test_cond(y - 4) {
                        self.pc = self.pc.wrapping_add(rel as u16);
                    }
                }
            },

            (0, 1) => {
                if q == 0 {
                    // LD dd, nn -- only the HL slot has an IX/IY form
                    if self.prefix_dd && p == 0b10 {
                        self.ix = self.read_nn(bus);
                        used.dd = true;
                    } else if self.prefix_fd && p == 0b10 {
                        self.iy = self.read_nn(bus);
                        used.fd = true;
                    } else {
                        let nn = self.read_nn(bus);
                        self.write_dd(p, nn);
                    }
                } else {
                    // ADD HL, ss. Only C, N and H are touched.
                    let mut ss = self.read_dd(p);
                    let (base, prefixed) = if self.prefix_dd {
                        used.dd = true;
                        (self.ix, true)
                    } else if self.prefix_fd {
                        used.fd = true;
                        (self.iy, true)
                    } else {
                        (self.hl(), false)
                    };
                    if prefixed && p == 0b10 {
                        ss = base; // ADD IX, IX / ADD IY, IY
                    }
                    let res = base as u32 + ss as u32;
                    self.set_flag(F_C, res > 0xffff);
                    self.set_flag(F_N, false);
                    self.set_flag(F_H, (base & 0xfff) + (ss & 0xfff) > 0xfff);
                    if self.prefix_dd {
                        self.ix = res as u16;
                    } else if self.prefix_fd {
                        self.iy = res as u16;
                    } else {
                        self.set_hl(res as u16);
                    }
                }
            }

            (0, 2) => match y {
                0 => {
                    // LD (BC), A
                    let (addr, a) = (self.bc(), self.a);
                    self.mem_write(bus, addr, a);
                }
                1 => {
                    // LD A, (BC)
                    let addr = self.bc();
                    self.a = self.mem_read(bus, addr);
                }
                2 => {
                    // LD (DE), A
                    let (addr, a) = (self.de(), self.a);
                    self.mem_write(bus, addr, a);
                }
                3 => {
                    // LD A, (DE)
                    let addr = self.de();
                    self.a = self.mem_read(bus, addr);
                }
                4 => {
                    // LD (nn), HL
                    let addr = self.read_nn(bus);
                    if self.prefix_dd {
                        bus.write16(addr as u32, self.ix, Endian::Little);
                        used.dd = true;
                    } else if self.prefix_fd {
                        bus.write16(addr as u32, self.iy, Endian::Little);
                        used.fd = true;
                    } else {
                        let (l, h) = (self.l, self.h);
                        self.mem_write(bus, addr, l);
                        self.mem_write(bus, addr.wrapping_add(1), h);
                    }
                }
                5 => {
                    // LD HL, (nn)
                    let addr = self.read_nn(bus);
                    if self.prefix_dd {
                        self.ix = bus.read16(addr as u32, Endian::Little);
                        used.dd = true;
                    } else if self.prefix_fd {
                        self.iy = bus.read16(addr as u32, Endian::Little);
                        used.fd = true;
                    } else {
                        self.l = self.mem_read(bus, addr);
                        self.h = self.mem_read(bus, addr.wrapping_add(1));
                    }
                }
                6 => {
                    // LD (nn), A
                    let addr = self.read_nn(bus);
                    let a = self.a;
                    self.mem_write(bus, addr, a);
                }
                _ => {
                    // LD A, (nn)
                    let addr = self.read_nn(bus);
                    self.a = self.mem_read(bus, addr);
                }
            },

            (0, 3) => {
                // INC ss / DEC ss -- no flags
                let bump = |v: u16| if q == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) };
                if self.prefix_dd && p == 0b10 {
                    self.ix = bump(self.ix);
                    used.dd = true;
                } else if self.prefix_fd && p == 0b10 {
                    self.iy = bump(self.iy);
                    used.fd = true;
                } else {
                    let val = self.read_dd(p);
                    self.write_dd(p, bump(val));
                }
            }

            (0, 4) => self.inc_dec_r(bus, y, used, true),  // INC r
            (0, 5) => self.inc_dec_r(bus, y, used, false), // DEC r

            (0, 6) => {
                if y == 0b110 {
                    // LD (HL), n. The indexed forms read two immediates: the
                    // displacement first, then the value.
                    if self.prefix_dd {
                        let addr = self.indexed_addr(bus, true);
                        let val = self.read_n(bus);
                        self.mem_write(bus, addr, val);
                        used.dd = true;
                    } else if self.prefix_fd {
                        let addr = self.indexed_addr(bus, false);
                        let val = self.read_n(bus);
                        self.mem_write(bus, addr, val);
                        used.fd = true;
                    } else {
                        let val = self.read_n(bus);
                        let addr = self.hl();
                        self.mem_write(bus, addr, val);
                    }
                } else {
                    // LD r, n. Under a prefix, r = H/L writes the IX/IY half
                    // and then aborts, since nothing consumes the prefix.
                    let val = self.read_n(bus);
                    self.write_r(y, val);
                }
            }

            (0, 7) => match y {
                0 => {
                    // RLCA. The accumulator rotates touch only C, H and N.
                    let a = self.a;
                    self.a = a.rotate_left(1);
                    self.set_flag(F_C, a & 0x80 != 0);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                }
                1 => {
                    // RRCA
                    let a = self.a;
                    self.a = a.rotate_right(1);
                    self.set_flag(F_C, a & 0x01 != 0);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                }
                2 => {
                    // RLA
                    let a = self.a;
                    self.a = (a << 1) | self.carry();
                    self.set_flag(F_C, a & 0x80 != 0);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                }
                3 => {
                    // RRA
                    let a = self.a;
                    self.a = (a >> 1) | (self.carry() << 7);
                    self.set_flag(F_C, a & 0x01 != 0);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                }
                4 => {
                    // DAA. Every flag decision reads the *old* accumulator
                    // while the accumulator itself is being mutated, so `a` is
                    // captured up front and the order here matters.
                    let a = self.a;
                    let mut correction = 0u8;
                    let mut carry = self.flag(F_C);
                    let h_carry = self.flag(F_H);

                    if h_carry || (a & 0x0f) > 9 {
                        correction |= 0x06;
                    }
                    if carry || a > 0x99 {
                        correction |= 0x60;
                        carry = true;
                    }

                    let sub = self.flag(F_N);
                    self.a = if sub {
                        a.wrapping_sub(correction)
                    } else {
                        a.wrapping_add(correction)
                    };

                    self.set_flag(F_C, carry);
                    self.set_flag(F_H, if sub { h_carry && (a & 0x0f) < 6 } else { (a & 0x0f) > 9 });
                    let res = self.a;
                    self.set_sz(res);
                    self.set_flag(F_PV, parity(res));
                }
                5 => {
                    // CPL
                    self.a = !self.a;
                    self.set_flag(F_H, true);
                    self.set_flag(F_N, true);
                }
                6 => {
                    // SCF
                    self.set_flag(F_C, true);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                }
                _ => {
                    // CCF -- H takes the *old* carry, then C inverts
                    let c = self.flag(F_C);
                    self.set_flag(F_H, c);
                    self.set_flag(F_C, !c);
                    self.set_flag(F_N, false);
                }
            },

            // LD r, r' -- plus HALT, which steals the r == r' == (HL) encoding
            (1, _) => {
                let (dst, src) = (y, z);
                if dst == 0b110 && src == 0b110 {
                    // HALT, treated as a NOP
                } else if self.prefix_dd && src == 0b110 {
                    // LD r, (IX+d). NOTE: `d` is added *unsigned* here, while
                    // every other indexed form sign-extends it. Bug in the C++,
                    // preserved deliberately.
                    let d = self.read_n(bus) as u16;
                    let addr = self.ix.wrapping_add(d);
                    let val = self.mem_read(bus, addr);
                    // dst can't be (HL): that combination is HALT, above
                    self.write_r(dst, val);
                    used.dd = true;
                } else if self.prefix_fd && src == 0b110 {
                    // LD r, (IY+d) -- unsigned displacement too
                    let d = self.read_n(bus) as u16;
                    let addr = self.iy.wrapping_add(d);
                    let val = self.mem_read(bus, addr);
                    self.write_r(dst, val);
                    used.fd = true;
                } else if self.prefix_dd && dst == 0b110 {
                    // LD (IX+d), r -- sign-extended, as everywhere else
                    let addr = self.indexed_addr(bus, true);
                    let val = self.read_r(src);
                    self.mem_write(bus, addr, val);
                    used.dd = true;
                } else if self.prefix_fd && dst == 0b110 {
                    // LD (IY+d), r
                    let addr = self.indexed_addr(bus, false);
                    let val = self.read_r(src);
                    self.mem_write(bus, addr, val);
                    used.fd = true;
                } else {
                    let val = self.read_r_or_hl(bus, src);
                    self.write_r_or_hl(bus, dst, val);
                }
            }

            // ALU A, r / (HL) / (IX+d)
            (2, _) => {
                let val = self.alu_operand(bus, z, used);
                self.alu(ALU_OPS[y as usize], val);
            }

            (3, 0) => {
                // RET cc
                if self.test_cond(y) {
                    self.pc = self.pop16(bus);
                }
            }

            (3, 1) => {
                if q == 0 {
                    // POP qq -- the pop happens before the prefix is examined
                    let val = self.pop16(bus);
                    if self.prefix_dd && p == 0b10 {
                        self.ix = val;
                        used.dd = true;
                    } else if self.prefix_fd && p == 0b10 {
                        self.iy = val;
                        used.fd = true;
                    } else {
                        self.write_qq(p, val);
                    }
                } else {
                    match p {
                        0 => self.pc = self.pop16(bus), // RET
                        1 => {
                            // EXX -- BC/DE/HL only, AF has its own instruction
                            std::mem::swap(&mut self.b, &mut self.b_alt);
                            std::mem::swap(&mut self.c, &mut self.c_alt);
                            std::mem::swap(&mut self.d, &mut self.d_alt);
                            std::mem::swap(&mut self.e, &mut self.e_alt);
                            std::mem::swap(&mut self.h, &mut self.h_alt);
                            std::mem::swap(&mut self.l, &mut self.l_alt);
                        }
                        2 => {
                            // JP (HL)
                            if self.prefix_dd {
                                self.pc = self.ix;
                                used.dd = true;
                            } else if self.prefix_fd {
                                self.pc = self.iy;
                                used.fd = true;
                            } else {
                                self.pc = self.hl();
                            }
                        }
                        _ => {
                            // LD SP, HL
                            if self.prefix_dd {
                                self.sp = self.ix;
                                used.dd = true;
                            } else if self.prefix_fd {
                                self.sp = self.iy;
                                used.fd = true;
                            } else {
                                self.sp = self.hl();
                            }
                        }
                    }
                }
            }

            (3, 2) => {
                // JP cc, nn -- the target is always read, taken or not
                let addr = self.read_nn(bus);
                if self.test_cond(y) {
                    self.pc = addr;
                }
            }

            (3, 3) => match y {
                0 => self.pc = self.read_nn(bus), // JP nn
                1 => unreachable!("the cb prefix is peeled off in step"),
                2 => {
                    // OUT (n), A
                    let port = self.read_n(bus);
                    let a = self.a;
                    bus.io_write8(port as u16, a);
                }
                3 => {
                    // IN A, (n)
                    let port = self.read_n(bus);
                    self.a = bus.io_read8(port as u16);
                }
                4 => {
                    // EX (SP), HL
                    let val = self.pop16(bus);
                    if self.prefix_dd {
                        let ix = self.ix;
                        self.push16(bus, ix);
                        self.ix = val;
                        used.dd = true;
                    } else if self.prefix_fd {
                        let iy = self.iy;
                        self.push16(bus, iy);
                        self.iy = val;
                        used.fd = true;
                    } else {
                        let hl = self.hl();
                        self.push16(bus, hl);
                        self.set_hl(val);
                    }
                }
                5 => {
                    // EX DE, HL -- note this ignores the prefix entirely
                    let (de, hl) = (self.de(), self.hl());
                    self.set_de(hl);
                    self.set_hl(de);
                }
                6 => {
                    // DI
                    self.iff1 = false;
                    self.iff2 = false;
                }
                7 => {
                    // EI
                    self.iff1 = true;
                    self.iff2 = true;
                }
                _ => unreachable!("y is three bits"),
            },

            (3, 4) => {
                // CALL cc, nn
                let addr = self.read_nn(bus);
                if self.test_cond(y) {
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = addr;
                }
            }

            (3, 5) => {
                if q == 0 {
                    // PUSH qq
                    if self.prefix_dd && p == 0b10 {
                        let ix = self.ix;
                        self.push16(bus, ix);
                        used.dd = true;
                    } else if self.prefix_fd && p == 0b10 {
                        let iy = self.iy;
                        self.push16(bus, iy);
                        used.fd = true;
                    } else {
                        let val = self.read_qq(p);
                        self.push16(bus, val);
                    }
                } else {
                    // CALL nn. The other three q == 1 encodings in this column
                    // are the DD, ED and FD prefixes, already peeled off.
                    let addr = self.read_nn(bus);
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = addr;
                }
            }

            (3, 6) => {
                // ALU A, n -- same eight operations as the register forms
                let val = self.read_n(bus);
                self.alu(ALU_OPS[y as usize], val);
            }

            (3, 7) => {
                // RST p
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = (y as u16) * 8;
            }

            _ => unreachable!("x is two bits and z is three"),
        }

        StepResult::Ok
    }

    /// `INI` / `INIR` / `IND` / `INDR`.
    fn block_in(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        let val = bus.io_read8(self.c as u16);
        let addr = self.hl();
        self.mem_write(bus, addr, val);
        self.set_hl(if inc { addr.wrapping_add(1) } else { addr.wrapping_sub(1) });
        self.b = self.b.wrapping_sub(1);
        self.finish_block_repeat(repeat);
    }

    /// `OUTI` / `OTIR` / `OUTD` / `OTDR`. Note B is decremented *before* the
    /// memory read here, unlike the input forms.
    fn block_out(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        self.b = self.b.wrapping_sub(1);
        let addr = self.hl();
        let val = self.mem_read(bus, addr);
        bus.io_write8(self.c as u16, val);
        self.set_hl(if inc { addr.wrapping_add(1) } else { addr.wrapping_sub(1) });
        self.finish_block_repeat(repeat);
    }

    /// The shared tail of the IN/OUT block ops. The repeating variants force Z
    /// set and then clear it again when they are about to repeat, which is not
    /// the same as the single-shot `Z = (b == 0)`: it differs when B is already
    /// zero on entry.
    fn finish_block_repeat(&mut self, repeat: bool) {
        if repeat {
            self.set_flag(F_Z, true);
            self.set_flag(F_N, true);
            if self.b != 0 {
                self.pc = self.pc.wrapping_sub(2); // repeat the instruction
                self.set_flag(F_Z, false);
            }
        } else {
            self.set_flag(F_Z, self.b == 0);
            self.set_flag(F_N, true);
        }
    }

    /// `CPI` / `CPIR` / `CPD` / `CPDR`. The comparison never writes A, and C is
    /// left alone.
    fn block_cp(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        let addr = self.hl();
        let val = self.mem_read(bus, addr);
        let a = self.a;
        let res = a.wrapping_sub(val);
        let bc = self.bc().wrapping_sub(1);
        self.set_bc(bc);
        self.set_hl(if inc { addr.wrapping_add(1) } else { addr.wrapping_sub(1) });

        self.set_flag(F_S, (res & 0x80) != 0);
        self.set_flag(F_Z, res == 0);
        self.set_flag(F_H, (a & 0x0f) < (val & 0x0f));
        self.set_flag(F_PV, bc != 0);
        self.set_flag(F_N, true);

        if repeat && bc != 0 && res != 0 {
            self.pc = self.pc.wrapping_sub(2);
        }
    }

    /// `RRD` and `RLD`, which differ only in which nibble goes where.
    fn rotate_decimal(&mut self, bus: &mut dyn Bus, right: bool) {
        let addr = self.hl();
        let mem = self.mem_read(bus, addr);
        let a_low = self.a & 0x0f;

        let (new_mem, new_a_low) = if right {
            ((a_low << 4) | (mem >> 4), mem & 0x0f)
        } else {
            (((mem & 0x0f) << 4) | a_low, mem >> 4)
        };
        self.mem_write(bus, addr, new_mem);
        self.a = (self.a & 0xf0) | new_a_low;

        let a = self.a;
        self.set_sz(a);
        self.set_flag(F_H, false);
        self.set_flag(F_PV, parity(a));
        self.set_flag(F_N, false);
    }

    /// The ED page.
    ///
    /// Deliberately full of holes -- see the module comment. It also never
    /// consumes a DD/FD prefix (the C++ has no prefix handling anywhere in this
    /// page), so `DD ED xx` executes the instruction and *then* ends the run,
    /// which is why this takes no `PrefixUse`.
    fn exec_ed(&mut self, bus: &mut dyn Bus) -> StepResult {
        let op = self.read_n(bus);
        let y = (op >> 3) & 0b111;
        let p = y >> 1;

        match op {
            // IN r, (C)
            0x40 | 0x48 | 0x50 | 0x58 | 0x60 | 0x68 | 0x70 | 0x78 => {
                let val = bus.io_read8(self.c as u16);
                if y != 0b110 {
                    self.write_r(y, val);
                }
                self.set_sz(val);
                self.set_flag(F_PV, parity(val));
                self.set_flag(F_H, false);
                self.set_flag(F_N, false);
            }

            // OUT (C), r -- the 0x71 encoding writes zero, undocumented
            0x41 | 0x49 | 0x51 | 0x59 | 0x61 | 0x69 | 0x71 | 0x79 => {
                let val = if y == 0b110 { 0 } else { self.read_r(y) };
                bus.io_write8(self.c as u16, val);
            }

            // SBC HL, ss -- H and PV use 16-bit-specific expressions
            0x42 | 0x52 | 0x62 | 0x72 => {
                let ss = self.read_dd(p);
                let hl = self.hl();
                let c = self.carry() as u32;
                let res = (hl as u32).wrapping_sub(ss as u32).wrapping_sub(c);

                self.set_flag(F_C, hl as i32 - ss as i32 - (c as i32) < 0);
                self.set_flag(F_N, true);
                self.set_flag(F_PV, ((hl ^ ss) & (hl ^ res as u16) & 0x8000) != 0);
                self.set_flag(F_H, (hl & 0xfff) as i32 - (ss & 0xfff) as i32 - (c as i32) < 0);
                self.set_flag(F_Z, res as u16 == 0);
                self.set_flag(F_S, (res & 0x8000) != 0);
                self.set_hl(res as u16);
            }

            // ADC HL, ss
            0x4a | 0x5a | 0x6a | 0x7a => {
                let ss = self.read_dd(p);
                let hl = self.hl();
                let c = self.carry() as u32;
                let res = hl as u32 + ss as u32 + c;

                self.set_flag(F_C, res > 0xffff);
                self.set_flag(F_N, false);
                self.set_flag(F_PV, ((hl ^ res as u16) & (ss ^ res as u16) & 0x8000) != 0);
                self.set_flag(F_H, (hl & 0xfff) as u32 + (ss & 0xfff) as u32 + c > 0xfff);
                self.set_flag(F_Z, res as u16 == 0);
                self.set_flag(F_S, (res & 0x8000) != 0);
                self.set_hl(res as u16);
            }

            // LD (nn), dd
            0x43 | 0x53 | 0x63 | 0x73 => {
                let val = self.read_dd(p);
                let addr = self.read_nn(bus);
                bus.write16(addr as u32, val, Endian::Little);
            }

            // LD dd, (nn)
            0x4b | 0x5b | 0x6b | 0x7b => {
                let addr = self.read_nn(bus);
                let val = bus.read16(addr as u32, Endian::Little);
                self.write_dd(p, val);
            }

            // NEG. Only this encoding: the seven aliases are holes.
            0x44 => {
                let old = self.a;
                let res = 0u8.wrapping_sub(old);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (old & 0xf) != 0);
                self.set_flag(F_PV, old == 0x80);
                self.set_flag(F_N, true);
                self.set_flag(F_C, old != 0);
                self.a = res;
            }

            // RETI -- a plain RET, with no interrupt-controller notification
            0x4d => self.pc = self.pop16(bus),

            // IM 0 / IM 1 / IM 2, each with its documented alternate encoding.
            // 0x76 and 0x7e are holes.
            0x46 | 0x4e => self.im = 0,
            0x56 | 0x5e => self.im = 1,
            0x66 | 0x6e => self.im = 2,

            0x47 => self.i = self.a, // LD I, A
            0x4f => self.r = self.a, // LD R, A

            // LD A, I / LD A, R -- PV picks up IFF2
            0x57 | 0x5f => {
                self.a = if op == 0x57 { self.i } else { self.r };
                let a = self.a;
                self.set_flag(F_PV, self.iff2);
                self.set_sz(a);
                self.set_flag(F_H, false);
                self.set_flag(F_N, false);
            }

            0x67 => self.rotate_decimal(bus, true),  // RRD
            0x6f => self.rotate_decimal(bus, false), // RLD

            0xa1 => self.block_cp(bus, true, false),  // CPI
            0xa9 => self.block_cp(bus, false, false), // CPD
            0xb1 => self.block_cp(bus, true, true),   // CPIR
            0xb9 => self.block_cp(bus, false, true),  // CPDR

            0xa2 => self.block_in(bus, true, false),  // INI
            0xaa => self.block_in(bus, false, false), // IND
            0xb2 => self.block_in(bus, true, true),   // INIR
            0xba => self.block_in(bus, false, true),  // INDR

            0xa3 => self.block_out(bus, true, false),  // OUTI
            0xab => self.block_out(bus, false, false), // OUTD
            0xb3 => self.block_out(bus, true, true),   // OTIR
            0xbb => self.block_out(bus, false, true),  // OTDR

            // LDIR. LDI, LDD and LDDR are *not* implemented -- leave them as
            // holes. PV is forced clear even when the instruction repeats.
            0xb0 => {
                let src = self.hl();
                let val = self.mem_read(bus, src);
                let dst = self.de();
                self.mem_write(bus, dst, val);

                self.set_hl(src.wrapping_add(1));
                self.set_de(dst.wrapping_add(1));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                if bc != 0 {
                    self.pc = self.pc.wrapping_sub(2); // repeat the instruction
                }

                self.set_flag(F_H, false);
                self.set_flag(F_PV, false);
                self.set_flag(F_N, false);
            }

            _ => {
                eprintln!("unhandled ED prefixed-opcode {op:#x}");
                return StepResult::BadOpcode;
            }
        }

        StepResult::Ok
    }

    /// The CB page: rotates, shifts and bit operations.
    ///
    /// Complete -- all 256 values decode. Under a DD/FD prefix the
    /// displacement is read here, before the opcode, and **most of the
    /// rotates ignore the prefix**: only RLC, BIT, RES and SET have an indexed
    /// form. The others fall through to the plain `(HL)` path and then end the
    /// run at the prefix check, having already read the displacement and
    /// touched `(HL)`. Preserved so traces match right up to the failure.
    /// `(IX+d)` / `(IY+d)` for the four CB operations that honour the prefix,
    /// marking it consumed. DD wins when both are set, which leaves FD
    /// unconsumed and ends the run.
    fn cb_indexed_addr(&mut self, d: i8, used: &mut PrefixUse) -> Option<u16> {
        if self.prefix_dd {
            used.dd = true;
            Some(self.ix.wrapping_add(d as u16))
        } else if self.prefix_fd {
            used.fd = true;
            Some(self.iy.wrapping_add(d as u16))
        } else {
            None
        }
    }

    fn exec_cb(&mut self, bus: &mut dyn Bus, used: &mut PrefixUse) -> StepResult {
        let d = if self.prefix_dd || self.prefix_fd { self.read_d(bus) } else { 0 };
        let op = self.read_n(bus);

        let x = op >> 6;
        let y = (op >> 3) & 0b111;
        let z = op & 0b111;

        match x {
            0 => {
                // RLC is the only rotate with an indexed form; y != 0 falls
                // through to the plain path even under a prefix, and aborts.
                let addr = if y == 0 { self.cb_indexed_addr(d, used) } else { None };
                match addr {
                    Some(addr) => {
                        let val = self.mem_read(bus, addr);
                        let res = self.rot(RotOp::Rlc, val);
                        self.mem_write(bus, addr, res);
                        if z != 0b110 {
                            self.write_r(z, res); // undocumented writeback
                        }
                    }
                    None => {
                        let val = self.read_r_or_hl(bus, z);
                        let res = self.rot(ROT_OPS[y as usize], val);
                        self.write_r_or_hl(bus, z, res);
                    }
                }
            }

            1 => {
                // BIT b, r. Neither PV nor C is touched, and S is only ever set
                // by bit 7.
                let val = match self.cb_indexed_addr(d, used) {
                    Some(addr) => self.mem_read(bus, addr),
                    None => self.read_r_or_hl(bus, z),
                };
                self.set_flag(F_Z, (val & (1 << y)) == 0);
                self.set_flag(F_H, true);
                self.set_flag(F_N, false);
                self.set_flag(F_S, y == 7 && (val & 0x80) != 0);
            }

            // RES b, r and SET b, r -- no flags
            _ => {
                let set = x == 3;
                let apply = |v: u8| if set { v | (1 << y) } else { v & !(1 << y) };
                match self.cb_indexed_addr(d, used) {
                    Some(addr) => {
                        let val = apply(self.mem_read(bus, addr));
                        self.mem_write(bus, addr, val);
                        if z != 0b110 {
                            self.write_r(z, val); // undocumented writeback
                        }
                    }
                    None => {
                        let val = apply(self.read_r_or_hl(bus, z));
                        self.write_r_or_hl(bus, z, val);
                    }
                }
            }
        }

        StepResult::Ok
    }
}

impl Cpu for CpuZ80 {
    /// No reset vector, unlike the 6800/6809: the Z80 starts at 0.
    fn reset(&mut self, _bus: &mut dyn Bus) {
        *self = CpuZ80::default();
    }

    /// Exactly one instruction, prefixes included.
    ///
    /// The C++ does this with a `restart:`/`decode:` label pair and three
    /// `goto`s, but prefix resolution already happens within one call there, so
    /// one `step` is still one instruction and the cycle-limit and trace
    /// semantics carry over unchanged.
    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        self.prefix_dd = false;
        self.prefix_fd = false;
        let mut used = PrefixUse::default();

        // Interrupt entry. The RC2014's SIO drives this for real -- its
        // console input is interrupt-driven and works no other way. IM 0 and
        // IM 2 fall back to IM 1's `rst 0x38`, as the C++ does; no machine here
        // selects either, and IM 2 would need a device-supplied vector on the
        // bus that nothing offers.
        let ints = bus.poll_interrupts();
        let op = if ints.irq && self.iff1 {
            self.iff1 = false;
            self.iff2 = false;
            0xff // rst 0x38
        } else {
            // DD and FD can stack. Both stick, and DD wins the register remap
            // in read_r/write_r; but only one of them can be consumed, so
            // `DD FD 21 nn nn` loads IX and then ends the run below.
            loop {
                let op = self.read_n(bus);
                match op {
                    0xdd => self.prefix_dd = true,
                    0xfd => self.prefix_fd = true,
                    _ => break op,
                }
            }
        };

        let result = match op {
            0xed => self.exec_ed(bus),
            0xcb => self.exec_cb(bus, &mut used),
            _ => self.exec_base(bus, op, &mut used),
        };
        if result != StepResult::Ok {
            return result;
        }

        // An instruction that saw a DD/FD prefix but did nothing with it ends
        // the run. This is not a corner case: most DD/FD combinations land
        // here, several of them after already reading operands and writing
        // memory, so it decides trace *length* as well as content.
        if self.prefix_dd && !used.dd {
            eprintln!("unhandled opcode dd prefix");
            return StepResult::BadOpcode;
        }
        if self.prefix_fd && !used.fd {
            eprintln!("unhandled opcode fd prefix");
            return StepResult::BadOpcode;
        }

        StepResult::Ok
    }

    fn dump(&self) {
        println!(
            "f 0x{:02x} ({}{}{}{}{}{}) a 0x{:02x} b 0x{:02x} c 0x{:02x} d 0x{:02x} e 0x{:02x} h 0x{:02x} l 0x{:02x} sp 0x{:04x} ix 0x{:04x} iy 0x{:04x} pc 0x{:04x}",
            self.f,
            if self.flag(F_C) { 'c' } else { ' ' },
            if self.flag(F_N) { 'n' } else { ' ' },
            if self.flag(F_PV) { 'p' } else { ' ' },
            if self.flag(F_H) { 'h' } else { ' ' },
            if self.flag(F_Z) { 'z' } else { ' ' },
            if self.flag(F_S) { 's' } else { ' ' },
            self.a, self.b, self.c, self.d, self.e, self.h, self.l,
            self.sp, self.ix, self.iy, self.pc
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            out,
            "PC={:04x} AF={:02x}{:02x} BC={:02x}{:02x} DE={:02x}{:02x} HL={:02x}{:02x} IX={:04x} IY={:04x} SP={:04x}",
            self.pc, self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l, self.ix, self.iy, self.sp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{run_steps, TestBus};

    /// Load a hand-assembled program at 0 and reset into it -- the z80 has no
    /// reset vector.
    fn boot(prog: &[u8]) -> (CpuZ80, TestBus) {
        let mut bus = TestBus::new();
        bus.load(0x0000, prog);
        let mut cpu = CpuZ80::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn reset_starts_at_zero_with_interrupt_mode_one() {
        let mut bus = TestBus::new();
        let mut cpu = CpuZ80::new();
        cpu.a = 0xff;
        cpu.ix = 0xffff;
        cpu.reset(&mut bus);
        assert_eq!(cpu.pc, 0);
        assert_eq!((cpu.a, cpu.ix, cpu.sp), (0, 0, 0));
        // the C++ `mRegs = {}` leaves the in-class initializer for im alone
        assert_eq!(cpu.im, 1);
    }

    /// Smoke test: sum four bytes through a djnz loop and store the result.
    #[test]
    fn sums_a_table_with_a_djnz_loop() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x31, 0x00, 0x80,       // 0000  ld   sp, 0x8000
            0x21, 0x00, 0x01,       // 0003  ld   hl, 0x0100
            0x06, 0x04,             // 0006  ld   b, 4
            0xaf,                   // 0008  xor  a
            0x86,                   // 0009  add  a, (hl)     <- loop
            0x23,                   // 000a  inc  hl
            0x10, 0xfc,             // 000b  djnz loop
            0x32, 0x00, 0x02,       // 000d  ld   (0x0200), a
        ]);
        bus.load(0x0100, &[0x01, 0x02, 0x03, 0x04]);

        // 4 setup + 4 iterations of 3 + the store
        run_steps(&mut cpu, &mut bus, 17);

        assert_eq!(cpu.a, 0x0a);
        assert_eq!(cpu.hl(), 0x0104);
        assert_eq!(cpu.b, 0);
        assert_eq!(cpu.sp, 0x8000);
        assert_eq!(cpu.pc, 0x0010);
        assert_eq!(bus.mem[0x0200], 0x0a);
    }

    /// `LD r, (IX+d)` adds the displacement **unsigned**, unlike every other
    /// indexed form. Bug in the C++, preserved deliberately: a real z80 would
    /// read 0x00ff here.
    #[test]
    fn ld_r_indexed_adds_the_displacement_unsigned() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x00, 0x01, // ld ix, 0x0100
            0xdd, 0x7e, 0xff,       // ld a, (ix-1)
        ]);
        bus.mem[0x01ff] = 0xaa; // where this core looks
        bus.mem[0x00ff] = 0x55; // where a real z80 would

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0xaa);
    }

    /// The store direction is the contrast: `LD (IX+d), r` sign-extends, as
    /// every indexed form other than the one above does.
    #[test]
    fn ld_indexed_r_sign_extends_the_displacement() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x00, 0x01, // ld ix, 0x0100
            0x3e, 0x5a,             // ld a, 0x5a
            0xdd, 0x77, 0xff,       // ld (ix-1), a
        ]);

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.mem[0x00ff], 0x5a);
        assert_eq!(bus.mem[0x01ff], 0x00);
    }

    #[test]
    fn exx_swaps_the_alternate_register_set() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x01, 0x22, 0x11,       // ld bc, 0x1122
            0x11, 0x44, 0x33,       // ld de, 0x3344
            0x21, 0x66, 0x55,       // ld hl, 0x5566
            0xd9,                   // exx
            0x01, 0xbb, 0xaa,       // ld bc, 0xaabb
            0xd9,                   // exx
        ]);

        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!((cpu.bc(), cpu.de(), cpu.hl()), (0, 0, 0));

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!((cpu.bc(), cpu.de(), cpu.hl()), (0x1122, 0x3344, 0x5566));
        assert_eq!(cpu.b_alt, 0xaa);
        assert_eq!(cpu.c_alt, 0xbb);
    }

    /// AF has its own exchange, which EXX must not touch.
    #[test]
    fn ex_af_swaps_only_the_accumulator_and_flags() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x3e, 0x12,             // ld a, 0x12
            0x37,                   // scf
            0x08,                   // ex af, af'
            0x3e, 0x34,             // ld a, 0x34
            0x08,                   // ex af, af'
        ]);

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.a, cpu.f), (0, 0));
        assert_eq!(cpu.af_alt(), 0x1201);

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!((cpu.a, cpu.f), (0x12, F_C));
        assert_eq!(cpu.af_alt(), 0x3400);
    }

    /// HALT is a NOP here -- nothing in the tree drives an interrupt line, so
    /// halting would deadlock the run rather than end it.
    #[test]
    fn halt_is_a_nop() {
        let (mut cpu, mut bus) = boot(&[0x76, 0x3e, 0x42]); // halt ; ld a, 0x42
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.pc, 0x0003);
    }

    #[test]
    fn ldir_copies_a_block() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x21, 0x00, 0x01,       // ld hl, 0x0100
            0x11, 0x00, 0x02,       // ld de, 0x0200
            0x01, 0x04, 0x00,       // ld bc, 4
            0xed, 0xb0,             // ldir
        ]);
        bus.load(0x0100, &[0xde, 0xad, 0xbe, 0xef]);

        // ldir rewinds pc by two to repeat, so it steps once per byte
        run_steps(&mut cpu, &mut bus, 7);

        assert_eq!(&bus.mem[0x0200..0x0204], &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!((cpu.hl(), cpu.de(), cpu.bc()), (0x0104, 0x0204, 0));
        assert_eq!(cpu.pc, 0x000b);
    }

    /// The ED page is deliberately incomplete: LDI, LDD and LDDR are holes even
    /// though LDIR is implemented. That is inherited from the C++ and kept; this
    /// test is what makes filling one in a visible change rather than a silent
    /// one.
    #[test]
    fn the_unimplemented_ed_block_moves_stop_the_run() {
        for op in [0xa0u8, 0xa8, 0xb8] {
            let (mut cpu, mut bus) = boot(&[0xed, op]);
            assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode, "ed {op:#04x}");
        }
    }

    /// An instruction that saw a DD/FD prefix but had no use for it ends the
    /// run -- not an error path so much as the decode's shape, since it decides
    /// how far a run gets.
    #[test]
    fn an_unconsumed_index_prefix_stops_the_run() {
        let (mut cpu, mut bus) = boot(&[0xdd, 0x00]); // dd nop
        assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode);

        // both prefixes stick, but only one of them can be consumed
        let (mut cpu, mut bus) = boot(&[0xdd, 0xfd, 0x21, 0x34, 0x12]); // dd fd ld ix, nn
        assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode);
        assert_eq!(cpu.ix, 0x1234, "the instruction still ran");
    }

    #[test]
    fn port_io_reaches_the_bus() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x3e, 0x5a,             // ld a, 0x5a
            0xd3, 0x10,             // out (0x10), a
            0xdb, 0x20,             // in  a, (0x20)
        ]);
        bus.ports[0x20] = 0xa5;

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.io_writes, vec![(0x10, 0x5a)]);
        assert_eq!(cpu.a, 0xa5);
    }

    #[test]
    fn add_sets_carry_half_carry_and_overflow() {
        let add = |a: u8, n: u8| {
            let (mut cpu, mut bus) = boot(&[0x3e, a, 0xc6, n]); // ld a, a ; add a, n
            run_steps(&mut cpu, &mut bus, 2);
            cpu
        };

        // half carry out of bit 3, nothing else
        let cpu = add(0x0f, 0x01);
        assert_eq!(cpu.a, 0x10);
        assert_eq!(
            (cpu.flag(F_C), cpu.flag(F_H), cpu.flag(F_Z), cpu.flag(F_S), cpu.flag(F_PV)),
            (false, true, false, false, false)
        );

        // carry out of bit 7, result zero
        let cpu = add(0xff, 0x01);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(
            (cpu.flag(F_C), cpu.flag(F_H), cpu.flag(F_Z), cpu.flag(F_S), cpu.flag(F_PV)),
            (true, true, true, false, false)
        );

        // signed overflow: PV, not C
        let cpu = add(0x7f, 0x01);
        assert_eq!(cpu.a, 0x80);
        assert_eq!(
            (cpu.flag(F_C), cpu.flag(F_H), cpu.flag(F_Z), cpu.flag(F_S), cpu.flag(F_PV)),
            (false, true, false, true, true)
        );

        // N is always cleared by add
        assert!(!add(0x01, 0x01).flag(F_N));
    }
}
