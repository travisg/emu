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
; The Model 33 echoes what is typed in hardware, so every keyboard is
; armed for it at start-up (DOT d,11) and nothing here echoes a character.
; The console's teletype is DIO device 14; teletypes 1, 2 and 3 are 12, 11
; and 10, on the console's interrupt level -- an invented configuration,
; as the line clock is, which src/dev/ray703.rs describes.
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
; * EACH TERMINAL IS A CONSOLE WITH ONE READER AT A TIME.  The console's
;   teletype and the three beside it each have a block in the terminal
;   table (TMT), and every task has its terminal (T.TTY): what it prints
;   goes to that one's printer, and its console is that one's.  The
;   block's TM.BSY says who reads it: the terminal's shell when it is
;   clear, the task RUN loaded when it is set.  RUN raises it in the same
;   masked window that clears the break flag, then waits on the cell
;   itself -- reading no queue -- until the task's K.EXIT clears it and
;   wakes the shell under SERV's guard.  Q.GET holds everybody else to
;   it: a terminal's queue answers its shell and a task of the terminal's
;   whose node says it has the console, and tells any other caller no.
;   Ctrl-C never enters the queue at all: SERV raises the terminal's
;   TM.BRK instead, so a running program that reads no input can still
;   be broken, and the grant clears the flag so a break aimed at nobody
;   cannot land on the session that follows it.  A second Ctrl-C that
;   finds the first still there raises TM.KIL and wakes the shell out of
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
;   a terminal's owner and last served, a queue's waiter -- holds the
;   node's address, a field
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
;              vector, then the exported cells
;   0040-      page 0: START; then ISRBEG..ISREND, which is SCHED, SERV,
;              KICK, PICK, SWTCH, Q.PUT and DSERV; then the kernel cells,
;              the console queue, the nodes of the tasks that are here
;              from power-on, the disc -- IOTRY, KENTER, DXFER -- Q.GET,
;              the idle task, the allocator, and the node arena's two
;              doors
;   0800-      page 1: the shell (shell.asm, the next file of this deck)
;              -- banner, prompt, commands, line buffer -- and its
;              loader, with the sector it reads
;   1000-6FFF  the pool, which the allocator hands out: what the shell
;              loads and its workspaces live there.  The fence is the
;              two words below 4000
;   7000-76FF  the filesystem (rexfs.asm, the next file of this deck):
;              the directory's routines, the channels and the bodies of
;              the file calls, then the four channels and the directory
;              itself
;   7800-7BFF  the swapper (rexswap.asm, the last): its task, its table
;              of extents in the swap file, and two sectors of block lists
;   7E00-7FAF  the node arena
;
; Build with make -C rex: asm703.py over rexmac.asm, this file, shell.asm,
; rexfs.asm and rexswap.asm, as one deck, into rex/build, with rexapi.asm, the
; interface the modules are assembled against, and the modules beside it
; -- brex.asm over test/703/bcore.asm is basic.obj, the module RUN BASIC
; loads.  make -C rex disc puts them on the platter, and
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
; there: a jump vector, then the cells a module may read.  The JMP at an
; entry carries the caller's link into the routine, whose SUBR takes it
; as from any caller.  An entry with nothing behind it is a zero word, so
; a call to it halts.
;
; The NTRY cards from here down to the file calls' answers are that
; interface as a module sees it.  The build writes their names out as
; build/rexapi.asm, the file every module deck begins with, each NTRY
; card's under the comment that stands over it and each name with the
; comment its own card carries (asm703.py --exports).  So those comments
; are written for a module's author, and what only the kernel wants to
; know of a name is in a comment of its own above the card.

; The REX kernel interface, for what loads under it.
;
; A module is assembled from word 0 and placed by the loader wherever the
; allocator finds room -- each of its segments, if it has several, in a
; block of its own -- so it can know nothing of the kernel's layout
; except what is here: a jump vector at fixed words, the cells the kernel
; exports, the shape of a task node and the values that go in it.
;
; A module reaches an entry with an adjacent SMB/JSX pair, which is what
; rexmac.asm's CALL writes, and a cell with an adjacent SMB/LDW or
; SMB/STW pair --
;
;       SMB     K.QGET
;       JSX     K.QGET
;
; which assemble to absolute words with a page selection in front, so
; nothing is linked at load time.  JSX leaves the link in IXR and forces
; global mode; the vector's JMP carries it into the kernel, and the
; kernel's EXIT returns through it to wherever the module lies.
;
; A task runs global -- its status word says so -- and gives the processor
; up either through K.SWTCH, after marking its own node sleeping or
; waiting in one masked window, or for good through K.EXIT, which frees
; everything the task allocated, its node included, and never returns.
;
; The vector, at X'10'-X'1B'.
                NTRY    K.ALLOC,K.ALLCW,K.ALLCB,K.FREE,K.EXIT,K.SWTCH
                NTRY    K.KICK,K.QGET,K.DREAD,K.DWRIT,K.ALLHI,K.ALHIW
                NTRY    K.BRKQ,K.BRKC
                ORG     X'10'
K.ALLOC         JMP     ALLOC           ; ACR = words wanted -> ACR = the block, or 0.
                                        ; These three hand out core below word
                                        ; X'4000' only
K.ALLCW         JMP     ALLOCW          ; ...contained in one 2048-word page
K.ALLCB         JMP     ALLOCB          ; ...contained in one 1024-word byte page
K.FREE          JMP     FREE            ; ACR = a block from K.ALLOC
K.EXIT          JMP     TEXIT           ; the task is over; never returns
K.SWTCH         JMP     SWTCH           ; hand the processor on; returns when next picked
K.KICK          JMP     KICKC           ; start the printer on the next full mailbox
K.QGET          JMP     Q.GET           ; ACR = a queue descriptor -> ACR = the next word
K.DREAD         JMP     KDREAD          ; ACR = a transfer block -> ACR = status: disc to core
K.DWRIT         JMP     KDWRIT          ; the same, core to disc.
                                        ; A transfer block is three words of
                                        ; the caller's own: the first sector's
                                        ; index, track*128+sector; the word
                                        ; count, up to 16,383, which may run
                                        ; across sectors and tracks; and the
                                        ; core address.  Status zero is clean.
                                        ; The call waits its turn at the disc.
K.ALLHI         JMP     ALLOCH          ; K.ALLOC and K.ALLCW for a caller that
K.ALHIW         JMP     ALLCHW          ; compares no byte addresses in what it
                                        ; gets: above word X'4000' if there is
                                        ; room there, below if not.  A byte
                                        ; address up there is negative as a
                                        ; signed number, which is the only kind
                                        ; of compare this machine has; a byte
                                        ; is still reached through the index.
K.BRKQ          JMP     BRKQ            ; -> ACR = nonzero if a Ctrl-C is waiting on
                                        ; this task's terminal.  The flag is the
                                        ; terminal's, so it is this task's to
                                        ; read only while it holds the console
                                        ; (T.CON)
K.BRKC          JMP     BRKC            ; spend it: the flag cleared
                WORD    0,0             ; X'1E'-X'1F'

; The cells, at X'20'-X'23', the block of level 8.
                NTRY    CURT,KCONSQ
                ORG     X'20'
; CURT starts on the shell's node, since the kernel becomes the shell.
CURT            WORD    SHTCB           ; the running task's node
                WORD    0,0
KCONSQ          WORD    QCONS           ; the console queue's descriptor address,
                                        ; which K.QGET takes to mean the queue
                                        ; of the caller's own terminal.  It is
                                        ; for the console's owner alone: a
                                        ; queue holds one waiter.  RUN grants
                                        ; the console and fills in T.CON; LOAD
                                        ; grants nothing, and a task without it
                                        ; gets X'FFFF' from K.QGET at once.  A
                                        ; module that reads the console tests
                                        ; T.CON at entry and says so when it is
                                        ; clear.

; Files: the vector goes on at X'28'-X'2F'.  A name is four words, as the
; directory holds one: six characters and then two of type, bit 7 set,
; each left-justified and zero-filled -- HELLO.RX is 'HE','LL',X'CF00',
; 'RX'.  A file is read or written a character at a time through a
; channel, of which the kernel has four; what a task leaves open is let
; go when it exits, and a file it was writing and never closed is gone.
; A new file replaces an older one of its name when it is closed, not
; before, and has half of the largest run of free sectors to grow into.
; Every answer that is not what was asked for is one of the E. values
; below, all minus.
                NTRY    K.LOOK,K.OPEN,K.GETC,K.PUTC,K.CLOSE,K.DELET,K.RENAM,K.SQUSH
                ORG     X'28'
; Each goes through KENTER to its body in rexfs.asm.
K.LOOK          JMP     KLOOK           ; ACR = eight words, the first four a name
                                        ; -> zero and the last four filled in --
                                        ; DE.SEC, DE.CNT, DE.LST, DE.FLG -- or E.
K.OPEN          JMP     KOPEN           ; ACR = five words: a name, then zero to
                                        ; read or one to write -> a channel, or E.
                                        ; E.PROT to write under a system file's name
K.GETC          JMP     KGETC           ; ACR = a channel -> the next character,
                                        ; or E.NOF at the end of the file
K.PUTC          JMP     KPUTC           ; ACR = channel*256 + character -> zero, or E.,
                                        ; and after one E. the same for the rest
K.CLOSE         JMP     KCLOSE          ; ACR = a channel -> zero, or E.  A file
                                        ; written replaces an older one of its name
                                        ; here; one K.PUTC refused a character of
                                        ; is given up instead, with K.PUTC's answer
K.DELET         JMP     KDELET          ; ACR = a name -> zero, or E.
K.RENAM         JMP     KRENAM          ; ACR = eight words, the old name and the
                                        ; new -> zero, or E.
K.SQUSH         JMP     KSQUSH          ; -> zero, or E.: every file moved down
                                        ; over the hole in front of it.  E.CHN
                                        ; while any channel is open.

L0PC            EQU     0               ; the level 0 block words SERV edits
L0ST            EQU     2               ; when it returns as another task
L1PC            EQU     4               ; and level 1's, which DSERV edits
L1ST            EQU     6
L2PC            EQU     8               ; the level 2 block words SCHED edits:
L2ST            EQU     10              ; rewriting them before INR 2 is the switch
L3PC            EQU     12              ; and the level 3 words SWTCH edits,
L3ST            EQU     14              ; for the same reason, before INR 3

; A task control block, one node on a circular linked list.  Everything
; that names a task -- CURT, a terminal's owner, a queue's waiter -- names it by
; the node's address, and a field is reached by putting that address in
; the index register and using the field offset as the displacement, so a
; node may live anywhere in core.  The ring is what the scans walk;
; adding a task is linking a node in under MSK.
                NTRY    T.STA,T.NXT,T.ACR,T.IXR,T.PCR,T.MST,T.DLY,T.MBX,T.NAM,T.CON
                NTRY    T.ID,T.KLK,T.KAR,T.KDS,T.ARG,T.SWP,T.MW,T.RES,T.TTY,T.LEN
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
T.CON           EQU     9               ; nonzero: this task holds the console --
                                        ; RUN set it, LOAD did not, and it is
                                        ; what says whether reading KCONSQ is
                                        ; this task's to do
T.ID            EQU     10              ; the job number, fixed for the node;
                                        ; zero for the tasks that are here
                                        ; from power-on, which no command
                                        ; can name
T.KLK           EQU     11              ; the kernel's, for a call this task is
T.KAR           EQU     12              ; blocked in: its way back, its argument
T.KDS           EQU     13              ; and its body -- see KENTER
T.ARG           EQU     14              ; the command tail: the word address of
                                        ; what followed the program's name on
                                        ; its line, a zero byte after it -- or
                                        ; zero, if nothing did
T.SWP           EQU     15              ; where the task's blocks are: zero,
                                        ; in core; minus one, on their way
                                        ; out or in; else on the disc, one
                                        ; more than the swap extent's sector
T.MW            EQU     16              ; the kernel's, while the task waits for
                                        ; core: 1 asking, 2 told there is
                                        ; nobody to put out, 3 told to try
                                        ; again
T.RES           EQU     17              ; the second its blocks last arrived
T.TTY           EQU     18              ; the task's terminal: its block in the
                                        ; kernel's terminal table, which is
                                        ; where what it prints goes and what
                                        ; its console is
T.LEN           EQU     19              ; words in a node

; What T.STA holds.
                NTRY    SRUN,SSLP,SOFF,SWAI
SRUN            EQU     0
SSLP            EQU     1
SOFF            EQU     2               ; suspended by the shell's STOP
SWAI            EQU     3               ; blocked on a queue or a device
SFREE           EQU     4               ; an arena node nobody is using

; A queue: a ring of words, a count, and the one task waiting on it.
; Words rather than characters because the next thing to go through one
; is a message between tasks.
                NTRY    Q.HEAD,Q.TAIL,Q.CNT,Q.CAP,Q.BUF,Q.WTR,QW
Q.HEAD          EQU     0               ; where the next one comes from
Q.TAIL          EQU     1               ; where the next one goes
Q.CNT           EQU     2
Q.CAP           EQU     3
Q.BUF           EQU     4               ; word address of the ring
Q.WTR           EQU     5               ; the node waiting, or -1
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
                NTRY    DE.NAM,DE.TYP,DE.SEC,DE.CNT,DE.LST,DE.FLG,DE.LEN
                NTRY    DK.FILE,DF.HIGH,DF.SYS
DE.NAM          EQU     0               ; the name: three words, six characters
DE.TYP          EQU     3               ; the type: one word, two more
DE.SEC          EQU     4               ; the first sector
DE.CNT          EQU     5               ; how many sectors
DE.LST          EQU     6               ; bytes used in the last one
DE.FLG          EQU     7               ; what it is in the low byte, DK.FILE to
                                        ; a module, and what is special about
                                        ; it above that: DF.HIGH, DF.SYS
DE.LEN          EQU     8
DK.HOLE         EQU     1               ; DE.FLG's low byte for a hole
DK.FILE         EQU     2               ; DE.FLG's low byte for a file
DK.TENT         EQU     3               ; and for one being written; zero is unused
DF.HIGH         EQU     X'0100'         ; a module that may load above X'4000'
DF.SYS          EQU     X'0200'         ; never moved, renamed or deleted

; What a file call answers when it cannot do what was asked: all minus.
                NTRY    E.NOF,E.CHN,E.DFUL,E.FULL,E.DISC,E.NDIR,E.PROT,E.EXST
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
                LDW     =POOLB          ; the pool: a free block below the
                STW     FREHD           ; fence, the fence, and a free
                CAX                     ; block above it
                LDW     =KPOOLN
                STW     *0
                LDW     =POOLH
                STW     *1
                LDX     =FENCE
                CLR
                STW     *0
                LDW     =X'8000'
                STW     *1
                LDX     =POOLH
                LDW     =KPOOHN
                STW     *0
                CLR
                STW     *1
                STW     NFREH           ; the node arena: every node onto
                LDW     =NARN           ; the free list, numbered, from the
                STW     NACNT           ; top down so the first one handed
                LDW     =NARB+(NARN-1)*T.LEN ; out is the lowest
.R              CAX
                LDW     NFREH
                STW     *T.NXT
                CXA
                STW     NFREH
                LDW     NACNT
                ADD     =NARID
                SUB     =1
                STW     *T.ID
                LDW     =SFREE
                STW     *T.STA
                LDW     NACNT
                SUB     =1
                STW     NACNT
                JNZ     .R1             ; the last of them?
                JMP     .R2
.R1             LDW     NFREH
                SUB     =T.LEN
                JMP     .R
.R2             LDW     =TMT            ; select every terminal's keyboard;
.R3             STW     SVTM            ; function 11 is the one that echoes,
                CAX                     ; which is the Model 33 printing what
                LDW     *TM.DKB         ; its own keyboard sent -- full
                STW     .R4             ; duplex, and free of the printer's
.R4             WORD    0               ; time.  A teletype that is not there
                LDW     SVTM            ; takes no notice
                ADD     =TM.LEN
                CMW     =TMTE
                JNE     .R3
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
                ADD     =1
                CMW     =60
                JGE     .SEC            ; a whole second of them?
                STW     TICKS
                JMP     .TK0
.SEC            CLR
                STW     TICKS
                LDW     SECS
                ADD     =1
                STW     SECS
.TK0            LDW     =SHTCB          ; once round the ring
                STW     SCIX
                LDW     KNRING
                STW     SCTRY
.TKL            LDX     SCIX
                LDW     *T.STA
                CMW     =SSLP
                JNE     .TKN            ; asleep?
                LDW     *T.DLY
                SUB     =1
                STW     *T.DLY
                JNZ     .TKN            ; the last tick of the sleep?
                CLR
                STW     *T.STA          ; wake it
.TKN            LDX     SCIX
                LDW     *T.NXT
                STW     SCIX
                LDW     SCTRY
                SUB     =1
                STW     SCTRY
                JNZ     .TKL            ; another node to visit?
                JMP     .DEF

.DEF            LDW     L2PC            ; where did the tick land?
                CMW     =ISRBEG
                JGE     .HI             ; below ISRBEG: task code
                JMP     .SW
.HI             CMW     =ISREND
                JGE     .SW             ; inside [ISRBEG,ISREND): service code
                                        ; above it: task code
                LDW     S2SAVA          ; defer -- restore untouched and let
                LDX     S2SAVX          ; a later tick do the switch
                INR     2

; A task was running: park its frame, pick the next, resume it.  The
; compare indicators and any overflow this arithmetic sets are clobber
; without consequence -- INR 2 restores the whole status from word 10.
.SW             LDX     CURT            ; IXR = the current task's node
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
; The teletypes.  They share the level, and the level says nothing of
; which of them interrupted, so the routine asks each one's status in turn
; and serves every one whose interrupt is up -- a pulse from one that
; arrives while another is being served holds the level, and brings the
; routine round again.
;
; One line serves both of a teletype's directions, so for each the
; routine decides what happened by what it started, the period driver's
; way: a node in the terminal's owner cell means a character was
; printing, so this is its completion.  The completion path falls into
; the keyboard check because the two events merge into one interrupt
; when they coincide (the demo's discipline).  A keystroke arriving
; *while* a character prints takes the completion path early: the
; mailbox is cleared and the next DOT queues behind the busy printer,
; which only tells the owner "printed" a tenth of a second soon and
; never reorders anything.
SERV            SMB     S0SAVA          ; the SMB lead again
                STW     S0SAVA
                STX     S0SAVX
                LDW     =TMT
.TM             STW     SVTM            ; each terminal in turn
                CAX
                LDW     *TM.DST
                STW     .ST
.ST             WORD    0               ; DIN d,0: its status
                AND     =X'0100'
                JNZ     .HIT            ; interrupting?
                JMP     .NXT
.HIT            LDX     SVTM
                LDW     *TM.OWN
                JPL     .TX0            ; no owner: a keystroke should wait
                JMP     .RX
.TX0            CAX                     ; the owner's node
                CLR
                STW     *T.MBX          ; the owner's "printed" signal

; ...and wake it, if it is waiting for exactly that.  Only if: a task the
; shell stopped between depositing and this completion must stay stopped,
; and the wake is advice rather than a promise -- the waiter looks at its
; own mailbox again when it runs, and finds it empty either way.
                LDW     *T.STA
                CMW     =SWAI
                JNE     .TX1            ; waiting on the printer?
                CLR
                STW     *T.STA
                LDW     =1
                STW     RESCHD
.TX1            LDX     SVTM
                LDW     =X'FFFF'
                STW     *TM.OWN
                LDW     SVTM
                JSX     KICK            ; start the next waiting character
.RX             LDX     SVTM
                LDW     *TM.DCL
                STW     .CL
.CL             WORD    0               ; DIN d,15: collect the frame, and
                JNZ     .RX1            ; ask for another; empty is the
                                        ; merge's other half, not an error
                JMP     .NXT

; Post the character to the terminal's queue and have done with it.  The
; driver keeps no line: what a line is -- where it ends, what a rubout
; does to it, which case it is in -- is the shell's business, and this
; routine's is to get the character off the teletype.  Nothing is echoed
; here either; the Model 33 is armed to print its own keyboard.
.RX1            STW     QITEM
                CLB     X'83'           ; Ctrl-C is a flag, not a queued
                JEQ     .RXBRK          ; character: nothing reads the queue
                                        ; while a program holds the console,
                LDX     SVTM            ; so an in-band break could never be
                LDW     *TM.Q           ; seen.  Whoever consumes the flag
                JSX     Q.PUT           ; clears it.
                JMP     .NXT
.RXBRK          LDX     SVTM
                LDW     *TM.BRK
                JNZ     .RXKIL          ; one already there that nobody took?
                LDW     =1
                STW     *TM.BRK
                JMP     .NXT
.RXKIL          LDW     =1              ; then this one is for the shell: the
                STW     *TM.KIL         ; program is not listening
                LDW     *TM.SH
                JNZ     .RXK1           ; a shell to tell?
                JMP     .NXT
.RXK1           CAX
                LDW     *T.STA
                CMW     =SWAI
                JNE     .NXT            ; only out of a wait, like any wake
                CLR
                STW     *T.STA
                LDW     =1
                STW     RESCHD
.NXT            LDW     SVTM
                ADD     =TM.LEN
                CMW     =TMTE
                JEQ     .EXIT           ; the last terminal?
                JMP     .TM

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
.EXIT           LDW     RESCHD
                JNZ     .EXSW           ; anything newly runnable?
                JMP     .EXPL
.EXSW           CLR
                STW     RESCHD
                MSK
                LDW     L0PC
                CMW     =ISRBEG
                JGE     .EXHI
                JMP     .EXDO
.EXHI           CMW     =ISREND
                JGE     .EXDO
                JMP     .EXPU           ; in the range: not a task's frame
.EXDO           LDX     CURT
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
.EXPU           UNM
.EXPL           LDW     S0SAVA
                LDX     S0SAVX
                INR     0

; Start the printer of the terminal whose block ACR holds on the next
; occupied mailbox of a task of that terminal's, round robin from the one
; served last, or leave it idle if every one is empty.  The owner is
; claimed before the DOT because the completion can arrive on the very
; next instruction (the disc exerciser's flag-before-DOT rule).
KICK            SUBR
                STW     KTM
                CAX
                LDW     *TM.OWN
                JPL     .DONE           ; still printing? the completion
                                        ; will call back here
                LDW     KNRING
                STW     KTRY
                LDW     *TM.LST
.SCN            CAX                     ; the next node round the ring
                LDW     *T.NXT
                CAX
                STW     KCAND
                LDW     *T.TTY
                CMW     KTM
                JNE     .NXT            ; this terminal's task?
                LDW     *T.MBX          ; its mailbox
                JNZ     .HIT
.NXT            LDW     KTRY            ; empty; any candidates left?
                SUB     =1
                STW     KTRY
                JNZ     .NX2
                JMP     .DONE           ; all empty: the printer stays idle
.NX2            LDW     KCAND
                JMP     .SCN
.HIT            LDX     KTM
                LDW     KCAND
                STW     *TM.OWN
                STW     *TM.LST
                LDW     *TM.DWR
                STW     .WR
                LDX     KCAND
                LDW     *T.MBX
.WR             WORD    0               ; DOT d,14: the character
.DONE           EXIT    KICK

; K.KICK: KICK for the running task's own terminal.  Tasks call it under
; MSK, which is what keeps it to one activation.
KICKC           SUBR
                LDX     CURT
                LDW     *T.TTY
                JSX     KICK
                EXIT    KICKC

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
.SCN            CAX
                STW     SCIX
                LDW     *T.STA
                JNZ     .NRD            ; runnable?
                LDW     *T.SWP
                JNZ     .OUT            ; and its blocks in core?
                JMP     .PIK
.OUT            LDX     =SWTCB          ; no: that is the swapper's to mend,
                LDW     *T.STA          ; so it is woken -- only out of its
                CMW     =SWAI           ; wait -- and the scan goes on
                JNE     .NRD
                CLR
                STW     *T.STA
.NRD            LDW     SCTRY
                SUB     =1
                STW     SCTRY
                JNZ     .NX2            ; any candidate left to look at?
                LDW     =IDTCB          ; nobody can run: go idle
                STW     SCIX
                JMP     .PIK
.NX2            LDX     SCIX
                LDW     *T.NXT
                JMP     .SCN
.PIK            LDW     SCIX
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
                AND     =KPGMSK         ; the status it resumes with: the page
                SLL     1               ; that address lies in, and global.
                ORI     =KGLB           ; The indicators are not carried -- a
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
                JEQ     .X              ; full?
                LDW     *Q.BUF
                ADD     *Q.TAIL
                CAX
                LDW     QITEM
                STW     *0
                LDX     QPD
                LDW     *Q.TAIL
                ADD     =1
                CMW     *Q.CAP
                SLS
                CLR                     ; round the ring
                STW     *Q.TAIL
                LDW     *Q.CNT
                ADD     =1
                STW     *Q.CNT
                LDW     *Q.WTR          ; anybody asleep on it?
                JPL     .WK
                JMP     .X
; Wake it only if it is waiting -- the same guard SERV's printer wake
; makes, for the same reason: a keystroke must not restart a task the
; shell has stopped between registering as the waiter and this put.  The
; registration is consumed either way; a stopped task re-registers when
; it next runs, because Q.GET looks again on every wake.
.WK             CAX                     ; the waiter's node
                LDW     *T.STA
                CMW     =SWAI
                JNE     .W2             ; waiting on the queue?
                CLR
                STW     *T.STA          ; wake it...
                LDW     =1              ; ...and ask the service routine to
                STW     RESCHD          ; return as whoever can run now
.W2             LDX     QPD             ; forget the waiter -- a queue holds
                LDW     =X'FFFF'        ; one, which is all a single reader
                STW     *Q.WTR          ; ever needs
.X              EXIT    Q.PUT

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
                JPL     .SWK            ; anybody's transfer?
                JMP     .EXIT
.SWK            CAX
                LDW     *T.STA
                CMW     =SWAI
                JNE     .EXIT           ; waiting on it?
                CLR
                STW     *T.STA
                LDW     =1
                STW     RESCHD
.EXIT           LDW     RESCHD
                JNZ     .EXSW
                JMP     .EXPL
.EXSW           CLR
                STW     RESCHD
                MSK
                LDW     L1PC
                CMW     =ISRBEG
                JGE     .EXHI
                JMP     .EXDO
.EXHI           CMW     =ISREND
                JGE     .EXDO
                JMP     .EXPU           ; in the range: not a task's frame
.EXDO           LDX     CURT
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
.EXPU           UNM
.EXPL           LDW     S1SAVA
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
SVTM            WORD    0               ; the terminal SERV is serving
KTM             WORD    0               ; and the one KICK is starting
KCAND           WORD    0               ; KICK's scan scratch
KTRY            WORD    0
BKRET           WORD    0               ; BRKQ's and BRKC's way back
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
K47             EQU     47              ; words in a sector
K3FFF           EQU     X'3FFF'         ; a transfer's word count; the unit is above it

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

; The terminals: the console's teletype and the three beside it on the
; same interrupt level, one block each.  The four instructions that drive
; a teletype carry its device code in the word, and there is no execute
; instruction, so each block holds them ready-made and the driver copies
; the one it wants into the word in front of it and runs on into it --
; X-RAY's way with a device it learns at run time.  The other three
; terminals' queues are shallower than the console's: they are typed at
; by a person, not by a command file run ahead of the shell.
                NTRY    TM.OWN,TM.BSY,TM.SH,TM.LEN,TMT,TMTE
TM.DST          EQU     0               ; DIN d,0: status, 0100 an interrupt
TM.DCL          EQU     1               ; DIN d,15: the frame, and ask for the next
TM.DWR          EQU     2               ; DOT d,14: print
TM.DKB          EQU     3               ; DOT d,11: select the keyboard, echoing
TM.OWN          EQU     4               ; node whose character is printing;
                                        ; minus one means the printer is idle
TM.LST          EQU     5               ; last node served, for fairness
TM.Q            EQU     6               ; the terminal's input queue
TM.BSY          EQU     7               ; the console belongs to a task, not the
                                        ; shell: the shell sets it as it grants
                                        ; it and waits until the task clears it
TM.BRK          EQU     8               ; Ctrl-C arrived; the console's holder
                                        ; clears it
TM.KIL          EQU     9               ; a second Ctrl-C came with the first
                                        ; still untaken: the shell's to act on
TM.SH           EQU     10              ; the terminal's shell, or zero
TM.LEN          EQU     11
TMQN            EQU     32              ; the other terminals' queue depth
TMT             DIN     14,0            ; the console
                DIN     14,15
                DOT     14,14
                DOT     14,11
                WORD    X'FFFF',SHTCB,QCONS,0,0,0,SHTCB
                DIN     12,0            ; teletype 1
                DIN     12,15
                DOT     12,14
                DOT     12,11
                WORD    X'FFFF',SHTCB,TMQ1,0,0,0,0
                DIN     11,0            ; teletype 2
                DIN     11,15
                DOT     11,14
                DOT     11,11
                WORD    X'FFFF',SHTCB,TMQ2,0,0,0,0
                DIN     10,0            ; teletype 3
                DIN     10,15
                DOT     10,14
                DOT     10,11
                WORD    X'FFFF',SHTCB,TMQ3,0,0,0,0
TMTE            EQU     $
TMQ1            WORD    0,0,0,TMQN,TMQ1B,X'FFFF'
TMQ2            WORD    0,0,0,TMQN,TMQ2B,X'FFFF'
TMQ3            WORD    0,0,0,TMQN,TMQ3B,X'FFFF'
TMQ1B           RES     TMQN
TMQ2B           RES     TMQN
TMQ3B           RES     TMQN
KNRING          WORD    NRING
KPGMSK          EQU     X'7C00'         ; the page bits of a word address, which
                                        ; doubled are a status word's EXR field
KGLB            EQU     X'0080'         ; and a status word's global bit

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
;                    ID KLK KAR KDS ARG SWP MW RES TTY
SHTCB           WORD SRUN, SWTCB,0,  0,  0,     0,     0,  0,  'SH',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0,  TMT
SWTCB           WORD SRUN, SHTCB,0,  0,  SWAPR, SWAPST,0,  0,  'SW',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0,  TMT
IDTCB           WORD SRUN, SHTCB,0,  0,  IDLE,  X'80', 0,  0,  'ID',0
                WORD 0, 0,  0,  0,  0,  0,  0, 0,  TMT

; The disc has one user at a time, and IOLOCK says who.  Everything that
; starts a transfer holds it first -- a task's call through the vector,
; the shell's loader -- so DXFER and whatever is built on it run one
; activation at a time and may keep their state in static cells like any
; other routine here, across the wait for the completion too.  IOTRY is
; the claim, for a caller that holds the mask: zero and the disc is the
; running task's, anything else and it is somebody's already.
IOTRY           SUBR
                LDW     IOLOCK
                JPL     .BSY            ; free?
                LDW     CURT
                STW     IOLOCK
                CLR
                EXIT    IOTRY
.BSY            LDW     IOWAITS
                ADD     =1
                STW     IOWAITS
                LDW     =1
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
                JNZ     .WAIT           ; ours?
                LDX     CURT
                LDW     *T.KDS
                STW     KEBDY
                LDW     *T.KAR
                LDX     KEBDY
                UNM
                JSX     *0              ; the body
                MSK
                STW     KERES
                LDW     =X'FFFF'
                STW     IOLOCK
                LDX     CURT
                LDW     *T.KLK
                STW     KELNK
                LDW     KERES
                LDX     KELNK
                UNM
                JSX     *0              ; and home
.WAIT           LDX     CURT            ; taken: sleep a tick and try again
                LDW     =1
                STW     *T.DLY
                LDW     =SSLP
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
                LDW     =KDRB
                JMP     KENTER
KDWRIT          MSK
                STX     KELNK
                STW     KEARG
                LDW     =KDWB
                JMP     KENTER

; The file calls: each to its body in rexfs.asm.
KLOOK           MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBLOOK
                JMP     KENTER
KOPEN           MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBOPEN
                JMP     KENTER
KGETC           MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBGETC
                JMP     KENTER
KPUTC           MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBPUTC
                JMP     KENTER
KCLOSE          MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBCLOS
                JMP     KENTER
KDELET          MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBDELE
                JMP     KENTER
KRENAM          MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBRENA
                JMP     KENTER

KSQUSH          MSK
                STX     KELNK
                STW     KEARG
                LDW     =FBSQSH
                JMP     KENTER

KDRB            SUBR
                STW     DXARG
                CLR
                STW     DXOP
                JSX     KDXP
                EXIT    KDRB
KDWB            SUBR
                STW     DXARG
                LDW     =1
                STW     DXOP
                JSX     KDXP
                EXIT    KDWB
KDXP            SUBR
                LDX     DXARG           ; the caller's three words
                LDW     *0
                STW     DXSEC
                LDW     *1
                AND     =K3FFF
                STW     DXCNT
                LDW     *2
                STW     DXBUF
                JSX     DXFER
                EXIT    KDXP

; One sector -- ACR is its index -- into the 47 words DXBUF names.
DRD1            SUBR
                STW     DXSEC
                LDW     =K47
                STW     DXCNT
                CLR
                STW     DXOP
                JSX     DXFER
                EXIT    DRD1

; ...and one sector the other way.
DWR1            SUBR
                STW     DXSEC
                LDW     =K47
                STW     DXCNT
                LDW     =1
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
                JMI     .NRD            ; not ready?  say so
                LDW     CURT
                STW     DWTR
                LDW     =1
                STW     DBUSY           ; before the DOT: the completion can be next
                LDW     DXBUF
                DOT     1,1             ; the core address (5-9.5.2)
                LDW     DXSEC
                AND     =X'7F'
                STW     DXTS
                LDW     DXSEC
                SRL     7
                SLL     10
                ORI     DXTS
                DOT     1,2             ; track in bits 0-5, sector in 7-15 (5-9.5.3)
                LDW     DXOP
                JNZ     .WR
                LDW     DXCNT
                DOT     1,6             ; unit 0, the count, read (5-9.5.4)
                JMP     .WT
.WR             LDW     DXCNT
                DOT     1,4             ; ...write
.WT             LDW     DBUSY
                JNZ     .BLK            ; over?
                JMP     .DN
.BLK            LDX     CURT
                LDW     =SWAI
                STW     *T.STA
                JSX     SWTCH           ; until DSERV says so
                MSK
                JMP     .WT
.DN             LDW     DSTAT
                STW     DXRES
                LDW     =X'FFFF'
                STW     DWTR
                UNM
                LDW     DXRES
                EXIT    DXFER
.NRD            UNM
                EXIT    DXFER

; K.BRKQ and K.BRKC: the running task's terminal's break flag, read and
; spent.  Any task may call them, so masked from the first instruction,
; and the link loaded back before the mask comes off.
BRKQ            MSK
                STX     BKRET
                LDX     CURT
                LDW     *T.TTY
                CAX
                LDW     *TM.BRK
                LDX     BKRET
                UNM
                JSX     *0
BRKC            MSK
                STX     BKRET
                LDX     CURT
                LDW     *T.TTY
                CAX
                CLR
                STW     *TM.BRK
                LDX     BKRET
                UNM
                JSX     *0

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
; A terminal's queue is therefore read only by whoever has that console,
; its shell or the task RUN gave it to, and any other caller gets minus
; one back instead of a place in the queue.  KCONSQ, the one queue a
; module knows of, stands for the caller's own terminal's.
; Tasks reading different queues may be in here together, so the link and
; the queue are kept in the caller's node across the wait, not in a slot
; of this routine's: masked from its first instruction, and the static
; cells good only until the processor is handed on.
Q.GET           MSK
                STX     QGLNK
                STW     QGD
                CMW     KCONSQ
                JNE     .IN             ; the console's queue?
                LDX     CURT            ; then the caller's own terminal's:
                LDW     *T.TTY
                CAX
                LDW     *TM.Q
                STW     QGD
                LDW     *TM.SH
                CMW     CURT
                JEQ     .IN             ; its shell reads it by right,
                LDX     CURT
                LDW     *T.CON
                JNZ     .IN             ; and so does the task RUN gave it to;
                LDW     =X'FFFF'        ; anybody else is told no, at once
                LDX     QGLNK
                UNM
                JSX     *0
.IN             LDX     CURT
                LDW     QGD
                STW     *T.KAR
                LDW     QGLNK
                STW     *T.KLK
.L              LDX     CURT
                LDW     *T.KAR
                STW     QGD
                CAX
                LDW     *Q.CNT
                JNZ     .T              ; anything in it?
                LDW     CURT            ; no: wait on it
                STW     *Q.WTR
                CAX
                LDW     =SWAI
                STW     *T.STA
                JSX     SWTCH           ; gone until Q.PUT wakes this task
                MSK
                JMP     .L              ; awake: look again
.T              LDW     *Q.BUF
                ADD     *Q.HEAD
                CAX
                LDW     *0
                STW     QGI
                LDX     QGD
                LDW     *Q.HEAD
                ADD     =1
                CMW     *Q.CAP
                SLS
                CLR                     ; round the ring
                STW     *Q.HEAD
                LDW     *Q.CNT
                SUB     =1
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
                LDW     =2047
                JMP     ALCOM
ALLOCB          MSK
                STW     ALSIZ
                LDW     =1023
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
                LDW     =2047
                JMP     ALCOMH
ALLCHB          MSK
                STW     ALSIZ
                LDW     =1023
                JMP     ALCOMH
ALLOCH          MSK
                STW     ALSIZ
                CLR
ALCOMH          STW     ALMSK
                STX     ALRET
                LDW     =1
                STW     ALHIQ           ; above the fence, and below it if
                                        ; there is nothing there
ALBEG           LDW     ALSIZ           ; an even number of words, always:
                ADD     =1              ; every block then starts on an even
                AND     =X'FFFE'        ; word, which is what lets one that
                STW     ALSIZ           ; was swapped out be cut out of the
                LDW     ALMSK           ; free list again exactly where it
                JNZ     .W              ; was (ALLOCAT).  And one that must
                                        ; lie in a window cannot be bigger
                JMP     .GO             ; than the window
.W              ADD     =1
                CMW     ALSIZ
                JGE     .GO
                JMP     ALFAIL
.GO             LDW     ALHIQ
                STW     ALHI
                JNZ     ALBH
ALLOW           CLR
                STW     ALFLO
                LDW     =FENCE
                STW     ALCEI
                JMP     ALGO
ALBH            LDW     =FENCE
                STW     ALFLO
                LDW     =POOLE
                STW     ALCEI
ALGO            CLR
                STW     ALPRV           ; the free block before F; zero: the head
                LDW     FREHD
.SCN            STW     ALF             ; F, a free block's header
                JNZ     .TRY0           ; the end of the list: nothing fits
                JMP     .NONE
.TRY0           CMW     ALCEI
                JGE     .NONE           ; at the ceiling: the list is in
                                        ; address order, so nothing fits
                CMW     ALFLO
                JGE     .TRY            ; below the floor: not this one
                JMP     .NXT
.TRY            CAX
                LDW     *0
                STW     ALS             ; S, its payload
                LDW     ALF
                ADD     =2
                STW     ALP             ; P, the candidate payload: F+2
                LDW     ALMSK
                JNZ     .CON            ; any containment asked for?
                JMP     .FIT

; Containment.  E is the first word of the window after P's; a payload
; that would cross it moves up to E -- or to E+1 when E is F+3, since the
; header must sit at F+2 or above to leave the leading fragment room for
; its own header (a fragment may hold zero words, but not minus one).
; Having moved, it must still lie inside the window it moved into.
.CON            LDW     ALP
                ORI     ALMSK
                ADD     =1
                STW     ALE
                LDW     ALP
                ADD     ALSIZ
                CMW     ALE
                JLE     .FIT            ; P+size > E: it would straddle
                                        ; no: P stands
                LDW     ALE
                SUB     ALF
                CMW     =3
                JEQ     .BP1            ; E == F+3?
                LDW     ALE
                STW     ALP
                JMP     .CK2
.BP1            LDW     ALE
                ADD     =1
                STW     ALP
.CK2            LDW     ALE             ; W, the window after E's
                ADD     ALMSK
                ADD     =1
                STW     ALW
                LDW     ALP
                ADD     ALSIZ
                CMW     ALW
                JLE     .FIT            ; still over the edge?
                JMP     .NXT

; Does [P, P+size) lie inside the block?  FE is the block's end, F+2+S.
; The test is on the room from P to FE against the size, not on where
; the block would end: with a size of several thousand words and P in
; the upper half of core, P+size passes X'7FFF' and is minus, and this
; machine's compares are signed.
.FIT            LDW     ALF
                ADD     =2
                ADD     ALS
                STW     ALFE
                SUB     ALP
                CMW     ALSIZ
                JGE     .FITS           ; less room than is wanted?
                JMP     .NXT
.FITS           LDW     ALP
                ADD     ALSIZ
                STW     ALEND
                JMP     .TAKE
.NXT            LDW     ALF             ; on to the next free block
                STW     ALPRV
                CAX
                LDW     *1
                JMP     .SCN

; Take it.  H is the new header.  A leading fragment keeps F on the list
; with a shorter size and becomes the predecessor of whatever follows.  A
; tail of three words or more becomes a free block of its own at the end
; of the allocation; a shorter one is simply given to the allocation, since
; two words of header with nothing behind them serve nobody.
.TAKE           LDW     ALP
                SUB     =2
                STW     ALH
                CMW     ALF
                JNE     .LEAD           ; a leading fragment?
                JMP     .TAIL
.LEAD           LDW     ALH
                SUB     ALF
                SUB     =2
                LDX     ALF
                STW     *0              ; [F] = H-F-2, its next as it was
                LDW     ALF
                STW     ALPRV
.TAIL           LDW     ALFE
                SUB     ALEND
                CMW     =3
                JGE     .SPLT           ; a tail worth keeping?
                LDW     ALFE            ; no: the allocation runs to the end
                SUB     ALH
                SUB     =2
                STW     ALSIZ
                LDX     ALF
                LDW     *1
                STW     ALNX            ; and the list goes on past F
                JMP     .LNK
.SPLT           LDX     ALEND           ; yes: [END] = FE-END-2, next = F's
                LDW     ALFE
                SUB     ALEND
                SUB     =2
                STW     *0
                LDX     ALF
                LDW     *1
                LDX     ALEND
                STW     *1
                LDW     ALEND
                STW     ALNX
.LNK            LDW     ALPRV           ; PRV's successor, or the head, is NX
                JNZ     .LNK1
                LDW     ALNX
                STW     FREHD
                JMP     .HDR
.LNK1           CAX
                LDW     ALNX
                STW     *1
.HDR            LDX     ALH             ; the header: size, and the owner
                LDW     ALSIZ
                STW     *0
                LDW     CURT
                ORI     =X'8000'
                STW     *1
                LDX     CURT            ; no longer waiting for core, if it was
                CLR
                STW     *T.MW
                LDW     ALP
                JMP     ALOUT
.NONE           LDW     ALHI
                JNZ     ALAGN           ; a second try to make, below the fence?
                LDW     SWSECS
                JNZ     ALWAIT          ; nothing fits.  Is there a swap file?
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
                CMW     =2
                JEQ     ALFAIL
                LDW     ALRET
                STW     *T.KLK
                LDW     ALSIZ
                STW     *T.KAR
                LDW     ALHIQ
                SAZ
                LDW     =X'8000'
                ORI     ALMSK
                STW     *T.KDS
                LDW     =1
                STW     *T.MW
                LDX     =SWTCB
                LDW     *T.STA
                CMW     =SWAI
                JNE     .W1             ; the swapper, out of its wait
                CLR
                STW     *T.STA
.W1             LDX     CURT
                LDW     =SWAI
                STW     *T.STA
                JSX     SWTCH
                MSK
                LDX     CURT
                LDW     *T.KLK
                STW     ALRET
                LDW     *T.KAR
                STW     ALSIZ
                LDW     *T.KDS
                AND     =X'7FFF'
                STW     ALMSK
                LDW     *T.KDS
                AND     =X'8000'
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
.TSC            STW     ALF
                JNZ     .T1
                JMP     .TNO
.T1             CMW     ATH
                JLE     .T2             ; a free block that starts past H:
                                        ; the list is in address order, so
                JMP     .TNO            ; H is in none
.T2             CAX
                LDW     *0
                ADD     ALF
                ADD     =2
                STW     ALFE
                CMW     ATH
                JLE     .TNX            ; ends at or before H: the next one
                LDW     ALFE
                CMW     ATE
                JGE     .TFIT           ; ends before H+n: not all free
                JMP     .TNO
.TNX            LDW     ALF
                STW     ALPRV
                CAX
                LDW     *1
                JMP     .TSC
.TFIT           LDX     ALF
                LDW     *1
                STW     ALNX
                LDW     ALFE            ; what is left above it stays free
                SUB     ATE
                JNZ     .TTL
                JMP     .TLD
.TTL            SUB     =2
                LDX     ATE
                STW     *0
                LDW     ALNX
                STW     *1
                LDW     ATE
                STW     ALNX
.TLD            LDW     ATH             ; and so does what is left below
                SUB     ALF
                JNZ     .TLF
                LDW     ALPRV           ; nothing below: F leaves the list
                JNZ     .TU1
                LDW     ALNX
                STW     FREHD
                JMP     .THDR
.TU1            CAX
                LDW     ALNX
                STW     *1
                JMP     .THDR
.TLF            SUB     =2
                LDX     ALF
                STW     *0
                LDW     ALNX
                STW     *1
.THDR           LDX     ATH
                LDW     ATN
                SUB     =2
                STW     *0
                LDW     ATTAG
                STW     *1
                LDW     =1
                EXIT    ALLOCAT
.TNO            CLR
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
                SUB     =2
                STW     FRH             ; H, the block's header
                CLR
                STW     FRPRV           ; the free block below it; zero: none
                LDW     FREHD
.SCN            STW     FRNX            ; NX, the first free block above it, or zero
                JNZ     .SC1
                JMP     .LNK
.SC1            CMW     FRH
                JLE     .SC2            ; NX > H: found the place
                JMP     .LNK
.SC2            STW     FRPRV
                CAX
                LDW     *1
                JMP     .SCN
.LNK            LDX     FRH             ; PRV -> H -> NX
                LDW     FRNX
                STW     *1
                LDW     FRPRV
                JNZ     .LK1
                LDW     FRH
                STW     FREHD
                JMP     .JN
.LK1            CAX
                LDW     FRH
                STW     *1
.JN             LDW     FRNX            ; does H run up to NX?
                JNZ     .JN1
                JMP     .JP
.JN1            LDX     FRH
                LDW     *0
                ADD     FRH
                ADD     =2
                CMW     FRNX
                JNE     .JP
                LDX     FRNX            ; [H] += [NX]+2, and NX's next is H's
                LDW     *0
                ADD     =2
                LDX     FRH
                ADD     *0
                STW     *0
                LDX     FRNX
                LDW     *1
                LDX     FRH
                STW     *1
.JP             LDW     FRPRV           ; does PRV run up to H?
                JNZ     .JP1
                JMP     .DN
.JP1            CAX
                LDW     *0
                ADD     FRPRV
                ADD     =2
                CMW     FRH
                JNE     .DN
                LDX     FRH             ; [PRV] += [H]+2, and H's next is PRV's
                LDW     *0
                ADD     =2
                LDX     FRPRV
                ADD     *0
                STW     *0
                LDX     FRH
                LDW     *1
                LDX     FRPRV
                STW     *1
.DN             EXIT    FREEI

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
KPOOLN          EQU     FENCE-POOLB-2   ; the pool's two parts, below the fence
KPOOHN          EQU     POOLE-POOLH-2   ; and above it, as free blocks' payloads

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
                AND     =KPGMSK
                SLL     1
                ORI     =KGLB
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
                ADD     =1
                STW     KNRING
                UNM
                EXIT    SPAWN

; TEXIT: the running task is over, and takes everything with it.  Its
; last character is waited out, since a terminal's owner must never name a
; node that is gone; the console is handed back if it held it -- the
; terminal's TM.BSY down, and its shell woken out of its wait, only out of
; that; REAP takes the rest;
; and the processor goes to whoever PICK finds, through the second half
; of SWTCH with nothing parked.  Masked throughout: the one unmasked
; instruction on the way out is SWTCH's INR 3, inside the range.
TEXIT           MSK
                LDX     CURT
                LDW     *T.MBX
                JNZ     .WT             ; a character still printing?
                JMP     .GO
.WT             LDW     =SWAI           ; wait for it, and start over
                STW     *T.STA
                JSX     SWTCH
                JMP     TEXIT
.GO             LDW     CURT
                STW     XDEAD
                LDW     *T.CON
                JNZ     .CON            ; the console's?
                JMP     .RING
.CON            LDW     *T.TTY          ; its terminal's, which comes back
                CAX
                CLR
                STW     *TM.BSY
                LDW     *TM.SH
                JNZ     .CON1           ; to the terminal's shell, if it has
                JMP     .RING           ; one
.CON1           CAX
                LDW     *T.STA
                CMW     =SWAI
                JNE     .RING           ; waiting on the hand-back?
                CLR
                STW     *T.STA
.RING           JSX     REAP
                JMP     SWRES           ; on to whoever can run

; The task XDEAD names is over, and everything it had goes: the node off
; the ring and counted off, its terminal's TM.LST -- where KICK's scan
; starts -- and CURT moved to its predecessor if they named it, CURT
; before anything is freed since PICK starts from CURT's link; the
; terminal told it has no shell, if this was its shell; its terminal's
; queue, if that had it down as its waiter, told otherwise; its channels let go; every block in the
; pool tagged with it given back; and the node returned to the arena,
; marked free.  Masked throughout, for the running task (TEXIT) and for
; one the shell is ending (KILLT) alike.
REAP            SUBR
                LDW     XDEAD           ; the predecessor: whose link names it
.PL             STW     XPRED
                CAX
                LDW     *T.NXT
                CMW     XDEAD
                JNE     .PL
                LDX     XDEAD           ; unlink
                LDW     *T.NXT
                LDX     XPRED
                STW     *T.NXT
                LDW     KNRING
                SUB     =1
                STW     KNRING
                LDX     XDEAD           ; its terminal's
                LDW     *T.TTY
                STW     XTM
                CAX
                LDW     *TM.LST
                CMW     XDEAD
                JNE     .SH
                LDW     XPRED
                STW     *TM.LST
.SH             LDW     *TM.SH
                CMW     XDEAD
                JNE     .CUR            ; the terminal's shell?
                CLR
                STW     *TM.SH
.CUR            LDW     CURT
                CMW     XDEAD
                JNE     .Q
                LDW     XPRED
                STW     CURT
.Q              LDX     XTM
                LDW     *TM.Q
                CAX
                LDW     *Q.WTR
                CMW     XDEAD
                JNE     .CH
                LDW     =X'FFFF'
                STW     *Q.WTR
.CH             LDW     XDEAD           ; its channels
                CALL    FCREL
                LDW     XDEAD           ; and its place in the swap file, if
                CALL    SWXFRE          ; its blocks are there and not here
                LDW     =POOLB          ; the sweep, block by block
.SWP            STW     XB
                CMW     =POOLE
                JGE     .NOD            ; the end of the pool?
                CAX
                LDW     *1
                JPL     .SNX            ; allocated?
                AND     =X'7FFF'
                CMW     XDEAD
                JNE     .SNX            ; its own?
                LDW     XB
                ADD     =2
                JSX     FREEI
.SNX            LDX     XB
                LDW     *0
                ADD     XB
                ADD     =2
                JMP     .SWP
.NOD            LDX     XDEAD           ; the node last, to the arena
                LDW     =SFREE
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
                JNZ     .CON            ; the console's?  it comes back
                JMP     .REAP
.CON            LDW     *T.TTY
                CAX
                CLR
                STW     *TM.BSY
.REAP           JSX     REAP
                EXIT    KILLT

; A node from the arena, or zero when every one is in use; and a node
; given back.  Callers hold the mask.  A node keeps its job number for
; good, and comes out with nothing else of its last task left in it.
NALLOC          SUBR
                LDW     NFREH
                JNZ     .L1
                EXIT    NALLOC
.L1             CAX
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
XTM             WORD    0               ; and its terminal
                LPOOL                   ; what this page's code writes as literals
