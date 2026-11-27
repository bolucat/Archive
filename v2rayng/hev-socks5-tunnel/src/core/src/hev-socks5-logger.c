/*
 ============================================================================
 Name        : hev-socks5-logger.c
 Author      : Heiher <r@hev.cc>
 Copyright   : Copyright (c) 2021 hev
 Description : Socks5 Logger
 ============================================================================
 */

#include <time.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <hev-sys.h>
#include <sys/stat.h>

#ifdef _WIN32
#include <io.h>
#endif

#include "hev-socks5-logger.h"
#include "hev-socks5-logger-priv.h"

#ifndef O_CLOEXEC
#define O_CLOEXEC 0
#endif

static int fd = -1;
static HevSocks5LoggerLevel req_level;

static int
dup_cloexec (int oldfd)
{
#ifdef F_DUPFD_CLOEXEC
    return fcntl (oldfd, F_DUPFD_CLOEXEC, 0);
#else
    return dup (oldfd);
#endif
}

int
hev_socks5_logger_init (HevSocks5LoggerLevel level, const char *path)
{
    if (!path)
        return 0;

    req_level = level;

    if (0 == strcmp (path, "stdout"))
        fd = dup_cloexec (1);
    else if (0 == strcmp (path, "stderr"))
        fd = dup_cloexec (2);
    else
        fd = open (path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0640);

    if (fd < 0)
        return -1;

    return 0;
}

void
hev_socks5_logger_fini (void)
{
    if (fd >= 0) {
        close (fd);
        fd = -1;
    }
}

int
hev_socks5_logger_enabled (HevSocks5LoggerLevel level)
{
    if (level >= req_level && fd >= 0)
        return 1;

    return 0;
}

void
hev_socks5_logger_log (HevSocks5LoggerLevel level, const char *fmt, ...)
{
    char buf[1024];
    struct tm *ti;
    size_t size;
    time_t now;
    va_list ap;
    char lc;
    int len;
    int n;

    if (level < req_level || fd < 0)
        return;

    switch (level) {
    case HEV_SOCKS5_LOGGER_DEBUG:
        lc = 'D';
        break;
    case HEV_SOCKS5_LOGGER_INFO:
        lc = 'I';
        break;
    case HEV_SOCKS5_LOGGER_WARN:
        lc = 'W';
        break;
    case HEV_SOCKS5_LOGGER_ERROR:
        lc = 'E';
        break;
    default:
        lc = '?';
        break;
    }

    time (&now);
    ti = localtime (&now);

    /* Gather the whole line in one buffer, so that it is written atomically. */
    len = snprintf (buf, 64, "[%04u-%02u-%02u %02u:%02u:%02u] [%c] ",
                    1900 + ti->tm_year, 1 + ti->tm_mon, ti->tm_mday,
                    ti->tm_hour, ti->tm_min, ti->tm_sec, lc);

    /* Reserve one byte for the trailing newline. */
    size = sizeof (buf) - len - 1;
    va_start (ap, fmt);
    n = vsnprintf (buf + len, size, fmt, ap);
    va_end (ap);

    if (n < 0)
        n = 0;
    else if ((size_t)n >= size)
        n = size - 1;
    len += n;
    buf[len++] = '\n';

    if (write (fd, buf, len)) {
        /* ignore return value */
    }
}
