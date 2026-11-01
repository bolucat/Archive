. /lib/upgrade/common.sh

glinet_get_fit_part() {
	local image="$1"
	local name="$2"

	dumpimage -l "$image" 2>/dev/null | \
		sed -n "s/^[[:space:]]*Image \([0-9][0-9]*\) ($name)$/\1/p"
}

glinet_emmc_check_image() {
	[ "$(identify_magic_long "$(get_magic_long "$1")")" != "fit" ] && return 0

	local hlos=$(glinet_get_fit_part "$1" hlos)
	local rootfs=$(glinet_get_fit_part "$1" rootfs)
	[ -n "$hlos" ] && [ -n "$rootfs" ] && return 0

	v "The factory image must contain hlos and rootfs sections."
	return 1
}

glinet_emmc_flash_fit_part() {
	local image="$1"
	local part="$2"
	local device="$3"
	local name="$4"
	local output="/tmp/glinet-emmc-$name.bin"
	local ret

	v "Extracting and flashing $name to $device"
	rm -f "$output"
	dumpimage -T flat_dt -p "$part" -o "$output" "$image" &&
		[ -s "$output" ] && dd if="$output" of="$device" bs=1M conv=fsync
	ret=$?
	rm -f "$output"
	return "$ret"
}

glinet_emmc_do_fit_upgrade() {
	local image="$1"
	local hlos=$(glinet_get_fit_part "$image" hlos)
	local rootfs=$(glinet_get_fit_part "$image" rootfs)
	local wifi_fw=$(glinet_get_fit_part "$image" wifi_fw)
	local hlos_dev=$(find_mmc_part "$CI_KERNPART" "$CI_ROOTDEV")
	local rootfs_dev=$(find_mmc_part "$CI_ROOTPART" "$CI_ROOTDEV")
	local wifi_fw_dev

	[ -n "$wifi_fw" ] && wifi_fw_dev=$(find_mmc_part "0:WIFIFW" "$CI_ROOTDEV")
	[ -n "$hlos" ] && [ -n "$rootfs" ] && \
		[ -n "$hlos_dev" ] && [ -n "$rootfs_dev" ] || {
		v "Unable to find the required GL.iNet image sections or eMMC partitions."
		return 1
	}
	[ -z "$wifi_fw" ] || [ -n "$wifi_fw_dev" ] || {
		v "Unable to find the GL.iNet Wi-Fi firmware partition."
		return 1
	}

	dd if=/dev/zero of="$hlos_dev" bs=512 count=8 conv=fsync || return 1
	glinet_emmc_flash_fit_part "$image" "$rootfs" "$rootfs_dev" rootfs || return 1
	[ -z "$wifi_fw" ] || \
		glinet_emmc_flash_fit_part "$image" "$wifi_fw" "$wifi_fw_dev" wifi_fw || return 1
	glinet_emmc_flash_fit_part "$image" "$hlos" "$hlos_dev" hlos
}

glinet_emmc_do_upgrade() {
	case "$(identify_magic_long "$(get_magic_long "$1")")" in
	fit)
		glinet_emmc_do_fit_upgrade "$1"
		;;
	*)
		emmc_do_upgrade "$1"
		;;
	esac
}
