#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for the RC2014's CP/M build: boot the factory rom's
monitor, let it page itself into RAM, have it load CP/M off the compact
flash card, and use the disk both ways -- list two drives, save a file,
and check that the save reached the image.

The card is a scratch copy of disks/rc2014-cf.img, since writes go through
to it. The emulator is driven over its debug port (tools/emudbg.py): it
starts halted so no output is missed, keystrokes go in as `key` commands,
and the output is read from the port's copy of the serial stream. Every
line is typed only after its prompt has appeared, because CP/M's console
output takes a waiting character as a possible Ctrl-S. Every wait raises
on its timeout and the one handler at the bottom turns that into a FAIL
with the transcript in the log, so there is no path to a PASS that did not
see each thing it asserts. The negative controls: a card that is not this
one (`CF_IMAGE` at a blank file) never reaches the `A>` prompt, and a card
that dropped writes fails the check of the image afterwards.

Needs `roms/rc2014/24886009.BIN` and `disks/rc2014-cf.img`
(tools/fetch-roms.py), so it runs locally rather than in CI.
"""

import os
import shutil
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
ROM_FILE = os.environ.get('ROM_FILE', os.path.join(ROOT, 'roms', 'rc2014', '24886009.BIN'))
CF_IMAGE = os.environ.get('CF_IMAGE', os.path.join(ROOT, 'disks', 'rc2014-cf.img'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'rc2014_cpm_test.log')

# The file SAVE makes on A:, and the directory entry it leaves on the card:
# user 0, then the name and type padded to 8 and 3.
SAVED = 'EMUTEST.TXT'
SAVED_ENTRY = b'\x00EMUTEST TXT'


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    for path, what in ((ROM_FILE, 'rom image'), (CF_IMAGE, 'compact flash image')):
        if not os.path.isfile(path):
            print('error: %s not found at %s' % (what, path), file=sys.stderr)
            print('fetch it with: tools/fetch-roms.py', file=sys.stderr)
            return 1

    emu = None
    with tempfile.TemporaryDirectory(prefix='rc2014-cpm-') as workdir, open(LOG_FILE, 'wb') as log:
        card = os.path.join(workdir, 'card.img')
        shutil.copyfile(CF_IMAGE, card)
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'rc2014-cpm', '-r', ROM_FILE, '--disk', card, '--no-throttle'], log=log)
            emu.run()
            # The same prompt comes round again and again, so every wait
            # starts where the last one ended.
            pos = 0

            def expect(text):
                nonlocal pos
                pos = emu.wait_for_output(text, start=pos).end()

            expect(b'Press [SPACE] to activate console')
            emu.key(' ')
            expect(b'Z80 SBC Boot ROM 1.1 by G. Searle')
            expect(b'\r\n>')
            emu.key('X')
            expect(b'Boot CP/M?')
            emu.key('Y')
            expect(b'Loading CP/M...')
            expect(b'CP/M 2.2 Copyright 1979 (c) by Digital Research')
            expect(b'\r\nA>')
            emu.key('DIR\r')
            expect(b'A: DOWNLOAD COM')
            expect(b'\r\nA>')
            emu.key('C:\r')
            expect(b'\r\nC>')
            emu.key('DIR\r')
            expect(b'STAT     COM')
            expect(b'\r\nC>')
            emu.key('A:\r')
            expect(b'\r\nA>')
            # a page of the TPA to a file: a directory entry and a block
            emu.key('SAVE 1 %s\r' % SAVED)
            expect(b'\r\nA>')
            emu.key('DIR\r')
            expect(b'EMUTEST  TXT')
            expect(b'\r\nA>')
            # another drive's directory, so the BIOS writes back whatever
            # it was still holding for A:
            emu.key('C:\r')
            expect(b'\r\nC>')
            emu.key('DIR\r')
            expect(b'STAT     COM')
            expect(b'\r\nC>')
            emu.kill()
            emu.wait_exited()
            status = emu.wait_exit()
            if status != 0:
                raise AssertionError('the emulator exited with status %d' % status)
            with open(card, 'rb') as f:
                # drive A's directory: one reserved 16K track, then 512
                # entries of 32 bytes
                directory = f.read(16 * 1024 + 512 * 32)[16 * 1024:]
            if SAVED_ENTRY not in directory:
                raise AssertionError('%s is not in the directory on the card' % SAVED)
            with open(CF_IMAGE, 'rb') as f:
                if SAVED_ENTRY in f.read(16 * 1024 + 512 * 32)[16 * 1024:]:
                    raise AssertionError('the pristine image already has %s: the check is vacuous' % SAVED)
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
            print('FAIL: rc2014 cp/m test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: rc2014 cp/m test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
