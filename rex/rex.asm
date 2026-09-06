; vim: ts=8:sw=8:expandtab:
;
; REX -- Raytheon EXec: a round-robin executive for the 703, preemptive
; and cooperative at once.
;
; Four tasks share the processor at power-on, their control blocks a ring
; of linked nodes the scheduler walks, and whatever the shell loads joins
; them.  Three print a letter through an interrupt-driven teletype driver
; and then sleep for a fixed number of ticks, so the scheduling is
; visible in the output: the letters arrive at their own intervals, and
; when every task is asleep the idle task -- a fifth node, off the ring
; -- has the processor.  One is a shell, which waits on a queue of
; keystrokes and runs a command, and can load a module off the disc as a
; task of its own: Tiny BASIC, the interpreter of test/703/bcore.asm
; behind the glue of brex.asm, is one, and RUN BASIC hands it the console
; until its BYE.
;
; The letter tasks come up stopped; START sets them going.  The commands:
;
;   HELP or ?    the command list
;   STAT         every task's state, and how long each sleeper has left
;   UPTIME, UP   seconds since the executive came up
;   STOP  [A-C]  suspend a letter task, or all three
;   START [A-C]  release one, or all three
;   ECHO text    print the rest of the line
;   MEM          the words free in the pool
;   LOAD name    a module off the disc, run behind the prompt
;   RUN  name    the same, given the console until it exits
;   BASIC        RUN BASIC
;   HALT         park the tasks, drain the printer and stop the machine
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
;   K.EXIT clears it and wakes the shell under SERV's guard.  Ctrl-C
;   never enters the queue at all: SERV raises BRKREQ instead, so a
;   running program that reads no input can still be broken, and the
;   grant clears the flag so a break aimed at nobody cannot land on the
;   session that follows it.
;
; * SHUTDOWN ORDERING.  Every letter task reads SHUTREQ inside the same
;   masked window as its deposit, and the shell -- the only writer of
;   SHUTREQ -- cannot run inside that window, so a task that saw zero has
;   its character banked before the shutdown exists and a task that saw
;   the flag parks without depositing.  Once the shell's drain finds the
;   letter tasks' mailboxes empty and the printer idle, nothing of theirs
;   can chase the down-message.
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
;   MSK -- the doorway a loader would use.  What a task IS is data in its
;   node: STOP, START, STAT and HALT act on whatever nodes the walk
;   finds, and a letter task is any node with a letter in T.CHR.  SPAWN
;   is that doorway, and the shell's loader goes through it.
;
; * A FRESH TASK'S STATUS is GLB plus its entry page.  A zero status word
;   would resume the task in local mode with EXR 0 and its first memory
;   reference would land in page 0.  For an entry on a 1024-word byte
;   page boundary the EXR field is exactly (entry * 2); LTASK and IDLE
;   are in page 0, where the field is plain zero.
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
;   freed, every block in the pool tagged with the node given back, and
;   the processor handed on through the second half of SWTCH with nothing
;   parked.  The loader tags a module's block and its node with the node
;   as it makes them, which is what makes the sweep complete.
;
; * EVERYTHING RUNS GLOBAL.  START sets it, the TCB statuses carry it,
;   and entry and JSX force it -- EXIT's indexed JSX and every indexed
;   reference here assume a flat address.
;
; Memory map, everything below X'4000':
;
;   0000-000F  interrupt blocks: level 0 (teletype), level 1 (disc),
;              level 2 (line clock), level 3 (never signalled -- SWTCH
;              stages a switch in it and loads it with INR 3)
;   0010-002F  the kernel interface for what loads under it: the jump
;              vector, then the exported cells (rexapi.asm)
;   0040-      page 0: START; then ISRBEG..ISREND, which is SCHED, SERV,
;              KICK, PICK, SWTCH, Q.PUT and DSERV; then the kernel cells,
;              the console queue, the TCB nodes, Q.GET, DREAD, the idle
;              task, LTASK -- the one letter-task body all three letter
;              nodes run -- and the allocator
;   0800-      page 1: the shell -- banner, prompt, commands, line
;              buffer -- and its loader, with the sector it reads
;   1000-3FFF  the pool, which the allocator hands out: what the shell
;              loads, its nodes and its workspaces live there
;
; Build with make -C rex: asm703.py over this file into rex/build, and the
; modules beside it -- brex.asm over test/703/bcore.asm is basic.obj, the
; module RUN BASIC loads.  make -C rex disc puts them on the platter, and
; then, from the repo root:
;
;   ./target/debug/emu -s ray703 -r rex/build/rex.bin --fast-io

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
K.DREAD         JMP     DREAD           ; ACR = sector, DRBUF = buffer -> status
                WORD    0,0,0,0,0,0,0   ; X'19'-X'1F'

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
DRBUF           WORD    0               ; K.DREAD's buffer, a word address

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
T.CHR           EQU     8               ; a letter task's letter, and what
                                        ; marks it as one: zero for the
                                        ; tasks STOP/START may not touch
T.NAP           EQU     9               ; a letter task's sleep, in ticks
T.NAM           EQU     10              ; two characters, for STAT
T.CON           EQU     11              ; nonzero: this task holds the console
T.LEN           EQU     12              ; words in a node

SRUN           EQU     0
SSLP           EQU     1
SOFF           EQU     2               ; suspended by the shell's STOP
SWAI           EQU     3               ; blocked on a queue

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

NRING           EQU     4               ; nodes on the ring at power-on: A,
                                        ; B, C and the shell.  Idle is off
                                        ; it, the scans' explicit fallback.

POOLB           EQU     X'1000'         ; the pool: core the allocator hands
POOLE           EQU     X'4000'         ; out, from the end of the image

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
                LDW     KPOOLB          ; the pool: one free block, the lot
                STW     FREHD
                CAX
                LDW     KPOOLN
                STW     *0
                CLR
                STW     *1
                DOT     14,11           ; connect the keyboard; function 11
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
SRXBRK          LDW     K1
                STW     BRKREQ

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
; served last, or leave it idle if all three are empty.  OWNER is claimed
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
                JMP     PKPIK
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
LASTS           WORD    ATCB            ; last node served, for fairness
KCAND           WORD    0               ; KICK's scan scratch
KTRY            WORD    0
SHUTREQ         WORD    0               ; set by the shell's HALT, read by tasks
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
DRSEC           WORD    0               ; DREAD's sector index...
DRTS            WORD    0               ; ...as track and sector...
DRRES           WORD    0               ; ...and its answer
K1              WORD    1
K47             WORD    47              ; words in a sector, unit 0 in the top bits
K7F             WORD    X'7F'
K60             WORD    60
KM1             WORD    X'FFFF'
KWAIT           WORD    SWAI

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
KRING           WORD    ATCB            ; where a walk of the ring starts
KIDLE           WORD    IDTCB           ; and where a scan with nothing
                                        ; runnable falls back to
KPGMSK          WORD    X'7C00'         ; the page bits of a word address, which
KGLB            WORD    X'0080'         ; doubled are a status word's EXR field
KISRB           WORD    ISRBEG
KISRE           WORD    ISREND

; The task control nodes: A -> B -> C -> SH and round again, with idle
; off the ring and its link re-entering it, which is what lets every walk
; start uniformly at *T.NXT.  The shell's context is blank because the
; kernel becomes the shell and the first tick fills it in.  A status is
; GLB (X'80') plus the entry page in the EXR field, and LTASK and IDLE
; live in page 0, so theirs is plain zero; SPAWN builds one for whatever
; the loader brings in.  A zero status word would resume a task in local
; mode pointed at page 0.
;
;                    STA    NXT   ACR IXR PCR    MST   DLY MBX CHR NAP NAM  CON
ATCB            WORD SOFF, BTCB, 0,  0,  LTASK, X'80', 0,  0,  'A',30, 'A ',0
BTCB            WORD SOFF, CTCB, 0,  0,  LTASK, X'80', 0,  0,  'B',45, 'B ',0
CTCB            WORD SOFF, SHTCB,0,  0,  LTASK, X'80', 0,  0,  'C',60, 'C ',0
SHTCB           WORD SRUN, ATCB, 0,  0,  0,     0,     0,  0,  0,  0,  'SH',0
IDTCB           WORD SRUN, ATCB, 0,  0,  IDLE,  X'80', 0,  0,  0,  0,  'ID',0

; Read one sector -- ACR is its index, track*128+sector -- into the 47
; words at DRBUF, and return the controller's status, zero for a clean
; transfer.  A task's call: it waits on the transfer the way a task waits
; on anything here, masked, look, and if not yet, mark itself waiting and
; hand the processor on, and DSERV wakes it.  DWTR names the node whose
; transfer is in flight, so a second reader sleeps a tick and looks again.
; A unit that is not there reads not-ready, and that comes back at once
; rather than waiting for a completion that would never come.
DREAD           SUBR
                STW     DRSEC
DRCLM           MSK
                LDW     DWTR
                SAM                     ; the disc is free?
                JMP     DRSLP
                JMP     DRGO
DRSLP           LDX     CURT            ; no: sleep a tick and look again
                LDW     K1
                STW     *T.DLY
                LDW     KSLP
                STW     *T.STA
                JSX     SWTCH
                JMP     DRCLM
DRGO            DIN     1,0
                SAP                     ; not ready?  say so
                JMP     DRNRD
                LDW     CURT
                STW     DWTR
                LDW     K1
                STW     DBUSY           ; before the DOT: the completion can be next
                LDW     DRBUF
                DOT     1,1             ; the core address (5-9.5.2)
                LDW     DRSEC
                AND     K7F
                STW     DRTS
                LDW     DRSEC
                SRL     7
                SLL     10
                ORI     DRTS
                DOT     1,2             ; track in bits 0-5, sector in 7-15 (5-9.5.3)
                LDW     K47
                DOT     1,6             ; unit 0, one sector, read (5-9.5.4)
DRWT            LDW     DBUSY
                SAZ                     ; over?
                JMP     DRBLK
                JMP     DRDN
DRBLK           LDX     CURT
                LDW     KWAIT
                STW     *T.STA
                JSX     SWTCH           ; until DSERV says so
                MSK
                JMP     DRWT
DRDN            LDW     DSTAT
                STW     DRRES
                LDW     KM1
                STW     DWTR
                UNM
                LDW     DRRES
                EXIT    DREAD
DRNRD           UNM
                EXIT    DREAD

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
Q.GET           SUBR
                STW     QGD
QGL             MSK
                LDX     QGD
                LDW     *Q.CNT
                SAZ                     ; anything in it?
                JMP     QGT
                JMP     QGW
QGW             LDW     CURT
                LDX     QGD
                STW     *Q.WTR
                LDX     CURT
                LDW     KWAIT
                STW     *T.STA
                JSX     SWTCH           ; gone until Q.PUT wakes this task
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
                UNM
                LDW     QGI
                EXIT    Q.GET

IDLE            JMP     IDLE

; ---------------------------------------------------------------- the letters
; The one letter task, run by three nodes at once: print my letter and
; sleep my nap, forever.  Which task this is comes from CURT -- the only
; identity that survives a switch -- and every read of it sits inside a
; masked window, where CURT can only name self.  A node is born stopped,
; and runs when the shell's START says so.  The first masked window is
; the shutdown protocol: SHUTREQ read and the character deposited with
; SERV locked out, so a deposit can never follow an observed shutdown.
; Everything this task touches is in page 0, so unlike its callers in
; other pages it needs no SMB anywhere.
LTASK           MSK
                LDW     SHUTREQ
                SAZ
                JMP     LQUIT
                LDX     CURT            ; my node
                LDW     *T.CHR          ; my letter...
                STW     *T.MBX          ; ...deposited in my mailbox
                JSX     KICK
                UNM
LWAIT           MSK
                LDX     CURT
                LDW     *T.MBX
                SAZ                     ; printed yet?
                JMP     LWBLK
                JMP     LWDON
LWBLK           LDW     KWAIT           ; no: stand down until SERV says so,
                STW     *T.STA          ; and look again when it does -- a
                JSX     SWTCH           ; wake is advice, not a promise
                JMP     LWAIT
LWDON           UNM

; Sleep my nap: store the delay and the state in one masked window, so
; the tick cannot read half of it, and hand the processor straight on.
; SWTCH returns when the scheduler next picks this task, which the scan
; will not do until the tick counts the delay down to nothing.
                MSK
                LDX     CURT
                LDW     *T.NAP
                STW     *T.DLY
                LDW     KSLP
                STW     *T.STA
                JSX     SWTCH           ; and the processor goes elsewhere
                JMP     LTASK           ; now, not at the next tick
LQUIT           UNM
LPARK           JMP     LPARK           ; parked; a legal idle, levels live

; ---------------------------------------------------------------- the pool
; First fit over a list of free blocks sorted by address, with the block
; layout the header describes: two words in front of every payload, its
; size and either the next free block (zero ends the list) or the owning
; node with bit 0 set.  Three entries share one body: K.ALLOC takes any
; address, K.ALLCW wants the payload inside one 2048-word page and
; K.ALLCB inside one 1024-word byte page -- what a module needs, since
; its M fields are page offsets.  The answer is the payload address, or
; zero, which is never in the pool.  The whole search is masked, and the
; link is kept by hand because the stubs fall into the body with IXR
; still holding it.
ALLOCW          STW     ALSIZ
                LDW     K2047
                JMP     ALCOM
ALLOCB          STW     ALSIZ
                LDW     K1023
                JMP     ALCOM
ALLOC           STW     ALSIZ
                CLR
ALCOM           STW     ALMSK           ; the window less one, or zero for any
                STX     ALRET
                MSK
                CLR
                STW     ALPRV           ; the free block before F; zero: the head
                LDW     FREHD
ALSCN           STW     ALF             ; F, a free block's header
                SAZ                     ; the end of the list: nothing fits
                JMP     ALTRY
                JMP     ALNONE
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
ALFIT           LDW     ALF
                ADD     K2
                ADD     ALS
                STW     ALFE
                LDW     ALP
                ADD     ALSIZ
                STW     ALEND
                CMW     ALFE
                SGR                     ; overruns the block?
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
                LDW     ALP
                JMP     ALOUT
ALNONE          CLR
ALOUT           UNM
                LDX     ALRET
                JSX     *0

; Give a block back.  FREEI does the work with the mask held by its
; caller, which is FREE for a task and the exit path for a task's whole
; estate; MSK and UNM do not nest, so a routine that is already masked
; comes here directly.  The block goes into the list where its address
; falls and is joined to a neighbour on either side that touches it, so
; the list never holds two adjacent free blocks.  A size field only ever
; grows here: the freed block's own may grow to swallow the one above it,
; and the one below it may grow to swallow this one, so a walk of the
; pool by size strides stays right through a block just freed.
FREE            SUBR
                MSK
                JSX     FREEI
                UNM
                EXIT    FREE

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
FRH             WORD    0               ; the block being freed
FRPRV           WORD    0               ; the free blocks on either side
FRNX            WORD    0
K2              WORD    2
K3              WORD    3
K1023           WORD    1023
K2047           WORD    2047
K8000           WORD    X'8000'
KPOOLB          WORD    POOLB           ; the pool: [POOLB, POOLE)
KPOOLE          WORD    POOLE
KPOOLN          WORD    POOLE-POOLB-2   ; as one free block's payload
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
                STW     *T.CHR
                STW     *T.NAP
                LDW     *T.PCR
                AND     KPGMSK
                SLL     1
                ORI     KGLB
                STW     *T.MST
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

; TEXIT: the running task is over, and takes everything with it.  In
; this order: its last character is waited out, since OWNER must never
; name a node that is gone; the console is handed back if it held it --
; CONBSY down, and the shell woken out of its wait, only out of that;
; the node is unlinked and counted off, and LASTS, where KICK's scan
; starts, moved off it; CURT is pointed at the predecessor BEFORE
; anything is freed, since PICK starts from CURT's link; then every block
; in the pool tagged with the node is given back, its own block among
; them, and the processor goes to whoever PICK finds, through the second
; half of SWTCH with nothing parked.  Masked throughout: the one unmasked
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
TXRING          LDW     XDEAD           ; the predecessor: whose link names us
TXPL            STW     XPRED
                CAX
                LDW     *T.NXT
                CMW     XDEAD
                SEQ
                JMP     TXPL
                LDX     XDEAD           ; unlink
                LDW     *T.NXT
                LDX     XPRED
                STW     *T.NXT
                LDW     KNRING
                SUB     K1
                STW     KNRING
                LDW     LASTS
                CMW     XDEAD
                SNE
                JMP     TXLS
                JMP     TXCUR
TXLS            LDW     XPRED
                STW     LASTS
TXCUR           LDW     XPRED
                STW     CURT
                LDW     KPOOLB          ; the sweep, block by block
TXSWP           STW     XB
                CMW     KPOOLE
                SLS                     ; the end of the pool?
                JMP     SWRES           ; then on to whoever can run
TXSW1           CAX
                LDW     *1
                SAM                     ; allocated?
                JMP     TXSNX
                AND     K7FFF
                CMW     XDEAD
                SEQ                     ; ours?
                JMP     TXSNX
                LDW     XB
                ADD     K2
                JSX     FREEI
TXSNX           LDX     XB
                LDW     *0
                ADD     XB
                ADD     K2
                JMP     TXSWP

SPNOD           WORD    0               ; SPAWN's node
XDEAD           WORD    0               ; TEXIT's: the node going, its
XPRED           WORD    0               ; predecessor on the ring, and the
XB              WORD    0               ; sweep's cursor

; ---------------------------------------------------------------- the shell
; Prints the banner and then reads a line and runs it, forever.  It is
; the only task running when the machine comes up.  Everything it prints goes
; through its own mailbox one character at a time like any other task's
; letter, so a command's output and the background letters interleave on
; the printer exactly as two users' output did.
                ORG     X'800'

; The letter tasks are born stopped, and stay that way until somebody
; asks for them: an executive with nothing running is a better place to
; start looking around than one already talking to itself.
SHELL           LDW     SHMBAN
                JSX     SHMSG
                LDW     SHMHNT
                JSX     SHMSG

SHLOOP          LDW     SHMPRM
                JSX     SHMSG
                JSX     SHGETL          ; a line, however long that takes
                LDW     SHKLBB
                STW     SHCUR
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
SHDNF           LDW     SHMWHT
                JSX     SHMSG
                JMP     SHLOOP

; ---------------------------------------------------------------- commands
SHHELP          LDW     SHMHL1
                JSX     SHMSG
                LDW     SHMHL2
                JSX     SHMSG
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

; The tasks: name, state, and how much longer a sleeper has.  A walk of
; the ring, then the idle node -- it is off the ring, so the walk ends by
; visiting it explicitly instead of following a link back to the start.
SHSTAT          JSX     SHPUP
                LDW     SHKRNG
                STW     SHTO            ; the node being printed
                SMB     KNRING          ; the ring as it stands, and idle
                LDW     KNRING
                ADD     SHK1
                STW     SHTI
SHSTL           LDX     SHTO
                LDW     *T.NAM          ; its two-character name
                JSX     SHPW2
                LDW     SHKSP
                JSX     SHPUTC
                LDX     SHTO            ; its state, as four characters
                LDW     *T.STA
                SLL     1
                ADD     SHKSTA
                STW     SHSA
                CAX
                LDW     *0
                JSX     SHPW2
                LDX     SHSA
                LDW     *1
                JSX     SHPW2
                LDX     SHTO
                LDW     *T.STA
                CMW     SHKSLP          ; asleep? then say for how long
                SEQ
                JMP     SHSTN
                LDX     SHTO
                LDW     *T.DLY
                JSX     SHDEC
SHSTN           JSX     SHNL
                LDW     SHTI
                SUB     SHK1
                STW     SHTI
                SAZ                     ; more of them to print?
                JMP     SHST2
                JMP     SHLOOP
SHST2           CMW     SHK1            ; only idle left?
                SEQ
                JMP     SHST3
                LDW     SHKIDL          ; then visit it, off the ring
                STW     SHTO
                JMP     SHSTL
SHST3           LDX     SHTO
                LDW     *T.NXT
                STW     SHTO
                JMP     SHSTL

; STOP and START differ only in the state they store.  With no argument
; they take every letter task; with one they take the task whose letter
; matches, and nothing else.  What marks a letter task is data, not
; position: a non-zero T.CHR.  The shell and BASIC have none, so neither
; can be named -- the shell must not be able to suspend itself, since it
; is the only way to start anything again, and a stopped console owner
; would leave the shell waiting on a grant nobody can return.
SHSTOP          LDW     SHKOFF
                STW     SHNST
                JMP     SHSSET
SHSTRT          CLR
                STW     SHNST
SHSSET          JSX     SHARG
                LDW     SHARGF
                SAZ                     ; no argument: all of them
                JMP     SHSONE
SHSALL          LDW     SHKRNG          ; once round the ring
                STW     SHTO
SHSAL1          LDX     SHTO
                LDW     *T.CHR
                SAZ                     ; a letter task?
                JMP     SHSAL2
                JMP     SHSAL3
SHSAL2          LDW     SHNST
                STW     *T.STA
SHSAL3          LDX     SHTO
                LDW     *T.NXT
                STW     SHTO
                CMW     SHKRNG          ; all the way round?
                SEQ
                JMP     SHSAL1
                JMP     SHLOOP
SHSONE          LDW     SHKRNG          ; find the named letter
                STW     SHTO
SHSO1           LDX     SHTO
                LDW     *T.CHR
                SAZ                     ; only a letter task can be named
                JMP     SHSO2
                JMP     SHSO3
SHSO2           CMW     SHARGV
                SNE
                JMP     SHSO4           ; this is the task
SHSO3           LDX     SHTO
                LDW     *T.NXT
                STW     SHTO
                CMW     SHKRNG
                SEQ
                JMP     SHSO1
                JMP     SHSOBD          ; all the way round: no such task
SHSO4           LDX     SHTO
                LDW     SHNST
                STW     *T.STA
                JMP     SHLOOP
SHSOBD          LDW     SHMNOT
                JSX     SHMSG
                JMP     SHLOOP

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
SHBWB           LDW     SHKWAI
                SMB     SH.STA
                STW     SH.STA
                SMB     SWTCH
                JSX     SWTCH
                JMP     SHBWT
SHBWD           UNM
                JMP     SHLOOP

; Shut the machine down.  The letter tasks park at their next masked
; window, so once their mailboxes -- found the way STOP finds the tasks,
; by T.CHR -- are empty and the printer is idle, nothing of theirs can
; appear inside the down-message.
; LOAD name and RUN name: a module off the disc, started as a task.  RUN
; hands it the console and waits for it to be over -- the break flag
; cleared and CONBSY raised in one masked window, as the grant to BASIC
; is made, then SHBWT's wait -- and LOAD leaves it to run behind the
; prompt.  The task's node is linked in by SPAWN under its own mask,
; after the grant: the task cannot run before it is on the ring, and
; nothing but a task that holds the console can lower CONBSY.
SHRUN           LDW     SHK1
                STW     LDCON
                JSX     SHTOK           ; the name
                JMP     SHLD1
SHLOAD          CLR
                STW     LDCON
                JSX     SHTOK
                JMP     SHLD1
SHBASI          LDW     SHKBA0          ; BASIC: RUN BASIC, the name put
                STW     STOK0           ; where SHTOK would have put it
                LDW     SHKBA1
                STW     STOK1
                LDW     SHK1
                STW     LDCON
SHLD1           JSX     LDMOD
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
                LDW     SHK1
                SMB     CONBSY
                STW     CONBSY
                UNM
                LDW     LDNODE
                SMB     SPAWN
                JSX     SPAWN
                JMP     SHBWT

; ---------------------------------------------------------------- the loader
; Load the module STOK0:STOK1 names: find it in the catalogue -- sector
; 1, entries of a four-character name packed as SHTOK packs one, a first
; sector and a sector count, a zero first sector ending the table -- and run its
; object text into a block from the pool, one record to a sector.  The
; text is the 1968 relocating loader's (tools/asm703.py's docstring lists
; the codes, tools/reload703.py is the reference reading of them): the
; SIZE code comes first and sizes the block, whose address is the
; relocation base; each repeatable code carries a run of words that are
; stored as they are, or with the base added into the 11-bit M field,
; the whole word, or twice for a byte address; a page selection is built
; here from the final address; END names the entry, which gets the base
; too.  Returns the node, with its entry, name and console flag filled
; in and both blocks tagged as the task's own, ready for SPAWN -- or
; zero, an error having been named: no such file, the disc's status (DE),
; a record failing its checksum (CK), a code this loader does not take or
; text out of order (LC), no room (MX).  An error path jumps straight out
; of whatever routine it was in, as RELOADB's does.
LDMOD           SUBR
                LDW     STOK1           ; the name, in STOK0:STOK1 -- a short
                SAZ                     ; one fills only the second word
                JMP     LDM1
                JMP     LDNOF
LDM1            CLR
                STW     LDBLK
                STW     LDNODE
                STW     LDBASE
                STW     LDPTR
                STW     LDLIM
                LDW     SHKLDB
                SMB     DRBUF
                STW     DRBUF
                LDW     SHK1            ; the catalogue
                SMB     DREAD
                JSX     DREAD
                SAZ
                JMP     LDEDE
                LDW     SHKLDB
                STW     LDCP
LDCL            LDX     LDCP
                LDW     *2
                SAZ                     ; the end of the table: no file
                JMP     LDC1            ; starts at sector 0, the boot sector
                JMP     LDNOF
LDC1            LDW     *0
                CMW     STOK0
                SEQ
                JMP     LDCN
                LDW     *1
                CMW     STOK1
                SEQ
                JMP     LDCN
                LDW     *2              ; found: where it starts...
                STW     LDSEC
                LDW     *3              ; ...and how many sectors
                STW     LDNSEC
                JMP     LDFND
LDCN            LDW     LDCP
                ADD     SHK4
                STW     LDCP
                JMP     LDCL
LDFND           LDW     SHKLDN          ; a node from the pool
                SMB     ALLOC
                JSX     ALLOC
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
                JMP     LDSIZ           ; entry the code names
LDSIZB          LDW     SHKALB
LDSIZ           STW     LDALE
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

; Loaded: fill the node, and tag both blocks as the new task's own, so
; that its exit finds them.
LDDONE          LDX     LDNODE
                LDW     LDEXEC
                STW     *T.PCR
                LDW     STOK0
                STW     *T.NAM
                LDW     LDCON
                STW     *T.CON
                LDW     LDNODE
                ORI     SHK8000
                STW     LDTAG
                LDW     LDNODE
                SUB     SHK1
                CAX
                LDW     LDTAG
                STW     *0
                LDW     LDBLK
                SUB     SHK1
                CAX
                LDW     LDTAG
                STW     *0
                LDW     LDNODE
                EXIT    LDMOD

LDNOF           LDW     SHMNOF
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
LDER3           SMB     FREE
                JSX     FREE
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
                LDW     LDSEC
                SMB     DREAD
                JSX     DREAD
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

; The words free in the pool: the free list's sizes added up, under the
; mask that every walk of the list holds.
SHMEM           MSK
                CLR
                STW     SHMTOT
                SMB     FREHD
                LDW     FREHD
SHMEML          SAZ                     ; the end of the list?
                JMP     SHMEM1
                JMP     SHMEMD
SHMEM1          CAX
                LDW     *0
                ADD     SHMTOT
                STW     SHMTOT
                LDW     *1
                JMP     SHMEML
SHMEMD          UNM
                LDW     SHMFRE
                JSX     SHMSG
                LDW     SHKSP
                JSX     SHPUTC
                LDW     SHMTOT
                JSX     SHDEC
                JSX     SHNL
                JMP     SHLOOP

SHHALT          LDW     SHK1
                SMB     SHUTREQ
                STW     SHUTREQ
SHHDR           CLR
                STW     SHW2            ; the letter mailboxes, ORed up
                LDW     SHKRNG
                STW     SHTO
SHHD0           LDX     SHTO
                LDW     *T.CHR
                SAZ                     ; a letter task?
                JMP     SHHD1
                JMP     SHHD2
SHHD1           LDW     *T.MBX
                ORI     SHW2
                STW     SHW2
SHHD2           LDX     SHTO
                LDW     *T.NXT
                STW     SHTO
                CMW     SHKRNG          ; all the way round?
                SEQ
                JMP     SHHD0
                LDW     SHW2
                SAZ                     ; every letter mailbox empty?
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

; An argument character: SHARGF is 0 for none, 1 with the character in
; SHARGV.  Whether it names a task is the ring's to say -- SHSONE walks
; the nodes comparing it against each T.CHR.
SHARG           SUBR
                CLR
                STW     SHARGF
                STW     SHARGV
                JSX     SHSKB
                LDX     SHCUR
                CLR
                LDB     *0
                SAZ                     ; end of line: no argument
                JMP     SHAG2
                EXIT    SHARG
SHAG2           STW     SHARGV
                LDW     SHK1
                STW     SHARGF
                EXIT    SHARG

; ---------------------------------------------------------------- output
; One character from ACR: deposit in the shell's mailbox, kick the
; printer, and stand down until SERV reports it printed.  The deposit
; window is masked for KICK's sake, and so is each look at the mailbox
; afterwards, so that the completion cannot land between the look and the
; decision to wait on it.  The letter tasks do the same thing; see task A.
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
SHGL            LDW     SHKCQ
                SMB     Q.GET
                JSX     Q.GET
                STW     SHCH
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
                EXIT    SHGETL

; ---------------------------------------------------------------- shell data
SH.STA          EQU     SHTCB+T.STA     ; this task's own state word...
SH.MBX          EQU     SHTCB+T.MBX     ; ...and its own mailbox

SHCUR           WORD    0               ; the cursor into the line, a byte
SHFIL           WORD    0               ; and where SHGETL is filling it
SHCH            WORD    0               ; the character it is filing
SHTP            WORD    0               ; the command table cursor
SHTI            WORD    0               ; STAT's countdown over the nodes...
SHTO            WORD    0               ; ...and the node a walk is visiting
SHSA            WORD    0               ; the state name being printed
SHNST           WORD    0               ; the state STOP or START will store
SHARGF          WORD    0               ; 0 no argument, 1 in SHARGV
SHARGV          WORD    0
STOK0           WORD    0               ; the command word, four characters
STOK1           WORD    0
STKN            WORD    0               ; how many of them are still wanted
SHW2            WORD    0               ; scratch
SHW2P           WORD    0               ; SHPW2's, which SHPUTC must not touch
SHMTOT          WORD    0               ; MEM's running total
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
LDCP            WORD    0               ; the catalogue cursor
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
SHK8            WORD    8
SHK0C           WORD    12              ; the control codes, 0..B
SHK0F           WORD    X'000F'
SHK80           WORD    X'0080'
SHK7FF          WORD    X'07FF'
SHKF800         WORD    X'F800'
SHK8000         WORD    X'8000'
SHK0FF          WORD    X'00FF'
SHKTEN          WORD    10
SHKZER          WORD    '0'
SHKSP           WORD    ' '
SHKCR           WORD    X'008D'
SHKLF           WORD    X'008A'
SHKSLP          WORD    SSLP
SHKOFF          WORD    SOFF
SHKWAI          WORD    SWAI
SHKUPM          WORD    X'FFDF'         ; folds a letter to upper case
SHKCQ           WORD    QCONS           ; the queue the keyboard fills
SHKRNG          WORD    ATCB            ; the ring, where the shell's walks
SHKIDL          WORD    IDTCB           ; start, and the idle node past it
SHKLBB          WORD    LBUF*2          ; the line, and the last byte its
SHKLBE          WORD    LBUF*2+62       ; zero terminator may need
SHKDBE          WORD    SHDB*2+5        ; the last byte of the digit buffer
SHKSTA          WORD    SHSTA
SHKTAB          WORD    SHTAB
SHKCTB          WORD    LDCTAB
SHKLDB          WORD    LDBUF           ; the sector buffer, as a word address...
SHKDBB          WORD    LDBUF*2         ; ...its first byte, the marker...
SHKDBL          WORD    LDBUF*2+93      ; ...and its last, the checksum
SHKLDN          WORD    T.LEN           ; words in a node
SHKALW          WORD    ALLOCW          ; the allocator's page entries, for
SHKALB          WORD    ALLOCB          ; an indexed JSX
SHKBA0          WORD    'BA'            ; BASIC's name, as SHTOK packs it
SHKBA1          WORD    'SI'

; Four characters a state, indexed by the state doubled.
SHSTA           WORD    'RU','N ','SL','P ','OF','F ','WA','IT'

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
                WORD    'BA','SI',SHBASI
                WORD    'M','EM',SHMEM
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
SHMEDE          WORD    SHEDE
SHMECK          WORD    SHECK
SHMELC          WORD    SHELC
SHMEMX          WORD    SHEMX
SHMUPM          WORD    SHUPM
SHMSEC          WORD    SHSEC
SHMDWN          WORD    SHDWN
SHMNOT          WORD    SHNOT

SHBAN           WORD    SHBANT*2,SHBANE*2
SHHNT           WORD    SHHNTT*2,SHHNTE*2
SHPRM           WORD    SHPRMT*2,SHPRME*2
SHWHT           WORD    SHWHTT*2,SHWHTE*2
SHHL1           WORD    SHHL1T*2,SHHL1E*2
SHHL2           WORD    SHHL2T*2,SHHL2E*2
SHFRE           WORD    SHFRET*2,SHFREE*2
SHNOF           WORD    SHNOFT*2,SHNOFE*2
SHEDE           WORD    SHEDET*2,SHEDEE*2
SHECK           WORD    SHECKT*2,SHECKE*2
SHELC           WORD    SHELCT*2,SHELCE*2
SHEMX           WORD    SHEMXT*2,SHEMXE*2
SHUPM           WORD    SHUPMT*2,SHUPME*2
SHSEC           WORD    SHSECT*2,SHSECE*2
SHDWN           WORD    SHDWNT*2,SHDWNE*2
SHNOT           WORD    SHNOTT*2,SHNOTE*2

SHBANT          TEXT    "REX 703 UP\r\n"
SHBANE          EQU     $
SHHNTT          TEXT    "TYPE HELP, OR START FOR THE LETTER TASKS\r\n"
SHHNTE          EQU     $
SHPRMT          TEXT    "REX>  "
SHPRME          EQU     $
SHWHTT          TEXT    "WHAT\r\n"
SHWHTE          EQU     $
SHHL1T          TEXT    "COMMANDS HELP STAT UPTIME STOP START ECHO MEM LOAD RUN BASIC HALT \r\n"
SHHL1E          EQU     $
SHHL2T          TEXT    "STOP AND START TAKE A B OR C\r\n"
SHHL2E          EQU     $
SHFRET          TEXT    "FREE"
SHFREE          EQU     $
SHNOFT          TEXT    "NO SUCH FILE\r\n"
SHNOFE          EQU     $
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
