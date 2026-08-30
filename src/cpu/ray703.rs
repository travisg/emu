// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Raytheon 703 interpreter core.
//!
//! Unlike the other cores in this tree, this one is not a port of anything:
//! the C++ emulator this tree grew out of never had a 703. It is written from
//! the *Raytheon 703 Computer Reference and Interface Manual*, whose relevant
//! sections are transcribed alongside the scans as `Raytheon703refMan_isa.txt`.
//! Section references in the comments below ("2-7.6", "1-3.3.2") are to that
//! manual.
//!
//! Cross-checked against two emulators written by Darwin Geiselbrecht, who
//! programmed these machines: `rustheon` (Rust) and `Raytheon` (Python), both
//! MIT, at github.com/IslandSparky. Where the three disagree the manual wins --
//! rustheon decodes the register generics one slot high (CLR at 0x011 rather
//! than 0x010), and the PTB paper-tape bootstrap listing, which encodes eight
//! different instructions in eleven words, agrees with the manual.
//!
//! Two conventions make this machine confusing to read if you don't hold them
//! in your head:
//!
//! * **Bit 0 is the most significant bit.** Every field description in the
//!   manual counts from the left. Comments here quote the manual's numbering;
//!   the code uses ordinary Rust shifts, so `(EXR)0-4` is `exr >> ...` of a
//!   value whose *own* bit 0 is its LSB. The two never appear in the same
//!   expression without a comment.
//! * **Memory is words, but bytes are addressable.** A word address indexes
//!   16-bit words; a byte address is `word << 1`, with byte 0 (the manual's
//!   "left" byte, ACR bits 0-7) at the even address. The bus underneath is a
//!   flat 64K byte space, so word access is `read16(waddr << 1, Endian::Big)`
//!   and the even-byte-is-high-half rule falls out of the composed accessor.

use super::{Cpu, StepResult};
use crate::bus::{Bus, Endian};
// The one cpu -> console dependency in the tree, the same direction the
// kaypro bus takes for `VideoBuffer`: the panel handle is defined next to
// the frontend machinery that renders it, and the core just stores into it.
use crate::console::{PanelCommand, PanelState, Selector};
use std::io::Write;

/// Words 0..64 are the sixteen four-word interrupt blocks (3-1), so this is
/// also the lowest word a program can safely occupy if it uses level 15.
const INT_BLOCK_WORDS: u16 = 4;

/// "1.75 usec cycle time" (1-1), i.e. 4/7 MHz. A u64 in Hz is 0.75 ppm off
/// the exact fraction, far inside the tolerance of any 1967 crystal.
pub const CLOCK_HZ: u64 = 571_429;

/// Bits of the machine status word saved and restored by the interrupt
/// sequence. The manual (3-3) says the status is "the contents of the
/// extension register, the comparison indicators, and the memory addressing
/// mode (local/global) at the time of interrupt" but never diagrams the word.
/// This layout is rustheon's, and it is the only thing in this file not
/// derived from the manual: EXR in the manual's bits 0-4, then the flip flops.
const ST_EXR_SHIFT: u32 = 11;
const ST_EXR_MASK: u16 = 0xf800;
const ST_NEG: u16 = 0x0400;
const ST_EQL: u16 = 0x0200;
const ST_OVF: u16 = 0x0100;
const ST_GLB: u16 = 0x0080;

/// Per-level interrupt state (3-1). The manual's four states are Disabled,
/// Idle, Wait and Active; they factor into two independent bits plus the
/// enable, which is how the hardware flow chart (figure 3-1) actually reads:
/// Disabled is `!enabled`, Wait is `pending`, Active is `active`, and Idle is
/// enabled with neither set.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
struct Level {
    enabled: bool,
    pending: bool,
    active: bool,
}

pub struct Cpu703 {
    /// accumulator
    acr: u16,
    /// index register, also the right extension of ACR for double shifts
    ixr: u16,
    /// program counter, 15 bits -- always masked, never allowed to reach 0x8000
    pcr: u16,
    /// memory address extension, 5 bits, a *byte* page number
    exr: u8,

    /// ADFNEG: the last compare found ACR less than its operand
    neg: bool,
    /// ADFEQL: the last compare found them equal
    eql: bool,
    /// ADFOVF. Sticky: set by ADD/SUB/CMP/SLA/SLAD and cleared only by SNO
    /// (2-13) or by an interrupt status restore.
    ovf: bool,
    /// CCFGLB. Global mode zeroes EXR when forming an *indexed* base address
    /// (1-3.3.3); direct addressing always uses EXR.
    global: bool,

    levels: [Level; 16],
    /// MSK/UNM. Inhibits entry to any level; conditions stay pending (2-7).
    inhibit: bool,

    /// Clock cycles the last `step()` consumed, for `last_step_cycles`.
    cycles: u32,

    /// The front panel, when this machine has one. Registers are published
    /// into it after every step, and the sense skips read its SENSE toggles.
    /// Held by the core rather than reached through the `Bus` because that
    /// is what it was physically: panel switches and the J2 sense line wire
    /// straight into the processor, not onto the DIO channel -- the same
    /// reasoning that put `set_index` here.
    panel: Option<PanelState>,
}

impl Default for Cpu703 {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu703 {
    pub fn new() -> Self {
        Cpu703 {
            acr: 0,
            ixr: 0,
            pcr: 0,
            exr: 0,
            neg: false,
            eql: false,
            ovf: false,
            global: false,
            levels: [Level::default(); 16],
            inhibit: false,
            cycles: 0,
            panel: None,
        }
    }

    /// Attach the front panel. Like `set_index`, this is the factory
    /// standing in for the machine room: the panel was not optional on a
    /// real 703, but a headless emulator run has no use for the stores.
    pub fn attach_panel(&mut self, panel: PanelState) {
        self.panel = Some(panel);
    }

    /// Preset the index register, which on real hardware was done from the
    /// front panel before pressing RUN. PTB requires it -- the operator keys
    /// in the load origin minus twelve bytes -- and there is no front panel
    /// here, so the machine's factory does it instead.
    pub fn set_index(&mut self, ixr: u16) {
        self.ixr = ixr;
    }

    // -- memory ------------------------------------------------------------

    fn read_word(&self, bus: &mut dyn Bus, waddr: u16) -> u16 {
        let val = bus.read16(((waddr & 0x7fff) as u32) << 1, Endian::Big);
        if let Some(p) = &self.panel {
            // Every word through here transits the real machine's MA and MB
            // registers, instruction fetches included, and the panel's MA/MB
            // positions show whatever went through last (5-2). MA is a byte
            // address: memory is 32K words, so 16 bits span it (1-3.3.2).
            p.set_memory((waddr & 0x7fff) << 1, val);
        }
        val
    }

    fn write_word(&self, bus: &mut dyn Bus, waddr: u16, val: u16) {
        bus.write16(((waddr & 0x7fff) as u32) << 1, val, Endian::Big);
        if let Some(p) = &self.panel {
            p.set_memory((waddr & 0x7fff) << 1, val);
        }
    }

    /// Panel MA/MB for the byte instructions, which go to the bus as bytes.
    /// Real hardware pulled the whole word through MB; reading the other
    /// half here just for the lamps would add a bus access that `TestBus`'s
    /// re-read watch (and any device with read side effects) would see, so
    /// MB shows the byte in its low half instead. Documented approximation.
    fn panel_byte_access(&self, ea: u16, byte: u8) {
        if let Some(p) = &self.panel {
            p.set_memory(ea, byte as u16);
        }
    }

    // -- status word -------------------------------------------------------

    fn status(&self) -> u16 {
        let mut s = (self.exr as u16) << ST_EXR_SHIFT;
        if self.neg {
            s |= ST_NEG;
        }
        if self.eql {
            s |= ST_EQL;
        }
        if self.ovf {
            s |= ST_OVF;
        }
        if self.global {
            s |= ST_GLB;
        }
        s
    }

    /// Publish the lamp registers after a step. The MS position's word is
    /// the saved machine status word with the sequence register in the low
    /// nibble -- which is figure 5-1's MS layout exactly: EXR in indicators
    /// 0-4 is `ST_EXR_SHIFT`, ADFNEG at 5 is `ST_NEG`, and so on. The status
    /// layout this file took from rustheon turns out to be the panel's own.
    ///
    /// The sequence register is the active level's number; the manual never
    /// says what it shows when no level is active, so base level shows 0.
    fn publish_panel(&self) {
        if let Some(p) = &self.panel {
            let seq = (0..16).rev().find(|&l| self.levels[l].active).unwrap_or(0);
            p.set_registers(self.pcr, self.acr, self.ixr, self.status() | seq as u16);
        }
    }

    /// The position of SENSE toggle `n`, false when there is no panel --
    /// which preserves the pre-panel behaviour of every sense skip skipping.
    fn sense_switch(&self, n: u8) -> bool {
        self.panel.as_ref().is_some_and(|p| p.sense(n))
    }

    /// End-of-step panel bookkeeping: publish the point samples, then charge
    /// this step's cycles to the lamp on-time accumulators. Both of step()'s
    /// return paths come through here -- the interrupt entry included, or
    /// interrupt-heavy programs would undercount their lamps.
    fn publish_step(&self) {
        self.publish_panel();
        if let Some(p) = &self.panel {
            p.accumulate(self.cycles);
        }
    }

    fn set_status(&mut self, s: u16) {
        self.exr = ((s & ST_EXR_MASK) >> ST_EXR_SHIFT) as u8;
        self.neg = s & ST_NEG != 0;
        self.eql = s & ST_EQL != 0;
        self.ovf = s & ST_OVF != 0;
        self.global = s & ST_GLB != 0;
    }

    // -- addressing (1-3) --------------------------------------------------

    /// Effective *word* address for a word instruction.
    ///
    /// Direct: the base is `(EXR)0-3 : M`, i.e. the top four bits of the
    /// five-bit byte page, which is the word page. Indexed: IXR is added as a
    /// word offset, over a base that is zero-extended instead of EXR-extended
    /// in global mode.
    fn word_ea(&self, insn: u16) -> u16 {
        let m = insn & 0x07ff;
        if insn & 0x0800 == 0 {
            // direct -- unaffected by local/global, which applies to indexing
            (((self.exr >> 1) as u16) << 11) | m
        } else {
            let base = if self.global { m } else { (((self.exr >> 1) as u16) << 11) | m };
            base.wrapping_add(self.ixr) & 0x7fff
        }
    }

    /// Effective *byte* address for a byte instruction.
    ///
    /// The whole five-bit EXR is the byte page here, so the base is a 16-bit
    /// byte address and IXR is added as a byte offset (1-3.3.2). M's low bit
    /// ends up selecting the half of the word, which is exactly what the flat
    /// byte bus does for free.
    fn byte_ea(&self, insn: u16) -> u16 {
        let m = insn & 0x07ff;
        let base = if insn & 0x0800 != 0 && self.global {
            m
        } else {
            ((self.exr as u16) << 11) | m
        };
        if insn & 0x0800 != 0 {
            base.wrapping_add(self.ixr)
        } else {
            base
        }
    }

    /// "After executing the memory reference instruction, the contents of EXR
    /// are replaced by bits 1 thru 5 of the program counter" (1-3), i.e. the
    /// byte page of the instruction about to be executed. This runs *after*
    /// the instruction, so a preceding SML/SMU governs exactly one memory
    /// reference and then evaporates.
    fn reload_exr(&mut self) {
        self.exr = ((self.pcr >> 10) & 0x1f) as u8;
    }

    // -- helpers -----------------------------------------------------------

    fn skip_if(&mut self, cond: bool) {
        if cond {
            self.pcr = self.pcr.wrapping_add(1) & 0x7fff;
        }
    }

    /// The compare flip flops, set by CMW/CMB/CLB (2-5, 2-11). They are the
    /// only record of a comparison: there is no carry, and the skips read
    /// these rather than recomputing anything.
    fn set_compare(&mut self, less: bool, equal: bool) {
        self.neg = less;
        self.eql = equal;
    }

    /// ACR:IXR as the 32-bit double-length accumulator used by the double
    /// shifts, "where the index register is treated as the right extension of
    /// the accumulator" (2-7.6).
    fn double(&self) -> u32 {
        ((self.acr as u32) << 16) | self.ixr as u32
    }

    fn set_double(&mut self, v: u32) {
        self.acr = (v >> 16) as u16;
        self.ixr = v as u16;
    }

    // -- interrupts (section 3) --------------------------------------------

    /// The level the hardware would enter right now, if any.
    ///
    /// A level advances from Wait to Active "when there is no higher priority
    /// interrupt level in the Active or the Wait state, the inhibit interrupt
    /// mask is off, and the execution of the current instruction is completed"
    /// (3-3). Scanning from 15 down and taking the first level that is pending
    /// or active gives exactly that: if the winner is already active we are
    /// inside its subroutine and nothing happens.
    fn ready_level(&self) -> Option<usize> {
        if self.inhibit {
            return None;
        }
        for level in (0..16).rev() {
            let l = &self.levels[level];
            if l.active {
                // a higher-priority subroutine is in progress; everything at
                // or below this level waits for its INR
                return None;
            }
            if l.pending {
                return Some(level);
            }
        }
        None
    }

    /// The fixed hardware sequence: save PC and status, force global mode,
    /// transfer to the linkage address (3-3).
    fn enter_interrupt(&mut self, bus: &mut dyn Bus, level: usize) {
        let base = (level as u16) * INT_BLOCK_WORDS;
        self.write_word(bus, base, self.pcr);
        self.write_word(bus, base + 2, self.status());
        self.levels[level].pending = false;
        self.levels[level].active = true;
        self.global = true;
        self.pcr = self.read_word(bus, base + 1) & 0x7fff;
    }

    /// INR: restore and return the level to Idle (2-6). A signal that arrived
    /// while the subroutine was running is still pending and fires again.
    fn interrupt_return(&mut self, bus: &mut dyn Bus, level: usize) {
        let base = (level as u16) * INT_BLOCK_WORDS;
        self.pcr = self.read_word(bus, base) & 0x7fff;
        let status = self.read_word(bus, base + 2);
        self.set_status(status);
        self.levels[level].active = false;
    }

    /// True when no interrupt can ever arrive, so a branch to self really is
    /// the end of the program rather than the idle loop every driver in this
    /// machine's software uses. PTB spends most of its life in `JMP $`.
    fn interrupts_are_dead(&self) -> bool {
        self.inhibit || !self.levels.iter().any(|l| l.enabled)
    }

    // -- execution ---------------------------------------------------------

    fn exec_memory(&mut self, bus: &mut dyn Bus, insn: u16) -> StepResult {
        let opcode = insn >> 12;
        let mut result = StepResult::Ok;

        match opcode {
            0x1 => {
                // JMP
                let target = self.word_ea(insn);
                // A jump to self with no way for an interrupt to change
                // anything is the C++ cores' "infinite loop" stop. Here it is
                // also the normal way to wait for I/O, so the enable state has
                // to be part of the test.
                if target == self.pcr.wrapping_sub(1) & 0x7fff && self.interrupts_are_dead() {
                    result = StepResult::InfiniteLoop;
                }
                self.pcr = target;
            }
            0x2 => {
                // JSX: "forces the computer into global addressing mode prior
                // to the transfer" (2-6), so the EA is computed first.
                let target = self.word_ea(insn);
                self.ixr = self.pcr;
                self.global = true;
                self.pcr = target;
            }
            0x3 => {
                let ea = self.byte_ea(insn);
                bus.write8(ea as u32, self.acr as u8);
                self.panel_byte_access(ea, self.acr as u8);
            }
            0x4 => {
                // CMB: bytes are signed 8-bit (2-5)
                let ea = self.byte_ea(insn);
                let a = self.acr as u8 as i8 as i32;
                let b = bus.read8(ea as u32);
                self.panel_byte_access(ea, b);
                let b = b as i8 as i32;
                self.set_compare(a - b < 0, a == b);
            }
            0x5 => {
                // LDB replaces only bits 8-15; the manual's 2-1 rule is that
                // anything not named is untouched, so no extension happens.
                let ea = self.byte_ea(insn);
                let b = bus.read8(ea as u32);
                self.panel_byte_access(ea, b);
                self.acr = (self.acr & 0xff00) | b as u16;
            }
            0x6 => {
                let ea = self.word_ea(insn);
                self.write_word(bus, ea, self.ixr);
            }
            0x7 => {
                let ea = self.word_ea(insn);
                self.write_word(bus, ea, self.acr);
            }
            0x8 => {
                let ea = self.word_ea(insn);
                self.acr = self.read_word(bus, ea);
            }
            0x9 => {
                let ea = self.word_ea(insn);
                self.ixr = self.read_word(bus, ea);
            }
            0xa => {
                let ea = self.word_ea(insn);
                let operand = self.read_word(bus, ea);
                let (sum, over) = (self.acr as i16).overflowing_add(operand as i16);
                self.acr = sum as u16;
                self.ovf |= over;
            }
            0xb => {
                let ea = self.word_ea(insn);
                let operand = self.read_word(bus, ea);
                let (diff, over) = (self.acr as i16).overflowing_sub(operand as i16);
                self.acr = diff as u16;
                self.ovf |= over;
            }
            0xc => {
                let ea = self.word_ea(insn);
                self.acr |= self.read_word(bus, ea);
            }
            0xd => {
                let ea = self.word_ea(insn);
                self.acr ^= self.read_word(bus, ea);
            }
            0xe => {
                let ea = self.word_ea(insn);
                self.acr &= self.read_word(bus, ea);
            }
            0xf => {
                let ea = self.word_ea(insn);
                let operand = self.read_word(bus, ea) as i16 as i32;
                let a = self.acr as i16 as i32;
                self.set_compare(a - operand < 0, a == operand);
            }
            _ => unreachable!("opcode 0 is not a memory reference"),
        }

        self.reload_exr();
        result
    }

    /// Opcode 0: "generics are non-memory reference instructions which share a
    /// common instruction code" (2-7). Bits 4-7 (FN) pick the class, bits 8-11
    /// (F1) a subclass, bits 12-15 (F2) a level, literal or shift count.
    fn exec_generic(&mut self, bus: &mut dyn Bus, insn: u16) -> StepResult {
        let fn_field = (insn >> 8) & 0x0f;
        let f1 = (insn >> 4) & 0x0f;
        let f2 = (insn & 0x0f) as usize;
        let literal = (insn & 0xff) as u8;

        match fn_field {
            0x0 => return self.exec_control(bus, f1, f2),
            0x1 => match f1 {
                0x0 => self.acr = 0,                     // CLR
                0x1 => {
                    // CMP: two's complement negate, overflow only on -2^15
                    let (neg, over) = (self.acr as i16).overflowing_neg();
                    self.acr = neg as u16;
                    self.ovf |= over;
                }
                0x2 => self.acr = !self.acr,             // INV
                0x3 => self.ixr = self.acr,              // CAX
                0x4 => self.acr = self.ixr,              // CXA
                _ => return StepResult::BadOpcode,
            },
            // DIN/DOT move a whole word over the DIO channel. The low byte of
            // the instruction is the DIO address: device in F1, function in
            // F2 (4-2.1).
            0x2 => self.acr = bus.io_read16(literal),
            0x3 => bus.io_write16(literal, self.acr),
            0x4 => {
                // IXS: unsigned literal, skip on the *new* index >= 0 (2-10)
                self.ixr = self.ixr.wrapping_add(literal as u16);
                self.skip_if(self.ixr as i16 >= 0);
            }
            0x5 => {
                self.ixr = self.ixr.wrapping_sub(literal as u16);
                self.skip_if((self.ixr as i16) < 0);
            }
            0x6 => self.acr = (self.acr & 0xff00) | literal as u16, // LLB
            0x7 => {
                // CLB: both sides are signed 8-bit (2-11)
                let a = self.acr as u8 as i8 as i32;
                let b = literal as i8 as i32;
                self.set_compare(a - b < 0, a == b);
            }
            0x8 => return self.exec_skip(f1),
            0x9 | 0xa => return self.shift(fn_field, f1, f2 as u32),
            // The optional multiply/divide hardware (section 6). Appendix B
            // never lists it -- the option was a plug-in card, and the
            // appendix indexes the base machine's 74 instructions -- but the
            // section 6 format diagrams give the encodings outright: first
            // word 0B0F (MPY) or 0C0F (DIV), second word a 15-bit address.
            // This machine has the option installed, like the full 32KW of
            // core. Every other encoding in classes B-F stays unassigned.
            0xb if literal == 0x0f => return self.mpy(bus),
            0xc if literal == 0x0f => return self.div(bus),
            _ => return StepResult::BadOpcode,
        }
        StepResult::Ok
    }

    fn exec_control(&mut self, bus: &mut dyn Bus, f1: u16, f2: usize) -> StepResult {
        match f1 {
            0x0 => return StepResult::Halted, // HLT
            0x1 => self.interrupt_return(bus, f2),
            // "Each interrupt level may advance from the Disabled state to the
            // Idle state by execution of an Enable Interrupt instruction"
            // (3-2) -- so ENB only lifts the disable, it does not cancel a
            // pending signal or an in-progress subroutine.
            0x2 => self.levels[f2].enabled = true,
            // DSB drops the level from any state, pending signal included.
            0x3 => self.levels[f2] = Level::default(),
            0x4 => self.global = false, // SLM
            0x5 => self.global = true,  // SGM
            // CEX/CXE move EXR in and out of the *top five bits* of IXR (2-8).
            0x6 => self.ixr = (self.ixr & 0x07ff) | ((self.exr as u16) << 11),
            0x7 => self.exr = (self.ixr >> 11) as u8,
            // SML/SMU set the byte page: F2 into the manual's EXR bits 1-4,
            // with bit 0 (the high bit of the five) clear or set. This lasts
            // exactly until the next memory reference instruction reloads EXR.
            0x8 => self.exr = f2 as u8,
            0x9 => self.exr = 0x10 | f2 as u8,
            0xa => self.inhibit = true,  // MSK
            0xb => self.inhibit = false, // UNM
            _ => return StepResult::BadOpcode,
        }
        StepResult::Ok
    }

    /// The skip generics (2-7.5). Every one is `PCR <- PCR + 1 + [condition]`;
    /// there are no conditional branches on this machine.
    fn exec_skip(&mut self, f1: u16) -> StepResult {
        let cond = match f1 {
            0x0 => self.acr == 0,                    // SAZ
            0x1 => self.acr as i16 >= 0,             // SAP
            0x2 => (self.acr as i16) < 0,            // SAM
            0x3 => self.acr & 1 != 0,                // SAO
            0x4 => self.neg,                         // SLS
            0x5 => self.ixr & 1 == 0,                // SXE
            0x6 => self.eql,                         // SEQ
            0x7 => !self.eql,                        // SNE
            0x8 => !self.eql && !self.neg,           // SGR
            0x9 => self.neg || self.eql,             // SLE
            0xa => {
                // SNO is the only thing that clears the overflow flip flop.
                let no_overflow = !self.ovf;
                self.ovf = false;
                no_overflow
            }
            // SSE reads the EXSENS line on connector J2. Nothing drives it
            // here, so it reads false -- and the skip takes the *false*
            // position, so SSE always skips.
            0xb => true,
            // SS0-SS3 read the front panel's SENSE toggles, "operator
            // setable and program testable", skipping on the down = false
            // position (5-4). With no panel attached they read false and
            // always skip, exactly as before the panel existed.
            0xc..=0xf => !self.sense_switch((f1 - 0xc) as u8),
            _ => return StepResult::BadOpcode,
        };
        self.skip_if(cond);
        StepResult::Ok
    }

    /// Arithmetic (FN=9) and logical (FN=A) shifts, 2-7.6.
    ///
    /// The count is F2, which is four bits, so `n` is always 0..=15 -- never
    /// wider than ACR, which is why the 16- and 32-bit forms below shift
    /// directly. The byte forms go through a `u16` because a count of 8 or
    /// more is a legal way to empty a byte, and shifting a `u8` that far is a
    /// panic in Rust rather than the zero the hardware produces.
    fn shift(&mut self, class: u16, f1: u16, n: u32) -> StepResult {
        debug_assert!(n < 16);
        if class == 0x9 {
            match f1 {
                0x0 => self.acr = ((self.acr as i16) >> n) as u16, // SRA
                0x1 => {
                    // SLA: "if the sign bit of the accumulator is changed by
                    // this operation, the overflow storage flip flop is set"
                    let before = self.acr & 0x8000;
                    self.acr <<= n;
                    if self.acr & 0x8000 != before {
                        self.ovf = true;
                    }
                }
                0x2 => {
                    // SRAD, the only shift that is arithmetic across all 32
                    // bits of the ACR:IXR pair
                    let v = ((self.double() as i32) >> n) as u32;
                    self.set_double(v);
                }
                0x3 => {
                    let before = self.acr & 0x8000;
                    let v = self.double() << n;
                    self.set_double(v);
                    if self.acr & 0x8000 != before {
                        self.ovf = true;
                    }
                }
                // Appendix B assigns 090 through 093 and stops. Everything
                // else in the arithmetic class is as unassigned as the
                // multiply and divide the optional hardware would have used.
                _ => return StepResult::BadOpcode,
            }
            return StepResult::Ok;
        }

        // Logical shifts. The manual omits the equations for these ("to keep
        // the explanation as simple as practicable") and describes each in
        // prose; the byte forms operate strictly within one half of ACR and
        // leave the other half alone.
        match f1 {
            0x0 => self.acr >>= n,                                 // SRL
            0x1 => self.acr <<= n,                                 // SLL
            0x2 => self.set_double(self.double() >> n),            // SRLD
            0x3 => self.set_double(self.double() << n),            // SLLD
            0x4 => self.acr = self.acr.rotate_right(n),            // SRC
            0x5 => self.acr = self.acr.rotate_left(n),             // SLC
            0x6 => self.set_double(self.double().rotate_right(n)), // SRCD
            0x7 => self.set_double(self.double().rotate_left(n)),  // SLCD
            0x8 => self.byte_shift(true, |b| (b as u16 >> n) as u8), // SRLL
            0x9 => self.byte_shift(true, |b| ((b as u16) << n) as u8), // SLLL
            0xa => self.byte_shift(false, |b| (b as u16 >> n) as u8), // SRLR
            0xb => self.byte_shift(false, |b| ((b as u16) << n) as u8), // SLLR
            0xc => self.byte_shift(true, |b| b.rotate_right(n % 8)), // SRCL
            0xd => self.byte_shift(true, |b| b.rotate_left(n % 8)), // SLCL
            0xe => self.byte_shift(false, |b| b.rotate_right(n % 8)), // SRCR
            0xf => self.byte_shift(false, |b| b.rotate_left(n % 8)), // SLCR
            // f1 is a nibble and all sixteen are assigned in this class
            _ => unreachable!("f1 is four bits"),
        }
        StepResult::Ok
    }

    // -- optional multiply/divide (section 6) ------------------------------

    /// Fetch the second word of a two-word MPY/DIV and resolve the operand.
    ///
    /// "The second location defines a 15-bit address which contains the
    /// multiplicand for a multiply or the divisor for a divide. The 15-bit
    /// address allows either instruction to reference any location in memory
    /// without using the index register." (6-1) -- so no EXR page base and no
    /// indexing, unlike every other memory operand on this machine. Bit 0 of
    /// the word is hatched out in both format diagrams and carries nothing.
    ///
    /// Section 6 says nothing about EXR, but these are memory reference
    /// instructions in the 1-3 sense, so both exec paths end with the same
    /// EXR reload as every other memory reference -- leaving it stale would
    /// make MPY/DIV the only way to carry a page selection *past* an
    /// instruction, which nothing else on the machine can do.
    fn mpy_div_operand(&mut self, bus: &mut dyn Bus) -> u16 {
        let addr = self.read_word(bus, self.pcr) & 0x7fff;
        self.pcr = self.pcr.wrapping_add(1) & 0x7fff;
        self.read_word(bus, addr)
    }

    /// MPY, first word 0B0F (6-2): ACR (multiplier) times the addressed word,
    /// both 16-bit two's complement.
    ///
    /// "Execution of the multiply instruction produces a 31-bit algebraic
    /// product in the index register and the accumulator. The most
    /// significant 16 bits of the product reside in the index register. The
    /// least significant 15 bits of the product reside in bits 1-15 of the
    /// accumulator. The most significant bit of the accumulator is set to the
    /// most significant bit of the index register." (6-2)
    ///
    /// "No overflow can result from the multiply instruction" (6-2), X'8000'
    /// squared included -- there the +2^30 product aliases to a negative in
    /// the 31-bit format and the flag still stays off. The 704 and 706
    /// versions of the option set OVF for exactly that case (704 technical
    /// manual 4-291, 706 users manual 2-1.10), and the 1968 *software*
    /// multiply (BLG, DN 390663) documents the flag too, but 6-2's sentence
    /// is unambiguous and this is a 703.
    fn mpy(&mut self, bus: &mut dyn Bus) -> StepResult {
        // "an execution time of 12.25 to 17.5 microseconds" (6-2) is 7 to 10
        // cycles; the manual never says what picks the point in the range.
        // The 706's per-ones formula -- 6 1/5 + N(1-15)/5 + 2 N(0)/5 cycles
        // rounded up, N counting the one bits of the multiplier (706 users
        // manual 2-1.10) -- spans exactly 7 to 10, so use it.
        let n = (self.acr & 0x7fff).count_ones() + 2 * (self.acr >> 15) as u32;
        self.cycles = (35 + n) / 5;

        let m = self.mpy_div_operand(bus);
        let product = (self.acr as i16 as i32) * (m as i16 as i32);
        self.ixr = (product >> 15) as u16;
        self.acr = (product as u16 & 0x7fff) | (self.ixr & 0x8000);
        self.reload_exr();
        StepResult::Ok
    }

    /// DIV, first word 0C0F (6-3): the 31-bit dividend in IXR (high 16) and
    /// ACR bits 1-15 -- "the most significant bit of the accumulator has no
    /// effect on the divide instruction" -- over the addressed 16-bit
    /// divisor. Quotient to ACR, remainder to IXR, and "the sign of the
    /// remainder is the same as that of the dividend" (6-3), which is also
    /// what Rust's truncating `/` and `%` produce.
    ///
    /// "An overflow condition exists when the quotient is greater than 16
    /// bits. The overflow indicator is turned ON and the dividend contained
    /// in the index register and the accumulator remains unchanged, except
    /// the most significant bit of the accumulator is set to the most
    /// significant bit of the index register." (6-3) The manual never gives
    /// the pretest a divider would actually run; the 706 does -- overflow
    /// when |divisor| <= |IXR| (706 users manual 2-1.10) -- and that is
    /// implemented here as a magnitude test on the dividend's high half.
    /// Read literally, signed |IXR| would flag every small negative dividend
    /// (-5 has IXR = FFFF), which no divider that divides magnitudes does.
    /// One real edge stays: a quotient of exactly -2^15 is representable but
    /// still trips the pretest, on this reading and on the 706's alike.
    fn div(&mut self, bus: &mut dyn Bus) -> StepResult {
        // "an execution time of 24.5 microseconds" (6-3): 14 cycles flat --
        // not the 706's 10-11, the two machines' dividers genuinely differed.
        self.cycles = 14;

        let m = self.mpy_div_operand(bus);
        let divisor = m as i16;
        let dividend = ((self.ixr as i16 as i32) << 15) | (self.acr & 0x7fff) as i32;

        if divisor == 0 || dividend.unsigned_abs() >> 15 >= divisor.unsigned_abs() as u32 {
            self.acr = (self.acr & 0x7fff) | (self.ixr & 0x8000);
            self.ovf = true;
            // "The execution time of the divide instruction for this
            // condition is 8.75 microseconds" (6-3): 5 cycles.
            self.cycles = 5;
        } else {
            self.acr = (dividend / divisor as i32) as u16;
            self.ixr = (dividend % divisor as i32) as u16;
        }
        self.reload_exr();
        StepResult::Ok
    }

    /// Clock cycles one instruction takes, from the opcode index (appendix B,
    /// transcribed in `Raytheon703refMan_isa.txt` section 8): JMP 1, STB 3,
    /// every other memory reference 2; INR 3, DIN/DOT/IXS/DXS 2, all other
    /// generics 1. Decoded from the word alone, independent of what the
    /// instruction then does -- appendix B gives one figure per opcode, with
    /// no data-dependent cases. The optional MPY/DIV are the exception:
    /// appendix B omits them and section 6 prices them by their data, so
    /// `mpy` and `div` overwrite whatever this returns.
    fn insn_cycles(insn: u16) -> u32 {
        match insn >> 12 {
            0x0 => {
                let fn_field = (insn >> 8) & 0x0f;
                let f1 = (insn >> 4) & 0x0f;
                match fn_field {
                    0x0 if f1 == 0x1 => 3, // INR
                    0x0 | 0x1 | 0x6 | 0x7 | 0x8 => 1, // controls, register ops, LLB/CLB, skips
                    0x2..=0x5 => 2, // DIN/DOT/IXS/DXS
                    // Shifts print as "2-5" for counts 0-15 and appendix B
                    // never says how the count splits the range, so this
                    // linear interpolation is a guess pinned only at the
                    // printed bounds: count 0 is 2 cycles, count 15 is 5.
                    0x9 | 0xa => 2 + (insn & 0x0f) as u32 / 5,
                    // MPY/DIV land here too: their exec paths set the real,
                    // data-dependent figure. Everything else in B-F stops
                    // the run as BadOpcode anyway.
                    _ => 1,
                }
            }
            0x1 => 1, // JMP
            0x3 => 3, // STB
            _ => 2,   // every other memory reference
        }
    }

    /// Apply a shift to one half of ACR. `left` picks the manual's "left
    /// byte", bits 0-7, which is the high half.
    fn byte_shift(&mut self, left: bool, f: impl Fn(u8) -> u8) {
        if left {
            self.acr = (self.acr & 0x00ff) | ((f((self.acr >> 8) as u8) as u16) << 8);
        } else {
            self.acr = (self.acr & 0xff00) | f(self.acr as u8) as u16;
        }
    }
}

impl Cpu for Cpu703 {
    fn reset(&mut self, _bus: &mut dyn Bus) {
        // The manual documents exactly this much: "Initialization of the
        // Raytheon 703 (power ON or RESET) sets all interrupt levels to the
        // Disabled state and removes the interrupt inhibit mask" (3-2).
        self.levels = [Level::default(); 16];
        self.inhibit = false;

        // There is no reset vector: on real hardware the operator keys a
        // starting address into the front panel and presses RUN. With no panel
        // here, word 0 is the convention, which is also where PTB is entered.
        self.pcr = 0;

        // ACR, IXR, EXR and the flip flops are left alone on purpose. Nothing
        // in the manual says RESET clears them, and PTB's operating procedure
        // is explicit that the operator sets the index register *after*
        // pressing RESET -- so clearing it here would wipe out this machine's
        // stand-in for that step. See `set_index`. (5-3 calls RESET "the
        // master reset switch that resets all registers"; deliberately not
        // taken literally, for the same PTB reason.)

        // A reset arrives halted, so nothing will republish the lamps until
        // the next step or keying -- without this, the panel's RESET button
        // looks like it did nothing.
        self.publish_panel();
    }

    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        // Latch whatever pulsed since the last instruction. A signal to a
        // Disabled level "is ignored" (3-1) rather than remembered; one to a
        // level that is already Active or Waiting just re-arms the same latch.
        //
        // The devices are paced by the cycle count handed over here, and what
        // is in `self.cycles` at this point is the *previous* step's, because
        // this instruction has not been decoded yet. That lag is one
        // instruction -- single-digit microseconds against a teletype's 100
        // milliseconds -- and it costs nothing to be exactly one behind
        // forever, where reordering the poll to after the instruction would
        // cost an interrupt its position between two instructions.
        let pulses = bus.poll_interrupt_lines(self.cycles);
        if pulses != 0 {
            for (level, l) in self.levels.iter_mut().enumerate() {
                if pulses & (1 << level) != 0 && l.enabled {
                    l.pending = true;
                }
            }
        }

        // The entry sequence runs between instructions, so PCR already points
        // at the return address that goes into the level's save word. Taking
        // it as a whole step keeps one `step()` equal to one thing the machine
        // did, and keeps the trace readable.
        if let Some(level) = self.ready_level() {
            self.enter_interrupt(bus, level);
            // Appendix B gives no figure for the entry sequence. It does the
            // mirror image of INR's three memory accesses (save PC and
            // status, read the linkage), and INR is 3 cycles, so call it 3.
            self.cycles = 3;
            // The entry is a step of its own, so the PC and SEQ lamps move;
            // the instruction register keeps its old contents, matching a
            // sequence that fetched nothing.
            self.publish_step();
            return StepResult::Ok;
        }

        let insn = self.read_word(bus, self.pcr);
        self.pcr = self.pcr.wrapping_add(1) & 0x7fff;
        self.cycles = Self::insn_cycles(insn);
        if let Some(p) = &self.panel {
            // 5-2 puts "the Instruction register in 0-7 of the SELECTED
            // DISPLAY indicators": the opcode byte.
            p.set_instruction((insn >> 8) as u8);
        }

        let result = if insn >> 12 == 0 {
            self.exec_generic(bus, insn)
        } else {
            self.exec_memory(bus, insn)
        };
        self.publish_step();
        result
    }

    fn last_step_cycles(&self) -> u32 {
        self.cycles
    }

    /// The panel's data-entry path (section 5). Runs on the CPU thread
    /// between instructions, halted or not -- the PROGRAM COUNTER row is
    /// "always active for both entry and display" (5-1), and keying the
    /// others while running is the operator's own lookout, as it was.
    /// Ends by republishing the point samples so the lamps update while
    /// halted; deliberately no accumulate -- switch actuations are not
    /// machine time.
    fn panel_command(&mut self, bus: &mut dyn Bus, cmd: &PanelCommand) {
        let Some(panel) = self.panel.clone() else { return };
        match cmd {
            // Indicator numbers count from the left, 0 the MSB; the PCR
            // mask drops indicator 0, which the row does not have.
            PanelCommand::TogglePcBit(i) => {
                self.pcr = (self.pcr ^ (0x8000 >> (i & 15))) & 0x7fff;
            }
            PanelCommand::ClearPc => self.pcr = 0,
            PanelCommand::ToggleSelectedBit(sel, i) => match sel {
                Selector::Mb => panel.toggle_mbr_bit(*i),
                Selector::Ix => self.ixr ^= 0x8000 >> (i & 15),
                Selector::Ac => self.acr ^= 0x8000 >> (i & 15),
                // MS, IN and MA are "active for display only" (5-2)
                _ => {}
            },
            PanelCommand::ClearSelected(sel) => match sel {
                Selector::Mb => panel.set_mbr(0),
                Selector::Ix => self.ixr = 0,
                Selector::Ac => self.acr = 0,
                _ => {}
            },
            // ENTER "enters the contents of Memory Buffer Register (MBR)
            // into the memory location specified by the 15-bit Program
            // Counter Register (PCR) and increments the PCR by one count"
            // (5-3). write_word lights the MA/MB lamps on the way.
            PanelCommand::Enter => {
                let mbr = panel.selected(Selector::Mb);
                self.write_word(bus, self.pcr, mbr);
                self.pcr = self.pcr.wrapping_add(1) & 0x7fff;
            }
            // DISPLAY "fetches data from the memory location specified by
            // the Program Counter Register (PCR) and enters the data into
            // the Memory Buffer Register (MBR)", incrementing the PCR
            // (5-3). read_word's publish is what loads the MBR.
            PanelCommand::Display => {
                let _ = self.read_word(bus, self.pcr);
                self.pcr = self.pcr.wrapping_add(1) & 0x7fff;
            }
            // the run-state switches are the run loop's, never sent here
            _ => {}
        }
        self.publish_panel();
    }

    fn dump(&self) {
        println!(
            "ACR 0x{:04x} IXR 0x{:04x} PCR 0x{:04x} EXR 0x{:02x} ({}{}{}{})",
            self.acr,
            self.ixr,
            self.pcr,
            self.exr,
            if self.neg { 'l' } else { ' ' },
            if self.eql { 'e' } else { ' ' },
            if self.ovf { 'o' } else { ' ' },
            if self.global { 'g' } else { ' ' },
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        // Nothing constrains this machine's format, so it simply follows
        // the shape of the others: PC first, then registers, and never the
        // opcode -- peeking at the instruction would be a memory read that an
        // untraced run doesn't do.
        writeln!(
            out,
            "PC={:04x} AC={:04x} IX={:04x} EX={:02x} ST={:04x}",
            self.pcr,
            self.acr,
            self.ixr,
            self.exr,
            self.status()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{run_steps, TestBus};

    /// Assemble words at word address `at` and reset with PC there.
    fn boot_at(at: u16, prog: &[u16]) -> (Cpu703, TestBus) {
        let mut bus = TestBus::new();
        let mut bytes = Vec::new();
        for w in prog {
            bytes.extend_from_slice(&w.to_be_bytes());
        }
        bus.load(at << 1, &bytes);
        let mut cpu = Cpu703::new();
        cpu.reset(&mut bus);
        cpu.pcr = at;
        (cpu, bus)
    }

    /// Most programs here start above the interrupt blocks, at word 0x40.
    fn boot(prog: &[u16]) -> (Cpu703, TestBus) {
        boot_at(0x40, prog)
    }

    fn word(bus: &mut TestBus, waddr: u16) -> u16 {
        bus.read16((waddr as u32) << 1, Endian::Big)
    }

    /// Reset does what the manual says it does -- disable every level, drop
    /// the inhibit mask -- and nothing more. In particular it must not touch
    /// the index register, which is the one thing PTB needs preset before RUN.
    #[test]
    fn reset_clears_the_interrupt_system_and_leaves_the_registers() {
        let mut bus = TestBus::new();
        let mut cpu = Cpu703::new();
        cpu.pcr = 0x1234;
        cpu.acr = 0x5678;
        cpu.set_index(0x01f4);
        cpu.inhibit = true;
        cpu.levels[3] = Level { enabled: true, pending: true, active: true };

        cpu.reset(&mut bus);

        assert_eq!(cpu.pcr, 0);
        assert!(!cpu.inhibit);
        assert!(cpu.levels.iter().all(|&l| l == Level::default()));
        assert_eq!(cpu.ixr, 0x01f4, "the front panel sets this after RESET");
        assert_eq!(cpu.acr, 0x5678);
    }

    // -- load/store, arithmetic, logic -------------------------------------

    #[test]
    fn word_loads_and_stores() {
        // LDW 0x50 / ADD 0x51 / STW 0x52
        let (mut cpu, mut bus) = boot(&[0x8050, 0xa051, 0x7052]);
        bus.load(0x50 << 1, &0x1111u16.to_be_bytes());
        bus.load(0x51 << 1, &0x2222u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.acr, 0x3333);
        assert_eq!(word(&mut bus, 0x52), 0x3333);
    }

    #[test]
    fn index_loads_and_stores() {
        // LDX 0x50 / STX 0x51
        let (mut cpu, mut bus) = boot(&[0x9050, 0x6051]);
        bus.load(0x50 << 1, &0xbeefu16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.ixr, 0xbeef);
        assert_eq!(word(&mut bus, 0x51), 0xbeef);
    }

    #[test]
    fn logic_and_subtract() {
        // LDW 0x50 / AND 0x51 / ORI 0x52 / ORE 0x53 / SUB 0x54
        let (mut cpu, mut bus) = boot(&[0x8050, 0xe051, 0xc052, 0xd053, 0xb054]);
        for (a, v) in [(0x50, 0xff00u16), (0x51, 0x0f0f), (0x52, 0x00f0), (0x53, 0xffff), (0x54, 1)]
        {
            bus.load(a << 1, &v.to_be_bytes());
        }
        run_steps(&mut cpu, &mut bus, 5);
        // ff00 & 0f0f = 0f00; | 00f0 = 0ff0; ^ ffff = f00f; - 1 = f00e
        assert_eq!(cpu.acr, 0xf00e);
    }

    /// Byte addressing: byte N is word N/2, high half when N is even. The PTB
    /// bootstrap depends on this -- it rewrites word 5 by storing to byte 10.
    #[test]
    fn byte_address_selects_the_half_word() {
        // LDB 0x0a / STB 0x0d  (byte 10 = word 5 high, byte 13 = word 6 low)
        let (mut cpu, mut bus) = boot(&[0x500a, 0x300d]);
        bus.load(0x05 << 1, &0xabcdu16.to_be_bytes());
        bus.load(0x06 << 1, &0x1122u16.to_be_bytes());
        cpu.acr = 0x9900;
        run_steps(&mut cpu, &mut bus, 2);
        // LDB replaced only the low half of ACR, leaving 0x99 on top
        assert_eq!(cpu.acr, 0x99ab);
        assert_eq!(word(&mut bus, 0x06), 0x11ab);
    }

    // -- compares and skips ------------------------------------------------

    #[test]
    fn word_compare_drives_the_skips() {
        // CMW 0x50 / SLS / (skipped) / SGR
        let (mut cpu, mut bus) = boot(&[0xf050, 0x0840, 0x0000, 0x0880]);
        bus.load(0x50 << 1, &0x0005u16.to_be_bytes());
        cpu.acr = 0x0003;
        run_steps(&mut cpu, &mut bus, 2);
        assert!(cpu.neg && !cpu.eql);
        assert_eq!(cpu.pcr, 0x43, "SLS should have skipped the filler word");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x44, "SGR must not skip when the compare was less");
    }

    #[test]
    fn word_compare_is_signed() {
        // CMW 0x50 with ACR = -1 against 1
        let (mut cpu, mut bus) = boot(&[0xf050]);
        bus.load(0x50 << 1, &0x0001u16.to_be_bytes());
        cpu.acr = 0xffff;
        run_steps(&mut cpu, &mut bus, 1);
        assert!(cpu.neg, "-1 < 1 -- an unsigned compare would say otherwise");
    }

    #[test]
    fn byte_compares_are_signed_eight_bit() {
        // CLB 0x01 with ACR low byte = 0x80 (-128)
        let (mut cpu, mut bus) = boot(&[0x0701]);
        cpu.acr = 0xff80;
        run_steps(&mut cpu, &mut bus, 1);
        assert!(cpu.neg && !cpu.eql, "-128 < 1 as signed bytes");
    }

    #[test]
    fn accumulator_skips() {
        let cases = [
            (0x0800u16, 0u16, true),      // SAZ, zero
            (0x0800, 1, false),           // SAZ, non-zero
            (0x0810, 0x7fff, true),       // SAP
            (0x0810, 0x8000, false),      // SAP on negative
            (0x0820, 0x8000, true),       // SAM
            (0x0830, 0x0003, true),       // SAO
            (0x0830, 0x0002, false),      // SAO on even
        ];
        for (insn, acr, skips) in cases {
            let (mut cpu, mut bus) = boot(&[insn]);
            cpu.acr = acr;
            run_steps(&mut cpu, &mut bus, 1);
            let expect = 0x41 + u16::from(skips);
            assert_eq!(cpu.pcr, expect, "insn {insn:04x} acr {acr:04x}");
        }
    }

    /// SNO is the only instruction that clears the overflow flip flop, so
    /// overflow is sticky across everything else (2-13).
    #[test]
    fn overflow_is_sticky_until_sno() {
        // ADD 0x50 / ADD 0x50 / SNO / SNO
        let (mut cpu, mut bus) = boot(&[0xa050, 0xa050, 0x08a0, 0x08a0]);
        bus.load(0x50 << 1, &0x7000u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert!(cpu.ovf, "0x7000 + 0x7000 overflows a signed 16-bit add");
        let pc = cpu.pcr;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, pc + 1, "SNO must not skip while overflow is set");
        assert!(!cpu.ovf, "SNO clears it");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, pc + 3, "and now SNO skips");
    }

    /// There is no front panel and no external sense line, so all five sense
    /// skips see a false input -- and all five skip when their input is false.
    #[test]
    fn sense_skips_always_skip() {
        for insn in [0x08b0u16, 0x08c0, 0x08d0, 0x08e0, 0x08f0] {
            let (mut cpu, mut bus) = boot(&[insn]);
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!(cpu.pcr, 0x42, "insn {insn:04x}");
        }
    }

    // -- literal generics --------------------------------------------------

    #[test]
    fn index_skips_test_the_new_value() {
        // IXS 1 from -1 lands on 0, which is >= 0, so it skips
        let (mut cpu, mut bus) = boot(&[0x0401]);
        cpu.ixr = 0xffff;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.ixr, 0);
        assert_eq!(cpu.pcr, 0x42);

        // DXS 1 from 0 lands on -1, which is < 0, so it skips
        let (mut cpu, mut bus) = boot(&[0x0501]);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.ixr, 0xffff);
        assert_eq!(cpu.pcr, 0x42);

        // ...and IXS with a still-negative result does not
        let (mut cpu, mut bus) = boot(&[0x0401]);
        cpu.ixr = 0xff00;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x41);
    }

    #[test]
    fn load_literal_byte_leaves_the_high_half() {
        let (mut cpu, mut bus) = boot(&[0x0638]);
        cpu.acr = 0xaa55;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xaa38);
    }

    // -- register generics -------------------------------------------------

    /// rustheon decodes this group one slot high. The manual's appendix B and
    /// section 2-7.2 both put CLR at 0x010, and everything else follows.
    #[test]
    fn register_generics_use_the_manuals_encoding() {
        let (mut cpu, mut bus) = boot(&[0x0100]);
        cpu.acr = 0x1234;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0, "0x0100 is CLR");

        let (mut cpu, mut bus) = boot(&[0x0110]);
        cpu.acr = 0x0003;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xfffd, "0x0110 is CMP, two's complement");

        let (mut cpu, mut bus) = boot(&[0x0120]);
        cpu.acr = 0x0003;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xfffc, "0x0120 is INV, one's complement");

        let (mut cpu, mut bus) = boot(&[0x0130]);
        cpu.acr = 0x4321;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.ixr, 0x4321, "0x0130 is CAX");

        let (mut cpu, mut bus) = boot(&[0x0140]);
        cpu.ixr = 0x8765;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x8765, "0x0140 is CXA");
    }

    #[test]
    fn complement_overflows_only_on_the_most_negative_value() {
        let (mut cpu, mut bus) = boot(&[0x0110]);
        cpu.acr = 0x8000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x8000);
        assert!(cpu.ovf);
    }

    // -- shifts ------------------------------------------------------------

    #[test]
    fn arithmetic_shifts_preserve_the_sign() {
        let (mut cpu, mut bus) = boot(&[0x0902]); // SRA 2
        cpu.acr = 0x8000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xe000);

        let (mut cpu, mut bus) = boot(&[0x0911]); // SLA 1
        cpu.acr = 0x4000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x8000);
        assert!(cpu.ovf, "SLA sets overflow when it changes the sign bit");
    }

    /// The index register is the right extension of the accumulator for the
    /// double-length shifts (2-7.6), so bits cross between them.
    #[test]
    fn double_shifts_couple_acr_and_ixr() {
        let (mut cpu, mut bus) = boot(&[0x0921]); // SRAD 1
        cpu.acr = 0x0001;
        cpu.ixr = 0x0000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.acr, cpu.ixr), (0x0000, 0x8000), "ACR bit 15 -> IXR bit 0");

        let (mut cpu, mut bus) = boot(&[0x0931]); // SLAD 1
        cpu.acr = 0x0000;
        cpu.ixr = 0x8000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.acr, cpu.ixr), (0x0001, 0x0000), "IXR bit 0 -> ACR bit 15");

        let (mut cpu, mut bus) = boot(&[0x0a64]); // SRCD 4
        cpu.acr = 0x1234;
        cpu.ixr = 0x5678;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.acr, cpu.ixr), (0x8123, 0x4567), "a 32-bit rotate");
    }

    #[test]
    fn logical_shifts_do_not_preserve_the_sign() {
        let (mut cpu, mut bus) = boot(&[0x0a02]); // SRL 2
        cpu.acr = 0x8000;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x2000);

        let (mut cpu, mut bus) = boot(&[0x0a44]); // SRC 4
        cpu.acr = 0x1234;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x4123);
    }

    /// The byte shifts work strictly within one half of ACR and leave the
    /// other half alone -- the one shift family with no 16-bit equivalent.
    #[test]
    fn byte_shifts_leave_the_other_half_alone() {
        let (mut cpu, mut bus) = boot(&[0x0a84]); // SRLL 4, high byte
        cpu.acr = 0xf00f;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x0f0f);

        let (mut cpu, mut bus) = boot(&[0x0aa4]); // SRLR 4, low byte
        cpu.acr = 0xf0f0;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xf00f);

        let (mut cpu, mut bus) = boot(&[0x0ac4]); // SRCL 4, rotate high byte
        cpu.acr = 0x12ff;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x21ff);

        let (mut cpu, mut bus) = boot(&[0x0af4]); // SLCR 4, rotate low byte
        cpu.acr = 0xff12;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0xff21);
    }

    // -- addressing --------------------------------------------------------

    /// EXR is a *byte* page number, so a word instruction uses only its top
    /// four bits: SML 4 selects byte page 4, which is word page 2, which
    /// starts at word 4096. This halving is the easiest thing in the ISA to
    /// get backwards, and nothing that runs in page 0 would ever notice.
    #[test]
    fn the_extension_register_is_a_byte_page() {
        // SML 4 / LDW 0x001   -- word page 2 = word 0x1000
        let (mut cpu, mut bus) = boot(&[0x0084, 0x8001]);
        bus.load(0x1001 << 1, &0x4141u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.acr, 0x4141);

        // SML 4 / LDB 0x001 -- byte page 4 = byte 0x2000, so byte 0x2001,
        // which is the *low* half of word 0x1000.
        let (mut cpu, mut bus) = boot(&[0x0084, 0x5001]);
        bus.load(0x1000 << 1, &0x3132u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.acr & 0xff, 0x32);
    }

    #[test]
    fn select_memory_upper_sets_the_top_page_bit() {
        // SMU 0 / LDW 0x002 -- byte page 0x10 = word page 8 = word 0x4000
        let (mut cpu, mut bus) = boot(&[0x0090, 0x8002]);
        bus.load(0x4002 << 1, &0x5555u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.acr, 0x5555);
    }

    /// The extension register reverts to the page of the program counter after
    /// every memory reference (1-3), so SML/SMU govern exactly one
    /// instruction. Get the order backwards and SML stops working entirely.
    #[test]
    fn the_extension_register_reverts_after_one_reference() {
        // SML 4 / LDW 0x001 / LDW 0x002
        let (mut cpu, mut bus) = boot(&[0x0084, 0x8001, 0x8002]);
        bus.load(0x1001 << 1, &0x4141u16.to_be_bytes());
        bus.load(0x0002 << 1, &0x2727u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.acr, 0x4141, "the first reference used the selected page");
        assert_eq!(cpu.exr, 0, "and then EXR fell back to the program's own page");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x2727, "the second reference is local again");
    }

    #[test]
    fn generics_do_not_disturb_the_extension_register() {
        // SML 4 / CLR / LDW 0x001 -- a generic between the two must not
        // reload EXR, or SML could never be used with anything.
        let (mut cpu, mut bus) = boot(&[0x0084, 0x0100, 0x8001]);
        bus.load(0x1001 << 1, &0x4141u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.acr, 0x4141);
    }

    #[test]
    fn a_program_outside_page_zero_addresses_its_own_page() {
        // LDW 0x001 executed from word 0x1000 (word page 2) must reach word
        // 0x1001, because EXR tracks the program counter.
        let (mut cpu, mut bus) = boot_at(0x1000, &[0x8001]);
        bus.load(0x1001 << 1, &0x6363u16.to_be_bytes());
        // reaching that page normally happens via a jump; do it by hand
        cpu.exr = 4;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x6363);
    }

    #[test]
    fn indexed_word_addressing_adds_words_and_indexed_byte_adds_bytes() {
        // LDW *0x050 with IXR = 2  -> word 0x52
        let (mut cpu, mut bus) = boot(&[0x8850]);
        bus.load(0x52 << 1, &0x1357u16.to_be_bytes());
        cpu.ixr = 2;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr, 0x1357);

        // LDB *0x0a0 with IXR = 3 -> byte 0xa3, the low half of word 0x51
        let (mut cpu, mut bus) = boot(&[0x58a0]);
        bus.load(0x51 << 1, &0x0099u16.to_be_bytes());
        cpu.ixr = 3;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.acr & 0xff, 0x99);
    }

    /// Global mode substitutes zeros for EXR when forming an indexed base
    /// address (1-3.3.3) but leaves direct addressing alone (1-3.1). rustheon
    /// tests this with `(status | ADFGBL) != 0`, which is always true.
    #[test]
    fn global_mode_affects_indexing_but_not_direct_addressing() {
        // SGM / SML 4 / LDW *0x001 -- indexed and global, so EXR is ignored
        // and the address is 0x001 + IXR. SGM comes first because SML only
        // survives until the next memory reference.
        let (mut cpu, mut bus) = boot(&[0x0050, 0x0084, 0x8801]);
        bus.load(0x0003 << 1, &0x1010u16.to_be_bytes());
        bus.load(0x1003 << 1, &0x2020u16.to_be_bytes());
        cpu.ixr = 2;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.acr, 0x1010, "global indexing ignores EXR");

        // the same thing in local mode reaches the selected page
        let (mut cpu, mut bus) = boot(&[0x0040, 0x0084, 0x8801]);
        bus.load(0x0003 << 1, &0x1010u16.to_be_bytes());
        bus.load(0x1003 << 1, &0x2020u16.to_be_bytes());
        cpu.ixr = 2;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.acr, 0x2020, "local indexing uses EXR");

        // and direct addressing uses EXR in either mode
        let (mut cpu, mut bus) = boot(&[0x0050, 0x0084, 0x8001]);
        bus.load(0x1001 << 1, &0x3030u16.to_be_bytes());
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.acr, 0x3030, "SGM must not disturb direct addressing");
    }

    #[test]
    fn copy_between_extension_and_index_uses_the_top_five_bits() {
        // CEX with EXR = 0x1f must set the top five bits of IXR
        let (mut cpu, mut bus) = boot(&[0x0060]);
        cpu.exr = 0x1f;
        cpu.ixr = 0x0123;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.ixr, 0xf923);

        // CXE reads them back
        let (mut cpu, mut bus) = boot(&[0x0070]);
        cpu.ixr = 0xf800;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.exr, 0x1f);
    }

    #[test]
    fn the_program_counter_wraps_at_fifteen_bits() {
        let (mut cpu, mut bus) = boot_at(0x7fff, &[0x0100]); // CLR at the top
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0, "PCR is 15 bits and must not reach 0x8000");
    }

    // -- jumps -------------------------------------------------------------

    #[test]
    fn jump_and_store_return_forces_global_mode() {
        // SLM / JSX 0x060
        let (mut cpu, mut bus) = boot(&[0x0040, 0x2060]);
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.pcr, 0x60);
        assert_eq!(cpu.ixr, 0x42, "the return address is the word after JSX");
        assert!(cpu.global, "JSX forces global mode prior to the transfer");
    }

    /// A branch to self stops the emulator only when nothing could ever wake
    /// it. PTB idles in `JMP $` waiting for a paper-tape frame, so copying the
    /// 6800's unconditional detection here would break every real program.
    #[test]
    fn jump_to_self_stops_only_with_interrupts_dead() {
        let (mut cpu, mut bus) = boot(&[0x1040]);
        assert_eq!(cpu.step(&mut bus), StepResult::InfiniteLoop);

        // with a level enabled, the same instruction is a legitimate idle loop
        let (mut cpu, mut bus) = boot(&[0x1040]);
        cpu.levels[0].enabled = true;
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);

        // ...unless the inhibit mask is on, in which case nothing can arrive
        let (mut cpu, mut bus) = boot(&[0x1040]);
        cpu.levels[0].enabled = true;
        cpu.inhibit = true;
        assert_eq!(cpu.step(&mut bus), StepResult::InfiniteLoop);
    }

    #[test]
    fn halt_stops_the_core() {
        let (mut cpu, mut bus) = boot(&[0x0000]);
        assert_eq!(cpu.step(&mut bus), StepResult::Halted);
    }

    /// Everything appendix B leaves out reports a bad opcode rather than
    /// quietly doing nothing: the tail of the arithmetic shift class -- 090
    /// to 093 are assigned and 094 upwards are not -- and all of classes B-F
    /// except the exact MPY/DIV words that section 6 diagrams. The near
    /// misses matter most: the option decodes 0B0F and 0C0F, nothing else.
    #[test]
    fn unassigned_generics_are_bad_opcodes() {
        for insn in [
            0x0940u16, // past SLAD, the last assigned arithmetic shift
            0x09f0,    // ...and the top of that class
            0x0150,    // past CXA, the last register generic
            0x00c0,    // past UNM, the last control generic
            0x0b00,    // class B holds only MPY = 0B0F...
            0x0b0e,    // ...its F2 is "always the number 15"
            0x0b1f,    // ...and its F1 is always 0 (0B8F is the 704's indexed
            0x0b8f,    //    MPY; the 703 manual says no indexing, 6-1)
            0x0c00,    // same for DIV in class C
            0x0c8f,
            0x0f00,    // class F has nothing at all
        ] {
            let (mut cpu, mut bus) = boot(&[insn]);
            assert_eq!(cpu.step(&mut bus), StepResult::BadOpcode, "insn {insn:04x}");
        }
    }

    /// ...but every encoding the appendix does assign executes.
    #[test]
    fn every_assigned_generic_executes() {
        let mut assigned: Vec<u16> = vec![0x0000]; // HLT is assigned but stops
        for f1 in 0x1..=0xb {
            assigned.push(f1 << 4); // control generics, class 0
        }
        for f1 in 0x0..=0x4 {
            assigned.push(0x0100 | (f1 << 4)); // register generics
        }
        for fn_field in [0x2, 0x3, 0x4, 0x5, 0x6, 0x7] {
            assigned.push(fn_field << 8); // DIN, DOT, IXS, DXS, LLB, CLB
        }
        for f1 in 0x0..=0xf {
            assigned.push(0x0800 | (f1 << 4)); // skips
            assigned.push(0x0a00 | (f1 << 4)); // logical shifts
        }
        for f1 in 0x0..=0x3 {
            assigned.push(0x0900 | (f1 << 4)); // arithmetic shifts
        }
        assigned.push(0x0b0f); // MPY (section 6; the address word reads as 0)
        assigned.push(0x0c0f); // DIV overflows on divisor 0, but executes

        for insn in assigned {
            let (mut cpu, mut bus) = boot(&[insn]);
            let expected =
                if insn == 0x0000 { StepResult::Halted } else { StepResult::Ok };
            assert_eq!(cpu.step(&mut bus), expected, "insn {insn:04x}");
        }
    }

    // -- optional multiply/divide (section 6) ------------------------------

    /// The 706 users manual's worked MPY example (2-1.10), transplanted: the
    /// 703's own section 6 diagrams the same encoding but prints no vector.
    /// X'4000' times X'0040' is 2^20: IXR gets the product's high 16 bits,
    /// ACR its low 15, and the PC steps over both instruction words.
    #[test]
    fn mpy_matches_the_manuals_worked_example() {
        let (mut cpu, mut bus) = boot_at(0x100, &[0x0b0f, 0x0200]);
        cpu.acr = 0x4000;
        bus.load(0x200 << 1, &0x0040u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.ixr, 0x0020);
        assert_eq!(cpu.acr, 0x0000);
        assert_eq!(cpu.pcr, 0x102, "two-word instruction");
        assert!(!cpu.ovf);
    }

    /// "The most significant bit of the accumulator is set to the most
    /// significant bit of the index register" (6-2): a negative product reads
    /// back as its own low 16 bits even though ACR only holds 15 of them.
    #[test]
    fn mpy_duplicates_the_product_sign_into_acr0() {
        let (mut cpu, mut bus) = boot(&[0x0b0f, 0x0200]);
        cpu.acr = 0xfffd; // -3
        bus.load(0x200 << 1, &0x0005u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.ixr, 0xffff, "-15 >> 15");
        assert_eq!(cpu.acr, 0xfff1, "low 15 bits of -15, sign duplicated");
    }

    /// "No overflow can result from the multiply instruction" (6-2), even for
    /// X'8000' squared where the +2^30 product aliases to a negative in the
    /// 31-bit format. The 704/706 option and the 1968 software multiply all
    /// flag that case; the 703's manual says the flag stays off.
    #[test]
    fn mpy_never_sets_overflow() {
        let (mut cpu, mut bus) = boot(&[0x0b0f, 0x0200]);
        cpu.acr = 0x8000;
        bus.load(0x200 << 1, &0x8000u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ixr, cpu.acr), (0x8000, 0x8000));
        assert!(!cpu.ovf, "6-2 is unambiguous, unlike the 704/706");
    }

    /// MPY prices itself by the ones in the multiplier (12.25 to 17.5 us,
    /// 6-2; the split follows the 706's per-ones formula, which spans the
    /// same 7 to 10 cycles).
    #[test]
    fn mpy_cycles_follow_the_ones_in_the_multiplier() {
        let (mut cpu, mut bus) = boot(&[0x0b0f, 0x0200]);
        cpu.acr = 0; // no ones: the 12.25us floor
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.last_step_cycles(), 7);

        let (mut cpu, mut bus) = boot(&[0x0b0f, 0x0200]);
        cpu.acr = 0xffff; // all ones: the 17.5us ceiling
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.last_step_cycles(), 10);
    }

    /// The second word is a flat 15-bit address: no EXR page base, no
    /// indexing, and its bit 0 (the MSB) is hatched out of the format
    /// diagram, so X'FFFF' addresses the very top word of core.
    #[test]
    fn mpy_operand_address_is_flat_15_bits() {
        let (mut cpu, mut bus) = boot(&[0x0b0f, 0xffff]);
        cpu.acr = 2;
        cpu.ixr = 0xaaaa; // must not participate in the addressing
        bus.load(0x7fff << 1, &0x0003u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ixr, cpu.acr), (0, 6));
    }

    /// MPY/DIV reference memory, so EXR reloads to the next instruction's
    /// byte page afterward, exactly like every other memory reference.
    #[test]
    fn mpy_reloads_exr_like_a_memory_reference() {
        let (mut cpu, mut bus) = boot_at(0x0c00, &[0x0b0f, 0x0200]);
        cpu.exr = 0x1f;

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.exr, 3, "byte page of the next instruction at 0xc02");
    }

    /// The 706 users manual's worked DIV example (2-1.10): the 31-bit
    /// dividend IXR:ACR(1-15) = 1:0 is +32768; over 2 it leaves the quotient
    /// X'4000' in ACR and no remainder in IXR.
    #[test]
    fn div_matches_the_manuals_worked_example() {
        let (mut cpu, mut bus) = boot_at(0x100, &[0x0c0f, 0x0200]);
        cpu.ixr = 0x0001;
        cpu.acr = 0x0000;
        bus.load(0x200 << 1, &0x0002u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.acr, 0x4000);
        assert_eq!(cpu.ixr, 0x0000);
        assert_eq!(cpu.pcr, 0x102, "two-word instruction");
        assert!(!cpu.ovf);
        assert_eq!(cpu.last_step_cycles(), 14, "24.5us, the 703's figure");
    }

    /// "The sign of the remainder is the same as that of the dividend" (6-3),
    /// and "the most significant bit of the accumulator has no effect": -5
    /// over 2 is quotient -2 remainder -1, whatever ACR bit 0 held.
    #[test]
    fn div_remainder_keeps_the_dividends_sign() {
        let (mut cpu, mut bus) = boot(&[0x0c0f, 0x0200]);
        cpu.ixr = 0xffff; // -5 in the 31-bit format...
        cpu.acr = 0xfffb; // ...with the no-effect bit deliberately set
        bus.load(0x200 << 1, &0x0002u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.acr as i16, -2);
        assert_eq!(cpu.ixr as i16, -1);
    }

    /// The overflow pretest reads the *magnitude* of the dividend's high
    /// half. A small negative dividend sign-extends to IXR = X'FFFF', which
    /// a literal signed |IXR| reading would flag; dividing it by 1 must
    /// simply return it.
    #[test]
    fn div_of_a_small_negative_dividend_does_not_overflow() {
        let (mut cpu, mut bus) = boot(&[0x0c0f, 0x0200]);
        cpu.ixr = 0xffff;
        cpu.acr = 0x7ffb; // -5
        bus.load(0x200 << 1, &0x0001u16.to_be_bytes());

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.acr as i16, -5);
        assert_eq!(cpu.ixr, 0, "no remainder");
        assert!(!cpu.ovf);
    }

    /// "The overflow indicator is turned ON and the dividend contained in the
    /// index register and the accumulator remains unchanged, except the most
    /// significant bit of the accumulator is set to the most significant bit
    /// of the index register" (6-3), in 8.75us -- for a divisor of zero and
    /// for a quotient that cannot fit 16 bits alike.
    #[test]
    fn div_overflow_leaves_the_dividend_and_flags() {
        // divisor zero; IXR's clear sign bit lands in ACR bit 0
        let (mut cpu, mut bus) = boot(&[0x0c0f, 0x0200]);
        cpu.ixr = 0x0001;
        cpu.acr = 0xa345;
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ixr, cpu.acr), (0x0001, 0x2345));
        assert!(cpu.ovf);
        assert_eq!(cpu.last_step_cycles(), 5, "8.75us early-out");

        // quotient too big: +65536 over 2 would be +32768
        let (mut cpu, mut bus) = boot(&[0x0c0f, 0x0200]);
        cpu.ixr = 0x0002;
        cpu.acr = 0x0000;
        bus.load(0x200 << 1, &0x0002u16.to_be_bytes());
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ixr, cpu.acr), (0x0002, 0x0000));
        assert!(cpu.ovf);

        // a negative high half copies a set sign bit into ACR bit 0
        let (mut cpu, mut bus) = boot(&[0x0c0f, 0x0200]);
        cpu.ixr = 0x8002;
        cpu.acr = 0x0000;
        bus.load(0x200 << 1, &0x0002u16.to_be_bytes());
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ixr, cpu.acr), (0x8002, 0x8000));
        assert!(cpu.ovf);
    }

    // -- interrupts --------------------------------------------------------

    /// The four words at `level * 4`: PC save, linkage, status save, unused.
    #[test]
    fn interrupt_entry_uses_the_level_block() {
        let (mut cpu, mut bus) = boot(&[0x0100]);
        bus.load(0x0d << 1, &0x0200u16.to_be_bytes()); // level 3 linkage
        cpu.levels[3].enabled = true;
        cpu.neg = true;
        cpu.exr = 4;
        bus.int_lines = 1 << 3;

        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(word(&mut bus, 0x0c), 0x40, "PC save at level*4");
        assert_eq!(word(&mut bus, 0x0e), 0x2400, "status save at level*4+2");
        assert_eq!(cpu.pcr, 0x200, "linkage at level*4+1");
        assert!(cpu.global, "entry places the CPU in global mode");
    }

    #[test]
    fn interrupt_return_restores_the_saved_state() {
        // INR 3 as the whole interrupt subroutine
        let (mut cpu, mut bus) = boot_at(0x200, &[0x0013]);
        bus.load(0x0d << 1, &0x0200u16.to_be_bytes());
        cpu.pcr = 0x40;
        cpu.levels[3].enabled = true;
        cpu.neg = true;
        cpu.exr = 4;
        bus.int_lines = 1 << 3;
        run_steps(&mut cpu, &mut bus, 1); // entry
        assert!(cpu.levels[3].active);
        run_steps(&mut cpu, &mut bus, 1); // INR
        assert_eq!(cpu.pcr, 0x40);
        assert_eq!(cpu.exr, 4);
        assert!(cpu.neg && !cpu.global);
        assert!(!cpu.levels[3].active, "the level is back to Idle");
    }

    /// "An interrupt signal sent from an external device is ignored" while the
    /// level is Disabled (3-1) -- not deferred, not latched.
    #[test]
    fn a_signal_to_a_disabled_level_is_dropped() {
        let (mut cpu, mut bus) = boot(&[0x0100, 0x0100]);
        bus.load(0x01 << 1, &0x0200u16.to_be_bytes());
        bus.int_lines = 1;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x41, "no entry happened");
        cpu.levels[0].enabled = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x42, "and enabling later does not resurrect it");
    }

    #[test]
    fn the_inhibit_mask_defers_rather_than_drops() {
        // MSK / CLR / UNM
        let (mut cpu, mut bus) = boot(&[0x00a0, 0x0100, 0x00b0]);
        bus.load(0x01 << 1, &0x0200u16.to_be_bytes());
        cpu.levels[0].enabled = true;
        run_steps(&mut cpu, &mut bus, 1); // MSK
        bus.int_lines = 1;
        run_steps(&mut cpu, &mut bus, 2); // CLR, UNM -- both must run
        assert_eq!(cpu.pcr, 0x43);
        run_steps(&mut cpu, &mut bus, 1); // now the deferred entry happens
        assert_eq!(cpu.pcr, 0x200);
    }

    /// A higher-priority subroutine in progress postpones lower levels, but a
    /// higher level interrupts a lower one (3-3).
    #[test]
    fn priority_runs_from_fifteen_down() {
        let (mut cpu, mut bus) = boot(&[0x0100]);
        bus.load(0x01 << 1, &0x0300u16.to_be_bytes()); // level 0 linkage
        bus.load(0x1d << 1, &0x0400u16.to_be_bytes()); // level 7 linkage
        bus.load(0x400 << 1, &0x0100u16.to_be_bytes()); // CLR, so the ISR runs
        cpu.levels[0].enabled = true;
        cpu.levels[7].enabled = true;
        bus.int_lines = (1 << 0) | (1 << 7);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x400, "level 7 wins");
        assert!(cpu.levels[0].pending, "level 0 is still waiting");

        // while level 7 is active, level 0 cannot run
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x401);

        // but level 8 can
        bus.load(0x21 << 1, &0x0500u16.to_be_bytes());
        cpu.levels[8].enabled = true;
        bus.int_lines = 1 << 8;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x500);
    }

    /// A pulse that arrives while the subroutine is running stays latched, so
    /// a device that interrupts once per character does not lose one to the
    /// service routine it just woke.
    #[test]
    fn a_signal_during_a_subroutine_fires_again_after_the_return() {
        // the subroutine at 0x200 is just INR 0
        let (mut cpu, mut bus) = boot(&[0x0100, 0x0100]);
        bus.load(0x01 << 1, &0x0200u16.to_be_bytes());
        bus.load(0x200 << 1, &0x0010u16.to_be_bytes());
        cpu.levels[0].enabled = true;
        bus.int_lines = 1;
        run_steps(&mut cpu, &mut bus, 1); // entry
        assert_eq!(cpu.pcr, 0x200);
        bus.int_lines = 1; // the device pulses again mid-subroutine
        run_steps(&mut cpu, &mut bus, 1); // INR
        assert_eq!(cpu.pcr, 0x40);
        run_steps(&mut cpu, &mut bus, 1); // and straight back in
        assert_eq!(cpu.pcr, 0x200);
    }

    #[test]
    fn disable_drops_a_pending_signal_and_an_active_subroutine() {
        // DSB 0
        let (mut cpu, mut bus) = boot(&[0x0030]);
        cpu.levels[0] = Level { enabled: true, pending: true, active: true };
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.levels[0], Level::default());
    }

    // -- direct I/O --------------------------------------------------------

    #[test]
    fn dio_moves_whole_words_over_the_device_function_address() {
        // DOT 14,9 / DIN 14,13, the two the PTB bootstrap uses
        let (mut cpu, mut bus) = boot(&[0x03e9, 0x02ed]);
        bus.ports16[0xed] = 0x00c1;
        cpu.acr = 0xa5a5;
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(bus.io16_writes, vec![(0xe9, 0xa5a5)]);
        assert_eq!(cpu.acr, 0x00c1);
    }

    // -- the bootstrap itself ----------------------------------------------

    /// The eleven words of PTB (drawing 390364) exercise eight instruction
    /// encodings, self-modify, and depend on byte addressing, the interrupt
    /// frame and the `JMP $` idle loop all being right. Running it against a
    /// synthetic tape is the closest thing this core has to a period
    /// regression test.
    #[test]
    fn the_paper_tape_bootstrap_loads_a_tape() {
        const PTB: [u16; 11] = [
            0x0020, // 0  ENB 0
            0x8004, // 1  LDW SERV   -- and the level 0 linkage word, see below
            0x03e9, // 2  DOT 14,9   start tape
            0x1003, // 3  JMP $      wait for interrupt
            0x02ed, // 4  DIN 14,13  input frame
            0x0800, // 5  SAZ        rewritten to STB *0 after the first frame
            0x0401, // 6  IXS 1
            0x0010, // 7  INR 0
            0x0638, // 8  LLB X'38'
            0x300a, // 9  STB /TEST
            0x0010, // A  INR 0
        ];

        let (mut cpu, mut bus) = boot_at(0, &PTB);

        // Word 1 does double duty, which is the trick that makes PTB fit in
        // eleven words: it is the level-0 linkage word (3-1), and it is also a
        // real instruction on the straight-line path. As a linkage address
        // 0x8004 masks to 15 bits and sends the hardware to word 4; executed
        // once on the way past, it is a harmless `LDW 4`.

        // The index is preset to (byte origin - 12); with an origin of word
        // 0x100 that is byte 0x200 - 12.
        cpu.ixr = 0x0200 - 12;

        // A tape of leader nulls, twelve bytes consumed by the self-
        // modification dance, then the payload.
        let payload: Vec<u8> = (0..8).map(|i| 0x40 + i).collect();
        let mut tape = vec![0u8, 0, 0];
        tape.extend(std::iter::repeat_n(0xffu8, 12));
        tape.extend_from_slice(&payload);

        let mut frames = tape.into_iter();
        let mut armed = true;
        for _ in 0..2000 {
            // the reader only runs once the program has started it with
            // DOT 14,9, and then interrupts level 0 once per frame
            if armed && !bus.io16_writes.is_empty() {
                match frames.next() {
                    Some(f) => {
                        bus.ports16[0xed] = f as u16;
                        bus.int_lines |= 1;
                        armed = false;
                    }
                    None => break,
                }
            }
            // it re-arms once the frame has been collected by DIN, which is
            // the only reason the program counter ever reaches word 5
            if cpu.pcr == 5 {
                armed = true;
            }
            assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        }

        assert_eq!(
            word(&mut bus, 5) >> 8,
            0x38,
            "the service routine should have rewritten TEST into STB *0"
        );
        for (i, b) in payload.iter().enumerate() {
            assert_eq!(
                bus.read8(0x200 + i as u32),
                *b,
                "payload byte {i} should have landed at the program origin"
            );
        }
    }

    // -- front panel -------------------------------------------------------

    /// The lamp registers reflect the machine after every step.
    #[test]
    fn panel_publishes_registers_after_each_step() {
        use crate::console::{PanelState, Selector};
        // LLB 0x21 / CAX / LLB 0x33
        let (mut cpu, mut bus) = boot(&[0x0621, 0x0130, 0x0633]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(panel.program_counter(), 0x42);
        assert_eq!(panel.selected(Selector::Ac), 0x0021);
        assert_eq!(panel.selected(Selector::Ix), 0x0021);
        // the CAX fetch was the last word through the memory buffer
        assert_eq!(panel.selected(Selector::Ma), 0x41 << 1);
        assert_eq!(panel.selected(Selector::Mb), 0x0130);
        // IN shows the opcode byte in indicators 0-7, i.e. the high half
        assert_eq!(panel.selected(Selector::In), 0x0100);

        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(panel.selected(Selector::Ac), 0x0033);
    }

    /// The MS position is the saved-status layout with the sequence register
    /// in indicators 12-15: take a level 5 interrupt with flags set and read
    /// the lamps.
    #[test]
    fn panel_ms_word_is_status_plus_sequence() {
        use crate::console::{PanelState, Selector};
        let (mut cpu, mut bus) = boot(&[0x0025]); // ENB 5
        bus.load(5 * 4 * 2 + 2, &[0x00, 0x50]); // level 5 linkage -> word 0x50
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        cpu.eql = true;
        cpu.global = true;

        run_steps(&mut cpu, &mut bus, 1);
        // base level: EXR follows the PC's byte page, SEQ shows 0
        assert_eq!(panel.selected(Selector::Ms), cpu.status());

        bus.int_lines = 1 << 5;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x50, "should have entered the level 5 routine");
        assert_eq!(panel.program_counter(), 0x50);
        assert_eq!(panel.selected(Selector::Ms), cpu.status() | 5);
        assert_ne!(panel.selected(Selector::Ms) & ST_EQL, 0);
    }

    /// A SENSE toggle in the up (true) position defeats its skip -- they
    /// take the false position (5-4) -- while SSE keeps skipping: nothing
    /// drives the external line, panel or no panel.
    #[test]
    fn sense_switch_up_defeats_the_skip() {
        use crate::console::PanelState;
        for switch in 0..4u8 {
            let insn = 0x08c0 + ((switch as u16) << 4);
            let (mut cpu, mut bus) = boot(&[insn]);
            let panel = PanelState::new();
            cpu.attach_panel(panel.clone());
            panel.toggle_sense(switch);
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!(cpu.pcr, 0x41, "SS{switch} must not skip with its toggle up");
        }

        let (mut cpu, mut bus) = boot(&[0x08b0]); // SSE
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        for n in 0..4 {
            panel.toggle_sense(n);
        }
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pcr, 0x42, "SSE reads the J2 line, not the toggles");
    }

    /// The lamp accumulators charge each step's cycle count to the bits the
    /// step left set, so a steadily lit bit reads the full total.
    #[test]
    fn lamp_accumulators_charge_per_instruction_cycles() {
        use crate::console::{PanelState, Selector};
        // LLB 0xff, then JMP back to it: two 1-cycle instructions
        let (mut cpu, mut bus) = boot(&[0x06ff, 0x1040]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        run_steps(&mut cpu, &mut bus, 10);
        let ac = panel.snapshot(Selector::Ac);
        assert_eq!(ac.cycles, 10);
        assert_eq!(ac.bits[8], 10, "ACR low byte is set from the first step's end");
        assert_eq!(ac.bits[0], 0);
    }

    /// The interrupt-entry step is published and accumulated too.
    #[test]
    fn interrupt_entry_charges_its_cycles_to_the_lamps() {
        use crate::console::PanelState;
        let (mut cpu, mut bus) = boot(&[0x0028]); // ENB 8, 1 cycle
        bus.load(8 * 4 * 2 + 2, &[0x00, 0x50]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        run_steps(&mut cpu, &mut bus, 1);
        bus.int_lines = 1 << 8;
        cpu.step(&mut bus); // entry sequence, 3 cycles
        assert_eq!(panel.snapshot_pc().cycles, 4);
    }

    /// Panel entry: PC and register bit toggles land where the indicator
    /// numbering says (0 = MSB), clears clear, display-only positions
    /// ignore entry.
    #[test]
    fn panel_entry_toggles_and_clears_registers() {
        use crate::console::{PanelCommand as Pc, PanelState, Selector};
        let (mut cpu, mut bus) = boot(&[0x0000]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());

        cpu.panel_command(&mut bus, &Pc::ClearPc);
        cpu.panel_command(&mut bus, &Pc::TogglePcBit(15));
        cpu.panel_command(&mut bus, &Pc::TogglePcBit(1));
        assert_eq!(cpu.pcr, 0x4001);
        assert_eq!(panel.program_counter(), cpu.pcr, "entry republishes the lamps");
        cpu.panel_command(&mut bus, &Pc::ClearPc);
        assert_eq!(cpu.pcr, 0);

        cpu.panel_command(&mut bus, &Pc::ToggleSelectedBit(Selector::Ac, 0));
        assert_eq!(cpu.acr, 0x8000);
        cpu.panel_command(&mut bus, &Pc::ToggleSelectedBit(Selector::Ix, 15));
        assert_eq!(cpu.ixr, 0x0001);
        cpu.panel_command(&mut bus, &Pc::ClearSelected(Selector::Ac, ));
        assert_eq!(cpu.acr, 0);

        // MS, IN, MA are display-only (5-2): entry there changes nothing
        let (acr, ixr, pcr) = (cpu.acr, cpu.ixr, cpu.pcr);
        for sel in [Selector::Ms, Selector::In, Selector::Ma] {
            cpu.panel_command(&mut bus, &Pc::ToggleSelectedBit(sel, 0));
            cpu.panel_command(&mut bus, &Pc::ClearSelected(sel));
        }
        assert_eq!((cpu.acr, cpu.ixr, cpu.pcr), (acr, ixr, pcr));
    }

    /// ENTER and DISPLAY walk memory through the PCR, 15-bit increment
    /// included (5-3).
    #[test]
    fn panel_enter_and_display_walk_memory() {
        use crate::console::{PanelCommand as Pc, PanelState, Selector};
        let (mut cpu, mut bus) = boot(&[0x1111, 0x2222]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());

        // DISPLAY from word 0x40: MBR loads it, PCR steps
        cpu.pcr = 0x40;
        cpu.panel_command(&mut bus, &Pc::Display);
        assert_eq!(panel.selected(Selector::Mb), 0x1111);
        assert_eq!(cpu.pcr, 0x41);

        // key a fresh word and ENTER it over the second one
        cpu.panel_command(&mut bus, &Pc::ClearSelected(Selector::Mb));
        cpu.panel_command(&mut bus, &Pc::ToggleSelectedBit(Selector::Mb, 0));
        cpu.panel_command(&mut bus, &Pc::ToggleSelectedBit(Selector::Mb, 15));
        cpu.panel_command(&mut bus, &Pc::Enter);
        assert_eq!(word(&mut bus, 0x41), 0x8001);
        assert_eq!(cpu.pcr, 0x42);

        // the increment wraps within 15 bits
        cpu.pcr = 0x7fff;
        cpu.panel_command(&mut bus, &Pc::Display);
        assert_eq!(cpu.pcr, 0);
    }

    /// RESET republishes the lamps. The panel's RESET button arrives while
    /// halted, when nothing else will publish until the next step -- so a
    /// reset that skips it looks like a button that does nothing.
    #[test]
    fn reset_republishes_the_panel() {
        use crate::console::PanelState;
        let (mut cpu, mut bus) = boot(&[0x0621]); // LLB 0x21
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(panel.program_counter(), 0x41);
        cpu.reset(&mut bus);
        assert_eq!(panel.program_counter(), 0, "the PC lamps must show the reset");
    }

    /// The whole 1968 ritual, headless: RESET, key a program in through
    /// MB and ENTER, key the PC back to its start, and run it.
    #[test]
    fn panel_keying_ritual_loads_a_runnable_program() {
        use crate::console::{PanelCommand as Pc, PanelState, Selector};
        let (mut cpu, mut bus) = boot(&[]);
        let panel = PanelState::new();
        cpu.attach_panel(panel.clone());
        cpu.reset(&mut bus);

        let key_word = |cpu: &mut Cpu703, bus: &mut TestBus, w: u16| {
            cpu.panel_command(bus, &Pc::ClearSelected(Selector::Mb));
            for i in 0..16 {
                if w & (0x8000 >> i) != 0 {
                    cpu.panel_command(bus, &Pc::ToggleSelectedBit(Selector::Mb, i));
                }
            }
            cpu.panel_command(bus, &Pc::Enter);
        };

        // PCR to word 0x40 (indicator 9), then LLB 0x55 and HLT
        cpu.panel_command(&mut bus, &Pc::TogglePcBit(9));
        assert_eq!(cpu.pcr, 0x40);
        key_word(&mut cpu, &mut bus, 0x0655);
        key_word(&mut cpu, &mut bus, 0x0000);

        // PC back to the start, and run
        cpu.panel_command(&mut bus, &Pc::ClearPc);
        cpu.panel_command(&mut bus, &Pc::TogglePcBit(9));
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.acr & 0x00ff, 0x0055, "the keyed LLB executed");
        assert_eq!(cpu.step(&mut bus), StepResult::Halted, "the keyed HLT executed");
    }

    // -- cycle counts ------------------------------------------------------

    /// Spot checks against appendix B's opcode index: one representative per
    /// distinct figure, plus the shift interpolation's pinned endpoints.
    #[test]
    fn cycle_counts_match_appendix_b() {
        for (insn, cycles) in [
            (0x1040u16, 1), // JMP: the only 1-cycle memory reference
            (0x8040, 2),    // LDW
            (0x3080, 3),    // STB: the only 3-cycle instruction
            (0x0010, 3),    // INR 0
            (0x0000, 1),    // HLT
            (0x0100, 1),    // CLR
            (0x02e9, 2),    // DIN 14,9
            (0x0401, 2),    // IXS 1
            (0x0610, 1),    // LLB
            (0x0800, 1),    // SAZ
            (0x0a40, 2),    // SRC 0: shift lower bound
            (0x0a4f, 5),    // SRC 15: shift upper bound
        ] {
            let (mut cpu, mut bus) = boot(&[insn, insn]);
            cpu.step(&mut bus);
            assert_eq!(cpu.last_step_cycles(), cycles, "insn {insn:04x}");
        }
    }

    /// The interrupt entry sequence is a step of its own and reports the
    /// 3-cycle estimate documented at its site.
    #[test]
    fn interrupt_entry_reports_three_cycles() {
        let (mut cpu, mut bus) = boot(&[0x0028]); // ENB 8
        bus.load(8 * 4 * 2 + 2, &[0x00, 0x50]); // level 8 linkage -> word 0x50
        run_steps(&mut cpu, &mut bus, 1);
        bus.int_lines = 1 << 8;
        cpu.step(&mut bus);
        assert_eq!(cpu.pcr, 0x50, "should have entered the level 8 routine");
        assert_eq!(cpu.last_step_cycles(), 3);
    }

    /// Machine time reaches the bus, because it is the only clock the devices
    /// on it have: a teletype at ten characters a second counts these cycles.
    /// The poll runs before the instruction is decoded, so what it reports is
    /// the previous step's count and the total lags by exactly one step --
    /// pinned here so it stays exactly one and does not quietly become none.
    #[test]
    fn the_bus_is_told_how_many_cycles_the_machine_spent() {
        // LDW (2 cycles), STB (3), INR 0 (3)
        let (mut cpu, mut bus) = boot(&[0x8040, 0x3080, 0x0010]);
        assert_eq!(bus.polled_cycles, 0);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.polled_cycles, 0, "the first poll has no previous step to report");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.polled_cycles, 2, "the LDW");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(bus.polled_cycles, 2 + 3, "and the STB");
    }
}
