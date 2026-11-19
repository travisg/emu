# vim: ts=4:sw=4:expandtab:
#
# REX, the 703 executive, and what runs under it. Everything here is
# assembled by tools/asm703.py out of this directory plus the Tiny BASIC
# interpreter it shares with the standalone guest in test/703; the products
# land in build/, which is gitignored. The emulator's own guests and tests
# stay under test/ and do not depend on anything here.

ASM703    := ../tools/asm703.py
# What the assembler warns of is a mistake here: the build stops on one.
ASM       := $(ASM703) --werror
RELOAD703 := ../tools/reload703.py
MKDISC703 := ../tools/mkdisc703.py
BCORE     := ../test/703/bcore.asm
BUILD     := build

.PHONY: all modules reloc-check bootdisc test clean

all: $(BUILD)/rex.bin $(BUILD)/boot.bin modules bootdisc

# REX itself, one deck: the kernel and the shell in pages 0 and 1, and
# the filesystem and the swapper at the top of core.
KERNEL := rexmac.asm rex.asm shell.asm rexfs.asm rexswap.asm rexname.asm

$(BUILD)/rex.bin: $(KERNEL) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(KERNEL) -o $@ -l $(BUILD)/rex.lst

# The kernel's vector, cells and node layout as EQUs, which is all a
# module knows of it: the names rex.asm's NTRY cards list, at the values
# the same deck gives them. A rule of its own, so that the file is made
# whenever it is missing or behind the kernel's sources, and a change to
# the kernel assembles every module again against the kernel it will run
# under.
API := $(BUILD)/rexapi.asm

$(API): $(KERNEL) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(KERNEL) --exports $@

# The boot sector the disc controller's LOAD button reads: one sector, 47
# words, which mkdisc703.py holds it to.
$(BUILD)/boot.bin: boot.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) boot.asm -o $@ -l $(BUILD)/boot.lst

# --- modules --------------------------------------------------------------
# What loads under REX: relocatable objects in the 1968 loader's record
# format, each a deck that begins with the kernel's rexapi.asm.
# The utilities, each a file in util/ behind rexlib.asm, a library: the
# assembler takes the routines a deck names and leaves the rest. And the test
# modules beside them.
UTILS := type copy del ren squish dump put dir
UTIL_OBJS := $(UTILS:%=$(BUILD)/%.obj)

modules: $(BUILD)/hello.obj $(BUILD)/basic.obj $(UTIL_OBJS) \
         $(BUILD)/dskt.obj $(BUILD)/fst.obj $(BUILD)/nap.obj $(BUILD)/hog.obj \
         $(BUILD)/blip.obj $(BUILD)/far.obj $(BUILD)/shell.obj

$(UTIL_OBJS): $(BUILD)/%.obj: util/%.asm $(API) rexmac.asm rexlib.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) rexmac.asm rexlib.asm $< --object $@ -l $(BUILD)/$*.lst

$(BUILD)/nap.obj: $(API) rexmac.asm rexlib.asm nap.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) rexmac.asm rexlib.asm nap.asm --object $@ -l $(BUILD)/nap.lst

$(BUILD)/blip.obj: $(API) rexmac.asm rexlib.asm blip.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) rexmac.asm rexlib.asm blip.asm --object $@ -l $(BUILD)/blip.lst

$(BUILD)/hog.obj: $(API) rexmac.asm rexlib.asm hog.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) rexmac.asm rexlib.asm hog.asm --object $@ -l $(BUILD)/hog.lst

# A module of two segments, which the loader gives a page each.
FAR := $(API) rexmac.asm rexlib.asm far.asm

$(BUILD)/far.obj: $(FAR) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(FAR) --object $@ -l $(BUILD)/far.lst

# The shell of every terminal but the console's: the console's own shell,
# assembled again as a module. It keeps byte addresses, so it loads low.
SHELLMOD := $(API) rexmac.asm shellmod.asm shell.asm

$(BUILD)/shell.obj: $(SHELLMOD) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(SHELLMOD) --object $@ -l $(BUILD)/shell.lst

$(BUILD)/hello.obj: $(API) hello.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) hello.asm --object $@ -l $(BUILD)/hello.lst

# The disc exerciser the scripted session runs three of at once.
$(BUILD)/dskt.obj: $(API) dskt.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) dskt.asm --object $@ -l $(BUILD)/dskt.lst

# The file calls' exerciser. rexlib.asm has the printer, for a module
# that would rather not write it again.
$(BUILD)/fst.obj: $(API) rexmac.asm rexlib.asm fst.asm $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) rexmac.asm rexlib.asm fst.asm --object $@ -l $(BUILD)/fst.lst

# Tiny BASIC as a module: the glue that pays the interpreter's wrapper
# debts with the executive's services, over the interpreter itself, which
# the standalone guest in test/703 shares.
$(BUILD)/basic.obj: $(API) brex.asm $(BCORE) $(ASM703)
	@mkdir -p $(BUILD)
	$(ASM) $(API) brex.asm $(BCORE) --object $@ -l $(BUILD)/basic.lst

# The relocation check: assemble a module absolute at a base, place its
# object at that base with the reference loader, and every word must
# agree. Hello's bases are one in each half of a word page, so a loader
# that took the byte page from >>11 instead of >>10 would be caught on
# the second, below word X'4000' and above it, where a byte address has
# its top bit set; BASIC fills its page to the last word,
# so its two are the only ones it has, the page's first word and second.
# Far has a base for each of its segments: neighbouring pages, and the
# second segment below the first with the fence between them. The shell
# is placed at a page's first word and further in.
HELLO_BASES := 0x1080 0x1C40 0x4080 0x5C40
BASIC_BASES := 0x1000 0x1001
FAR_BASES   := 0x1000,0x1800 0x4100,0x1040
SHELL_BASES := 0x1000 0x1022

reloc-check: $(BUILD)/hello.obj $(BUILD)/basic.obj $(BUILD)/far.obj $(BUILD)/shell.obj
	@for base in $(HELLO_BASES); do \
	    $(ASM) --org $$base $(API) hello.asm -m $(BUILD)/hello-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/hello.obj --base $$base --check $(BUILD)/hello-$$base.map || exit 1; \
	done
	@for base in $(BASIC_BASES); do \
	    $(ASM) --org $$base $(API) brex.asm $(BCORE) -m $(BUILD)/basic-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/basic.obj --base $$base --check $(BUILD)/basic-$$base.map || exit 1; \
	done
	@for base in $(FAR_BASES); do \
	    $(ASM) --org $$base $(FAR) -m $(BUILD)/far-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/far.obj --base $$base --check $(BUILD)/far-$$base.map || exit 1; \
	done
	@for base in $(SHELL_BASES); do \
	    $(ASM) --org $$base $(SHELLMOD) -m $(BUILD)/shell-$$base.map || exit 1; \
	    $(RELOAD703) $(BUILD)/shell.obj --base $$base --check $(BUILD)/shell-$$base.map || exit 1; \
	done

# The swap file: room for a dozen tasks the size of BASIC.
SWAP_SECTORS := 1024

# A platter that is the whole system: the boot sector, the kernel as
# REX.SY, a swap file and the modules -- every one but BASIC and the shell
# marked as loadable above word X'4000', which leaves the core below it to
# what has to be there. From the repo root,
#     ./target/debug/emu -s ray703-load -r rex/build/rex.img --fast-io
# presses LOAD on it. A build product, made afresh each time: nothing
# written to it outlives the next build, and the build writes nothing
# outside rex/build.
bootdisc: $(BUILD)/rex.bin $(BUILD)/boot.bin modules
	$(MKDISC703) $(BUILD)/rex.img --force --boot $(BUILD)/boot.bin --sys $(BUILD)/rex.bin \
	    --swap $(SWAP_SECTORS) \
	    --add BASIC $(BUILD)/basic.obj --add SHELL $(BUILD)/shell.obj \
	    --add-high HELLO $(BUILD)/hello.obj \
	    $(foreach u,$(UTILS),--add-high $(u) $(BUILD)/$(u).obj)

# The scripted session, on a platter it makes for itself in a scratch
# directory -- so it wants the kernel and the modules. EMU_BIN in the
# environment names another emulator binary; the default is the debug build.
test: $(BUILD)/rex.bin $(BUILD)/boot.bin modules
	./run_rex_test.py

clean:
	rm -rf $(BUILD)
