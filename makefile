
TARGET := emu
BUILDDIR := build-$(TARGET)

# compiler flags, default libs to link against
COMPILEFLAGS := -g -O2 -Wall -W -Ilibihex -I.
CFLAGS :=
CXXFLAGS := -std=c++11 -Wno-c99-designator
ASMFLAGS :=
LDFLAGS :=
LDLIBS := libihex/libihex.a

UNAME := $(shell uname -s)
ARCH := $(shell uname -m)

# switch any platform specific stuff here
# ifeq ($(findstring CYGWIN,$(UNAME)),CYGWIN)
# ...
# endif
ifeq ($(UNAME),Darwin)
CC := clang
CPLUSPLUS := clang++
COMPILEFLAGS += -I/opt/local/include
LDFLAGS += -L/opt/local/lib
LDLIBS +=
OTOOL := otool
endif
ifeq ($(UNAME),Linux)
CC := clang
CPLUSPLUS := clang++
OBJDUMP := objdump
LDLIBS += -lpthread
endif
NOECHO ?= @

CFLAGS += $(COMPILEFLAGS)
CXXFLAGS += $(COMPILEFLAGS)
ASMFLAGS += $(COMPILEFLAGS)

OBJS := \
	main.o \
	console.o \
\
	cpu/cpu.o \
	dev/memory.o \
	system/system.o

OBJS += \
	cpu/cpu6800.o \
	cpu/cpu6809.o \
	cpu/cpuz80.o \
	dev/mc6850.o \
	dev/uart16550.o \
	system/altair680.o \
	system/system09.o \
	system/system_kaypro.o

OBJS := $(addprefix $(BUILDDIR)/,$(OBJS))

DEPS := $(OBJS:.o=.d)

.PHONY: all
all: $(BUILDDIR)/$(TARGET) $(BUILDDIR)/$(TARGET).lst

$(BUILDDIR)/$(TARGET): $(OBJS) libihex/libihex.a
	$(CPLUSPLUS) $(LDFLAGS) $(OBJS) -o $@ $(LDLIBS)

$(BUILDDIR)/$(TARGET).lst: $(BUILDDIR)/$(TARGET)
ifeq ($(UNAME),Darwin)
	$(OTOOL) -Vt $< | c++filt > $@
else
	$(OBJDUMP) -Cd $< > $@
endif

libihex/libihex.a:
	$(MAKE) -C libihex

clean:
	rm -f $(OBJS) $(DEPS) $(TARGET)
	$(MAKE) -C libihex clean

spotless:
	rm -rf build-*

# makes sure the target dir exists
MKDIR = if [ ! -d $(dir $@) ]; then mkdir -p $(dir $@); fi

$(BUILDDIR)/%.o: %.c
	@$(MKDIR)
	@echo compiling $<
	$(NOECHO)$(CC) $(CFLAGS) -c $< -MD -MT $@ -MF $(@:%o=%d) -o $@

$(BUILDDIR)/%.o: %.cpp
	@$(MKDIR)
	@echo compiling $<
	$(NOECHO)$(CPLUSPLUS) $(CXXFLAGS) -c $< -MD -MT $@ -MF $(@:%o=%d) -o $@

$(BUILDDIR)/%.o: %.S
	@$(MKDIR)
	@echo compiling $<
	$(NOECHO)$(CC) $(ASMFLAGS) -c $< -MD -MT $@ -MF $(@:%o=%d) -o $@

ifeq ($(filter $(MAKECMDGOALS), clean), )
-include $(DEPS)
endif
