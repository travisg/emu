#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Turn a Raytheon 700-series absolute tape into an asm703.py deck that
assembles back to it.

Two steps. The first reduces an emulator trace of a run that works the
program to what the disassembly needs from it -- which words were executed,
and the extension register each one ran with:

    dis703.py trace TRACE -o RUN.json [--program HI START N[,N...]]

TRACE is `emu --trace` output, which may be a fifo: a run of any length
reduces to a few thousand lines. `--program` is for programs a loader
loads over each other: words from HI up count only while the program
started by the Nth entry into START is in core, an entry being a transfer
to START from below HI (the loader's), so that a program's own jumps to its
start do not count. Entries are numbered from 1, and a trace that begins
at START begins with the first.

The second writes the deck:

    dis703.py deck TAPE --run RUN.json [--run ...] [--alias LO-HI=AT]
                  [--notes NOTES] [-o DECK] [--check]

What is code is decided by execution, not by guessing: a word some run
executed is an instruction, and so is one reached from executed code by a
branch, a skip or falling through, unless it is the word after a JSX (where
a routine's arguments sit), zero, a cell some instruction reads or writes,
or text that does not assemble back to itself; those are marked `not run`.
Everything else is DATA, with its characters and what it would be as an
instruction in the comment. `--alias` is for code that runs somewhere
other than where the tape puts it: LO-HI executed is the tape's words from
AT on.

A direct operand's address is its M field under the extension register.
For an executed word that is the register the trace shows it ran with; for
the rest it is the page an SML or SMU in front selected, with only generics
between, or else the word's own. A location in the tape gets a label -- S
for a JSX's target, J for another branch's, D for anything else -- and one
outside it an EQU, R for an address code under `--alias` really runs at.
The text of each instruction is the emulator's own disassembler's, taken
over the debug port, with the operand rewritten as the label; a generic
must assemble back to its word with asm703.py or it is left as DATA.

`--notes` is what a person knows of the program -- names, comments and
code or data where the runs do not settle it -- kept apart from the deck so
that the deck can always be written again (see Notes). A deck is not edited
by hand.

`--check`, or on its own

    dis703.py check DECK TAPE

assembles the deck with asm703.py --werror and compares it with the tape
word for word; that is the test a deck is held to however it is commented.
asm703 lets a page selection reach only the reference right behind it,
where the machine lets it govern the next one to run: a reference the
machine takes into another page under a selection asm703 does not see is
written as the same M field in its own page, with where it lands beside it.
test/703/disasm is where the decks are kept.
"""

import argparse
import collections
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

import asm703  # noqa: E402
import tape703  # noqa: E402
from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))

# What ends a run of straight-line code, and what skips the word after it.
NO_FALL = {'JMP', 'HLT', 'INR'}
SKIPPERS = asm703.SKIPPERS
TWO_WORD = set(asm703.GEN_TWOWORD)


def reduce_trace(path, outs):
    """From a trace: the extension registers each executed word ran with,
    and every transfer other than to the next word or the one after (a
    skip) -- branches, calls, returns and interrupt entries as seen.

    `outs` is [(RUN.json, program)], all reduced in one pass; a program is
    None or (HI, START, entries) as `--program` says. Returned, for each:
    how many words ran, and how many entries into START there were."""
    seen = [collections.defaultdict(set) for _ in outs]
    flow = [set() for _ in outs]
    entries, prev = [0] * len(outs), None
    with open(path) as f:
        for line in f:
            if not line.startswith('PC=') or len(line) < 29:
                continue
            pc, ex = int(line[3:7], 16), int(line[27:29], 16)
            last, prev = prev, pc
            for k, (_, program) in enumerate(outs):
                if program is not None:
                    hi, start, which = program
                    if pc == start and (last is None or last < hi):
                        entries[k] += 1
                    if pc >= hi and entries[k] not in which:
                        continue
                seen[k][pc].add(ex)
                if last is not None and pc not in (last, last + 1, last + 2):
                    # a transfer from code another program loaded over this
                    # one is kept: it says who calls in, not what this one is
                    flow[k].add((last, pc))
    for (out, _), ran, edges in zip(outs, seen, flow):
        with open(out, 'w') as f:
            json.dump({'ran': {'%04X' % pc: sorted(v) for pc, v in sorted(ran.items())},
                       'flow': ['%04X %04X' % e for e in sorted(edges)]}, f, indent=0)
    return [(len(ran), n) for ran, n in zip(seen, entries)]


def emulator_text(origin, words):
    """The emulator's disassembly of every word, by address."""
    tmp = tempfile.mkdtemp(prefix='dis703-')
    try:
        core = os.path.join(tmp, 'core')
        with open(core, 'wb') as f:
            f.write(bytes(origin * 2) + b''.join(w.to_bytes(2, 'big') for w in words))
        emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle', '--halt'])
        try:
            text = {}
            for addr in range(origin, origin + len(words)):
                # one at a time: a word that looks like MPY would swallow the
                # next one in a listing of several
                _, raw, t = emu.dis(addr, 1)[0]
                text[addr] = (t, len(raw))
            return text
        finally:
            emu.kill()
            emu.child.wait()
            if emu.socket_dir:
                shutil.rmtree(emu.socket_dir, ignore_errors=True)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def split(text):
    op, _, arg = text.partition(' ')
    return op, arg.strip() or None


def assembles_to(text, word, addr):
    """Does a generic's text assemble back to its word?"""
    op, arg = split(text)
    if op in ('DATA', 'WORD') or op in asm703.MEMREF:
        return False
    try:
        return asm703.encode(0, op, arg, {}, addr) == word
    except asm703.AsmError:
        return False


def chars(word):
    """The word as two characters with bit 7 set, the machine's text, line
    feed and carriage return included -- or None."""
    out = ''
    for b in (word >> 8, word & 0xff):
        c = b & 0x7f
        if not b & 0x80:
            return None
        if 0x20 <= c < 0x7f:
            out += chr(c)
        elif c in (0x0a, 0x0d):
            out += '\\n' if c == 0x0a else '\\r'
        else:
            return None
    return out


class Notes:
    """What a person knows about a program, kept apart from the deck so the
    deck can be written again from the tape and the runs at any time.

        # a remark in the notes file itself
        0835 START          ; a comment on the line
        0835 > a line of commentary over it, as many as wanted
        0863 data           ; data whatever the runs say
        0950 code           ; an instruction, and a place to reach code from

    A name replaces the label the tool would make, wherever the address is
    referred to. Notes are keyed by the tape's address."""

    def __init__(self, path=None):
        self.names, self.comments, self.blocks, self.hints = {}, {}, collections.defaultdict(list), {}
        if not path:
            return
        with open(path) as f:
            for n, line in enumerate(f, 1):
                line = line.rstrip('\n')
                if not line.strip() or line.lstrip().startswith('#'):
                    continue
                m = re.match(r'\s*([0-9A-Fa-f]{1,4})\s*(.*)$', line)
                if not m:
                    raise SystemExit('%s:%d: a note begins with an address' % (path, n))
                addr, rest = int(m.group(1), 16), m.group(2)
                if rest.startswith('>'):
                    self.blocks[addr].append(rest[1:].strip())
                    continue
                body, _, comment = rest.partition(';')
                for token in body.split():
                    if token in ('code', 'data'):
                        self.hints[addr] = token
                    elif re.fullmatch(r'[A-Z][A-Z0-9.]{0,7}', token):
                        self.names[addr] = token
                    else:
                        raise SystemExit('%s:%d: %r is neither code, data nor a name' % (path, n, token))
                if comment.strip():
                    self.comments[addr] = comment.strip()


class Deck:
    def __init__(self, origin, words, text, runs, aliases, notes=None):
        self.origin, self.words, self.text = origin, words, text
        self.notes = notes or Notes()
        self.end = origin + len(words)
        self.aliases = aliases          # [(lo, hi, at)]
        self.ran = {}                   # image address -> set of ex
        self.runtime = {}               # image address -> where it ran
        self.flow = set()               # (from, to), runtime addresses
        for ran, flow in runs:
            for pc, exs in ran.items():
                at = self.image_of(pc)
                if at is not None:
                    self.ran.setdefault(at, set()).update(exs)
                    self.runtime[at] = pc
            self.flow |= flow

    def image_of(self, pc):
        for lo, hi, at in self.aliases:
            if lo <= pc <= hi:
                return at + pc - lo
        return pc if self.origin <= pc < self.end else None

    def where(self, at):
        """Where the word at `at` runs."""
        if at in self.runtime:
            return self.runtime[at]
        for lo, hi, base in self.aliases:
            if base <= at <= base + hi - lo:
                return lo + at - base
        return at

    def word(self, at):
        return self.words[at - self.origin]

    def op(self, at):
        return split(self.text[at][0])[0]

    def selected(self, at):
        """The byte page an SML/SMU in front of `at` selects, with only
        generics between, or None."""
        prev = at - 1
        while prev >= self.origin and prev in self.code:
            w = self.word(prev)
            op = self.op(prev)
            if w >> 12:
                return None
            if op in ('SML', 'SMU'):
                return (w & 0xf) | (0x10 if op == 'SMU' else 0)
            if op in asm703.SELECTION_ENDS or op == 'CXE':
                return None
            prev -= 1
        return None

    def licensed(self, at):
        """Does asm703 take a direct operand at `at` as under a selection?
        Its rule is stricter than the machine's: a skip, like a reference
        or data, ends the selection, since the skip may pass over the
        reference it was for."""
        prev = at - 1
        while prev >= self.origin and prev in self.code:
            w = self.word(prev)
            op = self.op(prev)
            if op in ('SML', 'SMU'):
                return True
            if w >> 12 or op in SKIPPERS or op in asm703.SELECTION_ENDS or op in TWO_WORD:
                return False
            prev -= 1
        return False

    def ex(self, at):
        """The extension register a memory reference at `at` uses, and a
        note when the runs disagree with each other or with the code."""
        static = self.selected(at)
        if static is None:
            static = (self.where(at) >> 10) & 0x1f
        seen = self.ran.get(at)
        if not seen:
            return static, None
        if len(seen) == 1:
            ex = next(iter(seen))
            return ex, None if ex == static else 'page from the run'
        ex = static if static in seen else min(seen)
        return ex, 'ran with EXR %s' % ', '.join('%02X' % e for e in sorted(seen))

    def target(self, at):
        """(kind, address) of a direct memory reference: kind 'w' a word
        address, 'b' a byte address."""
        w = self.word(at)
        ex, _ = self.ex(at)
        m = w & 0x7ff
        if self.op(at) in asm703.BYTE_REF:
            return 'b', (ex << 11) | m
        return 'w', ((ex >> 1) << 11) | m

    def classify(self):
        self.code = set(a for a in self.ran if self.text[a][0].split()[0] not in ('DATA', 'WORD'))
        hinted = set(a for a, h in self.notes.hints.items() if h == 'code' and self.origin <= a < self.end)
        self.code |= hinted
        self.code -= set(a for a, h in self.notes.hints.items() if h == 'data')
        self.learn_arguments()
        refs = set()
        for a in sorted(self.code):
            w = self.word(a)
            if w >> 12 and not w & 0x800 and self.op(a) not in ('JMP', 'JSX'):
                kind, t = self.target(a)
                refs.add(self.image_of(t >> 1 if kind == 'b' else t))
        self.static = set(hinted - set(self.ran))
        todo = list(self.code)
        while todo:
            a = todo.pop()
            for s in self.successors(a):
                if s in self.code or not self.origin <= s < self.end:
                    continue
                w = self.word(s)
                t, n = self.text[s]
                op, _ = split(t)
                if w == 0 or s in refs or op in ('DATA', 'WORD') or self.notes.hints.get(s) == 'data':
                    continue
                if not w >> 12 and not assembles_to(t, w, s):
                    continue
                self.code.add(s)
                self.static.add(s)
                todo.append(s)
                if w >> 12 and not w & 0x800 and op not in ('JMP', 'JSX'):
                    kind, tt = self.target(s)
                    refs.add(self.image_of(tt >> 1 if kind == 'b' else tt))

    def learn_arguments(self):
        """How many words of arguments each routine takes after its JSX,
        from the calls that ran: where the runs saw control come back to,
        from somewhere other than the word in front. A routine that
        returns to more than one place (a skip return) takes as many as
        its nearest return says.

        The words after a call that ran are no guide by themselves: a skip
        in front of a JSX passes over its first word only, so its argument
        runs -- PTIOS puts a skip there on purpose, with an argument that
        does nothing as an instruction."""
        back = collections.defaultdict(set)
        for src, dst in self.flow:
            back[dst].add(src)
        seen = collections.defaultdict(list)
        for a in self.code:
            w = self.word(a)
            if w >> 12 != 2 or w & 0x800 or a not in self.ran:
                continue
            rc = self.where(a)
            for k in range(1, 8):
                if any(src != rc for src in back.get(rc + k, ())):
                    seen[self.target(a)[1]].append(k - 1)
                    break
        self.arguments = {t: min(ks) for t, ks in seen.items()}

    def args_of(self, a):
        """The argument words behind a direct JSX at `a`, or None."""
        if self.op(a) != 'JSX' or self.word(a) & 0x800:
            return None
        return self.arguments.get(self.target(a)[1])

    def accesses(self):
        """image address -> [(instruction, 'r' or 'w')] for direct operands."""
        out = collections.defaultdict(list)
        for a in sorted(self.code):
            w = self.word(a)
            op = self.op(a)
            if not w >> 12 or w & 0x800 or op in ('JMP', 'JSX'):
                continue
            kind, t = self.target(a)
            img = self.image_of(t >> 1 if kind == 'b' else t)
            if img is not None:
                out[img].append((a, 'w' if op in ('STW', 'STX', 'STB') else 'r'))
        return out

    def successors(self, a):
        w = self.word(a)
        op = self.op(a)
        out = []
        if w >> 12:
            if not w & 0x800 and op in ('JMP', 'JSX'):
                out.append(self.image_of(self.target(a)[1]) or -1)
            if op == 'JSX':
                n = self.args_of(a)
                return out + ([a + 1 + n] if n is not None else [])
            if op == 'JMP':
                return out
            return out + [a + 1]
        if op in NO_FALL:
            return out
        if op in TWO_WORD:
            return [a + 2]
        if op in SKIPPERS:
            return [a + 1, a + 2]
        return [a + 1]

    def write(self, out, title):
        names = {}          # address -> name, for image words
        equs = {}           # address -> name, outside the image
        kinds = {}
        for a in sorted(self.code):
            w = self.word(a)
            if not w >> 12 or w & 0x800 or self.op(a) in TWO_WORD:
                continue
            kind, t = self.target(a)
            wt = t >> 1 if kind == 'b' else t
            op = self.op(a)
            k = 'S' if op == 'JSX' else 'J' if op == 'JMP' else 'D'
            aliased = any(lo <= wt <= hi for lo, hi, _ in self.aliases)
            if self.origin <= wt < self.end and not aliased:
                kinds[wt] = min(kinds.get(wt, 'D'), k, key='SJD'.index)
            else:
                equs[wt] = ('R' if aliased else 'A') + '%04X' % wt
        # entered by a computed jump, or from outside the tape
        for src, dst in self.flow:
            to, frm = self.image_of(dst), self.image_of(src)
            if to is None or to not in self.code or to in kinds or to != dst:
                continue
            if frm is None:
                kinds[to] = 'E'
            elif self.word(frm) >> 12 == 1 and self.word(frm) & 0x800:
                kinds[to] = 'J'
        for t, k in kinds.items():
            names[t] = k + '%04X' % t
        for t in names:
            if t in self.notes.names:
                names[t] = self.notes.names[t]
        for t in equs:
            img = self.image_of(t)
            if img is not None and img in self.notes.names:
                equs[t] = self.notes.names[img]
        self.names = names
        access = self.accesses()
        argument = {}
        for a in self.code:
            n = self.args_of(a)
            for i in range(1, (n or 0) + 1):
                argument[a + i] = a

        lines = []
        lines.append('; %s' % title)
        lines.append(';')
        lines.append('; Written by tools/dis703.py. A line marked "not run" is an')
        lines.append('; instruction only by being reached from one that ran.')
        lines.append('')
        for t in sorted(equs):
            note = ''
            img = self.image_of(t)
            if img is not None and img != t:
                note = '  ; the tape\'s %04X, where it runs' % img
            lines.append('%-8s EQU     X\'%04X\'%s' % (equs[t], t, note))
        if equs:
            lines.append('')
        lines.append('         ORG     X\'%04X\'' % self.origin)
        a = self.origin
        while a < self.end:
            w = self.word(a)
            label = names.get(a, '')
            slot = a in self.code and w == 0 and any(m == 'w' for _, m in access.get(a, ()))
            if a in self.code and not slot:
                stmt, notes, n = self.instruction(a, names, equs)
            else:
                stmt, notes, n = self.data(a)
            if slot:
                notes.insert(0, 'run: an instruction stored here')
            if a in self.static:
                notes.insert(0, 'not run')
            if a in argument:
                notes.insert(0, 'argument %d of %04X' % (a - argument[a], argument[a]))
                if a in self.code:
                    notes.insert(1, 'run when a skip passes over the JSX')
            if a in access:
                acc = access[a]
                for mode, word in (('w', 'written'), ('r', 'read')):
                    by = sorted(set(x for x, m in acc if m == mode))
                    if by:
                        notes.append('%s by %s' % (word, ' '.join('%04X' % x for x in by[:4])
                                                   + (' ...' if len(by) > 4 else '')))
            where = self.where(a)
            if where != a:
                notes.insert(0, 'runs at %04X' % where + (' = ' + equs[where] if where in equs else ''))
            for block in self.notes.blocks.get(a, ()):
                lines.append(('; ' + block).rstrip())
            if not label and a in self.notes.names:
                # named, but nothing refers to it: a label on a cell nobody
                # names is a warning in asm703, so the name is said instead
                notes.insert(0, self.notes.names[a] + ':')
            comment = '%04X %s' % (a, ' '.join('%04X' % self.word(a + i) for i in range(n)))
            if a in self.notes.comments:
                comment += '  ' + self.notes.comments[a] + ('  --' if notes else '')
            if notes:
                said = notes.pop(0) + ' ' if notes[0].endswith(':') else ''
                comment += '  ' + said + '; '.join(notes)
            lines.append(('%-8s %-24s ; %s' % (label, stmt, comment)).rstrip())
            a += n
        lines.append('         END')
        with open(out, 'w') as f:
            f.write('\n'.join(lines) + '\n')

    def instruction(self, a, names, equs):
        w = self.word(a)
        text, n = self.text[a]
        op, arg = split(text)
        notes = []
        if not w >> 12:
            return '%-7s %s' % (op, arg or ''), notes, n
        if w & 0x800:
            return '%-7s %s' % (op, arg), notes, 1
        kind, t = self.target(a)
        _, why = self.ex(a)
        if why:
            notes.append(why)
        wt = t >> 1
        # asm703 lets a selection license only the reference right behind
        # it, but the machine lets it govern the next one to run: SYM I
        # selects once in front of a skip and two branches. The word is the
        # same M field in the instruction's own page.
        own = self.where(a) >> (10 if kind == 'b' else 11)
        page = t >> 11
        if page != own and not self.licensed(a):
            alias = (own << 11) | (t & 0x7ff)
            dest = names.get(t if kind == 'w' else wt) or equs.get(t if kind == 'w' else wt)
            notes.append('lands at %04X%s under the selection in force' % (
                t, ' = ' + dest if dest else ''))
            return '%-7s X\'%04X\'' % (op, alias), notes, 1
        if kind == 'w':
            name = names.get(t) or equs.get(t)
            operand = name or 'X\'%04X\'' % t
        else:
            if wt in names:
                operand = names[wt] + ('+1' if t & 1 else '')
            else:
                operand = 'X\'%04X\'' % t
                if wt in equs:
                    notes.append('byte %d of %s' % (t & 1, equs[wt]))
        return '%-7s %s' % (op, operand), notes, 1

    def data(self, a):
        w = self.word(a)
        notes = []
        c = chars(w)
        if c:
            notes.append("'%s'" % c)
        text = self.text[a][0]
        if not c and w >> 12 and not text.startswith('DATA'):
            notes.append(text)
        return 'DATA    X\'%04X\'' % w, notes, 1


def check(deck, origin, words):
    tmp = tempfile.mkdtemp(prefix='dis703-')
    try:
        mp = os.path.join(tmp, 'map')
        r = subprocess.run([sys.executable, os.path.join(HERE, 'asm703.py'), '--werror', deck, '-m', mp,
                            '-o', os.path.join(tmp, 'img')], capture_output=True, text=True)
        if r.returncode:
            return 'does not assemble:\n' + r.stdout + r.stderr
        got = {}
        with open(mp) as f:
            for line in f:
                if line.strip():
                    a, v = line.split()[:2]
                    got[int(a, 16)] = int(v, 16)
        bad = [a for a in range(origin, origin + len(words)) if got.get(a) != words[a - origin]]
        extra = [a for a in got if not origin <= a < origin + len(words)]
        if bad or extra:
            return '%d words differ (first %s), %d outside the tape' % (
                len(bad), ', '.join('%04X' % a for a in bad[:5]), len(extra))
        return None
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    sub = ap.add_subparsers(dest='cmd', required=True)
    t = sub.add_parser('trace', help='reduce an emulator trace')
    t.add_argument('trace')
    t.add_argument('-o', '--output', required=True)
    t.add_argument('--program', nargs=3, metavar=('HI', 'START', 'N'))
    d = sub.add_parser('deck', help='write a deck for a tape')
    d.add_argument('tape')
    d.add_argument('--run', action='append', default=[], help='a reduced trace')
    d.add_argument('--alias', action='append', default=[], metavar='LO-HI=AT')
    d.add_argument('-o', '--output')
    d.add_argument('--title')
    d.add_argument('--check', action='store_true')
    d.add_argument('--notes', help='names, comments and hints, by address')
    c = sub.add_parser('check', help='does a deck assemble to a tape, word for word')
    c.add_argument('deck')
    c.add_argument('tape')
    args = ap.parse_args()

    if args.cmd == 'check':
        with open(args.tape, 'rb') as f:
            origin, image, _ = tape703.read_tape(f.read())
        problem = check(args.deck, origin, [image[i] << 8 | image[i + 1] for i in range(0, len(image), 2)])
        if problem:
            print('FAIL: %s %s' % (args.deck, problem), file=sys.stderr)
            return 1
        print('ok: %s assembles to %s' % (args.deck, args.tape))
        return 0

    if args.cmd == 'trace':
        program = None
        if args.program:
            hi, start, which = args.program
            program = (int(hi, 16), int(start, 16), set(int(n) for n in which.split(',')))
        (n, _), = reduce_trace(args.trace, [(args.output, program)])
        print('%s: %d words executed' % (args.output, n))
        return 0

    with open(args.tape, 'rb') as f:
        origin, image, _ = tape703.read_tape(f.read())
    words = [image[i] << 8 | image[i + 1] for i in range(0, len(image), 2)]
    runs = []
    for path in args.run:
        with open(path) as f:
            run = json.load(f)
        runs.append(({int(k, 16): set(v) for k, v in run['ran'].items()},
                     set(tuple(int(x, 16) for x in e.split()) for e in run.get('flow', []))))
    aliases = []
    for spec in args.alias:
        m = re.fullmatch(r'([0-9A-Fa-f]+)-([0-9A-Fa-f]+)=([0-9A-Fa-f]+)', spec)
        if not m:
            ap.error('--alias is LO-HI=AT, in hex')
        aliases.append(tuple(int(x, 16) for x in m.groups()))
    deck = Deck(origin, words, emulator_text(origin, words), runs, aliases, Notes(args.notes))
    deck.classify()
    out = args.output or os.path.splitext(os.path.basename(args.tape))[0] + '.asm'
    deck.write(out, args.title or os.path.basename(args.tape))
    print('%s: %04X-%04X, %d instructions run, %d more reached from them, %d words of data'
          % (out, origin, origin + len(words) - 1, len(deck.code - deck.static), len(deck.static),
             len(words) - len(deck.code)))
    if args.check:
        problem = check(out, origin, words)
        if problem:
            print('FAIL: %s %s' % (out, problem), file=sys.stderr)
            return 1
        print('ok: %s assembles to the tape' % out)
    return 0


if __name__ == '__main__':
    sys.exit(main())
