// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2019-2024 Chilledheart  */

#ifndef H_NET_CONTENT_SERVER
#define H_NET_CONTENT_SERVER

#include <absl/container/flat_hash_map.h>
#include <absl/flags/flag.h>
#include <absl/strings/str_format.h>
#include <algorithm>
#include <array>
#include <functional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <base/memory/scoped_refptr.h>

#include "config/config_tls.hpp"
#include "core/logging.hpp"
#include "core/utils.hpp"
#include "crypto/crypter_export.hpp"
#include "net/asio.hpp"
#include "net/connection.hpp"
#include "net/network.hpp"
#include "net/protocol.hpp"
#include "net/ssl_client_session_cache.hpp"
#include "net/ssl_socket.hpp"
#include "net/x509_util.hpp"

#define MAX_LISTEN_ADDRESSES 30

namespace net {

/// An interface used to provide service
template <typename T>
class ContentServer {
 public:
  using ConnectionType = typename T::ConnectionType;
  using tlsext_ctx_t = typename ConnectionType::tlsext_ctx_t;
  class Delegate {
   public:
    virtual ~Delegate() = default;
    virtual void OnConnect(int connection_id) = 0;
    virtual void OnDisconnect(int connection_id) = 0;
  };

 public:
  explicit ContentServer(asio::io_context& io_context,
                         std::string_view remote_host_ips = {},
                         std::string_view remote_host_sni = {},
                         uint16_t remote_port = {},
                         std::string_view upstream_certificate = {},
                         std::string_view certificate = {},
                         std::string_view private_key = {},
                         ContentServer::Delegate* delegate = nullptr)
      : io_context_(io_context),
        work_guard_(
            std::make_unique<asio::executor_work_guard<asio::io_context::executor_type>>(io_context_.get_executor())),
        remote_host_ips_(remote_host_ips),
        remote_host_sni_(remote_host_sni),
        remote_port_(remote_port),
        upstream_https_fallback_(CIPHER_METHOD_IS_HTTPS_FALLBACK(absl::GetFlag(FLAGS_method).method)),
        https_fallback_(upstream_https_fallback_),
        enable_upstream_tls_(CIPHER_METHOD_IS_TLS(absl::GetFlag(FLAGS_method).method)),
        enable_tls_(enable_upstream_tls_),
        upstream_certificate_(upstream_certificate),
        certificate_(certificate),
        private_key_(private_key),
        delegate_(delegate) {
    upstream_https_fallback_ &= T::Type == CONNECTION_FACTORY_CLIENT;
    https_fallback_ &= T::Type == CONNECTION_FACTORY_SERVER;
    enable_upstream_tls_ &= T::Type == CONNECTION_FACTORY_CLIENT;
    enable_tls_ &= T::Type == CONNECTION_FACTORY_SERVER;
    DCHECK_LE(remote_host_sni_.size(), (unsigned int)TLSEXT_MAXLEN_host_name);

    VLOG(1) << "ContentServer (" << T::Name << ") allocated memory";
  }

  ~ContentServer() {
    VLOG(1) << "ContentServer (" << T::Name << ") freed memory";

    CHECK_EQ(pending_next_listen_ctxes_.size(), 0u) << "ContentServer freed on pending listen ctx";
    CHECK_EQ(opened_connections_, 0u) << "ContentServer freed on non-closed connections";
    CHECK_EQ(connection_map_.size(), 0u) << "ContentServer freed on non-closed connections";

    client_instance_ = nullptr;

    work_guard_.reset();
  }

  ContentServer(const ContentServer&) = delete;
  ContentServer& operator=(const ContentServer&) = delete;

  // Retrieve last local endpoint
  const asio::ip::tcp::endpoint& endpoint() const {
    DCHECK_NE(next_listen_ctx_, 0) << "Server should listen to some address";
    return listen_ctxs_[next_listen_ctx_ - 1].endpoint;
  }

  void listen(const asio::ip::tcp::endpoint& endpoint,
              std::string_view server_name,
              int backlog,
              asio::error_code& ec) {
    if (next_listen_ctx_ >= MAX_LISTEN_ADDRESSES) {
      ec = asio::error::already_started;
      return;
    }
    if (server_name.size() > TLSEXT_MAXLEN_host_name) {
      ec = asio::error::invalid_argument;
      return;
    }
    ListenCtx& ctx = listen_ctxs_[next_listen_ctx_];
    ctx.server_name = server_name;
    ctx.endpoint = endpoint;
    ctx.acceptor = std::make_unique<asio::ip::tcp::acceptor>(io_context_);

    ctx.acceptor->open(endpoint.protocol(), ec);
    if (ec) {
      return;
    }
    if (absl::GetFlag(FLAGS_reuse_port)) {
      ctx.acceptor->set_option(asio::ip::tcp::acceptor::reuse_address(true), ec);
      SetSOReusePort(ctx.acceptor->native_handle(), ec);
    }
    if (ec) {
      return;
    }
    SetTCPFastOpen(ctx.acceptor->native_handle(), ec);
    if (ec) {
      return;
    }
    ctx.acceptor->bind(endpoint, ec);
    if (ec) {
      return;
    }
    ctx.acceptor->listen(backlog, ec);
    if (ec) {
      return;
    }
    ctx.endpoint = ctx.acceptor->local_endpoint(ec);
    if (ec) {
      return;
    }
    if (enable_upstream_tls_) {
      setup_upstream_ssl_ctx(ec);
      if (ec) {
        return;
      }
    }
    if (enable_tls_) {
      setup_ssl_ctx(ec);
      if (ec) {
        return;
      }
    }
    LOG(INFO) << "Listening (" << T::Name << ") on " << ctx.endpoint;
    int listen_ctx_num = next_listen_ctx_++;
    asio::post(io_context_, [this, listen_ctx_num]() { accept(listen_ctx_num); });
  }

  // Allow called from different threads
  void shutdown() {
    asio::post(io_context_, [this]() {
      for (int i = 0; i < next_listen_ctx_; ++i) {
        ListenCtx& ctx = listen_ctxs_[i];
        if (ctx.acceptor) {
          asio::error_code ec;
          ctx.acceptor->close(ec);
          ctx.acceptor.reset();
          if (ec) {
            LOG(WARNING) << "Connections (" << T::Name << ")"
                         << " acceptor (" << ctx.endpoint << ") close failed: " << ec;
          }
        }
      }

      pending_next_listen_ctxes_.clear();

      if (connection_map_.empty()) {
        LOG(WARNING) << "No more connections alive... ready to stop";
        work_guard_.reset();
        in_shutdown_ = false;
      } else {
        LOG(WARNING) << "Waiting for remaining connects: " << connection_map_.size();
        in_shutdown_ = true;
      }
    });
  }
  // Allow called from different threads
  void stop() {
    asio::post(io_context_, [this]() {
      for (int i = 0; i < next_listen_ctx_; ++i) {
        ListenCtx& ctx = listen_ctxs_[i];
        if (ctx.acceptor) {
          asio::error_code ec;
          ctx.acceptor->close(ec);
          ctx.acceptor.reset();
          if (ec) {
            LOG(WARNING) << "Connections (" << T::Name << ")"
                         << " acceptor (" << ctx.endpoint << ") close failed: " << ec;
          }
        }
      }

      pending_next_listen_ctxes_.clear();

      auto connection_map = std::move(connection_map_);
      // FIXME silence some false-positive warning from abseil-cpp
      connection_map_ = absl::flat_hash_map<int, scoped_refptr<ConnectionType>>();
      opened_connections_ = 0;
      for (auto [conn_id, conn] : connection_map) {
        VLOG(1) << "Connections (" << T::Name << ")"
                << " closing Connection: " << conn_id;
        conn->close();
      }

      work_guard_.reset();
    });
  }

  size_t num_of_connections() const { return opened_connections_; }

 private:
  void accept(int listen_ctx_num) {
    ListenCtx& ctx = listen_ctxs_[listen_ctx_num];
    ctx.acceptor->async_accept(
        ctx.peer_endpoint, [this, listen_ctx_num](asio::error_code ec, asio::ip::tcp::socket socket) {
          // acceptor->close might return success as well
          ListenCtx& ctx = listen_ctxs_[listen_ctx_num];
          if (!ctx.acceptor) {
            return;
          }
          // cancelled
          if (ec == asio::error::operation_aborted) {
            return;
          }
          if (ec) {
            LOG(WARNING) << "Acceptor (" << T::Name << ")"
                         << " failed to accept more due to: " << ec;
            work_guard_.reset();
            return;
          }
          tlsext_ctx_t* tlsext_ctx = nullptr;
          if (enable_tls_) {
            tlsext_ctx = new tlsext_ctx_t{this, next_connection_id_, listen_ctx_num};
            setup_ssl_ctx_alpn_cb(tlsext_ctx);
            setup_ssl_ctx_tlsext_cb(tlsext_ctx);
          }
          scoped_refptr<ConnectionType> conn =
              T::Create(io_context_, remote_host_ips_, remote_host_sni_, remote_port_, upstream_https_fallback_,
                        https_fallback_, enable_upstream_tls_, enable_tls_, upstream_ssl_ctx_.get(), ssl_ctx_.get());
          on_accept(conn, std::move(socket), listen_ctx_num, tlsext_ctx);
          if (in_shutdown_) {
            return;
          }
          if (connection_map_.size() >= absl::GetFlag(FLAGS_parallel_max)) {
            LOG(INFO) << "Disabling accepting new connection: " << listen_ctxs_[listen_ctx_num].endpoint;
            pending_next_listen_ctxes_.push_back(listen_ctx_num);
            return;
          }
          accept(listen_ctx_num);
        });
  }

  void on_accept(scoped_refptr<ConnectionType> conn,
                 asio::ip::tcp::socket&& socket,
                 int listen_ctx_num,
                 tlsext_ctx_t* tlsext_ctx) {
    asio::error_code ec;
    ListenCtx& ctx = listen_ctxs_[listen_ctx_num];

    int connection_id = next_connection_id_++;
    socket.non_blocking(true, ec);
    if constexpr (T::Type == CONNECTION_FACTORY_SERVER) {
      SetTCPCongestion(socket.native_handle(), ec);
    }
    SetTCPKeepAlive(socket.native_handle(), ec);
    SetSocketTcpNoDelay(&socket, ec);
    conn->on_accept(std::move(socket), ctx.endpoint, ctx.peer_endpoint, connection_id, tlsext_ctx,
                    ssl_socket_data_index_, ssl_client_session_cache_.get());
    conn->set_disconnect_cb([this, conn]() mutable { on_disconnect(conn); });
    connection_map_.insert(std::make_pair(connection_id, conn));
    ++opened_connections_;
    DCHECK_EQ(connection_map_.size(), opened_connections_);
    if (delegate_) {
      delegate_->OnConnect(connection_id);
    }
    VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " with " << conn->peer_endpoint() << " connected";
    conn->start();
  }

  void on_disconnect(scoped_refptr<ConnectionType> conn) {
    int connection_id = conn->connection_id();
    VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " disconnected (has ref " << std::boolalpha
            << conn->HasAtLeastOneRef() << std::noboolalpha << ")";
    auto iter = connection_map_.find(connection_id);
    if (iter != connection_map_.end()) {
      connection_map_.erase(iter);
      --opened_connections_;
      DCHECK_EQ(connection_map_.size(), (size_t)opened_connections_);
    }
    if (delegate_) {
      delegate_->OnDisconnect(connection_id);
    }
    // reset guard to quit io loop if in shutdown
    if (in_shutdown_) {
      pending_next_listen_ctxes_.clear();
      if (connection_map_.empty()) {
        LOG(WARNING) << "No more connections alive... ready to stop";
        work_guard_.reset();
        in_shutdown_ = false;
      } else {
        LOG(WARNING) << "Waiting for remaining connects: " << connection_map_.size();
      }
    }
    auto listen_ctxes = std::move(pending_next_listen_ctxes_);
    for (int listen_ctx_num : listen_ctxes) {
      LOG(INFO) << "Resuming accepting new connection: " << listen_ctxs_[listen_ctx_num].endpoint;
      accept(listen_ctx_num);
    }
  }

  void setup_ssl_ctx(asio::error_code& ec) {
    ssl_ctx_.reset(::SSL_CTX_new(::TLS_server_method()));
    SSL_CTX* ctx = ssl_ctx_.get();
    if (!ctx) {
      print_openssl_error();
      ec = asio::error::no_memory;
      return;
    }

    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, ::SSL_CTX_get_verify_callback(ctx));

    SSL_CTX_set_session_cache_mode(ctx, SSL_SESS_CACHE_SERVER);

    // Load Certificate Chain Files
    if (private_key_.empty()) {
      private_key_ = g_private_key_content;
      certificate_ = g_certificate_chain_content;
    }

    // Load Certificates (if set)
    if (!private_key_.empty()) {
      CHECK(!certificate_.empty()) << "certificate buffer is not provided";

      bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(certificate_.data(), certificate_.size()));

      static pem_password_cb* callback = [](char* buf, int size, int rwflag, void* userdata) -> int {
        std::string password = absl::GetFlag(FLAGS_private_key_password);
        /* not enough buffer size */
        if (size < (int)password.size()) {
          return -1;
        }
        /* empty password */
        if (password.empty()) {
          return 0;
        }
        memcpy(buf, password.c_str(), password.size());
        return password.size();
      };
      static void* cb_userdata = nullptr;
      bssl::UniquePtr<X509> cert(PEM_read_bio_X509_AUX(bio.get(), nullptr, callback, cb_userdata));
      if (!cert) {
        print_openssl_error();
        ec = asio::error::bad_descriptor;
        return;
      }

      ERR_clear_error();
      int result = SSL_CTX_use_certificate(ctx, cert.get());
      if (result == 0 || ::ERR_peek_error() != 0) {
        print_openssl_error();
        ec = asio::error::bad_descriptor;
        return;
      }

      VLOG(1) << "Using certificate (in-memory)";
      bio.reset(BIO_new_mem_buf(private_key_.data(), private_key_.size()));
      bssl::UniquePtr<EVP_PKEY> pkey(PEM_read_bio_PrivateKey(bio.get(), nullptr, callback, cb_userdata));

      if (SSL_CTX_use_PrivateKey(ctx, pkey.get()) != 1) {
        print_openssl_error();
        ec = asio::error::bad_descriptor;
        return;
      }
      VLOG(1) << "Using privated key (in-memory)";
    }
    SSL_CTX_set_early_data_enabled(ctx, absl::GetFlag(FLAGS_tls13_early_data));

    CHECK(SSL_CTX_set_min_proto_version(ctx, TLS1_2_VERSION));
    CHECK(SSL_CTX_set_max_proto_version(ctx, TLS1_3_VERSION));

    // OpenSSL defaults some options to on, others to off. To avoid ambiguity,
    // set everything we care about to an absolute value.
    SslSetClearMask options;
    options.ConfigureFlag(SSL_OP_NO_COMPRESSION, true);
    options.ConfigureFlag(SSL_OP_ALL, true);

    SSL_CTX_set_options(ctx, options.set_mask);
    SSL_CTX_clear_options(ctx, options.clear_mask);

    // Same as above, this time for the SSL mode.
    SslSetClearMask mode;

    mode.ConfigureFlag(SSL_MODE_RELEASE_BUFFERS, true);

    SSL_CTX_set_mode(ctx, mode.set_mask);
    SSL_CTX_clear_mode(ctx, mode.clear_mask);

    // Use BoringSSL defaults, but disable 3DES and HMAC-SHA1 ciphers in ECDSA.
    // These are the remaining CBC-mode ECDSA ciphers.
    std::string command(kSSLDefaultCiphersList);

#if 0
    // SSLPrivateKey only supports ECDHE-based ciphers because it lacks decrypt.
    if (ssl_server_config_.require_ecdhe || (!pkey_ && private_key_))
      command.append(":!kRSA");

    // Remove any disabled ciphers.
    for (uint16_t id : ssl_server_config_.disabled_cipher_suites) {
      const SSL_CIPHER* cipher = SSL_get_cipher_by_value(id);
      if (cipher) {
        command.append(":!");
        command.append(SSL_CIPHER_get_name(cipher));
      }
    }
#endif

    CHECK(SSL_CTX_set_strict_cipher_list(ctx, command.c_str()));

    // TODO: implement these SSL options
    // SSL_CTX_set_ocsp_response
    // SSL_CTX_set_signed_cert_timestamp_list
    // SSL_CTX_set1_ech_keys

    uint8_t session_ctx_id = 0;
    SSL_CTX_set_session_id_context(ctx, &session_ctx_id, sizeof(session_ctx_id));
    // Deduplicate all certificates minted from the SSL_CTX in memory.
    SSL_CTX_set0_buffer_pool(ctx, x509_util::GetBufferPool());

    load_ca_to_ssl_ctx(ctx);
  }

  void setup_ssl_ctx_alpn_cb(tlsext_ctx_t* tlsext_ctx) {
    SSL_CTX* ctx = ssl_ctx_.get();
    SSL_CTX_set_alpn_select_cb(ctx, &ContentServer::on_alpn_select, tlsext_ctx);
    VLOG(1) << "Alpn support (server) enabled for connection " << next_connection_id_;
  }

  static int on_alpn_select(SSL* ssl,
                            const unsigned char** out,
                            unsigned char* outlen,
                            const unsigned char* in,
                            unsigned int inlen,
                            void* arg) {
    auto tlsext_ctx = reinterpret_cast<tlsext_ctx_t*>(arg);
    auto server = reinterpret_cast<ContentServer*>(tlsext_ctx->server);
    int connection_id = tlsext_ctx->connection_id;
    while (inlen) {
      if (in[0] + 1u > inlen) {
        goto err;
      }
      auto alpn = std::string_view(reinterpret_cast<const char*>(in + 1), in[0]);
      if (!server->https_fallback_ && NextProtoFromString(alpn) == kProtoHTTP2) {
        VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " Alpn support (server) chosen: " << alpn;
        server->set_https_fallback(connection_id, false);
        *out = in + 1;
        *outlen = in[0];

        // Enable ALPS for HTTP/2 with empty data.
        std::vector<uint8_t> data;
        SSL_add_application_settings(ssl, reinterpret_cast<const uint8_t*>(alpn.data()), alpn.size(), data.data(),
                                     data.size());
        return SSL_TLSEXT_ERR_OK;
      }
      if (NextProtoFromString(alpn) == kProtoHTTP11) {
        VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " Alpn support (server) chosen: " << alpn;
        server->set_https_fallback(connection_id, true);
        *out = in + 1;
        *outlen = in[0];
        return SSL_TLSEXT_ERR_OK;
      }
      VLOG(2) << "Connection (" << T::Name << ") " << connection_id << " Alpn support (server) skipped: " << alpn;
      inlen -= 1u + in[0];
      in += 1u + in[0];
    }

  err:
    LOG(WARNING) << "Connection (" << T::Name << ") " << connection_id << " fatal error due to unexpected alpn protos";
    return SSL_TLSEXT_ERR_ALERT_FATAL;
  }

  void setup_ssl_ctx_tlsext_cb(tlsext_ctx_t* tlsext_ctx) {
    SSL_CTX* ctx = ssl_ctx_.get();
    SSL_CTX_set_tlsext_servername_callback(ctx, &ContentServer::on_tlsext);
    SSL_CTX_set_tlsext_servername_arg(ctx, tlsext_ctx);

    VLOG(1) << "TLSEXT: Servername (server) enabled for connection " << next_connection_id_
            << " server_name: " << listen_ctxs_[tlsext_ctx->listen_ctx_num].server_name;
  }

  static int on_tlsext(SSL* ssl, int* al, void* arg) {
    auto tlsext_ctx = reinterpret_cast<tlsext_ctx_t*>(arg);
    auto server = reinterpret_cast<ContentServer*>(tlsext_ctx->server);
    int connection_id = tlsext_ctx->connection_id;
    int listen_ctx_num = tlsext_ctx->listen_ctx_num;
    std::string_view expected_server_name = server->listen_ctxs_[listen_ctx_num].server_name;

    // SNI must be accessible from the SNI callback.
    const char* server_name_ptr = SSL_get_servername(ssl, TLSEXT_NAMETYPE_host_name);
    std::string_view server_name;
    if (server_name_ptr) {
      server_name = server_name_ptr;
    }
    // Allow once if matched
    if (server_name == expected_server_name) {
      return SSL_TLSEXT_ERR_OK;
    }

    VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " TLSEXT: Servername mismatch "
            << "(got " << server_name << "; want " << expected_server_name << ").";
    return SSL_TLSEXT_ERR_ALERT_FATAL;
  }

  void set_https_fallback(int connection_id, bool https_fallback) {
    auto iter = connection_map_.find(connection_id);
    if (iter != connection_map_.end()) {
      iter->second->set_https_fallback(https_fallback);
    } else {
      VLOG(1) << "Connection (" << T::Name << ") " << connection_id << " Set Https Fallback fatal error:"
              << " invalid connection id";
    }
  }

  void setup_upstream_ssl_ctx(asio::error_code& ec) {
    upstream_ssl_ctx_.reset(::SSL_CTX_new(::TLS_client_method()));
    SSL_CTX* ctx = upstream_ssl_ctx_.get();
    if (!ctx) {
      print_openssl_error();
      ec = asio::error::no_memory;
      return;
    }

    SslSetClearMask options;
    options.ConfigureFlag(SSL_OP_ALL, true);

    SSL_CTX_set_options(ctx, options.set_mask);
    SSL_CTX_clear_options(ctx, options.clear_mask);

    CHECK(SSL_CTX_set_min_proto_version(ctx, TLS1_2_VERSION));
    CHECK(SSL_CTX_set_max_proto_version(ctx, TLS1_3_VERSION));

    if (absl::GetFlag(FLAGS_insecure_mode)) {
      SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, ::SSL_CTX_get_verify_callback(ctx));
    } else {
      SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, ::SSL_CTX_get_verify_callback(ctx));
      // FIXME
      // reverify only supported on custom verify callback
      // SSL_CTX_set_reverify_on_resume(ctx, 1);
    }
    if (ec) {
      return;
    }

    if (upstream_certificate_.empty()) {
      upstream_certificate_ = g_certificate_chain_content;
    }

    const auto& cert = upstream_certificate_;
    if (!cert.empty()) {
      if (ec) {
        return;
      }

      bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(cert.data(), cert.size()));

      bssl::UniquePtr<X509> cert(PEM_read_bio_X509(bio.get(), nullptr, 0, nullptr));
      if (!cert) {
        print_openssl_error();
        ec = asio::error::bad_descriptor;
        return;
      }
      X509_STORE* store = SSL_CTX_get_cert_store(ctx);
      if (!store) {
        print_openssl_error();
        ec = asio::error::no_memory;
        return;
      }

      ERR_clear_error();

      if (X509_STORE_add_cert(store, cert.get()) != 1) {
        print_openssl_error();
        ec = asio::error::bad_descriptor;
        return;
      }

      VLOG(1) << "Using upstream certificate (in-memory)";
    }

    client_instance_ = this;
    ssl_socket_data_index_ = SSL_get_ex_new_index(0, nullptr, nullptr, nullptr, nullptr);
    ssl_client_session_cache_ = std::make_unique<SSLClientSessionCache>(SSLClientSessionCache::Config{});

    // Disable the internal session cache. Session caching is handled
    // externally (i.e. by SSLClientSessionCache).
    SSL_CTX_set_session_cache_mode(ctx, SSL_SESS_CACHE_CLIENT | SSL_SESS_CACHE_NO_INTERNAL);
    SSL_CTX_sess_set_new_cb(ctx, NewSessionCallback);

    SSL_CTX_set_timeout(ctx, 1 * 60 * 60 /* one hour */);

    SSL_CTX_set_grease_enabled(ctx, 1);

    // Deduplicate all certificates minted from the SSL_CTX in memory.
    SSL_CTX_set0_buffer_pool(ctx, x509_util::GetBufferPool());

    load_ca_to_ssl_ctx(ctx);
  }

 private:
  int ssl_socket_data_index_ = -1;
  static ContentServer<T>* client_instance_;
  static ContentServer* GetInstance() { return client_instance_; }
  SSLSocket* GetClientSocketFromSSL(const SSL* ssl) {
    DCHECK(ssl);
    SSLSocket* socket = static_cast<SSLSocket*>(SSL_get_ex_data(ssl, ssl_socket_data_index_));
    DCHECK(socket);
    return socket;
  }

  static int NewSessionCallback(SSL* ssl, SSL_SESSION* session) {
    SSLSocket* socket = GetInstance()->GetClientSocketFromSSL(ssl);
    return socket->NewSessionCallback(session);
  }

 private:
  asio::io_context& io_context_;
  /// stopping the io_context from running out of work
  std::unique_ptr<asio::executor_work_guard<asio::io_context::executor_type>> work_guard_;

  std::string remote_host_ips_;
  std::string remote_host_sni_;
  uint16_t remote_port_;

  bool upstream_https_fallback_;
  bool https_fallback_;
  bool enable_upstream_tls_;
  bool enable_tls_;
  std::string upstream_certificate_;
  bssl::UniquePtr<SSL_CTX> upstream_ssl_ctx_;
  std::unique_ptr<SSLClientSessionCache> ssl_client_session_cache_;

  std::string certificate_;
  std::string private_key_;
  bssl::UniquePtr<SSL_CTX> ssl_ctx_;

  ContentServer::Delegate* delegate_;

  struct ListenCtx {
    std::string server_name;
    asio::ip::tcp::endpoint endpoint;
    asio::ip::tcp::endpoint peer_endpoint;
    std::unique_ptr<asio::ip::tcp::acceptor> acceptor;
  };
  std::array<ListenCtx, MAX_LISTEN_ADDRESSES> listen_ctxs_;
  int next_listen_ctx_ = 0;
  std::vector<int> pending_next_listen_ctxes_;
  bool in_shutdown_ = false;

  absl::flat_hash_map<int, scoped_refptr<ConnectionType>> connection_map_;

  int next_connection_id_ = 1;
  std::atomic<size_t> opened_connections_ = 0;
};

template <typename T>
ContentServer<T>* ContentServer<T>::client_instance_ = nullptr;

}  // namespace net

#endif  // H_NET_CONTENT_SERVER
