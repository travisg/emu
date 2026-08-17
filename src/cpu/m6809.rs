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
//! Motorola 6809 interpreter core.
//!
//! Port of `cpu/cpu6809.cpp`. Same table-driven shape as the 6800, extended to
//! three 256-entry pages: the base page, the `0x10` prefix page at +0x100 and
//! the `0x11` prefix page at +0x200.
//!
//! Several details differ from the 6800 core in ways that are easy to get
//! wrong when porting both; each is called out at its use site:
//!   - `SET_V1` uses `result >> 1`, not `(a^b^result) >> 1`
//!   - stack pushes pre-decrement (the 6800 post-decrements)
//!   - `shared_memwrite` sets N/Z *after* the write, from the written value
//!   - `cmp` on a byte sets H as well
//!   - `asr` has no fallthrough bug here
//!
//! Preserved bug-for-bug against the oracle; see the 6800 core's note.

use super::{Cpu, StepResult};
use crate::bus::{Bus, Endian};
use std::io::Write;

const CC_C: u8 = 0x01;
const CC_V: u8 = 0x02;
const CC_Z: u8 = 0x04;
const CC_N: u8 = 0x08;
const CC_H: u8 = 0x20;

const COND_N: u8 = 0x1;
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
    Adc,
    Sub,
    Sbc,
    Cmp,
    And,
    Bit,
    Eor,
    Or,
    Nop,
    Exg,
    Abx,
    Clr,
    Com,
    Neg,
    Dec,
    Inc,
    Tst,
    Lea,
    Asl,
    Asr,
    Lsr,
    Rol,
    Ror,
    Tfr,
    Sex,
    Push,
    Pull,
    Bra,
    Bsr,
    Jmp,
    Jsr,
    Rts,
    Ld,
    St,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Reg {
    X,
    Y,
    U,
    S,
    A,
    B,
    D,
    Pc,
    Dp,
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
    cond: u8,
    calcaddr: bool,
}

const BAD: OpDecode = OpDecode {
    name: "???",
    mode: AddrMode::Implied,
    width: 1,
    op: Op::Bad,
    target: Reg::A,
    cond: 0,
    calcaddr: false,
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
        }
    };
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident, addr) => {
        OpDecode { calcaddr: true, ..op!($name, $mode, $w, $op, $t) }
    };
    ($name:expr, $mode:ident, $w:expr, $op:ident, $t:ident, cond = $c:expr) => {
        OpDecode { cond: $c, ..op!($name, $mode, $w, $op, $t) }
    };
}

#[rustfmt::skip]
const fn build_ops() -> [OpDecode; 256 * 3] {
    let mut t = [BAD; 256 * 3];

    // alu
    t[0x8b] = op!("adda", Immediate, 1, Add, A);
    t[0xcb] = op!("addb", Immediate, 1, Add, B);
    t[0xc3] = op!("addd", Immediate, 2, Add, D);
    t[0x9b] = op!("adda", Direct,    1, Add, A);
    t[0xdb] = op!("addb", Direct,    1, Add, B);
    t[0xd3] = op!("addd", Direct,    2, Add, D);
    t[0xab] = op!("adda", Indexed,   1, Add, A);
    t[0xeb] = op!("addb", Indexed,   1, Add, B);
    t[0xe3] = op!("addd", Indexed,   2, Add, D);
    t[0xbb] = op!("adda", Extended,  1, Add, A);
    t[0xfb] = op!("addb", Extended,  1, Add, B);
    t[0xf3] = op!("addd", Extended,  2, Add, D);

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
    t[0x83] = op!("subd", Immediate, 2, Sub, D);
    t[0x90] = op!("suba", Direct,    1, Sub, A);
    t[0xd0] = op!("subb", Direct,    1, Sub, B);
    t[0x93] = op!("subd", Direct,    2, Sub, D);
    t[0xa0] = op!("suba", Indexed,   1, Sub, A);
    t[0xe0] = op!("subb", Indexed,   1, Sub, B);
    t[0xa3] = op!("subd", Indexed,   2, Sub, D);
    t[0xb0] = op!("suba", Extended,  1, Sub, A);
    t[0xf0] = op!("subb", Extended,  1, Sub, B);
    t[0xb3] = op!("subd", Extended,  2, Sub, D);

    t[0x82] = op!("sbca", Immediate, 1, Sbc, A);
    t[0xc2] = op!("sbcb", Immediate, 1, Sbc, B);
    t[0x92] = op!("sbca", Direct,    1, Sbc, A);
    t[0xd2] = op!("sbcb", Direct,    1, Sbc, B);
    t[0xa2] = op!("sbca", Indexed,   1, Sbc, A);
    t[0xe2] = op!("sbcb", Indexed,   1, Sbc, B);
    t[0xb2] = op!("sbca", Extended,  1, Sbc, A);
    t[0xf2] = op!("sbcb", Extended,  1, Sbc, B);

    t[0x81]  = op!("cmpa", Immediate, 1, Cmp, A);
    t[0xc1]  = op!("cmpb", Immediate, 1, Cmp, B);
    t[0x183] = op!("cmpd", Immediate, 2, Cmp, D);
    t[0x28c] = op!("cmps", Immediate, 2, Cmp, S);
    t[0x283] = op!("cmpu", Immediate, 2, Cmp, U);
    t[0x8c]  = op!("cmpx", Immediate, 2, Cmp, X);
    t[0x18c] = op!("cmpy", Immediate, 2, Cmp, Y);

    t[0x91]  = op!("cmpa", Direct, 1, Cmp, A);
    t[0xd1]  = op!("cmpb", Direct, 1, Cmp, B);
    t[0x193] = op!("cmpd", Direct, 2, Cmp, D);
    t[0x29c] = op!("cmps", Direct, 2, Cmp, S);
    t[0x293] = op!("cmpu", Direct, 2, Cmp, U);
    t[0x9c]  = op!("cmpx", Direct, 2, Cmp, X);
    t[0x19c] = op!("cmpy", Direct, 2, Cmp, Y);

    t[0xa1]  = op!("cmpa", Indexed, 1, Cmp, A);
    t[0xe1]  = op!("cmpb", Indexed, 1, Cmp, B);
    t[0x1a3] = op!("cmpd", Indexed, 2, Cmp, D);
    t[0x2ac] = op!("cmps", Indexed, 2, Cmp, S);
    t[0x2a3] = op!("cmpu", Indexed, 2, Cmp, U);
    t[0xac]  = op!("cmpx", Indexed, 2, Cmp, X);
    t[0x1ac] = op!("cmpy", Indexed, 2, Cmp, Y);

    t[0xb1]  = op!("cmpa", Extended, 1, Cmp, A);
    t[0xf1]  = op!("cmpb", Extended, 1, Cmp, B);
    t[0x1b3] = op!("cmpd", Extended, 2, Cmp, D);
    t[0x2bc] = op!("cmps", Extended, 2, Cmp, S);
    t[0x2b3] = op!("cmpu", Extended, 2, Cmp, U);
    t[0xbc]  = op!("cmpx", Extended, 2, Cmp, X);
    t[0x1bc] = op!("cmpy", Extended, 2, Cmp, Y);

    t[0x84] = op!("anda",  Immediate, 1, And, A);
    t[0xc4] = op!("andb",  Immediate, 1, And, B);
    t[0x1c] = op!("andcc", Immediate, 1, And, Cc);
    t[0x94] = op!("anda",  Direct,    1, And, A);
    t[0xd4] = op!("andb",  Direct,    1, And, B);
    t[0xa4] = op!("anda",  Indexed,   1, And, A);
    t[0xe4] = op!("andb",  Indexed,   1, And, B);
    t[0xb4] = op!("anda",  Extended,  1, And, A);
    t[0xf4] = op!("andb",  Extended,  1, And, B);

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

    t[0x8a] = op!("ora",  Immediate, 1, Or, A);
    t[0xca] = op!("orb",  Immediate, 1, Or, B);
    t[0x1a] = op!("orcc", Immediate, 1, Or, Cc);
    t[0x9a] = op!("ora",  Direct,    1, Or, A);
    t[0xda] = op!("orb",  Direct,    1, Or, B);
    t[0xaa] = op!("ora",  Indexed,   1, Or, A);
    t[0xea] = op!("orb",  Indexed,   1, Or, B);
    t[0xba] = op!("ora",  Extended,  1, Or, A);
    t[0xfa] = op!("orb",  Extended,  1, Or, B);

    // misc
    t[0x12] = op!("nop", Implied, 1, Nop, X);
    t[0x1e] = op!("exg", Implied, 1, Exg, X);
    t[0x3a] = op!("abx", Implied, 2, Abx, X);
    t[0x1f] = op!("tfr", Implied, 1, Tfr, A);
    t[0x1d] = op!("sex", Implied, 1, Sex, A);

    t[0x4f] = op!("clra", Implied,  1, Clr, A);
    t[0x5f] = op!("clrb", Implied,  1, Clr, B);
    t[0x0f] = op!("clr",  Direct,   1, Clr, A, addr);
    t[0x6f] = op!("clr",  Indexed,  1, Clr, A, addr);
    t[0x7f] = op!("clr",  Extended, 1, Clr, A, addr);

    t[0x43] = op!("coma", Implied,  1, Com, A);
    t[0x53] = op!("comb", Implied,  1, Com, B);
    t[0x03] = op!("com",  Direct,   1, Com, A, addr);
    t[0x63] = op!("com",  Indexed,  1, Com, A, addr);
    t[0x73] = op!("com",  Extended, 1, Com, A, addr);

    t[0x40] = op!("nega", Implied,  1, Neg, A);
    t[0x50] = op!("negb", Implied,  1, Neg, B);
    t[0x00] = op!("neg",  Direct,   1, Neg, A, addr);
    t[0x60] = op!("neg",  Indexed,  1, Neg, A, addr);
    t[0x70] = op!("neg",  Extended, 1, Neg, A, addr);

    t[0x4a] = op!("deca", Implied,  1, Dec, A);
    t[0x5a] = op!("decb", Implied,  1, Dec, B);
    t[0x0a] = op!("dec",  Direct,   1, Dec, A, addr);
    t[0x6a] = op!("dec",  Indexed,  1, Dec, A, addr);
    t[0x7a] = op!("dec",  Extended, 1, Dec, A, addr);

    t[0x4c] = op!("inca", Implied,  1, Inc, A);
    t[0x5c] = op!("incb", Implied,  1, Inc, B);
    t[0x0c] = op!("inc",  Direct,   1, Inc, A, addr);
    t[0x6c] = op!("inc",  Indexed,  1, Inc, A, addr);
    t[0x7c] = op!("inc",  Extended, 1, Inc, A, addr);

    t[0x48] = op!("asla", Implied,  1, Asl, A);
    t[0x58] = op!("aslb", Implied,  1, Asl, B);
    t[0x08] = op!("asl",  Direct,   1, Asl, A, addr);
    t[0x68] = op!("asl",  Indexed,  1, Asl, A, addr);
    t[0x78] = op!("asl",  Extended, 1, Asl, A, addr);

    t[0x47] = op!("asra", Implied,  1, Asr, A);
    t[0x57] = op!("asrb", Implied,  1, Asr, B);
    t[0x07] = op!("asr",  Direct,   1, Asr, A, addr);
    t[0x67] = op!("asr",  Indexed,  1, Asr, A, addr);
    t[0x77] = op!("asr",  Extended, 1, Asr, A, addr);

    t[0x44] = op!("lsra", Implied,  1, Lsr, A);
    t[0x54] = op!("lsrb", Implied,  1, Lsr, B);
    t[0x04] = op!("lsr",  Direct,   1, Lsr, A, addr);
    t[0x64] = op!("lsr",  Indexed,  1, Lsr, A, addr);
    t[0x74] = op!("lsr",  Extended, 1, Lsr, A, addr);

    t[0x49] = op!("rola", Implied,  1, Rol, A);
    t[0x59] = op!("rolb", Implied,  1, Rol, B);
    t[0x09] = op!("rol",  Direct,   1, Rol, A, addr);
    t[0x69] = op!("rol",  Indexed,  1, Rol, A, addr);
    t[0x79] = op!("rol",  Extended, 1, Rol, A, addr);

    t[0x46] = op!("rora", Implied,  1, Ror, A);
    t[0x56] = op!("rorb", Implied,  1, Ror, B);
    t[0x06] = op!("ror",  Direct,   1, Ror, A, addr);
    t[0x66] = op!("ror",  Indexed,  1, Ror, A, addr);
    t[0x76] = op!("ror",  Extended, 1, Ror, A, addr);

    t[0x4d] = op!("tsta", Implied,  1, Tst, A);
    t[0x5d] = op!("tstb", Implied,  1, Tst, B);
    t[0x0d] = op!("tst",  Direct,   1, Tst, A, addr);
    t[0x6d] = op!("tst",  Indexed,  1, Tst, A, addr);
    t[0x7d] = op!("tst",  Extended, 1, Tst, A, addr);

    t[0x32] = op!("leas", Indexed, 2, Lea, S, addr);
    t[0x33] = op!("leau", Indexed, 2, Lea, U, addr);
    t[0x30] = op!("leax", Indexed, 2, Lea, X, addr);
    t[0x31] = op!("leay", Indexed, 2, Lea, Y, addr);

    // push/pull
    t[0x34] = op!("pshs", Immediate, 1, Push, S);
    t[0x36] = op!("pshu", Immediate, 1, Push, U);
    t[0x35] = op!("puls", Immediate, 1, Pull, S);
    t[0x37] = op!("pulu", Immediate, 1, Pull, U);

    // loads
    t[0x86]  = op!("lda", Immediate, 1, Ld, A);
    t[0xc6]  = op!("ldb", Immediate, 1, Ld, B);
    t[0xcc]  = op!("ldd", Immediate, 2, Ld, D);
    t[0x1ce] = op!("lds", Immediate, 2, Ld, S);
    t[0xce]  = op!("ldu", Immediate, 2, Ld, U);
    t[0x8e]  = op!("ldx", Immediate, 2, Ld, X);
    t[0x18e] = op!("ldy", Immediate, 2, Ld, Y);

    t[0x96]  = op!("lda", Direct, 1, Ld, A);
    t[0xd6]  = op!("ldb", Direct, 1, Ld, B);
    t[0xdc]  = op!("ldd", Direct, 2, Ld, D);
    t[0x1de] = op!("lds", Direct, 2, Ld, S);
    t[0xde]  = op!("ldu", Direct, 2, Ld, U);
    t[0x9e]  = op!("ldx", Direct, 2, Ld, X);
    t[0x19e] = op!("ldy", Direct, 2, Ld, Y);

    t[0xa6]  = op!("lda", Indexed, 1, Ld, A);
    t[0xe6]  = op!("ldb", Indexed, 1, Ld, B);
    t[0xec]  = op!("ldd", Indexed, 2, Ld, D);
    t[0x1ee] = op!("lds", Indexed, 2, Ld, S);
    t[0xee]  = op!("ldu", Indexed, 2, Ld, U);
    t[0xae]  = op!("ldx", Indexed, 2, Ld, X);
    t[0x1ae] = op!("ldy", Indexed, 2, Ld, Y);

    t[0xb6]  = op!("lda", Extended, 1, Ld, A);
    t[0xf6]  = op!("ldb", Extended, 1, Ld, B);
    t[0xfc]  = op!("ldd", Extended, 2, Ld, D);
    t[0x1fe] = op!("lds", Extended, 2, Ld, S);
    t[0xfe]  = op!("ldu", Extended, 2, Ld, U);
    t[0xbe]  = op!("ldx", Extended, 2, Ld, X);
    t[0x1be] = op!("ldy", Extended, 2, Ld, Y);

    // stores
    t[0x97]  = op!("sta", Direct, 1, St, A, addr);
    t[0xd7]  = op!("stb", Direct, 1, St, B, addr);
    t[0xdd]  = op!("std", Direct, 2, St, D, addr);
    t[0x1df] = op!("sts", Direct, 2, St, S, addr);
    t[0xdf]  = op!("stu", Direct, 2, St, U, addr);
    t[0x9f]  = op!("stx", Direct, 2, St, X, addr);
    t[0x19f] = op!("sty", Direct, 2, St, Y, addr);

    t[0xb7]  = op!("sta", Extended, 1, St, A, addr);
    t[0xf7]  = op!("stb", Extended, 1, St, B, addr);
    t[0xfd]  = op!("std", Extended, 2, St, D, addr);
    t[0x1ff] = op!("sts", Extended, 2, St, S, addr);
    t[0xff]  = op!("stu", Extended, 2, St, U, addr);
    t[0xbf]  = op!("stx", Extended, 2, St, X, addr);
    t[0x1bf] = op!("sty", Extended, 2, St, Y, addr);

    t[0xa7]  = op!("sta", Indexed, 1, St, A, addr);
    t[0xe7]  = op!("stb", Indexed, 1, St, B, addr);
    t[0xed]  = op!("std", Indexed, 2, St, D, addr);
    t[0x1ef] = op!("sts", Indexed, 2, St, S, addr);
    t[0xef]  = op!("stu", Indexed, 2, St, U, addr);
    t[0xaf]  = op!("stx", Indexed, 2, St, X, addr);
    t[0x1af] = op!("sty", Indexed, 2, St, Y, addr);

    // branches
    t[0x20] = op!("bra", Branch, 1, Bra, A, cond = 0x0);
    t[0x21] = op!("brn", Branch, 1, Bra, A, cond = COND_N);
    t[0x22] = op!("bhi", Branch, 1, Bra, A, cond = COND_HI);
    t[0x23] = op!("bls", Branch, 1, Bra, A, cond = COND_LS);
    t[0x24] = op!("bcc", Branch, 1, Bra, A, cond = COND_CC);
    t[0x25] = op!("bcs", Branch, 1, Bra, A, cond = COND_CS);
    t[0x26] = op!("bne", Branch, 1, Bra, A, cond = COND_NE);
    t[0x27] = op!("beq", Branch, 1, Bra, A, cond = COND_EQ);
    t[0x28] = op!("bvc", Branch, 1, Bra, A, cond = COND_VC);
    t[0x29] = op!("bvs", Branch, 1, Bra, A, cond = COND_VS);
    t[0x2a] = op!("bpl", Branch, 1, Bra, A, cond = COND_PL);
    t[0x2b] = op!("bmi", Branch, 1, Bra, A, cond = COND_MI);
    t[0x2c] = op!("bge", Branch, 1, Bra, A, cond = COND_GE);
    t[0x2d] = op!("blt", Branch, 1, Bra, A, cond = COND_LT);
    t[0x2e] = op!("bgt", Branch, 1, Bra, A, cond = COND_GT);
    t[0x2f] = op!("ble", Branch, 1, Bra, A, cond = COND_LE);
    t[0x8d] = op!("bsr", Branch, 1, Bsr, A, cond = 0x0);

    t[0x16]  = op!("lbra", Branch, 2, Bra, A, cond = 0x0);
    t[0x121] = op!("lbrn", Branch, 2, Bra, A, cond = COND_N);
    t[0x122] = op!("lbhi", Branch, 2, Bra, A, cond = COND_HI);
    t[0x123] = op!("lbls", Branch, 2, Bra, A, cond = COND_LS);
    t[0x124] = op!("lbcc", Branch, 2, Bra, A, cond = COND_CC);
    t[0x125] = op!("lbcs", Branch, 2, Bra, A, cond = COND_CS);
    t[0x126] = op!("lbne", Branch, 2, Bra, A, cond = COND_NE);
    t[0x127] = op!("lbeq", Branch, 2, Bra, A, cond = COND_EQ);
    t[0x128] = op!("lbvc", Branch, 2, Bra, A, cond = COND_VC);
    t[0x129] = op!("lbvs", Branch, 2, Bra, A, cond = COND_VS);
    t[0x12a] = op!("lbpl", Branch, 2, Bra, A, cond = COND_PL);
    t[0x12b] = op!("lbmi", Branch, 2, Bra, A, cond = COND_MI);
    t[0x12c] = op!("lbge", Branch, 2, Bra, A, cond = COND_GE);
    t[0x12d] = op!("lblt", Branch, 2, Bra, A, cond = COND_LT);
    t[0x12e] = op!("lbgt", Branch, 2, Bra, A, cond = COND_GT);
    t[0x12f] = op!("lble", Branch, 2, Bra, A, cond = COND_LE);
    t[0x17]  = op!("lbsr", Branch, 2, Bsr, A, cond = 0x0);

    t[0x0e] = op!("jmp", Direct,   1, Jmp, A, addr);
    t[0x6e] = op!("jmp", Indexed,  1, Jmp, A, addr);
    t[0x7e] = op!("jmp", Extended, 1, Jmp, A, addr);

    t[0x9d] = op!("jsr", Direct,   1, Jsr, A, addr);
    t[0xad] = op!("jsr", Indexed,  1, Jsr, A, addr);
    t[0xbd] = op!("jsr", Extended, 1, Jsr, A, addr);

    t[0x39] = op!("rts", Implied, 1, Rts, A);

    t
}

static OPS: [OpDecode; 256 * 3] = build_ops();

#[derive(Default)]
pub struct Cpu6809 {
    // A and B are the halves of D; the C++ overlays them in a union
    a: u8,
    b: u8,
    x: u16,
    y: u16,
    u: u16,
    s: u16,
    pc: u16,
    dp: u8,
    cc: u8,
}

impl Cpu6809 {
    pub fn new() -> Self {
        Self::default()
    }

    fn d(&self) -> u16 {
        ((self.a as u16) << 8) | self.b as u16
    }

    fn set_d(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.b = v as u8;
    }

    fn get_reg(&self, r: Reg) -> u16 {
        match r {
            Reg::X => self.x,
            Reg::Y => self.y,
            Reg::U => self.u,
            Reg::S => self.s,
            Reg::A => self.a as u16,
            Reg::B => self.b as u16,
            Reg::D => self.d(),
            Reg::Pc => self.pc,
            Reg::Dp => self.dp as u16,
            Reg::Cc => self.cc as u16,
        }
    }

    /// Returns the previous value, as the C++ `PutReg` does -- `exg` needs it.
    fn put_reg(&mut self, r: Reg, val: u16) -> u16 {
        let old = self.get_reg(r);
        match r {
            Reg::X => self.x = val,
            Reg::Y => self.y = val,
            Reg::U => self.u = val,
            Reg::S => self.s = val,
            Reg::A => self.a = val as u8,
            Reg::B => self.b = val as u8,
            Reg::D => self.set_d(val),
            Reg::Pc => self.pc = val,
            Reg::Dp => self.dp = val as u8,
            Reg::Cc => self.cc = val as u8,
        }
        old
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
    fn set_c2(&mut self, r: u32) {
        self.set_cc(CC_C, (r >> 16) & 1 != 0);
    }
    /// NOTE: `result >> 1`, *not* `(a^b^result) >> 1` as the 6800 core uses.
    fn set_v1(&mut self, a: u32, b: u32, r: u32) {
        self.set_cc(CC_V, ((a ^ b ^ r ^ (r >> 1)) >> 7) & 1 != 0);
    }
    fn set_v2(&mut self, a: u32, b: u32, r: u32) {
        self.set_cc(CC_V, ((a ^ b ^ r ^ (r >> 1)) >> 15) & 1 != 0);
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
    fn set_hnzvc1(&mut self, a: u32, b: u32, r: u32) {
        self.set_h(a, b, r);
        self.set_n1(r);
        self.set_z1(r);
        self.set_v1(a, b, r);
        self.set_c1(r);
    }
    fn set_nzvc2(&mut self, a: u32, b: u32, r: u32) {
        self.set_n2(r);
        self.set_z2(r);
        self.set_v2(a, b, r);
        self.set_c2(r);
    }

    fn test_branch_cond(&self, cond: u8) -> bool {
        let c = self.cc_set(CC_C);
        let n = self.cc_set(CC_N);
        let z = self.cc_set(CC_Z);
        let v = self.cc_set(CC_V);
        match cond {
            COND_N => false,
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
            _ => true,
        }
    }

    // Stack access. Unlike the 6800, pushes *pre*-decrement and pulls start at
    // the current pointer.
    fn push16(&mut self, bus: &mut dyn Bus, stack: Reg, val: u16) {
        let mut sp = self.get_reg(stack);
        sp = sp.wrapping_sub(1);
        bus.write8(sp as u32, (val & 0xff) as u8);
        sp = sp.wrapping_sub(1);
        bus.write8(sp as u32, (val >> 8) as u8);
        self.put_reg(stack, sp);
    }

    fn push8(&mut self, bus: &mut dyn Bus, stack: Reg, val: u8) {
        let sp = self.get_reg(stack).wrapping_sub(1);
        bus.write8(sp as u32, val);
        self.put_reg(stack, sp);
    }

    fn pull16(&mut self, bus: &mut dyn Bus, stack: Reg) -> u16 {
        let mut sp = self.get_reg(stack);
        let hi = bus.read8(sp as u32) as u16;
        sp = sp.wrapping_add(1);
        let lo = bus.read8(sp as u32) as u16;
        sp = sp.wrapping_add(1);
        self.put_reg(stack, sp);
        (hi << 8) | lo
    }

    fn pull8(&mut self, bus: &mut dyn Bus, stack: Reg) -> u8 {
        let sp = self.get_reg(stack);
        let v = bus.read8(sp as u32);
        self.put_reg(stack, sp.wrapping_add(1));
        v
    }

    fn rmw_read(&mut self, bus: &mut dyn Bus, op: &OpDecode, arg: i32) -> u8 {
        if op.mode == AddrMode::Implied {
            self.get_reg(op.target) as u8
        } else {
            bus.read8(arg as u32)
        }
    }

    /// The C++ `shared_memwrite` label: write back, *then* set N/Z from the
    /// written value.
    fn rmw_write(&mut self, bus: &mut dyn Bus, op: &OpDecode, arg: i32, val: u8) {
        if op.mode == AddrMode::Implied {
            self.put_reg(op.target, val as u16);
        } else if op.width == 1 {
            bus.write8(arg as u32, val);
        } else {
            bus.write16(arg as u32, val as u16, Endian::Big);
        }
        self.set_nz1(val as u32);
    }

    /// Decode an indexed postbyte, returning the effective address. Mutates
    /// the base register for the auto-increment/decrement modes.
    fn indexed_addr(&mut self, bus: &mut dyn Bus, postbyte: u8) -> Option<u16> {
        let mut off: i32 = 0;
        let mut prepostinc: i32 = 0;
        let mut indirect = postbyte & (1 << 4) != 0;

        #[derive(Copy, Clone, PartialEq)]
        enum Base {
            Reg(Reg),
            Pc,
            Zero,
        }
        let mut base = Base::Reg(match (postbyte >> 5) & 0b11 {
            0 => Reg::X,
            1 => Reg::Y,
            2 => Reg::U,
            _ => Reg::S,
        });

        if postbyte & 0x80 == 0 {
            // 5-bit signed offset; this mode has no indirect form
            off = ((postbyte & 0x1f) as i32) << 27 >> 27;
            indirect = false;
        } else {
            match postbyte & 0xf {
                0x0 => {
                    prepostinc = 1;
                    indirect = false;
                }
                0x1 => prepostinc = 2,
                0x2 => {
                    prepostinc = -1;
                    indirect = false;
                }
                0x3 => prepostinc = -2,
                0x4 => {}
                0x5 => off = self.b as i8 as i32,
                0x6 => off = self.a as i8 as i32,
                0x8 => {
                    off = bus.read8(self.pc as u32) as i8 as i32;
                    self.pc = self.pc.wrapping_add(1);
                }
                0x9 => {
                    off = bus.read16(self.pc as u32, Endian::Big) as i16 as i32;
                    self.pc = self.pc.wrapping_add(2);
                }
                0xb => off = self.d() as i16 as i32,
                0xc => {
                    off = bus.read8(self.pc as u32) as i8 as i32;
                    self.pc = self.pc.wrapping_add(1);
                    base = Base::Pc;
                }
                0xd => {
                    off = bus.read16(self.pc as u32, Endian::Big) as i16 as i32;
                    self.pc = self.pc.wrapping_add(2);
                    base = Base::Pc;
                }
                0xf => {
                    off = bus.read16(self.pc as u32, Endian::Big) as i16 as i32;
                    self.pc = self.pc.wrapping_add(2);
                    base = Base::Zero;
                    indirect = true;
                }
                // 0x7, 0xa, 0xe are 6309 E/F/W modes; the C++ asserts here
                _ => {
                    eprintln!("unhandled indexed addressing mode");
                    return None;
                }
            }
        }

        let read_base = |cpu: &Self| match base {
            Base::Reg(r) => cpu.get_reg(r),
            Base::Pc => cpu.pc,
            Base::Zero => 0,
        };

        if prepostinc < 0 {
            let v = read_base(self).wrapping_add(prepostinc as u16);
            match base {
                Base::Reg(r) => {
                    self.put_reg(r, v);
                }
                Base::Pc => self.pc = v,
                Base::Zero => {}
            }
        }

        let mut addr = read_base(self).wrapping_add(off as u16);

        if prepostinc > 0 {
            let v = read_base(self).wrapping_add(prepostinc as u16);
            match base {
                Base::Reg(r) => {
                    self.put_reg(r, v);
                }
                Base::Pc => self.pc = v,
                Base::Zero => {}
            }
        }

        if indirect {
            addr = bus.read16(addr as u32, Endian::Big);
        }

        Some(addr)
    }
}

impl Cpu for Cpu6809 {
    fn reset(&mut self, bus: &mut dyn Bus) {
        *self = Cpu6809::default();
        self.pc = bus.read16(0xfffe, Endian::Big);
    }

    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        let mut opcode = bus.read8(self.pc as u32) as usize;
        self.pc = self.pc.wrapping_add(1);

        // 0x10 and 0x11 select the second and third opcode pages
        let index = if opcode == 0x10 || opcode == 0x11 {
            let page = if opcode == 0x10 { 0x100 } else { 0x200 };
            opcode = bus.read8(self.pc as u32) as usize;
            self.pc = self.pc.wrapping_add(1);
            opcode + page
        } else {
            opcode
        };

        let op = OPS[index];

        if op.op == Op::Bad {
            println!("unhandled opcode {:#04x} at {:#06x}", opcode, self.pc.wrapping_sub(1));
            return StepResult::BadOpcode;
        }

        // ---- addressing mode ----
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
                let low = bus.read8(self.pc as u32) as u16;
                self.pc = self.pc.wrapping_add(1);
                let addr = ((self.dp as u16) << 8) | low;
                arg = if op.calcaddr {
                    addr as i32
                } else if op.width == 1 {
                    bus.read8(addr as u32) as i32
                } else {
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
                let postbyte = bus.read8(self.pc as u32);
                self.pc = self.pc.wrapping_add(1);
                let Some(addr) = self.indexed_addr(bus, postbyte) else {
                    return StepResult::BadOpcode;
                };
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
                if op.width == 1 {
                    self.set_hnzvc1(a, b, r);
                } else {
                    self.set_nzvc2(a, b, r);
                }
                self.put_reg(op.target, r as u16);
            }

            Op::Sub | Op::Sbc => {
                // negate-and-add, as the C++ does; carry is derived from the
                // sum rather than modelled as a borrow
                let a = self.get_reg(op.target) as u32;
                let b = (arg as u32).wrapping_neg();
                let mut r = a.wrapping_add(b);
                if op.op == Op::Sbc && self.cc_set(CC_C) {
                    r = r.wrapping_sub(1);
                }
                if op.width == 1 {
                    self.set_hnzvc1(a, b, r);
                } else {
                    self.set_nzvc2(a, b, r);
                }
                self.put_reg(op.target, r as u16);
            }

            Op::Cmp => {
                let a = self.get_reg(op.target) as u32;
                let b = arg as u32;
                let r = a.wrapping_sub(b);
                if op.width == 1 {
                    // byte compares set H here, unlike the 6800 core
                    self.set_hnzvc1(a, b, r);
                } else {
                    self.set_nzvc2(a, b, r);
                }
            }

            Op::And | Op::Bit | Op::Eor | Op::Or => {
                let a = self.get_reg(op.target) as u32;
                let b = arg as u32;
                let r = match op.op {
                    Op::And | Op::Bit => a & b,
                    Op::Eor => a ^ b,
                    _ => a | b,
                };
                self.set_nz1(r);
                self.set_cc(CC_V, false);
                if op.op != Op::Bit {
                    // For andcc/orcc the target *is* CC, so this overwrites the
                    // flag writes just made -- leaving cc = old_cc op arg,
                    // which is the intent.
                    self.put_reg(op.target, r as u16);
                }
            }

            Op::Tst => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, false);
                self.set_nz1(v as u32);
            }

            Op::Clr => {
                self.set_cc(CC_V, false);
                self.set_cc(CC_C, false);
                self.rmw_write(bus, &op, arg, 0);
            }

            Op::Com => {
                let v = !self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, false);
                self.set_cc(CC_C, true);
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Neg => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, v == 0x80);
                self.set_cc(CC_C, v != 0x00);
                self.rmw_write(bus, &op, arg, v.wrapping_neg());
            }

            Op::Asl => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_V, ((v >> 6) & 1) ^ ((v >> 7) & 1) != 0);
                self.set_cc(CC_C, (v >> 7) & 1 != 0);
                self.rmw_write(bus, &op, arg, v << 1);
            }

            Op::Asr => {
                // no fallthrough here, unlike the 6800 core
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_C, v & 1 != 0);
                self.rmw_write(bus, &op, arg, (v & 0x80) | (v >> 1));
            }

            Op::Lsr => {
                let v = self.rmw_read(bus, &op, arg);
                self.set_cc(CC_C, v & 1 != 0);
                self.rmw_write(bus, &op, arg, v >> 1);
            }

            Op::Rol => {
                let v = self.rmw_read(bus, &op, arg);
                let oldc = self.cc_set(CC_C);
                self.set_cc(CC_V, ((v >> 6) & 1) ^ ((v >> 7) & 1) != 0);
                self.set_cc(CC_C, (v >> 7) & 1 != 0);
                self.rmw_write(bus, &op, arg, (v << 1) | if oldc { 1 } else { 0 });
            }

            Op::Ror => {
                let v = self.rmw_read(bus, &op, arg);
                let oldc = self.cc_set(CC_C);
                self.set_cc(CC_C, v & 1 != 0);
                self.rmw_write(bus, &op, arg, if oldc { 0x80 } else { 0 } | (v >> 1));
            }

            Op::Dec => {
                let v = self.rmw_read(bus, &op, arg).wrapping_sub(1);
                self.set_cc(CC_V, v == 0x7f);
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Inc => {
                let v = self.rmw_read(bus, &op, arg).wrapping_add(1);
                self.set_cc(CC_V, v == 0x80);
                self.rmw_write(bus, &op, arg, v);
            }

            Op::Lea => {
                self.put_reg(op.target, arg as u16);
                // only leax/leay affect Z
                if op.target == Reg::X || op.target == Reg::Y {
                    self.set_z2(arg as u32);
                }
            }

            Op::Abx => {
                let v = self.get_reg(Reg::X).wrapping_add(self.get_reg(Reg::B));
                self.put_reg(Reg::X, v);
            }

            Op::Exg | Op::Tfr => {
                let postbyte = bus.read8(self.pc as u32);
                self.pc = self.pc.wrapping_add(1);

                // Illegal and mismatched-width combinations are not rejected,
                // matching the C++.
                let src = reg_from_nibble(postbyte >> 4);
                let dst = reg_from_nibble(postbyte & 0xf);

                let val = src.map(|r| self.get_reg(r)).unwrap_or(0);
                let olddest = dst.map(|r| self.put_reg(r, val)).unwrap_or(0);

                if op.op == Op::Exg {
                    if let Some(r) = src {
                        self.put_reg(r, olddest);
                    }
                }
            }

            Op::Sex => {
                let v = if self.b & 0x80 != 0 { 0xffu8 } else { 0x00 };
                self.put_reg(Reg::A, v as u16);
                let d = self.d();
                self.set_nz2(d as u32);
            }

            Op::Push => {
                // The postbyte names the registers; pushed high to low. When
                // pushing onto U, the "other" stack register is S and vice
                // versa.
                let stack = op.target;
                let other = if stack == Reg::U { Reg::S } else { Reg::U };
                if arg & 0x80 != 0 {
                    let v = self.pc;
                    self.push16(bus, stack, v);
                }
                if arg & 0x40 != 0 {
                    let v = self.get_reg(other);
                    self.push16(bus, stack, v);
                }
                if arg & 0x20 != 0 {
                    let v = self.y;
                    self.push16(bus, stack, v);
                }
                if arg & 0x10 != 0 {
                    let v = self.x;
                    self.push16(bus, stack, v);
                }
                if arg & 0x08 != 0 {
                    let v = self.dp;
                    self.push8(bus, stack, v);
                }
                if arg & 0x04 != 0 {
                    let v = self.b;
                    self.push8(bus, stack, v);
                }
                if arg & 0x02 != 0 {
                    let v = self.a;
                    self.push8(bus, stack, v);
                }
                if arg & 0x01 != 0 {
                    let v = self.cc;
                    self.push8(bus, stack, v);
                }
            }

            Op::Pull => {
                let stack = op.target;
                let other = if stack == Reg::U { Reg::S } else { Reg::U };
                if arg & 0x01 != 0 {
                    self.cc = self.pull8(bus, stack);
                }
                if arg & 0x02 != 0 {
                    self.a = self.pull8(bus, stack);
                }
                if arg & 0x04 != 0 {
                    self.b = self.pull8(bus, stack);
                }
                if arg & 0x08 != 0 {
                    self.dp = self.pull8(bus, stack);
                }
                if arg & 0x10 != 0 {
                    self.x = self.pull16(bus, stack);
                }
                if arg & 0x20 != 0 {
                    self.y = self.pull16(bus, stack);
                }
                if arg & 0x40 != 0 {
                    let v = self.pull16(bus, stack);
                    self.put_reg(other, v);
                }
                if arg & 0x80 != 0 {
                    self.pc = self.pull16(bus, stack);
                }
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

            Op::Bsr => {
                let pc = self.pc;
                self.push16(bus, Reg::S, pc);
                self.pc = (self.pc as i32).wrapping_add(arg) as u16;
            }

            Op::Jmp => {
                // as in the 6800 core, PC is already past the operand so this
                // only catches a jump to the following instruction
                if arg == self.pc as i32 {
                    eprintln!("infinite loop detected, aborting cpu");
                    result = StepResult::InfiniteLoop;
                }
                self.pc = arg as u16;
            }

            Op::Jsr => {
                let pc = self.pc;
                self.push16(bus, Reg::S, pc);
                self.pc = arg as u16;
            }

            Op::Rts => {
                self.pc = self.pull16(bus, Reg::S);
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

            Op::Bad => unreachable!("handled before operand fetch"),
        }

        result
    }

    fn dump(&self) {
        println!(
            "A 0x{:02x} B 0x{:02x} D 0x{:04x} X 0x{:04x} Y 0x{:04x} U 0x{:04x} S 0x{:04x} DP 0x{:02x} CC 0x{:02x} ({}{}{}{}{}) PC 0x{:04x}",
            self.a, self.b, self.d(), self.x, self.y, self.u, self.s, self.dp, self.cc,
            if self.cc_set(CC_H) { 'h' } else { ' ' },
            if self.cc_set(CC_N) { 'n' } else { ' ' },
            if self.cc_set(CC_Z) { 'z' } else { ' ' },
            if self.cc_set(CC_V) { 'v' } else { ' ' },
            if self.cc_set(CC_C) { 'c' } else { ' ' },
            self.pc
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        // must match Cpu6809::TraceInstruction() in cpu/cpu6809.cpp exactly
        writeln!(
            out,
            "PC={:04x} A={:02x} B={:02x} X={:04x} Y={:04x} U={:04x} S={:04x} DP={:02x} CC={:02x}",
            self.pc, self.a, self.b, self.x, self.y, self.u, self.s, self.dp, self.cc
        )
    }
}

/// exg/tfr register nibble encoding. Undefined codes read as 0 and discard
/// writes, matching the C++ `default:` arms.
fn reg_from_nibble(n: u8) -> Option<Reg> {
    match n & 0xf {
        0 => Some(Reg::D),
        1 => Some(Reg::X),
        2 => Some(Reg::Y),
        3 => Some(Reg::U),
        4 => Some(Reg::S),
        5 => Some(Reg::Pc),
        8 => Some(Reg::A),
        9 => Some(Reg::B),
        10 => Some(Reg::Cc),
        11 => Some(Reg::Dp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{run_steps, TestBus};

    /// Load a hand-assembled program at 0xe000 and reset into it.
    fn boot(prog: &[u8]) -> (Cpu6809, TestBus) {
        let mut bus = TestBus::new();
        bus.load(0xe000, prog);
        bus.set_reset_vector(0xe000);
        let mut cpu = Cpu6809::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn reset_loads_pc_from_the_vector() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x1234);
        let mut cpu = Cpu6809::new();
        cpu.x = 0xffff;
        cpu.dp = 0xff;
        cpu.reset(&mut bus);
        assert_eq!(cpu.pc, 0x1234);
        assert_eq!((cpu.a, cpu.b, cpu.x, cpu.y, cpu.u, cpu.s), (0, 0, 0, 0, 0, 0));
        assert_eq!((cpu.dp, cpu.cc), (0, 0));
    }

    /// Smoke test: sum four bytes through a post-increment indexed loop.
    /// Covers the 0x10 opcode page, indexed auto-increment, cmpx and a taken
    /// conditional branch.
    #[test]
    fn sums_a_table_with_a_post_increment_loop() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x10, 0xce, 0x00, 0xff, // e000  lds  #0x00ff
            0x8e, 0x01, 0x00,       // e004  ldx  #0x0100
            0x4f,                   // e007  clra
            0xab, 0x80,             // e008  adda ,x+         <- loop
            0x8c, 0x01, 0x04,       // e00a  cmpx #0x0104
            0x26, 0xf9,             // e00d  bne  loop
            0xb7, 0x02, 0x00,       // e00f  sta  0x0200
        ]);
        bus.load(0x0100, &[0x01, 0x02, 0x03, 0x04]);

        // 3 setup + 4 iterations of 3 + the store
        run_steps(&mut cpu, &mut bus, 16);

        assert_eq!(cpu.a, 0x0a);
        assert_eq!(cpu.x, 0x0104);
        assert_eq!(cpu.s, 0x00ff);
        assert_eq!(cpu.pc, 0xe012);
        assert_eq!(bus.mem[0x0200], 0x0a);
    }

    /// Execute one `lea` at 0xe000 over a known register file. `lea` writes the
    /// effective address straight into a register, so this reads out exactly
    /// what `indexed_addr` computed -- including any base-register mutation.
    fn lea_with(opcode: u8, operand: &[u8], setup: impl FnOnce(&mut Cpu6809)) -> Cpu6809 {
        let mut bus = TestBus::new();
        let mut prog = vec![opcode];
        prog.extend_from_slice(operand);
        bus.load(0xe000, &prog);
        // pointers the indirect modes chase
        bus.load(0x0100, &[0xca, 0xfe]);
        bus.load(0x1234, &[0xbe, 0xef]);

        let mut cpu = Cpu6809::new();
        cpu.x = 0x0100;
        cpu.y = 0x0200;
        cpu.u = 0x0300;
        cpu.s = 0x0400;
        cpu.a = 0x12;
        cpu.b = 0x34;
        cpu.pc = 0xe000;
        setup(&mut cpu);

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        cpu
    }

    fn lea(opcode: u8, operand: &[u8]) -> Cpu6809 {
        lea_with(opcode, operand, |_| {})
    }

    const LEAX: u8 = 0x30;
    const LEAU: u8 = 0x33;

    #[test]
    fn indexed_postbytes_compute_the_effective_address() {
        // no offset, and the 5-bit signed offset form
        assert_eq!(lea(LEAU, &[0x84]).u, 0x0100); // ,x
        assert_eq!(lea(LEAU, &[0x05]).u, 0x0105); // 5,x
        assert_eq!(lea(LEAU, &[0x1f]).u, 0x00ff); // -1,x

        // accumulator offsets, sign extended
        assert_eq!(lea(LEAU, &[0x85]).u, 0x0134); // b,x with b = 0x34
        assert_eq!(lea(LEAU, &[0x86]).u, 0x0112); // a,x with a = 0x12
        assert_eq!(lea(LEAU, &[0x8b]).u, 0x1334); // d,x with d = 0x1234
        assert_eq!(lea_with(LEAU, &[0x85], |c| c.b = 0xff).u, 0x00ff); // -1,x

        // 8- and 16-bit constant offsets
        assert_eq!(lea(LEAU, &[0x88, 0x10]).u, 0x0110);
        assert_eq!(lea(LEAU, &[0x88, 0xf0]).u, 0x00f0); // -16,x
        assert_eq!(lea(LEAU, &[0x89, 0x01, 0x00]).u, 0x0200);

        // program-counter relative: the base is PC *after* the offset bytes
        assert_eq!(lea(LEAU, &[0x8c, 0x05]).u, 0xe008);
        assert_eq!(lea(LEAU, &[0x8d, 0x01, 0x00]).u, 0xe104);

        // indirect
        assert_eq!(lea(LEAU, &[0x94]).u, 0xcafe); // [,x] -> the word at 0x0100
        assert_eq!(lea(LEAU, &[0x9f, 0x12, 0x34]).u, 0xbeef); // [0x1234]

        // the other three base registers
        assert_eq!(lea(LEAU, &[0xa4]).u, 0x0200); // ,y
        assert_eq!(lea(LEAU, &[0xe4]).u, 0x0400); // ,s
        assert_eq!(lea(LEAX, &[0xc4]).x, 0x0300); // ,u
    }

    #[test]
    fn indexed_auto_increment_modes_update_the_base_register() {
        let cpu = lea(LEAU, &[0x80]); // ,x+
        assert_eq!((cpu.u, cpu.x), (0x0100, 0x0101));

        let cpu = lea(LEAU, &[0x81]); // ,x++
        assert_eq!((cpu.u, cpu.x), (0x0100, 0x0102));

        // the decrements happen *before* the address is formed
        let cpu = lea(LEAU, &[0x82]); // ,-x
        assert_eq!((cpu.u, cpu.x), (0x00ff, 0x00ff));

        let cpu = lea(LEAU, &[0x83]); // ,--x
        assert_eq!((cpu.u, cpu.x), (0x00fe, 0x00fe));
    }

    /// Pushes go high register to low and pre-decrement; pulls come back in the
    /// mirrored order. Getting either backwards corrupts every subroutine call.
    #[test]
    fn push_and_pull_round_trip_in_postbyte_order() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x10, 0xce, 0x04, 0x00, // lds  #0x0400
            0x86, 0x11,             // lda  #0x11
            0xc6, 0x22,             // ldb  #0x22
            0x8e, 0x33, 0x44,       // ldx  #0x3344
            0x34, 0x16,             // pshs a,b,x
            0x4f,                   // clra
            0x5f,                   // clrb
            0x8e, 0x00, 0x00,       // ldx  #0x0000
            0x35, 0x16,             // puls a,b,x
        ]);

        run_steps(&mut cpu, &mut bus, 5);
        assert_eq!(cpu.s, 0x03fc);
        assert_eq!(&bus.mem[0x03fc..0x0400], &[0x11, 0x22, 0x33, 0x44]);

        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!((cpu.a, cpu.b, cpu.x), (0x11, 0x22, 0x3344));
        assert_eq!(cpu.s, 0x0400);
    }

    /// Unlike the 6800 core, `asr` here really is an arithmetic shift: the sign
    /// bit is preserved and the operand is read exactly once.
    #[test]
    fn asr_preserves_the_sign_bit_and_reads_its_operand_once() {
        let (mut cpu, mut bus) = boot(&[0x86, 0x80, 0x47]); // lda #0x80 ; asra
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.a, 0xc0);

        let (mut cpu, mut bus) = boot(&[0x77, 0x01, 0x00]); // asr 0x0100
        bus.mem[0x0100] = 0x81;
        bus.watch = Some(0x0100);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.watch_reads, 1);
        assert_eq!(bus.mem[0x0100], 0xc0);
        assert!(cpu.cc_set(CC_C));
        // N and Z come from the written value, set after the write
        assert!(cpu.cc_set(CC_N));
        assert!(!cpu.cc_set(CC_Z));
    }

    #[test]
    fn exg_swaps_a_pair_and_tfr_copies_one_way() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xcc, 0x12, 0x34,       // ldd #0x1234
            0x8e, 0x56, 0x78,       // ldx #0x5678
            0x1e, 0x01,             // exg d,x
            0x1f, 0x89,             // tfr a,b
        ]);

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.d(), cpu.x), (0x5678, 0x1234));

        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.a, cpu.b), (0x56, 0x56));
    }

    #[test]
    fn an_unimplemented_opcode_stops_the_run() {
        // 0x01 has no entry on the base page
        let (mut cpu, mut bus) = boot(&[0x01]);
        assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode);
    }
}
