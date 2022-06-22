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
class Cpu6800;
class MemoryDevice;

// Altair680
class Altair680 final : public System {
public:
    Altair680(const std::string &subsystem, Console &con);
    virtual ~Altair680() override;

    virtual int Init() override;

    virtual int Run() override;

    virtual uint8_t  MemRead8(size_t address) override;
    virtual void     MemWrite8(size_t address, uint8_t val) override;

private:
    void iHexParseCallback(const uint8_t *ptr, size_t offset, size_t len);

    MemoryDevice *GetDeviceAtAddr(size_t &address);

    std::unique_ptr<Cpu6800> mCpu;
    std::unique_ptr<MemoryDevice> mMem; // 32KB at 0
    std::unique_ptr<MemoryDevice> mRom_monitor; // 256 bytes at FF00
    std::unique_ptr<MemoryDevice> mRom_vtl; // 768 bytes at FC00
    std::unique_ptr<MemoryDevice> mUart;
};


