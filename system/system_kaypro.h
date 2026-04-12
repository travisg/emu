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

#include "console_sdl.h"
#include "dev/wd1793.h"
#include "dev/z80sio.h"
#include "system.h"
#include <cstdint>
#include <memory>
#include <string>

class Console;
class CpuZ80;
class MemoryDevice;
class Memory;

// a Z80 based Kaypro
class SystemKaypro final : public System {
  public:
    static SystemInfo GetSystemInfo();

    SystemKaypro(const std::string &subsystem);
    virtual ~SystemKaypro() override;

    virtual int Init() override;

    virtual int Run() override;

    virtual Console *GetConsole() override { return &mConsole; }

    virtual uint8_t MemRead8(size_t address) override;
    virtual void MemWrite8(size_t address, uint8_t val) override;

    virtual uint8_t IORead8(size_t address) override;
    virtual void IOWrite8(size_t address, uint8_t val) override;

  private:
    void iHexParseCallback(const uint8_t *ptr, size_t offset, size_t len);
    void RenderDisplay(SDL_Renderer *renderer);
    void DrawChar(SDL_Renderer *renderer, int x, int y, uint8_t c, int scale);

    MemoryDevice *GetDeviceAtAddr(size_t &address);

    std::unique_ptr<CpuZ80> mCpu;
    std::unique_ptr<Memory> mMem;
    std::unique_ptr<Memory> mVideoMem;
    std::unique_ptr<Memory> mRom;
    std::unique_ptr<Memory> mVideoRom;

    ConsoleSDL mConsole;

    std::string mVideoRomString;

    WD1793 mFdc;
    Z80Sio mSio;
    SDL_Texture *mFontTexture = nullptr;

    enum {
        BANK0,
        BANK1
    } mBankSwitch = BANK1;
};
