// Copyright 2022 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef POLYFILLS_BASE_MEMORY_RAW_PTR_H_
#define POLYFILLS_BASE_MEMORY_RAW_PTR_H_

namespace gurl_base {

template<typename T>
using raw_ptr = T*;

}  // namespace gurl_base

using gurl_base::raw_ptr;

#endif  // POLYFILLS_BASE_MEMORY_RAW_PTR_H_
