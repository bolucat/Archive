// Copyright (c) 2021 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "quiche_platform_impl/quiche_test_impl.h"

#include <string>

#include "base/files/file_path.h"
#include "base/path_service.h"
#include "net/test/test_data_directory.h"

namespace quiche {
namespace test {

std::string QuicheGetCommonSourcePathImpl() {
  base::FilePath net_path = net::GetTestNetDirectory();
  return net_path.AppendASCII("third_party/quiche/common").MaybeAsASCII();
}

}  // namespace test
}  // namespace quiche

std::string QuicheGetTestMemoryCachePathImpl() {
  base::FilePath path;
  base::PathService::Get(base::DIR_SOURCE_ROOT, &path);
  path = path.AppendASCII("net").AppendASCII("data").AppendASCII(
      "quic_http_response_cache_data");
  // The file path is known to be an ascii string.
  return path.MaybeAsASCII();
}
