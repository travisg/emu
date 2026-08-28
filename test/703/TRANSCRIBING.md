# Transcribing a Raytheon 700-series assembly listing

You are transcribing pages of a 1968 Raytheon 703 assembly listing from 600 dpi
scans. Accuracy matters far more than speed: a single wrong hex nibble is a
silent bug in a program nobody can debug by inspection. **Never guess. Never
infer a line you cannot actually see.**

Most of this file was written while transcribing X-RAY EXEC (DN 390779) and
uses it for its examples, but the listings all came off the same assembler and
the method transfers. Per-document facts — where the scans are, how page
numbers map, what the print does badly — are in your prompt and in "Other
listings" at the end. **Your prompt wins over the examples here.**

## Paths

    SCAN=<the directory holding pg-NNN.png, given to you in your prompt>
    STRIP=/mnt/nas2/src/svn/emu/test/703/scanstrip.sh

For X-RAY, listing PAGE N is `$SCAN/pg-MMM.png` where **MMM = N - 1**, zero
padded to three digits: page 6 is `pg-005.png`, page 40 is `pg-039.png`. Other
documents map differently — your prompt says which.

The strips now include the page number printed in the right margin. **Check it
against the page you were asked for, every time, and say in your summary what
it said.** A page read twice or skipped is a silent hole, which is the worst
thing that can happen here, and this is the cheap way to catch it.

## Procedure, per page

1. Cut the page into strips, into a working directory of your own so that you
   do not collide with other agents working on other pages:

       $STRIP strips $SCAN/pg-MMM.png $SCAN/w/pN

   where `pN` is unique to the page you are on (e.g. `p14`). It prints the
   band files it produced. Bands that are blank page are deleted, so you may
   get fewer than four.

2. Read every band image it printed, in order, with the Read tool.

   Consecutive bands **overlap by a line or two on purpose**. Do not transcribe
   the overlapping lines twice — the card numbers in the middle column tell you
   exactly where you already are.

3. Write the page to `$SCAN/txt/page-NNN.txt` (NNN = the listing page number,
   zero padded to three digits, so page 6 is `page-006.txt`).

## The line format

The listing prints, left to right:

    ADDR    OBJ   FIELDS    CARD  LABEL   OP    OPERAND      COMMENT
    0 066 0 603F  6 0 03F   515           STX   M.OPENR      SAVE RETURN
    0 069 0 8805  8 1 005   526           LDW   *5           LOAD MODE
            000C  000C      4     DUM     EQU   12
                            37    '

- **ADDR** prints as a flanked address, `0 066 0`. The middle three hex digits
  are the word address; the flanking digits are almost always `0`. Record it
  as printed.
- **OBJ** is the assembled word, four hex digits.
- **FIELDS** is the *same word* split into opcode / index bit / M field. On
  lines that are an `EQU` rather than an instruction, this column just repeats
  the value instead of splitting it.
- **CARD** is the source card number and runs 1..1862 across the whole
  listing, always increasing by one. It is your best check that you have not
  dropped or duplicated a line.
- A `'` (printed as a small blob, easy to mistake for `*`) in the label column
  marks a comment card.

Lines with no ADDR/OBJ generated no object code: they are comments, or they
sit inside a conditional assembly that was not taken.

## The built-in check — use it

For an instruction, FIELDS must recompose to OBJ:

    603F  =  6 0 03F     opcode 6, index bit 0, M field 03F
    8805  =  8 1 005     opcode 8, index bit 1, M field 005
    0A1F  =  0A1 F       a generic: the group 0A1, then the operand nibble F

**Check every instruction line.** If the two disagree, you misread one of
them — go back and look again at that line. If after looking again they still
disagree, transcribe both as you see them and add ` <<< CHECK: fields do not
match word` at the end of the line.

## What the print does badly

The printer's `W` is weak and often looks like `*`, `M`, or nothing: `LDW`
prints as `LD*`, `STW` as `ST*` or `ST-`. `0` and `U` and `D` blur together in
the hex columns, as do `5`/`S`, `6`/`b`, `8`/`B`, and `1`/`I`. Resolve these by
what is legal, not by what looks nice — the complete instruction set is:

    Memory reference (take an address operand, optionally `*` for indexed):
      JMP JSX STB CMB LDB STX STW LDW LDX ADD SUB ORI ORE AND CMW
    Control generics:
      HLT INR ENB DSB SLM SGM CEX CXE SML SMU MSK UNM
    Register generics (no operand):
      CLR CMP INV CAX CXA
    I/O generics (operand is `device,function`):
      DIN DOT
    Literal generics (8-bit literal):
      IXS DXS LLB CLB
    Skips (no operand):
      SAZ SAP SAM SAO SLS SXE SEQ SNE SGR SLE SNO SSE SS0 SS1 SS2 SS3
    Shifts (4-bit count):
      SRA SLA SRAD SLAD SRL SLL SRLD SLLD SRC SLC SRCD SLCD
      SRLL SLLL SRLR SLLR SRCL SLCL SRCR SLCR
    Assembler directives:
      EQU DATA RES ORG ORIG TRUE FALS ENDC END
    Assembler pseudo-instructions that generate one word:
      SMB   select the memory base holding a symbol -- assembles to an SML or
            an SMU for that symbol's byte page

**This list is known to be incomplete.** SYM II has directives nobody has
catalogued, and `ORIG` and `SMB` were both found this way. If a token is
clearly legible at high zoom and is clearly not a label, transcribe it exactly
as printed and add ` <<< CHECK: op not in the known list` at the end of the
line. Do not bend a clear reading to fit the list. Only when a token is
genuinely ambiguous should the list be used to break the tie.

## Character constants

A quoted constant in an operand holds one or two characters packed into a
word, and the characters carry the 703's high bit: `DATA 'XR','AY'` assembles
to `D8D2` and `C1D9`, which is ASCII with bit 7 set (`X`=58+80=D8). If you see
a hex word in the object column that decodes to letters that way, that is
confirmation you have read both correctly.

## Print damage is not yours to repair

Transcribe what is on the paper even when it is wrong. Card 325 prints
`SYR0 EQU X'80` with no closing quote, and the card below it prints `X'40'`
with both -- so the omission is in the original. Reproduce it and move on.

Labels frequently contain a period: `M.OPENR`, `S.TEMP1`, `M.OTMNW`. Operands
may be `X'1F'` style hex, decimal, a label, `$` (the current address), `$+1`,
or an expression. `*` before an operand means indexed addressing.

## Output format

Start the file with a separator and the page header exactly like this, then one
line per card, columns lined up as below:

    ================================================================
    PAGE 15     SUBROUTINE OPEN

            003F  003F            511   M.SRET  EQU   M.OPENR
            00B3  00B3            513   M.DF    EQU   M.TFA
    0 065 0 00A0  00A0            514   OPEN    MSK                 SET MASK OFF
    0 066 0 603F  6 0 03F         515           STX   M.OPENR       SAVE RETURN
                                  518           TRUE  ICHN=4
                                  519           LDW   ARH1

The page header title is the one printed at the top of the page (e.g.
`SUBROUTINE OPEN`, `LOGICAL UNIT ASSIGNMENTS`). Preserve comment text and
inline comments as printed, including the long rows of asterisks.

If a character is genuinely illegible, write it as `[?]` rather than guessing,
and keep going.

## When you are done

Reply with **only** a short summary: which pages you wrote, the card-number
range each covers, and any lines you flagged with `CHECK` or `[?]`. Do not
paste the transcription back — it is already on disk, and repeating it wastes
the parent's context.

## Things later pages will hit, learned from earlier ones

**Double and byte shifts print with a space.** The listing writes `SRC D`,
`SRA D`, `SRL L`, `SLC R` and so on, exactly as appendix B of the reference
manual does. That is one mnemonic (`SRCD`, `SRAD`, `SRLL`, `SLCR`) printed with
a gap, not an opcode and an operand. Transcribe it as printed, space and all --
the extractor knows to rejoin them. The FIELDS column tells you which you have:
`0A6 F` is `SRCD 15`, whereas plain `SRC 15` would be `0A4 F`.

**`D` is a directive**, SYM II's short form of `DATA`: one word of the operand's
value. There is a constants pool built with it on page 12.

**`BYTE`** emits bytes rather than words, and prints its second byte on a
continuation line with no card number of its own. The address column's trailing
digit -- normally 0 -- is the byte position within the word, so a BYTE pair
prints as `0 058 0` then `0 058 1`.

**`SUBR` and `EXIT`** are the subroutine convention on a machine with no stack,
and each generates two words. `SUBR` emits a zero return slot followed by an
`STX` into it; `EXIT sym` emits an `LDX` and an indexed `JSX`. Their printing
is irregular: for `SUBR` the card number is on the *first* word's line and the
label and mnemonic on the second, while for `EXIT` the source text is on the
first. Transcribe both as printed.

**`SXP`** assembles to `0400`, which is `IXS 0` -- increment the index by
nothing and skip if it is positive. Treat it as a real mnemonic.

## Settling a glyph for good

The band strips are a 2.7:1 downsample. They are fine for reading and wrong for
deciding: on page 12 the strip shows `UNM` as `0080` and card 398 as `00B0`,
which is both of them backwards. When a single character actually matters, go
to native resolution and compare it against a *known* instance of each
candidate from the same page. The procedure, which settled `SIGB`, `XB0`,
`RTIK` and the `O`/`0` question:

```bash
convert pg-0NN.png -rotate 90 rot.png          # the images are landscape-on-portrait

# find the text rows: one ink profile down the page, runs of dark = rows
convert rot.png -crop 4500x4400+560+150 +repage -resize 1x4400! -depth 8 txt:- |
    awk -F'[,:) (]+' 'NR>1{print $2+150, $4}' > prof.txt
# then group runs of value < 252; rows are ~76 px apart, first at y~550

convert rot.png -crop 58x76+X+Y +repage query.png     # the glyph in question
convert rot.png -crop 58x76+X2+Y2 +repage known.png   # a certain one, same page
convert known.png query.png +append -filter point -resize 700% cmp.png
```

`-filter point` matters: any smoothing invents detail that is not on the film.

What the glyphs actually look like at that magnification:

- **`8`** — two stacked bowls, symmetric, the left side pinched at the waist.
- **`B`** — a straight full-height left stem, both bowls opening off it to the
  right. The stem is the tell, not the counters.
- **`R`** — one closed bowl on top and **two splayed legs** below.
- **`O` vs `0`** — same shape, different width: letter **36 px**, digit **29
  px**. Measure it: `convert ... +repage -trim info:` prints the bounding box.

Anchors are free and you should always grab one. A card number is decimal, so
its digits are certain: card 398's own `8` is the reference for the `8` in its
object word four columns to the left. `UNM` is `00B0` by definition, so any
page with an `UNM` on it carries a certified `B`. `ORI` and `ORE` carry a
certified `R`.

**But an isolated glyph is never the last word.** The printer drops strokes
intermittently — the same drum printed a clean `B` on one card and, a few pages
away, a `B` so eroded it is indistinguishable from the `R` beside it. Card 906
reads `SIGR` at any magnification and is `SIGB`; the cross reference proves it,
because it holds one `SIG` row whose reference list includes that very card's
address, and the assembly trailer says `NO ERRORS`. Where object code or the
cross reference can adjudicate, they outrank the film. Use the film to settle
what nothing else can reach, and flag what neither can.

## Symbol spellings

Two agents have already disagreed about a symbol (`SIGB` against `SIGR`, both
for the word at `254`). Listing pages 54 and 55 are the assembler's own symbol
table: 278 names with their values. If you are unsure of a symbol's spelling
and it is used more than once, the value in the object code identifies it --
say so in your summary rather than picking silently, and the symbol table will
settle it.

Watch `M` against `H` in particular: `ARH0`..`ARH3` were read as `ARM0`..`ARM3`
for a while. The `H` has two full stems and a crossbar; the `M` in `M.OPENR`
directly above it on the page is visibly a different glyph.

**`EXCH`** swaps the accumulator and the index register and generates **two**
words, both `0A78`. That is `SLCD 8` twice: the double-length circular shift
rotates the 32-bit ACR:IXR pair, and two rotations of eight make sixteen. No
single instruction can do it, because the shift count is only four bits.

**`SXM`** assembles to `0500`, which is `DXS 0` -- the mirror of `SXP`.

**Byte instructions split the FIELDS column four ways.** `LDB *0` prints as
`5800  5 1 000 0`: opcode, index bit, the word part of the address, and then
which byte of that word. Recompose it as
`op<<12 | index<<11 | word<<1 | byte`.

**Sheets are not registered identically.** Some pages sit as much as 200 pixels
lower on the scan than others. `scanstrip.sh` now starts higher and cuts five
bands instead of four to cover it, but if a page looks like it is missing lines
at the top or bottom, render the whole page and check before believing the
strips. A dropped band is a silent hole in the transcript, which is the worst
thing that can happen here.

## Other listings

The same method covers the rest of the 700-series documents. What differs per
document is collected here as each one is done.

### Relocating Loader - Basic (DN 390682, revision C, 08/9/68)

`~/dropbox/tech_docs/computers/ray703/70x/390682C_RelocatingLoaderBasic_Nov1968.pdf`,
52 pages. Pages 1-21 are prose and flowcharts, page 23 the "APPENDIX A /
ASSEMBLY LISTING" divider, and **pages 24-51 are the listing, printed as PAGE 2
through PAGE 29**. Extract from PDF page 22 and the numbering lines up: `$SCAN`
holds `pg-000.png` (blank) through `pg-029.png`, and **`pg-NNN.png` is listing
PAGE NNN** with no offset — unlike X-RAY.

    ./scanstrip.sh extract <the pdf> 22 51 /storage/scratch/rl703

- **The print is clean.** This came off a well-inked printer, not the worn drum
  that printed X-RAY, and there is no stroke dropout. The `B`/`R`, `8`/`B` and
  `O`/`0` forensics below are not needed. If a character looks ambiguous here
  it probably *is* ambiguous, so flag it rather than measuring it.
- Header line: the program label `RELOADD` or the document title, a section
  title, the date `08/9/68`, `PASS B`, and the page number.
- The right margin carries `NP nnnnn` card sequence numbers, one per card.
  **Ignore them** — they are the punched-card sequence field and the card
  number column already gives a running count. They are in the strip only
  because the page number sits beside them.
- Card numbers run **1 to 1786**. The trailer on PAGE 29 reads
  `CARDS 1786  SYMBOLS 175  583  LITR 0  STACK 6`.
- Conditional assembly is on `LOADER`, which is `BASIC` (0), `STANDARD` (1) or
  `DISK` (2); this listing is `LOADER EQU BASIC`, so the standard and disk
  sections print with no object code.
- Both `*` and `'` appear as comment-card markers, sometimes on adjacent cards.
- The tail is a `SYMBOL TABLE` cross reference by PAGE 26, in the same shape as
  X-RAY's: value, name, then every referencing address.
