; vim: ts=8:sw=8:expandtab:
;
; DUMP name -- a file in hexadecimal, eight words to a line behind the
; byte offset the line starts at.  A last odd byte is shown as the high
; half of a word.  Ctrl-C stops it, where it has the console.

DUMP            JSX     L.ARGS
                LDW     =UNAM
                JSX     L.NAME
                JNZ     .OPEN
                LDW     =UUSE
                JSX     L.MSG
                JMP     L.EXIT
.OPEN             CLR
                STW     UOFF
                LDW     =UNAM
                CALL    K.OPEN
                JMI     .ERR
                STW     UCH
.LINE           JSX     L.BRK
                JNZ     .DONE
                JSX     UBYTE           ; the line's first byte, or none
                JMI     .DONE
                STW     UW
                LDW     UOFF            ; less the byte just read
                SUB     =1
                JSX     L.HEX
                LDW     =8
                STW     UN
                JMP     .WORD
.NEXT           JSX     UBYTE
                JMI     .EOL
                STW     UW
.WORD           LDW     =' '
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
                JNZ     .NEXT
                JSX     L.NL
                JMP     .LINE
.EOL            JSX     L.NL
.DONE           LDW     UCH
                CALL    K.CLOSE
                JMP     L.EXIT
.ERR            JSX     L.ERR
                JMP     L.EXIT

; The next byte, counted, or minus at the end.
UBYTE           SUBR
                LDW     UCH
                CALL    K.GETC
                JMI     .BX
                STW     UB
                LDW     UOFF
                ADD     =1
                STW     UOFF
                LDW     UB
.BX             EXIT    UBYTE

UNAM            WORD    0,0,0,0,0
UCH             WORD    0               ; the channel
UOFF            WORD    0               ; bytes read
UN              WORD    0               ; words left on the line
UW              WORD    0               ; a word's halves
UW2             WORD    0
UB              WORD    0
UUSE            MSG     "DUMP NAME\r\n"
                END     DUMP
