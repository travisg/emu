; vim: ts=8:sw=8:expandtab:
;
; HOG n -- n words of core, held and watched.
;
; A test module, for the swapper: it takes a block of n words, fills it
; with a pattern that carries its job number, and then HOGRND times over
; -- ten seconds of it -- sleeps a third of a second and checks that
; every word is still what it wrote.  Four of these wanting more core than there is cannot all be
; in it at once, so each goes out to the disc and comes back between
; checks, to the same words, which in the meantime were another hog's.
; It prints nothing; its verdict is a sector of its own on the platter,
; HOGSEC plus its job number: every word X'600D' if the block was whole
; every time, X'DEAD' if it was not, X'0C0E' if there was no core for it
; at all.
;
; Word i of job j's block is j*256 + i.

HOGSEC          EQU     4200
HOGRND          EQU     30
HKGOOD          EQU     X'600D'         ; the verdicts: every check passed,
HKDEAD          EQU     X'DEAD'         ; a word was not what was put there,
HKNOC           EQU     X'0C0E'         ; and there was no core to be had

HOG             SMB     CURT
                LDX     CURT
                LDW     *T.ID
                STW     HID
                ADD     =HOGSEC
                STW     HBLK            ; the verdict's sector
                JSX     L.ARGS
                CLR                     ; n, in decimal, from the command tail
                STW     HN
HNL             LDX     L.AP
                CLR
                LDB     *0
                SUB     ='0'
                STW     HD
                SAP                     ; below '0', the tail's zero included
                JMP     HND
                CMW     =10
                SLS
                JMP     HND
                LDW     HN
                SLL     3
                ADD     HN
                ADD     HN
                ADD     HD
                STW     HN
                LDW     L.AP
                ADD     =1
                STW     L.AP
                JMP     HNL
HND             LDW     HN
                SAZ
                JMP     HGET
                JMP     L.EXIT          ; no size, no hog
HGET            SMB     K.ALLHI
                JSX     K.ALLHI
                SAZ
                JMP     HGOT
                LDW     =HKNOC
                JMP     HVERD
HGOT            STW     HB
                JSX     HFIRST          ; the pattern
HFL             LDX     HP
                LDW     HV
                STW     *0
                JSX     HNEXT
                SAZ
                JMP     HFL
                LDW     =HOGRND
                STW     HR
HRL             LDW     =20
                JSX     L.SLEEP
                JSX     HFIRST          ; and is it still there?
HVL             LDX     HP
                LDW     *0
                CMW     HV
                SEQ
                JMP     HBAD
                JSX     HNEXT
                SAZ
                JMP     HVL
                LDW     HR
                SUB     =1
                STW     HR
                SAZ
                JMP     HRL
                LDW     =HKGOOD
                JMP     HVERD
HBAD            LDW     =HKDEAD

; The verdict, a sector of it.
HVERD           STW     HV
                LDW     =HBUF
                STW     HP
                LDW     =47
                STW     HC
HWL             LDX     HP
                LDW     HV
                STW     *0
                LDW     HP
                ADD     =1
                STW     HP
                LDW     HC
                SUB     =1
                STW     HC
                SAZ
                JMP     HWL
                LDW     =HBLK
                SMB     K.DWRIT
                JSX     K.DWRIT
                JMP     L.EXIT

; The walk over the block: HFIRST starts it, HNEXT steps the pointer and
; the value and answers zero once the last word is behind it.
HFIRST          SUBR
                LDW     HID
                SLL     8
                STW     HV
                LDW     HB
                STW     HP
                LDW     HN
                STW     HC
                EXIT    HFIRST
HNEXT           SUBR
                LDW     HV
                ADD     =1
                STW     HV
                LDW     HP
                ADD     =1
                STW     HP
                LDW     HC
                SUB     =1
                STW     HC
                EXIT    HNEXT

; ---------------------------------------------------------------- data
HID             WORD    0               ; this task's job number
HN              WORD    0               ; words asked for
HD              WORD    0               ; a digit of that
HB              WORD    0               ; the block
HR              WORD    0               ; rounds to go
HV              WORD    0               ; the walk: the value,
HP              WORD    0               ; where it is,
HC              WORD    0               ; and how many are left
HBLK            WORD    0,47,HBUF       ; the verdict's transfer: sector, words, core
HBUF            RES     47
                END     HOG
