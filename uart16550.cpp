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

//#define TRACEF(str, x...) do { printf("uart16550: " str, ## x); } while (0)
#define TRACEF(str, x...) do { } while (0)

using namespace std;

// registers
#define RBR 0
#define THR 0
#define IER 1
#define IIR 2
#define FCR 2
#define LCR 3
#define MCR 4
#define LSR 5
#define MSR 6
#define SCR 7
#define DLL 8 // if DLAB = 0
#define DLM 9

// control bits
#define LCR_DLAB (1<<7)


uart16550::uart16550(Console &con)
:   mConsole(con)
{
}

uart16550::~uart16550()
{
}

uint8_t uart16550::ReadByte(size_t address)
{
    uint8_t val;

    TRACEF("uart16550: readbyte address 0x%zx\n", address);

    /* device is mirrored for the entire address space */
    address &= 0x7;

    if (mPendingRx < 0) {
        mPendingRx = mConsole.GetNextChar();
        if (mPendingRx == 0xa) {
            mPendingRx = 0xd;
        }
    }

    val = 0;
    switch (address) {
        case RBR:
            if (mRegisters[LCR] & LCR_DLAB) {
                // DLL
                val = mRegisters[DLL];
            } else {
                // pseudo reg, read what's in the fifo
                if (mPendingRx >= 0) {
                    val = mPendingRx;
                    mPendingRx = -1;
                }
            }
            break;
        case IER:
            if (mRegisters[LCR] & LCR_DLAB) {
                // DLM
                val = mRegisters[DLM];
            } else {
                val = mRegisters[IER];
            }
            break;
        case IIR:
            // pseudo register, interrrupt status
            val = 0;
            break;
        case LCR:
            val = mRegisters[LCR];
            break;
        case MCR:
            val = mRegisters[MCR];
            break;
        case LSR:
            // line status + the transmitter hold register and transmitter empty always set
            val = (1<<5) | (1<<6);

            // if we have any pending receive data, set the Data Ready bit
            val |= (mPendingRx >= 0) ? (1<<0) : 0;
            break;
        case MSR:
            break;
        case SCR:
            val = mRegisters[SCR];
            break;
    }

    TRACEF("returning val 0x%hhx\n", val);
    return val;
}

void uart16550::WriteByte(size_t address, uint8_t val)
{
    TRACEF("uart16550: writebyte address 0x%zx, val 0x%hhx\n", address, val);

    /* device is mirrored for the entire address space */
    address &= 0x7;

    switch (address) {
        case THR: // DLL
            if (mRegisters[LCR] & LCR_DLAB) {
                // DLL
                mRegisters[DLL] = val;
            } else {
                // pseudo reg, write
                mConsole.Putchar(val);
            }
            break;
        case IER: // DLM
            if (mRegisters[LCR] & LCR_DLAB) {
                // DLM
                mRegisters[DLM] = val;
            } else {
                mRegisters[IER] = val;
            }
            break;
        case FCR:
            mRegisters[FCR] = val;
            break;
        case LCR:
            mRegisters[LCR] = val;
            break;
        case MCR:
            mRegisters[MCR] = val;
            break;
        case LSR:
            break;
        case MSR:
            break;
        case SCR:
            mRegisters[SCR] = val;
            break;
    }
}


