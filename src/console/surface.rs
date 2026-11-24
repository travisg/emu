// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! What the panels draw on: filled rectangles in a color, alpha blended,
//! and nothing else. An SDL canvas is one ([`Surface`] for
//! `Canvas<Window>`); a [`Framebuffer`] of RGBA bytes is the other, which
//! is how a host with no SDL -- the web page -- gets the same pixels from
//! the same drawing code.

/// A color with alpha, spelled as sdl2's is so the panels' constants read
/// the same on either surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    #[allow(non_snake_case)]
    pub const fn RGB(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 0xff }
    }

    #[allow(non_snake_case)]
    pub const fn RGBA(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
}

pub trait Surface {
    /// Fill a rectangle, blending by the color's alpha. The size follows
    /// sdl2's `Rect::new`: a zero width or height is one pixel, and a
    /// negative one cast to `u32` is clamped huge, then clipped.
    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color);

    /// Fill the whole surface, opaque.
    fn clear(&mut self, color: Color);
}

/// An RGBA image in memory, row by row, four bytes a pixel -- the layout
/// a browser's `ImageData` takes.
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Framebuffer { width, height, pixels: vec![0; width as usize * height as usize * 4] }
    }

    /// Blend `color` over the pixel at byte offset `o` by `alpha`.
    fn blend(&mut self, o: usize, color: Color, alpha: u8) {
        let p = &mut self.pixels[o..o + 4];
        if alpha == 0xff {
            p.copy_from_slice(&[color.r, color.g, color.b, 0xff]);
            return;
        }
        // SDL's software blend, to the bit: src * a / 255 + dst * (255 - a)
        // / 255, each term truncated on its own
        let (a, ia) = (alpha as u32, 0xff - alpha as u32);
        for (d, s) in p.iter_mut().zip([color.r, color.g, color.b]) {
            *d = (s as u32 * a / 0xff + *d as u32 * ia / 0xff) as u8;
        }
        p[3] = 0xff;
    }

    /// An 8-bit coverage mask `w` pixels wide, row after row, inked in
    /// `color` with its top left at (x, y) and clipped to the image: how
    /// the teletype's typewheel atlas prints.
    pub fn mask(&mut self, x: i32, y: i32, mask: &[u8], w: usize, color: Color) {
        for my in 0..mask.len() / w {
            let py = y + my as i32;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            for mx in 0..w {
                let px = x + mx as i32;
                let a = mask[my * w + mx];
                if a == 0 || px < 0 || px >= self.width as i32 {
                    continue;
                }
                let o = (py as usize * self.width as usize + px as usize) * 4;
                self.blend(o, color, a);
            }
        }
    }
}

/// sdl2's `Rect::new` clamp: zero is one, and nothing past `i32::MAX / 2`.
fn rect_size(v: u32) -> i64 {
    v.clamp(1, i32::MAX as u32 / 2) as i64
}

impl Surface for Framebuffer {
    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        let (x, y) = (x as i64, y as i64);
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + rect_size(w)).min(self.width as i64);
        let y1 = (y + rect_size(h)).min(self.height as i64);
        for py in y0..y1 {
            for px in x0..x1 {
                let o = (py as usize * self.width as usize + px as usize) * 4;
                self.blend(o, color, color.a);
            }
        }
    }

    fn clear(&mut self, color: Color) {
        for p in self.pixels.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[color.r, color.g, color.b, 0xff]);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod sdl {
    use super::{Color, Surface};
    use sdl2::rect::Rect;
    use sdl2::render::Canvas;
    use sdl2::video::Window;

    impl From<Color> for sdl2::pixels::Color {
        fn from(c: Color) -> Self {
            sdl2::pixels::Color::RGBA(c.r, c.g, c.b, c.a)
        }
    }

    /// Blending needs the canvas's blend mode set to `Blend`, which each
    /// panel window does as it builds its canvas.
    impl Surface for Canvas<Window> {
        fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
            self.set_draw_color(color);
            let _ = Canvas::fill_rect(self, Rect::new(x, y, w, h));
        }

        fn clear(&mut self, color: Color) {
            self.set_draw_color(color);
            Canvas::clear(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(f: &Framebuffer, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * f.width + x) * 4) as usize;
        f.pixels[o..o + 4].try_into().unwrap()
    }

    #[test]
    fn a_rect_is_clipped_and_a_zero_size_is_one_pixel() {
        let mut f = Framebuffer::new(4, 4);
        f.clear(Color::RGB(0, 0, 0));
        f.fill_rect(-2, -2, 3, 3, Color::RGB(9, 9, 9));
        assert_eq!(px(&f, 0, 0), [9, 9, 9, 255]);
        assert_eq!(px(&f, 1, 1), [0, 0, 0, 255], "the rect ends at x = 0");
        f.fill_rect(3, 3, 0, 0, Color::RGB(7, 7, 7));
        assert_eq!(px(&f, 3, 3), [7, 7, 7, 255]);
        // a width computed negative and cast, as a panel's rules can be
        f.fill_rect(2, 0, (-5i32) as u32, 1, Color::RGB(5, 5, 5));
        assert_eq!((px(&f, 2, 0), px(&f, 3, 0)), ([5, 5, 5, 255], [5, 5, 5, 255]));
    }

    #[test]
    fn alpha_blends_over_what_is_there() {
        let mut f = Framebuffer::new(1, 1);
        f.clear(Color::RGB(0, 100, 255));
        f.fill_rect(0, 0, 1, 1, Color::RGBA(255, 100, 0, 51));
        // 255 * 51 / 255 + 0, 100 * 51 / 255 + 100 * 204 / 255 = 20 + 80,
        // 0 + 255 * 204 / 255
        assert_eq!(px(&f, 0, 0), [51, 100, 204, 255]);
    }

    #[test]
    fn a_mask_inks_by_coverage_and_clips() {
        let mut f = Framebuffer::new(2, 1);
        f.clear(Color::RGB(255, 255, 255));
        f.mask(-1, 0, &[255, 0, 255], 3, Color::RGB(0, 0, 0));
        assert_eq!(px(&f, 0, 0), [255, 255, 255, 255], "coverage 0");
        assert_eq!(px(&f, 1, 0), [0, 0, 0, 255]);
    }

    fn rgb(f: &Framebuffer) -> Vec<u8> {
        f.pixels.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()
    }

    fn differ(a: &[u8], b: &[u8]) -> usize {
        assert_eq!(a.len(), b.len());
        a.chunks(3).zip(b.chunks(3)).filter(|(a, b)| a != b).count()
    }

    /// The 703's panel and its teletype paper draw the same pixels on a
    /// framebuffer as on SDL's software renderer, which is what lets the
    /// web page show what the windows show. One test for both, because
    /// SDL is initialized once per process and only ever from one thread.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_framebuffer_draws_what_sdl_draws() {
        use crate::console::panel703::PanelView;
        use crate::console::paper::Paper;
        use crate::console::paper_view::{PaperView, PAPER_BG};
        use crate::console::tty703::TtyWindow;
        use crate::console::{DiscRackState, PanelControl, PanelDisplay, PanelState, TtyDisplay};
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        std::env::set_var("SDL_RENDER_DRIVER", "software");
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();

        // The panel: frames with lamps at part duty, the HALT lens lit and
        // not, the knob turned and the rack winking. Two views over one
        // machine, so both see the same accumulators.
        let window = video.window("panel", PanelView::WIDTH, PanelView::HEIGHT).build().unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let mut fb = Framebuffer::new(PanelView::WIDTH, PanelView::HEIGHT);
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
            on_sdl.key('\t');
            on_fb.key('\t');
            on_sdl.render(&mut canvas);
            on_fb.render(&mut fb);
            let sdl_px = canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGB24).unwrap();
            let fb_px = rgb(&fb);
            assert_eq!(differ(&sdl_px, &fb_px), 0, "panel frame {frame}");
            // and the frame is a panel, not two blank surfaces agreeing --
            // by the last, with the bulbs warmed up, glows and all
            let colors = fb_px.chunks(3).collect::<std::collections::HashSet<_>>().len();
            assert!(colors > if frame == 3 { 40 } else { 10 }, "frame {frame}: only {colors} colors");
        }

        // The paper: the SDL window prints from a texture, tinted.
        let paper = Paper::new();
        let mut w = TtyWindow::new(&video, "t", TtyDisplay { paper: paper.clone(), line: None }, tx_keys()).unwrap();
        let creator = w.texture_creator();
        let font = w.build_font(&creator).unwrap();
        for b in *b"REX 703 UP\r\nREX> HELLO @#$%&*()[]\\^_\r\n" {
            paper.feed(b);
        }
        w.render_if_needed(&font);
        let mut fb = Framebuffer::new(PaperView::WIDTH, PaperView::HEIGHT);
        PaperView::new(paper).render(&mut fb);
        let fb_px = rgb(&fb);
        assert_eq!(differ(&w.pixels(), &fb_px), 0, "paper");
        let inked = fb_px.chunks(3).filter(|p| *p != [PAPER_BG.r, PAPER_BG.g, PAPER_BG.b]).count();
        assert!(inked > 300, "only {inked} pixels of ink: the page did not print");
    }

    fn tx_keys() -> std::sync::mpsc::Sender<u8> {
        std::sync::mpsc::channel().0
    }
}
