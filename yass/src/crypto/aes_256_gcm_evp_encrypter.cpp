// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2019-2024 Chilledheart  */

#include "crypto/aes_256_gcm_evp_encrypter.hpp"

#include "core/logging.hpp"

#include "crypto/crypter_export.hpp"
#include "third_party/boringssl/src/include/openssl/aead.h"

static const size_t kKeySize = 32;
static const size_t kNonceSize = 12;

namespace crypto {

Aes256GcmEvpEncrypter::Aes256GcmEvpEncrypter()
    : EvpAeadEncrypter(EVP_aead_aes_256_gcm, kKeySize, kAuthTagSize, kNonceSize) {
  static_assert(kKeySize <= kMaxKeySize, "key size too big");
  static_assert(kNonceSize <= kMaxNonceSize, "nonce size too big");
}

Aes256GcmEvpEncrypter::~Aes256GcmEvpEncrypter() = default;

uint32_t Aes256GcmEvpEncrypter::cipher_id() const {
  return CRYPTO_AES256GCMSHA256_EVP;
}

}  // namespace crypto
