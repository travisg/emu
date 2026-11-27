// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! MITS Altair 8800 (Intel 8080).
//!
//! The machine as it was most often filled out: 64K of RAM, an 88-2SIO
//! serial board at its factory address -- port 0 the console at 0x10/0x11,
//! port 1 at 0x12/0x13, each an MC6850 -- and the front panel's sense
//! switches, A15-A8, read at `IN 0xFF`. Like the 703 it has no ROM: the
//! operator toggled a bootstrap loader into the bottom of memory on the
//! front panel and ran it with a paper tape in the teletype's reader, and
//! the subsystems stand in for the operator:
//!
//! * `altair8800` keys in MITS's 88-2SIO bootstrap for BASIC 4.x (below)
//!   with the high address byte for 8K BASIC, and treats `-r` as the tape
//!   in the reader. The reader is on the console line, port 0, as an
//!   ASR-33's was: the tape's bytes reach the ACIA ahead of anything typed,
//!   as fast as the guest takes them, and the keyboard has the line once
//!   the tape has run out. The bootstrap loads the tape's checksum loader,
//!   which loads BASIC and starts it.
//! * `altair8800-4k` and `altair8800-ext` key in the same bootstrap for
//!   4K and Extended BASIC 4.x, whose checksum loaders sit lower and higher.
//! * `altair8800-image` loads `-r` as a memory image at 0 and runs it from
//!   there -- what an operator who had deposited it would have.
//! * `altair8800-disk` is the machine with the 88-DCDD floppy controller
//!   and its drives (`crate::dev::dcdd`) at ports 08h-0Ah, and MITS's disk
//!   boot loader in a 256-byte PROM at `0xff00`, which takes no writes:
//!   `-r` is the PROM (default `roms/altair8800/dbl.hex`, the DBL 4.1
//!   Martin Eberhard read off a MITS Turnkey board's EPROM). The operator
//!   started it by EXAMINE at `0xff00` and RUN; standing in, the subsystem
//!   keys `JMP 0FF00H` in at 0, which the loader's first sectors land on
//!   anyway. The loader copies itself to `0x2c00`, waits for drive 0,
//!   seeks track 0 and loads what the disk's first sectors say to.
//!   `altair8800-image-disk` puts the controller beside a memory image
//!   instead, with no PROM, for a program that drives the disk itself.
//!
//! Without a front panel the sense switches read all off, which BASIC 4.x
//! takes as console and load device both 88-2SIO port 0 (Martin Eberhard, *Loading Basic with
//! the 88-2SIO Board*, 2013). Both ports' interrupt outputs are jumpered to
//! the bus's PINT, as the 2SIO's jumpers allow, with no vectored interrupt
//! board: the pulled-up data bus answers INTA with `RST 7`. BASIC never
//! enables an ACIA interrupt, so the line only reaches a program that does.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::{AltairPanel, ConsoleEndpoint, DiskImage};
use crate::dev::dcdd::{self, Dcdd};
use crate::dev::mc6850::Mc6850;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/altair8800/basic8k40.tap";
/// The disk boot loader PROM, `altair8800-disk`'s `-r`.
pub const DBL_ROM: &str = "roms/altair8800/dbl.hex";
/// The floppy in drive 0 when `--disk` names none: Burcon CP/M 2.2 for 56K.
pub const DEFAULT_DISK: &str = "disks/altair8800-cpm.dsk";
/// Where the PROM board answers, and its size.
const PROM_BASE: u32 = 0xff00;
const PROM_SIZE: usize = 0x100;
/// `JMP 0FF00H`, keyed in at 0 to start the disk boot loader.
const JUMP_TO_PROM: [u8; 3] = [0xc3, 0x00, 0xff];
pub const SERIAL_PORT: &str = "88-2SIO port 1, ports 0x12/0x13";
/// The 8080A's 2 MHz, the 8800's only clock.
pub const CLOCK_HZ: u64 = 2_000_000;

/// MITS's bootstrap for BASIC 3.x and 4.x on an 88-2SIO, as Eberhard gives
/// it, in octal as it was toggled in: reset the ACIA, program it for eight
/// bits and one stop bit, then read bytes into memory downwards from
/// `LADDR`, skipping leader, until L reaches 0, and jump to what it loaded
/// -- the tape's checksum loader. The stack points at a word holding the
/// address of the read loop, so each `RNC`/`RZ`/`RNZ` returns into it.
#[rustfmt::skip]
const BOOTSTRAP: [u8; 26] = [
    0o076, 0o003,           // 000 mvi a,3      ACIA master reset
    0o323, 0o020,           // 002 out 0x10
    0o076, 0o025,           // 004 mvi a,0x15   /16, 8 bits, 1 stop bit
    0o323, 0o020,           // 006 out 0x10
    0o041, 0o302, 0o000,    // 010 lxi h,LADDR  H keyed by BASIC's size
    0o061, 0o032, 0o000,    // 013 lxi sp,032
    0o333, 0o020,           // 016 in 0x10
    0o017,                  // 020 rrc          receive data available
    0o320,                  // 021 rnc
    0o333, 0o021,           // 022 in 0x11
    0o275,                  // 024 cmp l        leader is LADDR's low byte
    0o310,                  // 025 rz
    0o055,                  // 026 dcr l
    0o167,                  // 027 mov m,a
    0o300,                  // 030 rnz
    0o351,                  // 031 pchl
];
/// The bootstrap's stack: the address of its read loop, at 032.
const BOOTSTRAP_STACK: [u8; 2] = [0o013, 0o000];
/// Where `LADDR`'s high byte sits in the bootstrap.
const LADDR_HIGH: usize = 0o012;
/// `LADDR`'s low byte, 302 for BASIC 4.x -- and so the byte a 4.x tape's
/// leader is punched with, which `CMP L` skips.
const LEADER: u8 = 0o302;

/// The tape in the reader: its bytes and how far it has been read.
struct Tape {
    bytes: Vec<u8>,
    pos: usize,
}

pub struct Altair8800 {
    ram: Memory,
    /// The 88-2SIO's two ports.
    sio: [Mc6850; 2],
    tape: Option<Tape>,
    /// A15-A8 on the front panel, when there is none to read them from.
    sense: u8,
    panel: Option<AltairPanel>,
    /// The disk boot loader's PROM at `0xff00`, under `disk`.
    prom: Option<Memory>,
    /// The floppy controller and its drives, under `disk`.
    disk: Option<Dcdd>,
}

/// A line with nothing on either end, for a port nobody has plugged in.
fn unplugged() -> ConsoleEndpoint {
    let (_tx, rx) = std::sync::mpsc::channel();
    ConsoleEndpoint::new(rx, Box::new(io::sink()))
}

impl Altair8800 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        let image = if rom_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("hex")) {
            // an Intel HEX file says where its bytes go: the PROM's are at
            // 0xff00, and anything else's are a memory image from 0
            let mut memory = vec![0u8; 0x10000];
            rom::load_ihex(rom_path, |addr, bytes| {
                for (i, b) in bytes.iter().enumerate() {
                    memory[(addr as usize + i) & 0xffff] = *b;
                }
            })?;
            if Self::boots_from_prom(subsystem) {
                memory.split_off(PROM_BASE as usize)
            } else {
                memory
            }
        } else {
            rom::load_binary(rom_path)?
        };
        Self::from_image(image, console, subsystem)
    }

    /// Whether `subsystem` is the disk machine with the boot loader PROM,
    /// whose `-r` is the PROM.
    pub fn boots_from_prom(subsystem: &str) -> bool {
        subsystem == "disk"
    }

    /// The machine around `image` -- the tape, or under `image` the
    /// memory image -- from bytes rather than a file, for a host with no
    /// files.
    pub fn from_image(image: Vec<u8>, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        let mut m = Altair8800 {
            ram: Memory::new(0x10000),
            sio: [Mc6850::raw(console), Mc6850::raw(unplugged())],
            tape: None,
            sense: 0,
            panel: None,
            prom: None,
            disk: None,
        };
        let laddr_high = match subsystem {
            "image" => {
                m.ram.load_at(0, &image);
                return Ok(m);
            }
            "disk" => {
                if image.len() > PROM_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("altair8800: a {}-byte PROM image; the board's is {PROM_SIZE}", image.len()),
                    ));
                }
                let mut prom = Memory::new(PROM_SIZE);
                prom.load_at(0, &image);
                m.prom = Some(prom);
                m.ram.load_at(0, &JUMP_TO_PROM);
                m.disk = Some(Dcdd::new());
                return Ok(m);
            }
            "image-disk" | "disk-image" => {
                m.ram.load_at(0, &image);
                m.disk = Some(Dcdd::new());
                return Ok(m);
            }
            "" | "8k" => 0o037,
            "4k" => 0o017,
            "ext" => 0o077,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "unknown altair8800 subsystem '{other}'; try 'altair8800' (8K BASIC), \
                         'altair8800-4k', 'altair8800-ext', 'altair8800-image', 'altair8800-disk' \
                         or 'altair8800-image-disk'"
                    ),
                ))
            }
        };
        if image.first() != Some(&LEADER) {
            eprintln!(
                "altair8800: the tape does not start with the 4.x leader ({LEADER:#04x}); \
                 the bootstrap will take it for the checksum loader"
            );
        }
        let mut boot = BOOTSTRAP;
        boot[LADDR_HIGH] = laddr_high;
        m.ram.load_at(0, &boot);
        m.ram.load_at(BOOTSTRAP.len(), &BOOTSTRAP_STACK);
        m.tape = Some(Tape { bytes: image, pos: 0 });
        Ok(m)
    }

    /// The front panel, whose A15-A8 toggles `IN 0xFF` reads from here on.
    pub fn attach_panel(&mut self, panel: AltairPanel) {
        self.panel = Some(panel);
    }

    /// Mount the image at `path` in floppy drive `drive`, as the drive's
    /// door would.
    pub fn mount_disk(&mut self, drive: usize, path: &Path) -> io::Result<()> {
        let image = DiskImage::open(path.to_path_buf()).map_err(io::Error::other)?;
        self.change_disk(drive, Some(image)).map_err(io::Error::other)
    }

    pub fn has_disk_controller(&self) -> bool {
        self.disk.is_some()
    }

    /// The host end of port 1, for `--serial`.
    pub fn attach_serial(&mut self, line: ConsoleEndpoint) {
        self.sio[1] = Mc6850::raw(line);
    }

    /// Port 0's next character: off the tape while there is any, then from
    /// the keyboard.
    fn poll_console(&mut self) {
        if let Some(tape) = &mut self.tape {
            if tape.pos < tape.bytes.len() {
                if self.sio[0].can_receive() {
                    self.sio[0].receive(tape.bytes[tape.pos]);
                    tape.pos += 1;
                }
                return;
            }
        }
        self.sio[0].poll();
    }
}

impl Bus for Altair8800 {
    fn read8(&mut self, addr: u32) -> u8 {
        let addr = addr & 0xffff;
        match &self.prom {
            Some(p) if addr >= PROM_BASE => p.peek(addr - PROM_BASE),
            _ => self.ram.read_byte(addr),
        }
    }

    /// The PROM takes no writes, and the RAM board under it is not
    /// addressed there.
    fn write8(&mut self, addr: u32, val: u8) {
        let addr = addr & 0xffff;
        if self.prom.is_none() || addr < PROM_BASE {
            self.ram.write_byte(addr, val);
        }
    }

    fn peek8(&self, addr: u32) -> Option<u8> {
        let addr = addr & 0xffff;
        Some(match &self.prom {
            Some(p) if addr >= PROM_BASE => p.peek(addr - PROM_BASE),
            _ => self.ram.peek(addr),
        })
    }

    /// A debugger write to the PROM would be dropped, as the guest's is.
    fn poke8(&mut self, addr: u32, val: u8) -> Result<(), String> {
        if self.prom.is_some() && addr & 0xffff >= PROM_BASE {
            return Err(format!("rom at {:04x}", addr & 0xffff));
        }
        self.write8(addr, val);
        Ok(())
    }

    /// The boards decode the low byte of the address, where the 8080 puts
    /// the port number (it is on both halves). Nothing answers an unused
    /// port, and the bus floats high.
    fn io_read8(&mut self, port: u16) -> u8 {
        match port as u8 {
            p @ 0x10..=0x13 => self.sio[(p as usize - 0x10) >> 1].read_byte(p as u32 & 1),
            p @ 0x08..=0x0a if self.disk.is_some() => self.disk.as_mut().map_or(0xff, |d| d.read(p - 0x08)),
            0xff => self.panel.as_ref().map_or(self.sense, |p| (p.switches() >> 8) as u8),
            _ => 0xff,
        }
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        match port as u8 {
            p @ 0x10..=0x13 => self.sio[(p as usize - 0x10) >> 1].write_byte(p as u32 & 1, val),
            p @ 0x08..=0x0a => {
                if let Some(d) = &mut self.disk {
                    d.write(p - 0x08, val);
                }
            }
            _ => {}
        }
    }

    /// The disk's sector-true interrupt is on PINT with the 2SIO's.
    fn poll_interrupts(&mut self, elapsed_cycles: u32) -> IntStatus {
        self.poll_console();
        self.sio[1].poll();
        let disk = self.disk.as_mut().is_some_and(|d| {
            d.tick(elapsed_cycles);
            d.irq()
        });
        IntStatus { irq: self.sio[0].irq() || self.sio[1].irq() || disk, ..IntStatus::NONE }
    }

    /// The disk's times, which `--fast-io` leaves alone: see `dcdd`.
    fn set_device_pacing_hz(&mut self, hz: u64) {
        if let Some(d) = &mut self.disk {
            d.set_pacing_hz(hz);
        }
    }

    /// An image must be the controller's 77 tracks of 32 sectors of 137
    /// bytes, padded by at most a sector; anything else would only look
    /// like a disk until the first sector.
    fn change_disk(&mut self, drive: usize, image: Option<DiskImage>) -> Result<(), String> {
        let Some(disk) = &mut self.disk else {
            return Err("this 8800 has no disk controller; altair8800-disk has one".into());
        };
        if drive >= dcdd::DRIVES {
            return Err(format!("no drive {drive}; the controller has drives 0 to {}", dcdd::DRIVES - 1));
        }
        match &image {
            Some(i) => {
                let len = i.size().map_err(|e| format!("{}: {e}", i.path.display()))?;
                if !dcdd::image_fits(len) {
                    return Err(format!(
                        "{}: {len} bytes is not an Altair 8-inch disk (77 tracks x 32 sectors x 137 bytes = {})",
                        i.path.display(),
                        dcdd::IMAGE_BYTES
                    ));
                }
                let note = if i.write_protected { " (read-only)" } else { "" };
                println!("88-DCDD: drive {drive}: inserted '{}'{note}", i.path.display());
            }
            None => println!("88-DCDD: drive {drive}: ejected"),
        }
        disk.mount(drive, image);
        Ok(())
    }

    fn save_disk(&self, drive: usize, path: &Path) -> Result<(), String> {
        let disk = self.disk.as_ref().ok_or("this 8800 has no disk controller")?;
        if drive >= dcdd::DRIVES {
            return Err(format!("no drive {drive}; the controller has drives 0 to {}", dcdd::DRIVES - 1));
        }
        crate::snapshot::write_new_file(path, &disk.disk_contents(drive)?)
    }

    fn commit_loaded_disks(&mut self) {
        if let Some(d) = &mut self.disk {
            d.commit_loaded();
        }
    }

    /// No vectored interrupt board: the pulled-up bus, `RST 7`.
    fn interrupt_acknowledge(&mut self) -> u8 {
        0xff
    }

    /// The tape is saved whole with how far it has been read, so a restore
    /// carries on mid-load with the tape it was taken with.
    fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        let Altair8800 { ram, sio, tape, sense, panel: _session, prom, disk } = self;
        w.section(b"RAM ", |w| ram.save(w));
        w.section(b"SIO0", |w| sio[0].save(w));
        w.section(b"SIO1", |w| sio[1].save(w));
        w.section(b"TAPE", |w| {
            w.opt(tape, |w, t| {
                w.bytes(&t.bytes);
                w.u32(t.pos as u32);
            })
        });
        w.section(b"SENS", |w| w.u8(*sense));
        w.section(b"PROM", |w| w.opt(prom, |w, p| p.save(w)));
        let mut disk_err = Ok(());
        w.section(b"DCDD", |w| {
            w.bool(disk.is_some());
            if let Some(d) = disk {
                disk_err = d.save_state(w);
            }
        });
        disk_err
    }

    fn load_state(&mut self, r: &mut crate::snapshot::Reader) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        r.section(b"RAM ", |r| self.ram.load(r))?;
        r.section(b"SIO0", |r| self.sio[0].load(r))?;
        r.section(b"SIO1", |r| self.sio[1].load(r))?;
        self.tape = r.section(b"TAPE", |r| {
            r.opt(|r| {
                let bytes = r.bytes()?.to_vec();
                let pos = r.u32()? as usize;
                if pos > bytes.len() {
                    return Err(format!("the tape is read to {pos} of {}", bytes.len()));
                }
                Ok(Tape { bytes, pos })
            })
        })?;
        self.sense = r.section(b"SENS", |r| r.u8())?;
        r.section(b"PROM", |r| {
            let has = r.bool()?;
            match &mut self.prom {
                Some(p) if has => p.load(r),
                None if !has => Ok(()),
                _ => Err("the snapshot's 8800 and this one differ in their PROM".into()),
            }
        })?;
        r.section(b"DCDD", |r| {
            let has = r.bool()?;
            match &mut self.disk {
                Some(d) if has => d.load_state(r),
                None if !has => Ok(()),
                _ => Err("the snapshot's 8800 and this one differ in their disk controller".into()),
            }
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::i8080::Cpu8080;
    use crate::cpu::{Cpu, StepResult};
    use std::sync::mpsc;

    fn build(sub: &str, image: &[u8]) -> (Altair8800, mpsc::Sender<u8>, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("emu-altair8800-{}-{n}.bin", std::process::id()));
        std::fs::write(&path, image).unwrap();
        let (tx, rx) = mpsc::channel();
        let out = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl io::Write for Shared {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let m = Altair8800::new(&path, ConsoleEndpoint::new(rx, Box::new(Shared(out.clone()))), sub);
        std::fs::remove_file(&path).ok();
        (m.unwrap(), tx, out)
    }

    fn run(m: &mut Altair8800, cpu: &mut Cpu8080, steps: usize) {
        for _ in 0..steps {
            assert_eq!(cpu.step(m), StepResult::Ok);
        }
    }

    /// An `image-disk` machine running `program` from 0, drive 0 an
    /// in-memory disk whose every byte is a function of where it is.
    fn disk_machine(program: &[u8]) -> (Altair8800, Vec<u8>) {
        let (_tx, rx) = mpsc::channel();
        let mut m = Altair8800::from_image(program.to_vec(), ConsoleEndpoint::new(rx, Box::new(io::sink())), "image-disk")
            .unwrap();
        let disk: Vec<u8> = (0..dcdd::IMAGE_BYTES as usize).map(|i| (i * 7 + (i >> 8)) as u8).collect();
        m.change_disk(0, Some(DiskImage::in_memory("disk.dsk".into(), disk.clone(), false))).unwrap();
        (m, disk)
    }

    /// Run to the program's `HLT`, which with interrupts off stops it.
    fn run_to_halt(m: &mut Altair8800, cpu: &mut Cpu8080) {
        for _ in 0..2_000_000 {
            match cpu.step(m) {
                StepResult::Ok => {}
                StepResult::Halted => return,
                other => panic!("{other:?} at {:04x}", cpu.pc()),
            }
        }
        panic!("no HLT; pc {:04x}", cpu.pc());
    }

    /// Select drive 0, load its head and wait for it to settle, then wait
    /// for sector `sector` to come round: the shape of the Burcon BIOS's
    /// `ldHead` and `secSync`. 31 bytes, ending at 0x001f.
    #[rustfmt::skip]
    fn find_sector(sector: u8) -> Vec<u8> {
        vec![
            0x31, 0x00, 0x10,       // 0000 lxi sp,1000h
            0xaf,                   // 0003 xra a
            0xd3, 0x08,             // 0004 out 08h         select drive 0
            0x3e, 0x04,             // 0006 mvi a,04h
            0xd3, 0x09,             // 0008 out 09h         head load
            0xdb, 0x08,             // 000a in 08h
            0xe6, 0x04,             // 000c ani 04h         head settled?
            0xc2, 0x0a, 0x00,       // 000e jnz 000ah
            0x1e, sector,           // 0011 mvi e,sector
            0xdb, 0x09,             // 0013 in 09h
            0x1f,                   // 0015 rar             sector true?
            0xda, 0x13, 0x00,       // 0016 jc 0013h
            0xe6, 0x1f,             // 0019 ani 1fh
            0xbb,                   // 001b cmp e
            0xc2, 0x13, 0x00,       // 001c jnz 0013h
        ]
    }

    /// The Burcon BIOS's `rdPSec` read loop, two bytes a pass and the second
    /// taken without asking NRDA, reads a whole sector off the disk: it
    /// works only if the bytes come 32 us apart, as they did.
    #[test]
    fn the_bios_read_loop_keeps_up_with_the_disk() {
        let mut program = find_sector(5);
        #[rustfmt::skip]
        program.extend([
            0x21, 0x00, 0x20,       // 001f lxi h,2000h
            0x0e, 137,              // 0022 mvi c,137
            0xdb, 0x08,             // 0024 in 08h          rdLoop
            0xb7,                   // 0026 ora a           NRDA?
            0xfa, 0x24, 0x00,       // 0027 jm 0024h
            0xdb, 0x0a,             // 002a in 0ah
            0x77,                   // 002c mov m,a
            0x23,                   // 002d inx h
            0x0d,                   // 002e dcr c
            0xca, 0x3b, 0x00,       // 002f jz 003bh
            0x0d,                   // 0032 dcr c
            0x00,                   // 0033 nop             timing
            0xdb, 0x0a,             // 0034 in 0ah          the 2nd byte
            0x77,                   // 0036 mov m,a
            0x23,                   // 0037 inx h
            0xc2, 0x24, 0x00,       // 0038 jnz 0024h
            0x76,                   // 003b hlt
        ]);
        let (mut m, disk) = disk_machine(&program);
        let mut cpu = Cpu8080::new();
        run_to_halt(&mut m, &mut cpu);
        let read: Vec<u8> = (0..137).map(|i| m.peek8(0x2000 + i).unwrap()).collect();
        let at = 5 * dcdd::SECTOR_BYTES;
        assert_eq!(read, disk[at..at + dcdd::SECTOR_BYTES], "track 0 sector 5");
    }

    /// The Burcon BIOS's `wrtSec` write loop: the sector's first byte with
    /// its sync bit set, the rest two a pass, then the 000 that ends it.
    /// The sector reaches the disk at its end, and the next is untouched.
    #[test]
    fn the_bios_write_loop_writes_a_sector() {
        let mut program = find_sector(9);
        #[rustfmt::skip]
        program.extend([
            0x3e, 0x80,             // 001f mvi a,80h
            0xd3, 0x09,             // 0021 out 09h         write enable
            0x21, 0x00, 0x30,       // 0023 lxi h,3000h
            0x01, 137, 0x01,        // 0026 lxi b,0189h     c=137, b=one 000
            0x16, 0x01,             // 0029 mvi d,01h       ENWD
            0x3e, 0x80,             // 002b mvi a,80h
            0xb6,                   // 002d ora m           the sync bit
            0x5f,                   // 002e mov e,a
            0x23,                   // 002f inx h
            0xdb, 0x08,             // 0030 in 08h          wrSec
            0xa2,                   // 0032 ana d           ENWD?
            0xc2, 0x30, 0x00,       // 0033 jnz 0030h
            0x83,                   // 0036 add e
            0xd3, 0x0a,             // 0037 out 0ah
            0x7e,                   // 0039 mov a,m
            0x23,                   // 003a inx h
            0x5e,                   // 003b mov e,m
            0x23,                   // 003c inx h
            0x0d,                   // 003d dcr c
            0xca, 0x47, 0x00,       // 003e jz 0047h
            0x0d,                   // 0041 dcr c
            0xd3, 0x0a,             // 0042 out 0ah         the 2nd byte
            0xc2, 0x30, 0x00,       // 0044 jnz 0030h
            0xdb, 0x08,             // 0047 in 08h          wrDone
            0xa2,                   // 0049 ana d
            0xc2, 0x47, 0x00,       // 004a jnz 0047h
            0xd3, 0x0a,             // 004d out 0ah         000
            0x05,                   // 004f dcr b
            0xc2, 0x47, 0x00,       // 0050 jnz 0047h
            0x76,                   // 0053 hlt
        ]);
        let (mut m, disk) = disk_machine(&program);
        let source: Vec<u8> = (0..137u32).map(|i| (i * 3 + 1) as u8).collect();
        for (i, b) in source.iter().enumerate() {
            m.write8(0x3000 + i as u32, *b);
        }
        let mut cpu = Cpu8080::new();
        run_to_halt(&mut m, &mut cpu);
        let sector = |m: &Altair8800, n: usize| {
            let d = m.disk.as_ref().unwrap().disk_contents(0).unwrap();
            d[n * dcdd::SECTOR_BYTES..(n + 1) * dcdd::SECTOR_BYTES].to_vec()
        };
        assert_eq!(sector(&m, 9), disk[9 * 137..10 * 137], "not on the disk before the sector ends");
        m.poll_interrupts(20_000);
        let mut expect = source.clone();
        expect[0] |= 0x80;
        assert_eq!(sector(&m, 9), expect);
        assert_eq!(sector(&m, 10), disk[10 * 137..11 * 137]);
    }

    /// The disk machine: the PROM at 0xff00 takes no writes from the guest
    /// or the debugger, the jump to it is keyed in at 0, and the PROM board
    /// has room for 256 bytes and no more.
    #[test]
    fn the_disk_machine_boots_from_a_read_only_prom() {
        let (_tx, rx) = mpsc::channel();
        let prom: Vec<u8> = (0..=255u8).collect();
        let mut m = Altair8800::from_image(prom.clone(), ConsoleEndpoint::new(rx, Box::new(io::sink())), "disk").unwrap();
        assert_eq!([m.peek8(0), m.peek8(1), m.peek8(2)], [Some(0xc3), Some(0x00), Some(0xff)]);
        assert_eq!(m.read8(0xff10), 0x10);
        m.write8(0xff10, 0xaa);
        assert_eq!(m.read8(0xff10), 0x10, "a PROM");
        assert!(m.poke8(0xff10, 0xaa).unwrap_err().contains("rom at ff10"));
        m.write8(0xfeff, 0x55);
        assert_eq!(m.read8(0xfeff), 0x55, "RAM up to the PROM");
        let (_tx, rx) = mpsc::channel();
        assert!(Altair8800::from_image(vec![0; 257], ConsoleEndpoint::new(rx, Box::new(io::sink())), "disk").is_err());
        let (_tx, rx) = mpsc::channel();
        assert!(Altair8800::from_image(vec![0], ConsoleEndpoint::new(rx, Box::new(io::sink())), "4k-disk").is_err());
    }

    /// A drive takes a disk of the controller's geometry, padded by less
    /// than a sector, and nothing else; the tape machine has no drives.
    #[test]
    fn a_drive_takes_only_an_8_inch_disk() {
        let (mut m, _) = disk_machine(&[0x76]);
        let disk = |n: usize| Some(DiskImage::in_memory("x.dsk".into(), vec![0; n], false));
        assert!(m.change_disk(1, disk(337_664)).is_ok());
        assert!(m.change_disk(1, disk(204_800)).unwrap_err().contains("not an Altair 8-inch disk"));
        assert!(m.change_disk(4, disk(337_568)).unwrap_err().contains("no drive 4"));
        let (mut tape, _, _) = build("", &[LEADER]);
        assert!(tape.change_disk(0, disk(337_568)).unwrap_err().contains("no disk controller"));
    }

    #[test]
    fn the_bootstrap_is_keyed_in_with_the_size_byte() {
        for (sub, high) in [("", 0o037u8), ("4k", 0o017), ("ext", 0o077)] {
            let (m, _, _) = build(sub, &[LEADER]);
            let mem: Vec<u8> = (0..0o034).map(|a| m.peek8(a).unwrap()).collect();
            assert_eq!(&mem[..0o012], &BOOTSTRAP[..0o012]);
            assert_eq!(mem[0o012], high, "{sub}");
            assert_eq!(&mem[0o032..], &[0o013, 0o000]);
        }
    }

    #[test]
    fn an_unknown_subsystem_is_refused() {
        let path = std::env::temp_dir().join(format!("emu-altair8800-bad-{}.bin", std::process::id()));
        std::fs::write(&path, [0u8]).unwrap();
        let (_tx, rx) = mpsc::channel();
        let m = Altair8800::new(&path, ConsoleEndpoint::new(rx, Box::new(io::sink())), "nope");
        std::fs::remove_file(&path).ok();
        assert!(m.is_err());
    }

    /// The real bootstrap over a synthetic tape: leader, then a checksum
    /// loader stand-in punched backwards (the bootstrap stores downwards),
    /// which it must load at the top of LADDR's page and jump to.
    #[test]
    fn the_bootstrap_loads_a_tape_backwards_and_runs_it() {
        // the loaded program, at 0x1f00: mvi a,'K'; out 0x11; hlt
        let program = [0x3e, b'K', 0xd3, 0x11, 0x76];
        let mut tape = vec![LEADER; 10];
        let mut body = vec![0u8; LEADER as usize];
        body[..program.len()].copy_from_slice(&program);
        tape.extend(body.iter().rev());
        let (mut m, _tx, out) = build("", &tape);
        let mut cpu = Cpu8080::new();
        cpu.reset(&mut m);
        let mut steps = 0;
        while cpu.step(&mut m) == StepResult::Ok {
            steps += 1;
            assert!(steps < 100_000, "never finished");
        }
        assert_eq!(cpu.pc(), 0x1f04, "halted on the loaded program's hlt");
        assert_eq!(&*out.lock().unwrap(), b"K");
        assert_eq!(m.peek8(0x1f00), Some(0x3e));
    }

    /// The keyboard has port 0 once the tape has run out, and not before.
    #[test]
    fn the_keyboard_waits_for_the_tape() {
        let (mut m, tx, _) = build("", &[LEADER, LEADER]);
        tx.send(b'a').unwrap();
        m.io_write8(0x1010, 0x15);
        for want in [LEADER, LEADER, b'a'] {
            m.poll_interrupts(0);
            assert_eq!(m.io_read8(0x1010) & 1, 1);
            assert_eq!(m.io_read8(0x1111), want, "nothing folded, and the tape first");
        }
    }

    #[test]
    fn sense_switches_and_unused_ports() {
        let (mut m, _, _) = build("image", &[]);
        assert_eq!(m.io_read8(0xffff), 0, "all off");
        let panel = AltairPanel::new();
        m.attach_panel(panel.clone());
        panel.toggle_switch(15);
        panel.toggle_switch(8);
        panel.toggle_switch(3);
        assert_eq!(m.io_read8(0xffff), 0x81, "A15-A8 off the panel");
        assert_eq!(m.io_read8(0x4040), 0xff, "nothing there");
        assert_eq!(m.io_read8(0x1212) & 0x02, 0x02, "port 1 is there, transmit empty");
    }

    /// Either port's receive interrupt reaches INT, and INTA reads RST 7.
    #[test]
    fn a_receive_interrupt_reaches_the_cpu_as_rst_7() {
        // lxi sp,0x8000; mvi a,0x95; out 0x10; ei; jmp $
        let (mut m, tx, _) = build("image", &[0x31, 0x00, 0x80, 0x3e, 0x95, 0xd3, 0x10, 0xfb, 0xc3, 0x08, 0x00]);
        let mut cpu = Cpu8080::new();
        cpu.reset(&mut m);
        run(&mut m, &mut cpu, 6);
        assert_eq!(cpu.pc(), 0x0008, "waiting");
        tx.send(b'x').unwrap();
        run(&mut m, &mut cpu, 1);
        assert_eq!(cpu.pc(), 0x0038);
        assert_eq!(m.interrupt_acknowledge(), 0xff);
    }

    #[test]
    fn a_snapshot_carries_the_tape_mid_read() {
        use crate::snapshot::{Reader, Writer};
        let (mut m, _, _) = build("", &[LEADER, 1, 2, 3]);
        m.io_write8(0x1010, 0x15);
        m.poll_interrupts(0);
        m.io_read8(0x1111);
        m.write8(0x4000, 0x42);
        let mut w = Writer::new();
        m.save_state(&mut w).unwrap();
        let bytes = w.into_bytes();
        let (mut n, _, _) = build("image", &[]);
        let mut r = Reader::new(&bytes);
        n.load_state(&mut r).unwrap();
        r.finish().unwrap();
        assert_eq!(n.peek8(0x4000), Some(0x42));
        n.poll_interrupts(0);
        assert_eq!(n.io_read8(0x1111), 1, "the read carries on where it was");
    }
}
