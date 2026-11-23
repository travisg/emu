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

    runs.py fortran [NAME ...]
                        PTIOS and Conversational FORTRAN, the FORTRAN
                        harness (`harness`) and the sessions below
    runs.py sym1 [NAME ...]
                        SYM I, the sessions below, or just these

What the decks say was run is what these sessions did, so a session added
here, and the deck written again, is how a deck learns more of its program.

PTIOS is worked by the FORTRAN harness: PTIOS's set-up and its commands,
its loader reading the compiler and the library, and both programs calling
it for the teletype, the reader and the punch. The compiler and the library
load over PTIOS's own words from X'80' up and over each other, so one trace
makes a run file for each, by which of the loader's entries into X'80'
started what is there (`--program 80 80 N`): PTIOS's own start is the
first. PTIOS's resident words below X'80' are its own throughout.

The FORTRAN sessions after the harness's are typed as the harness types its
own (its Session does the work): commands at the compiler's and the
library's teletype reads, the source threaded on the reader first, and
SENSE 3 up and down at each program's operator pause. `statements` compiles
a program with every function in the compiler's table and its statements
but four, and a subroutine, then runs it four times, a fresh library each
time, with each of the library's trace settings. Of the statements, LOCAT's
form is not known, and the three for positioning a tape stop on paper tape:
REWIND at a HLT in PTIOS (000E), ENDFILE in its wait for the punch (006D),
and BACKSPACE by going to X'80'. `errors` compiles a program of errors,
types statements at PREP, punches with B, starts again with I, and runs a
program that refers to a function the library does not have. Each command is one the program
was found to take: PREP straight after a compile with errors, before an I,
was still in PTIOS's transfer loop when the session gave up on it, and a
CALL of a subroutine never compiled sends the library off into empty
memory, so neither is in a session.

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

sys.path.insert(0, os.path.join(ROOT, 'test'))
import run_ray703_fortran_test as harness_test  # noqa: E402

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


def traced(names, argv_fn, programs=(None,)):
    """Run `argv_fn(trace_path, tmp)` with its trace reduced into
    runs/NAME.json for each name, each with its program as dis703's
    `--program` says: for each, how many entries into its START there were."""
    tmp = tempfile.mkdtemp(prefix='ray703-runs-')
    try:
        fifo = os.path.join(tmp, 'trace')
        os.mkfifo(fifo)
        outs = [(os.path.join(RUNS, name + '.json'), program) for name, program in zip(names, programs)]
        reduced = []
        reducer = threading.Thread(target=lambda: reduced.extend(dis703.reduce_trace(fifo, outs)))
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
        for (out, _), (_, entries) in zip(outs, reduced):
            with open(out) as f:
                print('%s: %d words ran' % (out, len(json.load(f)['ran'])))
        return [entries for _, entries in reduced]
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


# The FORTRAN harness's programs, by the loader's entries into X'80': PTIOS
# itself, then the compiler and the library twice each.
HARNESS_RUNS = [('ptios-fortran', (0x80, 0x80, {1})),
                ('fortran-harness', (0x80, 0x80, {2, 4})),
                ('fortlib-harness', (0x80, 0x80, {3, 5}))]


def harness():
    def run(fifo, tmp):
        wrapper = os.path.join(tmp, 'emu')
        with open(wrapper, 'w') as f:
            f.write('#!/bin/sh\nexec "%s" -t "%s" "$@"\n' % (EMU_BIN, fifo))
        os.chmod(wrapper, 0o755)
        env = dict(os.environ, EMU_BIN=wrapper, FORTRAN_TIMEOUT='900')
        subprocess.run([os.path.join(ROOT, 'test', 'run_ray703_fortran_test.py'),
                        os.path.join(tmp, 'fortran.log')], env=env, check=True)
    entries = traced([name for name, _ in HARNESS_RUNS], run, [program for _, program in HARNESS_RUNS])
    if entries[0] != 5:
        raise SystemExit('harness: the loader entered X\'80\' %d times, not 5' % entries[0])


# Each program's operator pause: SENSE 3 up, then down. The compiler's
# PREP pauses there before and after punching; the library's PAUSE is one.
SENSE3 = {'compiler': (0xb07, 0xb0a), 'library': (0x906, 0x909)}

STATEMENTS = """\
C     THE STATEMENTS AND FUNCTIONS IN THE COMPILER'S TABLE
      COMMON C(2),K
      DIMENSION A(5),B(2,3)
      K=7
      C(1)=1.5
      CALL SUB(C(2),K)
      DO 10 I=1,5
      A(I)=I*2
      IF (I-3) 10,20,10
20    B(1,2)=A(I)
10    CONTINUE
      N=2
      GO TO (30,40,50), N
30    STOP
40    X=EXP(1.)+ALOG(2.)+SIN(.5)+COS(.5)+SQRT(2.)+ATAN(1.)
      Y=ABS(-X)+SIGN(2.,-1.)+FLOAT(IABS(-3))
      J=IFIX(Y)+ISIGN(4,-1)
      L=IAND(12,10)+IOR(12,10)+IEOR(12,10)+NOT(0)
      M=ISWCH(3)
      READ (6,4) P,Q,KA
      GO TO 50
50    WRITE (13,1) X,Y,B(1,2),C(2)
      WRITE (13,2) J,L,M,K
      PRINT 3, X,P,Q,KA
      WRITE (5,1) P,Q
1     FORMAT (4F10.4)
2     FORMAT (4I6)
3     FORMAT (1X,E14.6/5H DONE,2E12.4,I5)
4     FORMAT (F6.2,E10.3,I4)
      PAUSE
      STOP
      END
"""
SUBROUTINE = """\
      SUBROUTINE SUB(Z,KK)
      COMMON C(2),K
      Z=KK
      Z=Z+C(1)
      K=K+1
      RETURN
      END
"""
DATA = """\
  1.25-3.500E+02  17
"""
# a line for each of the compiler's complaints that could be found
ERRORS = """\
      DIMENSION A(2),A(3)
      X=I+
      DIMENSION B(
      F(Y)=Y
      X=1.5E99
      X=I+1.5
10    X=1.
10    X=2.
      DO 20 I=1,5
      GO TO 999
      X=(1.+2.
      IF (X) 1,2
      Y=SIN(1.,2.)
      A(1,2)=1.
      WRITE (13,99) X
      FUNCTION G(X)
      RETURN
      READ (13) X
      ABCDEFGH=1
      X=1.+*2.
      END
"""
# a function the library does not have; a CALL of a subroutine never
# compiled sends the library off into empty memory instead
MISSING = """\
      X=TANH(1.)
      STOP
      END
"""
PREP = ['      I=3', '      X=I+', '      Y=X*2.', '      END']

# name: steps. A step is ('load', 'compiler' or 'library') at the loader;
# a command or a line for the teletype's next read, with a source for the
# reader threaded first; ('data', TEXT) for a data tape on the reader; or
# ('loader',), to run on to the loader.
FORTRAN_SESSIONS = {
    'statements': [('load', 'compiler'), ('GL', STATEMENTS), ('G', SUBROUTINE), ('E',),
                   ('load', 'library'), ('data', DATA), ('B',), ('G',), ('loader',),
                   ('load', 'library'), ('data', DATA), ('V',), ('G',), ('loader',),
                   ('load', 'library'), ('data', DATA), ('C',), ('G',), ('loader',),
                   ('load', 'library'), ('data', DATA), ('N',), ('G',), ('loader',)],
    'errors': [('load', 'compiler'), ('G', ERRORS), ('I',), ('P',)] + [(line,) for line in PREP] +
              [('B',), ('I',), ('G', MISSING), ('E',), ('load', 'library'), ('G',), ('E',), ('loader',)],
}


def fortran_windows(steps):
    """Which of the loader's entries into X'80' start the compiler and
    which the library: PTIOS's own start is the first."""
    windows, entry = {'compiler': set(), 'library': set()}, 1
    for step in steps:
        if step[0] == 'load':
            entry += 1
            windows[step[1]].add(entry)
    return windows


def fortran_session(name, steps, fifo, tmp):
    with open(os.path.join(TAPE_DIR, harness_test.PTIOS), 'rb') as f:
        origin, image, _ = tape703.read_tape(f.read())
    core = os.path.join(tmp, 'ptios.core')
    with open(core, 'wb') as f:
        f.write(bytes(origin * 2) + image)
    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle', '--fast-io', '--halt', '-t', fifo])
    tapes = {'compiler': harness_test.COMPILER, 'library': harness_test.RUNTIME}
    harness_test.TIMEOUT = 300
    try:
        session = harness_test.Session(emu, tmp, open(os.devnull, 'wb'))
        for addr in (harness_test.DOIO, harness_test.LOADER, harness_test.START):
            emu.add_break(addr)
        emu.set_reg('PC', origin)
        for assignment in harness_test.ASSIGN:
            session.keyboard('PTIOS')
            session.type(assignment)
        session.keyboard('PTIOS')
        session.type('AL')
        session.until('the loader', lambda at, device, function: at == harness_test.LOADER)
        program = None
        for step in steps:
            if step[0] == 'load':
                if program:
                    for addr in SENSE3[program]:
                        emu.remove_break(addr)
                program = step[1]
                session.load(tapes[program])
                for addr in SENSE3[program]:
                    emu.add_break(addr)
                continue
            if step[0] == 'data':
                session.tape(name + '.dat', harness_test.source_tape(step[1]))
                continue
            while True:
                at, device, function = session.stop()
                if program and at == SENSE3[program][0]:
                    emu.set_reg('SS', 8)
                elif program and at == SENSE3[program][1]:
                    emu.set_reg('SS', 0)
                elif step[0] == 'loader' and at == harness_test.LOADER:
                    break
                elif at == harness_test.DOIO and device == harness_test.TELETYPE and function in (0x9, 0xb):
                    if step[0] == 'loader':
                        raise SystemExit('%s: asked for a command on the way to the loader: %r'
                                         % (name, session.printed()))
                    if len(step) > 1:
                        session.tape(name + '.for', harness_test.source_tape(step[1]))
                    session.type(step[0])
                    break
                elif at == harness_test.LOADER:
                    raise SystemExit('%s: in the loader, waiting to type %r: %r'
                                     % (name, step[0], session.printed()))
        print('%s: %s' % (name, session.printed().replace('\r', '').strip()))
    finally:
        if emu.child.poll() is None:
            emu.child.kill()
        emu.child.wait()
        emu.child.stdin.close()
        emu.close()
        if emu.socket_dir:
            shutil.rmtree(emu.socket_dir, ignore_errors=True)


def fortran(names):
    if not names or 'harness' in names:
        harness()
    for name in names or FORTRAN_SESSIONS:
        if name == 'harness':
            continue
        steps = FORTRAN_SESSIONS[name]
        windows = fortran_windows(steps)
        runs = [(prefix + name, (0x80, 0x80, windows[program]))
                for prefix, program in (('fortran-', 'compiler'), ('fortlib-', 'library')) if windows[program]]
        entries = traced([run for run, _ in runs], lambda fifo, tmp: fortran_session(name, steps, fifo, tmp),
                         [window for _, window in runs])
        loads = 1 + sum(1 for step in steps if step[0] == 'load')
        if entries[0] != loads:
            raise SystemExit('%s: the loader entered X\'80\' %d times, not %d' % (name, entries[0], loads))


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
        traced(['sym1-' + name], lambda fifo, tmp: sym1_session(name, steps, cards, fast_io, fifo, tmp))


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ('fortran', 'sym1'):
        print(__doc__.split('\n\n')[1], file=sys.stderr)
        return 1
    os.makedirs(RUNS, exist_ok=True)
    if sys.argv[1] == 'fortran':
        fortran(sys.argv[2:])
    else:
        sym1(sys.argv[2:])
    return 0


if __name__ == '__main__':
    sys.exit(main())
