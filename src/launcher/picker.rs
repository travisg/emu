// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The picker's state, what the keys do to it, the command line it stands
//! for, and how it is drawn -- all without touching the terminal, which is
//! `term.rs`'s business, so every part of it is testable.
//!
//! Two panes: the machines on the left, and on the right the selected
//! machine's items (one per mode and file found) followed by its switches.
//! The cursor on an item selects it, so the switches below can be changed
//! without losing which item Enter will run.

use super::catalog::Entry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Tab,
    BackTab,
    Enter,
    Space,
    Esc,
    Quit,
    Other,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Continue,
    /// The arguments to run, without the program name.
    Launch(Vec<String>),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    Machines,
    Items,
}

/// A switch on the right pane, after the items.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Switch {
    /// The machine's `launch.windows[i]`.
    Window(usize),
    FastIo,
    FlatOut,
}

pub struct Picker {
    entries: Vec<Entry>,
    machine: usize,
    focus: Focus,
    /// The cursor on the right pane: items first, then switches.
    row: usize,
    /// Per machine: the selected item and the window switches.
    item: Vec<usize>,
    windows: Vec<Vec<bool>>,
    fast_io: bool,
    flat_out: bool,
    /// Arguments from the command line, after the launcher's own.
    extra: Vec<String>,
    /// One line of feedback, cleared by the next key.
    message: Option<String>,
}

impl Picker {
    pub fn new(entries: Vec<Entry>, extra: Vec<String>) -> Picker {
        let item = entries.iter().map(|e| e.items.iter().position(|i| i.is_default).unwrap_or(0)).collect();
        let windows = entries.iter().map(|e| vec![false; e.desc.launch.windows.len()]).collect();
        // start on the first machine there is something to run on
        let machine = entries.iter().position(|e| !e.items.is_empty()).unwrap_or(0);
        Picker {
            entries,
            machine,
            focus: Focus::Machines,
            row: 0,
            item,
            windows,
            fast_io: false,
            flat_out: false,
            extra,
            message: None,
        }
    }

    fn entry(&self) -> &Entry {
        &self.entries[self.machine]
    }

    fn switches(&self) -> Vec<Switch> {
        let mut s: Vec<Switch> = (0..self.entry().desc.launch.windows.len()).map(Switch::Window).collect();
        s.extend([Switch::FastIo, Switch::FlatOut]);
        s
    }

    /// The switch under the cursor, if the cursor is on one.
    fn switch_at_cursor(&self) -> Option<Switch> {
        let n = self.entry().items.len();
        (self.focus == Focus::Items && self.row >= n).then(|| self.switches()[self.row - n])
    }

    fn rows(&self) -> usize {
        self.entry().items.len() + self.switches().len()
    }

    fn toggle(&mut self, s: Switch) {
        match s {
            Switch::Window(i) => {
                let w = &mut self.windows[self.machine][i];
                *w = !*w;
            }
            Switch::FastIo => self.fast_io = !self.fast_io,
            Switch::FlatOut => self.flat_out = !self.flat_out,
        }
    }

    fn switch_on(&self, s: Switch) -> bool {
        match s {
            Switch::Window(i) => self.windows[self.machine][i],
            Switch::FastIo => self.fast_io,
            Switch::FlatOut => self.flat_out,
        }
    }

    fn enter_items(&mut self) {
        self.focus = Focus::Items;
        // onto the selected item, or the first switch when there is none
        self.row = self.item[self.machine].min(self.rows() - 1);
    }

    fn move_row(&mut self, to: usize) {
        self.row = to.min(self.rows() - 1);
        if self.row < self.entry().items.len() {
            self.item[self.machine] = self.row;
        }
    }

    pub fn key(&mut self, key: Key) -> Outcome {
        self.message = None;
        match (self.focus, key) {
            (_, Key::Quit) | (Focus::Machines, Key::Esc) => return Outcome::Quit,
            (_, Key::Enter) if self.focus == Focus::Items || self.entry().items.is_empty() => {
                return match self.args() {
                    Some(args) => Outcome::Launch(args),
                    None => {
                        self.message = Some(format!("nothing to run {} on yet; see below", self.entry().desc.launch.name));
                        Outcome::Continue
                    }
                };
            }
            (Focus::Machines, Key::Up) => self.machine = self.machine.saturating_sub(1),
            (Focus::Machines, Key::Down) => self.machine = (self.machine + 1).min(self.entries.len() - 1),
            (Focus::Machines, Key::Home) => self.machine = 0,
            (Focus::Machines, Key::End) => self.machine = self.entries.len() - 1,
            (Focus::Machines, Key::Right | Key::Tab | Key::Enter | Key::Space) => self.enter_items(),
            (Focus::Items, Key::Esc | Key::Tab | Key::BackTab) => self.focus = Focus::Machines,
            (Focus::Items, Key::Up) => self.move_row(self.row.saturating_sub(1)),
            (Focus::Items, Key::Down) => self.move_row(self.row + 1),
            (Focus::Items, Key::Home) => self.move_row(0),
            (Focus::Items, Key::End) => self.move_row(usize::MAX),
            (Focus::Items, Key::Space | Key::Left | Key::Right) => match self.switch_at_cursor() {
                Some(s) => self.toggle(s),
                None if key == Key::Left => self.focus = Focus::Machines,
                None => {}
            },
            _ => {}
        }
        Outcome::Continue
    }

    /// The arguments the current selection stands for, without the program
    /// name, or `None` when the machine has nothing to run.
    pub fn args(&self) -> Option<Vec<String>> {
        let e = self.entry();
        let item = e.items.get(self.item[self.machine])?;
        let mut system = vec![e.desc.name];
        if !item.variant.is_empty() {
            system.push(item.variant);
        }
        for (on, (token, _)) in self.windows[self.machine].iter().zip(e.desc.launch.windows) {
            if *on {
                system.push(token);
            }
        }
        let mut args = vec!["-s".to_string(), system.join("-")];
        if let Some(rom) = &item.rom {
            args.extend(["-r".to_string(), rom.display().to_string()]);
        }
        if let Some(disk) = &item.disk {
            args.extend(["--disk".to_string(), disk.display().to_string()]);
        }
        if self.fast_io {
            args.push("--fast-io".to_string());
        }
        if self.flat_out {
            args.push("--no-throttle".to_string());
        }
        args.extend(self.extra.iter().cloned());
        Some(args)
    }

    /// The screen, `height` lines each of at most `width` columns, with
    /// ANSI attributes. `argv0` is what the command line preview runs.
    pub fn render(&self, argv0: &str, width: usize, height: usize) -> Vec<String> {
        let width = width.max(20);
        let height = height.max(8);
        let mut lines = Vec::with_capacity(height);
        lines.push(style(REVERSE, &fit(" emu launcher: pick a machine and what to load", width)));

        // the left pane
        let left_w = self.entries.iter().map(|e| e.desc.launch.name.len()).max().unwrap_or(0) + 4;
        let left: Vec<String> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mark = if i == self.machine { '>' } else { ' ' };
                let text = fit(&format!(" {mark} {}", e.desc.launch.name), left_w);
                let attr = match (i == self.machine, self.focus == Focus::Machines, e.items.is_empty()) {
                    (true, true, _) => REVERSE,
                    (true, false, _) => BOLD,
                    (false, _, true) => DIM,
                    _ => "",
                };
                style(attr, &text)
            })
            .collect();

        // the right pane: (text, attribute) and which row each line is
        let right_w = width.saturating_sub(left_w + 3);
        let e = self.entry();
        let mut right: Vec<(String, &str, Option<usize>)> = Vec::new();
        let clock = e.desc.clock_hz.map(|hz| format!(" at {}", crate::system::registry::fmt_hz(hz))).unwrap_or_default();
        right.push((e.desc.title.to_string(), BOLD, None));
        right.push((format!("cpu {}{clock}", e.desc.cpu), DIM, None));
        right.push((String::new(), "", None));
        let what_w = e.items.iter().map(|i| i.what.len()).max().unwrap_or(0);
        for (n, item) in e.items.iter().enumerate() {
            let radio = if n == self.item[self.machine] { "(*)" } else { "( )" };
            let file = match (&item.rom, &item.disk) {
                (Some(p), _) | (None, Some(p)) => p.display().to_string(),
                (None, None) => e.desc.default_rom.to_string(),
            };
            let default = if item.is_default { "  (default)" } else { "" };
            right.push((format!("{radio} {:<what_w$}  {file}{default}", item.what), "", Some(n)));
        }
        if !e.items.is_empty() {
            right.push((String::new(), "", None));
        }
        let n_items = e.items.len();
        for (k, s) in self.switches().into_iter().enumerate() {
            let label = match s {
                Switch::Window(i) => {
                    let (token, what) = e.desc.launch.windows[i];
                    format!("{what} ({token})")
                }
                Switch::FastIo => "devices finish i/o at once (--fast-io)".to_string(),
                Switch::FlatOut => "run flat out, not at the real clock (--no-throttle)".to_string(),
            };
            let check = if self.switch_on(s) { "[x]" } else { "[ ]" };
            right.push((format!("{check} {label}"), "", Some(n_items + k)));
        }
        if !e.missing.is_empty() {
            right.push((String::new(), "", None));
            right.push(("not here:".to_string(), DIM, None));
            for m in &e.missing {
                right.push((format!("  {m}"), DIM, None));
            }
        }

        // two lines at the bottom: the command line and the keys
        let body = height - 1 - 1 - 2;
        // scroll the right pane to keep the cursor in view
        let cursor_line = right.iter().position(|(_, _, r)| *r == Some(self.row)).unwrap_or(0);
        let top = if self.focus == Focus::Items && cursor_line >= body { cursor_line + 1 - body } else { 0 };
        lines.push(String::new());
        for i in 0..body {
            let l = left.get(i).cloned().unwrap_or_else(|| " ".repeat(left_w));
            let r = match right.get(top + i) {
                Some((text, attr, row)) => {
                    let on_cursor = self.focus == Focus::Items && *row == Some(self.row);
                    style(if on_cursor { REVERSE } else { attr }, &fit(text, right_w))
                }
                None => String::new(),
            };
            lines.push(format!("{l} | {r}"));
        }
        let command = match self.args() {
            Some(args) => {
                let words: Vec<String> = std::iter::once(argv0.to_string()).chain(args).map(|a| quote(&a)).collect();
                style(BOLD, &fit(&format!(" $ {}", words.join(" ")), width))
            }
            None => style(DIM, &fit(" nothing to run on this machine", width)),
        };
        lines.push(command);
        let help = match &self.message {
            Some(m) => style(BOLD, &fit(&format!(" {m}"), width)),
            None => style(
                DIM,
                &fit(
                    match self.focus {
                        Focus::Machines => " up/down machine   right/tab choose   enter run   q quit",
                        Focus::Items => " up/down choose   space toggle   enter run   tab/esc machines   q quit",
                    },
                    width,
                ),
            ),
        };
        lines.push(help);
        lines
    }
}

const REVERSE: &str = "\x1b[7m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

fn style(attr: &str, text: &str) -> String {
    if attr.is_empty() {
        text.to_string()
    } else {
        format!("{attr}{text}\x1b[0m")
    }
}

/// `text` cut or padded to exactly `w` columns.
fn fit(text: &str, w: usize) -> String {
    let n = text.chars().count();
    if n > w {
        text.chars().take(w).collect()
    } else {
        format!("{text}{}", " ".repeat(w - n))
    }
}

/// A word as a shell would need it typed.
pub fn quote(s: &str) -> String {
    let plain = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=+,@%".contains(c));
    if plain {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::catalog::Item;
    use crate::system::registry;
    use std::path::PathBuf;

    fn entry(name: &str, items: Vec<Item>) -> Entry {
        Entry { desc: registry::find(name).unwrap(), items, missing: vec![] }
    }

    fn rom(variant: &'static str, path: &str, is_default: bool) -> Item {
        Item { variant, what: "image", rom: Some(PathBuf::from(path)), disk: None, is_default }
    }

    fn picker() -> Picker {
        Picker::new(
            vec![
                entry("6809", vec![]),
                entry(
                    "ray703",
                    vec![
                        rom("", "roms/703/basic.bin", false),
                        rom("", "roms/703/demo.bin", true),
                        rom("load", "rex/build/rex.img", false),
                    ],
                ),
                entry(
                    "kaypro",
                    vec![Item { variant: "", what: "floppy", rom: None, disk: Some("disks/a b.img".into()), is_default: false }],
                ),
            ],
            vec![],
        )
    }

    fn keys(p: &mut Picker, ks: &[Key]) -> Outcome {
        let mut last = Outcome::Continue;
        for &k in ks {
            last = p.key(k);
        }
        last
    }

    fn launch(words: &[&str]) -> Outcome {
        Outcome::Launch(words.iter().map(|w| w.to_string()).collect())
    }

    #[test]
    fn it_starts_on_the_first_runnable_machine_and_its_default_item() {
        let mut p = picker();
        assert_eq!(p.key(Key::Enter), Outcome::Continue, "enter on a machine moves to its items");
        assert_eq!(p.key(Key::Enter), launch(&["-s", "ray703", "-r", "roms/703/demo.bin"]));
    }

    #[test]
    fn moving_onto_an_item_selects_it_and_switches_keep_it() {
        let mut p = picker();
        // to rex.img, then down past it onto the panel and tty switches
        keys(&mut p, &[Key::Tab, Key::Down, Key::Down, Key::Space, Key::Down, Key::Right, Key::Down, Key::Space]);
        assert_eq!(
            p.key(Key::Enter),
            launch(&["-s", "ray703-load-panel-tty", "-r", "rex/build/rex.img", "--fast-io"])
        );
        // toggled twice is off again, and the others stay as they were
        keys(&mut p, &[Key::Up, Key::Left]);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "ray703-load-panel", "-r", "rex/build/rex.img", "--fast-io"]));
    }

    #[test]
    fn a_machine_with_nothing_to_run_says_so_instead_of_launching() {
        let mut p = picker();
        keys(&mut p, &[Key::Up]);
        assert_eq!(p.key(Key::Enter), Outcome::Continue);
        let screen = p.render("emu", 80, 24).join("\n");
        assert!(screen.contains("nothing to run System09 on yet"), "{screen}");
    }

    #[test]
    fn a_disk_item_runs_with_disk_and_extra_arguments_come_last() {
        let mut p = picker();
        p.extra = vec!["--debug".into(), "/tmp/s".into()];
        keys(&mut p, &[Key::End, Key::Enter]);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "kaypro", "--disk", "disks/a b.img", "--debug", "/tmp/s"]));
        let screen = p.render("emu", 100, 24).join("\n");
        assert!(screen.contains("$ emu -s kaypro --disk 'disks/a b.img' --debug /tmp/s"), "{screen}");
    }

    #[test]
    fn left_on_an_item_and_escape_go_back_and_escape_there_quits() {
        let mut p = picker();
        keys(&mut p, &[Key::Right, Key::Left]);
        assert_eq!(p.focus, Focus::Machines);
        keys(&mut p, &[Key::Right, Key::Esc]);
        assert_eq!(p.focus, Focus::Machines);
        assert_eq!(p.key(Key::Esc), Outcome::Quit);
        assert_eq!(picker().key(Key::Quit), Outcome::Quit);
    }

    #[test]
    fn the_screen_fits_the_terminal_and_scrolls_to_the_cursor() {
        let mut p = picker();
        keys(&mut p, &[Key::Tab, Key::End]);
        let screen = p.render("emu", 60, 10);
        assert_eq!(screen.len(), 10);
        let visible = |l: &str| {
            let mut out = String::new();
            let mut esc = false;
            for c in l.chars() {
                match (esc, c) {
                    (false, '\x1b') => esc = true,
                    (true, 'm') => esc = false,
                    (true, _) => {}
                    (false, c) => out.push(c),
                }
            }
            out
        };
        for l in &screen {
            assert!(visible(l).chars().count() <= 60, "{l:?}");
        }
        assert!(screen.iter().any(|l| l.contains("run flat out") && l.contains(REVERSE)), "{screen:#?}");
    }

    #[test]
    fn quoting_leaves_plain_words_alone() {
        assert_eq!(quote("roms/703/demo.bin"), "roms/703/demo.bin");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }
}
