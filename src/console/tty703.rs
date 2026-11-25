// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The 703's teletype window: an ASR33's roll of paper.
//!
//! One window of [`super::ray703`]'s frontend, drawing the [`Paper`] the
//! machine's serial output feeds as its [`PaperView`] lays it out -- 72
//! columns of the Model 33 typewheel on a paper-colored page, the last
//! lines visible and the mouse wheel rolling the platen back through the
//! scrollback. Glyphs come from the
//! atlas in [`tty33_font`], rasterized offline from a photograph-derived
//! recreation of the real typewheel -- the one deliberate font asset in a
//! tree that otherwise draws its text from hand-drawn tables (the panel's
//! 5x7 face reads as panel engraving; paper needs the printed letterform).
//!
//! The keyboard maps like the Kaypro window's, with the Model 33's own
//! conventions on top: letters upcase (the keyboard has no lowercase),
//! Return is CARRIAGE RETURN, shift-Return the LINE FEED key (X-RAY's
//! Ctrl-J convention still works too), and Backspace/Delete send RUBOUT --
//! the machine has no backspace. CR and LF stay distinct end to end, which
//! is the property the 703's software cannot live without.

use super::paper_view::{PaperView, INK, PAPER_BG};
use super::sdl::control_code;
use super::tty33_font as font;
use super::TtyDisplay;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext, WindowPos};
use std::sync::mpsc::Sender;

/// Glyphs per row in the texture atlas grid (64 glyphs, 8x8).
const ATLAS_PER_ROW: usize = 8;

pub(crate) struct TtyWindow {
    id: u32,
    /// Keystrokes to the machine -- the same channel the terminal feeds.
    tx: Sender<u8>,
    view: PaperView,
    canvas: Canvas<Window>,
}

impl TtyWindow {
    /// `console_keys` is the console's keyboard channel, which the
    /// console's teletype takes; another terminal's comes with its display.
    pub(crate) fn new(
        video: &sdl2::VideoSubsystem,
        title: &str,
        display: TtyDisplay,
        console_keys: Sender<u8>,
    ) -> Result<Self, String> {
        let (name, n, tx) = match display.line {
            None => (format!("{title} Teletype"), 0, console_keys),
            Some((n, keys)) => (format!("{title} Teletype {n}"), n, keys),
        };
        let mut window =
            video.window(&name, PaperView::WIDTH, PaperView::HEIGHT).position_centered().build().map_err(|e| e.to_string())?;
        // each terminal's paper down and across from the last, so none
        // lands squarely on another
        if n > 0 {
            let (x, y) = window.position();
            let step = 32 * n as i32;
            window.set_position(WindowPos::Positioned(x + step), WindowPos::Positioned(y + step));
        }
        let canvas = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
        Ok(TtyWindow { id: canvas.window().id(), tx, view: PaperView::new(display.paper), canvas })
    }

    /// The SDL window id the frontend routes events by.
    pub(crate) fn window_id(&self) -> u32 {
        self.id
    }

    /// The texture creator the font texture must be built against; owned by
    /// the caller because a `Texture` borrows its creator, so neither can
    /// live in this struct (the same shape as `SdlFrontend`'s atlas).
    pub(crate) fn texture_creator(&self) -> TextureCreator<WindowContext> {
        self.canvas.texture_creator()
    }

    /// The typewheel atlas as a texture: white glyphs with the rasterizer's
    /// alpha, tinted to ink at draw time with `set_color_mod`. RGBA32 (not
    /// RGBA8888) because the channels differ here, and RGBA32 is the
    /// byte-order format -- RGBA8888 is packed, so its memory layout flips
    /// with endianness.
    pub(crate) fn build_font<'a>(
        &self,
        creator: &'a TextureCreator<WindowContext>,
    ) -> Result<Texture<'a>, String> {
        let (tw, th) = (ATLAS_PER_ROW * font::GLYPH_W, ATLAS_PER_ROW * font::GLYPH_H);
        let mut pixels = vec![0u8; tw * th * 4];
        for g in 0..font::GLYPHS {
            for y in 0..font::GLYPH_H {
                for x in 0..font::GLYPH_W {
                    let a = font::ATLAS[(g * font::GLYPH_H + y) * font::GLYPH_W + x];
                    if a == 0 {
                        continue;
                    }
                    let px = (g % ATLAS_PER_ROW) * font::GLYPH_W + x;
                    let py = (g / ATLAS_PER_ROW) * font::GLYPH_H + y;
                    let o = (py * tw + px) * 4;
                    pixels[o..o + 4].copy_from_slice(&[0xff, 0xff, 0xff, a]);
                }
            }
        }
        let mut tex = creator
            .create_texture_static(PixelFormatEnum::RGBA32, tw as u32, th as u32)
            .map_err(|e| e.to_string())?;
        tex.update(None, &pixels, tw * 4).map_err(|e| e.to_string())?;
        tex.set_blend_mode(BlendMode::Blend);
        tex.set_color_mod(INK.r, INK.g, INK.b);
        Ok(tex)
    }

    /// Redraw when the page or the view of it changed.
    pub(crate) fn render_if_needed(&mut self, font_tex: &Texture) {
        if self.view.take_redraw() {
            self.render(font_tex);
        }
    }

    fn render(&mut self, font_tex: &Texture) {
        self.canvas.set_draw_color(PAPER_BG);
        self.canvas.clear();
        let canvas = &mut self.canvas;
        self.view.glyphs(|g, x, y| {
            let src = Rect::new(
                ((g % ATLAS_PER_ROW) * font::GLYPH_W) as i32,
                ((g / ATLAS_PER_ROW) * font::GLYPH_H) as i32,
                font::GLYPH_W as u32,
                font::GLYPH_H as u32,
            );
            let _ = canvas.copy(font_tex, src, Rect::new(x, y, font::GLYPH_W as u32, font::GLYPH_H as u32));
        });
        self.canvas.present();
    }

    /// Roll the platen: wheel up (positive y) is back into the scrollback.
    pub(crate) fn wheel(&mut self, dy: i32) {
        // the window scrolls nothing else, so a roll at a stop goes nowhere
        let _ = self.view.wheel(dy);
    }

    /// The window's pixels, RGB, for the test that holds them to a
    /// framebuffer's.
    #[cfg(test)]
    pub(crate) fn pixels(&self) -> Vec<u8> {
        self.canvas.read_pixels(None, PixelFormatEnum::RGB24).unwrap()
    }

    /// SDL text input: the printable characters, upcased -- the Model 33
    /// keyboard has no lowercase (the *printer's* fold lives in
    /// [`fold_to_typebox`]; this is the other half of the same fact).
    pub(crate) fn text_input(&mut self, text: &str) {
        for c in text.chars() {
            let b = c as u32;
            if (0x20..0x7f).contains(&b) {
                let _ = self.tx.send((b as u8).to_ascii_uppercase());
            }
        }
    }

    /// A key pressed with this window focused. Ctrl-D is the frontend's
    /// business and never arrives here.
    pub(crate) fn key_down(&mut self, key: Keycode, keymod: Mod) {
        if let Some(b) = tty_keyboard_code(key, keymod) {
            let _ = self.tx.send(b);
        }
    }
}

/// The non-printing keys of the Model 33 keyboard, on the Kaypro window's
/// mapping conventions (`sdl::keyboard_code` is the spec). Printables come
/// from SDL text input instead, so ctrl combinations -- which suppress text
/// input -- are handled here through the shared [`control_code`].
fn tty_keyboard_code(key: Keycode, keymod: Mod) -> Option<u8> {
    let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        return control_code(key, shift);
    }
    match key {
        // shift-Return is the LINE FEED key, its own key on the Model 33
        // and how an X-RAY record opens
        Keycode::Return | Keycode::KpEnter if shift => Some(0x0a),
        Keycode::Return | Keycode::KpEnter => Some(0x0d),
        Keycode::Escape => Some(0x1b),
        // RUBOUT is the Model 33's erase; there is no backspace key or
        // mechanism, so both host erase keys send it
        Keycode::Backspace | Keycode::Delete => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mod = Mod::empty();
    const SHIFT: Mod = Mod::LSHIFTMOD;
    const CTRL: Mod = Mod::LCTRLMOD;

    #[test]
    fn return_is_carriage_return_and_shift_return_is_the_line_feed_key() {
        assert_eq!(tty_keyboard_code(Keycode::Return, NONE), Some(0x0d));
        assert_eq!(tty_keyboard_code(Keycode::Return, SHIFT), Some(0x0a));
        assert_eq!(tty_keyboard_code(Keycode::KpEnter, NONE), Some(0x0d));
        assert_eq!(tty_keyboard_code(Keycode::KpEnter, SHIFT), Some(0x0a));
    }

    #[test]
    fn both_erase_keys_send_rubout() {
        assert_eq!(tty_keyboard_code(Keycode::Backspace, NONE), Some(0x7f));
        assert_eq!(tty_keyboard_code(Keycode::Delete, NONE), Some(0x7f));
    }

    #[test]
    fn control_codes_reach_the_guest() {
        assert_eq!(tty_keyboard_code(Keycode::C, CTRL), Some(0x03));
        assert_eq!(tty_keyboard_code(Keycode::J, CTRL), Some(0x0a), "X-RAY's record opener");
        assert_eq!(tty_keyboard_code(Keycode::LEFTBRACKET, CTRL), Some(0x1b));
    }

    /// The Model 33 has no TAB key or mechanism, and Tab must stay free
    /// for nothing here -- unlike the panel window, where it turns the
    /// selector knob (separate windows keep the two uses apart already,
    /// but a teletype that tabs would be inventing hardware).
    #[test]
    fn tab_sends_nothing() {
        assert_eq!(tty_keyboard_code(Keycode::Tab, NONE), None);
        assert_eq!(tty_keyboard_code(Keycode::A, NONE), None, "printables ride text input");
    }
}
