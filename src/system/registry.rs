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
//! hardcoded anywhere else (the help text is generated from this table).

use crate::bus::Bus;
use crate::console::{ConsoleEndpoint, Display};
use crate::cpu::Cpu;
use crate::system::{altair680, kaypro, ray703, rc2014, sys09};
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
    /// front panel: the panel window already holds a clone of the sender
    /// inside `display`, and `main` hands the receiver to the `Emulator`.
    pub control: Option<ControlChannel>,
    /// A handle on the panel's shared lamp state, for
    /// `Emulator::set_panel_state` -- the run loop publishes whether the
    /// machine is halted, which lights the HALT indicator's red lens.
    /// The frontend's own handle rides inside `display`.
    pub panel: Option<crate::console::PanelState>,
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
    /// `--disk PATH`: the image for the machine's disk drive, in place of
    /// the fixed name under `disks/`. The Kaypro's floppy takes it; the 703
    /// mounts four units by name and ignores it.
    pub disk: Option<PathBuf>,
}

/// `subsystem` is the part after the dash in e.g. `6809-obc`, or "".
type FactoryFn = fn(&Path, ConsoleEndpoint, &str, &MachineOpts) -> io::Result<Machine>;

pub struct SystemDescriptor {
    pub name: &'static str,
    pub cpu: &'static str,
    pub default_rom: &'static str,
    pub factory: FactoryFn,
    /// The real machine's clock rate, used by a bare `--throttle`. `None`
    /// until the rate is verified *and* the core reports cycle counts --
    /// throttling a core that reports none is an announced no-op.
    pub clock_hz: Option<u64>,
}

fn build_altair680(rom: &Path, console: ConsoleEndpoint, _sub: &str, _opts: &MachineOpts) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6800::Cpu6800::new()),
        bus: Box::new(altair680::Altair680::new(rom, console)?),
        display: None,
        throttle_hz: None,
        control: None,
        panel: None,
    })
}

fn build_rc2014(rom: &Path, console: ConsoleEndpoint, sub: &str, opts: &MachineOpts) -> io::Result<Machine> {
    let mut bus = rc2014::Rc2014::new(rom, console, sub)?;
    if opts.fast_io {
        bus.set_fast_io();
    }
    // The card mounts like the Kaypro's floppy: `--disk` or the fixed name
    // under disks/, non-fatal, gitignored. Only the cpm build has the slot.
    if bus.has_compact_flash() {
        let card = opts.disk.clone().unwrap_or_else(|| PathBuf::from(rc2014::DEFAULT_CF));
        bus.mount_compact_flash(&card);
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
        tty_display = Some(crate::console::TtyDisplay { paper });
    }
    let display = (panel || tty).then_some(Display::Ray703 {
        title: "Raytheon 703",
        panel: panel_display,
        tty: tty_display,
    });

    let mut bus = ray703::Ray703::new(rom, console, sub)?;
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
        panel: panel_state,
    })
}

fn build_kaypro(rom: &Path, console: ConsoleEndpoint, _sub: &str, opts: &MachineOpts) -> io::Result<Machine> {
    let floppy = opts.disk.clone().unwrap_or_else(|| PathBuf::from(kaypro::DEFAULT_FLOPPY));
    let (mut bus, display) = kaypro::Kaypro::new(
        rom,
        Path::new(kaypro::VIDEO_ROM),
        &floppy,
        console,
    )?;
    if opts.fast_io {
        bus.set_fast_io();
    }
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(bus),
        display: Some(display),
        throttle_hz: None,
        control: None,
        panel: None,
    })
}

pub static SYSTEMS: &[SystemDescriptor] = &[
    SystemDescriptor {
        name: "6809",
        cpu: "6809",
        default_rom: sys09::DEFAULT_ROM,
        factory: build_sys09,
        // Grant Searle's Simple 6809, where the default rom comes from: a
        // 7.3728 MHz crystal the 6809 divides by four, so the E clock the
        // datasheet's cycle counts tick at is 1.8432 MHz.
        clock_hz: Some(1_843_200),
    },
    SystemDescriptor {
        name: "altair680",
        cpu: "6800",
        default_rom: altair680::DEFAULT_ROM,
        factory: build_altair680,
        // The 680 ran its 6800 at 500 kHz -- MITS shipped it slow to suit
        // the memory it came with.
        clock_hz: Some(500_000),
    },
    SystemDescriptor {
        name: "kaypro",
        cpu: "z80",
        default_rom: kaypro::DEFAULT_ROM,
        factory: build_kaypro,
        // The stock Kaypro II Z80 rate (the 5 MHz machines came later).
        clock_hz: Some(kaypro::CLOCK_HZ),
    },
    SystemDescriptor {
        name: "ray703",
        cpu: "703",
        default_rom: ray703::DEFAULT_ROM,
        factory: build_ray703,
        clock_hz: Some(crate::cpu::ray703::CLOCK_HZ),
    },
    SystemDescriptor {
        name: "rc2014",
        cpu: "z80",
        default_rom: rc2014::DEFAULT_ROM,
        factory: build_rc2014,
        // The standard RC2014 crystal, 7.3728 MHz -- chosen so the serial
        // clock divides down to 115200 baud, and the Z80 runs off the same
        // can.
        clock_hz: Some(rc2014::CLOCK_HZ),
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

    /// Build a ray703 machine for one subsystem string. Uses the demo image
    /// only as bytes to load, via a temp file, so no ROM symlink is needed.
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
