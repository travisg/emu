#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the RC2014: boot the factory rom into Microsoft BASIC,
type a program at it, RUN it and check what it printed. The console goes
through the SIO both ways, so the debug port's output tap sees everything
the guest prints, and every character typed arrives as an SIO interrupt
into the rom's mode-1 handler at $0038 -- there is no other way in.

That handler is also what makes this the Z80 core's interrupt test. It ends
`EI; RETI`, which is only sound because a Z80 never interrupts the
instruction after an `EI`; a core without that shadow takes the still-
asserted line at the `RETI` and nests the handler once per character of a
burst. So the harness breaks on the handler, and for every character it
typed checks that the entry's return address is not that `RETI`.

The emulator is driven over its debug port (tools/emudbg.py): it starts
halted so no output is missed, keystrokes go in as `key` commands, and the
output is read from the port's copy of the serial stream. Every wait raises
on its timeout and the one handler at the bottom turns that into a FAIL
with the transcript in the log, so there is no path to a PASS that did not
see each thing it asserts. The negative controls: a rom that is not the
factory image fails at the byte check of the handler before anything is
typed, and a core that accepts an interrupt right after `EI` fails the
return-address check on the second character of the first line.

Needs `roms/rc2014/24886009.BIN` (tools/fetch-roms.py), so it runs locally
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
ROM_FILE = os.environ.get('ROM_FILE', os.path.join(ROOT, 'roms', 'rc2014', '24886009.BIN'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'rc2014_basic_test.log')

# The factory rom's SIO receive handler and the `EI; RETI` it ends with.
HANDLER = 0x0038
HANDLER_EXIT = 0x00b0
HANDLER_EXIT_BYTES = bytes([0xfb, 0xed, 0x4d])
RETI = HANDLER_EXIT + 1

# No FOR: this rom's BASIC enters its break check from FOR with A=1, which
# the SIO/2 rom's RST 18 takes as "check channel B", and channel B's buffer
# count cell is a byte of BASIC's own workspace that reads as a waiting
# character -- so the read that follows blocks on channel A until a key is
# pressed. Real hardware running this image does the same.
PROGRAM = [
    '10 I=0',
    '20 I=I+1',
    '30 PRINT "RC2014 LINE";I',
    '40 IF I<3 THEN GOTO 20',
    '50 PRINT "RC2014 BASIC TEST PASS"',
]
# The typed lines echo, so the PASS text alone would match line 40's echo;
# the run is the only thing that prints the three LINEs in front of it.
EXPECT = re.compile(rb'RC2014 LINE 1\s+RC2014 LINE 2\s+RC2014 LINE 3\s+RC2014 BASIC TEST PASS')


def type_line(emu, text):
    """Type `text` and Return as one burst, and see each character's
    interrupt through the breakpoint on the handler: the entry must not
    have come from the handler's own `RETI`. Then wait for the line to
    echo. The SIO here has no baud rate: the latch refills at the poll
    after the handler's `RETI`, so a burst reaches the rom's 64-byte ring
    buffer with BASIC getting one instruction per character to drain it,
    and a second burst on top of the first overflows it and is dropped.
    The echo is BASIC having consumed the line."""
    keys = text + '\r'
    emu.key(keys)
    for n, ch in enumerate(keys):
        stop = emu.wait_stopped()
        if stop.reason != 'break' or stop.pc != HANDLER:
            raise AssertionError('expected the SIO handler for %r, got %r' % (ch, stop))
        sp = emu.reg('SP')
        lo, hi = emu.mem(sp, 2)
        ret = lo | (hi << 8)
        if ret == RETI:
            raise AssertionError(
                'character %d of %r: the handler was entered from its own RETI '
                '(the interrupt was accepted right after EI)' % (n, text))
        emu.run()
    emu.wait_for_output(text.encode('latin-1') + b'\r\n')


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if not os.path.isfile(ROM_FILE):
        print('error: rom image not found at %s' % ROM_FILE, file=sys.stderr)
        print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    emu = None
    with open(LOG_FILE, 'wb') as log:
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'rc2014', '-r', ROM_FILE, '--no-throttle'], log=log)
            # The return-address check assumes the factory handler; a rom
            # with something else there must not pass it vacuously.
            found = emu.mem(HANDLER_EXIT, len(HANDLER_EXIT_BYTES))
            if found != HANDLER_EXIT_BYTES:
                raise AssertionError('no `EI; RETI` at %#06x: %s is not the factory rom'
                                     % (HANDLER_EXIT, ROM_FILE))
            emu.add_break(HANDLER)
            emu.run()
            emu.wait_for_output(b'Memory top? ')
            type_line(emu, '')          # the default
            emu.wait_for_output(b'Bytes free\r\nOk\r\n')
            for line in PROGRAM:
                type_line(emu, line)
            type_line(emu, 'RUN')
            emu.wait_for_output(EXPECT)
            emu.wait_for_output(b'TEST PASS\r\nOk\r\n')   # the program ran to its end
            emu.remove_break(HANDLER)
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
            print('FAIL: rc2014 basic test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: rc2014 basic test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
