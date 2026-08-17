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
//! Cross-validate the Rust 6800 core against the C++ oracle.
//!
//! Each case is a hand-assembled snippet dropped into a synthetic 256-byte
//! Altair 680 monitor ROM. The same ROM is run through both implementations
//! and their `--trace` output is compared line for line.
//!
//! This is the *primary* Phase 1 gate. Booting the real monitor ROM touches
//! only 36 distinct PCs before settling into an ACIA poll loop, so it catches
//! gross divergence but almost no opcode behaviour; these snippets are where
//! the flag and addressing-mode semantics actually get checked.
//!
//! Skipped (not failed) when no C++ oracle is available (`EMU_ORACLE`), so `cargo test` still
//! works without the C++ tree built.

use emu::bus::{Bus, MemoryDevice};
use emu::cpu::m6800::Cpu6800;
use emu::cpu::Cpu;
use emu::dev::memory::Memory;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where the monitor ROM lives, and therefore where snippets start.
const ROM_BASE: u16 = 0xff00;
const ROM_SIZE: usize = 256;
/// Offset of the reset vector within the ROM image.
const RESET_VECTOR_OFF: usize = 0xfe;

/// The Altair 680 address decode, as `system/altair680.cpp` does it.
///
/// The UART at 0xf000..=0xf001 is deliberately left unmapped here: the
/// snippets never touch it, and modelling it would mean pulling the MC6850's
/// console side-effects into a unit test.
struct Altair680TestBus {
    ram: Memory,
    rom_monitor: Memory,
    rom_vtl: Memory,
}

impl Altair680TestBus {
    fn new(rom_image: &[u8]) -> Self {
        let mut rom_monitor = Memory::new(ROM_SIZE);
        rom_monitor.load_at(0, rom_image);
        Altair680TestBus {
            ram: Memory::new(32 * 1024),
            rom_monitor,
            rom_vtl: Memory::new(768),
        }
    }
}

impl Bus for Altair680TestBus {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.read_byte(addr as u32),
            0xfc00..=0xfeff => self.rom_vtl.read_byte((addr - 0xfc00) as u32),
            0xff00..=0xffff => self.rom_monitor.read_byte((addr - 0xff00) as u32),
            // unmapped reads return 0, as the C++ does when the decode yields
            // no device
            _ => 0,
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        let addr = (addr & 0xffff) as u16;
        match addr {
            0x0000..=0x7fff => self.ram.write_byte(addr as u32, val),
            0xfc00..=0xfeff => self.rom_vtl.write_byte((addr - 0xfc00) as u32, val),
            0xff00..=0xffff => self.rom_monitor.write_byte((addr - 0xff00) as u32, val),
            // unmapped writes are dropped
            _ => {}
        }
    }
}

/// Assemble a snippet into a full 256-byte ROM image with the reset vector
/// pointing at its first byte.
fn rom_image(code: &[u8]) -> Vec<u8> {
    assert!(code.len() <= RESET_VECTOR_OFF, "snippet does not fit below the reset vector");
    let mut rom = vec![0x01u8; ROM_SIZE]; // fill with nop
    rom[..code.len()].copy_from_slice(code);
    rom[RESET_VECTOR_OFF] = (ROM_BASE >> 8) as u8;
    rom[RESET_VECTOR_OFF + 1] = ROM_BASE as u8;
    rom
}

fn rust_trace(rom: &[u8], instructions: usize) -> String {
    let mut bus = Altair680TestBus::new(rom);
    let mut cpu = Cpu6800::new();
    cpu.reset(&mut bus);

    let mut out: Vec<u8> = Vec::new();
    for _ in 0..instructions {
        cpu.trace_line(&mut out).unwrap();
        // stop where the C++ run loop would: a bad opcode or a detected
        // infinite loop ends it, after the line has already been emitted
        if cpu.step(&mut bus) != emu::cpu::StepResult::Ok {
            break;
        }
    }
    String::from_utf8(out).unwrap()
}

/// The C++ oracle. Not in the tree any more (removed in phase 5 of the
/// conversion): build it from the last commit that had it, in a worktree, and
/// point `EMU_ORACLE` at the binary -- see AGENTS.md. Falls back to the old
/// in-tree location for a worktree that still has one.
fn emu_binary() -> Option<PathBuf> {
    let p = std::env::var_os("EMU_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("build-emu/emu"));
    p.exists().then_some(p)
}

fn cpp_trace(bin: &Path, rom: &[u8], instructions: usize, name: &str) -> String {
    // tests run concurrently in one process, so the paths must be per-case
    let dir = std::env::temp_dir().join(format!("emu-tracediff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rom_path = dir.join(format!("{name}.rom"));
    let trace_path = dir.join(format!("{name}.trace"));
    std::fs::write(&rom_path, rom).unwrap();

    // The C++ cycle limit runs N-1 instructions for -l N, so ask for one more
    // than we want. stdin is a pipe we keep open for the child's lifetime:
    // closing it (or using /dev/null) would EOF the console, shut the cpu
    // thread down early, and yield a short trace that still looks successful.
    let log_path = dir.join(format!("{name}.log"));
    let log = std::fs::File::create(&log_path).unwrap();
    let log_err = log.try_clone().unwrap();

    let mut child = Command::new(bin)
        .args(["-s", "altair680"])
        .arg("-r")
        .arg(&rom_path)
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
    // like a successful run. Taking the handle out of the Child leaves wait()
    // nothing to close.
    let child_stdin = child.stdin.take();
    let status = child.wait().expect("c++ emulator did not exit");
    drop(child_stdin);

    let log_text = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(status.success(), "c++ emulator exited with {status}\n{log_text}");
    std::fs::remove_file(&log_path).ok();

    let trace = std::fs::read_to_string(&trace_path).unwrap();
    std::fs::remove_file(&rom_path).ok();
    std::fs::remove_file(&trace_path).ok();
    trace
}

/// Run a snippet through both cores and require identical traces.
///
/// `expect_lines` guards against the failure mode where both sides truncate:
/// comparing two empty traces would otherwise pass. Pass `None` when the
/// snippet may legitimately stop early (a bad opcode, a runaway jump).
fn assert_traces_match_upto(
    name: &str,
    code: &[u8],
    instructions: usize,
    expect_lines: Option<usize>,
) {
    let Some(bin) = emu_binary() else {
        eprintln!("skipping {name}: no C++ oracle (set EMU_ORACLE)");
        return;
    };

    let rom = rom_image(code);
    let rust = rust_trace(&rom, instructions);
    let cpp = cpp_trace(&bin, &rom, instructions, name);

    assert!(!cpp.is_empty(), "{name}: c++ produced an empty trace");
    assert!(!rust.is_empty(), "{name}: rust produced an empty trace");

    if rust != cpp {
        let mut msg = format!("trace mismatch in {name}\n");
        msg.push_str(&format!("{:<52} | {}\n", "rust", "c++"));
        for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
            let mark = if r == c { ' ' } else { '*' };
            msg.push_str(&format!("{mark}{i:4} {r:<46} | {c}\n"));
            if r != c && i > 0 {
                // enough context to see where it went wrong
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

// Handy opcode aliases, so the snippets read like assembly.
const LDA_IMM: u8 = 0x86;
const LDB_IMM: u8 = 0xc6;
const LDS_IMM: u8 = 0x8e;
const LDX_IMM: u8 = 0xce;
const STA_EXT: u8 = 0xb7;
const LDA_EXT: u8 = 0xb6;
const LDA_DIR: u8 = 0x96;
const STA_DIR: u8 = 0x97;
const LDA_IDX: u8 = 0xa6;
const STA_IDX: u8 = 0xa7;

#[test]
fn loads_and_stores() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x5a,
        LDB_IMM, 0xa5,
        LDX_IMM, 0x12, 0x34,
        LDS_IMM, 0x0f, 0xff,
        STA_EXT, 0x00, 0x40,      // sta $0040
        LDA_IMM, 0x00,            // clobber a, to prove the reload works
        LDA_EXT, 0x00, 0x40,      // lda $0040
        STA_DIR, 0x50,            // sta $50
        LDA_DIR, 0x50,            // lda $50
        LDX_IMM, 0x00, 0x60,
        STA_IDX, 0x05,            // sta 5,x
        LDA_IDX, 0x05,            // lda 5,x
    ];
    assert_traces_match("loads_and_stores", &code, 13);
}

#[test]
fn alu_ops_and_flags() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x7f,
        LDB_IMM, 0x01,
        0x1b,                     // aba   -> 0x80, sets V and N
        LDA_IMM, 0xff,
        0x8b, 0x01,               // adda #1 -> carry out
        LDA_IMM, 0x10,
        0x80, 0x20,               // suba #$20 -> borrow
        LDA_IMM, 0x80,
        0x81, 0x7f,               // cmpa #$7f
        LDA_IMM, 0xf0,
        0x84, 0x3c,               // anda #$3c
        0x8a, 0x0f,               // ora  #$0f
        0x88, 0xff,               // eora #$ff
        0x85, 0x10,               // bita #$10
    ];
    assert_traces_match("alu_ops_and_flags", &code, 16);
}

#[test]
fn adc_and_sbc_carry_chains() {
    #[rustfmt::skip]
    let code = [
        0x0d,                     // sec
        LDA_IMM, 0x00,
        0x89, 0x00,               // adca #0 -> picks up the carry
        0x0d,                     // sec
        LDA_IMM, 0x05,
        0x82, 0x02,               // sbca #2
        0x0c,                     // clc
        LDA_IMM, 0x05,
        0x82, 0x02,               // sbca #2 with carry clear
        LDA_IMM, 0xff,
        0x89, 0xff,               // adca #$ff
    ];
    assert_traces_match("adc_and_sbc_carry_chains", &code, 13);
}

/// The C++ `case ASR:` falls through into `case LSR:`, so asr behaves exactly
/// like lsr *and* reads its operand twice. If this test ever starts failing,
/// someone has "fixed" one side without re-baselining the other.
#[test]
fn shifts_and_rotates_including_the_asr_fallthrough() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x81,
        0x48,                     // asla
        LDA_IMM, 0x81,
        0x47,                     // asra  (behaves as lsra)
        LDA_IMM, 0x81,
        0x44,                     // lsra
        0x0d,                     // sec
        LDA_IMM, 0x40,
        0x49,                     // rola
        0x0d,                     // sec
        LDA_IMM, 0x01,
        0x46,                     // rora
        LDB_IMM, 0x81,
        0x57,                     // asrb  (behaves as lsrb)
    ];
    assert_traces_match("shifts_and_rotates", &code, 15);
}

/// Memory-operand read-modify-write, where the asr double-read is observable
/// as two bus reads rather than one.
#[test]
fn read_modify_write_on_memory() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x81,
        STA_EXT, 0x00, 0x40,
        0x77, 0x00, 0x40,         // asr $0040
        0x78, 0x00, 0x40,         // asl $0040
        0x74, 0x00, 0x40,         // lsr $0040
        0x73, 0x00, 0x40,         // com $0040
        0x70, 0x00, 0x40,         // neg $0040
        0x7c, 0x00, 0x40,         // inc $0040
        0x7a, 0x00, 0x40,         // dec $0040
        0x7d, 0x00, 0x40,         // tst $0040
        0x7f, 0x00, 0x40,         // clr $0040
    ];
    assert_traces_match("read_modify_write_on_memory", &code, 12);
}

#[test]
fn inc_dec_edge_cases() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x7f,
        0x4c,                     // inca -> 0x80, sets V
        LDA_IMM, 0x80,
        0x4a,                     // deca -> 0x7f, sets V
        LDX_IMM, 0x00, 0x01,
        0x09,                     // dex -> 0, sets Z
        0x08,                     // inx
        LDS_IMM, 0x00, 0x01,
        0x34,                     // des -- must NOT touch Z
        0x31,                     // ins
    ];
    assert_traces_match("inc_dec_edge_cases", &code, 11);
}

#[test]
fn branches_taken_and_not_taken() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x00,
        0x27, 0x02,               // beq +2  (taken)
        LDA_IMM, 0xee,            // skipped
        LDA_IMM, 0x01,
        0x26, 0x02,               // bne +2  (taken)
        LDA_IMM, 0xee,            // skipped
        LDA_IMM, 0x80,
        0x2b, 0x02,               // bmi +2  (taken)
        LDA_IMM, 0xee,            // skipped
        0x0d,                     // sec
        0x24, 0x02,               // bcc +2  (not taken)
        LDA_IMM, 0x42,
        0x20, 0x00,               // bra +0
    ];
    assert_traces_match("branches", &code, 14);
}

#[test]
fn signed_branch_conditions() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x05,
        0x81, 0x03,               // cmpa #3  -> a > b signed
        0x2e, 0x02,               // bgt +2 (taken)
        LDA_IMM, 0xee,
        LDA_IMM, 0xfb,            // -5
        0x81, 0x03,               // cmpa #3 -> less than
        0x2d, 0x02,               // blt +2 (taken)
        LDA_IMM, 0xee,
        0x2c, 0x02,               // bge +2
        LDA_IMM, 0xee,
        0x22, 0x02,               // bhi +2
        LDA_IMM, 0x77,
    ];
    assert_traces_match("signed_branches", &code, 14);
}

#[test]
fn stack_push_pull_and_subroutine_calls() {
    #[rustfmt::skip]
    let code = [
        LDS_IMM, 0x0f, 0xff,
        LDA_IMM, 0xaa,
        LDB_IMM, 0xbb,
        0x36,                     // psha
        0x37,                     // pshb
        LDA_IMM, 0x00,
        LDB_IMM, 0x00,
        0x33,                     // pulb
        0x32,                     // pula
        0x8d, 0x02,               // bsr +2 -> lands on the rts below
        0x20, 0x01,               // bra +1 (return target)
        0x39,                     // rts
    ];
    assert_traces_match("stack_and_calls", &code, 13);
}

#[test]
fn jsr_and_rts_via_extended_addressing() {
    // jsr to a subroutine placed later in the rom, which returns
    #[rustfmt::skip]
    let code = [
        LDS_IMM, 0x0f, 0xff,      // ff00
        LDA_IMM, 0x11,            // ff03
        0xbd, 0xff, 0x0b,         // ff05: jsr $ff0b
        LDA_IMM, 0x22,            // ff08
        0x01,                     // ff0a: nop
        0x4c,                     // ff0b: inca  (subroutine)
        0x39,                     // ff0c: rts
    ];
    assert_traces_match("jsr_rts", &code, 10);
}

#[test]
fn transfer_and_condition_code_ops() {
    #[rustfmt::skip]
    let code = [
        LDA_IMM, 0x5a,
        0x16,                     // tab
        LDB_IMM, 0xa5,
        0x17,                     // tba
        LDX_IMM, 0x12, 0x34,
        0x35,                     // txs -> sp = ix - 1
        0x30,                     // tsx -> ix = sp + 1
        0x07,                     // tpa -> a = cc | 0xc0
        LDA_IMM, 0x3f,
        0x06,                     // tap -> cc = a & 0x3f
        0x0b,                     // sev
        0x0a,                     // clv
        0x0f,                     // sei
        0x0e,                     // cli
    ];
    assert_traces_match("transfers_and_cc", &code, 15);
}

#[test]
fn sixteen_bit_compare_and_index_ops() {
    #[rustfmt::skip]
    let code = [
        LDX_IMM, 0x12, 0x34,
        0x8c, 0x12, 0x34,         // cpx #$1234 -> equal
        0x8c, 0x00, 0x01,         // cpx #$0001
        LDS_IMM, 0x20, 0x00,
        0xbf, 0x00, 0x40,         // sts $0040
        0xbe, 0x00, 0x40,         // lds $0040
        0xff, 0x00, 0x42,         // stx $0042
        0xfe, 0x00, 0x42,         // ldx $0042
    ];
    assert_traces_match("sixteen_bit_ops", &code, 8);
}

/// Every opcode the table implements, executed back to back in immediate or
/// implied form where possible. Catches table-entry transcription errors that
/// targeted tests would miss.
#[test]
fn implied_opcode_sweep() {
    let implied = [
        0x01u8, 0x16, 0x17, 0x30, 0x35, 0x07, 0x06, 0x0b, 0x0d, 0x0f, 0x0a, 0x0c, 0x0e, 0x4f,
        0x5f, 0x43, 0x53, 0x40, 0x50, 0x4a, 0x5a, 0x34, 0x09, 0x4c, 0x5c, 0x31, 0x08, 0x48,
        0x58, 0x47, 0x57, 0x44, 0x54, 0x49, 0x59, 0x46, 0x56, 0x4d, 0x5d, 0x11, 0x1b, 0x10,
    ];
    let mut code = vec![LDS_IMM, 0x0f, 0xff, LDA_IMM, 0x96, LDB_IMM, 0x5a];
    code.extend_from_slice(&implied);
    let n = 4 + implied.len();
    assert_traces_match("implied_opcode_sweep", &code, n);
}

/// Execute *every* opcode value, one per synthetic rom, and require both
/// implementations to agree.
///
/// The targeted tests above exercise the A-register and implied forms but
/// leave most of the B-register, indexed and direct families untouched -- and
/// a 180-entry decode table transcribed by hand is exactly where a transposed
/// mode, width or target register hides. Unimplemented opcodes are covered
/// too: one side stopping early shows up as a length difference.
#[test]
fn every_opcode_agrees() {
    if emu_binary().is_none() {
        eprintln!("skipping: no C++ oracle (set EMU_ORACLE)");
        return;
    }

    // Give the opcode under test something non-trivial to work with: a stack,
    // known accumulators, and an index register pointing into ram.
    #[rustfmt::skip]
    const SETUP: [u8; 10] = [
        LDS_IMM, 0x0f, 0xff,
        LDA_IMM, 0x96,
        LDB_IMM, 0x5a,
        LDX_IMM, 0x00, 0x40,
    ];
    const SETUP_INSNS: usize = 4;

    for opcode in 0u16..=0xff {
        let mut code = SETUP.to_vec();
        code.push(opcode as u8);
        // operand bytes for multi-byte forms; 0x40 keeps addresses in ram
        code.extend_from_slice(&[0x00, 0x40, 0x00, 0x40]);

        assert_traces_match_upto(
            &format!("opcode_{opcode:02x}"),
            &code,
            SETUP_INSNS + 4,
            None,
        );
    }
}

/// Gate (b): boot the real MITS monitor rom through both implementations and
/// require identical traces.
///
/// Broad but shallow -- the boot reaches only ~36 distinct PCs before settling
/// into an ACIA poll loop, so this catches gross divergence (a wrong reset
/// vector, a broken decode, a mis-sized rom bank) rather than opcode
/// semantics. The snippet tests above are what cover behaviour.
#[test]
fn real_monitor_rom_boot_matches() {
    let Some(bin) = emu_binary() else {
        eprintln!("skipping: no C++ oracle (set EMU_ORACLE)");
        return;
    };
    let rom_path = Path::new(emu::system::altair680::DEFAULT_ROM);
    if !rom_path.exists() {
        eprintln!("skipping: {} not present", rom_path.display());
        return;
    }

    const N: usize = 20_000;

    // rust side: the real machine, driven through the registry
    let (_tx, rx) = std::sync::mpsc::channel();
    let endpoint = emu::console::ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let desc = emu::system::registry::find("altair680").unwrap();
    let machine = (desc.factory)(rom_path, endpoint, "").expect("failed to build altair680");

    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut emu = emu::emulator::Emulator::new(machine.cpu, machine.bus, shutdown);
    emu.set_cycle_limit(Some(N as i64 + 1));
    let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Shared {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    emu.set_trace(Some(Box::new(Shared(std::sync::Arc::clone(&sink)))));
    emu.reset();
    emu.run();
    let rust = String::from_utf8(sink.lock().unwrap().clone()).unwrap();

    // c++ side: same rom, same instruction count
    let rom = std::fs::read(rom_path).unwrap();
    let cpp = cpp_trace(&bin, &rom, N, "real_boot");

    // Both lengths are asserted before any comparison: zip() stops at the
    // shorter side, so a truncated c++ trace would otherwise make this pass
    // without comparing anything.
    assert_eq!(rust.lines().count(), N, "rust trace length");
    assert_eq!(cpp.lines().count(), N, "c++ trace length");
    for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
        assert_eq!(r, c, "divergence at instruction {i}");
    }
}

/// Pin the *bus-level* consequence of the ASR fallthrough.
///
/// `read_modify_write_on_memory` runs against ram, where a doubled read is
/// indistinguishable from a single one -- it checks the result and flags, not
/// the access pattern. This counts reads per address, which is what makes the
/// quirk observable on a device register.
#[test]
fn asr_reads_its_operand_twice_but_lsr_reads_it_once() {
    struct CountingBus {
        ram: Vec<u8>,
        reads: std::collections::HashMap<u16, usize>,
    }
    impl Bus for CountingBus {
        fn read8(&mut self, addr: u32) -> u8 {
            let a = (addr & 0xffff) as u16;
            *self.reads.entry(a).or_insert(0) += 1;
            self.ram[a as usize]
        }
        fn write8(&mut self, addr: u32, val: u8) {
            self.ram[(addr & 0xffff) as usize] = val;
        }
    }

    // asr $0040 / lsr $0040, each starting from a fresh bus
    let counts = [0x77u8, 0x74].map(|opcode| {
        let mut bus = CountingBus { ram: vec![0; 0x10000], reads: Default::default() };
        bus.ram[0x0040] = 0x81;
        // reset vector -> 0x0100, where we place the instruction
        bus.ram[0xfffe] = 0x01;
        bus.ram[0xffff] = 0x00;
        bus.ram[0x0100] = opcode;
        bus.ram[0x0101] = 0x00;
        bus.ram[0x0102] = 0x40;

        let mut cpu = Cpu6800::new();
        cpu.reset(&mut bus);
        cpu.step(&mut bus);
        bus.reads.get(&0x0040).copied().unwrap_or(0)
    });

    assert_eq!(counts[0], 2, "asr must read its operand twice (the LSR fallthrough)");
    assert_eq!(counts[1], 1, "lsr must read its operand once");
}

/// An unimplemented opcode must stop the core the same way on both sides. The
/// C++ prints to stderr and ends its run loop; the trace simply stops.
#[test]
fn bad_opcode_halts() {
    let code = [LDA_IMM, 0x01, 0x00 /* not a valid 6800 opcode */];
    let rom = rom_image(&code);
    let mut bus = Altair680TestBus::new(&rom);
    let mut cpu = Cpu6800::new();
    cpu.reset(&mut bus);
    assert_eq!(cpu.step(&mut bus), emu::cpu::StepResult::Ok);
    assert_eq!(cpu.step(&mut bus), emu::cpu::StepResult::BadOpcode);
}
