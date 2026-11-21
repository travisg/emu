#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""End-to-end test for REX, the 703 executive: boot it -- the console's
shell is SHELL.RX off the platter, which has to be above word X'4000'
and to refuse BYE, the console never being without one -- start three
copies of blip.asm behind the prompt, each printing a mark of its own and
sleeping an interval of its own, and watch them run -- the scheduler, the
clock's tick, the context switch, the sleeps and the mailbox teletype
driver all at once -- load the
hello module off a scratch platter both ways (RUN, which waits for it, and
START, which leaves it running behind the prompt) and see the pool's free
words come back -- the first RUN costing the disc exactly two transfers,
the first record and the rest of the module -- run two Tiny BASIC
sessions with a silent loop broken by
Ctrl-C, then drive the rest of the commands and shut the machine down
with HALT, a task still loaded, which HALT has to leave stopped.

Job control: LOAD leaves a program stopped and says its number, and
nothing of it runs until START; ATTACH gives a task the console and
Ctrl-Z takes it back, the task stopped -- a BLIP, a BASIC whose loop then
runs behind the prompt, deaf to the Ctrl-C typed there until it is
attached again, and a NAP, which reads the console and so waits for it
behind the prompt (STAT's TTY) without taking the shell's keys.

The emulator is driven over its debug port (tools/emudbg.py), started
halted so no output is missed: keys go in as `key` commands, the paper is
the port's copy of the serial output, and the shutdown is the port's own
`! stopped reason=hlt`. The platter is made fresh in a scratch directory
and the emulator run from there, since it mounts disks/ray703-disc0.img
relative to where it runs -- the repo root's is the user's own.

--fast-io: the teletype's pacing is not what is under test, and the
scheduling slices stay machine time regardless, since the time-of-day
clock ignores the flag. TIME is asked at the start and again near the end:
two readings of the clock, the second later.

Every wait raises on its timeout and the handler at the bottom turns that
into a FAIL with the transcript in the log. The walk over the output keeps
a cursor: the same prompt comes round again, and a wait without one is
satisfied by the first. The printer is shared character by character, so
nothing here matches a line while a task behind the prompt could be
printing into it: the three blips are killed before anything is matched
exactly.

The filesystem is held to the host's reading of it (fs703.py): the
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
about the lock -- and none of them may ever be seen asleep, which is
what a claim that polled the lock would be. Across HELP, which the
shell then prints there at ten characters a second, three ticks in four
have to find the machine idle, which a shell spinning on the printer
would not. The same machine, where a file takes minutes of its time
to print, then has DUMP print one behind the prompt while a program that
ignores Ctrl-C has the console and the key is struck: the break is the
console holder's, and DUMP has to reach the file's last line.

A third machine, also without --fast-io, is the swapper's. Four copies
of hog.asm each take 8,000 words of core, which is more than there is,
fill them with a pattern of their own and check it twenty times over
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
with a word wrong would not list as it was written. Last, three BASICs
that never end take turns in the room for two: the one that is out has
to wait for one in core to have had its time without having its list
read off the disc again on every pass of the swapper. And a hog stopped
with all but a little of that core leaves BASIC's module no room, so
the load waits for core inside K.LOAD, holding the disc: it has to come
back to the shell with BASIC ready and the pool whole once the hog is
put out.

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

A sixth machine has teletypes 1, 2 and 3 on the Teletype Multiplexer's
channels 0-2, each on a localhost socket (--serial tcp:0), which the
harness types at and reads as it does the console. The first key at
teletype 1 has to bring up a shell of that terminal's own, loaded above
word X'4000' so that the core below is left to BASIC, and the two
shells run side by side: a BLIP behind each prompt prints on its own
terminal's paper and never on the other's, STAT on the console shows the
other terminal's shell and its task, a second Ctrl-C there ends only the
program in front of it, a Ctrl-C while it prints -- the set's BREAK --
stops a dump, teletype 3 gets a shell of its own, and teletype 1's BYE
leaves the pool as it was before the terminal had a shell -- after which
a key brings up another. Then, with
the console's shell waiting on a program it ran, teletype 2's first key
has to bring up a shell there too; and with more hogs loaded than core
holds, a terminal's shell that is waiting for a key has to be put out
and come back in for one.

A seventh machine has a second platter on unit 1, made with nothing but
a directory (fs703.py --dir), which makes it the data unit. A file
put with no unit named lands there, a program is still run from unit 0,
and 0: and 1: in front of a name reach either platter: COPY goes both
ways, a REN from one platter to the other is refused, DIR lists the data
unit and DIR 0: the other, BASIC saves to unit 1 and runs a program from
it, and SQUISH tidies unit 1. The host then reads both platters back:
every file on the one it was meant for, and unit 0 as it was made but for
the one file copied onto it.

An eighth machine's platter has no SHELL.RX: the console has none to
load, and has to say so before the machine stops.

REX_CONTROL names a negative control, each of which must FAIL:
  blank    a platter with nothing on it (no catalogue lines, no modules)
  nohello  a platter without the hello module
  nolock   the kernel's wait for the disc patched out, so a task that
           finds it taken goes in anyway
  nosave   the directory never written back to the disc
  noguard  the kernel's test of a console read by a task that does not
           have the console patched out, so it reads the shell's keys
  noswapin a task's blocks cut out of the free list where they were, but
           never read back into them
  nosys    a boot sector that was never told where the kernel is
  nopark   HALT's stopping of the loaded tasks patched out, so a NAP
           waiting for its console is left waiting
  nolatch  a channel forgetting that it refused a character, so the
           close makes a file of what did go in
  sysopen  a file opened for writing under a system file's name
  noseg    the loader deaf to the code that says a word holds an address
           in another segment of the module, so it adds the wrong base
  onlycon  the terminals' service routine never asking the multiplexer,
           as if the console were the only one
  onetty   the printer started on any task's character, whichever
           terminal the task is on
  nogetty  a key at a terminal with no shell asking for none
  nounit   the disc driver deaf to the unit, so every transfer is unit 0's
  dirunit  K.DIRNT reading whichever platter's directory is in core
  nodata   no data unit but unit 0, whatever unit 1 has
  fcwho    a channel's unit taken as 0 when the directory in core is asked
           who is writing, so a write on unit 1 looks abandoned
  nohold   TIME never reading the clock's word 3, so the hold it starts
           is never ended: the clock stands still, and the tick with it
  nobreak  a multiplexer set's BREAK, its Ctrl-C while it prints, never
           passed on, so a dump on teletype 1 cannot be stopped
  noloadstop  LOAD giving the task it loads a running state, so it runs at
           once and is never numbered
  nodetach a Ctrl-Z queued like any other key, so it never stops the
           console's program
  nobrkq   K.BRKQ answering a task without the console, so a BASIC behind
           the prompt takes the Ctrl-C typed there
  onesec   the loader's reader asking for a block of no sectors, so a
           program comes in a sector at a time
  nowake   the disc given up without waking whoever waits for it
  shlow    SHELL.RX put on the platter as a module that has to load
           below word X'4000'
  conbye   the console's shell taking BYE as a terminal's does
  nobootmsg  the boot's message that there is no shell never printed
  nobackoff  a task that cannot come back until the one in its way has had
           its time tried for on every pass of the swapper meanwhile
  nokehlk  KENTER not putting its link back after the body, so a load
           that waited for core returns into the loader, not the shell
  spin     the shell's wait for a character to print never marking it
           waiting, so it spins through every character it prints
  nokillwake  KILLT handing nothing back, so a KILL from the console of
           teletype 1's foreground program leaves that terminal's shell
           waiting for good
"""

import contextlib
import os
import re
import socket
import threading
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
    for name in ('TYPE', 'COPY', 'DEL', 'REN', 'SQUISH', 'DUMP', 'PUT', 'DIR', 'NAP', 'HOG', 'BLIP', 'FAR', 'PLAT',
                 'SHELL'))
# Every module but BASIC goes on the platter marked as loadable above
# word X'4000', where the kernel then puts it.
LOW_ONLY = ('BASIC',)

# The pool and the node arena, as rex.asm lays them out.
POOLB, POOLH, POOLE = 0x1000, 0x4000, 0x7000
NARB, NARN, T_LEN, T_STA, T_NAM, SSLP, SOFF, SFREE, SCON = 0x7e00, 24, 19, 0, 8, 1, 2, 4, 5
T_ID, T_TTY = 10, 18


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


def jobs_of(emu, name):
    """The loaded tasks of this two-character name, as their job numbers
    and their terminals' blocks (T.ID, T.TTY). For a machine that is
    running: it is halted to look."""
    emu.halt()
    arena = []
    for start in range(NARB, NARB + NARN * T_LEN, 6 * T_LEN):
        arena += emu.memw(start, 6 * T_LEN)
    emu.run()
    packed = ((ord(name[0]) | 0x80) << 8) | (ord(name[1]) | 0x80)
    return [(arena[i * T_LEN + T_ID], arena[i * T_LEN + T_TTY]) for i in range(NARN)
            if arena[i * T_LEN + T_STA] != SFREE and arena[i * T_LEN + T_NAM] == packed]


def blocks_of(emu, name, node=None):
    """The pool blocks of the loaded task whose two-character name this
    is -- or of the node named, which need not be the arena's -- as (first
    word, one past the last) of each payload. The machine is halted for
    the look and set running again."""
    emu.halt()
    try:
        if node is None:
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
# And a program that never ends, for as many BASICs as will not all fit,
# and how long their changing places is watched.
LOOP = b'10 GOTO 10\n'
LOOPERS = 3
LOOP_SECS = 10
# And a hog that leaves too little below word X'4000' for BASIC's module,
# so a load has to wait for core.
LOADWAIT_HOG = 11000


def job_lines(n):
    return [b'10 REM JOB %d' % n, b'20 FOR I=1 TO %d' % JOB_COUNT, b'30 NEXT I', b'40 SAVE OUT%d' % n]
# What PUT is given, and what it puts: the keys as the teletype sends
# them, bit 7 set, a line feed after every carriage return.
NOTE_KEYS = 'LINE ONE\rLINE TWO\r'
NOTE = bytes(c | 0x80 for c in b'LINE ONE\r\nLINE TWO\r\n')
REX_LST = os.path.join(BUILD, 'rex.lst')
SHELL_LST = os.path.join(BUILD, 'shell.lst')
MKDISC = os.path.join(ROOT, 'tools', 'mkdisc703.py')
FS703 = os.path.join(HERE, 'fs703.py')
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


class Terminal:
    """Another teletype, on the localhost socket --serial gave it: what it
    prints, and keys typed at it, with the same waits the debug port's
    console has, so that a Session can walk its paper too."""

    def __init__(self, port, timeout=60):
        self.sock = socket.create_connection(('127.0.0.1', port))
        self.timeout = timeout
        self.output = bytearray()
        self._changed = threading.Condition()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        while True:
            try:
                data = self.sock.recv(4096)
            except OSError:
                data = b''
            with self._changed:
                if not data:
                    return
                self.output += data
                self._changed.notify_all()

    def key(self, text):
        self.sock.sendall(text.encode('latin-1'))

    def wait_for_output(self, pattern, timeout=None, start=0):
        if isinstance(pattern, bytes):
            pattern = re.compile(re.escape(pattern))
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        with self._changed:
            while True:
                m = pattern.search(bytes(self.output), start)
                if m:
                    return m
                left = deadline - time.monotonic()
                if left <= 0:
                    raise TimeoutError('no %r on the terminal (have %r)'
                                       % (pattern.pattern, bytes(self.output[-200:])))
                self._changed.wait(left)

    def wait_quiet(self, idle=0.3, timeout=None):
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        with self._changed:
            while True:
                n = len(self.output)
                if not self._changed.wait(idle) and len(self.output) == n:
                    return
                if time.monotonic() > deadline:
                    raise TimeoutError('the terminal never went quiet')

    def close(self):
        self.sock.close()


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
        # Q.GET's loop asks CMW *T.KAR / SEQ / JMP .R: without the skip
        # every caller is let in
        emu.writew(listing_word('CMW     *T.KAR', 'Q.GET           MSK') + 1, [NOP])
    if CONTROL == 'nodetach':
        # SVPOST's CLB X'9A' / SNE / JMP .DET
        emu.writew(listing_word("CLB     X'9A'", 'SVPOST          SUBR') + 2, [NOP])
    if CONTROL == 'onesec':
        # the reader's LDW LRNSEC / .TRY STW LRCAP: a CLR, and the block it
        # asks for is of no sectors
        emu.writew(listing_word('LDW     LRNSEC          ; for all the rest', 'LDREC           SUBR'), [CLR])
    if CONTROL == 'nobrkq':
        # BRKQ is MSK / STX / LDX CURT / LDW *T.CON / SAZ / JMP .Q
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('BRKQ') + 4, [NOP])
    if CONTROL == 'noseg':
        # LDSEGR is JSX LDGETW / JSX LDSEGB / STW LDBASE: without the
        # store the base stays the segment's own
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('LDSEGR') + 2, [NOP])
    symbols = Symbols()
    symbols.load(REX_LST)
    held_at = symbols.value('TMT') + symbols.value('TM.BSY')
    emu.run()
    s.wait(b'REX 703 UP')
    if CONTROL == 'noloadstop':
        patch_shell(emu, 'LDW     =SOFF', 'SHLOAD          CLR', CLR)
    if CONTROL == 'nohold':
        # TIME's last read is the one that ends the hold
        patch_shell(emu, 'DIN     11,3', 'SHTIME          MSK', NOP)
    if CONTROL == 'conbye':
        # SHBYE's JNZ .B, SAZ / JMP .B: without the skip the console's
        # shell takes BYE too
        patch_shell(emu, 'JNZ     .B', 'SHBYE           JSX', NOP)

    # The console's shell is SHELL.RX, loaded off the platter into the
    # console's node, above the fence like every other terminal's
    low = [b for b in blocks_of(emu, None, symbols.value('SHTCB')) if b[0] < POOLH]
    if low:
        raise AssertionError("the console's shell has blocks below word X'4000': %r" % low)

    # INIT.CM is read before the keyboard is: its one command, echoed as
    # if typed, and what the command printed.
    s.wait(re.compile(rb'^INIT RAN\r', re.M))

    # BYE is a terminal's: the console is never left without a shell
    s.say('BYE')
    s.wait(b"BYE IS A TERMINAL'S\r\n")

    # Nothing is running yet, so these are quiet and exact.
    s.say('STAT')
    s.say('MEM')
    s.say('TIME')

    # Three blips behind the prompt, typed ahead in one go, and let them
    # run. They are the first three tasks loaded, which is what their job
    # numbers are and what their intervals come from. What is counted is
    # what follows the keyboard's echo of the last line, where the marks
    # are theirs alone.
    s.wait(PROMPT)
    emu.wait_quiet()
    mark = len(emu.output)
    lines = ['START BLIP %c' % c for c in MARKS]
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

    # LOAD leaves a program stopped and says its number, and nothing of it
    # prints until START lets it run. ATTACH gives it the console and the
    # shell waits; Ctrl-Z stops it again and gives the console back.
    s.say('LOAD BLIP +')
    job = s.wait(re.compile(rb'^JOB (\d+)\r', re.M)).group(1).decode()
    s.say('STAT')
    s.wait(re.compile(rb'^' + job.encode() + rb' BL OFF', re.M))
    after = s.say('UPTIME')
    emu.wait_for_output(PROMPT, start=after)
    emu.wait_quiet(idle=1.0)
    if b'+' in s.since(after):
        raise AssertionError('a LOADed blip printed before START: %r' % s.since(after))
    s.say('START ' + job)
    s.wait(re.compile(rb'\+[^+]*\+'))
    emu.key('ATTACH %s\r' % job)
    wait_held(emu, held_at, 'the attached blip never had the console')
    s.wait(re.compile(rb'\+[^+]*\+'))
    emu.key('\x1a')
    s.wait(b'JOB %s STOPPED\r\n' % job.encode())
    mark = s.say('STAT')
    s.wait(re.compile(rb'^' + job.encode() + rb' BL OFF', re.M))
    s.say('KILL ' + job)
    s.say('HELP')
    s.say('ECHO SHELL OUTPUT OK')
    s.say('FROB')

    # The hello module, both ways. RUN waits for it, so its line is clean
    # and the free words are back by the next prompt; START leaves it to run
    # behind the prompt, where a STAT typed at once finds it on the ring,
    # and STAT is repeated until it has gone before the pool is counted
    # again. A name not in the catalogue is refused.
    s.say('MEM')
    s.say('DIR')
    # A program is two transfers: its first record, and the rest of it in
    # one go into a block the loader is lent for the purpose. The
    # directory is in core already, DIR having read it.
    xfers = symbols.value('DXFERS')
    emu.wait_for_output(PROMPT, start=s.at)
    emu.wait_quiet()
    emu.halt()
    before = emu.memw(xfers, 1)[0]
    emu.run()
    s.say('RUN HELLO')
    s.wait(b'HELLO FROM WORD')
    emu.wait_for_output(PROMPT, start=s.at)
    emu.wait_quiet()
    emu.halt()
    spent = emu.memw(xfers, 1)[0] - before
    emu.run()
    if spent != 2:
        raise AssertionError('RUN HELLO took %d transfers' % spent)
    s.say('MEM')
    # A module of two segments, which cannot share a page: each calls
    # into the other, and says whether it got there.
    s.say('RUN FAR')
    s.wait(re.compile(rb'FAR (OK|BAD \d+)'))
    s.say('MEM')
    s.say('START HELLO')
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
    emu.key(NOTE_KEYS + '\x1b')        # the shell's queue holds it for PUT
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

    # Jobs. NAP reads the console once and then sleeps for ever. Started
    # behind the prompt it has no console, so its read waits for one --
    # STAT says TTY -- and the keys typed after it are still the shell's.
    # STOP, START, ATTACH and KILL take its number, as STAT gives it.
    nap_from = s.say('START NAP')
    mark = s.say('STAT')
    job = emu.wait_for_output(re.compile(rb'^(\d+) NA TTY', re.M), start=mark).group(1).decode()
    s.say('ECHO STILL THE SHELL')
    s.wait(re.compile(rb'^STILL THE SHELL\r', re.M))
    held = blocks_of(emu, 'NA')
    if not held or any(first < POOLH for first, _ in held):
        raise AssertionError('a module marked for it was not loaded high: %r' % held)
    s.say('STOP ' + job)
    mark = s.say('STAT')
    emu.wait_for_output(re.compile(rb'^' + job.encode() + rb' NA OFF', re.M), start=mark)
    s.say('START ' + job)
    mark = s.say('STAT')
    emu.wait_for_output(re.compile(rb'^' + job.encode() + rb' NA TTY', re.M), start=mark)
    emu.wait_quiet()
    if b'NAP:' in s.since(nap_from):
        raise AssertionError('NAP read the console behind the prompt: %r' % s.since(nap_from))
    s.say('ATTACH ' + job)
    wait_held(emu, held_at, 'the attached NAP never had the console')
    emu.key('X')
    s.wait(b'NAP: READ THE CONSOLE\r\n')
    emu.wait_quiet()
    emu.key('\x1a')
    s.wait(b'JOB %s STOPPED\r\n' % job.encode())
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

    # A loop that prints, stopped with Ctrl-Z: it keeps its place and the
    # shell has the console. START runs it behind the prompt, where the
    # Ctrl-C typed at the shell is not its to take; ATTACH gives it back
    # the console, and the next Ctrl-C breaks it.
    s.basic('NEW\r')
    s.basic('10 FOR I=1 TO 300\r')
    s.basic('20 NEXT I\r')
    s.basic('30 PRINT "ZZ"\r')
    s.basic('40 GOTO 10\r')
    s.basic('RUN\r')
    s.wait(b'ZZ\r\nZZ\r\n')
    emu.key('\x1a')
    job = s.wait(re.compile(rb'JOB (\d+) STOPPED\r\n')).group(1).decode()
    mark = s.say('STAT')
    s.wait(re.compile(rb'^' + job.encode() + rb' BA OFF', re.M))
    s.say('START ' + job)
    s.wait(b'ZZ\r\nZZ\r\n')
    mark = len(emu.output)
    emu.key('\x03')
    time.sleep(0.5)
    emu.wait_for_output(b'ZZ\r\nZZ\r\n', start=len(emu.output))
    if b'BREAK' in s.since(mark):
        raise AssertionError('a BASIC behind the prompt took the console\'s break: %r' % s.since(mark))
    emu.key('ATTACH %s\r' % job)
    wait_held(emu, held_at, 'the attached BASIC never had the console')
    s.at = len(emu.output)
    emu.key('\x03')
    s.wait(re.compile(rb'BREAK AT \d0'))
    mark = len(emu.output)
    s.basic('OLD NOPE\r')
    s.basic('BYE\r')
    if not re.search(rb'^HOW\?', s.since(mark), re.M):
        raise AssertionError('OLD of a file that is not there: %r' % s.since(mark))

    # A program named on the command line is loaded and run. In the
    # foreground the session stays at READY afterwards -- this one is a
    # host's file, with bare newlines -- and behind the prompt (START) the
    # task prints what the program prints and is gone when the program
    # ends.
    s.say('BASIC SQ')
    s.wait(b'FROM A HOST FILE\r\n1\r\n4\r\n9\r\n')
    s.basic('BYE\r')
    mark = s.say('START BASIC BG')
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
    mark = s.say('START BASIC LP')
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

    # A BASIC started behind the prompt with no program is a session
    # waiting for its console: up to its READY, then TTY. ATTACH is what
    # lets it read, and its BYE gives the memory back.
    mark = s.say('START BASIC')
    emu.wait_for_output(READY, start=mark)
    emu.wait_quiet()
    mark = s.say('STAT')
    job = emu.wait_for_output(re.compile(rb'^(\d+) BA TTY', re.M), start=mark).group(1).decode()
    s.say('ATTACH ' + job)
    wait_held(emu, held_at, 'the attached BASIC never had the console')
    emu.key('BYE\r')
    s.say('MEM')

    # HALT with a task loaded: it has to be left stopped, which is what
    # keeps anything of its out of the down-message.
    if CONTROL == 'nopark':
        # SHHD0 is LDX / LDW / CMW / SNE / JMP / LDW =SOFF / STW *T.STA
        patch_shell(emu, 'STW     *T.STA', 'SHHD0           LDX', NOP)
    mark = s.say('START NAP')
    s.say('STAT')
    s.wait(re.compile(rb'^\d+ NA TTY', re.M))
    s.say('TIME')
    s.say('HALT')
    s.wait(b'REX 703 DOWN')
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))
    _, node = node_of(emu, 'NA')
    if node[T_STA] != SOFF:
        raise AssertionError('HALT left NAP in state %d' % node[T_STA])


def wait_held(emu, held_at, why, timeout=120):
    """Until a terminal's TM.BSY, at held_at, says a task has its console:
    a grant made, so what is typed next is the task's."""
    deadline = time.time() + timeout
    while True:
        emu.halt()
        held = emu.memw(held_at, 1)[0]
        emu.run()
        if held:
            return
        if time.time() > deadline:
            raise AssertionError(why)
        time.sleep(0.05)


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
    if CONTROL == 'nowake':
        # IOREL's JNZ .W: SAZ / JMP .W -- without the jump nobody waiting
        # for the disc is ever woken
        emu.writew(listing_word('JNZ     .W', 'IOREL           SUBR') + 1, [NOP])
    s = Session(emu)
    emu.run()
    s.wait(b'REX 703 UP')
    if CONTROL == 'spin':
        # SHPUTC's wait marks the shell waiting before it hands the
        # processor on: without the store it is picked again at once
        patch_shell(emu, 'STW     *T.STA', 'SHPUTC          SUBR', NOP)
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
    emu.key('START DSKT\r' * DSKT_COPIES + 'STAT\r')
    idle_line = re.compile(rb'^0 ID RUN', re.M)
    emu.wait_for_output(idle_line, start=mark, timeout=120)
    together = len(re.findall(rb'^\d+ DS ', s.since(mark), re.M)) == DSKT_COPIES
    at_disc = re.search(rb'^\d+ DS WAIT DSK T0\r', s.since(mark), re.M) is not None

    # A claim that finds the disc taken waits to be woken when it is given
    # up, rather than sleeping a tick and looking again. dskt.asm never
    # sleeps of its own accord, so an exerciser seen asleep is one polling
    # the lock. Looked at a thousand times while all three are there.
    ds = ((ord('D') | 0x80) << 8) | (ord('S') | 0x80)
    looks = asleep = 0
    while looks < 1000:
        time.sleep(0.002)
        emu.halt()
        nodes = emu.memw(NARB, DSKT_COPIES * T_LEN)
        emu.run()
        nodes = [nodes[i * T_LEN:(i + 1) * T_LEN] for i in range(DSKT_COPIES)]
        if not all(n[T_STA] != SFREE and n[T_NAM] == ds for n in nodes):
            break
        looks += 1
        asleep += any(n[T_STA] == SSLP for n in nodes)

    # ...and then none.
    for _ in range(400):
        emu.wait_quiet(timeout=120)
        mark = len(emu.output)
        emu.key('STAT\r')
        emu.wait_for_output(idle_line, start=mark, timeout=120)
        at_disc |= re.search(rb'^\d+ DS WAIT DSK T0\r', s.since(mark), re.M) is not None
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

    # The tick counts the ticks that find the machine idle. Across HELP,
    # which the shell prints at ten characters a second, asleep between
    # them, most ticks do; a shell that spun instead would be found running
    # by nearly every one. A character takes five ticks to the cycle, so
    # the shell's work for each keeps one phase against the tick for the
    # whole of the text, and where that phase puts the work under a tick,
    # every character is a busy tick: a fifth of them, which is the most
    # there can be.
    def ticks():
        emu.halt()
        t, sec, it, isec = emu.memw(symbols.value('TICKS'), 4)
        emu.run()
        return sec * 50 + t, isec * 50 + it
    emu.wait_quiet(timeout=120)
    first = ticks()
    mark = len(emu.output)
    emu.key('HELP\r')
    emu.wait_for_output(re.compile(rb'^ANY OTHER WORD .* LISTS THEM\.\r\n', re.M), start=mark, timeout=120)
    emu.wait_quiet(timeout=120)
    last = ticks()
    spent, idle = last[0] - first[0], last[1] - first[1]
    if spent < 1000 or (spent - idle) * 4 > spent:
        raise AssertionError('%d of %d ticks found a task running' % (spent - idle, spent))
    s.at = len(emu.output)

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
    if not at_disc:
        raise AssertionError('STAT never showed an exerciser waiting for the disc')
    if looks < 200:
        raise AssertionError('only %d looks at the exercisers while all three were there' % looks)
    if asleep:
        raise AssertionError('%d of %d looks found an exerciser asleep' % (asleep, looks))

    # A Ctrl-C is for the program that has the console. DUMP, behind the
    # prompt, has minutes of machine time's printing to do; NAP has the
    # console and never takes the break, so the flag stays up from the
    # first key to the second, which is the one that ends NAP.
    size = fs703.Platter(bytearray(platter)).find('HELLO', 'RX').size
    last = b'%04X ' % ((size - 1) // 16 * 16)
    mark = len(emu.output)
    emu.key('START DUMP HELLO.RX\r')
    # its second line: the prompt prints into the first
    emu.wait_for_output(b'\n0010 ', start=mark, timeout=120)
    emu.key('NAP\r')
    deadline = time.time() + 120
    while True:
        emu.halt()
        # the console's terminal block, TMT, and its TM.BSY
        held = emu.memw(symbols.value('TMT') + symbols.value('TM.BSY'), 1)[0]
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
    if CONTROL == 'nobackoff':
        # the STW *T.RES that has a task wait for the one in its way to
        # have had its time
        emu.writew(listing_word('STW     *T.RES', 'tried for again until it has'), [NOP])
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
    load = 'START HOG %d\r' % HOG_WORDS
    mark = len(emu.output)
    emu.key(load * 2 + 'STOP %d\rSTOP %d\r' % (first, second) + load * 2 + 'STAT\r')
    emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark, timeout=300)
    listed = s.since(mark)
    for job in (first, second):
        if not re.search(rb'^%d HO OFF +OUT T0\r' % job, listed, re.M):
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
    emu.key(''.join('START BASIC JOB%d\r' % n for n in range(1, JOBS + 1)) + 'BASIC\r')
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

    # Three BASICs that never end, where two fit, so they take turns. The
    # one that is out cannot come back until one in core has had its
    # time, and is tried for once meanwhile, its list read off the disc
    # once -- not on every pass of the swapper. The swapper's transfers
    # are counted for LOOP_SECS of machine time: a list read for each one
    # brought back, and one more for a try that found it too soon.
    emu.wait_quiet()
    emu.key('START BASIC LOOP\r' * LOOPERS)
    deadline = time.monotonic() + 300
    while len(jobs_of(emu, 'BA')) < LOOPERS:
        if time.monotonic() > deadline:
            raise AssertionError('the looping BASICs never all started')
        time.sleep(0.5)
    swxfer, swhdi, secs = (symbols.value(n) for n in ('SWXFER', 'SWHDI', 'SECS'))
    emu.halt()
    began = emu.memw(secs, 1)[0]
    ins0 = emu.memw(symbols.value('SWINS'), 1)[0]
    emu.add_break(swxfer)
    emu.run()
    reads = 0
    while True:
        emu.wait_stopped(timeout=120)
        if emu.memw(secs, 1)[0] - began >= LOOP_SECS:
            break
        if emu.reg('AC') == 0 and emu.memw(symbols.value('SWADR'), 1)[0] == swhdi:
            reads += 1
        emu.run()
    emu.remove_break(swxfer)
    ins = emu.memw(symbols.value('SWINS'), 1)[0] - ins0
    emu.run()
    if ins < 2:
        raise AssertionError('the looping BASICs took no turns: %d brought back' % ins)
    if reads > 2 * ins + 2:
        raise AssertionError('%d lists read for %d brought back' % (reads, ins))
    emu.wait_quiet()
    emu.key(''.join('KILL %d\r' % job for job, _ in jobs_of(emu, 'BA')))
    stat_until_gone(emu, s, b'BA')
    s.at = len(emu.output)
    emu.key('MEM\r')
    emu.wait_for_output(b'FREE ' + before, start=s.at, timeout=60)

    # A load that waits for core: a stopped hog holds all but a little of
    # the core below word X'4000', so BASIC's module, which loads nowhere
    # else, has no room until the hog goes out. The load waits inside
    # K.LOAD, holding the disc -- seen at ALWAIT -- and has to come back to
    # the shell that asked for it with BASIC ready and the pool whole.
    if CONTROL == 'nokehlk':
        # KENTER's STW *T.KLK that puts its link back after the body
        emu.writew(listing_word('STW     *T.KLK', 'LDW     KEHLK'), [NOP])
    emu.wait_quiet()
    s.say('START HOG %d' % LOADWAIT_HOG)
    s.wait(PROMPT, timeout=60)
    emu.wait_quiet()
    (hog, _), = jobs_of(emu, 'HO')
    emu.key('STOP %d\r' % hog)
    s.wait(PROMPT, timeout=60)
    emu.wait_quiet()
    held = blocks_of(emu, 'HO')
    if not any(end <= POOLH and end - start >= LOADWAIT_HOG for start, end in held):
        raise AssertionError('the hog did not take its core below the fence: %r' % held)
    alwait, iolock, curt = (symbols.value(n) for n in ('ALWAIT', 'IOLOCK', 'CURT'))
    emu.halt()
    emu.add_break(alwait)
    emu.run()
    mark = len(emu.output)
    emu.key('BASIC\r')
    holder_waits = 0
    deadline = time.monotonic() + 120
    while not READY.search(bytes(emu.output), mark):
        if time.monotonic() > deadline:
            raise AssertionError('no READY from a load that waited for core (%d waits by the '
                                 'disc\'s holder): %r' % (holder_waits, s.since(mark)[-300:]))
        try:
            ev = emu.wait_stopped(timeout=0.5)
        except TimeoutError:
            continue
        if ev.pc == alwait:
            if emu.memw(iolock, 1)[0] == emu.memw(curt, 1)[0]:
                holder_waits += 1
            emu.run()
    emu.remove_break(alwait)
    if not holder_waits:
        raise AssertionError('BASIC\'s load never waited for core holding the disc')
    s.at = mark
    s.basic('BYE\r')
    s.say('KILL %d' % hog)
    stat_until_gone(emu, s, b'HO')
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


def noshell_session(emu):
    """A platter without SHELL.RX: the console has no shell to load, and
    says so before the machine stops."""
    if CONTROL == 'nobootmsg':
        # SHBOOT's STW *T.MBX: without it nothing reaches the printer
        emu.writew(listing_word('STW     *T.MBX', 'SHBOOT          LDW'), [NOP])
    emu.run()
    emu.wait_for_output(b'NO SHELL.RX\r\n', timeout=60)
    stopped = emu.wait_stopped()
    if stopped.reason != 'hlt':
        raise AssertionError('expected REX to halt the machine, got %r' % (stopped,))


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


def verdict(paper):
    """What the shell had to have printed, on lines of its own: the
    terminal's echo of a command begins with the prompt instead."""
    text = paper.replace(b'\r', b'')

    def line(pattern):
        return re.search(pattern, text, re.M) is not None

    def count(pattern):
        return len(re.findall(pattern, text, re.M))

    free = re.findall(rb'^FREE (\d+ LOW \d+ HIGH)$', text, re.M)
    # hello's line whole on a line of its own: the one START started prints
    # into whatever else is printing and may not be
    hellos = [int(n) for n in re.findall(rb'^HELLO FROM WORD (\d+)$', text, re.M)]
    checks = [
        ('uptime', line(rb'UPTIME \d+ SEC')),
        ('shell running', line(rb'^0 SH RUN +T0$')),
        ('a STAT header', line(rb'^JOB NM STATE TERM$')),
        ('idle listed', line(rb'^0 ID RUN')),
        ('help: what is built in', line(rb'^BUILT IN: HELP STAT UPTIME')),
        # every one of them, eight to a line
        ('help: where the programs are', line(rb'^ANY OTHER WORD RUNS THE PROGRAM OF THAT NAME\. DIR 0: LISTS THEM\.$')),
        ('echo', line(rb'^SHELL OUTPUT OK')),
        ('unknown command', line(rb'^WHAT')),
        ('directory: hello', line(rb'^HELLO\.RX \d+$')),
        ('directory: basic', line(rb'^BASIC\.RX \d+$')),
        ('directory: the free sectors', line(rb'^\d+ SECTORS FREE')),
        # RUN's hello has the printer to itself. START's prints behind the
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
        ('the time of day, twice, the second later', clock_moved(text)),
    ]
    return [name for name, ok in checks if not ok]


def clock_moved(text):
    """TIME's two readings, HH:MM:SS each, the second later -- across
    midnight too, and by less than an hour."""
    times = [(int(h), int(m), int(s)) for h, m, s in re.findall(rb'^(\d\d):(\d\d):(\d\d)$', text, re.M)]
    if len(times) != 2 or any(h > 23 or m > 59 or s > 59 for h, m, s in times):
        return False
    first, second = [h * 3600 + m * 60 + s for h, m, s in times]
    return 0 < (second - first) % 86400 < 3600


def make_platter(workdir, log, swap=False, boot=False, shell=True):
    os.makedirs(os.path.join(workdir, 'disks'))
    image = os.path.join(workdir, 'disks', 'ray703-disc0.img')
    blank = [sys.executable, MKDISC, image] + (['--boot', BOOT_BIN] if boot else [])
    cmd = [sys.executable, FS703, image]
    if boot and CONTROL != 'nosys':
        cmd += ['--sys', ROM_FILE]
    if swap:
        cmd += ['--swap', str(SWAP_SECTORS)]
    if CONTROL == 'blank':
        pass
    elif CONTROL in ('', 'nohello', 'nolock', 'nosave', 'noguard', 'noswapin', 'nosys', 'nopark',
                     'nolatch', 'sysopen', 'noseg', 'onlycon', 'onetty', 'nogetty',
                     'nounit', 'dirunit', 'nodata', 'fcwho', 'nohold', 'nobreak',
                     'noloadstop', 'nodetach', 'nobrkq', 'nokillwake', 'spin', 'nowake', 'onesec', 'nobackoff', 'shlow',
                     'conbye', 'nobootmsg', 'nokehlk'):
        files = []
        jobs = [('JOB%d.BA' % n, b'\n'.join(job_lines(n)) + b'\n') for n in range(1, JOBS + 1)] if swap else []
        jobs += [('LOOP.BA', LOOP)] if swap else []
        for name, data in [('PAD.TX', PAD), ('MSG.TX', MSG), ('CMDS.CM', CMDS), ('INIT.CM', INIT),
                           ('SQ.BA', SQ)] + jobs:
            files.append((name, os.path.join(workdir, name)))
            with open(files[-1][1], 'wb') as f:
                f.write(data)
        for name, path in files[:1] + list(MODULES) + files[1:]:
            if (CONTROL, name) != ('nohello', 'HELLO') and (shell or name != 'SHELL'):
                high = (name, path) in MODULES and name not in LOW_ONLY + (('SHELL',) if CONTROL == 'shlow' else ())
                cmd += ['--add-high' if high else '--add', name, path]
    else:
        sys.exit('REX_CONTROL=%s: not a control this harness has' % CONTROL)
    subprocess.run(blank, check=True, stdout=log)
    if cmd[3:]:
        subprocess.run(cmd, check=True, stdout=log)
    log.flush()
    return image


def platters_session(emu):
    """Unit 0, the platter make_platter makes, and unit 1, a directory and
    nothing else, which makes it the data unit."""
    if CONTROL == 'nounit':
        # DXFER's start word: LDW DXUNI / SLL 14 / ORI DXCNT -- with a CLR
        # for the load, the count goes out with unit 0 in front of it
        emu.writew(listing_word('the unit in bits 0-1', 'DXFER           SUBR'), [CLR])
    if CONTROL == 'dirunit':
        # FBDIRN's JSX FDLOAD, the platter's directory brought in: a CLR
        # in its place, the answer of a directory already there
        emu.writew(listing_word('JSX     FDLOAD', 'FBDIRN          SUBR'), [CLR])
    if CONTROL == 'nodata':
        # FSUNIT's finding-out: LDW =1 / STW FSUNI / JSX FDLOAD -- with the
        # call gone the answer is never zero, and the data unit unit 0
        emu.writew(listing_word('JSX     FDLOAD', 'FSUNIT          SUBR'), [NOP])
    if CONTROL == 'fcwho':
        # FCWHO's LDW *C.UNI / CMW FDUNIT: a CLR makes every channel unit
        # 0's, which unit 0 never notices
        emu.writew(listing_word('LDW     *C.UNI', 'FCWHO           SUBR'), [CLR])
    emu.run()
    s = Session(emu)
    s.wait(b'REX 703 UP')
    s.wait(re.compile(rb'^INIT RAN\r', re.M))   # INIT.CM, off unit 0
    emu.wait_quiet()
    s.say('PUT NOTE')                   # no unit: the data unit's
    emu.key(NOTE_KEYS + '\x1b')
    mark = s.say('TYPE NOTE')
    s.wait(PROMPT)
    if b'LINE ONE\r\nLINE TWO' not in s.since(mark):
        raise AssertionError('TYPE NOTE: %r' % s.since(mark))
    s.at = mark
    s.say('COPY NOTE 0:NOTE0')          # onto unit 0...
    s.say('COPY 0:MSG 1:MSG2')          # ...and off it
    mark = s.say('REN NOTE 0:X')
    s.wait(PROMPT)
    if b'NOT ONE PLATTER' not in s.since(mark):
        raise AssertionError('a REN across platters: %r' % s.since(mark))
    s.at = mark
    for command, there, absent in (('DIR', (b'NOTE.TX 1', b'MSG2.TX 1'), (b'HELLO.RX', b'NOTE0.TX')),
                                   ('DIR 0:', (b'HELLO.RX', b'NOTE0.TX 1', b'MSG.TX'), (b'NOTE.TX', b'MSG2.TX'))):
        mark = s.say(command)
        s.wait(b'SECTORS FREE')
        listed = s.since(mark)
        for name in there + absent:
            if (re.search(rb'^' + re.escape(name), listed, re.M) is not None) != (name in there):
                raise AssertionError('%s: %r' % (command, listed))

    # A write on unit 1 that a look at unit 0 comes between (plat.asm),
    # and a program run off unit 1.
    s.say('RUN PLAT')
    m = s.wait(re.compile(rb'PLAT (OK|BAD \d+)'))
    if m.group(1) != b'OK':
        raise AssertionError(m.group(0).decode())
    s.say('COPY 0:HELLO.RX 1:HI.RX')
    s.say('1:HI')
    s.wait(b'HELLO FROM WORD')

    # BASIC comes from unit 0; what it saves goes to unit 1, and a program
    # named on its line comes from there.
    s.say('BASIC')
    s.basic('10 PRINT "ON ONE"\r')
    s.basic('SAVE PROG\r')
    s.basic('BYE\r')
    s.say('BASIC PROG')
    s.wait(b'ON ONE')
    s.basic('BYE\r')
    s.say('DEL MSG2')
    s.say('SQUISH')
    s.say('MEM')


def check_platters(unit0, made0, unit1):
    """Both platters, read the host's way: unit 1 holds what was put
    there, and unit 0 is as it was made with one file more."""
    p1 = fs703.Platter(bytearray(open(unit1, 'rb').read()))
    p0 = fs703.Platter(bytearray(open(unit0, 'rb').read()))
    for name, p in (('unit 0', p0), ('unit 1', p1)):
        bad = p.check()
        if bad:
            raise AssertionError('%s breaks the rules: %s' % (name, '; '.join(bad)))
    prog = bytes(c | 0x80 for c in b'10 PRINT "ON ONE"\r\n')
    made1 = {e.title: p1.read(e.name, e.type) for e in p1.files()}
    with open(HELLO_OBJ, 'rb') as f:
        hello = f.read()
    if made1 != {'NOTE.TX': NOTE, 'PROG.BA': prog, 'PW.TX': bytes([0xc1]) * 200, 'HI.RX': hello}:
        raise AssertionError('unit 1 holds %r' % made1)
    kinds = [e.what for e in p1.entries()]
    if kinds != [fs703.FILE] * (len(kinds) - 1) + [fs703.HOLE]:
        raise AssertionError('unit 1 not squished: %r' % p1.entries())
    before = fs703.Platter(bytearray(made0))
    was = {e.title: before.read(e.name, e.type) for e in before.files()}
    now = {e.title: p0.read(e.name, e.type) for e in p0.files()}
    if now != dict(was, **{'NOTE0.TX': NOTE}):
        raise AssertionError('unit 0 changed: %r' % sorted(set(now.items()) ^ set(was.items())))


def listing_word(source, after, listing=REX_LST):
    """The address of the first word assembled from the first line of a
    listing, build/rex.lst unless another is named, past the line holding
    `after` whose text holds `source` -- for a control that patches a word
    no label names."""
    lines = open(listing, encoding='latin-1').read().splitlines()
    start = next(i for i, l in enumerate(lines) if after in l)
    at = next(i for i in range(start, len(lines)) if source in lines[i])
    for line in lines[at:]:
        m = re.match(r'([0-9A-F]{4})  [0-9A-F]{4}', line)
        if m:
            return int(m.group(1), 16)
    raise AssertionError('no word for %r in %s' % (source, listing))


def patch_shell(emu, source, after, word):
    """A word of the console's shell -- SHELL.RX, which the kernel loaded
    at start-up and entered at SHENT -- replaced, found as listing_word
    finds it in the module's own listing, which is assembled from word 0.
    For a machine whose banner has printed: it is halted to patch."""
    symbols, shell = Symbols(), Symbols()
    symbols.load(REX_LST)
    shell.load(SHELL_LST)
    emu.halt()
    entry = emu.memw(symbols.value('SHENT'), 1)[0]
    emu.writew(entry - shell.value('SHELL') + listing_word(source, after, SHELL_LST), [word])
    emu.run()


def terminal_port(log, n):
    """Where the emulator just said teletype n's line is: the last such line
    in the log, the startup line of the machine that was just spawned."""
    log.flush()
    found = re.findall(rb'serial: teletype %d, multiplexer channel %d is on 127\.0\.0\.1:(\d+)' % (n, n - 1),
                       open(LOG_FILE, 'rb').read())
    if not found:
        raise AssertionError('the emulator never said where teletype %d is' % n)
    return int(found[-1])


def wait_without(paper, pattern, start, marks, timeout=120):
    """`pattern` in what a paper printed from `start` on, with the
    characters in `marks` -- a blip's -- taken out of it first."""
    deadline = time.time() + timeout
    while True:
        text = bytes(paper.output[start:]).translate(None, marks)
        m = pattern.search(text)
        if m:
            return m
        if time.time() > deadline:
            raise TimeoutError('no %r on the paper, marks aside (have %r)' % (pattern.pattern, text[-300:]))
        time.sleep(0.1)


def terminals_session(emu, t1, t2, t3):
    """The console and teletype 1, a shell on each. While something prints
    behind a prompt its marks land inside whatever else that paper prints,
    so commands typed then go in without waiting for a prompt -- the shell
    reads them from its queue when it is ready -- and what comes back is
    read with the marks taken out."""
    if CONTROL == 'onlycon':
        # SERV's ask of the multiplexer's status: a CLR there, and it never
        # looks past the console
        emu.writew(listing_word('DIN     10,0', 'SERV            SMB'), [CLR])
    if CONTROL == 'nobreak':
        # SERV's BREAK test: SAZ / JSX SVBRK -- without the call a set's
        # BREAK goes nowhere
        emu.writew(listing_word("BREAK, the set's?", 'SERV            SMB') + 1, [NOP])
    if CONTROL == 'onetty':
        # KICK's test of a node's terminal: CMW KTM / SEQ / JMP .NXT --
        # without the jump every task's mailbox is the printer's
        emu.writew(listing_word("this terminal's task?", 'KICK            SUBR') + 1, [NOP])
    if CONTROL == 'nogetty':
        emu.writew(listing_word('STW     *TM.REQ', 'SERV            SMB'), [NOP])
    if CONTROL == 'nokillwake':
        # KILLT is the SUBR's STX, then STW XDEAD / JSX CONBK / JSX REAP
        symbols = Symbols()
        symbols.load(REX_LST)
        emu.writew(symbols.value('KILLT') + 2, [NOP])
    emu.run()
    con, term = Session(emu), Session(t1)
    con.wait(b'REX 703 UP')
    emu.wait_quiet()
    con.say('MEM')
    free = con.wait(re.compile(rb'FREE (\d+) LOW')).group(1)

    # the first key at teletype 1 brings it a shell, and the key itself
    # reaches that shell: an empty line, and the prompt again
    t1.key('\r')
    term.wait(b'REX 703 TERMINAL 1\r\n')
    term.wait(PROMPT)
    symbols = Symbols()
    symbols.load(REX_LST)
    tm0 = symbols.value('TMT')
    tm1 = tm0 + symbols.value('TM.LEN')
    ((shell, tty),) = jobs_of(emu, 'SH')
    if tty != tm1:
        raise AssertionError("teletype 1's shell is on terminal block %04X, not %04X" % (tty, tm1))
    # SHELL.RX compares no byte addresses, so it is loaded above the fence,
    # leaving the core below it to BASIC
    low = [b for b in blocks_of(emu, 'SH') if b[0] < POOLH]
    if low:
        raise AssertionError("teletype 1's shell has blocks below word X'4000': %r" % low)

    # a BLIP behind each prompt, each printing on its own paper
    con.say('START BLIP #')
    term.say('START BLIP ~')
    con_mark, t1_mark = len(emu.output), len(t1.output)
    deadline = time.time() + 120
    while (emu.output[con_mark:].count(b'#') < 3 or t1.output[t1_mark:].count(b'~') < 3):
        if time.time() > deadline:
            raise AssertionError('the blips did not both print: %r / %r'
                                 % (bytes(emu.output[con_mark:]), bytes(t1.output[t1_mark:])))
        time.sleep(0.2)
    blips = dict((tty, job) for job, tty in jobs_of(emu, 'BL'))
    if set(blips) != {tm0, tm1}:
        raise AssertionError('the blips are on terminal blocks %r' % sorted(blips))
    con_blip, t1_blip = blips[tm0], blips[tm1]
    if b'~' in emu.output or b'#' in t1.output:
        raise AssertionError("a terminal printed the other's blip: console %r, teletype 1 %r"
                             % (bytes(emu.output), bytes(t1.output)))

    # the console's STAT shows the other terminal's shell and its task
    mark = len(emu.output)
    emu.key('STAT\r')
    stat = wait_without(emu, re.compile(rb'STAT\r\n(.*?)0 ID RUN', re.S), mark, b'#').group(1)
    want = {b'%d SH' % shell, b'%d BL' % t1_blip}
    have = set(re.findall(rb'(\d+ (?:SH|BL)) [^\r\n]* T1\r\n', stat))
    if have != want:
        raise AssertionError("STAT does not show teletype 1's shell and blip: %r" % stat)
    if not re.search(rb'^%d SH WAIT KBD T1\r' % shell, stat, re.M):
        raise AssertionError("STAT does not have teletype 1's shell waiting for a key: %r" % stat)

    # ATTACH is of a task of the shell's own terminal only
    mark = len(emu.output)
    emu.key('ATTACH %d\r' % t1_blip)
    wait_without(emu, re.compile(rb'NO SUCH TASK\r\n'), mark, b'#')

    # KILL on the console reaches either terminal's task
    emu.key('KILL %d\r' % con_blip)
    emu.key('KILL %d\r' % t1_blip)
    emu.wait_quiet(timeout=120)
    t1.wait_quiet()
    con.at = len(emu.output)
    term.at = len(t1.output)
    emu.key('\r')
    t1.key('\r')

    # a program there that takes no break: the second Ctrl-C ends it, and
    # only it -- the console's shell goes on answering
    held_at = tm1 + symbols.value('TM.BSY')
    term.say('RUN NAP')
    deadline = time.time() + 120
    while True:
        emu.halt()
        held = emu.memw(held_at, 1)[0]
        emu.run()
        if held:
            break
        if time.time() > deadline:
            raise AssertionError('NAP never had teletype 1')
        time.sleep(0.05)
    t1.key('\x03')
    time.sleep(0.5)
    t1.key('\x03')
    term.wait(b'KILLED\r\n')
    con.say('ECHO STILL HERE')
    con.wait(b'STILL HERE\r\n')

    # and a KILL from the console of the program in front of teletype 1
    # hands that terminal's console back: its shell's prompt comes again
    term.say('RUN NAP')
    deadline = time.time() + 120
    while not jobs_of(emu, 'NA'):
        if time.time() > deadline:
            raise AssertionError('the second NAP never came')
        time.sleep(0.05)
    wait_held(emu, held_at, 'the second NAP never had teletype 1')
    ((nap, _),) = jobs_of(emu, 'NA')
    con.say('STAT')
    con.wait(re.compile(rb'^%d NA WAIT KBD CON T1\r' % nap, re.M))
    con.say('KILL %d' % nap)
    term.at = term.wait(PROMPT, timeout=120).start()     # the next say's prompt

    # a Ctrl-C while teletype 1 prints: its channel is not listening, so
    # the key is the set's BREAK, which stops a dump that has twenty
    # minutes of printing left in it
    mark = term.say('DUMP BASIC.RX')
    t1.wait_for_output(b'\n0010 ', start=mark)
    t1.key('\x03')
    term.at = term.wait(PROMPT, timeout=120).start()     # the next say's prompt
    if re.search(rb'\n0100 ', bytes(t1.output[mark:])):
        raise AssertionError('the dump ran on past the BREAK: %r' % bytes(t1.output[mark:])[-200:])

    # teletype 3, the multiplexer's channel 2
    term3 = Session(t3)
    t3.key('\r')
    term3.wait(b'REX 703 TERMINAL 3\r\n')
    term3.say('ECHO THREE')
    term3.wait(b'THREE\r\n')
    term3.say('BYE')
    t3.wait_quiet()
    if b'THREE' in emu.output or b'THREE' in t1.output or b'THREE' in t2.output:
        raise AssertionError("teletype 3's echo came out elsewhere")

    # both printers busy at once: a long dump behind the console's prompt
    # and a file typed out on teletype 1, three times over, which has to
    # come out there whole -- a printer that took the other terminal's
    # characters would leave holes in it
    con.say('START DUMP BASIC.RX')
    con_mark = len(emu.output)
    emu.wait_for_output(b'\n0010 ', start=con_mark)
    t1_mark = len(t1.output)
    for _ in range(3):
        term.say('TYPE MSG.TX')
        term.wait(b'SECOND LINE\r\n')
    typed = bytes(t1.output[t1_mark:])
    if typed.count(b'FIRST LINE\r\nSECOND LINE\r\n') != 3:
        raise AssertionError('MSG.TX did not come out whole on teletype 1: %r' % typed)
    if b'LINE' in emu.output[con_mark:]:
        raise AssertionError("teletype 1's file came out on the console: %r"
                             % bytes(emu.output[con_mark:]))
    if re.search(rb'\n[0-9A-F]{4} 003B', emu.output[con_mark:]):
        raise AssertionError('the dump was over before the typing was')

    # the dump ended, the terminal's shell let go: the pool is whole
    ((dump, _),) = jobs_of(emu, 'DU')
    emu.key('KILL %d\r' % dump)
    emu.wait_quiet(timeout=120)
    con.at = len(emu.output)
    emu.key('\r')
    term.say('BYE')
    t1.wait_quiet()
    con.say('STAT')
    stat = con.wait(re.compile(rb'STAT\r\n(.*?)0 ID RUN', re.S)).group(1)
    if b'T1' in stat or re.search(rb'^\d+ (?:BL|DU|NA) ', stat, re.M):
        raise AssertionError('something is left after BYE: %r' % stat)
    con.say('MEM')
    after = con.wait(re.compile(rb'FREE (\d+) LOW')).group(1)
    if after != free:
        raise AssertionError('the pool is %s after BYE, %s before the shell' % (after.decode(), free.decode()))

    # and a key at it again is a new shell
    t1.key('\r')
    term.wait(b'REX 703 TERMINAL 1\r\n')
    term.wait(PROMPT)

    # teletype 2 asks for its shell while the console's is waiting on a
    # RUN, which is woken for it rather than nudged at its prompt
    held_at = tm0 + symbols.value('TM.BSY')
    con.say('RUN NAP')
    deadline = time.time() + 120
    while True:
        emu.halt()
        held = emu.memw(held_at, 1)[0]
        emu.run()
        if held:
            break
        if time.time() > deadline:
            raise AssertionError('NAP never had the console')
        time.sleep(0.05)
    term2 = Session(t2)
    t2.key('\r')
    term2.wait(b'REX 703 TERMINAL 2\r\n')
    term2.say('ECHO TWO')
    term2.wait(b'TWO\r\n')
    emu.key('\x03')
    time.sleep(0.5)
    emu.key('\x03')
    con.wait(b'KILLED\r\n')
    if b'TWO' in emu.output or b'TWO' in t1.output:
        raise AssertionError("teletype 2's echo came out elsewhere")

    # more hogs than core holds: a terminal's shell waiting for a key is
    # one the swapper may put out, and a key at its terminal brings it in
    emu.wait_quiet(timeout=120)
    con.at = len(emu.output)
    emu.key('START HOG %d\r' % 6000 * 4)
    out = None
    deadline = time.monotonic() + 300
    while out is None:
        emu.wait_quiet(timeout=120)
        mark = len(emu.output)
        emu.key('STAT\r')
        emu.wait_for_output(re.compile(rb'^0 ID RUN', re.M), start=mark, timeout=120)
        out = re.search(rb'^\d+ SH [^\r\n]*OUT T([12])\r', bytes(emu.output[mark:]), re.M)
        if time.monotonic() > deadline:
            raise AssertionError('no shell was put out: %r' % bytes(emu.output[mark:]))
    paper = {b'1': (t1, term), b'2': (t2, term2)}[out.group(1)][1]
    paper.say('ECHO BACK IN')
    paper.wait(b'BACK IN\r\n')
    stat_until_gone(emu, con, b'HO')


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

            # And a platter with no shell on it for the console.
            noshdir = os.path.join(workdir, 'noshell')
            os.makedirs(noshdir)
            make_platter(noshdir, log, shell=False)
            with cwd(noshdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--fast-io', '--no-throttle'], log=log)
            noshell_session(emu)
            finish(emu)
            log.write(b'\n--- paper, the platter with no shell ---\n' + bytes(emu.output))

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

            # And a second platter, the data unit.
            platdir = os.path.join(workdir, 'platters')
            os.makedirs(platdir)
            unit0 = make_platter(platdir, log)
            with open(unit0, 'rb') as f:
                made0 = f.read()
            unit1 = os.path.join(platdir, 'disks', 'ray703-disc1.img')
            subprocess.run([sys.executable, FS703, '--dir', unit1], check=True, stdout=log)
            with cwd(platdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--fast-io', '--no-throttle'], log=log)
            platters_session(emu)
            finish(emu)
            log.write(b'\n--- paper, the platters session ---\n' + bytes(emu.output))
            check_platters(unit0, made0, unit1)

            # And a second terminal, on a socket. The teletypes at their
            # own ten characters a second: a character printing holds its
            # task's mailbox for a character's time, which is what gives a
            # printer the chance to take another terminal's.
            termdir = os.path.join(workdir, 'terminals')
            os.makedirs(termdir)
            make_platter(termdir, log, swap=True)
            with cwd(termdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE, '--no-throttle',
                                 '--serial', 'tcp:0', '--serial', 'tcp:0', '--serial', 'tcp:0'], log=log)
            t1, t2, t3 = (Terminal(terminal_port(log, n)) for n in (1, 2, 3))
            try:
                terminals_session(emu, t1, t2, t3)
            finally:
                for n, t in enumerate((t1, t2, t3), 1):
                    log.write(b'\n--- paper, the terminals session: teletype %d ---\n' % n + bytes(t.output))
                    t.close()
            finish(emu)
            log.write(b'\n--- paper, the terminals session: the console ---\n' + bytes(emu.output))
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
