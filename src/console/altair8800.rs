// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Altair 8800's frontend: the front panel window, with the console
//! left on the terminal -- the raw-mode [`TerminalFrontend`] on a second
//! thread feeding the same keystroke channel, as the 703's panel-only
//! frontend runs it. Closing the window or Ctrl-D into it shuts the
//! machine down, as Ctrl-D at the terminal does.

use super::{ConsoleFrontend, Display};
use crate::console::panel8800::PanelWindow8800;
use crate::console::terminal::TerminalFrontend;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Roughly 60 Hz, redrawing every frame: the lamps move every cycle.
const FRAME_DELAY: Duration = Duration::from_millis(16);

pub struct Altair8800Frontend {
    tx: Sender<u8>,
    sdl: sdl2::Sdl,
    panel: PanelWindow8800,
}

impl Altair8800Frontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let Display::Altair8800 { title, panel } = display else {
            return Err("Altair8800Frontend needs an Altair 8800 display".to_string());
        };
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let panel = PanelWindow8800::new(&video, title, panel)?;
        Ok(Altair8800Frontend { tx, sdl, panel })
    }

    fn event_loop(&mut self, shutdown: &Arc<AtomicBool>) {
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Altair8800Frontend: failed to create event pump: {e}");
                return;
            }
        };
        println!("Altair8800Frontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("Altair8800Frontend: stop requested, exiting");
                return;
            }
            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } | Event::Window { win_event: WindowEvent::Close, .. } => {
                        println!("Altair8800Frontend: window closed, exiting");
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

impl ConsoleFrontend for Altair8800Frontend {
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
