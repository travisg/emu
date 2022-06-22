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

#include <cstdio>
#include <cassert>
#include <iostream>

#include "memory.h"
#include "mc6850.h"

#define TRACE 0

#define TRACEF(str, x...) do { if (TRACE) printf(str, ## x); } while (0)

#define STAT_RDRF (1<<0)
#define STAT_TDRE (1<<1)
#define STAT_DCD  (1<<2)
#define STAT_CTS  (1<<3)
#define STAT_FE   (1<<4)
#define STAT_OVRN (1<<5)
#define STAT_PE   (1<<6)
#define STAT_IRQ  (1<<7)

using namespace std;

MC6850::MC6850(Console &con)
    :   mConsole(con) {
    mStatus = STAT_TDRE;
}

MC6850::~MC6850() {
}

uint8_t MC6850::ReadByte(size_t address) {
    uint8_t val;

    TRACEF("MC6850: readbyte address 0x%zx\n", address);

    if (mPendingRx < 0) {
        mPendingRx = mConsole.GetNextChar();
        if (mPendingRx == 0xa) {
            mPendingRx = 0xd;
        } else if (islower(mPendingRx)) {
            mPendingRx = toupper(mPendingRx);
        }
    }

    val = 0;
    if (address == 0) {
        // status register
        val = mStatus;
        if (mPendingRx >= 0)
            val |= STAT_RDRF;
    } else if (address == 1) {
        // data register
        if (mPendingRx >= 0) {
            val = mPendingRx;
            mPendingRx = -1;
            TRACEF("cpu read data %d\n", val);
        }
    } else {
        // unknown
    }
    return val;
}

void MC6850::WriteByte(size_t address, uint8_t val) {
    TRACEF("MC6850: writebyte address 0x%zx, val 0x%hhx\n", address, val);

    if (address == 0) {
        // control register
        // XXX ignore for now
        mControl = val;
        //printf("MC6850: control reg %#x\n", val);
    } else if (address == 1) {
        // data register
        //printf("MC6850: data reg %#x\n", val);
        mConsole.Putchar(val & 0x7f);
    } else {
        // unknown
    }
}


