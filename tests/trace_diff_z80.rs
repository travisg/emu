// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! Cross-validate the Rust Z80 core against the C++ oracle.
//!
//! Each case is a hand-assembled snippet dropped at offset 0 of a synthetic 64K
//! RC2014 rom image -- the Z80 has no reset vector, it just starts at 0. The
//! same image is run through both implementations and their `--trace` output is
//! compared line for line.
//!
//! This is the primary Phase 3 gate. Booting the real rom is broad but shallow
//! (it reaches a serial poll loop and stays there), so these snippets are where
//! the flag, prefix and addressing semantics actually get checked.
//!
//! The Rust side drives the real [`Rc2014`] machine rather than a stand-in bus,
//! so the address decode and the port decode are under test too. With no
//! console attached, every input port is deterministic.
//!
//! Skipped (not failed) when `build-emu/emu` is absent, so `cargo test` still
//! works without the C++ tree built.

use emu::cpu::z80::CpuZ80;
use emu::cpu::Cpu;
use emu::system::rc2014::Rc2014;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The rom image is a full 64K bank, but only the bottom 8K is visible.
const ROM_IMAGE_SIZE: usize = 64 * 1024;
const ROM_WINDOW: usize = 0x2000;

/// Set up a stack in ram plus known registers, so the opcode under test has
/// something non-trivial to work with. Ram lives at 0x8000 and up, and C is
/// loaded with a real port number so the `(C)` forms don't spray "unknown
/// port" onto stderr.
///
/// The three marker bytes matter: a displacement of `$f0` resolves to `$90f0`
/// read as unsigned and `$8ff0` read as signed, so seeding both with distinct
/// values is what makes the `LD r, (IX+d)` sign bug observable in a trace at
/// all. Without them every indexed read returns zero either way.
#[rustfmt::skip]
const PREAMBLE: [u8; 34] = [
    0x31, 0xff, 0xff,       // ld sp, $ffff
    0x3e, 0x96,             // ld a, $96
    0x01, 0x80, 0x04,       // ld bc, $0480
    0x11, 0x00, 0x90,       // ld de, $9000
    0x21, 0xf0, 0x90,       // ld hl, $90f0
    0x36, 0x5a,             // ld (hl), $5a    -- ix + $f0 unsigned
    0x21, 0xf0, 0x8f,       // ld hl, $8ff0
    0x36, 0xa5,             // ld (hl), $a5    -- ix + $f0 sign extended
    0x21, 0x00, 0x90,       // ld hl, $9000
    0x36, 0x3c,             // ld (hl), $3c    -- so (hl) reads aren't zero
    0xdd, 0x21, 0x00, 0x90, // ld ix, $9000
    0xfd, 0x21, 0x10, 0x90, // ld iy, $9010
];
const PREAMBLE_INSNS: usize = 12;

/// Assemble a snippet into a full 64K rom image. The snippet runs from 0.
fn rom_image(code: &[u8]) -> Vec<u8> {
    assert!(code.len() <= ROM_WINDOW, "snippet does not fit in the rom window");
    let mut rom = vec![0x00u8; ROM_IMAGE_SIZE]; // fill with nop
    rom[..code.len()].copy_from_slice(code);
    rom
}

/// A snippet with the register preamble in front of it.
fn with_preamble(code: &[u8]) -> Vec<u8> {
    let mut full = PREAMBLE.to_vec();
    full.extend_from_slice(code);
    full
}

fn emu_binary() -> Option<PathBuf> {
    let p = Path::new("build-emu/emu").to_path_buf();
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// Per-case scratch directory. `cargo test` runs tests concurrently in one
/// process, so shared filenames would race.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emu-z80-tracediff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn rust_trace(rom_path: &Path, instructions: usize) -> String {
    // no console attached: the receiver is disconnected, so every input port
    // reads deterministically
    let (_tx, rx) = std::sync::mpsc::channel();
    let console = emu::console::ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let mut bus = Rc2014::new(rom_path, console).expect("failed to build the rc2014 machine");

    let mut cpu = CpuZ80::new();
    cpu.reset(&mut bus);

    let mut out: Vec<u8> = Vec::new();
    for _ in 0..instructions {
        cpu.trace_line(&mut out).unwrap();
        // stop where the run loop would: a bad opcode (which includes an
        // unconsumed DD/FD prefix) ends the run, after the line is emitted
        if cpu.step(&mut bus) != emu::cpu::StepResult::Ok {
            break;
        }
    }
    String::from_utf8(out).unwrap()
}

fn cpp_trace(bin: &Path, rom_path: &Path, instructions: usize, name: &str) -> String {
    let trace_path = scratch(&format!("{name}.trace"));
    let log_path = scratch(&format!("{name}.log"));
    let log = std::fs::File::create(&log_path).unwrap();
    let log_err = log.try_clone().unwrap();

    // The C++ cycle limit runs N-1 instructions for -l N, so ask for one more.
    let mut child = Command::new(bin)
        .args(["-s", "rc2014"])
        .arg("-r")
        .arg(rom_path)
        .arg("-l")
        .arg((instructions + 1).to_string())
        .arg("--trace")
        .arg(&trace_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .expect("failed to spawn the c++ emulator");

    // Hold stdin open for the child's whole life. Both `wait()` and
    // `wait_with_output()` close it otherwise, which EOFs the console, shuts
    // the cpu thread down early, and yields a truncated trace that still looks
    // like a successful run.
    let child_stdin = child.stdin.take();
    let status = child.wait().expect("c++ emulator did not exit");
    drop(child_stdin);

    let log_text = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(status.success(), "c++ emulator exited with {status}\n{log_text}");
    std::fs::remove_file(&log_path).ok();

    let trace = std::fs::read_to_string(&trace_path).unwrap();
    std::fs::remove_file(&trace_path).ok();
    trace
}

/// Run a snippet through both cores and require identical traces.
///
/// `expect_lines` guards the failure mode where both sides truncate: comparing
/// two empty traces would otherwise pass vacuously. Pass `None` when the
/// snippet may legitimately stop early -- which for this core is the common
/// case, since most DD/FD combinations end the run.
fn assert_traces_match_upto(
    name: &str,
    code: &[u8],
    instructions: usize,
    expect_lines: Option<usize>,
) {
    let Some(bin) = emu_binary() else {
        eprintln!("skipping {name}: build-emu/emu not built");
        return;
    };

    let rom_path = scratch(&format!("{name}.rom"));
    std::fs::write(&rom_path, rom_image(code)).unwrap();

    let rust = rust_trace(&rom_path, instructions);
    let cpp = cpp_trace(&bin, &rom_path, instructions, name);
    std::fs::remove_file(&rom_path).ok();

    assert!(!cpp.is_empty(), "{name}: c++ produced an empty trace");
    assert!(!rust.is_empty(), "{name}: rust produced an empty trace");
    // Trace *length* is a real signal here, not just content: an instruction
    // that fails the end-of-instruction prefix check ends the run, so a core
    // that wrongly consumes (or wrongly fails to consume) a prefix shows up as
    // a length difference and nothing else.
    assert_eq!(
        rust.lines().count(),
        cpp.lines().count(),
        "{name}: trace lengths differ (rust {} lines, c++ {})",
        rust.lines().count(),
        cpp.lines().count()
    );

    if rust != cpp {
        let mut msg = format!("trace mismatch in {name}\n");
        msg.push_str(&format!("{:<74} | {}\n", "rust", "c++"));
        for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
            let mark = if r == c { ' ' } else { '*' };
            msg.push_str(&format!("{mark}{i:4} {r:<74} | {c}\n"));
            if r != c && i > 0 {
                break;
            }
        }
        panic!("{msg}");
    }
    if let Some(n) = expect_lines {
        assert_eq!(rust.lines().count(), n, "{name}: unexpected trace length");
    }
}

/// Straight-line snippet: both sides must run exactly `instructions` of it.
fn assert_traces_match(name: &str, code: &[u8], instructions: usize) {
    assert_traces_match_upto(name, code, instructions, Some(instructions));
}

/// Snippet plus the register preamble, counted for you.
fn assert_snippet(name: &str, code: &[u8], insns: usize) {
    assert_traces_match(name, &with_preamble(code), PREAMBLE_INSNS + insns);
}

#[test]
fn loads_and_stores() {
    #[rustfmt::skip]
    let code = [
        0x06, 0x11,             // ld b, $11
        0x0e, 0x22,             // ld c, $22
        0x16, 0x33,             // ld d, $33
        0x1e, 0x44,             // ld e, $44
        0x26, 0x90,             // ld h, $90
        0x2e, 0x40,             // ld l, $40
        0x36, 0x5a,             // ld (hl), $5a
        0x7e,                   // ld a, (hl)
        0x47,                   // ld b, a
        0x70,                   // ld (hl), b
        0x02,                   // ld (bc), a     -- bc is $1122, unmapped
        0x12,                   // ld (de), a
        0x0a,                   // ld a, (bc)
        0x1a,                   // ld a, (de)
        0x32, 0x50, 0x90,       // ld ($9050), a
        0x3a, 0x50, 0x90,       // ld a, ($9050)
        0x22, 0x60, 0x90,       // ld ($9060), hl
        0x2a, 0x60, 0x90,       // ld hl, ($9060)
        0xf9,                   // ld sp, hl
    ];
    assert_snippet("loads_and_stores", &code, 20);
}

#[test]
fn alu_register_and_immediate_forms_agree() {
    #[rustfmt::skip]
    let code = [
        0x3e, 0x7f,             // ld a, $7f
        0x06, 0x01,             // ld b, $01
        0x80,                   // add a, b       -- $80, sets S and PV
        0xc6, 0x01,             // add a, $01
        0x3e, 0xff,             // ld a, $ff
        0x88,                   // adc a, b
        0xce, 0x01,             // adc a, $01     -- carry set from above
        0x3e, 0x10,             // ld a, $10
        0x90,                   // sub b
        0xd6, 0x20,             // sub $20        -- borrow
        0x98,                   // sbc a, b
        0xde, 0x05,             // sbc a, $05
        0x3e, 0xf0,             // ld a, $f0
        0xa0,                   // and b
        0xe6, 0x3c,             // and $3c
        0xb0,                   // or b
        0xf6, 0x0f,             // or $0f
        0xa8,                   // xor b
        0xee, 0xff,             // xor $ff
        0xb8,                   // cp b
        0xfe, 0x00,             // cp $00
    ];
    assert_snippet("alu_register_and_immediate_forms_agree", &code, 22);
}

/// `(HL)` operands for the whole ALU column, which is a different fetch path
/// from the register forms.
#[test]
fn alu_against_memory() {
    #[rustfmt::skip]
    let code = [
        0x36, 0x0f,             // ld (hl), $0f
        0x86,                   // add a, (hl)
        0x8e,                   // adc a, (hl)
        0x96,                   // sub (hl)
        0x9e,                   // sbc a, (hl)
        0xa6,                   // and (hl)
        0xae,                   // xor (hl)
        0xb6,                   // or (hl)
        0xbe,                   // cp (hl)
        0x34,                   // inc (hl)
        0x35,                   // dec (hl)
    ];
    assert_snippet("alu_against_memory", &code, 11);
}

#[test]
fn inc_dec_edge_cases() {
    #[rustfmt::skip]
    let code = [
        0x3e, 0x7f,             // ld a, $7f
        0x3c,                   // inc a          -- PV set, H set
        0x3e, 0x80,             // ld a, $80
        0x3d,                   // dec a          -- PV set
        0x06, 0xff,             // ld b, $ff
        0x04,                   // inc b          -- wraps to 0, Z set
        0x05,                   // dec b          -- back to $ff
        0x0e, 0x00,             // ld c, $00
        0x0d,                   // dec c          -- H set
        0x03,                   // inc bc
        0x0b,                   // dec bc
        0x33,                   // inc sp
        0x3b,                   // dec sp
    ];
    assert_snippet("inc_dec_edge_cases", &code, 14);
}

#[test]
fn sixteen_bit_arithmetic() {
    #[rustfmt::skip]
    let code = [
        0x21, 0xff, 0x7f,       // ld hl, $7fff
        0x11, 0x01, 0x00,       // ld de, $0001
        0x19,                   // add hl, de     -- half-carry out of bit 11
        0x21, 0xff, 0xff,       // ld hl, $ffff
        0x19,                   // add hl, de     -- carry out
        0x29,                   // add hl, hl
        0x39,                   // add hl, sp
        0xed, 0x5a,             // adc hl, de
        0xed, 0x52,             // sbc hl, de
        0x21, 0x00, 0x80,       // ld hl, $8000
        0x11, 0x01, 0x00,       // ld de, $0001
        0xed, 0x52,             // sbc hl, de     -- signed overflow, PV set
        0xed, 0x42,             // sbc hl, bc
        0xed, 0x4a,             // adc hl, bc
    ];
    assert_snippet("sixteen_bit_arithmetic", &code, 15);
}

#[test]
fn accumulator_rotates_and_the_decimal_adjust() {
    #[rustfmt::skip]
    let code = [
        0x3e, 0x81,             // ld a, $81
        0x07,                   // rlca
        0x0f,                   // rrca
        0x37,                   // scf
        0x17,                   // rla
        0x1f,                   // rra
        0x3f,                   // ccf            -- H takes the old carry
        0x3f,                   // ccf
        0x2f,                   // cpl
        0x3e, 0x19,             // ld a, $19
        0x06, 0x28,             // ld b, $28
        0x80,                   // add a, b       -- $41, needs adjusting
        0x27,                   // daa            -- $47
        0x3e, 0x99,             // ld a, $99
        0xc6, 0x01,             // add a, $01
        0x27,                   // daa            -- wraps, carry out
        0x3e, 0x42,             // ld a, $42
        0xd6, 0x13,             // sub $13
        0x27,                   // daa            -- the N path
    ];
    assert_snippet("accumulator_rotates_and_the_decimal_adjust", &code, 21);
}

#[test]
fn cb_page_rotates_and_shifts() {
    #[rustfmt::skip]
    let code = [
        0x06, 0x81,             // ld b, $81
        0x37,                   // scf
        0xcb, 0x00,             // rlc b
        0xcb, 0x08,             // rrc b
        0xcb, 0x10,             // rl b
        0xcb, 0x18,             // rr b
        0xcb, 0x20,             // sla b
        0xcb, 0x28,             // sra b
        0xcb, 0x30,             // sll b          -- undocumented
        0xcb, 0x38,             // srl b
        0x36, 0x81,             // ld (hl), $81
        0xcb, 0x06,             // rlc (hl)
        0xcb, 0x0e,             // rrc (hl)
        0xcb, 0x16,             // rl (hl)
        0xcb, 0x1e,             // rr (hl)
        0xcb, 0x26,             // sla (hl)
        0xcb, 0x2e,             // sra (hl)
        0xcb, 0x36,             // sll (hl)
        0xcb, 0x3e,             // srl (hl)
    ];
    assert_snippet("cb_page_rotates_and_shifts", &code, 19);
}

#[test]
fn bit_res_and_set() {
    #[rustfmt::skip]
    let code = [
        0x06, 0x80,             // ld b, $80
        0xcb, 0x78,             // bit 7, b       -- the one bit that sets S
        0xcb, 0x40,             // bit 0, b
        0xcb, 0x50,             // bit 2, b
        0xcb, 0xb8,             // res 7, b
        0xcb, 0xf8,             // set 7, b
        0x36, 0xff,             // ld (hl), $ff
        0xcb, 0x7e,             // bit 7, (hl)
        0xcb, 0x86,             // res 0, (hl)
        0xcb, 0xc6,             // set 0, (hl)
        0xcb, 0xae,             // res 5, (hl)
    ];
    assert_snippet("bit_res_and_set", &code, 11);
}

/// The conditional forms, covered with relative jumps so the snippet stays
/// position independent.
#[test]
fn jumps_and_relative_branches() {
    #[rustfmt::skip]
    let code = [
        0xaf,                   // xor a          -- Z set
        0x20, 0x02,             // jr nz, +2      -- not taken
        0x28, 0x00,             // jr z, +0       -- taken, falls through
        0x18, 0x00,             // jr +0
        0x37,                   // scf
        0x30, 0x02,             // jr nc, +2      -- not taken
        0x38, 0x00,             // jr c, +0       -- taken
        0x06, 0x03,             // ld b, 3
        0x10, 0xfe,             // djnz -2        -- loops back onto itself
        0x00,                   // nop
    ];
    assert_snippet("jumps_and_relative_branches", &code, 14);
}

/// The absolute forms, which need a known target: `$0100` is inside the rom
/// window and filled with nops.
#[test]
fn absolute_jumps() {
    let mut code = vec![0u8; 0x104];
    code[..PREAMBLE.len()].copy_from_slice(&PREAMBLE);
    let entry = PREAMBLE.len();
    #[rustfmt::skip]
    let body: [u8; 12] = [
        0xaf,                   // xor a          -- Z set, C clear
        0xc2, 0x00, 0x01,       // jp nz, $0100   -- not taken
        0xd2, 0x00, 0x01,       // jp nc, $0100   -- taken
        0x00, 0x00, 0x00, 0x00, // (skipped)
        0x00,
    ];
    code[entry..entry + body.len()].copy_from_slice(&body);
    code[0x100] = 0xe9; // jp (hl) -- hl is $9000, so this leaves the rom
    assert_traces_match("absolute_jumps", &code, PREAMBLE_INSNS + 6);
}

#[test]
fn call_return_and_restart() {
    // The subroutine sits at $0038, which is also rst 7's target, so `rst 38h`
    // and `call $0038` exercise the same landing pad.
    let mut code = vec![0u8; 0x40];
    code[..PREAMBLE.len()].copy_from_slice(&PREAMBLE);
    let entry = PREAMBLE.len();
    #[rustfmt::skip]
    let body: [u8; 12] = [
        0xcd, 0x38, 0x00,       // call $0038
        0xaf,                   // xor a
        0xc4, 0x38, 0x00,       // call nz, $0038 -- not taken
        0xcc, 0x38, 0x00,       // call z, $0038  -- taken
        0xff,                   // rst 38h
        0x00,                   // nop
    ];
    code[entry..entry + body.len()].copy_from_slice(&body);
    // $0038: a conditional return that isn't taken (carry is clear), then a
    // plain one
    code[0x38] = 0xd8; // ret c
    code[0x39] = 0xc9; // ret

    // three calls plus the rst, each running two instructions at $0038
    assert_traces_match("call_return_and_restart", &code, PREAMBLE_INSNS + 12);
}

#[test]
fn stack_and_exchanges() {
    #[rustfmt::skip]
    let code = [
        0xc5,                   // push bc
        0xd5,                   // push de
        0xe5,                   // push hl
        0xf5,                   // push af
        0xf1,                   // pop af
        0xe1,                   // pop hl
        0xd1,                   // pop de
        0xc1,                   // pop bc
        0xeb,                   // ex de, hl
        0x08,                   // ex af, af'
        0xd9,                   // exx
        0x08,                   // ex af, af'     -- back again
        0xd9,                   // exx
        0xe5,                   // push hl
        0xe3,                   // ex (sp), hl
        0xe1,                   // pop hl
    ];
    assert_snippet("stack_and_exchanges", &code, 16);
}

/// The IX/IY forms, including the deliberate unsigned-displacement bug in
/// `LD r, (IX+d)`: with `d = $ff` it reads (IX+255), not (IX-1).
#[test]
fn index_register_forms() {
    #[rustfmt::skip]
    let code = [
        0xdd, 0x36, 0x00, 0x5a, // ld (ix+0), $5a
        0xdd, 0x36, 0x10, 0xa5, // ld (ix+16), $a5
        0xdd, 0x7e, 0x00,       // ld a, (ix+0)
        // The unsigned-d bug: this reads the $90f0 marker ($5a), not the
        // $8ff0 one ($a5) that a sign-extended displacement would reach.
        0xdd, 0x7e, 0xf0,       // ld a, (ix+$f0)
        0xdd, 0x77, 0x01,       // ld (ix+1), a   -- signed here
        0xdd, 0x86, 0x00,       // add a, (ix+0)
        0xdd, 0x96, 0x00,       // sub (ix+0)
        0xdd, 0xa6, 0x00,       // and (ix+0)
        0xdd, 0x34, 0x00,       // inc (ix+0)
        0xdd, 0x35, 0x00,       // dec (ix+0)
        0xdd, 0x23,             // inc ix
        0xdd, 0x2b,             // dec ix
        0xdd, 0x19,             // add ix, de
        0xdd, 0x29,             // add ix, ix
        0xdd, 0xe5,             // push ix
        0xdd, 0xe1,             // pop ix
        0xfd, 0x7e, 0x00,       // ld a, (iy+0)
        0xfd, 0x23,             // inc iy
        0xfd, 0x19,             // add iy, de
        0xdd, 0xf9,             // ld sp, ix
    ];
    assert_snippet("index_register_forms", &code, 20);
}

/// The DD/FD CB page: `RLC`, `BIT`, `RES` and `SET` have indexed forms, and the
/// first three of those also do the undocumented writeback into the encoded
/// register.
#[test]
fn indexed_bit_operations() {
    #[rustfmt::skip]
    let code = [
        0xdd, 0x36, 0x00, 0x81, // ld (ix+0), $81
        0xdd, 0xcb, 0x00, 0x06, // rlc (ix+0)
        0xdd, 0xcb, 0x00, 0x00, // rlc (ix+0) -> b  (undocumented writeback)
        0xdd, 0xcb, 0x00, 0x7e, // bit 7, (ix+0)
        0xdd, 0xcb, 0x00, 0x86, // res 0, (ix+0)
        0xdd, 0xcb, 0x00, 0xc1, // set 0, (ix+0) -> c (undocumented writeback)
        0xfd, 0xcb, 0x00, 0x46, // bit 0, (iy+0)
        0xfd, 0xcb, 0x00, 0xce, // set 1, (iy+0)
    ];
    assert_snippet("indexed_bit_operations", &code, 8);
}

/// Most CB shifts ignore an active prefix, fall through to the plain `(HL)`
/// form, and then end the run at the end-of-instruction prefix check -- after
/// having already read the displacement and touched `(HL)`. Both sides must
/// stop at the same instruction.
#[test]
fn a_prefixed_cb_shift_runs_then_aborts() {
    #[rustfmt::skip]
    let code = [
        0xdd, 0xcb, 0x00, 0x0e, // rrc (ix+0) -- not honoured; hits (hl), aborts
        0x00, 0x00, 0x00,
    ];
    // The abort happens after the instruction executes, so the run is the
    // preamble plus this one instruction and no more.
    assert_traces_match_upto(
        "a_prefixed_cb_shift_runs_then_aborts",
        &with_preamble(&code),
        PREAMBLE_INSNS + 5,
        Some(PREAMBLE_INSNS + 1),
    );
}

/// A DD prefix in front of an instruction with no indexed form reads the
/// remapped IXh/IXl and *then* ends the run.
#[test]
fn an_unconsumed_prefix_ends_the_run() {
    assert_traces_match_upto(
        "an_unconsumed_prefix_ends_the_run",
        &with_preamble(&[0xdd, 0x84, 0x00, 0x00]), // add a, ixh
        PREAMBLE_INSNS + 4,
        Some(PREAMBLE_INSNS + 1),
    );
}

#[test]
fn ed_page_block_and_misc_operations() {
    #[rustfmt::skip]
    let code = [
        0x21, 0x00, 0x90,       // ld hl, $9000
        0x36, 0x11,             // ld (hl), $11
        0x23,                   // inc hl
        0x36, 0x22,             // ld (hl), $22
        0x21, 0x00, 0x90,       // ld hl, $9000
        0x11, 0x00, 0x91,       // ld de, $9100
        0x01, 0x02, 0x00,       // ld bc, 2
        0xed, 0xb0,             // ldir           -- repeats once
        0x21, 0x00, 0x90,       // ld hl, $9000
        0x01, 0x02, 0x00,       // ld bc, 2
        0x3e, 0x22,             // ld a, $22
        0xed, 0xb1,             // cpir           -- repeats until it matches
        0x21, 0x00, 0x90,       // ld hl, $9000
        0x01, 0x02, 0x00,       // ld bc, 2
        0xed, 0xa9,             // cpd
        0xed, 0x44,             // neg
        0xed, 0x44,             // neg            -- back again
        0xed, 0x67,             // rrd
        0xed, 0x6f,             // rld
        0xed, 0x47,             // ld i, a
        0xed, 0x57,             // ld a, i
        0xed, 0x4f,             // ld r, a
        0xed, 0x5f,             // ld a, r
        0xed, 0x56,             // im 1
        0xed, 0x46,             // im 0
        0xed, 0x5e,             // im 1 (alternate encoding)
    ];
    // ldir and cpir each repeat once, so the instruction count is higher than
    // the encoding count.
    assert_snippet("ed_page_block_and_misc_operations", &code, 30);
}

#[test]
fn ed_page_port_block_operations() {
    #[rustfmt::skip]
    let code = [
        0x01, 0x02, 0x80,       // ld bc, $8002   -- b = 2 iterations, c = port $80
        0x21, 0x00, 0x90,       // ld hl, $9000
        0xed, 0xa2,             // ini
        0xed, 0xaa,             // ind
        0xed, 0xa3,             // outi
        0xed, 0xab,             // outd
        0x06, 0x02,             // ld b, 2
        0xed, 0xb2,             // inir           -- repeats
        0x06, 0x02,             // ld b, 2
        0xed, 0xb3,             // otir           -- repeats
        0xed, 0x40,             // in b, (c)
        0xed, 0x41,             // out (c), b
        0xed, 0x71,             // out (c), 0     -- undocumented
        0xed, 0x78,             // in a, (c)
    ];
    assert_snippet("ed_page_port_block_operations", &code, 18);
}

#[test]
fn port_io_and_interrupt_flags() {
    #[rustfmt::skip]
    let code = [
        0xf3,                   // di
        0xfb,                   // ei
        0xdb, 0x80,             // in a, ($80)    -- sio status, no console
        0xdb, 0x81,             // in a, ($81)    -- sio data
        0xd3, 0x81,             // out ($81), a   -- console out
        0xdb, 0x90,             // in a, ($90)    -- reads $ff
        0xd3, 0x10,             // out ($10), a   -- cf controller, ignored
    ];
    assert_snippet("port_io_and_interrupt_flags", &code, 7);
}

/// Execute *every* value on a page, one per synthetic rom, and require both
/// implementations to agree.
///
/// This is what catches decode transcription errors the targeted tests miss,
/// and -- because an unconsumed prefix or an unimplemented ED opcode ends the
/// run -- it also pins down exactly which encodings are holes. Filling one in
/// (or leaving one out) shows up as a trace-length difference.
fn sweep_page(label: &str, prefix: &[u8]) {
    if emu_binary().is_none() {
        eprintln!("skipping {label}: build-emu/emu not built");
        return;
    }

    for opcode in 0u16..=0xff {
        let mut code = PREAMBLE.to_vec();
        code.extend_from_slice(prefix);
        code.push(opcode as u8);
        // Operand bytes for the multi-byte forms. $90f0 keeps addresses and
        // jump targets inside ram; as a displacement, $f0 lands on one seeded
        // marker read signed and the other read unsigned, so the two are
        // distinguishable. Anything left over decodes as further instructions,
        // which is fine: both sides run the same ones.
        code.extend_from_slice(&[0xf0, 0x90, 0xf0, 0x90, 0x00, 0x00, 0x00, 0x00]);

        assert_traces_match_upto(
            &format!("{label}_{opcode:02x}"),
            &code,
            PREAMBLE_INSNS + 5,
            None,
        );
    }
}

#[test]
fn every_base_opcode_agrees() {
    sweep_page("base", &[]);
}

#[test]
fn every_ed_opcode_agrees() {
    sweep_page("ed", &[0xed]);
}

#[test]
fn every_cb_opcode_agrees() {
    sweep_page("cb", &[0xcb]);
}

#[test]
fn every_dd_prefixed_opcode_agrees() {
    sweep_page("dd", &[0xdd]);
}

#[test]
fn every_fd_prefixed_opcode_agrees() {
    sweep_page("fd", &[0xfd]);
}

#[test]
fn every_dd_cb_opcode_agrees() {
    // a signed displacement, so the operand lands on the $8ff0 marker
    sweep_page("ddcb", &[0xdd, 0xcb, 0xf0]);
}

#[test]
fn every_fd_cb_opcode_agrees() {
    sweep_page("fdcb", &[0xfd, 0xcb, 0xf0]);
}

/// A DD prefix in front of an ED opcode: the ED page has no prefix handling at
/// all, so every one of these executes and then ends the run.
#[test]
fn every_dd_ed_opcode_agrees() {
    sweep_page("dded", &[0xdd, 0xed]);
}

/// Gate: boot the real RC2014 rom through both implementations and require
/// identical traces.
///
/// Broad but shallow -- the monitor initialises the SIO and then settles into a
/// transmit-ready poll it can never satisfy (the C++ SIO never reports an empty
/// transmit buffer), so this catches gross divergence rather than opcode
/// semantics. The snippets above are what cover behaviour.
#[test]
fn real_rom_boot_matches() {
    let Some(bin) = emu_binary() else {
        eprintln!("skipping: build-emu/emu not built");
        return;
    };
    let rom_path = Path::new(emu::system::rc2014::DEFAULT_ROM);
    if !rom_path.exists() {
        eprintln!("skipping: {} not present", rom_path.display());
        return;
    }

    const N: usize = 200_000;

    let rust = rust_trace(rom_path, N);
    let cpp = cpp_trace(&bin, rom_path, N, "real_rom_boot");

    assert_eq!(rust.lines().count(), N, "rust boot trace is short");
    assert_eq!(cpp.lines().count(), N, "c++ boot trace is short");

    if rust != cpp {
        for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
            if r != c {
                panic!("boot trace diverges at instruction {i}\n  rust: {r}\n  c++:  {c}");
            }
        }
    }
}
