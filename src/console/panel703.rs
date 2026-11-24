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
//! Below the console sits the disc rack: one bay per 74601 unit with
//! ONLINE, READ and WRITE lamps, fed by the controller through
//! [`DiscRackState`]. The styling is invented -- no Raytheon document
//! describes the disc cabinet -- but the bulbs obey the same thermal
//! model as the console's own.
//!
//! The panel itself is [`PanelView`], which knows nothing of SDL: it draws
//! on any [`Surface`] and takes clicks and keys in its own pixels. The SDL
//! window around it, [`PanelWindow`], is one window of the 703's frontend:
//! [`super::ray703`] owns the SDL context and the event loop and routes
//! this window's events here. The web page is the other host, drawing the
//! view into a [`super::surface::Framebuffer`]. Keystrokes into the panel
//! are deliberately *not* forwarded to the guest -- the real panel has no
//! keyboard, and a second keyboard would ride the host's text-input CR/LF
//! conventions rather than the raw mode whose preserved CR-versus-LF
//! distinction the 703's software depends on.
//!
//! Everything is drawn with filled rectangles -- circles as stacks of
//! horizontal spans, text from the 5x7 font in [`super::lamps`] -- so there
//! are no textures, image files or font dependencies.
//!
//! The lamps' thermal filter and the rack's completion wink advance once
//! per [`PanelView::render`], and are tuned for sixty renders a second.

use super::{
    DiscRackState, LampSnapshot, PanelCommand, PanelControl, PanelDisplay, PanelState, RackSnapshot,
    Selector,
};
use super::lamps::{self, duties};
use super::surface::{Color, Surface};
use crate::console::RACK_UNITS;

const WINDOW_W: u32 = 1120;
const WINDOW_H: u32 = 480;

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

/// The disc rack band below the console: caption, then one bay per unit
/// with its three bulbs. Geometry is invented, tuned by eye like the rest
/// of the file.
const RACK_CAPTION_Y: i32 = 318;
const RACK_BAY_Y: i32 = 358;
const RACK_BAY_H: u32 = 74;
const RACK_LAMP_Y: i32 = 388;
const RACK_LAMP_R: i32 = 9;
const RACK_LAMP_PITCH: i32 = 68;
/// How many frames a completion wink holds the lamp at full duty -- about
/// 100 ms, the least an operator could see. This is what makes a
/// --fast-io transfer visible at all: it charges no cycles.
const RACK_BLIP_FRAMES: u8 = 6;

fn rack_bay_x(unit: usize) -> i32 {
    170 + unit as i32 * 254
}

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

/// The front panel and the disc rack: their state, their drawing and
/// their hit-testing, on no particular host.
pub struct PanelView {
    panel: PanelState,
    /// Switch actuations to the run loop. Send errors are ignored
    /// throughout: a dead CPU thread means shutdown is already in flight.
    control: PanelControl,
    rack: DiscRackState,
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
    /// The rack lamps run the same snapshot-diff-filter pipeline, plus the
    /// completion-wink counters.
    prev_rack: RackSnapshot,
    read_filters: [LampFilter; RACK_UNITS],
    write_filters: [LampFilter; RACK_UNITS],
    read_blip: [u8; RACK_UNITS],
    write_blip: [u8; RACK_UNITS],
}

impl PanelView {
    /// The view's size in pixels.
    pub const WIDTH: u32 = WINDOW_W;
    pub const HEIGHT: u32 = WINDOW_H;

    pub fn new(display: PanelDisplay) -> Self {
        let PanelDisplay { panel, control, rack } = display;
        // MB is the boot-procedure position ("turn the display selector to
        // MB before following the operating procedure" -- the PTB drawing),
        // and it is also the busiest lamp row on an idle machine.
        PanelView {
            panel,
            control,
            rack,
            selector: Selector::Mb,
            prev_pc: LampSnapshot::default(),
            prev_sel: [LampSnapshot::default(); 6],
            pc_filters: [LampFilter::default(); 16],
            sd_filters: [LampFilter::default(); 16],
            prev_rack: RackSnapshot::default(),
            read_filters: [LampFilter::default(); RACK_UNITS],
            write_filters: [LampFilter::default(); RACK_UNITS],
            read_blip: [0; RACK_UNITS],
            write_blip: [0; RACK_UNITS],
        }
    }

    fn cycle_selector(&mut self, dir: i32) {
        let i = SELECTOR_ORDER.iter().position(|&s| s == self.selector).unwrap() as i32;
        self.selector = SELECTOR_ORDER[(i + dir).rem_euclid(6) as usize];
    }

    // -- drawing -----------------------------------------------------------

    /// A caption between two rules, like the row headings in figure 5-1.
    fn caption(s: &mut dyn Surface, y: i32, label: &str) {
        let w = (label.chars().count() as i32 * 6 - 1) * 2;
        let (left, right) = (lamp_x(0) - LAMP_R, lamp_x(15) + LAMP_R);
        let cx = (left + right) / 2;
        s.fill_rect(left, y + 6, (cx - w / 2 - 8 - left) as u32, 1, PANEL_INK);
        s.fill_rect(cx + w / 2 + 8, y + 6, (right - cx - w / 2 - 8) as u32, 1, PANEL_INK);
        lamps::text_centered(s, cx, y, 2, label, PANEL_INK);
    }

    /// One row of lamps at the given brightnesses, indicator 0 (the MSB, as
    /// everywhere on this machine) leftmost. `first` skips leading
    /// indicators: the PROGRAM COUNTER row starts at 1 because the PCR is
    /// 15 bits.
    fn lamp_row(s: &mut dyn Surface, y: i32, brightness: &[f32; 16], first: i32) {
        for i in first..16 {
            let x = lamp_x(i);
            let t = perceived(brightness[i as usize]);
            // a bright bulb glows past its bezel
            if t > 0.25 {
                let alpha = (90.0 * (t - 0.25) / 0.75) as u8;
                let glow = Color::RGBA(LAMP_GLOW.r, LAMP_GLOW.g, LAMP_GLOW.b, alpha);
                lamps::circle(s, x, y, LAMP_R + 7, glow);
            }
            lamps::circle(s, x, y, LAMP_R + 3, LAMP_BEZEL);
            lamps::circle(s, x, y, LAMP_R, lamp_color(t));
            let label = i.to_string();
            lamps::text_centered(s, x, y - LAMP_R - 15, 1, &label, PANEL_INK);
        }
    }

    fn draw_buttons(&self, s: &mut dyn Surface) {
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
                    lamps::circle(s, x, CONTROLS_Y, BUTTON_R + 7, glow);
                    HALT_LIT
                } else {
                    HALT_UNLIT
                }
            } else {
                Color::RGB(0x8a, 0x8a, 0x84)
            };
            lamps::circle(s, x, CONTROLS_Y, BUTTON_R + 2, LAMP_BEZEL);
            lamps::circle(s, x, CONTROLS_Y, BUTTON_R, face);
            lamps::text_centered(s, x, CONTROLS_Y - BUTTON_R - 20, 1, label, PANEL_INK);
        }

        // ENTER and DISPLAY are switch-indicators, illuminated when the
        // selector sits on MB (5-3) -- the position their workflow serves.
        let lit = self.selector == Selector::Mb;
        for (x, label) in [(ENTER_X, "ENTER"), (DISPLAY_X, "DISPLAY")] {
            lamps::circle(s, x, CONTROLS_Y, BUTTON_R + 2, LAMP_BEZEL);
            lamps::circle(s, x, CONTROLS_Y, BUTTON_R, if lit { LAMP_LIT } else { LAMP_UNLIT });
            lamps::text_centered(s, x, CONTROLS_Y - BUTTON_R - 20, 1, label, PANEL_INK);
        }

        // one CLEAR at the head of each lamp row
        for y in [PC_LAMPS_Y, SD_LAMPS_Y] {
            lamps::circle(s, CLEAR_X, y, CLEAR_R + 2, LAMP_BEZEL);
            lamps::circle(s, CLEAR_X, y, CLEAR_R, Color::RGB(0x8a, 0x8a, 0x84));
            lamps::text_centered(s, CLEAR_X, y - LAMP_R - 15, 1, "CLEAR", PANEL_INK);
        }
    }

    fn draw_toggles(&self, s: &mut dyn Surface) {
        // a local caption over just the four toggles -- the full-width rule
        // would run through the button labels sharing this row
        let cx = (toggle_x(0) + toggle_x(3)) / 2;
        lamps::text_centered(s, cx, CONTROLS_Y - TOGGLE_H as i32 / 2 - 34, 1, "SENSE", PANEL_INK);
        s.fill_rect(toggle_x(0) - 12, CONTROLS_Y - TOGGLE_H as i32 / 2 - 31, (cx - 22 - toggle_x(0) + 12) as u32, 1, PANEL_INK);
        s.fill_rect(cx + 22, CONTROLS_Y - TOGGLE_H as i32 / 2 - 31, (toggle_x(3) + 12 - cx - 22) as u32, 1, PANEL_INK);
        for i in 0..4 {
            let x = toggle_x(i);
            let up = self.panel.sense(i as u8);
            s.fill_rect(
                x - TOGGLE_W as i32 / 2,
                CONTROLS_Y - TOGGLE_H as i32 / 2,
                TOGGLE_W,
                TOGGLE_H,
                LAMP_BEZEL,
            );
            let ky = if up { CONTROLS_Y - TOGGLE_H as i32 / 2 } else { CONTROLS_Y + TOGGLE_H as i32 / 2 };
            lamps::circle(s, x, ky, 10, PANEL_INK);
            lamps::circle(s, x, ky, 8, Color::RGB(0x90, 0x90, 0x90));
            lamps::text_centered(s, x, CONTROLS_Y - TOGGLE_H as i32 / 2 - 24, 1, &i.to_string(), PANEL_INK);
        }
    }

    fn draw_selector(&self, s: &mut dyn Surface) {
        lamps::text_centered(s, KNOB_X, KNOB_Y - KNOB_R - 42, 1, "DISPLAY", PANEL_INK);
        lamps::text_centered(s, KNOB_X, KNOB_Y - KNOB_R - 32, 1, "SELECTOR", PANEL_INK);
        lamps::circle(s, KNOB_X, KNOB_Y, KNOB_R + 3, LAMP_BEZEL);
        lamps::circle(s, KNOB_X, KNOB_Y, KNOB_R, Color::RGB(0x30, 0x30, 0x30));
        for (i, name) in SELECTOR_NAMES.iter().enumerate() {
            let a = selector_angle(i);
            let lx = KNOB_X + ((KNOB_R + 18) as f32 * a.cos()) as i32;
            let ly = KNOB_Y - ((KNOB_R + 18) as f32 * a.sin()) as i32;
            lamps::text_centered(s, lx, ly - 3, 1, name, PANEL_INK);
        }
        // the pointer: a pale dot on the knob's rim at the active position
        let i = SELECTOR_ORDER.iter().position(|&s| s == self.selector).unwrap();
        let a = selector_angle(i);
        let px = KNOB_X + ((KNOB_R - 8) as f32 * a.cos()) as i32;
        let py = KNOB_Y - ((KNOB_R - 8) as f32 * a.sin()) as i32;
        lamps::circle(s, px, py, 4, Color::RGB(0xe8, 0xe8, 0xe8));
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

    /// Advance the rack lamps one frame: READ/WRITE duty from the cycle
    /// accumulators, overridden to full for a few frames whenever a
    /// completion counter moved (the `--fast-io` wink -- such a transfer
    /// charges no cycles at all).
    fn update_rack(&mut self) -> ([f32; RACK_UNITS], [f32; RACK_UNITS]) {
        let now = self.rack.snapshot();
        let dc = now.cycles.wrapping_sub(self.prev_rack.cycles);
        let mut read = [0.0f32; RACK_UNITS];
        let mut write = [0.0f32; RACK_UNITS];
        for u in 0..RACK_UNITS {
            if now.read_ops[u] != self.prev_rack.read_ops[u] {
                self.read_blip[u] = RACK_BLIP_FRAMES;
            }
            if now.write_ops[u] != self.prev_rack.write_ops[u] {
                self.write_blip[u] = RACK_BLIP_FRAMES;
            }
            let duty = |on: u64, prev: u64, blip: &mut u8| {
                if *blip > 0 {
                    *blip -= 1;
                    1.0
                } else if dc == 0 {
                    // halted, or no controller poll this frame: nothing
                    // in flight, the lamp cools
                    0.0
                } else {
                    on.wrapping_sub(prev) as f32 / dc as f32
                }
            };
            let rd = duty(now.read_on[u], self.prev_rack.read_on[u], &mut self.read_blip[u]);
            let wd = duty(now.write_on[u], self.prev_rack.write_on[u], &mut self.write_blip[u]);
            read[u] = self.read_filters[u].update(rd);
            write[u] = self.write_filters[u].update(wd);
        }
        self.prev_rack = now;
        (read, write)
    }

    /// The invented disc cabinet: a captioned band of four bays, each a
    /// thin bezel with the unit number and its three bulbs.
    fn draw_rack(&mut self, s: &mut dyn Surface) {
        let (read, write) = self.update_rack();
        let online = self.prev_rack.online;

        Self::caption(s, RACK_CAPTION_Y, "74601 DISC MEMORY");
        for u in 0..RACK_UNITS {
            let cx = rack_bay_x(u);
            let (left, w) = (cx - RACK_LAMP_PITCH - 44, (2 * RACK_LAMP_PITCH + 88) as u32);
            for (bx, by, bw, bh) in [
                (left, RACK_BAY_Y, w, 1u32),
                (left, RACK_BAY_Y + RACK_BAY_H as i32, w, 1),
                (left, RACK_BAY_Y, 1, RACK_BAY_H),
                (left + w as i32, RACK_BAY_Y, 1, RACK_BAY_H + 1),
            ] {
                s.fill_rect(bx, by, bw, bh, LAMP_BEZEL);
            }
            lamps::text_centered(s, cx, RACK_BAY_Y + 7, 1, &format!("UNIT {u}"), PANEL_INK);
            for (i, (label, t)) in [
                // ONLINE is a steady state, not a duty: spindles don't flicker
                ("ONLINE", if online[u] { 1.0 } else { 0.0 }),
                ("READ", read[u]),
                ("WRITE", write[u]),
            ]
            .into_iter()
            .enumerate()
            {
                let x = cx + (i as i32 - 1) * RACK_LAMP_PITCH;
                let t = perceived(t);
                if t > 0.25 {
                    let alpha = (90.0 * (t - 0.25) / 0.75) as u8;
                    let glow = Color::RGBA(LAMP_GLOW.r, LAMP_GLOW.g, LAMP_GLOW.b, alpha);
                    lamps::circle(s, x, RACK_LAMP_Y, RACK_LAMP_R + 5, glow);
                }
                lamps::circle(s, x, RACK_LAMP_Y, RACK_LAMP_R + 3, LAMP_BEZEL);
                lamps::circle(s, x, RACK_LAMP_Y, RACK_LAMP_R, lamp_color(t));
                lamps::text_centered(s, x, RACK_LAMP_Y + RACK_LAMP_R + 8, 1, label, PANEL_INK);
            }
        }
    }

    /// Draw a frame, advancing the lamps' filters by one.
    pub fn render(&mut self, s: &mut dyn Surface) {
        let (pc, sd) = self.update_lamps();

        s.clear(PANEL_BG);

        Self::caption(s, PC_CAPTION_Y, "PROGRAM COUNTER");
        Self::lamp_row(s, PC_LAMPS_Y, &pc, 1);

        Self::caption(s, SD_CAPTION_Y, "SELECTED DISPLAY");
        Self::lamp_row(s, SD_LAMPS_Y, &sd, 0);

        self.draw_buttons(s);
        self.draw_toggles(s);
        self.draw_selector(s);
        self.draw_rack(s);

        lamps::text(s, LAMPS_X - LAMP_R, WINDOW_H as i32 - 34, 2, "703  CENTRAL PROCESSOR", PANEL_INK);
        lamps::text_centered(s, 
            KNOB_X,
            WINDOW_H as i32 - 34,
            2,
            "RAYTHEON",
            Color::RGB(0x8a, 0x1f, 0x1f),
        );
    }

    // -- input -------------------------------------------------------------

    /// A click at (x, y) in the view's pixels; `back` is the other mouse
    /// button, which turns the knob the other way.
    pub fn click(&mut self, x: i32, y: i32, back: bool) {
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
            self.cycle_selector(if back { -1 } else { 1 });
        }
    }

    /// A key pressed at the panel: Tab turns the knob, 1 to 4 flip the
    /// SENSE toggles. Everything else is deliberately ignored -- the
    /// guest's keyboard is the teletype, not this panel.
    pub fn key(&mut self, key: char) {
        match key {
            '\t' => self.cycle_selector(1),
            '1'..='4' => self.panel.toggle_sense(key as u8 - b'1'),
            _ => {}
        }
    }
}

/// The panel in an SDL window of its own.
#[cfg(not(target_arch = "wasm32"))]
mod window {
    use super::{PanelDisplay, PanelView, WINDOW_H, WINDOW_W};
    use sdl2::keyboard::Keycode;
    use sdl2::mouse::MouseButton;
    use sdl2::render::Canvas;
    use sdl2::video::Window;

    pub(crate) struct PanelWindow {
        id: u32,
        view: PanelView,
        canvas: Canvas<Window>,
    }

    impl PanelWindow {
        pub(crate) fn new(
            video: &sdl2::VideoSubsystem,
            title: &str,
            display: PanelDisplay,
        ) -> Result<Self, String> {
            let window = video
                .window(title, WINDOW_W, WINDOW_H)
                .position_centered()
                .build()
                .map_err(|e| e.to_string())?;
            let mut canvas = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
            // the lamp glow halos draw with alpha
            canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
            Ok(PanelWindow { id: canvas.window().id(), view: PanelView::new(display), canvas })
        }

        /// The SDL window id the frontend routes events by.
        pub(crate) fn window_id(&self) -> u32 {
            self.id
        }

        pub(crate) fn render(&mut self) {
            self.view.render(&mut self.canvas);
            self.canvas.present();
        }

        pub(crate) fn click(&mut self, x: i32, y: i32, button: MouseButton) {
            self.view.click(x, y, button == MouseButton::Right);
        }

        /// A key pressed with this window focused. Ctrl-D is the
        /// frontend's business and never arrives here.
        pub(crate) fn key_down(&mut self, key: Keycode) {
            let c = match key {
                Keycode::Tab => '\t',
                Keycode::Num1 => '1',
                Keycode::Num2 => '2',
                Keycode::Num3 => '3',
                Keycode::Num4 => '4',
                _ => return,
            };
            self.view.key(c);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use window::PanelWindow;

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

    /// The view draws the same pixels on a framebuffer as on SDL's
    /// software renderer, glows and all, which is what lets the web page
    /// show the panel the window shows. Frames with lamps at part duty, the
    /// HALT lens lit and not, the knob turned and the rack winking.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_framebuffer_draws_what_sdl_draws() {
        use crate::console::surface::Framebuffer;
        use crate::console::{DiscRackState, PanelControl};
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        std::env::set_var("SDL_RENDER_DRIVER", "software");
        let sdl = sdl2::init().unwrap();
        let window = sdl.video().unwrap().window("panel", WINDOW_W, WINDOW_H).build().unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let mut fb = Framebuffer::new(WINDOW_W, WINDOW_H);

        // two views over one machine, so both see the same accumulators
        let (panel, rack) = (PanelState::new(), DiscRackState::new());
        let (tx, _rx) = std::sync::mpsc::channel();
        let display = || PanelDisplay { panel: panel.clone(), control: PanelControl(tx.clone()), rack: rack.clone() };
        let (mut on_sdl, mut on_fb) = (PanelView::new(display()), PanelView::new(display()));
        rack.set_online(0, true);
        for (frame, pcr) in [0x1234u16, 0x04c2, 0x7fff, 0x2468].into_iter().enumerate() {
            panel.set_registers(pcr, !pcr, pcr >> 3, 0x8421);
            panel.set_memory(pcr << 1, pcr ^ 0x5a5a);
            panel.accumulate(300);
            panel.set_registers(pcr ^ 0x0f0f, pcr, !pcr, 0x0001);
            panel.accumulate(700);
            panel.set_halted(frame % 2 == 0);
            match frame {
                1 => {
                    rack.charge(0, false, 500);
                    rack.advance(1000);
                    panel.toggle_sense(1);
                }
                2 => rack.complete(0, true),
                _ => {}
            }
            for v in [&mut on_sdl, &mut on_fb] {
                v.key('\t');
            }
            on_sdl.render(&mut canvas);
            on_fb.render(&mut fb);
            let sdl_px = canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGB24).unwrap();
            let fb_px: Vec<u8> = fb.pixels.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
            assert_eq!(sdl_px.len(), fb_px.len());
            let differ = sdl_px.chunks(3).zip(fb_px.chunks(3)).filter(|(a, b)| a != b).count();
            assert_eq!(differ, 0, "frame {frame}: {differ} pixels differ");
            // and the frame is a panel, not two blank surfaces agreeing --
            // by the last, with the bulbs warmed up, glows and all
            let colors = fb_px.chunks(3).collect::<std::collections::HashSet<_>>().len();
            assert!(colors > if frame == 3 { 40 } else { 10 }, "frame {frame}: only {colors} colors");
        }
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
}
