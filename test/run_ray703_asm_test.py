#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""tools/asm703.py held to itself: its literals, its macros and local
labels, its warnings, the entries it writes for another deck, the library
parts it takes or leaves, and a module's segments.

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
mistake and not from its neighbour that does not. And the entries: a deck
that begins with what --exports wrote has to come out as the same deck
with the numbers written in. And a library: a deck over one has to come
out as the deck with only the parts it uses written into it. And a module
of several segments: placed by the reference loader, it has to be the
same deck assembled at the addresses its segments were placed at.
"""

import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.join(os.path.dirname(HERE), 'tools')
sys.path.insert(0, TOOLS)

import asm703  # noqa: E402
import reload703  # noqa: E402

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
    return ok


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
    # What a literal names may be defined below it: by the pool it is
    # known, and its cell is its value's.
    same('a literal met before its value is known shares that value\'s cell', """
START   LDW     =FIVE
        ADD     =5
        SUB     =TABLE
        LDX     =TABLE+ZERO
        HLT
TABLE   WORD    0
FIVE    EQU     5
ZERO    EQU     0
        END     START
""", """
START   LDW     L1
        ADD     L1
        SUB     L2
        LDX     L2
        HLT
TABLE   WORD    0
L1      WORD    5
L2      WORD    TABLE
        END     START
""")
    same('one defined below its pool is a cell to each spelling', """
START   LDW     =LATE
        ADD     =LATE
        SUB     =LATE+0
        HLT
        LPOOL
LATE    EQU     5
        END     START
""", """
START   LDW     L1
        ADD     L1
        SUB     L2
        HLT
L1      WORD    5
L2      WORD    5
        END     START
""")
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

    # A local label is in no symbol table, and the listing's has it under
    # its routine's name; a macro's own is in neither.
    r = build(MACROS + 'START   LDW     SAY\n.A      JMP     .A\nSAY     MSG     "HI"\n        END     START\n')
    check('local labels for the listing', not isinstance(r, asm703.AsmErrorList)
          and r['scoped'] == {'START.A': 1} and set(r['symbols']) == {'START', 'SAY'},
          getattr(r, 'errors', None) or (r['scoped'], sorted(r['symbols'])))

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

    # An EXIT reads its return slot in the page it runs in.
    far_exit = """
START   JSX     SUB
        HLT
SUB     SUBR
        SMB     ON
        JMP     ON
        ORG     X'1000'
ON      NOP
%s        EXIT    SUB
        END     START
"""
    warned('an EXIT in another page than its slot', far_exit % '',
           'EXIT SUB reads its return slot in word page 2, and the slot is in word page 0')
    warned('an EXIT behind a selection of its slot\'s page', far_exit % '        SMB     SUB\n', None)
    warned('an EXIT behind a selection of another page', far_exit % '        SMB     ON\n',
           'EXIT SUB reads its return slot in word page 2')

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


KERNEL = """; a kernel

; The vector.
;
; Called with SMB and JSX.
                NTRY    K.GET,K.PUT
                NTRY    CELL
                ORG     X'10'
; what only the kernel wants to know
K.GET           JMP     GET             ; ACR = a queue -> the next word
                                        ; or minus one
K.PUT           JMP     PUT
; and so is this, of the kernel only

; A node.
                NTRY    T.STA,T.LEN,T.TWO,E.NOF,T.BACK,A.LONG.NAME.OF.20
CELL            TEXT    "A;"            ; a text
T.STA           EQU     0               ; state
T.OWN           EQU     1               ; the kernel's own
T.LEN           EQU     18
T.TWO           EQU     T.LEN*2
E.NOF           EQU     X'FFFF'
T.BACK          EQU     0-1
A.LONG.NAME.OF.20 EQU   T.LEN+2
GET             HLT
PUT             HLT
                END
"""

# A module's deck over the kernel's entries, and the same with the
# numbers written in.
USER = """START   SMB     K.GET
        JSX     K.GET
        SMB     CELL
        LDW     CELL
        LDW     *T.LEN-1
        ADD     =T.TWO
        CMW     =E.NOF
        LDW     *T.BACK+1
        LDW     *A.LONG.NAME.OF.20
        HLT
        END     START
"""
USER_BY_HAND = """START   SMB     X'10'
        JSX     X'10'
        SMB     X'12'
        LDW     X'12'
        LDW     *17
        ADD     =36
        CMW     =X'FFFF'
        LDW     *0
        LDW     *20
        HLT
        END     START
"""


def entries():
    """NTRY and --exports: a deck's names, written out for the next deck."""
    asm = [sys.executable, os.path.join(TOOLS, 'asm703.py'), '--werror']
    with tempfile.TemporaryDirectory() as d:
        def put(name, text):
            path = os.path.join(d, name)
            with open(path, 'w') as f:
                f.write(text)
            return path

        def run(*args):
            return subprocess.run(asm + list(args), capture_output=True, text=True)

        kernel, api = put('k.asm', KERNEL), os.path.join(d, 'api.asm')
        r = run(kernel, '-o', os.path.join(d, 'k.bin'), '--exports', api)
        # CELL is a cell nothing in the kernel reads: being an entry is
        # being referred to, or --werror would have stopped this.
        if not check('a deck with entries assembles and writes them',
                     r.returncode == 0 and os.path.exists(api), (r.returncode, r.stderr)):
            return
        with open(api) as f:
            text = f.read()
        names = [line.split()[0] for line in text.split('\n')
                 if line and not line[0].isspace() and not line.startswith(';')]
        check('the entries are the names listed, in the cards\' order',
              names == ['K.GET', 'K.PUT', 'CELL', 'T.STA', 'T.LEN', 'T.TWO', 'E.NOF', 'T.BACK',
                        'A.LONG.NAME.OF.20'], names)
        for want, what in (
                ("K.GET           EQU     X'0010'         ; ACR = a queue -> the next word\n"
                 "                                        ; or minus one\n"
                 "K.PUT           EQU     X'0011'\n",
                 'a label is its address, with its card\'s comment and the indented ones under it'),
                ('\n; The vector.\n;\n; Called with SMB and JSX.\nK.GET ',
                 'the comment over an NTRY card goes over its names'),
                ("CELL            EQU     X'0012'         ; a text\n\n; A node.\nT.STA           EQU     0  ",
                 'a second NTRY card runs on, and a semicolon in a string begins no comment'),
                ("T.LEN           EQU     18\nT.TWO           EQU     X'0024'\nE.NOF           EQU     X'FFFF'\n",
                 'a number is said as the card says it, and an expression as its value'),
                ("T.BACK          EQU     -1\nA.LONG.NAME.OF.20 EQU     X'0014'\n",
                 'a value below zero is said as it is, and a long name stands clear of its EQU')):
            check(what, want in text, text)
        check('a comment in column 1 that is not over an NTRY card stays behind',
              'kernel only' not in text and 'only the kernel' not in text and 'a kernel' not in text, text)

        # The entries are worth what they say: a deck over them is the
        # deck with the numbers written in.
        outs = []
        for deck in ([api, put('u.asm', USER)], [put('h.asm', USER_BY_HAND)]):
            m = os.path.join(d, 'm%d.map' % len(outs))
            r = run(*deck, '--org', '0x100', '-m', m)
            check('a deck over the entries assembles', r.returncode == 0, r.stderr)
            outs.append(open(m).read() if os.path.exists(m) else None)
        check('a deck over the entries is the deck with the numbers written in',
              outs[0] is not None and outs[0] == outs[1], outs)

        r = run(put('n.asm', 'START   HLT\n        END     START\n'), '--exports', api + '2')
        check('--exports of a deck with no NTRY card fails',
              r.returncode == 1 and 'no NTRY card' in r.stderr and not os.path.exists(api + '2'),
              (r.returncode, r.stderr))

    def refused(name, deck, want, relocatable=False):
        got = errors_of(build(deck, relocatable))
        check(name, len(got) == 1 and want in got[0], got)

    frame = 'T.X     EQU     4\n%sGO      HLT\n.L      JMP     .L\n        END     GO\n'
    refused('an entry nothing defines', frame % '        NTRY    NOPE\n', "undefined symbol 'NOPE'")
    refused('an entry listed twice', frame % '        NTRY    T.X\n        NTRY    GO,T.X\n',
            'T.X is an entry twice')
    refused('an entry is a name', frame % '        NTRY    T.X+1\n', "'T.X+1' is not one")
    refused('a local label is no entry', 'T.X     EQU     4\nGO      HLT\n.L      JMP     .L\n'
            '        NTRY    .L\n        END     GO\n', 'is a local label')
    refused('a module\'s location is no entry', frame % '        NTRY    GO\n',
            'GO is a location in a module', relocatable=True)
    refused('a reservation of fewer than no words',
            '        ORG     10\nGO      JMP     GO\n        RES     5-$\n        END     GO\n',
            'RES -6')
    r = build(frame % '        NTRY    T.X\n', relocatable=True)
    check('a module\'s constant is an entry', not errors_of(r) and not isinstance(r, asm703.AsmErrorList)
          and [n for n, *_ in r['entries'][0][1]] == ['T.X'], errors_of(r))


# A library of five parts. SHOW needs PUT, which needs the cell LAST;
# SHOW comes first, so the part it needs is one further down. TWICE
# nothing below uses, and its macro, its EQU and its literal go with it.
LIBRARY = """
        LIBR    SHOW
SHOW    SUBR
        LDW     =WIDE
.LOOP   JSX     PUT
        SUB     =1
        SAZ
        JMP     .LOOP
        EXIT    SHOW
WIDE    EQU     3

        LIBR    TWICE,HALF
DOUBLE  MACRO
        ADD     =77
        ENDM
HALF    EQU     X'4000'
TWICE   SUBR
        DOUBLE
.LOOP   JMP     .LOOP
        EXIT    TWICE
SPARE   WORD    0

        LIBR    PUT
PUT     SUBR
        STW     LAST
        EXIT    PUT

        LIBR    LAST
LAST    WORD    0

        LIBR    NEVER
NEVER   WORD    LAST
"""

LIBRARY_USED = """
SHOW    SUBR
        LDW     =WIDE
.LOOP   JSX     PUT
        SUB     =1
        SAZ
        JMP     .LOOP
        EXIT    SHOW
WIDE    EQU     3
PUT     SUBR
        STW     LAST
        EXIT    PUT
LAST    WORD    0
"""

LIBRARY_MAIN = """
START   JSX     SHOW
        HLT
        END     START
"""


def library():
    """LIBR: the parts of a library a deck refers to, and no others."""
    def deck(*texts, relocatable=False):
        """Assemble several files as one deck."""
        paths = []
        for text in texts:
            with tempfile.NamedTemporaryFile('w', suffix='.asm', delete=False) as f:
                f.write(text)
            paths.append(f.name)
        try:
            return asm703._assemble(paths, relocatable=relocatable)
        except asm703.AsmErrorList as e:
            return e
        finally:
            for path in paths:
                os.unlink(path)

    for relocatable in (False, True):
        mode = ' (a module)' if relocatable else ''
        a = deck(LIBRARY, LIBRARY_MAIN, relocatable=relocatable)
        b = deck(LIBRARY_USED, LIBRARY_MAIN, relocatable=relocatable)
        if isinstance(a, asm703.AsmErrorList) or isinstance(b, asm703.AsmErrorList):
            check('a deck over a library assembles' + mode, False, (errors_of(a), errors_of(b)))
            return
        check('a deck over a library is the deck with the parts it uses' + mode,
              words(a) == words(b) and a['kinds'] == b['kinds'],
              ' '.join('%04X' % w for w in words(a)))
        check('a part\'s names are the deck\'s only if it is taken' + mode,
              {'SHOW', 'PUT', 'LAST', 'WIDE'} <= set(a['symbols'])
              and not {'TWICE', 'HALF', 'SPARE', 'NEVER'} & set(a['symbols']), sorted(a['symbols']))
        check('a part that is taken warns like any cards, and one that is not is silent' + mode,
              not warnings_of(a), warnings_of(a))
    bare = [text for addr, ws, text in a['listing'] if not ws]
    check('the listing has a part that was left as its cards, with no words',
          any('TWICE   SUBR' in text for text in bare) and any('NEVER   WORD' in text for text in bare)
          and not any('PUT     SUBR' in text for text in bare), bare)

    # What is taken is assembled like any cards: TWICE's cell SPARE is one
    # nothing refers to, once something refers to TWICE.
    r = deck(LIBRARY, 'START   JSX     TWICE\n        HLT\n        END     START\n')
    check('a taken part\'s idle cell is warned of',
          not errors_of(r) and warnings_of(r) == ['SPARE is never referred to'],
          (errors_of(r), warnings_of(r)))
    check('a taken part\'s macro and EQU are the deck\'s',
          not errors_of(r) and r['symbols'].get('HALF') == 0x4000 and 77 in words(r), errors_of(r))

    # A routine the deck writes under a library's name is the one it gets:
    # nothing is missing, so the part is never asked for.
    own = 'PUT     SUBR\n        HLT\n        EXIT    PUT\n'
    a = deck(LIBRARY, own + 'START   JSX     PUT\n        HLT\n        END     START\n')
    b = deck(own + 'START   JSX     PUT\n        HLT\n        END     START\n')
    check('a deck\'s own routine stands in for the library\'s',
          not errors_of(a) and not errors_of(b) and words(a) == words(b), (errors_of(a), errors_of(b)))

    # In one file: a LIBR card with no names closes the part above it.
    same('a bare LIBR closes a part', """
        LIBR    ONE
ONE     WORD    1
        LIBR    TWO
TWO     WORD    2
        LIBR
START   LDW     TWO
        HLT
        END     START
""", """
TWO     WORD    2
START   LDW     TWO
        HLT
        END     START
""")

    def refused(name, text, want, count=1):
        got = errors_of(build(text))
        check(name, len(got) == count and want in got[0], got)

    tail = '        LIBR\nGO      HLT\n        END     GO\n'
    refused('a label of a part its card does not list',
            '        LIBR    A\nA       HLT\nB       HLT\n        LIBR\nGO      JMP     B\n        END     GO\n',
            "undefined symbol 'B': ")
    got = errors_of(build('        LIBR    A\nA       HLT\nB       HLT\n        LIBR\n'
                          'GO      JMP     B\n        END     GO\n'))
    check('...and the error says which card', got and got[0].endswith('has it under LIBR A, which does not list it'), got)
    refused('a name nothing defines, parts or no', '        LIBR    A\nA       HLT\n        LIBR\n'
            'GO      JMP     NOPE\n        END     GO\n', "undefined symbol 'NOPE'")
    refused('a name on a card whose part does not define it', '        LIBR    A\nB       HLT\n        LIBR\n'
            'GO      JMP     A\n        END     GO\n', "undefined symbol 'A'")
    refused('a name on two cards', '        LIBR    A\nA       HLT\n        LIBR    B,A\nB       HLT\n' + tail,
            'A is on two LIBR cards')
    refused('a LIBR card with a label', 'X       LIBR    A\nA       HLT\n' + tail, 'LIBR takes no label')
    refused('a LIBR card of something not a name', '        LIBR    A+1\nA       HLT\n' + tail, "'A+1' is not one")
    refused('a local label on a LIBR card', '        LIBR    .A\nA       HLT\n' + tail, "'.A' is not one")
    refused('a LIBR card under a guard', 'C       EQU     1\n        TRUE    C=1\n        LIBR    A\nA       HLT\n'
            '        ENDC\n' + tail, 'LIBR is inside a TRUE or FALS')
    refused('a part that leaves a guard open', 'C       EQU     1\nGO      HLT\n        LIBR    A\n'
            '        TRUE    C=1\nA       HLT\n', 'the end of this part is inside a TRUE or FALS')
    refused('a LIBR card in a macro', 'M       MACRO\n        LIBR    A\n        ENDM\nA       HLT\n' + tail,
            'LIBR is inside a MACRO')
    refused('a LIBR card a macro makes', 'M       MACRO   OP\n        OP      A\n        ENDM\nA       HLT\n'
            'GO      HLT\n        M       LIBR\n        END     GO\n', 'LIBR from a macro')
    # A part's local labels are under its own labels: the first card of
    # one is under none, whatever stands above the LIBR card.
    refused('a local label does not reach across a LIBR card',
            'GO      JMP     .IN\n        LIBR    A\n.IN     HLT\nA       JMP     GO\n        LIBR\n'
            '        WORD    A\n        END     GO\n', "undefined symbol '.IN' under GO")


# A module of two segments with every kind of word that crosses from one
# to the other: a call and a cell behind page selections, a word and a
# byte address in data, literals, the two-word instructions' operand, `$`,
# and the entry in the second.
SEGMENTS = """
FIRST   SUBR
        ADD     =1
        SMB     CELL2
        ADD     CELL2
        LDX     =TEXT2*2
        LDB     *0
        EXIT    FIRST
CELL1   WORD    7
TABLE1  WORD    SECOND,TEXT2*2,CELL1,$
        SEG
SECOND  LDW     =5
        SMB     FIRST
        JSX     FIRST
        SMB     CELL1
        STW     CELL1
        MPY     CELL2
        MPY     CELL1
        DIV     =3
        LDX     =TABLE1
        LDW     TABLE2
        HLT
CELL2   WORD    3
TEXT2   TEXT    "AB"
TABLE2  WORD    FIRST,SECOND,CELL1*2,$,TEXT2-CELL2
        END     SECOND
"""


def segments():
    """SEG: a module in several streams, each placed by itself."""
    def placed(name, deck, bases):
        obj = build(deck, relocatable=True)
        absolute = build(deck, origin=bases)
        for r in (obj, absolute):
            if isinstance(r, asm703.AsmErrorList):
                check(name, False, errors_of(r))
                return None
        core, entry, sizes, byte_pages = reload703.load(asm703.object_stream(obj), bases)
        check(name, core == absolute['core'],
              sorted((hex(a), hex(core.get(a, -1)), hex(absolute['core'].get(a, -1)))
                     for a in set(core) | set(absolute['core']) if core.get(a) != absolute['core'].get(a)))
        check(name + ' (no warnings)', not warnings_of(obj), warnings_of(obj))
        return obj, entry, sizes, byte_pages

    for bases in ([0x1100, 0x4200], [0x4780, 0x1040], [0x1000, 0x1010]):
        got = placed('a module of two segments, placed at %s' % ', '.join('%#x' % b for b in bases),
                     SEGMENTS, bases)
        if got is None:
            return
        obj, entry, sizes, byte_pages = got
        check('...and entered in its second segment', entry == bases[1], hex(entry))
    check('each segment has its size, its literals with it', sizes == [16, 24], sizes)
    check('a module of two segments says so', len(obj['segments']) == 2 and obj['size'] == 40, obj['segments'])

    # One segment makes a direct byte reference into itself, and is the
    # one that wants a byte page.
    got = placed('a direct byte reference in one segment', """
ONE     LDW     =1
        HLT
        SEG
TWO     LDB     BYTES
        SMB     ONE
        JMP     ONE
BYTES   TEXT    "AB"
        END     ONE
""", [0x1100, 0x4200])
    if got is not None:
        check('...asks a byte page of that segment alone', got[3] == [False, True], got[3])
    # A selection says no more than which segment, so the segment a direct
    # byte reference is made into is the one that has to be in one byte
    # page, wherever the reference is made from.
    got = placed('a direct byte reference into another segment', """
ONE     SMB     BYTES
        LDB     BYTES+1
        HLT
        SEG
TWO     HLT
BYTES   TEXT    "ABCD"
        END     ONE
""", [0x1100, 0x4200])
    if got is not None:
        check('...asks a byte page of the segment it reaches', got[3] == [False, True], got[3])

    # A module of one segment is the text it always was: neither of the
    # codes that name a segment is in it.
    one = build('GO      LDW     =GO\n        HLT\n        END     GO\n', relocatable=True)
    codes = []
    reload703.load(asm703.object_stream(one), [0x1000], trace=codes)
    check('a module of one segment names none', not {0x0c, 0x0d} & {code for code, _ in codes}, codes)

    # Text the reference loader is given out of order is refused in its
    # own words, whatever the code that came too early.
    def record(text):
        body = bytes([0]) + bytes(text).ljust(asm703.RECORD_DATA, b'\0')
        return body + bytes([((sum(body) >> 8) + sum(body)) & 0xff])

    for what, text, want in (('an END', [0x06, 0, 0], 'no SIZE code'),
                             ('a word', [0x80, 0x80, 0x00], 'text before the SIZE code'),
                             ('a page selection', [0x03, 0, 0], 'text before the SIZE code'),
                             ('a segment', [0x0c, 0, 0], 'segment 0 has no SIZE code')):
        try:
            reload703.load(record(text), [0x1000])
            got = 'loaded'
        except reload703.LoadError as e:
            got = str(e)
        except Exception as e:  # noqa: BLE001
            got = repr(e)
        check('%s in front of the SIZE code is refused' % what, want in got, got)

    def refused(name, deck, want, relocatable=True, origin=0):
        got = errors_of(build(deck, relocatable, origin))
        check(name, len(got) == 1 and want in got[0], got)

    def warned(name, deck, want):
        r = build(deck, relocatable=True)
        got = warnings_of(r)
        check(name, not errors_of(r) and len(got) == 1 and want in got[0], (errors_of(r), got))

    two = 'ONE     HLT\nCELL    WORD    1\n        SEG\nTWO     %s\n        HLT\nMINE    WORD    CELL\n        END     ONE\n'
    refused('a reference into another segment with no page selected', two % 'LDW     CELL',
            'CELL is in another segment of the module')
    refused('...which is a number once the two are placed', 'ONE     HLT\nCELL    WORD    1\n        SEG\n'
            'TWO     LDW     CELL\n        HLT\n', 'is in word page', relocatable=False, origin=[0x1000, 0x1800])
    refused('the distance between two segments', two % 'LDW     =MINE-CELL',
            'puts locations of two segments together')
    refused('a SEG card with a label', 'ONE     HLT\nX       SEG\nTWO     HLT\n        END     ONE\n',
            'SEG takes no label')
    refused('a segment with nothing in it', 'ONE     HLT\n        SEG\n        END     ONE\n',
            'segment 1 of the module is empty')
    refused('a segment over a page', 'ONE     HLT\n        SEG\nTWO     HLT\n        RES     2048\n'
            '        END     ONE\n', 'segment 1 of the module is 2049 words')
    refused('a SEG card in a deck with one base', 'ONE     HLT\n        SEG\nTWO     HLT\n',
            'each wants a base', relocatable=False)
    warned('a selection of the wrong segment', 'ONE     HLT\nCELL    WORD    1\n        SEG\n'
           'TWO     SMB     MINE\n        LDW     CELL\n        HLT\nMINE    WORD    CELL\n        END     ONE\n',
           "LDW CELL is read under the selection of the module's own page")
    warned('a selection of another segment over a reference to this one', 'ONE     HLT\nCELL    WORD    1\n'
           '        SEG\nTWO     SMB     CELL\n        LDW     MINE\n        HLT\nMINE    WORD    CELL\n'
           '        END     ONE\n', "LDW MINE is read under the selection of another segment's page")
    warned('an EXIT from another segment than its subroutine', 'ONE     SUBR\n        HLT\n        SEG\n'
           'TWO     JSX     $+1\n        EXIT    ONE\n        END     TWO\n',
           'EXIT ONE reads its return slot in a page that is not')
    r = build('ONE     SUBR\n        HLT\n        SEG\nTWO     SMB     ONE\n        EXIT    ONE\n'
              '        END     TWO\n', relocatable=True)
    check('...unless its page is selected', not errors_of(r) and not warnings_of(r), (errors_of(r), warnings_of(r)))


def main():
    literals()
    forward_literals()
    macros()
    lint()
    command_line()
    entries()
    library()
    segments()
    if FAILED:
        print('FAIL: asm703 test (%d: %s)' % (len(FAILED), ', '.join(FAILED)))
        return 1
    print('PASS: asm703 test')
    return 0


if __name__ == '__main__':
    sys.exit(main())
