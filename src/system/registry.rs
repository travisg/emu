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
use std::path::Path;

/// Everything built for one machine: a core, the bus it drives, and -- if it
/// has a screen -- the display handle the main-thread frontend renders from.
pub struct Machine {
    pub cpu: Box<dyn Cpu + Send>,
    pub bus: Box<dyn Bus + Send>,
    /// `None` for terminal-only machines, which use the console's serial
    /// output instead.
    pub display: Option<Display>,
    /// A machine that wants real time by default sets its clock rate here
    /// (a live front panel is meaningless uncapped); `None` runs flat out.
    /// `--throttle` on the command line overrides either way.
    pub throttle_hz: Option<u64>,
    /// The receiving end of a front panel's command channel, for
    /// `Emulator::set_panel_control`; its presence is also what makes HLT
    /// halt to the panel instead of exiting.
    pub panel_control: Option<std::sync::mpsc::Receiver<crate::console::PanelCommand>>,
}

/// `subsystem` is the part after the dash in e.g. `6809-obc`, or "".
type FactoryFn = fn(&Path, ConsoleEndpoint, &str) -> io::Result<Machine>;

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

fn build_altair680(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6800::Cpu6800::new()),
        bus: Box::new(altair680::Altair680::new(rom, console)?),
        display: None,
        throttle_hz: None,
        panel_control: None,
    })
}

fn build_rc2014(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(rc2014::Rc2014::new(rom, console)?),
        display: None,
        throttle_hz: None,
        panel_control: None,
    })
}

fn build_sys09(rom: &Path, console: ConsoleEndpoint, sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6809::Cpu6809::new()),
        bus: Box::new(sys09::System09::new(rom, console, sub)?),
        display: None,
        throttle_hz: None,
        panel_control: None,
    })
}

fn build_ray703(rom: &Path, console: ConsoleEndpoint, sub: &str) -> io::Result<Machine> {
    // The subsystem is a set of tokens: "panel" opens the front panel
    // window, "ptb" keys in the bootstrap, in either order. Strip "panel"
    // here and hand the rest to the machine, which knows nothing about
    // displays.
    let mut tokens: Vec<&str> = sub.split('-').filter(|t| !t.is_empty()).collect();
    let panel = tokens.iter().position(|&t| t == "panel").map(|i| tokens.remove(i)).is_some();
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

    let mut display = None;
    let mut throttle_hz = None;
    let mut panel_control = None;
    if panel {
        let state = crate::console::PanelState::new();
        cpu.attach_panel(state.clone());
        // switch actuations flow frontend -> run loop over this channel
        let (ctl_tx, ctl_rx) = std::sync::mpsc::channel();
        display = Some(Display::Panel703 { title: "Raytheon 703", panel: state, control: ctl_tx });
        panel_control = Some(ctl_rx);
        // A live panel is meaningless uncapped -- the lamps would be a
        // uniform blur -- so panel machines default to real time. --throttle
        // on the command line still overrides.
        throttle_hz = Some(crate::cpu::ray703::CLOCK_HZ);
    }

    Ok(Machine {
        cpu: Box::new(cpu),
        bus: Box::new(ray703::Ray703::new(rom, console, sub)?),
        display,
        throttle_hz,
        panel_control,
    })
}

fn build_kaypro(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    let (bus, display) = kaypro::Kaypro::new(
        rom,
        Path::new(kaypro::VIDEO_ROM),
        Path::new(kaypro::DEFAULT_FLOPPY),
        console,
    )?;
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(bus),
        display: Some(display),
        throttle_hz: None,
        panel_control: None,
    })
}

pub static SYSTEMS: &[SystemDescriptor] = &[
    SystemDescriptor {
        name: "6809",
        cpu: "6809",
        default_rom: sys09::DEFAULT_ROM,
        factory: build_sys09,
        clock_hz: None,
    },
    SystemDescriptor {
        name: "altair680",
        cpu: "6800",
        default_rom: altair680::DEFAULT_ROM,
        factory: build_altair680,
        clock_hz: None,
    },
    SystemDescriptor {
        name: "kaypro",
        cpu: "z80",
        default_rom: kaypro::DEFAULT_ROM,
        factory: build_kaypro,
        clock_hz: None,
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
        clock_hz: None,
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
        let m = build_ray703(&rom, console, sub);
        std::fs::remove_dir_all(&dir).ok();
        m
    }

    #[test]
    fn the_panel_subsystem_attaches_a_panel_display() {
        let m = build_703("panel").unwrap();
        assert_eq!(m.throttle_hz, Some(crate::cpu::ray703::CLOCK_HZ));
        // the display's sender delivers to the machine's receiver
        let Some(Display::Panel703 { control, .. }) = m.display else {
            panic!("panel display expected");
        };
        control.send(crate::console::PanelCommand::Run).unwrap();
        let rx = m.panel_control.expect("panel machines carry the control channel");
        assert_eq!(rx.try_recv().unwrap(), crate::console::PanelCommand::Run);
        // ...and the tokens compose with ptb in either order
        for sub in ["panel-ptb", "ptb-panel"] {
            let m = build_703(sub).unwrap();
            assert!(matches!(m.display, Some(Display::Panel703 { .. })), "{sub}");
            assert!(m.panel_control.is_some(), "{sub}");
        }
    }

    #[test]
    fn plain_ray703_is_headless_and_unthrottled() {
        let m = build_703("").unwrap();
        assert!(m.display.is_none());
        assert_eq!(m.throttle_hz, None);
        assert!(m.panel_control.is_none());
    }

    #[test]
    fn junk_703_subsystem_tokens_are_rejected() {
        assert!(build_703("panel-bogus").is_err());
        assert!(build_703("panel-ptb-panel").is_err());
        assert!(build_703("bogus").is_err());
    }
}
