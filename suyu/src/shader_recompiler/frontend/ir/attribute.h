// SPDX-FileCopyrightText: Copyright 2021 yuzu Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <fmt/format.h>

#include "common/common_types.h"

namespace Shader::IR {

enum class Attribute : u64 {
    PrimitiveId = 24,
    Layer = 25,
    ViewportIndex = 26,
    PointSize = 27,
    PositionX = 28,
    PositionY = 29,
    PositionZ = 30,
    PositionW = 31,
    Generic0X = 32,
    Generic0Y = 33,
    Generic0Z = 34,
    Generic0W = 35,
    Generic1X = 36,
    Generic1Y = 37,
    Generic1Z = 38,
    Generic1W = 39,
    Generic2X = 40,
    Generic2Y = 41,
    Generic2Z = 42,
    Generic2W = 43,
    Generic3X = 44,
    Generic3Y = 45,
    Generic3Z = 46,
    Generic3W = 47,
    Generic4X = 48,
    Generic4Y = 49,
    Generic4Z = 50,
    Generic4W = 51,
    Generic5X = 52,
    Generic5Y = 53,
    Generic5Z = 54,
    Generic5W = 55,
    Generic6X = 56,
    Generic6Y = 57,
    Generic6Z = 58,
    Generic6W = 59,
    Generic7X = 60,
    Generic7Y = 61,
    Generic7Z = 62,
    Generic7W = 63,
    Generic8X = 64,
    Generic8Y = 65,
    Generic8Z = 66,
    Generic8W = 67,
    Generic9X = 68,
    Generic9Y = 69,
    Generic9Z = 70,
    Generic9W = 71,
    Generic10X = 72,
    Generic10Y = 73,
    Generic10Z = 74,
    Generic10W = 75,
    Generic11X = 76,
    Generic11Y = 77,
    Generic11Z = 78,
    Generic11W = 79,
    Generic12X = 80,
    Generic12Y = 81,
    Generic12Z = 82,
    Generic12W = 83,
    Generic13X = 84,
    Generic13Y = 85,
    Generic13Z = 86,
    Generic13W = 87,
    Generic14X = 88,
    Generic14Y = 89,
    Generic14Z = 90,
    Generic14W = 91,
    Generic15X = 92,
    Generic15Y = 93,
    Generic15Z = 94,
    Generic15W = 95,
    Generic16X = 96,
    Generic16Y = 97,
    Generic16Z = 98,
    Generic16W = 99,
    Generic17X = 100,
    Generic17Y = 101,
    Generic17Z = 102,
    Generic17W = 103,
    Generic18X = 104,
    Generic18Y = 105,
    Generic18Z = 106,
    Generic18W = 107,
    Generic19X = 108,
    Generic19Y = 109,
    Generic19Z = 110,
    Generic19W = 111,
    Generic20X = 112,
    Generic20Y = 113,
    Generic20Z = 114,
    Generic20W = 115,
    Generic21X = 116,
    Generic21Y = 117,
    Generic21Z = 118,
    Generic21W = 119,
    Generic22X = 120,
    Generic22Y = 121,
    Generic22Z = 122,
    Generic22W = 123,
    Generic23X = 124,
    Generic23Y = 125,
    Generic23Z = 126,
    Generic23W = 127,
    Generic24X = 128,
    Generic24Y = 129,
    Generic24Z = 130,
    Generic24W = 131,
    Generic25X = 132,
    Generic25Y = 133,
    Generic25Z = 134,
    Generic25W = 135,
    Generic26X = 136,
    Generic26Y = 137,
    Generic26Z = 138,
    Generic26W = 139,
    Generic27X = 140,
    Generic27Y = 141,
    Generic27Z = 142,
    Generic27W = 143,
    Generic28X = 144,
    Generic28Y = 145,
    Generic28Z = 146,
    Generic28W = 147,
    Generic29X = 148,
    Generic29Y = 149,
    Generic29Z = 150,
    Generic29W = 151,
    Generic30X = 152,
    Generic30Y = 153,
    Generic30Z = 154,
    Generic30W = 155,
    Generic31X = 156,
    Generic31Y = 157,
    Generic31Z = 158,
    Generic31W = 159,
    ColorFrontDiffuseR = 160,
    ColorFrontDiffuseG = 161,
    ColorFrontDiffuseB = 162,
    ColorFrontDiffuseA = 163,
    ColorFrontSpecularR = 164,
    ColorFrontSpecularG = 165,
    ColorFrontSpecularB = 166,
    ColorFrontSpecularA = 167,
    ColorBackDiffuseR = 168,
    ColorBackDiffuseG = 169,
    ColorBackDiffuseB = 170,
    ColorBackDiffuseA = 171,
    ColorBackSpecularR = 172,
    ColorBackSpecularG = 173,
    ColorBackSpecularB = 174,
    ColorBackSpecularA = 175,
    ClipDistance0 = 176,
    ClipDistance1 = 177,
    ClipDistance2 = 178,
    ClipDistance3 = 179,
    ClipDistance4 = 180,
    ClipDistance5 = 181,
    ClipDistance6 = 182,
    ClipDistance7 = 183,
    PointSpriteS = 184,
    PointSpriteT = 185,
    FogCoordinate = 186,
    TessellationEvaluationPointU = 188,
    TessellationEvaluationPointV = 189,
    InstanceId = 190,
    VertexId = 191,
    FixedFncTexture0S = 192,
    FixedFncTexture0T = 193,
    FixedFncTexture0R = 194,
    FixedFncTexture0Q = 195,
    FixedFncTexture1S = 196,
    FixedFncTexture1T = 197,
    FixedFncTexture1R = 198,
    FixedFncTexture1Q = 199,
    FixedFncTexture2S = 200,
    FixedFncTexture2T = 201,
    FixedFncTexture2R = 202,
    FixedFncTexture2Q = 203,
    FixedFncTexture3S = 204,
    FixedFncTexture3T = 205,
    FixedFncTexture3R = 206,
    FixedFncTexture3Q = 207,
    FixedFncTexture4S = 208,
    FixedFncTexture4T = 209,
    FixedFncTexture4R = 210,
    FixedFncTexture4Q = 211,
    FixedFncTexture5S = 212,
    FixedFncTexture5T = 213,
    FixedFncTexture5R = 214,
    FixedFncTexture5Q = 215,
    FixedFncTexture6S = 216,
    FixedFncTexture6T = 217,
    FixedFncTexture6R = 218,
    FixedFncTexture6Q = 219,
    FixedFncTexture7S = 220,
    FixedFncTexture7T = 221,
    FixedFncTexture7R = 222,
    FixedFncTexture7Q = 223,
    FixedFncTexture8S = 224,
    FixedFncTexture8T = 225,
    FixedFncTexture8R = 226,
    FixedFncTexture8Q = 227,
    FixedFncTexture9S = 228,
    FixedFncTexture9T = 229,
    FixedFncTexture9R = 230,
    FixedFncTexture9Q = 231,
    ViewportMask = 232,
    FrontFace = 255,

    // Implementation attributes
    BaseInstance = 256,
    BaseVertex = 257,
    DrawID = 258,
};

constexpr size_t NUM_GENERICS = 32;

constexpr size_t NUM_FIXEDFNCTEXTURE = 10;

[[nodiscard]] bool IsGeneric(Attribute attribute) noexcept;

[[nodiscard]] u32 GenericAttributeIndex(Attribute attribute);

[[nodiscard]] u32 GenericAttributeElement(Attribute attribute);

[[nodiscard]] std::string NameOf(Attribute attribute);

[[nodiscard]] constexpr IR::Attribute operator+(IR::Attribute attribute, size_t value) noexcept {
    return static_cast<IR::Attribute>(static_cast<size_t>(attribute) + value);
}

} // namespace Shader::IR

template <>
struct fmt::formatter<Shader::IR::Attribute> {
    constexpr auto parse(format_parse_context& ctx) {
        return ctx.begin();
    }
    template <typename FormatContext>
    auto format(const Shader::IR::Attribute& attribute, FormatContext& ctx) const {
        return fmt::format_to(ctx.out(), "{}", Shader::IR::NameOf(attribute));
    }
};
