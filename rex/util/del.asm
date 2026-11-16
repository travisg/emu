; vim: ts=8:sw=8:expandtab:
;
; DEL name -- a file gone, and its sectors a hole.

DEL             JSX     L.ARGS
                LDW     =DNAM
                JSX     L.NAME
                SAZ
                JMP     D1
                LDW     =DUSE
                JSX     L.MSG
                JMP     L.EXIT
D1              LDW     =DNAM
                SMB     K.DELET
                JSX     K.DELET
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

DNAM            WORD    0,0,0,0
DUSE            WORD    DUSET*2,DUSEE*2
DUSET           TEXT    "DEL NAME\r\n"
DUSEE           EQU     $
                END     DEL
