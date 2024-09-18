// Copyright 2015 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#ifndef BENCHMARK_RE_H_
#define BENCHMARK_RE_H_

#include "internal_macros.h"

// clang-format off

#include <re2/re2.h>

// clang-format on

#include <string>

#include "check.h"

namespace benchmark {

// A wrapper around the POSIX regular expression API that provides automatic
// cleanup
class Regex {
 public:
  Regex() : init_(false) {}

  ~Regex();

  // Compile a regular expression matcher from spec.  Returns true on success.
  //
  // On failure (and if error is not nullptr), error is populated with a human
  // readable error message if an error occurs.
  bool Init(const std::string& spec, std::string* error);

  // Returns whether str matches the compiled regular expression.
  bool Match(const std::string& str);

 private:
  bool init_;
  RE2 *re_;
};

inline bool Regex::Init(const std::string& spec, std::string* error) {
  RE2::Options options;
  options.set_posix_syntax(false); /* perl-like */
  re_ = new RE2(spec, options);
  if (!re_->ok()) {
    *error = re_->error();
    init_ = false;
    return init_;
  }
  init_ = true;
  return init_;
}

inline Regex::~Regex() { delete re_; }

inline bool Regex::Match(const std::string& str) {
  if (!init_) {
    return false;
  }
  return RE2::PartialMatch(str, *re_);
}

}  // end namespace benchmark

#endif  // BENCHMARK_RE_H_
