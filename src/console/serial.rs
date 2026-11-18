// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! `--serial`: the far end of a machine's spare serial port, on the host.
//!
//! Either a pseudo-terminal, whose slave side a terminal program opens
//! (`screen /dev/ttys004`, `picocom`, a Kermit), or a TCP listener on
//! localhost taking one client at a time (`nc localhost 2323`). Both come
//! back as a [`ConsoleEndpoint`], the same shape as the console's, so the
//! machine feeds the port's receive side from `try_next_char` and hands it
//! what the guest transmits through `put_char`.
//!
//! A serial line with nothing listening loses what is sent down it, and so
//! does this one: transmitted bytes go through a bounded queue to a writer
//! thread, and a far end that is absent or not reading drops them rather
//! than stalling the CPU thread. Nothing waits for a far end that turns up
//! later either: a TCP client's byte is sent without waiting or not at
//! all, and what sits unread in the pty for `PTY_STALE` is thrown away.
//! Received bytes queue without limit, and go to the SIO at its baud rate
//! -- held on the wire while RTS is off, on a machine whose far end
//! honours it.

use super::ConsoleEndpoint;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Bytes the guest may get ahead of a slow far end before they are lost.
const OUT_QUEUE: usize = 4096;

/// How long the pty holds bytes nobody is reading before they are lost.
const PTY_STALE: Duration = Duration::from_millis(250);

/// Where the far end of the line is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialSpec {
    /// A new pseudo-terminal; its slave's path is announced at startup.
    Pty,
    /// A TCP listener on localhost at this port; 0 picks a free one.
    Tcp(u16),
    /// A window of the emulator's own: a 703 teletype's paper, which the
    /// 703's factory makes in place of a line to the host. [`open`]
    /// refuses it -- no other port has a window to be.
    Window,
}

impl SerialSpec {
    /// `pty`, `tcp:PORT` or `window`.
    pub fn parse(s: &str) -> Result<Self, String> {
        if s == "pty" {
            return Ok(SerialSpec::Pty);
        }
        if s == "window" {
            return Ok(SerialSpec::Window);
        }
        if let Some(port) = s.strip_prefix("tcp:") {
            return port.parse().map(SerialSpec::Tcp).map_err(|_| format!("'{port}' is not a port number"));
        }
        Err(format!("'{s}' is not 'pty', 'tcp:PORT' or 'window'"))
    }
}

/// Open the far end of the line. Returns the endpoint and where it is,
/// for the startup announcement: the slave's path, or the address
/// listened on.
pub fn open(spec: &SerialSpec) -> io::Result<(ConsoleEndpoint, String)> {
    let (in_tx, in_rx) = mpsc::channel();
    let (out_tx, out_rx) = mpsc::sync_channel(OUT_QUEUE);
    let endpoint = ConsoleEndpoint::new(in_rx, Box::new(LineOut(out_tx)));
    let place = match spec {
        SerialSpec::Pty => open_pty(in_tx, out_rx)?,
        SerialSpec::Tcp(port) => open_tcp(*port, in_tx, out_rx)?,
        SerialSpec::Window => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a window is for a 703 teletype's line; this one wants pty or tcp:PORT",
            ))
        }
    };
    Ok((endpoint, place))
}

/// The guest's side of the transmit queue: a byte that does not fit is
/// lost, as on a line nobody is reading.
struct LineOut(SyncSender<u8>);

impl Write for LineOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            let _ = self.0.try_send(b);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn open_pty(in_tx: Sender<u8>, out_rx: Receiver<u8>) -> io::Result<String> {
    let mut master: libc::c_int = -1;
    let mut slave: libc::c_int = -1;
    // SAFETY: openpty writes the two descriptors; the name, termios and
    // winsize arguments may be null.
    let r = unsafe {
        libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    // Raw, so the line carries bytes and nothing on the host translates or
    // echoes them before a terminal program opens the slave and sets its
    // own modes. The slave stays open here for the process's life: with
    // no slave open a read on the master is an error, not a wait.
    // SAFETY: slave is the descriptor openpty just returned; the termios
    // is fully written by tcgetattr before it is read.
    let name = unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(slave, &mut t) == 0 {
            libc::cfmakeraw(&mut t);
            libc::tcsetattr(slave, libc::TCSANOW, &t);
        }
        // ttyname_r, into a buffer of this call's own: ttyname's is one
        // static shared by every thread opening a pty
        let mut buf = [0 as libc::c_char; 256];
        let e = libc::ttyname_r(slave, buf.as_mut_ptr(), buf.len());
        if e != 0 {
            return Err(io::Error::from_raw_os_error(e));
        }
        std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    };
    // SAFETY: master is ours alone from here, owned by the File.
    let mut reader = unsafe { File::from_raw_fd(master) };
    let mut writer = reader.try_clone()?;
    thread::spawn(move || {
        let mut buf = [0u8; 256];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || buf[..n].iter().any(|&b| in_tx.send(b).is_err()) {
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut unread = Unread::new(slave);
        loop {
            let b = match out_rx.recv_timeout(PTY_STALE) {
                Ok(b) => Some(b),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            unread.check();
            let Some(b) = b else { continue };
            // a slow reader is waited for; one that reads nothing has the
            // buffer emptied under it, which makes the room
            while !writable(writer.as_raw_fd(), PTY_STALE) {
                unread.check();
            }
            if writer.write_all(&[b]).is_ok() {
                unread.written += 1;
            }
        }
    });
    Ok(name)
}

/// What the pty holds that its slave side has not read, watched for a far
/// end that has stopped reading -- or was never there, which looks the
/// same from here, since the slave is open in this process either way.
struct Unread {
    slave: libc::c_int,
    /// Bytes written since the buffer was last seen empty or was emptied.
    written: u64,
    /// How many of those had been read at the last check, and when that
    /// last changed.
    read: u64,
    since: Instant,
}

impl Unread {
    fn new(slave: libc::c_int) -> Self {
        Unread { slave, written: 0, read: 0, since: Instant::now() }
    }

    /// Throw away what has sat unread for `PTY_STALE` with nothing read
    /// meanwhile, so a terminal opened later does not get it.
    fn check(&mut self) {
        let mut pending: libc::c_int = 0;
        // SAFETY: slave is the open descriptor openpty returned, and
        // FIONREAD writes one int.
        if unsafe { libc::ioctl(self.slave, libc::FIONREAD, &mut pending) } != 0 {
            return;
        }
        let read = self.written.saturating_sub(pending.max(0) as u64);
        if pending <= 0 {
            self.written = 0;
            self.read = 0;
            self.since = Instant::now();
        } else if read != self.read {
            self.read = read;
            self.since = Instant::now();
        } else if self.since.elapsed() >= PTY_STALE {
            // SAFETY: as above; the slave's input queue is what the master
            // wrote and nobody has read.
            unsafe { libc::tcflush(self.slave, libc::TCIFLUSH) };
            self.written = 0;
            self.read = 0;
            self.since = Instant::now();
        }
    }
}

/// Whether `fd` takes a write without blocking, waiting up to `timeout`.
fn writable(fd: libc::c_int, timeout: Duration) -> bool {
    let mut p = libc::pollfd { fd, events: libc::POLLOUT, revents: 0 };
    // SAFETY: one pollfd, fully initialised, for the length of the call.
    let n = unsafe { libc::poll(&mut p, 1, timeout.as_millis() as libc::c_int) };
    // an error on the descriptor is for the write to find
    n > 0 || (n < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted)
}

fn open_tcp(port: u16, in_tx: Sender<u8>, out_rx: Receiver<u8>) -> io::Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let place = listener.local_addr()?.to_string();
    // The connected client, if any: the reader sets and clears it, the
    // writer sends to it.
    let client: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(None));
    let for_writer = client.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_nodelay(true);
            *client.lock().unwrap() = stream.try_clone().ok();
            let mut buf = [0u8; 256];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if buf[..n].iter().any(|&b| in_tx.send(b).is_err()) {
                    return;
                }
            }
            *client.lock().unwrap() = None;
        }
    });
    thread::spawn(move || {
        for b in out_rx {
            // never a wait with the lock held: the reader needs it to let
            // a client go and take the next
            let mut c = for_writer.lock().unwrap();
            if c.as_ref().is_some_and(|s| send_nowait(s, b).is_err()) {
                *c = None;
            }
        }
    });
    Ok(place)
}

/// One byte to a client without waiting for room: `Ok(false)` is a client
/// that is not reading, its buffers full and the byte lost, and `Err` one
/// that has gone. Asked of `poll` first, with the write after it, since
/// macOS waits in a `send` even under `MSG_DONTWAIT`.
fn send_nowait(mut stream: &TcpStream, b: u8) -> io::Result<bool> {
    if !writable(stream.as_raw_fd(), Duration::ZERO) {
        return Ok(false);
    }
    stream.write_all(&[b]).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_parse() {
        assert_eq!(SerialSpec::parse("pty"), Ok(SerialSpec::Pty));
        assert_eq!(SerialSpec::parse("tcp:2323"), Ok(SerialSpec::Tcp(2323)));
        assert!(SerialSpec::parse("tcp:x").is_err());
        assert!(SerialSpec::parse("com1").is_err());
    }

    /// The next received byte, waiting up to a few seconds for the
    /// reader thread to deliver it.
    fn next_byte(ep: &mut ConsoleEndpoint) -> Option<u8> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(b) = ep.try_next_char() {
                return Some(b);
            }
            thread::yield_now();
        }
        None
    }

    /// Bytes cross a TCP line in both directions.
    #[test]
    fn a_tcp_line_carries_bytes_both_ways() {
        let (mut ep, place) = open(&SerialSpec::Tcp(0)).unwrap();
        let mut far = TcpStream::connect(&place).unwrap();
        far.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        far.write_all(b"hi").unwrap();
        assert_eq!(next_byte(&mut ep), Some(b'h'));
        assert_eq!(next_byte(&mut ep), Some(b'i'));
        // the reader has registered the client by the time its bytes
        // arrived, so this one has somewhere to go
        ep.put_char(b'o');
        ep.put_char(b'k');
        let mut got = [0u8; 2];
        far.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"ok");
    }

    /// A byte for a client that is not reading is lost once the socket's
    /// buffers are full, not waited on; the writer holds the client's
    /// lock, and the reader needs it to take the next client.
    #[test]
    fn a_tcp_client_that_is_not_reading_loses_bytes_without_a_wait() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut far = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (near, _) = listener.accept().unwrap();
        let mut sent = 0u32;
        while send_nowait(&near, b'x').unwrap() {
            sent += 1;
            assert!(sent < 1 << 28, "the buffers never filled");
        }
        // ...and it is a line again once the far end reads
        far.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = vec![0u8; 65536];
        let deadline = Instant::now() + Duration::from_secs(5);
        while !send_nowait(&near, b'y').unwrap() {
            assert!(Instant::now() < deadline, "no room after the far end read");
            assert_ne!(far.read(&mut buf).unwrap(), 0);
        }
        drop(far);
        let deadline = Instant::now() + Duration::from_secs(5);
        while send_nowait(&near, b'z').is_ok() {
            assert!(Instant::now() < deadline, "a client that has gone is an error");
            thread::yield_now();
        }
    }

    /// What the guest sent with nobody reading the slave is gone by the
    /// time somebody does: a terminal opened later starts with what is
    /// sent from then on.
    #[test]
    fn a_pty_keeps_nothing_for_a_far_end_that_was_not_reading() {
        let (mut ep, place) = open(&SerialSpec::Pty).unwrap();
        let mut far = std::fs::OpenOptions::new().read(true).write(true).open(&place).unwrap();
        let fd = far.as_raw_fd();
        let unread = || {
            let mut n: libc::c_int = 0;
            // SAFETY: far is open for the whole test, and FIONREAD writes
            // one int.
            assert_eq!(unsafe { libc::ioctl(fd, libc::FIONREAD, &mut n) }, 0);
            n
        };
        let wait_for = |want: &dyn Fn(libc::c_int) -> bool, what: &str| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !want(unread()) {
                assert!(Instant::now() < deadline, "{what}");
                thread::yield_now();
            }
        };
        for b in *b"old" {
            ep.put_char(b);
        }
        wait_for(&|n| n == 3, "the bytes never reached the slave");
        wait_for(&|n| n == 0, "unread bytes were kept");
        ep.put_char(b'n');
        let mut got = [0u8; 1];
        far.read_exact(&mut got).unwrap();
        assert_eq!(got[0], b'n');

        // more at once than the pty holds: the writer waits for room
        // where it can still see nothing is being read, not in the write
        for _ in 0..OUT_QUEUE {
            ep.put_char(b'x');
        }
        wait_for(&|n| n > 0, "the flood never reached the slave");
        wait_for(&|n| n == 0, "a full pty was kept");
    }

    /// A pty's slave is a raw line to the endpoint.
    #[test]
    fn a_pty_line_carries_bytes_both_ways() {
        let (mut ep, place) = open(&SerialSpec::Pty).unwrap();
        let mut far = std::fs::OpenOptions::new().read(true).write(true).open(&place).unwrap();
        far.write_all(b"\rq").unwrap();
        assert_eq!(next_byte(&mut ep), Some(b'\r'), "raw: no CR translation");
        assert_eq!(next_byte(&mut ep), Some(b'q'));
        ep.put_char(b'\n');
        let mut got = [0u8; 1];
        far.read_exact(&mut got).unwrap();
        assert_eq!(got[0], b'\n', "raw: no LF to CRLF");
    }
}
