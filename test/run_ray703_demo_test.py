#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the Raytheon 703: boot the demo image, type a line at
it, and check that the machine echoed the line back folded to upper case and
then halted. That exercises the whole stack -- the core, the interrupt
system, the DIO channel, the teletype device and the debug port -- because
the demo's echo runs entirely out of a level 0 interrupt service routine.

The emulator is driven over its debug port (tools/emudbg.py): it starts
halted so no output is missed, the keystrokes go in as `key` commands, the
echo is read from the port's copy of the serial output, and the halt is the
port's own stop event rather than a line grepped out of a log. No pty, no
fifo, no `script(1)`.

Every wait raises on its timeout and the one handler at the bottom turns
that into a FAIL with the transcript in the log, so there is no path to a
PASS that did not see each thing it asserts. The negative controls that
prove it: point ROM_FILE at basic.bin (the banner never comes) or at a demo
whose halt has been patched out (the echo passes, the stop never comes).
"""

import os
import sys
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
ROM_FILE = os.environ.get('ROM_FILE', os.path.join(ROOT, 'roms', '703', 'demo.bin'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_demo_test.log')

# Typed in lower case; the demo folds it, so finding the upper case version
# in the output proves the guest produced it -- nothing else echoes.
PHRASE = 'ray703 echo test pass'
EXPECT = b'RAY703 ECHO TEST PASS'


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if not os.path.isfile(ROM_FILE):
        print('error: demo image not found at %s' % ROM_FILE, file=sys.stderr)
        print('build it with: make -C test ray703-demo', file=sys.stderr)
        return 1

    emu = None
    with open(LOG_FILE, 'wb') as log:
        try:
            # --no-throttle: a machine runs at its own clock rate unless told
            # otherwise, and the harness wants the answer rather than the
            # period. The image is named absolutely so this works from any
            # directory -- `make -C test ray703-test` does not run at the root.
            emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--no-throttle'], log=log)
            emu.run()
            emu.wait_for_output(b'RAYTHEON 703 READY')
            emu.key(PHRASE + '\r')
            emu.wait_for_output(EXPECT)
            emu.key('.')            # the demo halts on a period
            stop = emu.wait_stopped()
            if stop.reason != 'hlt':
                raise AssertionError('expected the guest to halt, got %r' % (stop,))
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
                if emu.child is not None and emu.child.poll() is None:
                    emu.child.kill()
                    emu.child.wait()
                    emu.child.stdin.close()
                emu.close()
            print('FAIL: ray703 demo test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: ray703 demo test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
