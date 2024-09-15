// SPDX-FileCopyrightText: Copyright 2018 yuzu Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#include <algorithm>
#include <cstring>

#include <fmt/ostream.h>

#include "common/hex_util.h"
#include "common/logging/log.h"
#include "core/crypto/key_manager.h"
#include "core/file_sys/content_archive.h"
#include "core/file_sys/nca_metadata.h"
#include "core/file_sys/partition_filesystem.h"
#include "core/file_sys/program_metadata.h"
#include "core/file_sys/submission_package.h"
#include "core/loader/loader.h"

namespace FileSys {

NSP::NSP(VirtualFile file_, u64 title_id_, std::size_t program_index_)
    : file(std::move(file_)), expected_program_id(title_id_),
      program_index(program_index_), status{Loader::ResultStatus::Success},
      pfs(std::make_shared<PartitionFilesystem>(file)), keys{Core::Crypto::KeyManager::Instance()} {
    if (pfs->GetStatus() != Loader::ResultStatus::Success) {
        status = pfs->GetStatus();
        return;
    }

    const auto files = pfs->GetFiles();

    if (IsDirectoryExeFS(pfs)) {
        extracted = true;
        InitializeExeFSAndRomFS(files);
        return;
    }

    SetTicketKeys(files);
    ReadNCAs(files);
}

NSP::~NSP() = default;

Loader::ResultStatus NSP::GetStatus() const {
    return status;
}

Loader::ResultStatus NSP::GetProgramStatus() const {
    if (IsExtractedType() && GetExeFS() != nullptr && FileSys::IsDirectoryExeFS(GetExeFS())) {
        return Loader::ResultStatus::Success;
    }

    const auto iter = program_status.find(GetProgramTitleID());
    if (iter == program_status.end())
        return Loader::ResultStatus::ErrorNSPMissingProgramNCA;
    return iter->second;
}

u64 NSP::GetProgramTitleID() const {
    if (IsExtractedType()) {
        return GetExtractedTitleID() + program_index;
    }

    auto program_id = expected_program_id;
    if (program_id == 0) {
        if (!program_status.empty()) {
            program_id = program_status.begin()->first;
        }
    }

    program_id = program_id + program_index;
    if (program_status.find(program_id) != program_status.end()) {
        return program_id;
    }

    const auto ids = GetProgramTitleIDs();
    const auto iter =
        std::find_if(ids.begin(), ids.end(), [](u64 tid) { return (tid & 0x800) == 0; });
    return iter == ids.end() ? 0 : *iter;
}

u64 NSP::GetExtractedTitleID() const {
    if (GetExeFS() == nullptr || !IsDirectoryExeFS(GetExeFS())) {
        return 0;
    }

    ProgramMetadata meta;
    if (meta.Load(GetExeFS()->GetFile("main.npdm")) == Loader::ResultStatus::Success) {
        return meta.GetTitleID();
    } else {
        return 0;
    }
}

std::vector<u64> NSP::GetProgramTitleIDs() const {
    if (IsExtractedType()) {
        return {GetExtractedTitleID()};
    }

    std::vector<u64> out{program_ids.cbegin(), program_ids.cend()};
    return out;
}

bool NSP::IsExtractedType() const {
    return extracted;
}

VirtualFile NSP::GetRomFS() const {
    return romfs;
}

VirtualDir NSP::GetExeFS() const {
    return exefs;
}

std::vector<std::shared_ptr<NCA>> NSP::GetNCAsCollapsed() const {
    if (extracted)
        LOG_WARNING(Service_FS, "called on an NSP that is of type extracted.");
    std::vector<std::shared_ptr<NCA>> out;
    out.reserve(ncas.size());
    for (const auto& map : ncas) {
        out.reserve(map.second.size());
        for (const auto& inner_map : map.second)
            out.push_back(inner_map.second);
    }
    return out;
}

std::multimap<u64, std::shared_ptr<NCA>> NSP::GetNCAsByTitleID() const {
    if (extracted)
        LOG_WARNING(Service_FS, "called on an NSP that is of type extracted.");
    std::multimap<u64, std::shared_ptr<NCA>> out;
    for (const auto& map : ncas) {
        for (const auto& inner_map : map.second)
            out.emplace(map.first, inner_map.second);
    }
    return out;
}

std::map<u64, std::map<std::pair<TitleType, ContentRecordType>, std::shared_ptr<NCA>>>
NSP::GetNCAs() const {
    return ncas;
}

std::shared_ptr<NCA> NSP::GetNCA(u64 title_id, ContentRecordType type, TitleType title_type) const {
    if (extracted)
        LOG_WARNING(Service_FS, "called on an NSP that is of type extracted.");

    const auto title_id_iter = ncas.find(title_id);
    if (title_id_iter == ncas.end())
        return nullptr;

    const auto type_iter = title_id_iter->second.find({title_type, type});
    if (type_iter == title_id_iter->second.end())
        return nullptr;

    return type_iter->second;
}

VirtualFile NSP::GetNCAFile(u64 title_id, ContentRecordType type, TitleType title_type) const {
    if (extracted)
        LOG_WARNING(Service_FS, "called on an NSP that is of type extracted.");
    const auto nca = GetNCA(title_id, type, title_type);
    if (nca != nullptr)
        return nca->GetBaseFile();
    return nullptr;
}

std::vector<VirtualFile> NSP::GetFiles() const {
    return pfs->GetFiles();
}

std::vector<VirtualDir> NSP::GetSubdirectories() const {
    return pfs->GetSubdirectories();
}

std::string NSP::GetName() const {
    return file->GetName();
}

VirtualDir NSP::GetParentDirectory() const {
    return file->GetContainingDirectory();
}

void NSP::SetTicketKeys(const std::vector<VirtualFile>& files) {
    for (const auto& ticket_file : files) {
        if (ticket_file == nullptr) {
            continue;
        }

        if (ticket_file->GetExtension() != "tik") {
            continue;
        }

        auto ticket = Core::Crypto::Ticket::Read(ticket_file);
        if (!keys.AddTicket(ticket)) {
            LOG_WARNING(Common_Filesystem, "Could not load NSP ticket {}", ticket_file->GetName());
            continue;
        }
    }
}

void NSP::InitializeExeFSAndRomFS(const std::vector<VirtualFile>& files) {
    exefs = pfs;

    const auto iter = std::find_if(files.begin(), files.end(), [](const VirtualFile& entry) {
        return entry->GetName().rfind(".romfs") != std::string::npos;
    });

    if (iter == files.end()) {
        return;
    }

    romfs = *iter;
}

void NSP::ReadNCAs(const std::vector<VirtualFile>& files) {
    for (const auto& outer_file : files) {
        if (outer_file->GetName().size() < 9 ||
            outer_file->GetName().substr(outer_file->GetName().size() - 9) != ".cnmt.nca") {
            continue;
        }

        const auto nca = std::make_shared<NCA>(outer_file);
        if (nca->GetStatus() != Loader::ResultStatus::Success || nca->GetSubdirectories().empty()) {
            program_status[nca->GetTitleId()] = nca->GetStatus();
            continue;
        }

        const auto section0 = nca->GetSubdirectories()[0];

        for (const auto& inner_file : section0->GetFiles()) {
            if (inner_file->GetExtension() != "cnmt") {
                continue;
            }

            const CNMT cnmt(inner_file);

            ncas[cnmt.GetTitleID()][{cnmt.GetType(), ContentRecordType::Meta}] = nca;

            for (const auto& rec : cnmt.GetContentRecords()) {
                const auto id_string = Common::HexToString(rec.nca_id, false);
                auto next_file = pfs->GetFile(fmt::format("{}.nca", id_string));

                if (next_file == nullptr) {
                    if (rec.type != ContentRecordType::DeltaFragment) {
                        LOG_WARNING(Service_FS,
                                    "NCA with ID {}.nca is listed in content metadata, but cannot "
                                    "be found in PFS. NSP appears to be corrupted.",
                                    id_string);
                    }

                    continue;
                }

                auto next_nca = std::make_shared<NCA>(std::move(next_file));

                if (next_nca->GetType() == NCAContentType::Program) {
                    program_status[next_nca->GetTitleId()] = next_nca->GetStatus();
                    program_ids.insert(next_nca->GetTitleId() & 0xFFFFFFFFFFFFF000);
                }

                if (next_nca->GetStatus() != Loader::ResultStatus::Success &&
                    next_nca->GetStatus() != Loader::ResultStatus::ErrorMissingBKTRBaseRomFS) {
                    continue;
                }

                // If the last 3 hexadecimal digits of the CNMT TitleID is 0x800 or is missing the
                // BKTRBaseRomFS, this is an update NCA. Otherwise, this is a base NCA.
                if ((cnmt.GetTitleID() & 0x800) != 0 ||
                    next_nca->GetStatus() == Loader::ResultStatus::ErrorMissingBKTRBaseRomFS) {
                    // If the last 3 hexadecimal digits of the NCA's TitleID is between 0x1 and
                    // 0x7FF, this is a multi-program update NCA. Otherwise, this is a regular
                    // update NCA.
                    if ((next_nca->GetTitleId() & 0x7FF) != 0 &&
                        (next_nca->GetTitleId() & 0x800) == 0) {
                        ncas[next_nca->GetTitleId()][{cnmt.GetType(), rec.type}] =
                            std::move(next_nca);
                    } else {
                        ncas[cnmt.GetTitleID()][{cnmt.GetType(), rec.type}] = std::move(next_nca);
                    }
                } else {
                    ncas[next_nca->GetTitleId()][{cnmt.GetType(), rec.type}] = std::move(next_nca);
                }
            }

            break;
        }
    }
}
} // namespace FileSys
