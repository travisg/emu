; vim: ts=8:sw=8:expandtab:
;
; Raytheon 703 boot sector.
;
; A program that fits where the disc controller's LOAD button puts it: sector
; 0, track 0 of disc 0, read into words 0-46 (706 UM 5-9.10.3, Table 5-30).
; One 47-word sector is the whole budget, which forces PTB's kind of economy;
; this one prints its banner and halts, proving the button worked and the
; interrupt-driven teletype survives being loaded off a platter.
;
; Two consequences of living at word 0:
;
; The level 0 interrupt block is not somewhere the program avoids -- it IS the
; program's first four words, exactly as PTB's are.  Word 0 does double duty:
; it runs once as the entry jump and is then clobbered by the first PCR save.
;
; The service routine can skip the register saves every other guest performs.
; The only code that runs with interrupts unmasked is the drain spin below,
; which reloads the accumulator every pass and never touches the index
; register, so there is nothing live to preserve -- and the two cells and four
; instructions saved are real money in a 47-word sector.

                ORG     0
                JMP     START           ; word 0: clobbered by the PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0

; MSK before the first SEND for the demo's reason: SEND is not re-entrant,
; and its own DOT's completion must be held off until it has returned.
START           MSK
                ENB     0
                LDW     MSGA
                STW     OUTP
                LDW     MSGE
                STW     OUTE
                JSX     SEND
                UNM
WAIT            LDW     OUTP            ; the printer going quiet is the
                SAZ                     ; operator's cue that the boot worked
                JMP     WAIT
                HLT

; The demo's transmitter, verbatim.
SEND            STX     SRET
                LDW     OUTP
                CMW     OUTE
                SNE
                JMP     SDONE
                CAX
                ADD     ONE
                STW     OUTP
                LDB     *0
                DOT     14,14           ; teletype, write a character
                LDX     SRET
                JMP     *0
SDONE           CLR
                STW     OUTP
                LDX     SRET
                JMP     *0

; Level 0 service: only the printer can interrupt -- the keyboard is never
; armed -- and only the WAIT spin can be interrupted, so no saves (above).
SERV            JSX     SEND
                INR     0

OUTP            WORD    0               ; byte address of the next character;
OUTE            WORD    0               ; zero means the printer is idle
SRET            WORD    0
ONE             WORD    1
MSGA            WORD    MSG*2
MSGE            WORD    MSGEND*2
MSG             TEXT    "703 BOOT\r\n"
MSGEND          EQU     $
