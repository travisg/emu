// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The frontend for a machine with windows of its own -- a front panel
//! (the 703's, the 8800's or the 680b's), teletypes, or both -- behind one
//! SDL context.
//!
//! SDL allows one context and one event pump per process, so the windows
//! cannot each be a frontend the way the Kaypro's screen is: this frontend
//! owns the context, builds each window the [`Display`] carries, and
//! routes events to them by the `window_id` SDL stamps on every input
//! event. Closing any window -- or Ctrl-D into one -- shuts the whole
//! machine down, as closing the Kaypro's one window does.
//!
//! Without a window for the console's teletype it stays on the terminal:
//! the ordinary raw-mode [`TerminalFrontend`] runs on a second thread,
//! feeding the same keystroke channel. With one, the window is the
//! teletype and the terminal thread runs in idle mode -- raw, silent, only
//! Ctrl-D/EOF still acted on -- so the documented exit path holds whichever
//! windows are up. The 703's other teletypes' windows have keyboards of
//! their own and change nothing here.

use super::{ConsoleFrontend, Display};
use crate::console::panel680::PanelWindow680;
use crate::console::panel703::PanelWindow as PanelWindow703;
use crate::console::panel8800::PanelWindow8800;
use crate::console::terminal::TerminalFrontend;
use crate::console::tty33::TtyWindow;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Roughly 60 Hz. A panel has no dirty flag: its lamps change on every
/// instruction, so it redraws every frame unconditionally.
const FRAME_DELAY: Duration = Duration::from_millis(16);

/// Whichever machine's front panel this is.
enum PanelWindow {
    /// Boxed: much the largest, with its rack and its lamp filters.
    Ray703(Box<PanelWindow703>),
    Altair8800(PanelWindow8800),
    Altair680(PanelWindow680),
}

impl PanelWindow {
    fn window_id(&self) -> u32 {
        match self {
            PanelWindow::Ray703(w) => w.window_id(),
            PanelWindow::Altair8800(w) => w.window_id(),
            PanelWindow::Altair680(w) => w.window_id(),
        }
    }

    fn render(&mut self) {
        match self {
            PanelWindow::Ray703(w) => w.render(),
            PanelWindow::Altair8800(w) => w.render(),
            PanelWindow::Altair680(w) => w.render(),
        }
    }

    fn click(&mut self, x: i32, y: i32, button: MouseButton) {
        match self {
            PanelWindow::Ray703(w) => w.click(x, y, button),
            PanelWindow::Altair8800(w) => w.click(x, y, button),
            PanelWindow::Altair680(w) => w.click(x, y, button),
        }
    }

    /// Only the 703's panel takes keys: its knob and SENSE toggles. The
    /// Altairs' are mouse-only.
    fn key_down(&mut self, key: Keycode) {
        if let PanelWindow::Ray703(w) = self {
            w.key_down(key);
        }
    }
}

pub struct WindowsFrontend {
    tx: Sender<u8>,
    sdl: sdl2::Sdl,
    panel: Option<PanelWindow>,
    /// Every teletype window, the console's first if it has one.
    ttys: Vec<TtyWindow>,
    /// The console's teletype is one of them, so the terminal is not.
    console_tty: bool,
}

impl WindowsFrontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let (title, panel, tty, terminals) = match display {
            Display::Ray703 { title, panel, tty, terminals } => {
                let panel = panel.map(|p| PanelWindow703::new(&video, title, p).map(|w| PanelWindow::Ray703(Box::new(w)))).transpose()?;
                (title, panel, tty, terminals)
            }
            Display::Altair8800 { title, panel, tty } => {
                let panel =
                    panel.map(|p| PanelWindow8800::new(&video, title, p).map(PanelWindow::Altair8800)).transpose()?;
                (title, panel, tty, Vec::new())
            }
            Display::Altair680 { title, panel, tty } => {
                let panel =
                    panel.map(|p| PanelWindow680::new(&video, title, p).map(PanelWindow::Altair680)).transpose()?;
                (title, panel, tty, Vec::new())
            }
            Display::CharCell { .. } => return Err("WindowsFrontend takes no character screen".to_string()),
        };
        let console_tty = tty.is_some();
        let ttys = tty
            .into_iter()
            .chain(terminals)
            .map(|t| TtyWindow::new(&video, title, t, tx.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        if !ttys.is_empty() {
            // printable keystrokes for a teletype arrive as text input; a
            // panel alone keeps it off
            video.text_input().start();
        }
        Ok(WindowsFrontend { tx, sdl, panel, ttys, console_tty })
    }

    fn event_loop(&mut self, shutdown: &Arc<AtomicBool>) {
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("WindowsFrontend: failed to create event pump: {e}");
                return;
            }
        };

        // A teletype's font texture borrows its window's creator, so both
        // live in this scope and the texture is passed into each render --
        // the same shape as SdlFrontend's atlas. One of each to a window:
        // a texture belongs to the renderer that made it.
        let creators: Vec<_> = self.ttys.iter().map(|t| t.texture_creator()).collect();
        let fonts = match self.ttys.iter().zip(&creators).map(|(t, c)| t.build_font(c)).collect::<Result<Vec<_>, _>>() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("WindowsFrontend: failed to build the teletype font: {e}");
                return;
            }
        };

        println!("WindowsFrontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("WindowsFrontend: stop requested, exiting");
                return;
            }

            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        println!("WindowsFrontend: quit event received");
                        return;
                    }
                    // With more than one window SDL sends Quit only when
                    // the *last* closes; any window closing shuts the
                    // machine down here, so listen per-window too.
                    Event::Window { win_event: WindowEvent::Close, .. } => {
                        println!("WindowsFrontend: window closed, exiting");
                        return;
                    }
                    Event::KeyDown { keycode: Some(Keycode::D), keymod, .. }
                        if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) =>
                    {
                        // whichever window it landed in
                        println!("ctrl-d hit, exiting");
                        return;
                    }
                    Event::MouseButtonDown { window_id, x, y, mouse_btn, .. } => {
                        if let Some(p) = self.panel.as_mut().filter(|p| p.window_id() == window_id) {
                            p.click(x, y, mouse_btn);
                        }
                    }
                    Event::MouseWheel { window_id, y, .. } => {
                        if let Some(t) = self.ttys.iter_mut().find(|t| t.window_id() == window_id) {
                            t.wheel(y);
                        }
                    }
                    Event::TextInput { window_id, text, .. } => {
                        if let Some(t) = self.ttys.iter_mut().find(|t| t.window_id() == window_id) {
                            t.text_input(&text);
                        }
                    }
                    Event::KeyDown { window_id, keycode: Some(key), keymod, .. } => {
                        if let Some(p) = self.panel.as_mut().filter(|p| p.window_id() == window_id) {
                            p.key_down(key);
                        } else if let Some(t) = self.ttys.iter_mut().find(|t| t.window_id() == window_id) {
                            t.key_down(key, keymod);
                        }
                    }
                    _ => {}
                }
            }

            // the panel redraws every frame (its lamps move every
            // instruction); the paper only when it changed
            if let Some(p) = &mut self.panel {
                p.render();
            }
            for (t, f) in self.ttys.iter_mut().zip(&fonts) {
                t.render_if_needed(f);
            }
            std::thread::sleep(FRAME_DELAY);
        }
    }
}

impl ConsoleFrontend for WindowsFrontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        // The terminal runs on its own thread either way: feeding the
        // keystroke channel when it is the console, or in idle mode --
        // still raw, still the Ctrl-D exit -- when a window is. Its 100 ms
        // poll notices the shutdown flag, and its RawMode guard restores
        // termios when its run() returns.
        let mut terminal = if self.console_tty {
            TerminalFrontend::new_idle()
        } else {
            TerminalFrontend::new(self.tx.clone())
        };
        let pump_shutdown = Arc::clone(&shutdown);
        let pump = std::thread::spawn(move || {
            terminal.run(pump_shutdown);
        });

        self.event_loop(&shutdown);

        // Join the pump before returning so the terminal is restored before
        // main prints its exit messages -- whichever side quit first.
        shutdown.store(true, Ordering::SeqCst);
        let _ = pump.join();
    }
}
