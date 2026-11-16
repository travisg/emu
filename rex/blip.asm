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
BSKP            LDX     L.AP            ; past the blanks, to the character
                CLR
                LDB     *0
                CMW     L.KSP
                SEQ
                JMP     BGOT
                JSX     L.NADV
                JMP     BSKP
BGOT            SAZ                     ; the tail ran out first?
                JMP     BKEEP
                LDW     L.KDOT
BKEEP           STW     BCH

                SMB     CURT
                LDX     CURT
                LDW     *T.ID
                STW     BN
                LDW     BK15
                STW     BNAP
BMUL            LDW     BNAP            ; fifteen more, the job number times
                ADD     BK15
                STW     BNAP
                LDW     BN
                SUB     L.K1
                STW     BN
                SAZ
                JMP     BMUL

BLOOP           LDW     BCH
                JSX     L.PUTC
                LDW     BNAP
                JSX     L.SLEEP
                JMP     BLOOP

BCH             WORD    0               ; the character
BN              WORD    0               ; the multiply's count
BNAP            WORD    0               ; the sleep, in ticks
BK15            WORD    15
                END     BLIP
