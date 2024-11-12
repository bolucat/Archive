// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2024 Chilledheart  */

#ifndef H_NET_SSL_CLIENT_SESSION_CACHE
#define H_NET_SSL_CLIENT_SESSION_CACHE

#include <stddef.h>
#include <time.h>

#include <memory>
#include <optional>
#include <string>

#include "base/containers/lru_cache.h"
#include "base/memory/raw_ptr.h"
#include "net/asio.hpp"
#include "third_party/boringssl/src/include/openssl/base.h"

namespace net {

class SSLClientSessionCache {
 public:
  struct Config {
    // The maximum number of entries in the cache.
    size_t max_entries = 1024;
    // The number of calls to Lookup before a new check for expired sessions.
    size_t expiration_check_count = 256;
  };

  struct Key {
    Key();
    Key(const Key& other);
    Key(Key&& other);
    ~Key();
    Key& operator=(const Key& other);
    Key& operator=(Key&& other);

    bool operator==(const Key& other) const;
    bool operator<(const Key& other) const;

    std::pair<std::string, int> server;
    std::optional<asio::ip::address> dest_ip_addr;
  };

  explicit SSLClientSessionCache(const Config& config);

  SSLClientSessionCache(const SSLClientSessionCache&) = delete;
  SSLClientSessionCache& operator=(const SSLClientSessionCache&) = delete;

  ~SSLClientSessionCache();

  // Returns true if |entry| is expired as of |now|.
  static bool IsExpired(SSL_SESSION* session, time_t now);

  size_t size() const;

  // Returns the session associated with |cache_key| and moves it to the front
  // of the MRU list. Returns nullptr if there is none.
  bssl::UniquePtr<SSL_SESSION> Lookup(const Key& cache_key);

  // Inserts |session| into the cache at |cache_key|. If there is an existing
  // one, it is released. Every |expiration_check_count| calls, the cache is
  // checked for stale entries.
  void Insert(const Key& cache_key, bssl::UniquePtr<SSL_SESSION> session);

  // Clears early data support for all current sessions associated with
  // |cache_key|. This may be used after a 0-RTT reject to avoid unnecessarily
  // offering 0-RTT data on retries. See https://crbug.com/1066623.
  void ClearEarlyData(const Key& cache_key);

  // Removes all entries from the cache.
  void Flush();

 private:
  struct Entry {
    Entry();
    Entry(Entry&&);
    ~Entry();

    // Adds a new session onto this entry, dropping the oldest one if two are
    // already stored.
    void Push(bssl::UniquePtr<SSL_SESSION> session);

    // Retrieves the latest session from the entry, removing it if its
    // single-use.
    bssl::UniquePtr<SSL_SESSION> Pop();

    // Removes any expired sessions, returning true if this entry can be
    // deleted.
    bool ExpireSessions(time_t now);

    bssl::UniquePtr<SSL_SESSION> sessions[2];
  };

  // Removes all expired sessions from the cache.
  void FlushExpiredSessions();

#if 0
  // Clear cache on low memory notifications callback.
  void OnMemoryPressure(
      base::MemoryPressureListener::MemoryPressureLevel memory_pressure_level);
#endif

  Config config_;
  gurl_base::LRUCache<Key, Entry> cache_;
  size_t lookups_since_flush_ = 0;
  // std::unique_ptr<base::MemoryPressureListener> memory_pressure_listener_;
};

}  // namespace net

#endif  // H_NET_SSL_CLIENT_SESSION_CACHE
