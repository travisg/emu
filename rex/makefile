# vim: ts=4:sw=4:expandtab:
#
# REX, the 703 executive, and what runs under it. Everything here is
# assembled by tools/asm703.py out of this directory plus the Tiny BASIC
# interpreter it shares with the standalone guest in test/703; the products
# land in build/, which is gitignored. The emulator's own guests and tests
# stay under test/ and do not depend on anything here.

ASM703    := ../tools/asm703.py
MKDISC703 := ../tools/mkdisc703.py
BCORE     := ../test/703/bcore.asm
BUILD     := build

.PHONY: all test clean

all: $(BUILD)/rex.bin

# REX with Tiny BASIC aboard as a task: a three-file deck -- the executive,
# the glue that makes the interpreter a task, and the interpreter itself.
$(BUILD)/rex.bin: rex.asm brex.asm $(BCORE) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rex.asm brex.asm $(BCORE) -o $@ -l $(BUILD)/rex.lst

# The scripted session. EMU_BIN in the environment names another emulator
# binary; the default is the debug build.
test: all
	./run_rex_test.sh

clean:
	rm -rf $(BUILD)
