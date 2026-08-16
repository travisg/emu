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

#include <cassert>
#include <cstdio>

#include "bits.h"
#include "system/system.h"
#include "trace.h"
#include "trace_oracle.h"

#define LOCAL_TRACE 0

// TODO: double check that z80 is little endian in all uses of MemReadWrite16
using Endian = System::Endian;

uint16_t CpuZ80::read_qq_reg(int dd) {
    switch (dd) {
        default:
        case 0b00:
            return read_bc();
        case 0b01:
            return read_de();
        case 0b10:
            return read_hl();
        case 0b11:
            return read_af();
    }
}

void CpuZ80::write_qq_reg(int dd, uint16_t val) {
    switch (dd) {
        default:
        case 0b00:
            write_bc(val);
            break;
        case 0b01:
            write_de(val);
            break;
        case 0b10:
            write_hl(val);
            break;
        case 0b11:
            write_af(val);
            break;
    }
}

uint16_t CpuZ80::read_dd_reg(int dd) {
    switch (dd) {
        default:
        case 0b00:
            return read_bc();
        case 0b01:
            return read_de();
        case 0b10:
            return read_hl();
        case 0b11:
            return read_sp();
    }
}

void CpuZ80::write_dd_reg(int dd, uint16_t val) {
    switch (dd) {
        default:
        case 0b00:
            write_bc(val);
            break;
        case 0b01:
            write_de(val);
            break;
        case 0b10:
            write_hl(val);
            break;
        case 0b11:
            write_sp(val);
            break;
    }
}

uint8_t CpuZ80::read_r_reg(int r) {
    if (mPrefixDD) {
        if (r == 0b100) {
            return mRegs.ix >> 8;
        }
        if (r == 0b101) {
            return mRegs.ix & 0xff;
        }
    } else if (mPrefixFD) {
        if (r == 0b100) {
            return mRegs.iy >> 8;
        }
        if (r == 0b101) {
            return mRegs.iy & 0xff;
        }
    }

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
        return mSys.MemRead8(read_hl());
    } else {
        return read_r_reg(r);
    }
}

void CpuZ80::write_r_reg(int r, uint8_t val) {
    if (mPrefixDD) {
        if (r == 0b100) {
            mRegs.ix = (mRegs.ix & 0x00ff) | (val << 8);
            return;
        }
        if (r == 0b101) {
            mRegs.ix = (mRegs.ix & 0xff00) | val;
            return;
        }
    } else if (mPrefixFD) {
        if (r == 0b100) {
            mRegs.iy = (mRegs.iy & 0x00ff) | (val << 8);
            return;
        }
        if (r == 0b101) {
            mRegs.iy = (mRegs.iy & 0xff00) | val;
            return;
        }
    }

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
        mSys.MemWrite8(read_hl(), val);
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
    LPRINTF("OUT 0x%hhx = 0x%hhx\n", addr, val);

    mSys.IOWrite8(addr, val);
}

uint8_t CpuZ80::in(uint8_t addr) {
    uint8_t val = mSys.IORead8(addr);
    LPRINTF("IN 0x%02x = 0x%02x\n", addr, val);

    return val;
}

void CpuZ80::set_flag(Flag flag, bool val) {
    if (val) {
        mRegs.f |= (1 << (int)flag);
    } else {
        mRegs.f &= ~(1 << (int)flag);
    }
}

bool CpuZ80::get_flag(Flag flag) const {
    return !!(mRegs.f & (1 << (int)flag));
}

bool CpuZ80::test_cond(int cond) {
    switch (cond) {
        case 0:
            if (!get_flag(Flag::Z)) {
                return true;
            }
            break; // NZ
        case 1:
            if (get_flag(Flag::Z)) {
                return true;
            }
            break; // Z
        case 2:
            if (!get_flag(Flag::C)) {
                return true;
            }
            break; // NC
        case 3:
            if (get_flag(Flag::C)) {
                return true;
            }
            break; // C
        case 4:
            if (!get_flag(Flag::PV)) {
                return true;
            }
            break; // PO
        case 5:
            if (get_flag(Flag::PV)) {
                return true;
            }
            break; // PE
        case 6:
            if (!get_flag(Flag::S)) {
                return true;
            }
            break; // P
        case 7:
            if (get_flag(Flag::S)) {
                return true;
            }
            break; // M
    }
    return false;
}

/* count even number of bits */
static int calc_parity(uint8_t val) {
    int count;

    count = 0;
    while (val != 0) {
        if (val & 1) {
            count++;
        }
        val >>= 1;
    }

    if ((count & 1) == 0) {
        return 1;
    } else {
        return 0;
    }
}

void CpuZ80::set_z_flag(uint8_t val) {
    set_flag(Flag::Z, val == 0); // zero flag
}

void CpuZ80::set_s_flag(uint8_t val) {
    set_flag(Flag::S, BIT(val, 7) != 0); // sign flag
}

void CpuZ80::set_flags(uint8_t val) {
    set_s_flag(val);
    set_z_flag(val);
    set_flag(Flag::PV, calc_parity(val) != 0);
    set_flag(Flag::H, false);
    set_flag(Flag::N, false);
    set_flag(Flag::C, false);
}

int CpuZ80::Run() {
    LTRACEF("Run\n");

    extern int64_t g_cycle_limit;

    for (;;) {
        if (mSys.isShutdown()) {
            LPRINTF("Z80 Run Loop: system shutdown requested, exiting\n");
            return 0;
        }

        if (g_cycle_limit > 0) {
            g_cycle_limit--;
            if (g_cycle_limit == 0) {
                printf("cycle limit reached, exiting\n");
                return 1;
            }
        }

        // emitted here, ahead of the restart: label, so a prefixed instruction
        // (DD/FD loop back to restart) still traces exactly once
        if (g_trace_file) {
            TraceInstruction();
        }

        uint8_t temp8;
        uint16_t temp16;
        int dd;
        uint8_t op;

        fflush(stdout);

        mPrefixDD = false;
        mPrefixFD = false;

        // debugging to make sure the prefix is consumed
        // instructions that use up the prefix must set this to true
        bool consume_mPrefixDD = false, consume_mPrefixFD = false;

        // see if we need to service an irq
        if (mIRQLevel && mRegs.iff1 != 0) {
            printf("handling IRQ mode %d\n", mRegs.im);
            mRegs.iff1 = 0;
            mRegs.iff2 = 0;
            if (mRegs.im == 1) {
                op = 0b11111111; // rst 0x38
            } else {
                // TODO: handle anything other than interrupt mode 1
                op = 0b11111111; // rst 0x38
            }
            goto decode;
        }

        // fetch the instruction op
restart:
        op = mSys.MemRead8(mRegs.pc++);

decode:
        // look for certain prefixes
        if (op == 0xdd) {
            // ix prefix
            mPrefixDD = true;
            // Question: what happens if more than one prefix is seen in a row?
            goto restart;
        } else if (op == 0xfd) {
            // iy prefix
            mPrefixFD = true;
            goto restart;
        } else if (op == 0xed) {
            // ed prefix is a whole new space
            op = mSys.MemRead8(mRegs.pc++);

            LPRINTF("PC 0x%04hx: op ed%02hhx - ", (uint16_t)(mRegs.pc - 2), op);
            switch (op) {
                case 0b01000000:
                case 0b01001000:
                case 0b01010000:
                case 0b01011000:
                case 0b01100000:
                case 0b01101000:
                case 0b01110000:
                case 0b01111000: // IN r, (C)
                    LPRINTF("IN r, (C)\n");
                    temp8 = in(mRegs.c);
                    {
                        int r = BITS_SHIFT(op, 5, 3);
                        if (r != 0b110) {
                            write_r_reg(r, temp8);
                        }
                        set_s_flag(temp8);
                        set_z_flag(temp8);
                        set_flag(Flag::PV, calc_parity(temp8) != 0);
                        set_flag(Flag::H, false);
                        set_flag(Flag::N, false);
                    }
                    break;
                case 0b01000001:
                case 0b01001001:
                case 0b01010001:
                case 0b01011001:
                case 0b01100001:
                case 0b01101001:
                case 0b01110001: // OUT (C), 0 (undocumented)
                case 0b01111001: // OUT (C), r
                    LPRINTF("OUT (C), r\n");
                    if (BITS_SHIFT(op, 5, 3) == 0b110) { // OUT (c), 0
                        temp8 = 0;
                    } else {
                        temp8 = read_r_reg(BITS_SHIFT(op, 5, 3));
                    }
                    out(mRegs.c, temp8);
                    break;
                case 0b01000010:
                case 0b01010010:
                case 0b01100010:
                case 0b01110010: { // SBC HL, ss
                    LPRINTF("SBC HL, ss\n");
                    uint16_t ss = read_dd_reg(BITS_SHIFT(op, 5, 4));
                    uint16_t hl = read_hl();
                    uint16_t c = get_flag(Flag::C) ? 1 : 0;
                    uint32_t res = (uint32_t)hl - ss - c;

                    set_flag(Flag::C, (int32_t)hl - ss - c < 0);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::PV, ((hl ^ ss) & (hl ^ (uint16_t)res) & 0x8000) != 0);
                    set_flag(Flag::H, ((int32_t)(hl & 0xfff) - (ss & 0xfff) - c) < 0);
                    set_flag(Flag::Z, (uint16_t)res == 0);
                    set_flag(Flag::S, res & 0x8000);
                    write_hl(res);
                    break;
                }
                case 0b01001010:
                case 0b01011010:
                case 0b01101010:
                case 0b01111010: { // ADC HL, ss
                    LPRINTF("ADC HL, ss\n");
                    uint16_t ss = read_dd_reg(BITS_SHIFT(op, 5, 4));
                    uint16_t hl = read_hl();
                    uint16_t c = get_flag(Flag::C) ? 1 : 0;
                    uint32_t res = (uint32_t)hl + ss + c;

                    set_flag(Flag::C, res > 0xffff);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, ((hl ^ (uint16_t)res) & (ss ^ (uint16_t)res) & 0x8000) != 0);
                    set_flag(Flag::H, (hl & 0xfff) + (ss & 0xfff) + c > 0xfff);
                    set_flag(Flag::Z, (uint16_t)res == 0);
                    set_flag(Flag::S, res & 0x8000);
                    write_hl(res);
                    break;
                }
                case 0b10100010: // INI
                    LPRINTF("INI\n");
                    temp8 = in(mRegs.c);
                    mSys.MemWrite8(read_hl(), temp8);
                    write_hl(read_hl() + 1);
                    mRegs.b--;
                    set_flag(Flag::Z, mRegs.b == 0);
                    set_flag(Flag::N, 1);
                    break;
                case 0b10110010: // INIR
                    LPRINTF("INIR\n");
                    temp8 = in(mRegs.c);
                    mSys.MemWrite8(read_hl(), temp8);
                    write_hl(read_hl() + 1);
                    mRegs.b--;
                    set_flag(Flag::Z, 1);
                    set_flag(Flag::N, 1);
                    if (mRegs.b != 0) {
                        mRegs.pc -= 2;
                        set_flag(Flag::Z, 0);
                    }
                    break;
                case 0b10101010: // IND
                    LPRINTF("IND\n");
                    temp8 = in(mRegs.c);
                    mSys.MemWrite8(read_hl(), temp8);
                    write_hl(read_hl() - 1);
                    mRegs.b--;
                    set_flag(Flag::Z, mRegs.b == 0);
                    set_flag(Flag::N, 1);
                    break;
                case 0b10111010: // INDR
                    LPRINTF("INDR\n");
                    temp8 = in(mRegs.c);
                    mSys.MemWrite8(read_hl(), temp8);
                    write_hl(read_hl() - 1);
                    mRegs.b--;
                    set_flag(Flag::Z, 1);
                    set_flag(Flag::N, 1);
                    if (mRegs.b != 0) {
                        mRegs.pc -= 2;
                        set_flag(Flag::Z, 0);
                    }
                    break;
                case 0b10100011: // OUTI
                    LPRINTF("OUTI\n");
                    mRegs.b--;
                    temp8 = mSys.MemRead8(read_hl());
                    out(mRegs.c, temp8);
                    write_hl(read_hl() + 1);
                    set_flag(Flag::Z, mRegs.b == 0);
                    set_flag(Flag::N, 1);
                    break;
                case 0b10110011: // OTIR
                    LPRINTF("OTIR\n");
                    mRegs.b--;
                    temp8 = mSys.MemRead8(read_hl());
                    out(mRegs.c, temp8);
                    write_hl(read_hl() + 1);
                    set_flag(Flag::Z, 1);
                    set_flag(Flag::N, 1);
                    if (mRegs.b != 0) {
                        mRegs.pc -= 2;
                        set_flag(Flag::Z, 0);
                    }
                    break;
                case 0b10101011: // OUTD
                    LPRINTF("OUTD\n");
                    mRegs.b--;
                    temp8 = mSys.MemRead8(read_hl());
                    out(mRegs.c, temp8);
                    write_hl(read_hl() - 1);
                    set_flag(Flag::Z, mRegs.b == 0);
                    set_flag(Flag::N, 1);
                    break;
                case 0b10111011: // OTDR
                    LPRINTF("OTDR\n");
                    mRegs.b--;
                    temp8 = mSys.MemRead8(read_hl());
                    out(mRegs.c, temp8);
                    write_hl(read_hl() - 1);
                    set_flag(Flag::Z, 1);
                    set_flag(Flag::N, 1);
                    if (mRegs.b != 0) {
                        mRegs.pc -= 2;
                        set_flag(Flag::Z, 0);
                    }
                    break;
                case 0xa1: // CPI
                    LPRINTF("CPI\n");
                    {
                        uint8_t val = mSys.MemRead8(read_hl());
                        uint8_t res = mRegs.a - val;
                        uint16_t bc = read_bc() - 1;
                        write_bc(bc);
                        write_hl(read_hl() + 1);
                        set_flag(Flag::S, res & 0x80);
                        set_flag(Flag::Z, res == 0);
                        set_flag(Flag::H, (mRegs.a & 0x0f) < (val & 0x0f));
                        set_flag(Flag::PV, bc != 0);
                        set_flag(Flag::N, 1);
                    }
                    break;
                case 0xb1: // CPIR
                    LPRINTF("CPIR\n");
                    {
                        uint8_t val = mSys.MemRead8(read_hl());
                        uint8_t res = mRegs.a - val;
                        uint16_t bc = read_bc() - 1;
                        write_bc(bc);
                        write_hl(read_hl() + 1);
                        set_flag(Flag::S, res & 0x80);
                        set_flag(Flag::Z, res == 0);
                        set_flag(Flag::H, (mRegs.a & 0x0f) < (val & 0x0f));
                        set_flag(Flag::PV, bc != 0);
                        set_flag(Flag::N, 1);
                        if (bc != 0 && res != 0) {
                            mRegs.pc -= 2;
                        }
                    }
                    break;
                case 0xa9: // CPD
                    LPRINTF("CPD\n");
                    {
                        uint8_t val = mSys.MemRead8(read_hl());
                        uint8_t res = mRegs.a - val;
                        uint16_t bc = read_bc() - 1;
                        write_bc(bc);
                        write_hl(read_hl() - 1);
                        set_flag(Flag::S, res & 0x80);
                        set_flag(Flag::Z, res == 0);
                        set_flag(Flag::H, (mRegs.a & 0x0f) < (val & 0x0f));
                        set_flag(Flag::PV, bc != 0);
                        set_flag(Flag::N, 1);
                    }
                    break;
                case 0xb9: // CPDR
                    LPRINTF("CPDR\n");
                    {
                        uint8_t val = mSys.MemRead8(read_hl());
                        uint8_t res = mRegs.a - val;
                        uint16_t bc = read_bc() - 1;
                        write_bc(bc);
                        write_hl(read_hl() - 1);
                        set_flag(Flag::S, res & 0x80);
                        set_flag(Flag::Z, res == 0);
                        set_flag(Flag::H, (mRegs.a & 0x0f) < (val & 0x0f));
                        set_flag(Flag::PV, bc != 0);
                        set_flag(Flag::N, 1);
                        if (bc != 0 && res != 0) {
                            mRegs.pc -= 2;
                        }
                    }
                    break;
                case 0b10110000: // LDIR
                    LPRINTF("LDIR\n");

                    temp8 = mSys.MemRead8(read_hl());
                    mSys.MemWrite8(read_de(), temp8);

                    LPRINTF("copying from 0x%x to 0x%x value 0x%hhx\n", read_hl(),
                            read_de(), temp8);

                    write_hl(read_hl() + 1);
                    write_de(read_de() + 1);
                    write_bc(read_bc() - 1);
                    if (read_bc() != 0) {
                        mRegs.pc -= 2; // repeat the instruction
                    }

                    set_flag(Flag::H, 0);
                    set_flag(Flag::PV, 0);
                    set_flag(Flag::N, 0);
                    break;
                case 0b01000011:
                case 0b01010011:
                case 0b01100011:
                case 0b01110011: // LD (nn), dd
                    LPRINTF("LD (nn), dd\n");

                    temp16 = read_dd_reg(BITS_SHIFT(op, 5, 4));
                    mSys.MemWrite16(read_nn(), temp16, Endian::LITTLE);
                    break;
                case 0x67: // RRD
                    LPRINTF("RRD\n");
                    {
                        uint8_t hl_val = mSys.MemRead8(read_hl());
                        uint8_t a_low = mRegs.a & 0x0f;
                        uint8_t hl_low = hl_val & 0x0f;
                        uint8_t hl_high = hl_val >> 4;

                        mSys.MemWrite8(read_hl(), (a_low << 4) | hl_high);
                        mRegs.a = (mRegs.a & 0xf0) | hl_low;

                        set_s_flag(mRegs.a);
                        set_z_flag(mRegs.a);
                        set_flag(Flag::H, 0);
                        set_flag(Flag::PV, calc_parity(mRegs.a));
                        set_flag(Flag::N, 0);
                    }
                    break;
                case 0x6f: // RLD
                    LPRINTF("RLD\n");
                    {
                        uint8_t hl_val = mSys.MemRead8(read_hl());
                        uint8_t a_low = mRegs.a & 0x0f;
                        uint8_t hl_low = hl_val & 0x0f;
                        uint8_t hl_high = hl_val >> 4;

                        mSys.MemWrite8(read_hl(), (hl_low << 4) | a_low);
                        mRegs.a = (mRegs.a & 0xf0) | hl_high;

                        set_s_flag(mRegs.a);
                        set_z_flag(mRegs.a);
                        set_flag(Flag::H, 0);
                        set_flag(Flag::PV, calc_parity(mRegs.a));
                        set_flag(Flag::N, 0);
                    }
                    break;
                case 0x44: // NEG
                    LPRINTF("NEG\n");
                    {
                        uint8_t old = mRegs.a;
                        uint8_t res = 0 - old;
                        set_flag(Flag::S, res & 0x80);
                        set_flag(Flag::Z, res == 0);
                        set_flag(Flag::H, (old & 0xf) != 0);
                        set_flag(Flag::PV, old == 0x80);
                        set_flag(Flag::N, 1);
                        set_flag(Flag::C, old != 0);
                        mRegs.a = res;
                    }
                    break;
                case 0b01001011:
                case 0b01011011:
                case 0b01101011:
                case 0b01111011: // LD dd, (nn)
                    LPRINTF("LD dd, (nn)\n");

                    temp16 = mSys.MemRead16(read_nn(), Endian::LITTLE);
                    write_dd_reg(BITS_SHIFT(op, 5, 4), temp16);
                    break;

                // TODO: actually do something about this
                case 0x47: // LD I, A
                    LPRINTF("LD I, A\n");
                    mRegs.i = mRegs.a;
                    break;
                case 0x57: // LD A, I
                    LPRINTF("LD A, I\n");
                    mRegs.a = mRegs.i;
                    set_flag(Flag::PV, mRegs.iff2);
                    set_s_flag(mRegs.a);
                    set_z_flag(mRegs.a);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    break;
                case 0x4f: // LD R, A
                    LPRINTF("LD R, A\n");
                    mRegs.r = mRegs.a;
                    break;
                case 0x5f: // LD A, R
                    LPRINTF("LD A, R\n");
                    mRegs.a = mRegs.r;
                    set_flag(Flag::PV, mRegs.iff2);
                    set_s_flag(mRegs.a);
                    set_z_flag(mRegs.a);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    break;

                case 0b01000110: // IM 0
                case 0b01001110: // IM 0 (alternate)
                    LPRINTF("IM 0\n");
                    mRegs.im = 0;
                    break;
                case 0b01010110: // IM 1
                case 0b01011110: // IM 1 (alternate)
                    LPRINTF("IM 1\n");
                    mRegs.im = 1;
                    break;
                case 0b01100110: // IM 2
                case 0b01101110: // IM 2 (alternate)
                    LPRINTF("IM 2\n");
                    mRegs.im = 2;
                    break;

                case 0b01001101: // RETI
                    LPRINTF("RETI\n");
                    mRegs.pc = pop16();
                    // TODO: not exactly right, acts like a RET right now
                    break;

                default:
                    fflush(stdout);
                    fprintf(stderr, "unhandled ED prefixed-opcode 0x%hhx\n", op);
                    return -1;
            }
        } else if (op == 0xcb) {
            // cb prefix are for bit instructions
            int8_t d = 0;
            if (mPrefixDD || mPrefixFD) {
                d = (int8_t)mSys.MemRead8(mRegs.pc++);
            }
            op = mSys.MemRead8(mRegs.pc++);

            LPRINTF("PC 0x%04hx: op cb%02hhx - ", (uint16_t)(mRegs.pc - 2), op);
            switch (op) {
                case 0b00000000 ... 0b00000111: { // RLC r
                    int r = BITS(op, 2, 0);
                    LPRINTF("RLC r\n");
                    if (mPrefixDD) {
                        temp16 = read_ix() + d;
                        temp8 = mSys.MemRead8(temp16);
                        uint8_t res = (temp8 << 1) | (temp8 >> 7);
                        mSys.MemWrite8(temp16, res);
                        if (r != 0b110) {
                            write_r_reg(r, res); // undocumented
                        }
                        set_flag(Flag::C, temp8 & 0x80);
                        set_flag(Flag::H, 0);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::PV, calc_parity(res));
                        set_z_flag(res);
                        set_s_flag(res);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        temp16 = read_iy() + d;
                        temp8 = mSys.MemRead8(temp16);
                        uint8_t res = (temp8 << 1) | (temp8 >> 7);
                        mSys.MemWrite8(temp16, res);
                        if (r != 0b110) {
                            write_r_reg(r, res); // undocumented
                        }
                        set_flag(Flag::C, temp8 & 0x80);
                        set_flag(Flag::H, 0);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::PV, calc_parity(res));
                        set_z_flag(res);
                        set_s_flag(res);
                        consume_mPrefixFD = true;
                    } else {
                        temp8 = read_r_reg_or_hl(r);
                        uint8_t res = (temp8 << 1) | (temp8 >> 7);
                        write_r_reg_or_hl(r, res);
                        set_flag(Flag::C, temp8 & 0x80);
                        set_flag(Flag::H, 0);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::PV, calc_parity(res));
                        set_z_flag(res);
                        set_s_flag(res);
                    }
                    break;
                }
                case 0b00001000 ... 0b00001111: { // RRC r
                    int r = BITS(op, 2, 0);
                    LPRINTF("RRC r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = (temp8 >> 1) | (temp8 << 7);
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x01);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00010000 ... 0b00010111: { // RL r
                    int r = BITS(op, 2, 0);
                    LPRINTF("RL r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = (temp8 << 1) | (get_flag(Flag::C) ? 1 : 0);
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x80);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00011000 ... 0b00011111: { // RR r
                    int r = BITS(op, 2, 0);
                    LPRINTF("RR r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = (temp8 >> 1) | (get_flag(Flag::C) ? 0x80 : 0);
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x01);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00100000 ... 0b00100111: { // SLA r
                    int r = BITS(op, 2, 0);
                    LPRINTF("SLA r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = temp8 << 1;
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x80);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00101000 ... 0b00101111: { // SRA r
                    int r = BITS(op, 2, 0);
                    LPRINTF("SRA r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = (temp8 >> 1) | (temp8 & 0x80);
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x01);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00110000 ... 0b00110111: { // SLL r (undocumented)
                    int r = BITS(op, 2, 0);
                    LPRINTF("SLL r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = (temp8 << 1) | 0x01;
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x80);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0b00111000 ... 0b00111111: { // SRL r
                    int r = BITS(op, 2, 0);
                    LPRINTF("SRL r\n");
                    temp8 = read_r_reg_or_hl(r);
                    uint8_t res = temp8 >> 1;
                    write_r_reg_or_hl(r, res);
                    set_flag(Flag::C, temp8 & 0x01);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, calc_parity(res));
                    set_z_flag(res);
                    set_s_flag(res);
                    break;
                }
                case 0x40 ... 0x7f: { // BIT n, r
                    uint8_t bit = BITS_SHIFT(op, 5, 3);
                    int r = BITS(op, 2, 0);
                    LPRINTF("BIT %u, r\n", bit);
                    uint8_t val;
                    if (mPrefixDD) {
                        val = mSys.MemRead8(read_ix() + d);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        val = mSys.MemRead8(read_iy() + d);
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    set_flag(Flag::Z, (val & (1 << bit)) == 0);
                    set_flag(Flag::H, 1);
                    set_flag(Flag::N, 0);
                    // S and PV are usually unaffected or undefined, but standard Z80 sets them
                    set_flag(Flag::S, (bit == 7) && (val & 0x80));
                    break;
                }
                case 0x80 ... 0xbf: { // RES n, r
                    uint8_t bit = BITS_SHIFT(op, 5, 3);
                    int r = BITS(op, 2, 0);
                    LPRINTF("RES %u, r\n", bit);
                    if (mPrefixDD) {
                        temp16 = read_ix() + d;
                        temp8 = mSys.MemRead8(temp16) & ~(1 << bit);
                        mSys.MemWrite8(temp16, temp8);
                        if (r != 0b110) {
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        temp16 = read_iy() + d;
                        temp8 = mSys.MemRead8(temp16) & ~(1 << bit);
                        mSys.MemWrite8(temp16, temp8);
                        if (r != 0b110) {
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixFD = true;
                    } else {
                        write_r_reg_or_hl(r, read_r_reg_or_hl(r) & ~(1 << bit));
                    }
                    break;
                }
                case 0xc0 ... 0xff: { // SET n, r
                    uint8_t bit = BITS_SHIFT(op, 5, 3);
                    int r = BITS(op, 2, 0);
                    LPRINTF("SET %u, r\n", bit);
                    if (mPrefixDD) {
                        temp16 = read_ix() + d;
                        temp8 = mSys.MemRead8(temp16) | (1 << bit);
                        mSys.MemWrite8(temp16, temp8);
                        if (r != 0b110) {
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        temp16 = read_iy() + d;
                        temp8 = mSys.MemRead8(temp16) | (1 << bit);
                        mSys.MemWrite8(temp16, temp8);
                        if (r != 0b110) {
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixFD = true;
                    } else {
                        write_r_reg_or_hl(r, read_r_reg_or_hl(r) | (1 << bit));
                    }
                    break;
                }
                default:
                    fflush(stdout);
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
                case 0b11101001: // JP (HL)
                    LPRINTF("JP (HL)\n");
                    if (mPrefixDD) {
                        mRegs.pc = read_ix();
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        mRegs.pc = read_iy();
                        consume_mPrefixFD = true;
                    } else {
                        mRegs.pc = read_hl();
                    }
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

                    if (test_cond(cond)) {
                        mRegs.pc = temp16;
                    }
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
                    if (get_flag(Flag::C)) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case 0b00110000: { // JR NC, e
                    LPRINTF("JR NC, e\n");
                    int8_t rel = read_n();
                    if (!get_flag(Flag::C)) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case 0b00101000: { // JR Z, e
                    LPRINTF("JR Z, e\n");
                    int8_t rel = read_n();
                    if (get_flag(Flag::Z)) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case 0b00100000: { // JR NZ, e
                    LPRINTF("JR NZ, e\n");
                    int8_t rel = read_n();
                    if (!get_flag(Flag::Z)) {
                        mRegs.pc += rel;
                    }
                    break;
                }
                case 0b11110011: // DI
                    LPRINTF("DI\n");
                    mRegs.iff1 = 0;
                    mRegs.iff2 = 0;
                    break;
                case 0b11111011: // EI
                    LPRINTF("EI\n");
                    mRegs.iff1 = 1;
                    mRegs.iff2 = 1;
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

                    int r = BITS_SHIFT(op, 5, 3);
                    int r2 = BITS_SHIFT(op, 2, 0);

                    if (r == r2 && r == 0b110) { // HALT
                        LPRINTF("HALT (treating as NOP)\n");
                        break;
                    } else if (mPrefixDD && r2 == 0b110) {
                        LPRINTF("LD r, (IX+d)\n");
                        temp16 = read_ix() + read_n();
                        temp8 = mSys.MemRead8(temp16);
                        if (r != 0b110) { // no (HL) form
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r2 == 0b110) {
                        LPRINTF("LD r, (IY+d)\n");
                        temp16 = read_iy() + read_n();
                        temp8 = mSys.MemRead8(temp16);
                        if (r != 0b110) { // no (HL) form
                            write_r_reg(r, temp8);
                        }
                        consume_mPrefixFD = true;
                    } else if (mPrefixDD && r == 0b110) {
                        LPRINTF("LD (IX+d), r\n");
                        temp16 = read_ix() + (int8_t)read_n();
                        mSys.MemWrite8(temp16, read_r_reg(r2));
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        LPRINTF("LD (IY+d), r\n");
                        temp16 = read_iy() + (int8_t)read_n();
                        mSys.MemWrite8(temp16, read_r_reg(r2));
                        consume_mPrefixFD = true;
                    } else {
                        LPRINTF("LD r, r\n");
                        write_r_reg_or_hl(r, read_r_reg_or_hl(r2));
                    }

                    break;
                }

                case 0b00110010: // LD (nn), A
                    LPRINTF("LD (nn), A)\n");
                    mSys.MemWrite8(read_nn(), mRegs.a);
                    break;
                case 0b00000010: // LD (BC), A
                    LPRINTF("LD (BC), A)\n");
                    mSys.MemWrite8(read_bc(), mRegs.a);
                    break;
                case 0b00010010: // LD (DE), A
                    LPRINTF("LD (DE), A)\n");
                    mSys.MemWrite8(read_de(), mRegs.a);
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
                    if (mPrefixDD) {
                        temp16 = read_ix() + (int8_t)read_n();
                        mSys.MemWrite8(temp16, read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        temp16 = read_iy() + (int8_t)read_n();
                        mSys.MemWrite8(temp16, read_n());
                        consume_mPrefixFD = true;
                    } else {
                        mSys.MemWrite8(read_hl(), read_n());
                    }
                    break;
                case 0b00000001:
                case 0b00010001:
                case 0b00100001:
                case 0b00110001: // LD dd, nn
                    LPRINTF("LD dd, nn\n");
                    if (mPrefixDD && op == 0x21) {
                        write_ix(read_nn());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && op == 0x21) {
                        write_iy(read_nn());
                        consume_mPrefixFD = true;
                    } else {
                        dd = BITS_SHIFT(op, 5, 4);
                        write_dd_reg(dd, read_nn());
                    }
                    break;

                case 0b11111001: // LD SP, HL
                    LPRINTF("LD SP, HL\n");
                    if (mPrefixDD) {
                        write_sp(read_ix());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        write_sp(read_iy());
                        consume_mPrefixFD = true;
                    } else {
                        write_sp(read_hl());
                    }
                    break;

                case 0b00100010: // LD (nn), HL
                    LPRINTF("LD (nn), HL\n");
                    temp16 = read_nn();
                    if (mPrefixDD) {
                        mSys.MemWrite16(temp16, read_ix(), Endian::LITTLE);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        mSys.MemWrite16(temp16, read_iy(), Endian::LITTLE);
                        consume_mPrefixFD = true;
                    } else {
                        mSys.MemWrite8(temp16, mRegs.l);
                        mSys.MemWrite8(temp16 + 1, mRegs.h);
                    }
                    break;
                case 0b00101010: // LD HL, (nn)
                    LPRINTF("LD HL, (nn)\n");
                    temp16 = read_nn();
                    if (mPrefixDD) {
                        write_ix(mSys.MemRead16(temp16, Endian::LITTLE));
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        write_iy(mSys.MemRead16(temp16, Endian::LITTLE));
                        consume_mPrefixFD = true;
                    } else {
                        mRegs.l = mSys.MemRead8(temp16);
                        mRegs.h = mSys.MemRead8(temp16 + 1);
                    }
                    break;
                case 0b00111010: // LD A, (nn)
                    LPRINTF("LD A, (nn)\n");
                    temp16 = read_nn();

                    mRegs.a = mSys.MemRead8(temp16);
                    break;
                case 0b00001010: // LD A, (BC)
                    LPRINTF("LD A, (BC)\n");
                    mRegs.a = mSys.MemRead8(read_bc());
                    break;
                case 0b00011010: // LD A, (DE)
                    LPRINTF("LD A, (DE)\n");
                    mRegs.a = mSys.MemRead8(read_de());
                    break;
                case 0b11000101:
                case 0b11010101:
                case 0b11100101:
                case 0b11110101: // PUSH qq
                    LPRINTF("PUSH qq\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    if (mPrefixDD && dd == 0b10) {
                        push16(read_ix());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && dd == 0b10) {
                        push16(read_iy());
                        consume_mPrefixFD = true;
                    } else {
                        push16(read_qq_reg(dd));
                    }
                    break;

                case 0b11000001:
                case 0b11010001:
                case 0b11100001:
                case 0b11110001: // POP qq
                    LPRINTF("POP qq\n");
                    temp16 = pop16();
                    dd = BITS_SHIFT(op, 5, 4);
                    if (mPrefixDD && dd == 0b10) {
                        write_ix(temp16);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && dd == 0b10) {
                        write_iy(temp16);
                        consume_mPrefixFD = true;
                    } else {
                        write_qq_reg(dd, temp16);
                    }
                    break;

                case 0b11100011: // EX (SP), HL
                    LPRINTF("EX (SP), HL\n");
                    temp16 = pop16();
                    if (mPrefixDD) {
                        push16(read_ix());
                        write_ix(temp16);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        push16(read_iy());
                        write_iy(temp16);
                        consume_mPrefixFD = true;
                    } else {
                        push16(read_hl());
                        write_hl(temp16);
                    }
                    break;
                case 0b11000111:
                case 0b11001111:
                case 0b11010111:
                case 0b11011111:
                case 0b11100111:
                case 0b11101111:
                case 0b11110111:
                case 0b11111111: { // RST p
                    int p = BITS_SHIFT(op, 5, 3);
                    LPRINTF("RST %02xh\n", p * 8);
                    push_pc();
                    mRegs.pc = (uint16_t)p * 8;
                    break;
                }

                case 0b11101011: // EX DE, HL
                    LPRINTF("EX DE, HL\n");

                    temp16 = read_de();
                    write_de(read_hl());
                    write_hl(temp16);
                    break;

                case 0b00001000: // EX AF, AF'
                    LPRINTF("EX AF, AF'\n");

                    temp16 = read_af();
                    write_af(read_af_alt());
                    write_af_alt(temp16);
                    break;

                case 0b00001001:
                case 0b00011001:
                case 0b00101001:
                case 0b00111001: { // ADD HL, ss
                    LPRINTF("ADD HL, ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    uint16_t ss = read_dd_reg(dd);
                    if (mPrefixDD) {
                        uint16_t ix = read_ix();
                        if (dd == 0b10) {
                            ss = ix; // ADD IX, IX
                        }
                        uint32_t res = (uint32_t)ix + ss;
                        set_flag(Flag::C, res > 0xffff);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::H, (ix & 0xfff) + (ss & 0xfff) > 0xfff);
                        write_ix((uint16_t)res);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD) {
                        uint16_t iy = read_iy();
                        if (dd == 0b10) {
                            ss = iy; // ADD IY, IY
                        }
                        uint32_t res = (uint32_t)iy + ss;
                        set_flag(Flag::C, res > 0xffff);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::H, (iy & 0xfff) + (ss & 0xfff) > 0xfff);
                        write_iy((uint16_t)res);
                        consume_mPrefixFD = true;
                    } else {
                        uint16_t hl = read_hl();
                        uint32_t res = (uint32_t)hl + ss;
                        set_flag(Flag::C, res > 0xffff);
                        set_flag(Flag::N, 0);
                        set_flag(Flag::H, (hl & 0xfff) + (ss & 0xfff) > 0xfff);
                        write_hl((uint16_t)res);
                    }
                    break;
                }
                case 0b00000011:
                case 0b00010011:
                case 0b00100011:
                case 0b00110011: // INC ss
                    LPRINTF("INC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    if (mPrefixDD && dd == 0b10) {
                        write_ix(read_ix() + 1);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && dd == 0b10) {
                        write_iy(read_iy() + 1);
                        consume_mPrefixFD = true;
                    } else {
                        write_dd_reg(dd, read_dd_reg(dd) + 1);
                    }
                    break;
                case 0b00001011:
                case 0b00011011:
                case 0b00101011:
                case 0b00111011: // DEC ss
                    LPRINTF("DEC ss\n");
                    dd = BITS_SHIFT(op, 5, 4);
                    if (mPrefixDD && dd == 0b10) {
                        write_ix(read_ix() - 1);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && dd == 0b10) {
                        write_iy(read_iy() - 1);
                        consume_mPrefixFD = true;
                    } else {
                        write_dd_reg(dd, read_dd_reg(dd) - 1);
                    }
                    break;

                    // 8 bit alu

                case 0b00100100:
                case 0b00101100:
                case 0b00110100:
                case 0b00111100:
                case 0b00000100:
                case 0b00001100:
                case 0b00010100:
                case 0b00011100: { // INC r
                    LPRINTF("INC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;
                    if (mPrefixDD && r == 0b110) {
                        temp16 = read_ix() + (int8_t)read_n();
                        old = mSys.MemRead8(temp16);
                        temp8 = old + 1;
                        mSys.MemWrite8(temp16, temp8);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        temp16 = read_iy() + (int8_t)read_n();
                        old = mSys.MemRead8(temp16);
                        temp8 = old + 1;
                        mSys.MemWrite8(temp16, temp8);
                        consume_mPrefixFD = true;
                    } else {
                        old = read_r_reg_or_hl(r);
                        temp8 = old + 1;
                        write_r_reg_or_hl(r, temp8);
                    }
                    set_flag(Flag::PV, old == 0x7f);
                    set_s_flag(temp8);
                    set_z_flag(temp8);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::H, (old & 0x0f) == 0x0f);
                    break;
                }

                case 0b00000101:
                case 0b00001101:
                case 0b00010101:
                case 0b00011101:
                case 0b00100101:
                case 0b00101101:
                case 0b00110101:
                case 0b00111101: { // DEC r
                    LPRINTF("DEC r\n");
                    int r = BITS_SHIFT(op, 5, 3);
                    uint8_t old;
                    if (mPrefixDD && r == 0b110) {
                        temp16 = read_ix() + (int8_t)read_n();
                        old = mSys.MemRead8(temp16);
                        temp8 = old - 1;
                        mSys.MemWrite8(temp16, temp8);
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        temp16 = read_iy() + (int8_t)read_n();
                        old = mSys.MemRead8(temp16);
                        temp8 = old - 1;
                        mSys.MemWrite8(temp16, temp8);
                        consume_mPrefixFD = true;
                    } else {
                        old = read_r_reg_or_hl(r);
                        temp8 = old - 1;
                        write_r_reg_or_hl(r, temp8);
                    }
                    set_flag(Flag::PV, old == 0x80);
                    set_s_flag(temp8);
                    set_z_flag(temp8);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::H, (old & 0x0f) == 0);
                    break;
                }

                case 0b10000000 ... 0b10000111: { // ADD A, r or ADD A, (HL)
                    LPRINTF("ADD A, r\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    temp8 = mRegs.a + val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) + (val & 0xf) > 0xf);
                    set_flag(Flag::PV, ((mRegs.a ^ temp8) & (val ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::C, (uint16_t)mRegs.a + val > 0xff);
                    mRegs.a = temp8;
                    break;
                }

                case 0b10001000 ... 0b10001111: { // ADC A, r / ADC A, (HL)
                    LPRINTF("ADC A, r/(HL)\n");
                    int r = BITS(op, 2, 0);
                    uint8_t b;
                    if (mPrefixDD && r == 0b110) {
                        b = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        b = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        b = read_r_reg_or_hl(r);
                    }
                    uint8_t c = get_flag(Flag::C) ? 1 : 0;

                    uint8_t res = mRegs.a + b + c;

                    set_flag(Flag::C, (uint16_t)mRegs.a + b + c > 0xff);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, ((mRegs.a ^ res) & (b ^ res) & 0x80) != 0);
                    set_flag(Flag::H, (mRegs.a ^ res ^ b) & (1 << 4));
                    set_s_flag(res);
                    set_z_flag(res);

                    mRegs.a = res;
                    break;
                }

                case 0b10010000 ... 0b10010111: { // SUB r, SUB (HL)
                    LPRINTF("SUB r\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    temp8 = mRegs.a - val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) < (val & 0xf));
                    set_flag(Flag::PV, ((mRegs.a ^ val) & (mRegs.a ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::C, mRegs.a < val);
                    mRegs.a = temp8;
                    break;
                }

                case 0b10011000 ... 0b10011111: { // SBC A, r / SBC A, (HL)
                    LPRINTF("SBC A, r/(HL)\n");
                    int r = BITS(op, 2, 0);
                    uint8_t b;
                    if (mPrefixDD && r == 0b110) {
                        b = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        b = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        b = read_r_reg_or_hl(r);
                    }
                    uint8_t c = get_flag(Flag::C) ? 1 : 0;

                    uint8_t res = mRegs.a - b - c;

                    set_flag(Flag::C, (int)mRegs.a < (int)b + (int)c);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::PV, ((mRegs.a ^ b) & (mRegs.a ^ res) & 0x80) != 0);
                    set_flag(Flag::H, (mRegs.a ^ res ^ b) & (1 << 4));
                    set_s_flag(res);
                    set_z_flag(res);

                    mRegs.a = res;
                    break;
                }

                case 0b10100000 ... 0b10100111: { // AND r, AND (HL)
                    LPRINTF("AND r/(HL)\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    mRegs.a &= val;
                    set_flags(mRegs.a);
                    set_flag(Flag::H, 1);
                    break;
                }

                case 0b11100110: // AND n
                    LPRINTF("AND n\n");
                    mRegs.a &= read_n();
                    set_flags(mRegs.a);
                    set_flag(Flag::H, 1);
                    // TODO: check on overflow
                    break;

                case 0b10110000 ... 0b10110111: { // OR r, OR (HL)
                    LPRINTF("OR r/(HL)\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    mRegs.a |= val;
                    set_flags(mRegs.a);
                    break;
                }

                case 0b11110110: // OR n
                    LPRINTF("OR n\n");
                    mRegs.a |= read_n();
                    set_flags(mRegs.a);

                    break;

                case 0b10101000 ... 0b10101111: { // XOR r, XOR (HL)
                    LPRINTF("XOR r/(HL)\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    mRegs.a ^= val;
                    set_flags(mRegs.a);
                    break;
                }
                case 0b11101110: // XOR n
                    LPRINTF("XOR n\n");
                    mRegs.a ^= read_n();
                    set_flags(mRegs.a);
                    break;

                case 0b10111000 ... 0b10111111: { // CP r, CP (HL)
                    LPRINTF("CP r\n");
                    int r = BITS(op, 2, 0);
                    uint8_t val;
                    if (mPrefixDD && r == 0b110) {
                        val = mSys.MemRead8(read_ix() + (int8_t)read_n());
                        consume_mPrefixDD = true;
                    } else if (mPrefixFD && r == 0b110) {
                        val = mSys.MemRead8(read_iy() + (int8_t)read_n());
                        consume_mPrefixFD = true;
                    } else {
                        val = read_r_reg_or_hl(r);
                    }
                    temp8 = mRegs.a - val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) < (val & 0xf));
                    set_flag(Flag::PV, ((mRegs.a ^ val) & (mRegs.a ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::C, mRegs.a < val);
                    break;
                }

                case 0b11000110: { // ADD A, n
                    LPRINTF("ADD A, n\n");
                    uint8_t val = read_n();
                    temp8 = mRegs.a + val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) + (val & 0xf) > 0xf);
                    set_flag(Flag::PV, ((mRegs.a ^ temp8) & (val ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::C, (uint16_t)mRegs.a + val > 0xff);
                    mRegs.a = temp8;
                    break;
                }
                case 0b11001110: { // ADC A, n
                    LPRINTF("ADC A, n\n");
                    uint8_t val = read_n();
                    uint8_t c = get_flag(Flag::C) ? 1 : 0;
                    uint8_t res = mRegs.a + val + c;

                    set_flag(Flag::C, (uint16_t)mRegs.a + val + c > 0xff);
                    set_flag(Flag::N, 0);
                    set_flag(Flag::PV, ((mRegs.a ^ res) & (val ^ res) & 0x80) != 0);
                    set_flag(Flag::H, (mRegs.a ^ res ^ val) & (1 << 4));
                    set_s_flag(res);
                    set_z_flag(res);

                    mRegs.a = res;
                    break;
                }

                case 0b11010110: { // SUB n
                    LPRINTF("SUB n\n");
                    uint8_t val = read_n();
                    temp8 = mRegs.a - val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) < (val & 0xf));
                    set_flag(Flag::PV, ((mRegs.a ^ val) & (mRegs.a ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::C, mRegs.a < val);
                    mRegs.a = temp8;
                    break;
                }
                case 0b11011110: { // SBC A, n
                    LPRINTF("SBC A, n\n");
                    uint8_t val = read_n();
                    uint8_t c = get_flag(Flag::C) ? 1 : 0;
                    uint8_t res = mRegs.a - val - c;

                    set_flag(Flag::C, (int)mRegs.a < (int)val + (int)c);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::PV, ((mRegs.a ^ val) & (mRegs.a ^ res) & 0x80) != 0);
                    set_flag(Flag::H, (mRegs.a ^ res ^ val) & (1 << 4));
                    set_s_flag(res);
                    set_z_flag(res);

                    mRegs.a = res;
                    break;
                }

                case 0b11111110: { // CP n
                    LPRINTF("CP n\n");
                    uint8_t val = read_n();
                    temp8 = mRegs.a - val;
                    set_flag(Flag::S, temp8 & 0x80);
                    set_flag(Flag::Z, temp8 == 0);
                    set_flag(Flag::H, (mRegs.a & 0xf) < (val & 0xf));
                    set_flag(Flag::PV, ((mRegs.a ^ val) & (mRegs.a ^ temp8) & 0x80) != 0);
                    set_flag(Flag::N, 1);
                    set_flag(Flag::C, mRegs.a < val);
                    break;
                }

                case 0b00000111: // RLCA
                    LPRINTF("RLCA\n");
                    temp8 = (mRegs.a << 1) | ((mRegs.a >> 7) & 0x1);
                    set_flag(Flag::C, mRegs.a & 0x80);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00001111: // RRCA
                    LPRINTF("RRCA\n");
                    temp8 = (mRegs.a >> 1) | ((mRegs.a << 7) & 0x80);
                    set_flag(Flag::C, mRegs.a & 0x1);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00010111: // RLA
                    LPRINTF("RLA\n");
                    temp8 = (mRegs.a << 1) | (get_flag(Flag::C) ? 1 : 0);
                    set_flag(Flag::C, mRegs.a & 0x80);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    mRegs.a = temp8;
                    break;
                case 0b00011111: // RRA
                    LPRINTF("RRA\n");
                    temp8 = (mRegs.a >> 1);
                    temp8 |= get_flag(Flag::C) ? (1 << 7) : 0;
                    set_flag(Flag::C, mRegs.a & 0x1);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    mRegs.a = temp8;
                    break;

                case 0b00100111: { // DAA
                    LPRINTF("DAA\n");
                    uint8_t a = mRegs.a;
                    uint8_t correction = 0;
                    bool carry = get_flag(Flag::C);
                    bool h_carry = get_flag(Flag::H);

                    if (h_carry || (a & 0x0F) > 9) {
                        correction |= 0x06;
                    }

                    if (carry || a > 0x99) {
                        correction |= 0x60;
                        carry = true;
                    }

                    if (get_flag(Flag::N)) {
                        mRegs.a -= correction;
                    } else {
                        mRegs.a += correction;
                    }

                    set_flag(Flag::C, carry);
                    set_flag(Flag::H, get_flag(Flag::N) ? (h_carry && (a & 0x0F) < 6) : ((a & 0x0F) > 9));
                    set_s_flag(mRegs.a);
                    set_z_flag(mRegs.a);
                    set_flag(Flag::PV, calc_parity(mRegs.a));
                    break;
                }
                case 0b00101111: // CPL
                    LPRINTF("CPL\n");
                    mRegs.a = ~mRegs.a;
                    set_flag(Flag::H, 1);
                    set_flag(Flag::N, 1);
                    break;

                case 0b00111111: // CCF
                    LPRINTF("CCF\n");
                    set_flag(Flag::H, get_flag(Flag::C));
                    set_flag(Flag::C, !get_flag(Flag::C));
                    set_flag(Flag::N, 0);
                    break;

                case 0b00110111: // SCF
                    LPRINTF("SCF\n");
                    set_flag(Flag::C, 1);
                    set_flag(Flag::H, 0);
                    set_flag(Flag::N, 0);
                    break;

                case 0b11011001: // EXX
                    LPRINTF("EXX\n");
                    std::swap(mRegs.b, mRegs.b_alt);
                    std::swap(mRegs.c, mRegs.c_alt);
                    std::swap(mRegs.d, mRegs.d_alt);
                    std::swap(mRegs.e, mRegs.e_alt);
                    std::swap(mRegs.h, mRegs.h_alt);
                    std::swap(mRegs.l, mRegs.l_alt);
                    break;

                default:
                    fflush(stdout);
                    fprintf(stderr, "unhandled opcode 0x%hhx\n", op);
                    return -1;
            }
        }

        // instruction is completed, make sure we 'consumed' the dd or fd prefix
        if (mPrefixDD && !consume_mPrefixDD) {
            fflush(stdout);
            fprintf(stderr, "unhandled opcode dd prefix\n");
            return -1;
        }
        if (mPrefixFD && !consume_mPrefixFD) {
            fflush(stdout);
            fprintf(stderr, "unhandled opcode fd prefix\n");
            return -1;
        }

        // dump the state of the cpu if tracing is on
        if (LOCAL_TRACE) {
            Dump();
        }
    }

    return 0;
}

void CpuZ80::RaiseIRQ() {
    mIRQLevel = true;
}

void CpuZ80::RaiseNMI() {
    mNMILevel = true;
}

void CpuZ80::LowerIRQ() {
    mIRQLevel = false;
}

void CpuZ80::LowerNMI() {
    mNMILevel = false;
}

void CpuZ80::TraceInstruction() {
    fprintf(g_trace_file, "PC=%04x AF=%02x%02x BC=%02x%02x DE=%02x%02x HL=%02x%02x IX=%04x IY=%04x SP=%04x\n",
            mRegs.pc, mRegs.a, mRegs.f, mRegs.b, mRegs.c, mRegs.d, mRegs.e, mRegs.h, mRegs.l,
            read_ix(), read_iy(), read_sp());
}

void CpuZ80::Dump() {
    printf("f 0x%02hhx (%c%c%c%c%c%c) a 0x%02hhx b 0x%02hhx c 0x%02hhx d 0x%02hhx e 0x%02hhx h 0x%02hhx l 0x%02hhx ",
           mRegs.f, get_flag(Flag::C) ? 'c' : ' ', get_flag(Flag::N) ? 'n' : ' ', get_flag(Flag::PV) ? 'p' : ' ',
           get_flag(Flag::H) ? 'h' : ' ', get_flag(Flag::Z) ? 'z' : ' ', get_flag(Flag::S) ? 's' : ' ',
           mRegs.a, mRegs.b, mRegs.c, mRegs.d, mRegs.e, mRegs.h, mRegs.l);
    printf("sp 0x%04hx ix 0x%04hx iy 0x%04hx pc 0x%04hx\n",
           read_sp(), read_ix(), read_iy(), mRegs.pc);
}

void CpuZ80::Reset() {
    LTRACEF("Reset\n");
    mRegs = {};
}
