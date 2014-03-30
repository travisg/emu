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
#include "uart16550.h"

#define STAT_RDRF (1<<0)
#define STAT_TDRE (1<<1)
#define STAT_DCD  (1<<2)
#define STAT_CTS  (1<<3)
#define STAT_FE   (1<<4)
#define STAT_OVRN (1<<5)
#define STAT_PE   (1<<6)
#define STAT_IRQ  (1<<7)

using namespace std;

uart16550::uart16550()
{
}

uart16550::~uart16550()
{
}

uint8_t uart16550::ReadByte(size_t address)
{
    uint8_t val;

    printf("uart16550: readbyte address 0x%zx\n", address);

#if 0
    // XXX super nasty hack
    if (mPendingRx < 0) {
        mPendingRx = getchar();
        if (mPendingRx == 0x4) {
            // XXX
            exit(1);
        } else if (mPendingRx == 0xa) {
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
            //printf("cpu read data %d\n", val);
        }
    } else {
        // unknown
    }
#endif
    val = 0;
    return val;
}

void uart16550::WriteByte(size_t address, uint8_t val)
{
  printf("uart16550: writebyte address 0x%zx, val 0x%hhx\n", address, val);

#if 0
    if (address == 0) {
        // control register
        // XXX ignore for now
        mControl = val;
    } else if (address == 1) {
        // data register
        putchar(val);
    } else {
        // unknown
    }
#endif
}


