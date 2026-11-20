#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Compile and run FORTRAN on the 703 with Raytheon's Conversational FORTRAN,
the tapes going through the machine's own reader and punch.

Three tapes, as the University of Stuttgart's computer museum read them off
its 704 (tools/fetch-roms.py puts them in roms/703/tapes): the Paper Tape
I/O System the museum's site used in place of X-RAY (DN 393954), the
Conversational FORTRAN compiler for a 4K machine (DN 393295 rev B) and its
run-time library (DN 394002 rev B). The session is the one the 706 User's
Manual gives (1-6.4.3): load the compiler, compile into memory, load the
library over the compiler, execute.

PTIOS is written into core, as its bootstrap would have left it, and started
at word X'80'. Everything after that the machine does. Its units are
assigned from the teletype -- the compiler's source and the loader's tapes
to the high speed reader, one unit to the punch, one more to the reader for
data -- and each tape is threaded through the reader with the debug port's
`tape` when the program that wants it is about to ask: PTIOS's absolute
loader reads the compiler and the library, the compiler reads its source,
and the second program reads what the first one punched, torn off with
`savetape` in between. PTIOS runs every device by asking its status word,
with no interrupt level enabled, so this is the reader's and the punch's
status words and both tapes' frames under period code.

The session is paced on the guest's own requests rather than on the clock:
breakpoints on PTIOS's I/O entry (X'44'), its loader (X'78') and the word a
loaded tape starts at (X'80') say what it is about to do, and the file table
the I/O call names says to which unit. The commands -- G to compile, E to go
back to the loader, G to run -- are typed at the teletype when it asks.

The first program prints a table and punches it; the second reads the tape
back and prints a sum. Both are checked against the host's arithmetic. This
library's F conversion cuts a number off rather than rounding it, so a real
is good when it is within a unit of its last place.

Negative controls, each of which must FAIL: FORTRAN_CONTROL=source (a
statement the compiler rejects), =answer (the first program computes
something else) and =tape (a frame of the punched tape changed before it is
read back).
"""

import math
import os
import queue
import re
import shutil
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu, Stopped  # noqa: E402
import tape703  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
TAPE_DIR = os.path.abspath(os.environ.get('TAPE_DIR', os.path.join(ROOT, 'roms', '703', 'tapes')))
LOG_FILE = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_fortran_test.log')
CONTROL = os.environ.get('FORTRAN_CONTROL', '')
CONTROLS = ('source', 'answer', 'tape')
TIMEOUT = float(os.environ.get('FORTRAN_TIMEOUT', '120'))

PTIOS, COMPILER, RUNTIME = 'PTIOS.bin', '393295B.bin', '394002B.bin'

# PTIOS in core: its I/O entry, its absolute loader, and where the tapes it
# loads begin.
DOIO, LOADER, START = 0x44, 0x78, 0x80
TELETYPE, READER, PUNCH = 0xe, 0xd, 0xc
# The units: 2 is where the compiler reads its source and 4 where the
# loader reads a tape, by their own file tables; 5 and 6 are the programs'.
ASSIGN = ['2:R', '4:R', '5:P', '6:R']
ROWS = 5

TABLE = '''\
      DO 10 K=1,5
      X=K
      Y=SQRT(X)+X**2/3.
      J=K*K-7
      WRITE (13,1) K,J,Y
10    WRITE (5,2) K,J
1     FORMAT (I3,I5,F10.4)
2     FORMAT (I3,I5)
      STOP
      END
'''
SUM = '''\
      M=0
      DO 10 N=1,5
      READ (6,2) K,J
10    M=M+K*J
      WRITE (13,1) M
1     FORMAT (I8)
2     FORMAT (I3,I5)
      STOP
      END
'''


class Failed(Exception):
    pass


def source_tape(text):
    """A program as the reader wants it: each line a record, opened by a
    line feed and closed by a carriage return, blank tape between."""
    return b''.join(b'\xa0\xa0\xa0\x8a' + bytes(c | 0x80 for c in line.encode('ascii')) + b'\x8d'
                    for line in text.splitlines())


class Session:
    def __init__(self, emu, workdir, log):
        self.emu, self.workdir, self.log = emu, workdir, log
        self.seen = 0
        self.deadline = time.monotonic() + TIMEOUT

    def printed(self):
        """What the teletype has printed since this was last asked."""
        text = bytes(c & 0x7f for c in self.emu.output[self.seen:]).replace(b'\0', b'').decode('ascii', 'replace')
        self.seen = len(self.emu.output)
        return text

    def stop(self):
        """Run to the next breakpoint: its address, and for the I/O entry
        the device and the function of the call."""
        self.emu.run()
        try:
            ev = self.emu.events.get(timeout=max(0.0, self.deadline - time.monotonic()))
        except queue.Empty:
            self.emu.halt()
            raise Failed('it went no further; it is at %04X' % self.emu.status()['pc']) from None
        if not isinstance(ev, Stopped):
            raise Failed('the emulator went away: %r' % (ev,))
        if ev.reason != 'break':
            raise Failed('stopped (%s) at %04X in front of `%s`' % (ev.reason, ev.pc, ev.insn))
        if ev.pc != DOIO:
            return ev.pc, None, None
        # the call is JSX X'44' with the file table's address after it,
        # and the table's third word is the unit and the function
        table = self.emu.memw(self.emu.reg('IX'), 1)[0] & 0x7fff
        operation = self.emu.memw(table + 2, 1)[0]
        unit, function = (operation >> 4) & 0xf, operation & 0xf
        # PTIOS's assignment table: a byte to a unit, from word 0
        word = self.emu.memw(unit // 2, 1)[0]
        device = (word >> 8 if unit % 2 == 0 else word & 0xff) >> 4
        return ev.pc, device, function

    def until(self, what, want):
        """Run until `want(address, device, function)`."""
        while True:
            at = self.stop()
            if want(*at):
                return
            if at[0] == LOADER and what != 'the loader':
                raise Failed('back in the loader waiting for %s, having printed %r' % (what, self.printed()))

    def keyboard(self, what):
        """Run until the teletype's keyboard is asked for a line."""
        self.until(what, lambda at, device, function: at == DOIO and device == TELETYPE and function in (0x9, 0xb))

    def type(self, line):
        # A record opens on a line feed and whatever is ahead of it is
        # passed over, so the blank costs nothing and takes the place of a
        # character read ahead of the request.
        self.emu.key(' \n' + line + '\r')

    def tape(self, name, frames):
        path = os.path.join(self.workdir, name)
        with open(path, 'wb') as f:
            f.write(frames)
        self.emu.tape(0, path)

    def load(self, name):
        """In the loader: thread a tape and run to the program it loads."""
        with open(os.path.join(TAPE_DIR, name), 'rb') as f:
            self.tape(name, f.read())
        self.until('the loader', lambda at, device, function: at == START)

    def compile_and_run(self, title, source, data=None):
        """From the loader, round to the loader again: what the program
        printed."""
        self.load(COMPILER)
        self.tape(title + '.for', source_tape(source))
        self.keyboard('the compiler')
        self.printed()
        self.type('G')
        self.keyboard('the compiler, the source read')
        said = self.printed().replace('G', '', 1).strip()
        if said:
            # an error is two letters and the number of the line
            raise Failed('%s: the compiler said %r' % (title, said))
        self.type('E')
        self.until('the loader', lambda at, device, function: at == LOADER)
        self.load(RUNTIME)
        self.keyboard('the library')
        if data is not None:
            self.tape(title + '.dat', data)
        self.printed()
        self.type('G')
        self.until('the loader', lambda at, device, function: at == LOADER)
        text = self.printed()
        self.log.write(('--- %s ---\n%s\n' % (title, text.replace('\r', ''))).encode())
        return text


def numbers(text):
    """The lines a program printed, each as its numbers; the library's own
    lines -- the echo of the command, STOP -- have none."""
    rows = []
    for line in re.split(r'[\r\n]+', text):
        fields = line.split()
        if fields and all(re.fullmatch(r'-?\d+(\.\d*)?', field) for field in fields):
            rows.append([float(field) if '.' in field else int(field) for field in fields])
    return rows


def run(workdir, log):
    with open(os.path.join(TAPE_DIR, PTIOS), 'rb') as f:
        origin, image, _ = tape703.read_tape(f.read())
    core = os.path.join(workdir, 'ptios.core')
    with open(core, 'wb') as f:
        f.write(bytes(origin * 2) + image)

    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle', '--fast-io', '--halt'], log=log)
    try:
        session = Session(emu, workdir, log)
        for addr in (DOIO, LOADER, START):
            emu.add_break(addr)
        emu.set_reg('PC', origin)
        for assignment in ASSIGN:
            session.keyboard('PTIOS')
            session.type(assignment)
        session.keyboard('PTIOS')
        said = session.printed()
        if 'ASSIGN I-O' not in said or said.count('OK') != len(ASSIGN):
            raise Failed('PTIOS said %r to %s' % (said, ', '.join(ASSIGN)))
        session.type('AL')
        session.until('the loader', lambda at, device, function: at == LOADER)

        table = TABLE
        if CONTROL == 'source':
            table = table.replace('J=K*K-7', 'J=K*(K-7')
        elif CONTROL == 'answer':
            table = table.replace('J=K*K-7', 'J=K*K-6')
        rows = numbers(session.compile_and_run('table', table))
        want = [[k, k * k - 7, math.sqrt(k) + k * k / 3.0] for k in range(1, ROWS + 1)]
        if len(rows) != ROWS or any(len(row) != 3 for row in rows):
            raise Failed('the table is %r' % rows)
        for row, (k, j, y) in zip(rows, want):
            if row[:2] != [k, j] or abs(row[2] - y) > 1.5e-4:
                raise Failed('the table has %r where it should have %d %d %.4f' % (row, k, j, y))

        punched = os.path.join(workdir, 'punched.bin')
        emu.save_tape(punched)
        with open(punched, 'rb') as f:
            frames = f.read()
        # a record on the tape is an X'05', a line feed, the line and a
        # carriage return
        if numbers(bytes(c & 0x7f for c in frames).decode('ascii', 'replace').replace('\x05', '')) \
                != [row[:2] for row in want]:
            raise Failed('the punch punched %r' % frames)
        if CONTROL == 'tape':
            # the units digit of the second record's K
            at = [i for i, c in enumerate(frames) if c == 0xb2][0]
            frames = frames[:at] + b'\xb3' + frames[at + 1:]

        rows = numbers(session.compile_and_run('sum', SUM, data=frames))
        total = sum(k * j for k, j, _ in want)
        if rows != [[total]]:
            raise Failed('the sum of the tape read back is %r, not %d' % (rows, total))
    finally:
        if emu.child.poll() is None:
            emu.child.kill()
        emu.child.wait()
        emu.child.stdin.close()
        emu.close()
        if emu.socket_dir:
            shutil.rmtree(emu.socket_dir, ignore_errors=True)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if CONTROL and CONTROL not in CONTROLS:
        print('error: FORTRAN_CONTROL is one of %s' % ', '.join(CONTROLS), file=sys.stderr)
        return 1
    missing = [name for name in (PTIOS, COMPILER, RUNTIME) if not os.path.isfile(os.path.join(TAPE_DIR, name))]
    if missing:
        print('FAIL: ray703 fortran test: no %s in %s' % (', '.join(missing), TAPE_DIR), file=sys.stderr)
        print('fetch them with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    # The emulator runs in a directory of its own: from the root the 703
    # would mount whatever platters are in disks/, write-through.
    workdir = tempfile.mkdtemp(prefix='ray703-fortran-')
    try:
        os.chdir(workdir)
        with open(LOG_FILE, 'wb') as log:
            try:
                run(workdir, log)
            except Exception as e:
                log.write(('\n--- failure ---\n' + traceback.format_exc()).encode())
                print('FAIL: ray703 fortran test: %s (see %s)'
                      % (e if isinstance(e, Failed) else repr(e), LOG_FILE), file=sys.stderr)
                return 1
    finally:
        os.chdir(HERE)
        shutil.rmtree(workdir, ignore_errors=True)
    print('PASS: ray703 fortran test, two programs through the reader and the punch (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
