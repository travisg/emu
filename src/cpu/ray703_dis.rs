// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 703 disassembler, in `tools/asm703.py`'s syntax -- which is SYM II's,
//! as the 1968 listings print it: upper case, `X'1A4'` hex, `*` for an
//! indexed operand.
//!
//! A memory reference shows its eleven-bit M field as it stands, a word
//! address or a byte address by the instruction: the page it lands in is
//! EXR's at run time, which a word alone does not say. A field the core
//! ignores (HLT's F2, a skip's) does not show, so such a word reads as the
//! instruction it executes as. An encoding the core rejects is `DATA`.

use super::Disassembly;

const MEMREF: [&str; 16] = [
    "", "JMP", "JSX", "STB", "CMB", "LDB", "STX", "STW", "LDW", "LDX", "ADD", "SUB", "ORI", "ORE", "AND", "CMW",
];
const CONTROL: [&str; 12] = ["HLT", "INR", "ENB", "DSB", "SLM", "SGM", "CEX", "CXE", "SML", "SMU", "MSK", "UNM"];
const REGISTER: [&str; 5] = ["CLR", "CMP", "INV", "CAX", "CXA"];
const SKIPS: [&str; 16] = [
    "SAZ", "SAP", "SAM", "SAO", "SLS", "SXE", "SEQ", "SNE", "SGR", "SLE", "SNO", "SSE", "SS0", "SS1", "SS2", "SS3",
];
const ARITH_SHIFTS: [&str; 4] = ["SRA", "SLA", "SRAD", "SLAD"];
const LOGICAL_SHIFTS: [&str; 16] = [
    "SRL", "SLL", "SRLD", "SLLD", "SRC", "SLC", "SRCD", "SLCD", "SRLL", "SLLL", "SRLR", "SLLR", "SRCL", "SLCL", "SRCR",
    "SLCR",
];

/// `X'1A4'`, or a bare decimal digit for the values a listing writes so.
fn hex(v: u16) -> String {
    if v < 10 {
        v.to_string()
    } else {
        format!("X'{v:X}'")
    }
}

fn word(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<u16> {
    let at = (addr & 0x7fff) * 2;
    Some(u16::from_be_bytes([peek(at)?, peek(at + 1)?]))
}

/// The instruction at word `addr`.
pub fn disassemble(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<Disassembly> {
    let insn = word(peek, addr)?;
    let op = (insn >> 12) as usize;
    let one = |text: String| Some(Disassembly { text, len: 1 });
    if op != 0 {
        let star = if insn & 0x0800 != 0 { "*" } else { "" };
        return one(format!("{} {star}{}", MEMREF[op], hex(insn & 0x07ff)));
    }
    let class = (insn >> 8) & 0x0f;
    let f1 = ((insn >> 4) & 0x0f) as usize;
    let f2 = insn & 0x0f;
    let literal = insn & 0xff;
    match class {
        0x0 => match f1 {
            0x1..=0x3 | 0x8 | 0x9 => one(format!("{} {f2}", CONTROL[f1])),
            0x0 | 0x4..=0x7 | 0xa | 0xb => one(CONTROL[f1].to_string()),
            _ => one(format!("DATA {}", hex(insn))),
        },
        0x1 if f1 < REGISTER.len() => one(REGISTER[f1].to_string()),
        0x2 => one(format!("DIN {f1},{f2}")),
        0x3 => one(format!("DOT {f1},{f2}")),
        0x4 => one(format!("IXS {}", hex(literal))),
        0x5 => one(format!("DXS {}", hex(literal))),
        0x6 => one(format!("LLB {}", hex(literal))),
        0x7 => one(format!("CLB {}", hex(literal))),
        0x8 => one(SKIPS[f1].to_string()),
        0x9 if f1 < ARITH_SHIFTS.len() => one(format!("{} {f2}", ARITH_SHIFTS[f1])),
        0xa => one(format!("{} {f2}", LOGICAL_SHIFTS[f1])),
        0xb | 0xc if literal == 0x0f => {
            let operand = word(peek, addr + 1)? & 0x7fff;
            let name = if class == 0xb { "MPY" } else { "DIV" };
            Some(Disassembly { text: format!("{name} {}", hex(operand)), len: 2 })
        }
        _ => one(format!("DATA {}", hex(insn))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dis(words: &[u16]) -> Disassembly {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        disassemble(&|a| bytes.get(a as usize).copied(), 0).unwrap()
    }

    fn text(words: &[u16]) -> String {
        dis(words).text
    }

    /// One of each shape, against the words the relocating loader's
    /// listing prints beside its cards.
    #[test]
    fn each_shape_reads_as_the_listing_writes_it() {
        assert_eq!(text(&[0x854e]), "LDW X'54E'");
        assert_eq!(text(&[0x9800]), "LDX *0");
        assert_eq!(text(&[0x3b85]), "STB *X'385'");
        assert_eq!(text(&[0x0000]), "HLT");
        assert_eq!(text(&[0x0013]), "INR 3");
        assert_eq!(text(&[0x0084]), "SML 4");
        assert_eq!(text(&[0x0110]), "CMP");
        assert_eq!(text(&[0x039e]), "DOT 9,14");
        assert_eq!(text(&[0x0402]), "IXS 2");
        assert_eq!(text(&[0x06a0]), "LLB X'A0'");
        assert_eq!(text(&[0x07b9]), "CLB X'B9'");
        assert_eq!(text(&[0x0870]), "SNE");
        assert_eq!(text(&[0x0900]), "SRA 0");
        assert_eq!(text(&[0x0a78]), "SLCD 8");
        assert_eq!(text(&[0x0af3]), "SLCR 3");
    }

    #[test]
    fn multiply_and_divide_take_their_second_word() {
        assert_eq!(dis(&[0x0b0f, 0x8123]), Disassembly { text: "MPY X'123'".into(), len: 2 });
        assert_eq!(dis(&[0x0c0f, 0x0040]), Disassembly { text: "DIV X'40'".into(), len: 2 });
    }

    /// What the core rejects is data, and a field it ignores does not show.
    #[test]
    fn an_unassigned_word_is_data_and_ignored_fields_vanish() {
        for w in [0x00c0u16, 0x0150, 0x0940, 0x0b00, 0x0c10, 0x0d0f, 0x0fff] {
            assert_eq!(dis(&[w]), Disassembly { text: format!("DATA X'{w:X}'"), len: 1 }, "{w:04x}");
        }
        assert_eq!(text(&[0x0005]), "HLT");
        assert_eq!(text(&[0x0127]), "INV");
        assert_eq!(text(&[0x08b3]), "SSE");
    }

    /// `DATA` is exactly what the core refuses to execute: over every
    /// generic (opcode 0), the only words that are not all instructions.
    #[test]
    fn data_is_what_the_core_rejects() {
        use crate::bus::Bus;
        use crate::cpu::testbus::TestBus;
        use crate::cpu::{Cpu, StepResult};
        let mut bus = TestBus::new();
        for insn in 0..0x1000u16 {
            bus.write8(0x80, (insn >> 8) as u8);
            bus.write8(0x81, insn as u8);
            let mut cpu = crate::cpu::ray703::Cpu703::new();
            cpu.set_register("PC", 0x40);
            let bad = cpu.step(&mut bus) == StepResult::BadOpcode;
            let d = disassemble(&|a| bus.mem.get(a as usize).copied(), 0x40).unwrap();
            assert_eq!(d.text.starts_with("DATA"), bad, "{insn:04x}: {}", d.text);
        }
    }

    #[test]
    fn a_word_that_is_not_memory_is_none() {
        assert_eq!(disassemble(&|_| None, 0), None);
        let half = [0x0bu8, 0x0f];
        assert_eq!(disassemble(&|a| half.get(a as usize).copied(), 0), None, "MPY's second word");
    }
}
