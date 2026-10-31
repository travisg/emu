#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""The 703 disassembler over the debug port's `dis`, against two references
it shares no code with.

The 1968 listings: the transcripts in 703/listings print Raytheon's own
source card beside the word its assembler made of it, so every instruction
card names the mnemonic that word disassembles to. Each listing becomes a
core image straight from its object column (`xraylist.py`), boots, and has
every instruction card's address disassembled; the mnemonic has to be the
card's, after the assembler's own spellings (NOP is `SRA 0`, SMB an SML or
SMU, EXCH the first of its two `SLCD 8`s). Operands are not compared here:
the cards write them as symbols.

The assembler: every one of the 65,536 words, as the first word of an
instruction, is disassembled and its text put back through
`tools/asm703.py`, which has to give back the same word -- less any field
the core ignores and the disassembly therefore does not show -- or, for a
`DATA`, the word itself.

Needs python3 only, so CI runs it. Every check counts what it compared and
fails on too few, so a listing that stopped parsing cannot pass by
comparing nothing. Negative controls, which must FAIL: `DIS_CONTROL=rename`
swaps two mnemonics in what the port returns before comparing, and
`=rename-words` does it in the every-word check alone, skipping the
listings.
"""

import os
import sys
import tempfile
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

import asm703  # noqa: E402
import xraylist  # noqa: E402
from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_dis_test.log')
CONTROL = os.environ.get('DIS_CONTROL', '')
LISTINGS = os.path.join(HERE, '703', 'listings')

# (transcript, the fewest instruction cards it must yield)
REFERENCES = [
    ('390682C_RelocatingLoader_listing.txt', 450),
    ('390779_XRAY_listing.txt', 650),
]

# Cards that generate data, or nothing.
DIRECTIVES = {'D', 'DATA', 'WORD', 'BYTE', 'TEXT', 'RES', 'EQU', 'ORG', 'ORIG',
              'TRUE', 'FALS', 'ENDC', 'END'}
# Macros whose first word is not an instruction of the same name.
MACROS = {'SUBR', 'EXIT'}
# The assembler's names for words the machine knows by another.
ALIASES = {'NOP': {'SRA'}, 'SXP': {'IXS'}, 'SXM': {'DXS'}, 'SMB': {'SML', 'SMU'}, 'EXCH': {'SLCD'}}

DIS_CHUNK = 64


def mnemonic(text, control=('rename',)):
    name = text.split()[0]
    if CONTROL in control:
        name = {'LDW': 'STW', 'STW': 'LDW'}.get(name, name)
    return name


def spawn(image, log):
    return Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', image, '--no-throttle'], log=log)


def check_listing(name, minimum, workdir, log):
    cards, _ = xraylist.read_transcript([os.path.join(LISTINGS, name)])
    image = os.path.join(workdir, name + '.bin')
    with open(image, 'wb') as f:
        xraylist.emit_core(cards, f)
    emu = spawn(image, log)
    compared, overwritten, unread, bad = 0, 0, 0, []
    try:
        for c in cards:
            if c.addr is None or c.obj is None:
                continue
            _, op, _, _ = xraylist.split_card(c.text, c.has_label)
            if op is None or op in DIRECTIVES or op in MACROS:
                continue
            if op not in xraylist.OPCODES:
                # a label printed into its mnemonic (X-RAY's M.TDATAILDX)
                unread += 1
                continue
            want = ALIASES.get(op, {op})
            [(addr, raw, text)] = emu.dis(c.addr)
            if raw[0] != c.obj:
                # a later card at the same address, which the image keeps
                overwritten += 1
                continue
            compared += 1
            if mnemonic(text) not in want:
                bad.append('%s %04X %04X: card %d says %s, disassembled %s' % (name, c.addr, c.obj, c.card, op, text))
    finally:
        emu.kill()
        emu.wait_exit()
    log.write(('%s: %d instruction cards compared, %d disagree, %d overwritten by a later card, '
               '%d with no mnemonic to read\n' % (name, compared, len(bad), overwritten, unread)).encode())
    if bad:
        raise AssertionError('\n'.join(bad[:20]))
    if compared < minimum:
        raise AssertionError('%s: only %d instruction cards compared, expected %d' % (name, compared, minimum))
    return compared


def ignored_mask(word):
    """The bits of a generic the core ignores, which the text leaves out."""
    if word >> 12:
        return 0
    cls, f1 = (word >> 8) & 0xf, (word >> 4) & 0xf
    if (cls == 0 and f1 in (0x0, 0x4, 0x5, 0x6, 0x7, 0xa, 0xb)) or (cls == 1 and f1 <= 4) or cls == 8:
        return 0x000f
    return 0


def reassemble(text, workdir):
    """The words asm703 makes of `text` assembled at word 0."""
    op, _, arg = text.partition(' ')
    arg = arg or None
    if op == 'DATA':
        return [asm703.Expr(arg, {}, 0, 1).value() & 0xffff]
    if op in asm703.GEN_TWOWORD:
        path = os.path.join(workdir, 'two.asm')
        with open(path, 'w') as f:
            f.write('        ORG     0\n        %s\n        END\n' % text)
        built = asm703.assemble(path)[3]
        return [built[0], built[1]]
    return [asm703.encode2(1, op, arg, {}, 0)[0]]


def check_every_word(workdir, log):
    compared, bad = 0, []
    for half in (0, 0x8000):
        image = os.path.join(workdir, 'words-%04x.bin' % half)
        with open(image, 'wb') as f:
            f.write(b''.join((half + i).to_bytes(2, 'big') for i in range(0x8000)))
        emu = spawn(image, log)
        try:
            seen = {}
            addr = 0
            while addr < 0x8000:
                for a, raw, text in emu.dis(addr, DIS_CHUNK):
                    seen[a] = (raw, text)
                    addr = a + len(raw)
            for a in range(0x8000):
                if a not in seen:
                    seen[a] = emu.dis(a)[0][1:]
        finally:
            emu.kill()
            emu.wait_exit()
        for a in range(0x8000):
            raw, text = seen[a]
            word = half + a
            if raw[0] != word:
                raise AssertionError('%04X: memory holds %04X' % (a, raw[0]))
            text = mnemonic(text, ('rename', 'rename-words')) + text[len(text.split()[0]):]
            try:
                got = reassemble(text, workdir)
            except asm703.AsmError as e:
                bad.append('%04X: %s does not assemble: %s' % (word, text, e))
                continue
            compared += 1
            mask = 0xffff & ~ignored_mask(word)
            if got[0] != word & mask or got[1:] != [w & 0x7fff for w in raw[1:]]:
                bad.append('%04X: %s assembles to %s' % (word, text, ' '.join('%04X' % w for w in got)))
    log.write(('every word: %d compared, %d disagree\n' % (compared, len(bad))).encode())
    if bad:
        raise AssertionError('\n'.join(bad[:20]))
    if compared != 0x10000:
        raise AssertionError('only %d words compared' % compared)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix='ray703-dis-') as workdir, open(LOG_FILE, 'wb') as log:
        try:
            cards = 0
            if CONTROL != 'rename-words':
                cards = sum(check_listing(name, n, workdir, log) for name, n in REFERENCES)
            check_every_word(workdir, log)
        except Exception:
            log.write(b'\n--- harness failure ---\n')
            log.write(traceback.format_exc().encode())
            print('FAIL: ray703 disassembler test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1
    print('PASS: ray703 disassembler test (%d listing cards, every word; %s)' % (cards, EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
