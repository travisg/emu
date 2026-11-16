; vim: ts=8:sw=8:expandtab:
;
; DEL name -- a file gone, and its sectors a hole.

DEL             JSX     L.ARGS
                LDW     =DNAM
                JSX     L.NAME
                JNZ     .GO
                LDW     =DUSE
                JSX     L.MSG
                JMP     L.EXIT
.GO             LDW     =DNAM
                CALL    K.DELET
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

DNAM            WORD    0,0,0,0
DUSE            MSG     "DEL NAME\r\n"
                END     DEL
