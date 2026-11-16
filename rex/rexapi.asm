; vim: ts=8:sw=8:expandtab:
;
; The REX kernel interface, for what loads under it.
;
; A module is assembled from word 0 and placed by the loader wherever the
; allocator finds room, so it can know nothing of the kernel's layout
; except what this file states: a jump vector at fixed words, the cells
; the kernel exports, the shape of a task node and the values that go in
; it.  Every module deck begins with this file.  rex.asm keeps its own
; labels for the same things, and the build holds the two to each other:
; tools/apicheck703.py reads this file against rex's listing.
;
; A module reaches an entry with an adjacent SMB/JSX pair and a cell with
; an adjacent SMB/LDW or SMB/STW pair --
;
;       SMB     K.QGET
;       JSX     K.QGET
;
; which assemble to absolute words with a page selection in front, so
; nothing is linked at load time.  JSX leaves the link in IXR and forces
; global mode; the vector's JMP carries it into the kernel, and the
; kernel's EXIT returns through it to wherever the module lies.
;
; A task runs global -- its status word says so -- and gives the processor
; up either through K.SWTCH, after marking its own node sleeping or
; waiting in one masked window, or for good through K.EXIT, which frees
; everything the task allocated, its node included, and never returns.

; ---------------------------------------------------------------- the vector
; Words X'10'-X'1F' are the interrupt blocks of levels 4-7, which nothing on
; this machine signals; the kernel keeps its vector there.
K.ALLOC         EQU     X'10'           ; ACR = words wanted -> ACR = the block, or 0.
;                                       ; These three hand out core below word
;                                       ; X'4000' only
K.ALLCW         EQU     X'11'           ; ...contained in one 2048-word page
K.ALLCB         EQU     X'12'           ; ...contained in one 1024-word byte page
K.FREE          EQU     X'13'           ; ACR = a block from K.ALLOC
K.EXIT          EQU     X'14'           ; the task is over; never returns
K.SWTCH         EQU     X'15'           ; hand the processor on; returns when next picked
K.KICK          EQU     X'16'           ; start the printer on the next full mailbox
K.QGET          EQU     X'17'           ; ACR = a queue descriptor -> ACR = the next word
K.DREAD         EQU     X'18'           ; ACR = a transfer block -> ACR = status: disc to core
K.DWRIT         EQU     X'19'           ; the same, core to disc
K.ALLHI         EQU     X'1A'           ; K.ALLOC and K.ALLCW for a caller that
K.ALHIW         EQU     X'1B'           ; compares no byte addresses in what it
;                                       ; gets: above word X'4000' if there is
;                                       ; room there, below if not.  A byte
;                                       ; address up there is negative as a
;                                       ; signed number, which is the only kind
;                                       ; of compare this machine has; a byte
;                                       ; is still reached through the index.
;                                       ; A transfer block is three words of
;                                       ; the caller's own: the first sector's
;                                       ; index, track*128+sector; the word
;                                       ; count, up to 16,383, which may run
;                                       ; across sectors and tracks; and the
;                                       ; core address.  Status zero is clean.
;                                       ; The call waits its turn at the disc.

; Files, at X'28'-X'2E'.  A name is four words, as the directory holds
; one: six characters and then two of type, bit 7 set, each left-justified
; and zero-filled -- HELLO.RX is 'HE','LL',X'CF00','RX'.  A file is read
; or written a character at a time through a channel, of which the kernel
; has four; what a task leaves open is let go when it exits, and a file it
; was writing and never closed is gone.  A new file replaces an older one
; of its name when it is closed, not before, and has half of the largest
; run of free sectors to grow into.  Every answer that is not
; what was asked for is one of the E. values below, all minus.
K.LOOK          EQU     X'28'           ; ACR = eight words, the first four a name
;                                       ; -> zero and the last four filled in --
;                                       ; DE.SEC, DE.CNT, DE.LST, DE.FLG -- or E.
K.OPEN          EQU     X'29'           ; ACR = five words: a name, then zero to
;                                       ; read or one to write -> a channel, or E.
K.GETC          EQU     X'2A'           ; ACR = a channel -> the next character,
;                                       ; or E.NOF at the end of the file
K.PUTC          EQU     X'2B'           ; ACR = channel*256 + character -> zero, or E.
K.CLOSE         EQU     X'2C'           ; ACR = a channel -> zero, or E.
K.DELET         EQU     X'2D'           ; ACR = a name -> zero, or E.
K.RENAM         EQU     X'2E'           ; ACR = eight words, the old name and the
;                                       ; new -> zero, or E.
K.SQUSH         EQU     X'2F'           ; -> zero, or E.: every file moved down
;                                       ; over the hole in front of it.  E.CHN
;                                       ; while any channel is open.

; ---------------------------------------------------------------- the cells
; Words X'20'-X'23', the block of level 8.
CURT            EQU     X'20'           ; the running task's node
BRKREQ          EQU     X'21'           ; Ctrl-C arrived; the console's owner clears it
CONBSY          EQU     X'22'           ; the console belongs to a task, not the shell
KCONSQ          EQU     X'23'           ; the console queue's descriptor address
;                                       ; K.QGET on it is for the console's
;                                       ; owner alone: the queue holds one
;                                       ; waiter.  RUN grants the console and
;                                       ; fills in T.CON; LOAD grants nothing,
;                                       ; and a task without it gets X'FFFF'
;                                       ; from K.QGET at once.  A module that
;                                       ; reads the console tests T.CON at
;                                       ; entry and says so when it is clear.

; ---------------------------------------------------------------- a task node
T.STA           EQU     0               ; state
T.NXT           EQU     1               ; the next node on the ring
T.ACR           EQU     2               ; the saved context
T.IXR           EQU     3
T.PCR           EQU     4
T.MST           EQU     5
T.DLY           EQU     6               ; ticks left, while sleeping
T.MBX           EQU     7               ; the printer mailbox: one character, zero = empty
T.NAM           EQU     8               ; two characters, for STAT
T.CON           EQU     9               ; nonzero: this task holds the console --
                                        ; RUN set it, LOAD did not, and it is
                                        ; what says whether reading KCONSQ is
                                        ; this task's to do
T.ID            EQU     10              ; the job number
T.KLK           EQU     11              ; the kernel's, while the task is
T.KAR           EQU     12              ; blocked in one of its calls
T.KDS           EQU     13
T.ARG           EQU     14              ; the command tail: the word address of
                                        ; what followed the program's name on
                                        ; its line, a zero byte after it -- or
                                        ; zero, if nothing did
T.SWP           EQU     15              ; nonzero: the task is swapped out
T.MW            EQU     16              ; the kernel's, while the task waits for core
T.RES           EQU     17              ; the second its blocks last arrived in core
T.LEN           EQU     18              ; words in a node

SRUN            EQU     0
SSLP            EQU     1
SOFF            EQU     2               ; suspended by the shell's STOP
SWAI            EQU     3               ; blocked on a queue or a device

; ---------------------------------------------------------------- a file
DE.NAM          EQU     0               ; the name: three words
DE.TYP          EQU     3               ; the type: one
DE.SEC          EQU     4               ; the first sector
DE.CNT          EQU     5               ; how many sectors
DE.LST          EQU     6               ; bytes used in the last one
DE.FLG          EQU     7               ; DK.FILE, and perhaps DF.HIGH or DF.SYS
DE.LEN          EQU     8
DK.FILE         EQU     2
DF.HIGH         EQU     X'0100'         ; a module that may load above X'4000'
DF.SYS          EQU     X'0200'         ; never moved, renamed or deleted

E.NOF           EQU     X'FFFF'         ; no such file; K.GETC's end of file
E.CHN           EQU     X'FFFE'         ; no channel free, or not this task's, or the wrong way
E.DFUL          EQU     X'FFFD'         ; the directory is full
E.FULL          EQU     X'FFFC'         ; no hole, or the file has filled its hole
E.DISC          EQU     X'FFFB'         ; the disc is not ready, or a transfer failed
E.NDIR          EQU     X'FFFA'         ; the platter has no directory
E.PROT          EQU     X'FFF9'         ; a system file
E.EXST          EQU     X'FFF8'         ; the new name is taken

; ---------------------------------------------------------------- a queue
Q.HEAD          EQU     0
Q.TAIL          EQU     1
Q.CNT           EQU     2
Q.CAP           EQU     3
Q.BUF           EQU     4               ; word address of the ring
Q.WTR           EQU     5               ; the node waiting, or -1
QW              EQU     6               ; words per descriptor
