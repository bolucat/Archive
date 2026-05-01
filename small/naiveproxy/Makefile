# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2021 ImmortalWrt.org

include $(TOPDIR)/rules.mk

PKG_NAME:=naiveproxy
PKG_VERSION:=147.0.7727.49-3
PKG_RELEASE:=1

# intel 80386 & riscv64 & cortex-a76
ifeq ($(ARCH_PREBUILT),aarch64_cortex-a76)
  ARCH_PREBUILT:=aarch64_generic
else ifeq ($(ARCH_PREBUILT),i386_pentium-mmx)
  ARCH_PREBUILT:=x86
else ifeq ($(ARCH_PREBUILT),i386_pentium4)
  ARCH_PREBUILT:=x86
else ifeq ($(ARCH_PREBUILT),riscv64_riscv64)
  ARCH_PREBUILT:=riscv64
endif

ifeq ($(ARCH_PACKAGES),aarch64_cortex-a53)
  PKG_HASH:=151f2575bff137c598f021b8849a9379e341f7b7dce2b0580a5447b24af7b0ba
else ifeq ($(ARCH_PACKAGES),aarch64_cortex-a72)
  PKG_HASH:=fbf6eb82c01ce11731723672b028244177496f9b6fd45bf79d5d0b0c566a2f9d
else ifeq ($(ARCH_PACKAGES),aarch64_generic)
  PKG_HASH:=312170452579fc196c7588787ba06568099170ec0252f139954f50b816472867
else ifeq ($(ARCH_PACKAGES),arm_arm1176jzf-s_vfp)
  PKG_HASH:=a0e023a23277e425815d3b6f47038ac81f18c8e84a18f53cde0f508c400c16c0
else ifeq ($(ARCH_PACKAGES),arm_arm926ej-s)
  PKG_HASH:=168c94485dece117003396c92af610cd5053add7a417936471662af7e65bb59c
else ifeq ($(ARCH_PACKAGES),arm_cortex-a15_neon-vfpv4)
  PKG_HASH:=a007af9cbd329f240889c14f63d1000faadb0ba889889e00dbb3144d40d835d6
else ifeq ($(ARCH_PACKAGES),arm_cortex-a5_vfpv4)
  PKG_HASH:=0591942d4c687f7458052b317a53384ae2e233ce69d8cf772ed4ce0a66fdd501
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7)
  PKG_HASH:=5e9841e01591bc7509d6d24fc506cce35b460e5753f60cff892a9bc00bd370b3
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7_neon-vfpv4)
  PKG_HASH:=f65a8d7649ed222262bd8d0368066f0207ff6704b5839062bba007279254bb62
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7_vfpv4)
  PKG_HASH:=4fee51ec703dadf3ad4e4ef0273d5ef61124d4c41f03b90c0ad2af48c4546734
else ifeq ($(ARCH_PACKAGES),arm_cortex-a8_vfpv3)
  PKG_HASH:=6a491f6159b06cb471e5696339bb285012bd6b1bb832dba2ae229c21843052da
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9)
  PKG_HASH:=45def6fb0276352ac1cc8d96da9d00765d4902ad682232b54b96d1b57c4d0eaf
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9_neon)
  PKG_HASH:=5aca0384d3642abd5224483bb0f5996a53c22b4ceeeada08c3dfbb650e4e89bc
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9_vfpv3-d16)
  PKG_HASH:=8349414b8d0686843f5e4dc1323e0e0aa6d5068493866baeb12b6d8202c24217
else ifeq ($(ARCH_PACKAGES),arm_mpcore)
  PKG_HASH:=9dc4d65969b59a62d7f5503fc441256775ae857c4dceeb903e9fbe747bffd96a
else ifeq ($(ARCH_PACKAGES),arm_xscale)
  PKG_HASH:=885c4fbd28c2ef4d8b3ee47c1e4271f41f2a7daf69b38ef360ea6d318fe3b52d
else ifeq ($(ARCH_PACKAGES),mipsel_24kc)
  PKG_HASH:=6daaecd1ae7ad0166e88cdff30a12b7fdc5b304a20ef517636eb15ec18d2b00b
else ifeq ($(ARCH_PACKAGES),mipsel_mips32)
  PKG_HASH:=92e2e00bbc8b85dff2bf6121526608dab463f8c478efa3e681f92e5e440d7799
else ifeq ($(ARCH_PACKAGES),riscv64)
  PKG_HASH:=cb052933fab1b89924e8b1fba754a983cbc172e32caa0c70ef703ea801093fd5
else ifeq ($(ARCH_PACKAGES),x86)
  PKG_HASH:=f31c1880c1f030bbbe4d5c7a82b2d239c558f7c8c9793c547ded3138ba275357
else ifeq ($(ARCH_PACKAGES),x86_64)
  PKG_HASH:=0abe65b927c5a4256590ca09ccf1e17b73f0ceb807ae94e5d649484fe6969b3a
else
  PKG_HASH:=dummy
endif

PKG_SOURCE:=naiveproxy-v$(PKG_VERSION)-openwrt-$(ARCH_PACKAGES).tar.xz
PKG_SOURCE_URL:=https://github.com/klzgrad/naiveproxy/releases/download/v$(PKG_VERSION)/
PKG_BUILD_DIR:=$(BUILD_DIR)/naiveproxy-v$(PKG_VERSION)-openwrt-$(ARCH_PACKAGES)

PKG_LICENSE:=BSD 3-Clause
PKG_LICENSE_FILES:=LICENSE
PKG_MAINTAINER:=sbwml <admin@cooluc.com>

include $(INCLUDE_DIR)/package.mk

define Package/naiveproxy
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=Web Servers/Proxies
  TITLE:=Make a fortune quietly
  URL:=https://github.com/klzgrad/naiveproxy
  DEPENDS:=@(aarch64||arm||i386||mipsel||riscv64||x86_64) +libatomic
endef

define Package/naiveproxy/description
  NaïveProxy uses Chrome's network stack to camouflage traffic with strong
  censorship resistance and low detectability. Reusing Chrome's stack also
  ensures best practices in performance and security.
endef

define Package/naiveproxy/conffiles
/etc/config/naiveproxy
endef

define Build/Compile
endef

define Package/naiveproxy/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/naive $(1)/usr/bin/naive
	$(INSTALL_DIR) $(1)/etc/config $(1)/etc/init.d
	$(INSTALL_CONF) $(CURDIR)/files/naiveproxy.config $(1)/etc/config/naiveproxy
	$(INSTALL_BIN) $(CURDIR)/files/naiveproxy.init $(1)/etc/init.d/naiveproxy
endef

$(eval $(call BuildPackage,naiveproxy))
