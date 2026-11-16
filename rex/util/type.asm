; vim: ts=8:sw=8:expandtab:
;
; TYPE name -- a file on the printer.
;
; A line feed that does not follow a carriage return gets one in front
; of it, so a file made on a host with bare newlines still starts every
; line at the left margin.  Ctrl-C stops it.

TYPE            JSX     L.ARGS
                LDW     TKNAM
                JSX     L.NAME
                SAZ                     ; a name?
                JMP     TOPEN
                LDW     TKUSE
                JSX     L.MSG
                JMP     L.EXIT
TOPEN           CLR
                STW     TNAM+4          ; to read
                LDW     TKNAM
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     TERR
                STW     TCH
TLOOP           SMB     BRKREQ
                LDW     BRKREQ
                SAZ
                JMP     TDONE
                LDW     TCH
                SMB     K.GETC
                JSX     K.GETC
                SAP                     ; the end
                JMP     TDONE
                ORI     TK80            ; a host's file may lack bit 7
                STW     TC
                CMW     TKLF
                SEQ
                JMP     TPUT
                LDW     TLAST
                CMW     TKCR
                SNE
                JMP     TPUT
                LDW     TKCR
                JSX     L.PUTC
TPUT            LDW     TC
                STW     TLAST
                JSX     L.PUTC
                JMP     TLOOP
TDONE           LDW     TCH
                SMB     K.CLOSE
                JSX     K.CLOSE
                JMP     L.EXIT
TERR            JSX     L.ERR
                JMP     L.EXIT

TNAM            WORD    0,0,0,0,0
TCH             WORD    0               ; the channel
TC              WORD    0               ; the character, and the one before it
TLAST           WORD    0
TK80            WORD    X'0080'
TKCR            WORD    X'008D'
TKLF            WORD    X'008A'
TKNAM           WORD    TNAM
TKUSE           WORD    TUSE
TUSE            WORD    TUSET*2,TUSEE*2
TUSET           TEXT    "TYPE NAME \r\n"
TUSEE           EQU     $
                END     TYPE
