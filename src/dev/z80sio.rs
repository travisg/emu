// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Zilog Z80 SIO/2 dual-channel serial controller.
//!
//! Descends from the port of `dev/z80sio.{h,cpp}`, but is a model of the chip
//! rather than a byte latch behind a status bit. Shared by the Kaypro II
//! (channel A the RS-232 port, channel B the keyboard) and the RC2014
//! (channel A the console). Written from the Zilog SIO technical manual; the
//! register bits are cited where they are decoded.
//!
//! What is live:
//!
//! - **Time.** A character takes its frame time on the line in both
//!   directions -- start, data, parity and stop bits (WR3/WR4/WR5) at the
//!   baud rate the channel's clock input and WR4's divisor make. The far end
//!   sends no faster than that, so a burst pasted at the machine arrives one
//!   character per frame time, and the transmit buffer stays full for a
//!   frame after each write. The cycles come from the bus's interrupt poll
//!   (`tick`) and the rate they are issued at from `set_pacing_hz`, the same
//!   arrangement as the 703's teletype, so a baud rate is a baud rate of wall
//!   clock at any `--throttle`; `set_fast_io` makes both directions instant.
//! - **Interrupts.** WR1's receive modes (none, first character, every
//!   character), the transmit-buffer-empty interrupt, the receive overrun as
//!   a special receive condition, the fixed priority between sources and
//!   channels, WR2's vector and "status affects vector" (WR1 bit 2 of channel
//!   B, both channels' sources encoded in bits 3-1), and RR0's
//!   interrupt-pending bit on channel A. The acknowledge cycle sets the
//!   requesting source's interrupt-under-service bit, which holds back
//!   that source and everything below it until `RETI` (or channel A's
//!   "return from interrupt" command) clears the highest one -- the daisy
//!   chain as far as one chip goes. Nothing above the SIO on the chain is
//!   modelled, so IEI is always high.
//! - **Flow control**, optionally: a channel whose far end honours RTS
//!   (`set_honours_rts`, a terminal on a modem cable) receives nothing while
//!   WR5's RTS is off. A keyboard has no such input and ignores it.
//! - The three-deep receive FIFO, overrun into RR1, and the WR0 commands
//!   that touch any of the above.
//!
//! What is not: synchronous modes, CRC, the modem inputs (DCD and CTS read
//! asserted), break, parity and framing errors, external/status interrupts
//! (nothing here ever changes), and the transmitter's enable bit -- output
//! is passed to the machine on the write regardless.
//!
//! Two deliberate softenings, both because the far end here is a person at
//! a terminal rather than a modem: a byte typed while the receiver is
//! disabled (WR3 bit 0 clear, as it is between a channel reset and the
//! `WR3` that follows) waits on the line instead of being lost, and the
//! line's queue is unbounded. Everything past the FIFO is faithful: three
//! characters unread and the fourth overwrites the last and flags the
//! overrun.
//!
//! Injection happens on the CPU thread (the machine pulls from its console
//! channel and calls `receive` at its interrupt poll), so there is nothing
//! to lock.

use std::collections::VecDeque;

/// The two channels, and their fixed interrupt priority: A above B.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ch {
    A = 0,
    B = 1,
}

// RR0 bits (technical manual, "Read Register 0")
const RR0_RX_AVAILABLE: u8 = 1 << 0;
/// Channel A only: any interrupt pending in the device.
const RR0_INT_PENDING: u8 = 1 << 1;
const RR0_TX_EMPTY: u8 = 1 << 2;
const RR0_DCD: u8 = 1 << 3;
const RR0_CTS: u8 = 1 << 5;

// RR1 bits
const RR1_ALL_SENT: u8 = 1 << 0;
const RR1_RX_OVERRUN: u8 = 1 << 5;

// WR1 bits
const WR1_TX_INT_ENABLE: u8 = 1 << 1;
/// Channel B only; governs the vector both channels' interrupts present.
const WR1_STATUS_AFFECTS_VECTOR: u8 = 1 << 2;
/// Bits 4-3: 00 none, 01 first character, 10 all characters (parity error
/// is a special condition), 11 all characters (parity error is not).
const WR1_RX_INT_MODE_SHIFT: u8 = 3;

// WR3 bits
const WR3_RX_ENABLE: u8 = 1 << 0;

// WR5 bits
const WR5_RTS: u8 = 1 << 1;

/// Depth of the receive FIFO.
const RX_FIFO_DEPTH: usize = 3;

/// The interrupt sources, in the chip's priority order within a channel.
/// The discriminant is the V3-V1 code "status affects vector" writes into
/// the vector for channel B; channel A's is the same plus 4. Code 1 is
/// external/status, which nothing here raises.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Source {
    TxEmpty = 0,
    RxChar = 2,
    RxSpecial = 3,
}

struct Channel {
    wr: [u8; 8],
    /// WR0 register pointer: which register the next control access hits.
    pointer: u8,
    /// Bytes the far end has to send, not yet clocked in: the terminal's
    /// output queue.
    line: VecDeque<u8>,
    /// Cycles until the byte at the head of `line` lands in the FIFO; only
    /// meaningful while the line has one.
    rx_remaining: u32,
    rx_fifo: VecDeque<u8>,
    rx_overrun: bool,
    /// "Interrupt on first received character": armed by a channel reset
    /// and by WR0's "enable interrupt on next receive character", spent by
    /// the read of that character.
    rx_first_armed: bool,
    /// Cycles until the transmit buffer is empty again; 0 is empty.
    tx_remaining: u32,
    tx_int_pending: bool,
    /// The far end honours RTS: nothing arrives while WR5's RTS is off.
    honours_rts: bool,
    /// The RxC/TxC clock input, in Hz. Zero until the machine gives one,
    /// which is instant in both directions.
    clock_hz: u64,
    /// Cycles per character on the line at the current pacing rate, or 0 for
    /// instant.
    char_cycles: u32,
}

impl Channel {
    fn new(clock_hz: u64, honours_rts: bool) -> Self {
        Channel {
            wr: [0; 8],
            pointer: 0,
            line: VecDeque::new(),
            rx_remaining: 0,
            rx_fifo: VecDeque::new(),
            rx_overrun: false,
            rx_first_armed: true,
            tx_remaining: 0,
            tx_int_pending: false,
            honours_rts,
            clock_hz,
            char_cycles: 0,
        }
    }

    /// A channel reset (WR0 command 011): the registers, the FIFO and the
    /// interrupt state go; the line keeps what the far end has not sent yet
    /// and the frame in flight, and the clock and the far end's nature are
    /// wiring.
    fn reset(&mut self) {
        let line = std::mem::take(&mut self.line);
        let rx_remaining = self.rx_remaining;
        *self = Channel::new(self.clock_hz, self.honours_rts);
        self.line = line;
        self.rx_remaining = rx_remaining;
    }

    fn rx_enabled(&self) -> bool {
        self.wr[3] & WR3_RX_ENABLE != 0
    }

    fn rx_int_mode(&self) -> u8 {
        (self.wr[1] >> WR1_RX_INT_MODE_SHIFT) & 3
    }

    /// The character time in cycles. WR4 bits 7-6 divide the clock input:
    /// 00 x1, 01 x16, 10 x32, 11 x64. The frame is a start bit, WR3 bits
    /// 7-6's data bits for the receiver (00 five, 01 seven, 10 six, 11
    /// eight -- the manual's own order), a parity bit if WR4 bit 0, and
    /// WR4 bits 3-2's stop bits (01 one, 10 one and a half, 11 two; 00 is
    /// synchronous, taken as one). Counted in half bits for the one and a
    /// half.
    fn frame_cycles(&self, pacing_hz: u64) -> u32 {
        if self.clock_hz == 0 {
            return 0;
        }
        let divisor = match self.wr[4] >> 6 {
            0 => 1,
            1 => 16,
            2 => 32,
            _ => 64,
        };
        let data_bits = match self.wr[3] >> 6 {
            0 => 5,
            1 => 7,
            2 => 6,
            _ => 8,
        };
        let parity = (self.wr[4] & 1) as u64;
        let stop_half_bits = match (self.wr[4] >> 2) & 3 {
            2 => 3,
            3 => 4,
            _ => 2,
        };
        let half_bits = 2 * (1 + data_bits + parity) + stop_half_bits;
        let baud = self.clock_hz / divisor;
        (pacing_hz * half_bits / (2 * baud.max(1))) as u32
    }

    fn refresh(&mut self, pacing_hz: u64, fast_io: bool) {
        self.char_cycles = if fast_io { 0 } else { self.frame_cycles(pacing_hz) };
    }

    /// Advance the line by `elapsed` cycles: clock in what has had its frame
    /// time, finish the character being sent.
    fn tick(&mut self, elapsed: u32) {
        if self.tx_remaining > 0 {
            self.tx_remaining = self.tx_remaining.saturating_sub(elapsed);
            if self.tx_remaining == 0 && self.wr[1] & WR1_TX_INT_ENABLE != 0 {
                self.tx_int_pending = true;
            }
        }
        // The far end waits for RTS if it honours it, and this model also
        // waits for the receiver to be enabled (see the module doc).
        let held = !self.rx_enabled() || (self.honours_rts && self.wr[5] & WR5_RTS == 0);
        let mut budget = elapsed;
        while !self.line.is_empty() && !held {
            if self.rx_remaining > budget {
                self.rx_remaining -= budget;
                break;
            }
            budget -= self.rx_remaining;
            let byte = self.line.pop_front().unwrap();
            self.rx_remaining = self.char_cycles;
            if self.rx_fifo.len() >= RX_FIFO_DEPTH {
                // overrun: the newest character overwrites the last one in
                self.rx_fifo.pop_back();
                self.rx_overrun = true;
            }
            self.rx_fifo.push_back(byte);
        }
    }

    /// The far end sends a byte. An idle line starts a frame now.
    fn receive(&mut self, byte: u8) {
        if self.line.is_empty() {
            self.rx_remaining = self.char_cycles;
        }
        self.line.push_back(byte);
    }

    fn read_data(&mut self) -> u8 {
        let val = self.rx_fifo.pop_front().unwrap_or(0);
        if self.rx_int_mode() == 1 {
            self.rx_first_armed = false;
        }
        val
    }

    /// The byte itself is the machine's to deliver; this is its time on the
    /// wire. A write into a full buffer overwrites, as on the chip.
    fn write_data(&mut self) {
        self.tx_remaining = self.char_cycles;
        self.tx_int_pending = false;
        if self.char_cycles == 0 && self.wr[1] & WR1_TX_INT_ENABLE != 0 {
            self.tx_int_pending = true;
        }
    }

    fn write_control(&mut self, val: u8, pacing_hz: u64, fast_io: bool) {
        if self.pointer == 0 {
            // WR0: low 3 bits select the next register, bits 5-3 are a
            // command, bits 7-6 CRC controls this model has no use for
            self.pointer = val & 0x07;
            match (val >> 3) & 0x07 {
                // reset external/status interrupts: none are raised here
                0b010 => {}
                0b011 => self.reset(),
                // enable interrupt on next receive character
                0b100 => self.rx_first_armed = true,
                // reset transmit interrupt pending
                0b101 => self.tx_int_pending = false,
                // error reset
                0b110 => self.rx_overrun = false,
                // null, send abort, return from interrupt (no IUS here)
                _ => {}
            }
        } else {
            self.wr[self.pointer as usize] = val;
            self.pointer = 0;
            self.refresh(pacing_hz, fast_io);
        }
    }

    fn rr0(&self, int_pending: bool) -> u8 {
        let mut val = RR0_DCD | RR0_CTS;
        if !self.rx_fifo.is_empty() {
            val |= RR0_RX_AVAILABLE;
        }
        if int_pending {
            val |= RR0_INT_PENDING;
        }
        if self.tx_remaining == 0 {
            val |= RR0_TX_EMPTY;
        }
        val
    }

    fn rr1(&self) -> u8 {
        let mut val = 0;
        if self.tx_remaining == 0 {
            val |= RR1_ALL_SENT;
        }
        if self.rx_overrun {
            val |= RR1_RX_OVERRUN;
        }
        val
    }

    /// The highest-priority interrupt this channel is requesting.
    fn pending(&self) -> Option<Source> {
        let mode = self.rx_int_mode();
        if mode != 0 {
            if self.rx_overrun {
                return Some(Source::RxSpecial);
            }
            if !self.rx_fifo.is_empty() && (mode != 1 || self.rx_first_armed) {
                return Some(Source::RxChar);
            }
        }
        if self.tx_int_pending {
            return Some(Source::TxEmpty);
        }
        None
    }
}

pub struct Z80Sio {
    chan: [Channel; 2],
    /// Interrupt under service, by priority rank (`rank`): a source is
    /// acknowledged into it and `RETI` takes the highest out.
    ius: [bool; 4],
    /// The rate machine cycles are issued at: the machine's own clock until
    /// `set_pacing_hz` says otherwise.
    pacing_hz: u64,
    fast_io: bool,
}

impl Z80Sio {
    /// `clock_hz` is the machine's clock rate, what a character time is
    /// counted against until a pacing rate arrives. The channels have no
    /// serial clock until `set_clock_hz`, and are instant until then.
    pub fn new(clock_hz: u64) -> Self {
        Z80Sio {
            chan: [Channel::new(0, false), Channel::new(0, false)],
            ius: [false; 4],
            pacing_hz: clock_hz,
            fast_io: false,
        }
    }

    /// The channel's RxC/TxC input. A Kaypro's baud rate generator runs at
    /// sixteen times the baud and its WR4 divides by sixteen; the RC2014
    /// feeds its system clock straight in and divides by sixty-four.
    pub fn set_clock_hz(&mut self, ch: Ch, hz: u64) {
        let c = &mut self.chan[ch as usize];
        c.clock_hz = hz;
        c.refresh(self.pacing_hz, self.fast_io);
    }

    /// Whether the far end of the channel honours RTS. A terminal on a modem
    /// cable does; a keyboard has no such input.
    pub fn set_honours_rts(&mut self, ch: Ch, honours: bool) {
        self.chan[ch as usize].honours_rts = honours;
    }

    /// Pace the character times against `pacing_hz` cycles to the second,
    /// the resolved `--throttle`, so a baud rate stays a baud rate of wall
    /// clock however fast the cpu runs.
    pub fn set_pacing_hz(&mut self, pacing_hz: u64) {
        self.pacing_hz = pacing_hz;
        for c in &mut self.chan {
            c.refresh(pacing_hz, self.fast_io);
        }
    }

    /// `--fast-io`: both directions instant. Held as a flag of its own so a
    /// pacing rate arriving later cannot quietly put the time back.
    pub fn set_fast_io(&mut self) {
        self.fast_io = true;
        for c in &mut self.chan {
            c.refresh(self.pacing_hz, true);
        }
    }

    /// Advance both channels by `elapsed` machine cycles.
    pub fn tick(&mut self, elapsed: u32) {
        for c in &mut self.chan {
            c.tick(elapsed);
        }
    }

    /// The far end sends a byte on the channel.
    pub fn receive(&mut self, ch: Ch, byte: u8) {
        self.chan[ch as usize].receive(byte)
    }

    pub fn read_data(&mut self, ch: Ch) -> u8 {
        self.chan[ch as usize].read_data()
    }

    /// The machine delivers the byte itself; this charges its frame time.
    pub fn write_data(&mut self, ch: Ch, _val: u8) {
        self.chan[ch as usize].write_data()
    }

    pub fn write_control(&mut self, ch: Ch, val: u8) {
        let (pacing_hz, fast_io) = (self.pacing_hz, self.fast_io);
        // WR0 command 111, return from interrupt: channel A only, and the
        // same release RETI performs
        if ch == Ch::A && self.chan[0].pointer == 0 && (val >> 3) & 0x07 == 0b111 {
            self.reti();
        }
        self.chan[ch as usize].write_control(val, pacing_hz, fast_io)
    }

    /// RR0-RR2 through the channel's pointer, which resets on the read. RR2
    /// exists in channel B only and reads as zero on A.
    pub fn read_control(&mut self, ch: Ch) -> u8 {
        let pointer = self.chan[ch as usize].pointer;
        self.chan[ch as usize].pointer = 0;
        match (pointer, ch) {
            (0, Ch::A) => self.chan[0].rr0(self.pending().is_some()),
            (0, Ch::B) => self.chan[1].rr0(false),
            (1, _) => self.chan[ch as usize].rr1(),
            (2, Ch::B) => self.vector(),
            _ => 0,
        }
    }

    /// A source's place in the chip's fixed order: channel A's receive,
    /// then its transmit, then channel B's; lower is higher priority.
    fn rank(ch: Ch, source: Source) -> usize {
        2 * ch as usize + (source == Source::TxEmpty) as usize
    }

    /// The highest-priority request across the device that outranks every
    /// source under service: channel A's sources above channel B's,
    /// receive above transmit.
    fn pending(&self) -> Option<(Ch, Source)> {
        let request = if let Some(s) = self.chan[0].pending() {
            (Ch::A, s)
        } else {
            (Ch::B, self.chan[1].pending()?)
        };
        let serving = self.ius.iter().position(|&b| b).unwrap_or(self.ius.len());
        (Self::rank(request.0, request.1) < serving).then_some(request)
    }

    /// The INT line: something is requesting service.
    pub fn int_pending(&self) -> bool {
        self.pending().is_some()
    }

    /// The acknowledge cycle: the requesting source goes under service and
    /// the vector is the byte on the bus. With nothing requesting -- the
    /// core accepted an interrupt this chip was not raising -- the
    /// unmodified vector, and nothing changes.
    pub fn acknowledge(&mut self) -> u8 {
        let vector = self.vector();
        if let Some((ch, source)) = self.pending() {
            self.ius[Self::rank(ch, source)] = true;
        }
        vector
    }

    /// `RETI`: the highest source under service is released.
    pub fn reti(&mut self) {
        if let Some(slot) = self.ius.iter_mut().find(|b| **b) {
            *slot = false;
        }
    }

    /// The vector the device would put on the bus: channel B's WR2, with
    /// bits 3-1 replaced by the requesting source's code when channel B's
    /// WR1 says status affects vector. With nothing pending that is the
    /// unmodified vector.
    pub fn vector(&self) -> u8 {
        let base = self.chan[1].wr[2];
        if self.chan[1].wr[1] & WR1_STATUS_AFFECTS_VECTOR == 0 {
            return base;
        }
        match self.pending() {
            Some((ch, source)) => {
                let code = source as u8 + if ch == Ch::A { 4 } else { 0 };
                (base & !0x0e) | (code << 1)
            }
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An RC2014-shaped channel A: 7.3728 MHz in, WR4 x64 with one stop
    /// bit, eight data bits, no parity -- 115200 baud, 640 cycles a
    /// character at the machine's own rate. Receiver enabled.
    fn rc2014_a() -> Z80Sio {
        let mut sio = Z80Sio::new(7_372_800);
        sio.set_clock_hz(Ch::A, 7_372_800);
        program(&mut sio, Ch::A, &[(4, 0xc4), (3, 0xe1), (5, 0xea)]);
        sio
    }

    fn program(sio: &mut Z80Sio, ch: Ch, regs: &[(u8, u8)]) {
        for &(reg, val) in regs {
            sio.write_control(ch, reg);
            sio.write_control(ch, val);
        }
    }

    fn rx_available(sio: &mut Z80Sio, ch: Ch) -> bool {
        sio.read_control(ch) & RR0_RX_AVAILABLE != 0
    }

    #[test]
    fn idle_status_reports_tx_empty_and_no_rx() {
        let mut sio = rc2014_a();
        let rr0 = sio.read_control(Ch::A);
        assert_ne!(rr0 & RR0_TX_EMPTY, 0);
        assert_eq!(rr0 & RR0_RX_AVAILABLE, 0);
        assert_eq!(rr0 & RR0_INT_PENDING, 0);
    }

    /// A byte from the far end takes its frame time to arrive: ten bits
    /// at 115200 baud is 640 cycles of a 7.3728 MHz machine.
    #[test]
    fn a_character_is_clocked_in_after_its_frame_time() {
        let mut sio = rc2014_a();
        sio.receive(Ch::A, b'q');
        sio.tick(639);
        assert!(!rx_available(&mut sio, Ch::A), "still on the wire");
        sio.tick(1);
        assert!(rx_available(&mut sio, Ch::A));
        assert_eq!(sio.read_data(Ch::A), b'q');
        assert!(!rx_available(&mut sio, Ch::A));
    }

    /// A burst arrives one character per frame time, however many are
    /// queued, and comes out in order.
    #[test]
    fn a_burst_arrives_one_character_per_frame_time() {
        let mut sio = rc2014_a();
        for b in b"abc" {
            sio.receive(Ch::A, *b);
        }
        sio.tick(640);
        assert_eq!(sio.read_data(Ch::A), b'a');
        assert!(!rx_available(&mut sio, Ch::A));
        sio.tick(1280);
        assert_eq!(sio.read_data(Ch::A), b'b');
        assert_eq!(sio.read_data(Ch::A), b'c');
    }

    /// The frame follows the registers: a 300 baud keyboard on a 2.5 MHz
    /// Kaypro (a 4800 Hz clock through WR4's x16) is 83,333 cycles a
    /// character, and the pacing rate rescales it.
    #[test]
    fn the_character_time_follows_the_clock_the_registers_and_the_pacing() {
        let mut sio = Z80Sio::new(2_500_000);
        sio.set_clock_hz(Ch::B, 16 * 300);
        program(&mut sio, Ch::B, &[(4, 0x44), (3, 0xc1)]);
        assert_eq!(sio.chan[1].char_cycles, 83_333);
        // two stop bits and parity: twelve bits
        program(&mut sio, Ch::B, &[(4, 0x4d)]);
        assert_eq!(sio.chan[1].char_cycles, 100_000);
        // a machine paced at 25 kHz: the same tenth of a second of that
        sio.set_pacing_hz(25_000);
        assert_eq!(sio.chan[1].char_cycles, 1_000);
        // no clock at all is instant
        sio.set_clock_hz(Ch::B, 0);
        assert_eq!(sio.chan[1].char_cycles, 0);
    }

    #[test]
    fn fast_io_makes_both_directions_instant_and_stays() {
        let mut sio = rc2014_a();
        sio.set_fast_io();
        sio.set_pacing_hz(1_000_000);
        sio.receive(Ch::A, b'x');
        sio.tick(0);
        assert!(rx_available(&mut sio, Ch::A));
        sio.write_data(Ch::A, b'y');
        assert_ne!(sio.read_control(Ch::A) & RR0_TX_EMPTY, 0);
    }

    /// A far end that honours RTS sends nothing while it is off (WR5 bit
    /// 1), and resumes where it was; a keyboard has no RTS input.
    #[test]
    fn rts_holds_the_line_only_for_a_far_end_that_honours_it() {
        let mut sio = rc2014_a();
        sio.set_honours_rts(Ch::A, true);
        program(&mut sio, Ch::A, &[(5, 0xe8)]); // RTS off
        sio.receive(Ch::A, b'h');
        sio.tick(10_000);
        assert!(!rx_available(&mut sio, Ch::A), "held");
        program(&mut sio, Ch::A, &[(5, 0xea)]); // RTS on
        sio.tick(640);
        assert!(rx_available(&mut sio, Ch::A));

        let mut kbd = rc2014_a();
        program(&mut kbd, Ch::A, &[(5, 0xe8)]);
        kbd.receive(Ch::A, b'k');
        kbd.tick(640);
        assert!(rx_available(&mut kbd, Ch::A), "a keyboard does not care");
    }

    /// Softening, see the module doc: a byte typed while the receiver is
    /// disabled waits on the line rather than being lost.
    #[test]
    fn a_disabled_receiver_holds_the_line() {
        let mut sio = rc2014_a();
        program(&mut sio, Ch::A, &[(3, 0xe0)]);
        sio.receive(Ch::A, b'w');
        sio.tick(10_000);
        assert!(!rx_available(&mut sio, Ch::A));
        program(&mut sio, Ch::A, &[(3, 0xe1)]);
        sio.tick(640);
        assert_eq!(sio.read_data(Ch::A), b'w');
    }

    /// Three characters unread, and the fourth overwrites the last and sets
    /// RR1's overrun, which the error reset clears.
    #[test]
    fn the_fifo_is_three_deep_and_overruns_into_rr1() {
        let mut sio = rc2014_a();
        for b in b"1234" {
            sio.receive(Ch::A, *b);
        }
        sio.tick(4 * 640);
        sio.write_control(Ch::A, 1);
        assert_ne!(sio.read_control(Ch::A) & RR1_RX_OVERRUN, 0);
        assert_eq!(sio.read_data(Ch::A), b'1');
        assert_eq!(sio.read_data(Ch::A), b'2');
        assert_eq!(sio.read_data(Ch::A), b'4');
        assert_eq!(sio.read_data(Ch::A), 0, "empty fifo reads as zero");
        sio.write_control(Ch::A, 0b110 << 3);
        sio.write_control(Ch::A, 1);
        assert_eq!(sio.read_control(Ch::A) & RR1_RX_OVERRUN, 0);
    }

    /// WR1 bits 4-3: no receive interrupt, one for the first character
    /// only until re-armed, or one for every character.
    #[test]
    fn receive_interrupt_modes() {
        // mode 0: a character waits without asking
        let mut sio = rc2014_a();
        sio.receive(Ch::A, b'a');
        sio.tick(640);
        assert!(!sio.int_pending());

        // mode 2, the RC2014 rom's: every character
        program(&mut sio, Ch::A, &[(1, 0x18)]);
        assert!(sio.int_pending());
        assert_ne!(sio.read_control(Ch::A) & RR0_INT_PENDING, 0);
        sio.read_data(Ch::A);
        assert!(!sio.int_pending());
        sio.receive(Ch::A, b'b');
        sio.tick(640);
        assert!(sio.int_pending(), "and the next one");

        // mode 1: the first character, then nothing until re-armed
        let mut sio = rc2014_a();
        program(&mut sio, Ch::A, &[(1, 0x08)]);
        sio.receive(Ch::A, b'a');
        sio.tick(640);
        assert!(sio.int_pending());
        sio.read_data(Ch::A);
        sio.receive(Ch::A, b'b');
        sio.tick(640);
        assert!(!sio.int_pending(), "the second is for polling");
        sio.write_control(Ch::A, 0b100 << 3); // enable int on next rx char
        assert!(sio.int_pending(), "re-armed: the waiting character counts");
    }

    /// The buffer stays full for a frame after a write; when it empties
    /// with WR1's transmit interrupt enabled, that is a request, cleared by
    /// the reset command or by the next write.
    #[test]
    fn tx_empty_returns_after_a_frame_and_can_interrupt() {
        let mut sio = rc2014_a();
        sio.write_data(Ch::A, b'o');
        assert_eq!(sio.read_control(Ch::A) & RR0_TX_EMPTY, 0);
        sio.write_control(Ch::A, 1);
        assert_eq!(sio.read_control(Ch::A) & RR1_ALL_SENT, 0);
        sio.tick(640);
        assert_ne!(sio.read_control(Ch::A) & RR0_TX_EMPTY, 0);
        assert!(!sio.int_pending(), "not enabled");

        program(&mut sio, Ch::A, &[(1, 0x02)]);
        sio.write_data(Ch::A, b'p');
        sio.tick(640);
        assert!(sio.int_pending());
        sio.write_control(Ch::A, 0b101 << 3); // reset tx int pending
        assert!(!sio.int_pending());
        sio.write_data(Ch::A, b'q');
        sio.tick(640);
        assert!(sio.int_pending());
        sio.write_data(Ch::A, b'r');
        assert!(!sio.int_pending(), "the write clears it");
    }

    /// Channel B's WR2 is the vector; with channel B's "status affects
    /// vector" the requesting source writes bits 3-1, channel A's sources
    /// above channel B's and receive above transmit.
    #[test]
    fn the_vector_names_the_highest_priority_source() {
        let mut sio = Z80Sio::new(1_000_000);
        program(&mut sio, Ch::A, &[(3, 0xc1), (1, 0x18)]);
        program(&mut sio, Ch::B, &[(3, 0xc1), (1, 0x18), (2, 0x40)]);
        assert_eq!(sio.vector(), 0x40, "unmodified: nothing pending");
        sio.receive(Ch::B, 1);
        sio.tick(0);
        assert_eq!(sio.vector(), 0x40, "status does not affect it yet");
        program(&mut sio, Ch::B, &[(1, 0x1c)]);
        assert_eq!(sio.vector(), 0x44, "B rx char = 010");
        sio.receive(Ch::A, 2);
        sio.tick(0);
        assert_eq!(sio.vector(), 0x4c, "A rx char = 110 outranks it");
        sio.write_control(Ch::B, 2);
        assert_eq!(sio.read_control(Ch::B), 0x4c, "RR2 reads the same");
        sio.read_data(Ch::A);
        sio.read_data(Ch::B);
        program(&mut sio, Ch::A, &[(1, 0x02)]);
        sio.write_data(Ch::A, 0);
        assert_eq!(sio.vector(), 0x48, "A tx empty = 100");
        assert_eq!(sio.read_control(Ch::A) & RR0_INT_PENDING, RR0_INT_PENDING);
        sio.write_control(Ch::A, 2);
        assert_eq!(sio.read_control(Ch::A), 0, "RR2 is channel B's only");
    }

    /// An acknowledged source is under service: it and everything below
    /// it hold their requests until RETI, while a higher one may nest.
    /// Channel A's "return from interrupt" command releases the same way.
    #[test]
    fn an_acknowledged_source_holds_the_line_until_reti() {
        let mut sio = Z80Sio::new(1_000_000);
        program(&mut sio, Ch::A, &[(3, 0xc1), (1, 0x18)]);
        program(&mut sio, Ch::B, &[(3, 0xc1), (1, 0x1c), (2, 0x40)]);
        sio.receive(Ch::B, 1);
        sio.tick(0);
        assert_eq!(sio.acknowledge(), 0x44, "B rx acknowledged");
        assert!(!sio.int_pending(), "under service");
        sio.read_data(Ch::B);
        sio.receive(Ch::B, 2);
        sio.tick(0);
        assert!(!sio.int_pending(), "the next character waits for reti");
        sio.receive(Ch::A, 3);
        sio.tick(0);
        assert!(sio.int_pending(), "channel A outranks the service");
        assert_eq!(sio.acknowledge(), 0x4c);
        sio.read_data(Ch::A);
        assert!(!sio.int_pending());
        sio.reti();
        assert!(!sio.int_pending(), "A's service released, B's still holds");
        sio.reti();
        assert!(sio.int_pending(), "and now B's character");
        assert_eq!(sio.acknowledge(), 0x44);
        sio.write_control(Ch::A, 0b111 << 3); // return from interrupt
        sio.read_data(Ch::B);
        assert!(!sio.int_pending());
        assert_eq!(sio.ius, [false; 4]);

        // an acknowledge nothing here asked for changes nothing
        assert_eq!(sio.acknowledge(), 0x40);
        assert_eq!(sio.ius, [false; 4]);
    }

    #[test]
    fn channels_are_independent() {
        let mut sio = rc2014_a();
        sio.set_clock_hz(Ch::B, 7_372_800);
        program(&mut sio, Ch::B, &[(4, 0xc4), (3, 0xe1)]);
        sio.receive(Ch::A, 0x55);
        sio.tick(640);
        assert!(!rx_available(&mut sio, Ch::B));
        assert_eq!(sio.read_data(Ch::A), 0x55);
    }

    #[test]
    fn register_pointer_selects_then_resets() {
        let mut sio = rc2014_a();
        sio.write_control(Ch::A, 0x01);
        assert_eq!(sio.read_control(Ch::A), RR1_ALL_SENT);
        assert_ne!(sio.read_control(Ch::A) & RR0_TX_EMPTY, 0, "back at RR0");
        sio.write_control(Ch::A, 0x03);
        sio.write_control(Ch::A, 0xc1);
        assert_eq!(sio.chan[0].wr[3], 0xc1);
        assert_eq!(sio.chan[0].pointer, 0);
    }

    /// A channel reset drops what was received and the registers, but not
    /// what the far end has still to send, nor the wiring.
    #[test]
    fn channel_reset_drops_the_fifo_and_keeps_the_line() {
        let mut sio = rc2014_a();
        sio.set_honours_rts(Ch::A, true);
        sio.receive(Ch::A, 0x11);
        sio.receive(Ch::A, 0x22);
        sio.tick(640);
        sio.write_control(Ch::A, 0b011 << 3);
        assert!(!rx_available(&mut sio, Ch::A));
        assert_eq!(sio.chan[0].wr, [0; 8]);
        assert!(sio.chan[0].honours_rts);
        assert_eq!(sio.chan[0].clock_hz, 7_372_800);
        // the receiver is disabled now, so 0x22 waits for the WR3 and
        // then its frame
        program(&mut sio, Ch::A, &[(4, 0xc4), (3, 0xe1), (5, 0xea)]);
        sio.tick(640);
        assert_eq!(sio.read_data(Ch::A), 0x22);
    }
}
