// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Machines, and the registry that describes them.

pub mod altair680;
pub mod altair8800;
pub mod kaypro;
pub mod ray703;
pub mod rc2014;
// The command line's factory: it opens ROM and disk files and serial
// lines and reads the host's clock. A page builds its machine itself.
#[cfg(not(target_arch = "wasm32"))]
pub mod registry;
pub mod sys09;
