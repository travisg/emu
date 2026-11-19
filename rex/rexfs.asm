; vim: ts=8:sw=8:expandtab:
;
; REX's filesystem -- the file of the kernel's deck after shell.asm.
;
; Files are contiguous runs of sectors on a platter under a directory
; in sectors 1-8, which fs703.py describes word for word and writes
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
; The directory is kept in core, one platter's at a time, read when a
; call is for another unit than the one in core and written back whole
; after every change, one 376-word transfer each way.  Every call names
; its unit (FSUNIT): a name is four words and then one whose high byte is
; the unit, 0 to 3, or X'80' for no unit given -- a program, type RX, is
; then on unit 0 and any other file on the data unit, which is unit 1 if
; that has a directory and unit 0 if not.  A channel keeps its unit, and
; is told from another unit's only by it, sector numbers being every
; platter's own.
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
C.UNI           EQU     9               ; the unit
C.BUF           EQU     10              ; the sector, 47 words
C.LEN           EQU     C.BUF+47
CHN             EQU     4               ; channels

; ---------------------------------------------------------------- the directory
; Unit FSUNI's directory into core, if it is not there already: zero,
; or why not.  What it finds being written it makes a hole.
FDLOAD          SUBR
                LDW     FDUNIT
                CMW     FSUNI
                JEQ     .OK
                LDW     =X'FFFF'        ; nobody's until it is read
                STW     FDUNIT
                CLR
                JSX     FDXFR
                JNZ     .DE
                LDW     FDBUF
                CMW     ='RE'
                JNE     .ND
                LDW     FDBUF+1
                CMW     ='XF'
                JNE     .ND
                LDW     FSUNI
                STW     FDUNIT
                JSX     FDNORM
.OK             CLR
                EXIT    FDLOAD
.DE             LDW     =E.DISC
                EXIT    FDLOAD
.ND             LDW     =E.NDIR
                EXIT    FDLOAD

; The directory back to the disc: zero, or E.DISC with the copy in core
; no longer trusted.
FDSAVE          SUBR
                LDW     =1
                JSX     FDXFR
                JNZ     .BAD
                EXIT    FDSAVE
.BAD            LDW     =X'FFFF'
                STW     FDUNIT
                LDW     =E.DISC
                EXIT    FDSAVE

; The directory's transfer, on unit FSUNI: ACR zero to read it, else to
; write it.
FDXFR           SUBR
                SMB     DXOP
                STW     DXOP
                LDW     FSUNI
                SMB     DXUNI
                STW     DXUNI
                LDW     =FSDIRS
                SMB     DXSEC
                STW     DXSEC
                LDW     =FSDIRW
                SMB     DXCNT
                STW     DXCNT
                LDW     =FDBUF
                SMB     DXBUF
                STW     DXBUF
                CALL    DXFER
                EXIT    FDXFR

; The file whose four words of name and type FNPTR addresses: its entry,
; or zero.  Only a finished file answers to its name.
FDFIND          SUBR
                LDW     =FDENT
.FL             STW     FDE
                CMW     =FDEND
                JGE     .FNO            ; inside the table?
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .F1             ; an unused entry ends it
                JMP     .FNO
.F1             CMW     =DK.FILE
                JNE     .FNX
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
                JNE     .FNX
                LDW     *1
                CMW     FDN1
                JNE     .FNX
                LDW     *2
                CMW     FDN2
                JNE     .FNX
                LDW     *3
                CMW     FDN3
                JNE     .FNX
                LDW     FDE
                EXIT    FDFIND
.FNX            LDW     FDE
                ADD     =DE.LEN
                JMP     .FL
.FNO            CLR
                EXIT    FDFIND

; Every file being written that no channel is writing becomes a hole, and
; the holes are tidied.
FDNORM          SUBR
                LDW     =FDENT
.NL             STW     FDE
                CMW     =FDEND
                JGE     .NDN
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .N1A
                JMP     .NDN
.N1A            CMW     =DK.TENT
                JNE     .NNX
                LDW     *DE.SEC
                JSX     FCWHO           ; a channel writing from there?
                JNZ     .NNX
                LDW     FDE
                JSX     FDHOLE
.NNX            LDW     FDE
                ADD     =DE.LEN
                JMP     .NL
.NDN            JSX     FDMERG
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
                LDW     =DK.HOLE
                STW     *DE.FLG
                EXIT    FDHOLE

; Tidy the holes: one with no sectors goes, and two that touch become
; one.  The entry in hand is looked at again after either, since what
; follows it has moved up.
FDMERG          SUBR
                LDW     =FDENT
.ML             STW     FDM
                CMW     =FDEND
                JGE     .MDN
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .M1
                JMP     .MDN
.M1             CMW     =DK.HOLE
                JNE     .MNX
                LDW     *DE.CNT
                JNZ     .M2             ; a hole of nothing?
                LDW     FDM
                JSX     FDDEL
                LDW     FDM
                JMP     .ML
.M2             LDW     FDM             ; the one after it
                ADD     =DE.LEN
                STW     FDM2P
                CMW     =FDEND
                JGE     .MNX
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                CMW     =DK.HOLE
                JNE     .MNX
                LDW     *DE.CNT
                LDX     FDM
                ADD     *DE.CNT
                STW     *DE.CNT
                LDW     FDM2P
                JSX     FDDEL
                LDW     FDM
                JMP     .ML
.MNX            LDW     FDM
                ADD     =DE.LEN
                JMP     .ML
.MDN            EXIT    FDMERG

; Take the entry ACR addresses out: everything after it moves up one, and
; the last entry is blank.
FDDEL           SUBR
                STW     FDD
.L              LDW     FDD
                ADD     =DE.LEN
                CMW     =FDEND
                JGE     .Z              ; anything left to bring up?
                CAX
                LDW     *0
                LDX     FDD
                STW     *0
                LDW     FDD
                ADD     =1
                STW     FDD
                JMP     .L
.Z              LDW     FDD
                CMW     =FDEND
                JGE     .X
                CAX
                CLR
                STW     *0
                LDW     FDD
                ADD     =1
                STW     FDD
                JMP     .Z
.X              EXIT    FDDEL

; Open a blank entry where ACR addresses: that entry and everything after
; it moves down one.  The last entry falls off the end, so the caller has
; seen that it was unused (FDROOM).
FDINS           SUBR
                STW     FDI
                LDW     =FDEND
                SUB     =DE.LEN+1
                STW     FDD             ; from the last word that survives
.IL             LDW     FDD
                CMW     FDI
                JGE     .I1             ; below the opening: done
                JMP     .IZ
.I1             LDX     FDD
                LDW     *0
                STW     *8
                LDW     FDD
                SUB     =1
                STW     FDD
                JMP     .IL
.IZ             LDX     FDI
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
                LDW     =FDENT
.L              STW     FDE
                CMW     =FDEND
                JGE     .DN
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .NX
                LDW     FDR
                ADD     =1
                STW     FDR
.NX             LDW     FDE
                ADD     =DE.LEN
                JMP     .L
.DN             LDW     FDR
                EXIT    FDROOM

; The file being written from sector ACR: its entry, or zero.
FDTENT          SUBR
                STW     FDT
                LDW     =FDENT
.TL             STW     FDE
                CMW     =FDEND
                JGE     .TNO
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .T1
                JMP     .TNO
.T1             CMW     =DK.TENT
                JNE     .TNX
                LDW     *DE.SEC
                CMW     FDT
                JNE     .TNX
                LDW     FDE
                EXIT    FDTENT
.TNX            LDW     FDE
                ADD     =DE.LEN
                JMP     .TL
.TNO            CLR
                EXIT    FDTENT

; ---------------------------------------------------------------- channels
; The channel whose number is in ACR, if it is the running task's: its
; address, left in FCH as well, or zero.
FCHK            SUBR
                SUB     =1
                STW     FCT
                AND     =FKNCHN
                JNZ     .KNO            ; 1 to 4?
                LDW     FCT
                ADD     =FCHTAB
                CAX
                LDW     *0
                STW     FCH
                CAX
                LDW     *C.OWN
                SMB     CURT
                CMW     CURT
                JNE     .KNO
                LDW     FCH
                EXIT    FCHK
.KNO            CLR
                EXIT    FCHK

; Is a channel writing the file that starts at sector ACR of the
; directory in core?  And, with no sector to match -- ACR minus one -- how
; many channels are writing on its unit at all: FCWN is left holding the
; count either way.  A channel on another unit is none of these.
FCWHO           SUBR
                STW     FCT
                CLR
                STW     FCWN
                STW     FCWF
                LDW     =FCHTAB
                STW     FCWP
                LDW     =4
                STW     FCWC
.WL             LDX     FCWP
                LDW     *0
                CAX
                LDW     *C.OWN
                JNZ     .W1
                JMP     .WNX
.W1             LDW     *C.MOD
                CMW     =2
                JNE     .WNX
                LDW     *C.UNI
                CMW     FDUNIT
                JNE     .WNX
                LDW     FCWN
                ADD     =1
                STW     FCWN
                LDW     *C.STA
                CMW     FCT
                JNE     .WNX
                LDW     =1
                STW     FCWF
.WNX            LDW     FCWP
                ADD     =1
                STW     FCWP
                LDW     FCWC
                SUB     =1
                STW     FCWC
                JNZ     .WL
                LDW     FCWF
                EXIT    FCWHO

; Blank the buffer of the channel in FCH, so that what a short last
; sector is padded with is zeros.
FCCLR           SUBR
                LDW     FCH
                ADD     =C.BUF
                STW     FCP
                LDW     =47
                STW     FCT
.L              LDX     FCP
                CLR
                STW     *0
                LDW     FCP
                ADD     =1
                STW     FCP
                LDW     FCT
                SUB     =1
                STW     FCT
                JNZ     .L
                EXIT    FCCLR

; Write the buffer of the channel in FCH to its sector and move on to the
; next: zero, or E.DISC.
FCFLSH          SUBR
                LDW     FCH
                ADD     =C.BUF
                SMB     DXBUF
                STW     DXBUF
                LDX     FCH
                LDW     *C.UNI
                SMB     DXUNI
                STW     DXUNI
                LDW     *C.SEC
                CALL    DWR1
                JNZ     .BAD
                LDX     FCH
                LDW     *C.SEC
                ADD     =1
                STW     *C.SEC
                CLR
                STW     *C.POS
                JSX     FCCLR
                CLR
                EXIT    FCFLSH
.BAD            LDW     =E.DISC
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
                LDW     =FCHTAB
                STW     FCRP
                LDW     =4
                STW     FCRC
.L              LDX     FCRP
                LDW     *0
                CAX
                LDW     *C.OWN
                CMW     FCRN
                JNE     .NX
                CLR
                STW     *C.OWN
                STW     *C.MOD
.NX             LDW     FCRP
                ADD     =1
                STW     FCRP
                LDW     FCRC
                SUB     =1
                STW     FCRC
                JNZ     .L
                EXIT    FCREL

; ---------------------------------------------------------------- the calls
; Each is the body of a vector entry, run by KENTER with the disc
; claimed: ACR is the caller's argument on the way in and the answer on
; the way out.  The vector in rex.asm states what each takes and gives.

; The shell's own way in, with the disc claimed by SHIOC: the file whose
; name ACR addresses, as its entry's address -- or zero for no such
; file, or an error, which is minus.
FSFND           SUBR
                JSX     FSNAME
                JSX     FDLOAD
                JNZ     .X
                JSX     FDFIND
.X              EXIT    FSFND

; K.LOOK
FBLOOK          SUBR
                JSX     FSNAME
                JSX     FDLOAD
                JNZ     .LX
                JSX     FDFIND
                JNZ     .L1
                LDW     =E.NOF
.LX             EXIT    FBLOOK
.L1             CAX
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
                JNZ     .N              ; a name of nothing is no file's
                LDW     =E.NOF
                EXIT    FBOPEN
.N              LDW     =FCHTAB         ; a channel nobody has
                STW     FOP
                LDW     =4
                STW     FOC
.CL             LDX     FOP
                LDW     *0
                STW     FCH
                CAX
                LDW     *C.OWN
                JNZ     .CN
                JMP     .GOT
.CN             LDW     FOP
                ADD     =1
                STW     FOP
                LDW     FOC
                SUB     =1
                STW     FOC
                JNZ     .CL
                LDW     =E.CHN
                EXIT    FBOPEN
.GOT            LDW     =5              ; its number, 1 to 4
                SUB     FOC
                STW     FON
                LDW     FNPTR
                JSX     FSNAME
                LDX     FCH
                STW     *C.UNI
                JSX     FDLOAD
                JNZ     .X
                LDX     FNPTR
                LDW     *4
                AND     =X'00FF'
                JNZ     .CR             ; to read?

                JSX     FDFIND
                JNZ     .R1
                LDW     =E.NOF
.X              EXIT    FBOPEN
.R1             CAX
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
                LDW     =1
                STW     *C.MOD
                JMP     FBOOK

; To write: half of the largest hole, under the new name -- half, so
; that a second file can be begun before the first is closed, which with
; the whole hole taken it could not be when the free sectors are all in
; one piece.  Halving the hole takes an entry now, and closing the file
; may take another for what is left of its half, so there must be two
; unused entries more than there are files being written.  Closing it
; drops an older file of the name, so the name may not be a system file's.
.CR             JSX     FDFIND
                JNZ     FBOC1           ; an older one?
                JMP     FBOC2
FBOC1           CAX
                LDW     *DE.FLG
                AND     =DF.SYS
                JNZ     FBOPR
FBOC2           JSX     FDNORM
                LDW     =X'FFFF'
                JSX     FCWHO
                LDW     FCWN
                ADD     =1
                STW     FDN0
                JSX     FDROOM
                CMW     FDN0
                JLE     .DF
                CLR
                STW     FOB             ; the best hole so far, and its size
                STW     FOBN
                LDW     =FDENT
.HL             STW     FDE
                CMW     =FDEND
                JGE     .HD
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .H1
                JMP     .HD
.H1             CMW     =DK.HOLE
                JNE     .HN
                LDW     *DE.CNT
                CMW     FOBN
                JLE     .HN             ; bigger than the best?
                STW     FOBN
                LDW     FDE
                STW     FOB
.HN             LDW     FDE
                ADD     =DE.LEN
                JMP     .HL
.HD             LDW     FOB
                JNZ     .H2
                LDW     =E.FULL
                EXIT    FBOPEN
.H2             LDW     FOBN
                SRL     1
                JNZ     .HV             ; a hole of one sector goes whole
                JMP     .H3
.HV             STW     FDN0            ; the file's half; the rest follows
                LDX     FOB             ; it as a hole of its own
                LDW     *DE.SEC
                ADD     FDN0
                STW     FDN1
                LDW     FOBN
                SUB     FDN0
                STW     FDN2
                LDW     FOB
                ADD     =DE.LEN
                STW     FDN3
                JSX     FDINS
                LDX     FDN3
                LDW     FDN1
                STW     *DE.SEC
                LDW     FDN2
                STW     *DE.CNT
                LDW     =DK.HOLE
                STW     *DE.FLG
                LDX     FOB
                LDW     FDN0
                STW     *DE.CNT
.H3             LDX     FNPTR
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
                LDW     =DK.TENT
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
                LDW     =2              ; out, or FDNORM would take the
                STW     *C.MOD          ; file for one nobody is writing
                SMB     CURT
                LDW     CURT
                STW     *C.OWN
                JSX     FDSAVE
                JNZ     .SV
                LDW     FON
                EXIT    FBOPEN
.SV             STW     FDN0
                JSX     FCFREE
                LDW     FDN0
                EXIT    FBOPEN
.DF             LDW     =E.DFUL
                EXIT    FBOPEN
FBOPR           LDW     =E.PROT
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
                JNZ     .G1
.GBAD           LDW     =E.CHN
                EXIT    FBGETC
.G1             CAX
                LDW     *C.MOD
                CMW     =1
                JNE     .GBAD
.G2             LDX     FCH
                LDW     *C.POS
                CMW     *C.LIM
                JNE     .GHAV           ; the buffer used up?
                LDW     *C.SEC
                CMW     *C.END
                JEQ     .GEOF           ; and the file with it?
                LDW     FCH
                ADD     =C.BUF
                SMB     DXBUF
                STW     DXBUF
                LDX     FCH
                LDW     *C.UNI
                SMB     DXUNI
                STW     DXUNI
                LDW     *C.SEC
                CALL    DRD1
                JNZ     .GDE
                LDX     FCH
                LDW     *C.SEC
                ADD     =1
                STW     *C.SEC
                CMW     *C.END
                JNE     .GFUL           ; that was the last sector?
                LDW     *C.LST
                JMP     .GLIM
.GFUL           LDW     =94
.GLIM           STW     *C.LIM
                CLR
                STW     *C.POS
                JMP     .G2
.GHAV           STW     FDN0            ; the byte at POS
                ADD     =1
                STW     *C.POS
                LDW     FCH
                ADD     =C.BUF
                SLL     1
                ADD     FDN0
                CAX
                CLR
                LDB     *0
                EXIT    FBGETC
.GEOF           LDW     =X'FFFF'
                EXIT    FBGETC
.GDE            LDW     =E.DISC
                EXIT    FBGETC

; K.PUTC
FBPUTC          SUBR
                STW     FPC
                SRL     8
                JSX     FCHK
                JNZ     .P1
.PBAD           LDW     =E.CHN
                EXIT    FBPUTC
.P1             CAX
                LDW     *C.MOD
                CMW     =2
                JNE     .PBAD
                LDW     *C.ERR
                JNZ     .PX             ; refused one already?  the same again
                LDW     *C.POS
                CMW     =94
                JNE     .PST            ; a full sector waiting to go?
                JSX     FCFLSH
                JNZ     FBPERR
.PST            LDX     FCH
                LDW     *C.SEC
                CMW     *C.END
                JEQ     .PFUL           ; the hole all used?
                LDW     *C.POS
                STW     FDN0
                ADD     =1
                STW     *C.POS
                LDW     FCH
                ADD     =C.BUF
                SLL     1
                ADD     FDN0
                CAX
                LDW     FPC
                STB     *0
                CLR
.PX             EXIT    FBPUTC
.PFUL           LDW     =E.FULL
FBPERR          LDX     FCH             ; kept, for the characters after this
                STW     *C.ERR          ; one and for K.CLOSE
                EXIT    FBPUTC

; K.CLOSE
FBCLOS          SUBR
                JSX     FCHK
                JNZ     .C1
                LDW     =E.CHN
                EXIT    FBCLOS
.C1             CAX
                LDW     *C.UNI          ; the directory is its unit's
                STW     FSUNI
                LDW     *C.MOD
                CMW     =2
                JNE     .CRD            ; a reader just lets go
                LDW     *C.ERR
                JNZ     .CBAD           ; a character refused?
                LDW     *C.POS
                JNZ     .CW             ; part of a sector in the buffer?
                LDW     *C.SEC          ; no: the last sector written was a
                SUB     *C.STA          ; full one, if there was one at all
                JNZ     .CN1
                JMP     .CN2
.CN1            LDW     =94
.CN2            STW     FCL
                JMP     .CD
.CW             STW     FCL
                JSX     FCFLSH
                JNZ     .CERR
.CD             LDX     FCH
                LDW     *C.SEC
                SUB     *C.STA
                STW     FCN             ; sectors written
                JSX     FDLOAD
                JNZ     .CERR
                LDX     FCH
                LDW     *C.STA
                JSX     FDTENT
                JNZ     .CE1
                LDW     =E.DISC
                JMP     .CERR
.CE1            STW     FCE
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
                LDW     =DK.FILE
                STW     *DE.FLG
                LDW     FCR
                JNZ     .CR1
                JMP     .CM
.CR1            LDW     *DE.SEC
                ADD     FCN
                STW     FDN0
                LDW     FCE
                ADD     =DE.LEN
                STW     FDN1
                JSX     FDINS
                LDX     FDN1
                LDW     FDN0
                STW     *DE.SEC
                LDW     FCR
                STW     *DE.CNT
                LDW     =DK.HOLE
                STW     *DE.FLG
.CM             JSX     FDMERG
                JSX     FDSAVE
.CERR           STW     FDN0
                JSX     FCFREE
                LDW     FDN0
                EXIT    FBCLOS
.CRD            JSX     FCFREE
                CLR
                EXIT    FBCLOS
; A file that did not all go in is not a file: with its channel let go
; nobody is writing it, which is what FDNORM makes a hole of.  The answer
; is what the character was refused with.
.CBAD           STW     FCL
                JSX     FCFREE
                JSX     FDLOAD
                JNZ     .CBX
                JSX     FDNORM
                JSX     FDSAVE
                JNZ     .CBX
                LDW     FCL
.CBX            EXIT    FBCLOS

; K.DELET
FBDELE          SUBR
                JSX     FSNAME
                JSX     FDLOAD
                JNZ     .DX
                JSX     FDFIND
                JNZ     .D1
                LDW     =E.NOF
.DX             EXIT    FBDELE
.D1             STW     FCE
                CAX
                LDW     *DE.FLG
                AND     =DF.SYS
                JNZ     .DPR
                LDW     FCE
                JSX     FDHOLE
                JSX     FDMERG
                JSX     FDSAVE
                EXIT    FBDELE
.DPR            LDW     =E.PROT
                EXIT    FBDELE

; K.RENAM
FBRENA          SUBR
                STW     FRP
                ADD     =5              ; the new name's unit...
                JSX     FSNAME
                STW     FDN0
                LDW     FRP
                JSX     FSNAME          ; ...and the old one's, which it
                CMW     FDN0            ; must be
                JNE     .RU
                LDW     FRP
                ADD     =5
                STW     FNPTR
                JSX     FDLOAD
                JNZ     .RX
                JSX     FDFIND
                JNZ     .REX            ; the new name in use?
                LDW     FRP
                STW     FNPTR
                JSX     FDFIND
                JNZ     .R1
                LDW     =E.NOF
.RX             EXIT    FBRENA
.R1             STW     FCE
                CAX
                LDW     *DE.FLG
                AND     =DF.SYS
                JNZ     .RPR
                LDX     FRP
                LDW     *5
                STW     FDN0
                LDW     *6
                STW     FDN1
                LDW     *7
                STW     FDN2
                LDW     *8
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
.REX            LDW     =E.EXST
                EXIT    FBRENA
.RPR            LDW     =E.PROT
                EXIT    FBRENA
.RU             LDW     =E.UNIT
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
                STW     FDN0
                CLR                     ; no name: the data unit for none
                STW     FNPTR
                LDW     FDN0
                JSX     FSUNIT
                JSX     FDLOAD
                JNZ     .SX
                LDW     =FCHTAB
                STW     FCWP
                LDW     =4
                STW     FCWC
.SC             LDX     FCWP
                LDW     *0
                CAX
                LDW     *C.OWN
                JNZ     .SBSY
                LDW     FCWP
                ADD     =1
                STW     FCWP
                LDW     FCWC
                SUB     =1
                STW     FCWC
                JNZ     .SC
                JSX     FDNORM
                LDW     =FDENT
.SL             STW     FSH             ; a hole, perhaps
                CMW     =FDEND
                JGE     .SDN
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .S1
                JMP     .SDN
.S1             CMW     =DK.HOLE
                JNE     .SNX
                LDW     *DE.SEC
                STW     FSD
                STW     FSHS
                LDW     *DE.CNT
                STW     FSHN
                LDW     FSH             ; and what follows it
                ADD     =DE.LEN
                STW     FSF
                CMW     =FDEND
                JGE     .SDN
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                CMW     =DK.FILE
                JNE     .SNX
                LDW     *DE.FLG
                AND     =DF.SYS
                JNZ     .SNX
                LDW     *DE.SEC
                STW     FSS
                LDW     *DE.CNT
                STW     FSN
                STW     FSFN
.SM             LDW     FSN             ; the sectors, lowest first, so the
                JNZ     .SM1            ; move may overlap itself
                JMP     .SMD
.SM1            LDW     =FKSBUF
                SMB     DXBUF
                STW     DXBUF
                LDW     FSUNI
                SMB     DXUNI
                STW     DXUNI
                LDW     FSS
                CALL    DRD1
                JNZ     .SERR
                LDW     FSUNI
                SMB     DXUNI
                STW     DXUNI
                LDW     FSD
                CALL    DWR1
                JNZ     .SERR
                LDW     FSS
                ADD     =1
                STW     FSS
                LDW     FSD
                ADD     =1
                STW     FSD
                LDW     FSN
                SUB     =1
                STW     FSN
                JMP     .SM
.SMD            LDW     =DE.LEN         ; the two entries change places: the
                STW     FSN             ; file's goes into the hole's slot...
.SE             LDX     FSF
                LDW     *0
                LDX     FSH
                STW     *0
                LDW     FSF
                ADD     =1
                STW     FSF
                LDW     FSH
                ADD     =1
                STW     FSH
                LDW     FSN
                SUB     =1
                STW     FSN
                JNZ     .SE
                LDW     FSH             ; (both cursors are a slot on: FSH is
                SUB     =DE.LEN         ; the hole's new slot, the file's is
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
                JNZ     .SX
                LDW     FSH             ; on from the hole, which may have
                JMP     .SL             ; another file behind it
.SNX            LDW     FSH
                ADD     =DE.LEN
                JMP     .SL
.SDN            CLR
.SX             EXIT    FBSQSH
.SBSY           LDW     =E.CHN
                EXIT    FBSQSH
.SERR           LDW     =E.DISC
                EXIT    FBSQSH

; K.DIRNT: the directory's entry ACR+1 names, of the unit ACR's high byte
; names, copied out to ACR+2..ACR+9 -- so that a listing at the
; teletype's pace holds the disc for an entry at a time, while another
; task's call may change which unit's directory is in core in between.
FBDIRN          SUBR
                STW     FRP
                CLR                     ; no name: the data unit for none
                STW     FNPTR
                LDX     FRP
                LDW     *0
                JSX     FSUNIT
                JSX     FDLOAD
                JNZ     .X
                LDX     FRP
                LDW     *1
                CMW     =FSNENT
                JGE     .NO             ; inside the table?
                SLL     3
                ADD     =FDENT
                STW     FDE
                CAX
                LDW     *DE.FLG
                AND     =X'00FF'
                JNZ     .Y              ; an unused entry ends it
.NO             LDW     =E.NOF
.X              EXIT    FBDIRN
.Y              LDW     FRP
                ADD     =2
                STW     FDD
                LDW     =DE.LEN
                STW     FDR
.L              LDX     FDE
                LDW     *0
                LDX     FDD
                STW     *0
                LDW     FDE
                ADD     =1
                STW     FDE
                LDW     FDD
                ADD     =1
                STW     FDD
                LDW     FDR
                SUB     =1
                STW     FDR
                JNZ     .L
                EXIT    FBDIRN

; ---------------------------------------------------------------- units
; The name ACR addresses is the one a search is for, and its unit is the
; one the call is on: FSUNI, and ACR.
FSNAME          SUBR
                STW     FNPTR
                CAX
                LDW     *4
                JSX     FSUNIT
                EXIT    FSNAME

; The unit ACR's high byte names, into FSUNI and ACR: 0 to 3 as given,
; and X'80', none given, settled by the type of the name FNPTR addresses
; -- or by none, FNPTR zero -- as the header says.  Which unit is the
; data unit is found out once, the first time it is wanted: the 703's
; platters are not changed while it runs.
FSUNIT          SUBR
                SRL     8
                CMW     =X'80'
                JEQ     .D
                AND     =3
.S              STW     FSUNI
                EXIT    FSUNIT
.D              LDW     FNPTR
                JNZ     .T              ; a name?
                JMP     .DU
.T              CAX
                LDW     *3
                CMW     ='RX'
                JNE     .DU             ; a program is on unit 0
                CLR
                JMP     .S
.DU             LDW     FSDU
                JPL     .S              ; known already?
                LDW     =1
                STW     FSUNI
                JSX     FDLOAD
                JNZ     .D0             ; unit 1 has a directory?
                LDW     =1
                JMP     .DS
.D0             CLR
.DS             STW     FSDU
                JMP     .S

; ---------------------------------------------------------------- data
FDUNIT          WORD    X'FFFF'         ; the unit whose directory is in core; -1 none
FSUNI           WORD    0               ; the unit of the call in hand
FSDU            WORD    X'FFFF'         ; the data unit; -1 not found out yet
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
                LPOOL                   ; what this page's code writes as literals

FKNCHN          EQU     X'FFFC'         ; what a channel number less one may not have set
FKSBUF          EQU     FCH1+C.BUF      ; FBSQSH's sector: the first channel's

FCHTAB          WORD    FCH1,FCH2,FCH3,FCH4
FCH1            RES     C.LEN
FCH2            RES     C.LEN
FCH3            RES     C.LEN
FCH4            RES     C.LEN
FDBUF           RES     FSDIRW          ; the directory
FSEND           EQU     $
