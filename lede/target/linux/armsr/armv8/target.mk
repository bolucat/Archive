ARCH:=aarch64
SUBTARGET:=armv8
BOARDNAME:=64-bit (armv8) machines (Generic, Qualcomm)

define Target/Description
  Build multi-platform images for the ARMv8 instruction set architecture
endef

ifeq ($(CONFIG_TESTING_KERNEL),)
KERNELNAME:=Image dtbs
FEATURES += audio display gpio

DEFAULT_PACKAGES += grub2-efi-arm partx-utils kmod-fs-ext4 kmod-fs-f2fs \
	f2fsck losetup autocore-arm kmod-usb-net-rtl8152 kmod-video-core \
	kmod-gpio-tc956x qcom-q6a-firmware qcom-q8b-firmware \
	qcom-adreno-a6xx-firmware kmod-qcom-msm-vendor-drm kmod-qcom-iris-vendor \
	kmod-npu-qcom wpad-openssl kmod-eeprom-at24 kmod-stmmac-tc956x kmod-tc956x-pci
endif
