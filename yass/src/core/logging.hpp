// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2019-2024 Chilledheart  */
#ifndef H_CORE_LOGGING
#define H_CORE_LOGGING

#include "third_party/googleurl-override/polyfills/base/check.h"
#include "third_party/googleurl-override/polyfills/base/check_op.h"
#include "third_party/googleurl-override/polyfills/base/logging.h"

#include <iosfwd>

// override operator<< std::error_code from STL
inline std::ostream& operator<<(std::ostream& os, const std::error_code& ec) {
#ifdef _WIN32
  return os << ec.message() << " value: " << ec.value();
#else
  return os << ec.message();
#endif
}

#endif  //  H_CORE_LOGGING
