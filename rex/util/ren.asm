; vim: ts=8:sw=8:expandtab:
;
; REN old new -- a file under another name.

REN             JSX     L.ARGS
                LDW     RKOLD
                JSX     L.NAME
                SAZ
                JMP     R1
                JMP     RUSAGE
R1              LDW     RKNEW
                JSX     L.NAME
                SAZ
                JMP     R2
RUSAGE          LDW     RKUSE
                JSX     L.MSG
                JMP     L.EXIT
R2              LDW     RKOLD           ; the two names lie together, which
                SMB     K.RENAM         ; is how K.RENAM wants them
                JSX     K.RENAM
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

ROLD            WORD    0,0,0,0
RNEW            WORD    0,0,0,0
RKOLD           WORD    ROLD
RKNEW           WORD    RNEW
RKUSE           WORD    RUSE
RUSE            WORD    RUSET*2,RUSEE*2
RUSET           TEXT    "REN OLD NEW "
RUSEX           TEXT    "\r\n"
RUSEE           EQU     $
                END     REN
