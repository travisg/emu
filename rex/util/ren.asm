; vim: ts=8:sw=8:expandtab:
;
; REN old new -- a file under another name.

REN             JSX     L.ARGS
                LDW     =ROLD
                JSX     L.NAME
                JNZ     .NEW
                JMP     .USAGE
.NEW             LDW     =RNEW
                JSX     L.NAME
                JNZ     .GO
.USAGE          LDW     =RUSE
                JSX     L.MSG
                JMP     L.EXIT
.GO             LDW     =ROLD           ; the two names lie together, which
                CALL    K.RENAM         ; is how K.RENAM wants them
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

ROLD            WORD    0,0,0,0
RNEW            WORD    0,0,0,0
RUSE            MSG     "REN OLD NEW\r\n"
                END     REN
