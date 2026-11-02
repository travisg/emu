// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 8080 disassembler: Intel's mnemonics in ASxxxx's `as8085` spelling
//! (in its `.8080` mode) -- lower case, `#` before an immediate, `0x` hex,
//! `rst #n` by its number.
//!
//! Written from the manual's opcode table rather than shared with the
//! core's decode, so the two check each other (`lengths_are_the_cores_
//! fetches`). Every byte is an instruction on the 8080, so there is no data
//! directive: the holes in the map read as the instructions they run as --
//! `nop` for `08`..`38`, `jmp` for `CB`, `ret` for `D9`, `call` for `DD`,
//! `ED` and `FD`.

use super::Disassembly;

const R: [&str; 8] = ["b", "c", "d", "e", "h", "l", "m", "a"];
const RP: [&str; 4] = ["b", "d", "h", "sp"];
const RP_PUSH: [&str; 4] = ["b", "d", "h", "psw"];
const CC: [&str; 8] = ["nz", "z", "nc", "c", "po", "pe", "p", "m"];
const ALU: [&str; 8] = ["add", "adc", "sub", "sbb", "ana", "xra", "ora", "cmp"];
const ALU_IMM: [&str; 8] = ["adi", "aci", "sui", "sbi", "ani", "xri", "ori", "cpi"];
const ACC: [&str; 8] = ["rlc", "rrc", "ral", "rar", "daa", "cma", "stc", "cmc"];

/// The instruction at `addr`.
pub fn disassemble(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<Disassembly> {
    let at = addr as u16;
    let byte = |i: u16| peek(at.wrapping_add(i) as u32);
    let op = byte(0)?;
    let n = || Some(format!("#0x{:02x}", byte(1)?));
    let nn = || Some(format!("0x{:04x}", u16::from_le_bytes([byte(1)?, byte(2)?])));
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let (p, q) = ((y >> 1) as usize, y & 1);
    let (y, z) = (y as usize, z as usize);
    let (text, len) = match (x, z) {
        (0, 0) => ("nop".to_string(), 1),
        (0, 1) if q == 0 => (format!("lxi {},#{}", RP[p], nn()?), 3),
        (0, 1) => (format!("dad {}", RP[p]), 1),
        (0, 2) => match (p, q) {
            (0 | 1, 0) => (format!("stax {}", RP[p]), 1),
            (0 | 1, _) => (format!("ldax {}", RP[p]), 1),
            (2, 0) => (format!("shld {}", nn()?), 3),
            (2, _) => (format!("lhld {}", nn()?), 3),
            (_, 0) => (format!("sta {}", nn()?), 3),
            _ => (format!("lda {}", nn()?), 3),
        },
        (0, 3) => (format!("{} {}", if q == 0 { "inx" } else { "dcx" }, RP[p]), 1),
        (0, 4) => (format!("inr {}", R[y]), 1),
        (0, 5) => (format!("dcr {}", R[y]), 1),
        (0, 6) => (format!("mvi {},{}", R[y], n()?), 2),
        (0, _) => (ACC[y].to_string(), 1),
        (1, 6) if y == 6 => ("hlt".to_string(), 1),
        (1, _) => (format!("mov {},{}", R[y], R[z]), 1),
        (2, _) => (format!("{} {}", ALU[y], R[z]), 1),
        (3, 0) => (format!("r{}", CC[y]), 1),
        (3, 1) => match (q, p) {
            (0, _) => (format!("pop {}", RP_PUSH[p]), 1),
            (_, 0 | 1) => ("ret".to_string(), 1),
            (_, 2) => ("pchl".to_string(), 1),
            _ => ("sphl".to_string(), 1),
        },
        (3, 2) => (format!("j{} {}", CC[y], nn()?), 3),
        (3, 3) => match y {
            0 | 1 => (format!("jmp {}", nn()?), 3),
            2 => (format!("out {}", n()?), 2),
            3 => (format!("in {}", n()?), 2),
            4 => ("xthl".to_string(), 1),
            5 => ("xchg".to_string(), 1),
            6 => ("di".to_string(), 1),
            _ => ("ei".to_string(), 1),
        },
        (3, 4) => (format!("c{} {}", CC[y], nn()?), 3),
        (3, 5) if q == 0 => (format!("push {}", RP_PUSH[p]), 1),
        (3, 5) => (format!("call {}", nn()?), 3),
        (3, 6) => (format!("{} {}", ALU_IMM[y], n()?), 2),
        _ => (format!("rst #{y}"), 1),
    };
    Some(Disassembly { text, len })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dis_at(at: u16, bytes: &[u8]) -> Disassembly {
        disassemble(&|a| a.checked_sub(at as u32).and_then(|i| bytes.get(i as usize)).copied(), at as u32).unwrap()
    }

    fn text(bytes: &[u8]) -> String {
        dis_at(0x0100, bytes).text
    }

    #[test]
    fn each_shape_reads_as_as8085_writes_it() {
        assert_eq!(dis_at(0x0100, &[0x21, 0x34, 0x12]), Disassembly { text: "lxi h,#0x1234".into(), len: 3 });
        assert_eq!(text(&[0x3e, 0x20]), "mvi a,#0x20");
        assert_eq!(text(&[0x36, 0x2c]), "mvi m,#0x2c");
        assert_eq!(text(&[0x4e]), "mov c,m");
        assert_eq!(text(&[0x76]), "hlt");
        assert_eq!(text(&[0x86]), "add m");
        assert_eq!(text(&[0xfe, 0x0d]), "cpi #0x0d");
        assert_eq!(text(&[0xd3, 0x10]), "out #0x10");
        assert_eq!(text(&[0xdb, 0x11]), "in #0x11");
        assert_eq!(text(&[0xc2, 0x00, 0x10]), "jnz 0x1000");
        assert_eq!(text(&[0xfc, 0x00, 0x10]), "cm 0x1000");
        assert_eq!(text(&[0xe8]), "rpe");
        assert_eq!(text(&[0xf5]), "push psw");
        assert_eq!(text(&[0xc1]), "pop b");
        assert_eq!(text(&[0x3a, 0x00, 0x20]), "lda 0x2000");
        assert_eq!(text(&[0x12]), "stax d");
        assert_eq!(text(&[0x39]), "dad sp");
        assert_eq!(text(&[0xdf]), "rst #3");
        assert_eq!(text(&[0xe6, 0x7f]), "ani #0x7f");
    }

    #[test]
    fn the_holes_read_as_the_instructions_they_run_as() {
        assert_eq!(text(&[0x08]), "nop");
        assert_eq!(text(&[0x38]), "nop");
        assert_eq!(text(&[0xcb, 0x00, 0x20]), "jmp 0x2000");
        assert_eq!(text(&[0xd9]), "ret");
        for op in [0xdd, 0xed, 0xfd] {
            assert_eq!(dis_at(0x0100, &[op, 0x00, 0x20]), Disassembly { text: "call 0x2000".into(), len: 3 });
        }
    }

    /// Over every opcode: the length is how far the core's PC goes in one
    /// step, for every instruction that does not jump or wait.
    #[test]
    fn lengths_are_the_cores_fetches() {
        use crate::cpu::testbus::TestBus;
        use crate::cpu::Cpu;
        const FLOW: &[&str] = &["jmp", "call", "ret", "rst", "pchl", "hlt"];
        let mut compared = 0;
        let mut bus = TestBus::new();
        for op in 0..=0xffu8 {
            let bytes = [op, 0x05, 0x00];
            let d = dis_at(0x0100, &bytes);
            bus.load(0x0100, &bytes);
            let mut cpu = crate::cpu::i8080::Cpu8080::new();
            for (r, v) in [("PC", 0x0100), ("SP", 0x8000), ("HL", 0x4000)] {
                assert!(cpu.set_register(r, v));
            }
            cpu.step(&mut bus);
            let name = d.text.split(' ').next().unwrap();
            let flow = FLOW.contains(&name)
                || (name.len() <= 3 && b"jcr".contains(&name.as_bytes()[0]) && CC.contains(&&name[1..]));
            if !flow {
                assert_eq!((cpu.pc() as u16).wrapping_sub(0x0100) as u32, d.len, "{op:02x}: {}", d.text);
                compared += 1;
            }
        }
        assert!(compared > 190, "only {compared} lengths compared");
    }

    /// ASxxxx's own test of as8085's 8080 mode, which the distribution
    /// ships and this tree does not (`tools/fetch-asxxxx.py` puts it in
    /// `tests/data/`): every 8080 instruction line disassembles to what its
    /// source says. The file's 8085-only lines carry the assembler's `o`
    /// error flag in front of their bytes and are not cases, and nor is its
    /// `.word`. `cargo test -- --ignored t8080`.
    #[test]
    #[ignore]
    fn t8080_asm_disassembles_to_its_own_source() {
        use crate::cpu::asxxxx_check::{cases, normalise, symbols};
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/asxxxx/t8080.asm");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}; run tools/fetch-asxxxx.py"));
        let symbols = symbols(&text);
        let spell = |s: &str, here: u16| {
            let (name, op) = normalise(s, here, &symbols);
            (name, op.replace('#', ""))
        };
        let all = cases(&text, &|code| code.contains(".word"));
        let mut bad = Vec::new();
        for c in &all {
            let d = disassemble(&|a| a.checked_sub(c.addr as u32).and_then(|i| c.bytes.get(i as usize)).copied(), c.addr as u32)
                .unwrap();
            if spell(&c.source, c.addr) != spell(&d.text, c.addr) || d.len as usize != c.bytes.len() {
                bad.push(format!("line {}: {} ({:02x?}) disassembles to {}", c.line, c.source, c.bytes, d.text));
            }
        }
        assert!(bad.is_empty(), "{} of {} lines disagree:\n{}", bad.len(), all.len(), bad.join("\n"));
        assert!(all.len() > 90, "only {} lines compared", all.len());
    }

    #[test]
    fn a_byte_that_is_not_memory_is_none() {
        assert_eq!(disassemble(&|_| None, 0), None);
        assert_eq!(disassemble(&|a| (a == 0).then_some(0x21), 0), None, "the operand is not there");
    }
}
