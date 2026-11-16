#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""tools/asm703.py held to itself: its literals and its warnings.

The images the tree builds say the assembler still assembles what it did;
this says what the parts no image exercises do. Each case is a small deck
written out and assembled through the same entry the command line uses.

A literal is checked the strong way: a deck written with literals has to
come out word for word, and in a module relocation code for relocation
code, as the same deck written with the cells laid down by hand where the
pool goes. The rest are the rules -- which literals share a cell, where a
pool may be and what happens with none, what may not take one -- and the
warnings, each from a deck that has the mistake and not from its neighbour
that does not.
"""

import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.join(os.path.dirname(HERE), 'tools')
sys.path.insert(0, TOOLS)

import asm703  # noqa: E402

FAILED = []


def build(source, relocatable=False, origin=0):
    """Assemble a deck given as text: the assembler's result, or the
    AsmErrorList it raised."""
    with tempfile.NamedTemporaryFile('w', suffix='.asm', delete=False) as f:
        f.write(source)
    try:
        return asm703._assemble([f.name], origin=origin, relocatable=relocatable)
    except asm703.AsmErrorList as e:
        return e
    finally:
        os.unlink(f.name)


def words(result):
    return [result['core'][a] for a in sorted(result['core'])]


def check(name, ok, detail=''):
    if not ok:
        FAILED.append(name)
        print('FAIL: %s%s' % (name, ': %s' % (detail,) if detail else ''))


def errors_of(result):
    return [str(e).split(': ', 1)[1] for e in getattr(result, 'errors', [])]


def warnings_of(result):
    found = result.warnings if isinstance(result, asm703.AsmErrorList) else result['warnings']
    return [w.message for w in found]


def same(name, with_literals, by_hand, relocatable=False):
    a, b = build(with_literals, relocatable), build(by_hand, relocatable)
    for r in (a, b):
        if isinstance(r, asm703.AsmErrorList):
            check(name, False, errors_of(r))
            return
    check(name, words(a) == words(b),
          '%s against %s' % (' '.join('%04X' % w for w in words(a)), ' '.join('%04X' % w for w in words(b))))
    if relocatable:
        check(name + ' (relocation)', a['kinds'] == b['kinds'], (a['kinds'], b['kinds']))
    check(name + ' (no warnings)', not warnings_of(a), warnings_of(a))


def literals():
    # One cell to a value however it is spelled, in the order first asked
    # for, in front of the END.
    same('literals pooled at the end', """
ONE     EQU     1
START   LDW     =X'7FFF'
        ADD     =1
        SUB     =ONE
        AND     =X'0001'
        CMW     =-1
        ORI     =X'FFFF'
        LDX     ='AB'
        HLT
        END     START
""", """
START   LDW     L1
        ADD     L2
        SUB     L2
        AND     L2
        CMW     L3
        ORI     L3
        LDX     L4
        HLT
L1      WORD    X'7FFF'
L2      WORD    1
L3      WORD    X'FFFF'
L4      WORD    'AB'
        END     START
""")

    # An LPOOL takes what has been asked for so far; what comes after it
    # goes to the next, even a value the first already holds.
    same('two pools', """
START   LDW     =5
        ADD     =6
        JMP     ON
        LPOOL
ON      SUB     =5
        HLT
        END     START
""", """
START   LDW     P1
        ADD     P2
        JMP     ON
P1      WORD    5
P2      WORD    6
ON      SUB     Q1
        HLT
Q1      WORD    5
        END     START
""")

    # A symbol defined further down: known by its spelling, so two uses
    # share and the address in the cell is the symbol's.
    same('a literal naming what comes later', """
START   LDX     =TABLE
        LDW     =TABLE
        CMW     =TABLE+1
        HLT
TABLE   WORD    7,8
        END     START
""", """
START   LDX     L1
        LDW     L1
        CMW     L2
        HLT
TABLE   WORD    7,8
L1      WORD    TABLE
L2      WORD    TABLE+1
        END     START
""")

    # In a module the cell relocates as the same word written by hand
    # would: a word address, a byte address, a number.
    same('literals in a module', """
START   LDX     =BUF
        LDW     =BUF*2
        ADD     =2
        MPY     =10
        HLT
BUF     RES     2
        END     START
""", """
START   LDX     L1
        LDW     L2
        ADD     L3
        MPY     L4
        HLT
BUF     RES     2
L1      WORD    BUF
L2      WORD    BUF*2
L3      WORD    2
L4      WORD    10
        END     START
""", relocatable=True)

    # The pool's label is where it starts, and the listing has a line to
    # each literal.
    r = build("""
START   LDW     =3
        HLT
POOL    LPOOL
AFTER   WORD    9
        END     START
""")
    check('the pool is labelled', r['symbols'].get('POOL') == 2 and r['symbols'].get('AFTER') == 3,
          r['symbols'])
    check('no pool slot in the symbol table',
          not any(name.startswith(asm703.LITERAL_PREFIX) for name in r['symbols']), r['symbols'])
    check('the listing shows the literal',
          any(addr == 2 and ws == [3] and '=3' in text for addr, ws, text in r['listing']), r['listing'])

    # A deck of two pages: a literal reaches only a pool in its own.
    two_pages = """
        ORG     X'40'
START   LDW     =1
        HLT
%s        ORG     X'800'
HIGH    ADD     =1
        HLT
        END     START
"""
    r = build(two_pages % '')
    check('a literal out of reach of its pool',
          any('pooled at 0x802' in e and 'LPOOL' in e for e in errors_of(r)), errors_of(r))
    r = build(two_pages % '        LPOOL\n')
    check('a pool to each page', not isinstance(r, asm703.AsmErrorList)
          and r['core'][0x42] == 1 and r['core'][0x802] == 1, errors_of(r))

    for name, card, want in (
            ('a store takes no literal', '        STW     =1', 'STW takes no literal'),
            ('a jump takes no literal', '        JMP     =1', 'JMP takes no literal'),
            ('a byte reference takes no literal', '        LDB     =1', 'LDB takes no literal'),
            ('$ in a literal', '        LDW     =$+1', 'cannot hold $'),
            ('an empty literal', '        LDW     =', 'needs an expression'),
            ('LPOOL with an operand', '        LPOOL   3', 'LPOOL takes no operand'),
            ('an undefined symbol in a literal', '        LDW     =NOPE', "undefined symbol 'NOPE'")):
        r = build('START   HLT\n%s\n        END     START\n' % card)
        check(name, any(want in e for e in errors_of(r)), errors_of(r))


def lint():
    def warned(name, deck, want, relocatable=False):
        r = build(deck, relocatable)
        got = warnings_of(r)
        if want is None:
            check(name, not got and not errors_of(r), got + errors_of(r))
        else:
            check(name, len(got) == 1 and want in got[0] and not errors_of(r), got + errors_of(r))

    frame = """
        ORG     X'40'
START   LDW     HERE
        LDX     REFS
%s
        HLT
HERE    WORD    0
REFS    WORD    FAR,FAR2
SUB     SUBR
        EXIT    SUB
        ORG     X'1000'
FAR     WORD    0
        ORG     X'2000'
FAR2    WORD    0
        END     START
"""
    # A skip passes over one word.
    warned('skip over a page selection', frame % '        SAZ\n        SMB     FAR\n        LDW     FAR',
           'SAZ skips this SMB')
    warned('skip over SML', frame % '        IXS     1\n        SML     4\n        LDW     FAR',
           'IXS skips this SML')
    warned('skip into an EXIT', frame % '        SEQ\n        EXIT    SUB', 'EXIT is 2 words and SEQ skips only the first')
    warned('skip into a multiply', frame % '        SAM\n        MPY     HERE', 'MPY is 2 words and SAM skips only the first')
    warned('skip into EXCH', frame % '        SLS\n        EXCH', 'EXCH is 2 words')
    warned('skip over one word', frame % '        SAZ\n        LDW     HERE\n        SMB     FAR\n        LDW     FAR', None)

    # A page selection governs the next reference whatever it names.
    warned('a selection and its own reference', frame % '        SMB     FAR\n        LDW     FAR', None)
    warned('a selection and a local cell', frame % '        SMB     FAR\n        LDW     HERE',
           'LDW HERE is in word page 0 and word page 2 is selected')
    warned('a selection and another page', frame % '        SMB     FAR\n        STW     FAR2',
           'STW FAR2 is in word page 4 and word page 2 is selected')
    warned('an indexed reference is not paged', frame % '        SMB     FAR\n        LDW     *3', None)

    module = """
START   LDW     HERE
%s
        HLT
HERE    WORD    0
        END     START
"""
    warned('a module: its own cell under a foreign page',
           module % "        SMB     X'30'\n        LDW     HERE", 'which is not where it is', relocatable=True)
    warned('a module: an absolute cell under a foreign page',
           module % "        SMB     X'30'\n        LDW     X'30'", None, relocatable=True)
    warned('a module: an absolute cell under the wrong page',
           module % "        SMB     X'30'\n        LDW     X'1030'", 'is in word page 2 and word page 0', relocatable=True)

    # A pool is data: nothing may run on into it.
    warned('a pool fallen into', """
START   LDW     =1
        LPOOL
        HLT
        END     START
""", 'a literal pool follows LDW')
    warned('a pool behind a skipped jump', """
START   LDW     =1
        SAZ
        JMP     START
        LPOOL
        HLT
        END     START
""", 'a literal pool follows JMP under SAZ')
    warned('a pool behind a jump', """
START   LDW     =1
        JMP     START
        LPOOL
        END     START
""", None)

    # A cell nothing names.
    warned('a cell never referred to', """
START   LDW     USED
        HLT
USED    WORD    1
DEAD    WORD    2
        END     START
""", 'DEAD is never referred to')
    warned('a code label never referred to is not a cell', """
START   LDW     USED
ALONE   HLT
USED    WORD    1
        END     START
""", None)


def command_line():
    """Warnings go to stderr and leave the status alone, unless asked."""
    deck = 'START   LDW     USED\n        HLT\nUSED    WORD    1\nDEAD    WORD    2\n        END     START\n'
    with tempfile.TemporaryDirectory() as d:
        src, out = os.path.join(d, 'w.asm'), os.path.join(d, 'w.bin')
        with open(src, 'w') as f:
            f.write(deck)
        cmd = [sys.executable, os.path.join(TOOLS, 'asm703.py'), src, '-o', out]
        r = subprocess.run(cmd, capture_output=True, text=True)
        check('a warning is printed and the image written',
              r.returncode == 0 and 'warning: DEAD is never referred to' in r.stderr and os.path.exists(out),
              (r.returncode, r.stderr))
        os.unlink(out)
        r = subprocess.run(cmd + ['--werror'], capture_output=True, text=True)
        check('--werror', r.returncode == 1 and not os.path.exists(out), (r.returncode, r.stderr))


def main():
    literals()
    lint()
    command_line()
    if FAILED:
        print('FAIL: asm703 test (%d: %s)' % (len(FAILED), ', '.join(FAILED)))
        return 1
    print('PASS: asm703 test')
    return 0


if __name__ == '__main__':
    sys.exit(main())
