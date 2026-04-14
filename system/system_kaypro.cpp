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

#define DEFAULT_ROM "roms/kaypro/kayproii_u47.bin"
#define VIDEO_ROM   "roms/kaypro/kayproii_u43.bin"

#define LOCAL_TRACE 0

using namespace std;

System::SystemInfo SystemKaypro::GetSystemInfo() {
    return {"kaypro", "z80", DEFAULT_ROM};
}

// a simple 6809 based system
SystemKaypro::SystemKaypro(const std::string &subsystem)
    : System(subsystem) {
    mRomString = DEFAULT_ROM;
}

SystemKaypro::~SystemKaypro() {}

int SystemKaypro::Init() {
    cout << "initializing a Z80 based system. ";
    cout << "subsystem '" << mSubSystemString << "'" << endl;
    cout << "rom is " << mRomString << endl;

    // create a z80 based cpu
    mCpu.reset(new CpuZ80(*this));
    mCpu->Reset();

    // create a bank of memory
    mMem.reset(new Memory());
    mMem->Alloc(64 * 1024);

    // create a bank of video memory
    mVideoMem.reset(new Memory());
    mVideoMem->Alloc(4 * 1024);

    // create a bank of rom
    mRom.reset(new Memory());
    mRom->Alloc(4 * 1024);

    // read it in
    {
        FILE *fp = fopen(mRomString.c_str(), "rb");
        if (!fp) {
            printf("SystemKaypro: ERROR: could not open main ROM %s\n", mRomString.c_str());
            return -1;
        }

        size_t err = fread(mRom->GetPtr(), 1, mRom->GetSize(), fp);
        fclose(fp);

        if (err != mRom->GetSize()) {
            printf("SystemKaypro: ERROR: reading main ROM %s, read %zu bytes\n", mRomString.c_str(), err);
            return -1;
        }
        printf("SystemKaypro: Successfully loaded %zu bytes from main ROM %s\n", err, mRomString.c_str());
        uint8_t *ptr = (uint8_t *)mRom->GetPtr();
        printf("SystemKaypro: ROM SANITY [0-3]: %02x %02x %02x %02x\n", ptr[0], ptr[1], ptr[2], ptr[3]);
    }

    // create a bank of video rom
    mVideoRom.reset(new Memory());
    mVideoRom->Alloc(2 * 1024);

    // read it in
    {
        string video_binary = mVideoRomString;
        if (video_binary.empty()) {
            video_binary = VIDEO_ROM;
        }
        FILE *fp = fopen(video_binary.c_str(), "rb");
        if (!fp) {
            printf("SystemKaypro: ERROR: could not open video ROM %s\n", video_binary.c_str());
            return -1;
        }

        size_t err = fread(mVideoRom->GetPtr(), 1, mVideoRom->GetSize(), fp);
        fclose(fp);

        if (err != mVideoRom->GetSize()) {
            printf("SystemKaypro: ERROR: reading video ROM %s, read %zu bytes\n", video_binary.c_str(), err);
            return -1;
        }
        printf("SystemKaypro: Successfully loaded %zu bytes from video ROM %s\n", err, video_binary.c_str());
    }

    // Setup the draw callback
    mConsole.SetDrawCallback([this](SDL_Renderer *renderer) {
        this->RenderDisplay(renderer);
    });

    // Create the font texture from video ROM
    SDL_Renderer *renderer = mConsole.GetRenderer();
    if (renderer) {
        mFontTexture = SDL_CreateTexture(renderer, SDL_PIXELFORMAT_RGBA8888, SDL_TEXTUREACCESS_STATIC, 128, 128);
        if (!mFontTexture) {
            printf("SystemKaypro: ERROR: Failed to create font texture: %s\n", SDL_GetError());
        } else {
            printf("SystemKaypro: Font texture created successfully\n");
        }
        uint32_t pixels[128 * 128];
        memset(pixels, 0, sizeof(pixels));

        const uint8_t *font_ptr = (const uint8_t *)mVideoRom->GetPtr();
        for (int c = 0; c < 256; c++) {
            int cx = (c % 16) * 8;
            int cy = (c / 16) * 8;
            // The Kaypro II u43 ROM has the standard ASCII set shifted by 128
            int font_c = (c + 128) % 256;
            for (int y = 0; y < 8; y++) {
                // Try to detect if we need to invert or not
                uint8_t row = ~font_ptr[font_c * 8 + y];
                for (int x = 0; x < 8; x++) {
                    if (row & (0x80 >> x)) {
                        pixels[(cy + y) * 128 + (cx + x)] = 0xFFFFFFFF; // Pure White
                    }
                }
            }
        }
        SDL_UpdateTexture(mFontTexture, NULL, pixels, 128 * 4);
        SDL_SetTextureBlendMode(mFontTexture, SDL_BLENDMODE_BLEND);
    }

    return 0;
}

int SystemKaypro::Run() {
    cout << "starting main run loop" << endl;

    return mCpu->Run();
}

uint8_t SystemKaypro::MemRead8(size_t address) {
    uint8_t val = 0;

    size_t relative_addr = address;
    MemoryDevice *mem = GetDeviceAtAddr(relative_addr);
    if (mem) {
        val = mem->ReadByte(relative_addr);
    }

    LTRACEF("addr 0x%zx val 0x%x\n", address, val);
    return val;
}

void SystemKaypro::MemWrite8(size_t address, uint8_t val) {
    address &= 0xffff;

    LTRACEF("addr 0x%zx val 0x%x\n", address, val);

    size_t relative_addr = address;
    MemoryDevice *mem = GetDeviceAtAddr(relative_addr);
    if (mem) {
        mem->WriteByte(relative_addr, val);
        if (mem == mVideoMem.get()) {
            // printf("SystemKaypro: VIDEO WRITE 0x%02x at 0x%zx ('%c')\n", val, relative_addr, (val >= 32 && val < 127) ? val : '.');
            mConsole.ForceRefresh();
        }
    }
}

void SystemKaypro::RenderDisplay(SDL_Renderer *renderer) {
    if (!mFontTexture) {
        return;
    }

    for (int y = 0; y < 24; y++) {
        for (int x = 0; x < 80; x++) {
            // Reverting to 128-byte stride based on address logs
            uint8_t c = mVideoMem->ReadByte(y * 128 + x);
            DrawChar(renderer, x, y, c, 1);
        }
    }
}

void SystemKaypro::DrawChar(SDL_Renderer *renderer, int x, int y, uint8_t c, int scale) {
    if (c == 0x20 || c == 0x00 || c == 0xFF) {
        return;
    }

    // Native Kaypro is 8x8 font, but CRT scans made it tall.
    // We stretch 8x8 font to 8x16 on screen.
    int char_w = 8 * scale;
    int char_h = 16 * scale;
    SDL_Rect dst = {x * char_w, y * char_h, char_w, char_h};

    bool inverse = (c & 0x80);
    uint8_t char_idx = c & 0x7f;
    SDL_Rect src = {(char_idx % 16) * 8, (char_idx / 16) * 8, 8, 8};

    if (inverse) {
        // Background for inverse: Green
        SDL_SetRenderDrawColor(renderer, 0, 255, 0, 255);
        SDL_RenderFillRect(renderer, &dst);

        // Character for inverse: Black
        SDL_SetTextureColorMod(mFontTexture, 0, 0, 0);
    } else {
        // Character for normal: Green
        SDL_SetTextureColorMod(mFontTexture, 0, 255, 0);
    }

    SDL_RenderCopy(renderer, mFontTexture, &src, &dst);
}

uint8_t SystemKaypro::IORead8(size_t address) {
    uint8_t val = 0;

    switch (address) {
        case 0x04: // serial port A, data
            val = mSio.ReadDataA();
            break;
        case 0x06: // serial port A, control
            val = mSio.ReadControlA();
            break;
        case 0x05: // serial port B, data
            val = mSio.ReadDataB();
            break;
        case 0x07: // serial port B, control
            val = mSio.ReadControlB();
            break;
        case 0x10: // floppy status
        case 0x11: // floppy track
        case 0x12: // floppy sector
        case 0x13: // floppy data
            val = mFdc.Read(address - 0x10);
            break;
        case 0x1c: // PIO 2 channel A, data (System Data port)
            // Bit 6 is floppy INTRQ
            // Bit 7 is floppy DRQ
            if (mFdc.InterruptPending()) {
                val |= 0x40; // Set INTRQ bit (bit 6)
            }
            if (mFdc.DataReady()) {
                val |= 0x80; // Set DRQ bit (bit 7)
            }
            break;
    }

    TRACEF("addr 0x%zx val 0x%hhx\n", address, val);

    return val;
}

void SystemKaypro::IOWrite8(size_t address, uint8_t val) {
    TRACEF("addr 0x%zx val 0x%x\n", address, val);

    if (LOCAL_TRACE > 1) {
        for (uint i = 0; i <= 7; i++) {
            LTRACEF("A%u %zu\n", i, (address >> i) & 0x1);
        }
    }

    switch (address) {
        case 0x00: // baud rate generator A
            // dont care
            break;
        case 0x04: // serial port A, data
            mSio.WriteDataA(val);
            break;
        case 0x06: // serial port A, control
            mSio.WriteControlA(val);
            break;
        case 0x05: // serial port B, data
            mSio.WriteDataB(val);
            break;
        case 0x07: // serial port B, control
            mSio.WriteControlB(val);
            break;
        case 0x08: // PIO 1 channel A, data
        case 0x09: // PIO 1 channel A, control
        case 0x0a: // PIO 1 channel B, data
        case 0x0b: // PIO 1 channel B, control
            // pio_out(1, addr - 0x8, val);
            break;
        case 0x0c: // baud rate generator B
            // dont care
            break;
        case 0x10: // floppy status/command
        case 0x11: // floppy track
        case 0x12: // floppy sector
        case 0x13: // floppy data
            mFdc.Write(address - 0x10, val);
            break;
        case 0x14:
        case 0x15:
        case 0x16:
        case 0x17: // bank register and floppy PIO
            break;

        case 0x1c: // PIO 2 channel A, data
            printf("SystemKaypro: SYSTEM CONTROL Port 0x1C write: 0x%02x\n", val);
            mControlLatch = val;
            break;
        case 0x1d: // PIO 2 channel A, control
        case 0x1e: // PIO 2 channel B, data
        case 0x1f: // PIO 2 channel B, control
            // pio_out(2, addr - 0x1c, val);
            break;
        default:
            fprintf(stderr, "out to unknown port 0x%zx\n", address);
            break;
    }
}

MemoryDevice *SystemKaypro::GetDeviceAtAddr(size_t &address) {
    address &= 0xffff;

    if (CurrentBank() == Bank::BANK0 || address >= 0x4000) {
        return mMem.get();
    } else if (address >= 0x3000) { // BANK1 and within video memory
        address -= 0x3000;
        return mVideoMem.get();
    } else { // BANK1 and within rom memory
        return mRom.get();
    }
}
