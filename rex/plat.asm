; vim: ts=8:sw=8:expandtab:
;
; PLAT -- a file written on unit 1 across a look at unit 0.
;
; A test module, run where unit 1 is the data platter: it opens 1:PW.TX
; to write, puts a sector and a half into it, looks at 0:MSG.TX -- which
; brings unit 0's directory into core in place of unit 1's -- puts the
; rest, and closes it, which has to bring unit 1's back and find the file
; still being written there: the channel is told from unit 0's by its
; unit alone, the two platters' sector numbers being the same.  Then a
; look at 1:PW.TX has to find all of it.  It prints PLAT OK, or PLAT BAD
; and the number of the step that went wrong.

PLAT            LDW     =1              ; 1: open 1:PW.TX to write
                STW     PSTEP
                LDW     =PWW
                CALL    K.OPEN
                JMI     PFAIL
                SLL     8
                STW     PCHN
                LDW     =2              ; 2: a sector and a half of it
                STW     PSTEP
                LDW     =140
                JSX     PPUT
                LDW     =3              ; 3: a look at unit 0
                STW     PSTEP
                LDW     =PLM
                CALL    K.LOOK
                JNZ     PFAIL
                LDW     =4              ; 4: the rest, and closed
                STW     PSTEP
                LDW     =60
                JSX     PPUT
                LDW     PCHN
                SRL     8
                CALL    K.CLOSE
                JNZ     PFAIL
                LDW     =5              ; 5: all of it there, on unit 1
                STW     PSTEP
                LDW     =PLW
                CALL    K.LOOK
                JNZ     PFAIL
                LDW     PLW+DE.CNT      ; 200 bytes: three sectors, the
                CMW     =3              ; last holding 12
                JNE     PFAIL
                LDW     PLW+DE.LST
                CMW     =12
                JNE     PFAIL
                LDW     =PMOK
                JSX     L.MSG
                JMP     L.EXIT

PFAIL           LDW     =PMBAD
                JSX     L.MSG
                LDW     PSTEP
                JSX     L.DEC
                JSX     L.NL
                JMP     L.EXIT

; ACR bytes, each 'A', down the channel.
PPUT            SUBR
                STW     PN
.L              LDW     ='A'
                ORI     PCHN
                CALL    K.PUTC
                JNZ     PFAIL
                LDW     PN
                SUB     =1
                STW     PN
                JNZ     .L
                EXIT    PPUT

PSTEP           WORD    0               ; the step in hand
PCHN            WORD    0               ; the channel, times 256
PN              WORD    0               ; bytes to go
PWW             WORD    'PW',0,0,'TX',X'0101'   ; 1:PW.TX, to write
PLM             WORD    'MS',X'C700',0,'TX',0,0,0,0     ; 0:MSG.TX, and room for the answer
PLW             WORD    'PW',0,0,'TX',X'0100',0,0,0     ; 1:PW.TX
PMOK            MSG     "PLAT OK\r\n"
PMBAD           MSG     "PLAT BAD "
                END     PLAT
