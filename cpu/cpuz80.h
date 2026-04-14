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
#pragma once

#include <atomic>
#include <cstdint>

#include "cpu.h"

class CpuZ80 final : public Cpu {
  public:
    explicit CpuZ80(System &sys) : Cpu(sys) {};

    virtual void Reset() override;
    virtual int Run() override;

    virtual void Dump() override;

    void RaiseIRQ();
    void RaiseNMI();
    void LowerIRQ();
    void LowerNMI();

    uint16_t GetPC() { return mRegs.pc; }

  private:
    // internal routines
    uint16_t read_qq_reg(int dd);
    void write_qq_reg(int dd, uint16_t val);
    uint16_t read_dd_reg(int dd);
    void write_dd_reg(int dd, uint16_t val);

    uint8_t read_r_reg(int r);
    uint8_t read_r_reg_or_hl(int r);
    void write_r_reg(int r, uint8_t val);
    void write_r_reg_or_hl(int r, uint8_t val);

    uint16_t read_nn();
    uint8_t read_n();
    void push8(uint8_t val);
    void push16(uint16_t val);
    void push_pc();
    uint8_t pop8();
    uint16_t pop16();
    enum class Flag : uint8_t {
        C = 0,
        N = 1,
        PV = 2,
        F3 = 3,
        H = 4,
        F5 = 5,
        Z = 6,
        S = 7
    };

    void set_flag(Flag flag, bool val);
    void set_flags(uint8_t val);
    void set_s_flag(uint8_t val);
    void set_z_flag(uint8_t val);
    bool get_flag(Flag flag) const;
    bool test_cond(int cond);

    void out(uint8_t addr, uint8_t val);
    uint8_t in(uint8_t addr);

    // register accessors
    uint16_t read_af() const { return (mRegs.a << 8) | mRegs.f; }
    uint16_t read_bc() const { return (mRegs.b << 8) | mRegs.c; }
    uint16_t read_de() const { return (mRegs.d << 8) | mRegs.e; }
    uint16_t read_hl() const { return (mRegs.h << 8) | mRegs.l; }
    uint16_t read_ix() const { return mRegs.ix; }
    uint16_t read_iy() const { return mRegs.iy; }
    uint16_t read_sp() const { return mRegs.sp; }

    uint16_t read_af_alt() const { return (mRegs.a_alt << 8) | mRegs.f_alt; }
    uint16_t read_bc_alt() const { return (mRegs.b_alt << 8) | mRegs.c_alt; }
    uint16_t read_de_alt() const { return (mRegs.d_alt << 8) | mRegs.e_alt; }
    uint16_t read_hl_alt() const { return (mRegs.h_alt << 8) | mRegs.l_alt; }

    void write_af(uint16_t val) {
        mRegs.a = (val >> 8) & 0xff;
        mRegs.f = val & 0xff;
    }
    void write_bc(uint16_t val) {
        mRegs.b = (val >> 8) & 0xff;
        mRegs.c = val & 0xff;
    }
    void write_de(uint16_t val) {
        mRegs.d = (val >> 8) & 0xff;
        mRegs.e = val & 0xff;
    }
    void write_hl(uint16_t val) {
        mRegs.h = (val >> 8) & 0xff;
        mRegs.l = val & 0xff;
    }
    void write_ix(uint16_t val) { mRegs.ix = val; }
    void write_iy(uint16_t val) { mRegs.iy = val; }
    void write_sp(uint16_t val) { mRegs.sp = val; }

    void write_af_alt(uint16_t val) {
        mRegs.a_alt = (val >> 8) & 0xff;
        mRegs.f_alt = val & 0xff;
    }
    void write_bc_alt(uint16_t val) {
        mRegs.b_alt = (val >> 8) & 0xff;
        mRegs.c_alt = val & 0xff;
    }
    void write_de_alt(uint16_t val) {
        mRegs.d_alt = (val >> 8) & 0xff;
        mRegs.e_alt = val & 0xff;
    }
    void write_hl_alt(uint16_t val) {
        mRegs.h_alt = (val >> 8) & 0xff;
        mRegs.l_alt = val & 0xff;
    }

    // register file
    struct {
        uint8_t a;
        uint8_t f;
        uint8_t b;
        uint8_t c;
        uint8_t d;
        uint8_t e;
        uint8_t h;
        uint8_t l;

        uint8_t a_alt;
        uint8_t f_alt;
        uint8_t b_alt;
        uint8_t c_alt;
        uint8_t d_alt;
        uint8_t e_alt;
        uint8_t h_alt;
        uint8_t l_alt;

        uint16_t pc;
        uint16_t sp;
        uint16_t ix;
        uint16_t iy;

        int iff;
    } mRegs = {};

    // interrupts asserted
    std::atomic<bool> mIRQLevel = {};
    std::atomic<bool> mNMILevel = {};
};
