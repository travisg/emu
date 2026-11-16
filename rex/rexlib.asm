; vim: ts=8:sw=8:expandtab:
;
; What most modules want and none should write twice: the printer.
;
; A deck that wants these puts this file between rexapi.asm and its own,
; and they are assembled into the module like any of its own routines --
; nothing is shared at run time, and the words count against the module's
; one page.  Each is an ordinary subroutine of the module's task: one
; activation, static cells.
;
;   L.PUTC   the character in ACR's low half, through the task's mailbox
;   L.PUTW   the bytes from ACR up to L.PWEND
;   L.MSG    the text whose descriptor ACR addresses: two byte addresses,
;            the first byte and one past the last
;   L.NL     carriage return and line feed
;   L.DEC    ACR in decimal, unsigned
;   L.SLEEP  ACR ticks, a sixtieth of a second each

; Deposit under MSK, kick the printer, then masked-look/SWAI/SWTCH until
; SERV clears the cell -- the shell's SHPUTC against this task's own node.
L.PUTC          SUBR
                AND     L.K0FF
                MSK
                SMB     CURT
                LDX     CURT
                STW     *T.MBX
                SMB     K.KICK
                JSX     K.KICK
                UNM
L.PCWT          MSK
                SMB     CURT
                LDX     CURT
                LDW     *T.MBX
                SAZ                     ; printed yet?
                JMP     L.PCWB
                JMP     L.PCWD
L.PCWB          LDW     L.KWAI          ; no: stand down until SERV says so
                STW     *T.STA
                SMB     K.SWTCH
                JSX     K.SWTCH
                JMP     L.PCWT
L.PCWD          UNM
                EXIT    L.PUTC

L.PUTW          SUBR
                STW     L.PWP
L.PWL           LDW     L.PWP
                CMW     L.PWEND
                SNE
                JMP     L.PWDN
                CAX
                CLR                     ; LDB replaces only the low half
                LDB     *0
                JSX     L.PUTC
                LDW     L.PWP
                ADD     L.K1
                STW     L.PWP
                JMP     L.PWL
L.PWDN          EXIT    L.PUTW

L.MSG           SUBR
                CAX
                LDW     *1
                STW     L.PWEND
                LDW     *0
                JSX     L.PUTW
                EXIT    L.MSG

L.NL            SUBR
                LDW     L.KCR
                JSX     L.PUTC
                LDW     L.KLF
                JSX     L.PUTC
                EXIT    L.NL

; The digits come out of the divide backwards, so they go into a buffer
; from its end and print forwards.  The 31-bit dividend is IXR:ACR with
; the sign duplicated into ACR's top bit, which the shift and the copy
; build.
L.DEC           SUBR
                STW     L.V
                LDW     L.KDBE
                STW     L.DP
L.DL            LDW     L.V
                SRL     15
                CAX
                LDW     L.V
                DIV     L.KTEN
                STW     L.V             ; the quotient
                CXA                     ; the remainder is the digit
                ADD     L.KZER
                LDX     L.DP
                STB     *0
                LDW     L.DP
                SUB     L.K1
                STW     L.DP
                LDW     L.V
                SAZ                     ; nothing left of it?
                JMP     L.DL
                LDW     L.KDBE
                ADD     L.K1
                STW     L.PWEND
                LDW     L.DP
                ADD     L.K1
                JSX     L.PUTW
                EXIT    L.DEC

; The delay and the state go into the node in one masked window;
; K.SWTCH returns when the scheduler next picks this task.
L.SLEEP         SUBR
                MSK
                SMB     CURT
                LDX     CURT
                STW     *T.DLY
                LDW     L.KSLP
                STW     *T.STA
                SMB     K.SWTCH
                JSX     K.SWTCH
                EXIT    L.SLEEP

L.PWEND         WORD    0               ; L.PUTW's end of window
L.PWP           WORD    0               ; ...and cursor
L.V             WORD    0               ; L.DEC's running value...
L.DP            WORD    0               ; ...and where its next digit goes
L.DB            RES     3               ; six digits, filled backwards
L.K1            WORD    1
L.K0FF          WORD    X'00FF'
L.KTEN          WORD    10
L.KZER          WORD    '0'
L.KCR           WORD    X'008D'
L.KLF           WORD    X'008A'
L.KSLP          WORD    SSLP
L.KWAI          WORD    SWAI
L.KDBE          WORD    L.DB*2+5        ; the last byte of the digit buffer
