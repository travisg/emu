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

#include <cstdint>
#include <string>
#include <memory>
#include "system.h"

class Console;
class CpuZ80;
class MemoryDevice;
class Memory;

// a Z80 based RC2014
class SystemRC2014 final : public System {
public:
    SystemRC2014(const std::string &subsystem, Console &con);
    virtual ~SystemRC2014() override;

    virtual int Init() override;

    virtual int Run() override;

    virtual uint8_t  MemRead8(size_t address) override;
    virtual void     MemWrite8(size_t address, uint8_t val) override;

    virtual uint16_t MemRead16(size_t address) override;
    virtual void     MemWrite16(size_t address, uint16_t val) override;

    virtual uint8_t  IORead8(size_t address) override;
    virtual void     IOWrite8(size_t address, uint8_t val) override;

private:
    void iHexParseCallback(const uint8_t *ptr, size_t offset, size_t len);

    struct MemDeviceDesc {
        MemoryDevice *mem;
        size_t offset;
    };

    MemDeviceDesc GetDeviceAtAddr(size_t address);

    std::unique_ptr<CpuZ80> mCpu;
    std::unique_ptr<Memory> mMem;
    std::unique_ptr<Memory> mRom;

    // which rom bank is active
    unsigned int mRomBankSel = 0;
};


