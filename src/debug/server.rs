// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The debug port's listener and per-connection threads.
//!
//! Three kinds of thread: one accept loop, and for every connection a
//! reader that turns lines into requests and a writer that owns the
//! socket's output. Neither ever touches the machine -- a request goes
//! over the control channel to the run loop and its reply comes back on a
//! channel of its own -- and nothing here waits on the shutdown flag, so
//! the port cannot deadlock with the thread it is talking to.

use super::protocol::{self, Command};
use super::{DebugReply, DebugSink, Outbound};
use crate::emulator::Control;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How long a request may wait for the run loop. The loop answers between
/// instructions, or within one halted-wait tick; anything longer means the
/// CPU thread is gone.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// How often an idle connection or the accept loop looks at the shutdown
/// flag.
const POLL: Duration = Duration::from_millis(100);
/// How long a writer waits for the exit event after the shutdown flag
/// before giving up on it.
const EXIT_GRACE: Duration = Duration::from_secs(1);

/// Everything a connection needs: the run loop's channel, the guest's
/// keyboard, the sink the machine reports into, the shutdown flag `kill`
/// sets, and the banner a client is greeted with.
#[derive(Clone)]
pub struct DebugServer {
    pub control: Sender<Control>,
    pub keys: Sender<u8>,
    pub sink: DebugSink,
    pub shutdown: Arc<AtomicBool>,
    pub banner: String,
}

/// Listen on a Unix socket at `path` until the shutdown flag is set. A
/// stale socket file from an earlier run is removed first, and the file is
/// removed again on the way out.
pub fn spawn_unix(path: PathBuf, server: DebugServer) -> io::Result<JoinHandle<()>> {
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    Ok(thread::spawn(move || {
        let mut connections = Vec::new();
        while !server.shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    // Blocking again for the connection's own threads, with
                    // a read timeout so an idle reader still notices the
                    // flag. try_clone gives the writer its own handle.
                    let ok = stream
                        .set_nonblocking(false)
                        .and_then(|_| stream.set_read_timeout(Some(POLL)))
                        .and_then(|_| stream.try_clone());
                    match ok {
                        Ok(writer) => connections.push(serve_connection(stream, writer, server.clone())),
                        Err(e) => eprintln!("debug port: connection setup failed: {e}"),
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL),
                Err(e) => {
                    eprintln!("debug port: accept failed: {e}");
                    thread::sleep(POLL);
                }
            }
        }
        // Let every connection deliver what it has -- the exit event in
        // particular -- before the process goes.
        for (reader, writer) in connections {
            let _ = reader.join();
            let _ = writer.join();
        }
        let _ = std::fs::remove_file(&path);
    }))
}

/// Serve one client over any pair of streams; the transport is the
/// caller's business. Returns the reader and writer thread handles.
///
/// The reader must return periodically while idle for the shutdown flag
/// to be noticed: give it a stream with a read timeout.
pub fn serve_connection<R, W>(reader: R, writer: W, server: DebugServer) -> (JoinHandle<()>, JoinHandle<()>)
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let (out_tx, out_rx) = mpsc::channel::<Outbound>();
    // From here on the sink delivers to this connection; an earlier
    // client's sender is dropped by the replacement.
    let attachment = server.sink.attach(out_tx.clone());
    let _ = out_tx.send(Outbound::Line(format!("# {}", server.banner)));

    let shutdown = Arc::clone(&server.shutdown);
    let writer = thread::spawn(move || write_loop(writer, out_rx, &shutdown));
    let reader = thread::spawn(move || {
        if read_loop(reader, &out_tx, &server) == Hangup::Client {
            // The client is gone: only this connection's attachment goes,
            // a newer client keeps its own. out_tx drops with the thread,
            // and the writer's channel disconnects once nothing else holds
            // a sender.
            server.sink.detach(attachment);
        }
        // On shutdown the attachment stays: the run loop's exit event is
        // still to come, and the writer waits for it.
    });
    (reader, writer)
}

/// Why the reader stopped reading.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Hangup {
    /// EOF, `quit`, or an error on the stream.
    Client,
    /// The shutdown flag.
    Shutdown,
}

/// Drain the outbound channel onto the stream. Everything queued at once
/// goes out in one write, consecutive runs of guest output folded into a
/// single `! out` line -- under `--fast-io` the tap can deliver a byte per
/// instruction, and one line per byte would be a syscall per byte.
///
/// Ends when the channel disconnects (the client went away), after
/// delivering the exit event (the machine did), or a second after the
/// shutdown flag if no exit event ever comes.
fn write_loop<W: Write>(mut w: W, rx: mpsc::Receiver<Outbound>, shutdown: &AtomicBool) {
    let mut deadline = None;
    loop {
        let first = match rx.recv_timeout(POLL) {
            Ok(item) => item,
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if shutdown.load(Ordering::SeqCst) {
                    let d = *deadline.get_or_insert_with(|| std::time::Instant::now() + EXIT_GRACE);
                    if std::time::Instant::now() >= d {
                        return;
                    }
                }
                continue;
            }
        };
        let mut batch = vec![first];
        while let Ok(item) = rx.try_recv() {
            batch.push(item);
        }
        let last = batch.iter().any(|i| matches!(i, Outbound::Event(super::DebugEvent::Exit(_))));
        let mut text = String::new();
        let mut output: Vec<u8> = Vec::new();
        let flush_output = |text: &mut String, output: &mut Vec<u8>| {
            if !output.is_empty() {
                text.push_str(&protocol::format_output(output));
                text.push('\n');
                output.clear();
            }
        };
        for item in batch {
            match item {
                Outbound::Output(bytes) => output.extend_from_slice(&bytes),
                Outbound::Event(ev) => {
                    flush_output(&mut text, &mut output);
                    text.push_str(&protocol::format_event(&ev));
                    text.push('\n');
                }
                Outbound::Line(line) => {
                    flush_output(&mut text, &mut output);
                    text.push_str(&line);
                    text.push('\n');
                }
            }
        }
        flush_output(&mut text, &mut output);
        // a client that hung up is the reader's to notice
        if w.write_all(text.as_bytes()).and_then(|_| w.flush()).is_err() || last {
            return;
        }
    }
}

/// Read lines until EOF, `quit`, an error, or the shutdown flag.
fn read_loop<R: Read>(reader: R, out: &Sender<Outbound>, server: &DebugServer) -> Hangup {
    let mut lines = BufReader::new(reader);
    let mut line = String::new();
    loop {
        if server.shutdown.load(Ordering::SeqCst) {
            return Hangup::Shutdown;
        }
        match lines.read_line(&mut line) {
            Ok(0) => return Hangup::Client,
            Ok(_) if line.ends_with('\n') => {}
            // a timeout mid-line leaves the partial line in `line`
            Ok(_) => continue,
            Err(e) if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
            ) =>
            {
                continue
            }
            Err(_) => return Hangup::Client,
        }
        let text = line.trim_end_matches(['\n', '\r']).to_string();
        line.clear();
        let reply = |s: String| {
            let _ = out.send(Outbound::Line(s));
        };
        match protocol::parse(&text) {
            Err(e) => reply(format!("err {e}")),
            Ok(Command::Machine(op)) => reply(protocol::format_reply(&ask(server, op))),
            Ok(Command::Key(bytes)) => {
                if bytes.iter().any(|&b| server.keys.send(b).is_err()) {
                    reply("err the console is gone".into());
                } else {
                    reply("ok".into());
                }
            }
            Ok(Command::Backlog) => reply(format!("ok {}", protocol::escape(&server.sink.backlog()))),
            Ok(Command::Help) => {
                for l in protocol::HELP {
                    reply(format!("# {l}"));
                }
                reply("ok".into());
            }
            Ok(Command::Quit) => {
                reply("ok".into());
                return Hangup::Client;
            }
            Ok(Command::Kill) => {
                reply("ok".into());
                server.shutdown.store(true, Ordering::SeqCst);
            }
        }
    }
}

/// Put one request to the run loop and wait for its reply.
fn ask(server: &DebugServer, op: super::DebugOp) -> DebugReply {
    let (reply_tx, reply_rx) = mpsc::channel();
    let request = super::DebugRequest { op, reply: reply_tx };
    if server.control.send(Control::Debug(request)).is_err() {
        return DebugReply::Err("emulator stopped".into());
    }
    match reply_rx.recv_timeout(REPLY_TIMEOUT) {
        Ok(reply) => reply,
        Err(RecvTimeoutError::Disconnected) => DebugReply::Err("emulator stopped".into()),
        Err(RecvTimeoutError::Timeout) => DebugReply::Err("no reply from the emulator".into()),
    }
}
