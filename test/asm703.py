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

DIRECTIVES = {'ORG', 'WORD', 'TEXT', 'RES', 'EQU'}

TEXT_ESCAPES = {'r': 0x0d, 'n': 0x0a, 't': 0x09, '0': 0x00, '\\': 0x5c, '"': 0x22}


class AsmError(Exception):
    def __init__(self, lineno, message):
        super().__init__(f'line {lineno}: {message}')


class Expr:
    """Expression evaluator over labels and integer literals.

    Recursive descent rather than eval(), so a typo in the source is an
    assembler error rather than a Python traceback.
    """

    TOKEN = re.compile(r"""
        \s*(?:
            (?P<hexc>X'[0-9A-Fa-f]+')
          | (?P<hex>0[xX][0-9A-Fa-f]+)
          | (?P<char>'(?:\\.|[^'])')
          | (?P<num>\d+)
          | (?P<name>[A-Za-z_$][A-Za-z0-9_$]*)
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
            body = tok[1:-1]
            if body.startswith('\\'):
                return TEXT_ESCAPES.get(body[1], ord(body[1]))
            return ord(body)
        if tok[0].isdigit():
            return int(tok, 10)
        if tok == '$':
            return self.here
        if tok not in self.symbols:
            raise AsmError(self.lineno, f'undefined symbol {tok!r}')
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


def parse(path):
    """Split the source into (lineno, label, mnemonic, operand, text) tuples."""
    out = []
    with open(path, encoding='utf-8') as f:
        for lineno, raw in enumerate(f, 1):
            text = raw.rstrip('\n')
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
            out.append((lineno, label, op.upper() if op else None, arg, text))
    return out


def sizeof(lineno, op, arg):
    """How many words a statement occupies."""
    if op is None:
        return 0
    if op == 'EQU' or op == 'ORG':
        return 0
    if op == 'WORD':
        return len(arg.split(','))
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

    if op in GEN_NONE:
        if arg:
            raise AsmError(lineno, f'{op} takes no operand')
        return GEN_NONE[op]

    if op in GEN_LEVEL:
        return GEN_LEVEL[op] | value(arg, 15)

    if op in GEN_PAGE:
        return GEN_PAGE[op] | value(arg, 15)

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
    for lineno, label, op, arg, text in statements:
        if op == 'EQU':
            if not label:
                raise AsmError(lineno, 'EQU needs a label')
            symbols[label] = Expr(arg or '', symbols, here, lineno).value()
            placed.append((lineno, here, op, arg, text))
            continue
        if label:
            if label in symbols:
                raise AsmError(lineno, f'{label!r} is defined twice')
            symbols[label] = here
        if op == 'ORG':
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

    # Pass 2: emit.
    core = {}
    listing = []
    for lineno, addr, op, arg, text in placed:
        words = []
        if op == 'WORD':
            for part in arg.split(','):
                words.append(Expr(part, symbols, addr + len(words), lineno).value() & 0xffff)
        elif op == 'TEXT':
            body = string_body(lineno, arg)
            words = [(body[i] << 8) | body[i + 1] for i in range(0, len(body), 2)]
        elif op == 'RES':
            words = [0] * Expr(arg or '', symbols, addr, lineno).value()
        elif op in ('ORG', 'EQU', None):
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
    return bytes(image), listing, symbols


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
    ap.add_argument('-t', '--tape', help='also write a PTB-loadable paper tape image')
    ap.add_argument('--tape-origin', default='0x100',
                    help='word the tape loads at; must match the machine (default 0x100)')
    args = ap.parse_args()

    try:
        image, listing, symbols = assemble(args.source)
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
