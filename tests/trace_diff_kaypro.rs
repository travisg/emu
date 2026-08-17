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
//! Cross-validate the Rust Kaypro machine against the C++ oracle.
//!
//! The Z80 core is already covered by `trace_diff_z80.rs`; what's under test
//! here is the *machine*: the bank switch, the video window, the `0x1c` latch,
//! the WD1793 and the SIO, all as observed through the CPU registers. A
//! synthetic 4K boot rom drives each port directly, and the real rom is booted
//! far enough to reach CP/M (restore, seeks, read-address, sector reads).
//!
//! What this can *not* see: the rendered screen. The oracle emits registers
//! only, so a wrong row stride or a mis-drawn glyph produces an identical
//! trace. The visual gate for that is manual (boot it, look at the window).
//!
//! Requirements, all checked at runtime and skipped-not-failed if absent: the
//! C++ binary at `build-emu/emu`, the kaypro roms via the `roms` symlink, and
//! the floppy image `mbasic-games.img` in the crate root (the C++ loads it from
//! the cwd, so the Rust side does the same). The C++ child runs with the SDL
//! dummy video driver so no window flashes up during `cargo test`.

use emu::console::ConsoleEndpoint;
use emu::cpu::z80::CpuZ80;
use emu::cpu::Cpu;
use emu::system::kaypro::{Kaypro, DEFAULT_FLOPPY, DEFAULT_ROM, VIDEO_ROM};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ROM_SIZE: usize = 4 * 1024;

fn emu_binary() -> Option<PathBuf> {
    let p = Path::new("build-emu/emu").to_path_buf();
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// Everything a run needs beyond the C++ binary. Returns None (and says why)
/// when the environment can't run the gate.
fn prerequisites() -> Option<PathBuf> {
    let bin = emu_binary();
    if bin.is_none() {
        eprintln!("skipping: build-emu/emu not built");
    }
    for p in [VIDEO_ROM, DEFAULT_FLOPPY] {
        if !Path::new(p).exists() {
            eprintln!("skipping: {p} not present in the crate root");
            return None;
        }
    }
    bin
}

/// Per-case scratch directory. `cargo test` runs tests concurrently in one
/// process, so shared filenames would race.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emu-kaypro-tracediff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn rust_trace(rom_path: &Path, instructions: usize) -> String {
    // no keyboard attached: the receiver is disconnected, so the SIO's
    // keyboard channel stays empty and every port reads deterministically
    let (_tx, rx) = std::sync::mpsc::channel();
    let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let (mut bus, _display) =
        Kaypro::new(rom_path, Path::new(VIDEO_ROM), Path::new(DEFAULT_FLOPPY), console)
            .expect("failed to build the kaypro machine");

    let mut cpu = CpuZ80::new();
    cpu.reset(&mut bus);

    let mut out: Vec<u8> = Vec::new();
    for _ in 0..instructions {
        cpu.trace_line(&mut out).unwrap();
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
        .args(["-s", "kaypro"])
        .arg("-r")
        .arg(rom_path)
        .arg("-l")
        .arg((instructions + 1).to_string())
        .arg("--trace")
        .arg(&trace_path)
        // headless: no window during the test run. The C++ tolerates the
        // missing renderer and the trace doesn't depend on it.
        .env("SDL_VIDEODRIVER", "dummy")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .expect("failed to spawn the c++ emulator");

    // Hold stdin open for the child's whole life; see trace_diff_z80.rs.
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

fn assert_same(name: &str, rust: &str, cpp: &str, expect_lines: usize) {
    assert_eq!(rust.lines().count(), expect_lines, "{name}: rust trace is short");
    assert_eq!(cpp.lines().count(), expect_lines, "{name}: c++ trace is short");
    if rust != cpp {
        for (i, (r, c)) in rust.lines().zip(cpp.lines()).enumerate() {
            if r != c {
                panic!("{name}: trace diverges at instruction {i}\n  rust: {r}\n  c++:  {c}");
            }
        }
    }
}

/// Assemble a snippet into a 4K rom image, nop-filled. The Z80 starts at 0,
/// which is the rom while the latch has bank 1 selected (its reset state).
fn rom_image(code: &[u8]) -> Vec<u8> {
    assert!(code.len() <= ROM_SIZE, "snippet does not fit in the rom");
    let mut rom = vec![0x00u8; ROM_SIZE];
    rom[..code.len()].copy_from_slice(code);
    rom
}

/// Drive every port the machine decodes, with the results landing in A so
/// they show up in the trace: latch readback with and without a drive
/// selected, restore/seek/read-sector/read-address on the FDC (streaming a
/// real sector out of the image, and the `0xe5` filler for an out-of-range
/// track), the SIO's status and reset, a bank switch to ram and back, and a
/// video ram round trip.
#[test]
fn ports_and_banking_match() {
    let Some(bin) = prerequisites() else { return };

    #[rustfmt::skip]
    let code: Vec<u8> = vec![
        0x31, 0xff, 0xff,       // ld sp, $ffff
        0xdb, 0x1c,             // in a, ($1c)      -- reset latch readback
        0xdb, 0x10,             // in a, ($10)      -- fdc status, no drive selected
        0x3e, 0x83,             // ld a, $83
        0xd3, 0x1c,             // out ($1c), a     -- bank 1, no drive
        0xdb, 0x10,             // in a, ($10)      -- not ready
        0x3e, 0x82,             // ld a, $82
        0xd3, 0x1c,             // out ($1c), a     -- bank 1, drive A
        0xdb, 0x10,             // in a, ($10)      -- ready
        0xaf,                   // xor a
        0xd3, 0x10,             // out ($10), a     -- restore
        0xdb, 0x1c,             // in a, ($1c)      -- intrq low
        0xdb, 0x10,             // in a, ($10)      -- track 0, clears intrq
        0xdb, 0x1c,             // in a, ($1c)
        0xdb, 0x11,             // in a, ($11)      -- track register
        // seek to track 2 via the data register
        0x3e, 0x02,             // ld a, 2
        0xd3, 0x13,             // out ($13), a
        0x3e, 0x10,             // ld a, $10
        0xd3, 0x10,             // out ($10), a     -- seek
        0xdb, 0x11,             // in a, ($11)
        // read address
        0x3e, 0x05,             // ld a, 5
        0xd3, 0x12,             // out ($12), a     -- sector 5
        0x3e, 0xc0,             // ld a, $c0
        0xd3, 0x10,             // out ($10), a     -- read address
        0x06, 0x06,             // ld b, 6
        0xdb, 0x13,             // in a, ($13)      -- (loop) 6 id bytes
        0x10, 0xfc,             // djnz -4
        0xdb, 0x1c,             // in a, ($1c)      -- drq high again, intrq low
        // read a real sector: track 2 sector 5 of the image, all 512 bytes
        0x3e, 0x88,             // ld a, $88
        0xd3, 0x10,             // out ($10), a     -- read sector
        0xdb, 0x1c,             // in a, ($1c)      -- drq low
        0xdb, 0x10,             // in a, ($10)      -- busy + drq
        0x21, 0x00, 0x80,       // ld hl, $8000
        0x06, 0x00,             // ld b, 0          -- 256 twice
        0xdb, 0x13,             // in a, ($13)      -- (loop)
        0x77,                   // ld (hl), a
        0x23,                   // inc hl
        0x10, 0xfa,             // djnz -6
        0xdb, 0x13,             // in a, ($13)      -- (loop)
        0x77,                   // ld (hl), a
        0x23,                   // inc hl
        0x10, 0xfa,             // djnz -6
        0xdb, 0x1c,             // in a, ($1c)      -- drq high, intrq low
        0xdb, 0x10,             // in a, ($10)      -- idle
        0xdb, 0x13,             // in a, ($13)      -- past the end: data register
        // out-of-range track: filler
        0x3e, 0x30,             // ld a, 48
        0xd3, 0x11,             // out ($11), a     -- track 48
        0x3e, 0x80,             // ld a, $80
        0xd3, 0x10,             // out ($10), a     -- read sector
        0xdb, 0x13,             // in a, ($13)      -- $e5
        // force interrupt, both flavours
        0x3e, 0xd0,             // ld a, $d0
        0xd3, 0x10,             // out ($10), a
        0xdb, 0x1c,             // in a, ($1c)
        0x3e, 0xd8,             // ld a, $d8
        0xd3, 0x10,             // out ($10), a
        0xdb, 0x1c,             // in a, ($1c)
        // the latch's bits 6-7 are inputs: a write with them set must not
        // read back through them while intrq is pending
        0x3e, 0xc2,             // ld a, $c2
        0xd3, 0x1c,             // out ($1c), a
        0xdb, 0x1c,             // in a, ($1c)      -- $82, not $c2
        0x3e, 0x82,             // ld a, $82
        0xd3, 0x1c,             // out ($1c), a
        // sio: status, empty data, register pointer, channel reset
        0xdb, 0x07,             // in a, ($07)      -- rr0 B
        0xdb, 0x05,             // in a, ($05)      -- empty fifo
        0x3e, 0x01,             // ld a, 1
        0xd3, 0x07,             // out ($07), a     -- point at rr1
        0xdb, 0x07,             // in a, ($07)      -- rr1
        0x3e, 0x18,             // ld a, $18
        0xd3, 0x07,             // out ($07), a     -- channel reset
        0xdb, 0x07,             // in a, ($07)
        0xdb, 0x06,             // in a, ($06)      -- rr0 A
        0x3e, 0x41,             // ld a, 'A'
        0xd3, 0x04,             // out ($04), a     -- tx on A, into the ether
        0xdb, 0x06,             // in a, ($06)
    ];

    // The rest has to run from ram: switching to bank 0 takes the rom -- and
    // the code in it -- out of the address space, so a snippet that does it
    // in place just falls into whatever ram holds (nops). Copy the routine to
    // $9000 and jump there.
    #[rustfmt::skip]
    let ram_routine: Vec<u8> = vec![
        // bank switch: ram is under the rom and video windows
        0x3e, 0x00,             // ld a, 0
        0xd3, 0x1c,             // out ($1c), a     -- bank 0
        0x3a, 0x00, 0x00,       // ld a, ($0000)    -- ram, zero
        0x3e, 0x5a,             // ld a, $5a
        0x32, 0x00, 0x00,       // ld ($0000), a    -- lands in ram
        0x32, 0x00, 0x30,       // ld ($3000), a    -- lands in ram
        0x3e, 0x80,             // ld a, $80
        0xd3, 0x1c,             // out ($1c), a     -- bank 1
        0x3a, 0x00, 0x00,       // ld a, ($0000)    -- rom: $31
        0x3a, 0x00, 0x30,       // ld a, ($3000)    -- video ram: 0
        0x3e, 0x99,             // ld a, $99
        0x32, 0x00, 0x00,       // ld ($0000), a    -- dropped
        0x3a, 0x00, 0x00,       // ld a, ($0000)    -- still $31
        0x3e, 0x4b,             // ld a, 'K'
        0x32, 0x85, 0x30,       // ld ($3085), a    -- video, row 1 col 5
        0xaf,                   // xor a
        0x3a, 0x85, 0x30,       // ld a, ($3085)
        0x3e, 0x00,             // ld a, 0
        0xd3, 0x1c,             // out ($1c), a     -- bank 0 again
        0x3a, 0x00, 0x30,       // ld a, ($3000)    -- the ram write: $5a
        0x3a, 0x00, 0x00,       // ld a, ($0000)    -- $5a
        // type I context: poll status long enough for the index pulse to
        // toggle a few times
        0x3e, 0x82,             // ld a, $82
        0xd3, 0x1c,             // out ($1c), a
        0xaf,                   // xor a
        0xd3, 0x10,             // out ($10), a     -- restore
        0x06, 0x00,             // ld b, 0
        0xdb, 0x10,             // in a, ($10)      -- (loop) 256 status reads
        0x10, 0xfc,             // djnz -4
        // unmodelled ports, for the decode
        0xd3, 0x00,             // out ($00), a
        0xd3, 0x08,             // out ($08), a
        0xd3, 0x0c,             // out ($0c), a
        0xd3, 0x14,             // out ($14), a
        0xd3, 0x1d,             // out ($1d), a
        0xdb, 0x00,             // in a, ($00)
        0xdb, 0x08,             // in a, ($08)
        0xdb, 0x14,             // in a, ($14)
        0xdb, 0x1d,             // in a, ($1d)
        // ...and off the end into ram, which is zero: nops
    ];

    const RAM_ROUTINE: u16 = 0x9000;
    let src = (code.len() + 14) as u16; // the routine follows this 14-byte trampoline
    let len = ram_routine.len() as u16;
    let mut code = code;
    #[rustfmt::skip]
    code.extend_from_slice(&[
        0x21, src as u8, (src >> 8) as u8,                          // ld hl, src
        0x11, RAM_ROUTINE as u8, (RAM_ROUTINE >> 8) as u8,          // ld de, $9000
        0x01, len as u8, (len >> 8) as u8,                          // ld bc, len
        0xed, 0xb0,                                                 // ldir
        0xc3, RAM_ROUTINE as u8, (RAM_ROUTINE >> 8) as u8,          // jp $9000
    ]);
    code.extend_from_slice(&ram_routine);

    // Long enough to run the whole snippet (the sector loop alone is ~2,000
    // instructions) and well into the nop tail, on both sides.
    const N: usize = 4_000;
    let rom_path = scratch("ports.rom");
    std::fs::write(&rom_path, rom_image(&code)).unwrap();

    let rust = rust_trace(&rom_path, N);
    let cpp = cpp_trace(&bin, &rom_path, N, "ports");
    std::fs::remove_file(&rom_path).ok();
    assert_same("ports_and_banking", &rust, &cpp, N);

    // sanity: the snippet really ran through the ram routine and off its end
    // into the nop tail, so nothing above was cut short by a stray abort (or
    // by the bank switch pulling the rom out from under the code)
    let last = rust.lines().last().unwrap();
    let pc = u16::from_str_radix(&last[3..7], 16).unwrap();
    assert!(
        pc > RAM_ROUTINE + len && pc < 0xa000,
        "snippet did not run to completion: {last}"
    );
}

/// Gate: boot the real rom through both implementations to the CP/M prompt
/// and require identical traces. Covers restore, seeks, read-address and the
/// sector reads that load CP/M, plus the delay loops around them.
#[test]
fn real_rom_boot_matches() {
    let Some(bin) = prerequisites() else { return };
    let rom_path = Path::new(DEFAULT_ROM);
    if !rom_path.exists() {
        eprintln!("skipping: {} not present", rom_path.display());
        return;
    }

    // CP/M is up and at its prompt well before this
    const N: usize = 3_000_000;

    let rust = rust_trace(rom_path, N);
    let cpp = cpp_trace(&bin, rom_path, N, "real_rom_boot");
    assert_same("real_rom_boot", &rust, &cpp, N);
}
