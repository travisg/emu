; vim: ts=8:sw=8:expandtab:
;
; K.NAME, REX's one reading of a file name -- the file of the kernel's
; deck after rexswap.asm, in the rest of its page, where the rings of
; the other terminals' input queues come after it.  The shell, rexlib and
; BASIC all hand their text to it, so a name means the same at the
; prompt, to a utility and to SAVE.
;
; ACR addresses three words: the cursor, a byte address, which comes back
; past the name; the type a name without one is given; and the address of
; five words the name goes into, as K.OPEN takes one.  Blanks in front are
; passed over.  Then 0: to 3: is the unit, into the fifth word's high
; byte -- X'80' for none, which leaves the kernel to choose (K.OPEN) --
; with zero, to read, in its low.  Then up to six characters of name and,
; after a dot, two of type, each left-justified and zero-filled, the rest
; of an over-long word stepped over.  A zero ends the name, and so does a
; blank or any character below one; lower case goes in as upper.  The
; answer is the name's first word, zero for no name.
;
; Any task may call it, so it runs masked, which is what makes its cells
; its own: a few dozen instructions a character.

KNPARS          SUBR
                STW     KNB
                CAX
                LDW     *0
                STW     KNC
                LDW     *2
                STW     KNO
                LDW     *1
                LDX     KNO
                STW     *3
                CLR
                STW     *0
                STW     *1
                STW     *2
                LDW     =X'8000'
                STW     *4
.SK             LDX     KNC             ; past the blanks
                CLR
                LDB     *0
                CMW     =' '
                JNE     .U
                JSX     KNADV
                JMP     .SK
.U              CLR
                LDB     *1
                CMW     =':'
                JNE     .GO             ; a unit?
                CLR
                LDB     *0
                SUB     ='0'
                STW     KNT
                AND     =X'FFFC'
                JNZ     .GO             ; 0 to 3?
                LDW     KNT
                SLL     8
                LDX     KNO
                STW     *4
                JSX     KNADV
                JSX     KNADV
.GO             LDW     KNO
                SLL     1
                STW     KNQ
                LDW     =6
                STW     KNN
.L              LDX     KNC
                CLR
                LDB     *0
                JNZ     .C              ; a zero ends it
                JMP     .X
.C              CMW     =X'A1'
                JLT     .X              ; and so does a blank or below
                CMW     ='.'
                JEQ     .DOT            ; a dot starts the type
                CMW     =X'E1'
                JLT     .K              ; lower case?
                AND     =X'00DF'
.K              STW     KNT
                LDW     KNN
                JNZ     .P              ; room for it?
                JMP     .N
.P              SUB     =1
                STW     KNN
                LDX     KNQ
                LDW     KNT
                STB     *0
                LDW     KNQ
                ADD     =1
                STW     KNQ
.N              JSX     KNADV
                JMP     .L
.DOT            LDX     KNO
                CLR
                STW     *3
                LDW     KNO
                SLL     1
                ADD     =6
                STW     KNQ
                LDW     =2
                STW     KNN
                JMP     .N
.X              LDX     KNB             ; the cursor goes back
                LDW     KNC
                STW     *0
                LDX     KNO
                LDW     *0
                EXIT    KNPARS

KNADV           SUBR
                LDW     KNC
                ADD     =1
                STW     KNC
                EXIT    KNADV

KNB             WORD    0               ; the caller's three words,
KNC             WORD    0               ; the cursor,
KNO             WORD    0               ; the name's five words,
KNQ             WORD    0               ; where the next character goes,
KNN             WORD    0               ; how many more it will take,
KNT             WORD    0               ; and the character in hand
                LPOOL

; The terminals' input queues' rings, out of page 0, where their
; descriptors are: Q.PUT and Q.GET reach a ring by its descriptor's word
; address, so it may be anywhere.
QCONSB          RES     QCONSN
TMQ1B           RES     TMQN
TMQ2B           RES     TMQN
TMQ3B           RES     TMQN
