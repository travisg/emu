; vim: ts=8:sw=8:expandtab:
;
; NAP -- a task that never ends, for the shell to end.
;
; A test module.  It first reads a character from the console, and says
; so when it has one -- which behind the prompt, without the console, it
; must not have until it is given it, since a second reader there would
; take the shell's place.  Then it sleeps a second at a time for ever and
; ignores Ctrl-C: KILL, or a second Ctrl-C, is the only way out of it.

NAP             SMB     KCONSQ
                LDW     KCONSQ
                CALL    K.QGET
                LDW     =NREAD
                JSX     L.MSG
.LOOP           LDW     =50
                JSX     L.SLEEP
                JMP     .LOOP

NREAD           MSG     "NAP: READ THE CONSOLE\r\n"
                END     NAP
