#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""The Z80 instruction exercisers on the Kaypro: zexdoc, then zexall.

Frank Cringle's exercisers run every instruction over a set of register and
memory combinations, CRC the machine state after each, and compare the CRC
with one taken on real silicon -- zexdoc masks the undocumented flag bits,
zexall does not. A group prints OK or the two CRCs; there is no finer
verdict, so a failure names the instruction and the rest is reading the
core against the flag tables. They are CP/M programs, so the harness builds
a floppy for them: a copy of the stock Kaypro floppy (which carries the
system track a boot needs) with its files replaced by the exercisers, made
with cpmtools' kpii definition, which is this controller's geometry.

The emulator runs headless under SDL's dummy video driver and is driven over
its debug port (tools/emudbg.py). The Kaypro's console is its screen, which
the port's output tap never sees, so the transcript is taken at the BDOS
entry instead: a breakpoint at 5, function 2 a character in E, function 9
a '$'-terminated string at DE, and function 10 -- the CCP reading a command
line -- the moment to type the next program's name. Every wait raises on
its timeout and one handler turns that into a FAIL with the transcript in
the log; a group printing anything but OK, or the machine stopping for any
reason but the breakpoint (a bad opcode halts it), fails the same way.

zexall is billions of instructions: minutes in a release build with
--no-throttle, most of an hour in a debug one, so the release binary is
preferred when it exists. ZEX_PROGRAMS narrows the run (`ZEX_PROGRAMS=zexdoc`).
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402


def default_emu():
    release = os.path.join(ROOT, 'target', 'release', 'emu')
    debug = os.path.join(ROOT, 'target', 'debug', 'emu')
    return release if os.access(release, os.X_OK) else debug


EMU_BIN = os.environ.get('EMU_BIN', default_emu())
FLOPPY = os.path.join(ROOT, 'disks', 'mbasic-games.img')
CPM_DIR = os.path.join(ROOT, 'disks', 'cpm')
PROGRAMS = os.environ.get('ZEX_PROGRAMS', 'zexdoc zexall').split()
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'kaypro_zex_test.log')
TIMEOUT = float(os.environ.get('ZEX_TIMEOUT', '3600'))

BDOS = 0x0005


def build_floppy(workdir):
    """A copy of the stock floppy with the exercisers on it in place of the
    games, which fill it. cpmrm's wildcard is its own, not the shell's; its
    status is ignored because the stock floppy carries one entry (a
    lower-case `trade.asc`, empty) that it lists but cannot erase, and the
    copy that follows is the check that the room was made."""
    image = os.path.join(workdir, 'zex.img')
    shutil.copyfile(FLOPPY, image)
    subprocess.run(['cpmrm', '-f', 'kpii', image, '0:*.*'], check=False,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    files = [os.path.join(CPM_DIR, p + '.com') for p in PROGRAMS]
    subprocess.run(['cpmcp', '-f', 'kpii', image] + files + ['0:'], check=True)
    return image


def run_exercisers(emu, log):
    """Drive the CCP through the programs, transcribing BDOS console output
    to the log and checking each group's verdict as it comes. Returns the
    number of groups that reported OK."""
    programs = list(PROGRAMS)
    running = None          # the program typed last, until its CCP prompt returns
    completed = False       # it printed "Tests complete"
    line = bytearray()
    ok = 0

    def emit(data):
        nonlocal line, ok, completed
        log.write(data)
        log.flush()
        for b in data:
            if b not in (0x0d, 0x0a):
                line.append(b)
                continue
            text = line.decode('latin-1').strip()
            line = bytearray()
            if 'ERROR' in text:
                raise AssertionError('%s: %s' % (running, text))
            if text.endswith('OK'):
                ok += 1
            elif text == 'Tests complete':
                completed = True

    while True:
        stop = emu.wait_stopped(timeout=TIMEOUT)
        if stop.reason != 'break' or stop.pc != BDOS:
            raise AssertionError('the machine stopped: %r' % (stop,))
        regs = emu.regs()
        func = regs['BC'] & 0xff
        de = regs['DE']
        if func == 2:
            emit(bytes([de & 0xff]))
        elif func == 9:
            text = bytearray()
            while True:
                chunk = emu.mem(de, 64)
                end = chunk.find(b'$')
                if end >= 0:
                    text += chunk[:end]
                    break
                text += chunk
                de = (de + 64) & 0xffff
            emit(bytes(text))
        elif func == 10:
            # the CCP wants a command line: the previous program is done
            if running is not None and not completed:
                raise AssertionError('%s ended without "Tests complete"' % running)
            if not programs:
                return ok
            running = programs.pop(0)
            completed = False
            emu.key(running + '\r')
        emu.run()


def main():
    for path, hint in [
        (EMU_BIN, 'build first with: cargo build --release'),
        (FLOPPY, 'fetch it with: tools/fetch-roms.py'),
    ] + [(os.path.join(CPM_DIR, p + '.com'), 'fetch it with: tools/fetch-roms.py') for p in PROGRAMS]:
        if not os.path.exists(path):
            print('error: %s not found; %s' % (path, hint), file=sys.stderr)
            return 1
    if shutil.which('cpmcp') is None:
        print('error: cpmtools (cpmcp, cpmrm) not found', file=sys.stderr)
        return 1

    # headless: no window, software rendering
    os.environ['SDL_VIDEODRIVER'] = 'dummy'
    os.environ['SDL_RENDER_DRIVER'] = 'software'

    emu = None
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='kaypro-zex-') as workdir, open(LOG_FILE, 'wb') as log:
        try:
            image = build_floppy(workdir)
            # halted at start so the breakpoint is in place before CP/M's
            # first BDOS call; the ROM paths are relative to the repo root
            emu = Emu.spawn([EMU_BIN, '-s', 'kaypro', '--disk', image, '--no-throttle'],
                            log=log, timeout=TIMEOUT)
            emu.add_break(BDOS)
            emu.run()
            groups = run_exercisers(emu, log)
            if groups == 0:
                raise AssertionError('no exerciser group reported at all')
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
                log.write('\n'.join(emu.transcript[-200:]).encode('latin-1', 'replace') + b'\n')
                if emu.child is not None and emu.child.poll() is None:
                    emu.child.kill()
                    emu.child.wait()
                    emu.child.stdin.close()
                emu.close()
            print('FAIL: kaypro z80 exerciser test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: kaypro z80 exerciser test, %s: %d groups OK in %.0fs (%s)'
          % (' '.join(PROGRAMS), groups, time.monotonic() - started, EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
