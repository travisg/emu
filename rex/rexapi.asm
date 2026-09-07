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
K.ALLOC         EQU     X'10'           ; ACR = words wanted -> ACR = the block, or 0
K.ALLCW         EQU     X'11'           ; ...contained in one 2048-word page
K.ALLCB         EQU     X'12'           ; ...contained in one 1024-word byte page
K.FREE          EQU     X'13'           ; ACR = a block from K.ALLOC
K.EXIT          EQU     X'14'           ; the task is over; never returns
K.SWTCH         EQU     X'15'           ; hand the processor on; returns when next picked
K.KICK          EQU     X'16'           ; start the printer on the next full mailbox
K.QGET          EQU     X'17'           ; ACR = a queue descriptor -> ACR = the next word
K.DREAD         EQU     X'18'           ; ACR = a sector index, DRBUF = buffer -> ACR = status

; ---------------------------------------------------------------- the cells
; Words X'20'-X'2F', the blocks of levels 8-11.
CURT            EQU     X'20'           ; the running task's node
BRKREQ          EQU     X'21'           ; Ctrl-C arrived; the console's owner clears it
CONBSY          EQU     X'22'           ; the console belongs to a task, not the shell
KCONSQ          EQU     X'23'           ; the console queue's descriptor address
;                                       ; K.QGET on it is for the console's
;                                       ; owner alone: the queue holds one
;                                       ; waiter, so a second reader's
;                                       ; registration lands on the first's.
;                                       ; RUN grants the console and fills in
;                                       ; T.CON; LOAD grants nothing, so a
;                                       ; module that reads the console tests
;                                       ; T.CON at entry and refuses when it
;                                       ; is clear.
DRBUF           EQU     X'24'           ; K.DREAD's buffer: a word address, 47 words

; ---------------------------------------------------------------- a task node
T.STA           EQU     0               ; state
T.NXT           EQU     1               ; the next node on the ring
T.ACR           EQU     2               ; the saved context
T.IXR           EQU     3
T.PCR           EQU     4
T.MST           EQU     5
T.DLY           EQU     6               ; ticks left, while sleeping
T.MBX           EQU     7               ; the printer mailbox: one character, zero = empty
T.CHR           EQU     8               ; a letter task's letter, else zero
T.NAP           EQU     9               ; a letter task's sleep, in ticks
T.NAM           EQU     10              ; two characters, for STAT
T.CON           EQU     11              ; nonzero: this task holds the console --
                                        ; RUN set it, LOAD did not, and it is
                                        ; what says whether reading KCONSQ is
                                        ; this task's to do
T.LEN           EQU     12              ; words in a node

SRUN            EQU     0
SSLP            EQU     1
SOFF            EQU     2               ; suspended by the shell's STOP
SWAI            EQU     3               ; blocked on a queue or a device

; ---------------------------------------------------------------- a queue
Q.HEAD          EQU     0
Q.TAIL          EQU     1
Q.CNT           EQU     2
Q.CAP           EQU     3
Q.BUF           EQU     4               ; word address of the ring
Q.WTR           EQU     5               ; the node waiting, or -1
QW              EQU     6               ; words per descriptor
