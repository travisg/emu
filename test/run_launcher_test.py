#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test of the launcher, on a pty: the part the unit tests in
src/launcher cannot reach -- raw mode and its undoing, the decision in
main that bare `emu` on a terminal opens the menu, and the chosen line
spliced in front of the rest of the command line and run.

It runs the emulator in a scratch directory holding only the 703 demo and
a decoy core image beside it, so the menu has one machine to offer and
the same items on every host, whatever roms it has fetched. The cases:

  bare       `emu` on a terminal, keys one at a time: Enter onto the 703's
             items, Enter on the default -- the demo -- and its banner.
  burst      `emu -L --fast-io`, every key in one write: End to the last
             switch, space for --no-throttle, Enter. The printed line must
             be the menu's choice with --fast-io after it, and the banner
             must come.
  quit       `emu` and q: nothing starts and the exit status is 0.
  piped      bare `emu` with no terminal: no menu, the 6809 as ever (whose
             rom is not there, which is the point -- it is the 6809's path
             that is taken).

Each terminal case brackets the emulator with `stty -g` and requires the
two to match. Every wait raises on its timeout, and the handler at the
bottom turns that into a FAIL with the pty's output in the log. Negative
controls, both of which must FAIL: LAUNCHER_CONTROL=wrong-item moves up
onto the decoy in the bare case, which launches -- its line is the one
expected -- and never shows the banner; =stty turns echo off after the
emulator exits, which the settings comparison must catch.

Needs roms/703/demo.bin (`make -C test ray703-demo`) and python3.
"""

import os
import pty
import re
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'launcher_test.log')
CONTROL = os.environ.get('LAUNCHER_CONTROL', '')
DEMO = os.path.join(ROOT, 'roms', '703', 'demo.bin')

BANNER = b'RAYTHEON 703 READY'
ALT_SCREEN = b'\x1b[?1049h'
UP, END = b'\x1b[A', b'\x1b[F'
TIMEOUT = 20


class Pty:
    """A shell command on a pty of its own, its output collected."""

    def __init__(self, command, cwd, log):
        self.log = log
        self.out = b''
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(cwd)
            os.execvp('sh', ['sh', '-c', command])
        import fcntl
        import termios
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        self.status = None

    def pump(self, seconds):
        end = time.time() + seconds
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if not r:
                continue
            try:
                data = os.read(self.fd, 65536)
            except OSError:  # the far end is closed
                data = b''
            if not data:
                return False
            self.out += data
            self.log.write(data)
        return True

    def expect(self, what, start=0, timeout=TIMEOUT):
        """Wait for `what` in the output after `start`; its end."""
        end = time.time() + timeout
        while True:
            at = self.out.find(what, start)
            if at >= 0:
                return at + len(what)
            if time.time() > end or not self.pump(0.1) and self.out.find(what, start) < 0:
                raise AssertionError('never saw %r' % what)

    def send(self, data):
        os.write(self.fd, data)

    def wait(self, timeout=TIMEOUT):
        """Collect the rest of the output and the exit status."""
        end = time.time() + timeout
        while time.time() < end and self.pump(0.1):
            pass
        _, status = os.waitpid(self.pid, 0)
        self.status = status
        return os.waitstatus_to_exitcode(status)

    def kill(self):
        if self.status is None:
            try:
                os.kill(self.pid, 9)
                os.waitpid(self.pid, 0)
            except (ProcessLookupError, ChildProcessError):
                pass


def bracketed(emu_args):
    """The emulator between two `stty -g`, with its exit status after."""
    meddle = ' stty -echo;' if CONTROL == 'stty' else ''
    return 'stty -g; %s %s; echo "rc=$?";%s stty -g' % (EMU_BIN, emu_args, meddle)


def settings(out):
    """The two `stty -g` lines in a case's output."""
    lines = [line.strip() for line in out.decode('latin-1').replace('\r', '').split('\n')]
    found = [line for line in lines if re.fullmatch(r'[0-9a-f]+(:[0-9a-f]+){30,}', line)]
    if len(found) != 2:
        raise AssertionError('expected two stty -g lines, found %d' % len(found))
    return found


def exit_status(out):
    m = re.search(rb'rc=(\d+)', out)
    if not m:
        raise AssertionError('no exit status')
    return int(m.group(1))


def terminal_case(name, emu_args, keys, workdir, log, launches, printed=None):
    """Run one case: `keys` is a list of writes, paced apart. A launching
    case must print `printed`, show the banner and leave on Ctrl-D."""
    log.write(('\n=== %s: emu %s ===\n' % (name, emu_args)).encode())
    p = Pty(bracketed(emu_args), workdir, log)
    try:
        menu = p.expect(ALT_SCREEN)
        p.pump(0.5)  # the first draw
        for k in keys:
            p.send(k)
            p.pump(0.3)
        if launches:
            line = b'$ %s %s' % (EMU_BIN.encode(), printed.encode())
            after = p.expect(line + b'\r\n', menu)
            p.expect(BANNER, after)
            p.send(b'\x04')
        p.wait()
        before, after = settings(p.out)
        if before != after:
            raise AssertionError('%s: the terminal was left changed:\n  %s\n  %s' % (name, before, after))
        if exit_status(p.out) != 0:
            raise AssertionError('%s: exit status %d' % (name, exit_status(p.out)))
        if not launches and b'Starting system thread' in p.out:
            raise AssertionError('%s: a machine started' % name)
    finally:
        p.kill()


def piped_case(workdir, log):
    log.write(b'\n=== piped: emu with no terminal ===\n')
    r = subprocess.run([EMU_BIN], cwd=workdir, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                       stderr=subprocess.STDOUT, timeout=TIMEOUT)
    log.write(r.stdout)
    if ALT_SCREEN in r.stdout:
        raise AssertionError('piped: the menu opened with no terminal')
    if b'rom is roms/6809/BASIC.HEX' not in r.stdout:
        raise AssertionError('piped: bare emu was not the 6809')


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if not os.path.isfile(DEMO):
        print('error: %s is missing; make -C test ray703-demo' % DEMO, file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix='emu-launcher-') as workdir, open(LOG_FILE, 'wb') as log:
        os.makedirs(os.path.join(workdir, 'roms', '703'))
        shutil.copy(DEMO, os.path.join(workdir, 'roms', '703', 'demo.bin'))
        # sorts ahead of the demo, so it is the item above the default
        with open(os.path.join(workdir, 'roms', '703', 'decoy.bin'), 'wb') as f:
            f.write(bytes(256))
        try:
            if CONTROL == 'wrong-item':
                pick, image = [b'\r', UP, b'\r'], 'decoy.bin'
            else:
                pick, image = [b'\r', b'\r'], 'demo.bin'
            terminal_case('bare', '', pick, workdir, log, True, '-s ray703 -r roms/703/' + image)
            terminal_case('burst', '-L --fast-io', [b'\r' + END + b' \r'], workdir, log, True,
                          '-s ray703 -r roms/703/demo.bin --no-throttle --fast-io')
            terminal_case('quit', '', [b'q'], workdir, log, False)
            piped_case(workdir, log)
        except Exception:
            log.write(b'\n--- harness failure ---\n')
            log.write(traceback.format_exc().encode())
            print('FAIL: launcher test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: launcher test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
