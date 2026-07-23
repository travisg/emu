# AGENTS.md

Guidance for AI coding agents working in this repository. `CLAUDE.md` imports this file, so Claude Code and other tools read the same instructions.

## Purpose

Terminal-driven emulator for several vintage computer systems: Motorola 6809 (System09), MITS Altair 680 (6800), Kaypro II (Z80, CP/M, SDL2 video window), and RC2014 (Z80).

## Build

Prerequisites:
- `clang` / `clang++` (hard-set in the makefile) and GNU `make`
- SDL2 development package — the makefile invokes `sdl2-config` unconditionally
- `objdump` (Linux) or `otool` (macOS) for the listing file
- The `libihex` git submodule: run `git submodule update --init` on a fresh clone

Build from repo root:

```bash
make
```

Outputs: `build-emu/emu` (the emulator, C++17) and `build-emu/emu.lst` (disassembly of the emulator itself).

`make clean` removes objects but not the binary; `make spotless` removes `build-*` entirely.

## Run

Run from the repo root — default ROM paths are relative (`roms/...`), and `roms` is a symlink to storage outside the repo (ROM images are not tracked in git).

```bash
./build-emu/emu -h                  # help: lists systems, cpus, default ROMs
./build-emu/emu                     # default system (6809)
./build-emu/emu -s kaypro           # Kaypro II, opens an SDL window
./build-emu/emu -s 6809 -r roms/6809/BASIC.HEX   # override the ROM
./build-emu/emu -s 6809 -l 10000000              # stop after N cycles
```

- Systems: `6809`, `altair680`, `kaypro`, `rc2014`. An optional subsystem suffix selects a variant (e.g. `6809-obc`).
- `-c/--cpu` is accepted but ignored — the CPU is chosen by the system.
- `-l/--limit` bounds the run by cycle count; use it for non-interactive/automated runs.
- The console puts the terminal in raw mode and passes Ctrl-C through to the guest. **Ctrl-D exits cleanly** (or close the SDL window for kaypro).
- The kaypro system additionally loads the floppy image `mbasic-games.img` from the current directory (`system/system_kaypro.cpp`).

## Test

End-to-end regression: boots 6809 BASIC, feeds it `test/basic6809_lang_test.bas`, and checks the captured log for `BASIC LANGUAGE TEST PASS`:

```bash
make                                # build first
./test/run_basic6809_lang_test.sh   # also reachable as: make -C test basic6809-test
```

Requires the `roms` symlink to resolve, plus `script(1)` and `perl`.

`make -C test` rebuilds the 6809 test ROM sources — needs the ASxxxx toolchain (`as6809`, `aslink`); not required for normal development.

Beyond that, verification is manual: build, check `-h` output, boot a system, confirm Ctrl-D shuts down cleanly.

## Architecture

Threading/lifecycle (`main.cpp`): parse args → `System::Factory(name)` → `Init()` → `RunThreaded()` spawns a `std::thread` running the CPU loop while the main thread blocks in `Console::Run()`. Ctrl-D exits the console loop, then `ShutdownThreaded()` sets an atomic flag the CPU loop polls each instruction. If the CPU loop exits first (e.g. cycle limit), it stops the console instead.

- **`System`** (`system/system.h`) — abstract machine, one `final` subclass per system in `system/`. The System *is* the bus: subclasses implement `MemRead8`/`MemWrite8` (plus `IORead8`/`IOWrite8` for Z80 port I/O) and do all address decoding, own the `Memory` banks and devices, and load their own ROMs in `Init()` (Intel HEX via the `libihex` submodule, or flat-binary `fread`). Default ROM paths are per-system `#define DEFAULT_ROM` in each `system/*.cpp`.
- **`Cpu`** (`cpu/cpu.h`) — abstract core holding a `System &`; all bus access goes through the System. Cores (`cpu6800`, `cpu6809`, `cpuz80`) are switch-based interpreters that check the shutdown flag and the global `g_cycle_limit` every instruction.
- **`dev/`** — `MemoryDevice` interface; `Memory` (flat RAM/ROM, no bounds checking); UARTs (`mc6850`, `uart16550`, `z80sio`) and the `wd1793` floppy controller (Kaypro, backed by a raw disk-image file).
- **Console** — base `Console` (`console.cpp`) handles raw-termios stdin and queues input; each system wires the input callback into its UART device. `ConsoleSDL` (Kaypro) opens an SDL2 window and renders the 80×24 video RAM through a font extracted from the video ROM.

System metadata: each system exposes a static `GetSystemInfo()`, aggregated by `System::GetSupportedSystems()`, which drives the `-h` output. Never hardcode system/ROM info in `main.cpp` — extend the metadata.

Debug tracing: `trace.h` provides `LTRACEF`-style macros gated by a per-file `#define LOCAL_TRACE 0/1` — the standard debug knob throughout the codebase.

## Style

- `.clang-format` is authoritative: LLVM base, 4-space indent, attached (K&R) braces, `ColumnLimit: 0`, right-aligned pointers, braces required on all control statements.
- Files start with the modeline `// vim: ts=4:sw=4:expandtab:` and an MIT license header.
- Naming: `PascalCase` types and methods, `mCamelCase` members, `g_` globals.
- Prefer minimal, localized diffs; avoid broad reformatting.

## Commits

Subject line convention is a lowercase `scope: description`, e.g. `z80: clean up the flag handling`, `kaypro: massive pile of changes`. (Older history used `[scope] Description`; prefer the colon style.)
