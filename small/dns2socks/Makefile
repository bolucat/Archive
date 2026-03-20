# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2021-2026 ImmortalWrt.org

include $(TOPDIR)/rules.mk

PKG_NAME:=dns2socks
PKG_VERSION:=2.2
PKG_RELEASE:=1

PKG_SOURCE:=$(PKG_NAME)-$(PKG_VERSION).tar.gz
PKG_SOURCE_URL:=https://codeload.github.com/kenzok8/dns2socks/tar.gz/v$(PKG_VERSION)?
PKG_HASH:=790f7ca5ec07677a699121eb071c9e66cdafe65a22bb229eed7fd3cf215631d2
PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)-$(PKG_VERSION)

PKG_LICENSE:=BSD-3-Clause
PKG_LICENSE_FILE:=LICENSE
PKG_MAINTAINER:=ghostmaker

include $(INCLUDE_DIR)/package.mk

define Package/dns2socks
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=IP Addresses and Names
  TITLE:=DNS to SOCKS or HTTP proxy
  URL:=https://github.com/kenzok8/dns2socks
  DEPENDS:=+libpthread
endef

define Package/dns2socks/description
  This is a command line utility to resolve DNS requests via
  a SOCKS tunnel like Tor or a HTTP proxy.
endef

define Build/Compile
	$(TARGET_CC) \
	$(TARGET_CFLAGS) \
	$(TARGET_CPPFLAGS) \
	$(FPIC) \
	-o $(PKG_BUILD_DIR)/dns2socks \
	$(PKG_BUILD_DIR)/src/dns2socks.c \
	$(TARGET_LDFLAGS) -pthread
endef

define Package/dns2socks/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/dns2socks $(1)/usr/bin/dns2socks
endef

$(eval $(call BuildPackage,dns2socks))
