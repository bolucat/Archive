# SPDX-License-Identifier: GPL-2.0-only
#
# Copyright (C) 2016-2017 Yousong Zhou <yszhou4tech@gmail.com>

include $(TOPDIR)/rules.mk
include $(INCLUDE_DIR)/image.mk

UNIFIED_ARMV8:=$(and $(CONFIG_TARGET_armsr_armv8),$(CONFIG_LINUX_6_18))

GRUB2_VARIANT =
GRUB_TERMINALS =
GRUB_SERIAL_CONFIG =
GRUB_TERMINAL_CONFIG =
GRUB_CONSOLE_CMDLINE = earlycon

ifneq ($(CONFIG_GRUB_CONSOLE),)
  GRUB_CONSOLE_CMDLINE += $(if $(UNIFIED_ARMV8),console=tty0)
  GRUB_TERMINALS += console
endif

GRUB_SERIAL:=$(call qstrip,$(CONFIG_TARGET_SERIAL))

ifneq ($(GRUB_SERIAL),)
  GRUB_SERIAL_CONFIG := serial --unit=0 --speed=$(CONFIG_GRUB_BAUDRATE) --word=8 --parity=no --stop=1 --rtscts=$(if $(CONFIG_GRUB_FLOWCONTROL),on,off)
  ifneq ($(UNIFIED_ARMV8),)
    GRUB_CONSOLE_CMDLINE += console=$(GRUB_SERIAL),$(CONFIG_GRUB_BAUDRATE)n8$(if $(CONFIG_GRUB_FLOWCONTROL),r,)
  endif
  GRUB_TERMINALS += serial
endif

ifneq ($(GRUB_TERMINALS),)
  GRUB_TERMINAL_CONFIG := terminal_input $(GRUB_TERMINALS); terminal_output $(GRUB_TERMINALS)
endif

ROOTPART:=$(call qstrip,$(CONFIG_TARGET_ROOTFS_PARTNAME))
ROOTPART:=$(if $(ROOTPART),$(ROOTPART),PARTUUID=$(IMG_PART_SIGNATURE)-02)
GPT_ROOTPART:=$(call qstrip,$(CONFIG_TARGET_ROOTFS_PARTNAME))
GPT_ROOTPART:=$(if $(GPT_ROOTPART),$(GPT_ROOTPART),PARTUUID=$(shell echo $(IMG_PART_DISKGUID) | sed 's/00$$/02/'))

GRUB_TIMEOUT:=$(call qstrip,$(CONFIG_GRUB_TIMEOUT))
GRUB_TITLE:=$(call qstrip,$(CONFIG_GRUB_TITLE))

BOOTOPTS:=$(call qstrip,$(CONFIG_GRUB_BOOTOPTS))

# ARM64 EFI exposes the board UART through its console protocol. The GRUB
# legacy serial terminal probes PC COM ports and cannot drive these UARTs.
ifneq ($(UNIFIED_ARMV8),)
  GRUB_SERIAL_CONFIG :=
  GRUB_TERMINAL_CONFIG := terminal_input console; terminal_output console
endif

Q6A_EXTRA_CMDLINE:=earlycon=qcom_geni,0x00994000 ignore_loglevel loglevel=8 keep_bootcon clk_ignore_unused pd_ignore_unused arm64.nopauth efi=noruntime
Q8B_EXTRA_CMDLINE:=earlycon=qcom_geni,0x00884000 ignore_loglevel loglevel=8 keep_bootcon clk_ignore_unused pd_ignore_unused arm64.nopauth efi=noruntime
# Qualcomm uses ttyMSM0; do not pass the generic UART/earlycon to board entries.
BOARD_CONSOLE_CMDLINE:=$(if $(CONFIG_GRUB_CONSOLE),console=tty0)
GRUB_CONFIG:=$(if $(UNIFIED_ARMV8),grub-efi-armv8.cfg,grub-efi.cfg)

define Build/combined
  $(INSTALL_DIR) $@.boot/
	$(CP) $(KDIR)/$(KERNEL_NAME) $@.boot/efi/openwrt/
	$(if $(UNIFIED_ARMV8),
		$(CP) $(KDIR)/image-qcs6490-radxa-dragon-q6a.dtb $@.boot/efi/openwrt/q6a.dtb
		$(CP) $(KDIR)/image-sc8280xp-radxa-dragon-q8b.dtb $@.boot/efi/openwrt/q8b.dtb
	)
	-$(CP) $(STAGING_DIR_ROOT)/boot/. $@.boot/boot/
	$(if $(filter $(1),efi),
		$(INSTALL_DIR) $@.boot/efi/boot
		$(CP) $(STAGING_DIR_IMAGE)/grub2/boot$(if $(CONFIG_aarch64),aa64,arm).efi $@.boot/efi/openwrt/
		$(CP) $(STAGING_DIR_IMAGE)/grub2/boot$(if $(CONFIG_aarch64),aa64,arm).efi $@.boot/efi/boot/
	)
	KERNELPARTTYPE=ef FAT_TYPE="$(if $(UNIFIED_ARMV8),32)" PURE_GPT="$(if $(UNIFIED_ARMV8),1)" PADDING="1" SIGNATURE="$(IMG_PART_SIGNATURE)" \
		$(if $(filter $(1),efi),GUID="$(IMG_PART_DISKGUID)") $(SCRIPT_DIR)/gen_image_generic.sh \
		$@ \
		$(CONFIG_TARGET_KERNEL_PARTSIZE) $@.boot \
		$(CONFIG_TARGET_ROOTFS_PARTSIZE) $(IMAGE_ROOTFS) \
		256
endef

HOST_SFDISK := $(STAGING_DIR_HOST)/bin/sfdisk

define Build/combined-ufs
	$(INSTALL_DIR) $@.boot/efi/openwrt
	$(CP) $(KDIR)/$(KERNEL_NAME) $@.boot/efi/openwrt/
	$(CP) $(KDIR)/image-qcs6490-radxa-dragon-q6a.dtb $@.boot/efi/openwrt/q6a.dtb
	$(CP) $(KDIR)/image-sc8280xp-radxa-dragon-q8b.dtb $@.boot/efi/openwrt/q8b.dtb
	$(INSTALL_DIR) $@.boot/efi/boot
	$(CP) $(STAGING_DIR_IMAGE)/grub2/boot$(if $(CONFIG_aarch64),aa64,arm).efi $@.boot/efi/openwrt/
	$(CP) $(STAGING_DIR_IMAGE)/grub2/boot$(if $(CONFIG_aarch64),aa64,arm).efi $@.boot/efi/boot/
	set -e; \
	SFDISK_BIN="$(HOST_SFDISK)"; \
	SECTOR_SIZE=4096; \
	SECTORS_PER_MIB=$$((1024 * 1024 / SECTOR_SIZE)); \
	ESP_START=256; \
	ESP_SIZE_MIB=$(CONFIG_TARGET_KERNEL_PARTSIZE); \
	[ "$$ESP_SIZE_MIB" -ge 512 ] || ESP_SIZE_MIB=512; \
	ESP_SIZE_SECTORS=$$(( ESP_SIZE_MIB * SECTORS_PER_MIB )); \
	ROOTFS_START=$$(( (ESP_START + ESP_SIZE_SECTORS + 255) / 256 * 256 )); \
	ROOTFS_SIZE_SECTORS=$$(( $(CONFIG_TARGET_ROOTFS_PARTSIZE) * SECTORS_PER_MIB )); \
	TOTAL_SECTORS=$$(( ROOTFS_START + ROOTFS_SIZE_SECTORS + 256 )); \
	ESP_UUID="$(shell echo $(IMG_PART_DISKGUID) | sed 's/00$$/01/')"; \
	ROOTFS_UUID="$(shell echo $(IMG_PART_DISKGUID) | sed 's/00$$/02/')"; \
	rm -f "$@"; \
	dd if=/dev/zero of="$@" bs=$$SECTOR_SIZE count=$$TOTAL_SECTORS; \
	( \
		echo "label: gpt"; \
		echo "label-id: $(IMG_PART_DISKGUID)"; \
		echo "unit: sectors"; \
		echo "first-lba: $$ESP_START"; \
		echo ""; \
		echo "start=$$ESP_START, size=$$ESP_SIZE_SECTORS, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, uuid=$$ESP_UUID, name=kernel"; \
		echo "start=$$ROOTFS_START, size=$$ROOTFS_SIZE_SECTORS, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, uuid=$$ROOTFS_UUID, name=rootfs"; \
	) | "$$SFDISK_BIN" --sector-size $$SECTOR_SIZE "$@"; \
	mkfs.fat -F 32 -S $$SECTOR_SIZE -s 1 -n kernel -C "$@.esp" "$$((ESP_SIZE_MIB * 1024))"; \
	mmd -i "$@.esp" ::/efi ::/efi/boot ::/efi/openwrt; \
	mcopy -i "$@.esp" "$@.boot/efi/boot/boot$(if $(CONFIG_aarch64),aa64,arm).efi" ::/efi/boot/; \
	mcopy -i "$@.esp" "$@.boot/efi/openwrt/boot$(if $(CONFIG_aarch64),aa64,arm).efi" ::/efi/openwrt/; \
	mcopy -i "$@.esp" "$@.boot/efi/openwrt/$(KERNEL_NAME)" ::/efi/openwrt/; \
	mcopy -i "$@.esp" "$@.boot/efi/openwrt/q6a.dtb" "$@.boot/efi/openwrt/q8b.dtb" ::/efi/openwrt/; \
	mcopy -i "$@.esp" "$@.boot/efi/openwrt/grub.cfg" ::/efi/openwrt/; \
	dd if="$@.esp" of="$@" bs=$$SECTOR_SIZE seek=$$ESP_START conv=notrunc; \
	dd if="$(IMAGE_ROOTFS)" of="$@" bs=$$SECTOR_SIZE seek=$$ROOTFS_START conv=notrunc; \
	rm -f "$@.esp"
endef

define Build/grub-config
	rm -fR $@.boot
	$(INSTALL_DIR) $@.boot/efi/openwrt/
	sed \
		-e 's#@SERIAL_CONFIG@#$(strip $(GRUB_SERIAL_CONFIG))#g' \
		-e 's#@TERMINAL_CONFIG@#$(strip $(GRUB_TERMINAL_CONFIG))#g' \
		-e 's#@ROOTPART@#root=$(ROOTPART) rootwait#g' \
		-e 's#@GPT_ROOTPART@#root=$(GPT_ROOTPART) rootwait#g' \
		-e 's#@CMDLINE@#$(BOOTOPTS) $(GRUB_CONSOLE_CMDLINE)#g' \
		-e 's#@Q6A_CMDLINE@#$(BOOTOPTS) $(BOARD_CONSOLE_CMDLINE) console=ttyMSM0,$(CONFIG_GRUB_BAUDRATE)n8 $(Q6A_EXTRA_CMDLINE)#g' \
		-e 's#@Q8B_CMDLINE@#$(BOOTOPTS) $(BOARD_CONSOLE_CMDLINE) console=ttyMSM0,$(CONFIG_GRUB_BAUDRATE)n8 $(Q8B_EXTRA_CMDLINE)#g' \
		-e 's#@TIMEOUT@#$(GRUB_TIMEOUT)#g' \
		-e 's#@TITLE@#$(GRUB_TITLE)#g' \
		-e 's#@KERNEL_NAME@#$(KERNEL_NAME)#g' \
		./$(GRUB_CONFIG) > $@.boot/efi/openwrt/grub.cfg
endef

define Build/grub-install
	rm -fR $@.grub2
	$(INSTALL_DIR) $@.grub2
endef

DEVICE_VARS += GRUB2_VARIANT UBOOT
define Device/efi-default
  IMAGE/rootfs.img := append-rootfs | pad-to $(ROOTFS_PARTSIZE)
  IMAGE/rootfs.img.gz := append-rootfs | pad-to $(ROOTFS_PARTSIZE) | gzip
  IMAGE/combined-efi.img := grub-config efi | combined efi | grub-install efi | append-metadata
  IMAGE/combined-efi.img.gz := grub-config efi | combined efi | grub-install efi | gzip | append-metadata
  IMAGE/combined-efi.vmdk := grub-config efi | combined efi | grub-install efi | qemu-image vmdk
 ifeq ($(CONFIG_TARGET_IMAGES_GZIP),y)
    IMAGES-y := rootfs.img.gz
    IMAGES-y += combined-efi.img.gz
  else
    IMAGES-y := rootfs.img
    IMAGES-y += combined-efi.img
  endif
  ifeq ($(CONFIG_VMDK_IMAGES),y)
    IMAGES-y += combined-efi.vmdk
  endif
  KERNEL := kernel-bin
  KERNEL_INSTALL := 1
  IMAGES := $$(IMAGES-y)
  ARTIFACTS := $$(ARTIFACTS-y)
  SUPPORTED_DEVICES :=
 ifeq ($(CONFIG_arm),y)
  KERNEL_NAME = zImage
 else
  KERNEL_NAME := Image
 endif
endef

define Device/generic
  $(call Device/efi-default)
  DEVICE_TITLE := Generic EFI Boot
  GRUB2_VARIANT := generic
 ifneq ($(UNIFIED_ARMV8),)
  DEVICE_DTS_DIR := $(DTS_DIR)/qcom
  DEVICE_DTS := qcs6490-radxa-dragon-q6a sc8280xp-radxa-dragon-q8b
 endif
 ifneq ($(UNIFIED_ARMV8),)
  IMAGE/combined-efi-ufs.img := grub-config efi | combined-ufs | append-metadata
  IMAGE/combined-efi-ufs.img.gz := grub-config efi | combined-ufs | gzip | append-metadata
  IMAGES += $(if $(CONFIG_TARGET_IMAGES_GZIP),combined-efi-ufs.img.gz,combined-efi-ufs.img)
 endif
  FILESYSTEMS := ext4 squashfs
  UBOOT := $(if $(CONFIG_aarch64),qemu_armv8,qemu_armv7)
  DEVICE_PACKAGES += kmod-amazon-ena kmod-e1000e kmod-vmxnet3 kmod-rtc-rx8025 \
	kmod-i2c-mux-pca954x kmod-gpio-pca953x partx-utils kmod-wdt-sp805 \
	kmod-mvneta kmod-mvpp2 kmod-fsl-dpaa1-net kmod-fsl-dpaa2-net \
	kmod-fsl-enetc-net kmod-dwmac-imx kmod-fsl-fec \
	kmod-dwmac-rockchip kmod-dwmac-sun8i kmod-phy-aquantia kmod-phy-broadcom \
	kmod-phy-marvell kmod-phy-marvell-10g kmod-sfp kmod-atlantic \
	kmod-bcmgenet kmod-octeontx2-net kmod-renesas-net-avb \
	kmod-phy-realtek kmod-phy-smsc $(if $(CONFIG_aarch64),kmod-thunderx-net)
endef
TARGET_DEVICES += generic

$(eval $(call BuildImage))
