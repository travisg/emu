#include "z80sio.h"
#include "trace.h"
#include <cstdio>

#define LOCAL_TRACE 1

Z80Sio::Z80Sio() {
}

uint8_t Z80Sio::ReadControl(Channel &chan) {
    uint8_t val = chan.status_regs[chan.pointer];
    LTRACEF("Z80SIO: Read Control RR%d = 0x%02x\n", chan.pointer, val);
    chan.pointer = 0; // After reading, pointer resets to 0
    return val;
}

void Z80Sio::WriteControl(Channel &chan, uint8_t val) {
    if (chan.pointer == 0) {
        // Lower 3 bits select the next register if it's a register select command
        chan.pointer = val & 0x07;

        // Command decoding (upper bits of WR0)
        uint8_t cmd = (val >> 3) & 0x07;
        if (cmd == 0b011) { // Channel reset
            LTRACEF("Z80SIO: Channel Reset\n");
            chan = Channel();      // Reset channel state
        } else if (cmd == 0b010) { // Reset Rx CRC checker
            // Not implemented
        } else if (cmd == 0b001) { // Send abort (SDLC)
            // Not implemented
        } else if (cmd == 0b100) { // Enable int on next Rx character
            // Not implemented
        } else if (cmd == 0b101) { // Reset Tx int pending
            // Not implemented
        } else if (cmd == 0b110) {       // Error reset
            chan.status_regs[1] &= 0x8f; // Clear error bits
        } else if (cmd == 0b111) {       // Return from int (Ch. A only)
            // Not implemented
        }
    } else {
        LTRACEF("Z80SIO: Write WR%d = 0x%02x\n", chan.pointer, val);
        chan.control_regs[chan.pointer] = val;
        chan.pointer = 0; // Pointer resets after writing to selected register
    }
}

uint8_t Z80Sio::ReadData(Channel &chan) {
    uint8_t val = 0;
    if (!chan.rx_fifo.empty()) {
        val = chan.rx_fifo.front();
        chan.rx_fifo.pop();
        if (chan.rx_fifo.empty()) {
            chan.status_regs[0] &= ~0x01; // Clear Rx Character Available bit
        }
    }
    LTRACEF("Z80SIO: Read Data = 0x%02x\n", val);
    return val;
}

void Z80Sio::WriteData(Channel &chan, uint8_t val) {
    LTRACEF("Z80SIO: Write Data = 0x%02x\n", val);
    // Typically consumed instantly into ether for now
    chan.status_regs[0] |= 0x04; // Set Tx buffer empty bit (instant transmit)
}

void Z80Sio::InjectKeyboardByte(uint8_t val) {
    mChanB.rx_fifo.push(val);
    mChanB.status_regs[0] |= 0x01; // Set Rx Character Available bit
}

// Wrapper pass-through methods
uint8_t Z80Sio::ReadDataA() {
    return ReadData(mChanA);
}
uint8_t Z80Sio::ReadControlA() {
    return ReadControl(mChanA);
}
void Z80Sio::WriteDataA(uint8_t val) {
    WriteData(mChanA, val);
}
void Z80Sio::WriteControlA(uint8_t val) {
    WriteControl(mChanA, val);
}

uint8_t Z80Sio::ReadDataB() {
    return ReadData(mChanB);
}
uint8_t Z80Sio::ReadControlB() {
    return ReadControl(mChanB);
}
void Z80Sio::WriteDataB(uint8_t val) {
    WriteData(mChanB, val);
}
void Z80Sio::WriteControlB(uint8_t val) {
    WriteControl(mChanB, val);
}
