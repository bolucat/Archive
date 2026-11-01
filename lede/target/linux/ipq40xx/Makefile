include $(TOPDIR)/rules.mk

ARCH:=arm
BOARD:=ipq40xx
BOARDNAME:=Qualcomm Atheros IPQ40XX
FEATURES:=squashfs fpu ramdisk
CPU_TYPE:=cortex-a7
CPU_SUBTYPE:=neon-vfpv4
SUBTARGETS:=generic chromium mikrotik

KERNEL_PATCHVER:=6.18

KERNELNAME:=zImage Image dtbs

include $(INCLUDE_DIR)/target.mk
DEFAULT_PACKAGES += \
	kmod-usb-dwc3-qcom \
	kmod-leds-gpio kmod-gpio-button-hotplug \
	kmod-qca-edma kmod-qca-ssdk-10 swconfig \
	kmod-ath10k kmod-ath10k-pci wpad-basic-mbedtls \
	kmod-usb3 kmod-usb-dwc3 ath10k-firmware-qca4019 ath10k-board-qca4019 \
	autocore-arm uboot-envtools

$(eval $(call BuildTarget))
