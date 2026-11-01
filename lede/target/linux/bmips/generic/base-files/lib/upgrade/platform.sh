# SPDX-License-Identifier: GPL-2.0-or-later

. /lib/functions.sh

case "$(uname -r)" in
	6.18|6.18.*)
		case "$(board_name)" in
			comtrend,ar-5315u) . /lib/bmips/6.18/bcm6318/lib/upgrade/platform.sh ;;
			comtrend,ar-5387un) . /lib/bmips/6.18/bcm6328/lib/upgrade/platform.sh ;;
			comtrend,vr-3025u) . /lib/bmips/6.18/bcm6368/lib/upgrade/platform.sh ;;
			huawei,hg556a-b) . /lib/bmips/6.18/bcm6358/lib/upgrade/platform.sh ;;
			*) . /lib/bmips/legacy/generic/lib/upgrade/platform.sh ;;
		esac
		;;
	*) . /lib/bmips/legacy/generic/lib/upgrade/platform.sh ;;
esac
