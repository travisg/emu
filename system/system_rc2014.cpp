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

#include "system_rc2014.h"

#include <cassert>
#include <cstdio>
#include <iostream>

#include "cpu/cpuz80.h"
#include "dev/memory.h"
#include "console.h"
#include "ihex.h"
#include "trace.h"

// default rom from https://github.com/RC2014Z80/RC2014/tree/master/ROMs/Factory
//
// microsoft 32k basic for SIO/2, offset 0x0000
// microsoft 56k basic for SIO/2, offset 0x2000
// small computer monitor for pagable rom, 64k ram, at offset 0x4000 - 0x8000 (double banked)
// CP/M monitor for pageable rom for SIO/2 at offset 0x8000
// small computer monitor for everything at offset 0xe000
#define DEFAULT_ROM "rom/rc2014/24886009.BIN"

#define LOCAL_TRACE 0
#define TRACE_MEM 0
#define TRACE_IO 0

#define MTRACEF(x...) { if (TRACE_MEM) printf("MEM: " x); } while (0)
#define ITRACEF(x...) { if (TRACE_IO) printf("IO: " x); } while (0)

using namespace std;

// a simple z80 based system
SystemRC2014::SystemRC2014(const std::string &subsystem, Console &con)
    :   System(subsystem, con) {
    mRomString = DEFAULT_ROM;
}

SystemRC2014::~SystemRC2014() {
}

int SystemRC2014::Init() {
    cout << "initializing a RC2014 based system. ";
    cout << "subsystem '" << mSubSystemString << "'" << endl;
    cout << "rom is " << mRomString << endl;

    // create a z80 based cpu
    mCpu.reset(new CpuZ80(*this));
    mCpu->Reset();

    // create a full 64k bank of ram
    mMem.reset(new Memory());
    mMem->Alloc(64*1024);

    // create a full 64k bank of rom
    // only part of it is visible at a time
    mRom.reset(new Memory());
    mRom->Alloc(64*1024);

    // read it in
    {
        FILE *fp = fopen(mRomString.c_str(), "rb");
        if (!fp)
            return -1;

        size_t err = fread(mRom->GetPtr(), 1, mRom->GetSize(), fp);
        fclose(fp);

        if (err != mRom->GetSize()) {
            cout << "error reading rom " << mRomString << std::endl;
            return -1;
        }
    }

    // register a receive hook
    mConsole.RegisterInBufferCountAdd([this](size_t count) { return OnConsoleInBufferAdd(count); });

    return 0;
}

// callback from the console when a character is put in the input buffer
void SystemRC2014::OnConsoleInBufferAdd(size_t count) {
    if (count > 0 && !mSIORecvByte_valid) {
        int c = mConsole.GetNextChar();
        if (c < 0) {
            // doesn't make any sense!
            assert(0);
        }
        LTRACEF("consuming char '%c' from console input queue\n", c);
        mSIORecvByte = c;
        mSIORecvByte_valid = true;

        // TODO: interrupt
        mCpu->RaiseIRQ();
    }
}

int SystemRC2014::Run() {
    cout << "starting main run loop" << endl;

    return mCpu->Run();
}

uint8_t SystemRC2014::MemRead8(size_t address) {
    uint8_t val = 0;

    auto memdesc = GetDeviceAtAddr(address);
    if (memdesc.mem)
        val = memdesc.mem->ReadByte(address + memdesc.offset);

    MTRACEF("R %#zx val %#x\n", address, val);
    return val;
}

void SystemRC2014::MemWrite8(size_t address, uint8_t val) {
    address &= 0xffff;

    MTRACEF("W %#zx val %#x\n", address, val);

    auto memdesc = GetDeviceAtAddr(address);
    if (memdesc.mem)
        memdesc.mem->WriteByte(address + memdesc.offset, val);
}

uint16_t SystemRC2014::MemRead16(size_t address) {
    return MemRead8(address) << 8 | MemRead8(address + 1);
}

void SystemRC2014::MemWrite16(size_t address, uint16_t val) {
    MemWrite8(address, val >> 8);
    MemWrite8(address + 1, val);
}

uint8_t SystemRC2014::IORead8(size_t address) {
    uint8_t val = 0;

    switch (address) {
        case 0x80: // SIO/A control port
            if (mSIORecvByte_valid) {
                val |= 0b1; // receive character available
                val |= 0b10; // interrupt condition
            }
            break;
        case 0x81: // SIO/A data port
            if (mSIORecvByte_valid) {
                val = mSIORecvByte;
                mSIORecvByte_valid = false;
                // TODO: latch another value
                mCpu->LowerIRQ();
            }
            break;
        case 0x82: // SIO/B control port
            break;
        case 0x83: // SIO/B data port
            break;
        default:
            fprintf(stderr, "in from unknown port 0x%zx\n", address);
            break;
    }

    ITRACEF("PC 0x%04x R %#zx val 0x%02x\n", mCpu->GetPC(), address, val);

    return val;
}

void SystemRC2014::IOWrite8(size_t address, uint8_t val) {
    ITRACEF("PC 0x%04x W %#zx val 0x%02x\n", mCpu->GetPC(), address, val);

#if 0
    for (uint i = 0; i <= 7; i++) {
        ITRACEF("A%u %zu\n", i, (address >> i) & 0x1);
    }
#endif

    switch (address) {
        case 0x80: // SIO/A control port
            break;
        case 0x81: // SIO/A data port
            // cheezy!
            mConsole.Putchar(val);
            break;
        case 0x82: // SIO/B control port
            break;
        case 0x83: // SIO/B data port
            break;
        default:
            fprintf(stderr, "out to unknown port 0x%zx\n", address);
            break;
    }

#if 0

    switch (address) {
        case 0x00: // baud rate generator A
            // dont care
            break;
        case 0x04: // serial port A, data
        case 0x06: // serial port A, control
            //sio_out(1, addr - 4, val);
            break;
        case 0x05: // serial port B, data
        case 0x07: // serial port B, control
            //sio_out(2, addr - 5, val);
            break;
        case 0x08: // PIO 1 channel A, data
        case 0x09: // PIO 1 channel A, control
        case 0x0a: // PIO 1 channel B, data
        case 0x0b: // PIO 1 channel B, control
            //pio_out(1, addr - 0x8, val);
            break;
        case 0x0c: // baud rate generator B
            // dont care
            break;
        case 0x10: // floppy status/command
        case 0x11: // floppy track
        case 0x12: // floppy sector
        case 0x13: // floppy data
            //floppy_out(addr - 0x10, val);
            break;
        case 0x14:
        case 0x15:
        case 0x16:
        case 0x17: // bank register and floppy PIO
            mBankSwitch = (val & 0x1) ? BANK1 : BANK0;
            break;

        case 0x1c: // PIO 2 channel A, data
        case 0x1d: // PIO 2 channel A, control
        case 0x1e: // PIO 2 channel B, data
        case 0x1f: // PIO 2 channel B, control
            //pio_out(2, addr - 0x1c, val);
            break;
        default:
            fprintf(stderr, "out to unknown port 0x%zx\n", address);
            break;
    }
#endif
}

SystemRC2014::MemDeviceDesc SystemRC2014::GetDeviceAtAddr(size_t address) {
    address &= 0xffff;

    // at the moment decode
    // [0x0000 ... 0x1fff]: bank in rom selected by mRomBankSel
    // [0x2000 ... 0x7fff]: null
    // [0x8000 ... 0xffff]: ram, offset by 0x8000

    if (address < 0x2000) {
        return { mRom.get(), mRomBankSel * 0x2000};
    } else if (address >= 0x8000) {
        return { mMem.get(), 0};
    } else {
        return { nullptr, 0 };
    }
}


