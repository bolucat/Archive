#pragma once

#include "QvPlugin/Common/CommonTypes.hpp"

namespace Qv2rayPlugin::Profile
{
    class IProfilePreprocessor
    {
      public:
        IProfilePreprocessor() = default;
        virtual ~IProfilePreprocessor() = default;
        virtual ProfileContent PreprocessProfile(const ProfileContent &) = 0;
    };
} // namespace Qv2rayPlugin::Profile
