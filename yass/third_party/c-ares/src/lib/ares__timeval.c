
/* Copyright (C) 2008 by Daniel Stenberg et al
 *
 * Permission to use, copy, modify, and distribute this software and its
 * documentation for any purpose and without fee is hereby granted, provided
 * that the above copyright notice appear in all copies and that both that
 * copyright notice and this permission notice appear in supporting
 * documentation, and that the name of M.I.T. not be used in advertising or
 * publicity pertaining to distribution of the software without specific,
 * written prior permission.  M.I.T. makes no representations about the
 * suitability of this software for any purpose.  It is provided "as is"
 * without express or implied warranty.
 */

#include "ares_setup.h"
#include "ares.h"
#include "ares_private.h"

#if defined(WIN32) && !defined(MSDOS)

/* cherry-picked from 1.32.2 https://github.com/c-ares/c-ares/commit/8a50fc6c */
static struct timeval ares__tvnow_qpc(void)
{
  struct timeval now;

  /* see https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps */
  /* QueryPerformanceCounters() has been around since Windows 2000, though
   * significant fixes were made in later versions (aka Vista).  Documentation states
   * 1 microsecond or better resolution with a rollover not less than 100 years.
   * This differs from GetTickCount{64}() which has a resolution between 10 and
   * 16 ms. */
  LARGE_INTEGER freq;
  LARGE_INTEGER current;

  /* Not sure how long it takes to get the frequency, I see it recommended to
   * cache it */
  QueryPerformanceFrequency(&freq);
  QueryPerformanceCounter(&current);

  now.tv_sec = current.QuadPart / freq.QuadPart;
  /* We want to prevent overflows so we get the remainder, then multiply to
   * microseconds before dividing */
  now.tv_usec = (unsigned int)(((current.QuadPart % freq.QuadPart) * 1000000) /
              freq.QuadPart);
  return now;
}

struct timeval ares__tvnow(void)
{
#if defined(_WIN32) && defined(_WIN32_WINNT) && _WIN32_WINNT >= 0x0600
  return ares__tvnow_qpc();
#else
  /* if vista or later */
  static int init = 0;
  static void* fp = NULL;
  if (!init) {
    init = 1;
    fp = (void*)GetProcAddress(GetModuleHandleW(L"Kernel32.dll"), "GetTickCount64");
  }
  if (fp) {
    return ares__tvnow_qpc();
  }
  struct timeval now;

  DWORD milliseconds = GetTickCount();
  now.tv_sec = milliseconds / 1000;
  now.tv_usec = (milliseconds % 1000) * 1000;
  return now;
#endif
}

#elif defined(HAVE_CLOCK_GETTIME_MONOTONIC)

struct timeval ares__tvnow(void)
{
  /*
  ** clock_gettime() is granted to be increased monotonically when the
  ** monotonic clock is queried. Time starting point is unspecified, it
  ** could be the system start-up time, the Epoch, or something else,
  ** in any case the time starting point does not change once that the
  ** system has started up.
  */
  struct timeval now;
  struct timespec tsnow;
  if(0 == clock_gettime(CLOCK_MONOTONIC, &tsnow)) {
    now.tv_sec = tsnow.tv_sec;
    now.tv_usec = tsnow.tv_nsec / 1000;
  }
  /*
  ** Even when the configure process has truly detected monotonic clock
  ** availability, it might happen that it is not actually available at
  ** run-time. When this occurs simply fallback to other time source.
  */
#ifdef HAVE_GETTIMEOFDAY
  else
    (void)gettimeofday(&now, NULL);  /* LCOV_EXCL_LINE */
#else
  else {
    now.tv_sec = (long)time(NULL);
    now.tv_usec = 0;
  }
#endif
  return now;
}

#elif defined(HAVE_GETTIMEOFDAY)

struct timeval ares__tvnow(void)
{
  /*
  ** gettimeofday() is not granted to be increased monotonically, due to
  ** clock drifting and external source time synchronization it can jump
  ** forward or backward in time.
  */
  struct timeval now;
  (void)gettimeofday(&now, NULL);
  return now;
}

#else

struct timeval ares__tvnow(void)
{
  /*
  ** time() returns the value of time in seconds since the Epoch.
  */
  struct timeval now;
  now.tv_sec = (long)time(NULL);
  now.tv_usec = 0;
  return now;
}

#endif

#if 0 /* Not used */
/*
 * Make sure that the first argument is the more recent time, as otherwise
 * we'll get a weird negative time-diff back...
 *
 * Returns: the time difference in number of milliseconds.
 */
long ares__tvdiff(struct timeval newer, struct timeval older)
{
  return (newer.tv_sec-older.tv_sec)*1000+
    (newer.tv_usec-older.tv_usec)/1000;
}
#endif

