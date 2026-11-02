// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The MITS Altair 8800's front panel, after the machine's own: the status
//! lamps INTE through INT and the data lamps D7-D0 across the top, WAIT
//! and HLDA and the address lamps A15-A0 below them, the sixteen address
//! and data toggles under those -- A15-A8 doubling as the sense switches
//! -- and the row of momentary command switches: STOP/RUN, SINGLE STEP,
//! EXAMINE/EXAMINE NEXT, DEPOSIT/DEPOSIT NEXT, RESET/CLR, PROTECT/
//! UNPROTECT and the two AUX. The address toggles and lamps are grouped
//! in threes from the right, as the panel's octal markings group them.
//!
//! The lamps are LEDs, which have no filament to heat: each shows the
//! fraction of machine cycles of the last frame its line was high, with
//! no lag, through the eye's square-root response to light. Stopped, no
//! cycles pass, and each row shows what the 8080 holds in the wait state
//! of its next M1 cycle -- the address of the next instruction, its byte,
//! and MEMR, M1 and WO -- with WAIT lit, as STOP leaves a real one.
//!
//! A momentary switch is pressed up or down by clicking above or below
//! its pivot. SINGLE STEP steps one instruction, not the real switch's one
//! machine cycle, which is the 703 panel's divergence too. RESET is the
//! 8080's reset line and leaves STOP/RUN as it was. CLR, PROTECT,
//! UNPROTECT and AUX have nothing here to act on: no board takes the
//! bus's external clear, the RAM has no protect circuit (PROT stays
//! dark), and the AUX switches were wired to nothing at the factory.
//! Keystrokes into this window go nowhere -- the console is the terminal.

use super::lamps::{self, duties};
use super::{
    AltairPanel, AltairPanelDisplay, LampSnapshot, PanelCommand, PanelControl, ALTAIR_ADDRESS, ALTAIR_DATA,
    ALTAIR_STATUS,
};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

const WINDOW_W: u32 = 1140;
const WINDOW_H: u32 = 430;

const COL_PITCH: i32 = 52;
const GROUP_GAP: i32 = 14;
const COLS_X: i32 = 200;
const LED_R: i32 = 9;
const TITLE_Y: i32 = 22;
const STATUS_Y: i32 = 96;
const ADDRESS_Y: i32 = 176;
const TOGGLE_Y: i32 = 262;
const COMMAND_Y: i32 = 362;
const TOGGLE_W: u32 = 10;
const TOGGLE_LEVER: i32 = 22;
/// Frames a momentary switch stays drawn pressed after a click.
const PRESS_FRAMES: u8 = 8;

const PANEL_BG: Color = Color::RGB(0x1c, 0x2f, 0x5a);
const PANEL_LOWER: Color = Color::RGB(0xd8, 0xd8, 0xd2);
const PANEL_INK: Color = Color::RGB(0xee, 0xee, 0xe8);
const LOWER_INK: Color = Color::RGB(0x1c, 0x2f, 0x5a);
const LED_OFF: Color = Color::RGB(0x40, 0x0c, 0x0c);
const LED_ON: Color = Color::RGB(0xff, 0x30, 0x24);
const LED_BEZEL: Color = Color::RGB(0x10, 0x10, 0x14);
const CHROME: Color = Color::RGB(0xc8, 0xcc, 0xd2);
const CHROME_DARK: Color = Color::RGB(0x70, 0x74, 0x7a);

/// The status lamps, left to right, as indicators 0-9 of the status row.
const STATUS_NAMES: [&str; 10] = ["INTE", "PROT", "MEMR", "INP", "M1", "OUT", "HLTA", "STACK", "WO", "INT"];

/// The command switches: label above, label below, and what each way
/// sends. `None` is a position with nothing behind it.
type Command = (&'static str, &'static str, Option<PanelCommand>, Option<PanelCommand>);
const COMMANDS: [Command; 8] = [
    ("STOP", "RUN", Some(PanelCommand::Halt), Some(PanelCommand::Run)),
    ("SINGLE", "STEP", Some(PanelCommand::SingleCommand), None),
    ("EXAMINE", "NEXT", Some(PanelCommand::Examine), Some(PanelCommand::ExamineNext)),
    ("DEPOSIT", "NEXT", Some(PanelCommand::Deposit), Some(PanelCommand::DepositNext)),
    ("RESET", "CLR", Some(PanelCommand::ResetLine), None),
    ("PROTECT", "UNPROTECT", None, None),
    ("AUX", "AUX", None, None),
    ("AUX", "AUX", None, None),
];

/// Column `i` of the sixteen, A15 at 0: grouped in threes from A0, so A15
/// stands alone.
fn col_x(i: i32) -> i32 {
    let bit = 15 - i;
    COLS_X + i * COL_PITCH + (5 - bit / 3) * GROUP_GAP
}

/// The status lamps sit left of the columns, over WAIT and HLDA.
fn status_x(i: i32) -> i32 {
    40 + i * 40
}

fn command_x(i: i32) -> i32 {
    col_x(0) + i * 2 * COL_PITCH + COL_PITCH / 2
}

/// An LED's face for the fraction of the frame it was lit: linear light,
/// seen through the eye's square root.
fn led_color(duty: f32) -> Color {
    let t = duty.clamp(0.0, 1.0).sqrt();
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    Color::RGB(mix(LED_OFF.r, LED_ON.r), mix(LED_OFF.g, LED_ON.g), mix(LED_OFF.b, LED_ON.b))
}

pub(crate) struct PanelWindow8800 {
    id: u32,
    panel: AltairPanel,
    /// Commands to the run loop. Send errors are ignored: a dead CPU
    /// thread means shutdown is already in flight.
    control: PanelControl,
    prev: [LampSnapshot; 3],
    /// Frames left to draw each command switch pressed, and which way.
    pressed: [(u8, bool); 8],
    canvas: Canvas<Window>,
}

impl PanelWindow8800 {
    pub(crate) fn new(video: &sdl2::VideoSubsystem, title: &str, display: AltairPanelDisplay) -> Result<Self, String> {
        let AltairPanelDisplay { panel, control } = display;
        let window = video.window(title, WINDOW_W, WINDOW_H).position_centered().build().map_err(|e| e.to_string())?;
        let id = window.id();
        let canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
        let prev = [ALTAIR_ADDRESS, ALTAIR_DATA, ALTAIR_STATUS].map(|src| panel.snapshot(src));
        Ok(PanelWindow8800 { id, panel, control, prev, pressed: [(0, false); 8], canvas })
    }

    pub(crate) fn window_id(&self) -> u32 {
        self.id
    }

    fn led(&mut self, cx: i32, cy: i32, duty: f32) {
        lamps::circle(&mut self.canvas, cx, cy, LED_R + 3, LED_BEZEL);
        lamps::circle(&mut self.canvas, cx, cy, LED_R, led_color(duty));
    }

    /// A bat-handle toggle, its lever up, down or (0) level with its nut.
    fn toggle(&mut self, cx: i32, cy: i32, way: i32) {
        lamps::circle(&mut self.canvas, cx, cy, 9, CHROME_DARK);
        lamps::circle(&mut self.canvas, cx, cy, 6, CHROME);
        self.canvas.set_draw_color(CHROME);
        let half = TOGGLE_W as i32 / 2;
        let (top, len) = match way {
            1 => (cy - TOGGLE_LEVER, TOGGLE_LEVER),
            -1 => (cy, TOGGLE_LEVER),
            _ => (cy - 4, 8),
        };
        let _ = self.canvas.fill_rect(Rect::new(cx - half, top, TOGGLE_W, len as u32));
        let tip = if way == 1 { cy - TOGGLE_LEVER } else { cy + if way == -1 { TOGGLE_LEVER } else { 0 } };
        lamps::circle(&mut self.canvas, cx, tip, half + 1, CHROME);
    }

    fn update_duties(&mut self) -> [[f32; 16]; 3] {
        let mut out = [[0.0; 16]; 3];
        for (src, d) in out.iter_mut().enumerate() {
            let now = self.panel.snapshot(src);
            *d = duties(&now, &self.prev[src], self.panel.stopped(src));
            self.prev[src] = now;
        }
        out
    }

    pub(crate) fn render(&mut self) {
        let [address, data, status] = self.update_duties();
        let halted = self.panel.halted();
        self.canvas.set_draw_color(PANEL_BG);
        self.canvas.clear();
        self.canvas.set_draw_color(PANEL_LOWER);
        let lower = ADDRESS_Y + 26;
        let _ = self.canvas.fill_rect(Rect::new(0, lower, WINDOW_W, WINDOW_H - lower as u32));

        lamps::text(&mut self.canvas, 40, TITLE_Y, 3, "MITS ALTAIR 8800", PANEL_INK);
        lamps::text(&mut self.canvas, 360, TITLE_Y + 6, 2, "COMPUTER", PANEL_INK);

        // the status row and D7-D0 over the low eight columns
        for (i, name) in STATUS_NAMES.iter().enumerate() {
            let x = status_x(i as i32);
            lamps::text_centered(&mut self.canvas, x, STATUS_Y - 28, 1, name, PANEL_INK);
            self.led(x, STATUS_Y, status[i]);
        }
        lamps::text_centered(&mut self.canvas, (col_x(8) + col_x(15)) / 2, STATUS_Y - 44, 2, "DATA", PANEL_INK);
        for i in 8..16 {
            let x = col_x(i);
            lamps::text_centered(&mut self.canvas, x, STATUS_Y - 24, 1, &format!("D{}", 15 - i), PANEL_INK);
            self.led(x, STATUS_Y, data[i as usize]);
        }

        // WAIT and HLDA, then the address lamps
        for (i, (name, lit)) in [("WAIT", halted), ("HLDA", false)].into_iter().enumerate() {
            let x = status_x(i as i32);
            lamps::text_centered(&mut self.canvas, x, ADDRESS_Y - 28, 1, name, PANEL_INK);
            self.led(x, ADDRESS_Y, if lit { 1.0 } else { 0.0 });
        }
        lamps::text_centered(&mut self.canvas, (col_x(0) + col_x(15)) / 2, ADDRESS_Y - 44, 2, "ADDRESS", PANEL_INK);
        for i in 0..16 {
            let x = col_x(i);
            lamps::text_centered(&mut self.canvas, x, ADDRESS_Y - 24, 1, &format!("A{}", 15 - i), PANEL_INK);
            self.led(x, ADDRESS_Y, address[i as usize]);
        }

        // the toggles
        let switches = self.panel.switches();
        lamps::text_centered(&mut self.canvas, (col_x(0) + col_x(7)) / 2, lower + 4, 1, "SENSE SW", LOWER_INK);
        lamps::text_centered(&mut self.canvas, (col_x(8) + col_x(15)) / 2, lower + 4, 1, "DATA", LOWER_INK);
        for i in 0..16 {
            let x = col_x(i);
            lamps::text_centered(&mut self.canvas, x, lower + 16, 2, &format!("{}", 15 - i), LOWER_INK);
            let up = switches >> (15 - i) & 1 != 0;
            self.toggle(x, TOGGLE_Y, if up { 1 } else { -1 });
        }

        // the command switches
        for (i, (above, below, _, _)) in COMMANDS.iter().enumerate() {
            let x = command_x(i as i32);
            lamps::text_centered(&mut self.canvas, x, COMMAND_Y - 40, 1, above, LOWER_INK);
            lamps::text_centered(&mut self.canvas, x, COMMAND_Y + 34, 1, below, LOWER_INK);
            let (frames, up) = self.pressed[i];
            let way = if frames == 0 {
                0
            } else if up {
                1
            } else {
                -1
            };
            self.toggle(x, COMMAND_Y, way);
            self.pressed[i].0 = frames.saturating_sub(1);
        }
        self.canvas.present();
    }

    pub(crate) fn click(&mut self, x: i32, y: i32, button: MouseButton) {
        if button != MouseButton::Left {
            return;
        }
        for i in 0..16 {
            if (x - col_x(i)).abs() <= COL_PITCH / 2 - 4 && (y - TOGGLE_Y).abs() <= TOGGLE_LEVER + 8 {
                self.panel.toggle_switch(15 - i as u8);
                return;
            }
        }
        for (i, (_, _, up, down)) in COMMANDS.iter().enumerate() {
            if (x - command_x(i as i32)).abs() <= COL_PITCH - 8 && (y - COMMAND_Y).abs() <= TOGGLE_LEVER + 8 {
                let pressed_up = y < COMMAND_Y;
                self.pressed[i] = (PRESS_FRAMES, pressed_up);
                if let Some(cmd) = if pressed_up { up } else { down } {
                    let _ = self.control.send(*cmd);
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A15 alone, then threes down to A0, as the octal markings group them.
    #[test]
    fn the_columns_group_in_threes_from_a0() {
        let gap = |i: i32| col_x(i + 1) - col_x(i);
        assert_eq!(gap(0), COL_PITCH + GROUP_GAP, "A15 | A14");
        assert_eq!(gap(1), COL_PITCH, "A14 A13");
        assert_eq!(gap(3), COL_PITCH + GROUP_GAP, "A12 | A11");
        assert_eq!(gap(12), COL_PITCH + GROUP_GAP, "A3 | A2");
        assert_eq!(gap(14), COL_PITCH, "A1 A0");
        assert!(col_x(15) + COL_PITCH < WINDOW_W as i32);
        assert!(command_x(7) + COL_PITCH < WINDOW_W as i32);
    }

    #[test]
    fn an_led_is_as_bright_as_its_duty_looks() {
        assert_eq!(led_color(0.0), LED_OFF);
        assert_eq!(led_color(1.0), LED_ON);
        let quarter = led_color(0.25);
        assert_eq!(quarter.r, ((LED_OFF.r as f32 + (LED_ON.r as f32 - LED_OFF.r as f32) * 0.5) as u8));
    }
}
