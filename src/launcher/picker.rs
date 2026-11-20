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
//! Three columns: the machines, the selected machine's items (one per mode
//! and file found), and its options (window tokens, spare serial ports and
//! the speed switches). Each column keeps its own cursor, so an item stays
//! selected while the options are changed and the other way round.

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
    Options,
}

/// A row of the options column.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Opt {
    /// The machine's `launch.windows[i]`.
    Window(usize),
    /// The machine's `serial_ports[i]`.
    Serial(usize),
    FastIo,
    FlatOut,
}

/// What a spare serial port is plugged into.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Line {
    Off,
    Window,
    Pty,
    Tcp,
}

impl Line {
    /// The next setting a space gives, `Window` only where the machine
    /// takes one.
    fn next(self, window: bool) -> Line {
        match self {
            Line::Off if window => Line::Window,
            Line::Off | Line::Window => Line::Pty,
            Line::Pty => Line::Tcp,
            Line::Tcp => Line::Off,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Line::Off => "off",
            Line::Window => "window",
            Line::Pty => "pty",
            Line::Tcp => "tcp",
        }
    }

    /// The `--serial` argument; a port of 0 is a free one, named at startup.
    fn spec(self) -> Option<&'static str> {
        match self {
            Line::Off => None,
            Line::Window => Some("window"),
            Line::Pty => Some("pty"),
            Line::Tcp => Some("tcp:0"),
        }
    }
}

pub struct Picker {
    entries: Vec<Entry>,
    machine: usize,
    focus: Focus,
    /// The options column's cursor.
    opt: usize,
    /// Per machine: the selected item, the window switches and the serial
    /// ports.
    item: Vec<usize>,
    windows: Vec<Vec<bool>>,
    serial: Vec<Vec<Line>>,
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
        let serial = entries.iter().map(|e| vec![Line::Off; e.desc.serial_ports.len()]).collect();
        // start on the first machine there is something to run on
        let machine = entries.iter().position(|e| !e.items.is_empty()).unwrap_or(0);
        Picker {
            entries,
            machine,
            focus: Focus::Machines,
            opt: 0,
            item,
            windows,
            serial,
            fast_io: false,
            flat_out: false,
            extra,
            message: None,
        }
    }

    fn entry(&self) -> &Entry {
        &self.entries[self.machine]
    }

    /// The options column. `--serial` fills the ports in turn, so a port is
    /// offered once the one before it is plugged in.
    fn options(&self) -> Vec<Opt> {
        let mut o: Vec<Opt> = (0..self.entry().desc.launch.windows.len()).map(Opt::Window).collect();
        let lines = &self.serial[self.machine];
        let shown = lines.iter().position(|l| *l == Line::Off).map_or(lines.len(), |off| off + 1);
        o.extend((0..shown).map(Opt::Serial));
        o.extend([Opt::FastIo, Opt::FlatOut]);
        o
    }

    fn change(&mut self, o: Opt) {
        match o {
            Opt::Window(i) => {
                let w = &mut self.windows[self.machine][i];
                *w = !*w;
            }
            Opt::Serial(i) => {
                let window = self.entry().desc.launch.serial_window;
                let l = &mut self.serial[self.machine][i];
                *l = l.next(window);
            }
            Opt::FastIo => self.fast_io = !self.fast_io,
            Opt::FlatOut => self.flat_out = !self.flat_out,
        }
    }

    /// The column after (or before) this one, past an items column with
    /// nothing in it.
    fn column(&self, from: Focus, forward: bool, wrap: bool) -> Focus {
        let has_items = !self.entry().items.is_empty();
        match (from, forward) {
            (Focus::Machines, true) if has_items => Focus::Items,
            (Focus::Machines | Focus::Items, true) => Focus::Options,
            (Focus::Options, false) if has_items => Focus::Items,
            (Focus::Options | Focus::Items, false) => Focus::Machines,
            (Focus::Options, true) if wrap => Focus::Machines,
            (Focus::Machines, false) if wrap => Focus::Options,
            (f, _) => f,
        }
    }

    fn move_cursor(&mut self, to: usize) {
        match self.focus {
            Focus::Machines => {
                self.machine = to.min(self.entries.len() - 1);
                self.opt = self.opt.min(self.options().len() - 1);
            }
            Focus::Items => self.item[self.machine] = to.min(self.entry().items.len() - 1),
            Focus::Options => self.opt = to.min(self.options().len() - 1),
        }
    }

    fn cursor(&self) -> usize {
        match self.focus {
            Focus::Machines => self.machine,
            Focus::Items => self.item[self.machine],
            Focus::Options => self.opt,
        }
    }

    pub fn key(&mut self, key: Key) -> Outcome {
        self.message = None;
        match (self.focus, key) {
            (_, Key::Quit) | (Focus::Machines, Key::Esc) => return Outcome::Quit,
            (Focus::Machines, Key::Enter) if !self.entry().items.is_empty() => self.focus = Focus::Items,
            (_, Key::Enter) => {
                return match self.args() {
                    Some(args) => Outcome::Launch(args),
                    None => {
                        self.message = Some(format!("nothing to run {} on yet; see below", self.entry().desc.launch.name));
                        Outcome::Continue
                    }
                };
            }
            (_, Key::Esc) => self.focus = Focus::Machines,
            (Focus::Machines, Key::Space) => self.focus = self.column(self.focus, true, false),
            (Focus::Options, Key::Space) => self.change(self.options()[self.opt]),
            (f, Key::Tab) => self.focus = self.column(f, true, true),
            (f, Key::BackTab) => self.focus = self.column(f, false, true),
            (f, Key::Right) => self.focus = self.column(f, true, false),
            (f, Key::Left) => self.focus = self.column(f, false, false),
            (_, Key::Up) => self.move_cursor(self.cursor().saturating_sub(1)),
            (_, Key::Down) => self.move_cursor(self.cursor() + 1),
            (_, Key::Home) => self.move_cursor(0),
            (_, Key::End) => self.move_cursor(usize::MAX),
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
        for spec in self.serial[self.machine].iter().map_while(|l| l.spec()) {
            args.extend(["--serial".to_string(), spec.to_string()]);
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

    /// The items column, under a heading per mode: (text, attribute, row).
    fn item_lines(&self) -> Vec<(String, &'static str, Option<usize>)> {
        let e = self.entry();
        if e.items.is_empty() {
            return vec![("nothing found".to_string(), DIM, None)];
        }
        let mut lines = Vec::new();
        for (n, item) in e.items.iter().enumerate() {
            if n == 0 || e.items[n - 1].what != item.what {
                if n > 0 {
                    lines.push((String::new(), "", None));
                }
                lines.push((item.what.to_string(), DIM, None));
            }
            let radio = if n == self.item[self.machine] { "(*)" } else { "( )" };
            let file = match (&item.rom, &item.disk) {
                (Some(p), _) | (None, Some(p)) => p.display().to_string(),
                (None, None) => e.desc.default_rom.to_string(),
            };
            let default = if item.is_default { "  (default)" } else { "" };
            lines.push((format!("{radio} {file}{default}"), "", Some(n)));
        }
        lines
    }

    /// The options column, under a heading per kind: (text, attribute, row).
    fn option_lines(&self) -> Vec<(String, &'static str, Option<usize>)> {
        let e = self.entry();
        let token_w = e.desc.launch.windows.iter().map(|(t, _)| t.len()).max().unwrap_or(0);
        let mut lines = Vec::new();
        let mut heading = None;
        for (row, o) in self.options().into_iter().enumerate() {
            let (kind, text) = match o {
                Opt::Window(i) => {
                    let (token, what) = e.desc.launch.windows[i];
                    let check = if self.windows[self.machine][i] { "[x]" } else { "[ ]" };
                    ("windows", format!("{check} {token:<token_w$}  {what}"))
                }
                Opt::Serial(i) => {
                    let word = self.serial[self.machine][i].word();
                    ("serial ports (--serial)", format!("[{word:<6}] {}", e.desc.serial_ports[i]))
                }
                Opt::FastIo => ("speed", format!("[{}] --fast-io      devices finish i/o at once", check(self.fast_io))),
                Opt::FlatOut => ("speed", format!("[{}] --no-throttle  run flat out, not at the real clock", check(self.flat_out))),
            };
            if heading != Some(kind) {
                if heading.is_some() {
                    lines.push((String::new(), "", None));
                }
                lines.push((kind.to_string(), DIM, None));
                heading = Some(kind);
            }
            lines.push((text, "", Some(row)));
        }
        lines
    }

    /// The screen, `height` lines each of at most `width` columns, with
    /// ANSI attributes. `argv0` is what the command line preview runs.
    pub fn render(&self, argv0: &str, width: usize, height: usize) -> Vec<String> {
        let width = width.max(20);
        let height = height.max(8);
        let mut lines = Vec::with_capacity(height);
        lines.push(style(REVERSE, &fit(" emu launcher: pick a machine, what to load and how", width)));

        // the machines
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

        // to the right of them: the machine's name, then its items and its
        // options side by side, then what was looked for and not found
        let right_w = width.saturating_sub(left_w + 3);
        let e = self.entry();
        let clock = e.desc.clock_hz.map(|hz| format!(" at {}", crate::system::registry::fmt_hz(hz))).unwrap_or_default();
        let mut right: Vec<String> = vec![
            style(BOLD, &fit(e.desc.title, right_w)),
            style(DIM, &fit(&format!("cpu {}{clock}", e.desc.cpu), right_w)),
            String::new(),
        ];
        let items = self.item_lines();
        let options = self.option_lines();
        let mut missing: Vec<String> = Vec::new();
        if !e.missing.is_empty() {
            missing.push(String::new());
            missing.push(style(DIM, &fit("not here:", right_w)));
            missing.extend(e.missing.iter().map(|m| style(DIM, &fit(&format!("  {m}"), right_w))));
        }

        // two lines at the bottom: the command line and the keys
        let body = height - 1 - 1 - 2;
        let room = body.saturating_sub(right.len());
        let tall = items.len().max(options.len());
        let cols_h = tall.min(room.saturating_sub(missing.len()).max(room.min(5)));
        let inner = right_w.saturating_sub(3);
        let need = |c: &[(String, &str, Option<usize>)]| c.iter().map(|(t, _, _)| t.chars().count()).max().unwrap_or(0);
        let (items_need, opts_need) = (need(&items), need(&options));
        // the options lead with their token and lose only their description,
        // so the items have the room first
        let items_w = if items_need + opts_need <= inner { items_need } else { items_need.min(inner * 2 / 3).max(inner.saturating_sub(opts_need)) };
        let opts_w = inner.saturating_sub(items_w);
        let column = |c: &[(String, &'static str, Option<usize>)], w: usize, focus: Focus, cursor: usize| -> Vec<String> {
            // scrolled to keep the cursor in view
            let at = c.iter().position(|(_, _, r)| *r == Some(cursor)).unwrap_or(0);
            let top = if at >= cols_h { at + 1 - cols_h } else { 0 };
            (0..cols_h)
                .map(|i| match c.get(top + i) {
                    Some((text, attr, row)) => {
                        let on_cursor = self.focus == focus && *row == Some(cursor);
                        style(if on_cursor { REVERSE } else { attr }, &fit(text, w))
                    }
                    None => " ".repeat(w),
                })
                .collect()
        };
        let item_col = column(&items, items_w, Focus::Items, self.item[self.machine]);
        let opt_col = column(&options, opts_w, Focus::Options, self.opt);
        right.extend(item_col.into_iter().zip(opt_col).map(|(i, o)| format!("{i} | {o}")));
        right.extend(missing);

        lines.push(String::new());
        for i in 0..body {
            let l = left.get(i).cloned().unwrap_or_else(|| " ".repeat(left_w));
            let r = right.get(i).cloned().unwrap_or_default();
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
                        Focus::Machines => " up/down machine   right/tab what to load   enter choose   q quit",
                        Focus::Items => " up/down choose   right/tab options   left machines   enter run   q quit",
                        Focus::Options => " up/down move   space change   left back   enter run   esc machines   q quit",
                    },
                    width,
                ),
            ),
        };
        lines.push(help);
        lines
    }
}

fn check(on: bool) -> char {
    if on {
        'x'
    } else {
        ' '
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
    fn the_items_and_the_options_keep_their_own_cursors() {
        let mut p = picker();
        // rex.img, then over to the options: panel on, down past tty onto
        // the first serial port, and --fast-io at the end
        keys(&mut p, &[Key::Tab, Key::Down, Key::Tab, Key::Space, Key::End, Key::Up, Key::Space]);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "ray703-load-panel", "-r", "rex/build/rex.img", "--fast-io"]));
        // back to the items: the item moves, the options stay as they were
        keys(&mut p, &[Key::Left, Key::Up]);
        assert_eq!(p.focus, Focus::Items);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "ray703-panel", "-r", "roms/703/demo.bin", "--fast-io"]));
        // and Tab from the options goes round to the machines
        keys(&mut p, &[Key::Tab, Key::Tab]);
        assert_eq!(p.focus, Focus::Machines);
    }

    #[test]
    fn each_serial_port_is_offered_once_the_one_before_it_is_plugged_in() {
        let mut p = picker();
        keys(&mut p, &[Key::Tab, Key::Tab, Key::Down, Key::Down]);
        assert_eq!(p.options()[p.opt], Opt::Serial(0));
        assert!(!p.options().contains(&Opt::Serial(1)));
        // off, window, pty, tcp
        keys(&mut p, &[Key::Space, Key::Down, Key::Space, Key::Space, Key::Down, Key::Space, Key::Space, Key::Space]);
        assert_eq!(p.options().iter().filter(|o| matches!(o, Opt::Serial(_))).count(), 3);
        assert_eq!(
            p.key(Key::Enter),
            launch(&["-s", "ray703", "-r", "roms/703/demo.bin", "--serial", "window", "--serial", "pty", "--serial", "tcp:0"])
        );
        let screen = p.render("emu", 120, 30).join("\n");
        assert!(screen.contains("[window] teletype 1, multiplexer channel 0"), "{screen}");
        // the first unplugged again: the rest are kept but not passed, since
        // --serial fills the ports in turn
        keys(&mut p, &[Key::Up, Key::Up, Key::Space, Key::Space, Key::Space]);
        assert_eq!(p.serial[p.machine][0], Line::Off);
        assert_eq!(p.options().iter().filter(|o| matches!(o, Opt::Serial(_))).count(), 1);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "ray703", "-r", "roms/703/demo.bin"]));
    }

    #[test]
    fn a_port_with_no_window_goes_from_off_to_pty() {
        let mut p = picker();
        // the kaypro: no window tokens, so its serial port heads the options
        keys(&mut p, &[Key::Down, Key::Tab, Key::Tab, Key::Space]);
        assert_eq!(p.serial[p.machine][0], Line::Pty);
        assert_eq!(p.key(Key::Enter), launch(&["-s", "kaypro", "--disk", "disks/a b.img", "--serial", "pty"]));
    }

    #[test]
    fn a_machine_with_nothing_to_run_says_so_instead_of_launching() {
        let mut p = picker();
        keys(&mut p, &[Key::Up]);
        assert_eq!(p.key(Key::Enter), Outcome::Continue);
        let screen = p.render("emu", 80, 24).join("\n");
        assert!(screen.contains("nothing to run System09 on yet"), "{screen}");
        // and its items column is passed over on the way to its options
        keys(&mut p, &[Key::Right]);
        assert_eq!(p.focus, Focus::Options);
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
        keys(&mut p, &[Key::Right, Key::Right, Key::Esc]);
        assert_eq!(p.focus, Focus::Machines);
        assert_eq!(p.key(Key::Esc), Outcome::Quit);
        assert_eq!(picker().key(Key::Quit), Outcome::Quit);
    }

    fn visible(l: &str) -> String {
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
    }

    #[test]
    fn the_screen_fits_the_terminal_and_scrolls_to_the_cursor() {
        let mut p = picker();
        keys(&mut p, &[Key::Tab, Key::Tab, Key::End]);
        for (w, h) in [(60, 10), (80, 24), (200, 50)] {
            let screen = p.render("emu", w, h);
            assert_eq!(screen.len(), h);
            for l in &screen {
                assert!(visible(l).chars().count() <= w, "{w}x{h}: {l:?}");
            }
            assert!(screen.iter().any(|l| l.contains("--no-thro") && l.contains(REVERSE)), "{w}x{h}: {screen:#?}");
        }
    }

    #[test]
    fn the_items_and_the_options_are_side_by_side() {
        let mut p = picker();
        keys(&mut p, &[Key::Tab]);
        let screen: Vec<String> = p.render("emu", 160, 24).iter().map(|l| visible(l)).collect();
        let row = screen.iter().find(|l| l.contains("demo.bin")).expect("the default item");
        assert!(row.contains("(*)") && row.contains("| [ ] tty"), "{screen:#?}");
    }

    #[test]
    fn quoting_leaves_plain_words_alone() {
        assert_eq!(quote("roms/703/demo.bin"), "roms/703/demo.bin");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }
}
