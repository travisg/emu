; vim: ts=8:sw=8:expandtab:
;
; REX -- Raytheon EXec: a round-robin executive for the 703, preemptive
; and cooperative at once.
;
; Two tasks share the processor at power-on, their control blocks a ring
; of linked nodes the scheduler walks, and whatever the shell loads joins
; them; when every task is asleep or waiting the idle task -- a node off
; the ring -- has the processor.  One is a shell, which waits on a queue
; of keystrokes and runs a command, and can load a module off the disc
; as a task of its own, printing through an interrupt-driven teletype
; driver they all share: Tiny BASIC, the interpreter of
; test/703/bcore.asm behind the glue of brex.asm, is one, and RUN BASIC
; hands it the console until its BYE -- or LOAD BASIC PROG runs a saved
; program behind the prompt.  The other is the swapper (rexswap.asm),
; asleep until more is loaded than core holds.
;
; The commands the shell has in it:
;
;   HELP or ?    these, and then the programs the disc has
;   UPTIME, UP   seconds since the executive came up
;   STAT         every task's job number, name and state, how long each
;                sleeper has left, and OUT for one that is on the disc;
;                and, with a swap file, how many have gone out and come in
;   STOP n       suspend a loaded task, by job number
;   START n      release one
;   KILL n       end one
;   ECHO text    print the rest of the line
;   MEM          the words free in the pool
;   DIR          the disc's directory: name, type, sectors; then what is free
;   LOAD name    a module off the disc -- name.RX -- run behind the prompt
;   RUN  name    the same, given the console until it exits
;   @name        the shell's lines from name.CM until it runs out; INIT.CM
;                is read this way when the machine comes up
;   HALT         stop the loaded tasks, drain the printer and stop the
;                machine
;
; and any other word is RUN of that name: TYPE, COPY, DEL and the rest are
; programs on the disc, not commands here.  What follows a program's name
; on the line is its command tail, which the loader leaves in a block of
; the task's own for it to read (T.ARG).  A second Ctrl-C, with the first
; still untaken by the program that has the console, ends that program.
;
; The Model 33 echoes what is typed in hardware, so the keyboard is armed
; for it at start-up (DOT 14,11) and nothing here echoes a character.
;
; The timer is the emulator's invented 60 Hz line clock: DIO device 2,
; interrupt level 2, connected with DOT 2,1 -- the one device on this
; machine that no Raytheon document describes (src/dev/ray703.rs says so
; at length).  Everything else is the hardware the other guests drive.
;
; The machine does the heavy lifting.  There is no stack to switch: the
; interrupt entry saves the program counter at word 4L and the machine
; status -- EXR, the comparison indicators, the addressing mode -- at word
; 4L+2, and INR restores both (3-3).  A task's whole context is therefore
; four words (ACR, IXR and those two), and every switch here is the same
; handful of copies: park the outgoing task's four in its TCB, write the
; incoming task's program counter and status into an interrupt block, load
; its accumulator and index, and INR that block's level.  Because the
; status word travels with it, a task can be preempted between an SMB and
; the reference it governs, or between a compare and its skip, and resume
; none the wiser.
;
; The processor changes hands three ways, and the block each one uses is
; the block its INR names:
;
;   the tick        SCHED, on level 2, takes it from a task that has had
;                   it long enough.  Word 8.
;   a service exit  SERV returns as a task it has just made runnable,
;                   rather than leaving it until the next tick.  Word 0.
;                   DSERV, the disc's, does the same at word 4.
;   a task          SWTCH, called to sleep or to wait on a queue, hands
;                   it on there and then.  Word 12, level 3's block,
;                   which nothing interrupts -- see the rule below.
;
; The rules that keep it sound.  Each is enforced exactly where it is
; stated, and nothing else in the listing may care:
;
; * A SWITCH MADE FROM AN INTERRUPT PARKS THE FRAME IN THAT LEVEL'S
;   BLOCK, so it may only be made when the block holds a task's frame,
;   and the test for that is the saved program counter: outside
;   [ISRBEG, ISREND), the contiguous block holding SCHED, SERV, KICK,
;   PICK, SWTCH, Q.PUT and DSERV, it interrupted a task; inside, it did
;   not.
;   Q.GET is deliberately outside it: a task blocks in there, and a tick
;   that finds one has every business switching away from it.  Every
;   switching path makes it -- the tick at word 8, the teletype's service
;   routine at word 0, the disc's at word 4 -- and each simply returns
;   when it fails.  Inside
;   the range there are two cases and they want the same answer: an
;   interrupt landing in the driver would otherwise strand its INR and
;   park half a driver in a TCB, and one landing on SWTCH's single
;   unmasked instruction would park a half-made switch in the block of
;   the task being switched to.  Tasks execute the rest of the range only
;   under MSK, so no other saved program counter can fall inside it.
;
; * SCHEDULING HAPPENS AT BOTH ENDS.  The tick takes the processor from a
;   task that has had it long enough; a service routine that made a task
;   runnable gives it the processor as it returns, rather than leaving it
;   to wait up to a sixtieth of a second for the next tick.  A character
;   posted to the console queue therefore reaches the shell in the time
;   it takes to return from the interrupt that carried it.  SERV holds
;   the mask across its switch, because the tick outranks level 0 and
;   would otherwise land in the middle of the scan they share.
;
; * THE SMB LEAD.  The entry sequence does not reload EXR, so a service
;   routine's first memory reference resolves in the page of whatever it
;   interrupted.  Every service routine therefore opens with SMB before
;   it touches anything.  (The named core test:
;   interrupt_entry_leaves_exr_for_the_service_routines_first_reference.)
;
; * KICK IS NOT RE-ENTRANT, like every 703 subroutine -- one static link
;   slot -- and is guarded by its call sites: tasks call it only under
;   MSK, and SERV calls it with level 0 active, where the only thing that
;   can preempt is the tick, which defers and touches nothing of KICK's.
;   At most one activation can ever exist.
;
; * MAILBOX OWNERSHIP.  A task writes its own node's T.MBX, one
;   character, only when it holds zero and only inside its masked window;
;   SERV alone clears it, which is the task's "printed" signal; KICK only
;   reads.  Characters are
;   never zero, so zero means empty.  Having deposited, the task waits on
;   that cell the way anything waits here: masked, look; if the character
;   is still there, mark itself WAITING and hand the processor on.  SERV
;   wakes it as it clears the cell, and the task looks again -- so nothing
;   in this executive holds the processor to wait for a device.
;
; * INPUT GOES THROUGH A QUEUE, so the shell waits rather than polls and
;   what is typed while it is busy is held rather than lost.  The service
;   routine posts each character and wakes whoever waits on the queue;
;   the shell blocks in Q.GET until there is one, and empties the queue
;   in the slice it is next given.  One waiter to a queue, so one reader.
;   Putting and taking happen at interrupt level or under the mask, which
;   is what keeps the counts honest, and a full queue drops -- there is
;   no pointer here to store a character through before it is primed,
;   which was the shape of the bug basic.asm had.
;
; * THE CONSOLE HAS ONE READER AT A TIME, and CONBSY says which: the
;   shell when it is clear, the task RUN loaded when it is set.  RUN
;   raises it in the same masked window that clears the break flag, then
;   waits on the cell itself -- reading no queue -- until the task's
;   K.EXIT clears it and wakes the shell under SERV's guard.  Q.GET
;   holds everybody else to it: the console's queue answers the shell
;   and a task whose node says it has the console, and tells any other
;   caller no.  Ctrl-C never enters the queue at all: SERV raises BRKREQ
;   instead, so a running program that reads no input can still be
;   broken, and the grant clears the flag so a break aimed at nobody
;   cannot land on the session that follows it.  A second Ctrl-C that
;   finds the first still there raises KILLRQ and wakes the shell out of
;   its wait, and the shell ends the task that has the console: the way
;   out of a program that is not listening.
;
; * SHUTDOWN ORDERING.  HALT claims the disc, so no task is part way
;   through a transfer, and then stops every loaded task under the mask.
;   A task deposits a character only in a masked window of its own, so
;   one that is stopped has either banked its character or will not
;   deposit it.  Once the shell's drain finds the loaded tasks' mailboxes
;   empty and the printer idle, nothing of theirs can chase the
;   down-message.
;
; * SLEEP IS A STATE, A DELAY AND A SWITCH.  A task stores its delay and
;   marks itself SLEEPING in one masked window and calls SWTCH, which
;   gives the processor to somebody else there and then, without waiting
;   for a tick to come and take it away.  The scheduler
;   passes over a sleeping task, counts its delay down on every tick and
;   marks it runnable again at zero; SWTCH returns when the task is next
;   picked, so the sleep is simply how long the call takes.
;
; * SWTCH IS FOR TASKS.  A service routine that called it would rewrite
;   CURT and walk away from its own INR, leaving its level Active for
;   good -- and a level that never returns holds off every level at or
;   below it, which is the whole interrupt system, silently.  A service
;   routine that wants to reschedule does it the other way about, by
;   rewriting its own level's block and running on to its own INR, which
;   is what SERV does when RESCHD says a task has been woken.
;
; * THE IDLE TASK is what runs when every task is asleep or waiting, and
;   the reason the scheduler's scan can always finish.  It is a branch to self, which
;   is a legal idle here because the levels are enabled and unmasked, and
;   it is scanned by nobody: the scan covers the real tasks and falls back
;   to idle when none of them can run.
;
; * TASKS ARE NODES ON A RING.  Everything that names a task -- CURT,
;   OWNER, LASTS, a queue's waiter -- holds the node's address, a field
;   is the indexed displacement off it, and every scan is a walk of the
;   T.NXT links, counted or bounded by coming back around.  So a node may
;   live anywhere in core, and adding a task is linking a node in under
;   MSK.  SPAWN is that doorway, and the shell's loader goes through
;   it.  The shell does not walk the ring to find a task: STAT prints at
;   the teletype's pace, and a task leaving the ring under it would send
;   the walk astray, so it and the commands that take a job number go
;   through the nodes where they lie.
;
; * A FRESH TASK'S STATUS is GLB plus its entry page.  A zero status word
;   would resume the task in local mode with EXR 0 and its first memory
;   reference would land in page 0.  For an entry on a 1024-word byte
;   page boundary the EXR field is exactly (entry * 2); IDLE is in
;   page 0, where the field is plain zero.
;
; * A ROUTINE ANY TASK MAY CALL HOLDS THE MASK FROM ITS FIRST INSTRUCTION.
;   A subroutine here has one link slot and keeps its work in static
;   cells, so a second task entering while the first is inside would take
;   both.  The allocator's entries and FREE therefore mask before they
;   store anything and load the link back before they unmask.  A routine
;   that blocks cannot hold the mask across its wait, so what it needs
;   afterwards -- the link, the argument -- lives in the caller's node
;   (T.KLK, T.KAR, T.KDS), and the static cells are scratch that is good
;   only until the processor is handed on: Q.GET and KENTER.  Routines
;   only one task runs -- the shell's own -- need none of this.
;
; * THE DISC HAS ONE USER AT A TIME, and IOLOCK names it.  A task's call
;   comes through KENTER, which claims the disc a tick's sleep at a time
;   and gives it up when the body returns; the shell's loader makes the
;   same claim for itself.  Everything behind the claim -- DXFER, and
;   whatever is built on it -- therefore runs one activation at a time,
;   across the wait for a transfer's completion too, and is written like
;   any single-task routine.
;
; * A BYTE ADDRESS ABOVE WORD X'4000' IS NEGATIVE, and this machine's
;   compares are signed.  So the pool is in two parts with a fence
;   between them -- a block of no words that is never free, so nothing
;   is ever joined across it -- and what a task asks for comes from
;   below the fence unless it asks otherwise (K.ALLHI, K.ALHIW), which
;   is a promise to compare no byte addresses in it.  A module is put
;   above the fence only if its directory entry says it may be.
;
; * A TASK'S BLOCKS MAY BE ON THE DISC, AND ITS NODE NEVER IS.  When an
;   allocation finds nothing that fits, the caller's request goes into
;   its node and the swapper is woken (ALWAIT); the swapper writes out
;   every block some other task owns and frees them, and the caller
;   tries again.  A task that is out has T.SWP set, and PICK passes over
;   it -- waking the swapper instead, if the task could otherwise run --
;   until its blocks have been cut out of the free list again exactly
;   where they were (ALLOCAT) and read back.  They have to come back to
;   the same words: a module is relocated once, when it is loaded.
;   Everything that wakes a task touches only its node, so a task may
;   go out in any state.  rexswap.asm's header has the rest: who may be
;   put out, and how the disc is shared with a loader that is waiting
;   for core.
;
; * EVERY BLOCK IS AN EVEN NUMBER OF WORDS, header included, so every
;   block boundary in the pool is on an even word.  That is what makes
;   cutting a block back out of the free list always possible when its
;   words are free: what is left on either side is nothing, or room for
;   a header.
;
; * NOTHING IS COMPARED AGAINST WHERE A BLOCK WOULD END.  A block's
;   start plus a size of several thousand words can pass X'7FFF', which
;   is minus to this machine's compares; the allocator compares the room
;   in a free block with the size instead.
;
; * THE POOL IS OWNED WORD BY WORD.  Core from POOLB to POOLE is a
;   first-fit list of free blocks, every block two words of header in
;   front of its payload: the size, and either the next free block or the
;   node of the task that asked for it, with bit 0 set to tell the two
;   apart.  K.ALLOC takes a size; K.ALLCW and K.ALLCB take one that must
;   lie inside a single word page or byte page, which is what a module
;   needs, its M fields being page offsets.  The list is walked and cut
;   only under MSK and only from task context, so no interrupt ever finds
;   it half linked, and a free block is joined to any free neighbour, so
;   no two free blocks touch.  The owner tag is what lets everything a
;   task holds be found again by a walk of the pool.
;
; * A TASK ENDS THROUGH K.EXIT, and takes everything with it: its last
;   character waited out, the console handed back if it held it, the
;   node unlinked with CURT moved to its predecessor before anything is
;   freed, its channels let go, every block in the pool tagged with the
;   node given back, the node itself returned to its arena, and the
;   processor handed on through the second half of SWTCH with nothing
;   parked.  The loader tags a module's block and its command tail with
;   the node as it makes them, which is what makes the sweep complete.
;   REAP is everything after the console, and the shell's KILL ends
;   another task through the same routine -- having claimed the disc, so
;   the task is in no transfer, stopped it, and waited out a character
;   of its that the printer was on.
;
; * A LOADED TASK'S NODE COMES FROM THE ARENA, not the pool: NARN nodes
;   at the top of core, on a free list.  A node stays where it is for as
;   long as its task lives, whatever becomes of the task's blocks, so it
;   must not sit among them.  Its job number, T.ID, is the node's own
;   and outlasts every task that uses it.
;
; * EVERYTHING RUNS GLOBAL.  START sets it, the TCB statuses carry it,
;   and entry and JSX force it -- EXIT's indexed JSX and every indexed
;   reference here assume a flat address.
;
; Memory map:
;
;   0000-000F  interrupt blocks: level 0 (teletype), level 1 (disc),
;              level 2 (line clock), level 3 (never signalled -- SWTCH
;              stages a switch in it and loads it with INR 3)
;   0010-002F  the kernel interface for what loads under it: the jump
;              vector, then the exported cells (rexapi.asm)
;   0040-      page 0: START; then ISRBEG..ISREND, which is SCHED, SERV,
;              KICK, PICK, SWTCH, Q.PUT and DSERV; then the kernel cells,
;              the console queue, the nodes of the tasks that are here
;              from power-on, the disc -- IOTRY, KENTER, DXFER -- Q.GET,
;              the idle task, the allocator, and the node arena's two
;              doors
;   0800-      page 1: the shell -- banner, prompt, commands, line
;              buffer -- and its loader, with the sector it reads
;   1000-6FFF  the pool, which the allocator hands out: what the shell
;              loads and its workspaces live there.  The fence is the
;              two words below 4000
;   7000-76FF  the filesystem (rexfs.asm, the second file of this deck):
;              the directory's routines, the channels and the bodies of
;              the file calls, then the four channels and the directory
;              itself
;   7800-7BFF  the swapper (rexswap.asm, the third): its task, its table
;              of extents in the swap file, and two sectors of block lists
;   7E00-7FAF  the node arena
;
; Build with make -C rex: asm703.py over this file and rexfs.asm, as one
; deck, into rex/build, and the modules beside it -- brex.asm over test/703/bcore.asm is basic.obj, the
; module RUN BASIC loads.  make -C rex disc puts them on the platter, and
; then, from the repo root:
;
;   ./target/debug/emu -s ray703 -r rex/build/rex.bin --fast-io
;
; make -C rex bootdisc puts the kernel itself on a platter of its own,
; behind boot.asm, and -s ray703-load -r rex/build/rex.img boots that.

; ---------------------------------------------------------------- levels 0-3
                ORG     0
                JMP     START           ; word 0: clobbered by the PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0
                WORD    0               ; word 4: level 1 PCR save
                WORD    DSERV           ; level 1 linkage address (the disc)
                WORD    0               ; word 6: level 1 machine status save
                WORD    0
                WORD    0               ; word 8: level 2 PCR save
                WORD    SCHED           ; level 2 linkage address
                WORD    0               ; word 10: level 2 machine status save
                WORD    0
                WORD    0               ; word 12: SWTCH stages a program
                WORD    0               ; counter and a status here and
                WORD    0               ; loads them with INR 3 -- see the
                WORD    0               ; header. Level 3 is never enabled.

; ---------------------------------------------------------------- the api
; Words X'10'-X'3F' are the blocks of levels 4-15, which nothing on this
; machine signals, so the kernel's interface to what loads under it lives
; there: a jump vector, then the cells a module may read.  rexapi.asm
; states the same addresses as EQUs for the modules, and the build holds
; the two to each other (tools/apicheck703.py over the listing).  A
; module reaches an entry with an SMB/JSX pair: the JSX leaves its link
; in IXR and forces global, and the JMP here carries it into the routine,
; whose SUBR takes the link as from any caller.  An entry with nothing
; behind it is a zero word, so a call to it halts.
                ORG     X'10'
K.ALLOC         JMP     ALLOC           ; ACR = words wanted -> the block, or 0
K.ALLCW         JMP     ALLOCW          ; ...inside one 2048-word page
K.ALLCB         JMP     ALLOCB          ; ...inside one 1024-word byte page
K.FREE          JMP     FREE            ; ACR = a block from K.ALLOC
K.EXIT          JMP     TEXIT           ; the task is over; never returns
K.SWTCH         JMP     SWTCH           ; hand the processor on
K.KICK          JMP     KICK            ; start the printer
K.QGET          JMP     Q.GET           ; ACR = a queue descriptor -> the next word
K.DREAD         JMP     KDREAD          ; ACR = a transfer block -> status
K.DWRIT         JMP     KDWRIT          ; the same, core to disc
K.ALLHI         JMP     ALLOCH          ; K.ALLOC, above X'4000' for choice
K.ALHIW         JMP     ALLCHW          ; K.ALLCW, the same
                WORD    0,0,0,0         ; X'1C'-X'1F'

                ORG     X'20'
CURT            WORD    SHTCB           ; the current task's node: the kernel
                                        ; becomes the shell, so it starts on
                                        ; the shell's own
BRKREQ          WORD    0               ; Ctrl-C arrived; set by SERV, cleared
                                        ; by whoever owns the console
CONBSY          WORD    0               ; the console belongs to BASIC: set by
                                        ; the shell as it grants, cleared by
                                        ; BASIC as it hands back, and the
                                        ; cell the shell waits on meanwhile
KCONSQ          WORD    QCONS           ; the console queue's descriptor

; The vector goes on at X'28': files, each through KENTER to its body in
; rexfs.asm.
                ORG     X'28'
K.LOOK          JMP     KLOOK           ; ACR = a name and four words -> zero, or why not
K.OPEN          JMP     KOPEN           ; ACR = a name and a mode -> a channel, or why not
K.GETC          JMP     KGETC           ; ACR = a channel -> a character, or minus
K.PUTC          JMP     KPUTC           ; ACR = channel*256 + character -> zero, or why not
K.CLOSE         JMP     KCLOSE          ; ACR = a channel -> zero, or why not
K.DELET         JMP     KDELET          ; ACR = a name -> zero, or why not
K.RENAM         JMP     KRENAM          ; ACR = two names, old and new -> zero, or why not
K.SQUSH         JMP     KSQUSH          ; -> zero, or why not: the holes closed up

L0PC            EQU     0               ; the level 0 block words SERV edits
L0ST            EQU     2               ; when it returns as another task
L1PC            EQU     4               ; and level 1's, which DSERV edits
L1ST            EQU     6
L2PC            EQU     8               ; the level 2 block words SCHED edits:
L2ST            EQU     10              ; rewriting them before INR 2 is the switch
L3PC            EQU     12              ; and the level 3 words SWTCH edits,
L3ST            EQU     14              ; for the same reason, before INR 3

; A task control block, one node on a circular linked list.  Everything
; that names a task -- CURT, OWNER, LASTS, a queue's waiter -- names it by
; the node's address, and a field is reached by putting that address in
; the index register and using the field offset as the displacement, so a
; node may live anywhere in core.  The ring is what the scans walk;
; adding a task is linking a node in under MSK.
T.STA           EQU     0               ; state -- offset 0, so the pick
                                        ; scan's test is a bare *T.STA
T.NXT           EQU     1               ; the next node on the ring
T.ACR           EQU     2               ; the four words of context the
T.IXR           EQU     3               ; hardware and the switch move
T.PCR           EQU     4
T.MST           EQU     5               ; machine status: EXR, indicators, mode
T.DLY           EQU     6               ; ticks left, while sleeping
T.MBX           EQU     7               ; the task's printer mailbox, one
                                        ; character; zero means empty
T.NAM           EQU     8               ; two characters, for STAT
T.CON           EQU     9               ; nonzero: this task holds the console
T.ID            EQU     10              ; the job number, fixed for the node;
                                        ; zero for the tasks that are here
                                        ; from power-on, which no command
                                        ; can name
T.KLK           EQU     11              ; a kernel call this task is blocked
T.KAR           EQU     12              ; in: its way back, its argument and
T.KDS           EQU     13              ; its body -- see KENTER
T.ARG           EQU     14              ; the command tail's block, or zero
T.SWP           EQU     15              ; where the task's blocks are: zero,
                                        ; in core; minus one, on their way
                                        ; out or in; else on the disc, one
                                        ; more than the swap extent's sector
T.MW            EQU     16              ; waiting for core: 1 asking, 2 told
                                        ; there is nobody to put out, 3 told
                                        ; to try again
T.RES           EQU     17              ; the second its blocks last arrived
T.LEN           EQU     18              ; words in a node

SRUN           EQU     0
SSLP           EQU     1
SOFF           EQU     2               ; suspended by the shell's STOP
SWAI           EQU     3               ; blocked on a queue
SFREE          EQU     4               ; an arena node nobody is using

; A queue: a ring of words, a count, and the one task waiting on it.
; Words rather than characters because the next thing to go through one
; is a message between tasks.
Q.HEAD          EQU     0               ; where the next one comes from
Q.TAIL          EQU     1               ; where the next one goes
Q.CNT           EQU     2
Q.CAP           EQU     3
Q.BUF           EQU     4               ; word address of the ring
Q.WTR           EQU     5               ; the block waiting, or -1
QW              EQU     6               ; words per descriptor

NRING           EQU     2               ; nodes on the ring at power-on: the
                                        ; shell and the swapper.  Idle is
                                        ; off it, the scans' explicit
                                        ; fallback.

POOLB           EQU     X'1000'         ; the pool: core the allocator hands
POOLE           EQU     X'7000'         ; out, from the shell to the filesystem
POOLH           EQU     X'4000'         ; where a byte address turns negative:
                                        ; the pool's high part starts here,
FENCE           EQU     POOLH-2         ; behind a block that is never free

; A directory entry (rexfs.asm, tools/fs703.py), and the eight words a
; task hands K.LOOK, whose first four it fills in and whose last four the
; kernel does.
DE.NAM          EQU     0               ; six characters, left-justified, zero-filled
DE.TYP          EQU     3               ; two more
DE.SEC          EQU     4               ; the first sector
DE.CNT          EQU     5               ; how many
DE.LST          EQU     6               ; bytes used in the last
DE.FLG          EQU     7               ; what it is, and what is special about it
DE.LEN          EQU     8
DK.HOLE         EQU     1               ; DE.FLG's low byte: a hole,
DK.FILE         EQU     2               ; a file,
DK.TENT         EQU     3               ; a file being written; zero is unused
DF.HIGH         EQU     X'0100'         ; a module that may load above X'4000'
DF.SYS          EQU     X'0200'         ; never moved, renamed or deleted

; What a file call answers when it cannot do what was asked: all minus.
E.NOF           EQU     X'FFFF'         ; no such file; K.GETC's end of file
E.CHN           EQU     X'FFFE'         ; no channel free, or not this task's, or the wrong way
E.DFUL          EQU     X'FFFD'         ; the directory is full
E.FULL          EQU     X'FFFC'         ; no hole, or the file has filled its hole
E.DISC          EQU     X'FFFB'         ; the disc is not ready, or a transfer failed
E.NDIR          EQU     X'FFFA'         ; the platter has no directory
E.PROT          EQU     X'FFF9'         ; a system file
E.EXST          EQU     X'FFF8'         ; the new name is taken

SWAPST          EQU     X'F080'         ; the swapper's first status: global,
                                        ; and the page it starts in, X'7800'
NARB            EQU     X'7E00'         ; the node arena: NARN nodes for what
NARN            EQU     24              ; the shell loads, at the top of core
NARID           EQU     1               ; the first one's job number

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Connect the keyboard and the clock, then *become* the shell: its block
; is left blank and the first tick fills it in.  ENB
; before UNM because a masked signal is held where a disabled one is
; dropped; ENB 2 before the arming DOT so not even the first tick can be
; dropped -- it is 9,523 cycles out, held by the mask until the UNM.  A
; tick that lands on the two instructions after the UNM parks this tail in
; the shell's block, which is exactly right.
START           MSK
                SGM                     ; flat addressing, everywhere, always
                LDW     KPOOLB          ; the pool: a free block below the
                STW     FREHD           ; fence, the fence, and a free
                CAX                     ; block above it
                LDW     KPOOLN
                STW     *0
                LDW     KPOOLH
                STW     *1
                LDX     KFENCE
                CLR
                STW     *0
                LDW     K8000
                STW     *1
                LDX     KPOOLH
                LDW     KPOOHN
                STW     *0
                CLR
                STW     *1
                STW     NFREH           ; the node arena: every node onto
                LDW     KNARN           ; the free list, numbered, from the
                STW     NACNT           ; top down so the first one handed
                LDW     KNARL           ; out is the lowest
STNAR           CAX
                LDW     NFREH
                STW     *T.NXT
                CXA
                STW     NFREH
                LDW     NACNT
                ADD     KNARID
                SUB     K1
                STW     *T.ID
                LDW     KSFREE
                STW     *T.STA
                LDW     NACNT
                SUB     K1
                STW     NACNT
                SAZ                     ; the last of them?
                JMP     STNAR1
                JMP     STNAR2
STNAR1          LDW     NFREH
                SUB     KTLEN
                JMP     STNAR
STNAR2          DOT     14,11           ; connect the keyboard; function 11
                                        ; is the one that echoes, which is
                                        ; the Model 33 printing what its own
                                        ; keyboard sent -- full duplex, and
                                        ; free of the printer's time
                ENB     0
                ENB     1
                ENB     2
                DOT     2,1             ; connect the line clock
                UNM
                SMB     SHELL
                JMP     SHELL           ; the kernel becomes the shell

; ------------------------------------------------------- level 2 service
; The scheduler.  Everything from ISRBEG to ISREND runs at interrupt level
; or under MSK -- the defer check in the header relies on it, so nothing
; else may live between the two labels.
ISRBEG          EQU     $

SCHED           SMB     S2SAVA          ; the SMB lead: EXR still holds the
                STW     S2SAVA          ; interrupted task's page
                STX     S2SAVX

; Count the tick against every sleeping task, and do it before the defer
; check: a tick that caught the teletype's service routine still spent a
; sixtieth of a second, and a sleep that skipped those would stretch by
; however long the driver happened to be busy.  A delay is never stored
; below one, so the count reaches zero exactly and never runs past it.
                LDW     TICKS           ; uptime, for the shell to report
                ADD     K1
                CMW     K60
                SLS                     ; a whole second of them?
                JMP     SCSEC
                STW     TICKS
                JMP     SCTK0
SCSEC           CLR
                STW     TICKS
                LDW     SECS
                ADD     K1
                STW     SECS
SCTK0           LDW     KRING           ; once round the ring
                STW     SCIX
                LDW     KNRING
                STW     SCTRY
SCTKL           LDX     SCIX
                LDW     *T.STA
                CMW     KSLP
                SEQ                     ; asleep?
                JMP     SCTKN
                LDW     *T.DLY
                SUB     K1
                STW     *T.DLY
                SAZ                     ; the last tick of the sleep?
                JMP     SCTKN
                CLR
                STW     *T.STA          ; wake it
SCTKN           LDX     SCIX
                LDW     *T.NXT
                STW     SCIX
                LDW     SCTRY
                SUB     K1
                STW     SCTRY
                SAZ                     ; another node to visit?
                JMP     SCTKL
                JMP     SCDEF

SCDEF           LDW     L2PC            ; where did the tick land?
                CMW     KISRB
                SLS                     ; below ISRBEG: task code
                JMP     SCHI
                JMP     SCSW
SCHI            CMW     KISRE
                SLS                     ; inside [ISRBEG,ISREND): service code
                JMP     SCSW            ; above it: task code
                LDW     S2SAVA          ; defer -- restore untouched and let
                LDX     S2SAVX          ; a later tick do the switch
                INR     2

; A task was running: park its frame, pick the next, resume it.  The
; compare indicators and any overflow this arithmetic sets are clobber
; without consequence -- INR 2 restores the whole status from word 10.
SCSW            LDX     CURT            ; IXR = the current task's node
                LDW     S2SAVA
                STW     *T.ACR
                LDW     S2SAVX
                STW     *T.IXR
                LDW     L2PC
                STW     *T.PCR
                LDW     L2ST
                STW     *T.MST

                JSX     PICK
                LDX     CURT
                LDW     *T.PCR
                STW     L2PC            ; incoming PC and status go into the
                LDW     *T.MST          ; level block; INR does the loading
                STW     L2ST
                LDW     *T.IXR
                STW     S2SAVX          ; park the incoming IXR -- the index
                LDW     *T.ACR          ; register still holds the node
                LDX     S2SAVX
                INR     2

; ------------------------------------------------------- level 0 service
; The teletype.  One line serves both directions, so the routine decides
; what happened by what it started, the period driver's way: a task index
; in OWNER means a character was printing, so this is its completion.  The
; completion path falls into the keyboard check because the two events
; merge into one interrupt when they coincide (the demo's discipline).  A
; keystroke arriving *while* a character prints takes the completion path
; early: the mailbox is cleared and the next DOT queues behind the busy
; printer, which only tells the owner "printed" a tenth of a second soon
; and never reorders anything.
SERV            SMB     S0SAVA          ; the SMB lead again
                STW     S0SAVA
                STX     S0SAVX
                LDW     OWNER
                SAM                     ; no owner: a keystroke should wait
                JMP     STX0
                JMP     SRX
STX0            LDW     OWNER           ; the owner's node
                CAX
                CLR
                STW     *T.MBX          ; the owner's "printed" signal

; ...and wake it, if it is waiting for exactly that.  Only if: a task the
; shell stopped between depositing and this completion must stay stopped,
; and the wake is advice rather than a promise -- the waiter looks at its
; own mailbox again when it runs, and finds it empty either way.
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; waiting on the printer?
                JMP     STX1
                CLR
                STW     *T.STA
                LDW     K1
                STW     RESCHD
STX1            LDW     KM1
                STW     OWNER
                JSX     KICK            ; start the next waiting character
SRX             DIN     14,15           ; collect the frame, and ask for
                SAZ                     ; another; empty is the merge's
                JMP     SRX1            ; other half, not an error
                JMP     SEXIT

; Post the character to the console queue and have done with it.  The
; driver keeps no line: what a line is -- where it ends, what a rubout
; does to it, which case it is in -- is the shell's business, and this
; routine's is to get the character off the teletype.  Nothing is echoed
; here either; the Model 33 is armed to print its own keyboard.
SRX1            STW     QITEM
                CLB     X'83'           ; Ctrl-C is a flag, not a queued
                SNE                     ; character: nothing reads the queue
                JMP     SRXBRK          ; while a program holds the console,
                LDW     KCONSQ          ; so an in-band break could never be
                JSX     Q.PUT           ; seen.  Whoever consumes the flag
                JMP     SEXIT           ; clears it.
SRXBRK          LDW     BRKREQ
                SAZ                     ; one already there that nobody took?
                JMP     SRXKIL
                LDW     K1
                STW     BRKREQ
                JMP     SEXIT
SRXKIL          LDW     K1              ; then this one is for the shell: the
                STW     KILLRQ          ; program is not listening
                LDX     KSHELL
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; only out of a wait, like any wake
                JMP     SEXIT
                CLR
                STW     *T.STA
                LDW     K1
                STW     RESCHD

; Return -- as somebody else, if waking a task made one runnable that was
; not before.  This is the second half of the scheduling: the tick takes
; the processor away from a task that has had it long enough, and this
; gives it to a task that has just been given something to do, without
; waiting up to a sixtieth of a second for the next tick.  Together they
; are why a character posted to the console queue reaches the shell in
; the time it takes to return from the interrupt.
;
; The same test the tick makes, for the same reason: the block holds a
; task's frame only when the saved program counter lies outside the
; range.  Inside it, level 0 interrupted a task that was midway through
; SWTCH, and parking that frame would write a half-made switch into the
; block of the task it was switching to.  Masked from there on, so that
; the tick -- which outranks this level and would otherwise land in the
; middle of PICK -- is held until the UNM, where it defers.
SEXIT           LDW     RESCHD
                SAZ                     ; anything newly runnable?
                JMP     SEXSW
                JMP     SEXPL
SEXSW           CLR
                STW     RESCHD
                MSK
                LDW     L0PC
                CMW     KISRB
                SLS
                JMP     SEXHI
                JMP     SEXDO
SEXHI           CMW     KISRE
                SLS
                JMP     SEXDO
                JMP     SEXPU           ; in the range: not a task's frame
SEXDO           LDX     CURT
                LDW     S0SAVA
                STW     *T.ACR
                LDW     S0SAVX
                STW     *T.IXR
                LDW     L0PC
                STW     *T.PCR
                LDW     L0ST
                STW     *T.MST
                JSX     PICK
                LDX     CURT
                LDW     *T.PCR
                STW     L0PC            ; this level's own block, so the INR
                LDW     *T.MST          ; below returns as the chosen task
                STW     L0ST
                LDW     *T.IXR
                STW     S0SAVX
                LDW     *T.ACR
                LDX     S0SAVX
                UNM
                INR     0
SEXPU           UNM
SEXPL           LDW     S0SAVA
                LDX     S0SAVX
                INR     0

; Start the printer on the next occupied mailbox, round robin from the one
; served last, or leave it idle if every one is empty.  OWNER is claimed
; before the DOT because the completion can arrive on the very next
; instruction (the disc exerciser's flag-before-DOT rule).
KICK            SUBR
                LDW     OWNER
                SAM                     ; still printing? the completion
                JMP     KDONE           ; will call back here
                LDW     KNRING
                STW     KTRY
                LDW     LASTS
KSCN            CAX                     ; the next node round the ring
                LDW     *T.NXT
                CAX
                STW     KCAND
                LDW     *T.MBX          ; that task's mailbox
                SAZ
                JMP     KHIT
                LDW     KTRY            ; empty; any candidates left?
                SUB     K1
                STW     KTRY
                SAZ
                JMP     KNXT
                JMP     KDONE           ; all empty: the printer stays idle
KNXT            LDW     KCAND
                JMP     KSCN
KHIT            LDW     KCAND
                STW     OWNER
                STW     LASTS
                CAX
                LDW     *T.MBX
                DOT     14,14           ; teletype, write the character
KDONE           EXIT    KICK

; Round robin over the ring, starting past the node running now, and
; leave the choice in CURT.  A task that is asleep, waiting or stopped is
; simply skipped; the walk is bounded by a count of the nodes, and if
; none of them can run the idle node -- off the ring, so the walk never
; meets it -- always can.  Shared by the tick and by
; SWTCH, which cannot overlap: a task inside SWTCH holds the mask, and a
; tick that lands in its one unmasked instruction defers before it gets
; here.
PICK            SUBR
                LDW     KNRING
                STW     SCTRY
                LDX     CURT            ; start past the one running now --
                LDW     *T.NXT          ; idle's own link reenters the ring
PKSCN           CAX
                STW     SCIX
                LDW     *T.STA
                SAZ                     ; runnable?
                JMP     PKNRD
                LDW     *T.SWP
                SAZ                     ; and its blocks in core?
                JMP     PKOUT
                JMP     PKPIK
PKOUT           LDX     KSWAP           ; no: that is the swapper's to mend,
                LDW     *T.STA          ; so it is woken -- only out of its
                CMW     KWAIT           ; wait -- and the scan goes on
                SEQ
                JMP     PKNRD
                CLR
                STW     *T.STA
PKNRD           LDW     SCTRY
                SUB     K1
                STW     SCTRY
                SAZ                     ; any candidate left to look at?
                JMP     PKNX2
                LDW     KIDLE           ; nobody can run: go idle
                STW     SCIX
                JMP     PKPIK
PKNX2           LDX     SCIX
                LDW     *T.NXT
                JMP     PKSCN
PKPIK           LDW     SCIX
                STW     CURT
                EXIT    PICK

; Give the processor up now instead of waiting for the tick to take it.
; Called with JSX from task context only -- see the header -- and it does
; not return to its caller the way a subroutine does: it returns when the
; scheduler next picks this task, which is what makes it the whole of a
; sleep or a wait.
;
; The staging is the point.  A switch cannot be built in level 2's own
; block: the tick's entry sequence writes the program counter and status
; there before any instruction of the scheduler runs, so a tick landing in
; the window below would overwrite the context being loaded, and INR 2
; would then return here forever.  Level 3's block is untouched by a level
; 2 entry, level 3 is never enabled, and INR asks nothing of a level
; except that it name a block -- so INR 3 is simply this machine's one
; instruction for loading a program counter and a status word together.
;
; A tick may land on the UNM, which is why this routine sits inside the
; deferred range: the tick defers, returns here, and the INR 3 below then
; loads the context that was staged before the mask came off.  Nothing
; that the tick's bookkeeping touches is read after that UNM.
SWTCH           MSK
                STX     SWRET           ; where the caller resumes
                STW     SWACR           ; and what it had in the accumulator
                LDX     CURT
                LDW     SWACR
                STW     *T.ACR
                LDW     SWRET
                STW     *T.PCR
                STW     *T.IXR          ; resumed through EXIT, which wants
                                        ; the link in the index register
                AND     KPGMSK          ; the status it resumes with: the page
                SLL     1               ; that address lies in, and global.
                ORI     KGLB            ; The indicators are not carried -- a
                STW     *T.MST          ; task yields of its own accord, never
                                        ; between a compare and its skip, and
                                        ; an overflow does not survive a yield
SWRES           JSX     PICK            ; the resume half: TEXIT enters here,
                                        ; masked, with nothing to park
                LDX     CURT
                LDW     *T.PCR
                STW     L3PC
                LDW     *T.MST
                STW     L3ST
                LDW     *T.IXR
                STW     SWIXR
                LDW     *T.ACR
                LDX     SWIXR
                UNM
                INR     3

; Put the word in QITEM into the queue the accumulator addresses, and
; make the task waiting on it runnable if there is one.  Callers must be
; at interrupt level, as the teletype's service routine is, or hold the
; mask: this walks a queue that tasks read under MSK.  A full queue drops
; the word, which is what a teletype does to a line nobody is reading.
Q.PUT           SUBR
                STW     QPD
                CAX
                LDW     *Q.CNT
                CMW     *Q.CAP
                SNE                     ; full?
                JMP     QPX
                LDW     *Q.BUF
                ADD     *Q.TAIL
                CAX
                LDW     QITEM
                STW     *0
                LDX     QPD
                LDW     *Q.TAIL
                ADD     K1
                CMW     *Q.CAP
                SLS
                CLR                     ; round the ring
                STW     *Q.TAIL
                LDW     *Q.CNT
                ADD     K1
                STW     *Q.CNT
                LDW     *Q.WTR          ; anybody asleep on it?
                SAM
                JMP     QPWK
                JMP     QPX
; Wake it only if it is waiting -- the same guard SERV's printer wake
; makes, for the same reason: a keystroke must not restart a task the
; shell has stopped between registering as the waiter and this put.  The
; registration is consumed either way; a stopped task re-registers when
; it next runs, because Q.GET looks again on every wake.
QPWK            CAX                     ; the waiter's node
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; waiting on the queue?
                JMP     QPW2
                CLR
                STW     *T.STA          ; wake it...
                LDW     K1              ; ...and ask the service routine to
                STW     RESCHD          ; return as whoever can run now
QPW2            LDX     QPD             ; forget the waiter -- a queue holds
                LDW     KM1             ; one, which is all a single reader
                STW     *Q.WTR          ; ever needs
QPX             EXIT    Q.PUT

; ------------------------------------------------------- level 1 service
; The disc.  A completion: collect the unit's status, mark the transfer
; over and wake the task that started it -- only out of its wait, SERV's
; guard -- then return as it if the frame underneath is a task's, exactly
; as SERV does, so the reader has the processor the moment its sector is
; in core rather than at the next tick.  The tick outranks this level, so
; the switch is masked, and a tick landing here defers: the range covers
; this routine.
DSERV           SMB     S1SAVA          ; the SMB lead
                STW     S1SAVA
                STX     S1SAVX
                DIN     1,0             ; unit 0's status (5-9.7): zero is clean
                STW     DSTAT
                CLR
                STW     DBUSY
                LDW     DWTR
                SAM                     ; anybody's transfer?
                JMP     DSWK
                JMP     DEXIT
DSWK            CAX
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; waiting on it?
                JMP     DEXIT
                CLR
                STW     *T.STA
                LDW     K1
                STW     RESCHD
DEXIT           LDW     RESCHD
                SAZ
                JMP     DEXSW
                JMP     DEXPL
DEXSW           CLR
                STW     RESCHD
                MSK
                LDW     L1PC
                CMW     KISRB
                SLS
                JMP     DEXHI
                JMP     DEXDO
DEXHI           CMW     KISRE
                SLS
                JMP     DEXDO
                JMP     DEXPU           ; in the range: not a task's frame
DEXDO           LDX     CURT
                LDW     S1SAVA
                STW     *T.ACR
                LDW     S1SAVX
                STW     *T.IXR
                LDW     L1PC
                STW     *T.PCR
                LDW     L1ST
                STW     *T.MST
                JSX     PICK
                LDX     CURT
                LDW     *T.PCR
                STW     L1PC
                LDW     *T.MST
                STW     L1ST
                LDW     *T.IXR
                STW     S1SAVX
                LDW     *T.ACR
                LDX     S1SAVX
                UNM
                INR     1
DEXPU           UNM
DEXPL           LDW     S1SAVA
                LDX     S1SAVX
                INR     1

ISREND          EQU     $

; ---------------------------------------------------------------- kernel data
S0SAVA          WORD    0               ; level 0's register saves
S0SAVX          WORD    0
S1SAVA          WORD    0               ; level 1's
S1SAVX          WORD    0
S2SAVA          WORD    0               ; level 2's register saves
S2SAVX          WORD    0
OWNER           WORD    X'FFFF'         ; node whose character is printing;
                                        ; minus one means the printer is idle
LASTS           WORD    SHTCB           ; last node served, for fairness
KCAND           WORD    0               ; KICK's scan scratch
KTRY            WORD    0
KILLRQ          WORD    0               ; a second Ctrl-C came with the first
                                        ; still untaken: the shell's to act on
RESCHD          WORD    0               ; a wake happened: reschedule at the
                                        ; next service routine exit that is
                                        ; standing on a task's frame
SCIX            WORD    0               ; the scans' walk over the ring
SCTRY           WORD    0               ; candidates left in the scan
SWRET           WORD    0               ; SWTCH's caller: where it resumes...
SWACR           WORD    0               ; ...what it had in the accumulator
SWIXR           WORD    0               ; ...and the index the next task wants
TICKS           WORD    0               ; ticks into the current second...
SECS            WORD    0               ; ...and seconds since REX came up
QITEM           WORD    0               ; what Q.PUT is to put
QPD             WORD    0               ; and the queue it is putting it in
QGD             WORD    0               ; Q.GET's queue...
QGI             WORD    0               ; ...and what it took out
DSTAT           WORD    0               ; the disc's status at its last completion
DBUSY           WORD    0               ; a transfer is in flight
DWTR            WORD    X'FFFF'         ; the node whose transfer it is; -1 = none
IOLOCK          WORD    X'FFFF'         ; the node that has the disc; -1 = free
IOWAITS         WORD    0               ; times a task found it taken
DXOP            WORD    0               ; DXFER's: zero to read, else write...
DXSEC           WORD    0               ; ...the first sector's index...
DXCNT           WORD    0               ; ...how many words...
DXBUF           WORD    0               ; ...and the core they are, a word address
DXARG           WORD    0               ; a task's transfer block
DXTS            WORD    0               ; the sector as track and sector
DXRES           WORD    0               ; and the transfer's answer
KELNK           WORD    0               ; KENTER's scratch: good under the mask
KEARG           WORD    0               ; and no further, like every cell a
KEBDY           WORD    0               ; blocking routine shares
KERES           WORD    0
QGLNK           WORD    0               ; Q.GET's, on the same terms
NFREH           WORD    0               ; the first free node; zero: none
NACNT           WORD    0               ; start-up's count over the arena
K1              WORD    1
K47             WORD    47              ; words in a sector
K3FFF           WORD    X'3FFF'         ; a transfer's word count; the unit is above it
KNARL           WORD    NARB+(NARN-1)*T.LEN
KNARN           WORD    NARN
KNARID          WORD    NARID
KTLEN           WORD    T.LEN
KKDRB           WORD    KDRB            ; the bodies KENTER runs
KKDWB           WORD    KDWB
KKLOOK          WORD    FBLOOK
KKOPEN          WORD    FBOPEN
KKGETC          WORD    FBGETC
KKPUTC          WORD    FBPUTC
KKCLOS          WORD    FBCLOS
KKDELE          WORD    FBDELE
KKRENA          WORD    FBRENA
KKSQSH          WORD    FBSQSH
K7F             WORD    X'7F'
K60             WORD    60
KM1             WORD    X'FFFF'
KWAIT           WORD    SWAI
KSFREE          WORD    SFREE

; The console queue: what the teletype's service routine puts characters
; into and the shell takes them out of.  The service routine fills it at
; interrupt speed and the shell empties it a slice at a time, so its
; depth is how far input may run ahead of the shell being scheduled --
; several typed lines, which is more than a Model 33 can deliver in the
; sixtieth of a second the shell waits to be picked.  Past that it drops,
; the way a teletype drops what nobody is reading.
QCONS           WORD    0,0,0,QCONSN,QCONSB,X'FFFF'
QCONSN          EQU     128
QCONSB          RES     QCONSN
KSLP            WORD    SSLP
KNRING          WORD    NRING
KRING           WORD    SHTCB           ; where a walk of the ring starts
KSWAP           WORD    SWTCB           ; the swapper's node
KIDLE           WORD    IDTCB           ; and where a scan with nothing
                                        ; runnable falls back to
KPGMSK          WORD    X'7C00'         ; the page bits of a word address, which
KGLB            WORD    X'0080'         ; doubled are a status word's EXR field
KISRB           WORD    ISRBEG
KISRE           WORD    ISREND

; The task control nodes that are here from power-on: SH -> SW and round
; again, with idle off the ring and its link re-entering it, which is
; what lets every walk start uniformly at *T.NXT.  The shell's context
; is blank because the kernel becomes the shell and the first tick fills
; it in.  A status is GLB (X'80') plus the entry page in the EXR field:
; IDLE lives in page 0, so its is plain zero; the swapper's is its own
; page's; SPAWN builds one for whatever the loader brings in.  A zero
; status word would resume a task in local mode pointed at page 0.
;
;                    STA    NXT   ACR IXR PCR    MST   DLY MBX NAM  CON
;                    ID KLK KAR KDS ARG SWP MW RES
SHTCB           WORD SRUN, SWTCB,0,  0,  0,     0,     0,  0,  'SH',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0
SWTCB           WORD SRUN, SHTCB,0,  0,  SWAPR, SWAPST,0,  0,  'SW',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0
IDTCB           WORD SRUN, SHTCB,0,  0,  IDLE,  X'80', 0,  0,  'ID',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0

; The disc has one user at a time, and IOLOCK says who.  Everything that
; starts a transfer holds it first -- a task's call through the vector,
; the shell's loader -- so DXFER and whatever is built on it run one
; activation at a time and may keep their state in static cells like any
; other routine here, across the wait for the completion too.  IOTRY is
; the claim, for a caller that holds the mask: zero and the disc is the
; running task's, anything else and it is somebody's already.
IOTRY           SUBR
                LDW     IOLOCK
                SAM                     ; free?
                JMP     IOTBSY
                LDW     CURT
                STW     IOLOCK
                CLR
                EXIT    IOTRY
IOTBSY          LDW     IOWAITS
                ADD     K1
                STW     IOWAITS
                LDW     K1
                EXIT    IOTRY

; A task's call into a routine that needs the disc.  The vector's stub
; arrives here masked with the body's address in ACR and the caller's link
; and argument in KELNK and KEARG.  All three go into the caller's node,
; because the wait for the disc is a sleep and the static cells belong to
; whoever comes through next; then the claim, a tick's sleep at a time
; until it succeeds; then the body, unmasked, with the argument in ACR,
; which returns its answer here; then the disc is given up and the answer
; goes back to the caller.  A task is in at most one such call, so one
; set of node cells serves them all.
KENTER          LDX     CURT
                STW     *T.KDS
                LDW     KELNK
                STW     *T.KLK
                LDW     KEARG
                STW     *T.KAR
KECLM           JSX     IOTRY
                SAZ                     ; ours?
                JMP     KEWAIT
                LDX     CURT
                LDW     *T.KDS
                STW     KEBDY
                LDW     *T.KAR
                LDX     KEBDY
                UNM
                JSX     *0              ; the body
                MSK
                STW     KERES
                LDW     KM1
                STW     IOLOCK
                LDX     CURT
                LDW     *T.KLK
                STW     KELNK
                LDW     KERES
                LDX     KELNK
                UNM
                JSX     *0              ; and home
KEWAIT          LDX     CURT            ; taken: sleep a tick and try again
                LDW     K1
                STW     *T.DLY
                LDW     KSLP
                STW     *T.STA
                JSX     SWTCH
                MSK
                JMP     KECLM

; K.DREAD and K.DWRIT: ACR addresses three words in the caller's own core
; -- the first sector's index, track*128+sector; the word count, which
; may run across sectors and tracks; the core address -- and the answer
; is the controller's status, zero for a clean transfer.
KDREAD          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKDRB
                JMP     KENTER
KDWRIT          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKDWB
                JMP     KENTER

; The file calls: each to its body in rexfs.asm.
KLOOK           MSK
                STX     KELNK
                STW     KEARG
                LDW     KKLOOK
                JMP     KENTER
KOPEN           MSK
                STX     KELNK
                STW     KEARG
                LDW     KKOPEN
                JMP     KENTER
KGETC           MSK
                STX     KELNK
                STW     KEARG
                LDW     KKGETC
                JMP     KENTER
KPUTC           MSK
                STX     KELNK
                STW     KEARG
                LDW     KKPUTC
                JMP     KENTER
KCLOSE          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKCLOS
                JMP     KENTER
KDELET          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKDELE
                JMP     KENTER
KRENAM          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKRENA
                JMP     KENTER

KSQUSH          MSK
                STX     KELNK
                STW     KEARG
                LDW     KKSQSH
                JMP     KENTER

KDRB            SUBR
                STW     DXARG
                CLR
                STW     DXOP
                JSX     KDXP
                EXIT    KDRB
KDWB            SUBR
                STW     DXARG
                LDW     K1
                STW     DXOP
                JSX     KDXP
                EXIT    KDWB
KDXP            SUBR
                LDX     DXARG           ; the caller's three words
                LDW     *0
                STW     DXSEC
                LDW     *1
                AND     K3FFF
                STW     DXCNT
                LDW     *2
                STW     DXBUF
                JSX     DXFER
                EXIT    KDXP

; One sector -- ACR is its index -- into the 47 words DXBUF names.
DRD1            SUBR
                STW     DXSEC
                LDW     K47
                STW     DXCNT
                CLR
                STW     DXOP
                JSX     DXFER
                EXIT    DRD1

; ...and one sector the other way.
DWR1            SUBR
                STW     DXSEC
                LDW     K47
                STW     DXCNT
                LDW     K1
                STW     DXOP
                JSX     DXFER
                EXIT    DWR1

; Move DXCNT words between the disc, from sector DXSEC on, and core at
; DXBUF -- to the disc if DXOP is set -- and return the controller's
; status, zero for a clean transfer.  The caller holds IOLOCK.  It waits
; on the transfer the way a task waits on anything here: masked, look,
; and if not yet, mark itself waiting and hand the processor on, and
; DSERV wakes it; DWTR names the node for that.  A unit that is not
; there reads not-ready, and that comes back at once rather than waiting
; for a completion that would never come.
DXFER           SUBR
                MSK
                DIN     1,0
                SAP                     ; not ready?  say so
                JMP     DXNRD
                LDW     CURT
                STW     DWTR
                LDW     K1
                STW     DBUSY           ; before the DOT: the completion can be next
                LDW     DXBUF
                DOT     1,1             ; the core address (5-9.5.2)
                LDW     DXSEC
                AND     K7F
                STW     DXTS
                LDW     DXSEC
                SRL     7
                SLL     10
                ORI     DXTS
                DOT     1,2             ; track in bits 0-5, sector in 7-15 (5-9.5.3)
                LDW     DXOP
                SAZ
                JMP     DXWR
                LDW     DXCNT
                DOT     1,6             ; unit 0, the count, read (5-9.5.4)
                JMP     DXWT
DXWR            LDW     DXCNT
                DOT     1,4             ; ...write
DXWT            LDW     DBUSY
                SAZ                     ; over?
                JMP     DXBLK
                JMP     DXDN
DXBLK           LDX     CURT
                LDW     KWAIT
                STW     *T.STA
                JSX     SWTCH           ; until DSERV says so
                MSK
                JMP     DXWT
DXDN            LDW     DSTAT
                STW     DXRES
                LDW     KM1
                STW     DWTR
                UNM
                LDW     DXRES
                EXIT    DXFER
DXNRD           UNM
                EXIT    DXFER

; What the machine runs when every task is asleep.  A branch to self is a
; legal idle here -- the levels are enabled and unmasked, so the tick that
; ends somebody's sleep takes the processor away from it -- and it sits
; outside [ISRBEG, ISREND) like any other task's code, or the scheduler
; could never switch away from it.
; Take a word out of the queue the accumulator addresses, waiting for one
; if the queue is empty.  Outside the deferred range deliberately: a task
; blocks here, and a tick that finds it here has every business switching
; away from it.
;
; The wait is the queue's own: mark this task waiting, hang its block off
; the queue, and hand the processor on.  Q.PUT wakes it and forgets it.
; One waiter to a queue, so one reader to a queue -- a second task
; blocking here would displace the first, which would then never wake.
; The console's queue is therefore read only by whoever has the console,
; the shell or the task RUN gave it to, and any other caller gets minus
; one back instead of a place in the queue.
; Tasks reading different queues may be in here together, so the link and
; the queue are kept in the caller's node across the wait, not in a slot
; of this routine's: masked from its first instruction, and the static
; cells good only until the processor is handed on.
Q.GET           MSK
                STX     QGLNK
                STW     QGD
                CMW     KCONSQ
                SEQ                     ; the console's queue?
                JMP     QGIN
                LDW     CURT
                CMW     KSHELL
                SNE                     ; the shell reads it by right,
                JMP     QGIN
                CAX
                LDW     *T.CON
                SAZ                     ; and so does the task RUN gave it to;
                JMP     QGIN
                LDW     KM1             ; anybody else is told no, at once
                LDX     QGLNK
                UNM
                JSX     *0
QGIN            LDX     CURT
                LDW     QGD
                STW     *T.KAR
                LDW     QGLNK
                STW     *T.KLK
QGL             LDX     CURT
                LDW     *T.KAR
                STW     QGD
                CAX
                LDW     *Q.CNT
                SAZ                     ; anything in it?
                JMP     QGT
                LDW     CURT            ; no: wait on it
                STW     *Q.WTR
                CAX
                LDW     KWAIT
                STW     *T.STA
                JSX     SWTCH           ; gone until Q.PUT wakes this task
                MSK
                JMP     QGL             ; awake: look again
QGT             LDW     *Q.BUF
                ADD     *Q.HEAD
                CAX
                LDW     *0
                STW     QGI
                LDX     QGD
                LDW     *Q.HEAD
                ADD     K1
                CMW     *Q.CAP
                SLS
                CLR                     ; round the ring
                STW     *Q.HEAD
                LDW     *Q.CNT
                SUB     K1
                STW     *Q.CNT
                LDX     CURT
                LDW     *T.KLK
                STW     QGLNK
                LDW     QGI
                LDX     QGLNK
                UNM
                JSX     *0

IDLE            JMP     IDLE

; ---------------------------------------------------------------- the pool
; First fit over a list of free blocks sorted by address, with the block
; layout the header describes: two words in front of every payload, its
; size and either the next free block (zero ends the list) or the owning
; node with bit 0 set.  Three entries share one body: K.ALLOC takes any
; address, K.ALLCW wants the payload inside one 2048-word page and
; K.ALLCB inside one 1024-word byte page -- what a module needs, since
; its M fields are page offsets.  Those three hand out core below the
; fence only, where a byte address is positive; K.ALLHI and K.ALHIW (and
; ALLCHB, the loader's) are the same three for a caller that compares
; no byte addresses in what it gets, and look above the fence first.
; The search is the same either way, over the part of the list between
; a floor and a ceiling.  The answer is the payload address, or
; zero, which is never in the pool.  Any task may call, so every entry
; is masked from its first instruction to its return: the cells are one
; caller's at a time, and the link is kept by hand because the stubs
; fall into the body with IXR still holding it.
ALLOCW          MSK
                STW     ALSIZ
                LDW     K2047
                JMP     ALCOM
ALLOCB          MSK
                STW     ALSIZ
                LDW     K1023
                JMP     ALCOM
ALLOC           MSK
                STW     ALSIZ
                CLR
ALCOM           STW     ALMSK           ; the window less one, or zero for any
                STX     ALRET
                CLR
                STW     ALHIQ           ; below the fence, and nowhere else
                JMP     ALBEG
ALLCHW          MSK
                STW     ALSIZ
                LDW     K2047
                JMP     ALCOMH
ALLCHB          MSK
                STW     ALSIZ
                LDW     K1023
                JMP     ALCOMH
ALLOCH          MSK
                STW     ALSIZ
                CLR
ALCOMH          STW     ALMSK
                STX     ALRET
                LDW     K1
                STW     ALHIQ           ; above the fence, and below it if
                                        ; there is nothing there
ALBEG           LDW     ALSIZ           ; an even number of words, always:
                ADD     K1              ; every block then starts on an even
                AND     KFFFE           ; word, which is what lets one that
                STW     ALSIZ           ; was swapped out be cut out of the
                LDW     ALMSK           ; free list again exactly where it
                SAZ                     ; was (ALLOCAT).  And one that must
                JMP     ALBW            ; lie in a window cannot be bigger
                JMP     ALBGO           ; than the window
ALBW            ADD     K1
                CMW     ALSIZ
                SLS
                JMP     ALBGO
                JMP     ALFAIL
ALBGO           LDW     ALHIQ
                STW     ALHI
                SAZ
                JMP     ALBH
ALLOW           CLR
                STW     ALFLO
                LDW     KFENCE
                STW     ALCEI
                JMP     ALGO
ALBH            LDW     KFENCE
                STW     ALFLO
                LDW     KPOOLE
                STW     ALCEI
ALGO            CLR
                STW     ALPRV           ; the free block before F; zero: the head
                LDW     FREHD
ALSCN           STW     ALF             ; F, a free block's header
                SAZ                     ; the end of the list: nothing fits
                JMP     ALTRY0
                JMP     ALNONE
ALTRY0          CMW     ALCEI
                SLS                     ; at the ceiling: the list is in
                JMP     ALNONE          ; address order, so nothing fits
                CMW     ALFLO
                SLS                     ; below the floor: not this one
                JMP     ALTRY
                JMP     ALNXT
ALTRY           CAX
                LDW     *0
                STW     ALS             ; S, its payload
                LDW     ALF
                ADD     K2
                STW     ALP             ; P, the candidate payload: F+2
                LDW     ALMSK
                SAZ                     ; any containment asked for?
                JMP     ALCON
                JMP     ALFIT

; Containment.  E is the first word of the window after P's; a payload
; that would cross it moves up to E -- or to E+1 when E is F+3, since the
; header must sit at F+2 or above to leave the leading fragment room for
; its own header (a fragment may hold zero words, but not minus one).
; Having moved, it must still lie inside the window it moved into.
ALCON           LDW     ALP
                ORI     ALMSK
                ADD     K1
                STW     ALE
                LDW     ALP
                ADD     ALSIZ
                CMW     ALE
                SGR                     ; P+size > E: it would straddle
                JMP     ALFIT           ; no: P stands
                LDW     ALE
                SUB     ALF
                CMW     K3
                SNE                     ; E == F+3?
                JMP     ALBP1
                LDW     ALE
                STW     ALP
                JMP     ALCK2
ALBP1           LDW     ALE
                ADD     K1
                STW     ALP
ALCK2           LDW     ALE             ; W, the window after E's
                ADD     ALMSK
                ADD     K1
                STW     ALW
                LDW     ALP
                ADD     ALSIZ
                CMW     ALW
                SGR                     ; still over the edge?
                JMP     ALFIT
                JMP     ALNXT

; Does [P, P+size) lie inside the block?  FE is the block's end, F+2+S.
; The test is on the room from P to FE against the size, not on where
; the block would end: with a size of several thousand words and P in
; the upper half of core, P+size passes X'7FFF' and is minus, and this
; machine's compares are signed.
ALFIT           LDW     ALF
                ADD     K2
                ADD     ALS
                STW     ALFE
                SUB     ALP
                CMW     ALSIZ
                SLS                     ; less room than is wanted?
                JMP     ALFITS
                JMP     ALNXT
ALFITS          LDW     ALP
                ADD     ALSIZ
                STW     ALEND
                JMP     ALTAKE
ALNXT           LDW     ALF             ; on to the next free block
                STW     ALPRV
                CAX
                LDW     *1
                JMP     ALSCN

; Take it.  H is the new header.  A leading fragment keeps F on the list
; with a shorter size and becomes the predecessor of whatever follows.  A
; tail of three words or more becomes a free block of its own at the end
; of the allocation; a shorter one is simply given to the allocation, since
; two words of header with nothing behind them serve nobody.
ALTAKE          LDW     ALP
                SUB     K2
                STW     ALH
                CMW     ALF
                SEQ                     ; a leading fragment?
                JMP     ALLEAD
                JMP     ALTAIL
ALLEAD          LDW     ALH
                SUB     ALF
                SUB     K2
                LDX     ALF
                STW     *0              ; [F] = H-F-2, its next as it was
                LDW     ALF
                STW     ALPRV
ALTAIL          LDW     ALFE
                SUB     ALEND
                CMW     K3
                SLS                     ; a tail worth keeping?
                JMP     ALSPLT
                LDW     ALFE            ; no: the allocation runs to the end
                SUB     ALH
                SUB     K2
                STW     ALSIZ
                LDX     ALF
                LDW     *1
                STW     ALNX            ; and the list goes on past F
                JMP     ALLNK
ALSPLT          LDX     ALEND           ; yes: [END] = FE-END-2, next = F's
                LDW     ALFE
                SUB     ALEND
                SUB     K2
                STW     *0
                LDX     ALF
                LDW     *1
                LDX     ALEND
                STW     *1
                LDW     ALEND
                STW     ALNX
ALLNK           LDW     ALPRV           ; PRV's successor, or the head, is NX
                SAZ
                JMP     ALLNK1
                LDW     ALNX
                STW     FREHD
                JMP     ALHDR
ALLNK1          CAX
                LDW     ALNX
                STW     *1
ALHDR           LDX     ALH             ; the header: size, and the owner
                LDW     ALSIZ
                STW     *0
                LDW     CURT
                ORI     K8000
                STW     *1
                LDX     CURT            ; no longer waiting for core, if it was
                CLR
                STW     *T.MW
                LDW     ALP
                JMP     ALOUT
ALNONE          LDW     ALHI
                SAZ                     ; a second try to make, below the fence?
                JMP     ALAGN
                LDW     SWSECS
                SAZ                     ; nothing fits.  Is there a swap file?
                JMP     ALWAIT
ALFAIL          LDX     CURT
                CLR
                STW     *T.MW
ALOUT           LDX     ALRET           ; the link first: past the UNM the
                UNM                     ; cells are the next caller's
                JSX     *0
ALAGN           CLR
                STW     ALHI
                JMP     ALLOW

; Nothing fits, and there is a swapper: ask it to put somebody out, and
; wait.  The request goes into the caller's node -- its link, the size,
; and the window with bit 0 set for a caller that will take core above
; the fence -- since the cells here are the next caller's the moment the
; processor is handed on.  The swapper answers in T.MW: 3, somebody has
; gone, try again; 2, there is nobody to put out, and the caller gets its
; zero.  Any other wake is advice, and the asking is simply repeated.
ALWAIT          LDX     CURT
                LDW     *T.MW
                CMW     K2
                SNE
                JMP     ALFAIL
                LDW     ALRET
                STW     *T.KLK
                LDW     ALSIZ
                STW     *T.KAR
                LDW     ALHIQ
                SAZ
                LDW     K8000
                ORI     ALMSK
                STW     *T.KDS
                LDW     K1
                STW     *T.MW
                LDX     KSWAP
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; the swapper, out of its wait
                JMP     ALW1
                CLR
                STW     *T.STA
ALW1            LDX     CURT
                LDW     KWAIT
                STW     *T.STA
                JSX     SWTCH
                MSK
                LDX     CURT
                LDW     *T.KLK
                STW     ALRET
                LDW     *T.KAR
                STW     ALSIZ
                LDW     *T.KDS
                AND     K7FFF
                STW     ALMSK
                LDW     *T.KDS
                AND     K8000
                STW     ALHIQ
                JMP     ALBEG

; Cut the words [H, H+n) out of the free list as an allocated block: ACR
; is H, where its header goes, ATN is n, header and all, and ATTAG is the
; owner tag.  One if it is done; zero if those words are not all free.
; This is how a block that was swapped out comes back to where it was.
; Every block boundary is on an even word, so what is left of the free
; block on either side is nothing or at least a header.  The caller holds
; the mask.
ALLOCAT         SUBR
                STW     ATH
                ADD     ATN
                STW     ATE
                CLR
                STW     ALPRV
                LDW     FREHD
ATSC            STW     ALF
                SAZ
                JMP     AT1
                JMP     ATNO
AT1             CMW     ATH
                SGR                     ; a free block that starts past H:
                JMP     AT2             ; the list is in address order, so
                JMP     ATNO            ; H is in none
AT2             CAX
                LDW     *0
                ADD     ALF
                ADD     K2
                STW     ALFE
                CMW     ATH
                SGR                     ; ends at or before H: the next one
                JMP     ATNX
                LDW     ALFE
                CMW     ATE
                SLS                     ; ends before H+n: not all free
                JMP     ATFIT
                JMP     ATNO
ATNX            LDW     ALF
                STW     ALPRV
                CAX
                LDW     *1
                JMP     ATSC
ATFIT           LDX     ALF
                LDW     *1
                STW     ALNX
                LDW     ALFE            ; what is left above it stays free
                SUB     ATE
                SAZ
                JMP     ATTL
                JMP     ATLD
ATTL            SUB     K2
                LDX     ATE
                STW     *0
                LDW     ALNX
                STW     *1
                LDW     ATE
                STW     ALNX
ATLD            LDW     ATH             ; and so does what is left below
                SUB     ALF
                SAZ
                JMP     ATLF
                LDW     ALPRV           ; nothing below: F leaves the list
                SAZ
                JMP     ATU1
                LDW     ALNX
                STW     FREHD
                JMP     ATHDR
ATU1            CAX
                LDW     ALNX
                STW     *1
                JMP     ATHDR
ATLF            SUB     K2
                LDX     ALF
                STW     *0
                LDW     ALNX
                STW     *1
ATHDR           LDX     ATH
                LDW     ATN
                SUB     K2
                STW     *0
                LDW     ATTAG
                STW     *1
                LDW     K1
                EXIT    ALLOCAT
ATNO            CLR
                EXIT    ALLOCAT

; Give a block back.  FREEI does the work with the mask held by its
; caller, which is FREE for a task and the exit path for a task's whole
; estate; MSK and UNM do not nest, so a routine that is already masked
; comes here directly.  The block goes into the list where its address
; falls and is joined to a neighbour on either side that touches it, so
; the list never holds two adjacent free blocks.  A size field only ever
; grows here: the freed block's own may grow to swallow the one above it,
; and the one below it may grow to swallow this one, so a walk of the
; pool by size strides stays right through a block just freed.
FREE            MSK
                STX     FRRET
                JSX     FREEI
                LDX     FRRET
                UNM
                JSX     *0

FREEI           SUBR
                SUB     K2
                STW     FRH             ; H, the block's header
                CLR
                STW     FRPRV           ; the free block below it; zero: none
                LDW     FREHD
FRSCN           STW     FRNX            ; NX, the first free block above it, or zero
                SAZ
                JMP     FRSC1
                JMP     FRLNK
FRSC1           CMW     FRH
                SGR                     ; NX > H: found the place
                JMP     FRSC2
                JMP     FRLNK
FRSC2           STW     FRPRV
                CAX
                LDW     *1
                JMP     FRSCN
FRLNK           LDX     FRH             ; PRV -> H -> NX
                LDW     FRNX
                STW     *1
                LDW     FRPRV
                SAZ
                JMP     FRLK1
                LDW     FRH
                STW     FREHD
                JMP     FRJN
FRLK1           CAX
                LDW     FRH
                STW     *1
FRJN            LDW     FRNX            ; does H run up to NX?
                SAZ
                JMP     FRJN1
                JMP     FRJP
FRJN1           LDX     FRH
                LDW     *0
                ADD     FRH
                ADD     K2
                CMW     FRNX
                SEQ
                JMP     FRJP
                LDX     FRNX            ; [H] += [NX]+2, and NX's next is H's
                LDW     *0
                ADD     K2
                LDX     FRH
                ADD     *0
                STW     *0
                LDX     FRNX
                LDW     *1
                LDX     FRH
                STW     *1
FRJP            LDW     FRPRV           ; does PRV run up to H?
                SAZ
                JMP     FRJP1
                JMP     FRDN
FRJP1           CAX
                LDW     *0
                ADD     FRPRV
                ADD     K2
                CMW     FRH
                SEQ
                JMP     FRDN
                LDX     FRH             ; [PRV] += [H]+2, and H's next is PRV's
                LDW     *0
                ADD     K2
                LDX     FRPRV
                ADD     *0
                STW     *0
                LDX     FRH
                LDW     *1
                LDX     FRPRV
                STW     *1
FRDN            EXIT    FREEI

; The allocator's cells and constants.
FREHD           WORD    0               ; the first free block
ALSIZ           WORD    0               ; the request: words...
ALMSK           WORD    0               ; ...and the window less one, or zero
ALRET           WORD    0               ; the caller's link
ALFLO           WORD    0               ; the search's floor and ceiling:
ALCEI           WORD    0               ; a free block's header lies between
ALHI            WORD    0               ; nothing above the fence fits: try below
ALHIQ           WORD    0               ; the caller will take core above the fence
ATH             WORD    0               ; ALLOCAT's block, where it ends,
ATE             WORD    0
ATN             WORD    0               ; its length and its owner tag
ATTAG           WORD    0
SWSECS          WORD    0               ; sectors in the swap file; zero: none
ALPRV           WORD    0               ; the search: the free block before F
ALF             WORD    0               ; F, and its payload S
ALS             WORD    0
ALP             WORD    0               ; the candidate payload, and its end
ALEND           WORD    0
ALE             WORD    0               ; the next window's first word
ALW             WORD    0               ; and the one after that
ALFE            WORD    0               ; the block's end
ALH             WORD    0               ; the new header
ALNX            WORD    0               ; what follows it on the list
FRRET           WORD    0               ; FREE's link
FRH             WORD    0               ; the block being freed
FRPRV           WORD    0               ; the free blocks on either side
FRNX            WORD    0
K2              WORD    2
K3              WORD    3
K1023           WORD    1023
K2047           WORD    2047
K8000           WORD    X'8000'
KFFFE           WORD    X'FFFE'
KPOOLB          WORD    POOLB           ; the pool: [POOLB, POOLE)
KPOOLE          WORD    POOLE
KPOOLH          WORD    POOLH           ; its high part's first block,
KFENCE          WORD    FENCE           ; the fence in front of that,
KPOOLN          WORD    FENCE-POOLB-2   ; and the two parts as free blocks'
KPOOHN          WORD    POOLE-POOLH-2   ; payloads
K7FFF           WORD    X'7FFF'
KSHELL          WORD    SHTCB           ; the shell's node, for the console's return

; ---------------------------------------------------------------- tasks
; SPAWN: ACR is a node of T.LEN words with T.PCR, T.NAM and T.CON filled
; in.  The rest is set here -- the status is what SWTCH builds for any
; resume, the entry's page and global -- and the node is linked into the
; ring after the running task, so the next pick finds it first.
SPAWN           SUBR
                STW     SPNOD
                CAX
                CLR
                STW     *T.STA
                STW     *T.ACR
                STW     *T.IXR
                STW     *T.DLY
                STW     *T.MBX
                LDW     *T.PCR
                AND     KPGMSK
                SLL     1
                ORI     KGLB
                STW     *T.MST
                LDW     SECS
                STW     *T.RES
                MSK
                LDX     CURT
                LDW     *T.NXT
                LDX     SPNOD
                STW     *T.NXT
                LDW     SPNOD
                LDX     CURT
                STW     *T.NXT
                LDW     KNRING
                ADD     K1
                STW     KNRING
                UNM
                EXIT    SPAWN

; TEXIT: the running task is over, and takes everything with it.  Its
; last character is waited out, since OWNER must never name a node that
; is gone; the console is handed back if it held it -- CONBSY down, and
; the shell woken out of its wait, only out of that; REAP takes the rest;
; and the processor goes to whoever PICK finds, through the second half
; of SWTCH with nothing parked.  Masked throughout: the one unmasked
; instruction on the way out is SWTCH's INR 3, inside the range.
TEXIT           MSK
                LDX     CURT
                LDW     *T.MBX
                SAZ                     ; a character still printing?
                JMP     TXWT
                JMP     TXGO
TXWT            LDW     KWAIT           ; wait for it, and start over
                STW     *T.STA
                JSX     SWTCH
                JMP     TEXIT
TXGO            LDW     CURT
                STW     XDEAD
                LDW     *T.CON
                SAZ                     ; the console's?
                JMP     TXCON
                JMP     TXRING
TXCON           CLR
                STW     CONBSY
                LDX     KSHELL
                LDW     *T.STA
                CMW     KWAIT
                SEQ                     ; waiting on the hand-back?
                JMP     TXRING
                CLR
                STW     *T.STA
TXRING          JSX     REAP
                JMP     SWRES           ; on to whoever can run

; The task XDEAD names is over, and everything it had goes: the node off
; the ring and counted off, LASTS -- where KICK's scan starts -- and CURT
; moved to its predecessor if they named it, CURT before anything is
; freed since PICK starts from CURT's link; a queue that had it down as
; its waiter told otherwise; its channels let go; every block in the
; pool tagged with it given back; and the node returned to the arena,
; marked free.  Masked throughout, for the running task (TEXIT) and for
; one the shell is ending (KILLT) alike.
REAP            SUBR
                LDW     XDEAD           ; the predecessor: whose link names it
RPPL            STW     XPRED
                CAX
                LDW     *T.NXT
                CMW     XDEAD
                SEQ
                JMP     RPPL
                LDX     XDEAD           ; unlink
                LDW     *T.NXT
                LDX     XPRED
                STW     *T.NXT
                LDW     KNRING
                SUB     K1
                STW     KNRING
                LDW     LASTS
                CMW     XDEAD
                SEQ
                JMP     RPCUR
                LDW     XPRED
                STW     LASTS
RPCUR           LDW     CURT
                CMW     XDEAD
                SEQ
                JMP     RPQ
                LDW     XPRED
                STW     CURT
RPQ             LDW     QCONS+Q.WTR
                CMW     XDEAD
                SEQ
                JMP     RPCH
                LDW     KM1
                STW     QCONS+Q.WTR
RPCH            LDW     XDEAD           ; its channels
                SMB     FCREL
                JSX     FCREL
                LDW     XDEAD           ; and its place in the swap file, if
                SMB     SWXFRE          ; its blocks are there and not here
                JSX     SWXFRE
                LDW     KPOOLB          ; the sweep, block by block
RPSWP           STW     XB
                CMW     KPOOLE
                SLS                     ; the end of the pool?
                JMP     RPNOD
                CAX
                LDW     *1
                SAM                     ; allocated?
                JMP     RPSNX
                AND     K7FFF
                CMW     XDEAD
                SEQ                     ; its own?
                JMP     RPSNX
                LDW     XB
                ADD     K2
                JSX     FREEI
RPSNX           LDX     XB
                LDW     *0
                ADD     XB
                ADD     K2
                JMP     RPSWP
RPNOD           LDX     XDEAD           ; the node last, to the arena
                LDW     KSFREE
                STW     *T.STA
                LDW     XDEAD
                JSX     NFREE
                EXIT    REAP

; End the task ACR names, which is not the one running: the shell's KILL.
; The caller holds the mask and the disc -- so the task is in no transfer
; and no file call -- has stopped it, and has seen that the printer is
; not on a character of its.
KILLT           SUBR
                STW     XDEAD
                CAX
                LDW     *T.CON
                SAZ                     ; the console's?  it comes back
                JMP     KLCON
                JMP     KLREAP
KLCON           CLR
                STW     CONBSY
KLREAP          JSX     REAP
                EXIT    KILLT

; A node from the arena, or zero when every one is in use; and a node
; given back.  Callers hold the mask.  A node keeps its job number for
; good, and comes out with nothing else of its last task left in it.
NALLOC          SUBR
                LDW     NFREH
                SAZ
                JMP     NAL1
                EXIT    NALLOC
NAL1            CAX
                LDW     *T.NXT
                STW     NFREH
                CLR
                STW     *T.KLK
                STW     *T.KAR
                STW     *T.KDS
                STW     *T.ARG
                STW     *T.SWP
                STW     *T.MW
                CXA
                EXIT    NALLOC
NFREE           SUBR
                CAX
                LDW     NFREH
                STW     *T.NXT
                CXA
                STW     NFREH
                EXIT    NFREE

SPNOD           WORD    0               ; SPAWN's node
XDEAD           WORD    0               ; TEXIT's: the node going, its
XPRED           WORD    0               ; predecessor on the ring, and the
XB              WORD    0               ; sweep's cursor

; ---------------------------------------------------------------- the shell
; Prints the banner and then reads a line and runs it, forever.  It is
; the only task running when the machine comes up.  Everything it prints
; goes through its own mailbox one character at a time like any other
; task's, so a command's output and what a task behind the prompt prints
; interleave on the printer exactly as two users' output did.
                ORG     X'800'

SHELL           LDW     SHMBAN
                JSX     SHMSG
                LDW     SHMHNT
                JSX     SHMSG
                LDW     SHKIN0          ; INIT.CM, if the disc has one, is
                STW     SHFN            ; read before the keyboard is
                LDW     SHKIN1
                STW     SHFN+1
                CLR
                STW     SHFN+2
                LDW     SHKCM
                STW     SHFN+3
                JSX     SHCMOP

SHLOOP          CLR                     ; a kill aimed at nobody is forgotten
                SMB     KILLRQ
                STW     KILLRQ
                LDW     SHMPRM
                JSX     SHMSG
                JSX     SHGETL          ; a line, however long that takes
                LDW     SHKLBB
                STW     SHCUR
                JSX     SHSKB
                LDX     SHCUR
                CLR
                LDB     *0
                CMW     SHKAT
                SNE                     ; @NAME: commands from a file
                JMP     SHAT
                JSX     SHTOK           ; the command word: its second half,
                LDW     STOK1           ; since a short word fills only that
                SAZ                     ; an empty line is not an error
                JMP     SHDSP
                JMP     SHLOOP

; Walk the command table: two words of name, then the handler to jump to,
; and a zero handler ends it -- a short name leaves its first word zero.
; Only the first four characters are matched, which is how the period
; interpreters did it -- START and STAT differ inside four, and a longer
; word that starts the same is simply taken as the command.
SHDSP           LDW     SHKTAB
                STW     SHTP
SHDL            LDX     SHTP
                LDW     *2
                SAZ                     ; the end of the table?
                JMP     SHDCM
                JMP     SHDNF
SHDCM           LDW     *0
                CMW     STOK0
                SEQ
                JMP     SHDNX
                LDX     SHTP
                LDW     *1
                CMW     STOK1
                SEQ
                JMP     SHDNX
                LDX     SHTP
                LDW     *2              ; matched: the handler's address
                CAX
                JMP     *0
SHDNX           LDW     SHTP
                ADD     SHK3
                STW     SHTP
                JMP     SHDL
; Not a command: then it is a program -- the word as a name, type RX,
; run as RUN would run it -- and WHAT only if the disc has no such file.
SHDNF           LDW     SHKLBB
                STW     SHCUR
                LDW     SHK1
                STW     LDCON
                STW     SHIMPL
                LDW     SHKRX
                STW     SHDTY
                JSX     SHNAME
                JMP     SHLD1

; @NAME: the shell's lines come from NAME.CM until it runs out.
SHAT            LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                LDW     SHKCM
                STW     SHDTY
                JSX     SHNAME
                JSX     SHCMOP
                SAP                     ; no such file, or no channel
                JMP     SHATNO
                JMP     SHLOOP
SHATNO          LDW     SHMNOF
                JSX     SHMSG
                JMP     SHLOOP

; Open the file SHFN names as the command file, in place of one that is
; open already: the channel, or minus.
SHCMOP          SUBR
                JSX     SHCMCL
                CLR
                STW     SHFN+4          ; to read
                LDW     SHKFN
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     SHCMOX
                STW     SHCMCH
SHCMOX          EXIT    SHCMOP

; Close the command file, if one is open.
SHCMCL          SUBR
                LDW     SHCMCH
                SAZ
                JMP     SHCMC1
                EXIT    SHCMCL
SHCMC1          SMB     K.CLOSE
                JSX     K.CLOSE
                CLR
                STW     SHCMCH
                EXIT    SHCMCL

; ---------------------------------------------------------------- commands
; HELP: the commands that are in the shell, and then the ones that are
; not -- every program the directory has, eight names to a line.  With
; no disc, or no directory on it, there are none to name.
SHHELP          LDW     SHMHL1
                JSX     SHMSG
                LDW     SHMHL2
                JSX     SHMSG
                LDW     SHMHL3
                JSX     SHMSG
                JSX     SHDLD
                SAM
                JMP     SHHP0
                JMP     SHLOOP
SHHP0           CLR
                STW     SHTI            ; names on the line so far
                LDW     SHKFDE
                STW     LDCP
SHHPL           LDW     LDCP
                CMW     SHKFDX
                SLS                     ; inside the table?
                JMP     SHHPX
                CAX
                LDW     *DE.FLG
                AND     SHK0FF
                SAZ                     ; an unused entry ends it
                JMP     SHHP1
                JMP     SHHPX
SHHP1           CMW     SHKDKF
                SEQ                     ; a file...
                JMP     SHHPN
                LDW     *DE.TYP
                CMW     SHKRX
                SEQ                     ; ...and a program?
                JMP     SHHPN
                LDW     SHKSP
                JSX     SHPUTC
                JSX     SHPNAM
                LDW     SHTI
                ADD     SHK1
                STW     SHTI
                CMW     SHK8
                SLS                     ; room on the line for another?
                JMP     SHHPW
                JMP     SHHPN
SHHPW           JSX     SHNL
                CLR
                STW     SHTI
SHHPN           LDW     LDCP
                ADD     SHK8
                STW     LDCP
                JMP     SHHPL
SHHPX           LDW     SHTI
                SAZ                     ; a line part made?
                JMP     SHHPE
                JMP     SHLOOP
SHHPE           JSX     SHNL
                JMP     SHLOOP

; Seconds since the executive came up. SECS is the scheduler's, counted
; sixty ticks at a time.
SHUPT           JSX     SHPUP
                JMP     SHLOOP

SHPUP           SUBR
                LDW     SHMUPM
                JSX     SHMSG
                LDW     SHKSP
                JSX     SHPUTC
                SMB     SECS
                LDW     SECS
                JSX     SHDEC
                LDW     SHMSEC
                JSX     SHMSG
                EXIT    SHPUP

; The tasks: job number, name, state, how much longer a sleeper has, and
; OUT for one whose blocks are on the disc.  Not a walk of the ring,
; which a task leaving it part way through would send astray: the two
; nodes that are always here, in the order they lie in core, then every
; arena node that is in use, then idle.
SHSTAT          JSX     SHPUP
                SMB     SWSECS
                LDW     SWSECS
                SAZ                     ; a swap file: then how busy it has been
                JMP     SHSTSW
                JMP     SHST0
SHSTSW          LDW     SHMSWP
                JSX     SHMSG
                SMB     SWOUTS
                LDW     SWOUTS
                JSX     SHDEC
                LDW     SHMSWO
                JSX     SHMSG
                SMB     SWINS
                LDW     SWINS
                JSX     SHDEC
                LDW     SHMSWI
                JSX     SHMSG
SHST0           LDW     SHKSHN
                STW     SHTO
                LDW     SHK2
                STW     SHTI
SHST1           JSX     SHSTLN
                LDW     SHTO
                ADD     SHKTLN
                STW     SHTO
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ
                JMP     SHST1
                LDW     SHKNAB
                STW     SHTO
                LDW     SHKNAN
                STW     SHTI
SHST2           LDX     SHTO
                LDW     *T.STA
                CMW     SHKFRE
                SNE                     ; in use?
                JMP     SHST3
                JSX     SHSTLN
SHST3           LDW     SHTO
                ADD     SHKTLN
                STW     SHTO
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ
                JMP     SHST2
                LDW     SHKIDL
                STW     SHTO
                JSX     SHSTLN
                JMP     SHLOOP

; One line of it, for the node in SHTO.  The state is read once: the task
; may change it between the name and the number.
SHSTLN          SUBR
                LDX     SHTO
                LDW     *T.ID
                JSX     SHDEC
                LDW     SHKSP
                JSX     SHPUTC
                LDX     SHTO
                LDW     *T.NAM          ; its two-character name
                JSX     SHPW2
                LDW     SHKSP
                JSX     SHPUTC
                LDX     SHTO            ; its state, as four characters
                LDW     *T.STA
                STW     SHSS
                SLL     1
                ADD     SHKSTA
                STW     SHSA
                CAX
                LDW     *0
                JSX     SHPW2
                LDX     SHSA
                LDW     *1
                JSX     SHPW2
                LDW     SHSS
                CMW     SHKSLP          ; asleep? then say for how long
                SEQ
                JMP     SHSTN
                LDX     SHTO
                LDW     *T.DLY
                JSX     SHDEC
SHSTN           LDX     SHTO
                LDW     *T.SWP
                SAZ
                JMP     SHSTO
                JMP     SHSTE
SHSTO           LDW     SHMOUT
                JSX     SHMSG
SHSTE           JSX     SHNL
                EXIT    SHSTLN

; STOP n and START n differ only in the state they store, into the task
; with that job number: one the shell loaded, since no other has a
; number.  Not one that has the console, which would leave the shell
; waiting on a hand-back nobody could make; and the state goes in with
; the disc claimed, so that it is never a task part way through a
; transfer that is left standing.
SHSTOP          LDW     SHKOFF
                STW     SHNST
                JMP     SHSSET
SHSTRT          CLR
                STW     SHNST
SHSSET          JSX     SHJOB
                SAZ
                JMP     SHSJ1
                JMP     SHLOOP
SHSJ1           CAX
                LDW     *T.CON
                SAZ
                JMP     SHSOBD
                JSX     SHIOC
                LDX     SHKN
                LDW     SHNST
                STW     *T.STA
                JSX     SHIOR
                JMP     SHLOOP
SHSOBD          LDW     SHMNOT
                JSX     SHMSG
                JMP     SHLOOP

; KILL n: the task with that job number is over, whatever it was doing.
SHKILL          JSX     SHJOB
                SAZ
                JMP     SHKL1
                JMP     SHLOOP
SHKL1           JSX     SHKDO
                JMP     SHLOOP

; The job number at SHCUR as a task the shell loaded: its node, left in
; SHKN as well -- or zero, NO SUCH TASK having been said.
SHJOB           SUBR
                JSX     SHSKB
                CLR
                STW     SHNV
SHJBL           LDX     SHCUR
                CLR
                LDB     *0
                SUB     SHKZER
                STW     SHW2
                SAP                     ; below '0'
                JMP     SHJBD
                CMW     SHK10
                SLS                     ; above '9'
                JMP     SHJBD
                LDW     SHNV            ; times ten, and the digit
                SLL     3
                ADD     SHNV
                ADD     SHNV
                ADD     SHW2
                STW     SHNV
                LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                JMP     SHJBL
SHJBD           LDW     SHKNAB          ; the arena node with that number
                STW     SHKN
                LDW     SHKNAN
                STW     SHTI
SHJBF           LDX     SHKN
                LDW     *T.ID
                CMW     SHNV
                SEQ
                JMP     SHJBN
                LDW     *T.STA
                CMW     SHKFRE
                SNE                     ; and in use
                JMP     SHJBN
                LDW     SHKN
                EXIT    SHJOB
SHJBN           LDW     SHKN
                ADD     SHKTLN
                STW     SHKN
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ
                JMP     SHJBF
                LDW     SHMNOT
                JSX     SHMSG
                CLR
                EXIT    SHJOB

; End the task in SHKN.  The disc first: with it claimed here the task
; is inside no transfer and no file call.  Then, masked, it is stopped --
; so it deposits nothing more -- and if the printer is on a character of
; its, that is waited out a tick at a time, since OWNER must never name
; a node that is gone.
SHKDO           SUBR
                JSX     SHIOC
SHKDL           MSK
                LDX     SHKN
                LDW     SHKOFF
                STW     *T.STA
                SMB     OWNER
                LDW     OWNER
                CMW     SHKN
                SNE
                JMP     SHKDW
                LDW     SHKN
                SMB     KILLT
                JSX     KILLT
                UNM
                JSX     SHIOR
                EXIT    SHKDO
SHKDW           LDW     SHK1
                SMB     SH.DLY
                STW     SH.DLY
                LDW     SHKSLP
                SMB     SH.STA
                STW     SH.STA
                SMB     SWTCH
                JSX     SWTCH
                JMP     SHKDL

; Print the rest of the line back.
SHECHO          JSX     SHSKB
SHECL           LDX     SHCUR
                CLR
                LDB     *0
                SAZ                     ; the terminator?
                JMP     SHECN
                JMP     SHECD
SHECN           JSX     SHPUTC
                LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                JMP     SHECL
SHECD           JSX     SHNL
                JMP     SHLOOP

; Wait for the console to come back: masked look, SWAI, SWTCH, look
; again -- reading no queue, which is what keeps QCONS to its one reader.
; The console's reader is the shell exactly when CONBSY is clear, and the
; task RUN loaded exactly when it is set.
SHBWT           MSK
                SMB     CONBSY
                LDW     CONBSY
                SAZ                     ; handed back yet?
                JMP     SHBWB
                JMP     SHBWD
SHBWB           SMB     KILLRQ
                LDW     KILLRQ
                SAZ                     ; a second Ctrl-C nobody took?
                JMP     SHBWK
                LDW     SHKWAI
                SMB     SH.STA
                STW     SH.STA
                SMB     SWTCH
                JSX     SWTCH
                JMP     SHBWT
SHBWD           UNM
                JMP     SHLOOP

; Two Ctrl-Cs and the program took neither: it is not listening, and the
; task that has the console is ended from here.
SHBWK           CLR
                SMB     KILLRQ
                STW     KILLRQ
                UNM
                LDW     SHKNAB
                STW     SHKN
                LDW     SHKNAN
                STW     SHTI
SHBWF           LDX     SHKN
                LDW     *T.STA
                CMW     SHKFRE
                SNE
                JMP     SHBWN
                LDW     *T.CON
                SAZ
                JMP     SHBWG
SHBWN           LDW     SHKN
                ADD     SHKTLN
                STW     SHKN
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ
                JMP     SHBWF
                JMP     SHBWT           ; it went of its own accord
SHBWG           JSX     SHKDO
                LDW     SHMKIL
                JSX     SHMSG
                JMP     SHLOOP

; LOAD name and RUN name: a module off the disc, started as a task.  RUN
; hands it the console and waits for it to be over -- the break flag
; cleared and CONBSY raised in one masked window, as the grant to BASIC
; is made, then SHBWT's wait -- and LOAD leaves it to run behind the
; prompt.  The task's node is linked in by SPAWN under its own mask,
; after the grant: the task cannot run before it is on the ring, and
; nothing but a task that holds the console can lower CONBSY.
SHRUN           LDW     SHK1
                JMP     SHLD0
SHLOAD          CLR
SHLD0           STW     LDCON
                CLR
                STW     SHIMPL
                LDW     SHKRX
                STW     SHDTY
                JSX     SHNAME          ; the name
SHLD1           JSX     SHIOC           ; the disc, for the whole load
                JSX     LDMOD
                STW     SHW2
                JSX     SHIOR
                LDW     SHW2
                SAZ                     ; a node, or an error already named?
                JMP     SHLD2
                JMP     SHLOOP
SHLD2           LDW     LDCON
                SAZ
                JMP     SHLDRN
                LDW     LDNODE
                SMB     SPAWN
                JSX     SPAWN
                JMP     SHLOOP
SHLDRN          MSK
                CLR
                SMB     BRKREQ
                STW     BRKREQ
                SMB     KILLRQ
                STW     KILLRQ
                LDW     SHK1
                SMB     CONBSY
                STW     CONBSY
                UNM
                LDW     LDNODE
                SMB     SPAWN
                JSX     SPAWN
                JMP     SHBWT

; ---------------------------------------------------------------- the loader
; Load the module SHFN names: find it in the directory and run its
; object text into a block from the pool, one record to a sector.  The
; text is the 1968 relocating loader's (tools/asm703.py's docstring lists
; the codes, tools/reload703.py is the reference reading of them): the
; SIZE code comes first and sizes the block, whose address is the
; relocation base; each repeatable code carries a run of words that are
; stored as they are, or with the base added into the 11-bit M field,
; the whole word, or twice for a byte address; a page selection is built
; here from the final address; END names the entry, which gets the base
; too.  Returns the node, with its entry, name and console flag filled
; in and the block tagged as the task's own, ready for SPAWN -- or
; zero, an error having been named: no such file, the disc's status or
; no directory on it (DE), a record failing its checksum (CK), a code
; this loader does not take or text out of order (LC), no room (MX).  An
; error path jumps straight out of whatever routine it was in, as
; RELOADB's does.  The caller has claimed the disc.
LDMOD           SUBR
                LDW     SHFN            ; a name at all?
                SAZ
                JMP     LDM1
                JMP     LDNOF
LDM1            CLR
                STW     LDBLK
                STW     LDNODE
                STW     LDBASE
                STW     LDPTR
                STW     LDLIM
                LDW     SHKFN
                SMB     FSFND
                JSX     FSFND
                SAZ                     ; no such file?
                JMP     LDM2
                JMP     LDNOF
LDM2            SAP                     ; an error is minus
                JMP     LDEDE
                CAX
                LDW     *DE.SEC         ; where it starts...
                STW     LDSEC
                LDW     *DE.CNT         ; ...and how many sectors
                STW     LDNSEC
                LDW     *DE.FLG         ; and whether it may load high
                AND     SHKDFH
                STW     LDHIGH
LDFND           MSK                     ; a node from the arena
                SMB     NALLOC
                JSX     NALLOC
                UNM
                SAZ
                JMP     LDF1
                JMP     LDEMX
LDF1            STW     LDNODE
                LDW     SHKDBL          ; on the checksum byte: the first
                STW     LDBP            ; GETBYTE reads the first record

; The text: a code byte, then what it says follows.  The byte arrives
; zero-extended, so its lead bit is tested against X'80', not the sign.
LDPROC          JSX     LDGETB
                STW     LDCODE
                CMW     SHK80
                SLS                     ; below X'80': a control code
                JMP     LDRPT
                JMP     LDCTL
LDRPT           SRL     4               ; the class: 8n..Cn
                SUB     SHK8
                CMW     SHK4
                SGR
                JMP     LDR1
                JMP     LDELC
LDR1            STW     LDCLS
                LDW     LDCODE
                AND     SHK0F
                STW     LDREP           ; n: n+1 words follow
LDRGO           JSX     LDGETW
                STW     LDW1
                LDW     LDCLS
                SAZ
                JMP     LDRC1
                LDW     LDW1            ; RELW11
                JSX     LDRL11
                JMP     LDRST
LDRC1           CMW     SHK1
                SNE
                JMP     LDRW15
                CMW     SHK2
                SNE
                JMP     LDRB16
                CMW     SHK3
                SNE
                JMP     LDRB11
                LDW     LDW1            ; ABSO
                JMP     LDRST
LDRW15          LDW     LDW1
                ADD     LDBASE
                JMP     LDRST
LDRB16          LDW     LDW1            ; a byte address: the base twice
                ADD     LDBASE
                ADD     LDBASE
                JMP     LDRST
LDRB11          LDW     LDW1            ; RELO11 twice, for the same reason
                JSX     LDRL11
                JSX     LDRL11
LDRST           JSX     LDSTOR
                LDW     LDREP
                SAZ                     ; the last of the run?
                JMP     LDRMO
                JMP     LDPROC
LDRMO           SUB     SHK1
                STW     LDREP
                JMP     LDRGO

; The control codes, through a table of where each goes.
LDCTL           LDW     LDCODE
                CMW     SHK0C
                SLS                     ; past the table?
                JMP     LDELC
                ADD     SHKCTB
                CAX
                LDW     *0
                CAX
                JMP     *0
LDCTAB          WORD    LDPROC,LDNAME,LDELC,LDSMB,LDILOC,LDELC
                WORD    LDEND,LDELC,LDNAME,LDSIZW,LDSIZW,LDSIZB

LDNAME          JSX     LDGETW          ; a name: four words, not wanted
                JSX     LDGETW
                JSX     LDGETW
                JSX     LDGETW
                JMP     LDPROC
LDSMB           JSX     LDGETW          ; SMB: the instruction, from the
                ADD     LDBASE          ; final address -- RELOADB's own
                SRL     10              ; ADD BASE / SRL 10 / ORI X'80'
                ORI     SHK80
                JSX     LDSTOR
                JMP     LDPROC
LDILOC          JSX     LDGETW          ; ILOC: that many zeros
                STW     LDCNT
LDIL1           LDW     LDCNT
                SAZ
                JMP     LDIL2
                JMP     LDPROC
LDIL2           SUB     SHK1
                STW     LDCNT
                CLR
                JSX     LDSTOR
                JMP     LDIL1
LDSIZW          LDW     SHKALW          ; SIZE: the block, from the allocator
                JMP     LDSIZ           ; entry the code names -- or the one
LDSIZB          LDW     SHKALB          ; after it in the table, its twin
LDSIZ           STW     LDALE           ; for a module that may load high
                LDW     LDHIGH
                SAZ
                JMP     LDSIZH
                JMP     LDSIZG
LDSIZH          LDW     LDALE
                ADD     SHK1
                STW     LDALE
LDSIZG          LDX     LDALE
                LDW     *0
                STW     LDALE
                LDW     LDBASE
                SAZ                     ; a second SIZE?
                JMP     LDELC
                JSX     LDGETW
                STW     LDSIZE
                LDX     LDALE
                JSX     *0
                SAZ
                JMP     LDS1
                JMP     LDEMX
LDS1            STW     LDBLK
                STW     LDBASE
                STW     LDPTR
                ADD     LDSIZE
                STW     LDLIM
                JMP     LDPROC
LDEND           JSX     LDGETW          ; END: the entry, relocated
                ADD     LDBASE
                STW     LDEXEC
                LDW     LDBLK
                SAZ                     ; without a SIZE first?
                JMP     LDDONE
                JMP     LDELC

; Loaded: fill the node, and tag the block as the new task's own, so
; that its exit finds it.
LDDONE          LDX     LDNODE
                LDW     LDEXEC
                STW     *T.PCR
                LDW     SHFN            ; the name's first two characters
                STW     *T.NAM
                LDW     LDCON
                STW     *T.CON
                LDW     LDNODE
                ORI     SHK8000
                STW     LDTAG
                LDW     LDBLK
                SUB     SHK1
                CAX
                LDW     LDTAG
                STW     *0

; The command tail: what is left of the line past the name, blanks
; skipped, in a block of the task's own with a zero after it.  T.ARG
; stays zero when there is nothing.
                JSX     SHSKB
                LDW     SHCUR
                STW     LDTP
LDTL            LDX     LDTP
                CLR
                LDB     *0
                SAZ
                JMP     LDTN
                JMP     LDTE
LDTN            LDW     LDTP
                ADD     SHK1
                STW     LDTP
                JMP     LDTL
LDTE            LDW     LDTP
                SUB     SHCUR
                SAZ                     ; anything?
                JMP     LDTA
                JMP     LDTX
LDTA            ADD     SHK2            ; its bytes and the zero, in words
                SRL     1
                SMB     ALLOC
                JSX     ALLOC
                SAZ
                JMP     LDTB
                JMP     LDEMX
LDTB            STW     LDARG
                SUB     SHK1
                CAX
                LDW     LDTAG
                STW     *0
                LDW     LDARG
                SLL     1
                STW     LDTP
LDTC            LDX     SHCUR
                CLR
                LDB     *0
                STW     LDW1
                LDX     LDTP
                STB     *0
                LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                LDW     LDTP
                ADD     SHK1
                STW     LDTP
                LDW     LDW1
                SAZ                     ; the zero went across too
                JMP     LDTC
                LDX     LDNODE
                LDW     LDARG
                STW     *T.ARG
LDTX            LDW     LDNODE
                EXIT    LDMOD

LDNOF           LDW     SHIMPL          ; a word that was neither a command
                SAZ                     ; nor a program
                JMP     LDNOFW
                LDW     SHMNOF
                JMP     LDERR
LDNOFW          LDW     SHMWHT
                JMP     LDERR
LDEDE           LDW     SHMEDE
                JMP     LDERR
LDECK           LDW     SHMECK
                JMP     LDERR
LDELC           LDW     SHMELC
                JMP     LDERR
LDEMX           LDW     SHMEMX
LDERR           JSX     SHMSG           ; name it, and give back what was taken
                LDW     LDBLK
                SAZ
                JMP     LDER1
                JMP     LDER2
LDER1           SMB     FREE
                JSX     FREE
LDER2           LDW     LDNODE
                SAZ
                JMP     LDER3
                JMP     LDER4
LDER3           MSK
                SMB     NFREE
                JSX     NFREE
                UNM
LDER4           CLR
                EXIT    LDMOD

; RELO11: the base added into the M field, the opcode and index bit kept.
LDRL11          SUBR
                STW     LDW3
                ADD     LDBASE
                AND     SHK7FF
                STW     LDW4
                LDW     LDW3
                AND     SHKF800
                ORI     LDW4
                EXIT    LDRL11

; Store the next word of the module, inside the block the SIZE declared.
LDSTOR          SUBR
                STW     LDW3
                LDW     LDBASE
                SAZ                     ; text before the SIZE?
                JMP     LDST1
                JMP     LDELC
LDST1           LDW     LDPTR
                CMW     LDLIM
                SLS                     ; room?
                JMP     LDEMX
                CAX
                LDW     LDW3
                STW     *0
                LDW     LDPTR
                ADD     SHK1
                STW     LDPTR
                EXIT    LDSTOR

; A word of text, high byte first; a byte, from the sector buffer, the
; next record read in when the pointer stands on the checksum byte.
LDGETW          SUBR
                JSX     LDGETB
                SLL     8
                STW     LDW2
                JSX     LDGETB
                ORI     LDW2
                EXIT    LDGETW

LDGETB          SUBR
                LDW     LDBP
                CMW     SHKDBL
                SNE                     ; on the checksum byte?
                JSX     LDGCRD
                LDX     LDBP
                CLR
                LDB     *0
                STW     LDBV
                LDW     LDBP
                ADD     SHK1
                STW     LDBP
                LDW     LDBV
                EXIT    LDGETB

; The next record: the next sector of the file, opening on a zero marker
; and closing on the folded byte sum -- (sum >> 8) + sum -- of everything
; before it.  Leaves the pointer on the first byte of text.
LDGCRD          SUBR
                LDW     LDNSEC
                SAZ                     ; the file ran out first
                JMP     LDGC1
                JMP     LDELC
LDGC1           SUB     SHK1
                STW     LDNSEC
                LDW     SHKLDB
                SMB     DXBUF
                STW     DXBUF
                LDW     LDSEC
                SMB     DRD1
                JSX     DRD1
                SAZ
                JMP     LDEDE
                LDW     LDSEC
                ADD     SHK1
                STW     LDSEC
                LDX     SHKDBB
                CLR
                LDB     *0
                SAZ                     ; the marker
                JMP     LDECK
                STW     LDSUM
                LDW     SHKDBB
                ADD     SHK1
                STW     LDBP
LDCKL           LDW     LDBP
                CMW     SHKDBL
                SLS
                JMP     LDCKT
                CAX
                CLR
                LDB     *0
                ADD     LDSUM
                STW     LDSUM
                LDW     LDBP
                ADD     SHK1
                STW     LDBP
                JMP     LDCKL
LDCKT           LDW     LDSUM
                SRL     8
                ADD     LDSUM
                AND     SHK0FF
                STW     LDCKV           ; its own cell: a refill can come
                LDX     LDBP            ; between the two halves of a word
                CLR                     ; LDGETW is holding in LDW2
                LDB     *0
                CMW     LDCKV
                SEQ                     ; the checksum byte agrees?
                JMP     LDECK
                LDW     SHKDBB
                ADD     SHK1
                STW     LDBP
                EXIT    LDGCRD

; The disc for the shell, and the disc given up: the claim every user of
; DXFER makes, a tick's sleep at a time until it succeeds.  A task's call
; through the vector makes it in KENTER; this is the shell's own, for the
; loader and the directory's readers, which read the disc from the
; shell's task.
SHIOC           SUBR
SHIOCL          MSK
                SMB     IOTRY
                JSX     IOTRY
                SAZ                     ; ours?
                JMP     SHIOCW
                UNM
                EXIT    SHIOC
SHIOCW          LDW     SHK1
                SMB     SH.DLY
                STW     SH.DLY
                LDW     SHKSLP
                SMB     SH.STA
                STW     SH.STA
                SMB     SWTCH
                JSX     SWTCH
                JMP     SHIOCL
SHIOR           SUBR
                LDW     SHKM1
                SMB     IOLOCK
                STW     IOLOCK
                EXIT    SHIOR

; DIR: the directory, a line to a file -- its name, its type and how
; many sectors it has -- and then the sectors that are free.  The disc is
; claimed only to bring the directory into core; the walk is of the
; kernel's copy, unclaimed, so that a listing at the teletype's pace does
; not hold every other task off the disc -- and a file made or deleted
; meanwhile may show in it twice or not at all.
SHDIR           JSX     SHDLD
                SAM                     ; an error is minus
                JMP     SHDR0
                CMW     SHKNDR
                SEQ
                JMP     SHDRE
                LDW     SHMNDR
                JSX     SHMSG
                JMP     SHLOOP
SHDR0           CLR
                STW     SHMTOT
                LDW     SHKFDE
                STW     LDCP
SHDRL           LDW     LDCP
                CMW     SHKFDX
                SLS                     ; inside the table?
                JMP     SHDRT
                CAX
                LDW     *DE.FLG
                AND     SHK0FF
                SAZ                     ; an unused entry ends it
                JMP     SHDR1
                JMP     SHDRT
SHDR1           CMW     SHKDKF
                SEQ                     ; a file?
                JMP     SHDRH
                JSX     SHPNAM
                LDW     SHKDOT
                JSX     SHPUTC
                LDX     LDCP
                LDW     *DE.TYP
                JSX     SHPW2
                LDW     SHKSP
                JSX     SHPUTC
                LDX     LDCP
                LDW     *DE.CNT
                JSX     SHDEC
                JSX     SHNL
                JMP     SHDRN
SHDRH           LDW     *DE.CNT         ; a hole, or what is being written
                ADD     SHMTOT
                STW     SHMTOT
SHDRN           LDW     LDCP
                ADD     SHK8
                STW     LDCP
                JMP     SHDRL
SHDRT           LDW     SHMTOT
                JSX     SHDEC
                LDW     SHMSFR
                JSX     SHMSG
                JMP     SHLOOP
SHDRE           LDW     SHMNRD
                JSX     SHMSG
                JMP     SHLOOP

; Bring the directory into the kernel's copy of it: what FSFND says,
; which is minus for an error.  The name it looks for is whatever SHFN
; holds, and beside the point.
SHDLD           SUBR
                JSX     SHIOC
                LDW     SHKFN
                SMB     FSFND
                JSX     FSFND
                STW     SHW2
                JSX     SHIOR
                LDW     SHW2
                EXIT    SHDLD

; The name of the directory entry LDCP is at.
SHPNAM          SUBR
                LDX     LDCP
                LDW     *0
                JSX     SHPW2
                LDX     LDCP
                LDW     *1
                JSX     SHPW2
                LDX     LDCP
                LDW     *2
                JSX     SHPW2
                EXIT    SHPNAM

; The words free in the pool, below the fence and above it: the free
; list's sizes added up, under the mask that every walk of the list
; holds.
SHMEM           MSK
                CLR
                STW     SHMTOT
                STW     SHMHI
                SMB     FREHD
                LDW     FREHD
SHMEML          SAZ                     ; the end of the list?
                JMP     SHMEM1
                JMP     SHMEMD
SHMEM1          CAX
                CMW     SHKPLH
                SLS                     ; above the fence?
                JMP     SHMEMH
                LDW     *0
                ADD     SHMTOT
                STW     SHMTOT
                JMP     SHMEMN
SHMEMH          LDW     *0
                ADD     SHMHI
                STW     SHMHI
SHMEMN          LDW     *1
                JMP     SHMEML
SHMEMD          UNM
                LDW     SHMFRE
                JSX     SHMSG
                LDW     SHMTOT
                JSX     SHDEC
                LDW     SHMLOW
                JSX     SHMSG
                LDW     SHMHI
                JSX     SHDEC
                LDW     SHMHIG
                JSX     SHMSG
                JMP     SHLOOP

; Shut the machine down.  The disc first, and kept: with it claimed here
; no task is inside a transfer.  Then every loaded task is stopped, so
; it deposits nothing more, and once their mailboxes are empty and the
; printer is idle nothing of theirs can appear inside the down-message.
; Each pass stops them over again, for one that something woke.
SHHALT          JSX     SHIOC
SHHDR           MSK
                CLR
                STW     SHW2            ; their mailboxes, ORed up
                LDW     SHKNAB
                STW     SHTO
                LDW     SHKNAN
                STW     SHTI
SHHD0           LDX     SHTO
                LDW     *T.STA
                CMW     SHKFRE
                SNE                     ; in use?
                JMP     SHHD2
                LDW     SHKOFF
                STW     *T.STA
                LDW     *T.MBX
                ORI     SHW2
                STW     SHW2
SHHD2           LDW     SHTO
                ADD     SHKTLN
                STW     SHTO
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ
                JMP     SHHD0
                UNM
                LDW     SHW2
                SAZ                     ; every one of them empty?
                JMP     SHHDR
                SMB     OWNER
                LDW     OWNER
                SAM                     ; and the printer idle?
                JMP     SHHDR
                LDW     SHMDWN
                JSX     SHMSG
SHHDW           SMB     SH.MBX          ; and now its own last character
                LDW     SH.MBX
                SAZ
                JMP     SHHDW
                SMB     OWNER
                LDW     OWNER
                SAM
                JMP     SHHDW
                MSK
                DOT     2,0             ; disconnect the line clock
                HLT

; ---------------------------------------------------------------- parsing
; Step SHCUR over blanks.
SHSKB           SUBR
SHSKL           LDX     SHCUR
                CLR
                LDB     *0
                CMW     SHKSP
                SEQ                     ; a blank?
                JMP     SHSKD
                LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                JMP     SHSKL
SHSKD           EXIT    SHSKB

; The word at SHCUR, its first four characters packed into STOK0:STOK1 and
; the rest of it stepped over.
SHTOK           SUBR
                CLR
                STW     STOK0
                STW     STOK1
                JSX     SHSKB
                LDW     SHK4
                STW     STKN
SHTKL           LDX     SHCUR
                CLR
                LDB     *0
                SAZ                     ; the terminator ends it
                JMP     SHTK1
                JMP     SHTKD
SHTK1           CMW     SHKSP           ; and so does a blank
                SEQ
                JMP     SHTK2
                JMP     SHTKD
SHTK2           STW     SHW2
                LDW     STKN            ; room for another character?
                SAZ
                JMP     SHTK3
                JMP     SHTKS
SHTK3           LDW     STOK0           ; shift the pair up one character
                LDX     STOK1
                SLLD    8
                STW     STOK0
                CXA
                ORI     SHW2
                STW     STOK1
                LDW     STKN
                SUB     SHK1
                STW     STKN
SHTKS           LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                JMP     SHTKL
SHTKD           EXIT    SHTOK

; The file name at SHCUR into SHFN, as the directory holds one: up to
; six characters, left-justified in three words and zero-filled, then
; the type, two characters after a dot -- or the type in SHDTY when the
; name has no dot.  Characters past the six, or the two, are stepped
; over.
SHNAME          SUBR
                CLR
                STW     SHFN
                STW     SHFN+1
                STW     SHFN+2
                LDW     SHDTY
                STW     SHFN+3
                JSX     SHSKB
                LDW     SHKFNB
                STW     SHNP
                LDW     SHK6
                STW     SHNN
SHNL1           LDX     SHCUR
                CLR
                LDB     *0
                SAZ                     ; the terminator ends it
                JMP     SHN1
                JMP     SHNX
SHN1            CMW     SHKSP           ; and so does a blank
                SNE
                JMP     SHNX
                CMW     SHKDOT          ; a dot starts the type
                SNE
                JMP     SHNDOT
                STW     SHW2
                LDW     SHNN            ; room for another character?
                SAZ
                JMP     SHN2
                JMP     SHN3
SHN2            SUB     SHK1
                STW     SHNN
                LDX     SHNP
                LDW     SHW2
                STB     *0
                LDW     SHNP
                ADD     SHK1
                STW     SHNP
SHN3            LDW     SHCUR
                ADD     SHK1
                STW     SHCUR
                JMP     SHNL1
SHNDOT          CLR
                STW     SHFN+3
                LDW     SHKFTB
                STW     SHNP
                LDW     SHK2
                STW     SHNN
                JMP     SHN3
SHNX            EXIT    SHNAME

; ---------------------------------------------------------------- output
; One character from ACR: deposit in the shell's mailbox, kick the
; printer, and stand down until SERV reports it printed.  The deposit
; window is masked for KICK's sake, and so is each look at the mailbox
; afterwards, so that the completion cannot land between the look and the
; decision to wait on it.
SHPUTC          SUBR
                AND     SHK0FF
                MSK
                SMB     SH.MBX
                STW     SH.MBX
                SMB     KICK
                JSX     KICK
                UNM
SHPWT           MSK
                SMB     SH.MBX
                LDW     SH.MBX
                SAZ                     ; printed yet?
                JMP     SHPWB
                JMP     SHPWD
SHPWB           LDW     SHKWAI
                SMB     SH.STA
                STW     SH.STA
                SMB     SWTCH
                JSX     SWTCH
                JMP     SHPWT
SHPWD           UNM
                EXIT    SHPUTC

; The two characters packed in ACR, high half first.
SHPW2           SUBR
                STW     SHW2P
                SRL     8
                JSX     SHPUTC
                LDW     SHW2P
                JSX     SHPUTC
                EXIT    SHPW2

; The message whose two-word descriptor ACR points at: first byte, then
; one past the last.
SHMSG           SUBR
                CAX
                LDW     *0
                STW     SHSP
                LDW     *1
                STW     SHSE
                JSX     SHPRT
                EXIT    SHMSG

SHPRT           SUBR
SHPRL           LDW     SHSP
                CMW     SHSE
                SNE
                JMP     SHPRD
                CAX
                CLR                     ; LDB replaces only the low half (2-1)
                LDB     *0
                JSX     SHPUTC
                LDW     SHSP
                ADD     SHK1
                STW     SHSP
                JMP     SHPRL
SHPRD           EXIT    SHPRT

SHNL            SUBR
                LDW     SHKCR
                JSX     SHPUTC
                LDW     SHKLF
                JSX     SHPUTC
                EXIT    SHNL

; ACR in decimal, on the hardware divide.  The digits come out backwards,
; so they are laid into a small buffer from its end and printed forwards.
; The 31-bit dividend is IXR:ACR with the sign duplicated into ACR bit 0,
; which is what the shift and the copy in front of the DIV build.
SHDEC           SUBR
                STW     SHV
                LDW     SHKDBE
                STW     SHDP
SHDL2           LDW     SHV
                SRL     15
                CAX
                LDW     SHV
                DIV     SHKTEN
                STW     SHV             ; the quotient
                CXA                     ; the remainder is the digit
                ADD     SHKZER
                LDX     SHDP
                STB     *0
                LDW     SHDP
                SUB     SHK1
                STW     SHDP
                LDW     SHV
                SAZ                     ; nothing left of it?
                JMP     SHDL2
                LDW     SHDP
                ADD     SHK1
                STW     SHSP
                LDW     SHKDBE
                ADD     SHK1
                STW     SHSE
                JSX     SHPRT
                EXIT    SHDEC

; Collect a line into the buffer.  Every character comes from the console
; queue, which puts this task to sleep until the teletype's service
; routine has one -- so the shell holds no processor at all between
; keystrokes, and a burst typed while it is busy waits in the queue
; instead of being lost.
;
; What a line is belongs here rather than in the driver: a carriage
; return or a line feed ends it, a rubout backs up over a character, and
; lower case is folded up because that is all the commands are written
; in. The rubout itself prints, since a printing terminal cannot take ink
; back; only the buffer forgets.
SHGETL          SUBR
                LDW     SHKLBB
                STW     SHFIL
                CLR
                STW     SHLFF
SHGL            LDW     SHCMCH
                SAZ                     ; a command file open?
                JMP     SHGLF
                LDW     SHKCQ
                SMB     Q.GET
                JSX     Q.GET
                JMP     SHGLC

; From the file: a character through its channel, given bit 7 in case
; the file came from a host that leaves it off.  A carriage return ends
; a line as it does from the keyboard; a line feed ends one that has
; something in it and is otherwise passed over, so either ending, or
; both, reads as one line.  The end of the file, or Ctrl-C, closes it
; and the keyboard has the shell again -- after the last line, if that
; had no ending of its own.
SHGLF           SMB     BRKREQ
                LDW     BRKREQ
                SAZ
                JMP     SHGLFX
                LDW     SHCMCH
                SMB     K.GETC
                JSX     K.GETC
                SAP
                JMP     SHGLFX
                ORI     SHK80
                STW     SHCH
                LDW     SHK1
                STW     SHLFF
                LDW     SHCH
                CLB     X'8A'
                SEQ
                JMP     SHGLC
                LDW     SHFIL
                CMW     SHKLBB
                SEQ
                JMP     SHGLE
                JMP     SHGL
SHGLFX          JSX     SHCMCL
                CLR
                SMB     BRKREQ
                STW     BRKREQ
                LDW     SHFIL
                CMW     SHKLBB
                SEQ
                JMP     SHGLE
                JMP     SHGL

SHGLC           STW     SHCH
                CLB     X'8D'           ; carriage return ends the line
                SNE
                JMP     SHGLE
                CLB     X'8A'           ; and so does a line feed, so a
                SNE                     ; script piped in with newline
                JMP     SHGLE           ; endings reads like a typed Return
                CLB     X'FF'           ; rubout
                SNE
                JMP     SHGLR
                CLB     X'E1'           ; below 'a'?
                SLS
                JMP     SHGLU
                JMP     SHGLS
SHGLU           CLB     X'FA'           ; above 'z'?
                SGR
                AND     SHKUPM          ; in range: clear bit 5
SHGLS           STW     SHCH
                LDW     SHFIL
                CMW     SHKLBE          ; room for one more?
                SNE
                JMP     SHGL            ; no: drop it
                CAX
                LDW     SHCH
                STB     *0
                LDW     SHFIL
                ADD     SHK1
                STW     SHFIL
                JMP     SHGL
SHGLR           LDW     SHFIL
                CMW     SHKLBB          ; anything to back up over?
                SEQ
                JMP     SHGLR1
                JMP     SHGL
SHGLR1          SUB     SHK1
                STW     SHFIL
                JMP     SHGL
SHGLE           LDW     SHFIL           ; terminate it; SHKLBE leaves room
                CAX
                CLR
                STB     *0
                LDW     SHLFF
                SAZ                     ; from a file: nothing has printed
                JMP     SHGLP           ; it, so print it as the keyboard's
                EXIT    SHGETL          ; echo would have
SHGLP           LDW     SHKLBB
                STW     SHSP
                LDW     SHFIL
                STW     SHSE
                JSX     SHPRT
                JSX     SHNL
                EXIT    SHGETL

; ---------------------------------------------------------------- shell data
SH.STA          EQU     SHTCB+T.STA     ; this task's own state word...
SH.MBX          EQU     SHTCB+T.MBX     ; ...and its own mailbox
SH.DLY          EQU     SHTCB+T.DLY     ; and its delay, for a sleep

SHCUR           WORD    0               ; the cursor into the line, a byte
SHFIL           WORD    0               ; and where SHGETL is filling it
SHCH            WORD    0               ; the character it is filing
SHTP            WORD    0               ; the command table cursor
SHTI            WORD    0               ; STAT's countdown over the nodes...
SHTO            WORD    0               ; ...and the node a walk is visiting
SHSA            WORD    0               ; the state name being printed
SHNST           WORD    0               ; the state STOP or START will store
STOK0           WORD    0               ; the command word, four characters
STOK1           WORD    0
STKN            WORD    0               ; how many of them are still wanted
SHFN            WORD    0,0,0,0,0       ; a file's name and type, as SHNAME leaves
                                        ; them, and the mode K.OPEN wants after
SHDTY           WORD    0               ; the type a name without one is given
SHIMPL          WORD    0               ; the name was the command word itself
SHCMCH          WORD    0               ; the command file's channel; zero: none
SHLFF           WORD    0               ; this line came from it
SHKN            WORD    0               ; the node a job number named
SHNV            WORD    0               ; the number
SHSS            WORD    0               ; STAT's one reading of a state
LDARG           WORD    0               ; the command tail's block
LDHIGH          WORD    0               ; nonzero: the module may load high
LDTP            WORD    0               ; and a byte pointer over the tail
SHNP            WORD    0               ; where SHNAME's next character goes...
SHNN            WORD    0               ; ...and how many more it will take
SHW2            WORD    0               ; scratch
SHW2P           WORD    0               ; SHPW2's, which SHPUTC must not touch
SHMTOT          WORD    0               ; MEM's running totals, below the
SHMHI           WORD    0               ; fence and above it
LDCON           WORD    0               ; the loader's: RUN (1) or LOAD (0)
LDBLK           WORD    0               ; the module's block, and its node
LDNODE          WORD    0
LDBASE          WORD    0               ; the relocation base: the block
LDPTR           WORD    0               ; where the next word goes, and the limit
LDLIM           WORD    0
LDSIZE          WORD    0               ; the SIZE code's word
LDEXEC          WORD    0               ; the entry
LDSEC           WORD    0               ; the next sector, and how many are left
LDNSEC          WORD    0
LDCP            WORD    0               ; DIR's cursor over the directory
LDBP            WORD    0               ; the byte pointer into the sector buffer
LDBV            WORD    0               ; the byte it fetched
LDSUM           WORD    0               ; the record's byte sum, and its fold
LDCKV           WORD    0
LDCODE          WORD    0               ; the code, its class and its count
LDCLS           WORD    0
LDREP           WORD    0
LDCNT           WORD    0               ; ILOC's count
LDALE           WORD    0               ; the allocator entry the SIZE code names
LDTAG           WORD    0               ; the owner tag for the task's blocks
LDW1            WORD    0               ; scratch
LDW2            WORD    0
LDW3            WORD    0
LDW4            WORD    0
SHSP            WORD    0               ; SHPRT's cursor and limit, bytes
SHSE            WORD    0
SHV             WORD    0               ; SHDEC's running value...
SHDP            WORD    0               ; ...and where its next digit goes
SHDB            RES     3               ; six digits, filled backwards

SHK1            WORD    1
SHK2            WORD    2
SHK3            WORD    3
SHK4            WORD    4
SHK6            WORD    6
SHK8            WORD    8
SHK10           WORD    10
SHK9P           WORD    '9'+1
SHKAT           WORD    '@'
SHKCM           WORD    'CM'
SHKIN0          WORD    'IN'            ; INIT, as SHNAME leaves a name
SHKIN1          WORD    'IT'
SHKTLN          WORD    T.LEN
SHKFRE          WORD    SFREE
SHKNAB          WORD    NARB            ; the arena, and how many nodes
SHKNAN          WORD    NARN
SHK0C           WORD    12              ; the control codes, 0..B
SHK0F           WORD    X'000F'
SHK80           WORD    X'0080'
SHK7FF          WORD    X'07FF'
SHKF800         WORD    X'F800'
SHK8000         WORD    X'8000'
SHK0FF          WORD    X'00FF'
SHKM1           WORD    X'FFFF'
SHKTEN          WORD    10
SHKZER          WORD    '0'
SHKSP           WORD    ' '
SHKDOT          WORD    '.'
SHKRX           WORD    'RX'            ; a module's type
SHKDKF          WORD    DK.FILE
SHKPLH          WORD    POOLH
SHKNDR          WORD    E.NDIR
SHKFN           WORD    SHFN            ; the name, as a word address...
SHKFNB          WORD    SHFN*2          ; ...its first byte...
SHKFTB          WORD    SHFN*2+6        ; ...and the type's
SHKFDE          WORD    FDENT           ; the directory's entries in core,
SHKFDX          WORD    FDEND           ; and one past the last
SHKCR           WORD    X'008D'
SHKLF           WORD    X'008A'
SHKSLP          WORD    SSLP
SHKOFF          WORD    SOFF
SHKWAI          WORD    SWAI
SHKUPM          WORD    X'FFDF'         ; folds a letter to upper case
SHKCQ           WORD    QCONS           ; the queue the keyboard fills
SHKSHN          WORD    SHTCB           ; the nodes that are always here, as
SHKIDL          WORD    IDTCB           ; they lie in core, and the idle node
SHKLBB          WORD    LBUF*2          ; the line, and the last byte its
SHKLBE          WORD    LBUF*2+62       ; zero terminator may need
SHKDBE          WORD    SHDB*2+5        ; the last byte of the digit buffer
SHKSTA          WORD    SHSTA
SHKTAB          WORD    SHTAB
SHKCTB          WORD    LDCTAB
SHKLDB          WORD    LDBUF           ; the sector buffer, as a word address...
SHKDBB          WORD    LDBUF*2         ; ...its first byte, the marker...
SHKDBL          WORD    LDBUF*2+93      ; ...and its last, the checksum
SHKALW          WORD    SHALW           ; the allocator's page entries, for
SHKALB          WORD    SHALB           ; an indexed JSX: each with its
SHALW           WORD    ALLOCW,ALLCHW   ; twin above the fence after it
SHALB           WORD    ALLOCB,ALLCHB
SHKDFH          WORD    DF.HIGH

; Four characters a state, indexed by the state doubled.  The last is a
; node that went back to the arena while its line was being printed.
SHSTA           WORD    'RU','N ','SL','P ','OF','F ','WA','IT','GO','NE'

LBUF            RES     32              ; the line the shell is reading
LDBUF           RES     47              ; the loader's sector

; The commands: four characters of name, packed as SHTOK packs a word --
; a short one lands in the second word, zero-filled above -- then where
; to go. '?' is HELP under another name, and UP is UPTIME under a shorter
; one.
SHTAB           WORD    'HE','LP',SHHELP
                WORD    0,'?',SHHELP
                WORD    'ST','AT',SHSTAT
                WORD    'UP','TI',SHUPT
                WORD    0,'UP',SHUPT
                WORD    'ST','OP',SHSTOP
                WORD    'ST','AR',SHSTRT
                WORD    'EC','HO',SHECHO
                WORD    'KI','LL',SHKILL
                WORD    'M','EM',SHMEM
                WORD    'D','IR',SHDIR
                WORD    'LO','AD',SHLOAD
                WORD    'R','UN',SHRUN
                WORD    'HA','LT',SHHALT
                WORD    0,0,0

SHMBAN          WORD    SHBAN
SHMHNT          WORD    SHHNT
SHMPRM          WORD    SHPRM
SHMWHT          WORD    SHWHT
SHMHL1          WORD    SHHL1
SHMHL2          WORD    SHHL2
SHMFRE          WORD    SHFRE
SHMNOF          WORD    SHNOF
SHMNRD          WORD    SHNRD
SHMNDR          WORD    SHNDR
SHMSFR          WORD    SHSFR
SHMSWP          WORD    SHSWP
SHMSWO          WORD    SHSWO
SHMSWI          WORD    SHSWI
SHMOUT          WORD    SHOUT
SHMLOW          WORD    SHLOW
SHMHIG          WORD    SHHIG
SHMEDE          WORD    SHEDE
SHMECK          WORD    SHECK
SHMELC          WORD    SHELC
SHMEMX          WORD    SHEMX
SHMUPM          WORD    SHUPM
SHMSEC          WORD    SHSEC
SHMDWN          WORD    SHDWN
SHMNOT          WORD    SHNOT
SHMKIL          WORD    SHKIL
SHMHL3          WORD    SHHL3

SHBAN           WORD    SHBANT*2,SHBANE*2
SHHNT           WORD    SHHNTT*2,SHHNTE*2
SHPRM           WORD    SHPRMT*2,SHPRME*2
SHWHT           WORD    SHWHTT*2,SHWHTE*2
SHHL1           WORD    SHHL1T*2,SHHL1E*2
SHHL2           WORD    SHHL2T*2,SHHL2E*2
SHFRE           WORD    SHFRET*2,SHFRET*2+5
SHNOF           WORD    SHNOFT*2,SHNOFE*2
SHNRD           WORD    SHNRDT*2,SHNRDE*2
SHNDR           WORD    SHNDRT*2,SHNDRE*2
SHSFR           WORD    SHSFRT*2,SHSFRE*2
SHSWP           WORD    SHSWPT*2,SHSWPT*2+6
SHSWO           WORD    SHSWOT*2,SHSWOT*2+5
SHSWI           WORD    SHSWIT*2,SHSWIT*2+5
SHOUT           WORD    SHOUTT*2,SHOUTT*2+4
SHLOW           WORD    SHLOWT*2,SHLOWT*2+5
SHHIG           WORD    SHHIGT*2,SHHIGT*2+7
SHEDE           WORD    SHEDET*2,SHEDEE*2
SHECK           WORD    SHECKT*2,SHECKE*2
SHELC           WORD    SHELCT*2,SHELCE*2
SHEMX           WORD    SHEMXT*2,SHEMXE*2
SHUPM           WORD    SHUPMT*2,SHUPME*2
SHSEC           WORD    SHSECT*2,SHSECE*2
SHDWN           WORD    SHDWNT*2,SHDWNE*2
SHNOT           WORD    SHNOTT*2,SHNOTE*2
SHKIL           WORD    SHKILT*2,SHKILE*2
SHHL3           WORD    SHHL3T*2,SHHL3E*2

SHBANT          TEXT    "REX 703 UP\r\n"
SHBANE          EQU     $
SHHNTT          TEXT    "TYPE HELP FOR THE COMMANDS\r\n"
SHHNTE          EQU     $
SHPRMT          TEXT    "REX>  "
SHPRME          EQU     $
SHWHTT          TEXT    "WHAT\r\n"
SHWHTE          EQU     $
SHHL1T          TEXT    "BUILT IN: HELP STAT UPTIME STOP START KILL ECHO MEM DIR LOAD RUN HALT \r\n"
SHHL1E          EQU     $
SHHL2T          TEXT    "STOP, START AND KILL TAKE A JOB NUMBER. @NAME RUNS A FILE OF COMMANDS.\r\n"
SHHL2E          EQU     $
SHHL3T          TEXT    "ANY OTHER WORD RUNS THE PROGRAM OF THAT NAME. ON THE DISC:\r\n"
SHHL3E          EQU     $
SHFRET          TEXT    "FREE  "        ; five characters of it
SHLOWT          TEXT    " LOW  "        ; and five of this
SHSWPT          TEXT    "SWAPS "
SHSWOT          TEXT    " OUT  "        ; five
SHSWIT          TEXT    " IN\r\n "      ; five
SHOUTT          TEXT    " OUT"
SHHIGT          TEXT    " HIGH\r\n "    ; and seven
SHNOFT          TEXT    "NO SUCH FILE\r\n"
SHNOFE          EQU     $
SHNRDT          TEXT    "DISC NOT READY\r\n"
SHNRDE          EQU     $
SHNDRT          TEXT    "NO DIRECTORY\r\n"
SHNDRE          EQU     $
SHSFRT          TEXT    " SECTORS FREE \r\n"
SHSFRE          EQU     $
SHEDET          TEXT    "LOAD ERROR: DE\r\n"
SHEDEE          EQU     $
SHECKT          TEXT    "LOAD ERROR: CK\r\n"
SHECKE          EQU     $
SHELCT          TEXT    "LOAD ERROR: LC\r\n"
SHELCE          EQU     $
SHEMXT          TEXT    "LOAD ERROR: MX\r\n"
SHEMXE          EQU     $
SHUPMT          TEXT    "UPTIME"
SHUPME          EQU     $
SHSECT          TEXT    " SEC\r\n"
SHSECE          EQU     $
SHDWNT          TEXT    "REX 703 DOWN\r\n"
SHDWNE          EQU     $
SHNOTT          TEXT    "NO SUCH TASK\r\n"
SHNOTE          EQU     $
SHKILT          TEXT    "KILLED\r\n"
SHKILE          EQU     $
