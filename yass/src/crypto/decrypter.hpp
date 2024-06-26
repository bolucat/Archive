// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2019-2024 Chilledheart  */

#ifndef H_CRYPTO_DECRYPTER
#define H_CRYPTO_DECRYPTER

#include "crypto/crypter.hpp"

#include <stddef.h>
#include <stdint.h>
#include <memory>
#include <string>

namespace crypto {

class Decrypter : public Crypter {
 public:
  virtual ~Decrypter();

  static std::unique_ptr<Decrypter> CreateFromCipherSuite(uint32_t cipher_suite);

  // Sets the encryption key. Returns true on success, false on failure.
  // |DecryptPacket| may not be called until |SetDiversificationNonce| is
  // called and the preliminary keying material will be combined with that
  // nonce in order to create the actual key and nonce-prefix.
  //
  // If this function is called, neither |SetKey| nor |SetNoncePrefix| may be
  // called.
  virtual bool SetPreliminaryKey(const char* key, size_t key_len) = 0;

#if 0
  // SetDiversificationNonce uses |nonce| to derive final keys based on the
  // input keying material given by calling |SetPreliminaryKey|.
  //
  // Calling this function is a no-op if |SetPreliminaryKey| hasn't been
  // called.
  virtual bool SetDiversificationNonce(const DiversificationNonce& nonce) = 0;
#endif

  // Populates |output| with the decrypted |ciphertext| and populates
  // |output_length| with the length.  Returns 0 if there is an error.
  // |output| size is specified by |max_output_length| and must be
  // at least as large as the ciphertext.  |packet_number| is
  // appended to the |nonce_prefix| value provided in SetNoncePrefix()
  // to form the nonce.
  // TODO(wtc): add a way for DecryptPacket to report decryption failure due
  // to non-authentic inputs, as opposed to other reasons for failure.
  virtual bool DecryptPacket(uint64_t packet_number,
                             const char* associated_data,
                             size_t associated_data_len,
                             const char* ciphertext,
                             size_t ciphertext_len,
                             char* output,
                             size_t* output_length,
                             size_t max_output_length) = 0;
#if 0
  // Reads a sample of ciphertext from |sample_reader| and uses the header
  // protection key to generate a mask to use for header protection. If
  // successful, this function returns this mask, which is at least 5 bytes
  // long. Callers can detect failure by checking if the output string is empty.
  virtual std::string GenerateHeaderProtectionMask(
      QuicDataReader* sample_reader) = 0;
#endif

  // The ID of the cipher. Return 0x03000000 ORed with the 'cryptographic suite
  // selector'.
  virtual uint32_t cipher_id() const = 0;

  // For use by unit tests only.
  virtual const uint8_t* GetKey() const = 0;
  virtual const uint8_t* GetIV() const = 0;
  virtual const uint8_t* GetNoncePrefix() const = 0;

#if 0
  static void DiversifyPreliminaryKey(QuicStringPiece preliminary_key,
                                      QuicStringPiece nonce_prefix,
                                      const DiversificationNonce& nonce,
                                      size_t key_size,
                                      size_t nonce_prefix_size,
                                      std::string* out_key,
                                      std::string* out_nonce_prefix);
#endif
};

}  // namespace crypto

#endif  // H_CRYPTO_DECRYPTER
