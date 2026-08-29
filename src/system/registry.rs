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
    })
}

fn build_rc2014(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(rc2014::Rc2014::new(rom, console)?),
        display: None,
        throttle_hz: None,
    })
}

fn build_sys09(rom: &Path, console: ConsoleEndpoint, sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6809::Cpu6809::new()),
        bus: Box::new(sys09::System09::new(rom, console, sub)?),
        display: None,
        throttle_hz: None,
    })
}

fn build_ray703(rom: &Path, console: ConsoleEndpoint, sub: &str) -> io::Result<Machine> {
    let mut cpu = crate::cpu::ray703::Cpu703::new();
    // The one thing the 703 needs that the other machines don't: an operator
    // at the front panel. PTB is keyed in with the index register preset to
    // the load origin, so the factory stands in for the operator's hands.
    if let Some(ixr) = ray703::Ray703::ptb_index(sub) {
        cpu.set_index(ixr);
    }
    Ok(Machine {
        cpu: Box::new(cpu),
        bus: Box::new(ray703::Ray703::new(rom, console, sub)?),
        display: None,
        throttle_hz: None,
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
}
