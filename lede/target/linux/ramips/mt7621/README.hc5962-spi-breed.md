# HiWiFi HC5962, 16 MiB SPI NOR / Breed

> The hardware observations below describe the original QWRT vendor build.
> This LEDE port uses mainline mt76 wireless and per-board OpenWrt calibration
> bindings; the original vendor wireless test results do not validate this port.

Select `HiWiFi HC5962 16M SPI (Breed)` (`hiwifi_hc5962-spi-breed`).
The existing `hiwifi_hc5962` profile is for NAND and is not suitable for
this SPI conversion.

## Flash layout

The test unit reports a Winbond W25Q128, 16 MiB SPI NOR and 256 MiB RAM.
Breed reports its own board build as Newifi D1. Read-only flash dumps
confirmed MT7603 calibration at `0x40000`, MT76x2 calibration at `0x48000`,
and a legacy uImage header at `0x50000`.

| Partition | Offset | Size |
| --- | --- | --- |
| bootloader (read-only, including reserved boot settings) | 0x000000 | 0x040000 |
| factory (read-only) | 0x040000 | 0x010000 |
| firmware | 0x050000 | 0xfb0000 |

The bootloader region is deliberately not subdivided into an assumed
U-Boot environment: this Breed reports that its environment is unavailable.
NAND is disabled, and no UBI concatenation or NAND upgrade path is used.

The PCI EEPROM cells point to factory offsets `0` and `0x8000`; vendor
drivers use eeprom-api to read the actual calibration. Ethernet takes its
base MAC from factory offset `4`; LAN uses that address and WAN uses base + 1.
The default 2.4 GHz and 5 GHz radio addresses use base + 2 and base + 3,
respectively, since this board's two EEPROM blocks contain the same MAC.
These are runtime driver overrides; calibration stays unchanged and an
explicitly configured radio MAC is preserved.

The vendor switch uses ports 1/2/3 for LAN and port 4 for WAN, with VLANs
on GMAC0 (`eth0.1` / `eth0.2`). Both GMAC0 and switch port 6 use RGMII,
matching the original HC5962 CPU link. TRGMII on the test unit produced
CPU-port CRC/Jabber errors and Ethernet receive errors. GMAC1 is disabled.
The converted test unit boots and is reachable over wired Ethernet with
RGMII; observed Ethernet receive errors remain zero. Complete physical
port-label validation is still pending.

## Installation

Use the generated `hiwifi_hc5962-spi-breed-squashfs-sysupgrade.bin` as a
firmware image in Breed, with firmware starting at `0x50000`. This is not
a full-flash image; preserve the bootloader and factory regions. Do not
restore settings from the NAND profile. Subsequent upgrades from this
SPI profile use the normal SPI NOR sysupgrade path.

DTB checks and a successful build do not establish hardware acceptance.
Verify SPI detection, root filesystem mount, LAN/WAN port mapping,
wireless EEPROM reads, and absence of kernel faults after flashing.

## Wireless validation limits

The MT7603/MT76x2 status integration reports the radio model, AP mode,
transmit power and legacy station RSSI/rates without treating an unused
RSSI chain as the client signal. Noise remains unknown when not supplied
by the driver.

The test unit still has unresolved 5 GHz performance problems at 80 MHz.
Local WAN-to-Wi-Fi download samples at 40 MHz reached approximately
158-164 Mbps with 11-13% aggregate CPU use. VHT40 is a temporary setting
on that test unit, not a firmware-wide default or a completed driver fix.
