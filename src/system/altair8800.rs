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
//!
//! Without a front panel the sense switches read all off, which BASIC 4.x
//! takes as console and load device both 88-2SIO port 0 (Martin Eberhard, *Loading Basic with
//! the 88-2SIO Board*, 2013). Both ports' interrupt outputs are jumpered to
//! the bus's PINT, as the 2SIO's jumpers allow, with no vectored interrupt
//! board: the pulled-up data bus answers INTA with `RST 7`. BASIC never
//! enables an ACIA interrupt, so the line only reaches a program that does.

use crate::bus::{Bus, IntStatus, MemoryDevice};
use crate::console::{AltairPanel, ConsoleEndpoint};
use crate::dev::mc6850::Mc6850;
use crate::dev::memory::Memory;
use crate::rom;
use std::io;
use std::path::Path;

pub const DEFAULT_ROM: &str = "roms/altair8800/basic8k40.tap";
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
}

/// A line with nothing on either end, for a port nobody has plugged in.
fn unplugged() -> ConsoleEndpoint {
    let (_tx, rx) = std::sync::mpsc::channel();
    ConsoleEndpoint::new(rx, Box::new(io::sink()))
}

impl Altair8800 {
    pub fn new(rom_path: &Path, console: ConsoleEndpoint, subsystem: &str) -> io::Result<Self> {
        let image = rom::load_binary(rom_path)?;
        let mut m = Altair8800 {
            ram: Memory::new(0x10000),
            sio: [Mc6850::raw(console), Mc6850::raw(unplugged())],
            tape: None,
            sense: 0,
            panel: None,
        };
        let laddr_high = match subsystem {
            "image" => {
                m.ram.load_at(0, &image);
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
                         'altair8800-4k', 'altair8800-ext' or 'altair8800-image'"
                    ),
                ))
            }
        };
        if image.first() != Some(&LEADER) {
            eprintln!(
                "altair8800: {} does not start with the 4.x leader ({LEADER:#04x}); \
                 the bootstrap will take it for the checksum loader",
                rom_path.display()
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
        self.ram.read_byte(addr & 0xffff)
    }

    fn write8(&mut self, addr: u32, val: u8) {
        self.ram.write_byte(addr & 0xffff, val);
    }

    fn peek8(&self, addr: u32) -> Option<u8> {
        Some(self.ram.peek(addr & 0xffff))
    }

    /// The boards decode the low byte of the address, where the 8080 puts
    /// the port number (it is on both halves). Nothing answers an unused
    /// port, and the bus floats high.
    fn io_read8(&mut self, port: u16) -> u8 {
        match port as u8 {
            p @ 0x10..=0x13 => self.sio[(p as usize - 0x10) >> 1].read_byte(p as u32 & 1),
            0xff => self.panel.as_ref().map_or(self.sense, |p| (p.switches() >> 8) as u8),
            _ => 0xff,
        }
    }

    fn io_write8(&mut self, port: u16, val: u8) {
        if let p @ 0x10..=0x13 = port as u8 {
            self.sio[(p as usize - 0x10) >> 1].write_byte(p as u32 & 1, val);
        }
    }

    fn poll_interrupts(&mut self, _elapsed_cycles: u32) -> IntStatus {
        self.poll_console();
        self.sio[1].poll();
        IntStatus { irq: self.sio[0].irq() || self.sio[1].irq(), ..IntStatus::NONE }
    }

    /// No vectored interrupt board: the pulled-up bus, `RST 7`.
    fn interrupt_acknowledge(&mut self) -> u8 {
        0xff
    }

    /// The tape is saved whole with how far it has been read, so a restore
    /// carries on mid-load with the tape it was taken with.
    fn save_state(&self, w: &mut crate::snapshot::Writer) -> Result<(), String> {
        use crate::snapshot::Snapshot;
        let Altair8800 { ram, sio, tape, sense, panel: _session } = self;
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
        Ok(())
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
