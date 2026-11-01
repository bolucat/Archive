include $(TOPDIR)/rules.mk

PKG_NAME:=ath-firmware-ddwrt
PKG_VERSION:=2026.11.01~bd7c0abd
DDWRT_COMMIT:=bd7c0abd4575446a514c5f109de3c317ca26819a
PKG_RELEASE:=1
PKG_UNPACK:=true

PKG_LICENSE:=LicenseRef-Qualcomm-Proprietary
PKG_LICENSE_FILES:=ath12k/QCN9274/hw1.0/Notice.txt

include $(INCLUDE_DIR)/package.mk

RSTRIP:=:
STRIP:=:

# Download only the required firmware directories from the large DD-WRT tree.
# Map the single-radio IPQ5332 image to the driver's standard hw1.0 directory.
define DownloadFirmware
  define Download/ath-fw-$(subst /,_,$(2))
    FILE:=ath-fw-bd7c0abd-$(subst /,_,$(2))
    URL:=https://raw.githubusercontent.com/mirror/dd-wrt/$(DDWRT_COMMIT)/src/router/firmwares/wireless/$(dir $(1))
    URL_FILE:=$(notdir $(1))
    HASH:=$(3)
  endef
  $$(eval $$(call Download,ath-fw-$(subst /,_,$(2))))
  FIRMWARE_DIRS += $(dir $(2))
  FIRMWARE_INSTALL += $$(INSTALL_DATA) $$(DL_DIR)/ath-fw-bd7c0abd-$(subst /,_,$(2)) $$(PKG_BUILD_DIR)/$(2);
endef

include ./firmware.mk

define Package/ath-firmware-ddwrt-default
  SECTION:=firmware
  CATEGORY:=Firmware
  URL:=https://github.com/mirror/dd-wrt/tree/$(DDWRT_COMMIT)/src/router/firmwares/wireless
endef

define Package/ath11k-firmware-ipq9574-ddwrt
  $(Package/ath-firmware-ddwrt-default)
  TITLE:=IPQ9574 ath11k firmware (DD-WRT)
endef

define Package/ath12k-firmware-ipq5332-ddwrt
  $(Package/ath-firmware-ddwrt-default)
  TITLE:=IPQ5332 ath12k firmware (DD-WRT)
endef

define Package/ath12k-firmware-qcn9274-ddwrt
  $(Package/ath-firmware-ddwrt-default)
  TITLE:=QCN9274 ath12k firmware (DD-WRT and Qualcomm)
  CONFLICTS:=ath12k-firmware-qcn9274
endef

define Build/Prepare
	$(call Build/Prepare/Default)
	$(INSTALL_DIR) $(addprefix $(PKG_BUILD_DIR)/,$(sort $(FIRMWARE_DIRS)))
	$(FIRMWARE_INSTALL)
endef

define Build/Compile
endef

define Package/ath11k-firmware-ipq9574-ddwrt/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath11k/IPQ9574/hw1.0
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/ath11k/IPQ9574/hw1.0/* $(1)/lib/firmware/ath11k/IPQ9574/hw1.0/
endef

define Package/ath12k-firmware-ipq5332-ddwrt/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath12k/IPQ5332/hw1.0
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/ath12k/IPQ5332/hw1.0/* $(1)/lib/firmware/ath12k/IPQ5332/hw1.0/
endef

define Package/ath12k-firmware-qcn9274-ddwrt/install
	$(INSTALL_DIR) $(1)/lib/firmware/ath12k/QCN9274/hw1.0 $(1)/lib/firmware/ath12k/QCN9274/hw2.0
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/ath12k/QCN9274/hw1.0/* $(1)/lib/firmware/ath12k/QCN9274/hw1.0/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/ath12k/QCN9274/hw2.0/* $(1)/lib/firmware/ath12k/QCN9274/hw2.0/
endef

$(eval $(call BuildPackage,ath11k-firmware-ipq9574-ddwrt))
$(eval $(call BuildPackage,ath12k-firmware-ipq5332-ddwrt))
$(eval $(call BuildPackage,ath12k-firmware-qcn9274-ddwrt))
