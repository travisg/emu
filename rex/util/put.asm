; vim: ts=8:sw=8:expandtab:
;
; PUT name -- the keyboard into a file, until Ctrl-Z.
;
; What is typed goes in as it is typed -- there is no line to edit, and a
; rubout is a character like another -- except that a carriage return is
; followed into the file by a line feed, as the teletype's own echo of
; the key follows it on the paper.  It reads the console, so it has to
; have been run in the foreground.

PUT             SMB     CURT
                LDX     CURT
                LDW     *T.CON
                JNZ     .ARGS             ; the console?
                LDW     =PNCON
                JSX     L.MSG
                JMP     L.EXIT
.ARGS             JSX     L.ARGS
                LDW     =PNAM
                JSX     L.NAME
                JNZ     .OPEN
                LDW     =PUSE
                JSX     L.MSG
                JMP     L.EXIT
.OPEN             LDW     =1
                STW     PNAM+4          ; to write
                LDW     =PNAM
                CALL    K.OPEN
                JMI     .ERR
                SLL     8
                STW     PCH
.LOOP           SMB     KCONSQ
                LDW     KCONSQ
                CALL    K.QGET
                AND     =X'00FF'
                STW     PC
                CMW     =PKCTLZ
                JEQ     .DONE
                ORI     PCH
                CALL    K.PUTC
                JNZ     .ERR
                LDW     PC
                CMW     =L.KCR
                JNE     .LOOP
                LDW     =L.KLF
                ORI     PCH
                CALL    K.PUTC
                JNZ     .ERR
                JMP     .LOOP
.DONE           LDW     PCH
                SRL     8
                CALL    K.CLOSE
                JNZ     .ERR
                JSX     L.NL
                JMP     L.EXIT
.ERR            JSX     L.ERR
                JMP     L.EXIT

PNAM            WORD    0,0,0,0,0
PCH             WORD    0               ; the channel, times 256
PC              WORD    0               ; the character
PKCTLZ          EQU     X'009A'         ; Ctrl-Z
PUSE            MSG     "PUT NAME\r\n"
PNCON           MSG     "PUT NEEDS THE CONSOLE\r\n"
                END     PUT
