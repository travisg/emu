; vim: ts=8:sw=8:expandtab:
;
; REX's swapper -- the last file of the kernel's deck, after rexfs.asm.
;
; When more is loaded than core holds, a task that is not running goes
; out to the disc and comes back when it can run again.  A task cannot
; be moved: its module was relocated once, when it was loaded, and has
; addresses of its own all through it.  So what goes out is every block
; in the pool the task owns, each written from where it lies with its
; header, and what comes back is the same blocks cut out of the free list
; at the same addresses (ALLOCAT, rex.asm) -- whoever is in the way by
; then going out first.  The task's node never moves, and nothing that
; wakes a task touches anything but its node, so a task may be put out in
; any state; PICK passes over one whose blocks are not in core.
;
; The swapper is a task, the fifth on the ring, asleep until one of two
; things wants it:
;
;   a task waiting for core   an allocation found nothing that fits and
;                             left its request in the task's node (T.MW,
;                             ALWAIT in rex.asm).  One task is put out,
;                             and the asker is told to try again -- or
;                             told there is nobody to put out, and gets
;                             its zero.  If the only candidates are
;                             running and have not had their time, the
;                             asker is left waiting until one has.
;   a task that can run       but whose blocks are on the disc.  They
;                             are brought back, the owners of whatever
;                             lies where they go being put out first.
;
; Who may be put out: a task the shell loaded whose blocks are in core,
; that is not itself waiting for core, and does not have the disc.  One
; that is stopped, asleep or waiting goes before one that could run; one
; that could run goes only when it has been in core through the second
; it arrived in and the next, which is what stops two tasks that want
; the same core from spending all their time changing places.
;
; The disc is claimed like anybody's (IOLOCK), with one exception: the
; shell's loader holds the disc for a whole load, and may be the task
; waiting for core.  A holder that is waiting for core is not using the
; disc, so the swapper uses it then without the claim -- for that one
; task's request only, and wakes it last, when the disc is quiet again.
;
; On the disc a task is one extent of SWAP.SY, a system file made with
; the platter: a sector listing its blocks, address and length in words,
; then each block on a sector of its own to start.  The extents are kept
; in a table here, first fit.  With no SWAP.SY nothing is ever put out,
; and an allocation that finds no room fails as it always did.

                ORG     X'7800'

SWXN            EQU     16              ; extents: tasks that can be out at once
SWBMAX          EQU     23              ; blocks one sector's list will hold

; ---------------------------------------------------------------- the task
SWAPR           JSX     SWINIT
SWLOOP          MSK
                JSX     SWWORK
                JNZ     SWL1            ; anything to do?
                LDX     SKSELF          ; no: until somebody wakes it
                LDW     SKWAIT
                STW     *T.STA
                CALL    SWTCH
                JMP     SWLOOP
SWL1            UNM
                JSX     SWCLM
                JNZ     SWL2            ; the disc?
                JMP     SWNAP
SWL2            JSX     SWMEM
                LDW     SWBOR
                JNZ     SWNAP           ; on a borrowed disc, that was all
                JSX     SWBRNG
                LDW     SKM1
                SMB     IOLOCK
                STW     IOLOCK
; What is left is waiting for something -- a task's second in core to
; run out, the disc to come free -- so a few ticks pass before the next
; look.
SWNAP           MSK
                LDX     SKSELF
                LDW     SK3
                STW     *T.DLY
                LDW     SKSLP
                STW     *T.STA
                CALL    SWTCH
                JMP     SWLOOP

; Where SWAP.SY is, once.  No disc, no directory or no such file leaves
; SWSECS zero, and there is no swapping.
SWINIT          SUBR
SWIL            MSK
                CALL    IOTRY
                JNZ     SWIW
                UNM
                LDW     SKNAM
                CALL    FSFND
                JNZ     SWI1
                JMP     SWIX
SWI1            JMI     SWIX            ; an error is minus
                CAX
                LDW     *DE.SEC
                STW     SWBASE
                LDW     *DE.CNT
                SMB     SWSECS
                STW     SWSECS
SWIX            LDW     SKM1
                SMB     IOLOCK
                STW     IOLOCK
                EXIT    SWINIT
SWIW            LDX     SKSELF
                LDW     SK1
                STW     *T.DLY
                LDW     SKSLP
                STW     *T.STA
                CALL    SWTCH
                JMP     SWIL

; Is there anything to do?  A task asking for core, or one that can run
; with its blocks on the disc.  The caller holds the mask.
SWWORK          SUBR
                LDX     SKSHEL
                LDW     *T.MW
                CMW     SK1
                JEQ     SWWYES
                LDW     SKNARB
                STW     SWP
                LDW     SKNARN
                STW     SWC
SWWL            LDX     SWP
                LDW     *T.STA
                CMW     SKFREE
                JEQ     SWWNX
                LDW     *T.MW
                CMW     SK1
                JEQ     SWWYES
                JSX     SWWANT
                JNZ     SWWYES
SWWNX           JSX     SWSTEP
                JNZ     SWWL
                EXIT    SWWORK          ; zero
SWWYES          LDW     SK1
                EXIT    SWWORK

; Does the node SWP want bringing in?  Runnable, and on the disc -- not
; on its way.
SWWANT          SUBR
                LDX     SWP
                LDW     *T.STA
                JNZ     SWWNO
                LDW     *T.SWP
                JNZ     SWWA1
SWWNO           CLR
                EXIT    SWWANT
SWWA1           CMW     SKM1
                JEQ     SWWNO
                LDW     SK1
                EXIT    SWWANT

; On to the next arena node: zero when the last has been looked at.
SWSTEP          SUBR
                LDW     SWP
                ADD     SKTLEN
                STW     SWP
                LDW     SWC
                SUB     SK1
                STW     SWC
                EXIT    SWSTEP

; The disc: claimed, if it is free; borrowed, if whoever has it is
; waiting for core; else not to be had this time, and zero.
SWCLM           SUBR
                MSK
                CLR
                STW     SWBOR
                SMB     IOLOCK
                LDW     IOLOCK
                JPL     SWC1
                LDW     SKSELF
                SMB     IOLOCK
                STW     IOLOCK
SWCYES          UNM
                LDW     SK1
                EXIT    SWCLM
SWC1            STW     SWHOLD
                CAX
                LDW     *T.MW
                CMW     SK1
                JNE     SWCNO
                LDW     SK1
                STW     SWBOR
                JMP     SWCYES
SWCNO           UNM
                CLR
                EXIT    SWCLM

; ---------------------------------------------------------------- core wanted
; Every task asking for core: one task put out for each, and the asker
; told to try again -- or that there is nobody.  On a borrowed disc, only
; the task it was borrowed from.
SWMEM           SUBR
                LDW     SKSHEL
                STW     SWP
                JSX     SWMEM1
                LDW     SKNARB
                STW     SWP
                LDW     SKNARN
                STW     SWC
SWML            LDX     SWP
                LDW     *T.STA
                CMW     SKFREE
                SEQ
                JSX     SWMEM1
                JSX     SWSTEP
                JNZ     SWML
                EXIT    SWMEM

SWMEM1          SUBR
                LDX     SWP
                LDW     *T.MW
                CMW     SK1
                JNE     SWM1X           ; asking?
                LDW     SWBOR
                JNZ     SWM1B
                JMP     SWM1G
SWM1B           LDW     SWP
                CMW     SWHOLD
                JNE     SWM1X
SWM1G           LDW     SWP
                STW     SWASK
                JSX     SWVICT
                JNZ     SWM1V
                LDW     SWYNG           ; nobody.  For good, or only until
                JNZ     SWM1X           ; one that is running has had its
                                        ; time?  Then the asker goes on
                LDW     SK2             ; waiting, and is looked at again
                JMP     SWM1T           ; on the next pass
SWM1V           JSX     SWOUT
                JNZ     SWM1N           ; gone?
                LDW     SK3             ; try again
                JMP     SWM1T
SWM1N           LDW     SK2
SWM1T           MSK
                LDX     SWASK
                STW     *T.MW
                LDW     *T.STA
                CMW     SKWAIT
                JNE     SWM1U           ; out of its wait, and only that
                CLR
                STW     *T.STA
SWM1U           UNM
SWM1X           EXIT    SWMEM1

; ---------------------------------------------------------------- who goes
; A task to put out, other than the asker in SWASK: one that cannot run
; for choice, else one that has had its time in core.  Its node, or zero
; -- with SWYNG set if there is one that will do once it has had its
; time, which is the difference between "not yet" and "never".
SWVICT          SUBR
                CLR
                STW     SWV2
                STW     SWYNG
                LDW     SKNARB
                STW     SWQ
                LDW     SKNARN
                STW     SWQC
SWVL            LDW     SWQ
                CMW     SWASK
                JEQ     SWVNX
                JSX     SWELIG
                JNZ     SWVE
                JMP     SWVNX
SWVE            LDX     SWQ
                LDW     *T.STA
                JNZ     SWVHIT          ; it cannot run: this one
                JSX     SWAGED
                JNZ     SWVOLD
                LDW     SK1             ; it can, and has not had its time
                STW     SWYNG           ; yet: nobody for now, but not for good
                JMP     SWVNX
SWVOLD          LDW     SWQ             ; it can, and has had its time: kept
                STW     SWV2            ; in case there is nobody better
SWVNX           LDW     SWQ
                ADD     SKTLEN
                STW     SWQ
                LDW     SWQC
                SUB     SK1
                STW     SWQC
                JNZ     SWVL
                LDW     SWV2
                EXIT    SWVICT
SWVHIT          LDW     SWQ
                EXIT    SWVICT

; May the arena node SWQ be put out at all?  In use, its blocks in core,
; not asking for core itself, and not the task that has the disc.
SWELIG          SUBR
                LDX     SWQ
                LDW     *T.STA
                CMW     SKFREE
                JEQ     SWENO
                LDW     *T.SWP
                JNZ     SWENO
                LDW     *T.MW
                JNZ     SWENO
                LDW     SWQ
                SMB     IOLOCK
                CMW     IOLOCK
                JEQ     SWENO
                LDW     SK1
                EXIT    SWELIG
SWENO           CLR
                EXIT    SWELIG

; Has the node SWQ been in core long enough?  Not in the second it
; arrived, nor the one after.
SWAGED          SUBR
                LDX     SWQ
                SMB     SECS
                LDW     SECS
                SUB     *T.RES
                JNZ     SWAG1
                EXIT    SWAGED          ; zero: the same second
SWAG1           CMW     SK1
                JEQ     SWAGNO
                LDW     SK1
                EXIT    SWAGED
SWAGNO          CLR
                EXIT    SWAGED

; ---------------------------------------------------------------- out
; Put out the task whose node is in ACR: zero if it went.  It is marked
; on its way first, under the mask, so that it does not run -- and so
; neither takes nor gives back a block -- while its blocks are listed,
; written and freed.
SWOUT           SUBR
                STW     SWT
                MSK
                CAX
                LDW     SKM1
                STW     *T.SWP
                CLR                     ; the list: every block it owns
                STW     SWOK
                STW     SWOSEC
                LDW     SWT
                ORI     SK8000
                STW     SWTAG
                LDW     SKPOOB
SWOWL           STW     SWB
                CMW     SKPOOE
                JGE     SWOWD
                CAX
                LDW     *1
                CMW     SWTAG
                JNE     SWOWN
                LDW     SWOK
                CMW     SKBMAX
                JGE     SWOBAD          ; more blocks than the list holds
                SLL     1
                ADD     SKHDO
                ADD     SK1
                STW     SWHP
                LDX     SWB
                LDW     *0
                ADD     SK2
                STW     SWLEN
                LDX     SWHP
                STW     *1
                LDW     SWB
                STW     *0
                JSX     SWSECN          ; and the sectors it will take
                ADD     SWOSEC
                STW     SWOSEC
                LDW     SWOK
                ADD     SK1
                STW     SWOK
SWOWN           LDX     SWB
                LDW     *0
                ADD     SWB
                ADD     SK2
                JMP     SWOWL
SWOWD           LDW     SWOK
                JNZ     SWOW1           ; a task with no blocks has nothing
                                        ; to put out
                JMP     SWOBAD
SWOW1           STW     SWHDO
                LDW     SWOSEC          ; an extent: the list's sector too
                ADD     SK1
                JSX     SWXALC
                JMI     SWOBAD
                STW     SWOST
                UNM
                ADD     SWBASE          ; the list
                STW     SWSEC
                LDW     SKHDO
                STW     SWADR
                LDW     SK47
                STW     SWLEN
                LDW     SK1
                JSX     SWXFER
                JNZ     SWOERR
                CLR
                STW     SWI
SWOBL           LDW     SWI             ; and each block from where it lies
                CMW     SWOK
                JGE     SWOFR
                JSX     SWITEM
                LDW     SK1
                JSX     SWXFER
                JNZ     SWOERR
                LDW     SWI
                ADD     SK1
                STW     SWI
                JMP     SWOBL
SWOFR           MSK
                CLR
                STW     SWI
SWOFL           LDW     SWI             ; written: the core is free
                CMW     SWOK
                JGE     SWOFD
                JSX     SWITEM
                LDW     SWADR
                ADD     SK2
                CALL    FREEI
                LDW     SWI
                ADD     SK1
                STW     SWI
                JMP     SWOFL
SWOFD           LDX     SWT
                LDW     SWOST
                ADD     SK1
                STW     *T.SWP
                LDW     SWOUTS
                ADD     SK1
                STW     SWOUTS
                UNM
                CLR
                EXIT    SWOUT
SWOERR          MSK                     ; the disc said no: nothing has been
                LDW     SWT             ; freed, so the task is simply still
                JSX     SWXFR0          ; here
SWOBAD          LDX     SWT
                CLR
                STW     *T.SWP
                UNM
                LDW     SK1
                EXIT    SWOUT

; Block SWI of the list at SKHDO: its address into SWADR, its length
; into SWLEN, and the sector it is written at into SWSEC -- which this
; keeps count of, so the blocks are asked for in order from the first.
SWITEM          SUBR
                LDW     SWI
                JNZ     SWIT1
                LDW     SWOST           ; the first: past the list's sector
                ADD     SWBASE
                ADD     SK1
                STW     SWSEC
                JMP     SWIT2
SWIT1           JSX     SWSECN          ; past the one before
                ADD     SWSEC
                STW     SWSEC
SWIT2           LDW     SWI
                SLL     1
                ADD     SWLIST
                ADD     SK1
                CAX
                LDW     *0
                STW     SWADR
                LDW     *1
                STW     SWLEN
                EXIT    SWITEM

; How many sectors SWLEN words take.
SWSECN          SUBR
                CLR
                CAX
                LDW     SWLEN
                ADD     SK46
                DIV     SK47
                EXIT    SWSECN

; SWLEN words between core at SWADR and the disc from sector SWSEC: ACR
; zero to read, else to write.  The controller's status.
SWXFER          SUBR
                SMB     DXOP
                STW     DXOP
                LDW     SWSEC
                SMB     DXSEC
                STW     DXSEC
                LDW     SWLEN
                SMB     DXCNT
                STW     DXCNT
                LDW     SWADR
                SMB     DXBUF
                STW     DXBUF
                CALL    DXFER
                EXIT    SWXFER

; ---------------------------------------------------------------- in
; Every task that can run and is on the disc: brought back if its core
; can be had.
SWBRNG          SUBR
                LDW     SKNARB
                STW     SWP
                LDW     SKNARN
                STW     SWC
SWBL            LDX     SWP
                LDW     *T.STA
                CMW     SKFREE
                JEQ     SWBNX
                JSX     SWWANT
                SAZ
                JSX     SWIN
SWBNX           JSX     SWSTEP
                JNZ     SWBL
                EXIT    SWBRNG

; Bring the task SWP back.  Its list is read; then, under the mask, each
; block is cut out of the free list where it was.  If one cannot be --
; something is there -- the ones already cut are given back, and whoever
; owns what is in the way is put out, if it may be, and the cutting tried
; again; if it may not be, the task waits for another pass.  With every
; block cut, each is read back into its place, and the task can run.
SWIN            SUBR
                LDW     SWP
                STW     SWT2
                CAX
                LDW     *T.SWP
                SUB     SK1
                STW     SWIST
                ADD     SWBASE
                STW     SWSEC
                LDW     SKHDI
                STW     SWADR
                LDW     SK47
                STW     SWLEN
                CLR
                JSX     SWXFER
                JNZ     SWINX
                LDW     SK8
                STW     SWTRY
SWICUT          MSK
                CLR
                STW     SWJ
SWICL           LDW     SWJ
                CMW     SWHDI
                JGE     SWIRD
                SLL     1
                ADD     SKHDI
                ADD     SK1
                CAX
                LDW     *1
                SMB     ATN
                STW     ATN
                LDW     SWT2
                ORI     SK8000
                SMB     ATTAG
                STW     ATTAG
                LDW     *0
                STW     SWIH
                ADD     *1
                STW     SWIHE
                LDW     SWIH
                CALL    ALLOCAT
                JNZ     SWICN
                JMP     SWIBLK
SWICN           LDW     SWJ
                ADD     SK1
                STW     SWJ
                JMP     SWICL

; Something is where block SWJ goes.  Give back what has been cut, find
; the owner of what is in the way, and put it out if that is allowed.
SWIBLK          LDW     SWJ
                JNZ     SWIU1
                JMP     SWIU2
SWIU1           SUB     SK1
                STW     SWJ
                SLL     1
                ADD     SKHDI
                ADD     SK1
                CAX
                LDW     *0
                ADD     SK2
                CALL    FREEI
                JMP     SWIBLK
SWIU2           LDW     SWIH
                STW     SWOH
                LDW     SWIHE
                STW     SWOHE
                JSX     SWOWNR
                UNM
                STW     SWQ
                CMW     SKNARB
                JGE     SWIO1           ; a task the shell loaded, or nobody's
                                        ; to put out?
                JMP     SWINX
SWIO1           JSX     SWELIG
                JNZ     SWIO2
                JMP     SWINX
SWIO2           LDX     SWQ
                LDW     *T.STA
                JNZ     SWIO3           ; it could run: has it had its time?
                JSX     SWAGED
                JNZ     SWIO3
                JMP     SWINX
SWIO3           LDW     SWLIST          ; SWOUT's list is not this one
                STW     SWLSAV
                LDW     SKHDO
                STW     SWLIST
                LDW     SWQ
                JSX     SWOUT
                STW     SWRC
                LDW     SWLSAV
                STW     SWLIST
                LDW     SWRC
                JNZ     SWINX
                LDW     SWTRY
                SUB     SK1
                STW     SWTRY
                JNZ     SWICUT
                JMP     SWINX

; Every block is cut.  Read each back; the extent is free; the task is
; here, as of this second.
SWIRD           UNM
                LDW     SKHDI
                STW     SWLIST
                LDW     SWIST
                STW     SWOST
                CLR
                STW     SWI
SWIRL           LDW     SWI
                CMW     SWHDI
                JGE     SWIRDN
                JSX     SWITEM
                CLR
                JSX     SWXFER
                LDW     SWI
                ADD     SK1
                STW     SWI
                JMP     SWIRL
SWIRDN          LDW     SKHDO
                STW     SWLIST
                MSK
                LDW     SWT2
                JSX     SWXFR0
                LDX     SWT2
                CLR
                STW     *T.SWP
                SMB     SECS
                LDW     SECS
                STW     *T.RES
                LDW     SWINS
                ADD     SK1
                STW     SWINS
                UNM
SWINX           EXIT    SWIN

; Whose block is in the way of the words from SWOH up to SWOHE?  The
; owner of the first allocated block that reaches into them -- which is
; zero for the fence, or if nothing does.  The caller holds the mask.
SWOWNR          SUBR
                LDW     SKPOOB
SWONL           STW     SWB
                CMW     SKPOOE
                JGE     SWONO
                CMW     SWOHE
                JGE     SWONO           ; a block that starts past them: none
                CAX
                LDW     *0
                ADD     SWB
                ADD     SK2
                STW     SWOE
                CMW     SWOH
                JLE     SWONN           ; ends at or before them: the next
                LDX     SWB
                LDW     *1
                JPL     SWONN           ; free: the next
                AND     SK7FFF
                EXIT    SWOWNR
SWONN           LDW     SWOE
                JMP     SWONL
SWONO           CLR
                EXIT    SWOWNR

; ---------------------------------------------------------------- extents
; An extent of ACR sectors, first fit among those in use: its first
; sector within the file, or minus one.  The caller holds the mask, and
; SWT is the task it is for.
SWXALC          SUBR
                STW     SWXL
                CLR
                STW     SWXS
SWXA0           LDW     SKXTAB          ; does anything overlap [S, S+L)?
                STW     SWXP
                LDW     SKXN
                STW     SWXC
SWXA1           LDX     SWXP
                LDW     *0
                JNZ     SWXA2
                JMP     SWXA4
SWXA2           LDW     *1              ; its first sector, and one past its last
                STW     SWXB
                ADD     *2
                STW     SWXE
                CMW     SWXS
                JLE     SWXA4           ; it ends at or before S: clear of it
                LDW     SWXS
                ADD     SWXL
                CMW     SWXB
                JLE     SWXA4           ; S+L at or before its start: clear
                LDW     SWXE            ; no: try from where it ends
                STW     SWXS
                JMP     SWXA0
SWXA4           LDW     SWXP
                ADD     SK3
                STW     SWXP
                LDW     SWXC
                SUB     SK1
                STW     SWXC
                JNZ     SWXA1
                LDW     SWXS
                ADD     SWXL
                SMB     SWSECS
                CMW     SWSECS
                JLE     SWXA5           ; inside the file?
                JMP     SWXANO
SWXA5           LDW     SKXTAB          ; a table entry nobody has
                STW     SWXP
                LDW     SKXN
                STW     SWXC
SWXA6           LDX     SWXP
                LDW     *0
                JNZ     SWXA7
                LDW     SWT
                STW     *0
                LDW     SWXS
                STW     *1
                LDW     SWXL
                STW     *2
                LDW     SWXS
                EXIT    SWXALC
SWXA7           LDW     SWXP
                ADD     SK3
                STW     SWXP
                LDW     SWXC
                SUB     SK1
                STW     SWXC
                JNZ     SWXA6
SWXANO          LDW     SKM1
                EXIT    SWXALC

; The extent of the task whose node is in ACR is free, if it has one.
; SWXFRE is REAP's door, for a task that ends while it is out, and keeps
; to cells of its own, since the swapper may be part way through
; anything; SWXFR0 is the swapper's.  The caller holds the mask.
SWXFRE          SUBR
                STW     SWFN
                LDW     SKXTAB
                STW     SWFP
                LDW     SKXN
                STW     SWFC
SWXFL           LDX     SWFP
                LDW     *0
                CMW     SWFN
                JNE     SWXFN
                CLR
                STW     *0
SWXFN           LDW     SWFP
                ADD     SK3
                STW     SWFP
                LDW     SWFC
                SUB     SK1
                STW     SWFC
                JNZ     SWXFL
                EXIT    SWXFRE
SWXFR0          SUBR
                JSX     SWXFRE
                EXIT    SWXFR0

; ---------------------------------------------------------------- data
SWOUTS          WORD    0               ; tasks put out since the machine came up,
SWINS           WORD    0               ; and brought back
SWBASE          WORD    0               ; SWAP.SY's first sector
SWBOR           WORD    0               ; the disc is borrowed, not claimed,
SWHOLD          WORD    0               ; from this task
SWP             WORD    0               ; a walk of the arena, and its countdown
SWC             WORD    0
SWQ             WORD    0               ; another, for who goes out
SWQC            WORD    0
SWV2            WORD    0               ; the second choice
SWYNG           WORD    0               ; somebody will do, in a second or two
SWASK           WORD    0               ; the task asking for core
SWT             WORD    0               ; the task going out,
SWTAG           WORD    0               ; its owner tag,
SWOK            WORD    0               ; how many blocks,
SWOSEC          WORD    0               ; how many sectors,
SWOST           WORD    0               ; and its extent
SWB             WORD    0               ; a walk of the pool
SWHP            WORD    0               ; a place in a list
SWI             WORD    0               ; a block's number in one,
SWADR           WORD    0               ; its address,
SWLEN           WORD    0               ; its length,
SWSEC           WORD    0               ; and its sector
SWLIST          WORD    SWHDO           ; the list SWITEM reads
SWLSAV          WORD    0
SWT2            WORD    0               ; the task coming in,
SWIST           WORD    0               ; its extent,
SWJ             WORD    0               ; the block being cut,
SWIH            WORD    0               ; where it goes, and where it ends,
SWIHE           WORD    0
SWTRY           WORD    0               ; tries left,
SWRC            WORD    0               ; and what SWOUT said
SWOH            WORD    0               ; SWOWNR's words, first and one past
SWOHE           WORD    0               ; the last, and a block's end
SWOE            WORD    0
SWXL            WORD    0               ; SWXALC's length, start,
SWXS            WORD    0
SWXP            WORD    0               ; cursor, countdown,
SWXC            WORD    0
SWXB            WORD    0               ; and an extent's two ends
SWXE            WORD    0
SWFN            WORD    0               ; SWXFRE's node, cursor and countdown
SWFP            WORD    0
SWFC            WORD    0

SK1             WORD    1
SK2             WORD    2
SK3             WORD    3
SK8             WORD    8
SK46            WORD    46
SK47            WORD    47
SKM1            WORD    X'FFFF'
SK7FFF          WORD    X'7FFF'
SK8000          WORD    X'8000'
SKSLP           WORD    SSLP
SKWAIT          WORD    SWAI
SKFREE          WORD    SFREE
SKTLEN          WORD    T.LEN
SKSELF          WORD    SWTCB
SKSHEL          WORD    SHTCB
SKNARB          WORD    NARB
SKNARN          WORD    NARN
SKPOOB          WORD    POOLB
SKPOOE          WORD    POOLE
SKBMAX          WORD    SWBMAX
SKXN            WORD    SWXN
SKXTAB          WORD    SWXTAB
SKHDO           WORD    SWHDO
SKHDI           WORD    SWHDI
SKNAM           WORD    SWNAME
SWNAME          WORD    'SW','AP',0,'SY'

SWXTAB          RES     SWXN*3          ; an extent: the node, the first sector, how many
SWHDO           RES     47              ; a list going out: how many blocks,
SWHDI           RES     47              ; then address and length of each; and one coming in
SWEND           EQU     $
