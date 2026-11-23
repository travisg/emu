#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Read a Raytheon 700-series absolute paper tape.

Raytheon's absolute tapes -- X-RAY, the loaders, the diagnostics, SYM I --
are a core image behind a twelve-frame header:

    8A 00 02 hh ll 8D     a record: line feed, a count of two, the word the
                          image begins at, carriage return
    00 00 00
    8A cc cc              line feed, and the number of frames that follow
    ...                   the image, two frames to a word, high half first
    00 ss                 a check word, counted with the image
    8D

and often a last record of two frames after it, which looks like where to
start: its low fourteen bits are 26A on X-RAY's tape, 545 on the relocating
loader's. SYM I punches X'8000' with its END card's address there (8080 on
PTIOS's tape, 8800 on its own), and the check word: the image's byte sum
folded once, (sum >> 8) + sum, in the low half. Fifteen of the museum's
absolute tapes carry it; X-RAY's site copy, BRAINWASH and the SYM I/PREP
rev F tape do not. The rest of that reading is this tool's; no Raytheon
document at hand describes the format.

The header is twelve frames because that is what the PTB bootstrap takes to
rewrite itself (DN 390364), so the same tape loads with nothing in core:
the operator sets the index register to twice the origin less twelve --
the JXR written on a tape's label -- and the image lands on its origin.

    tape703.py TAPE                 say what is on it
    tape703.py TAPE -o CORE         and write it as a flat core image from
                                    word 0, which is what `emu -s ray703 -r`
                                    loads
    tape703.py TAPE -o CORE --start 80
                                    with a jump to word 80 in word 0, where
                                    that machine starts

A tape with fewer frames than its count says is refused: a core image padded
out with zeros would run as far as the hole.
"""

import argparse
import re
import sys

# Leader may be blank or punched with a legend, so the header is looked for
# rather than counted to.
HEADER = re.compile(rb'\x8a\x00\x02(..)\x8d\x00{0,4}\x8a(..)', re.S)
TRAILER = re.compile(rb'\x8a\x00\x02(..)\x8d', re.S)


class TapeError(Exception):
    pass


def read_tape(frames):
    """(origin word, image bytes, the last record's word or None)."""
    m = HEADER.search(frames)
    if not m:
        raise TapeError('no absolute-tape header (8A 00 02 hh ll 8D ... 8A cc cc)')
    origin = int.from_bytes(m.group(1), 'big')
    count = int.from_bytes(m.group(2), 'big')
    image = frames[m.end():m.end() + count]
    if len(image) < count:
        raise TapeError(f'the header counts {count} frames and {len(image)} follow it')
    t = TRAILER.search(frames, m.end() + count)
    return origin, image, int.from_bytes(t.group(1), 'big') if t else None


def check_word(image):
    """Whether the image's last word is the check word of the rest: True,
    False, or None for an image too short to have one."""
    if len(image) < 4 or len(image) % 2:
        return None
    s = sum(image[:-2])
    return image[-2] == 0 and image[-1] == ((s >> 8) + s) & 0xff


def read_source(frames):
    """The cards of a SYM source tape: each a record that opens on a line
    feed and closes on a carriage return, bit 7 set throughout, with
    whatever is punched between one record and the next -- PTIOS's tape
    has the card number there -- left out."""
    cards, at = [], 0
    while True:
        lf = frames.find(b'\x8a', at)
        if lf < 0:
            return cards
        cr = frames.find(b'\x8d', lf)
        if cr < 0:
            return cards
        cards.append(bytes(c & 0x7f for c in frames[lf + 1:cr]).decode('ascii', 'replace').rstrip())
        at = cr + 1


def main():
    ap = argparse.ArgumentParser(description='read a Raytheon 700-series absolute paper tape')
    ap.add_argument('tape')
    ap.add_argument('-o', '--output', help='write a flat core image from word 0')
    ap.add_argument('--start', help='put a jump to this word (hex) in word 0')
    args = ap.parse_args()

    with open(args.tape, 'rb') as f:
        frames = f.read()
    try:
        origin, image, last = read_tape(frames)
    except TapeError as e:
        print(f'{args.tape}: {e}', file=sys.stderr)
        return 1

    words = (len(image) + 1) // 2
    print(f'{args.tape}: words {origin:04X}-{origin + words - 1:04X}, {len(image)} frames; '
          f'PTB index {(origin * 2 - 12) & 0xffff:04X}')
    check = check_word(image)
    if check is not None:
        print('last word %04X: %s' % (int.from_bytes(image[-2:], 'big'),
                                      'the check word' if check else 'not a check word'))
    if last is not None:
        print(f'last record {last:04X}: start at {last & 0x3fff:04X}?')

    if args.output:
        core = bytearray(origin * 2) + image
        if len(core) % 2:
            core.append(0)
        if args.start:
            start = int(args.start, 16)
            if origin == 0 or start > 0x7ff:
                print('--start needs word 0 free and a start in word page 0', file=sys.stderr)
                return 1
            core[0:2] = (0x1000 | start).to_bytes(2, 'big')     # JMP start
        with open(args.output, 'wb') as f:
            f.write(core)
    return 0


if __name__ == '__main__':
    sys.exit(main())
