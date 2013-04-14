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
#include <iostream>

#include "memory.h"
#include "system09.h"
#include "cpu6809.h"
#include "mc6850.h"
#include "ihex.h"

using namespace std;

// a simple 6809 based system
System09::System09()
:   mCpu(0),
    mMem(0),
    mRom(0),
    mUart(0)
{
}

System09::~System09()
{
    delete mCpu;
    delete mMem;
    delete mRom;
    delete mUart;
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
    cout << "initializing a 6809 based system" << endl;

    // create a bank of memory
    Memory *mem = new Memory();
    mem->Alloc(32*1024);
    mMem = mem;

    // create a bank of rom
    mem = new Memory();
    mem->Alloc(16*1024);
    mRom = mem;

    // create a uart
    MC6850 *uart = new MC6850();
    mUart = uart;
    
    // create a 6809 based cpu
    mCpu = new Cpu6809();
    mCpu->SetSystem(this);
    mCpu->Reset();

    // preload some stuff into memory
    iHex hex;
    hex.SetCallback(&System09::iHexParseCallbackStatic, this);

#if 0
    hex.Open("test/t6809.ihx");
    hex.Parse();
    
    hex.Open("test/rom.ihx");
    hex.Parse();
#endif
    hex.Open("test/BASIC.HEX");
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
    if (*address < 0x8000) {
        return mMem;
    } else if (*address >= 0xc000) {
        *address -= 0xc000;
        return mRom;
    } else if (*address < 0xc000) {
        // UART
        *address -= 0xa000;
        return mUart;
    } else {
        // unknown
        return NULL;
    }
}


