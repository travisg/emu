// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Raytheon 703 as a WebAssembly module, for a page that is its
//! frontend: the machine REX boots on, with its front panel and disc rack,
//! driven through the tree's own run loop one animation frame at a time
//! (`Emulator::run_cycles`).
//!
//! The interface is plain exported functions over the module's memory, so
//! the module imports nothing and needs no binding generator. Bytes go in
//! through `staging`: the host asks for a buffer, copies an image into it,
//! and the next `boot_*` takes it. Teletype output comes back the same way,
//! `drain` then `drained_ptr`. The host has to fetch `memory.buffer` afresh
//! after any call, since a call can grow the memory and detach the old one.
//!
//! The machine is a panel machine: it powers on halted, and `panel(0)` is
//! RUN. Nothing paces it; the host passes the cycles one frame of real time
//! is worth (`CLOCK_HZ` / frame rate), and the devices count their periods
//! in those cycles, so the teletype types at ten characters a second of
//! machine time.
//!
//! Not here, deliberately: the debug port, `--serial`, snapshots and disc
//! writes that outlive the page.

use emu::console::{ConsoleEndpoint, DiscRackState, LampSnapshot, PanelCommand, PanelState};
use emu::cpu::ray703::Cpu703;
use emu::emulator::{Control, Emulator, StopPolicy};
use emu::system::ray703::Ray703;
use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

/// The machine's clock, for the host to divide by its frame rate.
#[no_mangle]
pub extern "C" fn clock_hz() -> u32 {
    emu::cpu::ray703::CLOCK_HZ as u32
}

/// Teletype output, collected until the host drains it.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Machine {
    emu: Emulator,
    keys: Sender<u8>,
    control: Sender<Control>,
    out: Sink,
    drained: Vec<u8>,
    panel: PanelState,
    rack: DiscRackState,
    /// The PC row's accumulators at the end of the last slice, and each
    /// lamp's share of the cycles in it.
    prev: LampSnapshot,
    duty: [f32; 16],
}

thread_local! {
    static STAGE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static MACHINE: RefCell<Option<Machine>> = const { RefCell::new(None) };
}

/// Run `f` on the machine; a call before any boot is the host's mistake,
/// and a trap says so.
fn with<R>(f: impl FnOnce(&mut Machine) -> R) -> R {
    MACHINE.with_borrow_mut(|m| f(m.as_mut().expect("no machine booted")))
}

/// A buffer of `len` bytes for the next `boot_*` to take.
#[no_mangle]
pub extern "C" fn staging(len: usize) -> *mut u8 {
    STAGE.with_borrow_mut(|s| {
        *s = vec![0; len];
        s.as_mut_ptr()
    })
}

/// Build the machine around `bus`, halted at the panel.
fn boot(make: impl FnOnce(ConsoleEndpoint) -> Option<Ray703>, time_of_day: u32) -> bool {
    let (keys, rx) = mpsc::channel();
    let out = Sink::default();
    let Some(mut bus) = make(ConsoleEndpoint::new(rx, Box::new(out.clone()))) else {
        return false;
    };
    let rack = DiscRackState::new();
    bus.attach_disc_rack(rack.clone());
    bus.set_time_of_day(time_of_day);
    let panel = PanelState::new();
    let mut cpu = Cpu703::new();
    cpu.attach_panel(panel.clone());
    let mut emu = Emulator::new(Box::new(cpu), Box::new(bus), Arc::new(AtomicBool::new(false)));
    let (control, control_rx) = mpsc::channel();
    emu.set_control(Some(control_rx));
    emu.set_stop_policy(StopPolicy::Halt);
    emu.set_panel_state(Some(Box::new(panel.clone())));
    emu.reset();
    emu.set_halted(true);
    let m = Machine {
        emu,
        keys,
        control,
        out,
        drained: Vec::new(),
        panel,
        rack,
        prev: LampSnapshot::default(),
        duty: [0.0; 16],
    };
    MACHINE.with_borrow_mut(|slot| *slot = Some(m));
    true
}

/// The staged bytes as a flat core image from word 0. `time_of_day` is
/// the clock's count, tenths of a millisecond since midnight.
#[no_mangle]
pub extern "C" fn boot_core(time_of_day: u32) -> u32 {
    let image = STAGE.with_borrow_mut(std::mem::take);
    boot(|c| Some(Ray703::with_core_image(&image, c)), time_of_day) as u32
}

/// The staged bytes as the platter on unit 0, and LOAD pressed: how REX
/// boots. 0 if they are not exactly one platter.
#[no_mangle]
pub extern "C" fn boot_disc(time_of_day: u32) -> u32 {
    let image = STAGE.with_borrow_mut(std::mem::take);
    let make = |c| {
        let mut bus = Ray703::bare(c);
        (bus.mount_disc_bytes(0, image) && bus.press_load()).then_some(bus)
    };
    boot(make, time_of_day) as u32
}

/// A panel switch: 0 RUN, 1 HALT, 2 SINGLE COMMAND, 3 RESET.
#[no_mangle]
pub extern "C" fn panel(switch: u32) {
    let cmd = match switch {
        0 => PanelCommand::Run,
        1 => PanelCommand::Halt,
        2 => PanelCommand::SingleCommand,
        _ => PanelCommand::Reset,
    };
    with(|m| m.control.send(Control::Panel(cmd)).ok());
}

/// Run about `cycles` machine cycles, or return at once if halted; 0 while
/// the machine goes on, 1 once its run has ended for good.
#[no_mangle]
pub extern "C" fn run(cycles: u32) -> u32 {
    with(|m| {
        let ended = m.emu.run_cycles(cycles as u64).is_some();
        // Each lamp's on-time over the slice, as the SDL panel computes it:
        // a halted machine accumulates nothing and shows the bits it holds.
        let now = m.panel.snapshot_pc();
        let dc = now.cycles - m.prev.cycles;
        let pc = m.panel.program_counter();
        for (i, d) in m.duty.iter_mut().enumerate() {
            *d = if dc == 0 {
                ((pc >> (15 - i)) & 1) as f32
            } else {
                (now.bits[i] - m.prev.bits[i]) as f32 / dc as f32
            };
        }
        m.prev = now;
        ended as u32
    })
}

/// A key at the console teletype.
#[no_mangle]
pub extern "C" fn key(c: u8) {
    with(|m| m.keys.send(c).ok());
}

/// Take the teletype output since the last drain; its length, the bytes
/// at `drained_ptr` until the next call.
#[no_mangle]
pub extern "C" fn drain() -> usize {
    with(|m| {
        m.drained = std::mem::take(&mut *m.out.0.lock().unwrap());
        m.drained.len()
    })
}

#[no_mangle]
pub extern "C" fn drained_ptr() -> *const u8 {
    with(|m| m.drained.as_ptr())
}

/// The PROGRAM COUNTER row as it stands.
#[no_mangle]
pub extern "C" fn pc_lamps() -> u32 {
    with(|m| m.panel.program_counter() as u32)
}

/// Indicator `i`'s duty over the last slice, indicator 0 the leftmost.
#[no_mangle]
pub extern "C" fn pc_duty(i: u32) -> f32 {
    with(|m| m.duty.get(i as usize).copied().unwrap_or(0.0))
}

/// The HALT switch-indicator's lens.
#[no_mangle]
pub extern "C" fn halted() -> u32 {
    with(|m| m.panel.halted() as u32)
}

/// A disc unit's ONLINE lamp.
#[no_mangle]
pub extern "C" fn rack_online(unit: u32) -> u32 {
    with(|m| m.rack.snapshot().online.get(unit as usize).copied().unwrap_or(false) as u32)
}

/// How many reads a disc unit has completed.
#[no_mangle]
pub extern "C" fn rack_reads(unit: u32) -> u32 {
    with(|m| m.rack.snapshot().read_ops.get(unit as usize).copied().unwrap_or(0) as u32)
}
