include $(TOPDIR)/rules.mk

PKG_NAME:=qcom-msm-vendor-drivers
PKG_RELEASE:=13
PKG_LICENSE:=GPL-2.0-only

PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)
PKG_BUILD_PARALLEL:=1

include $(INCLUDE_DIR)/kernel.mk
include $(INCLUDE_DIR)/package.mk

# Kernel helper packages own helpers enabled by the kernel configuration.
# Include a locally rebuilt helper only when it has no kernel package payload.
QCOM_MSM_VENDOR_UNPACKAGED_HELPER = $(if $(filter y m,$(shell sed -n 's/^$(2)=//p' $(LINUX_DIR)/.config 2>/dev/null)),,$(LINUX_DIR)/drivers/gpu/drm/$(1).ko)

define KernelPackage/qcom-msm-vendor-drm
  SUBMENU:=$(VIDEO_MENU)
  TITLE:=Radxa 7.0.11 Qualcomm MSM DRM driver
  DEPENDS:=@(TARGET_armsr_armv8&&LINUX_6_18) +kmod-backlight +kmod-drm-kms-helper \
	+kmod-drm-display-helper +kmod-drm-client-lib +kmod-drm-sched \
	+kmod-drm-gem-shmem-helper +kmod-qcom-mdt-loader \
	+kmod-qcom-ubwc-config +kmod-drm-simple-bridge \
	+kmod-drm-display-connector \
	+(LINUX_6_18||LINUX_7_0):kmod-drm-exec \
	+(LINUX_6_18||LINUX_7_0):kmod-drm-suballoc-helper
  CONFLICTS:=kmod-drm-msm
  FILES:= \
	$(LINUX_DIR)/drivers/gpu/drm/display/drm_dp_aux_bus.ko \
	$(call QCOM_MSM_VENDOR_UNPACKAGED_HELPER,drm_exec,CONFIG_DRM_EXEC) \
	$(LINUX_DIR)/drivers/gpu/drm/drm_gpuvm.ko \
	$(call QCOM_MSM_VENDOR_UNPACKAGED_HELPER,drm_suballoc_helper,CONFIG_DRM_SUBALLOC_HELPER) \
	$(PKG_BUILD_DIR)/drivers/gpu/drm/msm/msm.ko
endef

define KernelPackage/qcom-msm-vendor-drm/description
  Out-of-tree Qualcomm MSM display and Adreno GPU DRM driver from Radxa linux-7.0.11.
endef

define KernelPackage/qcom-msm-vendor-drm/install
	$(INSTALL_DIR) $(1)/etc/modules.d
	printf '%s\n' 'msm a6xx_max_opp=5' > $(1)/etc/modules.d/qcom-msm-vendor-drm
endef

define KernelPackage/qcom-iris-vendor
  SUBMENU:=$(VIDEO_MENU)
  TITLE:=Radxa 7.0.11 Qualcomm Iris VPU driver
  DEPENDS:=@(TARGET_armsr_armv8&&LINUX_6_18) +kmod-qcom-mdt-loader +kmod-video-mem2mem \
	+kmod-video-dma +kmod-qcom-videocc +kmod-qcom-ubwc-config
  CONFLICTS:=kmod-video-qcom-iris
  FILES:= \
	$(PKG_BUILD_DIR)/drivers/media/platform/qcom/iris/qcom-iris.ko
  AUTOLOAD:=$(call AutoProbe,qcom-iris)
  $(call AddDepends/video)
endef

define KernelPackage/qcom-iris-vendor/description
  Out-of-tree Qualcomm Iris video accelerator driver from Radxa linux-7.0.11.
endef

define KernelPackage/qcom-cdsp-nsp-vendor
  SUBMENU:=$(VIDEO_MENU)
  TITLE:=Radxa 7.0.11 Qualcomm CDSP/NSP remoteproc and FastRPC drivers
  DEPENDS:=@(TARGET_armsr_armv8&&LINUX_6_18) +kmod-qcom-mdt-loader +kmod-dma-buf
  CONFLICTS:=kmod-npu-qcom kmod-qcom-fastrpc
  FILES:= \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_common.ko \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_pil_info.ko \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_q6v5.ko \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_q6v5_adsp.ko \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_q6v5_pas.ko \
	$(PKG_BUILD_DIR)/drivers/remoteproc/qcom_sysmon.ko \
	$(PKG_BUILD_DIR)/drivers/rpmsg/qcom_glink_smem.ko \
	$(PKG_BUILD_DIR)/drivers/misc/fastrpc.ko
  AUTOLOAD:=$(call AutoProbe,qcom_common qcom_pil_info qcom_q6v5 qcom_q6v5_adsp qcom_q6v5_pas qcom_sysmon qcom_glink_smem fastrpc)
endef

define KernelPackage/qcom-cdsp-nsp-vendor/description
  Out-of-tree Qualcomm CDSP/NSP remoteproc, GLINK-SMEM and FastRPC drivers from Radxa linux-7.0.11.
endef

# Generic EFI graphics (e.g. QXL) can select DRM_EXEC built-in. Rebuilding
# that helper as a module would duplicate symbols already exported by vmlinux.
QCOM_MSM_VENDOR_DRM_EXEC=$(if $(filter y,$(shell sed -n 's/^CONFIG_DRM_EXEC=//p' $(LINUX_DIR)/.config 2>/dev/null)),y,m)

QCOM_MSM_VENDOR_KCONFIG:= \
	CONFIG_DRM_MSM=m \
	CONFIG_DRM_MSM_DP=y \
	CONFIG_DRM_BRIDGE_CONNECTOR=y \
	CONFIG_DRM_EXEC=$(QCOM_MSM_VENDOR_DRM_EXEC) \
	CONFIG_DRM_DISPLAY_DSC_HELPER=y \
	CONFIG_DRM_DISPLAY_HDMI_AUDIO_HELPER=y \
	CONFIG_DRM_DISPLAY_HDMI_CEC_HELPER=y \
	CONFIG_DRM_DISPLAY_HDMI_HELPER=y \
	CONFIG_DRM_DISPLAY_HDMI_STATE_HELPER=y \
	CONFIG_DRM_GPUVM=y \
	CONFIG_DRM_SUBALLOC_HELPER=m \
	CONFIG_QCOM_UBWC_CONFIG=m \
	CONFIG_DRM_MSM_DPU=y \
	CONFIG_DRM_MSM_KMS=y \
	CONFIG_DRM_MSM_KMS_FBDEV=y \
	CONFIG_DRM_MSM_MDSS=y \
	CONFIG_DRM_MSM_GPU_STATE=y \
	CONFIG_DRM_MSM_DSI=n \
	CONFIG_DRM_MSM_DSI_28NM_PHY=n \
	CONFIG_DRM_MSM_DSI_20NM_PHY=n \
	CONFIG_DRM_MSM_DSI_28NM_8960_PHY=n \
	CONFIG_DRM_MSM_DSI_14NM_PHY=n \
	CONFIG_DRM_MSM_DSI_10NM_PHY=n \
	CONFIG_DRM_MSM_DSI_7NM_PHY=n \
	CONFIG_DRM_MSM_HDMI=n \
	CONFIG_DRM_MSM_MDP4=n \
	CONFIG_DRM_MSM_MDP5=n \
	CONFIG_VIDEO_QCOM_IRIS=m

QCOM_MSM_VENDOR_KCFLAGS:= \
	-I$(PKG_BUILD_DIR)/include \
	-I$(LINUX_DIR)/drivers/gpu/drm \
	-include linux/errno.h \
	-include linux/err.h \
	$(if $(shell grep -l drm_fb_helper_alloc_info $(LINUX_DIR)/include/drm/drm_fb_helper.h 2>/dev/null),-DQCOM_FB_HELPER_HAS_ALLOC_INFO=1) \
	-DUBWC_6_0=0x60000000 \
	-DQCOM_MSM_VENDOR_NO_DSI_HDMI_MDP=1 \
	-DCONFIG_DRM_MSM_DP=1 \
	-DCONFIG_DRM_MSM_DPU=1 \
	-DCONFIG_DRM_MSM_KMS=1 \
	-DCONFIG_DRM_MSM_KMS_FBDEV=1 \
	-DCONFIG_DRM_MSM_MDSS=1 \
	-DCONFIG_DRM_MSM_GPU_STATE=1 \
	-UCONFIG_DRM_MSM_DSI \
	-UCONFIG_DRM_MSM_DSI_28NM_PHY \
	-UCONFIG_DRM_MSM_DSI_20NM_PHY \
	-UCONFIG_DRM_MSM_DSI_28NM_8960_PHY \
	-UCONFIG_DRM_MSM_DSI_14NM_PHY \
	-UCONFIG_DRM_MSM_DSI_10NM_PHY \
	-UCONFIG_DRM_MSM_DSI_7NM_PHY \
	-UCONFIG_DRM_MSM_HDMI \
	-UCONFIG_DRM_MSM_HDMI_HDCP \
	-UCONFIG_DRM_MSM_MDP4 \
	-UCONFIG_DRM_MSM_MDP5

QCOM_MSM_VENDOR_IRIS_KCFLAGS:= \
	$(QCOM_MSM_VENDOR_KCFLAGS) \
	-include $(PKG_BUILD_DIR)/include/linux/soc/qcom/ubwc.h

QCOM_MSM_VENDOR_EXTRA_SYMBOLS:= \
	$(LINUX_DIR)/drivers/gpu/drm/Module.symvers

QCOM_MSM_VENDOR_IRIS_EXTRA_SYMBOLS:= \
	$(if $(CONFIG_PACKAGE_kmod-qcom-msm-vendor-drm),$(QCOM_MSM_VENDOR_EXTRA_SYMBOLS))

QCOM_CDSP_NSP_VENDOR_KCFLAGS:= \
	-I$(PKG_BUILD_DIR)/include \
	-I$(PKG_BUILD_DIR)/drivers/remoteproc \
	-I$(PKG_BUILD_DIR)/drivers/rpmsg \
	-I$(LINUX_DIR)/drivers/remoteproc \
	-I$(LINUX_DIR)/drivers/rpmsg

define Build/Compile/drm-helpers
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
		$(KERNEL_MAKE_FLAGS) \
		M="$(LINUX_DIR)/drivers/gpu/drm" \
		CONFIG_DRM_BRIDGE_CONNECTOR=y \
		CONFIG_DRM_EXEC=$(QCOM_MSM_VENDOR_DRM_EXEC) \
		CONFIG_DRM_DISPLAY_DSC_HELPER=y \
		CONFIG_DRM_DISPLAY_HDMI_AUDIO_HELPER=y \
		CONFIG_DRM_DISPLAY_HDMI_CEC_HELPER=y \
		CONFIG_DRM_DISPLAY_HDMI_HELPER=y \
		CONFIG_DRM_DISPLAY_HDMI_STATE_HELPER=y \
		CONFIG_DRM_GPUVM=m \
		CONFIG_DRM_SUBALLOC_HELPER=m \
		modules
endef

define Build/Compile/msm
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
		$(KERNEL_MAKE_FLAGS) \
		M="$(PKG_BUILD_DIR)/drivers/gpu/drm/msm" \
		KCFLAGS="$(QCOM_MSM_VENDOR_KCFLAGS)" \
		KBUILD_EXTRA_SYMBOLS="$(QCOM_MSM_VENDOR_EXTRA_SYMBOLS)" \
		$(QCOM_MSM_VENDOR_KCONFIG) \
		modules
endef

define Build/Compile/iris
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
			$(KERNEL_MAKE_FLAGS) \
			M="$(PKG_BUILD_DIR)/drivers/media/platform/qcom/iris" \
			KCFLAGS="$(QCOM_MSM_VENDOR_IRIS_KCFLAGS)" \
			KBUILD_EXTRA_SYMBOLS="$(QCOM_MSM_VENDOR_IRIS_EXTRA_SYMBOLS)" \
		$(QCOM_MSM_VENDOR_KCONFIG) \
		modules
endef

define Build/Compile/cdsp-nsp
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
		$(KERNEL_MAKE_FLAGS) \
		M="$(PKG_BUILD_DIR)/drivers/remoteproc" \
		KCFLAGS="$(QCOM_CDSP_NSP_VENDOR_KCFLAGS)" \
		CONFIG_QCOM_PIL_INFO=m CONFIG_QCOM_RPROC_COMMON=m \
		CONFIG_QCOM_Q6V5_COMMON=m CONFIG_QCOM_Q6V5_ADSP=m \
		CONFIG_QCOM_Q6V5_PAS=m CONFIG_QCOM_SYSMON=m modules
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
		$(KERNEL_MAKE_FLAGS) \
		M="$(PKG_BUILD_DIR)/drivers/rpmsg" \
		KCFLAGS="$(QCOM_CDSP_NSP_VENDOR_KCFLAGS)" \
		CONFIG_RPMSG_QCOM_GLINK_SMEM=m modules
	+$(MAKE) $(PKG_JOBS) -C "$(LINUX_DIR)" \
		$(KERNEL_MAKE_FLAGS) \
		M="$(PKG_BUILD_DIR)/drivers/misc" \
		KCFLAGS="$(QCOM_CDSP_NSP_VENDOR_KCFLAGS)" \
		CONFIG_QCOM_FASTRPC=m modules
endef

define Build/Compile
	$(if $(CONFIG_PACKAGE_kmod-qcom-msm-vendor-drm),$(call Build/Compile/drm-helpers))
	$(if $(CONFIG_PACKAGE_kmod-qcom-msm-vendor-drm),$(call Build/Compile/msm))
	$(if $(CONFIG_PACKAGE_kmod-qcom-iris-vendor),$(call Build/Compile/iris))
	$(if $(CONFIG_PACKAGE_kmod-qcom-cdsp-nsp-vendor),$(call Build/Compile/cdsp-nsp))
endef

$(eval $(call KernelPackage,qcom-msm-vendor-drm))
$(eval $(call KernelPackage,qcom-iris-vendor))
$(eval $(call KernelPackage,qcom-cdsp-nsp-vendor))
