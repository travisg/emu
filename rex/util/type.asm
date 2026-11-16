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
                JNZ     .OPEN           ; a name?
                LDW     =TUSE
                JSX     L.MSG
                JMP     L.EXIT
.OPEN           CLR
                STW     TNAM+4          ; to read
                LDW     =TNAM
                CALL    K.OPEN
                JMI     .ERR
                STW     TCH
.LOOP           JSX     L.BRK
                JNZ     .DONE
                LDW     TCH
                CALL    K.GETC
                JMI     .DONE           ; the end
                ORI     =X'0080'        ; a host's file may lack bit 7
                STW     TC
                CMW     =L.KLF
                JNE     .PUT
                LDW     TLAST
                CMW     =L.KCR
                JEQ     .PUT
                LDW     =L.KCR
                JSX     L.PUTC
.PUT            LDW     TC
                STW     TLAST
                JSX     L.PUTC
                JMP     .LOOP
.DONE           LDW     TCH
                CALL    K.CLOSE
                JMP     L.EXIT
.ERR            JSX     L.ERR
                JMP     L.EXIT

TNAM            WORD    0,0,0,0,0
TCH             WORD    0               ; the channel
TC              WORD    0               ; the character, and the one before it
TLAST           WORD    0
TUSE            MSG     "TYPE NAME\r\n"
                END     TYPE
