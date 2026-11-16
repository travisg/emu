; vim: ts=8:sw=8:expandtab:
;
; REN old new -- a file under another name.

REN             JSX     L.ARGS
                LDW     =ROLD
                JSX     L.NAME
                SAZ
                JMP     R1
                JMP     RUSAGE
R1              LDW     =RNEW
                JSX     L.NAME
                SAZ
                JMP     R2
RUSAGE          LDW     =RUSE
                JSX     L.MSG
                JMP     L.EXIT
R2              LDW     =ROLD           ; the two names lie together, which
                SMB     K.RENAM         ; is how K.RENAM wants them
                JSX     K.RENAM
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

ROLD            WORD    0,0,0,0
RNEW            WORD    0,0,0,0
RUSE            WORD    RUSET*2,RUSEE*2
RUSET           TEXT    "REN OLD NEW "
                TEXT    "\r\n"
RUSEE           EQU     $
                END     REN
