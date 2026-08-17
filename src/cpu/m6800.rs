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
//! Motorola 6800 interpreter core.
//!
//! A direct port of `cpu/cpu6800.cpp`, which is table-driven: a 256-entry
//! decode table indexed by opcode, then a switch on the addressing mode to
//! fetch the operand and a switch on the operation to execute it.
//!
//! Behaviour is deliberately preserved bug-for-bug, because the C++ is the
//! trace-diff oracle. Where this core is knowingly wrong about a real 6800 it
//! is called out in a comment; do not "fix" those without also changing the
//! C++ and re-baselining the oracle.

use super::{Cpu, StepResult};
use crate::bus::{Bus, Endian};
use std::io::Write;

const CC_C: u8 = 0x01;
const CC_V: u8 = 0x02;
const CC_Z: u8 = 0x04;
const CC_N: u8 = 0x08;
const CC_I: u8 = 0x10;
const CC_H: u8 = 0x20;

const COND_A: u8 = 0x0;
const COND_HI: u8 = 0x2;
const COND_LS: u8 = 0x3;
const COND_CC: u8 = 0x4;
const COND_CS: u8 = 0x5;
const COND_NE: u8 = 0x6;
const COND_EQ: u8 = 0x7;
const COND_VC: u8 = 0x8;
const COND_VS: u8 = 0x9;
const COND_PL: u8 = 0xa;
const COND_MI: u8 = 0xb;
const COND_GE: u8 = 0xc;
const COND_LT: u8 = 0xd;
const COND_GT: u8 = 0xe;
const COND_LE: u8 = 0xf;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum AddrMode {
    Implied,
    Immediate,
    Direct,
    Extended,
    Indexed,
    Branch,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Op {
    Bad,
    Add,
    AddAccum,
    Adc,
    Sub,
    SubAccum,
    Sbc,
    Cmp,
    CmpAccum,
    And,
    Bit,
    Eor,
    Or,
    Nop,
    Clr,
    Com,
    Neg,
    Dec,
    Inc,
    Tst,
    Asl,
    Asr,
    Lsr,
    Rol,
    Ror,
    Tfr,
    TfrCc,
    Push,
    Pull,
    Bra,
    Bsr,
    Jmp,
    Jsr,
    Rts,
    Ld,
    St,
    SetCc,
    ClrCc,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Reg {
    A,
    B,
    Ix,
    Pc,
    Sp,
    Cc,
}

#[derive(Copy, Clone)]
struct OpDecode {
    #[allow(dead_code)]
    name: &'static str,
    mode: AddrMode,
    width: u8,
    op: Op,
    target: Reg,
    // The C++ overlays these three in a union; only the one relevant to the
    // operation is ever read, so separate fields are equivalent.
    cond: u8,
    calcaddr: bool,
    cc_flag: u8,
}

const BAD: OpDecode = OpDecode {
    name: "???",
    mode: AddrMode::Implied,
    width: 1,
    op: Op::Bad,
    target: Reg::A,
    cond: 0,
    calcaddr: false,
    cc_flag: 0,
};

macro_rules! op {
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident) => {
        OpDecode {
            name: $name,
            mode: AddrMode::$mode,
            width: $w,
            op: Op::$op,
            target: Reg::$t,
            cond: 0,
            calcaddr: false,
            cc_flag: 0,
        }
    };
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident, addr) => {
        OpDecode { calcaddr: true, ..op!($name, $mode, $w, $op, $t) }
    };
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident, cond = $c:expr) => {
        OpDecode { cond: $c, ..op!($name, $mode, $w, $op, $t) }
    };
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident, cc = $f:expr) => {
        OpDecode { cc_flag: $f, ..op!($name, $mode, $w, $op, $t) }
    };
}

#[rustfmt::skip]
const fn build_ops() -> [OpDecode; 256] {
    let mut t = [BAD; 256];

    // alu ops
    t[0x8b] = op!("adda", Immediate, 1, Add, A);
    t[0xcb] = op!("addb", Immediate, 1, Add, B);
    t[0x9b] = op!("adda", Direct,    1, Add, A);
    t[0xdb] = op!("addb", Direct,    1, Add, B);
    t[0xab] = op!("adda", Indexed,   1, Add, A);
    t[0xeb] = op!("addb", Indexed,   1, Add, B);
    t[0xbb] = op!("adda", Extended,  1, Add, A);
    t[0xfb] = op!("addb", Extended,  1, Add, B);

    t[0x1b] = op!("aba", Implied, 1, AddAccum, A);

    t[0x89] = op!("adca", Immediate, 1, Adc, A);
    t[0xc9] = op!("adcb", Immediate, 1, Adc, B);
    t[0x99] = op!("adca", Direct,    1, Adc, A);
    t[0xd9] = op!("adcb", Direct,    1, Adc, B);
    t[0xa9] = op!("adca", Indexed,   1, Adc, A);
    t[0xe9] = op!("adcb", Indexed,   1, Adc, B);
    t[0xb9] = op!("adca", Extended,  1, Adc, A);
    t[0xf9] = op!("adcb", Extended,  1, Adc, B);

    t[0x80] = op!("suba", Immediate, 1, Sub, A);
    t[0xc0] = op!("subb", Immediate, 1, Sub, B);
    t[0x90] = op!("suba", Direct,    1, Sub, A);
    t[0xd0] = op!("subb", Direct,    1, Sub, B);
    t[0xa0] = op!("suba", Indexed,   1, Sub, A);
    t[0xe0] = op!("subb", Indexed,   1, Sub, B);
    t[0xb0] = op!("suba", Extended,  1, Sub, A);
    t[0xf0] = op!("subb", Extended,  1, Sub, B);

    t[0x10] = op!("sba", Implied, 1, SubAccum, A);

    t[0x82] = op!("sbca", Immediate, 1, Sbc, A);
    t[0xc2] = op!("sbcb", Immediate, 1, Sbc, B);
    t[0x92] = op!("sbca", Direct,    1, Sbc, A);
    t[0xd2] = op!("sbcb", Direct,    1, Sbc, B);
    t[0xa2] = op!("sbca", Indexed,   1, Sbc, A);
    t[0xe2] = op!("sbcb", Indexed,   1, Sbc, B);
    t[0xb2] = op!("sbca", Extended,  1, Sbc, A);
    t[0xf2] = op!("sbcb", Extended,  1, Sbc, B);

    t[0x81] = op!("cmpa", Immediate, 1, Cmp, A);
    t[0xc1] = op!("cmpb", Immediate, 1, Cmp, B);
    t[0x8c] = op!("cpx",  Immediate, 2, Cmp, Ix);
    t[0x91] = op!("cmpa", Direct,    1, Cmp, A);
    t[0xd1] = op!("cmpb", Direct,    1, Cmp, B);
    t[0x9c] = op!("cpx",  Direct,    2, Cmp, Ix);
    t[0xa1] = op!("cmpa", Indexed,   1, Cmp, A);
    t[0xe1] = op!("cmpb", Indexed,   1, Cmp, B);
    t[0xac] = op!("cpx",  Indexed,   2, Cmp, Ix);
    t[0xb1] = op!("cmpa", Extended,  1, Cmp, A);
    t[0xf1] = op!("cmpb", Extended,  1, Cmp, B);
    t[0xbc] = op!("cpx",  Extended,  2, Cmp, Ix);

    t[0x11] = op!("cba", Implied, 1, CmpAccum, A);

    t[0x84] = op!("anda", Immediate, 1, And, A);
    t[0xc4] = op!("andb", Immediate, 1, And, B);
    t[0x94] = op!("anda", Direct,    1, And, A);
    t[0xd4] = op!("andb", Direct,    1, And, B);
    t[0xa4] = op!("anda", Indexed,   1, And, A);
    t[0xe4] = op!("andb", Indexed,   1, And, B);
    t[0xb4] = op!("anda", Extended,  1, And, A);
    t[0xf4] = op!("andb", Extended,  1, And, B);

    t[0x85] = op!("bita", Immediate, 1, Bit, A);
    t[0xc5] = op!("bitb", Immediate, 1, Bit, B);
    t[0x95] = op!("bita", Direct,    1, Bit, A);
    t[0xd5] = op!("bitb", Direct,    1, Bit, B);
    t[0xa5] = op!("bita", Indexed,   1, Bit, A);
    t[0xe5] = op!("bitb", Indexed,   1, Bit, B);
    t[0xb5] = op!("bita", Extended,  1, Bit, A);
    t[0xf5] = op!("bitb", Extended,  1, Bit, B);

    t[0x88] = op!("eora", Immediate, 1, Eor, A);
    t[0xc8] = op!("eorb", Immediate, 1, Eor, B);
    t[0x98] = op!("eora", Direct,    1, Eor, A);
    t[0xd8] = op!("eorb", Direct,    1, Eor, B);
    t[0xa8] = op!("eora", Indexed,   1, Eor, A);
    t[0xe8] = op!("eorb", Indexed,   1, Eor, B);
    t[0xb8] = op!("eora", Extended,  1, Eor, A);
    t[0xf8] = op!("eorb", Extended,  1, Eor, B);

    t[0x8a] = op!("ora", Immediate, 1, Or, A);
    t[0xca] = op!("orb", Immediate, 1, Or, B);
    t[0x9a] = op!("ora", Direct,    1, Or, A);
    t[0xda] = op!("orb", Direct,    1, Or, B);
    t[0xaa] = op!("ora", Indexed,   1, Or, A);
    t[0xea] = op!("orb", Indexed,   1, Or, B);
    t[0xba] = op!("ora", Extended,  1, Or, A);
    t[0xfa] = op!("orb", Extended,  1, Or, B);

    // misc
    t[0x01] = op!("nop", Implied, 1, Nop, A);

    t[0x16] = op!("tab", Implied, 1, Tfr, B);
    t[0x17] = op!("tba", Implied, 1, Tfr, A);

    t[0x35] = op!("txs", Implied, 2, Tfr, Sp);
    t[0x30] = op!("tsx", Implied, 2, Tfr, Ix);

    t[0x07] = op!("tpa", Implied, 1, TfrCc, A);
    t[0x06] = op!("tap", Implied, 1, TfrCc, Cc);

    t[0x0b] = op!("sev", Implied, 1, SetCc, Pc, cc = CC_V);
    t[0x0d] = op!("sec", Implied, 1, SetCc, Pc, cc = CC_C);
    t[0x0f] = op!("sei", Implied, 1, SetCc, Pc, cc = CC_I);

    t[0x0a] = op!("clv", Implied, 1, ClrCc, Pc, cc = CC_V);
    t[0x0c] = op!("clc", Implied, 1, ClrCc, Pc, cc = CC_C);
    t[0x0e] = op!("cli", Implied, 1, ClrCc, Pc, cc = CC_I);

    t[0x4f] = op!("clra", Implied,  1, Clr, A);
    t[0x5f] = op!("clrb", Implied,  1, Clr, B);
    t[0x6f] = op!("clr",  Indexed,  1, Clr, A, addr);
    t[0x7f] = op!("clr",  Extended, 1, Clr, A, addr);

    t[0x43] = op!("coma", Implied,  1, Com, A);
    t[0x53] = op!("comb", Implied,  1, Com, B);
    t[0x63] = op!("com",  Indexed,  1, Com, A, addr);
    t[0x73] = op!("com",  Extended, 1, Com, A, addr);

    t[0x40] = op!("nega", Implied,  1, Neg, A);
    t[0x50] = op!("negb", Implied,  1, Neg, B);
    t[0x60] = op!("neg",  Indexed,  1, Neg, A, addr);
    t[0x70] = op!("neg",  Extended, 1, Neg, A, addr);

    t[0x4a] = op!("deca", Implied,  1, Dec, A);
    t[0x5a] = op!("decb", Implied,  1, Dec, B);
    t[0x6a] = op!("dec",  Indexed,  1, Dec, A, addr);
    t[0x7a] = op!("dec",  Extended, 1, Dec, A, addr);
    t[0x34] = op!("des",  Implied,  2, Dec, Sp);
    t[0x09] = op!("dex",  Implied,  2, Dec, Ix);

    t[0x4c] = op!("inca", Implied,  1, Inc, A);
    t[0x5c] = op!("incb", Implied,  1, Inc, B);
    t[0x6c] = op!("inc",  Indexed,  1, Inc, A, addr);
    t[0x7c] = op!("inc",  Extended, 1, Inc, A, addr);
    t[0x31] = op!("ins",  Implied,  2, Inc, Sp);
    t[0x08] = op!("inx",  Implied,  2, Inc, Ix);

    t[0x48] = op!("asla", Implied,  1, Asl, A);
    t[0x58] = op!("aslb", Implied,  1, Asl, B);
    t[0x68] = op!("asl",  Indexed,  1, Asl, A, addr);
    t[0x78] = op!("asl",  Extended, 1, Asl, A, addr);

    t[0x47] = op!("asra", Implied,  1, Asr, A);
    t[0x57] = op!("asrb", Implied,  1, Asr, B);
    t[0x67] = op!("asr",  Indexed,  1, Asr, A, addr);
    t[0x77] = op!("asr",  Extended, 1, Asr, A, addr);

    t[0x44] = op!("lsra", Implied,  1, Lsr, A);
    t[0x54] = op!("lsrb", Implied,  1, Lsr, B);
    t[0x64] = op!("lsr",  Indexed,  1, Lsr, A, addr);
    t[0x74] = op!("lsr",  Extended, 1, Lsr, A, addr);

    t[0x49] = op!("rola", Implied,  1, Rol, A);
    t[0x59] = op!("rolb", Implied,  1, Rol, B);
    t[0x69] = op!("rol",  Indexed,  1, Rol, A, addr);
    t[0x79] = op!("rol",  Extended, 1, Rol, A, addr);

    t[0x46] = op!("rora", Implied,  1, Ror, A);
    t[0x56] = op!("rorb", Implied,  1, Ror, B);
    t[0x66] = op!("ror",  Indexed,  1, Ror, A, addr);
    t[0x76] = op!("ror",  Extended, 1, Ror, A, addr);

    t[0x4d] = op!("tsta", Implied,  1, Tst, A);
    t[0x5d] = op!("tstb", Implied,  1, Tst, B);
    t[0x6d] = op!("tst",  Indexed,  1, Tst, A, addr);
    t[0x7d] = op!("tst",  Extended, 1, Tst, A, addr);

    // push/pull
    t[0x36] = op!("psha", Implied, 1, Push, A);
    t[0x37] = op!("pshb", Implied, 1, Push, B);
    t[0x32] = op!("pula", Implied, 1, Pull, A);
    t[0x33] = op!("pulb", Implied, 1, Pull, B);

    // loads
    t[0x86] = op!("lda", Immediate, 1, Ld, A);
    t[0xc6] = op!("ldb", Immediate, 1, Ld, B);
    t[0x8e] = op!("lds", Immediate, 2, Ld, Sp);
    t[0xce] = op!("ldx", Immediate, 2, Ld, Ix);

    t[0x96] = op!("lda", Direct, 1, Ld, A);
    t[0xd6] = op!("ldb", Direct, 1, Ld, B);
    t[0x9e] = op!("lds", Direct, 2, Ld, Sp);
    t[0xde] = op!("ldx", Direct, 2, Ld, Ix);

    t[0xb6] = op!("lda", Extended, 1, Ld, A);
    t[0xf6] = op!("ldb", Extended, 1, Ld, B);
    t[0xbe] = op!("lds", Extended, 2, Ld, Sp);
    t[0xfe] = op!("ldx", Extended, 2, Ld, Ix);

    t[0xa6] = op!("lda", Indexed, 1, Ld, A);
    t[0xe6] = op!("ldb", Indexed, 1, Ld, B);
    t[0xae] = op!("lds", Indexed, 2, Ld, Sp);
    t[0xee] = op!("ldx", Indexed, 2, Ld, Ix);

    // stores
    t[0x97] = op!("sta", Direct, 1, St, A,  addr);
    t[0xd7] = op!("stb", Direct, 1, St, B,  addr);
    t[0x9f] = op!("sts", Direct, 2, St, Sp, addr);
    t[0xdf] = op!("stx", Direct, 2, St, Ix, addr);

    t[0xb7] = op!("sta", Extended, 1, St, A,  addr);
    t[0xf7] = op!("stb", Extended, 1, St, B,  addr);
    t[0xbf] = op!("sts", Extended, 2, St, Sp, addr);
    t[0xff] = op!("stx", Extended, 2, St, Ix, addr);

    t[0xa7] = op!("sta", Indexed, 1, St, A,  addr);
    t[0xe7] = op!("stb", Indexed, 1, St, B,  addr);
    t[0xaf] = op!("sts", Indexed, 2, St, Sp, addr);
    t[0xef] = op!("stx", Indexed, 2, St, Ix, addr);

    // branches
    t[0x20] = op!("bra", Branch, 1, Bra, Pc, cond = COND_A);
    t[0x22] = op!("bhi", Branch, 1, Bra, Pc, cond = COND_HI);
    t[0x23] = op!("bls", Branch, 1, Bra, Pc, cond = COND_LS);
    t[0x24] = op!("bcc", Branch, 1, Bra, Pc, cond = COND_CC);
    t[0x25] = op!("bcs", Branch, 1, Bra, Pc, cond = COND_CS);
    t[0x26] = op!("bne", Branch, 1, Bra, Pc, cond = COND_NE);
    t[0x27] = op!("beq", Branch, 1, Bra, Pc, cond = COND_EQ);
    t[0x28] = op!("bvc", Branch, 1, Bra, Pc, cond = COND_VC);
    t[0x29] = op!("bvs", Branch, 1, Bra, Pc, cond = COND_VS);
    t[0x2a] = op!("bpl", Branch, 1, Bra, Pc, cond = COND_PL);
    t[0x2b] = op!("bmi", Branch, 1, Bra, Pc, cond = COND_MI);
    t[0x2c] = op!("bge", Branch, 1, Bra, Pc, cond = COND_GE);
    t[0x2d] = op!("blt", Branch, 1, Bra, Pc, cond = COND_LT);
    t[0x2e] = op!("bgt", Branch, 1, Bra, Pc, cond = COND_GT);
    t[0x2f] = op!("ble", Branch, 1, Bra, Pc, cond = COND_LE);
    t[0x8d] = op!("bsr", Branch, 1, Bsr, Pc);

    t[0x6e] = op!("jmp", Indexed,  1, Jmp, Pc, addr);
    t[0x7e] = op!("jmp", Extended, 1, Jmp, Pc, addr);

    t[0xad] = op!("jsr", Indexed,  1, Jsr, Pc, addr);
    t[0xbd] = op!("jsr", Extended, 1, Jsr, Pc, addr);

    t[0x39] = op!("rts", Implied, 1, Rts, Pc);

    t
}

static OPS: [OpDecode; 256] = build_ops();

pub struct Cpu6800 {
    a: u8,
    b: u8,
    ix: u16,
    pc: u16,
    sp: u16,
    cc: u8,
}

impl Default for Cpu6800 {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu6800 {
    pub fn new() -> Self {
        Cpu6800 { a: 0, b: 0, ix: 0, pc: 0, sp: 0, cc: 0 }
    }

    fn get_reg(&self, r: Reg) -> u16 {
        match r {
            Reg::A => self.a as u16,
            Reg::B => self.b as u16,
            Reg::Ix => self.ix,
            Reg::Sp => self.sp,
            Reg::Pc => self.pc,
            Reg::Cc => self.cc as u16,
        }
    }

    fn put_reg(&mut self, r: Reg, val: u16) {
        match r {
            Reg::A => self.a = val as u8,
            Reg::B => self.b = val as u8,
            Reg::Ix => self.ix = val,
            Reg::Sp => self.sp = val,
            Reg::Pc => self.pc = val,
            Reg::Cc => self.cc = val as u8,
        }
    }

    fn set_cc(&mut self, bit: u8, on: bool) {
        if on {
            self.cc |= bit;
        } else {
            self.cc &= !bit;
        }
    }

    fn cc_set(&self, bit: u8) -> bool {
        self.cc & bit != 0
    }

    // Flag helpers, mirroring the SET_* macros. All operate on the 32-bit
    // intermediate so carry/overflow can be read out of bits 8/16.
    fn set_z1(&mut self, r: u32) {
        self.set_cc(CC_Z, r & 0xff == 0);
    }
    fn set_z2(&mut self, r: u32) {
        self.set_cc(CC_Z, r & 0xffff == 0);
    }
    fn set_n1(&mut self, r: u32) {
        self.set_cc(CC_N, (r >> 7) & 1 != 0);
    }
    fn set_n2(&mut self, r: u32) {
        self.set_cc(CC_N, (r >> 15) & 1 != 0);
    }
    fn set_c1(&mut self, r: u32) {
        self.set_cc(CC_C, (r >> 8) & 1 != 0);
    }
    fn set_v1(&mut self, a: u32, b: u32, r: u32) {
        let t = a ^ b ^ r;
        self.set_cc(CC_V, ((t ^ (t >> 1)) >> 7) & 1 != 0);
    }
    fn set_v2(&mut self, a: u32, b: u32, r: u32) {
        let t = a ^ b ^ r;
        self.set_cc(CC_V, ((t ^ (t >> 1)) >> 15) & 1 != 0);
    }
    fn set_h(&mut self, a: u32, b: u32, r: u32) {
        self.set_cc(CC_H, ((a ^ b ^ r) >> 4) & 1 != 0);
    }
    fn set_nz1(&mut self, r: u32) {
        self.set_n1(r);
        self.set_z1(r);
    }
    fn set_nz2(&mut self, r: u32) {
        self.set_n2(r);
        self.set_z2(r);
    }
    fn set_nzvc1(&mut self, a: u32, b: u32, r: u32) {
        self.set_n1(r);
        self.set_z1(r);
        self.set_v1(a, b, r);
        self.set_c1(r);
    }
    fn set_hnzvc1(&mut self, a: u32, b: u32, r: u32) {
        self.set_nzvc1(a, b, r);
        self.set_h(a, b, r);
    }
    fn set_nzv2(&mut self, a: u32, b: u32, r: u32) {
        self.set_n2(r);
        self.set_z2(r);
        self.set_v2(a, b, r);
    }

    /// V after a shift/rotate is N xor C, computed from the post-operation
    /// flags.
    fn set_v_from_n_xor_c(&mut self) {
        let c = self.cc_set(CC_C);
        let n = self.cc_set(CC_N);
        self.set_cc(CC_V, n ^ c);
    }

    fn test_branch_cond(&self, cond: u8) -> bool {
        let c = self.cc_set(CC_C);
        let n = self.cc_set(CC_N);
        let z = self.cc_set(CC_Z);
        let v = self.cc_set(CC_V);

        match cond {
            COND_HI => !(c || z),
            COND_LS => c || z,
            COND_CC => !c,
            COND_CS => c,
            COND_NE => !z,
            COND_EQ => z,
            COND_VC => !v,
            COND_VS => v,
            COND_PL => !n,
            COND_MI => n,
            COND_GE => !(n ^ v),
            COND_LT => n ^ v,
            COND_GT => !((n ^ v) || z),
            COND_LE => (n ^ v) || z,
            // COND_N is 0x1 and returns false; everything else (COND_A) is
            // unconditional, matching the C++ default: label
            0x1 => false,
            _ => true,
        }
    }

    fn push8(&mut self, bus: &mut dyn Bus, val: u8) {
        let sp = self.sp;
        bus.write8(sp as u32, val);
        self.sp = sp.wrapping_sub(1);
    }

    fn pull8(&mut self, bus: &mut dyn Bus) -> u8 {
        let sp = self.sp.wrapping_add(1);
        self.sp = sp;
        bus.read8(sp as u32)
    }

    fn push16(&mut self, bus: &mut dyn Bus, val: u16) {
        // low byte at the higher address, then high byte -- stack grows down
        let mut sp = self.sp;
        bus.write8(sp as u32, (val & 0xff) as u8);
        sp = sp.wrapping_sub(1);
        bus.write8(sp as u32, (val >> 8) as u8);
        sp = sp.wrapping_sub(1);
        self.sp = sp;
    }

    fn pull16(&mut self, bus: &mut dyn Bus) -> u16 {
        let mut sp = self.sp;
        sp = sp.wrapping_add(1);
        let hi = bus.read8(sp as u32) as u16;
        sp = sp.wrapping_add(1);
        let lo = bus.read8(sp as u32) as u16;
        self.sp = sp;
        (hi << 8) | lo
    }

    /// Read the operand for a read-modify-write op: the register for implied
    /// mode, otherwise the byte at the already-computed address.
    fn rmw_read(&mut self, bus: &mut dyn Bus, op: &OpDecode, arg: i32) -> u8 {
        if op.mode == AddrMode::Implied {
            self.get_reg(op.target) as u8
        } else {
            bus.read8(arg as u32)
        }
    }

    /// Write back a read-modify-write result, mirroring the C++
    /// `shared_memwrite` label.
    fn rmw_write(&mut self, bus: &mut dyn Bus, op: &OpDecode, arg: i32, val: u8) {
        if op.mode == AddrMode::Implied {
            self.put_reg(op.target, val as u16);
        } else {
            debug_assert_eq!(op.width, 1);
            bus.write8(arg as u32, val);
        }
    }
}

impl Cpu for Cpu6800 {
    fn reset(&mut self, bus: &mut dyn Bus) {
        self.a = 0;
        self.b = 0;
        self.ix = 0;
        self.sp = 0;
        self.cc = 0;
        // The C++ defers this to the first Run() iteration via an EXC_RESET
        // bit; doing it here is equivalent and keeps the trace lines aligned,
        // since the C++ emits its first trace line after that fixup.
        self.pc = bus.read16(0xfffe, Endian::Big);
    }

    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        let opcode = bus.read8(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);

        let op = OPS[opcode as usize];

        if op.op == Op::Bad {
            eprintln!("unhandled opcode {:#04x} at {:#06x}", opcode, self.pc.wrapping_sub(1));
            return StepResult::BadOpcode;
        }

        // ---- addressing mode: compute the operand (or its address) ----
        let mut arg: i32 = 0;
        match op.mode {
            AddrMode::Implied => {}
            AddrMode::Immediate => {
                if op.width == 1 {
                    arg = bus.read8(self.pc as u32) as i32;
                    self.pc = self.pc.wrapping_add(1);
                } else {
                    arg = bus.read16(self.pc as u32, Endian::Big) as i32;
                    self.pc = self.pc.wrapping_add(2);
                }
            }
            AddrMode::Direct => {
                let addr = bus.read8(self.pc as u32) as u16;
                self.pc = self.pc.wrapping_add(1);
                arg = if op.calcaddr {
                    addr as i32
                } else if op.width == 1 {
                    bus.read8(addr as u32) as i32
                } else {
                    // XXX doesn't handle wraparound, as in the C++
                    bus.read16(addr as u32, Endian::Big) as i32
                };
            }
            AddrMode::Extended => {
                let addr = bus.read16(self.pc as u32, Endian::Big);
                self.pc = self.pc.wrapping_add(2);
                arg = if op.calcaddr {
                    addr as i32
                } else if op.width == 1 {
                    bus.read8(addr as u32) as i32
                } else {
                    bus.read16(addr as u32, Endian::Big) as i32
                };
            }
            AddrMode::Branch => {
                if op.width == 1 {
                    arg = bus.read8(self.pc as u32) as i8 as i32;
                    self.pc = self.pc.wrapping_add(1);
                } else {
                    arg = bus.read16(self.pc as u32, Endian::Big) as i16 as i32;
                    self.pc = self.pc.wrapping_add(2);
                }
            }
            AddrMode::Indexed => {
                let off = bus.read8(self.pc as u32) as u16;
                self.pc = self.pc.wrapping_add(1);
                let addr = self.ix.wrapping_add(off);
                arg = if op.calcaddr {
                    addr as i32
                } else if op.width == 1 {
                    bus.read8(addr as u32) as i32
                } else {
                    bus.read16(addr as u32, Endian::Big) as i32
                };
            }
        }

        // ---- execute ----
        let mut result = StepResult::Ok;

        match op.op {
            Op::Nop => {}

            Op::Add | Op::Adc => {
                let a = self.get_reg(op.target) as u32;
                let b = arg as u32;
                let mut r = a.wrapping_add(b);
                if op.op == Op::Adc && self.cc_set(CC_C) {
                    r = r.wrapping_add(1);
                }
                self.set_hnzvc1(a, b, r);
                self.put_reg(op.target, r as u16);
            }

            Op::AddAccum => {
                let a = self.get_reg(Reg::A) as u32;
                let b = self.get_reg(Reg::B) as u32;
                let r = a.wrapping_add(b);
                self.set_hnzvc1(a, b, r);
                self.put_reg(op.target, r as u16);
            }

            Op::Sub | Op::Sbc => {
                // The C++ negates the operand and adds, then derives carry
                // from bit 8 of that sum -- not the same as a borrow. Preserved
                // deliberately; the C++ carries an "XXX make sure carry is
                // okay" comment here.
                let a = self.get_reg(op.target) as u32;
                let b = (arg as u32).wrapping_neg();
                let mut r = a.wrapping_add(b);
                if op.op == Op::Sbc && self.cc_set(CC_C) {
                    r = r.wrapping_sub(1);
                }
                self.set_nzvc1(a, b, r);
                self.put_reg(op.target, r as u16);
            }

            Op::SubAccum => {
                let a = self.get_reg(Reg::A) as u32;
                let b = (self.get_reg(Reg::B) as u32).wrapping_neg();
                let r = a.wrapping_add(b);
                self.set_nzvc1(a, b, r);
                self.put_reg(op.target, r as u16);
            }

            Op::Cmp => {
                let a = self.get_reg(op.target) as u32;
                let b = arg as u32;
                let r = a.wrapping_sub(b);
                if op.width == 1 {
                    self.set_nzvc1(a, b, r);
                } else {
                    self.set_nzv2(a, b, r);
                }
            }

            Op::CmpAccum => {
                let a = self.get_reg(Reg::A) as u32;
                let b = self.get_reg(Reg::B) as u32;
                let r = a.wrapping_sub(b);
                self.set_nzvc1(a, b, r);
            }

            Op::And | Op::Bit => {
                let a = self.get_reg(op.target) as u32;
                let r = a & arg as u32;
                self.set_nz1(r);
                self.set_cc(CC_V, false);
                if op.op == Op::And {
                    self.put_reg(op.target, r as u16);
                }
            }

            Op::Or => {
                let a = self.get_reg(op.target) as u32;
                let r = a | arg as u32;
                self.set_nz1(r);
                self.set_cc(CC_V, false);
                self.put_reg(op.target, r as u16);
            }

            Op::Eor => {
                let a = self.get_reg(op.target) as u32;
                let r = a ^ arg as u32;
                self.set_nz1(r);
                self.set_cc(CC_V, false);
                self.put_reg(op.target, r as u16);
            }

            Op::Asl => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_C, (v >> 7) & 1 != 0);
                let v = v << 1;
                self.set_nz1(v as u32);
                self.set_v_from_n_xor_c();
                self.rmw_write(bus, &op, arg, v);
            }

            // NOTE: the C++ `case ASR:` has no break and falls through into
            // `case LSR:`, which re-reads the operand and overwrites both the
            // result and the flags. ASR is therefore identical to LSR, and it
            // performs a *second* operand read -- which is observable on a
            // device register. Reproduced exactly; see the plan doc.
            Op::Asr | Op::Lsr => {
                if op.op == Op::Asr {
                    // the discarded ASR pass, kept for its bus read and for
                    // the flag writes the LSR pass then clobbers
                    let v = self.rmw_read(bus, &op, arg);
                    self.set_cc(CC_C, v & 1 != 0);
                    let shifted = (v & 0x80) | (v >> 1);
                    self.set_nz1(shifted as u32);
                    self.set_v_from_n_xor_c();
                }

                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_C, v & 1 != 0);
                let v = v >> 1;
                self.set_nz1(v as u32);
                self.set_v_from_n_xor_c();
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Rol => {
                let v = self.rmw_read(bus, &op, arg);
                let oldc = self.cc_set(CC_C);
                self.set_cc(CC_C, (v >> 7) & 1 != 0);
                let v = (v << 1) | if oldc { 1 } else { 0 };
                self.set_nz1(v as u32);
                self.set_v_from_n_xor_c();
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Ror => {
                let v = self.rmw_read(bus, &op, arg);
                let oldc = self.cc_set(CC_C);
                self.set_cc(CC_C, v & 1 != 0);
                let v = if oldc { 0x80 } else { 0 } | (v >> 1);
                self.set_nz1(v as u32);
                self.set_v_from_n_xor_c();
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Dec => {
                if op.width == 1 {
                    let v = self.rmw_read(bus, &op, arg).wrapping_sub(1);
                    self.set_cc(CC_V, v == 0x7f);
                    self.set_nz1(v as u32);
                    self.rmw_write(bus, &op, arg, v);
                } else {
                    let v = self.get_reg(op.target).wrapping_sub(1);
                    if op.target == Reg::Ix {
                        // only dex touches Z; des leaves the flags alone
                        self.set_z2(v as u32);
                    }
                    self.put_reg(op.target, v);
                }
            }

            Op::Inc => {
                if op.width == 1 {
                    let v = self.rmw_read(bus, &op, arg).wrapping_add(1);
                    self.set_cc(CC_V, v == 0x80);
                    self.set_nz1(v as u32);
                    self.rmw_write(bus, &op, arg, v);
                } else {
                    let v = self.get_reg(op.target).wrapping_add(1);
                    if op.target == Reg::Ix {
                        // only inx touches Z
                        self.set_z2(v as u32);
                    }
                    self.put_reg(op.target, v);
                }
            }

            Op::Clr => {
                self.set_cc(CC_N, false);
                self.set_cc(CC_V, false);
                self.set_cc(CC_C, false);
                self.set_cc(CC_Z, true);
                self.rmw_write(bus, &op, arg, 0);
            }

            Op::Com => {
                let v = !self.rmw_read(bus, &op, arg);
                self.set_nz1(v as u32);
                self.set_cc(CC_V, false);
                self.set_cc(CC_C, true);
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Neg => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, v == 0x80);
                self.set_cc(CC_C, v != 0x00);
                let v = v.wrapping_neg();
                self.set_nz1(v as u32);
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Tst => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, false);
                self.set_cc(CC_C, false);
                self.set_nz1(v as u32);
            }

            Op::Tfr => {
                if op.width == 1 {
                    // tab, tba
                    let v = if op.target == Reg::A {
                        self.get_reg(Reg::B)
                    } else {
                        self.get_reg(Reg::A)
                    };
                    self.put_reg(op.target, v);
                    self.set_nz1(v as u32);
                    self.set_cc(CC_V, false);
                } else {
                    // tsx loads IX with SP+1; txs loads SP with IX-1
                    let v = if op.target == Reg::Sp {
                        self.get_reg(Reg::Ix).wrapping_sub(1)
                    } else {
                        self.get_reg(Reg::Sp).wrapping_add(1)
                    };
                    self.put_reg(op.target, v);
                }
            }

            Op::TfrCc => {
                let v = if op.target == Reg::A {
                    // tpa: the top two condition-code bits read back as 1
                    self.get_reg(Reg::Cc) as u8 | 0b1100_0000
                } else {
                    // tap: only the low six bits are writable
                    self.get_reg(Reg::A) as u8 & 0b0011_1111
                };
                self.put_reg(op.target, v as u16);
            }

            Op::Push => {
                let v = self.get_reg(op.target) as u8;
                self.push8(bus, v);
            }

            Op::Pull => {
                let v = self.pull8(bus);
                self.put_reg(op.target, v as u16);
            }

            Op::Ld => {
                if op.width == 1 {
                    self.set_nz1(arg as u32);
                } else {
                    self.set_nz2(arg as u32);
                }
                self.set_cc(CC_V, false);
                self.put_reg(op.target, arg as u16);
            }

            Op::St => {
                if op.width == 1 {
                    let v = self.get_reg(op.target) as u8;
                    bus.write8(arg as u32, v);
                    self.set_nz1(v as u32);
                } else {
                    let v = self.get_reg(op.target);
                    bus.write16(arg as u32, v, Endian::Big);
                    self.set_nz2(v as u32);
                }
                self.set_cc(CC_V, false);
            }

            Op::Bra => {
                if self.test_branch_cond(op.cond) {
                    if arg == -2 {
                        eprintln!("infinite loop detected, aborting cpu");
                        result = StepResult::InfiniteLoop;
                    }
                    self.pc = (self.pc as i32).wrapping_add(arg) as u16;
                }
            }

            Op::Jmp => {
                // NOTE: PC here already points past the operand, so this only
                // fires for a jump to the *next* instruction, not a jump to
                // self. Preserved as-is from the C++.
                if arg == self.pc as i32 {
                    eprintln!("infinite loop detected, aborting cpu");
                    result = StepResult::InfiniteLoop;
                }
                self.pc = arg as u16;
            }

            Op::Jsr => {
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = arg as u16;
            }

            Op::Bsr => {
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = (self.pc as i32).wrapping_add(arg) as u16;
            }

            Op::Rts => {
                self.pc = self.pull16(bus);
            }

            Op::SetCc => self.cc |= op.cc_flag,
            Op::ClrCc => self.cc &= !op.cc_flag,

            Op::Bad => unreachable!("handled before operand fetch"),
        }

        result
    }

    fn dump(&self) {
        println!(
            "A 0x{:02x} B 0x{:02x} X 0x{:04x} S 0x{:04x} CC 0x{:02x} ({}{}{}{}{}) PC 0x{:04x}",
            self.a,
            self.b,
            self.ix,
            self.sp,
            self.cc,
            if self.cc_set(CC_H) { 'h' } else { ' ' },
            if self.cc_set(CC_N) { 'n' } else { ' ' },
            if self.cc_set(CC_Z) { 'z' } else { ' ' },
            if self.cc_set(CC_V) { 'v' } else { ' ' },
            if self.cc_set(CC_C) { 'c' } else { ' ' },
            self.pc
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        // must match Cpu6800::TraceInstruction() in cpu/cpu6800.cpp exactly
        writeln!(
            out,
            "PC={:04x} A={:02x} B={:02x} X={:04x} S={:04x} CC={:02x}",
            self.pc, self.a, self.b, self.ix, self.sp, self.cc
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{run_steps, TestBus};

    /// Load a hand-assembled program at 0xe000 and reset into it.
    fn boot(prog: &[u8]) -> (Cpu6800, TestBus) {
        let mut bus = TestBus::new();
        bus.load(0xe000, prog);
        bus.set_reset_vector(0xe000);
        let mut cpu = Cpu6800::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn reset_loads_pc_from_the_vector() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x1234);
        let mut cpu = Cpu6800::new();
        cpu.a = 0xff;
        cpu.ix = 0xffff;
        cpu.reset(&mut bus);
        assert_eq!(cpu.pc, 0x1234);
        assert_eq!((cpu.a, cpu.b, cpu.ix, cpu.sp, cpu.cc), (0, 0, 0, 0, 0));
    }

    /// Smoke test: sum four bytes through an indexed loop and store the result.
    /// Covers immediate/indexed/extended addressing, inx, cpx and a taken
    /// conditional branch.
    #[test]
    fn sums_a_table_with_an_indexed_loop() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x8e, 0x00, 0xff,       // e000  lds  #0x00ff
            0xce, 0x01, 0x00,       // e003  ldx  #0x0100
            0x4f,                   // e006  clra
            0xab, 0x00,             // e007  adda ,x          <- loop
            0x08,                   // e009  inx
            0x8c, 0x01, 0x04,       // e00a  cpx  #0x0104
            0x26, 0xf8,             // e00d  bne  loop
            0xb7, 0x02, 0x00,       // e00f  sta  0x0200
        ]);
        bus.load(0x0100, &[0x01, 0x02, 0x03, 0x04]);

        // 3 setup + 4 iterations of 4 + the store
        run_steps(&mut cpu, &mut bus, 20);

        assert_eq!(cpu.a, 0x0a);
        assert_eq!(cpu.ix, 0x0104);
        assert_eq!(cpu.sp, 0x00ff);
        assert_eq!(cpu.pc, 0xe012);
        assert_eq!(bus.mem[0x0200], 0x0a);
    }

    /// The 6800 stack post-decrements on push, so the pushed low byte lands at
    /// the higher address and the pointer ends one below the last byte written.
    #[test]
    fn jsr_and_rts_round_trip_through_the_stack() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x8e, 0x00, 0xff,       // e000  lds #0x00ff
            0xbd, 0xe0, 0x07,       // e003  jsr 0xe007
            0x01,                   // e006  nop      <- return lands here
            0x86, 0x42,             // e007  lda #0x42
            0x39,                   // e009  rts
        ]);

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.pc, 0xe007);
        assert_eq!(cpu.sp, 0x00fd);
        assert_eq!((bus.mem[0x00fe], bus.mem[0x00ff]), (0xe0, 0x06));

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.pc, 0xe006);
        assert_eq!(cpu.sp, 0x00ff);
    }

    /// `asr` is identical to `lsr` here: the C++ `case ASR:` falls through into
    /// `case LSR:`, which overwrites the result. A real 6800 would leave 0xc0.
    /// Do not "fix" this without re-baselining the oracle.
    #[test]
    fn asr_falls_through_into_lsr() {
        let (mut cpu, mut bus) = boot(&[0x86, 0x80, 0x47]); // lda #0x80 ; asra
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0x40);
        assert!(!cpu.cc_set(CC_C));
        assert!(!cpu.cc_set(CC_N));
    }

    /// The other half of the fallthrough: the discarded ASR pass reads the
    /// operand before the LSR pass reads it again, so a memory-form `asr` hits
    /// its address twice. That is invisible in RAM but not on a device
    /// register, and it is trace-visible.
    #[test]
    fn memory_asr_reads_its_operand_twice() {
        let (mut cpu, mut bus) = boot(&[0x77, 0x01, 0x00]); // asr 0x0100
        bus.mem[0x0100] = 0x81;
        bus.watch = Some(0x0100);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.watch_reads, 2);
        assert_eq!(bus.mem[0x0100], 0x40);
        // flags come from the LSR pass, which clobbers the ASR pass's
        assert!(cpu.cc_set(CC_C));
        assert!(!cpu.cc_set(CC_N));
        assert!(cpu.cc_set(CC_V)); // V is N xor C

        // lsr, the same operation without the extra read
        let (mut cpu, mut bus) = boot(&[0x74, 0x01, 0x00]); // lsr 0x0100
        bus.mem[0x0100] = 0x81;
        bus.watch = Some(0x0100);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.watch_reads, 1);
        assert_eq!(bus.mem[0x0100], 0x40);
    }

    #[test]
    fn cmpa_sets_z_n_and_c_without_touching_the_accumulator() {
        let cmp = |a: u8, m: u8| {
            let (mut cpu, mut bus) = boot(&[0x86, a, 0x81, m]); // lda #a ; cmpa #m
            run_steps(&mut cpu, &mut bus, 2);
            assert_eq!(cpu.a, a, "cmp must not write the accumulator");
            (cpu.cc_set(CC_Z), cpu.cc_set(CC_N), cpu.cc_set(CC_C))
        };

        assert_eq!(cmp(0x50, 0x50), (true, false, false));
        assert_eq!(cmp(0x50, 0x30), (false, false, false));
        assert_eq!(cmp(0x30, 0x50), (false, true, true)); // borrow
    }

    /// `tpa` reads the two unimplemented condition-code bits back as ones;
    /// `tap` can only write the low six.
    #[test]
    fn tpa_and_tap_mask_the_top_two_condition_code_bits() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x0d,                   // sec
            0x07,                   // tpa
            0x86, 0xff,             // lda #0xff
            0x06,                   // tap
        ]);

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0xc1, "tpa reads back cc | 0xc0");

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.cc, 0x3f, "tap writes only the low six bits");
    }

    #[test]
    fn an_unimplemented_opcode_stops_the_run() {
        // 0x00 is not a 6800 instruction and has no table entry
        let (mut cpu, mut bus) = boot(&[0x00]);
        assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode);
    }
}
