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

A module may be several segments, each placed by itself: the text then
opens with a SIZE code for each, and takes a base for each here, commas
between (`--base 0x1080,0x4100`). Two codes past the 1968 loader's twelve
say which segment is meant: 0C, that the text from here on is that
segment's, to be stored from its first word and relocated by its base; and
0D, that the words from here on are relocated by that segment's base
instead -- they hold addresses in it. A module of one segment has neither.

A library is module after module on one tape, each naming what it defines
(NTRY) and what it wants from the others (EXTN). `--library` places every
module on the tape back to back from a word address and fills the
references between them, as the loader's EXTN, NTRY and STRING11 do, and
writes what it placed as a core image and a list of the names:

    reload703.py 392325.bin --library 0x800 --image math.core --symbols math.sym

An external has no word of its own in the text. Each word that refers to
one holds, where its address belongs, the address of the next such word,
and the EXTN code gives the first: a chain through the module, ended by an
address field of all ones. Filling it puts the definition in each word by
the word's kind (cards 1087-1127): a word with no operation code is an SMB
for the definition's page, a store-byte word becomes the definition's
fifteen bits under the word's own index bit, and any other keeps its
operation code and index bit over the definition's low eleven.

Two things here are not the loader's. It takes the modules a program asks
for, in the order they come; this takes them all, and puts the ones with
byte references first, so that a library placed at the bottom of a word
page has those in the page's first 1,024 words, where a byte address formed
in the module's own page can reach them. And on paper tape each record is
framed as an absolute tape's are -- 8A, a count of 94, the record, 8D --
with whatever is punched between records, a legend or a BELL, to be passed
over; `--library` takes a tape either way.

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


def unframe(frames):
    """The records off a paper tape, end to end: each is 8A, its length in
    two frames, itself and 8D. A file with no such record is returned as it
    is, the records already end to end."""
    out = bytearray()
    i = 0
    head = b'\x8a' + RECORD.to_bytes(2, 'big')
    while True:
        i = frames.find(head, i)
        if i < 0:
            break
        end = i + len(head) + RECORD
        if frames[end:end + 1] == b'\x8d':
            out += frames[i + len(head):end]
            i = end + 1
        else:
            i += 1
    return bytes(out) if out else frames


def name(text):
    """A name as the text has it: eight characters, the eighth bit set."""
    return bytes(c & 0x7f for c in text).decode('ascii', 'replace').rstrip()


def relo11(word, base):
    """RELO11, cards 1165-1172: the address relocated within the M field and
    the opcode and index bit kept."""
    return (word & 0xf800) | ((word + base) & 0x07ff)


def load(data, bases, trace=None, names=None):
    """Place the module with its segments at `bases`: returns (core, entry,
    sizes, byte_pages) with `core` a dict of word address to word and one
    size and one byte-page flag to a segment.

    `data` is the records, or an iterator over their text that is left
    after the module's END code, for a tape of several. `names`, a dict,
    is given the module's 'library' and 'block' names, its 'entries' as
    (name, address) and its 'externals' as (name, address of the chain's
    first word, or None for a name with no chain); without it a module
    with entries or externals is refused."""
    if isinstance(bases, int):
        bases = [bases]
    bytes_ = stream(data) if isinstance(data, (bytes, bytearray)) else data
    if names is not None:
        names.update(library=[], block=None, entries=[], externals=[])

    def getbyte():
        try:
            return next(bytes_)
        except StopIteration:
            raise LoadError('the text ended without an END code') from None

    def getword():
        hi = getbyte()
        return (hi << 8) | getbyte()

    core = {}
    sizes = []
    byte_pages = []
    pointer = None
    limit = None
    base = 0                    # what is added to a location: BASE

    def store(word):
        nonlocal pointer
        if pointer is None:
            raise LoadError('text before the SIZE code')
        if pointer >= limit:
            raise LoadError('the text overruns the words its SIZE code declared')
        if pointer in core:
            raise LoadError(f'word {pointer:#x} is stored twice')
        core[pointer] = word & 0xffff
        pointer += 1

    def segment():
        seg = getword()
        if seg >= len(sizes):
            raise LoadError(f'segment {seg} has no SIZE code')
        return seg

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
            text = name(getbyte() for _ in range(8))
            if names is not None and code == 0x01:
                names['library'].append(text)
            elif names is not None:
                names['block'] = text
            continue
        if code in (0x02, 0x05) and names is not None:
            # EXTN and NTRY: a name and a value, which GETVAL relocates
            # unless it is all ones (cards 1430-1439)
            text = name(getbyte() for _ in range(8))
            value = getword()
            if code == 0x05:
                names['entries'].append((text, (value + base) & 0x7fff))
            else:
                names['externals'].append((text, None if value == 0xffff else (value + base) & 0x7fff))
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
            # SIZE, cards 886-902: the storage, and the relocation base. One
            # to a segment, all of them before any text; the first segment
            # is the one the text begins in.
            if core:
                raise LoadError('a SIZE code after text')
            if len(sizes) == len(bases):
                raise LoadError(f'the module has more than {len(bases)} '
                                f'segment{"s" if len(bases) != 1 else ""}: --base wants '
                                f'an address for each')
            size = getword()
            at = bases[len(sizes)]
            window = BYTE_PAGE if code == 0x0b else WORD_PAGE
            if at // window != (at + size - 1) // window:
                raise LoadError(
                    f'{size} words at {at:#x} straddle a {window}-word page; '
                    f'the module must be contained in one')
            sizes.append(size)
            byte_pages.append(code == 0x0b)
            if pointer is None:
                pointer, limit, base = at, at + size, at
            continue
        if code == 0x0c:
            seg = segment()
            base = pointer = bases[seg]
            limit = pointer + sizes[seg]
            continue
        if code == 0x0d:
            base = bases[segment()]
            continue
        raise LoadError(f'loader code {code:#04x} is not one this loader takes (LC)')

    if pointer is None:
        raise LoadError('no SIZE code')
    return core, entry, sizes, byte_pages


class Text:
    """The text of a tape of modules, with a look ahead for the next."""

    def __init__(self, data):
        self.text = bytes(stream(data))
        self.at = 0

    def __iter__(self):
        return self

    def __next__(self):
        if self.at >= len(self.text):
            raise StopIteration
        self.at += 1
        return self.text[self.at - 1]

    def another(self):
        """Whether a module follows, passing over the zeros before it."""
        while self.at < len(self.text) and self.text[self.at] == 0:
            self.at += 1
        return self.at < len(self.text)


def string11(core, head, value, where):
    """STRING11, cards 1087-1127: follow the chain from the word at `head`
    and put `value` in each word of it. Returns how many it filled."""
    block = head & 0x7800
    link = head
    filled = 0
    while link & 0x7ff != 0x7ff:
        addr = block | (link & 0x7ff)
        if addr not in core or filled > len(core):
            raise LoadError(f'{where}: the chain leaves the text at {addr:#x}')
        item = link = core[addr]
        if item & 0xf800 == 0:
            core[addr] = 0x80 | (value >> 10)                       # SMBDEF
        elif item >> 12 == 3:
            core[addr] = ((item & 0x0800) << 4) | (value & 0x7fff)  # DEF15, the X bit its sign
        else:
            core[addr] = (item & 0xf800) | (value & 0x7ff)          # DEF11
        filled += 1
    return filled


def library(data, start):
    """Place every module of a library from `start` on and fill their
    references to each other: returns (core, symbols, modules), `symbols`
    a dict of name to address and `modules` a list of (names, base, size)
    in the order placed."""
    # once through at word 0 for the sizes, and for which have byte
    # references, which go first
    found = []
    text = Text(data)
    while text.another():
        codes = []
        _, _, sizes, _ = load(text, [0], trace=codes, names={})
        if len(sizes) != 1:
            raise LoadError('a library module of more than one segment')
        found.append((sizes[0], any(code >> 4 in (0xa, 0xb) for code, _ in codes)))
    order = sorted(range(len(found)), key=lambda i: not found[i][1])
    bases = {}
    at = start
    for i in order:
        bases[i] = at
        at += found[i][0]

    core = {}
    symbols = {}
    placed = {}
    text = Text(data)
    for i in range(len(found)):
        names = {}
        module, _, _, _ = load(text, [bases[i]], names=names)
        core.update(module)
        for entry, addr in names['entries']:
            if entry in symbols:
                raise LoadError(f'{entry} is defined twice')
            symbols[entry] = addr
        placed[i] = (names, bases[i], found[i][0])
    for names, _, _ in placed.values():
        for external, head in names['externals']:
            if external not in symbols:
                raise LoadError(f'{external} is defined nowhere on the tape')
            if head is not None:
                string11(core, head, symbols[external], external)
    return core, symbols, [placed[i] for i in order]


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
        elif code == 0x0c:
            print(f'SEG   {getword()}')
        elif code == 0x0d:
            print(f'RELOC {getword()}')
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
    ap.add_argument('--base', default='0', help='word address to place the module at, or '
                                                'one for each of its segments, commas between')
    ap.add_argument('--map', help='write the placed module as "addr word" lines')
    ap.add_argument('--check', metavar='MAP',
                    help='compare the placed module against an "addr word" map, '
                         'as asm703.py -m writes for an absolute build at the same base')
    ap.add_argument('--dump', action='store_true', help='print the stream as codes')
    ap.add_argument('--library', metavar='ADDR',
                    help='the object is a library, on paper tape or not: place every module '
                         'from this word address on and fill the references between them')
    ap.add_argument('--image', help='with --library, write a core image from word 0')
    ap.add_argument('--symbols', help='with --library, write the names as "addr name" lines')
    args = ap.parse_args()

    with open(args.object, 'rb') as f:
        data = f.read()
    if args.library:
        try:
            core, symbols, modules = library(unframe(data), int(args.library, 0))
        except LoadError as e:
            print(f'{args.object}: {e}', file=sys.stderr)
            return 1
        for names, base, size in modules:
            print(f'{base:04X} {size:4} {"/".join(names["library"])}')
        print(f'{args.object}: {len(modules)} modules, {len(core)} words at '
              f'{min(core):#x}-{max(core):#x}, {len(symbols)} names')
        if args.image:
            image = bytearray(2 * (max(core) + 1))
            for addr, word in core.items():
                image[2 * addr:2 * addr + 2] = word.to_bytes(2, 'big')
            with open(args.image, 'wb') as f:
                f.write(image)
        if args.symbols:
            with open(args.symbols, 'w', encoding='utf-8') as f:
                for sym, addr in sorted(symbols.items(), key=lambda kv: (kv[1], kv[0])):
                    f.write(f'{addr:04X} {sym}\n')
        return 0
    bases = [int(base, 0) for base in args.base.split(',')]
    try:
        if args.dump:
            dump(data)
            return 0
        core, entry, sizes, byte_pages = load(data, bases)
    except LoadError as e:
        print(f'{args.object}: {e}', file=sys.stderr)
        return 1
    if len(sizes) < len(bases):
        print(f'{args.object}: the module has {len(sizes)} segment{"s" if len(sizes) != 1 else ""}, '
              f'and --base gave {len(bases)} addresses', file=sys.stderr)
        return 1
    where = ' and '.join(f'{base:#x}' for base in bases)
    words = ' + '.join(str(size) for size in sizes)

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
        print(f'{args.object} at {where}: {words} words, entry {entry:#x}, '
              f'{len(core)} compared, {bad} mismatch{"es" if bad != 1 else ""}')
        return 1 if bad else 0

    print(f'{args.object} at {where}: {words} words, entry {entry:#x}'
          f'{", byte page" if any(byte_pages) else ""}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
