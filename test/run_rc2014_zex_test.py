#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""The Z80 instruction exercisers on the RC2014's CP/M: zexdoc, then zexall.

The same exercisers as test/run_kaypro_zex_test.py, on the other CP/M
machine. Here the console is the SIO, so the debug port's output tap sees
everything the guest prints and there is no BDOS breakpoint to transcribe
through: the harness boots `rc2014-cpm` on a scratch copy of the card with
the exercisers put on drive A by cpmtools (test/diskdefs defines the
card's geometry), walks the monitor into CP/M, types each program's name
at the `A>` and reads its lines off the port. Every group must print OK;
an ERROR line, a program that does not print "Tests complete", or the
machine stopping (a bad opcode halts it) is a FAIL, with the transcript
in the log.

zexall is billions of instructions: minutes in a release build with
--no-throttle, most of an hour in a debug one, so the release binary is
preferred when it exists. ZEX_PROGRAMS narrows the run (`ZEX_PROGRAMS=zexdoc`).
"""

import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu, Stopped, Exited  # noqa: E402


def default_emu():
    release = os.path.join(ROOT, 'target', 'release', 'emu')
    debug = os.path.join(ROOT, 'target', 'debug', 'emu')
    return release if os.access(release, os.X_OK) else debug


EMU_BIN = os.environ.get('EMU_BIN', default_emu())
ROM_FILE = os.environ.get('ROM_FILE', os.path.join(ROOT, 'roms', 'rc2014', '24886009.BIN'))
CF_IMAGE = os.environ.get('CF_IMAGE', os.path.join(ROOT, 'disks', 'rc2014-cf.img'))
CPM_DIR = os.path.join(ROOT, 'disks', 'cpm')
PROGRAMS = os.environ.get('ZEX_PROGRAMS', 'zexdoc zexall').split()
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'rc2014_zex_test.log')
TIMEOUT = float(os.environ.get('ZEX_TIMEOUT', '3600'))

# One exerciser's report: group lines, then the summary.
GROUP_OK = re.compile(rb'^.*\.\.\s*OK\s*$', re.M)
DONE_OR_ERROR = re.compile(rb'Tests complete|ERROR')


def build_card(workdir):
    """A copy of the card with the exercisers on drive A. cpmtools takes
    the geometry from the diskdefs beside this script, so it runs there."""
    card = os.path.join(workdir, 'card.img')
    shutil.copyfile(CF_IMAGE, card)
    files = [os.path.join(CPM_DIR, p + '.com') for p in PROGRAMS]
    subprocess.run(['cpmcp', '-f', 'rc2014-a', card] + files + ['0:'], check=True, cwd=HERE)
    listing = subprocess.run(['cpmls', '-f', 'rc2014-a', card], check=True, cwd=HERE,
                             capture_output=True, text=True).stdout
    for p in PROGRAMS:
        if p + '.com' not in listing:
            raise AssertionError('%s.com did not land on the card:\n%s' % (p, listing))
    return card


def main():
    for path, hint in [
        (EMU_BIN, 'build first with: cargo build --release'),
        (ROM_FILE, 'fetch it with: tools/fetch-roms.py'),
        (CF_IMAGE, 'fetch it with: tools/fetch-roms.py'),
    ] + [(os.path.join(CPM_DIR, p + '.com'), 'fetch it with: tools/fetch-roms.py') for p in PROGRAMS]:
        if not os.path.exists(path):
            print('error: %s not found; %s' % (path, hint), file=sys.stderr)
            return 1
    if shutil.which('cpmcp') is None:
        print('error: cpmtools (cpmcp, cpmls) not found', file=sys.stderr)
        return 1

    emu = None
    started = time.monotonic()
    ok = 0
    with tempfile.TemporaryDirectory(prefix='rc2014-zex-') as workdir, open(LOG_FILE, 'wb') as log:
        try:
            card = build_card(workdir)
            emu = Emu.spawn([EMU_BIN, '-s', 'rc2014-cpm', '-r', ROM_FILE, '--disk', card, '--no-throttle'],
                            log=log, timeout=TIMEOUT)
            emu.run()
            pos = 0

            def expect(pattern, timeout=None):
                """The next match at or after the cursor. A stop or exit
                event arriving meanwhile is the machine giving up, and
                is reported as such rather than as a silent timeout."""
                nonlocal pos
                deadline = time.monotonic() + (timeout if timeout is not None else TIMEOUT)
                while True:
                    try:
                        m = emu.wait_for_output(pattern, timeout=5, start=pos)
                    except TimeoutError:
                        try:
                            ev = emu.events.get_nowait()
                        except queue.Empty:
                            ev = None
                        if isinstance(ev, (Stopped, Exited)):
                            raise AssertionError('the machine stopped: %r' % (ev,))
                        if time.monotonic() > deadline:
                            raise
                        continue
                    pos = m.end()
                    return m

            expect(b'Press [SPACE] to activate console', 20)
            emu.key(' ')
            expect(b'\r\n>', 20)
            emu.key('X')
            expect(b'Boot CP/M?', 20)
            emu.key('Y')
            expect(b'\r\nA>', 20)
            for program in PROGRAMS:
                emu.key(program + '\r')
                begin = pos
                m = expect(DONE_OR_ERROR)
                report = bytes(emu.output)[begin:m.end()]
                if m.group() != b'Tests complete':
                    line = report.splitlines()[-1].decode('latin-1')
                    raise AssertionError('%s: %s' % (program, line))
                groups = len(GROUP_OK.findall(report))
                if groups == 0:
                    raise AssertionError('%s printed no OK line before "Tests complete"' % program)
                ok += groups
                expect(b'\r\nA>', 20)
            emu.kill()
            emu.wait_exited()
            status = emu.wait_exit()
            if status != 0:
                raise AssertionError('the emulator exited with status %d' % status)
        except Exception:
            log.write(b'\n--- harness failure ---\n')
            log.write(traceback.format_exc().encode())
            if emu is not None:
                log.write(b'\n--- transcript ---\n')
                log.write('\n'.join(emu.transcript).encode('latin-1', 'replace') + b'\n')
                log.write(b'\n--- output ---\n')
                log.write(bytes(emu.output) + b'\n')
                if emu.child is not None and emu.child.poll() is None:
                    emu.child.kill()
                    emu.child.wait()
                    emu.child.stdin.close()
                emu.close()
            print('FAIL: rc2014 z80 exerciser test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: rc2014 z80 exerciser test, %s: %d groups OK in %.0fs (%s)'
          % (' '.join(PROGRAMS), ok, time.monotonic() - started, EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
