// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2013-2014 Travis Geiselbrecht
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


/* registers */
enum regnum {
    REG_X,
    REG_Y,
    REG_U,
    REG_S,
    REG_A,
    REG_B,
    REG_D,
    REG_PC,
    REG_DP,
    REG_CC,
};

class Cpu6809 : public Cpu {
public:
    explicit Cpu6809(System &sys);
    virtual ~Cpu6809() override;

    virtual void Reset() override;
    virtual int Run() override;

    virtual void Dump() override;

private:
    uint16_t GetReg(regnum r);
    void PutReg(regnum r, uint16_t val);

    bool TestBranchCond(unsigned int cond);

    // register file
    union {
        struct {
            uint8_t mB;
            uint8_t mA;
        };
        uint16_t mD; // D is a union of A & B (XXX is endian specific)
    };
    uint16_t mX;
    uint16_t mY;
    uint16_t mU;
    uint16_t mS;
    uint16_t mPC;
    uint8_t  mDP;
    uint8_t  mCC;

    // any exceptions pending?
    unsigned int mException;
};


