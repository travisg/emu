# Bringing X-RAY EXEC into the emulator — state of play

Transcribing DN 390779, *X-RAY EXEC - BASIC*, February 1968, from the scans in
`~/dropbox/tech_docs/computers/ray703/70x/` so the 703 emulator can run it.

## Why this listing

It is a whole system, not just the debugger: the I/O Monitor (DN 391476) and
the teletype driver (DN 392292) are built into the same assembly. And it is
assembled for **4096 words of core, one interrupt channel, a teletype on
level 0, and no other device at all** — which is exactly the machine
`src/system/ray703.rs` implements. Nothing else needs transcribing to run it.

X-RAY is entered at word `X'40'`. Its code runs from about `018` to about
`3F0`.

## Where things are

| what | where |
|---|---|
| the scans | `~/dropbox/tech_docs/computers/ray703/70x/390779_XRAY_ExecBasic_Feb1968.pdf` |
| ISA transcription | `~/dropbox/.../ray703/Raytheon703refMan_isa.txt` |
| the master transcript | `~/dropbox/.../70x/390779_XRAY_listing.txt` |
| survey notes | `~/dropbox/.../70x/390779_XRAY_TRANSCRIPTION_NOTES.md` |
| per-page working files | a scratch directory, one `page-NNN.txt` per listing page |

The per-page files are the working state; the master transcript is rebuilt
from them with `sync-transcript.sh` and is the durable artifact. Page images
are regenerable at any time:

```bash
./scanstrip.sh extract <pdf> 29 83 <scratchdir>     # listing page N -> pg-(N-1).png
```

## The tools

- **`scanstrip.sh`** — cuts a page into readable bands. The listings are
  landscape on portrait sheets at 600 dpi; whole-page views look like mush,
  but rotated and cut into fifteen-line bands they are sharp.
- **`TRANSCRIBING.md`** — the brief handed to each transcribing agent. It has
  accumulated everything earlier agents had to discover the hard way, and is
  the first thing to read before transcribing any of these listings.
- **`xraylist.py`** — takes the transcript apart into source (`--asm`) and
  object code (`--obj`), and checks it (`--check`, `--fix-references`).
- **`asm703.py`** — now speaks enough SYM II to reassemble the result.

## Progress

33 of 54 scanned pages transcribed and passing every check. Listing page 1 was
not scanned — the sheet in its place is the Appendix A divider — so card 1 is
lost; cards 2 onward are intact.

Remaining: the tail of the code pages, and the symbol table on pages 54–55.

## How this is being verified

Four independent checks, because hand transcription of 1968 microfilm is
exactly as error-prone as it sounds:

1. **Fields recompose.** The printer emitted every assembled word twice, once
   whole and once split into opcode / index / address. A misread nibble
   usually fails to recompose.
2. **Cards run unbroken.** 1..1862, no gaps, no repeats.
3. **Each card assembles to what was printed.** `xraylist.py` assembles every
   card in isolation and compares. This is what catches generics, where the
   check column merely repeats the word and a consistent misreading sails
   through both copies.
4. **Address fields match their symbols.** `LDX M.TFA` printed as `9083`
   recomposes perfectly from `9 0 083`; only knowing that M.TFA sits at `0B3`
   reveals it. `--fix-references` repairs these, but only where the symbol's
   value is corroborated by its defining card *and* a reference that already
   agrees — otherwise one misread definition would propagate outwards.

The endgame check, once the symbol table is in: assemble the extracted source
and diff it against the extracted object code, then verify all 278 symbols
land on their printed addresses. `asm703.py --map` emits exactly the shape
`xraylist.py --obj` does, so the first half of that is `diff` on two sorted
files with nothing to misparse in between. In order:

1. **Conditionals first.** The listing shows which `TRUE`/`FALS` branches the
   1968 assembly took, by printing no object for the ones it skipped. If our
   build-config equates differ, or `asm703` evaluates a guard differently, the
   word diff is enormous and every line of it is misleading. Check
   taken/not-taken agreement before reading a single word mismatch.
2. **Then the symbol table**, which is a *third* authority and the only one
   that can catch what check 4 structurally cannot: a misread definition
   corroborated by a misread reference. Cross-check every EQU and label value
   against the printed table.
3. **Then the word diff.**

## What is known to be left

- **`NOP`** appears on page 31 (cards 1235, 1237) and is not in appendix B.
  Both sit in untaken conditional code with no object printed, so its encoding
  is undetermined — and irrelevant to this build.
- ~~**Character literals.**~~ Done. SYM II stores them with the 703's high bit
  set, and `asm703.py` now reproduces card 363 (`DATA 'XR','AY'` → `D8D2 C1D9`)
  exactly. The one-character case has no example in the listing; it is packed
  right-justified, because `LLB`'s literal is only eight bits wide and a
  blank-filled left justification could not be loaded by it at all.
- ~~**Forward references in `EQU`.**~~ Done, and the listing does need it:
  card 298 is `MAXP EQU ENDP-PEAT+12` and neither operand is defined until
  much further down the deck. `asm703.py` now defers an EQU it cannot evaluate
  and sweeps the leftovers to a fixpoint.
- **`*` is a comment-card marker too**, not just `'`. The two are distinct
  glyphs on the page — `'` is a tall high tick, `*` a lobed star at mid-height
  — and both appear, sometimes on adjacent cards. `xraylist.py`'s `split_card`
  reads a leading `*` as a *label*, so those cards extract as nonsense source
  (`*  THIS  IS ...`). Pages 28, 36, 40 and 47 have them so far. Extractor fix,
  not a transcription error: the pages are right.
- **The strip crop loses the far right margin.** `scanstrip.sh` cuts at
  x=5060; comments on cards 1747 and 1825 run to x≈5300 (an author and a date,
  `J.R. NELSON  9/8/67`). Widen `W` and `OUT_W` together so the downsample
  ratio — and with it the legibility — stays where it is, then re-read the
  right edge of the pages already done. Nothing load-bearing is out there, but
  attributions are worth having.
- **Print damage that will not assemble.** Card 325 prints `SYR0 EQU X'80`
  with the closing quote missing, which the transcript reproduces faithfully
  and no expression parser will accept. The repair belongs in `xraylist.py`'s
  `--asm` path, not in the transcript and not in `asm703.py`: the transcript's
  job is fidelity, `--asm`'s job is producing something assemblable. A sweep
  of the 33 transcribed pages found this to be the only such card.
- **Output-completion interrupts.** `Tty703` completes a `DOT` instantly and
  never interrupts. The listing has a dedicated output-driver interrupt
  service area, so the real driver may well wait for a signal that never
  comes. This is the likeliest thing to need emulator work, and the plan is to
  hit it and diagnose it rather than guess now.

## Recurring failure mode, worth knowing before touching this again

Almost every error found so far has been **8 read as B**, in both directions,
by every agent including me. `UNM` (`00B0`) was read as `0080` on five cards;
`M.TFA` (`0B3`) as `083` on nine. On one card the `B`'s left stem had dropped
out of the print entirely and looked like a `9`, which produced a confident
report of a "genuine print discrepancy" in the original.

When two readings conflict, the answer is the film at high magnification, not
the more confident of the two reports.
