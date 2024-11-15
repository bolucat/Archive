// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2022-2024 Chilledheart  */

#include "net/asio_ssl_internal.hpp"

#ifdef _WIN32
#include "base/win/dirent.h"
#else
#include <dirent.h>
#endif

#include <absl/strings/str_cat.h>
#include <absl/strings/str_split.h>
#include <base/files/memory_mapped_file.h>
#include <base/files/platform_file.h>
#include <filesystem>
#include <string>

#include "config/config_tls.hpp"
#include "core/utils.hpp"
#include "net/x509_util.hpp"

#ifdef _WIN32
#include <wincrypt.h>
#undef X509_NAME
#elif BUILDFLAG(IS_MAC)
#include <Security/Security.h>
#include "base/apple/foundation_util.h"
#include "third_party/boringssl/src/pki/cert_errors.h"
#include "third_party/boringssl/src/pki/cert_issuer_source_static.h"
#include "third_party/boringssl/src/pki/extended_key_usage.h"
#include "third_party/boringssl/src/pki/parse_name.h"
#include "third_party/boringssl/src/pki/parsed_certificate.h"
#include "third_party/boringssl/src/pki/trust_store.h"
#endif

ABSL_FLAG(bool, ca_native, false, "Load CA certs from the OS.");

std::ostream& operator<<(std::ostream& o, asio::error_code ec) {
#ifdef _WIN32
  return o << ec.message() << " value: " << ec.value();
#else
  return o << ec.message();
#endif
}

#if BUILDFLAG(IS_MAC)

using namespace gurl_base::apple;

namespace {

// The rules for interpreting trust settings are documented at:
// https://developer.apple.com/reference/security/1400261-sectrustsettingscopytrustsetting?language=objc

// Indicates the trust status of a certificate.
enum class TrustStatus {
  // Trust status is unknown / uninitialized.
  UNKNOWN,
  // Certificate inherits trust value from its issuer. If the certificate is the
  // root of the chain, this implies distrust.
  UNSPECIFIED,
  // Certificate is a trust anchor.
  TRUSTED,
  // Certificate is blocked / explicitly distrusted.
  DISTRUSTED
};

// Returns trust status of usage constraints dictionary |trust_dict| for a
// certificate that |is_self_issued|.
TrustStatus IsTrustDictionaryTrustedForPolicy(CFDictionaryRef trust_dict,
                                              bool is_self_issued,
                                              const CFStringRef target_policy_oid) {
  // An empty trust dict should be interpreted as
  // kSecTrustSettingsResultTrustRoot. This is handled by falling through all
  // the conditions below with the default value of |trust_settings_result|.

  // Trust settings may be scoped to a single application, by checking that the
  // code signing identity of the current application matches the serialized
  // code signing identity in the kSecTrustSettingsApplication key.
  // As this is not presently supported, skip any trust settings scoped to the
  // application.
  if (CFDictionaryContainsKey(trust_dict, kSecTrustSettingsApplication))
    return TrustStatus::UNSPECIFIED;

  // Trust settings may be scoped using policy-specific constraints. For
  // example, SSL trust settings might be scoped to a single hostname, or EAP
  // settings specific to a particular WiFi network.
  // As this is not presently supported, skip any policy-specific trust
  // settings.
  if (CFDictionaryContainsKey(trust_dict, kSecTrustSettingsPolicyString))
    return TrustStatus::UNSPECIFIED;

  // Ignoring kSecTrustSettingsKeyUsage for now; it does not seem relevant to
  // the TLS case.

  // If the trust settings are scoped to a specific policy (via
  // kSecTrustSettingsPolicy), ensure that the policy is the same policy as
  // |target_policy_oid|. If there is no kSecTrustSettingsPolicy key, it's
  // considered a match for all policies.
  if (CFDictionaryContainsKey(trust_dict, kSecTrustSettingsPolicy)) {
    SecPolicyRef policy_ref = GetValueFromDictionary<SecPolicyRef>(trust_dict, kSecTrustSettingsPolicy);
    if (!policy_ref) {
      return TrustStatus::UNSPECIFIED;
    }
    ScopedCFTypeRef<CFDictionaryRef> policy_dict(SecPolicyCopyProperties(policy_ref));

    // kSecPolicyOid is guaranteed to be present in the policy dictionary.
    CFStringRef policy_oid = GetValueFromDictionary<CFStringRef>(policy_dict.get(), kSecPolicyOid);

    if (!CFEqual(policy_oid, target_policy_oid))
      return TrustStatus::UNSPECIFIED;
  }

  // If kSecTrustSettingsResult is not present in the trust dict,
  // kSecTrustSettingsResultTrustRoot is assumed.
  int trust_settings_result = kSecTrustSettingsResultTrustRoot;
  if (CFDictionaryContainsKey(trust_dict, kSecTrustSettingsResult)) {
    CFNumberRef trust_settings_result_ref = GetValueFromDictionary<CFNumberRef>(trust_dict, kSecTrustSettingsResult);
    if (!trust_settings_result_ref ||
        !CFNumberGetValue(trust_settings_result_ref, kCFNumberIntType, &trust_settings_result)) {
      return TrustStatus::UNSPECIFIED;
    }
  }

  if (trust_settings_result == kSecTrustSettingsResultDeny)
    return TrustStatus::DISTRUSTED;

  // This is a bit of a hack: if the cert is self-issued allow either
  // kSecTrustSettingsResultTrustRoot or kSecTrustSettingsResultTrustAsRoot on
  // the basis that SecTrustSetTrustSettings should not allow creating an
  // invalid trust record in the first place. (The spec is that
  // kSecTrustSettingsResultTrustRoot can only be applied to root(self-signed)
  // certs and kSecTrustSettingsResultTrustAsRoot is used for other certs.)
  // This hack avoids having to check the signature on the cert which is slow
  // if using the platform APIs, and may require supporting MD5 signature
  // algorithms on some older OSX versions or locally added roots, which is
  // undesirable in the built-in signature verifier.
  if (is_self_issued) {
    return (trust_settings_result == kSecTrustSettingsResultTrustRoot ||
            trust_settings_result == kSecTrustSettingsResultTrustAsRoot)
               ? TrustStatus::TRUSTED
               : TrustStatus::UNSPECIFIED;
  }

  // kSecTrustSettingsResultTrustAsRoot can only be applied to non-root certs.
  return (trust_settings_result == kSecTrustSettingsResultTrustAsRoot) ? TrustStatus::TRUSTED
                                                                       : TrustStatus::UNSPECIFIED;
}

// Returns true if the trust settings array |trust_settings| for a certificate
// that |is_self_issued| should be treated as a trust anchor.
TrustStatus IsTrustSettingsTrustedForPolicy(CFArrayRef trust_settings,
                                            bool is_self_issued,
                                            const CFStringRef policy_oid) {
  // An empty trust settings array (that is, the trust_settings parameter
  // returns a valid but empty CFArray) means "always trust this certificate"
  // with an overall trust setting for the certificate of
  // kSecTrustSettingsResultTrustRoot.
  if (CFArrayGetCount(trust_settings) == 0) {
    return is_self_issued ? TrustStatus::TRUSTED : TrustStatus::UNSPECIFIED;
  }

  for (CFIndex i = 0, settings_count = CFArrayGetCount(trust_settings); i < settings_count; ++i) {
    CFDictionaryRef trust_dict =
        reinterpret_cast<CFDictionaryRef>(const_cast<void*>(CFArrayGetValueAtIndex(trust_settings, i)));
    TrustStatus trust = IsTrustDictionaryTrustedForPolicy(trust_dict, is_self_issued, policy_oid);
    if (trust != TrustStatus::UNSPECIFIED)
      return trust;
  }
  return TrustStatus::UNSPECIFIED;
}

TrustStatus IsCertificateTrustedForPolicy(const bssl::ParsedCertificate* cert,
                                          SecCertificateRef cert_handle,
                                          const CFStringRef policy_oid) {
  const bool is_self_issued = cert->normalized_subject() == cert->normalized_issuer();

  // Evaluate user trust domain, then admin. User settings can override
  // admin (and both override the system domain, but we don't check that).
  for (const auto& trust_domain : {kSecTrustSettingsDomainUser, kSecTrustSettingsDomainAdmin}) {
    ScopedCFTypeRef<CFArrayRef> trust_settings;
    OSStatus err;
    err = SecTrustSettingsCopyTrustSettings(cert_handle, trust_domain, trust_settings.InitializeInto());
    if (err != errSecSuccess) {
      if (err == errSecItemNotFound) {
        // No trust settings for that domain.. try the next.
        continue;
      }
      // OSSTATUS_LOG(ERROR, err) << "SecTrustSettingsCopyTrustSettings error";
      LOG(ERROR) << "SecTrustSettingsCopyTrustSettings error: " << DescriptionFromOSStatus(err);
      continue;
    }
    TrustStatus trust = IsTrustSettingsTrustedForPolicy(trust_settings.get(), is_self_issued, policy_oid);
    if (trust != TrustStatus::UNSPECIFIED)
      return trust;
  }

  // No trust settings, or none of the settings were for the correct policy, or
  // had the correct trust result.
  return TrustStatus::UNSPECIFIED;
}

// Helper method to check if an EKU is present in a std::vector of EKUs.
bool HasEKU(const std::vector<bssl::der::Input>& list, const bssl::der::Input& eku) {
  for (const auto& oid : list) {
    if (oid == eku)
      return true;
  }
  return false;
}

// Returns true if |cert| would never be a valid intermediate. (A return
// value of false does not imply that it is valid.) This is an optimization
// to avoid using memory for caching certs that would never lead to a valid
// chain. It's not intended to exhaustively test everything that
// VerifyCertificateChain does, just to filter out some of the most obviously
// unusable certs.
bool IsNotAcceptableIntermediate(const bssl::ParsedCertificate* cert, const CFStringRef policy_oid) {
  if (!cert->has_basic_constraints() || !cert->basic_constraints().is_ca) {
    return true;
  }

  // EKU filter is only implemented for TLS server auth since that's all we
  // actually care about.
  if (cert->has_extended_key_usage() && CFEqual(policy_oid, kSecPolicyAppleSSL) &&
      !HasEKU(cert->extended_key_usage(), bssl::der::Input(bssl::kAnyEKU)) &&
      !HasEKU(cert->extended_key_usage(), bssl::der::Input(bssl::kServerAuth))) {
    return true;
  }

  // TODO(mattm): filter on other things too? (key usage, ...?)
  return false;
}

}  // namespace
#endif

static bool found_isrg_root_x1 = false;
static bool found_isrg_root_x2 = false;
static bool found_digicert_root_g2 = false;
static bool found_gts_root_r4 = false;

void print_openssl_error() {
  const char* file;
  int line;
  while (uint32_t error = ERR_get_error_line(&file, &line)) {
    char buf[120];
    ERR_error_string_n(error, buf, sizeof(buf));
    ::gurl_base::logging::LogMessage(file, line, ::gurl_base::logging::LOGGING_ERROR).stream()
        << "OpenSSL error: " << buf;
  }
}

static bool load_ca_cert_to_x509_trust(X509_STORE* store, bssl::UniquePtr<X509> cert) {
  char buf[4096] = {};
  const char* const subject_name = X509_NAME_oneline(X509_get_subject_name(cert.get()), buf, sizeof(buf));

  if (X509_cmp_current_time(X509_get0_notBefore(cert.get())) < 0 &&
      X509_cmp_current_time(X509_get0_notAfter(cert.get())) >= 0) {
    // look at the CN field for ISRG Root X1 and ISRG_Root X2 ca certificates
    int lastpos = -1;
    for (;;) {
      lastpos = X509_NAME_get_index_by_NID(X509_get_subject_name(cert.get()), NID_commonName, lastpos);
      if (lastpos == -1) {
        break;
      }

      X509_NAME_ENTRY* entry = X509_NAME_get_entry(X509_get_subject_name(cert.get()), lastpos);

      const ASN1_STRING* value = X509_NAME_ENTRY_get_data(entry);
      std::string_view commonName((const char*)ASN1_STRING_get0_data(value), ASN1_STRING_length(value));
      using std::string_view_literals::operator""sv;
      if (commonName == "ISRG Root X1"sv) {
        VLOG(1) << "Loading ISRG Root X1 CA";
        found_isrg_root_x1 = true;
      }
      if (commonName == "ISRG Root X2"sv) {
        VLOG(1) << "Loading ISRG Root X2 CA";
        found_isrg_root_x2 = true;
      }
      if (commonName == "DigiCert Global Root G2"sv) {
        VLOG(1) << "Loading DigiCert Global Root G2 CA";
        found_digicert_root_g2 = true;
      }
      if (commonName == "GTS Root R4"sv) {
        VLOG(1) << "Loading GTS Root R4 CA";
        found_gts_root_r4 = true;
      }
    }

    if (X509_STORE_add_cert(store, cert.get()) == 1) {
      VLOG(2) << "Loaded ca: " << subject_name;
      return true;
    } else {
      print_openssl_error();
      LOG(WARNING) << "Loading ca failure with: " << subject_name;
    }
  } else {
    LOG(WARNING) << "Ignore inactive cert: " << subject_name;
  }
  return false;
}

static bool load_ca_content_to_x509_trust(X509_STORE* store, std::string_view cacert) {
  bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(cacert.data(), cacert.size()));
  bssl::UniquePtr<X509> cert(PEM_read_bio_X509(bio.get(), nullptr, 0, nullptr));
  if (!cert) {
    print_openssl_error();
    LOG(WARNING) << "Loading ca failure: with " << cacert;
    return false;
  }
  return load_ca_cert_to_x509_trust(store, std::move(cert));
}

static constexpr std::string_view kEndCertificateMark = "-----END CERTIFICATE-----\n";
int load_ca_to_ssl_ctx_from_mem(SSL_CTX* ssl_ctx, std::string_view cadata) {
  X509_STORE* store = nullptr;
  int count = 0;
  store = SSL_CTX_get_cert_store(ssl_ctx);
  if (!store) {
    LOG(WARNING) << "Can't get SSL CTX cert store";
    goto out;
  }
  for (size_t pos = 0, end = pos; end < cadata.size(); pos = end) {
    end = cadata.find(kEndCertificateMark, pos);
    if (end == std::string_view::npos) {
      break;
    }
    end += kEndCertificateMark.size();

    std::string_view cacert = cadata.substr(pos, end - pos);
    if (load_ca_content_to_x509_trust(store, cacert)) {
      ++count;
    }
  }

out:
  VLOG(2) << "Loaded ca from memory: " << count << " certificates";
  return count;
}

static int load_ca_to_ssl_ctx_bundle(SSL_CTX* ssl_ctx, const std::string& bundle_path) {
  PlatformFile pf = OpenReadFile(bundle_path);
  if (pf == gurl_base::kInvalidPlatformFile) {
    return 0;
  }
  gurl_base::MemoryMappedFile mappedFile;
  // take ownship of pf
  if (!(mappedFile.Initialize(pf, gurl_base::MemoryMappedFile::Region::kWholeFile))) {
    LOG(ERROR) << "Couldn't mmap file: " << bundle_path;
    return 0;  // To debug http://crbug.com/445616.
  }

  std::string_view buffer(reinterpret_cast<const char*>(mappedFile.data()), mappedFile.length());

  return load_ca_to_ssl_ctx_from_mem(ssl_ctx, buffer);
}

static int load_ca_to_ssl_ctx_path(SSL_CTX* ssl_ctx, const std::string& dir_path) {
  int count = 0;

#ifdef _WIN32
  std::wstring wdir_path = SysUTF8ToWide(dir_path);
  _WDIR* dir;
  struct _wdirent* dent;
  dir = _wopendir(wdir_path.c_str());
  if (dir != nullptr) {
    while ((dent = _wreaddir(dir)) != nullptr) {
      if (dent->d_type != DT_REG && dent->d_type != DT_LNK) {
        continue;
      }
      std::filesystem::path wca_bundle = std::filesystem::path(wdir_path) / dent->d_name;
      std::string ca_bundle = SysWideToUTF8(wca_bundle);
      int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
      if (result > 0) {
        VLOG(1) << "Loaded cert from: " << ca_bundle << " with " << result << " certificates";
        count += result;
      }
    }
    _wclosedir(dir);
  }
#else
  DIR* dir;
  struct dirent* dent;
  dir = opendir(dir_path.c_str());
  if (dir != nullptr) {
    while ((dent = readdir(dir)) != nullptr) {
      if (dent->d_type != DT_REG && dent->d_type != DT_LNK) {
        continue;
      }
      if (dent->d_name[0] == '.') {
        continue;
      }
      std::string ca_bundle = absl::StrCat(dir_path, "/", dent->d_name);
      int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
      if (result > 0) {
        VLOG(1) << "Loaded ca cert from: " << ca_bundle << " with " << result << " certificates";
        count += result;
      }
    }
    closedir(dir);
  }
#endif

  return count;
}

static std::optional<int> load_ca_to_ssl_ctx_yass_ca_bundle(SSL_CTX* ssl_ctx) {
#ifdef _WIN32
#define CA_BUNDLE L"yass-ca-bundle.crt"
  // The windows version will automatically look for a CA certs file named 'ca-bundle.crt',
  // either in the same directory as yass.exe, or in the Current Working Directory,
  // or in any folder along your PATH.

  std::vector<std::filesystem::path> ca_bundles;

  // 1. search under executable directory
  std::wstring exe_path;
  CHECK(GetExecutablePath(&exe_path));
  std::filesystem::path exe_dir = std::filesystem::path(exe_path).parent_path();

  ca_bundles.push_back(exe_dir / CA_BUNDLE);

  // 2. search under current directory
  std::wstring current_dir;
  {
    wchar_t buf[32767];
    DWORD ret = GetCurrentDirectoryW(sizeof(buf), buf);
    if (ret == 0) {
      PLOG(FATAL) << "GetCurrentDirectoryW failed";
    }
    // the return value specifies the number of characters that are written to
    // the buffer, not including the terminating null character.
    current_dir = std::wstring(buf, ret);
  }

  ca_bundles.push_back(std::filesystem::path(current_dir) / CA_BUNDLE);

  // 3. search under path directory
  std::string path;
  {
    wchar_t buf[32767];
    DWORD ret = GetEnvironmentVariableW(L"PATH", buf, sizeof(buf));
    if (ret == 0) {
      PLOG(FATAL) << "GetEnvironmentVariableW failed on PATH";
    }
    // the return value is the number of characters stored in the buffer pointed
    // to by lpBuffer, not including the terminating null character.
    path = SysWideToUTF8(std::wstring(buf, ret));
  }
  std::vector<std::string> paths = absl::StrSplit(path, ';');
  for (const auto& path : paths) {
    if (path.empty())
      continue;
    ca_bundles.push_back(std::filesystem::path(path) / CA_BUNDLE);
  }

  for (const auto& wca_bundle : ca_bundles) {
    auto ca_bundle = SysWideToUTF8(wca_bundle);
    VLOG(1) << "Attempt to load ca bundle from: " << ca_bundle;
    int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
    if (result > 0) {
      LOG(INFO) << "Loaded ca bundle from: " << ca_bundle << " with " << result << " certificates";
      return result;
    }
  }
#undef CA_BUNDLE
#endif

  return std::nullopt;
}

static std::optional<int> load_ca_to_ssl_ctx_cacert(SSL_CTX* ssl_ctx) {
  if (absl::GetFlag(FLAGS_ca_native)) {
    int result = load_ca_to_ssl_ctx_system(ssl_ctx);
    if (!result) {
      LOG(WARNING) << "Loading ca bundle failure from system";
    }
    return result;
  }
  std::string ca_bundle = absl::GetFlag(FLAGS_cacert);
  if (!ca_bundle.empty()) {
    int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
    if (result) {
      LOG(INFO) << "Loaded ca bundle from: " << ca_bundle << " with " << result << " certificates";
    } else {
      print_openssl_error();
      LOG(WARNING) << "Loading ca bundle failure from: " << ca_bundle;
    }
    return result;
  }
  std::string ca_path = absl::GetFlag(FLAGS_capath);
  if (!ca_path.empty()) {
    int result = load_ca_to_ssl_ctx_path(ssl_ctx, ca_path);
    if (result) {
      LOG(INFO) << "Loaded ca from directory: " << ca_path << " with " << result << " certificates";
    } else {
      LOG(WARNING) << "Loading ca directory failure from: " << ca_path;
    }
    return result;
  }
  return load_ca_to_ssl_ctx_yass_ca_bundle(ssl_ctx);
}

#ifdef _WIN32
// Returns true if the cert can be used for server authentication, based on
// certificate properties.
//
// While there are a variety of certificate properties that can affect how
// trust is computed, the main property is CERT_ENHKEY_USAGE_PROP_ID, which
// is intersected with the certificate's EKU extension (if present).
// The intersection is documented in the Remarks section of
// CertGetEnhancedKeyUsage, and is as follows:
// - No EKU property, and no EKU extension = Trusted for all purpose
// - Either an EKU property, or EKU extension, but not both = Trusted only
//   for the listed purposes
// - Both an EKU property and an EKU extension = Trusted for the set
//   intersection of the listed purposes
// CertGetEnhancedKeyUsage handles this logic, and if an empty set is
// returned, the distinction between the first and third case can be
// determined by GetLastError() returning CRYPT_E_NOT_FOUND.
//
// See:
// https://docs.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-certgetenhancedkeyusage
//
// If we run into any errors reading the certificate properties, we fail
// closed.
bool IsCertTrustedForServerAuth(PCCERT_CONTEXT cert) {
  DWORD usage_size = 0;

  if (!CertGetEnhancedKeyUsage(cert, 0, nullptr, &usage_size)) {
    return false;
  }

  std::vector<BYTE> usage_bytes(usage_size);
  CERT_ENHKEY_USAGE* usage = reinterpret_cast<CERT_ENHKEY_USAGE*>(usage_bytes.data());
  if (!CertGetEnhancedKeyUsage(cert, 0, usage, &usage_size)) {
    return false;
  }

  if (usage->cUsageIdentifier == 0) {
    // check GetLastError
    HRESULT error_code = GetLastError();

    switch (error_code) {
      case CRYPT_E_NOT_FOUND:
        return true;
      case S_OK:
        return false;
      default:
        return false;
    }
  }
  for (DWORD i = 0; i < usage->cUsageIdentifier; i++) {
    std::string_view eku = std::string_view(usage->rgpszUsageIdentifier[i]);
    if ((eku == szOID_PKIX_KP_SERVER_AUTH) || (eku == szOID_ANY_ENHANCED_KEY_USAGE)) {
      return true;
    }
  }
  return false;
}

int load_ca_to_ssl_store_from_schannel_store(X509_STORE* store, HCERTSTORE cert_store) {
  PCCERT_CONTEXT cert_context = NULL;
  int count = 0;

  while ((cert_context = CertEnumCertificatesInStore(cert_store, cert_context))) {
    const char* data = reinterpret_cast<const char*>(cert_context->pbCertEncoded);
    size_t len = cert_context->cbCertEncoded;
    bssl::UniquePtr<CRYPTO_BUFFER> buffer = net::x509_util::CreateCryptoBuffer(std::string_view(data, len));
    bssl::UniquePtr<X509> cert(X509_parse_from_buffer(buffer.get()));
    if (!cert) {
      print_openssl_error();
      LOG(WARNING) << "Loading ca failure from: cert store";
      continue;
    }
    if (!IsCertTrustedForServerAuth(cert_context)) {
      char buf[4096] = {};
      const char* const subject_name = X509_NAME_oneline(X509_get_subject_name(cert.get()), buf, sizeof(buf));
      LOG(WARNING) << "Skip cert without server auth support: " << subject_name;
      continue;
    }
    if (load_ca_cert_to_x509_trust(store, std::move(cert))) {
      ++count;
    }
  }

  return count;
}

void GatherEnterpriseCertsForLocation(LPCSTR provider, HCERTSTORE cert_store, DWORD location, LPCWSTR store_name) {
  if (!(location == CERT_SYSTEM_STORE_LOCAL_MACHINE || location == CERT_SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY ||
        location == CERT_SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE || location == CERT_SYSTEM_STORE_CURRENT_USER ||
        location == CERT_SYSTEM_STORE_CURRENT_USER_GROUP_POLICY)) {
    return;
  }

  DWORD flags = location | CERT_STORE_OPEN_EXISTING_FLAG | CERT_STORE_READONLY_FLAG;

  HCERTSTORE enterprise_root_store = NULL;

  enterprise_root_store = CertOpenStore(provider, 0, NULL, flags, store_name);
  if (!enterprise_root_store) {
    return;
  }
  // Priority of the opened cert store in the collection does not matter, so set
  // everything to priority 0.
  CertAddStoreToCollection(cert_store, enterprise_root_store,
                           /*dwUpdateFlags=*/0, /*dwPriority=*/0);
  if (!CertCloseStore(enterprise_root_store, 0)) {
    PLOG(WARNING) << "CertCloseStore() call failed";
  }
}
#endif  // _WIN32
#if BUILDFLAG(IS_MAC)
int load_ca_to_ssl_store_from_sec_trust_domain(X509_STORE* store, SecTrustSettingsDomain domain) {
  const CFStringRef policy_oid = kSecPolicyAppleSSL;
  CFArrayRef certs;
  OSStatus err;
  CFIndex size;
  int count = 0;

  err = SecTrustSettingsCopyCertificates(domain, &certs);
  // Note: SecTrustSettingsCopyCertificates can legitimately return
  // errSecNoTrustSettings if there are no trust settings in |domain|.
  if (err == errSecNoTrustSettings) {
    goto out;
  }
  if (err != errSecSuccess) {
    LOG(ERROR) << "SecTrustSettingsCopyCertificates error: " << DescriptionFromOSStatus(err) << " at domain 0x"
               << std::hex << domain;
    goto out;
  }

  size = CFArrayGetCount(certs);
  for (CFIndex i = 0; i < size; ++i) {
    SecCertificateRef sec_cert = (SecCertificateRef)CFArrayGetValueAtIndex(certs, i);
    ScopedCFTypeRef<CFDataRef> der_data(SecCertificateCopyData(sec_cert));

    if (!der_data) {
      LOG(ERROR) << "SecCertificateCopyData error";
      continue;
    }
    const char* data = (const char*)CFDataGetBytePtr(der_data.get());
    CFIndex len = CFDataGetLength(der_data.get());

    bssl::UniquePtr<CRYPTO_BUFFER> buffer = net::x509_util::CreateCryptoBuffer(std::string_view(data, len));

    // keep reference to buffer
    bssl::UniquePtr<X509> cert(X509_parse_from_buffer(buffer.get()));
    if (!cert) {
      print_openssl_error();
      LOG(WARNING) << "Loading ca failure from: SecTrust";
      continue;
    }

    char buf[4096] = {};
    const char* const subject_name = X509_NAME_oneline(X509_get_subject_name(cert.get()), buf, sizeof(buf));

    bssl::CertErrors errors;
    bssl::ParseCertificateOptions options;
    options.allow_invalid_serial_numbers = true;
    std::shared_ptr<const bssl::ParsedCertificate> parsed_cert =
        bssl::ParsedCertificate::Create(std::move(buffer), options, &errors);
    if (!parsed_cert) {
      LOG(ERROR) << "Error parsing certificate:\n" << errors.ToDebugString();
      continue;
    }

    TrustStatus trust_status = IsCertificateTrustedForPolicy(parsed_cert.get(), sec_cert, policy_oid);

    if (trust_status == TrustStatus::DISTRUSTED) {
      LOG(WARNING) << "Ignore distrusted cert: " << subject_name;
      continue;
    }

    if (IsNotAcceptableIntermediate(parsed_cert.get(), policy_oid)) {
      LOG(WARNING) << "Ignore Unacceptable cert: " << subject_name;
      continue;
    }

    if (load_ca_cert_to_x509_trust(store, std::move(cert))) {
      ++count;
    }
  }

  CFRelease(certs);

out:
  VLOG(1) << "Loaded ca from SecTrust: " << count << " certificates at domain 0x" << std::hex << domain;
  return count;
}
#endif  // BUILDFLAG(IS_MAC)

int load_ca_to_ssl_ctx_system(SSL_CTX* ssl_ctx) {
#ifdef _WIN32
  HCERTSTORE root_store = NULL;
  int count = 0;

  X509_STORE* store = SSL_CTX_get_cert_store(ssl_ctx);
  if (!store) {
    LOG(WARNING) << "Can't get SSL CTX cert store";
    goto out;
  }
  root_store = CertOpenStore(CERT_STORE_PROV_COLLECTION, 0, NULL, 0, nullptr);
  if (!root_store) {
    LOG(WARNING) << "Can't get cert store";
    goto out;
  }
  // Grab the user-added roots.
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE, L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY,
                                   L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE,
                                   L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER, L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER_GROUP_POLICY,
                                   L"ROOT");

  // Grab the user-added intermediates (optional).
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE, L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY,
                                   L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE,
                                   L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER, L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER_GROUP_POLICY,
                                   L"CA");

  count = load_ca_to_ssl_store_from_schannel_store(store, root_store);

  if (!CertCloseStore(root_store, 0)) {
    PLOG(WARNING) << "CertCloseStore() call failed";
  }

out:
  LOG(INFO) << "Loaded ca from SChannel: " << count << " certificates";
  return count;
#elif BUILDFLAG(IS_MAC)
  X509_STORE* store = SSL_CTX_get_cert_store(ssl_ctx);
  int count = 0;
  if (!store) {
    LOG(WARNING) << "Can't get SSL CTX cert store";
    goto out;
  }
  count += load_ca_to_ssl_store_from_sec_trust_domain(store, kSecTrustSettingsDomainSystem);
  count += load_ca_to_ssl_store_from_sec_trust_domain(store, kSecTrustSettingsDomainAdmin);
  count += load_ca_to_ssl_store_from_sec_trust_domain(store, kSecTrustSettingsDomainUser);

out:
  LOG(INFO) << "Loaded ca from SecTrust: " << count << " certificates";
  return count;
#elif BUILDFLAG(IS_IOS)
  return 0;
#else
  int count = 0;
  // cert list copied from golang src/crypto/x509/root_unix.go
  static const char* ca_bundle_paths[] = {
      "/etc/ssl/certs/ca-certificates.crt",      // Debian/Ubuntu/Gentoo etc.
      "/etc/pki/tls/certs/ca-bundle.crt",        // Fedora/RHEL
      "/etc/ssl/ca-bundle.pem",                  // OpenSUSE
      "/etc/openssl/certs/ca-certificates.crt",  // NetBSD
      "/etc/ssl/cert.pem",                       // OpenBSD
      "/usr/local/share/certs/ca-root-nss.crt",  // FreeBSD/DragonFly
      "/etc/pki/tls/cacert.pem",                 // OpenELEC
      "/etc/certs/ca-certificates.crt",          // Solaris 11.2+
  };
  for (auto ca_bundle : ca_bundle_paths) {
    int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
    if (result > 0) {
      LOG(INFO) << "Loaded ca bundle from: " << ca_bundle << " with " << result << " certificates";
      count += result;
    }
  }
  static const char* ca_paths[] = {
      "/etc/ssl/certs",                // SLES10/SLES11, https://golang.org/issue/12139
      "/etc/pki/tls/certs",            // Fedora/RHEL
      "/system/etc/security/cacerts",  // Android
  };

  for (auto ca_path : ca_paths) {
    int result = load_ca_to_ssl_ctx_path(ssl_ctx, ca_path);
    if (result > 0) {
      LOG(INFO) << "Loaded ca from directory: " << ca_path << " with " << result << " certificates";
      count += result;
    }
  }
  return count;
#endif
}

int load_ca_to_ssl_ctx_system_extra(SSL_CTX* ssl_ctx) {
#ifdef _WIN32
  HCERTSTORE root_store = NULL;
  int count = 0;

  X509_STORE* store = SSL_CTX_get_cert_store(ssl_ctx);
  if (!store) {
    LOG(WARNING) << "Can't get SSL CTX cert store";
    goto out;
  }
  root_store = CertOpenStore(CERT_STORE_PROV_COLLECTION, 0, NULL, 0, nullptr);
  if (!root_store) {
    LOG(WARNING) << "Can't get cert store";
    goto out;
  }
  // Grab the user-added roots.
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE,
                                   L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY, L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE, L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER,
                                   L"ROOT");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_CURRENT_USER_GROUP_POLICY, L"ROOT");

  // Grab the user-added intermediates (optional).
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store, CERT_SYSTEM_STORE_LOCAL_MACHINE,
                                   L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY, L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE, L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store, CERT_SYSTEM_STORE_CURRENT_USER,
                                   L"CA");
  GatherEnterpriseCertsForLocation(CERT_STORE_PROV_SYSTEM_REGISTRY_W, root_store,
                                   CERT_SYSTEM_STORE_CURRENT_USER_GROUP_POLICY, L"CA");

  count = load_ca_to_ssl_store_from_schannel_store(store, root_store);

  if (!CertCloseStore(root_store, 0)) {
    PLOG(WARNING) << "CertCloseStore() call failed";
  }

out:
  LOG(INFO) << "Loaded user-added ca from SChannel: " << count << " certificates";
  return count;
#elif BUILDFLAG(IS_MAC)
  X509_STORE* store = SSL_CTX_get_cert_store(ssl_ctx);
  int count = 0;
  if (!store) {
    LOG(WARNING) << "Can't get SSL CTX cert store";
    goto out;
  }
  count += load_ca_to_ssl_store_from_sec_trust_domain(store, kSecTrustSettingsDomainAdmin);
  count += load_ca_to_ssl_store_from_sec_trust_domain(store, kSecTrustSettingsDomainUser);

out:
  LOG(INFO) << "Loaded user-added ca from SecTrust: " << count << " certificates";
  return count;
#elif BUILDFLAG(IS_IOS)
  return 0;
#else
  int count = 0;
  // cert list copied from golang src/crypto/x509/root_unix.go
  static const char* ca_bundle_paths[] = {
      "/etc/ssl/certs/ca-certificates.crt",      // Debian/Ubuntu/Gentoo etc.
      "/etc/pki/tls/certs/ca-bundle.crt",        // Fedora/RHEL
      "/etc/ssl/ca-bundle.pem",                  // OpenSUSE
      "/etc/openssl/certs/ca-certificates.crt",  // NetBSD
      "/etc/ssl/cert.pem",                       // OpenBSD
      "/usr/local/share/certs/ca-root-nss.crt",  // FreeBSD/DragonFly
      "/etc/pki/tls/cacert.pem",                 // OpenELEC
      "/etc/certs/ca-certificates.crt",          // Solaris 11.2+
  };
  for (auto ca_bundle : ca_bundle_paths) {
    int result = load_ca_to_ssl_ctx_bundle(ssl_ctx, ca_bundle);
    if (result > 0) {
      LOG(INFO) << "Loaded ca bundle from: " << ca_bundle << " with " << result << " certificates";
      count += result;
    }
  }
  static const char* ca_paths[] = {
      "/etc/ssl/certs",                // SLES10/SLES11, https://golang.org/issue/12139
      "/etc/pki/tls/certs",            // Fedora/RHEL
      "/system/etc/security/cacerts",  // Android
  };

  for (auto ca_path : ca_paths) {
    int result = load_ca_to_ssl_ctx_path(ssl_ctx, ca_path);
    if (result > 0) {
      LOG(INFO) << "Loaded ca from directory: " << ca_path << " with " << result << " certificates";
      count += result;
    }
  }
  return count;
#endif
}

// loading ca certificates:
// 1. load --capath and --cacert certificates
// 2. load ca bundle from in sequence
//    - builtin ca bundle if specified
//    - yass-ca-bundle.crt if present (windows)
//    - system ca certificates
// 3. force fallback to builtin ca bundle if step 2 failes
void load_ca_to_ssl_ctx(SSL_CTX* ssl_ctx) {
  found_isrg_root_x1 = false;
  found_isrg_root_x2 = false;
  found_digicert_root_g2 = false;
  found_gts_root_r4 = false;
  if (load_ca_to_ssl_ctx_cacert(ssl_ctx).has_value()) {
    goto done;
  }

  load_ca_to_ssl_ctx_system_extra(ssl_ctx);
  {
    std::string_view ca_bundle_content(_binary_ca_bundle_crt_start,
                                       _binary_ca_bundle_crt_end - _binary_ca_bundle_crt_start);
    int result = load_ca_to_ssl_ctx_from_mem(ssl_ctx, ca_bundle_content);
    LOG(INFO) << "Loaded builtin ca bundle with: " << result << " ceritificates";
  }

done:
  // TODO we can add the missing CA if required
  if (!found_isrg_root_x1 || !found_isrg_root_x2 || !found_digicert_root_g2 || !found_gts_root_r4) {
    if (!found_isrg_root_x1) {
      LOG(INFO) << "Missing ISRG Root X1 CA";
    }
    if (!found_isrg_root_x2) {
      LOG(INFO) << "Missing ISRG Root X2 CA";
    }
    if (!found_digicert_root_g2) {
      LOG(INFO) << "Missing DigiCert Global Root G2 CA";
    }
    if (!found_gts_root_r4) {
      LOG(INFO) << "Missing GTS Root R4 CA";
    }
    std::string_view ca_content(_binary_supplementary_ca_bundle_crt_start,
                                _binary_supplementary_ca_bundle_crt_end - _binary_supplementary_ca_bundle_crt_start);
    int result = load_ca_to_ssl_ctx_from_mem(ssl_ctx, ca_content);
    LOG(INFO) << "Loaded supplementary ca bundle with " << result << " certificates";
  }
}
