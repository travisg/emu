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

FST             LDW     =1              ; 1: make T1.TX, 300 bytes
                STW     FSTEP
                LDW     =FO1W
                JSX     FOPEN
                LDW     =300
                JSX     FWRITE
                JSX     FCLOSE

                LDW     =2              ; 2: four sectors, eighteen bytes
                STW     FSTEP           ; in the last
                LDW     =FL1
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL1+DE.CNT
                CMW     =4
                SEQ
                JMP     FFAIL
                LDW     FL1+DE.LST
                CMW     =18
                SEQ
                JMP     FFAIL

                LDW     =3              ; 3: read it back, to its end and
                STW     FSTEP           ; no further
                LDW     =FO1R
                JSX     FOPEN
                LDW     =300
                JSX     FREAD
                LDW     FCHN
                SMB     K.GETC
                JSX     K.GETC
                CMW     =E.NOF
                SEQ
                JMP     FFAIL
                JSX     FCLOSE

                LDW     =4              ; 4: T2.TX, one sector exactly
                STW     FSTEP
                LDW     =FO2W
                JSX     FOPEN
                LDW     =94
                JSX     FWRITE
                JSX     FCLOSE
                LDW     =FL2
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL2+DE.CNT
                CMW     =1
                SEQ
                JMP     FFAIL
                LDW     FL2+DE.LST
                CMW     =94
                SEQ
                JMP     FFAIL

                LDW     =5              ; 5: T1.TX becomes T3.TX
                STW     FSTEP
                LDW     =FR13
                SMB     K.RENAM
                JSX     K.RENAM
                SAZ
                JMP     FFAIL
                LDW     =FL1
                JSX     FLOOK
                CMW     =E.NOF
                SEQ
                JMP     FFAIL
                LDW     =FL3
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL3+DE.CNT
                CMW     =4
                SEQ
                JMP     FFAIL

                LDW     =6              ; 6: a new T2.TX takes the old one's
                STW     FSTEP           ; place when it is closed
                LDW     =FO2W
                JSX     FOPEN
                LDW     =10
                JSX     FWRITE
                JSX     FCLOSE
                LDW     =FL2
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL2+DE.LST
                CMW     =10
                SEQ
                JMP     FFAIL
                LDW     =FO2R
                JSX     FOPEN
                LDW     =10
                JSX     FREAD
                JSX     FCLOSE

                LDW     =7              ; 7: T3.TX goes
                STW     FSTEP
                LDW     =FL3
                SMB     K.DELET
                JSX     K.DELET
                SAZ
                JMP     FFAIL
                LDW     =FL3
                JSX     FLOOK
                CMW     =E.NOF
                SEQ
                JMP     FFAIL

                LDW     =8              ; 8: T4.TX, nothing in it
                STW     FSTEP
                LDW     =FO4W
                JSX     FOPEN
                JSX     FCLOSE
                LDW     =FL4
                JSX     FLOOK
                SAZ
                JMP     FFAIL
                LDW     FL4+DE.CNT
                SAZ
                JMP     FFAIL
                LDW     FL4+DE.LST
                SAZ
                JMP     FFAIL

                LDW     =9              ; 9: a name in use is not to be
                STW     FSTEP           ; had by renaming
                LDW     =FR24
                SMB     K.RENAM
                JSX     K.RENAM
                CMW     =E.EXST
                SEQ
                JMP     FFAIL

                LDW     =10             ; 10: four channels and no fifth
                STW     FSTEP
                LDW     =FO2R
                JSX     FOPEN
                LDW     =FO2R
                JSX     FOPEN
                LDW     =FO2R
                JSX     FOPEN
                LDW     =FO2R
                JSX     FOPEN
                LDW     =FO2R
                SMB     K.OPEN
                JSX     K.OPEN
                CMW     =E.CHN
                SEQ
                JMP     FFAIL
                LDW     =4
                STW     FCHN
FCL4            JSX     FCLOSE
                LDW     FCHN
                SUB     =1
                STW     FCHN
                SAZ
                JMP     FCL4

                LDW     =11             ; 11: T5.TX, begun and abandoned
                STW     FSTEP
                LDW     =FO5W
                JSX     FOPEN
                LDW     =300
                JSX     FWRITE

                LDW     =FMOK
                JSX     L.MSG
                SMB     K.EXIT
                JMP     K.EXIT

FFAIL           LDW     =FMBAD
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
                ADD     =1
                AND     =X'00FF'
                EXIT    FBYTE
FSTEPI          SUBR                    ; on to the next: zero when all are done
                LDW     FI
                ADD     =1
                STW     FI
                LDW     FN
                SUB     =1
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

FMOK            WORD    FMOKT*2,FMOKE*2
FMBAD           WORD    FMBADT*2,FMBADE*2
FMOKT           TEXT    "FST OK\r\n"
FMOKE           EQU     $
FMBADT          TEXT    "FST BAD "
FMBADE          EQU     $
                END     FST
