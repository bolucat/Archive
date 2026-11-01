Package/qcom-venus-firmware = $(call Package/firmware-default,Qualcomm Venus video firmware)
define Package/qcom-venus-firmware/install
	$(INSTALL_DIR) $(1)/lib/firmware/qcom
	for d in $(PKG_BUILD_DIR)/qcom/venus-*; do [ -d "$$d" ] || continue; $(CP) -R "$$d" $(1)/lib/firmware/qcom/; done
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/vpu
	$(CP) $(PKG_BUILD_DIR)/qcom/vpu/vpu20_p1.mbn $(PKG_BUILD_DIR)/qcom/vpu/vpu20_p4.mbn $(1)/lib/firmware/qcom/vpu/
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/vpu-1.0 $(1)/lib/firmware/qcom/vpu-2.0
	$(LN) ../vpu/vpu20_p4.mbn $(1)/lib/firmware/qcom/vpu-1.0/venus.mbn
	$(LN) ../vpu/vpu20_p1.mbn $(1)/lib/firmware/qcom/vpu-2.0/venus.mbn
endef
$(eval $(call BuildPackage,qcom-venus-firmware))

Package/qcom-iris-firmware = $(call Package/firmware-default,Qualcomm Iris video firmware)
define Package/qcom-iris-firmware/install
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/vpu
	$(CP) $(PKG_BUILD_DIR)/qcom/vpu/vpu20_p4.mbn $(1)/lib/firmware/qcom/vpu/
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/vpu-1.0
	$(LN) ../vpu/vpu20_p4.mbn $(1)/lib/firmware/qcom/vpu-1.0/venus.mbn
endef
$(eval $(call BuildPackage,qcom-iris-firmware))

Package/qcom-adreno-a6xx-firmware = $(call Package/firmware-default,Qualcomm Adreno A6xx GPU firmware,@(TARGET_armsr_armv8&&LINUX_6_18))
define Package/qcom-adreno-a6xx-firmware/install
	$(INSTALL_DIR) $(1)/lib/firmware/qcom
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/qcom/a660_gmu.bin \
		$(PKG_BUILD_DIR)/qcom/a660_sqe.fw \
		$(1)/lib/firmware/qcom/
	$(INSTALL_DIR) $(1)/lib/firmware/qcom/sc8280xp/LENOVO/21BX
	$(INSTALL_DATA) \
		$(PKG_BUILD_DIR)/qcom/sc8280xp/LENOVO/21BX/qcdxkmsuc8280.mbn \
		$(1)/lib/firmware/qcom/sc8280xp/LENOVO/21BX/
	$(LN) sc8280xp/LENOVO/21BX/qcdxkmsuc8280.mbn \
		$(1)/lib/firmware/qcom/a690_zap.mdt
endef

$(eval $(call BuildPackage,qcom-adreno-a6xx-firmware))
