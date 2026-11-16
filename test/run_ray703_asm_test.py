#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""tools/asm703.py held to itself: its literals, its macros and local
labels, and its warnings.

The images the tree builds say the assembler still assembles what it did;
this says what the parts no image exercises do. Each case is a small deck
written out and assembled through the same entry the command line uses.

A literal is checked the strong way: a deck written with literals has to
come out word for word, and in a module relocation code for relocation
code, as the same deck written with the cells laid down by hand where the
pool goes. The rest are the rules -- which literals share a cell, where a
pool may be and what happens with none, what may not take one. Macros and
local labels get the same treatment: the deck written with them against
the deck written out. Then the warnings, each from a deck that has the
mistake and not from its neighbour that does not.
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


FORWARD = """
START   LDW     %s
        ADD     %s
        HLT
TABLE   WORD    0
%s        END     START
"""


def forward_literals():
    # Two literals that name something further down are one cell when
    # they are spelled alike, and a space inside a string is spelling.
    same('forward literals that differ inside a string are two cells',
         FORWARD % ('=TABLE+LEN("A B")', '=TABLE+LEN("AB")', ''),
         FORWARD % ('L1', 'L2', 'L1      WORD    TABLE+3\nL2      WORD    TABLE+2\n'))
    same('forward literals that differ in spacing are one',
         FORWARD % ('=TABLE+1', '= TABLE + 1', ''),
         FORWARD % ('L1', 'L1', 'L1      WORD    TABLE+1\n'))


MACROS = """
JZ      MACRO   TARGET
        SAZ
        JMP     $+2
        JMP     TARGET
        ENDM
JNZ     MACRO   TARGET
        SAZ
        JMP     TARGET
        ENDM
CALL    MACRO   ROUTINE
        SMB     ROUTINE
        JSX     ROUTINE
        ENDM
MSG     MACRO   TEXT
        WORD    .T*2,.T*2+LEN(TEXT)
.T      STR     TEXT
        ENDM
"""


def macros():
    # Two routines with the same local names, calls that pass a local in,
    # a macro's own label at two calls, and a call with a label of its own.
    same('macros and local labels', MACROS + """
        ORG     X'40'
FIRST   LDW     COUNT
        JZ      .DONE
.LOOP   SUB     =1
        JNZ     .LOOP
.DONE   HLT
SECOND  LDW     COUNT
.LOOP   ADD     =1
        JZ      .DONE
        JMP     .LOOP
.DONE   CALL    FAR
        HLT
COUNT   WORD    3
ONE     MSG     "ODD"
TWO     MSG     "EVEN, 4\\r\\n"
        LPOOL
        ORG     X'1000'
FAR     SUBR
        EXIT    FAR
        END     FIRST
""", """
        ORG     X'40'
FIRST   LDW     COUNT
        SAZ
        JMP     $+2
        JMP     DONE1
LOOP1   SUB     K1
        SAZ
        JMP     LOOP1
DONE1   HLT
SECOND  LDW     COUNT
LOOP2   ADD     K1
        SAZ
        JMP     $+2
        JMP     DONE2
        JMP     LOOP2
DONE2   SMB     FAR
        JSX     FAR
        HLT
COUNT   WORD    3
ONE     WORD    T1*2,T1*2+3
T1      BYTE    'O'-128,'D'-128,'D'-128
TWO     WORD    T2*2,T2*2+9
T2      BYTE    'E'-128,'V'-128,'E'-128,'N'-128,X'2C',X'20',X'34',X'0D',X'0A'
K1      WORD    1
        ORG     X'1000'
FAR     SUBR
        EXIT    FAR
        END     FIRST
""")

    # One macro calling another, and an argument left out.
    same('a macro in a macro', MACROS + """
BOTH    MACRO   A,B
        JZ      A
        LDW     B
        ENDM
START   LDW     HERE
        BOTH    START,HERE
        HLT
HERE    WORD    0
        END     START
""", """
START   LDW     HERE
        SAZ
        JMP     $+2
        JMP     START
        LDW     HERE
        HLT
HERE    WORD    0
        END     START
""")

    r = build(MACROS + """
START   LDW     HERE
.L      JZ      .L
        HLT
HERE    WORD    0
SAY     MSG     "HI"
        END     START
""")
    if isinstance(r, asm703.AsmErrorList):
        check('a deck of calls and local labels', False, errors_of(r))
    else:
        check('no local label in the symbol table',
              sorted(r['symbols']) == ['HERE', 'SAY', 'START'], sorted(r['symbols']))
        check('a call is listed, and what it made under it',
              any(text.startswith('+') and 'JMP' in text and '.L' in text for _, _, text in r['listing'])
              and any('JZ      .L' in text and not ws for _, ws, text in r['listing']), r['listing'])
        check("a call's label is its first word", r['symbols']['SAY'] == r['symbols']['HERE'] + 1, r['symbols'])

    for name, deck, want in (
            ('a local label out of its scope',
             'START   HLT\n.A      HLT\nNEXT    JMP     .A\n        END     START\n',
             "undefined symbol '.A' under NEXT"),
            ('a local label twice in a scope',
             'START   HLT\n.A      HLT\n.A      HLT\n        END     START\n',
             "'.A' under START is defined twice"),
            ('too many arguments', MACROS + 'START   JZ      START,START\n        END     START\n',
             'JZ takes 1 argument, not 2'),
            ('ENDM alone', 'START   HLT\n        ENDM\n        END     START\n', 'ENDM with no MACRO'),
            ('MACRO unclosed', 'M       MACRO\nSTART   HLT\n        END     START\n', 'M has no ENDM'),
            ('a macro named for an instruction', 'LDW     MACRO\n        ENDM\nSTART   HLT\n        END     START\n',
             "LDW is the assembler's already"),
            ('a macro with no name', '        MACRO\n        ENDM\nSTART   HLT\n        END     START\n',
             'MACRO needs a name'),
            ('a macro calling itself', 'M       MACRO\n        M\n        ENDM\nSTART   M\n        END     START\n',
             'one of them calls itself'),
            ('an odd TEXT', 'START   HLT\n        TEXT    "ODD"\n        END     START\n', 'TEXT is 3 bytes')):
        r = build(deck)
        check(name, any(want in e for e in errors_of(r)), errors_of(r))

    # An error in a macro says where the card is and where the call was.
    r = build(MACROS + 'START   JZ      NOWHERE\n        END     START\n')
    said = [str(e) for e in getattr(r, 'errors', [])]
    check('an error under a call names the call', any('(in JZ at ' in s and "'NOWHERE'" in s for s in said), said)

    # A string's comma divides no operands, behind an escaped quote or not.
    r = build('START   LDW     T\n        HLT\nT       WORD    LEN("A\\",B"),7,\',\'\n        END     START\n')
    check('a comma in a string behind an escaped quote', not isinstance(r, asm703.AsmErrorList)
          and [r['core'][a] for a in (2, 3, 4)] == [4, 7, 0xac], getattr(r, 'errors', None) or words(r))

    # Conditional assembly comes first: the half not taken defines no
    # macro, calls none and is no local label's scope.
    same('a macro defined both ways is the one whose guard holds', """
WIDE    EQU     0
        TRUE    WIDE=0
PAD     MACRO
        NOP
        ENDM
        ENDC
        FALS    WIDE=0
PAD     MACRO
        HLT
        HLT
        ENDM
        ENDC
START   PAD
        HLT
        END     START
""", 'START   NOP\n        HLT\n        END     START\n')
    same('a label that is not assembled is no scope', """
WIDE    EQU     0
START   JMP     .ON
        TRUE    WIDE=1
OTHER   HLT
        ENDC
.ON     HLT
        END     START
""", 'START   JMP     ON\nON      HLT\n        END     START\n')
    same('a call that is not assembled is not looked at', MACROS + """
START   NOP
        TRUE    0=1
        JZ      START,START
        NOSUCH  1,2
        ENDC
        HLT
        END     START
""", 'START   NOP\n        HLT\n        END     START\n')
    same('a guard in a macro chooses among its cards', MACROS + """
PICK    MACRO   WHICH,WHERE
        TRUE    WHICH=1
        JNZ     WHERE
        ENDC
        FALS    WHICH=1
        NOP
        ENDC
        ENDM
START   PICK    1,START
        PICK    0,START
        HLT
        END     START
""", 'START   SAZ\n        JMP     START\n        NOP\n        HLT\n        END     START\n')

    # STR pads an odd string with a zero byte, and LEN is what was written.
    r = build('START   LDW     =LEN("ODD")\n        HLT\nS       STR     "ODD"\n        END     START\n')
    check('STR and LEN', not isinstance(r, asm703.AsmErrorList)
          and [r['core'][a] for a in (2, 3, 4)] == [0x4f44, 0x4400, 3], getattr(r, 'errors', None) or words(r))


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

    # A skip in front of a macro call passes over one word of it.
    warned('skip into a macro of three words', MACROS + frame % '        SEQ\n        JZ      START',
           'JZ is 3 words here and SEQ skips only the first')
    warned('skip into a macro of one word',
           'ONE     MACRO\n        CLR\n        ENDM\n' + frame % '        SEQ\n        ONE', None)

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

    warned('a pool a macro\'s own jump lands in', MACROS + """
START   LDW     =1
        JZ      START
        LPOOL
        HLT
        END     START
""", 'JMP $+2 goes to 0x4, which is in the literal pool at 0x4')
    warned('a jump over a pool', """
START   LDW     =1
        JMP     ON
        LPOOL
ON      HLT
        END     START
""", None)

    # A selection stands until a memory reference uses it, whatever
    # generics come between.
    warned('a selection over a generic, and a reference in another page',
           frame % '        SMB     FAR\n        NOP\n        LDW     HERE',
           'LDW HERE is in word page 0 and word page 2 is selected')
    warned('a selection over generics, and a reference in its page',
           frame % '        SMB     FAR\n        NOP\n        CAX\n        LDW     FAR', None)
    warned('a selection is used up by one reference',
           frame % '        SMB     FAR\n        LDW     FAR\n        NOP\n        LDW     HERE', None)
    r = build(frame % '        SMB     FAR\n        SAZ\n        LDW     FAR')
    check('a skip ends a selection', any('would land in the wrong page' in e for e in errors_of(r)),
          errors_of(r) + warnings_of(r))

    # A macro's words end where it ends, not where an ORG behind it goes.
    warned('a one-word macro under a skip, an ORG behind it', """
ONE     MACRO
        NOP
        ENDM
START   SAZ
        ONE
        ORG     X'40'
        HLT
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
    forward_literals()
    macros()
    lint()
    command_line()
    if FAILED:
        print('FAIL: asm703 test (%d: %s)' % (len(FAILED), ', '.join(FAILED)))
        return 1
    print('PASS: asm703 test')
    return 0


if __name__ == '__main__':
    sys.exit(main())
