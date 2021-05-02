// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2016 Travis Geiselbrecht
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
#include "cpuz80.h"

#include <cstdio>
#include <cassert>
#include <iostream>

#include "system/system.h"
#include "bits.h"
#include "trace.h"

#define LOCAL_TRACE 0

#define FLAG_C  (0)
#define FLAG_N  (1)
#define FLAG_PV (2)
#define FLAG_F3 (3)
#define FLAG_H  (4)
#define FLAG_F5 (5)
#define FLAG_Z  (6)
#define FLAG_S  (7)

#define READ_AF() ((mRegs.a << 8) | mRegs.f)
#define READ_BC() ((mRegs.b << 8) | mRegs.c)
#define READ_DE() ((mRegs.d << 8) | mRegs.e)
#define READ_HL() ((mRegs.h << 8) | mRegs.l)
#define READ_SP() (mRegs.sp)

#define READ_AF_ALT() ((mRegs.a_alt << 8) | mRegs.f_alt)
#define READ_BC_ALT() ((mRegs.b_alt << 8) | mRegs.c_alt)
#define READ_DE_ALT() ((mRegs.d_alt << 8) | mRegs.e_alt)
#define READ_HL_ALT() ((mRegs.h_alt << 8) | mRegs.l_alt)

#define WRITE_AF(val) do { mRegs.a = ((val) >> 8) & 0xff; mRegs.f = (val) & 0xff; } while (0)
#define WRITE_BC(val) do { mRegs.b = ((val) >> 8) & 0xff; mRegs.c = (val) & 0xff; } while (0)
#define WRITE_DE(val) do { mRegs.d = ((val) >> 8) & 0xff; mRegs.e = (val) & 0xff; } while (0)
#define WRITE_HL(val) do { mRegs.h = ((val) >> 8) & 0xff; mRegs.l = (val) & 0xff; } while (0)
#define WRITE_SP(val) do { mRegs.sp = (val); } while (0)

#define WRITE_AF_ALT(val) do { mRegs.a_alt = ((val) >> 8) & 0xff; mRegs.f_alt = (val) & 0xff; } while (0)
#define WRITE_BC_ALT(val) do { mRegs.b_alt = ((val) >> 8) & 0xff; mRegs.c_alt = (val) & 0xff; } while (0)
#define WRITE_DE_ALT(val) do { mRegs.d_alt = ((val) >> 8) & 0xff; mRegs.e_alt = (val) & 0xff; } while (0)
#define WRITE_HL_ALT(val) do { mRegs.h_alt = ((val) >> 8) & 0xff; mRegs.l_alt = (val) & 0xff; } while (0)

uint16_t CpuZ80::read_qq_reg(int dd) {
    switch (dd) {
        default:
        case 0b00:
            return READ_BC();
        case 0b01:
            return READ_DE();
        case 0b10:
            return READ_HL();
        case 0b11:
            return READ_AF();
    }
}

void CpuZ80::write_qq_reg(int dd, uint16_t val) {
    switch (dd) {
        default:
        case 0b00:
            WRITE_BC(val);
            break;
        case 0b01:
            WRITE_DE(val);
            break;
        case 0b10:
            WRITE_HL(val);
            break;
        case 0b11:
            WRITE_AF(val);
            break;
    }
}

uint16_t CpuZ80::read_dd_reg(int dd) {
    switch (dd) {
        default:
        case 0b00:
            return READ_BC();
        case 0b01:
            return READ_DE();
        case 0b10:
            return READ_HL();
        case 0b11:
            return READ_SP();
    }
}

void CpuZ80::write_dd_reg(int dd, uint16_t val) {
    switch (dd) {
        default:
        case 0b00:
            WRITE_BC(val);
            break;
        case 0b01:
            WRITE_DE(val);
            break;
        case 0b10:
            WRITE_HL(val);
            break;
        case 0b11:
            WRITE_SP(val);
            break;
    }
}

uint8_t CpuZ80::read_r_reg(int r) {
    switch (r) {
        case 0b000:
            return mRegs.b;
        case 0b001:
            return mRegs.c;
        case 0b010:
            return mRegs.d;
        case 0b011:
            return mRegs.e;
        case 0b100:
            return mRegs.h;
        case 0b101:
            return mRegs.l;
        case 0b111:
            return mRegs.a;
        default:
            /* no 0xb110 */
            assert(0);
    }
}

// for opcodes where the missing register encoding hole is for (HL)
uint8_t CpuZ80::read_r_reg_or_hl(int r) {
    if (r == 0b110) {
        return mSys.MemRead8(READ_HL());
    } else {
        return read_r_reg(r);
    }
}

void CpuZ80::write_r_reg(int r, uint8_t val) {
    switch (r) {
        case 0b000:
            mRegs.b = val;
            break;
        case 0b001:
            mRegs.c = val;
            break;
        case 0b010:
            mRegs.d = val;
            break;
        case 0b011:
            mRegs.e = val;
            break;
        case 0b100:
            mRegs.h = val;
            break;
        case 0b101:
            mRegs.l = val;
            break;
        case 0b111:
            mRegs.a = val;
            break;
        default:
            /* no 0xb110 */
            assert(0);
    }
}

// for opcodes where the missing register encoding hole is for (HL)
void CpuZ80::write_r_reg_or_hl(int r, uint8_t val) {
    if (r == 0b110) {
        mSys.MemWrite8(READ_HL(), val);
    } else {
        write_r_reg(r, val);
    }
}

uint16_t CpuZ80::read_nn() {
    uint16_t val = mSys.MemRead8(mRegs.pc) + (mSys.MemRead8(mRegs.pc + 1) << 8);
    mRegs.pc += 2;
    return val;
}

uint8_t CpuZ80::read_n() {
    return mSys.MemRead8(mRegs.pc++);
}

void CpuZ80::push8(uint8_t val) {
    mSys.MemWrite8(--mRegs.sp, val);
}

void CpuZ80::push16(uint16_t val) {
    LTRACEF("pushing 0x%hx\n", val);
    push8((val >> 8) & 0xff);
    push8(val & 0xff);
}

void CpuZ80::push_pc() {
    push16(mRegs.pc);
}

uint8_t CpuZ80::pop8() {
    return mSys.MemRead8(mRegs.sp++);
}

uint16_t CpuZ80::pop16() {
    uint16_t val;

    val = pop8();
    val |= pop8() << 8;

    LTRACEF("popping 0x%hx\n", val);

    return val;
}

void CpuZ80::out(uint8_t addr, uint8_t val) {
    LTRACEF("OUT 0x%hhx = 0x%hhx\n", addr, val);

    mSys.IOWrite8(addr, val);
}

uint8_t CpuZ80::in(uint8_t addr) {
    LTRACEF("IN 0x%hhx\n", addr);

    return mSys.IORead8(addr);
}

void CpuZ80::set_flag(int flag, int val) {
    if (val)
        mRegs.f |= (1 << flag);
    else
        mRegs.f &= ~(1 << flag);
}

bool CpuZ80::get_flag(int flag) {
    return !!(mRegs.f & (1<<flag));
}


bool CpuZ80::test_cond(int cond) {
    switch (cond) {
        case 0:
            if (!get_flag(FLAG_Z)) return true;
            break; // NZ
        case 1:
            if (get_flag(FLAG_Z))  return true;
            break; // Z
        case 2:
            if (!get_flag(FLAG_C)) return true;
            break; // NC
        case 3:
            if (get_flag(FLAG_C))  return true;
            break; // C
        case 4:
            if (!get_flag(FLAG_PV)) return true;
            break; // PO
        case 5:
            if (get_flag(FLAG_PV)) return true;
            break; // PE
        case 6:
            if (!get_flag(FLAG_S)) return true;
            break; // P
        case 7:
            if (get_flag(FLAG_S))  return true;
            break; // M
    }
    return false;
}

/* count even number of bits */
static int calc_parity(uint8_t val) {
    int count;

    count = 0;
    while (val != 0) {
        if (val & 1)
            count++;
        val >>= 1;
    }

    if ((count & 1) == 0)
        return 1;
    else
        return 0;
}

void CpuZ80::set_flags(uint8_t val) {
    set_flag(FLAG_S, BIT(val, 7)); // sign flag
    set_flag(FLAG_Z, val == 0); // zero flag
    set_flag(FLAG_PV, calc_parity(val));
    set_flag(FLAG_H, 0);
    set_flag(FLAG_N, 0);
    set_flag(FLAG_C, 0);
}

int CpuZ80::Run() {
    LTRACEF("Run\n");

    int dd;

    for (;;) {
        uint8_t temp8;
        uint16_t temp16;

        uint8_t op = mSys.MemRead8(mRegs.pc++);

        // look for certain prefixes
        if (op == 0xed) {
            // ed prefix is a whole new space
            op = mSys.MemRead8(mRegs.pc++);

            LPRINTF("PC 0x%04hx: op ed%02hhx - ", (uint16_t)(mRegs.pc - 2), op);
            switch (op) {
                case 0b01000001:
                case 0b01001001:
                case 0b01010001:
                case 0b01011001:
                case 0b01100001:
                case 0b01101001:
//              case 0b01110001: /* doesn't officially exist */
                case 0b01111001: // OUT (C), r
                    LPRINTF("OUT (C), r\n");
                    if (BITS_SHIFT(op, 5, 3) == 0b110) { // OUT (c), 0
                        temp8 = 0;
                    } else {
                        temp8 = read_r_reg(BITS_SHIFT(op, 5, 3));
                    }
                    out(mRegs.c, temp8);
                    break;
                case 0b10110000: // LDIR
                    LPRINTF("LDIR\n");

                    temp8 = mSys.MemRead8(READ_HL());
                    mSys.MemWrite8(READ_DE(), temp8);

                    LPRINTF("copying from 0x%x to 0x%x value 0x%hhx\n", READ_HL(), READ_DE(), temp8);

                    WRITE_HL(READ_HL() + 1);
                    WRITE_DE(READ_DE() + 1);
                    WRITE_BC(READ_BC() - 1);
                    if (READ_BC() != 0)
                        mRegs.pc -= 2; // repeat the instruction

                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_PV, 0);
                    set_flag(FLAG_N, 0);
                    break;
                case 0b01000011:
                case 0b01010011:
                case 0b01100011:
                case 0b01110011: // LD (nn), dd
                    LPRINTF("LD (nn), dd\n");

                    temp16 = read_dd_reg(BITS_SHIFT(op, 5, 4));
                    mSys.MemWrite16(read_nn(), temp16);
                    break;
                case 0b01001011:
                case 0b01011011:
                case 0b01101011:
                case 0b01111011: // LD dd, (nn)
                    LPRINTF("LD dd, (nn)\n");

                    temp16 = mSys.MemRead16(read_nn());
                    write_dd_reg(BITS_SHIFT(op, 5, 4), temp16);
                    break;
                default:
                    fprintf(stderr, "unhandled ED prefixed-opcode 0x%hhx\n", op);
                    return -1;
            }
        } else if (op == 0xcb) {
            // cb prefix are for bit instructions
            op = mSys.MemRead8(mRegs.pc++);

            LPRINTF("PC 0x%04hx: op cb%02hhx - ", (uint16_t)(mRegs.pc - 2), op);
            switch (op) {
                case 0x40 ... 0x7f: { // BIT
                    uint8_t bit = BITS_SHIFT(op, 6, 3);
                    LPRINTF("RES %u, r\n", bit);

                    temp8 = read_r_reg_or_hl(BITS(op, 2, 0)) & (1<<bit);
                    set_flag(FLAG_Z, temp8);
                    break;
                }
                case 0x80 ... 0xbf: { // RES
                    uint8_t bit = BITS_SHIFT(op, 6, 3);
                    LPRINTF("RES %u, r\n", bit);

                    write_r_reg_or_hl(BITS(op, 2, 0), read_r_reg_or_hl(BITS(op, 2, 0)) & ~(1<<bit));
                    break;
                }
                case 0xc0 ... 0xff: { // SET
                    uint8_t bit = BITS_SHIFT(op, 6, 3);
                    LPRINTF("SET %u, r\n", bit);

                    write_r_reg_or_hl(BITS(op, 2, 0), read_r_reg_or_hl(BITS(op, 2, 0)) | (1<<bit));
                    break;
                }
                default:
                    fprintf(stderr, "unhandled CB prefixed-opcode 0x%hhx\n", op);
                    return -1;
            }
        } else {
            LPRINTF("PC 0x%04hx: op %02hhx - ", (uint16_t)(mRegs.pc - 1), op);
            switch (op) {
                case 0x00: // NOP
                    LPRINTF("NOP\n");
                    break;
                case 0b11000011: // JP nn
                    LPRINTF("JP nn\n");
                    mRegs.pc = read_nn();
                    break;
                case 0b11000010:
                case 0b11001010:
                case 0b11010010:
                case 0b11011010:
                case 0b11100010:
                case 0b11101010:
                case 0b11110010:
                case 0b11111010: { // JP cc, nn
                    LPRINTF("JP cc, nn\n");
                    int cond = BITS_SHIFT(op, 5, 3);
                    temp16 = read_nn();

                    if (test_cond(cond))
                        mRegs.pc = temp16;
                    break;
                }
                case 0b11001101: // CALL nn
                    LPRINTF("CALL nn\n");
                    temp16 = read_nn();
                    push_pc();
                    mRegs.pc = temp16;
                    break;
                case 0b11000100:
                case 0b11001100:
                case 0b11010100:
                case 0b11011100:
                case 0b11100100:
                case 0b11101100:
                case 0b11110100:
                case 0b11111100: { // CALL cc, nn
                    LPRINTF("CALL cc, nn\n");
                    int cond = BITS_SHIFT(op, 5, 3);
                    temp16 = read_nn();

                    if (test_cond(cond)) {
                        push_pc();
                        mRegs.pc = temp16;
                    }
                    break;
                }

                case 0b11001001: // RET
                    LPRINTF("RET\n");
                    mRegs.pc = pop16();
                    break;

                case 0b11000000:
                case 0b11001000:
                case 0b11010000:
                case 0b11011000:
                case 0b11100000:
                case 0b11101000:
                case 0b11110000:
                case 0b11111000: { // RET cc
                    LPRINTF("RET cc\n");
                    int cond = BITS_SHIFT(op, 5, 3);

                    if (test_cond(cond)) {
                        mRegs.pc = pop16();
                    }
                    break;
                }

                case 0b00010000: { // DJNZ
                    LPRINTF("DJNZ, e\n");
                    int8_t rel = read_n();
                    mRegs.b--;
                    if (mRegs.b) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case 0b00011000: { // JR e
                    LPRINTF("JR e\n");
                    int8_t rel = read_n();
                    mRegs.pc += rel;
                    break;
                }
                case 0b00111000: { // JR C, e
                    LPRINTF("JR C, e\n");
                    int8_t rel = read_n();
                    if (get_flag(FLAG_C))
                        mRegs.pc += rel;
                    break;
                }
                case 0b00110000: { // JR NC, e
                    LPRINTF("JR NC, e\n");
                    int8_t rel = read_n();
                    if (!get_flag(FLAG_C))
                        mRegs.pc += rel;
                    break;
                }
                case 0b00101000: { // JR Z, e
                    LPRINTF("JR Z, e\n");
                    int8_t rel = read_n();
                    if (get_flag(FLAG_Z))
                        mRegs.pc += rel;
                    break;
                }
                case 0b00100000: { // JR NZ, e
                    LPRINTF("JR NZ, e\n");
                    int8_t rel = read_n();
                    if (!get_flag(FLAG_Z))
                        mRegs.pc += rel;
                    break;
                }
                case 0b11110011: // DI
                    LPRINTF("DI\n");
                    mRegs.iff = 0;
                    break;
                case 0b11111011: // EI
                    LPRINTF("EI\n");
                    mRegs.iff = 1;
                    break;

                case 0b11010011: // OUT (n), A
                    LPRINTF("OUT (n), A\n");
                    out(read_n(), mRegs.a);
                    break;

                case 0b11011011: // IN A, (n)
                    LPRINTF("IN A, (n)\n");
                    mRegs.a = in(read_n());
                    break;

                case 0b01000000 ... 0b01111111: { // LD r, r or LD r, (HL)
                    LPRINTF("LD r, r\n");

                    int r = BITS_SHIFT(op, 5, 3);
                    int r2 = BITS_SHIFT(op, 2, 0);

                    if (r == r2 && r == 0b110) { // HALT
                        printf("unhandled halt opcode\n");
                        return -1;
                    }

                    write_r_reg_or_hl(r, read_r_reg_or_hl(r2));
                    break;
                }

                case 0b00110010: // LD (nn), A
                    LPRINTF("LD (nn), A)\n");
                    mSys.MemWrite8(read_nn(), mRegs.a);
                    break;
                case 0b00000010: // LD (BC), A
                    LPRINTF("LD (BC), A)\n");
                    mSys.MemWrite8(READ_BC(), mRegs.a);
                    break;
                case 0b00010010: // LD (DE), A
                    LPRINTF("LD (DE), A)\n");
                    mSys.MemWrite8(READ_DE(), mRegs.a);
                    break;

                case 0b00000110:
                case 0b00001110:
                case 0b00010110:
                case 0b00011110:
                case 0b00100110:
                case 0b00101110:
                case 0b00111110: // LD r, n
                    LPRINTF("LD r, n\n");
                    write_r_reg(BITS_SHIFT(op, 5, 3), read_n());
                    break;
                case 0b00110110: // LD (HL), n
                    LPRINTF("LD (HL), n\n");
                    mSys.MemWrite8(READ_HL(), read_n());
                    break;
                case 0b00000001:
                case 0b00010001:
                case 0b00100001:
                case 0b00110001: // LD dd, nn
                    LPRINTF("LD dd, nn\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_nn());
                    break;

                case 0b00100010: // LD (nn), HL
                    LPRINTF("LD (nn), HL\n");
                    temp16 = read_nn();

                    mSys.MemWrite8(temp16, mRegs.l);
                    mSys.MemWrite8(temp16 + 1, mRegs.h);
                    break;
                case 0b00101010: // LD HL, (nn)
                    LPRINTF("LD HL, (nn)\n");
                    temp16 = read_nn();

                    mRegs.l = mSys.MemRead8(temp16);
                    mRegs.h = mSys.MemRead8(temp16 + 1);
                    break;
                case 0b00111010: // LD A, (nn)
                    LPRINTF("LD A, (nn)\n");
                    temp16 = read_nn();

                    mRegs.a = mSys.MemRead8(temp16);
                    break;
                case 0b00001010: // LD A, (BC)
                    LPRINTF("LD A, (BC)\n");
                    mRegs.a = mSys.MemRead8(READ_BC());
                    break;
                case 0b00011010: // LD A, (DE)
                    LPRINTF("LD A, (DE)\n");
                    mRegs.a = mSys.MemRead8(READ_DE());
                    break;
                case 0b11000101:
                case 0b11010101:
                case 0b11100101:
                case 0b11110101: // PUSH qq
                    LPRINTF("PUSH qq\n");
                    push16(read_qq_reg(BITS_SHIFT(op, 5, 4)));
                    break;

                case 0b11000001:
                case 0b11010001:
                case 0b11100001:
                case 0b11110001: // POP qq
                    LPRINTF("POP qq\n");
                    temp16 = pop16();
                    write_qq_reg(BITS_SHIFT(op, 5, 4), temp16);
                    break;

                case 0b11100011: // EX (SP), HL
                    LPRINTF("EX (SP), HL\n");
                    temp16 = pop16();
                    push16(READ_HL());
                    WRITE_HL(temp16);
                    break;

                case 0b00001000: // EX AF, AF'
                    LPRINTF("EX AF, AF'\n");

                    temp16 = READ_AF();
                    WRITE_AF(READ_AF_ALT());
                    WRITE_AF_ALT(temp16);
                    break;

                case 0b00001001:
                case 0b00011001:
                case 0b00101001:
                case 0b00111001: { // ADD HL, ss
                    LPRINTF("ADD HL, ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    temp16 = read_dd_reg(dd);
                    uint16_t hl = READ_HL();
                    WRITE_HL(hl + temp16);

                    // compute the flags
                    set_flag(FLAG_C, (uint32_t)hl + temp16 > 0xff); // carry out of bit 15
                    set_flag(FLAG_H, (hl & 0xfff) + (temp16 & 0xfff) > 0xfff); // carry out of bit 11
                    set_flag(FLAG_N, 0);
                    break;
                }
                case 0b00001011:
                case 0b00011011:
                case 0b00101011:
                case 0b00111011: // DEC ss
                    LPRINTF("DEC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_dd_reg(dd) - 1);
                    break;
                case 0b00000011:
                case 0b00010011:
                case 0b00100011:
                case 0b00110011: // INC ss
                    LPRINTF("INC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_dd_reg(dd) + 1);
                    break;

                case 0b00000100:
                case 0b00001100:
                case 0b00010100:
                case 0b00011100:
                case 0b00100100:
                case 0b00101100:
                case 0b00111100: { // INC r
                    LPRINTF("INC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;

                    old = read_r_reg_or_hl(r);
                    temp8 = old + 1;
                    write_r_reg_or_hl(r, temp8);

                    set_flag(FLAG_PV, old == 0x7f);
                    set_flag(FLAG_S, temp8 & 0x80);
                    set_flag(FLAG_Z, temp8 == 0);
                    set_flag(FLAG_N, 0);

                    // half carry
                    if (BIT(old, 3) && !BIT(temp8, 3)) {
                        set_flag(FLAG_H, 1);
                    } else {
                        set_flag(FLAG_H, 0);
                    }

                    break;
                }

                case 0b00000101:
                case 0b00001101:
                case 0b00010101:
                case 0b00011101:
                case 0b00100101:
                case 0b00101101:
                case 0b00111101: { // DEC r
                    LPRINTF("DEC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;

                    old = read_r_reg_or_hl(r);
                    temp8 = old - 1;
                    write_r_reg_or_hl(r, temp8);

                    set_flag(FLAG_PV, old == 0x80);
                    set_flag(FLAG_S, temp8 & 0x80);
                    set_flag(FLAG_Z, temp8 == 0);
                    set_flag(FLAG_N, 0);

                    // half carry
                    if (BIT(old, 4) && !BIT(temp8, 4)) {
                        set_flag(FLAG_H, 1);
                    } else {
                        set_flag(FLAG_H, 0);
                    }

                    break;
                }

                case 0b10100000 ... 0b10100111: // AND r, AND (HL)
                    LPRINTF("AND r\n");
                    mRegs.a &= read_r_reg_or_hl(BITS(op, 2, 0));
                    set_flags(mRegs.a);
                    set_flag(FLAG_H, 1);
                    break;

                case 0b11100110: // AND n
                    LPRINTF("AND n\n");
                    mRegs.a &= mSys.MemRead8(read_n());
                    set_flags(mRegs.a);
                    set_flag(FLAG_H, 1);
                    break;

                case 0b10110000 ... 0b10110111: // OR r, OR (HL)
                    LPRINTF("OR r\n");
                    mRegs.a |= read_r_reg_or_hl(BITS(op, 2, 0));
                    set_flags(mRegs.a);
                    break;

                case 0b10101000 ... 0b10101111: // XOR r, XOR (HL)
                    LPRINTF("XOR r\n");
                    mRegs.a ^= read_r_reg_or_hl(BITS(op, 2, 0));
                    set_flags(mRegs.a);
                    break;

                case 0b10111000 ... 0b10111111: // CP r, CP (HL)
                    LPRINTF("CP r\n");
                    temp8 = mRegs.a - read_r_reg_or_hl(BITS(op, 2, 0));
                    set_flags(temp8);
                    break;

                case 0b11111110: // CP n
                    LPRINTF("CP n\n");
                    temp8 = mSys.MemRead8(read_n());
                    temp8 = mRegs.a - temp8;
                    set_flags(temp8);
                    break;

                case 0b00000111: // RLCA
                    LPRINTF("RLCA\n");
                    temp8 = (mRegs.a << 1) | ((mRegs.a >> 7) & 0x1);
                    set_flag(FLAG_C, mRegs.a & 0x80);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00001111: // RRCA
                    LPRINTF("RRCA\n");
                    temp8 = (mRegs.a >> 1) | ((mRegs.a << 7) & 0x80);
                    set_flag(FLAG_C, mRegs.a & 0x1);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00011111: // RRA
                    LPRINTF("RRA\n");
                    temp8 = (mRegs.a >> 1);
                    temp8 |= get_flag(FLAG_C) ? (1 << 7) : 0;
                    set_flag(FLAG_C, mRegs.a & 0x1);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00111111: // CCF
                    LPRINTF("CCF\n");
                    set_flag(FLAG_H, get_flag(FLAG_C));
                    set_flag(FLAG_C, !(mRegs.f & FLAG_C));
                    set_flag(FLAG_N, 0);
                    break;

                case 0b00110111: // SCF
                    LPRINTF("SCF\n");
                    set_flag(FLAG_C, 1);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    break;

                default:
                    fprintf(stderr, "unhandled opcode 0x%hhx\n", op);
                    return -1;
            }
        }

        if (LOCAL_TRACE)
            Dump();
    }

    return 0;
}

void CpuZ80::Dump() {
    printf("a 0x%02hhx f 0x%02hhx b 0x%02hhx c 0x%02hhx d 0x%02hhx e 0x%02hhx h 0x%02hhx l 0x%02hhx ",
           mRegs.a, mRegs.f, mRegs.b, mRegs.c, mRegs.d, mRegs.e, mRegs.h, mRegs.l);
    printf("sp 0x%04hx ix 0x%04hx iy 0x%04hx, pc 0x%04hx\n",
           mRegs.sp, mRegs.ix, mRegs.iy, mRegs.pc);
}

void CpuZ80::Reset() {
    LTRACEF("Reset\n");
    mRegs = {};

}



