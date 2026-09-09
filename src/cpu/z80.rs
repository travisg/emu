// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Zilog Z80 interpreter core.
//!
//! Port of `cpu/cpuz80.cpp`. Several of its behaviours are kept deliberately
//! rather than corrected -- see Faithfulness below.
//!
//! # Why this core is shaped differently from the 6800/6809 ones
//!
//! Those two are a 256-entry `OpDecode` table plus a handful of shared
//! operation handlers, because their opcode maps really are a cross product of
//! (operation x addressing mode x target register). The Z80's is not: the
//! DD/FD prefix changes what an opcode *means* per-opcode -- an operand slot
//! here, a register half there, nothing at all elsewhere (see
//! [`CpuZ80::exec_base`]). A 256-entry table would end up one bespoke entry
//! per opcode -- a switch statement wearing a table costume.
//!
//! What does factor cleanly is the *operation*, once the operand is in hand.
//! So the decode is the standard `x/y/z/p/q` bit split of the opcode, and the
//! semantics live in small op-kind enums shared by every encoding that reaches
//! them:
//!
//!   - [`AluOp`] -- one implementation of the eight 8-bit ALU operations,
//!     shared by the register forms (`0x80..=0xbf`) and the immediate forms
//!     (`0xc6`, `0xce`, ... `0xfe`). The C++ writes those flag expressions out
//!     twice; they are identical, checked expression by expression.
//!   - [`RotOp`] -- the eight CB-page rotates/shifts, which the C++ writes out
//!     as eight near-identical blocks.
//!   - [`CpuZ80::block_in`] / [`block_out`](CpuZ80::block_out) /
//!     [`block_cp`](CpuZ80::block_cp) -- the ED-page block operations,
//!     parameterized by direction and whether they repeat.
//!
//! Operand *fetch* deliberately stays at the call site rather than moving into
//! the shared handlers, because that is exactly where the prefix rules differ.
//!
//! # Faithfulness
//!
//! The base page and the CB page are complete (every opcode value decodes), and
//! so is every ED encoding a real Z80 defines -- aliases and undocumented forms
//! included. What is left undecoded on the ED page is genuinely undefined, and
//! ends the run.
//!
//! The undocumented side is modelled to the extent an instruction exerciser
//! can see it: the flag register's bits 3 and 5 (copies of the result's, or
//! of the operand's for `CP`, of the address's for `BIT n, (HL)`), the
//! halves of IX/IY as registers, every `DD CB` operation with its register
//! writeback, the block-I/O flags, the `MEMPTR` register (`wz`) that `BIT n,
//! (HL)` leaks, and the R refresh counter. A DD/FD prefix on an instruction
//! that has no use for it is what it is on silicon: four T-states and
//! nothing else.
//!
//! Interrupts are the silicon's: NMI on the line's rising edge into `0x66`
//! with IFF1 saved in IFF2, the maskable line accepted only with IFF1 set
//! and never on the instruction after `EI`, an acknowledge cycle the bus
//! answers (`Bus::interrupt_acknowledge`) -- `rst` on the bus in IM 0, a
//! vector through I in IM 2, ignored in IM 1 -- `HALT` sleeping in four-cycle
//! NOPs until either line wakes it, and `RETI` telling the bus
//! (`Bus::interrupt_return`) so the device under service can release the
//! daisy chain. A `HALT` with nothing to wake it sleeps forever, as on the
//! real part; the machines here can all wake one.

use super::{Addressing, Cpu, Register, StepResult};
use crate::bus::{Bus, Endian};
use std::io::Write;

// Flag bits. X and Y (bits 3 and 5) are the undocumented pair: unused by any
// condition, but written by every flag-setting instruction, so a guest that
// pushes AF or an exerciser that CRCs it sees them.
const F_C: u8 = 1 << 0;
const F_N: u8 = 1 << 1;
const F_PV: u8 = 1 << 2;
const F_X: u8 = 1 << 3;
const F_H: u8 = 1 << 4;
const F_Y: u8 = 1 << 5;
const F_Z: u8 = 1 << 6;
const F_S: u8 = 1 << 7;
const F_XY: u8 = F_X | F_Y;

/// The eight 8-bit ALU operations, indexed by the opcode's `y` field.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum AluOp {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
}

const ALU_OPS: [AluOp; 8] = [
    AluOp::Add,
    AluOp::Adc,
    AluOp::Sub,
    AluOp::Sbc,
    AluOp::And,
    AluOp::Xor,
    AluOp::Or,
    AluOp::Cp,
];

/// The eight CB-page rotates and shifts, indexed by the opcode's `y` field.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum RotOp {
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    /// undocumented
    Sll,
    Srl,
}

const ROT_OPS: [RotOp; 8] = [
    RotOp::Rlc,
    RotOp::Rrc,
    RotOp::Rl,
    RotOp::Rr,
    RotOp::Sla,
    RotOp::Sra,
    RotOp::Sll,
    RotOp::Srl,
];

/// T-states per unprefixed opcode, from the Zilog Z80 CPU User Manual.
/// Conditionals hold their not-taken value (the taken surcharge is added at
/// the four decision sites); `JP cc, nn` really is 10 either way. The r and
/// `(HL)` forms are distinct opcodes, so one flat table covers both; what it
/// cannot see is an active DD/FD, charged separately (+4 per prefix byte at
/// the fetch, +8 where a displacement is read -- +5 for `LD (IX+d), n`,
/// whose d and n fetches overlap). The four prefix values (0xcb/0xdd/0xed/
/// 0xfd) never reach `exec_base` and hold 0.
#[rustfmt::skip]
const MAIN_CYCLES: [u8; 256] = [
    //  x0  x1  x2  x3  x4  x5  x6  x7  x8  x9  xa  xb  xc  xd  xe  xf
         4, 10,  7,  6,  4,  4,  7,  4,  4, 11,  7,  6,  4,  4,  7,  4, // 0x
         8, 10,  7,  6,  4,  4,  7,  4, 12, 11,  7,  6,  4,  4,  7,  4, // 1x: djnz 8, jr 12
         7, 10, 16,  6,  4,  4,  7,  4,  7, 11, 16,  6,  4,  4,  7,  4, // 2x: jr cc 7, ld (nn),hl 16
         7, 10, 13,  6, 11, 11, 10,  4,  7, 11, 13,  6,  4,  4,  7,  4, // 3x: inc/dec (hl) 11
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 4x: ld r,r' 4, ld r,(hl) 7
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 5x
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 6x
         7,  7,  7,  7,  7,  7,  4,  7,  4,  4,  4,  4,  4,  4,  7,  4, // 7x: ld (hl),r 7, halt 4
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 8x: alu a,r 4, alu a,(hl) 7
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // 9x
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // ax
         4,  4,  4,  4,  4,  4,  7,  4,  4,  4,  4,  4,  4,  4,  7,  4, // bx
         5, 10, 10, 10, 10, 11,  7, 11,  5, 10, 10,  0, 10, 17,  7, 11, // cx: ret cc 5, call cc 10
         5, 10, 10, 11, 10, 11,  7, 11,  5,  4, 10, 11, 10,  0,  7, 11, // dx: out/in (n) 11
         5, 10, 10, 19, 10, 11,  7, 11,  5,  4, 10,  4, 10,  0,  7, 11, // ex: ex (sp),hl 19, jp (hl) 4
         5, 10, 10,  4, 10, 11,  7, 11,  5,  6, 10,  4, 10,  0,  7, 11, // fx: ld sp,hl 6
];

/// T-states per ED-page opcode, both fetches included. Block ops hold their
/// non-repeating value (+5 is added where the PC rewinds); the aliases
/// charge like the encodings they decode to. 0 marks the genuinely
/// undefined values, whose BadOpcode ends the run anyway.
#[rustfmt::skip]
const ED_CYCLES: [u8; 256] = [
    //  x0  x1  x2  x3  x4  x5  x6  x7  x8  x9  xa  xb  xc  xd  xe  xf
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 0x
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 1x
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 2x
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 3x
        12, 12, 15, 20,  8, 14,  8,  9, 12, 12, 15, 20,  8, 14,  8,  9, // 4x
        12, 12, 15, 20,  8, 14,  8,  9, 12, 12, 15, 20,  8, 14,  8,  9, // 5x
        12, 12, 15, 20,  8, 14,  8, 18, 12, 12, 15, 20,  8, 14,  8, 18, // 6x: rrd/rld 18
        12, 12, 15, 20,  8, 14,  8,  0, 12, 12, 15, 20,  8, 14,  8,  0, // 7x
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 8x
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // 9x
        16, 16, 16, 16,  0,  0,  0,  0, 16, 16, 16, 16,  0,  0,  0,  0, // ax: ldi/cpi/ini/outi
        16, 16, 16, 16,  0,  0,  0,  0, 16, 16, 16, 16,  0,  0,  0,  0, // bx: ...and the repeaters
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // cx
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // dx
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // ex
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, // fx
];

#[derive(Clone)]
pub struct CpuZ80 {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,

    a_alt: u8,
    f_alt: u8,
    b_alt: u8,
    c_alt: u8,
    d_alt: u8,
    e_alt: u8,
    h_alt: u8,
    l_alt: u8,

    pc: u16,
    sp: u16,
    ix: u16,
    iy: u16,

    im: u8,
    iff1: bool,
    iff2: bool,
    /// Set by `EI`, cleared by the next `step`'s interrupt poll, which it
    /// suppresses: a maskable interrupt is not accepted until the instruction
    /// after `EI` has run. That is what makes the `EI; RETI` a handler ends
    /// with atomic -- the RC2014 factory rom's is -- and consecutive `EI`s
    /// each renew it.
    ei_shadow: bool,
    /// Sleeping in `HALT`: every step is a four-cycle NOP with `pc` held at
    /// the `HALT` until an interrupt is accepted, which returns past it.
    halted: bool,
    /// The NMI line as last sampled, so the step that sees it rise takes
    /// the interrupt: the pin is edge-triggered.
    nmi_line: bool,
    i: u8,
    r: u8,

    /// Active DD/FD prefix, at most one of them: a later prefix byte
    /// replaces an earlier one. Struct state rather than locals because
    /// [`read_r`](CpuZ80::read_r) and [`write_r`](CpuZ80::write_r) consult it
    /// to remap `H`/`L` onto the halves of IX/IY.
    prefix_dd: bool,
    prefix_fd: bool,

    /// `MEMPTR` -- the internal address latch (WZ), which the silicon leaks
    /// through `BIT n, (HL)`: that instruction's X and Y come from its high
    /// byte. Every instruction that puts an address through the latch sets
    /// it, following the memptr_eng.txt description; nothing else reads it.
    wz: u16,

    /// T-states the last `step()` consumed, for `last_step_cycles`.
    cycles: u32,
}

impl Default for CpuZ80 {
    fn default() -> Self {
        // The C++ `mRegs = {}` leaves the in-class initializers alone, so `im`
        // starts at 1 while everything else zeroes. It only matters on the
        // interrupt path, which is dead code, but keep it faithful.
        CpuZ80 {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_alt: 0,
            f_alt: 0,
            b_alt: 0,
            c_alt: 0,
            d_alt: 0,
            e_alt: 0,
            h_alt: 0,
            l_alt: 0,
            pc: 0,
            sp: 0,
            ix: 0,
            iy: 0,
            im: 1,
            iff1: false,
            iff2: false,
            ei_shadow: false,
            halted: false,
            nmi_line: false,
            i: 0,
            r: 0,
            prefix_dd: false,
            prefix_fd: false,
            wz: 0,
            cycles: 0,
        }
    }
}

/// Even parity: true when the number of set bits is even, as the C++
/// `calc_parity` returns.
fn parity(val: u8) -> bool {
    val.count_ones().is_multiple_of(2)
}

impl CpuZ80 {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- register pairs ----

    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | self.f as u16
    }
    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }
    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }
    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }
    fn af_alt(&self) -> u16 {
        ((self.a_alt as u16) << 8) | self.f_alt as u16
    }

    fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = val as u8;
    }
    fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }
    fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }
    fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }
    fn set_af_alt(&mut self, val: u16) {
        self.a_alt = (val >> 8) as u8;
        self.f_alt = val as u8;
    }

    /// The `dd` register pair encoding: BC, DE, HL, SP.
    fn read_dd(&self, dd: u8) -> u16 {
        match dd {
            0b00 => self.bc(),
            0b01 => self.de(),
            0b10 => self.hl(),
            _ => self.sp,
        }
    }

    fn write_dd(&mut self, dd: u8, val: u16) {
        match dd {
            0b00 => self.set_bc(val),
            0b01 => self.set_de(val),
            0b10 => self.set_hl(val),
            _ => self.sp = val,
        }
    }

    /// The `qq` register pair encoding: BC, DE, HL, AF.
    fn read_qq(&self, qq: u8) -> u16 {
        match qq {
            0b00 => self.bc(),
            0b01 => self.de(),
            0b10 => self.hl(),
            _ => self.af(),
        }
    }

    fn write_qq(&mut self, qq: u8, val: u16) {
        match qq {
            0b00 => self.set_bc(val),
            0b01 => self.set_de(val),
            0b10 => self.set_hl(val),
            _ => self.set_af(val),
        }
    }

    /// The HL slot of a register-pair encoding: IX or IY under a prefix.
    fn hl_or_index(&self) -> u16 {
        if self.prefix_dd {
            self.ix
        } else if self.prefix_fd {
            self.iy
        } else {
            self.hl()
        }
    }

    fn write_hl_or_index(&mut self, val: u16) {
        if self.prefix_dd {
            self.ix = val;
        } else if self.prefix_fd {
            self.iy = val;
        } else {
            self.set_hl(val);
        }
    }

    // ---- 8-bit register file ----

    /// The `r` encoding. Under a DD/FD prefix, `H` and `L` are remapped onto
    /// the halves of IX/IY -- the undocumented `IXh`/`IXl` registers, which
    /// is what makes the prefix flags struct state rather than locals.
    fn read_r(&self, r: u8) -> u8 {
        match r {
            0b100 if self.prefix_dd => (self.ix >> 8) as u8,
            0b101 if self.prefix_dd => self.ix as u8,
            0b100 if self.prefix_fd => (self.iy >> 8) as u8,
            0b101 if self.prefix_fd => self.iy as u8,
            _ => self.read_r_plain(r),
        }
    }

    /// The `r` encoding with H and L meaning H and L whatever the prefix.
    /// An instruction whose prefix is spent on an `(IX+d)` operand -- `LD r,
    /// (IX+d)`, `LD (IX+d), r`, the `DD CB` writebacks -- names the plain
    /// register with the other operand: `DD 66 d` is `LD H, (IX+d)`.
    fn read_r_plain(&self, r: u8) -> u8 {
        match r {
            0b000 => self.b,
            0b001 => self.c,
            0b010 => self.d,
            0b011 => self.e,
            0b100 => self.h,
            0b101 => self.l,
            0b111 => self.a,
            // 0b110 is the (HL) hole; every caller either guards it or routes
            // through read_r_or_hl. The C++ asserts here.
            _ => unreachable!("read_r called with the (HL) encoding"),
        }
    }

    fn write_r(&mut self, r: u8, val: u8) {
        match r {
            0b100 if self.prefix_dd => self.ix = (self.ix & 0x00ff) | ((val as u16) << 8),
            0b101 if self.prefix_dd => self.ix = (self.ix & 0xff00) | val as u16,
            0b100 if self.prefix_fd => self.iy = (self.iy & 0x00ff) | ((val as u16) << 8),
            0b101 if self.prefix_fd => self.iy = (self.iy & 0xff00) | val as u16,
            _ => self.write_r_plain(r, val),
        }
    }

    fn write_r_plain(&mut self, r: u8, val: u8) {
        match r {
            0b000 => self.b = val,
            0b001 => self.c = val,
            0b010 => self.d = val,
            0b011 => self.e = val,
            0b100 => self.h = val,
            0b101 => self.l = val,
            0b111 => self.a = val,
            _ => unreachable!("write_r called with the (HL) encoding"),
        }
    }

    /// For encodings where the missing register slot means `(HL)`.
    fn read_r_or_hl(&mut self, bus: &mut dyn Bus, r: u8) -> u8 {
        if r == 0b110 {
            let addr = self.hl();
            self.mem_read(bus, addr)
        } else {
            self.read_r(r)
        }
    }

    fn write_r_or_hl(&mut self, bus: &mut dyn Bus, r: u8, val: u8) {
        if r == 0b110 {
            let addr = self.hl();
            self.mem_write(bus, addr, val);
        } else {
            self.write_r(r, val);
        }
    }

    // ---- bus and stack ----

    /// Addresses wrap at 64K. The C++ lets `temp16 + 1` promote to `int` and
    /// hands 0x10000 to the decode, which masks it for the decode but not for
    /// the offset -- an out-of-bounds read of the rom bank. That is UB, not
    /// behaviour worth reproducing; wrapping is the sane reading.
    fn mem_read(&self, bus: &mut dyn Bus, addr: u16) -> u8 {
        bus.read8(addr as u32)
    }

    fn mem_write(&self, bus: &mut dyn Bus, addr: u16, val: u8) {
        bus.write8(addr as u32, val);
    }

    fn read_n(&mut self, bus: &mut dyn Bus) -> u8 {
        let val = bus.read8(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn read_nn(&mut self, bus: &mut dyn Bus) -> u16 {
        let val = bus.read16(self.pc as u32, Endian::Little);
        self.pc = self.pc.wrapping_add(2);
        val
    }

    /// The signed displacement of an indexed operand.
    fn read_d(&mut self, bus: &mut dyn Bus) -> i8 {
        self.read_n(bus) as i8
    }

    /// Whether a DD/FD prefix is active.
    fn indexed(&self) -> bool {
        self.prefix_dd || self.prefix_fd
    }

    /// `(IX+d)` / `(IY+d)`, picked by whichever prefix is active: reads the
    /// displacement, and puts the address through MEMPTR.
    fn indexed_addr(&mut self, bus: &mut dyn Bus) -> u16 {
        let base = if self.prefix_dd { self.ix } else { self.iy };
        let d = self.read_d(bus);
        let addr = base.wrapping_add(d as u16);
        self.wz = addr;
        addr
    }

    /// One M1 cycle's worth of refresh: R's low seven bits count opcode
    /// fetches, prefix bytes included; bit 7 is only ever written by `LD R, A`.
    /// An interrupt accepted while asleep in `HALT` returns past it.
    fn wake(&mut self) {
        if self.halted {
            self.halted = false;
            self.pc = self.pc.wrapping_add(1);
        }
    }

    fn bump_r(&mut self) {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7f);
    }

    fn push8(&mut self, bus: &mut dyn Bus, val: u8) {
        self.sp = self.sp.wrapping_sub(1);
        self.mem_write(bus, self.sp, val);
    }

    fn push16(&mut self, bus: &mut dyn Bus, val: u16) {
        self.push8(bus, (val >> 8) as u8);
        self.push8(bus, val as u8);
    }

    fn pop8(&mut self, bus: &mut dyn Bus) -> u8 {
        let val = self.mem_read(bus, self.sp);
        self.sp = self.sp.wrapping_add(1);
        val
    }

    fn pop16(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = self.pop8(bus) as u16;
        let hi = self.pop8(bus) as u16;
        (hi << 8) | lo
    }

    // ---- flags ----

    fn set_flag(&mut self, bit: u8, on: bool) {
        if on {
            self.f |= bit;
        } else {
            self.f &= !bit;
        }
    }

    fn flag(&self, bit: u8) -> bool {
        (self.f & bit) != 0
    }

    fn carry(&self) -> u8 {
        u8::from(self.flag(F_C))
    }

    fn set_sz(&mut self, val: u8) {
        self.set_flag(F_S, (val & 0x80) != 0);
        self.set_flag(F_Z, val == 0);
    }

    /// The undocumented X and Y flags, copied from bits 3 and 5 of `val` --
    /// the result for most instructions; the operand for `CP`, the high byte
    /// of an address for `BIT n, (HL)` and `BIT n, (IX+d)`, and a derived
    /// byte for the block operations.
    fn set_xy(&mut self, val: u8) {
        self.f = (self.f & !F_XY) | (val & F_XY);
    }

    /// The logical-operation flag set: S, Z and parity from the result, H/N/C
    /// cleared. `AND` re-sets H afterwards.
    fn set_logic_flags(&mut self, val: u8) {
        self.set_sz(val);
        self.set_xy(val);
        self.set_flag(F_PV, parity(val));
        self.set_flag(F_H, false);
        self.set_flag(F_N, false);
        self.set_flag(F_C, false);
    }

    /// The eight branch conditions, in encoding order.
    fn test_cond(&self, cond: u8) -> bool {
        match cond {
            0 => !self.flag(F_Z),
            1 => self.flag(F_Z),
            2 => !self.flag(F_C),
            3 => self.flag(F_C),
            4 => !self.flag(F_PV),
            5 => self.flag(F_PV),
            6 => !self.flag(F_S),
            _ => self.flag(F_S),
        }
    }

    // ---- shared operations ----

    /// One 8-bit ALU operation against the accumulator.
    ///
    /// Shared by the register/`(HL)`/indexed forms and the immediate forms; the
    /// C++ spells those out separately but every flag expression matches.
    fn alu(&mut self, op: AluOp, val: u8) {
        let a = self.a;
        match op {
            AluOp::Add => {
                let res = a.wrapping_add(val);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (a & 0xf) + (val & 0xf) > 0xf);
                self.set_flag(F_PV, ((a ^ res) & (val ^ res) & 0x80) != 0);
                self.set_flag(F_N, false);
                self.set_flag(F_C, a as u16 + val as u16 > 0xff);
                self.set_xy(res);
                self.a = res;
            }
            AluOp::Adc => {
                let c = self.carry();
                let res = a.wrapping_add(val).wrapping_add(c);
                self.set_flag(F_C, a as u16 + val as u16 + c as u16 > 0xff);
                self.set_flag(F_N, false);
                self.set_flag(F_PV, ((a ^ res) & (val ^ res) & 0x80) != 0);
                self.set_flag(F_H, (a ^ res ^ val) & 0x10 != 0);
                self.set_sz(res);
                self.set_xy(res);
                self.a = res;
            }
            AluOp::Sub | AluOp::Cp => {
                let res = a.wrapping_sub(val);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (a & 0xf) < (val & 0xf));
                self.set_flag(F_PV, ((a ^ val) & (a ^ res) & 0x80) != 0);
                self.set_flag(F_N, true);
                self.set_flag(F_C, a < val);
                // CP is SUB without the writeback -- and its X/Y come from
                // the operand, the one place they are not the result's
                if op == AluOp::Sub {
                    self.set_xy(res);
                    self.a = res;
                } else {
                    self.set_xy(val);
                }
            }
            AluOp::Sbc => {
                let c = self.carry();
                let res = a.wrapping_sub(val).wrapping_sub(c);
                self.set_flag(F_C, (a as i32) < val as i32 + c as i32);
                self.set_flag(F_N, true);
                self.set_flag(F_PV, ((a ^ val) & (a ^ res) & 0x80) != 0);
                self.set_flag(F_H, (a ^ res ^ val) & 0x10 != 0);
                self.set_sz(res);
                self.set_xy(res);
                self.a = res;
            }
            AluOp::And => {
                self.a = a & val;
                let res = self.a;
                self.set_logic_flags(res);
                // AND is the one logical op that leaves H set
                self.set_flag(F_H, true);
            }
            AluOp::Xor => {
                self.a = a ^ val;
                let res = self.a;
                self.set_logic_flags(res);
            }
            AluOp::Or => {
                self.a = a | val;
                let res = self.a;
                self.set_logic_flags(res);
            }
        }
    }

    /// One CB-page rotate/shift. Returns the result; sets every flag.
    fn rot(&mut self, op: RotOp, val: u8) -> u8 {
        let (res, carry) = match op {
            RotOp::Rlc => (val.rotate_left(1), val & 0x80 != 0),
            RotOp::Rrc => (val.rotate_right(1), val & 0x01 != 0),
            RotOp::Rl => ((val << 1) | self.carry(), val & 0x80 != 0),
            RotOp::Rr => ((val >> 1) | (self.carry() << 7), val & 0x01 != 0),
            RotOp::Sla => (val << 1, val & 0x80 != 0),
            RotOp::Sra => ((val >> 1) | (val & 0x80), val & 0x01 != 0),
            RotOp::Sll => ((val << 1) | 0x01, val & 0x80 != 0),
            RotOp::Srl => (val >> 1, val & 0x01 != 0),
        };

        self.set_flag(F_C, carry);
        self.set_flag(F_H, false);
        self.set_flag(F_N, false);
        self.set_flag(F_PV, parity(res));
        self.set_sz(res);
        self.set_xy(res);
        res
    }

    /// The operand of an 8-bit ALU instruction.
    ///
    /// Fetch stays out of [`alu`](CpuZ80::alu) because this is where the prefix
    /// rules live: the `(HL)` slot becomes `(IX+d)`, and H and L become the
    /// index register's halves (`DD 84` is `ADD A, IXh`).
    fn alu_operand(&mut self, bus: &mut dyn Bus, r: u8) -> u8 {
        if r == 0b110 && self.indexed() {
            let addr = self.indexed_addr(bus);
            self.cycles += 8; // the displacement fetch and index add
            self.mem_read(bus, addr)
        } else {
            self.read_r_or_hl(bus, r)
        }
    }

    /// `INC r` and `DEC r`, which differ only in the direction and three flags.
    fn inc_dec_r(&mut self, bus: &mut dyn Bus, r: u8, inc: bool) {
        let bump = |v: u8| {
            if inc {
                v.wrapping_add(1)
            } else {
                v.wrapping_sub(1)
            }
        };

        let (old, new) = if r == 0b110 && self.indexed() {
            let addr = self.indexed_addr(bus);
            let old = self.mem_read(bus, addr);
            self.mem_write(bus, addr, bump(old));
            self.cycles += 8; // the displacement fetch and index add
            (old, bump(old))
        } else {
            let old = self.read_r_or_hl(bus, r);
            self.write_r_or_hl(bus, r, bump(old));
            (old, bump(old))
        };

        self.set_flag(F_PV, old == if inc { 0x7f } else { 0x80 });
        self.set_sz(new);
        self.set_xy(new);
        self.set_flag(F_N, !inc);
        self.set_flag(
            F_H,
            if inc {
                (old & 0x0f) == 0x0f
            } else {
                (old & 0x0f) == 0
            },
        );
    }

    /// The unprefixed opcode page.
    ///
    /// Decoded by the standard bit split: `x` = bits 7-6, `y` = bits 5-3,
    /// `z` = bits 2-0, with `p` = `y >> 1` and `q` = `y & 1` for the encodings
    /// that use `y` as a register-pair plus a direction bit.
    ///
    /// This page is complete -- every one of the 256 values decodes to
    /// something, so there is no `BadOpcode` path here. `0xcb`, `0xdd`, `0xed`
    /// and `0xfd` never arrive: `step` peels them off first.
    ///
    /// Under a DD/FD prefix the `(HL)` operand slot means `(IX+d)`, H and L
    /// mean the index register's halves, and the HL register pair means the
    /// index register -- and an instruction with none of those is simply
    /// itself, four T-states dearer.
    fn exec_base(&mut self, bus: &mut dyn Bus, op: u8) -> StepResult {
        let x = op >> 6;
        let y = (op >> 3) & 0b111;
        let z = op & 0b111;
        let p = y >> 1;
        let q = y & 1;

        self.cycles += MAIN_CYCLES[op as usize] as u32;

        match (x, z) {
            (0, 0) => match y {
                0 => {} // NOP
                1 => {
                    // EX AF, AF'
                    let cur = self.af();
                    let alt = self.af_alt();
                    self.set_af(alt);
                    self.set_af_alt(cur);
                }
                2 => {
                    // DJNZ e
                    let rel = self.read_d(bus);
                    self.b = self.b.wrapping_sub(1);
                    if self.b != 0 {
                        self.pc = self.pc.wrapping_add(rel as u16);
                        self.wz = self.pc;
                        self.cycles += 5; // 13 taken, 8 not
                    }
                }
                3 => {
                    // JR e
                    let rel = self.read_d(bus);
                    self.pc = self.pc.wrapping_add(rel as u16);
                    self.wz = self.pc;
                }
                // JR cc, e -- NZ, Z, NC, C, which are conditions 0..3
                _ => {
                    let rel = self.read_d(bus);
                    if self.test_cond(y - 4) {
                        self.pc = self.pc.wrapping_add(rel as u16);
                        self.wz = self.pc;
                        self.cycles += 5; // 12 taken, 7 not
                    }
                }
            },

            (0, 1) => {
                if q == 0 {
                    // LD dd, nn -- the HL slot is the index register under a prefix
                    let nn = self.read_nn(bus);
                    if p == 0b10 {
                        self.write_hl_or_index(nn);
                    } else {
                        self.write_dd(p, nn);
                    }
                } else {
                    // ADD HL, ss. Only C, N, H and the X/Y copies are
                    // touched; S, Z and PV survive. Under a prefix both the
                    // HL operand and the HL slot of ss are the index
                    // register (ADD IX, IX), never a mix.
                    let base = self.hl_or_index();
                    let ss = if p == 0b10 { base } else { self.read_dd(p) };
                    let res = base as u32 + ss as u32;
                    self.wz = base.wrapping_add(1);
                    self.set_flag(F_C, res > 0xffff);
                    self.set_flag(F_N, false);
                    self.set_flag(F_H, (base & 0xfff) + (ss & 0xfff) > 0xfff);
                    self.set_xy((res >> 8) as u8);
                    self.write_hl_or_index(res as u16);
                }
            }

            // The accumulator and register-pair loads through an address.
            // MEMPTR takes the address plus one, except that a store of A
            // puts A in its high byte (the silicon reuses the latch for the
            // data), with only the low byte incremented.
            (0, 2) => match y {
                0 | 2 => {
                    // LD (BC), A / LD (DE), A
                    let addr = if y == 0 { self.bc() } else { self.de() };
                    let a = self.a;
                    self.mem_write(bus, addr, a);
                    self.wz = ((a as u16) << 8) | (addr.wrapping_add(1) & 0xff);
                }
                1 | 3 => {
                    // LD A, (BC) / LD A, (DE)
                    let addr = if y == 1 { self.bc() } else { self.de() };
                    self.a = self.mem_read(bus, addr);
                    self.wz = addr.wrapping_add(1);
                }
                4 => {
                    // LD (nn), HL
                    let addr = self.read_nn(bus);
                    let val = self.hl_or_index();
                    bus.write16(addr as u32, val, Endian::Little);
                    self.wz = addr.wrapping_add(1);
                }
                5 => {
                    // LD HL, (nn)
                    let addr = self.read_nn(bus);
                    let val = bus.read16(addr as u32, Endian::Little);
                    self.write_hl_or_index(val);
                    self.wz = addr.wrapping_add(1);
                }
                6 => {
                    // LD (nn), A
                    let addr = self.read_nn(bus);
                    let a = self.a;
                    self.mem_write(bus, addr, a);
                    self.wz = ((a as u16) << 8) | (addr.wrapping_add(1) & 0xff);
                }
                _ => {
                    // LD A, (nn)
                    let addr = self.read_nn(bus);
                    self.a = self.mem_read(bus, addr);
                    self.wz = addr.wrapping_add(1);
                }
            },

            (0, 3) => {
                // INC ss / DEC ss -- no flags
                let bump = |v: u16| {
                    if q == 0 {
                        v.wrapping_add(1)
                    } else {
                        v.wrapping_sub(1)
                    }
                };
                if p == 0b10 {
                    let val = self.hl_or_index();
                    self.write_hl_or_index(bump(val));
                } else {
                    let val = self.read_dd(p);
                    self.write_dd(p, bump(val));
                }
            }

            (0, 4) => self.inc_dec_r(bus, y, true), // INC r
            (0, 5) => self.inc_dec_r(bus, y, false), // DEC r

            (0, 6) => {
                if y == 0b110 && self.indexed() {
                    // LD (IX+d), n reads two immediates: the displacement
                    // first, then the value.
                    let addr = self.indexed_addr(bus);
                    let val = self.read_n(bus);
                    self.mem_write(bus, addr, val);
                    // +5, not the usual +8: the d and n fetches overlap,
                    // so the whole thing is 19 = 4 prefix + 10 base + 5
                    self.cycles += 5;
                } else {
                    // LD r, n -- and LD IXh, n under a prefix
                    let val = self.read_n(bus);
                    self.write_r_or_hl(bus, y, val);
                }
            }

            (0, 7) => match y {
                // The accumulator rotates touch only C, H, N and the X/Y
                // copies of the result; S, Z and PV survive.
                0..=3 => {
                    let a = self.a;
                    let (res, carry) = match y {
                        0 => (a.rotate_left(1), a & 0x80 != 0),  // RLCA
                        1 => (a.rotate_right(1), a & 0x01 != 0), // RRCA
                        2 => ((a << 1) | self.carry(), a & 0x80 != 0), // RLA
                        _ => ((a >> 1) | (self.carry() << 7), a & 0x01 != 0), // RRA
                    };
                    self.a = res;
                    self.set_flag(F_C, carry);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                    self.set_xy(res);
                }
                4 => {
                    // DAA. Every flag decision reads the *old* accumulator
                    // while the accumulator itself is being mutated, so `a` is
                    // captured up front and the order here matters.
                    let a = self.a;
                    let mut correction = 0u8;
                    let mut carry = self.flag(F_C);
                    let h_carry = self.flag(F_H);

                    if h_carry || (a & 0x0f) > 9 {
                        correction |= 0x06;
                    }
                    if carry || a > 0x99 {
                        correction |= 0x60;
                        carry = true;
                    }

                    let sub = self.flag(F_N);
                    self.a = if sub {
                        a.wrapping_sub(correction)
                    } else {
                        a.wrapping_add(correction)
                    };

                    self.set_flag(F_C, carry);
                    self.set_flag(
                        F_H,
                        if sub {
                            h_carry && (a & 0x0f) < 6
                        } else {
                            (a & 0x0f) > 9
                        },
                    );
                    let res = self.a;
                    self.set_sz(res);
                    self.set_xy(res);
                    self.set_flag(F_PV, parity(res));
                }
                5 => {
                    // CPL
                    self.a = !self.a;
                    let a = self.a;
                    self.set_flag(F_H, true);
                    self.set_flag(F_N, true);
                    self.set_xy(a);
                }
                // SCF and CCF take X and Y from A. (NMOS silicon ORs in
                // the previous flags when the instruction before did not
                // set any; that is beyond what an exerciser checks and is
                // not modelled.)
                6 => {
                    // SCF
                    let a = self.a;
                    self.set_flag(F_C, true);
                    self.set_flag(F_H, false);
                    self.set_flag(F_N, false);
                    self.set_xy(a);
                }
                _ => {
                    // CCF -- H takes the *old* carry, then C inverts
                    let (a, c) = (self.a, self.flag(F_C));
                    self.set_flag(F_H, c);
                    self.set_flag(F_C, !c);
                    self.set_flag(F_N, false);
                    self.set_xy(a);
                }
            },

            // LD r, r' -- plus HALT, which steals the r == r' == (HL) encoding
            (1, _) => {
                let (dst, src) = (y, z);
                if dst == 0b110 && src == 0b110 {
                    // HALT: sleep, with pc back on the instruction. The
                    // silicon keeps refetching it, and an interrupt accepted
                    // while asleep pushes the address after it.
                    self.halted = true;
                    self.pc = self.pc.wrapping_sub(1);
                } else if src == 0b110 && self.indexed() {
                    // LD r, (IX+d). The prefix is spent on the operand: the
                    // register is the plain one (DD 66 d is LD H, (IX+d)).
                    let addr = self.indexed_addr(bus);
                    let val = self.mem_read(bus, addr);
                    self.write_r_plain(dst, val);
                    self.cycles += 8; // the displacement fetch and index add
                } else if dst == 0b110 && self.indexed() {
                    // LD (IX+d), r -- likewise the plain register
                    let addr = self.indexed_addr(bus);
                    let val = self.read_r_plain(src);
                    self.mem_write(bus, addr, val);
                    self.cycles += 8;
                } else {
                    // register to register, with H and L remapped under a
                    // prefix on both sides (DD 65 is LD IXh, IXl)
                    let val = self.read_r_or_hl(bus, src);
                    self.write_r_or_hl(bus, dst, val);
                }
            }

            // ALU A, r / (HL) / (IX+d)
            (2, _) => {
                let val = self.alu_operand(bus, z);
                self.alu(ALU_OPS[y as usize], val);
            }

            (3, 0) => {
                // RET cc
                if self.test_cond(y) {
                    self.pc = self.pop16(bus);
                    self.wz = self.pc;
                    self.cycles += 6; // 11 taken, 5 not
                }
            }

            (3, 1) => {
                if q == 0 {
                    // POP qq
                    let val = self.pop16(bus);
                    if p == 0b10 {
                        self.write_hl_or_index(val);
                    } else {
                        self.write_qq(p, val);
                    }
                } else {
                    match p {
                        0 => {
                            // RET
                            self.pc = self.pop16(bus);
                            self.wz = self.pc;
                        }
                        1 => {
                            // EXX -- BC/DE/HL only, AF has its own instruction
                            std::mem::swap(&mut self.b, &mut self.b_alt);
                            std::mem::swap(&mut self.c, &mut self.c_alt);
                            std::mem::swap(&mut self.d, &mut self.d_alt);
                            std::mem::swap(&mut self.e, &mut self.e_alt);
                            std::mem::swap(&mut self.h, &mut self.h_alt);
                            std::mem::swap(&mut self.l, &mut self.l_alt);
                        }
                        2 => self.pc = self.hl_or_index(), // JP (HL) -- MEMPTR untouched
                        _ => self.sp = self.hl_or_index(), // LD SP, HL
                    }
                }
            }

            (3, 2) => {
                // JP cc, nn -- the target is always read, taken or not, and
                // MEMPTR takes it either way
                let addr = self.read_nn(bus);
                self.wz = addr;
                if self.test_cond(y) {
                    self.pc = addr;
                }
            }

            (3, 3) => match y {
                0 => {
                    // JP nn
                    self.pc = self.read_nn(bus);
                    self.wz = self.pc;
                }
                1 => unreachable!("the cb prefix is peeled off in step"),
                2 => {
                    // OUT (n), A -- the port is A:n, and MEMPTR takes A:n+1
                    let port = self.read_n(bus);
                    let a = self.a;
                    bus.io_write8(port as u16, a);
                    self.wz = ((a as u16) << 8) | (port.wrapping_add(1) as u16);
                }
                3 => {
                    // IN A, (n) -- MEMPTR is A:n + 1, with the A before the read
                    let port = self.read_n(bus);
                    self.wz = (((self.a as u16) << 8) | port as u16).wrapping_add(1);
                    self.a = bus.io_read8(port as u16);
                }
                4 => {
                    // EX (SP), HL -- MEMPTR takes the value HL receives
                    let val = self.pop16(bus);
                    let old = self.hl_or_index();
                    self.push16(bus, old);
                    self.write_hl_or_index(val);
                    self.wz = val;
                }
                5 => {
                    // EX DE, HL -- note this ignores the prefix entirely
                    let (de, hl) = (self.de(), self.hl());
                    self.set_de(hl);
                    self.set_hl(de);
                }
                6 => {
                    // DI
                    self.iff1 = false;
                    self.iff2 = false;
                }
                7 => {
                    // EI: enabled, but not sampled until after the next
                    // instruction (`ei_shadow`)
                    self.iff1 = true;
                    self.iff2 = true;
                    self.ei_shadow = true;
                }
                _ => unreachable!("y is three bits"),
            },

            (3, 4) => {
                // CALL cc, nn -- MEMPTR takes the target, taken or not
                let addr = self.read_nn(bus);
                self.wz = addr;
                if self.test_cond(y) {
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = addr;
                    self.cycles += 7; // 17 taken, 10 not
                }
            }

            (3, 5) => {
                if q == 0 {
                    // PUSH qq
                    let val = if p == 0b10 { self.hl_or_index() } else { self.read_qq(p) };
                    self.push16(bus, val);
                } else {
                    // CALL nn. The other three q == 1 encodings in this column
                    // are the DD, ED and FD prefixes, already peeled off.
                    let addr = self.read_nn(bus);
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = addr;
                    self.wz = addr;
                }
            }

            (3, 6) => {
                // ALU A, n -- same eight operations as the register forms
                let val = self.read_n(bus);
                self.alu(ALU_OPS[y as usize], val);
            }

            (3, 7) => {
                // RST p
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = (y as u16) * 8;
                self.wz = self.pc;
            }

            _ => unreachable!("x is two bits and z is three"),
        }

        StepResult::Ok
    }

    /// `INI` / `INIR` / `IND` / `INDR`. MEMPTR takes BC, with the B from
    /// before the decrement, plus or minus one.
    fn block_in(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        let bc = self.bc();
        self.wz = if inc { bc.wrapping_add(1) } else { bc.wrapping_sub(1) };
        let val = bus.io_read8(self.c as u16);
        let addr = self.hl();
        self.mem_write(bus, addr, val);
        self.set_hl(if inc {
            addr.wrapping_add(1)
        } else {
            addr.wrapping_sub(1)
        });
        self.b = self.b.wrapping_sub(1);
        let c = if inc { self.c.wrapping_add(1) } else { self.c.wrapping_sub(1) };
        self.finish_block_io(val, val as u16 + c as u16, repeat);
    }

    /// `OUTI` / `OTIR` / `OUTD` / `OTDR`. Note B is decremented *before* the
    /// memory read here, unlike the input forms, and MEMPTR takes the
    /// decremented BC plus or minus one.
    fn block_out(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        self.b = self.b.wrapping_sub(1);
        let bc = self.bc();
        self.wz = if inc { bc.wrapping_add(1) } else { bc.wrapping_sub(1) };
        let addr = self.hl();
        let val = self.mem_read(bus, addr);
        bus.io_write8(self.c as u16, val);
        self.set_hl(if inc {
            addr.wrapping_add(1)
        } else {
            addr.wrapping_sub(1)
        });
        let l = self.l;
        self.finish_block_io(val, val as u16 + l as u16, repeat);
    }

    /// The shared tail of the IN/OUT block ops: the flags, and the rewind.
    ///
    /// Only Z (B reached zero) and N (set) are documented. The rest follow
    /// The Undocumented Z80 Documented, 4.2: S, Z, X and Y from the
    /// decremented B; N from bit 7 of the byte transferred; H and C both
    /// from the carry out of `k`, which is that byte plus C+1 (INI), C-1
    /// (IND) or the new L (OUTx); PV from the parity of `k`'s low three bits
    /// xor B. A repeating form recomputes these every iteration, so its
    /// final flags are the last iteration's.
    fn finish_block_io(&mut self, val: u8, k: u16, repeat: bool) {
        let b = self.b;
        self.set_sz(b);
        self.set_xy(b);
        self.set_flag(F_N, val & 0x80 != 0);
        self.set_flag(F_H, k > 0xff);
        self.set_flag(F_C, k > 0xff);
        self.set_flag(F_PV, parity(((k & 7) as u8) ^ b));
        if repeat && b != 0 {
            self.pc = self.pc.wrapping_sub(2); // repeat the instruction
            self.cycles += 5; // 21 when repeating, 16 on the last
        }
    }

    /// The X and Y of the block moves and compares: bit 1 and bit 3 of a
    /// byte the silicon happens to have on its bus -- Y from bit 1, X from
    /// bit 3, not the usual 5 and 3.
    fn set_block_xy(&mut self, n: u8) {
        self.set_flag(F_Y, n & 0x02 != 0);
        self.set_flag(F_X, n & 0x08 != 0);
    }

    /// `LDI` / `LDIR` / `LDD` / `LDDR`.
    ///
    /// PV is "BC is still non-zero after the transfer", so it stays *set* while
    /// a repeating form has work left and clears on the final byte -- which is
    /// how a guest spots the last iteration. X and Y come from the byte moved
    /// plus A. A repeating form that rewinds leaves MEMPTR at the address of
    /// its own second byte, as the silicon does.
    fn block_move(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        let src = self.hl();
        let val = self.mem_read(bus, src);
        let dst = self.de();
        self.mem_write(bus, dst, val);

        let step = if inc { 1u16 } else { 0xffffu16 };
        self.set_hl(src.wrapping_add(step));
        self.set_de(dst.wrapping_add(step));
        let bc = self.bc().wrapping_sub(1);
        self.set_bc(bc);

        if repeat && bc != 0 {
            self.pc = self.pc.wrapping_sub(2); // repeat the instruction
            self.wz = self.pc.wrapping_add(1);
            self.cycles += 5; // 21 when repeating, 16 on the last
        }

        self.set_flag(F_H, false);
        self.set_flag(F_PV, bc != 0);
        self.set_flag(F_N, false);
        self.set_block_xy(val.wrapping_add(self.a));
    }

    /// `CPI` / `CPIR` / `CPD` / `CPDR`. The comparison never writes A, and C is
    /// left alone. X and Y come from the difference less the half borrow.
    /// MEMPTR steps with HL, except that a rewinding repeat leaves it at the
    /// address of the instruction's own second byte.
    fn block_cp(&mut self, bus: &mut dyn Bus, inc: bool, repeat: bool) {
        let addr = self.hl();
        let val = self.mem_read(bus, addr);
        let a = self.a;
        let res = a.wrapping_sub(val);
        let bc = self.bc().wrapping_sub(1);
        self.set_bc(bc);
        self.set_hl(if inc {
            addr.wrapping_add(1)
        } else {
            addr.wrapping_sub(1)
        });
        self.wz = if inc { self.wz.wrapping_add(1) } else { self.wz.wrapping_sub(1) };

        let half = (a & 0x0f) < (val & 0x0f);
        self.set_flag(F_S, (res & 0x80) != 0);
        self.set_flag(F_Z, res == 0);
        self.set_flag(F_H, half);
        self.set_flag(F_PV, bc != 0);
        self.set_flag(F_N, true);
        self.set_block_xy(res.wrapping_sub(half as u8));

        if repeat && bc != 0 && res != 0 {
            self.pc = self.pc.wrapping_sub(2);
            self.wz = self.pc.wrapping_add(1);
            self.cycles += 5; // 21 when repeating, 16 on the last
        }
    }

    /// `RRD` and `RLD`, which differ only in which nibble goes where.
    fn rotate_decimal(&mut self, bus: &mut dyn Bus, right: bool) {
        let addr = self.hl();
        self.wz = addr.wrapping_add(1);
        let mem = self.mem_read(bus, addr);
        let a_low = self.a & 0x0f;

        let (new_mem, new_a_low) = if right {
            ((a_low << 4) | (mem >> 4), mem & 0x0f)
        } else {
            (((mem & 0x0f) << 4) | a_low, mem >> 4)
        };
        self.mem_write(bus, addr, new_mem);
        self.a = (self.a & 0xf0) | new_a_low;

        let a = self.a;
        self.set_sz(a);
        self.set_xy(a);
        self.set_flag(F_H, false);
        self.set_flag(F_PV, parity(a));
        self.set_flag(F_N, false);
    }

    /// The ED page.
    ///
    /// Deliberately full of holes -- see the module comment. A DD/FD prefix
    /// in front of it is dropped, as on silicon: `DD ED 60` is `IN H, (C)`,
    /// with the prefix's four T-states still charged.
    fn exec_ed(&mut self, bus: &mut dyn Bus) -> StepResult {
        self.prefix_dd = false;
        self.prefix_fd = false;
        let op = self.read_n(bus);
        self.bump_r();
        let y = (op >> 3) & 0b111;
        let p = y >> 1;

        self.cycles += ED_CYCLES[op as usize] as u32;

        match op {
            // IN r, (C) -- the 0x70 encoding sets the flags and keeps the byte
            0x40 | 0x48 | 0x50 | 0x58 | 0x60 | 0x68 | 0x70 | 0x78 => {
                self.wz = self.bc().wrapping_add(1);
                let val = bus.io_read8(self.c as u16);
                if y != 0b110 {
                    self.write_r(y, val);
                }
                self.set_sz(val);
                self.set_xy(val);
                self.set_flag(F_PV, parity(val));
                self.set_flag(F_H, false);
                self.set_flag(F_N, false);
            }

            // OUT (C), r -- the 0x71 encoding writes zero, undocumented
            0x41 | 0x49 | 0x51 | 0x59 | 0x61 | 0x69 | 0x71 | 0x79 => {
                self.wz = self.bc().wrapping_add(1);
                let val = if y == 0b110 { 0 } else { self.read_r(y) };
                bus.io_write8(self.c as u16, val);
            }

            // SBC HL, ss -- H and PV use 16-bit-specific expressions
            0x42 | 0x52 | 0x62 | 0x72 => {
                let ss = self.read_dd(p);
                let hl = self.hl();
                let c = self.carry() as u32;
                let res = (hl as u32).wrapping_sub(ss as u32).wrapping_sub(c);
                self.wz = hl.wrapping_add(1);

                self.set_flag(F_C, hl as i32 - ss as i32 - (c as i32) < 0);
                self.set_flag(F_N, true);
                self.set_flag(F_PV, ((hl ^ ss) & (hl ^ res as u16) & 0x8000) != 0);
                self.set_flag(
                    F_H,
                    (hl & 0xfff) as i32 - (ss & 0xfff) as i32 - (c as i32) < 0,
                );
                self.set_flag(F_Z, res as u16 == 0);
                self.set_flag(F_S, (res & 0x8000) != 0);
                self.set_xy((res >> 8) as u8);
                self.set_hl(res as u16);
            }

            // ADC HL, ss
            0x4a | 0x5a | 0x6a | 0x7a => {
                let ss = self.read_dd(p);
                let hl = self.hl();
                let c = self.carry() as u32;
                let res = hl as u32 + ss as u32 + c;
                self.wz = hl.wrapping_add(1);

                self.set_flag(F_C, res > 0xffff);
                self.set_flag(F_N, false);
                self.set_flag(F_PV, ((hl ^ res as u16) & (ss ^ res as u16) & 0x8000) != 0);
                self.set_flag(F_H, (hl & 0xfff) as u32 + (ss & 0xfff) as u32 + c > 0xfff);
                self.set_flag(F_Z, res as u16 == 0);
                self.set_flag(F_S, (res & 0x8000) != 0);
                self.set_xy((res >> 8) as u8);
                self.set_hl(res as u16);
            }

            // LD (nn), dd
            0x43 | 0x53 | 0x63 | 0x73 => {
                let val = self.read_dd(p);
                let addr = self.read_nn(bus);
                bus.write16(addr as u32, val, Endian::Little);
                self.wz = addr.wrapping_add(1);
            }

            // LD dd, (nn)
            0x4b | 0x5b | 0x6b | 0x7b => {
                let addr = self.read_nn(bus);
                let val = bus.read16(addr as u32, Endian::Little);
                self.write_dd(p, val);
                self.wz = addr.wrapping_add(1);
            }

            // NEG, with the seven undocumented aliases that decode to it on
            // real silicon (every ED opcode whose low three bits are 0b100).
            0x44 | 0x4c | 0x54 | 0x5c | 0x64 | 0x6c | 0x74 | 0x7c => {
                let old = self.a;
                let res = 0u8.wrapping_sub(old);
                self.set_flag(F_S, (res & 0x80) != 0);
                self.set_flag(F_Z, res == 0);
                self.set_flag(F_H, (old & 0xf) != 0);
                self.set_flag(F_PV, old == 0x80);
                self.set_flag(F_N, true);
                self.set_flag(F_C, old != 0);
                self.set_xy(res);
                self.a = res;
            }

            // RETI: RET, and the bus is told, so the device under service
            // releases the daisy chain -- the SIO's IUS is what that means
            // here. Nothing else distinguishes it from RET.
            0x4d => {
                self.pc = self.pop16(bus);
                self.wz = self.pc;
                bus.interrupt_return();
            }

            // RETN and its undocumented aliases: RET, then IFF1 is restored
            // from the copy IFF2 kept when the interrupt was accepted.
            0x45 | 0x55 | 0x5d | 0x65 | 0x6d | 0x75 | 0x7d => {
                self.pc = self.pop16(bus);
                self.wz = self.pc;
                self.iff1 = self.iff2;
            }

            // IM 0 / IM 1 / IM 2, over all eight encodings -- the mapping is
            // not the regular one it looks like, so check it against silicon
            // rather than the pattern. 0x4e and 0x6e are the "IM 0/1" holes,
            // undefined on NMOS and taken as IM 0 here.
            0x46 | 0x4e | 0x66 | 0x6e => self.im = 0,
            0x56 | 0x76 => self.im = 1,
            0x5e | 0x7e => self.im = 2,

            0x47 => self.i = self.a, // LD I, A
            0x4f => self.r = self.a, // LD R, A

            // LD A, I / LD A, R -- PV picks up IFF2
            0x57 | 0x5f => {
                self.a = if op == 0x57 { self.i } else { self.r };
                let a = self.a;
                self.set_flag(F_PV, self.iff2);
                self.set_sz(a);
                self.set_xy(a);
                self.set_flag(F_H, false);
                self.set_flag(F_N, false);
            }

            0x67 => self.rotate_decimal(bus, true),  // RRD
            0x6f => self.rotate_decimal(bus, false), // RLD

            0xa1 => self.block_cp(bus, true, false),  // CPI
            0xa9 => self.block_cp(bus, false, false), // CPD
            0xb1 => self.block_cp(bus, true, true),   // CPIR
            0xb9 => self.block_cp(bus, false, true),  // CPDR

            0xa2 => self.block_in(bus, true, false),  // INI
            0xaa => self.block_in(bus, false, false), // IND
            0xb2 => self.block_in(bus, true, true),   // INIR
            0xba => self.block_in(bus, false, true),  // INDR

            0xa3 => self.block_out(bus, true, false), // OUTI
            0xab => self.block_out(bus, false, false), // OUTD
            0xb3 => self.block_out(bus, true, true),  // OTIR
            0xbb => self.block_out(bus, false, true), // OTDR

            0xa0 => self.block_move(bus, true, false), // LDI
            0xa8 => self.block_move(bus, false, false), // LDD
            0xb0 => self.block_move(bus, true, true),  // LDIR
            0xb8 => self.block_move(bus, false, true), // LDDR

            _ => {
                eprintln!("unhandled ED prefixed-opcode {op:#x}");
                return StepResult::BadOpcode;
            }
        }

        StepResult::Ok
    }

    /// The CB page: rotates, shifts and bit operations.
    ///
    /// Complete -- all 256 values decode. Under a DD/FD prefix the
    /// displacement is read here, before the opcode, and every operation
    /// works on `(IX+d)`: the `z` field then names a register that also
    /// receives the result (the undocumented writeback, plain H and L
    /// included), except for `BIT`, which has nothing to write. The fourth
    /// byte is fetched as an operand rather than an M1 cycle, so it does not
    /// count refresh.
    fn exec_cb(&mut self, bus: &mut dyn Bus) -> StepResult {
        let indexed = if self.indexed() {
            Some(self.indexed_addr(bus))
        } else {
            self.bump_r();
            None
        };
        let op = self.read_n(bus);

        let x = op >> 6;
        let y = (op >> 3) & 0b111;
        let z = op & 0b111;

        match x {
            0 => match indexed {
                Some(addr) => {
                    self.cycles += 19; // 23 with the prefix's 4
                    let val = self.mem_read(bus, addr);
                    let res = self.rot(ROT_OPS[y as usize], val);
                    self.mem_write(bus, addr, res);
                    if z != 0b110 {
                        self.write_r_plain(z, res); // undocumented writeback
                    }
                }
                None => {
                    self.cycles += if z == 0b110 { 15 } else { 8 };
                    let val = self.read_r_or_hl(bus, z);
                    let res = self.rot(ROT_OPS[y as usize], val);
                    self.write_r_or_hl(bus, z, res);
                }
            },

            1 => {
                // BIT b, r. C is untouched, S is only ever set by bit 7, PV
                // copies Z. X and Y are the tell: from the register for the
                // register forms, but from the high byte of the address for
                // `(IX+d)`, and from MEMPTR's high byte for `(HL)` -- the
                // one place the internal latch shows through.
                let (val, xy) = match indexed {
                    Some(addr) => {
                        self.cycles += 16; // 20 with the prefix's 4
                        (self.mem_read(bus, addr), (addr >> 8) as u8)
                    }
                    None if z == 0b110 => {
                        self.cycles += 12;
                        let addr = self.hl();
                        (self.mem_read(bus, addr), (self.wz >> 8) as u8)
                    }
                    None => {
                        self.cycles += 8;
                        let val = self.read_r(z);
                        (val, val)
                    }
                };
                let zero = (val & (1 << y)) == 0;
                self.set_flag(F_Z, zero);
                self.set_flag(F_PV, zero);
                self.set_flag(F_H, true);
                self.set_flag(F_N, false);
                self.set_flag(F_S, y == 7 && (val & 0x80) != 0);
                self.set_xy(xy);
            }

            // RES b, r and SET b, r -- no flags
            _ => {
                let set = x == 3;
                let apply = |v: u8| if set { v | (1 << y) } else { v & !(1 << y) };
                match indexed {
                    Some(addr) => {
                        self.cycles += 19; // 23 with the prefix's 4
                        let val = apply(self.mem_read(bus, addr));
                        self.mem_write(bus, addr, val);
                        if z != 0b110 {
                            self.write_r_plain(z, val); // undocumented writeback
                        }
                    }
                    None => {
                        self.cycles += if z == 0b110 { 15 } else { 8 };
                        let val = apply(self.read_r_or_hl(bus, z));
                        self.write_r_or_hl(bus, z, val);
                    }
                }
            }
        }

        StepResult::Ok
    }
}

impl Cpu for CpuZ80 {
    /// No reset vector, unlike the 6800/6809: the Z80 starts at 0.
    fn reset(&mut self, _bus: &mut dyn Bus) {
        *self = CpuZ80::default();
    }

    /// Exactly one instruction, prefixes included.
    ///
    /// The C++ does this with a `restart:`/`decode:` label pair and three
    /// `goto`s, but prefix resolution already happens within one call there, so
    /// one `step` is still one instruction and the cycle-limit and trace
    /// semantics carry over unchanged.
    fn step(&mut self, bus: &mut dyn Bus) -> StepResult {
        self.prefix_dd = false;
        self.prefix_fd = false;
        let elapsed = std::mem::replace(&mut self.cycles, 0);

        // Interrupt entry. NMI on the line's rising edge, ahead of the
        // maskable line; the maskable line with IFF1 set and not on the one
        // instruction after an `EI` (`ei_shadow`). Either wakes a `HALT`,
        // and the address pushed is then the one past it.
        let ints = bus.poll_interrupts(elapsed);
        let nmi_edge = ints.nmi && !self.nmi_line;
        self.nmi_line = ints.nmi;
        let shadowed = std::mem::replace(&mut self.ei_shadow, false);
        if nmi_edge {
            self.wake();
            self.iff2 = self.iff1;
            self.iff1 = false;
            self.bump_r();
            self.push16(bus, self.pc);
            self.pc = 0x66;
            self.wz = self.pc;
            self.cycles += 11;
            return StepResult::Ok;
        }
        let op = if ints.irq && self.iff1 && !shadowed {
            self.wake();
            self.iff1 = false;
            self.iff2 = false;
            // The acknowledge cycle is an M1 for refresh, and what the bus
            // answers with is the vector (IM 2), the instruction to run (IM
            // 0), or nothing anyone reads (IM 1).
            self.bump_r();
            let bus_byte = bus.interrupt_acknowledge();
            match self.im {
                // IM 2: 19 T-states, the vector through I to a table entry
                2 => {
                    let table = ((self.i as u16) << 8) | bus_byte as u16;
                    self.push16(bus, self.pc);
                    self.pc = bus.read16(table as u32, Endian::Little);
                    self.wz = self.pc;
                    self.cycles += 19;
                    return StepResult::Ok;
                }
                // IM 0: the byte on the bus is an instruction, and an `rst`
                // is the only one anything puts there -- anything else is
                // taken as the pulled-up bus's `rst 0x38`. IM 1: `rst 0x38`
                // regardless. Both are 13 T-states: the RST arm below charges
                // its fetched 11, and no opcode fetch happened here.
                0 if bus_byte & 0xc7 == 0xc7 => {
                    self.cycles += 2;
                    bus_byte
                }
                _ => {
                    self.cycles += 2;
                    0xff
                }
            }
        } else if self.halted {
            // asleep: a NOP's worth of refresh, nothing fetched
            self.bump_r();
            self.cycles += 4;
            return StepResult::Ok;
        } else {
            // A run of DD/FD prefixes: each is its own 4 T-state M1 cycle,
            // and the last one is the one that counts -- `DD FD 21 nn nn`
            // is `LD IY, nn` with a four-cycle DD in front of it.
            loop {
                let op = self.read_n(bus);
                self.bump_r();
                match op {
                    0xdd => {
                        self.prefix_dd = true;
                        self.prefix_fd = false;
                        self.cycles += 4;
                    }
                    0xfd => {
                        self.prefix_fd = true;
                        self.prefix_dd = false;
                        self.cycles += 4;
                    }
                    _ => break op,
                }
            }
        };

        let result = match op {
            0xed => self.exec_ed(bus),
            0xcb => self.exec_cb(bus),
            _ => self.exec_base(bus, op),
        };
        if result != StepResult::Ok {
            return result;
        }

        // Nothing costs less than the 4 T-states of the opcode fetch. A path
        // that forgot to charge must not report 0: the run loop reads that as
        // "this core does not count" and permanently disables the throttle.
        debug_assert!(self.cycles >= 4, "uncosted path for opcode {op:#04x}");

        StepResult::Ok
    }

    fn last_step_cycles(&self) -> u32 {
        self.cycles
    }

    fn pc(&self) -> u32 {
        self.pc as u32
    }

    /// The trace line's registers first, then the alternate set and the
    /// interrupt state the trace leaves out.
    fn registers(&self) -> Vec<Register> {
        let pair = |hi: u8, lo: u8| ((hi as u16) << 8) | lo as u16;
        vec![
            Register::new("PC", self.pc, 16),
            Register::new("AF", self.af(), 16),
            Register::new("BC", self.bc(), 16),
            Register::new("DE", self.de(), 16),
            Register::new("HL", self.hl(), 16),
            Register::new("IX", self.ix, 16),
            Register::new("IY", self.iy, 16),
            Register::new("SP", self.sp, 16),
            Register::new("AF2", self.af_alt(), 16),
            Register::new("BC2", pair(self.b_alt, self.c_alt), 16),
            Register::new("DE2", pair(self.d_alt, self.e_alt), 16),
            Register::new("HL2", pair(self.h_alt, self.l_alt), 16),
            Register::new("I", self.i, 8),
            Register::new("R", self.r, 8),
            Register::new("IM", self.im, 2),
            Register::new("IFF1", self.iff1 as u8, 1),
            Register::new("IFF2", self.iff2 as u8, 1),
            Register::new("WZ", self.wz, 16),
            Register::new("HALT", self.halted as u8, 1),
        ]
    }

    fn set_register(&mut self, name: &str, value: u32) -> bool {
        let w = value as u16;
        let (hi, lo) = ((w >> 8) as u8, w as u8);
        match name {
            "PC" => self.pc = w,
            "AF" => self.set_af(w),
            "BC" => self.set_bc(w),
            "DE" => self.set_de(w),
            "HL" => self.set_hl(w),
            "IX" => self.ix = w,
            "IY" => self.iy = w,
            "SP" => self.sp = w,
            "AF2" => self.set_af_alt(w),
            "BC2" => (self.b_alt, self.c_alt) = (hi, lo),
            "DE2" => (self.d_alt, self.e_alt) = (hi, lo),
            "HL2" => (self.h_alt, self.l_alt) = (hi, lo),
            "I" => self.i = lo,
            "R" => self.r = lo,
            "IM" => self.im = lo & 3,
            "IFF1" => self.iff1 = value & 1 != 0,
            "IFF2" => self.iff2 = value & 1 != 0,
            "WZ" => self.wz = w,
            "HALT" => self.halted = value & 1 != 0,
            _ => return false,
        }
        true
    }

    fn addressing(&self) -> Addressing {
        Addressing { endian: Endian::Little, unit_bytes: 1 }
    }

    fn dump(&self) {
        println!(
            "f 0x{:02x} ({}{}{}{}{}{}{}{}) a 0x{:02x} b 0x{:02x} c 0x{:02x} d 0x{:02x} e 0x{:02x} h 0x{:02x} l 0x{:02x} sp 0x{:04x} ix 0x{:04x} iy 0x{:04x} pc 0x{:04x}",
            self.f,
            if self.flag(F_C) { 'c' } else { ' ' },
            if self.flag(F_N) { 'n' } else { ' ' },
            if self.flag(F_PV) { 'p' } else { ' ' },
            if self.flag(F_X) { 'x' } else { ' ' },
            if self.flag(F_H) { 'h' } else { ' ' },
            if self.flag(F_Y) { 'y' } else { ' ' },
            if self.flag(F_Z) { 'z' } else { ' ' },
            if self.flag(F_S) { 's' } else { ' ' },
            self.a, self.b, self.c, self.d, self.e, self.h, self.l,
            self.sp, self.ix, self.iy, self.pc
        );
    }

    fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            out,
            "PC={:04x} AF={:02x}{:02x} BC={:02x}{:02x} DE={:02x}{:02x} HL={:02x}{:02x} IX={:04x} IY={:04x} SP={:04x}",
            self.pc, self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l, self.ix, self.iy, self.sp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::testbus::{
        check_set_register, registers_as_trace, run_steps, trace_of, TestBus,
    };

    /// Load a hand-assembled program at 0 and reset into it -- the z80 has no
    /// reset vector.
    fn boot(prog: &[u8]) -> (CpuZ80, TestBus) {
        let mut bus = TestBus::new();
        bus.load(0x0000, prog);
        let mut cpu = CpuZ80::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn registers_match_the_trace_line() {
        let (mut cpu, _bus) = boot(&[0x00]);
        cpu.pc = 0x0100;
        cpu.set_af(0x12c5);
        cpu.set_bc(0x3456);
        cpu.set_de(0x789a);
        cpu.set_hl(0xbcde);
        cpu.ix = 0xbeef;
        cpu.iy = 0xcafe;
        cpu.sp = 0xfffe;
        assert_eq!(registers_as_trace(&cpu, 8), trace_of(&cpu));
        assert_eq!(cpu.pc(), 0x100);
        assert_eq!(cpu.addressing(), Addressing { endian: Endian::Little, unit_bytes: 1 });
    }

    #[test]
    fn set_register_round_trips() {
        let (mut cpu, _bus) = boot(&[0x00]);
        check_set_register(
            &mut cpu,
            &[
                ("PC", 0x1234, 0x1234),
                ("AF", 0x12c5, 0x12c5),
                ("BC", 0x3456, 0x3456),
                ("DE", 0x789a, 0x789a),
                ("HL", 0xbcde, 0xbcde),
                ("IX", 0xbeef, 0xbeef),
                ("IY", 0xcafe, 0xcafe),
                ("SP", 0xfffe, 0xfffe),
                ("AF2", 0x2211, 0x2211),
                ("BC2", 0x4433, 0x4433),
                ("DE2", 0x6655, 0x6655),
                ("HL2", 0x8877, 0x8877),
                ("I", 0x1ab, 0xab),
                ("R", 0x7f, 0x7f),
                ("IM", 0x6, 0x2),
                ("IFF1", 1, 1),
                ("IFF2", 0, 0),
                ("HALT", 1, 1),
            ],
        );
        assert_eq!((cpu.a, cpu.f, cpu.h_alt, cpu.l_alt), (0x12, 0xc5, 0x88, 0x77));
    }

    #[test]
    fn reset_starts_at_zero_with_interrupt_mode_one() {
        let mut bus = TestBus::new();
        let mut cpu = CpuZ80::new();
        cpu.a = 0xff;
        cpu.ix = 0xffff;
        cpu.reset(&mut bus);
        assert_eq!(cpu.pc, 0);
        assert_eq!((cpu.a, cpu.ix, cpu.sp), (0, 0, 0));
        // the C++ `mRegs = {}` leaves the in-class initializer for im alone
        assert_eq!(cpu.im, 1);
    }

    /// Smoke test: sum four bytes through a djnz loop and store the result.
    #[test]
    fn sums_a_table_with_a_djnz_loop() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x31, 0x00, 0x80,       // 0000  ld   sp, 0x8000
            0x21, 0x00, 0x01,       // 0003  ld   hl, 0x0100
            0x06, 0x04,             // 0006  ld   b, 4
            0xaf,                   // 0008  xor  a
            0x86,                   // 0009  add  a, (hl)     <- loop
            0x23,                   // 000a  inc  hl
            0x10, 0xfc,             // 000b  djnz loop
            0x32, 0x00, 0x02,       // 000d  ld   (0x0200), a
        ]);
        bus.load(0x0100, &[0x01, 0x02, 0x03, 0x04]);

        // 4 setup + 4 iterations of 3 + the store
        run_steps(&mut cpu, &mut bus, 17);

        assert_eq!(cpu.a, 0x0a);
        assert_eq!(cpu.hl(), 0x0104);
        assert_eq!(cpu.b, 0);
        assert_eq!(cpu.sp, 0x8000);
        assert_eq!(cpu.pc, 0x0010);
        assert_eq!(bus.mem[0x0200], 0x0a);
    }

    /// `LD r, (IX+d)` adds the displacement **unsigned**, unlike every other
    /// indexed form, in both directions and under either prefix.
    #[test]
    fn ld_r_indexed_sign_extends_the_displacement() {
        for (prefix, load) in [(0xddu8, 0x21u8), (0xfd, 0x21)] {
            #[rustfmt::skip]
            let (mut cpu, mut bus) = boot(&[
                prefix, load, 0x00, 0x01,   // ld ix/iy, 0x0100
                prefix, 0x7e, 0xff,         // ld a, (ix-1) / (iy-1)
            ]);
            bus.mem[0x01ff] = 0xaa; // where an unsigned displacement would look
            bus.mem[0x00ff] = 0x55; // where a real z80 looks

            run_steps(&mut cpu, &mut bus, 2);
            assert_eq!(cpu.a, 0x55, "prefix {prefix:#04x}");
        }
    }

    /// The store direction is the contrast: `LD (IX+d), r` sign-extends, as
    /// every indexed form other than the one above does.
    #[test]
    fn ld_indexed_r_sign_extends_the_displacement() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x00, 0x01, // ld ix, 0x0100
            0x3e, 0x5a,             // ld a, 0x5a
            0xdd, 0x77, 0xff,       // ld (ix-1), a
        ]);

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.mem[0x00ff], 0x5a);
        assert_eq!(bus.mem[0x01ff], 0x00);
    }

    #[test]
    fn exx_swaps_the_alternate_register_set() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x01, 0x22, 0x11,       // ld bc, 0x1122
            0x11, 0x44, 0x33,       // ld de, 0x3344
            0x21, 0x66, 0x55,       // ld hl, 0x5566
            0xd9,                   // exx
            0x01, 0xbb, 0xaa,       // ld bc, 0xaabb
            0xd9,                   // exx
        ]);

        run_steps(&mut cpu, &mut bus, 4);
        assert_eq!((cpu.bc(), cpu.de(), cpu.hl()), (0, 0, 0));

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!((cpu.bc(), cpu.de(), cpu.hl()), (0x1122, 0x3344, 0x5566));
        assert_eq!(cpu.b_alt, 0xaa);
        assert_eq!(cpu.c_alt, 0xbb);
    }

    /// AF has its own exchange, which EXX must not touch.
    #[test]
    fn ex_af_swaps_only_the_accumulator_and_flags() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x3e, 0x12,             // ld a, 0x12
            0x37,                   // scf
            0x08,                   // ex af, af'
            0x3e, 0x34,             // ld a, 0x34
            0x08,                   // ex af, af'
        ]);

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.a, cpu.f), (0, 0));
        assert_eq!(cpu.af_alt(), 0x1201);

        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!((cpu.a, cpu.f), (0x12, F_C));
        assert_eq!(cpu.af_alt(), 0x3400);
    }

    /// HALT sleeps: pc stays on it, each step is a four-cycle NOP that
    /// refreshes and fetches nothing, and an interrupt wakes it with the
    /// address after the HALT pushed -- the Kaypro's `HALT; INI` disk loops
    /// depend on exactly that return address.
    #[test]
    fn halt_sleeps_until_an_interrupt_returns_past_it() {
        // ei; halt; ld a, 0x42 -- and rst 0x38 is a plain ret
        let (mut cpu, mut bus) = boot(&[0xfb, 0x76, 0x3e, 0x42]);
        bus.load(0x38, &[0xc9]);
        run_steps(&mut cpu, &mut bus, 2);
        assert!(cpu.halted);
        assert_eq!(cpu.pc, 0x0001, "held on the halt");
        let r = cpu.r;
        bus.watch = Some(0x0001);
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.pc, 0x0001);
        assert_eq!(cpu.last_step_cycles(), 4);
        assert_eq!(cpu.r, r + 3, "refresh goes on");
        assert_eq!(bus.watch_reads, 0, "nothing is fetched while asleep");
        assert_eq!(cpu.a, 0, "the ld has not run");

        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert!(!cpu.halted);
        assert_eq!(cpu.pc, 0x38);
        assert_eq!(bus.read16(cpu.sp as u32, Endian::Little), 0x0002, "past the halt");
        bus.irq = false;
        run_steps(&mut cpu, &mut bus, 2); // ret; ld a, 0x42
        assert_eq!((cpu.a, cpu.pc), (0x42, 0x0004));
    }

    /// NMI is edge-triggered: the step that sees the line rise takes it,
    /// with IFF1 copied into IFF2 and cleared, and a line still held does
    /// not take it again. It outranks the maskable line, ignores IFF1, and
    /// wakes a HALT.
    #[test]
    fn nmi_takes_the_rising_edge_and_saves_iff1_in_iff2() {
        // ei; halt; inc a -- nmi handler at 0x66: retn
        let (mut cpu, mut bus) = boot(&[0xfb, 0x76, 0x3c]);
        bus.load(0x66, &[0xed, 0x45]);
        run_steps(&mut cpu, &mut bus, 2);
        bus.nmi = true;
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x66, "nmi first, though irq was up too");
        assert_eq!(cpu.last_step_cycles(), 11);
        assert!((!cpu.iff1) && cpu.iff2, "iff1 saved in iff2");
        assert_eq!(bus.read16(cpu.sp as u32, Endian::Little), 0x0002);
        bus.irq = false;
        run_steps(&mut cpu, &mut bus, 1); // retn
        assert!(cpu.iff1, "retn restores it");
        assert_eq!(cpu.pc, 0x0002);
        run_steps(&mut cpu, &mut bus, 1); // inc a: the line is still high, no edge
        assert_eq!((cpu.a, cpu.pc), (1, 0x0003));
        bus.nmi = false;
        run_steps(&mut cpu, &mut bus, 1);
        bus.nmi = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x66, "a fresh edge");

        // with interrupts disabled it is taken all the same
        let (mut cpu, mut bus) = boot(&[0xf3, 0x00]);
        run_steps(&mut cpu, &mut bus, 1);
        bus.nmi = true;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x66);
        assert!(!cpu.iff2);
    }

    /// The acknowledge cycle asks the bus. IM 2 takes its answer as the low
    /// byte of a table entry under I, 19 T-states; IM 0 executes the `rst`
    /// it finds there, and takes anything else as the pulled-up bus; IM 1
    /// ignores it. All three acknowledge, and RETI reports back.
    #[test]
    fn interrupt_modes_take_the_bus_byte_as_the_silicon_does() {
        // im 2 ; ld a, 0x12 ; ld i, a ; ei ; nop ...
        let prog = [0xed, 0x5e, 0x3e, 0x12, 0xed, 0x47, 0xfb, 0x00, 0x00];
        let (mut cpu, mut bus) = boot(&prog);
        bus.load(0x1240, &[0x34, 0x12]); // table entry: handler at 0x1234
        bus.load(0x1234, &[0xed, 0x4d]); // reti
        run_steps(&mut cpu, &mut bus, 5);
        bus.irq = true;
        bus.vector = 0x40;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x1234);
        assert_eq!(cpu.last_step_cycles(), 19);
        assert_eq!(bus.acks, 1);
        assert_eq!(bus.read16(cpu.sp as u32, Endian::Little), 0x0008);
        bus.irq = false;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.pc, bus.retis), (0x0008, 1), "reti returns and reports");

        // im 0 with rst 0x10 on the bus
        let (mut cpu, mut bus) = boot(&[0xed, 0x46, 0xfb, 0x00, 0x00]);
        run_steps(&mut cpu, &mut bus, 3);
        bus.irq = true;
        bus.vector = 0xd7;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.pc, cpu.last_step_cycles(), bus.acks), (0x10, 13, 1));

        // im 0 with a byte that is not an rst: the pulled-up bus
        let (mut cpu, mut bus) = boot(&[0xed, 0x46, 0xfb, 0x00, 0x00]);
        run_steps(&mut cpu, &mut bus, 3);
        bus.irq = true;
        bus.vector = 0x00;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x38);

        // im 1 acknowledges too, and the byte does not matter
        let (mut cpu, mut bus) = boot(&[0xfb, 0x00, 0x00]);
        run_steps(&mut cpu, &mut bus, 2);
        bus.irq = true;
        bus.vector = 0xd7;
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.pc, bus.acks), (0x38, 1));
    }

    #[test]
    fn ldir_copies_a_block() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x21, 0x00, 0x01,       // ld hl, 0x0100
            0x11, 0x00, 0x02,       // ld de, 0x0200
            0x01, 0x04, 0x00,       // ld bc, 4
            0xed, 0xb0,             // ldir
        ]);
        bus.load(0x0100, &[0xde, 0xad, 0xbe, 0xef]);

        // ldir rewinds pc by two to repeat, so it steps once per byte
        run_steps(&mut cpu, &mut bus, 7);

        assert_eq!(&bus.mem[0x0200..0x0204], &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!((cpu.hl(), cpu.de(), cpu.bc()), (0x0104, 0x0204, 0));
        assert_eq!(cpu.pc, 0x000b);
    }

    /// LDI and LDD move a single byte and leave PC alone -- the repeat is what
    /// separates them from LDIR/LDDR, which share the same helper.
    #[test]
    fn the_single_shot_block_moves_step_in_both_directions() {
        for (op, hl, de) in [(0xa0u8, 0x0101u16, 0x0201u16), (0xa8, 0x00ff, 0x01ff)] {
            #[rustfmt::skip]
            let (mut cpu, mut bus) = boot(&[
                0x21, 0x00, 0x01,   // ld hl, 0x0100
                0x11, 0x00, 0x02,   // ld de, 0x0200
                0x01, 0x02, 0x00,   // ld bc, 2
                0xed, op,           // ldi / ldd
            ]);
            bus.load(0x0100, &[0x5a]);
            run_steps(&mut cpu, &mut bus, 4);

            assert_eq!(bus.mem[0x0200], 0x5a, "ed {op:#04x}");
            assert_eq!((cpu.hl(), cpu.de(), cpu.bc()), (hl, de, 1), "ed {op:#04x}");
            assert_eq!(cpu.pc, 0x000b, "single-shot: no rewind");
        }
    }

    #[test]
    fn lddr_copies_a_block_backwards() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x21, 0x03, 0x01,       // ld hl, 0x0103 -- last byte of the source
            0x11, 0x03, 0x02,       // ld de, 0x0203
            0x01, 0x04, 0x00,       // ld bc, 4
            0xed, 0xb8,             // lddr
        ]);
        bus.load(0x0100, &[0xde, 0xad, 0xbe, 0xef]);
        run_steps(&mut cpu, &mut bus, 7);

        assert_eq!(&bus.mem[0x0200..0x0204], &[0xde, 0xad, 0xbe, 0xef]);
        assert_eq!((cpu.hl(), cpu.de(), cpu.bc()), (0x00ff, 0x01ff, 0));
        assert_eq!(cpu.pc, 0x000b);
    }

    /// PV is "BC is still non-zero", so it is set on every iteration but the
    /// last -- which is how a guest spots the final byte.
    #[test]
    fn a_block_move_reports_bc_in_pv() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x21, 0x00, 0x01,       // ld hl, 0x0100
            0x11, 0x00, 0x02,       // ld de, 0x0200
            0x01, 0x02, 0x00,       // ld bc, 2
            0xed, 0xb0,             // ldir
        ]);
        run_steps(&mut cpu, &mut bus, 4);
        assert_ne!(cpu.f & F_PV, 0, "one byte still to go");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.f & F_PV, 0, "bc hit zero");
        // H and N are always cleared, whichever way it went
        assert_eq!(cpu.f & (F_H | F_N), 0);
    }

    /// NEG decodes at all eight `ED xx4/xxC` encodings, and RETN at 0x45 plus
    /// its six aliases. Assemblers emit the canonical form, but hand-written
    /// and self-modifying code does reach the others.
    #[test]
    fn the_neg_and_retn_aliases_all_decode() {
        for op in [0x44u8, 0x4c, 0x54, 0x5c, 0x64, 0x6c, 0x74, 0x7c] {
            let (mut cpu, mut bus) = boot(&[0x3e, 0x01, 0xed, op]); // ld a,1 ; neg
            run_steps(&mut cpu, &mut bus, 2);
            assert_eq!(cpu.a, 0xff, "ed {op:#04x}");
            assert_eq!(cpu.f & (F_N | F_C), F_N | F_C, "ed {op:#04x}");
        }

        for op in [0x45u8, 0x55, 0x5d, 0x65, 0x6d, 0x75, 0x7d] {
            // push 0x1234 as the return address, then retn to it
            let (mut cpu, mut bus) = boot(&[0x21, 0x34, 0x12, 0xe5, 0xed, op]);
            cpu.sp = 0x8000;
            // iff2 is the copy an interrupt entry left behind; retn restores it
            cpu.iff1 = false;
            cpu.iff2 = true;
            run_steps(&mut cpu, &mut bus, 3);
            assert_eq!(cpu.pc, 0x1234, "ed {op:#04x}");
            assert!(cpu.iff1, "ed {op:#04x}: retn restores iff1 from iff2");
        }
    }

    /// All eight IM encodings. The mapping is irregular -- 0x5e is IM 2 while
    /// the neighbouring 0x56 is IM 1, and 0x66 drops back to IM 0 -- so it is
    /// worth pinning every one rather than trusting the pattern.
    #[test]
    fn every_im_encoding_selects_the_right_mode() {
        for (op, mode) in [
            (0x46u8, 0u8),
            (0x4e, 0),
            (0x66, 0),
            (0x6e, 0),
            (0x56, 1),
            (0x76, 1),
            (0x5e, 2),
            (0x7e, 2),
        ] {
            let (mut cpu, mut bus) = boot(&[0xed, op]);
            cpu.im = 3; // a value no encoding can produce
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!(cpu.im, mode, "ed {op:#04x}");
        }
    }

    /// An instruction that saw a DD/FD prefix but had no use for it ends the
    /// run -- not an error path so much as the decode's shape, since it decides
    /// how far a run gets.
    #[test]
    /// A DD/FD in front of an instruction that has no use for it is four
    /// T-states and nothing else; a run of them counts each, and the last
    /// one decides the index register.
    fn an_index_prefix_without_a_use_is_only_its_fetch() {
        let (mut cpu, mut bus) = boot(&[0xdd, 0x00, 0x00]); // dd nop
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.pc, 2);
        assert_eq!(cpu.last_step_cycles(), 8);

        let (mut cpu, mut bus) = boot(&[0xdd, 0xfd, 0x21, 0x34, 0x12]); // dd fd ld iy, nn
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!((cpu.ix, cpu.iy), (0, 0x1234), "the last prefix wins");
        assert_eq!(cpu.last_step_cycles(), 18);

        // the ED page drops the prefix: DD ED 60 is IN H, (C), not IN IXh
        let (mut cpu, mut bus) = boot(&[0x0e, 0x20, 0xdd, 0xed, 0x60]); // ld c, 0x20; dd in h, (c)
        bus.ports[0x20] = 0x5a;
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!((cpu.h, cpu.ix), (0x5a, 0));
        assert_eq!(cpu.last_step_cycles(), 16);
    }

    #[test]
    fn the_index_register_halves_are_registers_under_a_prefix() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x34, 0x12, // ld ix, 0x1234
            0xdd, 0x26, 0x56,       // ld ixh, 0x56
            0xdd, 0x2c,             // inc ixl
            0xdd, 0x7d,             // ld a, ixl
            0xdd, 0x84,             // add a, ixh
            0xfd, 0x65,             // ld iyh, iyl
            0x26, 0xaa,             // ld h, 0xaa
        ]);
        cpu.iy = 0x00bb;
        run_steps(&mut cpu, &mut bus, 7);
        assert_eq!(cpu.ix, 0x5635);
        assert_eq!(cpu.a, 0x56 + 0x35);
        assert_eq!(cpu.iy, 0xbbbb);
        assert_eq!(cpu.h, 0xaa, "H itself is untouched by all of it");
        assert_eq!(cpu.l, 0);
    }

    /// The prefix on `LD r, (IX+d)` and `LD (IX+d), r` is spent on the
    /// operand: DD 66 d is LD H, (IX+d), and DD 74 d stores H, never IXh.
    #[test]
    fn indexed_loads_name_the_plain_h_and_l() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x00, 0x20, // ld ix, 0x2000
            0xdd, 0x66, 0x01,       // ld h, (ix+1)
            0xdd, 0x74, 0x02,       // ld (ix+2), h
        ]);
        bus.load(0x2001, &[0x99]);
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.h, 0x99);
        assert_eq!(cpu.ix, 0x2000, "ixh untouched");
        assert_eq!(bus.mem[0x2002], 0x99);
    }

    /// Every DD CB operation works on (IX+d), and the register the low
    /// bits name -- plain H and L included -- receives the result.
    #[test]
    fn indexed_cb_operations_write_back_to_the_named_register() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0xdd, 0x21, 0x00, 0x20, // ld ix, 0x2000
            0xdd, 0xcb, 0x01, 0x14, // rl (ix+1), h
            0xdd, 0xcb, 0x01, 0x3f, // srl (ix+1), a
            0xdd, 0xcb, 0x01, 0xc5, // set 0, (ix+1), l
            0xdd, 0xcb, 0x01, 0x4e, // bit 1, (ix+1)
        ]);
        bus.load(0x2001, &[0x81]);
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(bus.mem[0x2001], 0x02);
        assert_eq!(cpu.h, 0x02);
        assert!(cpu.flag(F_C));
        assert_eq!(cpu.last_step_cycles(), 23);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((bus.mem[0x2001], cpu.a), (0x01, 0x01));
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((bus.mem[0x2001], cpu.l), (0x01, 0x01));
        run_steps(&mut cpu, &mut bus, 1);
        assert!(cpu.flag(F_Z));
        assert_eq!(cpu.f & F_XY, 0x20 & F_XY, "x/y from the address's high byte, 0x20");
        assert_eq!(cpu.last_step_cycles(), 20);
    }

    /// Bits 3 and 5 of F copy the result's, except where they copy
    /// something else: CP takes the operand's, BIT n, (HL) takes MEMPTR's
    /// high byte, and the block moves and compares build a byte of their
    /// own with bit 1 standing in for bit 5.
    #[test]
    fn flag_bits_3_and_5_follow_the_result() {
        // add a, n: result 0x28 has both bits set
        let (mut cpu, mut bus) = boot(&[0x3e, 0x20, 0xc6, 0x08]); // ld a, 0x20; add a, 8
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.f & F_XY, F_XY);
        // dec a: 0x28 -> 0x27, bit 5 set, bit 3 clear
        let (mut cpu, mut bus) = boot(&[0x3e, 0x28, 0x3d]); // ld a, 0x28; dec a
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.f & F_XY, F_Y);
        // cp n: the result 0x00 has neither, the operand 0x28 has both
        let (mut cpu, mut bus) = boot(&[0x3e, 0x28, 0xfe, 0x28]); // ld a, 0x28; cp 0x28
        run_steps(&mut cpu, &mut bus, 2);
        assert!(cpu.flag(F_Z));
        assert_eq!(cpu.f & F_XY, F_XY, "cp takes x/y from the operand");
        // add hl, bc: from the high byte of the result
        let (mut cpu, mut bus) = boot(&[0x21, 0x00, 0x20, 0x01, 0x00, 0x08, 0x09]); // ld hl, 0x2000; ld bc, 0x0800; add hl, bc
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.f & F_XY, F_XY);
        // scf and ccf copy A's
        let (mut cpu, mut bus) = boot(&[0x3e, 0x28, 0x37, 0x3f]); // ld a, 0x28; scf; ccf
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.f & F_XY, F_XY);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.f & F_XY, F_XY);
        assert!(!cpu.flag(F_C) && cpu.flag(F_H));
    }

    #[test]
    fn bit_on_hl_leaks_memptr() {
        // ld a, (0x27ff) leaves MEMPTR at 0x2800; bit 0, (hl) then shows 0x28's bits
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x21, 0x00, 0x20,       // ld hl, 0x2000
            0x3a, 0xff, 0x27,       // ld a, (0x27ff)
            0xcb, 0x46,             // bit 0, (hl)
            0x01, 0xff, 0x07,       // ld bc, 0x07ff
            0x0a,                   // ld a, (bc) -- MEMPTR 0x0800
            0xcb, 0x46,             // bit 0, (hl)
        ]);
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.wz, 0x2800);
        assert_eq!(cpu.f & F_XY, F_XY);
        assert!(cpu.flag(F_Z) && cpu.flag(F_PV), "pv copies z");
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(cpu.wz, 0x0800);
        assert_eq!(cpu.f & F_XY, F_X);
        // the register form takes them from the register
        let (mut cpu, mut bus) = boot(&[0x06, 0x28, 0xcb, 0x40]); // ld b, 0x28; bit 0, b
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.f & F_XY, F_XY);
    }

    #[test]
    fn block_moves_and_compares_build_their_own_x_and_y() {
        // ldi: n = byte + a = 0x08 + 0x02 = 0x0a -> y from bit 1 (set), x from bit 3 (set)
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x3e, 0x02,             // ld a, 2
            0x21, 0x00, 0x20,       // ld hl, 0x2000
            0x11, 0x00, 0x30,       // ld de, 0x3000
            0x01, 0x02, 0x00,       // ld bc, 2
            0xed, 0xa0,             // ldi
            0x3e, 0x10,             // ld a, 0x10
            0xed, 0xa1,             // cpi: 0x10 - 0x08 = 0x08 with a half borrow -> n = 0x07
        ]);
        bus.load(0x2000, &[0x08, 0x08]);
        run_steps(&mut cpu, &mut bus, 5);
        assert_eq!(cpu.f & F_XY, F_XY);
        assert!(cpu.flag(F_PV), "bc still non-zero");
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.f & F_XY, F_Y);
        assert!(!cpu.flag(F_PV), "bc reached zero");
    }

    /// INI/OUTI flags beyond the documented Z and N, per The Undocumented
    /// Z80 Documented 4.2: N from bit 7 of the byte, H and C from the
    /// carry of byte + (C+1) for INI, S/Z/X/Y from the decremented B, PV
    /// from the parity of (k & 7) ^ B.
    #[test]
    fn block_io_sets_the_undocumented_flags() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x01, 0xff, 0x02,       // ld bc, 0x02ff
            0x21, 0x00, 0x20,       // ld hl, 0x2000
            0xed, 0xa2,             // ini
        ]);
        bus.ports[0xff] = 0x81;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.mem[0x2000], 0x81);
        assert_eq!(cpu.b, 1);
        // k = 0x81 + ((0xff + 1) & 0xff) = 0x81: no carry; n from bit 7 of 0x81
        assert!(cpu.flag(F_N) && !cpu.flag(F_H) && !cpu.flag(F_C));
        assert!(!cpu.flag(F_Z) && !cpu.flag(F_S));
        assert_eq!(cpu.f & F_XY, 0);
        // pv: parity((0x81 & 7) ^ 1) = parity(0) = even -> set
        assert!(cpu.flag(F_PV));
        assert_eq!(cpu.wz, 0x0300, "bc before the decrement, plus one");
    }

    /// R counts M1 cycles: one per opcode byte and per prefix, but the
    /// displacement and opcode of a DD CB are operands and count nothing.
    /// Bit 7 belongs to LD R, A.
    #[test]
    fn r_counts_opcode_fetches() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x00,                   // nop: 1
            0xcb, 0x00,             // rlc b: 2
            0xed, 0x44,             // neg: 2
            0xdd, 0xcb, 0x01, 0x06, // rlc (ix+1): 2
            0xdd, 0xfd, 0x00,       // dd fd nop: 3
            0xed, 0x5f,             // ld a, r: 2 -- read after its own fetches
        ]);
        cpu.r = 0x80;
        run_steps(&mut cpu, &mut bus, 6);
        assert_eq!(cpu.r, 0x80 | 12);
        assert_eq!(cpu.a, 0x80 | 12);
    }

    #[test]
    fn port_io_reaches_the_bus() {
        #[rustfmt::skip]
        let (mut cpu, mut bus) = boot(&[
            0x3e, 0x5a,             // ld a, 0x5a
            0xd3, 0x10,             // out (0x10), a
            0xdb, 0x20,             // in  a, (0x20)
        ]);
        bus.ports[0x20] = 0xa5;

        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!(bus.io_writes, vec![(0x10, 0x5a)]);
        assert_eq!(cpu.a, 0xa5);
    }

    #[test]
    fn add_sets_carry_half_carry_and_overflow() {
        let add = |a: u8, n: u8| {
            let (mut cpu, mut bus) = boot(&[0x3e, a, 0xc6, n]); // ld a, a ; add a, n
            run_steps(&mut cpu, &mut bus, 2);
            cpu
        };

        // half carry out of bit 3, nothing else
        let cpu = add(0x0f, 0x01);
        assert_eq!(cpu.a, 0x10);
        assert_eq!(
            (
                cpu.flag(F_C),
                cpu.flag(F_H),
                cpu.flag(F_Z),
                cpu.flag(F_S),
                cpu.flag(F_PV)
            ),
            (false, true, false, false, false)
        );

        // carry out of bit 7, result zero
        let cpu = add(0xff, 0x01);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(
            (
                cpu.flag(F_C),
                cpu.flag(F_H),
                cpu.flag(F_Z),
                cpu.flag(F_S),
                cpu.flag(F_PV)
            ),
            (true, true, true, false, false)
        );

        // signed overflow: PV, not C
        let cpu = add(0x7f, 0x01);
        assert_eq!(cpu.a, 0x80);
        assert_eq!(
            (
                cpu.flag(F_C),
                cpu.flag(F_H),
                cpu.flag(F_Z),
                cpu.flag(F_S),
                cpu.flag(F_PV)
            ),
            (false, true, false, true, true)
        );

        // N is always cleared by add
        assert!(!add(0x01, 0x01).flag(F_N));
    }

    // -- cycle counts ---

    /// Representative rows of the Zilog manual's timing tables, one per
    /// encoding family, conditionals in their not-taken state (the taken
    /// surcharges have their own test below).
    #[test]
    fn cycle_counts_match_the_zilog_manual() {
        #[rustfmt::skip]
        let cases: &[(&[u8], u32)] = &[
            (&[0x00],                   4), // nop
            (&[0x06, 0x12],             7), // ld b, n
            (&[0x01, 0x34, 0x12],      10), // ld bc, nn
            (&[0x7e],                   7), // ld a, (hl)
            (&[0x70],                   7), // ld (hl), b
            (&[0x36, 0x12],            10), // ld (hl), n
            (&[0x34],                  11), // inc (hl)
            (&[0x80],                   4), // add a, b
            (&[0x86],                   7), // add a, (hl)
            (&[0xc6, 0x12],             7), // add a, n
            (&[0x09],                  11), // add hl, bc
            (&[0x22, 0x00, 0x20],      16), // ld (nn), hl
            (&[0x32, 0x00, 0x20],      13), // ld (nn), a
            (&[0x18, 0x02],            12), // jr e
            (&[0xc3, 0x00, 0x10],      10), // jp nn
            (&[0xe9],                   4), // jp (hl)
            (&[0xc9],                  10), // ret
            (&[0xcd, 0x00, 0x10],      17), // call nn
            (&[0xc5],                  11), // push bc
            (&[0xc1],                  10), // pop bc
            (&[0xe3],                  19), // ex (sp), hl
            (&[0xf9],                   6), // ld sp, hl
            (&[0xc7],                  11), // rst 00
            (&[0xd3, 0x40],            11), // out (n), a
            (&[0xdb, 0x40],            11), // in a, (n)
            (&[0x76],                   4), // halt
            // dd/fd forms: +4 for the prefix, +8 where a displacement is read
            (&[0xdd, 0x21, 0x34, 0x12], 14), // ld ix, nn
            (&[0xdd, 0x7e, 0x02],       19), // ld a, (ix+d)
            (&[0xfd, 0x70, 0x02],       19), // ld (iy+d), b
            (&[0xdd, 0x36, 0x02, 0x12], 19), // ld (ix+d), n -- the +5 overlap
            (&[0xdd, 0x34, 0x02],       23), // inc (ix+d)
            (&[0xdd, 0x86, 0x02],       19), // add a, (ix+d)
            (&[0xdd, 0x09],             15), // add ix, bc
            (&[0xdd, 0xe3],             23), // ex (sp), ix
            (&[0xdd, 0xe9],              8), // jp (ix)
            (&[0xdd, 0xe5],             15), // push ix
            (&[0xdd, 0xdd, 0x21, 0x34, 0x12], 18), // stacked prefixes: 4 each
            // cb page
            (&[0xcb, 0x00],              8), // rlc b
            (&[0xcb, 0x06],             15), // rlc (hl)
            (&[0xcb, 0x40],              8), // bit 0, b
            (&[0xcb, 0x46],             12), // bit 0, (hl)
            (&[0xcb, 0xc6],             15), // set 0, (hl)
            (&[0xdd, 0xcb, 0x02, 0x06], 23), // rlc (ix+d)
            (&[0xdd, 0xcb, 0x02, 0x46], 20), // bit 0, (ix+d)
            (&[0xfd, 0xcb, 0x02, 0xc6], 23), // set 0, (iy+d)
            // ed page
            (&[0xed, 0x44],              8), // neg
            (&[0xed, 0x47],              9), // ld i, a
            (&[0xed, 0x4a],             15), // adc hl, bc
            (&[0xed, 0x43, 0x00, 0x20], 20), // ld (nn), bc
            (&[0xed, 0x45],             14), // retn
            (&[0xed, 0x56],              8), // im 1
            (&[0xed, 0x67],             18), // rrd
            (&[0xed, 0x78],             12), // in a, (c)
            (&[0xed, 0xa0],             16), // ldi
            (&[0xed, 0xa1],             16), // cpi
        ];
        for (prog, cycles) in cases {
            let (mut cpu, mut bus) = boot(prog);
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!(cpu.last_step_cycles(), *cycles, "bytes {prog:02x?}");
        }
    }

    /// The conditional flow costs, taken vs not. After reset every flag is
    /// clear, so NZ/NC take and Z/C do not. `JP cc` really is 10 both ways:
    /// the target is always read.
    #[test]
    fn conditional_flow_charges_the_taken_surcharge() {
        #[rustfmt::skip]
        let cases: &[(&[u8], u32)] = &[
            (&[0x20, 0x02],       12), // jr nz (taken)
            (&[0x28, 0x02],        7), // jr z (not taken)
            (&[0xc0],             11), // ret nz (taken)
            (&[0xc8],              5), // ret z (not taken)
            (&[0xc4, 0x00, 0x10], 17), // call nz (taken)
            (&[0xcc, 0x00, 0x10], 10), // call z (not taken)
            (&[0xc2, 0x00, 0x10], 10), // jp nz (taken)
            (&[0xca, 0x00, 0x10], 10), // jp z (not taken)
        ];
        for (prog, cycles) in cases {
            let (mut cpu, mut bus) = boot(prog);
            run_steps(&mut cpu, &mut bus, 1);
            assert_eq!(cpu.last_step_cycles(), *cycles, "bytes {prog:02x?}");
        }

        // djnz: b wraps 0 -> 255 first (taken), then reaches 0 from 1 (not)
        let (mut cpu, mut bus) = boot(&[0x10, 0x02]);
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.last_step_cycles(), 13, "djnz taken");
        let (mut cpu, mut bus) = boot(&[0x06, 0x01, 0x10, 0x02]);
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.last_step_cycles(), 8, "djnz not taken");
    }

    /// A repeating block op is 21 T-states per iteration and 16 on the last.
    #[test]
    fn block_repeats_charge_the_rewind() {
        // ld bc, 2; ldir
        let (mut cpu, mut bus) = boot(&[0x01, 0x02, 0x00, 0xed, 0xb0]);
        run_steps(&mut cpu, &mut bus, 2);
        assert_eq!(cpu.last_step_cycles(), 21, "bc still nonzero: repeats");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.last_step_cycles(), 16, "final iteration");
    }

    /// IM 1 interrupt acceptance is 13 T-states -- no opcode fetch happens,
    /// so it is not the fetched `rst 0x38`'s 11.
    #[test]
    fn im1_acceptance_reports_thirteen() {
        let (mut cpu, mut bus) = boot(&[0xfb, 0x00]); // ei; nop
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 2); // ei, and the nop its shadow covers
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x38);
        assert_eq!(cpu.last_step_cycles(), 13);
    }

    /// A maskable interrupt is not accepted until the instruction after
    /// `EI` has run, and every `EI` renews that shadow. A handler that ends
    /// `EI; RETI` -- the RC2014 factory rom's does -- relies on it: without
    /// the shadow a line still asserted is taken at the `RETI` and the
    /// handler nests once per character.
    #[test]
    fn an_interrupt_waits_for_the_instruction_after_ei() {
        // ei; inc a; inc a -- with the line held from the start
        let (mut cpu, mut bus) = boot(&[0xfb, 0x3c, 0x3c]);
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 1); // ei: iff1 was clear at the poll
        assert!(cpu.iff1);
        run_steps(&mut cpu, &mut bus, 1); // the shadowed instruction runs
        assert_eq!((cpu.pc, cpu.a), (2, 1), "inc a ran before the interrupt");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!((cpu.pc, cpu.a), (0x38, 1), "accepted after it");
        assert!(!cpu.iff1 && !cpu.iff2);

        // ei; ei; inc a: the second ei is under the first's shadow and
        // casts its own
        let (mut cpu, mut bus) = boot(&[0xfb, 0xfb, 0x3c]);
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.pc, cpu.a), (3, 1), "both eis and the inc ran");
        run_steps(&mut cpu, &mut bus, 1);
        assert_eq!(cpu.pc, 0x38);

        // di under the shadow: the shadow lapses, the line stays masked
        let (mut cpu, mut bus) = boot(&[0xfb, 0xf3, 0x3c]);
        bus.irq = true;
        run_steps(&mut cpu, &mut bus, 3);
        assert_eq!((cpu.pc, cpu.a), (3, 1));
    }

    /// Every opcode value that completes charges at least the 4 T-states of
    /// its own fetch. A path that forgot to charge would report 0, which the
    /// run loop reads as "this core does not count cycles" -- and that
    /// permanently disables the throttle, not just one instruction's pacing.
    #[test]
    fn every_completed_step_charges_at_least_the_fetch() {
        for op in 0..=0xffu8 {
            let (mut cpu, mut bus) = boot(&[op, 0x00, 0x00, 0x00]);
            if cpu.step(&mut bus) == StepResult::Ok {
                assert!(cpu.last_step_cycles() >= 4, "op {op:#04x}");
            }
        }
    }
}
