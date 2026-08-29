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
; The code lives in word page 0, but the banner, the echo buffer and a record
; counter live out at word X'2100' -- past the 4K-word mark -- so the demo
; exercises the addressing a bigger program would.  The byte instructions
; never notice: an indexed byte reference adds the 16-bit index register to
; its base (1-3.3.2), which spans all of core in one instruction, so pointing
; the buffer cells at high byte addresses is the whole change.  The direct
; word references to the counter are the part that has to work at it: a word
; address is only the top four bits of EXR over an 11-bit M field (1-3.3.1),
; so each one selects its page with SMB first and writes the M field as an
; explicit offset from the page base.  EXR reloads from the program counter
; after every memory reference (1-3), so an SMB governs exactly one
; reference; an interrupt cannot split the pair, because the entry sequence
; saves the machine status -- EXR included -- after the SMB has set it, and
; INR restores it (3-3).

; ---------------------------------------------------------------- level 0
                ORG     0
                JMP     START           ; clobbered by the first PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0               ; unused by the interrupt sequence

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Connect the keyboard, point the transmitter at the banner, and start it.
; Function 9 does not echo -- the service routine below does that, so that what
; appears on the terminal is what the guest produced.
;
; MSK is not decoration: SEND is not re-entrant, and without the inhibit the
; completion interrupt from its own DOT would arrive before it had returned.
; Every other call to SEND comes from inside the service routine, where level 0
; is Active and the hardware will not re-enter it.  Nothing is lost by
; inhibiting either -- a masked interrupt is held and taken at the UNM, whereas
; a signal to a level that is merely disabled is dropped, so ENB has to come
; first.
START           MSK
                DOT     14,9
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

; The buffer is empty; mark the transmitter idle.  From here the next interrupt
; is a keystroke.
SDONE           CLR
                STW     OUTP
                LDX     SRET
                JMP     *0

; ------------------------------------------------------- level 0 service
; The teletype has one interrupt line for both directions, so the routine has
; to work out which one this was.  It decides the way the period driver decides:
; by looking at what it started rather than by asking the hardware.  A non-zero
; output pointer means a character is still being printed, so the interrupt is
; that character's completion.
SERV            STW     SAVEA
                STX     SAVEX
                LDW     OUTP
                SAZ                     ; transmitter idle?
                JMP     TXNEXT          ; no: the printer wants the next one

; Nothing is printing, so a keystroke should be waiting.  It need not be: one
; interrupt line means a completion and a keystroke arriving together are a
; single interrupt, and the tail of TXNEXT below collects the character that
; merge would otherwise strand.  So a DIN that comes back empty is not an
; error, it is the other half of that arrangement.
RX              DIN     14,13           ; collect the frame, and ask for another
                SAZ
                JMP     RXHAVE
                JMP     EXIT
RXHAVE          AND     M7BIT           ; the 703 carries the eighth bit set

                CLB     X'2E'           ; '.' -- stop the machine
                SNE
                HLT

                CLB     X'0D'           ; carriage return?
                SNE
                JMP     RXCR
                CLB     X'0A'           ; a line feed counts as one too, so a
                SNE                     ; script piped in with newline endings
                JMP     RXCR            ; reads the same as a typed Return

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

; Count the record, in a word that lives out in page 4.  Each reference picks
; its page with SMB and gives the 11-bit M field as an offset from the page
; base -- the assembler refuses a bare high address, because the machine
; cannot encode one.  The ADD between them needs nothing: EXR has already
; reloaded to this code's own page (1-3), which is where ONE lives.
                SMB     NECHO
                LDW     NECHO-HIBASE
                ADD     ONE
                SMB     NECHO
                STW     NECHO-HIBASE

TXNEXT          JSX     SEND
; If that finished the record it left the output pointer zero.  A character can
; have arrived while the last one was printing; its interrupt merged with the
; completion, so nothing will interrupt again on its behalf and it would sit in
; the teletype forever.  Go and look.  RX cannot loop back here more than once:
; whatever it finds it starts printing, which makes the pointer non-zero again.
                LDW     OUTP
                SAZ
                JMP     EXIT
                JMP     RX

EXIT            LDW     SAVEA
                LDX     SAVEX
                INR     0

; ---------------------------------------------------------------- data
OUTP            WORD    0               ; byte address of the next character to
                                        ; print; zero means the printer is idle
OUTE            WORD    0               ; one past the last
SRET            WORD    0               ; SEND's return link
SAVEA           WORD    0               ; the interrupted program's registers
SAVEX           WORD    0
ONE             WORD    1
M7BIT           WORD    X'007F'
UPMASK          WORD    X'FFDF'

; Byte addresses of the two buffers.  There is no load-immediate on this
; machine wider than LLB's eight bits, so a sixteen bit constant is a cell like
; any other, and the byte instructions reach it through the index register.
; The buffers themselves are out in page 4; a byte address is sixteen bits, so
; these cells carry the full distance and the byte instructions never know.
ECHOA           WORD    ECHOB*2
ECHOA1          WORD    ECHOB*2+1
ECHOA2          WORD    ECHOB*2+2
BANADR          WORD    BANNER*2
BANEND          WORD    BANSTOP*2

; ------------------------------------------------------------ high memory
; Word page 4 (bytes X'4200' up).  The assembler pads the gap with zeros, so
; the image grows to about 8K words of mostly nothing -- which is the point:
; a machine bought with more than 4K of core should have something living in
; it.  Select MA on the panel and the lamps flick up here on every keystroke.
HIBASE          EQU     X'2000'         ; word base of the page pair SMB selects
                ORG     X'2100'
NECHO           WORD    0               ; records echoed since power-on
ECHOB           WORD    0               ; up to two characters of echo
BANNER          TEXT    "RAYTHEON 703 READY\r\n"
BANSTOP         EQU     $
