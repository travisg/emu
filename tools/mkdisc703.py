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

With --add NAME FILE, the file goes onto the platter under REX's filesystem
(tools/fs703.py states the format; rex/rexfs.asm is the kernel's side of it):
a directory in sectors 1-8 and contiguous files after it. NAME is `NAME` or
`NAME.TY`, six characters and two, and a name without a type is a module,
`.RX`. The file takes the first hole it fits and replaces a namesake; nothing
else on the platter moves, so files the guest wrote stay as they are. --add
works on an existing image without --force, and makes the image if there is
none; a platter with no directory is given one first, provided sectors 1-8
are blank or hold the four-word catalogue this tool wrote before there was a
filesystem -- whose files were only ever modules it put there itself, so
they are dropped rather than carried over. --add-high is --add for a module
that compares no byte addresses within itself, and so may be loaded above word
X'4000' (rex/rex.asm's header says why that matters); the directory entry
carries the fact. --swap N makes SWAP.SY, N sectors of system file that REX
puts tasks out to when more is loaded than core holds, unless the platter has
one already; without it REX does no swapping. --list prints the directory.

An existing image is otherwise left alone unless --force is given: a disc the
guest has been writing to is data, not a build product.
"""

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fs703  # noqa: E402

TRACKS = 64
SECTORS_PER_TRACK = 128
WORDS_PER_SECTOR = 47

WORDS_PER_UNIT = TRACKS * SECTORS_PER_TRACK * WORDS_PER_SECTOR
IMAGE_BYTES = WORDS_PER_UNIT * 2
SECTOR_BYTES = WORDS_PER_SECTOR * 2
SECTORS = TRACKS * SECTORS_PER_TRACK


def old_catalogue(image):
    """The entries of the four-word catalogue in sector 1 -- a name in two
    words, a first sector, a count, files one after another from sector 2,
    a zero first sector ending the table -- or None if sector 1 is not
    one."""
    cat = image[SECTOR_BYTES:2 * SECTOR_BYTES]
    words = [int.from_bytes(cat[i:i + 2], 'big') for i in range(0, SECTOR_BYTES, 2)]
    at, entries = 2, []
    for i in range(WORDS_PER_SECTOR // 4):
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


def filesystem(image, path):
    """The platter's filesystem, made first if the platter has none."""
    platter = fs703.Platter(image)
    if platter.has_directory():
        return platter, ''
    old = old_catalogue(image)
    if old is None and not platter.directory_area_blank():
        sys.exit(f"{path}: sectors 1-8 hold something that is not a directory; "
                 "pass --force for a blank platter")
    platter.format()
    if old:
        return platter, f", the old catalogue ({' '.join(old)}) dropped"
    return platter, ''


def main():
    parser = argparse.ArgumentParser(description="make a Raytheon 703 disc image")
    parser.add_argument("output", type=Path, help="image to write")
    parser.add_argument("--boot", type=Path, metavar="PROGRAM",
                        help="program to place in sector 0, track 0 for the LOAD button")
    parser.add_argument("--add", nargs=2, action="append", default=[],
                        metavar=("NAME", "FILE"),
                        help="put FILE on the platter as NAME, or NAME.TY; a bare name is a module, .RX")
    parser.add_argument("--add-high", nargs=2, action="append", default=[],
                        metavar=("NAME", "FILE"),
                        help="the same, for a module that may be loaded above word X'4000'")
    parser.add_argument("--swap", type=int, metavar="SECTORS",
                        help="make SWAP.SY, the file REX swaps tasks out to, if there is none")
    parser.add_argument("--list", action="store_true", help="print the directory")
    parser.add_argument("--force", action="store_true",
                        help="overwrite an existing image")
    args = parser.parse_args()

    exists = args.output.exists()
    adding = [(name, path, 0) for name, path in args.add]
    adding += [(name, path, fs703.HIGH) for name, path in args.add_high]
    if exists and (adding or args.list or args.swap) and not args.boot and not args.force:
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

    try:
        if args.swap and not adding:
            platter, note = filesystem(image, args.output)
        if adding:
            platter, note = filesystem(image, args.output)
            for name, path, flags in adding:
                platter.add(*fs703.parse_name(name), Path(path).read_bytes(), flags)
        if args.swap and platter.find('SWAP', 'SY') is None:
            platter.add('SWAP', 'SY', bytes(args.swap * SECTOR_BYTES), fs703.SYSTEM)
        if adding or args.swap:
            files = len(platter.files())
            what += f", {files} file{'s' if files != 1 else ''}, {platter.free_sectors()} sectors free{note}"

        if args.list:
            print('\n'.join(fs703.listing(fs703.Platter(image))))
            if not adding and not args.boot and not args.swap:
                return
    except fs703.FsError as e:
        sys.exit(f"{args.output}: {e}")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(image)
    print(f"{args.output}: {IMAGE_BYTES} bytes, {what}")


if __name__ == "__main__":
    main()
