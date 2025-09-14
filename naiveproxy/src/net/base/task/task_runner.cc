// Copyright 2025 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "net/base/task/task_runner.h"

#include "base/metrics/histogram_functions.h"
#include "base/no_destructor.h"
#include "base/rand_util.h"

namespace net {

namespace {
base::MetricsSubSampler& GetMetricsSubSampler() {
  static base::MetricsSubSampler sampler;
  return sampler;
}

}  // namespace

const scoped_refptr<base::SingleThreadTaskRunner>& GetTaskRunner(
    RequestPriority priority) {
  // Sample with a 0.001 probability to reduce metrics overhead.
  if (GetMetricsSubSampler().ShouldSample(0.001)) {
    base::UmaHistogramEnumeration("Net.TaskRunner.RequestPriority", priority);
  }
  if (priority == RequestPriority::HIGHEST &&
      internal::GetTaskRunnerGlobals().high_priority_task_runner) {
    return internal::GetTaskRunnerGlobals().high_priority_task_runner;
  }
  return base::SingleThreadTaskRunner::GetCurrentDefault();
}

namespace internal {

TaskRunnerGlobals::TaskRunnerGlobals() = default;
TaskRunnerGlobals::~TaskRunnerGlobals() = default;

TaskRunnerGlobals& GetTaskRunnerGlobals() {
  static base::NoDestructor<TaskRunnerGlobals> globals;
  return *globals;
}

}  // namespace internal

}  // namespace net
