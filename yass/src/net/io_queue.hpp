// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2023-2024 Chilledheart  */

#ifndef CORE_IO_QUEUE_HPP
#define CORE_IO_QUEUE_HPP

#include <absl/container/inlined_vector.h>
#include <build/build_config.h>
#include <memory>
#include "net/iobuf.hpp"

namespace net {

template <typename X = IOBuf,
          int DEFAULT_QUEUE_LENGTH =
#if BUILDFLAG(IS_ANDROID) || BUILDFLAG(IS_IOS) || BUILDFLAG(IS_OHOS) || defined(__MUSL__)
              8
#else
              16
#endif
          >
class IoQueue {
  using T = std::shared_ptr<X>;
  using Vector = absl::InlinedVector<T, DEFAULT_QUEUE_LENGTH>;

 public:
  IoQueue() { queue_.resize(DEFAULT_QUEUE_LENGTH); }
  IoQueue(const IoQueue&) = delete;
  IoQueue& operator=(const IoQueue&) = delete;
  IoQueue(IoQueue&& rhs) {
    idx_ = rhs.idx_;
    end_idx_ = rhs.end_idx_;
    queue_.resize(DEFAULT_QUEUE_LENGTH);
    std::swap(queue_, rhs.queue_);
    dirty_front_ = rhs.dirty_front_;
    rhs.idx_ = {};
    rhs.end_idx_ = {};
    rhs.dirty_front_ = {};
    DCHECK(rhs.empty());
#if DCHECK_IS_ON()
    for (auto buf : rhs.queue_) {
      DCHECK(!buf);
    }
#endif
  }
  IoQueue& operator=(IoQueue&& rhs) {
    idx_ = rhs.idx_;
    end_idx_ = rhs.end_idx_;
    DCHECK(queue_.size());
    std::swap(queue_, rhs.queue_);
    dirty_front_ = rhs.dirty_front_;
    rhs.idx_ = {};
    rhs.end_idx_ = {};
    rhs.dirty_front_ = {};
    DCHECK(rhs.empty());
#if DCHECK_IS_ON()
    for (auto buf : rhs.queue_) {
      DCHECK(!buf);
    }
#endif
    return *this;
  }

  bool empty() const { return idx_ == end_idx_; }

  void replace_front(T buf) {
    DCHECK(!empty());
    dirty_front_ = true;
    queue_[idx_] = buf;
  }

  void push_back(T buf) {
    queue_[end_idx_] = buf;
    end_idx_ = (end_idx_ + 1) % queue_.size();
    if (end_idx_ == idx_) {
      LOG(INFO) << "Current IO queue is full, enlarging by 2x to " << 2 * queue_.size();
      enlarge_queue_by_2x();
    }
  }

  void push_back(const char* data, size_t length) { push_back(IOBuf::copyBuffer(data, length)); }

  T front() {
    DCHECK(!empty());
    dirty_front_ = true;
    return queue_[idx_];
  }

  void pop_front() {
    DCHECK(!empty());
    dirty_front_ = false;
    queue_[idx_] = nullptr;
    idx_ = (idx_ + 1) % queue_.size();
  }

  T back() {
    DCHECK(!empty());
    return queue_[(end_idx_ + queue_.size() - 1) % queue_.size()];
  }

  size_t length() const { return (end_idx_ + queue_.size() - idx_) % queue_.size(); }

  size_t byte_length() const {
    if (empty()) {
      return 0u;
    }
    size_t ret = 0u;
    for (int i = idx_; i != end_idx_; i = (i + 1) % queue_.size())
      ret += queue_[i]->length();
    return ret;
  }

  void clear() { *this = IoQueue(); }

 private:
  void enlarge_queue_by_2x() {
    DCHECK(queue_.size());
    DCHECK_LE(queue_.size(), 32u << 10);
    Vector new_queue;
    new_queue.reserve(queue_.size() << 1);
    if (idx_ < end_idx_) {
      new_queue.insert(new_queue.end(), queue_.begin() + idx_, queue_.begin() + end_idx_);
    } else /* if (idx_ >= end_idx_) */ {
      new_queue.insert(new_queue.end(), queue_.begin() + idx_, queue_.end());
      new_queue.insert(new_queue.end(), queue_.begin(), queue_.begin() + end_idx_);
    }
    idx_ = 0;
    end_idx_ = queue_.size();
    new_queue.resize(queue_.size() << 1);
    std::swap(queue_, new_queue);
  }

 private:
  int idx_ = 0;
  int end_idx_ = 0;
  Vector queue_;
  bool dirty_front_ = false;
};

}  // namespace net

#endif  // CORE_IO_QUEUE_HPP
