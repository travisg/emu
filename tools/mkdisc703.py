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
"""Make a disc image for the Raytheon 703's 74601 controller.

A platter is 64 tracks of 128 sectors of 47 words (src/dev/disc74601.rs), so
an image is exactly 385,024 words -- 770,048 bytes -- and the emulator refuses
anything else. Blank is all zeros, which is a formatted but empty disc.

With --boot, a program is placed in sector 0 of track 0, where the controller's
LOAD button reads it: the button's fixed sequence pulls that one sector into
words 0-46 and starts the machine there (706 UM 5-9.10.3, Table 5-30). One
sector is the entire budget, so a program over 94 bytes would boot truncated
and is refused here instead.

An existing image is left alone unless --force is given: a disc the guest has
been writing to is data, not a build product.
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


def main():
    parser = argparse.ArgumentParser(description="make a Raytheon 703 disc image")
    parser.add_argument("output", type=Path, help="image to write")
    parser.add_argument("--boot", type=Path, metavar="PROGRAM",
                        help="program to place in sector 0, track 0 for the LOAD button")
    parser.add_argument("--force", action="store_true",
                        help="overwrite an existing image")
    args = parser.parse_args()

    if args.output.exists() and not args.force:
        sys.exit(f"{args.output} exists; pass --force to replace it")

    image = bytearray(IMAGE_BYTES)
    what = "blank"

    if args.boot:
        boot = args.boot.read_bytes()
        if len(boot) > SECTOR_BYTES:
            sys.exit(f"{args.boot} is {len(boot)} bytes; the LOAD button reads "
                     f"one {SECTOR_BYTES}-byte sector")
        image[:len(boot)] = boot
        what = f"{len(boot) // 2} words of {args.boot.name} in sector 0"

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(image)
    print(f"{args.output}: {IMAGE_BYTES} bytes, {what}")


if __name__ == "__main__":
    main()
