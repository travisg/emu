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
use crate::system::altair680;
use std::io;
use std::path::Path;

/// Everything built for one machine: a core and the bus it drives.
pub struct Machine {
    pub cpu: Box<dyn Cpu + Send>,
    pub bus: Box<dyn Bus + Send>,
}

type FactoryFn = fn(&Path, ConsoleEndpoint) -> io::Result<Machine>;

pub struct SystemDescriptor {
    pub name: &'static str,
    pub cpu: &'static str,
    pub default_rom: &'static str,
    pub factory: FactoryFn,
}

fn build_altair680(rom: &Path, console: ConsoleEndpoint) -> io::Result<Machine> {
    Ok(Machine {
        cpu: Box::new(crate::cpu::m6800::Cpu6800::new()),
        bus: Box::new(altair680::Altair680::new(rom, console)?),
    })
}

pub static SYSTEMS: &[SystemDescriptor] = &[SystemDescriptor {
    name: "altair680",
    cpu: "6800",
    default_rom: altair680::DEFAULT_ROM,
    factory: build_altair680,
}];

/// Look a machine up by name. A subsystem suffix (`6809-obc`) selects a
/// variant; only the part before the dash names the system.
pub fn find(name: &str) -> Option<&'static SystemDescriptor> {
    let base = name.split('-').next().unwrap_or(name);
    SYSTEMS.iter().find(|s| s.name == base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_known_system() {
        assert_eq!(find("altair680").map(|s| s.cpu), Some("6800"));
    }

    #[test]
    fn a_subsystem_suffix_selects_the_base_system() {
        assert!(find("altair680-obc").is_some());
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
