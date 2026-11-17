// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The picker's terminal: raw mode on the alternate screen, keys decoded
//! from what the terminal sends, and the size.
//!
//! Everything is put back on drop, panics included, and before `run`
//! returns, so the machine's own console frontend saves and restores the
//! user's settings rather than the picker's.

use super::picker::Key;
use std::io::{self, Write};

/// Raw mode and the alternate screen, undone on drop.
pub struct Screen {
    saved: libc::termios,
    /// Bytes read and not yet decoded: one read can hold several keys.
    pending: Vec<u8>,
}

impl Screen {
    /// `Err` when stdin is not a terminal: there is nobody to pick.
    pub fn open() -> io::Result<Screen> {
        // SAFETY: plain termios calls on stdin; the struct is filled in by
        // tcgetattr before it is read.
        let saved = unsafe {
            let mut saved: libc::termios = std::mem::zeroed();
            if libc::isatty(0) == 0 || libc::isatty(1) == 0 || libc::tcgetattr(0, &mut saved) != 0 {
                return Err(io::Error::other("needs a terminal on stdin and stdout"));
            }
            let mut t = saved;
            // ISIG off too, so Ctrl-C arrives as a key and quits through the
            // same path as q, restoring the terminal on the way
            t.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
            t.c_iflag &= !(libc::ICRNL | libc::IXON);
            t.c_cc[libc::VMIN] = 1;
            t.c_cc[libc::VTIME] = 0;
            libc::tcsetattr(0, libc::TCSANOW, &t);
            saved
        };
        // alternate screen, cursor hidden
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout().flush()?;
        Ok(Screen { saved, pending: Vec::new() })
    }

    /// Columns and rows, or 80x24 when the terminal will not say.
    pub fn size(&self) -> (usize, usize) {
        // SAFETY: TIOCGWINSZ fills in a winsize we own.
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
                (ws.ws_col as usize, ws.ws_row as usize)
            } else {
                (80, 24)
            }
        }
    }

    pub fn draw(&self, lines: &[String]) -> io::Result<()> {
        let mut out = String::from("\x1b[H");
        for (i, l) in lines.iter().enumerate() {
            out.push_str(l);
            out.push_str("\x1b[K");
            // no newline after the last line, which would scroll the screen
            if i + 1 < lines.len() {
                out.push_str("\r\n");
            }
        }
        out.push_str("\x1b[J");
        let mut stdout = io::stdout().lock();
        stdout.write_all(out.as_bytes())?;
        stdout.flush()
    }

    /// The next key, or `None` after `timeout_ms` with none, so a resize
    /// is redrawn without waiting for one.
    pub fn key(&mut self, timeout_ms: i32) -> io::Result<Option<Key>> {
        if self.pending.is_empty() {
            if !readable(timeout_ms) {
                return Ok(None);
            }
            if self.fill()? == 0 {
                return Ok(Some(Key::Quit)); // end of input
            }
        }
        loop {
            if let Some((key, used)) = decode(&self.pending) {
                self.pending.drain(..used);
                return Ok(Some(key));
            }
            // the start of a sequence: the rest follows at once, or this
            // was the Esc key (or something unknown) on its own
            if !readable(30) || self.fill()? == 0 {
                let alone = self.pending == [0x1b];
                self.pending.clear();
                return Ok(Some(if alone { Key::Esc } else { Key::Other }));
            }
        }
    }

    /// Read what there is onto `pending`. Straight from the descriptor, not
    /// through std's buffered stdin, which could take bytes `poll` then
    /// cannot see.
    fn fill(&mut self) -> io::Result<usize> {
        let mut buf = [0u8; 64];
        // SAFETY: reading into a buffer we own, no further than its length
        let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        self.pending.extend_from_slice(&buf[..n as usize]);
        Ok(n as usize)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
        // SAFETY: restoring the settings saved in open()
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.saved);
        }
    }
}

fn readable(timeout_ms: i32) -> bool {
    let mut fds = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
    // SAFETY: one pollfd we own
    unsafe { libc::poll(&mut fds, 1, timeout_ms) > 0 }
}

/// The first key in `b` and how many bytes it took, or `None` when `b` is
/// the start of an escape sequence that has not all arrived.
pub fn decode(b: &[u8]) -> Option<(Key, usize)> {
    let one = |k| Some((k, 1));
    match b {
        [] => None,
        [0x1b] => None,
        [0x1b, 0x1b, ..] => one(Key::Esc),
        // CSI and SS3 forms both: which one arrives depends on the
        // terminal's cursor-key mode
        [0x1b, b'[' | b'O'] => None,
        [0x1b, b'[' | b'O', k, ..] if !k.is_ascii_digit() => {
            let key = match k {
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                b'H' => Key::Home,
                b'F' => Key::End,
                b'Z' => Key::BackTab,
                _ => Key::Other,
            };
            Some((key, 3))
        }
        // ESC [ digits ~, and anything else in CSI's parameter bytes
        [0x1b, b'[', rest @ ..] => {
            let end = rest.iter().position(|c| !c.is_ascii_digit() && *c != b';')?;
            let key = match (&rest[..end], rest[end]) {
                (b"1" | b"7", b'~') => Key::Home,
                (b"4" | b"8", b'~') => Key::End,
                _ => Key::Other,
            };
            Some((key, 2 + end + 1))
        }
        // Alt and a key, or an SS3 we do not know
        [0x1b, ..] => Some((Key::Other, 2)),
        [b'\r', b'\n', ..] => Some((Key::Enter, 2)),
        [c, ..] => one(match c {
            b'\r' | b'\n' => Key::Enter,
            b'\t' => Key::Tab,
            b' ' => Key::Space,
            b'k' => Key::Up,
            b'j' => Key::Down,
            b'h' => Key::Left,
            b'l' => Key::Right,
            b'q' | b'Q' | 0x03 | 0x04 => Key::Quit,
            _ => Key::Other,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_what_terminals_send() {
        for (bytes, key) in [
            (&b"\x1b[A"[..], Key::Up),
            (b"\x1bOB", Key::Down),
            (b"\x1b[C", Key::Right),
            (b"\x1bOD", Key::Left),
            (b"\x1b[Z", Key::BackTab),
            (b"\x1b[1~", Key::Home),
            (b"\x1b[4~", Key::End),
            (b"\x1b", Key::Esc),
            (b"\r", Key::Enter),
            (b"\n", Key::Enter),
            (b"\t", Key::Tab),
            (b" ", Key::Space),
            (b"q", Key::Quit),
            (b"\x03", Key::Quit),
            (b"\x04", Key::Quit),
            (b"x", Key::Other),
            (b"\x1b[15~", Key::Other),
        ] {
            match bytes {
                // a lone escape waits to see whether more follows
                b"\x1b" => assert_eq!(decode(bytes), None),
                _ => assert_eq!(decode(bytes), Some((key, bytes.len())), "{bytes:?}"),
            }
        }
    }

    #[test]
    fn several_keys_in_one_read_are_each_decoded() {
        let mut b: &[u8] = b"\x1b[B\x1bOA\x1b[4~\r\nq\x1b\x1b[A";
        let mut got = Vec::new();
        while let Some((key, used)) = decode(b) {
            got.push(key);
            b = &b[used..];
        }
        assert_eq!(got, [Key::Down, Key::Up, Key::End, Key::Enter, Key::Quit, Key::Esc, Key::Up]);
        assert!(b.is_empty());
    }

    #[test]
    fn a_sequence_cut_short_waits_for_the_rest() {
        for part in [&b"\x1b"[..], b"\x1b[", b"\x1bO", b"\x1b[1", b"\x1b[15"] {
            assert_eq!(decode(part), None, "{part:?}");
        }
    }
}
