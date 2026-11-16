; vim: ts=8:sw=8:expandtab:
;
; DSKT -- the disc, from a task, against other tasks doing the same.
;
; A test module: it takes sector DSKSEC plus its own job number as its
; own, and DSKRND times over writes a pattern there through K.DWRIT,
; clears its buffer, reads the sector back through K.DREAD and compares.
; The pattern carries the job number and the round, so a transfer that
; landed in another task's buffer or sector, or never happened, shows.
; It prints nothing -- three of these run at once and the printer would
; make soup of it -- and leaves its verdict on the platter instead: the
; last round's pattern if every round came back whole, a sector of
; X'DEAD' if one did not.  run_rex_test.py loads three and reads the
; platter afterwards.
;
; Word i of round r from job j is j*256 + r + i.

DSKSEC          EQU     4000            ; job j's sector is this plus j
DSKRND          EQU     250             ; under 256: the round shares a byte

DSKT            SMB     CURT
                LDX     CURT
                LDW     *T.ID
                STW     DID
                ADD     DKSEC
                STW     DBLK            ; the transfer block's sector
                CLR
                STW     DRND

DROUND          LDW     DRND
                ADD     DK1
                STW     DRND
                JSX     DFIRST          ; lay the pattern down
DFILL           LDX     DP
                LDW     DVAL
                STW     *0
                JSX     DNEXT
                SAZ                     ; the whole sector?
                JMP     DFILL
                LDW     DKBLK
                SMB     K.DWRIT
                JSX     K.DWRIT
                SAZ                     ; a clean write?
                JMP     DFAIL

                JSX     DFIRST          ; forget it
DCLR            LDX     DP
                CLR
                STW     *0
                JSX     DNEXT
                SAZ
                JMP     DCLR
                LDW     DKBLK
                SMB     K.DREAD
                JSX     K.DREAD
                SAZ                     ; a clean read?
                JMP     DFAIL

                JSX     DFIRST          ; and it must be what was written
DCMP            LDX     DP
                LDW     *0
                CMW     DVAL
                SEQ
                JMP     DFAIL
                JSX     DNEXT
                SAZ
                JMP     DCMP

                LDW     DRND
                CMW     DKRND
                SLS                     ; more rounds to go?
                JMP     DDONE
                JMP     DROUND

; A round failed: say so on the platter, where the pattern would be.
DFAIL           JSX     DFIRST
DFL             LDX     DP
                LDW     DKDEAD
                STW     *0
                JSX     DNEXT
                SAZ
                JMP     DFL
                LDW     DKBLK
                SMB     K.DWRIT
                JSX     K.DWRIT
DDONE           SMB     K.EXIT
                JMP     K.EXIT

; The walk over the buffer: DFIRST starts it with this round's first
; value, DNEXT steps the pointer and the value and answers zero once the
; sector's last word is behind it.
DFIRST          SUBR
                LDW     DID
                SLL     8
                ADD     DRND
                STW     DVAL
                LDW     DKBUF
                STW     DP
                LDW     DK47
                STW     DN
                EXIT    DFIRST
DNEXT           SUBR
                LDW     DVAL
                ADD     DK1
                STW     DVAL
                LDW     DP
                ADD     DK1
                STW     DP
                LDW     DN
                SUB     DK1
                STW     DN
                EXIT    DNEXT

; ---------------------------------------------------------------- data
DID             WORD    0               ; this task's job number
DRND            WORD    0               ; the round
DVAL            WORD    0               ; the walk: the value...
DP              WORD    0               ; ...where it goes or comes from...
DN              WORD    0               ; ...and how many are left
DBLK            WORD    0,47,DBUF       ; the transfer: sector, words, core
DBUF            RES     47
DK1             WORD    1
DK47            WORD    47
DKSEC           WORD    DSKSEC
DKRND           WORD    DSKRND
DKDEAD          WORD    X'DEAD'
DKBLK           WORD    DBLK
DKBUF           WORD    DBUF
                END     DSKT
