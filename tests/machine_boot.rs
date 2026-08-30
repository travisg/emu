// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Boot a registry-built machine from an Intel HEX image.
//!
//! The core-level claims here are covered by the in-module tests in
//! `src/cpu/`; what this file adds is the path a real run takes and they do
//! not -- `registry::find` to a factory, a `.hex` off disk through
//! `rom::load_ihex`, and the boxed `Cpu`/`Bus` trait objects the `Machine`
//! hands back.

use emu::console::ConsoleEndpoint;
use emu::cpu::StepResult;
use emu::system::registry;

/// Start of the rom bank in the System09 map.
const ROM_BASE: u16 = 0xc000;

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

/// Build a .hex image: the code at 0xc000, reset vector at 0xfffe.
fn hex_image(code: &[u8]) -> String {
    let mut s = String::new();
    for (i, chunk) in code.chunks(16).enumerate() {
        s.push_str(&hex_record(ROM_BASE + (i * 16) as u16, 0x00, chunk));
    }
    s.push_str(&hex_record(0xfffe, 0x00, &[(ROM_BASE >> 8) as u8, ROM_BASE as u8]));
    s.push_str(&hex_record(0, 0x01, &[]));
    s
}

const LDA_IMM: u8 = 0x86;
const LDB_IMM: u8 = 0xc6;
const LDX_IMM: u8 = 0x8e;
const LDS_IMM_H: u8 = 0x10; // 0x10 0xce
const LDU_IMM: u8 = 0xce;

/// Stack + accumulators, so the code that follows has something to work with.
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

/// The whole build-and-run path, ending on a deliberate bad opcode so the run
/// stops on a known instruction rather than on whatever the rom fill decodes
/// to.
#[test]
fn a_registry_built_6809_runs_a_hex_image() {
    // 0x01 is not a valid 6809 opcode
    let code = with_setup(&[0x01]);
    let dir = std::env::temp_dir().join(format!("emu-machine-boot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hex_path = dir.join("badop.hex");
    std::fs::write(&hex_path, hex_image(&code)).unwrap();

    let (_tx, rx) = std::sync::mpsc::channel();
    let endpoint = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
    let desc = registry::find("6809").unwrap();
    let machine = (desc.factory)(&hex_path, endpoint, "", &registry::MachineOpts::default()).unwrap();
    std::fs::remove_file(&hex_path).ok();
    std::fs::remove_dir(&dir).ok();

    let mut cpu = machine.cpu;
    let mut bus = machine.bus;
    cpu.reset(&mut *bus);
    for _ in 0..SETUP_INSNS {
        assert_eq!(cpu.step(&mut *bus), StepResult::Ok);
    }
    assert_eq!(cpu.step(&mut *bus), StepResult::BadOpcode);
}
