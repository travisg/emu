#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test of the 8080 core's interrupt and the Altair 8800's
88-2SIO, whose port 0 interrupt is jumpered to the bus's PINT with no
vectored interrupt board, so the pulled-up data bus answers INTA with
RST 7. A hand-assembled program, in a memory image made here and run with
`altair8800-image`, programs the ACIA for a receive interrupt and waits in
`EI; HLT`, the 8080's way of sleeping until an interrupt. The handler at
0x0038 reads the character, echoes it, counts it, and returns with
`EI; RET` into a jump back to the HLT.

What the harness holds the machine to, with a breakpoint on the handler's
body, for each character typed: the machine was asleep on the HLT (PC
held on it, HALT set) before the key; the entry came through RST 7 (the
vector jumps to the body, and nothing else does) with INTE cleared; the
return address on the stack is the one past the HLT; the count in memory
keeps up; and the echo comes back on the port.

Needs nothing outside the repo, so CI runs it. Every wait raises on its
timeout, and the handler at the bottom makes that a FAIL with the
transcript in the log. Negative controls, both of which must FAIL:
ALTAIR8800_IRQ_CONTROL=masked waits in DI; HLT, which never wakes, and
=rie-off programs the ACIA without its receive interrupt.
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
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'altair8800_irq_test.log')
CONTROL = os.environ.get('ALTAIR8800_IRQ_CONTROL', '')

PROGRAM = 0x0100
HANDLER = 0x0200
COUNTER = 0x0050
STACK = 0x8000

ACIA_CONTROL = 0x15 if CONTROL == 'rie-off' else 0x95   # RIE, 8N1, divide by 16
ENABLE = 0xf3 if CONTROL == 'masked' else 0xfb          # di / ei

MAIN = bytes([
    0x31, STACK & 0xff, STACK >> 8,     # lxi sp,0x8000
    0x3e, 0x03,                         # mvi a,3
    0xd3, 0x10,                         # out 0x10     ; master reset
    0x3e, ACIA_CONTROL,                 # mvi a,0x95
    0xd3, 0x10,                         # out 0x10     ; receive interrupt on
    ENABLE,                             # ei
    0x76,                               # hlt          ; 0x010c: wait
    0xc3, 0x0b, 0x01,                   # jmp 0x010b   ; back to the ei
])
WAIT = PROGRAM + 0x0c
HANDLER_CODE = bytes([
    0xf5,                               # push psw
    0xdb, 0x11,                         # in 0x11      ; the character, which drops IRQ
    0xd3, 0x11,                         # out 0x11     ; echoed
    0x21, COUNTER, 0x00,                # lxi h,0x0050
    0x34,                               # inr m
    0xf1,                               # pop psw
    0xfb,                               # ei
    0xc9,                               # ret
])
TYPED = 'IRQ8080'


def image():
    """Memory from 0: a jump to the program, RST 7's vector to the handler,
    the program and the handler."""
    mem = bytearray(HANDLER + len(HANDLER_CODE))
    mem[0:3] = bytes([0xc3, PROGRAM & 0xff, PROGRAM >> 8])
    mem[0x38:0x3b] = bytes([0xc3, HANDLER & 0xff, HANDLER >> 8])
    mem[PROGRAM:PROGRAM + len(MAIN)] = MAIN
    mem[HANDLER:HANDLER + len(HANDLER_CODE)] = HANDLER_CODE
    return bytes(mem)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1

    emu = None
    with tempfile.TemporaryDirectory(prefix='altair8800-irq-') as workdir, open(LOG_FILE, 'wb') as log:
        path = os.path.join(workdir, 'image.bin')
        with open(path, 'wb') as f:
            f.write(image())
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'altair8800-image', '-r', path, '--no-throttle'], log=log)
            emu.write(COUNTER, b'\x00')
            emu.add_break(HANDLER)
            emu.run()
            for n, ch in enumerate(TYPED):
                emu.halt()
                regs = emu.regs()
                if regs['PC'] != WAIT or regs['HALT'] != 1:
                    raise AssertionError('%r: not asleep on the hlt: %r' % (ch, regs))
                emu.run()
                emu.key(ch)
                stop = emu.wait_stopped(timeout=5)
                if stop.reason != 'break' or stop.pc != HANDLER:
                    raise AssertionError('%r: expected the handler, got %r' % (ch, stop))
                regs = emu.regs()
                if regs['SP'] != STACK - 2:
                    raise AssertionError('%r: SP is %#06x, not one return address stacked' % (ch, regs['SP']))
                ret = int.from_bytes(emu.mem(regs['SP'], 2), 'little')
                if ret != WAIT + 1:
                    raise AssertionError('%r: returns to %#06x, not past the hlt' % (ch, ret))
                if regs['INTE'] != 0 or regs['HALT'] != 0:
                    raise AssertionError('%r: INTE should be clear and the hlt over: %r' % (ch, regs))
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
                raise AssertionError('not back on the hlt')
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
            print('FAIL: altair8800 irq test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: altair8800 irq test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
