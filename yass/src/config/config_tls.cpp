// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2023-2024 Chilledheart  */

#include "config/config.hpp"

#include <absl/flags/flag.h>
#include <iostream>
#include "core/utils.hpp"

std::string g_certificate_chain_content;
std::string g_private_key_content;

ABSL_FLAG(std::string, certificate_chain_file, "", "Use custom certificate chain file to verify server's certificate");
ABSL_FLAG(std::string,
          private_key_file,
          "",
          "Use custom private key file to secure connection between server and client");
ABSL_FLAG(std::string,
          private_key_password,
          "",
          "Use custom private key password to decrypt server's encrypted private key");
ABSL_FLAG(bool,
          insecure_mode,
          false,
          "Or '-k', This option makes to skip the verification step and proceed without checking (Client Only)");
ABSL_FLAG(std::string,
          cacert,
          getenv("SSL_CERT_FILE") ? getenv("SSL_CERT_FILE") : "",
          "CA certificate to verify peer against. "
          "You can override it with SSL_CERT_FILE environment variable.");
ABSL_FLAG(std::string,
          capath,
          getenv("SSL_CERT_DIR") ? getenv("SSL_CERT_DIR") : "",
          "CA directory to verify peer against. "
          "You can override it with SSL_CERT_DIR environment variable. "
#ifdef _WIN32
          "It is a semicolon separated list of directories."
#else
          "It is a colon separated list of directories."
#endif
);

ABSL_FLAG(bool, tls13_early_data, true, "Enable 0RTTI Early Data (risk at production)");

ABSL_FLAG(bool,
          enable_post_quantum_kyber,
          false,
          "Enables post-quantum key-agreements in TLS 1.3 connections. "
          "The use_ml_kem flag controls whether ML-KEM or Kyber is used.");
ABSL_FLAG(bool,
          use_ml_kem,
          true,
          "Use ML-KEM in TLS 1.3. "
          "Causes TLS 1.3 connections to use the ML-KEM standard instead of the Kyber "
          "draft standard for post-quantum key-agreement. "
          "The enable_post_quantum_kyber flag must be enabled "
          "for this to have an effect.");

namespace config {
bool ReadTLSConfigFile() {
  do {
    static constexpr const size_t kBufferSize = 256 * 1024;
    const bool is_server = pType_IsServer();

    ssize_t ret;
    if (is_server) {
      std::string private_key, private_key_path = absl::GetFlag(FLAGS_private_key_file);
      if (private_key_path.empty()) {
        std::cerr << "No private key file for certificate provided" << std::endl;
        return false;
      }
      private_key.resize(kBufferSize);
      ret = ReadFileToBuffer(private_key_path, as_writable_bytes(make_span(private_key)));
      if (ret <= 0) {
        std::cerr << "private key " << private_key_path << " failed to read" << std::endl;
        return false;
      }
      private_key.resize(ret);
      g_private_key_content = private_key;
      std::cerr << "Using private key file: " << private_key_path << std::endl;
    }
    std::string certificate_chain, certificate_chain_path = absl::GetFlag(FLAGS_certificate_chain_file);
    if (is_server && certificate_chain_path.empty()) {
      std::cerr << "No certificate file provided" << std::endl;
      return false;
    }
    if (!certificate_chain_path.empty()) {
      certificate_chain.resize(kBufferSize);
      ret = ReadFileToBuffer(certificate_chain_path, as_writable_bytes(make_span(certificate_chain)));
      if (ret <= 0) {
        std::cerr << "certificate file " << certificate_chain_path << " failed to read" << std::endl;
        return false;
      }
      certificate_chain.resize(ret);
      g_certificate_chain_content = certificate_chain;
      std::cerr << "Using certificate chain file: " << certificate_chain_path << std::endl;
    }
  } while (false);
  return true;
}
}  // namespace config
