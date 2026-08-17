# Emu: Terminal-driven Vintage System Emulator

A terminal-driven emulator for several vintage computer systems, written in Rust.

## Supported Systems

- **System09 (Motorola 6809)**: boots the 6809 BASIC ROM to a prompt on the terminal.
- **MITS Altair 680 (Motorola 6800)**: boots the MITS monitor ROM on the terminal.
- **RC2014 (Zilog Z80)**: runs the factory ROM image (see the note in `rust-conversion-plan.md`
  about its serial port).
- **Kaypro II (Zilog Z80)**: boots CP/M 2.2 from a floppy image into an SDL2 window, with a
  keyboard and a read-only floppy.

## Prerequisites

- A Rust toolchain (`cargo`)
- The SDL2 development package (`libsdl2-dev` on Debian/Ubuntu, `sdl2` on Homebrew/MacPorts) — the
  Kaypro window links it dynamically, and it's a build requirement even if you never run the Kaypro
- ROM images. They aren't in the repo; the emulator looks for them under `roms/` relative to the
  current directory (`-h` lists the default path per system, `-r` overrides it)

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
./target/debug/emu -s kaypro                # opens a window; also loads mbasic-games.img from the cwd
./target/debug/emu -s 6809 -r roms/6809/BASIC.HEX
./target/debug/emu -s 6809 -l 1000000       # stop after a million instructions
./target/debug/emu -s 6809 -t trace.txt     # log one line of CPU state per instruction
```

The terminal systems run in raw mode and pass Ctrl-C through to the guest. **Ctrl-D exits**; so does
closing the Kaypro window.

## Testing

```bash
cargo test                          # unit tests; the trace-diff suites skip without an oracle
./test/run_basic6809_lang_test.sh   # end-to-end: boots BASIC and runs a language test program
```

The `tests/trace_diff_*.rs` suites compare this emulator instruction by instruction against the
original C++ implementation it was ported from. The C++ tree is gone from the working tree but not
from history — `AGENTS.md` has the recipe for building it in a worktree and pointing `EMU_ORACLE` at
it.

## Project Structure

- `src/cpu/`: CPU cores (6800, 6809, Z80), each implementing the `Cpu` trait against a `Bus`.
- `src/system/`: one file per machine (the bus, address decode, devices, ROM loading) and the
  registry that describes them.
- `src/dev/`: devices — memory banks, MC6850 ACIA, Z80 SIO, WD1793 floppy controller.
- `src/console/`: the terminal and SDL2 frontends, and the channel/handles that connect them to
  the CPU thread.
- `src/emulator.rs`, `src/bus.rs`, `src/rom.rs`, `src/main.rs`.
- `tests/`: trace-diff suites. `test/`: the 6809 BASIC regression and the assembly sources of the
  6809 test ROMs.
- `rust-conversion-plan.md`: how the port from C++ was done and validated.
