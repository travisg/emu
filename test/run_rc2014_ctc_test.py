#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test of the Z80 core's IM 2 and the RC2014's CTC: a
hand-assembled program, written into RAM over the debug port, sets IM 2
with a vector table under I, programs CTC channel 0 as a free-running
timer -- prescaler 256, time constant 256, an interrupt every 65,536
cycles -- and sleeps in HALT. The handler counts the ticks and ends
`EI; RETI`.

What the harness holds the machine to, with a breakpoint on the handler:
the entry comes through the vector table (the breakpoint is on the address
the table names, nothing else points there); each one was taken from the
HALT, with the address after it on the stack; the tick count in memory
keeps up; and the ticks come 65,536 cycles apart, which is the timer's
period through the core's own cycle accounting -- while asleep the core
charges four cycles a step, so the steps between ticks are the period over
four, less a little: the handler's and the loop's instructions each cost
more cycles than a sleeping step's four, and the acceptance costs
nineteen. The tolerance is those, not the timer.

The rom is a file of zeros made here, so this needs nothing outside the
repo and CI runs it: the program never returns to the rom and never
touches the SIO. Every wait raises on its timeout, and the handler at the
bottom makes that a FAIL with the transcript in the log.
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
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'rc2014_ctc_test.log')

CTC = 0x88
PROGRAM = 0x9000
TABLE = 0x9800          # I = 0x98; the CTC's vector 0x10 selects entry 0x9810
HANDLER = 0x9820
COUNTER = 0x9830
PERIOD = 256 * 256      # prescaler 256, time constant 0 = 256

MAIN = bytes([
    0xf3,               # di
    0xed, 0x5e,         # im 2
    0x3e, TABLE >> 8,   # ld a, 0x98
    0xed, 0x47,         # ld i, a
    0x3e, 0x10,         # ld a, 0x10       ; the vector: bits 7-3, channel in 2-1
    0xd3, CTC,          # out (0x88), a
    0x3e, 0xa5,         # ld a, 0xa5       ; interrupt, timer, prescaler 256, constant follows
    0xd3, CTC,          # out (0x88), a
    0x3e, 0x00,         # ld a, 0          ; time constant 256
    0xd3, CTC,          # out (0x88), a
    0xfb,               # ei
    0x76,               # halt             ; 0x9014: sleep until the tick
    0x18, 0xfd,         # jr 0x9014        ; 0x9015: and again
])
HALT_ADDR = PROGRAM + 0x14
AFTER_HALT = HALT_ADDR + 1
HANDLER_CODE = bytes([
    0xf5,                               # push af
    0x21, COUNTER & 0xff, COUNTER >> 8, # ld hl, counter
    0x34,                               # inc (hl)
    0xf1,                               # pop af
    0xfb,                               # ei
    0xed, 0x4d,                         # reti
])
ENTRY = bytes([HANDLER & 0xff, HANDLER >> 8])

TICKS = 5


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1

    emu = None
    with tempfile.TemporaryDirectory(prefix='rc2014-ctc-') as workdir, open(LOG_FILE, 'wb') as log:
        rom = os.path.join(workdir, 'zeros.bin')
        with open(rom, 'wb') as f:
            f.write(bytes(64 * 1024))
        try:
            emu = Emu.spawn([EMU_BIN, '-s', 'rc2014', '-r', rom, '--no-throttle'], log=log)
            emu.write(PROGRAM, MAIN)
            emu.write(HANDLER, HANDLER_CODE)
            emu.write(TABLE + 0x10, ENTRY)
            emu.write(COUNTER, b'\x00')
            emu.set_reg('PC', PROGRAM)
            emu.add_break(HANDLER)
            emu.run()
            last = None
            for n in range(1, TICKS + 1):
                stop = emu.wait_stopped()
                if stop.reason != 'break' or stop.pc != HANDLER:
                    raise AssertionError('tick %d: expected the handler, got %r' % (n, stop))
                regs = emu.regs()
                if (regs['IM'], regs['I'], regs['IFF1']) != (2, TABLE >> 8, 0):
                    raise AssertionError('tick %d: registers %r' % (n, regs))
                ret = int.from_bytes(emu.mem(regs['SP'], 2), 'little')
                if ret != AFTER_HALT:
                    raise AssertionError('tick %d: return address %#06x, not past the halt' % (n, ret))
                count = emu.mem(COUNTER, 1)[0]
                if count != n - 1:
                    raise AssertionError('tick %d: the counter reads %d' % (n, count))
                insns = emu.status()['insns']
                if last is not None:
                    steps = insns - last
                    # the period in four-cycle sleeping steps, less the
                    # cycles the handler, the loop and the acceptance cost
                    # beyond a step each: 16,369 with this handler
                    if not PERIOD // 4 - 40 <= steps <= PERIOD // 4:
                        raise AssertionError('tick %d: %d steps since the last, not a period' % (n, steps))
                last = insns
                emu.run()
            emu.remove_break(HANDLER)
            emu.halt()
            if emu.mem(COUNTER, 1)[0] < TICKS:
                raise AssertionError('the counter fell behind after the breakpoint came off')
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
            print('FAIL: rc2014 ctc test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: rc2014 ctc test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
