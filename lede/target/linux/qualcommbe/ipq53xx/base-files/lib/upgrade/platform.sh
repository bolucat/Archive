REQUIRE_IMAGE_METADATA=1
RAMFS_COPY_BIN='dumpimage'

platform_do_upgrade() {
	case "$(board_name)" in
	jdcloud,re-cs-06|\
	jdcloud,re-cs-08)
		CI_KERNPART="0:HLOS"
		CI_ROOTPART="rootfs"
		emmc_do_upgrade "$1"
		;;
	gl.inet,gl-be9300)
		CI_KERNPART="0:HLOS"
		CI_ROOTPART="rootfs"
		glinet_emmc_do_upgrade "$1" || return 1
		;;
	*)
		echo "Sysupgrade is not supported on your board yet."
		return 1
		;;
	esac
}

platform_check_image() {
	[ "$#" -gt 1 ] && return 1

	case "$(board_name)" in
	jdcloud,re-cs-06|\
	jdcloud,re-cs-08)
		return 0
		;;
	gl.inet,gl-be9300)
		glinet_emmc_check_image "$1"
		;;
	*)
		echo "Sysupgrade is not supported on your board yet."
		return 1
		;;
	esac
}

platform_copy_config() {
	case "$(board_name)" in
	gl.inet,gl-be9300|\
	jdcloud,re-cs-06|\
	jdcloud,re-cs-08)
		emmc_copy_config
		;;
	esac
}
