// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The debug port: what an outside program can ask of a running machine.
//!
//! The types here cross the thread boundary. A [`DebugRequest`] goes from
//! whoever is talking to the port to the run loop on the CPU thread, which
//! is the only thread that ever touches the cpu or the bus, and its
//! [`DebugReply`] comes back on the request's own channel. Everything the
//! machine volunteers -- serial output, stop events, the run ending --
//! goes the other way through a [`DebugSink`].

use crate::cpu::Register;
use crate::emulator::ExitReason;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// One thing the debugger asks the run loop to do. Addresses in `Peek` and
/// `Poke` are bus bytes; those in the `Words` forms and the breakpoints are
/// in the core's own units (`Cpu::addressing`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugOp {
    Status,
    Halt,
    Run,
    Step(u32),
    Reset,
    Regs,
    GetReg(String),
    SetReg(String, u32),
    Peek { addr: u32, len: u32 },
    Poke { addr: u32, bytes: Vec<u8> },
    PeekWords { addr: u32, len: u32 },
    PokeWords { addr: u32, words: Vec<u16> },
    AddBreak(u32),
    RemoveBreak(u32),
    Breaks,
}

/// A request and the channel its reply goes back on.
pub struct DebugRequest {
    pub op: DebugOp,
    pub reply: Sender<DebugReply>,
}

/// Why the machine is halted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// A HALT from the debugger or the panel.
    Request,
    /// A step budget ran out.
    Step,
    /// The PC reached a breakpoint.
    Break,
    /// The guest executed HLT.
    Hlt,
    /// The guest hit an opcode the core doesn't implement.
    BadOpcode,
    /// RESET, which leaves the machine halted.
    Reset,
}

impl StopReason {
    pub fn name(self) -> &'static str {
        match self {
            StopReason::Request => "request",
            StopReason::Step => "step",
            StopReason::Break => "break",
            StopReason::Hlt => "hlt",
            StopReason::BadOpcode => "badop",
            StopReason::Reset => "reset",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugReply {
    Ok,
    Err(String),
    Status { halted: bool, reason: Option<StopReason>, pc: u32, insns: u64 },
    Registers(Vec<Register>),
    Register(Register),
    Bytes(Vec<u8>),
    Words(Vec<u16>),
    Breaks(Vec<u32>),
}

/// Something the machine did on its own, as opposed to a reply.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DebugEvent {
    /// The machine went from running (or stepping) to halted.
    Stopped { reason: StopReason, pc: u32 },
    /// The run loop returned; the process is on its way out.
    Exit(ExitReason),
}

/// One item on its way to the client: guest serial output, an event, or
/// a line the server composed itself (a reply, the banner).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outbound {
    Output(Vec<u8>),
    Event(DebugEvent),
    Line(String),
}

/// How much guest output the sink keeps for a client that attaches late.
pub const BACKLOG_BYTES: usize = 16 * 1024;

#[derive(Default)]
struct SinkInner {
    client: Option<Sender<Outbound>>,
    backlog: VecDeque<u8>,
}

/// The machine's side of the port: the run loop and the console tap write
/// here, and whichever client is attached reads. Output is kept in a
/// bounded backlog whether or not anyone is attached, so a debugger
/// arriving at a stuck machine can still ask what it printed last; it is
/// not replayed on attach, since a client pattern-matching live output
/// must not see stale bytes mixed into it.
#[derive(Clone, Default)]
pub struct DebugSink(Arc<Mutex<SinkInner>>);

impl DebugSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Guest serial output, from the console tap on the CPU thread.
    pub fn output(&self, bytes: &[u8]) {
        let mut inner = self.0.lock().unwrap();
        for &b in bytes {
            if inner.backlog.len() == BACKLOG_BYTES {
                inner.backlog.pop_front();
            }
            inner.backlog.push_back(b);
        }
        Self::deliver(&mut inner, Outbound::Output(bytes.to_vec()));
    }

    /// A stop or exit event, from the run loop.
    pub fn event(&self, ev: DebugEvent) {
        let mut inner = self.0.lock().unwrap();
        Self::deliver(&mut inner, Outbound::Event(ev));
    }

    /// A client whose channel has gone away is simply forgotten.
    fn deliver(inner: &mut SinkInner, item: Outbound) {
        if let Some(client) = &inner.client {
            if client.send(item).is_err() {
                inner.client = None;
            }
        }
    }

    /// Make `tx` the client. One at a time: a later attach replaces an
    /// earlier one, whose sender is dropped here.
    pub fn attach(&self, tx: Sender<Outbound>) {
        self.0.lock().unwrap().client = Some(tx);
    }

    pub fn detach(&self) {
        self.0.lock().unwrap().client = None;
    }

    pub fn is_attached(&self) -> bool {
        self.0.lock().unwrap().client.is_some()
    }

    /// The last `BACKLOG_BYTES` of guest output.
    pub fn backlog(&self) -> Vec<u8> {
        self.0.lock().unwrap().backlog.iter().copied().collect()
    }
}

/// The `Write` the console endpoint's tap is given: every byte of serial
/// output lands in the sink.
pub struct TapWriter(pub DebugSink);

impl Write for TapWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.output(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn the_backlog_keeps_the_last_bytes_and_a_client_gets_live_output() {
        let sink = DebugSink::new();
        let filler = vec![b'x'; BACKLOG_BYTES];
        sink.output(&filler);
        sink.output(b"tail");
        let backlog = sink.backlog();
        assert_eq!(backlog.len(), BACKLOG_BYTES);
        assert_eq!(&backlog[BACKLOG_BYTES - 4..], b"tail");

        let (tx, rx) = mpsc::channel();
        sink.attach(tx);
        assert!(rx.try_recv().is_err(), "the backlog is not replayed on attach");
        sink.output(b"live");
        sink.event(DebugEvent::Exit(ExitReason::Halted));
        assert_eq!(rx.try_recv().unwrap(), Outbound::Output(b"live".to_vec()));
        assert_eq!(rx.try_recv().unwrap(), Outbound::Event(DebugEvent::Exit(ExitReason::Halted)));
    }

    #[test]
    fn detaching_drops_output_but_keeps_the_backlog() {
        let sink = DebugSink::new();
        let (tx, rx) = mpsc::channel();
        sink.attach(tx);
        assert!(sink.is_attached());
        sink.detach();
        assert!(!sink.is_attached());
        TapWriter(sink.clone()).write_all(b"gone").unwrap();
        assert!(rx.try_recv().is_err());
        assert_eq!(sink.backlog(), b"gone");
    }

    /// A client that hung up is forgotten on the next delivery, so a dead
    /// channel never stalls the CPU thread or leaks.
    #[test]
    fn a_dead_client_is_forgotten() {
        let sink = DebugSink::new();
        let (tx, rx) = mpsc::channel();
        sink.attach(tx);
        drop(rx);
        sink.output(b"?");
        assert!(!sink.is_attached());
    }
}
