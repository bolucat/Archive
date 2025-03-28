// Copyright 2010 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "base/strings/utf_string_conversions.h"

#include <stddef.h>

#include "base/strings/string_piece.h"
#include "base/strings/string_util.h"
#include "build/build_config.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace gurl_base {

namespace {

const wchar_t* const kConvertRoundtripCases[] = {
    L"Google Video",
    // "网页 图片 资讯更多 »"
    L"\x7f51\x9875\x0020\x56fe\x7247\x0020\x8d44\x8baf\x66f4\x591a\x0020\x00bb",
    //  "Παγκόσμιος Ιστός"
    L"\x03a0\x03b1\x03b3\x03ba\x03cc\x03c3\x03bc\x03b9"
    L"\x03bf\x03c2\x0020\x0399\x03c3\x03c4\x03cc\x03c2",
    // "Поиск страниц на русском"
    L"\x041f\x043e\x0438\x0441\x043a\x0020\x0441\x0442"
    L"\x0440\x0430\x043d\x0438\x0446\x0020\x043d\x0430"
    L"\x0020\x0440\x0443\x0441\x0441\x043a\x043e\x043c",
    // "전체서비스"
    L"\xc804\xccb4\xc11c\xbe44\xc2a4",

// Test characters that take more than 16 bits. This will depend on whether
// wchar_t is 16 or 32 bits.
#if defined(WCHAR_T_IS_16_BIT)
    L"\xd800\xdf00",
    // ?????  (Mathematical Alphanumeric Symbols (U+011d40 - U+011d44 :
    // A,B,C,D,E)
    L"\xd807\xdd40\xd807\xdd41\xd807\xdd42\xd807\xdd43\xd807\xdd44",
#elif defined(WCHAR_T_IS_32_BIT)
    L"\x10300",
    // ?????  (Mathematical Alphanumeric Symbols (U+011d40 - U+011d44 :
    // A,B,C,D,E)
    L"\x11d40\x11d41\x11d42\x11d43\x11d44",
#endif
};

}  // namespace

TEST(UTFStringConversionsTest, ConvertUTF8AndWide) {
  // we round-trip all the wide strings through UTF-8 to make sure everything
  // agrees on the conversion. This uses the stream operators to test them
  // simultaneously.
  for (auto* i : kConvertRoundtripCases) {
    std::ostringstream utf8;
    utf8 << WideToUTF8(i);
    std::wostringstream wide;
    wide << UTF8ToWide(utf8.str());

    EXPECT_EQ(i, wide.str());
  }
}

TEST(UTFStringConversionsTest, ConvertUTF8AndWideEmptyString) {
  // An empty std::wstring should be converted to an empty std::string,
  // and vice versa.
  std::wstring wempty;
  std::string empty;
  EXPECT_EQ(empty, WideToUTF8(wempty));
  EXPECT_EQ(wempty, UTF8ToWide(empty));
}

TEST(UTFStringConversionsTest, ConvertUTF8ToWide) {
  struct UTF8ToWideCase {
    const char* utf8;
    const wchar_t* wide;
    bool success;
  } convert_cases[] = {
    // Regular UTF-8 input.
    {"\xe4\xbd\xa0\xe5\xa5\xbd", L"\x4f60\x597d", true},
    // Non-character is passed through.
    {"\xef\xbf\xbfHello", L"\xffffHello", true},
    // Truncated UTF-8 sequence.
    {"\xe4\xa0\xe5\xa5\xbd", L"\xfffd\x597d", false},
    // Truncated off the end.
    {"\xe5\xa5\xbd\xe4\xa0", L"\x597d\xfffd", false},
    // Non-shortest-form UTF-8.
    {"\xf0\x84\xbd\xa0\xe5\xa5\xbd", L"\xfffd\xfffd\xfffd\xfffd\x597d", false},
    // This UTF-8 character decodes to a UTF-16 surrogate, which is illegal.
    {"\xed\xb0\x80", L"\xfffd\xfffd\xfffd", false},
    // Non-BMP characters. The second is a non-character regarded as valid.
    // The result will either be in UTF-16 or UTF-32.
#if defined(WCHAR_T_IS_16_BIT)
    {"A\xF0\x90\x8C\x80z", L"A\xd800\xdf00z", true},
    {"A\xF4\x8F\xBF\xBEz", L"A\xdbff\xdffez", true},
#elif defined(WCHAR_T_IS_32_BIT)
    {"A\xF0\x90\x8C\x80z", L"A\x10300z", true},
    {"A\xF4\x8F\xBF\xBEz", L"A\x10fffez", true},
#endif
  };

  for (const auto& i : convert_cases) {
    std::wstring converted;
    EXPECT_EQ(i.success, UTF8ToWide(i.utf8, strlen(i.utf8), &converted));
    std::wstring expected(i.wide);
    EXPECT_EQ(expected, converted);
  }

  // Manually test an embedded NULL.
  std::wstring converted;
  EXPECT_TRUE(UTF8ToWide("\00Z\t", 3, &converted));
  ASSERT_EQ(3U, converted.length());
  EXPECT_EQ(static_cast<wchar_t>(0), converted[0]);
  EXPECT_EQ('Z', converted[1]);
  EXPECT_EQ('\t', converted[2]);

  // Make sure that conversion replaces, not appends.
  EXPECT_TRUE(UTF8ToWide("B", 1, &converted));
  ASSERT_EQ(1U, converted.length());
  EXPECT_EQ('B', converted[0]);
}

#if defined(WCHAR_T_IS_16_BIT)
// This test is only valid when wchar_t == UTF-16.
TEST(UTFStringConversionsTest, ConvertUTF16ToUTF8) {
  struct WideToUTF8Case {
    const wchar_t* utf16;
    const char* utf8;
    bool success;
  } convert_cases[] = {
    // Regular UTF-16 input.
    {L"\x4f60\x597d", "\xe4\xbd\xa0\xe5\xa5\xbd", true},
    // Test a non-BMP character.
    {L"\xd800\xdf00", "\xF0\x90\x8C\x80", true},
    // Non-characters are passed through.
    {L"\xffffHello", "\xEF\xBF\xBFHello", true},
    {L"\xdbff\xdffeHello", "\xF4\x8F\xBF\xBEHello", true},
    // The first character is a truncated UTF-16 character.
    {L"\xd800\x597d", "\xef\xbf\xbd\xe5\xa5\xbd", false},
    // Truncated at the end.
    {L"\x597d\xd800", "\xe5\xa5\xbd\xef\xbf\xbd", false},
  };

  for (const auto& test : convert_cases) {
    std::string converted;
    EXPECT_EQ(test.success,
              WideToUTF8(test.utf16, wcslen(test.utf16), &converted));
    std::string expected(test.utf8);
    EXPECT_EQ(expected, converted);
  }
}

#elif defined(WCHAR_T_IS_32_BIT)
// This test is only valid when wchar_t == UTF-32.
TEST(UTFStringConversionsTest, ConvertUTF32ToUTF8) {
  struct WideToUTF8Case {
    const wchar_t* utf32;
    const char* utf8;
    bool success;
  } convert_cases[] = {
    // Regular 16-bit input.
    {L"\x4f60\x597d", "\xe4\xbd\xa0\xe5\xa5\xbd", true},
    // Test a non-BMP character.
    {L"A\x10300z", "A\xF0\x90\x8C\x80z", true},
    // Non-characters are passed through.
    {L"\xffffHello", "\xEF\xBF\xBFHello", true},
    {L"\x10fffeHello", "\xF4\x8F\xBF\xBEHello", true},
    // Invalid Unicode code points.
    {L"\xfffffffHello", "\xEF\xBF\xBDHello", false},
    // The first character is a truncated UTF-16 character.
    {L"\xd800\x597d", "\xef\xbf\xbd\xe5\xa5\xbd", false},
    {L"\xdc01Hello", "\xef\xbf\xbdHello", false},
  };

  for (const auto& test : convert_cases) {
    std::string converted;
    EXPECT_EQ(test.success,
              WideToUTF8(test.utf32, wcslen(test.utf32), &converted));
    std::string expected(test.utf8);
    EXPECT_EQ(expected, converted);
  }
}
#endif  // defined(WCHAR_T_IS_32_BIT)

TEST(UTFStringConversionsTest, ConvertMultiString) {
  static char16_t multi16[] = {'f',  'o', 'o', '\0', 'b',  'a', 'r',
                               '\0', 'b', 'a', 'z',  '\0', '\0'};
  static char multi[] = {
    'f', 'o', 'o', '\0',
    'b', 'a', 'r', '\0',
    'b', 'a', 'z', '\0',
    '\0'
  };
  std::u16string multistring16;
  memcpy(WriteInto(&multistring16, std::size(multi16)), multi16,
         sizeof(multi16));
  EXPECT_EQ(std::size(multi16) - 1, multistring16.length());
  std::string expected;
  memcpy(WriteInto(&expected, std::size(multi)), multi, sizeof(multi));
  EXPECT_EQ(std::size(multi) - 1, expected.length());
  const std::string& converted = UTF16ToUTF8(multistring16);
  EXPECT_EQ(std::size(multi) - 1, converted.length());
  EXPECT_EQ(expected, converted);
}

}  // namespace base
