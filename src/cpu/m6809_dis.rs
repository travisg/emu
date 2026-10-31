// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 6809 disassembler, in ASxxxx's `as6809` syntax -- the assembler
//! `test/makefile` builds the 6809 guests with: lower case, `0x` hex, `*`
//! for direct page, a branch or `,pcr` operand as its target address.
//!
//! Written from the datasheet's opcode map rather than the core's table,
//! so the two check each other (`data_is_what_the_core_rejects`). Where the
//! core gives an illegal postbyte a meaning -- `[,x+]` runs as `,x+`, and
//! `[n]`'s register bits are ignored -- the text is what it runs as. An
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
    Rel8,
    Rel16,
    /// TFR/EXG's register pair.
    Pair,
    /// PSHS/PULS, and PSHU/PULU, whose bit 6 is the other stack.
    StackS,
    StackU,
}

use Mode::*;

const BRANCHES: [&str; 16] =
    ["bra", "brn", "bhi", "bls", "bcc", "bcs", "bne", "beq", "bvc", "bvs", "bpl", "bmi", "bge", "blt", "bgt", "ble"];
/// 0x00-0x0f, and the same at 0x60/0x70; "" is no instruction.
const MEMORY: [&str; 16] =
    ["neg", "", "", "com", "lsr", "", "ror", "asr", "asl", "rol", "dec", "", "inc", "tst", "jmp", "clr"];
const A_GROUP: [&str; 16] = [
    "suba", "cmpa", "sbca", "subd", "anda", "bita", "lda", "sta", "eora", "adca", "ora", "adda", "cmpx", "jsr", "ldx",
    "stx",
];
const B_GROUP: [&str; 16] = [
    "subb", "cmpb", "sbcb", "addd", "andb", "bitb", "ldb", "stb", "eorb", "adcb", "orb", "addb", "ldd", "std", "ldu",
    "stu",
];
/// TFR/EXG's register codes; the gaps are no register.
const PAIR: [&str; 16] = ["d", "x", "y", "u", "s", "pc", "?", "?", "a", "b", "cc", "dp", "?", "?", "?", "?"];

/// The mode of a column of the A and B groups, by the opcode's high nibble.
fn group_mode(op: u8, wide: bool) -> Mode {
    match op >> 4 {
        0x8 | 0xc if wide => Imm16,
        0x8 | 0xc => Imm8,
        0x9 | 0xd => Direct,
        0xa | 0xe => Indexed,
        _ => Extended,
    }
}

fn page0(op: u8) -> Option<(String, Mode)> {
    let lo = (op & 0xf) as usize;
    let named = |n: &str, m| Some((n.to_string(), m));
    match op {
        0x00..=0x0f | 0x60..=0x7f if !MEMORY[lo].is_empty() => {
            let mode = match op >> 4 {
                0x0 => Direct,
                0x6 => Indexed,
                _ => Extended,
            };
            named(MEMORY[lo], mode)
        }
        0x40..=0x5f if !MEMORY[lo].is_empty() && lo != 0xe => {
            let reg = if op < 0x50 { "a" } else { "b" };
            Some((format!("{}{reg}", MEMORY[lo]), Inherent))
        }
        0x12 => named("nop", Inherent),
        0x13 => named("sync", Inherent),
        0x16 => named("lbra", Rel16),
        0x17 => named("lbsr", Rel16),
        0x19 => named("daa", Inherent),
        0x1a => named("orcc", Imm8),
        0x1c => named("andcc", Imm8),
        0x1d => named("sex", Inherent),
        0x1e => named("exg", Pair),
        0x1f => named("tfr", Pair),
        0x20..=0x2f => named(BRANCHES[lo], Rel8),
        0x30 => named("leax", Indexed),
        0x31 => named("leay", Indexed),
        0x32 => named("leas", Indexed),
        0x33 => named("leau", Indexed),
        0x34 => named("pshs", StackS),
        0x35 => named("puls", StackS),
        0x36 => named("pshu", StackU),
        0x37 => named("pulu", StackU),
        0x39 => named("rts", Inherent),
        0x3a => named("abx", Inherent),
        0x3b => named("rti", Inherent),
        0x3c => named("cwai", Imm8),
        0x3d => named("mul", Inherent),
        0x3f => named("swi", Inherent),
        // no store immediate
        0x87 | 0x8f | 0xc7 | 0xcd | 0xcf => None,
        0x8d => named("bsr", Rel8),
        0x80..=0xbf => named(A_GROUP[lo], group_mode(op, matches!(lo, 0x3 | 0xc | 0xe))),
        0xc0..=0xff => named(B_GROUP[lo], group_mode(op, matches!(lo, 0x3 | 0xc | 0xe))),
        _ => None,
    }
}

/// The 0x10 page.
fn page2(op: u8) -> Option<(String, Mode)> {
    let lo = (op & 0xf) as usize;
    let wide = |n: &str| Some((n.to_string(), group_mode(op, true)));
    match op {
        0x21..=0x2f => Some((format!("l{}", BRANCHES[lo]), Rel16)),
        0x3f => Some(("swi2".into(), Inherent)),
        0x83 | 0x93 | 0xa3 | 0xb3 => wide("cmpd"),
        0x8c | 0x9c | 0xac | 0xbc => wide("cmpy"),
        0x8e | 0x9e | 0xae | 0xbe => wide("ldy"),
        0x9f | 0xaf | 0xbf => wide("sty"),
        0xce | 0xde | 0xee | 0xfe => wide("lds"),
        0xdf | 0xef | 0xff => wide("sts"),
        _ => None,
    }
}

/// The 0x11 page.
fn page3(op: u8) -> Option<(String, Mode)> {
    let wide = |n: &str| Some((n.to_string(), group_mode(op, true)));
    match op {
        0x3f => Some(("swi3".into(), Inherent)),
        0x83 | 0x93 | 0xa3 | 0xb3 => wide("cmpu"),
        0x8c | 0x9c | 0xac | 0xbc => wide("cmps"),
        _ => None,
    }
}

fn signed_hex(v: i32) -> String {
    if v < 0 {
        format!("-0x{:02x}", -v)
    } else {
        format!("0x{v:02x}")
    }
}

/// An indexed operand from its postbyte, reading any offset through
/// `next`; `end` is the address past the whole instruction, which a
/// `,pcr` offset counts from, once `next` has read everything.
fn indexed(pb: u8, next: &mut dyn FnMut() -> Option<u8>, len_so_far: &dyn Fn() -> u16) -> Option<String> {
    let reg = ["x", "y", "u", "s"][((pb >> 5) & 3) as usize];
    if pb & 0x80 == 0 {
        let off = ((pb & 0x1f) as i8) << 3 >> 3;
        return Some(format!("{off},{reg}"));
    }
    let indirect = pb & 0x10 != 0;
    let inner = match pb & 0xf {
        // no indirect form: the core runs these as the direct one
        0x0 => return Some(format!(",{reg}+")),
        0x2 => return Some(format!(",-{reg}")),
        0x1 => format!(",{reg}++"),
        0x3 => format!(",--{reg}"),
        0x4 => format!(",{reg}"),
        0x5 => format!("b,{reg}"),
        0x6 => format!("a,{reg}"),
        0x8 => format!("{},{reg}", signed_hex(next()? as i8 as i32)),
        0x9 => format!("0x{:04x},{reg}", u16::from_be_bytes([next()?, next()?])),
        0xb => format!("d,{reg}"),
        0xc => {
            let off = next()? as i8 as u16;
            format!("0x{:04x},pcr", len_so_far().wrapping_add(off))
        }
        0xd => {
            let off = u16::from_be_bytes([next()?, next()?]);
            format!("0x{:04x},pcr", len_so_far().wrapping_add(off))
        }
        // [n]: indirect whatever the bit says, the register ignored
        0xf => return Some(format!("[0x{:04x}]", u16::from_be_bytes([next()?, next()?]))),
        _ => return None,
    };
    Some(if indirect { format!("[{inner}]") } else { inner })
}

fn stack_list(pb: u8, other: &str) -> String {
    let names = ["cc", "a", "b", "dp", "x", "y", other, "pc"];
    names.iter().enumerate().filter(|(i, _)| pb & (1 << i) != 0).map(|(_, n)| *n).collect::<Vec<_>>().join(",")
}

/// The instruction at `addr`.
pub fn disassemble(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<Disassembly> {
    let addr = addr as u16;
    let pos = std::cell::Cell::new(addr);
    let mut next = || {
        let b = peek(pos.get() as u32);
        pos.set(pos.get().wrapping_add(1));
        b
    };
    let first = next()?;
    let decoded = match first {
        0x10 => page2(next()?),
        0x11 => page3(next()?),
        op => page0(op),
    };
    let data = || Some(Disassembly { text: format!(".db 0x{first:02x}"), len: 1 });
    let Some((name, mode)) = decoded else {
        return data();
    };
    let here = || pos.get();
    let operand = match mode {
        Inherent => String::new(),
        Imm8 => format!("#0x{:02x}", next()?),
        Imm16 => format!("#0x{:04x}", u16::from_be_bytes([next()?, next()?])),
        Direct => format!("*0x{:02x}", next()?),
        Extended => format!("0x{:04x}", u16::from_be_bytes([next()?, next()?])),
        Indexed => {
            let pb = next()?;
            match indexed(pb, &mut next, &here) {
                Some(op) => op,
                None => return data(),
            }
        }
        Rel8 => {
            let off = next()? as i8 as u16;
            format!("0x{:04x}", here().wrapping_add(off))
        }
        Rel16 => {
            let off = u16::from_be_bytes([next()?, next()?]);
            format!("0x{:04x}", here().wrapping_add(off))
        }
        Pair => {
            let pb = next()?;
            format!("{},{}", PAIR[(pb >> 4) as usize], PAIR[(pb & 0xf) as usize])
        }
        StackS => stack_list(next()?, "u"),
        StackU => stack_list(next()?, "s"),
    };
    let text = if operand.is_empty() { name } else { format!("{name} {operand}") };
    Some(Disassembly { text, len: pos.get().wrapping_sub(addr) as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dis_at(at: u16, bytes: &[u8]) -> Disassembly {
        disassemble(&|a| a.checked_sub(at as u32).and_then(|i| bytes.get(i as usize)).copied(), at as u32).unwrap()
    }

    fn text(bytes: &[u8]) -> String {
        dis_at(0x1000, bytes).text
    }

    #[test]
    fn each_mode_reads_as_as6809_writes_it() {
        assert_eq!(dis_at(0x1000, &[0x86, 0x15]), Disassembly { text: "lda #0x15".into(), len: 2 });
        assert_eq!(text(&[0xcc, 0x00, 0x19]), "ldd #0x0019");
        assert_eq!(text(&[0x96, 0x16]), "lda *0x16");
        assert_eq!(text(&[0xb6, 0xf0, 0x01]), "lda 0xf001");
        assert_eq!(text(&[0x10, 0xde, 0x1a]), "lds *0x1a");
        assert_eq!(text(&[0x11, 0x83, 0x00, 0x0f]), "cmpu #0x000f");
        assert_eq!(text(&[0x20, 0x10]), "bra 0x1012");
        assert_eq!(text(&[0x20, 0xfe]), "bra 0x1000");
        assert_eq!(text(&[0x10, 0x27, 0x00, 0x10]), "lbeq 0x1014");
        assert_eq!(text(&[0x17, 0xff, 0xfd]), "lbsr 0x1000");
        assert_eq!(text(&[0x1f, 0x12]), "tfr x,y");
        assert_eq!(text(&[0x1e, 0x8a]), "exg a,cc");
        assert_eq!(text(&[0x34, 0xff]), "pshs cc,a,b,dp,x,y,u,pc");
        assert_eq!(text(&[0x37, 0x40]), "pulu s");
        assert_eq!(text(&[0x3f]), "swi");
        assert_eq!(text(&[0x11, 0x3f]), "swi3");
        assert_eq!(text(&[0x4f]), "clra");
        assert_eq!(text(&[0x3c, 0xef]), "cwai #0xef");
    }

    #[test]
    fn every_indexed_shape() {
        assert_eq!(text(&[0x60, 0x1f]), "neg -1,x");
        assert_eq!(text(&[0x60, 0x2f]), "neg 15,y");
        assert_eq!(text(&[0x60, 0xe1]), "neg ,s++");
        assert_eq!(text(&[0x60, 0xc2]), "neg ,-u");
        assert_eq!(text(&[0x60, 0x85]), "neg b,x");
        assert_eq!(text(&[0x60, 0x88, 0xf0]), "neg -0x10,x");
        assert_eq!(text(&[0x60, 0x89, 0x22, 0x33]), "neg 0x2233,x");
        assert_eq!(text(&[0x60, 0x8b]), "neg d,x");
        assert_eq!(text(&[0x60, 0x8c, 0x10]), "neg 0x1013,pcr");
        assert_eq!(dis_at(0x1000, &[0x60, 0x9d, 0x10, 0x00]), Disassembly { text: "neg [0x2004,pcr]".into(), len: 4 });
        assert_eq!(text(&[0x60, 0x93]), "neg [,--x]");
        assert_eq!(text(&[0x60, 0x9f, 0x22, 0x33]), "neg [0x2233]");
        assert_eq!(text(&[0x10, 0xae, 0x98, 0x02]), "ldy [0x02,x]");
    }

    #[test]
    fn what_is_no_instruction_is_a_byte_of_data() {
        for bytes in [&[0x01u8][..], &[0x87], &[0x10, 0x20], &[0x11, 0x8e], &[0x4e], &[0x60, 0x87]] {
            assert_eq!(dis_at(0x1000, bytes), Disassembly { text: format!(".db 0x{:02x}", bytes[0]), len: 1 });
        }
    }

    /// Run `bytes` at 0x1000 on the core: whether it was refused, and how
    /// far the PC went.
    fn run_on_core(bytes: &[u8]) -> (bool, u16) {
        use crate::cpu::testbus::TestBus;
        use crate::cpu::{Cpu, StepResult};
        let mut bus = TestBus::new();
        bus.load(0x1000, bytes);
        let mut cpu = crate::cpu::m6809::Cpu6809::new();
        for (r, v) in [("PC", 0x1000), ("S", 0x8000), ("U", 0x9000), ("X", 0x2000), ("Y", 0x3000)] {
            assert!(cpu.set_register(r, v));
        }
        let bad = cpu.step(&mut bus) == StepResult::BadOpcode;
        (bad, (cpu.pc() as u16).wrapping_sub(0x1000))
    }

    /// Over every opcode of all three pages, and every postbyte under an
    /// indexed `lda` and `ldy`: `.db` is exactly what the core refuses, and
    /// the length is how far the core's PC goes, for every instruction that
    /// does not branch.
    #[test]
    fn data_is_what_the_core_rejects_and_lengths_are_its_fetches() {
        const FLOW: &[&str] = &[
            "jmp", "jsr", "bsr", "lbra", "lbsr", "rts", "rti", "swi", "swi2", "swi3", "cwai", "sync", "puls", "pulu",
        ];
        let mut cases: Vec<Vec<u8>> = Vec::new();
        for prefix in [None, Some(0x10u8), Some(0x11)] {
            for op in 0..=0xffu8 {
                let mut b: Vec<u8> = prefix.into_iter().collect();
                b.extend([op, 0x84, 0x00, 0x00, 0x00]);
                cases.push(b);
            }
        }
        for pb in 0..=0xffu8 {
            cases.push(vec![0xa6, pb, 0x00, 0x00]);
            cases.push(vec![0x10, 0xae, pb, 0x00, 0x00]);
        }
        let mut compared = 0;
        for bytes in cases {
            let d = dis_at(0x1000, &bytes);
            let (bad, advance) = run_on_core(&bytes);
            assert_eq!(d.text.starts_with(".db"), bad, "{bytes:02x?}: {}", d.text);
            let name = d.text.split(' ').next().unwrap();
            let branch = (name.starts_with('b') && name != "bita" && name != "bitb")
                || (name.starts_with("lb") && name.len() == 4);
            if !bad && !branch && !FLOW.contains(&name) {
                assert_eq!(advance as u32, d.len, "{bytes:02x?}: {}", d.text);
                compared += 1;
            }
        }
        assert!(compared > 650, "only {compared} lengths compared");
    }

    /// `t6809.asm`, ASxxxx's own test of as6809: every line that is one
    /// instruction disassembles to what its source says. The 6800
    /// compatibility forms expand to other instructions and are skipped,
    /// and the aliases name the opcode the disassembler calls otherwise.
    #[test]
    fn t6809_asm_disassembles_to_its_own_source() {
        use crate::cpu::asxxxx_check::{cases, normalise, symbols};
        const COMPAT: &[&str] = &[
            "aba", "cba", "clc", "cli", "clv", "des", "dex", "ins", "inx", "psha", "pshb", "pula", "pulb", "sba", "sec",
            "sei", "sev", "tab", "tap", "tba", "tpa", "tsx", "txs", "wai",
        ];
        const ALIASES: &[(&str, &str)] = &[
            ("ldaa", "lda"), ("ldab", "ldb"), ("staa", "sta"), ("stab", "stb"), ("oraa", "ora"), ("orab", "orb"),
            ("bhs", "bcc"), ("bhis", "bcc"), ("blo", "bcs"), ("blos", "bls"), ("lbhs", "lbcc"), ("lbhis", "lbcc"),
            ("lblo", "lbcs"), ("lblos", "lbls"), ("lsl", "asl"), ("lsla", "asla"), ("lslb", "aslb"), ("swi1", "swi"),
        ];
        let text = include_str!("../../test/6809/t6809.asm");
        let symbols = symbols(text);
        let all = cases(text, &|src| COMPAT.contains(&src.split_whitespace().next().unwrap_or("")));
        let mut bad = Vec::new();
        for c in &all {
            let d = disassemble(&|a| a.checked_sub(c.addr as u32).and_then(|i| c.bytes.get(i as usize)).copied(), c.addr as u32);
            let Some(d) = d else {
                bad.push(format!("line {}: {} does not disassemble", c.line, c.source));
                continue;
            };
            let (mut name, operand) = normalise(&c.source, c.addr, &symbols);
            if let Some(&(_, canon)) = ALIASES.iter().find(|(a, _)| *a == name) {
                name = canon.to_string();
            }
            let got = normalise(&d.text, c.addr, &symbols);
            if (name, operand) != got || d.len as usize != c.bytes.len() {
                bad.push(format!("line {}: {} ({:02x?}) disassembles to {} ({})", c.line, c.source, c.bytes, d.text, d.len));
            }
        }
        assert!(bad.is_empty(), "{} of {} lines disagree:\n{}", bad.len(), all.len(), bad.join("\n"));
        assert!(all.len() > 380, "only {} lines compared", all.len());
    }

    #[test]
    fn a_byte_that_is_not_memory_is_none() {
        assert_eq!(disassemble(&|_| None, 0), None);
        assert_eq!(disassemble(&|a| (a == 0).then_some(0xb6), 0), None, "the operand is not there");
    }
}
