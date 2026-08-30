# Emu: Terminal-driven Vintage System Emulator

A terminal-driven emulator for several vintage computer systems, written in Rust.

## Supported Systems

- **System09 (Motorola 6809)**: boots the 6809 BASIC ROM to a prompt on the terminal.
- **MITS Altair 680 (Motorola 6800)**: boots the MITS monitor ROM on the terminal.
- **RC2014 (Zilog Z80)**: boots the factory ROM image — Grant Searle's monitor into Microsoft
  BASIC 4.7b — on the terminal, over an interrupt-driven serial port.
- **Kaypro II (Zilog Z80)**: boots CP/M 2.2 from a floppy image into an SDL2 window, with a
  keyboard and a read-only floppy.
- **Raytheon 703 (1967, 16-bit)**: a machine with no ROM at all, running sixteen levels of real
  interrupts, an interrupt-driven teletype, a fixed-head disc and a working front panel. Not a
  port of anything — it is written from the manufacturer's manuals. See below.

## Prerequisites

- A Rust toolchain (`cargo`)
- The SDL2 development package (`libsdl2-dev` on Debian/Ubuntu, `sdl2` on Homebrew/MacPorts) — the
  Kaypro window links it dynamically, and it's a build requirement even if you never run the Kaypro
- `pkg-config`, which is how the build finds that SDL2: any install prefix works as long as
  `pkg-config sdl2` resolves, so MacPorts (`/opt/local`) and Homebrew (`/usr/local` or
  `/opt/homebrew`) need no configuration. Point `PKG_CONFIG_PATH` at the directory holding
  `sdl2.pc` if it's installed somewhere unusual
- ROM images, which are not in the repo — see [ROM and disk images](#rom-and-disk-images) below.
  Only the 703 runs without any

## Building

```bash
cargo build              # target/debug/emu
cargo build --release    # target/release/emu, considerably faster
```

## ROM and disk images

None of the images the machines boot are in the repository. They are third-party ROM dumps and a
CP/M floppy, and their licensing does not allow redistribution — most of them are somebody's build
of Microsoft BASIC. `tools/rom-manifest.txt` records what each one is and what it hashes to, and

```bash
tools/fetch-roms.py      # into roms/ and disks/, verified against the manifest
```

fetches them: from a local archive if `EMU_ROM_ARCHIVE` names one, otherwise from the source that
publishes it. Four of the six have a public home and arrive on their own; the Altair 680b monitor
and the Kaypro floppy do not, and the script names them so they can be put in place by hand. An
image already there is checked and never overwritten.

The emulator looks for them under `roms/` and `disks/` relative to the **current directory**, so
run it from the top of the checkout. `-h` lists the default ROM per system and `-r` overrides it.

The Raytheon 703 needs none of this. It has no ROM to fetch, and its guests are built from source
in this tree:

```bash
make -C test ray703      # assembles every 703 guest into roms/703
```

## Running

Show the help message and the supported systems with their default ROMs:

```bash
./target/debug/emu -h
```

Run a system:

```bash
./target/debug/emu                          # the default system (6809)
./target/debug/emu -s altair680
./target/debug/emu -s kaypro                # opens a window; also loads disks/mbasic-games.img
./target/debug/emu -s 6809 -r roms/6809/BASIC.HEX
./target/debug/emu -s 6809 -l 1000000       # stop after a million instructions
./target/debug/emu -s 6809 -t trace.txt     # log one line of CPU state per instruction
```

The terminal systems run in raw mode and pass Ctrl-C through to the guest. **Ctrl-D exits**; so does
closing the Kaypro window.

## The Raytheon 703

A 16-bit machine from 1967, and the one system here that is not a port of an earlier emulator —
it is written from the manufacturer's own documentation. It runs sixteen prioritized interrupt
levels for real, and its teletype takes a tenth of a second per character in each direction, as a
Model 33 did.

There was nothing in core at power-on: an operator keyed a bootstrap in from the front panel and
fed the machine a paper tape. The subsystems stand in for the operator, so `-s ray703` loads a
core image and starts it.

```bash
make -C test ray703                                   # build the guests first

./target/debug/emu -s ray703                          # a demo: banner, then echoes what you type
./target/debug/emu -s ray703 -r roms/703/basic.bin    # Tiny BASIC, ~1300 words, written for it
./target/debug/emu -s ray703 -r roms/703/xray.bin     # X-RAY EXEC, transcribed from a 1968 listing
```

Ctrl-D exits. X-RAY wants **Ctrl-J before a command and Return after it** — its records open on a
line feed and close on a carriage return, and it silently discards anything typed without the
leading Ctrl-J.

The front panel from the manual's figure 5-1 opens in a window, and it works: the lamps are
switch-indicators, so clicking one keys that bit into a register, and they glow at the duty cycle
of the bit behind them rather than flickering. **A panel machine starts halted, as the real one did
at power-on — press RUN.**

```bash
./target/debug/emu -s ray703-panel                    # throttled to 571 kHz so the lamps move
./target/debug/emu -s ray703-panel-ptb -r roms/703/tape.tape --throttle 10   # a tape, in slow motion
./target/debug/emu -s ray703 --fast-io                # skip the teletype's real 10 chars/sec
```

The disc is a Raytheon 74601 fixed-head unit, and its controller's LOAD button boots from it:

```bash
make -C test ray703-boot-disc
./target/debug/emu -s ray703-load -r disks/ray703-boot.img
```

### Where the documentation came from

All of it is on **bitsavers**, at <https://bitsavers.org/pdf/raytheon/70x/> — `Raytheon703refMan.pdf`
is the reference manual the CPU is written from, `Raytheon706usersMan_Feb70.pdf` documents the disc
in §5-9, and the program listings there are the 1968 software. Two of them,
`390779_XRAY_ExecBasic_Feb1968.pdf` and `390682C_RelocatingLoaderBasic_Nov1968.pdf`, are transcribed
by hand in `test/703/listings/` and assembled back into running code; the scans have no text layer,
so every card was read off the page. `test/703/README.md` is the index and the to-do list, and the
comments in `src/cpu/ray703.rs` cite the manual's own section numbers.

### Other emulators of this machine

Darwin Geiselbrecht, who wrote a lot of assembly for 703s when they were current hardware, has
written two: [rustheon](https://github.com/IslandSparky/rustheon) in Rust and
[Raytheon](https://github.com/IslandSparky/Raytheon) in Python. Both were an inspiration for this
one and the reference it was checked against — on a machine this obscure, someone who actually used
it is worth as much as a manual.

They were a cross-check rather than a base, and where they and the manual disagree the manual wins,
with the disagreement written down at the line it affects. `rustheon` decodes the register generics
one slot high, for instance — `CLR` at `0x011` rather than `0x010` — and the 1968 paper-tape
bootstrap, which packs eight distinct instructions into eleven words, sides with the manual.

It goes the other way once. The saved machine status word is the only thing in the core not taken
from the manual, which names the word's contents but never diagrams it, so the bit layout follows
`rustheon`'s. That turned out to be the layout of the front panel's own MS lamps in figure 5-1 —
which is a good sign it was right.

## Testing

```bash
cargo test                          # the whole suite: no ROMs, no external binaries
./test/run_basic6809_lang_test.sh   # end-to-end: boots BASIC and runs a language test program
```

`cargo test` needs nothing outside the repo. The end-to-end scripts do: `run_basic6809_lang_test.sh`
needs `roms/6809/BASIC.HEX` plus `script(1)` and `perl`, and the Raytheon 703 scripts
(`make -C test ray703-test` and friends) need `script(1)` and python3. `AGENTS.md` covers them all.

## Project Structure

- `src/cpu/`: CPU cores (6800, 6809, Z80, Raytheon 703), each implementing the `Cpu` trait against a `Bus`.
- `src/system/`: one file per machine (the bus, address decode, devices, ROM loading) and the
  registry that describes them.
- `src/dev/`: devices — memory banks, MC6850 ACIA, Z80 SIO, WD1793 floppy controller.
- `src/console/`: the terminal and SDL2 frontends, and the channel/handles that connect them to
  the CPU thread.
- `src/emulator.rs`, `src/bus.rs`, `src/rom.rs`, `src/main.rs`.
- `tests/`: the one integration test. `test/`: the end-to-end regression scripts and the guest
  programs they drive, by machine — `test/6809/` and `test/703/` (the latter including a Tiny
  BASIC and transcriptions of two 1968 program listings). See `test/README.md`.
- `tools/`: development tools — the 703 assembler, the listing transcription workflow, and the
  ROM fetcher.
- `roms/`, `disks/`: where the images the machines boot and mount go. Untracked; see the README
  in each.
- `AGENTS.md`: the full guide — build, run, test, architecture, and the deliberate quirks.
