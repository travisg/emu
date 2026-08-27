# AGENTS.md

Guidance for AI coding agents working in this repository. `CLAUDE.md` imports this file, so Claude Code and other tools read the same instructions.

## Purpose

Terminal-driven emulator for several vintage computer systems: Motorola 6809 (System09), MITS Altair 680 (6800), Kaypro II (Z80, CP/M, SDL2 video window), RC2014 (Z80), and the Raytheon 703 (1967, 16-bit).

Written in Rust. Most of it is a port of an earlier C++ tree, validated against it instruction by instruction; the C++ tree was removed once the port was complete; its last version is commit `332e1cd`. Comments in the Rust cite C++ files and line numbers (`cpuz80.cpp:1196` and the like) — those resolve in that commit, e.g. `git show 332e1cd:cpu/cpuz80.cpp`. `rust-conversion-plan.md` is the history and rationale of the port.

The **Raytheon 703 is not a port** — the C++ never had it, so there is no oracle for it and no trace to match. It is written from the *703 Computer Reference and Interface Manual*; the parts of that scan an emulator needs are transcribed next to it as `Raytheon703refMan_isa.txt`, along with the PTB bootstrap and the teletype driver listings, and the comments in `src/cpu/ray703.rs` cite the manual's own section numbers (`2-7.6`, `1-3.3.2`). Darwin Geiselbrecht's `rustheon` and `Raytheon` emulators (github.com/IslandSparky, both MIT) were used as a cross-check, not as a base; where they and the manual disagree, the manual wins, and the disagreements are named at their use sites.

## Build

Prerequisites:

- A Rust toolchain (`cargo`, edition 2021)
- SDL2 development package (`libsdl2-dev` / `sdl2`) — the `sdl2` crate links it dynamically, and it is a hard dependency even for the terminal-only systems
- `pkg-config` — the `sdl2` crate is built with `use-pkgconfig`, so SDL2 is located by `pkg-config sdl2` rather than by the linker's default search path. That is what makes a non-default prefix work (MacPorts' `/opt/local`, Homebrew's `/opt/homebrew` on Apple silicon); set `PKG_CONFIG_PATH` if SDL2 lives somewhere `pkg-config` doesn't already look

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
./target/debug/emu -s ray703                        # Raytheon 703, banner + interrupt-driven echo
./target/debug/emu -s ray703-ptb -r roms/703/tape.tape   # ...loaded off paper tape instead
```

- Systems: `6809`, `altair680`, `kaypro`, `ray703`, `rc2014`. An optional subsystem suffix selects a variant (e.g. `6809-obc` — currently rejected with an explicit error, it needs an unported `uart16550`).
- `-c/--cpu` is accepted but ignored — the CPU is chosen by the system.
- `-l/--limit` bounds the run by *instruction* count (the name is historical); use it for non-interactive/automated runs. A limit of N executes N−1 instructions.
- `-t/--trace` writes one line of register state per instruction to a file. This was the cross-validation oracle format; it's still the fastest way to see what a machine is doing.
- The console puts the terminal in raw mode and passes Ctrl-C through to the guest. **Ctrl-D exits cleanly** (or close the SDL window for kaypro).
- The kaypro system additionally loads the floppy image `mbasic-games.img` from the current directory (`src/system/kaypro.rs`); it's not tracked, and is gitignored so a symlink in the checkout is fine.
- The 703 has no ROM — an operator keyed a bootstrap in from the front panel and fed the machine a paper tape, and there was nothing in core at power-on. There is no front panel here, so the subsystems stand in for it: `ray703` loads a flat core image from word 0 and starts there, and `ray703-ptb` keys in the real eleven-word PTB bootstrap instead and reinterprets `-r` as the tape to feed it. PTB is a loader and stops when the tape runs out, exactly as it did in 1968 — there is no way to start what it loaded, so that subsystem is groundwork for running transcribed period software rather than a way to run something today. Build both images with `make -C test ray703` (needs only python3); they land in `roms/703/`.

## Test

```bash
cargo test                       # unit tests; the oracle-gated cases report as ignored
cargo clippy --all-targets       # kept clean
```

Each CPU core has an in-module `mod tests` driving hand-assembled programs over `cpu::testbus::TestBus` (flat 64K plus an IO space, a `watch` counter for operand re-reads, and `run_steps`). These need no ROMs and no oracle, so they are what `cargo test` actually covers on a bare checkout. Several of them pin the deliberate quirks listed under Architecture. Derive expected values from the implementation, not from a datasheet, and keep a named test per quirk: those are what a future cleanup would silently break.

End-to-end regression: boots 6809 BASIC, feeds it `test/basic6809_lang_test.bas`, and checks the captured log for `BASIC LANGUAGE TEST PASS`:

```bash
cargo build                         # build first
./test/run_basic6809_lang_test.sh   # or EMU_BIN=./target/release/emu ./test/run_basic6809_lang_test.sh
```

Requires the `roms` symlink to resolve, plus `script(1)` and `perl`.

The 703 has the same shape of test: boot the demo image, type a line at it, and check the log for the upper-cased echo the guest produced and for a clean halt. It covers the core, the interrupt system, the DIO channel, the teletype and the frontend at once, because the demo's echo runs entirely out of a level 0 service routine.

```bash
make -C test ray703-test            # builds the image, then runs the test
./test/run_ray703_demo_test.sh      # if the image is already built
```

Needs `script(1)` and python3, and no oracle. No period ROM image is involved — the demo is built from source in this tree — but the `roms` symlink still has to resolve, because that is where `make -C test ray703` writes the image and where the registry's `default_rom` looks for it. The test waits for the banner to appear in the live log rather than sleeping: the emulator only starts listening once it has put the terminal in raw mode, and anything typed before that is eaten by the line discipline, which looks exactly like a broken emulator.

**Trace-diff against the C++ oracle** (`tests/trace_diff_{6800,6809,z80,kaypro}.rs`). These drive both implementations over the same ROM image and require byte-identical `--trace` output; they were the load-bearing gate for the port and remain the strongest regression check on the cores. All 59 oracle-dependent cases are `#[ignore]`d, so plain `cargo test` reports them as ignored rather than passing a comparison that never ran; the handful in those files that need only the Rust tree still run by default. To run the gate, build an oracle:

```bash
git worktree add /tmp/emu-cpp 332e1cd
git -C /tmp/emu-cpp submodule update --init     # libihex
make -C /tmp/emu-cpp                            # needs clang, make, sdl2-config, objdump
EMU_ORACLE=/tmp/emu-cpp/build-emu/emu cargo test -- --include-ignored
```

Once asked for explicitly, a missing oracle, ROM or floppy image is a hard failure with a message naming what is absent — the tests are opt-in, so silently passing would defeat the point. Run from the repo root (the ROMs and `mbasic-games.img` are resolved relative to it). About 2.5 minutes wall clock, dominated by spawning the oracle once per case. Two harness rules, learned the hard way and worth keeping: hold the child's stdin open for its whole life (`child.stdin.take()` before `wait()`), and assert trace *lengths* before comparing content.

The trace-diff gate does not and cannot cover the 703: there is no C++ 703 to diff against. Its `--trace` format is therefore ours to choose rather than something to match, and its regression cover is the in-module tests plus the demo test above. The last of the core's unit tests runs the actual PTB bootstrap over a synthetic tape, which is the closest thing to a period artifact available: it self-modifies, so it pins byte addressing, the interrupt frame and the idle loop at once.

`make -C test` rebuilds the 6809 test ROM sources — needs the ASxxxx toolchain (`as6809`, `aslink`) and `objcopy`; not required for normal development. `make -C test ray703` rebuilds the 703 demo and tape images with `test/asm703.py`, a small two-pass absolute assembler in stdlib python — the 703's own assembler (SYM II) exists only as scans.

Beyond that, verification is manual: build, check `-h` output, boot a system, confirm Ctrl-D shuts down cleanly. For kaypro also confirm the window renders and window-close exits.

## Architecture

Threading/lifecycle (`src/main.rs`): parse args → `registry::find(name)` → factory builds a `Machine { cpu, bus, display }` → `Emulator::new` → the whole emulator moves onto a spawned CPU thread running `Emulator::run()` while the main thread runs the frontend (`TerminalFrontend`, or `SdlFrontend` if the machine has a `display`). Only lightweight handles cross the thread boundary: an `mpsc` channel for keystrokes, an `Arc<AtomicBool>` shutdown flag, and for the Kaypro an `Arc<Mutex<..>>` video buffer plus dirty flag. Whichever side stops first sets the flag; the other notices and exits.

- **`Bus`** (`src/bus.rs`) — the trait a CPU talks to: `read8`/`write8` (the one required primitive), `io_read8`/`io_write8` for Z80 port I/O, `io_read16`/`io_write16` for the 703's 16-bit DIO channel, composed wide accessors parameterised by `Endian`, and two interrupt polls (`poll_interrupts` for the ported cores' irq/nmi pair, `poll_interrupt_lines` for the 703's sixteen prioritized levels). Each machine in `src/system/` implements it directly and does all its own address decoding, owns its `Memory` banks and devices, and loads its ROMs in its constructor. There is no back-reference from CPU to bus: the bus is passed into every `step()`.
- **`Cpu`** (`src/cpu/mod.rs`) — `reset`, `step` (exactly one instruction), `dump`, `trace_line`. Cores: `m6800.rs`, `m6809.rs` (table-driven), `z80.rs` (x/y/z decode with a prefix loop), `ray703.rs` (match-based, written from the manual rather than ported). They touch nothing but the `Bus`.
- **`Emulator`** (`src/emulator.rs`) — the run loop: shutdown check, instruction limit (decremented once per `step()`), trace, step. Returns an `ExitReason`.
- **`src/dev/`** — `MemoryDevice` implementors (`Memory` bank, `Mc6850`) and port-mapped devices driven by the machine (`Z80Sio`, `Wd1793`, read-only floppy over a `File`; `ray703::Tty703` and `ray703::TapeReader703` on the DIO channel).
- **`src/console/`** — `ConsoleEndpoint` (CPU-thread side: keystroke receiver + serial output sink), `ConsoleFrontend` trait with `terminal.rs` (raw termios, `poll()` with a timeout so a cycle-limit exit is noticed) and `sdl.rs` (window, event pump, font atlas from the video ROM, 80×24 render); `VideoBuffer`/`Display` for machines with a screen.
- **`src/rom.rs`** — flat-binary and Intel HEX loaders (`ihex` crate).

System metadata: `src/system/registry.rs` holds a static `SYSTEMS` table (`name`, `cpu`, `default_rom`, factory), which drives `-h` and the factory. Adding a machine = one entry there plus a `system/*.rs`. Never hardcode system/ROM info in `main.rs` — extend the table. `main.rs` chooses the frontend by whether the machine returned a `Display`, not by system name.

Known, deliberate quirks preserved from the C++ (all documented at their use sites; don't "fix" them casually, they are trace-validated behaviour): the 6800 `ASR` falls through into `LSR`; the Z80 `LD r,(IX+d)` adds `d` unsigned; the Z80 ED page is intentionally incomplete; `HALT` is a `NOP`; the RC2014 SIO never reports transmit-empty so its monitor never prints. `IntStatus`/`poll_interrupts` remain scaffolding: no ported machine asserts a line. See "Known defects reproduced, not fixed" in `rust-conversion-plan.md`.

### The 703

The Raytheon 703 is the one machine here that runs interrupts for real, and the only one with no C++ ancestor. Four things about it trip people up, and each has a named test:

- **Bit 0 is the most significant bit**, in every field description and every comment quoting the manual.
- **`EXR` is a *byte* page number**, so a word instruction uses only its top four bits: `SML 4` selects word page 2, not 4. It also reloads from the program counter after every memory reference, so `SML`/`SMU` govern exactly one instruction — compute the effective address before the reload, never after.
- **A branch to self is how this machine waits for I/O.** `JMP $` reports `InfiniteLoop` only when no level is enabled or the inhibit mask is on; copying the 6800's unconditional detection would break every real 703 program, starting with PTB.
- **Words 0-63 are the sixteen four-word interrupt blocks**, so a program cannot simply live at word 0 — word 0 is where the hardware saves the program counter on a level 0 interrupt.

Deliberate divergences, all documented at their use sites: `HLT` exits the emulator, because there is no front panel RUN switch to resume from; the external sense line and the four sense switches read false, so all five sense skips always skip; the optional multiply/divide hardware decodes as `BadOpcode`, since appendix B lists no opcodes for it; `DOT` completes instantly, so there are no output-completion interrupts. The saved machine status word's bit layout is the one thing here not taken from the manual — the manual names its contents but never diagrams it, so the layout follows rustheon's.

## Style

- Idiomatic Rust; `cargo clippy --all-targets` clean. The tree is not `rustfmt`-formatted wholesale — match the surrounding code rather than reformatting files.
- Files start with the modeline `// vim: ts=4:sw=4:expandtab:`, the MIT license header (copy it from any existing file; the text is in `LICENSE`), and a `//!` module doc explaining what the file is (and, where relevant, which C++ file it ports).
- Every non-obvious behaviour gets a comment saying *why*, especially anything preserved for oracle compatibility.
- Prefer minimal, localized diffs; avoid broad reformatting.

## Commits

Subject line convention is a lowercase `scope: description`, e.g. `z80: clean up the flag handling`, `kaypro: massive pile of changes`, `rust: add the z80 core and RC2014`, `docs: ...`. (Older history used `[scope] Description`; prefer the colon style.)
