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
//! The Raytheon 703's front panel, drawn after figure 5-1 of the reference
//! manual (section 5, "Controls and Indicators").
//!
//! Two rows of round lamp-indicators -- the PROGRAM COUNTER row, always
//! live, and the SELECTED DISPLAY row behind the six-position DISPLAY
//! SELECTOR rotary -- plus the four SENSE toggles. The lamps sample the
//! [`PanelState`] the core publishes after every step; the selector knob is
//! state of this frontend alone, which is what lets it be "changed while
//! the program is running" (5-2). The rest of the real panel (RUN, HALT,
//! RESET, SINGLE STEP/COMMAND, the CLEAR and ENTER data-entry path) is not
//! here yet.
//!
//! The teletype stays on the terminal: this frontend spawns the ordinary
//! [`TerminalFrontend`] on a second thread to pump stdin, and guest output
//! goes to stdout as on every other machine. Keystrokes into the panel
//! window are deliberately *not* forwarded to the guest -- the real panel
//! has no keyboard, and a second keyboard would ride SDL's text-input
//! CR/LF conventions rather than the terminal's raw mode, whose preserved
//! CR-versus-LF distinction the 703's software depends on.
//!
//! Everything is drawn with filled rectangles -- circles as stacks of
//! horizontal spans, text from a 5x7 font embedded below -- so there are no
//! textures, image files or font dependencies.

use super::{ConsoleFrontend, Display, PanelState, Selector};
use crate::console::terminal::TerminalFrontend;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

const WINDOW_W: u32 = 1120;
const WINDOW_H: u32 = 340;
/// Roughly 60 Hz. No dirty flag: the program counter lamps change on
/// every instruction, so the panel redraws every frame unconditionally.
const FRAME_DELAY: Duration = Duration::from_millis(16);

// -- layout, all in window pixels ------------------------------------------

/// Lamp centers: indicator `i` at `LAMPS_X + i * LAMP_PITCH`, plus a gap
/// after indicators 3, 7 and 11 -- figure 5-1 groups the lamps in fours,
/// which is also what makes them readable as hex digits.
const LAMPS_X: i32 = 64;
const LAMP_PITCH: i32 = 54;
const GROUP_GAP: i32 = 18;
const LAMP_R: i32 = 13;

const PC_CAPTION_Y: i32 = 30;
const PC_LAMPS_Y: i32 = 78;
const SD_CAPTION_Y: i32 = 118;
const SD_LAMPS_Y: i32 = 166;
const CONTROLS_Y: i32 = 252;

/// The DISPLAY SELECTOR knob.
const KNOB_X: i32 = 1020;
const KNOB_Y: i32 = 130;
const KNOB_R: i32 = 32;

/// SENSE toggles sit under lamps 8-11, as in figure 5-1.
const TOGGLE_W: u32 = 18;
const TOGGLE_H: u32 = 36;

const PANEL_BG: Color = Color::RGB(0xd6, 0xd2, 0xc9);
const PANEL_INK: Color = Color::RGB(0x22, 0x22, 0x22);
const LAMP_LIT: Color = Color::RGB(0xff, 0xb4, 0x3c);
const LAMP_UNLIT: Color = Color::RGB(0x54, 0x42, 0x2a);
const LAMP_BEZEL: Color = Color::RGB(0x3a, 0x3a, 0x3a);

/// The selector's positions in [`SELECTOR_ORDER`] order, spread over the top
/// arc of the knob like the rotary in figure 5-1.
const SELECTOR_ORDER: [Selector; 6] =
    [Selector::Ms, Selector::In, Selector::Ma, Selector::Mb, Selector::Ix, Selector::Ac];
const SELECTOR_NAMES: [&str; 6] = ["MS", "IN", "MA", "MB", "IX", "AC"];

fn lamp_x(i: i32) -> i32 {
    LAMPS_X + i * LAMP_PITCH + (i / 4) * GROUP_GAP
}

fn toggle_x(i: i32) -> i32 {
    lamp_x(8 + i)
}

/// Angle of selector position `i` in radians: 210 degrees around to -30,
/// counterclockwise-positive with 0 at three o'clock, so the six positions
/// sweep over the top of the knob.
fn selector_angle(i: usize) -> f32 {
    (210.0 - 48.0 * i as f32).to_radians()
}

pub struct Panel703Frontend {
    tx: Sender<u8>,
    panel: PanelState,
    selector: Selector,
    sdl: sdl2::Sdl,
    canvas: Canvas<Window>,
}

impl Panel703Frontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let Display::Panel703 { title, panel } = display else {
            return Err("Panel703Frontend needs a 703 panel display".to_string());
        };
        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        let window = video_subsystem
            .window(title, WINDOW_W, WINDOW_H)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;
        let canvas = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
        // MB is the boot-procedure position ("turn the display selector to
        // MB before following the operating procedure" -- the PTB drawing),
        // and it is also the busiest lamp row on an idle machine.
        Ok(Panel703Frontend { tx, panel, selector: Selector::Mb, sdl, canvas })
    }

    fn cycle_selector(&mut self, dir: i32) {
        let i = SELECTOR_ORDER.iter().position(|&s| s == self.selector).unwrap() as i32;
        self.selector = SELECTOR_ORDER[(i + dir).rem_euclid(6) as usize];
    }

    // -- drawing -----------------------------------------------------------

    /// A filled circle as one fill_rect span per scanline.
    fn circle(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        self.canvas.set_draw_color(color);
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt() as i32;
            let _ = self.canvas.fill_rect(Rect::new(cx - half, cy + dy, (2 * half + 1) as u32, 1));
        }
    }

    /// 5x7 text, scaled up `scale` times. Unknown characters draw as space.
    fn text(&mut self, x: i32, y: i32, scale: u32, s: &str, color: Color) {
        self.canvas.set_draw_color(color);
        let mut cx = x;
        for c in s.chars() {
            if let Some(glyph) = glyph(c) {
                for (row, bits) in glyph.iter().enumerate() {
                    for col in 0..5 {
                        if bits & (0x10 >> col) != 0 {
                            let _ = self.canvas.fill_rect(Rect::new(
                                cx + col * scale as i32,
                                y + row as i32 * scale as i32,
                                scale,
                                scale,
                            ));
                        }
                    }
                }
            }
            cx += 6 * scale as i32;
        }
    }

    /// Text centered on `cx`.
    fn text_centered(&mut self, cx: i32, y: i32, scale: u32, s: &str, color: Color) {
        let w = (s.chars().count() as i32 * 6 - 1) * scale as i32;
        self.text(cx - w / 2, y, scale, s, color);
    }

    /// A caption between two rules, like the row headings in figure 5-1.
    fn caption(&mut self, y: i32, label: &str) {
        let w = (label.chars().count() as i32 * 6 - 1) * 2;
        let (left, right) = (lamp_x(0) - LAMP_R, lamp_x(15) + LAMP_R);
        let cx = (left + right) / 2;
        self.canvas.set_draw_color(PANEL_INK);
        let _ = self.canvas.fill_rect(Rect::new(left, y + 6, (cx - w / 2 - 8 - left) as u32, 1));
        let _ =
            self.canvas.fill_rect(Rect::new(cx + w / 2 + 8, y + 6, (right - cx - w / 2 - 8) as u32, 1));
        self.text_centered(cx, y, 2, label, PANEL_INK);
    }

    /// One row of lamps lighting `value`, most significant bit at indicator
    /// 0 as everywhere on this machine. `first` skips leading indicators:
    /// the PROGRAM COUNTER row starts at 1 because the PCR is 15 bits.
    fn lamp_row(&mut self, y: i32, value: u16, first: i32) {
        for i in first..16 {
            let x = lamp_x(i);
            let lit = value >> (15 - i) & 1 != 0;
            self.circle(x, y, LAMP_R + 3, LAMP_BEZEL);
            self.circle(x, y, LAMP_R, if lit { LAMP_LIT } else { LAMP_UNLIT });
            let label = i.to_string();
            self.text_centered(x, y - LAMP_R - 15, 1, &label, PANEL_INK);
        }
    }

    fn draw_toggles(&mut self) {
        self.caption(CONTROLS_Y - 34, "SENSE");
        for i in 0..4 {
            let x = toggle_x(i);
            let up = self.panel.sense(i as u8);
            self.canvas.set_draw_color(LAMP_BEZEL);
            let _ = self.canvas.fill_rect(Rect::new(
                x - TOGGLE_W as i32 / 2,
                CONTROLS_Y - TOGGLE_H as i32 / 2,
                TOGGLE_W,
                TOGGLE_H,
            ));
            let ky = if up { CONTROLS_Y - TOGGLE_H as i32 / 2 } else { CONTROLS_Y + TOGGLE_H as i32 / 2 };
            self.circle(x, ky, 10, PANEL_INK);
            self.circle(x, ky, 8, Color::RGB(0x90, 0x90, 0x90));
            self.text_centered(x, CONTROLS_Y - TOGGLE_H as i32 / 2 - 24, 1, &i.to_string(), PANEL_INK);
        }
    }

    fn draw_selector(&mut self) {
        self.text_centered(KNOB_X, KNOB_Y - KNOB_R - 42, 1, "DISPLAY", PANEL_INK);
        self.text_centered(KNOB_X, KNOB_Y - KNOB_R - 32, 1, "SELECTOR", PANEL_INK);
        self.circle(KNOB_X, KNOB_Y, KNOB_R + 3, LAMP_BEZEL);
        self.circle(KNOB_X, KNOB_Y, KNOB_R, Color::RGB(0x30, 0x30, 0x30));
        for (i, name) in SELECTOR_NAMES.iter().enumerate() {
            let a = selector_angle(i);
            let lx = KNOB_X + ((KNOB_R + 18) as f32 * a.cos()) as i32;
            let ly = KNOB_Y - ((KNOB_R + 18) as f32 * a.sin()) as i32;
            self.text_centered(lx, ly - 3, 1, name, PANEL_INK);
        }
        // the pointer: a pale dot on the knob's rim at the active position
        let i = SELECTOR_ORDER.iter().position(|&s| s == self.selector).unwrap();
        let a = selector_angle(i);
        let px = KNOB_X + ((KNOB_R - 8) as f32 * a.cos()) as i32;
        let py = KNOB_Y - ((KNOB_R - 8) as f32 * a.sin()) as i32;
        self.circle(px, py, 4, Color::RGB(0xe8, 0xe8, 0xe8));
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(PANEL_BG);
        self.canvas.clear();

        self.caption(PC_CAPTION_Y, "PROGRAM COUNTER");
        let pcr = self.panel.program_counter();
        self.lamp_row(PC_LAMPS_Y, pcr, 1);

        self.caption(SD_CAPTION_Y, "SELECTED DISPLAY");
        let sd = self.panel.selected(self.selector);
        self.lamp_row(SD_LAMPS_Y, sd, 0);

        self.draw_toggles();
        self.draw_selector();

        self.text(LAMPS_X - LAMP_R, WINDOW_H as i32 - 34, 2, "703  CENTRAL PROCESSOR", PANEL_INK);
        self.text_centered(
            KNOB_X,
            WINDOW_H as i32 - 34,
            2,
            "RAYTHEON",
            Color::RGB(0x8a, 0x1f, 0x1f),
        );

        self.canvas.present();
    }

    // -- input -------------------------------------------------------------

    fn click(&mut self, x: i32, y: i32, button: MouseButton) {
        // a SENSE toggle flips
        for i in 0..4 {
            let dx = x - toggle_x(i);
            let dy = y - CONTROLS_Y;
            if dx.abs() <= 16 && dy.abs() <= TOGGLE_H as i32 / 2 + 12 {
                self.panel.toggle_sense(i as u8);
                return;
            }
        }
        // a selector label jumps straight there
        for (i, _) in SELECTOR_NAMES.iter().enumerate() {
            let a = selector_angle(i);
            let lx = KNOB_X + ((KNOB_R + 18) as f32 * a.cos()) as i32;
            let ly = KNOB_Y - ((KNOB_R + 18) as f32 * a.sin()) as i32;
            if (x - lx).abs() <= 12 && (y - ly).abs() <= 8 {
                self.selector = SELECTOR_ORDER[i];
                return;
            }
        }
        // the knob itself cycles: left click clockwise, right back
        let (dx, dy) = (x - KNOB_X, y - KNOB_Y);
        if dx * dx + dy * dy <= (KNOB_R + 6) * (KNOB_R + 6) {
            self.cycle_selector(if button == MouseButton::Right { -1 } else { 1 });
        }
    }

    fn event_loop(&mut self, shutdown: &Arc<AtomicBool>) {
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Panel703Frontend: failed to create event pump: {e}");
                return;
            }
        };

        println!("Panel703Frontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("Panel703Frontend: stop requested, exiting");
                return;
            }

            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        println!("Panel703Frontend: quit event received");
                        return;
                    }
                    Event::MouseButtonDown { x, y, mouse_btn, .. } => {
                        self.click(x, y, mouse_btn);
                    }
                    Event::KeyDown { keycode: Some(key), keymod, .. } => match key {
                        Keycode::D if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                            println!("ctrl-d hit on the panel, exiting");
                            return;
                        }
                        Keycode::Tab => self.cycle_selector(1),
                        Keycode::Num1 => self.panel.toggle_sense(0),
                        Keycode::Num2 => self.panel.toggle_sense(1),
                        Keycode::Num3 => self.panel.toggle_sense(2),
                        Keycode::Num4 => self.panel.toggle_sense(3),
                        // everything else is deliberately ignored: the guest's
                        // keyboard is the terminal, not this window
                        _ => {}
                    },
                    _ => {}
                }
            }

            self.render();
            std::thread::sleep(FRAME_DELAY);
        }
    }
}

impl ConsoleFrontend for Panel703Frontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        // The teletype keeps the terminal: the ordinary raw-mode frontend
        // runs on its own thread, feeding the same keystroke channel. Its
        // 100 ms poll notices the shutdown flag, and its RawMode guard
        // restores termios when its run() returns.
        let pump_tx = self.tx.clone();
        let pump_shutdown = Arc::clone(&shutdown);
        let pump = std::thread::spawn(move || {
            TerminalFrontend::new(pump_tx).run(pump_shutdown);
        });

        self.event_loop(&shutdown);

        // Join the pump before returning so the terminal is restored before
        // main prints its exit messages -- whichever side quit first.
        shutdown.store(true, Ordering::SeqCst);
        let _ = pump.join();
    }
}

/// A 5x7 font covering the panel's labels: digits, upper-case letters, the
/// dash. Each row is one byte, bit 4 the leftmost pixel.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '-' => [0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00],
        _ => return None,
    })
}
