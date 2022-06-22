// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2013,2022 Travis Geiselbrecht
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
#include "cpu6800.h"

#include <cstdio>
#include <cassert>
#include <iostream>

#include "system/system.h"
#include "bits.h"

#define TRACE 0

#define TRACEF(str, x...) do { if (TRACE) printf(str, ## x); } while (0)

using namespace std;

using Endian = System::Endian;
using regnum = Cpu6800::regnum;

/* exceptions */
#define EXC_RESET 0x1
#define EXC_NMI   0x2
#define EXC_SWI   0x4
#define EXC_IRQ   0x8

#define COND_A  0x0
#define COND_N  0x1
#define COND_HI 0x2
#define COND_LS 0x3
#define COND_CC 0x4
#define COND_CS 0x5
#define COND_NE 0x6
#define COND_EQ 0x7
#define COND_VC 0x8
#define COND_VS 0x9
#define COND_PL 0xa
#define COND_MI 0xb
#define COND_GE 0xc
#define COND_LT 0xd
#define COND_GT 0xe
#define COND_LE 0xf

#define CC_C  (0x01)
#define CC_V  (0x02)
#define CC_Z  (0x04)
#define CC_N  (0x08)
#define CC_I  (0x10)
#define CC_H  (0x20)

/* decode table */
enum addrMode {
    UNKNOWN,
    IMPLIED,
    IMMEDIATE,
    DIRECT,
    EXTENDED,
    INDEXED,
    BRANCH,
};

enum op {
    BADOP,
    ADD,
    ADD_ACCUM,
    ADC,
    SUB,
    SUB_ACCUM,
    SBC,
    CMP,
    CMP_ACCUM,
    AND,
    BIT,
    EOR,
    OR,
    NOP,
    CLR,
    COM,
    NEG,
    DEC,
    INC,
    TST,
    ASL,
    ASR,
    LSR,
    ROL,
    ROR,
    TFR,
    TFR_CC,
    PUSH,
    PULL,
    BRA,
    BSR,
    JMP,
    JSR,
    RTS,
    LD,
    ST,
    SEcc,
    CLcc,
};

struct opdecode {
    const char *name;
    addrMode mode;
    int width; // 1 or 2
    enum op op;
    Cpu6800::regnum targetreg;
    union {
        unsigned int cond;
        bool calcaddr;
        uint8_t cc_flag;
    };
};

// opcode table, one per byte
static constexpr opdecode ops[256] = {
// alu ops
    [0x8b] = { "adda", IMMEDIATE, 1, ADD, regnum::REG_A, { 0 } },
    [0xcb] = { "addb", IMMEDIATE, 1, ADD, regnum::REG_B, { 0 } },
    [0x9b] = { "adda", DIRECT, 1, ADD, regnum::REG_A, { 0 } },
    [0xdb] = { "addb", DIRECT, 1, ADD, regnum::REG_B, { 0 } },
    [0xab] = { "adda", INDEXED, 1, ADD, regnum::REG_A, { 0 } },
    [0xeb] = { "addb", INDEXED, 1, ADD, regnum::REG_B, { 0 } },
    [0xbb] = { "adda", EXTENDED, 1, ADD, regnum::REG_A, { 0 } },
    [0xfb] = { "addb", EXTENDED, 1, ADD, regnum::REG_B, { 0 } },

    [0x1b] = { "aba",  IMPLIED, 1, ADD_ACCUM, regnum::REG_A, { 0 } },

    [0x89] = { "adca", IMMEDIATE, 1, ADC, regnum::REG_A, { 0 } },
    [0xc9] = { "adcb", IMMEDIATE, 1, ADC, regnum::REG_B, { 0 } },
    [0x99] = { "adca", DIRECT, 1, ADC, regnum::REG_A, { 0 } },
    [0xd9] = { "adcb", DIRECT, 1, ADC, regnum::REG_B, { 0 } },
    [0xa9] = { "adca", INDEXED, 1, ADC, regnum::REG_A, { 0 } },
    [0xe9] = { "adcb", INDEXED, 1, ADC, regnum::REG_B, { 0 } },
    [0xb9] = { "adca", EXTENDED, 1, ADC, regnum::REG_A, { 0 } },
    [0xf9] = { "adcb", EXTENDED, 1, ADC, regnum::REG_B, { 0 } },

    [0x80] = { "suba", IMMEDIATE, 1, SUB, regnum::REG_A, { 0 } },
    [0xc0] = { "subb", IMMEDIATE, 1, SUB, regnum::REG_B, { 0 } },
    [0x90] = { "suba", DIRECT, 1, SUB, regnum::REG_A, { 0 } },
    [0xd0] = { "subb", DIRECT, 1, SUB, regnum::REG_B, { 0 } },
    [0xa0] = { "suba", INDEXED, 1, SUB, regnum::REG_A, { 0 } },
    [0xe0] = { "subb", INDEXED, 1, SUB, regnum::REG_B, { 0 } },
    [0xb0] = { "suba", EXTENDED, 1, SUB, regnum::REG_A, { 0 } },
    [0xf0] = { "subb", EXTENDED, 1, SUB, regnum::REG_B, { 0 } },

    [0x10] = { "sba",  IMPLIED, 1, SUB_ACCUM, regnum::REG_A, { 0 } },

    [0x82] = { "sbca", IMMEDIATE, 1, SBC, regnum::REG_A, { 0 } },
    [0xc2] = { "sbcb", IMMEDIATE, 1, SBC, regnum::REG_B, { 0 } },
    [0x92] = { "sbca", DIRECT, 1, SBC, regnum::REG_A, { 0 } },
    [0xd2] = { "sbcb", DIRECT, 1, SBC, regnum::REG_B, { 0 } },
    [0xa2] = { "sbca", INDEXED, 1, SBC, regnum::REG_A, { 0 } },
    [0xe2] = { "sbcb", INDEXED, 1, SBC, regnum::REG_B, { 0 } },
    [0xb2] = { "sbca", EXTENDED, 1, SBC, regnum::REG_A, { 0 } },
    [0xf2] = { "sbcb", EXTENDED, 1, SBC, regnum::REG_B, { 0 } },

    [0x81] = { "cmpa", IMMEDIATE, 1, CMP, regnum::REG_A, { 0 } },
    [0xc1] = { "cmpb", IMMEDIATE, 1, CMP, regnum::REG_B, { 0 } },
    [0x8c] = { "cpx", IMMEDIATE, 2, CMP, regnum::REG_IX, { 0 } },
    [0x91] = { "cmpa", DIRECT, 1, CMP, regnum::REG_A, { 0 } },
    [0xd1] = { "cmpb", DIRECT, 1, CMP, regnum::REG_B, { 0 } },
    [0x9c] = { "cpx", DIRECT, 2, CMP, regnum::REG_IX, { 0 } },
    [0xa1] = { "cmpa", INDEXED, 1, CMP, regnum::REG_A, { 0 } },
    [0xe1] = { "cmpb", INDEXED, 1, CMP, regnum::REG_B, { 0 } },
    [0xac] = { "cpx", INDEXED, 2, CMP, regnum::REG_IX, { 0 } },
    [0xb1] = { "cmpa", EXTENDED, 1, CMP, regnum::REG_A, { 0 } },
    [0xf1] = { "cmpb", EXTENDED, 1, CMP, regnum::REG_B, { 0 } },
    [0xbc] = { "cpx", EXTENDED, 2, CMP, regnum::REG_IX, { 0 } },

    [0x11] = { "cba", IMPLIED, 1, CMP_ACCUM, regnum::REG_A, { 0 } },

    [0x84] = { "anda", IMMEDIATE, 1, AND, regnum::REG_A, { 0 } },
    [0xc4] = { "andb", IMMEDIATE, 1, AND, regnum::REG_B, { 0 } },
    [0x94] = { "anda", DIRECT, 1, AND, regnum::REG_A, { 0 } },
    [0xd4] = { "andb", DIRECT, 1, AND, regnum::REG_B, { 0 } },
    [0xa4] = { "anda", INDEXED, 1, AND, regnum::REG_A, { 0 } },
    [0xe4] = { "andb", INDEXED, 1, AND, regnum::REG_B, { 0 } },
    [0xb4] = { "anda", EXTENDED, 1, AND, regnum::REG_A, { 0 } },
    [0xf4] = { "andb", EXTENDED, 1, AND, regnum::REG_B, { 0 } },

    [0x85] = { "bita", IMMEDIATE, 1, BIT, regnum::REG_A, { 0 } },
    [0xc5] = { "bitb", IMMEDIATE, 1, BIT, regnum::REG_B, { 0 } },
    [0x95] = { "bita", DIRECT, 1, BIT, regnum::REG_A, { 0 } },
    [0xd5] = { "bitb", DIRECT, 1, BIT, regnum::REG_B, { 0 } },
    [0xa5] = { "bita", INDEXED, 1, BIT, regnum::REG_A, { 0 } },
    [0xe5] = { "bitb", INDEXED, 1, BIT, regnum::REG_B, { 0 } },
    [0xb5] = { "bita", EXTENDED, 1, BIT, regnum::REG_A, { 0 } },
    [0xf5] = { "bitb", EXTENDED, 1, BIT, regnum::REG_B, { 0 } },

    [0x88] = { "eora", IMMEDIATE, 1, EOR, regnum::REG_A, { 0 } },
    [0xc8] = { "eorb", IMMEDIATE, 1, EOR, regnum::REG_B, { 0 } },
    [0x98] = { "eora", DIRECT, 1, EOR, regnum::REG_A, { 0 } },
    [0xd8] = { "eorb", DIRECT, 1, EOR, regnum::REG_B, { 0 } },
    [0xa8] = { "eora", INDEXED, 1, EOR, regnum::REG_A, { 0 } },
    [0xe8] = { "eorb", INDEXED, 1, EOR, regnum::REG_B, { 0 } },
    [0xb8] = { "eora", EXTENDED, 1, EOR, regnum::REG_A, { 0 } },
    [0xf8] = { "eorb", EXTENDED, 1, EOR, regnum::REG_B, { 0 } },

    [0x8a] = { "ora", IMMEDIATE, 1, OR, regnum::REG_A, { 0 } },
    [0xca] = { "orb", IMMEDIATE, 1, OR, regnum::REG_B, { 0 } },
    [0x9a] = { "ora", DIRECT, 1, OR, regnum::REG_A, { 0 } },
    [0xda] = { "orb", DIRECT, 1, OR, regnum::REG_B, { 0 } },
    [0xaa] = { "ora", INDEXED, 1, OR, regnum::REG_A, { 0 } },
    [0xea] = { "orb", INDEXED, 1, OR, regnum::REG_B, { 0 } },
    [0xba] = { "ora", EXTENDED, 1, OR, regnum::REG_A, { 0 } },
    [0xfa] = { "orb", EXTENDED, 1, OR, regnum::REG_B, { 0 } },

// misc
    [0x01] = { "nop",  IMPLIED, 1, NOP, regnum::REG_A, { 0 } },

    [0x16] = { "tab",  IMPLIED, 1, TFR, regnum::REG_B, { 0 } },
    [0x17] = { "tba",  IMPLIED, 1, TFR, regnum::REG_A, { 0 } },

    [0x35] = { "txs",  IMPLIED, 2, TFR, regnum::REG_SP, { 0 } },
    [0x30] = { "tsx",  IMPLIED, 2, TFR, regnum::REG_IX, { 0 } },

    [0x07] = { "tpa",  IMPLIED, 1, TFR_CC, regnum::REG_A, { 0 } },
    [0x06] = { "tap",  IMPLIED, 1, TFR_CC, regnum::REG_CC, { 0 } },

    [0x0b] = { "sev",  IMPLIED, 1, SEcc, regnum::REG_PC, { .cc_flag = CC_V } },
    [0x0d] = { "sec",  IMPLIED, 1, SEcc, regnum::REG_PC, { .cc_flag = CC_C } },
    [0x0f] = { "sei",  IMPLIED, 1, SEcc, regnum::REG_PC, { .cc_flag = CC_I } },

    [0x0a] = { "clv",  IMPLIED, 1, CLcc, regnum::REG_PC, { .cc_flag = CC_V } },
    [0x0c] = { "clc",  IMPLIED, 1, CLcc, regnum::REG_PC, { .cc_flag = CC_C } },
    [0x0e] = { "cli",  IMPLIED, 1, CLcc, regnum::REG_PC, { .cc_flag = CC_I } },

    [0x4f] = { "clra", IMPLIED,  1, CLR, regnum::REG_A, { 0 } },
    [0x5f] = { "clrb", IMPLIED,  1, CLR, regnum::REG_B, { 0 } },
    [0x6f] = { "clr",  INDEXED,  1, CLR, regnum::REG_A, { .calcaddr = true } },
    [0x7f] = { "clr",  EXTENDED, 1, CLR, regnum::REG_A, { .calcaddr = true } },

    [0x43] = { "coma", IMPLIED,  1, COM, regnum::REG_A, { 0 } },
    [0x53] = { "comb", IMPLIED,  1, COM, regnum::REG_B, { 0 } },
    [0x63] = { "com",  INDEXED,  1, COM, regnum::REG_A, { .calcaddr = true } },
    [0x73] = { "com",  EXTENDED, 1, COM, regnum::REG_A, { .calcaddr = true } },

    [0x40] = { "nega", IMPLIED,  1, NEG, regnum::REG_A, { 0 } },
    [0x50] = { "negb", IMPLIED,  1, NEG, regnum::REG_B, { 0 } },
    [0x60] = { "neg",  INDEXED,  1, NEG, regnum::REG_A, { .calcaddr = true } },
    [0x70] = { "neg",  EXTENDED, 1, NEG, regnum::REG_A, { .calcaddr = true } },

    [0x4a] = { "deca", IMPLIED,  1, DEC, regnum::REG_A, { 0 } },
    [0x5a] = { "decb", IMPLIED,  1, DEC, regnum::REG_B, { 0 } },
    [0x6a] = { "dec",  INDEXED,  1, DEC, regnum::REG_A, { .calcaddr = true } },
    [0x7a] = { "dec",  EXTENDED, 1, DEC, regnum::REG_A, { .calcaddr = true } },
    [0x34] = { "des",  IMPLIED, 2, DEC, regnum::REG_SP, { 0 } },
    [0x09] = { "dex",  IMPLIED, 2, DEC, regnum::REG_IX, { 0 } },

    [0x4c] = { "inca", IMPLIED,  1, INC, regnum::REG_A, { 0 } },
    [0x5c] = { "incb", IMPLIED,  1, INC, regnum::REG_B, { 0 } },
    [0x6c] = { "inc",  INDEXED,  1, INC, regnum::REG_A, { .calcaddr = true } },
    [0x7c] = { "inc",  EXTENDED, 1, INC, regnum::REG_A, { .calcaddr = true } },
    [0x31] = { "ins",  IMPLIED, 2, INC, regnum::REG_SP, { 0 } },
    [0x08] = { "inx",  IMPLIED, 2, INC, regnum::REG_IX, { 0 } },

    [0x48] = { "asla", IMPLIED,  1, ASL, regnum::REG_A, { 0 } },
    [0x58] = { "aslb", IMPLIED,  1, ASL, regnum::REG_B, { 0 } },
    [0x68] = { "asl",  INDEXED,  1, ASL, regnum::REG_A, { .calcaddr = true } },
    [0x78] = { "asl",  EXTENDED, 1, ASL, regnum::REG_A, { .calcaddr = true } },

    [0x47] = { "asra", IMPLIED,  1, ASR, regnum::REG_A, { 0 } },
    [0x57] = { "asrb", IMPLIED,  1, ASR, regnum::REG_B, { 0 } },
    [0x67] = { "asr",  INDEXED,  1, ASR, regnum::REG_A, { .calcaddr = true } },
    [0x77] = { "asr",  EXTENDED, 1, ASR, regnum::REG_A, { .calcaddr = true } },

    [0x44] = { "lsra", IMPLIED,  1, LSR, regnum::REG_A, { 0 } },
    [0x54] = { "lsrb", IMPLIED,  1, LSR, regnum::REG_B, { 0 } },
    [0x64] = { "lsr",  INDEXED,  1, LSR, regnum::REG_A, { .calcaddr = true } },
    [0x74] = { "lsr",  EXTENDED, 1, LSR, regnum::REG_A, { .calcaddr = true } },

    [0x49] = { "rola", IMPLIED,  1, ROL, regnum::REG_A, { 0 } },
    [0x59] = { "rolb", IMPLIED,  1, ROL, regnum::REG_B, { 0 } },
    [0x69] = { "rol",  INDEXED,  1, ROL, regnum::REG_A, { .calcaddr = true } },
    [0x79] = { "rol",  EXTENDED, 1, ROL, regnum::REG_A, { .calcaddr = true } },

    [0x46] = { "rora", IMPLIED,  1, ROR, regnum::REG_A, { 0 } },
    [0x56] = { "rorb", IMPLIED,  1, ROR, regnum::REG_B, { 0 } },
    [0x66] = { "ror",  INDEXED,  1, ROR, regnum::REG_A, { .calcaddr = true } },
    [0x76] = { "ror",  EXTENDED, 1, ROR, regnum::REG_A, { .calcaddr = true } },

    [0x4d] = { "tsta", IMPLIED,  1, TST, regnum::REG_A, { 0 } },
    [0x5d] = { "tstb", IMPLIED,  1, TST, regnum::REG_B, { 0 } },
    [0x6d] = { "tst",  INDEXED,  1, TST, regnum::REG_A, { .calcaddr = true } },
    [0x7d] = { "tst",  EXTENDED, 1, TST, regnum::REG_A, { .calcaddr = true } },

// push/pull
    [0x36] = { "psha", IMPLIED, 1, PUSH, regnum::REG_A, { 0 } },
    [0x37] = { "pshb", IMPLIED, 1, PUSH, regnum::REG_B, { 0 } },

    [0x32] = { "pula", IMPLIED, 1, PULL, regnum::REG_A, { 0 } },
    [0x33] = { "pulb", IMPLIED, 1, PULL, regnum::REG_B, { 0 } },

// loads
    [0x86] = { "lda",  IMMEDIATE, 1, LD, regnum::REG_A, { 0 } },
    [0xc6] = { "ldb",  IMMEDIATE, 1, LD, regnum::REG_B, { 0 } },
    [0x8e] = { "lds",  IMMEDIATE, 2, LD, regnum::REG_SP, { 0 } },
    [0xce] = { "ldx",  IMMEDIATE, 2, LD, regnum::REG_IX, { 0 } },

    [0x96] = { "lda",  DIRECT, 1, LD, regnum::REG_A, { 0 } },
    [0xd6] = { "ldb",  DIRECT, 1, LD, regnum::REG_B, { 0 } },
    [0x9e] = { "lds",  DIRECT, 2, LD, regnum::REG_SP, { 0 } },
    [0xde] = { "ldx",  DIRECT, 2, LD, regnum::REG_IX, { 0 } },

    [0xb6] = { "lda",  EXTENDED, 1, LD, regnum::REG_A, { 0 } },
    [0xf6] = { "ldb",  EXTENDED, 1, LD, regnum::REG_B, { 0 } },
    [0xbe] = { "lds",  EXTENDED, 2, LD, regnum::REG_SP, { 0 } },
    [0xfe] = { "ldx",  EXTENDED, 2, LD, regnum::REG_IX, { 0 } },

    [0xa6] = { "lda",  INDEXED, 1, LD, regnum::REG_A, { 0 } },
    [0xe6] = { "ldb",  INDEXED, 1, LD, regnum::REG_B, { 0 } },
    [0xae] = { "lds",  INDEXED, 2, LD, regnum::REG_SP, { 0 } },
    [0xee] = { "ldx",  INDEXED, 2, LD, regnum::REG_IX, { 0 } },

// stores
    [0x97] = { "sta",  DIRECT, 1, ST, regnum::REG_A, { .calcaddr = true } },
    [0xd7] = { "stb",  DIRECT, 1, ST, regnum::REG_B, { .calcaddr = true } },
    [0x9f] = { "sts",  DIRECT, 2, ST, regnum::REG_SP, { .calcaddr = true } },
    [0xdf] = { "stx",  DIRECT, 2, ST, regnum::REG_IX, { .calcaddr = true } },

    [0xb7] = { "sta",  EXTENDED, 1, ST, regnum::REG_A, { .calcaddr = true } },
    [0xf7] = { "stb",  EXTENDED, 1, ST, regnum::REG_B, { .calcaddr = true } },
    [0xbf] = { "sts",  EXTENDED, 2, ST, regnum::REG_SP, { .calcaddr = true } },
    [0xff] = { "stx",  EXTENDED, 2, ST, regnum::REG_IX, { .calcaddr = true } },

    [0xa7] = { "sta",  INDEXED, 1, ST, regnum::REG_A, { .calcaddr = true } },
    [0xe7] = { "stb",  INDEXED, 1, ST, regnum::REG_B, { .calcaddr = true } },
    [0xaf] = { "sts",  INDEXED, 2, ST, regnum::REG_SP, { .calcaddr = true } },
    [0xef] = { "stx",  INDEXED, 2, ST, regnum::REG_IX, { .calcaddr = true } },

// branches
    [0x20] = { "bra",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_A } },
    [0x22] = { "bhi",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_HI } },
    [0x23] = { "bls",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_LS } },
    [0x24] = { "bcc",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_CC } },
    [0x25] = { "bcs",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_CS } },
    [0x26] = { "bne",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_NE } },
    [0x27] = { "beq",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_EQ } },
    [0x28] = { "bvc",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_VC } },
    [0x29] = { "bvs",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_VS } },
    [0x2a] = { "bpl",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_PL } },
    [0x2b] = { "bmi",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_MI } },
    [0x2c] = { "bge",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_GE } },
    [0x2d] = { "blt",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_LT } },
    [0x2e] = { "bgt",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_GT } },
    [0x2f] = { "ble",  BRANCH, 1, BRA, regnum::REG_PC, { .cond = COND_LE } },
    [0x8d] = { "bsr",  BRANCH, 1, BSR, regnum::REG_PC, { 0 } },

    [0x6e] = { "jmp",  INDEXED,  1, JMP, regnum::REG_PC, { .calcaddr = true } },
    [0x7e] = { "jmp",  EXTENDED, 1, JMP, regnum::REG_PC, { .calcaddr = true } },

    [0xad] = { "jsr",  INDEXED,  1, JSR, regnum::REG_PC, { .calcaddr = true } },
    [0xbd] = { "jsr",  EXTENDED, 1, JSR, regnum::REG_PC, { .calcaddr = true } },

    [0x39] = { "rts",  IMPLIED,  1, RTS, regnum::REG_PC, { 0 } },
};

Cpu6800::Cpu6800(System &mSys)
    :   Cpu(mSys) {
    Reset();
}

Cpu6800::~Cpu6800() = default;

void Cpu6800::Reset() {
    // put all the registers into their default values
    mA = mB = 0;
    mIX = 0;
    mSP = 0;
    mCC = 0;
    mPC = 0;

    // put the cpu in reset
    mException = EXC_RESET;
}

bool Cpu6800::TestBranchCond(unsigned int cond) {
    const bool C = mCC & CC_C;
    const bool N = mCC & CC_N;
    const bool Z = mCC & CC_Z;
    const bool V = mCC & CC_V;

    switch (cond) {
        default:
        case COND_A:
            return true;
        case COND_N:
            return false;
        case COND_HI:
            return !(C || Z);
        case COND_LS:
            return C || Z;
        case COND_CC:
            return !C;
        case COND_CS:
            return C;
        case COND_NE:
            return !Z;
        case COND_EQ:
            return Z;
        case COND_VC:
            return !V;
        case COND_VS:
            return V;
        case COND_PL:
            return !N;
        case COND_MI:
            return N;

        case COND_GE:
            return !(N ^ V);
        case COND_LT:
            return N ^ V;
        case COND_GT:
            return !((N ^ V) || Z);
        case COND_LE:
            return (N ^ V) || Z;
    }
}

#define SET_CC_BIT(bit) (mCC | (bit))
#define CLR_CC_BIT(bit) (mCC & ~(bit))

#define SET_Z1(result) do { mCC = ((result & 0xff) == 0) ? SET_CC_BIT(CC_Z) : CLR_CC_BIT(CC_Z); } while (0)
#define SET_Z2(result) do { mCC = ((result & 0xffff) == 0) ? SET_CC_BIT(CC_Z) : CLR_CC_BIT(CC_Z); } while (0)
#define SET_N1(result) do { mCC = BIT(result, 7) ? SET_CC_BIT(CC_N) : CLR_CC_BIT(CC_N); } while (0)
#define SET_N2(result) do { mCC = BIT(result, 15) ? SET_CC_BIT(CC_N) : CLR_CC_BIT(CC_N); } while (0)
#define SET_C1(result) do { mCC = BIT(result, 8) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C); } while (0)
#define SET_C2(result) do { mCC = BIT(result, 16) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C); } while (0)
#define SET_V1(a, b, result) do { mCC = BIT((a)^(b)^(result)^((result)>>1), 7) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V); } while (0)
#define SET_V2(a, b, result) do { mCC = BIT((a)^(b)^(result)^((result)>>1), 15) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V); } while (0)
#define SET_H(a, b, result) do { mCC = BIT((a)^(b)^(result), 4) ? SET_CC_BIT(CC_H) : CLR_CC_BIT(CC_H); } while (0)

#define SET_NZ1(result) do { SET_N1(result); SET_Z1(result); } while (0)
#define SET_NZ2(result) do { SET_N2(result); SET_Z2(result); } while (0)

#define SET_NZVC1(a, b, result) do { SET_N1(result); SET_Z1(result); SET_V1(a, b, result); SET_C1(result); } while (0)
#define SET_HNZVC1(a, b, result) do { SET_NZVC1(a, b, result); SET_H(a, b, result); } while (0)
#define SET_NZV2(a, b, result) do { SET_N2(result); SET_Z2(result); SET_V2(a, b, result); } while (0)
#define SET_NZVC2(a, b, result) do { SET_NZV2(a, b, result); SET_C2(result); } while (0)

#define PUSH16(val) do { \
    uint16_t __sp = GetReg(regnum::REG_SP); \
    mSys.MemWrite8(__sp--, (val) & 0xff); \
    mSys.MemWrite8(__sp--, ((val) >> 8) & 0xff); \
    PutReg(regnum::REG_SP, __sp); \
} while (0)
#define PUSH8(val) do { \
    uint16_t __sp = GetReg(regnum::REG_SP); \
    mSys.MemWrite8(__sp--, val); \
    PutReg(regnum::REG_SP, __sp); \
} while (0)
#define PULL16(val) do { \
    uint16_t __sp = GetReg(regnum::REG_SP); \
    uint16_t __temp = mSys.MemRead8(++__sp) << 8;\
    __temp |= mSys.MemRead8(++__sp); \
    PutReg(regnum::REG_SP, __sp); \
    val = __temp; \
} while (0)
#define PULL8(val) do { \
    uint16_t __sp = GetReg(regnum::REG_SP); \
    val = mSys.MemRead8(++__sp);\
    PutReg(regnum::REG_SP, __sp); \
} while (0)

int Cpu6800::Run() {

    bool done = false;
    while (!done) {
        uint8_t opcode;

        if (mException) {
            if (mException & EXC_RESET) {
                // reset, branch to the reset vector
                mPC = mSys.MemRead16(0xfffe, Endian::BIG);
                mException = 0; // clear the rest of the pending irqs
            }
            assert(!mException);
        }

        // fetch the first byte of the opcode
        opcode = mSys.MemRead8(mPC++);

        const opdecode *op = &ops[opcode];
        uint8_t temp8;
        uint16_t temp16;
        int arg = 0;

        TRACEF("opcode %#02x %s", opcode, op->name);

        if (op->op == BADOP) {
            goto badop;
        }

        // get the addressing mode
        TRACEF(" amode");
        switch (op->mode) {
            case IMPLIED:
                TRACEF(" IMP");
                break;
            case IMMEDIATE:
                TRACEF(" IMM");
                if (op->width == 1)
                    arg = mSys.MemRead8(mPC++);
                else {
                    arg = mSys.MemRead16(mPC, Endian::BIG);
                    mPC += 2;
                }
                break;
            case DIRECT:
                TRACEF(" DIR");
                temp8 = mSys.MemRead8(mPC++);
                temp16 = temp8;
                if (op->calcaddr) {
                    arg = temp16;
                } else {
                    if (op->width == 1)
                        arg = mSys.MemRead8(temp16);
                    else
                        arg = mSys.MemRead16(temp16, Endian::BIG); // XXX doesn't handle wraparound
                }
                break;
            case EXTENDED:
                TRACEF(" EXT");
                temp16 = mSys.MemRead16(mPC, Endian::BIG);
                mPC += 2;
                if (op->calcaddr) {
                    arg = temp16;
                } else {
                    if (op->width == 1)
                        arg = mSys.MemRead8(temp16);
                    else
                        arg = mSys.MemRead16(temp16, Endian::BIG);
                }
                break;
            case BRANCH:
                TRACEF(" BRA");
                if (op->width == 1)
                    arg = SignExtend(mSys.MemRead8(mPC++));
                else {
                    arg = SignExtend(mSys.MemRead16(mPC, Endian::BIG));
                    mPC += 2;
                }
                break;
            case INDEXED: {
                temp8 = mSys.MemRead8(mPC++);
                TRACEF(" IDX word %#02x", temp8);

                temp16 = mIX + temp8;
                TRACEF(" addr %#04x", temp16);
                if (op->calcaddr) {
                    arg = temp16;
                } else {
                    if (op->width == 1)
                        arg = mSys.MemRead8(temp16);
                    else
                        arg = mSys.MemRead16(temp16, Endian::BIG);
                }
                break;
            }
            default:
                fprintf(stderr, "unhandled addressing mode\n");
                fflush(stderr);
                assert(0);
        }

        TRACEF(" arg %#02x", arg);

        // decode on our table based opcode
        switch (op->op) {
            case NOP: // nop
                break;
            case ADC:   // adc[ab]
            case ADD: { // add[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a + b;

                if (op->op == ADC && (mCC & CC_C)) {
                    result++;
                }

                SET_HNZVC1(a, b, result);

                PutReg(op->targetreg, result);
                break;
            }
            case ADD_ACCUM: { // aba -- add accum B to A
                uint32_t a = GetReg(regnum::REG_A);
                uint32_t b = GetReg(regnum::REG_B);
                uint32_t result = a + b;

                SET_HNZVC1(a, b, result);

                PutReg(op->targetreg, result);
                break;
            }
            case SBC:   // sbc[ab]
            case SUB: { // sub[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = -arg;
                uint32_t result = a + b;

                if (op->op == SBC && (mCC & CC_C)) {
                    result--;
                }

                // XXX make sure carry is okay
                SET_NZVC1(a, b, result);

                PutReg(op->targetreg, result);
                break;
            }
            case SUB_ACCUM: { // sba -- sub accum B from A
                uint32_t a = GetReg(regnum::REG_A);
                uint32_t b = -GetReg(regnum::REG_B);
                uint32_t result = a + b;

                // XXX make sure carry is okay
                SET_NZVC1(a, b, result);

                PutReg(op->targetreg, result);
                break;
            }
            case CMP: { // cmp[abx]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a - b;

                if (op->width == 1) {
                    // XXX make sure carry is okay
                    SET_NZVC1(a, b, result);
                } else {
                    SET_NZV2(a, b, result);
                }
                break;
            }
            case CMP_ACCUM: { // cba -- compare accumulator A with B
                uint32_t a = GetReg(regnum::REG_A);
                uint32_t b = GetReg(regnum::REG_B);
                uint32_t result = a - b;

                // XXX make sure carry is okay
                SET_NZVC1(a, b, result);
                break;
            }
            case AND: { // and[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a & b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, result);
                break;
            }
            case BIT: { // bit[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a & b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);
                break;
            }
            case OR: { // or[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a | b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, result);
                break;
            }
            case EOR: { // eor[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a ^ b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, result);
                break;
            }
            case ASL: // asl[ab],asl
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                // C is bit 7 before shifted
                mCC = BIT(temp8, 7) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = (temp8 << 1) & 0xff;

                // N and Z are usual result bits
                SET_NZ1(temp8);

                // V is N^C post operation
                {
                    const bool C = mCC & CC_C;
                    const bool N = mCC & CC_N;
                    mCC = (N ^ C) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                }
                goto shared_memwrite;

            case ASR: // asr[ab],asr
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                // C is bit 0 before shifted
                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = BIT(temp8, 7) | ((temp8 & 0xff) >> 1);

                // N and Z are usual result bits
                SET_NZ1(temp8);

                // V is N^C post operation
                {
                    const bool C = mCC & CC_C;
                    const bool N = mCC & CC_N;
                    mCC = (N ^ C) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                }

            case LSR: // lsr[ab],lsr
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                // C is bit 0 before shifted
                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = ((temp8 & 0xff) >> 1);

                // N and Z are usual result bits
                SET_NZ1(temp8);

                // V is N^C post operation
                {
                    const bool C = mCC & CC_C;
                    const bool N = mCC & CC_N;
                    mCC = (N ^ C) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                }

                goto shared_memwrite;

            case DEC: // dec[absx]
                if (op->width == 1) { // dec[ab]
                    if (op->mode == IMPLIED) {
                        temp8 = GetReg(op->targetreg);
                    } else {
                        temp8 = mSys.MemRead8(arg);
                    }
                    temp8--;

                    mCC = (temp8 == 0x7f) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                    SET_NZ1(temp8);
                    goto shared_memwrite;
                } else { // de[sx]
                    // 16bit dec behaves differently
                    temp16 = GetReg(op->targetreg);
                    temp16--;
                    if (op->targetreg == regnum::REG_IX) {
                        // only dex updates Z flag
                        SET_Z2(temp16);
                    }
                    PutReg(op->targetreg, temp16);
                }
                break;

            case INC: // inc[absx]
                if (op->width == 1) { // inc[ab]
                    if (op->mode == IMPLIED) {
                        temp8 = GetReg(op->targetreg);
                    } else {
                        temp8 = mSys.MemRead8(arg);
                    }
                    temp8++;

                    mCC = (temp8 == 0x80) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                    SET_NZ1(temp8);
                    goto shared_memwrite;
                } else { // in[sx]
                    // 16bit inc behaves differently
                    temp16 = GetReg(op->targetreg);
                    temp16++;
                    if (op->targetreg == regnum::REG_IX) {
                        // only inx updates Z flag
                        SET_Z2(temp16);
                    }
                    PutReg(op->targetreg, temp16);
                }
                break;
            case CLR: // clr[ab],clr
                temp8 = 0;
                mCC = CLR_CC_BIT(CC_N);
                mCC = CLR_CC_BIT(CC_V);
                mCC = CLR_CC_BIT(CC_C);
                mCC = SET_CC_BIT(CC_Z);
                goto shared_memwrite;
            case COM: // com[ab],com
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }
                temp8 = ~temp8;

                SET_NZ1(temp8);
                mCC = CLR_CC_BIT(CC_V);
                mCC = SET_CC_BIT(CC_C);
                goto shared_memwrite;
            case NEG: // neg[ab],neg
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = (temp8 == 0x80) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                mCC = (temp8 != 0x00) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = -temp8;

                SET_NZ1(temp8);
                goto shared_memwrite;
            case ROL: { // rol[ab],rol
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                const bool oldc = mCC & CC_C;

                // rotate into the C bit
                mCC = BIT(temp8, 7) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                // rotate in the old C bit
                temp8 = ((temp8 << 1) & 0xff) | (oldc ? 1 : 0);

                SET_NZ1(temp8);

                // V = N xor C
                {
                    const bool C = mCC & CC_C;
                    const bool N = mCC & CC_N;
                    mCC = (N ^ C) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                }

                goto shared_memwrite;
            }

            case ROR: { // ror[ab],ror
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                const bool oldc = mCC & CC_C;

                // C = prior to rotate bit 0 was set
                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                // rotate in the C bit
                temp8 = (oldc ? 0x80 : 0) | ((temp8 & 0xff) >> 1);

                SET_NZ1(temp8);

                // V = N xor C
                {
                    const bool C = mCC & CC_C;
                    const bool N = mCC & CC_N;
                    mCC = (N ^ C) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                }

                goto shared_memwrite;
            }

shared_memwrite:
                if (op->mode == IMPLIED) {
                    PutReg(op->targetreg, temp8);
                } else {
                    assert(op->width == 1);
                    mSys.MemWrite8(arg, temp8);
                }

                break;

            case TST: // tst[ab],tst
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = CLR_CC_BIT(CC_V);
                mCC = CLR_CC_BIT(CC_C);
                SET_NZ1(temp8);
                break;
            case TFR:
                if (op->width == 1) { // tab,tba
                    if (op->targetreg == regnum::REG_A) {
                        temp8 = GetReg(regnum::REG_B);
                    } else { // regnum::REG_B
                        temp8 = GetReg(regnum::REG_A);
                    }
                    PutReg(op->targetreg, temp8);
                    SET_NZ1(temp8);
                    mCC = CLR_CC_BIT(CC_V);
                } else { // tsx,txs
                    if (op->targetreg == regnum::REG_SP) {
                        temp16 = GetReg(regnum::REG_IX);
                        temp16--; // loads SP with IX - 1
                    } else { // regnum::REG_IX
                        temp16 = GetReg(regnum::REG_SP);
                        temp16++; // loads IX with SP + 1
                    }
                    PutReg(op->targetreg, temp16);
                }
                break;
            case TFR_CC: // tap,tpa
                if (op->targetreg == regnum::REG_A) {
                    temp8 = GetReg(regnum::REG_CC);
                    temp8 |= 0b11000000;
                } else { // target == regnum::REG_CC
                    temp8 = GetReg(regnum::REG_A);
                    temp8 &= 0b00111111;
                }
                PutReg(op->targetreg, temp8);
                break;
            case PUSH: { // psha,pshb
                temp8 = GetReg(op->targetreg);
                TRACEF(" push byte %#02x to sp %#0x", temp8, mSP);
                PUSH8(temp8);
                break;
            }
            case PULL: { // pula,pulb
                TRACEF(" pull byte from sp %#02x", mSP + 1);
                PULL8(temp8);
                PutReg(op->targetreg, temp8);
                break;
            }
            case LD: // ld[absx]
                if (op->width == 1) {
                    SET_NZ1(arg);
                } else {
                    SET_NZ2(arg);
                }
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, arg);
                break;
            case ST: // st[absx]
                if (op->width == 1) {
                    temp8 = GetReg(op->targetreg);
                    mSys.MemWrite8(arg, temp8);
                    SET_NZ1(temp8);
                } else {
                    temp16 = GetReg(op->targetreg);
                    mSys.MemWrite16(arg, temp16, Endian::BIG);
                    SET_NZ2(temp16);
                }
                mCC = CLR_CC_BIT(CC_V);
                break;
            case BRA: { // branch
                TRACEF(" arg %d", arg);

                bool takebranch = TestBranchCond(op->cond);

                if (takebranch) {
                    if (arg == -2) {
                        fprintf(stderr, "infinite loop detected, aborting cpu\n");
                        fflush(stderr);
                        done = true;
                    }
                    mPC += arg;
                    mPC &= 0xffff;
                    TRACEF(" target %#04x", mPC);
                }
                break;
            }
            case JMP: { // jmp
                TRACEF(" arg %#04x", arg);

                if (arg == mPC) {
                    fprintf(stderr, "infinite loop detected, aborting cpu\n");
                    fflush(stderr);
                    done = true;
                }

                mPC = arg;
                break;
            }
            case JSR: { // jsr
                TRACEF(" arg %#04x", arg);

                PUSH16(mPC);

                mPC = arg;
                break;
            }
            case BSR: { // bsr
                TRACEF(" arg %d", arg);

                PUSH16(mPC);

                mPC += arg;
                TRACEF(" target %#04x", mPC);
                break;
            }
            case RTS: { // rts
                PULL16(temp16);
                TRACEF(" from stack %#04x", temp16);

                mPC = temp16;
                break;
            }
            case SEcc: // sec,sev,sei
                mCC = SET_CC_BIT(op->cc_flag);
                break;
            case CLcc: // clc,clv,cli
                mCC = CLR_CC_BIT(op->cc_flag);
                break;

            case BADOP:
badop:
                fflush(stdout);
                fprintf(stderr, "unhandled opcode %#02x at %#04x\n", opcode, mPC - 1);
                fflush(stderr);
                done = true;
        }

        TRACEF("\n");

        if (TRACE) {
            Dump();
            fflush(stdout);
        }

        // see if the system has requested a shutdown
        if (mSys.isShutdown()) {
            printf("cpu: exiting due to shutdown\n");
            done = true;
        }
    }

    printf("cpu: exiting\n");

    return 0;
}

void Cpu6800::Dump() {
    printf("A 0x%02x B 0x%02x X 0x%04x S 0x%04x CC 0x%02x (%c%c%c%c%c) PC 0x%04x\n",
             mA, mB, mIX, mSP, mCC,
             (mCC & CC_H) ? 'h' : ' ',
             (mCC & CC_N) ? 'n' : ' ',
             (mCC & CC_Z) ? 'z' : ' ',
             (mCC & CC_V) ? 'v' : ' ',
             (mCC & CC_C) ? 'c' : ' ',
             mPC);
}

uint16_t Cpu6800::GetReg(regnum r) {
    switch (r) {
        case regnum::REG_A:
            return mA;
        case regnum::REG_B:
            return mB;
        case regnum::REG_IX:
            return mIX;
        case regnum::REG_SP:
            return mSP;
        case regnum::REG_PC:
            return mPC;
        case regnum::REG_CC:
            return mCC;
    }
}

uint16_t Cpu6800::PutReg(regnum r, uint16_t val) {
    uint16_t old = GetReg(r);;
    switch (r) {
        case regnum::REG_A:
            mA = val;
            break;
        case regnum::REG_B:
            mB = val;
            break;
        case regnum::REG_IX:
            mIX = val;
            break;
        case regnum::REG_SP:
            mSP = val;
            break;
        case regnum::REG_PC:
            mPC = val;
            break;
        case regnum::REG_CC:
            mCC = val;
            break;
    }
    return old;
}


