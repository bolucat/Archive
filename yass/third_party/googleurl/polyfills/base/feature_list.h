// Copyright 2022 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef POLYFILLS_BASE_FEATURE_LIST_H_
#define POLYFILLS_BASE_FEATURE_LIST_H_

#define BASE_DECLARE_FEATURE(feature) extern const gurl_base::Feature feature

#define BASE_FEATURE(feature, name, default_value) \
  const gurl_base::Feature feature(name, default_value)

namespace gurl_base {

enum FeatureState {
  FEATURE_DISABLED_BY_DEFAULT,
  FEATURE_ENABLED_BY_DEFAULT,
};

struct Feature {
  constexpr Feature(const char* name, FeatureState default_state)
      : name(name), default_state(default_state) {}

  const char* const name;
  const FeatureState default_state;
};

class FeatureList {
 public:
  static bool IsEnabled(const Feature& feature) {
    return feature.default_state == FEATURE_ENABLED_BY_DEFAULT;
  }

  static FeatureList* GetInstance() { return nullptr; }
};

}  // namespace gurl_base

#endif  // POLYFILLS_BASE_FEATURE_LIST_H_
