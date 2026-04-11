# AGENTS.md

## Purpose
This repository is a terminal-driven emulator for several vintage systems (6809, Altair 680, Kaypro/Z80), with a small interactive console loop.

## High-Level Layout
- `main.cpp`: CLI parsing, system selection, startup/shutdown flow.
- `console.cpp` / `console.h`: raw terminal mode + input queue; console loop exits on Ctrl-D.
- `system/`: concrete system implementations and system factory.
- `cpu/`: CPU cores (`6800`, `6809`, `z80`).
- `dev/`: emulated devices (memory, UARTs).
- `libihex/`: Git submodule that implements Intel HEX read/parse support used by ROM loading.
- `test/`: ROM/assembly artifacts, CPU test code, and helper scripts used for manual testing.

## Build
Prerequisites (as currently used by the makefile):
- `clang` / `clang++`
- `make`
- `objdump` (Linux) or `otool` (Darwin)

Build from repo root:
```bash
make
```

Outputs:
- `build-emu/emu` (emulator binary)
- `build-emu/emu.lst` (disassembly listing)

Clean:
```bash
make clean
make spotless
```

## Run
Show CLI help:
```bash
./build-emu/emu -h
```

Run default system (currently `6809`):
```bash
./build-emu/emu
```

Run a specific system and ROM:
```bash
./build-emu/emu -s 6809 -r test/BASIC.HEX
./build-emu/emu -s altair680 -r mits680b.bin
./build-emu/emu -s kaypro -r rom/kaypro/kayproii_u47.bin
```

Important console behavior:
- Press `Ctrl-D` to exit the interactive console loop cleanly.

## Test Strategy (Current State)
There is no modern automated unit/integration test harness wired into `make`.
Use manual verification:
1. Build with `make`.
2. Check help output (`./build-emu/emu -h`) for valid systems and default ROMs.
3. Boot at least one target system and verify expected console behavior.
4. Use `Ctrl-D` to ensure clean shutdown path (`console.Run()` exits, system thread stops).

If you add tests, keep them scriptable and runnable from repo root.

## Code Style and Formatting (Observed)
Follow existing file conventions unless explicitly requested otherwise:
- Top-of-file modeline commonly present: `// vim: ts=4:sw=4:expandtab:`
- Indentation: 4 spaces, no tabs in C/C++ sources.
- Braces: K&R style (`if (...) {` on same line).
- Includes: standard headers first, then project headers.
- Naming:
  - Types/classes: `PascalCase`
  - methods/functions: `PascalCase` for class methods in this codebase
  - members: `mCamelCase`
- Prefer minimal, localized changes; avoid broad reformatting.

## System Metadata Pattern
System metadata is exposed via static members and aggregated by the factory layer:
- Each concrete system provides `GetSystemInfo()`.
- `System::GetSupportedSystems()` aggregates those records.
- CLI/help should consume this metadata instead of hardcoding values in `main.cpp`.

## Commit Message Convention (Observed)
Recent history strongly suggests bracketed scopes:
- Format: `[scope] short description`
- Multi-scope format appears as: `[scope1][scope2] short description`

Examples from history:
- `[help] Generate help system list from system metadata`
- `[cpu][mc6800] Add support for Motorola 6800`
- `[build] squelch a warning about C99 designators`

Suggested practice:
- Keep subject line concise and imperative.
- Use one or more bracketed scopes that match touched areas (`help`, `cpu`, `z80`, `6809`, `build`, `console`, `misc`, etc.).

## Guidance for Future AI Agents
- Read this file first, then inspect touched subsystem files before editing.
- Preserve existing style and architecture.
- Prefer extending existing system/cpu abstractions over adding ad-hoc logic in `main.cpp`.
- Validate with `make` and at least one manual run.
- Mention manual verification steps in your final summary.
