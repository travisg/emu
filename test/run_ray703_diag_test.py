#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Run Raytheon's own 700-series diagnostics on the 703 and take their word
for it.

The tapes are the 1970 set the University of Stuttgart's computer museum
read off its 704 (tools/fetch-roms.py puts them in roms/703/diag). Each is a
core image behind a twelve-frame header (tools/tape703.py), loaded here the
way the absolute loader would have left it and started over the debug port.

No operating instructions for them are at hand, so what the SENSE switches
mean is read out of the code. In the instruction diagnostics every check
ends the same way --

            SS1             halt on an error, if switch 1 is up
            HLT
            ...             (flag the error)
            SS0             go round this check again, if switch 0 is up
            JMP     back

-- and their report routine prints `-ERROR-` with switch 2 up and each
group's name as it finishes with switch 3 up. So they are run with 1, 2
and 3 up, set through the debug port's SS register, and a tape passes when
it has printed its whole list of names twice over, in order, without a halt
and without the word ERROR: a halt is the diagnostic's own verdict, and the
names are how far it got.

390390C says nothing for thousands of passes and is watched with
breakpoints instead, and it runs with one word changed: see its entry
below. BRAINWASH (the memory test) and the teletype test print as they go.
The punch and reader test, 390444B, is a program for X-RAY's I/O entries,
and is not run (test/703/README.md says what was tried).

Negative controls, each of which must FAIL: DIAG_CONTROL=constant (the
primary diagnostic with its all-ones constant zeroed, which halts in its
first group), =unpatched (390390C as the museum read it, which never comes round) and
=memory (BRAINWASH comparing against a pattern it did not write, which
prints its error report). DIAG_TAPES=390392C,390443B runs some of them.
"""

import os
import queue
import re
import shutil
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu, Stopped  # noqa: E402
import tape703  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
DIAG_DIR = os.path.abspath(os.environ.get('DIAG_DIR', os.path.join(ROOT, 'roms', '703', 'diag')))
LOG_FILE = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_diag_test.log')
CONTROL = os.environ.get('DIAG_CONTROL', '')
ONLY = [t for t in os.environ.get('DIAG_TAPES', '').split(',') if t]
TIMEOUT = float(os.environ.get('DIAG_TIMEOUT', '120'))

# SENSE switches as the SS register takes them: bit n is switch n, set is up.
HALT_ON_ERROR, PRINT_ERRORS, PRINT_NAMES = 2, 4, 8
REPORT = HALT_ON_ERROR | PRINT_ERRORS | PRINT_NAMES

FOX = 'THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789'
TYPEBOX = '0123456789!"#$%&ABCDEFGHIJ\'():*-KLMNOPQRST=@_;+/UVWXYZ?.>,<^'


def twice(names):
    return names * 2


# One entry per run. `start` is the word to begin at and `switches` what the
# run is made with. `preamble` is the operator's part of a tape that has
# one: the switches to set, and where the HLT that follows them leaves the
# program counter. Then one of `lines`, what the teletype must print, from
# its first line, in order; `stops`, the breakpoints the run must come to,
# in order; or `keys`, each key to type and the line it must bring.
# `patches` are words written over the tape's.
CASES = [
    dict(tape='390392C', what='primary instructions, composite', start=0x80, switches=REPORT,
         # The tape opens by checking the switches themselves: all four up to
         # the HLT at 1A4, all four down to the one at 1AD, then the options.
         preamble=[(0xf, 0x1a5), (0x0, 0x1ae)],
         lines=twice(['ACR SKIPS', 'CLR INST.', 'INV INST.', 'ORE INST.', 'AND INST.', 'ORI INST.',
                      'ADD INST.', 'SUB INST.', 'CMP INST.', 'OVERFLOW', 'CAX CXA INST', 'LDX INST.',
                      'STX INST.', 'SXE INST.', 'CMW SEQ INST', 'EQU MAG INST'])),
    dict(tape='390389C', what='adder, composite', start=0x80, switches=REPORT,
         lines=twice(['ADC15  TEST', 'ADC14- TEST', 'ADC13- TEST', 'ADC12- TEST', 'ADC11- TEST',
                      'ADC10- TEST', 'ADC09- TEST', 'ADC08- TEST', 'ADC07 TEST', 'ADC06- TEST',
                      'ADC05- TEST', 'ADC04- TEST', 'ADC03- TEST', 'ADC02- TEST', 'ADC01- TEST'])),
    # The second-instruction composite prints one name in 65,536, so its
    # passes are counted at 85, where each begins. Its last section, the
    # extension register read back from code planted at 4K, 8K and 16K,
    # runs on the first pass of a machine whose memory does not wrap below
    # 32K, and ends at 542. On the tape as the museum read it, it cannot
    # get there: the 16K block's second check jumps on success to the
    # block's own first word (53A, `JMP X'528'`) where the 4K and 8K blocks
    # before it jump on to their `SS0` (`JMP X'50B'`, `JMP X'526'`), so a
    # machine that passes goes round that block for good. 53A is run as
    # `JMP X'540'`, the block's `SS0`.
    dict(tape='390390C', what='second instructions, composite', start=0x80, switches=HALT_ON_ERROR,
         patches={0x53a: 0x1540},
         stops=[0x85, 0x542, 0x85, 0x85]),
    dict(tape='390894B', what='secondary instructions, detail', start=0x40, switches=REPORT,
         lines=twice(['LLB INST.', 'LDB INST.', 'SMU,SML CXE', 'STW,STX,STB', 'IXS, DXS', 'CLB, CMB',
                      'INR STATUS', 'SHIFT OVFLO'])),
    dict(tape='390403C', what='logical instructions, detail', start=0x80, switches=REPORT,
         lines=twice(['ORE INST.', 'AND INST.', 'ORI INST.', 'ADD INST.'])),
    dict(tape='390388C', what='adder, detail, part 1', start=0x80, switches=REPORT,
         lines=twice(['ADC15  TEST', 'ADC14- TEST', 'ADC13- TEST', 'ADC12- TEST', 'ADC11- TEST',
                      'ADC10- TEST', 'ADC09- TEST', 'ADC08- TEST', 'ADC07  TEST'])),
    dict(tape='390387C', what='adder, detail, part 2', start=0x80, switches=REPORT,
         lines=twice(['ADC06- TEST', 'ADC05- TEST', 'ADC04- TEST', 'ADC03- TEST', 'ADC02- TEST',
                      'ADC01- TEST'])),
    dict(tape='390391C', what='shift instructions, detail', start=0x80, switches=REPORT,
         lines=twice(['SLA D', 'SRA D', 'SRL D', 'SLL D', 'SRC', 'SLC', 'SRC D', 'SLC D', 'SRC L',
                      'SLC L', 'SRC R', 'SLC R'])),
    # BRAINWASH IV writes and reads back seven patterns over the rest of a
    # 4K core above itself (the count is the tape's word 11) and says END OF
    # TEST after each round of them. Its switches are its own: a word that reads back wrong
    # is printed under a heading unless switch 3 is up, and with switch 2 up
    # it then halts.
    dict(tape='390332B', what='memory, BRAINWASH IV', start=0x185, switches=4,
         lines=['BRAINWASH IV', 'END OF TEST', 'END OF TEST']),
    # The teletype test does one thing for each switch that is up.
    dict(tape='390443B', what='teletype, switch 1: the fox', start=0x40, switches=2,
         lines=[FOX, FOX]),
    dict(tape='390443B', what='teletype, switch 0: the typebox, rippled', start=0x40, switches=1,
         lines=[TYPEBOX[n:] + TYPEBOX[:n] for n in range(3)]),
    # Switch 2 reads a key and prints a line of it. The first of each
    # line's characters is the Model 33 printing what its keyboard sent.
    dict(tape='390443B', what='teletype, switch 2: a line of the key typed', start=0x40, switches=4,
         keys=[('A', 'A' * 65), ('Z', 'Z' * 65)]),
]

CONTROLS = {
    # word 8B is the primary diagnostic's X'FFFF'
    'constant': ('390392C', lambda case: case['patches'].update({0x8b: 0})),
    'unpatched': ('390390C', lambda case: case['patches'].clear()),
    # 195 reloads the pattern to compare with: `LDW X'13'` made `LDW X'212'`
    'memory': ('390332B', lambda case: case['patches'].update({0x195: 0x8212})),
}


class Failed(Exception):
    pass


def printed(emu):
    """What the teletype has printed, as its lines: the characters without
    the high bit they carry and the nulls that pad them, blank lines
    dropped, each line trimmed. The last may still be printing."""
    text = bytes(c & 0x7f for c in emu.output).replace(b'\0', b'')
    return [line.strip().decode('ascii', 'replace')
            for line in re.split(rb'[\r\n]+', text) if line.strip()]


def next_stop(emu, timeout):
    """The next stop event, or None when `timeout` passes without one."""
    try:
        ev = emu.events.get(timeout=timeout)
    except queue.Empty:
        return None
    if not isinstance(ev, Stopped):
        raise Failed('the emulator went away: %r' % (ev,))
    return ev


def no_stop(emu, lines):
    ev = next_stop(emu, 0.05)
    if ev is not None:
        raise Failed('stopped (%s) at %04X in front of `%s`, having printed %r'
                     % (ev.reason, ev.pc, ev.insn, lines[-3:]))


def expect_lines(emu, want, deadline, more=True):
    """Wait for the teletype's lines to begin with `want`. Every line but
    the last printed is whole, and the last is still coming: `more` says
    the program prints on past `want`, so its last line is whole once
    another follows; without it, once the printer has gone quiet."""
    while True:
        lines = printed(emu)
        have = lines[:len(want)]
        for n, line in enumerate(have):
            coming = n == len(lines) - 1 and want[n].startswith(line)
            if line != want[n] and not coming:
                raise Failed('line %d is %r, not %r' % (n + 1, line, want[n]))
        if any('ERROR' in line for line in lines):
            raise Failed('it reports an error: %r' % [line for line in lines if 'ERROR' in line])
        if have == want:
            if len(lines) > len(want):
                return
            if not more:
                emu.wait_quiet(idle=0.3, timeout=max(1.0, deadline - time.monotonic()))
                if printed(emu) == lines:
                    return
                continue
        no_stop(emu, lines)
        if time.monotonic() > deadline:
            raise Failed('printed %d of %d lines and went no further: %r'
                         % (len(have), len(want), lines[-3:]))


def run_case(case, tape_path, workdir, log):
    with open(tape_path, 'rb') as f:
        origin, image, _ = tape703.read_tape(f.read())
    core = os.path.join(workdir, case['tape'] + '.core')
    with open(core, 'wb') as f:
        f.write(bytes(origin * 2) + image)

    # --no-throttle: the teletype still takes its tenth of a second a
    # character, but of the machine's time, not the wall's.
    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle'], log=log)
    try:
        for addr, word in case['patches'].items():
            emu.writew(addr, [word])
        emu.set_reg('PC', case['start'])
        deadline = time.monotonic() + TIMEOUT

        for switches, pc in case.get('preamble', []):
            emu.set_reg('SS', switches)
            emu.run()
            ev = next_stop(emu, TIMEOUT)
            if ev is None or ev.reason != 'hlt' or ev.pc != pc:
                raise Failed('with the switches at %X it should halt at %04X, not %r' % (switches, pc, ev))
        emu.set_reg('SS', case['switches'])

        if 'stops' in case:
            for addr in set(case['stops']):
                emu.add_break(addr)
            for n, addr in enumerate(case['stops']):
                emu.run()
                ev = next_stop(emu, max(0.0, deadline - time.monotonic()))
                if ev is None:
                    emu.halt()
                    raise Failed('never came to %04X (stop %d of %d); it is at %04X'
                                 % (addr, n + 1, len(case['stops']), emu.status()['pc']))
                if ev.reason != 'break' or ev.pc != addr:
                    raise Failed('stopped (%s) at %04X in front of `%s`, not at %04X'
                                 % (ev.reason, ev.pc, ev.insn, addr))
            if printed(emu):
                raise Failed('it printed %r' % printed(emu))
        elif 'keys' in case:
            emu.run()
            want = []
            for key, line in case['keys']:
                want.append(line)
                emu.key(key)
                expect_lines(emu, want, deadline, more=False)
        else:
            emu.run()
            expect_lines(emu, case['lines'], deadline)
        return printed(emu)
    finally:
        if emu.child.poll() is None:
            emu.child.kill()
        emu.child.wait()
        emu.child.stdin.close()
        emu.close()
        if emu.socket_dir:
            shutil.rmtree(emu.socket_dir, ignore_errors=True)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1

    cases = [dict(case, patches=dict(case.get('patches', {}))) for case in CASES]
    if CONTROL:
        if CONTROL not in CONTROLS:
            print('error: DIAG_CONTROL is one of %s' % ', '.join(sorted(CONTROLS)), file=sys.stderr)
            return 1
        tape, change = CONTROLS[CONTROL]
        cases = [case for case in cases if case['tape'] == tape]
        for case in cases:
            change(case)
    if ONLY:
        cases = [case for case in cases if case['tape'] in ONLY]
    if not cases:
        print('error: DIAG_TAPES names no tape this runs', file=sys.stderr)
        return 1

    missing = sorted({case['tape'] for case in cases
                      if not os.path.isfile(os.path.join(DIAG_DIR, case['tape'] + '.bin'))})
    if missing:
        print('FAIL: ray703 diag test: no %s in %s' % (', '.join(missing), DIAG_DIR), file=sys.stderr)
        print('fetch them with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    # The emulator runs in a directory of its own: from the root the 703
    # would mount whatever platters are in disks/, write-through.
    workdir = tempfile.mkdtemp(prefix='ray703-diag-')
    failed = 0
    try:
        os.chdir(workdir)
        with open(LOG_FILE, 'wb') as log:
            for case in cases:
                title = '%s %s' % (case['tape'], case['what'])
                log.write(('--- %s ---\n' % title).encode())
                log.flush()
                try:
                    lines = run_case(case, os.path.join(DIAG_DIR, case['tape'] + '.bin'), workdir, log)
                    log.write(('\n'.join(lines) + '\npass\n').encode())
                    print('  ok    %s' % title)
                except Exception as e:
                    failed += 1
                    log.write(('\n--- failure ---\n' + traceback.format_exc()).encode())
                    print('  FAIL  %s: %s' % (title, e if isinstance(e, Failed) else repr(e)))
    finally:
        os.chdir(HERE)
        shutil.rmtree(workdir, ignore_errors=True)

    if failed:
        print('FAIL: ray703 diag test, %d of %d (see %s)' % (failed, len(cases), LOG_FILE), file=sys.stderr)
        return 1
    print('PASS: ray703 diag test, %d runs of %d tapes (%s)'
          % (len(cases), len({case['tape'] for case in cases}), EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
