// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Z80 core against SingleStepTests (github.com/SingleStepTests/z80).
//!
//! 1604 files, one per opcode with its prefixes, of 1000 cases each: the
//! whole register file -- `WZ`, `Q`, the EI shadow and the interrupt state
//! included -- and a few bytes of RAM before one instruction and after it,
//! the port traffic with its full 16-bit address, and the T-state count.
//! Where zexall CRCs each instruction group over thousands of states, this
//! names the instruction, the state and the register that differs.
//!
//! The data is not in the repo: `tools/fetch-sst.py` fetches a pinned
//! commit and converts it into `tests/data/sst-z80/` (its doc has the
//! format), so the test is `#[ignore]`d and run on purpose, in release:
//!
//! ```text
//! cargo test --release -- --ignored sst
//! SST_FILTER="ed a2" cargo test --release -- --ignored sst   # one file
//! ```
//!
//! It reports every failing file with a count per register and the first
//! failing case, rather than stopping at the first mismatch. Without a
//! filter it requires all 1604 files and every case in them, so a missing
//! or truncated data directory fails rather than passing on nothing.
//!
//! Two fields are compared on terms of their own:
//! - **`PC` after a `HALT`.** The suite has the silicon's PC, one past the
//!   `HALT`; the core holds PC on it while asleep and pushes the address
//!   past it when it wakes, which is the same to everything but a debugger
//!   -- and that way a breakpoint on a `HALT` is the one that fires every
//!   sleeping step, not one on the instruction after it. A sleeping core is
//!   compared at PC + 1.
//! - **`p`**, the suite's mark that the instruction was `LD A,I` or `LD
//!   A,R`, for an interrupt that would see their stale PV, is not modelled
//!   and not converted: no case here takes an interrupt.

use super::z80::CpuZ80;
use super::{Cpu, StepResult};
use crate::bus::Bus;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The converted register line's order, as `tools/fetch-sst.py` writes it,
/// and the core's name for each.
const REGS: [(&str, &str); 24] = [
    ("pc", "PC"),
    ("sp", "SP"),
    ("a", ""),
    ("f", ""),
    ("b", ""),
    ("c", ""),
    ("d", ""),
    ("e", ""),
    ("h", ""),
    ("l", ""),
    ("i", "I"),
    ("r", "R"),
    ("ei", "EI"),
    ("wz", "WZ"),
    ("ix", "IX"),
    ("iy", "IY"),
    ("af_", "AF2"),
    ("bc_", "BC2"),
    ("de_", "DE2"),
    ("hl_", "HL2"),
    ("im", "IM"),
    ("q", "Q"),
    ("iff1", "IFF1"),
    ("iff2", "IFF2"),
];

/// The eight 8-bit registers the line carries singly, which the core sets
/// and reports as pairs.
const PAIRS: [(&str, usize, usize); 4] = [("AF", 2, 3), ("BC", 4, 5), ("DE", 6, 7), ("HL", 8, 9)];

const FILES: usize = 1604;
const CASES_PER_FILE: usize = 1000;

struct Case {
    name: String,
    regs0: [u32; 24],
    ram0: Vec<(u16, u8)>,
    regs1: [u32; 24],
    ram1: Vec<(u16, u8)>,
    ports: Vec<(u16, u8, bool)>,
    cycles: u32,
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("bad hex '{s}'"))
}

fn parse_regs(s: &str) -> [u32; 24] {
    let mut r = [0; 24];
    let mut n = 0;
    for (slot, v) in r.iter_mut().zip(s.split(' ')) {
        *slot = hex(v);
        n += 1;
    }
    assert_eq!(n, 24, "register line '{s}'");
    r
}

fn parse_ram(s: &str) -> Vec<(u16, u8)> {
    s.split(' ')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (a, v) = p.split_once('=').unwrap();
            (hex(a) as u16, hex(v) as u8)
        })
        .collect()
}

fn parse_ports(s: &str) -> Vec<(u16, u8, bool)> {
    s.split(' ')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (a, rest) = p.split_once('=').unwrap();
            let (v, d) = rest.split_once('/').unwrap();
            (hex(a) as u16, hex(v) as u8, d == "w")
        })
        .collect()
}

fn parse(line: &str) -> Case {
    let f: Vec<&str> = line.split('\t').collect();
    assert_eq!(f.len(), 7, "case line '{line}'");
    Case {
        name: f[0].to_string(),
        regs0: parse_regs(f[1]),
        ram0: parse_ram(f[2]),
        regs1: parse_regs(f[3]),
        ram1: parse_ram(f[4]),
        ports: parse_ports(f[5]),
        cycles: f[6].parse().unwrap(),
    }
}

/// Flat RAM that remembers what it was given and what was written, and
/// answers port reads from the case's own traffic.
struct SstBus {
    mem: Vec<u8>,
    written: Vec<u16>,
    /// the case's port reads, in order, answered one per `io_read8`
    reads: Vec<u8>,
    /// every port access the core made: address, value, write
    ports: Vec<(u16, u8, bool)>,
}

impl Bus for SstBus {
    fn read8(&mut self, addr: u32) -> u8 {
        self.mem[(addr & 0xffff) as usize]
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.mem[(addr & 0xffff) as usize] = val;
        self.written.push(addr as u16);
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        let v = if self.reads.is_empty() { 0xff } else { self.reads.remove(0) };
        self.ports.push((port, v, false));
        v
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        self.ports.push((port, val, true));
    }
}

fn reg(cpu: &CpuZ80, name: &str) -> Option<u32> {
    cpu.registers().into_iter().find(|r| r.name == name).map(|r| r.value)
}

/// Run one case; the names of the fields that differ, and a description of
/// the first difference.
fn run(case: &Case, bus: &mut SstBus) -> (Vec<&'static str>, String) {
    let mut cpu = CpuZ80::new();
    let r0 = &case.regs0;
    for (i, &(_, name)) in REGS.iter().enumerate() {
        if !name.is_empty() {
            cpu.set_register(name, r0[i]);
        }
    }
    for &(name, hi, lo) in &PAIRS {
        cpu.set_register(name, (r0[hi] << 8) | r0[lo]);
    }
    for &(a, v) in &case.ram0 {
        bus.mem[a as usize] = v;
    }
    bus.written.clear();
    bus.ports.clear();
    bus.reads = case.ports.iter().filter(|p| !p.2).map(|p| p.1).collect();

    let mut bad = Vec::new();
    let mut detail = String::new();
    let result = cpu.step(bus);
    if result != StepResult::Ok {
        bad.push("step");
        let _ = write!(detail, "step {result:?}; ");
    }

    let r1 = &case.regs1;
    let halted = reg(&cpu, "HALT") == Some(1);
    for (i, &(field, name)) in REGS.iter().enumerate() {
        let got = if name.is_empty() {
            let (pair, shift) = PAIRS
                .iter()
                .find_map(|&(p, hi, lo)| if hi == i { Some((p, 8)) } else if lo == i { Some((p, 0)) } else { None })
                .unwrap();
            reg(&cpu, pair).map(|v| (v >> shift) & 0xff)
        } else if name == "PC" && halted {
            reg(&cpu, name).map(|v| (v + 1) & 0xffff)
        } else {
            reg(&cpu, name)
        };
        if got != Some(r1[i]) {
            bad.push(field);
            let _ = write!(detail, "{field}: want {:x} got {}; ", r1[i], got.map_or("none".into(), |v| format!("{v:x}")));
        }
    }

    for &(a, v) in &case.ram1 {
        if bus.mem[a as usize] != v {
            bad.push("ram");
            let _ = write!(detail, "ram {a:04x}: want {v:02x} got {:02x}; ", bus.mem[a as usize]);
        }
    }
    for &a in &bus.written {
        if !case.ram1.iter().any(|&(fa, _)| fa == a) {
            bad.push("ram");
            let _ = write!(detail, "stray write {a:04x}; ");
        }
    }
    if bus.ports != case.ports {
        bad.push("ports");
        let _ = write!(detail, "ports: want {:x?} got {:x?}; ", case.ports, bus.ports);
    }
    if cpu.last_step_cycles() != case.cycles {
        bad.push("cycles");
        let _ = write!(detail, "cycles: want {} got {}; ", case.cycles, cpu.last_step_cycles());
    }

    // only what this case touched goes back to zero
    for &(a, _) in case.ram0.iter().chain(&case.ram1) {
        bus.mem[a as usize] = 0;
    }
    for &a in &bus.written {
        bus.mem[a as usize] = 0;
    }
    bad.sort_unstable();
    bad.dedup();
    (bad, detail)
}

#[test]
#[ignore = "needs tools/fetch-sst.py's data; run with --ignored, in release"]
fn sst() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sst-z80");
    let filter = std::env::var("SST_FILTER").ok();
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e} -- run tools/fetch-sst.py", dir.display()))
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".txt"))
        .filter(|n| filter.as_ref().is_none_or(|f| n.trim_end_matches(".txt") == f))
        .collect();
    names.sort();

    let mut bus = SstBus { mem: vec![0; 0x10000], written: Vec::new(), reads: Vec::new(), ports: Vec::new() };
    let mut cases = 0;
    let mut failed = 0;
    let mut report = String::new();
    for n in &names {
        let text = std::fs::read_to_string(dir.join(n)).unwrap();
        let mut fields: BTreeMap<&str, usize> = BTreeMap::new();
        let mut first = None;
        let mut file_cases = 0;
        let mut file_failed = 0;
        for line in text.lines() {
            let case = parse(line);
            file_cases += 1;
            let (bad, detail) = run(&case, &mut bus);
            if !bad.is_empty() {
                file_failed += 1;
                for f in bad {
                    *fields.entry(f).or_default() += 1;
                }
                first.get_or_insert_with(|| format!("{}: {detail}", case.name));
            }
        }
        if filter.is_none() {
            assert_eq!(file_cases, CASES_PER_FILE, "{n}: truncated?");
        }
        cases += file_cases;
        failed += file_failed;
        if file_failed > 0 {
            let _ = writeln!(report, "{n}: {file_failed} failed {fields:?}\n    first {}", first.unwrap());
        }
    }
    println!("{report}{} files, {cases} cases, {failed} failed", names.len());
    if filter.is_none() {
        assert_eq!(names.len(), FILES, "{}: not the whole suite", dir.display());
    } else {
        assert!(!names.is_empty(), "SST_FILTER matched no file");
    }
    assert_eq!(failed, 0, "cases failed; the report is above");
}
