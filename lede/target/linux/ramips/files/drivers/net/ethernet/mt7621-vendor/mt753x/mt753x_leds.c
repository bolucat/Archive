/*
 *  A Sample GPIO LED driver for MT753X Switch
 *
 *  Copyright (c) 2021 lintel<lintel.huang@gmail.com>
 *
 */

#include <linux/gpio.h>
#include <linux/init.h>
#include <linux/irq.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/slab.h>
#include <linux/leds.h>
#include <linux/delay.h>
#include <linux/proc_fs.h>
#include <linux/uaccess.h>

#include "mt753x.h"

/* 32 GPIOs */
#define MT753X_GPIO_MAX_NUM		32

#define PROC_NAME_MT753X_LEDS "mt753x_gpio_leds"

enum {
	MT753X_GPIO_OUTPUT_LOW = 0,
	MT753X_GPIO_OUTPUT_HIGH = 1
};

enum {
	MT753X_LED_GPIO_MODE = 0,
	MT753X_LED_FUNC_MODE = 1
};


/* Registers for LED GPIO control (MT7530 only)
 * All registers follow this pattern:
 * [ 2: 0]  port 0
 * [ 6: 4]  port 1
 * [10: 8]  port 2
 * [14:12]  port 3
 * [18:16]  port 4
 */

/* LED enable, 0: Disable, 1: Enable (Default) */
#define MT753X_LED_EN			0x7d00

/* LED mode, 0: GPIO mode, 1: PHY mode (Default) */
#define MT753X_LED_IO_MODE		0x7d04

/* GPIO direction, 0: Input, 1: Output */
#define MT753X_LED_GPIO_DIR		0x7d10

/* GPIO output enable, 0: Disable, 1: Enable */
#define MT753X_LED_GPIO_OE		0x7d14

/* GPIO value, 0: Low, 1: High */
#define MT753X_LED_GPIO_DATA		0x7d18

#define to_mt753x_led(led_cdev) \
	container_of(led_cdev, struct mt753x_led, cdev)

#define MT7530_LED_GPIO(port)  (port * 4)

struct mt753x_led {
	char *name;
	struct led_classdev cdev;
	unsigned char gpio;
	struct gsw_mt753x *gsw;
	u8 active_low;
};

static int mt753x_leds_enable = 0;
static struct proc_dir_entry *mt753x_leds_proc_fs = NULL;
static struct mutex  mt753x_leds_proc_mutex;

extern u32 mt753x_reg_read(struct gsw_mt753x *gsw, u32 reg);
extern void mt753x_reg_write(struct gsw_mt753x *gsw, u32 reg, u32 val);
extern void mt753x_reg_update(struct gsw_mt753x *gsw, u32 reg, u32 val, u32 mask);

/* FIXME:
 *
 *  这个万恶的锁不是故意干，是为了解决多核的竞态问题，有空再来填.
 *
 */
static spinlock_t	mt753x_led_lock;
static unsigned long mt753x_led_flags;

static void mt753x_set_gpiomode(struct gsw_mt753x *gsw, unsigned int gpio, int mode)
{
	u32 temp;
	u32 bit;

	spin_lock_irqsave(&mt753x_led_lock, mt753x_led_flags);

	bit = BIT(gpio);

	temp = mt753x_reg_read(gsw, MT753X_LED_IO_MODE);

	if (mode) {
		temp |= bit;
	} else {
		temp &= ~bit;
	}

	mt753x_reg_write(gsw, MT753X_LED_IO_MODE, temp);

	spin_unlock_irqrestore(&mt753x_led_lock, mt753x_led_flags);
}

static void mt753x_gpio_set(struct gsw_mt753x *gsw, unsigned int gpio, int value)
{
	u32 temp;
	u32 bit;

	spin_lock_irqsave(&mt753x_led_lock, mt753x_led_flags);

	bit = BIT(gpio);

	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_DATA);

	if (value) {
		temp |= bit;
	} else {
		temp &= ~bit;
	}

	mt753x_reg_write(gsw, MT753X_LED_GPIO_DATA, temp);

	spin_unlock_irqrestore(&mt753x_led_lock, mt753x_led_flags);
}

#if 0
static int mt753x_gpio_get(struct gsw_mt753x *gsw, unsigned int gpio)
{
	u32 bit = BIT(gpio);

	return !!(mt753x_reg_read(gsw, MT753X_LED_GPIO_DATA) & bit);
}

static int mt753x_gpio_get_direction(struct gsw_mt753x *gsw, unsigned int gpio)
{
	u32 bit = BIT(gpio);

	return (mt753x_reg_read(gsw, MT753X_LED_GPIO_DIR) & bit) ?
	       GPIOF_DIR_OUT : GPIOF_DIR_IN;
}

static int mt753x_gpio_direction_input(struct gsw_mt753x *gsw, unsigned int gpio)
{
	u32 bit = BIT(gpio);
	u32 temp;

	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_OE);
	temp &= ~bit;
	mt753x_reg_write(gsw, MT753X_LED_GPIO_OE, temp);

	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_DIR);
	temp &= ~bit;
	mt753x_reg_write(gsw, MT753X_LED_GPIO_DIR, temp);

	return 0;
}
#endif

static int mt753x_gpio_direction_output(struct gsw_mt753x *gsw, unsigned int gpio, int value)
{
	u32 temp;
	u32 bit;

	spin_lock_irqsave(&mt753x_led_lock, mt753x_led_flags);

	bit = BIT(gpio);

	/* set GPIO direction */
	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_DIR);
	temp |= bit;
	mt753x_reg_write(gsw, MT753X_LED_GPIO_DIR, temp);

	/* set GPIO output enable */
	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_OE);
	temp |= bit;
	mt753x_reg_write(gsw, MT753X_LED_GPIO_OE, temp);

	/* set GPIO data  */
	temp = mt753x_reg_read(gsw, MT753X_LED_GPIO_DATA);

	if (value) {
		temp |= bit;
	} else {
		temp &= ~bit;
	}

	mt753x_reg_write(gsw, MT753X_LED_GPIO_DATA, temp);

	spin_unlock_irqrestore(&mt753x_led_lock, mt753x_led_flags);

	return 0;
}


static struct mt753x_led mt753x_led_devices[] = {
	{
		.name = "mt753x:port0:link",
		.gpio =  MT7530_LED_GPIO(0),
		.active_low = 1,
	}, {
		.name = "mt753x:port1:link",
		.gpio =  MT7530_LED_GPIO(1),
		.active_low = 1,
	}, {
		.name = "mt753x:port2:link",
		.gpio =  MT7530_LED_GPIO(2),
		.active_low = 1,
	}, {
		.name = "mt753x:port3:link",
		.gpio =  MT7530_LED_GPIO(3),
		.active_low = 0,
	},  {
		.name = "mt753x:port4:link",
		.gpio =  MT7530_LED_GPIO(4),
		.active_low = 1,
	},
};

static void  mt753x_led_brightness_set(struct led_classdev *led, enum led_brightness brightness)
{
	struct mt753x_led *mt753x_led = to_mt753x_led(led);
	struct gsw_mt753x *gsw = mt753x_led->gsw;
	u32 value;

	if (brightness == LED_OFF) {
		value =  !!(MT753X_GPIO_OUTPUT_LOW ^ mt753x_led->active_low);
	} else {
		value =  !!(MT753X_GPIO_OUTPUT_HIGH ^ mt753x_led->active_low);
	}

	mt753x_gpio_set(gsw, mt753x_led->gpio, value);
}

static int register_mt753x_leds(struct gsw_mt753x *gsw, struct mt753x_led *mt753x_led, unsigned int led_num)
{
	int i;
	int ret;

	for(i = 0; i < led_num; i++ ) {
		mt753x_led->gsw = gsw;

		/* swtich led mode to GPIO mode */
		mt753x_set_gpiomode(gsw, mt753x_led->gpio, MT753X_LED_GPIO_MODE);

		/* set GPIO output mode and LED default off */
		mt753x_gpio_direction_output(gsw, mt753x_led->gpio, 0);

		/* LED default OFF */
		mt753x_led_brightness_set(&mt753x_led->cdev, LED_OFF);

		mt753x_led->cdev.name = mt753x_led->name;
		mt753x_led->cdev.brightness_set = mt753x_led_brightness_set;
		ret = led_classdev_register(NULL, &mt753x_led->cdev);

		if(!ret) {
			pr_info("registered led devices: %s\n", mt753x_led->name);
		}

		mt753x_led++;
	}

	mt753x_leds_enable = 1;

	return 0;
}

static int unregister_mt753x_leds(struct gsw_mt753x *gsw, struct mt753x_led *mt753x_led, unsigned int led_num)
{
	int i;

	for(i = 0; i < led_num; i++ ) {
		led_classdev_unregister(&mt753x_led->cdev);

		/* swtich led mode to GPIO mode */
		mt753x_set_gpiomode(gsw, mt753x_led->gpio, MT753X_LED_FUNC_MODE);

		mt753x_led++;
	}

	mt753x_leds_enable = 0;
	return 0;
}

static int mt753x_leds_proc_read(struct seq_file *seq, void *v)
{
	seq_printf(seq, "gpio leds enable: %u\n", mt753x_leds_enable);
	return 0;
}

static ssize_t mt753x_leds_proc_write(struct file *file, const char __user *buffer,
				      size_t count, loff_t *data)
{
	int ret;
	char buf[32] = {0};
	int len = count, to_set;
	struct gsw_mt753x *gsw = pde_data(file_inode(file));

	if (copy_from_user(buf, buffer, sizeof(buf))) {
		goto error;
	}

	to_set = simple_strtol(buf, 0, 10);

	if(to_set == mt753x_leds_enable)
		goto error;

	mt753x_leds_enable = to_set;

	if(to_set == 1) {
		ret = register_mt753x_leds(gsw, &mt753x_led_devices[0], ARRAY_SIZE(mt753x_led_devices));
	} else {
		ret = unregister_mt753x_leds(gsw, &mt753x_led_devices[0], ARRAY_SIZE(mt753x_led_devices));
	}

error:
	mutex_unlock(&mt753x_leds_proc_mutex);
	return len;
}

static int mt753x_leds_proc_open(struct inode *inode, struct file *file)
{

	return single_open(file, mt753x_leds_proc_read, NULL);
}

static const struct proc_ops mt753x_leds_proc_operations = {
	.proc_open		= mt753x_leds_proc_open,
	.proc_write 		= mt753x_leds_proc_write,
	.proc_read		= seq_read,
	.proc_lseek		= seq_lseek,
	.proc_release	= single_release,
};

int mt753x_leds_init(struct gsw_mt753x *gsw)
{
	int ret = 0;
	struct device_node *np = gsw->dev->of_node;
	int led_gpio;

	led_gpio = of_property_read_bool(np, "mediatek,leds-gpio");

	if(led_gpio) {
		dev_info(gsw->dev, "LED initialized\n");
		ret = register_mt753x_leds(gsw, &mt753x_led_devices[0], ARRAY_SIZE(mt753x_led_devices));
	}

	mt753x_leds_proc_fs = proc_create_data(PROC_NAME_MT753X_LEDS, 0, NULL, &mt753x_leds_proc_operations, gsw);

	if(!mt753x_leds_proc_fs)
		return -EINVAL;

	return ret;
}

int mt753x_leds_exit(struct gsw_mt753x *gsw)
{
	int ret;
	struct device_node *np = gsw->dev->of_node;
	int led_gpio;

	led_gpio = of_property_read_bool(np, "mediatek,led-gpio");

	if(led_gpio) {
		dev_info(gsw->dev, "LED unregister\n");

		ret = unregister_mt753x_leds(gsw, &mt753x_led_devices[0], ARRAY_SIZE(mt753x_led_devices));
	}

	proc_remove(mt753x_leds_proc_fs);

	return ret;
}
