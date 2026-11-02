// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! What the front panel windows share: the duty cycle a lamp shows over a
//! frame, from the monotonic accumulators a core feeds, and drawing with
//! filled rectangles alone -- circles as stacks of horizontal spans, text
//! from a 5x7 font -- so no panel needs a texture, an image file or a font.
//!
//! How a duty looks is each panel's own: the 703's lamps are filaments
//! with a thermal lag, the Altair 8800's are LEDs with none.

use super::LampSnapshot;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

/// Per-indicator duty cycles between two snapshots of one lamp source.
/// While the machine is halted no cycles accrue, and the point-sampled
/// register value stands in -- full on or off, which is what a static
/// register looks like on a real panel.
pub(crate) fn duties(now: &LampSnapshot, prev: &LampSnapshot, halted_value: u16) -> [f32; 16] {
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

/// A filled circle as one fill_rect span per scanline.
pub(crate) fn circle(canvas: &mut Canvas<Window>, cx: i32, cy: i32, r: i32, color: Color) {
    canvas.set_draw_color(color);
    for dy in -r..=r {
        let half = ((r * r - dy * dy) as f32).sqrt() as i32;
        let _ = canvas.fill_rect(Rect::new(cx - half, cy + dy, (2 * half + 1) as u32, 1));
    }
}

/// 5x7 text, scaled up `scale` times. Unknown characters draw as space.
pub(crate) fn text(canvas: &mut Canvas<Window>, x: i32, y: i32, scale: u32, s: &str, color: Color) {
    canvas.set_draw_color(color);
    let mut cx = x;
    for c in s.chars() {
        if let Some(glyph) = glyph(c) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        let _ = canvas.fill_rect(Rect::new(
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

/// How wide `text` draws `s`.
pub(crate) fn text_width(scale: u32, s: &str) -> i32 {
    (s.chars().count() as i32 * 6 - 1) * scale as i32
}

/// Text centered on `cx`.
pub(crate) fn text_centered(canvas: &mut Canvas<Window>, cx: i32, y: i32, scale: u32, s: &str, color: Color) {
    text(canvas, cx - text_width(scale, s) / 2, y, scale, s, color);
}

/// A 5x7 font covering the panels' labels: digits, upper-case letters, the
/// dash. Each row is one byte, bit 4 the leftmost pixel.
pub(crate) fn glyph(c: char) -> Option<[u8; 7]> {
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

    #[test]
    fn the_font_covers_the_labels() {
        for c in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-".chars() {
            assert!(glyph(c).is_some(), "{c}");
        }
        assert!(glyph(' ').is_none(), "a space draws nothing");
    }
}
