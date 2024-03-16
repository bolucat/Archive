/* This file is part of the dynarmic project.
 * Copyright (c) 2022 MerryMage
 * SPDX-License-Identifier: 0BSD
 */

#pragma once

#include <mcl/stdint.hpp>

namespace oaknut {
struct CodeGenerator;
}  // namespace oaknut

namespace Dynarmic::Backend::Arm64 {

class FpsrManager {
public:
    explicit FpsrManager(oaknut::CodeGenerator& code, size_t state_fpsr_offset);

    void Spill();
    void Load();
    void Overwrite() { fpsr_loaded = false; }

private:
    oaknut::CodeGenerator& code;
    size_t state_fpsr_offset;
    bool fpsr_loaded = false;
};

}  // namespace Dynarmic::Backend::Arm64
