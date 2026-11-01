/*
 * Common part for MediaTek MT753x gigabit switch
 *
 * Copyright (C) 2018 MediaTek Inc. All Rights Reserved.
 *
 * Author: Weijie Gao <weijie.gao@mediatek.com>
 *
 * SPDX-License-Identifier:	GPL-2.0+
 */

#include <linux/kernel.h>
#include <linux/delay.h>

#include "mt753x.h"
#include "mt753x_regs.h"
#include "mt753x_nl.h"

void mt753x_irq_enable(struct gsw_mt753x *gsw)
{
	u32 val;
	int i;

	/* Record initial PHY link status */
	for (i = 0; i < MT753X_NUM_PHYS; i++) {
		val = gsw->mii_read(gsw, i, MII_BMSR);
		if (val & BMSR_LSTATUS)
			gsw->phy_link_sts |= BIT(i);
	}

	val = BIT(MT753X_NUM_PHYS) - 1;

	mt753x_reg_write(gsw, SYS_INT_EN, val);
}

#define PORT_SKB_SIZE	2048
#define WORKQUEUE_DELAY_TIME	300 // 1.3s
#define EVENT_PORT_NUMBER		6

#define PORT_CHECK_DELAY_TIME	1000

struct port_event{
	const char		name[16];
	char			type[16];
	unsigned int		port;
	unsigned int		status;
	char			speed[12];
	char			duplex[8];
	char			action[8];
	struct delayed_work	work;
	struct sk_buff		*skb;
};

static struct port_event event_port[EVENT_PORT_NUMBER]= {0}; //event_port[0]: Port0; event_port[1]: Port1;

static DEFINE_MUTEX(port_event_lock);

static int port_event_add_var(struct port_event *event, int argv,
                              const char *format, ...)
{
	char buf[128];
	char *s;
	va_list args;
	int len;

	if (argv)
		return 0;

	va_start(args, format);
	len = vsnprintf(buf, sizeof(buf), format, args);
	va_end(args);

	if (len >= sizeof(buf))
	{
		printk("buffer size too small\n");
		WARN_ON(1);
		return -ENOMEM;
	}

	s = skb_put(event->skb, len + 1);
	strcpy(s, buf);

	//printk("[hotplug]added variable '%s'\n", s);
	return 0;
}


static void port_link_hotplug(struct work_struct *work)
{
	int ret = 0;
	struct port_event *event = container_of(to_delayed_work(work), struct port_event, work);

	if(in_interrupt())
	    printk("%s:%d in_interrupt!!!!",__FUNCTION__,__LINE__);

	event->skb = alloc_skb(PORT_SKB_SIZE, GFP_KERNEL);

	if (!event->skb)
	    return ;

	ret = port_event_add_var(event, 0, "%s@", event->action);
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "HOME=%s", "/");
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "PATH=%s","/sbin:/bin:/usr/sbin:/usr/bin");
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "SUBSYSTEM=%s", event->type);
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "ACTION=%s", event->action);
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "PORT=%d", event->port);
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "SPEED=%s", event->speed);
	if (ret)
	    goto out_free_skb;

	ret = port_event_add_var(event, 0, "DUPLEX=%s", event->duplex);
	if (ret)
	    goto out_free_skb;
	NETLINK_CB(event->skb).dst_group = 1;
	broadcast_uevent(event->skb, 0, 1, GFP_KERNEL);

out_free_skb:
	if (ret){
	    printk("work error %d\n", ret);
	    kfree_skb(event->skb);
	}
}


void mt753x_port_link_hotplug_work(int port_num, int port_status, char *link_speed, char *duplex)
{
	struct port_event *event = NULL;

	if(port_num >= ARRAY_SIZE(event_port)  ||  port_num < 0){
	    printk("[Warning] invalid port number\n");
	    return;
	}

	mutex_lock(&port_event_lock);
	event = &event_port[port_num];
	if (!event->work.work.func)
		INIT_DELAYED_WORK(&event->work, port_link_hotplug);
	else
		cancel_delayed_work_sync(&event->work);


	strcpy(event->type, "switch");
	event->port = port_num;
	event->status = port_status;
	strcpy(event->action, event->status? "UP":"DOWN");
	strcpy(event->speed,link_speed);
	strcpy(event->duplex,duplex);

	schedule_delayed_work(&event->work, msecs_to_jiffies(WORKQUEUE_DELAY_TIME));
	mutex_unlock(&port_event_lock);
}


static void display_port_link_status(struct gsw_mt753x *gsw, u32 port)
{
	u32 pmsr, speed_bits;
	const char *speed;

	pmsr = mt753x_reg_read(gsw, PMSR(port));

	speed_bits = (pmsr & MAC_SPD_STS_M) >> MAC_SPD_STS_S;

	switch (speed_bits) {
	case MAC_SPD_10:
		speed = "10Mbps";
		break;
	case MAC_SPD_100:
		speed = "100Mbps";
		break;
	case MAC_SPD_1000:
		speed = "1Gbps";
		break;
	case MAC_SPD_2500:
		speed = "2.5Gbps";
		break;
	}

	if (pmsr & MAC_LNK_STS) {
	dev_info(gsw->dev, "Port %d Link is Up - %s/%s\n",
		 port, speed, (pmsr & MAC_DPX_STS) ? "Full" : "Half");
	} else {
		dev_info(gsw->dev, "Port %d Link is Down\n", port);
	}
	mt753x_nl_port_status_notify(port, (pmsr & MAC_LNK_STS) ? 1 : 0);
	mt753x_port_link_hotplug_work(port, pmsr & MAC_LNK_STS, speed, (pmsr & MAC_DPX_STS) ? "Full" : "Half");
}

void mt753x_irq_worker(struct work_struct *work)
{
	struct gsw_mt753x *gsw;
	u32 sts, physts, laststs;
	int i;

	gsw = container_of(work, struct gsw_mt753x, irq_worker);

	sts = mt753x_reg_read(gsw, SYS_INT_STS);

	/* Check for changed PHY link status */
	for (i = 0; i < MT753X_NUM_PHYS; i++) {
		if (!(sts & PHY_LC_INT(i)))
			continue;

		laststs = gsw->phy_link_sts & BIT(i);
		physts = !!(gsw->mii_read(gsw, i, MII_BMSR) & BMSR_LSTATUS);
		physts <<= i;

		if (physts ^ laststs) {
			gsw->phy_link_sts ^= BIT(i);
			display_port_link_status(gsw, i);
		}
	}

	mt753x_reg_write(gsw, SYS_INT_STS, sts);

	enable_irq(gsw->irq);
}
