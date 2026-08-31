; vim: ts=8:sw=8:expandtab:
;
; REX -- Raytheon EXec: a round-robin executive for the 703, preemptive
; and cooperative at once.
;
; Five tasks share the processor, their control blocks a ring of linked
; nodes the scheduler walks.  Three print a letter through an
; interrupt-driven teletype driver and then sleep for a fixed number of
; ticks, so the scheduling is visible in the output: the letters arrive
; at their own intervals, and when every task is asleep the idle task --
; a sixth node, off the ring -- has the processor.  One is a shell, which
; waits on a queue of keystrokes and runs a command.  And one is Tiny
; BASIC -- the interpreter of bcore.asm behind the glue of brex.asm --
; which the shell's BASIC command hands the console to, and whose BYE
; hands it back with the stored program kept for the next session.
;
; The letter tasks come up stopped; START sets them going.  The commands:
;
;   HELP or ?    the command list
;   STAT         every task's state, and how long each sleeper has left
;   UPTIME, UP   seconds since the executive came up
;   STOP  [A-C]  suspend a letter task, or all three
;   START [A-C]  release one, or all three
;   ECHO text    print the rest of the line
;   BASIC        the console goes to Tiny BASIC, until its BYE
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
;   PICK, SWTCH and Q.PUT, it interrupted a task; inside, it did not.
;   Q.GET is deliberately outside it: a task blocks in there, and a tick
;   that finds one has every business switching away from it.  Both
;   switching paths make it -- the tick at word 8, the teletype's service
;   routine at word 0 -- and both simply return when it fails.  Inside
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
;   interrupted.  Both service routines therefore open with SMB before
;   they touch anything.  (The named core test:
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
;   shell when it is clear, BASIC when it is set.  The shell's BASIC
;   command raises it in the same masked window that sets the task
;   running, then waits on the cell itself -- reading no queue -- until
;   BASIC's BYE clears it, wakes the shell under SERV's guard, and parks
;   the task.  Ctrl-C never enters the queue at all: SERV raises BRKREQ
;   instead, so a running program that reads no input can still be
;   broken, and the grant clears the flag so a break aimed at nobody
;   cannot land on the session that follows it.
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
;   finds, and a letter task is any node with a letter in T.CHR.
;
; * A FRESH TASK'S STATUS is GLB plus its entry page.  A zero status word
;   would resume the task in local mode with EXR 0 and its first memory
;   reference would land in page 0.  For an entry on a 1024-word byte
;   page boundary the EXR field is exactly (entry * 2); LTASK and IDLE
;   are in page 0, where the field is plain zero.
;
; * EVERYTHING RUNS GLOBAL.  START sets it, the TCB statuses carry it,
;   and entry and JSX force it -- EXIT's indexed JSX and every indexed
;   reference here assume a flat address.
;
; Memory map, everything below X'4000':
;
;   0000-000F  interrupt blocks: level 0 (teletype), level 1 unused,
;              level 2 (line clock), level 3 (never signalled -- SWTCH
;              stages a switch in it and loads it with INR 3)
;   0040-      page 0: START; then ISRBEG..ISREND, which is SCHED, SERV,
;              KICK, PICK, SWTCH and Q.PUT; then the kernel cells, the
;              console queue, the TCB nodes, Q.GET, the idle task and
;              LTASK, the one letter-task body all three letter nodes run
;   0800-      page 1: the shell -- banner, prompt, commands, line buffer
;
; Build with asm703.py; see the makefile's ray703-rex target.  Run:
;
;   ./target/debug/emu -s ray703 -r roms/703/rex.bin --fast-io

; ---------------------------------------------------------------- levels 0-3
                ORG     0
                JMP     START           ; word 0: clobbered by the PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0
                WORD    0,0,0,0         ; level 1: never enabled
                WORD    0               ; word 8: level 2 PCR save
                WORD    SCHED           ; level 2 linkage address
                WORD    0               ; word 10: level 2 machine status save
                WORD    0
                WORD    0               ; word 12: SWTCH stages a program
                WORD    0               ; counter and a status here and
                WORD    0               ; loads them with INR 3 -- see the
                WORD    0               ; header. Level 3 is never enabled.

L0PC            EQU     0               ; the level 0 block words SERV edits
L0ST            EQU     2               ; when it returns as another task
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

NRING           EQU     5               ; nodes on the ring: A, B, C, the
                                        ; shell and BASIC.  Idle is off it,
                                        ; the scans' explicit fallback.

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
                DOT     14,11           ; connect the keyboard; function 11
                                        ; is the one that echoes, which is
                                        ; the Model 33 printing what its own
                                        ; keyboard sent -- full duplex, and
                                        ; free of the printer's time
                ENB     0
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
                JSX     PICK
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

ISREND          EQU     $

; ---------------------------------------------------------------- kernel data
S0SAVA          WORD    0               ; level 0's register saves
S0SAVX          WORD    0
S2SAVA          WORD    0               ; level 2's register saves
S2SAVX          WORD    0
OWNER           WORD    X'FFFF'         ; node whose character is printing;
                                        ; minus one means the printer is idle
LASTS           WORD    ATCB            ; last node served, for fairness
KCAND           WORD    0               ; KICK's scan scratch
KTRY            WORD    0
SHUTREQ         WORD    0               ; set by the shell's HALT, read by tasks
BRKREQ          WORD    0               ; Ctrl-C arrived; set by SERV, cleared
                                        ; by whoever owns the console
CONBSY          WORD    0               ; the console belongs to BASIC: set by
                                        ; the shell as it grants, cleared by
                                        ; BASIC as it hands back, and the
                                        ; cell the shell waits on meanwhile
CURT            WORD    SHTCB           ; the current task's node: the kernel
                                        ; becomes the shell, so it starts on
                                        ; the shell's own
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
K1              WORD    1
K60             WORD    60
KM1             WORD    X'FFFF'
KWAIT           WORD    SWAI
KCONSQ          WORD    QCONS

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

; The task control nodes: A -> B -> C -> SH -> BA and round again, with
; idle off the ring and its link re-entering it, which is what lets every
; walk start uniformly at *T.NXT.  The shell's context is blank because
; the kernel becomes the shell and the first tick fills it in.  A status
; is GLB (X'80') plus the entry page in the EXR field, which for a
; 1024-word-aligned entry is exactly the entry doubled -- BASIC's entry
; is pinned to such a boundary, while LTASK and IDLE live in page 0 and
; their EXR is therefore plain zero.  A zero status word would resume a
; task in local mode pointed at page 0.
;
;                    STA    NXT   ACR IXR PCR    MST            DLY MBX CHR NAP NAM
ATCB            WORD SOFF, BTCB, 0,  0,  LTASK, X'80',         0,  0,  'A',30, 'A '
BTCB            WORD SOFF, CTCB, 0,  0,  LTASK, X'80',         0,  0,  'B',45, 'B '
CTCB            WORD SOFF, SHTCB,0,  0,  LTASK, X'80',         0,  0,  'C',60, 'C '
SHTCB           WORD SRUN, BATCB,0,  0,  0,     0,             0,  0,  0,  0,  'SH'
BATCB           WORD SOFF, ATCB, 0,  0,  BASENT,(BASENT*2)+X'80',0,0,  0,  0,  'BA'
IDTCB           WORD SRUN, ATCB, 0,  0,  IDLE,  X'80',         0,  0,  0,  0,  'ID'

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
                JSX     SHTOK           ; the command word
                LDW     STOK0
                SAZ                     ; an empty line is not an error
                JMP     SHDSP
                JMP     SHLOOP

; Walk the command table: two words of name, then the handler to jump to.
; Only the first four characters are matched, which is how the period
; interpreters did it -- START and STAT differ inside four, and a longer
; word that starts the same is simply taken as the command.
SHDSP           LDW     SHKTAB
                STW     SHTP
SHDL            LDX     SHTP
                LDW     *0
                SAZ                     ; the end of the table?
                JMP     SHDCM
                JMP     SHDNF
SHDCM           CMW     STOK0
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
                LDW     SHKNB           ; the ring, and idle
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

; Hand the console to BASIC.  One masked window grants it: the break
; flag cleared, so a Ctrl-C typed at this prompt cannot break the
; session's first statement; the task set running -- its context resumes
; wherever BYE parked it, and the very first grant starts it at BASENT;
; and CONBSY raised.  Then the shell waits on CONBSY the way everything
; waits here: masked look, SWAI, SWTCH, look again.  While it waits it
; reads no queue, which is what keeps QCONS to its one reader -- the
; console's reader is the shell exactly when CONBSY is clear, and BASIC
; exactly when it is set.
SHBAS           MSK
                CLR
                SMB     BRKREQ
                STW     BRKREQ
                CLR
                SMB     BA.STA
                STW     BA.STA
                LDW     SHK1
                SMB     CONBSY
                STW     CONBSY
                UNM
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
BA.STA          EQU     BATCB+T.STA     ; the BASIC node's, for the grant
BA.MBX          EQU     BATCB+T.MBX     ; and for brex.asm's output

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
SHSP            WORD    0               ; SHPRT's cursor and limit, bytes
SHSE            WORD    0
SHV             WORD    0               ; SHDEC's running value...
SHDP            WORD    0               ; ...and where its next digit goes
SHDB            RES     3               ; six digits, filled backwards

SHK1            WORD    1
SHK3            WORD    3
SHK4            WORD    4
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
SHKNB           WORD    NRING+1         ; nodes STAT prints, idle included
SHKRNG          WORD    ATCB            ; the ring, where the shell's walks
SHKIDL          WORD    IDTCB           ; start, and the idle node past it
SHKLBB          WORD    LBUF*2          ; the line, and the last byte its
SHKLBE          WORD    LBUF*2+62       ; zero terminator may need
SHKDBE          WORD    SHDB*2+5        ; the last byte of the digit buffer
SHKSTA          WORD    SHSTA
SHKTAB          WORD    SHTAB

; Four characters a state, indexed by the state doubled.
SHSTA           WORD    'RU','N ','SL','P ','OF','F ','WA','IT'

LBUF            RES     32              ; the line the shell is reading

; The commands: four characters of name, then where to go. '?' is HELP
; under another name, and UP is UPTIME under a shorter one.
SHTAB           WORD    'HE','LP',SHHELP
                WORD    X'BF00',0,SHHELP
                WORD    'ST','AT',SHSTAT
                WORD    'UP','TI',SHUPT
                WORD    'UP',0,SHUPT
                WORD    'ST','OP',SHSTOP
                WORD    'ST','AR',SHSTRT
                WORD    'EC','HO',SHECHO
                WORD    'BA','SI',SHBAS
                WORD    'HA','LT',SHHALT
                WORD    0,0,0

SHMBAN          WORD    SHBAN
SHMHNT          WORD    SHHNT
SHMPRM          WORD    SHPRM
SHMWHT          WORD    SHWHT
SHMHL1          WORD    SHHL1
SHMHL2          WORD    SHHL2
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
SHHL1T          TEXT    "COMMANDS HELP STAT UPTIME STOP START ECHO BASIC HALT\r\n"
SHHL1E          EQU     $
SHHL2T          TEXT    "STOP AND START TAKE A B OR C\r\n"
SHHL2E          EQU     $
SHUPMT          TEXT    "UPTIME"
SHUPME          EQU     $
SHSECT          TEXT    " SEC\r\n"
SHSECE          EQU     $
SHDWNT          TEXT    "REX 703 DOWN\r\n"
SHDWNE          EQU     $
SHNOTT          TEXT    "NO SUCH TASK\r\n"
SHNOTE          EQU     $
