/*
 * Driver for MediaTek MT7531 gigabit switch
 *
 * Copyright (C) 2018 MediaTek Inc. All Rights Reserved.
 *
 * Author: Landen Chao <landen.chao@mediatek.com>
 *
 * SPDX-License-Identifier:	GPL-2.0+
 */

#include <linux/kernel.h>
#include <linux/mii.h>
#include <linux/mdio.h>
#include <linux/delay.h>
#include <linux/workqueue.h>  

#include "mt753x.h"
#include "mt753x_regs.h"
#include "mt753x_extphy.h"

/*MDIO control*/
#define MII_MMD_ACC_CTL_REG             0x0d
#define MII_MMD_ADDR_DATA_REG           0x0e
#define MMD_OP_MODE_DATA BIT(14)

#define RTL821x_EXT_PAGE_SELECT			0x1e
#define RTL821x_PAGE_SELECT			0x1f

#define RTLGEN_SPEED_MASK			0x0630

#ifndef MDIO_PCS_EEE_ABLE2
#define MDIO_PCS_EEE_ABLE2	21	/* EEE Capability register 2 */
#define MDIO_AN_EEE_ADV2	62	/* EEE advertisement 2 */
#define MDIO_AN_EEE_LPABLE2	63	/* EEE link partner ability 2 */
#endif


int mtk_mdio_read(struct mii_bus *bus, int phy_addr, int phy_reg);
int mtk_mdio_write(struct mii_bus *bus, int phy_addr,int phy_reg, u16 val);

static int ext_phy_cl22_read(struct gsw_mt753x *gsw, int port, int reg)
{
	int data = 0;

	mutex_lock(&gsw->host_bus->mdio_lock);

	data = gsw->host_bus->read(gsw->host_bus, port, reg);

	mutex_unlock(&gsw->host_bus->mdio_lock);

	return data;
}

static int ext_phy_cl22_write(struct gsw_mt753x *gsw, int port, int reg, u16 data)
{
	mutex_lock(&gsw->host_bus->mdio_lock);

	gsw->host_bus->write(gsw->host_bus, port, reg, data);

	mutex_unlock(&gsw->host_bus->mdio_lock);

	return 0;
}

#if 0
static int rtl821x_read_page(struct gsw_mt753x *gsw, int port)
{
	return ext_phy_cl22_read(gsw, port, RTL821x_PAGE_SELECT);
}

static int rtl821x_write_page(struct gsw_mt753x *gsw, int port, int page)
{
	return ext_phy_cl22_write(gsw, port, RTL821x_PAGE_SELECT, page);
}

static int rtlgen_read_mmd(struct gsw_mt753x *gsw, int port, int devnum, u16 regnum)
{
	int ret;

	if (devnum == MDIO_MMD_PCS && regnum == MDIO_PCS_EEE_ABLE) {
		rtl821x_write_page(gsw, port, 0xa5c);
		ret = ext_phy_cl22_read(gsw, port, 0x12);
		rtl821x_write_page(gsw, port, 0);
	} else if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_ADV) {
		rtl821x_write_page(gsw, port, 0xa5d);
		ret = ext_phy_cl22_read(gsw, port, 0x10);
		rtl821x_write_page(gsw, port, 0);
	} else if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_LPABLE) {
		rtl821x_write_page(gsw, port, 0xa5d);
		ret = ext_phy_cl22_read(gsw, port, 0x11);
		rtl821x_write_page(gsw, port, 0);
	} else {
		ret = -EOPNOTSUPP;
	}

	return ret;
}

static int rtlgen_write_mmd(struct gsw_mt753x *gsw, int port, int devnum, u16 regnum,
			    u16 val)
{
	int ret;

	if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_ADV) {
		rtl821x_write_page(gsw, port, 0xa5d);
		ret = ext_phy_cl22_write(gsw, port, 0x10, val);
		rtl821x_write_page(gsw, port, 0);
	} else {
		ret = -EOPNOTSUPP;
	}

	return ret;
}

static int rtl822x_read_mmd(struct gsw_mt753x *gsw, int port, int devnum, u16 regnum)
{
 	int ret;
	ret = rtlgen_read_mmd(gsw, port, devnum, regnum);

	if (ret != -EOPNOTSUPP)
		return ret;

	if (devnum == MDIO_MMD_PCS && regnum == MDIO_PCS_EEE_ABLE2) {
		rtl821x_write_page(gsw, port, 0xa6e);
		ret = ext_phy_cl22_read(gsw, port, 0x16);
		rtl821x_write_page(gsw, port, 0);
	} else if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_ADV2) {
		rtl821x_write_page(gsw, port, 0xa6d);
		ret = ext_phy_cl22_read(gsw, port, 0x12);
		rtl821x_write_page(gsw, port, 0);
	} else if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_LPABLE2) {
		rtl821x_write_page(gsw, port, 0xa6d);
		ret = ext_phy_cl22_read(gsw, port, 0x10);
		rtl821x_write_page(gsw, port, 0);
	}

	return ret;
}

static int rtl822x_write_mmd(struct gsw_mt753x *gsw, int port, int devnum, u16 regnum,
			     u16 val)
{
	int ret;
	ret = rtlgen_write_mmd(gsw, port, devnum, regnum, val);

	if (ret != -EOPNOTSUPP)
		return ret;

	if (devnum == MDIO_MMD_AN && regnum == MDIO_AN_EEE_ADV2) {
		rtl821x_write_page(gsw, port, 0xa6d);
		ret = ext_phy_cl22_write(gsw, port, 0x12, val);
		rtl821x_write_page(gsw, port, 0);
	}

	return ret;
}
#endif

/* get actual speed to cover the downshift case */
static char *rtl8221_get_speed(struct gsw_mt753x *gsw, int port)
{
	int val;

	ext_phy_cl22_write(gsw, port, RTL821x_PAGE_SELECT, 0xa43);
	val = ext_phy_cl22_read(gsw, port, 0x12);
	ext_phy_cl22_write(gsw, port, RTL821x_PAGE_SELECT, 0);

	switch (val & RTLGEN_SPEED_MASK) {
	case 0x0000:
		return "10Mbps";
		break;
	case 0x0010:
		return "100Mbps";
		break;
	case 0x0020:
		return "1Gbps";
		break;
	case 0x0200:
		return "10Gbps";
		break;
	case 0x0210:
		return "2.5Gbps";
		break;
	case 0x0220:
		return "5Gbps";
		break;
	default:
		return "Unknow";
		break;
	}

	return "Unknow";
}

/* get actual speed to cover the downshift case */
static int rtl8221_get_dulplex(struct gsw_mt753x *gsw, int port)
{
	int val;

	ext_phy_cl22_write(gsw, port, RTL821x_PAGE_SELECT, 0xa43);
	val = ext_phy_cl22_read(gsw, port, 0x1a);
	ext_phy_cl22_write(gsw, port, RTL821x_PAGE_SELECT, 0);

	if(val & (1<<3))
		return 1;

	return 0;
}

struct port_work{
	const char	name[16];
	int addr;
	struct gsw_mt753x *gsw;
	struct delayed_work	work;
	int status;
};

#define EXT_PHY_LINK_CHECK_DELAY_TIME	1000
	
static u32 ext_phy_cl45_ind_read(struct gsw_mt753x *gsw, int port, u32 devad, int reg)
{
	u16 data = 0;;

	mutex_lock(&gsw->host_bus->mdio_lock);

	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ACC_CTL_REG, devad);
	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ADDR_DATA_REG, reg);
	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ACC_CTL_REG, MMD_OP_MODE_DATA | devad);
	data = gsw->host_bus->read(gsw->host_bus, port, MII_MMD_ADDR_DATA_REG);

	mutex_unlock(&gsw->host_bus->mdio_lock);

	return data;
}

static u32 ext_phy_cl45_ind_write(struct gsw_mt753x *gsw, int port, u32 devad, int reg, u16 data)
{
	mutex_lock(&gsw->host_bus->mdio_lock);

	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ACC_CTL_REG, devad);
	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ADDR_DATA_REG, reg);
	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ACC_CTL_REG, MMD_OP_MODE_DATA | devad);
	gsw->host_bus->write(gsw->host_bus,  port, MII_MMD_ADDR_DATA_REG, data);

	mutex_unlock(&gsw->host_bus->mdio_lock);

	return 0;
}

extern void mt753x_port_link_hotplug_work(int port_num, int port_status, char *link_speed, char *duplex);
static void ext_phy_port_work_handle(struct work_struct *work)
{
	struct port_work *ext_phy_work = container_of(to_delayed_work(work), struct port_work, work);
	struct gsw_mt753x *gsw = ext_phy_work->gsw;
	int port = ext_phy_work->addr;

	int val;
	int link_status = 0;
	int link_changed = 0;

	/* PHY_SR */
	val = ext_phy_cl22_read(gsw, port, 1);

	link_status = val & 0x4;

	if(link_status != ext_phy_work->status) {
		link_changed = 1;
		ext_phy_work->status = link_status;
	}

	if(link_changed) {
		char *speed;
		int dulplex;
		speed = rtl8221_get_speed(gsw, port);
		dulplex = rtl8221_get_dulplex(gsw, port);
	
		if (ext_phy_work->status) {
			dev_info(gsw->dev, "Port %d Link is Up - %s/%s\n",
				port, speed, dulplex ? "Full" : "Half");
		} else {
				dev_info(gsw->dev, "Port %d Link is Down\n", port);
		}

		mt753x_port_link_hotplug_work(port, ext_phy_work->status? 1 : 0, speed, dulplex ? "Full" : "Half");
	}

	schedule_delayed_work(&ext_phy_work->work, msecs_to_jiffies(EXT_PHY_LINK_CHECK_DELAY_TIME));  
}  

static struct port_work ext_phy_port_work = {
	.work = __DELAYED_WORK_INITIALIZER(ext_phy_port_work.work,
					 ext_phy_port_work_handle, 0),
};

static int ext_phy_port_work_init(struct gsw_mt753x *gsw, int addr)
{
	cancel_delayed_work_sync(&ext_phy_port_work.work);

	ext_phy_port_work.gsw = gsw;
	ext_phy_port_work.addr = addr;

	schedule_delayed_work(&ext_phy_port_work.work, msecs_to_jiffies(EXT_PHY_LINK_CHECK_DELAY_TIME));

	return 0;
} 

int gpy211_init(struct gsw_mt753x *gsw, int addr)
{
	/* Enable rate adaption */
	gsw->mmd_write(gsw, addr, 0x1e, 0x8, 0x24e2);

	return 0;
}

int rtl8221_init(struct gsw_mt753x *gsw, int addr)
{
	int val;

	dev_info(gsw->dev, "RTL8221 GPHY initializing.\n");

	/* Do RTL8221 Reset ?*/
	val = ext_phy_cl45_ind_read(gsw, addr, 30, 0x75f3);
	val &= ~(0x1);
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x75f3, val);


	/* Serdes set to  HSGMII Mode and force link up */
	val = ext_phy_cl45_ind_read(gsw, addr, 30, 0x697a);
	val &= ~(0x3f);
	val |= 0x3;
	val |= 1<<15;
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x697a, val);

	/* Serdes set to  HSGMII 2.5Gbps */
	val = ext_phy_cl45_ind_read(gsw, addr, 30, 0x7580);
	val &= ~(0xf);
	val |= 0x12;
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x7580, val);

	/* Disable HSGMII Auto Negotiation */
	val = 0x2;
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x7588, val);
	val = 0x71d0;
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x7589, val);
	val = 0x3;
	ext_phy_cl45_ind_write(gsw, addr, 30, 0x7587, val);	

	while(1) {
		val = ext_phy_cl45_ind_read(gsw, addr, 30, 0x7587);
		if(!(val & 1)) {
			//printk("RTL8221: HiSGMII ready.\n");
			break;
		}
	}

	while(1) {
		val = ext_phy_cl45_ind_read(gsw, addr, 30, 0x758c);
		if(!(val & 1)) {
			//printk("RTL8221: Serdes signal ready.\n");
			break;
		}
	}

	ext_phy_port_work_init(gsw, addr);

	return 0;
}

static struct mt753x_extphy_id extphy_tbl[] = {
        {0x67c9de00, 0x0fffffff0, gpy211_init},
        {0x001cc849, 0x0ffffffff, rtl8221_init},
};

static u32 get_cl22_phy_id(struct gsw_mt753x *gsw, int addr)
{
	int phy_reg;
	u32 phy_id = 0;

	phy_reg = gsw->mii_read(gsw, addr, MII_PHYSID1);
	if (phy_reg < 0)
		return 0;
	phy_id = (phy_reg & 0xffff) << 16;

	/* Grab the bits from PHYIR2, and put them in the lower half */
	phy_reg = gsw->mii_read(gsw, addr, MII_PHYSID2);
	if (phy_reg < 0)
		return 0;

	phy_id |= (phy_reg & 0xffff);

	return phy_id;
}

static u32 get_cl45_phy_id(struct gsw_mt753x *gsw, int addr)
{
	int phy_reg;
	u32 phy_id = 0;

	phy_reg =ext_phy_cl45_ind_read(gsw, addr, 1, MII_PHYSID1);

	if (phy_reg < 0)
		return 0;
	phy_id = (phy_reg & 0xffff) << 16;

	/* Grab the bits from PHYIR2, and put them in the lower half */
	phy_reg = ext_phy_cl45_ind_read(gsw, addr, 1, MII_PHYSID2);
	if (phy_reg < 0)
		return 0;

	phy_id |= (phy_reg & 0xffff);

	return phy_id;
}


static inline bool phy_id_is_match(u32 id, struct mt753x_extphy_id *phy)
{
	return ((id & phy->phy_id_mask) == (phy->phy_id & phy->phy_id_mask));
}

int extphy_init(struct gsw_mt753x *gsw, int addr)
{
	int i;
	u32 phy_id;
	struct mt753x_extphy_id *extphy;

	phy_id = get_cl22_phy_id(gsw, addr);

	if(phy_id == 0 || phy_id == 0xffff)
		phy_id = get_cl45_phy_id(gsw, addr);

	for (i = 0; i < ARRAY_SIZE(extphy_tbl); i++) {
		extphy = &extphy_tbl[i];
		if(phy_id_is_match(phy_id, extphy))
			extphy->init(gsw, addr);
	}

	return 0;
}
