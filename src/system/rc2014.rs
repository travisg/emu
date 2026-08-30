// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! RC2014 (Zilog Z80).
//!
//! Port of `system/system_rc2014.cpp`. The rom is a flat 64K binary -- the
//! `#include "ihex.h"` in the C++ file is vestigial, there is no parser behind
//! it.
//!
//! The serial port is hand-rolled here rather than reusing a device: the C++
//! machine has a single-byte receive latch inline (`mSIORecvByte`), and does
//! *not* use `dev/z80sio.*`, which despite the generic name is Kaypro-only.
//! The C++ fills that latch from a console callback on the console thread while
//! the CPU thread reads it, which is what made the mutex in `753dd4b`
//! necessary; here the latch is pulled from the channel on the CPU thread at
//! the point of the read, so there is nothing to synchronize.
//!
//! The SIO raises IRQ while the latch is full, and it is the only device in
//! the tree that drives `Bus::poll_interrupts` -- the C++ left a TODO where
//! the raise would go and never filled it in. That mattered: the factory rom's
//! console input is *entirely* interrupt-driven. Its mode-1 handler at $0038
//! reads the data port into a 64-byte ring buffer at $8000 and RST 10h at
//! $00b3 spins on the buffer's count, so a machine that never interrupts can
//! never be typed at, however full the receive latch gets.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

// from https://github.com/RC2014Z80/RC2014/tree/master/ROMs/Factory
//
// microsoft 32k basic for SIO/2, offset 0x0000
// microsoft 56k basic for SIO/2, offset 0x2000
// small computer monitor for pagable rom, 64k ram, at offset 0x4000 - 0x8000
// CP/M monitor for pageable rom for SIO/2 at offset 0x8000
// small computer monitor for everything at offset 0xe000
pub const DEFAULT_ROM: &str = "roms/rc2014/24886009.BIN";

const BANK_SIZE: usize = 64 * 1024;
/// Size of the rom window at the bottom of the address space.
const ROM_WINDOW: u16 = 0x2000;

// SIO/A status bits
const SIO_RX_AVAILABLE: u8 = 1 << 0;
const SIO_INT_PENDING: u8 = 1 << 1;
const SIO_TX_EMPTY: u8 = 1 << 2;

pub struct Rc2014 {
    ram: Memory,
    rom: Memory,
    /// Which 8K page of the rom image is visible at 0x0000.
    ///
    /// Nothing ever changes this: the C++ has no IO port that writes it, so it
    /// stays 0 for the machine's whole life. Kept as a field because the decode
    /// is written in terms of it, not because it is live.
    rom_bank: u32,
    console: ConsoleEndpoint,
    /// The single-byte receive latch. `None` is the C++ `!mSIORecvByte_valid`.
    sio_rx: Option<u8>,
}

impl Rc2014 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint) -> io::Result<Self> {
        let image = rom::load_binary(rom_path)?;
        // The C++ requires a full-size read: a short rom is an error, not a
        // partial load.
        if image.len() != BANK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "rom {} is {} bytes, expected exactly {}",
                    rom_path.display(),
                    image.len(),
                    BANK_SIZE
                ),
            ));
        }

        let mut rom = Memory::new(BANK_SIZE);
        rom.load_at(0, &image);

        Ok(Rc2014 { ram: Memory::new(BANK_SIZE), rom, rom_bank: 0, console, sio_rx: None })
    }

    /// The address decode, shared by reads and writes.
    ///
    ///   `0x0000..=0x1fff` rom, at `rom_bank * 0x2000`
    ///   `0x2000..=0x7fff` unmapped
    ///   `0x8000..=0xffff` ram, at offset 0 -- i.e. the *top* half of the 64K
    ///                     buffer, which is what the address itself indexes
    fn device_at(&mut self, addr: u16) -> Option<(&mut dyn MemoryDevice, u32)> {
        if addr < ROM_WINDOW {
            Some((&mut self.rom, addr as u32 + self.rom_bank * ROM_WINDOW as u32))
        } else if addr >= 0x8000 {
            Some((&mut self.ram, addr as u32))
        } else {
            None
        }
    }

    /// Top up the receive latch from the console.
    ///
    /// The C++ does this from a console-thread callback the moment a character
    /// lands in the input queue; doing it lazily on the read side is
    /// equivalent from the guest's point of view, since the latch is only ever
    /// observable through these two ports.
    fn poll_console(&mut self) {
        if self.sio_rx.is_none() {
            self.sio_rx = self.console.try_next_char();
        }
    }
}

impl Bus for Rc2014 {
    fn read8(&mut self, addr: u32) -> u8 {
        match self.device_at((addr & 0xffff) as u16) {
            Some((dev, a)) => dev.read_byte(a),
            None => 0,
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        if let Some((dev, a)) = self.device_at((addr & 0xffff) as u16) {
            dev.write_byte(a, val);
        }
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        match port & 0xff {
            // SIO/A control port: receive-available plus the interrupt
            // condition, which the guest polls because no interrupt is ever
            // actually raised, and transmit-buffer-empty.
            //
            // TX-empty is unconditional: transmit here is a synchronous write
            // to the console, so the buffer is always empty by the time the
            // guest can look. `dev/mc6850` reports its `TDRE` the same way.
            // The C++ never set this bit at all, which is why the factory
            // rom's output routine at $0116 -- `in a,($80)` / `rrca` /
            // `bit 1,a` / `jr z,-10` -- spun forever and the machine never
            // printed anything.
            0x80 => {
                self.poll_console();
                let mut status = SIO_TX_EMPTY;
                if self.sio_rx.is_some() {
                    status |= SIO_RX_AVAILABLE | SIO_INT_PENDING;
                }
                status
            }
            // SIO/A data port: the byte, and the latch clears
            0x81 => {
                self.poll_console();
                self.sio_rx.take().unwrap_or(0)
            }
            // SIO/B, and the second serial port on an SC129/SC110 (which may
            // instead be a CF controller or a CTC on an SC114/SC706)
            0x82 | 0x83 => 0,
            0x90 | 0x91 => 0xff,
            _ => {
                eprintln!("in from unknown port {port:#x}");
                0xff
            }
        }
    }

    /// IRQ is asserted while the receive latch is full. Level-held, not a
    /// pulse: the guest's handler clears it by reading the data port, which is
    /// what a real SIO does too.
    fn poll_interrupts(&mut self) -> IntStatus {
        self.poll_console();
        IntStatus { irq: self.sio_rx.is_some(), nmi: false, vector: 0 }
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        match port & 0xff {
            // compact flash controller: accepted and ignored
            0x10..=0x17 => {}
            // SIO/A control
            0x80 => {}
            // SIO/A data: this is the console
            0x81 => self.console.put_char(val),
            0x82 | 0x83 => {}
            0x90 | 0x91 => {}
            _ => eprintln!("out to unknown port {port:#x}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a machine over a synthetic full-size rom, and hand back the
    /// keystroke channel so a test can feed the SIO.
    fn build(name: &str) -> (Rc2014, std::sync::mpsc::Sender<u8>) {
        let dir = std::env::temp_dir().join(format!("emu-rc2014-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom_path = dir.join("rom.bin");
        // rom byte i = i, so reads are recognisable
        let image: Vec<u8> = (0..BANK_SIZE).map(|i| i as u8).collect();
        std::fs::File::create(&rom_path).unwrap().write_all(&image).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let machine = Rc2014::new(&rom_path, console).unwrap();
        std::fs::remove_file(&rom_path).ok();
        std::fs::remove_dir(&dir).ok();
        (machine, tx)
    }

    /// Port $80 reports "transmit buffer empty" (bit 2) unconditionally,
    /// alongside receive-available and the interrupt condition.
    ///
    /// This was the machine's defining defect until August 2026: the C++ this
    /// was ported from never set bit 2, and the factory rom's output routine
    /// at $0116 polls exactly that bit, so the monitor initialised the SIO and
    /// then spun there forever. The machine had never printed anything. A real
    /// SIO/2 reports it, and `dev/mc6850`'s `TDRE` is the same idea; this test
    /// is what keeps the bit from going away again.
    #[test]
    fn the_sio_status_always_reports_transmit_empty() {
        // latch empty: nothing to receive, but the transmitter is ready
        let (mut sys, tx) = build("txempty");
        assert_eq!(sys.io_read8(0x80), SIO_TX_EMPTY);

        // latch full: all three bits
        tx.send(b'q').unwrap();
        assert_eq!(sys.io_read8(0x80), SIO_RX_AVAILABLE | SIO_INT_PENDING | SIO_TX_EMPTY);

        // and it stays set across the read that drains the latch
        assert_eq!(sys.io_read8(0x81), b'q');
        assert_eq!(sys.io_read8(0x80), SIO_TX_EMPTY);
    }

    /// The other half of the same repair: a waiting character asserts IRQ,
    /// and reading the data port drops it again. The factory rom's console
    /// input path is nothing but its mode-1 handler, so without this the
    /// machine prints its prompt and can never be typed at.
    #[test]
    fn a_waiting_character_asserts_irq_until_the_data_port_is_read() {
        let (mut sys, tx) = build("irq");
        assert!(!sys.poll_interrupts().irq);

        tx.send(b'z').unwrap();
        let ints = sys.poll_interrupts();
        assert!(ints.irq);
        assert!(!ints.nmi, "nothing here drives NMI");

        // level-held: still asserted on the next poll, until the guest reads
        assert!(sys.poll_interrupts().irq);
        assert_eq!(sys.io_read8(0x81), b'z');
        assert!(!sys.poll_interrupts().irq);
    }
}
