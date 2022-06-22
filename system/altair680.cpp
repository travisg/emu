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

#include "altair680.h"

#include <cstdio>
#include <iostream>

#include "cpu/cpu6800.h"
#include "dev/memory.h"
#include "dev/mc6850.h"
#include "ihex.h"

#define DEFAULT_ROM "mits680b.bin"

using namespace std;

Altair680::Altair680(const std::string &subsystem, Console &con)
    :   System(subsystem, con) {
    mRomString = DEFAULT_ROM;
}

Altair680::~Altair680() {
}

void Altair680::iHexParseCallback(const uint8_t *ptr, size_t address, size_t len) {
    //printf("parsecallback %p address %#zx length %#zx\n", ptr, address, len);

    for (size_t i = 0; i < len; i++) {
        size_t addr = address;
        MemoryDevice *mem = GetDeviceAtAddr(addr);
        mem->WriteByte(addr, ptr[i]);

        address++;
    }
}

int Altair680::Init() {
    cout << "initializing an Altair 680..." << endl;
    cout << "rom is " << mRomString << endl;

    // create a bank of memory
    Memory *mem = new Memory();
    mem->Alloc(32*1024);
    mMem.reset(mem);

    // create a bank of rom for the monitor
    Memory *rommem = new Memory();
    rommem->Alloc(256);
    mRom_monitor.reset(rommem);

    // set a bank for the VTL rom
    Memory *rom_vtl = new Memory();
    rom_vtl->Alloc(768);
    mRom_vtl.reset(rom_vtl);

    // read the prom directly from file
    FILE *fp = fopen(mRomString.c_str(), "rb");
    if (!fp) {
        cerr << "Error opening rom file " << mRomString << endl;
        return -errno;
    }

    if (fread(rommem->GetPtr(), 256, 1, fp) != 1) {
        fclose(fp);
        cerr << "Error reading from rom file " << mRomString << endl;
        return -errno;
    }

    fclose(fp);

    // create a 6800 based cpu
    mCpu.reset(new Cpu6800(*this));
    mCpu->Reset();

    // add some peripherals
    // create a MC6850 uart
    mUart.reset(new MC6850(mConsole));

    return 0;
}

int Altair680::Run() {
    cout << "starting main run loop" << endl;

    return mCpu->Run();
}

uint8_t Altair680::MemRead8(size_t address) {
    uint8_t val = 0;

    MemoryDevice *mem = GetDeviceAtAddr(address);
    if (mem)
        val = mem->ReadByte(address);

    //cout << "\tMemRead8 at 0x" << hex << address << " val " << (unsigned int)val << endl;
    return val;
}

void Altair680::MemWrite8(size_t address, uint8_t val) {
    address &= 0xffff;

    MemoryDevice *mem = GetDeviceAtAddr(address);
    if (mem)
        mem->WriteByte(address, val);

    //cout << "\tMemWrite8 at 0x" << hex << address << " val " << (unsigned int)val << endl;
}

MemoryDevice *Altair680::GetDeviceAtAddr(size_t &address) {
    address &= 0xffff;

    // figure out which memory device this address belongs to
    switch (address) {
        // main memory bank
        case 0x0000 ... 0x7fff:
            return mMem.get();

#if 0
        // device space
        // 8 slots of 0x800 bytes

        case 0x8000 ... 0x87ff:
            if (mMemoryLayout == MemoryLayout::OBC) {
                address -= 0x8000;
                return mUart.get();
            }
            return NULL;

        // old location for BASIC.HEX
        case 0xa000 ... 0xa7ff:
            if (mMemoryLayout == MemoryLayout::STANDARD) {
                address -= 0xa000;
                return mUart.get();
            }
            return NULL;
#endif
        // hardware
        case 0xf000 ... 0xf001:
            address -= 0xf000;
            return mUart.get();

        // roms
        case 0xfc00 ... 0xfeff:
            address -= 0xfc00;
            return mRom_vtl.get();

        case 0xff00 ... 0xffff:
            address -= 0xff00;
            return mRom_monitor.get();

        default:
            return NULL;
    }
}


