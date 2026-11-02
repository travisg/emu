#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Fetch the 8080 instruction exercisers for the core's ignored test.

Four CP/M programs, as superzazu/8080 collects them in its cpu_tests/:
Microcosm Associates' TST8080 diagnostic, SuperSoft's CPUTEST, and Ian
Bartholomew's 8080PRE and 8080EXM -- Frank Cringle's Z80 exerciser ported
to the 8080, with the CRCs of real silicon built in so that each group says
PASS or ERROR. They land in tests/data/i8080/, where `cargo test --release
-- --ignored i8080_exercisers` (src/cpu/i8080_exer.rs) runs them. They are
other people's programs under other people's terms, so they are fetched
rather than kept here, pinned by commit and checked by hash.

    tools/fetch-i8080-tests.py              # download
    tools/fetch-i8080-tests.py --from DIR   # copy from a directory already here
"""

import argparse
import hashlib
import sys
import urllib.request
from pathlib import Path

COMMIT = "274ffd700b81baabea99b0963bc1260b67132185"
URL = f"https://raw.githubusercontent.com/superzazu/8080/{COMMIT}/cpu_tests/"
FILES = {
    "8080PRE.COM": "18eb3c79cba42c0718f160be6a1853cb64cdce7aa47d65780189a57bdd98c4e0",
    "TST8080.COM": "9561c6fb6c99efe3de00eb77e4044fd102151058b39ac2d7bce10483838a08e7",
    "CPUTEST.COM": "e61a9a75348c774486c2207080ea4effbf6c2367fdace31b0731081a4144030b",
    "8080EXM.COM": "6e3286e11bb1a8f47b8ee1280b4a067be813193363e3223c99b0d21912f44aeb",
}
DEST = Path(__file__).resolve().parent.parent / "tests" / "data" / "i8080"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--from", dest="src", help="a directory holding the files, instead of downloading them")
    args = ap.parse_args()

    DEST.mkdir(parents=True, exist_ok=True)
    for name, want in FILES.items():
        if args.src:
            data = (Path(args.src) / name).read_bytes()
        else:
            data = urllib.request.urlopen(URL + name).read()
        digest = hashlib.sha256(data).hexdigest()
        if digest != want:
            print(f"error: {name} hashes to {digest}, not {want}", file=sys.stderr)
            return 1
        (DEST / name).write_bytes(data)
        print(DEST / name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
