#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Make a disc image for the Raytheon 703's 74601 controller, and put files
on it.

A platter is 64 tracks of 128 sectors of 47 words (src/dev/disc74601.rs), so
an image is exactly 385,024 words -- 770,048 bytes -- and the emulator refuses
anything else. Blank is all zeros, which is a formatted but empty disc.

With --boot, a program is placed in sector 0 of track 0, where the controller's
LOAD button reads it: the button's fixed sequence pulls that one sector into
words 0-46 and starts the machine there (706 UM 5-9.10.3, Table 5-30). One
sector is the entire budget, so a program over 94 bytes would boot truncated
and is refused here instead.

With --add NAME FILE, the file goes onto the platter under the catalogue REX's
loader reads (rex/rex.asm, LDMOD): sector 1 holds entries of four words -- the
name's first four characters packed as REX's shell packs a word, bit-7-set
ASCII with a short name right-justified into the two words and zero above;
the first sector; the sector count -- and a zero first sector ends the table,
so eleven entries fit. The files lie one after another from sector 2, one
94-byte record to a sector, padded with zeros. Adding rewrites the whole file
area: every file already catalogued is read back and laid down again with the
new one replacing a namesake or going on the end, so the operation is
idempotent and never touches a sector the catalogue does not own. --add works
on an existing image without --force, and makes the image if there is none;
--list prints the catalogue.

An existing image is otherwise left alone unless --force is given: a disc the
guest has been writing to is data, not a build product.
"""

import argparse
import sys
from pathlib import Path

TRACKS = 64
SECTORS_PER_TRACK = 128
WORDS_PER_SECTOR = 47

WORDS_PER_UNIT = TRACKS * SECTORS_PER_TRACK * WORDS_PER_SECTOR
IMAGE_BYTES = WORDS_PER_UNIT * 2
SECTOR_BYTES = WORDS_PER_SECTOR * 2
SECTORS = TRACKS * SECTORS_PER_TRACK

CATALOGUE_SECTOR = 1
FIRST_FILE_SECTOR = 2
ENTRY_WORDS = 4
ENTRIES = WORDS_PER_SECTOR // ENTRY_WORDS      # eleven


def pack_name(name):
    """The two words REX's SHTOK makes of a word: its first four characters,
    bit 7 set, accumulated from the right."""
    name = name.upper()[:4]
    if not name or not all(c.isalnum() for c in name):
        sys.exit(f"'{name}': a name is letters or digits")
    value = 0
    for c in name:
        value = (value << 8) | (ord(c) | 0x80)
    return (value >> 16) & 0xffff, value & 0xffff


def unpack_name(w0, w1):
    return ''.join(chr(b & 0x7f) for b in (w0 >> 8, w0 & 0xff, w1 >> 8, w1 & 0xff) if b)


def sector(image, n):
    return image[n * SECTOR_BYTES:(n + 1) * SECTOR_BYTES]


def read_catalogue(image):
    """(name, first sector, sector count) per entry, in catalogue order."""
    cat = sector(image, CATALOGUE_SECTOR)
    words = [int.from_bytes(cat[i:i + 2], 'big') for i in range(0, SECTOR_BYTES, 2)]
    entries = []
    for i in range(ENTRIES):
        w0, w1, start, length = words[i * ENTRY_WORDS:(i + 1) * ENTRY_WORDS]
        if start == 0:
            break
        entries.append((unpack_name(w0, w1), start, length))
    return entries


def write_files(image, files):
    """Lay the files down from sector 2 in order and write the catalogue."""
    if len(files) > ENTRIES:
        sys.exit(f"the catalogue holds {ENTRIES} entries; {len(files)} would not fit")
    cat = bytearray(SECTOR_BYTES)
    at = FIRST_FILE_SECTOR
    for i, (name, data) in enumerate(files):
        count = (len(data) + SECTOR_BYTES - 1) // SECTOR_BYTES
        if at + count > SECTORS:
            sys.exit(f"{name}: the platter is full")
        image[at * SECTOR_BYTES:(at + count) * SECTOR_BYTES] = data.ljust(count * SECTOR_BYTES, b'\0')
        w0, w1 = pack_name(name)
        for j, w in enumerate((w0, w1, at, count)):
            cat[(i * ENTRY_WORDS + j) * 2:(i * ENTRY_WORDS + j) * 2 + 2] = w.to_bytes(2, 'big')
        at += count
    image[CATALOGUE_SECTOR * SECTOR_BYTES:(CATALOGUE_SECTOR + 1) * SECTOR_BYTES] = cat
    return at


def main():
    parser = argparse.ArgumentParser(description="make a Raytheon 703 disc image")
    parser.add_argument("output", type=Path, help="image to write")
    parser.add_argument("--boot", type=Path, metavar="PROGRAM",
                        help="program to place in sector 0, track 0 for the LOAD button")
    parser.add_argument("--add", nargs=2, action="append", default=[],
                        metavar=("NAME", "FILE"),
                        help="put FILE on the platter under NAME, for REX's loader")
    parser.add_argument("--list", action="store_true", help="print the catalogue")
    parser.add_argument("--force", action="store_true",
                        help="overwrite an existing image")
    args = parser.parse_args()

    exists = args.output.exists()
    if exists and (args.add or args.list) and not args.boot and not args.force:
        image = bytearray(args.output.read_bytes())
        if len(image) != IMAGE_BYTES:
            sys.exit(f"{args.output} is {len(image)} bytes, not a {IMAGE_BYTES}-byte platter")
        what = "kept"
    elif exists and not args.force:
        sys.exit(f"{args.output} exists; pass --force to replace it")
    else:
        image = bytearray(IMAGE_BYTES)
        what = "blank"

    if args.boot:
        boot = args.boot.read_bytes()
        if len(boot) > SECTOR_BYTES:
            sys.exit(f"{args.boot} is {len(boot)} bytes; the LOAD button reads "
                     f"one {SECTOR_BYTES}-byte sector")
        image[:len(boot)] = boot
        what = f"{len(boot) // 2} words of {args.boot.name} in sector 0"

    if args.add:
        files = [(name, bytes(image[start * SECTOR_BYTES:(start + count) * SECTOR_BYTES]))
                 for name, start, count in read_catalogue(image)]
        for name, path in args.add:
            name = name.upper()[:4]     # the catalogue knows four characters
            pack_name(name)
            data = Path(path).read_bytes()
            files = [f for f in files if f[0] != name] + [(name, data)]
        end = write_files(image, files)
        what += f", {len(files)} file{'s' if len(files) != 1 else ''} in sectors 2-{end - 1}"

    if args.list:
        for name, start, count in read_catalogue(image):
            print(f"{name:<4} sector {start:5} ({count} sector{'s' if count != 1 else ''})")
        if not args.add and not args.boot:
            return

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(image)
    print(f"{args.output}: {IMAGE_BYTES} bytes, {what}")


if __name__ == "__main__":
    main()
