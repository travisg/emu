// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! SDL2 window frontend, for machines with a screen (the Kaypro).
//!
//! Port of `console_sdl.cpp` plus the render half of `system_kaypro.cpp`
//! (`RenderDisplay`/`DrawChar`). It owns the window, pumps events, sends
//! keystrokes down the same channel the terminal frontend uses, and redraws the
//! shared [`VideoBuffer`] whenever the CPU thread has dirtied it.
//!
//! The `sdl2` crate's context types are `!Send`, so main-thread confinement is
//! enforced by the compiler; and the shutdown flag is polled inside the
//! already-ticking loop, so the C++ `SDL_PushEvent(SDL_QUIT)` wake-up hack in
//! `ConsoleSDL::Stop` has no equivalent here.

use super::{ConsoleFrontend, Display, VideoBuffer};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

/// Text-mode geometry. The Kaypro's video ram is laid out with a 128-byte row
/// stride, of which the first 80 columns are visible.
pub const COLUMNS: usize = 80;
pub const ROWS: usize = 24;
pub const ROW_STRIDE: usize = 128;

/// The character generator is an 8x8 font; the CRT's aspect stretched each
/// row, so it is drawn at 8x16.
const GLYPH_W: u32 = 8;
const GLYPH_H: u32 = 8;
const CELL_W: u32 = 8;
const CELL_H: u32 = 16;
const WINDOW_W: u32 = COLUMNS as u32 * CELL_W;
const WINDOW_H: u32 = ROWS as u32 * CELL_H;

/// The font atlas: 16x16 glyphs of 8x8, one per character code.
const ATLAS_GLYPHS_PER_ROW: u32 = 16;
const ATLAS_SIZE: u32 = ATLAS_GLYPHS_PER_ROW * GLYPH_W;

/// Roughly 60 Hz.
const FRAME_DELAY: Duration = Duration::from_millis(16);

const PHOSPHOR: Color = Color::RGB(0, 255, 0);
const BLACK: Color = Color::RGB(0, 0, 0);

pub struct SdlFrontend {
    tx: Sender<u8>,
    video: VideoBuffer,
    font_rom: Vec<u8>,
    sdl: sdl2::Sdl,
    /// Kept alive for the text-input state.
    video_subsystem: sdl2::VideoSubsystem,
    canvas: Canvas<Window>,
}

impl SdlFrontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        // main.rs routes each Display variant to its own frontend; this one
        // renders character cells and nothing else.
        let Display::CharCell { title, video, font_rom } = display else {
            return Err("SdlFrontend needs a character-cell display".to_string());
        };
        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        let window = video_subsystem
            .window(title, WINDOW_W, WINDOW_H)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;
        let canvas = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
        video_subsystem.text_input().start();

        Ok(SdlFrontend { tx, video, font_rom, sdl, video_subsystem, canvas })
    }

    /// Build the font atlas from the character generator rom.
    ///
    /// The Kaypro II u43 rom stores the ASCII set shifted by 128 and with the
    /// pixel sense inverted, so glyph `c` comes from rom entry `(c + 128) %
    /// 256` and each row byte is complemented.
    fn build_font_atlas<'a>(
        &self,
        creator: &'a TextureCreator<WindowContext>,
    ) -> Result<Texture<'a>, String> {
        let mut pixels = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
        for c in 0..256usize {
            let cx = (c % ATLAS_GLYPHS_PER_ROW as usize) * GLYPH_W as usize;
            let cy = (c / ATLAS_GLYPHS_PER_ROW as usize) * GLYPH_H as usize;
            let font_c = (c + 128) % 256;
            for y in 0..GLYPH_H as usize {
                let row = !self.font_rom.get(font_c * 8 + y).copied().unwrap_or(0xff);
                for x in 0..GLYPH_W as usize {
                    if row & (0x80 >> x) != 0 {
                        let i = ((cy + y) * ATLAS_SIZE as usize + cx + x) * 4;
                        // opaque white in every channel, whichever byte order
                        // the packed format has
                        pixels[i..i + 4].copy_from_slice(&[0xff; 4]);
                    }
                }
            }
        }

        let mut tex = creator
            .create_texture_static(PixelFormatEnum::RGBA8888, ATLAS_SIZE, ATLAS_SIZE)
            .map_err(|e| e.to_string())?;
        tex.update(None, &pixels, (ATLAS_SIZE * 4) as usize).map_err(|e| e.to_string())?;
        tex.set_blend_mode(BlendMode::Blend);
        Ok(tex)
    }

    fn render(&mut self, font: &mut Texture) {
        // snapshot the frame under the lock, then draw without holding it
        let frame: Vec<u8> = self.video.with_ram(|ram| ram.to_vec());

        self.canvas.set_draw_color(BLACK);
        self.canvas.clear();
        for y in 0..ROWS {
            for x in 0..COLUMNS {
                let c = frame.get(y * ROW_STRIDE + x).copied().unwrap_or(0);
                self.draw_char(font, x as i32, y as i32, c);
            }
        }
        self.canvas.present();
    }

    fn draw_char(&mut self, font: &mut Texture, x: i32, y: i32, c: u8) {
        // blank cells: nothing to draw over the cleared background
        if c == 0x20 || c == 0x00 || c == 0xff {
            return;
        }

        let dst = Rect::new(x * CELL_W as i32, y * CELL_H as i32, CELL_W, CELL_H);
        let inverse = c & 0x80 != 0;
        let idx = (c & 0x7f) as i32;
        let src = Rect::new(
            (idx % ATLAS_GLYPHS_PER_ROW as i32) * GLYPH_W as i32,
            (idx / ATLAS_GLYPHS_PER_ROW as i32) * GLYPH_H as i32,
            GLYPH_W,
            GLYPH_H,
        );

        if inverse {
            self.canvas.set_draw_color(PHOSPHOR);
            let _ = self.canvas.fill_rect(dst);
            font.set_color_mod(0, 0, 0);
        } else {
            font.set_color_mod(0, 255, 0);
        }
        let _ = self.canvas.copy(font, src, dst);
    }

    /// Queue a keystroke for the machine. False if the CPU thread is gone.
    fn send(&self, c: u8) -> bool {
        self.tx.send(c).is_ok()
    }
}

impl ConsoleFrontend for SdlFrontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        let creator = self.canvas.texture_creator();
        let mut font = match self.build_font_atlas(&creator) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("SdlFrontend: failed to create font texture: {e}");
                shutdown.store(true, Ordering::SeqCst);
                return;
            }
        };
        let mut pump = match self.sdl.event_pump() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("SdlFrontend: failed to create event pump: {e}");
                shutdown.store(true, Ordering::SeqCst);
                return;
            }
        };
        let _ = &self.video_subsystem;

        // paint the empty screen once so the window isn't undefined until the
        // guest first touches video ram
        self.render(&mut font);

        println!("SdlFrontend: entering event loop");
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("SdlFrontend: stop requested, exiting");
                return;
            }

            for event in pump.poll_iter() {
                match event {
                    Event::Quit { .. } => {
                        println!("SdlFrontend: quit event received");
                        shutdown.store(true, Ordering::SeqCst);
                        return;
                    }
                    Event::TextInput { text, .. } => {
                        for b in text.bytes() {
                            if !self.send(b) {
                                shutdown.store(true, Ordering::SeqCst);
                                return;
                            }
                        }
                    }
                    Event::KeyDown { keycode: Some(key), keymod, .. } => {
                        // the non-printable keys text input doesn't deliver
                        let c = match key {
                            Keycode::Backspace => Some(0x08),
                            Keycode::Return => Some(0x0d),
                            Keycode::Escape => Some(0x1b),
                            Keycode::D if keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => {
                                println!("ctrl-d hit on SDL console, exiting");
                                shutdown.store(true, Ordering::SeqCst);
                                return;
                            }
                            _ => None,
                        };
                        if let Some(c) = c {
                            if !self.send(c) {
                                shutdown.store(true, Ordering::SeqCst);
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if self.video.take_dirty() {
                self.render(&mut font);
            }

            std::thread::sleep(FRAME_DELAY);
        }
    }
}
