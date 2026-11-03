DTS_DIR := $(DTS_DIR)/qcom

metadata_gl_json = \
	'{ $(if $(IMAGE_METADATA),$(IMAGE_METADATA)$(comma)) \
		"metadata_version": "1.1", \
		"compat_version": "$(call json_quote,$(compat_version))", \
		$(if $(DEVICE_COMPAT_MESSAGE),"compat_message": "$(call json_quote,$(DEVICE_COMPAT_MESSAGE))"$(comma)) \
		$(if $(filter-out 1.0,$(compat_version)),"new_supported_devices": \
			[$(call metadata_devices,$(SUPPORTED_DEVICES))]$(comma) \
			"supported_devices": ["$(call json_quote,$(legacy_supported_message))"]$(comma)) \
		$(if $(filter 1.0,$(compat_version)),"supported_devices":[$(call metadata_devices,$(SUPPORTED_DEVICES))]$(comma)) \
		"version": { \
			"release": "$(call json_quote,$(VERSION_NUMBER))", \
			"date": "$(shell TZ='Asia/Chongqing' date $(if $(SOURCE_DATE_EPOCH),-d@$(SOURCE_DATE_EPOCH)) '+%Y%m%d%H%M%S')", \
			"dist": "$(call json_quote,$(VERSION_DIST))", \
			"version": "$(call json_quote,$(VERSION_NUMBER))", \
			"revision": "$(call json_quote,$(REVISION))", \
			"target": "$(call json_quote,$(TARGETID))", \
			"board": "$(call json_quote,$(if $(BOARD_NAME),$(BOARD_NAME),$(DEVICE_NAME)))" \
		} \
	}'

define Build/append-gl-metadata
	$(if $(SUPPORTED_DEVICES),-echo $(call metadata_gl_json,$(SUPPORTED_DEVICES)) | fwtool -I - $@)
	sha256sum "$@" | cut -d" " -f1 > "$@.sha256sum"
	$(if $(CONFIG_SIGN_FIRMWARE),[ ! -s "$(BUILD_KEY)" -o ! -s "$(BUILD_KEY).ucert" -o ! -s "$@" ] || { \
		cp "$(BUILD_KEY).ucert" "$@.ucert" ;\
		usign -S -m "$@" -s "$(BUILD_KEY)" -x "$@.sig" ;\
		ucert -A -c "$@.ucert" -x "$@.sig" ;\
		fwtool -S "$@.ucert" "$@" ;\
	})
endef

define Build/gl-be9300-factory
	$(TOPDIR)/scripts/mkits-qsdk-ipq-image.sh $@.its \
		$(TOPDIR)/target/linux/qualcommbe/image/gl-be9300-factory.bootscript \
		hlos $(IMAGE_KERNEL) rootfs $@
	PATH=$(LINUX_DIR)/scripts/dtc:$(PATH) mkimage -f $@.its $@.new
	mv $@.new $@
	rm -f $@.its
endef

define Device/glinet_gl-be9300
	$(call Device/FitImage)
	$(call Device/EmmcImage)
	DEVICE_VENDOR := GL.iNet
	DEVICE_MODEL := GL-BE9300
	DEVICE_ALT0_VENDOR := GL.iNet
	DEVICE_ALT0_MODEL := Flint 3
	# Stock U-Boot has no AP-MI01.6 entry in its board->config table, so
	# bootipq falls back to asking for "config-1". Any other name - a
	# board-specific config@mi01.6, or OpenWrt's own default config@1 -
	# fails with "Config not available" and the unit will not boot from
	# eMMC. (BE6500 can use config@mi01.2 because that board IS in the
	# table.)
	DEVICE_DTS_CONFIG := config-1
	SOC := ipq5332
	SUPPORTED_DEVICES += gl.inet,gl-be9300
	# The kernel FIT is dd'd into 0:HLOS (mmcblk0p12, 7 MiB) by
	# glinet_emmc_do_fit_upgrade() with no size check; a truncated kernel
	# never boots and the unit comes up dark. A default build is ~5.75 MiB,
	# so refuse to build anything the partition cannot hold. The initramfs
	# image is never written to eMMC and is exempt.
	KERNEL_SIZE := 7168k
	KERNEL := kernel-bin | libdeflate-gzip | fit gzip $$(KDIR)/image-$$(DEVICE_DTS).dtb | check-size $$(KERNEL_SIZE)
	KERNEL_INITRAMFS := kernel-bin | libdeflate-gzip | fit gzip $$(KDIR)/image-$$(DEVICE_DTS).dtb
	IMAGE/factory.bin := append-rootfs | pad-rootfs | pad-to 64k | \
		gl-be9300-factory | append-gl-metadata
	DEVICE_PACKAGES := kmod-ath12k ath12k-firmware-ipq5332-ddwrt \
		ath12k-firmware-qcn9274-ddwrt ipq-wifi-glinet_gl-be9300 \
		kmod-hwmon-pwmfan kmod-qrtr-smd kmod-dsa-rtl837x \
		kmod-phy-realtek ethtool e2fsprogs f2fsck mkf2fs dumpimage \
		kmod-usb-core kmod-usb2 kmod-usb3 kmod-usb-dwc3 \
		kmod-usb-dwc3-qcom kmod-usb-xhci-hcd kmod-scsi-core \
		kmod-usb-storage kmod-usb-storage-uas kmod-fs-ext4 \
		block-mount blockd usbutils
endef
TARGET_DEVICES += glinet_gl-be9300

define Device/jdcloud_re-cs-06
	$(call Device/FitImage)
	$(call Device/EmmcImage)
	DEVICE_VENDOR := JDCloud
	DEVICE_MODEL := BE6500
	DEVICE_DTS_CONFIG := config@mi01.6
	KERNEL_SIZE := 7168k
	SOC := ipq5322
	SUPPORTED_DEVICES += jdcloud,re-cs-06
	DEVICE_PACKAGES := kmod-ath12k ath12k-firmware-ipq5332-ddwrt ath12k-firmware-qcn9274-ddwrt ipq-wifi-jdcloud_re-cs-06
	IMAGE/factory.bin := append-kernel | pad-to $$(KERNEL_SIZE) | append-rootfs | append-metadata
endef
TARGET_DEVICES += jdcloud_re-cs-06

define Device/jdcloud_re-cs-08
	$(call Device/FitImage)
	$(call Device/EmmcImage)
	DEVICE_VENDOR := JDCloud
	DEVICE_MODEL := ER2
	DEVICE_DTS_CONFIG := config@mi01.6
	KERNEL_SIZE := 7168k
	SOC := ipq5322
	SUPPORTED_DEVICES += jdcloud,re-cs-08
	DEVICE_PACKAGES := -kmod-ath12k -wpad-openssl kmod-sfp \
		kmod-phy-marvell kmod-phy-marvell-10g
	IMAGE/factory.bin := append-kernel | pad-to $$(KERNEL_SIZE) | append-rootfs | append-metadata
endef
TARGET_DEVICES += jdcloud_re-cs-08
