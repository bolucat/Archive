// SPDX-License-Identifier: GPL-2.0
/* Copyright (c) 2024 Chilledheart  */

#include <build/build_config.h>
#include <gtest/gtest.h>

#include <vector>

#include "core/utils.hpp"
#include "net/io_queue.hpp"
#include "net/iobuf.hpp"

using namespace net;

static constexpr unsigned int kDefaultDepth = 8u;
static constexpr unsigned int kBufferSize = 4096u;
static constexpr const char kBuffer[kBufferSize] = {};

TEST(IoQueueTest, Construct) {
  IoQueue<IOBuf, kDefaultDepth> queue;
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, PushBackAndPopFrontVariant0) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  ASSERT_EQ(kDefaultDepth, queue.length());
  ASSERT_EQ(kDefaultDepth * kBufferSize, queue.byte_length());
  for (unsigned int i = 0u; i < kDefaultDepth / 2; ++i) {
    queue.pop_front();
  }
  ASSERT_EQ(kDefaultDepth / 2, queue.length());
  ASSERT_EQ(kDefaultDepth / 2 * kBufferSize, queue.byte_length());
  for (unsigned int i = 0u; i < kDefaultDepth / 2; ++i) {
    queue.pop_front();
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, PushBackAndPopFrontVariant1) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(IOBuf::copyBuffer(std::string(kBuffer, kBufferSize)));
  }
  ASSERT_EQ(kDefaultDepth, queue.length());
  ASSERT_EQ(kDefaultDepth * kBufferSize, queue.byte_length());
  for (unsigned int i = 0u; i < kDefaultDepth / 2; ++i) {
    queue.pop_front();
  }
  ASSERT_EQ(kDefaultDepth / 2, queue.length());
  ASSERT_EQ(kDefaultDepth / 2 * kBufferSize, queue.byte_length());
  for (unsigned int i = 0u; i < kDefaultDepth / 2; ++i) {
    queue.pop_front();
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, MoveConstruct) {
  IoQueue<IOBuf, kDefaultDepth * 2> pending_data;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  auto queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth, queue.length());
}

TEST(IoQueueTest, MoveConstructOverInlinedStorage) {
  IoQueue<IOBuf, kDefaultDepth * 2> pending_data;
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  auto queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
}

TEST(IoQueueTest, MoveAssignment) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue, pending_data;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth, queue.length());
}

TEST(IoQueueTest, MoveAssignmentLhsOverInlinedStorage) {
  IoQueue<IOBuf, kDefaultDepth * 2> pending_data, queue;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth, queue.length());
}

TEST(IoQueueTest, MoveAssignmentRhsOverInlinedStorage) {
  IoQueue<IOBuf, kDefaultDepth * 2> pending_data, queue;
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
}

TEST(IoQueueTest, MoveAssignmentBothOverInlinedStorage) {
  IoQueue<IOBuf, kDefaultDepth * 2> pending_data, queue;
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    pending_data.push_back(kBuffer, kBufferSize);
  }
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  queue = std::move(pending_data);
  ASSERT_TRUE(pending_data.empty());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
}

TEST(IoQueueTest, Clear) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  queue.clear();
  ASSERT_TRUE(queue.empty());
  ASSERT_EQ(0u, queue.length());
  ASSERT_EQ(0u, queue.byte_length());
}

TEST(IoQueueTest, ClearOverInlinedStorage) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    queue.push_back(kBuffer, kBufferSize);
  }
  queue.clear();
  ASSERT_TRUE(queue.empty());
  ASSERT_EQ(0u, queue.length());
}

TEST(IoQueueTest, SwapEmptyWith) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue, empty_queue;

  ASSERT_TRUE(queue.empty());
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    empty_queue.push_back(kBuffer, kBufferSize);
  }

  std::swap(queue, empty_queue);

  ASSERT_TRUE(empty_queue.empty());
  ASSERT_EQ(0u, empty_queue.length());

  ASSERT_EQ(kDefaultDepth, queue.length());
  ASSERT_EQ(kDefaultDepth * kBufferSize, queue.byte_length());
}

TEST(IoQueueTest, SwapWithEmpty) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue, empty_queue;

  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    empty_queue.push_back(kBuffer, kBufferSize);
  }

  std::swap(empty_queue, queue);

  ASSERT_TRUE(empty_queue.empty());
  ASSERT_EQ(0u, empty_queue.length());

  ASSERT_EQ(kDefaultDepth, queue.length());
  ASSERT_EQ(kDefaultDepth * kBufferSize, queue.byte_length());
}

TEST(IoQueueTest, SwapNonEmpty) {
  IoQueue<IOBuf, kDefaultDepth * 10> lhs, rhs;

  for (unsigned int i = 0u; i < kDefaultDepth * 3; ++i) {
    lhs.push_back(kBuffer, kBufferSize * 5);
  }
  for (unsigned int i = 0u; i < kDefaultDepth * 7; ++i) {
    rhs.push_back(kBuffer, kBufferSize * 9);
  }

  std::swap(lhs, rhs);

  ASSERT_EQ(7 * kDefaultDepth, lhs.length());
  ASSERT_EQ(7 * 9 * kDefaultDepth * kBufferSize, lhs.byte_length());

  ASSERT_EQ(3 * kDefaultDepth, rhs.length());
  ASSERT_EQ(3 * 5 * kDefaultDepth * kBufferSize, rhs.byte_length());
}

TEST(IoQueueTest, EnlargeVariant0) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  ASSERT_TRUE(queue.empty());
  std::vector<std::shared_ptr<IOBuf>> v;

  // push idx_ to kDefaultDepth - 1
  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
    queue.pop_front();
  }
  ASSERT_TRUE(queue.empty());

  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    std::shared_ptr<IOBuf> buf = IOBuf::copyBuffer(kBuffer, kBufferSize);
    v.push_back(buf);
    queue.push_back(buf);
  }

  ASSERT_EQ(kDefaultDepth * 2, v.size());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    auto buf = queue.front();
    queue.pop_front();
    EXPECT_EQ(v[i].get(), buf.get());
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, EnlargeVariant1) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  ASSERT_TRUE(queue.empty());
  std::vector<std::shared_ptr<IOBuf>> v;

  // push idx_ to 0

  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    std::shared_ptr<IOBuf> buf = IOBuf::copyBuffer(kBuffer, kBufferSize);
    v.push_back(buf);
    queue.push_back(buf);
  }

  ASSERT_EQ(kDefaultDepth * 2, v.size());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    auto buf = queue.front();
    queue.pop_front();
    EXPECT_EQ(v[i].get(), buf.get());
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, EnlargeVariant2) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  ASSERT_TRUE(queue.empty());
  std::vector<std::shared_ptr<IOBuf>> v;

  // push idx_ to kDefaultDepth * 2 - 1
  for (unsigned int i = 0u; i < kDefaultDepth * 2 - 1; ++i) {
    queue.push_back(kBuffer, kBufferSize);
    queue.pop_front();
  }
  // ASSERT_EQ(queue.end_idx_, queue.length() - 1);
  ASSERT_TRUE(queue.empty());

  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    std::shared_ptr<IOBuf> buf = IOBuf::copyBuffer(kBuffer, kBufferSize);
    v.push_back(buf);
    queue.push_back(buf);
  }

  ASSERT_EQ(kDefaultDepth * 2, v.size());
  ASSERT_EQ(kDefaultDepth * 2, queue.length());
  for (unsigned int i = 0u; i < kDefaultDepth * 2; ++i) {
    auto buf = queue.front();
    queue.pop_front();
    EXPECT_EQ(v[i].get(), buf.get());
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, EnlargeTwice) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  ASSERT_TRUE(queue.empty());
  std::vector<std::shared_ptr<IOBuf>> v;

  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
    queue.pop_front();
  }

  for (unsigned int i = 0u; i < kDefaultDepth * 4; ++i) {
    std::shared_ptr<IOBuf> buf = IOBuf::copyBuffer(kBuffer, kBufferSize);
    v.push_back(buf);
    queue.push_back(buf);
  }

  ASSERT_EQ(kDefaultDepth * 4, v.size());
  ASSERT_EQ(kDefaultDepth * 4, queue.length());
  for (unsigned int i = 0u; i < kDefaultDepth * 4; ++i) {
    auto buf = queue.front();
    queue.pop_front();
    EXPECT_EQ(v[i].get(), buf.get());
  }
  ASSERT_TRUE(queue.empty());
}

TEST(IoQueueTest, EnlargeThird) {
  IoQueue<IOBuf, kDefaultDepth * 2> queue;
  ASSERT_TRUE(queue.empty());
  std::vector<std::shared_ptr<IOBuf>> v;

  for (unsigned int i = 0u; i < kDefaultDepth; ++i) {
    queue.push_back(kBuffer, kBufferSize);
    queue.pop_front();
  }

  for (unsigned int i = 0u; i < kDefaultDepth * 8; ++i) {
    std::shared_ptr<IOBuf> buf = IOBuf::copyBuffer(kBuffer, kBufferSize);
    v.push_back(buf);
    queue.push_back(buf);
  }

  ASSERT_EQ(kDefaultDepth * 8, v.size());
  ASSERT_EQ(kDefaultDepth * 8, queue.length());
  for (unsigned int i = 0u; i < kDefaultDepth * 8; ++i) {
    auto buf = queue.front();
    queue.pop_front();
    EXPECT_EQ(v[i].get(), buf.get());
  }
  ASSERT_TRUE(queue.empty());
}
