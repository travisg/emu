// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Raytheon 703 as a WebAssembly module, for the page in `page/` that
//! is its frontend: the machine REX boots on, with its front panel, disc
//! rack and console teletype, driven through the tree's own run loop one
//! frame at a time (`Emulator::run_cycles`). The panel and the paper are
//! the windows' own views (`PanelView`, `PaperView`), drawn into RGBA
//! framebuffers the page puts on its canvases as they are.
//!
//! The interface is plain exported functions over the module's memory, so
//! the module imports nothing and needs no binding generator. Bytes go in
//! through `staging`: the host asks for a buffer, copies an image into it,
//! and the next `boot_*` takes it. A pixel buffer or the teletype's output
//! comes back as a pointer into the module's memory, good until the next
//! call; the host has to take `memory.buffer` afresh after any call, since
//! a call can grow the memory and detach the old one.
//!
//! The machine is a panel machine: it powers on halted, and RUN on the
//! panel starts it. Nothing paces it; the host passes the cycles a frame
//! of real time is worth (`clock_hz` / 60), and the devices count their
//! periods in those cycles, so the teletype types at ten characters a
//! second of machine time. The panel's lamps are tuned for sixty frames a
//! second, so the host renders the panel once for each such slice.
//!
//! Not here, deliberately: the debug port, `--serial`, snapshots and disc
//! writes that outlive the page.

use emu::console::panel703::PanelView;
use emu::console::paper::Paper;
use emu::console::paper_view::PaperView;
use emu::console::surface::Framebuffer;
use emu::console::{ConsoleEndpoint, DiscRackState, PanelControl, PanelDisplay, PanelState};
use emu::cpu::ray703::Cpu703;
use emu::emulator::{Emulator, StopPolicy};
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

/// The console's output: printed on the paper, and kept for `drain` too.
#[derive(Clone)]
struct Console {
    paper: Paper,
    kept: Arc<Mutex<Vec<u8>>>,
}

impl Write for Console {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.paper.write_all(b)?;
        self.kept.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Machine {
    emu: Emulator,
    keys: Sender<u8>,
    kept: Arc<Mutex<Vec<u8>>>,
    drained: Vec<u8>,
    panel: PanelState,
    rack: DiscRackState,
    view: PanelView,
    view_fb: Framebuffer,
    paper: PaperView,
    paper_fb: Framebuffer,
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

/// Build the machine around the bus `make` returns, halted at the panel.
/// `time_of_day` is the clock's count, tenths of a millisecond since
/// midnight; `fast_io` is `--fast-io`.
fn boot(make: impl FnOnce(ConsoleEndpoint) -> Option<Ray703>, time_of_day: u32, fast_io: u32) -> u32 {
    let (keys, rx) = mpsc::channel();
    let paper = Paper::new();
    let kept = Arc::new(Mutex::new(Vec::new()));
    let console = Console { paper: paper.clone(), kept: kept.clone() };
    let Some(mut bus) = make(ConsoleEndpoint::new(rx, Box::new(console))) else {
        return 0;
    };
    let rack = DiscRackState::new();
    bus.attach_disc_rack(rack.clone());
    bus.set_time_of_day(time_of_day);
    if fast_io != 0 {
        bus.set_fast_io();
    }
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
    let view = PanelView::new(PanelDisplay {
        panel: panel.clone(),
        control: PanelControl(control),
        rack: rack.clone(),
    });
    let m = Machine {
        emu,
        keys,
        kept,
        drained: Vec::new(),
        panel,
        rack,
        view,
        view_fb: Framebuffer::new(PanelView::WIDTH, PanelView::HEIGHT),
        paper: PaperView::new(paper),
        paper_fb: Framebuffer::new(PaperView::WIDTH, PaperView::HEIGHT),
    };
    MACHINE.with_borrow_mut(|slot| *slot = Some(m));
    1
}

/// The staged bytes as a flat core image from word 0.
#[no_mangle]
pub extern "C" fn boot_core(time_of_day: u32, fast_io: u32) -> u32 {
    let image = STAGE.with_borrow_mut(std::mem::take);
    boot(|c| Some(Ray703::with_core_image(&image, c)), time_of_day, fast_io)
}

/// The staged bytes as the platter on unit 0, and LOAD pressed: how REX
/// boots. 0 if they are not exactly one platter.
#[no_mangle]
pub extern "C" fn boot_disc(time_of_day: u32, fast_io: u32) -> u32 {
    let image = STAGE.with_borrow_mut(std::mem::take);
    let make = |c| {
        let mut bus = Ray703::bare(c);
        (bus.mount_disc_bytes(0, image) && bus.press_load()).then_some(bus)
    };
    boot(make, time_of_day, fast_io)
}

/// Run about `cycles`, or return at once if halted; 0 while the machine
/// goes on, 1 once its run has ended for good.
#[no_mangle]
pub extern "C" fn run(cycles: u32) -> u32 {
    with(|m| m.emu.run_cycles(cycles as u64).is_some() as u32)
}

/// A key at the console teletype, as the Model 33 sends it.
#[no_mangle]
pub extern "C" fn key(c: u8) {
    with(|m| m.keys.send(c).ok());
}

// -- the panel --------------------------------------------------------------

#[no_mangle]
pub extern "C" fn panel_width() -> u32 {
    PanelView::WIDTH
}

#[no_mangle]
pub extern "C" fn panel_height() -> u32 {
    PanelView::HEIGHT
}

/// Draw the panel, advancing its lamps a frame; the RGBA pixels.
#[no_mangle]
pub extern "C" fn panel_render() -> *const u8 {
    with(|m| {
        m.view.render(&mut m.view_fb);
        m.view_fb.pixels.as_ptr()
    })
}

/// A click at (x, y) in the panel's pixels; `back` the other button.
#[no_mangle]
pub extern "C" fn panel_click(x: i32, y: i32, back: u32) {
    with(|m| m.view.click(x, y, back != 0));
}

/// A key at the panel: Tab turns the knob, 1 to 4 flip the SENSE toggles.
#[no_mangle]
pub extern "C" fn panel_key(c: u32) {
    with(|m| m.view.key(char::from_u32(c).unwrap_or('\0')));
}

// -- the paper --------------------------------------------------------------

#[no_mangle]
pub extern "C" fn paper_width() -> u32 {
    PaperView::WIDTH
}

#[no_mangle]
pub extern "C" fn paper_height() -> u32 {
    PaperView::HEIGHT
}

/// The paper's RGBA pixels if they changed since the last call, else null.
#[no_mangle]
pub extern "C" fn paper_render() -> *const u8 {
    with(|m| {
        if !m.paper.take_redraw() {
            return std::ptr::null();
        }
        m.paper.render(&mut m.paper_fb);
        m.paper_fb.pixels.as_ptr()
    })
}

/// Roll the platen: positive is back into the scrollback.
#[no_mangle]
pub extern "C" fn paper_wheel(dy: i32) {
    with(|m| m.paper.wheel(dy));
}

// -- for a host checking the machine ---------------------------------------

/// Take the teletype output since the last drain; its length, the bytes
/// at `drained_ptr` until the next call.
#[no_mangle]
pub extern "C" fn drain() -> usize {
    with(|m| {
        m.drained = std::mem::take(&mut *m.kept.lock().unwrap());
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

/// The HALT switch-indicator's lens.
#[no_mangle]
pub extern "C" fn halted() -> u32 {
    with(|m| m.panel.halted() as u32)
}

/// How many reads a disc unit has completed.
#[no_mangle]
pub extern "C" fn rack_reads(unit: u32) -> u32 {
    with(|m| m.rack.snapshot().read_ops.get(unit as usize).copied().unwrap_or(0) as u32)
}
