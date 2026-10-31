#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Fetch ASxxxx's own assembler tests for the disassemblers' ignored tests.

Alan Baldwin's ASxxxx distribution ships a test file per assembler, every
line an instruction with the bytes it assembles to in its comment. This
tree already carries the 6809's (`test/6809/t6809.asm`, which `cargo test`
reads); the 6800's and the Z80's come from the v6.10 release into
tests/data/asxxxx/, where `cargo test -- --ignored t6800 tz80` finds them.
The release is a fixed file, so it is pinned by its hash. Only the two
files are kept; nothing in the archive is built or run.

    tools/fetch-asxxxx.py                    # download (~80 MB) and extract
    tools/fetch-asxxxx.py --zip asxv6p10.zip # extract from a copy already here
"""

import argparse
import hashlib
import io
import sys
import urllib.request
import zipfile
from pathlib import Path

URL = "https://shop-pdp.net/_ftp/asxxxx/asxv6p10.zip"
SHA256 = "486eb14b963fb3b8217b9b57c6550a54d895c9177648b9e1dff060fd8c634777"
FILES = {"asxv6pxx/as6800/t6800.asm": "t6800.asm", "asxv6pxx/asz80/tz80.asm": "tz80.asm"}
DEST = Path(__file__).resolve().parent.parent / "tests" / "data" / "asxxxx"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--zip", help="a copy of the release, instead of downloading it")
    args = ap.parse_args()

    if args.zip:
        data = Path(args.zip).read_bytes()
    else:
        print(f"fetching {URL} (~80 MB)")
        data = urllib.request.urlopen(URL).read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != SHA256:
        print(f"error: the release hashes to {digest}, not {SHA256}", file=sys.stderr)
        return 1

    DEST.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for member, name in FILES.items():
            (DEST / name).write_bytes(z.read(member))
            print(f"{DEST / name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
