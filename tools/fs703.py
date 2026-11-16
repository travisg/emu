#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""REX's filesystem on a Raytheon 703 platter, from the host's side.

The kernel's side is rex/rexfs.asm, and the two are held to each other by
rex's scripted session: what one writes the other has to read.

A platter is 8,192 sectors of 47 words. Sector 0 is the LOAD button's boot
sector, sectors 1-8 are the directory, and the rest is data. The directory
is read and written whole, as one 376-word transfer:

    words 0-1   'RE','XF'
    word  2     the format's version, 1
    word  3     how many sectors the directory has, 8
    word  4     the first data sector, 9
    then 46 entries of eight words:
      0-2   the name: six characters, bit 7 set, left-justified, zero-filled
      3     the type: two characters, the same way
      4     the first sector
      5     how many sectors
      6     how many bytes of the last sector are the file's
      7     what the entry is, in its low byte -- 0 unused, 1 a hole, 2 a
            file, 3 a file being written -- and above that X'0100', a
            module that may load above word X'4000', and X'0200', a system
            file, which is never moved or deleted

Files are contiguous. The entries are in disc order and account for every
data sector between them: a hole is an entry like any other, each entry
starts where the one before it ends, no two holes touch, and the unused
entries are all at the end. A file being written holds what it was given --
half of the largest hole there was -- until it is closed, when it is cut to
its length and the rest is a hole again; one that nobody closed is a hole
to whoever looks next.

As a program this lists, checks, extracts and deletes; tools/mkdisc703.py
is what makes a platter and puts files on it.

    fs703.py IMAGE                  the directory
    fs703.py IMAGE --check          the rules above, held to the image
    fs703.py IMAGE --get NAME FILE  a file's bytes, out to the host
    fs703.py IMAGE --delete NAME
"""

import argparse
import sys
from pathlib import Path

SECTOR_WORDS = 47
SECTOR_BYTES = SECTOR_WORDS * 2
SECTORS = 64 * 128
IMAGE_BYTES = SECTORS * SECTOR_BYTES

DIR_SECTOR = 1
DIR_SECTORS = 8
DATA_SECTOR = DIR_SECTOR + DIR_SECTORS
DIR_WORDS = DIR_SECTORS * SECTOR_WORDS
HEADER_WORDS = 5
ENTRY_WORDS = 8
ENTRIES = (DIR_WORDS - HEADER_WORDS) // ENTRY_WORDS

MAGIC = (0xd2c5, 0xd8c6)        # 'RE', 'XF', bit 7 set
VERSION = 1

UNUSED, HOLE, FILE, TENTATIVE = 0, 1, 2, 3
KIND = 0x00ff
HIGH = 0x0100
SYSTEM = 0x0200

DEFAULT_TYPE = 'RX'


class FsError(Exception):
    pass


def pack(text, width):
    """Characters as the directory holds them, two to a word."""
    text = text.ljust(width, '\0')
    return [((ord(a) | 0x80 if a != '\0' else 0) << 8) | (ord(b) | 0x80 if b != '\0' else 0)
            for a, b in zip(text[0::2], text[1::2])]


def unpack(words):
    return ''.join(chr(b & 0x7f) for w in words for b in (w >> 8, w & 0xff) if b)


def parse_name(text, default_type=DEFAULT_TYPE):
    """`NAME` or `NAME.TY` as (name, type), upper case."""
    name, dot, kind = text.upper().partition('.')
    kind = kind if dot else default_type
    if not 1 <= len(name) <= 6 or not name.isalnum():
        raise FsError("'%s': a name is one to six letters or digits" % text)
    if not 1 <= len(kind) <= 2 or not kind.isalnum():
        raise FsError("'%s': a type is one or two letters or digits" % text)
    return name, kind


class Entry:
    def __init__(self, name='', kind='', start=0, count=0, last=0, flags=UNUSED):
        self.name, self.type = name, kind
        self.start, self.count, self.last, self.flags = start, count, last, flags

    @property
    def what(self):
        return self.flags & KIND

    @property
    def title(self):
        return '%s.%s' % (self.name, self.type)

    @property
    def size(self):
        """A file's length in bytes."""
        return (self.count - 1) * SECTOR_BYTES + self.last if self.count else 0

    def words(self):
        return pack(self.name, 6) + pack(self.type, 2) + [self.start, self.count, self.last, self.flags]

    def __repr__(self):
        return 'Entry(%s, %d+%d, last %d, %04x)' % (self.title, self.start, self.count, self.last, self.flags)


class Platter:
    """A platter image, a bytearray, as a filesystem."""

    def __init__(self, image):
        if len(image) != IMAGE_BYTES:
            raise FsError('%d bytes is not a %d-byte platter' % (len(image), IMAGE_BYTES))
        self.image = image

    # -- words ---------------------------------------------------------------

    def _words(self, sector, n):
        at = sector * SECTOR_BYTES
        return [int.from_bytes(self.image[at + 2 * i:at + 2 * i + 2], 'big') for i in range(n)]

    def _put_words(self, sector, words):
        at = sector * SECTOR_BYTES
        self.image[at:at + 2 * len(words)] = b''.join(w.to_bytes(2, 'big') for w in words)

    # -- the directory -------------------------------------------------------

    def has_directory(self):
        return tuple(self._words(DIR_SECTOR, 2)) == MAGIC

    def directory_area_blank(self):
        at = DIR_SECTOR * SECTOR_BYTES
        return not any(self.image[at:at + DIR_SECTORS * SECTOR_BYTES])

    def format(self):
        """An empty directory: the whole data area one hole."""
        self.write_entries([Entry(start=DATA_SECTOR, count=SECTORS - DATA_SECTOR, flags=HOLE)])

    def entries(self):
        """Every entry in use, in disc order."""
        if not self.has_directory():
            raise FsError('no directory')
        words = self._words(DIR_SECTOR, DIR_WORDS)
        out = []
        for i in range(ENTRIES):
            w = words[HEADER_WORDS + i * ENTRY_WORDS:HEADER_WORDS + (i + 1) * ENTRY_WORDS]
            if w[7] & KIND == UNUSED:
                break
            out.append(Entry(unpack(w[0:3]), unpack(w[3:4]), w[4], w[5], w[6], w[7]))
        return out

    def write_entries(self, entries):
        if len(entries) > ENTRIES:
            raise FsError('the directory holds %d entries; %d would not fit' % (ENTRIES, len(entries)))
        words = list(MAGIC) + [VERSION, DIR_SECTORS, DATA_SECTOR]
        for e in entries:
            words += e.words()
        self._put_words(DIR_SECTOR, words + [0] * (DIR_WORDS - len(words)))

    def files(self):
        return [e for e in self.entries() if e.what == FILE]

    def find(self, name, kind):
        for e in self.files():
            if (e.name, e.type) == (name, kind):
                return e
        return None

    def free_sectors(self):
        return sum(e.count for e in self.entries() if e.what != FILE)

    def check(self):
        """The directory's rules, as a list of what breaks them."""
        if not self.has_directory():
            return ['no directory']
        words = self._words(DIR_SECTOR, DIR_WORDS)
        bad = []
        if words[2:5] != [VERSION, DIR_SECTORS, DATA_SECTOR]:
            bad.append('header says version %d, %d sectors, data at %d' % tuple(words[2:5]))
        kinds = [words[HEADER_WORDS + i * ENTRY_WORDS + 7] & KIND for i in range(ENTRIES)]
        used = kinds.index(UNUSED) if UNUSED in kinds else ENTRIES
        if any(k != UNUSED for k in kinds[used:]):
            bad.append('an entry in use after an unused one')
        at, before, names = DATA_SECTOR, None, set()
        for e in self.entries():
            if e.what not in (HOLE, FILE, TENTATIVE):
                bad.append('%r: no such kind' % e)
            if e.start != at:
                bad.append('%r: starts at %d, where %d was expected' % (e, e.start, at))
            if e.what == HOLE and (e.count == 0 or before == HOLE):
                bad.append('%r: an empty hole, or one touching another' % e)
            if e.what == FILE:
                if e.title in names:
                    bad.append('%r: a second file of that name' % e)
                names.add(e.title)
                if (e.count == 0) != (e.last == 0) or e.last > SECTOR_BYTES:
                    bad.append('%r: %d bytes in its last sector' % (e, e.last))
            at, before = e.start + e.count, e.what
        if at != SECTORS:
            bad.append('the entries end at sector %d, not %d' % (at, SECTORS))
        return bad

    # -- files ---------------------------------------------------------------

    def read(self, name, kind):
        e = self.find(name, kind)
        if e is None:
            raise FsError('%s.%s: no such file' % (name, kind))
        return bytes(self.image[e.start * SECTOR_BYTES:e.start * SECTOR_BYTES + e.size])

    def delete(self, name, kind):
        entries = self.entries()
        for e in entries:
            if e.what == FILE and (e.name, e.type) == (name, kind):
                e.name, e.type, e.last, e.flags = '', '', 0, HOLE
                self.write_entries(self._merged(entries))
                return True
        return False

    def add(self, name, kind, data, flags=0):
        """Put a file on the platter, in place of one of the same name: the
        first hole it fits, the rest of the hole staying a hole."""
        self.delete(name, kind)
        count = (len(data) + SECTOR_BYTES - 1) // SECTOR_BYTES
        last = len(data) - (count - 1) * SECTOR_BYTES if count else 0
        entries = [e if e.what != TENTATIVE else Entry(start=e.start, count=e.count, flags=HOLE)
                   for e in self.entries()]
        entries = self._merged(entries)
        for i, hole in enumerate(entries):
            if hole.what == HOLE and hole.count >= count:
                break
        else:
            raise FsError('%s.%s: no hole of %d sectors' % (name, kind, count))
        new = Entry(name, kind, hole.start, count, last, FILE | flags)
        rest = Entry(start=hole.start + count, count=hole.count - count, flags=HOLE)
        entries[i:i + 1] = [new] + ([rest] if rest.count else [])
        at = new.start * SECTOR_BYTES
        self.image[at:at + count * SECTOR_BYTES] = data.ljust(count * SECTOR_BYTES, b'\0')
        self.write_entries(entries)
        return new

    @staticmethod
    def _merged(entries):
        out = []
        for e in entries:
            if e.what == HOLE and e.count == 0:
                continue
            if e.what == HOLE and out and out[-1].what == HOLE:
                out[-1].count += e.count
            else:
                out.append(e)
        return out


def listing(platter):
    lines = []
    for e in platter.entries():
        if e.what == FILE:
            marks = ''.join(c for c, bit in (('H', HIGH), ('S', SYSTEM)) if e.flags & bit)
            lines.append('%-9s sector %5d  %4d sector%s %6d bytes %s'
                         % (e.title, e.start, e.count, ' ' if e.count == 1 else 's', e.size, marks))
        else:
            what = 'hole' if e.what == HOLE else 'unclosed'
            lines.append('%-9s sector %5d  %4d sector%s' % ('<%s>' % what, e.start, e.count,
                                                           ' ' if e.count == 1 else 's'))
    return lines


def main():
    parser = argparse.ArgumentParser(description="look at REX's filesystem on a 703 platter")
    parser.add_argument('image', type=Path)
    parser.add_argument('--check', action='store_true', help='hold the directory to its rules')
    parser.add_argument('--get', nargs=2, metavar=('NAME', 'FILE'), help='copy a file out')
    parser.add_argument('--delete', metavar='NAME')
    args = parser.parse_args()
    try:
        platter = Platter(bytearray(args.image.read_bytes()))
        if args.check:
            bad = platter.check()
            for line in bad:
                print(line)
            if bad:
                return 1
            print('%s: %d files, %d sectors free' % (args.image, len(platter.files()), platter.free_sectors()))
        elif args.get:
            Path(args.get[1]).write_bytes(platter.read(*parse_name(args.get[0])))
        elif args.delete:
            if not platter.delete(*parse_name(args.delete)):
                raise FsError('%s: no such file' % args.delete)
            args.image.write_bytes(platter.image)
        else:
            print('\n'.join(listing(platter)))
    except FsError as e:
        print('%s: %s' % (args.image, e), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
