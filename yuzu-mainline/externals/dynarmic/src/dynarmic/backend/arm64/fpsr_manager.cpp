/* This file is part of the dynarmic project.
 * Copyright (c) 2022 MerryMage
 * SPDX-License-Identifier: 0BSD
 */

#include "dynarmic/backend/arm64/fpsr_manager.h"

#include <oaknut/oaknut.hpp>

#include "dynarmic/backend/arm64/abi.h"

namespace Dynarmic::Backend::Arm64 {

using namespace oaknut::util;

FpsrManager::FpsrManager(oaknut::CodeGenerator& code, size_t state_fpsr_offset)
        : code{code}, state_fpsr_offset{state_fpsr_offset} {}

void FpsrManager::Spill() {
    if (!fpsr_loaded)
        return;

    code.LDR(Wscratch0, Xstate, state_fpsr_offset);
    code.MRS(Xscratch1, oaknut::SystemReg::FPSR);
    code.ORR(Wscratch0, Wscratch0, Wscratch1);
    code.STR(Wscratch0, Xstate, state_fpsr_offset);

    fpsr_loaded = false;
}

void FpsrManager::Load() {
    if (fpsr_loaded)
        return;

    code.MSR(oaknut::SystemReg::FPSR, XZR);

    fpsr_loaded = true;
}

}  // namespace Dynarmic::Backend::Arm64
