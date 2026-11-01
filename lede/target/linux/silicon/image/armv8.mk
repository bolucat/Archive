# SPDX-License-Identifier: GPL-2.0-only

define Device/generic
  $(call Device/efi-default)
  DEVICE_VENDOR := Apple
  DEVICE_MODEL := Silicon M1/M2 (EFI)
  DEVICE_TITLE := Apple Silicon EFI Boot
  GRUB2_VARIANT := generic
  FILESYSTEMS := ext4 squashfs
endef
TARGET_DEVICES += generic
