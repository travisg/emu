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
./target/debug/emu -s ray703-panel                  # ...with the front panel in an SDL window
```

- Systems: `6809`, `altair680`, `kaypro`, `ray703`, `rc2014`. An optional subsystem suffix selects a variant (e.g. `6809-obc` — currently rejected with an explicit error, it needs an unported `uart16550`).
- `-c/--cpu` is accepted but ignored — the CPU is chosen by the system.
- `-l/--limit` bounds the run by *instruction* count (the name is historical); use it for non-interactive/automated runs. A limit of N executes N−1 instructions.
- `-t/--trace` writes one line of register state per instruction to a file. This was the cross-validation oracle format; it's still the fastest way to see what a machine is doing.
- `--throttle` paces the CPU to the machine's real clock rate (from the registry's `clock_hz`); `--throttle N` paces to N Hz, which is slow motion for free. Cores report cycles per step via `Cpu::last_step_cycles`; a core that reports 0 (all the ported ones, so far) announces itself once and runs uncapped. Only the 703 counts cycles today, from appendix B's opcode index.
- `--fast-io` makes devices complete I/O instantly instead of at their period rates — currently that means the 703's teletype, which otherwise takes its real tenth of a second per character. It is a different axis from `--throttle` (whether device models charge machine time, versus whether machine time is paced against the wall clock), so the two compose: `-s ray703-panel --fast-io` is a real-time panel with an instant terminal. The flag reaches every machine through `registry::MachineOpts`; a machine with no device timing to disable ignores it.
- The console puts the terminal in raw mode and passes Ctrl-C through to the guest. **Ctrl-D exits cleanly** (or close the SDL window for kaypro).
- The kaypro system additionally loads the floppy image `mbasic-games.img` from the current directory (`src/system/kaypro.rs`); it's not tracked, and is gitignored so a symlink in the checkout is fine.
- The 703 has no ROM — an operator keyed a bootstrap in from the front panel and fed the machine a paper tape, and there was nothing in core at power-on. The subsystems stand in for the operator: `ray703` loads a flat core image from word 0 and starts there, and `ray703-ptb` keys in the real eleven-word PTB bootstrap instead and reinterprets `-r` as the tape to feed it. PTB is a loader and stops when the tape runs out, exactly as it did in 1968 — there is no way to start what it loaded, so that subsystem is groundwork for running transcribed period software rather than a way to run something today. Build the guest images with `make -C test ray703` (needs only python3); they land in `roms/703/`. That includes `basic.bin` — a Tiny BASIC written for this machine, see Test below — run with `./target/debug/emu -s ray703 -r roms/703/basic.bin`.
- Adding a `panel` token (`ray703-panel`, `ray703-panel-ptb`) opens the front panel from figure 5-1 of the manual in an SDL window, and it is a working panel: the PROGRAM COUNTER lamps, the SELECTED DISPLAY row behind the six-position DISPLAY SELECTOR (click the knob or a label, or Tab), RUN/HALT/RESET/SINGLE COMMAND, the CLEARs, ENTER/DISPLAY memory access, the SENSE toggles (click, or keys 1–4) driving the SS0–SS3 skips — and the lamps are switch-indicators, so clicking one keys that bit into the PCR or the selected MB/IX/AC register. The lamps render as incandescent bulbs: per-bit duty cycle over each frame through a thermal filter, so a bit lit 30% of the time glows at 30% instead of flickering. **A panel machine starts halted, like the real one at power-on — press RUN.** The whole 1968 boot ritual works: run PTB, HALT the idle loop when the tape ends, key the start address into the PC row, RUN. The teletype stays on the terminal; keys into the panel window are deliberately not forwarded to the guest. Panel machines default `--throttle` on at the 703's 4/7 MHz so the lamps move at period speed; `ray703-panel-ptb --throttle 10` watching the index register crawl through a tape load is the slow-motion demo.

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

**Tiny BASIC** (`test/703/basic.asm`, ~1300 words) is the tree's largest guest: a Palo Alto-class 16-bit integer BASIC — variables A–Z, `@(0..1023)`, `PRINT INPUT LET IF GOTO GOSUB RETURN FOR NEXT REM END`, `LIST RUN NEW`, `RND/ABS/SIZE`, `BYE` to halt — with an interrupt-driven teletype on the demo's model and its `*`/`/` running on the hardware MPY/DIV. `make -C test ray703-basic` builds it; `./target/debug/emu -s ray703 -r roms/703/basic.bin` talks to it; `make -C test ray703-basic-test` runs the scripted session (`test/run_ray703_basic_test.sh`): types a program exercising the editor, evaluator, FOR/GOSUB, the array, RND's bounds and INPUT, RUNs it, and greps for the outputs plus the clean `BYE` halt. One rule for driving it from a script: **pace every line on the guest falling silent**, not merely on the prompt appearing. Keys typed while the previous line is still being processed are dropped, Ctrl-C excepted, exactly as a busy 1968 machine dropped what a Model 33 fed it — and since the teletype prints at ten characters a second, the last character of `READY` or `? ` reaches the terminal while the guest is still spinning for it to drain, a good tenth of a second before the `T.GETL` that opens the line buffer. So wait for the prompt *and then for the output to stop growing*, which is `wait_quiet` in `test/run_ray703_basic_test.sh` and was the printer going quiet for the operator. The source's header comment is the map: page-per-ORG layout, SMB/JSX pairs for every cross-page transfer, no direct byte references, characters kept bit-7-set end to end.

**Trace-diff against the C++ oracle** (`tests/trace_diff_{6800,6809,z80,kaypro}.rs`). These drive both implementations over the same ROM image and require byte-identical `--trace` output; they were the load-bearing gate for the port and remain the strongest regression check on the cores. All 59 oracle-dependent cases are `#[ignore]`d, so plain `cargo test` reports them as ignored rather than passing a comparison that never ran; the handful in those files that need only the Rust tree still run by default. To run the gate, build an oracle:

```bash
git worktree add /tmp/emu-cpp 332e1cd
git -C /tmp/emu-cpp submodule update --init     # libihex
make -C /tmp/emu-cpp                            # needs clang, make, sdl2-config, objdump
EMU_ORACLE=/tmp/emu-cpp/build-emu/emu cargo test -- --include-ignored
```

Once asked for explicitly, a missing oracle, ROM or floppy image is a hard failure with a message naming what is absent — the tests are opt-in, so silently passing would defeat the point. Run from the repo root (the ROMs and `mbasic-games.img` are resolved relative to it). About 2.5 minutes wall clock, dominated by spawning the oracle once per case. Two harness rules, learned the hard way and worth keeping: hold the child's stdin open for its whole life (`child.stdin.take()` before `wait()`), and assert trace *lengths* before comparing content.

The trace-diff gate does not and cannot cover the 703: there is no C++ 703 to diff against. Its `--trace` format is therefore ours to choose rather than something to match, and its regression cover is the in-module tests plus the demo test above. The last of the core's unit tests runs the actual PTB bootstrap over a synthetic tape, which is the closest thing to a period artifact available: it self-modifies, so it pins byte addressing, the interrupt frame and the idle loop at once.

**Running the period software.** `test/703/listings/` holds hand transcriptions of two 1968 Raytheon program listings — X-RAY EXEC (DN 390779) and the relocating loader (DN 390682C). `make -C test ray703-listings` turns them into core images in `roms/703/`, and `./target/debug/emu -s ray703 -r roms/703/xray.bin` boots X-RAY. **Type Ctrl-J before a command and Return after it**: X-RAY's records open on a line feed and close on a carriage return, and without the leading Ctrl-J it silently discards everything typed. `make -C test ray703-verify` re-assembles a transcript and diffs it against the object code the 1968 assembler printed — the loader matches on all 596 words; X-RAY does not assemble yet and says why. `test/703/README.md` is the index and the to-do list.

`make -C test` rebuilds the 6809 test ROM sources — needs the ASxxxx toolchain (`as6809`, `aslink`) and `objcopy`; not required for normal development. `make -C test ray703` rebuilds the 703 demo and tape images with `test/asm703.py`, a small two-pass absolute assembler in stdlib python — the 703's own assembler (SYM II) exists only as scans.

Beyond that, verification is manual: build, check `-h` output, boot a system, confirm Ctrl-D shuts down cleanly. For kaypro also confirm the window renders and window-close exits.

## Architecture

Threading/lifecycle (`src/main.rs`): parse args → `registry::find(name)` → factory builds a `Machine { cpu, bus, display }` → `Emulator::new` → the whole emulator moves onto a spawned CPU thread running `Emulator::run()` while the main thread runs the frontend (`TerminalFrontend`, or `SdlFrontend` if the machine has a `display`). Only lightweight handles cross the thread boundary: an `mpsc` channel for keystrokes, an `Arc<AtomicBool>` shutdown flag, and for the Kaypro an `Arc<Mutex<..>>` video buffer plus dirty flag. Whichever side stops first sets the flag; the other notices and exits.

- **`Bus`** (`src/bus.rs`) — the trait a CPU talks to: `read8`/`write8` (the one required primitive), `io_read8`/`io_write8` for Z80 port I/O, `io_read16`/`io_write16` for the 703's 16-bit DIO channel, composed wide accessors parameterised by `Endian`, and two interrupt polls (`poll_interrupts` for the ported cores' irq/nmi pair, `poll_interrupt_lines` for the 703's sixteen prioritized levels). Each machine in `src/system/` implements it directly and does all its own address decoding, owns its `Memory` banks and devices, and loads its ROMs in its constructor. There is no back-reference from CPU to bus: the bus is passed into every `step()`.
- **`Cpu`** (`src/cpu/mod.rs`) — `reset`, `step` (exactly one instruction), `dump`, `trace_line`. Cores: `m6800.rs`, `m6809.rs` (table-driven), `z80.rs` (x/y/z decode with a prefix loop), `ray703.rs` (match-based, written from the manual rather than ported). They touch nothing but the `Bus`.
- **`Emulator`** (`src/emulator.rs`) — the run loop: shutdown check, instruction limit (decremented once per `step()`), trace, step. Returns an `ExitReason`.
- **`src/dev/`** — `MemoryDevice` implementors (`Memory` bank, `Mc6850`) and port-mapped devices driven by the machine (`Z80Sio`, `Wd1793`, read-only floppy over a `File`; `ray703::Tty703` and `ray703::TapeReader703` on the DIO channel).
- **`src/console/`** — `ConsoleEndpoint` (CPU-thread side: keystroke receiver + serial output sink), `ConsoleFrontend` trait with `terminal.rs` (raw termios, `poll()` with a timeout so a cycle-limit exit is noticed), `sdl.rs` (window, event pump, font atlas from the video ROM, 80×24 render) and `panel703.rs` (the 703 front panel, which also runs the terminal frontend on a second thread for the teletype); `VideoBuffer`/`PanelState`/`Display` for machines with something to show — `Display` is an enum whose variant picks the frontend.
- **`src/rom.rs`** — flat-binary and Intel HEX loaders (`ihex` crate).

System metadata: `src/system/registry.rs` holds a static `SYSTEMS` table (`name`, `cpu`, `default_rom`, factory), which drives `-h` and the factory. Adding a machine = one entry there plus a `system/*.rs`. Never hardcode system/ROM info in `main.rs` — extend the table. `main.rs` chooses the frontend by which `Display` variant the machine returned (or none), not by system name.

Known, deliberate quirks preserved from the C++ (all documented at their use sites; don't "fix" them casually, they are trace-validated behaviour): the 6800 `ASR` falls through into `LSR`; the Z80 `LD r,(IX+d)` adds `d` unsigned; the Z80 ED page is intentionally incomplete; `HALT` is a `NOP`; the RC2014 SIO never reports transmit-empty so its monitor never prints. `IntStatus`/`poll_interrupts` remain scaffolding: no ported machine asserts a line. See "Known defects reproduced, not fixed" in `rust-conversion-plan.md`.

### The 703

The Raytheon 703 is the one machine here that runs interrupts for real, and the only one with no C++ ancestor. Four things about it trip people up, and each has a named test:

- **Bit 0 is the most significant bit**, in every field description and every comment quoting the manual.
- **`EXR` is a *byte* page number**, so a word instruction uses only its top four bits: `SML 4` selects word page 2, not 4. It also reloads from the program counter after every memory reference, so `SML`/`SMU` govern exactly one instruction — compute the effective address before the reload, never after.
- **A branch to self is how this machine waits for I/O.** `JMP $` reports `InfiniteLoop` only when no level is enabled or the inhibit mask is on; copying the 6800's unconditional detection would break every real 703 program, starting with PTB.
- **Words 0-63 are the sixteen four-word interrupt blocks**, so a program cannot simply live at word 0 — word 0 is where the hardware saves the program counter on a level 0 interrupt.
- **Output is interrupt-driven too, on the same level as input.** A `DOT 14,E` raises the teletype's interrupt when the character has been printed, and that completion is the only thing that advances a driver's output loop; the device and the demo are both built around it. One line serves both directions, so a program tells them apart by knowing what it started, not by asking the hardware — X-RAY's level 0 stub is commented "FOR TY AND HSPT" and its driver tests the sign of the operation word.
- **The teletype takes a tenth of a second per character, in both directions.** The driver listing says the Model 33 "runs at up to ten characters per second", so a write's completion interrupt comes 57,142 clock cycles after the `DOT`, and the keyboard hands over at most one character per 57,142 cycles however fast a pipe feeds it. It is paced in *machine* time, not off a wall clock, which is what makes it compose with `--throttle`: throttled, that is ten a second of real time; unthrottled, ten per 57,142 emulated cycles, which is all a guest can observe anyway. The cycle count reaches the device through `Bus::poll_interrupt_lines`, which the 703 core hands the previous instruction's cycles; `--fast-io` switches the pacing off and returns the teletype to completing on the next poll. Two guest bugs fell out of turning this on, both latent since the day they were written and both invisible while a character completed on the next instruction: BASIC's banner was reclaimed by the first `PRINT` after one character, and its line buffer took a keystroke through the null `T.INPP` that only `T.GETL` primes, straight over word 0.
- **A collecting DIN starts the read**; there is no arming DOT on X-RAY's read path at all. And the collect function is the read function with bit 2 set, not a constant: function 9 collects with D, function B with F. The driver derives it by exclusive-oring `X'8004'`, which is also what clears the sign bit marking a read. Every DIO function code the tree relies on, `DOT dev,0` as the disconnect included, is cited at its constant in `src/dev/ray703.rs`.
- **Carriage return and line feed are distinct and nothing folds them**, neither the device nor the terminal's raw mode. They are separate keys on a Model 33 and separate characters to the software: X-RAY's record format opens on a line feed and closes on a carriage return, so a guest driven with either folded into the other is unreachable. A program that wants to accept both does it itself, as `test/703/demo.asm` does.
- **The multiply/divide option is installed, and it is the machine's only two-word instruction.** Appendix B never lists it — the option was a plug-in card — but section 6's format diagrams encode it outright: `MPY` = first word `0B0F`, `DIV` = `0C0F`, second word a flat 15-bit word address with no EXR page and no indexing. The 31-bit double format is IXR high, ACR bits 1–15 low with the sign duplicated into ACR bit 0 — the *opposite* halves from the double shifts' ACR:IXR pair. On a 703 `MPY` "can never" set overflow, even for `X'8000'` squared; the 704/706 versions of the option flag exactly that case, and the 703's own 1968 *software* multiply does too, but 6-2 is unambiguous. A skip in front of a two-word instruction skips only its first word — that is how the hardware works, and guest code simply must not write it.

Deliberate divergences, all documented at their use sites: `HLT` exits the emulator on headless machines, where there is no RUN switch to resume from — with the panel it halts to the panel and RUN resumes, and likewise a bad opcode halts recoverably and the idle `JMP $` just keeps running until HALT; SINGLE STEP steps one instruction exactly like SINGLE COMMAND, because the real switch's sub-instruction phases aren't modelled; the external sense line reads false so SSE always skips, and without a panel the four sense switches read false too — with one, SS0–SS3 read the SENSE toggles for real; the high speed reader free-runs instead of taking its 300 frames a second, because nothing watches a tape go by and pacing it would make `ray703-panel-ptb --throttle 10` a tape load that never visibly finishes. The saved machine status word's bit layout is the one thing here not taken from the manual — the manual names its contents but never diagrams it, so the layout follows rustheon's (and it turned out to be figure 5-1's MS lamp layout, see `publish_panel`).

## Style

- Idiomatic Rust; `cargo clippy --all-targets` clean. The tree is not `rustfmt`-formatted wholesale — match the surrounding code rather than reformatting files.
- Files start with the modeline `// vim: ts=4:sw=4:expandtab:`, the MIT license header (copy it from any existing file; the text is in `LICENSE`), and a `//!` module doc explaining what the file is (and, where relevant, which C++ file it ports).
- Every non-obvious behaviour gets a comment saying *why*, especially anything preserved for oracle compatibility.
- Prefer minimal, localized diffs; avoid broad reformatting.

## Commits

Subject line convention is a lowercase `scope: description`, e.g. `z80: clean up the flag handling`, `kaypro: massive pile of changes`, `rust: add the z80 core and RC2014`, `docs: ...`. (Older history used `[scope] Description`; prefer the colon style.)
