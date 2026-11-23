# Raytheon 703 period software

Transcribing the 1968 Raytheon 700-series program listings off microfilm and
running them on the emulator in this tree. This file is the index and the
to-do list; the detail is in the documents it points at.

## The documents

| file | what it is |
|---|---|
| `TRANSCRIBING.md` | the method, and the brief handed to each transcribing agent. **Read this first** before transcribing anything. |
| `WRITING.md` | how to write a program for the machine with `asm703.py`: the idioms the guests here are built from, and the mistakes the machine makes easy. |
| `XRAY-STATUS.md` | X-RAY EXEC - BASIC (DN 390779). Transcribed, runs, answers commands. |
| `LOADER-STATUS.md` | Relocating Loader - Basic (DN 390682C). Transcribed, and re-assembles to the printed object exactly. |

## The tools

    ../../tools/scanstrip.sh         cut a 600 dpi page into readable bands
    ../../tools/split-transcript.sh  master transcript -> per-page files + header.txt
    ../../tools/sync-transcript.sh   the per-page files back into the master
    ../../tools/xraylist.py          transcript -> source, object, core image
    ../../tools/asm703.py            a SYM II assembler, enough to rebuild both decks
    ../../tools/reload703.py         a reference loader for the relocatable object text asm703.py --object writes
    ../../tools/tape703.py           a Raytheon absolute paper tape -> what is on it, and a core image

The **master transcripts are the source of everything** and live here:

    listings/390779_XRAY_listing.txt              X-RAY EXEC - BASIC
    listings/390682C_RelocatingLoader_listing.txt Relocating Loader - Basic

They are hand transcriptions of 1968 Raytheon program listings, made from the
600 dpi scans in `~/dropbox/tech_docs/computers/ray703/70x/`, which is where
the PDFs stay -- they are large and they are the only thing here that cannot be
regenerated. Everything else can: the split/sync pair round-trips both masters
byte-exactly, header included, and `xraylist.py` reads a master directly, so
the per-page working files are scratch and may be deleted freely.

The guests in this directory are *new* software written for the machine
rather than transcriptions: `demo.asm`, the interrupt-driven echo the
end-to-end test drives; `bcore.asm`, a Tiny BASIC interpreter running on
the hardware multiply/divide option, with `basic.asm` the wrapper that
makes it a standalone machine (and `../../rex/brex.asm` the one that makes
it a module under REX); `disc.asm`, the 74601 disc exerciser;
`boot.asm`, the one-sector program the controller's LOAD button reads;
and `tape.asm`, the image the PTB bootstrap loads off paper tape.
AGENTS.md's Test section describes the harness each of them runs under.
REX, the executive that puts the same interpreter aboard as a task, is its
own project under `../../rex/`.

## Running the period software

    make -C test ray703-listings          # -> roms/703/xray.bin, listings/loader.bin
    ./target/debug/emu -s ray703 -r roms/703/xray.bin

**Type a LINE FEED before every command and a RETURN after it.** That is
Ctrl-J, then the directive, then Return. It is not a quirk of the emulator: the
documentation says "all directives must be preceded by a line feed and followed
by a carriage return", because the driver's record format opens each record on
a line feed and closes it on a carriage return. Without the leading Ctrl-J
X-RAY reads your characters and discards every one of them, which looks exactly
like a machine that is ignoring you.

    <Ctrl-J>D 0040,0050<Return>

    0040  0080  1266  0080  1065  0080  1083  0080  101C
    0048  2801  2801  2801  2801  2801  2801  2801  2801
    0050  0080  11F3  2801  2801  03B9  0000  0080  12DF

That is X-RAY's own system jump table, by the way -- `UNM` then a `JMP` per
entry, and the relocating loader's opening equates name the same addresses.

Things worth knowing at the console:

- **Directives are two characters.** Dump is `D` plus its space, so `D 300,310`
  is really `D␣` with one argument. `D 40 50` with a space instead of a comma
  parses as something else entirely and prints one unexplained line.
- **All arguments are hexadecimal**, leading zeroes optional, and only the last
  four hex digits of an argument are used.
- **A wrong directive gets you `??`** and X-RAY asks for another line.
- **Ctrl-D exits** the emulator.
- Output is paced by an interrupt per character, so a long dump takes a moment
  to appear. Wait for it rather than assuming it hung.
- The rest of the command set is documented in the **front matter of the PDF**
  -- the thirty pages ahead of Appendix A, which are not transcribed. Read them
  rather than guessing; that is where the comma came from.

`test/703/listings/loader.bin` builds too, but it does not stand alone: it is written to
call a resident X-RAY through the jump table at `44`/`46`. See item 2 below.

## Raytheon's diagnostics

    tools/fetch-roms.py                  # the tapes, into roms/703/diag
    make -C test ray703-diag-test

Ten of the 1970 diagnostic tapes the University of Stuttgart's computer
museum read off its 704 run here, and the machine passes them: the eight
instruction diagnostics, BRAINWASH IV for memory and the teletype test.
One of them, the second instruction composite, is run with a word of the
tape changed. `../run_ray703_diag_test.py` says which and why, how each
tape is run, and what its SENSE switches were read to mean, since no
operating instructions came with them.

To watch one, make a core image of it and set the switches from the debug
port -- `e` is switches 1, 2 and 3 up, which in the instruction diagnostics
halts on an error, prints it, and prints each group's name as it passes:

    tools/tape703.py roms/703/diag/390389C.bin -o /tmp/adder.core --start 80
    ./target/debug/emu -s ray703 -r /tmp/adder.core --debug /tmp/emu.sock --halt
    tools/emudbg.py /tmp/emu.sock -c 'set SS e' -c run     # from another terminal

The teletype test (start at 40) prints the quick brown fox with switch 1 up,
its typebox rippled with switch 0, and a line of whatever key is typed with
switch 2. The primary instruction diagnostic, 390392C, first checks the
switches themselves: all up and RUN to its first halt, all down and RUN to
its second, then the options.

## FORTRAN and the math library

    tools/fetch-roms.py                  # the tapes, into roms/703/tapes
    make -C test ray703-fortran-test
    make -C test ray703-math-test

Four more of the museum's tapes. `PTIOS.bin` is the Paper Tape I/O System
its site ran in place of X-RAY: 128 words that sit in words 0-7F with the
executive's I/O entry at 44 and an absolute loader at 78, and drive the
teletype, the high speed reader and the punch by asking each one's status
word, with no interrupt. `393295B.bin` and `394002B.bin` are Conversational
FORTRAN for a 4K machine, the compiler and the run-time library that is
loaded over it; `392325.bin` is the math library, as relocatable text.

`../run_ray703_fortran_test.py` is the reference for the session: PTIOS
started at 80, its units assigned (`4:R` is the loader's tape to the reader,
`2:R` the compiler's source, `n:P` a unit to the punch, `n:T` to the
teletype; it answers `OK`), `AL` for the loader, then `G` to the compiler,
`E` back to the loader for the library, and `G` to run. Every line typed at
PTIOS or a program under it opens with a line feed and closes with a
carriage return, as at X-RAY. By hand it is a debug port and three
commands of its own -- `tape 0 FILE` threads a tape through the high speed
reader, `tape 1 FILE` through the teletype's, and `savetape FILE` tears off
what the punch has punched:

    tools/tape703.py roms/703/tapes/PTIOS.bin -o /tmp/ptios.core --start 80
    ./target/debug/emu -s ray703 -r /tmp/ptios.core --debug /tmp/emu.sock
    tools/emudbg.py /tmp/emu.sock -c 'tape 0 roms/703/tapes/393295B.bin'   # from another terminal

The math library is not loaded by the machine: `tools/reload703.py --library`
places its twenty-seven modules on the host and fills the references between
them, and `../run_ray703_math_test.py` calls the result routine by routine.
Its module doc says which two routines the tape has at mismatched revisions
and how each is run.

## SYM I

    make -C test ray703-sym-test

`390470.BIN` is SYM I/PREP, Raytheon's one-pass assembler for a 4K machine
(7/10/70), and `PTIOS_SYM.bin` the SYM source of PTIOS. SYM I assembles the
source back into PTIOS's binary tape, frame for frame, the tape's last word
included -- a check word that no card of the listing shows, which SYM I
computes. It loads at 5B0-FF5, starts at 800, and brings its own
interrupt-driven driver for the teletype, the reader and the punch; its
symbol table grows down from 5AF.

At its `??` it reads a line feed and two characters, no carriage return:
`CH`, `CT` or `CK` takes the source from the high speed reader, the
teletype's reader or the keyboard, `CJ` or `CP` punches the object on the
high speed punch or the teletype's; `SA` assembles absolute and `S ` (a
space) relocatable. It stops for the operator by rolling a code through the
accumulator's lamps until SENSE 3 goes up and comes down: 1 to ready the
source, 2 to ready the punch once the listing is done, 3 when it has
punched; a code above F00 is fatal, and the card goes to the teletype with
a letter in front. `../run_ray703_sym_test.py` drives it and is the
reference.

SENSE 0 is the listing. SYM I assembles into core and lists afterwards,
walking what it made, one line per word, and tests SENSE 0 before each:
up, the listing stops there and the punch is wanted; down, it goes on. So
the operator can skip a listing or cut one short, at ten characters a
second worth having. With the source at the keyboard, SENSE 0 down also
has each card answered as it is typed, one card behind: the previous
card's address and word, `*` in front when it holds a forward reference
not yet resolved (`* 0100 87FF` for `LDW ONE` before `ONE` is defined; the
final listing has it as `0100 8102`). SENSE 1 and 2 and the external sense
line are tested nowhere in SYM I; SENSE 3 is the operator's continue.

`../../tools/asm703.py --sym1` assembles the same cards to the same tape,
frame for frame (`-a` punches it as SYM I does), and the harness requires
that too. What it took: SYM I's comment cards (`*`), its byte form of a
data word (`D /PYTECT+PYTECT-A`, the locations doubled and the constants
not), its NOP (0A00, `SRL 0`, where the 1968 listings' assembler has 0900)
and the tape's check word, the image's byte sum folded once, which
`tape703.py` now checks on any absolute tape.

`S ` lists the program from word 0 and punches relocatable text in the 1968
loader's records, which `tools/reload703.py` decodes, but the run stops
short of the last record and its END code, with or without `--fast-io`:
what that wants is not yet known. `P` (PREP) checks each card as it is
typed and punches the source; `X` and `XH` punch a bootstrap and halt.

Off the teletype's reader (`CT`) it assembles PTIOS as it does off the
high speed reader. Both readers keep their frame time under `--fast-io`,
which is what lets SYM I's driver disconnect behind a card's carriage
return before the next frame arrives; a frame on the next instruction would
raise the level first, and the driver's next read would take that pulse and
lose a card.

## Disassemblies

    make -C test ray703-disasm-test

`disasm/` holds PTIOS, SYM I and Conversational FORTRAN's compiler and
library as decks that assemble back to their tapes,
written by `../../tools/dis703.py` from the tape, traced runs and a notes
file; its README says how they are made and kept.

## Where things stand

Both listings are transcribed in full and reconcile against their own trailers.
X-RAY boots, takes a `D 0300,0310` and prints a dump that matches its image
byte for byte. The loader re-assembles to **596 of 596 printed words with zero
mismatches**, which is the strongest statement available about a transcription:
the source text and the object column are independent readings of the same
card, and an assembler that saw neither agrees with both.

## What is still missing

Roughly in the order that unblocks the most.

### 1. Make X-RAY assemble, then verify it the way the loader was verified

`make -C test ray703-verify` reports that X-RAY does not assemble, and says
why. Two repairs are needed, and both belong in the `--asm` path rather than
the transcript — the transcript's job is fidelity, `--asm`'s is producing
something that assembles:

- **card 741** is `TRUE ISHARF=YES`, a keypunch typo for `ISHARE`;
- **card 325** is `SYR0 EQU X'80` with the closing quote missing, which the
  card below it has.

Then the same word-for-word diff X-RAY has never had. Expect it to find real
SYM II semantics, as the loader's did — five separate rules came out of that
exercise, none of them documented anywhere.

Note the diff will only cover the two thirds of X-RAY that was assembled. Its
listing prints untaken conditional bodies without object code, so several
hundred words of disk and mag-tape driver have no printed object to check
against and never will.

### 2. Run the loader and X-RAY together

They do not overlap: X-RAY occupies `018`–`3B9` and the loader `545`–`7FF`.
And the loader's opening equates are X-RAY's system jump table — `DOIO` at
`44`, `STAT` at `46` — so the loader is *written* to call into a resident
X-RAY. Loading both into one core image and starting X-RAY is the obvious next
milestone, and it is the first thing in this project that would exercise two
transcribed programs against each other.

### 3. Drive X-RAY's other commands

`D` works. The rest of the command set has not been tried, and a wrong word in
a routine nothing has executed is still a wrong word. Driving each command is
the cheapest remaining check on the transcription: a single `D` executes 262
distinct words against 195 for the boot alone.

**Use the documentation, do not guess the syntax.** The front matter of the
X-RAY PDF — the thirty pages ahead of Appendix A, which are not transcribed —
gives every directive's input format. Directives are two characters, all
arguments are hexadecimal, and every one must be preceded by a line feed and
followed by a carriage return.

### 4. The extractor's last known bug

A four-digit card number is also valid hex, and `xraylist.py`'s `LINE` regex
takes it for the object word when what follows can pass as a fields split.
X-RAY's cards 1126 and 1619 are lost this way. The regex has to anchor on the
address and object columns rather than pattern-match them.

### 5. Emulator gaps

- **Raytheon's punch and reader test, 390444B, is not run.** It is a program
  for X-RAY's I/O entries -- `JSX` through 42, 44 and 46 with a file table,
  logical unit 13 for the console, 5 to punch and 4 to read -- which prints
  its name, halts for the operator, punches 256 frames, halts for the tape
  to be moved to the reader, and reads it back. The reader and the punch it
  wants are here (PTIOS and Conversational FORTRAN drive both); what it has
  not had is an X-RAY to run under. The 1968 listing was assembled with no
  punch, and PTIOS has no entry at 42. The museum's own X-RAY tape
  (`XRAY.bin`, a later revision, not in the manifest) does run here --
  written into core and started at 26A it answers `D 0040,0050` -- but
  which devices it gives units 4 and 5 is not known, and the test has to be
  loaded and started by X-RAY's own directives, which are in the listing's
  untranscribed front matter: moving the program counter to the test while
  X-RAY waited at its console got no further than X-RAY's own code.
- **The punch and the readers take their tapes from the debug port only**
  (`tape`, `savetape`); `ray703-ptb -r` is the one way onto the high speed
  reader from the command line.

### 6. More listings

Bitsavers has the Symbolic Program Editor (DN 390941E, with a full assembly
listing in the same format and the same clean print), the I/O Monitor (DN
391476, prose — it documents the FIOT and the STAT call), and an Initial Loader
(DN 393260). SYM II's own listing is not among them and probably does not
survive; it does not need to, because each of these is another sample of the
language and another oracle for `asm703.py`.

## Readings that rest on the film alone

Everything else in both transcripts is corroborated by object code, by the
cross reference, by re-assembly, or by the documentation. These are not:

- **Loader card 652's third digit** was inferred from its position between 651
  and 653, not read — the only character in that transcript that was supplied
  rather than seen. Everything else on the line is confirmed by object code.
- **Loader `GB1` (79A) and `HERRM` (7D6)** are single occurrences. The cross
  reference confirms their values, which is evidence about the address and not
  about the letters.
- **X-RAY's open readings** are listed in `XRAY-STATUS.md` under "Readings
  still open": `SIGR` on page 30, `S.XPNU` at `39C`, `NTRY DUMP` on card 1704,
  and `S.LLIB`/`S.LLIR` on card 1565. None can affect the running image.

And one caution about method, learned expensively and worth repeating here:
**neither the code pages nor the cross reference is systematically right.**
Each has corrected the other. They are independent witnesses, not authorities.
