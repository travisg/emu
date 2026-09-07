// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The teletype's paper: the page an ASR33 window renders.
//!
//! A [`Paper`] is the [`VideoBuffer`](super::VideoBuffer) shape -- a `Clone`
//! handle over a mutex plus a dirty flag -- shared between the CPU thread
//! (the 703's `Tty703` writes serial output into it through
//! `ConsoleEndpoint`, so `Paper` implements `Write`) and the teletype window
//! on the main thread, which redraws when the flag says the page changed.
//!
//! The byte semantics are the Model 33's own mechanism, not a terminal's:
//! carriage return and line feed are independent motions (CR returns the
//! carriage, LF advances the paper -- X-RAY's records depend on the two
//! staying distinct end to end), printing at an already-struck column
//! overstrikes, a space moves the carriage without hammering, and the
//! carriage pins at the right stop where further characters hammer in
//! place. Control characters the basic machine has no mechanism for are
//! simply not printed.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The Model 33's carriage width.
pub const PAPER_COLS: usize = 72;

/// Lines of paper kept behind the platen; the oldest tear off the top.
pub const PAPER_SCROLLBACK: usize = 500;

/// One printed page of teletype output plus the carriage position, shared
/// across the thread boundary.
#[derive(Clone, Default)]
pub struct Paper(Arc<PaperShared>);

struct PaperShared {
    inner: Mutex<PaperInner>,
    dirty: AtomicBool,
}

impl Default for PaperShared {
    fn default() -> Self {
        PaperShared {
            inner: Mutex::new(PaperInner {
                lines: VecDeque::from([[b' '; PAPER_COLS]]),
                col: 0,
            }),
            dirty: AtomicBool::new(false),
        }
    }
}

struct PaperInner {
    /// The page, oldest line first; the last entry is the line under the
    /// print head. Blank is spaces -- nothing struck.
    lines: VecDeque<[u8; PAPER_COLS]>,
    /// Carriage column, 0..=PAPER_COLS; PAPER_COLS is the right stop.
    col: usize,
}

impl Paper {
    pub fn new() -> Self {
        Self::default()
    }

    /// CPU-side: one byte of serial output reaches the print mechanism.
    pub fn feed(&self, byte: u8) {
        let mut p = self.0.inner.lock().unwrap();
        match byte {
            // CR returns the carriage; the paper does not move.
            0x0d => p.col = 0,
            // LF advances the paper; the carriage does not move.
            0x0a => {
                p.lines.push_back([b' '; PAPER_COLS]);
                while p.lines.len() > PAPER_SCROLLBACK {
                    p.lines.pop_front();
                }
                self.0.dirty.store(true, Ordering::Release);
            }
            // A space advances the carriage without hammering, so it never
            // erases what an earlier pass printed at that column.
            0x20 => p.col = (p.col + 1).min(PAPER_COLS),
            // A printing character. Overstriking is modelled as replacement
            // (the last strike shows); at the right stop the carriage stays
            // put and the hammer strikes the final column in place.
            0x21..=0x7e => {
                let col = p.col.min(PAPER_COLS - 1);
                let line = p.lines.back_mut().expect("paper always has a line");
                line[col] = byte;
                p.col = (p.col + 1).min(PAPER_COLS);
                self.0.dirty.store(true, Ordering::Release);
            }
            // Everything else is non-printing: NUL and RUBOUT are tape and
            // timing padding, and the basic Model 33 has no tab, backspace
            // or other motion mechanism to actuate. BEL rings a real bell,
            // but nothing here models sound.
            _ => {}
        }
    }

    /// Frontend-side: run `f` over the page and the carriage column, under
    /// the lock.
    pub fn with_lines<R>(&self, f: impl FnOnce(&VecDeque<[u8; PAPER_COLS]>, usize) -> R) -> R {
        let p = self.0.inner.lock().unwrap();
        f(&p.lines, p.col)
    }

    /// Frontend-side: consume the dirty flag. True if a redraw is due.
    pub fn take_dirty(&self) -> bool {
        self.0.dirty.swap(false, Ordering::AcqRel)
    }
}

/// The Model 33 prints from a 64-glyph typebox: the whole 0x60-0x7E column
/// folds down onto 0x40-0x5E (a->A, {->[, `->@), so stored bytes stay raw
/// and the fold happens where the hammer meets the paper -- at render time.
pub fn fold_to_typebox(c: u8) -> u8 {
    if (0x60..=0x7e).contains(&c) {
        c - 0x20
    } else {
        c
    }
}

/// `Write` is how the byte stream arrives: a `Paper` clone is the
/// `ConsoleEndpoint`'s output sink when the teletype window is up.
impl Write for Paper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &b in buf {
            self.feed(b);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_str(p: &Paper, i: usize) -> String {
        p.with_lines(|lines, _| String::from_utf8_lossy(&lines[i]).trim_end().to_string())
    }

    #[test]
    fn cr_returns_the_carriage_without_advancing_paper() {
        let p = Paper::new();
        for &b in b"AB\rC" {
            p.feed(b);
        }
        p.with_lines(|lines, col| {
            assert_eq!(lines.len(), 1);
            assert_eq!(col, 1);
        });
        assert_eq!(line_str(&p, 0), "CB");
    }

    #[test]
    fn lf_advances_paper_without_moving_the_carriage() {
        let p = Paper::new();
        for &b in b"AB\nC" {
            p.feed(b);
        }
        p.with_lines(|lines, col| {
            assert_eq!(lines.len(), 2);
            assert_eq!(col, 3);
        });
        assert_eq!(line_str(&p, 0), "AB");
        assert_eq!(line_str(&p, 1), "  C");
    }

    #[test]
    fn overstrike_replaces_at_the_same_column() {
        let p = Paper::new();
        for &b in b"A\rB" {
            p.feed(b);
        }
        assert_eq!(line_str(&p, 0), "B");
    }

    #[test]
    fn a_space_advances_the_carriage_without_erasing() {
        let p = Paper::new();
        for &b in b"AB\r " {
            p.feed(b);
        }
        p.with_lines(|_, col| assert_eq!(col, 1));
        assert_eq!(line_str(&p, 0), "AB");
    }

    #[test]
    fn the_carriage_pins_at_the_right_stop_and_hammers_in_place() {
        let p = Paper::new();
        for _ in 0..PAPER_COLS {
            p.feed(b'A');
        }
        p.feed(b'B');
        p.feed(b'C');
        p.with_lines(|lines, col| {
            assert_eq!(col, PAPER_COLS);
            assert_eq!(lines[0][PAPER_COLS - 1], b'C');
            assert_eq!(lines[0][PAPER_COLS - 2], b'A');
        });
    }

    #[test]
    fn controls_are_non_printing() {
        let p = Paper::new();
        for &b in &[0x00u8, 0x07, 0x08, 0x09, 0x7f, 0x1b] {
            p.feed(b);
        }
        p.with_lines(|lines, col| {
            assert_eq!(lines.len(), 1);
            assert_eq!(col, 0);
        });
        assert_eq!(line_str(&p, 0), "");
        assert!(!p.take_dirty(), "nothing visible changed");
    }

    #[test]
    fn scrollback_is_capped() {
        let p = Paper::new();
        for _ in 0..(PAPER_SCROLLBACK + 50) {
            p.feed(b'X');
            p.feed(0x0a);
            p.feed(0x0d);
        }
        p.with_lines(|lines, _| assert_eq!(lines.len(), PAPER_SCROLLBACK));
    }

    #[test]
    fn printing_sets_the_dirty_flag() {
        let p = Paper::new();
        assert!(!p.take_dirty());
        p.feed(b'A');
        assert!(p.take_dirty());
        assert!(!p.take_dirty(), "take consumes the flag");
        p.feed(0x0a);
        assert!(p.take_dirty(), "a line feed is a visible change");
        p.feed(0x0d);
        assert!(!p.take_dirty(), "a carriage return is not");
    }

    #[test]
    fn fold_maps_the_lower_range_onto_the_typebox() {
        assert_eq!(fold_to_typebox(b'a'), b'A');
        assert_eq!(fold_to_typebox(b'z'), b'Z');
        assert_eq!(fold_to_typebox(b'{'), b'[');
        assert_eq!(fold_to_typebox(b'`'), b'@');
        assert_eq!(fold_to_typebox(b'~'), b'^');
        assert_eq!(fold_to_typebox(b'A'), b'A');
        assert_eq!(fold_to_typebox(b'_'), b'_');
    }
}
