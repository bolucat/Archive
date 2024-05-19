#ifndef QUICHE_HTTP2_ADAPTER_NGHTTP2_H_
#define QUICHE_HTTP2_ADAPTER_NGHTTP2_H_

#include <cstddef>

#if NGHTTP2_VERSION_NUM < 0x013c00
// Required to build on Windows.
using ssize_t = ptrdiff_t;
#endif

#include "nghttp2/nghttp2.h"  // IWYU pragma: export

#endif  // QUICHE_HTTP2_ADAPTER_NGHTTP2_H_
