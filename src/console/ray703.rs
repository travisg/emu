// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 703's frontend: however many windows the subsystem tokens asked
//! for, behind one SDL context.
//!
//! SDL allows one context and one event pump per process, so the front
//! panel and any future windows cannot each be a frontend the way the
//! Kaypro's screen is: this frontend owns the context, builds each window
//! the [`Display::Ray703`] variant carries, and routes events to them by
//! the `window_id` SDL stamps on every input event. Closing any window --
//! or Ctrl-D into one -- shuts the whole machine down, as closing the
//! Kaypro's one window does.
//!
//! Without a teletype window the teletype stays on the terminal: the
//! ordinary raw-mode [`TerminalFrontend`] runs on a second thread, feeding
//! the same keystroke channel, exactly as when the panel was its own
//! frontend. With one, the window is the teletype and the terminal thread
//! runs in idle mode -- raw, silent, only Ctrl-D/EOF still acted on -- so
//! the documented exit path holds whichever windows are up.

use super::{ConsoleFrontend, Display};
use crate::console::panel703::PanelWindow;
use crate::console::terminal::TerminalFrontend;
use crate::console::tty703::TtyWindow;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Roughly 60 Hz. The panel has no dirty flag: the program counter lamps
/// change on every instruction, so it redraws every frame unconditionally.
const FRAME_DELAY: Duration = Duration::from_millis(16);

pub struct Ray703Frontend {
    tx: Sender<u8>,
    sdl: sdl2::Sdl,
    panel: Option<PanelWindow>,
    tty: Option<TtyWindow>,
}

impl Ray703Frontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let Display::Ray703 { title, panel, tty } = display else {
            return Err("Ray703Frontend needs a 703 display".to_string());
        };
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let panel = panel.map(|p| PanelWindow::new(&video, title, p)).transpose()?;
        let tty = tty.map(|t| TtyWindow::new(&video, title, t, tx.clone())).transpose()?;
        if tty.is_some() {
            // printable keystrokes for the teletype arrive as text input;
            // the panel-only frontend keeps it off, as it always has
            video.text_input().start();
        }
        Ok(Ray703Frontend { tx, sdl, panel, tty })
    }

    fn event_loop(&mut self, shutdown: &Arc<AtomicBool>) {
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Ray703Frontend: failed to create event pump: {e}");
                return;
            }
        };

        // The teletype's font texture borrows its creator, so both live in
        // this scope and the texture is passed into each render -- the same
        // shape as SdlFrontend's atlas.
        let tty_creator = self.tty.as_ref().map(|t| t.texture_creator());
        let tty_font = match (&self.tty, &tty_creator) {
            (Some(t), Some(c)) => match t.build_font(c) {
                Ok(f) => Some(f),
                Err(e) => {
                    eprintln!("Ray703Frontend: failed to build the teletype font: {e}");
                    return;
                }
            },
            _ => None,
        };

        println!("Ray703Frontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("Ray703Frontend: stop requested, exiting");
                return;
            }

            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        println!("Ray703Frontend: quit event received");
                        return;
                    }
                    // With more than one window SDL sends Quit only when
                    // the *last* closes; any window closing shuts the
                    // machine down here, so listen per-window too.
                    Event::Window { win_event: WindowEvent::Close, .. } => {
                        println!("Ray703Frontend: window closed, exiting");
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
                        if let Some(p) = self.panel.as_mut().filter(|p| p.window_id() == window_id)
                        {
                            p.click(x, y, mouse_btn);
                        }
                    }
                    Event::MouseWheel { window_id, y, .. } => {
                        if let Some(t) = self.tty.as_mut().filter(|t| t.window_id() == window_id) {
                            t.wheel(y);
                        }
                    }
                    Event::TextInput { window_id, text, .. } => {
                        if let Some(t) = self.tty.as_mut().filter(|t| t.window_id() == window_id) {
                            t.text_input(&text);
                        }
                    }
                    Event::KeyDown { window_id, keycode: Some(key), keymod, .. } => {
                        if let Some(p) = self.panel.as_mut().filter(|p| p.window_id() == window_id)
                        {
                            p.key_down(key);
                        } else if let Some(t) =
                            self.tty.as_mut().filter(|t| t.window_id() == window_id)
                        {
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
            if let (Some(t), Some(f)) = (&mut self.tty, &tty_font) {
                t.render_if_needed(f);
            }
            std::thread::sleep(FRAME_DELAY);
        }
    }
}

impl ConsoleFrontend for Ray703Frontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        // The terminal runs on its own thread either way: feeding the
        // keystroke channel when it is the teletype, or in idle mode --
        // still raw, still the Ctrl-D exit -- when the window is. Its
        // 100 ms poll notices the shutdown flag, and its RawMode guard
        // restores termios when its run() returns.
        let mut terminal = if self.tty.is_some() {
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
