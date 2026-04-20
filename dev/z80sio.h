#pragma once

#include <mutex>
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

    // Provide keyboard/serial data
    void InjectCharA(uint8_t val);
    void InjectCharB(uint8_t val);

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
            status_regs[0] = 0x2c; // Tx buffer empty (0x04) | DCD (0x08) | CTS (0x20)
        }
    };

    Channel mChanA; // Usually RS-232 port (Port 0x04/0x06 on Kaypro II)
    Channel mChanB; // Usually Keyboard in Kaypro

    std::recursive_mutex mLock;

    uint8_t ReadControl(Channel &chan);
    void WriteControl(Channel &chan, uint8_t val);
    uint8_t ReadData(Channel &chan);
    void WriteData(Channel &chan, uint8_t val);
};
