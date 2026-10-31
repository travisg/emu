// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Motorola MC6850 ACIA (serial port).
//!
//! Port of `dev/mc6850.{h,cpp}`, with the chip's interrupt side added: the
//! control register's master reset and its receive and transmit interrupt
//! enables, the IRQ bit in the status register, and the IRQ output a
//! machine wires to its CPU or not (`irq`). A character reaches the receive
//! data register when the machine polls (`poll`), not when the guest reads
//! a register, so it can raise IRQ before anyone looks; reading the data
//! register empties it, and so does master reset, which also holds the
//! receiver until the guest programs the chip.
//!
//! Not modelled: the baud rate (a character arrives as soon as the
//! register is empty, and transmit is always ready), word formats, framing
//! and overrun errors, and the modem lines. Two behaviours are
//! Altair-680-monitor specific rather than real ACIA semantics, and are
//! preserved deliberately: incoming LF is remapped to CR and lowercase is
//! folded to upper case.

use crate::bus::MemoryDevice;
use crate::console::ConsoleEndpoint;

const STAT_RDRF: u8 = 1 << 0;
const STAT_TDRE: u8 = 1 << 1;
const STAT_IRQ: u8 = 1 << 7;

/// Counter divide select 0b11: master reset, which keeps IRQ off until the
/// next control write selects a divide.
const CTRL_MASTER_RESET: u8 = 0b11;
/// Transmit control 0b01: RTS low and the transmit interrupt enabled.
const CTRL_TIE: u8 = 0b01 << 5;
const CTRL_TC_MASK: u8 = 0b11 << 5;
const CTRL_RIE: u8 = 1 << 7;

pub struct Mc6850 {
    console: ConsoleEndpoint,
    /// The receive data register; full is RDRF.
    rx: Option<u8>,
    control: u8,
}

impl Mc6850 {
    pub fn new(console: ConsoleEndpoint) -> Self {
        // Powers up in master reset, as the data sheet asks software to
        // assume: no interrupt until the guest programs it.
        Mc6850 { console, rx: None, control: CTRL_MASTER_RESET }
    }

    fn in_master_reset(&self) -> bool {
        self.control & CTRL_MASTER_RESET == CTRL_MASTER_RESET
    }

    /// Take the next character from the console into an empty receive
    /// register. The machine calls this between instructions. In master
    /// reset the receiver is held, and the character waits at the console.
    pub fn poll(&mut self) {
        if self.rx.is_none() && !self.in_master_reset() {
            if let Some(c) = self.console.try_next_char() {
                let c = if c == 0x0a {
                    0x0d
                } else if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                self.rx = Some(c);
            }
        }
    }

    /// The IRQ output, asserted: a full receive register with the receive
    /// interrupt enabled, or the always-empty transmit register with the
    /// transmit interrupt enabled -- never in master reset.
    pub fn irq(&self) -> bool {
        if self.in_master_reset() {
            return false;
        }
        (self.control & CTRL_RIE != 0 && self.rx.is_some()) || self.control & CTRL_TC_MASK == CTRL_TIE
    }

    fn status(&self) -> u8 {
        let mut s = STAT_TDRE;
        if self.rx.is_some() {
            s |= STAT_RDRF;
        }
        if self.irq() {
            s |= STAT_IRQ;
        }
        s
    }
}

impl crate::snapshot::Snapshot for Mc6850 {
    fn save(&self, w: &mut crate::snapshot::Writer) {
        let Mc6850 { console: _session, rx, control } = self;
        w.opt(rx, |w, c| w.u8(*c));
        w.u8(*control);
    }

    fn load(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        self.rx = r.opt(|r| r.u8())?;
        self.control = r.u8()?;
        Ok(())
    }
}

impl MemoryDevice for Mc6850 {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0 => self.status(),
            1 => self.rx.take().unwrap_or(0),
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            0 => {
                self.control = val;
                // master reset empties the receiver (data sheet: RDRF is
                // cleared by a data read or a master reset)
                if self.in_master_reset() {
                    self.rx = None;
                }
            }
            1 => self.console.put_char(val & 0x7f),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// An ACIA out of master reset, receive interrupt off, `input` waiting
    /// at the console and the first character already polled in.
    fn uart_with_input(input: &[u8]) -> Mc6850 {
        let (tx, rx) = mpsc::channel();
        for &c in input {
            tx.send(c).unwrap();
        }
        let mut u = Mc6850::new(ConsoleEndpoint::new(rx, Box::new(Vec::new())));
        u.write_byte(0, 0x15); // divide by 16, 8N1, RTS low, no interrupts
        u.poll();
        u
    }

    /// Read the data register, then poll the next character in, as the
    /// machine would between the guest's reads.
    fn read_data(u: &mut Mc6850) -> u8 {
        let c = u.read_byte(1);
        u.poll();
        c
    }

    #[test]
    fn transmit_is_always_ready() {
        let mut u = uart_with_input(&[]);
        assert_eq!(u.read_byte(0) & STAT_TDRE, STAT_TDRE);
    }

    #[test]
    fn status_reports_rdrf_only_once_a_character_is_waiting() {
        let mut u = uart_with_input(&[]);
        assert_eq!(u.read_byte(0) & STAT_RDRF, 0);
        let mut u = uart_with_input(b"A");
        assert_eq!(u.read_byte(0) & STAT_RDRF, STAT_RDRF);
        assert_eq!(u.read_byte(0) & STAT_RDRF, STAT_RDRF, "reading status takes nothing");
    }

    #[test]
    fn data_register_returns_then_clears_the_byte() {
        let mut u = uart_with_input(b"A");
        assert_eq!(u.read_byte(1), b'A');
        // consumed: nothing left to report
        assert_eq!(u.read_byte(0) & STAT_RDRF, 0);
        assert_eq!(u.read_byte(1), 0);
    }

    /// The receive interrupt: IRQ while a character waits with RIE set,
    /// in the status register and on the pin, dropped by reading the data.
    #[test]
    fn a_waiting_character_raises_irq_when_the_receive_interrupt_is_enabled() {
        let mut u = uart_with_input(b"A");
        assert!(!u.irq(), "RIE clear");
        u.write_byte(0, 0x95); // what the 6809 BASIC writes: RIE, 8N1, /16
        assert!(u.irq());
        assert_eq!(u.read_byte(0) & STAT_IRQ, STAT_IRQ);
        assert_eq!(u.read_byte(1), b'A');
        assert!(!u.irq(), "reading the data clears it");
        assert_eq!(u.read_byte(0) & STAT_IRQ, 0);
    }

    /// The transmit interrupt, enabled by transmit control 01, is asserted
    /// for as long as the (never busy) transmitter is empty.
    #[test]
    fn the_transmit_interrupt_follows_the_transmit_control_bits() {
        let mut u = uart_with_input(&[]);
        u.write_byte(0, 0x35); // TC = 01
        assert!(u.irq());
        u.write_byte(0, 0x55); // TC = 10: RTS high, no interrupt
        assert!(!u.irq());
    }

    /// The part powers up in master reset, and in it receives nothing: a
    /// character typed before the guest programs the chip waits for it.
    #[test]
    fn master_reset_receives_nothing() {
        let (tx, rx) = mpsc::channel();
        tx.send(b'A').unwrap();
        let mut u = Mc6850::new(ConsoleEndpoint::new(rx, Box::new(Vec::new())));
        u.poll();
        assert_eq!(u.read_byte(0) & STAT_RDRF, 0, "power-up is master reset");
        u.write_byte(0, 0x83); // RIE with master reset
        u.poll();
        assert!(!u.irq());
        assert_eq!(u.read_byte(0) & STAT_RDRF, 0);
        u.write_byte(0, 0x81);
        u.poll();
        assert!(u.irq(), "out of reset the character comes in");
        assert_eq!(u.read_byte(1), b'A');
    }

    /// Master reset empties a full receiver: RDRF and IRQ drop, and stay
    /// down when the receive interrupt comes back on. The next character
    /// is still waiting.
    #[test]
    fn master_reset_clears_a_waiting_character() {
        let mut u = uart_with_input(b"AB");
        u.write_byte(0, 0x95);
        assert!(u.irq());
        u.write_byte(0, 0x03);
        assert_eq!(u.read_byte(0) & (STAT_RDRF | STAT_IRQ), 0);
        u.write_byte(0, 0x95);
        assert!(!u.irq());
        u.poll();
        assert_eq!(u.read_byte(1), b'B');
    }

    #[test]
    fn lowercase_is_folded_to_upper() {
        let mut u = uart_with_input(b"abz");
        assert_eq!(read_data(&mut u), b'A');
        assert_eq!(read_data(&mut u), b'B');
        assert_eq!(read_data(&mut u), b'Z');
    }

    #[test]
    fn linefeed_is_remapped_to_carriage_return() {
        let mut u = uart_with_input(b"\n");
        assert_eq!(u.read_byte(1), 0x0d);
    }

    #[test]
    fn uppercase_and_digits_pass_through_untouched() {
        let mut u = uart_with_input(b"Q7");
        assert_eq!(read_data(&mut u), b'Q');
        assert_eq!(read_data(&mut u), b'7');
    }
}
