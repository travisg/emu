; vim: ts=8:sw=8:expandtab:
;
; What most modules want and none should write twice: the printer, and
; the command line.
;
; A library: a deck that wants these puts this file behind rexapi.asm and
; rexmac.asm and in front of its own, and each part under a LIBR card is
; assembled into the module only if the module, or a part it has taken,
; names what the card lists.  Nothing is shared at run time, and the
; words that are taken count against the module's one page.  Each routine
; is an ordinary subroutine of the module's task: one activation, static
; cells.  Nothing here compares one byte address with another, so a
; module that uses it may load above word X'4000'.
;
;   L.PUTC   the character in ACR's low half, through the task's mailbox
;   L.PUTW   the bytes from ACR up to L.PWEND
;   L.MSG    the text whose descriptor ACR addresses: two byte addresses,
;            the first byte and one past the last
;   L.NL     carriage return and line feed
;   L.DEC    ACR in decimal, unsigned
;   L.HEX    ACR as four hexadecimal digits
;   L.SLEEP  ACR ticks, a fiftieth of a second each
;   L.ARGS   start reading the command tail (T.ARG) from its beginning:
;            L.AP is then the byte address of its next character
;   L.NADV   L.AP one character on
;   L.NAME   the next file name on it, into the five words ACR addresses,
;            as K.OPEN takes one, unit and all; a name with no type gets
;            the one in L.DTY, which is TX until a module stores another.
;            Zero if the tail has run out, else not
;   L.ERR    what a file call answered, ACR an E. value, said in words
;   L.BRK    nonzero if a Ctrl-C is waiting for this task: the flag is the
;            console's, so a task without the console is never told of one
;   L.EXIT   the task is over

L.KCR           EQU     X'008D'         ; carriage return and line feed, as
L.KLF           EQU     X'008A'         ; the teletype sends and takes them

                LIBR    L.PUTC
; Deposit under MSK, kick the printer, then masked-look/SWAI/SWTCH until
; SERV clears the cell -- the shell's SHPUTC against this task's own node.
L.PUTC          SUBR
                AND     =X'00FF'
                MSK
                SMB     CURT
                LDX     CURT
                STW     *T.MBX
                CALL    K.KICK
                UNM
L.PCWT          MSK
                SMB     CURT
                LDX     CURT
                LDW     *T.MBX
                JNZ     L.PCWB          ; printed yet?
                JMP     L.PCWD
L.PCWB          LDW     =SWAI           ; no: stand down until SERV says so
                STW     *T.STA
                CALL    K.SWTCH
                JMP     L.PCWT
L.PCWD          UNM
                EXIT    L.PUTC

                LIBR    L.PUTW,L.PWEND
L.PUTW          SUBR
                STW     L.PWP
L.PWL           LDW     L.PWP
                CMW     L.PWEND
                JEQ     L.PWDN
                CAX
                CLR                     ; LDB replaces only the low half
                LDB     *0
                JSX     L.PUTC
                LDW     L.PWP
                ADD     =1
                STW     L.PWP
                JMP     L.PWL
L.PWDN          EXIT    L.PUTW
L.PWEND         WORD    0               ; L.PUTW's end of window
L.PWP           WORD    0               ; ...and cursor

                LIBR    L.MSG
L.MSG           SUBR
                CAX
                LDW     *1
                STW     L.PWEND
                LDW     *0
                JSX     L.PUTW
                EXIT    L.MSG

                LIBR    L.NL
L.NL            SUBR
                LDW     =L.KCR
                JSX     L.PUTC
                LDW     =L.KLF
                JSX     L.PUTC
                EXIT    L.NL

                LIBR    L.DEC
; The digits come out of the divide backwards, so they go into a buffer
; from its end and print forwards.  The 31-bit dividend is IXR:ACR with
; the sign duplicated into ACR's top bit, which the shift and the copy
; build.
L.DEC           SUBR
                STW     L.V
                LDW     =L.KDBE
                STW     L.DP
L.DL            LDW     L.V
                SRL     15
                CAX
                LDW     L.V
                DIV     =10
                STW     L.V             ; the quotient
                CXA                     ; the remainder is the digit
                ADD     ='0'
                LDX     L.DP
                STB     *0
                LDW     L.DP
                SUB     =1
                STW     L.DP
                LDW     L.V
                JNZ     L.DL            ; nothing left of it?
                LDW     =L.KDBE
                ADD     =1
                STW     L.PWEND
                LDW     L.DP
                ADD     =1
                JSX     L.PUTW
                EXIT    L.DEC
L.DB            RES     3               ; six digits, filled backwards
L.KDBE          EQU     L.DB*2+5        ; the last byte of the digit buffer

; The cells L.DEC, L.HEX and L.ERR work in, one activation of any of
; them at a time.
                LIBR    L.V
L.V             WORD    0               ; the value in hand
                LIBR    L.DP
L.DP            WORD    0               ; the next digit's place, or its count

                LIBR    L.SLEEP
; The delay and the state go into the node in one masked window;
; K.SWTCH returns when the scheduler next picks this task.
L.SLEEP         SUBR
                MSK
                SMB     CURT
                LDX     CURT
                STW     *T.DLY
                LDW     =SSLP
                STW     *T.STA
                CALL    K.SWTCH
                EXIT    L.SLEEP

                LIBR    L.HEX
L.HEX           SUBR
                STW     L.V
                LDW     =4
                STW     L.DP
L.HXL           LDW     L.V             ; the top digit, each time round
                SRL     12
                CMW     =10
                SLS                     ; a letter?
                ADD     =L.K7
                ADD     ='0'
                JSX     L.PUTC
                LDW     L.V
                SLL     4
                STW     L.V
                LDW     L.DP
                SUB     =1
                STW     L.DP
                JNZ     L.HXL
                EXIT    L.HEX
L.K7            EQU     7               ; from '9'+1 to 'A'

                LIBR    L.ARGS,L.AP
L.ARGS          SUBR
                SMB     CURT
                LDX     CURT
                LDW     *T.ARG
                JNZ     L.AR1           ; a tail at all?
                LDW     =L.NULW         ; none: a tail of nothing
L.AR1           SLL     1
                STW     L.AP
                EXIT    L.ARGS
L.AP            WORD    0               ; the command tail's cursor, a byte address
L.NULW          WORD    0               ; a tail of nothing

                LIBR    L.NADV
L.NADV          SUBR
                LDW     L.AP
                ADD     =1
                STW     L.AP
                EXIT    L.NADV

                LIBR    L.NAME,L.DTY
; The next name in the tail into the five words ACR addresses, the type
; in L.DTY for one without, and the cursor past it: K.NAME's reading of
; a name, as everything's.  The answer is its first word, zero for none.
L.NAME          SUBR
                STW     L.NB+2
                LDW     L.AP
                STW     L.NB
                LDW     L.DTY
                STW     L.NB+1
                LDW     =L.NB
                CALL    K.NAME
                STW     L.NC
                LDW     L.NB
                STW     L.AP
                LDW     L.NC
                EXIT    L.NAME
L.NB            WORD    0,0,0           ; L.NAME's three words for K.NAME
L.NC            WORD    0               ; and its answer
L.DTY           WORD    'TX'            ; the type a bare name is given

                LIBR    L.ERR
; The E. values run down from minus one, so minus one less the value is
; an index into the table.
L.ERR           SUBR
                STW     L.V
                LDW     =X'FFFF'
                SUB     L.V
                CMW     =L.KNERR
                SLS                     ; one of them?
                CLR
                SLL     1
                ADD     =L.ETAB
                JSX     L.MSG
                JSX     L.NL
                EXIT    L.ERR
L.KNERR         EQU     9               ; how many E. values there are
L.ETAB          WORD    L.E1T*2,L.E1E*2 ; E.NOF, and on down
                WORD    L.E2T*2,L.E2E*2
                WORD    L.E3T*2,L.E3E*2
                WORD    L.E4T*2,L.E4E*2
                WORD    L.E5T*2,L.E5E*2
                WORD    L.E6T*2,L.E6E*2
                WORD    L.E7T*2,L.E7E*2
                WORD    L.E8T*2,L.E8E*2
                WORD    L.E9T*2,L.E9E*2
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
L.E9T           TEXT    "NOT ONE PLATTER "
L.E9E           EQU     $

                LIBR    L.BRK
L.BRK           SUBR
                SMB     CURT
                LDX     CURT
                LDW     *T.CON
                JNZ     L.BRK1          ; the console's holder?
                EXIT    L.BRK
L.BRK1          SMB     K.BRKQ
                JSX     K.BRKQ
                EXIT    L.BRK

                LIBR    L.EXIT
L.EXIT          SMB     K.EXIT
                JMP     K.EXIT
