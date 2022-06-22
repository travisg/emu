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
#include "system.h"
#include "system09.h"
#include "system_kaypro.h"
#include "altair680.h"

#include <cstdio>
#include <cassert>

#define TRACE 0

#define TRACEF(str, x...) do { if (TRACE) printf(str, ## x); } while (0)

using namespace std;

System::System(const string &subsystem, Console &con)
    :   mSubSystemString(subsystem),
        mConsole(con) {
}

System::~System() {
    ShutdownThreaded();
}

std::unique_ptr<System> System::Factory(const std::string &system, Console &con) {
    // split the system string into a few pieces
    size_t pos = system.find('-');
    string mainsystem = system.substr(0, pos);

    string subsystem;
    if (pos != string::npos)
        subsystem = system.substr(pos + 1, string::npos);

    if (mainsystem == "6809") {
        return std::unique_ptr<System>(new System09(subsystem, con));
    } else if (mainsystem == "altair680") {
        return std::unique_ptr<System>(new Altair680(subsystem, con));
    } else if (mainsystem == "kaypro") {
        return std::unique_ptr<System>(new SystemKaypro(subsystem, con));
    } else {
        return NULL;
    }
}

int System::RunThreaded() {
    assert(!mThread);

    auto start = [this]() {
        printf("Starting system thread\n");
        this->Run();
    };

    mThread.reset(new std::thread(start));

    return 0;
}

void System::ShutdownThreaded() {
    if (!mThread)
        return;

    // tell the run loop to shut down
    mShutdown = true;
    mThread->join();

    mThread.release();
}

uint16_t System::MemRead16(size_t address, Endian e) {
    uint16_t val;
    switch (e) {
        case Endian::LITTLE:
            val = MemRead8(address) | MemRead8(address + 1) << 8;
            break;
        case Endian::BIG:
            val = MemRead8(address) << 8 | MemRead8(address + 1);
            break;
    }
    TRACEF("%s: address %#zx, endian %d, val %#x\n", __func__, address, e, val);
    return val;
}

void System::MemWrite16(size_t address, uint16_t val, Endian e) {
    TRACEF("%s: address %#zx, val %#x, endian %d\n", __func__, address, val, e);
    switch (e) {
        case Endian::LITTLE:
            MemWrite8(address, val);
            MemWrite8(address + 1, val >> 8);
            break;
        case Endian::BIG:
            MemWrite8(address, val >> 8);
            MemWrite8(address + 1, val);
            break;
    }
}

