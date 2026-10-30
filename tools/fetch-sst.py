#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Fetch the SingleStepTests Z80 suite and convert it for the Rust test.

github.com/SingleStepTests/z80 is 1604 JSON files, one per opcode (prefixes
included), of 1000 cases each: a register file and a few bytes of RAM before
one instruction and after it, the port traffic, and the instruction's bus
cycles. It is 1.4 GB of JSON in a ~290 MB tarball, which is why it is opt-in
and not in tools/rom-manifest.txt: the suite is a reference to check the core
against, not something any machine needs to run.

The tree is pinned by commit, which names its contents, rather than by a hash
of the tarball GitHub generates for it, which is not promised to be stable.

Each file becomes tests/data/sst-z80/<name>.txt, one case per line, five tab
separated fields after the name -- the format `src/cpu/z80_sst.rs` reads:

    name  initial-regs  initial-ram  final-regs  final-ram  ports  cycles

regs is REGS below as space separated hex, ram is `addr=val` pairs, ports is
`addr=val/r` or `/w`, cycles is the case's T-state count. The per-cycle bus
states are dropped (the core is not cycle-stepped) and so is `p`, which marks
an `LD A,I`/`LD A,R` for the interrupt that would read its stale PV -- no
single-step case takes an interrupt.

    tools/fetch-sst.py                    # download and convert
    tools/fetch-sst.py --tarball z80.tgz  # convert a tarball already here
"""

import argparse
import io
import json
import sys
import tarfile
import urllib.request
from pathlib import Path

COMMIT = "ebe1875d48f374bcfd4b505d8eb8ee751568b5f7"
URL = f"https://codeload.github.com/SingleStepTests/z80/tar.gz/{COMMIT}"
ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / "tests" / "data" / "sst-z80"

REGS = ["pc", "sp", "a", "f", "b", "c", "d", "e", "h", "l", "i", "r", "ei", "wz",
        "ix", "iy", "af_", "bc_", "de_", "hl_", "im", "q", "iff1", "iff2"]


def regs(state):
    return " ".join(f"{state[k]:x}" for k in REGS)


def ram(state):
    return " ".join(f"{a:x}={v:x}" for a, v in state["ram"])


def ports(case):
    return " ".join(f"{a:x}={v:x}/{d}" for a, v, d in case.get("ports") or [])


def convert(cases):
    for c in cases:
        i, f = c["initial"], c["final"]
        yield "\t".join([c["name"], regs(i), ram(i), regs(f), ram(f), ports(c), str(len(c["cycles"]))])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--tarball", help="a tarball of the pinned commit, instead of downloading it")
    args = ap.parse_args()

    if args.tarball:
        src = open(args.tarball, "rb")
    else:
        print(f"fetching {URL} (~290 MB)")
        src = io.BytesIO(urllib.request.urlopen(URL).read())

    DEST.mkdir(parents=True, exist_ok=True)
    prefix = f"z80-{COMMIT}/v1/"
    files = 0
    with tarfile.open(fileobj=src, mode="r:gz") as tar:
        for m in tar:
            if not (m.isfile() and m.name.startswith(prefix) and m.name.endswith(".json")):
                continue
            name = m.name[len(prefix):-len(".json")]
            if "/" in name:
                continue
            cases = json.load(tar.extractfile(m))
            (DEST / f"{name}.txt").write_text("\n".join(convert(cases)) + "\n")
            files += 1
    if files != 1604:
        sys.exit(f"converted {files} files, expected 1604: not the pinned tree?")
    print(f"{files} files into {DEST.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
