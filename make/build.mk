# comment out or override if you want to see the full output of each command
NOECHO ?= @

buildrust:
	cd rust && cargo build --target aarch64-unknown-none

$(OUTBIN): $(OUTELF)
	@echo generating image: $@
	$(NOECHO)$(SIZE) $<
	$(NOCOPY)$(OBJCOPY) -O binary $< $@

ifeq ($(ENABLE_TRUSTZONE), 1)
$(OUTELF): $(ALLOBJS) $(LINKER_SCRIPT) $(OUTPUT_TZ_BIN)
	@echo linking $@
	$(NOECHO)$(LD) $(LDFLAGS) -T $(LINKER_SCRIPT) $(OUTPUT_TZ_BIN) $(ALLOBJS) $(LIBGCC) -Map=$(OUTELF).map -o $@
else
$(OUTELF): $(ALLOBJS) $(LINKER_SCRIPT) buildrust
	@echo linking $@
	$(NOECHO)$(LD) $(LDFLAGS) -T $(LINKER_SCRIPT) $(ALLOBJS) rust/target/aarch64-unknown-none/debug/librust.a $(LIBGCC) -Map=$(OUTELF).map -o $@
endif


$(OUTELF).sym: $(OUTELF)
	@echo generating symbols: $@
	$(NOECHO)$(OBJDUMP) -t $< | $(CPPFILT) > $@

$(OUTELF).lst: $(OUTELF)
	@echo generating listing: $@
	$(NOECHO)$(OBJDUMP) -Mreg-names-raw -d $< | $(CPPFILT) > $@

$(OUTELF).debug.lst: $(OUTELF)
	@echo generating listing: $@
	$(NOECHO)$(OBJDUMP) -Mreg-names-raw -S $< | $(CPPFILT) > $@

$(OUTELF).size: $(OUTELF)
	@echo generating size map: $@
	$(NOECHO)$(NM) -S --size-sort $< > $@

ifeq ($(ENABLE_TRUSTZONE), 1)
$(OUTPUT_TZ_BIN): $(INPUT_TZ_BIN)
	@echo generating TZ output from TZ input
	$(NOECHO)$(OBJCOPY) -I binary -B arm -O elf32-littlearm $(INPUT_TZ_BIN) $(OUTPUT_TZ_BIN)
endif

$(OUTELF_STRIP): $(OUTELF)
	@echo generating stripped elf: $@
	$(NOECHO)$(STRIP) -S $< -o $@

include arch/$(ARCH)/compile.mk

$(BUILDDIR)/%.dtb: %.dts
	@$(MKDIR)
	@echo compiling $<
	$(NOECHO)$(TOOLCHAIN_PREFIX)cpp -nostdinc -undef -x assembler-with-cpp \
		$(DT_INCLUDES) $< -MD -MT $@ -MF $@.d -o $@.dts
	$(NOECHO)dtc -O dtb -I dts --align 16 -o $@ $@.dts
