#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test of the 6800 core's IRQ and the Altair 680's ACIA, whose
IRQ output the 680b ties straight to the CPU's: a hand-assembled program,
written into RAM over the debug port, programs the ACIA for a receive
interrupt, clears I and waits in a branch to itself. The handler reads the
character, echoes it, counts it and returns with RTI.

What the harness holds the machine to, with a breakpoint on the handler,
for each character typed: the entry comes through the IRQ vector (nothing
else points at the handler); it was taken from the waiting branch, whose
address is the PC on the stack, with everything stacked under it and I
clear in the stacked CC and set in the live one; the count in memory keeps
up; and the echo comes back on the port.

The rom is a 256-byte monitor image made here, holding only the reset and
IRQ vectors, so this needs nothing outside the repo and CI runs it. Every
wait raises on its timeout, and the handler at the bottom makes that a
FAIL with the transcript in the log. Negative controls, both of which must
FAIL by never reaching the handler: ALTAIR_IRQ_CONTROL=masked sets I
instead of clearing it, and =rie-off programs the ACIA without its receive
interrupt.
"""

import os
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'altair680_irq_test.log')
CONTROL = os.environ.get('ALTAIR_IRQ_CONTROL', '')

PROGRAM = 0x0200
HANDLER = 0x0300
COUNTER = 0x0050
STACK = 0x7f00

ACIA_CONTROL = 0x15 if CONTROL == 'rie-off' else 0x95   # RIE, 8N1, divide by 16
MASK = 0x0f if CONTROL == 'masked' else 0x0e            # sei / cli

MAIN = bytes([
    0x86, 0x03,                 # ldaa #$03
    0xb7, 0xf0, 0x00,           # staa $f000      ; master reset
    0x86, ACIA_CONTROL,         # ldaa #$95
    0xb7, 0xf0, 0x00,           # staa $f000      ; receive interrupt on
    0x8e, STACK >> 8, 0x00,     # lds #$7f00
    MASK,                       # cli
    0x20, 0xfe,                 # bra *           ; 0x020e: wait
])
WAIT = PROGRAM + 0x0e
HANDLER_CODE = bytes([
    0xb6, 0xf0, 0x01,           # ldaa $f001      ; the character, which drops IRQ
    0xb7, 0xf0, 0x01,           # staa $f001      ; echoed
    0x7c, 0x00, COUNTER,        # inc $0050
    0x3b,                       # rti
])
TYPED = 'IRQ6800'


def monitor_rom():
    """The 680b's 256-byte prom at 0xff00, holding only the IRQ vector
    (0xfff8) and the reset vector (0xfffe)."""
    rom = bytearray(256)
    rom[0xf8:0xfa] = HANDLER.to_bytes(2, 'big')
    rom[0xfe:0x100] = PROGRAM.to_bytes(2, 'big')
    return bytes(rom)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1

    emu = None
    with tempfile.TemporaryDirectory(prefix='altair680-irq-') as workdir, open(LOG_FILE, 'wb') as log:
        rom = os.path.join(workdir, 'monitor.bin')
        with open(rom, 'wb') as f:
            f.write(monitor_rom())
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'altair680', '-r', rom, '--no-throttle'], log=log)
            if emu.reg('PC') != PROGRAM:
                raise AssertionError('reset did not come through the rom made here')
            emu.write(PROGRAM, MAIN)
            emu.write(HANDLER, HANDLER_CODE)
            emu.write(COUNTER, b'\x00')
            emu.add_break(HANDLER)
            emu.run()
            for n, ch in enumerate(TYPED):
                emu.key(ch)
                stop = emu.wait_stopped(timeout=5)
                if stop.reason != 'break' or stop.pc != HANDLER:
                    raise AssertionError('%r: expected the handler, got %r' % (ch, stop))
                regs = emu.regs()
                sp = regs['S']
                if sp != STACK - 7:
                    raise AssertionError('%r: S is %#06x, not seven bytes stacked' % (ch, sp))
                frame = emu.mem(sp + 1, 7)          # cc, b, a, x, pc
                pc = int.from_bytes(frame[5:7], 'big')
                if pc != WAIT:
                    raise AssertionError('%r: stacked pc %#06x, not the waiting branch' % (ch, pc))
                if frame[0] & 0x10 or not regs['CC'] & 0x10:
                    raise AssertionError('%r: I should be clear on the stack and set now (%#04x, %#04x)'
                                         % (ch, frame[0], regs['CC']))
                count = emu.mem(COUNTER, 1)[0]
                if count != n:
                    raise AssertionError('%r: the counter reads %d, not %d' % (ch, count, n))
                emu.run()
                emu.wait_for_output(TYPED[:n + 1])
            emu.remove_break(HANDLER)
            emu.halt()
            if emu.mem(COUNTER, 1)[0] != len(TYPED):
                raise AssertionError('the counter is not %d' % len(TYPED))
            if emu.reg('PC') != WAIT:
                raise AssertionError('not back in the waiting branch')
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
            print('FAIL: altair680 irq test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: altair680 irq test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
