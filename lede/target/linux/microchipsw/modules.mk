# SPDX-License-Identifier: GPL-2.0-only

define KernelPackage/sparx5-switch
  SUBMENU:=$(NETWORK_DEVICES_MENU)
  TITLE:=Sparx5 switch driver
  DEPENDS:=@TARGET_microchipsw +kmod-phylink +kmod-ptp
  KCONFIG:= \
  CONFIG_NET_SWITCHDEV=y \
  CONFIG_DCB=y \
  CONFIG_SPARX5_SWITCH \
  CONFIG_LAN969X_SWITCH=y \
  CONFIG_SPARX5_DCB=y
  FILES:=$(LINUX_DIR)/drivers/net/ethernet/microchip/sparx5/sparx5-switch.ko
  AUTOLOAD:=$(call AutoProbe,sparx5-switch,1)
endef

define KernelPackage/sparx5-switch/description
  This driver supports the Sparx5 network switch device.
endef

$(eval $(call KernelPackage,sparx5-switch))
