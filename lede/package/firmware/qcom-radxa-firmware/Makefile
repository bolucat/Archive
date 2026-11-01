include $(TOPDIR)/rules.mk

PKG_NAME:=qcom-radxa-firmware
PKG_SOURCE_DATE:=2026-06-29
PKG_SOURCE_VERSION:=6a9efd21ebd78d4dd73939fd3b6a7ff3ef8801ca
PKG_RELEASE:=1

PKG_SOURCE_PROTO:=git
PKG_SOURCE_URL:=https://github.com/radxa-pkg/radxa-firmware.git
PKG_MIRROR_HASH:=7949cc0b0691453e773ebb91a91958f26ca127abb7368a529966ccc47472e5ce

PKG_LICENSE:=Proprietary

include $(INCLUDE_DIR)/package.mk

RSTRIP:=:
STRIP:=:

define Package/qcom-radxa-firmware/Default
  SECTION:=firmware
  CATEGORY:=Firmware
  TITLE:=$(1)
  URL:=$(PKG_SOURCE_URL)
endef

define Build/Compile
endef

define Package/qcom-q6a-firmware
$(call Package/qcom-radxa-firmware/Default,Qualcomm QCS6490 Radxa Dragon Q6A firmware)
  DEPENDS:=@(TARGET_armsr_armv8&&LINUX_6_18)
endef

define Package/qcom-q6a-firmware/install
	$(CP) $(PKG_BUILD_DIR)/radxa-firmware-qcs6490/lib $(1)/
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64
	$(LN) ../dragon-q6a/adsp.mbn \
		$(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64/adsp.mbn
	$(LN) ../dragon-q6a/adspr.jsn \
		$(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64/adspr.jsn
	$(LN) ../dragon-q6a/adspua.jsn \
		$(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64/adspua.jsn
	$(LN) ../dragon-q6a/cdsp.mbn \
		$(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64/cdsp.mbn
	$(LN) ../dragon-q6a/cdspr.jsn \
		$(1)/lib/firmware/qcom/qcs6490/radxa/cm-q64/cdspr.jsn
endef

define Package/qcom-q8b-firmware
$(call Package/qcom-radxa-firmware/Default,Qualcomm SC8280XP Radxa Dragon Q8B firmware)
  DEPENDS:=@(TARGET_armsr_armv8&&LINUX_6_18)
endef

define Package/qcom-q8b-firmware/install
	$(CP) $(PKG_BUILD_DIR)/radxa-firmware-sc8280xp/lib $(1)/
endef

$(eval $(call BuildPackage,qcom-q6a-firmware))
$(eval $(call BuildPackage,qcom-q8b-firmware))
