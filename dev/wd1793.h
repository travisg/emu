#pragma once

#include <cstdint>

class WD1793 {
public:
    WD1793();
    ~WD1793() = default;

    uint8_t Read(int reg);
    void Write(int reg, uint8_t val);

    // Provide a way to poll if an interrupt is pending
    bool InterruptPending() const { return mIntrq; }
    void ClearInterrupt() { mIntrq = false; }
    
    // Provide a way to poll if data is ready (DRQ)
    bool DataReady() const { return mDrq; }

private:
    uint8_t mStatus;
    uint8_t mTrack;
    uint8_t mSector;
    uint8_t mData;
    uint8_t mCommand;

    bool mIntrq;
    bool mDrq;

    uint16_t mSectorIndex;
    uint16_t mBufferCount;
    uint8_t mSectorBytes[512];

    void ProcessCommand();
};
