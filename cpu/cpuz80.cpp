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

#include <boost/utility/binary.hpp>

#include "system/system.h"
#include "bits.h"
#include "trace.h"

#define LOCAL_TRACE 1

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

uint16_t CpuZ80::read_qq_reg(int dd)
{
    switch (dd) {
        default:
        case BOOST_BINARY(00): return READ_BC();
        case BOOST_BINARY(01): return READ_DE();
        case BOOST_BINARY(10): return READ_HL();
        case BOOST_BINARY(11): return READ_AF();
    }
}

void CpuZ80::write_qq_reg(int dd, uint16_t val)
{
    switch (dd) {
        default:
        case BOOST_BINARY(00): WRITE_BC(val); break;
        case BOOST_BINARY(01): WRITE_DE(val); break;
        case BOOST_BINARY(10): WRITE_HL(val); break;
        case BOOST_BINARY(11): WRITE_AF(val); break;
    }
}

uint16_t CpuZ80::read_dd_reg(int dd)
{
    switch (dd) {
        default:
        case BOOST_BINARY(00): return READ_BC();
        case BOOST_BINARY(01): return READ_DE();
        case BOOST_BINARY(10): return READ_HL();
        case BOOST_BINARY(11): return READ_SP();
    }
}

void CpuZ80::write_dd_reg(int dd, uint16_t val)
{
    switch (dd) {
        default:
        case BOOST_BINARY(00): WRITE_BC(val); break;
        case BOOST_BINARY(01): WRITE_DE(val); break;
        case BOOST_BINARY(10): WRITE_HL(val); break;
        case BOOST_BINARY(11): WRITE_SP(val); break;
    }
}

uint8_t CpuZ80::read_r_reg(int r)
{
    switch (r) {
        case BOOST_BINARY(000): return mRegs.b;
        case BOOST_BINARY(001): return mRegs.c;
        case BOOST_BINARY(010): return mRegs.d;
        case BOOST_BINARY(011): return mRegs.e;
        case BOOST_BINARY(100): return mRegs.h;
        case BOOST_BINARY(101): return mRegs.l;
        case BOOST_BINARY(111): return mRegs.a;
        default:
            /* no 0xb110 */
            assert(0);
    }
}

void CpuZ80::write_r_reg(int r, uint8_t val)
{
    switch (r) {
        case BOOST_BINARY(000): mRegs.b = val; break;
        case BOOST_BINARY(001): mRegs.c = val; break;
        case BOOST_BINARY(010): mRegs.d = val; break;
        case BOOST_BINARY(011): mRegs.e = val; break;
        case BOOST_BINARY(100): mRegs.h = val; break;
        case BOOST_BINARY(101): mRegs.l = val; break;
        case BOOST_BINARY(111): mRegs.a = val; break;
        default:
            /* no 0xb110 */
            assert(0);
    }
}

uint16_t CpuZ80::read_nn(void)
{
    uint16_t val = mSys.MemRead8(mRegs.pc) + (mSys.MemRead8(mRegs.pc + 1) << 8);
    mRegs.pc += 2;
    return val;
}

uint8_t CpuZ80::read_n(void)
{
    return mSys.MemRead8(mRegs.pc++);
}

void CpuZ80::push8(uint8_t val)
{
    mSys.MemWrite8(--mRegs.sp, val);
}

void CpuZ80::push16(uint16_t val)
{
    LTRACEF("pushing 0x%hx\n", val);
    push8((val >> 8) & 0xff);
    push8(val & 0xff);
}

void CpuZ80::push_pc(void)
{
    push16(mRegs.pc);
}

uint8_t CpuZ80::pop8(void)
{
    return mSys.MemRead8(mRegs.sp++);
}

uint16_t CpuZ80::pop16(void)
{
    uint16_t val;

    val = pop8();
    val |= pop8() << 8;

    LTRACEF("popping 0x%hx\n", val);

    return val;
}

void CpuZ80::out(uint8_t addr, uint8_t val)
{
    LTRACEF("OUT 0x%hhx = 0x%hhx\n", addr, val);

    mSys.IOWrite8(addr, val);
}

void CpuZ80::set_flag(int flag, int val)
{
    if (val)
        mRegs.f |= (1 << flag);
    else
        mRegs.f &= ~(1 << flag);
}

/* count even number of bits */
static int calc_parity(uint8_t val)
{
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

void CpuZ80::set_flags(uint8_t val)
{
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

        LPRINTF("PC 0x%04hx: op %02hhx", (uint16_t)(mRegs.pc - 1), op);

        // look for certain prefixes
        if (op == 0xed) {
            // ed prefix is a whole new space
            op = mSys.MemRead8(mRegs.pc++);

            LPRINTF("%02hhx - ", op);
            switch (op) {
                case BOOST_BINARY(01000001):
                case BOOST_BINARY(01001001):
                case BOOST_BINARY(01010001):
                case BOOST_BINARY(01011001):
                case BOOST_BINARY(01100001):
                case BOOST_BINARY(01101001):
//              case BOOST_BINARY(01110001): /* doesn't officially exist */
                case BOOST_BINARY(01111001): // OUT (C), r
                    LPRINTF("OUT (C), r\n");
                    temp8 = read_r_reg(BITS_SHIFT(op, 5, 3));
                    out(mRegs.c, temp8);
                    break;
                case BOOST_BINARY(10110000): // LDIR
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
                default:
                    fprintf(stderr, "unhandled ED prefixed-opcode 0x%hhx\n", op);
                    return -1;
            }
        } else {
            LPRINTF(" - ");
            switch (op) {
                case 0x00: // NOP
                    LPRINTF("NOP\n");
                    break;
                case 0xc3: // JP nn
                    LPRINTF("JP nn\n");
                    mRegs.pc = read_nn();
                    break;
                case BOOST_BINARY(11000010):
                case BOOST_BINARY(11001010):
                case BOOST_BINARY(11010010):
                case BOOST_BINARY(11011010):
                case BOOST_BINARY(11100010):
                case BOOST_BINARY(11101010):
                case BOOST_BINARY(11110010):
                case BOOST_BINARY(11111010): { // JP cc, nn
                    LPRINTF("JP cc, nn\n");
                    int cond = BITS_SHIFT(op, 5, 3);
                    temp16 = read_nn();

                    switch (cond) {
                        case 0: if (!(mRegs.f & FLAG_Z)) mRegs.pc = temp16; break; // NZ
                        case 1: if (mRegs.f & FLAG_Z) mRegs.pc = temp16; break; // Z
                        case 2: if (!(mRegs.f & FLAG_C)) mRegs.pc = temp16; break; // NC
                        case 3: if (mRegs.f & FLAG_C) mRegs.pc = temp16; break; // C
                        case 4: if (!(mRegs.f & FLAG_PV)) mRegs.pc = temp16; break; // PO
                        case 5: if (mRegs.f & FLAG_PV) mRegs.pc = temp16; break; // PE
                        case 6: if (!(mRegs.f & FLAG_S)) mRegs.pc = temp16; break; // P
                        case 7: if (mRegs.f & FLAG_S) mRegs.pc = temp16; break; // M
                    }
                    break;
                }
                case 0xcd: // CALL nn
                    LPRINTF("CALL nn\n");
                    temp16 = read_nn();
                    push_pc();
                    mRegs.pc = temp16;
                    break;
                case BOOST_BINARY(11000100):
                case BOOST_BINARY(11001100):
                case BOOST_BINARY(11010100):
                case BOOST_BINARY(11011100):
                case BOOST_BINARY(11100100):
                case BOOST_BINARY(11101100):
                case BOOST_BINARY(11110100):
                case BOOST_BINARY(11111100): { // CALL cc, nn
                    LPRINTF("CALL cc, nn\n");
                    int dobranch = 0;
                    int cond = BITS_SHIFT(op, 5, 3);
                    temp16 = read_nn();

                    switch (cond) {
                        case 0: if (!(mRegs.f & FLAG_Z)) dobranch = 1; break; // NZ
                        case 1: if (mRegs.f & FLAG_Z) dobranch = 1; break; // Z
                        case 2: if (!(mRegs.f & FLAG_C)) dobranch = 1; break; // NC
                        case 3: if (mRegs.f & FLAG_C) dobranch = 1; break; // C
                        case 4: if (!(mRegs.f & FLAG_PV)) dobranch = 1; break; // PO
                        case 5: if (mRegs.f & FLAG_PV) dobranch = 1; break; // PE
                        case 6: if (!(mRegs.f & FLAG_S)) dobranch = 1; break; // P
                        case 7: if (mRegs.f & FLAG_S) dobranch = 1; break; // M
                    }

                    if (dobranch) {
                        temp16 = read_nn();
                        push_pc();
                        mRegs.pc = temp16;
                    }
                    break;
                }

                case 0xc9: // RET
                    LPRINTF("RET\n");
                    mRegs.pc = pop16();
                    break;

                case BOOST_BINARY(11000000):
                case BOOST_BINARY(11001000):
                case BOOST_BINARY(11010000):
                case BOOST_BINARY(11011000):
                case BOOST_BINARY(11100000):
                case BOOST_BINARY(11101000):
                case BOOST_BINARY(11110000):
                case BOOST_BINARY(11111000): { // RET cc
                    LPRINTF("RET cc\n");
                    int doret = 0;
                    int cond = BITS_SHIFT(op, 5, 3);

                    switch (cond) {
                        case 0: if (!(mRegs.f & FLAG_Z)) doret = 1; break; // NZ
                        case 1: if (mRegs.f & FLAG_Z) doret = 1; break; // Z
                        case 2: if (!(mRegs.f & FLAG_C)) doret = 1; break; // NC
                        case 3: if (mRegs.f & FLAG_C) doret = 1; break; // C
                        case 4: if (!(mRegs.f & FLAG_PV)) doret = 1; break; // PO
                        case 5: if (mRegs.f & FLAG_PV) doret = 1; break; // PE
                        case 6: if (!(mRegs.f & FLAG_S)) doret = 1; break; // P
                        case 7: if (mRegs.f & FLAG_S) doret = 1; break; // M
                    }

                    if (doret) {
                        mRegs.pc = pop16();
                    }
                    break;
                }

                case 0x10: { // DJNZ
                    LPRINTF("DJNZ, e\n");
                    int8_t rel = read_n();
                    mRegs.b--;
                    if (mRegs.b) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case BOOST_BINARY(00011000): { // JR e
                    LPRINTF("JR e\n");
                    int8_t rel = read_n();
                    mRegs.pc += rel;
                    break;
                }
                case BOOST_BINARY(00111000): { // JR C, e
                    LPRINTF("JR C, e\n");
                    int8_t rel = read_n();
                    if (mRegs.f & FLAG_C)
                        mRegs.pc += rel;
                    break;
                }
                case BOOST_BINARY(00110000): { // JR NC, e
                    LPRINTF("JR NC, e\n");
                    int8_t rel = read_n();
                    if (!(mRegs.f & FLAG_C))
                        mRegs.pc += rel;
                    break;
                }
                case BOOST_BINARY(00101000): { // JR Z, e
                    LPRINTF("JR Z, e\n");
                    int8_t rel = read_n();
                    if (mRegs.f & FLAG_Z)
                        mRegs.pc += rel;
                    break;
                }
                case BOOST_BINARY(00100000): { // JR NZ, e
                    LPRINTF("JR NZ, e\n");
                    int8_t rel = read_n();
                    if (!(mRegs.f & FLAG_Z))
                        mRegs.pc += rel;
                    break;
                }
                case 0xf3: // DI
                    LPRINTF("DI\n");
                    mRegs.iff = 0;
                    break;
                case 0xfb: // EI
                    LPRINTF("EI\n");
                    mRegs.iff = 1;
                    break;

                case BOOST_BINARY(11010011): // OUT (n), A
                    LPRINTF("OUT (n), A\n");
                    out(read_n(), mRegs.a);
                    break;

                case BOOST_BINARY(01000000) ... BOOST_BINARY(01111111): { // LD r, r or LD r, (HL)

                    int r = BITS_SHIFT(op, 5, 3);
                    int r2 = BITS_SHIFT(op, 2, 0);

                    if (r2 == BOOST_BINARY(110)) { // (HL)
                        LPRINTF("LD r, (HL)\n");
                        temp8 = mSys.MemRead8(READ_HL());
                        write_r_reg(r, temp8);
                    } else {
                        LPRINTF("LD r, r\n");
                        write_r_reg(r, read_r_reg(r2));
                    }
                    break;
                }

                case BOOST_BINARY(00110010): // LD (nn), A
                    LPRINTF("LD (nn), A)\n");
                    mSys.MemWrite8(read_nn(), mRegs.a);
                    break;
                case BOOST_BINARY(00000010): // LD (BC), A
                    LPRINTF("LD (BC), A)\n");
                    mSys.MemWrite8(READ_BC(), mRegs.a);
                    break;
                case BOOST_BINARY(00010010): // LD (DE), A
                    LPRINTF("LD (DE), A)\n");
                    mSys.MemWrite8(READ_DE(), mRegs.a);
                    break;

                case BOOST_BINARY(00000110):
                case BOOST_BINARY(00001110):
                case BOOST_BINARY(00010110):
                case BOOST_BINARY(00011110):
                case BOOST_BINARY(00100110):
                case BOOST_BINARY(00101110):
                case BOOST_BINARY(00111110): // LD r, n
                    LPRINTF("LD r, n\n");
                    write_r_reg(BITS_SHIFT(op, 5, 3), read_n());
                    break;
                case BOOST_BINARY(00110110): // LD (HL), n
                    LPRINTF("LD (HL), n\n");
                    mSys.MemWrite8(READ_HL(), read_n());
                    break;
                case BOOST_BINARY(00000001):
                case BOOST_BINARY(00010001):
                case BOOST_BINARY(00100001):
                case BOOST_BINARY(00110001): // LD dd, nn
                    LPRINTF("LD dd, nn\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_nn());
                    break;

                case BOOST_BINARY(00100010): // LD (nn), HL
                    LPRINTF("LD (nn), HL\n");
                    temp16 = read_nn();

                    mSys.MemWrite8(temp16, mRegs.l);
                    mSys.MemWrite8(temp16 + 1, mRegs.h);
                    break;
                case BOOST_BINARY(00101010): // LD HL, (nn)
                    LPRINTF("LD HL, (nn)\n");
                    temp16 = read_nn();

                    mRegs.l = mSys.MemRead8(temp16);
                    mRegs.h = mSys.MemRead8(temp16 + 1);
                    break;
                case BOOST_BINARY(00001010): // LD A, (BC)
                    LPRINTF("LD A, (BC)\n");
                    mRegs.a = mSys.MemRead8(READ_BC());
                    break;
                case BOOST_BINARY(00011010): // LD A, (DE)
                    LPRINTF("LD A, (DE)\n");
                    mRegs.a = mSys.MemRead8(READ_DE());
                    break;
                case BOOST_BINARY(11000101):
                case BOOST_BINARY(11010101):
                case BOOST_BINARY(11100101):
                case BOOST_BINARY(11110101): // PUSH qq
                    LPRINTF("PUSH qq\n");
                    push16(read_qq_reg(BITS_SHIFT(op, 5, 4)));
                    break;

                case BOOST_BINARY(11100011): // EX (SP), HL
                    LPRINTF("EX (SP), HL\n");
                    temp16 = pop16();
                    push16(READ_HL());
                    WRITE_HL(temp16);
                    break;

                case BOOST_BINARY(00001000): { // EX AF, AF'
                    LPRINTF("EX AF, AF'\n");

                    uint16_t temp16 = READ_AF();
                    WRITE_AF(READ_AF_ALT());
                    WRITE_AF_ALT(temp16);
                    break;
                }
                case BOOST_BINARY(00001011):
                case BOOST_BINARY(00011011):
                case BOOST_BINARY(00101011):
                case BOOST_BINARY(00111011): // DEC ss
                    LPRINTF("DEC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_dd_reg(dd) - 1);
                    break;
                case BOOST_BINARY(00000011):
                case BOOST_BINARY(00010011):
                case BOOST_BINARY(00100011):
                case BOOST_BINARY(00110011): // INC ss
                    LPRINTF("INC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    write_dd_reg(dd, read_dd_reg(dd) + 1);
                    break;

                case BOOST_BINARY(00000100):
                case BOOST_BINARY(00001100):
                case BOOST_BINARY(00010100):
                case BOOST_BINARY(00011100):
                case BOOST_BINARY(00100100):
                case BOOST_BINARY(00101100):
                case BOOST_BINARY(00111100): { // INC r
                    LPRINTF("INC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;

                    old = read_r_reg(r);
                    temp8 = old + 1;
                    write_r_reg(r, temp8);

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

                case BOOST_BINARY(00000101):
                case BOOST_BINARY(00001101):
                case BOOST_BINARY(00010101):
                case BOOST_BINARY(00011101):
                case BOOST_BINARY(00100101):
                case BOOST_BINARY(00101101):
                case BOOST_BINARY(00111101): { // DEC r
                    LPRINTF("DEC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;

                    old = read_r_reg(r);
                    temp8 = old - 1;
                    write_r_reg(r, temp8);

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

                case BOOST_BINARY(10110000) ... BOOST_BINARY(10110111): // OR r
                    LPRINTF("OR r\n");
                    mRegs.a = mRegs.a | read_r_reg(BITS(op, 2, 0));
                    set_flags(mRegs.a);
                    break;

                case BOOST_BINARY(10101000) ... BOOST_BINARY(10101111): // XOR r
                    LPRINTF("XOR r\n");
                    mRegs.a = mRegs.a ^ read_r_reg(BITS(op, 2, 0));
                    set_flags(mRegs.a);
                    break;

                case BOOST_BINARY(00000111): // RLCA
                    LPRINTF("RLCA\n");
                    temp8 = (mRegs.a << 1) | ((mRegs.a >> 7) & 0x1);
                    set_flag(FLAG_C, mRegs.a & 0x80);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    mRegs.a = temp8;
                    break;

                case BOOST_BINARY(00001111): // RRCA
                    LPRINTF("RRCA\n");
                    temp8 = (mRegs.a >> 1) | ((mRegs.a << 7) & 0x80);
                    set_flag(FLAG_C, mRegs.a & 0x1);
                    set_flag(FLAG_H, 0);
                    set_flag(FLAG_N, 0);
                    mRegs.a = temp8;
                    break;

                case BOOST_BINARY(00111111): // CCF
                    LPRINTF("ccf\n");
                    set_flag(FLAG_C, !(mRegs.f & FLAG_C));
                    set_flag(FLAG_N, 0);
                    break;

                default:
                    fprintf(stderr, "unhandled opcode 0x%hhx\n", op);
                    return -1;
            }
        }

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



