# SPDX-License-Identifier: GPL-2.0-or-later
#

include $(TOPDIR)/rules.mk

ARCH:=aarch64
BOARD:=silicon
BOARDNAME:=Apple Silicon family
FEATURES:=audio boot-part display ext4 fpu gpio pci pcie rootfs-part rtc squashfs usb usbgadget
FEATURES+=cpiogz ramdisk targz
SUBTARGETS:=armv8

KERNEL_PATCHVER:=6.18
KERNEL_TESTING_PATCHVER:=6.1

define Target/Description
	Build firmware image for Apple Silicon family
endef

include $(INCLUDE_DIR)/target.mk

DEFAULT_PACKAGES += \
	e2fsprogs \
	kmod-sound-core \
	kmod-usb-hid \
	mkf2fs \
	partx-utils \
	blkid \
	grub2-efi-arm \
	kmod-fs-vfat \
	kmod-usb-net-rtl8152

KERNELNAME:=Image dtbs

$(eval $(call BuildTarget))
