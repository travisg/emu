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

    scanstrip.sh        cut a 600 dpi page into readable bands
    split-transcript.sh master transcript -> per-page working files + header.txt
    sync-transcript.sh  the per-page files back into the master
    xraylist.py         transcript -> source, object, core image; --check, --verify
    ../asm703.py        a SYM II assembler, enough of one to rebuild both decks

**Only the master transcripts and the PDFs are durable.** Everything else
regenerates, and the split/sync pair round-trips both masters byte-exactly,
header included — verified, not assumed. Anything under `/storage/scratch` may
be deleted freely.

    ~/dropbox/tech_docs/computers/ray703/70x/390779_XRAY_listing.txt
    ~/dropbox/tech_docs/computers/ray703/70x/390682C_RelocatingLoader_listing.txt

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

`xraylist.py <pagedir> --verify` reports that X-RAY does not assemble, and says
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
