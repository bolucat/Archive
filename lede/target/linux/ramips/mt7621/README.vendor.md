# MT7621 networking on Linux 6.18

This target retains QWRT's MediaTek vendor Ethernet, MT753x swconfig and
HNAT v1 integration. Mainline MediaTek Ethernet and DSA are disabled for
MT7621; the other ramips subtargets retain their driver selection.

Wireless uses the mainline mt76 drivers and the corresponding OpenWrt
board calibration bindings:

| Radio | Driver | Firmware package |
| --- | --- | --- |
| MT7603 | kmod-mt7603 | Selected by the driver |
| MT7602 / MT7612 | kmod-mt76x2 | Selected by the driver |
| MT7663 | kmod-mt7615e | kmod-mt7663-firmware-ap |
| MT7615 | kmod-mt7615e | kmod-mt7615-firmware |
| MT7915 | kmod-mt7915e | Selected by the driver |

The default authentication daemon is wpad-basic-mbedtls. Vendor mtwifi
profiles and mtwifi_service are not used. Each board keeps its factory
EEPROM partition and offset, radio frequency limits and interface topology.

Image compilation verifies build integration. Physical Ethernet forwarding,
radio calibration and RF operation, and HNAT offload performance require
validation on the corresponding hardware.
