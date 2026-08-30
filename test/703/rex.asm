; vim: ts=8:sw=8:expandtab:
;
; REX -- Raytheon EXec: a preemptive round-robin executive for the 703.
;
; Three tasks share the processor under a 60 Hz timer interrupt, each
; printing its letter through an interrupt-driven teletype driver, so the
; output is the scheduler made visible: at ten characters a second the
; letters interleave; under --fast-io each task streams a run of its letter
; until the next tick takes the processor away.  A '.' from the keyboard
; shuts it down -- the tasks park, the printer drains, REX 703 DOWN, halt.
; Nothing else typed is echoed or used.
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
; * SHUTDOWN ORDERING.  B and C read SHUTREQ inside the same masked window
;   as their deposit.  SERV -- the only writer of SHUTREQ -- cannot run
;   inside that window, so a task that saw zero has its character banked
;   before the shutdown exists, and a task that saw the flag parks without
;   depositing.  Once task A's drain finds every mailbox empty and the
;   printer idle, no letter can chase the down-message.
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
;              then the kernel cells and the TCB table
;   0800-      page 1: task A -- banner, gate, letter loop, supervisor
;   1000-      page 2: task B -- letter loop
;   1800-      page 3: task C -- letter loop
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
                DOT     14,9            ; connect the keyboard, no echo
                ENB     0
                ENB     2
                DOT     2,1             ; connect the line clock
                UNM
                SMB     ATASK
                JMP     ATASK

; ------------------------------------------------------- level 2 service
; The scheduler.  Everything from ISRBEG to ISREND runs at interrupt level
; or under MSK -- the defer check in the header relies on it, so nothing
; else may live between the two labels.
ISRBEG          EQU     $

SCHED           SMB     S2SAVA          ; the SMB lead: EXR still holds the
                STW     S2SAVA          ; interrupted task's page
                STX     S2SAVX
                LDW     L2PC            ; where did the tick land?
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
SCSW            LDX     CURX            ; IXR = 4 * current task
                LDW     S2SAVA
                STW     *TCBT           ; outgoing ACR
                LDW     S2SAVX
                STW     *TCBT+1         ; outgoing IXR
                LDW     L2PC
                STW     *TCBT+2         ; outgoing PC
                LDW     L2ST
                STW     *TCBT+3         ; outgoing status
                LDW     CURX            ; round robin: 0, 4, 8, 0, ...
                ADD     K4
                CMW     K12
                SLS
                CLR
                STW     CURX
                LDX     CURX
                LDW     *TCBT+2
                STW     L2PC            ; incoming PC and status go into the
                LDW     *TCBT+3         ; level block; INR does the loading
                STW     L2ST
                LDW     *TCBT+1
                STW     S2SAVX          ; park the incoming IXR -- the index
                LDW     *TCBT           ; register still holds the TCB offset
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
SRX             DIN     14,13           ; collect the frame, and ask for
                SAZ                     ; another; empty is the merge's
                JMP     SRX1            ; other half, not an error
                JMP     SEXIT
SRX1            CLB     X'AE'           ; '.' as the teletype delivers it,
                SNE                     ; bit 7 set -- ask REX to shut down
                JMP     SDOTK
                JMP     SEXIT           ; everything else is ignored
SDOTK           LDW     K1
                STW     SHUTREQ
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
                LDW     K3
                STW     KTRY
                LDW     LASTS
KSCN            ADD     K1              ; next candidate, wrapping 3 to 0
                CMW     K3
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
MBCH0           WORD    0               ; one-character mailboxes, task A, B,
MBCH1           WORD    0               ; C -- contiguous, KICK and SERV
MBCH2           WORD    0               ; index them from MBCH0
SHUTREQ         WORD    0               ; set by SERV on '.', read by tasks
BGATE           WORD    0               ; set by task A when the banner is out
CURX            WORD    0               ; 4 * current task, pre-scaled
K1              WORD    1
K3              WORD    3
K4              WORD    4
K12             WORD    12
KM1             WORD    X'FFFF'
KISRB           WORD    ISRBEG
KISRE           WORD    ISREND

; The TCB table, four words per task: ACR, IXR, PC, status.  Slot 0 is
; blank because the kernel becomes task A and the first tick fills it.  A
; status is GLB (X'80') plus the entry page in the EXR field, which for a
; 1024-word-aligned entry is exactly the entry doubled; a zero status
; would resume the task in local mode pointed at page 0.
TCBT            WORD    0,0,0,0
                WORD    0,0,BTASK,(BTASK*2)+X'80'
                WORD    0,0,CTASK,(CTASK*2)+X'80'

; ---------------------------------------------------------------- task A
; Prints the banner, opens the gate for B and C, then prints its letter
; like everyone else -- and watches SHUTREQ, because somebody has to run
; the shutdown.  APUTC and APRT are private to this task, so their link
; slots and cells never see two callers.
                ORG     X'800'

ATASK           LDW     ABANA
                STW     ABANP
                LDW     ABANE
                STW     APEND
                JSX     APRT            ; the banner, ten characters a second
                LDW     AK1
                SMB     BGATE
                STW     BGATE           ; B and C may speak now
ARUN            SMB     SHUTREQ
                LDW     SHUTREQ
                SAZ
                JMP     AQUIT
                LDW     ACH
                JSX     APUTC
                JMP     ARUN

; Shutdown.  Wait for every letter still in flight to reach the printer --
; B and C saw the flag inside their masked windows and cannot deposit
; again, so the mailboxes only empty -- then the down-message, then halt.
; The clock is disconnected first so no tick lands mid-drain with the
; interrupt system half torn down.
AQUIT           SMB     MBCH0
                LDW     MBCH0
                SMB     MBCH1
                ORI     MBCH1
                SMB     MBCH2
                ORI     MBCH2
                SAZ
                JMP     AQUIT
                SMB     OWNER
                LDW     OWNER
                SAM                     ; and the last character has printed
                JMP     AQUIT
                LDW     ADWNA
                STW     ABANP
                LDW     ADWNE
                STW     APEND
                JSX     APRT
                MSK
                DOT     2,0             ; disconnect the line clock
                HLT

; Print one character from ACR: deposit in task A's mailbox, kick the
; printer, spin until SERV reports it gone.  The deposit window is masked
; for KICK's sake; the spin is not, so the tick is free to schedule around
; a task that is only waiting on the printer.
APUTC           SUBR
                MSK
                SMB     MBCH0
                STW     MBCH0
                SMB     KICK
                JSX     KICK
                UNM
APWT            SMB     MBCH0
                LDW     MBCH0
                SAZ
                JMP     APWT
                EXIT    APUTC

; Print the bytes from ABANP up to APEND through APUTC.
APRT            SUBR
APRL            LDW     ABANP
                CMW     APEND
                SNE
                JMP     APRD
                CAX                     ; index <- the byte address
                CLR                     ; LDB replaces only the low half (2-1)
                LDB     *0
                JSX     APUTC
                LDW     ABANP
                ADD     AK1
                STW     ABANP
                JMP     APRL
APRD            EXIT    APRT

ACH             WORD    'A'
AK1             WORD    1
ABANP           WORD    0               ; APRT's cursor and limit, byte
APEND           WORD    0               ; addresses
ABANA           WORD    ABAN*2
ABANE           WORD    ABANST*2
ADWNA           WORD    ADWN*2
ADWNE           WORD    ADWNST*2

; Neither message contains an A, a B or a C, so the end-to-end test can
; isolate what the tasks printed.
ABAN            TEXT    "REX 703 UP\r\n"
ABANST          EQU     $
ADWN            TEXT    "\r\nREX 703 DOWN\r\n"
ADWNST          EQU     $

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
                JMP     BRUN
BQUIT           UNM
BPARK           JMP     BPARK           ; parked; a legal idle, levels live

BCH             WORD    'B'

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
                JMP     CRUN
CQUIT           UNM
CPARK           JMP     CPARK

CCH             WORD    'C'
