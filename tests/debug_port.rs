// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The debug port end to end, short of a process: a 703 built through the
//! registry, the run loop on a thread with the socket server beside it,
//! and a client on a real Unix socket exchanging the protocol's lines.
//! Every wait is a blocking read under a timeout; nothing sleeps.

use emu::console::ConsoleEndpoint;
use emu::debug::server::{spawn_unix, DebugServer};
use emu::debug::{DebugSink, TapWriter};
use emu::emulator::{Emulator, ExitReason, StopPolicy};
use emu::system::registry;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::Duration;

struct Client {
    lines: BufReader<UnixStream>,
    out: UnixStream,
    /// Events that arrived while a reply was being waited for. An event
    /// is asynchronous and may precede the reply of the command that
    /// caused it -- the stop from `run` reaches the writer straight from
    /// the run loop, the `ok` by way of the reader thread.
    events: std::collections::VecDeque<String>,
}

impl Client {
    fn connect(path: &std::path::Path) -> Self {
        let stream = UnixStream::connect(path).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Client {
            lines: BufReader::new(stream.try_clone().unwrap()),
            out: stream,
            events: Default::default(),
        }
    }

    fn line(&mut self) -> String {
        let mut s = String::new();
        let n = self.lines.read_line(&mut s).expect("a line before the timeout");
        assert!(n > 0, "the server closed the connection");
        s.trim_end().to_string()
    }

    /// Send a command and return its reply: the next `ok`/`err` line,
    /// events on the way being kept for `event()`.
    fn ask(&mut self, cmd: &str) -> String {
        self.out.write_all(format!("{cmd}\n").as_bytes()).unwrap();
        loop {
            let line = self.line();
            if line.starts_with('!') {
                self.events.push_back(line);
            } else {
                return line;
            }
        }
    }

    /// The next event: one already seen, or the next line, which must be
    /// one.
    fn event(&mut self) -> String {
        if let Some(e) = self.events.pop_front() {
            return e;
        }
        let line = self.line();
        assert!(line.starts_with('!'), "expected an event, got {line:?}");
        line
    }
}

#[test]
fn a_client_drives_a_703_over_the_socket() {
    // JMP 0x40 at word 0 and JMP $ at word 0x40: a machine that runs
    // forever, which under the halt policy is exactly what it should do.
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("emu-debug-port-{pid}"));
    std::fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("spin.bin");
    let mut image = vec![0u8; 0x82];
    image[0..2].copy_from_slice(&[0x10, 0x40]);
    image[0x80..0x82].copy_from_slice(&[0x10, 0x40]);
    std::fs::write(&rom, &image).unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let (keys_tx, keys_rx) = mpsc::channel();
    let sink = DebugSink::new();
    let mut endpoint = ConsoleEndpoint::new(keys_rx, Box::new(std::io::sink()));
    endpoint.set_tap(Box::new(TapWriter(sink.clone())));
    let desc = registry::find("ray703").unwrap();
    let machine = (desc.factory)(&rom, endpoint, "", &registry::MachineOpts::default()).unwrap();

    let (ctl_tx, ctl_rx) = mpsc::channel();
    let mut emu = Emulator::new(machine.cpu, machine.bus, Arc::clone(&shutdown));
    emu.set_control(Some(ctl_rx));
    emu.set_stop_policy(StopPolicy::Halt);
    emu.set_halted(true);
    emu.set_debug_sink(Some(sink.clone()));
    emu.reset();

    let socket = dir.join("s");
    let server = spawn_unix(
        socket.clone(),
        DebugServer {
            control: ctl_tx,
            keys: keys_tx,
            sink,
            shutdown: Arc::clone(&shutdown),
            banner: "test".into(),
        },
    )
    .unwrap();
    let cpu = std::thread::spawn(move || emu.run());

    let mut c = Client::connect(&socket);
    assert_eq!(c.line(), "# test");
    assert_eq!(c.ask("status"), "ok state=halted pc=0000 insns=0");
    assert_eq!(c.ask("set ac 1234"), "ok AC=1234");
    assert_eq!(c.ask("reg AC"), "ok AC=1234");
    assert_eq!(c.ask("write 80 ab cd"), "ok");
    assert_eq!(c.ask("mem 80 2"), "ok ab cd");
    assert_eq!(c.ask("memw 40 1"), "ok abcd", "word 0x40 is bytes 0x80-0x81, big-endian");
    assert_eq!(c.ask("writew 40 1040"), "ok");
    assert_eq!(c.ask("memw 0 1"), "ok 1040");
    assert_eq!(c.ask("break 40"), "ok");
    assert_eq!(c.ask("breaks"), "ok 0040");
    assert_eq!(c.ask("run"), "ok");
    assert_eq!(c.event(), "! stopped reason=break pc=0040");
    assert_eq!(c.ask("status"), "ok state=halted reason=break pc=0040 insns=1");
    assert_eq!(c.ask("step"), "ok");
    assert_eq!(c.event(), "! stopped reason=step pc=0040", "JMP $ lands where it started");
    assert!(c.ask("regs").starts_with("ok PC=0040 AC=1234 IX=0000 EX=00 ST=0000 "));
    assert_eq!(c.ask("key A\\r"), "ok");
    assert_eq!(c.ask("mem 1c ff"), "err 'ff' is not a count");
    assert_eq!(c.ask("mem 0 300"), "err at most 256 bytes per request");
    assert_eq!(c.ask("bogus"), "err unknown command 'bogus'; try help");
    assert_eq!(c.ask("kill"), "ok");
    assert_eq!(c.event(), "! exit reason=shutdown");

    assert_eq!(cpu.join().unwrap(), ExitReason::Shutdown);
    server.join().unwrap();
    assert!(!socket.exists(), "the socket file is removed on exit");
    std::fs::remove_dir_all(&dir).ok();
}
