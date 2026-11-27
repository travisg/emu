#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the Altair 8800's floppies: boot Burcon CP/M 2.2
off the 88-DCDD through MITS's disk boot loader PROM, and use two drives
-- list A:, run MBASIC off it and a line of BASIC in it, change to B:,
save a file there, and check on the host that the save reached B's image
and nowhere else.

`altair8800-disk` keys `JMP 0FF00H` in at 0 and runs it: the DBL copies
itself to 2C00, seeks drive 0 to track 0 and loads the boot sectors,
which load CP/M. Every byte crosses the controller at the disk's own 32 us
a byte, so this is the 8080 core, the controller's timing and the BIOS's
two-bytes-a-pass loops together, read and write.

Both disks are scratch copies of disks/altair8800-cpm.dsk, the second on
`--disk` again for drive B, since writes go through to them. Driven over
the debug port, started halted so no output is missed; every line is typed
only after its prompt has appeared. Every wait raises on its timeout and
the handler at the bottom makes that a FAIL with the transcript in the
log. Negative controls, which must FAIL: ALTAIR8800_CPM_CONTROL=blank puts
a blank disk in drive 0, which the loader never boots (no `A>`), and
=readonly makes B's copy read-only, so the save never reaches its image.

Needs `roms/altair8800/dbl.hex` and `disks/altair8800-cpm.dsk`
(tools/fetch-roms.py), so it runs locally rather than in CI.
"""

import os
import shutil
import stat
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
PROM_FILE = os.path.join(ROOT, 'roms', 'altair8800', 'dbl.hex')
DISK_IMAGE = os.path.abspath(os.environ.get('DISK_IMAGE', os.path.join(ROOT, 'disks', 'altair8800-cpm.dsk')))
LOG_FILE = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'altair8800_cpm_test.log'))
CONTROL = os.environ.get('ALTAIR8800_CPM_CONTROL', '')

# The file SAVE makes on B:, and the directory entry it leaves on the
# disk: user 0, then the name and type padded to 8 and 3.
SAVED = 'EMUTEST.TXT'
SAVED_ENTRY = b'\x00EMUTEST TXT'


def command(emu, line, prompt, at, timeout=60):
    """Type `line` and Return at a prompt, and wait for the next `prompt`
    after its echo; returns where that prompt ended."""
    emu.key(line + '\r')
    at = emu.wait_for_output(line.encode() + b'\r\n', start=at, timeout=timeout).end()
    return emu.wait_for_output(prompt, start=at, timeout=timeout).end()


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    for path, what in ((PROM_FILE, 'boot loader PROM'), (DISK_IMAGE, 'CP/M disk image')):
        if not os.path.isfile(path):
            print('error: %s not found at %s' % (what, path), file=sys.stderr)
            print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
            return 1

    # the PROM is the machine's default -r, relative to the top of the tree
    os.chdir(ROOT)
    emu = None
    with tempfile.TemporaryDirectory(prefix='altair8800-cpm-') as workdir, open(LOG_FILE, 'wb') as log:
        disk_a = os.path.join(workdir, 'a.dsk')
        disk_b = os.path.join(workdir, 'b.dsk')
        shutil.copyfile(DISK_IMAGE, disk_b)
        if CONTROL == 'blank':
            with open(disk_a, 'wb') as f:
                f.write(bytes(os.path.getsize(DISK_IMAGE)))
        else:
            shutil.copyfile(DISK_IMAGE, disk_a)
        if CONTROL == 'readonly':
            os.chmod(disk_b, stat.S_IRUSR)
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'altair8800-disk', '--disk', disk_a, '--disk', disk_b, '--no-throttle'],
                            log=log)
            emu.run()
            at = emu.wait_for_output(b'56K CP/M\r\n', timeout=30).end()
            at = emu.wait_for_output(b'A>', start=at, timeout=30).end()
            at = command(emu, 'DIR', b'A>', at)
            listing = bytes(emu.output[:at])
            if b'PIP      COM' not in listing or b'MBASIC   COM' not in listing:
                raise AssertionError('A: lists no PIP or MBASIC')
            # a 24K program off the disk, and BASIC in it
            at = command(emu, 'MBASIC', b'Ok\r\n', at)
            at = command(emu, 'PRINT 6*7', b'Ok\r\n', at)
            if b' 42 \r\n' not in bytes(emu.output[at - 20:at]):
                raise AssertionError('MBASIC did not print 42')
            at = command(emu, 'SYSTEM', b'A>', at)
            at = command(emu, 'B:', b'B>', at)
            at = command(emu, 'SAVE 2 %s' % SAVED, b'B>', at)
            at = command(emu, 'DIR %s' % SAVED, b'B>', at)
            if b'B: EMUTEST  TXT' not in bytes(emu.output[at - 40:at]):
                raise AssertionError('B: does not list %s' % SAVED)
            emu.kill()
            emu.wait_exited()
            status = emu.wait_exit()
            if status != 0:
                raise AssertionError('the emulator exited with status %d' % status)
            with open(disk_b, 'rb') as f:
                b = f.read()
            with open(disk_a, 'rb') as f:
                a = f.read()
            with open(DISK_IMAGE, 'rb') as f:
                pristine = f.read()
            if SAVED_ENTRY in pristine:
                raise AssertionError('the pristine disk already has %s' % SAVED)
            if SAVED_ENTRY not in b:
                raise AssertionError("%s's directory entry is not on B's image" % SAVED)
            if SAVED_ENTRY in a:
                raise AssertionError('%s reached A, not only B' % SAVED)
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
            print('FAIL: altair8800 cpm test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: altair8800 cpm test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
