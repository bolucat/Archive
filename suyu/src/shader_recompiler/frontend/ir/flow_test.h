// SPDX-FileCopyrightText: Copyright 2021 yuzu Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <string>
#include <fmt/format.h>

#include "common/common_types.h"

namespace Shader::IR {

enum class FlowTest : u64 {
    F,
    LT,
    EQ,
    LE,
    GT,
    NE,
    GE,
    NUM,
    NaN,
    LTU,
    EQU,
    LEU,
    GTU,
    NEU,
    GEU,
    T,
    OFF,
    LO,
    SFF,
    LS,
    HI,
    SFT,
    HS,
    OFT,
    CSM_TA,
    CSM_TR,
    CSM_MX,
    FCSM_TA,
    FCSM_TR,
    FCSM_MX,
    RLE,
    RGT,
};

[[nodiscard]] std::string NameOf(FlowTest flow_test);

} // namespace Shader::IR

template <>
struct fmt::formatter<Shader::IR::FlowTest> {
    constexpr auto parse(format_parse_context& ctx) {
        return ctx.begin();
    }
    template <typename FormatContext>
    auto format(const Shader::IR::FlowTest& flow_test, FormatContext& ctx) const {
        return fmt::format_to(ctx.out(), "{}", Shader::IR::NameOf(flow_test));
    }
};
