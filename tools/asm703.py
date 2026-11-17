#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""A small two-pass assembler for the Raytheon 703.

The 703 predates anything this tree could reuse, and its own assembler (SYM II)
exists only as scans, so the guests need something to build them. There is
no linking: a deck is assembled whole. It produces two things, and a third
for the decks that come after:

  - an absolute core image (`-o`): a flat binary of big-endian words starting
    at word 0, which is what `emu -s ray703` loads into core;
  - a relocatable module (`--object`): the deck assembled from word 0 and
    written as loader text in the record format of the 1968 relocating
    loader (DN 390682C), for an executive's loader to place wherever its
    allocator finds room. The codes, each a byte:

        8n  n+1 words whose 11-bit M field is a location  (W11)
        9n  n+1 words holding a word address              (W15)
        An  n+1 words holding a byte address              (B16)
        Bn  n+1 byte references whose M field is a location (B11)
        Cn  n+1 absolute words                            (A)
        03  a page selection of a location: one word, the address, and
            the loader builds the SML/SMU from where the module lands
        04  a reserved run: one word, the count
        06  the end, with one word naming the entry
        0A  the module's size in words, first of all; 0B the same for a
            module that byte-addresses directly and so must sit inside
            one 1024-word byte page rather than one 2048-word page

    The text is cut into records of a zero marker, ninety-two bytes and a
    checksum -- the byte sum folded, (sum >> 8) + sum -- which is one
    47-word disc sector each; the marker and the checksum are framing the
    loader strips, and the text runs on across them. What relocates is decided by the relocation count an expression
    carries (see Expr); a direct reference to an absolute address needs a
    page selection in front of it, since a module's own page is unknown
    until it is loaded, and a module must fit one page, since its M fields
    are page offsets. tools/reload703.py is the reference loader for this
    text, and rex/ has the one that runs on the machine.
  - an entry deck (`--exports`): the names the deck's NTRY cards list, as
    EQUs at the values they have here, for another deck to begin with.
    That is how what loads under an executive knows its vector and its
    cells without being linked to it:

                NTRY    K.ALLOC,K.FREE,CURT

    NTRY is the 1968 loader's word for a name a deck gives away. Each
    name goes out with the comment of the card that defines it and of
    the indented comment cards under that one, and each NTRY card with
    the comment cards that stand directly over it, so the entry deck
    reads as the interface's own description; a comment that begins in
    column 1 anywhere else stays behind. In a module a location has no
    value until it is loaded, so only its constants may be entries.

Several source files may be named at once and are assembled strictly in order
as one deck -- one location counter, one symbol table, one image -- exactly as
if they had been concatenated, except that errors name their file. The END
card belongs in the deck's last file. Two rules follow from the single stream:
an ORG whose operand names a symbol needs that symbol defined in an earlier
file (ORG evaluates as pass 1 reaches it), and a page selection (SML/SMU/SMB)
as a file's last emitting statement would license the next file's first
reference, so don't end a file on one.

Source format follows the period listings: a label starts in column 0, the
mnemonic is indented, and `;` begins a comment.

    ; a comment
            ORG     X'40'
    START   LDX     COUNT           ; direct
            LDB     */BUFFER        ; indexed, byte address
            DOT     14,14           ; device 14, function 14
            IXS     1
            JMP     START
    COUNT   WORD    -20
    BUFFER  TEXT    "HELLO\\r\\n"
    BUFEND  EQU     $

Operand prefixes, both from the PTB bootstrap listing:

    *EXPR   indexed -- sets the X bit
    /EXPR   the byte address of word EXPR, i.e. EXPR * 2

Numbers are decimal, `0x1F`, `X'1F'` or `'A'`; expressions take + - * and
parentheses, and `$` is the address of the word being assembled.

Four things here are this assembler's and no period listing's.

A literal is a constant written where it is used. The machine has no
instruction that carries a word, so every constant is a cell somewhere and
the instruction a reference to it; `=` in front of an operand has the
assembler make the cell:

            LDW     =X'7FFF'
            ADD     =1
            LDX     =TABLE          ; the address of TABLE
            ...
            LPOOL                   ; the cells go here, each once

An instruction that reads its operand may take one (LDW LDX ADD SUB ORI ORE
AND CMW MPY DIV). The literals met since the last LPOOL are laid down at
the next, and whatever is left over goes in front of the END. The reference
is an ordinary direct one, so a pool must be in the page of the
instructions that use it -- a deck of several pages wants an LPOOL in each,
and the assembler says which literal cannot reach its pool -- and where
execution does not run into it. The Literal class says when two literals
are one cell.

A label that begins with a dot is local to the last label above it that
does not, so every routine may have its own `.LOOP` and `.DONE`:

    TYPE    JSX     L.ARGS
    .LOOP   LDW     TCH
            ...
            JMP     .LOOP

and nothing outside the routine can name them. The listing's symbol table
has them as ROUTINE.NAME -- TYPE.LOOP -- for whoever reads it to say where
a branch goes.

A macro is a run of cards under a name, with the call's arguments put in
where its parameters stand (see Macro for the rules):

    JNZ     MACRO   TARGET          ; go there if ACR is not zero
            SAZ
            JMP     TARGET
            ENDM
            ...
            JNZ     .LOOP

A `.LABEL` in a macro's body is its own at each call. The listing shows a
call and then, marked `+`, what it generated. Two things exist for macros
to use: STR, which is TEXT that may be an odd number of bytes, the last
word's low half then zero; and LEN("..."), in any expression, the number
of bytes in a string.

And it warns. A warning is something the machine does exactly as written
and nobody means; it goes to stderr, and --werror makes it an error:

  - a skip in front of a page selection, or of an instruction two words
    long (EXIT, MPY, DIV, EXCH, a JSX with arguments), or of a macro call
    that generates more than one: a skip passes over one word, so the
    selection is skipped and its reference runs without it, or the
    instruction is entered at its second word;
  - a direct reference behind a page selection, to something that is not
    in the page selected: the selection governs the next memory reference
    whatever it names, the generics between the two notwithstanding;
  - an EXIT in another page than its subroutine's return slot, with no
    selection of that page in front of it: the slot is read where the
    EXIT runs;
  - a literal pool, or the cells before an END's, that execution can run
    on into or that a JMP names a word of;
  - a labelled cell that nothing in the deck refers to.
"""

from collections import ChainMap
import argparse
import itertools
import re
import sys
import textwrap

# Memory reference instructions: opcode in bits 0-3 (the manual numbers from
# the left), index flag in bit 4, an 11-bit address in bits 5-15.
MEMREF = {
    'JMP': 0x1, 'JSX': 0x2, 'STB': 0x3, 'CMB': 0x4,
    'LDB': 0x5, 'STX': 0x6, 'STW': 0x7, 'LDW': 0x8,
    'LDX': 0x9, 'ADD': 0xa, 'SUB': 0xb, 'ORI': 0xc,
    'ORE': 0xd, 'AND': 0xe, 'CMW': 0xf,
}

# The three that take a byte address rather than a word address.
BYTE_REF = {'STB', 'CMB', 'LDB'}

# Generics, opcode 0. Every table below holds the instruction with its operand
# field zeroed; the operand is ORed in.
GEN_NONE = {
    'HLT': 0x0000, 'SLM': 0x0040, 'SGM': 0x0050, 'CEX': 0x0060, 'CXE': 0x0070,
    'MSK': 0x00a0, 'UNM': 0x00b0,
    'CLR': 0x0100, 'CMP': 0x0110, 'INV': 0x0120, 'CAX': 0x0130, 'CXA': 0x0140,
}
GEN_LEVEL = {'INR': 0x0010, 'ENB': 0x0020, 'DSB': 0x0030}
GEN_PAGE = {'SML': 0x0080, 'SMU': 0x0090}

# SYM II's shorthand for "select whichever page holds this symbol", which
# saves the programmer working out by hand whether it needs SML or SMU. The
# X-RAY listing uses it wherever it reaches out of the current page.
SELECT_BASE = 'SMB'
GEN_LITERAL = {'IXS': 0x0400, 'DXS': 0x0500, 'LLB': 0x0600, 'CLB': 0x0700}
GEN_DIO = {'DIN': 0x0200, 'DOT': 0x0300}

# The optional multiply/divide hardware: the machine's only two-word
# instructions. Section 6 of the reference manual diagrams the encodings even
# though appendix B never lists the option -- a fixed first word, then a flat
# 15-bit word address that "allows either instruction to reference any
# location in memory without using the index register" (6-1). So no EXR page,
# no indexing, and none of the M-field checks that apply everywhere else.
GEN_TWOWORD = {'MPY': 0x0b0f, 'DIV': 0x0c0f}

SKIPS = ['SAZ', 'SAP', 'SAM', 'SAO', 'SLS', 'SXE', 'SEQ', 'SNE',
         'SGR', 'SLE', 'SNO', 'SSE', 'SS0', 'SS1', 'SS2', 'SS3']
GEN_SKIP = {name: 0x0800 | (i << 4) for i, name in enumerate(SKIPS)}

ARITH_SHIFTS = ['SRA', 'SLA', 'SRAD', 'SLAD']
LOGICAL_SHIFTS = ['SRL', 'SLL', 'SRLD', 'SLLD', 'SRC', 'SLC', 'SRCD', 'SLCD',
                  'SRLL', 'SLLL', 'SRLR', 'SLLR', 'SRCL', 'SLCL', 'SRCR', 'SLCR']
GEN_SHIFT = {name: 0x0900 | (i << 4) for i, name in enumerate(ARITH_SHIFTS)}
GEN_SHIFT.update({name: 0x0a00 | (i << 4) for i, name in enumerate(LOGICAL_SHIFTS)})

# `DATA` is SYM II's name for a word of data; `WORD` is ours. `TRUE`/`FALS`
# bracket conditionally assembled code and are closed by `ENDC` -- they are not
# an if/else pair, they are two independent guards, so the X-RAY listing writes
# out both halves of a choice as `TRUE c ... ENDC` followed by `FALS c ... ENDC`.
DIRECTIVES = {'ORG', 'ORIG', 'WORD', 'DATA', 'D', 'BYTE', 'TEXT', 'STR', 'RES', 'EQU',
              'TRUE', 'FALS', 'ENDC', 'END', 'LPOOL', 'MACRO', 'ENDM', 'NTRY'}

# The instructions that only read their operand, and so may be given a
# literal for one: `ADD =1` is a reference to a word holding 1, which the
# assembler lays down in the next literal pool (see Literal).
LITERAL_OPS = {'LDW', 'LDX', 'ADD', 'SUB', 'ORI', 'ORE', 'AND', 'CMW', 'MPY', 'DIV'}

# What skips: the sixteen conditional skips, and the index steps, which skip
# on the index's sign under either of their names.
SKIPPERS = set(SKIPS) | {'IXS', 'DXS', 'SXP', 'SXM'}

# Statements that are instructions and more than one word long. A skip in
# front of one passes over its first word only.
PAGE_SELECTS = {'SML', 'SMU', 'SMB'}
# The generics a page selection does not outlast although they refer to no
# memory: two leave for somewhere else, and CXE loads the register itself.
SELECTION_ENDS = {'HLT', 'INR', 'CXE'}
DATA_OPS = {'WORD', 'DATA', 'D', 'BYTE', 'TEXT', 'STR', 'RES', 'LPOOL'}

# Mnemonics found only in the period listings, never in the reference manual,
# because they are the assembler's rather than the machine's. Each is given
# here with the word it actually produced in the X-RAY listing.
#
# SXP is `IXS 0` and SXM is `DXS 0`: change the index by nothing and skip on
# its sign. The literal forms are loop steps; under these names they are what
# they are being used for, a test of the index.
#
# NOP is `SRA 0`, an arithmetic shift by nothing. X-RAY uses the mnemonic twice
# but only inside conditionals it did not assemble, so no object was ever
# printed beside it there; the relocating loader's card 1074 prints 0900.
GEN_ALIAS = {'SXP': 0x0400, 'SXM': 0x0500, 'NOP': 0x0900}

# Shift mnemonics that take a trailing D, L or R to name their double-length
# and single-byte variants. Appendix B of the manual, and the listings, print
# the suffix separated by a space.
SHIFT_STEMS = {'SRA', 'SLA', 'SRL', 'SLL', 'SRC', 'SLC'}

# The subroutine convention on a machine with no stack, as two macros that
# generate two words each. JSX leaves the return address in the index
# register, so a subroutine's first act is to put it somewhere:
#
#   FDA   SUBR            0000            a slot for the return address
#                         STX  slot       ...filled in on entry
#
# and its last act is to go back through it:
#
#         EXIT FDA        LDX  slot
#                         JSX  *0         indexed through the index register
#
# Note the label binds to the STX, not to the slot, so that `JSX FDA` from a
# caller lands on the instruction rather than on the data word in front of it.
# Both forms and both encodings are taken from the X-RAY listing, which prints
# FDA's prologue as 0000 / 60B4 at words 0B4 and 0B5, and its EXIT as 90B4 /
# 2800.
SUBR_MACROS = {'SUBR', 'EXIT'}

# EXCH swaps the accumulator and the index register, as two SLCD 8 in a row:
# the double-length circular shift rotates the 32-bit ACR:IXR pair, so two
# rotations of eight make sixteen, which lands each register in the other.
# There is no single instruction that does it -- the shift count is only four
# bits, so 16 cannot be encoded. Taken from the X-RAY listing, which prints
# 0A78 twice at words 13F and 140.
EXCH_WORD = 0x0a78

# Directives that are processed even inside a conditional that is not being
# assembled, because they are what opens and closes those conditionals.
CONDITIONALS = {'TRUE', 'FALS', 'ENDC'}

TEXT_ESCAPES = {'r': 0x0d, 'n': 0x0a, 't': 0x09, '0': 0x00, '\\': 0x5c, '"': 0x22}

# A quoted constant in an operand holds one or two characters, and on this
# machine characters carry bit 7: the X-RAY listing assembles
# `DATA 'XR','AY'` (card 363) to D8D2 C1D9, which is ASCII with the high bit
# set. Pack them left to right, so the value of a literal is simply the integer
# its bytes spell. The one-character case has no example in the listing, but
# right justification is forced by `LLB`, whose literal is eight bits wide --
# a blank-filled left justification could not be loaded by it at all.
CHAR_HIGH_BIT = 0x80

# Every identifier in an expression, used to decide whether an EQU inherits its
# operand's location-ness.
NAMES = re.compile(r'[A-Za-z][A-Za-z0-9_.]*')


class Loc:
    """A source position that knows its file.

    A multi-file deck reports errors as `path line N`; `seq` is the card's
    position in the whole deck, so a mixed pile of errors sorts back into the
    order the cards were read. Plain integers still work everywhere a Loc
    does (xraylist.py hands encode() bare card numbers) and print as `line N`.
    """
    __slots__ = ('path', 'line', 'seq', 'via', 'exps')

    def __init__(self, path, line, seq, via=None, exps=()):
        self.path = path
        self.line = line
        self.seq = seq
        # For a card a macro generated: the call it came from, as words for
        # a message, and the expansions it is inside, outermost first.
        self.via = via
        self.exps = exps

    def __str__(self):
        where = f'{self.path} line {self.line}'
        return f'{where} (in {self.via})' if self.via else where


class AsmError(Exception):
    def __init__(self, lineno, message):
        where = lineno if isinstance(lineno, Loc) else f'line {lineno}'
        super().__init__(f'{where}: {message}')
        self.lineno = lineno


class AsmWarning:
    """Something that assembles and is almost certainly not what was meant."""
    def __init__(self, lineno, message):
        self.lineno = lineno
        self.message = message

    def __str__(self):
        where = self.lineno if isinstance(self.lineno, Loc) else f'line {self.lineno}'
        return f'{where}: warning: {self.message}'


class Literal:
    """A constant written where it is used: `LDW =X'7FFF'`, `ADD =1`,
    `LDX =TABLE`, `CMW =BUF*2`.

    The machine has no instruction that carries a word, so a constant is a
    cell and the instruction a reference to it. A literal is that cell made
    by the assembler: the ones met since the last pool are laid down, each
    once, where an LPOOL card says, and whatever is left when the deck ends
    goes in front of its END. The reference is an ordinary direct one, so
    the pool has to be in the page of everything that uses it -- a deck of
    several pages wants an LPOOL in each -- and somewhere execution does
    not run into it.

    Two literals are one cell when they are the same word: the same value
    and the same relocation, as far as that is known where their pool is
    laid down -- `=1`, `=X'1'` and `=ONE` share, wherever above the pool
    `ONE EQU 1` stands. One that names a symbol defined below its pool is
    known only by its spelling, and shares with others spelled the same.
    `$` has no place in one: the cell is not where the instruction is.
    """
    __slots__ = ('name', 'body', 'lineno')

    def __init__(self, name, body, lineno):
        self.name = name        # the symbol the pool slot is given
        self.body = body        # the expression, as written
        self.lineno = lineno    # where it was first asked for


# What a literal's slot is called in the symbol table while the deck is
# assembled: a name no card may use for a label, taken out again before
# anyone sees the table.
LITERAL_PREFIX = '_$LIT.'


class UndefinedSymbol(AsmError):
    """Raised on its own so that EQU can retry a forward reference."""


# A label that begins with a dot is local: it belongs to the last label
# above it that does not, and two routines may each have their `.LOOP`.
# Before anything else looks at the deck each one is rewritten into a name
# of the whole deck's -- the scope, this, and the label less its dot -- so
# the two passes know nothing about them. A macro's own are scoped to the
# one expansion. They are taken out of the symbol table again at the end.
LOCAL_MARK = '$$'
TOP_SCOPE = '_$TOP'
MACRO_SCOPE = '_$M'


LOCALIZED = re.compile(r'[A-Za-z0-9_$.]+?\$\$(?=[A-Za-z0-9_])')


def shown(name):
    """A symbol as a message should name it: a local by what was written."""
    if LOCAL_MARK not in name:
        return repr(name)
    scope, local = name.split(LOCAL_MARK, 1)
    if scope.startswith(MACRO_SCOPE):
        return f"'.{local}' of the macro"
    if scope == TOP_SCOPE:
        return f"'.{local}', above the deck's first label,"
    return f"'.{local}' under {scope}"


def rewrite(text, name_fn):
    """`text` with every name in it passed through `name_fn`, which returns
    what to put in its place. Quoted text is left alone -- a string, a
    character constant, the digits of X'1F' -- and so is a number."""
    out = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c in '\'"':
            j = i + 1
            while j < n and text[j] != c:
                j += 2 if text[j] == '\\' else 1
            out.append(text[i:j + 1])
            i = j + 1
        elif c.isdigit():
            j = i
            while j < n and (text[j].isalnum() or text[j] == '_'):
                j += 1
            out.append(text[i:j])
            i = j
        elif c.isalpha() or c in '_$.':
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] in '_$.'):
                j += 1
            name = text[i:j]
            # X'1F': the X is the constant's, not a name
            quoted = j < n and text[j] == "'" and name in ('X', 'x')
            out.append(name if quoted else name_fn(name))
            i = j
        else:
            out.append(c)
            i += 1
    return ''.join(out)


def localize(text, scope):
    """The local labels in an operand, as the deck-wide names they stand for."""
    if text is None or '.' not in text:
        return text
    return rewrite(text, lambda name: f'{scope}{LOCAL_MARK}{name[1:]}'
                   if name.startswith('.') and len(name) > 1 else name)


CHAR = re.compile(r"'(?:\\.|[^'])(?:\\.|[^'])?'")


def commas(text):
    """Where the commas that divide a list of operands are: the ones
    outside a string, a character constant and an X'..' number.

    A string's quote may be escaped. A character constant is one or two
    characters, either of which may be -- so `'\\'` alone is a backslash
    and `','` a comma.
    """
    found, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == '\\' else 1
        elif c == "'":
            m = None if text[i - 1:i] in ('X', 'x') else CHAR.match(text, i)
            if m:
                i = m.end() - 1
            else:
                i = text.find("'", i + 1)
                if i < 0:
                    break
        elif c == ',':
            found.append(i)
        i += 1
    return found


def split_args(text):
    """A macro call's arguments: the commas outside quotes divide them."""
    cuts = [-1] + commas(text) + [len(text)]
    parts = [text[a + 1:b].strip() for a, b in zip(cuts, cuts[1:])]
    return parts if any(parts) else []


class Macro:
    """A run of cards under a name, with holes for the call's arguments:

        JZ      MACRO   TARGET          ; go there if ACR is zero
                SAZ
                JMP     $+2
                JMP     TARGET
                ENDM
        CALL    MACRO   ROUTINE         ; a subroutine in another page
                SMB     ROUTINE
                JSX     ROUTINE
                ENDM
        MSG     MACRO   TEXT            ; a text and the two byte addresses
                WORD    .T*2,.T*2+LEN(TEXT) ; that say where it is
        .T      STR     TEXT
                ENDM

    A card whose mnemonic is the name is replaced by the body, each
    parameter's name replaced by the call's argument wherever it stands as
    a name -- in a label, a mnemonic or an operand, never inside quotes.
    A missing argument is nothing. The call's label is the first word
    generated. A body may call other macros, and a `.LABEL` in it is its
    own at every call; one the call passes in is the caller's. A macro is
    defined before it is used, by cards the deck reads before the call --
    and assembles: a definition in the half of a TRUE or FALS that is not
    taken defines nothing, so a guard may choose between two of one name.
    """
    __slots__ = ('name', 'params', 'body', 'loc')

    def __init__(self, name, params, loc):
        self.name = name
        self.params = params
        self.body = []
        self.loc = loc


MACRO_DEPTH = 32


def expand(statements, errors, skipping):
    """The deck with its macro definitions taken out, its macro calls
    replaced by what they generate, and every local label given its
    deck-wide name: what the two passes assemble.

    A generator, a card at a time, because what a card means depends on
    the conditional assembly around it and only pass 1 knows that:
    `skipping()` says whether the next card is in the untaken half of a
    TRUE or FALS, and it is asked after pass 1 has placed everything
    before it. A skipped card defines no macro, calls none and moves no
    local label's scope; it goes through as written, for pass 1 to count
    its ENDC or list it.
    """
    macros = {}
    numbers = itertools.count(1)
    scope = [TOP_SCOPE]         # the caller's: the last label without a dot
    defining = None             # the macro whose body is being read,
    kept = True                 # and whether its MACRO card was assembled

    def card(loc, label, op, arg, text, within, depth, via, exps):
        if label and label.startswith('.') and len(label) > 1:
            label = f'{within}{LOCAL_MARK}{label[1:]}'
        elif label and depth == 0 and op != 'EQU':
            scope[0] = within = label
        if op not in macros:
            yield loc, label, op, localize(arg, within), text
            return
        macro = macros[op]
        if depth >= MACRO_DEPTH:
            raise AsmError(loc, f'{macro.name} is {MACRO_DEPTH} macros deep: one of them calls itself')
        args = [localize(a, within) for a in split_args(arg or '')]
        if len(args) > len(macro.params):
            raise AsmError(loc, f'{macro.name} takes {len(macro.params)} '
                                f'argument{"" if len(macro.params) == 1 else "s"}, not {len(args)}')
        given = dict(zip(macro.params, args + [''] * (len(macro.params) - len(args))))
        number = next(numbers)
        inner = f'{MACRO_SCOPE}{number}'
        # The call itself: nothing but its label, which is the first word.
        yield loc, label, None, None, text
        called = f'{macro.name} at {loc.path} line {loc.line}' if isinstance(loc, Loc) else macro.name
        for bloc, blabel, bop, barg, _ in macro.body:
            fill = lambda name: given.get(name, name)   # noqa: E731
            blabel = rewrite(blabel, fill) if blabel else blabel
            bop = rewrite(bop, fill).upper() if bop else bop
            barg = rewrite(barg, fill) if barg else barg
            # the listing's line: the card as filled in, a local label
            # the call passed in spelled as the call spelled it
            shown_as = LOCALIZED.sub('.', f'+{blabel or "":<15}{bop or "":<8}{barg or ""}'.rstrip())
            where = Loc(bloc.path, bloc.line, loc.seq if isinstance(loc, Loc) else bloc.seq,
                        via or called, exps + (number,))
            if skipping() and bop not in CONDITIONALS:
                yield where, None, None, None, shown_as
                continue
            try:
                yield from card(where, blabel or None, bop or None, barg or None, shown_as,
                                inner, depth + 1, via or called, exps + (number,))
            except AsmError as e:
                errors.append(e)

    for loc, label, op, arg, text in statements:
        try:
            if defining is not None:
                if op == 'ENDM':
                    if kept:
                        macros[defining.name] = defining
                    defining = None
                elif op == 'MACRO':
                    raise AsmError(loc, f'MACRO inside {defining.name}: one definition at a time')
                else:
                    defining.body.append((loc, label, op, arg, text))
                yield loc, None, None, None, text
                continue
            if op == 'MACRO':
                name = (label or '').upper()
                if not name:
                    raise AsmError(loc, 'MACRO needs a name in the label field')
                if (name in MEMREF or name in GEN_NONE or name in GEN_LEVEL or name in GEN_PAGE
                        or name in GEN_LITERAL or name in GEN_DIO or name in GEN_SKIP
                        or name in GEN_SHIFT or name in GEN_ALIAS or name in GEN_TWOWORD
                        or name in DIRECTIVES or name in SUBR_MACROS or name in ('EXCH', SELECT_BASE)):
                    raise AsmError(loc, f'{name} is the assembler\'s already')
                params = split_args(arg or '')
                bad = [q for q in params if not NAMES.fullmatch(q)]
                if bad or len(set(params)) != len(params):
                    raise AsmError(loc, f'{name}: its parameters are names, each once')
                # In the untaken half of a choice the body is read to its
                # ENDM all the same, and then let go.
                defining, kept = Macro(name, params, loc), not skipping()
                yield loc, None, None, None, text
                continue
            if skipping() and op not in CONDITIONALS:
                yield loc, label, op, arg, text
                continue
            if op == 'ENDM':
                raise AsmError(loc, 'ENDM with no MACRO above it')
            yield from card(loc, label, op, arg, text, scope[0], 0, None, ())
        except AsmError as e:
            errors.append(e)
            yield loc, None, None, None, text
    if defining is not None:
        errors.append(AsmError(defining.loc, f'{defining.name} has no ENDM'))


class AsmErrorList(AsmError):
    """Every error a run collected, raised once at the end so the first bad
    card does not hide the rest of a long file's problems.

    A subclass of AsmError so existing `except asm703.AsmError` callers keep
    working -- str() joins the individual messages. Carries the partial
    listing and symbol table so main() can still write the -l file, which is
    where a pile of errors actually gets read.
    """
    def __init__(self, errors, listing, symbols, warnings=()):
        self.errors = errors
        self.listing = listing
        self.symbols = symbols
        self.warnings = list(warnings)
        Exception.__init__(self, '\n'.join(str(e) for e in errors))


def char_literal(lineno, body):
    """Value of a SYM II quoted character constant, high bit set per byte."""
    chars = []
    i = 0
    while i < len(body):
        if body[i] == '\\':
            i += 1
            if i >= len(body):
                raise AsmError(lineno, 'character constant ends with a backslash')
            chars.append(TEXT_ESCAPES.get(body[i], ord(body[i])))
        else:
            chars.append(ord(body[i]))
        i += 1
    if not 1 <= len(chars) <= 2:
        raise AsmError(lineno, 'a character constant holds one or two characters')
    value = 0
    for c in chars:
        if c > 0x7f:
            raise AsmError(lineno, f'{chr(c)!r} is not ASCII')
        value = (value << 8) | (c | CHAR_HIGH_BIT)
    return value


class Expr:
    """Expression evaluator over labels and integer literals.

    Recursive descent rather than eval(), so a typo in the source is an
    assembler error rather than a Python traceback.

    Every term carries a relocation count beside its value -- a label counts
    one, a literal none, and the arithmetic combines them: SYM+1 is one,
    SYM*2 is two, SYM-SYM is none. Absolute assembly reads only the value.
    The object writer classifies each word by the count, and a byte
    instruction's operand doubles the counted symbols along with their
    values, which is how SYM II told a location from a constant.
    """

    # Every symbol an expression has looked up, while an assembly is
    # collecting them: what was never looked up was never used.
    seen = None

    TOKEN = re.compile(r"""
        \s*(?:
            (?P<hexc>X'[0-9A-Fa-f]+')
          | (?P<hex>0[xX][0-9A-Fa-f]+)
          | (?P<char>'(?:\\.|[^'])(?:\\.|[^'])?')
          | (?P<num>\d+)
          | (?P<len>LEN\("(?:\\.|[^"\\])*"\))
          | (?P<name>[A-Za-z_$][A-Za-z0-9_$.]*)
          | (?P<op>[-+*()])
        )
    """, re.VERBOSE)

    def __init__(self, text, symbols, here, lineno, counts=None, here_count=1):
        self.tokens = []
        pos = 0
        while pos < len(text):
            if text[pos].isspace():
                pos += 1
                continue
            m = self.TOKEN.match(text, pos)
            if not m:
                raise AsmError(lineno, f'cannot parse expression at {text[pos:]!r}')
            self.tokens.append(m.group(m.lastgroup))
            pos = m.end()
        self.pos = 0
        self.symbols = symbols
        self.counts = counts if counts is not None else {}
        self.here = here
        self.here_count = here_count
        self.lineno = lineno

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self):
        tok = self.peek()
        self.pos += 1
        return tok

    def value(self):
        return self.reloc()[0]

    def reloc(self):
        """The expression's value and its relocation count."""
        v, c = self.sum()
        if self.peek() is not None:
            raise AsmError(self.lineno, f'trailing {self.peek()!r} in expression')
        return v, c

    def sum(self):
        v, c = self.product()
        while self.peek() in ('+', '-'):
            op = self.take()
            rv, rc = self.product()
            if op == '+':
                v, c = v + rv, c + rc
            else:
                v, c = v - rv, c - rc
        return v, c

    def product(self):
        v, c = self.unary()
        while self.peek() == '*':
            self.take()
            rv, rc = self.unary()
            if c and rc:
                raise AsmError(self.lineno, 'the product of two locations has no meaning')
            v, c = v * rv, c * rv + rc * v
        return v, c

    def unary(self):
        if self.peek() == '-':
            self.take()
            v, c = self.unary()
            return -v, -c
        return self.atom()

    def atom(self):
        tok = self.take()
        if tok is None:
            raise AsmError(self.lineno, 'expression ended early')
        if tok == '(':
            v, c = self.sum()
            if self.take() != ')':
                raise AsmError(self.lineno, 'unbalanced parentheses')
            return v, c
        if tok.startswith("X'"):
            return int(tok[2:-1], 16), 0
        if tok.lower().startswith('0x'):
            return int(tok, 16), 0
        if tok.startswith("'"):
            return char_literal(self.lineno, tok[1:-1]), 0
        if tok[0].isdigit():
            return int(tok, 10), 0
        if tok.startswith('LEN("'):
            # how many bytes a string is, for the address one past its last
            return len(unescape(tok[5:-2], self.lineno)), 0
        if tok == '$':
            return self.here, self.here_count
        if tok not in self.symbols:
            raise UndefinedSymbol(self.lineno, f'undefined symbol {shown(tok)}')
        if Expr.seen is not None:
            Expr.seen.add(tok)
        return self.symbols[tok], self.counts.get(tok, 0)


def unescape(text, lineno):
    """Decode a TEXT literal's body into bytes."""
    out = bytearray()
    i = 0
    while i < len(text):
        c = text[i]
        if c == '\\':
            i += 1
            if i >= len(text):
                raise AsmError(lineno, 'string ends with a backslash')
            if text[i] not in TEXT_ESCAPES:
                raise AsmError(lineno, f'unknown escape \\{text[i]}')
            out.append(TEXT_ESCAPES[text[i]])
        else:
            if ord(c) > 0x7f:
                raise AsmError(lineno, f'{c!r} is not ASCII')
            out.append(ord(c))
        i += 1
    return bytes(out)


LINE = re.compile(r'^(?P<label>\S+)?\s*(?:(?P<op>\S+)(?:\s+(?P<arg>.*?))?)?\s*$')

# The relations a TRUE/FALS guard can test. SYM II writes them without spaces,
# as in `TRUE NDSK=0` or `FALS CORESIZE=4096`.
RELATION = re.compile(r'^(?P<lhs>.+?)(?P<rel><=|>=|<>|=|<|>)(?P<rhs>.+)$')


def prescan_equates(statements):
    """Values for every EQU whose expression does not depend on where it sits.

    Only conditional guards use this, and only as a fallback behind the symbols
    pass 1 has actually placed. Equates inside a branch that turns out to be
    untaken are collected too, which is harmless: nothing here reaches the code
    generator, and a guard naming a symbol that only exists in the other half
    of a choice would have no answer otherwise either.

    `$` is refused rather than guessed at -- the location counter has no value
    before pass 1 places anything.
    """
    values = {}
    pending = [(lineno, label, arg) for lineno, label, op, arg, _ in statements
               if op == 'EQU' and label and arg and '$' not in arg]
    while pending:
        rest = []
        for lineno, label, arg in pending:
            try:
                values[label] = Expr(arg, values, None, lineno).value()
            except (UndefinedSymbol, AsmError):
                rest.append((lineno, label, arg))
        if len(rest) == len(pending):
            break       # the leftovers name something no equate defines
        pending = rest
    return values


def condition(text, symbols, here, lineno):
    """Evaluate a TRUE/FALS guard.

    Relations are handled here rather than in `Expr` because they appear
    nowhere else: an operand is always a plain address expression, and letting
    `<` into the general grammar would only create ways to write nonsense.
    """
    m = RELATION.match(text or '')
    if not m:
        raise AsmError(lineno, f'{text!r} is not a condition')
    lhs = Expr(m.group('lhs'), symbols, here, lineno).value()
    rhs = Expr(m.group('rhs'), symbols, here, lineno).value()
    rel = m.group('rel')
    return {
        '=': lhs == rhs,
        '<>': lhs != rhs,
        '<': lhs < rhs,
        '>': lhs > rhs,
        '<=': lhs <= rhs,
        '>=': lhs >= rhs,
    }[rel]


def parse(path, seq=None):
    """Split one source file into (loc, label, mnemonic, operand, text) tuples.

    `seq` is a shared counter handing out deck-order positions when several
    files make up one deck; left alone, the file is a deck of its own.
    """
    if seq is None:
        seq = itertools.count()
    out = []
    with open(path, encoding='utf-8') as f:
        for lineno, raw in enumerate(f, 1):
            text = raw.rstrip('\n')
            # SYM II's comment card: a quote in column 1. Period listings are
            # full of them, and it cannot be confused with a character literal
            # or an X'..' constant because those never start a line.
            if text.startswith("'"):
                continue
            # strip comments, but not a ';' inside a string literal
            stripped, quoted = [], False
            for c in text:
                if c == '"':
                    quoted = not quoted
                if c == ';' and not quoted:
                    break
                stripped.append(c)
            code = ''.join(stripped).rstrip()
            if not code.strip():
                continue
            # A label starts in column 0, so the leading `\S+` in LINE can only
            # match on an unindented line -- which is exactly the rule.
            m = LINE.match(code)
            label, op, arg = m.group('label'), m.group('op'), m.group('arg')
            if label:
                label = label.rstrip(':')
            op = op.upper() if op else None
            # The double and byte shifts are printed with a gap in the period
            # listings -- `SRC D 15`, `SRL L 4` -- following appendix B of the
            # reference manual. That is one mnemonic, not a mnemonic and an
            # operand, so glue it back together before anything else looks.
            if op in SHIFT_STEMS and arg:
                head, _, tail = arg.partition(' ')
                if head.upper() in ('D', 'L', 'R'):
                    op, arg = op + head.upper(), tail.strip() or None
            out.append((Loc(path, lineno, next(seq)), label, op, arg, text))
    return out


def operand_list(arg):
    """Split a comma-separated operand list, ignoring commas inside quotes.

    `D ','` is one operand, not two empty ones -- a comma is a perfectly good
    character constant -- and so is the comma in `LEN("A, B")`.
    """
    cuts = [-1] + commas(arg) + [len(arg)]
    return [arg[a + 1:b] for a, b in zip(cuts, cuts[1:])]


def sizeof(lineno, op, arg):
    """How many words a statement occupies."""
    if op is None:
        return 0
    if op in ('EQU', 'ORG', 'ORIG', 'TRUE', 'FALS', 'ENDC', 'END', 'LPOOL', 'NTRY'):
        return 0    # a pool's words are counted where pass 1 lays it down
    if op in SUBR_MACROS or op == 'EXCH':
        return 2
    if op in GEN_TWOWORD:
        return 2  # opcode word, then the 15-bit address word
    if op in ('WORD', 'DATA', 'D'):
        return len(operand_list(arg))
    if op == 'BYTE':
        # two bytes to the word, rounded up
        return (len(operand_list(arg)) + 1) // 2
    if op in ('TEXT', 'STR'):
        return None  # needs the decoded string; handled by the caller
    if op == 'RES':
        return None
    if op == 'JSX':
        # SYM II's monitor calling sequence lays the arguments down as inline
        # words after the jump: `JSX DOIO,PRINFIOT,BUF3,BUF3CT` assembles to
        # 2044 0557 05BD 85C3. A plain `JSX ADDR` is the one-operand case and
        # still occupies one word.
        return len(operand_list(arg)) if arg else 1
    return 1


def string_body(lineno, arg):
    if arg is None or not arg.startswith('"') or not arg.endswith('"') or len(arg) < 2:
        raise AsmError(lineno, 'TEXT and STR need a double-quoted string')
    return unescape(arg[1:-1], lineno)


def encode(lineno, op, arg, symbols, here, address_syms=None,
           prev_selects_page=False, counts=None, relocatable=False):
    """Assemble one instruction into a single word.

    `prev_selects_page` says an SML/SMU/SMB is in force: emitted in front of
    this instruction with nothing but generics between, so that its page
    selection governs exactly this instruction's memory reference -- which
    is what licenses a direct operand outside the instruction's own page.

    Returns the word, or `(word, kind)` when `relocatable` -- see KINDS.
    `address_syms` is accepted for older callers and ignored: which symbols
    are locations is what `counts` says.
    """
    word, kind = encode2(lineno, op, arg, symbols, here, prev_selects_page,
                         counts, relocatable)
    return (word, kind) if relocatable else word


# What the object writer needs to know about each assembled word, and the
# relocating loader's code for it:
#
#   A     absolute, stored verbatim               Cn
#   W11   a memory reference whose 11-bit M field is a location  8n
#   W15   a data word holding a word address       9n
#   B16   a data word holding a byte address       An
#   B11   a byte reference whose M field is a location (direct)   Bn
#   B11I  the same, indexed -- relocated alike, but it does not ask for a
#         byte page, since an indexed byte reference is flat
#   SMB   a page selection of a location: the loader manufactures the
#         instruction from the final address       3
#   ILOC  a reserved word                          4
KINDS = {'A': 4, 'W11': 0, 'W15': 1, 'B16': 2, 'B11': 3, 'B11I': 3}


def encode2(lineno, op, arg, symbols, here, prev_selects_page=False,
            counts=None, relocatable=False, info=None):
    """encode(), always returning `(word, kind)`. `info`, a dict, is told
    the page a direct operand of an absolute build is in -- a word page for
    a word reference, a byte page for a byte reference."""
    if counts is None:
        counts = {}

    def reloc(text):
        return Expr(text, symbols, here, lineno, counts).reloc()

    def value(text, limit=None):
        v = reloc(text)[0]
        if limit is not None and not 0 <= v <= limit:
            raise AsmError(lineno, f'{text.strip()} = {v} does not fit in {limit + 1} values')
        return v

    if op in MEMREF:
        if arg is None:
            raise AsmError(lineno, f'{op} needs an address')
        text = arg.strip()
        indexed = text.startswith('*')
        if indexed:
            text = text[1:]
        if text.startswith('/'):
            # the PTB listing's "STB /TEST": the byte address of a word label
            addr, cnt = reloc(text[1:])
            addr, cnt = addr * 2, cnt * 2
        elif op in BYTE_REF:
            # A byte instruction's operand is a byte address, and SYM II
            # converts the symbols in it for you: the relocating loader writes
            # `STB BUF3END+1` where BUF3END is the word 5C2, and the assembler
            # emits byte B85 -- the symbol doubled, the constant added as a
            # byte. `STB * BUF3+NAMESIZE+NAMESIZE+6` is the same rule with four
            # terms and settles that it is per symbol rather than applied to
            # the whole expression, which would have doubled the constants too.
            # ...but only the symbols that are *locations*. `STB * BUF3+
            # NAMESIZE+NAMESIZE+6` doubles BUF3, a location, and leaves
            # NAMESIZE, which is `EQU 2`, alone: byte B7A + 2 + 2 + 6 = B84,
            # which is what the listing prints. SYM II is a relocating
            # assembler and had to know the difference; so does this, and the
            # relocation count is the difference.
            doubled = {name: v * 2 if counts.get(name) else v
                       for name, v in symbols.items()}
            dcounts = {name: c * 2 for name, c in counts.items()}
            addr, cnt = Expr(text, doubled, here * 2, lineno, dcounts,
                             here_count=2).reloc()
        else:
            addr, cnt = reloc(text)
        # An indexed operand's M is a bare base or displacement under IXR, not
        # a paged address, so it keeps the old rules: byte bases wrap within
        # the eleven-bit field -- the relocating loader writes `STB * BUF3+...`
        # with the full byte address B84 and SYM II emits 384 -- and word bases
        # must simply fit.
        #
        # A *direct* operand is a paged address: the hardware reads M under
        # EXR, and EXR tracks the executing instruction's page unless a
        # selection in front of it, with only generics between, names another. So a direct
        # operand in the instruction's own page emits its low eleven bits
        # (what SYM II printed for the loader's own-page byte references), one
        # in a different page rides on a preceding SML/SMU/SMB, and anything
        # else would silently reference the wrong page at run time -- which
        # was assembled without complaint before this check existed, for
        # operands that happened to fit the field.
        #
        # A module has no page of its own until the loader gives it one, so
        # the object writer's rules differ: a location is relocated by the
        # loader (the module is held to one page, so it is always in reach),
        # and an absolute address is reachable only through a page selection
        # in front of it.
        unit = 'byte' if op in BYTE_REF else 'word'
        kind = 'A'
        if indexed:
            if op in BYTE_REF:
                addr &= 0x7ff
            if relocatable and cnt:
                if op in BYTE_REF and cnt == 2:
                    kind = 'B11I'
                else:
                    raise AsmError(
                        lineno,
                        f'indexed {unit} base {text.strip()!r} is a location; an '
                        f'indexed M is a displacement under IXR, so put the '
                        f'address in a cell and load it into the index register')
            if not 0 <= addr <= 0x7ff:
                raise AsmError(
                    lineno,
                    f'indexed {unit} base {addr:#x} is outside the 11-bit M field')
        else:
            if addr < 0:
                raise AsmError(lineno, f'{unit} address {addr} is negative')
            if relocatable:
                if cnt == 0:
                    if info is not None:
                        info['page'] = addr >> 11
                    if not prev_selects_page:
                        raise AsmError(
                            lineno,
                            f'{unit} address {addr:#x} is absolute and no page is '
                            f'selected; a module runs in whatever page the loader '
                            f'puts it, so put SMB in front, or go through the '
                            f'index register')
                elif cnt == (2 if op in BYTE_REF else 1):
                    kind = 'B11' if op in BYTE_REF else 'W11'
                else:
                    raise AsmError(
                        lineno,
                        f'{unit} address {text.strip()!r} relocates {cnt} times '
                        f'over; a direct operand is one location or none')
            else:
                ipage = here >> 10 if op in BYTE_REF else here >> 11
                tpage = addr >> 11
                if info is not None:
                    info['page'] = tpage
                if tpage != ipage and not prev_selects_page:
                    raise AsmError(
                        lineno,
                        f'{unit} address {addr:#x} is in {unit} page {tpage} and this '
                        f'instruction runs in {unit} page {ipage}, so the reference '
                        f'would land in the wrong page -- put SMB in front, or go '
                        f'through the index register')
            addr &= 0x7ff
        return (MEMREF[op] << 12) | (0x0800 if indexed else 0) | addr, kind

    if op in GEN_ALIAS:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_ALIAS[op], 'A'

    if op in GEN_NONE:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_NONE[op], 'A'

    if op in GEN_LEVEL:
        return GEN_LEVEL[op] | value(arg, 15), 'A'

    if op in GEN_PAGE:
        return GEN_PAGE[op] | value(arg, 15), 'A'

    if op == SELECT_BASE:
        # EXR holds a five-bit byte page. A word address's byte page is the
        # top five bits of twice it, so word >> 10; SML reaches the lower
        # sixteen pages and SMU the upper sixteen.
        target, cnt = reloc(arg)
        if relocatable and cnt:
            if cnt != 1:
                raise AsmError(lineno, f'SMB {arg.strip()!r} is not one location')
            # The page is not known until the loader places the module; the
            # object carries the address and the loader builds the word.
            return target & 0x7fff, 'SMB'
        page = (target >> 10) & 0x1f
        if page < 16:
            return GEN_PAGE['SML'] | page, 'A'
        return GEN_PAGE['SMU'] | (page - 16), 'A'

    if op in GEN_LITERAL:
        v = value(arg)
        if not -128 <= v <= 255:
            raise AsmError(lineno, f'{op} literal {v} does not fit in a byte')
        return GEN_LITERAL[op] | (v & 0xff), 'A'

    if op in GEN_DIO:
        if arg is None or ',' not in arg:
            raise AsmError(lineno, f'{op} needs "device,function"')
        dev, func = arg.split(',', 1)
        return GEN_DIO[op] | (value(dev, 15) << 4) | value(func, 15), 'A'

    if op in GEN_SKIP:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_SKIP[op], 'A'

    if op in GEN_SHIFT:
        return GEN_SHIFT[op] | value(arg, 15), 'A'

    raise AsmError(lineno, f'unknown mnemonic {op!r}')


def data_kind(lineno, cnt, relocatable, what):
    """The kind of a data word from its relocation count."""
    if cnt == 0:
        return 'A'
    if cnt == 1:
        return 'W15'
    if cnt == 2:
        return 'B16'
    if not relocatable:
        return 'A'
    raise AsmError(lineno, f'{what} relocates {cnt} times over; a data word holds '
                           f'a number, a word address or a byte address')


def assemble(paths, origin=0):
    """Assemble one file, or a list of files as a single deck in order, into
    an absolute core image: `(image, listing, symbols, core)`.

    `origin` is where the location counter starts, for a deck with no ORG of
    its own -- a module source assembled absolute at a chosen base.
    """
    r = _assemble(paths, origin=origin, relocatable=False)
    return r['image'], r['listing'], r['symbols'], r['core']


def assemble_object(paths):
    """Assemble a deck as a relocatable module: `(stream, listing, symbols,
    core, size)`, where `stream` is the loader text in the relocating
    loader's record format (see the module docstring) and `core` is the
    module at base 0 -- with each page selection of a location holding the
    address the loader will build the instruction from.
    """
    r = _assemble(paths, origin=0, relocatable=True)
    stream = object_stream(r['core'], r['kinds'], r['size'], r['entry'], r['sizeb'])
    return stream, r['listing'], r['symbols'], r['core'], r['size']


def _assemble(paths, origin=0, relocatable=False):
    if isinstance(paths, str):
        paths = [paths]
    seq = itertools.count()
    statements = []
    for path in paths:
        if len(paths) > 1:
            # A marker card per file, so the listing says where each begins.
            statements.append((Loc(path, 0, next(seq)), None, None, None,
                               f'; ==== {path}'))
        statements.extend(parse(path, seq))

    # The configuration equates a TRUE/FALS guard names do not have to come
    # before it. X-RAY states its whole system description on its fifth page
    # and never needs this, but the relocating loader tests `LOADER=STANDARD`
    # on card 5 and equates `LOADER` on card 30 -- and the listing shows SYM II
    # evaluating it correctly, which a single forward pass cannot do. So
    # collect the location-independent equates first, and let a guard fall back
    # on them. Everything else still resolves in order.
    # Errors from every stage, reported together at the end: on a long file
    # the first bad card should not hide the rest.
    errors = []
    # The cards as written: what a macro would generate is not known this
    # early, since which macro a name means can itself hang on a guard.
    config = prescan_equates(statements)

    # Pass 1: place every statement and collect the labels.
    symbols = {}
    placed = []
    here = origin
    # Stack of "are we assembling?" flags, one per open TRUE/FALS.
    cond = []
    # EQUs whose operand named something not yet defined; settled after the pass.
    deferred = []
    # Each symbol's relocation count: a label is one location, an EQU is
    # whatever its expression comes to. A byte instruction's operand is a
    # byte address, and SYM II converts the locations in it while leaving the
    # constants alone -- so the two have to be told apart -- and the object
    # writer relocates by the same count.
    counts = {}
    # The END card's operand, which names a module's entry.
    end_card = None
    # Literals: the ones waiting for a pool, by what makes two of them the
    # same cell, and every one there is, by its slot's name.
    pending = {}
    literals = {}
    warnings = []
    Expr.seen = set()
    # Labels on data, for the report of cells nothing refers to.
    cells = {}
    # Where the location counter stood when an ORG moved it, by the ORG's
    # place in `placed`: the end of whatever is in front of it.
    left = {}
    # The card each name is defined on, and the NTRY cards in deck order,
    # each with the names it lists: what --exports writes.
    defined = {}
    entries = []

    def literal(lineno, op, arg):
        """The name of the pool slot `=expr` asks for."""
        if op not in LITERAL_OPS:
            raise AsmError(lineno, f'{op} takes no literal: one is a cell the '
                                   f'assembler makes, for an instruction that reads it')
        body = arg.strip()[1:].strip()
        if not body:
            raise AsmError(lineno, 'a literal needs an expression after its =')
        probe = Expr(body, symbols, None, lineno, counts)
        if '$' in probe.tokens:
            raise AsmError(lineno, 'a literal cannot hold $: its cell is in the '
                                   'pool, not here, so name the place')
        try:
            value, cnt = probe.reloc()
            key = (value & 0xffff if cnt == 0 else value, cnt)
        except UndefinedSymbol:
            # defined further down: by spelling, as the tokens have it, so
            # that the space in a string is not taken for spacing
            key = ('spelled', *probe.tokens)
        if key not in pending:
            name = f'{LITERAL_PREFIX}{len(literals)}'
            pending[key] = literals[name] = Literal(name, body, lineno)
        return pending[key].name

    def pool():
        """Lay the waiting literals down from `here` on: the cells, each
        once. A literal that was met before what it names was defined is
        looked at again here, and takes the cell of its value if there is
        one."""
        nonlocal here
        laid, cell = [], {}
        for key, lit in pending.items():
            if key[:1] == ('spelled',):
                try:
                    value, cnt = Expr(lit.body, symbols, None, lit.lineno, counts).reloc()
                    key = (value & 0xffff if cnt == 0 else value, cnt)
                except UndefinedSymbol:
                    pass
            if key not in cell:
                cell[key] = here
                laid.append(lit)
                here += 1
            symbols[lit.name] = cell[key]
            counts[lit.name] = 1
        pending.clear()
        return laid

    # Macros and local labels are settled as pass 1 comes to each card, so
    # that the cards it is skipping settle nothing: what it is handed is
    # plain cards under deck-wide names.
    for lineno, label, op, arg, text in expand(statements, errors, lambda: not all(cond)):
        try:
            if op in CONDITIONALS:
                if op == 'ENDC':
                    if not cond:
                        raise AsmError(lineno, 'ENDC without TRUE or FALS')
                    cond.pop()
                elif all(cond):
                    taken = condition(arg, ChainMap(symbols, config), here, lineno)
                    cond.append(taken if op == 'TRUE' else not taken)
                else:
                    # already inside skipped code: the guard is not evaluated at
                    # all, since the symbols it names may never have been defined
                    cond.append(False)
                placed.append((lineno, here, op, arg, text))
                continue
            if not all(cond):
                # Skipped code places nothing and defines nothing, not even its
                # labels -- otherwise a label in the untaken half of a choice would
                # collide with the one in the taken half.
                placed.append((lineno, here, None, arg, text))
                continue
            if op == 'END':
                if pending:
                    at = here
                    placed.append((lineno, at, 'LPOOL', pool(), None))
                placed.append((lineno, here, op, arg, text))
                end_card = (lineno, arg)
                # Trailing cards in the same file are dropped, as they always
                # were; a whole later file after the END is a deck mistake.
                tail = {s[0].path for s in statements
                        if isinstance(s[0], Loc) and isinstance(lineno, Loc)
                        and s[0].seq > lineno.seq and s[0].path != lineno.path}
                if tail:
                    errors.append(AsmError(
                        lineno,
                        f'END here leaves {", ".join(sorted(tail))} unassembled'))
                break
            if op == 'EQU':
                if not label:
                    raise AsmError(lineno, 'EQU needs a label')
                try:
                    symbols[label], counts[label] = Expr(arg or '', symbols, here,
                                                         lineno, counts).reloc()
                except UndefinedSymbol:
                    # SYM II allowed an EQU to name a symbol defined further down
                    # the deck -- the X-RAY listing's card 298 is
                    # `MAXP EQU ENDP-PEAT+12`, and both of those are defined much
                    # later. Set it aside and settle it once the pass has seen
                    # every label. An EQU that some *later* EQU depends on still
                    # resolves, because the leftovers are iterated to a fixpoint.
                    deferred.append((lineno, label, arg, here))
                defined[label] = (lineno, arg)
                placed.append((lineno, here, op, arg, text))
                continue
            if label:
                if label in symbols:
                    raise AsmError(lineno, f'{shown(label)} is defined twice')
                if label.startswith(LITERAL_PREFIX):
                    raise AsmError(lineno, f'{label!r}: names beginning '
                                           f'{LITERAL_PREFIX} are the literal pools\'')
                # SUBR lays down a return slot and then the STX that fills it; the
                # name belongs to the STX, so that callers jump to code.
                symbols[label] = here + 1 if op == 'SUBR' else here
                counts[label] = 1
                defined[label] = (lineno, None)
                if op in DATA_OPS:
                    cells[label] = lineno
            if op == 'LPOOL':
                if arg:
                    raise AsmError(lineno, 'LPOOL takes no operand')
                at = here
                placed.append((lineno, at, op, pool(), text))
                continue
            if arg and arg.lstrip().startswith('='):
                arg = literal(lineno, op, arg)
            if op in ('ORG', 'ORIG'):
                target, cnt = Expr(arg or '', symbols, here, lineno, counts).reloc()
                # A module is one contiguous stream from word 0, placed by the
                # loader; the one ORG it may carry is the no-op that names where
                # it already is (bcore.asm's `ORG B.CORE`).
                if relocatable and (target, cnt) != (here, 1):
                    raise AsmError(
                        lineno,
                        f'{op} {arg.strip()!r} in a module: the loader decides where '
                        f'a module goes, so its words are one stream from word 0')
                left[len(placed)] = here
                here = target
                placed.append((lineno, here, op, arg, text))
                continue
            placed.append((lineno, here, op, arg, text))
            if op == 'TEXT':
                body = string_body(lineno, arg)
                if len(body) % 2:
                    raise AsmError(
                        lineno,
                        f'TEXT is {len(body)} bytes; a word is two bytes, so pad it '
                        f'to an even length rather than let the assembler guess, or '
                        f'say STR and have a zero byte')
                here += len(body) // 2
            elif op == 'STR':
                here += (len(string_body(lineno, arg)) + 1) // 2
            elif op == 'RES':
                here += Expr(arg or '', symbols, here, lineno).value()
            else:
                here += sizeof(lineno, op, arg)
        except AsmError as e:
            errors.append(e)
            # Neutralize the statement so pass 2 does not report it a second
            # time, then advance by its best-guess size so every later address
            # -- and so every later error message -- stays right.
            if placed and placed[-1][0] == lineno:
                placed[-1] = (lineno, placed[-1][1], None, arg, text)
            else:
                placed.append((lineno, here, None, arg, text))
            try:
                here += sizeof(lineno, op, arg) or 0
            except (AsmError, TypeError):
                here += 1

    if pending:
        # a deck with no END card: the pool closes it all the same
        at = here
        placed.append((statements[-1][0] if statements else 0, at, 'LPOOL', pool(), None))

    # Settle the deferred EQUs. Each sweep that resolves anything may unblock
    # another, so sweep until one achieves nothing: either they are all done or
    # what is left is a genuine undefined symbol (or a cycle), and every one of
    # those is worth reporting.
    while deferred:
        rest = []
        for lineno, label, arg, at in deferred:
            try:
                symbols[label], counts[label] = Expr(arg or '', symbols, at,
                                                     lineno, counts).reloc()
            except UndefinedSymbol as err:
                rest.append((lineno, label, arg, at, err))
        if len(rest) == len(deferred):
            errors.extend(item[4] for item in rest)
            break
        deferred = [item[:4] for item in rest]

    # Where each macro expansion's words begin and end, outermost included.
    spans = {}
    for i, (lineno, addr, _, _, _) in enumerate(placed):
        if isinstance(lineno, Loc) and lineno.exps:
            after = left.get(i + 1, placed[i + 1][1]) if i + 1 < len(placed) else here
            for number in lineno.exps:
                spans.setdefault(number, [addr, after])[1] = after

    # Pass 2: emit.
    core = {}
    kinds = {}
    listing = []
    entry = None
    # The page selection in force, as (byte page, whether it is a
    # location's -- which in a module is all that is known of it), or None.
    # SML/SMU/SMB make one and the next memory reference uses it up: the
    # hardware reloads the extension register after a memory reference and
    # after nothing else, so the generics between the two leave it standing,
    # and it licenses that reference to leave its own page. A skip between
    # them makes which reference that is a matter of the data, so the
    # assembler takes the selection to end there. Statements that emit
    # nothing (EQU, ORG, comments) are invisible to the hardware.
    selection = None
    # Each direct JMP's target, and each pool laid down, for the jumps that
    # land in one.
    jumps = []
    pools = []

    def data(part, at, lineno, what):
        v, c = Expr(part, symbols, at, lineno, counts).reloc()
        return v & 0xffff, data_kind(lineno, c, relocatable, what)

    # The lint's view of what has been emitted: the last two statements
    # that put words down, as (mnemonic, words).
    before = [(None, 0), (None, 0)]

    def warn(lineno, message):
        warnings.append(AsmWarning(lineno, message))

    def falls_into(lineno, what):
        """Warn if execution can run on into data laid down here."""
        (op2, _), (op1, _) = before
        if op1 is None or op1 in DATA_OPS:
            return
        if op1 in ('JMP', 'EXIT', 'HLT') and op2 not in SKIPPERS:
            return
        warn(lineno, f'{what} follows {op1}'
                     + (f' under {op2}' if op2 in SKIPPERS and op1 in ('JMP', 'EXIT', 'HLT') else '')
                     + ', which can run on into it')

    def lint(lineno, addr, op, arg, words, wkinds, info):
        """The mistakes this machine makes easy, looked for in what is about
        to be emitted. Each is something the hardware does exactly as
        written and nobody means."""
        prev = before[1][0]
        if prev in SKIPPERS:
            # A skip passes over one word.
            exps = lineno.exps if isinstance(lineno, Loc) else ()
            start, end = spans[exps[0]] if exps else (addr, addr)
            if exps and addr == start and end - start > 1:
                # the first word of a macro call that is more than one
                name = lineno.via.split(' at ')[0]
                warn(lineno, f'{name} is {end - start} words here and {prev} skips '
                             f'only the first')
            elif op in PAGE_SELECTS:
                warn(lineno, f'{prev} skips this {op} and not the reference after it, '
                             f'which then runs without its page')
            elif len(words) > 1 and op not in DATA_OPS:
                warn(lineno, f'{op} is {len(words)} words and {prev} skips only the first')
        if selection is not None and op in MEMREF and not arg.strip().startswith('*'):
            # A page selection governs this reference whatever it names, so
            # what it names had better be in the page selected.
            page, located = selection
            if relocatable and located != (wkinds[0] != 'A'):
                # all a module knows of its own page is that it is its own
                where = 'the module\'s own page' if located else f'byte page {page}'
                warn(lineno, f'{op} {arg.strip()} is read under the selection of '
                             f'{where}, which is not where it is')
            elif 'page' in info:
                unit = 'byte' if op in BYTE_REF else 'word'
                want = page if op in BYTE_REF else page >> 1
                if info['page'] != want:
                    warn(lineno, f'{op} {arg.strip()} is in {unit} page {info["page"]} and '
                                 f'{unit} page {want} is selected')

    for lineno, addr, op, arg, text in placed:
        words = []
        wkinds = []
        pooled = None
        info = {}
        try:
            if op == 'LPOOL':
                pooled = arg
                for i, lit in enumerate(pooled):
                    w, k = data(lit.body, addr + i, lit.lineno, '=' + lit.body)
                    words.append(w)
                    wkinds.append(k)
                if pooled:
                    falls_into(lineno, 'a literal pool')
                    pools.append((addr, len(pooled)))
            elif arg in literals and op in MEMREF and not relocatable \
                    and symbols[arg] >> 11 != addr >> 11:
                lit, slot = literals[arg], symbols[arg]
                raise AsmError(
                    lineno,
                    f'the literal ={lit.body} is pooled at {slot:#x}, in word page '
                    f'{slot >> 11}, and this instruction runs in word page {addr >> 11}: '
                    f'put an LPOOL after it in its own page')
            elif op in ('WORD', 'DATA', 'D'):
                for part in operand_list(arg):
                    w, k = data(part, addr + len(words), lineno, part.strip())
                    words.append(w)
                    wkinds.append(k)
            elif op in ('TEXT', 'STR'):
                # STR is TEXT that may be an odd number of bytes: the low
                # half of its last word is then zero, and whoever reads it
                # knows the length some other way (LEN).
                body = string_body(lineno, arg)
                body += b'\0' * (len(body) % 2)
                words = [(body[i] << 8) | body[i + 1] for i in range(0, len(body), 2)]
                wkinds = ['A'] * len(words)
            elif op == 'EXCH':
                words = [EXCH_WORD, EXCH_WORD]
                wkinds = ['A', 'A']
            elif op == 'SUBR':
                words = [0x0000, (MEMREF['STX'] << 12) | (addr & 0x07ff)]
                wkinds = ['A', 'W11']
            elif op == 'EXIT':
                # `EXIT sym` returns through the subroutine's slot; `EXIT sym,n`
                # returns n words further on, which is how a subroutine takes an
                # error return -- the relocating loader's `EXIT RELOAD,1` emits
                # 963A 2801, a JSX indexed by one rather than zero.
                parts = operand_list(arg or '')
                entry_addr = Expr(parts[0], symbols, addr, lineno).value()
                skip = Expr(parts[1], symbols, addr, lineno).value() if len(parts) > 1 else 0
                if not 0 <= skip <= 0x7ff:
                    raise AsmError(lineno, f'EXIT return offset {skip} does not fit')
                words = [(MEMREF['LDX'] << 12) | ((entry_addr - 1) & 0x07ff),
                         (MEMREF['JSX'] << 12) | 0x0800 | skip]
                wkinds = ['W11', 'A']
                # The LDX is a direct reference to the return slot, so the
                # slot has to be in the page it is read under: this one, or
                # the one a selection in front names. A module is one page.
                slot = entry_addr - 1
                under = addr >> 11 if selection is None else selection[0] >> 1
                if not relocatable and slot >> 11 != under:
                    warn(lineno, f'EXIT {parts[0].strip()} reads its return slot in word page '
                                 f'{under}, and the slot is in word page {slot >> 11}')
            elif op == 'BYTE':
                # Two bytes to a word, high half first, as everywhere else on this
                # machine. An odd count leaves the low half zero.
                vals = [Expr(part, symbols, addr, lineno).value() & 0xff
                        for part in operand_list(arg)]
                if len(vals) % 2:
                    vals.append(0)
                words = [(vals[i] << 8) | vals[i + 1] for i in range(0, len(vals), 2)]
                wkinds = ['A'] * len(words)
            elif op == 'RES':
                words = [0] * Expr(arg or '', symbols, addr, lineno).value()
                wkinds = ['ILOC'] * len(words)
            elif op == 'JSX' and arg and len(operand_list(arg)) > 1:
                parts = operand_list(arg)
                w, k = encode2(lineno, op, parts[0], symbols, addr, selection is not None,
                               counts, relocatable, info)
                words, wkinds = [w], [k]
                for i, part in enumerate(parts[1:], 1):
                    w, k = data(part, addr + i, lineno, part.strip())
                    # The last argument carries bit 0 -- this machine's most
                    # significant -- to mark the end of the list. That is the
                    # assembler's doing, not the programmer's: `JSX STAT,LOADFIOT`
                    # emits 854F where LOADFIOT is 54F. X-RAY's I/O done area
                    # confirms it from the other side, masking the bit off with
                    # `AND X7FF` under the comment DO NOT TEST SIGN BIT.
                    if i == len(parts) - 1:
                        w |= 0x8000
                    words.append(w)
                    wkinds.append(k)
            elif op in GEN_TWOWORD:
                # MPY/DIV: the fixed opcode word, then the operand as a flat
                # 15-bit word address (section 6). No page arithmetic, no
                # SMB, and no indexed form -- the second word already reaches
                # all of core, and is the one field on the machine the loader
                # relocates on its own.
                if arg is None:
                    raise AsmError(lineno, f'{op} needs an address')
                if arg.strip().startswith('*'):
                    raise AsmError(
                        lineno,
                        f'{op} has no indexed form; its second word is a flat '
                        f'15-bit address that already reaches all of core')
                operand, cnt = Expr(arg, symbols, addr + 1, lineno, counts).reloc()
                if not 0 <= operand <= 0x7fff:
                    raise AsmError(
                        lineno, f'{op} operand {operand:#x} is not a 15-bit word address')
                if relocatable and cnt not in (0, 1):
                    raise AsmError(lineno, f'{op} operand {arg.strip()!r} is not one location')
                words = [GEN_TWOWORD[op], operand]
                wkinds = ['A', 'W15' if cnt == 1 else 'A']
            elif op == 'END':
                if relocatable:
                    # The entry is the one address the loader hands the
                    # executive, so a module's END must name it.
                    if not arg:
                        raise AsmError(lineno, 'a module\'s END must name its entry')
                    entry, cnt = Expr(arg, symbols, addr, lineno, counts).reloc()
                    if cnt != 1:
                        raise AsmError(lineno, f'entry {arg.strip()!r} is not a location in the module')
                words = []
            elif op == 'NTRY':
                names = [n.strip() for n in operand_list(arg or '')]
                for name in names:
                    if LOCAL_MARK in name:
                        raise AsmError(lineno, f'{shown(name)} is a local label, '
                                               f'which nothing outside its routine names')
                    if not NAMES.fullmatch(name):
                        raise AsmError(lineno, f'NTRY takes names, and {name!r} is not one')
                    if any(name in listed for _, listed in entries):
                        raise AsmError(lineno, f'{name} is an entry twice')
                    _, cnt = Expr(name, symbols, addr, lineno, counts).reloc()
                    if relocatable and cnt:
                        raise AsmError(
                            lineno,
                            f'{name} is a location in a module, which has no value '
                            f'until the loader places it: an entry here is a constant')
                entries.append((lineno, names))
                words = []
            elif op in ('ORG', 'ORIG', 'EQU', 'TRUE', 'FALS', 'ENDC', None):
                words = []
            else:
                w, k = encode2(lineno, op, arg, symbols, addr, selection is not None,
                               counts, relocatable, info)
                words, wkinds = [w], [k]
                if op == 'JMP' and not arg.strip().startswith(('*', '/')):
                    jumps.append((Expr(arg, symbols, addr, lineno, counts).value(),
                                  lineno, arg.strip()))

            for i, (w, k) in enumerate(zip(words, wkinds)):
                if addr + i in core:
                    raise AsmError(lineno, f'word {addr + i:#x} is assembled twice')
                core[addr + i] = w
                kinds[addr + i] = k
            if words and pooled is None:
                lint(lineno, addr, op, arg, words, wkinds, info)
        except AsmError as e:
            errors.append(e)
            words = []
        if words:
            before = [before[1], (op, len(words))]
            if op == SELECT_BASE:
                target, cnt = Expr(arg, symbols, addr, lineno, counts).reloc()
                selection = ((target >> 10) & 0x1f, bool(cnt))
            elif op in PAGE_SELECTS:
                selection = ((words[0] & 0xf) | (0x10 if op == 'SMU' else 0), False)
            elif (pooled is not None or len(words) != 1 or op in MEMREF or op in SKIPPERS
                  or op in DATA_OPS or op in SELECTION_ENDS):
                selection = None
        if pooled is None:
            listing.append((addr, words, text))
        else:
            # the card, if there was one, and then a line to each literal
            if text is not None:
                listing.append((addr, [], text))
            for i, lit in enumerate(pooled):
                listing.append((addr + i, words[i:i + 1], f'{"":32}; ={lit.body}'))

    # A jump into a pool is a way in that the card in front of the pool
    # does not show: the JMP $+2 a macro ends on, with the pool behind it.
    for start, n in pools:
        for target, lineno, arg in jumps:
            if start <= target < start + n:
                warn(lineno, f'JMP {arg} goes to {target:#x}, which is in the '
                             f'literal pool at {start:#x}')

    size = max(core) + 1 if core else 0
    sizeb = False
    if relocatable and not errors:
        # Direct byte references reach only their own 1024-word byte page,
        # so a module that makes any must be placed inside one; the size
        # code tells the loader which containment it needs.
        sizeb = any(k == 'B11' for k in kinds.values())
        limit = 1024 if sizeb else 2048
        where = end_card[0] if end_card else 0
        if size > limit:
            errors.append(AsmError(
                where,
                f'the module is {size} words; a module must fit one {limit}-word '
                f'page, since its M fields are page offsets'))
        elif set(core) != set(range(size)):
            errors.append(AsmError(where, 'a module is one contiguous stream from word 0'))
        elif entry is None:
            errors.append(AsmError(where, 'a module needs an END naming its entry'))

    Expr.seen, seen = None, Expr.seen
    for name in literals:
        symbols.pop(name, None)
    # A local label is nobody's to name, but a listing may say where it
    # is: under its routine's name, a dot between. A macro's own are left
    # out, one to every call.
    scoped = {}
    for name in [name for name in symbols if LOCAL_MARK in name]:
        scope, _, local = name.partition(LOCAL_MARK)
        if not scope.startswith((TOP_SCOPE, MACRO_SCOPE)):
            scoped.setdefault(f'{scope}.{local}', symbols[name])
        del symbols[name]
    # A cell nothing names is a word of core doing nothing: a label on
    # data that no expression in the deck ever looked up.
    for name, lineno in cells.items():
        if name not in seen and not errors:
            warn(lineno, f'{shown(name).strip(chr(39)) if LOCAL_MARK not in name else shown(name)} is never referred to')

    def order(e):
        where = getattr(e, 'lineno', 0)
        return where.seq if isinstance(where, Loc) else where
    warnings.sort(key=order)
    if errors:
        # Both passes contribute, so put the report back in deck order.
        errors.sort(key=order)
        raise AsmErrorList(errors, listing, symbols, warnings)
    if not core:
        raise AsmError(0, 'nothing was assembled')

    image = bytearray()
    for word in range(size):
        image += core.get(word, 0).to_bytes(2, 'big')
    return {'image': bytes(image), 'listing': listing, 'symbols': symbols,
            'core': core, 'kinds': kinds, 'size': size, 'entry': entry,
            'sizeb': sizeb, 'warnings': warnings, 'scoped': scoped,
            'entries': [(lineno, [(name, symbols[name], *defined[name]) for name in names])
                        for lineno, names in entries]}


# The relocating loader's record: a zero marker, ninety-two bytes of text and
# a checksum, which is one 47-word disc sector.
RECORD_DATA = 92


def object_stream(core, kinds, size, entry, sizeb):
    """The loader text for a module at base 0, framed into records.

    Repeatable codes carry runs of up to sixteen words of one kind; a page
    selection and a reserved run each take a control code and one word; the
    entry closes the stream. Everything after END is padding the loader
    never reads.
    """
    out = bytearray([0x0b if sizeb else 0x0a]) + size.to_bytes(2, 'big')
    addr = 0
    while addr < size:
        kind = kinds[addr]
        run = addr
        while run < size and kinds[run] == kind and run - addr < 16:
            run += 1
        if kind == 'ILOC':
            while run < size and kinds[run] == 'ILOC':
                run += 1
            out += bytes([0x04]) + (run - addr).to_bytes(2, 'big')
        elif kind == 'SMB':
            out += bytes([0x03]) + core[addr].to_bytes(2, 'big')
            run = addr + 1
        else:
            out += bytes([0x80 | (KINDS[kind] << 4) | (run - addr - 1)])
            for a in range(addr, run):
                out += core[a].to_bytes(2, 'big')
        addr = run
    out += bytes([0x06]) + entry.to_bytes(2, 'big')

    records = bytearray()
    for i in range(0, len(out), RECORD_DATA):
        body = bytes([0x00]) + bytes(out[i:i + RECORD_DATA]).ljust(RECORD_DATA, b'\0')
        total = sum(body)
        records += body + bytes([((total >> 8) + total) & 0xff])
    return bytes(records)


def punch_tape(image, origin):
    """Wrap a core image in the frames PTB expects to read off paper tape.

    The bootstrap (drawing 390364) needs the operator to set the index register
    to the load origin minus twelve bytes, because getting its service routine
    to rewrite itself costs twelve frames: the first non-zero frame triggers
    the rewrite, and the eleven after it land in the bytes below the origin.
    So a tape carries eleven-plus-one frames of leader before the program.

    Zero frames ahead of that are blank tape, which PTB skips without counting,
    so a run of them is free and matches what an operator actually threaded
    into the reader.
    """
    if origin * 2 > len(image):
        raise AsmError(0, f'nothing is assembled at or above word {origin:#x}')
    return bytes([0x00] * 8 + [0xff] * 12) + image[origin * 2:]


def write_listing(path, listing, symbols, scoped=None):
    """The listing, and under it the symbol table: every name and its
    value, a local label as ROUTINE.NAME -- which is what a debugger
    reading the table shows a branch target as."""
    table = dict(scoped or {})
    table.update(symbols)
    with open(path, 'w', encoding='utf-8') as f:
        for addr, words, text in listing:
            if words:
                f.write(f'{addr:04X}  {" ".join(f"{w:04X}" for w in words[:4]):<20}{text}\n')
            else:
                f.write(f'{"":4}  {"":<20}{text}\n')
        f.write('\nsymbols:\n')
        for name in sorted(table):
            f.write(f'  {name:<10} {table[name] & 0xffff:04X}\n')


# A number as a card writes one, which an entry deck can say again as written.
NUMBER = re.compile(r"\d+|0[xX][0-9A-Fa-f]+|X'[0-9A-Fa-f]+'")


def comment_of(line):
    """The comment on a card, without its semicolon, or None."""
    quoted = False
    for i, c in enumerate(line):
        if c == '"':
            quoted = not quoted
        elif c == ';' and not quoted:
            return line[i + 1:].strip()
    return None


def write_exports(path, sources, entries):
    """The deck's entries as a deck of EQUs (see the module docstring)."""
    files = {}

    def lines(loc):
        """The source around a card, or nothing for one a macro made."""
        if not isinstance(loc, Loc) or loc.via or not loc.line:
            return []
        if loc.path not in files:
            with open(loc.path, encoding='utf-8') as f:
                files[loc.path] = f.read().split('\n')
        return files[loc.path]

    out = ['; vim: ts=8:sw=8:expandtab:', ';']
    out += textwrap.wrap(
        f'The entries of {", ".join(sources)}: each name an NTRY card there lists, '
        f'at its value and with the comments it has there.  asm703.py --exports '
        f'writes this file.', width=72, initial_indent='; ', subsequent_indent='; ')
    for loc, names in entries:
        src = lines(loc)
        over = loc.line - 1 if src else 0
        while over > 0 and src[over - 1].startswith(';'):
            over -= 1
        if src and over < loc.line - 1:
            out.append('')
            out.extend(src[over:loc.line - 1])
        for name, value, at, arg in names:
            # The value whole: a consumer does arithmetic on it, and minus
            # one is not X'FFFF' until it is a word in core.
            said = f"X'{value:04X}'" if value >= 0 else str(value)
            if arg and NUMBER.fullmatch(arg.strip()):
                said = arg.strip()
            src = lines(at)
            comment = comment_of(src[at.line - 1]) if src else None
            card = f'{name:<15} EQU     {said}'
            out.append(f'{card:<40}; {comment}' if comment else card)
            under = at.line if src else 0
            while src and under < len(src) and src[under][:1].isspace() \
                    and src[under].lstrip().startswith(';'):
                out.append(src[under])
                under += 1
    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(out) + '\n')


def main():
    ap = argparse.ArgumentParser(description='Raytheon 703 assembler')
    ap.add_argument('sources', metavar='source', nargs='+',
                    help='source files, assembled in order as one deck')
    ap.add_argument('-o', '--output', help='flat big-endian core image')
    ap.add_argument('--object', help='write the deck as a relocatable module, in '
                                     'the relocating loader\'s record format')
    ap.add_argument('--org', default='0',
                    help='where the location counter starts (default 0); an '
                         'absolute build of a module source at a chosen base')
    ap.add_argument('-l', '--listing', help='write an address/word/source listing')
    ap.add_argument('-x', '--exports', help='write the names on the deck\'s NTRY cards '
                                            'as a deck of EQUs')
    # Deliberately the same "addr word" shape that xraylist.py --obj emits, so
    # that reassembling a transcribed listing and diffing it against the object
    # code the 1968 assembler printed is a plain `diff` of two sorted files.
    ap.add_argument('-m', '--map', help='write "addr word" lines, one per assembled word')
    ap.add_argument('-t', '--tape', help='also write a PTB-loadable paper tape image')
    # Must agree with PTB_LOAD_ORIGIN in src/system/ray703.rs, which is where
    # the machine presets the index register. A tape is a bare run of frames
    # with no address in it, so it is meaningless anywhere else.
    ap.add_argument('-W', '--werror', action='store_true',
                    help='a warning is an error: nothing is written')
    ap.add_argument('--tape-origin', default='0x100',
                    help='word the tape loads at; must match PTB_LOAD_ORIGIN '
                         'in src/system/ray703.rs (default 0x100)')
    args = ap.parse_args()

    if not (args.output or args.object or args.map or args.listing or args.exports):
        ap.error('nothing to write: give -o, --object, -m, -l or -x')
    if args.object and args.org != '0':
        ap.error('--org places an absolute build; a module is placed by the loader')

    deck = ' '.join(args.sources)
    try:
        r = _assemble(args.sources, origin=int(args.org, 0), relocatable=bool(args.object))
        image, listing, symbols, core, size = (r['image'], r['listing'], r['symbols'],
                                               r['core'], r['size'])
        if args.object:
            stream = object_stream(core, r['kinds'], size, r['entry'], r['sizeb'])
            image = b''.join(core[a].to_bytes(2, 'big') for a in range(size))
        for w in r['warnings']:
            print(w, file=sys.stderr)
        if args.werror and r['warnings']:
            n = len(r['warnings'])
            print(f'{deck}: {n} warning{"s" if n != 1 else ""} under --werror, '
                  f'no image written', file=sys.stderr)
            return 1
    except AsmErrorList as e:
        for err in sorted(e.errors + e.warnings, key=lambda x: (
                x.lineno.seq if isinstance(x.lineno, Loc) else x.lineno)):
            print(err, file=sys.stderr)
        n = len(e.errors)
        print(f'{deck}: {n} error{"s" if n != 1 else ""}, no image written',
              file=sys.stderr)
        # The listing is where a pile of errors actually gets read, so write
        # it even though there is nothing to load.
        if args.listing:
            write_listing(args.listing, e.listing, e.symbols)
        return 1
    except AsmError as e:
        print(e, file=sys.stderr)
        return 1

    if args.output:
        with open(args.output, 'wb') as f:
            f.write(image)

    if args.object:
        with open(args.object, 'wb') as f:
            f.write(stream)

    if args.tape:
        try:
            tape = punch_tape(image, int(args.tape_origin, 0))
        except AsmError as e:
            print(f'{deck}: {e}', file=sys.stderr)
            return 1
        with open(args.tape, 'wb') as f:
            f.write(tape)
        print(f'{deck}: {len(tape)} frames -> {args.tape}')

    if args.map:
        with open(args.map, 'w', encoding='utf-8') as f:
            for addr in sorted(core):
                f.write(f'{addr:04X} {core[addr] & 0xffff:04X}\n')

    if args.listing:
        write_listing(args.listing, listing, symbols, r['scoped'])

    if args.exports:
        if not r['entries']:
            print(f'{deck}: no NTRY card, so nothing to write to {args.exports}', file=sys.stderr)
            return 1
        write_exports(args.exports, args.sources, r['entries'])

    if args.object:
        records = len(stream) // (RECORD_DATA + 2)
        print(f'{deck}: {size} words, {records} records -> {args.object}')
    elif args.output:
        print(f'{deck}: {len(image) // 2} words -> {args.output}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
