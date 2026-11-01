include $(TOPDIR)/rules.mk
include $(INCLUDE_DIR)/kernel.mk

PKG_NAME:=qca-edma
PKG_RELEASE:=4
PKG_LICENSE:=ISC GPL-2.0-only
PKG_BUILD_DEPENDS:=qca-ssdk-10

include $(INCLUDE_DIR)/package.mk

define KernelPackage/qca-edma
  SUBMENU:=Network Devices
  TITLE:=QCA IPQ40xx ESS EDMA driver
  DEPENDS:=@TARGET_ipq40xx +kmod-qca-ssdk-10
  FILES:=$(PKG_BUILD_DIR)/essedma.ko
  AUTOLOAD:=$(call AutoLoad,45,essedma,1)
endef

define Build/Compile
	+$(KERNEL_MAKE) M="$(PKG_BUILD_DIR)" \
		SSDK_INCLUDE="$(STAGING_DIR)/usr/include/qca-ssdk-10/ref" modules
endef

$(eval $(call KernelPackage,qca-edma))
