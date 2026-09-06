#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Hold a kernel to the interface file its modules are assembled against.

    apicheck703.py rexapi.asm rex.lst

rexapi.asm states REX's jump vector, exported cells, node layout and state
values as EQUs, and every module deck begins with it. rex.asm keeps its own
labels for the same things, so nothing stops the two drifting apart except
this: every EQU in the interface file must appear in the kernel's listing at
the same value. The listing is the one asm703.py -l writes, whose symbol
table is its tail.
"""

import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import asm703  # noqa: E402

SYMBOL = re.compile(r'^  (\S+)\s+([0-9A-F]{4})$')


def api_values(path):
    """The interface file's EQUs, in order, each evaluated over the ones
    before it."""
    values, counts = {}, {}
    for loc, label, op, arg, _ in asm703.parse(path):
        if op != 'EQU':
            raise asm703.AsmError(loc, f'{path} is EQUs only; {op} does not belong in it')
        values[label], counts[label] = asm703.Expr(arg or '', values, None, loc, counts).reloc()
    return values


def listing_symbols(path):
    symbols, table = {}, False
    with open(path, encoding='utf-8') as f:
        for line in f:
            line = line.rstrip('\n')
            if line == 'symbols:':
                table = True
                continue
            m = SYMBOL.match(line) if table else None
            if m:
                symbols[m.group(1)] = int(m.group(2), 16)
    if not table:
        raise SystemExit(f'{path}: no symbol table; is it an asm703.py -l listing?')
    return symbols


def main():
    if len(sys.argv) != 3:
        print('usage: apicheck703.py rexapi.asm rex.lst', file=sys.stderr)
        return 2
    api_path, lst_path = sys.argv[1:]
    try:
        api = api_values(api_path)
    except asm703.AsmError as e:
        print(e, file=sys.stderr)
        return 1
    kernel = listing_symbols(lst_path)
    bad = 0
    for name, value in api.items():
        if name not in kernel:
            print(f'{name}: {api_path} says {value & 0xffff:04X}, {lst_path} does not define it')
            bad += 1
        elif kernel[name] != value & 0xffff:
            print(f'{name}: {api_path} says {value & 0xffff:04X}, {lst_path} has {kernel[name]:04X}')
            bad += 1
    if bad:
        print(f'{api_path}: {bad} of {len(api)} names disagree with {lst_path}', file=sys.stderr)
        return 1
    print(f'{api_path}: {len(api)} names agree with {lst_path}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
