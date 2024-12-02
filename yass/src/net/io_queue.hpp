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
          unsigned int DEFAULT_QUEUE_LENGTH =
#if BUILDFLAG(IS_ANDROID) || BUILDFLAG(IS_IOS) || BUILDFLAG(IS_OHOS) || defined(__MUSL__)
              8
#else
              16
#endif
          >
class IoQueue {
  using T = std::shared_ptr<X>;
  using Vector = absl::InlinedVector<T, DEFAULT_QUEUE_LENGTH>;
  static_assert(DEFAULT_QUEUE_LENGTH >= 2, "Default Queue Depth is too small");

 public:
  using size_type = Vector::size_type;

 public:
  IoQueue() { DCHECK_EQ(DEFAULT_QUEUE_LENGTH, queue_.size()); }
  IoQueue(const IoQueue&) = delete;
  IoQueue& operator=(const IoQueue&) = delete;
  IoQueue(IoQueue&& rhs) {
    idx_ = rhs.idx_;
    end_idx_ = rhs.end_idx_;
    DCHECK_EQ(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, rhs.queue_.size());
    std::swap(queue_, rhs.queue_);
    rhs.idx_ = {};
    rhs.end_idx_ = {};
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK(rhs.empty());
    DCHECK_EQ(DEFAULT_QUEUE_LENGTH, rhs.queue_.size());
  }
  IoQueue& operator=(IoQueue&& rhs) {
    idx_ = rhs.idx_;
    end_idx_ = rhs.end_idx_;
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, rhs.queue_.size());
    std::swap(queue_, rhs.queue_);
    rhs.idx_ = {};
    rhs.end_idx_ = {};
    // better way to optimize below code
    // rhs.queue_.clear();
    // rhs.queue_.resize(DEFAULT_QUEUE_LENGTH);
    Vector empty_queue{DEFAULT_QUEUE_LENGTH};
    std::swap(empty_queue, rhs.queue_);
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK(rhs.empty());
    DCHECK_EQ(DEFAULT_QUEUE_LENGTH, rhs.queue_.size());
    return *this;
  }

  bool empty() const {
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    if (idx_ == end_idx_) {
#if DCHECK_IS_ON()
      for (auto buf : queue_) {
        DCHECK(!buf);
      }
#endif
      return true;
    }
    return false;
  }

  void push_back(T buf) {
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK(buf);
    queue_[end_idx_] = buf;
    end_idx_ = (end_idx_ + 1) % queue_.size();
    if (end_idx_ == idx_) {
      VLOG(1) << "Current IO queue is full, enlarging by 2x to " << 2 * queue_.size();
      enlarge_queue_by_2x();
    }
  }

  void push_back(const char* data, size_t length) { push_back(IOBuf::copyBuffer(data, length)); }

  T front() {
    DCHECK(!empty());
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    return queue_[idx_];
  }

  void pop_front() {
    DCHECK(!empty());
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    queue_[idx_] = nullptr;
    idx_ = (idx_ + 1) % queue_.size();
  }

  T back() {
    DCHECK(!empty());
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    return queue_[(end_idx_ + queue_.size() - 1) % queue_.size()];
  }

  size_type length() const {
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    return (end_idx_ + queue_.size() - idx_) % queue_.size();
  }

  size_t byte_length() const {
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    if (empty()) {
      return 0u;
    }
    size_t ret = 0u;
    for (int i = idx_; i != end_idx_; i = (i + 1) % queue_.size())
      ret += queue_[i]->length();
    return ret;
  }

  void clear() {
    DCHECK_LE(DEFAULT_QUEUE_LENGTH, queue_.size());
    *this = IoQueue();
    DCHECK_EQ(DEFAULT_QUEUE_LENGTH, queue_.size());
    DCHECK(empty());
  }

  void swap(IoQueue& other) {
    if (this != std::addressof(other)) {
      std::swap(idx_, other.idx_);
      std::swap(end_idx_, other.end_idx_);
      std::swap(queue_, other.queue_);
    }
  }

 private:
  void enlarge_queue_by_2x() {
    DCHECK(queue_.size());
    DCHECK_EQ(idx_, end_idx_);
    DCHECK_LE(queue_.size() << 2, static_cast<size_type>(INT_MAX)) << "index overflow";
    Vector new_queue;
    DCHECK_EQ(0u, new_queue.size());
    new_queue.reserve(queue_.size() << 1);
    DCHECK_LE(queue_.size() << 1, new_queue.capacity());
    if (idx_ < end_idx_) {
      new_queue.insert(new_queue.end(), queue_.begin() + idx_, queue_.begin() + end_idx_);
    } else /* if (idx_ >= end_idx_) */ {
      new_queue.insert(new_queue.end(), queue_.begin() + idx_, queue_.end());
      new_queue.insert(new_queue.end(), queue_.begin(), queue_.begin() + end_idx_);
    }
    idx_ = 0;
    end_idx_ = queue_.size();
    new_queue.resize(queue_.size() << 1);
    DCHECK_EQ(queue_.size() << 1, new_queue.size());
    std::swap(queue_, new_queue);
    DCHECK_EQ(new_queue.size() << 1, queue_.size());
  }

 private:
  int idx_ = 0;
  int end_idx_ = 0;
  Vector queue_{static_cast<size_type>(DEFAULT_QUEUE_LENGTH)};
};

template <typename X, unsigned int Q>
void swap(IoQueue<X, Q>& lhs, IoQueue<X, Q>& rhs) {
  lhs.swap(rhs);
}

}  // namespace net

#endif  // CORE_IO_QUEUE_HPP
