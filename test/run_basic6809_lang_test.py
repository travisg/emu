#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the 6809: boot Grant Searle's build of Microsoft
Extended BASIC, type test/6809/lang_test.bas at it, RUN it, and require
the program's own verdict -- arithmetic, FOR, GOSUB, arrays, DATA, the
string functions and the floating-point library, each of which ENDs with
a `FAIL` line on a wrong answer, and the last of which prints `BASIC
LANGUAGE TEST PASS`.

The console is the MC6850, so the debug port's output tap sees everything
the guest prints. The emulator is driven over that port (tools/emudbg.py):
started halted so the banner is not missed, each line typed as `key` and
paced on its echo, the verdict read from the port's copy of the output.
Every wait raises on its timeout and the one handler at the bottom turns
that into a FAIL with the transcript in the log.

The program's last line prints the PASS text, so typing it echoes the PASS
text too: the verdict is looked for only after the RUN's own echo, and has
to be followed by BASIC's `OK`, which is the program having ended rather
than stopped on an error. Negative controls: a program with a line changed
to a wrong answer prints its FAIL and never the PASS, and one with a
syntax error stops on a `?SN ERROR`; both must FAIL.

Needs `roms/6809/BASIC.HEX` (tools/fetch-roms.py), so it runs locally
rather than in CI.
"""

import os
import re
import sys
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
ROM_FILE = os.environ.get('ROM_FILE', os.path.join(ROOT, 'roms', '6809', 'BASIC.HEX'))
PROGRAM_FILE = os.environ.get('PROGRAM_FILE', os.path.join(HERE, '6809', 'lang_test.bas'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'basic6809_lang_test.log')

# The verdict, then BASIC back at its prompt with nothing in between.
VERDICT = re.compile(rb'BASIC LANGUAGE TEST PASS\r\nOK\r\n')
# What a run that went wrong prints instead, looked for only after the
# RUN, so the FAILs in the program's echo do not count.
WRONG = re.compile(rb'FAIL \S+|\?\w\w ERROR')


def type_line(emu, text, start):
    """Type `text` and Return, and wait for BASIC to echo it, which is it
    having taken the line; returns where the output picks up after it. A
    character lost on the way fails the echo, not the run."""
    emu.key(text + '\r')
    return emu.wait_for_output(text.encode('latin-1') + b'\r\n', start=start).end()


def command(emu, text, start):
    """A direct command, paced on BASIC's `OK` as well as the echo: a key
    typed while it is still carrying the command out is read by its break
    check and lost, as on the machine."""
    at = type_line(emu, text, start)
    return emu.wait_for_output(b'OK\r\n', start=at).end()


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if not os.path.isfile(ROM_FILE):
        print('error: rom image not found at %s' % ROM_FILE, file=sys.stderr)
        print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
        return 1
    with open(PROGRAM_FILE) as f:
        program = [line.rstrip('\r\n') for line in f if line.strip()]

    emu = None
    with open(LOG_FILE, 'wb') as log:
        try:
            emu = Emu.spawn([EMU_BIN, '-s', '6809', '-r', ROM_FILE, '--no-throttle'], log=log)
            emu.run()
            at = emu.wait_for_output(b'BY MICROSOFT\r\n\r\nOK\r\n').end()
            at = command(emu, 'NEW', at)
            for line in program:
                at = type_line(emu, line, at)
            at = type_line(emu, 'RUN', at)
            try:
                emu.wait_for_output(VERDICT, start=at, timeout=60)
            except TimeoutError:
                wrong = WRONG.search(bytes(emu.output), at)
                if wrong:
                    raise AssertionError('the program stopped on %r' % wrong.group(0).strip())
                raise
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
            print('FAIL: 6809 BASIC language test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: 6809 BASIC language test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
