// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2013 Travis Geiselbrecht
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
#include "cpu6809.h"

#include <cstdio>
#include <cassert>
#include <iostream>

#include "system/system.h"
#include "bits.h"

#define TRACE 0

#define TRACEF(str, x...) do { if (TRACE) printf(str, ## x); } while (0)

using namespace std;

/* exceptions */
#define EXC_RESET 0x1
#define EXC_NMI   0x2
#define EXC_SWI   0x4
#define EXC_IRQ   0x8
#define EXC_FIRQ  0x10
#define EXC_SWI2  0x20
#define EXC_SWI3  0x40

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
#define CC_H (0x20)
#define CC_F (0x40)
#define CC_E (0x80)

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
    ADC,
    SUB,
    CMP,
    AND,
    BIT,
    EOR,
    OR,
    ABX,
    CLR,
    COM,
    NEG,
    DEC,
    INC,
    TST,
    LEA,
    ASL,
    ASR,
    LSR,
    ROL,
    ROR,
    TFR,
    PUSH,
    PULL,
    BRA,
    BSR,
    JMP,
    JSR,
    RTS,
    LD,
    ST,
};

struct opdecode {
    const char *name;
    addrMode mode;
    int width; // 1 or 2
    enum op op;
    regnum targetreg;
    union {
        unsigned int cond;
        bool calcaddr;
    };
};

// opcode table, 0x10 extended opcodes are 0x100 -> 0x1ff, 0x11 is 0x200 -> 0x2ff
static const opdecode ops[256 * 3] = {
// alu ops
[0x8b] = { "adda", IMMEDIATE, 1, ADD, REG_A, { 0 } },
[0xcb] = { "addb", IMMEDIATE, 1, ADD, REG_B, { 0 } },
[0xc3] = { "addd", IMMEDIATE, 2, ADD, REG_D, { 0 } },
[0x9b] = { "adda", DIRECT, 1, ADD, REG_A, { 0 } },
[0xdb] = { "addb", DIRECT, 1, ADD, REG_B, { 0 } },
[0xd3] = { "addd", DIRECT, 2, ADD, REG_D, { 0 } },
[0xab] = { "adda", INDEXED, 1, ADD, REG_A, { 0 } },
[0xeb] = { "addb", INDEXED, 1, ADD, REG_B, { 0 } },
[0xe3] = { "addd", INDEXED, 2, ADD, REG_D, { 0 } },
[0xbb] = { "adda", EXTENDED, 1, ADD, REG_A, { 0 } },
[0xfb] = { "addb", EXTENDED, 1, ADD, REG_B, { 0 } },
[0xf3] = { "addd", EXTENDED, 2, ADD, REG_D, { 0 } },

[0x89] = { "adca", IMMEDIATE, 1, ADC, REG_A, { 0 } },
[0xc9] = { "adcb", IMMEDIATE, 1, ADC, REG_B, { 0 } },
[0x99] = { "adca", DIRECT, 1, ADC, REG_A, { 0 } },
[0xd9] = { "adcb", DIRECT, 1, ADC, REG_B, { 0 } },
[0xa9] = { "adca", INDEXED, 1, ADC, REG_A, { 0 } },
[0xe9] = { "adcb", INDEXED, 1, ADC, REG_B, { 0 } },
[0xb9] = { "adca", EXTENDED, 1, ADC, REG_A, { 0 } },
[0xf9] = { "adcb", EXTENDED, 1, ADC, REG_B, { 0 } },

[0x80] = { "suba", IMMEDIATE, 1, SUB, REG_A, { 0 } },
[0xc0] = { "subb", IMMEDIATE, 1, SUB, REG_B, { 0 } },
[0x83] = { "subd", IMMEDIATE, 2, SUB, REG_D, { 0 } },
[0x90] = { "suba", DIRECT, 1, SUB, REG_A, { 0 } },
[0xd0] = { "subb", DIRECT, 1, SUB, REG_B, { 0 } },
[0x93] = { "subd", DIRECT, 2, SUB, REG_D, { 0 } },
[0xa0] = { "suba", INDEXED, 1, SUB, REG_A, { 0 } },
[0xe0] = { "subb", INDEXED, 1, SUB, REG_B, { 0 } },
[0xa3] = { "subd", INDEXED, 2, SUB, REG_D, { 0 } },
[0xb0] = { "suba", EXTENDED, 1, SUB, REG_A, { 0 } },
[0xf0] = { "subb", EXTENDED, 1, SUB, REG_B, { 0 } },
[0xb3] = { "subd", EXTENDED, 2, SUB, REG_D, { 0 } },

[0x81]  = { "cmpa", IMMEDIATE, 1, CMP, REG_A, { 0 } },
[0xc1]  = { "cmpb", IMMEDIATE, 1, CMP, REG_B, { 0 } },
[0x183] = { "cmpd", IMMEDIATE, 2, CMP, REG_D, { 0 } },
[0x28c] = { "cmps", IMMEDIATE, 2, CMP, REG_S, { 0 } },
[0x283] = { "cmpu", IMMEDIATE, 2, CMP, REG_U, { 0 } }  ,
[0x8c]  = { "cmpx", IMMEDIATE, 2, CMP, REG_X, { 0 } },
[0x18c] = { "cmpy", IMMEDIATE, 2, CMP, REG_Y, { 0 } },

[0x91]  = { "cmpa", DIRECT, 1, CMP, REG_A, { 0 } },
[0xd1]  = { "cmpb", DIRECT, 1, CMP, REG_B, { 0 } },
[0x193] = { "cmpd", DIRECT, 2, CMP, REG_D, { 0 } },
[0x29c] = { "cmps", DIRECT, 2, CMP, REG_S, { 0 } },
[0x293] = { "cmpu", DIRECT, 2, CMP, REG_U, { 0 } },
[0x9c]  = { "cmpx", DIRECT, 2, CMP, REG_X, { 0 } },
[0x19c] = { "cmpy", DIRECT, 2, CMP, REG_Y, { 0 } },

[0xa1]  = { "cmpa", INDEXED, 1, CMP, REG_A, { 0 } },
[0xe1]  = { "cmpb", INDEXED, 1, CMP, REG_B, { 0 } },
[0x1a3] = { "cmpd", INDEXED, 2, CMP, REG_D, { 0 } },
[0x2ac] = { "cmps", INDEXED, 2, CMP, REG_S, { 0 } },
[0x2a3] = { "cmpu", INDEXED, 2, CMP, REG_U, { 0 } },
[0xac]  = { "cmpx", INDEXED, 2, CMP, REG_X, { 0 } },
[0x1ac] = { "cmpy", INDEXED, 2, CMP, REG_Y, { 0 } },

[0xb1]  = { "cmpa", EXTENDED, 1, CMP, REG_A, { 0 } },
[0xf1]  = { "cmpb", EXTENDED, 1, CMP, REG_B, { 0 } },
[0x1b3] = { "cmpd", EXTENDED, 2, CMP, REG_D, { 0 } },
[0x2bc] = { "cmps", EXTENDED, 2, CMP, REG_S, { 0 } },
[0x2b3] = { "cmpu", EXTENDED, 2, CMP, REG_U, { 0 } },
[0xbc]  = { "cmpx", EXTENDED, 2, CMP, REG_X, { 0 } },
[0x1bc] = { "cmpy", EXTENDED, 2, CMP, REG_Y, { 0 } },

[0x84] = { "anda", IMMEDIATE, 1, AND, REG_A, { 0 } },
[0xc4] = { "andb", IMMEDIATE, 1, AND, REG_B, { 0 } },
[0x1c] = { "andcc", IMMEDIATE, 1, AND, REG_CC, { 0 } },
[0x94] = { "anda", DIRECT, 1, AND, REG_A, { 0 } },
[0xd4] = { "andb", DIRECT, 1, AND, REG_B, { 0 } },
[0xa4] = { "anda", INDEXED, 1, AND, REG_A, { 0 } },
[0xe4] = { "andb", INDEXED, 1, AND, REG_B, { 0 } },
[0xb4] = { "anda", EXTENDED, 1, AND, REG_A, { 0 } },
[0xf4] = { "andb", EXTENDED, 1, AND, REG_B, { 0 } },

[0x85] = { "bita", IMMEDIATE, 1, BIT, REG_A, { 0 } },
[0xc5] = { "bitb", IMMEDIATE, 1, BIT, REG_B, { 0 } },
[0x95] = { "bita", DIRECT, 1, BIT, REG_A, { 0 } },
[0xd5] = { "bitb", DIRECT, 1, BIT, REG_B, { 0 } },
[0xa5] = { "bita", INDEXED, 1, BIT, REG_A, { 0 } },
[0xe5] = { "bitb", INDEXED, 1, BIT, REG_B, { 0 } },
[0xb5] = { "bita", EXTENDED, 1, BIT, REG_A, { 0 } },
[0xf5] = { "bitb", EXTENDED, 1, BIT, REG_B, { 0 } },

[0x88] = { "eora", IMMEDIATE, 1, EOR, REG_A, { 0 } },
[0xc8] = { "eorb", IMMEDIATE, 1, EOR, REG_B, { 0 } },
[0x98] = { "eora", DIRECT, 1, EOR, REG_A, { 0 } },
[0xd8] = { "eorb", DIRECT, 1, EOR, REG_B, { 0 } },
[0xa8] = { "eora", INDEXED, 1, EOR, REG_A, { 0 } },
[0xe8] = { "eorb", INDEXED, 1, EOR, REG_B, { 0 } },
[0xb8] = { "eora", EXTENDED, 1, EOR, REG_A, { 0 } },
[0xf8] = { "eorb", EXTENDED, 1, EOR, REG_B, { 0 } },

[0x8a] = { "ora", IMMEDIATE, 1, OR, REG_A, { 0 } },
[0xca] = { "orb", IMMEDIATE, 1, OR, REG_B, { 0 } },
[0x1a] = { "orcc", IMMEDIATE, 1, OR, REG_CC, { 0 } },
[0x9a] = { "ora", DIRECT, 1, OR, REG_A, { 0 } },
[0xda] = { "orb", DIRECT, 1, OR, REG_B, { 0 } },
[0xaa] = { "ora", INDEXED, 1, OR, REG_A, { 0 } },
[0xea] = { "orb", INDEXED, 1, OR, REG_B, { 0 } },
[0xba] = { "ora", EXTENDED, 1, OR, REG_A, { 0 } },
[0xfa] = { "orb", EXTENDED, 1, OR, REG_B, { 0 } },

// misc
[0x3a] = { "abx",  IMPLIED, 2, ABX, REG_X, { 0 } },
[0x1f] = { "tfr",  IMPLIED, 1, TFR, REG_A, { 0 } },

[0x4f] = { "clra", IMPLIED,  1, CLR, REG_A, { 0 } },
[0x5f] = { "clrb", IMPLIED,  1, CLR, REG_B, { 0 } },
[0x0f] = { "clr",  DIRECT,   1, CLR, REG_A, { .calcaddr = true } },
[0x6f] = { "clr",  INDEXED,  1, CLR, REG_A, { .calcaddr = true } },
[0x7f] = { "clr",  EXTENDED, 1, CLR, REG_A, { .calcaddr = true } },

[0x43] = { "coma", IMPLIED,  1, COM, REG_A, { 0 } },
[0x53] = { "comb", IMPLIED,  1, COM, REG_B, { 0 } },
[0x03] = { "com",  DIRECT,   1, COM, REG_A, { .calcaddr = true } },
[0x63] = { "com",  INDEXED,  1, COM, REG_A, { .calcaddr = true } },
[0x73] = { "com",  EXTENDED, 1, COM, REG_A, { .calcaddr = true } },

[0x40] = { "nega", IMPLIED,  1, NEG, REG_A, { 0 } },
[0x50] = { "negb", IMPLIED,  1, NEG, REG_B, { 0 } },
[0x00] = { "neg",  DIRECT,   1, NEG, REG_A, { .calcaddr = true } },
[0x60] = { "neg",  INDEXED,  1, NEG, REG_A, { .calcaddr = true } },
[0x70] = { "neg",  EXTENDED, 1, NEG, REG_A, { .calcaddr = true } },

[0x4a] = { "deca", IMPLIED,  1, DEC, REG_A, { 0 } },
[0x5a] = { "decb", IMPLIED,  1, DEC, REG_B, { 0 } },
[0x0a] = { "dec",  DIRECT,   1, DEC, REG_A, { .calcaddr = true } },
[0x6a] = { "dec",  INDEXED,  1, DEC, REG_A, { .calcaddr = true } },
[0x7a] = { "dec",  EXTENDED, 1, DEC, REG_A, { .calcaddr = true } },

[0x4c] = { "inca", IMPLIED,  1, INC, REG_A, { 0 } },
[0x5c] = { "incb", IMPLIED,  1, INC, REG_B, { 0 } },
[0x0c] = { "inc",  DIRECT,   1, INC, REG_A, { .calcaddr = true } },
[0x6c] = { "inc",  INDEXED,  1, INC, REG_A, { .calcaddr = true } },
[0x7c] = { "inc",  EXTENDED, 1, INC, REG_A, { .calcaddr = true } },

[0x48] = { "asla", IMPLIED,  1, ASL, REG_A, { 0 } },
[0x58] = { "aslb", IMPLIED,  1, ASL, REG_B, { 0 } },
[0x08] = { "asl",  DIRECT,   1, ASL, REG_A, { .calcaddr = true } },
[0x68] = { "asl",  INDEXED,  1, ASL, REG_A, { .calcaddr = true } },
[0x78] = { "asl",  EXTENDED, 1, ASL, REG_A, { .calcaddr = true } },

[0x47] = { "asra", IMPLIED,  1, ASR, REG_A, { 0 } },
[0x57] = { "asrb", IMPLIED,  1, ASR, REG_B, { 0 } },
[0x07] = { "asr",  DIRECT,   1, ASR, REG_A, { .calcaddr = true } },
[0x67] = { "asr",  INDEXED,  1, ASR, REG_A, { .calcaddr = true } },
[0x77] = { "asr",  EXTENDED, 1, ASR, REG_A, { .calcaddr = true } },

[0x44] = { "lsra", IMPLIED,  1, LSR, REG_A, { 0 } },
[0x54] = { "lsrb", IMPLIED,  1, LSR, REG_B, { 0 } },
[0x04] = { "lsr",  DIRECT,   1, LSR, REG_A, { .calcaddr = true } },
[0x64] = { "lsr",  INDEXED,  1, LSR, REG_A, { .calcaddr = true } },
[0x74] = { "lsr",  EXTENDED, 1, LSR, REG_A, { .calcaddr = true } },

[0x49] = { "rola", IMPLIED,  1, ROL, REG_A, { 0 } },
[0x59] = { "rolb", IMPLIED,  1, ROL, REG_B, { 0 } },
[0x09] = { "rol",  DIRECT,   1, ROL, REG_A, { .calcaddr = true } },
[0x69] = { "rol",  INDEXED,  1, ROL, REG_A, { .calcaddr = true } },
[0x79] = { "rol",  EXTENDED, 1, ROL, REG_A, { .calcaddr = true } },

[0x46] = { "rora", IMPLIED,  1, ROR, REG_A, { 0 } },
[0x56] = { "rorb", IMPLIED,  1, ROR, REG_B, { 0 } },
[0x06] = { "ror",  DIRECT,   1, ROR, REG_A, { .calcaddr = true } },
[0x66] = { "ror",  INDEXED,  1, ROR, REG_A, { .calcaddr = true } },
[0x76] = { "ror",  EXTENDED, 1, ROR, REG_A, { .calcaddr = true } },

[0x4d] = { "tsta", IMPLIED,  1, TST, REG_A, { 0 } },
[0x5d] = { "tstb", IMPLIED,  1, TST, REG_B, { 0 } },
[0x0d] = { "tst",  DIRECT,   1, TST, REG_A, { .calcaddr = true } },
[0x6d] = { "tst",  INDEXED,  1, TST, REG_A, { .calcaddr = true } },
[0x7d] = { "tst",  EXTENDED, 1, TST, REG_A, { .calcaddr = true } },

[0x32] = { "leas", INDEXED,  2, LEA, REG_S, { .calcaddr = true } },
[0x33] = { "leau", INDEXED,  2, LEA, REG_U, { .calcaddr = true } },
[0x30] = { "leax", INDEXED,  2, LEA, REG_X, { .calcaddr = true } },
[0x31] = { "leay", INDEXED,  2, LEA, REG_Y, { .calcaddr = true } },

// push/pull
[0x34] = { "pshs", IMMEDIATE, 1, PUSH, REG_S, { 0 } },
[0x36] = { "pshu", IMMEDIATE, 1, PUSH, REG_U, { 0 } },

[0x35] = { "puls", IMMEDIATE, 1, PULL, REG_S, { 0 } },
[0x37] = { "pulu", IMMEDIATE, 1, PULL, REG_U, { 0 } },

// loads
[0x86] = { "lda",  IMMEDIATE, 1, LD, REG_A, { 0 } },
[0xc6] = { "ldb",  IMMEDIATE, 1, LD, REG_B, { 0 } },
[0xcc] = { "ldd",  IMMEDIATE, 2, LD, REG_D, { 0 } },
[0x1ce] = { "lds",  IMMEDIATE, 2, LD, REG_S, { 0 } },
[0xce] = { "ldu",  IMMEDIATE, 2, LD, REG_U, { 0 } },
[0x8e] = { "ldx",  IMMEDIATE, 2, LD, REG_X, { 0 } },
[0x18e] = { "ldy",  IMMEDIATE, 2, LD, REG_Y, { 0 } },

[0x96] = { "lda",  DIRECT, 1, LD, REG_A, { 0 } },
[0xd6] = { "ldb",  DIRECT, 1, LD, REG_B, { 0 } },
[0xdc] = { "ldd",  DIRECT, 2, LD, REG_D, { 0 } },
[0x1de] = { "lds",  DIRECT, 2, LD, REG_S, { 0 } },
[0xde] = { "ldu",  DIRECT, 2, LD, REG_U, { 0 } },
[0x9e] = { "ldx",  DIRECT, 2, LD, REG_X, { 0 } },
[0x19e] = { "ldy",  DIRECT, 2, LD, REG_Y, { 0 } },

[0xa6] = { "lda",  INDEXED, 1, LD, REG_A, { 0 } },
[0xe6] = { "ldb",  INDEXED, 1, LD, REG_B, { 0 } },
[0xec] = { "ldd",  INDEXED, 2, LD, REG_D, { 0 } },
[0x1ee] = { "lds",  INDEXED, 2, LD, REG_S, { 0 } },
[0xee] = { "ldu",  INDEXED, 2, LD, REG_U, { 0 } },
[0xae] = { "ldx",  INDEXED, 2, LD, REG_X, { 0 } },
[0x1ae] = { "ldy",  INDEXED, 2, LD, REG_Y, { 0 } },

[0xb6] = { "lda",  EXTENDED, 1, LD, REG_A, { 0 } },
[0xf6] = { "ldb",  EXTENDED, 1, LD, REG_B, { 0 } },
[0xfc] = { "ldd",  EXTENDED, 2, LD, REG_D, { 0 } },
[0x1fe] = { "lds",  EXTENDED, 2, LD, REG_S, { 0 } },
[0xfe] = { "ldu",  EXTENDED, 2, LD, REG_U, { 0 } },
[0xbe] = { "ldx",  EXTENDED, 2, LD, REG_X, { 0 } },
[0x1be] = { "ldy",  EXTENDED, 2, LD, REG_Y, { 0 } },

// stores
[0x97] = { "sta",  DIRECT, 1, ST, REG_A, { .calcaddr = true } },
[0xd7] = { "stb",  DIRECT, 1, ST, REG_B, { .calcaddr = true } },
[0xdd] = { "std",  DIRECT, 2, ST, REG_D, { .calcaddr = true } },
[0x1df] = { "sts",  DIRECT, 2, ST, REG_S, { .calcaddr = true } },
[0xdf] = { "stu",  DIRECT, 2, ST, REG_U, { .calcaddr = true } },
[0x9f] = { "stx",  DIRECT, 2, ST, REG_X, { .calcaddr = true } },
[0x19f] = { "sty",  DIRECT, 2, ST, REG_Y, { .calcaddr = true } },

[0xb7] = { "sta",  EXTENDED, 1, ST, REG_A, { .calcaddr = true } },
[0xf7] = { "stb",  EXTENDED, 1, ST, REG_B, { .calcaddr = true } },
[0xfd] = { "std",  EXTENDED, 2, ST, REG_D, { .calcaddr = true } },
[0x1ff] = { "sts",  EXTENDED, 2, ST, REG_S, { .calcaddr = true } },
[0xff] = { "stu",  EXTENDED, 2, ST, REG_U, { .calcaddr = true } },
[0xbf] = { "stx",  EXTENDED, 2, ST, REG_X, { .calcaddr = true } },
[0x1bf] = { "sty",  EXTENDED, 2, ST, REG_Y, { .calcaddr = true } },

[0xa7] = { "sta",  INDEXED, 1, ST, REG_A, { .calcaddr = true } },
[0xe7] = { "stb",  INDEXED, 1, ST, REG_B, { .calcaddr = true } },
[0xed] = { "std",  INDEXED, 2, ST, REG_D, { .calcaddr = true } },
[0x1ef] = { "sts",  INDEXED, 2, ST, REG_S, { .calcaddr = true } },
[0xef] = { "stu",  INDEXED, 2, ST, REG_U, { .calcaddr = true } },
[0xaf] = { "stx",  INDEXED, 2, ST, REG_X, { .calcaddr = true } },
[0x1af] = { "sty",  INDEXED, 2, ST, REG_Y, { .calcaddr = true } },

// branches
[0x20] = { "bra",  BRANCH, 1, BRA, REG_A, { .cond = COND_A } },
[0x21] = { "brn",  BRANCH, 1, BRA, REG_A, { .cond = COND_N } },
[0x22] = { "bhi",  BRANCH, 1, BRA, REG_A, { .cond = COND_HI } },
[0x23] = { "bls",  BRANCH, 1, BRA, REG_A, { .cond = COND_LS } },
[0x24] = { "bcc",  BRANCH, 1, BRA, REG_A, { .cond = COND_CC } },
[0x25] = { "bcs",  BRANCH, 1, BRA, REG_A, { .cond = COND_CS } },
[0x26] = { "bne",  BRANCH, 1, BRA, REG_A, { .cond = COND_NE } },
[0x27] = { "beq",  BRANCH, 1, BRA, REG_A, { .cond = COND_EQ } },
[0x28] = { "bvc",  BRANCH, 1, BRA, REG_A, { .cond = COND_VC } },
[0x29] = { "bvs",  BRANCH, 1, BRA, REG_A, { .cond = COND_VS } },
[0x2a] = { "bpl",  BRANCH, 1, BRA, REG_A, { .cond = COND_PL } },
[0x2b] = { "bmi",  BRANCH, 1, BRA, REG_A, { .cond = COND_MI } },
[0x2c] = { "bge",  BRANCH, 1, BRA, REG_A, { .cond = COND_GE } },
[0x2d] = { "blt",  BRANCH, 1, BRA, REG_A, { .cond = COND_LT } },
[0x2e] = { "bgt",  BRANCH, 1, BRA, REG_A, { .cond = COND_GT } },
[0x2f] = { "ble",  BRANCH, 1, BRA, REG_A, { .cond = COND_LE } },
[0x8d] = { "bsr",  BRANCH, 1, BSR, REG_A, { .cond = COND_A } },

[0x16] =  { "lbra",  BRANCH, 2, BRA, REG_A, { .cond = COND_A } },
[0x121] = { "lbrn",  BRANCH, 2, BRA, REG_A, { .cond = COND_N } },
[0x122] = { "lbhi",  BRANCH, 2, BRA, REG_A, { .cond = COND_HI } },
[0x123] = { "lbls",  BRANCH, 2, BRA, REG_A, { .cond = COND_LS } },
[0x124] = { "lbcc",  BRANCH, 2, BRA, REG_A, { .cond = COND_CC } },
[0x125] = { "lbcs",  BRANCH, 2, BRA, REG_A, { .cond = COND_CS } },
[0x126] = { "lbne",  BRANCH, 2, BRA, REG_A, { .cond = COND_NE } },
[0x127] = { "lbeq",  BRANCH, 2, BRA, REG_A, { .cond = COND_EQ } },
[0x128] = { "lbvc",  BRANCH, 2, BRA, REG_A, { .cond = COND_VC } },
[0x129] = { "lbvs",  BRANCH, 2, BRA, REG_A, { .cond = COND_VS } },
[0x12a] = { "lbpl",  BRANCH, 2, BRA, REG_A, { .cond = COND_PL } },
[0x12b] = { "lbmi",  BRANCH, 2, BRA, REG_A, { .cond = COND_MI } },
[0x12c] = { "lbge",  BRANCH, 2, BRA, REG_A, { .cond = COND_GE } },
[0x12d] = { "lblt",  BRANCH, 2, BRA, REG_A, { .cond = COND_LT } },
[0x12e] = { "lbgt",  BRANCH, 2, BRA, REG_A, { .cond = COND_GT } },
[0x12f] = { "lble",  BRANCH, 2, BRA, REG_A, { .cond = COND_LE } },
[0x17] =  { "lbsr",  BRANCH, 2, BSR, REG_A, { .cond = COND_A } },

[0x0e] = { "jmp",  DIRECT,   1, JMP, REG_A, { .calcaddr = true } },
[0x6e] = { "jmp",  INDEXED,  1, JMP, REG_A, { .calcaddr = true } },
[0x7e] = { "jmp",  EXTENDED, 1, JMP, REG_A, { .calcaddr = true } },

[0x9d] = { "jsr",  DIRECT,   1, JSR, REG_A, { .calcaddr = true } },
[0xad] = { "jsr",  INDEXED,  1, JSR, REG_A, { .calcaddr = true } },
[0xbd] = { "jsr",  EXTENDED, 1, JSR, REG_A, { .calcaddr = true } },

[0x39] = { "rts",  IMPLIED,  1, RTS, REG_A, { 0 } },
};

Cpu6809::Cpu6809(System &mSys)
:   Cpu(mSys)
{
    Reset();
}

Cpu6809::~Cpu6809()
{
}

void Cpu6809::Reset()
{
    // put all the registers into their default values
    mA = mB = mX = mY = 0;
    mU = mS = 0;
    mDP = 0;
    mCC = 0;

    mPC = 0;

    // put the cpu in reset
    mException = EXC_RESET;
}

static inline int RegWidth(regnum r)
{
    switch (r) {
        default:
        case REG_A:
        case REG_B:
            return 1;
        case REG_X:
        case REG_Y:
        case REG_U:
        case REG_S:
        case REG_D:
        case REG_PC:
            return 2;
    }
}

bool Cpu6809::TestBranchCond(unsigned int cond)
{
    bool C = !!(mCC & CC_C);
    bool N = !!(mCC & CC_N);
    bool Z = !!(mCC & CC_Z);
    bool V = !!(mCC & CC_V);

    switch (cond) {
        default:
        case COND_A:  return true;
        case COND_N:  return false;
        case COND_HI: return !(C || Z);
        case COND_LS: return C || Z;
        case COND_CC: return !C;
        case COND_CS: return C;
        case COND_NE: return !Z;
        case COND_EQ: return Z;
        case COND_VC: return !V;
        case COND_VS: return V;
        case COND_PL: return !N;
        case COND_MI: return N;

        case COND_GE: return !(N ^ V);
        case COND_LT: return N ^ V;
        case COND_GT: return !((N ^ V) || Z);
        case COND_LE: return (N ^ V) || Z;
    }
}

#define SET_CC_BIT(bit) (mCC | (bit))
#define CLR_CC_BIT(bit) (mCC & ~(bit))

#define SET_Z(result) do { mCC = ((result) == 0) ? SET_CC_BIT(CC_Z) : CLR_CC_BIT(CC_Z); } while (0)
#define SET_N1(result) do { mCC = BIT(result, 7) ? SET_CC_BIT(CC_N) : CLR_CC_BIT(CC_N); } while (0)
#define SET_N2(result) do { mCC = BIT(result, 15) ? SET_CC_BIT(CC_N) : CLR_CC_BIT(CC_N); } while (0)
#define SET_C1(result) do { mCC = BIT(result, 8) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C); } while (0)
#define SET_C2(result) do { mCC = BIT(result, 16) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C); } while (0)
#define SET_V1(a, b, result) do { mCC = BIT((a)^(b)^(result)^((result)>>1), 7) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V); } while (0)
#define SET_V2(a, b, result) do { mCC = BIT((a)^(b)^(result)^((result)>>1), 15) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V); } while (0)
#define SET_H(a, b, result) do { mCC = BIT((a)^(b)^(result), 4) ? SET_CC_BIT(CC_H) : CLR_CC_BIT(CC_H); } while (0)

#define SET_NZ1(result) do { SET_N1(result); SET_Z(result); } while (0)
#define SET_NZ2(result) do { SET_N2(result); SET_Z(result); } while (0)

#define SET_HNVC1(a, b, result) do { SET_H(a, b, result); SET_N1(result); SET_V1(a, b, result); SET_C1(result); } while (0)
#define SET_NVC2(a, b, result) do { SET_N2(result); SET_V2(a, b, result); SET_C2(result); } while (0)

#define PUSH16(stack, val) do { \
    uint16_t __sp = GetReg(stack); \
    mSys.MemWrite8(--__sp, (val) & 0xff); \
    mSys.MemWrite8(--__sp, ((val) >> 8) & 0xff); \
    PutReg(stack, __sp); \
} while (0)
#define PUSH8(stack, val) do { \
    uint16_t __sp = GetReg(stack); \
    mSys.MemWrite8(--__sp, val); \
    PutReg(stack, __sp); \
} while (0)
#define PULL16(stack, val) do { \
    uint16_t __sp = GetReg(stack); \
    uint16_t __temp = mSys.MemRead8(__sp++) << 8;\
    __temp |= mSys.MemRead8(__sp++); \
    PutReg(stack, __sp); \
    val = __temp; \
} while (0)
#define PULL8(stack, val) do { \
    uint16_t __sp = GetReg(stack); \
    val = mSys.MemRead8(__sp++);\
    PutReg(stack, __sp); \
} while (0)

int Cpu6809::Run()
{

    bool done = false;
    while (!done) {
        uint8_t opcode;

        if (mException) {
            if (mException & EXC_RESET) {
                // reset, branch to the reset vector
                mPC = mSys.MemRead16(0xfffe);
                mException = 0; // clear the rest of the pending irqs
            }
            assert(!mException);
        }

        // fetch the first byte of the opcode
        opcode = mSys.MemRead8(mPC++);

        TRACEF("opcode");

        const opdecode *op = &ops[opcode];

        // see if it's an extended opcode
        if (opcode == 0x10) {
            opcode = mSys.MemRead8(mPC++);
            op = &ops[opcode + 0x100];
            TRACEF(" %#02x", 0x10);
        } else if (opcode == 0x11) {
            opcode = mSys.MemRead8(mPC++);
            op = &ops[opcode + 0x200];
            TRACEF(" %#02x", 0x11);
        } else {
            op = &ops[opcode];
        }

        uint8_t temp8;
        uint16_t temp16;
        int arg = 0;

        TRACEF(" %#02x %s", opcode, op->name);

        if (op->op == BADOP) {
            TRACEF("\n");
            fprintf(stdout, "unhandled opcode %#02x at %#04x\n", opcode, mPC - 1);
            return -1;
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
                    arg = mSys.MemRead16(mPC);
                    mPC += 2;
                }
                break;
            case DIRECT:
                TRACEF(" DIR");
                temp8 = mSys.MemRead8(mPC++);
                temp16 = (mDP << 8) | temp8;
                if (op->calcaddr) {
                    arg = temp16;
                } else {
                    if (op->width == 1)
                        arg = mSys.MemRead8(temp16);
                    else
                        arg = mSys.MemRead16(temp16); // XXX doesn't handle wraparound
                }
                break;
            case EXTENDED:
                TRACEF(" EXT");
                temp16 = mSys.MemRead16(mPC);
                mPC += 2;
                if (op->calcaddr) {
                    arg = temp16;
                } else {
                    if (op->width == 1)
                        arg = mSys.MemRead8(temp16);
                    else
                        arg = mSys.MemRead16(temp16);
                }
                break;
            case BRANCH:
                TRACEF(" BRA");
                if (op->width == 1)
                    arg = SignExtend(mSys.MemRead8(mPC++));
                else {
                    arg = SignExtend(mSys.MemRead16(mPC));
                    mPC += 2;
                }
                break;
            case INDEXED: {
                temp8 = mSys.MemRead8(mPC++);
                TRACEF(" IDX word %#02x", temp8);

                // parse the index word
                int off = 0;
                int prepostinc = 0;
                uint16_t *reg = NULL;
                uint16_t zero = 0;
                bool indirect = !!BIT(temp8, 4);
                if (BIT(temp8, 7) == 0) {
                    // 5 bit offset
                    off = SignExtend(BITS(temp8, 4, 0), 4);
                    indirect = false; // no indirect for this moe
                } else {
                    switch (BITS(temp8, 3, 0)) {
                        case 0x0: // ,R+
                            prepostinc = 1;
                            indirect = false; // no indirect for this moe
                            break;
                        case 0x1: // ,R++
                            prepostinc = 2;
                            break;
                        case 0x2: // ,-R
                            prepostinc = -1;
                            indirect = false; // no indirect for this mode
                            break;
                        case 0x3: // ,--R
                            prepostinc = -2;
                            break;
                        case 0x4: // ,R
                            break;
                        case 0x5: // B,R
                            off = SignExtend(mB);
                            break;
                        case 0x6: // A,R
                            off = SignExtend(mA);
                            break;
                        case 0x8: // n,R (8 bit offset)
                            temp8 = mSys.MemRead8(mPC++);
                            off = SignExtend(temp8);
                            break;
                        case 0x9: // n,R (16 bit offset)
                            temp16 = mSys.MemRead16(mPC);
                            mPC += 2;
                            off = SignExtend(temp16);
                            break;
                        case 0xb: // D,R
                            off = SignExtend(mD);
                            break;
                        case 0xc: // n,PC (8 bit offset)
                            temp8 = mSys.MemRead8(mPC++);
                            off = SignExtend(temp8);
                            reg = &mPC;
                            break;
                        case 0xd: // n,PC (16 bit offset)
                            temp16 = mSys.MemRead16(mPC);
                            mPC += 2;
                            off = SignExtend(temp16);
                            reg = &mPC;
                            break;
                        case 0xf: // [n] (16 bit absolute indirect)
                            temp16 = mSys.MemRead16(mPC);
                            mPC += 2;
                            off = SignExtend(temp16);
                            reg = &zero;
                            indirect = true;
                            break;
                        default:
                            // unhandled mode 0x7, 0xa, 0xe (6309 modes for E, F, and W register)
                            fflush(stdout);
                            fprintf(stderr, "unhandled indexed addressing mode\n");
                            fflush(stderr);
                            assert(0);
                    }
                }

                TRACEF(" offset %d prepostinc %d", off, prepostinc);

                // register we're offsetting from in the usual case
                if (!reg) {
                    switch (BITS_SHIFT(temp8, 6, 5)) {
                        case 0: reg = &mX; break;
                        case 1: reg = &mY; break;
                        case 2: reg = &mU; break;
                        case 3: reg = &mS; break;
                    }
                }

                // handle predecrement
                if (prepostinc < 0)
                    *reg += prepostinc;

                // compute offset
                uint16_t addr = *reg + off;

                // handle postincrement
                if (prepostinc > 0)
                    *reg += prepostinc;

                TRACEF(" addr %#04x", addr);

                // if we're indirecting, load the address from addr
                if (indirect) {
                    addr = mSys.MemRead16(addr);
                    TRACEF(" [addr] %#04x", addr);
                }

                if (!op->calcaddr) {
                    if (op->width == 1) {
                        arg = mSys.MemRead8(addr);
                    } else {
                        arg = mSys.MemRead16(addr);
                    }
                } else {
                    arg = addr;
                }
                break;
            }
            default:
                fprintf(stderr, "unhandled addressing mode\n");
                assert(0);
        }

        TRACEF(" arg %#02x", arg);

        // decode on our table based opcode
        switch (op->op) {
            case ADD: { // add[abd]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a + b;

                SET_Z(result);
                if (op->width == 1) {
                    SET_HNVC1(a, b, result);
                } else {
                    SET_NVC2(a, b, result);
                }

                PutReg(op->targetreg, result);
                break;
            }
            case ADC: { // adc[ab]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a + b;

                if (!!(mCC & CC_C))
                    result += 1;

                SET_Z(result);
                if (op->width == 1) {
                    SET_HNVC1(a, b, result);
                } else {
                    SET_NVC2(a, b, result);
                }

                PutReg(op->targetreg, result);
                break;
            }
            case SUB: { // sub[abd]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = -arg;
                uint32_t result = a + b;

                // XXX make sure carry is okay
                SET_Z(result);
                if (op->width == 1) {
                    SET_HNVC1(a, b, result);
                } else {
                    SET_NVC2(a, b, result);
                }

                PutReg(op->targetreg, result);
                break;
            }
            case CMP: { // cmp[abdsuxy]
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a - b;

                SET_Z(result);
                if (op->width == 1) {
                    SET_HNVC1(a, b, result);
                } else {
                    SET_NVC2(a, b, result);
                }
                break;
            }
            case AND: { // and[ab],andcc
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
            case EOR: { // eor[ab],eorcc
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a ^ b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, result);
                break;
            }
            case OR: { // or[ab],orcc
                uint32_t a = GetReg(op->targetreg);
                uint32_t b = arg;
                uint32_t result = a | b;

                SET_NZ1(result);
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, result);
                break;
            }

            case TST: // tst[ab],tst
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = CLR_CC_BIT(CC_V);
                SET_NZ1(temp8);
                break;

            // class with a similar set of memory access pattern
            case CLR: // clr[ab],clr
                temp8 = 0;
                mCC = CLR_CC_BIT(CC_V);
                mCC = CLR_CC_BIT(CC_C);
                goto shared_memwrite;

            case COM: // com[ab],com
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }
                temp8 = ~temp8;

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
                goto shared_memwrite;

            case ASL: // asl[ab],asl
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = (BIT_SHIFT(temp8, 6) ^ BIT_SHIFT(temp8, 7)) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                mCC = BIT(temp8, 7) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = (temp8 << 1) & 0xff;
                goto shared_memwrite;

            case ASR: // asr[ab],asr
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = BIT(temp8, 7) | ((temp8 & 0xff) >> 1);
                goto shared_memwrite;

            case LSR: // lsr[ab],lsr
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = ((temp8 & 0xff) >> 1);
                goto shared_memwrite;

            case ROL: { // rol[ab],rol
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                bool oldc = !!(mCC & CC_C);

                mCC = (BIT_SHIFT(temp8, 6) ^ BIT_SHIFT(temp8, 7)) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                mCC = BIT(temp8, 7) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = ((temp8 << 1) & 0xff) | (oldc ? 1 : 0);
                goto shared_memwrite;
            }

            case ROR: { // ror[ab],ror
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }

                bool oldc = !!(mCC & CC_C);
                mCC = BIT(temp8, 0) ? SET_CC_BIT(CC_C) : CLR_CC_BIT(CC_C);

                temp8 = (oldc ? 0x80 : 0) | ((temp8 & 0xff) >> 1);
                goto shared_memwrite;
            }

            case DEC: // dec[ab],dec
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }
                temp8--;

                mCC = (temp8 == 0x7f) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                goto shared_memwrite;

            case INC: // inc[ab],inc
                if (op->mode == IMPLIED) {
                    temp8 = GetReg(op->targetreg);
                } else {
                    temp8 = mSys.MemRead8(arg);
                }
                temp8++;

                mCC = (temp8 == 0x80) ? SET_CC_BIT(CC_V) : CLR_CC_BIT(CC_V);
                goto shared_memwrite;

shared_memwrite:
                if (op->mode == IMPLIED) {
                    PutReg(op->targetreg, temp8);
                } else {
                    if (op->width == 1)
                        mSys.MemWrite8(arg, temp8);
                    else
                        mSys.MemWrite16(arg, temp8);
                }

                SET_NZ1(temp8);
                break;
            case LEA: // lea[suxy]
                PutReg(op->targetreg, (uint16_t)arg);

                if (op->targetreg == REG_X || op->targetreg == REG_Y)
                    SET_Z(arg);
                break;
            case ABX: // X += B
                PutReg(REG_X, GetReg(REG_X) + GetReg(REG_B));
                break;
            case TFR: { // R <= R1
                temp8 = mSys.MemRead8(mPC++);
                TRACEF(" arg %#02x", temp8);

                // source register
                switch (BITS_SHIFT(temp8, 7, 4)) {
                    case 0: temp16 = GetReg(REG_A) << 8 | GetReg(REG_B); break;
                    case 1: temp16 = GetReg(REG_X); break;
                    case 2: temp16 = GetReg(REG_Y); break;
                    case 3: temp16 = GetReg(REG_U); break;
                    case 4: temp16 = GetReg(REG_S); break;
                    case 5: temp16 = GetReg(REG_PC); break;

                    case 8: temp16 = GetReg(REG_A); break;
                    case 9: temp16 = GetReg(REG_B); break;
                    case 10: temp16 = mCC; break;
                    case 11: temp16 = mDP; break;

                    default: // undefined
                        temp16 = 0;
                        break;
                }

                // dest register
                switch (BITS_SHIFT(temp8, 3, 0)) {
                    case 0:
                        PutReg(REG_A, temp16 >> 8);
                        PutReg(REG_B, temp16);
                        break;
                    case 1: PutReg(REG_X, temp16); break;
                    case 2: PutReg(REG_Y, temp16); break;
                    case 3: PutReg(REG_U, temp16); break;
                    case 4: PutReg(REG_S, temp16); break;
                    case 5: PutReg(REG_PC, temp16); break;

                    case 8: PutReg(REG_A, temp16); break;
                    case 9: PutReg(REG_B, temp16); break;
                    case 10: mCC = temp16; break; // XXX deal with bits that are masked?
                    case 11: mDP = temp16; break;
                    default: // undefined
                        break;
                }
                break;
            }
            case PUSH: { // pshs,pshu
                TRACEF(" push word %#02x", arg);
                if (BIT(arg, 7)) {
                    TRACEF(" PC");
                    PUSH16(op->targetreg, mPC);
                }
                if (BIT(arg, 6)) {
                    if (op->targetreg == REG_U) {
                        TRACEF(" SP");
                        PUSH16(op->targetreg, mS);
                    } else {
                        TRACEF(" UP");
                        PUSH16(op->targetreg, mU);
                    }
                }
                if (BIT(arg, 5)) {
                    TRACEF(" Y");
                    PUSH16(op->targetreg, mY);
                }
                if (BIT(arg, 4)) {
                    TRACEF(" X");
                    PUSH16(op->targetreg, mX);
                }
                if (BIT(arg, 3)) {
                    TRACEF(" DP");
                    PUSH8(op->targetreg, mDP);
                }
                if (BIT(arg, 2)) {
                    TRACEF(" B");
                    PUSH8(op->targetreg, mB);
                }
                if (BIT(arg, 1)) {
                    TRACEF(" A");
                    PUSH8(op->targetreg, mA);
                }
                if (BIT(arg, 0)) {
                    TRACEF(" CC");
                    PUSH8(op->targetreg, mCC);
                }
                break;
            }
            case PULL: { // puls,pulu
                TRACEF(" pull word %#02x", arg);
                if (BIT(arg, 0)) {
                    TRACEF(" CC");
                    PULL8(op->targetreg, mCC);
                }
                if (BIT(arg, 1)) {
                    TRACEF(" A");
                    PULL8(op->targetreg, mA);
                }
                if (BIT(arg, 2)) {
                    TRACEF(" B");
                    PULL8(op->targetreg, mB);
                }
                if (BIT(arg, 3)) {
                    TRACEF(" DP");
                    PUSH8(op->targetreg, mDP);
                }
                if (BIT(arg, 4)) {
                    TRACEF(" X");
                    PULL16(op->targetreg, mX);
                }
                if (BIT(arg, 5)) {
                    TRACEF(" Y");
                    PULL16(op->targetreg, mY);
                }
                if (BIT(arg, 6)) {
                    if (op->targetreg == REG_U) {
                        TRACEF(" SP");
                        PULL16(op->targetreg, mS);
                    } else {
                        TRACEF(" UP");
                        PULL16(op->targetreg, mU);
                    }
                }
                if (BIT(arg, 7)) {
                    TRACEF(" PC");
                    PULL16(op->targetreg, mPC);
                }
                break;
            }
            case BRA: { // branch
                TRACEF(" arg %d", arg);

                bool takebranch = TestBranchCond(op->cond);

                if (takebranch) {
                    if (arg == -2) {
                        printf("infinite loop detected, aborting\n");
                        done = true;
                    }
                    mPC += arg;
                    mPC &= 0xffff;
                    TRACEF(" target %#04x", mPC);
                }
                break;
            }
            case BSR: { // bsr
                TRACEF(" arg %d", arg);

                PUSH16(REG_S, mPC);

                mPC += arg;
                TRACEF(" target %#04x", mPC);
                break;
            }
            case JMP: { // jmp
                TRACEF(" arg %#04x", arg);

                mPC = arg;
                break;
            }
            case JSR: { // jsr
                TRACEF(" arg %#04x", arg);

                PUSH16(REG_S, mPC);

                mPC = arg;
                break;
            }
            case RTS: { // rts
                PULL16(REG_S, temp16);
                TRACEF(" from stack %#04x", temp16);

                mPC = temp16;
                break;
            }
            case LD: // ld[abdsuxy]
                if (op->width == 1) {
                    SET_NZ1(arg);
                } else {
                    SET_NZ2(arg);
                }
                mCC = CLR_CC_BIT(CC_V);

                PutReg(op->targetreg, arg);
                break;
            case ST: // sta,stb,std,sts,stu,stx,sty
                if (op->width == 1) {
                    temp8 = GetReg(op->targetreg);
                    mSys.MemWrite8(arg, temp8);
                    SET_NZ1(temp8);
                } else {
                    temp16 = GetReg(op->targetreg);
                    mSys.MemWrite16(arg, temp16);
                    SET_NZ2(temp16);
                }
                mCC = CLR_CC_BIT(CC_V);
                break;
            case BADOP:
            default:
                fflush(stdout);
                fprintf(stderr, "unhandled opcode %#02x\n", op->op);
                done = true;
        }

        TRACEF("\n");

        if (TRACE)
            Dump();

        // see if the system has requested a shutdown
        if (mSys.isShutdown())
            done = true;
    }

    return 0;
}

void Cpu6809::Dump()
{
    char str[256];

    snprintf(str, sizeof(str), "A 0x%02x B 0x%02x D 0x%04x X 0x%04x Y 0x%04x U 0x%04x S 0x%04x DP 0x%02x CC 0x%02x (%c%c%c%c%c) PC 0x%04x",
        mA, mB, mD, mX, mY, mU, mS, mDP, mCC,
        (mCC & CC_H) ? 'h' : ' ',
        (mCC & CC_N) ? 'n' : ' ',
        (mCC & CC_Z) ? 'z' : ' ',
        (mCC & CC_V) ? 'v' : ' ',
        (mCC & CC_C) ? 'c' : ' ',
        mPC);

    printf("%s\n", str);
}

uint16_t Cpu6809::GetReg(regnum r)
{
    switch (r) {
        case REG_X: return mX;
        case REG_Y: return mY;
        case REG_U: return mU;
        case REG_S: return mS;
        case REG_A: return mA;
        case REG_B: return mB;
        case REG_D: return mD;
        case REG_PC: return mPC;
        case REG_DP: return mDP;
        case REG_CC: return mCC;
    }
}

void Cpu6809::PutReg(regnum r, uint16_t val)
{
    switch (r) {
        case REG_X: mX = val; break;
        case REG_Y: mY = val; break;
        case REG_U: mU = val; break;
        case REG_S: mS = val; break;
        case REG_A: mA = val; break;
        case REG_B: mB = val; break;
        case REG_D: mD = val; break;
        case REG_PC: mPC = val; break;
        case REG_DP: mDP = val; break;
        case REG_CC: mCC = val; break;
    }
}

