#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Permission is hereby granted, free of charge, to any person obtaining
# a copy of this software and associated documentation files
# (the "Software"), to deal in the Software without restriction,
# including without limitation the rights to use, copy, modify, merge,
# publish, distribute, sublicense, and/or sell copies of the Software,
# and to permit persons to whom the Software is furnished to do so,
# subject to the following conditions:
#
# The above copyright notice and this permission notice shall be
# included in all copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
# EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
# MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
# IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
# CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
# TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
# SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
"""A small two-pass assembler for the Raytheon 703.

The 703 predates anything this tree could reuse, and its own assembler (SYM II)
exists only as scans, so the demo image needs something to build it. This is
deliberately minimal -- no macros, no relocation, no object format -- because
the only thing it has to produce is an absolute core image.

Output is a flat binary of big-endian words starting at word 0, which is what
`emu -s ray703` loads into core.

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
"""

import argparse
import re
import sys

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
DIRECTIVES = {'ORG', 'ORIG', 'WORD', 'DATA', 'D', 'BYTE', 'TEXT', 'RES', 'EQU',
              'TRUE', 'FALS', 'ENDC', 'END'}

# Mnemonics found only in the period listings, never in the reference manual,
# because they are the assembler's rather than the machine's. Each is given
# here with the word it actually produced in the X-RAY listing.
#
# SXP is `IXS 0` and SXM is `DXS 0`: change the index by nothing and skip on
# its sign. The literal forms are loop steps; under these names they are what
# they are being used for, a test of the index.
GEN_ALIAS = {'SXP': 0x0400, 'SXM': 0x0500}

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


class AsmError(Exception):
    def __init__(self, lineno, message):
        super().__init__(f'line {lineno}: {message}')


class UndefinedSymbol(AsmError):
    """Raised on its own so that EQU can retry a forward reference."""


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
    """

    TOKEN = re.compile(r"""
        \s*(?:
            (?P<hexc>X'[0-9A-Fa-f]+')
          | (?P<hex>0[xX][0-9A-Fa-f]+)
          | (?P<char>'(?:\\.|[^'])(?:\\.|[^'])?')
          | (?P<num>\d+)
          | (?P<name>[A-Za-z_$][A-Za-z0-9_$.]*)
          | (?P<op>[-+*()])
        )
    """, re.VERBOSE)

    def __init__(self, text, symbols, here, lineno):
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
        self.here = here
        self.lineno = lineno

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self):
        tok = self.peek()
        self.pos += 1
        return tok

    def value(self):
        v = self.sum()
        if self.peek() is not None:
            raise AsmError(self.lineno, f'trailing {self.peek()!r} in expression')
        return v

    def sum(self):
        v = self.product()
        while self.peek() in ('+', '-'):
            op = self.take()
            rhs = self.product()
            v = v + rhs if op == '+' else v - rhs
        return v

    def product(self):
        v = self.unary()
        while self.peek() == '*':
            self.take()
            v *= self.unary()
        return v

    def unary(self):
        if self.peek() == '-':
            self.take()
            return -self.unary()
        return self.atom()

    def atom(self):
        tok = self.take()
        if tok is None:
            raise AsmError(self.lineno, 'expression ended early')
        if tok == '(':
            v = self.sum()
            if self.take() != ')':
                raise AsmError(self.lineno, 'unbalanced parentheses')
            return v
        if tok.startswith("X'"):
            return int(tok[2:-1], 16)
        if tok.lower().startswith('0x'):
            return int(tok, 16)
        if tok.startswith("'"):
            return char_literal(self.lineno, tok[1:-1])
        if tok[0].isdigit():
            return int(tok, 10)
        if tok == '$':
            return self.here
        if tok not in self.symbols:
            raise UndefinedSymbol(self.lineno, f'undefined symbol {tok!r}')
        return self.symbols[tok]


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


def parse(path):
    """Split the source into (lineno, label, mnemonic, operand, text) tuples."""
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
            out.append((lineno, label, op, arg, text))
    return out


def operand_list(arg):
    """Split a comma-separated operand list, ignoring commas inside quotes.

    `D ','` is one operand, not two empty ones -- a comma is a perfectly good
    character constant.
    """
    parts, depth, start = [], False, 0
    for i, c in enumerate(arg):
        if c == "'":
            depth = not depth
        elif c == ',' and not depth:
            parts.append(arg[start:i])
            start = i + 1
    parts.append(arg[start:])
    return parts


def sizeof(lineno, op, arg):
    """How many words a statement occupies."""
    if op is None:
        return 0
    if op in ('EQU', 'ORG', 'ORIG', 'TRUE', 'FALS', 'ENDC', 'END'):
        return 0
    if op in SUBR_MACROS or op == 'EXCH':
        return 2
    if op in ('WORD', 'DATA', 'D'):
        return len(operand_list(arg))
    if op == 'BYTE':
        # two bytes to the word, rounded up
        return (len(operand_list(arg)) + 1) // 2
    if op == 'TEXT':
        return None  # needs the decoded string; handled by the caller
    if op == 'RES':
        return None
    return 1


def string_body(lineno, arg):
    if arg is None or not arg.startswith('"') or not arg.endswith('"') or len(arg) < 2:
        raise AsmError(lineno, 'TEXT needs a double-quoted string')
    return unescape(arg[1:-1], lineno)


def encode(lineno, op, arg, symbols, here):
    """Assemble one instruction into a single word."""

    def value(text, limit=None):
        v = Expr(text, symbols, here, lineno).value()
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
        scale = 1
        if text.startswith('/'):
            # the PTB listing's "STB /TEST": the byte address of a word label
            text, scale = text[1:], 2
        addr = value(text) * scale
        if not 0 <= addr <= 0x7ff:
            unit = 'byte' if op in BYTE_REF else 'word'
            raise AsmError(
                lineno,
                f'{unit} address {addr:#x} is outside the 11-bit M field; '
                f'the 703 reaches the rest of core through EXR (SML/SMU)')
        return (MEMREF[op] << 12) | (0x0800 if indexed else 0) | addr

    if op in GEN_ALIAS:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_ALIAS[op]

    if op in GEN_NONE:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_NONE[op]

    if op in GEN_LEVEL:
        return GEN_LEVEL[op] | value(arg, 15)

    if op in GEN_PAGE:
        return GEN_PAGE[op] | value(arg, 15)

    if op == SELECT_BASE:
        # EXR holds a five-bit byte page. A word address's byte page is the
        # top five bits of twice it, so word >> 10; SML reaches the lower
        # sixteen pages and SMU the upper sixteen.
        page = (value(arg) >> 10) & 0x1f
        if page < 16:
            return GEN_PAGE['SML'] | page
        return GEN_PAGE['SMU'] | (page - 16)

    if op in GEN_LITERAL:
        v = value(arg)
        if not -128 <= v <= 255:
            raise AsmError(lineno, f'{op} literal {v} does not fit in a byte')
        return GEN_LITERAL[op] | (v & 0xff)

    if op in GEN_DIO:
        if arg is None or ',' not in arg:
            raise AsmError(lineno, f'{op} needs "device,function"')
        dev, func = arg.split(',', 1)
        return GEN_DIO[op] | (value(dev, 15) << 4) | value(func, 15)

    if op in GEN_SKIP:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_SKIP[op]

    if op in GEN_SHIFT:
        return GEN_SHIFT[op] | value(arg, 15)

    raise AsmError(lineno, f'unknown mnemonic {op!r}')


def assemble(path):
    statements = parse(path)

    # Pass 1: place every statement and collect the labels.
    symbols = {}
    placed = []
    here = 0
    # Stack of "are we assembling?" flags, one per open TRUE/FALS. Conditions
    # are evaluated here in pass 1, which is why the configuration equates have
    # to come before the code they configure -- as they do in the X-RAY
    # listing, which states the whole system description on its fifth page.
    cond = []
    # EQUs whose operand named something not yet defined; settled after the pass.
    deferred = []
    for lineno, label, op, arg, text in statements:
        if op in CONDITIONALS:
            if op == 'ENDC':
                if not cond:
                    raise AsmError(lineno, 'ENDC without TRUE or FALS')
                cond.pop()
            elif all(cond):
                taken = condition(arg, symbols, here, lineno)
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
            placed.append((lineno, here, op, arg, text))
            break
        if op == 'EQU':
            if not label:
                raise AsmError(lineno, 'EQU needs a label')
            try:
                symbols[label] = Expr(arg or '', symbols, here, lineno).value()
            except UndefinedSymbol:
                # SYM II allowed an EQU to name a symbol defined further down
                # the deck -- the X-RAY listing's card 298 is
                # `MAXP EQU ENDP-PEAT+12`, and both of those are defined much
                # later. Set it aside and settle it once the pass has seen
                # every label. An EQU that some *later* EQU depends on still
                # resolves, because the leftovers are iterated to a fixpoint.
                deferred.append((lineno, label, arg, here))
            placed.append((lineno, here, op, arg, text))
            continue
        if label:
            if label in symbols:
                raise AsmError(lineno, f'{label!r} is defined twice')
            # SUBR lays down a return slot and then the STX that fills it; the
            # name belongs to the STX, so that callers jump to code.
            symbols[label] = here + 1 if op == 'SUBR' else here
        if op in ('ORG', 'ORIG'):
            here = Expr(arg or '', symbols, here, lineno).value()
            placed.append((lineno, here, op, arg, text))
            continue
        placed.append((lineno, here, op, arg, text))
        if op == 'TEXT':
            body = string_body(lineno, arg)
            if len(body) % 2:
                raise AsmError(
                    lineno,
                    f'TEXT is {len(body)} bytes; a word is two bytes, so pad it '
                    f'to an even length rather than let the assembler guess')
            here += len(body) // 2
        elif op == 'RES':
            here += Expr(arg or '', symbols, here, lineno).value()
        else:
            here += sizeof(lineno, op, arg)

    # Settle the deferred EQUs. Each sweep that resolves anything may unblock
    # another, so sweep until one achieves nothing: either they are all done or
    # what is left is a genuine undefined symbol (or a cycle), and reporting
    # the first of those is more useful than reporting how many there were.
    while deferred:
        rest = []
        for lineno, label, arg, at in deferred:
            try:
                symbols[label] = Expr(arg or '', symbols, at, lineno).value()
            except UndefinedSymbol as err:
                rest.append((lineno, label, arg, at, err))
        if len(rest) == len(deferred):
            raise rest[0][4]
        deferred = [item[:4] for item in rest]

    # Pass 2: emit.
    core = {}
    listing = []
    for lineno, addr, op, arg, text in placed:
        words = []
        if op in ('WORD', 'DATA', 'D'):
            for part in operand_list(arg):
                words.append(Expr(part, symbols, addr + len(words), lineno).value() & 0xffff)
        elif op == 'TEXT':
            body = string_body(lineno, arg)
            words = [(body[i] << 8) | body[i + 1] for i in range(0, len(body), 2)]
        elif op == 'EXCH':
            words = [EXCH_WORD, EXCH_WORD]
        elif op == 'SUBR':
            words = [0x0000, (MEMREF['STX'] << 12) | (addr & 0x07ff)]
        elif op == 'EXIT':
            entry = Expr(arg or '', symbols, addr, lineno).value()
            words = [(MEMREF['LDX'] << 12) | ((entry - 1) & 0x07ff),
                     (MEMREF['JSX'] << 12) | 0x0800]
        elif op == 'BYTE':
            # Two bytes to a word, high half first, as everywhere else on this
            # machine. An odd count leaves the low half zero.
            vals = [Expr(part, symbols, addr, lineno).value() & 0xff
                    for part in operand_list(arg)]
            if len(vals) % 2:
                vals.append(0)
            words = [(vals[i] << 8) | vals[i + 1] for i in range(0, len(vals), 2)]
        elif op == 'RES':
            words = [0] * Expr(arg or '', symbols, addr, lineno).value()
        elif op in ('ORG', 'ORIG', 'EQU', 'TRUE', 'FALS', 'ENDC', 'END', None):
            words = []
        else:
            words = [encode(lineno, op, arg, symbols, addr)]

        for i, w in enumerate(words):
            if addr + i in core:
                raise AsmError(lineno, f'word {addr + i:#x} is assembled twice')
            core[addr + i] = w
        listing.append((addr, words, text))

    if not core:
        raise AsmError(0, 'nothing was assembled')

    top = max(core)
    image = bytearray()
    for word in range(top + 1):
        image += core.get(word, 0).to_bytes(2, 'big')
    return bytes(image), listing, symbols, core


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


def main():
    ap = argparse.ArgumentParser(description='Raytheon 703 assembler')
    ap.add_argument('source')
    ap.add_argument('-o', '--output', required=True, help='flat big-endian core image')
    ap.add_argument('-l', '--listing', help='write an address/word/source listing')
    # Deliberately the same "addr word" shape that xraylist.py --obj emits, so
    # that reassembling a transcribed listing and diffing it against the object
    # code the 1968 assembler printed is a plain `diff` of two sorted files.
    ap.add_argument('-m', '--map', help='write "addr word" lines, one per assembled word')
    ap.add_argument('-t', '--tape', help='also write a PTB-loadable paper tape image')
    # Must agree with PTB_LOAD_ORIGIN in src/system/ray703.rs, which is where
    # the machine presets the index register. A tape is a bare run of frames
    # with no address in it, so it is meaningless anywhere else.
    ap.add_argument('--tape-origin', default='0x100',
                    help='word the tape loads at; must match PTB_LOAD_ORIGIN '
                         'in src/system/ray703.rs (default 0x100)')
    args = ap.parse_args()

    try:
        image, listing, symbols, core = assemble(args.source)
    except AsmError as e:
        print(f'{args.source}: {e}', file=sys.stderr)
        return 1

    with open(args.output, 'wb') as f:
        f.write(image)

    if args.tape:
        try:
            tape = punch_tape(image, int(args.tape_origin, 0))
        except AsmError as e:
            print(f'{args.source}: {e}', file=sys.stderr)
            return 1
        with open(args.tape, 'wb') as f:
            f.write(tape)
        print(f'{args.source}: {len(tape)} frames -> {args.tape}')

    if args.map:
        with open(args.map, 'w', encoding='utf-8') as f:
            for addr in sorted(core):
                f.write(f'{addr:04X} {core[addr] & 0xffff:04X}\n')

    if args.listing:
        with open(args.listing, 'w', encoding='utf-8') as f:
            for addr, words, text in listing:
                if words:
                    f.write(f'{addr:04X}  {" ".join(f"{w:04X}" for w in words[:4]):<20}{text}\n')
                else:
                    f.write(f'{"":4}  {"":<20}{text}\n')
            f.write('\nsymbols:\n')
            for name in sorted(symbols):
                f.write(f'  {name:<10} {symbols[name] & 0xffff:04X}\n')

    print(f'{args.source}: {len(image) // 2} words -> {args.output}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
