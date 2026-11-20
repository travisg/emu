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
                JNZ     .LOOP           ; the console is ours?  then no test
                SMB     KCONSQ
                LDW     KCONSQ
                CALL    K.QGET
                CMW     =X'FFFF'
                JNE     .READ
                LDW     =NREF
                JSX     L.MSG
                JMP     .LOOP
.READ           LDW     =NBAD
                JSX     L.MSG
.LOOP           LDW     =50
                JSX     L.SLEEP
                JMP     .LOOP

NREF            MSG     "NAP: THE CONSOLE WAS REFUSED\r\n"
NBAD            MSG     "NAP: READ THE CONSOLE\r\n"
                END     NAP
