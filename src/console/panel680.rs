// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The MITS Altair 680b's front panel, after its display/control board:
//! the 680 display schematic (680-100, sheet 4) for what is there, and the
//! assembly manual's drawings of the board (pages 64 and 65) for where.
//!
//! Twenty-seven LEDs and twenty-seven switches in two bands. The upper
//! band has AC, HALT and RUN at the left over RESET, RUN/HALT and DEP,
//! and D7-D0 over their eight data toggles at the right, above A7-A0; the
//! lower band is A15-A0 over the sixteen address toggles, grouped by four
//! since the manual has them read in hexadecimal. A toggle up is a one.
//! The colors, the lettering and the title are invented: no document at
//! hand shows the panel's silkscreen.
//!
//! The lamps are LEDs on the buses themselves. Running, each shows its
//! line's share of the last frame's bus accesses (`Cpu6800`'s tap),
//! through the eye's square-root response, as the 8800's do. Halted --
//! and in WAI, the other time the 6800 gives up the bus -- the address
//! toggles drive the address lines, so the address lamps show the toggles
//! and the data lamps the byte at that location: "When in the HALT mode,
//! the unit is continuously addressing whatever location is defined by
//! the ADDRESS switches" (Operator's Manual IV). A view that renders while
//! halted asks the core to look again each frame, which is that continuous
//! addressing, and catches a debugger's write as the real lamps would.
//!
//! RUN/HALT is a latching toggle, drawn from the machine's run state so
//! the debug port's `run` and `halt` move it too; clicking above or below
//! its pivot runs or halts. RESET and DEP are momentary. RESET acts only
//! on a halted machine, the RUN/HALT flip-flop gating it (Theory of
//! Operation 10A), and leaves it halted at the reset vector; DEP writes the
//! data toggles into the selected location. Keystrokes into this window go
//! nowhere: the console is the terminal.

use super::lamps::{self, duties};
use super::surface::{Color, Surface};
use super::{AltairPanel, AltairPanelDisplay, LampSnapshot, PanelCommand, PanelControl, ALTAIR_ADDRESS, ALTAIR_DATA};

const WINDOW_W: u32 = 968;
const WINDOW_H: u32 = 430;

const COL_PITCH: i32 = 52;
const GROUP_GAP: i32 = 16;
const COLS_X: i32 = 70;
const LED_R: i32 = 9;
const UPPER_LAMP_Y: i32 = 92;
const UPPER_SWITCH_Y: i32 = 170;
const LOWER_LAMP_Y: i32 = 276;
const LOWER_SWITCH_Y: i32 = 354;
const TOGGLE_W: u32 = 10;
const TOGGLE_LEVER: i32 = 22;
/// Frames a momentary switch stays drawn pressed after a click.
const PRESS_FRAMES: u8 = 8;

const PANEL_BG: Color = Color::RGB(0x1c, 0x2f, 0x5a);
const PANEL_INK: Color = Color::RGB(0xee, 0xee, 0xe8);
const RULE: Color = Color::RGB(0x5a, 0x6c, 0x92);
const LED_OFF: Color = Color::RGB(0x40, 0x0c, 0x0c);
const LED_ON: Color = Color::RGB(0xff, 0x30, 0x24);
const LED_BEZEL: Color = Color::RGB(0x10, 0x10, 0x14);
const CHROME: Color = Color::RGB(0xc8, 0xcc, 0xd2);
const CHROME_DARK: Color = Color::RGB(0x70, 0x74, 0x7a);

/// The three switches under AC, HALT and RUN, in their columns.
const RESET: i32 = 0;
const RUN_HALT: i32 = 1;
const DEP: i32 = 2;

/// Column `i` of the sixteen, A15 at 0, grouped by four from A0.
fn col_x(i: i32) -> i32 {
    COLS_X + i * COL_PITCH + (i / 4) * GROUP_GAP
}

/// An LED's face for the fraction of the frame it was lit: linear light,
/// seen through the eye's square root.
fn led_color(duty: f32) -> Color {
    let t = duty.clamp(0.0, 1.0).sqrt();
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    Color::RGB(mix(LED_OFF.r, LED_ON.r), mix(LED_OFF.g, LED_ON.g), mix(LED_OFF.b, LED_ON.b))
}

/// The toggle bit behind column `i` of a band: An in the lower band, Dn
/// (bit 16 + n) in the upper band's right half.
fn toggle_bit(upper: bool, i: i32) -> Option<u8> {
    match (upper, i) {
        (false, 0..=15) => Some(15 - i as u8),
        (true, 8..=15) => Some(16 + 15 - i as u8),
        _ => None,
    }
}

/// The panel: its lamps, its toggles and its hit-testing, on no
/// particular host.
pub struct PanelView680 {
    panel: AltairPanel,
    /// Switch actuations to the run loop. Send errors are ignored: a dead
    /// CPU thread means shutdown is already in flight.
    control: PanelControl,
    prev: [LampSnapshot; 2],
    /// The last frame's duties while running, for a frame the core's tap
    /// flushed nothing in: slowed far enough, a run has frames like that.
    running: [[f32; 16]; 2],
    /// Frames left to draw RESET and DEP pressed.
    pressed: [u8; 3],
}

impl PanelView680 {
    /// The view's size in pixels.
    pub const WIDTH: u32 = WINDOW_W;
    pub const HEIGHT: u32 = WINDOW_H;

    pub fn new(display: AltairPanelDisplay) -> Self {
        let AltairPanelDisplay { panel, control } = display;
        let prev = [ALTAIR_ADDRESS, ALTAIR_DATA].map(|src| panel.snapshot(src));
        PanelView680 { panel, control, prev, running: [[0.0; 16]; 2], pressed: [0; 3] }
    }

    fn led(s: &mut dyn Surface, cx: i32, cy: i32, duty: f32) {
        lamps::circle(s, cx, cy, LED_R + 3, LED_BEZEL);
        lamps::circle(s, cx, cy, LED_R, led_color(duty));
    }

    /// A bat-handle toggle, its lever up, down or (0) level with its nut.
    fn toggle(s: &mut dyn Surface, cx: i32, cy: i32, way: i32) {
        lamps::circle(s, cx, cy, 9, CHROME_DARK);
        lamps::circle(s, cx, cy, 6, CHROME);
        let half = TOGGLE_W as i32 / 2;
        let (top, len) = match way {
            1 => (cy - TOGGLE_LEVER, TOGGLE_LEVER),
            -1 => (cy, TOGGLE_LEVER),
            _ => (cy - 4, 8),
        };
        s.fill_rect(cx - half, top, TOGGLE_W, len as u32, CHROME);
        let tip = match way {
            1 => cy - TOGGLE_LEVER,
            -1 => cy + TOGGLE_LEVER,
            _ => cy,
        };
        lamps::circle(s, cx, tip, half + 1, CHROME);
    }

    /// Each row's duty over the frame, or what is on the bus with the
    /// 6800 off it. A running frame with nothing flushed shows what the
    /// last one did: the toggles' location is on the bus only off it.
    fn update_duties(&mut self, halted: bool) -> [[f32; 16]; 2] {
        let mut out = [[0.0; 16]; 2];
        let mut flushed = false;
        for (i, src) in [ALTAIR_ADDRESS, ALTAIR_DATA].into_iter().enumerate() {
            let now = self.panel.snapshot(src);
            flushed |= now.cycles != self.prev[i].cycles;
            out[i] = duties(&now, &self.prev[i], self.panel.stopped(src));
            self.prev[i] = now;
        }
        if halted {
            return out;
        }
        if flushed {
            self.running = out;
        }
        self.running
    }

    pub fn render(&mut self, s: &mut dyn Surface) {
        let halted = self.panel.halted();
        if halted {
            let _ = self.control.send(PanelCommand::Switches);
        }
        let [address, data] = self.update_duties(halted);
        let switches = self.panel.switches();
        s.clear(PANEL_BG);

        lamps::text(s, col_x(3) + 10, UPPER_LAMP_Y - 34, 3, "ALTAIR 680B", PANEL_INK);
        lamps::text(s, col_x(3) + 10, UPPER_LAMP_Y + 4, 2, "MITS", PANEL_INK);
        let rule_y = UPPER_SWITCH_Y + 48;
        s.fill_rect(COLS_X - 30, rule_y, (col_x(15) - COLS_X + 60) as u32, 2, RULE);

        // AC, HALT, RUN and their switches
        for (i, (name, lit)) in [("AC", true), ("HALT", halted), ("RUN", !halted)].into_iter().enumerate() {
            let x = col_x(i as i32);
            lamps::text_centered(s, x, UPPER_LAMP_Y - 24, 1, name, PANEL_INK);
            Self::led(s, x, UPPER_LAMP_Y, if lit { 1.0 } else { 0.0 });
        }
        for (col, above, below) in [(RESET, "RESET", ""), (RUN_HALT, "RUN", "HALT"), (DEP, "DEP", "")] {
            let x = col_x(col);
            lamps::text_centered(s, x, UPPER_SWITCH_Y - 40, 1, above, PANEL_INK);
            lamps::text_centered(s, x, UPPER_SWITCH_Y + 32, 1, below, PANEL_INK);
            let way = if col == RUN_HALT {
                if halted {
                    -1
                } else {
                    1
                }
            } else if self.pressed[col as usize] > 0 {
                self.pressed[col as usize] -= 1;
                1
            } else {
                0
            };
            Self::toggle(s, x, UPPER_SWITCH_Y, way);
        }

        // D7-D0 over A7-A0, then A15-A0
        lamps::text_centered(s, (col_x(8) + col_x(15)) / 2, UPPER_LAMP_Y - 44, 2, "DATA", PANEL_INK);
        for i in 8..16 {
            let x = col_x(i);
            lamps::text_centered(s, x, UPPER_LAMP_Y - 24, 1, &format!("D{}", 15 - i), PANEL_INK);
            Self::led(s, x, UPPER_LAMP_Y, data[i as usize]);
        }
        lamps::text_centered(s, (col_x(0) + col_x(15)) / 2, LOWER_LAMP_Y - 44, 2, "ADDRESS", PANEL_INK);
        for i in 0..16 {
            let x = col_x(i);
            lamps::text_centered(s, x, LOWER_LAMP_Y - 24, 1, &format!("A{}", 15 - i), PANEL_INK);
            Self::led(s, x, LOWER_LAMP_Y, address[i as usize]);
        }

        // the toggles
        for (upper, y) in [(true, UPPER_SWITCH_Y), (false, LOWER_SWITCH_Y)] {
            for i in 0..16 {
                if let Some(bit) = toggle_bit(upper, i) {
                    let up = switches >> bit & 1 != 0;
                    Self::toggle(s, col_x(i), y, if up { 1 } else { -1 });
                }
            }
        }
    }

    /// A click at (x, y) in the view's pixels.
    pub fn click(&mut self, x: i32, y: i32) {
        let near = |cx: i32, cy: i32| (x - cx).abs() <= COL_PITCH / 2 - 4 && (y - cy).abs() <= TOGGLE_LEVER + 8;
        for (upper, cy) in [(true, UPPER_SWITCH_Y), (false, LOWER_SWITCH_Y)] {
            for i in 0..16 {
                if !near(col_x(i), cy) {
                    continue;
                }
                if let Some(bit) = toggle_bit(upper, i) {
                    self.panel.toggle_switch(bit);
                    let _ = self.control.send(PanelCommand::Switches);
                    return;
                }
                let cmd = match i {
                    RESET => PanelCommand::ResetWhenHalted,
                    RUN_HALT if y < cy => PanelCommand::Run,
                    RUN_HALT => PanelCommand::Halt,
                    DEP => PanelCommand::Deposit,
                    _ => return,
                };
                if i != RUN_HALT {
                    self.pressed[i as usize] = PRESS_FRAMES;
                }
                let _ = self.control.send(cmd);
                return;
            }
        }
    }
}

/// The panel in an SDL window of its own.
#[cfg(not(target_arch = "wasm32"))]
mod window {
    use super::{AltairPanelDisplay, PanelView680, WINDOW_H, WINDOW_W};
    use sdl2::mouse::MouseButton;
    use sdl2::render::Canvas;
    use sdl2::video::Window;

    pub(crate) struct PanelWindow680 {
        id: u32,
        view: PanelView680,
        canvas: Canvas<Window>,
    }

    impl PanelWindow680 {
        pub(crate) fn new(video: &sdl2::VideoSubsystem, title: &str, display: AltairPanelDisplay) -> Result<Self, String> {
            let window = video.window(title, WINDOW_W, WINDOW_H).position_centered().build().map_err(|e| e.to_string())?;
            let canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
            Ok(PanelWindow680 { id: canvas.window().id(), view: PanelView680::new(display), canvas })
        }

        pub(crate) fn window_id(&self) -> u32 {
            self.id
        }

        pub(crate) fn render(&mut self) {
            self.view.render(&mut self.canvas);
            self.canvas.present();
        }

        pub(crate) fn click(&mut self, x: i32, y: i32, button: MouseButton) {
            if button == MouseButton::Left {
                self.view.click(x, y);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use window::PanelWindow680;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::surface::Framebuffer;
    use crate::console::RunLamp;
    use crate::emulator::Control;
    use std::sync::mpsc;

    fn view() -> (PanelView680, AltairPanel, mpsc::Receiver<Control>) {
        let panel = AltairPanel::new();
        let (tx, rx) = mpsc::channel();
        let view = PanelView680::new(AltairPanelDisplay { panel: panel.clone(), control: PanelControl(tx) });
        (view, panel, rx)
    }

    fn commands(rx: &mpsc::Receiver<Control>) -> Vec<PanelCommand> {
        rx.try_iter()
            .map(|c| match c {
                Control::Panel(cmd) => cmd,
                _ => panic!("the panel sends panel commands"),
            })
            .collect()
    }

    fn pixel(fb: &Framebuffer, x: i32, y: i32) -> Color {
        let o = (y as usize * fb.width as usize + x as usize) * 4;
        Color::RGB(fb.pixels[o], fb.pixels[o + 1], fb.pixels[o + 2])
    }

    /// Four groups of four, A15 at the left, and everything inside the
    /// window.
    #[test]
    fn the_columns_group_by_four_from_a0() {
        let gap = |i: i32| col_x(i + 1) - col_x(i);
        assert_eq!(gap(2), COL_PITCH, "A13 A12");
        assert_eq!(gap(3), COL_PITCH + GROUP_GAP, "A12 | A11");
        assert_eq!(gap(11), COL_PITCH + GROUP_GAP, "A4 | A3");
        assert_eq!(col_x(15) + COLS_X, WINDOW_W as i32, "the margins match");
    }

    /// The data toggles are D7 over A7 to D0 over A0, bits 23 to 16; the
    /// address toggles A15 to A0; the upper band's left half has none.
    #[test]
    fn toggles_click_to_their_bits_and_say_so() {
        let (mut view, panel, rx) = view();
        view.click(col_x(8), UPPER_SWITCH_Y);
        view.click(col_x(15), UPPER_SWITCH_Y);
        view.click(col_x(0), LOWER_SWITCH_Y);
        view.click(col_x(15), LOWER_SWITCH_Y);
        assert_eq!(panel.switches(), 1 << 23 | 1 << 16 | 1 << 15 | 1);
        assert_eq!(commands(&rx), vec![PanelCommand::Switches; 4]);
        view.click(col_x(5), UPPER_SWITCH_Y);
        assert_eq!(commands(&rx), vec![]);
    }

    /// RUN above the RUN/HALT pivot, HALT below; RESET and DEP either way.
    #[test]
    fn the_command_switches_send_their_commands() {
        let (mut view, _panel, rx) = view();
        view.click(col_x(RUN_HALT), UPPER_SWITCH_Y - 10);
        view.click(col_x(RUN_HALT), UPPER_SWITCH_Y + 10);
        view.click(col_x(RESET), UPPER_SWITCH_Y);
        view.click(col_x(DEP), UPPER_SWITCH_Y + 10);
        assert_eq!(
            commands(&rx),
            vec![PanelCommand::Run, PanelCommand::Halt, PanelCommand::ResetWhenHalted, PanelCommand::Deposit]
        );
    }

    /// Halted, the lamps are what the core says is on the bus, and each
    /// frame asks it to look again; HALT is lit and RUN dark, AC always.
    #[test]
    fn halted_the_lamps_show_the_selected_location() {
        let (mut view, panel, rx) = view();
        panel.set_halted(true);
        panel.set_stopped(0x8001, 0x81, 0);
        let mut fb = Framebuffer::new(PanelView680::WIDTH, PanelView680::HEIGHT);
        view.render(&mut fb);
        assert_eq!(commands(&rx), vec![PanelCommand::Switches]);
        let lit = |fb: &Framebuffer, x, y| pixel(fb, x, y) == LED_ON;
        assert!(lit(&fb, col_x(0), LOWER_LAMP_Y), "A15");
        assert!(!lit(&fb, col_x(1), LOWER_LAMP_Y), "A14");
        assert!(lit(&fb, col_x(15), LOWER_LAMP_Y), "A0");
        assert!(lit(&fb, col_x(8), UPPER_LAMP_Y), "D7");
        assert!(!lit(&fb, col_x(9), UPPER_LAMP_Y), "D6");
        assert!(lit(&fb, col_x(15), UPPER_LAMP_Y), "D0");
        assert!(lit(&fb, col_x(0), UPPER_LAMP_Y) && lit(&fb, col_x(1), UPPER_LAMP_Y), "AC, HALT");
        assert!(!lit(&fb, col_x(2), UPPER_LAMP_Y), "RUN");

        panel.set_halted(false);
        let mut counts = [[0; 16]; 3];
        counts[ALTAIR_ADDRESS][14] = 128; // A1, throughout
        panel.flush(&counts, 128);
        view.render(&mut fb);
        assert_eq!(commands(&rx), vec![], "running: the core's tap feeds the lamps");
        assert!(lit(&fb, col_x(2), UPPER_LAMP_Y) && !lit(&fb, col_x(1), UPPER_LAMP_Y), "RUN, not HALT");
        let running = |fb: &Framebuffer| (lit(fb, col_x(14), LOWER_LAMP_Y), lit(fb, col_x(0), LOWER_LAMP_Y));
        assert_eq!(running(&fb), (true, false), "A1 lit, A15 not");
        // a frame with nothing flushed, as in slow motion: the same again,
        // not the toggles' location
        view.render(&mut fb);
        assert_eq!(running(&fb), (true, false));
    }

    #[test]
    fn an_led_is_as_bright_as_its_duty_looks() {
        assert_eq!(led_color(0.0), LED_OFF);
        assert_eq!(led_color(1.0), LED_ON);
    }
}
