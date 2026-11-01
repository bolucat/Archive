# SPDX-License-Identifier: GPL-2.0-only

define KernelPackage/qcom-ppe
  SUBMENU:=$(NETWORK_DEVICES_MENU)
  DEPENDS:=@TARGET_qualcommbe +kmod-libphy +kmod-pcs-qcom-ipq9574 +kmod-nft-offload
  TITLE:=Qualcomm PPE ethernet controller
  KCONFIG:=CONFIG_QCOM_PPE CONFIG_NET_CLS_ACT=y
  FILES:=$(LINUX_DIR)/drivers/net/ethernet/qualcomm/ppe/qcom-ppe.ko
  AUTOLOAD:=$(call AutoProbe,qcom-ppe)
endef

define KernelPackage/qcom-ppe/description
  This driver supports Qualcomm PPE ethernet controller
  devices.
endef

$(eval $(call KernelPackage,qcom-ppe))

define KernelPackage/pcs-qcom-ipq9574
  SUBMENU:=$(NETWORK_DEVICES_MENU)
  TITLE:=Qualcomm IPQ9574 PCS driver
  DEPENDS:=@TARGET_qualcommbe +kmod-phylink
  KCONFIG:=CONFIG_PCS_QCOM_IPQ9574
  FILES:=$(LINUX_DIR)/drivers/net/pcs/pcs-qcom-ipq9574.ko
  AUTOLOAD:=$(call AutoProbe,pcs-qcom-ipq9574)
endef

$(eval $(call KernelPackage,pcs-qcom-ipq9574))
