// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
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
/// Mounted from `disks/` under the current directory. Not a rom, so not
/// tracked in the repo -- see disks/README.md.
pub const DEFAULT_FLOPPY: &str = "disks/mbasic-games.img";

pub const WINDOW_TITLE: &str = "Kaypro II Emulator";

const RAM_SIZE: usize = 64 * 1024;
const ROM_SIZE: usize = 4 * 1024;
const VIDEO_RAM_SIZE: usize = 4 * 1024;
const VIDEO_ROM_SIZE: usize = 2 * 1024;

const VIDEO_BASE: u16 = 0x3000;

// port 0x1c control latch bits
const LATCH_DRIVE_A_N: u8 = 1 << 0; // active low
const LATCH_DRIVE_B_N: u8 = 1 << 1; // active low
const LATCH_BANK1: u8 = 1 << 7; // rom/video bank in

pub struct Kaypro {
    ram: Memory,
    rom: Memory,
    video: VideoBuffer,
    console: ConsoleEndpoint,
    fdc: Wd1793,
    sio: Z80Sio,
    /// Port `0x1c`, a read/write system port. Bit 7 selects the bank (set =
    /// rom + video ram visible), bits 0-1 are active-low drive selects, and
    /// bit 6 is the active-low drive-motor control. Reset with the rom
    /// banked in. Reads return the byte as written -- the BIOS depends on
    /// that, see `io_read8`.
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
            // System port: reads back exactly what was written. The BIOS
            // keeps the drive-motor state in the latch itself -- before
            // every disk operation it reads the port, tests bit 6, and only
            // when it believes the motors are off pays a 50 x 16ms spin-up
            // delay. The C++ instead returned the FDC's INTRQ/DRQ lines in
            // bits 6-7 on read; since a force-interrupt precedes the check,
            // that always read "motors off" and every disk operation cost
            // 800ms of machine time -- invisible while the machine ran
            // uncapped, and a two-minute program load under --throttle.
            // Nothing polls INTRQ/DRQ here: the BIOS waits for the disk
            // with HALT, woken by the lines' NMI gate on real hardware
            // (and falling through to a busy-poll of the status register
            // under this core's HALT-is-a-NOP quirk).
            0x1c => self.control_latch,
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

    /// The system port reads back the byte as written, the motor bit
    /// included. This is load-bearing for speed, not just fidelity: the
    /// BIOS keeps its drive-motor state in the latch (bit 6, active low)
    /// and pays a 800ms spin-up delay whenever a readback claims the
    /// motors are off. A model that substitutes FDC lines into bits 6-7 on
    /// read -- as the C++ did -- makes it pay that delay before every disk
    /// operation, which stretched CP/M's boot to 32 machine-seconds and a
    /// 40KB program load to two minutes.
    #[test]
    fn the_latch_reads_back_what_was_written_and_selects_drives() {
        let fx = Fixture::new("latch");
        let (mut sys, _) = fx.build();
        sys.io_write8(0x1c, 0xc3); // motors off (bit 6 set)
        assert_eq!(sys.io_read8(0x1c), 0xc3);
        sys.io_write8(0x1c, 0x83); // motors on
        assert_eq!(
            sys.io_read8(0x1c),
            0x83,
            "the motor bit must survive the readback or the BIOS re-pays the spin-up delay"
        );
        // no drive selected (bits 0-1 high): not ready
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
        // select drive A: ready, and a sector read streams data
        sys.io_write8(0x1c, 0x82);
        assert_eq!(sys.io_read8(0x10) & 0x80, 0);
        sys.io_write8(0x10, 0x80);
        assert_eq!(sys.io_read8(0x13), 0x11);
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
