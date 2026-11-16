; vim: ts=8:sw=8:expandtab:
;
; REX's filesystem -- the second file of the kernel's deck, after rex.asm.
;
; Files are contiguous runs of sectors on disc unit 0 under a directory
; in sectors 1-8, which tools/fs703.py describes word for word and writes
; from the host; this is the side that runs on the machine, and the
; scripted session holds the two to each other.  In short: a five-word
; header, then 46 entries of eight words -- a name of six characters and
; a type of two, the first sector, the sector count, the bytes used in
; the last sector, and what the entry is.  The entries are in disc order
; and account for every data sector between them, a hole being an entry
; like any other, so there is no free map: a new file is given half of
; the largest hole as a file being written -- half, as RT-11 gave it, so
; that another can be begun meanwhile -- and closing it cuts it to its
; length and makes the rest a hole again.  A file being written
; that no channel holds -- its task ended, or the machine stopped -- is a
; hole to whoever looks next (FDNORM), which is all the recovery there is.
; A file takes the place of an older one of its name only when it is
; closed, and only whole: a channel that has refused a character refuses
; the rest, and closing it gives the sectors back and leaves the older
; file be.  A system file's name is nobody's to write under.
;
; The directory is kept in core, read once and written back whole after
; every change, one 376-word transfer each way.
;
; A task reads or writes a file a character at a time through one of
; four channels, each a sector buffer and its place in the file.  The
; whole of it runs behind the disc's claim: every entry a task has comes
; through KENTER (rex.asm), so one activation exists at a time and the
; routines here keep their work in static cells and call each other like
; any single-task code.  The shell, which runs the loader in its own
; task, makes the claim itself and calls FSFND.
;
; This page is above word X'4000', where a byte address is negative as a
; signed number.  Bytes are reached here through the index register,
; which is indifferent to that; what is never done is compare two byte
; addresses.  Positions are counted instead.

                ORG     X'7000'

FSDIRS          EQU     1               ; the directory's first sector...
FSDIRW          EQU     376             ; ...and its size, eight sectors of words
FSNENT          EQU     46              ; entries
FDENT           EQU     FDBUF+5         ; the first, past the header
FDEND           EQU     FDENT+FSNENT*DE.LEN

; A channel.
C.OWN           EQU     0               ; the node that has it; zero: free
C.MOD           EQU     1               ; 1 reading, 2 writing
C.SEC           EQU     2               ; the next sector to read, or the
                                        ; one the buffer will be written to
C.END           EQU     3               ; one past the file's, or the hole's, last
C.POS           EQU     4               ; the next byte in the buffer
C.LIM           EQU     5               ; reading: bytes in the buffer
C.LST           EQU     6               ; reading: bytes in the last sector
C.STA           EQU     7               ; writing: the hole's first sector
C.ERR           EQU     8               ; writing: what a character was refused
                                        ; with, which the rest and the close get
C.BUF           EQU     9               ; the sector, 47 words
C.LEN           EQU     C.BUF+47
CHN             EQU     4               ; channels

; ---------------------------------------------------------------- the directory
; The directory into core, if it is not there already: zero, or why not.
; What it finds being written it makes a hole.
FDLOAD          SUBR
                LDW     FDVLD
                SAZ
                JMP     FDLOK
                CLR
                JSX     FDXFR
                SAZ
                JMP     FDLDE
                LDW     FDBUF
                CMW     FKMAG0
                SEQ
                JMP     FDLND
                LDW     FDBUF+1
                CMW     FKMAG1
                SEQ
                JMP     FDLND
                LDW     FK1
                STW     FDVLD
                JSX     FDNORM
FDLOK           CLR
                EXIT    FDLOAD
FDLDE           LDW     FKEDSC
                EXIT    FDLOAD
FDLND           LDW     FKENDR
                EXIT    FDLOAD

; The directory back to the disc: zero, or E.DISC with the copy in core
; no longer trusted.
FDSAVE          SUBR
                LDW     FK1
                JSX     FDXFR
                SAZ
                JMP     FDSBAD
                EXIT    FDSAVE
FDSBAD          CLR
                STW     FDVLD
                LDW     FKEDSC
                EXIT    FDSAVE

; The directory's transfer: ACR zero to read it, else to write it.
FDXFR           SUBR
                SMB     DXOP
                STW     DXOP
                LDW     FKDIRS
                SMB     DXSEC
                STW     DXSEC
                LDW     FKDIRW
                SMB     DXCNT
                STW     DXCNT
                LDW     FKDBUF
                SMB     DXBUF
                STW     DXBUF
                SMB     DXFER
                JSX     DXFER
                EXIT    FDXFR

; The file whose four words of name and type FNPTR addresses: its entry,
; or zero.  Only a finished file answers to its name.
FDFIND          SUBR
                LDW     FKENT
FDFL            STW     FDE
                CMW     FKEND
                SLS                     ; inside the table?
                JMP     FDFNO
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ                     ; an unused entry ends it
                JMP     FDF1
                JMP     FDFNO
FDF1            CMW     FKFILE
                SEQ
                JMP     FDFNX
                LDW     *0
                STW     FDN0
                LDW     *1
                STW     FDN1
                LDW     *2
                STW     FDN2
                LDW     *3
                STW     FDN3
                LDX     FNPTR
                LDW     *0
                CMW     FDN0
                SEQ
                JMP     FDFNX
                LDW     *1
                CMW     FDN1
                SEQ
                JMP     FDFNX
                LDW     *2
                CMW     FDN2
                SEQ
                JMP     FDFNX
                LDW     *3
                CMW     FDN3
                SEQ
                JMP     FDFNX
                LDW     FDE
                EXIT    FDFIND
FDFNX           LDW     FDE
                ADD     FK8
                JMP     FDFL
FDFNO           CLR
                EXIT    FDFIND

; Every file being written that no channel is writing becomes a hole, and
; the holes are tidied.
FDNORM          SUBR
                LDW     FKENT
FDNL            STW     FDE
                CMW     FKEND
                SLS
                JMP     FDNDN
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FDN1A
                JMP     FDNDN
FDN1A           CMW     FKTENT
                SEQ
                JMP     FDNNX
                LDW     *DE.SEC
                JSX     FCWHO           ; a channel writing from there?
                SAZ
                JMP     FDNNX
                LDW     FDE
                JSX     FDHOLE
FDNNX           LDW     FDE
                ADD     FK8
                JMP     FDNL
FDNDN           JSX     FDMERG
                EXIT    FDNORM

; Make the entry ACR addresses a hole: its sectors kept, the rest blank.
FDHOLE          SUBR
                CAX
                CLR
                STW     *0
                STW     *1
                STW     *2
                STW     *3
                STW     *DE.LST
                LDW     FKHOLE
                STW     *DE.FLG
                EXIT    FDHOLE

; Tidy the holes: one with no sectors goes, and two that touch become
; one.  The entry in hand is looked at again after either, since what
; follows it has moved up.
FDMERG          SUBR
                LDW     FKENT
FDML            STW     FDM
                CMW     FKEND
                SLS
                JMP     FDMDN
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FDM1
                JMP     FDMDN
FDM1            CMW     FKHOLE
                SEQ
                JMP     FDMNX
                LDW     *DE.CNT
                SAZ                     ; a hole of nothing?
                JMP     FDM2
                LDW     FDM
                JSX     FDDEL
                LDW     FDM
                JMP     FDML
FDM2            LDW     FDM             ; the one after it
                ADD     FK8
                STW     FDM2P
                CMW     FKEND
                SLS
                JMP     FDMNX
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                CMW     FKHOLE
                SEQ
                JMP     FDMNX
                LDW     *DE.CNT
                LDX     FDM
                ADD     *DE.CNT
                STW     *DE.CNT
                LDW     FDM2P
                JSX     FDDEL
                LDW     FDM
                JMP     FDML
FDMNX           LDW     FDM
                ADD     FK8
                JMP     FDML
FDMDN           EXIT    FDMERG

; Take the entry ACR addresses out: everything after it moves up one, and
; the last entry is blank.
FDDEL           SUBR
                STW     FDD
FDDL            LDW     FDD
                ADD     FK8
                CMW     FKEND
                SLS                     ; anything left to bring up?
                JMP     FDDZ
                CAX
                LDW     *0
                LDX     FDD
                STW     *0
                LDW     FDD
                ADD     FK1
                STW     FDD
                JMP     FDDL
FDDZ            LDW     FDD
                CMW     FKEND
                SLS
                JMP     FDDX
                CAX
                CLR
                STW     *0
                LDW     FDD
                ADD     FK1
                STW     FDD
                JMP     FDDZ
FDDX            EXIT    FDDEL

; Open a blank entry where ACR addresses: that entry and everything after
; it moves down one.  The last entry falls off the end, so the caller has
; seen that it was unused (FDROOM).
FDINS           SUBR
                STW     FDI
                LDW     FKEND
                SUB     FK9
                STW     FDD             ; from the last word that survives
FDIL            LDW     FDD
                CMW     FDI
                SLS                     ; below the opening: done
                JMP     FDI1
                JMP     FDIZ
FDI1            LDX     FDD
                LDW     *0
                STW     *8
                LDW     FDD
                SUB     FK1
                STW     FDD
                JMP     FDIL
FDIZ            LDX     FDI
                CLR
                STW     *0
                STW     *1
                STW     *2
                STW     *3
                STW     *4
                STW     *5
                STW     *6
                STW     *7
                EXIT    FDINS

; How many entries are unused.
FDROOM          SUBR
                CLR
                STW     FDR
                LDW     FKENT
FDRL            STW     FDE
                CMW     FKEND
                SLS
                JMP     FDRDN
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FDRNX
                LDW     FDR
                ADD     FK1
                STW     FDR
FDRNX           LDW     FDE
                ADD     FK8
                JMP     FDRL
FDRDN           LDW     FDR
                EXIT    FDROOM

; The file being written from sector ACR: its entry, or zero.
FDTENT          SUBR
                STW     FDT
                LDW     FKENT
FDTL            STW     FDE
                CMW     FKEND
                SLS
                JMP     FDTNO
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FDT1
                JMP     FDTNO
FDT1            CMW     FKTENT
                SEQ
                JMP     FDTNX
                LDW     *DE.SEC
                CMW     FDT
                SEQ
                JMP     FDTNX
                LDW     FDE
                EXIT    FDTENT
FDTNX           LDW     FDE
                ADD     FK8
                JMP     FDTL
FDTNO           CLR
                EXIT    FDTENT

; ---------------------------------------------------------------- channels
; The channel whose number is in ACR, if it is the running task's: its
; address, left in FCH as well, or zero.
FCHK            SUBR
                SUB     FK1
                STW     FCT
                AND     FKNCHN
                SAZ                     ; 1 to 4?
                JMP     FCKNO
                LDW     FCT
                ADD     FKCHA
                CAX
                LDW     *0
                STW     FCH
                CAX
                LDW     *C.OWN
                SMB     CURT
                CMW     CURT
                SEQ
                JMP     FCKNO
                LDW     FCH
                EXIT    FCHK
FCKNO           CLR
                EXIT    FCHK

; Is a channel writing the file that starts at sector ACR?  And, with no
; sector to match -- ACR minus one -- how many channels are writing at
; all: FCWN is left holding the count either way.
FCWHO           SUBR
                STW     FCT
                CLR
                STW     FCWN
                STW     FCWF
                LDW     FKCHA
                STW     FCWP
                LDW     FK4
                STW     FCWC
FCWL            LDX     FCWP
                LDW     *0
                CAX
                LDW     *C.OWN
                SAZ
                JMP     FCW1
                JMP     FCWNX
FCW1            LDW     *C.MOD
                CMW     FK2
                SEQ
                JMP     FCWNX
                LDW     FCWN
                ADD     FK1
                STW     FCWN
                LDW     *C.STA
                CMW     FCT
                SEQ
                JMP     FCWNX
                LDW     FK1
                STW     FCWF
FCWNX           LDW     FCWP
                ADD     FK1
                STW     FCWP
                LDW     FCWC
                SUB     FK1
                STW     FCWC
                SAZ
                JMP     FCWL
                LDW     FCWF
                EXIT    FCWHO

; Blank the buffer of the channel in FCH, so that what a short last
; sector is padded with is zeros.
FCCLR           SUBR
                LDW     FCH
                ADD     FKCBUF
                STW     FCP
                LDW     FK47
                STW     FCT
FCCL            LDX     FCP
                CLR
                STW     *0
                LDW     FCP
                ADD     FK1
                STW     FCP
                LDW     FCT
                SUB     FK1
                STW     FCT
                SAZ
                JMP     FCCL
                EXIT    FCCLR

; Write the buffer of the channel in FCH to its sector and move on to the
; next: zero, or E.DISC.
FCFLSH          SUBR
                LDW     FCH
                ADD     FKCBUF
                SMB     DXBUF
                STW     DXBUF
                LDX     FCH
                LDW     *C.SEC
                SMB     DWR1
                JSX     DWR1
                SAZ
                JMP     FCFBAD
                LDX     FCH
                LDW     *C.SEC
                ADD     FK1
                STW     *C.SEC
                CLR
                STW     *C.POS
                JSX     FCCLR
                CLR
                EXIT    FCFLSH
FCFBAD          LDW     FKEDSC
                EXIT    FCFLSH

; Let go of the channel in FCH.
FCFREE          SUBR
                LDX     FCH
                CLR
                STW     *C.OWN
                STW     *C.MOD
                EXIT    FCFREE

; A task is over: whatever channels it had are free.  Called by TEXIT
; under the mask, so it keeps to cells of its own -- a body here may be
; part way through for another task.  A file it was writing is left as it
; is in the directory, and FDNORM finds it.
FCREL           SUBR
                STW     FCRN
                LDW     FKCHA
                STW     FCRP
                LDW     FK4
                STW     FCRC
FCRL            LDX     FCRP
                LDW     *0
                CAX
                LDW     *C.OWN
                CMW     FCRN
                SEQ
                JMP     FCRNX
                CLR
                STW     *C.OWN
                STW     *C.MOD
FCRNX           LDW     FCRP
                ADD     FK1
                STW     FCRP
                LDW     FCRC
                SUB     FK1
                STW     FCRC
                SAZ
                JMP     FCRL
                EXIT    FCREL

; ---------------------------------------------------------------- the calls
; Each is the body of a vector entry, run by KENTER with the disc
; claimed: ACR is the caller's argument on the way in and the answer on
; the way out.  The vector in rex.asm states what each takes and gives.

; The shell's own way in, with the disc claimed by SHIOC: the file whose
; name ACR addresses, as its entry's address -- or zero for no such
; file, or an error, which is minus.
FSFND           SUBR
                STW     FNPTR
                JSX     FDLOAD
                SAZ
                JMP     FSFX
                JSX     FDFIND
FSFX            EXIT    FSFND

; K.LOOK
FBLOOK          SUBR
                STW     FNPTR
                JSX     FDLOAD
                SAZ
                JMP     FBLX
                JSX     FDFIND
                SAZ
                JMP     FBL1
                LDW     FKENOF
FBLX            EXIT    FBLOOK
FBL1            CAX
                LDW     *DE.SEC
                STW     FDN0
                LDW     *DE.CNT
                STW     FDN1
                LDW     *DE.LST
                STW     FDN2
                LDW     *DE.FLG
                STW     FDN3
                LDX     FNPTR
                LDW     FDN0
                STW     *DE.SEC
                LDW     FDN1
                STW     *DE.CNT
                LDW     FDN2
                STW     *DE.LST
                LDW     FDN3
                STW     *DE.FLG
                CLR
                EXIT    FBLOOK

; K.OPEN
FBOPEN          SUBR
                STW     FNPTR
                CAX
                LDW     *0
                SAZ                     ; a name of nothing is no file's
                JMP     FBON
                LDW     FKENOF
                EXIT    FBOPEN
FBON            LDW     FKCHA           ; a channel nobody has
                STW     FOP
                LDW     FK4
                STW     FOC
FBOCL           LDX     FOP
                LDW     *0
                STW     FCH
                CAX
                LDW     *C.OWN
                SAZ
                JMP     FBOCN
                JMP     FBOGOT
FBOCN           LDW     FOP
                ADD     FK1
                STW     FOP
                LDW     FOC
                SUB     FK1
                STW     FOC
                SAZ
                JMP     FBOCL
                LDW     FKECHN
                EXIT    FBOPEN
FBOGOT          LDW     FK5             ; its number, 1 to 4
                SUB     FOC
                STW     FON
                JSX     FDLOAD
                SAZ
                JMP     FBOX
                LDX     FNPTR
                LDW     *4
                SAZ                     ; to read?
                JMP     FBOCR

                JSX     FDFIND
                SAZ
                JMP     FBOR1
                LDW     FKENOF
FBOX            EXIT    FBOPEN
FBOR1           CAX
                LDW     *DE.SEC
                STW     FDN0
                ADD     *DE.CNT
                STW     FDN1
                LDW     *DE.LST
                STW     FDN2
                LDX     FCH
                LDW     FDN0
                STW     *C.SEC
                LDW     FDN1
                STW     *C.END
                LDW     FDN2
                STW     *C.LST
                CLR
                STW     *C.POS
                STW     *C.LIM
                LDW     FK1
                STW     *C.MOD
                JMP     FBOOK

; To write: half of the largest hole, under the new name -- half, so
; that a second file can be begun before the first is closed, which with
; the whole hole taken it could not be when the free sectors are all in
; one piece.  Halving the hole takes an entry now, and closing the file
; may take another for what is left of its half, so there must be two
; unused entries more than there are files being written.  Closing it
; drops an older file of the name, so the name may not be a system file's.
FBOCR           JSX     FDFIND
                SAZ                     ; an older one?
                JMP     FBOC1
                JMP     FBOC2
FBOC1           CAX
                LDW     *DE.FLG
                AND     FKSYS
                SAZ
                JMP     FBOPR
FBOC2           JSX     FDNORM
                LDW     FKM1
                JSX     FCWHO
                LDW     FCWN
                ADD     FK1
                STW     FDN0
                JSX     FDROOM
                CMW     FDN0
                SGR
                JMP     FBODF
                CLR
                STW     FOB             ; the best hole so far, and its size
                STW     FOBN
                LDW     FKENT
FBOHL           STW     FDE
                CMW     FKEND
                SLS
                JMP     FBOHD
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FBOH1
                JMP     FBOHD
FBOH1           CMW     FKHOLE
                SEQ
                JMP     FBOHN
                LDW     *DE.CNT
                CMW     FOBN
                SGR                     ; bigger than the best?
                JMP     FBOHN
                STW     FOBN
                LDW     FDE
                STW     FOB
FBOHN           LDW     FDE
                ADD     FK8
                JMP     FBOHL
FBOHD           LDW     FOB
                SAZ
                JMP     FBOH2
                LDW     FKEFUL
                EXIT    FBOPEN
FBOH2           LDW     FOBN
                SRL     1
                SAZ                     ; a hole of one sector goes whole
                JMP     FBOHV
                JMP     FBOH3
FBOHV           STW     FDN0            ; the file's half; the rest follows
                LDX     FOB             ; it as a hole of its own
                LDW     *DE.SEC
                ADD     FDN0
                STW     FDN1
                LDW     FOBN
                SUB     FDN0
                STW     FDN2
                LDW     FOB
                ADD     FK8
                STW     FDN3
                JSX     FDINS
                LDX     FDN3
                LDW     FDN1
                STW     *DE.SEC
                LDW     FDN2
                STW     *DE.CNT
                LDW     FKHOLE
                STW     *DE.FLG
                LDX     FOB
                LDW     FDN0
                STW     *DE.CNT
FBOH3           LDX     FNPTR
                LDW     *0
                STW     FDN0
                LDW     *1
                STW     FDN1
                LDW     *2
                STW     FDN2
                LDW     *3
                STW     FDN3
                LDX     FOB
                LDW     FDN0
                STW     *0
                LDW     FDN1
                STW     *1
                LDW     FDN2
                STW     *2
                LDW     FDN3
                STW     *3
                CLR
                STW     *DE.LST
                LDW     FKTENT
                STW     *DE.FLG
                LDW     *DE.SEC
                STW     FDN0
                ADD     *DE.CNT
                STW     FDN1
                LDX     FCH
                LDW     FDN0
                STW     *C.SEC
                STW     *C.STA
                LDW     FDN1
                STW     *C.END
                CLR
                STW     *C.POS
                STW     *C.ERR
                JSX     FCCLR
                LDX     FCH             ; owned before the directory goes
                LDW     FK2             ; out, or FDNORM would take the
                STW     *C.MOD          ; file for one nobody is writing
                SMB     CURT
                LDW     CURT
                STW     *C.OWN
                JSX     FDSAVE
                SAZ
                JMP     FBOSV
                LDW     FON
                EXIT    FBOPEN
FBOSV           STW     FDN0
                JSX     FCFREE
                LDW     FDN0
                EXIT    FBOPEN
FBODF           LDW     FKEDFL
                EXIT    FBOPEN
FBOPR           LDW     FKEPRT
                EXIT    FBOPEN
FBOOK           LDX     FCH
                SMB     CURT
                LDW     CURT
                STW     *C.OWN
                LDW     FON
                EXIT    FBOPEN

; K.GETC
FBGETC          SUBR
                JSX     FCHK
                SAZ
                JMP     FBG1
FBGBAD          LDW     FKECHN
                EXIT    FBGETC
FBG1            CAX
                LDW     *C.MOD
                CMW     FK1
                SEQ
                JMP     FBGBAD
FBG2            LDX     FCH
                LDW     *C.POS
                CMW     *C.LIM
                SEQ                     ; the buffer used up?
                JMP     FBGHAV
                LDW     *C.SEC
                CMW     *C.END
                SNE                     ; and the file with it?
                JMP     FBGEOF
                LDW     FCH
                ADD     FKCBUF
                SMB     DXBUF
                STW     DXBUF
                LDX     FCH
                LDW     *C.SEC
                SMB     DRD1
                JSX     DRD1
                SAZ
                JMP     FBGDE
                LDX     FCH
                LDW     *C.SEC
                ADD     FK1
                STW     *C.SEC
                CMW     *C.END
                SEQ                     ; that was the last sector?
                JMP     FBGFUL
                LDW     *C.LST
                JMP     FBGLIM
FBGFUL          LDW     FK94
FBGLIM          STW     *C.LIM
                CLR
                STW     *C.POS
                JMP     FBG2
FBGHAV          STW     FDN0            ; the byte at POS
                ADD     FK1
                STW     *C.POS
                LDW     FCH
                ADD     FKCBUF
                SLL     1
                ADD     FDN0
                CAX
                CLR
                LDB     *0
                EXIT    FBGETC
FBGEOF          LDW     FKM1
                EXIT    FBGETC
FBGDE           LDW     FKEDSC
                EXIT    FBGETC

; K.PUTC
FBPUTC          SUBR
                STW     FPC
                SRL     8
                JSX     FCHK
                SAZ
                JMP     FBP1
FBPBAD          LDW     FKECHN
                EXIT    FBPUTC
FBP1            CAX
                LDW     *C.MOD
                CMW     FK2
                SEQ
                JMP     FBPBAD
                LDW     *C.ERR
                SAZ                     ; refused one already?  the same again
                JMP     FBPX
                LDW     *C.POS
                CMW     FK94
                SEQ                     ; a full sector waiting to go?
                JMP     FBPST
                JSX     FCFLSH
                SAZ
                JMP     FBPERR
FBPST           LDX     FCH
                LDW     *C.SEC
                CMW     *C.END
                SNE                     ; the hole all used?
                JMP     FBPFUL
                LDW     *C.POS
                STW     FDN0
                ADD     FK1
                STW     *C.POS
                LDW     FCH
                ADD     FKCBUF
                SLL     1
                ADD     FDN0
                CAX
                LDW     FPC
                STB     *0
                CLR
FBPX            EXIT    FBPUTC
FBPFUL          LDW     FKEFUL
FBPERR          LDX     FCH             ; kept, for the characters after this
                STW     *C.ERR          ; one and for K.CLOSE
                EXIT    FBPUTC

; K.CLOSE
FBCLOS          SUBR
                JSX     FCHK
                SAZ
                JMP     FBC1
                LDW     FKECHN
                EXIT    FBCLOS
FBC1            CAX
                LDW     *C.MOD
                CMW     FK2
                SEQ                     ; a reader just lets go
                JMP     FBCRD
                LDW     *C.ERR
                SAZ                     ; a character refused?
                JMP     FBCBAD
                LDW     *C.POS
                SAZ                     ; part of a sector in the buffer?
                JMP     FBCW
                LDW     *C.SEC          ; no: the last sector written was a
                SUB     *C.STA          ; full one, if there was one at all
                SAZ
                JMP     FBCN1
                JMP     FBCN2
FBCN1           LDW     FK94
FBCN2           STW     FCL
                JMP     FBCD
FBCW            STW     FCL
                JSX     FCFLSH
                SAZ
                JMP     FBCERR
FBCD            LDX     FCH
                LDW     *C.SEC
                SUB     *C.STA
                STW     FCN             ; sectors written
                JSX     FDLOAD
                SAZ
                JMP     FBCERR
                LDX     FCH
                LDW     *C.STA
                JSX     FDTENT
                SAZ
                JMP     FBCE1
                LDW     FKEDSC
                JMP     FBCERR
FBCE1           STW     FCE
                STW     FNPTR           ; an entry begins with its name
                JSX     FDFIND
                SAZ                     ; an older file of the same name?
                JSX     FDHOLE
                LDX     FCE
                LDW     *DE.CNT
                SUB     FCN
                STW     FCR             ; what is left of the hole
                LDW     FCN
                STW     *DE.CNT
                LDW     FCL
                STW     *DE.LST
                LDW     FKFILE
                STW     *DE.FLG
                LDW     FCR
                SAZ
                JMP     FBCR1
                JMP     FBCM
FBCR1           LDW     *DE.SEC
                ADD     FCN
                STW     FDN0
                LDW     FCE
                ADD     FK8
                STW     FDN1
                JSX     FDINS
                LDX     FDN1
                LDW     FDN0
                STW     *DE.SEC
                LDW     FCR
                STW     *DE.CNT
                LDW     FKHOLE
                STW     *DE.FLG
FBCM            JSX     FDMERG
                JSX     FDSAVE
FBCERR          STW     FDN0
                JSX     FCFREE
                LDW     FDN0
                EXIT    FBCLOS
FBCRD           JSX     FCFREE
                CLR
                EXIT    FBCLOS
; A file that did not all go in is not a file: with its channel let go
; nobody is writing it, which is what FDNORM makes a hole of.  The answer
; is what the character was refused with.
FBCBAD          STW     FCL
                JSX     FCFREE
                JSX     FDLOAD
                SAZ
                JMP     FBCBX
                JSX     FDNORM
                JSX     FDSAVE
                SAZ
                JMP     FBCBX
                LDW     FCL
FBCBX           EXIT    FBCLOS

; K.DELET
FBDELE          SUBR
                STW     FNPTR
                JSX     FDLOAD
                SAZ
                JMP     FBDX
                JSX     FDFIND
                SAZ
                JMP     FBD1
                LDW     FKENOF
FBDX            EXIT    FBDELE
FBD1            STW     FCE
                CAX
                LDW     *DE.FLG
                AND     FKSYS
                SAZ
                JMP     FBDPR
                LDW     FCE
                JSX     FDHOLE
                JSX     FDMERG
                JSX     FDSAVE
                EXIT    FBDELE
FBDPR           LDW     FKEPRT
                EXIT    FBDELE

; K.RENAM
FBRENA          SUBR
                STW     FRP
                ADD     FK4
                STW     FNPTR
                JSX     FDLOAD
                SAZ
                JMP     FBRX
                JSX     FDFIND
                SAZ                     ; the new name in use?
                JMP     FBREX
                LDW     FRP
                STW     FNPTR
                JSX     FDFIND
                SAZ
                JMP     FBR1
                LDW     FKENOF
FBRX            EXIT    FBRENA
FBR1            STW     FCE
                CAX
                LDW     *DE.FLG
                AND     FKSYS
                SAZ
                JMP     FBRPR
                LDX     FRP
                LDW     *4
                STW     FDN0
                LDW     *5
                STW     FDN1
                LDW     *6
                STW     FDN2
                LDW     *7
                STW     FDN3
                LDX     FCE
                LDW     FDN0
                STW     *0
                LDW     FDN1
                STW     *1
                LDW     FDN2
                STW     *2
                LDW     FDN3
                STW     *3
                JSX     FDSAVE
                EXIT    FBRENA
FBREX           LDW     FKEEXS
                EXIT    FBRENA
FBRPR           LDW     FKEPRT
                EXIT    FBRENA

; K.SQUSH: every file that has a hole in front of it moves down over the
; hole, a sector at a time through the first channel's buffer, until the
; free sectors are one hole at the end.  A system file stays where it is,
; and so does a hole in front of one.  No channel may be open: a channel
; knows its file by sector.  The directory goes back to the disc after
; each file has moved, so what the disc says is true between moves; a
; machine stopped part way through one loses that file if the hole was
; smaller than it.
FBSQSH          SUBR
                JSX     FDLOAD
                SAZ
                JMP     FBSX
                LDW     FKCHA
                STW     FCWP
                LDW     FK4
                STW     FCWC
FBSC            LDX     FCWP
                LDW     *0
                CAX
                LDW     *C.OWN
                SAZ
                JMP     FBSBSY
                LDW     FCWP
                ADD     FK1
                STW     FCWP
                LDW     FCWC
                SUB     FK1
                STW     FCWC
                SAZ
                JMP     FBSC
                JSX     FDNORM
                LDW     FKENT
FBSL            STW     FSH             ; a hole, perhaps
                CMW     FKEND
                SLS
                JMP     FBSDN
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                SAZ
                JMP     FBS1
                JMP     FBSDN
FBS1            CMW     FKHOLE
                SEQ
                JMP     FBSNX
                LDW     *DE.SEC
                STW     FSD
                STW     FSHS
                LDW     *DE.CNT
                STW     FSHN
                LDW     FSH             ; and what follows it
                ADD     FK8
                STW     FSF
                CMW     FKEND
                SLS
                JMP     FBSDN
                CAX
                LDW     *DE.FLG
                AND     FK0FF
                CMW     FKFILE
                SEQ
                JMP     FBSNX
                LDW     *DE.FLG
                AND     FKSYS
                SAZ
                JMP     FBSNX
                LDW     *DE.SEC
                STW     FSS
                LDW     *DE.CNT
                STW     FSN
                STW     FSFN
FBSM            LDW     FSN             ; the sectors, lowest first, so the
                SAZ                     ; move may overlap itself
                JMP     FBSM1
                JMP     FBSMD
FBSM1           LDW     FKSBUF
                SMB     DXBUF
                STW     DXBUF
                LDW     FSS
                SMB     DRD1
                JSX     DRD1
                SAZ
                JMP     FBSERR
                LDW     FSD
                SMB     DWR1
                JSX     DWR1
                SAZ
                JMP     FBSERR
                LDW     FSS
                ADD     FK1
                STW     FSS
                LDW     FSD
                ADD     FK1
                STW     FSD
                LDW     FSN
                SUB     FK1
                STW     FSN
                JMP     FBSM
FBSMD           LDW     FK8             ; the two entries change places: the
                STW     FSN             ; file's goes into the hole's slot...
FBSE            LDX     FSF
                LDW     *0
                LDX     FSH
                STW     *0
                LDW     FSF
                ADD     FK1
                STW     FSF
                LDW     FSH
                ADD     FK1
                STW     FSH
                LDW     FSN
                SUB     FK1
                STW     FSN
                SAZ
                JMP     FBSE
                LDW     FSH             ; (both cursors are a slot on: FSH is
                SUB     FK8             ; the hole's new slot, the file's is
                CAX                     ; the one before it)
                LDW     FSHS
                STW     *DE.SEC         ; ...at the hole's sector...
                LDW     FSH
                JSX     FDHOLE          ; ...and the hole follows it
                LDX     FSH
                LDW     FSHS
                ADD     FSFN
                STW     *DE.SEC
                LDW     FSHN
                STW     *DE.CNT
                JSX     FDMERG
                JSX     FDSAVE
                SAZ
                JMP     FBSX
                LDW     FSH             ; on from the hole, which may have
                JMP     FBSL            ; another file behind it
FBSNX           LDW     FSH
                ADD     FK8
                JMP     FBSL
FBSDN           CLR
FBSX            EXIT    FBSQSH
FBSBSY          LDW     FKECHN
                EXIT    FBSQSH
FBSERR          LDW     FKEDSC
                EXIT    FBSQSH

; ---------------------------------------------------------------- data
FDVLD           WORD    0               ; the directory is in core
FNPTR           WORD    0               ; the name a search is for
FDE             WORD    0               ; a walk's entry
FDN0            WORD    0               ; four words in hand
FDN1            WORD    0
FDN2            WORD    0
FDN3            WORD    0
FDM             WORD    0               ; FDMERG's entry, and the next
FDM2P           WORD    0
FDD             WORD    0               ; FDDEL's and FDINS's cursor
FDI             WORD    0               ; FDINS's opening
FDR             WORD    0               ; FDROOM's count
FDT             WORD    0               ; FDTENT's sector
FCH             WORD    0               ; the channel in hand
FCT             WORD    0               ; channel scratch
FCP             WORD    0
FCWN            WORD    0               ; FCWHO's count, its answer,
FCWF            WORD    0
FCWP            WORD    0               ; its cursor and its countdown
FCWC            WORD    0
FCRN            WORD    0               ; FCREL's node, cursor and countdown
FCRP            WORD    0
FCRC            WORD    0
FOP             WORD    0               ; FBOPEN's channel cursor, countdown
FOC             WORD    0
FON             WORD    0               ; and the channel's number;
FOB             WORD    0               ; the best hole, and its size
FOBN            WORD    0
FPC             WORD    0               ; FBPUTC's argument
FCL             WORD    0               ; FBCLOS's last-sector bytes,
FCN             WORD    0               ; its sector count,
FCE             WORD    0               ; its entry
FCR             WORD    0               ; and what is left of the hole
FRP             WORD    0               ; FBRENA's argument
FSH             WORD    0               ; FBSQSH's hole, and the file after it
FSF             WORD    0
FSHS            WORD    0               ; the hole's sector and size
FSHN            WORD    0
FSS             WORD    0               ; the move: from, to, and how many
FSD             WORD    0
FSN             WORD    0
FSFN            WORD    0               ; the file's sectors

FK1             WORD    1
FK2             WORD    2
FK4             WORD    4
FK5             WORD    5
FK8             WORD    DE.LEN
FK9             WORD    DE.LEN+1
FK47            WORD    47
FK94            WORD    94
FK0FF           WORD    X'00FF'
FKM1            WORD    X'FFFF'
FKNCHN          WORD    X'FFFC'         ; what a channel number less one may not have set
FKHOLE          WORD    DK.HOLE
FKFILE          WORD    DK.FILE
FKTENT          WORD    DK.TENT
FKSYS           WORD    DF.SYS
FKMAG0          WORD    'RE'
FKMAG1          WORD    'XF'
FKDIRS          WORD    FSDIRS
FKDIRW          WORD    FSDIRW
FKDBUF          WORD    FDBUF
FKENT           WORD    FDENT
FKEND           WORD    FDEND
FKCBUF          WORD    C.BUF
FKSBUF          WORD    FCH1+C.BUF      ; FBSQSH's sector: the first channel's
FKCHA           WORD    FCHTAB
FKENOF          WORD    E.NOF
FKECHN          WORD    E.CHN
FKEDFL          WORD    E.DFUL
FKEFUL          WORD    E.FULL
FKEDSC          WORD    E.DISC
FKENDR          WORD    E.NDIR
FKEPRT          WORD    E.PROT
FKEEXS          WORD    E.EXST

FCHTAB          WORD    FCH1,FCH2,FCH3,FCH4
FCH1            RES     C.LEN
FCH2            RES     C.LEN
FCH3            RES     C.LEN
FCH4            RES     C.LEN
FDBUF           RES     FSDIRW          ; the directory
FSEND           EQU     $
