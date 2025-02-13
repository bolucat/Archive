#ifndef POLYFILLS_BASE_ALLOCATOR_BUILDFLAGS_H_
#define POLYFILLS_BASE_ALLOCATOR_BUILDFLAGS_H_

#include "build/buildflag.h"

#define BUILDFLAG_INTERNAL_USE_ALLOCATOR_SHIM() (1)
#define BUILDFLAG_INTERNAL_USE_PARTITION_ALLOC() (1)
#define BUILDFLAG_INTERNAL_USE_PARTITION_ALLOC_AS_MALLOC() (0)
#define BUILDFLAG_INTERNAL_USE_BACKUP_REF_PTR() (0)
#define BUILDFLAG_INTERNAL_USE_ASAN_BACKUP_REF_PTR() (0)
#define BUILDFLAG_INTERNAL_ENABLE_BACKUP_REF_PTR_SLOW_CHECKS() (0)
#define BUILDFLAG_INTERNAL_ENABLE_DANGLING_RAW_PTR_CHECKS() (0)
#define BUILDFLAG_INTERNAL_PUT_REF_COUNT_IN_PREVIOUS_SLOT() (0)
#define BUILDFLAG_INTERNAL_NEVER_REMOVE_FROM_BRP_POOL_BLOCKLIST() (0)
#define BUILDFLAG_INTERNAL_USE_FAKE_BINARY_EXPERIMENT() (0)
#define BUILDFLAG_INTERNAL_RECORD_ALLOC_INFO() (0)
#define BUILDFLAG_INTERNAL_FORCE_ENABLE_RAW_PTR_EXCLUSION() (0)

#endif  // POLYFILLS_BASE_ALLOCATOR_BUILDFLAGS_H_
