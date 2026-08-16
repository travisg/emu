// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! Raw-terminal console frontend.

use super::ConsoleFrontend;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Saves the terminal settings and restores them on drop.
struct RawMode {
    saved_stdin: libc::termios,
    saved_stdout: libc::termios,
}

impl RawMode {
    fn enable() -> Option<RawMode> {
        // SAFETY: plain termios calls on fds we own; the structs are fully
        // initialised by tcgetattr before being read.
        unsafe {
            let mut saved_stdin: libc::termios = std::mem::zeroed();
            let mut saved_stdout: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut saved_stdin) != 0 {
                // not a tty (piped input, as the test harness does) -- nothing
                // to configure and nothing to restore
                return None;
            }
            libc::tcgetattr(1, &mut saved_stdout);

            let mut t = saved_stdin;
            // no input processing, and pass the control characters through to
            // the guest rather than acting on them
            t.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            t.c_cc[libc::VINTR] = 0;
            t.c_cc[libc::VQUIT] = 0;
            t.c_cc[libc::VSUSP] = 0;
            t.c_cc[libc::VMIN] = 1;
            t.c_cc[libc::VTIME] = 0;
            libc::tcsetattr(0, libc::TCSANOW, &t);

            Some(RawMode { saved_stdin, saved_stdout })
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // SAFETY: restoring settings captured in enable()
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.saved_stdin);
            libc::tcsetattr(1, libc::TCSANOW, &self.saved_stdout);
        }
    }
}

pub struct TerminalFrontend {
    tx: Sender<u8>,
    _raw: Option<RawMode>,
}

impl TerminalFrontend {
    pub fn new(tx: Sender<u8>) -> Self {
        TerminalFrontend { tx, _raw: RawMode::enable() }
    }
}

impl ConsoleFrontend for TerminalFrontend {
    fn run(&mut self, shutdown: Arc<AtomicBool>) {
        loop {
            if shutdown.load(Ordering::SeqCst) {
                println!("console stop requested, exiting");
                return;
            }

            // Wait for stdin with a timeout rather than blocking in a read, so
            // a cycle-limit exit on the cpu thread is noticed promptly. This
            // mirrors the C++ fix in 774f5da; blocking here is what used to
            // hang the emulator until the user pressed ctrl-d.
            let mut pfd = libc::pollfd { fd: 0, events: libc::POLLIN, revents: 0 };
            // SAFETY: single valid pollfd, 100ms timeout
            let ret = unsafe { libc::poll(&mut pfd, 1, 100) };
            if ret <= 0 {
                continue;
            }

            let mut buf = [0u8; 1];
            // SAFETY: reading one byte into a stack buffer we own
            let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, 1) };
            if n <= 0 {
                println!("EOF on console, exiting");
                shutdown.store(true, Ordering::SeqCst);
                return;
            }

            if buf[0] == 0x4 {
                println!("ctrl-d on console, exiting");
                shutdown.store(true, Ordering::SeqCst);
                return;
            }

            if self.tx.send(buf[0]).is_err() {
                // cpu thread is gone
                shutdown.store(true, Ordering::SeqCst);
                return;
            }
        }
    }
}
