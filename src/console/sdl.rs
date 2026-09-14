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
//! The keyboard half is more than a passthrough: the window stands in for the
//! Kaypro's own detached keyboard, so the keys that never sent ascii -- the
//! arrows above all -- send what that keyboard sent. See [`keyboard_code`].
//!
//! To the right of the screen is the drive bay, laid out as the Kaypro II
//! was: two full-height 5.25" drives one above the other, A on top, each
//! with its slot, its door lever and its red light. The light is drawn
//! from the controller's on-time duty over the frame, through the
//! [`DriveBayState`] the machine publishes. Clicking a drive opens its door:
//! the disk in it is ejected, and a file chooser picks the next one. The
//! chooser is the platform's own, run as a subprocess on a helper thread
//! (`osascript` on macOS, `zenity` or `kdialog` elsewhere) so the screen
//! keeps drawing while it is up; what it picks is opened here, on the main
//! thread, and goes to the machine over the run loop's control channel as
//! a [`DiskImage`], for the machine to judge -- a file that is not a disk
//! for that drive is refused with the reason on the terminal. Nothing here
//! touches the machine. The debug port's `disk` and `eject` are the same
//! doors without the window.
//!
//! The `sdl2` crate's context types are `!Send`, so main-thread confinement is
//! enforced by the compiler; and the shutdown flag is polled inside the
//! already-ticking loop, so the C++ `SDL_PushEvent(SDL_QUIT)` wake-up hack in
//! `ConsoleSDL::Stop` has no equivalent here.

use super::{ConsoleFrontend, DiskImage, Display, DriveBayDisplay, DriveBaySnapshot, VideoBuffer, DRIVE_BAYS};
use crate::dev::wd1793::drive_letter;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
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
const SCREEN_W: u32 = COLUMNS as u32 * CELL_W;
const SCREEN_H: u32 = ROWS as u32 * CELL_H;

/// The drive bay: a column to the right of the screen with the two drives
/// stacked in it, A above B.
const BAY_W: u32 = 176;
const SCREEN_X: i32 = 0;
const BAY_X: i32 = SCREEN_W as i32;
const WINDOW_W: u32 = SCREEN_W + BAY_W;
const WINDOW_H: u32 = SCREEN_H;

/// A full-height 5.25" drive's bezel, about 146 x 82 mm, at a pixel to
/// the millimetre and a bit.
const FACE_W: u32 = 152;
const FACE_H: u32 = 86;
/// The top of drive A's face, and the pitch down to B's: each face with
/// its label below it, the pair centred in the column.
const FACE_Y: i32 = 36;
const FACE_PITCH: i32 = 174;
/// The diskette slot, across the upper part of the face.
const SLOT_Y: i32 = 24;
const SLOT_H: u32 = 5;
const SLOT_INSET: i32 = 12;
/// The door lever pivots below the slot: horizontal with the door open,
/// turned down across it with a disk latched in.
const LEVER_CY: i32 = 54;
const LEVER_LEN: u32 = 40;
const LEVER_W: u32 = 8;
/// The LED at the bottom left of the bezel, and the drive letter at the
/// bottom right.
const LED_X: i32 = 16;
const LED_Y: i32 = FACE_H as i32 - 18;
const LED_W: u32 = 12;
const LED_H: u32 = 6;
/// The image's name, this far below its face, in the machine's own
/// character set.
const LABEL_GAP: i32 = 8;
/// Characters of name that fit the column.
const LABEL_CHARS: usize = (BAY_W / CELL_W) as usize - 2;

/// The font atlas: 16x16 glyphs of 8x8, one per character code.
const ATLAS_GLYPHS_PER_ROW: u32 = 16;
const ATLAS_SIZE: u32 = ATLAS_GLYPHS_PER_ROW * GLYPH_W;

/// Roughly 60 Hz.
const FRAME_DELAY: Duration = Duration::from_millis(16);

const PHOSPHOR: Color = Color::RGB(0, 255, 0);
const BLACK: Color = Color::RGB(0, 0, 0);
/// The cabinet around the screen, the Kaypro's blue-grey.
const CABINET: Color = Color::RGB(0x3a, 0x44, 0x52);
/// The drive bezels, black plastic with a lighter rim.
const BEZEL: Color = Color::RGB(0x1e, 0x1e, 0x20);
const BEZEL_RIM: Color = Color::RGB(0x50, 0x52, 0x56);
const SLOT: Color = Color::RGB(0x06, 0x06, 0x08);
/// A diskette's jacket edge showing in the slot.
const JACKET: Color = Color::RGB(0x2c, 0x2c, 0x34);
const LEVER: Color = Color::RGB(0xa8, 0xa8, 0xa4);
const LED_OFF: Color = Color::RGB(0x48, 0x0c, 0x0a);
const LED_ON: Color = Color::RGB(0xff, 0x30, 0x1c);
/// The painted labels: the drive letter on the bezel, the name below it.
const LABEL: Color = Color::RGB(0xc8, 0xcc, 0xd0);
const LABEL_DIM: Color = Color::RGB(0x80, 0x88, 0x90);

const CTRL: Mod = Mod::from_bits_truncate(Mod::LCTRLMOD.bits() | Mod::RCTRLMOD.bits());
const SHIFT: Mod = Mod::from_bits_truncate(Mod::LSHIFTMOD.bits() | Mod::RSHIFTMOD.bits());

/// The four arrow keys, as the keyboard sends them.
///
/// The Kaypro's keyboard is a detached serial one with an MCS-48 of its own
/// (an NEC 8049 on the later boards), and its arrow keys and 14-key pad --
/// the "18 programmable keys" of the sales copy -- send codes above 0x7f
/// rather than ascii. The BIOS translates them through a pair of tables, and
/// the stock table turns these four into the cursor motions the video section
/// takes: ^K up, ^J down, ^H left, ^L right. Sending the raw codes rather
/// than the translated ones is what a program that repatches that table gets
/// to keep working with.
const KEY_UP: u8 = 0xf1;
const KEY_DOWN: u8 = 0xf2;
const KEY_LEFT: u8 = 0xf3;
const KEY_RIGHT: u8 = 0xf4;

/// The byte a Kaypro keyboard sends for `key`, for the keys SDL's text input
/// doesn't deliver: the arrows, the control codes, and the keys whose caps
/// (BACK SPACE, DEL, LINE FEED) name a code rather than a character.
///
/// The keypad is deliberately absent. Its keys send codes of their own too,
/// but the stock BIOS table maps them to exactly the digits and punctuation
/// on the caps, which is what text input already delivers for a host keypad.
fn keyboard_code(key: Keycode, keymod: Mod) -> Option<u8> {
    let shift = keymod.intersects(SHIFT);
    match key {
        Keycode::Up => Some(KEY_UP),
        Keycode::Down => Some(KEY_DOWN),
        Keycode::Left => Some(KEY_LEFT),
        Keycode::Right => Some(KEY_RIGHT),
        Keycode::Backspace => Some(0x08),
        Keycode::Tab => Some(0x09),
        // the keyboard's LINE FEED key, which no host keyboard has a cap for.
        // Distinct from Return to the guest: CP/M's line editor takes 0x0a as
        // "end of the physical line", and it is a separate key here because
        // it was a separate key there.
        Keycode::Return if shift => Some(0x0a),
        Keycode::Return | Keycode::KpEnter => Some(0x0d),
        Keycode::Escape => Some(0x1b),
        Keycode::Delete => Some(0x7f),
        _ if keymod.intersects(CTRL) => control_code(key, shift),
        _ => None,
    }
}

/// The control code for a ctrl-modified key. SDL delivers no text input while
/// ctrl is down, so without this the guest could never see one -- and CP/M is
/// driven by them (^C warm start, ^S pause, ^Z end of file).
pub(crate) fn control_code(key: Keycode, shift: bool) -> Option<u8> {
    let code = key.into_i32();
    if (Keycode::A.into_i32()..=Keycode::Z.into_i32()).contains(&code) {
        // ctrl-a..ctrl-z are 0x01..0x1a
        return Some((code - Keycode::A.into_i32()) as u8 + 1);
    }
    match key {
        Keycode::LEFTBRACKET => Some(0x1b),
        Keycode::BACKSLASH => Some(0x1c),
        Keycode::RIGHTBRACKET => Some(0x1d),
        // the shifted caps of the us layout, since ctrl-shift-6 is how a
        // keyboard without a caret key of its own reaches 0x1e
        Keycode::NUM_6 if shift => Some(0x1e),
        Keycode::MINUS if shift => Some(0x1f),
        Keycode::NUM_2 if shift => Some(0x00),
        Keycode::SPACE => Some(0x00),
        _ => None,
    }
}

/// Where the drive faces sit: stacked in the bay, A on top.
fn face_rect(drive: usize) -> Rect {
    Rect::new(BAY_X + (BAY_W - FACE_W) as i32 / 2, FACE_Y + drive as i32 * FACE_PITCH, FACE_W, FACE_H)
}

/// The top of the name under a drive's face.
fn label_y(drive: usize) -> i32 {
    face_rect(drive).bottom() + LABEL_GAP
}

/// The drive under a click, if any.
fn drive_at(x: i32, y: i32) -> Option<usize> {
    (0..DRIVE_BAYS).find(|&d| face_rect(d).contains_point((x, y)))
}

/// The label under a drive: the image's file name, cut to fit, or what an
/// empty drive says.
fn label_for(path: Option<&Path>) -> String {
    let Some(path) = path else {
        return "(empty)".to_string();
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if name.chars().count() <= LABEL_CHARS {
        name
    } else {
        let head: String = name.chars().take(LABEL_CHARS - 3).collect();
        format!("{head}...")
    }
}

/// The light's duty this frame: the fraction of the machine's cycles the
/// drive spent selected with its motor on. With no cycles in the frame it
/// is off, which is what a stopped machine's light looks like.
fn led_duty(now: &DriveBaySnapshot, prev: &DriveBaySnapshot, drive: usize) -> f32 {
    let dc = now.cycles.wrapping_sub(prev.cycles);
    if dc == 0 {
        0.0
    } else {
        (now.select_on[drive].wrapping_sub(prev.select_on[drive]) as f32 / dc as f32).clamp(0.0, 1.0)
    }
}

fn mix(a: Color, b: Color, f: f32) -> Color {
    let ch = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
    Color::RGB(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b))
}

/// Run the platform's file chooser and return what it picked, or `None`
/// on cancel or with no chooser to run. Blocks for as long as the dialog
/// is up, so it runs on a helper thread.
///
/// The choosers are subprocesses rather than a crate: a native dialog
/// binding would pull in a toolkit on every platform for one call. Each
/// prints the chosen path on stdout and exits non-zero on cancel.
fn choose_file(prompt: &str, start: &Path) -> Option<PathBuf> {
    let start = start.to_string_lossy().into_owned();
    // (program, arguments) in order of preference for the platform; the
    // first one that can be spawned decides.
    let candidates: Vec<(&str, Vec<String>)> = if cfg!(target_os = "macos") {
        // AppleScript string literals take backslash escapes
        let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        vec![(
            "osascript",
            vec![
                "-e".into(),
                format!(
                    "POSIX path of (choose file with prompt \"{}\" default location (POSIX file \"{}\"))",
                    quote(prompt),
                    quote(&start)
                ),
            ],
        )]
    } else {
        vec![
            (
                "zenity",
                vec![
                    "--file-selection".into(),
                    format!("--title={prompt}"),
                    format!("--filename={start}/"),
                ],
            ),
            ("kdialog", vec!["--getopenfilename".into(), start.clone(), prompt.to_string()]),
        ]
    };
    for (program, args) in &candidates {
        let out = match Command::new(program).args(args).output() {
            Ok(out) => out,
            // not installed: try the next
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("{program}: {e}");
                return None;
            }
        };
        if !out.status.success() {
            // cancelled, or the tool complained -- either way no file
            return None;
        }
        let picked = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\r', '\n']).to_string();
        return (!picked.is_empty()).then(|| PathBuf::from(picked));
    }
    let names: Vec<&str> = candidates.iter().map(|(p, _)| *p).collect();
    eprintln!("no file chooser found (need one of: {})", names.join(", "));
    None
}

pub struct SdlFrontend {
    tx: Sender<u8>,
    video: VideoBuffer,
    font_rom: Vec<u8>,
    drives: DriveBayDisplay,
    sdl: sdl2::Sdl,
    /// Kept alive for the text-input state.
    video_subsystem: sdl2::VideoSubsystem,
    canvas: Canvas<Window>,
    /// Last frame's LED counters, to diff against.
    prev_bay: DriveBaySnapshot,
    /// A file chooser up for a drive, and where its answer arrives. One
    /// at a time: clicks on either drive are ignored while it is up.
    chooser: Option<(usize, Receiver<Option<PathBuf>>)>,
}

impl SdlFrontend {
    pub fn new(tx: Sender<u8>, display: Display) -> Result<Self, String> {
        // main.rs routes each Display variant to its own frontend; this one
        // renders character cells and the drive bay beside them.
        let Display::CharCell { title, video, font_rom, drives } = display else {
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

        Ok(SdlFrontend {
            tx,
            video,
            font_rom,
            drives,
            sdl,
            video_subsystem,
            canvas,
            prev_bay: DriveBaySnapshot::default(),
            chooser: None,
        })
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

    /// The whole window: the screen, then the drive bay beside it.
    fn render(&mut self, font: &mut Texture) {
        // snapshot the frame under the lock, then draw without holding it
        let frame: Vec<u8> = self.video.with_ram(|ram| ram.to_vec());

        self.canvas.set_draw_color(CABINET);
        self.canvas.clear();
        self.canvas.set_draw_color(BLACK);
        let _ = self.canvas.fill_rect(Rect::new(SCREEN_X, 0, SCREEN_W, SCREEN_H));
        for y in 0..ROWS {
            for x in 0..COLUMNS {
                let c = frame.get(y * ROW_STRIDE + x).copied().unwrap_or(0);
                self.draw_char(font, x as i32, y as i32, c);
            }
        }

        let now = self.drives.state.snapshot();
        for drive in 0..DRIVE_BAYS {
            let duty = led_duty(&now, &self.prev_bay, drive);
            let mounted = self.drives.state.mounted(drive);
            self.draw_drive(font, drive, mounted.as_deref(), duty);
        }
        self.prev_bay = now;
        self.canvas.present();
    }

    /// One drive: bezel, slot (with a jacket edge showing if a disk is
    /// in), the door lever, the LED, the letter, and the name below.
    fn draw_drive(&mut self, font: &mut Texture, drive: usize, mounted: Option<&Path>, duty: f32) {
        let face = face_rect(drive);
        let (x, y) = (face.x(), face.y());
        self.canvas.set_draw_color(BEZEL_RIM);
        let _ = self.canvas.fill_rect(face);
        self.canvas.set_draw_color(BEZEL);
        let _ = self.canvas.fill_rect(Rect::new(x + 2, y + 2, FACE_W - 4, FACE_H - 4));

        let slot = Rect::new(x + SLOT_INSET, y + SLOT_Y, FACE_W - 2 * SLOT_INSET as u32, SLOT_H);
        self.canvas.set_draw_color(SLOT);
        let _ = self.canvas.fill_rect(slot);
        if mounted.is_some() {
            self.canvas.set_draw_color(JACKET);
            let _ = self.canvas.fill_rect(Rect::new(slot.x() + 2, slot.y() + 1, slot.width() - 4, SLOT_H - 2));
        }

        let cx = x + FACE_W as i32 / 2;
        let cy = y + LEVER_CY;
        let lever = if mounted.is_some() {
            // latched: the lever turned down across the slot
            Rect::new(cx - LEVER_W as i32 / 2, cy - LEVER_LEN as i32 / 2, LEVER_W, LEVER_LEN)
        } else {
            Rect::new(cx - LEVER_LEN as i32 / 2, cy - LEVER_W as i32 / 2, LEVER_LEN, LEVER_W)
        };
        self.canvas.set_draw_color(LEVER);
        let _ = self.canvas.fill_rect(lever);

        self.canvas.set_draw_color(mix(LED_OFF, LED_ON, duty));
        let _ = self.canvas.fill_rect(Rect::new(x + LED_X, y + LED_Y, LED_W, LED_H));

        let letter = drive_letter(drive) as u8;
        self.draw_glyph_at(font, x + FACE_W as i32 - 24, y + FACE_H as i32 - 22, letter, LABEL);

        let label = label_for(mounted);
        let color = if mounted.is_some() { LABEL } else { LABEL_DIM };
        let w = label.len() as i32 * CELL_W as i32;
        let mut lx = cx - w / 2;
        let ly = label_y(drive);
        for &c in label.as_bytes() {
            self.draw_glyph_at(font, lx, ly, c, color);
            lx += CELL_W as i32;
        }
    }

    /// A screen cell at column `x`, row `y`.
    fn draw_char(&mut self, font: &mut Texture, x: i32, y: i32, c: u8) {
        // blank cells: nothing to draw over the cleared background
        if c == 0x20 || c == 0x00 || c == 0xff {
            return;
        }

        let dst = Rect::new(SCREEN_X + x * CELL_W as i32, y * CELL_H as i32, CELL_W, CELL_H);
        let inverse = c & 0x80 != 0;
        if inverse {
            self.canvas.set_draw_color(PHOSPHOR);
            let _ = self.canvas.fill_rect(dst);
        }
        let color = if inverse { BLACK } else { PHOSPHOR };
        self.draw_glyph(font, dst, c & 0x7f, color);
    }

    /// A glyph from the machine's own character generator at a pixel
    /// position, for the bay's labels.
    fn draw_glyph_at(&mut self, font: &mut Texture, px: i32, py: i32, c: u8, color: Color) {
        self.draw_glyph(font, Rect::new(px, py, CELL_W, CELL_H), c & 0x7f, color);
    }

    fn draw_glyph(&mut self, font: &mut Texture, dst: Rect, idx: u8, color: Color) {
        let idx = idx as i32;
        let src = Rect::new(
            (idx % ATLAS_GLYPHS_PER_ROW as i32) * GLYPH_W as i32,
            (idx / ATLAS_GLYPHS_PER_ROW as i32) * GLYPH_H as i32,
            GLYPH_W,
            GLYPH_H,
        );
        font.set_color_mod(color.r, color.g, color.b);
        let _ = self.canvas.copy(font, src, dst);
    }

    /// A click on a drive: the disk comes out, and the chooser goes up for
    /// the next one. With a chooser already up the click is ignored.
    fn open_door(&mut self, drive: usize) {
        if self.chooser.is_some() {
            return;
        }
        if self.drives.state.mounted(drive).is_some() {
            let _ = self.drives.control.change_disk(drive, None);
        }
        let prompt = format!("Insert a floppy in drive {}:", drive_letter(drive));
        // start where the images live, if that directory is there
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let start = if cwd.join("disks").is_dir() { cwd.join("disks") } else { cwd };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(choose_file(&prompt, &start));
        });
        self.chooser = Some((drive, rx));
    }

    /// The chooser's answer, if it has one this frame: open the image and
    /// hand it to the machine. A file that won't open leaves the drive
    /// empty and says why on the terminal; one the machine refuses does the
    /// same from its side.
    fn poll_chooser(&mut self) {
        let Some((drive, rx)) = &self.chooser else {
            return;
        };
        let picked = match rx.try_recv() {
            Ok(p) => p,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
        let drive = *drive;
        self.chooser = None;
        if let Some(path) = picked {
            match DiskImage::open(path) {
                Ok(image) => {
                    let _ = self.drives.control.change_disk(drive, Some(image));
                }
                Err(e) => eprintln!("drive {}: {e}", drive_letter(drive)),
            }
        }
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
                        // printable ascii only, which makes the KeyDown arm
                        // below the single source of every byte under 0x20:
                        // whatever text input delivers for Tab, Return or a
                        // ctrl-modified key on any given platform, the guest
                        // sees that byte once. It also keeps a non-ascii
                        // character from reaching the guest as its utf-8
                        // bytes -- one of which could be an arrow's 0xf1.
                        for b in text.bytes().filter(|b| (0x20..0x7f).contains(b)) {
                            if !self.send(b) {
                                shutdown.store(true, Ordering::SeqCst);
                                return;
                            }
                        }
                    }
                    Event::KeyDown { keycode: Some(key), keymod, .. } => {
                        if key == Keycode::D && keymod.intersects(CTRL) {
                            println!("ctrl-d hit on SDL console, exiting");
                            shutdown.store(true, Ordering::SeqCst);
                            return;
                        }
                        if let Some(c) = keyboard_code(key, keymod) {
                            if !self.send(c) {
                                shutdown.store(true, Ordering::SeqCst);
                                return;
                            }
                        }
                    }
                    Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                        if let Some(drive) = drive_at(x, y) {
                            self.open_door(drive);
                        }
                    }
                    _ => {}
                }
            }

            self.poll_chooser();
            // every frame, not just on a dirty screen: the LEDs are a duty
            // over the frame, and the labels change under the doors
            self.video.take_dirty();
            self.render(&mut font);

            std::thread::sleep(FRAME_DELAY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arrows send the keyboard's own codes, not the cursor controls the
    /// BIOS turns them into -- 0xf1..0xf4 in up, down, left, right order,
    /// which is the order the BIOS table's ^K ^J ^H ^L are in.
    #[test]
    fn the_arrow_keys_send_the_raw_keyboard_codes() {
        let none = Mod::NOMOD;
        assert_eq!(keyboard_code(Keycode::Up, none), Some(0xf1));
        assert_eq!(keyboard_code(Keycode::Down, none), Some(0xf2));
        assert_eq!(keyboard_code(Keycode::Left, none), Some(0xf3));
        assert_eq!(keyboard_code(Keycode::Right, none), Some(0xf4));
    }

    #[test]
    fn the_named_keys_send_the_codes_on_their_caps() {
        let none = Mod::NOMOD;
        assert_eq!(keyboard_code(Keycode::Backspace, none), Some(0x08));
        assert_eq!(keyboard_code(Keycode::Tab, none), Some(0x09));
        assert_eq!(keyboard_code(Keycode::Return, none), Some(0x0d));
        assert_eq!(keyboard_code(Keycode::KpEnter, none), Some(0x0d));
        assert_eq!(keyboard_code(Keycode::Escape, none), Some(0x1b));
        assert_eq!(keyboard_code(Keycode::Delete, none), Some(0x7f));
    }

    /// The keyboard's LINE FEED key, which shift-Return stands in for.
    #[test]
    fn shift_return_is_the_line_feed_key() {
        assert_eq!(keyboard_code(Keycode::Return, Mod::LSHIFTMOD), Some(0x0a));
    }

    #[test]
    fn ctrl_letters_reach_the_guest() {
        assert_eq!(keyboard_code(Keycode::C, Mod::LCTRLMOD), Some(0x03));
        assert_eq!(keyboard_code(Keycode::S, Mod::RCTRLMOD), Some(0x13));
        assert_eq!(keyboard_code(Keycode::Z, Mod::LCTRLMOD), Some(0x1a));
        // ctrl-@ and ctrl-space are both the null
        assert_eq!(keyboard_code(Keycode::SPACE, Mod::LCTRLMOD), Some(0x00));
        assert_eq!(keyboard_code(Keycode::NUM_2, Mod::LCTRLMOD | Mod::LSHIFTMOD), Some(0x00));
        assert_eq!(keyboard_code(Keycode::LEFTBRACKET, Mod::LCTRLMOD), Some(0x1b));
        assert_eq!(keyboard_code(Keycode::NUM_6, Mod::LCTRLMOD | Mod::LSHIFTMOD), Some(0x1e));
        assert_eq!(keyboard_code(Keycode::MINUS, Mod::LCTRLMOD | Mod::LSHIFTMOD), Some(0x1f));
    }

    /// Unmodified printable keys are text input's job -- mapping them here
    /// too would send every keystroke twice.
    /// A live event carries the lock bits too -- every modifier test here is
    /// an `intersects`, so they pass through.
    #[test]
    fn the_lock_modifiers_do_not_disturb_the_mapping() {
        let live = Mod::LCTRLMOD | Mod::NUMMOD | Mod::CAPSMOD;
        assert_eq!(keyboard_code(Keycode::C, live), Some(0x03));
        assert_eq!(keyboard_code(Keycode::Up, Mod::NUMMOD | Mod::CAPSMOD), Some(0xf1));
        assert_eq!(keyboard_code(Keycode::Return, Mod::LSHIFTMOD | Mod::NUMMOD), Some(0x0a));
    }

    #[test]
    fn printable_keys_are_left_to_text_input() {
        assert_eq!(keyboard_code(Keycode::A, Mod::NOMOD), None);
        assert_eq!(keyboard_code(Keycode::NUM_1, Mod::LSHIFTMOD), None);
        assert_eq!(keyboard_code(Keycode::Kp5, Mod::NOMOD), None);
    }

    /// The faces stack in the bay to the right of the screen, A above B
    /// with room for each one's label, and a click lands on the drive
    /// whose face it is in.
    #[test]
    fn the_drive_faces_stack_beside_the_screen_and_take_clicks() {
        let a = face_rect(0);
        let b = face_rect(1);
        assert!(a.left() >= SCREEN_X + SCREEN_W as i32);
        assert_eq!(a.left(), b.left());
        assert!(a.right() <= WINDOW_W as i32);
        assert!(label_y(0) + CELL_H as i32 <= b.top(), "A's label clears B");
        assert!(label_y(1) + CELL_H as i32 <= WINDOW_H as i32, "B's label fits the window");
        assert_eq!(drive_at(a.center().x(), a.center().y()), Some(0));
        assert_eq!(drive_at(b.center().x(), b.center().y()), Some(1));
        assert_eq!(drive_at(SCREEN_X + 10, 10), None, "the screen is not a door");
        assert_eq!(drive_at(a.center().x(), label_y(0) + 4), None, "nor is the label");
    }

    /// The light shows the drive's on-time duty over the frame, and a
    /// frame with no cycles in it shows it off.
    #[test]
    fn the_led_duty_is_the_selected_fraction_of_the_frame() {
        let prev = DriveBaySnapshot { select_on: [100, 0], cycles: 1000 };
        let now = DriveBaySnapshot { select_on: [350, 0], cycles: 2000 };
        assert!((led_duty(&now, &prev, 0) - 0.25).abs() < 1e-6);
        assert_eq!(led_duty(&now, &prev, 1), 0.0);
        assert_eq!(led_duty(&prev, &prev, 0), 0.0, "no cycles: off");
    }

    /// Labels are the file name, cut to the column with an ellipsis, or
    /// the empty-drive text.
    #[test]
    fn labels_fit_the_bay() {
        assert_eq!(label_for(None), "(empty)");
        assert_eq!(label_for(Some(Path::new("disks/mbasic-games.img"))), "mbasic-games.img");
        let long = "a-very-long-floppy-image-name-indeed.img";
        let label = label_for(Some(Path::new(long)));
        assert_eq!(label.chars().count(), LABEL_CHARS);
        assert!(label.ends_with("..."));
        assert!(long.starts_with(&label[..LABEL_CHARS - 3]));
    }
}
