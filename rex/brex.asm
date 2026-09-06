; vim: ts=8:sw=8:expandtab:
;
; Tiny BASIC under REX -- the wrapper that makes the interpreter a module.
;
; This file sits between rexapi.asm and test/703/bcore.asm in the deck
; that builds basic.obj, a relocatable module the shell's RUN BASIC loads
; off the disc and hands the console to.  bcore.asm's header lists what a
; wrapper owes the core; this one pays those debts with the executive's
; services, through the vector and the cells rexapi.asm names:
;
;   - the workspace is one block from the kernel's pool, asked for at
;     entry: the core reaches every part of it through address cells,
;     assembled here as offsets into the block, and BASENT adds the
;     block's address to each before the core runs.  No core to be had
;     prints NO CORE LEFT and exits.
;   - output deposits one character at a time in this task's own
;     mailbox, the node CURT names, and stands down until SERV reports
;     it printed -- the same dance as the shell's SHPUTC.  The old
;     driver's column count for PRINT's comma zones lives in T.PUTC here.
;   - input takes characters from the console queue with K.QGET, which
;     parks this task in the kernel until the teletype has one.  The
;     Model 33 is armed for hardware echo, so nothing here echoes an
;     ordinary character; the rubout's backslash still prints, because
;     only the buffer can take a character back, not the paper.
;   - T.BRK is the kernel's BRKREQ: SERV sniffs Ctrl-C out of the input
;     stream and raises it, and the core's break check between statements
;     reads and clears it exactly as it did the driver's flag.  That cell
;     is in the kernel's page, which is why the core keeps an SMB in front
;     of its two references.
;   - BYE is K.EXIT: the kernel hands the console back to the shell and
;     gives back the module, its node and the workspace, so the next RUN
;     BASIC starts afresh.
;
; The glue and the core are one module in one 2048-word page, so the core
; runs with no page selection anywhere; the SMB pairs below are this
; wrapper's own, reaching the kernel's cells and entries.  Every module
; reference here is to a location the loader relocates, or to an
; rexapi.asm address behind an SMB.
;
; The workspace, as offsets into the block.  Everything byte-addressed --
; the line buffer and the heap -- must land below word X'4000' so byte
; pointers stay positive under this machine's signed-only compares, which
; the pool's own ceiling sees to.
REXGLUE         EQU     1               ; BYE hands the console back
B.ENTRY         EQU     BASENT          ; the module's entry, for END

W.LBUF          EQU     0               ; input line buffer, 41 words
W.LBUFSZ        EQU     79              ; typed bytes; byte 80 holds the CR
W.VARS          EQU     41              ; A-Z, 26 words
W.ESTK          EQU     67              ; expression operand stack, 16 words
W.OSTK          EQU     83              ; expression operator stack, 16 words
W.GSTK          EQU     99              ; GOSUB stack, 8 one-word frames
W.FSTK          EQU     107             ; FOR stack, 8 four-word frames
W.NBUF          EQU     139             ; number-print digit scratch, 5 words
W.ARRAY         EQU     144             ; @(0..1023)
W.HEAP          EQU     1168            ; program line heap...
W.HEAPSZ        EQU     1024            ; ...this big
W.HEAPTOP       EQU     W.HEAP+W.HEAPSZ
W.SIZE          EQU     W.HEAPTOP       ; the block

T.BRK           EQU     BRKREQ          ; the break flag is the kernel's cell

; ---------------------------------------------------------------- entry
; Take the workspace, point the core's address cells at it -- the word
; cells get the block's address, the byte cells twice that -- print the
; banner, and hand over to the core.
BASENT          LDW     BKWSZ
                SMB     K.ALLOC
                JSX     K.ALLOC
                SAZ                     ; a block?
                JMP     BWOK
                LDW     BKNOCA          ; none: say so and go
                JSX     M.MSG
                SMB     K.EXIT
                JMP     K.EXIT
BWOK            STW     BWBASE
                SLL     1
                STW     BWBAS2
                LDW     BKWTAB
                STW     BWADD
                LDW     BWBASE
                STW     BWADV
                JSX     BWFIX
                LDW     BKBTAB
                STW     BWADD
                LDW     BWBAS2
                STW     BWADV
                JSX     BWFIX
                LDW     BKBANP          ; the banner descriptor's address --
                JSX     M.MSG           ; M.MSG wants the pointer, and prints
                JMP     B.COLD          ; through the core's own window path

; Add BWADV to every cell the table at BWADD names; a zero ends the table.
BWFIX           SUBR
BWFL            LDX     BWADD
                LDW     *0
                SAZ
                JMP     BWF1
                EXIT    BWFIX
BWF1            CAX
                LDW     *0
                ADD     BWADV
                STW     *0
                LDW     BWADD
                ADD     BK1
                STW     BWADD
                JMP     BWFL

; ---------------------------------------------------------------- output
; T.PUTC: print the character in ACR's low half.  Count the column first
; -- carriage return restarts it, line feed leaves it alone, anything
; else advances it -- then the mailbox dance: deposit under MSK, kick the
; printer, and wait masked-look/SWAI/SWTCH until SERV clears the cell.
T.PUTC          SUBR
                AND     BK0FF           ; LLB callers leave the high half
                STW     BCH             ; full of whatever came before
                CLB     X'8D'
                SNE
                JMP     BPCCR
                CLB     X'8A'
                SEQ
                JMP     BPCINC
                JMP     BPCGO
BPCCR           CLR
                STW     T.COL
                JMP     BPCGO
BPCINC          LDW     T.COL
                ADD     BK1
                STW     T.COL
BPCGO           MSK
                LDW     BCH
                SMB     CURT
                LDX     CURT
                STW     *T.MBX
                SMB     K.KICK
                JSX     K.KICK
                UNM
BPCWT           MSK
                SMB     CURT
                LDX     CURT
                LDW     *T.MBX
                SAZ                     ; printed yet?
                JMP     BPCWB
                JMP     BPCWD
BPCWB           LDW     BKWAI           ; no: stand down until SERV says so,
                STW     *T.STA          ; and look again when it does -- a
                SMB     K.SWTCH         ; wake is advice, not a promise
                JSX     K.SWTCH
                JMP     BPCWT
BPCWD           UNM
                EXIT    T.PUTC

; T.PUTW: print the byte window [ACR, T.PWEND), one character at a time
; through T.PUTC.  Each character blocks until printed, so returning is
; the drain the core's callers count on.
T.PUTW          SUBR
                STW     BPWP
BPWL            LDW     BPWP
                CMW     T.PWEND
                SNE                     ; anything left?
                JMP     BPWDN
                CAX
                CLR                     ; LDB replaces only the low half (2-1)
                LDB     *0
                JSX     T.PUTC
                LDW     BPWP
                ADD     BK1
                STW     BPWP
                JMP     BPWL
BPWDN           EXIT    T.PUTW

; T.CRLF: a carriage return and a line feed.  They are distinct
; characters on this machine and both are wanted.
T.CRLF          SUBR
                LDW     BKCR
                JSX     T.PUTC
                LDW     BKLF
                JSX     T.PUTC
                EXIT    T.CRLF

; ---------------------------------------------------------------- input
; T.GETL: collect a line into W.LBUF, CR-terminated, leaving T.INPP the
; byte address of that CR.  Characters are stored as they come, bit 7
; set and case untouched -- folding is the interpreter's business, where
; letters are recognized.  What a line is belongs here: a carriage
; return or a line feed ends it, a rubout or Ctrl-H backs up over a
; character, and a full buffer rings the bell.
T.GETL          SUBR
                LDW     BKLBA
                STW     T.INPP
BGL             SMB     KCONSQ          ; a character from the console
                LDW     KCONSQ          ; queue, parked in the kernel until
                SMB     K.QGET          ; the teletype has one
                JSX     K.QGET
                STW     BCH2
                CLB     X'8D'           ; carriage return ends the line
                SNE
                JMP     BGLE
                CLB     X'8A'           ; and so does a line feed, so a
                SNE                     ; script piped in with newline
                JMP     BGLE            ; endings reads like a typed Return
                CLB     X'FF'           ; RUBOUT, and Ctrl-H for a modern
                SNE                     ; keyboard: take back one character
                JMP     BGLR
                CLB     X'88'
                SNE
                JMP     BGLR
                LDW     T.INPP          ; an ordinary character: room left?
                CMW     BKLBE
                SLS
                JMP     BGLBEL          ; no: ring the bell instead
                CAX
                LDW     BCH2
                STB     *0
                LDW     T.INPP
                ADD     BK1
                STW     T.INPP
                JMP     BGL
BGLR            LDW     T.INPP          ; anything to take back?
                CMW     BKLBA
                SGR
                JMP     BGL
                SUB     BK1
                STW     T.INPP
                LLB     X'DC'           ; echo a backslash, period style --
                JSX     T.PUTC          ; the paper cannot take ink back
                JMP     BGL
BGLBEL          LLB     X'87'           ; BEL
                JSX     T.PUTC
                JMP     BGL
BGLE            LLB     X'8D'           ; terminate with a CR whichever key
                LDX     T.INPP          ; arrived; T.INPP is left naming it
                STB     *0
                EXIT    T.GETL

; ---------------------------------------------------------------- BYE
; The task is over: the kernel hands the console back to the shell and
; gives back everything this task holds.
B.BYEX          SMB     K.EXIT
                JMP     K.EXIT

; ---------------------------------------------------------------- glue data
; The cells of the core's seam, and this wrapper's own.
T.PWEND         WORD    0               ; T.PUTW's end-of-window argument
T.COL           WORD    0               ; print column, for the comma zones
T.INPP          WORD    0               ; line buffer fill pointer (byte)
K.LBUFA         WORD    W.LBUF*2        ; the line buffer as a byte address

BWBASE          WORD    0               ; the workspace block, and twice it
BWBAS2          WORD    0
BWADD           WORD    0               ; BWFIX's cursor over a table...
BWADV           WORD    0               ; ...and what it adds
BPWP            WORD    0               ; T.PUTW's window cursor
BCH             WORD    0               ; T.PUTC's character
BCH2            WORD    0               ; T.GETL's character
BK1             WORD    1
BK0FF           WORD    X'00FF'
BKCR            WORD    X'008D'
BKLF            WORD    X'008A'
BKWAI           WORD    SWAI
BKWSZ           WORD    W.SIZE          ; the workspace, in words
BKLBA           WORD    W.LBUF*2
BKLBE           WORD    W.LBUF*2+W.LBUFSZ
BKWTAB          WORD    BWWTAB          ; the fix-up tables' addresses
BKBTAB          WORD    BWBTAB
BKBANP          WORD    BKBAND
BKNOCA          WORD    BKNOCD

; The core's address cells that hold word addresses into the workspace,
; and those that hold byte addresses -- the wrapper's own among them.
BWWTAB          WORD    K1.HEAP,K1.HPTOP,K1.VARSW,K1.VARSE,K1.ARRW
                WORD    K2.ESTKW,K2.OSTKW,K2.GSTKW,K2.FSTKW,K2.NBUFW
                WORD    K2.VARSW,K2.ARRW,K2.HPTOP,0
BWBTAB          WORD    K1.LBUFA,K.LBUFA,BKLBA,BKLBE,0

BKBAND          WORD    BKBANT*2,BKBANE*2
BKBANT          TEXT    "TINY BASIC UNDER REX\r\n"
BKBANE          EQU     $
BKNOCD          WORD    BKNOCT*2,BKNOCE*2
BKNOCT          TEXT    "NO CORE LEFT\r\n"
BKNOCE          EQU     $

; The core follows the glue, in the same module.
B.CORE          EQU     $
