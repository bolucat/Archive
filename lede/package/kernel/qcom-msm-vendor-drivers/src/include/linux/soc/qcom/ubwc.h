/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef _QCOM_MSM_VENDOR_UBWC_COMPAT_H
#define _QCOM_MSM_VENDOR_UBWC_COMPAT_H

#include_next <linux/soc/qcom/ubwc.h>

#ifndef UBWC_6_0
#define UBWC_6_0 0x60000000
#endif

#ifndef _QCOM_MSM_VENDOR_UBWC_HELPERS
#define _QCOM_MSM_VENDOR_UBWC_HELPERS

static inline bool qcom_ubwc_min_acc_length_64b(const struct qcom_ubwc_cfg_data *cfg)
{
	return cfg->ubwc_enc_version == UBWC_1_0 &&
		(cfg->ubwc_dec_version == UBWC_2_0 ||
		 cfg->ubwc_dec_version == UBWC_3_0);
}

static inline bool qcom_ubwc_macrotile_mode(const struct qcom_ubwc_cfg_data *cfg)
{
	return cfg->macrotile_mode;
}

static inline bool qcom_ubwc_bank_spread(const struct qcom_ubwc_cfg_data *cfg)
{
	return cfg->ubwc_bank_spread;
}

static inline u32 qcom_ubwc_swizzle(const struct qcom_ubwc_cfg_data *cfg)
{
	return cfg->ubwc_swizzle;
}

#endif

#endif
