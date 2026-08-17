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
//! Cross-validate the Rust 6809 core against the C++ oracle.
//!
//! Same idea as the 6800 suite, but System09 loads Intel HEX rather than a
//! flat binary, so each snippet is emitted as a synthetic .hex with the code
//! at 0xc000 and the reset vector pointing at it.
//!
//! Every case here is `#[ignore]`d, because it needs a C++ oracle binary that
//! is no longer in the tree: plain `cargo test` reports them as ignored rather
//! than silently passing a comparison that never ran. To run them, build an
//! oracle and use `EMU_ORACLE=... cargo test -- --include-ignored` (AGENTS.md
//! has the worktree recipe). Anything missing then fails loudly.

use emu::console::ConsoleEndpoint;
use emu::cpu::StepResult;
use emu::emulator::Emulator;
use emu::system::registry;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Start of the rom bank in the System09 map.
const ROM_BASE: u16 = 0xc000;

/// The C++ oracle. Not in the tree any more (removed in phase 5 of the
/// conversion): build it from the last commit that had it, in a worktree, and
/// point `EMU_ORACLE` at the binary -- see AGENTS.md. Falls back to the old
/// in-tree location for a worktree that still has one.
///
/// Panics rather than skipping: every case that calls this is `#[ignore]`d, so
/// reaching it means the oracle gate was asked for explicitly. Returning early
/// instead would report a pass for a comparison that never ran.
fn oracle() -> PathBuf {
    let p = std::env::var_os("EMU_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("build-emu/emu"));
    assert!(
        p.exists(),
        "no C++ oracle at {}: build one and point EMU_ORACLE at it -- see AGENTS.md",
        p.display()
    );
    p
}

/// Emit one Intel HEX record.
fn hex_record(addr: u16, kind: u8, data: &[u8]) -> String {
    let mut sum = data.len() as u8;
    sum = sum.wrapping_add((addr >> 8) as u8).wrapping_add(addr as u8).wrapping_add(kind);
    let mut s = format!(":{:02X}{:04X}{:02X}", data.len(), addr, kind);
    for b in data {
        s.push_str(&format!("{b:02X}"));
        sum = sum.wrapping_add(*b);
    }
    s.push_str(&format!("{:02X}\n", sum.wrapping_neg()));
    s
}

/// Build a .hex image: the snippet at 0xc000, reset vector at 0xfffe.
fn hex_image(code: &[u8]) -> String {
    let mut s = String::new();
    for (i, chunk) in code.chunks(16).enumerate() {
        s.push_str(&hex_record(ROM_BASE + (i * 16) as u16, 0x00, chunk));
    }
    s.push_str(&hex_record(0xfffe, 0x00, &[(ROM_BASE >> 8) as u8, ROM_BASE as u8]));
    s.push_str(&hex_record(0, 0x01, &[]));
    s
}

struct Sink(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn rust_trace(hex_path: &Path, instructions: usize) -> String {
    let (_tx, rx) = std::sync::mpsc::channel();
    let endpoint = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let desc = registry::find("6809").unwrap();
    let machine = (desc.factory)(hex_path, endpoint, "").expect("failed to build system09");

    let mut emu = Emulator::new(machine.cpu, machine.bus, Arc::new(AtomicBool::new(false)));
    emu.set_cycle_limit(Some(instructions as i64 + 1));
    let sink = Arc::new(Mutex::new(Vec::new()));
    emu.set_trace(Some(Box::new(Sink(Arc::clone(&sink)))));
    emu.reset();
    emu.run();

    let out = sink.lock().unwrap().clone();
    String::from_utf8(out).unwrap()
}

fn cpp_trace(bin: &Path, hex_path: &Path, instructions: usize, name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("emu-tracediff09-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let trace_path = dir.join(format!("{name}.trace"));
    let log_path = dir.join(format!("{name}.log"));
    let log = std::fs::File::create(&log_path).unwrap();
    let log_err = log.try_clone().unwrap();

    let mut child = Command::new(bin)
        .args(["-s", "6809"])
        .arg("-r")
        .arg(hex_path)
        .arg("-l")
        .arg((instructions + 1).to_string())
        .arg("--trace")
        .arg(&trace_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .expect("failed to spawn the c++ emulator");

    // stdin must stay open for the child's lifetime; see the 6800 suite
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

fn assert_traces_match_upto(name: &str, code: &[u8], instructions: usize, expect: Option<usize>) {
    let bin = oracle();

    let dir = std::env::temp_dir().join(format!("emu-tracediff09-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hex_path = dir.join(format!("{name}.hex"));
    std::fs::write(&hex_path, hex_image(code)).unwrap();

    let rust = rust_trace(&hex_path, instructions);
    let cpp = cpp_trace(&bin, &hex_path, instructions, name);
    std::fs::remove_file(&hex_path).ok();

    assert!(!cpp.is_empty(), "{name}: c++ produced an empty trace");
    assert!(!rust.is_empty(), "{name}: rust produced an empty trace");

    if rust != cpp {
        let mut msg = format!("trace mismatch in {name}\n{:<58} | c++\n", "rust");
        for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
            let mark = if r == c { ' ' } else { '*' };
            msg.push_str(&format!("{mark}{i:4} {r:<52} | {c}\n"));
            if r != c && i > 0 {
                break;
            }
        }
        panic!("{msg}");
    }
    if let Some(n) = expect {
        assert_eq!(rust.lines().count(), n, "{name}: unexpected trace length");
    }
}

fn assert_traces_match(name: &str, code: &[u8], instructions: usize) {
    assert_traces_match_upto(name, code, instructions, Some(instructions));
}

const LDA_IMM: u8 = 0x86;
const LDB_IMM: u8 = 0xc6;
const LDX_IMM: u8 = 0x8e;
const LDS_IMM_H: u8 = 0x10; // 0x10 0xce
const LDU_IMM: u8 = 0xce;
const LDD_IMM: u8 = 0xcc;

/// Stack + accumulators, so snippets have something to work with.
#[rustfmt::skip]
const SETUP: [u8; 13] = [
    LDS_IMM_H, 0xce, 0x1f, 0xff,   // lds #$1fff
    LDU_IMM, 0x1e, 0xff,           // ldu #$1eff
    LDA_IMM, 0x96,
    LDB_IMM, 0x5a,
    LDX_IMM, 0x00,                 // (completed below)
];

fn with_setup(code: &[u8]) -> Vec<u8> {
    let mut v = SETUP.to_vec();
    v.push(0x40); // finishes ldx #$0040
    v.extend_from_slice(code);
    v
}
const SETUP_INSNS: usize = 5;

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn alu_and_flags() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0x8b, 0x01,             // adda #1
        0x80, 0x20,             // suba #$20
        0x89, 0x10,             // adca #$10
        0x82, 0x05,             // sbca #5
        0x81, 0x96,             // cmpa #$96
        0x84, 0x0f,             // anda #$0f
        0x8a, 0xf0,             // ora  #$f0
        0x88, 0xff,             // eora #$ff
        0x85, 0x10,             // bita #$10
    ]);
    assert_traces_match("alu_and_flags", &code, SETUP_INSNS + 9);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn sixteen_bit_alu_on_d() {
    #[rustfmt::skip]
    let code = with_setup(&[
        LDD_IMM, 0x12, 0x34,
        0xc3, 0x00, 0x01,       // addd #1
        0x83, 0x00, 0x10,       // subd #$10
        0x10, 0x83, 0x12, 0x25, // cmpd #$1225
        0x8c, 0x00, 0x40,       // cmpx #$0040
        0x11, 0x83, 0x1e, 0xff, // cmpu #$1eff
        0x11, 0x8c, 0x1f, 0xff, // cmps #$1fff
    ]);
    assert_traces_match("sixteen_bit_alu", &code, SETUP_INSNS + 7);
}

/// The indexed postbyte is the most intricate part of the 6809 decode:
/// constant offsets of three widths, accumulator offsets, auto in/decrement by
/// one and two, PC-relative, and the indirect forms of most of them.
#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn indexed_addressing_modes() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0xa6, 0x00,             // lda ,x
        0xa6, 0x05,             // lda 5,x        (5-bit offset)
        0xa6, 0x1f,             // lda -1,x       (5-bit negative)
        0xa6, 0x88, 0x10,       // lda $10,x      (8-bit offset)
        0xa6, 0x89, 0x01, 0x00, // lda $100,x     (16-bit offset)
        0xa6, 0x85,             // lda b,x
        0xa6, 0x86,             // lda a,x
        0xa6, 0x8b,             // lda d,x
        0xa6, 0x80,             // lda ,x+
        0xa6, 0x81,             // lda ,x++
        0xa6, 0x82,             // lda ,-x
        0xa6, 0x83,             // lda ,--x
        0xa6, 0x8c, 0x04,       // lda $4,pcr
        0xa6, 0x8d, 0x00, 0x04, // lda $4,pcr (16-bit)
        0xa6, 0xa0,             // lda ,y+
        0xa6, 0xc0,             // lda ,u+
        0xa6, 0xe0,             // lda ,s+
    ]);
    assert_traces_match("indexed_modes", &code, SETUP_INSNS + 17);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn indexed_indirect_modes() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0xa6, 0x94,             // lda [,x]
        0xa6, 0x98, 0x10,       // lda [$10,x]
        0xa6, 0x99, 0x01, 0x00, // lda [$100,x]
        0xa6, 0x91,             // lda [,x++]
        0xa6, 0x93,             // lda [,--x]
        0xa6, 0x9f, 0x00, 0x40, // lda [$0040]   (extended indirect)
        0xa6, 0x9b,             // lda [d,x]
    ]);
    assert_traces_match("indexed_indirect", &code, SETUP_INSNS + 7);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn lea_instructions() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0x30, 0x05,             // leax 5,x
        0x31, 0x88, 0x10,       // leay $10,x
        0x32, 0x84,             // leas ,x
        0x33, 0x85,             // leau b,x
        0x30, 0x1f,             // leax -1,x  -> exercises Z on leax
    ]);
    assert_traces_match("lea", &code, SETUP_INSNS + 5);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn exg_and_tfr_register_pairs() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0x1f, 0x01,             // tfr d,x
        0x1f, 0x89,             // tfr a,b
        0x1f, 0x8b,             // tfr a,dp
        0x1f, 0xa8,             // tfr cc,a
        0x1e, 0x01,             // exg d,x
        0x1e, 0x89,             // exg a,b
        0x1e, 0x12,             // exg x,y
        0x1e, 0x34,             // exg u,s
    ]);
    assert_traces_match("exg_tfr", &code, SETUP_INSNS + 8);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn push_and_pull_register_sets() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0x34, 0x06,             // pshs a,b
        0x35, 0x06,             // puls a,b
        0x34, 0xff,             // pshs everything
        0x35, 0xff,             // puls everything
        0x36, 0x06,             // pshu a,b
        0x37, 0x06,             // pulu a,b
        0x34, 0x40,             // pshs u
        0x35, 0x40,             // puls u
    ]);
    assert_traces_match("push_pull", &code, SETUP_INSNS + 8);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn read_modify_write_and_shifts() {
    #[rustfmt::skip]
    let code = with_setup(&[
        LDA_IMM, 0x81,
        0xb7, 0x00, 0x40,       // sta $0040
        0x48,                   // asla
        0x47,                   // asra   (no fallthrough on the 6809)
        0x44,                   // lsra
        0x49,                   // rola
        0x46,                   // rora
        0x43,                   // coma
        0x40,                   // nega
        0x4c,                   // inca
        0x4a,                   // deca
        0x4d,                   // tsta
        0x4f,                   // clra
        0x78, 0x00, 0x40,       // asl $0040 (extended)
        0x77, 0x00, 0x40,       // asr $0040
        0x74, 0x00, 0x40,       // lsr $0040
        0x63, 0x84,             // com ,x    (indexed)
        0x60, 0x84,             // neg ,x
    ]);
    assert_traces_match("rmw_and_shifts", &code, SETUP_INSNS + 19);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn branches_and_subroutines() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0x81, 0x96,             // cmpa #$96 -> equal
        0x27, 0x02,             // beq +2
        LDA_IMM, 0xee,
        0x26, 0x02,             // bne +2 (not taken)
        LDA_IMM, 0x11,
        0x21, 0x02,             // brn +2 (never taken)
        LDA_IMM, 0x22,
        0x16, 0x00, 0x02,       // lbra +2
        LDA_IMM, 0xee,
        0x8d, 0x02,             // bsr +2
        0x20, 0x01,             // bra +1
        0x39,                   // rts
    ]);
    assert_traces_match("branches", &code, SETUP_INSNS + 12);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn sex_abx_and_dp_relative() {
    #[rustfmt::skip]
    let code = with_setup(&[
        LDB_IMM, 0x80,
        0x1d,                   // sex -> a = 0xff
        LDB_IMM, 0x7f,
        0x1d,                   // sex -> a = 0x00
        0x3a,                   // abx
        LDA_IMM, 0x00,
        0x1f, 0x8b,             // tfr a,dp  (dp = 0)
        0x96, 0x40,             // lda <$40  (direct page)
        0x97, 0x41,             // sta <$41
        0x0c, 0x41,             // inc <$41
        0x0f, 0x41,             // clr <$41
    ]);
    assert_traces_match("sex_abx_dp", &code, SETUP_INSNS + 12);
}

#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn loads_and_stores_all_widths() {
    #[rustfmt::skip]
    let code = with_setup(&[
        0xcc, 0x12, 0x34,       // ldd #$1234
        0xfd, 0x00, 0x50,       // std $0050
        0xfc, 0x00, 0x50,       // ldd $0050
        0x10, 0x8e, 0x0a, 0x0b, // ldy #$0a0b
        0x10, 0xbf, 0x00, 0x52, // sty $0052
        0x10, 0xbe, 0x00, 0x52, // ldy $0052
        0xff, 0x00, 0x54,       // stu $0054
        0xfe, 0x00, 0x54,       // ldu $0054
        0xb7, 0x00, 0x56,       // sta $0056
        0xf7, 0x00, 0x57,       // stb $0057
    ]);
    assert_traces_match("loads_stores", &code, SETUP_INSNS + 10);
}

/// Every opcode value on all three pages. Catches decode-table transcription
/// errors that targeted tests miss -- the 6809 table has ~250 entries spread
/// across three pages, which is a lot of hand-copied rows.
#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn every_opcode_agrees() {
    for page in [0u8, 0x10, 0x11] {
        for opcode in 0u16..=0xff {
            let mut body = Vec::new();
            if page != 0 {
                body.push(page);
            }
            body.push(opcode as u8);
            // operand bytes; 0x40 keeps addresses inside ram
            body.extend_from_slice(&[0x00, 0x40, 0x00, 0x40]);

            let code = with_setup(&body);
            assert_traces_match_upto(
                &format!("op_{page:02x}_{opcode:02x}"),
                &code,
                SETUP_INSNS + 3,
                None,
            );
        }
    }
}

/// Gate: boot the real BASIC.HEX through both implementations.
#[test]
#[ignore = "needs the C++ oracle; see AGENTS.md"]
fn real_basic_rom_boot_matches() {
    let bin = oracle();
    let rom = Path::new(emu::system::sys09::DEFAULT_ROM);
    assert!(rom.exists(), "{} not present -- run from the repo root", rom.display());

    const N: usize = 20_000;
    let rust = rust_trace(rom, N);
    let cpp = cpp_trace(&bin, rom, N, "real_boot");

    assert_eq!(rust.lines().count(), N, "rust trace length");
    assert_eq!(cpp.lines().count(), N, "c++ trace length");
    for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
        assert_eq!(r, c, "divergence at instruction {i}");
    }
}

#[test]
fn bad_opcode_halts() {
    // 0x01 is not a valid 6809 opcode
    let code = with_setup(&[0x01]);
    let dir = std::env::temp_dir().join(format!("emu-tracediff09-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hex_path = dir.join("badop.hex");
    std::fs::write(&hex_path, hex_image(&code)).unwrap();

    let (_tx, rx) = std::sync::mpsc::channel();
    let endpoint = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let desc = registry::find("6809").unwrap();
    let machine = (desc.factory)(&hex_path, endpoint, "").unwrap();
    std::fs::remove_file(&hex_path).ok();

    let mut cpu = machine.cpu;
    let mut bus = machine.bus;
    cpu.reset(&mut *bus);
    for _ in 0..SETUP_INSNS {
        assert_eq!(cpu.step(&mut *bus), StepResult::Ok);
    }
    assert_eq!(cpu.step(&mut *bus), StepResult::BadOpcode);
}
