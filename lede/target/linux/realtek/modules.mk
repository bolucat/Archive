#
# Copyright (C) 2011 OpenWrt.org
#
# This is free software, licensed under the GNU General Public License v2.
# See /LICENSE for more information.
#

MFD_MENU:=MultiFunction Device (MFD) Support
WATCHDOG_MENU:=Watchdog Timer Support

define KernelPackage/hasivo-mcu-wdt
  SUBMENU:=$(WATCHDOG_MENU)
  TITLE:=Hasivo MCU watchdog driver
  KCONFIG:=CONFIG_HASIVO_MCU_WATCHDOG
  FILES:=$(LINUX_DIR)/drivers/watchdog/hasivo-mcu-wdt.ko
  DEPENDS:=@TARGET_realtek +kmod-mfd-hasivo-stc8
  AUTOLOAD:=$(call AutoLoad,20,hasivo-mcu-wdt,1)
endef

define KernelPackage/hasivo-mcu-wdt/description
 Hardware watchdog driver for the external management MCU found on
 Hasivo / Horaco network switches. Registers a Linux watchdog device;
 the kernel watchdog core feeds it automatically via its own timer.
endef

$(eval $(call KernelPackage,hasivo-mcu-wdt))

define KernelPackage/mfd-hasivo-stc8
  SUBMENU:=$(MFD_MENU)
  TITLE:=Hasivo STC8 microcontroller support
  KCONFIG:=CONFIG_MFD_HASIVO_STC8
  FILES:=$(LINUX_DIR)/drivers/mfd/hasivo-stc8-mfd.ko
  DEPENDS:=@TARGET_realtek +kmod-mfd +kmod-regmap-i2c
  AUTOLOAD:=$(call AutoLoad,19,hasivo-stc8-mfd,1)
endef

define KernelPackage/mfd-hasivo-stc8/description
 Kernel module for Hasivo STC8 microcontroller support
endef

$(eval $(call KernelPackage,mfd-hasivo-stc8))

define KernelPackage/hasivo-mcu-sensor
  SUBMENU:=$(HWMON_MENU)
  TITLE:=Hasivo management MCU sensor driver
  KCONFIG:=CONFIG_SENSORS_HASIVO_MCU
  FILES:=$(LINUX_DIR)/drivers/hwmon/hasivo-mcu-sensor.ko
  DEPENDS:=@TARGET_realtek +kmod-mfd-hasivo-stc8
  AUTOLOAD:=$(call AutoProbe,hasivo-mcu-sensor,1)
endef

define KernelPackage/hasivo-mcu-sensor/description
 sensor driver for the Hasivo management MCU (CPU/system
 temperatures and fan control) on Hasivo / Horaco network switches.
 A child of the hasivo MCU MFD.
endef

$(eval $(call KernelPackage,hasivo-mcu-sensor))

# Linux 6.18 PoE drivers used by the current Realtek device profiles.
PSE_MENU:=PSE-PD / PoE support

define KernelPackage/pse-pd
  SUBMENU:=$(PSE_MENU)
  TITLE:=PSE-PD Support
  DEPENDS:=@TARGET_realtek @LINUX_6_18
  KCONFIG:=CONFIG_REGULATOR=y CONFIG_PSE_CONTROLLER=y
endef

define KernelPackage/pse-pd/description
 Kernel module for PSE-PD support.
endef

$(eval $(call KernelPackage,pse-pd))

define AddDepends/pse-pd
  SUBMENU:=$(PSE_MENU)
  DEPENDS+=+kmod-pse-pd $(1)
endef

define KernelPackage/pse-realtek-mcu
  SUBMENU:=$(PSE_MENU)
  TITLE:=Realtek/Broadcom PSE MCU driver (core)
  KCONFIG:=CONFIG_PSE_REALTEK_MCU
  DEPENDS:=@TARGET_realtek @LINUX_6_18
  FILES:=$(LINUX_DIR)/drivers/net/pse-pd/realtek-pse-mcu-core.ko
  $(call AddDepends/pse-pd)
endef

define KernelPackage/pse-realtek-mcu/description
 Library module shared by the drivers for the Realtek/Broadcom PSE MCU
 (kmod-pse-realtek-mcu-i2c, kmod-pse-realtek-mcu-uart).
 Pulled in automatically when one of the transport modules is selected.
endef

$(eval $(call KernelPackage,pse-realtek-mcu))

define KernelPackage/pse-realtek-mcu-i2c
  SUBMENU:=$(PSE_MENU)
  TITLE:=Realtek/Broadcom PSE MCU driver (I2C transport)
  KCONFIG:=CONFIG_PSE_REALTEK_MCU_I2C
  DEPENDS:=@TARGET_realtek @LINUX_6_18 +kmod-i2c-core +kmod-pse-realtek-mcu
  FILES:=$(LINUX_DIR)/drivers/net/pse-pd/realtek-pse-mcu-i2c.ko
  AUTOLOAD:=$(call AutoProbe,realtek-pse-mcu-i2c)
  $(call AddDepends/pse-pd)
endef

define KernelPackage/pse-realtek-mcu-i2c/description
 Realtek/Broadcom PSE MCU driver, widely used on Realtek-based switches.
 Attached via I2C/SMBus.
endef

$(eval $(call KernelPackage,pse-realtek-mcu-i2c))

define KernelPackage/pse-realtek-mcu-uart
  SUBMENU:=$(PSE_MENU)
  TITLE:=Realtek/Broadcom PSE MCU driver (UART transport)
  KCONFIG:= \
	CONFIG_PSE_REALTEK_MCU_UART \
	CONFIG_SERIAL_DEV_BUS=y \
	CONFIG_SERIAL_DEV_CTRL_TTYPORT=y
  DEPENDS:=@TARGET_realtek @LINUX_6_18 +kmod-pse-realtek-mcu
  FILES:=$(LINUX_DIR)/drivers/net/pse-pd/realtek-pse-mcu-uart.ko
  AUTOLOAD:=$(call AutoProbe,realtek-pse-mcu-uart)
  $(call AddDepends/pse-pd)
endef

define KernelPackage/pse-realtek-mcu-uart/description
 Realtek/Broadcom PSE MCU driver, widely used on Realtek-based switches.
 Attached via UART.
endef

$(eval $(call KernelPackage,pse-realtek-mcu-uart))

define KernelPackage/pse-hasivo-hs104
  SUBMENU:=$(PSE_MENU)
  TITLE:=Hasivo HS104 PSE controller support
  KCONFIG:=CONFIG_PSE_HASIVO_HS104
  DEPENDS:=@TARGET_realtek @LINUX_6_18 +kmod-i2c-core
  FILES:=$(LINUX_DIR)/drivers/net/pse-pd/hasivo_hs104.ko
  AUTOLOAD:=$(call AutoProbe,hasivo_hs104)
  $(call AddDepends/pse-pd)
endef

define KernelPackage/pse-hasivo-hs104/description
 Kernel module for the Hasivo HS104 PoE PSE controller chips.
 Supports the HS104PTI/HS104PBI single-chip PoE PSE controllers
 managing 4 delivery channels for 802.3af/at/bt power over I2C.
endef

$(eval $(call KernelPackage,pse-hasivo-hs104))
