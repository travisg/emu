#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""A client for the emulator's debug port (`emu --debug PATH`).

As a library:

    from emudbg import Emu
    emu = Emu.spawn([EMU_BIN, '-s', 'ray703', '-r', rom, '--no-throttle'])
    emu.run()
    emu.wait_for_output(b'READY')
    emu.wait_quiet()            # a 703 guest drops keys until its prompt has drained
    emu.key('PRINT 6*7\\r')
    emu.wait_for_output(rb'42\\r\\n')
    print(emu.halt(), emu.regs())
    emu.kill(); emu.wait_exit()

`spawn` starts the emulator halted with a socket in a temporary directory,
connects, and holds the child's stdin open for its whole life (the terminal
frontend reads EOF as ctrl-d). `Emu(path)` attaches to one already running.
Every command method returns the reply parsed, raises `DebugError` on an
`err`, and the machine's own events -- stops, the exit -- land in `events`;
guest output accumulates in `output`, with `wait_for_output` and
`wait_quiet` to pace on it. The protocol is documented in src/debug/mod.rs.

As a program:

    emudbg.py SOCK                    talk to a running emulator
    emudbg.py SOCK -c regs -c 'mem 0 16'   one command each, replies printed
    emudbg.py --spawn -- emu -s ray703 -r roms/703/basic.bin

Interactive mode is a terminal on the guest: what you type goes to its
keyboard and what it prints comes back, with the machine's events in
brackets. Ctrl-] opens a `dbg>` prompt for one debugger command (an empty
line returns to the guest); Ctrl-D leaves, the emulator running on.

Two commands are the client's own, in either mode, moving host files in
and out of guest memory over `write` and `mem`: `load ADDR FILE` and
`save ADDR LEN FILE` (addresses hex, as the port's; the length any Python
integer literal).

Symbols are the client's too, so the port stays address-only: `--symbols
FILE` (repeatable), `Emu.load_symbols`, reads an asm703.py listing (its
`symbols:` table) or an ASxxxx linker map (`aslink -m`). Then any address
a command takes may be a name, or a name plus hex (`break START`, `dis
SEND+2 8`) -- in core units for `break`, `dis`, `memw` and `writew` and
as bus bytes for `mem`, `write`, `watch`, `load` and `save`, by the unit
the port's banner gives -- `sym NAME` or `sym ADDR` looks one up, and a
`dis` or `history` listing names what it can: an address that is a
label, and an operand that is a symbol's value, as a `; name` comment. Halt the machine first. To put a CP/M program on a disk:
stop the guest in its TPA with a breakpoint on the BDOS entry (`break 5`,
then Return at the `A>`), `load 100 prog.com`, `unbreak 5`, `run`, then
type `SAVE n PROG.COM` -- n pages of 256 bytes.
"""

import argparse
import collections
import os
import queue
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time


# The most `mem` reads in one request, and so the chunk the file transfers
# move in.
MEM_CHUNK = 256


class DebugError(Exception):
    """An `err` reply, or the connection going away."""


# `insn` is the instruction at the PC, or None from a core without a
# disassembler; a watch's stop gives the PC the writing step began at
# (`by`; the interrupted instruction, when an interrupt's entry stacked),
# the first watched byte written (`addr`) and the value (`val`).
Stopped = collections.namedtuple('Stopped', 'reason pc insn by addr val', defaults=(None,) * 4)
Exited = collections.namedtuple('Exited', 'reason')


def escape(data):
    """Bytes to the protocol's text: printable ASCII as is, \\r \\n \\t \\\\
    by name, \\xHH for the rest."""
    out = []
    for b in data:
        if b == 0x5c:
            out.append('\\\\')
        elif b == 0x0d:
            out.append('\\r')
        elif b == 0x0a:
            out.append('\\n')
        elif b == 0x09:
            out.append('\\t')
        elif 0x20 <= b <= 0x7e:
            out.append(chr(b))
        else:
            out.append('\\x%02x' % b)
    return ''.join(out)


_ESCAPES = {'\\': b'\\', 'r': b'\r', 'n': b'\n', 't': b'\t', 'e': b'\x1b'}


def unescape(text):
    """The inverse of `escape`."""
    out = bytearray()
    i = 0
    while i < len(text):
        c = text[i]
        i += 1
        if c != '\\':
            out += c.encode('latin-1', 'replace')
            continue
        if i >= len(text):
            raise ValueError('trailing backslash')
        e = text[i]
        i += 1
        if e in _ESCAPES:
            out += _ESCAPES[e]
        elif e == 'x' and i + 2 <= len(text):
            out.append(int(text[i:i + 2], 16))
            i += 2
        else:
            raise ValueError('bad escape \\%s' % e)
    return bytes(out)


def _as_bytes(data):
    return data.encode('latin-1') if isinstance(data, str) else bytes(data)


class Symbols:
    """Names for addresses, in the core's own units, out of the listings
    and maps this tree's assemblers write."""

    # asm703.py's listing ends `symbols:` and then `  NAME  HHHH` lines;
    # aslink's map has `HHHH  name` cells, `|`-separated, under `Value
    # Global` headings.
    _LISTING_ROW = re.compile(r'^\s+(\S+)\s+([0-9A-Fa-f]{4})\s*$')
    _MAP_CELL = re.compile(r'([0-9A-Fa-f]{4,8})\s+(\S+)')

    def __init__(self):
        self.by_name = {}
        self.by_value = {}

    def add(self, name, value):
        self.by_name[name.upper()] = value
        self.by_value.setdefault(value, []).append(name)

    def load(self, path):
        """Read a listing or a map; returns how many names it gave."""
        with open(path, encoding='latin-1') as f:
            lines = f.read().splitlines()
        before = len(self.by_name)
        if 'symbols:' in lines:
            for line in lines[lines.index('symbols:') + 1:]:
                m = self._LISTING_ROW.match(line)
                if m:
                    self.add(m.group(1), int(m.group(2), 16))
        else:
            in_table = False
            for line in lines:
                if 'Value' in line and 'Global' in line:
                    in_table = True
                elif in_table and line.strip() and not line.strip().startswith('-'):
                    cells = [self._MAP_CELL.search(c) for c in line.split('|')]
                    if not any(cells):
                        in_table = False
                    for m in filter(None, cells):
                        if not m.group(2).startswith('.__.'):
                            self.add(m.group(2), int(m.group(1), 16))
        return len(self.by_name) - before

    def value(self, text):
        """`NAME` or `NAME+HEX` as a number, or None if it is neither."""
        name, _, off = text.partition('+')
        if name.upper() not in self.by_name:
            return None
        try:
            return self.by_name[name.upper()] + (int(off, 16) if off else 0)
        except ValueError:
            return None

    def name(self, value):
        names = self.by_value.get(value)
        return names[0] if names else None

    # a listing's numbers: ASxxxx's 0x hex and asm703's X'..', not after
    # a `#`, which makes them immediates
    _OPERAND = re.compile(r"(?<![#\w])(?:0x([0-9a-fA-F]+)|X'([0-9A-F]+)')")

    def annotate(self, addr, text):
        """A listing line's text with what the symbols say about it."""
        names = [n for n in (self.name(int(a or b, 16)) for a, b in self._OPERAND.findall(text)) if n]
        label = self.name(addr)
        out = ('%s: ' % label if label else '') + text
        return out + ('    ; ' + ', '.join(names) if names else '')


class Emu:
    """One connection to a debug port."""

    def __init__(self, path, timeout=20.0):
        self.timeout = timeout
        self.child = None
        self.socket_dir = None
        self.output = bytearray()
        self.events = queue.Queue()
        self.info = []
        self.transcript = []
        self._replies = queue.Queue()
        self._lock = threading.Lock()
        self._changed = threading.Condition(self._lock)
        self._closed = False
        self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._sock.connect(path)
        self._file = self._sock.makefile('rb')
        self.symbols = Symbols()
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        self.banner = self._wait_info()
        unit = re.search(r'\bunit=(\d+)', self.banner)
        # bus bytes to one of the core's address units
        self.unit = int(unit.group(1)) if unit else 1

    # -- spawning -----------------------------------------------------------

    @classmethod
    def spawn(cls, argv, *, socket_path=None, log=None, halt=True, timeout=20.0):
        """Start `argv` with a debug port and connect to it. `log` is a file
        object for the emulator's stdout and stderr (None discards them).
        The socket defaults to a fresh temporary directory rather than the
        working directory: a unix socket path is limited to about a hundred
        bytes, which a checkout on a network share can exceed."""
        socket_dir = None
        if socket_path is None:
            socket_dir = tempfile.mkdtemp(prefix='emudbg-')
            socket_path = os.path.join(socket_dir, 's')
        cmd = list(argv) + ['--debug', socket_path] + (['--halt'] if halt else [])
        child = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=log if log is not None else subprocess.DEVNULL,
            stderr=subprocess.STDOUT,
        )
        deadline = time.monotonic() + timeout
        while not os.path.exists(socket_path):
            if child.poll() is not None:
                raise DebugError('the emulator exited (status %d) before opening %s'
                                 % (child.returncode, socket_path))
            if time.monotonic() > deadline:
                child.kill()
                raise TimeoutError('no debug socket at %s after %gs' % (socket_path, timeout))
            time.sleep(0.02)
        emu = cls(socket_path, timeout=timeout)
        emu.child = child
        emu.socket_dir = socket_dir
        return emu

    def wait_exit(self, timeout=None):
        """Wait for a spawned emulator to exit; returns its status. Its stdin
        is closed only now -- while it runs, EOF there would stop it."""
        if self.child is None:
            raise DebugError('not spawned by this client')
        status = self.child.wait(timeout if timeout is not None else self.timeout)
        self.child.stdin.close()
        self.close()
        return status

    # -- the wire -----------------------------------------------------------

    def _read_loop(self):
        for raw in self._file:
            line = raw.rstrip(b'\r\n').decode('latin-1')
            self.transcript.append('< ' + line)
            if line.startswith('! out '):
                data = unescape(line[6:])
                with self._changed:
                    self.output += data
                    self._changed.notify_all()
            elif line.startswith('! stopped '):
                # insn= is last and runs to the end of the line
                head, _, insn = line[10:].partition(' insn=')
                fields = dict(f.split('=', 1) for f in head.split())
                watch = [int(fields[k], 16) if k in fields else None for k in ('by', 'addr', 'val')]
                self.events.put(Stopped(fields.get('reason', '?'), int(fields.get('pc', '0'), 16), insn or None, *watch))
            elif line.startswith('! exit '):
                fields = dict(f.split('=', 1) for f in line[7:].split())
                self.events.put(Exited(fields.get('reason', '?')))
            elif line.startswith('#'):
                self.info.append(line[2:] if line.startswith('# ') else line[1:])
                self._replies.put(line)
            else:
                self._replies.put(line)
        with self._changed:
            self._closed = True
            self._changed.notify_all()
        self._replies.put(None)
        self.events.put(None)

    def _wait_info(self):
        line = self._replies.get(timeout=self.timeout)
        if line is None:
            raise DebugError('connection closed')
        return line[2:]

    def command(self, line):
        """Send one command line; return the text after `ok`. `#` lines on
        the way (help, dis) are collected in `info`."""
        self.transcript.append('> ' + line)
        try:
            self._sock.sendall((line + '\n').encode('latin-1'))
        except OSError as e:
            raise DebugError('connection closed: %s' % e)
        while True:
            reply = self._replies.get(timeout=self.timeout)
            if reply is None:
                raise DebugError('connection closed')
            if reply.startswith('#'):
                continue
            if reply.startswith('err'):
                raise DebugError(reply[4:])
            return reply[3:] if reply.startswith('ok ') else ''

    def close(self):
        """Drop the connection; the emulator runs on."""
        if not self._closed:
            try:
                self._sock.sendall(b'quit\n')
            except OSError:
                pass
        try:
            self._sock.close()
        except OSError:
            pass

    # -- commands -------------------------------------------------------------

    @staticmethod
    def _status(text):
        fields = dict(f.split('=', 1) for f in text.split())
        return {
            'state': fields.get('state'),
            'reason': fields.get('reason'),
            'pc': int(fields.get('pc', '0'), 16),
            'insns': int(fields.get('insns', '0')),
        }

    def status(self):
        return self._status(self.command('status'))

    def halt(self):
        """Halt; returns the status. The stop event a halt raises precedes
        its reply on the wire, so it is consumed here rather than left for
        `wait_stopped` to trip over."""
        status = self._status(self.command('halt'))
        self._drain_stop('request')
        return status

    def _drain_stop(self, reason):
        kept = []
        while True:
            try:
                ev = self.events.get_nowait()
            except queue.Empty:
                break
            if isinstance(ev, Stopped) and ev.reason == reason:
                continue
            kept.append(ev)
        for ev in kept:
            self.events.put(ev)

    def run(self):
        self.command('run')

    def step(self, n=1):
        """Execute `n` instructions; returns the stop."""
        self.command('step %d' % n)
        return self.wait_stopped()

    def reset(self):
        self.command('reset')
        return self.wait_stopped()

    def regs(self):
        return {k: int(v, 16) for k, v in (f.split('=', 1) for f in self.command('regs').split())}

    def reg(self, name):
        return int(self.command('reg %s' % name).split('=', 1)[1], 16)

    def set_reg(self, name, value):
        return int(self.command('set %s %x' % (name, value)).split('=', 1)[1], 16)

    def mem(self, addr, n):
        return bytes(int(b, 16) for b in self.command('mem %x %d' % (addr, n)).split())

    def load_file(self, addr, path):
        """Write the file at `path` into memory from `addr`, in the port's
        bus bytes, and return its length. Stop the machine first: a write
        lands wherever the address decodes at that moment, and a write the
        machine would drop -- under a banked-in rom -- is a `DebugError`."""
        with open(path, 'rb') as f:
            data = f.read()
        for off in range(0, len(data), MEM_CHUNK):
            self.write(addr + off, data[off:off + MEM_CHUNK])
        return len(data)

    def save_file(self, addr, n, path):
        """Read `n` bytes of memory from `addr` into the file at `path`, as
        the address decodes at the moment: on the Kaypro, a machine stopped
        in its rom reads the rom at the bottom of memory, not the TPA."""
        data = b''.join(self.mem(addr + off, min(MEM_CHUNK, n - off)) for off in range(0, n, MEM_CHUNK))
        with open(path, 'wb') as f:
            f.write(data)
        return len(data)

    def memw(self, addr, n):
        return [int(w, 16) for w in self.command('memw %x %d' % (addr, n)).split()]

    def write(self, addr, data):
        self.command('write %x %s' % (addr, ' '.join('%02x' % b for b in _as_bytes(data))))

    def writew(self, addr, words):
        self.command('writew %x %s' % (addr, ' '.join('%04x' % w for w in words)))

    def add_break(self, addr):
        self.command('break %x' % addr)

    def remove_break(self, addr):
        self.command('unbreak %x' % addr)

    def breaks(self):
        return [int(b, 16) for b in self.command('breaks').split()]

    def watch(self, addr, n=1):
        """Stop after an instruction writes into `n` bus bytes from `addr`."""
        self.command('watch %x %d' % (addr, n))

    def unwatch(self, addr):
        self.command('unwatch %x' % addr)

    def watches(self):
        """(addr, len) for each watch."""
        return [tuple(int(v, 16) if i == 0 else int(v) for i, v in enumerate(w.split(':')))
                for w in self.command('watches').split()]

    def load_symbols(self, path):
        """Names from an asm703.py listing or an ASxxxx map; returns how
        many."""
        return self.symbols.load(path)

    def dis(self, addr, n=1):
        """`n` instructions from `addr` (core units), as (addr, raw, text)
        tuples: raw a list of the instruction's units."""
        return self._listing('dis %x %d' % (addr, n))

    def history(self, n=16):
        """The last `n` instructions executed, oldest first, as `dis` lists
        them."""
        return self._listing('history %d' % n)

    def _listing(self, line):
        before = len(self.info)
        self.command(line)
        out = []
        for line in self.info[before:]:
            a, raw_text = line.split('  ', 1)
            raw, text = raw_text[:14].split(), raw_text[15:]
            out.append((int(a, 16), [int(u, 16) for u in raw], text))
        return out

    def key(self, data):
        """Keystrokes for the guest. A string is sent as latin-1."""
        self.command('key ' + escape(_as_bytes(data)))

    def backlog(self):
        return unescape(self.command('backlog'))

    def disk(self, drive, path):
        """Put the image at path in a drive ('a', 'b', or a number)."""
        self.command('disk %s %s' % (drive, path))

    def eject(self, drive):
        self.command('eject %s' % drive)

    def kill(self):
        self.command('kill')

    # -- waiting --------------------------------------------------------------

    def wait_for_output(self, pattern, timeout=None, start=0):
        """Block until `pattern` (bytes, str or a compiled bytes regex)
        matches the accumulated output at or after byte `start`; returns
        the match, whose `end()` is where a script that walks the output
        in order picks up next -- the same prompt comes round again, and
        without a cursor the first one satisfies every wait for it."""
        if isinstance(pattern, str):
            pattern = pattern.encode('latin-1')
        if isinstance(pattern, bytes):
            pattern = re.compile(re.escape(pattern))
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        with self._changed:
            while True:
                m = pattern.search(bytes(self.output), start)
                if m:
                    return m
                if self._closed:
                    raise DebugError('connection closed waiting for %r' % pattern.pattern)
                left = deadline - time.monotonic()
                if left <= 0:
                    raise TimeoutError('no %r in the output (have %r)'
                                       % (pattern.pattern, bytes(self.output[-200:])))
                self._changed.wait(left)

    def wait_quiet(self, idle=0.3, timeout=None):
        """Block until the output has not grown for `idle` seconds -- the
        printer going quiet, which is how an operator knew a 703 guest was
        ready for the next line."""
        deadline = time.monotonic() + (timeout if timeout is not None else self.timeout)
        with self._changed:
            while True:
                n = len(self.output)
                if not self._changed.wait(idle):
                    if len(self.output) == n:
                        return
                if time.monotonic() > deadline:
                    raise TimeoutError('the output never went quiet')

    def wait_stopped(self, timeout=None):
        """The next stop event. An exit or a closed connection instead is a
        `DebugError`."""
        try:
            ev = self.events.get(timeout=timeout if timeout is not None else self.timeout)
        except queue.Empty:
            raise TimeoutError('the machine did not stop')
        if ev is None:
            raise DebugError('connection closed waiting for a stop')
        if isinstance(ev, Exited):
            raise DebugError('the emulator exited (%s) waiting for a stop' % ev.reason)
        return ev

    def wait_exited(self, timeout=None):
        """The exit event."""
        try:
            ev = self.events.get(timeout=timeout if timeout is not None else self.timeout)
        except queue.Empty:
            raise TimeoutError('the emulator did not exit')
        if ev is None:
            raise DebugError('connection closed waiting for the exit')
        if not isinstance(ev, Exited):
            raise DebugError('expected the exit, got %r' % (ev,))
        return ev


# -- the program -------------------------------------------------------------

# Which argument of a command is an address, and in what: the core's
# units, or bus bytes.
_CORE_ADDRESS = {'break': 0, 'unbreak': 0, 'dis': 0, 'memw': 0, 'writew': 0, 'set': 1}
_BYTE_ADDRESS = {'mem': 0, 'write': 0, 'watch': 0, 'unwatch': 0, 'load': 0, 'save': 0}


def _resolve(emu, line):
    """`line` with a symbol in its address argument made hex."""
    words = line.split()
    if not words or not emu.symbols.by_name:
        return line
    verb = words[0].lower()
    at = _CORE_ADDRESS.get(verb, _BYTE_ADDRESS.get(verb))
    if at is None or len(words) <= at + 1:
        return line
    value = emu.symbols.value(words[at + 1])
    if value is None:
        return line
    if verb in _BYTE_ADDRESS:
        value *= emu.unit
    words[at + 1] = '%x' % value
    return ' '.join(words)


def _info_lines(emu, line, before):
    """The `#` lines a command brought, a listing's with its symbols."""
    listing = line.split()[:1] in (['dis'], ['history'])
    for l in emu.info[before:]:
        if listing and emu.symbols.by_name:
            addr, _, rest = l.partition('  ')
            l = '%s  %s %s' % (addr, rest[:14], emu.symbols.annotate(int(addr, 16), rest[15:]))
        yield '# ' + l


def _command(emu, line):
    """One command line: the client's own `load ADDR FILE`, `save ADDR
    LEN FILE` and `sym NAME|ADDR`, or a line for the port."""
    line = _resolve(emu, line)
    words = line.split(None, 3)
    if words and words[0] == 'sym':
        if len(words) != 2:
            raise DebugError('usage: sym NAME|ADDR')
        value = emu.symbols.value(words[1])
        if value is not None:
            return '%s=%x' % (words[1].upper(), value)
        try:
            names = emu.symbols.by_value.get(int(words[1], 16))
        except ValueError:
            names = None
        if not names:
            raise DebugError('no symbol %s' % words[1])
        return '%s=%s' % (words[1], ','.join(names))
    if words and words[0] == 'load':
        if len(words) != 3:
            raise DebugError('usage: load ADDR FILE')
        return '%d bytes' % emu.load_file(int(words[1], 16), words[2])
    if words and words[0] == 'save':
        if len(words) != 4:
            raise DebugError('usage: save ADDR LEN FILE')
        return '%d bytes' % emu.save_file(int(words[1], 16), int(words[2], 0), words[3])
    return emu.command(line)


def _one_shot(emu, commands):
    status = 0
    for cmd in commands:
        before = len(emu.info)
        try:
            reply = _command(emu, cmd)
        except DebugError as e:
            print('err %s' % e)
            status = 1
            continue
        for line in _info_lines(emu, cmd, before):
            print(line)
        print('ok %s' % reply if reply else 'ok')
    return status


def _interactive(emu):
    import termios
    import tty

    fd = sys.stdin.fileno()
    saved = termios.tcgetattr(fd)
    out = sys.stdout.buffer

    def show(data):
        out.write(data)
        out.flush()

    def pump():
        # guest output and events, as they arrive
        seen = 0
        while True:
            with emu._changed:
                emu._changed.wait(0.1)
                chunk = bytes(emu.output[seen:])
                seen = len(emu.output)
                closed = emu._closed
            if chunk:
                show(chunk)
            while True:
                try:
                    ev = emu.events.get_nowait()
                except queue.Empty:
                    break
                if ev is None:
                    return
                show(('\r\n[%s]\r\n' % ' '.join('%s=%s' % (k, ('%04x' % v) if k == 'pc' else v)
                                              for k, v in ev._asdict().items() if v is not None)).encode())
            if closed:
                return

    show(('[%s]\r\n[ctrl-] for a debugger command, ctrl-d to leave]\r\n' % emu.banner).encode())
    threading.Thread(target=pump, daemon=True).start()
    try:
        tty.setraw(fd)
        while True:
            c = os.read(fd, 1)
            if not c or c == b'\x04':
                break
            if c == b'\x1d':
                termios.tcsetattr(fd, termios.TCSADRAIN, saved)
                try:
                    line = input('\ndbg> ').strip()
                    if line:
                        before = len(emu.info)
                        try:
                            reply = _command(emu, line)
                            for l in _info_lines(emu, line, before):
                                print(l)
                            print('ok %s' % reply if reply else 'ok')
                        except DebugError as e:
                            print('err %s' % e)
                except EOFError:
                    break
                finally:
                    tty.setraw(fd)
                continue
            try:
                emu.key(c)
            except DebugError as e:
                show(('\r\n[%s]\r\n' % e).encode())
                break
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, saved)
        print()


def main(argv=None):
    ap = argparse.ArgumentParser(description='talk to an emulator debug port')
    ap.add_argument('socket', nargs='?', help='the socket the emulator was given with --debug')
    ap.add_argument('-c', '--command', action='append', default=[],
                    help='send one command and print its reply (repeatable)')
    ap.add_argument('--spawn', nargs=argparse.REMAINDER, metavar='EMU ARGS',
                    help='start the emulator with these arguments and attach to it; last, '
                         'everything after it is the emulator\'s')
    ap.add_argument('--symbols', action='append', default=[], metavar='FILE',
                    help='names from an asm703.py listing or an ASxxxx map (repeatable)')
    ap.add_argument('--timeout', type=float, default=20.0)
    # Everything after --spawn is the emulator's, split off here: argparse's
    # REMAINDER does not reliably take a `--` and what follows it.
    argv = list(sys.argv[1:] if argv is None else argv)
    spawn = None
    if '--spawn' in argv:
        at = argv.index('--spawn')
        argv, spawn = argv[:at], argv[at + 1:]
    args = ap.parse_args(argv)

    if spawn is not None:
        cmd = spawn[1:] if spawn[:1] == ['--'] else spawn
        if not cmd:
            ap.error('--spawn needs the emulator command after it')
        emu = Emu.spawn(cmd, log=sys.stderr, halt=False, timeout=args.timeout)
    elif args.socket:
        emu = Emu(args.socket, timeout=args.timeout)
    else:
        ap.error('a socket path or --spawn is required')

    for path in args.symbols:
        print('[%s: %d symbols]' % (path, emu.load_symbols(path)), file=sys.stderr)
    try:
        if args.command:
            return _one_shot(emu, args.command)
        _interactive(emu)
        return 0
    finally:
        emu.close()
        if emu.child is not None:
            emu.child.stdin.close()
            emu.child.wait()


if __name__ == '__main__':
    sys.exit(main())
