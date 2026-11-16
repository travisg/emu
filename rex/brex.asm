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
;   - the break flag is the kernel's BRKREQ: SERV sniffs Ctrl-C out of
;     the input stream and raises it.  It is the flag of whoever has the
;     console, so the core asks through T.BRKQ and T.BRKC, which answer
;     for the kernel's cell when this task has the console and say "no
;     break" when it has not.
;   - BYE is K.EXIT: the kernel hands the console back to the shell and
;     gives back the module, its node and the workspace, so the next RUN
;     BASIC starts afresh.
;   - SAVE name and OLD name are this wrapper's, since the files are the
;     executive's: SAVE is LIST with the characters going down a channel
;     instead of to the printer, and OLD empties the program and then
;     reads the READY loop's lines out of the file instead of from the
;     keyboard, in silence, until the file runs out.  A name without a
;     type is NAME.BA.
;   - a name on the command line -- BASIC PROG -- is OLD PROG and then
;     RUN.  Started that way by LOAD, with no console, the program runs
;     behind the prompt: only what it PRINTs reaches the printer, and the
;     task is over when the program ends or asks for input.
;   - a file that is not there, or a disc that says no, is the core's
;     HOW?, there being no room in the page for a message of this file's.
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

; ---------------------------------------------------------------- entry
; The console first, because the console has one reader at a time.  RUN
; grants it and fills in the node's T.CON; LOAD grants nothing.  Without
; it this task can only run a program it was given by name, so with
; neither it says so and goes.  The message goes out before the workspace
; exists, which is the same ground BKNOCA's does.
BASENT          SMB     CURT
                LDX     CURT
                LDW     *T.CON
                SAZ                     ; the console?
                JMP     BECON
                LDW     BK1
                STW     BNOC
                LDW     *T.ARG
                SAZ                     ; without it, a program to run?
                JMP     BECON
                LDW     BKNCON
                JMP     BEMSG

; Take the workspace, point the core's address cells at it -- the word
; cells get the block's address, the byte cells twice that -- print the
; banner for whoever is at the console, start the program named on the
; command line coming in if there is one, and hand over to the core.
BECON           LDW     BKWSZ
                SMB     K.ALLOC
                JSX     K.ALLOC
                SAZ                     ; a block?
                JMP     BWOK
                LDW     BKNOCA          ; none: say so and go
BEMSG           JSX     M.MSG
                JSX     T.CRLF
                JMP     B.BYEX
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
                LDW     BNOC
                SAZ
                JMP     BWARG
                LDW     BKBANP          ; the banner descriptor's address --
                JSX     M.MSG           ; M.MSG wants the pointer, and prints
                                        ; through the core's own window path
BWARG           SMB     CURT
                LDX     CURT
                LDW     *T.ARG
                SAZ                     ; a program named on the line?
                JMP     BEPGM
                JMP     B.COLD
BEPGM           SLL     1               ; the tail, as a byte address
                STW     BNP
                JSX     BNAME
                CLR
                JSX     BOPEN
                SAP
                JMP     BENOF
                STW     BIN             ; the READY loop reads it in,
                STW     BAUTO           ; it runs when it is in,
                LDW     BNOC            ; and behind the prompt only a
                STW     BSIL            ; running program prints
                JMP     B.COLD
BENOF           LDW     K2.HOWD         ; no such program: the core's HOW?
                JMP     BEMSG

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
; Unless the character is not for the printer: SAVE's LIST goes down a
; channel, and nothing is printed while a program is being read in, or
; by a task without the console that is not running a program -- its
; banner and its READY are nobody's to read.
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
BPCGO           LDW     BOUT
                SAZ                     ; SAVE's LIST: down the channel
                JMP     BPCFIL
                LDW     BIN
                SAZ                     ; a program coming in: silence
                JMP     BPCX
                LDW     BSIL
                SAZ                     ; behind the prompt, the printer is
                JMP     BPCBG           ; a running program's alone
BPCPR           MSK
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
BPCX            EXIT    T.PUTC
BPCBG           LDW     S.RUNMOD
                SAZ
                JMP     BPCPR
                JMP     BPCX
BPCFIL          ORI     BCH             ; a character the file has no room
                SMB     K.PUTC          ; for is the channel's to remember:
                JSX     K.PUTC          ; the close answers for it
                JMP     BPCX

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
; byte address of that CR.  The line comes from the file OLD opened while
; there is one; then, if the program was named on the command line, the
; wait for a line is where it is set running; and a task with no console
; that gets as far as wanting the keyboard is over.  Characters are stored as they come, bit 7
; set and case untouched -- folding is the interpreter's business, where
; letters are recognized.  What a line is belongs here: a carriage
; return or a line feed ends it, a rubout or Ctrl-H backs up over a
; character, and a full buffer rings the bell.
T.GETL          SUBR
                LDW     BKLBA
                STW     T.INPP
BGL             LDW     BIN
                SAZ                     ; a program coming in from a file?
                JMP     BGLF
                LDW     BAUTO
                SAZ                     ; in, and to be run?
                JMP     BGLRUN
                LDW     BNOC
                SAZ                     ; no console: nothing more can come
                JMP     B.BYEX
                SMB     KCONSQ          ; a character from the console
                LDW     KCONSQ          ; queue, parked in the kernel until
                SMB     K.QGET          ; the teletype has one
                JSX     K.QGET
                JMP     BGLC
BGLRUN          CLR
                STW     BAUTO
                JMP     S.RUN

; From the file: bit 7 goes on, in case a host made the file without it.
; A carriage return ends a line as it does from the keyboard; a line feed
; ends one that has something in it and is otherwise passed over, so
; either ending, or both, reads as one line.  At the end the channel is
; let go and whatever is in the buffer -- most often nothing -- is the
; last line.
BGLF            SMB     K.GETC
                JSX     K.GETC
                SAP
                JMP     BGLFE
                ORI     BK80
                CLB     X'8A'
                SEQ
                JMP     BGLC
                LDW     T.INPP
                CMW     BKLBA
                SEQ
                JMP     BGLE
                JMP     BGL
BGLFE           LDW     BIN
                SMB     K.CLOSE
                JSX     K.CLOSE
                CLR
                STW     BIN
                JMP     BGLE
BGLC            STW     BCH2
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

; ---------------------------------------------------------------- the break
; The flag is the console's, so it is this task's only while the console
; is: T.BRKQ answers whether a break is waiting, T.BRKC spends it.
T.BRKQ          SUBR
                LDW     BNOC
                SAZ
                JMP     BBQZ
                SMB     BRKREQ
                LDW     BRKREQ
BBQX            EXIT    T.BRKQ
BBQZ            CLR
                JMP     BBQX
T.BRKC          SUBR
                LDW     BNOC
                SAZ                     ; ACR is zero where it is wanted
                JMP     BBCX
                SMB     BRKREQ
                STW     BRKREQ
BBCX            EXIT    T.BRKC

; ---------------------------------------------------------------- files
; SAVE name: the listing, down a channel.  LIST comes back through
; B.LISTX when it has listed everything, which is where the file is
; closed -- and where an ordinary LIST simply goes on.
B.SAVEX         LDW     L.TXTPTR
                STW     BNP
                JSX     BNAME
                LDW     BK1
                JSX     BOPEN
                SAP
                JMP     BFERR
                SLL     8
                STW     BOUT
                JMP     S.LIST
B.LISTX         LDW     BOUT
                SAZ
                JMP     BLX1
                JMP     B.NEXT
BLX1            SRL     8
                SMB     K.CLOSE
                JSX     K.CLOSE
                SAZ
                JMP     BFERR
                STW     BOUT
                JMP     B.NEXT

; OLD name: the program emptied, and the READY loop's lines out of the
; file until it ends.
B.OLDX          LDW     L.TXTPTR
                STW     BNP
                JSX     BNAME
                CLR
                JSX     BOPEN
                SAP
                JMP     BFERR
                STW     BIN
                JMP     S.NEW

; A file call said no: SAVE's channel forgotten, and the core's HOW?.
BFERR           CLR
                STW     BOUT
                JMP     E.HOW

; Open the file BFN names: ACR zero to read it, one to write it.
BOPEN           SUBR
                STW     BFN+4
                LDW     BKFN
                SMB     K.OPEN
                JSX     K.OPEN
                EXIT    BOPEN

; The file name at byte BNP into BFN, as the directory holds one: blanks
; passed over, up to six characters, then two of type after a dot, BA
; without one.  A zero ends the name, and so does a blank or any
; character below one.  Lower case goes in as upper: the core keeps a
; line as it was typed, and the directory's names are the shell's, which
; folds what it reads.
BNAME           SUBR
                CLR
                STW     BFN
                STW     BFN+1
                STW     BFN+2
                LDW     BKBA
                STW     BFN+3
                LDW     BKFNB
                STW     BNQ
                LDW     BK6
                STW     BNN
BNSK            LDX     BNP
                CLR
                LDB     *0
                CLB     X'A0'
                SEQ                     ; a blank in front?
                JMP     BNL1
                LDW     BNP
                ADD     BK1
                STW     BNP
                JMP     BNSK
BNL             LDX     BNP
                CLR
                LDB     *0
BNL1            SAZ                     ; a command tail ends on a zero,
                JMP     BNL2            ; which is above every character as
BNX             EXIT    BNAME           ; CLB counts -- it compares signed
BNL2            CLB     X'A1'
                SLS                     ; and a blank or a CR ends it too
                JMP     BN1
                JMP     BNX
BN1             CLB     X'AE'           ; a dot starts the type
                SNE
                JMP     BNDOT
                CLB     X'E1'           ; from 'a' up: bit 5 off
                SLS
                AND     K1.DF
                STW     BCH2
                LDW     BNN
                SAZ                     ; room for it?
                JMP     BN2
                JMP     BN3
BN2             SUB     BK1
                STW     BNN
                LDX     BNQ
                LDW     BCH2
                STB     *0
                LDW     BNQ
                ADD     BK1
                STW     BNQ
BN3             LDW     BNP
                ADD     BK1
                STW     BNP
                JMP     BNL
BNDOT           CLR
                STW     BFN+3
                LDW     BKFTB
                STW     BNQ
                LDW     BK2
                STW     BNN
                JMP     BN3
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
BIN             WORD    0               ; the channel a program is coming in by
BOUT            WORD    0               ; the one SAVE lists down, times 256
BAUTO           WORD    0               ; RUN, once the program is in
BNOC            WORD    0               ; this task has no console
BSIL            WORD    0               ; ...and so only a running program prints
BFN             WORD    0,0,0,0,0       ; a file's name, type and mode
BNP             WORD    0               ; BNAME's source, a byte address,
BNQ             WORD    0               ; where its next character goes,
BNN             WORD    0               ; and how many more it will take
BK1             WORD    1
BK2             WORD    2
BK6             WORD    6
BK80            WORD    X'0080'
BKBA            WORD    'BA'            ; a program's type
BKFN            WORD    BFN
BKFNB           WORD    BFN*2
BKFTB           WORD    BFN*2+6
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
BKNCON          WORD    BKNCOD

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
BKNOCT          TEXT    "NO CORE LEFT"
BKNOCE          EQU     $
BKNCOD          WORD    BKNCOT*2,BKNCOE*2
BKNCOT          TEXT    "NEEDS THE CONSOLE: USE RUN BASIC"
BKNCOE          EQU     $

; The core follows the glue, in the same module.
B.CORE          EQU     $
