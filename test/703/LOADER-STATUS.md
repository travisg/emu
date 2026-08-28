# Relocating Loader - Basic (DN 390682C): transcription status

Companion to `XRAY-STATUS.md`. The method is in `TRANSCRIBING.md`, including
the per-document facts for this listing.

## Where things are

The durable pair is the master transcript and the PDF; everything else is
regenerable.

    ~/dropbox/tech_docs/computers/ray703/70x/390682C_RelocatingLoaderBasic_Nov1968.pdf
    ~/dropbox/tech_docs/computers/ray703/70x/390682C_RelocatingLoader_listing.txt

    ./scanstrip.sh extract <pdf> 22 51 <scandir>      # page images, 600 dpi
    ./split-transcript.sh <master.txt> <pagedir>      # master -> per-page files
    ./sync-transcript.sh  <pagedir> <header.txt> <master.txt> 28   # and back

Round trip verified exact, so the per-page files are working state and may be
deleted freely.

## Complete, and reconciled against the listing's own trailer

All 28 scanned pages, transcribed by seven agents working in parallel. The
listing prints as PAGE 2 through PAGE 29; PAGE 1 is not in the scan (the
appendix divider stands in its place), so card 1 is lost exactly as X-RAY's is.

The arithmetic closes:

| | cards |
|---|---|
| printed and transcribed | 873 |
| suppressed, across 53 untaken conditionals | 912 |
| lost with the unscanned PAGE 1 | 1 |
| **total** | **1786** |

which is the trailer's `CARDS 1786`. The 873 card numbers are strictly
increasing from 2 to 1786 with no repeats, and page boundaries are contiguous.

The cross reference reconciles too: 41 + 43 + 49 + 42 = **175 symbols** against
the trailer's `SYMBOLS 175`, no duplicate names, in strict EBCDIC collation
order from `ABSO` to `X8000`. And the listing prints **`NO ERRORS`**.

**The two halves check each other**, with one caveat worth stating plainly.
The cross reference and the listing pages were transcribed by different agents
from different pages, and comparing them finds **zero value mismatches** — 169
of the 175 symbols matched to a definition and checked; the remaining six are
`SUBR` definitions, whose label prints on the second of the two words they
generate and which the checker does not parse. Re-run it from the published
master with a dozen lines of python; it does not depend on trusting anyone's
summary.

The caveat: seven *reference addresses* in the cross reference were corrected
during transcription to match what the listing pages said (`680`→`6B0`,
`688`→`68B`, `638`→`63B`, `78E`→`7BE` among them). For those seven the cross
reference is no longer independent evidence — it was edited into agreement.
The other several hundred were read once and agree, and the 175 symbol values
were never edited. Both directions of the check are worth having; only the
seven are circular.

Even so this is a real mutual check rather than a self-consistency check, which
is the thing X-RAY's transcription never had.

## What this listing settled that X-RAY could not

- **`NOP` is `0900`** — `SRA 0`, a shift by zero. X-RAY uses `NOP` twice but
  only inside untaken code, so no object was ever printed beside it.
- **The flanked address format.** `ENDLOAD EQU $` assembles to `0800` and its
  cross-reference row prints `1 000 0`: the leading digit is the word page
  above the 11-bit M field, not decoration. See `TRANSCRIBING.md`.
- **The I/O Monitor calling sequence.** A `JSX` to a monitor entry is followed
  by inline argument words — `JSX DOIO,PRINFIOT,BUF3,BUF3CT` emits three,
  printed with address and object but no card number of their own. **The last
  argument word carries bit 0 set.** X-RAY corroborates from the other side:
  its card 1344 reads `LDW *6` / `AND X7FF` under the comment `DO NOT TEST SIGN
  BIT`, masking exactly this marker off.
- **X-RAY's system jump table, independently.** The loader opens by equating
  the monitor entry points — `XRAY X'40'`, `DOIO X'44'`, `STAT X'46'`,
  `BKSP X'48'`, `RWND X'4E'`, `ENDA X'54'`, `RBEG X'55'`, `ULIM X'5A'`,
  `STYP X'58'`, `TYPE X'6E'`, `DVEC X'74'` — and they match what a running
  X-RAY dumps out of words 40-57.

## What is known to be left

- **The literal hex constants have not been verified.** The 2.7:1 band strips
  invert `8` and `B`, and they do it in *every* column at once, so neither the
  FIELDS-recomposes-to-OBJ check nor a future re-assembly can catch it. Three
  instances were caught during transcription, each by a check other than the
  eye: `XB0` at 70C (native resolution plus the cross reference's sort order),
  `B5F3 SUB D1` on page 4 (the opcode map — `SUB` is B, `LDW` is 8), and `5BD`
  against `58D` on page 3 (the address chain closing arithmetically). The pages
  26-29 agent reports that `8` and `B` are not reliably separable by eye even
  at 700% native zoom, and had seven of its own confident readings overturned
  by the listing pages.

  Symbol references and computed operands are covered — by the cross reference
  and by re-assembly respectively. What is not covered is **literal hex
  constants** (`D X'..'`, `EQU X'..'`) whose object column was misread the same
  way as the literal. Enumerate those cards from the transcript and check just
  them at native resolution. Bounded, and the only pass that can find them.

- **`xraylist.py` has not been pointed at this transcript.** Two changes it
  needs beyond the four bugs listed in `XRAY-STATUS.md`: card numbers are not
  contiguous here, so `--check` must be relaxed from "increases by one" to
  "never decreases"; and indexed operands print as `LDW * 0` with the star in
  its own column rather than `LDW *0`, so both spellings have to parse.

  One extractor trap: page 21 card 1551 is a `'` comment card that nonetheless
  carries `0 797 0 0000` in the address columns — the second word of page 20's
  `NAME RES NAMESIZE`. Anything keying on "line has ADDR and OBJ" synthesises a
  bogus instruction there.

- **The reason for doing this at all: `asm703.py` has never assembled a whole
  program.** This transcript is the first opportunity — assemble the source and
  diff word-for-word against the printed object. Unlike X-RAY, every card here
  was assembled, so the diff covers the entire deck rather than two thirds of
  it.

## Readings still open

- **Card 652's third digit was inferred, not read.** Badly inked; written from
  its position between 651 and 653. Everything else on that line is confirmed
  by object code. This is the only character in the transcript that was
  supplied rather than seen.
- `GB1` (79A) and `HERRM` (7D6) are single occurrences whose spelling rests on
  the film alone. Both survived the cross-reference comparison, which is
  evidence about the value, not the letters.
- `DBLOCD` and `DBLOCF` (cards 937, 938) are two distinct symbols both equated
  to `IDERR` at 671, and the cross reference carries a row for each — so the
  differing final letter is real, whatever it is.

## Original typos, preserved as printed

Per the "print damage is not yours to repair" rule: card 47 `LOAD DRUG AND HALT
BEFORE EXECUTION` (for DBUG), card 490 `UNRECECOVERABLE`, card 621 `GET THE
ANME`, card 646 `IT IT ALL ONES`, card 1732 `LABFLED`.
