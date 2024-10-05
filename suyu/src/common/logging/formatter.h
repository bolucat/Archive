// SPDX-FileCopyrightText: Copyright 2022 yuzu Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <type_traits>

#include <fmt/format.h>

// adapted from https://github.com/fmtlib/fmt/issues/2704
// a generic formatter for enum classes
#if FMT_VERSION >= 80100
template <typename T>
struct fmt::formatter<T, std::enable_if_t<std::is_enum_v<T>, char>>
    : formatter<std::underlying_type_t<T>> {
    template <typename FormatContext>
    auto format(const T& value, FormatContext& ctx) const -> decltype(ctx.out()) {
        return fmt::formatter<std::underlying_type_t<T>>::format(
            static_cast<std::underlying_type_t<T>>(value), ctx);
    }
};
#endif
