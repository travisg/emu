; vim: ts=8:sw=8:expandtab:
;
; REX -- Raytheon EXec: a preemptive round-robin executive for the 703.
;
; Four tasks share the processor under a 60 Hz timer interrupt.  Three of
; them print their letter through an interrupt-driven teletype driver and
; then sleep for a fixed number of ticks, so the scheduler is visible in
; the output: the letters arrive at their own intervals, and when every
; task is asleep the idle task has the processor.  The fourth is a shell,
; which reads a line from the keyboard and runs a command:
;
;   HELP or ?    the command list
;   STAT         every task's state, and how long each sleeper has left
;   UPTIME, UP   seconds since the executive came up
;   STOP  [A-C]  suspend a letter task, or all three
;   START [A-C]  release one, or all three
;   ECHO text    print the rest of the line
;   HALT         park the tasks, drain the printer and stop the machine
;
; The Model 33 echoes what is typed in hardware, so the shell arms the
; keyboard for it (DOT 14,11) and never echoes a character itself.
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
; four words (ACR, IXR and those two), and a context switch is: copy them
; into the outgoing task's TCB, copy the incoming task's four back, INR 2.
; Because the status word travels too, a task can be preempted between an
; SMB and the reference it governs, or between a compare and its skip, and
; resume none the wiser.
;
; The rules that keep it sound.  Each is enforced exactly where it is
; stated, and nothing else in the listing may care:
;
; * THE DEFER CHECK.  SCHED switches only when the saved PC at word 8 lies
;   outside [ISRBEG, ISREND), the contiguous block holding SCHED, SERV and
;   KICK.  A tick that lands inside that range preempted the level 0
;   service (or its entry), and the level 2 block then holds the service
;   routine's frame, not a task's: switching on it would strand SERV's
;   INR 0 and park half a driver in a TCB.  Deferring costs at most one
;   tick.  The classification is exact because tasks execute that range
;   only under MSK -- a masked tick is held pending and fires after the
;   UNM, by which time the saved PC is back in task code.
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
; * MAILBOX OWNERSHIP.  Task i writes MBCH+i, one character, only when it
;   holds zero and only inside its masked window; SERV alone clears it,
;   which is the task's "printed" signal; KICK only reads.  Characters are
;   never zero, so zero means empty.
;
; * THE LINE BUFFER IS PRIMED BEFORE IT IS OPENED.  SERV fills it through
;   INPP and stops when LNRDY says a line is waiting, so the shell primes
;   the pointer and clears the flag in one masked window.  Both cells are
;   born with the buffer already primed and the flag already set, which is
;   what makes the gap between arming the keyboard and the shell's first
;   prime safe: a character typed into it is dropped rather than stored
;   through a pointer that is still zero, which is byte address zero,
;   which is the level 0 program counter save.
;
; * SHUTDOWN ORDERING.  Every letter task reads SHUTREQ inside the same
;   masked window as its deposit, and the shell -- the only writer of
;   SHUTREQ -- cannot run inside that window, so a task that saw zero has
;   its character banked before the shutdown exists and a task that saw
;   the flag parks without depositing.  Once the shell's drain finds the
;   letter tasks' mailboxes empty and the printer idle, nothing of theirs
;   can chase the down-message.
;
; * SLEEP IS SPIN PLUS STATE, because the machine has no instruction with
;   which to yield.  A task stores its delay and marks itself SLEEPING in
;   one masked window, then spins reading its own state; the scheduler
;   passes over a sleeping task, counts its delay down on every tick, and
;   marks it runnable again at zero, whereupon the spin falls through.  The
;   cost is the fraction of a tick the task spins before the first tick
;   takes the processor away -- the price of having nowhere to yield to.
;   The spin must live OUTSIDE [ISRBEG, ISREND): a task whose spin the
;   defer check mistook for the driver would never be switched away from.
;
; * THE IDLE TASK is what runs when every task is asleep, and the reason
;   the scheduler's scan can always finish.  It is a branch to self, which
;   is a legal idle here because the levels are enabled and unmasked, and
;   it is scanned by nobody: the scan covers the real tasks and falls back
;   to idle when none of them can run.
;
; * A FRESH TASK'S STATUS is GLB plus its entry page.  A zero status word
;   would resume the task in local mode with EXR 0 and its first memory
;   reference would land in page 0.  The entries sit on 1024-word byte
;   page boundaries, so the EXR field is exactly (entry * 2).
;
; * EVERYTHING RUNS GLOBAL.  START sets it, the TCB statuses carry it,
;   and entry and JSX force it -- EXIT's indexed JSX and every indexed
;   reference here assume a flat address.
;
; Memory map, everything below X'4000':
;
;   0000-000B  interrupt blocks: level 0 (teletype), level 1 unused,
;              level 2 (line clock; word 8 = saved PC, word 10 = status)
;   0040-      page 0: START, then ISRBEG..ISREND (SCHED, SERV, KICK),
;              then the kernel cells, the TCB table and the idle task
;   0800-      page 1: task A -- letter loop
;   1000-      page 2: task B -- letter loop
;   1800-      page 3: task C -- letter loop
;   2000-      page 4: the shell -- banner, prompt, commands
;
; Build with asm703.py; see the makefile's ray703-rex target.  Run:
;
;   ./target/debug/emu -s ray703 -r roms/703/rex.bin --fast-io

; ---------------------------------------------------------------- levels 0-2
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

L2PC            EQU     8               ; the level 2 block words SCHED edits:
L2ST            EQU     10              ; rewriting them before INR 2 is the switch

; A task control block: the four words of context the hardware and the
; switch move between them, then the two the scheduler runs on.
T.ACR           EQU     0
T.IXR           EQU     1
T.PCR           EQU     2
T.MST           EQU     3               ; machine status: EXR, indicators, mode
T.STA           EQU     4               ; S.RUN or S.SLP
T.DLY           EQU     5               ; ticks left, while sleeping
TCBW            EQU     6               ; words per block

S.RUN           EQU     0
S.SLP           EQU     1
S.OFF           EQU     2               ; suspended by the shell's STOP

NTASK           EQU     4               ; the tasks the scheduler scans...
TIDLE           EQU     TCBW*NTASK      ; ...and the idle task's block, past them
NLETT           EQU     3               ; of which the first three print letters

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Connect the keyboard and the clock, then *become* task A: TCB slot 0 is
; left blank and the first tick fills it with whatever A was doing.  ENB
; before UNM because a masked signal is held where a disabled one is
; dropped; ENB 2 before the arming DOT so not even the first tick can be
; dropped -- it is 9,523 cycles out, held by the mask until the UNM.  A
; tick that lands on the two instructions after the UNM parks this tail in
; TCB slot 0, which is exactly right.
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
SCTK0           CLR
                STW     SCIX
SCTKL           LDX     SCIX
                LDW     *TCBT+T.STA
                CMW     KSLP
                SEQ                     ; asleep?
                JMP     SCTKN
                LDW     *TCBT+T.DLY
                SUB     K1
                STW     *TCBT+T.DLY
                SAZ                     ; the last tick of the sleep?
                JMP     SCTKN
                CLR
                STW     *TCBT+T.STA     ; wake it
SCTKN           LDW     SCIX
                ADD     KTCBW
                STW     SCIX
                CMW     KTIDLE
                SLS                     ; another real task to visit?
                JMP     SCDEF
                JMP     SCTKL

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
SCSW            LDX     CURX            ; IXR = the current task's block
                LDW     S2SAVA
                STW     *TCBT+T.ACR
                LDW     S2SAVX
                STW     *TCBT+T.IXR
                LDW     L2PC
                STW     *TCBT+T.PCR
                LDW     L2ST
                STW     *TCBT+T.MST

; Round robin over the tasks that can run, starting past the one just
; parked.  A task that is asleep is simply skipped; if none of them can
; run the idle task always can, which is what makes this scan terminate.
                LDW     CURX
                STW     SCIX
                LDW     KNTASK
                STW     SCTRY
SCSCN           LDW     SCIX
                ADD     KTCBW
                CMW     KTIDLE
                SLS
                CLR                     ; past the last block: wrap
                STW     SCIX
                CAX
                LDW     *TCBT+T.STA
                SAZ                     ; runnable?
                JMP     SCNRD
                JMP     SCPIK
SCNRD           LDW     SCTRY
                SUB     K1
                STW     SCTRY
                SAZ                     ; any candidate left to look at?
                JMP     SCSCN
                LDW     KTIDLE          ; every task is asleep: go idle
                STW     SCIX
SCPIK           LDW     SCIX
                STW     CURX
                LDX     CURX
                LDW     *TCBT+T.PCR
                STW     L2PC            ; incoming PC and status go into the
                LDW     *TCBT+T.MST     ; level block; INR does the loading
                STW     L2ST
                LDW     *TCBT+T.IXR
                STW     S2SAVX          ; park the incoming IXR -- the index
                LDW     *TCBT+T.ACR     ; register still holds the TCB offset
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
STX0            LDX     OWNER
                CLR
                STW     *MBCH0          ; the owner's "printed" signal
                LDW     KM1
                STW     OWNER
                JSX     KICK            ; start the next waiting character
SRX             DIN     14,15           ; collect the frame, and ask for
                SAZ                     ; another; empty is the merge's
                JMP     SRX1            ; other half, not an error
                JMP     SEXIT

; File the character in the line buffer.  Nothing is echoed here: the
; teletype is armed for its own echo, so what the operator sees is the
; Model 33 printing its own keyboard, and this routine only stores.
SRX1            STW     SCHR
                LDW     LNRDY
                SAZ                     ; a line still waiting to be read?
                JMP     SEXIT           ; yes: drop this one, as a busy
                JMP     SRXOK           ; machine dropped what was typed at it
SRXOK           LDW     SCHR
                CLB     X'8D'           ; carriage return ends the line
                SNE
                JMP     SRXEOL
                CLB     X'8A'           ; and so does a line feed, so a
                SNE                     ; script piped in with newline
                JMP     SRXEOL          ; endings reads like a typed Return
                CLB     X'FF'           ; rubout backs up over a character
                SNE
                JMP     SRXRUB
                CLB     X'E1'           ; below 'a'?
                SLS
                JMP     SRXHI
                JMP     SRXPUT
SRXHI           CLB     X'FA'           ; above 'z'?
                SGR
                AND     UPMASK          ; in range: clear bit 5, folding to
                                        ; upper case, which is all the
                                        ; commands are written in
SRXPUT          STW     SCHR
                LDW     INPP
                CMW     INPE            ; room for one more?
                SNE
                JMP     SEXIT           ; no: drop it
                CAX
                LDW     SCHR
                STB     *0
                LDW     INPP
                ADD     K1
                STW     INPP
                JMP     SEXIT

; A printing terminal cannot take ink back, so the rubout prints as itself
; and only the buffer forgets the character.
SRXRUB          LDW     INPP
                CMW     INPB            ; anything to back up over?
                SEQ
                JMP     SRXR1
                JMP     SEXIT
SRXR1           SUB     K1
                STW     INPP
                JMP     SEXIT

; Terminate the line and hand it to the shell. INPE leaves room for this
; zero even when the buffer filled up.
SRXEOL          LDW     INPP
                CAX
                CLR
                STB     *0
                LDW     K1
                STW     LNRDY
SEXIT           LDW     S0SAVA
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
                LDW     KNTASK
                STW     KTRY
                LDW     LASTS
KSCN            ADD     K1              ; next candidate, wrapping to the first
                CMW     KNTASK
                SLS
                CLR
                STW     KCAND
                CAX
                LDW     *MBCH0          ; that task's mailbox
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
                LDW     *MBCH0
                DOT     14,14           ; teletype, write the character
KDONE           EXIT    KICK

ISREND          EQU     $

; ---------------------------------------------------------------- kernel data
S0SAVA          WORD    0               ; level 0's register saves
S0SAVX          WORD    0
S2SAVA          WORD    0               ; level 2's register saves
S2SAVX          WORD    0
OWNER           WORD    X'FFFF'         ; task whose character is printing;
                                        ; minus one means the printer is idle
LASTS           WORD    0               ; last mailbox served, for fairness
KCAND           WORD    0               ; KICK's scan scratch
KTRY            WORD    0
MBCH0           WORD    0               ; one-character mailboxes, tasks A, B,
MBCH1           WORD    0               ; C and the shell -- contiguous, KICK
MBCH2           WORD    0               ; and SERV index them from MBCH0
MBCH3           WORD    0
SHUTREQ         WORD    0               ; set by the shell's HALT, read by tasks
BGATE           WORD    0               ; set by the shell when the banner is out
CURX            WORD    TCBW*3          ; the current task's block offset: the
                                        ; kernel becomes the shell, so it
                                        ; starts on the shell's own block
SCIX            WORD    0               ; SCHED's walk over the blocks
SCTRY           WORD    0               ; candidates left in the scan
TICKS           WORD    0               ; ticks into the current second...
SECS            WORD    0               ; ...and seconds since REX came up
SCHR            WORD    0               ; SERV's character scratch

; The line the shell reads.  INPP and LNRDY are born primed and ready --
; see the header: a character typed before the shell's first prime has to
; be dropped, not stored through a pointer that is still zero.
LNRDY           WORD    1
INPP            WORD    LBUF*2
INPB            WORD    LBUF*2          ; where the line starts...
INPE            WORD    LBUF*2+62       ; ...and the last byte the zero
                                        ; terminator may need
K1              WORD    1
K60             WORD    60
UPMASK          WORD    X'FFDF'
KM1             WORD    X'FFFF'
KSLP            WORD    S.SLP
KTCBW           WORD    TCBW
KNTASK          WORD    NTASK
KTIDLE          WORD    TIDLE
KISRB           WORD    ISRBEG
KISRE           WORD    ISREND

; The task control blocks.  The shell's is blank because the kernel
; becomes the shell and the first tick fills it in.  A status is GLB (X'80') plus the
; entry page in the EXR field, which for a 1024-word-aligned entry is
; exactly the entry doubled -- the identity holds for the three task pages
; and not for the idle task, which lives in page 0 and whose EXR is
; therefore plain zero.  A zero status word would resume a task in local
; mode pointed at page 0.
TCBT            WORD    0,0,ATASK,(ATASK*2)+X'80',S.RUN,0
                WORD    0,0,BTASK,(BTASK*2)+X'80',S.RUN,0
                WORD    0,0,CTASK,(CTASK*2)+X'80',S.RUN,0
                WORD    0,0,0,0,S.RUN,0
                WORD    0,0,IDLE,X'80',S.RUN,0

; What the machine runs when every task is asleep.  A branch to self is a
; legal idle here -- the levels are enabled and unmasked, so the tick that
; ends somebody's sleep takes the processor away from it -- and it sits
; outside [ISRBEG, ISREND) like any other task's code, or the scheduler
; could never switch away from it.
IDLE            JMP     IDLE

LBUF            RES     32              ; the shell's line, 64 bytes

; ---------------------------------------------------------------- task A
; A letter task, and the model for the two below it: wait for the shell to
; finish the banner, then print a letter and sleep, forever.  The masked
; window is the whole protocol -- SHUTREQ read and the character deposited
; with SERV locked out, so a deposit can never follow an observed shutdown.
                ORG     X'800'

ATASK           SMB     BGATE
                LDW     BGATE
                SAZ
                JMP     ARUN
                JMP     ATASK
ARUN            MSK
                SMB     SHUTREQ
                LDW     SHUTREQ
                SAZ
                JMP     AQUIT
                LDW     ACH
                SMB     MBCH0
                STW     MBCH0
                SMB     KICK
                JSX     KICK
                UNM
AWAIT           SMB     MBCH0
                LDW     MBCH0
                SAZ                     ; cleared by SERV when printed
                JMP     AWAIT

; Sleep ANAPN ticks: store the delay and the state in one masked window,
; so the tick cannot read half of it, then spin on the state until the
; scheduler marks this task runnable again.  The spin burns whatever is
; left of the current tick -- there is no instruction with which to give
; the processor back -- and nothing after that, because a sleeping task is
; passed over by the scan.  It sits here in the task's own page, outside
; the range the scheduler defers on, or the tick could never take the
; processor away from it.
                MSK
                LDW     ANAPN
                SMB     A.DLY
                STW     A.DLY
                LDW     AKSLP
                SMB     A.STA
                STW     A.STA
                UNM
ANAPW           SMB     A.STA
                LDW     A.STA
                SAZ                     ; awake again?
                JMP     ANAPW
                JMP     ARUN
AQUIT           UNM
APARK           JMP     APARK           ; parked; a legal idle, levels live

A.STA           EQU     TCBT+0*TCBW+T.STA   ; this task's own block fields
A.DLY           EQU     TCBT+0*TCBW+T.DLY

ACH             WORD    'A'
AKSLP           WORD    S.SLP
ANAPN           WORD    30              ; half a second between letters

; ---------------------------------------------------------------- task B
; Wait for the banner, then print the letter forever.  The masked window
; is the whole protocol: SHUTREQ checked and the character deposited with
; SERV locked out, so a deposit can never follow an observed shutdown.
                ORG     X'1000'

BTASK           SMB     BGATE
                LDW     BGATE
                SAZ
                JMP     BRUN
                JMP     BTASK
BRUN            MSK
                SMB     SHUTREQ
                LDW     SHUTREQ
                SAZ
                JMP     BQUIT
                LDW     BCH
                SMB     MBCH1
                STW     MBCH1
                SMB     KICK
                JSX     KICK
                UNM
BWAIT           SMB     MBCH1
                LDW     MBCH1
                SAZ                     ; cleared by SERV when printed
                JMP     BWAIT

; Sleep BNAPN ticks; see task A's ANAP for what the two cells mean and why
; the spin has to sit out here in the task's own page.
                MSK
                LDW     BNAPN
                SMB     B.DLY
                STW     B.DLY
                LDW     BKSLP
                SMB     B.STA
                STW     B.STA
                UNM
BNAPW           SMB     B.STA
                LDW     B.STA
                SAZ                     ; awake again?
                JMP     BNAPW
                JMP     BRUN
BQUIT           UNM
BPARK           JMP     BPARK           ; parked; a legal idle, levels live

B.STA           EQU     TCBT+1*TCBW+T.STA
B.DLY           EQU     TCBT+1*TCBW+T.DLY

BCH             WORD    'B'
BKSLP           WORD    S.SLP
BNAPN           WORD    45              ; three quarters of a second

; ---------------------------------------------------------------- task C
                ORG     X'1800'

CTASK           SMB     BGATE
                LDW     BGATE
                SAZ
                JMP     CRUN
                JMP     CTASK
CRUN            MSK
                SMB     SHUTREQ
                LDW     SHUTREQ
                SAZ
                JMP     CQUIT
                LDW     CCH
                SMB     MBCH2
                STW     MBCH2
                SMB     KICK
                JSX     KICK
                UNM
CWAIT           SMB     MBCH2
                LDW     MBCH2
                SAZ
                JMP     CWAIT
                MSK
                LDW     CNAPN
                SMB     C.DLY
                STW     C.DLY
                LDW     CKSLP
                SMB     C.STA
                STW     C.STA
                UNM
CNAPW           SMB     C.STA
                LDW     C.STA
                SAZ                     ; awake again?
                JMP     CNAPW
                JMP     CRUN
CQUIT           UNM
CPARK           JMP     CPARK

C.STA           EQU     TCBT+2*TCBW+T.STA
C.DLY           EQU     TCBT+2*TCBW+T.DLY

CCH             WORD    'C'
CKSLP           WORD    S.SLP
CNAPN           WORD    60              ; a second

; ---------------------------------------------------------------- the shell
; Task 3.  Prints the banner, opens the gate for the letter tasks, and
; then reads a line and runs it, forever.  Everything it prints goes
; through its own mailbox one character at a time like any other task's
; letter, so a command's output and the background letters interleave on
; the printer exactly as two users' output did.
                ORG     X'2000'

SHELL           LDW     SHMBAN
                JSX     SHMSG
                LDW     SHK1
                SMB     BGATE
                STW     BGATE           ; the letter tasks may speak now

SHLOOP          LDW     SHMPRM
                JSX     SHMSG

; Open the line buffer.  Masked, because SERV fills it through INPP and
; must see the pointer primed and the flag cleared together or not at all.
                MSK
                LDW     SHKLBB
                SMB     INPP
                STW     INPP
                CLR
                SMB     LNRDY
                STW     LNRDY
                UNM

; Wait for a line, sleeping between looks: a shell that spun here would be
; runnable forever and the idle task would never see the processor.
SHWT            SMB     LNRDY
                LDW     LNRDY
                SAZ                     ; a line yet?
                JMP     SHGO
                JSX     SHNAP
                JMP     SHWT

SHGO            LDW     SHKLBB
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

; The task table: index, name, state, and how much longer a sleeper has.
SHSTAT          JSX     SHPUP
                CLR
                STW     SHTI
                STW     SHTO
SHSTL           LDW     SHTI
                JSX     SHDIG
                LDW     SHKSP
                JSX     SHPUTC
                LDW     SHKNAM          ; its two-character name
                ADD     SHTI
                CAX
                LDW     *0
                JSX     SHPW2
                LDW     SHKSP
                JSX     SHPUTC
                LDX     SHTO            ; its state, as four characters
                LDW     *TCBT+T.STA
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
                LDW     *TCBT+T.STA
                CMW     SHKSLP          ; asleep? then say for how long
                SEQ
                JMP     SHSTN
                LDX     SHTO
                LDW     *TCBT+T.DLY
                JSX     SHDEC
SHSTN           JSX     SHNL
                LDW     SHTO
                ADD     SHKTCW
                STW     SHTO
                LDW     SHTI
                ADD     SHK1
                STW     SHTI
                CMW     SHKNB           ; every block, the idle one included
                SLS
                JMP     SHLOOP
                JMP     SHSTL

; STOP and START differ only in the state they store. With no argument
; they take all three letter tasks; with one they take the task that
; letter names, and nothing else -- the shell must not be able to suspend
; itself, since it is the only way to start anything again.
SHSTOP          LDW     SHKOFF
                STW     SHNST
                JMP     SHSSET
SHSTRT          CLR
                STW     SHNST
SHSSET          JSX     SHARG
                LDW     SHARGF
                SAZ                     ; no argument: all of them
                JMP     SHSONE
                JMP     SHSALL
SHSALL          CLR
                STW     SHTO
SHSAL1          LDX     SHTO
                LDW     SHNST
                STW     *TCBT+T.STA
                LDW     SHTO
                ADD     SHKTCW
                STW     SHTO
                CMW     SHKNL           ; the letter tasks only
                SLS
                JMP     SHLOOP
                JMP     SHSAL1
SHSONE          LDW     SHARGF
                CMW     SHK2            ; not a task letter at all?
                SEQ
                JMP     SHSO1
                JMP     SHSOBD
SHSO1           LDW     SHARGV          ; six words to a block
                SLL     1
                STW     SHW2
                SLL     1
                ADD     SHW2
                CAX
                LDW     SHNST
                STW     *TCBT+T.STA
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

; Shut the machine down.  The letter tasks park at their next masked
; window, so once their mailboxes are empty and the printer is idle
; nothing of theirs can appear inside the down-message.
SHHALT          LDW     SHK1
                SMB     SHUTREQ
                STW     SHUTREQ
SHHDR           SMB     MBCH0
                LDW     MBCH0
                SMB     MBCH1
                ORI     MBCH1
                SMB     MBCH2
                ORI     MBCH2
                SAZ                     ; all three empty?
                JMP     SHHDR
                JMP     SHHD1
SHHD1           SMB     OWNER
                LDW     OWNER
                SAM                     ; and the printer idle?
                JMP     SHHDR
                LDW     SHMDWN
                JSX     SHMSG
SHHDW           SMB     MBCH3           ; and now its own last character
                LDW     MBCH3
                SAZ
                JMP     SHHDW
                JMP     SHHD2
SHHD2           SMB     OWNER
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

; An argument naming a letter task: SHARGF is 0 for none, 1 with the index
; in SHARGV, 2 for something that is not a task at all.
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
SHAG2           STW     SHW2
                CMW     SHKCA           ; below 'A'?
                SLS
                JMP     SHAG3
                JMP     SHAGB
SHAG3           LDW     SHW2
                CMW     SHKCC           ; above 'C'?
                SGR
                JMP     SHAG4
                JMP     SHAGB
SHAG4           LDW     SHW2
                SUB     SHKCA
                STW     SHARGV
                LDW     SHK1
                STW     SHARGF
                EXIT    SHARG
SHAGB           LDW     SHK2
                STW     SHARGF
                EXIT    SHARG

; ---------------------------------------------------------------- output
; One character from ACR: deposit in the shell's mailbox, kick the
; printer, spin until SERV reports it gone.  The deposit window is masked
; for KICK's sake; the spin is not, so the tick is free to schedule around
; a task that is only waiting on the printer.
SHPUTC          SUBR
                AND     SHK0FF
                MSK
                SMB     MBCH3
                STW     MBCH3
                SMB     KICK
                JSX     KICK
                UNM
SHPWT           SMB     MBCH3
                LDW     MBCH3
                SAZ
                JMP     SHPWT
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

SHDIG           SUBR
                ADD     SHKZER
                JSX     SHPUTC
                EXIT    SHDIG

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

; Sleep SHKNAP ticks; the letter tasks' ANAP says how this works.
SHNAP           SUBR
                MSK
                LDW     SHKNAP
                SMB     SH.DLY
                STW     SH.DLY
                LDW     SHKSLP
                SMB     SH.STA
                STW     SH.STA
                UNM
SHNAPW          SMB     SH.STA
                LDW     SH.STA
                SAZ                     ; awake again?
                JMP     SHNAPW
                EXIT    SHNAP

; ---------------------------------------------------------------- shell data
SH.STA          EQU     TCBT+3*TCBW+T.STA
SH.DLY          EQU     TCBT+3*TCBW+T.DLY

SHCUR           WORD    0               ; the cursor into the line, a byte
SHTP            WORD    0               ; the command table cursor            
SHTI            WORD    0               ; STAT's task index...
SHTO            WORD    0               ; ...and its block offset
SHSA            WORD    0               ; the state name being printed
SHNST           WORD    0               ; the state STOP or START will store
SHARGF          WORD    0               ; 0 none, 1 in SHARGV, 2 not a task
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
SHK2            WORD    2
SHK3            WORD    3
SHK4            WORD    4
SHK0FF          WORD    X'00FF'
SHKTEN          WORD    10
SHKZER          WORD    '0'
SHKSP           WORD    ' '
SHKCR           WORD    X'008D'
SHKLF           WORD    X'008A'
SHKCA           WORD    'A'
SHKCC           WORD    'C'
SHKSLP          WORD    S.SLP
SHKOFF          WORD    S.OFF
SHKNAP          WORD    2               ; ticks between looks at the buffer
SHKTCW          WORD    TCBW
SHKNB           WORD    NTASK+1         ; blocks STAT prints, idle included
SHKNL           WORD    TCBW*NLETT      ; past the last letter task's block
SHKLBB          WORD    LBUF*2
SHKDBE          WORD    SHDB*2+5        ; the last byte of the digit buffer
SHKNAM          WORD    SHNAM
SHKSTA          WORD    SHSTA
SHKTAB          WORD    SHTAB

; Two characters a task, indexed by its number.
SHNAM           WORD    'A ','B ','C ','SH','ID'

; Four a state, indexed by the state doubled.
SHSTA           WORD    'RU','N ','SL','P ','OF','F '

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
                WORD    'HA','LT',SHHALT
                WORD    0,0,0

SHMBAN          WORD    SHBAN
SHMPRM          WORD    SHPRM
SHMWHT          WORD    SHWHT
SHMHL1          WORD    SHHL1
SHMHL2          WORD    SHHL2
SHMUPM          WORD    SHUPM
SHMSEC          WORD    SHSEC
SHMDWN          WORD    SHDWN
SHMNOT          WORD    SHNOT

SHBAN           WORD    SHBANT*2,SHBANE*2
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
SHPRMT          TEXT    "REX>  "
SHPRME          EQU     $
SHWHTT          TEXT    "WHAT\r\n"
SHWHTE          EQU     $
SHHL1T          TEXT    "COMMANDS HELP STAT UPTIME STOP START ECHO HALT\r\n"
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
