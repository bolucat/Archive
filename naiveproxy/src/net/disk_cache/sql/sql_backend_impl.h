// Copyright 2025 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef NET_DISK_CACHE_SQL_SQL_BACKEND_IMPL_H_
#define NET_DISK_CACHE_SQL_SQL_BACKEND_IMPL_H_

#include <list>
#include <map>
#include <queue>
#include <set>
#include <vector>

#include "base/files/file_path.h"
#include "base/memory/raw_ref.h"
#include "base/memory/weak_ptr.h"
#include "base/time/time.h"
#include "net/base/net_export.h"
#include "net/disk_cache/buildflags.h"
#include "net/disk_cache/disk_cache.h"
#include "net/disk_cache/sql/cache_entry_key.h"
#include "net/disk_cache/sql/exclusive_operation_coordinator.h"
#include "net/disk_cache/sql/sql_persistent_store.h"

// This backend is experimental and only available when the build flag is set.
static_assert(BUILDFLAG(ENABLE_DISK_CACHE_SQL_BACKEND));

namespace base {
class SequencedTaskRunner;
}  // namespace base

namespace disk_cache {

class SqlEntryImpl;

// Provides a concrete implementation of the disk cache backend that stores
// entries in a SQLite database. This class is responsible for all operations
// related to creating, opening, dooming, and enumerating cache entries.
//
// NOTE: This is currently a skeleton implementation, and some methods are not
// yet implemented, returning `net::ERR_NOT_IMPLEMENTED`.
class NET_EXPORT_PRIVATE SqlBackendImpl final : public Backend {
 public:
  // An enumeration of errors that can occur during the fake index file check.
  // These values are persisted to logs. Entries should not be renumbered and
  // numeric values should never be reused.
  //
  // LINT.IfChange(FakeIndexFileError)
  enum class FakeIndexFileError {
    kOkNew = 0,
    kOkExisting = 1,
    kCreateFileFailed = 2,
    kWriteFileFailed = 3,
    kWrongFileSize = 4,
    kOpenFileFailed = 5,
    kReadFileFailed = 6,
    kWrongMagicNumber = 7,
    kFailedToCreateDirectory = 8,
    kMaxValue = kFailedToCreateDirectory,
  };
  // LINT.ThenChange(//tools/metrics/histograms/metadata/net/enums.xml:SqlDiskCacheFakeIndexFileError)

  SqlBackendImpl(const base::FilePath& path,
                 int64_t max_bytes,
                 net::CacheType cache_type);

  SqlBackendImpl(const SqlBackendImpl&) = delete;
  SqlBackendImpl& operator=(const SqlBackendImpl&) = delete;

  ~SqlBackendImpl() override;

  // Initializes the backend, which includes initializing the persistent store
  // and checking for a fake index file. These two operations are performed in
  // parallel.
  void Init(CompletionOnceCallback callback);

  // Backend interface.
  int64_t MaxFileSize() const override;
  int32_t GetEntryCount(
      net::Int32CompletionOnceCallback callback) const override;
  EntryResult OpenOrCreateEntry(const std::string& key,
                                net::RequestPriority priority,
                                EntryResultCallback callback) override;
  EntryResult OpenEntry(const std::string& key,
                        net::RequestPriority priority,
                        EntryResultCallback callback) override;
  EntryResult CreateEntry(const std::string& key,
                          net::RequestPriority priority,
                          EntryResultCallback callback) override;
  net::Error DoomEntry(const std::string& key,
                       net::RequestPriority priority,
                       CompletionOnceCallback callback) override;
  net::Error DoomAllEntries(CompletionOnceCallback callback) override;
  net::Error DoomEntriesBetween(base::Time initial_time,
                                base::Time end_time,
                                CompletionOnceCallback callback) override;
  net::Error DoomEntriesSince(base::Time initial_time,
                              CompletionOnceCallback callback) override;
  int64_t CalculateSizeOfAllEntries(
      Int64CompletionOnceCallback callback) override;
  int64_t CalculateSizeOfEntriesBetween(
      base::Time initial_time,
      base::Time end_time,
      Int64CompletionOnceCallback callback) override;
  std::unique_ptr<Iterator> CreateIterator() override;
  void GetStats(base::StringPairs* stats) override;
  void OnExternalCacheHit(const std::string& key) override;

  // Called by SqlEntryImpl when it's being closed and is not doomed.
  // Removes the entry from `active_entries_`.
  void ReleaseActiveEntry(SqlEntryImpl& entry);
  // Called by SqlEntryImpl when it's being closed and is doomed.
  // Removes the entry from `doomed_entries_`.
  void ReleaseDoomedEntry(SqlEntryImpl& entry);

  // Marks an active entry as doomed and initiates its removal from the store.
  // If `callback` is provided, it will be run upon completion.
  void DoomActiveEntry(SqlEntryImpl& entry, CompletionOnceCallback callback);

  // Updates the `last_used` timestamp for an entry.
  void UpdateEntryLastUsed(const CacheEntryKey& key,
                           const base::UnguessableToken& token,
                           base::Time last_used,
                           SqlPersistentStore::ErrorCallback callback);

  // Updates the header data and `last_used` timestamp for an entry.
  void UpdateEntryHeaderAndLastUsed(const CacheEntryKey& key,
                                    const base::UnguessableToken& token,
                                    base::Time last_used,
                                    scoped_refptr<net::GrowableIOBuffer> buffer,
                                    int64_t header_size_delta,
                                    SqlPersistentStore::ErrorCallback callback);

  // Writes data to an entry's body (stream 1). This can be used to write new
  // data, overwrite existing data, or append to the entry. The operation is
  // scheduled via the `ExclusiveOperationCoordinator` to ensure proper
  // serialization.
  void WriteEntryData(const CacheEntryKey& key,
                      const base::UnguessableToken& token,
                      int64_t old_body_end,
                      int64_t body_end,
                      int64_t offset,
                      scoped_refptr<net::IOBuffer> buffer,
                      int buf_len,
                      bool truncate,
                      SqlPersistentStore::ErrorCallback callback);

  // Reads data from an entry's body (stream 1). The operation is scheduled via
  // the `ExclusiveOperationCoordinator`. `sparse_reading` controls whether
  // gaps in the data are filled with zeros or cause the read to stop.
  void ReadEntryData(const CacheEntryKey& key,
                     const base::UnguessableToken& token,
                     int64_t offset,
                     scoped_refptr<net::IOBuffer> buffer,
                     int buf_len,
                     int64_t body_end,
                     bool sparse_reading,
                     SqlPersistentStore::IntOrErrorCallback callback);

  // Finds the available contiguous range of data for a given entry. The
  // operation is scheduled via the `ExclusiveOperationCoordinator` to ensure
  // proper serialization.
  void GetEntryAvailableRange(const CacheEntryKey& key,
                              const base::UnguessableToken& token,
                              int64_t offset,
                              int len,
                              RangeResultCallback callback);

  // Sends a dummy operation through the background task runner via the
  // operation coordinator, for unit tests.
  int FlushQueueForTest(CompletionOnceCallback callback);

  scoped_refptr<base::SequencedTaskRunner> GetBackgroundTaskRunnerForTest() {
    return background_task_runner_;
  }

  // Enables a strict corruption checking mode for testing purposes. When
  // enabled, any detected database corruption will cause an immediate crash
  // via a `CHECK` failure. This is primarily useful for fuzzers, which can more
  // easily identify problematic inputs if the process fails fast, rather than
  // silently recovering.
  void EnableStrictCorruptionCheckForTesting();

 private:
  class IteratorImpl;

  // Identifies the type of a entry operation.
  enum class OpenOrCreateEntryOperationType {
    kCreateEntry,
    kOpenEntry,
    kOpenOrCreateEntry,
  };

  // Represents an in-flight modification to an entry's metadata (e.g.,
  // last_used, header). These modifications are queued and applied when the
  // entry is re-activated by `Iterator::OpenNextEntry()`.
  struct InFlightEntryModification {
    InFlightEntryModification(const base::UnguessableToken& token,
                              base::Time last_used);
    InFlightEntryModification(const base::UnguessableToken& token,
                              base::Time last_used,
                              scoped_refptr<net::GrowableIOBuffer> head);
    InFlightEntryModification(const base::UnguessableToken& token,
                              int64_t body_end);
    ~InFlightEntryModification();
    InFlightEntryModification(InFlightEntryModification&&);

    base::UnguessableToken token;
    std::optional<base::Time> last_used;
    std::optional<scoped_refptr<net::GrowableIOBuffer>> head;
    std::optional<int64_t> body_end;
  };

  void OnInitialized(CompletionOnceCallback callback,
                     const std::vector<bool>& results);

  SqlEntryImpl* GetActiveEntry(const CacheEntryKey& key);

  // Checks if the cache size has exceeded the high watermark and, if so,
  // schedules an eviction task. This is typically called after operations that
  // might increase the cache size. The eviction itself is run as an exclusive
  // operation to prevent conflicts with other cache activities.
  void MaybeTriggerEviction();

  // Internal helper for Open/Create/OpenOrCreate operations. It uses
  // `ExclusiveOperationCoordinator` to serialize operations on the same key and
  // to correctly handle synchronous vs. asynchronous returns.
  EntryResult OpenOrCreateEntryInternal(OpenOrCreateEntryOperationType type,
                                        const std::string& key,
                                        EntryResultCallback callback);

  // Handles the backend logic for Open/Create/OpenOrCreate operations. This
  // method is scheduled as a normal operation via the
  // `ExclusiveOperationCoordinator` to ensure proper serialization against
  // other operations on the same key.
  void HandleOpenOrCreateEntryOperation(
      OpenOrCreateEntryOperationType type,
      const CacheEntryKey& entry_key,
      EntryResultCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Callback for store operations that return an EntryInfo (`OpenOrCreate()`,
  // `Create()`).
  void OnEntryOperationFinished(
      const CacheEntryKey& key,
      EntryResultCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle,
      SqlPersistentStore::EntryInfoOrError result);
  // Callback for store operations that return an optional<EntryInfo>
  // (`Open()`).
  void OnOptionalEntryOperationFinished(
      const CacheEntryKey& key,
      EntryResultCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle,
      SqlPersistentStore::OptionalEntryInfoOrError result);

  // Handles the backend logic for `DoomActiveEntry()`. This method is scheduled
  // as a normal operation via the `ExclusiveOperationCoordinator`.
  void HandleDoomActiveEntryOperation(
      scoped_refptr<SqlEntryImpl> entry,
      CompletionOnceCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Dooms an active entry. This method must be called while holding an
  // `ExclusiveOperationCoordinator::OperationHandle` to ensure proper
  // serialization of operations on the entry.
  void DoomActiveEntryInternal(SqlEntryImpl& entry,
                               CompletionOnceCallback callback);

  // Handles the backend logic for `ReleaseDoomedEntry()`. This method is
  // scheduled as a normal operation via the `ExclusiveOperationCoordinator`.
  void HandleDeleteDoomedEntry(
      const CacheEntryKey& key,
      const base::UnguessableToken& token,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `DoomEntry()`. This method is scheduled as a
  // normal operation via the `ExclusiveOperationCoordinator`.
  void HandleDoomEntryOperation(
      const CacheEntryKey& key,
      net::RequestPriority priority,
      CompletionOnceCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `DoomEntriesBetween()`. This method is
  // scheduled as an exclusive operation via the
  // `ExclusiveOperationCoordinator`.
  void HandleDoomEntriesBetweenOperation(
      base::Time initial_time,
      base::Time end_time,
      CompletionOnceCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `UpdateEntryLastUsed()`. This method is
  // scheduled as a normal operation via the `ExclusiveOperationCoordinator`.
  void HandleUpdateEntryLastUsedOperation(
      const CacheEntryKey& key,
      const base::UnguessableToken& token,
      base::Time last_used,
      SqlPersistentStore::ErrorCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `UpdateEntryHeaderAndLastUsed()`. This method
  // is scheduled as a normal operation via the `ExclusiveOperationCoordinator`.
  void HandleUpdateEntryHeaderAndLastUsedOperation(
      const CacheEntryKey& key,
      const base::UnguessableToken& token,
      base::Time last_used,
      scoped_refptr<net::GrowableIOBuffer> buffer,
      int64_t header_size_delta,
      SqlPersistentStore::ErrorCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `WriteEntryData()`. This method is scheduled
  // as a normal operation via the `ExclusiveOperationCoordinator` and forwards
  // the call to the persistent store.
  void HandleWriteEntryDataOperation(
      const CacheEntryKey& key,
      const base::UnguessableToken& token,
      int64_t old_body_end,
      int64_t offset,
      scoped_refptr<net::IOBuffer> buffer,
      int buf_len,
      bool truncate,
      SqlPersistentStore::ErrorCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `ReadEntryData()`. This method is scheduled
  // as a normal operation via the `ExclusiveOperationCoordinator` and forwards
  // the call to the persistent store.
  void HandleReadEntryDataOperation(
      const base::UnguessableToken& token,
      int64_t offset,
      scoped_refptr<net::IOBuffer> buffer,
      int buf_len,
      int64_t body_end,
      bool sparse_reading,
      SqlPersistentStore::IntOrErrorCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `GetEntryAvailableRange()`. This method is
  // scheduled as a normal operation via the `ExclusiveOperationCoordinator`
  // and forwards the call to the persistent store.
  void HandleGetEntryAvailableRangeOperation(
      const base::UnguessableToken& token,
      int64_t offset,
      int len,
      RangeResultCallback callback,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for cache eviction. This method is scheduled as
  // an exclusive operation to ensure no other cache activities are running. It
  // gathers the keys of all active entries to prevent them from being evicted
  // and then delegates the actual eviction logic to the persistent store.
  void HandleTriggerEvictionOperation(
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Handles the backend logic for `OnExternalCacheHit()`. This method is
  // scheduled as a normal operation via the `ExclusiveOperationCoordinator`.
  void HandleOnExternalCacheHitOperation(
      const CacheEntryKey& key,
      base::Time now,
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  // Applies in-flight modifications to an entry's info.
  void ApplyInFlightEntryModifications(
      const CacheEntryKey& key,
      SqlPersistentStore::EntryInfo& entry_info);

  // Wraps an `ErrorCallback` to pop the oldest in-flight entry modification
  // from `in_flight_entry_modifications_` once the callback is invoked. This
  // ensures that the queue of in-flight modifications is managed correctly.
  SqlPersistentStore::ErrorCallback
  WrapErrorCallbackToPopInFlightEntryModification(
      const CacheEntryKey& key,
      SqlPersistentStore::ErrorCallback callback);

  // Schedules the `HandleDeleteDoomedEntriesOperation` task to run. This is the
  // entry point for the one-time cleanup of entries that were doomed in a
  // previous session.
  void TriggerDeleteDoomedEntries();

  // Physically deletes entries that were marked as "doomed" in previous
  // sessions from the database. It excludes any currently active doomed entries
  // to prevent data corruption. This method is executed as an exclusive
  // operation to ensure it has sole access to the cache during cleanup.
  void HandleDeleteDoomedEntriesOperation(
      std::unique_ptr<ExclusiveOperationCoordinator::OperationHandle> handle);

  const base::FilePath path_;

  // Task runner for all background SQLite operations.
  scoped_refptr<base::SequencedTaskRunner> background_task_runner_;

  // The persistent store that manages the SQLite database.
  std::unique_ptr<SqlPersistentStore> store_;

  // Map of cache keys to currently active (opened) entries.
  // `raw_ref` is used because the SqlEntryImpl objects are ref-counted and
  // their lifetime is managed by their ref_count. This map only holds
  // non-owning references to them.
  std::map<CacheEntryKey, raw_ref<SqlEntryImpl>> active_entries_;

  // Set of entries that have been marked as doomed but are still active
  // (i.e., have outstanding references).
  std::set<raw_ref<const SqlEntryImpl>> doomed_entries_;

  // Coordinates exclusive and normal operations to ensure that exclusive
  // operations have exclusive access.
  ExclusiveOperationCoordinator exclusive_operation_coordinator_;

  // Queue of in-flight entry modifications that need to be applied.
  // These are typically updates to `last_used` or header data that occur
  // while an entry is not actively open.
  std::map<CacheEntryKey, std::list<InFlightEntryModification>>
      in_flight_entry_modifications_;

  // A flag to prevent queuing multiple eviction operations. It is set to true
  // when an eviction operation is posted to the `ExclusiveOperationCoordinator`
  // and reset to false when the operation begins execution. This ensures that
  // even if `MaybeTriggerEviction()` is called multiple times while an eviction
  // task is pending, only one will be in the queue at any time.
  bool eviction_operation_queued_ = false;

  // Weak pointer factory for this class.
  base::WeakPtrFactory<SqlBackendImpl> weak_factory_{this};
};

}  // namespace disk_cache

#endif  // NET_DISK_CACHE_SQL_SQL_BACKEND_IMPL_H_
