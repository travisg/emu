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

#include <stdint.h>
#include <string>
#include "system.h"

class Console;
class Cpu6809;
class MemoryDevice;
class MC6850;

// a simple 6809 based system
class System09 : public System {
public:
    System09(const std::string &subsystem, Console &con);
    virtual ~System09() override;

    virtual int Init() override;

    virtual int Run() override;

    virtual int SetRom(const std::string &rom) override;
    virtual int SetCpu(const std::string &cpu) override;

    virtual uint8_t  MemRead8(size_t address) override;
    virtual void     MemWrite8(size_t address, uint8_t val) override;

    virtual uint16_t MemRead16(size_t address) override;
    virtual void     MemWrite16(size_t address, uint16_t val) override;

private:
    void iHexParseCallback(const uint8_t *ptr, size_t offset, size_t len);
    static void iHexParseCallbackStatic(void *context, const uint8_t *ptr, size_t offset, size_t len);

    MemoryDevice *GetDeviceAtAddr(size_t *address);

    Cpu6809 *mCpu;
    MemoryDevice *mMem;
    MemoryDevice *mRom;
    MC6850 *mUart;

    std::string mRomString;
    std::string mCpuString;
};


