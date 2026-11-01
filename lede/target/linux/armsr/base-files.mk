GRUB_SERIAL:=$(call qstrip,$(CONFIG_TARGET_SERIAL))
ifeq ($(GRUB_SERIAL),)
$(error This platform requires CONFIG_TARGET_SERIAL be set!)
endif

define Package/base-files/install-target
	$(SED) '$(if $(filter ttyAMA0 tty1 hvc0 ttymxc0 ttymxc1 ttymxc2 ttySC0 ttyMSM0,$(GRUB_SERIAL)),/@GRUB_SERIAL@/d,s#@GRUB_SERIAL@#$(GRUB_SERIAL)#)' $(1)/etc/inittab
endef
