include $(TOPDIR)/rules.mk
include $(INCLUDE_DIR)/kernel.mk

PKG_NAME:=qca-ssdk-10
PKG_RELEASE:=3
PKG_LICENSE:=GPL-2.0-only ISC
PKG_BUILD_PARALLEL:=1

include $(INCLUDE_DIR)/package.mk

define KernelPackage/qca-ssdk-10
  SUBMENU:=Network Devices
  TITLE:=QCA SSDK 10 DESS switch and hardware NAT
  DEPENDS:=@TARGET_ipq40xx +kmod-swconfig +kmod-nf-conntrack +kmod-nf-nat +kmod-nf-flow +kmod-ppp
  KCONFIG:=CONFIG_NET_SWITCHDEV=y
  FILES:=$(PKG_BUILD_DIR)/qca-ssdk.ko
  AUTOLOAD:=$(call AutoLoad,30,qca-ssdk,1)
endef

define Build/Compile
	+$(KERNEL_MAKE) M="$(PKG_BUILD_DIR)" modules
endef

define Build/InstallDev
	$(INSTALL_DIR) $(1)/usr/include/qca-ssdk-10
	$(CP) $(PKG_BUILD_DIR)/include/. $(1)/usr/include/qca-ssdk-10/
endef

$(eval $(call KernelPackage,qca-ssdk-10))
