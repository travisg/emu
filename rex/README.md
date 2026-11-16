# REX

REX -- Raytheon EXec -- is a round-robin executive for the Raytheon 703,
preemptive and cooperative at once, running on the emulator's invented 60 Hz
line clock. It is new software written for a 1967 machine, not a
transcription, and this directory is its own project: the executive, the
glue that puts Tiny BASIC aboard it as a task, and the scripted session that
tests it.

    rex.asm            the executive: scheduler, drivers, queues, the shell
    rexfs.asm          the filesystem: the directory, channels, the file calls
    rexapi.asm         the kernel interface a module is assembled against
    rexlib.asm         the printer and the command line, for a module's deck
    brex.asm           the wrapper that makes Tiny BASIC a module
    hello.asm          a module: prints where it landed, sleeps, exits
    util/              the utilities: TYPE, PUT, COPY, REN, DEL, DUMP, SQUISH
    dskt.asm           a test module: writes a disc sector and reads it back, over and over
    fst.asm            a test module: makes, reads, replaces, renames and deletes files
    nap.asm            a test module: never ends, for the shell to end
    run_rex_test.py    the end-to-end test
    makefile           builds everything into build/, which is gitignored

Two things it depends on stay where they are, reached by relative path from
the makefile: the interpreter itself is `../test/703/bcore.asm`, shared with
the standalone Tiny BASIC guest the emulator's own tests boot, and the
assembler is `../tools/asm703.py`. The emulator's tests do not depend on
anything here.

## Build, run, test

    cargo build                                                  # the emulator
    make -C rex disc                                             # rex.bin, the modules, and the platter
    ./target/debug/emu -s ray703 -r rex/build/rex.bin --fast-io  # a usable shell
    make -C rex test                                             # the scripted session
    make -C rex reloc-check                                      # the relocation check

`--fast-io` makes the teletype instant; without it the Model 33 takes its real
tenth of a second per character, and the scheduling slices are real machine
time either way. The shell's commands:

    HELP or ?    the command list
    STAT         every task's job number, name and state, and how long
                 each sleeper has left
    UPTIME, UP   seconds since the executive came up
    STOP  [A-C]  suspend a letter task, or all three
    START [A-C]  release one, or all three
    STOP n       the same for a task the shell loaded, by job number
    START n
    KILL n       end a loaded task, whatever it was doing
    ECHO text    print the rest of the line
    MEM          the words free in the pool, below word X'4000' and above
    DIR          the disc's directory: name, type, sectors; then what is free
    LOAD name    a module off the disc -- name.RX -- run behind the prompt
    RUN  name    the same, given the console until it exits -- the only
                 way to start a module that reads the console
    @name        take the shell's lines from name.CM until it runs out
    HALT         park the tasks, drain the printer and stop the machine

Any other word is a program: `BASIC` is `RUN BASIC`, and whatever follows
the name on the line is the program's to read -- `BASIC PROG` loads and
runs the saved program `PROG.BA`. The utilities are programs
on the disc, each a small module from `util/`:

    TYPE name       a file on the printer
    PUT name        the keyboard into a file, until Ctrl-Z
    COPY from to    a file again, under another name
    REN old new
    DEL name
    DUMP name       a file in hexadecimal
    SQUISH          close up the holes between the files

A file name is up to six characters and a type of two after a dot --
`NOTE.TX`. A name given to `RUN`, `LOAD` or the shell without a type is a
module, `.RX`; one given to `@` is a command file, `.CM`; one given to a
utility is text, `.TX`. If the disc has an `INIT.CM`, the shell reads it
when the machine comes up, before the keyboard.

Ctrl-C is the running program's to act on, and Tiny BASIC does. A second
Ctrl-C while the first is still untaken ends the program that has the
console, which is the way out of one that is not listening.

`START` sets the letter tasks going and they tick along behind whatever is
typed -- a BASIC session included, letters interleaving with `PRINT`'s output
on the one printer. The Model 33 echoes what is typed in hardware here
(`DOT 14,11`), so typing interleaves with the tasks' output character by
character, the way two users on one printer did. Unlike standalone Tiny
BASIC it has type-ahead: input goes through a queue, so a burst typed while
it is busy is held and run in order, and only a burst deeper than the queue
is lost.

## What it is

Four tasks share the processor at power-on, their control blocks a ring
of linked nodes, and whatever the shell loads joins them: everything that
names a task -- the current-task cell, the printer's owner, a queue's
waiter -- holds a node's address, a field is the indexed displacement off
it, every scan walks the `T.NXT` links, and what a task *is* is data in
its node, so `STOP`, `START` and `HALT` act on whatever the walk finds
and adding a task is linking a node in under `MSK`, which is what
the loader does through `SPAWN`. Three nodes run the one shared letter
body (`LTASK`, which reads its letter, nap and mailbox out of its own node
via `CURT`): print, sleep, repeat -- stopped at power-on, so the machine
comes up quiet and `START` sets them going. One is the shell. Tiny BASIC
is a module behind the `brex.asm` glue, which pays bcore's wrapper debts
with the executive's services: a workspace from the pool at entry, output
through the task's own mailbox, input through the console queue, the
break taken from the kernel's flag while the task has the console, `SAVE`
and `OLD` through the file calls, `BYE` routed to `K.EXIT`. The idle
task is a fifth node off the ring, the scans' explicit fallback.

A context is four words, ACR, IXR and the hardware-saved PC and status, so
the switch is a handful of word copies and an `INR 2`; the status word
carries EXR, the indicators and the addressing mode, which is why a task can
be preempted between an `SMB` and its reference, or a compare and its skip,
and resume intact. A node also carries a state and a delay, which is the
whole of sleep: a task marks itself sleeping and calls `SWTCH`, the
scheduler counts the delay down every tick and marks it runnable at zero,
and the scan passes over it meanwhile. `SWTCH` is the cooperative half of
the switch -- the machine has no yield instruction, so a task hands the
processor on by staging the incoming context in *level 3's* interrupt block
and executing `INR 3`, which is this machine's only instruction that loads a
program counter and a status word together. It must be level 3 and not
level 2: a tick's entry sequence writes level 2's block before any scheduler
instruction runs, so a switch staged there would be overwritten by the very
tick that deferred to it. Scheduling then happens at both ends -- the tick
takes the processor away from a task that has had it long enough, and a
service routine that made a task runnable returns as that task instead of
leaving it to wait for the next tick, so a character posted to the console
queue reaches its reader in the time it takes to return from the interrupt.
Both switching paths first test that the block they are about to park holds
a task's frame and not a driver's, which is the same range test. Nothing in
it holds the processor to wait for a device: a task that has handed a
character to the printer marks itself waiting and stands down until the
completion interrupt wakes it, exactly as the shell waits on its input
queue. So when every task is asleep or waiting the idle node -- a branch to
self -- has the processor, which in a traced run with the letters going is
~97% of the time, and the same ~97% whether or not `--fast-io` is on, which
is the sign that nothing is spinning on I/O.

## The rules

Each is stated once in `rex.asm`'s header, which is the reference; the
machine facts they rest on -- the interrupt blocks, EXR, the entry sequence
that does not reload it -- are in AGENTS.md's 703 section.

The scheduler **defers** whenever the saved PC lies inside the range holding
the service routines and the switch: a tick there either interrupted the
driver or found a switch half made, and both want the same answer. Every
service routine **leads with `SMB`**, because the entry sequence does not
reload EXR. **`SWTCH` is for tasks** -- a service routine that called it
would walk away from its own `INR` and leave its level Active for good,
which silently holds off every level at or below it. **Input goes through a
queue**: the service routine posts a character and wakes the waiter -- only
out of its wait, so a keystroke cannot restart a stopped task -- the reader
blocks in `Q.GET` rather than polling, and one waiter to a queue means one
reader. And **the console has one reader at a time**, named by the `CONBSY`
cell: `RUN` raises it in the masked window that clears the break flag,
then waits on it -- reading no queue -- until the task's `K.EXIT` clears
it and wakes the shell. That is also why **only a task `RUN` started may
read the console**: `LOAD` grants nothing and leaves the shell in `Q.GET`,
so a module that read the queue behind the prompt would put its own
registration in the queue's one waiter slot on top of the shell's. A
module that needs the console tests its node's `T.CON` at entry and
says so if it is clear, which is what `brex.asm` does when it has no
program to run instead; `hello.asm` only prints, which is what `LOAD` is
for. Ctrl-C never enters the queue at all: the service routine raises the
kernel's `BRKREQ` instead, so a running program that reads nothing can
still be broken. **The break is the console's**: BASIC's check between
statements goes through the wrapper, which answers for the kernel's flag
only while its task has the console, so a BASIC running behind the prompt
neither stops for a Ctrl-C nor takes one that was meant for the program
in front. A break belongs to a run: the
interpreter spends the flag at its `READY` loop, once the line is in, so
one typed while the prompt was up is not kept for whatever runs next.
`STOP`/`START` take a letter task by its letter or a loaded task by its job
number, never the shell and never the task that has the console -- stopping
the console's owner would leave the shell waiting on a hand-back nobody can
make. The kernel holds the one-reader rule itself as well: `Q.GET` on the
console's queue answers the shell and the task whose node says it has the
console, and tells anybody else no at once. **A task is ended by one routine
however it ends** -- `REAP`, from its own `K.EXIT` or from the shell's `KILL`
-- and the shell claims the disc before it ends another task, so the task is
inside no transfer and no file call when it goes. **The
pool is owned word by word**: every block carries the node that asked for
it, and a task's exit gives back all of them, which is what makes a
module's memory come and go with the module. The node itself is not in the
pool: a loaded task's comes from an arena of twenty-four at the top of
core, and stays put for as long as the task lives.

The pool runs from the end of the shell to the filesystem's page, words
X'1000' to X'6FFF', and **it is in two parts, because a byte address above
word X'4000' is negative** and this machine compares signed. Reaching a
byte up there through the index register works like anywhere else; what
goes wrong is comparing two byte addresses, which is how most code walks a
buffer to its end. So a fence stands at X'4000' -- a block of no words that
is never free, so nothing is ever joined across it -- and what a task asks
for comes from below it unless the task asks otherwise (`K.ALLHI`,
`K.ALHIW`), which is a promise to compare no byte addresses in what it
gets. A module is loaded above the fence only if its directory entry says
it may be (`mkdisc703.py --add-high`): everything here but BASIC, whose
interpreter walks its own tables and its workspace by byte address and so
has the core below the fence to itself.

Any number of tasks may be in the kernel at once, on a machine whose
subroutines each have one static slot for the return address. So **a
routine any task may call holds the mask from its first instruction** --
the allocator -- and one that blocks keeps what it needs afterwards in
the caller's node rather than in cells of its own, which is `Q.GET` and
`KENTER`. And **the disc has one user at a time**, named by `IOLOCK`: a
task's `K.DREAD` or `K.DWRIT` waits its turn in `KENTER`, a tick's sleep
at a time, and the shell's loader makes the same claim, so the driver
behind the claim is written like any single-task routine.

## The test

`make -C rex test` runs `run_rex_test.py` on a platter it makes in a
scratch directory, driving the emulator over its debug port
(`../tools/emudbg.py`): boot, `STAT`, `START` and watch the letters interleave,
`STOP`, the rest of the commands, hello both ways with `MEM` reading the
same after each, then two BASIC sessions -- a program entered and RUN, a
silent `GOTO` loop broken with Ctrl-C through the kernel's flag, the
program saved, `BYE` handing the console and the memory back, and a second
session starting afresh, with nothing to list until `OLD` brings the
program back -- then a host's program run by name, one run behind the
prompt until it ends, and one that never ends left running behind the
prompt through a Ctrl-C and then killed -- and `HALT`. The saved programs
are among the files the host reads back afterwards. In between it runs `fst.asm` twice, which makes,
reads back, replaces, renames and deletes files through the vector and
checks every answer; afterwards the harness reads the platter the host's
way (`../tools/fs703.py`): the directory has to pass the check of its
rules, hold exactly the files the exerciser leaves with the bytes it
wrote, and still hold every module byte for byte, and the platter as it
was made has to hold none of them. The same session drives the shell and
the utilities: a program run by its bare name, a host's file typed, a file
made with `PUT` and typed back, copied, renamed, deleted and dumped, a
command file, `INIT.CM` at start-up, a file deleted from in front of every
module and `SQUISH` moving the lot down -- after which the next program has
to load from where it was moved to -- and a task that never ends stopped,
started and killed by job number and then, in the foreground, by two
Ctrl-Cs. Where things are loaded is looked at directly, through the port,
in the pool's own block headers: a module marked for it has to be above
word X'4000', hello has to print an address up there every time it runs,
and BASIC's module and workspace both have to be below. A second
machine, without `--fast-io` so that a disc
transfer takes its half revolution, then runs three copies of `dskt.asm`
at once: each writes and reads back a sector of its own 250 times, leaves
its verdict on the platter where the harness reads it, and the kernel's
count of tasks that found the disc taken has to be more than zero.
It runs `--fast-io` (the slices stay real machine time; the clock ignores
the flag), walks the paper with a cursor so each wait finds the next prompt
rather than the first, and paces every command on that prompt and then on
the printer falling quiet, which is the rule for driving any 703 guest from
a script. Every wait raises on its timeout, and `REX_CONTROL=blank`,
`=nohello`, `=nolock`, `=nosave` and `=noguard` are its negative controls
-- a platter with nothing on it, one without the hello module, a kernel
with its wait for the disc patched out, one that never writes the
directory back, and one that lets any task read the console -- which must
all FAIL. Two things that make it hard
to verify by hand: under `--fast-io` the keyboard has no rate limit at all,
so a burst of input outruns any consumer and proves nothing about the
scheduling -- measure a switch in a `--trace` instead; and a number printed
at ten characters a second has to be read as a whole field, not matched on
its first digit.

## The filesystem

Disc unit 0 holds files: contiguous runs of sectors under a directory in
sectors 1-8, the way OS/8 and RT-11 kept theirs. `../tools/fs703.py` states
the format word for word and is the host's side of it; `rexfs.asm` is the
kernel's. An entry is eight words -- a name of six characters and a type
of two, the first sector, the sector count, the bytes used in the last
sector, and what the entry is -- and there are forty-six of them. They are
kept in disc order and **account for every data sector between them, a
hole being an entry like any other**, so there is no free map and nothing
to get out of step with the directory: a new file is given the largest
hole, whole, as a file being written, and closing it cuts it to its length
and makes the rest a hole again. A new file replaces an older one of its
name when it is closed, not before. **A file being written that no channel
holds is a hole** to whoever looks next -- its task exited, or the machine
stopped -- which is all the recovery there is and all that is needed.

The kernel keeps the directory in core, reads it once and writes it back
whole after every change, one 376-word transfer each way. A task reads or
writes a file a character at a time through one of four channels
(`K.OPEN`, `K.GETC`, `K.PUTC`, `K.CLOSE`), and there are `K.LOOK`,
`K.DELET` and `K.RENAM` beside them and the raw `K.DREAD` and `K.DWRIT`
underneath; `rexapi.asm` says what each takes and gives. Every one of them
comes through `KENTER` and so runs with the disc claimed, one at a time.

## Modules

A module is a program assembled from word 0 against `rexapi.asm` and
written by `asm703.py --object` as relocatable object text -- the record
format of the 1968 relocating loader (DN 390682C), whose transcript is
`../test/703/listings/`. The assembler's docstring lists the codes. Every
memory reference in a module is a page offset the loader relocates, so a
module must fit one 2048-word page (one 1024-word byte page if it
byte-addresses directly); a data word holding an address is relocated as a
whole; and an absolute address -- a kernel entry, an exported cell -- is
reached only through a page selection in front of it, since the module's
own page is not known until it is placed.

`rexapi.asm` is the whole of what a module may know about the kernel: a
jump vector at words X'10'-X'1F' and X'28'-X'3F', exported cells at
X'20'-X'23', the shape of a task node and of a directory entry, the state
values and the file calls' error values, all as EQUs. A module calls an entry
with an adjacent `SMB`/`JSX` pair and reads a cell with an adjacent
`SMB`/`LDW` pair; nothing is linked at load time.

`hello.asm` is the smallest module that exercises every kind of word the
format carries. `make -C rex reloc-check` assembles it absolute at two
bases, one in each half of a word page, and holds the reference loader's
placement of the object (`../tools/reload703.py`) to those images word for
word -- the check that the assembler's records and the loader's semantics
agree, with no emulator in the loop.

Modules are files of type `RX` on disc unit 0 (`../tools/mkdisc703.py
--add`, which `make -C rex disc` runs for the platter the emulator mounts
from the repo root; `DIR` at the shell lists it). The shell's `LOAD` finds
one by name, runs its text into a block from the kernel's pool -- first fit,
word-granular, with the page containment a module's page-offset M fields
need -- gives it a task node and links it into the ring; `RUN` does the
same and hands it the console until it exits. A task ends through
`K.EXIT`, which gives back every block in the pool tagged with its node,
the module included, and then the node; `MEM` prints the free words,
below the fence and above it, and reads the same before and after. What followed the module's name on
the command line is copied into a block of the task's own, a zero after it,
and the node's `T.ARG` holds its address. `RUN HELLO` prints the address the
loader put it at. `rexlib.asm` is a file a module's deck may put behind
`rexapi.asm`: the printer's routines, the command tail read as file names,
and a file call's error said in words. Tiny BASIC is the real module: `brex.asm` over
`../test/703/bcore.asm` is `basic.obj`, and `RUN BASIC` (or `BASIC`) loads
it, whereupon it takes its workspace -- line buffer, variables, stacks, the
array and a heap, 2,192 words -- as one block from the pool, adds the
block's address to the core's address cells and prints READY; its `BYE`
gives everything back, so the next session starts afresh. Under REX it has
two statements the standalone interpreter has not: `SAVE name` lists the
program down a file channel instead of to the printer, and `OLD name`
empties the program and reads it back in from one, both as `name.BA`. A
name on the command line -- `BASIC PROG` -- is `OLD PROG` and then `RUN`,
and `LOAD BASIC PROG` runs it behind the prompt with no console: what the
program prints is printed, nothing else is, and the task is over when the
program ends or asks for input. A file that is not there is the
interpreter's `HOW?`. The module is 2,047 words of its 2,048-word page.
