# Emu: Terminal-driven Vintage System Emulator

A terminal-driven emulator for several vintage computer systems, written in Rust.

## Supported Systems

- **System09 (Motorola 6809)**: boots the 6809 BASIC ROM to a prompt on the terminal.
- **MITS Altair 680 (Motorola 6800)**: boots the MITS monitor ROM on the terminal.
- **RC2014 (Zilog Z80)**: boots the factory ROM image — Grant Searle's monitor into Microsoft
  BASIC 4.7b — on the terminal, over an interrupt-driven serial port.
- **Kaypro II (Zilog Z80)**: boots CP/M 2.2 from a floppy image into an SDL2 window, with a
  keyboard and a read-only floppy.

## Prerequisites

- A Rust toolchain (`cargo`)
- The SDL2 development package (`libsdl2-dev` on Debian/Ubuntu, `sdl2` on Homebrew/MacPorts) — the
  Kaypro window links it dynamically, and it's a build requirement even if you never run the Kaypro
- `pkg-config`, which is how the build finds that SDL2: any install prefix works as long as
  `pkg-config sdl2` resolves, so MacPorts (`/opt/local`) and Homebrew (`/usr/local` or
  `/opt/homebrew`) need no configuration. Point `PKG_CONFIG_PATH` at the directory holding
  `sdl2.pc` if it's installed somewhere unusual
- ROM images. They aren't in the repo; the emulator looks for them under `roms/` relative to the
  current directory (`-h` lists the default path per system, `-r` overrides it). `tools/fetch-roms.py`
  fetches the third-party images listed in `tools/rom-manifest.txt` and checks them against their
  hashes; the Raytheon 703 guests need no fetching and are built with `make -C test ray703`

## Building

```bash
cargo build              # target/debug/emu
cargo build --release    # target/release/emu, considerably faster
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
- `tests/`: the one integration test. `test/`: the end-to-end regression scripts, the assembly
  sources of the 6809 test ROMs, and everything Raytheon 703 (`test/703/`, including a Tiny BASIC
  and transcriptions of two 1968 program listings).
- `tools/`: development tools — the 703 assembler, the listing transcription workflow, and the
  ROM fetcher.
- `roms/`, `disks/`: where the images the machines boot and mount go. Untracked; see the README
  in each.
- `AGENTS.md`: the full guide — build, run, test, architecture, and the deliberate quirks.
