#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""The disassembled decks: which tape each comes from, the runs and notes it
is written from, and writing them again.

    decks.py              write every deck from its tape, runs and notes
    decks.py NAME ...     just these

A deck is never edited by hand: what a person knows goes in its notes file
(see Notes in tools/dis703.py), and the deck is written again. That keeps a
comment from being lost when the runs grow or the tool learns something,
and test/run_ray703_disasm_test.py holds each deck to being exactly what
this writes.

The tapes are the museum's, in roms/703/tapes (tools/fetch-roms.py). Writing
a deck needs the emulator, for its disassembler: EMU_BIN, by default the
debug build.
"""

import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
TAPE_DIR = os.path.abspath(os.environ.get('TAPE_DIR', os.path.join(ROOT, 'roms', '703', 'tapes')))
DIS = os.path.join(ROOT, 'tools', 'dis703.py')


class Deck:
    def __init__(self, name, tape, title, runs, aliases=()):
        self.name, self.tape, self.title = name, tape, title
        self.runs, self.aliases = runs, aliases

    @property
    def path(self):
        return os.path.join(HERE, self.name + '.asm')

    @property
    def notes(self):
        return os.path.join(HERE, self.name + '.notes')

    @property
    def tape_path(self):
        return os.path.join(TAPE_DIR, self.tape)

    def argv(self, out):
        argv = [sys.executable, DIS, 'deck', self.tape_path, '-o', out, '--title', self.title,
                '--notes', self.notes]
        for run in self.runs:
            argv += ['--run', os.path.join(HERE, 'runs', run + '.json')]
        for alias in self.aliases:
            argv += ['--alias', alias]
        return argv


SYM1_RUNS = ['sym1-' + name for name in (
    'ptios-sa', 'ptios-s', 'ptios-ss0', 'teletype', 'keyboard-sa', 'keyboard-s', 'keyboard-errors',
    'undefined', 'operation', 'expression', 'duplicate', 'range', 'external', 'prep', 'boot', 'boot-h')]

FORTRAN_SESSIONS = ('harness', 'statements', 'errors')
FORTRAN_RUNS = ['fortran-' + name for name in FORTRAN_SESSIONS]
FORTLIB_RUNS = ['fortlib-' + name for name in FORTRAN_SESSIONS]

DECKS = [
    # PTIOS copies its words from X'E2' on to word 0 and runs them there
    Deck('ptios', 'PTIOS.bin', 'PTIOS, the Paper Tape I/O System, DN 393954 (1981)',
         ['ptios-fortran'], aliases=['0-7F=E2']),
    Deck('sym1', '390470.BIN', 'SYM I/PREP, the one-pass assembler, DN 390470 (7/10/70)', SYM1_RUNS),
    Deck('fortran', '393295B.bin', 'Conversational FORTRAN, the compiler for a 4K machine, DN 393295 rev B',
         FORTRAN_RUNS),
    Deck('fortlib', '394002B.bin', 'Conversational FORTRAN, the run-time library, DN 394002 rev B',
         FORTLIB_RUNS),
]


def write(deck, out):
    r = subprocess.run(deck.argv(out), capture_output=True, text=True)
    if r.returncode:
        raise SystemExit('%s: %s%s' % (deck.name, r.stdout, r.stderr))
    return r.stdout.strip()


def main():
    want = sys.argv[1:]
    for deck in DECKS:
        if want and deck.name not in want:
            continue
        print(write(deck, deck.path))
    return 0


if __name__ == '__main__':
    sys.exit(main())
