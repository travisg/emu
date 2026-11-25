// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! A teletype's [`Paper`] as it is seen: which lines of the roll show, and
//! where each typewheel glyph lands in pixels -- 72 columns on a page of
//! newsprint, the last lines visible and the platen rolled back through
//! the scrollback on request. The SDL window ([`super::tty703`]) prints
//! the glyphs from a texture; [`PaperView::render`] prints them into a
//! [`Framebuffer`] from the same atlas, which is how the web page shows
//! the paper.

use super::paper::{fold_to_typebox, Paper, PAPER_COLS};
use super::surface::{Color, Framebuffer, Surface};
use super::tty33_font as font;

/// Lines of paper above the platen.
const VISIBLE_LINES: usize = 25;
/// Line pitch in pixels; the glyph cell is shorter, the rest is the
/// Model 33's generous single-space leading.
const LINE_PITCH: i32 = 20;
const MARGIN: i32 = 24;

/// Teletype paper: not white -- a warm newsprint.
pub const PAPER_BG: Color = Color::RGB(0xf2, 0xee, 0xe2);
/// The ribbon's ink, a soft black.
pub const INK: Color = Color::RGB(0x30, 0x2c, 0x28);

pub struct PaperView {
    paper: Paper,
    /// Lines rolled back from the live end of the paper.
    scroll: usize,
    /// The paper's dirty flag covers printing; this covers everything else
    /// (the first frame, a scroll). Starts true to paint the blank page.
    needs_redraw: bool,
}

impl PaperView {
    /// The view's size in pixels.
    pub const WIDTH: u32 = ((PAPER_COLS - 1) * font::ADVANCE + font::GLYPH_W) as u32 + 2 * MARGIN as u32;
    pub const HEIGHT: u32 = VISIBLE_LINES as u32 * LINE_PITCH as u32 + 2 * MARGIN as u32;

    pub fn new(paper: Paper) -> Self {
        PaperView { paper, scroll: 0, needs_redraw: true }
    }

    /// Whether the page wants drawing again: it changed under the print
    /// head (which snaps the view back to the live end -- the platen
    /// moved), or the view itself did.
    pub fn take_redraw(&mut self) -> bool {
        if self.paper.take_dirty() {
            self.scroll = 0;
            self.needs_redraw = true;
        }
        std::mem::take(&mut self.needs_redraw)
    }

    /// Where `dy` notches would leave the platen: up (positive) is back
    /// into the scrollback, three lines a notch, pinned at either end.
    fn rolled(&self, dy: i32) -> usize {
        let max = self.paper.with_lines(|l, _| l.len().saturating_sub(VISIBLE_LINES));
        (self.scroll as i64 + dy as i64 * 3).clamp(0, max as i64) as usize
    }

    /// Roll the platen `dy` notches; whether it moved, which a host whose
    /// page scrolls too uses to hand the wheel on at either end.
    pub fn wheel(&mut self, dy: i32) -> bool {
        let s = self.rolled(dy);
        let moved = s != self.scroll;
        if moved {
            self.scroll = s;
            self.needs_redraw = true;
        }
        moved
    }

    /// Whether a roll in `dy`'s direction would move the platen at all.
    pub fn can_roll(&self, dy: i32) -> bool {
        self.rolled(dy.signum()) != self.scroll
    }

    /// Each glyph showing, as its index in the atlas and the top left of
    /// its cell.
    pub fn glyphs(&self, mut f: impl FnMut(usize, i32, i32)) {
        self.paper.with_lines(|lines, _col| {
            // A part-filled page reads from the top, as a fresh roll does;
            // once full, the last VISIBLE_LINES (less the scrollback
            // offset) are shown.
            let end = lines.len() - self.scroll.min(lines.len().saturating_sub(VISIBLE_LINES));
            let start = end.saturating_sub(VISIBLE_LINES);
            for (row, line) in lines.iter().skip(start).take(end - start).enumerate() {
                let y = MARGIN + row as i32 * LINE_PITCH;
                for (col, &c) in line.iter().enumerate() {
                    if c != b' ' {
                        let g = (fold_to_typebox(c) - font::FIRST_CHAR) as usize;
                        f(g, MARGIN + (col * font::ADVANCE) as i32, y);
                    }
                }
            }
        });
    }

    /// The page, printed into `fb`, which is [`Self::WIDTH`] by
    /// [`Self::HEIGHT`].
    pub fn render(&self, fb: &mut Framebuffer) {
        fb.clear(PAPER_BG);
        let cell = font::GLYPH_W * font::GLYPH_H;
        self.glyphs(|g, x, y| fb.mask(x, y, &font::ATLAS[g * cell..(g + 1) * cell], font::GLYPH_W, INK));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(fb: &Framebuffer) -> usize {
        fb.pixels.as_chunks::<4>().0.iter().filter(|p| p[..3] != [PAPER_BG.r, PAPER_BG.g, PAPER_BG.b]).count()
    }

    #[test]
    fn printing_dirties_the_page_and_a_scroll_snaps_back() {
        let paper = Paper::new();
        let mut v = PaperView::new(paper.clone());
        assert!(v.take_redraw(), "the blank page paints once");
        assert!(!v.take_redraw());
        for _ in 0..40 {
            for b in *b"LINE\r\n" {
                paper.feed(b);
            }
        }
        assert!(v.take_redraw());
        assert!(!v.can_roll(-1) && !v.wheel(-1), "already at the live end");
        assert!(v.can_roll(1) && v.wheel(1));
        assert!(v.take_redraw(), "a roll back redraws");
        assert_eq!(v.scroll, 3);
        paper.feed(b'X');
        assert!(v.take_redraw());
        assert_eq!(v.scroll, 0, "the print head brings the paper back");
    }

    /// A page shorter than the window has nowhere to roll, and a full one
    /// stops at its first line: either way the wheel says it did not move.
    #[test]
    fn the_platen_stops_at_both_ends() {
        let paper = Paper::new();
        let mut v = PaperView::new(paper.clone());
        for b in *b"SHORT\r\n" {
            paper.feed(b);
        }
        assert!(!v.can_roll(1) && !v.wheel(1), "less than a window of paper");
        for _ in 0..VISIBLE_LINES + 4 {
            for b in *b"LINE\r\n" {
                paper.feed(b);
            }
        }
        let max = paper.with_lines(|l, _| l.len()) - VISIBLE_LINES;
        assert!(v.wheel(100));
        assert_eq!(v.scroll, max, "pinned at the first line");
        assert!(!v.can_roll(1) && !v.wheel(1));
        assert!(v.can_roll(-1));
    }

    #[test]
    fn the_page_prints_where_the_glyphs_land() {
        let paper = Paper::new();
        let v = PaperView::new(paper.clone());
        let mut fb = Framebuffer::new(PaperView::WIDTH, PaperView::HEIGHT);
        v.render(&mut fb);
        assert_eq!(ink(&fb), 0, "a blank page");
        for b in *b"HI\r\n" {
            paper.feed(b);
        }
        let mut cells = Vec::new();
        v.glyphs(|g, x, y| cells.push((g, x, y)));
        let first = MARGIN + font::ADVANCE as i32;
        assert_eq!(cells, [((b'H' - b' ') as usize, MARGIN, MARGIN), ((b'I' - b' ') as usize, first, MARGIN)]);
        v.render(&mut fb);
        assert!(ink(&fb) > 20, "two letters of ink");
    }
}
