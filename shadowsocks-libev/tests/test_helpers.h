#ifndef SS_TEST_HELPERS_H
#define SS_TEST_HELPERS_H
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include "platform.h"
#ifndef _WIN32
#include <sys/resource.h>
#endif

static inline void test_network_cleanup(void)
{
#ifdef _WIN32
    WSACleanup();
#endif
}
static inline void test_network_init(void)
{
#ifdef _WIN32
    WSADATA data;
    assert(WSAStartup(MAKEWORD(2, 2), &data) == 0);
    assert(atexit(test_network_cleanup) == 0);
#endif
}
/* Make sure at least `want` descriptors can be opened; the default soft
 * limit is only 256 on macOS, which is below what descriptor-heavy tests
 * such as test_event need. Windows sockets are not bounded by RLIMIT_NOFILE. */
static inline void test_raise_fd_limit(unsigned want)
{
#ifndef _WIN32
    struct rlimit limit;
    assert(getrlimit(RLIMIT_NOFILE, &limit) == 0);
    if (limit.rlim_cur != RLIM_INFINITY && limit.rlim_cur < want) {
        limit.rlim_cur = want;
        if (limit.rlim_max != RLIM_INFINITY && limit.rlim_max < limit.rlim_cur) {
            limit.rlim_cur = limit.rlim_max;
        }
        assert(setrlimit(RLIMIT_NOFILE, &limit) == 0);
    }
#else
    (void)want;
#endif
}
static inline FILE *test_tempfile(char *path, size_t size)
{
#ifdef _WIN32
    char directory[MAX_PATH];
    DWORD length = GetTempPathA(sizeof(directory), directory);
    assert(length != 0 && length < sizeof(directory));
    assert(size >= MAX_PATH);
    assert(GetTempFileNameA(directory, "ssc", 0, path) != 0);
    return fopen(path, "w");
#else
    assert(snprintf(path, size, "/tmp/ss-test.XXXXXX") < (int)size);
    int fd = mkstemp(path);
    assert(fd >= 0);
    return fdopen(fd, "w");
#endif
}
#endif
