// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The Raytheon 703's front panel, drawn after figure 5-1 of the reference
//! manual (section 5, "Controls and Indicators").
//!
//! Two rows of round lamp-indicators -- the PROGRAM COUNTER row, always
//! live, and the SELECTED DISPLAY row behind the six-position DISPLAY
//! SELECTOR rotary -- rendered as incandescent bulbs from the duty-cycle
//! accumulators the core feeds; the switches: RUN, HALT -- a red
//! switch-indicator, lit while the machine is halted -- RESET, SINGLE
//! STEP/COMMAND, the two CLEARs, ENTER/DISPLAY, the SENSE toggles, and
//! the lamps themselves, which are switch-indicators -- clicking one keys
//! that bit. Switch actuations go to the run loop as [`PanelCommand`]s;
//! the selector knob is state of this frontend alone, which is what lets
//! it be "changed while the program is running" (5-2).
//!
//! The machine starts halted, as a real one did at power-on: the HALT
//! lens glows red until RUN is pressed.
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

use super::{ConsoleFrontend, Display, LampSnapshot, PanelCommand, PanelState, Selector};
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

/// The momentary buttons in the control row, left of the SENSE toggles,
/// in figure 5-1's order: center x and label. SINGLE STEP is drawn as the
/// figure has it but wired to the same one-instruction step as SINGLE
/// COMMAND -- the real switch stepped sub-instruction phases (5-4) this
/// emulator does not model, and its RUN/SINGLE COMMAND inhibit mode goes
/// with them. A documented divergence.
const BUTTONS: [(i32, &str); 5] =
    [(84, "RESET"), (172, "HALT"), (256, "RUN"), (352, "SINGLE STEP"), (472, "SINGLE COMMAND")];
const BUTTON_R: i32 = 11;
/// ENTER and DISPLAY, switch-indicators lit when MB is selected (5-3).
const ENTER_X: i32 = 800;
const DISPLAY_X: i32 = 902;
/// The CLEAR button at the head of each lamp row (5-1, 5-3).
const CLEAR_X: i32 = 26;
const CLEAR_R: i32 = 8;

const PANEL_BG: Color = Color::RGB(0xd6, 0xd2, 0xc9);
const PANEL_INK: Color = Color::RGB(0x22, 0x22, 0x22);
/// Full drive is a near-white amber -- a filament at temperature, not the
/// orange of a half-lit one (that lives in `lamp_color`'s ember stop).
const LAMP_LIT: Color = Color::RGB(0xff, 0xd6, 0x82);
/// The glow a bright bulb throws past its bezel, drawn with alpha.
const LAMP_GLOW: Color = Color::RGB(0xff, 0xb0, 0x50);
const LAMP_UNLIT: Color = Color::RGB(0x54, 0x42, 0x2a);
const LAMP_BEZEL: Color = Color::RGB(0x3a, 0x3a, 0x3a);
/// The HALT indicator's red lens: a bulb behind red glass lit, a deep
/// maroon unlit -- which is the near-black circle figure 5-1 draws.
const HALT_LIT: Color = Color::RGB(0xe8, 0x38, 0x24);
const HALT_UNLIT: Color = Color::RGB(0x46, 0x12, 0x0e);
const HALT_GLOW: Color = Color::RGB(0xe0, 0x30, 0x20);

/// The selector's positions in [`SELECTOR_ORDER`] order, spread over the top
/// arc of the knob like the rotary in figure 5-1.
const SELECTOR_ORDER: [Selector; 6] =
    [Selector::Ms, Selector::In, Selector::Ma, Selector::Mb, Selector::Ix, Selector::Ac];
const SELECTOR_NAMES: [&str; 6] = ["MS", "IN", "MA", "MB", "IX", "AC"];

fn lamp_x(i: i32) -> i32 {
    LAMPS_X + i * LAMP_PITCH + (i / 4) * GROUP_GAP
}

/// One incandescent bulb's thermal state. That they *are* incandescent is
/// documented, not assumed: the 704 technical manual (July 1970, same
/// panel design) says 6-volt power "is used solely to light indicator
/// lamps on the front panel assembly" with drivers switching the lamps'
/// return lines (section 4-745), and the SELECTED DISPLAY drivers ground
/// those returns straight from register outputs (4-76) -- DC-driven
/// filaments flickering at logic speed, no neon anywhere (the machine has
/// no high-voltage rail at all, 4-749). A 6V panel bulb of the era has a
/// thermal time constant in the tens of milliseconds.
///
/// The lamps are driven by bits that flip thousands of times per frame,
/// and what a filament shows for that is its duty cycle, arrived at with
/// a lag: it heats faster than it cools. The constants are per 16 ms
/// frame and tuned by eye within that regime.
#[derive(Copy, Clone, Default)]
struct LampFilter {
    brightness: f32,
}

impl LampFilter {
    const K_HEAT: f32 = 0.6;
    const K_COOL: f32 = 0.25;

    fn update(&mut self, duty: f32) -> f32 {
        let duty = duty.clamp(0.0, 1.0);
        let k = if duty > self.brightness { Self::K_HEAT } else { Self::K_COOL };
        self.brightness += (duty - self.brightness) * k;
        self.brightness
    }
}

/// Per-indicator duty cycles between two snapshots of one lamp source.
/// While the machine is halted no cycles accrue, and the point-sampled
/// register value stands in -- full on or off, which is what a static
/// register looks like on a real panel.
fn duties(now: &LampSnapshot, prev: &LampSnapshot, halted_value: u16) -> [f32; 16] {
    let dc = now.cycles.wrapping_sub(prev.cycles);
    let mut out = [0.0f32; 16];
    for (i, out) in out.iter_mut().enumerate() {
        *out = if dc == 0 {
            (halted_value >> (15 - i) & 1) as f32
        } else {
            now.bits[i].wrapping_sub(prev.bits[i]) as f32 / dc as f32
        };
    }
    out
}

/// Perceived brightness for a duty cycle. A PWM'd filament's light output
/// runs superlinear in duty (roughly phi ~ D^1.7 -- temperature tracks
/// duty, radiance runs away with temperature) and the eye compresses
/// that by roughly a square root, so perceived ~ sqrt(D^1.7) ~ D^0.85.
fn perceived(brightness: f32) -> f32 {
    brightness.clamp(0.0, 1.0).powf(0.85)
}

/// Lamp face color for a perceived brightness. Two segments, because a
/// filament changes color as it brightens: cold brown through dim ember
/// orange to a near-white amber at full drive.
fn lamp_color(t: f32) -> Color {
    const EMBER: Color = Color::RGB(0xc8, 0x5e, 0x14);
    let mix = |a: u8, b: u8, f: f32| (a as f32 + (b as f32 - a as f32) * f) as u8;
    let (lo, hi, f) =
        if t < 0.5 { (LAMP_UNLIT, EMBER, t * 2.0) } else { (EMBER, LAMP_LIT, (t - 0.5) * 2.0) };
    Color::RGB(mix(lo.r, hi.r, f), mix(lo.g, hi.g, f), mix(lo.b, hi.b, f))
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
    /// Switch actuations to the run loop. Send errors are ignored
    /// throughout: a dead CPU thread means shutdown is already in flight.
    control: Sender<PanelCommand>,
    selector: Selector,
    /// Last frame's accumulator snapshots, one per lamp source (PC + the
    /// six selector positions), so turning the knob always has a fresh
    /// delta to diff against.
    prev_pc: LampSnapshot,
    prev_sel: [LampSnapshot; 6],
    /// One filter per *physical bulb*: the SELECTED DISPLAY row is sixteen
    /// real lamps switched between registers by the rotary, so their
    /// thermal state carries across a selector change.
    pc_filters: [LampFilter; 16],
    sd_filters: [LampFilter; 16],
    sdl: sdl2::Sdl,
    canvas: Canvas<Window>,
}

impl Panel703Frontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        let Display::Panel703 { title, panel, control } = display else {
            return Err("Panel703Frontend needs a 703 panel display".to_string());
        };
        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        let window = video_subsystem
            .window(title, WINDOW_W, WINDOW_H)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;
        let mut canvas = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
        // the lamp glow halos draw with alpha
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        // MB is the boot-procedure position ("turn the display selector to
        // MB before following the operating procedure" -- the PTB drawing),
        // and it is also the busiest lamp row on an idle machine.
        Ok(Panel703Frontend {
            tx,
            panel,
            control,
            selector: Selector::Mb,
            prev_pc: LampSnapshot::default(),
            prev_sel: [LampSnapshot::default(); 6],
            pc_filters: [LampFilter::default(); 16],
            sd_filters: [LampFilter::default(); 16],
            sdl,
            canvas,
        })
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

    /// One row of lamps at the given brightnesses, indicator 0 (the MSB, as
    /// everywhere on this machine) leftmost. `first` skips leading
    /// indicators: the PROGRAM COUNTER row starts at 1 because the PCR is
    /// 15 bits.
    fn lamp_row(&mut self, y: i32, brightness: &[f32; 16], first: i32) {
        for i in first..16 {
            let x = lamp_x(i);
            let t = perceived(brightness[i as usize]);
            // a bright bulb glows past its bezel; alpha needs the canvas
            // blend mode set in new()
            if t > 0.25 {
                let alpha = (90.0 * (t - 0.25) / 0.75) as u8;
                let glow = Color::RGBA(LAMP_GLOW.r, LAMP_GLOW.g, LAMP_GLOW.b, alpha);
                self.circle(x, y, LAMP_R + 7, glow);
            }
            self.circle(x, y, LAMP_R + 3, LAMP_BEZEL);
            self.circle(x, y, LAMP_R, lamp_color(t));
            let label = i.to_string();
            self.text_centered(x, y - LAMP_R - 15, 1, &label, PANEL_INK);
        }
    }

    fn draw_buttons(&mut self) {
        for (x, label) in BUTTONS {
            // HALT is a red light -- figure 5-1's one dark circle is its
            // lens -- so it renders as a switch-indicator like ENTER and
            // DISPLAY: still pressed to halt, and lit while the machine
            // sits halted, from the run state the Emulator publishes.
            // (The lamp rows' cycle deltas can't drive it: under a slow
            // --throttle a running machine has whole frames with no
            // cycles in them.)
            let face = if label == "HALT" {
                if self.panel.halted() {
                    let glow = Color::RGBA(HALT_GLOW.r, HALT_GLOW.g, HALT_GLOW.b, 80);
                    self.circle(x, CONTROLS_Y, BUTTON_R + 7, glow);
                    HALT_LIT
                } else {
                    HALT_UNLIT
                }
            } else {
                Color::RGB(0x8a, 0x8a, 0x84)
            };
            self.circle(x, CONTROLS_Y, BUTTON_R + 2, LAMP_BEZEL);
            self.circle(x, CONTROLS_Y, BUTTON_R, face);
            self.text_centered(x, CONTROLS_Y - BUTTON_R - 20, 1, label, PANEL_INK);
        }

        // ENTER and DISPLAY are switch-indicators, illuminated when the
        // selector sits on MB (5-3) -- the position their workflow serves.
        let lit = self.selector == Selector::Mb;
        for (x, label) in [(ENTER_X, "ENTER"), (DISPLAY_X, "DISPLAY")] {
            self.circle(x, CONTROLS_Y, BUTTON_R + 2, LAMP_BEZEL);
            self.circle(x, CONTROLS_Y, BUTTON_R, if lit { LAMP_LIT } else { LAMP_UNLIT });
            self.text_centered(x, CONTROLS_Y - BUTTON_R - 20, 1, label, PANEL_INK);
        }

        // one CLEAR at the head of each lamp row
        for y in [PC_LAMPS_Y, SD_LAMPS_Y] {
            self.circle(CLEAR_X, y, CLEAR_R + 2, LAMP_BEZEL);
            self.circle(CLEAR_X, y, CLEAR_R, Color::RGB(0x8a, 0x8a, 0x84));
            self.text_centered(CLEAR_X, y - LAMP_R - 15, 1, "CLEAR", PANEL_INK);
        }
    }

    fn draw_toggles(&mut self) {
        // a local caption over just the four toggles -- the full-width rule
        // would run through the button labels sharing this row
        let cx = (toggle_x(0) + toggle_x(3)) / 2;
        self.text_centered(cx, CONTROLS_Y - TOGGLE_H as i32 / 2 - 34, 1, "SENSE", PANEL_INK);
        self.canvas.set_draw_color(PANEL_INK);
        let _ = self.canvas.fill_rect(Rect::new(toggle_x(0) - 12, CONTROLS_Y - TOGGLE_H as i32 / 2 - 31, (cx - 22 - toggle_x(0) + 12) as u32, 1));
        let _ = self.canvas.fill_rect(Rect::new(cx + 22, CONTROLS_Y - TOGGLE_H as i32 / 2 - 31, (toggle_x(3) + 12 - cx - 22) as u32, 1));
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

    /// Advance the lamp filters one frame from fresh accumulator snapshots
    /// and return the two rows' brightnesses. Separated from render() so
    /// the filter pipeline is testable without a canvas.
    fn update_lamps(&mut self) -> ([f32; 16], [f32; 16]) {
        let now_pc = self.panel.snapshot_pc();
        let pc_duty = duties(&now_pc, &self.prev_pc, self.panel.program_counter());
        self.prev_pc = now_pc;

        // snapshot all six sources so the one behind the knob always has a
        // one-frame-old baseline the moment it is selected
        let mut now_sel = [LampSnapshot::default(); 6];
        for (i, s) in SELECTOR_ORDER.iter().enumerate() {
            now_sel[i] = self.panel.snapshot(*s);
        }
        let si = self.selector as usize;
        let sd_duty =
            duties(&now_sel[si], &self.prev_sel[si], self.panel.selected(self.selector));
        self.prev_sel = now_sel;

        let mut pc = [0.0f32; 16];
        let mut sd = [0.0f32; 16];
        for i in 0..16 {
            pc[i] = self.pc_filters[i].update(pc_duty[i]);
            sd[i] = self.sd_filters[i].update(sd_duty[i]);
        }
        (pc, sd)
    }

    fn render(&mut self) {
        let (pc, sd) = self.update_lamps();

        self.canvas.set_draw_color(PANEL_BG);
        self.canvas.clear();

        self.caption(PC_CAPTION_Y, "PROGRAM COUNTER");
        self.lamp_row(PC_LAMPS_Y, &pc, 1);

        self.caption(SD_CAPTION_Y, "SELECTED DISPLAY");
        self.lamp_row(SD_LAMPS_Y, &sd, 0);

        self.draw_buttons();
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
        let hit_circle = |cx: i32, cy: i32, r: i32| {
            let (dx, dy) = (x - cx, y - cy);
            dx * dx + dy * dy <= r * r
        };

        // the momentary buttons
        for (i, (bx, _)) in BUTTONS.iter().enumerate() {
            if hit_circle(*bx, CONTROLS_Y, BUTTON_R + 4) {
                let cmd = match i {
                    0 => PanelCommand::Reset,
                    1 => PanelCommand::Halt,
                    2 => PanelCommand::Run,
                    // SINGLE STEP and SINGLE COMMAND both step once here
                    _ => PanelCommand::SingleCommand,
                };
                let _ = self.control.send(cmd);
                return;
            }
        }

        // ENTER and DISPLAY, only in the MB workflow they illuminate for
        if self.selector == Selector::Mb {
            if hit_circle(ENTER_X, CONTROLS_Y, BUTTON_R + 4) {
                let _ = self.control.send(PanelCommand::Enter);
                return;
            }
            if hit_circle(DISPLAY_X, CONTROLS_Y, BUTTON_R + 4) {
                let _ = self.control.send(PanelCommand::Display);
                return;
            }
        }

        // the CLEAR at the head of each row; the display CLEAR reaches
        // only the writable positions (5-3)
        if hit_circle(CLEAR_X, PC_LAMPS_Y, CLEAR_R + 5) {
            let _ = self.control.send(PanelCommand::ClearPc);
            return;
        }
        if hit_circle(CLEAR_X, SD_LAMPS_Y, CLEAR_R + 5) {
            if matches!(self.selector, Selector::Mb | Selector::Ix | Selector::Ac) {
                let _ = self.control.send(PanelCommand::ClearSelected(self.selector));
            }
            return;
        }

        // the lamps are switch-indicators: clicking one keys that bit.
        // The PC row has no indicator 0, and SELECTED DISPLAY entry
        // reaches only MB, IX and AC (5-2).
        for i in 0..16i32 {
            if i >= 1 && hit_circle(lamp_x(i), PC_LAMPS_Y, LAMP_R + 3) {
                let _ = self.control.send(PanelCommand::TogglePcBit(i as u8));
                return;
            }
            if hit_circle(lamp_x(i), SD_LAMPS_Y, LAMP_R + 3) {
                if matches!(self.selector, Selector::Mb | Selector::Ix | Selector::Ac) {
                    let _ =
                        self.control.send(PanelCommand::ToggleSelectedBit(self.selector, i as u8));
                }
                return;
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The filament heats faster than it cools: a step up gets closer to
    /// its target in one frame than a step down does.
    #[test]
    fn lamp_filter_heats_faster_than_it_cools() {
        let mut heating = LampFilter::default();
        let rise = heating.update(1.0);
        let mut cooling = LampFilter { brightness: 1.0 };
        let fall = 1.0 - cooling.update(0.0);
        assert!(rise > fall, "rise {rise} should beat fall {fall}");
    }

    /// The perceptual transfer is D^0.85: pinned at the endpoints, above
    /// linear in the middle, and clamped.
    #[test]
    fn perceived_brightness_follows_the_power_law() {
        assert_eq!(perceived(0.0), 0.0);
        assert_eq!(perceived(1.0), 1.0);
        assert!((perceived(0.5) - 0.5f32.powf(0.85)).abs() < 1e-6);
        assert!(perceived(0.5) > 0.5);
        assert_eq!(perceived(7.0), 1.0);
    }

    #[test]
    fn lamp_filter_converges_and_clamps() {
        let mut f = LampFilter::default();
        for _ in 0..40 {
            f.update(0.3);
        }
        assert!((f.brightness - 0.3).abs() < 0.01);
        for _ in 0..40 {
            f.update(7.0); // out-of-range duty clamps to 1
        }
        assert!(f.brightness <= 1.0 && f.brightness > 0.99);
    }

    /// Halted (no cycles accrued) falls back to the point-sample value;
    /// running divides on-cycles by total cycles per indicator.
    #[test]
    fn duties_divide_deltas_and_fall_back_when_halted() {
        let prev = LampSnapshot { bits: [0; 16], cycles: 100 };
        let mut now = LampSnapshot { bits: [0; 16], cycles: 200 };
        now.bits[0] = 100; // indicator 0 lit the whole interval
        now.bits[15] = 25; // indicator 15 lit a quarter of it
        let d = duties(&now, &prev, 0);
        assert_eq!(d[0], 1.0);
        assert_eq!(d[15], 0.25);
        assert_eq!(d[7], 0.0);

        // no cycles: the 0x8001 point sample lights indicators 0 and 15
        let d = duties(&prev, &prev, 0x8001);
        assert_eq!(d[0], 1.0);
        assert_eq!(d[15], 1.0);
        assert_eq!(d[1], 0.0);
    }
}
