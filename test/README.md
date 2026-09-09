# test

The tests that live outside the Rust crate. The bulk of the emulator's cover is
in-module `mod tests` under `src/`, and `tests/machine_boot.rs` is the one cargo
integration test; what is here is the end-to-end harnesses and the guest
programs they drive.

## Layout

    run_*.sh        the end-to-end harnesses -- one per machine and per feature
    run_*.py        the same, driving the emulator over its debug port
    makefile        builds the guest programs, and has a target per harness
    6809/           guest sources and test data for the 6809
    703/            everything Raytheon 703; see 703/README.md

The harnesses stay at this level deliberately: each finds the repo root as its
own directory's parent, so moving one into a subdirectory would break the paths
it uses to reach `target/` and `roms/`.

## Running them

Build the emulator first (`cargo build`); the harnesses run the debug binary
unless `EMU_BIN` says otherwise. Each writes a log beside itself. The shell
ones drive the emulator through a pty and grep the log for the guest's own
report of success; the python one talks to the emulator's debug port
(`tools/emudbg.py`) instead, reads the guest's output from there and takes
the halt from the port's stop event, so it needs no pty and no `script(1)`.

    make -C test basic6809-test      # boots 6809 BASIC, runs 6809/lang_test.bas
    make -C test kaypro-zex-test     # the Z80 instruction exercisers under CP/M
    make -C test rc2014-basic-test   # factory BASIC, every keystroke an interrupt
    make -C test rc2014-ctc-test     # IM 2 and the CTC, a program written into RAM
    make -C test ray703-test         # the 703 demo: banner, echo, clean halt
    make -C test ray703-basic-test   # a scripted Tiny BASIC session
    make -C test ray703-disc-test    # the 74601 disc, over two interrupt levels
    make -C test ray703-boot-test    # the disc controller's LOAD button

    make -C test ray703-boot-disc    # a disc that boots, in disks/
    make -C test ray703-blank-disc   # a blank platter on unit 0

All but the first three need nothing outside the repo, and CI runs them.
REX's session is `make -C rex test`, under its own directory. The 6809 one
boots Microsoft BASIC, so it needs `roms/6809/BASIC.HEX` in place
(`tools/fetch-roms.py`) and runs only locally. The RC2014 one boots the
factory rom's BASIC, `roms/rc2014/24886009.BIN` from the same script, and
is local likewise; it is also the Z80 core's interrupt test, since every
character typed at that machine is an SIO interrupt. The CTC one writes its
program into RAM itself under a rom of zeros, so it is not local. The Kaypro one boots CP/M
off the stock floppy with the Kaypro ROMs, needs `cpmtools` to put the
exercisers (`disks/cpm/`, also from `tools/fetch-roms.py`) on a scratch copy
of it, and is local for the same reason; it is the Z80 core's reference,
see the Test section of AGENTS.md.

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
