#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Make the runs the decks are written from: sessions that work each program
under `emu --trace`, each trace reduced as it is written (through a fifo)
to which words ran, with which extension register, and where control went
-- `tools/dis703.py trace`. A run file is what is kept; the traces would be
hundreds of megabytes.

    runs.py ptios       PTIOS, under test/run_ray703_fortran_test.py
    runs.py sym1 [NAME ...]
                        SYM I, the sessions below, or just these

What the decks say was run is what these sessions did, so a session added
here, and the deck written again, is how a deck learns more of its program.

PTIOS is worked by the FORTRAN harness: PTIOS's set-up and its commands,
its loader reading the compiler and the library, and both programs calling
it for the teletype, the reader and the punch. They load over PTIOS's own
words from X'80' up, so those count only until the loader starts what it
loaded (`--until 80 80`); its resident words below are its own throughout.

SYM I is driven over the debug port as test/run_ray703_sym_test.py drives
it (that module doc says what was read of how to run it): directives at its
`??`, SENSE 3 at its pauses, a source from the high speed reader, the
teletype's or the keyboard. The keyboard sessions type cards at its source
read. A source with an error stops the session where SYM I stops. `range`
and `expression` make no words, and the checksum of an empty object runs
its loop until the index turns, when the IXS that skips the running sum
(0EA6) no longer does: the sum runs as an instruction, and from there the
machine wanders to a HLT at word 3. The words that ran only that way are
pinned as data in sym1.notes. `keyboard-errors` ends in SYM I's own loop at
its END, `NTRY START` there coming ahead of START. None of that depends on
--fast-io.
"""

import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(ROOT, 'tools'))

import dis703  # noqa: E402
import tape703  # noqa: E402
from emudbg import Emu, Stopped  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'release', 'emu')))
TAPE_DIR = os.path.abspath(os.environ.get('TAPE_DIR', os.path.join(ROOT, 'roms', '703', 'tapes')))
RUNS = os.path.join(HERE, 'runs')

# SYM I: its start, keyboard read and the wait in it, source read, the
# device byte that read uses, its pause and the pause's wait for SENSE 3
START, KEYBOARD, LISTENING, SOURCE, SOURCE_DEVICE, PAUSE, SWITCH_DOWN = (
    0x800, 0x6fc, 0x70f, 0x747, 0x6d3, 0x7b7, 0x7d1)
KEYS, TELETYPE_TAPE = 0xeb, 0xe9

FEATURES = """\
         ORIG  X'200'
TEN      EQU   10
START    LDW   ONE
         NTRY  START
         LDX * 2
         LDB * 0
         JSX   SUB
         DATA  'AB',X'1F',-1,TEN+TEN,$
         BYTE  1,2,3
BUF      RES   3
CH       DATA  'Z'
ONE      DATA  1
         STB   BUF+1
         CMB   CH
SUB      STX   RET
         SML   1
         LDW   X'800'
         DIN   14,0
         DOT   14,9
         SRA   3
         SLC   4
         ENB   0
         LLB   X'41'
         CLB   'A'
         IXS   2
         SAZ
         HLT
         LDX   RET
         JSX * 0
RET      DATA  0
         TRUE  TEN-10
         DATA  5
         ENDC
         FALS  TEN-10
         DATA  6
         ENDC
         END   START
"""

ERRORS = """\
         ORIG  X'200'
TEN      EQU   10
         NTRY  START
START    LDW   ONE
         LDX * 2
         STB   BUF+1
         LDB * 0
         CMB   CH
         JSX   SUB
         DATA  'AB',X'1F',-1,TEN+TEN
         BYTE  1,2,3
BUF      RES   3
CH       DATA  'Z'
ONE      DATA  1
SUB      STX   RET
RET      DATA  0
         END   START
"""


def one(card):
    return "         ORIG  X'300'\n" + card + "         END   A\n"


PTIOS_SOURCE = ('tape', 'PTIOS_SYM.bin')

# name: (steps, keyboard source or None, fast io)
# A step is a directive at `??`, ('tape', NAME) for the high speed reader,
# ('teletype', NAME) for the teletype's reader at the first source read, or
# ('sense', N) for the SENSE switches other than 3.
SESSIONS = {
    'ptios-sa': (['CH', 'CJ', PTIOS_SOURCE, 'SA'], None, True),
    'ptios-s': (['CH', 'CJ', PTIOS_SOURCE, 'S '], None, True),
    'ptios-ss0': ([('sense', 1), 'CH', 'CJ', PTIOS_SOURCE, 'SA'], None, True),
    'teletype': (['CT', 'CP', ('teletype', 'PTIOS_SYM.bin'), 'SA'], None, True),
    'keyboard-sa': (['CK', 'CJ', 'SA'], FEATURES, True),
    'keyboard-s': (['CK', 'CJ', 'S '], FEATURES, True),
    'keyboard-errors': (['CK', 'CJ', 'SA'], ERRORS, True),
    'undefined': (['CK', 'CJ', 'SA'], one("A        LDW   NOPE\n"), True),
    'operation': (['CK', 'CJ', 'SA'], one("A        FOO   1\n"), True),
    'expression': (['CK', 'CJ', 'SA'], one("A        LDW   A+\n"), True),
    'duplicate': (['CK', 'CJ', 'SA'], one("A        LDW   A\nA        LDW   A\n"), True),
    'range': (['CK', 'CJ', 'SA'], one("A        LLB   X'1FF'\n         DOT   99,1\n"), True),
    'external': (['CK', 'CJ', 'SA'], one("         LOAD  EXT\nA        JSX   EXT\n         LIBR  LIB\n"), True),
    'prep': (['CK', 'CJ', 'P '], FEATURES, True),
    'boot': (['CJ', 'X '], None, True),
    'boot-h': (['CJ', 'XH'], None, True),
}


def traced(name, argv_fn, until=None):
    """Run `argv_fn(trace_path)` with its trace reduced into runs/NAME.json."""
    tmp = tempfile.mkdtemp(prefix='ray703-runs-')
    try:
        fifo = os.path.join(tmp, 'trace')
        os.mkfifo(fifo)
        out = os.path.join(RUNS, name + '.json')
        reducer = threading.Thread(target=dis703.reduce_trace, args=(fifo, out, until))
        reducer.start()
        try:
            argv_fn(fifo, tmp)
        finally:
            # a run that never opened the fifo leaves the reducer waiting to
            # open it; one that did leaves it finishing, with no reader to
            # find, so the poke must not wait
            reducer.join(timeout=5)
            if reducer.is_alive():
                try:
                    os.close(os.open(fifo, os.O_WRONLY | os.O_NONBLOCK))
                except OSError:
                    pass
            reducer.join()
        with open(out) as f:
            print('%s: %d words ran' % (out, len(json.load(f)['ran'])))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def ptios():
    def run(fifo, tmp):
        wrapper = os.path.join(tmp, 'emu')
        with open(wrapper, 'w') as f:
            f.write('#!/bin/sh\nexec "%s" -t "%s" "$@"\n' % (EMU_BIN, fifo))
        os.chmod(wrapper, 0o755)
        env = dict(os.environ, EMU_BIN=wrapper)
        subprocess.run([os.path.join(ROOT, 'test', 'run_ray703_fortran_test.py'),
                        os.path.join(tmp, 'fortran.log')], env=env, check=True)
    traced('ptios-fortran', run, until=(0x80, 0x80))


def sym1_session(name, steps, cards, fast_io, fifo, tmp):
    with open(os.path.join(TAPE_DIR, '390470.BIN'), 'rb') as f:
        origin, image, _ = tape703.read_tape(f.read())
    core = os.path.join(tmp, 'sym.core')
    with open(core, 'wb') as f:
        f.write(bytes(origin * 2) + image)
    argv = [EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle', '--halt', '-t', fifo]
    if fast_io:
        argv.append('--fast-io')
    emu = Emu.spawn(argv)
    steps, cards = list(steps), (cards or '').splitlines()
    sense, teletype = 0, []

    def listen():
        for _ in range(64):
            if emu.status()['pc'] == LISTENING:
                return
            emu.step()
        raise SystemExit('%s: the read never reached its wait' % name)

    try:
        for addr in (KEYBOARD, SOURCE, PAUSE, SWITCH_DOWN):
            emu.add_break(addr)
        emu.set_reg('PC', START)
        while True:
            emu.run()
            try:
                ev = emu.events.get(timeout=10 if fast_io else 300)
            except queue.Empty:
                break           # SYM I's own loop: the session ends there
            if not isinstance(ev, Stopped) or ev.reason != 'break':
                break           # its HLT
            if ev.pc == KEYBOARD:
                while steps and isinstance(steps[0], tuple):
                    kind, value = steps.pop(0)
                    if kind == 'tape':
                        emu.tape(0, os.path.join(TAPE_DIR, value))
                    elif kind == 'teletype':
                        teletype.append(os.path.join(TAPE_DIR, value))
                    elif kind == 'sense':
                        sense = value
                        emu.set_reg('SS', sense)
                if not steps:
                    break
                listen()
                emu.key('\n' + steps.pop(0))
            elif ev.pc == SOURCE:
                device = emu.memw(SOURCE_DEVICE, 1)[0]
                if device == TELETYPE_TAPE and teletype:
                    listen()
                    emu.tape(1, teletype.pop(0))
                elif device == KEYS:
                    listen()
                    emu.key('\n' + (cards.pop(0) if cards else '         END') + '\r')
            elif ev.pc == PAUSE:
                emu.set_reg('SS', sense | 8)
            elif ev.pc == SWITCH_DOWN:
                emu.set_reg('SS', sense)
    finally:
        if emu.child.poll() is None:
            emu.child.kill()
        emu.child.wait()
        emu.child.stdin.close()
        emu.close()
        if emu.socket_dir:
            shutil.rmtree(emu.socket_dir, ignore_errors=True)


def sym1(names):
    for name in names or SESSIONS:
        steps, cards, fast_io = SESSIONS[name]
        traced('sym1-' + name, lambda fifo, tmp: sym1_session(name, steps, cards, fast_io, fifo, tmp))


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ('ptios', 'sym1'):
        print(__doc__.split('\n\n')[1], file=sys.stderr)
        return 1
    os.makedirs(RUNS, exist_ok=True)
    if sys.argv[1] == 'ptios':
        ptios()
    else:
        sym1(sys.argv[2:])
    return 0


if __name__ == '__main__':
    sys.exit(main())
