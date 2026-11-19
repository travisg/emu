# Writing Raytheon 703 assembly

How to write a program for the 703 with `tools/asm703.py`: what the machine
gives you, the idioms the guests in this tree are built from, and the
mistakes it makes easy. It is a guide to the references, not a replacement
for them:

| for | read |
|---|---|
| the assembler's syntax, literals, macros, libraries, modules, warnings | the docstring of `tools/asm703.py` |
| a complete small program | `demo.asm` |
| the rules a large one lives by | the header of `bcore.asm` |
| a program that loads under REX | `../../rex/hello.asm`, and "Modules" in `../../rex/README.md` |
| the macros | `../../rex/rexmac.asm` |
| every device function code | `../../src/dev/ray703.rs` |
| what the machine does with an instruction | `../../src/cpu/ray703.rs`, which cites the manual's sections |
| how Raytheon wrote it in 1968 | `listings/` |

Every example below is assembled with `--werror` and run on the emulator.

## The machine

One accumulator (ACR), one index register (IXR), 32K words of core. Bit 0 is
the most significant bit. There is no stack, no constant in an instruction
wider than the eight bits of `LLB`, no conditional branch and no carry.

**IXR is scratch, always.** It is the index, the return address `JSX`
leaves, the low half of the double shifts and the high half of `MPY` and
`DIV`. A pointer that has to last lives in a cell and is loaded where it is
used. A subroutine takes its argument in ACR or in cells, never in IXR, since
the `JSX` that reaches it has just overwritten IXR.

**Pages.** A memory reference carries an 11-bit address, so it reaches the
2048-word page its own instruction is in. Anything else needs a page
selection in front of it, and `SMB NAME` writes the right one:

            SMB     FAR
            LDW     FAR

A selection governs exactly the next memory reference; the two cards stay
together, and the second is never the target of a skip or a jump. The
assembler refuses a reference outside the page with no selection in front.

**Indexing** is `*`: `LDW *3` is the word at IXR + 3. In global mode an
indexed address is the whole of IXR plus the displacement, with no page in
it. The machine starts in local mode, so a program begins with `SGM`
(`JSX` and an interrupt entry select global mode themselves). A structure is
then its address in IXR and its fields as displacements:

            LDX     NODE
            LDW     *T.STA

**Bytes.** Three instructions take a byte address: `LDB`, `STB`, `CMB`. A
byte address is twice the word address (`NAME*2`, or `/NAME` as an operand).
`LDB` replaces only the low half of ACR, so `CLR` comes first. A direct byte
reference reaches only the 1024-word half page its instruction is in, which
is easy to get wrong, so the guests index every byte access from a cell:

            LDX     PTR             ; a byte address
            CLR
            LDB     *0

**Characters carry bit 7.** `'A'` assembles to X'C1', which is what the
teletype delivers, and the printer strips the bit on the way out. Carriage
return and line feed are separate characters and nothing folds them.

**Words 0-63 are the interrupt blocks**, four words to each of sixteen
levels: where the program counter is saved, the service routine's address,
where the status is saved, one spare. A program starts above them:

            ORG     0
            JMP     START           ; overwritten by the first saved PC
            WORD    SERV            ; where level 0 is served
            WORD    0               ; its saved status
            WORD    0

            ORG     X'40'
    START   SGM
            MSK
            DOT     14,9            ; connect the keyboard
            ENB     0
            UNM
    IDLE    JMP     IDLE            ; wait for interrupts

`JMP $` is how this machine waits. A level interrupts only after its `ENB`;
`MSK` holds interrupts until `UNM`, and one that arrives meanwhile is kept,
where a signal to a level not enabled is lost.

## Branching

A skip passes over one word when its condition holds.

| skip | when |
|---|---|
| `SAZ` `SAP` `SAM` | ACR is zero; zero or more; negative |
| `SAO` `SXE` | ACR is odd; IXR is even |
| `SEQ` `SNE` | the last compare was equal; was not |
| `SLS` `SGR` `SLE` | ACR was less than what it was compared with; greater; less or equal |
| `SNO` | no overflow; it also clears the overflow |
| `IXS n` | adds n to IXR, then skips if IXR is zero or more |
| `DXS n` | subtracts n from IXR, then skips if IXR is negative |

So a skip and a jump go to the label when the condition does **not** hold,
and this is read wrong about as often as it is read right:

            SAZ
            JMP     NOTZERO         ; taken when ACR is not zero

`rexmac.asm` has macros that say which is meant -- `JNZ JZ JMI JPL JNE JEQ
JGE JLE JGT JLT` -- and new code is written with them. Name the file first
in the deck.

A skip passes over one word, so it never stands in front of anything longer:
a page selection and its reference, `EXIT`, `MPY`, `DIV`, `EXCH`, a macro of
more than one word. The assembler warns of each.

A counted loop runs the index up from minus the count:

            LDX     =-20
    .LOOP   ...
            IXS     1
            JMP     .LOOP

**Every compare is signed**, `CMW`, `CMB` and `CLB` alike, and three kinds
of value go negative without looking it:

- a byte with bit 7 set, which every character is: a zero byte compares
  *above* all of them;
- a byte address above word X'4000', so a buffer walked to an end pointer
  stays below that;
- an address plus a size that passes X'7FFF'.

## Subroutines

`JSX` leaves the return address in IXR and that is all the machine does.
`SUBR` and `EXIT` are the convention the 1968 listings use: `SUBR` lays down
a word for the return address and the `STX` that fills it, and `EXIT` loads
it back and goes.

    ; ACR doubled
    DOUBLE  SUBR
            STW     .T
            ADD     .T
            EXIT    DOUBLE
    .T      WORD    0

The return address is kept in the routine's own text, in the word in front
of its entry, and so is everything else: each routine has cells of its own
for its argument and its work, named for it. `.T` above is `DOUBLE.T` and no
other routine's.

That is the whole design, and it has three consequences.

**Nothing recurses.** A second call into a routine overwrites the first's
return address and its cells. The programs of the day were written not to,
and so are these. What would recurse is written as a loop over a stack kept
by hand -- a depth, the stack's address, a check for full:

    PUSH    SUBR
            STW     .T
            LDW     SP
            CMW     =8
            JGE     FULL
            ADD     =STACK
            CAX
            LDW     .T
            STW     *0
            LDW     SP
            ADD     =1
            STW     SP
            EXIT    PUSH
    .T      WORD    0

Tiny BASIC's expression evaluator is two such stacks, operands and
operators, with an open parenthesis on the second as a marker; `GOSUB` and
`FOR` are two more (`bcore.asm`, `EVAL`, `S.GOSUB`).

**A routine has one caller at a time.** A routine an interrupt may call as
well as the program is called by the program only under `MSK`. Under an
executive, a routine any task may call holds the mask from its first
instruction, and loads its return address back before it lets go:

    SHARED  MSK
            STX     .LINK
            ...
            LDX     .LINK           ; the link first:
            UNM                     ; past here the cells are the next caller's
            JMP     *0

A routine that waits cannot hold the mask across the wait, so what it needs
afterwards goes into something the caller owns. REX keeps it in the calling
task's node (`rex.asm`, `Q.GET` and `KENTER`), and puts the disc behind a
lock so that everything under it is written like `DOUBLE` above. Where two
copies of a program must run at once, there are two copies: REX's shell is
assembled a second time as a module, one for each terminal.

**Code is data, and is written to.** There is no indirect jump: a dispatch
loads an address out of a table and jumps through the index.

            ADD     =TABLE          ; ACR numbers the case
            CAX
            LDW     *0
            CAX
            JMP     *0
    TABLE   WORD    CASE0,CASE1

And `DIN`, `DOT`, `ENB` and `INR` carry their device or level in the
instruction itself, so a routine that serves several keeps the instructions
in a table and puts the one it wants in front of itself:

    ; the status of teletype ACR (0-3)
    STATUS  SUBR
            ADD     =STATT
            CAX
            LDW     *0
            STW     .DO
    .DO     WORD    0               ; the DIN, put here by the line above
            EXIT    STATUS
    STATT   DIN     14,0
            DIN     12,0
            DIN     11,0
            DIN     10,0

The listings do more of this than the guests here do, and `asm703.py`
assembles all of it: arguments as the words after the `JSX`, read through
the return address (`JSX DOIO,FIOT,BUF,WC`); a return one word further on
for the caller's error exit (`EXIT NAME,1`); a return cell shared by
routines that never call one another (`RETSAVE` in the loader).

## Interrupts

The entry saves the program counter and the status and nothing else, so a
service routine saves ACR and IXR itself, puts them back, and leaves with
`INR` and its level. The entry does not reselect the page either: the
routine's first memory reference is made in the page of whatever was
interrupted, so a routine that can be entered from another page leads with
a selection.

    SERV    SMB     SAVEA
            STW     SAVEA
            STX     SAVEX
            ...
            LDW     SAVEA
            LDX     SAVEX
            INR     0

A service routine cannot wait, so a driver is a state the next interrupt
advances. Output is the case every program meets: `DOT 14,14` starts a
character and the teletype interrupts when it has printed, a tenth of a
second later, and that interrupt is what sends the next. One level serves
the keyboard and the printer both, and the routine tells them apart by
knowing what it started.

That tenth of a second is real under the emulator, and `--fast-io` removes
it. A program that reuses a buffer or a cell before the completion has come
works one way and not the other, so a new program is run both ways.

## Arithmetic

`MPY` and `DIV` are the machine's two-word instructions; the operand is a
whole address, or a literal. The double-length value is IXR above ACR, with
the sign repeated in ACR's top bit; `DIV` leaves the quotient in ACR and the
remainder in IXR. An unsigned sixteen bits divide as

            LDW     V
            SRL     15
            CAX
            LDW     V
            DIV     =10

Shift counts are 0 to 15. `EXCH` exchanges ACR and IXR, in two words.

## The assembler

    ../../tools/asm703.py --werror FILE... -o X.bin -l X.lst

Files are assembled in order as one deck with one symbol table, so a name in
one file is taken in all of them. The `END` card is in the last.

- **Literals.** `LDW =X'7FFF'`, `ADD =1`, `LDX =TABLE`: the assembler makes
  the cell. They are laid down at the next `LPOOL` card, which goes in the
  page of the instructions that use it, below the page's `EQU`s, where
  execution does not run into it.
- **Local labels.** A label beginning with a dot belongs to the last label
  above it that does not. The listing has it as `ROUTINE.NAME`.
- **Macros.** `rexmac.asm`: the conditional jumps, `CALL` for a subroutine
  in another page, `MSG` for a text and the two byte addresses that say
  where it is.
- **Warnings** are for what the machine does as written and nobody means,
  and `--werror` is how both makefiles assemble.

## Finding out what went wrong

Run it under the debug port with the listing for its names:

    emu -s ray703 -r X.bin --debug /tmp/s --halt --no-throttle
    ../../tools/emudbg.py /tmp/s --symbols X.lst -c 'break IDLE' -c run \
        -c 'memw DOUBLE.T 1' -c 'history 12'

`history` is the instructions that led here, disassembled with their
labels; `watch` stops on a write to a cell. A routine is quickest proved by
itself, called from a few lines that halt on a label when it is right.
