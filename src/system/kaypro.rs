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
//! channel B is the keyboard, a WD1793 floppy controller with drives A and
//! B reading raw disk images, and the multi-function latch at port `0x1c`.
//!
//! What's *not* here: the video ram is a [`VideoBuffer`] shared with the
//! frontend rather than a private `Memory`, and the frontend gets it (plus the
//! character generator rom, which the CPU never sees) through the [`Display`]
//! the factory returns alongside the bus. The drive bay beside the screen is
//! the frontend's too: the controller publishes each drive's LED and label
//! into the [`DriveBayDisplay`] the factory hands in, and the window's drive
//! doors come back as `Bus::change_disk` over the run loop's channel.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::{ConsoleEndpoint, DiskImage, Display, DriveBayDisplay, VideoBuffer};
use crate::dev::memory::Memory;
use crate::dev::wd1793::{drive_letter, Wd1793, DRIVES, IMAGE_SIZE};
use crate::dev::z80sio::{Ch, Z80Sio};
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/kaypro/kayproii_u47.bin";
pub const VIDEO_ROM: &str = "roms/kaypro/kayproii_u43.bin";
/// Mounted in drive A from `disks/` under the current directory. Not a
/// rom, so not tracked in the repo -- see disks/README.md. Drive B starts
/// empty; the window's door fills it.
pub const DEFAULT_FLOPPY: &str = "disks/mbasic-games.img";

pub const WINDOW_TITLE: &str = "Kaypro II Emulator";

/// The Z80's clock.
pub const CLOCK_HZ: u64 = 2_500_000;

/// The COM8116 baud rate generators on ports `0x00` (serial) and `0x0c`
/// (keyboard): the rate each four-bit code selects, from the part's table.
/// The generator's output is sixteen times the baud, which the rom's WR4
/// (x16) divides back down.
const COM8116_BAUD: [u64; 16] = [
    50, 75, 110, 134, 150, 300, 600, 1200, 1800, 2000, 2400, 3600, 4800, 7200, 9600, 19200,
];

const RAM_SIZE: usize = 64 * 1024;
const ROM_SIZE: usize = 4 * 1024;
const VIDEO_RAM_SIZE: usize = 4 * 1024;
const VIDEO_ROM_SIZE: usize = 2 * 1024;

const VIDEO_BASE: u16 = 0x3000;

// port 0x1c control latch bits. The drive selects are active high: the
// rom's select routine (at $049c) clears both bits, ors in the drive
// number plus one and writes it back, so A is bit 0 set and B bit 1 set.
// The motor bit is active low, and the rom's motor-off (set 6, at $04b5)
// leaves the selects as they were.
const LATCH_DRIVE_A: u8 = 1 << 0;
const LATCH_DRIVE_B: u8 = 1 << 1;
const LATCH_MOTOR_OFF: u8 = 1 << 6;
const LATCH_BANK1: u8 = 1 << 7; // rom/video bank in

pub struct Kaypro {
    ram: Memory,
    rom: Memory,
    video: VideoBuffer,
    console: ConsoleEndpoint,
    fdc: Wd1793,
    sio: Z80Sio,
    /// Port `0x1c`, a read/write system port. Bit 7 selects the bank (set =
    /// rom + video ram visible), bits 0-1 are the drive selects, and bit 6
    /// is the active-low drive-motor control. Reset with the rom banked
    /// in. Reads return the byte as written -- the BIOS depends on that,
    /// see `io_read8`.
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
    /// Build the machine, with `floppy_path` in drive A. Returns the bus
    /// and the display handle the frontend needs, the drive bay inside it.
    pub fn new(
        rom_path: &Path,
        video_rom_path: &Path,
        floppy_path: &Path,
        console: ConsoleEndpoint,
        drives: DriveBayDisplay,
    ) -> io::Result<(Self, Display)> {
        let rom_image = load_exact(rom_path, ROM_SIZE, "main ROM")?;
        println!("Kaypro: loaded {ROM_SIZE} bytes from main ROM {}", rom_path.display());
        let mut font_rom = load_exact(video_rom_path, VIDEO_ROM_SIZE, "video ROM")?;
        font_rom.truncate(VIDEO_ROM_SIZE);
        println!("Kaypro: loaded {VIDEO_ROM_SIZE} bytes from video ROM {}", video_rom_path.display());

        let mut rom = Memory::new(ROM_SIZE);
        rom.load_at(0, &rom_image[..ROM_SIZE]);

        let mut fdc = Wd1793::new();
        fdc.attach_bay(drives.state.clone());
        fdc.load_image(0, floppy_path);

        let video = VideoBuffer::new(VIDEO_RAM_SIZE);
        let display = Display::CharCell { title: WINDOW_TITLE, video: video.clone(), font_rom, drives };

        let mut sys = Kaypro {
            ram: Memory::new(RAM_SIZE),
            rom,
            video,
            console,
            fdc,
            // the keyboard is a one-way serial line with no RTS input;
            // the rom sets the baud rates before it programs the channels
            sio: Z80Sio::new(CLOCK_HZ),
            control_latch: LATCH_BANK1,
        };
        sys.select_fdc();
        Ok((sys, display))
    }

    /// `--fast-io`: the serial ports complete instantly in both directions.
    /// The floppy keeps its byte time, which is what the rom's HALT loops
    /// need -- see `dev/wd1793`.
    pub fn set_fast_io(&mut self) {
        self.sio.set_fast_io();
    }

    fn bank1(&self) -> bool {
        self.control_latch & LATCH_BANK1 != 0
    }

    fn drive_a_selected(&self) -> bool {
        self.control_latch & LATCH_DRIVE_A != 0
    }

    fn drive_b_selected(&self) -> bool {
        self.control_latch & LATCH_DRIVE_B != 0
    }

    fn motor_on(&self) -> bool {
        self.control_latch & LATCH_MOTOR_OFF == 0
    }

    /// Hand queued keystrokes to the keyboard end of SIO channel B's line;
    /// the SIO clocks them in at the keyboard's 300 baud from there.
    fn poll_keyboard(&mut self) {
        while let Some(c) = self.console.try_next_char() {
            self.sio.receive(Ch::B, c);
        }
    }

    /// The latch's drive selects and motor bit, to the controller: A, else
    /// B, else none (not ready). Follows every latch write, so the selected
    /// drive's light is lit for exactly as long as the latch says.
    fn select_fdc(&mut self) {
        let drive = if self.drive_a_selected() {
            Some(0)
        } else if self.drive_b_selected() {
            Some(1)
        } else {
            None
        };
        self.fdc.select(drive, self.motor_on());
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

    /// The same decode as `read8`, which has no side effects on this
    /// machine -- the video ram is a locked buffer, the floppy and SIO are
    /// on ports.
    /// IRQ is the SIO's INT: the rom programs both channels with WR1 = 0,
    /// so the line is wired but nothing on this machine ever requests
    /// service. NMI is the floppy controller's INTRQ or DRQ, and that one
    /// is the disk system: the rom's transfer loops sleep in HALT for each
    /// byte's DRQ and for the completion, with a RET planted at 0x0066 to
    /// return them past the HALT. Keystrokes reach the keyboard's line, and
    /// the SIO's frames and the floppy's bytes advance here, once an
    /// instruction.
    fn poll_interrupts(&mut self, elapsed_cycles: u32) -> IntStatus {
        self.poll_keyboard();
        self.sio.tick(elapsed_cycles);
        self.fdc.tick(elapsed_cycles);
        IntStatus { irq: self.sio.int_pending(), nmi: self.fdc.nmi_line() }
    }

    fn interrupt_acknowledge(&mut self) -> u8 {
        self.sio.acknowledge()
    }

    fn interrupt_return(&mut self) {
        self.sio.reti();
    }

    fn set_device_pacing_hz(&mut self, hz: u64) {
        self.sio.set_pacing_hz(hz);
        self.fdc.set_pacing_hz(hz);
    }

    /// A drive door: the disk in `drive` changes under the controller,
    /// mid-transfer or not, as a real one would. An image has to be
    /// exactly one Kaypro II floppy's worth of bytes, since anything else
    /// is not a disk this drive can read and would only look like one
    /// until the first sector.
    fn change_disk(&mut self, drive: usize, image: Option<DiskImage>) -> Result<(), String> {
        if drive >= DRIVES {
            return Err(format!("no drive {drive}; the Kaypro II has A and B"));
        }
        match &image {
            Some(i) => {
                let len = i.file.metadata().map_err(|e| format!("{}: {e}", i.path.display()))?.len();
                if len != IMAGE_SIZE {
                    return Err(format!(
                        "{}: {len} bytes is not a Kaypro II image (40 tracks x 10 sectors x 512 bytes = {IMAGE_SIZE})",
                        i.path.display()
                    ));
                }
                println!("Kaypro: drive {}: inserted '{}'", drive_letter(drive), i.path.display());
            }
            None => println!("Kaypro: drive {}: ejected", drive_letter(drive)),
        }
        self.fdc.mount(drive, image);
        Ok(())
    }

    fn peek8(&self, addr: u32) -> Option<u8> {
        let addr = (addr & 0xffff) as u16;
        Some(if self.bank1() && addr < ROM_SIZE as u16 {
            self.rom.peek(addr as u32)
        } else if self.bank1() && (VIDEO_BASE..VIDEO_BASE + VIDEO_RAM_SIZE as u16).contains(&addr) {
            self.video.read((addr - VIDEO_BASE) as usize)
        } else {
            self.ram.peek(addr as u32)
        })
    }

    fn io_read8(&mut self, port: u16) -> u8 {
        match port & 0xff {
            // serial port A: data, control
            0x04 => self.sio.read_data(Ch::A),
            0x06 => self.sio.read_control(Ch::A),
            // serial port B (keyboard): data, control
            0x05 => self.sio.read_data(Ch::B),
            0x07 => self.sio.read_control(Ch::B),
            // floppy: status, track, sector, data
            0x10..=0x13 => self.fdc.read((port & 0x03) as u8),
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
            // with HALT, woken by the lines' NMI gate (`poll_interrupts`).
            0x1c => self.control_latch,
            _ => 0,
        }
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        match port & 0xff {
            // baud rate generators: the serial port's and the keyboard's
            0x00 => self.sio.set_clock_hz(Ch::A, 16 * COM8116_BAUD[(val & 0x0f) as usize]),
            0x0c => self.sio.set_clock_hz(Ch::B, 16 * COM8116_BAUD[(val & 0x0f) as usize]),
            // serial port A transmits into the ether; the keyboard channel
            // has nothing listening either
            0x04 => self.sio.write_data(Ch::A, val),
            0x06 => self.sio.write_control(Ch::A, val),
            0x05 => self.sio.write_data(Ch::B, val),
            0x07 => self.sio.write_control(Ch::B, val),
            // PIO 1: unmodelled
            0x08..=0x0b => {}
            0x10..=0x13 => self.fdc.write((port & 0x03) as u8, val),
            // bank register and floppy PIO: unmodelled
            0x14..=0x17 => {}
            0x1c => {
                self.control_latch = val;
                self.select_fdc();
            }
            // PIO 2 control and channel B: unmodelled
            0x1d..=0x1f => {}
            _ => eprintln!("out to unknown port {port:#x}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{DriveBayState, DriveControl};
    use std::io::Write;

    /// A bay with nobody at its doors.
    fn bay() -> DriveBayDisplay {
        DriveBayDisplay { state: DriveBayState::new(), control: DriveControl(std::sync::mpsc::channel().0) }
    }

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
            Kaypro::new(&self.rom, &self.video_rom, &self.floppy, console, bay()).unwrap()
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

    /// Program the keyboard channel the way the rom's table at $05c2 does:
    /// reset, 300 baud on the generator, x16 clock with one stop bit, eight
    /// bits in with the receiver on, RTS off, and no interrupts.
    fn init_keyboard(sys: &mut Kaypro) {
        sys.io_write8(0x07, 0x18);
        sys.io_write8(0x0c, 0x05);
        for byte in [0x04, 0x44, 0x03, 0xc1, 0x05, 0xe8, 0x01, 0x00] {
            sys.io_write8(0x07, byte);
        }
    }

    /// A keystroke arrives on SIO channel B a frame time after it is typed
    /// -- 300 baud, ten bits, 83,333 cycles of the 2.5 MHz clock -- and
    /// asks for no interrupt, the rom's WR1 being zero. The keyboard has no
    /// RTS input, so the rom's RTS-off does not hold it.
    #[test]
    fn keyboard_arrives_on_sio_channel_b_at_300_baud() {
        let fx = Fixture::new("kbd");
        let (tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &fx.floppy, console, bay()).unwrap();
        init_keyboard(&mut sys);
        assert_eq!(sys.io_read8(0x07) & 0x01, 0);
        tx.send(b'x').unwrap();
        assert!(!sys.poll_interrupts(83_332).irq);
        assert_eq!(sys.io_read8(0x07) & 0x01, 0, "still on the wire");
        assert!(!sys.poll_interrupts(1).irq, "no interrupt asked for");
        assert_ne!(sys.io_read8(0x07) & 0x01, 0);
        assert_eq!(sys.io_read8(0x05), b'x');
        assert_eq!(sys.io_read8(0x07) & 0x01, 0);
    }

    /// The floppy controller's INTRQ and DRQ are the NMI line: a read
    /// sector raises it for each byte, a byte time apart, and reading the
    /// data register drops it again -- the edges the rom's `HALT; INI` loop
    /// sleeps for.
    #[test]
    fn the_floppy_drives_nmi_a_byte_at_a_time() {
        let fx = Fixture::new("nmi");
        let (mut sys, _) = fx.build();
        assert!(!sys.poll_interrupts(0).nmi);
        sys.io_write8(0x1c, 0x81); // bank in, drive a selected, motors on
        sys.io_write8(0x10, 0x88); // read sector
        assert!(!sys.poll_interrupts(79).nmi);
        assert!(sys.poll_interrupts(1).nmi, "drq");
        sys.io_read8(0x13);
        assert!(!sys.poll_interrupts(0).nmi, "taken");
        assert!(sys.poll_interrupts(80).nmi, "the next");
    }

    /// The baud rate generator's code sets the rate: 9600 is 260 cycles a
    /// character.
    #[test]
    fn the_baud_rate_generator_code_sets_the_keyboards_rate() {
        let fx = Fixture::new("baud");
        let (tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &fx.floppy, console, bay()).unwrap();
        init_keyboard(&mut sys);
        sys.io_write8(0x0c, 0x0e);
        tx.send(b'y').unwrap();
        sys.poll_interrupts(2_603);
        assert_eq!(sys.io_read8(0x07) & 0x01, 0);
        sys.poll_interrupts(1);
        assert_ne!(sys.io_read8(0x07) & 0x01, 0);
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
        sys.io_write8(0x1c, 0xc1); // motors off (bit 6 set)
        assert_eq!(sys.io_read8(0x1c), 0xc1);
        // and, motors off, the drive is not ready even though selected
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
        sys.io_write8(0x1c, 0x81); // motors on
        assert_eq!(
            sys.io_read8(0x1c),
            0x81,
            "the motor bit must survive the readback or the BIOS re-pays the spin-up delay"
        );
        // no drive selected (bits 0-1 clear): not ready
        sys.io_write8(0x1c, 0x80);
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
        // select drive A: ready, and a sector read streams data
        sys.io_write8(0x1c, 0x81);
        assert_eq!(sys.io_read8(0x10) & 0x80, 0);
        sys.io_write8(0x10, 0x80);
        sys.poll_interrupts(80); // the first byte's time
        assert_eq!(sys.io_read8(0x13), 0x11);
    }

    /// Drive B is its own slot: empty at power-on, filled by the door,
    /// read through the latch's second select bit; and the bay's light
    /// counters follow the select bits as the latch is written, gated by
    /// the motor bit.
    #[test]
    fn drive_b_is_a_second_slot_filled_by_the_door() {
        let fx = Fixture::new("driveb");
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let bay = bay();
        let state = bay.state.clone();
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &fx.floppy, console, bay).unwrap();
        assert_eq!(state.mounted(0), Some(fx.floppy.clone()));
        assert_eq!(state.mounted(1), None);
        sys.io_write8(0x1c, 0x82); // select B
        assert_ne!(sys.io_read8(0x10) & 0x80, 0, "empty");
        // a second image, distinguishable from A's 0x11
        let b_path = fx.dir.join("b.img");
        std::fs::File::create(&b_path).unwrap().write_all(&vec![0x22; 512 * 10 * 40]).unwrap();
        sys.change_disk(1, Some(DiskImage::open(b_path.clone()).unwrap())).unwrap();
        assert_eq!(state.mounted(1), Some(b_path));
        assert_eq!(sys.io_read8(0x10) & 0x80, 0);
        sys.io_write8(0x10, 0x80);
        sys.poll_interrupts(80);
        assert_eq!(sys.io_read8(0x13), 0x22);
        sys.io_write8(0x10, 0xd0);
        sys.io_write8(0x1c, 0x81); // select A
        sys.io_write8(0x10, 0x80);
        sys.poll_interrupts(80);
        assert_eq!(sys.io_read8(0x13), 0x11);
        sys.io_write8(0x10, 0xd0);
        // the lights: A was selected for the last 80 cycles and B for 80
        let s = state.snapshot();
        assert_eq!(s.select_on, [80, 80]);
        assert_eq!(s.cycles, 160);
        sys.io_write8(0x1c, 0x80); // neither
        sys.poll_interrupts(50);
        let s = state.snapshot();
        assert_eq!(s.select_on, [80, 80], "nothing lit");
        assert_eq!(s.cycles, 210);
        sys.io_write8(0x1c, 0xc1); // A selected, motors off: the rom's idle
        sys.poll_interrupts(50);
        let s = state.snapshot();
        assert_eq!(s.select_on, [80, 80], "the light goes out with the motors");
        assert_eq!(s.cycles, 260);
        // eject A: the mounted name goes, and the drive reads not ready
        sys.change_disk(0, None).unwrap();
        assert_eq!(state.mounted(0), None);
        sys.io_write8(0x1c, 0x81);
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
    }

    /// A door refuses a file that isn't one floppy's worth of bytes, or a
    /// drive the machine doesn't have, and the drive keeps what it had.
    #[test]
    fn the_door_refuses_what_is_not_a_floppy() {
        let fx = Fixture::new("refuse");
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let bay = bay();
        let state = bay.state.clone();
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &fx.floppy, console, bay).unwrap();
        let short = fx.dir.join("short.img");
        std::fs::write(&short, [0u8; 512]).unwrap();
        let err = sys.change_disk(0, Some(DiskImage::open(short).unwrap())).unwrap_err();
        assert!(err.contains("512 bytes"), "{err}");
        assert_eq!(state.mounted(0), Some(fx.floppy.clone()), "A keeps its disk");
        let err = sys.change_disk(2, None).unwrap_err();
        assert!(err.contains("no drive 2"), "{err}");
        assert!(sys.change_disk(1, Some(DiskImage::open(fx.floppy.clone()).unwrap())).is_ok());
        assert_eq!(state.mounted(1), Some(fx.floppy.clone()));
    }

    #[test]
    fn missing_roms_are_errors() {
        let fx = Fixture::new("missing");
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let bad = fx.dir.join("nope.bin");
        assert!(Kaypro::new(&bad, &fx.video_rom, &fx.floppy, console, bay()).is_err());
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        assert!(Kaypro::new(&fx.rom, &bad, &fx.floppy, console, bay()).is_err());
        // a missing floppy is not: the controller just reports not-ready
        let (_tx, rx) = std::sync::mpsc::channel();
        let console = ConsoleEndpoint::new(rx, Box::new(Vec::new()));
        let (mut sys, _) = Kaypro::new(&fx.rom, &fx.video_rom, &bad, console, bay()).unwrap();
        sys.io_write8(0x1c, 0x81);
        assert_ne!(sys.io_read8(0x10) & 0x80, 0);
    }
}
