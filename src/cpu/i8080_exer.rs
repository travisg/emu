// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 8080 core against the CP/M instruction exercisers.
//!
//! TST8080 and CPUTEST are diagnostics that check each instruction's result
//! and flags against expected values in the program; 8080PRE is the quick
//! check that the exerciser can run at all; 8080EXM runs each instruction
//! group over thousands of operand and flag combinations and CRCs the
//! results -- the flag byte whole, its fixed bits included -- against the
//! CRCs of real silicon. A group that fails names the instruction and
//! nothing finer.
//!
//! No CP/M is involved: each program loads at 0x0100 over a flat 64K with
//! a `JMP` at 5 (the BDOS entry) to a `RET` at the top of memory. The test
//! answers BDOS functions 2 (the character in E) and 9 (the `$`-ended
//! string at DE) whenever PC reaches 5, and takes PC reaching 0, CP/M's
//! warm boot, as the program's exit. The programs are fetched, not kept here
//! (`tools/fetch-i8080-tests.py`), so the test is `#[ignore]`d and run on
//! purpose, in release -- 8080EXM is minutes of it:
//!
//! ```text
//! cargo test --release -- --ignored i8080_exercisers
//! I8080_FILTER=TST8080 cargo test --release -- --ignored i8080_exercisers
//! ```

use super::i8080::Cpu8080;
use super::testbus::TestBus;
use super::{Cpu, StepResult};
use std::path::Path;

/// Where the BDOS's `RET` sits, which the program also reads at 6 as the
/// top of its memory.
const BDOS: u16 = 0xfe00;

/// Each program, what it prints when it passes and what it prints when
/// anything has not, and how many steps it gets.
const PROGRAMS: [(&str, &str, &[&str], u64); 4] = [
    ("8080PRE", "8080 Preliminary tests complete", &["ERROR", "Error"], 10_000_000),
    ("TST8080", "CPU IS OPERATIONAL", &["CPU HAS FAILED"], 10_000_000),
    ("CPUTEST", "CPU TESTS OK", &["ERROR", "FAIL"], 1_000_000_000),
    ("8080EXM", "Tests complete", &["ERROR"], 50_000_000_000),
];

/// Run one program to its exit, returning what it printed.
fn run(image: &[u8], limit: u64) -> Result<String, String> {
    let mut bus = TestBus::new();
    bus.load(0x0100, image);
    bus.load(0x0005, &[0xc3, BDOS as u8, (BDOS >> 8) as u8]);
    bus.load(BDOS, &[0xc9]);
    let mut cpu = Cpu8080::new();
    cpu.reset(&mut bus);
    cpu.set_register("PC", 0x0100);
    cpu.set_register("SP", BDOS as u32);
    let mut out = String::new();
    for _ in 0..limit {
        if cpu.pc() == 0 {
            return Ok(out);
        }
        if cpu.pc() == 5 {
            let reg = |n| cpu.registers().into_iter().find(|r| r.name == n).unwrap().value as u16;
            let (c, de) = (reg("BC") as u8, reg("DE"));
            match c {
                2 => out.push(de as u8 as char),
                9 => {
                    let mut at = de;
                    while bus.mem[at as usize] != b'$' {
                        out.push(bus.mem[at as usize] as char);
                        at = at.wrapping_add(1);
                    }
                }
                _ => return Err(format!("BDOS function {c} at {:04x}\n{out}", reg("SP"))),
            }
        }
        match cpu.step(&mut bus) {
            StepResult::Ok => {}
            r => return Err(format!("{r:?} at {:04x}\n{out}", cpu.pc())),
        }
    }
    Err(format!("still running after {limit} steps\n{out}"))
}

#[test]
#[ignore = "needs tools/fetch-i8080-tests.py's programs; run with --ignored, in release"]
fn i8080_exercisers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/i8080");
    let filter = std::env::var("I8080_FILTER").ok();
    let mut failed = Vec::new();
    let mut ran = 0;
    for (name, pass, fails, limit) in PROGRAMS {
        if filter.as_ref().is_some_and(|f| f != name) {
            continue;
        }
        let path = dir.join(format!("{name}.COM"));
        let image = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} -- run tools/fetch-i8080-tests.py", path.display()));
        ran += 1;
        match run(&image, limit) {
            Ok(out) => {
                println!("--- {name}\n{out}");
                if !out.contains(pass) || fails.iter().any(|f| out.contains(f)) {
                    failed.push(format!("{name}:\n{out}"));
                }
            }
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    assert!(ran > 0, "no program matches I8080_FILTER={filter:?}");
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
