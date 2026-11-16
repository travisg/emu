; vim: ts=8:sw=8:expandtab:
;
; DUMP name -- a file in hexadecimal, eight words to a line behind the
; byte offset the line starts at.  A last odd byte is shown as the high
; half of a word.  Ctrl-C stops it, where it has the console.

DUMP            JSX     L.ARGS
                LDW     =UNAM
                JSX     L.NAME
                SAZ
                JMP     U1
                LDW     =UUSE
                JSX     L.MSG
                JMP     L.EXIT
U1              CLR
                STW     UNAM+4
                STW     UOFF
                LDW     =UNAM
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     UERR
                STW     UCH
ULINE           JSX     L.BRK
                SAZ
                JMP     UDONE
                JSX     UBYTE           ; the line's first byte, or none
                SAP
                JMP     UDONE
                STW     UW
                LDW     UOFF            ; less the byte just read
                SUB     =1
                JSX     L.HEX
                LDW     =8
                STW     UN
                JMP     UWORD
UNEXT           JSX     UBYTE
                SAP
                JMP     UEOL
                STW     UW
UWORD           LDW     =' '
                JSX     L.PUTC
                JSX     UBYTE           ; the word's low half
                SAP
                CLR
                STW     UW2
                LDW     UW
                SLL     8
                ORI     UW2
                JSX     L.HEX
                LDW     UN
                SUB     =1
                STW     UN
                SAZ
                JMP     UNEXT
                JSX     L.NL
                JMP     ULINE
UEOL            JSX     L.NL
UDONE           LDW     UCH
                SMB     K.CLOSE
                JSX     K.CLOSE
                JMP     L.EXIT
UERR            JSX     L.ERR
                JMP     L.EXIT

; The next byte, counted, or minus at the end.
UBYTE           SUBR
                LDW     UCH
                SMB     K.GETC
                JSX     K.GETC
                SAP
                JMP     UBX
                STW     UB
                LDW     UOFF
                ADD     =1
                STW     UOFF
                LDW     UB
UBX             EXIT    UBYTE

UNAM            WORD    0,0,0,0,0
UCH             WORD    0               ; the channel
UOFF            WORD    0               ; bytes read
UN              WORD    0               ; words left on the line
UW              WORD    0               ; a word's halves
UW2             WORD    0
UB              WORD    0
UUSE            WORD    UUSET*2,UUSEE*2
UUSET           TEXT    "DUMP NAME \r\n"
UUSEE           EQU     $
                END     DUMP
