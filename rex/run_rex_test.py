#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for REX, the 703 executive: boot it, load three
copies of blip.asm behind the prompt, each printing a mark of its own and
sleeping an interval of its own, and watch them run -- the scheduler, the
line clock, the context switch, the sleeps and the mailbox teletype
driver all at once -- load the
hello module off a scratch platter both ways (RUN, which waits for it, and
LOAD, which leaves it running behind the prompt) and see the pool's free
words come back, run two Tiny BASIC sessions with a silent loop broken by
Ctrl-C, then drive the rest of the commands and shut the machine down
with HALT, a task still loaded, which HALT has to leave stopped.

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
nothing here matches a line while a task behind the prompt could be
printing into it: the three blips are killed before anything is matched
exactly.

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
about the lock. The same machine, where a file takes minutes of its time
to print, then has DUMP print one behind the prompt while a program that
ignores Ctrl-C has the console and the key is struck: the break is the
console holder's, and DUMP has to reach the file's last line.

A third machine, also without --fast-io, is the swapper's. Four copies
of hog.asm each take 8,000 words of core, which is more than there is,
fill them with a pattern of their own and check it thirty times over
ten seconds; each leaves its verdict in a sector of its own. The first
two are stopped before the other two are loaded, so they are the ones
put out to make room, and STAT has to show them out and stopped; then
they are started again. For all four to finish, each has to go out to
the disc and come back to the same words, which another had in
between, and at the end as many have to have come back as went. Then four Tiny BASICs want the
core below word X'4000', which holds two: three run a program behind
the prompt that counts and then saves itself under another name, and
one is typed at in front of them. The saved listings are read back on
the host -- a program that came through being put out and brought back
with a word wrong would not list as it was written.

A fourth machine is given no core image at all, only a platter and the
disc controller's LOAD button (-s ray703-load): the boot sector has to
bring the kernel in from REX.SY and start it, and the kernel that comes
up that way has to list the directory and run a program off it.

A fifth machine's platter has two sectors free, a saved program and a
swap file. A SAVE under the program's name of more than the file's sector
can take has to be refused and leave the program as it was, and a COPY
onto the swap file's name has to be refused before it begins; the host
then has to find both files untouched and the two sectors free again,
and its own tools are held to the same rule about system files and to
ASCII in a name.

REX_CONTROL names a negative control, each of which must FAIL:
  blank    a platter with nothing on it (no catalogue lines, no modules)
  nohello  a platter without the hello module
  nolock   the kernel's wait for the disc patched out, so a task that
           finds it taken goes in anyway
  nosave   the directory never written back to the disc
  noguard  the kernel's refusal of a console read by a task that does
           not have the console patched out
  noswapin a task's blocks cut out of the free list where they were, but
           never read back into them
  nosys    a boot sector that was never told where the kernel is
  nopark   HALT's stopping of the loaded tasks patched out
  nolatch  a channel forgetting that it refused a character, so the
           close makes a file of what did go in
  sysopen  a file opened for writing under a system file's name
  noseg    the loader deaf to the code that says a word holds an address
           in another segment of the module, so it adds the wrong base
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
BOOT_BIN = os.path.join(BUILD, 'boot.bin')
HELLO_OBJ = os.path.join(BUILD, 'hello.obj')
BASIC_OBJ = os.path.join(BUILD, 'basic.obj')
DSKT_OBJ = os.path.join(BUILD, 'dskt.obj')
FST_OBJ = os.path.join(BUILD, 'fst.obj')
MODULES = (('HELLO', HELLO_OBJ), ('BASIC', BASIC_OBJ), ('DSKT', DSKT_OBJ), ('FST', FST_OBJ)) + tuple(
    (name, os.path.join(BUILD, name.lower() + '.obj'))
    for name in ('TYPE', 'COPY', 'DEL', 'REN', 'SQUISH', 'DUMP', 'PUT', 'NAP', 'HOG', 'BLIP', 'FAR'))
# Every module but BASIC goes on the platter marked as loadable above
# word X'4000', where the kernel then puts it.
LOW_ONLY = ('BASIC',)

# The pool and the node arena, as rex.asm lays them out.
POOLB, POOLH, POOLE = 0x1000, 0x4000, 0x7000
NARB, NARN, T_LEN, T_STA, T_NAM, SOFF, SFREE = 0x7e00, 24, 18, 0, 8, 2, 4


def node_of(emu, name):
    """The node of the one loaded task whose two-character name this is,
    as its address and its words. For a machine that is halted."""
    # the port gives 128 words to a request
    arena = []
    for start in range(NARB, NARB + NARN * T_LEN, 6 * T_LEN):
        arena += emu.memw(start, 6 * T_LEN)
    packed = ((ord(name[0]) | 0x80) << 8) | (ord(name[1]) | 0x80)
    nodes = [(NARB + i * T_LEN, arena[i * T_LEN:(i + 1) * T_LEN]) for i in range(NARN)
             if arena[i * T_LEN + T_STA] != SFREE and arena[i * T_LEN + T_NAM] == packed]
    if len(nodes) != 1:
        raise AssertionError('%d tasks named %s' % (len(nodes), name))
    return nodes[0]


def blocks_of(emu, name):
    """The pool blocks of the loaded task whose two-character name this
    is, as (first word, one past the last) of each payload. The machine
    is halted for the look and set running again."""
    emu.halt()
    try:
        node, _ = node_of(emu, name)
        mine, at = [], POOLB
        while at < POOLE:
            size, tag = emu.memw(at, 2)     # a block's header: its size, its owner
            if tag == 0x8000 | node:
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
# The marks the three blips are given to print: characters that nothing
# the shell prints or echoes while they run has in it.
MARKS = b'+=*'

# The swap session's: how big the swap file is, hog.asm's constants, and
# the three programs the background BASICs run -- each counts, for long
# enough that all three are wanted in core at once whatever order they
# were loaded in, and then saves itself as OUTn.BA.
SWAP_SECTORS = 1024
HOG_WORDS = 8000
HOG_COPIES = 4
HOG_SECTOR = 4200
HOG_GOOD = 0x600d
JOBS = 3
JOB_COUNT = 3000


def job_lines(n):
    return [b'10 REM JOB %d' % n, b'20 FOR I=1 TO %d' % JOB_COUNT, b'30 NEXT I', b'40 SAVE OUT%d' % n]
# What PUT is given, and what it puts: the keys as the teletype sends
# them, bit 7 set, a line feed after every carriage return.
NOTE_KEYS = 'LINE ONE\rLINE TWO\r'
NOTE = bytes(c | 0x80 for c in b'LINE ONE\r\nLINE TWO\r\n')
REX_LST = os.path.join(BUILD, 'rex.lst')
MKDISC = os.path.join(ROOT, 'tools', 'mkdisc703.py')
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(BUILD, 'rex_test.log')
CONTROL = os.environ.get('REX_CONTROL', '')

PROMPT = re.compile(rb'REX>')
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
        quiet, unless the caller says something is printing and it never
        will. Returns where the paper stood when the keys went in."""
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


def marks_ran(text):
    """Every blip has run several times and the printer has changed hands
    repeatedly. Thresholds rather than exact counts: they sleep for 30,
    45 and 60 ticks and drift through every phase against each other."""
    only = re.sub(rb'[^' + re.escape(MARKS) + rb']', b'', text)
    runs = len(re.sub(rb'(.)\1+', rb'\1', only))
    return all(only.count(bytes([mark])) >= 5 for mark in MARKS) and runs >= 9


def session(emu):
    s = Session(emu)
    if CONTROL == 'nosave':
        # FDSAVE is STX slot / LDW FK1 / JSX FDXFR: a CLR in place of the
        # call and the save answers "clean" having written nothing
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('FDSAVE') + 2, [CLR])
    if CONTROL == 'noguard':
        # Q.GET is MSK / STX / STW / CMW KCONSQ / SEQ / JMP .IN: without
        # the skip every caller is let in
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('Q.GET') + 4, [NOP])
    if CONTROL == 'noseg':
        # LDSEGR is JSX LDGETW / JSX LDSEGB / STW LDBASE: without the
        # store the base stays the segment's own
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('LDSEGR') + 2, [NOP])
    emu.run()
    s.wait(b'REX 703 UP')

    # INIT.CM is read before the keyboard is: its one command, echoed as
    # if typed, and what the command printed.
    s.wait(re.compile(rb'^INIT RAN\r', re.M))

    # Nothing is running yet, so these are quiet and exact.
    s.say('STAT')
    s.say('MEM')

    # Three blips behind the prompt, typed ahead in one go, and let them
    # run. They are the first three tasks loaded, which is what their job
    # numbers are and what their intervals come from. What is counted is
    # what follows the keyboard's echo of the last line, where the marks
    # are theirs alone.
    s.wait(PROMPT)
    emu.wait_quiet()
    mark = len(emu.output)
    lines = ['LOAD BLIP %c' % c for c in MARKS]
    emu.key(''.join(line + '\r' for line in lines))
    after = emu.wait_for_output(lines[-1].encode(), start=mark).end()
    deadline = time.monotonic() + 30
    while not marks_ran(s.since(after)):
        if time.monotonic() > deadline:
            raise AssertionError('the blips did not all run: %r' % s.since(after))
        time.sleep(0.05)

    # Then end them: with the printer quiet every command's output can be
    # matched exactly, and waiting for quiet becomes usable for pacing.
    emu.key(''.join('KILL %d\r' % job for job in range(FIRST_JOB, FIRST_JOB + len(MARKS))))
    stat_until_gone(emu, s, b'BL')
    s.at = len(emu.output)
    emu.key('STAT\r')
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
    # A module of two segments, which cannot share a page: each calls
    # into the other, and says whether it got there.
    s.say('RUN FAR')
    s.wait(re.compile(rb'FAR (OK|BAD \d+)'))
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
    s.basic('SAVE bg\r')              # the directory's names are upper case
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

    # HALT with a task loaded: it has to be left stopped, which is what
    # keeps anything of its out of the down-message.
    if CONTROL == 'nopark':
        # SHHD0 is LDX / LDW / CMW / SNE / JMP / LDW SHKOFF / STW *T.STA
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.halt()
        emu.writew(symbols.value('SHHD0') + 6, [NOP])
        emu.run()
    mark = s.say('LOAD NAP')
    emu.wait_for_output(b'THE CONSOLE WAS REFUSED', start=mark)
    emu.wait_quiet()
    s.say('HALT')
    s.wait(b'REX 703 DOWN')
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))
    _, node = node_of(emu, 'NA')
    if node[T_STA] != SOFF:
        raise AssertionError('HALT left NAP in state %d' % node[T_STA])


# dskt.asm's constants, and the platter's geometry.
DSKT_SECTOR = 4000
DSKT_ROUNDS = 250
DSKT_COPIES = 3
FIRST_JOB = 1                   # NARID: the first node the loader hands out
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
        # KECLM is JSX IOTRY / SAZ / JMP .WAIT: without the jump a task
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

    # A Ctrl-C is for the program that has the console. DUMP, behind the
    # prompt, has minutes of machine time's printing to do; NAP has the
    # console and never takes the break, so the flag stays up from the
    # first key to the second, which is the one that ends NAP.
    size = fs703.Platter(bytearray(platter)).find('HELLO', 'RX').size
    last = b'%04X ' % ((size - 1) // 16 * 16)
    mark = len(emu.output)
    emu.key('LOAD DUMP HELLO.RX\r')
    # its second line: the prompt prints into the first
    emu.wait_for_output(b'\n0010 ', start=mark, timeout=120)
    emu.key('NAP\r')
    deadline = time.time() + 120
    while True:
        emu.halt()
        held = emu.memw(symbols.value('CONBSY'), 1)[0]
        emu.run()
        if held:
            break
        if time.time() > deadline:
            raise AssertionError('NAP never had the console')
        time.sleep(0.05)
    emu.key('\x03')
    emu.wait_for_output(re.compile(re.escape(last) + rb'[0-9A-F]{4}'), start=mark, timeout=120)
    emu.wait_quiet(timeout=120)
    emu.key('\x03')
    emu.wait_for_output(b'KILLED', start=mark, timeout=120)
    emu.wait_quiet(timeout=120)
    s.at = len(emu.output)

    emu.key('HALT\r')
    s.wait(b'REX 703 DOWN', timeout=120)
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))


def stat_until_gone(emu, s, name, timeout=300):
    """STAT, over and over, until no task of that two-character name is
    listed; every listing there was, for the caller to look through."""
    seen = []
    row = re.compile(rb'^\d+ ' + name + rb' ', re.M)
    deadline = time.monotonic() + timeout
    while True:
        emu.wait_quiet(timeout=120)
        mark = len(emu.output)
        emu.key('STAT\r')
        emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark, timeout=120)
        seen.append(s.since(mark))
        if not row.search(seen[-1]):
            emu.wait_quiet(timeout=120)
            return seen
        if time.monotonic() > deadline:
            raise AssertionError('tasks named %s never finished: %r' % (name.decode(), seen[-1]))


def swap_session(emu, image):
    """More loaded than core holds, twice over."""
    symbols = Symbols()
    symbols.load(REX_LST)
    if CONTROL == 'noswapin':
        # SWIRL is LDW / CMW / SLS / JMP / JSX SWITEM / CLR / JSX SWXFER:
        # without the last a block comes back as whatever is lying there
        emu.writew(symbols.value('SWIRL') + 6, [NOP])
    s = Session(emu)
    emu.run()
    s.wait(b'REX 703 UP')
    s.wait(re.compile(rb'^INIT RAN\r', re.M))
    mark = s.say('MEM')
    s.wait(PROMPT)
    before = re.search(rb'^FREE (\d+ LOW \d+ HIGH)', s.since(mark), re.M).group(1)
    emu.wait_quiet()

    # Two hogs, stopped; then two more, which there is no room for until
    # the two that cannot run have gone out. All typed ahead, so that it
    # is the machine's time the hogs' ten seconds are measured against,
    # and the STAT at the end lists them as the fourth load leaves them.
    first, second = FIRST_JOB, FIRST_JOB + 1
    load = 'LOAD HOG %d\r' % HOG_WORDS
    mark = len(emu.output)
    emu.key(load * 2 + 'STOP %d\rSTOP %d\r' % (first, second) + load * 2 + 'STAT\r')
    emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark, timeout=300)
    listed = s.since(mark)
    for job in (first, second):
        if not re.search(rb'^%d HO OFF +OUT\r' % job, listed, re.M):
            raise AssertionError('hog %d was not stopped and out: %r' % (job, listed))
    if len(re.findall(rb'^\d+ HO ', listed, re.M)) != HOG_COPIES:
        raise AssertionError('not four hogs: %r' % listed)
    emu.wait_quiet(timeout=120)
    emu.key('START %d\rSTART %d\r' % (first, second))
    listings = stat_until_gone(emu, s, b'HO')
    for job in range(FIRST_JOB, FIRST_JOB + HOG_COPIES):
        with open(image, 'rb') as f:
            f.seek((HOG_SECTOR + job) * SECTOR_WORDS * 2)
            got = f.read(SECTOR_WORDS * 2)
        if got != HOG_GOOD.to_bytes(2, 'big') * SECTOR_WORDS:
            raise AssertionError('hog %d: its verdict is %s' % (job, got[:8].hex()))
    swaps = re.search(rb'^SWAPS (\d+) OUT (\d+) IN', listings[-1], re.M)
    if not swaps or int(swaps.group(1)) < HOG_COPIES or swaps.group(1) != swaps.group(2):
        raise AssertionError('the swap counts: %r' % (swaps and swaps.group(0)))
    s.at = len(emu.output)
    emu.key('MEM\r')
    emu.wait_for_output(b'FREE ' + before, start=s.at, timeout=60)

    # BASIC lives below word 4000, where two of them fit: three behind
    # the prompt and one in front.
    emu.wait_quiet()
    s.at = len(emu.output)
    emu.key(''.join('LOAD BASIC JOB%d\r' % n for n in range(1, JOBS + 1)) + 'BASIC\r')
    s.wait(b'TINY BASIC UNDER REX', timeout=300)
    s.basic('10 FOR I=1 TO 3\r')
    s.basic('20 PRINT "FG";I\r')
    s.basic('30 NEXT I\r')
    s.basic('RUN\r')
    s.wait(b'FG1\r\nFG2\r\nFG3\r\n', timeout=300)
    s.basic('SAVE FG\r')
    s.basic('BYE\r')
    s.wait(PROMPT, timeout=300)
    listings = stat_until_gone(emu, s, b'BA')
    swaps2 = re.search(rb'^SWAPS (\d+) OUT (\d+) IN', listings[-1], re.M)
    if not swaps2 or int(swaps2.group(1)) <= int(swaps.group(1)) or swaps2.group(1) != swaps2.group(2):
        raise AssertionError('four BASICs, and the swap counts: %r' % (swaps2 and swaps2.group(0)))
    s.at = len(emu.output)
    emu.key('MEM\r')
    emu.wait_for_output(b'FREE ' + before, start=s.at, timeout=60)

    emu.key('HALT\r')
    s.wait(b'REX 703 DOWN', timeout=120)
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))

    with open(image, 'rb') as f:
        platter = fs703.Platter(bytearray(f.read()))
    bad = platter.check()
    if bad:
        raise AssertionError('the directory breaks its rules: ' + '; '.join(bad))

    def listing(lines):
        return bytes(c | 0x80 for line in lines for c in line + b'\r\n')

    for n in range(1, JOBS + 1):
        if platter.read('OUT%d' % n, 'BA') != listing(job_lines(n)):
            raise AssertionError('OUT%d.BA is not the program job %d ran' % (n, n))
    if platter.read('FG', 'BA') != listing([b'10 FOR I=1 TO 3', b'20 PRINT "FG";I', b'30 NEXT I']):
        raise AssertionError('FG.BA is not the program that was typed')


def boot_session(emu):
    """The kernel, read in by the boot sector."""
    s = Session(emu)
    emu.run()
    s.wait(b'REX 703 UP')
    mark = s.say('DIR')
    s.wait(b'SECTORS FREE')
    if not re.search(rb'^REX\.SY \d+\r', s.since(mark), re.M):
        raise AssertionError('no REX.SY in the directory: %r' % s.since(mark))
    s.say('HELLO')
    s.wait(re.compile(rb'^HELLO FROM WORD \d+\r', re.M))
    s.say('HALT')
    s.wait(b'REX 703 DOWN')
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))


KEEP = b'10 PRINT "KEEP"\n'
FULL_FREE = 2                   # sectors: a new file is given half


def fill_platter(image):
    """A saved program, and then all but FULL_FREE sectors taken."""
    with open(image, 'rb') as f:
        platter = fs703.Platter(bytearray(f.read()))
    platter.add('KEEP', 'BA', KEEP)
    platter.add('FILL', 'TX', bytes((platter.free_sectors() - FULL_FREE) * fs703.SECTOR_BYTES))
    with open(image, 'wb') as f:
        f.write(platter.image)


def full_session(emu, image):
    """A file that does not fit, and a system file's name."""
    symbols = Symbols()
    symbols.load(REX_LST)
    if CONTROL == 'nolatch':
        # FBPERR is LDX FCH / STW *C.ERR
        emu.writew(symbols.value('FBPERR') + 1, [NOP])
    if CONTROL == 'sysopen':
        # FBOC1 is CAX / LDW *DE.FLG / AND FKSYS / SAZ / JMP FBOPR
        emu.writew(symbols.value('FBOC1') + 4, [NOP])
    with open(image, 'rb') as f:
        before = fs703.Platter(bytearray(f.read()))
    s = Session(emu)
    emu.run()
    s.wait(b'REX 703 UP')
    s.say('BASIC')
    for n in (10, 20, 30):          # more than a sector of listing
        s.basic('%d REM %s\r' % (n, 'X' * 40))
    mark = len(emu.output)
    s.basic('SAVE KEEP\r')
    s.basic('BYE\r')
    if not re.search(rb'^HOW\?', s.since(mark), re.M):
        raise AssertionError('a SAVE with no room for it: %r' % s.since(mark))
    mark = s.say('COPY MSG SWAP.SY')
    emu.wait_for_output(b'SYSTEM FILE', start=mark)
    s.say('HALT')
    s.wait(b'REX 703 DOWN')
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))

    with open(image, 'rb') as f:
        after = fs703.Platter(bytearray(f.read()))
    bad = after.check()
    if bad:
        raise AssertionError('the directory breaks its rules: ' + '; '.join(bad))
    if after.read('KEEP', 'BA') != KEEP:
        raise AssertionError('KEEP.BA is %r' % after.read('KEEP', 'BA'))

    def shape(platter):
        return [(e.what, e.title, e.start, e.count, e.flags) for e in platter.entries()]
    if shape(after) != shape(before) or after.free_sectors() != FULL_FREE:
        raise AssertionError('the directory is not as it was: %r' % after.entries())

    # The host's tools, on the same platter.
    for what, attempt in (('deleted a system file', lambda: after.delete('SWAP', 'SY')),
                          ('put a file in a system file\'s place', lambda: after.add('SWAP', 'SY', b'X')),
                          ('took a name that is not ASCII', lambda: fs703.parse_name('\u00e9.TX'))):
        try:
            attempt()
        except fs703.FsError:
            continue
        raise AssertionError('fs703 ' + what)


def verdict(paper, programs):
    """What the shell had to have printed, on lines of its own: the
    terminal's echo of a command begins with the prompt instead. The
    programs are the platter's, by name, in the directory's order."""
    text = paper.replace(b'\r', b'')

    def line(pattern):
        return re.search(pattern, text, re.M) is not None

    def count(pattern):
        return len(re.findall(pattern, text, re.M))

    free = re.findall(rb'^FREE (\d+ LOW \d+ HIGH)$', text, re.M)
    # hello's line whole on a line of its own: the one LOAD started prints
    # into whatever else is printing and may not be
    hellos = [int(n) for n in re.findall(rb'^HELLO FROM WORD (\d+)$', text, re.M)]
    checks = [
        ('uptime', line(rb'UPTIME \d+ SEC')),
        ('shell running', line(rb'^0 SH RUN')),
        ('idle listed', line(rb'^0 ID RUN')),
        ('help: what is built in', line(rb'^BUILT IN: HELP STAT UPTIME')),
        # every one of them, eight to a line
        ('help: what is on the disc', bool(programs) and line(b'^' + b'\n'.join(
            b''.join(b' ' + name.encode() for name in programs[i:i + 8])
            for i in range(0, len(programs), 8)) + b'$')),
        ('echo', line(rb'^SHELL OUTPUT OK')),
        ('unknown command', line(rb'^WHAT')),
        ('directory: hello', line(rb'^HELLO\.RX \d+$')),
        ('directory: basic', line(rb'^BASIC\.RX \d+$')),
        ('directory: the free sectors', line(rb'^\d+ SECTORS FREE')),
        # RUN's hello has the printer to itself. LOAD's prints behind the
        # prompt, a character at a time into whatever else is printing, so
        # it is taken from STAT's ring and the pool instead (session).
        ('hello ran under RUN', line(rb'^HELLO FROM WORD \d')),
        ('hello loaded high every time', len(hellos) >= 4 and all(n >= POOLH for n in hellos)),
        ('a module of two segments', line(rb'^FAR OK$')),
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


def make_platter(workdir, log, swap=False, boot=False):
    os.makedirs(os.path.join(workdir, 'disks'))
    image = os.path.join(workdir, 'disks', 'ray703-disc0.img')
    cmd = [sys.executable, MKDISC, image]
    if boot:
        cmd += ['--boot', BOOT_BIN] + ([] if CONTROL == 'nosys' else ['--sys', ROM_FILE])
    if swap:
        cmd += ['--swap', str(SWAP_SECTORS)]
    if CONTROL == 'blank':
        pass
    elif CONTROL in ('', 'nohello', 'nolock', 'nosave', 'noguard', 'noswapin', 'nosys', 'nopark',
                     'nolatch', 'sysopen', 'noseg'):
        files = []
        jobs = [('JOB%d.BA' % n, b'\n'.join(job_lines(n)) + b'\n') for n in range(1, JOBS + 1)] if swap else []
        for name, data in [('PAD.TX', PAD), ('MSG.TX', MSG), ('CMDS.CM', CMDS), ('INIT.CM', INIT),
                           ('SQ.BA', SQ)] + jobs:
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
    for path in (ROM_FILE, REX_LST, BOOT_BIN) + tuple(path for _, path in MODULES):
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
            programs = [e.name for e in fs703.Platter(bytearray(pristine)).files() if e.type == 'RX']
            failed = verdict(bytes(emu.output), programs)
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

            # A platter of its own, with a swap file: the other sessions
            # run without one, where an allocation that finds no room fails.
            swapdir = os.path.join(workdir, 'swap')
            os.makedirs(swapdir)
            image = make_platter(swapdir, log, swap=True)
            with cwd(swapdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--no-throttle'], log=log)
            swap_session(emu, image)
            finish(emu)
            log.write(b'\n--- paper, the swap session ---\n' + bytes(emu.output))

            # And a platter that is the whole system, with no core image:
            # the LOAD button reads its sector 0 and starts that.
            bootdir = os.path.join(workdir, 'boot')
            os.makedirs(bootdir)
            image = make_platter(bootdir, log, swap=True, boot=True)
            emu = Emu.spawn([EMU_BIN, '-s', 'ray703-load', '-r', image, '--fast-io', '--no-throttle'], log=log)
            boot_session(emu)
            finish(emu)
            log.write(b'\n--- paper, the boot session ---\n' + bytes(emu.output))

            # And one with next to no room on it.
            fulldir = os.path.join(workdir, 'full')
            os.makedirs(fulldir)
            image = make_platter(fulldir, log, swap=True)
            fill_platter(image)
            with cwd(fulldir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE,
                                 '--fast-io', '--no-throttle'], log=log)
            full_session(emu, image)
            finish(emu)
            log.write(b'\n--- paper, the full platter ---\n' + bytes(emu.output))
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
