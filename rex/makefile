# vim: ts=4:sw=4:expandtab:
#
# REX, the 703 executive, and what runs under it. Everything here is
# assembled by tools/asm703.py out of this directory plus the Tiny BASIC
# interpreter it shares with the standalone guest in test/703; the products
# land in build/, which is gitignored. The emulator's own guests and tests
# stay under test/ and do not depend on anything here.

ASM703    := ../tools/asm703.py
RELOAD703 := ../tools/reload703.py
APICHECK  := ../tools/apicheck703.py
MKDISC703 := ../tools/mkdisc703.py
BCORE     := ../test/703/bcore.asm
BUILD     := build

.PHONY: all modules reloc-check disc test clean

all: $(BUILD)/rex.bin modules disc

# REX itself: the kernel and the shell, pages 0 and 1. rexapi.asm states
# the kernel's vector, cells and node layout as EQUs for the modules; the
# check holds rex.asm's own labels to them.
$(BUILD)/rex.bin: rex.asm rexapi.asm $(ASM703) $(APICHECK)
	@mkdir -p $(BUILD)
	$(ASM703) rex.asm -o $@ -l $(BUILD)/rex.lst
	$(APICHECK) rexapi.asm $(BUILD)/rex.lst

# --- modules --------------------------------------------------------------
# What loads under REX: relocatable objects in the 1968 loader's record
# format, each a deck that begins with rexapi.asm.
modules: $(BUILD)/hello.obj $(BUILD)/basic.obj

$(BUILD)/hello.obj: rexapi.asm hello.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm hello.asm --object $@ -l $(BUILD)/hello.lst

# Tiny BASIC as a module: the glue that pays the interpreter's wrapper
# debts with the executive's services, over the interpreter itself, which
# the standalone guest in test/703 shares.
$(BUILD)/basic.obj: rexapi.asm brex.asm $(BCORE) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm brex.asm $(BCORE) --object $@ -l $(BUILD)/basic.lst

# The relocation check: assemble a module absolute at a base, place its
# object at that base with the reference loader, and every word must
# agree. Two bases to a module, one in each half of a word page, so a
# loader that took the byte page from >>11 instead of >>10 would be caught
# on the second; BASIC is too big for the upper half, so its second base
# is as high as it fits.
HELLO_BASES := 0x1080 0x1C40
BASIC_BASES := 0x1080 0x1840

reloc-check: $(BUILD)/hello.obj $(BUILD)/basic.obj
	@for base in $(HELLO_BASES); do \
	    $(ASM703) --org $$base rexapi.asm hello.asm -m $(BUILD)/hello-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/hello.obj --base $$base --check $(BUILD)/hello-$$base.map || exit 1; \
	done
	@for base in $(BASIC_BASES); do \
	    $(ASM703) --org $$base rexapi.asm brex.asm $(BCORE) -m $(BUILD)/basic-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/basic.obj --base $$base --check $(BUILD)/basic-$$base.map || exit 1; \
	done

# The platter on unit 0, with the modules on it: the emulator mounts
# disks/ray703-disc0.img from the repo root, so after this the run line
# in README.md has them. Made if absent; adding rewrites only the sectors
# the catalogue owns.
DISC := ../disks/ray703-disc0.img

disc: modules
	@mkdir -p ../disks
	$(MKDISC703) $(DISC) --add HELLO $(BUILD)/hello.obj --add BASIC $(BUILD)/basic.obj

# The scripted session. EMU_BIN in the environment names another emulator
# binary; the default is the debug build.
test: all modules
	./run_rex_test.sh

clean:
	rm -rf $(BUILD)
