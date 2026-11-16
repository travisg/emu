; vim: ts=8:sw=8:expandtab:
;
; BLIP c -- print a character, sleep, and again, for ever.
;
; A test module: several of these behind the prompt are the scheduler
; made visible.  Each prints the first character of its command tail --
; a dot, given none -- through its mailbox and then sleeps fifteen ticks
; for its job number and fifteen more, so the first three loaded come
; round every 30, 45 and 60 ticks and drift through every phase against
; each other.  KILL is the way out of it.

BLIP            JSX     L.ARGS
.SKP            LDX     L.AP            ; past the blanks, to the character
                CLR
                LDB     *0
                CMW     =' '
                JNE     .GOT
                JSX     L.NADV
                JMP     .SKP
.GOT            JNZ     .KEEP           ; the tail ran out first?
                LDW     ='.'
.KEEP           STW     BCH

                SMB     CURT
                LDX     CURT
                LDW     *T.ID
                STW     BN
                LDW     =15
                STW     BNAP
.MUL            LDW     BNAP            ; fifteen more, the job number times
                ADD     =15
                STW     BNAP
                LDW     BN
                SUB     =1
                STW     BN
                JNZ     .MUL

.LOOP           LDW     BCH
                JSX     L.PUTC
                LDW     BNAP
                JSX     L.SLEEP
                JMP     .LOOP

BCH             WORD    0               ; the character
BN              WORD    0               ; the multiply's count
BNAP            WORD    0               ; the sleep, in ticks
                END     BLIP
