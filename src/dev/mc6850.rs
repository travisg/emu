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
//! Port of `dev/mc6850.{h,cpp}`. Two behaviours here are Altair-680-monitor
//! specific rather than real ACIA semantics, and are preserved deliberately:
//! incoming LF is remapped to CR and lowercase is folded to upper case.

use crate::bus::MemoryDevice;
use crate::console::ConsoleEndpoint;

const STAT_RDRF: u8 = 1 << 0;
const STAT_TDRE: u8 = 1 << 1;

pub struct Mc6850 {
    console: ConsoleEndpoint,
    /// Latched receive byte. The C++ uses -1 for "empty".
    pending_rx: Option<u8>,
    status: u8,
    #[allow(dead_code)]
    control: u8,
}

impl Mc6850 {
    pub fn new(console: ConsoleEndpoint) -> Self {
        Mc6850 {
            console,
            pending_rx: None,
            // Transmit is modelled as always ready and never goes busy, so
            // TDRE is set once here and never cleared. There is a STAT_IRQ bit
            // in the C++ that is never set or read: this ACIA is polled only,
            // so no interrupt is modelled.
            status: STAT_TDRE,
            control: 0,
        }
    }
}

impl MemoryDevice for Mc6850 {
    fn read_byte(&mut self, addr: u32) -> u8 {
        // Note this runs for *any* register read, including the status
        // register -- reading status is what pulls a character out of the
        // console queue. Matches the C++.
        if self.pending_rx.is_none() {
            if let Some(c) = self.console.try_next_char() {
                let c = if c == 0x0a {
                    0x0d
                } else if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                self.pending_rx = Some(c);
            }
        }

        match addr {
            // status register
            0 => self.status | if self.pending_rx.is_some() { STAT_RDRF } else { 0 },
            // data register
            1 => self.pending_rx.take().unwrap_or(0),
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            // control register: the C++ stores it and ignores it -- no
            // baud-rate divider or word-format handling exists
            0 => self.control = val,
            1 => self.console.put_char(val & 0x7f),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn uart_with_input(input: &[u8]) -> Mc6850 {
        let (tx, rx) = mpsc::channel();
        for &c in input {
            tx.send(c).unwrap();
        }
        Mc6850::new(ConsoleEndpoint::new(rx, Box::new(Vec::new())))
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
    }

    #[test]
    fn data_register_returns_then_clears_the_byte() {
        let mut u = uart_with_input(b"A");
        assert_eq!(u.read_byte(1), b'A');
        // consumed: nothing left to report
        assert_eq!(u.read_byte(0) & STAT_RDRF, 0);
        assert_eq!(u.read_byte(1), 0);
    }

    #[test]
    fn lowercase_is_folded_to_upper() {
        let mut u = uart_with_input(b"abz");
        assert_eq!(u.read_byte(1), b'A');
        assert_eq!(u.read_byte(1), b'B');
        assert_eq!(u.read_byte(1), b'Z');
    }

    #[test]
    fn linefeed_is_remapped_to_carriage_return() {
        let mut u = uart_with_input(b"\n");
        assert_eq!(u.read_byte(1), 0x0d);
    }

    #[test]
    fn uppercase_and_digits_pass_through_untouched() {
        let mut u = uart_with_input(b"Q7");
        assert_eq!(u.read_byte(1), b'Q');
        assert_eq!(u.read_byte(1), b'7');
    }
}
