#
# Copyright (C) 2022 OpenWrt.org
#
# This is free software, licensed under the GNU General Public License v2.
# See /LICENSE for more information.
#

include $(TOPDIR)/rules.mk

PKG_NAME:=ath11k-firmware
PKG_SOURCE_DATE:=2023-03-31
PKG_SOURCE_VERSION:=a039049a9349722fa5c74185452ab04644a0d351
PKG_MIRROR_HASH:=ed401e3f6e91d70565b3396139193f7e815f410db93700697205ac8ed1b828c5
PKG_RELEASE:=1

PKG_SOURCE_PROTO:=git
PKG_SOURCE_URL:=https://github.com/quic/upstream-wifi-fw.git

PKG_LICENSE_FILES:=LICENSE.qca_firmware

PKG_MAINTAINER:=Robert Marko <robimarko@gmail.com>

include $(INCLUDE_DIR)/package.mk

RSTRIP:=:
STRIP:=:

ATH11K_IPQ6018_FW_DATE:=2024-10-14
ATH11K_IPQ6018_FW_RELEASE:=15f05012
ATH11K_IPQ6018_FW_SUBDIR:=ath11k-ipq6018-firmware-$(ATH11K_IPQ6018_FW_DATE)~$(ATH11K_IPQ6018_FW_RELEASE)
ATH11K_IPQ6018_FW_SOURCE:=$(ATH11K_IPQ6018_FW_SUBDIR).tar.zst

define Download/ath11k-ipq6018-firmware
	FILE:=$(ATH11K_IPQ6018_FW_SOURCE)
	VERSION:=15f050122da5ef5bef2cc8c7c19dfb7f98060a49
	PROTO:=git
	URL:=https://git.codelinaro.org/clo/ath-firmware/ath11k-firmware.git
	SOURCE_VERSION:=15f050122da5ef5bef2cc8c7c19dfb7f98060a49
	MIRROR_HASH:=e7a89d1570f32552ade539376181494724b5eafb31d00451ff2dcf5a2922a0cb
	SUBDIR:=$(ATH11K_IPQ6018_FW_SUBDIR)
endef
$(eval $(call Download,ath11k-ipq6018-firmware))

define Build/Prepare
	$(call Build/Prepare/Default,)
	$(TAR) -C $(BUILD_DIR) -xf $(DL_DIR)/$(ATH11K_IPQ6018_FW_SOURCE)
endef

define Build/Clean
	$(call Build/Clean/Default,)
	rm -rf $(BUILD_DIR)/$(ATH11K_IPQ6018_FW_SUBDIR)
endef

define Package/ath11k-firmware-default
  SECTION:=firmware
  CATEGORY:=Firmware
  URL:=$(PKG_SOURCE_URL)
  DEPENDS:=
endef

define Package/ath11k-firmware-ipq5018
$(Package/ath11k-firmware-default)
  TITLE:=IPQ5018 ath11k firmware
endef

define Package/ath11k-firmware-ipq6018
$(Package/ath11k-firmware-default)
  TITLE:=IPQ6018 ath11k firmware
endef

define Package/ath11k-firmware-ipq8074
$(Package/ath11k-firmware-default)
  TITLE:=IPQ8074 ath11k firmware
endef

define Package/ath11k-firmware-qcn6122
$(Package/ath11k-firmware-default)
  TITLE:=QCN6122 ath11k firmware
  DEPENDS:=ath11k-firmware-ipq5018
endef

define Package/ath11k-firmware-qcn9074
$(Package/ath11k-firmware-default)
  TITLE:=QCN9074 ath11k firmware
endef

define Build/Compile

endef

define Package/ath11k-firmware-ipq5018/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath11k/IPQ5018/hw1.0
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/ath11k-firmware/IPQ5018_QCN6122_QCN6122/hw1.0/2.7.0.1/WLAN.HK.2.7.0.1-01744-QCAHKSWPL_SILICONZ-1/*.* \
		$(1)/lib/firmware/ath11k/IPQ5018/hw1.0/
endef

define Package/ath11k-firmware-ipq6018/install
	$(INSTALL_DIR) $(1)/lib/firmware/IPQ6018
	$(INSTALL_DATA) \
		$(BUILD_DIR)/$(ATH11K_IPQ6018_FW_SUBDIR)/IPQ6018/hw1.0/2.5.0.1/WLAN.HK.2.5.0.1-03982-QCAHKSWPL_SILICONZ-3/* \
		$(1)/lib/firmware/IPQ6018/
endef

define Package/ath11k-firmware-ipq8074/install
	$(INSTALL_DIR) $(1)/lib/firmware/IPQ8074
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/ath11k-firmware/IPQ8074/hw2.0/testing/2.9.0.1/WLAN.HK.2.9.0.1-01385-QCAHKSWPL_SILICONZ-1/* \
		$(1)/lib/firmware/IPQ8074/
endef

define Package/ath11k-firmware-qcn6122/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath11k/QCN6122/hw1.0
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/ath11k-firmware/IPQ5018_QCN6122_QCN6122/hw1.0/2.7.0.1/WLAN.HK.2.7.0.1-01744-QCAHKSWPL_SILICONZ-1/qcn6122/* \
		$(1)/lib/firmware/ath11k/QCN6122/hw1.0/
endef

define Package/ath11k-firmware-qcn9074/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath11k/QCN9074/hw1.0
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/ath11k-firmware/QCN9074/hw1.0/testing/2.9.0.1/WLAN.HK.2.9.0.1-01385-QCAHKSWPL_SILICONZ-1/* \
		$(1)/lib/firmware/ath11k/QCN9074/hw1.0/
endef

$(eval $(call BuildPackage,ath11k-firmware-ipq5018))
$(eval $(call BuildPackage,ath11k-firmware-ipq6018))
$(eval $(call BuildPackage,ath11k-firmware-ipq8074))
$(eval $(call BuildPackage,ath11k-firmware-qcn6122))
$(eval $(call BuildPackage,ath11k-firmware-qcn9074))
