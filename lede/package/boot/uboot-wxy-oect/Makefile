#
# This is free software, licensed under the GNU General Public License v2.
# See /LICENSE for more information.
#
include $(TOPDIR)/rules.mk

PKG_NAME:=uboot-rk35xx
PKG_RELEASE:=5

PKG_SOURCE_PROTO:=git
PKG_SOURCE_URL:=https://github.com/radxa/u-boot

PKG_SOURCE_DATE:=2025-11-07
PKG_SOURCE_VERSION:=24bdf5a8a34e52962a066765fea23b4237309fe3
PKG_MIRROR_HASH:=1950cece2af4f71badb896dc915a73cadfb03118cd1a1bad1621f23d95ff3bdd

PKG_MAINTAINER:=lean

include $(INCLUDE_DIR)/u-boot.mk
include $(INCLUDE_DIR)/package.mk

define U-Boot/Default
  BUILD_TARGET:=rockchip
  UENV:=default
  HIDDEN:=1
endef

define U-Boot/wxy-oect
  NAME:=WXY OEC Turbo
  BUILD_SUBTARGET:=armv8
  BUILD_DEVICES:=wxy_oec-turbo-4g
endef

UBOOT_TARGETS := wxy-oect

define Build/Configure
	true
endef

define Build/Compile
	true
endef

define Build/InstallDev
	$(INSTALL_DIR) $(STAGING_DIR_IMAGE)
	$(CP) ./bootloader/* $(STAGING_DIR_IMAGE)/
endef

define Package/u-boot/install/default
endef

$(eval $(call BuildPackage/U-Boot))
