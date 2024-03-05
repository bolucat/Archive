// Copyright 2023 The Abseil Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#include "absl/synchronization/internal/win32_xp_waiter.h"

#ifdef ABSL_INTERNAL_HAVE_WIN32_XP_WAITER

#ifdef ABSL_HAVE_SEMAPHORE_H
#include <semaphore.h>
#endif

#include <windows.h>

#include "absl/base/config.h"
#include "absl/base/internal/raw_logging.h"
#include "absl/base/internal/thread_identity.h"
#include "absl/base/optimization.h"
#include "absl/synchronization/internal/kernel_timeout.h"

namespace absl {
ABSL_NAMESPACE_BEGIN
namespace synchronization_internal {

#ifdef ABSL_INTERNAL_NEED_REDUNDANT_CONSTEXPR_DECL
constexpr char Win32XpWaiter::kName[];
#endif

class Win32XpWaiter::WinHelper {
 public:
  static HANDLE GetLock(Win32XpWaiter *w) {
    return reinterpret_cast<HANDLE>(w->sem_);
  }

  static_assert(sizeof(HANDLE) == sizeof(void *),
                "`sem_` does not have the same size as HANDLE");
  static_assert(alignof(HANDLE) == alignof(void *),
                "`sem_` does not have the same alignment as HANDLE");

  // The HANDLE types must be trivially constructible
  // and destructible because we never call their constructors or destructors.
  static_assert(std::is_trivially_constructible<HANDLE>::value,
                "The `HANDLE` type must be trivially constructible");
  static_assert(std::is_trivially_destructible<HANDLE>::value,
                "The `HANDLE` type must be trivially destructible");
};

Win32XpWaiter::Win32XpWaiter() {
  sem_ = CreateSemaphore(nullptr, 0, INT_MAX, nullptr);
  if (sem_ == nullptr) {
    const unsigned long err{GetLastError()};  // NOLINT(runtime/int)
    ABSL_RAW_LOG(FATAL, "CreateSemaphore failed with error %lu\n",
                 err);
  }
  wakeups_.store(0, std::memory_order_relaxed);
}

bool Win32XpWaiter::Wait(KernelTimeout t) {
  DWORD rel_timeout;
  if (t.has_timeout()) {
    rel_timeout = t.InMillisecondsFromNow();
  }

  // Loop until we timeout or consume a wakeup.
  // Note that, since the thread ticker is just reset, we don't need to check
  // whether the thread is idle on the very first pass of the loop.
  bool first_pass = true;
  while (true) {
    int x = wakeups_.load(std::memory_order_relaxed);
    while (x != 0) {
      if (!wakeups_.compare_exchange_weak(x, x - 1,
                                          std::memory_order_acquire,
                                          std::memory_order_relaxed)) {
        continue;  // Raced with someone, retry.
      }
      // Successfully consumed a wakeup, we're done.
      return true;
    }

    if (!first_pass) MaybeBecomeIdle();
    // Nothing to consume, wait (looping on EINTR).
    DWORD dwWaitResult;
    while (true) {
      if (!t.has_timeout()) {
        dwWaitResult = WaitForSingleObject(WinHelper::GetLock(this), INFINITE);
        const unsigned long err{GetLastError()};  // NOLINT(runtime/int)
        if (dwWaitResult == WAIT_OBJECT_0)
          break;
        ABSL_RAW_LOG(FATAL, "sem_wait failed: %lu", err);
      } else {
        dwWaitResult = WaitForSingleObject(WinHelper::GetLock(this), rel_timeout);
        const unsigned long err{GetLastError()};  // NOLINT(runtime/int)
        if (dwWaitResult == WAIT_OBJECT_0)
          break;
        if (dwWaitResult == WAIT_TIMEOUT)
          return false;
        ABSL_RAW_LOG(FATAL, "sem_timedwait failed: %lu", err);
      }
    }
    first_pass = false;
  }
}

void Win32XpWaiter::Post() {
  // Post a wakeup.
  if (wakeups_.fetch_add(1, std::memory_order_release) == 0) {
    // We incremented from 0, need to wake a potential waiter.
    Poke();
  }
}

void Win32XpWaiter::Poke() {
  if (!ReleaseSemaphore(WinHelper::GetLock(this), 1, nullptr)) {  // Wake any semaphore waiter.
    const unsigned long err{GetLastError()};  // NOLINT(runtime/int)
    ABSL_RAW_LOG(FATAL, "sem_post failed with errno %lu\n", err);
  }
}

}  // namespace synchronization_internal
ABSL_NAMESPACE_END
}  // namespace absl

#endif  // ABSL_INTERNAL_HAVE_WIN32_WAITER
