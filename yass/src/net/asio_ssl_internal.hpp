// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2024 Chilledheart  */

#ifndef H_NET_ASIO_SSL_INTERNAL
#define H_NET_ASIO_SSL_INTERNAL

#include "net/asio.hpp"

#include <string_view>

extern "C" const char _binary_ca_bundle_crt_start[];
extern "C" const char _binary_ca_bundle_crt_end[];

extern "C" const char _binary_supplementary_ca_bundle_crt_start[];
extern "C" const char _binary_supplementary_ca_bundle_crt_end[];

int load_ca_to_ssl_ctx_from_mem(SSL_CTX* ssl_ctx, std::string_view cadata);
int load_ca_to_ssl_ctx_system(SSL_CTX* ssl_ctx);
int load_ca_to_ssl_ctx_system_extra(SSL_CTX* ssl_ctx);

#endif  // H_NET_ASIO_SSL_INTERNAL
