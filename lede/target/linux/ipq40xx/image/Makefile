include $(TOPDIR)/rules.mk
include $(INCLUDE_DIR)/image.mk

define Device/Default
	PROFILES := Default
	KERNEL_DEPENDS = $$(wildcard $(DTS_DIR)/$$(DEVICE_DTS).dts)
	KERNEL_LOADADDR := 0x80208000
	DEVICE_DTS_DIR = ../dts
	DEVICE_DTS = $$(SOC)-$(lastword $(subst _, ,$(1)))
	DEVICE_DTS_CONFIG := config@1
	IMAGES := sysupgrade.bin
	IMAGE/sysupgrade.bin = sysupgrade-tar | append-metadata
	IMAGE/sysupgrade.bin/squashfs :=
	DEVICE_COMPAT_VERSION := 1.2
	DEVICE_COMPAT_MESSAGE := Network and wireless configuration must be reset for QSDK 10 SSDK/EDMA and mac80211 wireless. Upgrade without keeping settings.
endef

include $(SUBTARGET).mk

ifeq ($(SUBTARGET),generic)
include generic-tp-link.mk
endif

$(eval $(call BuildImage))
