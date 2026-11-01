/*
 * Driver for MediaTek MT753x gigabit switch
 *
 * Copyright (C) 2018 MediaTek Inc. All Rights Reserved.
 *
 * Author: Sirui Zhao <Sirui.Zhao@mediatek.com>
 *
 * SPDX-License-Identifier:	GPL-2.0+
 */

#ifndef _MT753X_NL_H_
#define _MT753X_NL_H_

#define MT753X_GENL_NAME		"mt753x"
#define MT753X_GENL_VERSION		0x1

enum mt753x_cmd {
	MT753X_CMD_UNSPEC = 0,
	MT753X_CMD_REQUEST,
	MT753X_CMD_REPLY,
	MT753X_CMD_READ,
	MT753X_CMD_WRITE,
	MT753X_CMD_NOTIFY,

	__MT753X_CMD_MAX,
};

enum mt753x_attr {
	MT753X_ATTR_TYPE_UNSPEC = 0,
	MT753X_ATTR_TYPE_MESG,
	MT753X_ATTR_TYPE_PHY,
	MT753X_ATTR_TYPE_DEVAD,
	MT753X_ATTR_TYPE_REG,
	MT753X_ATTR_TYPE_VAL,
	MT753X_ATTR_TYPE_DEV_NAME,
	MT753X_ATTR_TYPE_DEV_ID,

	__MT753X_ATTR_TYPE_MAX,
};

enum mt753x_multicast_groups {
	MT753X_MCGRP_PORT_STATUS,
	__MT753X_MCGRP_MAX
};

#define MT753X_MULTICAST_GROUP_PORT_STATUS		"port_status"
#define MT753X_NR_ATTR_TYPE		(__MT753X_ATTR_TYPE_MAX - 1)

#ifdef __KERNEL__
int __init mt753x_nl_init(void);
void __exit mt753x_nl_exit(void);
#endif /* __KERNEL__ */
int mt753x_nl_port_status_notify(int port, int status);

#endif /* _MT753X_NL_H_ */
