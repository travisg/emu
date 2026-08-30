# test

The tests that live outside the Rust crate. The bulk of the emulator's cover is
in-module `mod tests` under `src/`, and `tests/machine_boot.rs` is the one cargo
integration test; what is here is the end-to-end harnesses and the guest
programs they drive.

## Layout

    run_*.sh        the end-to-end harnesses -- one per machine and per feature
    makefile        builds the guest programs, and has a target per harness
    6809/           guest sources and test data for the 6809
    703/            everything Raytheon 703; see 703/README.md

The harnesses stay at this level deliberately: each finds the repo root as its
own directory's parent, so moving one into a subdirectory would break the paths
it uses to reach `target/` and `roms/`.

## Running them

Build the emulator first (`cargo build`); the harnesses run the debug binary
unless `EMU_BIN` says otherwise. Each writes a log beside itself and greps it
for the guest's own report of success.

    make -C test basic6809-test      # boots 6809 BASIC, runs 6809/lang_test.bas
    make -C test ray703-test         # the 703 demo: banner, echo, clean halt
    make -C test ray703-basic-test   # a scripted Tiny BASIC session
    make -C test ray703-disc-test    # the 74601 disc, over two interrupt levels
    make -C test ray703-boot-test    # the disc controller's LOAD button
    make -C test ray703-rex-test     # REX: preemptive scheduling, sleep, and its shell

    make -C test ray703-boot-disc    # a disc that boots, in disks/
    make -C test ray703-blank-disc   # a blank platter on unit 0

All but the first need nothing outside the repo, and CI runs them. The 6809 one
boots Microsoft BASIC, so it needs `roms/6809/BASIC.HEX` in place
(`tools/fetch-roms.py`) and runs only locally.

## 6809/

    memtest.asm     a bootable ROM: sizes and walks every memory bank, then
                    loops. Configures a 16550 UART at $8000, so it targets the
                    obc variant rather than the 6809 the registry builds today.
                    `make -C test` assembles it and flattens it to memtest.bin.
    t6809.asm       every 6809 instruction with the bytes it should assemble to,
                    in comments. This is ASxxxx's own test file (its as6809
                    distribution ships it), kept here as an encoding reference
                    for work on the decoder; `cwai` and `daa` are commented out.
    addressing.asm  every addressing mode the 6809 has, indexed and indirect
                    forms included. Assembles; it is not meant to run.
    lang_test.bas   the BASIC program run_basic6809_lang_test.sh types at
                    6809 BASIC. It prints BASIC LANGUAGE TEST PASS if every
                    case agrees.

Building any of these needs the ASxxxx toolchain (`as6809`, `aslink`) and
`objcopy`, none of which normal development requires. Output lands beside the
source and is gitignored.
