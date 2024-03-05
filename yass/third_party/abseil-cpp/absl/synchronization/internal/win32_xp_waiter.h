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
//

#ifndef ABSL_SYNCHRONIZATION_INTERNAL_WIN32_XP_WAITER_H_
#define ABSL_SYNCHRONIZATION_INTERNAL_WIN32_XP_WAITER_H_

#ifdef _WIN32
#include <sdkddkver.h>
#endif

#if defined(_WIN32) && _WIN32_WINNT >= _WIN32_WINNT_WINXP

#include "absl/base/config.h"
#include "absl/synchronization/internal/kernel_timeout.h"
#include "absl/synchronization/internal/waiter_base.h"

namespace absl {
ABSL_NAMESPACE_BEGIN
namespace synchronization_internal {

#define ABSL_INTERNAL_HAVE_WIN32_XP_WAITER 1

class Win32XpWaiter : public WaiterCrtp<Win32XpWaiter> {
 public:
  Win32XpWaiter();

  bool Wait(KernelTimeout t);
  void Post();
  void Poke();

  static constexpr char kName[] = "Win32XpWaiter";

 private:
  // WinHelper - Used to define utilities for accessing the lock and
  // condition variable storage once the types are complete.
  class WinHelper;

  // We can't include Windows.h in our headers
  void* sem_;
  // This seems superfluous, but for Poke() we need to cause spurious
  // wakeups on the semaphore. Hence we can't actually use the
  // semaphore's count.
  std::atomic<int> wakeups_;
};

}  // namespace synchronization_internal
ABSL_NAMESPACE_END
}  // namespace absl

#endif  // defined(_WIN32) && _WIN32_WINNT >= _WIN32_WINNT_WINXP

#endif  // ABSL_SYNCHRONIZATION_INTERNAL_WIN32_XP_WAITER_H_
