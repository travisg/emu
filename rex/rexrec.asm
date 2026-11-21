; vim: ts=8:sw=8:expandtab:
;
; The loader's record reader -- the last file of the kernel's deck, after
; rexname.asm, in the rest of its page.  Every shell's loader reads a
; module's object text through it (K.LDREC), a record at a time: one
; copy for them all, since a loader holds the disc for the whole of a
; load and so only one runs it at once.  Placing and relocating what it
; reads stays the shell's.
;
; A record is a sector, 47 words: a zero marker byte, 92 bytes of text,
; and the folded byte sum -- (sum >> 8) + sum -- of everything before it.
; The first record comes off the disc into LDBUF on its own, since its
; SIZE codes are what take the module's blocks, and nothing else should
; be in the pool's way when they do.  Then a temporary block is asked of
; the pool for the rest of the file -- anywhere, never waiting (ALLOCT),
; and half as much while that fails -- and the rest comes in one transfer,
; or as many as the block takes; with no block to be had at all, a
; sector at a time through LDBUF, as the first.  The block may lie above
; word X'4000', so no byte address is compared here or in the shell's
; walk of a record: what is left of one is counted instead.
;
; ACR is zero for the next record, the address of three words -- the
; unit, the first sector, the count of sectors -- to begin a file with
; its first record, or one when the load is over, to give the block back.
; The answer is zero with LRBP the byte address of the record's first
; byte of text, of which there are LRKTXT; one for a record that fails
; its marker or its sum; two when the file has no more; minus when the
; disc says no.

LRKTXT          EQU     92              ; a record's bytes of text

LDREC           SUBR
                STW     LRARG
                CMW     =1
                JEQ     .CLS            ; the load over?
                LDW     LRARG
                JNZ     .BEG            ; a file to begin?
                JMP     .NXT
.BEG            CAX
                LDW     *0
                STW     LRUNI
                LDW     *1
                STW     LRSEC
                LDW     *2
                STW     LRNSEC
                CLR
                STW     LRTMP
                STW     LRTN
                STW     LRST
.NXT            LDW     LRTN
                JNZ     .VER            ; a record read and not yet handed over?
                LDW     LRNSEC
                JNZ     .MOR            ; any more on the disc?
                LDW     =2
                EXIT    LDREC
.MOR            LDW     LRST
                JNZ     .ST1
                LDW     =1              ; the first: through LDBUF
                STW     LRST
                JMP     .ONE
.ST1            CMW     =1
                JNE     .FIL            ; the second: the block, once
                LDW     =2
                STW     LRST
                LDW     LRNSEC          ; for all the rest, or half of that
.TRY            STW     LRCAP
                JNZ     .TR1
                JMP     .FIL            ; nothing to be had
.TR1            MPY     =K47
                CALL    ALLOCT
                JNZ     .GOT
                LDW     LRCAP
                SRL     1
                JMP     .TRY
.GOT            STW     LRTMP
.FIL            LDW     LRTMP
                JNZ     .BLK            ; a block to fill?
.ONE            LDW     =LDBUF
                STW     LRREC
                LDW     =1
                JMP     .RD
.BLK            STW     LRREC
                LDW     LRNSEC          ; as much as is left, or it holds
                CMW     LRCAP
                SLS
                LDW     LRCAP
.RD             STW     LRTN
                MPY     =K47
                SMB     DXCNT
                STW     DXCNT
                LDW     LRREC
                SMB     DXBUF
                STW     DXBUF
                LDW     LRSEC
                SMB     DXSEC
                STW     DXSEC
                ADD     LRTN
                STW     LRSEC
                LDW     LRNSEC
                SUB     LRTN
                STW     LRNSEC
                CLR
                SMB     DXOP
                STW     DXOP
                LDW     LRUNI           ; last, since a swapper that borrows
                SMB     DXUNI           ; the disc sets its own
                STW     DXUNI
                CALL    DXFER
                JNZ     .EDI            ; the disc said no?

; The next record read: its marker, and its sum, a word at a time with the
; checksum byte taken back out.
.VER            LDW     LRREC
                STW     LRCUR
                ADD     =K47
                STW     LRREC
                LDW     LRTN
                SUB     =1
                STW     LRTN
                LDX     LRCUR
                LDW     *0
                AND     =X'FF00'
                JNZ     .ECK            ; the marker?
                STW     LRSUM
                LDW     =K47
                STW     LRCNT
.CK             LDW     *0
                SRL     8
                ADD     LRSUM
                STW     LRSUM
                LDW     *0
                AND     =X'00FF'
                ADD     LRSUM
                STW     LRSUM
                CXA
                ADD     =1
                CAX
                LDW     LRCNT
                SUB     =1
                STW     LRCNT
                JNZ     .CK
                LDX     LRCUR
                LDW     *K47-1
                AND     =X'00FF'
                STW     LRCK            ; the checksum byte
                LDW     LRSUM
                SUB     LRCK
                STW     LRSUM
                SRL     8
                ADD     LRSUM
                AND     =X'00FF'
                CMW     LRCK
                JNE     .ECK            ; and it agrees?
                LDW     LRCUR
                SLL     1
                ADD     =1
                STW     LRBP
                CLR
                EXIT    LDREC
.ECK            LDW     =1
                EXIT    LDREC
.EDI            LDW     =X'FFFF'
                EXIT    LDREC
.CLS            LDW     LRTMP
                JNZ     .CL1            ; a block to give back?
                EXIT    LDREC
.CL1            CALL    FREE
                CLR
                STW     LRTMP
                EXIT    LDREC

LRARG           WORD    0               ; the call's argument
LRUNI           WORD    0               ; the file: its unit, its next sector
LRSEC           WORD    0               ; on the disc, and how many are left
LRNSEC          WORD    0
LRST            WORD    0               ; records begun: 0, 1, then 2 for good
LRTMP           WORD    0               ; the temporary block, or zero
LRCAP           WORD    0               ; the sectors it holds
LRTN            WORD    0               ; records read and not handed over yet
LRREC           WORD    0               ; the next of them
LRCUR           WORD    0               ; the record being handed over
LRBP            WORD    0               ; and its first byte of text
LRSUM           WORD    0               ; its byte sum,
LRCNT           WORD    0               ; the words left to add,
LRCK            WORD    0               ; and the checksum byte
                LPOOL
LREND           EQU     $

                TRUE    LREND>NARB      ; into the node arena: a negative RES,
                RES     NARB-LREND      ; which the assembler refuses
                ENDC
