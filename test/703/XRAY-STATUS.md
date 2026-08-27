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
land on their printed addresses.

## What is known to be left

- **`NOP`** appears on page 31 (cards 1235, 1237) and is not in appendix B.
  Both sit in untaken conditional code with no object printed, so its encoding
  is undetermined — and irrelevant to this build.
- **Character literals.** SYM II stores them with the 703's high bit set:
  `DATA 'XR','AY'` assembles to `D8D2 C1D9`. `asm703.py` does not do this yet;
  it matters only when the transcript is assembled.
- **Forward references in `EQU`.** `asm703.py` evaluates EQU in pass 1, so an
  EQU naming a symbol defined later will fail. Not yet known whether the
  listing needs it.
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
