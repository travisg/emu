; vim: ts=8:sw=8:expandtab:
;
; COPY from to -- a file, again, under another name.

COPY            JSX     L.ARGS
                LDW     =CFROM
                JSX     L.NAME
                SAZ
                JMP     C1
                JMP     CUSAGE
C1              LDW     =CTO
                JSX     L.NAME
                SAZ
                JMP     C2
CUSAGE          LDW     =CUSE
                JSX     L.MSG
                JMP     L.EXIT
C2              CLR
                STW     CFROM+4         ; one to read...
                LDW     =CFROM
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     CERR
                STW     CIN
                LDW     =1
                STW     CTO+4           ; ...and one to write
                LDW     =CTO
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     CERR
                SLL     8
                STW     COUT
CLOOP           LDW     CIN
                SMB     K.GETC
                JSX     K.GETC
                SAP                     ; the end
                JMP     CDONE
                ORI     COUT
                SMB     K.PUTC
                JSX     K.PUTC
                SAZ
                JMP     CERR
                JMP     CLOOP
CDONE           LDW     COUT
                SRL     8
                SMB     K.CLOSE
                JSX     K.CLOSE
                SAZ
                JMP     CERR
                JMP     L.EXIT          ; the exit lets the other channel go
CERR            JSX     L.ERR
                JMP     L.EXIT

CFROM           WORD    0,0,0,0,0
CTO             WORD    0,0,0,0,0
CIN             WORD    0               ; the channel read from
COUT            WORD    0               ; and the one written to, times 256
CUSE            WORD    CUSET*2,CUSEE*2
CUSET           TEXT    "COPY FROM TO\r\n"
CUSEE           EQU     $
                END     COPY
