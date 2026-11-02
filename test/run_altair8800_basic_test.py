#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the Altair 8800: load 8K BASIC 4.0 the way it was
loaded in 1976 and run a program on it.

`-s altair8800` keys MITS's 2SIO bootstrap into the bottom of memory and
feeds the paper tape through the console port; the bootstrap loads the
tape's checksum loader, which loads BASIC record by record, checking each
record's checksum, and starts it. So by the time `MEMORY SIZE?` appears
the 8080 core has run two of MITS's loaders over 7.7K of tape without a
checksum error, and the rest is BASIC itself: the initialisation dialog,
then a program that checks its own answers -- loops, GOSUB, strings,
floating point and the SIN/SQR the dialog was asked to keep -- and prints
one verdict.

Driven over the debug port, started halted so no output is missed. Each
line is typed in one burst and paced on its echo; the verdict is looked
for only after the RUN's echo, since typing the program echoes the PASS
text too. Every wait raises on its timeout and the handler at the bottom
makes that a FAIL with the transcript in the log. Negative controls, both
of which must FAIL: ALTAIR8800_BASIC_CONTROL=wrong makes one of the
program's checks wrong (it prints its FAIL line), and =tape loads a copy
of the tape with one byte of BASIC changed (the checksum loader stops with
its C error before BASIC starts).

Needs `roms/altair8800/basic8k40.tap` (tools/fetch-roms.py), so it runs
locally rather than in CI.
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

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
TAPE_FILE = os.environ.get('TAPE_FILE', os.path.join(ROOT, 'roms', 'altair8800', 'basic8k40.tap'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'altair8800_basic_test.log')
CONTROL = os.environ.get('ALTAIR8800_BASIC_CONTROL', '')

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
# Where the control's changed byte goes: well into BASIC's records, past
# the leader and the checksum loader.
CORRUPT_AT = 0x1000


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

    emu = None
    with tempfile.TemporaryDirectory(prefix='altair8800-basic-') as workdir, open(LOG_FILE, 'wb') as log:
        tape = TAPE_FILE
        if CONTROL == 'tape':
            data = bytearray(open(TAPE_FILE, 'rb').read())
            data[CORRUPT_AT] ^= 0x01
            tape = os.path.join(workdir, 'corrupt.tap')
            with open(tape, 'wb') as f:
                f.write(data)
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'altair8800', '-r', tape, '--no-throttle'], log=log)
            emu.run()
            at = emu.wait_for_output(b'MEMORY SIZE? ', timeout=30).end()
            at = type_line(emu, '', at)
            at = emu.wait_for_output(b'TERMINAL WIDTH? ', start=at).end()
            at = type_line(emu, '', at)
            at = emu.wait_for_output(b'WANT SIN-COS-TAN-ATN? ', start=at).end()
            at = type_line(emu, 'Y', at)
            at = emu.wait_for_output(
                re.compile(rb'\d+ BYTES FREE\r\nALTAIR BASIC REV\. 4\.0\r\n\[EIGHT-K VERSION\]\r\n'
                           rb'COPYRIGHT 1976 BY MITS INC\.\r\nOK\r\n'), start=at).end()
            for line in PROGRAM:
                at = type_line(emu, line, at)
            at = type_line(emu, 'RUN', at)
            verdict = emu.wait_for_output(re.compile(rb'ALTAIR BASIC TEST (PASS|FAIL)[^\r]*\r\nOK\r\n'), start=at)
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
            print('FAIL: altair8800 basic test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: altair8800 basic test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
