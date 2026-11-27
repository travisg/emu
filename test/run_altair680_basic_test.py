#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the Altair 680b: load Altair 680 BASIC off its
paper tape through the prom monitor and run a program on it.

`-s altair680-basic` stands in for the operator: `L` at the monitor's
prompt, the tape through the terminal's line, `J0000` once it is in. The
monitor's own loader reads the tape's S1 records and checks each one's
checksum, so by the time `MEMORY SIZE?` appears the 6800 core has run the
1976 monitor over 18K of tape and jumped into what it loaded. The rest is
BASIC itself: the initialisation dialog, then a program that checks its
own answers -- loops, GOSUB, strings, floating point and the SIN/SQR the
dialog was asked to keep -- and prints one verdict.

Driven over the debug port, started halted so no output is missed. Each
line is paced on its echo; the verdict is looked for only after the RUN's
echo, since typing the program echoes the PASS text too. Every wait
raises on its timeout and the handler at the bottom makes that a FAIL
with the transcript in the log. Negative controls, both of which must
FAIL: ALTAIR680_BASIC_CONTROL=wrong makes one of the program's checks
wrong (it prints its FAIL line), and =tape loads a copy of the tape with
one hex digit of an early record changed, which the monitor's checksum
stops on, so BASIC never loads whole and never asks its first question.

Needs `roms/mits680b/mits680b.bin` and `roms/mits680b/basic680.s19`
(tools/fetch-roms.py), so it runs locally rather than in CI.
"""

import os
import re
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
TAPE_FILE = os.path.abspath(os.environ.get('TAPE_FILE', os.path.join(ROOT, 'roms', 'mits680b', 'basic680.s19')))
LOG_FILE = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'altair680_basic_test.log'))
CONTROL = os.environ.get('ALTAIR680_BASIC_CONTROL', '')

PROGRAM = [
    '10 E=0:S=0',
    '20 FOR I=1 TO 10:S=S+I:NEXT I',
    '30 IF S<>%d THEN E=E+1' % (56 if CONTROL == 'wrong' else 55),
    '40 A$="ALTAIR":IF LEN(A$)<>6 OR LEFT$(A$,3)<>"ALT" THEN E=E+2',
    '50 GOSUB 100:IF R<>12 THEN E=E+4',
    '60 IF ABS(SIN(0))>1E-6 OR ABS(SQR(2)*SQR(2)-2)>1E-5 THEN E=E+8',
    '70 IF INT(7/2)<>3 OR 7/2<>3.5 THEN E=E+16',
    '80 IF E=0 THEN PRINT "ALTAIR BASIC TEST PASS":END',
    '90 PRINT "ALTAIR BASIC TEST FAIL";E:END',
    '100 R=3*4:RETURN',
]
# The control's changed record: the tenth line, well into BASIC's code.
CORRUPT_LINE = 10


def corrupt(tape):
    """The tape with one hex digit of a record's data changed, its
    checksum left as it was."""
    lines = tape.split(b'\n')
    line = bytearray(lines[CORRUPT_LINE - 1])
    if not line.startswith(b'S1'):
        raise AssertionError('line %d of the tape is not a data record' % CORRUPT_LINE)
    # past S1, the count and the address: the first data byte's high digit
    line[8] = ord('1') if line[8] != ord('1') else ord('2')
    lines[CORRUPT_LINE - 1] = bytes(line)
    return b'\n'.join(lines)


def type_line(emu, text, start):
    """Type `text` and Return, and wait for BASIC to echo it; returns where
    the echo ended."""
    emu.key(text + '\r')
    return emu.wait_for_output(text.encode('latin-1') + b'\r\n', start=start).end()


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if not os.path.isfile(TAPE_FILE):
        print('error: tape image not found at %s' % TAPE_FILE, file=sys.stderr)
        print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    # the monitor is the machine's default rom, relative to the top of the
    # tree as for any run of the emulator; `make -C test` starts this in test/
    os.chdir(ROOT)
    emu = None
    with tempfile.TemporaryDirectory(prefix='altair680-basic-') as workdir, open(LOG_FILE, 'wb') as log:
        tape = TAPE_FILE
        if CONTROL == 'tape':
            tape = os.path.join(workdir, 'corrupt.s19')
            with open(TAPE_FILE, 'rb') as f, open(tape, 'wb') as out:
                out.write(corrupt(f.read()))
        system = 'altair680-basic'
        try:
            emu = Emu.spawn([EMU_BIN, '-s', system, '-r', tape, '--no-throttle'], log=log)
            emu.run()
            # the monitor's prompt and the load command, then the jump
            at = emu.wait_for_output(b'\r\n.L ', timeout=30).end()
            at = emu.wait_for_output(b'.J 0000\r\nMEMORY SIZE? ', start=at, timeout=30).end()
            at = type_line(emu, '', at)
            at = emu.wait_for_output(b'TERMINAL WIDTH? ', start=at).end()
            at = type_line(emu, '', at)
            at = emu.wait_for_output(b'WANT SIN-COS-TAN-ATN? ', start=at).end()
            at = type_line(emu, 'Y', at)
            at = emu.wait_for_output(
                re.compile(rb'\d+ BYTES FREE\r\n\r\nMITS ALTAIR 680 BASIC VERSION 1\.1 REV 3\.2\r\n'
                           rb'COPYRIGHT 1976 BY MITS INC\.\r\n\r\nOK\r\n'), start=at).end()
            for line in PROGRAM:
                at = type_line(emu, line, at)
            at = type_line(emu, 'RUN', at)
            verdict = emu.wait_for_output(re.compile(rb'ALTAIR BASIC TEST (PASS|FAIL)[^\r]*\r\n\r\nOK\r\n'), start=at)
            if verdict.group(1) != b'PASS':
                raise AssertionError('the program says %r' % verdict.group(0))
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
            print('FAIL: altair680 basic test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: altair680 basic test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
