#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""A reference loader for the 703's relocatable object text.

asm703.py --object writes a module as loader text in the record format of the
1968 relocating loader (DN 390682C; the transcript is test/703/listings/), and
rex/ has a loader that runs on the machine. This one runs on the host and does
exactly what the transcript's PROCTEXT, RELO11 and STORE do, so that the two
can be checked against each other without the emulator: assemble a module
source absolute at some base with `asm703.py --org`, place the object at the
same base here, and every word must agree.

    reload703.py module.obj --base 0x1080 --check module-0x1080.map
    reload703.py module.obj --base 0x1080 --map placed.map
    reload703.py module.obj --dump

The records: a zero marker, ninety-two bytes of text and a checksum which is
the byte sum folded, (sum >> 8) + sum. The marker and the checksum are the
record's framing and never reach the code stream -- GETCARD leaves the byte
pointer on the first byte of text and GETBYTE hands over text up to the
checksum (cards 1553-1563, 1596-1601) -- so a code and its operands run
straight across a record boundary. The codes are listed in asm703.py's
docstring; the loader's own reading of each is cited here by the
transcript's card numbers.
"""

import argparse
import sys

RECORD = 94
WORD_PAGE = 2048
BYTE_PAGE = 1024


class LoadError(Exception):
    pass


def records(data):
    """The text of each record, checked."""
    if len(data) % RECORD:
        raise LoadError(f'{len(data)} bytes is not a whole number of {RECORD}-byte records')
    for i in range(0, len(data), RECORD):
        rec = data[i:i + RECORD]
        if rec[0] != 0:
            # GETCARD, cards 1602-1603: "WAS THE BYTE ZERO -- NO, THIS ISN'T A BINARY RECORD"
            raise LoadError(f'record {i // RECORD} does not open on a zero marker')
        total = sum(rec[:-1])
        if ((total >> 8) + total) & 0xff != rec[-1]:
            # TESTSUM, cards 1616-1620: the sum's high byte folded into it
            raise LoadError(f'record {i // RECORD} fails its checksum')
        yield rec[1:-1]


def stream(data):
    """The text, one byte at a time across the records, GETBYTE's way."""
    for rec in records(data):
        yield from rec


def relo11(word, base):
    """RELO11, cards 1165-1172: the address relocated within the M field and
    the opcode and index bit kept."""
    return (word & 0xf800) | ((word + base) & 0x07ff)


def load(data, base, trace=None):
    """Place the module at `base`: returns (core, entry, size, byte_page)
    with `core` a dict of word address to word."""
    bytes_ = stream(data)

    def getbyte():
        try:
            return next(bytes_)
        except StopIteration:
            raise LoadError('the text ended without an END code') from None

    def getword():
        hi = getbyte()
        return (hi << 8) | getbyte()

    core = {}
    pointer = None
    limit = None
    byte_page = False

    def store(word):
        nonlocal pointer
        if pointer is None:
            raise LoadError('text before the SIZE code')
        if pointer >= limit:
            raise LoadError(f'the text overruns the {limit - base} words the SIZE code declared')
        core[pointer] = word & 0xffff
        pointer += 1

    while True:
        code = getbyte()
        if trace is not None:
            trace.append((code, pointer))
        if code & 0x80:
            # repeatable: the low nibble counts n+1 words (REPEATER, cards 561-568)
            kind, count = code >> 4, (code & 0x0f) + 1
            for _ in range(count):
                w = getword()
                if kind == 0x8:      # RELW11
                    store(relo11(w, base))
                elif kind == 0x9:    # RELW15
                    store(w + base)
                elif kind == 0xa:    # RELB16, cards 586-588: BASE added twice
                    store(w + base + base)
                elif kind == 0xb:    # RELB11: RELO11 twice
                    store(relo11(relo11(w, base), base))
                elif kind == 0xc:    # ABSO
                    store(w)
                else:
                    raise LoadError(f'loader code {code:#04x} is not one this loader takes (LC)')
            continue
        if code == 0x00:
            continue
        if code in (0x01, 0x08):
            # a name: GETNAME reads four words and keeps two (cards 1534-1547)
            for _ in range(8):
                getbyte()
            continue
        if code == 0x03:
            # SMB, cards 744-748: ADD BASE / SRL 10 / ORI X'80'
            store(((getword() + base) >> 10) | 0x80)
            continue
        if code == 0x04:
            # ILOC, cards 753-769: that many zeros
            for _ in range(getword()):
                store(0)
            continue
        if code == 0x06:
            entry = getword() + base
            break
        if code in (0x09, 0x0a, 0x0b):
            if pointer is not None:
                raise LoadError('a second SIZE code')
            size = getword()
            byte_page = code == 0x0b
            window = BYTE_PAGE if byte_page else WORD_PAGE
            if base // window != (base + size - 1) // window:
                raise LoadError(
                    f'{size} words at {base:#x} straddle a {window}-word page; '
                    f'the module must be contained in one')
            pointer, limit = base, base + size
            continue
        raise LoadError(f'loader code {code:#04x} is not one this loader takes (LC)')

    if pointer is None:
        raise LoadError('no SIZE code')
    return core, entry, limit - base, byte_page


def dump(data):
    """The stream as codes, for reading."""
    bytes_ = stream(data)
    getbyte = lambda: next(bytes_)
    getword = lambda: (getbyte() << 8) | getbyte()
    names = {0x8: 'W11', 0x9: 'W15', 0xa: 'B16', 0xb: 'B11', 0xc: 'ABSO'}
    while True:
        try:
            code = getbyte()
        except StopIteration:
            return
        if code & 0x80:
            words = [getword() for _ in range((code & 0x0f) + 1)]
            print(f'{names.get(code >> 4, "??"):5} {" ".join(f"{w:04X}" for w in words)}')
        elif code == 0:
            continue
        elif code in (0x01, 0x08):
            print('NAME ', ' '.join(f'{getword():04X}' for _ in range(4)))
        elif code == 0x03:
            print(f'SMB   {getword():04X}')
        elif code == 0x04:
            print(f'ILOC  {getword()}')
        elif code == 0x06:
            print(f'END   {getword():04X}')
            return
        elif code in (0x09, 0x0a, 0x0b):
            print(f'{"SIZEB" if code == 0x0b else "SIZEW"} {getword()}')
        else:
            print(f'??    {code:02X}')
            return


def read_map(path):
    core = {}
    with open(path, encoding='utf-8') as f:
        for line in f:
            addr, word = line.split()
            core[int(addr, 16)] = int(word, 16)
    return core


def main():
    ap = argparse.ArgumentParser(description='reference loader for 703 relocatable object text')
    ap.add_argument('object', help='the module, as asm703.py --object writes it')
    ap.add_argument('--base', default='0', help='word address to place the module at')
    ap.add_argument('--map', help='write the placed module as "addr word" lines')
    ap.add_argument('--check', metavar='MAP',
                    help='compare the placed module against an "addr word" map, '
                         'as asm703.py -m writes for an absolute build at the same base')
    ap.add_argument('--dump', action='store_true', help='print the stream as codes')
    args = ap.parse_args()

    with open(args.object, 'rb') as f:
        data = f.read()
    base = int(args.base, 0)
    try:
        if args.dump:
            dump(data)
            return 0
        core, entry, size, byte_page = load(data, base)
    except LoadError as e:
        print(f'{args.object}: {e}', file=sys.stderr)
        return 1

    if args.map:
        with open(args.map, 'w', encoding='utf-8') as f:
            for addr in sorted(core):
                f.write(f'{addr:04X} {core[addr]:04X}\n')

    if args.check:
        want = read_map(args.check)
        bad = 0
        for addr in sorted(set(core) | set(want)):
            if addr not in want:
                print(f'{addr:04X}: loaded {core[addr]:04X}, the absolute build has nothing there')
            elif addr not in core:
                print(f'{addr:04X}: the absolute build has {want[addr]:04X}, the loader placed nothing')
            elif core[addr] != want[addr]:
                print(f'{addr:04X}: loaded {core[addr]:04X}, the absolute build has {want[addr]:04X}')
            else:
                continue
            bad += 1
        print(f'{args.object} at {base:#x}: {size} words, entry {entry:#x}, '
              f'{len(core)} compared, {bad} mismatch{"es" if bad != 1 else ""}')
        return 1 if bad else 0

    print(f'{args.object} at {base:#x}: {size} words, entry {entry:#x}'
          f'{", byte page" if byte_page else ""}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
