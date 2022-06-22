// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2013-2014,2022 Travis Geiselbrecht
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


class Cpu6800 final : public Cpu {
public:
    explicit Cpu6800(System &sys);
    virtual ~Cpu6800() override;

    virtual void Reset() override;
    virtual int Run() override;

    virtual void Dump() override;

    // registers
    enum class regnum {
        REG_A,
        REG_B,
        REG_IX,
        REG_PC,
        REG_SP,
        REG_CC,
    };

private:
    uint16_t GetReg(regnum r);
    uint16_t PutReg(regnum r, uint16_t val); // returns old value

    bool TestBranchCond(unsigned int cond);

    // register file
    uint8_t mA;
    uint8_t mB;
    uint16_t mIX;
    uint16_t mPC;
    uint16_t mSP;
    uint8_t  mCC;

    // any exceptions pending?
    unsigned int mException;
};


