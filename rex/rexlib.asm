; vim: ts=8:sw=8:expandtab:
;
; What most modules want and none should write twice: the printer, and
; the command line.
;
; A deck that wants these puts this file between rexapi.asm and its own,
; and they are assembled into the module like any of its own routines --
; nothing is shared at run time, and the words count against the module's
; one page.  Each is an ordinary subroutine of the module's task: one
; activation, static cells.  Nothing here compares one byte address with
; another, so a module that uses it may load above word X'4000'.
;
;   L.PUTC   the character in ACR's low half, through the task's mailbox
;   L.PUTW   the bytes from ACR up to L.PWEND
;   L.MSG    the text whose descriptor ACR addresses: two byte addresses,
;            the first byte and one past the last
;   L.NL     carriage return and line feed
;   L.DEC    ACR in decimal, unsigned
;   L.HEX    ACR as four hexadecimal digits
;   L.SLEEP  ACR ticks, a sixtieth of a second each
;   L.ARGS   start reading the command tail (T.ARG) from its beginning
;   L.NAME   the next file name on it, into the four words ACR addresses,
;            as the directory holds one; a name with no type gets the one
;            in L.DTY, which is TX until a module stores another.  Zero
;            if the tail has run out, else not
;   L.ERR    what a file call answered, ACR an E. value, said in words
;   L.BRK    nonzero if a Ctrl-C is waiting for this task: the flag is the
;            console's, so a task without the console is never told of one
;   L.EXIT   the task is over

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

L.HEX           SUBR
                STW     L.V
                LDW     L.K4
                STW     L.DP
L.HXL           LDW     L.V             ; the top digit, each time round
                SRL     12
                CMW     L.KTEN
                SLS                     ; a letter?
                ADD     L.K7
                ADD     L.KZER
                JSX     L.PUTC
                LDW     L.V
                SLL     4
                STW     L.V
                LDW     L.DP
                SUB     L.K1
                STW     L.DP
                SAZ
                JMP     L.HXL
                EXIT    L.HEX

L.ARGS          SUBR
                SMB     CURT
                LDX     CURT
                LDW     *T.ARG
                SAZ                     ; a tail at all?
                JMP     L.AR1
                LDW     L.KNUL          ; none: a tail of nothing
L.AR1           SLL     1
                STW     L.AP
                EXIT    L.ARGS

; The shell's SHNAME over the tail: up to six characters of name, two of
; type after a dot, each left-justified and zero-filled, the rest of an
; over-long word stepped over.
L.NAME          SUBR
                STW     L.NB
                CAX
                CLR
                STW     *0
                STW     *1
                STW     *2
                LDW     L.DTY
                STW     *3
L.NSK           LDX     L.AP            ; past the blanks
                CLR
                LDB     *0
                CMW     L.KSP
                SEQ
                JMP     L.NGO
                JSX     L.NADV
                JMP     L.NSK
L.NGO           LDW     L.NB
                SLL     1
                STW     L.NP
                LDW     L.K6
                STW     L.NN
L.NL1           LDX     L.AP
                CLR
                LDB     *0
                SAZ                     ; the zero ends it
                JMP     L.N1
                JMP     L.NX
L.N1            CMW     L.KSP           ; and so does a blank
                SNE
                JMP     L.NX
                CMW     L.KDOT          ; a dot starts the type
                SNE
                JMP     L.NDOT
                STW     L.NC
                LDW     L.NN            ; room for it?
                SAZ
                JMP     L.N2
                JMP     L.N3
L.N2            SUB     L.K1
                STW     L.NN
                LDX     L.NP
                LDW     L.NC
                STB     *0
                LDW     L.NP
                ADD     L.K1
                STW     L.NP
L.N3            JSX     L.NADV
                JMP     L.NL1
L.NDOT          LDX     L.NB
                CLR
                STW     *3
                LDW     L.NB
                SLL     1
                ADD     L.K6
                STW     L.NP
                LDW     L.K2
                STW     L.NN
                JMP     L.N3
L.NX            LDX     L.NB
                LDW     *0
                EXIT    L.NAME
L.NADV          SUBR
                LDW     L.AP
                ADD     L.K1
                STW     L.AP
                EXIT    L.NADV

; The E. values run down from minus one, so minus one less the value is
; an index into the table.
L.ERR           SUBR
                STW     L.V
                LDW     L.KM1
                SUB     L.V
                CMW     L.KNERR
                SLS                     ; one of them?
                CLR
                SLL     1
                ADD     L.KETAB
                JSX     L.MSG
                JSX     L.NL
                EXIT    L.ERR

L.BRK           SUBR
                SMB     CURT
                LDX     CURT
                LDW     *T.CON
                SAZ                     ; the console's holder?
                JMP     L.BRK1
                EXIT    L.BRK
L.BRK1          SMB     BRKREQ
                LDW     BRKREQ
                EXIT    L.BRK

L.EXIT          SMB     K.EXIT
                JMP     K.EXIT

L.PWEND         WORD    0               ; L.PUTW's end of window
L.PWP           WORD    0               ; ...and cursor
L.V             WORD    0               ; L.DEC's running value...
L.DP            WORD    0               ; ...and where its next digit goes
L.DB            RES     3               ; six digits, filled backwards
L.AP            WORD    0               ; the command tail's cursor, a byte address
L.NB            WORD    0               ; L.NAME's four words,
L.NP            WORD    0               ; where its next character goes,
L.NN            WORD    0               ; how many more it will take,
L.NC            WORD    0               ; and the character in hand
L.DTY           WORD    'TX'            ; the type a bare name is given
L.NULW          WORD    0               ; a tail of nothing
L.K1            WORD    1
L.K2            WORD    2
L.K4            WORD    4
L.K6            WORD    6
L.K7            WORD    7               ; from '9'+1 to 'A'
L.K0FF          WORD    X'00FF'
L.KM1           WORD    X'FFFF'
L.KTEN          WORD    10
L.KZER          WORD    '0'
L.KSP           WORD    ' '
L.KDOT          WORD    '.'
L.KCR           WORD    X'008D'
L.KLF           WORD    X'008A'
L.KSLP          WORD    SSLP
L.KWAI          WORD    SWAI
L.KDBE          WORD    L.DB*2+5        ; the last byte of the digit buffer
L.KNUL          WORD    L.NULW
L.KNERR         WORD    8               ; how many E. values there are
L.KETAB         WORD    L.ETAB

L.ETAB          WORD    L.E1T*2,L.E1E*2 ; E.NOF, and on down
                WORD    L.E2T*2,L.E2E*2
                WORD    L.E3T*2,L.E3E*2
                WORD    L.E4T*2,L.E4E*2
                WORD    L.E5T*2,L.E5E*2
                WORD    L.E6T*2,L.E6E*2
                WORD    L.E7T*2,L.E7E*2
                WORD    L.E8T*2,L.E8E*2
L.E1T           TEXT    "NO SUCH FILE"
L.E1E           EQU     $
L.E2T           TEXT    "NO CHANNEL"
L.E2E           EQU     $
L.E3T           TEXT    "DIRECTORY FULL"
L.E3E           EQU     $
L.E4T           TEXT    "DISC FULL "
L.E4E           EQU     $
L.E5T           TEXT    "DISC ERROR"
L.E5E           EQU     $
L.E6T           TEXT    "NO DIRECTORY"
L.E6E           EQU     $
L.E7T           TEXT    "SYSTEM FILE "
L.E7E           EQU     $
L.E8T           TEXT    "NAME IN USE "
L.E8E           EQU     $
