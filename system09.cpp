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

#include "system09.h"

#include <cstdio>
#include <iostream>

#include "memory.h"
#include "cpu6809.h"
#include "mc6850.h"
#include "ihex.h"

#define DEFAULT_ROM "test/BASIC.HEX"

using namespace std;

// a simple 6809 based system
System09::System09(const std::string &subsystem, Console &con)
:   System(subsystem, con),
    mCpu(0),
    mMem(0),
    mRom(0),
    mUart(0),
    mRomString(DEFAULT_ROM)
{
}

System09::~System09()
{
    delete mCpu;
    delete mMem;
    delete mRom;
    delete mUart;
}

int System09::SetRom(const std::string &rom)
{
    mRomString = rom;

    return 0;
}

int System09::SetCpu(const std::string &cpu)
{
    mCpuString = cpu;

    return 0;
}

void System09::iHexParseCallbackStatic(void *context, const uint8_t *ptr, size_t offset, size_t len)
{
    System09 *s = static_cast<System09 *>(context);
    s->iHexParseCallback(ptr, offset, len);
}

void System09::iHexParseCallback(const uint8_t *ptr, size_t address, size_t len)
{
    //printf("parsecallback %p address %#zx length %#zx\n", ptr, address, len);

    for (size_t i = 0; i < len; i++) {
        size_t addr = address;
        MemoryDevice *mem = GetDeviceAtAddr(&addr);
        mem->WriteByte(addr, ptr[i]);

        address++;
    }
}

int System09::Init()
{
    cout << "initializing a 6809 based system. ";
    cout << "subsystem '" << mSubSystemString << "'" << endl;

    // create a bank of memory
    Memory *mem = new Memory();
    mem->Alloc(32*1024);
    mMem = mem;

    // create a bank of rom
    mem = new Memory();
    mem->Alloc(16*1024);
    mRom = mem;

    // create a uart
    MC6850 *uart = new MC6850(mConsole);
    mUart = uart;

    // create a 6809 based cpu
    mCpu = new Cpu6809(*this);
    mCpu->Reset();

    // preload some stuff into memory
    iHex hex;
    hex.SetCallback(&System09::iHexParseCallbackStatic, this);

    cout << "rom is " << mRomString << endl;

    hex.Open(mRomString);
    hex.Parse();

    return 0;
}

int System09::Run()
{
    cout << "starting main run loop" << endl;

    return mCpu->Run();
}

uint8_t System09::MemRead8(size_t address)
{
    uint8_t val = 0;

    MemoryDevice *mem = GetDeviceAtAddr(&address);
    if (mem)
        val = mem->ReadByte(address);

    //cout << "MemRead8 @0x" << hex << address << " val " << (unsigned int)val << endl;
    return val;
}

void System09::MemWrite8(size_t address, uint8_t val)
{
    address &= 0xffff;

    MemoryDevice *mem = GetDeviceAtAddr(&address);
    if (mem)
        mem->WriteByte(address, val);

    //cout << "MemWrite8 @0x" << hex << address << " val " << (unsigned int)val << endl;
}

uint16_t System09::MemRead16(size_t address)
{
    return MemRead8(address) << 8 | MemRead8(address + 1);
}

void System09::MemWrite16(size_t address, uint16_t val)
{
    mMem->WriteByte(address, val >> 8);
    mMem->WriteByte(address + 1, val);
}

MemoryDevice *System09::GetDeviceAtAddr(size_t *address)
{
    *address &= 0xffff;

    // figure out which memory device this address belongs to
    switch (*address) {
        // main memory bank
        case 0x0000 ... 0x7fff:
            return mMem;

#if 0
        // device space
        // 8 slots of 0x800 bytes
        case 0x8000 ... 0x87ff:
            *address -= 0x8000;
            return mUart;

        // only the 1st slot is used at the moment
        case 0x8800 ... 0xbfff:
            return NULL;
#else
        // old location for BASIC.HEX
        case 0xa000 ... 0xa7ff:
            *address -= 0xa000;
            return mUart;
#endif

        // rom
        case 0xc000 ... 0xffff:
            *address -= 0xc000;
            return mRom;
        default:
            return NULL;
    }
}


