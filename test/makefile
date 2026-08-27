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
	rm -f 703/demo.lst

basic6809-test:
	./run_basic6809_lang_test.sh

# --- Raytheon 703 ---------------------------------------------------------
# The 703's own assembler (SYM II) survives only as scans, so the demo image is
# built by asm703.py in this directory. The output goes straight to roms/703,
# which is where the registry's default_rom points; roms is a symlink to
# storage outside the repo, so nothing generated lands in git.
ASM703      := ./asm703.py
RAY703_ROMS := ../roms/703

.PHONY: ray703 ray703-demo ray703-tape ray703-test

ray703: ray703-demo ray703-tape

ray703-demo: $(RAY703_ROMS)/demo.bin

$(RAY703_ROMS)/demo.bin: 703/demo.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $@ -l 703/demo.lst

# tape.asm is punched rather than loaded: the .tape is what the ray703-ptb
# subsystem feeds through the paper tape reader. The .bin beside it is only a
# build by-product.
ray703-tape: $(RAY703_ROMS)/tape.tape

$(RAY703_ROMS)/tape.tape: 703/tape.asm $(ASM703)
	@mkdir -p $(RAY703_ROMS)
	$(ASM703) $< -o $(RAY703_ROMS)/tape.bin -t $@ -l 703/tape.lst

ray703-test: ray703-demo
	./run_ray703_demo_test.sh
