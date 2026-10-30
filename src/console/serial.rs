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
//! than stalling the CPU thread. Received bytes queue without limit; the
//! machine's SIO holds them on the wire while its RTS is off.

use super::ConsoleEndpoint;
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::FromRawFd;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Bytes the guest may get ahead of a slow far end before they are lost.
const OUT_QUEUE: usize = 4096;

/// Where the far end of the line is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialSpec {
    /// A new pseudo-terminal; its slave's path is announced at startup.
    Pty,
    /// A TCP listener on localhost at this port; 0 picks a free one.
    Tcp(u16),
}

impl SerialSpec {
    /// `pty` or `tcp:PORT`.
    pub fn parse(s: &str) -> Result<Self, String> {
        if s == "pty" {
            return Ok(SerialSpec::Pty);
        }
        if let Some(port) = s.strip_prefix("tcp:") {
            return port.parse().map(SerialSpec::Tcp).map_err(|_| format!("'{port}' is not a port number"));
        }
        Err(format!("'{s}' is not 'pty' or 'tcp:PORT'"))
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
        let p = libc::ttyname(slave);
        if p.is_null() {
            return Err(io::Error::last_os_error());
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
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
        for b in out_rx {
            let _ = writer.write_all(&[b]);
        }
    });
    Ok(name)
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
            let mut c = for_writer.lock().unwrap();
            if let Some(s) = c.as_mut() {
                if s.write_all(&[b]).is_err() {
                    *c = None;
                }
            }
        }
    });
    Ok(place)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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

    /// Bytes cross a TCP line in both directions; a byte sent with nobody
    /// connected is lost rather than queued for whoever comes next.
    #[test]
    fn a_tcp_line_carries_bytes_both_ways() {
        let (mut ep, place) = open(&SerialSpec::Tcp(0)).unwrap();
        ep.put_char(b'!'); // nobody there
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
        assert_eq!(&got, b"ok", "the byte sent to nobody is not among them");
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
