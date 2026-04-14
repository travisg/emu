#pragma once

#include <cstdint>
#include <queue>

// Z80 SIO (Serial Input/Output) Controller
class Z80Sio {
  public:
    Z80Sio();
    ~Z80Sio() = default;

    uint8_t ReadDataA();
    uint8_t ReadControlA();
    void WriteDataA(uint8_t val);
    void WriteControlA(uint8_t val);

    uint8_t ReadDataB();
    uint8_t ReadControlB();
    void WriteDataB(uint8_t val);
    void WriteControlB(uint8_t val);

    // Provide keyboard data
    void InjectKeyboardByte(uint8_t val);

  private:
    struct Channel {
        uint8_t control_regs[8];
        uint8_t status_regs[3];
        uint8_t pointer;

        std::queue<uint8_t> rx_fifo;

        Channel() : pointer(0) {
            for (auto &r : control_regs) {
                r = 0;
            }
            for (auto &r : status_regs) {
                r = 0;
            }
            status_regs[0] = 0x04; // Tx buffer empty
        }
    };

    Channel mChanA; // Usually RS-232 port
    Channel mChanB; // Usually Keyboard in Kaypro

    uint8_t ReadControl(Channel &chan);
    void WriteControl(Channel &chan, uint8_t val);
    uint8_t ReadData(Channel &chan);
    void WriteData(Channel &chan, uint8_t val);
};
