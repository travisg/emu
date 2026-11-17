// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! `--launch`: a picker on the terminal for what to run.
//!
//! The machines and what each can be given come from the registry's
//! `launch` entries (`catalog`), resolved against the files under the
//! directory the emulator runs from; the picker (`picker`) turns the
//! choice into an ordinary command line, which `main` then parses as if it
//! had been typed. So the launcher can start nothing the command line
//! cannot, and the line it prints is the way to start the same thing again.

pub mod catalog;
pub mod picker;
pub mod term;

use picker::{Outcome, Picker};
use std::io;
use std::path::Path;

/// Run the picker over what is under `root`. `Some(args)` is the command
/// line chosen, without the program name and with `extra` at its end;
/// `None` is a quit. The terminal is back as it was by the time this
/// returns.
pub fn run(root: &Path, argv0: &str, extra: Vec<String>) -> io::Result<Option<Vec<String>>> {
    let mut picker = Picker::new(catalog::scan(root), extra);
    let mut screen = term::Screen::open()?;
    let mut drawn_at = None;
    let mut dirty = true;
    loop {
        let size = screen.size();
        if dirty || drawn_at != Some(size) {
            screen.draw(&picker.render(argv0, size.0, size.1))?;
            drawn_at = Some(size);
            dirty = false;
        }
        // a timeout notices a resize
        let Some(key) = screen.key(250)? else {
            continue;
        };
        dirty = true;
        match picker.key(key) {
            Outcome::Continue => {}
            Outcome::Launch(args) => return Ok(Some(args)),
            Outcome::Quit => return Ok(None),
        }
    }
}
