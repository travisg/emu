# vim: ts=4:sw=4:expandtab:

AS        := as6809
ASFLAGS   := -gloaxff
LINK      := aslink
LINKFLAGS := -ni

IHEXTOBIN := ../libihex/ihextobin

.PHONY: all clean

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

# Build ihextobin if it does not exist yet
$(IHEXTOBIN):
	$(MAKE) -C ../libihex

# Convert ihex to flat binary at ROM origin 0xc000
rom.bin: rom.ihx $(IHEXTOBIN)
	$(IHEXTOBIN) -o 0xc000 $< $@

# Generate C array header from the flat binary
rom.h: rom.bin
	@echo "prog_uchar rom[] PROGMEM = {" > $@
	hexdump -v -e '16/1 "0x%02x, "' -e '"\n"' $< >> $@
	@echo "};" >> $@

clean:
	rm -f t6809.rel t6809.lst t6809.rst t6809.ihx
	rm -f test.rel test.lst test.rst test.ihx
	rm -f rom.rel rom.lst rom.rst rom.ihx rom.bin rom.h
