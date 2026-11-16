; vim: ts=8:sw=8:expandtab:
;
; REX's boot sector: the disc controller's LOAD button reads this, sector
; 0 of unit 0, into words 0-46 and starts it at word 0 (706 UM 5-9.10.3),
; and it reads the kernel in off the same platter and starts that.
;
; The kernel's image begins at word 0, where this is, so this cannot
; stay: its first act is to have the controller read the same sector
; again, to BOOTAT, and to carry on from there.  BOOTAT is in what will
; be the pool, which is nobody's until the kernel's first instructions
; lay it out.  Nothing here is assembled for the second address and
; nothing needs to be: BOOTAT is the first word of a page, as word 0 is,
; so every reference in this sector -- an offset within the page it is
; executed in -- means the same word of the copy as it did of the
; original.  The one jump that changes pages selects its page first.
;
; The kernel is two runs of words with the whole pool between them, the
; shell's end and the filesystem's beginning, so it is kept as two: the
; file REX.SY holds the low part and then, from the next sector, the high
; part, and tools/mkdisc703.py --sys, which makes that file, writes where
; each is, how long, and where it goes into the six words at the end of
; this sector.  A transfer is polled to its end -- the controller's
; status reads zero when it is over -- since nothing is enabled yet to
; be interrupted with.

BOOTAT          EQU     X'6000'         ; where this goes to get out of the way

                ORG     0
BOOT            SGM                     ; flat addressing
                LDW     BKAT
                DOT     1,1             ; this sector again: the core address,
                CLR
                DOT     1,2             ; track 0, sector 0,
                LDW     BK47
                DOT     1,6             ; unit 0, one sector, read
BWT0            DIN     1,0
                SAZ                     ; over?
                JMP     BWT0
                SMB     BOOT2H
                JMP     BOOT2H          ; and on, in the copy

BOOT2           LDW     BTAB+2          ; the kernel's low part
                DOT     1,1
                LDW     BTAB
                DOT     1,2
                LDW     BTAB+1
                DOT     1,6
BWT1            DIN     1,0
                SAZ
                JMP     BWT1
                LDW     BTAB+5          ; and its high part
                DOT     1,1
                LDW     BTAB+3
                DOT     1,2
                LDW     BTAB+4
                DOT     1,6
BWT2            DIN     1,0
                SAZ
                JMP     BWT2
                SML     0               ; page 0, where the kernel starts
                JMP     X'40'

BOOT2H          EQU     BOOTAT+BOOT2
BKAT            WORD    BOOTAT
BK47            WORD    47

; What mkdisc703.py --sys fills in, having seen the mark in front of it:
; for each part of the kernel, the track and sector it starts at as the
; controller wants them, how many words it is, and the core address it
; goes to.
                ORG     40
BMARK           WORD    'RX'
BTAB            WORD    0,0,0
                WORD    0,0,0
                END     BOOT
