; vim: ts=8:sw=8:expandtab:
;
; SQUISH -- the holes between the files closed up, so that what is free
; is one hole at the end.  The kernel does the moving (K.SQUSH), since
; the directory is its; this is the way to ask.

SQUISH          CALL    K.SQUSH
                SAZ
                JSX     L.ERR
                JMP     L.EXIT
                END     SQUISH
