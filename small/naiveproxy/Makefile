# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2021 ImmortalWrt.org

include $(TOPDIR)/rules.mk

PKG_NAME:=naiveproxy
PKG_VERSION:=147.0.7727.49-2
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
  PKG_HASH:=bfac7e9345c4dc56835e718c06dbf9efad3357eb5641321c860c8b84a95c1189
else ifeq ($(ARCH_PACKAGES),aarch64_generic)
  PKG_HASH:=34e1c30fd757f34d268866a2b7dd5904252e1003636777c3bbd13a6b8c7f52ae
else ifeq ($(ARCH_PACKAGES),arm_arm1176jzf-s_vfp)
  PKG_HASH:=82ea9fa5987cae82322e569225e1e7ff0c4b6f0ae8a7be67d5b1f79290d56a4b
else ifeq ($(ARCH_PACKAGES),arm_arm926ej-s)
  PKG_HASH:=168c94485dece117003396c92af610cd5053add7a417936471662af7e65bb59c
else ifeq ($(ARCH_PACKAGES),arm_cortex-a15_neon-vfpv4)
  PKG_HASH:=8c5b98147aecdb26e1c2e3ac21baa9a9b204b46c091e56640d2ea38b78228f6b
else ifeq ($(ARCH_PACKAGES),arm_cortex-a5_vfpv4)
  PKG_HASH:=b6568b2d2ccc5e55d0becc20b2da6c40a2266889f8dd676ebfaa959476a0bdec
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7)
  PKG_HASH:=c03b7d12423287befd0244f3be0a4f6e67f28ddfda592562d73e13bd5913c074
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7_neon-vfpv4)
  PKG_HASH:=a681bc4ace91d5f22cc559d23bd4d7b6f6f49b6bbfc576a93edbbe6c57f63b1e
else ifeq ($(ARCH_PACKAGES),arm_cortex-a7_vfpv4)
  PKG_HASH:=4fee51ec703dadf3ad4e4ef0273d5ef61124d4c41f03b90c0ad2af48c4546734
else ifeq ($(ARCH_PACKAGES),arm_cortex-a8_vfpv3)
  PKG_HASH:=6a491f6159b06cb471e5696339bb285012bd6b1bb832dba2ae229c21843052da
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9)
  PKG_HASH:=0e7783788bf7d7813fc1465d5d97f4479e2f269da98eae7d925eec94a5816be0
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9_neon)
  PKG_HASH:=ee6c8f1dd49a52f49b057c47a78dc3f30fa2b7145fd4881f7b96ef12bec3931d
else ifeq ($(ARCH_PACKAGES),arm_cortex-a9_vfpv3-d16)
  PKG_HASH:=e80c0309c62572ca3e90eb1c827c0568c27efc3ebceb6d1defe3f91e01564253
else ifeq ($(ARCH_PACKAGES),arm_mpcore)
  PKG_HASH:=7169c287308d886716fdbbcc08942fbd403ad7babcbc9c7664fcec7e574bf9d7
else ifeq ($(ARCH_PACKAGES),arm_xscale)
  PKG_HASH:=519bfce3af71d35e9087c2296645313763063ffffc87b404f5ee1a7685f39c4b
else ifeq ($(ARCH_PACKAGES),mipsel_24kc)
  PKG_HASH:=2ab647277df7807e1b0b0108ccdabcc2538ac4f703ad1f5c4a23d89656a568cb
else ifeq ($(ARCH_PACKAGES),mipsel_mips32)
  PKG_HASH:=1604181bea6a1b77a318341442ee878ac1aa1b4a760f633061dc13a80bf3ce23
else ifeq ($(ARCH_PACKAGES),riscv64)
  PKG_HASH:=cb052933fab1b89924e8b1fba754a983cbc172e32caa0c70ef703ea801093fd5
else ifeq ($(ARCH_PACKAGES),x86)
  PKG_HASH:=8970a9618a4b42eeaf5a265efd73ee167c539da7c2f69a07ebb5271e9f44c95c
else ifeq ($(ARCH_PACKAGES),x86_64)
  PKG_HASH:=ef7aca95e59a796a6955630ca6cd4593432c827e00a84b6b8b03d221f9d17892
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
