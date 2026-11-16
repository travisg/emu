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

# REX itself, one deck: the kernel and the shell in pages 0 and 1, and
# the filesystem and the swapper at the top of core. rexapi.asm states the kernel's
# vector, cells and node layout as EQUs for the modules; the check holds
# the kernel's own labels to them.
KERNEL := rex.asm rexfs.asm rexswap.asm

$(BUILD)/rex.bin: $(KERNEL) rexapi.asm $(ASM703) $(APICHECK)
	@mkdir -p $(BUILD)
	$(ASM703) $(KERNEL) -o $@ -l $(BUILD)/rex.lst
	$(APICHECK) rexapi.asm $(BUILD)/rex.lst

# --- modules --------------------------------------------------------------
# What loads under REX: relocatable objects in the 1968 loader's record
# format, each a deck that begins with rexapi.asm.
# The utilities, each a file in util/ behind rexlib.asm, and the test
# modules beside them.
UTILS := type copy del ren squish dump put
UTIL_OBJS := $(UTILS:%=$(BUILD)/%.obj)

modules: $(BUILD)/hello.obj $(BUILD)/basic.obj $(UTIL_OBJS) \
         $(BUILD)/dskt.obj $(BUILD)/fst.obj $(BUILD)/nap.obj $(BUILD)/hog.obj

$(UTIL_OBJS): $(BUILD)/%.obj: util/%.asm rexapi.asm rexlib.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm rexlib.asm $< --object $@ -l $(BUILD)/$*.lst

$(BUILD)/nap.obj: rexapi.asm rexlib.asm nap.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm rexlib.asm nap.asm --object $@ -l $(BUILD)/nap.lst

$(BUILD)/hog.obj: rexapi.asm rexlib.asm hog.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm rexlib.asm hog.asm --object $@ -l $(BUILD)/hog.lst

$(BUILD)/hello.obj: rexapi.asm hello.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm hello.asm --object $@ -l $(BUILD)/hello.lst

# The disc exerciser the scripted session runs three of at once.
$(BUILD)/dskt.obj: rexapi.asm dskt.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm dskt.asm --object $@ -l $(BUILD)/dskt.lst

# The file calls' exerciser. rexlib.asm is the printer, for a module
# that would rather not write it again.
$(BUILD)/fst.obj: rexapi.asm rexlib.asm fst.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm rexlib.asm fst.asm --object $@ -l $(BUILD)/fst.lst

# Tiny BASIC as a module: the glue that pays the interpreter's wrapper
# debts with the executive's services, over the interpreter itself, which
# the standalone guest in test/703 shares.
$(BUILD)/basic.obj: rexapi.asm brex.asm $(BCORE) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM703) rexapi.asm brex.asm $(BCORE) --object $@ -l $(BUILD)/basic.lst

# The relocation check: assemble a module absolute at a base, place its
# object at that base with the reference loader, and every word must
# agree. Hello's bases are one in each half of a word page, so a loader
# that took the byte page from >>11 instead of >>10 would be caught on
# the second, below word X'4000' and above it, where a byte address has
# its top bit set; BASIC fills its page to the last word,
# so its two are the only ones it has, the page's first word and second.
HELLO_BASES := 0x1080 0x1C40 0x4080 0x5C40
BASIC_BASES := 0x1000 0x1001

reloc-check: $(BUILD)/hello.obj $(BUILD)/basic.obj
	@for base in $(HELLO_BASES); do \
	    $(ASM703) --org $$base rexapi.asm hello.asm -m $(BUILD)/hello-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/hello.obj --base $$base --check $(BUILD)/hello-$$base.map || exit 1; \
	done
	@for base in $(BASIC_BASES); do \
	    $(ASM703) --org $$base rexapi.asm brex.asm $(BCORE) -m $(BUILD)/basic-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/basic.obj --base $$base --check $(BUILD)/basic-$$base.map || exit 1; \
	done

# The platter on unit 0, with the modules on it -- every one but BASIC
# marked as loadable above word X'4000', which leaves the core below it
# to what has to be there, and with a swap file: the emulator mounts
# disks/ray703-disc0.img from the repo root, so after this the run line
# in README.md has them. Made if absent; each module replaces its
# namesake and nothing else on the platter moves.
DISC := ../disks/ray703-disc0.img
# The swap file, made once: room for a dozen tasks the size of BASIC.
SWAP_SECTORS := 1024

disc: modules
	@mkdir -p ../disks
	$(MKDISC703) $(DISC) --swap $(SWAP_SECTORS) \
	    --add BASIC $(BUILD)/basic.obj --add-high HELLO $(BUILD)/hello.obj \
	    $(foreach u,$(UTILS),--add-high $(u) $(BUILD)/$(u).obj)

# The scripted session, on a platter it makes for itself in a scratch
# directory -- so it wants the kernel and the modules, not `disc`, which
# writes the user's own platter. EMU_BIN in the environment names another
# emulator binary; the default is the debug build.
test: $(BUILD)/rex.bin modules
	./run_rex_test.py

clean:
	rm -rf $(BUILD)
