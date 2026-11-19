; vim: ts=8:sw=8:expandtab:
;
; SQUISH [unit:] -- the holes between the files closed up, so that what
; is free is one hole at the end.  The kernel does the moving (K.SQUSH),
; since the directory is its; this is the way to ask.  The platter is the
; data unit unless one is named, as 1: is.

SQUISH          JSX     L.ARGS
                LDW     =SNAM
                JSX     L.NAME          ; a name of nothing, but its unit
                LDW     SNAM+4
                CALL    K.SQUSH
                SAZ
                JSX     L.ERR
                JMP     L.EXIT

SNAM            WORD    0,0,0,0,0
                END     SQUISH
