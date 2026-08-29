// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! Kaypro II (Zilog Z80, CP/M, video window).
//!
//! Port of `system/system_kaypro.cpp`, minus the rendering, which lives in
//! `console::sdl`. The machine here is the bus: a 64K ram, a 4K boot rom and
//! 4K of video ram bank-switched over the bottom of it, a Z80 SIO whose
//! channel B is the keyboard, a WD1793 floppy controller reading a raw disk
//! image, and the multi-function latch at port `0x1c`.
//!
//! What's *not* here: the video ram is a [`VideoBuffer`] shared with the
//! frontend rather than a private `Memory`, and the frontend gets it (plus the
//! character generator rom, which the CPU never sees) through the [`Display`]
//! the factory returns alongside the bus.

use crate::bus::{Bus, MemoryDevice};
use crate::console::{ConsoleEndpoint, Display, VideoBuffer};
use crate::dev::memory::Memory;
use crate::dev::wd1793::Wd1793;
use crate::dev::z80sio::Z80Sio;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/kaypro/kayproii_u47.bin";
pub const VIDEO_ROM: &str = "roms/kaypro/kayproii_u43.bin";
/// Loaded from the current directory, as the C++ does. Not a rom, so not
/// tracked in the repo -- see AGENTS.md.
pub const DEFAULT_FLOPPY: &str = "mbasic-games.img";

pub const WINDOW_TITLE: &str = "Kaypro II Emulator";

const RAM_SIZE: usize = 64 * 1024;
const ROM_SIZE: usize = 4 * 1024;
const VIDEO_RAM_SIZE: usize = 4 * 1024;
const VIDEO_ROM_SIZE: usize = 2 * 1024;

const VIDEO_BASE: u16 = 0x3000;

// port 0x1c control latch bits
const LATCH_DRIVE_A_N: u8 = 1 << 0; // active low
const LATCH_DRIVE_B_N: u8 = 1 << 1; // active low
const LATCH_FDC_INTRQ_N: u8 = 1 << 6; // active low, read-only
const LATCH_FDC_DRQ_N: u8 = 1 << 7; // active low, read-only
const LATCH_BANK1: u8 = 1 << 7; // on write: rom/video bank in

pub struct Kaypro {
    ram: Memory,
    rom: Memory,
    video: VideoBuffer,
    console: ConsoleEndpoint,
    fdc: Wd1793,
    sio: Z80Sio,
    /// Port `0x1c`. Bit 7 selects the bank (set = rom + video ram visible),
    /// bits 0-1 are active-low drive selects, and on read bits 6-7 are the
    /// FDC's INTRQ/DRQ lines instead. Reset with the rom banked in.
    control_latch: u8,
}

/// Read a flat rom of exactly `size` bytes; a short file is an error, as in
/// the C++.
fn load_exact(path: &Path, size: usize, what: &str) -> io::Result<Vec<u8>> {
    let image = rom::load_binary(path).map_err(|e| {
        io::Error::new(e.kind(), format!("could not open {what} {}: {e}", path.display()))
    })?;
    if image.len() < size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("reading {what} {}: read {} bytes, expected {size}", path.display(), image.len()),
        ));
    }
    Ok(image)
}

impl Kaypro {
    /// Build the machine. Returns the bus and the display handle the frontend
    /// needs.
    pub fn new(
        rom_path: &Path,
        video_rom_path: &Path,
        floppy_path: &Path,
        console: ConsoleEndpoint,
    ) -> io::Result<(Self, Display)> {
        let rom_image = load_exact(rom_path, ROM_SIZE, "main ROM")?;
        println!("Kaypro: loaded {ROM_SIZE} bytes from main ROM {}", rom_path.display());
        let mut font_rom = load_exact(video_rom_path, VIDEO_ROM_SIZE, "video ROM")?;
        font_rom.truncate(VIDEO_ROM_SIZE);
        println!("Kaypro: loaded {VIDEO_ROM_SIZE} bytes from video ROM {}", video_rom_path.display());

        let mut rom = Memory::new(ROM_SIZE);
        rom.load_at(0, &rom_image[..ROM_SIZE]);

        let mut fdc = Wd1793::new();
        fdc.load_image(floppy_path);

        let video = VideoBuffer::new(VIDEO_RAM_SIZE);
        let display = Display::CharCell { title: WINDOW_TITLE, video: video.clone(), font_rom };

        let sys = Kaypro {
            ram: Memory::new(RAM_SIZE),
            rom,
            video,
            console,
            fdc,
            sio: Z80Sio::new(),
            control_latch: LATCH_BANK1,
        };
        Ok((sys, display))
    }

    fn bank1(&self) -> bool {
        self.control_latch & LATCH_BANK1 != 0
    }

    fn drive_a_selected(&self) -> bool {
        self.control_latch & LATCH_DRIVE_A_N == 0
    }

    fn drive_b_selected(&self) -> bool {
        self.control_latch & LATCH_DRIVE_B_N == 0
    }

    /// Feed queued keystrokes to the SIO's keyboard channel.
    ///
    /// The C++ does this from a console-thread callback the moment a key
    /// arrives; here it happens on the CPU thread at the point of the SIO
    /// access, which is the only place the guest could observe the difference.
    fn poll_keyboard(&mut self) {
        while let Some(c) = self.console.try_next_char() {
            self.sio.inject_char_b(c);
        }
    }

    /// The floppy is only ready while a drive is selected via the latch. As
    /// in the C++, either drive maps onto the single image.
    fn select_fdc(&mut self) {
        let selected = self.drive_a_selected() || self.drive_b_selected();
        self.fdc.set_selected(selected);
    }
}

impl Bus for Kaypro {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = (addr & 0xffff) as u16;
        if self.bank1() && addr < ROM_SIZE as u16 {
            self.rom.read_byte(addr as u32)
        } else if self.bank1() && (VIDEO_BASE..VIDEO_BASE + VIDEO_RAM_SIZE as u16).contains(&addr) {
            self.video.read((addr - VIDEO_BASE) as usize)
        } else {
            self.ram.read_byte(addr as u32)
        }
    }

    fn write8(&mut self, addr: u32, val: u8) {
        let addr = (addr & 0xffff) as u16;
        if self.bank1() {
            if addr < ROM_SIZE as u16 {
                // writes to the rom window are dropped
                return;
            }
            if (VIDEO_BASE..VIDEO_BASE + VIDEO_RAM_SIZE as u16).contains(&addr) {
                self.video.write((addr - VIDEO_BASE) as usize, val);
                return;
            }
        }
        // all of bank 0, or the unmapped parts of bank 1
        self.ram.write_byte(addr as u32, val);
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        match port & 0xff {
            // serial port A: data, control
            0x04 => self.sio.read_data_a(),
            0x06 => self.sio.read_control_a(),
            // serial port B (keyboard): data, control
            0x05 => {
                self.poll_keyboard();
                self.sio.read_data_b()
            }
            0x07 => {
                self.poll_keyboard();
                self.sio.read_control_b()
            }
            // floppy: status, track, sector, data
            0x10..=0x13 => {
                self.select_fdc();
                self.fdc.read((port & 0x03) as u8)
            }
            // system port: the last-written output bits plus the FDC's
            // active-low INTRQ/DRQ inputs
            0x1c => {
                let mut val = self.control_latch & 0x3f;
                if !self.fdc.interrupt_pending() {
                    val |= LATCH_FDC_INTRQ_N;
                }
                if !self.fdc.data_ready() {
                    val |= LATCH_FDC_DRQ_N;
                }
                val
            }
            _ => 0,
        }
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        match port & 0xff {
            // baud rate generators A and B: don't care
            0x00 | 0x0c => {}
            0x04 => self.sio.write_data_a(val),
            0x06 => self.sio.write_control_a(val),
            0x05 => self.sio.write_data_b(val),
            0x07 => self.sio.write_control_b(val),
            // PIO 1: unmodelled
            0x08..=0x0b => {}
            0x10..=0x13 => {
                self.select_fdc();
                self.fdc.write((port & 0x03) as u8, val);
            }
            // bank register and floppy PIO: unmodelled
            0x14..=0x17 => {}
            0x1c => self.control_latch = val,
            // PIO 2 control and channel B: unmodelled
            0x1d..=0x1f => {}
            _ => eprintln!("out to unknown port {port:#x}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture {
        dir: std::path::PathBuf,
        rom: std::path::PathBuf,
        video_rom: std::path::PathBuf,
        floppy: std::path::PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("emu-kaypro-test-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let rom = dir.join("rom.bin");
            let video_rom = dir.join("video.bin");
            let floppy = dir.join("floppy.img");
            // rom byte i = i, so reads are recognisable
            let rom_bytes: Vec<u8> = (0..ROM_SIZE).map(|i| i as u8).collect();
            std::fs::File::create(&rom).unwrap().write_all(&rom_bytes).unwrap();
            std::fs::File::create(&video_rom).unwrap().write_all(&vec![0xaa; VIDEO_ROM_SIZE]).unwrap();
            std::fs::File::create(&floppy).unwrap().write_all(&vec![0x11; 512 * 10 * 40]).unwrap();
            Fixture { dir, rom, video_rom, floppy }
        }

        fn build(&self) -> (Kaypro, Display) {
            let (_tx, rx) = std::sync::mpsc::channel();
            let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
            Kaypro::new(&self.rom, &self.video_rom, &self.floppy, console).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    #[test]
    fn resets_with_the_rom_banked_in() {
        let fx = Fixture::new("reset");
        let (mut sys, _) = fx.build();
        assert_eq!(sys.read8(0x0000), 0x00);
        assert_eq!(sys.read8(0x0123), 0x23);
        // and rom writes are dropped
        sys.write8(0x0123, 0x99);
        assert_eq!(sys.read8(0x0123), 0x23);
    }

    #[test]
    fn bank_switch_exposes_ram_underneath() {
        let fx = Fixture::new("bank");
        let (mut sys, _) = fx.build();
        // bank 0: the whole 64K is ram, including under the rom and video
        sys.io_write8(0x1c, 0x00);
        sys.write8(0x0123, 0x99);
        sys.write8(0x3000, 0x77);
        assert_eq!(sys.read8(0x0123), 0x99);
        assert_eq!(sys.read8(0x3000), 0x77);
        // back to bank 1: rom and (still zero) video ram are in front
        sys.io_write8(0x1c, 0x80);
        assert_eq!(sys.read8(0x0123), 0x23);
        assert_eq!(sys.read8(0x3000), 0x00);
        // and the ram is still there behind them
        sys.io_write8(0x1c, 0x00);
        assert_eq!(sys.read8(0x0123), 0x99);
    }

    #[test]
    fn video_writes_land_in_the_shared_buffer_and_dirty_it() {
        let fx = Fixture::new("video");
        let (mut sys, display) = fx.build();
        let Display::CharCell { video, .. } = display else {
            panic!("kaypro builds a char-cell display");
        };
        assert!(!video.take_dirty());
        sys.write8(0x3000 + 128 * 2 + 5, b'K');
        assert!(video.take_dirty());
        assert_eq!(video.read(128 * 2 + 5), b'K');
        assert_eq!(sys.read8(0x3000 + 128 * 2 + 5), b'K');
        // the top of the video window is the last video byte, not ram
        sys.write8(0x3fff, 0x42);
        assert_eq!(video.read(0xfff), 0x42);
        // and the byte past it is plain ram
        sys.write8(0x4000, 0x43);
        assert_eq!(video.read(0), 0x00);
    }

    #[test]
    fn display_carries_the_font_rom() {
        let fx = Fixture::new("font");
        let (_, display) = fx.build();
        let Display::CharCell { title, font_rom, .. } = display else {
            panic!("kaypro builds a char-cell display");
        };
        assert_eq!(font_rom.len(), VIDEO_ROM_SIZE);
        assert!(font_rom.iter().all(|&b| b == 0xaa));
        assert_eq!(title, WINDOW_TITLE);
    }

    #[test]
    fn keyboard_arrives_on_sio_channel_b() {
        let fx = Fixture::new("kbd");
        let (tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &fx.floppy, console).unwrap();
        assert_eq!(sys.io_read8(0x07) & 0x01, 0);
        tx.send(b'x').unwrap();
        assert_ne!(sys.io_read8(0x07) & 0x01, 0);
        assert_eq!(sys.io_read8(0x05), b'x');
        assert_eq!(sys.io_read8(0x07) & 0x01, 0);
    }

    #[test]
    fn latch_reads_back_fdc_lines_and_selects_drives() {
        let fx = Fixture::new("latch");
        let (mut sys, _) = fx.build();
        // idle: no intrq, no drq -> both active-low bits set
        assert_eq!(sys.io_read8(0x1c) & 0xc0, 0xc0);
        // no drive selected (bits 0-1 high after writing 0x83): not ready
        sys.io_write8(0x1c, 0x83);
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
        // select drive A: ready
        sys.io_write8(0x1c, 0x82);
        assert_eq!(sys.io_read8(0x10) & 0x80, 0);
        // read a sector: drq goes low in the latch, and data streams
        sys.io_write8(0x10, 0x80);
        assert_eq!(sys.io_read8(0x1c) & 0x80, 0);
        assert_eq!(sys.io_read8(0x13), 0x11);
        // the latch echoes the low output bits
        assert_eq!(sys.io_read8(0x1c) & 0x3f, 0x02);
    }

    #[test]
    fn missing_roms_are_errors() {
        let fx = Fixture::new("missing");
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let bad = fx.dir.join("nope.bin");
        assert!(Kaypro::new(&bad, &fx.video_rom, &fx.floppy, console).is_err());
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        assert!(Kaypro::new(&fx.rom, &bad, &fx.floppy, console).is_err());
        // a missing floppy is not: the controller just reports not-ready
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &bad, console).unwrap();
        sys.io_write8(0x1c, 0x82);
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
    }
}
