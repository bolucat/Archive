# H3C TX180x Plus (U-Boot mod)

> The hardware observations below describe the original QWRT vendor build.
> This LEDE port uses mainline mt76 wireless and per-board OpenWrt calibration
> bindings; the original vendor wireless test results do not validate this port.

Select `h3c_tx180x-plus-uboot-mod` for the modified bootloader that boots a
legacy LZMA uImage directly from the kernel partition. This is separate from
H3C's stock bootloader profiles, which use a prefixed FIT image.

The inspected TX1800 Plus has 256 MiB RAM, 128 MiB NAND (2 KiB pages,
128 KiB eraseblocks), and an existing NMBM table. Keep its bootloader,
environment and factory calibration partitions. The kernel starts at 0x200000
and occupies 8 MiB; UBI starts at 0xa00000 and occupies 110 MiB. The final
8 MiB is reserved for NMBM. NMBM is built into the kernel; no forced table
creation is enabled.

The vendor MT753x switch uses TRGMII on CPU port 6. VLAN 1 is LAN ports 1/2/3
plus CPU 6t (`eth0.1`); VLAN 2 is WAN port 4 plus CPU 6t (`eth0.2`). HNAT uses
these same interfaces and one GMAC. WAN takes the U-Boot `ethaddr`, LAN adds
one. MT7915 uses the existing vendor DBDC package and EEPROM integration;
wireless and offload performance are not first-boot acceptance criteria.

## Modified U-Boot HTTP recovery

Use the `h3c_tx180x-plus-uboot-mod-squashfs-factory-ubi.bin` image in the
modified U-Boot HTTP firmware updater, following the original mt76xx
`Device/h3c_tx1800-plus` format. It contains a legacy uImage at offset zero,
padding to 8 MiB, then UBI (128 KiB eraseblocks, 2 KiB pages). This is the
firmware partition payload; it does not contain a bootloader, environment,
factory EEPROM, or an NMBM management region. Use the bootloader's firmware
update function, not a whole-NAND/bootloader write operation.

## Upgrade from the inspected 4.4 QWRT firmware

Its board name is `tx1800-plus`. This image's metadata accepts that legacy
name, and its NAND upgrade handler reads the kernel/root members of the
sysupgrade tar regardless of the directory's profile name. Legacy sibling
names `tx1801-plus` and `tx1806-plus` are included for the same modified
bootloader/flash layout; those models have not been physically inspected.
Do not use this image for a stock bootloader based only on the product name.

Copy the `h3c_tx180x-plus-uboot-mod-squashfs-sysupgrade.bin` image to `/tmp`, then:

```sh
sysupgrade -T /tmp/tx180x-uboot-mod.bin
sysupgrade -n /tmp/tx180x-uboot-mod.bin
```

The second command flashes and reboots. Do not retain settings or force an
image that fails validation. The old firmware's metadata checker does not
support enforcing configuration compatibility versions, so `-n` must be
supplied explicitly (or disable keeping settings in the web UI).

A successful image check does not prove physical NAND boot or switch operation.
The current management address was a WAN DHCP lease; after upgrading, discover
the new lease or connect to LAN using the image's default management address.

## Validation

The 6.18.51 kernel build enables `CONFIG_MTD_NAND_MTK_BMT=y` and links
`mtk_bmt_attach`, `mtk_bmt_init_nmbm`, and `mtk_bmt_nmbm_ops` into `vmlinux`.
The new Kconfig/Makefile patch applies without fuzz. MIPS QEMU userspace
execution of the board setup produces the LAN/WAN VLANs listed above.
Neither check substitutes for reading the existing NMBM table and exercising
physical ports after a real boot.

The complete firmware build passed with `-j30`. The generated sysupgrade
kernel is 3,932,160 bytes (below the 8 MiB limit), has valid legacy uImage
header/data CRCs, and decompresses to a kernel containing NMBM code plus the
new profile's DTB. The inspected TX1800 Plus running 4.4 accepted the uploaded
image with `sysupgrade -T -n` (exit 0), and its SHA-256 matched the local image.
That check did not flash or reboot the device.

## First hardware boot and NAPI fix

The first hardware boot log confirms that Linux 6.18.51 reads the existing
NMBM signature at block 1023, loads both tables at blocks 960/963, attaches
NMBM, and mounts the 110 MiB UBI device with 880 good PEBs and no bad or
corrupted PEBs. It creates the rootfs ubiblock device before Ethernet probe.

That image then panicked in `netif_napi_dev_list_add()` during `mtk_probe()`.
The legacy embedded dummy netdev was passed to `init_dummy_netdev()`, which
no longer initializes the NAPI list and device lock in 6.18. The Ethernet
driver now uses `alloc_netdev_dummy()` and managed `free_netdev()` cleanup;
remove still deletes both NAPIs before releasing the dummy netdev.

A MIPS Malta QEMU test on the same 6.18 network core completed 16 cycles of
dummy netdev allocation, two NAPI registrations with the driver's weight 256,
enable/disable, deletion and release (module load exit 0). The weight warning
also occurs in this passing test; it was not the cause of the panic.
The fixed image still requires another hardware boot and wired traffic test.

## Second hardware boot

The next boot reached userspace and wired management. Link-up on ports 3/4
reported WARN stacks in `cancel_delayed_work_sync()`: the legacy hotplug code
cancelled zero-initialized delayed work before its first initialization.
Initialize once before cancellation, use correctly typed work callbacks,
serialize event updates, and reject an index equal to the port-array size.
The external-PHY polling work also gets static initialization. QEMU exercises
the actual hotplug function's first use, repeated requeue and flush paths.

The running firewall initially selected nftables flow offload and disabled
the vendor HNAT hook despite the module having started. MT7621 now installs
the existing vendor-HNAT defaults (`flow_offloading=3`, hardware-flow flag 0).
On the test router, changing to that mode and reloading the firewall removed
the nftables flowtable and read back `value=1, hook is enabled now!`.
PPE BIND/throughput acceptance is deferred until a LAN-to-WAN traffic test.

The ramips 6.18 mtdblock patch suppresses the NAND-to-UBI suggestion for every
partition, as requested. It does not suppress actual I/O/ECC/bad-block errors.

## Wireless, restart and status follow-up (2026-10-23)

`mtwifi_service` now depends on `ralink-wifi-base` for MT7621, providing the
missing netifd wireless protocol scripts. MT7915 private queries report the
operational bandwidth, PHY mode and security bitmaps to iwinfo while retaining
the legacy 8-byte security response. iwinfo resolves DBDC interface sections,
recognizes the MT7915 profile and no longer reports transmit power as RSSI.
Live results: ra0 HE40, rax0 HE80, both radios up, correct open-network status.

An orphaned odhcp6c retained `odhcp6c.eth0.2` across netifd re-exec, causing new
clients to fail ubus registration and retry continuously. The re-exec path now
kills tracked children before discarding process state. Both init-service
restart and `ubus call network restart` recovered on hardware with one DHCPv6
client and no repeated Invalid argument errors.

HNAT's cached PPD device held eth0.1 through VLAN deletion. The netdevice
unregister notifier now clears and releases it after network RCU readers
finish; NETDEV_UP reacquires a recreated device. Debugfs replacement is
serialized with RTNL. Three explicit VLAN create/bind/delete cycles passed.
Probe/remove use a mutex because coherent DMA allocation/free and PPE waits
may sleep. MT7915 uses eth_hw_addr_set() on modern kernels to maintain the
netdevice address shadow across interface reopen.

The read-only `/sys/kernel/debug/hnat/hnat_stats` implements the MT798x-style
PPE_NUM/ALL_PPE0/BIND_PPE0 ABI for the single MT7621 PPE. BIND counting is fixed
to the hardware BIND state, independent of the debug entry filter. The MT7621
`/sbin/usage` script exposes it beside CPU usage in LuCI. The live table has
16384 slots; BIND traffic/throughput acceptance remains deferred.

Companion LuCI feed changes in `modules/luci-base/root/usr/share/rpcd/ucode/luci`
hide mainline hardware offload on vendor-HNAT MT7621 and accept both RxByte and
RxGoodByte plus Uni/UniCast MIB names. WAN and LAN2 now have nonzero receive
counters. swconfig's new `showports` command skips the VLAN table; live command
time fell from 1.85 seconds to 0.04 seconds. LuCI falls back to `show` when used
with older swconfig versions.

## Channel selection and H3C wireless MACs

MT7621 uses the standard 32-bit WEXT bitrate array layout. Enable iwinfo's
WEXT32 ABI support so it reads `iw_range.num_frequency` at the correct offset.
MT7915 now exports the active band's driver channel table rather than a fixed
list selected by interface name. On the CN test configuration, live queries
return channels 1-13 and 36-64/149-165.

The H3C TX1800 Plus, TX1801 Plus, TX1806 and U-Boot-mod profiles derive wireless
MACs from `u-boot-env:ethaddr`, matching the old mt76xx board setup: add two,
set the locally administered bit, then replace the high nibble of byte four
with 1 (2.4 GHz) or 7 (5 GHz). Explicit UCI overrides are preserved. The mtwifi
startup script applies this mapping even when mtwifi_service has already
created the wireless configuration. EEPROM calibration contents are untouched.

MT7915 DBDC wdev address updates now use RtmpOSNetDevAddrSet(), covering the
remaining direct writes to netdev->dev_addr. Firmware metadata uses bounded
single-record string/hex formatting instead of one printk per character.

For the switch configuration page, `swconfig showlinks` and LuCI's optional
`linkonly` RPC argument skip unused MIB counters. Live RPC time fell from
0.59 s to 0.06 s; the browser switch table rendered in about 1.7 s rather than
2.5 s in the measured session. The overview retains full traffic counters.

## Clean sysupgrade hardware acceptance (2026-10-23)

The sysupgrade image with SHA-256
`46ac0e35e511c6badee44d1dbd40662dc1e869ba08d8cdc62c2982c4625cb949`
passed header/data CRC, rootfs content comparison, device-side checksum and
`sysupgrade -T`. It was flashed with old configuration discarded. A separate
one-file defaults archive added only TCP management access from the authorized
workstation; that test rule is not part of the distributable image.

The fresh boot attached NMBM and UBI, obtained the WAN lease, generated both
H3C wireless MACs, and enabled vendor HNAT. Browser checks confirmed both
channel selectors. Applying channels 6 and 149 worked; restoring auto selected
1 and 36. Init-service network restart returned zero and recovered WAN plus
both radios with correct MACs after wireless setup completed. The inspected
boot/reload/restart logs contained no kernel WARNING, Oops, panic, unregister
wait or repeated wan6 ubus Invalid argument errors. Only one odhcp6c remained.
The switch page rendered in about 1.65 s. At this checkpoint PPE BIND remained zero without a LAN-to-WAN workload;
the subsequent wired validation below supersedes that pending result. RF
performance acceptance remains open.


## Wired HNAT and TCP window validation (2026-10-23)

A USB Ethernet adapter connected to LAN3 negotiates 1 Gbit/s. A separate
macvlan container is the LAN client; iperf3 runs on the workstation's WAN-side
interface. This prevents local host routing from bypassing the router. The
router performs IPv4 NAT in both forward and reverse tests.

Initially, four TCP streams created four one-way BIND entries but stalled at
less than 0.2 Mbit/s. Disabling HNAT restored software forwarding. The MT798x
optional TCP-window patch identified the missing compatibility setting:
Linux 6.18 has no nf_conntrack_tcp_no_window_check sysctl in this target, but
its existing per-netns nf_conntrack_tcp_be_liberal control accepts packets
when hardware forwarding has left software TCP window tracking stale.
An on/off/on diagnostic reproduced 0.26 versus 940.65 Mbit/s with HNAT enabled.

MT7621 now installs `/etc/sysctl.d/30-mt7621-hnat.conf` with
`net.netfilter.nf_conntrack_tcp_be_liberal=1`. This uses the existing kernel
interface instead of adding a second window-check bypass. The same file was
installed on the test router; the normal sysctl startup service reapplies it.

Each measurement runs for 20 seconds after a 3-second warm-up. CPU busy time
comes from /proc/stat deltas over the steady interval, normalized across four
logical CPUs. Directions are relative to the LAN client.

| Mode | TCP streams | Direction | Mbit/s | Average CPU busy |
| --- | ---: | --- | ---: | ---: |
| HNAT off | 4 | upload | 884.2 | 43.4% |
| HNAT off | 4 | download | 941.5 | 33.1% |
| HNAT on, window compatibility | 4 | upload | 941.4 | 1.7% |
| HNAT on, window compatibility | 4 | download | 941.0 | 1.1% |
| HNAT on, window compatibility | 1 | upload | 933.5 | 1.1% |
| HNAT on, window compatibility | 1 | download | 939.1 | 1.2% |

The corrected four-stream tests show eight bidirectional BIND records with
matching original/NAT tuples, zero TCP retransmits, and near-line-rate traffic.
The hottest logical CPU was about 94% busy with software forwarding. Kernel
and serial logs contain no panic/Oops/WARNING during testing. HNAT and the
persistent window compatibility setting are enabled after testing.

The full firmware rebuild passed and the sysctl file was verified inside the
sysupgrade rootfs (SHA-256
`3b148982eb624c9f8373a4b23766c8cf3094ac7a0df9371a9855cf55109527ca`).
The router received the equivalent configuration file directly; this follow-up
image has not been reflashed. This validates wired IPv4 TCP NAT, not IPv6,
PPPoE, wireless offload or RF performance.
