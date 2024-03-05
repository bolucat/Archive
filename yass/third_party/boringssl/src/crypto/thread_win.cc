/* Copyright (c) 2015, Google Inc.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
 * SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */

// Ensure we can't call OPENSSL_malloc circularly.
#define _BORINGSSL_PROHIBIT_OPENSSL_MALLOC
#include "internal.h"

#if defined(OPENSSL_WINDOWS_THREADS)

OPENSSL_MSVC_PRAGMA(warning(push, 3))
#include <windows.h>
OPENSSL_MSVC_PRAGMA(warning(pop))

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/mem.h>

#ifdef OPENSSL_WINDOWS_ALLOW_WINXP
#ifndef HAVE_LIBCXX
#error Missing Implementation in Thread API
#endif // HAVE_LIBCXX
#endif // OPENSSL_WINDOWS_ALLOW_WINXP

#ifdef HAVE_LIBCXX

#include <__thread/support.h>

#ifndef _LIBCPP_HAS_THREAD_API_WIN32
#error "libc++'s Win32 API must be defined"
#endif

using namespace std;

#ifdef OPENSSL_WINDOWS_ALLOW_WINXP

static_assert(sizeof(CRYPTO_once_t) == sizeof(__libcpp_exec_once_flag), "");
static_assert(alignof(CRYPTO_once_t) == alignof(__libcpp_exec_once_flag), "");

static_assert(sizeof(__libcpp_mutex_t) == sizeof(CRITICAL_SECTION), "");
static_assert(alignof(__libcpp_mutex_t) == alignof(CRITICAL_SECTION), "");

void CRYPTO_once(CRYPTO_once_t *in_once, void (*init)(void)) {
  __libcpp_execute_once(in_once, init);
}

static_assert(sizeof(CRYPTO_MUTEX) >= sizeof(CRYPTO_once_t) + sizeof(__libcpp_mutex_t), "");
static_assert(alignof(CRYPTO_MUTEX) >= alignof(__libcpp_mutex_t), "");

static void static_lock_init(void* arg) {
  auto lock = reinterpret_cast<struct CRYPTO_MUTEX*>(arg);
  auto mutex = reinterpret_cast<__libcpp_mutex_t*>(&lock->lock);
  __libcpp_mutex_init(mutex);
}

void CRYPTO_MUTEX_init(CRYPTO_MUTEX *lock) {
  *lock = CRYPTO_MUTEX_INIT;
  __libcpp_execute_once(&lock->once, lock, static_lock_init);
}

void CRYPTO_MUTEX_lock_read(struct CRYPTO_MUTEX *lock) {
  __libcpp_execute_once(&lock->once, lock, static_lock_init);
  auto mutex = reinterpret_cast<__libcpp_mutex_t*>(&lock->lock);
  __libcpp_mutex_lock(mutex);
}

void CRYPTO_MUTEX_lock_write(struct CRYPTO_MUTEX *lock) {
  CRYPTO_MUTEX_lock_read(lock);
}

void CRYPTO_MUTEX_unlock_read(struct CRYPTO_MUTEX *lock) {
  auto mutex = reinterpret_cast<__libcpp_mutex_t*>(&lock->lock);
  __libcpp_mutex_unlock(mutex);
}

void CRYPTO_MUTEX_unlock_write(struct CRYPTO_MUTEX *lock) {
  auto mutex = reinterpret_cast<__libcpp_mutex_t*>(&lock->lock);
  __libcpp_mutex_unlock(mutex);
}

void CRYPTO_MUTEX_cleanup(CRYPTO_MUTEX *lock) {
  auto mutex = reinterpret_cast<__libcpp_mutex_t*>(&lock->lock);
  __libcpp_mutex_destroy(mutex);
}

#endif // OPENSSL_WINDOWS_ALLOW_WINXP

#include <thread>

static __libcpp_tls_key g_tls_keys[NUM_OPENSSL_THREAD_LOCALS];
static std::once_flag g_tls_init_flags[NUM_OPENSSL_THREAD_LOCALS];
static_assert(_LIBCPP_EXEC_ONCE_INITIALIZER == 0, "");

typedef struct thread_local_init_ctx {
  __libcpp_tls_key* key;
  thread_local_destructor_t destructor;
} thread_local_init_ctx;

static void thread_local_init(void* arg) {
  thread_local_init_ctx *ctx = reinterpret_cast<thread_local_init_ctx*>(arg);
  __libcpp_tls_key* key = ctx->key;
  thread_local_destructor_t destructor = ctx->destructor;
  if (__libcpp_tls_create(key, destructor) != 0) {
    abort();
  }
}

void *CRYPTO_get_thread_local(thread_local_data_t index) {
  if (g_tls_keys[index] == _LIBCPP_EXEC_ONCE_INITIALIZER) {
    return NULL;
  }
  return __libcpp_tls_get(g_tls_keys[index]);
}

int CRYPTO_set_thread_local(thread_local_data_t index, void *value,
                            thread_local_destructor_t destructor) {
  thread_local_init_ctx ctx;
  ctx.key = &g_tls_keys[index];
  ctx.destructor = destructor;

  std::call_once(g_tls_init_flags[index], thread_local_init, &ctx);

  return __libcpp_tls_set(g_tls_keys[index], value) == 0;
}

#endif // HAVE_LIBCXX

#endif  // OPENSSL_WINDOWS_THREADS
