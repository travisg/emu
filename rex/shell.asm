; vim: ts=8:sw=8:expandtab:
;
; REX's shell, assembled twice.  In the kernel's deck, the file after
; rex.asm, it is the console's shell in page 1: the kernel becomes it at
; start-up, its node is SHTCB, and it reads INIT.CM.  Assembled as a
; module of its own after shellmod.asm (SHMOD=1) it is SHELL.RX, which the
; console's shell loads for each of the other terminals the first time a
; key is typed there (SHGTY): a copy for each, since every routine here
; has one link slot and keeps its work in cells of its own.  So nothing
; here names the shell's node or its terminal but through SHME and SHTM,
; which the module fills in from its node as it starts, and whatever it
; reaches in the kernel it reaches as a module does, through the names
; the kernel exports.  What only the console's shell does -- INIT.CM,
; giving the other terminals their shells, HALT -- is under SHMOD=0, and
; what only a terminal's does -- BYE -- under SHMOD=1.  SHELL.RX loads
; above word X'4000', where its byte addresses are negative, so nothing
; here compares two byte addresses but for equal: a buffer is walked to
; its exact end or to a zero, and a record's bytes are counted.
;
; ---------------------------------------------------------------- the shell
; Prints the banner and then reads a line and runs it, forever.
; Everything it prints goes through its own mailbox one character at a
; time like any other task's, so a command's output and what a task
; behind the prompt prints interleave on the printer exactly as two users'
; output did.
                TRUE    SHMOD=0
                ORG     X'800'

; The console's: the only task running when the machine comes up.
SHELL           LDW     =SHBAN
                JSX     SHMSG
                LDW     =SHHNT
                JSX     SHMSG
                LDW     =SHKIN0         ; INIT.CM, if the disc has one, is
                STW     SHFN            ; read before the keyboard is
                LDW     ='IT'
                STW     SHFN+1
                CLR
                STW     SHFN+2
                LDW     ='CM'
                STW     SHFN+3
                CLR                     ; on unit 0, the system's platter
                STW     SHFN+4
                JSX     SHCMOP
                ENDC

                TRUE    SHMOD=1
; A terminal's: its node and its terminal from the node the console's
; shell spawned it in, then the banner, which says which terminal.
SHELL           SMB     CURT
                LDX     CURT
                STX     SHME
                LDW     *T.TTY
                STW     SHTM
                LDW     =SHBAN
                JSX     SHMSG
                LDX     SHTM
                LDW     *TM.NO
                JSX     SHDEC
                JSX     SHNL
                LDW     =SHHNT
                JSX     SHMSG
                ENDC

SHLOOP          CLR                     ; a kill aimed at nobody is forgotten
                LDX     SHTM
                STW     *TM.KIL
                LDW     =SHPRM
                JSX     SHMSG
                JSX     SHGETL          ; a line, however long that takes
                LDW     =SHKLBB
                STW     SHCUR
                JSX     SHSKB
                LDX     SHCUR
                CLR
                LDB     *0
                CMW     ='@'
                JEQ     .AT             ; @NAME: commands from a file
                JSX     SHTOK           ; the command word: its second half,
                LDW     STOK1           ; since a short word fills only that
                JNZ     .DSP            ; an empty line is not an error
                JMP     SHLOOP

; Walk the command table: two words of name, then the handler to jump to,
; and a zero handler ends it -- a short name leaves its first word zero.
; Only the first four characters are matched, which is how the period
; interpreters did it -- START and STAT differ inside four, and a longer
; word that starts the same is simply taken as the command.
.DSP            LDW     =SHTAB
                STW     SHTP
.DL             LDX     SHTP
                LDW     *2
                JNZ     .DCM            ; the end of the table?
                JMP     .DNF
.DCM            LDW     *0
                CMW     STOK0
                JNE     .DNX
                LDX     SHTP
                LDW     *1
                CMW     STOK1
                JNE     .DNX
                LDX     SHTP
                LDW     *2              ; matched: the handler's address
                CAX
                JMP     *0
.DNX            LDW     SHTP
                ADD     =3
                STW     SHTP
                JMP     .DL
; Not a command: then it is a program -- the word as a name, type RX,
; run as RUN would run it -- and WHAT only if the disc has no such file.
.DNF            LDW     =SHKLBB
                STW     SHCUR
                LDW     =1
                STW     LDCON
                STW     SHIMPL
                LDW     =SHKRX
                STW     SHDTY
                JSX     SHNAME
                JMP     SHLD1

; @NAME: the shell's lines come from NAME.CM until it runs out.
.AT             LDW     SHCUR
                ADD     =1
                STW     SHCUR
                LDW     ='CM'
                STW     SHDTY
                JSX     SHNAME
                JSX     SHCMOP
                JMI     .ATNO           ; no such file, or no channel
                JMP     SHLOOP
.ATNO           LDW     =SHNOF
                JSX     SHMSG
                JMP     SHLOOP

; Open the file SHFN names as the command file, in place of one that is
; open already: the channel, or minus.  SHFN+4's mode is to read.
SHCMOP          SUBR
                JSX     SHCMCL
                LDW     =SHFN
                CALL    K.OPEN
                JMI     .X
                STW     SHCMCH
.X              EXIT    SHCMOP

; Close the command file, if one is open.
SHCMCL          SUBR
                LDW     SHCMCH
                JNZ     .C1
                EXIT    SHCMCL
.C1             CALL    K.CLOSE
                CLR
                STW     SHCMCH
                EXIT    SHCMCL

; ---------------------------------------------------------------- commands
; HELP: the commands that are in the shell, and a word on the ones that
; are not.
SHHELP          LDW     =SHHL1
                JSX     SHMSG
                LDW     =SHHL2
                JSX     SHMSG
                LDW     =SHHL3
                JSX     SHMSG
                JMP     SHLOOP

; Seconds since the executive came up. SECS is the scheduler's, counted
; fifty ticks at a time.
SHUPT           JSX     SHPUP
                JMP     SHLOOP

SHPUP           SUBR
                LDW     =SHUPM
                JSX     SHMSG
                LDW     =' '
                JSX     SHPUTC
                SMB     SECS
                LDW     SECS
                JSX     SHDEC
                LDW     =SHSEC
                JSX     SHMSG
                EXIT    SHPUP

; The time of day, HH:MM:SS, off the clock (706 UM 5-18.3.5): a hold,
; then the words, the least significant last because reading it is what
; ends the hold -- masked, since the whole read has to be over inside 60
; microseconds for the clock to lose nothing.
SHTIME          MSK
                DOT     11,0            ; hold the clock
                DIN     11,1            ; 10 hr, hr, 10 min, min, 10 s
                STW     SHTW1
                DIN     11,2            ; s, 100 ms, 10 ms, ms
                STW     SHTW2
                DIN     11,3            ; tenths of a ms, which ends the hold
                UNM
                LDW     SHTW1
                SRL     14
                JSX     SHTDIG
                LDW     SHTW1
                SRL     10
                JSX     SHTDIG
                JSX     SHTCOL
                LDW     SHTW1
                SRL     7
                AND     =7
                JSX     SHTDIG
                LDW     SHTW1
                SRL     3
                JSX     SHTDIG
                JSX     SHTCOL
                LDW     SHTW1
                AND     =7
                JSX     SHTDIG
                LDW     SHTW2
                SRL     12
                JSX     SHTDIG
                JSX     SHNL
                JMP     SHLOOP

SHTDIG          SUBR                    ; the BCD digit in ACR's low four bits
                AND     =X'000F'
                ADD     ='0'
                JSX     SHPUTC
                EXIT    SHTDIG

SHTCOL          SUBR
                LDW     =':'
                JSX     SHPUTC
                EXIT    SHTCOL

SHTW1           WORD    0               ; the clock's word 1
SHTW2           WORD    0               ; and word 2

; The tasks: the uptime and the seconds' worth of ticks that found idle
; running, then each task's job number, name, state, how much longer a
; sleeper has, and OUT for one whose blocks are on the disc.  Not a walk of the ring,
; which a task leaving it part way through would send astray: the two
; nodes that are always here, in the order they lie in core, then every
; arena node that is in use, then idle.
SHSTAT          JSX     SHPUP
                LDW     =SHIDM          ; and how much of it was idle
                JSX     SHMSG
                SMB     IDSECS
                LDW     IDSECS
                JSX     SHDEC
                LDW     =SHSEC
                JSX     SHMSG
                SMB     SWSECS
                LDW     SWSECS
                JNZ     .TSW            ; a swap file: then how busy it has been
                JMP     .T0
.TSW            LDW     =SHSWP
                JSX     SHMSG
                SMB     SWOUTS
                LDW     SWOUTS
                JSX     SHDEC
                LDW     =SHSWO
                JSX     SHMSG
                SMB     SWINS
                LDW     SWINS
                JSX     SHDEC
                LDW     =SHSWI
                JSX     SHMSG
.T0             LDW     =SHSTH
                JSX     SHMSG
                LDW     =SHTCB
                STW     SHTO
                LDW     =2
                STW     SHTI
.T1             JSX     SHSTLN
                LDW     SHTO
                ADD     =T.LEN
                STW     SHTO
                LDW     SHTI
                SUB     =1
                STW     SHTI
                JNZ     .T1
                LDW     =NARB
                STW     SHTO
                LDW     =NARN
                STW     SHTI
.T2             LDX     SHTO
                LDW     *T.STA
                CMW     =SFREE
                JEQ     .T3             ; in use?
                JSX     SHSTLN
.T3             LDW     SHTO
                ADD     =T.LEN
                STW     SHTO
                LDW     SHTI
                SUB     =1
                STW     SHTI
                JNZ     .T2
                LDW     =IDTCB
                STW     SHTO
                JSX     SHSTLN
                JMP     SHLOOP

; One line of it, for the node in SHTO: its job number, its name, its
; state -- read once, since the task may change it between the name and
; the number -- then for a sleeper how long it has left, and for a waiter
; what it waits for, where that is known; CON for its terminal's
; console's holder, OUT for a task whose blocks are on the disc, and its
; terminal.
SHSTLN          SUBR
                LDX     SHTO
                LDW     *T.ID
                JSX     SHDEC
                LDW     =' '
                JSX     SHPUTC
                LDX     SHTO
                LDW     *T.NAM          ; its two-character name
                JSX     SHPW2
                LDW     =' '
                JSX     SHPUTC
                LDX     SHTO            ; its state, as four characters,
                LDW     *T.STA          ; and what it waits for, looked at
                STW     SHSS            ; with the state and before either
                JSX     SHWHY           ; takes the printer's time
                LDW     SHSS
                SLL     1
                ADD     =SHSTA
                JSX     SHP4
                LDW     SHSS
                CMW     =SSLP
                JNE     .W              ; asleep?  then for how long
                LDX     SHTO
                LDW     *T.DLY
                JSX     SHDEC
.W              LDW     SHW2
                SAZ
                JSX     SHP4
                LDX     SHTO            ; each of these four characters
                LDW     *T.CON          ; where it applies
                SAZ
                LDW     =SHCONW
                SAZ
                JSX     SHP4
                LDX     SHTO
                LDW     *T.SWP
                SAZ
                LDW     =SHOUTW
                SAZ
                JSX     SHP4
                LDW     =SHTTY          ; and the terminal
                JSX     SHMSG
                LDX     SHTO
                LDW     *T.TTY
                CAX
                LDW     *TM.NO
                JSX     SHDEC
                JSX     SHNL
                EXIT    SHSTLN

; The four characters packed in the two words ACR addresses.
SHP4            SUBR
                STW     SHSA
                CAX
                LDW     *0
                JSX     SHPW2
                LDX     SHSA
                LDW     *1
                JSX     SHPW2
                EXIT    SHP4

; What the task in SHTO, in state SHSS, waits for, as four characters'
; address in SHW2, or zero for none of these: the disc -- its lock, or a
; transfer of its -- core, the printer, its terminal's keyboard.  A task
; that is not waiting, the shell waiting for its console back and the
; swapper waiting for work are none of them.
SHWHY           SUBR
                LDW     SHSS
                CMW     =SWAI
                JNE     .Z              ; waiting?
                LDX     SHTO
                LDW     *T.DLY
                CMW     =X'FFFF'
                JEQ     .D              ; waiting for the disc's lock,
                SMB     DWTR
                LDW     DWTR
                CMW     SHTO
                JEQ     .D              ; or on a transfer?
                LDX     SHTO
                LDW     *T.MW
                JNZ     .M              ; for core?
                LDW     *T.MBX
                JNZ     .P              ; for the printer?
                LDW     *T.TTY
                CAX
                LDW     *TM.Q
                CAX
                LDW     *Q.WTR
                CMW     SHTO
                JEQ     .K              ; for a key?
.Z              CLR
                JMP     .X
.D              LDW     =SHWDSK
                JMP     .X
.M              LDW     =SHWCOR
                JMP     .X
.P              LDW     =SHWPRT
                JMP     .X
.K              LDW     =SHWKBD
.X              STW     SHW2
                EXIT    SHWHY

; STOP n and START n differ only in the state they store, into the task
; with that job number: one the shell loaded, since no other has a
; number.  Not one that has a console, which would leave a shell waiting
; on a hand-back nobody could make; and the state goes in with the disc
; claimed, so that it is never a task part way through a transfer that is
; left standing.  START with a name rather than a number loads that
; program and starts it at once, with no console.
SHSTOP          LDW     =SOFF
                STW     SHNST
                JMP     SHSSET
SHSTRT          JSX     SHSKB
                LDX     SHCUR
                CLR
                LDB     *0
                SUB     ='0'
                JMI     SHSTNM          ; a digit first: a job number
                CMW     =10
                JGE     SHSTNM
                CLR
                STW     SHNST
SHSSET          JSX     SHJOB
                JNZ     .J1
                JMP     SHLOOP
.J1             CAX
                LDW     *T.CON
                JNZ     .OBD
                JSX     SHIOC
                LDX     SHKN
                LDW     SHNST
                STW     *T.STA
                JSX     SHIOR
                JMP     SHLOOP
.OBD            LDW     =SHNOT
                JSX     SHMSG
                JMP     SHLOOP

; KILL n: the task with that job number is over, whatever it was doing.
SHKILL          JSX     SHJOB
                JNZ     .L1
                JMP     SHLOOP
.L1             JSX     SHKDO
                JMP     SHLOOP

; The job number at SHCUR as a task the shell loaded: its node, left in
; SHKN as well -- or zero, NO SUCH TASK having been said.  A terminal's
; shell has a job number of its own, which is not one of its tasks':
; stopping or ending itself in the middle of the command would leave the
; disc claimed for good.
SHJOB           SUBR
                JSX     SHSKB
                CLR
                STW     SHNV
.L              LDX     SHCUR
                CLR
                LDB     *0
                SUB     ='0'
                STW     SHW2
                JMI     .D              ; below '0'
                CMW     =10
                JGE     .D              ; above '9'
                LDW     SHNV            ; times ten, and the digit
                SLL     3
                ADD     SHNV
                ADD     SHNV
                ADD     SHW2
                STW     SHNV
                LDW     SHCUR
                ADD     =1
                STW     SHCUR
                JMP     .L
.D              LDW     =NARB           ; the arena node with that number
                STW     SHKN
                LDW     =NARN
                STW     SHTI
.F              LDX     SHKN
                LDW     *T.ID
                CMW     SHNV
                JNE     .N
                LDW     *T.STA
                CMW     =SFREE
                JEQ     .N              ; and in use
                LDW     SHKN
                CMW     SHME
                JEQ     .N              ; and not this shell itself
                EXIT    SHJOB
.N              LDW     SHKN
                ADD     =T.LEN
                STW     SHKN
                LDW     SHTI
                SUB     =1
                STW     SHTI
                JNZ     .F
                LDW     =SHNOT
                JSX     SHMSG
                CLR
                EXIT    SHJOB

; End the task in SHKN.  The disc first: with it claimed here the task
; is inside no transfer and no file call.  Then, masked, it is stopped --
; so it deposits nothing more -- and if its terminal's printer is on a
; character of its, that is waited out a tick at a time, since a
; terminal's owner must never name a node that is gone.
SHKDO           SUBR
                JSX     SHIOC
.L              MSK
                LDX     SHKN
                LDW     =SOFF
                STW     *T.STA
                LDW     *T.TTY
                CAX
                LDW     *TM.OWN
                CMW     SHKN
                JEQ     .W
                LDW     SHKN
                CALL    KILLT
                UNM
                JSX     SHIOR
                EXIT    SHKDO
.W              LDW     =1
                LDX     SHME
                STW     *T.DLY
                LDW     =SSLP
                LDX     SHME
                STW     *T.STA
                CALL    K.SWTCH
                JMP     .L

; Print the rest of the line back.
SHECHO          JSX     SHSKB
.L              LDX     SHCUR
                CLR
                LDB     *0
                JNZ     .N              ; the terminator?
                JMP     .D
.N              JSX     SHPUTC
                LDW     SHCUR
                ADD     =1
                STW     SHCUR
                JMP     .L
.D              JSX     SHNL
                JMP     SHLOOP

; Wait for the console to come back: masked look, SWAI, SWTCH, look
; again -- reading no queue, which is what keeps the terminal's queue to
; its one reader.  The console's reader is the shell exactly when the
; terminal's TM.BSY is clear, and the task RUN or ATTACH gave it to
; exactly when it is set.
SHBWT           MSK
                LDX     SHTM
                LDW     *TM.BSY
                JNZ     .B              ; handed back yet?
                JMP     .D
.B              LDW     *TM.KIL
                JNZ     .K              ; a second Ctrl-C, or a Ctrl-Z?
                TRUE    SHMOD=0
                SMB     GETTY
                LDW     GETTY
                JNZ     .Y              ; a terminal waiting for a shell?
                ENDC
                LDW     =SWAI
                LDX     SHME
                STW     *T.STA
                CALL    K.SWTCH
                JMP     SHBWT
.D              UNM
                JMP     SHLOOP
                TRUE    SHMOD=0
.Y              UNM
                JSX     SHGTY
                JMP     SHBWT
                ENDC

; Two Ctrl-Cs and the program took neither: it is not listening, and the
; task that has the console is ended from here.  A Ctrl-Z (2): it is
; stopped and keeps its place, and the console is the shell's again --
; with the disc claimed, as STOP claims it, and the holder looked at once
; more under the mask, since it may have gone of its own accord since it
; was found.
.K              STW     SHW2
                CLR
                STW     *TM.KIL
                UNM
                JSX     SHFGD
                JNZ     .G
                JMP     SHBWT           ; it went of its own accord
.G              LDW     SHW2
                CMW     =2
                JEQ     .Z
                JSX     SHKDO
                LDW     =SHKIL
                JSX     SHMSG
                JMP     SHLOOP
.Z              JSX     SHIOC
                MSK
                LDX     SHKN
                LDW     *T.STA
                CMW     =SFREE
                JEQ     .ZG             ; still there,
                LDW     *T.CON
                JNZ     .ZS             ; and still holding it?
.ZG             UNM
                JSX     SHIOR
                JMP     SHBWT
.ZS             CLR
                STW     *T.CON
                LDW     =SOFF
                STW     *T.STA
                LDX     SHTM
                CLR
                STW     *TM.BSY
                UNM
                JSX     SHIOR
                JSX     SHNL
                JSX     SHJBN
                LDW     =SHSTP
                JSX     SHMSG
                JMP     SHLOOP

; This terminal's console's holder: its node in SHKN, or zero.
SHFGD           SUBR
                LDW     =NARB
                STW     SHKN
                LDW     =NARN
                STW     SHTI
.F              LDX     SHKN
                LDW     *T.STA
                CMW     =SFREE
                JEQ     .N
                LDW     *T.TTY
                CMW     SHTM
                JNE     .N              ; this terminal's,
                LDW     *T.CON
                JNZ     .G              ; and holding it?
.N              LDW     SHKN
                ADD     =T.LEN
                STW     SHKN
                LDW     SHTI
                SUB     =1
                STW     SHTI
                JNZ     .F
                CLR
.G              EXIT    SHFGD

; "JOB n" for the node in SHKN.
SHJBN           SUBR
                LDW     =SHJOBM
                JSX     SHMSG
                LDX     SHKN
                LDW     *T.ID
                JSX     SHDEC
                EXIT    SHJBN

; ATTACH n: this terminal's task with that number is given the console,
; running unless it is asleep or waiting, and the shell waits for it to
; be handed back, as after RUN.
SHATT           JSX     SHJOB
                JNZ     .A1
                JMP     SHLOOP
.A1             CAX
                LDW     *T.TTY
                CMW     SHTM
                JEQ     .A2             ; a task of this terminal's?
                LDW     =SHNOT
                JSX     SHMSG
                JMP     SHLOOP
.A2             MSK
                JSX     SHGRT
                UNM
                JMP     SHBWT

; The console to the node in SHKN, in the caller's masked window: the
; break and the kill forgotten, so that one aimed at nobody cannot land
; on the session that follows, TM.BSY raised, the node's T.CON set, and
; its state runnable unless it is asleep or waiting, so that a task
; running behind the prompt has its sleep out.
SHGRT           SUBR
                LDX     SHTM
                CLR
                STW     *TM.BRK
                STW     *TM.KIL
                LDW     =1
                STW     *TM.BSY
                LDX     SHKN
                STW     *T.CON
                LDW     *T.STA
                CMW     =SSLP
                JEQ     .X
                CMW     =SWAI
                JEQ     .X
                CLR
                STW     *T.STA
.X              EXIT    SHGRT

; LOAD name, RUN name and START name: a module off the disc, made a
; task.  RUN hands it the console and waits for it to be over -- the
; grant made in one masked window with the SPAWN that puts the node on
; the ring (SHGRT), then SHBWT's wait; LOAD leaves it stopped, for START
; or ATTACH, and says its number; START runs it behind the prompt.  The
; node's state goes in in the same window as the SPAWN, so the task is
; never on the ring in any other, and nothing but a task that holds the
; console can lower TM.BSY.
SHRUN           LDW     =1
                STW     LDCON
                JMP     SHLD0
SHLOAD          CLR
                STW     LDCON
                LDW     =SOFF
                JMP     SHLDS
SHSTNM          CLR
                STW     LDCON
SHLDS           STW     LDSTA
SHLD0           CLR
                STW     SHIMPL
                LDW     =SHKRX
                STW     SHDTY
                JSX     SHNAME          ; the name
SHLD1           JSX     SHIOC           ; the disc, for the whole load
                JSX     LDMOD
                STW     SHW2
                JSX     SHIOR
                LDW     SHW2
                JNZ     .D2             ; a node, or an error already named?
                JMP     SHLOOP
.D2             LDW     LDNODE
                STW     SHKN
                LDW     LDCON
                JNZ     .DRN
                MSK                     ; SPAWN unmasks
                LDX     SHKN
                LDW     LDSTA
                STW     *T.STA
                LDW     SHKN
                CALL    SPAWN
                LDW     LDSTA
                JNZ     .DJB            ; LOAD's?  then its number
                JMP     SHLOOP
.DJB            JSX     SHJBN
                JSX     SHNL
                JMP     SHLOOP
.DRN            MSK
                JSX     SHGRT
                LDW     SHKN
                CALL    SPAWN
                JMP     SHBWT

                TRUE    SHMOD=0
; Give every terminal that has asked for one a shell.  SERV raises
; GETTY, and the terminal's TM.REQ, when a key is typed at a terminal
; that has none; this loads SHELL.RX as START would load a module, gives
; it the terminal, makes it the terminal's shell -- which is what lets it
; read the terminal's queue -- and starts it.  The key that asked is in
; that queue for it.  GETTY goes down first, so a request raised while
; the terminals are being walked raises it again.
SHGTY           SUBR
                CLR
                MSK
                SMB     GETTY
                STW     GETTY
                UNM
                LDW     =TMT+TM.LEN     ; the console has its shell
.T              STW     SHGT
                MSK
                CAX
                LDW     *TM.REQ
                JNZ     .R              ; this one asked?
                JMP     .N
.R              CLR
                STW     *TM.REQ
                LDW     *TM.SH
                JNZ     .N              ; and has no shell yet?
                UNM
                LDW     ='SH'           ; SHELL.RX
                STW     SHFN
                LDW     ='EL'
                STW     SHFN+1
                LDW     =X'CC00'
                STW     SHFN+2
                LDW     =SHKRX
                STW     SHFN+3
                CLR
                STW     SHFN+4          ; unit 0's
                STW     LDCON           ; given no console: it is the
                STW     SHIMPL          ; terminal's shell instead
                LDW     =SHKZB          ; and no command tail
                STW     SHCUR
                JSX     SHIOC
                JSX     LDMOD
                STW     SHW2
                JSX     SHIOR
                LDW     SHW2
                JNZ     .S              ; loaded?  if not, the loader has
                JMP     .X              ; said why
.S              LDX     LDNODE
                LDW     SHGT
                STW     *T.TTY
                MSK                     ; SPAWN unmasks
                LDX     SHGT
                LDW     LDNODE
                STW     *TM.SH
                CAX
                CLR
                STW     *T.STA
                CXA
                CALL    SPAWN
                JMP     .X
.N              UNM
.X              LDW     SHGT
                ADD     =TM.LEN
                CMW     =TMTE
                JEQ     .D              ; the last terminal?
                JMP     .T
.D              EXIT    SHGTY
                ENDC

                TRUE    SHMOD=1
; BYE: this terminal's shell is over.  The terminal has none until a key
; is typed at it again.
SHBYE           SMB     K.EXIT
                JMP     K.EXIT

; HALT is the console's: stopping every task would stop this one too.
SHNHLT          LDW     =SHNHM
                JSX     SHMSG
                JMP     SHLOOP
                ENDC

; ---------------------------------------------------------------- the loader
; Load the module SHFN names: find it in the directory and run its
; object text into a block from the pool, a record at a time from the
; kernel's reader (K.LDREC, rexrec.asm), which reads them off the disc.  The
; text is the 1968 relocating loader's (tools/asm703.py's docstring lists
; the codes, tools/reload703.py is the reference reading of them): the
; SIZE code comes first and sizes the block, whose address is the
; relocation base; each repeatable code carries a run of words that are
; stored as they are, or with the base added into the 11-bit M field,
; the whole word, or twice for a byte address; a page selection is built
; here from the final address; END names the entry, which gets the base
; too.  A module of several segments has a SIZE code and a block for each,
; up to LDKSEG of them, and two codes to say whose text is being stored
; and whose base added.  Returns the node, with its entry, name and console flag filled
; in and each block tagged as the task's own, ready for SPAWN -- or
; zero, an error having been named: no such file, the disc's status or
; no directory on it (DE), a record failing its checksum (CK), a code
; this loader does not take or text out of order (LC), no room (MX).  An
; error path jumps straight out of whatever routine it was in, as
; RELOADB's does.  The caller has claimed the disc.
LDMOD           SUBR
                LDW     SHFN            ; a name at all?
                JNZ     .M1
                JMP     LDNOF
.M1             CLR
                STW     LDNSG
                STW     LDNODE
                STW     LDBASE
                STW     LDPTR
                STW     LDLIM
                LDW     =SHFN
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
LDFND           MSK                     ; a node from the arena
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
                CMW     =SHK0C
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
LDSIZW          LDW     =SHALW          ; SIZE: the block, from the allocator
                JMP     LDSIZ           ; entry the code names -- or the one
LDSIZB          LDW     =SHALB          ; after it in the table, its twin
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

; Loaded: the reader's block given back, the node filled, and each
; segment's block tagged as the new task's own, so that its exit finds
; them.
.DONE           LDW     =1
                CALL    K.LDREC
                LDX     LDNODE
                LDW     LDEXEC
                STW     *T.PCR
                LDW     SHFN            ; the name's first two characters
                STW     *T.NAM
                LDW     LDCON
                STW     *T.CON
                LDW     SHTM            ; and the shell's terminal is its
                STW     *T.TTY
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

; The command tail: what is left of the line past the name, blanks
; skipped, in a block of the task's own with a zero after it.  T.ARG
; stays zero when there is nothing.
                JSX     SHSKB
                LDW     SHCUR
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
                SUB     SHCUR
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
.TC             LDX     SHCUR
                CLR
                LDB     *0
                STW     LDW1
                LDX     LDTP
                STB     *0
                LDW     SHCUR
                ADD     =1
                STW     SHCUR
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

LDNOF           LDW     SHIMPL          ; a word that was neither a command
                JNZ     .W              ; nor a program
                LDW     =SHNOF
                JMP     LDERR
.W              LDW     =SHWHT
                JMP     LDERR
LDEDE           LDW     =SHEDE
                JMP     LDERR
LDECK           LDW     =SHECK
                JMP     LDERR
LDELC           LDW     =SHELC
                JMP     LDERR
LDEMX           LDW     =SHEMX
LDERR           JSX     SHMSG           ; name it, and give back what was taken:
                LDW     =1              ; the reader's block,
                CALL    K.LDREC
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
.R2             LDW     LDNODE
                JNZ     .R3
                JMP     .R4
.R3             MSK
                CALL    NFREE
                UNM
.R4             CLR
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
; kernel's reader last handed over, which is LDLFT bytes from LDBP on --
; counted, not compared, since it may lie above word X'4000' -- and the
; next asked for when it runs out.  Both bytes of a word at once while
; the record has them.
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

; The next record from the kernel's reader -- ACR zero -- or ACR the
; address of the file's unit, first sector and count, to begin it: its
; text then from LDBP on.  A record failing its checksum (CK), the file
; running out first (LC) or the disc saying no (DE) is an error.
LDNREC          SUBR
                CALL    K.LDREC
                JNZ     .E
                SMB     LRBP
                LDW     LRBP
                STW     LDBP
                LDW     =LRKTXT
                STW     LDLFT
                EXIT    LDNREC
.E              JMI     LDEDE
                CMW     =1
                JEQ     LDECK
                JMP     LDELC

; The disc for the shell, and the disc given up: the claim every user of
; DXFER makes, waiting in IOWT until it succeeds, and IOREL, which wakes
; whoever waited.  A task's call through the vector makes it in KENTER;
; this is the shell's own, for the loader and the directory's readers,
; which read the disc from the shell's task.
SHIOC           SUBR
.L              MSK
                CALL    IOTRY
                JNZ     .W              ; ours?
                UNM
                EXIT    SHIOC
.W              CALL    IOWT
                JMP     .L
SHIOR           SUBR
                MSK
                CALL    IOREL
                UNM
                EXIT    SHIOR

; The words free in the pool, below the fence and above it: the free
; list's sizes added up, under the mask that every walk of the list
; holds.
SHMEM           MSK
                CLR
                STW     SHMTOT
                STW     SHMHI
                SMB     FREHD
                LDW     FREHD
.ML             JNZ     .M1             ; the end of the list?
                JMP     .MD
.M1             CAX
                CMW     =POOLH
                JGE     .MH             ; above the fence?
                LDW     *0
                ADD     SHMTOT
                STW     SHMTOT
                JMP     .MN
.MH             LDW     *0
                ADD     SHMHI
                STW     SHMHI
.MN             LDW     *1
                JMP     .ML
.MD             UNM
                LDW     =SHFRE
                JSX     SHMSG
                LDW     SHMTOT
                JSX     SHDEC
                LDW     =SHLOW
                JSX     SHMSG
                LDW     SHMHI
                JSX     SHDEC
                LDW     =SHHIG
                JSX     SHMSG
                JMP     SHLOOP

                TRUE    SHMOD=0
; Shut the machine down.  The disc first, and kept: with it claimed here
; no task is inside a transfer.  Then every loaded task is stopped, so
; it deposits nothing more, and once their mailboxes are empty and the
; printer is idle nothing of theirs can appear inside the down-message.
; Each pass stops them over again, for one that something woke.
SHHALT          JSX     SHIOC
SHHDR           MSK
                CLR
                STW     SHW2            ; their mailboxes, ORed up
                LDW     =NARB
                STW     SHTO
                LDW     =NARN
                STW     SHTI
SHHD0           LDX     SHTO
                LDW     *T.STA
                CMW     =SFREE
                JEQ     .D2             ; in use?
                LDW     =SOFF
                STW     *T.STA
                LDW     *T.MBX
                ORI     SHW2
                STW     SHW2
.D2             LDW     SHTO
                ADD     =T.LEN
                STW     SHTO
                LDW     SHTI
                SUB     =1
                STW     SHTI
                JNZ     SHHD0
                UNM
                LDW     SHW2
                JNZ     SHHDR           ; every one of them empty?
                LDW     =TMT
.OW             CAX
                LDW     *TM.OWN
                JPL     SHHDR           ; and every printer idle?
                CXA
                ADD     =TM.LEN
                CMW     =TMTE
                JNE     .OW
                LDW     =SHDWN
                JSX     SHMSG
.DW             LDX     SHME            ; and now its own last character
                LDW     *T.MBX
                JNZ     .DW
                LDX     SHTM
                LDW     *TM.OWN
                JPL     .DW
                MSK                     ; the clock keeps the time
                HLT
                ENDC

; ---------------------------------------------------------------- parsing
; Step SHCUR over blanks.
SHSKB           SUBR
.L              LDX     SHCUR
                CLR
                LDB     *0
                CMW     =' '
                JNE     .D              ; a blank?
                LDW     SHCUR
                ADD     =1
                STW     SHCUR
                JMP     .L
.D              EXIT    SHSKB

; The word at SHCUR, its first four characters packed into STOK0:STOK1 and
; the rest of it stepped over.
SHTOK           SUBR
                CLR
                STW     STOK0
                STW     STOK1
                JSX     SHSKB
                LDW     =4
                STW     STKN
.KL             LDX     SHCUR
                CLR
                LDB     *0
                JNZ     .K1             ; the terminator ends it
                JMP     .KD
.K1             CMW     =' '            ; and so does a blank
                JNE     .K2
                JMP     .KD
.K2             STW     SHW2
                LDW     STKN            ; room for another character?
                JNZ     .K3
                JMP     .KS
.K3             LDW     STOK0           ; shift the pair up one character
                LDX     STOK1
                SLLD    8
                STW     STOK0
                CXA
                ORI     SHW2
                STW     STOK1
                LDW     STKN
                SUB     =1
                STW     STKN
.KS             LDW     SHCUR
                ADD     =1
                STW     SHCUR
                JMP     .KL
.KD             EXIT    SHTOK

; The file name at SHCUR into SHFN, the type in SHDTY for one without,
; and SHCUR past it: K.NAME's reading of a name, as everything's.
SHNAME          SUBR
                LDW     SHCUR
                STW     SHNB
                LDW     SHDTY
                STW     SHNB+1
                LDW     =SHNB
                CALL    K.NAME
                LDW     SHNB
                STW     SHCUR
                EXIT    SHNAME

; ---------------------------------------------------------------- output
; One character from ACR: deposit in the shell's mailbox, kick the
; printer, and stand down until SERV reports it printed.  The deposit
; window is masked for KICK's sake, and so is each look at the mailbox
; afterwards, so that the completion cannot land between the look and the
; decision to wait on it.
SHPUTC          SUBR
                AND     =X'00FF'
                MSK
                LDX     SHME
                STW     *T.MBX
                CALL    K.KICK
                UNM
.T              MSK
                LDX     SHME
                LDW     *T.MBX
                JNZ     .B              ; printed yet?
                JMP     .D
.B              LDW     =SWAI
                LDX     SHME
                STW     *T.STA
                CALL    K.SWTCH
                JMP     .T
.D              UNM
                EXIT    SHPUTC

; The two characters packed in ACR, high half first.
SHPW2           SUBR
                STW     SHW2P
                SRL     8
                JSX     SHPUTC
                LDW     SHW2P
                JSX     SHPUTC
                EXIT    SHPW2

; The message whose two-word descriptor ACR points at: first byte, then
; one past the last.
SHMSG           SUBR
                CAX
                LDW     *0
                STW     SHSP
                LDW     *1
                STW     SHSE
                JSX     SHPRT
                EXIT    SHMSG

SHPRT           SUBR
.L              LDW     SHSP
                CMW     SHSE
                JEQ     .D
                CAX
                CLR                     ; LDB replaces only the low half (2-1)
                LDB     *0
                JSX     SHPUTC
                LDW     SHSP
                ADD     =1
                STW     SHSP
                JMP     .L
.D              EXIT    SHPRT

SHNL            SUBR
                LDW     =X'008D'
                JSX     SHPUTC
                LDW     =X'008A'
                JSX     SHPUTC
                EXIT    SHNL

; ACR in decimal, on the hardware divide.  The digits come out backwards,
; so they are laid into a small buffer from its end and printed forwards.
; The 31-bit dividend is IXR:ACR with the sign duplicated into ACR bit 0,
; which is what the shift and the copy in front of the DIV build.
SHDEC           SUBR
                STW     SHV
                LDW     =SHKDBE
                STW     SHDP
.L2             LDW     SHV
                SRL     15
                CAX
                LDW     SHV
                DIV     =10
                STW     SHV             ; the quotient
                CXA                     ; the remainder is the digit
                ADD     ='0'
                LDX     SHDP
                STB     *0
                LDW     SHDP
                SUB     =1
                STW     SHDP
                LDW     SHV
                JNZ     .L2             ; nothing left of it?
                LDW     SHDP
                ADD     =1
                STW     SHSP
                LDW     =SHKDBE
                ADD     =1
                STW     SHSE
                JSX     SHPRT
                EXIT    SHDEC

; Collect a line into the buffer.  Every character comes from the console
; queue, which puts this task to sleep until the teletype's service
; routine has one -- so the shell holds no processor at all between
; keystrokes, and a burst typed while it is busy waits in the queue
; instead of being lost.
;
; What a line is belongs here rather than in the driver: a carriage
; return or a line feed ends it, a rubout backs up over a character, and
; lower case is folded up because that is all the commands are written
; in. The rubout itself prints, since a printing terminal cannot take ink
; back; only the buffer forgets.
SHGETL          SUBR
                LDW     =SHKLBB
                STW     SHFIL
                CLR
                STW     SHLFF
.L              LDW     SHCMCH
                JNZ     .LF             ; a command file open?
                TRUE    SHMOD=0
                MSK                     ; a terminal waiting for a shell?
                SMB     GETTY           ; Looked at in the same masked
                LDW     GETTY           ; window as Q.GET's look at the
                JNZ     .LG             ; queue, so that SERV either sees
                ENDC                    ; this shell as the queue's waiter
                LDW     =QCONS          ; and nudges it, or this sees GETTY
                CALL    K.QGET
                TRUE    SHMOD=0
                CMW     =KNUDGE
                JEQ     .LH             ; the nudge SERV gave the queue?
                ENDC
                JMP     .LC
                TRUE    SHMOD=0
.LG             UNM
.LH             JSX     SHGTY
                JMP     .L
                ENDC

; From the file: a character through its channel, given bit 7 in case
; the file came from a host that leaves it off.  A carriage return ends
; a line as it does from the keyboard; a line feed ends one that has
; something in it and is otherwise passed over, so either ending, or
; both, reads as one line.  The end of the file, or Ctrl-C, closes it
; and the keyboard has the shell again -- after the last line, if that
; had no ending of its own.
.LF             LDX     SHTM
                LDW     *TM.BRK
                JNZ     .LFX
                LDW     SHCMCH
                CALL    K.GETC
                JMI     .LFX
                ORI     =X'0080'
                STW     SHCH
                LDW     =1
                STW     SHLFF
                LDW     SHCH
                CLB     X'8A'
                JNE     .LC
                LDW     SHFIL
                CMW     =SHKLBB
                JNE     .LE
                JMP     .L
.LFX            JSX     SHCMCL
                CLR
                LDX     SHTM
                STW     *TM.BRK
                LDW     SHFIL
                CMW     =SHKLBB
                JNE     .LE
                JMP     .L

.LC             STW     SHCH
                CLB     X'8D'           ; carriage return ends the line
                JEQ     .LE
                CLB     X'8A'           ; and so does a line feed, so a
                JEQ     .LE             ; script piped in with newline
                                        ; endings reads like a typed Return
                CLB     X'FF'           ; rubout
                JEQ     .LR
                CLB     X'E1'           ; below 'a'?
                JGE     .LU
                JMP     .LS
.LU             CLB     X'FA'           ; above 'z'?
                SGR
                AND     =SHKUPM         ; in range: clear bit 5
.LS             STW     SHCH
                LDW     SHFIL
                CMW     =SHKLBE         ; room for one more?
                JEQ     .L              ; no: drop it
                CAX
                LDW     SHCH
                STB     *0
                LDW     SHFIL
                ADD     =1
                STW     SHFIL
                JMP     .L
.LR             LDW     SHFIL
                CMW     =SHKLBB         ; anything to back up over?
                JNE     .LR1
                JMP     .L
.LR1            SUB     =1
                STW     SHFIL
                JMP     .L
.LE             LDW     SHFIL           ; terminate it; SHKLBE leaves room
                CAX
                CLR
                STB     *0
                LDW     SHLFF
                JNZ     .LP             ; from a file: nothing has printed
                                        ; it, so print it as the keyboard's
                EXIT    SHGETL          ; echo would have
.LP             LDW     =SHKLBB
                STW     SHSP
                LDW     SHFIL
                STW     SHSE
                JSX     SHPRT
                JSX     SHNL
                EXIT    SHGETL

; ---------------------------------------------------------------- shell data

                TRUE    SHMOD=0
SHGT            WORD    0               ; the terminal SHGTY is serving
SHZW            WORD    0               ; a zero byte, for no command tail
SHKZB           EQU     SHZW*2
                ENDC
SHTM            WORD    TMT             ; the shell's terminal
SHME            WORD    SHTCB           ; and its own node
SHCUR           WORD    0               ; the cursor into the line, a byte
SHFIL           WORD    0               ; and where SHGETL is filling it
SHCH            WORD    0               ; the character it is filing
SHTP            WORD    0               ; the command table cursor
SHTI            WORD    0               ; STAT's countdown over the nodes...
SHTO            WORD    0               ; ...and the node a walk is visiting
SHSA            WORD    0               ; the state name being printed
SHNST           WORD    0               ; the state STOP or START will store
STOK0           WORD    0               ; the command word, four characters
STOK1           WORD    0
STKN            WORD    0               ; how many of them are still wanted
SHFN            WORD    0,0,0,0,0       ; a file's name and type, as SHNAME leaves
                                        ; them, and the mode K.OPEN wants after
SHDTY           WORD    0               ; the type a name without one is given
SHIMPL          WORD    0               ; the name was the command word itself
SHCMCH          WORD    0               ; the command file's channel; zero: none
SHLFF           WORD    0               ; this line came from it
SHKN            WORD    0               ; the node a job number named
SHNV            WORD    0               ; the number
SHSS            WORD    0               ; STAT's one reading of a state
LDARG           WORD    0               ; the command tail's block
LDHIGH          WORD    0               ; nonzero: the module may load high
LDTP            WORD    0               ; and a byte pointer over the tail
SHNB            WORD    0,0,SHFN        ; SHNAME's three words for K.NAME
SHW2            WORD    0               ; scratch
SHW2P           WORD    0               ; SHPW2's, which SHPUTC must not touch
SHMTOT          WORD    0               ; MEM's running totals, below the
SHMHI           WORD    0               ; fence and above it
LDCON           WORD    0               ; the loader's: RUN (1), or not (0)
LDSTA           WORD    0               ; and the state LOAD or START gives
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
LDNSEC          WORD    0               ; order, for K.LDREC
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
SHSP            WORD    0               ; SHPRT's cursor and limit, bytes
SHSE            WORD    0
SHV             WORD    0               ; SHDEC's running value...
SHDP            WORD    0               ; ...and where its next digit goes
SHDB            RES     3               ; six digits, filled backwards
                LPOOL                   ; what this page's code writes as literals

SHKIN0          EQU     'IN'            ; INIT, as SHNAME leaves a name
SHK0C           EQU     14              ; the control codes, 0..D
SHKRX           EQU     'RX'            ; a module's type
SHKUPM          EQU     X'FFDF'         ; folds a letter to upper case
SHKLBB          EQU     LBUF*2          ; the line, and the last byte its
SHKLBE          EQU     LBUF*2+62       ; zero terminator may need
SHKDBE          EQU     SHDB*2+5        ; the last byte of the digit buffer
SHALW           WORD    ALLOCW,ALLCHW   ; the allocator's page entries, for an
SHALB           WORD    ALLOCB,ALLCHB   ; indexed JSX: each with its twin above
                                        ; the fence after it

; Four characters a state, indexed by the state doubled.  GONE is a node
; that went back to the arena while its line was being printed, and TTY
; a task waiting to be given its console.
SHSTA           WORD    'RU','N ','SL','P ','OF','F ','WA','IT','GO','NE'
                WORD    'TT','Y '
SHWDSK          WORD    ' D','SK'       ; what a waiter waits for,
SHWCOR          WORD    ' C','OR'
SHWPRT          WORD    ' P','RT'
SHWKBD          WORD    ' K','BD'
SHCONW          WORD    ' C','ON'       ; the console's holder,
SHOUTW          WORD    ' O','UT'       ; and a task out on the disc

                TRUE    SHMOD=1         ; the console's is in page 0 (rex.asm)
LBUF            RES     32              ; the line the shell is reading
                ENDC

; The commands: four characters of name, packed as SHTOK packs a word --
; a short one lands in the second word, zero-filled above -- then where
; to go. '?' is HELP under another name, and UP is UPTIME under a shorter
; one.
SHTAB           WORD    'HE','LP',SHHELP
                WORD    0,'?',SHHELP
                WORD    'ST','AT',SHSTAT
                WORD    'UP','TI',SHUPT
                WORD    0,'UP',SHUPT
                WORD    'TI','ME',SHTIME
                WORD    'ST','OP',SHSTOP
                WORD    'ST','AR',SHSTRT
                WORD    'EC','HO',SHECHO
                WORD    'KI','LL',SHKILL
                WORD    'AT','TA',SHATT
                WORD    'M','EM',SHMEM
                WORD    'LO','AD',SHLOAD
                WORD    'R','UN',SHRUN
                TRUE    SHMOD=0
                WORD    'HA','LT',SHHALT
                ENDC
                TRUE    SHMOD=1
                WORD    'HA','LT',SHNHLT
                WORD    'B','YE',SHBYE
                ENDC
                WORD    0,0,0


                TRUE    SHMOD=0
SHBAN           MSG     "REX 703 UP\r\n"
                ENDC
                TRUE    SHMOD=1
SHBAN           MSG     "REX 703 TERMINAL "
SHNHM           MSG     "HALT IS THE CONSOLE'S\r\n"
                ENDC
SHTTY           MSG     " T"
SHHNT           MSG     "TYPE HELP FOR THE COMMANDS\r\n"
SHPRM           MSG     "REX>  "
SHWHT           MSG     "WHAT\r\n"
                TRUE    SHMOD=0
SHHL1           MSG     "BUILT IN: HELP STAT UPTIME TIME ECHO MEM RUN LOAD START HALT\r\n"
                ENDC
                TRUE    SHMOD=1
SHHL1           MSG     "BUILT IN: HELP STAT UPTIME TIME ECHO MEM RUN LOAD START BYE \r\n"
                ENDC
SHHL2           MSG     "STOP, START, ATTACH AND KILL TAKE A JOB NUMBER; CTRL-Z STOPS A RUN.\r\n@NAME RUNS A FILE OF COMMANDS.\r\n"
SHFRE           MSG     "FREE "
SHNOF           MSG     "NO SUCH FILE\r\n"
SHSWP           MSG     "SWAPS "
SHSWO           MSG     " OUT "
SHSWI           MSG     " IN\r\n"
SHLOW           MSG     " LOW "
SHHIG           MSG     " HIGH\r\n"
SHEDE           MSG     "LOAD ERROR: DE\r\n"
SHECK           MSG     "LOAD ERROR: CK\r\n"
SHELC           MSG     "LOAD ERROR: LC\r\n"
SHEMX           MSG     "LOAD ERROR: MX\r\n"
SHUPM           MSG     "UPTIME"
SHIDM           MSG     "IDLE "
SHSEC           MSG     " SEC\r\n"
SHDWN           MSG     "REX 703 DOWN\r\n"
SHNOT           MSG     "NO SUCH TASK\r\n"
SHKIL           MSG     "KILLED\r\n"
SHJOBM          MSG     "JOB "
SHSTP           MSG     " STOPPED\r\n"
                TRUE    SHMOD=1         ; the console's is in page 0 (rex.asm)
SHHL3           MSG     "ANY OTHER WORD RUNS THE PROGRAM OF THAT NAME. DIR 0: LISTS THEM.\r\n"
                ENDC
SHSTH           MSG     "JOB NM STATE TERM\r\n"

                TRUE    SHMOD=0         ; the rest of page 1, which is where
                RES     X'1000'-$       ; the console's shell must end: past
                ENDC                    ; it is the pool

                TRUE    SHMOD=1         ; SHELL.RX's deck ends here
                END     SHELL
                ENDC
