#include "wd1793.h"
#include "trace.h"
#include <cstdio>

#define LOCAL_TRACE 2

WD1793::WD1793() {
}

WD1793::~WD1793() {
    if (mFp) {
        fclose((FILE *)mFp);
    }
}

bool WD1793::LoadImage(const char *filename) {
    if (mFp) {
        fclose((FILE *)mFp);
    }
    mFp = fopen(filename, "rb");
    if (!mFp) {
        LPRINTF("WD1793: failed to open image '%s'\n", filename);
        return false;
    }
    LPRINTF("WD1793: loaded image '%s'\n", filename);
    return true;
}

uint8_t WD1793::Read(int reg) {
    uint8_t val = 0;
    switch (reg) {
        case 0: // Status Register
            val = mStatus;

            // Not Ready if no image or not selected
            if (!mFp || !mSelected) {
                val |= 0x80;
            }

            if ((mCommand & 0x80) == 0) {
                // Type I or Type IV command context: bit 1 is Index Pulse, bit 2 is Track 0
                if (mTrack == 0) {
                    val |= 0x04;
                }

                // Index pulse: toggle every 512 reads to simulate spinning
                {
                    static int index_counter = 0;
                    if (++index_counter > 64) {
                        mIndexPulse = !mIndexPulse;
                        index_counter = 0;
                    }
                    if (mIndexPulse) {
                        val |= 0x02; // Index Pulse
                    }
                }
            } else {
                // Type II or Type III command context: bit 1 is DRQ
                if (mDrq) {
                    val |= 0x02;
                }
            }

            mIntrq = false; // Reading status clears interrupt
            printf("WD1793: mIntrq cleared by Read(0)\n");
            break;
        case 1: // Track Register
            val = mTrack;
            break;
        case 2: // Sector Register
            val = mSector;
            break;
        case 3: // Data Register
            if (mDrq && mSectorIndex < mBufferCount) {
                val = mSectorBytes[mSectorIndex++];
                if (mSectorIndex >= mBufferCount) {
                    mDrq = false;   // No more data
                    mIntrq = true;  // Operation complete
                    mStatus = 0x00; // clear busy
                }
            } else {
                val = mData;
            }
            break;
    }
    if (reg != 3) {
        LTRACEF("WD1793: read reg %d = 0x%02x\n", reg, val);
    }
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
        LPRINTF("WD1793: Read Sector track %d sector %d\n", mTrack, mSector);
        mStatus = 0x01; // Status Busy
        mIntrq = false;
        mSectorIndex = 0;
        mBufferCount = mSectorSize;

        bool found = false;
        if (mFp) {
            // Calculate offset. Kaypro 2 uses 0-indexed sectors (0-9).
            int sector_offset = mSector % mSectorsPerTrack;

            if (mTrack < mTracks) {
                long offset = (long)(mTrack * mSectorsPerTrack + sector_offset) * mSectorSize;
                if (fseek((FILE *)mFp, offset, SEEK_SET) == 0) {
                    if (fread(mSectorBytes, 1, mSectorSize, (FILE *)mFp) == (size_t)mSectorSize) {
                        found = true;
                    }
                }
            }
        }

        if (!found) {
            LPRINTF("WD1793: Read Sector failed (track %d sector %d), filling with 0xe5\n", mTrack, mSector);
            for (int i = 0; i < mSectorSize; i++) {
                mSectorBytes[i] = 0xe5;
            }
        }
        mDrq = true;
    } else if ((mCommand & 0xf0) == 0xc0) {
        // Type III: Read Address (0xc0-0xcf)
        LPRINTF("WD1793: Read Address command\n");
        mStatus = 0x01; // Status Busy
        mIntrq = false;
        mSectorIndex = 0;
        mBufferCount = 6;
        mSectorBytes[0] = mTrack;
        mSectorBytes[1] = 0;       // side 0
        mSectorBytes[2] = mSector; // sector
        mSectorBytes[3] = 2;       // 512 byte sectors
        mSectorBytes[4] = 0;       // crc
        mSectorBytes[5] = 0;       // crc
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
        mStatus = 0x00;
        mIntrq = true; // complete it
    }
}
