#pragma once

#include <cstdint>
#include <cstdio>

class WD1793 {
  public:
    WD1793();
    ~WD1793();

    uint8_t Read(int reg);
    void Write(int reg, uint8_t val);

    bool LoadImage(const char *filename);

    // Provide a way to poll if an interrupt is pending
    bool InterruptPending() const { return mIntrq; }
    void ClearInterrupt() { mIntrq = false; }

    // Provide a way to poll if data is ready (DRQ)
    bool DataReady() const { return mDrq; }

    void SetSelected(bool selected) { mSelected = selected; }

  private:
    uint8_t mStatus = 0;
    uint8_t mTrack = 0;
    uint8_t mSector = 0;
    uint8_t mData = 0;
    uint8_t mCommand = 0;

    bool mIntrq = false;
    bool mDrq = false;
    bool mIndexPulse = false;
    bool mSelected = true;

    uint16_t mSectorIndex = 0;
    uint16_t mBufferCount = 0;
    uint8_t mSectorBytes[512] = {};

    void *mFp = nullptr; // Use void* to avoid including cstdio in header if possible, or just use FILE*
    int mTracks = 40;
    int mSectorsPerTrack = 10;
    int mSectorSize = 512;

    void ProcessCommand();
};
