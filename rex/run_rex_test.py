#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for REX, the 703 executive: boot it, start the three
letter tasks from its shell and watch them run -- the scheduler, the line
clock, the context switch, the sleeps and the mailbox teletype driver all
at once -- load the hello module off a scratch platter both ways (RUN,
which waits for it, and LOAD, which leaves it running behind the prompt)
and see the pool's free words come back, run two Tiny BASIC sessions with
a silent loop broken by Ctrl-C, then drive the rest of the commands and
shut the machine down with HALT.

The emulator is driven over its debug port (tools/emudbg.py), started
halted so no output is missed: keys go in as `key` commands, the paper is
the port's copy of the serial output, and the shutdown is the port's own
`! stopped reason=hlt`. The platter is made fresh in a scratch directory
and the emulator run from there, since it mounts disks/ray703-disc0.img
relative to where it runs -- the repo root's is the user's own.

--fast-io: the teletype's pacing is not what is under test, and the
scheduling slices stay machine time regardless, since the line clock
ignores the flag.

Every wait raises on its timeout and the handler at the bottom turns that
into a FAIL with the transcript in the log. The walk over the output keeps
a cursor: the same prompt comes round again, and a wait without one is
satisfied by the first. The printer is shared character by character, so
nothing here matches a line a letter task could be printing into -- the
letter tasks are stopped before anything is matched exactly.

The filesystem is held to the host's reading of it (tools/fs703.py): the
session runs the file calls' exerciser (fst.asm) twice, makes, copies,
renames and deletes files with the utilities, deletes a file the platter
was made with in front of everything else and has SQUISH move the lot
down over it, and afterwards the platter's directory has to pass the
host's check of its rules, be one run of files with one hole behind it,
hold exactly the text files expected with the bytes the guest wrote, and
still hold every module byte for byte -- and the platter as it was made
has to hold none of the guest's files, so the check cannot pass on
nothing.

A second machine, without --fast-io so that a transfer takes its time,
runs three copies of the disc exerciser (dskt.asm) at once. They print
nothing; each leaves its verdict in a sector of its own, read here off
the platter, and the kernel's count of tasks that found the disc taken
must not be zero -- three tasks that never met at the disc prove nothing
about the lock.

REX_CONTROL names a negative control, each of which must FAIL:
  blank    a platter with nothing on it (no catalogue lines, no modules)
  nohello  a platter without the hello module
  nolock   the kernel's wait for the disc patched out, so a task that
           finds it taken goes in anyway
  nosave   the directory never written back to the disc
  noguard  the kernel's refusal of a console read by a task that does
           not have the console patched out
"""

import contextlib
import os
import re
import subprocess
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))

from emudbg import Emu, Symbols  # noqa: E402
import fs703  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
BUILD = os.path.join(HERE, 'build')
ROM_FILE = os.path.join(BUILD, 'rex.bin')
HELLO_OBJ = os.path.join(BUILD, 'hello.obj')
BASIC_OBJ = os.path.join(BUILD, 'basic.obj')
DSKT_OBJ = os.path.join(BUILD, 'dskt.obj')
FST_OBJ = os.path.join(BUILD, 'fst.obj')
MODULES = (('HELLO', HELLO_OBJ), ('BASIC', BASIC_OBJ), ('DSKT', DSKT_OBJ), ('FST', FST_OBJ)) + tuple(
    (name, os.path.join(BUILD, name.lower() + '.obj'))
    for name in ('TYPE', 'COPY', 'DEL', 'REN', 'SQUISH', 'DUMP', 'PUT', 'NAP'))
# Every module but BASIC goes on the platter marked as loadable above
# word X'4000', where the kernel then puts it.
LOW_ONLY = ('BASIC',)

# The pool and the node arena, as rex.asm lays them out.
POOLB, POOLH, POOLE = 0x1000, 0x4000, 0x7000
NARB, NARN, T_LEN, T_STA, T_NAM, SFREE = 0x7e00, 24, 18, 0, 10, 4


def blocks_of(emu, name):
    """The pool blocks of the loaded task whose two-character name this
    is, as (first word, one past the last) of each payload. The machine
    is halted for the look and set running again."""
    emu.halt()
    try:
        # the port gives 128 words to a request
        arena = []
        for start in range(NARB, NARB + NARN * T_LEN, 108):
            arena += emu.memw(start, 108)
        packed = ((ord(name[0]) | 0x80) << 8) | (ord(name[1]) | 0x80)
        nodes = [NARB + i * T_LEN for i in range(NARN)
                 if arena[i * T_LEN + T_STA] != SFREE and arena[i * T_LEN + T_NAM] == packed]
        if len(nodes) != 1:
            raise AssertionError('%d tasks named %s' % (len(nodes), name))
        mine, at = [], POOLB
        while at < POOLE:
            size, tag = emu.memw(at, 2)     # a block's header: its size, its owner
            if tag == 0x8000 | nodes[0]:
                mine.append((at + 2, at + 2 + size))
            at += size + 2
        if at != POOLE:
            raise AssertionError('the pool\'s blocks end at %04x' % at)
        return mine
    finally:
        emu.run()

# What the platter is made with besides the modules. PAD.TX goes on first,
# in front of everything, so that deleting it leaves a hole SQUISH has to
# move every other file down over. MSG.TX has a host's bare newlines.
PAD = bytes(range(256)) + bytes(144)
MSG = b'FIRST LINE\nSECOND LINE\n'
CMDS = b'ECHO FROM A FILE\r\nMEM\r\nHELLO\r\n'
INIT = b'ECHO INIT RAN\r\n'
SQ = b'10 PRINT "FROM A HOST FILE"\n20 FOR I=1 TO 3\n30 PRINT I*I\n40 NEXT I\n'
# What PUT is given, and what it puts: the keys as the teletype sends
# them, bit 7 set, a line feed after every carriage return.
NOTE_KEYS = 'LINE ONE\rLINE TWO\r'
NOTE = bytes(c | 0x80 for c in b'LINE ONE\r\nLINE TWO\r\n')
REX_LST = os.path.join(BUILD, 'rex.lst')
MKDISC = os.path.join(ROOT, 'tools', 'mkdisc703.py')
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(BUILD, 'rex_test.log')
CONTROL = os.environ.get('REX_CONTROL', '')

# The prompt, allowing for a letter task's letter landing anywhere inside
# it: a task started at the prompt can put its letter between any two of
# the shell's characters.
PROMPT = re.compile(rb'R[ABC]*E[ABC]*X[ABC]*>')
READY = re.compile(rb'READY')


@contextlib.contextmanager
def cwd(path):
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


class Session:
    """The paper, walked in order."""

    def __init__(self, emu):
        self.emu = emu
        self.at = 0             # where the next wait starts

    def wait(self, pattern, timeout=None):
        m = self.emu.wait_for_output(pattern, timeout=timeout, start=self.at)
        self.at = m.end()
        return m

    def say(self, line, quiet=True):
        """Type a command at the next prompt -- once the printer has gone
        quiet, unless the letter tasks are printing and it never will.
        Returns where the paper stood when the keys went in."""
        self.wait(PROMPT)
        if quiet:
            self.emu.wait_quiet()
        mark = len(self.emu.output)
        self.emu.key(line + '\r')
        return mark

    def basic(self, line):
        """The same, at BASIC's prompt."""
        self.wait(READY)
        self.emu.wait_quiet()
        self.emu.key(line)

    def since(self, mark, end=None):
        return bytes(self.emu.output[mark:end])


def letters_ran(text):
    """Every letter task has run several times and the printer has changed
    hands repeatedly. Thresholds rather than exact counts: the tasks sleep
    for 30, 45 and 60 ticks and drift through every phase against each
    other."""
    only = re.sub(rb'[^ABC]', b'', text)
    runs = len(re.sub(rb'(.)\1+', rb'\1', only))
    return all(only.count(c) >= 5 for c in (b'A', b'B', b'C')) and runs >= 9


def session(emu):
    s = Session(emu)
    if CONTROL == 'nosave':
        # FDSAVE is STX slot / LDW FK1 / JSX FDXFR: a CLR in place of the
        # call and the save answers "clean" having written nothing
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('FDSAVE') + 2, [CLR])
    if CONTROL == 'noguard':
        # Q.GET is MSK / STX / STW / CMW KCONSQ / SEQ / JMP QGIN: without
        # the skip every caller is let in
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('Q.GET') + 4, [NOP])
    emu.run()
    s.wait(b'REX 703 UP')

    # INIT.CM is read before the keyboard is: its one command, echoed as
    # if typed, and what the command printed.
    s.wait(re.compile(rb'^INIT RAN\r', re.M))

    # Nothing is running yet, so this one is quiet and exact.
    s.say('STAT')

    # Set the three going and let them run. What follows the echo of START
    # until STOP is typed is prompts and letters, and neither the prompt
    # nor anything else the shell prints there contains an A, a B or a C.
    mark = s.say('START')
    after = emu.wait_for_output(b'START', start=mark).end()
    deadline = time.monotonic() + 30
    while not letters_ran(s.since(after)):
        if time.monotonic() > deadline:
            raise AssertionError('the letter tasks did not all run: %r' % s.since(after))
        time.sleep(0.05)

    # Then stop them: with the printer quiet every command's output can be
    # matched exactly, and waiting for quiet becomes usable for pacing.
    s.say('STOP', quiet=False)

    s.say('STAT')
    s.say('HELP')
    s.say('ECHO SHELL OUTPUT OK')
    s.say('FROB')

    # The hello module, both ways. RUN waits for it, so its line is clean
    # and the free words are back by the next prompt; LOAD leaves it to run
    # behind the prompt, where a STAT typed at once finds it on the ring,
    # and STAT is repeated until it has gone before the pool is counted
    # again. A name not in the catalogue is refused.
    s.say('MEM')
    s.say('DIR')
    s.say('RUN HELLO')
    s.wait(b'HELLO FROM WORD')
    s.say('MEM')
    s.say('LOAD HELLO')
    mark = s.say('STAT', quiet=False)
    seen = False
    for _ in range(40):
        s.wait(PROMPT)
        emu.wait_quiet()
        if not re.search(rb'^\d+ HE ', s.since(mark), re.M):
            break
        seen = True
        mark = len(emu.output)
        emu.key('STAT\r')
    else:
        raise AssertionError('hello never left the ring')
    if not seen:
        raise AssertionError('STAT never showed the loaded hello on the ring')
    emu.key('MEM\r')
    s.say('RUN NOPE')

    # The file calls, twice: the second run begins by finding the file
    # the first one left unclosed, and has to make a hole of it.
    for _ in range(2):
        s.say('RUN FST')
        s.wait(re.compile(rb'FST (OK|BAD \d+)'))
    s.say('DIR')
    s.say('MEM')

    # A word that is not a command is a program: its name, then the rest
    # of the line for it to read.
    s.say('HELLO')
    s.wait(b'HELLO FROM WORD')
    s.say('TYPE MSG')
    s.say('PUT NOTE')
    emu.key(NOTE_KEYS + '\x1a')        # the shell's queue holds it for PUT
    s.say('TYPE NOTE')
    s.say('COPY NOTE COPY2')
    s.say('REN COPY2 THIRD')
    s.say('DEL NOTE')
    s.say('DEL NOPE')
    s.say('DUMP THIRD')
    mark = s.say('DIR')
    s.wait(b'SECTORS FREE')
    listed = s.since(mark)
    for name, there in ((b'THIRD.TX 1', True), (b'NOTE.TX', False), (b'COPY2.TX', False)):
        if (re.search(rb'^' + re.escape(name), listed, re.M) is not None) != there:
            raise AssertionError('DIR after the utilities: %r' % listed)

    # The file in front of everything goes, and every other file moves
    # down over the hole -- the modules too, which is what the next
    # program run is loaded from.
    s.say('DEL PAD')
    s.say('SQUISH')
    s.say('HELLO')
    s.wait(b'HELLO FROM WORD')
    s.say('@CMDS')
    s.wait(re.compile(rb'^FROM A FILE\r', re.M))
    s.wait(b'HELLO FROM WORD')
    s.say('MEM')

    # Jobs. NAP never ends: started behind the prompt it first tries to
    # read the console, which the kernel refuses, and then sleeps for
    # ever. STOP, START and KILL take its number, as STAT gives it.
    mark = s.say('LOAD NAP')
    emu.wait_for_output(b'THE CONSOLE WAS REFUSED', start=mark)
    emu.wait_quiet()
    held = blocks_of(emu, 'NA')
    if not held or any(first < POOLH for first, _ in held):
        raise AssertionError('a module marked for it was not loaded high: %r' % held)
    mark = s.say('STAT')
    job = emu.wait_for_output(re.compile(rb'^(\d+) NA ', re.M), start=mark).group(1).decode()
    s.say('STOP ' + job)
    mark = s.say('STAT')
    emu.wait_for_output(re.compile(rb'^' + job.encode() + rb' NA OFF', re.M), start=mark)
    s.say('START ' + job)
    s.say('KILL ' + job)
    mark = s.say('STAT')
    emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark)
    emu.wait_quiet()
    if re.search(rb'^\d+ NA ', s.since(mark), re.M):
        raise AssertionError('NAP is still there after KILL')
    mark = s.say('KILL ' + job)
    emu.wait_for_output(b'NO SUCH TASK', start=mark)
    s.say('MEM')

    # In the foreground it has the console and takes no notice of Ctrl-C,
    # so the second one is the shell's to act on.
    s.say('NAP')
    emu.wait_quiet()
    emu.key('\x03')
    time.sleep(0.2)
    emu.key('\x03')
    s.wait(b'KILLED')
    s.say('MEM')

    # A BASIC session: the module comes off the platter, the console
    # changes hands, a program goes in and runs, and Ctrl-C -- which never
    # enters the queue; SERV raises the kernel's break flag -- stops a loop
    # that prints nothing and reads nothing.
    s.say('RUN BASIC')
    s.wait(b'TINY BASIC UNDER REX')
    s.wait(READY)
    emu.wait_quiet()
    held = blocks_of(emu, 'BA')         # the module and its workspace
    if len(held) != 2 or any(end > POOLH for _, end in held):
        raise AssertionError('BASIC is not all below word 4000: %r' % held)
    s.at -= len(b'READY')               # for the next line to find
    s.basic('10 FOR I=1 TO 3\r')
    s.basic('20 PRINT "SQ";I*I\r')
    s.basic('30 NEXT I\r')
    s.basic('50 GOTO 50\r')
    s.basic('RUN\r')
    s.wait(b'SQ9')
    emu.wait_quiet()            # the loop at 50 prints nothing: quiet is in it
    emu.key('\x03')
    s.wait(b'BREAK AT 50')

    # A break belongs to a run. This one is typed at the prompt, where
    # nothing is running, so the interpreter spends it when the next line
    # goes in and this RUN reaches the loop again instead of breaking at
    # its first statement -- which is what a kept flag would do.
    s.basic('\x03')
    time.sleep(0.5)
    emu.key('RUN\r')
    s.wait(b'SQ9')
    emu.wait_quiet()
    emu.key('\x03')
    s.wait(b'BREAK AT 50')
    s.basic('SAVE PROG\r')
    s.basic('BYE\r')

    # Back at the shell: BYE gave back the module, its node and its
    # workspace, so STAT has no BASIC node, MEM reads as before, and a
    # second session -- the bare word BASIC runs the program of that
    # name -- starts afresh, with nothing to LIST until OLD brings the
    # saved program back. Two more are made and saved for what follows:
    # one that ends, and one that never does.
    mark = s.say('STAT')
    s.say('MEM')
    if re.search(rb'^\d+ BA ', s.since(mark), re.M):
        raise AssertionError('BASIC is still on the ring after BYE')
    s.say('BASIC')
    mark = len(emu.output)
    s.basic('LIST\r')
    s.basic('OLD PROG\r')
    if b'PRINT' in s.since(mark):
        raise AssertionError('a fresh BASIC had a program to list')
    mark = len(emu.output)
    s.basic('LIST\r')
    s.basic('NEW\r')
    if not re.search(rb'^10 FOR I=1 TO 3\r\n20 PRINT "SQ";I\*I\r\n30 NEXT I\r\n50 GOTO 50\r', s.since(mark), re.M):
        raise AssertionError('OLD did not bring the saved program back: %r' % s.since(mark))
    s.basic('10 FOR I=1 TO 3\r')
    s.basic('20 PRINT "BG";I\r')
    s.basic('30 NEXT I\r')
    s.basic('SAVE BG\r')
    s.basic('NEW\r')
    s.basic('10 PRINT "LP"\r')
    s.basic('20 GOTO 20\r')
    s.basic('SAVE LP\r')
    mark = len(emu.output)
    s.basic('OLD NOPE\r')
    s.basic('BYE\r')
    if not re.search(rb'^HOW\?', s.since(mark), re.M):
        raise AssertionError('OLD of a file that is not there: %r' % s.since(mark))

    # A program named on the command line is loaded and run. In the
    # foreground the session stays at READY afterwards -- this one is a
    # host's file, with bare newlines -- and behind the prompt the task
    # prints what the program prints and is gone when the program ends.
    s.say('BASIC SQ')
    s.wait(b'FROM A HOST FILE\r\n1\r\n4\r\n9\r\n')
    s.basic('BYE\r')
    mark = s.say('LOAD BASIC BG')
    emu.wait_for_output(b'BG3', start=mark)
    for _ in range(40):
        s.wait(PROMPT)
        emu.wait_quiet()
        mark = len(emu.output)
        emu.key('STAT\r')
        emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark)
        if not re.search(rb'^\d+ BA ', s.since(mark), re.M):
            break
    else:
        raise AssertionError('the background BASIC never ended')
    s.say('MEM')

    # The break is the console's. A BASIC behind the prompt does not have
    # the console, so a Ctrl-C typed at the shell is not its to take: the
    # loop is still running afterwards, and KILL is what ends it.
    mark = s.say('LOAD BASIC LP')
    emu.wait_for_output(b'LP', start=mark)
    emu.wait_quiet()
    emu.key('\x03')
    time.sleep(0.5)
    emu.key('STAT\r')
    job = emu.wait_for_output(re.compile(rb'^(\d+) BA RUN', re.M), start=mark).group(1).decode()
    if b'BREAK' in s.since(mark):
        raise AssertionError('a background BASIC took the console\'s break')
    s.say('KILL ' + job)
    s.say('MEM')

    # The console has one reader, and LOAD grants it to nobody, so a module
    # that reads it refuses rather than registering over the shell's place
    # in the queue. The shell must still be answering afterwards, and the
    # MEM is what says the refusal gave its memory back.
    # The refusal prints behind the prompt LOAD returns to, so
    # the cursor stays where it is for the MEM to find that prompt.
    mark = s.say('LOAD BASIC')
    emu.wait_for_output(b'THE CONSOLE: USE RUN BASIC', start=mark)
    emu.wait_quiet()
    s.say('MEM')

    # One task back on its feet, and only that one: A and C stay stopped,
    # so between the echo of START B and the STOP that ends the window
    # neither of their letters may appear.
    mark = s.say('START B')
    after = emu.wait_for_output(b'START B', start=mark).end()
    emu.wait_for_output(re.compile(rb'B[^B]*B[^B]*B'), start=after)
    stop = s.say('STOP', quiet=False)
    alone = s.since(after, stop)
    if alone.count(b'B') < 3 or re.search(rb'[AC]', alone):
        raise AssertionError('START B: wanted B alone, got %r' % alone)

    s.say('HALT')
    s.wait(b'REX 703 DOWN')
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))


# dskt.asm's constants, and the platter's geometry.
DSKT_SECTOR = 4000
DSKT_ROUNDS = 250
DSKT_COPIES = 3
FIRST_JOB = 5                   # NARID: the first node the loader hands out
SECTOR_WORDS = 47
NOP = 0x0900                    # SRA 0
CLR = 0x0100


def fst_byte(i):
    return (i * 5 + 1) & 0xff


def check_platter(image, pristine):
    """The platter after the session, read the host's way."""
    with open(image, 'rb') as f:
        platter = fs703.Platter(bytearray(f.read()))
    bad = platter.check()
    if bad:
        raise AssertionError('the directory breaks its rules: ' + '; '.join(bad))
    for name, path in MODULES:
        with open(path, 'rb') as f:
            if platter.read(name, 'RX') != f.read():
                raise AssertionError('%s.RX is not the module that was put there' % name)
    def listing(*lines):
        return bytes(c | 0x80 for line in lines for c in line + b'\r\n')

    want = {'T2.TX': bytes(fst_byte(i) for i in range(10)), 'T4.TX': b'', 'MSG.TX': MSG, 'THIRD.TX': NOTE,
            'SQ.BA': SQ,
            'PROG.BA': listing(b'10 FOR I=1 TO 3', b'20 PRINT "SQ";I*I', b'30 NEXT I', b'50 GOTO 50'),
            'BG.BA': listing(b'10 FOR I=1 TO 3', b'20 PRINT "BG";I', b'30 NEXT I'),
            'LP.BA': listing(b'10 PRINT "LP"', b'20 GOTO 20')}
    made = {e.title: platter.read(e.name, e.type) for e in platter.files() if e.type in ('TX', 'BA')}
    if made != want:
        raise AssertionError('the text files and saved programs are %r' % made)
    # SQUISH ran after the last file was deleted, and every file made
    # since went into the hole at the end: the files are one run from
    # the first data sector, and what is free is one hole.
    kinds = [e.what for e in platter.entries()]
    if kinds != [fs703.FILE] * (len(kinds) - 1) + [fs703.HOLE]:
        raise AssertionError('not squished: %r' % platter.entries())
    before = {e.title for e in fs703.Platter(bytearray(pristine)).files()}
    if before & {'T2.TX', 'T4.TX', 'THIRD.TX', 'PROG.BA', 'BG.BA', 'LP.BA'} or 'PAD.TX' not in before:
        raise AssertionError('the platter as made holds %r' % sorted(before))


def disc_session(emu, image):
    """Three tasks at the disc at once, each writing and reading back a
    sector of its own 250 times over."""
    symbols = Symbols()
    symbols.load(REX_LST)
    if CONTROL == 'nolock':
        # KECLM is JSX IOTRY / SAZ / JMP KEWAIT: without the jump a task
        # that finds the disc taken carries on into the body
        emu.writew(symbols.value('KECLM') + 2, [NOP])
    s = Session(emu)
    emu.run()
    s.wait(b'REX 703 UP')
    # Typed ahead in one go, with the STAT behind them, so the three loads
    # follow each other as fast as the shell can make them and the STAT
    # finds the three on the ring together -- they are DS there until
    # they exit. The shell's queue holds the lot. The keyboard's echo of
    # the later commands lands inside the prompts in between, so there is
    # no prompt to count here; STAT's last line is the idle node's, and
    # nothing is typed after it to land in its output. Paced on the
    # machine and not on the wall clock: an exerciser lives a few dozen
    # machine seconds, which unthrottled is not long.
    s.wait(PROMPT)
    emu.wait_quiet()
    mark = len(emu.output)
    emu.key('LOAD DSKT\r' * DSKT_COPIES + 'STAT\r')
    idle_line = re.compile(rb'^0 ID RUN', re.M)
    emu.wait_for_output(idle_line, start=mark, timeout=120)
    together = len(re.findall(rb'^\d+ DS ', s.since(mark), re.M)) == DSKT_COPIES

    # ...and then none.
    for _ in range(400):
        emu.wait_quiet(timeout=120)
        mark = len(emu.output)
        emu.key('STAT\r')
        emu.wait_for_output(idle_line, start=mark, timeout=120)
        if not re.search(rb'^\d+ DS ', s.since(mark), re.M):
            break
    else:
        raise AssertionError('the disc exercisers never finished')
    emu.wait_quiet(timeout=120)
    s.at = len(emu.output)

    emu.halt()
    waits = emu.memw(symbols.value('IOWAITS'), 1)[0]
    emu.run()
    if waits == 0:
        raise AssertionError('no task ever found the disc taken: the lock was not exercised')

    with open(image, 'rb') as f:
        platter = f.read()
    for job in range(FIRST_JOB, FIRST_JOB + DSKT_COPIES):
        at = (DSKT_SECTOR + job) * SECTOR_WORDS * 2
        got = [int.from_bytes(platter[at + 2 * i:at + 2 * i + 2], 'big') for i in range(SECTOR_WORDS)]
        want = [(job * 256 + DSKT_ROUNDS + i) & 0xffff for i in range(SECTOR_WORDS)]
        if got != want:
            raise AssertionError('job %d: its sector holds %s' % (job, ' '.join('%04X' % w for w in got[:8])))

    # Last, since an exerciser that failed exits at once and the sectors
    # above say so more plainly than this would.
    if not together:
        raise AssertionError('STAT never showed the three exercisers on the ring together')

    emu.key('HALT\r')
    s.wait(b'REX 703 DOWN', timeout=120)
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))


def verdict(paper):
    """What the shell had to have printed, on lines of its own: the
    terminal's echo of a command begins with the prompt instead."""
    text = paper.replace(b'\r', b'')

    def line(pattern):
        return re.search(pattern, text, re.M) is not None

    def count(pattern):
        return len(re.findall(pattern, text, re.M))

    free = re.findall(rb'^FREE (\d+ LOW \d+ HIGH)$', text, re.M)
    hellos = [int(n) for n in re.findall(rb'HELLO FROM WORD (\d+)', text)]
    checks = [
        ('uptime', line(rb'UPTIME \d+ SEC')),
        ('A stopped', line(rb'^1 A  OFF')),
        ('shell running', line(rb'^4 SH RUN')),
        ('idle listed', line(rb'^0 ID RUN')),
        ('help', line(rb'^COMMANDS HELP STAT UPTIME')),
        ('echo', line(rb'^SHELL OUTPUT OK')),
        ('unknown command', line(rb'^WHAT')),
        ('directory: hello', line(rb'^HELLO\.RX \d+$')),
        ('directory: basic', line(rb'^BASIC\.RX \d+$')),
        ('directory: the free sectors', line(rb'^\d+ SECTORS FREE')),
        # RUN's hello has the printer to itself. LOAD's prints behind the
        # prompt, a character at a time into whatever else is printing, so
        # it is taken from STAT's ring and the pool instead (session).
        ('hello ran under RUN', line(rb'^HELLO FROM WORD \d')),
        ('hello loaded high every time', len(hellos) >= 5 and all(n >= POOLH for n in hellos)),
        ('no such file', line(rb'^NO SUCH FILE')),
        ('the file calls, twice', count(rb'^FST OK$') == 2),
        ('directory: the file left', line(rb'^T2\.TX 1$')),
        ('directory: the empty file', line(rb'^T4\.TX 0$')),
        ('a host file typed', line(rb'^FIRST LINE\nSECOND LINE$')),
        ('a file made here typed', count(rb'^LINE ONE\nLINE TWO$') == 2),
        ('an error in words', count(rb'^NO SUCH FILE$') == 2),
        ('a file dumped', line(rb'^0000 CCC9 CEC5 A0CF CEC5 8D8A CCC9 CEC5 A0D4$')),
        ('a command file', line(rb'^FROM A FILE$')),
        ('ten MEMs', len(free) >= 10),
        ('the pool whole after every load', len(set(free)) == 1),
        ('two runs', count(rb'^SQ9') == 2),
        ('two breaks', count(rb'BREAK AT 50') == 2),
        ('the program typed once and listed once', count(rb'20 PRINT "SQ"') == 2),
    ]
    return [name for name, ok in checks if not ok]


def make_platter(workdir, log):
    os.makedirs(os.path.join(workdir, 'disks'))
    image = os.path.join(workdir, 'disks', 'ray703-disc0.img')
    cmd = [sys.executable, MKDISC, image]
    if CONTROL == 'blank':
        pass
    elif CONTROL in ('', 'nohello', 'nolock', 'nosave', 'noguard'):
        files = []
        for name, data in (('PAD.TX', PAD), ('MSG.TX', MSG), ('CMDS.CM', CMDS), ('INIT.CM', INIT),
                           ('SQ.BA', SQ)):
            files.append((name, os.path.join(workdir, name)))
            with open(files[-1][1], 'wb') as f:
                f.write(data)
        for name, path in files[:1] + list(MODULES) + files[1:]:
            if (CONTROL, name) != ('nohello', 'HELLO'):
                high = (name, path) in MODULES and name not in LOW_ONLY
                cmd += ['--add-high' if high else '--add', name, path]
    else:
        sys.exit('REX_CONTROL=%s: not a control this harness has' % CONTROL)
    subprocess.run(cmd, check=True, stdout=log)
    log.flush()
    return image


def finish(emu):
    emu.kill()
    emu.wait_exited()
    status = emu.wait_exit()
    if status != 0:
        raise AssertionError('the emulator exited with status %d' % status)


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    for path in (ROM_FILE, REX_LST) + tuple(path for _, path in MODULES):
        if not os.path.isfile(path):
            print('error: %s is missing' % path, file=sys.stderr)
            print('build it with: make -C rex', file=sys.stderr)
            return 1

    emu = None
    os.makedirs(os.path.dirname(LOG_FILE), exist_ok=True)
    with open(LOG_FILE, 'wb') as log, tempfile.TemporaryDirectory(prefix='rex-test-') as workdir:
        try:
            image = make_platter(workdir, log)
            with open(image, 'rb') as f:
                pristine = f.read()
            with cwd(workdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE,
                                 '--fast-io', '--no-throttle'], log=log)
            session(emu)
            failed = verdict(bytes(emu.output))
            if failed:
                raise AssertionError('not on the paper: ' + ', '.join(failed))
            finish(emu)
            log.write(b'\n--- paper ---\n' + bytes(emu.output))
            check_platter(image, pristine)

            # Device time is machine time here: a transfer takes its half
            # revolution and a tick can land inside it.
            with cwd(workdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--no-throttle'], log=log)
            disc_session(emu, image)
            finish(emu)
            log.write(b'\n--- paper, the disc session ---\n' + bytes(emu.output))
        except Exception:
            log.write(b'\n--- harness failure ---\n')
            log.write(traceback.format_exc().encode())
            if emu is not None:
                log.write(b'\n--- paper ---\n' + bytes(emu.output))
                log.write(b'\n--- transcript ---\n')
                log.write('\n'.join(emu.transcript).encode('latin-1', 'replace') + b'\n')
                if emu.child is not None and emu.child.poll() is None:
                    emu.child.kill()
                    emu.child.wait()
                    emu.child.stdin.close()
                emu.close()
            print('FAIL: rex test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1

    print('PASS: rex test (%s)' % EMU_BIN)
    return 0


if __name__ == '__main__':
    sys.exit(main())
