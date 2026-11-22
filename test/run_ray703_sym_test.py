#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Assemble the Paper Tape I/O System with Raytheon's SYM I, and hold what
it punches to the binary tape of the same program.

Three tapes, as the University of Stuttgart's computer museum read them off
its 704 (tools/fetch-roms.py puts them in roms/703/tapes): SYM I/PREP, the
one-pass assembler for a 4K machine (DN 390470, 7/10/70), the SYM source of
PTIOS (DN 393954, 1981), and the PTIOS binary tape the museum ran. SYM I
assembles the source off the high speed reader and punches an absolute tape;
that tape has to be the binary tape, frame for frame from its first record
to its last, and the listing has to show every word of it.

SYM I is written into core at its origin and started at word X'800', where
its last record points. It owns the machine: its own interrupt-driven driver
for the teletype, the reader and the punch, nothing below X'5B0' but its
symbol table. No operating instructions for it are at hand (the 706 User's
Manual's 1-6.4.1 describes it, not how to run it), so this is what was read
out of the code and seen in runs:

- It prints `??` and reads a directive from the keyboard: a line feed and
  then exactly two characters, no carriage return -- the read takes two and
  disconnects, so a return typed after them would arrive at a teletype the
  program no longer expects anything from. `CT`, `CK` or `CH` takes the
  source from the teletype's reader, its keyboard or the high speed reader;
  `CP` or `CJ` punches the object on the teletype or the high speed punch.
  `SA` assembles absolute and `S ` (a space) relocatable; `P` is PREP, `X`
  punches a bootstrap and `D` starts over.
- It stops for the operator by showing a code in the accumulator's lamps,
  rolling it round, until SENSE switch 3 goes up and comes down again
  (`X'7B7'`, the code its argument): 1 to ready the source, 2 when the
  listing is done and the punch is wanted, 3 when the object is punched.
  A code above X'F00' is a fatal error, after which it prints the card and
  starts over.
- `SA` lists each word as its address, the word and the first four letters
  of its label; an error flags the card with a letter in front. The listing
  comes after the assembly, from core, and SENSE 0 up skips it or cuts it
  short, being tested before each word; this run leaves it down.

The session is paced on breakpoints rather than the clock: SYM I's keyboard
read (`X'6FC'`), stepped on to its wait so the teletype is listening before a
key goes in; its pause (`X'7B7'`), where SENSE 3 goes up; and the pause's
wait for the switch to come down (`X'7D1'`).

The binary tape's last word, X'16A', is in no card of the listing. SYM I
punches the same word there, so it is the program's own, a check word of
some kind; it is compared with the rest of the tape.

Negative controls, each of which must FAIL: SYM_CONTROL=source (one card's
operand changed) and =tape (one frame of SYM I's tape changed, in its
mnemonic table, so that LDX assembles as LDW).
"""

import os
import queue
import re
import shutil
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu, Stopped  # noqa: E402
import tape703  # noqa: E402

EMU_BIN = os.path.abspath(os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu')))
TAPE_DIR = os.path.abspath(os.environ.get('TAPE_DIR', os.path.join(ROOT, 'roms', '703', 'tapes')))
LOG_FILE = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, 'ray703_sym_test.log')
CONTROL = os.environ.get('SYM_CONTROL', '')
CONTROLS = ('source', 'tape')
TIMEOUT = float(os.environ.get('SYM_TIMEOUT', '120'))

SYM, SOURCE, BINARY = '390470.BIN', 'PTIOS_SYM.bin', 'PTIOS.bin'

# SYM I in core: where it starts, its keyboard read and the wait in it, its
# pause for the operator and the pause's wait for SENSE 3 to come down.
START, KEYBOARD, LISTENING, PAUSE, SWITCH_DOWN = 0x800, 0x6fc, 0x70f, 0x7b7, 0x7d1
SENSE_3 = 1 << 3
# The pause codes of one assembly, in order.
CODES = [1, 2, 3]


class Failed(Exception):
    pass


class Session:
    def __init__(self, emu, log):
        self.emu, self.log = emu, log
        self.deadline = time.monotonic() + TIMEOUT
        self.codes = []

    def stop(self):
        """Run to the next breakpoint and return its address."""
        self.emu.run()
        try:
            ev = self.emu.events.get(timeout=max(0.0, self.deadline - time.monotonic()))
        except queue.Empty:
            self.emu.halt()
            raise Failed('it went no further; it is at %04X' % self.emu.status()['pc']) from None
        if not isinstance(ev, Stopped):
            raise Failed('the emulator went away: %r' % (ev,))
        if ev.reason != 'break':
            raise Failed('stopped (%s) at %04X in front of `%s`' % (ev.reason, ev.pc, ev.insn))
        return ev.pc

    def operator(self):
        """At the pause: note its code, raise SENSE 3, and lower it again
        once the pause is waiting for that."""
        code = self.emu.memw(self.emu.reg('IX'), 1)[0] & 0xfff
        self.codes.append(code)
        self.log.write(('--- pause %X ---\n' % code).encode())
        if code >= 0xf00:
            # the card in error is printed after the switch, so let it go by
            self.emu.set_reg('SS', SENSE_3)
            self.until(lambda at: at == SWITCH_DOWN)
            self.emu.set_reg('SS', 0)
            self.until(lambda at: at == KEYBOARD)
            raise Failed('SYM I stopped with code %X: %r' % (code, self.printed()[-80:]))
        self.emu.set_reg('SS', SENSE_3)
        if self.stop() != SWITCH_DOWN:
            raise Failed('the pause did not wait for SENSE 3 to come down')
        self.emu.set_reg('SS', 0)

    def until(self, want):
        while True:
            at = self.stop()
            if want(at):
                return
            if at == PAUSE:
                self.operator()

    def directive(self, text):
        """At the `??`: wait for the keyboard read to be listening and type
        a line feed and the two characters."""
        self.until(lambda at: at == KEYBOARD)
        for _ in range(64):
            if self.emu.status()['pc'] == LISTENING:
                break
            self.emu.step()
        else:
            raise Failed('the keyboard read never reached its wait')
        self.emu.key('\n' + text)

    def printed(self):
        return bytes(c & 0x7f for c in self.emu.output).replace(b'\0', b'').decode('ascii', 'replace')


def words(frames):
    origin, image, _ = tape703.read_tape(frames)
    return origin, [image[i] << 8 | image[i + 1] for i in range(0, len(image), 2)]


def records(frames):
    """A tape's frames from its first record on, the trailer dropped."""
    first = frames.index(b'\x8a\x00\x02')
    return frames[first:].rstrip(b'\0')


def run(workdir, log):
    with open(os.path.join(TAPE_DIR, SYM), 'rb') as f:
        sym = f.read()
    with open(os.path.join(TAPE_DIR, SOURCE), 'rb') as f:
        source = f.read()
    with open(os.path.join(TAPE_DIR, BINARY), 'rb') as f:
        binary = f.read()

    if CONTROL == 'tape':
        # the mnemonic table: four letters and the opcode word, here LDX's
        entry = sym.index(bytes(c | 0x80 for c in b'LDX '))
        sym = sym[:entry + 4] + bytes([0x80]) + sym[entry + 5:]
    if CONTROL == 'source':
        card = bytes(c | 0x80 for c in b'LDX   H127')
        if card not in source:
            raise Failed('the control card is not on the source tape')
        source = source.replace(card, bytes(c | 0x80 for c in b'LDX   H9  '))

    origin, image, _ = tape703.read_tape(sym)
    core = os.path.join(workdir, 'sym.core')
    with open(core, 'wb') as f:
        f.write(bytes(origin * 2) + image)
    tape = os.path.join(workdir, 'source.bin')
    with open(tape, 'wb') as f:
        f.write(source)

    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', core, '--no-throttle', '--fast-io', '--halt'], log=log)
    try:
        session = Session(emu, log)
        for addr in (KEYBOARD, PAUSE, SWITCH_DOWN):
            emu.add_break(addr)
        emu.set_reg('PC', START)
        session.directive('CH')
        session.directive('CJ')
        emu.tape(0, tape)
        session.directive('SA')
        # the assembly, round to the next `??`
        session.until(lambda at: at == KEYBOARD)
        if session.codes != CODES:
            raise Failed('the pauses were %r, not %r' % (session.codes, CODES))

        printed = session.printed()
        log.write(('--- printed ---\n%s\n' % printed.replace('\r', '')).encode())
        listing = printed[printed.index('SA') + 2:]
        flagged = [line for line in listing.split('\r') if re.match(r'\n?[A-Z]', line)]
        if flagged:
            raise Failed('SYM I flagged %r' % flagged[:3])
        listed = {int(a, 16): int(w, 16) for a, w in re.findall(r'\r +([0-9A-F]{4}) +([0-9A-F]{4})', listing)}

        b_origin, b_words = words(binary)
        want = {b_origin + i: w for i, w in enumerate(b_words)}
        last = max(want)
        if sorted(listed) != [a for a in sorted(want) if a != last]:
            raise Failed('the listing has words %s, the binary tape %04X-%04X'
                         % (', '.join('%04X' % a for a in sorted(listed)[:4]) + '...', min(want), last))
        wrong = [a for a in listed if listed[a] != want[a]]
        if wrong:
            raise Failed('the listing has %d words other than the binary tape\'s, the first %04X: %04X, not %04X'
                         % (len(wrong), wrong[0], listed[wrong[0]], want[wrong[0]]))

        punched = os.path.join(workdir, 'punched.bin')
        emu.save_tape(punched)
        with open(punched, 'rb') as f:
            frames = f.read()
        if words(frames) != (b_origin, b_words):
            raise Failed('the punched tape is not the binary tape\'s program')
        if records(frames) != records(binary):
            raise Failed('the punched tape\'s records are not the binary tape\'s: %d frames, not %d'
                         % (len(records(frames)), len(records(binary))))
        return len(listed), len(records(frames))
    finally:
        if emu.child.poll() is None:
            emu.child.kill()
        emu.child.wait()
        emu.child.stdin.close()
        emu.close()
        if emu.socket_dir:
            shutil.rmtree(emu.socket_dir, ignore_errors=True)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    if CONTROL and CONTROL not in CONTROLS:
        print('error: SYM_CONTROL is one of %s' % ', '.join(CONTROLS), file=sys.stderr)
        return 1
    missing = [name for name in (SYM, SOURCE, BINARY) if not os.path.isfile(os.path.join(TAPE_DIR, name))]
    if missing:
        print('FAIL: ray703 sym test: no %s in %s' % (', '.join(missing), TAPE_DIR), file=sys.stderr)
        print('fetch them with: tools/fetch-roms.py', file=sys.stderr)
        return 1

    # The emulator runs in a directory of its own: from the root the 703
    # would mount whatever platters are in disks/, write-through.
    workdir = tempfile.mkdtemp(prefix='ray703-sym-')
    try:
        os.chdir(workdir)
        with open(LOG_FILE, 'wb') as log:
            try:
                listed, frames = run(workdir, log)
            except Exception as e:
                log.write(('\n--- failure ---\n' + traceback.format_exc()).encode())
                print('FAIL: ray703 sym test: %s (see %s)'
                      % (e if isinstance(e, Failed) else repr(e), LOG_FILE), file=sys.stderr)
                return 1
    finally:
        os.chdir(HERE)
        shutil.rmtree(workdir, ignore_errors=True)
    print('PASS: ray703 sym test, PTIOS assembled by SYM I: %d words listed, %d frames punched as on its binary tape (%s)'
          % (listed, frames, EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
