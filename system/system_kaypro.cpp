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

#include "system_kaypro.h"

#include <cstdio>
#include <iostream>

#include "cpu/cpuz80.h"
#include "dev/memory.h"
#include "ihex.h"
#include "trace.h"

#define DEFAULT_ROM "rom/kaypro/kayproii_u47.bin"
#define VIDEO_ROM "rom/kaypro/kayproii_u43.bin"

#define LOCAL_TRACE 0

using namespace std;

// a simple 6809 based system
SystemKaypro::SystemKaypro(const std::string &subsystem, Console &con)
:   System(subsystem, con)
{
    mRomString = DEFAULT_ROM;
}

SystemKaypro::~SystemKaypro()
{
}

int SystemKaypro::Init()
{
    cout << "initializing a Z80 based system. ";
    cout << "subsystem '" << mSubSystemString << "'" << endl;
    cout << "rom is " << mRomString << endl;

    // create a z80 based cpu
    mCpu.reset(new CpuZ80(*this));
    mCpu->Reset();

    // create a bank of memory
    mMem.reset(new Memory());
    mMem->Alloc(64*1024);

    // create a bank of video memory
    mVideoMem.reset(new Memory());
    mVideoMem->Alloc(4*1024);

    // create a bank of rom
    mRom.reset(new Memory());
    mRom->Alloc(4*1024);

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

    // create a bank of video rom
    mVideoRom.reset(new Memory());
    mVideoRom->Alloc(2*1024);

    // read it in
    {
        FILE *fp = fopen(VIDEO_ROM, "rb");
        if (!fp)
            return -1;

        size_t err = fread(mVideoRom->GetPtr(), 1, mVideoRom->GetSize(), fp);
        fclose(fp);

        if (err != mVideoRom->GetSize()) {
            cout << "error reading rom " << VIDEO_ROM << std::endl;
            return -1;
        }
    }

    return 0;
}

int SystemKaypro::Run()
{
    cout << "starting main run loop" << endl;

    return mCpu->Run();
}

uint8_t SystemKaypro::MemRead8(size_t address)
{
    uint8_t val = 0;

    MemoryDevice *mem = GetDeviceAtAddr(address);
    if (mem)
        val = mem->ReadByte(address);

    LTRACEF("addr 0x%zx val 0x%x\n", address, val);
    return val;
}

void SystemKaypro::MemWrite8(size_t address, uint8_t val)
{
    address &= 0xffff;

    LTRACEF("addr 0x%zx val 0x%x\n", address, val);

    MemoryDevice *mem = GetDeviceAtAddr(address);
    if (mem)
        mem->WriteByte(address, val);
}

uint16_t SystemKaypro::MemRead16(size_t address)
{
    return MemRead8(address) << 8 | MemRead8(address + 1);
}

void SystemKaypro::MemWrite16(size_t address, uint16_t val)
{
    MemWrite8(address, val >> 8);
    MemWrite8(address + 1, val);
}

uint8_t SystemKaypro::IORead8(size_t address)
{
    uint8_t val = 0;

    LTRACEF("addr 0x%zx val 0x%x\n", address, val);

    return val;
}

void SystemKaypro::IOWrite8(size_t address, uint8_t val)
{
    LTRACEF("addr 0x%zx val 0x%x\n", address, val);

    if (LOCAL_TRACE) {
        for (uint i = 0; i <= 7; i++)
            LTRACEF("A%u %zu\n", i, (address >> i) & 0x1);
    }

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
}

MemoryDevice *SystemKaypro::GetDeviceAtAddr(size_t &address)
{
    address &= 0xffff;

    if (mBankSwitch == 0 || address >= 0x4000) {
        return mMem.get();
    } else if (address >= 0x3000) {
        address -= 0x3000;
        return mVideoMem.get();
    } else {
        return mRom.get();
    }
}


