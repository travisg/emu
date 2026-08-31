; vim: ts=8:sw=8:expandtab:
;
; Tiny BASIC for the Raytheon 703 -- the standalone machine.
;
; This is the wrapper half of the two-file deck that builds basic.bin:
; bcore.asm, assembled after this file, is the interpreter; this file is
; the machine around it -- the interrupt block, the interrupt-driven
; console teletype on the demo.asm model, the workspace, and the layout
; of the core's sections.  bcore.asm's header lists what the core expects
; a wrapper to provide, and the program-wide ground rules.  The rules
; that are this driver's own:
;
;  - The level 0 service routine leads with SMB, because the interrupt
;    entry sequence saves EXR without reloading it (3-3): its first
;    memory reference resolves in the page of whatever it interrupted.
;    Only the first one needs it, since that reference reloads EXR from
;    the program counter like any other.  (T.SERV's own comment says why
;    the lead is kept with the whole program in one page.)
;  - Keys typed while the previous line is still being processed are
;    dropped (Ctrl-C excepted), as they were on the iron; anything driving
;    this program from a script must pace its typing on the guest's own
;    output, the way run_ray703_basic_test.sh does.  The prompt appearing
;    is not the cue -- the teletype prints at ten characters a second, so
;    the last character of READY lands a tenth of a second before T.GETL
;    opens the line buffer.  The cue is the printer going quiet.
;
; Build with `make -C test ray703-basic`; run with
; `./target/debug/emu -s ray703 -r roms/703/basic.bin`.
;
; Memory map (everything below word X'4000', so byte pointers stay positive
; under this machine's signed-only compares):
;
;   0000-003F  interrupt blocks (level 0 in use)
;   0040-....  the whole program, in word page 0: this driver, then the
;              core straight after it (B.CORE EQU $ at the end of this
;              file is what places it).  One page is what lets the core
;              run with no page selection anywhere -- see its ground
;              rules -- and the assembler's page check fails the build
;              if the deck ever outgrows it.
;   2000-3FFF  workspace, all of it EQU-defined so the image stays small

REXGLUE         EQU     0               ; BYE halts the machine

; ---------------------------------------------------------------- level 0
                ORG     0
                JMP     START           ; clobbered by the first PCR save
                WORD    T.SERV          ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0               ; unused by the interrupt sequence

; ------------------------------------------------------------- workspace
W.LBUF          EQU     X'2000'         ; input line buffer, 40 words
W.LBUFSZ        EQU     79              ; typed bytes; byte 80 holds the CR
W.VARS          EQU     X'2040'         ; A-Z, 26 words
W.ESTK          EQU     X'2060'         ; expression operand stack, 16 words
W.OSTK          EQU     X'2070'         ; expression operator stack, 16 words
W.GSTK          EQU     X'2080'         ; GOSUB stack, 8 one-word frames
W.FSTK          EQU     X'2090'         ; FOR stack, 8 four-word frames
W.NBUF          EQU     X'20B0'         ; number-print digit scratch, 5 words
W.HEAP          EQU     X'2100'         ; program line heap...
W.HEAPTOP       EQU     X'3000'         ; ...up to here
W.ARRAY         EQU     X'3000'         ; @(0..1023)

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Connect the keyboard (function 9: no device echo, the service routine
; echoes so the terminal shows what the guest produced), set global
; addressing for the life of the program, clear the driver's state, and
; print the banner.  The MSK around the first SEND is load-bearing: SEND is
; not re-entrant and its own completion interrupt would arrive before it
; returned (demo.asm says this at length).  ENB must precede UNM: a masked
; interrupt is held where a disabled level's signal is dropped.  T.PUTW
; does both, and -- the reason it is used here rather than a bare SEND --
; waits for the banner to drain: the printer takes a second and a half over
; seventeen characters at ten a second, and B.COLD's first PRINT would
; otherwise reclaim the output window after the first one and the machine
; would come up saying "R".  (It did, until the teletype was paced.)
START           MSK
                DOT     14,9
                SGM
                CLR
                STW     T.OUTP
                STW     T.BRK
                STW     T.COL
                LDW     K.ONE           ; a line is "already waiting", so a
                STW     T.LNRDY         ; key struck during the banner is
                                        ; dropped by the service routine
                                        ; instead of being stored through the
                                        ; T.INPP that only T.GETL primes --
                                        ; which is byte address zero here, the
                                        ; level 0 program counter save.  The
                                        ; first T.GETL primes the pointer and
                                        ; clears this, in that order, and the
                                        ; gate is shut again for every line
                                        ; after it.
                ENB     0
                LDW     K.BANE
                STW     T.PWEND
                LDW     K.BANA
                JSX     T.PUTW
                JMP     B.COLD

; ---------------------------------------------------------------- output
; Hand the next character to the printer, or wind the transmission up if
; there is none left.  Taken from demo.asm, with one addition: the column
; counter every character passes, which is what PRINT's comma zones read.
; The advance happens before the DOT because the completion can arrive on
; the very next instruction.
T.SEND          STX     T.SRET
                LDW     T.OUTP
                CMW     T.OUTE          ; anything left to print?
                SNE
                JMP     T.SDONE
                CAX                     ; index <- the byte to send
                ADD     K.ONE
                STW     T.OUTP
                LDB     *0
                DOT     14,14           ; teletype, write a character
                CLB     X'8D'           ; the char is still in ACR 8-15:
                SNE                     ; carriage return restarts the
                JMP     T.SNCR          ; column count,
                CLB     X'8A'           ; a line feed leaves it alone,
                SEQ
                JMP     T.SNINC         ; anything else advances it
                JMP     T.SNDN
T.SNCR          CLR
                STW     T.COL
                JMP     T.SNDN
T.SNINC         LDW     T.COL
                ADD     K.ONE
                STW     T.COL
T.SNDN          LDX     T.SRET
                JMP     *0
T.SDONE         CLR                     ; buffer empty; transmitter idle
                STW     T.OUTP
                LDX     T.SRET
                JMP     *0

; T.PUTW: print the byte window [ACR, T.PWEND) and wait for it to finish.
; The wait is the period shape -- X-RAY starts an I/O and spins on its
; status word -- and it is what makes PRINT simple: each piece of a line is
; handed over whole and the completion interrupts drain it.
;
; It waits for the printer to fall idle before seizing the window, because
; the service routine echoes through the same two cells and its echo takes
; a tenth of a second to print like anything else.  The commonest case is
; the one that ends every line: T.RXEOL starts the carriage return and line
; feed echoing and sets T.LNRDY in the same breath, so the mainline is let
; out of T.GETL with two characters still to go.  Seizing the window under
; them truncates the echo and hands the printer a second character while
; it is still working on the first -- what a real teletype's one-word
; buffer made of that is anyone's guess, and this program does not find
; out.  The test-and-seize is under MSK because the echo it is testing
; for is started by an interrupt.
T.PUTW          SUBR
                STW     T.PWSAV         ; ACR is the window's start address
T.PWIDL         MSK                     ; no interrupt between the test and
                LDW     T.OUTP          ; the seize, nor between the pointer
                SAZ                     ; stores and SEND's own DOT
                JMP     T.PWBSY
                LDW     T.PWSAV
                STW     T.OUTP
                LDW     T.PWEND
                STW     T.OUTE
                JSX     T.SEND
                UNM
T.PWSP          LDW     T.OUTP          ; spin until the window drains;
                SAZ                     ; level 0 is live, so this is the
                JMP     T.PWSP          ; machine's normal I/O wait
                EXIT    T.PUTW
T.PWBSY         UNM                     ; still printing: let the completion
                JMP     T.PWIDL         ; in and look again

; T.PUTC: print the single character in ACR bits 8-15.
T.PUTC          SUBR
                LDX     K.CBA
                STB     *0
                LDW     K.CBA1
                STW     T.PWEND
                LDW     K.CBA
                JSX     T.PUTW
                EXIT    T.PUTC

; T.CRLF: print a carriage return and a line feed.  They are distinct
; characters on this machine and both are wanted.
T.CRLF          SUBR
                LDW     K.CRLFE
                STW     T.PWEND
                LDW     K.CRLFA
                JSX     T.PUTW
                EXIT    T.CRLF

; T.GETL: hand the line buffer to the service routine and wait for a
; complete line.  On return the buffer holds the typed characters with a
; CR terminator, and T.INPP is the byte address of that CR.
T.GETL          SUBR
                LDW     K.LBUFA
                STW     T.INPP
                CLR
                STW     T.LNRDY
T.GETW          LDW     T.LNRDY
                SAZ
                JMP     T.GETD
                JMP     T.GETW
T.GETD          EXIT    T.GETL

; ------------------------------------------------------- level 0 service
; One interrupt line serves both directions, so the routine decides the way
; the period driver decides: a non-zero output pointer means a character is
; still printing and this is its completion.  Only the hardware's PC and
; status are saved automatically; ACR and IXR are this routine's problem.
;
; The SMB in front of the first store is the service-routine lead every
; interruptible layout needs: the interrupt entry sequence saves EXR but
; does not reload it (3-3), so the routine's first memory reference
; resolves in the page of whatever it interrupted.  With the whole program
; in one page the interrupted page is always this one, but the lead is
; what keeps the routine correct wherever a layout puts the code, and it
; is the shape brex.asm's executive holds its service routines to.  Only
; the first reference needs it: the store reloads EXR from the program
; counter the way every memory reference does.
T.SERV          SMB     T.SAVEA
                STW     T.SAVEA
                STX     T.SAVEX
                LDW     T.OUTP
                SAZ                     ; transmitter idle?
                JMP     T.TXNEXT        ; no: the printer wants the next one

; A DIN that comes back empty is not an error: a completion and a keystroke
; arriving together merge into one interrupt, and the tail of TXNEXT
; collects the character that merge would otherwise strand.
T.RX            DIN     14,13           ; collect the frame, ask for another
                SAZ
                JMP     T.RXHAVE
                JMP     T.EXIT
T.RXHAVE        STW     T.CHAR

                CLB     X'83'           ; Ctrl-C: flag the break and drop it
                SNE
                JMP     T.RXBRK

                LDW     T.LNRDY         ; previous line not yet consumed?
                SAZ                     ; then this key has nowhere to go;
                JMP     T.EXIT          ; drop it
                LDW     T.CHAR

                CLB     X'8D'           ; carriage return ends the line,
                SNE
                JMP     T.RXEOL
                CLB     X'8A'           ; and a line feed counts as one so a
                SNE                     ; script piped in with newline
                JMP     T.RXEOL         ; endings reads the same as Return

                CLB     X'FF'           ; RUBOUT, and Ctrl-H for a modern
                SNE                     ; keyboard: take back one character
                JMP     T.RXRUB
                CLB     X'88'
                SNE
                JMP     T.RXRUB

                LDW     T.INPP          ; an ordinary character: room left?
                CMW     K.LBUFE
                SLS
                JMP     T.RXBEL         ; no: ring the bell instead
                LDW     T.CHAR
                LDX     T.INPP
                STB     *0
                LDW     T.INPP
                ADD     K.ONE
                STW     T.INPP
                LDW     T.CHAR          ; echo what was stored

; Echo one character: put it in the echo cell and start it printing.  The
; transmitter is idle here (this is the keystroke path), so the window can
; be set directly.
T.ECHO1         LDX     K.EBA
                STB     *0
                LDW     K.EBA
                STW     T.OUTP
                ADD     K.ONE
                STW     T.OUTE
                JMP     T.TXNEXT

; End of line: terminate the buffer with a CR whichever key arrived, flag
; the mainline, echo the pair.
T.RXEOL         LLB     X'8D'
                LDX     T.INPP
                STB     *0
                LDW     K.ONE
                STW     T.LNRDY
                LDW     K.CRLFE
                STW     T.OUTE
                LDW     K.CRLFA
                STW     T.OUTP
                JMP     T.TXNEXT

T.RXRUB         LDW     T.INPP          ; anything to take back?
                CMW     K.LBUFA
                SGR
                JMP     T.EXIT
                SUB     K.ONE
                STW     T.INPP
                LLB     X'DC'           ; echo a backslash, period style
                JMP     T.ECHO1

T.RXBEL         LLB     X'87'           ; BEL
                JMP     T.ECHO1

T.RXBRK         LDW     K.ONE
                STW     T.BRK
                JMP     T.EXIT

; Start the next character, then look again: if that emptied the window, a
; keystroke whose interrupt merged with the last completion may be waiting,
; and nothing will interrupt again on its behalf.  RX cannot loop back here
; more than once -- whatever it finds it starts printing, which makes the
; pointer non-zero again.  Taken from demo.asm unchanged.
T.TXNEXT        JSX     T.SEND
                LDW     T.OUTP
                SAZ
                JMP     T.EXIT
                JMP     T.RX

T.EXIT          LDW     T.SAVEA
                LDX     T.SAVEX
                INR     0

; --------------------------------------------------- driver data and cells
T.OUTP          WORD    0               ; byte address of the next character
                                        ; to print; zero = transmitter idle
T.OUTE          WORD    0               ; one past the last
T.SRET          WORD    0               ; SEND's return link
T.SAVEA         WORD    0               ; the interrupted program's registers
T.SAVEX         WORD    0
T.CHAR          WORD    0               ; the keystroke being processed
T.INPP          WORD    0               ; line buffer fill pointer (byte)
T.LNRDY         WORD    0               ; a complete line is waiting
T.BRK           WORD    0               ; Ctrl-C was seen
T.COL           WORD    0               ; print column, for the comma zones
T.PWEND         WORD    0               ; T.PUTW's end-of-window argument
T.PWSAV         WORD    0               ; ...and its start, held across the
                                        ; wait for the printer to fall idle
T.EB            WORD    0               ; the echo character lives here
T.CB            WORD    0               ; T.PUTC's character lives here

; Constants.  There is no immediate wider than LLB's eight bits, so every
; sixteen-bit constant is a cell.  (K.ONE is the core's.)
K.CRLF          WORD    X'8D8A'         ; CR, LF as a byte pair
K.CRLFA         WORD    K.CRLF*2
K.CRLFE         WORD    K.CRLF*2+2
K.EBA           WORD    T.EB*2          ; byte addresses of the two
K.CBA           WORD    T.CB*2          ; one-character buffers
K.CBA1          WORD    T.CB*2+1
K.LBUFA         WORD    W.LBUF*2        ; the line buffer as byte addresses
K.LBUFE         WORD    W.LBUF*2+W.LBUFSZ
K.BANA          WORD    P0BAN*2         ; the banner window
K.BANE          WORD    P0BANEND*2

P0BAN           TEXT    "RAY703 TINY BASIC \r\n"
P0BANEND        EQU     $

; The core follows the driver, in the same page.
B.CORE          EQU     $
