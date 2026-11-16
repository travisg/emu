; vim: ts=8:sw=8:expandtab:
;
; TYPE name -- a file on the printer.
;
; A line feed that does not follow a carriage return gets one in front
; of it, so a file made on a host with bare newlines still starts every
; line at the left margin.  Ctrl-C stops it, where it has the console.

TYPE            JSX     L.ARGS
                LDW     =TNAM
                JSX     L.NAME
                SAZ                     ; a name?
                JMP     TOPEN
                LDW     =TUSE
                JSX     L.MSG
                JMP     L.EXIT
TOPEN           CLR
                STW     TNAM+4          ; to read
                LDW     =TNAM
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     TERR
                STW     TCH
TLOOP           JSX     L.BRK
                SAZ
                JMP     TDONE
                LDW     TCH
                SMB     K.GETC
                JSX     K.GETC
                SAP                     ; the end
                JMP     TDONE
                ORI     =X'0080'        ; a host's file may lack bit 7
                STW     TC
                CMW     =L.KLF
                SEQ
                JMP     TPUT
                LDW     TLAST
                CMW     =L.KCR
                SNE
                JMP     TPUT
                LDW     =L.KCR
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
TUSE            WORD    TUSET*2,TUSEE*2
TUSET           TEXT    "TYPE NAME \r\n"
TUSEE           EQU     $
                END     TYPE
