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
use crate::console::ConsoleEndpoint;
use crate::cpu::Cpu;
use crate::system::{altair680, rc2014, sys09};
use std::io;
use std::path::Path;

/// Everything built for one machine: a core and the bus it drives.
pub struct Machine {
    pub cpu: Box<dyn Cpu + Send>,
    pub bus: Box<dyn Bus + Send>,
}

/// `subsystem` is the part after the dash in e.g. `6809-obc`, or "".
type FactoryFn = fn(&Path, ConsoleEndpoint, &str) -> io::Result<Machine>;

pub struct SystemDescriptor {
    pub name: &'static str,
    pub cpu: &'static str,
    pub default_rom: &'static str,
    pub factory: FactoryFn,
}

fn build_altair680(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6800::Cpu6800::new()),
        bus: Box::new(altair680::Altair680::new(rom, console)?),
    })
}

fn build_rc2014(rom: &Path, console: ConsoleEndpoint, _sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::z80::CpuZ80::new()),
        bus: Box::new(rc2014::Rc2014::new(rom, console)?),
    })
}

fn build_sys09(rom: &Path, console: ConsoleEndpoint, sub: &str) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6809::Cpu6809::new()),
        bus: Box::new(sys09::System09::new(rom, console, sub)?),
    })
}

pub static SYSTEMS: &[SystemDescriptor] = &[
    SystemDescriptor {
        name: "6809",
        cpu: "6809",
        default_rom: sys09::DEFAULT_ROM,
        factory: build_sys09,
    },
    SystemDescriptor {
        name: "altair680",
        cpu: "6800",
        default_rom: altair680::DEFAULT_ROM,
        factory: build_altair680,
    },
    SystemDescriptor {
        name: "rc2014",
        cpu: "z80",
        default_rom: rc2014::DEFAULT_ROM,
        factory: build_rc2014,
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
        }
    }
}
