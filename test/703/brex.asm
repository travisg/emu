; vim: ts=8:sw=8:expandtab:
;
; Tiny BASIC under REX -- the wrapper that makes the interpreter a task.
;
; This file sits between rex.asm and bcore.asm in the deck that builds
; rex.bin.  bcore.asm's header lists what a wrapper owes the core; this
; one pays those debts with the executive's services instead of a driver
; of its own:
;
;   - output deposits one character at a time in BASIC's own mailbox and
;     stands down until SERV reports it printed -- the same dance as the
;     shell's SHPUTC, against this task's node.  The old driver's column
;     count for PRINT's comma zones lives in T.PUTC here, since there is
;     no service routine of ours to keep it.
;   - input takes characters from the console queue with Q.GET, which
;     parks this task in the kernel until the teletype has one.  The
;     Model 33 is armed for hardware echo, so nothing here echoes an
;     ordinary character; the rubout's backslash still prints, because
;     only the buffer can take a character back, not the paper.
;   - T.BRK is the kernel's BRKREQ: SERV sniffs Ctrl-C out of the input
;     stream and raises it, and the core's break check between statements
;     reads and clears it exactly as it did the driver's flag.  That cell
;     is in another page, which is why the core keeps an SMB in front of
;     its two references.
;   - BYE is B.BYEX: hand the console back to the shell, park this task
;     SOFF, and resume at READY -- with the heap intact -- when the
;     shell's BASIC command next grants the console.
;
; The glue and the core sit together in one 2048-word page, so the core
; runs with no page selection anywhere; the SMB pairs below are this
; wrapper's own, reaching the kernel's cells and services in page 0.
;
; The workspace: everything byte-addressed -- the line buffer and the
; heap -- sits below word X'4000' so byte pointers stay positive under
; this machine's signed-only compares.  The word-addressed workspace has
; no such ceiling and sits above it.
REXGLUE         EQU     1               ; BYE hands the console back

W.LBUF          EQU     X'1800'         ; input line buffer, 41 words
W.LBUFSZ        EQU     79              ; typed bytes; byte 80 holds the CR
W.HEAP          EQU     X'1830'         ; program line heap...
W.HEAPTOP       EQU     X'4000'         ; ...up to here -- 10,192 words
W.ARRAY         EQU     X'4000'         ; @(0..1023)
W.VARS          EQU     X'4400'         ; A-Z, 26 words
W.ESTK          EQU     X'4420'         ; expression operand stack, 16 words
W.OSTK          EQU     X'4430'         ; expression operator stack, 16 words
W.GSTK          EQU     X'4440'         ; GOSUB stack, 8 one-word frames
W.FSTK          EQU     X'4450'         ; FOR stack, 8 four-word frames
W.NBUF          EQU     X'4470'         ; number-print digit scratch, 5 words

T.BRK           EQU     BRKREQ          ; the break flag is the kernel's cell

; ---------------------------------------------------------------- entry
; BASENT must sit on a 1024-word byte page boundary: BATCB's status word
; is (BASENT*2)+X'80', an identity that holds only there.  It runs once,
; on the first grant of the console; every later grant resumes wherever
; BYE parked the task.
                ORG     X'1000'

BASENT          LDW     BKBANP          ; the banner descriptor's address --
                JSX     M.MSG           ; M.MSG wants the pointer, and prints
                JMP     B.COLD          ; through the core's own window path

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
                SMB     BA.MBX
                STW     BA.MBX
                SMB     KICK
                JSX     KICK
                UNM
BPCWT           MSK
                SMB     BA.MBX
                LDW     BA.MBX
                SAZ                     ; printed yet?
                JMP     BPCWB
                JMP     BPCWD
BPCWB           LDW     BKWAI           ; no: stand down until SERV says so,
                SMB     BA.STA          ; and look again when it does -- a
                STW     BA.STA          ; wake is advice, not a promise
                SMB     SWTCH
                JSX     SWTCH
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
BGL             LDW     BKCQ            ; a character from the console
                SMB     Q.GET           ; queue, parked in the kernel until
                JSX     Q.GET           ; the teletype has one
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
; Hand the console back and stand down.  The whole window is masked into
; SWTCH, so it is one act.  CONBSY falls first: a wake the shell was not
; yet waiting for is then recovered by the shell's own masked re-check.
; The wake carries SERV's guard -- only a shell in SWAI is touched --
; because a wake is advice, not a promise.  The park is SOFF, and the
; next BASIC command's grant resumes this task right here, whereupon it
; goes back to READY with the heap intact.
B.BYEX          MSK
                CLR
                SMB     CONBSY
                STW     CONBSY
                SMB     SH.STA
                LDW     SH.STA
                CMW     BKWAI
                SEQ                     ; waiting on the hand-back?
                JMP     BBY2
                CLR
                SMB     SH.STA
                STW     SH.STA
BBY2            LDW     BKOFF
                SMB     BA.STA
                STW     BA.STA
                SMB     SWTCH
                JSX     SWTCH           ; gone until the next BASIC command
                JMP     B.RLOOP

; ---------------------------------------------------------------- glue data
; The cells of the core's seam, and this wrapper's own.
T.PWEND         WORD    0               ; T.PUTW's end-of-window argument
T.COL           WORD    0               ; print column, for the comma zones
T.INPP          WORD    0               ; line buffer fill pointer (byte)
K.LBUFA         WORD    W.LBUF*2        ; the line buffer as a byte address

BPWP            WORD    0               ; T.PUTW's window cursor
BCH             WORD    0               ; T.PUTC's character
BCH2            WORD    0               ; T.GETL's character
BK1             WORD    1
BK0FF           WORD    X'00FF'
BKCR            WORD    X'008D'
BKLF            WORD    X'008A'
BKWAI           WORD    SWAI
BKOFF           WORD    SOFF
BKCQ            WORD    QCONS           ; the queue the keyboard fills
BKLBA           WORD    W.LBUF*2
BKLBE           WORD    W.LBUF*2+W.LBUFSZ
BKBANP          WORD    BKBAND
BKBAND          WORD    BKBANT*2,BKBANE*2
BKBANT          TEXT    "TINY BASIC UNDER REX\r\n"
BKBANE          EQU     $

; The core follows the glue, in the same page.
B.CORE          EQU     $

; A tripwire: the deck outgrowing the page lands on this word and the
; assembler's "assembled twice" error names it.
                ORG     X'17FF'
                WORD    0
