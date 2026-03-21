#
# Copyright (C) 2015-2016 OpenWrt.org
#
# This is free software, licensed under the GNU General Public License v3.
#

include $(TOPDIR)/rules.mk

PKG_NAME:=v2dat
PKG_VERSION:=0.1.3
PKG_RELEASE:=1

PKG_SOURCE:=$(PKG_NAME)-$(PKG_VERSION).tar.gz
PKG_SOURCE_URL:=https://codeload.github.com/kenzok8/v2dat/tar.gz/v$(PKG_VERSION)?
PKG_HASH:=301e6c96266a512d5f0caa143c1128ac8b5b9c4bfbf96ec0c7027cc7aa5ce865
PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)-$(PKG_VERSION)

PKG_LICENSE:=GPL-3.0
PKG_LICENSE_FILE:=LICENSE
PKG_MAINTAINER:=sbwml <admin@cooluc.com>

PKG_BUILD_DEPENDS:=golang/host
PKG_BUILD_PARALLEL:=1
PKG_USE_MIPS16:=0
PKG_BUILD_FLAGS:=no-mips16

GO_PKG:=github.com/kenzok8/v2dat

include $(INCLUDE_DIR)/package.mk
include $(TOPDIR)/feeds/packages/lang/golang/golang-package.mk

define Package/v2dat
  SECTION:=utils
  CATEGORY:=Utilities
  TITLE:=V2ray geo/ip data unpack tools
  URL:=https://github.com/kenzok8/v2dat
  DEPENDS:=$(GO_ARCH_DEPENDS)
endef

define Package/v2dat/install
	$(call GoPackage/Package/Install/Bin,$(1))
endef

$(eval $(call GoBinPackage,v2dat))
$(eval $(call BuildPackage,v2dat))
