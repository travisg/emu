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

#include <cstdint>

#include "cpu.h"

class CpuZ80 final : public Cpu {
public:
    explicit CpuZ80(System &sys) : Cpu(sys) {};

    virtual void Reset() override;
    virtual int Run() override;

    virtual void Dump() override;

private:
    // internal routines
    uint16_t read_qq_reg(int dd);
    void write_qq_reg(int dd, uint16_t val);
    uint16_t read_dd_reg(int dd);
    void write_dd_reg(int dd, uint16_t val);
    uint8_t read_r_reg(int r);
    void write_r_reg(int r, uint8_t val);
    uint16_t read_nn(void);
    uint8_t read_n(void);
    void push8(uint8_t val);
    void push16(uint16_t val);
    void push_pc(void);
    uint8_t pop8(void);
    uint16_t pop16(void);
    void set_flag(int flag, int val);
    void set_flags(uint8_t val);

    void out(uint8_t addr, uint8_t val);

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
};



