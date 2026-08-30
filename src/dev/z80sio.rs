// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Zilog Z80 SIO/2 dual-channel serial controller (Kaypro II).
//!
//! Port of `dev/z80sio.{h,cpp}`. Despite the generic name this device is used
//! by the Kaypro only; the RC2014 hand-rolls its own single-byte SIO inline.
//!
//! The model is shallow: a WR0 register-pointer, the write registers stored
//! but not interpreted, three read registers of which RR0 carries the only
//! live bits (rx available, tx empty), and a receive FIFO per channel.
//! Transmit is instantaneous and goes nowhere. On the Kaypro, channel A is the
//! RS-232 port and channel B is the keyboard.
//!
//! The C++ guards everything with a recursive mutex because the console thread
//! injects keystrokes directly. Here injection happens on the CPU thread (the
//! machine pulls from its console channel and calls `inject_*` at the point of
//! the port access), so there is nothing to lock.

use std::collections::VecDeque;

// RR0 bits
const RR0_RX_AVAILABLE: u8 = 1 << 0;
const RR0_TX_EMPTY: u8 = 1 << 2;
const RR0_DCD: u8 = 1 << 3;
const RR0_CTS: u8 = 1 << 5;

struct Channel {
    control_regs: [u8; 8],
    status_regs: [u8; 3],
    /// WR0 register pointer: which register the next control access hits.
    pointer: u8,
    rx_fifo: VecDeque<u8>,
}

impl Channel {
    fn new() -> Self {
        Channel {
            control_regs: [0; 8],
            // tx buffer empty, carrier detect and clear-to-send all asserted
            status_regs: [RR0_TX_EMPTY | RR0_DCD | RR0_CTS, 0, 0],
            pointer: 0,
            rx_fifo: VecDeque::new(),
        }
    }

    fn read_control(&mut self) -> u8 {
        // Only RR0..RR2 exist. The C++ indexes its 3-entry array with the
        // raw pointer (an out-of-bounds read for RR3..RR7); here those read
        // as zero.
        let val = self.status_regs.get(self.pointer as usize).copied().unwrap_or(0);
        // the pointer resets after every register access
        self.pointer = 0;
        val
    }

    fn write_control(&mut self, val: u8) {
        if self.pointer == 0 {
            // WR0: low 3 bits select the next register, bits 3-5 are a command
            self.pointer = val & 0x07;
            match (val >> 3) & 0x07 {
                // channel reset
                0b011 => *self = Channel::new(),
                // error reset: clear the RR1 error bits
                0b110 => self.status_regs[1] &= 0x8f,
                // reset rx crc, send abort, enable int on next rx, reset tx
                // int pending, return from int: all accepted and ignored
                _ => {}
            }
        } else {
            self.control_regs[self.pointer as usize] = val;
            self.pointer = 0;
        }
    }

    fn read_data(&mut self) -> u8 {
        let val = self.rx_fifo.pop_front().unwrap_or(0);
        if self.rx_fifo.is_empty() {
            self.status_regs[0] &= !RR0_RX_AVAILABLE;
        }
        val
    }

    fn write_data(&mut self, _val: u8) {
        // consumed instantly into the ether
        self.status_regs[0] |= RR0_TX_EMPTY;
    }

    fn inject(&mut self, val: u8) {
        self.rx_fifo.push_back(val);
        self.status_regs[0] |= RR0_RX_AVAILABLE;
    }
}

pub struct Z80Sio {
    chan_a: Channel,
    chan_b: Channel,
}

impl Default for Z80Sio {
    fn default() -> Self {
        Self::new()
    }
}

impl Z80Sio {
    pub fn new() -> Self {
        Z80Sio { chan_a: Channel::new(), chan_b: Channel::new() }
    }

    pub fn read_data_a(&mut self) -> u8 {
        self.chan_a.read_data()
    }
    pub fn read_control_a(&mut self) -> u8 {
        self.chan_a.read_control()
    }
    pub fn write_data_a(&mut self, val: u8) {
        self.chan_a.write_data(val)
    }
    pub fn write_control_a(&mut self, val: u8) {
        self.chan_a.write_control(val)
    }

    pub fn read_data_b(&mut self) -> u8 {
        self.chan_b.read_data()
    }
    pub fn read_control_b(&mut self) -> u8 {
        self.chan_b.read_control()
    }
    pub fn write_data_b(&mut self, val: u8) {
        self.chan_b.write_data(val)
    }
    pub fn write_control_b(&mut self, val: u8) {
        self.chan_b.write_control(val)
    }

    /// Queue a received byte on channel A (RS-232).
    pub fn inject_char_a(&mut self, val: u8) {
        self.chan_a.inject(val)
    }

    /// Queue a received byte on channel B (the keyboard on a Kaypro).
    pub fn inject_char_b(&mut self, val: u8) {
        self.chan_b.inject(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_reports_tx_empty_and_no_rx() {
        let mut sio = Z80Sio::new();
        let rr0 = sio.read_control_b();
        assert_ne!(rr0 & RR0_TX_EMPTY, 0);
        assert_eq!(rr0 & RR0_RX_AVAILABLE, 0);
    }

    #[test]
    fn injected_bytes_come_out_in_order_and_clear_the_flag() {
        let mut sio = Z80Sio::new();
        sio.inject_char_b(b'a');
        sio.inject_char_b(b'b');
        assert_ne!(sio.read_control_b() & RR0_RX_AVAILABLE, 0);
        assert_eq!(sio.read_data_b(), b'a');
        assert_ne!(sio.read_control_b() & RR0_RX_AVAILABLE, 0, "one still queued");
        assert_eq!(sio.read_data_b(), b'b');
        assert_eq!(sio.read_control_b() & RR0_RX_AVAILABLE, 0);
        assert_eq!(sio.read_data_b(), 0, "empty fifo reads as zero");
    }

    #[test]
    fn channels_are_independent() {
        let mut sio = Z80Sio::new();
        sio.inject_char_a(0x55);
        assert_eq!(sio.read_control_b() & RR0_RX_AVAILABLE, 0);
        assert_eq!(sio.read_data_a(), 0x55);
    }

    #[test]
    fn register_pointer_selects_then_resets() {
        let mut sio = Z80Sio::new();
        // point at RR1 (all zero), read it, then the pointer is back at RR0
        sio.write_control_a(0x01);
        assert_eq!(sio.read_control_a(), 0);
        assert_ne!(sio.read_control_a() & RR0_TX_EMPTY, 0);
        // pointer at WR3, then a write lands there and resets the pointer
        sio.write_control_a(0x03);
        sio.write_control_a(0xc1);
        assert_eq!(sio.chan_a.control_regs[3], 0xc1);
        assert_eq!(sio.chan_a.pointer, 0);
    }

    #[test]
    fn channel_reset_drops_pending_input() {
        let mut sio = Z80Sio::new();
        sio.inject_char_b(0x11);
        sio.write_control_b(0b011 << 3);
        assert_eq!(sio.read_control_b() & RR0_RX_AVAILABLE, 0);
        assert_eq!(sio.read_data_b(), 0);
    }
}
