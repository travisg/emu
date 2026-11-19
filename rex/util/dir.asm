; vim: ts=8:sw=8:expandtab:
;
; DIR [unit:] -- a platter's directory, a line to a file -- its name, its
; type and how many sectors it has -- and then the sectors that are free.
; The platter is the data unit unless one is named: DIR 0: is the one REX
; runs from, whose programs are what a bare word at the prompt runs.  The
; entries come a call at a time (K.DIRNT), each with the disc claimed only
; for its copy, so that a listing at the teletype's pace holds no other
; task off the disc -- and a file made or deleted meanwhile may show twice
; or not at all.  Ctrl-C stops it, where it has the console.

DIR             JSX     L.ARGS
                LDW     =DNAM
                JSX     L.NAME          ; a name of nothing, but its unit
                LDW     DNAM+4
                STW     DENT
.L              JSX     L.BRK
                JNZ     .DONE
                LDW     =DENT
                CALL    K.DIRNT
                JNZ     .END            ; past the last?
                LDW     DENT+2+DE.FLG
                AND     =X'00FF'
                CMW     =DK.FILE
                JEQ     .F              ; a file, or a hole or one being written
                LDW     DENT+2+DE.CNT
                ADD     DFREE
                STW     DFREE
                JMP     .N
.F              LDW     =DENT+2         ; name...
                STW     DP
                LDW     =3
                JSX     DPRN
                LDW     ='.'
                JSX     L.PUTC
                LDW     =DENT+2+DE.TYP  ; ...type...
                STW     DP
                LDW     =1
                JSX     DPRN
                LDW     =' '
                JSX     L.PUTC
                LDW     DENT+2+DE.CNT   ; ...and sectors
                JSX     L.DEC
                JSX     L.NL
.N              LDW     DENT+1
                ADD     =1
                STW     DENT+1
                JMP     .L
.END            CMW     =E.NOF
                JEQ     .FREE           ; the end of the directory, or why not
                JSX     L.ERR
                JMP     L.EXIT
.FREE           LDW     DFREE
                JSX     L.DEC
                LDW     =DSFR
                JSX     L.MSG
.DONE           JMP     L.EXIT

; ACR words from DP, two characters each, the zeros that pad a name left out.
DPRN            SUBR
                STW     DN
.W              LDX     DP
                LDW     *0
                STW     DW
                SRL     8
                SAZ
                JSX     L.PUTC
                LDW     DW
                AND     =X'00FF'
                SAZ
                JSX     L.PUTC
                LDW     DP
                ADD     =1
                STW     DP
                LDW     DN
                SUB     =1
                STW     DN
                JNZ     .W
                EXIT    DPRN

DNAM            WORD    0,0,0,0,0
DENT            WORD    0,0             ; K.DIRNT's unit and index,
                RES     8               ; and the entry it copies out
DFREE           WORD    0               ; sectors in holes
DP              WORD    0               ; DPRN's cursor, count and word
DN              WORD    0
DW              WORD    0
DSFR            MSG     " SECTORS FREE\r\n"
                END     DIR
