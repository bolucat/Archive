define Device/Default
  PROFILES := Default
  FILESYSTEMS := squashfs ext4
  KERNEL_INSTALL := 1
  KERNEL_SUFFIX := -uImage
  KERNEL_NAME := zImage
  DTS_DIR := $(DTS_DIR)$(if $(filter 6.18,$(KERNEL_PATCHVER)),/nxp/imx)
  KERNEL := kernel-bin | uImage none
  KERNEL_LOADADDR := 0x80008000
  IMAGES :=
endef
