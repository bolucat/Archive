PART_NAME=firmware

case "$(uname -r)" in
	6.18|6.18.*)
		. /lib/realtek/6.18/upgrade.sh
		;;
	*)
		. /lib/realtek/legacy/upgrade.sh
		;;
esac
