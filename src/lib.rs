// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Terminal-driven emulator for several vintage computer systems.
//!
//! Most of it is a port of an earlier C++ emulator, and was validated against
//! that emulator instruction by instruction (`--trace` on both sides) while
//! the port was under way. The C++ tree was removed once the port was
//! complete, and the comparison with it is retired; comments throughout still
//! name the C++ files they came from, and those resolve in the last commit
//! that had them -- see AGENTS.md. The Raytheon 703 is not a port and has no
//! such ancestor.

pub mod bus;
pub mod console;
pub mod cpu;
pub mod dev;
pub mod emulator;
pub mod rom;
pub mod system;
