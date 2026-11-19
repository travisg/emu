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
            file, which is never moved or deleted, here or on the
            machine, nor replaced by anything but another system file

Files are contiguous. The entries are in disc order and account for every
data sector between them: a hole is an entry like any other, each entry
starts where the one before it ends, no two holes touch, and the unused
entries are all at the end. A file being written holds what it was given --
half of the largest hole there was -- until it is closed, when it is cut to
its length and the rest is a hole again; one that nobody closed is a hole
to whoever looks next.

As a program it lists, checks, extracts and deletes, and it puts files
on a platter -- making the platter, all zeros, if there is none, and the
directory if the platter has none. A blank platter or a boot sector is
the hardware's business, and tools/mkdisc703.py's.

    fs703.py IMAGE                  the directory
    fs703.py IMAGE --check          the rules above, held to the image
    fs703.py IMAGE --get NAME FILE  a file's bytes, out to the host
    fs703.py IMAGE --delete NAME    unless it is a system file
    fs703.py IMAGE --dir            a directory, empty, if it has none:
                                    disks/ray703-disc1.img made this way
                                    is REX's data unit
    fs703.py IMAGE --add NAME FILE  FILE as NAME, or NAME.TY, a bare name
                                    a module, .RX; it takes the first hole
                                    it fits and replaces a namesake, and
                                    nothing else moves, so files the guest
                                    wrote stay as they are
    fs703.py IMAGE --add-high ...   the same for a module that compares no
                                    byte addresses within itself, and so
                                    may load above word X'4000' (rex.asm's
                                    header says why that matters)
    fs703.py IMAGE --swap N         SWAP.SY, N sectors of system file REX
                                    puts tasks out to, unless there is one
    fs703.py IMAGE --sys KERNEL     REX's core image as REX.SY -- its two
                                    parts, either side of the pool -- and
                                    where they are written into the six
                                    words at the end of REX's boot sector
                                    (boot.asm), which must be in sector 0
                                    already; emu -s ray703-load -r IMAGE
                                    then boots it

A platter with no directory is given one by any of the writing options,
provided sectors 1-8 are blank or hold the four-word catalogue the tools
wrote before there was a filesystem -- whose files were only ever modules
they put there, so they are dropped rather than carried over.
"""

import argparse
import sys
from pathlib import Path

SECTOR_WORDS = 47
SECTOR_BYTES = SECTOR_WORDS * 2
TRACKS = 64
SECTORS_PER_TRACK = 128
SECTORS = TRACKS * SECTORS_PER_TRACK
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
    # ASCII's letters and digits: a character goes into the directory as
    # seven bits, and isalnum alone would let through ones that are not
    if not 1 <= len(name) <= 6 or not (name.isascii() and name.isalnum()):
        raise FsError("'%s': a name is one to six letters or digits" % text)
    if not 1 <= len(kind) <= 2 or not (kind.isascii() and kind.isalnum()):
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

    def delete(self, name, kind, system=False):
        """Make a hole of a file; False if there is none of that name. A
        system file goes only for a caller that says it means one: the
        boot sector names REX.SY's sectors, and REX swaps to SWAP.SY's."""
        entries = self.entries()
        for e in entries:
            if e.what == FILE and (e.name, e.type) == (name, kind):
                if e.flags & SYSTEM and not system:
                    raise FsError('%s: a system file' % e.title)
                e.name, e.type, e.last, e.flags = '', '', 0, HOLE
                self.write_entries(self._merged(entries))
                return True
        return False

    def add(self, name, kind, data, flags=0):
        """Put a file on the platter, in place of one of the same name: the
        first hole it fits, the rest of the hole staying a hole. Only a
        system file takes a system file's place."""
        self.delete(name, kind, system=bool(flags & SYSTEM))
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


def old_catalogue(image):
    """The entries of the four-word catalogue in sector 1 -- a name in two
    words, a first sector, a count, files one after another from sector 2,
    a zero first sector ending the table -- or None if sector 1 is not
    one."""
    cat = image[SECTOR_BYTES:2 * SECTOR_BYTES]
    words = [int.from_bytes(cat[i:i + 2], 'big') for i in range(0, SECTOR_BYTES, 2)]
    at, entries = 2, []
    for i in range(SECTOR_WORDS // 4):
        w0, w1, start, count = words[i * 4:(i + 1) * 4]
        if start == 0:
            break
        name = ''.join(chr(b & 0x7f) for b in (w0 >> 8, w0 & 0xff, w1 >> 8, w1 & 0xff) if b)
        if start != at or count == 0 or not name.isalnum():
            return None
        entries.append(name)
        at += count
    if any(words[len(entries) * 4:]):
        return None
    return entries or None


BOOT_MARK = 0xd2d8              # 'RX', word 40 of REX's boot sector
BOOT_TABLE = 41                 # the six words after it
MAX_TRANSFER = 0x3fff           # words the controller moves at once


def kernel_parts(kernel):
    """A kernel image as the two runs of words either side of its longest
    run of zeros: (address, bytes) for each. REX is the shell's end and
    the filesystem's beginning with the whole pool between."""
    words = [int.from_bytes(kernel[i:i + 2], 'big') for i in range(0, len(kernel), 2)]
    best, start = (0, 0), None
    for i, w in enumerate(words + [1]):
        if w == 0 and start is None:
            start = i
        elif w != 0 and start is not None:
            if i - start > best[1] - best[0]:
                best = (start, i)
            start = None
    low, high = best
    if low == 0 or high >= len(words):
        sys.exit("the kernel image is not two parts with a gap between")
    return (0, kernel[:low * 2]), (high, kernel[high * 2:])


def track_sector(n):
    """A sector's index as the controller's DOT 1,2 wants it: the track in
    the top six bits, the sector in the low seven."""
    return (n // SECTORS_PER_TRACK) << 10 | n % SECTORS_PER_TRACK


def add_kernel(image, platter, path):
    """REX.SY, and where its two parts are, written into the boot sector."""
    if int.from_bytes(image[80:82], 'big') != BOOT_MARK:
        sys.exit("sector 0 is not REX's boot sector; put rex/build/boot.bin there first, "
                 "with tools/mkdisc703.py --boot")
    parts = kernel_parts(Path(path).read_bytes())
    for address, data in parts:
        if len(data) // 2 > MAX_TRANSFER:
            sys.exit(f"{path}: {len(data) // 2} words at {address:04X} is more than one transfer")
    low = parts[0][1]
    low += bytes(-len(low) % SECTOR_BYTES)
    entry = platter.add('REX', 'SY', low + parts[1][1], SYSTEM)
    table, sector = [], entry.start
    for address, data in parts:
        table += [track_sector(sector), len(data) // 2, address]
        sector += (len(data) + SECTOR_BYTES - 1) // SECTOR_BYTES
    image[BOOT_TABLE * 2:(BOOT_TABLE + 6) * 2] = b''.join(w.to_bytes(2, 'big') for w in table)


def filesystem(image, path):
    """The platter's filesystem, made first if the platter has none."""
    platter = Platter(image)
    if platter.has_directory():
        return platter, ''
    old = old_catalogue(image)
    if old is None and not platter.directory_area_blank():
        sys.exit(f"{path}: sectors 1-8 hold something that is not a directory; "
                 "tools/mkdisc703.py --force makes a blank platter")
    platter.format()
    if old:
        return platter, f", the old catalogue ({' '.join(old)}) dropped"
    return platter, ''


def main():
    parser = argparse.ArgumentParser(description="REX's filesystem on a 703 platter: look at it, put files on it")
    parser.add_argument('image', type=Path)
    parser.add_argument('--dir', action='store_true',
                        help='give the platter a directory, empty, if it has none')
    parser.add_argument('--add', nargs=2, action='append', default=[], metavar=('NAME', 'FILE'),
                        help='put FILE on the platter as NAME, or NAME.TY; a bare name is a module, .RX')
    parser.add_argument('--add-high', nargs=2, action='append', default=[], metavar=('NAME', 'FILE'),
                        help="the same, for a module that may be loaded above word X'4000'")
    parser.add_argument('--sys', type=Path, metavar='KERNEL',
                        help="put REX's core image on the platter as REX.SY, for its boot sector to load")
    parser.add_argument('--swap', type=int, metavar='SECTORS',
                        help='make SWAP.SY, the file REX swaps tasks out to, if there is none')
    parser.add_argument('--check', action='store_true', help='hold the directory to its rules')
    parser.add_argument('--get', nargs=2, metavar=('NAME', 'FILE'), help='copy a file out')
    parser.add_argument('--delete', metavar='NAME')
    args = parser.parse_args()
    adding = [(name, path, 0) for name, path in args.add]
    adding += [(name, path, HIGH) for name, path in args.add_high]
    writing = adding or args.swap or args.sys or args.dir
    try:
        if args.image.exists():
            image = bytearray(args.image.read_bytes())
            if len(image) != IMAGE_BYTES:
                raise FsError('%d bytes, not a %d-byte platter' % (len(image), IMAGE_BYTES))
        elif writing:
            image = bytearray(IMAGE_BYTES)
        else:
            raise FsError('no such platter')
        if writing:
            platter, note = filesystem(image, args.image)
            if args.sys:
                add_kernel(image, platter, args.sys)
            for name, path, flags in adding:
                platter.add(*parse_name(name), Path(path).read_bytes(), flags)
            if args.swap and platter.find('SWAP', 'SY') is None:
                platter.add('SWAP', 'SY', bytes(args.swap * SECTOR_BYTES), SYSTEM)
            args.image.parent.mkdir(parents=True, exist_ok=True)
            args.image.write_bytes(image)
            files = len(platter.files())
            print('%s: %d file%s, %d sectors free%s' % (args.image, files, '' if files == 1 else 's',
                                                        platter.free_sectors(), note))
            return 0
        platter = Platter(image)
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
