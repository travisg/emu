; vim: ts=8:sw=8:expandtab:
;
; The loader, in page 1 -- the file of the kernel's deck after rex.asm,
; with the loader's record reader (rexrec.asm) behind it in the rest of
; the page.  K.LOAD reaches it through KENTER, so a load runs with the
; disc claimed throughout, one at a time, and its cells are static like
; any single-task routine's.  Every shell loads its programs through it,
; and SHBOOT, here, loads the console's shell with it at start-up.

                ORG     X'800'

; ---------------------------------------------------------------- the boot
; The kernel's last act at start-up, as the console's node SHTCB: the
; console's shell is SHELL.RX off unit 0, loaded into SHTCB itself and
; entered there -- an indexed JSX, which is global, so the shell runs in
; its own page as SPAWN's status word would have it.  Without one the
; console says so and the machine stops.
SHBOOT          LDW     =BTPB
                CALL    K.LOAD
                JMI     .NO             ; loaded?
                LDW     LDEXEC
                STW     SHENT
                CAX
                JSX     *0              ; and never back
.NO             LDW     BTMSG           ; the first byte of the message
                STW     BTP
.L              LDW     BTP
                CMW     BTMSG+1
                JEQ     .H              ; the end of it?
                CAX
                CLR
                LDB     *0
                MSK
                LDX     =SHTCB          ; a character through the mailbox,
                STW     *T.MBX          ; and waited for, as a shell prints
                CALL    K.KICK
.W              MSK
                LDX     =SHTCB
                LDW     *T.MBX
                JZ      .N              ; printed yet?
                LDW     =SWAI
                STW     *T.STA
                CALL    K.SWTCH
                JMP     .W
.N              UNM
                LDW     BTP
                ADD     =1
                STW     BTP
                JMP     .L
.H              MSK
                HLT

BTPB            WORD    'SH','EL',X'CC00','RX',0  ; SHELL.RX, on unit 0,
                WORD    0,TMT,SHTCB     ; with no tail, for the console, into SHTCB
BTP             WORD    0               ; the message's cursor
BTMSG           MSG     "NO SHELL.RX\r\n"
SHENT           WORD    0               ; the console's shell's entry

; ---------------------------------------------------------------- the loader
; Load a module: find it in the directory and run its object text into
; a block from the pool, a record at a time from the record reader
; (LDREC, rexrec.asm), which reads them off the disc.  The text is the
; 1968 relocating loader's (tools/asm703.py's docstring lists the codes,
; tools/reload703.py is the reference reading of them): the SIZE code
; comes first and sizes the block, whose address is the relocation base;
; each repeatable code carries a run of words that are stored as they
; are, or with the base added into the 11-bit M field, the whole word,
; or twice for a byte address; a page selection is built here from the
; final address; END names the entry, which gets the base too.  A module
; of several segments has a SIZE code and a block for each, up to LDKSEG
; of them, and two codes to say whose text is being stored and whose base
; added.
;
; ACR addresses eight words: the module's name as K.OPEN takes it (five);
; the byte address of the command tail, a zero after it, or zero for
; none; the terminal the task will be on; and the node to load it into,
; or zero for one from the arena.  The answer is the node, with its
; entry, name and terminal filled in and each block tagged as the task's
; own, ready for SPAWN -- or LE.NOF for no such file, LE.DE for the
; disc's status or no directory on it, LE.CK for a record failing its
; checksum, LE.LC for a code this loader does not take or text out of
; order, LE.MX for no room, all of them minus.  An error path jumps
; straight out of whatever routine it was in, as RELOADB's does.
LDMOD           SUBR
                STW     LDPB
                CAX
                LDW     *0              ; a name at all?
                JNZ     .M1
                JMP     LDNOF
.M1             CLR
                STW     LDNSG
                STW     LDNODE
                STW     LDBASE
                STW     LDPTR
                STW     LDLIM
                LDW     LDPB
                CALL    FSFND
                JNZ     .M2             ; no such file?
                JMP     LDNOF
.M2             JMI     LDEDE           ; an error is minus
                SMB     FSUNI           ; the unit it was found on
                LDX     FSUNI
                STX     LDUNI
                CAX
                LDW     *DE.SEC         ; where it starts...
                STW     LDSEC
                LDW     *DE.CNT         ; ...and how many sectors
                STW     LDNSEC
                LDW     *DE.FLG         ; and whether it may load high
                AND     =DF.HIGH
                STW     LDHIGH
LDFND           LDX     LDPB            ; the node it was given,
                LDW     *7
                STW     LDOWN
                JNZ     .F1
                MSK                     ; or one from the arena
                CALL    NALLOC
                UNM
                JNZ     .F1
                JMP     LDEMX
.F1             STW     LDNODE
                LDW     =LDUNI          ; the file begun, its first record
                JSX     LDNREC          ; read

; The text: a code byte, then what it says follows.  The byte arrives
; zero-extended, so its lead bit is tested against X'80', not the sign.
LDPROC          JSX     LDGETB
                STW     LDCODE
                CMW     =X'0080'
                JGE     .RPT            ; below X'80': a control code
                JMP     .CTL
.RPT            SRL     4               ; the class: 8n..Cn
                SUB     =8
                CMW     =4
                JLE     .R1
                JMP     LDELC
.R1             ADD     =.RTAB          ; the class's handler, once a run
                CAX
                LDW     *0
                STW     LDCLS
                LDW     LDCODE
                AND     =X'000F'
                STW     LDREP           ; n: n+1 words follow
.RGO            JSX     LDGETW
                LDX     LDCLS
                JMP     *0
.R11            JSX     LDRL11          ; RELW11
                JMP     .RST
.RW15           ADD     LDBASE
                JMP     .RST
.RB16           ADD     LDBASE          ; a byte address: the base twice
                ADD     LDBASE
                JMP     .RST
.RB11           JSX     LDRL11          ; RELO11 twice, for the same reason
                JSX     LDRL11
.RST            JSX     LDSTOR          ; ABSO comes straight here
                LDW     LDREP
                JNZ     .RMO            ; the last of the run?
                JMP     LDPROC
.RMO            SUB     =1
                STW     LDREP
                JMP     .RGO

; The control codes, through a table of where each goes.
.CTL            LDW     LDCODE
                CMW     =LDK0C
                JGE     LDELC           ; past the table?
                ADD     =LDCTAB
                CAX
                LDW     *0
                CAX
                JMP     *0
.RTAB           WORD    .R11,.RW15,.RB16,.RB11,.RST     ; the classes, 8n..Cn
LDCTAB          WORD    LDPROC,LDNAME,LDELC,LDSMB,LDILOC,LDELC
                WORD    LDEND,LDELC,LDNAME,LDSIZW,LDSIZW,LDSIZB
                WORD    LDSEGS,LDSEGR

LDNAME          JSX     LDGETW          ; a name: four words, not wanted
                JSX     LDGETW
                JSX     LDGETW
                JSX     LDGETW
                JMP     LDPROC
LDSMB           JSX     LDGETW          ; SMB: the instruction, from the
                ADD     LDBASE          ; final address -- RELOADB's own
                SRL     10              ; ADD BASE / SRL 10 / ORI X'80'
                ORI     =X'0080'
                JSX     LDSTOR
                JMP     LDPROC
LDILOC          JSX     LDGETW          ; ILOC: that many zeros
                STW     LDCNT
.L1             LDW     LDCNT
                JNZ     .L2
                JMP     LDPROC
.L2             SUB     =1
                STW     LDCNT
                CLR
                JSX     LDSTOR
                JMP     .L1
LDSIZW          LDW     =LDALW          ; SIZE: the block, from the allocator
                JMP     LDSIZ           ; entry the code names -- or the one
LDSIZB          LDW     =LDALB          ; after it in the table, its twin
LDSIZ           STW     LDALE           ; for a module that may load high
                LDW     LDHIGH
                JNZ     .SIZH
                JMP     .SIZG
.SIZH           LDW     LDALE
                ADD     =1
                STW     LDALE
.SIZG           LDX     LDALE
                LDW     *0
                STW     LDALE
                LDW     LDNSG           ; one more than the table holds?
                CMW     =LDKSEG
                JGE     LDELC
                JSX     LDGETW
                STW     LDSIZE
                LDX     LDALE
                JSX     *0
                JNZ     .S1
                JMP     LDEMX
.S1             STW     LDW1            ; the segment's block into the table,
                LDW     LDNSG           ; and its size LDKSEG further on
                ADD     =LDSGB
                CAX
                LDW     LDW1
                STW     *0
                LDW     LDSIZE
                STW     *LDKSEG
                LDW     LDNSG
                ADD     =1
                STW     LDNSG
                SUB     =1
                JNZ     LDPROC          ; the first is where the text begins
                JMP     LDSEGG

; The two codes a module of several segments has.  0C: the text from
; here on is this segment's, stored from its first word and relocated by
; its base.  0D: the words from here on hold addresses in this segment,
; and take its base instead.
LDSEGS          JSX     LDGETW
LDSEGG          JSX     LDSEGB
                STW     LDBASE
                STW     LDPTR
                LDX     LDW1
                ADD     *LDKSEG
                STW     LDLIM
                JMP     LDPROC
LDSEGR          JSX     LDGETW
                JSX     LDSEGB
                STW     LDBASE
                JMP     LDPROC

; The block of the segment ACR numbers, LDW1 left addressing its place
; in the table.
LDSEGB          SUBR
                JMI     LDELC           ; one the text gave a SIZE for?
                CMW     LDNSG
                JGE     LDELC
                ADD     =LDSGB
                STW     LDW1
                CAX
                LDW     *0
                EXIT    LDSEGB
LDEND           JSX     LDGETW          ; END: the entry, relocated
                ADD     LDBASE
                STW     LDEXEC
                LDW     LDNSG
                JNZ     .DONE           ; without a SIZE first?
                JMP     LDELC

; Loaded: the reader's block given back, the node filled -- the console
; never its yet: RUN gives it in the window SPAWN is called in -- and
; each segment's block tagged as the new task's own, so that its exit
; finds them.
.DONE           LDW     =1
                JSX     LDREC
                LDX     LDPB
                LDW     *0              ; the name's first two characters
                STW     LDW2
                LDW     *6              ; and the terminal
                STW     LDW3
                LDX     LDNODE
                LDW     LDEXEC
                STW     *T.PCR
                LDW     LDW2
                STW     *T.NAM
                LDW     LDW3
                STW     *T.TTY
                CLR
                STW     *T.CON
                LDW     LDNODE
                ORI     =X'8000'
                STW     LDTAG
                LDW     LDNSG
                STW     LDW1
.TG             LDW     LDW1            ; the last segment first
                ADD     =LDSGB-1
                CAX
                LDW     *0
                SUB     =1
                CAX
                LDW     LDTAG
                STW     *0
                LDW     LDW1
                SUB     =1
                STW     LDW1
                JNZ     .TG

; The command tail: from the byte the caller named to its zero, in a
; block of the task's own with a zero after it.  T.ARG stays zero when
; there is nothing.
                LDX     LDPB
                LDW     *5
                JNZ     .TS             ; a tail at all?
                JMP     .TX
.TS             STW     LDTS
                STW     LDTP
.TL             LDX     LDTP
                CLR
                LDB     *0
                JNZ     .TN
                JMP     .TE
.TN             LDW     LDTP
                ADD     =1
                STW     LDTP
                JMP     .TL
.TE             LDW     LDTP
                SUB     LDTS
                JNZ     .TA             ; anything?
                JMP     .TX
.TA             ADD     =2              ; its bytes and the zero, in words
                SRL     1
                CALL    K.ALLOC
                JNZ     .TB
                JMP     LDEMX
.TB             STW     LDARG
                SUB     =1
                CAX
                LDW     LDTAG
                STW     *0
                LDW     LDARG
                SLL     1
                STW     LDTP
.TC             LDX     LDTS
                CLR
                LDB     *0
                STW     LDW1
                LDX     LDTP
                STB     *0
                LDW     LDTS
                ADD     =1
                STW     LDTS
                LDW     LDTP
                ADD     =1
                STW     LDTP
                LDW     LDW1
                JNZ     .TC             ; the zero went across too
                LDX     LDNODE
                LDW     LDARG
                STW     *T.ARG
.TX             LDW     LDNODE
                EXIT    LDMOD

LDNOF           LDW     =LE.NOF
                JMP     LDERR
LDEDE           LDW     =LE.DE
                JMP     LDERR
LDECK           LDW     =LE.CK
                JMP     LDERR
LDELC           LDW     =LE.LC
                JMP     LDERR
LDEMX           LDW     =LE.MX
LDERR           STW     LDERC           ; and give back what was taken:
                LDW     =1              ; the reader's block,
                JSX     LDREC
.R0             LDW     LDNSG           ; each segment that got a block
                JNZ     .R1
                JMP     .R2
.R1             SUB     =1
                STW     LDNSG
                ADD     =LDSGB
                CAX
                LDW     *0
                CALL    K.FREE
                JMP     .R0
.R2             LDW     LDOWN
                JNZ     .R4             ; a node it was given is not its to free
                LDW     LDNODE
                JNZ     .R3
                JMP     .R4
.R3             MSK
                CALL    NFREE
                UNM
.R4             LDW     LDERC
                EXIT    LDMOD

; RELO11: the base added into the M field, the opcode and index bit kept.
LDRL11          SUBR
                STW     LDW3
                ADD     LDBASE
                AND     =X'07FF'
                STW     LDW4
                LDW     LDW3
                AND     =X'F800'
                ORI     LDW4
                EXIT    LDRL11

; Store the next word of the module, inside the block the SIZE declared.
LDSTOR          SUBR
                STW     LDW3
                LDW     LDBASE
                JNZ     .T1             ; text before the SIZE?
                JMP     LDELC
.T1             LDW     LDPTR
                CMW     LDLIM
                JGE     LDEMX           ; room?
                CAX
                LDW     LDW3
                STW     *0
                LDW     LDPTR
                ADD     =1
                STW     LDPTR
                EXIT    LDSTOR

; A word of text, high byte first, and a byte: from the record the
; reader last handed over, which is LDLFT bytes from LDBP on -- counted,
; not compared, since it may lie above word X'4000' -- and the next
; asked for when it runs out.  Both bytes of a word at once while the
; record has them.
LDGETW          SUBR
                LDW     LDLFT
                CMW     =2
                JLT     .SL             ; both in this record?
                LDX     LDBP
                CLR
                LDB     *0
                SLL     8
                LDB     *1              ; LDB fills the low half alone
                STW     LDW2
                LDW     LDBP
                ADD     =2
                STW     LDBP
                LDW     LDLFT
                SUB     =2
                STW     LDLFT
                LDW     LDW2
                EXIT    LDGETW
.SL             JSX     LDGETB
                SLL     8
                STW     LDW2
                JSX     LDGETB
                ORI     LDW2
                EXIT    LDGETW

LDGETB          SUBR
                LDW     LDLFT
                JNZ     .B              ; anything left of the record?
                JSX     LDNREC          ; ACR zero: the next
.B              LDW     LDLFT
                SUB     =1
                STW     LDLFT
                LDX     LDBP
                CLR
                LDB     *0
                STW     LDBV
                LDW     LDBP
                ADD     =1
                STW     LDBP
                LDW     LDBV
                EXIT    LDGETB

; The next record from the reader -- ACR zero -- or ACR the address of
; the file's unit, first sector and count, to begin it: its text then
; from LDBP on.  A record failing its checksum (CK), the file running
; out first (LC) or the disc saying no (DE) is an error.
LDNREC          SUBR
                JSX     LDREC
                JNZ     .E
                LDW     LRBP
                STW     LDBP
                LDW     =LRKTXT
                STW     LDLFT
                EXIT    LDNREC
.E              JMI     LDEDE
                CMW     =1
                JEQ     LDECK
                JMP     LDELC

LDPB            WORD    0               ; the caller's eight words
LDOWN           WORD    0               ; the node they gave, or zero
LDERC           WORD    0               ; the answer an error path gives
LDARG           WORD    0               ; the command tail's block
LDHIGH          WORD    0               ; nonzero: the module may load high
LDTS            WORD    0               ; the tail's cursor, and where its
LDTP            WORD    0               ; copy goes
LDKSEG          EQU     4               ; the segments a module may have
LDNSG           WORD    0               ; how many segments have a block,
LDSGB           RES     LDKSEG+LDKSEG   ; each one's block and, LDKSEG on, its size
LDNODE          WORD    0
LDBASE          WORD    0               ; the relocation base: the block
LDPTR           WORD    0               ; where the next word goes, and the limit
LDLIM           WORD    0
LDSIZE          WORD    0               ; the SIZE code's word
LDEXEC          WORD    0               ; the entry
LDUNI           WORD    0               ; the unit the module is on, its first
LDSEC           WORD    0               ; sector and how many it has: in this
LDNSEC          WORD    0               ; order, for LDREC
LDBP            WORD    0               ; the byte pointer into the record,
LDLFT           WORD    0               ; the bytes of it left,
LDBV            WORD    0               ; and the byte it fetched
LDCODE          WORD    0               ; the code, its class's handler and its count
LDCLS           WORD    0
LDREP           WORD    0
LDCNT           WORD    0               ; ILOC's count
LDALE           WORD    0               ; the allocator entry the SIZE code names
LDTAG           WORD    0               ; the owner tag for the task's blocks
LDW1            WORD    0               ; scratch
LDW2            WORD    0
LDW3            WORD    0
LDW4            WORD    0
LDK0C           EQU     14              ; the control codes, 0..D
LDALW           WORD    ALLOCW,ALLCHW   ; the allocator's page entries, for an
LDALB           WORD    ALLOCB,ALLCHB   ; indexed JSX: each with its twin above
                                        ; the fence after it
                LPOOL                   ; what this page's code writes as literals
