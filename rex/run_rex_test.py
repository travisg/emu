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

REX_CONTROL names a negative control, each of which must FAIL:
  blank    a platter with nothing on it (no catalogue lines, no modules)
  nohello  a platter without the hello module
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

from emudbg import Emu  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
BUILD = os.path.join(HERE, 'build')
ROM_FILE = os.path.join(BUILD, 'rex.bin')
HELLO_OBJ = os.path.join(BUILD, 'hello.obj')
BASIC_OBJ = os.path.join(BUILD, 'basic.obj')
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
    emu.run()
    s.wait(b'REX 703 UP')

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
        if not re.search(rb'^HE ', s.since(mark), re.M):
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

    # A BASIC session: the module comes off the platter, the console
    # changes hands, a program goes in and runs, and Ctrl-C -- which never
    # enters the queue; SERV raises the kernel's break flag -- stops a loop
    # that prints nothing and reads nothing.
    s.say('RUN BASIC')
    s.wait(b'TINY BASIC UNDER REX')
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
    s.basic('BYE\r')

    # Back at the shell: BYE gave back the module, its node and its
    # workspace, so STAT has no BASIC node, MEM reads as before, and a
    # second session -- BASIC is RUN BASIC under a shorter name -- starts
    # afresh, with nothing to LIST.
    mark = s.say('STAT')
    s.say('MEM')
    if re.search(rb'^BA ', s.since(mark), re.M):
        raise AssertionError('BASIC is still on the ring after BYE')
    s.say('BASIC')
    s.basic('LIST\r')
    s.basic('BYE\r')

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


def verdict(paper):
    """What the shell had to have printed, on lines of its own: the
    terminal's echo of a command begins with the prompt instead."""
    text = paper.replace(b'\r', b'')

    def line(pattern):
        return re.search(pattern, text, re.M) is not None

    def count(pattern):
        return len(re.findall(pattern, text, re.M))

    free = re.findall(rb'^FREE (\d+)', text, re.M)
    checks = [
        ('uptime', line(rb'UPTIME \d+ SEC')),
        ('A stopped', line(rb'^A  OFF')),
        ('shell running', line(rb'^SH RUN')),
        ('idle listed', line(rb'^ID RUN')),
        ('help', line(rb'^COMMANDS HELP STAT UPTIME')),
        ('echo', line(rb'^SHELL OUTPUT OK')),
        ('unknown command', line(rb'^WHAT')),
        ('catalogue: hello', line(rb'^HELL 2 4')),
        ('catalogue: basic', line(rb'^BASI 6 47')),
        # RUN's hello has the printer to itself. LOAD's prints behind the
        # prompt, a character at a time into whatever else is printing, so
        # it is taken from STAT's ring and the pool instead (session).
        ('hello ran under RUN', line(rb'^HELLO FROM WORD \d')),
        ('no such file', line(rb'^NO SUCH FILE')),
        ('five MEMs', len(free) >= 5),
        ('the pool whole after every load', len(set(free)) == 1),
        ('two runs', count(rb'^SQ9') == 2),
        ('two breaks', count(rb'BREAK AT 50') == 2),
        ('the program typed once and listed never', count(rb'20 PRINT "SQ"') == 1),
    ]
    return [name for name, ok in checks if not ok]


def make_platter(workdir, log):
    os.makedirs(os.path.join(workdir, 'disks'))
    image = os.path.join(workdir, 'disks', 'ray703-disc0.img')
    cmd = [sys.executable, MKDISC, image]
    if CONTROL == 'blank':
        pass
    elif CONTROL == 'nohello':
        cmd += ['--add', 'BASIC', BASIC_OBJ]
    elif CONTROL == '':
        cmd += ['--add', 'HELLO', HELLO_OBJ, '--add', 'BASIC', BASIC_OBJ]
    else:
        sys.exit('REX_CONTROL=%s: not a control this harness has' % CONTROL)
    subprocess.run(cmd, check=True, stdout=log)
    log.flush()


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    for path in (ROM_FILE, HELLO_OBJ, BASIC_OBJ):
        if not os.path.isfile(path):
            print('error: %s is missing' % path, file=sys.stderr)
            print('build it with: make -C rex', file=sys.stderr)
            return 1

    emu = None
    os.makedirs(os.path.dirname(LOG_FILE), exist_ok=True)
    with open(LOG_FILE, 'wb') as log, tempfile.TemporaryDirectory(prefix='rex-test-') as workdir:
        try:
            make_platter(workdir, log)
            with cwd(workdir):
                emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', ROM_FILE,
                                 '--fast-io', '--no-throttle'], log=log)
            session(emu)
            failed = verdict(bytes(emu.output))
            if failed:
                raise AssertionError('not on the paper: ' + ', '.join(failed))
            emu.kill()
            emu.wait_exited()
            status = emu.wait_exit()
            if status != 0:
                raise AssertionError('the emulator exited with status %d' % status)
            log.write(b'\n--- paper ---\n' + bytes(emu.output))
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
