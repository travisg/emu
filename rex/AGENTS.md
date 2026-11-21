# REX: the reference

How REX is made, for whoever changes it: the design, the rules the kernel
and every module live by, what the scripted session checks, and the
formats on a platter and in a module. `README.md` beside this file is the
user's side -- building and booting it, the shell's commands, file names
and platters, the map of this directory -- and is where to look for
those. Each source file's header is the reference for its own part:
`rex.asm` for the rules and the vector, `rexfs.asm`, `rexswap.asm`, and
`fs703.py` for the platter's format.

Two things it depends on stay where they are, reached by relative path from
the makefile: the interpreter itself is `../test/703/bcore.asm`, shared with
the standalone Tiny BASIC guest the emulator's own tests boot, and the
assembler is `../tools/asm703.py`, beside `../tools/mkdisc703.py`, which
makes a blank platter and puts a boot sector on it. The emulator's tests
depend on nothing here but the snapshot harness's REX case.

## Working on it

- **The build stops on a warning.** The makefile assembles everything
  with `asm703.py --werror`, and every module's rule depends on
  `build/rexapi.asm`, so a change to the kernel assembles them all again.
- **Where things are in core** is the memory map in `rex.asm`'s header.
  In short: the interrupt blocks and the kernel's vector at the bottom,
  the kernel in page 0 from X'40', the console's shell in page 1 from
  X'800', the pool from X'1000' to X'6FFF' with its fence at X'4000', the
  filesystem from X'7000', the swapper, `K.NAME` and the loader's record
  reader from X'7800', and
  the node arena from X'7E00'.
- **The pages are nearly full.** Page 0 has 44 words left, the console's
  shell 1 before X'1000', where the pool begins -- its line buffer and
  the last of HELP's lines live in page 0 for want of room -- and the top of core 20
  in front of the node arena; `build/rex.lst` shows where each ends. Running over either
  fails the build -- "assembled twice" at the shell's `ORG` for page 0, a
  `RES` below zero at the end of `shell.asm` for page 1. BASIC's module
  has 88 words of its one page left (see Modules).
- **`shell.asm` is assembled twice** and a change has to work both ways.
  In the kernel's deck (`SHMOD=0`) it is the console's shell in page 1:
  it reads `INIT.CM`, gives the other terminals their shells and has
  `HALT`. Behind `shellmod.asm` (`SHMOD=1`) it is the module `SHELL.RX`,
  a copy loaded for each other terminal: it has `BYE` instead, names its
  node and terminal only through `SHME` and `SHTM`, and reaches the
  kernel only through the exported names. It keeps byte addresses, so it
  is loaded below the fence.
- **The session** is `make -C rex test`, which builds the kernel and the
  modules and makes its own platters, so it does not need or remake
  `build/rex.img`. `EMU_BIN` names another emulator binary, the
  transcript is `build/rex_test.log`, and one negative control is run as
  `REX_CONTROL=nolock make -C rex test`, which must FAIL.
- **A stuck kernel** is looked at through the debug port, with the
  kernel's listing for names:
  `../tools/emudbg.py --symbols rex/build/rex.lst SOCK` gives `break`,
  `dis` and `watch` by label. The root `AGENTS.md` describes the port.

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
break taken from the kernel's flag, which is the console's holder's, `SAVE`
and `OLD` through the file calls, `BYE` routed to `K.EXIT`. The idle
task is a third node, off the ring, the scans' explicit fallback.

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
nothing is spinning on I/O. A tick that finds idle running and nothing
woken since the last pick returns at once, and counts itself, which is
the `IDLE` line of `STAT`.

## The rules

Each is stated once in `rex.asm`'s header, which is the reference; the
machine facts they rest on -- the interrupt blocks, EXR, the entry sequence
that does not reload it -- are in the 703 section of `../AGENTS.md`.

Interrupts and the switch:

- The scheduler **defers** whenever the saved PC lies inside the range
  holding the service routines and the switch: a tick there either
  interrupted the driver or found a switch half made, and both want the
  same answer.
- Every service routine **leads with `SMB`**, because the entry sequence
  does not reload EXR.
- **`SWTCH` is for tasks** -- a service routine that called it would walk
  away from its own `INR` and leave its level Active for good, which
  silently holds off every level at or below it.

Terminals and the console:

- **Input goes through a queue**: the service routine posts a character
  and wakes the waiter -- only out of its wait, so a keystroke cannot
  restart a stopped task -- the reader blocks in `Q.GET` rather than
  polling, and one waiter to a queue means one reader.
- **There are four terminals**, the console's teletype and teletypes 1-3
  on the Teletype Multiplexer's channels 0-2, sharing the console's
  interrupt level: each has a block in the kernel's terminal table, the
  service routine asks the teletype's status and then the multiplexer's,
  taking the stopped channel's word for as long as its scanner stops on
  one, and every task has its terminal (`T.TTY`), where what it prints
  goes and whose console it may be given. A multiplexer channel goes one
  way at a time: the driver selects it for output while a task has a
  character for it and for input again when none is left, so keys typed
  while one prints wait in its line -- all but a Ctrl-C, which is the
  set's BREAK there and raises the break flag like any other.
- **A console has one reader at a time**, named by its terminal's
  `TM.BSY` cell and the holder's `T.CON`: `RUN` or `ATTACH` raises both
  in the masked window that clears the break flag, then the shell waits
  on the cell -- reading no queue -- until it comes down: the task's
  `K.EXIT`, a `KILL` from any terminal, both of which hand it back
  through `CONBK` and wake the shell, or the shell's own Ctrl-Z.
- **The right is live, and a reader without it parks.** `Q.GET` looks
  at it every time round its wait, not once at entry: a terminal's queue
  answers its shell and the console's holder, and any other task of the
  terminal's marks itself `SCON` and waits for `ATTACH`, which is the
  only thing that wakes it -- so a module that reads the console behind
  the prompt never puts its registration in the queue's one waiter slot
  on top of the shell's, and needs no test of its own. `STAT` shows such
  a task as `TTY`.
- **`LOAD` loads stopped.** It makes the node `SOFF` under the mask it
  calls `SPAWN` with, so the task is never on the ring in any other
  state, and prints its job number; `START n` lets it run behind the
  prompt and `START name` loads and runs in one step. `SPAWN` leaves the
  state to its caller for that reason.
- **Ctrl-C never enters the queue** at all: the service routine raises
  the terminal's break flag instead, so a running program that reads
  nothing can still be broken. **Ctrl-Z does not either**: it raises
  `TM.KIL` to 2, if it is down, and wakes the shell, which -- with the
  disc claimed, as `STOP` claims it -- stops the holder, takes its
  `T.CON` away and lowers `TM.BSY`.
- **The break is the console's holder's**, by the kernel's own test:
  `K.BRKQ` and `K.BRKC` answer zero, and spend nothing, for a task
  without `T.CON`, so a BASIC running behind the prompt neither stops for
  a Ctrl-C nor takes one that was meant for the program in front, and
  `brex.asm`'s hooks and `rexlib.asm`'s `L.BRK` are the kernel's entries
  and nothing more. A break belongs to a run: the interpreter spends the
  flag at its `READY` loop, once the line is in, so one typed while the
  prompt was up is not kept for whatever runs next.
- **`STOP`/`START` take a loaded task by its job number**, never the
  shell -- which has none -- and never a task that has a console --
  stopping another terminal's console's holder would leave its shell
  waiting on a hand-back nobody can make. `ATTACH` takes only a task of
  the shell's own terminal, since a task prints on and reads its
  `T.TTY`. On teletypes 1-3 a channel that is printing passes only
  BREAK, so a Ctrl-Z reaches the program there when its printing pauses.

Tasks and the pool:

- **A task is ended by one routine however it ends** -- `REAP`, from its
  own `K.EXIT` or from the shell's `KILL` -- and the shell claims the disc
  before it ends another task, so the task is inside no transfer and no
  file call when it goes.
- **The pool is owned word by word**: every block carries the node that
  asked for it, and a task's exit gives back all of them, which is what
  makes a module's memory come and go with the module. The node itself is
  not in the pool: a loaded task's comes from an arena of twenty-four at
  the top of core, and stays put for as long as the task lives.
- The pool runs from the end of the shell to the filesystem's page, words
  X'1000' to X'6FFF', and **it is in two parts, because a byte address
  above word X'4000' is negative** and this machine compares signed.
  Reaching a byte up there through the index register works like anywhere
  else; what goes wrong is comparing two byte addresses, which is how most
  code walks a buffer to its end. So a fence stands at X'4000' -- a block
  of no words that is never free, so nothing is ever joined across it --
  and what a task asks for comes from below it unless the task asks
  otherwise (`K.ALLHI`, `K.ALHIW`), which is a promise to compare no byte
  addresses in what it gets. A module is loaded above the fence only if
  its directory entry says it may be (`fs703.py --add-high`): everything
  here but BASIC, whose interpreter walks its own tables and its workspace
  by byte address, and `SHELL.RX`, which keeps byte addresses too, so
  those two have the core below the fence to themselves.

Many tasks in one kernel:

- Any number of tasks may be in the kernel at once, on a machine whose
  subroutines each have one static slot for the return address. So **a
  routine any task may call holds the mask from its first instruction**
  -- the allocator -- and one that blocks keeps what it needs afterwards
  in the caller's node rather than in cells of its own, which is `Q.GET`
  and `KENTER`.
- **The disc has one user at a time**, named by `IOLOCK`: a task's
  `K.DREAD` or `K.DWRIT` waits its turn in `KENTER`, and the shell's
  loader makes the same claim, so the driver behind the claim is written
  like any single-task routine. A claim that finds it taken waits in
  `IOWT`, marked waiting with `T.DLY` at X'FFFF', and `IOREL` -- the one
  way the disc is given up -- wakes every task so marked; `KENTER` then
  hands the processor on once, so the waiter gets the disc before the
  releaser can take it again. The swapper claims
  it like anybody, with one exception: a shell's loader holds the disc
  for a whole load and may be the task waiting for core, and a holder
  that is waiting for core is not using the disc, so the swapper uses it
  then without the claim -- for that one task's request only -- and wakes
  that task last, when the disc is quiet again (`SWBOR`, and `rexswap.asm`'s
  header).

## The test

`make -C rex test` runs `run_rex_test.py` on a platter it makes in a scratch
directory, driving the emulator over its debug port (`../tools/emudbg.py`).
Its module doc says the same of each machine at more length. There are
seven:

- **The main session.** Boot, `STAT`, three copies of `blip.asm` started
  behind the prompt, each printing a mark of its own at an interval of its
  own, watched until they have interleaved and then killed; a fourth
  `LOAD`ed, which prints nothing until `START`, then `ATTACH`ed and stopped
  with Ctrl-Z; the rest of the
  commands, hello both ways with `MEM` reading the same after each, then two
  BASIC sessions -- a program entered and RUN, a silent `GOTO` loop broken
  with Ctrl-C through the kernel's flag, the program saved, `BYE` handing
  the console and the memory back, and a second session starting afresh,
  with nothing to list until `OLD` brings the program back, and a loop that
  prints stopped with Ctrl-Z, started behind the prompt, where a Ctrl-C at
  the shell does not break it, and attached again for one that does --
  then a host's program run by name, one run behind the prompt until it
  ends, one that never ends left running behind the prompt through a
  Ctrl-C and then killed, and one with no program, which waits for its
  console until it is attached -- and `HALT`, typed with a task still
  loaded, whose node has to say stopped once the machine has. The saved
  programs are among the files the host reads back afterwards.

  In between it runs `fst.asm` twice, which makes, reads back, replaces,
  renames and deletes files through the vector and checks every answer; afterwards the harness reads the platter the
  host's way (`fs703.py`): the directory has to pass the check of its rules,
  hold exactly the files the exerciser leaves with the bytes it wrote, and
  still hold every module byte for byte, and the platter as it was made has
  to hold none of them.

  The same session drives the shell and the utilities: a program run by its
  bare name, a host's file typed, a file made with
  `PUT` and typed back, copied, renamed, deleted and dumped, a command file,
  `INIT.CM` at start-up, a file deleted from in front of every module and
  `SQUISH` moving the lot down -- after which the next program has to load
  from where it was moved to -- and a task that never ends, and reads the
  console once, started behind the prompt, where it waits for its console
  (`TTY`) while the shell goes on reading, stopped and started by job
  number, attached and given a key, stopped with Ctrl-Z and killed, and
  then, in the foreground, ended by two Ctrl-Cs.

  Where things are loaded is looked at directly, through the port, in the
  pool's own block headers: a module marked for it has to be above word
  X'4000', hello has to print an address up there every time it runs, and
  BASIC's module and workspace both have to be below, and `RUN HELLO`
  has to cost the disc exactly two transfers: its first record, and the
  rest.
- **A second machine**, without `--fast-io` so that a disc transfer takes
  its half revolution, runs three copies of `dskt.asm` at once: each writes
  and reads back a sector of its own 250 times, leaves its verdict on the
  platter where the harness reads it, and the kernel's count of tasks that
  found the disc taken has to be more than zero, `STAT` has to show one
  waiting for the disc (`DSK`), and none of the three may
  be seen asleep in a thousand looks, which is what a claim that polled
  the lock would be. Across `HELP`, which the
  shell then prints at ten characters a second, at most one tick in four
  may find a task running rather than idle -- a character is five ticks
  to the cycle, so the shell's work for each keeps one phase against the
  tick, and at worst every character lands under one. On the same machine, where
  printing takes its time, `DUMP` then prints a file behind the prompt while
  a program that ignores Ctrl-C has the console and the key is struck: it
  has to reach the file's last line.
- **A third machine** is the swapper's, on a platter with a swap file. Four
  copies of `hog.asm` each take 8,000 words, which is more than there is,
  fill them with a pattern of their own and check it twenty times over ten
  seconds. The first two are stopped before the other two are loaded, so
  they are the ones put out to make room, and `STAT` has to show them out
  and stopped; then they are started again, and all four have to finish with
  their blocks whole -- each having come back to the same words, which
  another hog had in between -- and as many swaps in as out. Then four Tiny
  BASICs want the core below X'4000', which holds two: three run a program
  behind the prompt that counts and then saves itself under another name,
  one is typed at in front of them, and the saved listings are read back on
  the host.
- **A fourth machine** is given no core image, only a platter and the LOAD
  button, and has to come up, list its directory and run a program.
- **A fifth machine** has a platter with two sectors free: a `SAVE` too long
  for them under a saved program's name has to be refused with the program
  left as it was, a `COPY` onto the swap file's name refused outright, and
  the host's tools have to refuse the same of a system file.
- **A sixth machine** has the three multiplexer teletypes on localhost
  sockets: a key at each brings it a shell, a blip behind each prompt prints
  on its own terminal's paper alone, `ATTACH` from the console of teletype
  1's job is refused, the console's `STAT` shows teletype 1's shell
  waiting for a key and a program there holding its console, a second
  Ctrl-C there ends only its own program, a
  `KILL` from the console of the program in front of teletype 1 brings
  that terminal's prompt back, a Ctrl-C while one prints -- the set's
  BREAK -- stops a dump,
  `BYE` gives the pool back, and a terminal's shell waiting for a key is put
  out and comes back for one.
- **A seventh machine** has a data platter on unit 1 made with `fs703.py
  --dir`: a file with no unit lands there and a program still comes from
  unit 0, `COPY` goes both ways, a `REN` across them is refused, `DIR` and
  `DIR 0:` list their own platters, `plat.asm` writes a file on unit 1
  across a look at unit 0 -- the close has to find the write it began in
  unit 1's directory, brought back in -- a program copied there runs as
  `1:HI`, and BASIC saves to unit 1 and runs from it; the host then reads
  both platters back, unit 0 as it was made but for the one file copied onto
  it.

The main session runs `--fast-io` (the slices stay real machine time; the
clock ignores the flag), walks the paper with a cursor so each wait finds
the next prompt rather than the first, and paces every command on that
prompt and then on the printer falling quiet, which is the rule for driving
any 703 guest from a script.

Every wait raises on its timeout, and each `REX_CONTROL=name` is a negative
control, a kernel, platter or loader with one thing broken, which must FAIL.
The table of them is in `run_rex_test.py`'s module doc, a name to a line
with what it breaks, and a new one goes there.

Two things that make it hard to verify by hand: under `--fast-io` the
keyboard has no rate limit at all, so a burst of input outruns any consumer
and proves nothing about the scheduling -- measure a switch in a `--trace`
instead; and a number printed at ten characters a second has to be read as a
whole field, not matched on its first digit.

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

The swapper is a task, on the ring from power-on, asleep until an allocation
finds nothing that fits -- the caller then waits in the allocator with its
request in its node, where it used to be told no -- or until a task that
is out can run. It puts out a task that cannot run for choice. One that
can run goes only once it has been in core through the second it arrived
in and the next, which is what stops two tasks that want the same core
from spending all their time changing places; if the only candidates are
that new, the asker waits for one to age rather than fail. A task that is
itself waiting for core, or has the disc, is never put out. A loader that
holds the disc and is waiting for core does not stand in the swapper's
way: the swapper borrows the disc for that request, as the disc's rule
above says.

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
format carries. `make -C rex reloc-check` assembles it absolute at four
bases, one in each half of a word page below word X'4000' and one in each
half above it, and holds the reference loader's placement of the object
(`../tools/reload703.py`) to those images word for word -- the check that
the assembler's records and the loader's semantics agree, with no emulator
in the loop. `far.asm` gets the same with a base for each of its two
segments, once in neighbouring pages and once with the fence between them,
and the scripted session runs it: each half is over half a page, so the
loader cannot put both in one. BASIC is checked at the first word of a
page and the second, and the shell's module at a page's first word and
further in.

Modules are files of type `RX`, on unit 0 unless a name says otherwise
(`fs703.py --add`, which `make -C rex bootdisc` runs; `DIR
0:` lists them). The shell's `LOAD` finds
one by name, runs its text into a block from the kernel's pool -- first fit,
word-granular, with the page containment a module's page-offset M fields
need -- gives it a task node and links it into the ring, stopped; `START`
does the same and lets it run, and `RUN` hands it the console until it
exits. The text comes off the disc through the kernel's record reader,
`K.LDREC` (`rexrec.asm`), one copy for every shell, since a loader holds
the disc for the whole of a load: the first record on its own, so that
its `SIZE` codes take the module's blocks first, then the rest of the
file in one transfer into a block the pool lends it for the load --
anywhere, never waited for (`ALLOCT`), half as big while that fails, and
a sector at a time only if there is no block at all. That block may lie
above word X'4000', so neither the reader nor the shell's walk of a
record compares a byte address: what is left of a record is counted. The
shell keeps the placing and the relocation. A task ends through
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
and `START BASIC PROG` runs it behind the prompt with no console: what the
program prints is printed, nothing else is, an `INPUT` waits for the
console to be given it, and the task is over when the program ends;
`START BASIC` with no program is a session that waits at its first read
until it is attached. A file that is not there is the interpreter's
`HOW?`, and so is a `SAVE` the disc has no room for. The module is 1,960
words of its 2,048-word page.
