; vim: ts=8:sw=8:expandtab:
;
; FAR -- a module of two segments, each calling into the other.
;
; A test module, for the loader: a SEG card divides it, and each half is
; more than half a page, so the two cannot be given blocks in the same
; one.  The first checks that they were not, then calls the second with a
; number; the second adds a table of the first's to it through the
; table's address, a cell of the first's behind a page selection and
; what a routine of the first's adds, and multiplies by a cell of its
; own.  The first prints what that came to -- FAR OK, in a text that is
; the second's -- or FAR BAD and which check it was.

FWANT           EQU     75              ; (5 + 1+2+3 + 7 + 7) * 3

FAR             LDW     =FSUM           ; where is the other segment?
                SRL     11
                STW     FPAGE
                LDW     =FAR
                SRL     11
                CMW     FPAGE
                JNE     .APART
                LDW     =1              ; in this one's page
                JMP     FBAD
.APART          LDW     =5
                CALL    FSUM
                CMW     =FWANT
                JEQ     .RIGHT
                LDW     =2              ; a reference went astray
                JMP     FBAD
.RIGHT          LDW     =FOK
                JSX     L.MSG
                JMP     L.EXIT
FBAD            STW     FPAGE
                LDW     =FNOT
                JSX     L.MSG
                LDW     FPAGE
                JSX     L.DEC
                JSX     L.NL
                JMP     L.EXIT

FHOME           SUBR                    ; the second segment's way back here
                ADD     FSEVEN
                EXIT    FHOME

FPAGE           WORD    0               ; a page, and then which check failed
FSEVEN          WORD    7
FTABLE          WORD    1,2,3
FOK             WORD    FOKT*2,FOKE*2   ; the second segment's text
FNOT            MSG     "FAR BAD "
                RES     1100            ; more than half a page

                SEG
FSUM            SUBR
                STW     FACC
                LDX     =FTABLE
                LDW     *0
                ADD     *1
                ADD     *2
                ADD     FACC
                SMB     FSEVEN
                ADD     FSEVEN
                CALL    FHOME
                MPY     FTHREE
                EXIT    FSUM

FACC            WORD    0
FTHREE          WORD    3
FOKT            TEXT    "FAR OK\r\n"
FOKE            EQU     $
                RES     1100
                END     FAR
