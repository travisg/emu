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
                SAZ                     ; the console?
                JMP     P0
                LDW     =PNCON
                JSX     L.MSG
                JMP     L.EXIT
P0              JSX     L.ARGS
                LDW     =PNAM
                JSX     L.NAME
                SAZ
                JMP     P1
                LDW     =PUSE
                JSX     L.MSG
                JMP     L.EXIT
P1              LDW     =1
                STW     PNAM+4          ; to write
                LDW     =PNAM
                SMB     K.OPEN
                JSX     K.OPEN
                SAP
                JMP     PERR
                SLL     8
                STW     PCH
PLOOP           SMB     KCONSQ
                LDW     KCONSQ
                SMB     K.QGET
                JSX     K.QGET
                AND     =X'00FF'
                STW     PC
                CMW     =PKCTLZ
                SNE
                JMP     PDONE
                ORI     PCH
                SMB     K.PUTC
                JSX     K.PUTC
                SAZ
                JMP     PERR
                LDW     PC
                CMW     =L.KCR
                SEQ
                JMP     PLOOP
                LDW     =L.KLF
                ORI     PCH
                SMB     K.PUTC
                JSX     K.PUTC
                SAZ
                JMP     PERR
                JMP     PLOOP
PDONE           LDW     PCH
                SRL     8
                SMB     K.CLOSE
                JSX     K.CLOSE
                SAZ
                JMP     PERR
                JSX     L.NL
                JMP     L.EXIT
PERR            JSX     L.ERR
                JMP     L.EXIT

PNAM            WORD    0,0,0,0,0
PCH             WORD    0               ; the channel, times 256
PC              WORD    0               ; the character
PKCTLZ          EQU     X'009A'         ; Ctrl-Z
PUSE            WORD    PUSET*2,PUSEE*2
PNCON           WORD    PNCONT*2,PNCONE*2
PUSET           TEXT    "PUT NAME\r\n"
PUSEE           EQU     $
PNCONT          TEXT    "PUT NEEDS THE CONSOLE \r\n"
PNCONE          EQU     $
                END     PUT
