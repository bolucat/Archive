# SPDX-License-Identifier: GPL-2.0-only
# Image commands used by the imported Linux 6.18 device profiles.

define Build/zyimage
	$(STAGING_DIR_HOST)/bin/zyimage $(1) $@
endef

define Build/buffalo-trx
	$(eval magic=$(word 1,$(1)))
	$(eval kern_bin=$(if $(1),$(IMAGE_KERNEL),$@))
	$(eval rtfs_bin=$(word 2,$(1)))
	$(eval apnd_bin=$(word 3,$(1)))
	$(eval kern_size=$(if $(KERNEL_SIZE),$(KERNEL_SIZE),0x400000))

	$(if $(rtfs_bin),touch $(rtfs_bin))
	$(STAGING_DIR_HOST)/bin/otrx create $@.new \
		$(if $(magic),-M $(magic),) \
		-f $(kern_bin) \
		$(if $(rtfs_bin),\
			-a 0x20000 \
			-b $$(( $(subst k,* 1024,$(subst m,* 1024k,$(kern_size))) )) \
			-f $(rtfs_bin),) \
		$(if $(apnd_bin),\
			-A $(apnd_bin) \
			-a 0x20000)
	mv $@.new $@
endef

define Build/dlink-sge-image
	$(STAGING_DIR_HOST)/bin/dlink-sge-image $(1) $@ $@.enc
	mv $@.enc $@
endef

define Build/append-md5sum-ascii-salted
	cp $@ $@.salted
	echo -ne $(1) >> $@.salted
	$(STAGING_DIR_HOST)/bin/mkhash md5 $@.salted | head -c32 >> $@
	rm $@.salted
endef

define Build/kernel-pack-npk
	$(STAGING_DIR_HOST)/bin/npk_pack_kernel $@ $@.npk
	mv $@.npk $@
endef

define Build/dualboot-datachk-nand-image
	$(TOPDIR)/scripts/nand-fwupgradecfg-gen.sh \
		"$(call param_get_default,ce_type,$(1),$(DEVICE_NAME))" \
		"$@-fwupgrade.cfg" \
		"$(call param_get_default,kernel,$(1),$(IMAGE_KERNEL))" \
		"$(call param_get_default,rootfs,$(1),$@)"
	$(TOPDIR)/scripts/combined-ext-image.sh \
		"$(call param_get_default,ce_type,$(1),$(DEVICE_NAME))" "$@" \
		"$@-fwupgrade.cfg" "fwupgrade.cfg" \
		"$(call param_get_default,kernel,$(1),$(IMAGE_KERNEL))" "kernel" \
		"$(call param_get_default,rootfs,$(1),$@)" "rootfs"
endef

define Build/append-teltonika-metadata
	echo \
		'{$(if $(IMAGE_METADATA),$(IMAGE_METADATA)$(comma)) \
			"metadata_version": "1.1", \
			"compat_version": "$(call json_quote,$(compat_version))", \
			"version":"$(call json_quote,$(VERSION_DIST))-$(call json_quote,$(VERSION_NUMBER))-$(call json_quote,$(REVISION))", \
			"device_code": [".*"], \
			"hwver": [".*"], \
			"batch": [".*"], \
			"serial": [".*"], \
			$(if $(DEVICE_COMPAT_MESSAGE),"compat_message": "$(call json_quote,$(DEVICE_COMPAT_MESSAGE))"$(comma)) \
			$(if $(filter-out 1.0,$(compat_version)),"new_supported_devices": \
				[$(call metadata_devices,$(SUPPORTED_TELTONIKA_DEVICES))]$(comma) \
				"supported_devices": ["$(call json_quote,$(legacy_supported_message))"]$(comma)) \
			$(if $(filter 1.0,$(compat_version)),"supported_devices":[$(call metadata_devices,$(SUPPORTED_TELTONIKA_DEVICES))]$(comma)) \
			"version_wrt": { \
			"dist": "$(call json_quote,$(VERSION_DIST))", \
			"version": "$(call json_quote,$(VERSION_NUMBER))", \
			"revision": "$(call json_quote,$(REVISION))", \
			"target": "$(call json_quote,$(TARGETID))", \
			"board": "$(call json_quote,$(if $(BOARD_NAME),$(BOARD_NAME),$(DEVICE_NAME)))" \
		}, \
			"hw_support": {}, \
			"hw_mods": {$(shell i=1; for mod in $(SUPPORTED_TELTONIKA_HW_MODS); do [ $$i -gt 1 ] && echo -n ,; echo -n "\"mod$$i\": \"$$mod\""; i=$$((i+1)); done)} \
		}' | fwtool -I - $@
endef
