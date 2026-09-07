# REX

REX -- Raytheon EXec -- is a round-robin executive for the Raytheon 703,
preemptive and cooperative at once, running on the emulator's invented 60 Hz
line clock. It is new software written for a 1967 machine, not a
transcription, and this directory is its own project: the executive, the
glue that puts Tiny BASIC aboard it as a task, and the scripted session that
tests it.

    rex.asm            the executive: scheduler, drivers, queues, the shell
    rexapi.asm         the kernel interface a module is assembled against
    brex.asm           the wrapper that makes Tiny BASIC a module
    hello.asm          a module: prints where it landed, sleeps, exits
    run_rex_test.sh    the end-to-end test
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
    STAT         every task's state, and how long each sleeper has left
    UPTIME, UP   seconds since the executive came up
    STOP  [A-C]  suspend a letter task, or all three
    START [A-C]  release one, or all three
    ECHO text    print the rest of the line
    MEM          the words free in the pool
    DIR          the disc's catalogue: name, first sector, sectors
    LOAD name    a module off the disc, run behind the prompt
    RUN  name    the same, given the console until it exits -- the only
                 way to start a module that reads the console
    BASIC        RUN BASIC
    HALT         park the tasks, drain the printer and stop the machine

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
its node, so `STOP`, `START`, `STAT` and `HALT` act on whatever the walk
finds and adding a task is linking a node in under `MSK`, which is what
the loader does through `SPAWN`. Three nodes run the one shared letter
body (`LTASK`, which reads its letter, nap and mailbox out of its own node
via `CURT`): print, sleep, repeat -- stopped at power-on, so the machine
comes up quiet and `START` sets them going. One is the shell. Tiny BASIC
is a module behind the `brex.asm` glue, which pays bcore's wrapper debts
with the executive's services: a workspace from the pool at entry, output
through the task's own mailbox, input through the console queue, `T.BRK`
aliased to the kernel's break cell, `BYE` routed to `K.EXIT`. The idle
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
refuses if it is clear, which is what `brex.asm` does; `hello.asm` only
prints, which is what `LOAD` is for. Ctrl-C never enters the queue at
all: the service routine raises the kernel's `BRKREQ` instead (BASIC's
break check reads it through the `T.BRK` alias), so a running program
that reads nothing can still be broken. A break belongs to a run: the
interpreter spends the flag at its `READY` loop, once the line is in, so
one typed while the prompt was up is not kept for whatever runs next.
`STOP`/`START` cannot name the shell or
a loaded task -- only nodes with a letter -- because stopping the console's
owner would leave the shell waiting on a hand-back nobody can make. **The
pool is owned word by word**: every block carries the node that asked for
it, and a task's exit gives back all of them, which is what makes a
module's memory come and go with the module.

## The test

`make -C rex test` runs `run_rex_test.sh` on a platter it makes in a
scratch directory: boot, `STAT`, `START` and watch the letters interleave,
`STOP`, the rest of the commands, hello both ways with `MEM` reading the
same after each, then two BASIC sessions -- a program entered and RUN, a
silent `GOTO` loop broken with Ctrl-C through the kernel's flag, `BYE`
handing the console and the memory back, and a second session starting
afresh -- and `HALT`.
It runs `--fast-io` (the slices stay real machine time; the clock ignores
the flag) with a `-l` instruction-limit hang guard, and paces every command
on the prompt count and then on the printer falling quiet, which is the
rule for driving any 703 guest from a script. Two things that make it hard
to verify by hand: under `--fast-io` the keyboard has no rate limit at all,
so a burst of input outruns any consumer and proves nothing about the
scheduling -- measure a switch in a `--trace` instead; and a number printed
at ten characters a second has to be read as a whole field, not matched on
its first digit.

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
jump vector at words X'10'-X'1F', exported cells at X'20'-X'2F', the shape
of a task node and the state values, all as EQUs. A module calls an entry
with an adjacent `SMB`/`JSX` pair and reads a cell with an adjacent
`SMB`/`LDW` pair; nothing is linked at load time.

`hello.asm` is the smallest module that exercises every kind of word the
format carries. `make -C rex reloc-check` assembles it absolute at two
bases, one in each half of a word page, and holds the reference loader's
placement of the object (`../tools/reload703.py`) to those images word for
word -- the check that the assembler's records and the loader's semantics
agree, with no emulator in the loop.

Modules live on disc unit 0 under a catalogue in sector 1 (`../tools/
mkdisc703.py --add`, which `make -C rex disc` runs for the platter the
emulator mounts from the repo root; `DIR` at the shell lists it). The
shell's `LOAD` finds one by name, runs its text into a block from the kernel's pool -- first fit,
word-granular, with the page containment a module's page-offset M fields
need -- gives it a task node and links it into the ring; `RUN` does the
same and hands it the console until it exits. A task ends through
`K.EXIT`, which gives back every block in the pool tagged with its node,
the module and the node themselves included; `MEM` prints the free words,
and it reads the same before and after. `RUN HELLO` prints the address the
loader put it at. Tiny BASIC is the real module: `brex.asm` over
`../test/703/bcore.asm` is `basic.obj`, and `RUN BASIC` (or `BASIC`) loads
it, whereupon it takes its workspace -- line buffer, variables, stacks, the
array and a heap, 2,192 words -- as one block from the pool, adds the
block's address to the core's address cells and prints READY; its `BYE`
gives everything back, so the next session starts afresh.
