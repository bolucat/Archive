// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2019-2024 Chilledheart  */

#ifndef H_CONFIG_CONFIG_TLS
#define H_CONFIG_CONFIG_TLS

#include <absl/flags/declare.h>
#include <string>

extern std::string g_certificate_chain_content;
extern std::string g_private_key_content;
ABSL_DECLARE_FLAG(std::string, certificate_chain_file);
ABSL_DECLARE_FLAG(std::string, private_key_file);
ABSL_DECLARE_FLAG(std::string, private_key_password);
ABSL_DECLARE_FLAG(bool, insecure_mode);
ABSL_DECLARE_FLAG(std::string, cacert);
ABSL_DECLARE_FLAG(std::string, capath);
ABSL_DECLARE_FLAG(bool, tls13_early_data);
ABSL_DECLARE_FLAG(bool, enable_post_quantum_kyber);
ABSL_DECLARE_FLAG(bool, use_ml_kem);

namespace config {
bool ReadTLSConfigFile();
}  // namespace config

#endif  // H_CONFIG_CONFIG_TLS
