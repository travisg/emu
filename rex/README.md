# REX

REX -- Raytheon EXec -- is a round-robin executive for the Raytheon 703,
preemptive and cooperative at once, running on the emulator's invented 60 Hz
line clock. It is new software written for a 1967 machine, not a
transcription, and this directory is its own project: the executive, the
glue that puts Tiny BASIC aboard it as a task, and the scripted session that
tests it.

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

`build/rexapi.asm`, the kernel interface every module deck begins with, is
made there with the kernel (see Modules).

Two things it depends on stay where they are, reached by relative path from
the makefile: the interpreter itself is `../test/703/bcore.asm`, shared with
the standalone Tiny BASIC guest the emulator's own tests boot, and the
assembler is `../tools/asm703.py`, beside `../tools/mkdisc703.py`, which
makes a blank platter and puts a boot sector on it. The emulator's tests
depend on nothing here but the snapshot harness's REX case.

## Build, run, test

    cargo build                                                  # the emulator
    make -C rex bootdisc                                         # rex/build/rex.img, the whole system on a platter
    ./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io   # booted with the LOAD button
    ./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io --serial window  # and a second terminal
    make -C rex test                                             # the scripted session
    make -C rex reloc-check                                      # the relocation check

`--fast-io` makes the teletype instant; without it the Model 33 takes its real
tenth of a second per character, and the scheduling slices are real machine
time either way. The commands the shell has in it:

    HELP or ?    these, and where the programs are listed (DIR 0:)
    STAT         every task's job number, name and state, how long each
                 sleeper has left, and OUT for one that is on the disc
    UPTIME, UP   seconds since the executive came up
    STOP n       suspend a task the shell loaded, by job number
    START n      release one
    KILL n       end one, whatever it was doing
    ECHO text    print the rest of the line
    MEM          the words free in the pool, below word X'4000' and above
    LOAD name    a module off the disc -- name.RX -- run behind the prompt
    RUN  name    the same, given the console until it exits -- the only
                 way to start a module that reads the console
    @name        take the shell's lines from name.CM until it runs out
    HALT         stop the loaded tasks, drain the printers and stop the
                 machine; the console's shell only
    BYE          end this shell; every other terminal's shell only

**The other terminals.** Teletypes 1-3 sit beside the console, each given a
host end by one `--serial` -- `--serial tcp:7031`, and then `nc localhost
7031` -- or a teletype window of its own by `--serial window`. A key
typed at one that has no shell brings it one: the console's shell loads
`SHELL.RX` for it -- `shell.asm`, the console's own shell, assembled again
as a module behind `shellmod.asm` -- and the key goes to that shell, so the
first Return gets a banner naming the terminal and a prompt. Each shell
runs the same commands; a task belongs to the terminal of the shell that
started it, prints there, and is given that console by `RUN`, while `STAT`,
`KILL`, `STOP` and `START` reach every task, with `STAT` marking another
terminal's (`T1`). `BYE` ends a terminal's shell, and the next key there
brings a new one. A shell is about 2,000 words of the pool below the fence,
and an idle one waiting for a key is the swapper's first choice to put out.

Any other word is a program: `BASIC` is `RUN BASIC`, and whatever follows
the name on the line is the program's to read -- `BASIC PROG` loads and
runs the saved program `PROG.BA`. The utilities are programs
on the disc, each a small module from `util/`:

    DIR [unit:]     a platter's directory: name, type, sectors; then what is free
    TYPE name       a file on the printer
    PUT name        the keyboard into a file, until Ctrl-Z
    COPY from to    a file again, under another name
    REN old new
    DEL name
    DUMP name       a file in hexadecimal
    SQUISH [unit:]  close up the holes between the files

A file name is up to six characters and a type of two after a dot --
`NOTE.TX`. A name given to `RUN`, `LOAD` or the shell without a type is a
module, `.RX`; one given to `@` is a command file, `.CM`; one given to a
utility is text, `.TX`. If unit 0 has an `INIT.CM`, the shell reads it
when the machine comes up, before the keyboard.

**Platters.** In front of a name, `0:` to `3:` is the disc unit it is on:
`TYPE 0:MSG`, `COPY NOTE 1:NOTE`, `DIR 0:`. Without one, a program
(`.RX`) is on unit 0, the platter REX boots from, and any other file is on
the *data unit* -- unit 1 if it has a directory, unit 0 if not. So with a
data platter mounted, what is saved goes on it and survives the next
build of `rex.img`:

    rex/fs703.py --dir disks/ray703-disc1.img             # once: an empty platter
    ./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io

Under `-s ray703-load` the `-r` platter is unit 0 and
`disks/ray703-disc1.img` is mounted as unit 1 by its name. `DIR` lists the
data unit, and `DIR 0:` the programs. Without a data platter, everything
is on unit 0, as `rex.img` alone has it. A prefix only ever overrides the
default: nothing is refused on either platter, except a `REN` from one to
the other. Every name is read by one routine in the kernel, `K.NAME`, so
the shell, the utilities and BASIC's `SAVE` and `OLD` all take the same
names.

Ctrl-C is the running program's to act on, and Tiny BASIC does. A second
Ctrl-C while the first is still untaken ends the program that has the
console, which is the way out of one that is not listening.

A task `LOAD` started ticks along behind whatever is typed -- a BASIC
session included, its output interleaving with `PRINT`'s on the one
printer. The Model 33 echoes what is typed in hardware here
(`DOT 14,11`), so typing interleaves with the tasks' output character by
character, the way two users on one printer did. Unlike standalone Tiny
BASIC it has type-ahead: input goes through a queue, so a burst typed while
it is busy is held and run in order, and only a burst deeper than the queue
is lost.

## What it is

Two tasks share the processor at power-on, their control blocks a ring
of linked nodes, and whatever the shell loads joins them: everything that
names a task -- the current-task cell, the printer's owner, a queue's
waiter -- holds a node's address, a field is the indexed displacement off
it, every scan walks the `T.NXT` links, and adding a task is linking a
node in under `MSK`, which is what the loader does through `SPAWN`. One
is the shell, and one the swapper, asleep until more is loaded than core
holds; the machine comes up quiet, with nothing else running. Tiny BASIC
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
self -- has the processor, which with three tasks printing and sleeping
behind the prompt is ~98% of the time, sampled through the debug port, and
the same ~98% whether or not `--fast-io` is on, which is the sign that
nothing is spinning on I/O.

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
reader. **There are four terminals**, the console's teletype and teletypes
1-3 beside it (DIO devices 12, 11 and 10, on the console's interrupt
level): each has a block in the kernel's terminal table, the service
routine asks each teletype's status word in turn to find which
interrupted, and every task has its terminal (`T.TTY`), where what it
prints goes and whose console it may be given. And **a console has one
reader at a time**, named by its
terminal's `TM.BSY` cell: `RUN` raises it in the masked window that clears
the break flag, then waits on it -- reading no queue -- until the task's
`K.EXIT` clears it and wakes the shell. That is also why **only a task `RUN` started may
read the console**: `LOAD` grants nothing and leaves the shell in `Q.GET`,
so a module that read the queue behind the prompt would put its own
registration in the queue's one waiter slot on top of the shell's. A
module that needs the console tests its node's `T.CON` at entry and
says so if it is clear, which is what `brex.asm` does when it has no
program to run instead; `hello.asm` only prints, which is what `LOAD` is
for. Ctrl-C never enters the queue at all: the service routine raises the
terminal's break flag instead, so a running program that reads nothing can
still be broken. **The break is the console's**: BASIC's check between
statements goes through the wrapper, which asks the kernel (`K.BRKQ`)
only while its task has the console, so a BASIC running behind the prompt
neither stops for a Ctrl-C nor takes one that was meant for the program
in front; `TYPE` and `DUMP` ask through `rexlib.asm`'s `L.BRK`, which
answers the same way. A break belongs to a run: the
interpreter spends the flag at its `READY` loop, once the line is in, so
one typed while the prompt was up is not kept for whatever runs next.
`STOP`/`START` take a loaded task by its job number, never the shell --
which has none -- and never the task that has the console -- stopping
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
it may be (`fs703.py --add-high`): everything here but BASIC, whose
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
(`../tools/emudbg.py`): boot, `STAT`, three copies of `blip.asm` loaded
behind the prompt, each printing a mark of its own at an interval of its
own, watched until they have interleaved and then killed, the rest of the
commands, hello both ways with `MEM` reading the
same after each, then two BASIC sessions -- a program entered and RUN, a
silent `GOTO` loop broken with Ctrl-C through the kernel's flag, the
program saved, `BYE` handing the console and the memory back, and a second
session starting afresh, with nothing to list until `OLD` brings the
program back -- then a host's program run by name, one run behind the
prompt until it ends, and one that never ends left running behind the
prompt through a Ctrl-C and then killed -- and `HALT`, typed with a task
still loaded, whose node has to say stopped once the machine has. The saved programs
are among the files the host reads back afterwards. In between it runs `fst.asm` twice, which makes,
reads back, replaces, renames and deletes files through the vector and
checks every answer; afterwards the harness reads the platter the host's
way (`fs703.py`): the directory has to pass the check of its
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
count of tasks that found the disc taken has to be more than zero. On the
same machine, where printing takes its time, `DUMP` then prints a file
behind the prompt while a program that ignores Ctrl-C has the console and
the key is struck: it has to reach the file's last line.

A third machine is the swapper's, on a platter with a swap file. Four
copies of `hog.asm` each take 8,000 words, which is more than there is,
fill them with a pattern of their own and check it thirty times over ten
seconds. The first two are stopped before the other two are loaded, so
they are the ones put out to make room, and `STAT` has to show them out
and stopped; then they are started again, and all four have to finish
with their blocks whole -- each having come back to the same words, which
another hog had in between -- and as many swaps in as out. Then four Tiny
BASICs want the core below X'4000', which holds two: three run a program
behind the prompt that counts and then saves itself under another name,
one is typed at in front of them, and the saved listings are read back on
the host. A fourth machine is given no core image, only a platter and the
LOAD button, and has to come up, list its directory and run a program. A
fifth has a platter with two sectors free: a `SAVE` too long for them
under a saved program's name has to be refused with the program left as
it was, a `COPY` onto the swap file's name refused outright, and the
host's tools have to refuse the same of a system file. A sixth has two
more teletypes on localhost sockets: a key at each brings it a shell, a
blip behind each prompt prints on its own terminal's paper alone, a second
Ctrl-C there ends only its own program, `BYE` gives the pool back, and a
terminal's shell waiting for a key is put out and comes back for one. A
seventh has a data platter on unit 1 made with `fs703.py --dir`: a
file with no unit lands there and a program still comes from unit 0,
`COPY` goes both ways, a `REN` across them is refused, `DIR` and `DIR 0:`
list their own platters, `plat.asm` writes a file on unit 1 across a look
at unit 0 -- the close has to find the write it began in unit 1's
directory, brought back in -- a program copied there runs as `1:HI`, and
BASIC saves to unit 1 and runs from it; the host then reads both platters
back, unit 0 as it was made but for the one file copied onto it.
The main session runs `--fast-io` (the slices stay real machine time; the clock ignores
the flag), walks the paper with a cursor so each wait finds the next prompt
rather than the first, and paces every command on that prompt and then on
the printer falling quiet, which is the rule for driving any 703 guest from
a script. Every wait raises on its timeout, and `REX_CONTROL=blank`,
`=nohello`, `=nolock`, `=nosave`, `=noguard`, `=noswapin`, `=nosys`,
`=nopark`, `=nolatch`, `=sysopen`, `=noseg`, `=onlycon`, `=onetty`,
`=nogetty`, `=nounit`, `=dirunit`, `=nodata` and `=fcwho` are its negative
controls --
a platter with nothing on it, one
without the hello module, a kernel with its wait for the disc patched out,
one that never writes the directory back, one that lets any task read the
console, one that cuts a returning task's blocks out of the free list but
never reads them back, a boot sector never told where the kernel is, a
`HALT` that stops no task, a channel that forgets it refused a character,
a file opened for writing under a system file's name, a loader that
adds a segment's own base to an address in another, a teletypes' service
routine that asks only the console's status, a printer started on any
terminal's task, a key that asks for no shell, a disc driver deaf to the
unit, a directory entry read from whichever platter's directory is in
core, no data unit but unit 0, and a channel's unit forgotten when the
directory is asked who is writing -- which must all FAIL. Two things that make it hard
to verify by hand: under `--fast-io` the keyboard has no rate limit at all,
so a burst of input outruns any consumer and proves nothing about the
scheduling -- measure a switch in a `--trace` instead; and a number printed
at ten characters a second has to be read as a whole field, not matched on
its first digit.

## Booting

`make -C rex bootdisc` makes the platter REX runs from, and `-s
ray703-load` presses the disc controller's LOAD button on it: the button reads sector 0 into words 0-46 and starts
there, and `boot.asm` is what it finds.

The kernel's image begins at word 0, so the boot sector cannot stay where
it was put. Its first act is to have the controller read the same sector
again, to word X'6000' -- in what will be the pool, nobody's until the
kernel lays it out -- and carry on in the copy. Nothing in it is assembled
for the second address and nothing needs to be: both are the first word of
a page, and every reference is an offset within the page it is executed
in. Then it reads the kernel, which is two runs of words with the whole
pool between them and is kept as two: `REX.SY` holds the low part and,
from the next sector, the high part, and `fs703.py --sys`, which makes
that file from `rex.bin`, writes where each part is, how long, and where
it goes into the six words at the end of the boot sector. Each transfer is
polled to its end, there being nothing enabled yet to interrupt; then a
jump to the kernel's first instruction. `ray703-panel-load` does the same
from the front panel, starting halted like any panel machine.

## Swapping

When more is loaded than core holds, a task that is not running goes out
to the disc and comes back when it can run again -- what TSS/8 did for its
users, and what OS/8 never needed, having one. `rexswap.asm` is the whole
of it, and its header is the reference.

**A task cannot be moved**: its module was relocated once, when it was
loaded, and has addresses of its own all through it. So what goes out is
every block in the pool the task owns, each written from where it lies,
and what comes back is the same blocks cut out of the free list at the
same addresses -- whoever is in the way by then going out first. The
task's node never moves, and everything that wakes a task touches only
its node, so a task may be put out in any state: asleep, stopped, waiting
on the keyboard or the printer, or runnable. The scheduler passes over one
whose blocks are not in core, and wakes the swapper if it could otherwise
run.

The swapper is a task, the fifth on the ring, asleep until an allocation
finds nothing that fits -- the caller then waits in the allocator with its
request in its node, where it used to be told no -- or until a task that
is out can run. It puts out a task that cannot run for choice. One that
can run goes only once it has been in core through the second it arrived
in and the next, which is what stops two tasks that want the same core
from spending all their time changing places; if the only candidates are
that new, the asker waits for one to age rather than fail. A task that is
itself waiting for core, or has the disc, is never put out.

On the disc a task is one extent of `SWAP.SY`, a system file the platter is
made with (`fs703.py --swap N`): a sector listing its blocks and then
each block. With no `SWAP.SY` there is no swapping, and an allocation that
finds no room fails as it always did. `STAT` marks a task that is out and
counts how many have gone out and come in.

Two things had to be true of the allocator first. **Every block is an even
number of words**, so every boundary in the pool is on an even word and
what is left either side of a block cut back out of the free list is
nothing or room for a header. And **nothing is compared against where a
block would end**: a start above X'4000' plus a size of several thousand
words passes X'7FFF', which is minus to a machine whose compares are
signed -- an 8,000-word request was once given a 3,280-word block that
way -- so the room in a free block is compared with the size instead.

## The filesystem

A platter holds files: contiguous runs of sectors under a directory in
sectors 1-8, the way OS/8 and RT-11 kept theirs. `fs703.py` states
the format word for word and is the host's side of it; `rexfs.asm` is the
kernel's. An entry is eight words -- a name of six characters and a type
of two, the first sector, the sector count, the bytes used in the last
sector, and what the entry is -- and there are forty-six of them. They are
kept in disc order and **account for every data sector between them, a
hole being an entry like any other**, so there is no free map and nothing
to get out of step with the directory: a new file is given half of the
largest hole as a file being written -- half, as RT-11 gave it, so that a
second file can be begun before the first is closed -- and closing it cuts
it to its length and makes the rest a hole again. A new file replaces an older one of its
name when it is closed, not before, and only if all of it went in: a
channel that has refused a character -- the file filled its half of the
hole -- refuses the rest, and closing it gives the sectors back and
answers with the refusal, the older file still there. A system file --
the kernel's, the swap file -- is not deleted, renamed or written over,
by the kernel's calls or by the host's tools short of `fs703.py --sys`.
Names are upper case: the shell folds what it is typed, and BASIC folds
the name it is given. **A file being written that no channel
holds is a hole** to whoever looks next -- its task exited, or the machine
stopped -- which is all the recovery there is and all that is needed.

The kernel keeps one platter's directory in core, reads it when a call is
for another unit and writes it back whole after every change, one
376-word transfer each way. A name in a call is five words: the four the
directory holds and one whose high byte is the unit, `X'80'` for none.
A task reads or writes a file a character at a time through one of four
channels (`K.OPEN`, `K.GETC`, `K.PUTC`, `K.CLOSE`), each of which keeps
its unit, and there are `K.LOOK`, `K.DELET`, `K.RENAM`, `K.SQUSH` and
`K.DIRNT` (a directory entry, copied out, which is how `DIR` lists one
an entry at a time) beside them and the raw `K.DREAD` and `K.DWRIT`
underneath, which are unit 0's; the vector in `rex.asm` says what each takes and gives. Every one of them
comes through `KENTER` and so runs with the disc claimed, one at a time.

## Modules

A module is a program assembled from word 0 against `build/rexapi.asm` and
written by `asm703.py --object` as relocatable object text -- the record
format of the 1968 relocating loader (DN 390682C), whose transcript is
`../test/703/listings/`. The assembler's docstring lists the codes. Every
memory reference in a module is a page offset the loader relocates, so a
module must fit one 2048-word page (one 1024-word byte page if it
byte-addresses directly); a data word holding an address is relocated as a
whole; and an absolute address -- a kernel entry, an exported cell -- is
reached only through a page selection in front of it, since the module's
own page is not known until it is placed.

A module of more than a page is several segments, a `SEG` card in the
source between one and the next -- up to four. Each is a page's worth at
most, assembled from a word 0 of its own, and the loader gives each a block
of its own wherever one fits, so one segment reaches into another as it
reaches the kernel: through a page selection in front of the reference
(`CALL` for a routine), or through an address in a cell. The assembler
refuses a reference that crosses without one. The object text says each
segment's size first, and two codes past the 1968 loader's own say whose
text follows and whose base a word takes.

`build/rexapi.asm` is the whole of what a module may know about the kernel: a
jump vector at words X'10'-X'1F' and X'28'-X'3F', exported cells at
X'20'-X'23', the shape of a task node and of a directory entry, the state
values and the file calls' error values, all as EQUs. A module calls an entry
with an adjacent `SMB`/`JSX` pair and reads a cell with an adjacent
`SMB`/`LDW` pair; nothing is linked at load time. The file is the kernel's
own statement of itself: `rex.asm` lists the interface's names on `NTRY`
cards, and assembling the kernel writes each out at the value it has in
that build, with the comments it has there (`asm703.py --exports`). So the
interface is described once, in `rex.asm`, a module cannot be assembled
against a layout the kernel does not have, and a change to the kernel
assembles every module again.

`hello.asm` is the smallest module that exercises every kind of word the
format carries. `make -C rex reloc-check` assembles it absolute at two
bases, one in each half of a word page, and holds the reference loader's
placement of the object (`../tools/reload703.py`) to those images word for
word -- the check that the assembler's records and the loader's semantics
agree, with no emulator in the loop. `far.asm` gets the same with a base
for each of its two segments, and the scripted session runs it: each half
is over half a page, so the loader cannot put both in one.

Modules are files of type `RX`, on unit 0 unless a name says otherwise
(`fs703.py --add`, which `make -C rex bootdisc` runs; `DIR
0:` lists them). The shell's `LOAD` finds
one by name, runs its text into a block from the kernel's pool -- first fit,
word-granular, with the page containment a module's page-offset M fields
need -- gives it a task node and links it into the ring; `RUN` does the
same and hands it the console until it exits. A task ends through
`K.EXIT`, which gives back every block in the pool tagged with its node,
the module included, and then the node; `MEM` prints the free words,
below the fence and above it, and reads the same before and after. What followed the module's name on
the command line is copied into a block of the task's own, a zero after it,
and the node's `T.ARG` holds its address. `RUN HELLO` prints the address the
loader put it at. `rexmac.asm` is the macros a deck is written with --
the conditional jumps that say which way a skip and a jump go, `CALL` for
the kernel's vector, `MSG` for a text and its descriptor -- and
`rexlib.asm` is a library a module's deck may put behind it and
`rexapi.asm`: the printer's routines, the command tail read as file names,
and a file call's error said in words, each a part under a `LIBR` card
that is assembled into the module only if the module, or a part it has
taken, names it -- so a module is its own code and what of the library it
uses, and nothing is shared at run time. Tiny BASIC is the real module: `brex.asm` over
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
interpreter's `HOW?`, and so is a `SAVE` the disc has no room for. The
module is 2,045 words of its 2,048-word page.
