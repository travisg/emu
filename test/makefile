# vim: ts=4:sw=4:expandtab:

AS        := as6809
ASFLAGS   := -gloaxff
LINK      := aslink
LINKFLAGS := -ni

# ihex -> flat binary. The libihex submodule (and its ihextobin) went with the
# C++ tree; objcopy does the same job for a contiguous image.
OBJCOPY   := objcopy

.PHONY: all clean basic6809-test

all: t6809.ihx test.ihx rom.h

# Assemble: <stem>.asm -> <stem>.rel (side-effects: .lst, .rst)
%.rel: %.asm
	$(AS) $(ASFLAGS) $*

# Link: <stem>.rel -> <stem>.ihx
%.ihx: %.rel
	$(LINK) $(LINKFLAGS) $*

# rom needs the -u flag to update the listing with absolute addresses
rom.ihx: rom.rel
	$(LINK) -niu $(basename $@)

# Convert ihex to flat binary. objcopy starts the output at the lowest address
# in the image, which for rom.asm is its 0xc000 origin.
rom.bin: rom.ihx
	$(OBJCOPY) -I ihex -O binary $< $@

# Generate C array header from the flat binary
rom.h: rom.bin
	@echo "prog_uchar rom[] PROGMEM = {" > $@
	hexdump -v -e '16/1 "0x%02x, "' -e '"\n"' $< >> $@
	@echo "};" >> $@

clean:
	rm -f t6809.rel t6809.lst t6809.rst t6809.ihx
	rm -f test.rel test.lst test.rst test.ihx
	rm -f rom.rel rom.lst rom.rst rom.ihx rom.bin rom.h
	rm -f 703/demo.lst 703/basic.lst

basic6809-test:
	./run_basic6809_lang_test.sh

# --- Raytheon 703 ---------------------------------------------------------
# The 703's own assembler (SYM II) survives only as scans, so the demo image is
# built by asm703.py in this directory. The output goes straight to roms/703,
# which is where the registry's default_rom points; roms is a symlink to
# storage outside the repo, so nothing generated lands in git.
ASM703      := ./asm703.py
RAY703_ROMS := ../roms/703

.PHONY: ray703 ray703-demo ray703-tape ray703-basic ray703-test ray703-basic-test ray703-listings ray703-verify

ray703: ray703-demo ray703-tape ray703-basic ray703-listings

ray703-demo: $(RAY703_ROMS)/demo.bin

$(RAY703_ROMS)/demo.bin: 703/demo.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/demo.lst

# Tiny BASIC, the largest guest in the tree and the reason the emulator has
# hardware multiply/divide.
ray703-basic: $(RAY703_ROMS)/basic.bin

$(RAY703_ROMS)/basic.bin: 703/basic.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/basic.lst

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
XRAYLIST    := python3 703/xraylist.py
LISTINGS    := 703/listings

ray703-listings: $(RAY703_ROMS)/xray.bin $(RAY703_ROMS)/loader.bin

$(RAY703_ROMS)/xray.bin: $(LISTINGS)/390779_XRAY_listing.txt 703/xraylist.py
	@mkdir -p $(RAY703_ROMS)
	$(XRAYLIST) $< --core $@

$(RAY703_ROMS)/loader.bin: $(LISTINGS)/390682C_RelocatingLoader_listing.txt 703/xraylist.py
	@mkdir -p $(RAY703_ROMS)
	$(XRAYLIST) $< --core $@

# Re-assemble a transcript and diff it against the object code the 1968
# assembler printed, word for word. This is the check the listing cannot do for
# itself: the source text and the object column are independent readings of the
# same card, and asm703.py saw neither. X-RAY does not assemble yet and says so.
ray703-verify:
	$(XRAYLIST) $(LISTINGS)/390682C_RelocatingLoader_listing.txt --verify
	-$(XRAYLIST) $(LISTINGS)/390779_XRAY_listing.txt --verify

ray703-test: ray703-demo
	./run_ray703_demo_test.sh

ray703-basic-test: ray703-basic
	./run_ray703_basic_test.sh
