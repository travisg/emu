; vim: ts=8:sw=8:expandtab:
;
; Raytheon 703 demonstration program.
;
; Prints a banner, then echoes the console teletype, folding lower case to
; upper on the way through.  A '.' halts the machine, which is how the
; end-to-end test stops it; otherwise Ctrl-D quits the emulator from the
; terminal side.
;
; Build with asm703.py; see the makefile's ray703-demo target.
;
; Both directions run under interrupt control, which is the part worth reading.
; The teletype interrupts once per character received and once per character
; printed, and the printer's completion interrupt is the only thing that moves
; an output loop along: the start-up code below hands the first character over
; and everything after it happens inside the service routine.  That is exactly
; the shape of the period driver -- X-RAY's setup routine outputs one character
; and returns to "WAIT FOR IRS", and its jump table for the output side hangs
; off the same interrupt entry as the input side.
;
; Three things about this machine shape the listing:
;
; Words 0-63 are the sixteen four-word interrupt blocks, so a program cannot
; simply start at word 0 -- word 0 is where the hardware saves the program
; counter on a level 0 interrupt.  A jump there is safe only because it has
; already done its job by the time the first interrupt arrives.  Everything
; else starts above the blocks.
;
; The hardware saves the program counter and the machine status and nothing
; else, so the service routine saves the accumulator and the index register
; itself, the way X-RAY's own level 0 stub does.
;
; The whole program lives in word page 0, so the extension register is always
; zero and local and global addressing are the same thing here.  A program
; that crossed a 2048-word page would need SML/SMU, and would care.

; ---------------------------------------------------------------- level 0
                ORG     0
                JMP     START           ; clobbered by the first PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0               ; unused by the interrupt sequence

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Point the transmitter at the banner and start it.  MSK is not decoration:
; SEND is not re-entrant, and without the inhibit the completion interrupt from
; its own DOT would arrive before it had returned.  Every other call to SEND
; comes from inside the service routine, where level 0 is Active and the
; hardware will not re-enter it.  Nothing is lost by inhibiting either -- a
; masked interrupt is held and taken at the UNM, whereas a signal to a level
; that is merely disabled is dropped, so ENB has to come first.
START           MSK
                LDW     BANADR
                STW     OUTP
                LDW     BANEND
                STW     OUTE
                ENB     0               ; level 0 may now interrupt
                JSX     SEND
                UNM
IDLE            JMP     IDLE            ; everything from here is interrupts

; ---------------------------------------------------------------- output
; Hand the next character to the printer, or wind the transmission up if there
; is none left.  Called with JSX, which leaves the return address in the index
; register; this machine has no stack, so the link is saved in a cell like
; every 703 subroutine.
SEND            STX     SRET
                LDW     OUTP
                CMW     OUTE            ; anything left to print?
                SNE
                JMP     SDONE
                CAX                     ; index <- the byte to send
                ADD     ONE
                STW     OUTP            ; advance before the DOT: the printer's
                                        ; completion can arrive on the very
                                        ; next instruction
                LDB     *0
                DOT     14,14           ; teletype, write a character
                LDX     SRET
                JMP     *0

; The buffer is empty.  Mark the transmitter idle and connect the keyboard;
; from here the next interrupt is a keystroke.  Function 9 does not echo -- the
; service routine below does that, so that what appears on the terminal is what
; the guest produced.
SDONE           CLR
                STW     OUTP
                DOT     14,9
                LDX     SRET
                JMP     *0

; ------------------------------------------------------- level 0 service
; The teletype has one interrupt line for both directions, so the routine has
; to work out which one this was.  It never has to guess: the keyboard stays
; disconnected for as long as a transmission is in progress, so a non-zero
; output pointer means a printer completion and nothing else.  The period
; driver settles the same question the same way, by looking at what it started
; rather than by asking the hardware.
SERV            STW     SAVEA
                STX     SAVEX
                LDW     OUTP
                SAZ                     ; transmitter idle?
                JMP     TXNEXT          ; no: the printer wants the next one

; A character arrived.  Disconnect the keyboard before collecting it: the
; device holds a captured frame until it is collected and will not take another
; while it is held, so those two instructions together are atomic, and the echo
; below runs with only one possible source of interrupts.
                DOT     14,0            ; function 0 disconnects the device
                DIN     14,13           ; collect the frame the teletype read
                AND     M7BIT           ; the 703 carries the eighth bit set

                CLB     X'2E'           ; '.' -- stop the machine
                SNE
                HLT

                CLB     X'0D'           ; carriage return?
                SNE
                JMP     RXCR

                CLB     X'61'           ; below 'a'?
                SLS
                JMP     RXHI
                JMP     RXPUT
RXHI            CLB     X'7A'           ; above 'z'?
                SGR
                AND     UPMASK          ; in range: clear bit 5, folding to
                                        ; upper case

; Queue the one character and start it printing.
RXPUT           LDX     ECHOA
                STB     *0
                LDW     ECHOA
                STW     OUTP
                ADD     ONE
                STW     OUTE
                JMP     TXNEXT

; A carriage return from the keyboard needs its line feed supplied, so this
; time the echo buffer holds two characters.
RXCR            LDX     ECHOA
                LLB     X'0D'
                STB     *0
                LDX     ECHOA1
                LLB     X'0A'
                STB     *0
                LDW     ECHOA
                STW     OUTP
                LDW     ECHOA2
                STW     OUTE

TXNEXT          JSX     SEND
                LDW     SAVEA
                LDX     SAVEX
                INR     0

; ---------------------------------------------------------------- data
OUTP            WORD    0               ; byte address of the next character to
                                        ; print; zero means the printer is idle
OUTE            WORD    0               ; one past the last
SRET            WORD    0               ; SEND's return link
SAVEA           WORD    0               ; the interrupted program's registers
SAVEX           WORD    0
ECHOB           WORD    0               ; up to two characters of echo
ONE             WORD    1
M7BIT           WORD    X'007F'
UPMASK          WORD    X'FFDF'

; Byte addresses of the two buffers.  There is no load-immediate on this
; machine wider than LLB's eight bits, so a sixteen bit constant is a cell like
; any other, and the byte instructions reach it through the index register.
ECHOA           WORD    ECHOB*2
ECHOA1          WORD    ECHOB*2+1
ECHOA2          WORD    ECHOB*2+2
BANADR          WORD    BANNER*2
BANEND          WORD    BANSTOP*2

BANNER          TEXT    "RAYTHEON 703 READY\r\n"
BANSTOP         EQU     $
