include $(TOPDIR)/rules.mk

ARCH:=arm
BOARD:=airoha
BOARDNAME:=Airoha ARM
SUBTARGETS:=an7581 en7523 
FEATURES:=dt squashfs nand ramdisk gpio

KERNEL_PATCHVER:=6.12
KERNEL_TESTING_PATCHVER:=6.6

include $(INCLUDE_DIR)/target.mk

$(eval $(call BuildTarget))
