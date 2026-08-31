; vim: ts=8:sw=8:expandtab:
;
; Tiny BASIC for the Raytheon 703 -- the interpreter core.
;
; A 16-bit integer BASIC of the Palo Alto class: variables A-Z, one array
; @(0..1023), PRINT INPUT LET IF GOTO GOSUB RETURN FOR NEXT REM END, the
; commands LIST RUN NEW, functions RND(n) ABS(n) SIZE, and BYE to leave.
; Line numbers 1-32767; lines are stored as typed and parsed at run time;
; IF takes an optional THEN and any statement after it; LET may be
; implied.  The arithmetic runs on the hardware multiply/divide option
; (section 6 of the reference manual, MPY/DIV).
;
; This file is the interpreter alone.  It is the tail of a two-file deck:
; a wrapper in front of it owns the machine -- the interrupt block, the
; console, the workspace layout -- and asm703.py assembles the pair as
; one image.  basic.asm is the wrapper for the standalone machine.
;
; What the wrapper provides, and what the core holds it to:
;
;   REXGLUE     EQU: 0 = the standalone machine, where BYE halts; 1 =
;               under an executive, where BYE jumps to the wrapper's
;               B.BYEX and the machine belongs to other tasks too
;   B.CORE      EQU: where this file begins.  The core is one contiguous
;               stream, and the whole deck -- wrapper and core together --
;               must sit inside one 2048-word page (see the ground rules)
;   W.LBUF, W.LBUFSZ, W.VARS, W.ESTK, W.OSTK, W.GSTK, W.FSTK, W.NBUF,
;   W.HEAP, W.HEAPTOP, W.ARRAY
;               EQU: the workspace.  Anything byte-addressed -- the line
;               buffer, the heap -- must sit below word X'4000' so byte
;               pointers stay positive under this machine's signed-only
;               compares
;   T.PUTW      print the byte window [ACR, T.PWEND) and return when it
;               has drained
;   T.PUTC      print the character in ACR bits 8-15
;   T.CRLF      print a carriage return and a line feed; they are
;               distinct characters here and both are wanted
;   T.GETL      read a line into W.LBUF, CR-terminated, leaving T.INPP
;               the byte address of that CR
;   T.PWEND     cell: T.PUTW's end-of-window argument
;   T.COL       cell: the print column -- CR resets it, LF leaves it,
;               anything else advances it; PRINT's comma zones read it
;   T.INPP      cell: the line buffer fill pointer, a byte address
;   T.BRK       cell: Ctrl-C was seen; B.STEP0 polls it between
;               statements and B.BREAK clears it
;   K.LBUFA     cell: W.LBUF*2, the line buffer as a byte address
;
; Ground rules, program-wide -- the wrapper's driver follows them too
; (see AGENTS.md for the manual citations):
;
;  - One accumulator, one index register, no stack.  IXR is always caller
;    scratch: it is the index, the JSX return link, the low half of the
;    double shifts, and MPY/DIV's high half, so every long-lived pointer
;    lives in a cell and is loaded at the point of use.
;  - The whole program lives in one 2048-word page, wrapper included, so
;    a direct reference reaches everything and there is no page selection
;    anywhere in this file -- the assembler's page check holds each build
;    to that (a reference that landed outside the page is a hard error,
;    not a wrong page at run time).  The two exceptions are the T.BRK
;    pairs: that cell is the wrapper's to place, and an executive's
;    wrapper points it at a kernel cell in another page, so its two
;    references keep the adjacent SMB in front (SMB governs exactly the
;    one memory reference after it, 1-3, and selecting the current page
;    costs nothing when the cell is near).  Where an SMB/JMP or SMB/JSX
;    pair does appear -- here or in a wrapper -- it is kept adjacent and
;    never made a skip target: a skip landing on the JMP would jump the
;    SMB, and one fired between the two would leave the selected page
;    live for the next memory reference wherever execution falls.
;  - No direct byte references, by the same kind of choice -- the machine
;    has them: a byte instruction uses all five EXR bits where a word
;    instruction drops the low one, so a direct LDB/STB reaches any of
;    the 64K bytes (1-3.3.1).  But the byte-page-is-half-a-word-page
;    split is the easiest thing on this machine to get wrong, and this
;    program runs in global mode, where an indexed byte access is a flat
;    16-bit address with no page arithmetic at all; so every byte access
;    is indexed through IXR from an address cell.
;  - Characters are handled bit-7-set, exactly as the teletype delivers
;    them ('A' assembles to C1 and compares equal to a received A).  Case
;    is folded where letters are recognized, not in storage.  DOT 14,14
;    strips bit 7 on the way out, so nothing here ever masks it.
;  - A skip, IXS or DXS in front of a two-word MPY/DIV would skip only the
;    opcode word; none is ever written that way.

; ------------------------------------------------- source cursor
; P.PEEK: skip blanks at the source cursor and return the character under
; it, word-clean, without consuming it.  At the end of the line it returns
; the CR, and keeps returning it -- the CR is the parser's floor.
; P.GET: the same, but consume the character (never the CR).
                ORG     B.CORE
P.PEEK          SUBR
P.PKLP          LDX     L.TXTPTR
                CLR
                LDB     *0
                CLB     X'A0'           ; blank?
                SEQ
                JMP     P.PKDN
                LDW     L.TXTPTR
                ADD     K.ONE
                STW     L.TXTPTR
                JMP     P.PKLP
P.PKDN          EXIT    P.PEEK

P.GET           SUBR
                JSX     P.PEEK
                CLB     X'8D'
                SNE
                JMP     P.GTDN          ; never step past the CR
                STW     T.CHR2
                LDW     L.TXTPTR
                ADD     K.ONE
                STW     L.TXTPTR
                LDW     T.CHR2
P.GTDN          EXIT    P.GET

T.CHR2          WORD    0               ; P.GET's scratch

; Interpreter state.
L.TXTPTR        WORD    0               ; byte address of the next source char
L.CURLIN        WORD    0               ; word address of the running record
L.PGMEND        WORD    0               ; word address one past the heap's end
S.RUNMOD        WORD    0               ; zero = immediate, else running

; Constants.  There is no immediate wider than LLB's eight bits, so every
; sixteen-bit constant is a cell.
K.ONE           WORD    1

; ========================================================== the mainline
; The READY loop, the run loop, the line editor and the keyword dispatch,
; plus the statements that need no expression stack of their own.

B.COLD          LDW     K1.HEAP         ; empty program
                STW     L.PGMEND
                CLR
                STW     S.RUNMOD
                JSX     E.RESET
                JSX     T.CRLF

; ------------------------------------------------------------ READY loop
B.RLOOP         LDW     K1.RDYD         ; prompt; the scripted harness paces
                JSX     M.MSG           ; its typing on this string
                JSX     T.GETL
                LDW     K1.LBUFA
                STW     L.TXTPTR
                JSX     P.PEEK
                CLB     X'8D'           ; empty line
                SNE
                JMP     B.RLOOP
                CLB     X'B0'           ; leading digit: edit the program
                SLS
                JMP     B.RDIG
                JMP     B.EXEC0         ; below '0': a statement
B.RDIG          CLB     X'B9'
                SGR
                JMP     B.EDIT          ; a digit
                JMP     B.EXEC0         ; above '9': a statement
B.EXEC0         CLR                     ; immediate statement
                STW     S.RUNMOD
                JMP     B.EXEC

; ---------------------------------------------------------- line editor
; A leading number edits the heap: delete any record with that number,
; then insert the new text unless the line was bare.
B.EDIT          JSX     M.GETN
                STW     S.LNO
                SAZ                     ; line 0 is not a line
                JMP     B.ED1
                JMP     E.WHAT
B.ED1           LDW     S.LNO
                JSX     L.FIND
                LDW     L.FEQ
                SAZ
                JMP     B.ED2
                JMP     B.ED3
B.ED2           JSX     L.DEL           ; replace = delete, then insert
B.ED3           JSX     P.PEEK
                CLB     X'8D'
                SNE
                JMP     B.RLOOP         ; bare number: delete only
                LDW     T.INPP          ; bytes to store, CR included
                ADD     K1.ONE
                SUB     L.TXTPTR
                STW     S.NB
                ADD     K1.ONE          ; record words = 2 + (bytes+1)/2
                SRL     1
                ADD     K1.TWO
                STW     S.RLEN
                ADD     L.PGMEND        ; room?
                CMW     K1.HPTOP
                SGR
                JMP     B.ED4
                JMP     E.SORRY
B.ED4           JSX     L.OPEN          ; open the gap at L.FP
                LDW     L.FP            ; header
                CAX
                LDW     S.LNO
                STW     *0
                LDW     S.RLEN
                STW     *1
                LDW     L.FP            ; text bytes from the line buffer
                ADD     K1.TWO
                SLL     1
                STW     L.MD
                LDW     L.TXTPTR
                STW     L.MS
                ADD     S.NB
                STW     L.ME
B.EDCP          LDX     L.MS
                CLR
                LDB     *0
                STW     L.MT
                LDW     L.MS
                ADD     K1.ONE
                STW     L.MS
                LDX     L.MD
                LDW     L.MT
                STB     *0
                LDW     L.MD
                ADD     K1.ONE
                STW     L.MD
                LDW     L.MS
                CMW     L.ME
                SEQ
                JMP     B.EDCP
                JMP     B.RLOOP

; L.FIND: set L.FP to the first record whose line number is >= ACR, or to
; the end of the heap, and L.FEQ if it is an exact match.
L.FIND          SUBR
                STW     L.FT
                CLR
                STW     L.FEQ
                LDW     K1.HEAP
                STW     L.FP
L.FDLP          LDW     L.FP
                CMW     L.PGMEND
                SLS
                JMP     L.FDX           ; off the end
                CAX
                LDW     *0              ; the record's line number
                CMW     L.FT
                SLS
                JMP     L.FD4           ; not below: this is the spot
                JMP     L.FD3           ; still below: keep walking
L.FD4           SEQ
                JMP     L.FDX           ; past it, nothing exact
                LDW     K1.ONE          ; exactly it
                STW     L.FEQ
L.FDX           EXIT    L.FIND
L.FD3           LDW     *1              ; step over the record -- IXR still
                ADD     L.FP            ; holds it after the compare
                STW     L.FP
                JMP     L.FDLP

; L.DEL: remove the record at L.FP, closing the gap with an ascending copy.
L.DEL           SUBR
                LDW     L.FP
                CAX
                LDW     *1
                STW     L.DLN           ; its length
                ADD     L.FP
                STW     L.MS            ; source: the record after it
                LDW     L.FP
                STW     L.MD
L.DLLP          LDW     L.MS
                CMW     L.PGMEND
                SLS
                JMP     L.DLDN          ; everything below is moved
                CAX
                LDW     *0
                STW     L.MT
                LDX     L.MD
                LDW     L.MT
                STW     *0
                LDW     L.MS
                ADD     K1.ONE
                STW     L.MS
                LDW     L.MD
                ADD     K1.ONE
                STW     L.MD
                JMP     L.DLLP
L.DLDN          LDW     L.PGMEND
                SUB     L.DLN
                STW     L.PGMEND
                EXIT    L.DEL

; L.OPEN: open a gap of S.RLEN words at L.FP with a descending copy, and
; grow the heap.
L.OPEN          SUBR
                LDW     L.PGMEND
                STW     L.MS            ; one past the last source word
                ADD     S.RLEN
                STW     L.PGMEND
L.OPLP          LDW     L.MS
                CMW     L.FP
                SGR
                JMP     L.OPDN          ; nothing left above the gap
                SUB     K1.ONE
                STW     L.MS
                CAX
                LDW     *0
                STW     L.MT
                LDW     L.MS
                ADD     S.RLEN
                CAX
                LDW     L.MT
                STW     *0
                JMP     L.OPLP
L.OPDN          EXIT    L.OPEN

; ------------------------------------------------------------- run loop
; B.EXEC executes the statement at the source cursor; B.NEXT is where a
; finished statement goes; B.STEP0 runs the line at L.CURLIN.
B.EXEC          JSX     P.PEEK
                CLB     X'8D'
                SNE
                JMP     B.NEXT          ; an empty statement is fine
                CLR
                STW     S.KN
B.EXLP          LDW     S.KN
                CMW     K1.KWN
                SLS
                JMP     B.IMPLT         ; table exhausted: an implied LET?
                LDW     K1.KWADRW       ; the keyword's counted string
                ADD     S.KN
                CAX
                LDW     *0
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     B.EXHIT
                LDW     S.KN
                ADD     K1.ONE
                STW     S.KN
                JMP     B.EXLP
B.EXHIT         LDW     K1.KWHNDW       ; dispatch through the parallel table
                ADD     S.KN
                CAX
                LDW     *0
                CAX
                JMP     *0

B.IMPLT         JSX     P.PEEK          ; a letter or '@' starts a LET
                CLB     X'C0'           ; '@'
                SNE
                JMP     S.LET
                AND     K1.DF
                CLB     'A'
                SLS
                JMP     B.IMP2
                JMP     E.WHAT          ; below 'A'
B.IMP2          CLB     'Z'
                SGR
                JMP     S.LET           ; a letter
                JMP     E.WHAT          ; above 'Z'

B.NEXT          LDW     S.RUNMOD
                SAZ
                JMP     B.STEP
                JMP     B.RLOOP

B.STEP          LDW     L.CURLIN        ; on to the next record
                CAX
                LDW     *1
                ADD     L.CURLIN
                STW     L.CURLIN
B.STEP0         SMB     T.BRK           ; Ctrl-C lands between statements.
                LDW     T.BRK           ; The SMB stays: T.BRK is the
                                        ; wrapper's to place, and brex.asm
                                        ; points it at a kernel cell in
                                        ; another page.
                SAZ
                JMP     B.BREAK
                LDW     L.CURLIN        ; fell off the end?
                CMW     L.PGMEND
                SLS
                JMP     B.FIN
B.STP2          LDW     L.CURLIN        ; point the cursor at its text
                ADD     K1.TWO
                SLL     1
                STW     L.TXTPTR
                JMP     B.EXEC

B.FIN           CLR
                STW     S.RUNMOD
                JMP     B.RLOOP

B.BREAK         CLR
                SMB     T.BRK           ; the SMB stays; see B.STEP0
                STW     T.BRK
                LDW     K1.BRKD
                JSX     M.MSG
                JMP     E.ERR           ; " AT n", unwind, READY

; ------------------------------------------------------------ statements
; LET, spelled out or implied: a variable A-Z or an element @(i), an equals
; sign, an expression.
S.LET           JSX     P.PEEK
                CLB     X'C0'           ; '@'
                SNE
                JMP     S.LTAR
                JSX     P.GET           ; variable letter
                AND     K1.DF
                CLB     'A'
                SLS
                JMP     S.LT1
                JMP     E.WHAT          ; below 'A'
S.LT1           CLB     'Z'
                SGR
                JMP     S.LTV           ; a letter
                JMP     E.WHAT          ; above 'Z'
S.LTV           SUB     K1.CA           ; its cell
                ADD     K1.VARSW
                STW     S.LTGT
                JMP     S.LTEQ
S.LTAR          JSX     S.SUBSC         ; @(i): the element's cell
                STW     S.LTGT
S.LTEQ          JSX     P.GET
                CLB     X'BD'           ; '='
                SNE
                JMP     S.LTE2
                JMP     E.WHAT
S.LTE2          JSX     EVAL
                LDX     S.LTGT
                STW     *0
                JMP     B.NEXT

; S.SUBSC: consume '@ ( expr )' from the cursor ('@' still under it) and
; return the element's word address.
S.SUBSC         SUBR
                JSX     P.GET           ; the '@'
                JSX     P.GET
                CLB     X'A8'           ; '('
                SNE
                JMP     S.SB2
                JMP     E.WHAT
S.SB2           JSX     EVAL
                STW     S.SBT
                JSX     P.GET
                CLB     X'A9'           ; ')'
                SNE
                JMP     S.SB3
                JMP     E.WHAT
S.SB3           LDW     S.SBT
                SAM                     ; 0 <= i < 1024
                JMP     S.SB4
                JMP     E.HOW
S.SB4           CMW     K1.1024
                SLS
                JMP     E.HOW          ; 1024 and up is out of bounds
                ADD     K1.ARRW
                EXIT    S.SUBSC

; IF expr [THEN] statement: false skips the rest of the line.
S.IF            JSX     EVAL
                SAZ
                JMP     S.IF2
                JMP     B.NEXT          ; false
S.IF2           LDW     K1.THEND        ; an optional THEN
                STW     M.LP
                JSX     M.LIT
                JMP     B.EXEC          ; and the rest is a statement

; GOTO expr / GOSUB's shared tail: find the line and run it.
S.GOTO          JSX     EVAL
S.GOX           JSX     L.FIND
                LDW     L.FEQ
                SAZ
                JMP     S.GO2
                JMP     E.HOW          ; no such line
S.GO2           LDW     L.FP
                STW     L.CURLIN
                LDW     K1.ONE
                STW     S.RUNMOD
                JMP     B.STEP0         ; through the break check, so a
                                        ; GOTO-to-self loop stays stoppable

; RUN: clear the variables, reset every stack, start at the first line.
S.RUN           LDW     K1.VARSW
                STW     L.MT
S.RNCL          LDW     L.MT
                CAX
                CLR
                STW     *0
                LDW     L.MT
                ADD     K1.ONE
                STW     L.MT
                CMW     K1.VARSE
                SLS
                JMP     S.RN2           ; all clear
                JMP     S.RNCL
S.RN2           JSX     E.RESET
                LDW     K1.ONE
                STW     S.RUNMOD
                LDW     K1.HEAP
                STW     L.CURLIN
                JMP     B.STEP0

; PRINT: items separated by ; (nothing) and , (the next eight-column
; zone); a quoted string prints itself; anything else is an expression.
; A trailing separator holds the carriage return for the next PRINT.
S.PRT           CLR
                STW     S.PSEP
S.PRLP          JSX     P.PEEK
                CLB     X'8D'
                SNE
                JMP     S.PREOL
                CLB     X'A2'           ; '"'
                SNE
                JMP     S.PRSTR
                CLB     X'BB'           ; ';'
                SNE
                JMP     S.PRSEP
                CLB     X'AC'           ; ','
                SNE
                JMP     S.PRCOM
                JSX     EVAL            ; an expression
                JSX     M.PUTN
                CLR
                STW     S.PSEP
                JMP     S.PRLP
S.PRSEP         JSX     P.GET
                LDW     K1.ONE
                STW     S.PSEP
                JMP     S.PRLP
S.PRCOM         JSX     P.GET
S.PRC2          LLB     X'A0'           ; at least one space, then out to
                JSX     T.PUTC          ; the zone boundary
                LDW     T.COL
                AND     K1.7
                SAZ
                JMP     S.PRC2
                LDW     K1.ONE
                STW     S.PSEP
                JMP     S.PRLP
S.PRSTR         JSX     P.GET           ; the opening quote
                LDW     L.TXTPTR        ; print straight out of the source
                STW     S.PRS
S.PRSC          LDX     S.PRS           ; find the closing quote
                CLR
                LDB     *0
                CLB     X'A2'
                SNE
                JMP     S.PRS2
                CLB     X'8D'           ; unterminated
                SNE
                JMP     E.WHAT
                LDW     S.PRS
                ADD     K1.ONE
                STW     S.PRS
                JMP     S.PRSC
S.PRS2          LDW     S.PRS
                STW     T.PWEND
                LDW     L.TXTPTR
                JSX     T.PUTW
                LDW     S.PRS           ; consume through the quote
                ADD     K1.ONE
                STW     L.TXTPTR
                CLR
                STW     S.PSEP
                JMP     S.PRLP
S.PREOL         LDW     S.PSEP          ; a trailing separator holds the CR
                SAZ
                JMP     B.NEXT
                JSX     T.CRLF
                JMP     B.NEXT

; LIST: every record, number then text.
S.LIST          LDW     K1.HEAP
                STW     S.LP
S.LSLP          LDW     S.LP
                CMW     L.PGMEND
                SLS
                JMP     B.NEXT          ; every record listed
                CAX
                LDW     *0
                JSX     M.PUTN
                LLB     X'A0'
                JSX     T.PUTC
                LDW     S.LP            ; the record's text
                ADD     K1.TWO
                SLL     1
                STW     S.LT
                STW     S.LE
S.LSSC          LDX     S.LE            ; find its CR
                CLR
                LDB     *0
                CLB     X'8D'
                SNE
                JMP     S.LS3
                LDW     S.LE
                ADD     K1.ONE
                STW     S.LE
                JMP     S.LSSC
S.LS3           LDW     S.LE
                STW     T.PWEND
                LDW     S.LT
                JSX     T.PUTW
                JSX     T.CRLF
                LDW     S.LP            ; next record
                CAX
                LDW     *1
                ADD     S.LP
                STW     S.LP
                JMP     S.LSLP

; The one-line statements.
S.NEW           LDW     K1.HEAP
                STW     L.PGMEND
                JMP     B.FIN
S.REM           JMP     B.NEXT
S.END           JMP     B.FIN
                TRUE    REXGLUE=0
S.BYE           HLT                     ; the operator's exit -- and the
                JMP     B.NEXT          ; harness's; RUN resumes it
                ENDC
                FALS    REXGLUE=0
S.BYE           JMP     B.BYEX          ; the executive's exit: hand the
                                        ; console back to the shell
                ENDC

; ---------------------------------------------- mainline data and cells
S.KN            WORD    0               ; dispatch trial index
S.LNO           WORD    0               ; editor: the target line number
S.NB            WORD    0               ; editor: text bytes, CR included
S.RLEN          WORD    0               ; editor: record length in words
L.FT            WORD    0               ; L.FIND target
L.FP            WORD    0               ; L.FIND result
L.FEQ           WORD    0               ; ...exact match
L.DLN           WORD    0               ; L.DEL: doomed record's length
L.MS            WORD    0               ; the copy loops' source,
L.MD            WORD    0               ; destination,
L.ME            WORD    0               ; end,
L.MT            WORD    0               ; and carried word
S.LTGT          WORD    0               ; LET's target cell
S.SBT           WORD    0               ; S.SUBSC's index
S.PSEP          WORD    0               ; PRINT: a separator was last
S.PRS           WORD    0               ; PRINT: string scan
S.LP            WORD    0               ; LIST: record
S.LT            WORD    0               ; LIST: text start (byte)
S.LE            WORD    0               ; LIST: text end (byte)

K1.ONE          WORD    1
K1.TWO          WORD    2
K1.7            WORD    7
K1.DF           WORD    X'FFDF'         ; the case fold
K1.CA           WORD    X'00C1'         ; 'A', for variable arithmetic
K1.1024         WORD    1024
K1.LBUFA        WORD    W.LBUF*2
K1.HEAP         WORD    W.HEAP
K1.HPTOP        WORD    W.HEAPTOP
K1.VARSW        WORD    W.VARS
K1.VARSE        WORD    W.VARS+26
K1.ARRW         WORD    W.ARRAY
K1.RDYD         WORD    P1RDYD          ; message descriptors for M.MSG
K1.BRKD         WORD    P1BRKD
K1.THEND        WORD    P1THEN*2
K1.KWN          WORD    15
K1.KWADRW       WORD    KWADR
K1.KWHNDW       WORD    KWHND

P1RDYD          WORD    P1RDY*2
                WORD    P1RDYEND*2
P1RDY           TEXT    "READY \r\n"
P1RDYEND        EQU     $
P1BRKD          WORD    P1BRK*2
                WORD    P1BRK*2+5       ; five characters; the pad blank
P1BRK           TEXT    "BREAK "        ; stays inside the window's edge
P1THEN          BYTE    4,'T','H','E','N'

; The keyword table: counted strings, and the parallel handler table.
KW01            BYTE    5,'P','R','I','N','T'
KW02            BYTE    5,'I','N','P','U','T'
KW03            BYTE    3,'L','E','T'
KW04            BYTE    2,'I','F'
KW05            BYTE    5,'G','O','S','U','B'
KW06            BYTE    4,'G','O','T','O'
KW07            BYTE    6,'R','E','T','U','R','N'
KW08            BYTE    3,'F','O','R'
KW09            BYTE    4,'N','E','X','T'
KW10            BYTE    3,'R','E','M'
KW11            BYTE    3,'E','N','D'
KW12            BYTE    4,'L','I','S','T'
KW13            BYTE    3,'R','U','N'
KW14            BYTE    3,'N','E','W'
KW15            BYTE    3,'B','Y','E'
KWADR           WORD    KW01*2,KW02*2,KW03*2,KW04*2,KW05*2
                WORD    KW06*2,KW07*2,KW08*2,KW09*2,KW10*2
                WORD    KW11*2,KW12*2,KW13*2,KW14*2,KW15*2
KWHND           WORD    S.PRT,S.INPUT,S.LET,S.IF,S.GOSUB
                WORD    S.GOTO,S.RET,S.FOR,S.NXT,S.REM
                WORD    S.END,S.LIST,S.RUN,S.NEW,S.BYE

; ========================================================= the evaluator
; The expression evaluator, the arithmetic, and the statements that lean on
; them.

; E.RESET: empty every stack.  RUN does this, and so does the error path.
E.RESET         SUBR
                CLR
                STW     E.SP
                STW     E.OSP
                STW     S.GSP
                STW     S.FSP
                EXIT    E.RESET

; M.MSG: print the message whose two-word window descriptor is at the word
; address in ACR.
M.MSG           SUBR
                CAX
                LDW     *1
                STW     T.PWEND
                LDW     *0
                JSX     T.PUTW
                EXIT    M.MSG

; M.LIT: try the counted keyword at byte address M.LP against the source.
; A match consumes it (case folded, leading blanks skipped) and sets M.LF;
; a miss leaves the cursor where it was.
M.LIT           SUBR
                JSX     P.PEEK          ; settle the cursor on a character
                LDW     L.TXTPTR
                STW     M.SP2
                LDW     M.LP
                CAX
                CLR
                LDB     *0
                STW     M.LC
                LDW     M.LP
                ADD     K2.ONE
                STW     M.KP
M.LTLP          LDW     M.LC
                SAZ
                JMP     M.LT2
                JMP     M.LHIT
M.LT2           LDX     M.SP2
                CLR
                LDB     *0
                AND     K2.DF
                STW     M.LT
                LDX     M.KP
                CLR
                LDB     *0
                CMW     M.LT
                SEQ
                JMP     M.LMISS
                LDW     M.SP2
                ADD     K2.ONE
                STW     M.SP2
                LDW     M.KP
                ADD     K2.ONE
                STW     M.KP
                LDW     M.LC
                SUB     K2.ONE
                STW     M.LC
                JMP     M.LTLP
M.LMISS         CLR
                STW     M.LF
                EXIT    M.LIT
M.LHIT          LDW     M.SP2
                STW     L.TXTPTR
                LDW     K2.ONE
                STW     M.LF
                EXIT    M.LIT

; M.GETN: an unsigned decimal number at the cursor, via the hardware
; multiply; more than 32767 is a HOW.
M.GETN          SUBR
                CLR
                STW     M.NV
M.GNLP          JSX     P.PEEK
                CLB     X'B0'
                SLS
                JMP     M.GNDG
                JMP     M.GNDN
M.GNDG          CLB     X'B9'
                SGR
                JMP     M.GNOK
                JMP     M.GNDN
M.GNOK          STW     M.NC
                LDW     M.NV
                MPY     K2.TEN
                STW     M.NV            ; low fifteen bits, sign duplicated
                CXA                     ; anything in the high half is
                SAZ                     ; already past 32767
                JMP     M.GNOV
                LDW     M.NV
                AND     K2.7FFF
                ADD     M.NC
                SUB     K2.B0
                SAP
                JMP     M.GNOV
                STW     M.NV
                LDW     L.TXTPTR        ; consume the digit
                ADD     K2.ONE
                STW     L.TXTPTR
                JMP     M.GNLP
M.GNOV          JMP     E.HOW
M.GNDN          LDW     M.NV
                EXIT    M.GETN

; M.PUTN: ACR as signed decimal.  The magnitude path is unsigned, which is
; what lets -32768 through without a special case: its bit pattern is its
; own magnitude, and the 31-bit dividend below carries it as IXR:ACR.
M.PUTN          SUBR
                STW     M.PV
                SAM
                JMP     M.PNPOS
                LLB     X'AD'           ; '-'
                JSX     T.PUTC
                LDW     M.PV
                CMP
                STW     M.PV
M.PNPOS         CLR
                STW     M.PD
M.PNLP          LDW     M.PV            ; the unsigned value as a 31-bit
                SRL     15              ; dividend: top bit to IXR
                CAX
                LDW     M.PV
                DIV     K2.TEN
                STW     M.PV
                CXA                     ; the remainder is the digit
                ADD     K2.B0
                STW     M.PC
                LDW     K2.NBUFW
                ADD     M.PD
                CAX
                LDW     M.PC
                STW     *0
                LDW     M.PD
                ADD     K2.ONE
                STW     M.PD
                LDW     M.PV
                SAZ
                JMP     M.PNLP
M.PNEM          LDW     M.PD            ; emit them in reverse
                SUB     K2.ONE
                SAM
                JMP     M.PNE2
                EXIT    M.PUTN
M.PNE2          STW     M.PD
                LDW     K2.NBUFW
                ADD     M.PD
                CAX
                LDW     *0
                JSX     T.PUTC
                JMP     M.PNEM

; ------------------------------------------------------------- evaluator
; The classic two-stack loop, with no recursion anywhere: parentheses,
; @(...), RND(...) and ABS(...) all sit on the operator stack as markers
; that the closing parenthesis resolves.  Operator codes and precedences:
;
;   0 (        1 +   2 -   3 *   4 /   5 unary -
;   6 =   7 <>   8 <   9 >   10 <=   11 >=
;   12 @(   13 RND(   14 ABS(
;
; A ')' with no marker open ends the expression, unconsumed -- that is what
; lets LET and S.SUBSC share the evaluator for subscripts.
EVAL            SUBR
                CLR
                STW     E.SP
                STW     E.OSP
                STW     E.NPAR
E.OPD           JSX     P.PEEK          ; operand position
                CLB     X'AD'           ; unary minus
                SNE
                JMP     E.ONEG
                CLB     X'A8'           ; '('
                SNE
                JMP     E.OPAR
                CLB     X'C0'           ; '@('
                SNE
                JMP     E.OARR
                CLB     X'B0'           ; a number
                SLS
                JMP     E.OD1
                JMP     E.ONUM0         ; below '0': try a name
E.OD1           CLB     X'B9'
                SGR
                JMP     E.ONUM          ; a digit
                JMP     E.ONUM0         ; above '9': a name
E.ONUM0         AND     K2.DF           ; a name: RND, ABS, SIZE, or a
                CLB     'A'             ; variable
                SLS
                JMP     E.OD2
                JMP     E.WHAT          ; below 'A'
E.OD2           CLB     'Z'
                SGR
                JMP     E.ONAM          ; a letter
                JMP     E.WHAT          ; above 'Z'
E.ONUM          JSX     M.GETN
                JSX     E.PUSH
                JMP     E.OPER
E.ONEG          JSX     P.GET
                LDW     K2.FIVE
                JSX     E.OPSH
                JMP     E.OPD
E.OPAR          JSX     P.GET
                CLR
                JSX     E.MARK
                JMP     E.OPD
E.OARR          JSX     E.EATLP         ; '@' then '('
                LDW     K2.TWELVE
                JSX     E.MARK
                JMP     E.OPD
E.ONAM          LDW     K2.RNDD         ; RND( ?
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     E.ORND
                LDW     K2.ABSD         ; ABS( ?
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     E.OABS
                LDW     K2.SIZD         ; SIZE ?
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     E.OSIZ
                JSX     P.GET           ; a variable, then
                AND     K2.DF
                SUB     K2.CA
                ADD     K2.VARSW
                CAX
                LDW     *0
                JSX     E.PUSH
                JMP     E.OPER
E.ORND          JSX     E.EATP          ; its '('
                LDW     K2.THIRT
                JSX     E.MARK
                JMP     E.OPD
E.OABS          JSX     E.EATP
                LDW     K2.FOURT
                JSX     E.MARK
                JMP     E.OPD
E.OSIZ          LDW     L.PGMEND
                CMP
                ADD     K2.HPTOP        ; heap top - heap end = free words
                JSX     E.PUSH
                JMP     E.OPER

E.EATLP         SUBR                    ; consume '@' and its '('
                JSX     P.GET
                JSX     E.EATP
                EXIT    E.EATLP
E.EATP          SUBR                    ; consume a required '('
                JSX     P.GET
                CLB     X'A8'
                SNE
                JMP     E.EAT2
                JMP     E.WHAT
E.EAT2          EXIT    E.EATP

E.MARK          SUBR                    ; push the marker in ACR and count it
                JSX     E.OPSH
                LDW     E.NPAR
                ADD     K2.ONE
                STW     E.NPAR
                EXIT    E.MARK

; Operator position: classify what follows the operand.
E.OPER          JSX     P.PEEK
                CLB     X'AA'           ; '*'
                SNE
                JMP     E.BMUL
                CLB     X'AF'           ; '/'
                SNE
                JMP     E.BDIV
                CLB     X'AB'           ; '+'
                SNE
                JMP     E.BADD
                CLB     X'AD'           ; '-'
                SNE
                JMP     E.BSUB
                CLB     X'BD'           ; '='
                SNE
                JMP     E.BEQ
                CLB     X'BC'           ; '<', '<>', '<='
                SNE
                JMP     E.BLT
                CLB     X'BE'           ; '>', '>='
                SNE
                JMP     E.BGT
                CLB     X'A9'           ; ')'
                SNE
                JMP     E.RPAR
                JMP     E.FIN           ; anything else ends the expression
E.BMUL          LDW     K2.THREE
                JMP     E.BIN1
E.BDIV          LDW     K2.FOUR
                JMP     E.BIN1
E.BADD          LDW     K2.ONE
                JMP     E.BIN1
E.BSUB          LDW     K2.TWO
                JMP     E.BIN1
E.BEQ           LDW     K2.SIX
E.BIN1          STW     E.OPC
                JSX     P.GET
                JMP     E.BINOP
E.BLT           JSX     P.GET
                JSX     P.PEEK
                CLB     X'BE'           ; '<>'
                SNE
                JMP     E.BNE2
                CLB     X'BD'           ; '<='
                SNE
                JMP     E.BLE2
                LDW     K2.EIGHT
                STW     E.OPC
                JMP     E.BINOP
E.BNE2          JSX     P.GET
                LDW     K2.SEVEN
                STW     E.OPC
                JMP     E.BINOP
E.BLE2          JSX     P.GET
                LDW     K2.TEN2
                STW     E.OPC
                JMP     E.BINOP
E.BGT           JSX     P.GET
                JSX     P.PEEK
                CLB     X'BD'           ; '>='
                SNE
                JMP     E.BGE2
                LDW     K2.NINE
                STW     E.OPC
                JMP     E.BINOP
E.BGE2          JSX     P.GET
                LDW     K2.ELEVEN
                STW     E.OPC

; Reduce while the stacked operator binds at least as tightly, then stack
; the new one.  Markers have precedence zero, so they stop the reduction by
; themselves.
E.BINOP         LDW     E.OSP
                SAZ
                JMP     E.BI2
                JMP     E.BPUSH
E.BI2           JSX     E.OTOP
                JSX     E.PREC
                STW     E.T2            ; stacked precedence
                LDW     E.OPC
                JSX     E.PREC
                CMW     E.T2
                SLE
                JMP     E.BPUSH         ; new binds tighter: stack it
                JSX     E.APPLY
                JMP     E.BINOP
E.BPUSH         LDW     E.OPC
                JSX     E.OPSH
                JMP     E.OPD

; A ')' closes the nearest marker; with none open it ends the expression
; and is left for the caller.
E.RPAR          LDW     E.NPAR
                SAZ
                JMP     E.RP1
                JMP     E.FIN
E.RP1           JSX     P.GET
E.RPLP          LDW     E.OSP
                SAZ
                JMP     E.RP2
                JMP     E.WHAT
E.RP2           JSX     E.OTOP
                JSX     E.PREC
                SAZ
                JMP     E.RP3           ; a real operator: reduce it
                JMP     E.RPMK          ; the marker itself
E.RP3           JSX     E.APPLY
                JMP     E.RPLP
E.RPMK          JSX     E.OPOP          ; which marker was it?
                STW     E.T2
                LDW     E.NPAR
                SUB     K2.ONE
                STW     E.NPAR
                LDW     E.T2
                CMW     K2.TWELVE       ; @(
                SNE
                JMP     E.RPARR
                CMW     K2.THIRT        ; RND(
                SNE
                JMP     E.RPRND
                CMW     K2.FOURT        ; ABS(
                SNE
                JMP     E.RPABS
                JMP     E.OPER          ; a plain parenthesis
E.RPARR         JSX     E.POP           ; @(i): fetch the element
                SAM
                JMP     E.RPA2
                JMP     E.HOW
E.RPA2          CMW     K2.1024
                SLS
                JMP     E.HOW           ; 1024 and up is out of bounds
                ADD     K2.ARRW
                CAX
                LDW     *0
                JSX     E.PUSH
                JMP     E.OPER
E.RPRND         JSX     E.POP           ; RND(n), n >= 1
                STW     M.ARG
                SAP
                JMP     E.HOW
                SAZ
                JMP     E.RPR2
                JMP     E.HOW
E.RPR2          LDW     M.SEED          ; a 16-bit xorshift step (7,9,8):
                SLL     7               ; full period over 1..65535
                ORE     M.SEED
                STW     M.SEED
                SRL     9
                ORE     M.SEED
                STW     M.SEED
                SLL     8
                ORE     M.SEED
                STW     M.SEED
                CLR                     ; magnitude, mod n, plus one: the
                CAX                     ; masked seed is a 15-bit dividend
                LDW     M.SEED
                AND     K2.7FFF
                DIV     M.ARG
                CXA                     ; the remainder
                ADD     K2.ONE
                JSX     E.PUSH
                JMP     E.OPER
E.RPABS         JSX     E.POP
                SAM
                JMP     E.RPB2
                CMP
E.RPB2          JSX     E.PUSH
                JMP     E.OPER

; End of expression: reduce what is left; exactly one operand must remain.
E.FIN           LDW     E.OSP
                SAZ
                JMP     E.FN2
                JMP     E.FN3
E.FN2           JSX     E.APPLY         ; an unclosed marker is a WHAT,
                JMP     E.FIN           ; which E.APPLY delivers itself
E.FN3           LDW     E.SP
                CMW     K2.ONE
                SEQ
                JMP     E.WHAT
                JSX     E.POP
                EXIT    EVAL

; ------------------------------------------------- evaluator plumbing
; The operand stack.
E.PUSH          SUBR
                STW     E.T
                LDW     E.SP
                CMW     K2.16
                SLS
                JMP     E.SORRY         ; too deep
                ADD     K2.ESTKW
                CAX
                LDW     E.T
                STW     *0
                LDW     E.SP
                ADD     K2.ONE
                STW     E.SP
                EXIT    E.PUSH
E.POP           SUBR
                LDW     E.SP
                SAZ
                JMP     E.PO2
                JMP     E.WHAT
E.PO2           SUB     K2.ONE
                STW     E.SP
                ADD     K2.ESTKW
                CAX
                LDW     *0
                EXIT    E.POP

; The operator stack, plus a peek at its top.
E.OPSH          SUBR
                STW     E.T
                LDW     E.OSP
                CMW     K2.16
                SLS
                JMP     E.SORRY         ; too deep
                ADD     K2.OSTKW
                CAX
                LDW     E.T
                STW     *0
                LDW     E.OSP
                ADD     K2.ONE
                STW     E.OSP
                EXIT    E.OPSH
E.OPOP          SUBR
                LDW     E.OSP
                SUB     K2.ONE
                STW     E.OSP
                ADD     K2.OSTKW
                CAX
                LDW     *0
                EXIT    E.OPOP
E.OTOP          SUBR
                LDW     E.OSP
                SUB     K2.ONE
                ADD     K2.OSTKW
                CAX
                LDW     *0
                EXIT    E.OTOP

; Precedence of the operator in ACR, from the table.
E.PREC          SUBR
                ADD     K2.PRECW
                CAX
                LDW     *0
                EXIT    E.PREC

; Apply the top operator to the top operands, through the jump table.
E.APPLY         SUBR
                JSX     E.OPOP
                ADD     K2.APLW
                CAX
                LDW     *0
                CAX
                JMP     *0
E.APDN          EXIT    E.APPLY

E.AMRK          JMP     E.WHAT          ; a marker never applies

E.AADD          JSX     E.POP
                STW     E.RHS
                JSX     E.POP
                ADD     E.RHS
                JMP     E.APSH
E.ASUB          JSX     E.POP
                STW     E.RHS
                JSX     E.POP
                SUB     E.RHS
                JMP     E.APSH
E.AMUL          JSX     E.POP           ; the low sixteen bits of the
                STW     M.ARG           ; product, reassembled from the
                JSX     E.POP           ; 31-bit IXR:ACR format
                MPY     M.ARG
                AND     K2.7FFF
                STW     M.T
                CXA
                SLL     15
                ORI     M.T
                JMP     E.APSH
E.ADIV          JSX     E.POP
                STW     M.ARG
                JSX     E.POP
                STW     M.T
                SRA     15              ; widen to the 31-bit dividend
                CAX
                LDW     M.T
                SNO                     ; a clean flag going in
                NOP
                DIV     M.ARG
                SNO
                JMP     E.HOW           ; divide by zero, or -32768/-1
                JMP     E.APSH
E.ANEG          JSX     E.POP
                CMP
                SNO                     ; negating -32768 wraps, deliberately;
                NOP                     ; keep the sticky flag clean for '/'
                JMP     E.APSH
E.AEQ           JSX     E.ACMP
                SEQ
                JMP     E.AP0
                JMP     E.AP1
E.ANE           JSX     E.ACMP
                SNE
                JMP     E.AP0
                JMP     E.AP1
E.ALT           JSX     E.ACMP
                SLS
                JMP     E.AP0
                JMP     E.AP1
E.AGT           JSX     E.ACMP
                SGR
                JMP     E.AP0
                JMP     E.AP1
E.ALE           JSX     E.ACMP
                SLE
                JMP     E.AP0
                JMP     E.AP1
E.AGE           JSX     E.ACMP
                SLS
                JMP     E.AP1
                JMP     E.AP0
E.ACMP          SUBR                    ; set the compare flip flops on
                JSX     E.POP           ; lhs ? rhs
                STW     E.RHS
                JSX     E.POP
                CMW     E.RHS
                EXIT    E.ACMP
E.AP1           LDW     K2.ONE
                JMP     E.APSH
E.AP0           CLR
E.APSH          JSX     E.PUSH
                JMP     E.APDN

; ---------------------------------------------------------------- errors
; Print the complaint, name the line when a program was running, throw
; everything away, and go back to READY.
E.WHAT          LDW     K2.WHTD
                JSX     M.MSG
                JMP     E.ERR
E.HOW           LDW     K2.HOWD
                JSX     M.MSG
                JMP     E.ERR
E.SORRY         LDW     K2.SRYD
                JSX     M.MSG
E.ERR           LDW     S.RUNMOD
                SAZ
                JMP     E.ERAT
                JMP     E.ERDN
E.ERAT          LDW     K2.ATD
                JSX     M.MSG
                LDW     L.CURLIN
                CAX
                LDW     *0
                JSX     M.PUTN
E.ERDN          JSX     T.CRLF
                JSX     E.RESET
                CLR
                STW     S.RUNMOD
                JMP     B.RLOOP

; ---------------------------------------------------- FOR / NEXT
; The frame holds the variable's cell, the limit, the step, and the word
; address of the FOR line itself; NEXT restarts the line after it.
S.FOR           JSX     P.GET
                AND     K2.DF
                CLB     'A'
                SLS
                JMP     S.FO0
                JMP     E.WHAT          ; below 'A'
S.FO0           CLB     'Z'
                SGR
                JMP     S.FO1           ; a letter
                JMP     E.WHAT          ; above 'Z'
S.FO1           SUB     K2.CA
                ADD     K2.VARSW
                STW     S.FVAR
                JSX     P.GET
                CLB     X'BD'           ; '='
                SNE
                JMP     S.FO2
                JMP     E.WHAT
S.FO2           JSX     EVAL
                LDX     S.FVAR
                STW     *0
                LDW     K2.TOD          ; TO is not optional
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     S.FO3
                JMP     E.WHAT
S.FO3           JSX     EVAL
                STW     S.FLIM
                LDW     K2.ONE          ; STEP is
                STW     S.FSTP
                LDW     K2.STPD
                STW     M.LP
                JSX     M.LIT
                LDW     M.LF
                SAZ
                JMP     S.FO4
                JMP     S.FO5
S.FO4           JSX     EVAL
                STW     S.FSTP
S.FO5           LDW     S.FSP           ; push the frame
                CMW     K2.EIGHT
                SLS
                JMP     E.SORRY         ; nested eight deep already
                SLL     2               ; four words each
                ADD     K2.FSTKW
                STW     S.FT
                CAX
                LDW     S.FVAR
                STW     *0
                LDW     S.FLIM
                STW     *1
                LDX     S.FT
                LDW     S.FSTP
                STW     *2
                LDX     S.FT
                LDW     L.CURLIN
                STW     *3
                LDW     S.FSP
                ADD     K2.ONE
                STW     S.FSP
                JMP     B.NEXT

; NEXT: find the frame for this variable, discarding anything stacked
; inside it, step, test, and either loop or fall out.
S.NXT           JSX     P.GET
                AND     K2.DF
                CLB     'A'
                SLS
                JMP     S.NX0
                JMP     E.WHAT          ; below 'A'
S.NX0           CLB     'Z'
                SGR
                JMP     S.NX1           ; a letter
                JMP     E.WHAT          ; above 'Z'
S.NX1           SUB     K2.CA
                ADD     K2.VARSW
                STW     S.FVAR
S.NXSCN         LDW     S.FSP           ; scan down for it
                SAZ
                JMP     S.NX2
                JMP     E.HOW           ; NEXT without FOR
S.NX2           SUB     K2.ONE
                SLL     2
                ADD     K2.FSTKW
                STW     S.FT
                CAX
                LDW     *0
                CMW     S.FVAR
                SNE
                JMP     S.NX3           ; this is the loop
                JMP     S.NXPOP         ; an inner loop left behind
S.NXPOP         LDW     S.FSP
                SUB     K2.ONE
                STW     S.FSP
                JMP     S.NXSCN
S.NX3           LDX     S.FT            ; step the variable
                LDW     *2
                STW     S.FSTP
                LDX     S.FVAR
                LDW     *0
                ADD     S.FSTP
                SNO                     ; wrap silently, flag clean
                NOP
                LDX     S.FVAR
                STW     *0
                STW     S.FT2
                LDW     S.FSTP          ; which way is the test?
                SAM
                JMP     S.NXUP
                JMP     S.NXDWN
S.NXUP          LDX     S.FT
                LDW     *1              ; limit
                CMW     S.FT2
                SLS
                JMP     S.NXGO          ; var <= limit: go around
                JMP     S.NXOUT         ; var passed it: done
S.NXDWN         LDX     S.FT
                LDW     *1
                CMW     S.FT2
                SGR
                JMP     S.NXGO          ; var >= limit: go around
                JMP     S.NXOUT
S.NXGO          LDX     S.FT            ; loop: rerun from the FOR line
                LDW     *3
                STW     L.CURLIN
                JMP     B.STEP
S.NXOUT         LDW     S.FSP
                SUB     K2.ONE
                STW     S.FSP
                JMP     B.NEXT

; ---------------------------------------------------- GOSUB / RETURN
; The frame is one word: the line the GOSUB was on.  RETURN steps past it.
S.GOSUB         JSX     EVAL
                STW     S.FT2
                LDW     S.GSP
                CMW     K2.EIGHT
                SLS
                JMP     E.SORRY         ; eight calls deep already
                ADD     K2.GSTKW
                CAX
                LDW     L.CURLIN
                STW     *0
                LDW     S.GSP
                ADD     K2.ONE
                STW     S.GSP
                LDW     S.FT2
                JMP     S.GOX           ; find it and run it, like GOTO

S.RET           LDW     S.GSP
                SAZ
                JMP     S.RT2
                JMP     E.HOW           ; RETURN without GOSUB
S.RT2           SUB     K2.ONE
                STW     S.GSP
                ADD     K2.GSTKW
                CAX
                LDW     *0
                STW     L.CURLIN
                JMP     B.STEP

; ---------------------------------------------------------------- INPUT
; INPUT var[,var]...: prompt with '? ' and read a signed number for each.
; Anything unparseable just asks again.  Only meaningful while a program
; is running -- in immediate mode the statement itself lives in the very
; buffer the reply would land in, so it is refused.
S.INPUT         LDW     S.RUNMOD
                SAZ
                JMP     S.IN1
                JMP     E.HOW
S.IN1           JSX     P.PEEK          ; the target
                CLB     X'C0'
                SNE
                JMP     S.INAR
                JSX     P.GET
                AND     K2.DF
                CLB     'A'
                SLS
                JMP     S.IN1A
                JMP     E.WHAT          ; below 'A'
S.IN1A          CLB     'Z'
                SGR
                JMP     S.IN2           ; a letter
                JMP     E.WHAT          ; above 'Z'
S.IN2           SUB     K2.CA
                ADD     K2.VARSW
                STW     S.ITGT
                JMP     S.INASK
S.INAR          JSX     S.SUBSC
                STW     S.ITGT
S.INASK         LDW     L.TXTPTR        ; the program cursor, kept safe
                STW     S.ITP           ; while the reply overwrites LBUF
S.INAGN         LDW     K2.ASKD
                JSX     M.MSG
                JSX     T.GETL
                LDW     K.LBUFA         ; parse the reply instead
                STW     L.TXTPTR
                CLR
                STW     S.ISGN
                JSX     P.PEEK
                CLB     X'AD'           ; a leading minus
                SNE
                JMP     S.INMIN
                JMP     S.INDIG
S.INMIN         JSX     P.GET
                LDW     K2.ONE
                STW     S.ISGN
S.INDIG         JSX     P.PEEK          ; a digit had better follow
                CLB     X'B0'
                SLS
                JMP     S.IND2
                JMP     S.INAGN         ; below '0': ask again
S.IND2          CLB     X'B9'
                SGR
                JMP     S.IN3           ; a digit
                JMP     S.INAGN         ; above '9': ask again
S.IN3           JSX     M.GETN
                STW     S.FT2
                LDW     S.ISGN
                SAZ
                JMP     S.IN4
                JMP     S.IN5
S.IN4           LDW     S.FT2
                CMP
                STW     S.FT2
S.IN5           LDW     S.FT2
                LDX     S.ITGT
                STW     *0
                LDW     S.ITP           ; back to the program text
                STW     L.TXTPTR
                JSX     P.PEEK
                CLB     X'AC'           ; ',' and around again
                SNE
                JMP     S.IN6
                JMP     B.NEXT
S.IN6           JSX     P.GET
                JMP     S.IN1

; --------------------------------------------- evaluator data and cells
E.SP            WORD    0               ; operand stack depth
E.OSP           WORD    0               ; operator stack depth
E.NPAR          WORD    0               ; open markers
E.T             WORD    0
E.T2            WORD    0
E.OPC           WORD    0               ; the operator being stacked
E.RHS           WORD    0
M.ARG           WORD    0               ; MPY/DIV operand cell
M.T             WORD    0
M.SEED          WORD    1               ; RND state; never zero
M.LP            WORD    0               ; M.LIT: the keyword (byte address)
M.LF            WORD    0               ; M.LIT: matched
M.LC            WORD    0
M.KP            WORD    0
M.SP2           WORD    0
M.LT            WORD    0
M.NV            WORD    0               ; M.GETN value
M.NC            WORD    0
M.PV            WORD    0               ; M.PUTN value
M.PD            WORD    0
M.PC            WORD    0
S.GSP           WORD    0               ; GOSUB stack depth
S.FSP           WORD    0               ; FOR stack depth
S.FVAR          WORD    0
S.FLIM          WORD    0
S.FSTP          WORD    0
S.FT            WORD    0
S.FT2           WORD    0
S.ITGT          WORD    0               ; INPUT target cell
S.ITP           WORD    0               ; INPUT: saved program cursor
S.ISGN          WORD    0

K2.ONE          WORD    1
K2.TWO          WORD    2
K2.THREE        WORD    3
K2.FOUR         WORD    4
K2.FIVE         WORD    5
K2.SIX          WORD    6
K2.SEVEN        WORD    7
K2.EIGHT        WORD    8
K2.NINE         WORD    9
K2.TEN2         WORD    10
K2.ELEVEN       WORD    11
K2.TWELVE       WORD    12
K2.THIRT        WORD    13
K2.FOURT        WORD    14
K2.16           WORD    16
K2.TEN          WORD    10              ; MPY/DIV's radix
K2.7FFF         WORD    X'7FFF'
K2.B0           WORD    X'00B0'         ; '0'
K2.CA           WORD    X'00C1'         ; 'A'
K2.DF           WORD    X'FFDF'
K2.1024         WORD    1024
K2.ESTKW        WORD    W.ESTK
K2.OSTKW        WORD    W.OSTK
K2.GSTKW        WORD    W.GSTK
K2.FSTKW        WORD    W.FSTK
K2.NBUFW        WORD    W.NBUF
K2.VARSW        WORD    W.VARS
K2.ARRW         WORD    W.ARRAY
K2.HPTOP        WORD    W.HEAPTOP
K2.TOD          WORD    P2TO*2          ; M.LIT keywords
K2.STPD         WORD    P2STEP*2
K2.RNDD         WORD    P2RND*2
K2.ABSD         WORD    P2ABS*2
K2.SIZD         WORD    P2SIZE*2
K2.WHTD         WORD    P2WHTD          ; M.MSG descriptors
K2.HOWD         WORD    P2HOWD
K2.SRYD         WORD    P2SRYD
K2.ATD          WORD    P2ATD
K2.ASKD         WORD    P2ASKD

; The operator precedence and apply tables, indexed by operator code.
K2.PRECW        WORD    PRECT
K2.APLW         WORD    APLT
PRECT           WORD    0,2,2,3,3,4,1,1,1,1,1,1,0,0,0
APLT            WORD    E.AMRK,E.AADD,E.ASUB,E.AMUL,E.ADIV
                WORD    E.ANEG,E.AEQ,E.ANE,E.ALT,E.AGT
                WORD    E.ALE,E.AGE,E.AMRK,E.AMRK,E.AMRK

P2TO            BYTE    2,'T','O'
P2STEP          BYTE    4,'S','T','E','P'
P2RND           BYTE    3,'R','N','D'
P2ABS           BYTE    3,'A','B','S'
P2SIZE          BYTE    4,'S','I','Z','E'

P2WHTD          WORD    P2WHT*2
                WORD    P2WHT*2+5
P2WHT           TEXT    "WHAT? "
P2HOWD          WORD    P2HOW*2
                WORD    P2HOW*2+4
P2HOW           TEXT    "HOW?  "
P2SRYD          WORD    P2SRY*2
                WORD    P2SRY*2+5
P2SRY           TEXT    "SORRY "
P2ATD           WORD    P2AT*2
                WORD    P2AT*2+4
P2AT            TEXT    " AT "
P2ASKD          WORD    P2ASK*2
                WORD    P2ASK*2+2
P2ASK           TEXT    "? "

                END
