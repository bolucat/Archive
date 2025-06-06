// Copyright 2012 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <stddef.h>

#include <string>
#include <string_view>

#include "base/strings/string_piece.h"
#include "base/strings/utf_string_conversions.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace gurl_base {

template <typename CharT>
class CommonStringPieceTest : public ::testing::Test {
 public:
  static std::string as_string(const char* input) { return input; }
  static const std::string& as_string(const std::string& input) {
    return input;
  }
};

template <>
class CommonStringPieceTest<char16_t> : public ::testing::Test {
 public:
  static std::u16string as_string(const char* input) {
    return UTF8ToUTF16(input);
  }
  static std::u16string as_string(const std::string& input) {
    return UTF8ToUTF16(input);
  }
};

typedef ::testing::Types<char, char16_t> SupportedCharTypes;

TYPED_TEST_SUITE(CommonStringPieceTest, SupportedCharTypes);

TYPED_TEST(CommonStringPieceTest, CheckComparisonOperators) {
#define CMP_Y(op, x, y)                                                   \
  {                                                                       \
    std::basic_string<TypeParam> lhs(TestFixture::as_string(x));          \
    std::basic_string<TypeParam> rhs(TestFixture::as_string(y));          \
    ASSERT_TRUE((BasicStringPiece<TypeParam>((lhs.c_str()))               \
                     op BasicStringPiece<TypeParam>((rhs.c_str()))));     \
    ASSERT_TRUE(BasicStringPiece<TypeParam>(lhs) op rhs);                 \
    ASSERT_TRUE(lhs op BasicStringPiece<TypeParam>(rhs));                 \
    ASSERT_TRUE((BasicStringPiece<TypeParam>((lhs.c_str()))               \
                     .compare(BasicStringPiece<TypeParam>((rhs.c_str()))) \
                         op 0));                                          \
  }

#define CMP_N(op, x, y)                                                    \
  {                                                                        \
    std::basic_string<TypeParam> lhs(TestFixture::as_string(x));           \
    std::basic_string<TypeParam> rhs(TestFixture::as_string(y));           \
    ASSERT_FALSE((BasicStringPiece<TypeParam>((lhs.c_str()))               \
                      op BasicStringPiece<TypeParam>((rhs.c_str()))));     \
    ASSERT_FALSE(BasicStringPiece<TypeParam>(lhs) op rhs);                 \
    ASSERT_FALSE(lhs op BasicStringPiece<TypeParam>(rhs));                 \
    ASSERT_FALSE((BasicStringPiece<TypeParam>((lhs.c_str()))               \
                      .compare(BasicStringPiece<TypeParam>((rhs.c_str()))) \
                          op 0));                                          \
  }

  CMP_Y(==, "", "")
  CMP_Y(==, "a", "a")
  CMP_Y(==, "aa", "aa")
  CMP_N(==, "a", "")
  CMP_N(==, "", "a")
  CMP_N(==, "a", "b")
  CMP_N(==, "a", "aa")
  CMP_N(==, "aa", "a")

  CMP_N(!=, "", "")
  CMP_N(!=, "a", "a")
  CMP_N(!=, "aa", "aa")
  CMP_Y(!=, "a", "")
  CMP_Y(!=, "", "a")
  CMP_Y(!=, "a", "b")
  CMP_Y(!=, "a", "aa")
  CMP_Y(!=, "aa", "a")

  CMP_Y(<, "a", "b")
  CMP_Y(<, "a", "aa")
  CMP_Y(<, "aa", "b")
  CMP_Y(<, "aa", "bb")
  CMP_N(<, "a", "a")
  CMP_N(<, "b", "a")
  CMP_N(<, "aa", "a")
  CMP_N(<, "b", "aa")
  CMP_N(<, "bb", "aa")

  CMP_Y(<=, "a", "a")
  CMP_Y(<=, "a", "b")
  CMP_Y(<=, "a", "aa")
  CMP_Y(<=, "aa", "b")
  CMP_Y(<=, "aa", "bb")
  CMP_N(<=, "b", "a")
  CMP_N(<=, "aa", "a")
  CMP_N(<=, "b", "aa")
  CMP_N(<=, "bb", "aa")

  CMP_N(>=, "a", "b")
  CMP_N(>=, "a", "aa")
  CMP_N(>=, "aa", "b")
  CMP_N(>=, "aa", "bb")
  CMP_Y(>=, "a", "a")
  CMP_Y(>=, "b", "a")
  CMP_Y(>=, "aa", "a")
  CMP_Y(>=, "b", "aa")
  CMP_Y(>=, "bb", "aa")

  CMP_N(>, "a", "a")
  CMP_N(>, "a", "b")
  CMP_N(>, "a", "aa")
  CMP_N(>, "aa", "b")
  CMP_N(>, "aa", "bb")
  CMP_Y(>, "b", "a")
  CMP_Y(>, "aa", "a")
  CMP_Y(>, "b", "aa")
  CMP_Y(>, "bb", "aa")

  std::string x;
  for (int i = 0; i < 256; i++) {
    x += 'a';
    std::string y = x;
    CMP_Y(==, x, y);
    for (int j = 0; j < i; j++) {
      std::string z = x;
      z[j] = 'b';       // Differs in position 'j'
      CMP_N(==, x, z);
    }
  }

#undef CMP_Y
#undef CMP_N
}

TYPED_TEST(CommonStringPieceTest, CheckSTL) {
  std::basic_string<TypeParam> alphabet(
      TestFixture::as_string("abcdefghijklmnopqrstuvwxyz"));
  std::basic_string<TypeParam> abc(TestFixture::as_string("abc"));
  std::basic_string<TypeParam> xyz(TestFixture::as_string("xyz"));
  std::basic_string<TypeParam> foobar(TestFixture::as_string("foobar"));

  BasicStringPiece<TypeParam> a(alphabet);
  BasicStringPiece<TypeParam> b(abc);
  BasicStringPiece<TypeParam> c(xyz);
  BasicStringPiece<TypeParam> d(foobar);
  BasicStringPiece<TypeParam> e;
  std::basic_string<TypeParam> temp(TestFixture::as_string("123"));
  temp += static_cast<TypeParam>(0);
  temp += TestFixture::as_string("456");
  BasicStringPiece<TypeParam> f(temp);

  ASSERT_EQ(a[6], static_cast<TypeParam>('g'));
  ASSERT_EQ(b[0], static_cast<TypeParam>('a'));
  ASSERT_EQ(c[2], static_cast<TypeParam>('z'));
  ASSERT_EQ(f[3], static_cast<TypeParam>('\0'));
  ASSERT_EQ(f[5], static_cast<TypeParam>('5'));

  ASSERT_EQ(*d.data(), static_cast<TypeParam>('f'));
  ASSERT_EQ(d.data()[5], static_cast<TypeParam>('r'));
  ASSERT_EQ(e.data(), nullptr);

  ASSERT_EQ(*a.begin(), static_cast<TypeParam>('a'));
  ASSERT_EQ(*(b.begin() + 2), static_cast<TypeParam>('c'));
  ASSERT_EQ(*(c.end() - 1), static_cast<TypeParam>('z'));

  ASSERT_EQ(*a.rbegin(), static_cast<TypeParam>('z'));
  ASSERT_EQ(*(b.rbegin() + 2), static_cast<TypeParam>('a'));
  ASSERT_EQ(*(c.rend() - 1), static_cast<TypeParam>('x'));
  ASSERT_EQ(a.rbegin() + 26, a.rend());

  ASSERT_EQ(a.size(), 26U);
  ASSERT_EQ(b.size(), 3U);
  ASSERT_EQ(c.size(), 3U);
  ASSERT_EQ(d.size(), 6U);
  ASSERT_EQ(e.size(), 0U);
  ASSERT_EQ(f.size(), 7U);

  ASSERT_TRUE(!d.empty());
  ASSERT_TRUE(d.begin() != d.end());
  ASSERT_EQ(d.begin() + 6, d.end());

  ASSERT_TRUE(e.empty());
  ASSERT_EQ(e.begin(), e.end());

  d = BasicStringPiece<TypeParam>();
  ASSERT_EQ(d.size(), 0U);
  ASSERT_TRUE(d.empty());
  ASSERT_EQ(d.data(), nullptr);
  ASSERT_EQ(d.begin(), d.end());

  ASSERT_GE(a.max_size(), a.size());
}

TYPED_TEST(CommonStringPieceTest, CheckFind) {
  typedef BasicStringPiece<TypeParam> Piece;

  std::basic_string<TypeParam> alphabet(
      TestFixture::as_string("abcdefghijklmnopqrstuvwxyz"));
  std::basic_string<TypeParam> abc(TestFixture::as_string("abc"));
  std::basic_string<TypeParam> xyz(TestFixture::as_string("xyz"));
  std::basic_string<TypeParam> foobar(TestFixture::as_string("foobar"));

  BasicStringPiece<TypeParam> a(alphabet);
  BasicStringPiece<TypeParam> b(abc);
  BasicStringPiece<TypeParam> c(xyz);
  BasicStringPiece<TypeParam> d(foobar);

  d = Piece();
  Piece e;
  std::basic_string<TypeParam> temp(TestFixture::as_string("123"));
  temp.push_back('\0');
  temp += TestFixture::as_string("456");
  Piece f(temp);

  TypeParam buf[4] = {'%', '%', '%', '%'};
  ASSERT_EQ(a.copy(buf, 4), 4U);
  ASSERT_EQ(buf[0], a[0]);
  ASSERT_EQ(buf[1], a[1]);
  ASSERT_EQ(buf[2], a[2]);
  ASSERT_EQ(buf[3], a[3]);
  ASSERT_EQ(a.copy(buf, 3, 7), 3U);
  ASSERT_EQ(buf[0], a[7]);
  ASSERT_EQ(buf[1], a[8]);
  ASSERT_EQ(buf[2], a[9]);
  ASSERT_EQ(buf[3], a[3]);
  ASSERT_EQ(c.copy(buf, 99), 3U);
  ASSERT_EQ(buf[0], c[0]);
  ASSERT_EQ(buf[1], c[1]);
  ASSERT_EQ(buf[2], c[2]);
  ASSERT_EQ(buf[3], a[3]);

  ASSERT_EQ(Piece::npos, std::basic_string<TypeParam>::npos);

  ASSERT_EQ(a.find(b), 0U);
  ASSERT_EQ(a.find(b, 1), Piece::npos);
  ASSERT_EQ(a.find(c), 23U);
  ASSERT_EQ(a.find(c, 9), 23U);
  ASSERT_EQ(a.find(c, Piece::npos), Piece::npos);
  ASSERT_EQ(b.find(c), Piece::npos);
  ASSERT_EQ(b.find(c, Piece::npos), Piece::npos);
  ASSERT_EQ(a.find(d), 0U);
  ASSERT_EQ(a.find(e), 0U);
  ASSERT_EQ(a.find(d, 12), 12U);
  ASSERT_EQ(a.find(e, 17), 17U);
  std::basic_string<TypeParam> not_found(
      TestFixture::as_string("xx not found bb"));
  Piece g(not_found);
  ASSERT_EQ(a.find(g), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.find(b), Piece::npos);
  ASSERT_EQ(e.find(b), Piece::npos);
  ASSERT_EQ(d.find(b, 4), Piece::npos);
  ASSERT_EQ(e.find(b, 7), Piece::npos);

  size_t empty_search_pos =
      std::basic_string<TypeParam>().find(std::basic_string<TypeParam>());
  ASSERT_EQ(d.find(d), empty_search_pos);
  ASSERT_EQ(d.find(e), empty_search_pos);
  ASSERT_EQ(e.find(d), empty_search_pos);
  ASSERT_EQ(e.find(e), empty_search_pos);
  ASSERT_EQ(d.find(d, 4), std::string().find(std::string(), 4));
  ASSERT_EQ(d.find(e, 4), std::string().find(std::string(), 4));
  ASSERT_EQ(e.find(d, 4), std::string().find(std::string(), 4));
  ASSERT_EQ(e.find(e, 4), std::string().find(std::string(), 4));

  constexpr TypeParam kNul = '\0';
  ASSERT_EQ(a.find('a'), 0U);
  ASSERT_EQ(a.find('c'), 2U);
  ASSERT_EQ(a.find('z'), 25U);
  ASSERT_EQ(a.find('$'), Piece::npos);
  ASSERT_EQ(a.find(kNul), Piece::npos);
  ASSERT_EQ(f.find(kNul), 3U);
  ASSERT_EQ(f.find('3'), 2U);
  ASSERT_EQ(f.find('5'), 5U);
  ASSERT_EQ(g.find('o'), 4U);
  ASSERT_EQ(g.find('o', 4), 4U);
  ASSERT_EQ(g.find('o', 5), 8U);
  ASSERT_EQ(a.find('b', 5), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.find(kNul), Piece::npos);
  ASSERT_EQ(e.find(kNul), Piece::npos);
  ASSERT_EQ(d.find(kNul, 4), Piece::npos);
  ASSERT_EQ(e.find(kNul, 7), Piece::npos);
  ASSERT_EQ(d.find('x'), Piece::npos);
  ASSERT_EQ(e.find('x'), Piece::npos);
  ASSERT_EQ(d.find('x', 4), Piece::npos);
  ASSERT_EQ(e.find('x', 7), Piece::npos);

  ASSERT_EQ(a.find(b.data(), 1, 0), 1U);
  ASSERT_EQ(a.find(c.data(), 9, 0), 9U);
  ASSERT_EQ(a.find(c.data(), Piece::npos, 0), Piece::npos);
  ASSERT_EQ(b.find(c.data(), Piece::npos, 0), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.find(b.data(), 4, 0), Piece::npos);
  ASSERT_EQ(e.find(b.data(), 7, 0), Piece::npos);

  ASSERT_EQ(a.find(b.data(), 1), Piece::npos);
  ASSERT_EQ(a.find(c.data(), 9), 23U);
  ASSERT_EQ(a.find(c.data(), Piece::npos), Piece::npos);
  ASSERT_EQ(b.find(c.data(), Piece::npos), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.find(b.data(), 4), Piece::npos);
  ASSERT_EQ(e.find(b.data(), 7), Piece::npos);

  ASSERT_EQ(a.rfind(b), 0U);
  ASSERT_EQ(a.rfind(b, 1), 0U);
  ASSERT_EQ(a.rfind(c), 23U);
  ASSERT_EQ(a.rfind(c, 22U), Piece::npos);
  ASSERT_EQ(a.rfind(c, 1U), Piece::npos);
  ASSERT_EQ(a.rfind(c, 0U), Piece::npos);
  ASSERT_EQ(b.rfind(c), Piece::npos);
  ASSERT_EQ(b.rfind(c, 0U), Piece::npos);
  ASSERT_EQ(a.rfind(d),
            static_cast<size_t>(a.rfind(std::basic_string<TypeParam>())));
  ASSERT_EQ(a.rfind(e), a.rfind(std::basic_string<TypeParam>()));
  ASSERT_EQ(a.rfind(d),
            static_cast<size_t>(std::basic_string<TypeParam>(a).rfind(
                std::basic_string<TypeParam>())));
  ASSERT_EQ(a.rfind(e), std::basic_string<TypeParam>(a).rfind(
                            std::basic_string<TypeParam>()));
  ASSERT_EQ(a.rfind(d, 12), 12U);
  ASSERT_EQ(a.rfind(e, 17), 17U);
  ASSERT_EQ(a.rfind(g), Piece::npos);
  ASSERT_EQ(d.rfind(b), Piece::npos);
  ASSERT_EQ(e.rfind(b), Piece::npos);
  ASSERT_EQ(d.rfind(b, 4), Piece::npos);
  ASSERT_EQ(e.rfind(b, 7), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.rfind(d, 4), std::string().rfind(std::string()));
  ASSERT_EQ(e.rfind(d, 7), std::string().rfind(std::string()));
  ASSERT_EQ(d.rfind(e, 4), std::string().rfind(std::string()));
  ASSERT_EQ(e.rfind(e, 7), std::string().rfind(std::string()));
  ASSERT_EQ(d.rfind(d), std::string().rfind(std::string()));
  ASSERT_EQ(e.rfind(d), std::string().rfind(std::string()));
  ASSERT_EQ(d.rfind(e), std::string().rfind(std::string()));
  ASSERT_EQ(e.rfind(e), std::string().rfind(std::string()));

  ASSERT_EQ(g.rfind('o'), 8U);
  ASSERT_EQ(g.rfind('q'), Piece::npos);
  ASSERT_EQ(g.rfind('o', 8), 8U);
  ASSERT_EQ(g.rfind('o', 7), 4U);
  ASSERT_EQ(g.rfind('o', 3), Piece::npos);
  ASSERT_EQ(f.rfind(kNul), 3U);
  ASSERT_EQ(f.rfind(kNul, 12), 3U);
  ASSERT_EQ(f.rfind('3'), 2U);
  ASSERT_EQ(f.rfind('5'), 5U);
  // empty string nonsense
  ASSERT_EQ(d.rfind('o'), Piece::npos);
  ASSERT_EQ(e.rfind('o'), Piece::npos);
  ASSERT_EQ(d.rfind('o', 4), Piece::npos);
  ASSERT_EQ(e.rfind('o', 7), Piece::npos);

  ASSERT_EQ(a.rfind(b.data(), 1, 0), 1U);
  ASSERT_EQ(a.rfind(c.data(), 22U, 0), 22U);
  ASSERT_EQ(a.rfind(c.data(), 1U, 0), 1U);
  ASSERT_EQ(a.rfind(c.data(), 0U, 0), 0U);
  ASSERT_EQ(b.rfind(c.data(), 0U, 0), 0U);
  ASSERT_EQ(d.rfind(b.data(), 4, 0), 0U);
  ASSERT_EQ(e.rfind(b.data(), 7, 0), 0U);

  std::basic_string<TypeParam> one_two_three_four(
      TestFixture::as_string("one,two:three;four"));
  std::basic_string<TypeParam> comma_colon(TestFixture::as_string(",:"));
  ASSERT_EQ(3U, Piece(one_two_three_four).find_first_of(comma_colon));
  ASSERT_EQ(a.find_first_of(b), 0U);
  ASSERT_EQ(a.find_first_of(b, 0), 0U);
  ASSERT_EQ(a.find_first_of(b, 1), 1U);
  ASSERT_EQ(a.find_first_of(b, 2), 2U);
  ASSERT_EQ(a.find_first_of(b, 3), Piece::npos);
  ASSERT_EQ(a.find_first_of(c), 23U);
  ASSERT_EQ(a.find_first_of(c, 23), 23U);
  ASSERT_EQ(a.find_first_of(c, 24), 24U);
  ASSERT_EQ(a.find_first_of(c, 25), 25U);
  ASSERT_EQ(a.find_first_of(c, 26), Piece::npos);
  ASSERT_EQ(g.find_first_of(b), 13U);
  ASSERT_EQ(g.find_first_of(c), 0U);
  ASSERT_EQ(a.find_first_of(f), Piece::npos);
  ASSERT_EQ(f.find_first_of(a), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(a.find_first_of(d), Piece::npos);
  ASSERT_EQ(a.find_first_of(e), Piece::npos);
  ASSERT_EQ(d.find_first_of(b), Piece::npos);
  ASSERT_EQ(e.find_first_of(b), Piece::npos);
  ASSERT_EQ(d.find_first_of(d), Piece::npos);
  ASSERT_EQ(e.find_first_of(d), Piece::npos);
  ASSERT_EQ(d.find_first_of(e), Piece::npos);
  ASSERT_EQ(e.find_first_of(e), Piece::npos);

  ASSERT_EQ(a.find_first_not_of(b), 3U);
  ASSERT_EQ(a.find_first_not_of(c), 0U);
  ASSERT_EQ(b.find_first_not_of(a), Piece::npos);
  ASSERT_EQ(c.find_first_not_of(a), Piece::npos);
  ASSERT_EQ(f.find_first_not_of(a), 0U);
  ASSERT_EQ(a.find_first_not_of(f), 0U);
  ASSERT_EQ(a.find_first_not_of(d), 0U);
  ASSERT_EQ(a.find_first_not_of(e), 0U);
  ASSERT_EQ(a.find_first_not_of(d, 1), 1U);
  ASSERT_EQ(a.find_first_not_of(e, 1), 1U);
  ASSERT_EQ(a.find_first_not_of(d, a.size()), Piece::npos);
  ASSERT_EQ(a.find_first_not_of(e, a.size()), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(d.find_first_not_of(a), Piece::npos);
  ASSERT_EQ(e.find_first_not_of(a), Piece::npos);
  ASSERT_EQ(d.find_first_not_of(d), Piece::npos);
  ASSERT_EQ(e.find_first_not_of(d), Piece::npos);
  ASSERT_EQ(d.find_first_not_of(e), Piece::npos);
  ASSERT_EQ(e.find_first_not_of(e), Piece::npos);

  std::basic_string<TypeParam> equals(TestFixture::as_string("===="));
  Piece h(equals);
  ASSERT_EQ(h.find_first_not_of('='), Piece::npos);
  ASSERT_EQ(h.find_first_not_of('=', 3), Piece::npos);
  ASSERT_EQ(h.find_first_not_of(kNul), 0U);
  ASSERT_EQ(g.find_first_not_of('x'), 2U);
  ASSERT_EQ(f.find_first_not_of(kNul), 0U);
  ASSERT_EQ(f.find_first_not_of(kNul, 3), 4U);
  ASSERT_EQ(f.find_first_not_of(kNul, 2), 2U);
  // empty string nonsense
  ASSERT_EQ(d.find_first_not_of('x'), Piece::npos);
  ASSERT_EQ(e.find_first_not_of('x'), Piece::npos);
  ASSERT_EQ(d.find_first_not_of(kNul), Piece::npos);
  ASSERT_EQ(e.find_first_not_of(kNul), Piece::npos);

  //  Piece g("xx not found bb");
  std::basic_string<TypeParam> fifty_six(TestFixture::as_string("56"));
  Piece i(fifty_six);
  ASSERT_EQ(h.find_last_of(a), Piece::npos);
  ASSERT_EQ(g.find_last_of(a), g.size()-1);
  ASSERT_EQ(a.find_last_of(b), 2U);
  ASSERT_EQ(a.find_last_of(c), a.size()-1);
  ASSERT_EQ(f.find_last_of(i), 6U);
  ASSERT_EQ(a.find_last_of('a'), 0U);
  ASSERT_EQ(a.find_last_of('b'), 1U);
  ASSERT_EQ(a.find_last_of('z'), 25U);
  ASSERT_EQ(a.find_last_of('a', 5), 0U);
  ASSERT_EQ(a.find_last_of('b', 5), 1U);
  ASSERT_EQ(a.find_last_of('b', 0), Piece::npos);
  ASSERT_EQ(a.find_last_of('z', 25), 25U);
  ASSERT_EQ(a.find_last_of('z', 24), Piece::npos);
  ASSERT_EQ(f.find_last_of(i, 5), 5U);
  ASSERT_EQ(f.find_last_of(i, 6), 6U);
  ASSERT_EQ(f.find_last_of(a, 4), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(f.find_last_of(d), Piece::npos);
  ASSERT_EQ(f.find_last_of(e), Piece::npos);
  ASSERT_EQ(f.find_last_of(d, 4), Piece::npos);
  ASSERT_EQ(f.find_last_of(e, 4), Piece::npos);
  ASSERT_EQ(d.find_last_of(d), Piece::npos);
  ASSERT_EQ(d.find_last_of(e), Piece::npos);
  ASSERT_EQ(e.find_last_of(d), Piece::npos);
  ASSERT_EQ(e.find_last_of(e), Piece::npos);
  ASSERT_EQ(d.find_last_of(f), Piece::npos);
  ASSERT_EQ(e.find_last_of(f), Piece::npos);
  ASSERT_EQ(d.find_last_of(d, 4), Piece::npos);
  ASSERT_EQ(d.find_last_of(e, 4), Piece::npos);
  ASSERT_EQ(e.find_last_of(d, 4), Piece::npos);
  ASSERT_EQ(e.find_last_of(e, 4), Piece::npos);
  ASSERT_EQ(d.find_last_of(f, 4), Piece::npos);
  ASSERT_EQ(e.find_last_of(f, 4), Piece::npos);

  ASSERT_EQ(a.find_last_not_of(b), a.size()-1);
  ASSERT_EQ(a.find_last_not_of(c), 22U);
  ASSERT_EQ(b.find_last_not_of(a), Piece::npos);
  ASSERT_EQ(b.find_last_not_of(b), Piece::npos);
  ASSERT_EQ(f.find_last_not_of(i), 4U);
  ASSERT_EQ(a.find_last_not_of(c, 24), 22U);
  ASSERT_EQ(a.find_last_not_of(b, 3), 3U);
  ASSERT_EQ(a.find_last_not_of(b, 2), Piece::npos);
  // empty string nonsense
  ASSERT_EQ(f.find_last_not_of(d), f.size()-1);
  ASSERT_EQ(f.find_last_not_of(e), f.size()-1);
  ASSERT_EQ(f.find_last_not_of(d, 4), 4U);
  ASSERT_EQ(f.find_last_not_of(e, 4), 4U);
  ASSERT_EQ(d.find_last_not_of(d), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(e), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(d), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(e), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(f), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(f), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(d, 4), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(e, 4), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(d, 4), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(e, 4), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(f, 4), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(f, 4), Piece::npos);

  ASSERT_EQ(h.find_last_not_of('x'), h.size() - 1);
  ASSERT_EQ(h.find_last_not_of('='), Piece::npos);
  ASSERT_EQ(b.find_last_not_of('c'), 1U);
  ASSERT_EQ(h.find_last_not_of('x', 2), 2U);
  ASSERT_EQ(h.find_last_not_of('=', 2), Piece::npos);
  ASSERT_EQ(b.find_last_not_of('b', 1), 0U);
  // empty string nonsense
  ASSERT_EQ(d.find_last_not_of('x'), Piece::npos);
  ASSERT_EQ(e.find_last_not_of('x'), Piece::npos);
  ASSERT_EQ(d.find_last_not_of(kNul), Piece::npos);
  ASSERT_EQ(e.find_last_not_of(kNul), Piece::npos);

  ASSERT_EQ(a.substr(0, 3), b);
  ASSERT_EQ(a.substr(23), c);
  ASSERT_EQ(a.substr(23, 3), c);
  ASSERT_EQ(a.substr(23, 99), c);
  ASSERT_EQ(a.substr(), a);
  ASSERT_EQ(a.substr(0), a);
  ASSERT_EQ(a.substr(3, 2), TestFixture::as_string("de"));
  ASSERT_EQ(d.substr(0, 99), e);
}

TYPED_TEST(CommonStringPieceTest, CheckCustom) {
  std::basic_string<TypeParam> foobar(TestFixture::as_string("foobar"));
  BasicStringPiece<TypeParam> a(foobar);
  std::basic_string<TypeParam> s1(TestFixture::as_string("123"));
  s1 += static_cast<TypeParam>('\0');
  s1 += TestFixture::as_string("456");
  [[maybe_unused]] BasicStringPiece<TypeParam> b(s1);
  BasicStringPiece<TypeParam> e;
  std::basic_string<TypeParam> s2;

  // remove_prefix
  BasicStringPiece<TypeParam> c(a);
  c.remove_prefix(3);
  ASSERT_EQ(c, TestFixture::as_string("bar"));
  c = a;
  c.remove_prefix(0);
  ASSERT_EQ(c, a);
  c.remove_prefix(c.size());
  ASSERT_EQ(c, e);

  // remove_suffix
  c = a;
  c.remove_suffix(3);
  ASSERT_EQ(c, TestFixture::as_string("foo"));
  c = a;
  c.remove_suffix(0);
  ASSERT_EQ(c, a);
  c.remove_suffix(c.size());
  ASSERT_EQ(c, e);

  // assignment
  c = foobar.c_str();
  ASSERT_EQ(c, a);
  c = {foobar.c_str(), 6};
  ASSERT_EQ(c, a);
  c = {foobar.c_str(), 0};
  ASSERT_EQ(c, e);
  c = {foobar.c_str(), 7};  // Note, has an embedded NULL
  ASSERT_NE(c, a);

  // operator STRING_TYPE()
  std::basic_string<TypeParam> s5(std::basic_string<TypeParam>(a).c_str(),
                                  7);  // Note, has an embedded NULL
  ASSERT_EQ(c, s5);
  std::basic_string<TypeParam> s6(e);
  ASSERT_TRUE(s6.empty());
}

TEST(StringPieceTest, CheckCustom) {
  StringPiece a("foobar");
  std::string s1("123");
  s1 += '\0';
  s1 += "456";
  [[maybe_unused]] StringPiece b(s1);
  StringPiece e;
  std::string s2;

  StringPiece c;
  c = {"foobar", 6};
  ASSERT_EQ(c, a);
  c = {"foobar", 0};
  ASSERT_EQ(c, e);
  c = {"foobar", 7};
  ASSERT_NE(c, a);
}

TYPED_TEST(CommonStringPieceTest, CheckNULL) {
  BasicStringPiece<TypeParam> s;
  ASSERT_EQ(s.data(), nullptr);
  ASSERT_EQ(s.size(), 0U);

  std::basic_string<TypeParam> str(s);
  ASSERT_EQ(str.length(), 0U);
  ASSERT_EQ(str, std::basic_string<TypeParam>());
}

TYPED_TEST(CommonStringPieceTest, CheckComparisons2) {
  std::basic_string<TypeParam> alphabet(
      TestFixture::as_string("abcdefghijklmnopqrstuvwxyz"));
  std::basic_string<TypeParam> alphabet_z(
      TestFixture::as_string("abcdefghijklmnopqrstuvwxyzz"));
  std::basic_string<TypeParam> alphabet_y(
      TestFixture::as_string("abcdefghijklmnopqrstuvwxyy"));
  BasicStringPiece<TypeParam> abc(alphabet);

  // check comparison operations on strings longer than 4 bytes.
  ASSERT_EQ(abc, BasicStringPiece<TypeParam>(alphabet));
  ASSERT_EQ(abc.compare(BasicStringPiece<TypeParam>(alphabet)), 0);

  ASSERT_TRUE(abc < BasicStringPiece<TypeParam>(alphabet_z));
  ASSERT_LT(abc.compare(BasicStringPiece<TypeParam>(alphabet_z)), 0);

  ASSERT_TRUE(abc > BasicStringPiece<TypeParam>(alphabet_y));
  ASSERT_GT(abc.compare(BasicStringPiece<TypeParam>(alphabet_y)), 0);
}

TYPED_TEST(CommonStringPieceTest, StringCompareNotAmbiguous) {
  ASSERT_TRUE(TestFixture::as_string("hello").c_str() ==
              TestFixture::as_string("hello"));
  ASSERT_TRUE(TestFixture::as_string("hello").c_str() <
              TestFixture::as_string("world"));
}

TYPED_TEST(CommonStringPieceTest, HeterogenousStringPieceEquals) {
  std::basic_string<TypeParam> hello(TestFixture::as_string("hello"));

  ASSERT_EQ(BasicStringPiece<TypeParam>(hello), hello);
  ASSERT_EQ(hello.c_str(), BasicStringPiece<TypeParam>(hello));
}

// std::u16string-specific stuff
TEST(StringPiece16Test, CheckSTL) {
  // Check some non-ascii characters.
  std::u16string fifth(u"123");
  fifth.push_back(0x0000);
  fifth.push_back(0xd8c5);
  fifth.push_back(0xdffe);
  StringPiece16 f(fifth);

  ASSERT_EQ(f[3], '\0');
  ASSERT_EQ(f[5], 0xdffe);

  ASSERT_EQ(f.size(), 6U);
}

TEST(StringPiece16Test, CheckConversion) {
  // Make sure that we can convert from UTF8 to UTF16 and back. We use a
  // character (G clef) outside the BMP to test this.
  const char* kTest = "\U0001D11E";
  ASSERT_EQ(UTF16ToUTF8(UTF8ToUTF16(kTest)), kTest);
}

TYPED_TEST(CommonStringPieceTest, CheckConstructors) {
  std::basic_string<TypeParam> str(TestFixture::as_string("hello world"));
  std::basic_string<TypeParam> empty;

  ASSERT_EQ(str, BasicStringPiece<TypeParam>(str));
  ASSERT_EQ(str, BasicStringPiece<TypeParam>(str.c_str()));
  ASSERT_TRUE(TestFixture::as_string("hello") ==
              BasicStringPiece<TypeParam>(str.c_str(), 5));
  ASSERT_EQ(
      empty,
      BasicStringPiece<TypeParam>(
          str.c_str(),
          static_cast<typename BasicStringPiece<TypeParam>::size_type>(0)));
  ASSERT_EQ(empty, BasicStringPiece<TypeParam>());
  ASSERT_TRUE(
      empty ==
      BasicStringPiece<TypeParam>(
          nullptr,
          static_cast<typename BasicStringPiece<TypeParam>::size_type>(0)));
  ASSERT_EQ(empty, BasicStringPiece<TypeParam>());
  ASSERT_EQ(empty, BasicStringPiece<TypeParam>(empty));
}

TEST(StringPieceTest, ConstexprCtor) {
  {
    constexpr StringPiece piece;
    std::ignore = piece;
  }

  {
    constexpr StringPiece piece("abc");
    std::ignore = piece;
  }

  {
    constexpr StringPiece piece("abc", 2);
    std::ignore = piece;
  }
}

// Chromium development assumes StringPiece (which is std::string_view) is
// implemented with an STL that enables hardening checks. We treat bugs that
// trigger one of these conditions as functional rather than security bugs. If
// this test fails on some embedder, it should not be disabled. Instead, the
// embedder should fix their STL or build configuration to enable corresponding
// hardening checks.
//
// See https://chromium.googlesource.com/chromium/src/+/main/docs/security/faq.md#indexing-a-container-out-of-bounds-hits-a-libcpp_verbose_abort_is-this-a-security-bug
TEST(StringPieceTest, OutOfBoundsDeath) {
  {
    constexpr StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece[0], "");
  }

  {
    constexpr StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.front(), "");
  }

  {
    constexpr StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.back(), "");
  }

  {
    StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.remove_suffix(1), "");
  }

  {
    StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.remove_prefix(1), "");
  }

  {
    StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.copy(nullptr, 0, 1), "");
  }

  {
    StringPiece piece;
    ASSERT_DEATH_IF_SUPPORTED(piece.substr(1), "");
  }
}

TEST(StringPieceTest, InvalidLengthDeath) {
  int length = -1;
  ASSERT_DEATH_IF_SUPPORTED({ StringPiece piece("hello", length); }, "");
}

TEST(StringPieceTest, ConstexprData) {
  {
    constexpr StringPiece piece;
    static_assert(piece.data() == nullptr, "");
  }

  {
    constexpr StringPiece piece("abc");
    static_assert(piece.data()[0] == 'a', "");
    static_assert(piece.data()[1] == 'b', "");
    static_assert(piece.data()[2] == 'c', "");
  }

  {
    constexpr StringPiece piece("def", 2);
    static_assert(piece.data()[0] == 'd', "");
    static_assert(piece.data()[1] == 'e', "");
  }
}

TEST(StringPieceTest, ConstexprSize) {
  {
    constexpr StringPiece piece;
    static_assert(piece.size() == 0, "");
  }

  {
    constexpr StringPiece piece("abc");
    static_assert(piece.size() == 3, "");
  }

  {
    constexpr StringPiece piece("def", 2);
    static_assert(piece.size() == 2, "");
  }
}

TEST(StringPieceTest, ConstexprFront) {
  static_assert(StringPiece("abc").front() == 'a', "");
}

TEST(StringPieceTest, ConstexprBack) {
  static_assert(StringPiece("abc").back() == 'c', "");
}

TEST(StringPieceTest, Compare) {
  constexpr StringPiece piece = "def";

  static_assert(piece.compare("ab") == 1, "");
  static_assert(piece.compare("abc") == 1, "");
  static_assert(piece.compare("abcd") == 1, "");
  static_assert(piece.compare("de") == 1, "");
  static_assert(piece.compare("def") == 0, "");
  static_assert(piece.compare("defg") == -1, "");
  static_assert(piece.compare("gh") == -1, "");
  static_assert(piece.compare("ghi") == -1, "");
  static_assert(piece.compare("ghij") == -1, "");

  static_assert(piece.compare(0, 0, "") == 0, "");
  static_assert(piece.compare(0, 1, "d") == 0, "");
  static_assert(piece.compare(0, 2, "de") == 0, "");
  static_assert(piece.compare(0, 3, "def") == 0, "");
  static_assert(piece.compare(1, 0, "") == 0, "");
  static_assert(piece.compare(1, 1, "e") == 0, "");
  static_assert(piece.compare(1, 2, "ef") == 0, "");
  static_assert(piece.compare(1, 3, "ef") == 0, "");
  static_assert(piece.compare(2, 0, "") == 0, "");
  static_assert(piece.compare(2, 1, "f") == 0, "");
  static_assert(piece.compare(2, 2, "f") == 0, "");
  static_assert(piece.compare(2, 3, "f") == 0, "");
  static_assert(piece.compare(3, 0, "") == 0, "");
  static_assert(piece.compare(3, 1, "") == 0, "");
  static_assert(piece.compare(3, 2, "") == 0, "");
  static_assert(piece.compare(3, 3, "") == 0, "");

  static_assert(piece.compare(0, 0, "def", 0) == 0, "");
  static_assert(piece.compare(0, 1, "def", 1) == 0, "");
  static_assert(piece.compare(0, 2, "def", 2) == 0, "");
  static_assert(piece.compare(0, 3, "def", 3) == 0, "");
  static_assert(piece.compare(1, 0, "ef", 0) == 0, "");
  static_assert(piece.compare(1, 1, "ef", 1) == 0, "");
  static_assert(piece.compare(1, 2, "ef", 2) == 0, "");
  static_assert(piece.compare(1, 3, "ef", 2) == 0, "");
  static_assert(piece.compare(2, 0, "f", 0) == 0, "");
  static_assert(piece.compare(2, 1, "f", 1) == 0, "");
  static_assert(piece.compare(2, 2, "f", 1) == 0, "");
  static_assert(piece.compare(2, 3, "f", 1) == 0, "");
  static_assert(piece.compare(3, 0, "", 0) == 0, "");
  static_assert(piece.compare(3, 1, "", 0) == 0, "");
  static_assert(piece.compare(3, 2, "", 0) == 0, "");
  static_assert(piece.compare(3, 3, "", 0) == 0, "");

  static_assert(piece.compare(0, 0, "def", 0, 0) == 0, "");
  static_assert(piece.compare(0, 1, "def", 0, 1) == 0, "");
  static_assert(piece.compare(0, 2, "def", 0, 2) == 0, "");
  static_assert(piece.compare(0, 3, "def", 0, 3) == 0, "");
  static_assert(piece.compare(1, 0, "def", 1, 0) == 0, "");
  static_assert(piece.compare(1, 1, "def", 1, 1) == 0, "");
  static_assert(piece.compare(1, 2, "def", 1, 2) == 0, "");
  static_assert(piece.compare(1, 3, "def", 1, 3) == 0, "");
  static_assert(piece.compare(2, 0, "def", 2, 0) == 0, "");
  static_assert(piece.compare(2, 1, "def", 2, 1) == 0, "");
  static_assert(piece.compare(2, 2, "def", 2, 2) == 0, "");
  static_assert(piece.compare(2, 3, "def", 2, 3) == 0, "");
  static_assert(piece.compare(3, 0, "def", 3, 0) == 0, "");
  static_assert(piece.compare(3, 1, "def", 3, 1) == 0, "");
  static_assert(piece.compare(3, 2, "def", 3, 2) == 0, "");
  static_assert(piece.compare(3, 3, "def", 3, 3) == 0, "");
}

TEST(StringPieceTest, Substr) {
  constexpr StringPiece piece = "abcdefghijklmnopqrstuvwxyz";

  static_assert(piece.substr(0, 2) == "ab", "");
  static_assert(piece.substr(0, 3) == "abc", "");
  static_assert(piece.substr(0, 4) == "abcd", "");
  static_assert(piece.substr(3, 2) == "de", "");
  static_assert(piece.substr(3, 3) == "def", "");
  static_assert(piece.substr(23) == "xyz", "");
  static_assert(piece.substr(23, 3) == "xyz", "");
  static_assert(piece.substr(23, 99) == "xyz", "");
  static_assert(piece.substr() == piece, "");
  static_assert(piece.substr(0) == piece, "");
  static_assert(piece.substr(0, 99) == piece, "");
}

TEST(StringPieceTest, Find) {
  constexpr StringPiece foobar("foobar", 6);
  constexpr StringPiece foo = foobar.substr(0, 3);
  constexpr StringPiece bar = foobar.substr(3);

  // find
  static_assert(foobar.find(bar, 0) == 3, "");
  static_assert(foobar.find('o', 0) == 1, "");
  static_assert(foobar.find("ox", 0, 1) == 1, "");
  static_assert(foobar.find("ox", 0) == StringPiece::npos, "");

  // rfind
  static_assert(foobar.rfind(bar, 5) == 3, "");
  static_assert(foobar.rfind('o', 5) == 2, "");
  static_assert(foobar.rfind("ox", 5, 1) == 2, "");
  static_assert(foobar.rfind("ox", 5) == StringPiece::npos, "");

  // find_first_of
  static_assert(foobar.find_first_of(foo, 2) == 2, "");
  static_assert(foobar.find_first_of('o', 2) == 2, "");
  static_assert(foobar.find_first_of("ox", 2, 2) == 2, "");
  static_assert(foobar.find_first_of("ox", 2) == 2, "");

  // find_last_of
  static_assert(foobar.find_last_of(foo, 5) == 2, "");
  static_assert(foobar.find_last_of('o', 5) == 2, "");
  static_assert(foobar.find_last_of("ox", 5, 2) == 2, "");
  static_assert(foobar.find_last_of("ox", 5) == 2, "");

  // find_first_not_of
  static_assert(foobar.find_first_not_of(foo, 2) == 3, "");
  static_assert(foobar.find_first_not_of('o', 2) == 3, "");
  static_assert(foobar.find_first_not_of("ox", 2, 2) == 3, "");
  static_assert(foobar.find_first_not_of("ox", 2) == 3, "");

  // find_last_not_of
  static_assert(foobar.find_last_not_of(bar, 5) == 2, "");
  static_assert(foobar.find_last_not_of('a', 4) == 3, "");
  static_assert(foobar.find_last_not_of("ox", 2, 2) == 0, "");
  static_assert(foobar.find_last_not_of("ox", 2) == 0, "");
}

// Test that `gurl_base::StringPiece` and `std::string_view` are interoperable.
TEST(StringPieceTest, StringPieceToStringView) {
  constexpr StringPiece kPiece = "foo";
  constexpr std::string_view kView = kPiece;
  static_assert(kPiece.data() == kView.data());
  static_assert(kPiece.size() == kView.size());
}

TEST(StringPieceTest, StringViewToStringPiece) {
  constexpr std::string_view kView = "bar";
  constexpr StringPiece kPiece = kView;
  static_assert(kView.data() == kPiece.data());
  static_assert(kView.size() == kPiece.size());
}

}  // namespace base
