; vim: ts=8:sw=8:expandtab:
;
; COPY from to -- a file, again, under another name.

COPY            JSX     L.ARGS
                LDW     =CFROM
                JSX     L.NAME
                JNZ     .TO
                JMP     .USAGE
.TO             LDW     =CTO
                JSX     L.NAME
                JNZ     .GO
.USAGE          LDW     =CUSE
                JSX     L.MSG
                JMP     L.EXIT
.GO             CLR
                STW     CFROM+4         ; one to read...
                LDW     =CFROM
                CALL    K.OPEN
                JMI     .ERR
                STW     CIN
                LDW     =1
                STW     CTO+4           ; ...and one to write
                LDW     =CTO
                CALL    K.OPEN
                JMI     .ERR
                SLL     8
                STW     COUT
.LOOP           LDW     CIN
                CALL    K.GETC
                JMI     .DONE           ; the end
                ORI     COUT
                CALL    K.PUTC
                JNZ     .ERR
                JMP     .LOOP
.DONE           LDW     COUT
                SRL     8
                CALL    K.CLOSE
                JNZ     .ERR
                JMP     L.EXIT          ; the exit lets the other channel go
.ERR            JSX     L.ERR
                JMP     L.EXIT

CFROM           WORD    0,0,0,0,0
CTO             WORD    0,0,0,0,0
CIN             WORD    0               ; the channel read from
COUT            WORD    0               ; and the one written to, times 256
CUSE            MSG     "COPY FROM TO\r\n"
                END     COPY
