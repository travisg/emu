#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Save states, over the debug port, on each machine.

Each machine is brought to a moment where a device is holding something --
the Altair's ACIA with a character in its receive register -- and
snapshotted there (S1). It is stepped M instructions and snapshotted again
(S_M), the guest's output in those steps kept. Then the live machine is
driven somewhere else, with more input, so that everything a load fails to
put back is now different, and S1 is restored. Two things must hold:

  * a snapshot taken straight after the restore is S1, byte for byte --
    every saved field is loaded back;
  * M steps from there make S_M again, byte for byte, and the same output
    -- nothing that matters to the next M steps was left out of the save.

The second is only worth anything because the device was mid-flight at S1
and the live machine was moved on before the restore: a field the save
forgot then carries the later value into the rerun, and the rerun differs.
A snapshot of another machine must also be refused.

`SNAPSHOT_MACHINES` picks the machines (comma-separated); the default is
every one whose guest this tree builds itself, which is what CI runs. The
negative control is a build with one device field left out of its save and
load, which must FAIL.
"""

import contextlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))
sys.path.insert(0, HERE)

from emudbg import DebugError, Emu  # noqa: E402
import run_altair680_irq_test as altair_irq  # noqa: E402
import run_altair8800_irq_test as altair8800_irq  # noqa: E402
import run_rc2014_ctc_test as rc2014_ctc  # noqa: E402

EMU_BIN = os.environ.get('EMU_BIN', os.path.join(ROOT, 'target', 'debug', 'emu'))
LOG_FILE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, 'snapshot_test.log')
STEPS = 2000


def altair680(workdir, log):
    """The IRQ harness's program, stopped on its handler's first
    instruction: the ACIA holds the character and has IRQ up."""
    rom = os.path.join(workdir, 'altair.bin')
    with open(rom, 'wb') as f:
        f.write(altair_irq.monitor_rom())
    emu = Emu.spawn([EMU_BIN, '-s', 'altair680', '-r', rom, '--no-throttle'], log=log)
    emu.write(altair_irq.PROGRAM, altair_irq.MAIN)
    emu.write(altair_irq.HANDLER, altair_irq.HANDLER_CODE)
    emu.add_break(altair_irq.HANDLER)
    emu.run()
    emu.key('S')
    stop = emu.wait_stopped()
    if stop.pc != altair_irq.HANDLER:
        raise AssertionError('altair680: stopped at %04x, not the handler' % stop.pc)
    emu.remove_break(altair_irq.HANDLER)

    def elsewhere():
        at = len(emu.output)
        emu.run()
        emu.key('ELSEWHERE')
        emu.wait_for_output('ELSEWHERE', start=at)
        emu.halt()
    return emu, elsewhere


def altair8800(workdir, log):
    """The 8080 IRQ harness's program, stopped on its handler's first
    instruction: the ACIA holds the character, INTE is clear and the HLT
    it woke from is over."""
    image = os.path.join(workdir, 'altair8800.bin')
    with open(image, 'wb') as f:
        f.write(altair8800_irq.image())
    emu = Emu.spawn([EMU_BIN, '-s', 'altair8800-image', '-r', image, '--no-throttle'], log=log)
    emu.add_break(altair8800_irq.HANDLER)
    emu.run()
    emu.key('S')
    stop = emu.wait_stopped()
    if stop.pc != altair8800_irq.HANDLER:
        raise AssertionError('altair8800: stopped at %04x, not the handler' % stop.pc)
    emu.remove_break(altair8800_irq.HANDLER)

    def elsewhere():
        at = len(emu.output)
        emu.run()
        emu.key('ELSEWHERE')
        emu.wait_for_output('ELSEWHERE', start=at)
        emu.halt()
    return emu, elsewhere


def altair8800_tape(workdir, log):
    """8K BASIC part way through loading off its paper tape: the checksum
    loader running, the tape read to a point the save must keep. Nothing
    is typed, so the load is the same every time. Needs the tape."""
    emu = Emu.spawn([EMU_BIN, '-s', 'altair8800', '--no-throttle'], log=log)
    emu.step(60000)

    def elsewhere():
        emu.step(90000)
    return emu, elsewhere, 30000


def type_line(emu, line):
    """A line typed a key at a time, each paced on its echo, as the 6809
    BASIC harness types: a key the guest reads while busy is lost."""
    for ch in line:
        at = len(emu.output)
        emu.key(ch)
        emu.wait_for_output(ch if ch != '\r' else '\n', start=at)


def system09(workdir, log):
    """6809 BASIC in a loop that prints, stopped part way through it.
    Needs roms/6809/BASIC.HEX."""
    emu = Emu.spawn([EMU_BIN, '-s', '6809', '--no-throttle'], log=log)
    emu.run()
    emu.wait_for_output('OK\r\n')
    type_line(emu, 'FOR I=1 TO 400:PRINT I;:NEXT\r')
    emu.wait_for_output(' 100 ')
    emu.halt()

    def elsewhere():
        emu.run()
        at = emu.wait_for_output('OK\r\n', start=len(emu.output) - 4).end()
        type_line(emu, 'PRINT 6*7\r')
        emu.wait_for_output(' 42 ', start=at)
        emu.halt()
    return emu, elsewhere, 50000


def rc2014_ctc_case(workdir, log):
    """The CTC harness's program, a timer under IM 2 waking a HALT, stopped
    between ticks: the channel is part way through its countdown."""
    rom = os.path.join(workdir, 'zeros.bin')
    with open(rom, 'wb') as f:
        f.write(bytes(64 * 1024))
    emu = Emu.spawn([EMU_BIN, '-s', 'rc2014', '-r', rom, '--no-throttle'], log=log)
    emu.write(rc2014_ctc.PROGRAM, rc2014_ctc.MAIN)
    emu.write(rc2014_ctc.HANDLER, rc2014_ctc.HANDLER_CODE)
    emu.write(rc2014_ctc.TABLE + 0x10, rc2014_ctc.ENTRY)
    emu.write(rc2014_ctc.COUNTER, b'\x00')
    emu.set_reg('PC', rc2014_ctc.PROGRAM)
    emu.step(rc2014_ctc.PERIOD // 4 * 2 + 5000)

    def elsewhere():
        emu.step(rc2014_ctc.PERIOD // 4 * 3 + 777)
    return emu, elsewhere, rc2014_ctc.PERIOD // 4 * 2 + 1000


RC2014_ROM = os.path.join(ROOT, 'roms', 'rc2014', '24886009.BIN')


def rc2014_basic(workdir, log):
    """The factory rom's BASIC printing a string, stopped with the SIO part
    way through sending it. Needs the rom."""
    emu = Emu.spawn([EMU_BIN, '-s', 'rc2014', '-r', RC2014_ROM, '--no-throttle'], log=log)
    emu.run()
    emu.wait_for_output('Memory top?')
    emu.key('\r')
    at = emu.wait_for_output('Ok\r\n').end()
    type_line(emu, 'PRINT "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"\r')
    emu.wait_for_output('\nABCDEFGH', start=at)
    emu.halt()

    def elsewhere():
        emu.run()
        at = emu.wait_for_output('Ok\r\n', start=len(emu.output) - 4).end()
        type_line(emu, 'PRINT 7*6\r')
        emu.wait_for_output(' 42', start=at)
        emu.halt()
    # short enough to end with the output still going: a run that reaches
    # BASIC's idle loop looks the same whatever its timing on the way
    return emu, elsewhere, 1500


def rc2014_cpm(workdir, log):
    """CP/M off a scratch copy of the card, stopped part way through a DIR:
    the card between sectors and the SIO sending. Needs the rom and the
    card. Then the card is written, and a restore must refuse it."""
    card = os.path.join(workdir, 'card.img')
    shutil.copyfile(os.path.join(ROOT, 'disks', 'rc2014-cf.img'), card)
    emu = Emu.spawn([EMU_BIN, '-s', 'rc2014-cpm', '-r', RC2014_ROM, '--disk', card, '--no-throttle'], log=log)
    emu.run()
    at = emu.wait_for_output('activate console').end()
    emu.key(' ')
    at = emu.wait_for_output('\r\n>', start=at).end()
    emu.key('X')
    at = emu.wait_for_output('Boot CP/M?', start=at).end()
    emu.key('Y')
    at = emu.wait_for_output('\r\nA>', start=at).end()
    emu.key('DIR\r')
    emu.wait_for_output('A: ', start=at)
    emu.halt()

    def elsewhere():
        emu.run()
        at = emu.wait_for_output('\r\nA>', start=len(emu.output) - 4).end()
        emu.key('C:\r')
        at = emu.wait_for_output('\r\nC>', start=at).end()
        emu.key('DIR\r')
        emu.wait_for_output('\r\nC>', start=at)
        emu.halt()

    def refuse(s1):
        emu.run()
        at = len(emu.output)
        emu.key('A:\r')
        at = emu.wait_for_output('\r\nA>', start=at).end()
        emu.key('SAVE 1 SNAP.TXT\r')
        at = emu.wait_for_output('\r\nA>', start=at).end()
        emu.key('DIR\r')
        emu.wait_for_output('SNAP', start=at)
        emu.halt()
        try:
            emu.command('restore %s' % s1)
        except DebugError as e:
            if 'has changed' not in str(e):
                raise
            return
        raise AssertionError('rc2014-cpm: a snapshot was restored over a card written since')
    return emu, elsewhere, 200000, refuse


def kaypro_keys(emu, text):
    """Keys at the Kaypro's keyboard, slowly enough for its 300 baud SIO
    to take each before the next, so none is left in the host's queue --
    which no snapshot holds."""
    for ch in text:
        emu.key(ch)
        time.sleep(0.3)


# CP/M's BIOS on the stock floppy reads through its own copies of the
# rom's HALT; INI; JR NZ loop, at 0xff2a and 0xff2f: each stopped on at
# its JR
KAYPRO_READ_LOOPS = (0xff2d, 0xff32)


def kaypro(workdir, log):
    """CP/M on a scratch copy of the stock floppy, stopped while TYPE reads
    a file: the controller part way through a sector, its next DRQ
    counting down. Its console is the screen, which the snapshot's video
    ram carries. Needs the roms and the floppy."""
    os.environ.setdefault('SDL_VIDEODRIVER', 'dummy')
    os.environ.setdefault('SDL_RENDER_DRIVER', 'software')
    disk = os.path.join(workdir, 'kaypro.img')
    shutil.copyfile(os.path.join(ROOT, 'disks', 'mbasic-games.img'), disk)
    emu = Emu.spawn([EMU_BIN, '-s', 'kaypro', '--disk', disk, '--no-throttle'], log=log)
    emu.run()
    time.sleep(3)
    # the JR NZ of a sector read loop: stopped there, the controller has
    # handed over a byte and is counting down to the next one's DRQ
    for pc in KAYPRO_READ_LOOPS:
        emu.add_break(pc)
    kaypro_keys(emu, 'TYPE CATCHUM.DAT')
    emu.key('\r')
    stop = emu.wait_stopped(timeout=30)
    if stop.pc not in KAYPRO_READ_LOOPS:
        raise AssertionError('kaypro: stopped at %04x, not in a read loop' % stop.pc)
    log.write(b'kaypro: in the read loop at %04x\n' % stop.pc)
    for pc in KAYPRO_READ_LOOPS:
        emu.remove_break(pc)
    emu.step(40)

    def elsewhere():
        emu.run()
        time.sleep(1)
        kaypro_keys(emu, 'TYPE LADDER.DAT\r')
        time.sleep(1)
        emu.halt()
    return emu, elsewhere, 100000


ROMS_703 = os.path.join(ROOT, 'roms', '703')


@contextlib.contextmanager
def cwd(path):
    """Spawn from `path`: the 703 mounts disks/ray703-disc*.img relative to
    where it runs, and the repo root's are the user's own."""
    old = os.getcwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)


def ray703_demo(workdir, log):
    """The demo printing its banner, stopped with the teletype part way
    through a character's tenth of a second. Needs make -C test ray703."""
    with cwd(workdir):
        emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', os.path.join(ROMS_703, 'demo.bin'), '--no-throttle'], log=log)
    emu.run()
    emu.wait_for_output('RAYTHEON')
    emu.halt()

    def elsewhere():
        emu.run()
        emu.wait_for_output('READY')
        emu.wait_quiet()
        emu.key('somewhere else\r')
        emu.wait_for_output('SOMEWHERE ELSE')
        # every key taken: the host's queue is no machine's state
        emu.wait_quiet()
        emu.halt()
    return emu, elsewhere, 60000


def rex(workdir, log):
    """REX with its three letter tasks running, stopped between ticks: the
    line clock armed and part way to its next, the scheduler's slices in
    flight. Needs make -C rex all modules."""
    build = os.path.join(ROOT, 'rex', 'build')
    os.makedirs(os.path.join(workdir, 'disks'), exist_ok=True)
    subprocess.run([sys.executable, os.path.join(ROOT, 'tools', 'mkdisc703.py'),
                    os.path.join(workdir, 'disks', 'ray703-disc0.img'),
                    '--add', 'HELLO', os.path.join(build, 'hello.obj'),
                    '--add', 'BASIC', os.path.join(build, 'basic.obj')], check=True, stdout=log)
    with cwd(workdir):
        emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', os.path.join(build, 'rex.bin'), '--no-throttle'], log=log)
    emu.run()
    emu.wait_for_output('REX 703 UP')
    emu.wait_quiet()
    emu.key('START\r')
    emu.wait_for_output('START')
    time.sleep(1)
    emu.halt()

    def elsewhere():
        emu.run()
        emu.key('STOP\r')
        emu.wait_quiet(timeout=60)
        emu.halt()
    return emu, elsewhere, 200000


DISC_WRITE_STARTED = 0x005a


def ray703_disc(workdir, log):
    """The disc exerciser against a blank platter, stopped with its first
    write in flight, the teletype on level 0 and the disc on level 1.
    Needs make -C test ray703."""
    os.makedirs(os.path.join(workdir, 'disks'), exist_ok=True)
    subprocess.run([sys.executable, os.path.join(ROOT, 'tools', 'mkdisc703.py'),
                    os.path.join(workdir, 'disks', 'ray703-disc0.img')], check=True, stdout=log)
    with cwd(workdir):
        emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', os.path.join(ROMS_703, 'disc.bin'), '--no-throttle'], log=log)
    # Before anything is written: a snapshot of this machine cut short,
    # refused part way through the load and rolled back. The platter must
    # still be write-through after it, which the exerciser's write proves
    # by reaching the file.
    platter = os.path.join(workdir, 'disks', 'ray703-disc0.img')
    with open(platter, 'rb') as f:
        blank = f.read()
    cut = os.path.join(workdir, 'cut.snap')
    with open(cut, 'wb') as f:
        f.write(snap(emu, os.path.join(workdir, 'pre.snap'))[:-1000])
    try:
        emu.command('restore %s' % cut)
        raise AssertionError('ray703-disc: a cut-short snapshot was restored')
    except DebugError as e:
        if 'ends early' not in str(e):
            raise
    # just past the DOT 1,4 that starts the write (test/703/disc.lst)
    emu.add_break(DISC_WRITE_STARTED)
    emu.run()
    stop = emu.wait_stopped()
    if stop.pc != DISC_WRITE_STARTED:
        raise AssertionError('ray703-disc: stopped at %04x, not past the write' % stop.pc)
    emu.remove_break(DISC_WRITE_STARTED)
    emu.step(20)

    written = {}

    def elsewhere():
        emu.run()
        emu.wait_for_output('DISC TEST PASS')
        emu.halt()
        with open(platter, 'rb') as f:
            written['file'] = f.read()
        if written['file'] == blank:
            raise AssertionError('ray703-disc: the platter rolled back at the start no longer writes its file')

    def kept(s1):
        """The restore put the platter back in memory: what the rerun wrote
        never reached the file, and savedisk is the way to keep it -- never
        over a file that is there."""
        with open(platter, 'rb') as f:
            if f.read() != written['file']:
                raise AssertionError('ray703-disc: the restored machine wrote into the image file')
        out = os.path.join(workdir, 'kept.img')
        emu.save_disk(0, out)
        if os.path.getsize(out) != len(written['file']):
            raise AssertionError('ray703-disc: savedisk wrote %d bytes' % os.path.getsize(out))
        try:
            emu.save_disk(0, platter)
        except DebugError as e:
            if 'exists' not in str(e):
                raise
        else:
            raise AssertionError('ray703-disc: savedisk wrote over a file that was there')
    return emu, elsewhere, 100000, kept


MACHINES = {
    'altair680': altair680, 'altair8800': altair8800, 'altair8800-tape': altair8800_tape, '6809': system09, 'rc2014-ctc': rc2014_ctc_case, 'rc2014': rc2014_basic,
    'rc2014-cpm': rc2014_cpm, 'kaypro': kaypro, 'ray703': ray703_demo, 'rex': rex, 'ray703-disc': ray703_disc,
}
DEFAULT = ['altair680', 'altair8800', 'rc2014-ctc', 'ray703', 'ray703-disc', 'rex']


def snap(emu, path):
    emu.command('snapshot %s' % path)
    with open(path, 'rb') as f:
        return f.read()


def check(name, setup, workdir, log):
    emu, elsewhere, *rest = setup(workdir, log)
    steps = rest[0] if rest else STEPS
    # a last check of the machine's own, after the rest: given the S1 path
    after = rest[1] if len(rest) > 1 else None
    try:
        s1, sm = os.path.join(workdir, name + '-1.snap'), os.path.join(workdir, name + '-m.snap')
        first = snap(emu, s1)
        start = len(emu.output)
        emu.step(steps)
        first_m = snap(emu, sm)
        first_out = bytes(emu.output[start:])

        elsewhere()
        if snap(emu, os.path.join(workdir, name + '-x.snap')) == first:
            raise AssertionError('%s: the live machine did not move on' % name)

        emu.command('restore %s' % s1)
        stop = emu.wait_stopped()
        if stop.reason != 'restore':
            raise AssertionError('%s: the restore stopped with %r' % (name, stop))
        again = snap(emu, os.path.join(workdir, name + '-1b.snap'))
        if again != first:
            at = next(i for i, (a, b) in enumerate(zip(first, again)) if a != b) if len(first) == len(again) else None
            raise AssertionError('%s: the restored machine saves differently (%d bytes against %d, first differing '
                                 'at %s)' % (name, len(again), len(first), at))
        start = len(emu.output)
        emu.step(steps)
        again_m = snap(emu, os.path.join(workdir, name + '-mb.snap'))
        again_out = bytes(emu.output[start:])
        if again_out != first_out:
            raise AssertionError('%s: %d steps from the restore printed %r, the first run %r'
                                 % (name, steps, again_out, first_out))
        if again_m != first_m:
            raise AssertionError('%s: %d steps from the restore end in a different state' % (name, steps))

        # the header: magic, version, then the system's name, length first
        n = int.from_bytes(first[12:16], 'little')
        other = os.path.join(workdir, 'other.snap')
        with open(other, 'wb') as f:
            f.write(first[:16] + b'x' * n + first[16 + n:])
        try:
            emu.command('restore %s' % other)
            raise AssertionError('%s: a snapshot of another machine was taken' % name)
        except DebugError as e:
            if 'this machine is' not in str(e):
                raise
        if after:
            after(s1)
        log.write(('%s: %d-byte snapshot, %d steps reproduced, %d bytes of output\n'
                   % (name, len(first), steps, len(first_out))).encode())
    finally:
        emu.kill()
        emu.wait_exit()


def main():
    if not os.access(EMU_BIN, os.X_OK):
        print('error: emulator binary not found at %s' % EMU_BIN, file=sys.stderr)
        print('build first with: cargo build', file=sys.stderr)
        return 1
    names = os.environ.get('SNAPSHOT_MACHINES', ','.join(DEFAULT)).split(',')
    with tempfile.TemporaryDirectory(prefix='snapshot-') as workdir, open(LOG_FILE, 'wb') as log:
        try:
            for name in names:
                check(name, MACHINES[name], workdir, log)
        except Exception:
            log.write(b'\n--- harness failure ---\n')
            log.write(traceback.format_exc().encode())
            print('FAIL: snapshot test (see %s)' % LOG_FILE, file=sys.stderr)
            return 1
    print('PASS: snapshot test (%s; %s)' % (', '.join(names), EMU_BIN))
    return 0


if __name__ == '__main__':
    sys.exit(main())
