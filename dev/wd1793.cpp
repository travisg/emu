#include "wd1793.h"
#include "trace.h"
#include <cstdio>

#define LOCAL_TRACE 1

WD1793::WD1793() : mStatus(0), mTrack(0), mSector(0), mData(0), mCommand(0), mIntrq(false), mDrq(false), mSectorIndex(0) {
}

uint8_t WD1793::Read(int reg) {
    uint8_t val = 0;
    switch (reg) {
        case 0: // Status Register
            val = mStatus;
            mIntrq = false; // Reading status clears interrupt
            break;
        case 1: // Track Register
            val = mTrack;
            break;
        case 2: // Sector Register
            val = mSector;
            break;
        case 3: // Data Register
            if (mDrq && mSectorIndex < sizeof(mSectorBytes)) {
                val = mSectorBytes[mSectorIndex++];
                if (mSectorIndex >= sizeof(mSectorBytes)) {
                    mDrq = false;   // No more data
                    mIntrq = true;  // Operation complete
                    mStatus = 0x00; // clear busy
                }
            } else {
                val = mData;
            }
            break;
    }
    LTRACEF("WD1793: read reg %d = 0x%02x\n", reg, val);
    return val;
}

void WD1793::Write(int reg, uint8_t val) {
    LTRACEF("WD1793: write reg %d = 0x%02x\n", reg, val);
    switch (reg) {
        case 0: // Command Register
            mCommand = val;
            ProcessCommand();
            break;
        case 1: // Track Register
            mTrack = val;
            break;
        case 2: // Sector Register
            mSector = val;
            break;
        case 3: // Data Register
            mData = val;
            break;
    }
}

void WD1793::ProcessCommand() {
    uint8_t cmd = mCommand & 0xf0;

    // Type I commands
    if ((mCommand & 0x80) == 0) {
        if (cmd == 0x00) {
            // Restore (Seek to track 0)
            LPRINTF("WD1793: Restore command\n");
            mTrack = 0;
            mStatus = 0x04; // Track 0 hit
            mIntrq = true;  // Trigger interrupt indicating completion
        } else if (cmd == 0x10) {
            // Seek
            LPRINTF("WD1793: Seek command\n");
            mTrack = mData; // simplistic seek
            mStatus = 0x00;
            mIntrq = true;
        } else {
            LPRINTF("WD1793: Unhandled Type I command 0x%02x\n", mCommand);
            mIntrq = true; // just complete it immediately
        }
    } else if ((mCommand & 0xe0) == 0x80) {
        // Type II: Read Sector (0x80-0x9F)
        LPRINTF("WD1793: Read Sector command\n");
        mStatus = 0x01; // Status Busy
        mIntrq = false;
        mSectorIndex = 0;
        for (int i = 0; i < 512; i++) {
            mSectorBytes[i] = 0xe5;
        }
        mDrq = true;
    } else if ((mCommand & 0xf0) == 0xd0) {
        // Type IV: Force Interrupt (0xd0-0xdf)
        LPRINTF("WD1793: Force Interrupt command\n");
        mStatus = 0x00;
        // If lower 4 bits are 0, it means terminate with no interrupt
        if ((mCommand & 0x0f) == 0) {
            mIntrq = false;
        } else {
            mIntrq = true;
        }
    } else {
        LPRINTF("WD1793: Unhandled command 0x%02x\n", mCommand);
        mIntrq = true; // complete it
    }
}
