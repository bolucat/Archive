# Makefile for lwip

PROJECT=lwip

CROSS_PREFIX :=
PP=$(CROSS_PREFIX)cpp
CC=$(CROSS_PREFIX)gcc
LD=$(CROSS_PREFIX)ld
AR=$(CROSS_PREFIX)ar
CCFLAGS=-O3 -pipe $(CFLAGS)
LDFLAGS=$(LFLAGS)

CCFLAGS+=$(shell $(CC) -Werror -fno-stack-clash-protection -E -x c /dev/null \
           >/dev/null 2>&1 && echo -fno-stack-clash-protection)

SRCDIR=src
BINDIR=bin
BUILDDIR=build

STATIC_TARGET=$(BINDIR)/lib$(PROJECT).a
SHARED_TARGET=$(BINDIR)/lib$(PROJECT).so

# Native Windows: clang targeting MSVC, built with the very same rules.
# Run make from a Visual Studio developer prompt (x64, x86 or arm64) with
# a POSIX sh.exe on PATH, e.g. Git for Windows:
#   make ARCH=x64
ifeq ($(OS),Windows_NT)
WIN_NATIVE=1
ARCH?=x64
CROSS_TARGET_x64=x86_64
CROSS_TARGET_x86=i686
CROSS_TARGET_arm64=aarch64
CC=clang --target=$(CROSS_TARGET_$(ARCH))-pc-windows-msvc
PP=$(CC)
AR=llvm-ar
STATIC_TARGET=$(BINDIR)/$(PROJECT).lib
SHARED_TARGET=$(BINDIR)/$(PROJECT).dll
SHARED_IMPLIB=$(BINDIR)/$(PROJECT).dll.lib
CCFLAGS+=-D_CRT_SECURE_NO_WARNINGS -D_CRT_NONSTDC_NO_WARNINGS \
         -D_WINSOCK_DEPRECATED_NO_WARNINGS -Wno-pragma-pack -Wno-empty-body
endif

ifneq ($(WIN_NATIVE),1)
$(SHARED_TARGET) : CCFLAGS+=-fPIC
$(SHARED_TARGET) : LDFLAGS+=-shared -pthread
else
$(SHARED_TARGET) : LDFLAGS+=-shared -Wl,/IMPLIB:$(SHARED_IMPLIB)
endif

-include build.mk
CCFLAGS+=-Isrc/include -Isrc/ports/include

CCSRCS=$(filter %.c,$(SRCFILES))
ASSRCS=$(filter %.S,$(SRCFILES))
LDOBJS=$(patsubst $(SRCDIR)/%.c,$(BUILDDIR)/%.o,$(CCSRCS)) \
       $(patsubst $(SRCDIR)/%.S,$(BUILDDIR)/%.o,$(ASSRCS))
DEPEND=$(LDOBJS:.o=.dep)

BUILDMSG="\e[1;31mBUILD\e[0m %s\n"
LINKMSG="\e[1;34mLINK\e[0m  \e[1;32m%s\e[0m\n"
CLEANMSG="\e[1;34mCLEAN\e[0m %s\n"
TESTMSG="\e[1;34mTEST\e[0m  \e[1;32m%s\e[0m\n"

ENABLE_DEBUG :=
ifeq ($(ENABLE_DEBUG),1)
	CCFLAGS+=-g -O0 -DENABLE_DEBUG
endif

V:=
ECHO_PREFIX:=@
ifeq ($(V),1)
    undefine ECHO_PREFIX
endif

.PHONY: static shared clean

static : $(STATIC_TARGET)

shared : $(SHARED_TARGET)

clean :
	$(ECHO_PREFIX) $(RM) -rf $(BINDIR) $(BUILDDIR)
	@printf $(CLEANMSG) $(PROJECT)

$(STATIC_TARGET) : $(LDOBJS)
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(AR) csq $@ $^
	@printf $(LINKMSG) $@

$(SHARED_TARGET) : $(LDOBJS)
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(CC) -o $@ $^ $(LDFLAGS)
	@printf $(LINKMSG) $@

$(BUILDDIR)/%.dep : $(SRCDIR)/%.c
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(PP) $(CCFLAGS) -MM -MT$(@:.dep=.o) -MF$@ $< 2>/dev/null

$(BUILDDIR)/%.dep : $(SRCDIR)/%.S
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(PP) $(CCFLAGS) -MM -MT$(@:.dep=.o) -MF$@ $< 2>/dev/null

$(BUILDDIR)/%.o : $(SRCDIR)/%.c
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(CC) $(CCFLAGS) -c -o $@ $<
	@printf $(BUILDMSG) $<

$(BUILDDIR)/%.o : $(SRCDIR)/%.S
	$(ECHO_PREFIX) mkdir -p $(dir $@)
	$(ECHO_PREFIX) $(CC) $(CCFLAGS) -c -o $@ $<
	@printf $(BUILDMSG) $<

ifneq ($(MAKECMDGOALS),clean)
-include $(DEPEND)
endif
