// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The table of supported machines.
//!
//! Generalizes the C++ `System::GetSupportedSystems`. Adding a machine means
//! adding one entry here; per AGENTS.md, system and rom metadata must not be
//! hardcoded anywhere else (the help text and the `--launch` menu are
//! generated from this table).

use crate::bus::Bus;
use crate::console::serial::{self, SerialSpec};
use crate::console::{ConsoleEndpoint, Display};
use crate::cpu::Cpu;
use crate::system::{altair680, altair8800, kaypro, ray703, rc2014, sys09};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use crate::emulator::Control;

/// Everything built for one machine: a core, the bus it drives, and -- if it
/// has a screen -- the display handle the main-thread frontend renders from.
pub struct Machine {
    pub cpu: Box<dyn Cpu + Send>,
    pub bus: Box<dyn Bus + Send>,
    /// `None` for terminal-only machines, which use the console's serial
    /// output instead.
    pub display: Option<Display>,
    /// A machine that wants real time whatever the registry says sets its
    /// clock rate here (a live front panel is meaningless uncapped). `None`
    /// leaves the default to the registry's `clock_hz`; the command line
    /// overrides either way.
    pub throttle_hz: Option<u64>,
    /// The run loop's control channel, both ends, when the machine has a
    /// front panel or drive doors in its window: the window already holds
    /// a clone of the sender inside `display`, and `main` hands the
    /// receiver to the `Emulator`.
    pub control: Option<ControlChannel>,
    /// A handle on the panel's shared lamp state, for
    /// `Emulator::set_panel_state` -- the run loop publishes whether the
    /// machine is halted, which lights the HALT indicator's red lens.
    /// The frontend's own handle rides inside `display`.
    pub panel: Option<Box<dyn crate::console::RunLamp>>,
}

/// Both ends of the run loop's control channel.
pub struct ControlChannel {
    pub tx: mpsc::Sender<Control>,
    pub rx: mpsc::Receiver<Control>,
}

/// Build-time options that apply across systems. Every factory receives
/// them; a machine honors what is meaningful for it and ignores the rest,
/// so a flag like `--fast-io` needs no per-system plumbing in `main.rs`.
#[derive(Clone, Default)]
pub struct MachineOpts {
    /// Devices complete I/O instantly instead of at their period rates.
    /// Meaningful only on machines that model device timing at all --
    /// currently the 703's teletype; everywhere else it is already true.
    pub fast_io: bool,
    /// `--disk PATH`, once per drive: the images for the machine's disk
    /// drives in turn, the first in place of the fixed name under `disks/`.
    /// The Kaypro's floppies, the RC2014's card and the Altair 8800's
    /// floppies take them, each machine refusing more than it has drives;
    /// the 703 mounts four units by name and ignores them.
    pub disks: Vec<PathBuf>,
    /// `--serial SPEC`, once per spare serial port: the host ends, in the
    /// order of the descriptor's `serial_ports`. `main` refuses more than a
    /// machine has.
    pub serial: Vec<SerialSpec>,
}

/// Open the `--serial` lines asked for, in `ports` order, and announce
/// where each one is.
fn open_serial(opts: &MachineOpts, ports: &[&str]) -> io::Result<Vec<ConsoleEndpoint>> {
    opts.serial
        .iter()
        .zip(ports)
        .map(|(spec, port)| {
            let (endpoint, place) = serial::open(spec)?;
            println!("serial: {port} is on {place}");
            Ok(endpoint)
        })
        .collect()
}

/// The one `--disk` of a machine with one drive, refusing a second.
fn only_disk(opts: &MachineOpts, why: &str) -> io::Result<Option<PathBuf>> {
    match opts.disks.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.clone())),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{why}: one --disk"))),
    }
}

/// `subsystem` is the part after the dash in e.g. `6809-obc`, or "".
type FactoryFn = fn(&Path, ConsoleEndpoint, &str, &MachineOpts) -> io::Result<Machine>;

pub struct SystemDescriptor {
    pub name: &'static str,
    /// One line for `-h`: what the machine is and what it boots.
    pub title: &'static str,
    pub cpu: &'static str,
    pub default_rom: &'static str,
    pub factory: FactoryFn,
    /// The real machine's clock rate, used by a bare `--throttle`. `None`
    /// until the rate is verified *and* the core reports cycle counts --
    /// throttling a core that reports none is an announced no-op.
    pub clock_hz: Option<u64>,
    /// The spare serial ports, each taking one `--serial` in turn: what `-h`
    /// and the startup line call them.
    pub serial_ports: &'static [&'static str],
    /// What `--disk` replaces, for `-h`, or why it has nothing to.
    pub disks: Option<&'static str>,
    /// The subsystem tokens `-h` lists, each with what it does.
    pub variants: &'static [(&'static str, &'static str)],
    /// What `--launch` offers for this machine.
    pub launch: Launch,
}

/// The launcher's view of a machine: the ways to start it and where the
/// files each one takes are looked for. `src/launcher` scans for them and
/// knows nothing about any machine beyond this.
pub struct Launch {
    /// The machine as the launcher's list names it.
    pub name: &'static str,
    pub modes: &'static [LaunchMode],
    /// Files every mode needs besides what it picks. A mode that picks no
    /// rom needs `default_rom` as well, without its being listed here.
    pub needs: &'static [&'static str],
    /// Subsystem tokens that compose with every mode -- the windows --
    /// offered as switches, each with what it does.
    pub windows: &'static [(&'static str, &'static str)],
    /// Whether a `serial_ports` line can be `--serial window` as well as a
    /// pty or a port.
    pub serial_window: bool,
}

/// One way of starting a machine: a subsystem and what it is given.
pub struct LaunchMode {
    /// The subsystem token, "" for the plain machine.
    pub variant: &'static str,
    /// What one of this mode's files is, as the launcher lists it.
    pub what: &'static str,
    pub media: Media,
    /// How to come by the files when there are none.
    pub fetch: &'static str,
}

/// Which file a mode picks, and so which option carries it.
pub enum Media {
    /// Nothing to pick: the machine runs its `default_rom`.
    Fixed,
    /// The file goes to `-r`.
    Rom(Files),
    /// The file goes to `--disk`, and the machine runs the rom named: its
    /// `default_rom`, or the one its factory runs for this mode instead.
    Disk(Files, &'static str),
}

/// Where a mode's files are looked for: the plain files in `dirs` (relative
/// to the directory the emulator runs from, as `default_rom` is) with one
/// of `exts`, any case, `name_has` somewhere in the name, and a size `size`
/// allows.
pub struct Files {
    pub dirs: &'static [&'static str],
    pub exts: &'static [&'static str],
    pub name_has: &'static str,
    pub size: FileSize,
}

pub enum FileSize {
    Any,
    Exactly(u64),
    AtLeast(u64),
}

impl FileSize {
    pub fn allows(&self, len: u64) -> bool {
        match *self {
            FileSize::Any => true,
            FileSize::Exactly(n) => len == n,
            FileSize::AtLeast(n) => len >= n,
        }
    }
}

const FETCH_ROMS: &str = "tools/fetch-roms.py";

/// A rate as `-h` and the launcher show it: 1843200 is "1.8432 MHz".
pub fn fmt_hz(hz: u64) -> String {
    let (div, unit) = if hz >= 1_000_000 { (1e6, "MHz") } else if hz >= 1_000 { (1e3, "kHz") } else { (1.0, "Hz") };
    let n = format!("{:.4}", hz as f64 / div);
    format!("{} {unit}", n.trim_end_matches('0').trim_end_matches('.'))
}

/// An Altair's window tokens, taken out of its subsystem: "panel" the
/// front panel, "tty" the console's teletype, each beside whatever the
/// rest picks and in any order (altair680-basic-tty-panel). What is left
/// goes to the machine, a duplicated token included, which it refuses.
fn altair_windows(sub: &str) -> (bool, bool, String) {
    let mut tokens: Vec<&str> = sub.split('-').filter(|t| !t.is_empty()).collect();
    let mut take = |name: &str| tokens.iter().position(|&t| t == name).map(|i| tokens.remove(i)).is_some();
    let (panel, tty) = (take("panel"), take("tty"));
    (panel, tty, tokens.join("-"))
}

/// The console's teletype in a window: the paper is the endpoint's sink
/// from here on, the debug port's tap surviving the change. No pacing of
/// its own: the Altairs' ACIAs have no baud rate here, so it prints as fast
/// as the guest writes.
fn altair_tty(console: &mut ConsoleEndpoint) -> crate::console::TtyDisplay {
    let paper = crate::console::Paper::new();
    console.set_output(Box::new(paper.clone()));
    crate::console::TtyDisplay { paper, line: None }
}

/// An Altair's front panel: the lamps' state, the switches' channel to the
/// run loop and the run lamp.
fn altair_panel(state: &crate::console::AltairPanel) -> (crate::console::AltairPanelDisplay, ControlChannel, Box<dyn crate::console::RunLamp>) {
    let (ctl_tx, ctl_rx) = mpsc::channel();
    let display =
        crate::console::AltairPanelDisplay { panel: state.clone(), control: crate::console::PanelControl(ctl_tx.clone()) };
    (display, ControlChannel { tx: ctl_tx, rx: ctl_rx }, Box::new(state.clone()))
}

fn build_altair680(rom: &Path, mut console: ConsoleEndpoint, sub: &str, _opts: &MachineOpts) -> io::Result<Machine> {
    let (panel, tty, sub) = altair_windows(sub);
    let tty = tty.then(|| altair_tty(&mut console));
    let bus = altair680::Altair680::new(rom, console, &sub)?;
    let mut cpu = crate::cpu::m6800::Cpu6800::new();
    let (mut panel_display, mut control, mut run_lamp, mut throttle_hz) = (None, None, None, None);
    if panel {
        let state = crate::console::AltairPanel::new();
        cpu.attach_panel(state.clone());
        let (d, c, l) = altair_panel(&state);
        (panel_display, control, run_lamp) = (Some(d), Some(c), Some(l));
        // a live panel wants real time, as the 703's and the 8800's do
        throttle_hz = Some(ALTAIR680_CLOCK_HZ);
    }
    let display = (panel || tty.is_some())
        .then_some(Display::Altair680 { title: "MITS Altair 680b", panel: panel_display, tty });
    Ok(Machine { cpu: Box::new(cpu), bus: Box::new(bus), display, throttle_hz, control, panel: run_lamp })
}

/// The 680 ran its 6800 at 500 kHz -- MITS shipped it slow to suit the
/// memory it came with.
const ALTAIR680_CLOCK_HZ: u64 = 500_000;

/// Each `--disk` in its drive in turn, drive 0 the default floppy when
/// none is named. A default that is not there leaves the drive empty, and
/// the boot loader waiting for it, as a machine with its door open would.
fn mount_altair8800_disks(bus: &mut altair8800::Altair8800, opts: &MachineOpts) -> io::Result<()> {
    if !bus.has_disk_controller() {
        if !opts.disks.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "this 8800 has no disk drive; altair8800-disk has"));
        }
        return Ok(());
    }
    if opts.disks.len() > crate::dev::dcdd::DRIVES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the controller has {} drives: at most that many --disk", crate::dev::dcdd::DRIVES),
        ));
    }
    if opts.disks.is_empty() {
        let default = Path::new(altair8800::DEFAULT_DISK);
        if default.exists() {
            bus.mount_disk(0, default)?;
        } else {
            println!("88-DCDD: drive 0 is empty: {} is not there (tools/fetch-roms.py)", default.display());
        }
    }
    for (drive, path) in opts.disks.iter().enumerate() {
        bus.mount_disk(drive, path)?;
    }
    Ok(())
}

fn build_altair8800(rom: &Path, mut console: ConsoleEndpoint, sub: &str, opts: &MachineOpts) -> io::Result<Machine> {
    let (panel, tty, sub) = altair_windows(sub);
    let tty = tty.then(|| altair_tty(&mut console));
    // the disk machine's -r is its boot PROM, not BASIC's tape
    let rom = if altair8800::Altair8800::boots_from_prom(&sub) && rom == Path::new(altair8800::DEFAULT_ROM) {
        println!("boot prom is {}", altair8800::DBL_ROM);
        Path::new(altair8800::DBL_ROM)
    } else {
        rom
    };
    let mut bus = altair8800::Altair8800::new(rom, console, &sub)?;
    mount_altair8800_disks(&mut bus, opts)?;
    if let Some(line) = open_serial(opts, &[altair8800::SERIAL_PORT])?.pop() {
        bus.attach_serial(line);
    }
    let mut cpu = crate::cpu::i8080::Cpu8080::new();
    let (mut panel_display, mut control, mut run_lamp, mut throttle_hz) = (None, None, None, None);
    if panel {
        let state = crate::console::AltairPanel::new();
        cpu.attach_panel(state.clone());
        bus.attach_panel(state.clone());
        let (d, c, l) = altair_panel(&state);
        (panel_display, control, run_lamp) = (Some(d), Some(c), Some(l));
        // a live panel wants real time, as the 703's does
        throttle_hz = Some(altair8800::CLOCK_HZ);
    }
    let display = (panel || tty.is_some())
        .then_some(Display::Altair8800 { title: "MITS Altair 8800", panel: panel_display, tty });
    Ok(Machine { cpu: Box::new(cpu), bus: Box::new(bus), display, throttle_hz, control, panel: run_lamp })
}

fn build_rc2014(rom: &Path, console: ConsoleEndpoint, sub: &str, opts: &MachineOpts) -> io::Result<Machine> {
    let mut bus = rc2014::Rc2014::new(rom, console, sub)?;
    if opts.fast_io {
        bus.set_fast_io();
    }
    // The card mounts like the Kaypro's floppy: `--disk` or the fixed name
    // under disks/, non-fatal, gitignored. Only the cpm build has the slot.
    if bus.has_compact_flash() {
        let card = only_disk(opts, "the RC2014 has one card slot")?.unwrap_or_else(|| PathBuf::from(rc2014::DEFAULT_CF));
        bus.mount_compact_flash(&card);
    }
    if let Some(line) = open_serial(opts, &[rc2014::SERIAL_PORT])?.pop() {
        bus.attach_serial(line);
    }
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(bus),
        display: None,
        throttle_hz: None,
        control: None,
        panel: None,
    })
}

fn build_sys09(rom: &Path, console: ConsoleEndpoint, sub: &str, _opts: &MachineOpts) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6809::Cpu6809::new()),
        bus: Box::new(sys09::System09::new(rom, console, sub)?),
        display: None,
        throttle_hz: None,
        control: None,
        panel: None,
    })
}

/// The host's local time of day, in the 703 clock's tenths of a
/// millisecond since midnight; midnight if the host cannot say.
fn host_time_of_day() -> u32 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let t = now.as_secs() as libc::time_t;
    // SAFETY: localtime_r writes only the tm it is given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return 0;
    }
    let secs = (tm.tm_hour * 3600 + tm.tm_min * 60 + tm.tm_sec) as u32;
    secs * 10_000 + now.subsec_micros() / 100
}

fn build_ray703(
    rom: &Path,
    mut console: ConsoleEndpoint,
    sub: &str,
    opts: &MachineOpts,
) -> io::Result<Machine> {
    // The subsystem is a set of tokens: "panel" opens the front panel
    // window, "tty" the teletype window, "ptb" keys in the bootstrap, in
    // any order. Strip the window tokens here and hand the rest to the
    // machine, which knows nothing about displays. A duplicated window
    // token leaves a copy behind for the match below to reject.
    let take = |tokens: &mut Vec<&str>, name: &str| {
        tokens.iter().position(|&t| t == name).map(|i| tokens.remove(i)).is_some()
    };
    let mut tokens: Vec<&str> = sub.split('-').filter(|t| !t.is_empty()).collect();
    let panel = take(&mut tokens, "panel");
    let tty = take(&mut tokens, "tty");
    let sub = match tokens.as_slice() {
        [] => "",
        [one] => *one,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unknown ray703 subsystem '{sub}'"),
            ))
        }
    };

    let mut cpu = crate::cpu::ray703::Cpu703::new();
    // The one thing the 703 needs that the other machines don't: an operator
    // at the front panel. PTB is keyed in with the index register preset to
    // the load origin, so the factory stands in for the operator's hands.
    if let Some(ixr) = ray703::Ray703::ptb_index(sub) {
        cpu.set_index(ixr);
    }

    let mut throttle_hz = None;
    let mut control = None;
    let mut panel_state = None;
    let mut rack = None;
    let mut panel_display = None;
    if panel {
        let state = crate::console::PanelState::new();
        cpu.attach_panel(state.clone());
        // ...one clone for the run loop's halt reporting...
        panel_state = Some(state.clone());
        // ...and switch actuations flow frontend -> run loop over this channel
        let (ctl_tx, ctl_rx) = mpsc::channel();
        // the rack lamps under the console, fed by the disc controller
        let rack_state = crate::console::DiscRackState::new();
        rack = Some(rack_state.clone());
        panel_display = Some(crate::console::PanelDisplay {
            panel: state,
            control: crate::console::PanelControl(ctl_tx.clone()),
            rack: rack_state,
        });
        control = Some(ControlChannel { tx: ctl_tx, rx: ctl_rx });
        // A live panel is meaningless uncapped -- the lamps would be a
        // uniform blur -- so a panel machine asks for real time even if its
        // registry entry names no clock rate to default to. --no-throttle on
        // the command line still overrides.
        throttle_hz = Some(crate::cpu::ray703::CLOCK_HZ);
    }
    let mut tty_display = None;
    if tty {
        // The teletype window replaces stdout as the endpoint's sink: from
        // here on the guest's serial output prints on this paper and
        // nowhere else. Deliberately no throttle_hz of its own -- the
        // device pacing already types at ten characters a second, and
        // --no-throttle blasts the paper exactly as it blasts the terminal.
        let paper = crate::console::Paper::new();
        console.set_output(Box::new(paper.clone()));
        tty_display = Some(crate::console::TtyDisplay { paper, line: None });
    }

    let mut bus = ray703::Ray703::new(rom, console, sub)?;
    // The operator set the time-of-day clock from its panel and started it;
    // the host's local time is what an operator would have set.
    bus.set_time_of_day(host_time_of_day());
    // Each --serial in turn is the next teletype's line: a window of its
    // own -- the paper its sink, the window's keys its keyboard -- or a
    // line to the host.
    let mut terminals = Vec::new();
    for (n, (spec, port)) in opts.serial.iter().zip(ray703::SERIAL_PORTS).enumerate() {
        let line = if *spec == SerialSpec::Window {
            let paper = crate::console::Paper::new();
            let (keys, rx) = mpsc::channel();
            terminals.push(crate::console::TtyDisplay { paper: paper.clone(), line: Some((n + 1, keys)) });
            println!("serial: {port} is in a window");
            ConsoleEndpoint::new(rx, Box::new(paper))
        } else {
            let (line, place) = serial::open(spec)?;
            println!("serial: {port} is on {place}");
            line
        };
        bus.attach_terminal(n + 1, line);
    }
    let display = (panel || tty || !terminals.is_empty()).then_some(Display::Ray703 {
        title: "Raytheon 703",
        panel: panel_display,
        tty: tty_display,
        terminals,
    });
    if opts.fast_io {
        bus.set_fast_io();
    }
    // Attach before the mounting loop lights the ONLINE lamps -- though
    // attach_rack syncs from whatever is mounted, so the load subsystem's
    // unit 0 (mounted inside Ray703::new) is caught either way.
    if let Some(rack) = &rack {
        bus.attach_disc_rack(rack.clone());
    }
    // The disc images mount like the Kaypro's floppy: fixed names under
    // disks/, non-fatal, gitignored. A file that simply is not there is a
    // drive that was never installed, and stays silent. The load subsystem
    // has already put `-r` in unit 0, and the boot disc the user named
    // outranks whatever the working directory holds.
    for unit in 0..crate::dev::disc74601::DISC_UNITS {
        if sub == "load" && unit == 0 {
            continue;
        }
        bus.mount_disc(unit, Path::new(&format!("disks/ray703-disc{unit}.img")));
    }

    Ok(Machine {
        cpu: Box::new(cpu),
        bus: Box::new(bus),
        display,
        throttle_hz,
        control,
        panel: panel_state.map(|p| Box::new(p) as Box<dyn crate::console::RunLamp>),
    })
}

fn build_kaypro(rom: &Path, console: ConsoleEndpoint, _sub: &str, opts: &MachineOpts) -> io::Result<Machine> {
    if opts.disks.len() > 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "the Kaypro II has two drives: at most two --disk"));
    }
    let floppy = opts.disks.first().cloned().unwrap_or_else(|| PathBuf::from(kaypro::DEFAULT_FLOPPY));
    // The window's drive doors reach the machine over the run loop's
    // control channel, as the 703's panel switches do: the window holds
    // the sender inside the display, the run loop gets the receiver.
    let (ctl_tx, ctl_rx) = mpsc::channel();
    let drives = crate::console::DriveBayDisplay {
        state: crate::console::DriveBayState::new(),
        control: crate::console::DriveControl(ctl_tx.clone()),
    };
    let (mut bus, display) = kaypro::Kaypro::new(
        rom,
        Path::new(kaypro::VIDEO_ROM),
        &floppy,
        Path::new(kaypro::DEFAULT_FLOPPY),
        console,
        drives,
    )?;
    if opts.fast_io {
        bus.set_fast_io();
    }
    // a second --disk is drive B, through the door a disk changes by, which
    // judges it as it would any
    if let Some(b) = opts.disks.get(1) {
        let image = crate::console::DiskImage::open(b.clone()).map_err(io::Error::other)?;
        bus.change_disk(1, Some(image)).map_err(io::Error::other)?;
    }
    if let Some(line) = open_serial(opts, &[kaypro::SERIAL_PORT])?.pop() {
        bus.attach_serial(line);
    }
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(bus),
        display: Some(display),
        throttle_hz: None,
        control: Some(ControlChannel { tx: ctl_tx, rx: ctl_rx }),
        panel: None,
    })
}

pub static SYSTEMS: &[SystemDescriptor] = &[
    SystemDescriptor {
        name: "6809",
        title: "System09: Microsoft BASIC on a 6809",
        cpu: "6809",
        default_rom: sys09::DEFAULT_ROM,
        factory: build_sys09,
        // Grant Searle's Simple 6809, where the default rom comes from: a
        // 7.3728 MHz crystal the 6809 divides by four, so the E clock the
        // datasheet's cycle counts tick at is 1.8432 MHz.
        clock_hz: Some(1_843_200),
        serial_ports: &[],
        disks: None,
        variants: &[],
        launch: Launch {
            name: "System09",
            modes: &[LaunchMode {
                variant: "",
                what: "rom",
                media: Media::Rom(Files { dirs: &["roms/6809"], exts: &["hex", "ihx"], name_has: "", size: FileSize::Any }),
                fetch: FETCH_ROMS,
            }],
            needs: &[],
            windows: &[],
            serial_window: false,
        },
    },
    SystemDescriptor {
        name: "altair680",
        title: "MITS Altair 680b: its prom monitor, and VTL-2 at FC00",
        cpu: "6800",
        default_rom: altair680::DEFAULT_ROM,
        factory: build_altair680,
        clock_hz: Some(ALTAIR680_CLOCK_HZ),
        serial_ports: &[],
        disks: None,
        variants: &[
            ("basic", "loads 680 BASIC off its paper tape: -r roms/mits680b/basic680.s19"),
            ("noterm", "strapped for no terminal: reset jumps to 0"),
            ("panel", "the front panel in a window; starts halted"),
            ("tty", "the console's teletype in a window"),
        ],
        launch: Launch {
            name: "MITS Altair 680",
            modes: &[
                LaunchMode {
                    variant: "",
                    what: "monitor rom",
                    media: Media::Rom(Files {
                        dirs: &["roms/mits680b"],
                        exts: &["bin"],
                        name_has: "mits680b",
                        size: FileSize::Any,
                    }),
                    fetch: FETCH_ROMS,
                },
                LaunchMode {
                    variant: "basic",
                    what: "BASIC tape",
                    media: Media::Rom(Files {
                        dirs: &["roms/mits680b"],
                        exts: &["s19"],
                        name_has: "basic",
                        size: FileSize::Any,
                    }),
                    fetch: FETCH_ROMS,
                },
                LaunchMode {
                    variant: "noterm",
                    what: "front panel only, reset to 0",
                    media: Media::Fixed,
                    fetch: FETCH_ROMS,
                },
            ],
            // the tape loads through the monitor
            needs: &[altair680::DEFAULT_ROM],
            windows: &[("panel", "front panel window; starts halted"), ("tty", "teletype in a window")],
            serial_window: false,
        },
    },
    SystemDescriptor {
        name: "altair8800",
        title: "MITS Altair 8800: loads 8K BASIC off paper tape",
        cpu: "8080",
        default_rom: altair8800::DEFAULT_ROM,
        factory: build_altair8800,
        clock_hz: Some(altair8800::CLOCK_HZ),
        serial_ports: &[altair8800::SERIAL_PORT],
        disks: Some("disk: floppy drives 0-3 in turn, drive 0 default disks/altair8800-cpm.dsk"),
        variants: &[
            ("4k", "for 4K BASIC: -r roms/altair8800/basic4k40.tap"),
            ("ext", "for Extended BASIC 4.x, its tape given with -r"),
            ("image", "-r is a memory image, run from address 0"),
            ("disk", "88-DCDD floppies; boots drive 0 by the DBL PROM, -r roms/altair8800/dbl.hex"),
            ("panel", "the front panel in a window; starts stopped"),
            ("tty", "the console's teletype in a window"),
        ],
        // A tape is for the loader its BASIC's size keys, which nothing in
        // the file says, so each loader lists the tapes named for it.
        launch: Launch {
            name: "MITS Altair 8800",
            modes: &[
                LaunchMode {
                    variant: "",
                    what: "8K BASIC tape",
                    media: Media::Rom(Files { dirs: &["roms/altair8800"], exts: &["tap"], name_has: "8k", size: FileSize::Any }),
                    fetch: FETCH_ROMS,
                },
                LaunchMode {
                    variant: "4k",
                    what: "4K BASIC tape",
                    media: Media::Rom(Files { dirs: &["roms/altair8800"], exts: &["tap"], name_has: "4k", size: FileSize::Any }),
                    fetch: FETCH_ROMS,
                },
                LaunchMode {
                    variant: "ext",
                    what: "Extended BASIC tape",
                    media: Media::Rom(Files { dirs: &["roms/altair8800"], exts: &["tap"], name_has: "ext", size: FileSize::Any }),
                    fetch: "a tape of your own, in roms/altair8800",
                },
                LaunchMode {
                    variant: "disk",
                    what: "floppy in drive 0",
                    media: Media::Disk(
                        Files {
                            dirs: &["disks"],
                            exts: &["dsk"],
                            name_has: "",
                            size: FileSize::AtLeast(crate::dev::dcdd::IMAGE_BYTES),
                        },
                        altair8800::DBL_ROM,
                    ),
                    fetch: FETCH_ROMS,
                },
                LaunchMode {
                    variant: "image",
                    what: "memory image",
                    media: Media::Rom(Files { dirs: &["roms/altair8800"], exts: &["bin"], name_has: "", size: FileSize::Any }),
                    fetch: "an image of your own, in roms/altair8800",
                },
            ],
            needs: &[],
            windows: &[("panel", "front panel window; starts stopped"), ("tty", "teletype in a window")],
            serial_window: false,
        },
    },
    SystemDescriptor {
        name: "kaypro",
        title: "Kaypro II: CP/M and the screen in an SDL window",
        cpu: "z80",
        default_rom: kaypro::DEFAULT_ROM,
        factory: build_kaypro,
        // The stock Kaypro II Z80 rate (the 5 MHz machines came later).
        clock_hz: Some(kaypro::CLOCK_HZ),
        serial_ports: &[kaypro::SERIAL_PORT],
        disks: Some("drive A, default disks/mbasic-games.img; a second is drive B"),
        variants: &[],
        launch: Launch {
            name: "Kaypro II",
            modes: &[LaunchMode {
                variant: "",
                what: "floppy in drive A",
                media: Media::Disk(Files {
                    dirs: &["disks"],
                    exts: &["img"],
                    name_has: "",
                    size: FileSize::Exactly(crate::dev::wd1793::IMAGE_SIZE),
                }, kaypro::DEFAULT_ROM),
                fetch: FETCH_ROMS,
            }],
            needs: &[kaypro::VIDEO_ROM],
            windows: &[],
            serial_window: false,
        },
    },
    SystemDescriptor {
        name: "ray703",
        title: "Raytheon 703 (1967): -r is a core image loaded at word 0",
        cpu: "703",
        default_rom: ray703::DEFAULT_ROM,
        factory: build_ray703,
        clock_hz: Some(crate::cpu::ray703::CLOCK_HZ),
        serial_ports: &ray703::SERIAL_PORTS,
        disks: Some("unused; disks/ray703-disc0..3.img by name"),
        variants: &[
            ("ptb", "key in PTB; -r is the paper tape it reads"),
            ("load", "press LOAD; -r is the boot disc for unit 0"),
            ("panel", "front panel and disc rack window; starts halted"),
            ("tty", "the teletype in a window"),
        ],
        launch: Launch {
            name: "Raytheon 703",
            modes: &[
                LaunchMode {
                    variant: "",
                    what: "core image",
                    media: Media::Rom(Files { dirs: &["roms/703"], exts: &["bin"], name_has: "", size: FileSize::Any }),
                    fetch: "make -C test ray703",
                },
                LaunchMode {
                    variant: "ptb",
                    what: "paper tape via PTB",
                    media: Media::Rom(Files { dirs: &["roms/703"], exts: &["tape"], name_has: "", size: FileSize::Any }),
                    fetch: "make -C test ray703",
                },
                LaunchMode {
                    variant: "load",
                    what: "boot disc via LOAD",
                    media: Media::Rom(Files {
                        dirs: &["disks", "rex/build"],
                        exts: &["img"],
                        name_has: "",
                        size: FileSize::Exactly(crate::dev::disc74601::IMAGE_BYTES as u64),
                    }),
                    fetch: "make -C rex bootdisc",
                },
            ],
            needs: &[],
            windows: &[("panel", "front panel and disc rack; starts halted"), ("tty", "teletype in a window")],
            serial_window: true,
        },
    },
    SystemDescriptor {
        name: "rc2014",
        title: "RC2014 Classic: 32K BASIC on the factory rom",
        cpu: "z80",
        default_rom: rc2014::DEFAULT_ROM,
        factory: build_rc2014,
        // The standard RC2014 crystal, 7.3728 MHz -- chosen so the serial
        // clock divides down to 115200 baud, and the Z80 runs off the same
        // can.
        clock_hz: Some(rc2014::CLOCK_HZ),
        serial_ports: &[rc2014::SERIAL_PORT],
        disks: Some("the cpm card, default disks/rc2014-cf.img"),
        variants: &[("cpm", "RC2014 Pro: CP/M on a compact flash card")],
        // Any card of a megabyte or more: a card has no fixed size, and that
        // keeps the floppy and platter images in disks/ out of the list.
        launch: Launch {
            name: "RC2014",
            modes: &[
                LaunchMode { variant: "", what: "32K BASIC", media: Media::Fixed, fetch: FETCH_ROMS },
                LaunchMode {
                    variant: "cpm",
                    what: "CP/M card",
                    media: Media::Disk(
                        Files { dirs: &["disks"], exts: &["img"], name_has: "", size: FileSize::AtLeast(1 << 20) },
                        rc2014::DEFAULT_ROM,
                    ),
                    fetch: FETCH_ROMS,
                },
            ],
            needs: &[],
            windows: &[],
            serial_window: false,
        },
    },
];

/// Split a system option into its system and subsystem halves, as the C++
/// `System::Factory` does: `6809-obc` is the `6809` system, subsystem `obc`.
pub fn split_name(name: &str) -> (&str, &str) {
    match name.split_once('-') {
        Some((base, sub)) => (base, sub),
        None => (name, ""),
    }
}

/// Look a machine up by name, ignoring any subsystem suffix.
pub fn find(name: &str) -> Option<&'static SystemDescriptor> {
    let (base, _) = split_name(name);
    SYSTEMS.iter().find(|s| s.name == base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_known_system() {
        assert_eq!(find("altair680").map(|s| s.cpu), Some("6800"));
        assert_eq!(find("6809").map(|s| s.cpu), Some("6809"));
    }

    #[test]
    fn a_subsystem_suffix_selects_the_base_system() {
        assert!(find("altair680-obc").is_some());
        assert_eq!(split_name("6809-obc"), ("6809", "obc"));
        assert_eq!(split_name("6809"), ("6809", ""));
    }

    #[test]
    fn unknown_systems_are_not_found() {
        assert!(find("apple2").is_none());
    }

    #[test]
    fn every_entry_is_self_consistent() {
        for s in SYSTEMS {
            assert!(!s.name.is_empty());
            assert!(!s.cpu.is_empty());
            assert!(!s.default_rom.is_empty());
            assert!(find(s.name).is_some(), "{} is not findable", s.name);
            // a zero rate would make a bare --throttle divide by zero
            assert_ne!(s.clock_hz, Some(0), "{} has a zero clock rate", s.name);
        }
    }

    /// `-h` lists each variant, so each must be one its factory takes. A
    /// variant may still fail on a rom this test does not supply, so what
    /// is checked is that the refusal is not the unknown-subsystem one --
    /// which a junk token, the control, must get.
    #[test]
    fn every_listed_variant_is_one_the_factory_takes() {
        let dir = std::env::temp_dir().join(format!("emu-registry-variants-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("rom.bin");
        std::fs::write(&rom, [0u8; 256]).unwrap();
        let build = |s: &SystemDescriptor, sub: &str| {
            let (_tx, rx) = std::sync::mpsc::channel();
            let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
            (s.factory)(&rom, console, sub, &MachineOpts::default()).err().map(|e| e.to_string())
        };
        for s in SYSTEMS.iter().filter(|s| !s.variants.is_empty()) {
            let junk = build(s, "junk").unwrap_or_default();
            assert!(junk.contains("unknown"), "{}-junk: {junk:?}", s.name);
            for (token, _) in s.variants {
                if let Some(err) = build(s, token) {
                    assert!(!err.contains("unknown"), "{}-{token}: {err}", s.name);
                }
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Build a ray703 machine for one subsystem string. Uses the demo image
    /// only as bytes to load, via a temp file, so no ROM symlink is needed.
    #[test]
    fn every_launch_mode_is_a_listed_variant_the_factory_takes() {
        let dir = std::env::temp_dir().join(format!("emu-registry-launch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("rom.bin");
        std::fs::write(&rom, [0u8; 256]).unwrap();
        let listed = |s: &SystemDescriptor, token: &str| token.is_empty() || s.variants.iter().any(|(t, _)| *t == token);
        for s in SYSTEMS {
            assert!(!s.launch.modes.is_empty(), "{} offers nothing to launch", s.name);
            for (window, _) in s.launch.windows {
                assert!(listed(s, window), "{}: window {window} is not in -h", s.name);
            }
            for mode in s.launch.modes {
                assert!(listed(s, mode.variant), "{}: mode {:?} is not in -h", s.name, mode.variant);
                // each mode alone and with every window, as the launcher composes them
                let mut subs = vec![mode.variant.to_string()];
                let windows: Vec<&str> = s.launch.windows.iter().map(|(w, _)| *w).collect();
                if !windows.is_empty() {
                    subs.push([mode.variant].iter().chain(&windows).filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join("-"));
                }
                for sub in subs {
                    let (_tx, rx) = std::sync::mpsc::channel();
                    let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
                    if let Err(e) = (s.factory)(&rom, console, &sub, &MachineOpts::default()) {
                        assert!(!e.to_string().contains("unknown"), "{}-{sub}: {e}", s.name);
                    }
                }
            }
            // the launcher offers a window on every port at once
            if s.launch.serial_window {
                let (_tx, rx) = std::sync::mpsc::channel();
                let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
                let opts = MachineOpts { serial: vec![SerialSpec::Window; s.serial_ports.len()], ..MachineOpts::default() };
                if let Err(e) = (s.factory)(&rom, console, s.launch.modes[0].variant, &opts) {
                    panic!("{} with a window on every port: {e}", s.name);
                }
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    fn build_703(sub: &str) -> io::Result<Machine> {
        let dir = std::env::temp_dir().join(format!("emu-registry-{sub}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("image.bin");
        std::fs::write(&rom, [0x10u8, 0x40]).unwrap(); // JMP 0x40
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let m = build_ray703(&rom, console, sub, &MachineOpts::default());
        std::fs::remove_dir_all(&dir).ok();
        m
    }

    #[test]
    fn the_panel_subsystem_attaches_a_panel_display() {
        let mut m = build_703("panel").unwrap();
        assert_eq!(m.throttle_hz, Some(crate::cpu::ray703::CLOCK_HZ));
        // the display's sender delivers to the machine's receiver
        let Some(Display::Ray703 { panel: Some(panel), tty: None, .. }) = m.display else {
            panic!("panel display expected");
        };
        panel.control.send(crate::console::PanelCommand::Run).unwrap();
        let channel = m.control.expect("panel machines carry the control channel");
        assert!(matches!(
            channel.rx.try_recv().unwrap(),
            Control::Panel(crate::console::PanelCommand::Run)
        ));
        // ...the rack rides along, wired to the controller: polling the bus
        // advances its duty denominator (what a unit's ONLINE lamp shows
        // depends on which images the working directory happens to hold, so
        // the cycle counter is the assertion that travels)
        let before = panel.rack.snapshot().cycles;
        m.bus.poll_interrupt_lines(100);
        assert_eq!(panel.rack.snapshot().cycles - before, 100);
        // ...and the tokens compose with ptb in either order
        for sub in ["panel-ptb", "ptb-panel"] {
            let m = build_703(sub).unwrap();
            assert!(matches!(m.display, Some(Display::Ray703 { .. })), "{sub}");
            assert!(m.control.is_some(), "{sub}");
        }
    }

    #[test]
    fn plain_ray703_is_headless_and_unthrottled() {
        let m = build_703("").unwrap();
        assert!(m.display.is_none());
        assert_eq!(m.throttle_hz, None);
        assert!(m.control.is_none());
    }

    /// `--fast-io` reaches the teletype through the factory. The probe is
    /// the second keystroke: the first rides the device's standing credit
    /// either way, but the second is available on the very next poll only
    /// when the pacing is off.
    #[test]
    fn fast_io_reaches_the_teletype() {
        for (fast_io, second_key) in [(true, 1u16), (false, 0u16)] {
            let dir = std::env::temp_dir()
                .join(format!("emu-registry-fastio-{fast_io}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let rom = dir.join("image.bin");
            std::fs::write(&rom, [0x10u8, 0x40]).unwrap(); // JMP 0x40
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(b'A').unwrap();
            tx.send(b'B').unwrap();
            let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
            let opts = MachineOpts { fast_io, ..Default::default() };
            let mut m = build_ray703(&rom, console, "", &opts).unwrap();
            std::fs::remove_dir_all(&dir).ok();

            m.bus.io_write16(0xe9, 0); // DOT 14,9: connect the keyboard
            assert_eq!(m.bus.poll_interrupt_lines(0), 1, "the first key is free either way");
            m.bus.io_read16(0xed); // DIN 14,D: collect it
            assert_eq!(m.bus.poll_interrupt_lines(0), second_key, "fast_io={fast_io}");
        }
    }

    #[test]
    fn junk_703_subsystem_tokens_are_rejected() {
        assert!(build_703("panel-bogus").is_err());
        assert!(build_703("panel-ptb-panel").is_err());
        assert!(build_703("bogus").is_err());
        // a duplicated window token leaves a copy for the core match to
        // reject
        assert!(build_703("tty-tty").is_err());
        assert!(build_703("panel-tty-tty").is_err());
    }

    /// The tty token reroutes the endpoint's serial sink onto the paper:
    /// this drives the whole seam -- factory to endpoint to device to
    /// display payload -- through one teletype write.
    #[test]
    fn the_tty_subsystem_routes_output_onto_the_paper() {
        let mut m = build_703("tty").unwrap();
        let Some(Display::Ray703 { panel: None, tty: Some(tty), .. }) = m.display else {
            panic!("tty display without a panel expected");
        };
        // a teletype window alone does not ask for real time: the device
        // pacing already types at ten characters a second
        assert_eq!(m.throttle_hz, None);
        assert!(m.control.is_none());
        // DOT 14,E with 'A': the device's write function, straight to paper
        m.bus.io_write16(0xee, 0x8000 | b'A' as u16);
        tty.paper.with_lines(|lines, _| assert_eq!(lines[0][0], b'A'));
    }

    /// An Altair built through its factory over an image in a scratch file:
    /// the 8800's `image` subsystem, or the 680's monitor rom.
    fn build_altair(name: &str, sub: &str) -> io::Result<Machine> {
        let dir = std::env::temp_dir().join(format!("emu-registry-{name}-{sub}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("image.bin");
        std::fs::write(&rom, [0u8; 256]).unwrap();
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let m = (find(name).unwrap().factory)(&rom, console, sub, &MachineOpts::default());
        std::fs::remove_dir_all(&dir).ok();
        m
    }

    /// Either Altair's `tty` puts the console on a paper, with or without
    /// a panel and in any order, and a teletype alone asks for no clock of
    /// its own; a second `tty` is refused.
    #[test]
    fn an_altair_tty_is_the_consoles_paper() {
        for sub in ["image-tty", "tty-image", "tty-image-panel", "panel-tty-image"] {
            let mut m = build_altair("altair8800", sub).unwrap();
            let Some(Display::Altair8800 { panel, tty: Some(tty), .. }) = m.display else {
                panic!("{sub}: an 8800 teletype expected");
            };
            assert_eq!(panel.is_some(), sub.contains("panel"), "{sub}");
            assert_eq!(m.throttle_hz.is_some(), sub.contains("panel"), "{sub}");
            m.bus.io_write8(0x11, b'A');
            tty.paper.with_lines(|lines, _| assert_eq!(lines[0][0], b'A', "{sub}"));
        }
        for sub in ["tty", "noterm-tty", "tty-panel"] {
            let mut m = build_altair("altair680", sub).unwrap();
            let Some(Display::Altair680 { panel, tty: Some(tty), .. }) = m.display else {
                panic!("{sub}: a 680 teletype expected");
            };
            assert_eq!(panel.is_some(), sub.contains("panel"), "{sub}");
            m.bus.write8(0xf000, 0x15); // out of master reset
            m.bus.write8(0xf001, b'Z');
            tty.paper.with_lines(|lines, _| assert_eq!(lines[0][0], b'Z', "{sub}"));
        }
        assert!(build_altair("altair8800", "image").unwrap().display.is_none());
        assert!(build_altair("altair680", "").unwrap().display.is_none());
        assert!(build_altair("altair8800", "image-tty-tty").is_err());
        assert!(build_altair("altair680", "tty-tty").is_err());
    }

    /// `--serial window` is teletype 1's line: a paper of its own for what
    /// multiplexer channel 0 prints, and the window's keys for its keyboard
    /// -- with no console window, which stays on the terminal.
    #[test]
    fn a_window_line_is_teletype_1s_paper_and_keyboard() {
        let dir = std::env::temp_dir().join(format!("emu-registry-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("image.bin");
        std::fs::write(&rom, [0x10u8, 0x40]).unwrap(); // JMP 0x40
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let opts = MachineOpts { fast_io: true, serial: vec![SerialSpec::Window], ..Default::default() };
        let mut m = build_ray703(&rom, console, "", &opts).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        let Some(Display::Ray703 { panel: None, tty: None, terminals, .. }) = m.display else {
            panic!("a display with teletype 1's window alone expected");
        };
        let [terminal] = &terminals[..] else { panic!("one terminal window expected") };
        let Some((1, keys)) = &terminal.line else { panic!("teletype 1's window expected") };
        m.bus.io_write16(0xa2, 0); // DOT A,2: channel 0 for output
        assert_eq!(m.bus.poll_interrupt_lines(0), 1);
        assert_eq!(m.bus.io_read16(0xa1), 0, "channel 0, ready");
        m.bus.io_write16(0xa3, 0x80 | b'A' as u16); // DOT A,3
        terminal.paper.with_lines(|lines, _| assert_eq!(lines[0][0], b'A'));
        m.bus.io_write16(0xa1, 0); // DOT A,1: listening
        keys.send(b'K').unwrap();
        assert_eq!(m.bus.poll_interrupt_lines(0), 1);
        assert_eq!(m.bus.io_read16(0xa1), 0x8000 | 0x80 | b'K' as u16);
        terminal.paper.with_lines(|lines, _| assert_eq!(lines[0][1], b'K', "the set prints its key"));
        assert_eq!(m.bus.io_read16(0xef), 0, "nothing reached the console");
    }

    /// A port that is a line to the host has no window to be.
    #[test]
    fn only_a_703_teletype_takes_a_window() {
        assert!(serial::open(&SerialSpec::Window).is_err());
        assert_eq!(SerialSpec::parse("window"), Ok(SerialSpec::Window));
    }

    #[test]
    fn the_window_tokens_compose_in_any_order() {
        for sub in ["panel-tty", "tty-panel", "tty-ptb", "ptb-tty", "panel-tty-ptb"] {
            let m = build_703(sub);
            // three tokens exceed the one-non-window-token rule only if a
            // window token was not stripped; all of these must build
            let m = m.unwrap_or_else(|e| panic!("{sub}: {e}"));
            let Some(Display::Ray703 { panel, tty, .. }) = m.display else {
                panic!("{sub}: display expected");
            };
            assert_eq!(panel.is_some(), sub.contains("panel"), "{sub}");
            assert!(tty.is_some(), "{sub}");
        }
    }
}
