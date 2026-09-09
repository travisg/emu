// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Zilog Z80 CTC, the four-channel counter/timer: the RC2014's CTC module
//! at ports `0x88`-`0x8b`.
//!
//! Written from the Z80 CTC technical manual. Each channel takes a control
//! word (bit 0 set; bits: 7 interrupt enable, 6 counter rather than timer
//! mode, 5 prescaler 256 rather than 16, 4 trigger edge, 3 timer waits for
//! a trigger rather than starting on its time constant, 2 a time constant
//! follows, 1 reset) and a time constant (0 is 256), and reads back its
//! down-counter. A write with bit 0 clear to channel 0 is the interrupt
//! vector, of which bits 7-3 are kept and bits 2-1 name the channel.
//!
//! What is live: timer mode, counting the system clock through the
//! prescaler and interrupting at each zero (the count reloads and goes on,
//! as the chip does), the per-channel interrupt-under-service state with
//! channel 0 the highest priority, and the vector. Counter mode counts
//! pulses on a channel's CLK/TRG input, and nothing here drives one -- the
//! module's jumpers can feed a clock in, but no guest of this tree asks --
//! so a counter-mode channel sits at its constant; likewise a timer told to
//! wait for a trigger never starts. ZC/TO outputs go nowhere.
//!
//! The chip counts the cpu's clock, so its cycles are the machine's own and
//! it needs no pacing rate: a timer set for a millisecond is a millisecond
//! of machine time at any `--throttle`, and `--fast-io` has no meaning for
//! it. The cycles arrive through `tick`, from the machine's interrupt poll.

const CONTROL_INT_ENABLE: u8 = 1 << 7;
const CONTROL_COUNTER_MODE: u8 = 1 << 6;
const CONTROL_PRESCALER_256: u8 = 1 << 5;
const CONTROL_WAIT_TRIGGER: u8 = 1 << 3;
const CONTROL_CONSTANT_FOLLOWS: u8 = 1 << 2;
const CONTROL_RESET: u8 = 1 << 1;
const CONTROL_WORD: u8 = 1 << 0;

pub const CHANNELS: usize = 4;

#[derive(Default)]
struct Channel {
    control: u8,
    /// The time constant as written; 0 counts as 256.
    constant: u8,
    /// The down-counter, 1..=256.
    count: u16,
    /// System clocks accumulated toward the next prescaler carry.
    prescale: u32,
    running: bool,
    /// The next write is the time constant.
    expect_constant: bool,
    int_pending: bool,
}

impl Channel {
    fn period(&self) -> u16 {
        if self.constant == 0 { 256 } else { self.constant as u16 }
    }

    fn prescaler(&self) -> u32 {
        if self.control & CONTROL_PRESCALER_256 != 0 { 256 } else { 16 }
    }

    fn write(&mut self, val: u8) {
        if self.expect_constant {
            self.expect_constant = false;
            self.constant = val;
            self.count = self.period();
            self.prescale = 0;
            // a timer starts on its constant unless told to wait for a
            // trigger; a counter waits for pulses that never come
            self.running = self.control & (CONTROL_COUNTER_MODE | CONTROL_WAIT_TRIGGER) == 0;
            return;
        }
        if val & CONTROL_WORD == 0 {
            return; // the vector, handled by the device
        }
        self.control = val;
        if val & CONTROL_RESET != 0 {
            self.running = false;
            self.int_pending = false;
        }
        if val & CONTROL_CONSTANT_FOLLOWS != 0 {
            self.expect_constant = true;
        }
    }

    fn tick(&mut self, elapsed: u32) {
        if !self.running {
            return;
        }
        self.prescale += elapsed;
        let prescaler = self.prescaler();
        while self.prescale >= prescaler {
            self.prescale -= prescaler;
            self.count -= 1;
            if self.count == 0 {
                self.count = self.period();
                if self.control & CONTROL_INT_ENABLE != 0 {
                    self.int_pending = true;
                }
            }
        }
    }

    fn read(&self) -> u8 {
        self.count as u8
    }
}

pub struct Z80Ctc {
    chan: [Channel; CHANNELS],
    vector: u8,
    /// Interrupt under service, by channel.
    ius: [bool; CHANNELS],
}

impl Default for Z80Ctc {
    fn default() -> Self {
        Self::new()
    }
}

impl Z80Ctc {
    pub fn new() -> Self {
        Z80Ctc { chan: Default::default(), vector: 0, ius: [false; CHANNELS] }
    }

    pub fn write(&mut self, ch: usize, val: u8) {
        let chan = &mut self.chan[ch & 3];
        if ch & 3 == 0 && !chan.expect_constant && val & CONTROL_WORD == 0 {
            self.vector = val & 0xf8;
            return;
        }
        chan.write(val);
    }

    pub fn read(&self, ch: usize) -> u8 {
        self.chan[ch & 3].read()
    }

    /// Advance every channel by `elapsed` system clocks.
    pub fn tick(&mut self, elapsed: u32) {
        for c in &mut self.chan {
            c.tick(elapsed);
        }
    }

    /// The highest-priority channel requesting, if it outranks every
    /// channel under service.
    fn pending(&self) -> Option<usize> {
        let serving = self.ius.iter().position(|&b| b).unwrap_or(CHANNELS);
        (0..serving).find(|&ch| self.chan[ch].int_pending)
    }

    /// The INT line.
    pub fn int_pending(&self) -> bool {
        self.pending().is_some()
    }

    pub fn under_service(&self) -> bool {
        self.ius.iter().any(|&b| b)
    }

    /// The acknowledge cycle: the requesting channel goes under service,
    /// its request is taken, and the vector names it. With nothing
    /// requesting, the bare vector.
    pub fn acknowledge(&mut self) -> u8 {
        match self.pending() {
            Some(ch) => {
                self.chan[ch].int_pending = false;
                self.ius[ch] = true;
                self.vector | (ch as u8) << 1
            }
            None => self.vector,
        }
    }

    /// `RETI`: the highest channel under service is released.
    pub fn reti(&mut self) {
        if let Some(slot) = self.ius.iter_mut().find(|b| **b) {
            *slot = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Channel `ch` as a free-running timer: prescaler 256, constant
    /// `constant`, interrupting.
    fn timer(ctc: &mut Z80Ctc, ch: usize, constant: u8) {
        ctc.write(ch, CONTROL_INT_ENABLE | CONTROL_PRESCALER_256 | CONTROL_CONSTANT_FOLLOWS | CONTROL_WORD);
        ctc.write(ch, constant);
    }

    /// A timer counts the system clock through its prescaler and
    /// interrupts at zero, reloading as it goes; the count reads back.
    #[test]
    fn a_timer_interrupts_every_constant_times_prescaler_clocks() {
        let mut ctc = Z80Ctc::new();
        timer(&mut ctc, 1, 10);
        assert_eq!(ctc.read(1), 10);
        ctc.tick(256 * 10 - 1);
        assert_eq!(ctc.read(1), 1);
        assert!(!ctc.int_pending());
        ctc.tick(1);
        assert!(ctc.int_pending());
        assert_eq!(ctc.read(1), 10, "reloaded");
        ctc.acknowledge();
        ctc.reti();
        ctc.tick(256 * 10);
        assert!(ctc.int_pending(), "and again");

        // prescaler 16, constant 0 = 256
        let mut ctc = Z80Ctc::new();
        ctc.write(2, CONTROL_INT_ENABLE | CONTROL_CONSTANT_FOLLOWS | CONTROL_WORD);
        ctc.write(2, 0);
        ctc.tick(16 * 256 - 1);
        assert!(!ctc.int_pending());
        ctc.tick(1);
        assert!(ctc.int_pending());
    }

    /// The vector is channel 0's bit-0-clear write, and the acknowledge
    /// puts the channel number in bits 2-1. Channel 0 outranks the rest,
    /// and a channel under service holds the lower ones until RETI.
    #[test]
    fn the_vector_names_the_channel_and_service_holds_the_lower_ones() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, 0x40);
        timer(&mut ctc, 3, 1);
        timer(&mut ctc, 1, 1);
        ctc.tick(256);
        assert_eq!(ctc.acknowledge(), 0x42, "channel 1 first");
        assert!(!ctc.int_pending(), "channel 3 waits under channel 1's service");
        assert!(ctc.under_service());
        ctc.write(1, CONTROL_RESET | CONTROL_WORD); // no more from channel 1
        timer(&mut ctc, 0, 1);
        ctc.tick(256);
        assert!(ctc.int_pending(), "channel 0 outranks the service");
        assert_eq!(ctc.acknowledge(), 0x40);
        ctc.reti();
        ctc.reti();
        assert!(!ctc.under_service());
        assert_eq!(ctc.acknowledge(), 0x46, "channel 3 at last");
        ctc.reti();
        assert_eq!(ctc.acknowledge(), 0x40, "nothing requesting: the bare vector");
    }

    /// Interrupts off is a timer that counts and asks for nothing; reset
    /// stops it; a counter-mode channel and a timer waiting for a trigger
    /// never start, there being no CLK/TRG here.
    #[test]
    fn quiet_channels() {
        let mut ctc = Z80Ctc::new();
        ctc.write(0, CONTROL_PRESCALER_256 | CONTROL_CONSTANT_FOLLOWS | CONTROL_WORD);
        ctc.write(0, 2);
        ctc.tick(512);
        assert!(!ctc.int_pending());
        assert_eq!(ctc.read(0), 2, "reloaded all the same");
        ctc.write(0, CONTROL_RESET | CONTROL_WORD);
        ctc.tick(512);
        assert_eq!(ctc.read(0), 2, "stopped");

        ctc.write(1, CONTROL_INT_ENABLE | CONTROL_COUNTER_MODE | CONTROL_CONSTANT_FOLLOWS | CONTROL_WORD);
        ctc.write(1, 1);
        ctc.write(2, CONTROL_INT_ENABLE | CONTROL_WAIT_TRIGGER | CONTROL_CONSTANT_FOLLOWS | CONTROL_WORD);
        ctc.write(2, 1);
        ctc.tick(100_000);
        assert!(!ctc.int_pending());
        assert_eq!((ctc.read(1), ctc.read(2)), (1, 1));
    }
}
