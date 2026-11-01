# SPDX-License-Identifier: GPL-2.0-or-later

. /lib/functions.sh

case "$(uname -r)" in
	6.18|6.18.*)
		case "$(board_name)" in
			comtrend,vr-3032u) . /lib/bmips/6.18/bcm63268/lib/upgrade/platform.sh ;;
			netgear,dgnd3700-v2) . /lib/bmips/6.18/bcm6362/lib/upgrade/platform.sh ;;
			*) . /lib/bmips/legacy/nand/lib/upgrade/platform.sh ;;
		esac
		;;
	*) . /lib/bmips/legacy/nand/lib/upgrade/platform.sh ;;
esac
