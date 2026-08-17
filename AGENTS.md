# AGENTS.md

Guidance for AI coding agents working in this repository. `CLAUDE.md` imports this file, so Claude Code and other tools read the same instructions.

## Purpose

Terminal-driven emulator for several vintage computer systems: Motorola 6809 (System09), MITS Altair 680 (6800), Kaypro II (Z80, CP/M, SDL2 video window), and RC2014 (Z80).

Written in Rust. It is a port of an earlier C++ tree, validated against it instruction by instruction; the C++ tree was removed once the port was complete; its last version is commit `332e1cd`. Comments in the Rust cite C++ files and line numbers (`cpuz80.cpp:1196` and the like) — those resolve in that commit, e.g. `git show 332e1cd:cpu/cpuz80.cpp`. `rust-conversion-plan.md` is the history and rationale of the port.

## Build

Prerequisites:

- A Rust toolchain (`cargo`, edition 2021)
- SDL2 development package (`libsdl2-dev` / `sdl2`) — the `sdl2` crate links it dynamically, and it is a hard dependency even for the terminal-only systems

```bash
cargo build              # target/debug/emu
cargo build --release    # target/release/emu -- the interpreter cores are much faster here
```

Dependencies are deliberately few: `ihex` (Intel HEX), `libc` (termios/poll), `sdl2`. Don't add more without a reason.

## Run

Run from the repo root — default ROM paths are relative (`roms/...`), and `roms` is a symlink to storage outside the repo (ROM images are not tracked in git).

```bash
./target/debug/emu -h                  # help: lists systems, cpus, default ROMs
./target/debug/emu                     # default system (6809)
./target/debug/emu -s kaypro           # Kaypro II, opens an SDL window
./target/debug/emu -s 6809 -r roms/6809/BASIC.HEX   # override the ROM
./target/debug/emu -s 6809 -l 10000000              # stop after N instructions
./target/debug/emu -s 6809 -l 100000 -t /tmp/t.txt  # plus one line of CPU state per instruction
```

- Systems: `6809`, `altair680`, `kaypro`, `rc2014`. An optional subsystem suffix selects a variant (e.g. `6809-obc` — currently rejected with an explicit error, it needs an unported `uart16550`).
- `-c/--cpu` is accepted but ignored — the CPU is chosen by the system.
- `-l/--limit` bounds the run by *instruction* count (the name is historical); use it for non-interactive/automated runs. A limit of N executes N−1 instructions.
- `-t/--trace` writes one line of register state per instruction to a file. This was the cross-validation oracle format; it's still the fastest way to see what a machine is doing.
- The console puts the terminal in raw mode and passes Ctrl-C through to the guest. **Ctrl-D exits cleanly** (or close the SDL window for kaypro).
- The kaypro system additionally loads the floppy image `mbasic-games.img` from the current directory (`src/system/kaypro.rs`); it's not tracked, and is gitignored so a symlink in the checkout is fine.

## Test

```bash
cargo test                       # unit tests + trace-diff suites (which skip without an oracle)
cargo clippy --all-targets       # kept clean
```

End-to-end regression: boots 6809 BASIC, feeds it `test/basic6809_lang_test.bas`, and checks the captured log for `BASIC LANGUAGE TEST PASS`:

```bash
cargo build                         # build first
./test/run_basic6809_lang_test.sh   # or EMU_BIN=./target/release/emu ./test/run_basic6809_lang_test.sh
```

Requires the `roms` symlink to resolve, plus `script(1)` and `perl`.

**Trace-diff against the C++ oracle** (`tests/trace_diff_{6800,6809,z80,kaypro}.rs`). These drive both implementations over the same ROM image and require byte-identical `--trace` output; they were the load-bearing gate for the port and remain the strongest regression check on the cores. They skip themselves unless `EMU_ORACLE` points at a C++ binary. To build one:

```bash
git worktree add /tmp/emu-cpp 332e1cd
git -C /tmp/emu-cpp submodule update --init     # libihex
make -C /tmp/emu-cpp                            # needs clang, make, sdl2-config, objdump
EMU_ORACLE=/tmp/emu-cpp/build-emu/emu cargo test
```

Run from the repo root (the ROMs and `mbasic-games.img` are resolved relative to it). About 2.5 minutes wall clock, dominated by spawning the oracle once per case. Two harness rules, learned the hard way and worth keeping: hold the child's stdin open for its whole life (`child.stdin.take()` before `wait()`), and assert trace *lengths* before comparing content.

`make -C test` rebuilds the 6809 test ROM sources — needs the ASxxxx toolchain (`as6809`, `aslink`) and `objcopy`; not required for normal development.

Beyond that, verification is manual: build, check `-h` output, boot a system, confirm Ctrl-D shuts down cleanly. For kaypro also confirm the window renders and window-close exits.

## Architecture

Threading/lifecycle (`src/main.rs`): parse args → `registry::find(name)` → factory builds a `Machine { cpu, bus, display }` → `Emulator::new` → the whole emulator moves onto a spawned CPU thread running `Emulator::run()` while the main thread runs the frontend (`TerminalFrontend`, or `SdlFrontend` if the machine has a `display`). Only lightweight handles cross the thread boundary: an `mpsc` channel for keystrokes, an `Arc<AtomicBool>` shutdown flag, and for the Kaypro an `Arc<Mutex<..>>` video buffer plus dirty flag. Whichever side stops first sets the flag; the other notices and exits.

- **`Bus`** (`src/bus.rs`) — the trait a CPU talks to: `read8`/`write8` (the one required primitive), `io_read8`/`io_write8` for Z80 port I/O, composed wide accessors parameterised by `Endian`, and `poll_interrupts`. Each machine in `src/system/` implements it directly and does all its own address decoding, owns its `Memory` banks and devices, and loads its ROMs in its constructor. There is no back-reference from CPU to bus: the bus is passed into every `step()`.
- **`Cpu`** (`src/cpu/mod.rs`) — `reset`, `step` (exactly one instruction), `dump`, `trace_line`. Cores: `m6800.rs`, `m6809.rs` (table-driven), `z80.rs` (x/y/z decode with a prefix loop). They touch nothing but the `Bus`.
- **`Emulator`** (`src/emulator.rs`) — the run loop: shutdown check, instruction limit (decremented once per `step()`), trace, step. Returns an `ExitReason`.
- **`src/dev/`** — `MemoryDevice` implementors (`Memory` bank, `Mc6850`) and port-mapped devices driven by the machine (`Z80Sio`, `Wd1793`, read-only floppy over a `File`).
- **`src/console/`** — `ConsoleEndpoint` (CPU-thread side: keystroke receiver + serial output sink), `ConsoleFrontend` trait with `terminal.rs` (raw termios, `poll()` with a timeout so a cycle-limit exit is noticed) and `sdl.rs` (window, event pump, font atlas from the video ROM, 80×24 render); `VideoBuffer`/`Display` for machines with a screen.
- **`src/rom.rs`** — flat-binary and Intel HEX loaders (`ihex` crate).

System metadata: `src/system/registry.rs` holds a static `SYSTEMS` table (`name`, `cpu`, `default_rom`, factory), which drives `-h` and the factory. Adding a machine = one entry there plus a `system/*.rs`. Never hardcode system/ROM info in `main.rs` — extend the table. `main.rs` chooses the frontend by whether the machine returned a `Display`, not by system name.

Known, deliberate quirks preserved from the C++ (all documented at their use sites; don't "fix" them casually, they are trace-validated behaviour): the 6800 `ASR` falls through into `LSR`; the Z80 `LD r,(IX+d)` adds `d` unsigned; the Z80 ED page is intentionally incomplete; `HALT` is a `NOP`; the RC2014 SIO never reports transmit-empty so its monitor never prints. Interrupts are scaffolding only — no machine asserts a line. See "Known defects reproduced, not fixed" in `rust-conversion-plan.md`.

## Style

- Idiomatic Rust; `cargo clippy --all-targets` clean. The tree is not `rustfmt`-formatted wholesale — match the surrounding code rather than reformatting files.
- Files start with the modeline `// vim: ts=4:sw=4:expandtab:`, the MIT license header (copy it from any existing file; the text is in `LICENSE`), and a `//!` module doc explaining what the file is (and, where relevant, which C++ file it ports).
- Every non-obvious behaviour gets a comment saying *why*, especially anything preserved for oracle compatibility.
- Prefer minimal, localized diffs; avoid broad reformatting.

## Commits

Subject line convention is a lowercase `scope: description`, e.g. `z80: clean up the flag handling`, `kaypro: massive pile of changes`, `rust: add the z80 core and RC2014`, `docs: ...`. (Older history used `[scope] Description`; prefer the colon style.)
