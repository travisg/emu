#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Run Raytheon's 700-series math library on the 703 and hold it to the host's
arithmetic.

The tape is DN 392325 as the University of Stuttgart's computer museum read
it (tools/fetch-roms.py puts it in roms/703/tapes): twenty-seven modules of
relocatable text, the mid-precision floating point package and the functions
over it. tools/reload703.py places them all from word X'800' and fills the
references between them; the image goes into a 703's core, and each check
is a three-call program at word X'100' --

            SMB     FLD         the argument into the pseudo-accumulator
            JSX     FLD
            DATA    X
            SMB     f           the routine, its operand's address inline
            JSX     f
            DATA    Y
            SMB     FST         and the result out
            JSX     FST
            DATA    R
            HLT

-- which is the calling sequence of the library's manual (Math Library, July
1969, p. 6). A number is three words: the exponent, excess X'80', and a
thirty-bit two's complement fraction, fifteen bits to a word (p. 7). The
result must be the host's to a part in ten million, some thirty times the
library's worst error over these arguments.

Checked: FAD FSB FMP FDV, MSQR MSIN MCOS MATN MEXP MLOG and MTNH. This is
the build with the multiply and divide in software, so the 703's MPY and DIV
are not in it. Two routines are run other than as the tape has them, both
because the tape's modules are not all of one revision:

  * MLOG calls FLT with its argument's bare address, and the FLT on the tape
    takes a bare address for another form of call and returns a word late.
    The address is given the X'8000' that this FLT's one-argument form
    wants; without it MLOG is wrong for every argument.
  * MTNH reaches into MEXP at offsets that are not this tape's MEXP's, and
    is wrong below one. It is checked from one up, where it does not use
    them.

Negative controls, each of which must FAIL: MATH_CONTROL=unpatched (MLOG as
the tape has it) and =swapped (FAD called where FSB is meant).
"""

import math
import os
import shutil
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402
import reload703  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
TAPE_DIR = os.path.abspath(os.environ.get('TAPE_DIR', os.path.join(ROOT, 'roms', '703', 'tapes')))
LOG_FILE = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_math_test.log')
CONTROL = os.environ.get('MATH_CONTROL', '')
CONTROLS = ('unpatched', 'swapped')
TAPE = '392325.bin'

LIBRARY = 0x800                     # where the modules go
PROGRAM, X, Y, R = 0x100, 0x200, 0x204, 0x208
TOLERANCE = 1e-7

UNARY = {
    'MSQR': (math.sqrt, [0.25, 0.5, 1.0, 2.0, 3.0, 10.0, 12345.678, 1e-6, 1e10]),
    'MLOG': (math.log, [0.25, 0.5, 1.5, 2.0, 3.0, 10.0, 12345.678, 1e-6, 1e10]),
    'MSIN': (math.sin, [-3.0, -1.0, -0.5, 0.0, 0.1, 0.5, 1.0, 1.5, 3.0, 6.0]),
    'MCOS': (math.cos, [-3.0, -1.0, -0.5, 0.0, 0.1, 0.5, 1.0, 1.5, 3.0, 6.0]),
    'MATN': (math.atan, [-3.0, -1.0, -0.5, 0.0, 0.1, 0.5, 1.0, 1.5, 3.0, 6.0]),
    'MEXP': (math.exp, [-3.0, -1.0, -0.5, 0.0, 0.1, 0.5, 1.0, 1.5, 3.0, 6.0]),
    'MTNH': (math.tanh, [-3.0, -1.5, -1.0, 1.0, 1.5, 3.0, 6.0]),
}
BINARY = {
    'FAD': lambda x, y: x + y,
    'FSB': lambda x, y: x - y,
    'FMP': lambda x, y: x * y,
    'FDV': lambda x, y: x / y,
}
PAIRS = [(1.5, 2.25), (-7.0, 3.0), (1e6, 1e-3), (123.456, -0.001), (3.0, 3.0), (0.1, 0.2)]


class Failed(Exception):
    pass


def encode(x):
    """A host float as the library's three words."""
    if x == 0:
        return [0, 0, 0]
    fraction, exponent = math.frexp(x)      # x = fraction * 2**exponent, 0.5 <= |fraction| < 1
    q = round(fraction * (1 << 30))
    if q == 1 << 30:
        q >>= 1
        exponent += 1
    q &= (1 << 31) - 1
    return [(exponent + 0x80) & 0xff, (q >> 15) & 0xffff, q & 0x7fff]


def decode(words):
    if not (words[1] or words[2]):
        return 0.0
    q = ((words[1] - (0x10000 if words[1] & 0x8000 else 0)) << 15) + (words[2] & 0x7fff)
    return q / (1 << 30) * 2.0 ** ((words[0] & 0xff) - 0x80)


def call(emu, symbols, routine, x, y):
    """FLD X, routine Y, FST R on the machine, and R as a host float."""
    program = []
    for name, operand in (('FLD', X), (routine, Y), ('FST', R)):
        addr = symbols[name]
        program += [0x80 | (addr >> 10), 0x2000 | (addr & 0x7ff), operand]
    emu.writew(PROGRAM, program + [0x0000])
    emu.writew(X, encode(x))
    emu.writew(Y, encode(y))
    emu.writew(R, [0xdead, 0xdead, 0xdead])
    emu.set_reg('PC', PROGRAM)
    emu.set_reg('OVF', 0)
    emu.run()
    ev = emu.wait_stopped(timeout=30)
    end = PROGRAM + len(program) + 1
    if ev.reason != 'hlt' or ev.pc != end:
        raise Failed('%s stopped (%s) at %04X in front of `%s`, not at the program\'s end'
                     % (routine, ev.reason, ev.pc, ev.insn))
    return decode(emu.memw(R, 3))


def run(image, symbols, log):
    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', image, '--no-throttle', '--halt'], log=log)
    try:
        if CONTROL != 'unpatched':
            # MLOG's `JSX FLT / DATA TMP6`, 2B words past its entry
            cell = symbols['MLOG'] + 0x2c
            if emu.memw(cell - 1, 2) != [0x2000 | (symbols['FLT'] & 0x7ff), symbols['TMP6']]:
                raise Failed('MLOG does not call FLT where this patches it')
            emu.writew(cell, [symbols['TMP6'] | 0x8000])
        if CONTROL == 'swapped':
            symbols = dict(symbols, FSB=symbols['FAD'])

        wrong = []
        worst = {}

        def check(routine, what, got, want):
            err = abs(got - want) / max(abs(want), 1.0)
            worst[routine] = max(worst.get(routine, 0.0), err)
            if err > TOLERANCE:
                wrong.append('%s%s is %.10g, not %.10g' % (routine, what, got, want))

        for routine, (function, arguments) in UNARY.items():
            for x in arguments:
                check(routine, '(%g)' % x, call(emu, symbols, routine, x, x), function(x))
        for routine, function in BINARY.items():
            for x, y in PAIRS:
                check(routine, '(%g, %g)' % (x, y), call(emu, symbols, routine, x, y), function(x, y))
        log.write(('worst error: %s\n' % ', '.join('%s %.1e' % kv for kv in worst.items())).encode())
        if wrong:
            raise Failed('%d wrong, the first: %s' % (len(wrong), wrong[0]))
        return len(UNARY) + len(BINARY)
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
        print('error: MATH_CONTROL is one of %s' % ', '.join(CONTROLS), file=sys.stderr)
        return 1
    tape = os.path.join(TAPE_DIR, TAPE)
    if not os.path.isfile(tape):
        print('FAIL: ray703 math test: no %s in %s' % (TAPE, TAPE_DIR), file=sys.stderr)
        print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    # The emulator runs in a directory of its own: from the root the 703
    # would mount whatever platters are in disks/, write-through.
    workdir = tempfile.mkdtemp(prefix='ray703-math-')
    try:
        os.chdir(workdir)
        with open(LOG_FILE, 'wb') as log:
            try:
                with open(tape, 'rb') as f:
                    core, symbols, modules = reload703.library(reload703.unframe(f.read()), LIBRARY)
                log.write(('%d modules, %d words from %04X\n' % (len(modules), len(core), LIBRARY)).encode())
                image = bytearray(2 * (max(core) + 1))
                for addr, word in core.items():
                    image[2 * addr:2 * addr + 2] = word.to_bytes(2, 'big')
                path = os.path.join(workdir, 'math.core')
                with open(path, 'wb') as f:
                    f.write(image)
                routines = run(path, symbols, log)
            except Exception as e:
                log.write(('\n--- failure ---\n' + traceback.format_exc()).encode())
                print('FAIL: ray703 math test: %s (see %s)' % (e if isinstance(e, Failed) else repr(e), LOG_FILE),
                      file=sys.stderr)
                return 1
    finally:
        os.chdir(HERE)
        shutil.rmtree(workdir, ignore_errors=True)
    print('PASS: ray703 math test, %d routines of %d modules (%s)' % (routines, len(modules), EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
