/**
 * Copyright (c) 2016 Tino Reichardt
 * All rights reserved.
 *
 * You can contact the author at:
 * - zstdmt source repository: https://github.com/mcmilk/zstdmt
 *
 * This source code is licensed under both the BSD-style license (found in the
 * LICENSE file in the root directory of this source tree) and the GPLv2 (found
 * in the COPYING file in the root directory of this source tree).
 * You may select, at your option, one of the above-listed licenses.
 */

/**
 * This file will hold wrapper for systems, which do not support pthreads
 */

#include "threading.h"

/* create fake symbol to avoid empty translation unit warning */
int g_ZSTD_threading_useless_symbol;

#if defined(ZSTD_MULTITHREAD) && defined(_WIN32)

/**
 * Windows minimalist Pthread Wrapper
 */


/* ===  Dependencies  === */
#include <process.h>
#include <errno.h>


/* ===  Implementation  === */


#if _WIN32_WINNT < 0x0600

#define ZSTD_DEPS_NEED_MALLOC
#include "zstd_deps.h"

typedef struct {
  int nwaiters_blocked;
  int nwaiters_gone;
  int nwaiters_to_unblock;
  int reserved;
  HANDLE sem_block_queue;
  HANDLE sem_block_lock;
  CRITICAL_SECTION mtx_unblock_lock;
} ZSTD_pthread_cond_do_t;
#define _ZSTD_SEMAPHORE_MAX LONG_MAX

static inline void
__ZSTD_pthread_cond_do_signal(ZSTD_pthread_cond_do_t *__cond, BOOL __broadcast)
{
  int nsignal = 0;

  EnterCriticalSection(&__cond->mtx_unblock_lock);
  if (__cond->nwaiters_to_unblock != 0) {
    if (__cond->nwaiters_blocked == 0) {
      LeaveCriticalSection(&__cond->mtx_unblock_lock);
      return;
    }
    if (__broadcast) {
      __cond->nwaiters_to_unblock += nsignal = __cond->nwaiters_blocked;
      __cond->nwaiters_blocked = 0;
    } else {
      nsignal = 1;
      __cond->nwaiters_to_unblock++;
      __cond->nwaiters_blocked--;
    }
  } else if (__cond->nwaiters_blocked > __cond->nwaiters_gone) {
    WaitForSingleObject(__cond->sem_block_lock, INFINITE);
    if (__cond->nwaiters_gone != 0) {
      __cond->nwaiters_blocked -= __cond->nwaiters_gone;
      __cond->nwaiters_gone = 0;
    }
    if (__broadcast) {
      nsignal = __cond->nwaiters_to_unblock = __cond->nwaiters_blocked;
      __cond->nwaiters_blocked = 0;
    } else {
      nsignal = __cond->nwaiters_to_unblock = 1;
      __cond->nwaiters_blocked--;
    }
  }
  LeaveCriticalSection(&__cond->mtx_unblock_lock);

  if (0 < nsignal)
    ReleaseSemaphore(__cond->sem_block_queue, nsignal, NULL);
}

static inline int
__ZSTD_pthread_cond_do_wait(ZSTD_pthread_cond_do_t *__cond,
                            ZSTD_pthread_mutex_t *__m)
{
  int nleft = 0;
  int nnwaiters_gone = 0;
  int timeout = 0;
  DWORD w;

  WaitForSingleObject(__cond->sem_block_lock, INFINITE);
  __cond->nwaiters_blocked++;
  ReleaseSemaphore(__cond->sem_block_lock, 1, NULL);

  ZSTD_pthread_mutex_unlock(__m);

  w = WaitForSingleObject(__cond->sem_block_queue, INFINITE);
  timeout = (w == WAIT_TIMEOUT);

  EnterCriticalSection(&__cond->mtx_unblock_lock);
  if ((nleft = __cond->nwaiters_to_unblock) != 0) {
    if (timeout) {
      if (__cond->nwaiters_blocked != 0) {
        __cond->nwaiters_blocked--;
      } else {
        __cond->nwaiters_gone++;
      }
    }
    if (--__cond->nwaiters_to_unblock == 0) {
      if (__cond->nwaiters_blocked != 0) {
        ReleaseSemaphore(__cond->sem_block_lock, 1, NULL);
        nleft = 0;
      }
      else if ((nnwaiters_gone = __cond->nwaiters_gone) != 0) {
        __cond->nwaiters_gone = 0;
      }
    }
  } else if (++__cond->nwaiters_gone == INT_MAX / 2) {
    WaitForSingleObject(__cond->sem_block_lock, INFINITE);
    __cond->nwaiters_blocked -= __cond->nwaiters_gone;
    ReleaseSemaphore(__cond->sem_block_lock, 1, NULL);
    __cond->nwaiters_gone = 0;
  }
  LeaveCriticalSection(&__cond->mtx_unblock_lock);

  if (nleft == 1) {
    while (nnwaiters_gone--)
      WaitForSingleObject(__cond->sem_block_queue, INFINITE);
    ReleaseSemaphore(__cond->sem_block_lock, 1, NULL);
  }

  ZSTD_pthread_mutex_lock(__m);
  return timeout ? /* busy */ ETIMEDOUT : 0;
}

int ZSTD_pthread_cond_init(ZSTD_pthread_cond_t *__cv, const void* attr)
{
  (void)attr;

  ZSTD_pthread_cond_do_t* __cond = (void*)ZSTD_malloc(sizeof(ZSTD_pthread_cond_do_t));
  __cond->nwaiters_blocked = 0;
  __cond->nwaiters_gone = 0;
  __cond->nwaiters_to_unblock = 0;
  __cond->reserved = 0;
  __cond->sem_block_queue = CreateSemaphore(NULL, 0, _ZSTD_SEMAPHORE_MAX,
                                            NULL);
  __cond->sem_block_lock = CreateSemaphore(NULL, 1, 1, NULL);
  InitializeCriticalSection(&__cond->mtx_unblock_lock);
  *(ZSTD_pthread_cond_do_t**)(__cv) = __cond;
  return 0;
}

int ZSTD_pthread_cond_signal(ZSTD_pthread_cond_t *__cv)
{
  ZSTD_pthread_cond_do_t* __do_cv = *(ZSTD_pthread_cond_do_t**)(__cv);
  __ZSTD_pthread_cond_do_signal(__do_cv, FALSE);
  return 0;
}

int ZSTD_pthread_cond_broadcast(ZSTD_pthread_cond_t *__cv)
{
  ZSTD_pthread_cond_do_t* __do_cv = *(ZSTD_pthread_cond_do_t**)(__cv);
  __ZSTD_pthread_cond_do_signal(__do_cv, TRUE);
  return 0;
}

int ZSTD_pthread_cond_wait(ZSTD_pthread_cond_t *__cv, ZSTD_pthread_mutex_t *__m)
{
  ZSTD_pthread_cond_do_t* __do_cv = *(ZSTD_pthread_cond_do_t**)(__cv);
  return __ZSTD_pthread_cond_do_wait(__do_cv, __m);
}

int ZSTD_pthread_cond_destroy(ZSTD_pthread_cond_t *__cv)
{
  ZSTD_pthread_cond_do_t* __cond = *(ZSTD_pthread_cond_do_t**)(__cv);
  CloseHandle(__cond->sem_block_queue);
  CloseHandle(__cond->sem_block_lock);
  DeleteCriticalSection(&__cond->mtx_unblock_lock);
  ZSTD_free(__cond);
  return 0;
}

#endif

typedef struct {
    void* (*start_routine)(void*);
    void* arg;
    int initialized;
    ZSTD_pthread_cond_t initialized_cond;
    ZSTD_pthread_mutex_t initialized_mutex;
} ZSTD_thread_params_t;

static unsigned __stdcall worker(void *arg)
{
    void* (*start_routine)(void*);
    void* thread_arg;

    /* Initialized thread_arg and start_routine and signal main thread that we don't need it
     * to wait any longer.
     */
    {
        ZSTD_thread_params_t*  thread_param = (ZSTD_thread_params_t*)arg;
        thread_arg = thread_param->arg;
        start_routine = thread_param->start_routine;

        /* Signal main thread that we are running and do not depend on its memory anymore */
        ZSTD_pthread_mutex_lock(&thread_param->initialized_mutex);
        thread_param->initialized = 1;
        ZSTD_pthread_cond_signal(&thread_param->initialized_cond);
        ZSTD_pthread_mutex_unlock(&thread_param->initialized_mutex);
    }

    start_routine(thread_arg);

    return 0;
}

int ZSTD_pthread_create(ZSTD_pthread_t* thread, const void* unused,
            void* (*start_routine) (void*), void* arg)
{
    ZSTD_thread_params_t thread_param;
    (void)unused;

    if (thread==NULL) return -1;
    *thread = NULL;

    thread_param.start_routine = start_routine;
    thread_param.arg = arg;
    thread_param.initialized = 0;

    /* Setup thread initialization synchronization */
    if(ZSTD_pthread_cond_init(&thread_param.initialized_cond, NULL)) {
        /* Should never happen on Windows */
        return -1;
    }
    if(ZSTD_pthread_mutex_init(&thread_param.initialized_mutex, NULL)) {
        /* Should never happen on Windows */
        ZSTD_pthread_cond_destroy(&thread_param.initialized_cond);
        return -1;
    }

    /* Spawn thread */
    *thread = (HANDLE)_beginthreadex(NULL, 0, worker, &thread_param, 0, NULL);
    if (*thread==NULL) {
        ZSTD_pthread_mutex_destroy(&thread_param.initialized_mutex);
        ZSTD_pthread_cond_destroy(&thread_param.initialized_cond);
        return errno;
    }

    /* Wait for thread to be initialized */
    ZSTD_pthread_mutex_lock(&thread_param.initialized_mutex);
    while(!thread_param.initialized) {
        ZSTD_pthread_cond_wait(&thread_param.initialized_cond, &thread_param.initialized_mutex);
    }
    ZSTD_pthread_mutex_unlock(&thread_param.initialized_mutex);
    ZSTD_pthread_mutex_destroy(&thread_param.initialized_mutex);
    ZSTD_pthread_cond_destroy(&thread_param.initialized_cond);

    return 0;
}

int ZSTD_pthread_join(ZSTD_pthread_t thread)
{
    DWORD result;

    if (!thread) return 0;

    result = WaitForSingleObject(thread, INFINITE);
    CloseHandle(thread);

    switch (result) {
    case WAIT_OBJECT_0:
        return 0;
    case WAIT_ABANDONED:
        return EINVAL;
    default:
        return GetLastError();
    }
}

#endif   /* ZSTD_MULTITHREAD */

#if defined(ZSTD_MULTITHREAD) && DEBUGLEVEL >= 1 && !defined(_WIN32)

#define ZSTD_DEPS_NEED_MALLOC
#include "zstd_deps.h"

int ZSTD_pthread_mutex_init(ZSTD_pthread_mutex_t* mutex, pthread_mutexattr_t const* attr)
{
    assert(mutex != NULL);
    *mutex = (pthread_mutex_t*)ZSTD_malloc(sizeof(pthread_mutex_t));
    if (!*mutex)
        return 1;
    return pthread_mutex_init(*mutex, attr);
}

int ZSTD_pthread_mutex_destroy(ZSTD_pthread_mutex_t* mutex)
{
    assert(mutex != NULL);
    if (!*mutex)
        return 0;
    {
        int const ret = pthread_mutex_destroy(*mutex);
        ZSTD_free(*mutex);
        return ret;
    }
}

int ZSTD_pthread_cond_init(ZSTD_pthread_cond_t* cond, pthread_condattr_t const* attr)
{
    assert(cond != NULL);
    *cond = (pthread_cond_t*)ZSTD_malloc(sizeof(pthread_cond_t));
    if (!*cond)
        return 1;
    return pthread_cond_init(*cond, attr);
}

int ZSTD_pthread_cond_destroy(ZSTD_pthread_cond_t* cond)
{
    assert(cond != NULL);
    if (!*cond)
        return 0;
    {
        int const ret = pthread_cond_destroy(*cond);
        ZSTD_free(*cond);
        return ret;
    }
}

#endif
