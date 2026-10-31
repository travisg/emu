// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Z80 disassembler: Zilog's mnemonics in ASxxxx's `asz80` spelling --
//! lower case, `0x` hex, `(ix+0x05)`, a relative jump as its target.
//!
//! The decode is the x/y/z split of the opcode byte, the same one the core
//! uses, but the tables are written out here rather than shared, so the two
//! check each other (`lengths_are_the_cores_fetches`). An instruction is
//! what the core takes in one step: a run of DD/FD prefixes and the
//! instruction after it, the last prefix the one that counts, and a prefix
//! the instruction has no use for showing only in the raw bytes. The
//! undocumented side reads as the core runs it: IXh/IXl as `ixh`/`ixl`,
//! `sll`, a DD CB operation's register writeback as `rlc (ix+0x05),b`, and
//! the ED page's aliases under their instructions' names. The rest of the
//! ED page is the two-byte no-op it is on silicon, shown as `.db`.

use super::Disassembly;

const R: [&str; 8] = ["b", "c", "d", "e", "h", "l", "(hl)", "a"];
const RP: [&str; 4] = ["bc", "de", "hl", "sp"];
const RP2: [&str; 4] = ["bc", "de", "hl", "af"];
const CC: [&str; 8] = ["nz", "z", "nc", "c", "po", "pe", "p", "m"];
const ALU: [&str; 8] = ["add a,", "adc a,", "sub ", "sbc a,", "and ", "xor ", "or ", "cp "];
const ROT: [&str; 8] = ["rlc", "rrc", "rl", "rr", "sla", "sra", "sll", "srl"];
const ROTATE_A: [&str; 8] = ["rlca", "rrca", "rla", "rra", "daa", "cpl", "scf", "ccf"];
const BLOCK: [[&str; 4]; 4] = [
    ["ldi", "cpi", "ini", "outi"],
    ["ldd", "cpd", "ind", "outd"],
    ["ldir", "cpir", "inir", "otir"],
    ["lddr", "cpdr", "indr", "otdr"],
];
/// IM's operand by y: the 0/1 holes are IM 0 on the core.
const IM: [&str; 8] = ["0", "0", "1", "2", "0", "0", "1", "2"];

/// Reads the instruction's bytes in order.
struct Reader<'a> {
    peek: &'a dyn Fn(u32) -> Option<u8>,
    pos: u16,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let b = (self.peek)(self.pos as u32);
        self.pos = self.pos.wrapping_add(1);
        b
    }

    fn word(&mut self) -> Option<u16> {
        let lo = self.byte()?;
        Some(u16::from_le_bytes([lo, self.byte()?]))
    }

    fn n(&mut self) -> Option<String> {
        Some(format!("0x{:02x}", self.byte()?))
    }

    fn nn(&mut self) -> Option<String> {
        Some(format!("0x{:04x}", self.word()?))
    }

    /// A relative jump's target, from its displacement byte.
    fn rel(&mut self) -> Option<String> {
        let d = self.byte()? as i8 as u16;
        Some(format!("0x{:04x}", self.pos.wrapping_add(d)))
    }
}

/// The index register a prefix selects, or none.
#[derive(Copy, Clone, PartialEq)]
enum Index {
    Hl,
    Ix,
    Iy,
}

impl Index {
    fn pair(self) -> &'static str {
        match self {
            Index::Hl => "hl",
            Index::Ix => "ix",
            Index::Iy => "iy",
        }
    }

    /// `(ix+0x05)` from a displacement.
    fn indexed(self, d: u8) -> String {
        let d = d as i8;
        let sign = if d < 0 { '-' } else { '+' };
        format!("({}{sign}0x{:02x})", self.pair(), d.unsigned_abs())
    }
}

/// The `r` operand `z`: under a prefix H and L are the index halves, and
/// `(hl)` is `(ix+d)`, its displacement read here.
fn reg(ix: Index, z: u8, rd: &mut Reader) -> Option<String> {
    Some(match (ix, z) {
        (Index::Hl, _) => R[z as usize].to_string(),
        (_, 6) => ix.indexed(rd.byte()?),
        (_, 4) => format!("{}h", ix.pair()),
        (_, 5) => format!("{}l", ix.pair()),
        _ => R[z as usize].to_string(),
    })
}

fn rp(ix: Index, p: u8) -> &'static str {
    if p == 2 {
        ix.pair()
    } else {
        RP[p as usize]
    }
}

fn base(op: u8, ix: Index, rd: &mut Reader) -> Option<String> {
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let (p, q) = (y >> 1, y & 1);
    let hl = ix.pair();
    Some(match x {
        0 => match z {
            0 => match y {
                0 => "nop".into(),
                1 => "ex af,af'".into(),
                2 => format!("djnz {}", rd.rel()?),
                3 => format!("jr {}", rd.rel()?),
                _ => format!("jr {},{}", CC[(y - 4) as usize], rd.rel()?),
            },
            1 if q == 0 => format!("ld {},{}", rp(ix, p), rd.nn()?),
            1 => format!("add {hl},{}", rp(ix, p)),
            2 => match (q, p) {
                (0, 0) => "ld (bc),a".into(),
                (0, 1) => "ld (de),a".into(),
                (0, 2) => format!("ld ({}),{hl}", rd.nn()?),
                (0, _) => format!("ld ({}),a", rd.nn()?),
                (_, 0) => "ld a,(bc)".into(),
                (_, 1) => "ld a,(de)".into(),
                (_, 2) => format!("ld {hl},({})", rd.nn()?),
                _ => format!("ld a,({})", rd.nn()?),
            },
            3 => format!("{} {}", if q == 0 { "inc" } else { "dec" }, rp(ix, p)),
            4 => format!("inc {}", reg(ix, y, rd)?),
            5 => format!("dec {}", reg(ix, y, rd)?),
            // the displacement comes before the immediate
            6 => format!("ld {},{}", reg(ix, y, rd)?, rd.n()?),
            _ => ROTATE_A[y as usize].into(),
        },
        1 if y == 6 && z == 6 => "halt".into(),
        // with (ix+d) on one side, H and L on the other stay H and L
        1 if y == 6 => format!("ld {},{}", reg(ix, 6, rd)?, R[z as usize]),
        1 if z == 6 => format!("ld {},{}", R[y as usize], reg(ix, 6, rd)?),
        1 => format!("ld {},{}", reg(ix, y, rd)?, reg(ix, z, rd)?),
        2 => format!("{}{}", ALU[y as usize], reg(ix, z, rd)?),
        _ => match z {
            0 => format!("ret {}", CC[y as usize]),
            1 if q == 0 => format!("pop {}", if p == 2 { hl } else { RP2[p as usize] }),
            1 => match p {
                0 => "ret".into(),
                1 => "exx".into(),
                2 => format!("jp ({hl})"),
                _ => format!("ld sp,{hl}"),
            },
            2 => format!("jp {},{}", CC[y as usize], rd.nn()?),
            3 => match y {
                0 => format!("jp {}", rd.nn()?),
                2 => format!("out ({}),a", rd.n()?),
                3 => format!("in a,({})", rd.n()?),
                4 => format!("ex (sp),{hl}"),
                5 => "ex de,hl".into(),
                6 => "di".into(),
                7 => "ei".into(),
                _ => unreachable!("CB is decoded before this"),
            },
            4 => format!("call {},{}", CC[y as usize], rd.nn()?),
            5 if q == 0 => format!("push {}", if p == 2 { hl } else { RP2[p as usize] }),
            5 => format!("call {}", rd.nn()?),
            6 => format!("{}{}", ALU[y as usize], rd.n()?),
            _ => format!("rst 0x{:02x}", y * 8),
        },
    })
}

/// The CB page; under a prefix its displacement comes before the opcode,
/// and every operation but BIT also writes its result to the register the
/// low bits name.
fn cb(ix: Index, rd: &mut Reader) -> Option<String> {
    let d = if ix == Index::Hl { 0 } else { rd.byte()? };
    let op = rd.byte()?;
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let target = if ix == Index::Hl { R[z as usize].to_string() } else { ix.indexed(d) };
    let writeback = if ix != Index::Hl && z != 6 && x != 1 { format!(",{}", R[z as usize]) } else { String::new() };
    Some(match x {
        0 => format!("{} {target}{writeback}", ROT[y as usize]),
        1 => format!("bit {y},{target}"),
        2 => format!("res {y},{target}{writeback}"),
        _ => format!("set {y},{target}{writeback}"),
    })
}

/// The ED page; None for an encoding with no instruction.
fn ed(rd: &mut Reader) -> Option<Option<String>> {
    let op = rd.byte()?;
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let (p, q) = (y >> 1, y & 1);
    Some(Some(match (x, z) {
        (1, 0) if y == 6 => "in (c)".into(),
        (1, 0) => format!("in {},(c)", R[y as usize]),
        (1, 1) if y == 6 => "out (c),0".into(),
        (1, 1) => format!("out (c),{}", R[y as usize]),
        (1, 2) => format!("{} hl,{}", if q == 0 { "sbc" } else { "adc" }, RP[p as usize]),
        (1, 3) if q == 0 => format!("ld ({}),{}", rd.nn()?, RP[p as usize]),
        (1, 3) => format!("ld {},({})", RP[p as usize], rd.nn()?),
        (1, 4) => "neg".into(),
        (1, 5) if y == 1 => "reti".into(),
        (1, 5) => "retn".into(),
        (1, 6) => format!("im {}", IM[y as usize]),
        (1, 7) => match y {
            0 => "ld i,a".into(),
            1 => "ld r,a".into(),
            2 => "ld a,i".into(),
            3 => "ld a,r".into(),
            4 => "rrd".into(),
            5 => "rld".into(),
            _ => return Some(None),
        },
        (2, 0..=3) if y >= 4 => BLOCK[(y - 4) as usize][z as usize].into(),
        _ => return Some(None),
    }))
}

/// The instruction at `addr`.
pub fn disassemble(peek: &dyn Fn(u32) -> Option<u8>, addr: u32) -> Option<Disassembly> {
    let mut rd = Reader { peek, pos: addr as u16 };
    let mut ix = Index::Hl;
    let op = loop {
        match rd.byte()? {
            0xdd => ix = Index::Ix,
            0xfd => ix = Index::Iy,
            op => break op,
        }
    };
    let text = match op {
        0xcb => cb(ix, &mut rd)?,
        // a prefix in front of ED is spent on nothing
        0xed => match ed(&mut rd)? {
            Some(text) => text,
            None => {
                let at = rd.pos.wrapping_sub(2);
                format!(".db 0x{:02x},0x{:02x}", peek(at as u32)?, peek(at.wrapping_add(1) as u32)?)
            }
        },
        op => base(op, ix, &mut rd)?,
    };
    Some(Disassembly { text, len: rd.pos.wrapping_sub(addr as u16) as u32 })
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
    fn each_shape_reads_as_asz80_writes_it() {
        assert_eq!(dis_at(0x0100, &[0x21, 0x34, 0x12]), Disassembly { text: "ld hl,0x1234".into(), len: 3 });
        assert_eq!(text(&[0x3e, 0x20]), "ld a,0x20");
        assert_eq!(text(&[0x18, 0xfe]), "jr 0x0100");
        assert_eq!(text(&[0x20, 0x10]), "jr nz,0x0112");
        assert_eq!(text(&[0x10, 0xfe]), "djnz 0x0100");
        assert_eq!(text(&[0x08]), "ex af,af'");
        assert_eq!(text(&[0x22, 0x00, 0x80]), "ld (0x8000),hl");
        assert_eq!(text(&[0x76]), "halt");
        assert_eq!(text(&[0x78]), "ld a,b");
        assert_eq!(text(&[0x86]), "add a,(hl)");
        assert_eq!(text(&[0x96]), "sub (hl)");
        assert_eq!(text(&[0xfe, 0x0d]), "cp 0x0d");
        assert_eq!(text(&[0xd3, 0x80]), "out (0x80),a");
        assert_eq!(text(&[0xdb, 0x81]), "in a,(0x81)");
        assert_eq!(text(&[0xe9]), "jp (hl)");
        assert_eq!(text(&[0xc4, 0x00, 0x10]), "call nz,0x1000");
        assert_eq!(text(&[0xff]), "rst 0x38");
        assert_eq!(text(&[0xf5]), "push af");
    }

    #[test]
    fn the_index_prefixes() {
        assert_eq!(text(&[0xdd, 0x21, 0x00, 0x40]), "ld ix,0x4000");
        assert_eq!(text(&[0xfd, 0x7e, 0xfb]), "ld a,(iy-0x05)");
        assert_eq!(text(&[0xdd, 0x66, 0x05]), "ld h,(ix+0x05)");
        assert_eq!(text(&[0xdd, 0x36, 0x05, 0x99]), "ld (ix+0x05),0x99");
        assert_eq!(text(&[0xdd, 0x64]), "ld ixh,ixh");
        assert_eq!(text(&[0xfd, 0x85]), "add a,iyl");
        assert_eq!(text(&[0xdd, 0xe9]), "jp (ix)");
        assert_eq!(text(&[0xdd, 0xe3]), "ex (sp),ix");
        assert_eq!(text(&[0xdd, 0x29]), "add ix,ix");
        assert_eq!(text(&[0xdd, 0xeb]), "ex de,hl", "no use for the prefix");
        assert_eq!(dis_at(0x0100, &[0xdd, 0x00]), Disassembly { text: "nop".into(), len: 2 });
        assert_eq!(dis_at(0x0100, &[0xdd, 0xfd, 0x21, 0x34, 0x12]), Disassembly { text: "ld iy,0x1234".into(), len: 5 });
        assert_eq!(dis_at(0x0100, &[0xdd, 0xed, 0xb0]), Disassembly { text: "ldir".into(), len: 3 });
    }

    #[test]
    fn the_cb_and_ed_pages() {
        assert_eq!(text(&[0xcb, 0x46]), "bit 0,(hl)");
        assert_eq!(text(&[0xcb, 0x37]), "sll a");
        assert_eq!(dis_at(0x0100, &[0xdd, 0xcb, 0x55, 0x46]), Disassembly { text: "bit 0,(ix+0x55)".into(), len: 4 });
        assert_eq!(text(&[0xdd, 0xcb, 0x55, 0x40]), "bit 0,(ix+0x55)");
        assert_eq!(text(&[0xfd, 0xcb, 0x01, 0x00]), "rlc (iy+0x01),b");
        assert_eq!(text(&[0xdd, 0xcb, 0xff, 0xfe]), "set 7,(ix-0x01)");
        assert_eq!(text(&[0xed, 0xb0]), "ldir");
        assert_eq!(text(&[0xed, 0x4d]), "reti");
        assert_eq!(text(&[0xed, 0x55]), "retn");
        assert_eq!(text(&[0xed, 0x5e]), "im 2");
        assert_eq!(text(&[0xed, 0x70]), "in (c)");
        assert_eq!(text(&[0xed, 0x71]), "out (c),0");
        assert_eq!(text(&[0xed, 0x43, 0x00, 0x80]), "ld (0x8000),bc");
        assert_eq!(text(&[0xed, 0x6b, 0x00, 0x80]), "ld hl,(0x8000)");
        assert_eq!(text(&[0xed, 0x57]), "ld a,i");
        assert_eq!(dis_at(0x0100, &[0xed, 0x00]), Disassembly { text: ".db 0xed,0x00".into(), len: 2 });
        assert_eq!(text(&[0xed, 0x77]), ".db 0xed,0x77");
    }

    /// Over every opcode of every page, prefixes included: the length is
    /// how far the core's PC goes in one step, for every instruction that
    /// does not jump, and wait or repeat in place.
    #[test]
    fn lengths_are_the_cores_fetches() {
        use crate::cpu::testbus::TestBus;
        use crate::cpu::Cpu;
        const FLOW: &[&str] = &[
            "jr", "djnz", "jp", "call", "ret", "rst", "reti", "retn", "halt", "ldir", "lddr", "cpir", "cpdr", "inir",
            "indr", "otir", "otdr",
        ];
        let mut cases: Vec<Vec<u8>> = Vec::new();
        for prefix in [&[][..], &[0xdd], &[0xfd], &[0xed], &[0xcb], &[0xdd, 0xcb, 0x05], &[0xfd, 0xcb, 0xfb]] {
            for op in 0..=0xffu8 {
                let mut b = prefix.to_vec();
                b.extend([op, 0x05, 0x00, 0x00]);
                cases.push(b);
            }
        }
        let mut compared = 0;
        let mut bus = TestBus::new();
        for bytes in cases {
            let d = dis_at(0x0100, &bytes);
            bus.load(0x0100, &bytes);
            let mut cpu = crate::cpu::z80::CpuZ80::new();
            for (r, v) in [("PC", 0x0100), ("SP", 0x8000), ("HL", 0x4000), ("IX", 0x5000), ("IY", 0x6000)] {
                assert!(cpu.set_register(r, v));
            }
            cpu.step(&mut bus);
            let name = d.text.split(' ').next().unwrap();
            if !FLOW.contains(&name) {
                assert_eq!((cpu.pc() as u16).wrapping_sub(0x0100) as u32, d.len, "{bytes:02x?}: {}", d.text);
                compared += 1;
            }
        }
        assert!(compared > 1600, "only {compared} lengths compared");
    }

    /// ASxxxx's own test of asz80, which the distribution ships and this
    /// tree does not (`tools/fetch-asxxxx.py` puts it in `tests/data/`):
    /// every Z80 instruction line -- the file goes on to the HD64180's and
    /// others', which stop at `.hd64` -- disassembles to what its source
    /// says, once both spell an index the same way (`0x55(ix)` and `(ix)`
    /// are `(ix+0x55)` and `(ix+0)`) and drop the optional `a,` and `#`.
    /// `cargo test -- --ignored tz80`.
    #[test]
    #[ignore]
    fn tz80_asm_disassembles_to_its_own_source() {
        use crate::cpu::asxxxx_check::{cases, normalise, symbols};
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/asxxxx/tz80.asm");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}; run tools/fetch-asxxxx.py"));
        let z80 = &text[..text.find("\t.hd64").expect("the HD64180 section")];
        let symbols = symbols(z80);
        let spell = |s: &str, here: u16| {
            let (name, mut op) = normalise(s, here, &symbols);
            op = op.replace('#', "");
            for r in ["ix", "iy"] {
                // `n(ix)` and `(ix)` are `(ix+n)` and `(ix+0)`
                op = op.replace(&format!("({r})"), &format!("({r}+0)"));
                while let Some(at) = op.find(&format!("({r}+0)")).filter(|&at| at > 0) {
                    let start = op[..at].rfind(|c: char| !(c.is_ascii_digit() || c == '-')).map_or(0, |i| i + 1);
                    if start == at {
                        break;
                    }
                    let n: i64 = op[start..at].parse().unwrap();
                    let sign = if n < 0 { "" } else { "+" };
                    op = format!("{}({r}{sign}{n}){}", &op[..start], &op[at + r.len() + 4..]);
                }
            }
            const OPTIONAL_A: &[&str] =
                &["add", "adc", "sub", "sbc", "and", "xor", "or", "cp", "rlc", "rrc", "rl", "rr", "sla", "sra", "sll", "srl"];
            if OPTIONAL_A.contains(&name.as_str()) {
                if let Some(rest) = op.strip_prefix("a,").filter(|r| !r.is_empty()) {
                    op = rest.to_string();
                }
            }
            (name, op)
        };
        let all = cases(z80, &|_| false);
        let mut bad = Vec::new();
        for c in &all {
            let d = disassemble(&|a| a.checked_sub(c.addr as u32).and_then(|i| c.bytes.get(i as usize)).copied(), c.addr as u32)
                .unwrap();
            if spell(&c.source, c.addr) != spell(&d.text, c.addr) || d.len as usize != c.bytes.len() {
                bad.push(format!("line {}: {} ({:02x?}) disassembles to {}", c.line, c.source, c.bytes, d.text));
            }
        }
        assert!(bad.is_empty(), "{} of {} lines disagree:\n{}", bad.len(), all.len(), bad.join("\n"));
        assert!(all.len() > 600, "only {} lines compared", all.len());
    }

    #[test]
    fn a_byte_that_is_not_memory_is_none() {
        assert_eq!(disassemble(&|_| None, 0), None);
        assert_eq!(disassemble(&|a| (a == 0).then_some(0x21), 0), None, "the operand is not there");
    }
}
