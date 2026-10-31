// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 6800 disassembler, in ASxxxx's `as6800` syntax: Motorola's
//! mnemonics (`ldaa`, `oraa`), `0x` hex, `*` for direct page, `,x` for an
//! indexed operand with no offset, a branch operand as its target address.
//!
//! Written from the datasheet's opcode map rather than the core's table,
//! so the two check each other (`data_is_what_the_core_rejects`). An
//! opcode the core rejects is `.db`.

use super::Disassembly;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Mode {
    Inherent,
    Imm8,
    Imm16,
    Direct,
    Indexed,
    Extended,
    Rel,
}

use Mode::*;

const BRANCHES: [&str; 16] =
    ["bra", "", "bhi", "bls", "bcc", "bcs", "bne", "beq", "bvc", "bvs", "bpl", "bmi", "bge", "blt", "bgt", "ble"];
/// 0x40-0x7f by the low nibble; "" is no instruction.
const MEMORY: [&str; 16] =
    ["neg", "", "", "com", "lsr", "", "ror", "asr", "asl", "rol", "dec", "", "inc", "tst", "jmp", "clr"];
/// 0x80-0xbf; "" is no instruction in any column.
const A_GROUP: [&str; 16] =
    ["suba", "cmpa", "sbca", "", "anda", "bita", "ldaa", "staa", "eora", "adca", "oraa", "adda", "cpx", "jsr", "lds", "sts"];
const B_GROUP: [&str; 16] =
    ["subb", "cmpb", "sbcb", "", "andb", "bitb", "ldab", "stab", "eorb", "adcb", "orab", "addb", "", "", "ldx", "stx"];

fn inherent(op: u8) -> Option<&'static str> {
    Some(match op {
        0x01 => "nop",
        0x06 => "tap",
        0x07 => "tpa",
        0x08 => "inx",
        0x09 => "dex",
        0x0a => "clv",
        0x0b => "sev",
        0x0c => "clc",
        0x0d => "sec",
        0x0e => "cli",
        0x0f => "sei",
        0x10 => "sba",
        0x11 => "cba",
        0x16 => "tab",
        0x17 => "tba",
        0x19 => "daa",
        0x1b => "aba",
        0x30 => "tsx",
        0x31 => "ins",
        0x32 => "pula",
        0x33 => "pulb",
        0x34 => "des",
        0x35 => "txs",
        0x36 => "psha",
        0x37 => "pshb",
        0x39 => "rts",
        0x3b => "rti",
        0x3e => "wai",
        0x3f => "swi",
        _ => return None,
    })
}

fn decode(op: u8) -> Option<(String, Mode)> {
    let lo = (op & 0xf) as usize;
    if let Some(name) = inherent(op) {
        return Some((name.into(), Inherent));
    }
    let (name, mode) = match op >> 4 {
        0x2 => (BRANCHES[lo].to_string(), Rel),
        0x4 | 0x5 if lo != 0xe && !MEMORY[lo].is_empty() => {
            (format!("{}{}", MEMORY[lo], if op < 0x50 { 'a' } else { 'b' }), Inherent)
        }
        0x6 => (MEMORY[lo].to_string(), Indexed),
        0x7 => (MEMORY[lo].to_string(), Extended),
        hi @ 0x8..=0xf => {
            let name = if hi < 0xc { A_GROUP[lo] } else { B_GROUP[lo] };
            let mode = match hi & 3 {
                0 => match lo {
                    // no store immediate, and bsr sits in jsr's column
                    0x7 | 0xf => return None,
                    0xd if hi == 0x8 => return Some(("bsr".into(), Rel)),
                    0xd => return None,
                    0xc | 0xe => Imm16,
                    _ => Imm8,
                },
                // jsr has no direct form before the 6801
                1 if lo == 0xd => return None,
                1 => Direct,
                2 => Indexed,
                _ => Extended,
            };
            (name.to_string(), mode)
        }
        _ => return None,
    };
    (!name.is_empty()).then_some((name, mode))
}

/// The instruction at `addr`.
pub fn disassemble(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<Disassembly> {
    let addr = addr as u16;
    let at = |i: u16| peek(addr.wrapping_add(i) as u32);
    let first = at(0)?;
    let Some((name, mode)) = decode(first) else {
        return Some(Disassembly { text: format!(".db 0x{first:02x}"), len: 1 });
    };
    let word = || Some(u16::from_be_bytes([at(1)?, at(2)?]));
    let (operand, len) = match mode {
        Inherent => (String::new(), 1),
        Imm8 => (format!("#0x{:02x}", at(1)?), 2),
        Imm16 => (format!("#0x{:04x}", word()?), 3),
        Direct => (format!("*0x{:02x}", at(1)?), 2),
        Indexed => match at(1)? {
            0 => (",x".to_string(), 2),
            off => (format!("0x{off:02x},x"), 2),
        },
        Extended => (format!("0x{:04x}", word()?), 3),
        Rel => (format!("0x{:04x}", addr.wrapping_add(2).wrapping_add(at(1)? as i8 as u16)), 2),
    };
    let text = if operand.is_empty() { name } else { format!("{name} {operand}") };
    Some(Disassembly { text, len })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dis_at(at: u16, bytes: &[u8]) -> Disassembly {
        disassemble(&|a| a.checked_sub(at as u32).and_then(|i| bytes.get(i as usize)).copied(), at as u32).unwrap()
    }

    fn text(bytes: &[u8]) -> String {
        dis_at(0x0200, bytes).text
    }

    #[test]
    fn each_mode_reads_as_as6800_writes_it() {
        assert_eq!(dis_at(0x0200, &[0x86, 0x03]), Disassembly { text: "ldaa #0x03".into(), len: 2 });
        assert_eq!(text(&[0x8e, 0x7f, 0x00]), "lds #0x7f00");
        assert_eq!(text(&[0xde, 0x50]), "ldx *0x50");
        assert_eq!(text(&[0xa6, 0x00]), "ldaa ,x");
        assert_eq!(text(&[0xe7, 0x44]), "stab 0x44,x");
        assert_eq!(text(&[0xb7, 0xf0, 0x01]), "staa 0xf001");
        assert_eq!(text(&[0x20, 0xfe]), "bra 0x0200");
        assert_eq!(text(&[0x8d, 0x10]), "bsr 0x0212");
        assert_eq!(text(&[0xbd, 0xff, 0x00]), "jsr 0xff00");
        assert_eq!(text(&[0x7c, 0x00, 0x50]), "inc 0x0050");
        assert_eq!(text(&[0x4f]), "clra");
        assert_eq!(text(&[0x3b]), "rti");
        assert_eq!(text(&[0x0e]), "cli");
    }

    #[test]
    fn what_is_no_instruction_is_a_byte_of_data() {
        for b in [0x00u8, 0x02, 0x21, 0x41, 0x4e, 0x87, 0x8f, 0x9d, 0xcd, 0xdc] {
            assert_eq!(dis_at(0x0200, &[b, 0, 0]), Disassembly { text: format!(".db 0x{b:02x}"), len: 1 }, "{b:02x}");
        }
    }

    /// Run `bytes` at 0x0200 on the core: whether it was refused, and how
    /// far the PC went.
    fn run_on_core(bytes: &[u8]) -> (bool, u16) {
        use crate::cpu::testbus::TestBus;
        use crate::cpu::{Cpu, StepResult};
        let mut bus = TestBus::new();
        bus.load(0x0200, bytes);
        let mut cpu = crate::cpu::m6800::Cpu6800::new();
        for (r, v) in [("PC", 0x0200), ("S", 0x7f00), ("X", 0x2000)] {
            assert!(cpu.set_register(r, v));
        }
        let bad = cpu.step(&mut bus) == StepResult::BadOpcode;
        (bad, (cpu.pc() as u16).wrapping_sub(0x0200))
    }

    /// Over every opcode: `.db` is exactly what the core refuses, and the
    /// length is how far the core's PC goes, for every instruction that
    /// does not branch.
    #[test]
    fn data_is_what_the_core_rejects_and_lengths_are_its_fetches() {
        const FLOW: &[&str] = &["jmp", "jsr", "bsr", "rts", "rti", "swi", "wai"];
        let mut compared = 0;
        for op in 0..=0xffu8 {
            let bytes = [op, 0x10, 0x00];
            let d = dis_at(0x0200, &bytes);
            let (bad, advance) = run_on_core(&bytes);
            assert_eq!(d.text.starts_with(".db"), bad, "{op:02x}: {}", d.text);
            let name = d.text.split(' ').next().unwrap();
            let branch = BRANCHES.contains(&name) || name == "bsr";
            if !bad && !branch && !FLOW.contains(&name) {
                assert_eq!(advance as u32, d.len, "{op:02x}: {}", d.text);
                compared += 1;
            }
        }
        assert!(compared > 170, "only {compared} lengths compared");
    }

    /// ASxxxx's own test of as6800, which the distribution ships and this
    /// tree does not (`tools/fetch-asxxxx.py` puts it in `tests/data/`):
    /// every instruction line disassembles to what its source says, the
    /// two-word spellings (`lda a`) folded into one and the aliases mapped
    /// to the names the disassembler uses. `cargo test -- --ignored t6800`.
    #[test]
    #[ignore]
    fn t6800_asm_disassembles_to_its_own_source() {
        use crate::cpu::asxxxx_check::{cases, normalise, symbols};
        const ALIASES: &[(&str, &str)] =
            &[("bhs", "bcc"), ("blo", "bcs"), ("lsl", "asl"), ("lsla", "asla"), ("lslb", "aslb")];
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/asxxxx/t6800.asm");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}; run tools/fetch-asxxxx.py"));
        let mut symbols = symbols(&text);
        // a label in the direct page area, past its .blkb
        symbols.push(("dir".into(), 0x33));
        let all = cases(&text, &|_| false);
        let mut bad = Vec::new();
        for c in &all {
            let d = disassemble(&|a| a.checked_sub(c.addr as u32).and_then(|i| c.bytes.get(i as usize)).copied(), c.addr as u32)
                .unwrap();
            // `adc a #1` is `adca #1`, `psh a` is `psha`
            let words: Vec<&str> = c.source.split_whitespace().collect();
            let source = match words.as_slice() {
                [op, r @ ("a" | "b"), rest @ ..] => format!("{op}{r} {}", rest.join(" ")),
                _ => c.source.clone(),
            };
            let (mut name, mut operand) = normalise(&source, c.addr, &symbols);
            if let Some(&(_, canon)) = ALIASES.iter().find(|(a, _)| *a == name) {
                name = canon.to_string();
            }
            // a bare address in the direct page, which `.setdp` makes direct
            if matches!(c.bytes[0] >> 4, 0x9 | 0xd) && !operand.starts_with('*') {
                operand.insert(0, '*');
            }
            let got = normalise(&d.text, c.addr, &symbols);
            if (name, operand) != got || d.len as usize != c.bytes.len() {
                bad.push(format!("line {}: {} ({:02x?}) disassembles to {}", c.line, c.source, c.bytes, d.text));
            }
        }
        assert!(bad.is_empty(), "{} of {} lines disagree:\n{}", bad.len(), all.len(), bad.join("\n"));
        assert!(all.len() > 400, "only {} lines compared", all.len());
    }

    #[test]
    fn a_byte_that_is_not_memory_is_none() {
        assert_eq!(disassemble(&|_| None, 0), None);
        assert_eq!(disassemble(&|a| (a == 0).then_some(0xb6), 0), None, "the operand is not there");
    }
}
