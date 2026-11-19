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
                JNZ     FFAIL
                LDW     FL1+DE.CNT
                CMW     =4
                JNE     FFAIL
                LDW     FL1+DE.LST
                CMW     =18
                JNE     FFAIL

                LDW     =3              ; 3: read it back, to its end and
                STW     FSTEP           ; no further
                LDW     =FO1R
                JSX     FOPEN
                LDW     =300
                JSX     FREAD
                LDW     FCHN
                CALL    K.GETC
                CMW     =E.NOF
                JNE     FFAIL
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
                JNZ     FFAIL
                LDW     FL2+DE.CNT
                CMW     =1
                JNE     FFAIL
                LDW     FL2+DE.LST
                CMW     =94
                JNE     FFAIL

                LDW     =5              ; 5: T1.TX becomes T3.TX
                STW     FSTEP
                LDW     =FR13
                CALL    K.RENAM
                JNZ     FFAIL
                LDW     =FL1
                JSX     FLOOK
                CMW     =E.NOF
                JNE     FFAIL
                LDW     =FL3
                JSX     FLOOK
                JNZ     FFAIL
                LDW     FL3+DE.CNT
                CMW     =4
                JNE     FFAIL

                LDW     =6              ; 6: a new T2.TX takes the old one's
                STW     FSTEP           ; place when it is closed
                LDW     =FO2W
                JSX     FOPEN
                LDW     =10
                JSX     FWRITE
                JSX     FCLOSE
                LDW     =FL2
                JSX     FLOOK
                JNZ     FFAIL
                LDW     FL2+DE.LST
                CMW     =10
                JNE     FFAIL
                LDW     =FO2R
                JSX     FOPEN
                LDW     =10
                JSX     FREAD
                JSX     FCLOSE

                LDW     =7              ; 7: T3.TX goes
                STW     FSTEP
                LDX     =FL3            ; unit 0 again, over the look's answer
                CLR
                STW     *4
                LDW     =FL3
                CALL    K.DELET
                JNZ     FFAIL
                LDW     =FL3
                JSX     FLOOK
                CMW     =E.NOF
                JNE     FFAIL

                LDW     =8              ; 8: T4.TX, nothing in it
                STW     FSTEP
                LDW     =FO4W
                JSX     FOPEN
                JSX     FCLOSE
                LDW     =FL4
                JSX     FLOOK
                JNZ     FFAIL
                LDW     FL4+DE.CNT
                JNZ     FFAIL
                LDW     FL4+DE.LST
                JNZ     FFAIL

                LDW     =9              ; 9: a name in use is not to be
                STW     FSTEP           ; had by renaming
                LDW     =FR24
                CALL    K.RENAM
                CMW     =E.EXST
                JNE     FFAIL

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
                CALL    K.OPEN
                CMW     =E.CHN
                JNE     FFAIL
                LDW     =4
                STW     FCHN
.CL4            JSX     FCLOSE
                LDW     FCHN
                SUB     =1
                STW     FCHN
                JNZ     .CL4

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
                CALL    K.OPEN
                JMI     FFAIL           ; an error is minus
                STW     FCHN
                EXIT    FOPEN

FCLOSE          SUBR
                LDW     FCHN
                CALL    K.CLOSE
                JNZ     FFAIL
                EXIT    FCLOSE

; K.LOOK on the eight words ACR addresses, on unit 0 -- the fifth word
; is the unit going in and the first of the answer coming out -- and the
; answer is the caller's to judge.
FLOOK           SUBR
                STW     FLB
                CAX
                CLR
                STW     *4
                LDW     FLB
                CALL    K.LOOK
                EXIT    FLOOK

; Write ACR bytes of the pattern to FCHN, and read as many and hold them
; to it.
FWRITE          SUBR
                STW     FN
                CLR
                STW     FI
.WL             JSX     FBYTE
                STW     FB
                LDW     FCHN
                SLL     8
                ORI     FB
                CALL    K.PUTC
                JNZ     FFAIL
                JSX     FSTEPI
                JNZ     .WL
                EXIT    FWRITE

FREAD           SUBR
                STW     FN
                CLR
                STW     FI
.RL             JSX     FBYTE
                STW     FB
                LDW     FCHN
                CALL    K.GETC
                CMW     FB
                JNE     FFAIL
                JSX     FSTEPI
                JNZ     .RL
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
FLB             WORD    0               ; FLOOK's block

FO1W            WORD    'T1',0,0,'TX',1 ; names on unit 0, with the mode K.OPEN wants
FO1R            WORD    'T1',0,0,'TX',0
FO2W            WORD    'T2',0,0,'TX',1
FO2R            WORD    'T2',0,0,'TX',0
FO4W            WORD    'T4',0,0,'TX',1
FO5W            WORD    'T5',0,0,'TX',1
FL1             WORD    'T1',0,0,'TX',0,0,0,0   ; and with room for K.LOOK's answer
FL2             WORD    'T2',0,0,'TX',0,0,0,0
FL3             WORD    'T3',0,0,'TX',0,0,0,0
FL4             WORD    'T4',0,0,'TX',0,0,0,0
FR13            WORD    'T1',0,0,'TX',0,'T3',0,0,'TX',0
FR24            WORD    'T2',0,0,'TX',0,'T4',0,0,'TX',0

FMOK            MSG     "FST OK\r\n"
FMBAD           MSG     "FST BAD "
                END     FST
