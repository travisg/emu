// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The emulator's machines with front panels as a WebAssembly module, for
//! the pages in `page/` that are their frontends: the Raytheon 703 REX
//! boots on, with its panel, disc rack, console teletype and the three
//! teletypes on its multiplexer, and the Altair 8800 with its panel and
//! its console teletype. Each is driven through the tree's own run loop
//! one frame at a time (`Emulator::run_cycles`); the panels and the paper
//! are the windows' own views (`PanelView`, `PanelView8800`, `PaperView`),
//! drawn into RGBA framebuffers the page puts on its canvases as they are.
//!
//! The interface is plain exported functions over the module's memory, so
//! the module imports nothing and needs no binding generator. Bytes go in
//! through `staging`: the host asks for a buffer, copies an image into it,
//! and the next `boot_*` takes it. A pixel buffer or a teletype's output
//! comes back as a pointer into the module's memory, good until the next
//! call; the host has to take `memory.buffer` afresh after any call, since
//! a call can grow the memory and detach the old one.
//!
//! One machine at a time: a boot replaces whatever ran before, and a page
//! wanting a fresh machine can as well take a fresh instance. Every other
//! call is to the machine booted last; one before any boot traps.
//!
//! Teletypes are numbered as the machine numbers them: 0 the console, and
//! on the 703 1 to 3 the multiplexer's channels 0 to 2. `terminals` says
//! how many there are, and a number past them is no teletype -- a key there
//! goes nowhere and its paper never changes.
//!
//! The machine is a panel machine: it powers on halted, and RUN on the
//! panel starts it. Nothing paces it; the host passes the cycles a frame
//! of real time is worth (`clock_hz` / 60), and the devices count their
//! periods in those cycles, so the 703's teletypes type at ten characters
//! a second of machine time. The panels' lamps are tuned for sixty frames a
//! second, so the host renders the panel once for each such slice.
//!
//! Not here, deliberately: the debug port, `--serial`, snapshots and disc
//! writes that outlive the page.

use emu::console::panel703::PanelView;
use emu::console::panel8800::PanelView8800;
use emu::console::paper::Paper;
use emu::console::paper_view::PaperView;
use emu::console::surface::Framebuffer;
use emu::console::{
    AltairPanel, AltairPanelDisplay, ConsoleEndpoint, DiscRackState, PanelControl, PanelDisplay, PanelState, RunLamp,
};
use emu::cpu::i8080::Cpu8080;
use emu::cpu::ray703::Cpu703;
use emu::cpu::Cpu;
use emu::emulator::{Control, Emulator, StopPolicy};
use emu::system::altair8800::Altair8800;
use emu::system::ray703::{Ray703, SERIAL_PORTS};
use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// The most of a teletype's output kept for `drain`: the newest, the
/// oldest dropped first. A page that never drains -- the pages don't --
/// stays bounded however long a guest prints; a host that drains every
/// frame never comes near it, even under Fast I/O.
const KEPT_CAP: usize = 64 * 1024;

/// A teletype's output: printed on its paper, and kept for `drain` too.
#[derive(Clone)]
struct Printer {
    paper: Paper,
    kept: Arc<Mutex<Vec<u8>>>,
}

impl Write for Printer {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.paper.write_all(b)?;
        let mut kept = self.kept.lock().unwrap();
        kept.extend_from_slice(b);
        let over = kept.len().saturating_sub(KEPT_CAP);
        kept.drain(..over);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// One teletype: its keyboard, its paper as seen, and what it printed.
struct Teletype {
    keys: Sender<u8>,
    kept: Arc<Mutex<Vec<u8>>>,
    paper: PaperView,
    /// The paper changed while the host was not drawing it.
    unseen: bool,
}

impl Teletype {
    /// A teletype and the machine's end of its line.
    fn new() -> (Self, ConsoleEndpoint) {
        let (keys, rx) = mpsc::channel();
        let paper = Paper::new();
        let kept = Arc::new(Mutex::new(Vec::new()));
        let line = ConsoleEndpoint::new(rx, Box::new(Printer { paper: paper.clone(), kept: kept.clone() }));
        let mut view = PaperView::new(paper);
        // A blank page is nothing unseen: a host draws the paper it shows
        // with `force`, and from then on a change is something printed.
        view.take_redraw();
        (Teletype { keys, kept, paper: view, unseen: false }, line)
    }
}

enum Panel {
    Ray703 { view: Box<PanelView>, state: PanelState, rack: DiscRackState },
    Altair8800 { view: Box<PanelView8800>, state: AltairPanel },
}

struct Machine {
    emu: Emulator,
    clock_hz: u32,
    panel: Panel,
    panel_fb: Framebuffer,
    teletypes: Vec<Teletype>,
    paper_fb: Framebuffer,
    drained: Vec<u8>,
}

impl Machine {
    /// The run loop around a cpu and a bus, halted at the panel whose run
    /// lamp it publishes to, taking the panel's commands from `control`.
    fn new(
        cpu: Box<dyn Cpu + Send>,
        bus: Box<dyn emu::bus::Bus + Send>,
        run_lamp: Box<dyn RunLamp>,
        control: Receiver<Control>,
        clock_hz: u64,
        panel: Panel,
        teletypes: Vec<Teletype>,
    ) -> Self {
        let mut emu = Emulator::new(cpu, bus, Arc::new(AtomicBool::new(false)));
        emu.set_control(Some(control));
        emu.set_stop_policy(StopPolicy::Halt);
        emu.set_panel_state(Some(run_lamp));
        emu.reset();
        emu.set_halted(true);
        let (w, h) = match panel {
            Panel::Ray703 { .. } => (PanelView::WIDTH, PanelView::HEIGHT),
            Panel::Altair8800 { .. } => (PanelView8800::WIDTH, PanelView8800::HEIGHT),
        };
        Machine {
            emu,
            clock_hz: clock_hz as u32,
            panel,
            panel_fb: Framebuffer::new(w, h),
            teletypes,
            paper_fb: Framebuffer::new(PaperView::WIDTH, PaperView::HEIGHT),
            drained: Vec::new(),
        }
    }
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

/// Run `f` on teletype `n`, if the machine has one.
fn with_teletype<R>(n: u32, f: impl FnOnce(&mut Teletype) -> R) -> Option<R> {
    with(|m| m.teletypes.get_mut(n as usize).map(f))
}

fn install(m: Machine) -> u32 {
    MACHINE.with_borrow_mut(|slot| *slot = Some(m));
    1
}

/// A buffer of `len` bytes for the next `boot_*` to take.
#[no_mangle]
pub extern "C" fn staging(len: usize) -> *mut u8 {
    STAGE.with_borrow_mut(|s| {
        *s = vec![0; len];
        s.as_mut_ptr()
    })
}

fn staged() -> Vec<u8> {
    STAGE.with_borrow_mut(std::mem::take)
}

// -- booting ----------------------------------------------------------------

/// A 703 around the bus `make` returns, its console teletype and the
/// multiplexer's three on lines of their own, halted at the panel.
/// `time_of_day` is the clock's count, tenths of a millisecond since
/// midnight; `fast_io` is `--fast-io`.
fn boot_703(make: impl FnOnce(ConsoleEndpoint) -> Option<Ray703>, time_of_day: u32, fast_io: u32) -> u32 {
    let (console, line) = Teletype::new();
    let Some(mut bus) = make(line) else {
        return 0;
    };
    let mut teletypes = vec![console];
    for n in 1..=SERIAL_PORTS.len() {
        let (tty, line) = Teletype::new();
        bus.attach_terminal(n, line);
        teletypes.push(tty);
    }
    let rack = DiscRackState::new();
    bus.attach_disc_rack(rack.clone());
    bus.set_time_of_day(time_of_day);
    if fast_io != 0 {
        bus.set_fast_io();
    }
    let state = PanelState::new();
    let mut cpu = Cpu703::new();
    cpu.attach_panel(state.clone());
    let (control, control_rx) = mpsc::channel();
    let view = PanelView::new(PanelDisplay { panel: state.clone(), control: PanelControl(control), rack: rack.clone() });
    let panel = Panel::Ray703 { view: Box::new(view), state: state.clone(), rack };
    let clock = emu::cpu::ray703::CLOCK_HZ;
    install(Machine::new(Box::new(cpu), Box::new(bus), Box::new(state), control_rx, clock, panel, teletypes))
}

/// The staged bytes as a 703's flat core image from word 0. 0 if nothing
/// was staged: an empty core has nothing to run, as `Ray703::new` says too.
#[no_mangle]
pub extern "C" fn boot_core(time_of_day: u32, fast_io: u32) -> u32 {
    let image = staged();
    boot_703(|c| (!image.is_empty()).then(|| Ray703::with_core_image(&image, c)), time_of_day, fast_io)
}

/// The staged bytes as the platter on the 703's unit 0, and LOAD pressed:
/// how REX boots. 0 if they are not exactly one platter.
#[no_mangle]
pub extern "C" fn boot_disc(time_of_day: u32, fast_io: u32) -> u32 {
    let image = staged();
    let make = |c| {
        let mut bus = Ray703::bare(c);
        (bus.mount_disc_bytes(0, image) && bus.press_load()).then_some(bus)
    };
    boot_703(make, time_of_day, fast_io)
}

/// An Altair 8800 around the staged bytes, halted at its panel: with
/// `image` 0 they are a BASIC 4.x paper tape and MITS's bootstrap is keyed
/// in to read it (`altair8800`), with 1 a memory image loaded at 0
/// (`altair8800-image`). 0 if nothing was staged.
#[no_mangle]
pub extern "C" fn boot_altair8800(image: u32) -> u32 {
    let bytes = staged();
    if bytes.is_empty() {
        return 0;
    }
    let (console, line) = Teletype::new();
    let Ok(mut bus) = Altair8800::from_image(bytes, line, if image != 0 { "image" } else { "" }) else {
        return 0;
    };
    let state = AltairPanel::new();
    let mut cpu = Cpu8080::new();
    cpu.attach_panel(state.clone());
    bus.attach_panel(state.clone());
    let (control, control_rx) = mpsc::channel();
    let view = PanelView8800::new(AltairPanelDisplay { panel: state.clone(), control: PanelControl(control) });
    let panel = Panel::Altair8800 { view: Box::new(view), state: state.clone() };
    let clock = emu::system::altair8800::CLOCK_HZ;
    install(Machine::new(Box::new(cpu), Box::new(bus), Box::new(state), control_rx, clock, panel, vec![console]))
}

// -- running ----------------------------------------------------------------

/// The machine's clock, for the host to divide by its frame rate.
#[no_mangle]
pub extern "C" fn clock_hz() -> u32 {
    with(|m| m.clock_hz)
}

/// Run about `cycles`, or return at once if halted; 0 while the machine
/// goes on, 1 once its run has ended for good.
#[no_mangle]
pub extern "C" fn run(cycles: u32) -> u32 {
    with(|m| m.emu.run_cycles(cycles as u64).is_some() as u32)
}

/// The panel's run lamp: the 703's HALT lens, the 8800's WAIT.
#[no_mangle]
pub extern "C" fn halted() -> u32 {
    with(|m| match &m.panel {
        Panel::Ray703 { state, .. } => state.halted(),
        Panel::Altair8800 { state, .. } => state.halted(),
    } as u32)
}

// -- the panel --------------------------------------------------------------

#[no_mangle]
pub extern "C" fn panel_width() -> u32 {
    with(|m| m.panel_fb.width)
}

#[no_mangle]
pub extern "C" fn panel_height() -> u32 {
    with(|m| m.panel_fb.height)
}

/// Draw the panel, advancing its lamps a frame; the RGBA pixels.
#[no_mangle]
pub extern "C" fn panel_render() -> *const u8 {
    with(|m| {
        match &mut m.panel {
            Panel::Ray703 { view, .. } => view.render(&mut m.panel_fb),
            Panel::Altair8800 { view, .. } => view.render(&mut m.panel_fb),
        }
        m.panel_fb.pixels.as_ptr()
    })
}

/// A click at (x, y) in the panel's pixels; `back` the other button, which
/// only the 703's lamps take (it clears a bit where a click sets one).
#[no_mangle]
pub extern "C" fn panel_click(x: i32, y: i32, back: u32) {
    with(|m| match &mut m.panel {
        Panel::Ray703 { view, .. } => view.click(x, y, back != 0),
        Panel::Altair8800 { view, .. } if back == 0 => view.click(x, y),
        Panel::Altair8800 { .. } => {}
    });
}

/// A key at the 703's panel: 1 to 4 flip the SENSE toggles. The 8800's
/// panel takes none.
#[no_mangle]
pub extern "C" fn panel_key(c: u32) {
    with(|m| {
        if let Panel::Ray703 { view, .. } = &mut m.panel {
            view.key(char::from_u32(c).unwrap_or('\0'));
        }
    });
}

/// Turn the 703's DISPLAY SELECTOR: positive clockwise, negative back.
#[no_mangle]
pub extern "C" fn panel_knob(dir: i32) {
    with(|m| {
        if let Panel::Ray703 { view, .. } = &mut m.panel {
            view.turn_knob(dir);
        }
    });
}

// -- the teletypes ----------------------------------------------------------

/// How many teletypes the machine has, the console included.
#[no_mangle]
pub extern "C" fn terminals() -> u32 {
    with(|m| m.teletypes.len() as u32)
}

/// A key at teletype `n`, as the Model 33 sends it.
#[no_mangle]
pub extern "C" fn key(n: u32, c: u8) {
    with_teletype(n, |t| t.keys.send(c).ok());
}

#[no_mangle]
pub extern "C" fn paper_width() -> u32 {
    PaperView::WIDTH
}

#[no_mangle]
pub extern "C" fn paper_height() -> u32 {
    PaperView::HEIGHT
}

/// Teletype `n`'s paper as RGBA pixels if it changed since it was last
/// drawn, or whatever it shows if `force` -- a host showing one paper of
/// several draws the one it switches to -- and null otherwise.
#[no_mangle]
pub extern "C" fn paper_render(n: u32, force: u32) -> *const u8 {
    with(|m| {
        let Some(t) = m.teletypes.get_mut(n as usize) else {
            return std::ptr::null();
        };
        let changed = t.paper.take_redraw() | std::mem::take(&mut t.unseen);
        if !changed && force == 0 {
            return std::ptr::null();
        }
        t.paper.render(&mut m.paper_fb);
        m.paper_fb.pixels.as_ptr()
    })
}

/// 1 if teletype `n` has printed since its paper was last drawn: a host
/// showing another one marks it.
#[no_mangle]
pub extern "C" fn paper_unseen(n: u32) -> u32 {
    with_teletype(n, |t| {
        t.unseen |= t.paper.take_redraw();
        t.unseen as u32
    })
    .unwrap_or(0)
}

/// Roll teletype `n`'s platen `dy` notches, positive back into the
/// scrollback; 1 if it moved, 0 at a stop.
#[no_mangle]
pub extern "C" fn paper_wheel(n: u32, dy: i32) -> u32 {
    with_teletype(n, |t| t.paper.wheel(dy) as u32).unwrap_or(0)
}

/// 1 if a roll in `dy`'s direction would move teletype `n`'s platen: the
/// host hands the wheel on to its page when it would not.
#[no_mangle]
pub extern "C" fn paper_can_roll(n: u32, dy: i32) -> u32 {
    with_teletype(n, |t| t.paper.can_roll(dy) as u32).unwrap_or(0)
}

// -- for a host checking the machine ---------------------------------------

/// Take teletype `n`'s output since its last drain; its length, the bytes
/// at `drained_ptr` until the next call.
#[no_mangle]
pub extern "C" fn drain(n: u32) -> usize {
    with(|m| {
        m.drained = match m.teletypes.get(n as usize) {
            Some(t) => std::mem::take(&mut *t.kept.lock().unwrap()),
            None => Vec::new(),
        };
        m.drained.len()
    })
}

#[no_mangle]
pub extern "C" fn drained_ptr() -> *const u8 {
    with(|m| m.drained.as_ptr())
}

/// The 703's PROGRAM COUNTER row as it stands; 0 on the 8800.
#[no_mangle]
pub extern "C" fn pc_lamps() -> u32 {
    with(|m| match &m.panel {
        Panel::Ray703 { state, .. } => state.program_counter() as u32,
        Panel::Altair8800 { .. } => 0,
    })
}

/// How many reads one of the 703's disc units has completed.
#[no_mangle]
pub extern "C" fn rack_reads(unit: u32) -> u32 {
    with(|m| match &m.panel {
        Panel::Ray703 { rack, .. } => rack.snapshot().read_ops.get(unit as usize).copied().unwrap_or(0) as u32,
        Panel::Altair8800 { .. } => 0,
    })
}
