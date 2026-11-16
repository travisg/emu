; vim: ts=8:sw=8:expandtab:
;
; NAP -- a task that never ends, for the shell to end.
;
; A test module.  Started by LOAD, with no console, it first asks the
; console's queue for a character -- which the kernel must refuse, since
; a second reader there would take the shell's place -- and says which
; happened.  Then, however it was started, it sleeps a second at a time
; for ever and ignores Ctrl-C: KILL, or a second Ctrl-C, is the only way
; out of it.

NAP             SMB     CURT
                LDX     CURT
                LDW     *T.CON
                SAZ                     ; the console is ours?  then no test
                JMP     NLOOP
                SMB     KCONSQ
                LDW     KCONSQ
                SMB     K.QGET
                JSX     K.QGET
                CMW     =X'FFFF'
                SEQ
                JMP     NREAD
                LDW     =NREF
                JSX     L.MSG
                JMP     NLOOP
NREAD           LDW     =NBAD
                JSX     L.MSG
NLOOP           LDW     =60
                JSX     L.SLEEP
                JMP     NLOOP

NREF            WORD    NREFT*2,NREFE*2
NBAD            WORD    NBADT*2,NBADE*2
NREFT           TEXT    "NAP: THE CONSOLE WAS REFUSED\r\n"
NREFE           EQU     $
NBADT           TEXT    "NAP: READ THE CONSOLE \r\n"
NBADE           EQU     $
                END     NAP
