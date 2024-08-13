/*
 * Rockchip Generic power configuration support.
 *
 * Copyright (c) 2017 ROCKCHIP, Co. Ltd.
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 */

#include <linux/arm-smccc.h>
#include <linux/bitops.h>
#include <linux/cpu.h>
#include <linux/module.h>
#include <linux/of_gpio.h>
#include <linux/platform_device.h>
#include <linux/pm.h>
#include <linux/regulator/driver.h>
#include <linux/regulator/machine.h>
#include <linux/rockchip/rockchip_sip.h>
#include <linux/suspend.h>
#include <dt-bindings/input/input.h>
#include <../drivers/regulator/internal.h>

#define PM_INVALID_GPIO			0xffff
#define MAX_ON_OFF_REG_NUM		30
#define MAX_ON_OFF_REG_PROP_NAME_LEN	60
#define MAX_CONFIG_PROP_NAME_LEN	60

enum rk_pm_state {
	RK_PM_MEM = 0,
	RK_PM_MEM_LITE,
	RK_PM_MEM_ULTRA,
	RK_PM_STATE_MAX
};

#ifndef MODULE
static const char * const pm_state_str[RK_PM_STATE_MAX] = {
	[RK_PM_MEM] = "mem",
	[RK_PM_MEM_LITE] = "mem-lite",
	[RK_PM_MEM_ULTRA] = "mem-ultra",
};

static struct rk_on_off_regulator_list {
	struct regulator_dev *on_reg_list[MAX_ON_OFF_REG_NUM];
	struct regulator_dev *off_reg_list[MAX_ON_OFF_REG_NUM];
} on_off_regs_list[RK_PM_STATE_MAX];
#endif

static struct rk_sleep_config {
	u32 mode_config;
	u32 wakeup_config;
} sleep_config[RK_PM_STATE_MAX];

static const struct of_device_id pm_match_table[] = {
	{ .compatible = "rockchip,pm-px30",},
	{ .compatible = "rockchip,pm-rk1808",},
	{ .compatible = "rockchip,pm-rk322x",},
	{ .compatible = "rockchip,pm-rk3288",},
	{ .compatible = "rockchip,pm-rk3308",},
	{ .compatible = "rockchip,pm-rk3328",},
	{ .compatible = "rockchip,pm-rk3368",},
	{ .compatible = "rockchip,pm-rk3399",},
	{ .compatible = "rockchip,pm-rk3528",},
	{ .compatible = "rockchip,pm-rk3562",},
	{ .compatible = "rockchip,pm-rk3568",},
	{ .compatible = "rockchip,pm-rk3588",},
	{ .compatible = "rockchip,pm-rv1126",},
	{ },
};

#ifndef MODULE
static void rockchip_pm_virt_pwroff_prepare(void)
{
	int error;

	regulator_suspend_prepare(PM_SUSPEND_MEM);

	error = suspend_disable_secondary_cpus();
	if (error) {
		pr_err("Disable nonboot cpus failed!\n");
		return;
	}

	sip_smc_set_suspend_mode(VIRTUAL_POWEROFF, 0, 1);
	sip_smc_virtual_poweroff();
}

static int parse_sleep_config(struct device_node *node, enum rk_pm_state state)
{
	char mode_prop_name[MAX_CONFIG_PROP_NAME_LEN];
	char wkup_prop_name[MAX_CONFIG_PROP_NAME_LEN];
	struct rk_sleep_config *config;

	if (state == RK_PM_MEM || state >= RK_PM_STATE_MAX)
		return -EINVAL;

	snprintf(mode_prop_name, sizeof(mode_prop_name),
		 "sleep-mode-config-%s", pm_state_str[state]);
	snprintf(wkup_prop_name, sizeof(wkup_prop_name),
		 "wakeup-config-%s", pm_state_str[state]);

	config = &sleep_config[state];

	if (of_property_read_u32_array(node,
				       mode_prop_name,
				       &config->mode_config, 1))
		pr_info("%s not set sleep-mode-config for %s\n",
			node->name, pm_state_str[state]);

	if (of_property_read_u32_array(node,
				       wkup_prop_name,
				       &config->wakeup_config, 1))
		pr_info("%s not set wakeup-config for %s\n",
			node->name, pm_state_str[state]);

	return 0;
}

static int parse_regulator_list(struct device_node *node,
				char *prop_name,
				struct regulator_dev **out_list)
{
	struct device_node *dn;
	struct regulator_dev *reg;
	int i, j;

	if (of_find_property(node, prop_name, NULL)) {
		for (i = 0, j = 0;
		     (dn = of_parse_phandle(node, prop_name, i)) && j < MAX_ON_OFF_REG_NUM;
		     i++) {
			reg = of_find_regulator_by_node(dn);
			if (reg == NULL) {
				pr_warn("failed to find regulator %s for %s\n",
					dn->name, prop_name);
			} else {
				pr_debug("%s %s regulator=%s\n", __func__,
					 prop_name,
					 reg->desc->name);
				out_list[j++] = reg;
			}
			of_node_put(dn);
		}
	}

	return 0;
}

static int parse_on_off_regulator(struct device_node *node, enum rk_pm_state state)
{
	char on_prop_name[MAX_ON_OFF_REG_PROP_NAME_LEN];
	char off_prop_name[MAX_ON_OFF_REG_PROP_NAME_LEN];

	if (state >= RK_PM_STATE_MAX)
		return -EINVAL;

	snprintf(on_prop_name, sizeof(on_prop_name),
		 "rockchip,regulator-on-in-%s", pm_state_str[state]);
	snprintf(off_prop_name, sizeof(off_prop_name),
		 "rockchip,regulator-off-in-%s", pm_state_str[state]);

	parse_regulator_list(node, on_prop_name, on_off_regs_list[state].on_reg_list);
	parse_regulator_list(node, off_prop_name, on_off_regs_list[state].off_reg_list);

	return 0;
}
#endif

static int pm_config_probe(struct platform_device *pdev)
{
	const struct of_device_id *match_id;
	struct device_node *node;
	struct rk_sleep_config *config = &sleep_config[RK_PM_MEM];
	u32 pwm_regulator_config = 0;
	int gpio_temp[10];
	u32 sleep_debug_en = 0;
	u32 apios_suspend = 0;
	u32 io_ret_config = 0;
#ifndef MODULE
	u32 virtual_poweroff_en = 0;
#endif
	enum of_gpio_flags flags;
	int i = 0;
	int length;
	int ret;

	match_id = of_match_node(pm_match_table, pdev->dev.of_node);
	if (!match_id)
		return -ENODEV;

	node = of_find_node_by_name(NULL, "rockchip-suspend");

	if (IS_ERR_OR_NULL(node)) {
		dev_err(&pdev->dev, "%s dev node err\n",  __func__);
		return -ENODEV;
	}

	if (of_property_read_u32_array(node,
				       "rockchip,sleep-mode-config",
				       &config->mode_config, 1))
		dev_warn(&pdev->dev, "not set sleep mode config\n");
	else
		sip_smc_set_suspend_mode(SUSPEND_MODE_CONFIG, config->mode_config, 0);

	if (of_property_read_u32_array(node,
				       "rockchip,wakeup-config",
				       &config->wakeup_config, 1))
		dev_warn(&pdev->dev, "not set wakeup-config\n");
	else
		sip_smc_set_suspend_mode(WKUP_SOURCE_CONFIG, config->wakeup_config, 0);

	if (of_property_read_u32_array(node,
				       "rockchip,pwm-regulator-config",
				       &pwm_regulator_config, 1))
		dev_warn(&pdev->dev, "not set pwm-regulator-config\n");
	else
		sip_smc_set_suspend_mode(PWM_REGULATOR_CONFIG,
					 pwm_regulator_config,
					 0);

	length = of_gpio_named_count(node, "rockchip,power-ctrl");

	if (length > 0 && length < 10) {
		for (i = 0; i < length; i++) {
			gpio_temp[i] = of_get_named_gpio_flags(node,
							     "rockchip,power-ctrl",
							     i,
							     &flags);
			if (!gpio_is_valid(gpio_temp[i]))
				break;
			sip_smc_set_suspend_mode(GPIO_POWER_CONFIG,
						 i,
						 gpio_temp[i]);
		}
	}
	sip_smc_set_suspend_mode(GPIO_POWER_CONFIG, i, PM_INVALID_GPIO);

	if (!of_property_read_u32_array(node,
					"rockchip,sleep-debug-en",
					&sleep_debug_en, 1))
		sip_smc_set_suspend_mode(SUSPEND_DEBUG_ENABLE,
					 sleep_debug_en,
					 0);

	if (!of_property_read_u32_array(node,
					"rockchip,apios-suspend",
					&apios_suspend, 1))
		sip_smc_set_suspend_mode(APIOS_SUSPEND_CONFIG,
					 apios_suspend,
					 0);

	if (!of_property_read_u32_array(node,
					"rockchip,sleep-io-ret-config",
					&io_ret_config, 1)) {
		ret = sip_smc_set_suspend_mode(SUSPEND_IO_RET_CONFIG, io_ret_config, 0);
		if (ret)
			dev_warn(&pdev->dev,
				 "sleep-io-ret-config failed (%d), check parameters or update trust\n",
				 ret);
	}

#ifndef MODULE
	if (!of_property_read_u32_array(node,
					"rockchip,virtual-poweroff",
					&virtual_poweroff_en, 1) &&
	    virtual_poweroff_en)
		pm_power_off_prepare = rockchip_pm_virt_pwroff_prepare;

	for (i = RK_PM_MEM; i < RK_PM_STATE_MAX; i++) {
		parse_sleep_config(node, i);
		parse_on_off_regulator(node, i);
	}
#endif

	return 0;
}

#ifndef MODULE
static int pm_config_prepare(struct device *dev)
{
	int i;
	suspend_state_t suspend_state = mem_sleep_current;
	enum rk_pm_state state = suspend_state - PM_SUSPEND_MEM;
	struct regulator_dev **on_list;
	struct regulator_dev **off_list;
	struct rk_sleep_config *config, *def_config = &sleep_config[RK_PM_MEM];

	sip_smc_set_suspend_mode(LINUX_PM_STATE,
				 suspend_state,
				 0);

	if (state >= RK_PM_STATE_MAX)
		return 0;

	config = &sleep_config[state];

	if (config->mode_config)
		sip_smc_set_suspend_mode(SUSPEND_MODE_CONFIG,
					 config->mode_config, 0);
	else if (def_config->mode_config)
		sip_smc_set_suspend_mode(SUSPEND_MODE_CONFIG,
					 def_config->mode_config, 0);

	if (config->wakeup_config)
		sip_smc_set_suspend_mode(WKUP_SOURCE_CONFIG,
					 config->wakeup_config, 0);
	else if (def_config->wakeup_config)
		sip_smc_set_suspend_mode(WKUP_SOURCE_CONFIG,
					 def_config->wakeup_config, 0);

	on_list = on_off_regs_list[state].on_reg_list;
	off_list = on_off_regs_list[state].off_reg_list;

	for (i = 0; i < MAX_ON_OFF_REG_NUM && on_list[i]; i++)
		regulator_suspend_enable(on_list[i], PM_SUSPEND_MEM);

	for (i = 0; i < MAX_ON_OFF_REG_NUM && off_list[i]; i++)
		regulator_suspend_disable(off_list[i], PM_SUSPEND_MEM);

	return 0;
}

static const struct dev_pm_ops rockchip_pm_ops = {
	.prepare = pm_config_prepare,
};
#endif

static struct platform_driver pm_driver = {
	.probe = pm_config_probe,
	.driver = {
		.name = "rockchip-pm",
		.of_match_table = pm_match_table,
#ifndef MODULE
		.pm = &rockchip_pm_ops,
#endif
	},
};

static int __init rockchip_pm_drv_register(void)
{
	return platform_driver_register(&pm_driver);
}
late_initcall_sync(rockchip_pm_drv_register);
MODULE_DESCRIPTION("Rockchip suspend mode config");
MODULE_LICENSE("GPL");
