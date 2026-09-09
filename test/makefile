# vim: ts=4:sw=4:expandtab:

AS        := as6809
ASFLAGS   := -gloaxff
LINK      := aslink
LINKFLAGS := -ni

# ihex -> flat binary. The libihex submodule (and its ihextobin) went with the
# C++ tree; objcopy does the same job for a contiguous image.
OBJCOPY   := objcopy

.PHONY: all clean basic6809-test kaypro-zex-test rc2014-basic-test rc2014-cpm-test rc2014-ctc-test

# The 6809 sources, all built in place under 6809/. memtest is the only one
# meant to run; the other two exist to be assembled. README.md says which is
# which.
M6809 := 6809

all: $(M6809)/memtest.bin $(M6809)/t6809.ihx $(M6809)/addressing.ihx

# Assemble: <stem>.asm -> <stem>.rel (side-effects: .lst, .rst)
%.rel: %.asm
	$(AS) $(ASFLAGS) $*

# Link: <stem>.rel -> <stem>.ihx
%.ihx: %.rel
	$(LINK) $(LINKFLAGS) $*

# memtest needs the -u flag to update the listing with absolute addresses
$(M6809)/memtest.ihx: $(M6809)/memtest.rel
	$(LINK) -niu $(basename $@)

# Convert ihex to flat binary. objcopy starts the output at the lowest address
# in the image, which for memtest.asm is its 0xc000 origin.
$(M6809)/memtest.bin: $(M6809)/memtest.ihx
	$(OBJCOPY) -I ihex -O binary $< $@

clean:
	rm -f $(M6809)/*.rel $(M6809)/*.lst $(M6809)/*.rst $(M6809)/*.ihx $(M6809)/*.bin
	rm -f 703/demo.lst 703/basic.lst 703/disc.lst 703/boot.lst 703/tape.lst
	rm -f *.log

basic6809-test:
	./run_basic6809_lang_test.sh

# The Z80 exercisers on the Kaypro. Needs the Kaypro ROMs, the stock floppy
# and disks/cpm/zex*.com (all from tools/fetch-roms.py), cpmtools, and the
# release binary if it is to finish in minutes rather than an hour.
kaypro-zex-test:
	./run_kaypro_zex_test.py

# The RC2014 factory rom into BASIC over the debug port, with the SIO
# interrupt handler under a breakpoint. Needs roms/rc2014/24886009.BIN
# (tools/fetch-roms.py).
rc2014-basic-test:
	./run_rc2014_basic_test.py

# CP/M off the compact flash card, on a scratch copy of it. Needs the rom
# and disks/rc2014-cf.img (tools/fetch-roms.py).
rc2014-cpm-test:
	./run_rc2014_cpm_test.py

# IM 2 and the CTC: the harness writes the program into RAM under a rom of
# zeros it makes itself, so this needs nothing fetched.
rc2014-ctc-test:
	./run_rc2014_ctc_test.py

# --- Raytheon 703 ---------------------------------------------------------
# The 703's own assembler (SYM II) survives only as scans, so the demo image is
# built by tools/asm703.py. The output goes straight to roms/703, which is
# where the registry's default_rom points; the contents of roms are gitignored,
# so nothing generated lands in git.
ASM703       := ../tools/asm703.py
MKDISC703    := ../tools/mkdisc703.py
RAY703_ROMS  := ../roms/703
RAY703_DISKS := ../disks

.PHONY: ray703 ray703-demo ray703-tape ray703-basic ray703-disc ray703-boot ray703-test ray703-basic-test ray703-disc-test ray703-boot-test ray703-listings ray703-verify ray703-boot-disc ray703-blank-disc

ray703: ray703-demo ray703-tape ray703-basic ray703-disc ray703-boot ray703-listings

ray703-demo: $(RAY703_ROMS)/demo.bin

$(RAY703_ROMS)/demo.bin: 703/demo.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/demo.lst

# Exercises the 74601 disc: write, read back, verify, report on the teletype.
ray703-disc: $(RAY703_ROMS)/disc.bin

$(RAY703_ROMS)/disc.bin: 703/disc.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/disc.lst

# The boot sector for the disc controller's LOAD button. It must fit in the
# one sector the button reads -- 47 words, 94 bytes -- so the build enforces it.
ray703-boot: $(RAY703_ROMS)/boot.bin

$(RAY703_ROMS)/boot.bin: 703/boot.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/boot.lst
	@size=$$(wc -c < $@); if [ $$size -gt 94 ]; then \
		echo "error: boot.bin is $$size bytes; the LOAD button reads one 94-byte sector"; \
		rm -f $@; exit 1; fi

# Tiny BASIC, the largest guest in the tree and the reason the emulator has
# hardware multiply/divide.  Two files, one deck: basic.asm is the machine
# (driver, workspace, section layout) and bcore.asm the interpreter -- which
# rex/ also assembles, behind its own wrapper, into REX.
ray703-basic: $(RAY703_ROMS)/basic.bin

$(RAY703_ROMS)/basic.bin: 703/basic.asm 703/bcore.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) 703/basic.asm 703/bcore.asm -o $@ -l 703/basic.lst

# tape.asm is punched rather than loaded: the .tape is what the ray703-ptb
# subsystem feeds through the paper tape reader. The .bin beside it is only a
# build by-product.
ray703-tape: $(RAY703_ROMS)/tape.tape

$(RAY703_ROMS)/tape.tape: 703/tape.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $(RAY703_ROMS)/tape.bin -t $@ -l 703/tape.lst

# --- the transcribed period software -------------------------------------
# X-RAY EXEC and the relocating loader, from the hand transcriptions of their
# 1968 assembly listings in 703/listings. Every card that generated a word
# printed its own absolute address beside it, so a listing already *is* a core
# image and no assembler is on the path: xraylist.py --core writes one out.
XRAYLIST    := python3 ../tools/xraylist.py
LISTINGS    := 703/listings

ray703-listings: $(RAY703_ROMS)/xray.bin $(RAY703_ROMS)/loader.bin

$(RAY703_ROMS)/xray.bin: $(LISTINGS)/390779_XRAY_listing.txt ../tools/xraylist.py
	@mkdir -p $(RAY703_ROMS)
	$(XRAYLIST) $< --core $@

$(RAY703_ROMS)/loader.bin: $(LISTINGS)/390682C_RelocatingLoader_listing.txt ../tools/xraylist.py
	@mkdir -p $(RAY703_ROMS)
	$(XRAYLIST) $< --core $@

# Re-assemble a transcript and diff it against the object code the 1968
# assembler printed, word for word. This is the check the listing cannot do for
# itself: the source text and the object column are independent readings of the
# same card, and asm703.py saw neither. X-RAY does not assemble yet and says so.
ray703-verify:
	$(XRAYLIST) $(LISTINGS)/390682C_RelocatingLoader_listing.txt --verify
	-$(XRAYLIST) $(LISTINGS)/390779_XRAY_listing.txt --verify

# --- disc images ----------------------------------------------------------
# disks/ is gitignored like roms/, so the images a machine mounts are made
# here rather than checked in.

# A disc that boots: the boot sector in sector 0 of track 0, where the
# controller's LOAD button reads it. Derived from boot.bin, so it is rebuilt
# whenever that changes.
#
#   ./target/debug/emu -s ray703-load -r disks/ray703-boot.img
#
ray703-boot-disc: $(RAY703_DISKS)/ray703-boot.img

$(RAY703_DISKS)/ray703-boot.img: $(RAY703_ROMS)/boot.bin $(MKDISC703)
	@mkdir -p $(RAY703_DISKS)
	$(MKDISC703) $@ --boot $< --force

# A blank platter on unit 0, which is what the disc exerciser wants to write
# to. Never overwritten: once a guest has written to a disc it is data, so
# delete it by hand to start over.
ray703-blank-disc:
	@mkdir -p $(RAY703_DISKS)
	$(MKDISC703) $(RAY703_DISKS)/ray703-disc0.img

ray703-test: ray703-demo
	./run_ray703_demo_test.py

ray703-basic-test: ray703-basic
	./run_ray703_basic_test.sh

ray703-disc-test: ray703-disc
	./run_ray703_disc_test.sh

ray703-boot-test: ray703-boot
	./run_ray703_boot_test.sh
