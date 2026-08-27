; vim: ts=8:sw=8:expandtab:
;
; Raytheon 703 demonstration program.
;
; Prints a banner, then echoes the console teletype under interrupt control,
; folding lower case to upper on the way through. A '.' halts the machine,
; which is how the end-to-end test stops it; otherwise Ctrl-D quits the
; emulator from the terminal side.
;
; Build with asm703.py; see the makefile's ray703-demo target.
;
; Two things about this machine shape the listing:
;
; Words 0-63 are the sixteen four-word interrupt blocks, so a program cannot
; simply start at word 0 -- word 0 is where the hardware saves the program
; counter on a level 0 interrupt.  A jump there is safe only because it has
; already done its job by the time the first interrupt arrives.  Everything
; else starts above the blocks.
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

; Print the banner.  The index counts up from minus the banner length and the
; load reaches back from the end of it, so IXS's skip -- taken as soon as the
; index reaches zero -- is the loop exit.  That is what the instruction is for.
START           LDX     BANLEN
BANLP           LDB     */BANEND
                DOT     14,14           ; teletype, write a character
                IXS     1
                JMP     BANLP

                DOT     14,9            ; arm the keyboard; the device does
                                        ; not echo on this function, the
                                        ; service routine below does
                ENB     0               ; level 0 may now interrupt
IDLE            JMP     IDLE            ; everything from here is interrupts

; ------------------------------------------------------- level 0 service
; Entered by the hardware with the program counter and machine status saved in
; words 0 and 2, and the CPU forced into global mode.
SERV            DIN     14,13           ; collect the frame the teletype read
                AND     M7BIT           ; the 703 carries the eighth bit set

                CLB     X'0D'           ; carriage return?
                SNE
                JMP     DOCR

                CLB     X'2E'           ; '.' -- stop the machine
                SNE
                HLT

                CLB     X'61'           ; below 'a'?
                SLS
                JMP     CHKHI
                JMP     EMIT
CHKHI           CLB     X'7A'           ; above 'z'?
                SGR
                AND     UPMASK          ; in range: clear bit 5, folding to
                                        ; upper case
EMIT            DOT     14,14
                INR     0

; A carriage return from the keyboard needs its line feed supplied.
DOCR            LLB     X'0D'
                DOT     14,14
                LLB     X'0A'
                DOT     14,14
                INR     0

; ---------------------------------------------------------------- data
M7BIT           WORD    X'007F'
UPMASK          WORD    X'FFDF'
BANLEN          WORD    (BANNER-BANEND)*2
BANNER          TEXT    "RAYTHEON 703 READY\r\n"
BANEND          EQU     $
