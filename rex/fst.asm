; vim: ts=8:sw=8:expandtab:
;
; FST -- the file calls, from a task.
;
; A test module: it makes, reads back, replaces, renames and deletes
; files through the vector, checking every answer, and prints FST OK or
; FST BAD and the number of the step that went wrong.  What it leaves on
; the platter is what run_rex_test.py then reads from the host's side:
;
;   T2.TX   ten bytes, having replaced one of ninety-four
;   T4.TX   no bytes at all
;   T5.TX   being written and never closed, which is nobody's file
;
; and T1.TX and T3.TX gone.  Byte i of anything written here is i*5+1,
; cut to eight bits.  Run it twice and the second run starts by making a
; hole of the T5.TX the first one abandoned.

FST             LDW     FKS1            ; 1: make T1.TX, 300 bytes
                STW     FSTEP
                LDW     FKO1W
                JSX     FOPEN
                LDW     FK300
                JSX     FWRITE
                JSX     FCLOSE

                LDW     FKS2            ; 2: four sectors, eighteen bytes
                STW     FSTEP           ; in the last
                LDW     FKL1
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL1+DE.CNT
                CMW     FK4
                SEQ
                JMP     FFAIL
                LDW     FL1+DE.LST
                CMW     FK18
                SEQ
                JMP     FFAIL

                LDW     FKS3            ; 3: read it back, to its end and
                STW     FSTEP           ; no further
                LDW     FKO1R
                JSX     FOPEN
                LDW     FK300
                JSX     FREAD
                LDW     FCHN
                SMB     K.GETC
                JSX     K.GETC
                CMW     FKENOF
                SEQ
                JMP     FFAIL
                JSX     FCLOSE

                LDW     FKS4            ; 4: T2.TX, one sector exactly
                STW     FSTEP
                LDW     FKO2W
                JSX     FOPEN
                LDW     FK94
                JSX     FWRITE
                JSX     FCLOSE
                LDW     FKL2
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL2+DE.CNT
                CMW     FK1
                SEQ
                JMP     FFAIL
                LDW     FL2+DE.LST
                CMW     FK94
                SEQ
                JMP     FFAIL

                LDW     FKS5            ; 5: T1.TX becomes T3.TX
                STW     FSTEP
                LDW     FKR13
                SMB     K.RENAM
                JSX     K.RENAM
                SAZ
                JMP     FFAIL
                LDW     FKL1
                JSX     FLOOK
                CMW     FKENOF
                SEQ
                JMP     FFAIL
                LDW     FKL3
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL3+DE.CNT
                CMW     FK4
                SEQ
                JMP     FFAIL

                LDW     FKS6            ; 6: a new T2.TX takes the old one's
                STW     FSTEP           ; place when it is closed
                LDW     FKO2W
                JSX     FOPEN
                LDW     FK10
                JSX     FWRITE
                JSX     FCLOSE
                LDW     FKL2
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL2+DE.LST
                CMW     FK10
                SEQ
                JMP     FFAIL
                LDW     FKO2R
                JSX     FOPEN
                LDW     FK10
                JSX     FREAD
                JSX     FCLOSE

                LDW     FKS7            ; 7: T3.TX goes
                STW     FSTEP
                LDW     FKL3
                SMB     K.DELET
                JSX     K.DELET
                SAZ
                JMP     FFAIL
                LDW     FKL3
                JSX     FLOOK
                CMW     FKENOF
                SEQ
                JMP     FFAIL

                LDW     FKS8            ; 8: T4.TX, nothing in it
                STW     FSTEP
                LDW     FKO4W
                JSX     FOPEN
                JSX     FCLOSE
                LDW     FKL4
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL4+DE.CNT
                SAZ
                JMP     FFAIL
                LDW     FL4+DE.LST
                SAZ
                JMP     FFAIL

                LDW     FKS9            ; 9: a name in use is not to be
                STW     FSTEP           ; had by renaming
                LDW     FKR24
                SMB     K.RENAM
                JSX     K.RENAM
                CMW     FKEEXS
                SEQ
                JMP     FFAIL

                LDW     FKS10           ; 10: four channels and no fifth
                STW     FSTEP
                LDW     FKO2R
                JSX     FOPEN
                LDW     FKO2R
                JSX     FOPEN
                LDW     FKO2R
                JSX     FOPEN
                LDW     FKO2R
                JSX     FOPEN
                LDW     FKO2R
                SMB     K.OPEN
                JSX     K.OPEN
                CMW     FKECHN
                SEQ
                JMP     FFAIL
                LDW     FK4
                STW     FCHN
FCL4            JSX     FCLOSE
                LDW     FCHN
                SUB     FK1
                STW     FCHN
                SAZ
                JMP     FCL4

                LDW     FKS11           ; 11: T5.TX, begun and abandoned
                STW     FSTEP
                LDW     FKO5W
                JSX     FOPEN
                LDW     FK300
                JSX     FWRITE

                LDW     FKMOK
                JSX     L.MSG
                SMB     K.EXIT
                JMP     K.EXIT

FFAIL           LDW     FKMBAD
                JSX     L.MSG
                LDW     FSTEP
                JSX     L.DEC
                JSX     L.NL
                SMB     K.EXIT
                JMP     K.EXIT

; ---------------------------------------------------------------- the steps' parts
; Open the file whose five words ACR addresses, leaving the channel in
; FCHN.
FOPEN           SUBR
                SMB     K.OPEN
                JSX     K.OPEN
                SAP                     ; an error is minus
                JMP     FFAIL
                STW     FCHN
                EXIT    FOPEN

FCLOSE          SUBR
                LDW     FCHN
                SMB     K.CLOSE
                JSX     K.CLOSE
                SAZ
                JMP     FFAIL
                EXIT    FCLOSE

; K.LOOK on the eight words ACR addresses; the answer is the caller's to
; judge.
FLOOK           SUBR
                SMB     K.LOOK
                JSX     K.LOOK
                EXIT    FLOOK

; Write ACR bytes of the pattern to FCHN, and read as many and hold them
; to it.
FWRITE          SUBR
                STW     FN
                CLR
                STW     FI
FWL             JSX     FBYTE
                STW     FB
                LDW     FCHN
                SLL     8
                ORI     FB
                SMB     K.PUTC
                JSX     K.PUTC
                SAZ
                JMP     FFAIL
                JSX     FSTEPI
                SAZ
                JMP     FWL
                EXIT    FWRITE

FREAD           SUBR
                STW     FN
                CLR
                STW     FI
FRL             JSX     FBYTE
                STW     FB
                LDW     FCHN
                SMB     K.GETC
                JSX     K.GETC
                CMW     FB
                SEQ
                JMP     FFAIL
                JSX     FSTEPI
                SAZ
                JMP     FRL
                EXIT    FREAD

FBYTE           SUBR                    ; byte FI of the pattern
                LDW     FI
                SLL     2
                ADD     FI
                ADD     FK1
                AND     FK0FF
                EXIT    FBYTE
FSTEPI          SUBR                    ; on to the next: zero when all are done
                LDW     FI
                ADD     FK1
                STW     FI
                LDW     FN
                SUB     FK1
                STW     FN
                EXIT    FSTEPI

; ---------------------------------------------------------------- data
FSTEP           WORD    0               ; the step in hand
FCHN            WORD    0               ; the channel in hand
FN              WORD    0               ; bytes to go
FI              WORD    0               ; and how far in
FB              WORD    0               ; the byte expected

FO1W            WORD    'T1',0,0,'TX',1 ; names, with the mode K.OPEN wants
FO1R            WORD    'T1',0,0,'TX',0
FO2W            WORD    'T2',0,0,'TX',1
FO2R            WORD    'T2',0,0,'TX',0
FO4W            WORD    'T4',0,0,'TX',1
FO5W            WORD    'T5',0,0,'TX',1
FL1             WORD    'T1',0,0,'TX',0,0,0,0   ; and with room for K.LOOK's answer
FL2             WORD    'T2',0,0,'TX',0,0,0,0
FL3             WORD    'T3',0,0,'TX',0,0,0,0
FL4             WORD    'T4',0,0,'TX',0,0,0,0
FR13            WORD    'T1',0,0,'TX','T3',0,0,'TX'
FR24            WORD    'T2',0,0,'TX','T4',0,0,'TX'

FKO1W           WORD    FO1W
FKO1R           WORD    FO1R
FKO2W           WORD    FO2W
FKO2R           WORD    FO2R
FKO4W           WORD    FO4W
FKO5W           WORD    FO5W
FKL1            WORD    FL1
FKL2            WORD    FL2
FKL3            WORD    FL3
FKL4            WORD    FL4
FKR13           WORD    FR13
FKR24           WORD    FR24
FK1             WORD    1
FK4             WORD    4
FK10            WORD    10
FK18            WORD    18
FK94            WORD    94
FK300           WORD    300
FK0FF           WORD    X'00FF'
FKS1            WORD    1
FKS2            WORD    2
FKS3            WORD    3
FKS4            WORD    4
FKS5            WORD    5
FKS6            WORD    6
FKS7            WORD    7
FKS8            WORD    8
FKS9            WORD    9
FKS10           WORD    10
FKS11           WORD    11
FKENOF          WORD    E.NOF
FKECHN          WORD    E.CHN
FKEEXS          WORD    E.EXST
FKMOK           WORD    FMOK
FKMBAD          WORD    FMBAD
FMOK            WORD    FMOKT*2,FMOKE*2
FMBAD           WORD    FMBADT*2,FMBADE*2
FMOKT           TEXT    "FST OK\r\n"
FMOKE           EQU     $
FMBADT          TEXT    "FST BAD "
FMBADE          EQU     $
                END     FST
