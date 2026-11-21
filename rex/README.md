# REX

REX -- Raytheon EXec -- is a small multitasking operating system for the
Raytheon 703, a 16-bit minicomputer from 1967 with 32K words of core. It is
new software written for an old machine, not a transcription of anything
Raytheon shipped. It boots off a disc platter and gives you:

- a shell on the console teletype, and on up to three more teletypes
- programs loaded off the disc, run in front of the prompt or behind it
- a filesystem, with the usual utilities: `DIR`, `TYPE`, `COPY`, `DEL` ...
- Tiny BASIC, with programs saved to disc
- swapping, when more is loaded than core holds

Underneath is a round-robin scheduler, preemptive and cooperative at
once, ticking fifty times a second off Raytheon's Time-of-Day Clock.

This file is how to build it, boot it and use it. How it works inside,
and the rules for changing it, are in [AGENTS.md](AGENTS.md).

## Quick start

From the repository root. REX's own build needs only python3; the
emulator's prerequisites are in the [root README](../README.md).

```bash
cargo build                    # the emulator
make -C rex bootdisc           # rex/build/rex.img: the whole system on a platter
./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io
```

That presses the disc controller's LOAD button on the platter, and REX
comes up at its prompt. `HALT` shuts it down, and Ctrl-D leaves the
emulator at any time.

`--fast-io` makes the teletype instant. Without it the Model 33 prints at
its real ten characters a second, which is authentic and slow. Scheduling
runs in real machine time either way.

To see the whole installation -- front panel, disc rack and a teletype
window -- use `-s ray703-panel-tty-load` instead and press RUN.

Or let the launcher put the line together: bare `./target/debug/emu` on a
terminal opens a menu, and once the platter is built it is listed there
under the Raytheon 703's "boot disc via LOAD", beside the windows, the
serial ports and `--fast-io` to switch on.

## A first session

Return ends a line, and what is typed is folded to upper case. Backspace
is the teletype's RUBOUT: it takes back the last character of the line,
though a teletype cannot unprint it, so the paper still shows it.

```
REX 703 UP
TYPE HELP FOR THE COMMANDS
REX>  RUN HELLO
HELLO FROM WORD 16386
REX>  LOAD HELLO
JOB 1
REX>  START 1
RHELLO FROM WORD 16386
EX>  STAT
UPTIME 5 SEC
SWAPS 0 OUT 0 IN
0 SH RUN
0 SW WAIT
1 HE SLP 62
0 ID RUN
REX>  BASIC
TINY BASIC UNDER REX

READY
10 FOR I=1 TO 5
READY
20 PRINT I, I*I
READY
30 NEXT I
READY
RUN
1       1
2       4
3       9
4       16
5       25
READY
SAVE SQ
READY

JOB 2 STOPPED
REX>  STAT
UPTIME 12 SEC
SWAPS 0 OUT 0 IN
0 SH RUN
0 SW WAIT
2 BA OFF
0 ID RUN
REX>  ATTACH 2
BYE
REX>  TYPE SQ.BA
10 FOR I=1 TO 5
20 PRINT I, I*I
30 NEXT I
REX>  HALT
REX 703 DOWN
```

`RUN` gives a program the console until it exits. `LOAD` loads it
stopped and says its job number, and `START` lets it run behind the
prompt, which is why the second `HELLO` prints into the prompt that has
come back, and why `STAT` finds it as job 1, asleep (`SLP`) with 62 ticks
to go. The other lines of `STAT` are the shell, the swapper and the idle
task. The Ctrl-Z typed at BASIC's `READY` prints nothing; it stops BASIC
and gives the shell the console, and `ATTACH` gives it back, BASIC
taking up where it was.

## The shell

| command | what it does |
|---|---|
| `HELP` or `?` | the commands, and where the programs are listed |
| `STAT` | every task: job number, name, state, how long a sleeper has left, `OUT` for one swapped to disc |
| `UPTIME` or `UP` | seconds since REX came up |
| `TIME` | the time of day, off the clock (`HH:MM:SS`) |
| `MEM` | free words in the pool, below word X'4000' and above it |
| `ECHO text` | print the rest of the line |
| `RUN name` | load the program `name.RX` and give it the console until it exits |
| `LOAD name` | load it stopped, and say its job number |
| `START name` | load it and run it behind the prompt |
| `START n` | let a stopped task run behind the prompt, by job number |
| `STOP n` | stop a task |
| `ATTACH n` | give one of this terminal's tasks the console, as `RUN` does |
| `KILL n` | end it, whatever it was doing |
| `@name` | take the shell's lines from the command file `name.CM` |
| `HALT` | stop every task, let the printers finish and stop the machine (console only) |
| `BYE` | end this shell (the other terminals only) |

Any other word is a program to `RUN`: `BASIC` is `RUN BASIC`, and
whatever follows the name on the line is the program's to read. A program
behind the prompt has no console: if it reads the keyboard it waits,
and `STAT` shows it as `TTY`, until `ATTACH` gives it the console.

The utilities are programs on the disc:

| program | what it does |
|---|---|
| `DIR [unit:]` | a platter's directory: name, type, sectors, then what is free |
| `TYPE name` | print a file |
| `PUT name` | the keyboard into a file, until ESC (the Model 33's ALT MODE) |
| `COPY from to` | copy a file |
| `REN old new` | rename one |
| `DEL name` | delete one |
| `DUMP name` | print a file in hexadecimal |
| `SQUISH [unit:]` | close up the holes between the files |

**Ctrl-C** is the running program's to act on, and the ones that print
at length do: Tiny BASIC stops the program it is running, `TYPE` and
`DUMP` stop printing, and a command file (`@name`) is closed. A second
Ctrl-C while the first is still untaken ends whatever program has the
console, which is the way out of one that is not listening. A program
behind the prompt never sees a Ctrl-C; it is for the one with the
console.

**Ctrl-Z** stops the program that has the console and gives the console
back to the shell, which says `JOB n STOPPED`. The program keeps its
place: `START n` lets it go on behind the prompt, and `ATTACH n` brings
it back to the front.

**Background tasks** keep running behind whatever is typed, a BASIC
session included, and print on the same paper. Every terminal prints its
own keys -- the console's Model 33 armed for it (`DOT 14,11`), the
multiplexer's sets wired for it -- so typing interleaves with the tasks'
output character by character, the way two users on one printer did.
Input goes through a queue, so there is type-ahead: a burst typed while
the machine is busy is held and run in order, and only a burst deeper
than the queue is lost. (Standalone Tiny BASIC, in `../test/703`, has
none.)

## Files

A file name is up to six characters and a type of two after a dot:
`NOTE.TX`. Names are upper case. A name given without a type gets one
from who it is given to:

| given to | type | which is |
|---|---|---|
| `RUN`, `LOAD`, or the shell as a bare word | `.RX` | a program |
| `@` | `.CM` | a command file |
| a utility | `.TX` | text |
| BASIC's `SAVE` and `OLD` | `.BA` | a BASIC program |

If unit 0 has an `INIT.CM`, the shell reads it when the machine comes up,
before the keyboard.

### Platters

The machine has four disc units. In front of a name, `0:` to `3:` says
which one it is on: `TYPE 0:MSG`, `COPY NOTE 1:NOTE`, `DIR 0:`.

Without a prefix, a program (`.RX`) is on unit 0, the platter REX boots
from, and any other file is on the *data unit*: unit 1 if it has a
directory, unit 0 if not.

That matters because `rex.img` is a build product, made afresh by every
`make -C rex bootdisc`, and anything saved on it goes with it. To keep
your files, give REX a data platter once:

```bash
rex/fs703.py --dir disks/ray703-disc1.img      # an empty platter with a directory
./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io
```

Under `-s ray703-load` the `-r` platter is unit 0, and
`disks/ray703-disc1.img` is mounted as unit 1 by its name. From then on
what is saved lands on unit 1 and survives the rebuilds; `DIR` lists the
data unit and `DIR 0:` the programs. A prefix only ever overrides the
default: nothing is refused on either platter, except a `REN` from one to
the other.

Every name is read by one routine in the kernel, `K.NAME`, so the shell,
the utilities and BASIC all take the same names.

`fs703.py` is the same filesystem from the host's side: it lists and
checks a platter and puts files on one. `rex/fs703.py -h` has the options.

## Tiny BASIC

`BASIC` starts the same integer BASIC the emulator's standalone guest
runs (`../test/703/bcore.asm`): variables `A`-`Z`, an array `@(0..1023)`,
`PRINT INPUT LET IF GOTO GOSUB RETURN FOR NEXT REM END`, `LIST RUN NEW`,
and `BYE` to return to the shell. Under REX it has two statements more:

- `SAVE name` writes the program to the file `name.BA`
- `OLD name` empties the program and reads `name.BA` back in

`BASIC PROG` loads `PROG.BA` and runs it. `START BASIC PROG` runs it behind
the prompt with no console: what the program prints is printed, an
`INPUT` waits until the task is attached, and the task is over when the
program ends.

## More terminals

Teletypes 1 to 3 sit beside the console, on Raytheon's Teletype
Multiplexer. Each `--serial` on the command line plugs one in, in turn:

```bash
./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io --serial window     # a teletype window
./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io --serial tcp:7031   # then: nc localhost 7031
```

A terminal has no shell until someone uses it. The first key typed at one
brings it a shell (the console's shell loads `SHELL.RX` for it), so the
first Return gets a banner naming the terminal and a prompt.

Each shell runs the same commands. A task belongs to the terminal of the
shell that started it: it prints there, and `RUN` or `ATTACH` gives it
that terminal's console, so `ATTACH` takes only the shell's own
terminal's tasks. `STAT`, `KILL`, `STOP` and `START` reach every task,
and `STAT` marks another terminal's (`T1`). `BYE` ends a terminal's
shell, and the next key there brings a new one.

A multiplexer channel goes one way at a time, and while one is printing
only its BREAK -- Ctrl-C -- gets through, so a Ctrl-Z reaches a program
there when its printing pauses. Two Ctrl-Cs end one that never pauses.

A shell costs about 2,000 words of core, and an idle one waiting for a key
is the first thing swapped out when core runs short.

## What is in this directory

    rex.asm            the executive: scheduler, drivers, queues
    shell.asm          the shell, and the loader it runs modules with
    shellmod.asm       the front of the shell's deck as SHELL.RX, every other terminal's
    boot.asm           the boot sector: reads the kernel in off the platter
    rexfs.asm          the filesystem: the directory, channels, the file calls
    rexswap.asm        the swapper: tasks out to the disc and back
    rexname.asm        K.NAME, the one reading of a file name, unit and all
    rexmac.asm         the macros a deck is written with: conditional jumps, CALL, MSG
    rexlib.asm         a library for a module's deck: the printer, the command line
    brex.asm           the wrapper that makes Tiny BASIC a module
    hello.asm          a module: prints where it landed, sleeps, exits
    util/              the utilities: DIR, TYPE, PUT, COPY, REN, DEL, DUMP, SQUISH
    dskt.asm           a test module: writes a disc sector and reads it back, over and over
    fst.asm            a test module: makes, reads, replaces, renames and deletes files
    nap.asm            a test module: never ends, for the shell to end
    blip.asm           a test module: prints a character and sleeps, over and over
    hog.asm            a test module: holds a block of core and watches it
    far.asm            a test module: two segments, each calling into the other
    plat.asm           a test module: a write on unit 1 across a look at unit 0
    fs703.py           the filesystem from the host's side: lists, checks and
                       puts files on a platter, the kernel and swap file too
    run_rex_test.py    the end-to-end test
    makefile           builds everything into build/, which is gitignored

`build/rexapi.asm`, the kernel interface every module's deck begins with,
is made there with the kernel.

## Writing a program for REX

A program is a *module*: a deck assembled from word 0 by
`../tools/asm703.py --object` into relocatable text, which the shell's
loader places wherever there is room. It reaches the kernel through the
entries named in `build/rexapi.asm`.

- Start from one of the utilities in `util/`: they use the macros
  (`rexmac.asm`) and the library (`rexlib.asm`), and `del.asm` is twenty
  lines. `hello.asm` uses neither, so it shows the kernel calls bare.
- [WRITING.md](../test/703/WRITING.md) is the guide to writing assembly
  for this machine.
- "Modules" in [AGENTS.md](AGENTS.md) says what a module may and may not
  do, and "The rules" there what the kernel expects of a task.
- The makefile's rule for a utility is the build line to copy, and
  `fs703.py --add` in its `bootdisc` rule is what puts the result on the
  platter (`--add-high` for a module that may be loaded above word
  X'4000', which "The rules" explains).

## Testing

```bash
make -C rex test           # the scripted session
make -C rex reloc-check    # the assembler's modules against the reference loader
```

The session boots REX on platters it makes in a scratch directory and
drives it over the emulator's debug port; it needs python3 and the
emulator binary (`EMU_BIN` names one other than `target/debug/emu`). The
relocation check runs no emulator at all. "The test" in
[AGENTS.md](AGENTS.md) says what the session covers, and the module doc
of `run_rex_test.py` lists its negative controls.
