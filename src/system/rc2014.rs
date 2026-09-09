// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! RC2014 (Zilog Z80).
//!
//! Port of `system/system_rc2014.cpp`. The rom is a flat 64K binary -- the
//! `#include "ihex.h"` in the C++ file is vestigial, there is no parser behind
//! it.
//!
//! The serial port is the SIO/2 module at `$80`-`$83` (`dev/z80sio`), channel
//! A the console. The C++ had a single-byte receive latch inline instead; the
//! shared model replaced it. The chip is clocked straight off the 7.3728 MHz
//! system clock and the factory rom's WR4 divides by 64, which is 115200 baud:
//! a character every 640 cycles in either direction, and the console never
//! delivers faster than that. The rom drops RTS when its ring buffer is
//! nearly full, and the terminal on the other end of the cable honours it,
//! so a long paste waits rather than overruns.
//!
//! The SIO's INT is the IRQ line, and that is not decoration: the factory
//! rom's console input is *entirely* interrupt-driven. The rom programs WR1
//! for an interrupt on every received character; its mode-1 handler at
//! $0038 reads the data port into a 64-byte ring buffer at $8000 and RST 10h
//! at $00b3 spins on the buffer's count, so a machine that never interrupts
//! can never be typed at. The CTC module at `0x88`-`0x8b` (`dev/z80ctc`) sits
//! below the SIO on the interrupt daisy chain; nothing in the factory rom
//! programs it, and `test/run_rc2014_ctc_test.py` is what does.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::ConsoleEndpoint;
use crate::dev::memory::Memory;
use crate::dev::z80ctc::Z80Ctc;
use crate::dev::z80sio::{Ch, Z80Sio};
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

/// The system clock, which is also the SIO's serial clock input.
pub const CLOCK_HZ: u64 = 7_372_800;

const BANK_SIZE: usize = 64 * 1024;
/// Size of the rom window at the bottom of the address space.
const ROM_WINDOW: u16 = 0x2000;

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
    sio: Z80Sio,
    ctc: Z80Ctc,
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

        let mut sio = Z80Sio::new(CLOCK_HZ);
        for ch in [Ch::A, Ch::B] {
            sio.set_clock_hz(ch, CLOCK_HZ);
        }
        // the console is a terminal on a serial cable, and honours RTS
        sio.set_honours_rts(Ch::A, true);
        Ok(Rc2014 { ram: Memory::new(BANK_SIZE), rom, rom_bank: 0, console, sio, ctc: Z80Ctc::new() })
    }

    /// `--fast-io`: the serial port completes instantly in both directions.
    pub fn set_fast_io(&mut self) {
        self.sio.set_fast_io();
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

    /// Hand the console's keystrokes to the terminal end of channel A's
    /// line; the SIO clocks them in at the baud rate from there.
    fn poll_console(&mut self) {
        while let Some(c) = self.console.try_next_char() {
            self.sio.receive(Ch::A, c);
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

    /// The rom window and the ram; the SIO is port-mapped, so nothing in
    /// the memory space has a read side effect.
    fn peek8(&self, addr: u32) -> Option<u8> {
        let addr = (addr & 0xffff) as u16;
        if addr < ROM_WINDOW {
            Some(self.rom.peek(addr as u32 + self.rom_bank * ROM_WINDOW as u32))
        } else if addr >= 0x8000 {
            Some(self.ram.peek(addr as u32))
        } else {
            None
        }
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        match port & 0xff {
            // SIO/A control and data. The factory rom's output routine at
            // $0116 -- `in a,($80)` / `rrca` / `bit 1,a` / `jr z,-10` --
            // polls RR0's transmit-buffer-empty and prints nothing without
            // it; the SIO reports it a frame after each write.
            0x80 => self.sio.read_control(Ch::A),
            0x81 => self.sio.read_data(Ch::A),
            // SIO/B: the second serial port, with nothing on its line
            0x82 => self.sio.read_control(Ch::B),
            0x83 => self.sio.read_data(Ch::B),
            0x88..=0x8b => self.ctc.read((port & 0x03) as usize),
            0x90 | 0x91 => 0xff,
            _ => {
                eprintln!("in from unknown port {port:#x}");
                0xff
            }
        }
    }

    /// INT is the daisy chain: the SIO, then the CTC below it, so the CTC
    /// requests only while nothing in the SIO is under service. The SIO's
    /// request is level-held: with the rom's "interrupt on every
    /// character" it stays asserted until the handler reads the data port.
    /// The keystrokes reach the line, the frames advance and the timers
    /// count here, once an instruction.
    fn poll_interrupts(&mut self, elapsed_cycles: u32) -> IntStatus {
        self.poll_console();
        self.sio.tick(elapsed_cycles);
        self.ctc.tick(elapsed_cycles);
        let irq = self.sio.int_pending() || (!self.sio.under_service() && self.ctc.int_pending());
        IntStatus { irq, nmi: false }
    }

    /// The acknowledge goes to the first device on the chain that is
    /// requesting; nothing requesting is the pulled-up bus.
    fn interrupt_acknowledge(&mut self) -> u8 {
        if self.sio.int_pending() {
            self.sio.acknowledge()
        } else if self.ctc.int_pending() {
            self.ctc.acknowledge()
        } else {
            0xff
        }
    }

    /// RETI releases the device under service nearest the cpu.
    fn interrupt_return(&mut self) {
        if self.sio.under_service() {
            self.sio.reti();
        } else {
            self.ctc.reti();
        }
    }

    fn set_device_pacing_hz(&mut self, hz: u64) {
        self.sio.set_pacing_hz(hz);
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        match port & 0xff {
            // compact flash controller: accepted and ignored
            0x10..=0x17 => {}
            0x80 => self.sio.write_control(Ch::A, val),
            // SIO/A data: this is the console. The byte is out at once and
            // the SIO charges its frame time.
            0x81 => {
                self.console.put_char(val);
                self.sio.write_data(Ch::A, val);
            }
            0x82 => self.sio.write_control(Ch::B, val),
            0x83 => self.sio.write_data(Ch::B, val),
            0x88..=0x8b => self.ctc.write((port & 0x03) as usize, val),
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

    /// The debugger's peek follows the guest's decode: the rom window at
    /// the bottom, ram at the top, nothing in between.
    #[test]
    fn peek_is_none_in_the_unmapped_hole() {
        let (mut m, _tx) = build("peek");
        assert_eq!(m.peek8(0x0123), Some(0x23), "rom window, bank 0");
        assert_eq!(m.peek8(0x4000), None, "the hole");
        m.write8(0x9000, 0x42);
        assert_eq!(m.peek8(0x9000), Some(0x42), "ram");
    }

    // RR0 bits the factory rom looks at
    const RX_AVAILABLE: u8 = 1 << 0;
    const INT_PENDING: u8 = 1 << 1;
    const TX_EMPTY: u8 = 1 << 2;

    /// The frame time at 115200 baud, 8N1, in cycles of the 7.3728 MHz
    /// clock.
    const FRAME: u32 = 640;

    /// Program channel A the way the factory rom does at $0199: reset,
    /// x64 clock with one stop bit, an interrupt on every received
    /// character, eight bits in with the receiver on, eight bits out with
    /// the transmitter on and RTS up.
    fn init_sio(sys: &mut Rc2014) {
        for byte in [0x18, 0x04, 0xc4, 0x01, 0x18, 0x03, 0xe1, 0x05, 0xea] {
            sys.io_write8(0x80, byte);
        }
    }

    /// Port $80 reports "transmit buffer empty" (bit 2) when idle and a
    /// frame after each write. The factory rom's output routine at $0116
    /// polls that bit and spins forever without it, so the machine prints
    /// nothing at all if it goes missing.
    #[test]
    fn the_sio_status_reports_transmit_empty_a_frame_after_a_write() {
        let (mut sys, _tx) = build("txempty");
        init_sio(&mut sys);
        assert_ne!(sys.io_read8(0x80) & TX_EMPTY, 0);
        sys.io_write8(0x81, b'x');
        assert_eq!(sys.io_read8(0x80) & TX_EMPTY, 0);
        sys.poll_interrupts(FRAME - 1);
        assert_eq!(sys.io_read8(0x80) & TX_EMPTY, 0);
        sys.poll_interrupts(1);
        assert_ne!(sys.io_read8(0x80) & TX_EMPTY, 0);
    }

    /// A keystroke takes its frame time to arrive -- the console is a
    /// terminal at 115200 baud -- and then asserts IRQ until the data port
    /// is read. The factory rom's console input path is nothing but its
    /// mode-1 handler, so without the interrupt the machine prints its
    /// prompt and can never be typed at.
    #[test]
    fn a_character_arrives_at_the_baud_rate_and_asserts_irq_until_read() {
        let (mut sys, tx) = build("irq");
        init_sio(&mut sys);
        assert!(!sys.poll_interrupts(0).irq);

        tx.send(b'z').unwrap();
        assert!(!sys.poll_interrupts(FRAME - 1).irq, "still on the wire");
        assert_eq!(sys.io_read8(0x80) & RX_AVAILABLE, 0);
        let ints = sys.poll_interrupts(1);
        assert!(ints.irq);
        assert!(!ints.nmi, "nothing here drives NMI");
        assert_eq!(sys.io_read8(0x80) & (RX_AVAILABLE | INT_PENDING), RX_AVAILABLE | INT_PENDING);

        // level-held: still asserted on the next poll, until the guest reads
        assert!(sys.poll_interrupts(0).irq);
        assert_eq!(sys.io_read8(0x81), b'z');
        assert!(!sys.poll_interrupts(0).irq);
    }

    /// The rom drops RTS (WR5 bit 1) when its ring buffer is nearly full,
    /// and the terminal honours it: a paste waits on the wire instead of
    /// overrunning the FIFO.
    #[test]
    fn rts_off_holds_the_console() {
        let (mut sys, tx) = build("rts");
        init_sio(&mut sys);
        sys.io_write8(0x80, 0x05);
        sys.io_write8(0x80, 0xe8); // RTS off, as the rom's handler writes it
        tx.send(b'w').unwrap();
        assert!(!sys.poll_interrupts(100 * FRAME).irq);
        sys.io_write8(0x80, 0x05);
        sys.io_write8(0x80, 0xea);
        assert!(sys.poll_interrupts(FRAME).irq);
        assert_eq!(sys.io_read8(0x81), b'w');
    }

    /// The acknowledge cycle puts the SIO's receive source under service,
    /// which holds the next character back until the handler's RETI --
    /// the factory rom ends its handler with one for exactly this.
    #[test]
    fn a_second_character_waits_for_the_handlers_reti() {
        let (mut sys, tx) = build("ius");
        init_sio(&mut sys);
        tx.send(b'a').unwrap();
        tx.send(b'b').unwrap();
        assert!(sys.poll_interrupts(FRAME).irq);
        sys.interrupt_acknowledge();
        assert_eq!(sys.io_read8(0x81), b'a');
        assert!(!sys.poll_interrupts(FRAME).irq, "b has arrived, but the service holds it");
        sys.interrupt_return();
        assert!(sys.poll_interrupts(0).irq);
        assert_eq!(sys.io_read8(0x81), b'b');
    }

    /// The CTC sits below the SIO on the daisy chain: its request is held
    /// while the SIO is under service, the acknowledge goes to whichever
    /// is first, and RETI releases the nearer one.
    #[test]
    fn the_ctc_waits_behind_the_sio_on_the_chain() {
        let (mut sys, tx) = build("chain");
        init_sio(&mut sys);
        sys.io_write8(0x88, 0x40); // ctc vector
        sys.io_write8(0x88, 0xa5); // channel 0: interrupt, prescaler 256, constant follows
        sys.io_write8(0x88, 1);
        assert!(sys.poll_interrupts(256).irq, "the timer");
        assert_eq!(sys.interrupt_acknowledge(), 0x40);
        tx.send(b'k').unwrap();
        assert!(sys.poll_interrupts(FRAME).irq, "the SIO outranks the CTC's service");
        assert_eq!(sys.interrupt_acknowledge(), 0x00, "the SIO's vector, WR2 never written");
        assert!(!sys.poll_interrupts(256).irq, "the timer's next tick waits");
        sys.io_read8(0x81);
        sys.interrupt_return();
        assert!(!sys.poll_interrupts(0).irq, "the CTC is still under its own service");
        sys.interrupt_return();
        assert!(sys.poll_interrupts(0).irq, "and now the held tick");
        assert_eq!(sys.interrupt_acknowledge(), 0x40);
    }

    /// `--fast-io` takes the frame time off both directions.
    #[test]
    fn fast_io_makes_the_console_instant() {
        let (mut sys, tx) = build("fastio");
        init_sio(&mut sys);
        sys.set_fast_io();
        tx.send(b'f').unwrap();
        assert!(sys.poll_interrupts(0).irq);
        sys.io_write8(0x81, b'g');
        assert_ne!(sys.io_read8(0x80) & TX_EMPTY, 0);
    }
}
