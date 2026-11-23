#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Hold the disassembled decks in test/703/disasm to their tapes.

Each deck there is a Raytheon program off one of the museum's tapes, written
by tools/dis703.py from the tape, the runs that worked it and the notes a
person keeps on it (test/703/disasm/decks.py has the list). Two things are
checked of each:

- it assembles with asm703.py --werror to its tape, word for word -- what
  a comment may say, this cannot be wrong about;
- it is exactly what writing it again gives, so that what is in it is
  what its notes and its runs say and nothing was edited in by hand.

The tapes are in roms/703/tapes (tools/fetch-roms.py), so it is local. The
second check needs the emulator, whose disassembler the tool uses.

Negative controls, each of which must FAIL: DISASM_CONTROL=operand (one
operand in a copy of a deck changed) and =notes (a note added that the
deck was not written again for).
"""

import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(HERE, '703', 'disasm'))

import decks  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
CONTROL = os.environ.get('DISASM_CONTROL', '')
CONTROLS = ('operand', 'notes')
DIS = os.path.join(ROOT, 'tools', 'dis703.py')


class Failed(Exception):
    pass


def check_assembles(deck, path):
    r = subprocess.run([sys.executable, DIS, 'check', path, deck.tape_path], capture_output=True, text=True)
    if r.returncode:
        raise Failed((r.stdout + r.stderr).strip())


def operand_changed(path, tmp):
    """A copy of the deck with the first branch to a label going one word on."""
    with open(path) as f:
        lines = f.read().split('\n')
    for i, line in enumerate(lines):
        fields = line.split(';')[0].split()
        if 'JMP' in fields and not fields[-1].startswith(('*', 'X\'')):
            lines[i] = line.replace(fields[-1], fields[-1] + '+1', 1)
            break
    copy = os.path.join(tmp, os.path.basename(path))
    with open(copy, 'w') as f:
        f.write('\n'.join(lines))
    return copy


def run(tmp):
    os.environ['EMU_BIN'] = EMU_BIN
    checked = []
    for deck in decks.DECKS:
        if not os.path.isfile(deck.tape_path):
            raise Failed('no %s in %s; fetch it with tools/fetch-roms.py' % (deck.tape, decks.TAPE_DIR))
        path = deck.path
        if CONTROL == 'operand':
            path = operand_changed(path, tmp)
        check_assembles(deck, path)

        if CONTROL == 'notes':
            notes = os.path.join(tmp, deck.name + '.notes')
            shutil.copy(deck.notes, notes)
            with open(notes, 'a') as f:
                f.write('%X ; a note the deck was not written again for\n' % deck_origin(deck))
            deck_notes = notes
        else:
            deck_notes = deck.notes
        again = os.path.join(tmp, deck.name + '.asm')
        argv = deck.argv(again)
        argv[argv.index('--notes') + 1] = deck_notes
        r = subprocess.run(argv, capture_output=True, text=True)
        if r.returncode:
            raise Failed('%s: writing it again: %s' % (deck.name, (r.stdout + r.stderr).strip()))
        with open(deck.path) as f, open(again) as g:
            mine, theirs = f.read().split('\n'), g.read().split('\n')
        if mine != theirs:
            at = next((i for i, (a, b) in enumerate(zip(mine, theirs)) if a != b), min(len(mine), len(theirs)))
            raise Failed('%s.asm is not what writing it again gives, from line %d:\n  %s\n  %s'
                         % (deck.name, at + 1, mine[at] if at < len(mine) else '(end)',
                            theirs[at] if at < len(theirs) else '(end)'))
        checked.append(deck.name)
    return checked


def deck_origin(deck):
    sys.path.insert(0, os.path.join(ROOT, 'tools'))
    import tape703
    with open(deck.tape_path, 'rb') as f:
        return tape703.read_tape(f.read())[0]


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if CONTROL and CONTROL not in CONTROLS:
        print('error: DISASM_CONTROL is one of %s' % ', '.join(CONTROLS), file=sys.stderr)
        return 1
    tmp = tempfile.mkdtemp(prefix='ray703-disasm-')
    try:
        checked = run(tmp)
    except Failed as e:
        print('FAIL: ray703 disasm test: %s' % e, file=sys.stderr)
        return 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print('PASS: ray703 disasm test, %s assemble to their tapes and are what their runs and notes give (%s)'
          % (', '.join(checked), EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
