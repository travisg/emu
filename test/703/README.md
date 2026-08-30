# Raytheon 703 period software

Transcribing the 1968 Raytheon 700-series program listings off microfilm and
running them on the emulator in this tree. This file is the index and the
to-do list; the detail is in the documents it points at.

## The documents

| file | what it is |
|---|---|
| `TRANSCRIBING.md` | the method, and the brief handed to each transcribing agent. **Read this first** before transcribing anything. |
| `XRAY-STATUS.md` | X-RAY EXEC - BASIC (DN 390779). Transcribed, runs, answers commands. |
| `LOADER-STATUS.md` | Relocating Loader - Basic (DN 390682C). Transcribed, and re-assembles to the printed object exactly. |

## The tools

    ../../tools/scanstrip.sh         cut a 600 dpi page into readable bands
    ../../tools/split-transcript.sh  master transcript -> per-page files + header.txt
    ../../tools/sync-transcript.sh   the per-page files back into the master
    ../../tools/xraylist.py          transcript -> source, object, core image
    ../../tools/asm703.py            a SYM II assembler, enough to rebuild both decks

The **master transcripts are the source of everything** and live here:

    listings/390779_XRAY_listing.txt              X-RAY EXEC - BASIC
    listings/390682C_RelocatingLoader_listing.txt Relocating Loader - Basic

They are hand transcriptions of 1968 Raytheon program listings, made from the
600 dpi scans in `~/dropbox/tech_docs/computers/ray703/70x/`, which is where
the PDFs stay -- they are large and they are the only thing here that cannot be
regenerated. Everything else can: the split/sync pair round-trips both masters
byte-exactly, header included, and `xraylist.py` reads a master directly, so
the per-page working files are scratch and may be deleted freely.

Six guests in this directory are *new* software written for the machine
rather than transcriptions: `demo.asm`, the interrupt-driven echo the
end-to-end test drives; `basic.asm`, a Tiny BASIC running on the hardware
multiply/divide option; `disc.asm`, the 74601 disc exerciser; `boot.asm`,
the one-sector program the controller's LOAD button reads; `tape.asm`, the
image the PTB bootstrap loads off paper tape; and `rex.asm`, REX, a
preemptive round-robin executive with a shell, running on the emulator's
invented 60 Hz line clock. AGENTS.md's Test section describes the harness each of them runs
under.

## Running the period software

    make -C test ray703-listings          # -> roms/703/xray.bin, loader.bin
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

`roms/703/loader.bin` builds too, but it does not stand alone: it is written to
call a resident X-RAY through the jump table at `44`/`46`. See item 2 below.

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

- **A punch (DIO device `C`).** PEAT names `M.THSPT` for it, the same driver
  the teletype uses, so no new guest-side work would be needed.
- **The status DIN, `DIN dev,0`.** The driver's shared-interrupt path reads one
  and tests bit 7 for "IR OR NO", which pins that single bit and nothing else
  about the layout — so nothing was invented. Only assembled when
  `ISHARE=YES`, which this build has off.
- **`tx_pending` is a flag, not a count**, so two writes with no poll between
  them yield one completion. Nothing does that, and the comment at the field
  says what to do if anything ever starts.

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
