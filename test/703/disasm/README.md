# Disassembled period programs

Raytheon programs off the museum's tapes, written out as `asm703.py`
decks that assemble back to the tape word for word, for reading and
commenting. The tapes stay out of git, in `roms/703/tapes`
(`tools/fetch-roms.py`).

| deck | tape | what it is |
|---|---|---|
| `ptios.asm` | `PTIOS.bin` | the Paper Tape I/O System, DN 393954 (1981), 0080-016A |
| `sym1.asm` | `390470.BIN` | SYM I/PREP, the one-pass assembler, DN 390470 (7/10/70), 05B0-0FF5 |
| `fortran.asm` | `393295B.bin` | Conversational FORTRAN, the compiler for a 4K machine, DN 393295 rev B, 0080-0D04 |
| `fortlib.asm` | `394002B.bin` | Conversational FORTRAN, the run-time library, DN 394002 rev B, 0080-0CF6 |

    make -C test ray703-disasm          # write the decks again
    make -C test ray703-disasm-test     # hold them to their tapes

## How a deck is made

`tools/dis703.py` writes a deck from three things:

- **the tape**, through `tools/tape703.py`;
- **runs**: which words of the program ran, with which extension register,
  and where control went, from an `emu --trace` of sessions that work the
  program. `runs.py` makes them (`runs.py fortran`, `runs.py sym1`) and its
  module doc says what each session does; the run files in `runs/` are what
  is kept;
- **notes**: what a person knows, in `NAME.notes`, by the tape's address --
  a name, a comment on the line, lines of commentary over it, and `code` or
  `data` where the runs do not settle it.

What is code is decided by execution. A word a run executed is an
instruction; so is one reached from those by a branch, a skip or falling
through, marked `not run`. The words after a JSX are its arguments when
the runs saw that routine return past them. Everything else is DATA, with
its characters, or what it would be as an instruction, beside it. Each
line's comment begins with the tape's address and word, then what the
tool found: where the word runs, if somewhere else; who reads and writes
it; that it is an argument.

**A deck is never edited by hand.** A comment goes in the notes and the
deck is written again, so a comment survives new runs and a better tool;
`make -C test ray703-disasm-test` fails a deck that is not what writing
it again gives, as well as one that does not assemble to its tape.

Labels the tool makes say what they are: `S` a JSX's target, `J` another
branch's, `D` a cell, `E` a word entered from outside the tape, `R` an
address code under `--alias` runs at (PTIOS's resident words, run at
0000-007F from the tape's 00E2 on) and `A` any other address outside the
tape. A name in the notes replaces one.

## What the runs taught

Measured once, when the decks were first made (2026-11-22), by scripts that
were not kept: PTIOS's split against asm703's assembly of its own SYM
source, whose listing gives every card an address (the `*` cards made
comments, and the three `D /...` arguments, which are SYM's byte form,
given as numbers); and SYM I's descent by writing its deck from one session
and comparing with what the other fifteen executed.


- **PTIOS** agrees with its own SYM source on 202 of its 205 words as to
  code or data. The other three are data in the source and executed all
  the same: 0040 (the tape's 0122), the carriage return `ONECHAR`, is where
  the FORTRAN library jumps to leave, as it would to X-RAY's exit, and runs
  as a harmless `LDW` into `JMP 78`, the loader; 0065 is the argument of a
  JSX that a skip passes over, chosen to run as `SML 0`; 0072 is a cell
  PTIOS puts a DIN or DOT in and runs. `WEOF` (0132) is a routine no run
  reached.
- **SYM I** selects a page once in front of a skip and two branches, so the
  one taken runs under it (085B, 085C). asm703 lets a selection reach only
  the reference right behind it, so the second is written as the same M
  field in its own page, its comment saying where it lands. Of SYM I's
  executed references, that one is the only one whose page the static rule
  would have had wrong, which is the check on the `not run` lines' pages.
- Held to its other sessions, the descent from one SYM I session (the
  PTIOS assembly) reaches 237 of the 442 words they executed beyond it, and
  claims 120 more that none of the sixteen sessions ran. So `not run` means
  what it says: reached by reading, unconfirmed.
- **Conversational FORTRAN**'s compiler and library load over PTIOS from
  X'80' and over each other, so their runs come from one trace each, split
  by which of the loader's entries into X'80' started the program in core.
  Every word either ran is an instruction on the tape or a cell it plants
  one in, as the library does with its shift counts. The library interprets
  what the compiler makes: the compiled program sits from X'CFD' up, past
  the library's last word, and no word there ran.

## Limits

- An instruction stored at run time is shown as the word the tape has.
  Where that is zero the line is DATA, with `run: an instruction stored
  here`.
- A routine whose argument count the runs differ on is given the fewest,
  and the last argument of a variable list that ends on X'8000' (the math
  library's convention) is not told apart.
- A trace line for the instruction an interrupt cut short comes before the
  entry, so a transfer the flow records may begin at a word that did not
  transfer.

## Not yet

Pass 1 for each program -- routine boundaries, a call graph, a name and a
line for every routine.
