# Phase 3 handoff: Z80 core + RC2014

> **Status: complete (`81c3ea8`).** Kept as a record of what the pre-flight survey got right and
> wrong; the durable summary now lives in `rust-conversion-plan.md`. Two of the predictions below
> turned out to be backwards, and they're worth reading before writing the Phase 4 handoff:
>
> 1. **"Several flag expressions differ subtly between the `r` and `n` forms — don't factor early"
>    was wrong.** All eight ALU operations are identical in both forms; they were compared expression
>    by expression before factoring. One shared `alu()` was safe from the start.
> 2. **The real hazard was the reverse of the one predicted.** Duplication wasn't the risk —
>    *over*-generalizing was. A decode table invites you to write what a real Z80 does, and the ED
>    page here is deliberately incomplete (no `LDI`/`LDD`/`LDDR`, `NEG` only at `0x44`, no `RETN`).
>    Every hole had to stay a hole. The opcode sweep catches a filled one as a trace-*length*
>    difference, which was confirmed by mutation.
> 3. **Everything else in this document held up**, in particular the prefix-consume analysis, the
>    `LD r, (IX+d)` sign bug, the CB-shift fallthrough, and every test-harness gotcha. The one gap:
>    the `(IX+d)` sign bug is invisible unless the signed and unsigned target addresses hold
>    *different* bytes — the test preamble seeds markers at `$90f0` and `$8ff0` for exactly that.
>
> Also found, and **not** fixed: the RC2014 monitor can never transmit. Port `$80`'s status byte
> never sets bit 2 ("transmit buffer empty"), so the ROM spins at `$0116` forever. The C++ behaves
> identically — this is a pre-existing defect in the C++ tree, not a porting error, and fixing it
> means changing both trees together or the trace oracle stops agreeing.

## Where things stand

Phases 0–2 of the Rust port are done and validated. Phase 3 is **not started** — one attempt was made
and abandoned (see "Why the first attempt failed" below); nothing was committed and the tree is green.

| Commit | What |
|---|---|
| `753dd4b` | C++ fix: RC2014 SIO receive-byte race (mutex) |
| `5839ef9` | C++ `--trace` golden oracle, all three cores |
| `4cbd94d` | Rust crate scaffold + 6800 core |
| `7f5b346` | Rust MC6850, Altair680, console frontend, main |
| `82cad17` | Rust 6800 trace-diff coverage hardening |
| `905b0f9` | Rust 6809 core + System09 |
| `81c3ea8` | Rust Z80 core + RC2014 (this phase) |

Verified today: 6800 and 6809 both byte-identical to the C++ oracle on real-ROM boots, on every opcode
value, and on targeted snippets; the Rust 6809 passes the full BASIC language regression. 58 tests,
clippy clean.

## Goal of Phase 3

Port `cpu/cpuz80.cpp` → `src/cpu/z80.rs` and `system/system_rc2014.cpp` → `src/system/rc2014.rs`, then
add a `tests/trace_diff_z80.rs` mirroring the existing suites. Done means:

1. `cargo test` green, clippy clean.
2. RC2014 boot trace-diff byte-identical against the C++ oracle over ≥50k instructions.
3. Every opcode value agrees (base page, plus the ED and CB pages, plus DD/FD combinations).
4. `./target/debug/emu -s rc2014` boots and responds to typed input the way the C++ build does.

## Why the first attempt failed — read this before starting

I tried to write the whole core in one pass and produced a file that was incomplete and syntactically
broken. Deleted it rather than commit it.

The misjudgment: the Z80 is **not** like the 6800/6809. Those are table-driven — most of the work is
transcribing a decode table, and the semantics collapse into ~35 shared operation handlers. **The Z80
has no decode table at all.** It is one deep nested match on raw opcodes with hand-written flag
expressions per opcode, and several of those differ subtly between the `r` and `n` forms of the same
operation. Budget it as roughly the 6800 and 6809 combined, and **do not try to factor the flag
handling early** — port opcodes one at a time, let duplication stand, and only factor once the
trace-diff is green.

**Recommended approach: build it in verified slices**, running the trace-diff after each so divergence
is always caught against a known-good prefix instead of at the very end:

1. Register file, helpers (`read_r`/`write_r`/`read_nn`/push/pop/`test_cond`/`calc_parity`), the
   prefix fetch loop, and `trace_line`. Get it compiling with `step()` returning `BadOpcode`.
2. Base-page opcodes — enough to get RC2014 partway through boot. Trace-diff; the first divergence
   tells you the next opcode to implement.
3. ED page.
4. CB page.
5. DD/FD combinations.
6. RC2014 system + registry entry + the full gates.

## Z80 findings (from reading `cpu/cpuz80.cpp` in full)

Line numbers are against the current tree.

### Structure

- **Prefix loop.** The C++ uses `restart:` (line 401) / `decode:` (404) with three `goto`s. This maps
  cleanly onto a `loop` at the top of `step()`: read a byte; if it's `0xdd`/`0xfd`, set the flag and
  `continue`; otherwise break out with the opcode. Confirmed — prefix resolution already happens
  within one call in the C++, no recursion. One `step()` remains exactly one instruction, so the
  cycle-limit and trace semantics carry over unchanged.
- **`prefix_dd`/`prefix_fd` must be struct fields, not locals**, because `read_r_reg`/`write_r_reg`
  (103, 150) consult them to remap `H`/`L` onto the halves of IX/IY. Reset both at the top of every
  `step()`, as the C++ does at the top of its loop (379–380).
- **End-of-instruction prefix check** (1944–1953): if a DD/FD prefix was set but the instruction
  didn't "consume" it, the C++ prints and ends the run. Reproduce — several encodings hit this
  deliberately (see below). Track it with a local `consumed` bool.
- **Reset** (1982) just zeroes the register file. Unlike the 6800/6809 there is **no reset vector
  read** — PC starts at 0.

### Bugs and quirks to preserve

- **`LD r, (IX+d)` adds `d` unsigned** (1196: `read_ix() + read_n()`), while every other indexed form
  casts to `int8_t` first (e.g. 1212, 1254, 1505, 1570). Reproduce exactly.
- **Most CB-prefixed shifts ignore an active DD/FD prefix.** Only `RLC` (812), `BIT` (956), `RES`
  (977) and `SET` (1002) honour it. `RRC`/`RL`/`RR`/`SLA`/`SRA`/`SLL`/`SRL` (858–955) fall through to
  the plain `(HL)` form and then fail the end-of-instruction prefix check, ending the run. Preserve
  the wasted `d` read at 804–807 and the `(HL)` access so traces match right up to the failure point.
- **`HALT` is treated as `NOP`** (1191).
- **`RETI` is just a `RET`** (791) — no interrupt-controller notification.
- **`OUT (C), 0`** — the `0x71` encoding writes zero (456–464), undocumented but implemented.
- **Undocumented `RLC`/`RES`/`SET` writeback**: under DD/FD these also write the result into the
  encoded register when `r != 0b110` (820, 985, 1010).
- **Interrupt path is dead code.** `RaiseIRQ` is never called (`system_rc2014.cpp:128` is a TODO), NMI
  is never read, and IM 0 / IM 2 both fall back to `rst 0x38` (387–398). Build the shape, don't claim
  it as trace-validated.

### Flag details worth copying carefully

- `set_flags` (345) is the logical-op set: S, Z, parity, and clears H/N/C. `AND` then re-sets H
  afterwards (1684, 1692).
- `calc_parity` (319) returns true for an **even** number of set bits.
- `SBC HL,ss` / `ADC HL,ss` (466–503) compute H and PV with 16-bit-specific expressions — copy
  verbatim.
- `DAA` (1876) reads the *old* A for its flag decisions while mutating `mRegs.a` — order matters.
- `CCF` (1912) sets H from the *old* carry before inverting it.
- Block ops (`LDIR`, `CPIR`, `INIR`, `OTIR`, …) repeat by doing `pc -= 2`; the repeating variants
  force `Z` differently from the single-shot ones (e.g. 513–525 vs 504–512).

### Trace format

Already implemented on the C++ side (`CpuZ80::TraceInstruction`, added in `5839ef9`). The Rust
`trace_line` must match byte for byte:

```
PC=%04x AF=%02x%02x BC=%02x%02x DE=%02x%02x HL=%02x%02x IX=%04x IY=%04x SP=%04x
```

fed from `a, f, b, c, d, e, h, l, ix, iy, sp`.

## RC2014 system

Small — the easy half of the phase. From `system/system_rc2014.cpp`:

- Default ROM `roms/rc2014/24886009.BIN`, read as a flat 64K binary (the `#include "ihex.h"` in that
  file is vestigial; RC2014 does **not** use Intel HEX).
- 64K RAM bank and a 64K ROM bank.
- Decode (`GetDeviceAtAddr`, post-fix):
  - `0x0000–0x1fff` → ROM at offset `mRomBankSel * 0x2000`
  - `0x2000–0x7fff` → unmapped (reads 0, writes dropped)
  - `0x8000–0xffff` → RAM at offset **0** (the top half of the 64K buffer; this is the `1e7005d` fix,
    don't reintroduce the old `0x8000`)
- IO ports: `0x80` SIO/A control, `0x81` SIO/A data, `0x82`/`0x83` SIO/B, `0x90`/`0x91` second serial,
  `0x10–0x17` CF controller (accepted and ignored). Unknown ports print to stderr.
- `0x80` read returns bit 0 (rx available) and bit 1 (interrupt condition); `0x81` read returns the
  byte and clears the flag; `0x81` write goes to the console.
- **Nothing writes `mRomBankSel`** — there is no IO case for it, so it stays 0. Worth a comment in the
  Rust port so it doesn't read as an oversight.
- RC2014 hand-rolls a **single-byte** SIO inline (`mSIORecvByte`/`mSIORecvByte_valid`); it does not use
  `dev/z80sio.*`, which is Kaypro-only despite the generic name. In Rust this becomes an
  `Option<u8>` fed from the `mpsc` channel — no mutex needed, unlike the C++ (`753dd4b`).

## Test-harness gotchas (already paid for — don't rediscover)

- **Child stdin must stay open for the child's whole life.** Both `Child::wait()` and
  `wait_with_output()` close it, which EOFs the console, shuts the CPU thread down early, and yields a
  truncated trace that still looks like a successful run. Take the handle out first
  (`let s = child.stdin.take();`) so `wait()` has nothing to close, then drop it after. Same trap as
  `< /dev/null`. For shell testing use `mkfifo` + `exec 3<> fifo` and redirect `<&3`.
- **Per-test temp paths.** `cargo test` runs tests concurrently in one process; shared filenames race.
- **Assert both trace lengths before comparing.** `zip` stops at the shorter side, so an empty C++
  trace makes a naive comparison pass vacuously.
- **`-l N` yields N−1 instructions** on both sides (decrement-then-test). Ask for `N+1`.
- **Mutation-test the opcode sweep** once it's green — flip one case and confirm it fails. That's what
  proved the 6800 sweep had teeth.
- Copy the shape from `tests/trace_diff_6809.rs`; it's the closer template (its `.hex` writer isn't
  needed here since RC2014 loads flat binaries — use the `tests/trace_diff_6800.rs` ROM writer
  instead).

## Commands

```bash
make -j$(nproc)                      # C++ oracle (needed by the trace-diff tests)
cargo build && cargo test            # Rust
cargo clippy --all-targets

# manual trace-diff, stdin held open so the cycle limit is what stops it
mkfifo /storage/scratch/f; exec 3<> /storage/scratch/f
./target/debug/emu -s rc2014 -l 50000 --trace /storage/scratch/r.log <&3 >/dev/null 2>&1
./build-emu/emu   -s rc2014 -l 50000 --trace /storage/scratch/c.log <&3 >/dev/null 2>&1
exec 3>&-; cmp /storage/scratch/r.log /storage/scratch/c.log

# regressions that must stay green
./test/run_basic6809_lang_test.sh
EMU_BIN=./target/debug/emu ./test/run_basic6809_lang_test.sh
```

## After Phase 3

- **Phase 4** — Kaypro: SDL2 frontend, WD1793 (read-only; the C++ has no Write Sector at all), Z80Sio,
  128-byte video row stride, and port `0x1c` which is simultaneously a ROM/RAM bank switch, a floppy
  drive-select latch (bits 0–1, active low) and a status readback register (WD1793 INTRQ/DRQ in bits
  6–7).
- **Phase 5** — delete the C++ tree and the `libihex` submodule, update `AGENTS.md`/`README.md` for
  Cargo. Note the trace oracle dies with the C++ tree, so don't do this until everything else is
  validated.
- Also still open: `6809-obc` needs `uart16550` and currently returns an explicit error.
