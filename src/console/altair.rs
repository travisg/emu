// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Altairs' frontend: the front panel window -- the 8800's or the
//! 680b's -- with the console left on the terminal, the raw-mode
//! [`TerminalFrontend`] on a second thread feeding the same keystroke
//! channel, as the 703's panel-only frontend runs it. Closing the window
//! or Ctrl-D into it shuts the machine down, as Ctrl-D at the terminal
//! does.

use super::{ConsoleFrontend, Display};
use crate::console::panel680::PanelWindow680;
use crate::console::panel8800::PanelWindow8800;
use crate::console::terminal::TerminalFrontend;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Roughly 60 Hz, redrawing every frame: the lamps move every cycle.
const FRAME_DELAY: Duration = Duration::from_millis(16);

/// Whichever Altair's panel this is.
enum PanelWindow {
    Altair8800(PanelWindow8800),
    Altair680(PanelWindow680),
}

impl PanelWindow {
    fn window_id(&self) -> u32 {
        match self {
            PanelWindow::Altair8800(w) => w.window_id(),
            PanelWindow::Altair680(w) => w.window_id(),
        }
    }

    fn render(&mut self) {
        match self {
            PanelWindow::Altair8800(w) => w.render(),
            PanelWindow::Altair680(w) => w.render(),
        }
    }

    fn click(&mut self, x: i32, y: i32, button: MouseButton) {
        match self {
            PanelWindow::Altair8800(w) => w.click(x, y, button),
            PanelWindow::Altair680(w) => w.click(x, y, button),
        }
    }
}

pub struct AltairFrontend {
    tx: Sender<u8>,
    sdl: sdl2::Sdl,
    panel: PanelWindow,
}

impl AltairFrontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let panel = match display {
            Display::Altair8800 { title, panel } => PanelWindow::Altair8800(PanelWindow8800::new(&video, title, panel)?),
            Display::Altair680 { title, panel } => PanelWindow::Altair680(PanelWindow680::new(&video, title, panel)?),
            _ => return Err("AltairFrontend needs an Altair display".to_string()),
        };
        Ok(AltairFrontend { tx, sdl, panel })
    }

    fn event_loop(&mut self, shutdown: &Arc<AtomicBool>) {
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("AltairFrontend: failed to create event pump: {e}");
                return;
            }
        };
        println!("AltairFrontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("AltairFrontend: stop requested, exiting");
                return;
            }
            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } | Event::Window { win_event: WindowEvent::Close, .. } => {
                        println!("AltairFrontend: window closed, exiting");
                        return;
                    }
                    Event::KeyDown { keycode: Some(Keycode::D), keymod, .. }
                        if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) =>
                    {
                        println!("ctrl-d hit, exiting");
                        return;
                    }
                    Event::MouseButtonDown { window_id, x, y, mouse_btn, .. } if window_id == self.panel.window_id() => {
                        self.panel.click(x, y, mouse_btn);
                    }
                    _ => {}
                }
            }
            self.panel.render();
            std::thread::sleep(FRAME_DELAY);
        }
    }
}

impl ConsoleFrontend for AltairFrontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        let mut terminal = TerminalFrontend::new(self.tx.clone());
        let pump_shutdown = Arc::clone(&shutdown);
        let pump = std::thread::spawn(move || {
            terminal.run(pump_shutdown);
        });

        self.event_loop(&shutdown);

        // join the terminal first, so termios is restored before main
        // prints its exit messages
        shutdown.store(true, Ordering::SeqCst);
        let _ = pump.join();
    }
}
