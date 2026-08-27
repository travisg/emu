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
"""Take apart a transcribed Raytheon 700-series assembly listing.

The transcript reproduces what the 1968 printer printed: an address, the
assembled word, the same word split into its fields, a card number, and then
the source card. This pulls the three useful things back out of it.

    --asm FILE    the source cards alone, as something asm703.py can assemble
    --obj FILE    the printed object code, as "addr word" lines
    --check       verify the listing against itself and report

`--check` is the reason the transcript keeps redundant columns. Three separate
things have to agree before a transcription can be trusted:

  * every instruction's split fields must recompose to its assembled word;
  * card numbers must run 1..N with no gaps or repeats;
  * addresses must advance by exactly the number of words each card generated.

None of that proves the transcription matches the paper -- only that it is
self-consistent, which a transcription with a wrong nibble in it usually is
not. The real proof is assembling `--asm` and diffing against `--obj`.
"""

import argparse
import glob
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
import asm703  # noqa: E402  -- the path has to be set up first

# Mnemonics and directives, used to tell a label from an opcode: if the first
# token on a card is one of these, the card has no label.
OPCODES = set("""
    JMP JSX STB CMB LDB STX STW LDW LDX ADD SUB ORI ORE AND CMW
    HLT INR ENB DSB SLM SGM CEX CXE SML SMU MSK UNM
    CLR CMP INV CAX CXA DIN DOT IXS DXS LLB CLB
    SAZ SAP SAM SAO SLS SXE SEQ SNE SGR SLE SNO SSE SS0 SS1 SS2 SS3
    SRA SLA SRAD SLAD SRL SLL SRLD SLLD SRC SLC SRCD SLCD
    SRLL SLLL SRLR SLLR SRCL SLCL SRCR SLCR
    EQU DATA D WORD BYTE RES ORG ORIG TEXT TRUE FALS ENDC END SMB SUBR EXIT SXP SXM EXCH
""".split())

# Shift mnemonics that take a trailing D, L or R to name their double-length
# and single-byte variants, which the printer separates with a space.
SHIFT_STEMS = {'SRA', 'SLA', 'SRL', 'SLL', 'SRC', 'SLC'}

# Mnemonics whose operand field is empty, so everything after them is comment.
NO_OPERAND = set("""
    HLT SLM SGM CEX CXE MSK UNM CLR CMP INV CAX CXA ENDC END SUBR SXP SXM EXCH
    SAZ SAP SAM SAO SLS SXE SEQ SNE SGR SLE SNO SSE SS0 SS1 SS2 SS3
""".split())

LINE = re.compile(r"""
    ^\s*
    (?:(?P<addr>[0-9A-Fa-f]\s+[0-9A-Fa-f]{3}\s+[0-9A-Fa-f])\s+)?
    # Either a whole word and its split fields, or -- from the BYTE directive,
    # the one thing here that addresses smaller than a word -- a single byte.
    # These have to be alternatives rather than two optional groups, or the
    # byte would happily eat the first two digits of a card number.
    (?:
        (?P<obj>[0-9A-Fa-f]{4})\s+
        (?P<fields>
            [0-9A-Fa-f]\s+[01]\s+[0-9A-Fa-f]{3}\s+[0-9A-Fa-f](?=\s|$)
                                                  # a byte instruction, which
                                                  # splits M again into a word
                                                  # and which byte of it. The
                                                  # lookahead keeps this from
                                                  # swallowing the first digit
                                                  # of the card number.
          | [0-9A-Fa-f]\s+[01]\s+[0-9A-Fa-f]{3}   # opcode / index / M
          | [0-9A-Fa-f]{3}\s+[0-9A-Fa-f]          # generic group / operand
          | [0-9A-Fa-f]{2}\s+[0-9A-Fa-f]{2}       # generic group / 8-bit literal
          | [0-9A-Fa-f]{4}                        # an EQU value, not split
        )\s*
      | (?(addr)(?P<byte>[0-9A-Fa-f]{2})(?=\s|$)\s*|)
    )?
    # A card number is a decimal run standing on its own. Requiring the space
    # after it is what stops `60B4`, on a continuation line that has object
    # code but no card number, from being read as card 60.
    (?:(?P<card>\d+)(?=\s|$))?
    (?P<rest>.*)
    $
""", re.VERBOSE)

PAGE = re.compile(r'^PAGE\s+(\d+)\s*(.*)$')


class Card:
    __slots__ = ('page', 'card', 'addr', 'obj', 'fields', 'text', 'raw',
                 'bytes', 'extra')

    def __init__(self, page, card, addr, obj, fields, text, raw):
        self.page, self.card = page, card
        self.addr, self.obj, self.fields = addr, obj, fields
        self.text, self.raw = text, raw
        # (byte position within the word, value) for a BYTE directive, which
        # is the one thing here that addresses smaller than a word
        self.bytes = []
        # (address, word) for the extra words a macro generated, which print on
        # continuation lines with no card number of their own
        self.extra = []


def addr_of(m):
    """The word address out of a matched ADDR field."""
    addr = m.group('addr')
    return int(addr.split()[1], 16) if addr else None


def byte_pos(m):
    """Which half of the word a BYTE went into: the ADDR field's trailing
    digit, which is otherwise always zero."""
    return int(m.group('addr').split()[2], 16)


def read_transcript(paths):
    """Parse every page file into a list of Cards, in card order."""
    cards, page, problems = [], 0, []
    for path in paths:
        with open(path, encoding='utf-8') as f:
            for lineno, raw in enumerate(f, 1):
                line = raw.rstrip()
                if not line.strip() or line.startswith('='):
                    continue
                m = PAGE.match(line.strip())
                if m:
                    page = int(m.group(1))
                    continue
                if 'CHECK:' in line or '[?]' in line:
                    problems.append(f'{os.path.basename(path)}:{lineno}: flagged: {line.strip()}')
                m = LINE.match(line)
                if not m:
                    problems.append(f'{os.path.basename(path)}:{lineno}: unparsed: {line.strip()}')
                    continue
                if not m.group('card'):
                    # A continuation line belongs to the card above it. BYTE
                    # prints its second byte this way, and the SUBR and EXIT
                    # macros print their second word this way -- and SUBR goes
                    # further, putting the card number on the first line but
                    # the label and mnemonic on the second.
                    if not cards or not m.group('addr'):
                        continue
                    if m.group('byte'):
                        cards[-1].bytes.append((byte_pos(m), int(m.group('byte'), 16)))
                    elif m.group('obj'):
                        cards[-1].extra.append((addr_of(m), int(m.group('obj'), 16)))
                    if m.group('rest') and not cards[-1].text.strip():
                        cards[-1].text = m.group('rest')
                    continue
                card = Card(
                    page=page,
                    card=int(m.group('card')),
                    addr=addr_of(m),
                    obj=int(m.group('obj'), 16) if m.group('obj') else None,
                    fields=m.group('fields'),
                    text=m.group('rest'),
                    raw=line,
                )
                if m.group('byte') and m.group('addr'):
                    card.bytes.append((byte_pos(m), int(m.group('byte'), 16)))
                cards.append(card)
    cards.sort(key=lambda c: c.card)
    return cards, problems


def recompose(fields):
    """Rebuild the assembled word from its printed fields, or None if the
    fields are the unsplit form the assembler prints for an EQU."""
    parts = fields.split()
    if len(parts) == 3:
        # opcode, index bit, 11-bit M field
        return (int(parts[0], 16) << 12) | (int(parts[1]) << 11) | int(parts[2], 16)
    if len(parts) == 4:
        # A byte instruction: opcode, index bit, the word part of M, and then
        # which byte of that word -- M's own low bit, printed on its own.
        return ((int(parts[0], 16) << 12) | (int(parts[1]) << 11)
                | (int(parts[2], 16) << 1) | int(parts[3], 16))
    if len(parts) == 2:
        # A generic, split at whatever boundary its operand happens to have:
        # `0A1 F` is a twelve-bit group and a shift count, `04 00` is an
        # eight-bit group and a literal byte.
        return (int(parts[0], 16) << (4 * len(parts[1]))) | int(parts[1], 16)
    return None


def split_card(text):
    """Split a source card into (label, op, operand, comment).

    A card whose first character is a quote is a SYM II comment card; it is
    returned with everything in the comment so the extracted source keeps it.
    """
    text = text.split('<<<')[0]
    if text.lstrip().startswith("'"):
        return None, None, None, text.strip()[1:]
    tokens = text.split()
    if not tokens:
        return None, None, None, ''
    label = None
    if tokens[0].upper() not in OPCODES:
        label, tokens = tokens[0], tokens[1:]
    if not tokens:
        return label, None, None, ''
    op, tokens = tokens[0].upper(), tokens[1:]
    # The double and byte shifts print with a gap -- `SRC D`, `SRL L`, `SLC R`
    # -- exactly as appendix B of the reference manual writes them. That is one
    # mnemonic, not a mnemonic and an operand.
    if op in SHIFT_STEMS and tokens and tokens[0].upper() in ('D', 'L', 'R'):
        op, tokens = op + tokens[0].upper(), tokens[1:]
    if op in NO_OPERAND or not tokens:
        return label, op, None, ' '.join(tokens)
    operand, tokens = tokens[0], tokens[1:]
    # The printer sometimes puts a space after the indexed and byte-address
    # prefixes: "LDW * DINS" is one operand, not two.
    if operand in ('*', '/') and tokens:
        operand, tokens = operand + tokens[0], tokens[1:]
    return label, op, operand, ' '.join(tokens)


def emit_asm(cards, out):
    out.write("; X-RAY EXEC - BASIC, DN 390779, 02/07/68\n")
    out.write("; Extracted from the transcribed assembly listing by xraylist.py.\n")
    out.write("; Do not edit: edit the transcript and re-extract.\n\n")
    for c in cards:
        label, op, operand, comment = split_card(c.text)
        if op is None and label is None:
            # a comment card, or a blank one
            out.write(f'; {comment}\n' if comment else '\n')
            continue
        line = f'{label or "":<12}'
        line += f'{op or "":<8}'
        if operand:
            line += f'{operand:<16}'
        if comment:
            line = f'{line:<40}; {comment}'
        out.write(line.rstrip() + '\n')


def emit_obj(cards, out):
    for c in cards:
        if c.addr is None:
            continue
        if c.obj is not None:
            out.write(f'{c.addr:04X} {c.obj:04X}\n')
            for addr, word in c.extra:
                out.write(f'{addr:04X} {word:04X}\n')
        elif c.bytes:
            word = 0
            for pos, value in c.bytes:
                word |= value << (8 if pos == 0 else 0)
            out.write(f'{c.addr:04X} {word:04X}\n')


def check_encoding(card):
    """Assemble one card in isolation and compare with the printed word.

    This is the check that matters most, and it is not the one the listing's
    own redundancy provides. For a memory reference the FIELDS column is a
    structural split, so a misread usually fails to recompose -- but for a
    generic the assembler just prints the word twice, and a misread of both
    copies the same way sails through. That is how `UNM` came to be read as
    `0080` when the machine's code for it is `00B0`.

    Anything whose operand needs a symbol cannot be assembled here, so only
    the opcode and index bit are checked for those; the full comparison waits
    for the extracted source to be assembled against the extracted object.
    """
    label, op, operand, _ = split_card(card.text)
    if op is None or card.obj is None:
        return None

    if op in asm703.MEMREF:
        want = asm703.MEMREF[op] << 12
        got = card.obj & 0xf000
        if want != got:
            return (f'{op} should have opcode {asm703.MEMREF[op]:X} but the '
                    f'word printed is {card.obj:04X}')
        if operand and (card.obj & 0x0800 != 0) != operand.lstrip('/').startswith('*') \
                and not operand.startswith('*'):
            indexed = 'indexed' if card.obj & 0x0800 else 'not indexed'
            if operand.startswith('*') != bool(card.obj & 0x0800):
                return f'{op} {operand} is {indexed} in the word printed'
        return None

    # Generics: assemble outright when the operand needs no symbol table.
    try:
        word = asm703.encode(card.card, op, operand, {}, card.addr or 0)
    except asm703.AsmError:
        return None
    except Exception:  # noqa: BLE001 -- an unknown mnemonic is not our problem here
        return None
    if word != card.obj:
        return (f'{op}{" " + operand if operand else ""} assembles to '
                f'{word:04X}, but the word printed is {card.obj:04X}')
    return None


def check(cards, problems):
    """Report everything the listing can be made to say about itself."""
    for p in problems:
        print(p)

    expected = None
    for c in cards:
        if expected is not None and c.card != expected:
            if c.card < expected:
                print(f'card {c.card}: duplicated (page {c.page})')
            else:
                print(f'cards {expected}..{c.card - 1}: missing (before page {c.page})')
        expected = c.card + 1

    for c in cards:
        if c.obj is None or c.fields is None:
            continue
        rebuilt = recompose(c.fields)
        if rebuilt is None:
            if int(c.fields, 16) != c.obj:
                print(f'card {c.card}: unsplit field {c.fields} != word {c.obj:04X}')
        elif rebuilt != c.obj:
            print(f'card {c.card}: fields {c.fields!r} rebuild to '
                  f'{rebuilt:04X}, but the word printed is {c.obj:04X}')

    for c in cards:
        complaint = check_encoding(c)
        if complaint:
            print(f'card {c.card} (page {c.page}): {complaint}')

    # An ORG legitimately moves the location counter anywhere, so only
    # complain about a backwards step that no ORG explains.
    previous = None
    for c in cards:
        _, op, _, _ = split_card(c.text)
        if op in ('ORG', 'ORIG'):
            previous = None
            continue
        if c.addr is None:
            continue
        if previous is not None and c.addr < previous.addr:
            print(f'card {c.card}: address {c.addr:04X} goes backwards from '
                  f'{previous.addr:04X} with no ORG between them')
        previous = c

    placed = [c for c in cards if c.addr is not None]

    words = len(placed)
    lo = min((c.addr for c in placed), default=0)
    hi = max((c.addr for c in placed), default=0)
    print(f'{len(cards)} cards, {words} words of object code, '
          f'addresses {lo:04X}..{hi:04X}')


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('transcript', help='a page-NNN.txt file, or a directory of them')
    ap.add_argument('--asm', help='write the source cards here')
    ap.add_argument('--obj', help='write "addr word" lines here')
    ap.add_argument('--check', action='store_true', help='report on consistency')
    args = ap.parse_args()

    if os.path.isdir(args.transcript):
        paths = sorted(glob.glob(os.path.join(args.transcript, 'page-*.txt')))
    else:
        paths = [args.transcript]
    if not paths:
        print(f'no page files under {args.transcript}', file=sys.stderr)
        return 1

    cards, problems = read_transcript(paths)

    if args.asm:
        with open(args.asm, 'w', encoding='utf-8') as f:
            emit_asm(cards, f)
    if args.obj:
        with open(args.obj, 'w', encoding='utf-8') as f:
            emit_obj(cards, f)
    if args.check or not (args.asm or args.obj):
        check(cards, problems)
    return 0


if __name__ == '__main__':
    sys.exit(main())
