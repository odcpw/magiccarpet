#include "mc2_exporter_source_provenance.h"
#include "level_selection.h"

#include "engine/Basic.h"
#include "engine/CommandLineParser.h"
#include "engine/EventDispatcher.h"
#include "engine/EventsFunctions.h"
#include "engine/Level.h"
#include "engine/TextureMaps.h"
#include "engine/Terrain.h"
#include "engine/Type_WORD_D951C.h"
#include "engine/engine_support.h"
#include "portability/port_sdl_vga_mouse.h"
#include "sub_main.h"

#include <SDL.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace fs = std::filesystem;
using json = nlohmann::ordered_json;

void StopProgrammableIntervalTimer();

namespace {

constexpr std::size_t kTerrainCells = 65536;
constexpr std::size_t kEntitySlots = 1000;
constexpr std::size_t kAuthoredFeatures = 1200;
constexpr std::size_t kAuthoredLevelBytes = 0x6604;
constexpr std::size_t kTerrainBytes = 0x70000;
constexpr std::size_t kLegacyPlayerRecordBytes = 573;
constexpr std::size_t kTypeStr164AuxiliaryBytes = 89;
constexpr std::size_t kLevel2BanishmentStateBytes = 1;
constexpr std::size_t kLevel2PlayerRecordBytes =
    kLegacyPlayerRecordBytes + kTypeStr164AuxiliaryBytes + kLevel2BanishmentStateBytes;
constexpr std::size_t kFeatureStartBytes = 16;
constexpr std::size_t kFeatureAllocatorHeaderBytes = 32;
constexpr std::size_t kFeatureAllocatorRelationCount = 1000;
constexpr std::size_t kFeatureAllocatorTraceRecordBytes = 8;
constexpr std::uint16_t kCollisionMaskHeaderBytes = 20;
constexpr std::uint32_t kCollisionMaskRecordHeaderBytes = 12;
constexpr std::uint32_t kFeatureStartGlobalRng = 0x0000BFCA;
constexpr std::array<std::uint32_t, 7> kFeatureGlobalRngBarriers = {
    0x1B712AE9,
    0x2C34E568,
    0x3D870B47,
    0xAF843886,
    0xFA1A8925,
    0x05F99924,
    0xDB828483,
};
// GenerateLevelMap leaves 9775 at the isolated input seam. The load-feature
// topology/house passes then advance the terrain RNG to this post-feature value.
constexpr std::uint16_t kFeatureStartTerrainRng = 9775;
constexpr std::uint16_t kFeatureEndTerrainRng = 44470;
constexpr const char* kExpectedTerrainSha256 =
    "05d4d12788d3754577b800883f2e4dac32ed722628a303eb5cd7577cb09c967c";
constexpr const char* kExpectedTerrainTypeSha256 =
    "59a9d0c19d12fc068bbd5e8364a66db5519040ceacb2aa008de31de7c5538a73";
constexpr const char* kExpectedTerrainHeightSha256 =
    "50f3a45214f435ac3b8423254a69b7cc87b80a8039085c25a0fe2033eed1ecba";
constexpr const char* kExpectedTerrainShadeSha256 =
    "6e15d0bebdbbd89ab82eae6dbb1c3e8f1fdc000449aa35eb99a11a1d9bca139c";
constexpr const char* kExpectedTerrainAngleSha256 =
    "342e64a393dc8b35518d0e10753734cbc1d2006ce85dcff1562c8a34172ec02f";
constexpr const char* kExpectedTerrainSecondarySha256 =
    "944112ef6eb54e042889c9cc841ed1d7bdb129c38c1c61ce4c8b8e81589f466b";
constexpr const char* kExpectedTerrainEntitySha256 =
    "d4a35266bee1854dfb8b4fb6c9175549a12e0de2edafff667cc0872d4cb3e72f";
constexpr const char* kNormalizedInactivePlaneSha256 =
    "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31";

class Sha256 {
public:
    Sha256() { reset(); }

    void update(const std::uint8_t* data, std::size_t length) {
        for (std::size_t index = 0; index < length; ++index) {
            block_[block_size_++] = data[index];
            if (block_size_ == 64) {
                transform();
                bit_length_ += 512;
                block_size_ = 0;
            }
        }
    }

    void update(const std::string& value) {
        update(reinterpret_cast<const std::uint8_t*>(value.data()), value.size());
    }

    std::string finish() {
        std::size_t index = block_size_;
        block_[index++] = 0x80;
        if (index > 56) {
            while (index < 64) block_[index++] = 0;
            transform();
            index = 0;
        }
        while (index < 56) block_[index++] = 0;

        bit_length_ += static_cast<std::uint64_t>(block_size_) * 8;
        for (int shift = 7; shift >= 0; --shift) {
            block_[index++] = static_cast<std::uint8_t>(bit_length_ >> (shift * 8));
        }
        transform();

        std::ostringstream result;
        result << std::hex << std::setfill('0');
        for (std::uint32_t word : state_) result << std::setw(8) << word;
        reset();
        return result.str();
    }

private:
    static std::uint32_t rotate_right(std::uint32_t value, std::uint32_t count) {
        return (value >> count) | (value << (32 - count));
    }

    void reset() {
        block_size_ = 0;
        bit_length_ = 0;
        state_ = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    }

    void transform() {
        static constexpr std::array<std::uint32_t, 64> constants = {
            0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
            0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
            0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
            0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
            0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
            0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
            0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
            0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
        };

        std::array<std::uint32_t, 64> words{};
        for (std::size_t i = 0; i < 16; ++i) {
            words[i] = (static_cast<std::uint32_t>(block_[i * 4]) << 24) |
                       (static_cast<std::uint32_t>(block_[i * 4 + 1]) << 16) |
                       (static_cast<std::uint32_t>(block_[i * 4 + 2]) << 8) |
                       static_cast<std::uint32_t>(block_[i * 4 + 3]);
        }
        for (std::size_t i = 16; i < 64; ++i) {
            const std::uint32_t s0 = rotate_right(words[i - 15], 7) ^
                                     rotate_right(words[i - 15], 18) ^
                                     (words[i - 15] >> 3);
            const std::uint32_t s1 = rotate_right(words[i - 2], 17) ^
                                     rotate_right(words[i - 2], 19) ^
                                     (words[i - 2] >> 10);
            words[i] = words[i - 16] + s0 + words[i - 7] + s1;
        }

        auto working = state_;
        for (std::size_t i = 0; i < 64; ++i) {
            const std::uint32_t s1 = rotate_right(working[4], 6) ^
                                     rotate_right(working[4], 11) ^
                                     rotate_right(working[4], 25);
            const std::uint32_t choose = (working[4] & working[5]) ^
                                         (~working[4] & working[6]);
            const std::uint32_t temp1 = working[7] + s1 + choose + constants[i] + words[i];
            const std::uint32_t s0 = rotate_right(working[0], 2) ^
                                     rotate_right(working[0], 13) ^
                                     rotate_right(working[0], 22);
            const std::uint32_t majority = (working[0] & working[1]) ^
                                           (working[0] & working[2]) ^
                                           (working[1] & working[2]);
            const std::uint32_t temp2 = s0 + majority;
            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3] + temp1;
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp1 + temp2;
        }
        for (std::size_t i = 0; i < state_.size(); ++i) state_[i] += working[i];
    }

    std::array<std::uint8_t, 64> block_{};
    std::array<std::uint32_t, 8> state_{};
    std::size_t block_size_ = 0;
    std::uint64_t bit_length_ = 0;
};

std::string sha256(const std::vector<std::uint8_t>& bytes) {
    Sha256 hash;
    hash.update(bytes.data(), bytes.size());
    return hash.finish();
}

std::string sha256_pointer(const std::uint8_t* bytes, std::size_t size) {
    Sha256 hash;
    hash.update(bytes, size);
    return hash.finish();
}

std::string sha256_range(
    const std::vector<std::uint8_t>& bytes,
    std::size_t offset,
    std::size_t size) {
    if (offset > bytes.size() || size > bytes.size() - offset) {
        throw std::runtime_error("SHA-256 range is outside buffer");
    }
    Sha256 hash;
    hash.update(bytes.data() + offset, size);
    return hash.finish();
}

std::string sha256_file(const fs::path& path) {
    std::ifstream stream(path, std::ios::binary);
    if (!stream) throw std::runtime_error("cannot hash " + path.filename().string());
    Sha256 hash;
    std::array<std::uint8_t, 1024 * 1024> buffer{};
    while (stream) {
        stream.read(reinterpret_cast<char*>(buffer.data()), buffer.size());
        hash.update(buffer.data(), static_cast<std::size_t>(stream.gcount()));
    }
    return hash.finish();
}

class Bytes {
public:
    void u8(std::uint8_t value) { data.push_back(value); }
    void i8(std::int8_t value) { u8(static_cast<std::uint8_t>(value)); }
    void u16(std::uint16_t value) {
        u8(static_cast<std::uint8_t>(value));
        u8(static_cast<std::uint8_t>(value >> 8));
    }
    void i16(std::int16_t value) { u16(static_cast<std::uint16_t>(value)); }
    void u32(std::uint32_t value) {
        u16(static_cast<std::uint16_t>(value));
        u16(static_cast<std::uint16_t>(value >> 16));
    }
    void i32(std::int32_t value) { u32(static_cast<std::uint32_t>(value)); }
    void raw(const void* source, std::size_t size) {
        const auto* begin = static_cast<const std::uint8_t*>(source);
        data.insert(data.end(), begin, begin + size);
    }
    void magic(const char (&value)[5]) { raw(value, 4); }
    std::vector<std::uint8_t> data;
};

struct Arguments {
    fs::path owner_root;
    fs::path output;
    mc2_exporter::ExternalLevelSelection level;
};

Arguments parse_arguments(int argc, char** argv) {
    Arguments result;
    bool level_seen = false;
    for (int index = 1; index < argc; ++index) {
        const std::string argument = argv[index];
        if ((argument == "--owner-root" || argument == "--output" || argument == "--level") &&
            index + 1 >= argc) {
            throw std::runtime_error("missing value after " + argument);
        }
        if (argument == "--owner-root") result.owner_root = argv[++index];
        else if (argument == "--output") result.output = argv[++index];
        else if (argument == "--level") {
            if (level_seen) throw std::runtime_error("--level may be specified only once");
            level_seen = true;
            result.level = mc2_exporter::parse_external_level(argv[++index]);
        }
        else if (argument == "--help") {
            std::cout << "usage: remc2-level-exporter --owner-root PATH --output PRIVATE_PATH "
                         "[--level EXTERNAL_LEVEL]\n"
                         "  --level EXTERNAL_LEVEL  owner-data level 1..25 (default: 1)\n";
            std::exit(0);
        } else {
            throw std::runtime_error("unknown argument: " + argument);
        }
    }
    if (result.owner_root.empty() || result.output.empty()) {
        throw std::runtime_error("--owner-root and --output are required");
    }
    return result;
}

bool is_within(const fs::path& child, const fs::path& parent) {
    auto child_part = child.begin();
    for (auto parent_part = parent.begin(); parent_part != parent.end(); ++parent_part, ++child_part) {
        if (child_part == child.end() || *child_part != *parent_part) return false;
    }
    return true;
}

fs::path validated_output(const fs::path& requested) {
    const fs::path private_root = fs::canonical(MC2_PRIVATE_ROOT);
    const fs::path absolute = fs::absolute(requested).lexically_normal();
    if (absolute.filename().empty()) throw std::runtime_error("output must name a new package directory");
    if (!fs::is_directory(absolute.parent_path())) {
        throw std::runtime_error("output parent must already exist");
    }
    const fs::path canonical_parent = fs::canonical(absolute.parent_path());
    const fs::path candidate = canonical_parent / absolute.filename();
    if (!is_within(candidate, private_root) || candidate == private_root) {
        throw std::runtime_error("output must remain below research/private");
    }
    if (fs::exists(candidate)) throw std::runtime_error("output already exists");
    return candidate;
}

std::uint32_t entity_relation(const type_entity_0x6E8E* pointer) {
    if (pointer == nullptr) return 0;
    const auto address = reinterpret_cast<std::uintptr_t>(pointer);
    const auto base = reinterpret_cast<std::uintptr_t>(&D41A0_0.struct_0x6E8E[0]);
    const auto end = reinterpret_cast<std::uintptr_t>(&D41A0_0.struct_0x6E8E[kEntitySlots]);
    if (address < base || address >= end) return std::numeric_limits<std::uint32_t>::max();
    const auto delta = address - base;
    if (delta % sizeof(type_entity_0x6E8E) != 0) return std::numeric_limits<std::uint32_t>::max();
    return static_cast<std::uint32_t>(delta / sizeof(type_entity_0x6E8E)) + 1;
}

std::uint32_t authored_relation(const type_entity_0x30311* pointer) {
    if (pointer == nullptr) return 0;
    const auto address = reinterpret_cast<std::uintptr_t>(pointer);
    const auto base = reinterpret_cast<std::uintptr_t>(&D41A0_0.terrain_2FECE.entity_0x30311[0]);
    const auto end = reinterpret_cast<std::uintptr_t>(
        &D41A0_0.terrain_2FECE.entity_0x30311[kAuthoredFeatures]);
    if (address < base || address >= end) return std::numeric_limits<std::uint32_t>::max();
    const auto delta = address - base;
    if (delta % sizeof(type_entity_0x30311) != 0) return std::numeric_limits<std::uint32_t>::max();
    return static_cast<std::uint32_t>(delta / sizeof(type_entity_0x30311)) + 1;
}

std::uint32_t player_state_relation(const type_str_164* pointer) {
    if (pointer == nullptr) return 0;
    for (std::size_t index = 0; index < 8; ++index) {
        if (pointer == &D41A0_0.array_0x2BDE[index].dword_0x3E6_2BE4_12228) {
            return static_cast<std::uint32_t>(index) + 1;
        }
    }
    return std::numeric_limits<std::uint32_t>::max();
}

std::uint32_t parameter_relation(const type_str_160* pointer) {
    if (pointer == nullptr) return 0;
    const auto address = reinterpret_cast<std::uintptr_t>(pointer);
    const auto base = reinterpret_cast<std::uintptr_t>(&str_D7BD6[0]);
    const auto end = reinterpret_cast<std::uintptr_t>(&str_D7BD6[157]);
    if (address < base || address >= end) return std::numeric_limits<std::uint32_t>::max();
    const auto delta = address - base;
    if (delta % sizeof(type_str_160) != 0) return std::numeric_limits<std::uint32_t>::max();
    return static_cast<std::uint32_t>(delta / sizeof(type_str_160)) + 1;
}

Bytes capture_terrain() {
    Bytes result;
    result.raw(mapTerrainType_10B4E0, kTerrainCells);
    result.raw(mapHeightmap_11B4E0, kTerrainCells);
    result.raw(mapShading_12B4E0, kTerrainCells);
    result.raw(mapAngle_13B4E0, kTerrainCells);
    result.raw(x_BYTE_14B4E0_second_heightmap, kTerrainCells);
    for (std::size_t index = 0; index < kTerrainCells; ++index) {
        result.i16(mapEntityIndex_15B4E0[index]);
    }
    if (result.data.size() != kTerrainBytes) throw std::runtime_error("terrain serialization size mismatch");
    return result;
}

void normalize_non_cave_inactive_terrain_plane(Bytes& terrain) {
    if (terrain.data.size() != kTerrainBytes) {
        throw std::runtime_error("terrain normalization extent mismatch");
    }
    if (D41A0_0.terrain_2FECE.MapType == MapType_t::Cave) {
        throw std::runtime_error("inactive-plane normalization is forbidden for Cave maps");
    }
    // remc2's stock regression comparison deliberately omits this cave-only
    // plane on non-Cave maps. It is uninitialized host state at both capture
    // phases, so normalize the pre-feature and frame-zero files identically.
    std::fill(
        terrain.data.begin() + 0x40000,
        terrain.data.begin() + 0x50000,
        std::uint8_t{0});
}

Bytes capture_feature_boundary(const char (&magic)[5]) {
    Bytes result;
    result.magic(magic);
    result.u16(1);
    result.u16(kFeatureStartBytes);
    result.u16(rand2_17B4E0);
    result.u16(0);
    result.u32(D41A0_0.rand_0x8);
    if (result.data.size() != kFeatureStartBytes) {
        throw std::runtime_error("feature-boundary serialization size mismatch");
    }
    return result;
}

std::vector<std::uint16_t> building_selectors(
    const Type_Level_2FECE& authored) {
    std::set<std::uint16_t> selectors;
    for (std::size_t index = 1; index < kAuthoredFeatures; ++index) {
        const auto& feature = authored.entity_0x30311[index];
        if (feature.DisId == -1 &&
            feature.type_0x30311 == 10 &&
            feature.subtype_0x30311 == 45) {
            if (feature.par1_14 >= str_D93C0_bldgprmbuffer.size()) {
                throw std::runtime_error("authored house BLDGPRM selector is outside the source table");
            }
            selectors.insert(feature.par1_14);
        }
    }
    return {selectors.begin(), selectors.end()};
}

struct CollisionMaskCapture {
    Bytes bytes;
    json metadata;
};

CollisionMaskCapture capture_collision_masks(
    const fs::path& owner_root,
    const std::vector<std::uint16_t>& selectors) {
    if (x_WORD_180660_VGA_type_resolution != 1) {
        throw std::runtime_error("MSPRD collision masks require VGA selector 1");
    }
    if (selectors.empty()) {
        throw std::runtime_error("Level 2 has no authored model-45 collision selectors");
    }
    if (MSPRD00TAB_BEGIN_BUFFER == nullptr || MSPRD00TAB_END_BUFFER == nullptr ||
        MSPRD00DAT_BEGIN_BUFFER == nullptr) {
        throw std::runtime_error("MSPRD0-0 source buffers are unavailable");
    }
    const std::size_t table_count = static_cast<std::size_t>(
        MSPRD00TAB_END_BUFFER - MSPRD00TAB_BEGIN_BUFFER);
    const fs::path tab_path = owner_root / "CD_Files/DATA/MSPRD0-0.TAB";
    const fs::path dat_path = owner_root / "CD_Files/DATA/MSPRD0-0.DAT";
    if (!fs::is_regular_file(tab_path) || !fs::is_regular_file(dat_path)) {
        throw std::runtime_error("MSPRD0-0 owner source files are unavailable");
    }
    const std::size_t tab_bytes = fs::file_size(tab_path);
    const std::size_t dat_bytes = fs::file_size(dat_path);
    if (tab_bytes != table_count * sizeof(bitmap_pos_struct2_t)) {
        throw std::runtime_error("MSPRD0-0 loaded table disagrees with owner source extent");
    }

    CollisionMaskCapture result;
    result.bytes.magic("MCM1");
    result.bytes.u16(1);
    result.bytes.u16(kCollisionMaskHeaderBytes);
    result.bytes.u16(x_WORD_180660_VGA_type_resolution);
    result.bytes.u16(0);
    result.bytes.u32(static_cast<std::uint32_t>(selectors.size()));
    result.bytes.u32(kCollisionMaskRecordHeaderBytes);

    json selector_metadata = json::array();
    for (const std::uint16_t selector : selectors) {
        if (selector >= table_count) {
            throw std::runtime_error("authored MSPRD selector is outside MSPRD0-0.TAB");
        }
        const bitmap_pos_struct2_t& record = MSPRD00TAB_BEGIN_BUFFER[selector];
        if (record.width_4 == 0 || record.height_5 == 0) {
            throw std::runtime_error("authored MSPRD selector has an empty collision mask");
        }
        const std::uint64_t pixel_bytes =
            static_cast<std::uint64_t>(record.width_4) *
            static_cast<std::uint64_t>(record.height_5) * 2;
        const std::uint64_t data_end =
            static_cast<std::uint64_t>(record.data_0) + pixel_bytes;
        if (data_end > dat_bytes || pixel_bytes > std::numeric_limits<std::uint32_t>::max()) {
            throw std::runtime_error("authored MSPRD selector exceeds MSPRD0-0.DAT");
        }
        const auto* pixels = MSPRD00DAT_BEGIN_BUFFER + record.data_0;
        result.bytes.u16(selector);
        result.bytes.u16(record.width_4);
        result.bytes.u16(record.height_5);
        result.bytes.u16(0);
        result.bytes.u32(static_cast<std::uint32_t>(pixel_bytes));
        result.bytes.raw(pixels, static_cast<std::size_t>(pixel_bytes));
        selector_metadata.push_back({
            {"selector", selector},
            {"width", record.width_4},
            {"height", record.height_5},
            {"pixel_bytes", pixel_bytes},
            {"sha256", sha256_pointer(pixels, static_cast<std::size_t>(pixel_bytes))}
        });
    }

    result.metadata = {
        {"file", "collision-masks.bin"},
        {"magic", "MCM1"},
        {"version", 1},
        {"header_bytes", kCollisionMaskHeaderBytes},
        {"record_header_bytes", kCollisionMaskRecordHeaderBytes},
        {"bank", "MSPRD0-0"},
        {"vga_resolution_selector", x_WORD_180660_VGA_type_resolution},
        {"selector_source", "authored class-10/model-45 par1 selectors"},
        {"pixel_encoding", "signed i16 little-endian"},
        {"pixel_order", "row-major (y then x)"},
        {"transparent_value", -1},
        {"source_files", {
            {"tab", {
                {"logical_path", "CD_Files/DATA/MSPRD0-0.TAB"},
                {"bytes", fs::file_size(tab_path)},
                {"sha256", sha256_file(tab_path)}
            }},
            {"dat", {
                {"logical_path", "CD_Files/DATA/MSPRD0-0.DAT"},
                {"bytes", fs::file_size(dat_path)},
                {"sha256", sha256_file(dat_path)}
            }}
        }},
        {"selectors", selector_metadata}
    };
    return result;
}

Bytes capture_feature_construction_inputs(
    const Type_Level_2FECE& authored,
    std::vector<std::uint16_t>& selectors,
    bool level_one) {
    if (level_one && x_WORD_180660_VGA_type_resolution != 1) {
        throw std::runtime_error("isolated Level 1 feature seam requires VGA selector 1");
    }
    selectors = building_selectors(authored);
    if (level_one && selectors.empty()) {
        throw std::runtime_error("Level 1 has no source house construction selectors");
    }

    Bytes result;
    result.magic("MCI3");
    result.u16(1);
    result.u16(16);
    result.u16(static_cast<std::uint16_t>(selectors.size()));
    result.u16(1);
    result.u32(0);
    for (const std::uint16_t selector : selectors) {
        const auto& parameter = str_D93C0_bldgprmbuffer[selector];
        const auto& bitmap =
            (*filearray_2aa18c[filearrayindex_BUILD00DATTAB].posistruct)[selector];
        const std::uint32_t data_bytes =
            static_cast<std::uint32_t>(bitmap.width_4) *
            static_cast<std::uint32_t>(bitmap.height_5) * 2;
        if (bitmap.data == nullptr || bitmap.width_4 == 0 || bitmap.height_5 == 0) {
            throw std::runtime_error("authored BUILD selector has an empty source footprint");
        }
        result.u16(selector);
        result.u16(parameter.word_0);
        result.u8(parameter.byte_2);
        result.u8(parameter.byte_3);
        result.u8(bitmap.width_4);
        result.u8(bitmap.height_5);
        result.u32(data_bytes);
        result.raw(bitmap.data, data_bytes);
    }
    return result;
}

void append_entity_record(Bytes& output, const type_entity_0x6E8E& entity, std::size_t index) {
    output.u16(static_cast<std::uint16_t>(index));
    output.u8(entity.class_0x3F_63 == 0 ? 0 : 1);
    output.u8(0);
    const std::uint32_t next = entity_relation(entity.next_0);
    if (next == std::numeric_limits<std::uint32_t>::max()) {
        throw std::runtime_error("entity contains an invalid next pointer");
    }
    output.u32(next);
    output.i32(entity.maxLife_0x4);
    output.i32(entity.life_0x8);
    output.u32(entity.struct_byte_0xc_12_15.dword);
    output.i32(entity.dword_0x10_16);
    output.u16(entity.rand_0x14_20);
    output.u16(entity.oldMapEntity_0x16_22);
    output.u16(entity.nextEntity_0x18_24);
    output.u16(entity.id_0x1A_26);
    output.i16(entity.yaw_0x1C_28);
    output.i16(entity.pitch_0x1E_30);
    output.i16(entity.roll_0x20_32);
    output.i16(entity.fov_0x22_34);
    output.i16(entity.word_0x24_36);
    output.i16(entity.word_0x26_38);
    output.u16(entity.parentId_0x28_40);
    output.u16(entity.subSpellIndex_0x2A_42);
    output.i16(entity.word_0x2C_44);
    output.i16(entity.word_0x2E_46);
    output.u16(entity.word_0x30_48);
    output.u16(entity.word_0x32_50);
    output.u16(entity.word_0x34_52);
    output.u16(entity.word_0x36_54);
    output.i8(entity.byte_0x38_56);
    output.i8(entity.byte_0x39_57);
    output.i8(entity.byte_0x3A_58);
    output.i8(entity.byte_0x3B_59);
    output.i8(entity.byte_0x3C_60);
    output.i8(entity.fontTypeIndex_0x3D_61);
    output.u8(entity.byte_0x3E_62);
    output.u8(entity.class_0x3F_63);
    output.u8(entity.model_0x40_64);
    output.i8(entity.xtype_0x41_65);
    output.i8(entity.xsubtype_0x42_66);
    output.i8(entity.byte_0x43_67);
    output.i8(entity.byte_0x44_68);
    output.u8(entity.actionIndex_0x45_69);
    output.i8(entity.byte_0x46_70);
    output.i8(entity.byte_0x47_71_xx);
    output.i8(entity.StageVar1_0x48_72);
    output.i8(entity.StageVar2_0x49_73);
    output.i16(entity.word_0x4A_74);
    output.i16(entity.position_0x4C_76.x);
    output.i16(entity.position_0x4C_76.y);
    output.i16(entity.position_0x4C_76.z);
    output.i16(entity.array_0x52_82.yaw);
    output.i16(entity.array_0x52_82.pitch);
    output.i16(entity.array_0x52_82.roll);
    output.i16(entity.array_0x52_82.fov);
    output.i16(entity.word_0x5A_90);
    output.i8(entity.animationFrame_0x5C_92);
    output.i8(entity.byte_0x5D_93);
    output.i32(entity.str_0x5E_94.dword_0x5E_94);
    output.i16(entity.str_0x5E_94.word_0x62_98);
    output.i32(entity.str_0x5E_94.dword_0x64_100);
    output.i16(entity.str_0x5E_94.word_0x68_104);
    output.raw(entity.str_0x5E_94.stub5b, sizeof(entity.str_0x5E_94.stub5b));
    output.i32(entity.str_0x5E_94.dword_0x70_112);
    output.i16(entity.str_0x5E_94.word_0x74_116);
    output.i16(entity.str_0x5E_94.word_0x76_118);
    output.i16(entity.str_0x5E_94.word_0x78_120);
    output.i16(entity.str_0x5E_94.word_0x7A_122);
    output.i16(entity.str_0x5E_94.word_0x7C_124);
    output.raw(entity.str_0x5E_94.stub5d, sizeof(entity.str_0x5E_94.stub5d));
    output.u16(entity.str_0x5E_94.word_0x80_128);
    output.i16(entity.actSpeed_0x82_130);
    output.i16(entity.minSpeed_0x84_132);
    output.i16(entity.maxSpeed_0x86_134);
    output.i32(entity.manaRegen_0x88_136);
    output.i32(entity.maxMana_0x8C_140);
    output.i32(entity.mana_0x90_144);
    output.u16(entity.playerEntityIndex_0x94_148);
    output.i16(entity.word_0x96_150);
    output.u16(entity.word_0x98_152);
    output.i16(entity.axis_0x9A_154x.x);
    output.i16(entity.axis_0x9A_154x.y);
    output.i16(entity.axis_0x9A_154x.z);
    const std::uint32_t parameters = parameter_relation(entity.dword_0xA0_160x);
    if (parameters == std::numeric_limits<std::uint32_t>::max()) {
        throw std::runtime_error("entity contains an unknown parameter-table pointer");
    }
    output.u32(parameters);
    const std::uint32_t player = player_state_relation(entity.dword_0xA4_164x);
    if (entity.dword_0xA4_164x == nullptr) {
        output.u8(0);
        output.u8(0);
        output.u16(0);
        output.u32(0);
    } else if (player != std::numeric_limits<std::uint32_t>::max()) {
        output.u8(1);
        output.u8(0);
        output.u16(0);
        output.u32(player);
    } else if (entity.dword_0xA4_164x == unk_F42B0x) {
        output.u8(2);
        output.u8(0);
        output.u16(0);
        output.u32(1);
    } else {
        throw std::runtime_error("entity contains an unknown special-state pointer");
    }
}

Bytes capture_entities(std::uint32_t& active_count, std::uint16_t& record_size) {
    Bytes records;
    active_count = 0;
    std::size_t first_size = 0;
    for (std::size_t index = 0; index < kEntitySlots; ++index) {
        const std::size_t before = records.data.size();
        append_entity_record(records, D41A0_0.struct_0x6E8E[index], index);
        const std::size_t size = records.data.size() - before;
        if (index == 0) first_size = size;
        else if (size != first_size) throw std::runtime_error("entity record size changed");
        if (D41A0_0.struct_0x6E8E[index].class_0x3F_63 != 0) ++active_count;
    }
    if (first_size > std::numeric_limits<std::uint16_t>::max()) {
        throw std::runtime_error("entity record too large");
    }
    record_size = static_cast<std::uint16_t>(first_size);
    Bytes result;
    result.magic("MCE1");
    result.u16(1);
    result.u16(record_size);
    result.u32(kEntitySlots);
    result.raw(records.data.data(), records.data.size());
    return result;
}

Bytes capture_normalized_feature_entities(
    std::uint32_t& active_count,
    std::uint16_t& record_size) {
    Bytes records;
    active_count = 0;
    std::size_t first_size = 0;
    const std::array<std::uint8_t, 172> zero_payload{};
    for (std::size_t index = 0; index < kEntitySlots; ++index) {
        const auto& entity = D41A0_0.struct_0x6E8E[index];
        const std::size_t before = records.data.size();
        if (entity.class_0x3F_63 == 0) {
            records.u16(static_cast<std::uint16_t>(index));
            records.u8(0);
            records.u8(0);
            records.raw(zero_payload.data(), zero_payload.size());
        } else {
            append_entity_record(records, entity, index);
            ++active_count;
        }
        const std::size_t size = records.data.size() - before;
        if (index == 0) first_size = size;
        else if (size != first_size) {
            throw std::runtime_error("normalized feature entity record size changed");
        }
    }
    record_size = static_cast<std::uint16_t>(first_size);
    Bytes result;
    result.magic("MCE3");
    result.u16(1);
    result.u16(record_size);
    result.u32(kEntitySlots);
    result.raw(records.data.data(), records.data.size());
    return result;
}

struct FeatureAllocatorTrace {
    std::uint8_t operation;
    std::uint8_t role;
    std::uint16_t slot;
    std::uint16_t authored_index;
};

std::vector<FeatureAllocatorTrace> expected_feature_allocator_trace(
    const Type_Level_2FECE& authored) {
    std::vector<FeatureAllocatorTrace> trace = {
        {1, 1, 1, 62},
        {1, 1, 2, 63},
        {2, 1, 1, 62},
        {2, 1, 2, 63},
    };
    std::uint16_t next_ordinary_slot = 1;
    bool saw_special = false;
    for (int pass = 0; pass < 2; ++pass) {
        for (std::size_t index = 1; index < kAuthoredFeatures; ++index) {
            const auto& feature = authored.entity_0x30311[index];
            if (feature.DisId != -1 ||
                feature.type_0x30311 != 10 ||
                feature.subtype_0x30311 != 45) {
                continue;
            }
            const bool special =
                (str_D93C0_bldgprmbuffer[feature.par1_14].byte_2 & 0x10) != 0;
            if (special != (pass == 0)) continue;
            std::uint16_t slot = 0;
            if (special) {
                if (saw_special) {
                    throw std::runtime_error("Level 1 has more than one special-pass house");
                }
                saw_special = true;
                slot = 2;
            } else if (next_ordinary_slot == 1) {
                slot = next_ordinary_slot++;
            } else {
                slot = ++next_ordinary_slot;
            }
            trace.push_back({
                1,
                static_cast<std::uint8_t>(special ? 2 : 3),
                slot,
                static_cast<std::uint16_t>(index),
            });
        }
    }
    if (!saw_special || trace.size() != 51) {
        throw std::runtime_error(
            "Level 1 allocator trace is not 2 path pops, 2 frees, and 47 houses: special=" +
            std::to_string(saw_special) + " entries=" + std::to_string(trace.size()));
    }
    return trace;
}

Bytes capture_feature_allocator(const Type_Level_2FECE& authored, bool level_one) {
    if (level_one && (D41A0_0.dword_0x35 != 951 || D41A0_0.dword_0x11e6 != -1)) {
        throw std::runtime_error("isolated feature allocator counters differ from the pinned source seam");
    }
    if (D41A0_0.dword_0x35 < 0 || D41A0_0.dword_0x35 >= static_cast<int>(kFeatureAllocatorRelationCount)) {
        throw std::runtime_error("feature allocator free-stack count is outside the exported relation table");
    }
    for (std::size_t index = 0; index <= static_cast<std::size_t>(D41A0_0.dword_0x35); ++index) {
        const std::uint32_t relation = entity_relation(D41A0_0.pointers_0x246[index]);
        const std::uint32_t expected = 999 - static_cast<std::uint32_t>(index);
        if (level_one && relation != expected + 1) {
            throw std::runtime_error("isolated feature free-stack relation order changed");
        }
    }
    const auto trace = level_one
        ? expected_feature_allocator_trace(authored)
        : std::vector<FeatureAllocatorTrace>{};
    Bytes result;
    result.magic("MCA3");
    result.u16(1);
    result.u16(kFeatureAllocatorHeaderBytes);
    result.i32(D41A0_0.dword_0x35);
    result.i32(D41A0_0.dword_0x11e6);
    result.u32(static_cast<std::uint32_t>(D41A0_0.dword_0x35 + 1));
    result.u32(0);
    result.u32(static_cast<std::uint32_t>(trace.size()));
    result.u16(2);
    result.u16(kFeatureAllocatorTraceRecordBytes);
    for (std::size_t index = 0; index < kFeatureAllocatorRelationCount; ++index) {
        if (index > static_cast<std::size_t>(D41A0_0.dword_0x35)) {
            result.u16(0);
            continue;
        }
        const std::uint32_t relation = entity_relation(D41A0_0.pointers_0x246[index]);
        if (relation == std::numeric_limits<std::uint32_t>::max() || relation == 0) {
            throw std::runtime_error("feature allocator free-stack contains an invalid entity relation");
        }
        result.u16(static_cast<std::uint16_t>(relation - 1));
    }
    for (std::size_t index = 0; index < kFeatureAllocatorRelationCount; ++index) {
        result.u16(0);
    }
    for (const auto& entry : trace) {
        result.u8(entry.operation);
        result.u8(entry.role);
        result.u16(entry.slot);
        result.u16(entry.authored_index);
        result.u16(0);
    }
    return result;
}

void validate_feature_house_fixture(const Type_Level_2FECE& authored) {
    if (D41A0_0.rand_0x8 != kFeatureGlobalRngBarriers.back()) {
        throw std::runtime_error("isolated feature final global RNG differs from barrier seven");
    }
    if (rand2_17B4E0 != kFeatureEndTerrainRng) {
        throw std::runtime_error(
            "isolated feature final terrain RNG differs from the pinned source seam: " +
            std::to_string(rand2_17B4E0));
    }

    const auto trace = expected_feature_allocator_trace(authored);
    std::array<bool, 1000> active{};
    std::size_t active_count = 0;
    for (std::size_t slot = 0; slot < kEntitySlots; ++slot) {
        active[slot] = D41A0_0.struct_0x6E8E[slot].class_0x3F_63 != 0;
        if (active[slot]) ++active_count;
    }
    if (active_count != 47 || active[0]) {
        throw std::runtime_error("isolated feature arena is not exactly 47 nonzero source slots");
    }
    for (std::size_t slot = 1; slot <= 47; ++slot) {
        if (!active[slot]) {
            throw std::runtime_error("isolated feature active slots are not exactly 1..47");
        }
    }
    for (std::size_t slot = 48; slot < kEntitySlots; ++slot) {
        if (active[slot]) {
            throw std::runtime_error("isolated feature arena has an active slot above 47");
        }
    }

    for (const auto& entry : trace) {
        if (entry.role < 2) continue;
        const auto& source = authored.entity_0x30311[entry.authored_index];
        const auto& house = D41A0_0.struct_0x6E8E[entry.slot];
        if (house.class_0x3F_63 != 10 ||
            house.model_0x40_64 != 45 ||
            house.actionIndex_0x45_69 != 52 ||
            house.byte_0x46_70 != source.par1_14 ||
            house.xtype_0x41_65 != 0 ||
            source.par2_16 != 0) {
            throw std::runtime_error("isolated feature house identity/selector/xtype changed");
        }
        const std::uint8_t expected_phase =
            entry.slot == 2
                ? 64
                : static_cast<std::uint8_t>(entry.slot + 31);
        const std::uint16_t allocation_rng =
            entry.slot == 2
                ? static_cast<std::uint16_t>(kFeatureGlobalRngBarriers[4] + entry.slot)
                : static_cast<std::uint16_t>(kFeatureGlobalRngBarriers[5] + entry.slot);
        if (house.byte_0x3E_62 != expected_phase ||
            house.rand_0x14_20 != allocation_rng ||
            house.life_0x8 !=
                1000 * str_D93C0_bldgprmbuffer[source.par1_14].word_0) {
            throw std::runtime_error(
                "isolated feature house phase/RNG/life changed at slot " +
                std::to_string(entry.slot) +
                ": phase=" + std::to_string(house.byte_0x3E_62) +
                "/" + std::to_string(expected_phase) +
                " rng=" + std::to_string(house.rand_0x14_20) +
                "/" + std::to_string(allocation_rng) +
                " life=" + std::to_string(house.life_0x8) +
                "/" + std::to_string(
                    1000 * str_D93C0_bldgprmbuffer[source.par1_14].word_0));
        }
    }

    std::array<bool, 1000> linked{};
    for (std::size_t cell = 0; cell < kTerrainCells; ++cell) {
        std::uint16_t slot = static_cast<std::uint16_t>(mapEntityIndex_15B4E0[cell]);
        std::uint16_t newer = 0;
        while (slot != 0) {
            if (slot > 47 || !active[slot] || linked[slot]) {
                throw std::runtime_error("isolated feature map head/chain is invalid or duplicated");
            }
            const auto& entity = D41A0_0.struct_0x6E8E[slot];
            if (entity.nextEntity_0x18_24 != newer) {
                throw std::runtime_error("isolated feature next-map relation changed");
            }
            linked[slot] = true;
            newer = slot;
            slot = entity.oldMapEntity_0x16_22;
        }
    }
    for (std::size_t slot = 1; slot <= 47; ++slot) {
        if (!linked[slot]) {
            throw std::runtime_error("isolated feature house is absent from map heads/chains");
        }
    }
}

void append_spell_bytes(Bytes& output, const type_8_1a& values) {
    for (std::uint8_t value : values.SpellIndex) output.u8(value);
}

void append_spell_words(Bytes& output, const type_16_1a& values) {
    for (std::int16_t value : values.SpellEnabled) output.i16(value);
}

void append_spell_dwords(Bytes& output, const type_32_1a& values) {
    for (std::int32_t value : values.SpellExperience) output.i32(value);
}

Bytes capture_players(bool include_level_two_carriers, std::uint16_t& record_size) {
    Bytes records;
    std::size_t first_size = 0;
    for (std::size_t index = 0; index < 8; ++index) {
        const auto& player = D41A0_0.array_0x2BDE[index];
        const auto& state = player.dword_0x3E6_2BE4_12228;
        const auto& spells = state.str_611;
        const std::size_t before = records.data.size();
        records.u16(static_cast<std::uint16_t>(index));
        records.i8(player.IsAiPlayer_0x009_2BE4_11239);
        records.u8(0);
        records.u16(player.playerIndex_0x00a_2BE4_11240);
        records.i16(player.ActPlayerIndex_0x00e_2BDE_11244);
        records.i32(player.Turn_2BE0_11248);
        records.i32(state.entityIndex_0x0);
        records.i16(state.rollDelta_0x4_4);
        records.i16(state.pitchDelta_0x6_6);
        records.i16(state.speed_0xc_12);
        records.i16(state.strafeSpeed_0x10_16);
        records.i16(state.position_backup_20.x);
        records.i16(state.position_backup_20.y);
        records.i16(state.nextEntity_0x18_24);
        records.i16(state.entityIndex2_0x1A_26);
        records.i16(state.moveBoost_0x1E_30);
        records.i16(state.yaw_0x1E_30);
        records.i16(state.fov_0x22_34);
        records.i16(state.pitch_0x24_36);
        records.i16(state.playerColorIndex_0x38_56);
        records.i16(state.CastleEntityIndex_0x3A_58);
        records.u8(state.moveSpeed_0x14C_332);
        records.i8(state.moveSpeedCounter_0x14D_333);
        records.i16(state.roll_0x155_341);
        records.i16(state.pitch_0x157_343);
        // This is an SDL/host clock sample, not simulation state. Consecutive
        // frame-zero captures differ here by elapsed initialization time.
        records.i32(0);
        records.i16(state.xAdd_0x1A6_422);
        records.i16(state.yAdd_0x1A8_424);
        records.i16(state.zAdd_0x1AA_426);
        append_spell_dwords(records, spells.SpellExperience_0x263_611x);
        for (std::int32_t value : spells.spellsExperience_0x2CB_715x) records.i32(value);
        append_spell_words(records, spells.SpellsEnabled_0x333_819x);
        append_spell_words(records, spells.array_0x367_871x);
        append_spell_bytes(records, spells.SpellIndexes_0x39B_923x);
        append_spell_bytes(records, spells.array_0x3B5_949x);
        append_spell_bytes(records, spells.array_0x3CF_975x);
        append_spell_bytes(records, spells.array_0x3E9_1001x);
        append_spell_bytes(records, spells.array_0x403_1027x);
        append_spell_bytes(records, spells.SpellLevels_0x41D_1053z);
        append_spell_bytes(records, spells.array_0x437_1079x);
        records.i16(spells.SpellIndexLeft_0x451_1105);
        records.i16(spells.SpellIndexRight_0x453_1107);
        records.i8(spells.SubSpellIndexLeft_1109);
        records.i8(spells.SubSpellIndexRight_1110);
        records.i8(spells.byte_0x457_1111);
        records.i8(spells.spellIndex_0x458_1112);
        records.i8(spells.subSpellIndex_0x459_1113);
        records.i8(spells.byte_0x45A_1114);
        records.i8(spells.byte_0x45B_1115);
        // v2 appends these fields after the complete 565-byte v1 record.
        // They are the two transient ambient-distance accumulators consumed
        // by source loops 5 and 31 and projected by the Rust FlightState.
        records.i32(state.dword_0x19A_410);
        records.i32(state.maxDistance_0x19E_414);
        if (include_level_two_carriers) {
            // This is a packed, pointer-free carrier for the exact Type_str_164
            // fields consumed by the Nyphur/common-upkeep and terminal paths.
            // Keep the source widths and order explicit; do not synthesize
            // alignment, policy, or values absent from the live remc2 state.
            for (std::int16_t value : state.word_0x26_38) records.i16(value);
            records.u8(state.byte_0x154_340);
            records.i32(state.dword_0x16D_365);
            records.i16(state.word_0x1A2_418);
            for (std::uint16_t value : state.array_0x1FC_508) records.u16(value);
            records.i16(state.word_0x242_578);

            // This is not part of the compact player-state prefix: retain the
            // exact source byte separately so type-3 objectives cannot alias
            // ActPlayerIndex at compact record bytes 6..8.
            records.u8(player.byte_0x006_2BE4_11236);
        }
        const std::size_t size = records.data.size() - before;
        if (index == 0) first_size = size;
        else if (size != first_size) throw std::runtime_error("player record size changed");
    }
    record_size = static_cast<std::uint16_t>(first_size);
    Bytes result;
    result.magic("MCP1");
    result.u16(include_level_two_carriers ? 4 : 2);
    result.u16(record_size);
    result.u32(8);
    result.raw(records.data.data(), records.data.size());
    return result;
}

Bytes capture_runtime(json& layout) {
    Bytes result;
    result.magic("MCR1");
    result.u16(1);
    result.u16(0);
    result.u32(D41A0_0.dword_0x4);
    result.u32(D41A0_0.rand_0x8);
    result.i16(D41A0_0.LevelIndex_0xc);
    result.i16(D41A0_0.NumberOfPlayers_0xe);
    result.i32(D41A0_0.dword_0x2d);
    result.u16(D41A0_0.word_0x31);
    result.u16(D41A0_0.word_0x33);
    result.i32(D41A0_0.dword_0x35);
    result.i32(D41A0_0.maptypeMusic_0x235);
    result.u8(D41A0_0.byte_0x239);
    result.i32(D41A0_0.dword_0x23a);
    result.i32(D41A0_0.dword_0x23e);
    result.i32(D41A0_0.dword_0x242);
    result.u8(D41A0_0.m_GameSettings.m_Graphics.m_wReflections);
    result.u8(D41A0_0.m_GameSettings.m_Graphics.m_wShadows);
    result.u8(D41A0_0.m_GameSettings.m_Graphics.m_wSky);
    result.u8(D41A0_0.m_GameSettings.m_Graphics.m_wViewPortSize);
    result.u8(D41A0_0.m_GameSettings.m_Display.m_wMiniMap);
    result.u8(D41A0_0.m_GameSettings.m_Display.m_wTopBar);
    result.u8(D41A0_0.m_GameSettings.m_Display.m_uiScreenSize);
    result.u8(D41A0_0.m_GameSettings.m_Display.xxxx_0x2191);
    result.u8(D41A0_0.m_GameSettings.str_0x2192.xxxx_0x2192);
    result.u8(D41A0_0.m_GameSettings.str_0x2192.xxxx_0x2193);
    result.u8(D41A0_0.m_GameSettings.str_0x2192.m_wResolution);
    result.u8(D41A0_0.m_GameSettings.str_0x2192.shadows_0x2195);
    result.u8(D41A0_0.m_GameSettings.str_0x2196.m_wDynamicLighting);
    result.u8(D41A0_0.m_GameSettings.str_0x2196.setting_0x2197);
    result.u8(D41A0_0.m_GameSettings.str_0x2196.transparency_0x2198);
    result.u8(D41A0_0.m_GameSettings.str_0x2196.flat_0x2199);

    result.u8(D41A0_0.stageIndex_0x36E01);
    result.u8(D41A0_0.countStageVars_0x36E00);
    result.i8(D41A0_0.byte_0x36E02);
    result.i8(D41A0_0.byte_0x36E03);
    result.u8(D41A0_0.byte_counter_current_objective_box_0x36E04);

    result.u32(D41A0_0.str_0x364D2.dword_0x364D2);
    for (std::uint32_t count : D41A0_0.str_0x364D2.dword_0x364D6) result.u32(count);
    result.i16(D41A0_0.word_0x3653E);
    result.i16(D41A0_0.word_0x36540);
    result.i16(D41A0_0.word_0x36542);
    result.i16(D41A0_0.word_0x36544);
    result.i16(D41A0_0.word_0x36546);
    result.i16(D41A0_0.word_0x36548);
    result.i16(D41A0_0.word_0x3654A);

    const std::size_t stages_offset = result.data.size();
    for (const auto& stage : D41A0_0.stages_0x3654C) {
        result.i8(stage.stages_3654C_byte0);
        result.i8(stage.str_3654D_byte1);
        result.i16(stage.str_3654E_axis.x);
        result.i16(stage.str_3654E_axis.y);
        const int type = stage.stages_3654C_byte0;
        if (type == 1 || type == 2 || type == 4) {
            const bool live = (stage.str_3654D_byte1 & 1) != 0;
            const std::uint32_t relation = live
                ? entity_relation(stage.str_36552_un.ptr0x6E8E)
                : authored_relation(stage.str_36552_un.ptr0x30311);
            if (relation == std::numeric_limits<std::uint32_t>::max()) {
                throw std::runtime_error("objective stage contains an invalid pointer relation");
            }
            result.u8(live ? 2 : 1);
            result.u8(0);
            result.u16(0);
            result.u32(relation);
        } else if (type == 3 && (stage.str_3654D_byte1 & 1) != 0) {
            const std::uint32_t relation =
                entity_relation(stage.str_36552_un.ptr0x6E8E);
            if (relation == std::numeric_limits<std::uint32_t>::max()) {
                throw std::runtime_error("type-3 objective stage contains an invalid live entity relation");
            }
            result.u8(2);
            result.u8(0);
            result.u16(0);
            result.u32(relation);
        } else if (type == 6) {
            const bool resolved_entity_index = (stage.str_3654D_byte1 & 1) != 0;
            if (resolved_entity_index) {
                const std::uint32_t index = stage.str_36552_un.dword;
                if (index >= kEntitySlots) {
                    throw std::runtime_error("type-6 objective stage contains an invalid entity index");
                }
                result.u8(3);
                result.u8(0);
                result.u16(0);
                result.u32(index);
            } else {
                const std::uint32_t relation =
                    authored_relation(stage.str_36552_un.ptr0x30311);
                if (relation == std::numeric_limits<std::uint32_t>::max()) {
                    throw std::runtime_error("type-6 objective stage contains an invalid authored relation");
                }
                result.u8(1);
                result.u8(0);
                result.u16(0);
                result.u32(relation);
            }
        } else {
            result.u8(0);
            result.u8(0);
            result.u16(0);
            result.u32(stage.str_36552_un.dword);
        }
    }
    const std::size_t stages_end = result.data.size();
    const std::size_t objectives_offset = result.data.size();
    for (const auto& objective : D41A0_0.struct_0x3659C) {
        result.u8(objective.substr_3659C.IsLevelEnd_0);
        result.u8(objective.substr_3659C.ObjectiveText_1);
        result.u8(objective.substr_3659C.ObjectiveDone_2);
        result.raw(objective.substr_3659C.stage_0x3659F,
                   sizeof(objective.substr_3659C.stage_0x3659F));
    }
    const std::size_t objectives_end = result.data.size();
    const std::size_t stage_vars_offset = result.data.size();
    for (const auto& stage_var : D41A0_0.StageVars2_0x365F4) {
        result.i8(stage_var.index_0x3647A_0);
        result.i8(stage_var.stage_0x3647A_1);
        result.u16(stage_var.str_0x3647A_2.word);
        const int type = stage_var.index_0x3647A_0;
        const bool pointer_payload =
            ((type >= 3 && type <= 5) || (type >= 8 && type <= 9)) &&
            (stage_var.stage_0x3647A_1 & 2) == 0;
        if (pointer_payload) {
            const std::uint32_t relation =
                entity_relation(stage_var.str_0x3647C_4.pointer_0x6E8E);
            if (relation == std::numeric_limits<std::uint32_t>::max()) {
                throw std::runtime_error("stage variable contains an invalid entity relation");
            }
            result.u8(1);
            result.u8(0);
            result.u16(0);
            result.u32(relation);
        } else {
            result.u8(0);
            result.u8(0);
            result.u16(0);
            result.u16(stage_var.str_0x3647C_4.axis.x);
            result.u16(stage_var.str_0x3647C_4.axis.y);
        }
    }
    const std::size_t stage_vars_end = result.data.size();
    const std::size_t events_offset = result.data.size();
    for (const auto& event : D41A0_0.str_0x3664C) {
        result.i8(event.byte_0);
        result.i8(event.byte_1);
        result.u8(event.byte_2);
        result.i8(event.byte_3);
        result.i16(event.axis3d_4.x);
        result.i16(event.axis3d_4.y);
        result.i16(event.axis3d_4.z);
        if (event.byte_0 != 0) {
            const std::uint32_t relation = entity_relation(event.event_A.pointer_0x6E8E);
            if (relation == std::numeric_limits<std::uint32_t>::max()) {
                throw std::runtime_error("runtime event contains an invalid entity relation");
            }
            result.u8(1);
            result.u8(0);
            result.u16(0);
            result.u32(relation);
        } else {
            result.u8(0);
            result.u8(0);
            result.u16(0);
            result.u32(0);
        }
        result.raw(event.array_E, sizeof(event.array_E));
    }
    const std::size_t events_end = result.data.size();
    const std::size_t player_inputs_offset = result.data.size();
    for (const auto& player_input : D41A0_0.playerInputs_0x6E3E) {
        result.i8(player_input.PlayerAction_byte0);
        result.i8(player_input.str_0x6E3E_byte1);
        result.i8(player_input.str_0x6E3E_byte2);
        result.i8(player_input.roll);
        result.i8(player_input.pitch);
        result.i8(player_input.entityIndex_0x6E3E_byte5);
        result.u16(player_input.nextEntity_0x6E3E_word6);
        result.u16(player_input.entityIndex2_0x6E3E_word8);
    }
    const std::size_t player_inputs_end = result.data.size();
    const std::size_t stage_count =
        sizeof(D41A0_0.stages_0x3654C) / sizeof(D41A0_0.stages_0x3654C[0]);
    const std::size_t objective_count =
        sizeof(D41A0_0.struct_0x3659C) / sizeof(D41A0_0.struct_0x3659C[0]);
    const std::size_t stage_variable_count =
        sizeof(D41A0_0.StageVars2_0x365F4) / sizeof(D41A0_0.StageVars2_0x365F4[0]);
    const std::size_t event_count =
        sizeof(D41A0_0.str_0x3664C) / sizeof(D41A0_0.str_0x3664C[0]);
    const std::size_t player_input_count =
        sizeof(D41A0_0.playerInputs_0x6E3E) / sizeof(D41A0_0.playerInputs_0x6E3E[0]);
    if (stages_end - stages_offset != stage_count * 14 ||
        stage_vars_end - stage_vars_offset != stage_variable_count * 12 ||
        stage_variable_count != 11 ||
        (objectives_end - objectives_offset) % objective_count != 0 ||
        (events_end - events_offset) % event_count != 0 ||
        (player_inputs_end - player_inputs_offset) % player_input_count != 0) {
        throw std::runtime_error("runtime section record layout is not fixed-width");
    }
    const auto section = [](std::size_t offset, std::size_t end, std::size_t count) {
        return json{
            {"offset", offset},
            {"count", count},
            {"record_bytes", count == 0 ? 0 : (end - offset) / count}
        };
    };
    layout = {
        {"scalar_prefix_bytes", stages_offset},
        {"stages", section(
            stages_offset,
            stages_end,
            stage_count)},
        {"objectives", section(
            objectives_offset,
            objectives_end,
            objective_count)},
        {"stage_variables", section(
            stage_vars_offset,
            stage_vars_end,
            stage_variable_count)},
        {"events", section(
            events_offset,
            events_end,
            event_count)},
        {"player_inputs", section(
            player_inputs_offset,
            player_inputs_end,
            player_input_count)},
        {"bytes", result.data.size()}
    };
    return result;
}

Bytes capture_presentation() {
    Bytes result;
    result.magic("MCV1");
    result.u16(1);
    result.u16(0);
    result.u32(347);
    for (const auto& particle : particlesParameters_D951C) {
        result.u16(particle.word_0);
        result.u16(particle.word_2);
        result.u16(particle.word_4);
        result.u16(particle.speed_6);
        result.u16(particle.rotSpeed_8);
        result.u8(particle.byte_10);
        result.i8(particle.byte_11);
        result.u8(particle.byte_12);
        result.u8(particle.byte_13);
    }
    result.u32(504);
    result.raw(m_LevelSpriteList_F5340, 504);
    result.u32(157);
    for (const auto& parameters : str_D7BD6) {
        result.i16(parameters.type_160_0x0_0);
        result.i16(parameters.subtype_160_0x2_2);
        result.i16(parameters.word_160_0x4_4);
        result.i16(parameters.word_160_0x6_6);
        result.i16(parameters.word_160_0x8_8);
        result.i16(parameters.word_160_0xa_10);
        result.i16(parameters.word_160_0xc_12);
        result.i16(parameters.word_160_0xe_14);
        result.i16(parameters.word_160_0x10_16);
        result.i16(parameters.word_160_0x12_18);
        result.i32(parameters.dword_160_0x14_20);
        result.raw(parameters.stuba, sizeof(parameters.stuba));
        result.i16(parameters.word_160_0x1a_26);
        result.i16(parameters.word_160_0x1c_28);
        result.i16(parameters.word_160_0x1e_30);
        result.i8(parameters.byte_160_0x20_32);
        result.raw(parameters.stubb, sizeof(parameters.stubb));
    }
    return result;
}

struct MapTypeProfile {
    std::uint8_t id;
    const char* name;
    char asset_code;
};

MapTypeProfile map_type_profile(MapType_t map_type) {
    switch (map_type) {
    case MapType_t::Day: return {0, "Day", 'D'};
    case MapType_t::Night: return {1, "Night", 'N'};
    case MapType_t::Cave: return {2, "Cave", 'C'};
    }
    throw std::runtime_error("remc2 returned an unknown map type");
}

Bytes serialize_authored_level(const Type_Level_2FECE& level) {
    Bytes result;
    result.u16(level.word_2FECE);
    result.u16(level.levelID_2FED0);
    result.u8(level.byte_0x2FED2);
    result.u8(level.byte_0x2FED3);
    result.u8(static_cast<std::uint8_t>(level.MapType));
    result.i16(level.word_0x2FED5);
    result.i16(level.word_0x2FED7);
    result.raw(level.player_0x2FED9, sizeof(level.player_0x2FED9));
    result.raw(level.stubb, sizeof(level.stubb));
    result.u16(level.seed_0x2FEE5);
    result.raw(level.stubb2, sizeof(level.stubb2));
    result.u16(level.offset_0x2FEE9);
    result.raw(level.stubc, sizeof(level.stubc));
    result.u16(level.raise_0x2FEED);
    result.raw(level.stubc2, sizeof(level.stubc2));
    result.u16(level.gnarl_0x2FEF1);
    result.raw(level.stubc3, sizeof(level.stubc3));
    result.u32(level.river_0x2FEF5);
    result.u16(level.lriver_0x2FEF9);
    result.raw(level.stube, sizeof(level.stube));
    result.u16(level.source_0x2FEFD);
    result.raw(level.stubf, sizeof(level.stubf));
    result.u16(level.snLin_0x2FF01);
    result.raw(level.stubg, sizeof(level.stubg));
    result.u16(level.snFlt_0x2FF05);
    result.raw(level.stubh, sizeof(level.stubh));
    result.u16(level.bhLin_0x2FF09);
    result.raw(level.stubi, sizeof(level.stubi));
    result.u16(level.bhFlt_0x2FF0D);
    result.raw(level.stubj, sizeof(level.stubj));
    result.u16(level.rkSte_0x2FF11);
    result.raw(level.stubk, sizeof(level.stubk));
    for (const auto& feature : level.entity_0x30311) {
        result.u16(feature.type_0x30311);
        result.i16(feature.subtype_0x30311);
        result.u16(feature.axis2d_4.x);
        result.u16(feature.axis2d_4.y);
        result.i16(feature.DisId);
        result.u16(feature.word_10);
        result.i16(feature.stageTag_12);
        result.u16(feature.par1_14);
        result.u16(feature.par2_16);
        result.u16(feature.par3_18);
    }
    result.u8(level.next_0x360D1);
    for (const auto& wizard : level.WizardMapSettings_0x360D2) {
        result.raw(wizard.stuba, sizeof(wizard.stuba));
        result.i16(wizard.Aggression_0x360D5);
        result.raw(wizard.stubb, sizeof(wizard.stubb));
        result.i16(wizard.Reflexes_0x360D9);
        result.raw(wizard.stubc, sizeof(wizard.stubc));
        result.i16(wizard.Perception_0x360DD);
        result.raw(wizard.stubd, sizeof(wizard.stubd));
        result.raw(wizard.StartingSpells_0x360E1x, sizeof(wizard.StartingSpells_0x360E1x));
        result.raw(wizard.byte_0x360FBx, sizeof(wizard.byte_0x360FBx));
        result.raw(wizard.BlockedSpells_0x36115x, sizeof(wizard.BlockedSpells_0x36115x));
        result.i16(wizard.Life_0x3612F);
        result.raw(wizard.stubf, sizeof(wizard.stubf));
    }
    for (const auto& stage : level.stages_0x36442) {
        result.i8(stage.index_0);
        result.i16(stage.stage_1);
        result.i16(stage._axis_2d.x);
        result.i16(stage._axis_2d.y);
    }
    for (const auto& stage_var : level.StageVars_0x3647A) {
        result.i8(stage_var.index_0x3647A_0);
        result.i8(stage_var.stage_0x3647A_1);
        result.u16(stage_var.str_0x3647A_2.word);
        result.u16(stage_var.str_0x3647C_4.axis.x);
        result.u16(stage_var.str_0x3647C_4.axis.y);
    }
    if (result.data.size() != kAuthoredLevelBytes) {
        throw std::runtime_error("authored level serialization size mismatch");
    }
    return result;
}

json make_owner_sources(
    const fs::path& owner_root,
    const MapTypeProfile& map_profile,
    bool include_collision_masks) {
    const std::string map_code(1, map_profile.asset_code);
    const std::string terrain_bank = "TMAPS" + std::to_string(map_profile.id) + "-0";
    const std::string medium_sprite = "MSPR" + map_code + "0-0";
    const std::string high_sprite = "HSPR" + map_code + "0-0";
    const std::string medium_web = "MWEB" + map_code + "0-0";
    const std::string high_web = "HWEB" + map_code + "0-0";
    const std::string palette = "PAL" + map_code + "-0";
    const std::string sky = "SKY" + map_code + "0-0";
    std::vector<std::string> candidates = {
        "GAME/NETHERW/CLEVELS/LEVELS.DAT",
        "GAME/NETHERW/CLEVELS/LEVELS.TAB",
        "GAME/NETHERW/CDATA/" + terrain_bank + ".DAT",
        "GAME/NETHERW/CDATA/" + terrain_bank + ".TAB",
        "CD_Files/DATA/" + terrain_bank + ".DAT",
        "CD_Files/DATA/" + terrain_bank + ".TAB",
        "CD_Files/DATA/" + medium_sprite + ".DAT",
        "CD_Files/DATA/" + medium_sprite + ".TAB",
        "CD_Files/DATA/" + high_sprite + ".DAT",
        "CD_Files/DATA/" + high_sprite + ".TAB",
        "CD_Files/DATA/" + palette + ".DAT",
        "CD_Files/DATA/PALF-0.DAT",
        "CD_Files/DATA/BL16N0-0.DAT",
        "CD_Files/DATA/BL16F0-0.DAT",
        "CD_Files/DATA/BL32N0-0.DAT",
        "CD_Files/DATA/BL32F0-0.DAT",
        "CD_Files/DATA/" + sky + ".DAT",
        "CD_Files/DATA/" + medium_web + ".DAT",
        "CD_Files/DATA/" + medium_web + ".TAB",
        "CD_Files/DATA/" + high_web + ".DAT",
        "CD_Files/DATA/" + high_web + ".TAB",
        "CD_Files/LEVELS/LEVELS.DAT",
        "CD_Files/LEVELS/LEVELS.TAB",
        "CD_Files/DATA/TABLESN.DAT",
        "CD_Files/DATA/BUILD0-0.DAT",
        "CD_Files/DATA/BUILD0-0.TAB",
        "CD_Files/DATA/SPELLS.DAT",
        "CD_Files/DATA/FONT1.DAT",
        "CD_Files/DATA/FONT1.TAB",
        "CD_Files/LANGUAGE/L1.TXT",
        "CD_Files/SOUND/SOUND.DAT",
        "memimages/regressions/sequence-002285FF-00356038.bin",
        "memimages/regressions/sequence-002285FF-002DC4E0.bin",
        "regression-config.json"
    };
    if (include_collision_masks) {
        candidates.push_back("CD_Files/DATA/MSPRD0-0.DAT");
        candidates.push_back("CD_Files/DATA/MSPRD0-0.TAB");
    }
    json result = json::array();
    for (const std::string& logical_name : candidates) {
        const fs::path path = owner_root / logical_name;
        if (!fs::is_regular_file(path)) continue;
        result.push_back({
            {"logical_path", logical_name},
            {"bytes", fs::file_size(path)},
            {"sha256", sha256_file(path)}
        });
    }
    return result;
}

void write_bytes(const fs::path& path, const std::vector<std::uint8_t>& bytes) {
    const fs::path temporary = path.string() + ".tmp";
    std::ofstream stream(temporary, std::ios::binary | std::ios::trunc);
    if (!stream) throw std::runtime_error("cannot create " + path.filename().string());
    stream.write(reinterpret_cast<const char*>(bytes.data()),
                 static_cast<std::streamsize>(bytes.size()));
    stream.close();
    if (!stream) throw std::runtime_error("cannot finish " + path.filename().string());
    fs::permissions(
        temporary,
        fs::perms::owner_read | fs::perms::owner_write,
        fs::perm_options::replace);
    fs::rename(temporary, path);
}

std::string resolved_owner_asset(
    const fs::path& owner_root,
    const std::string& preferred,
    const std::string& fallback) {
    if (fs::is_regular_file(owner_root / preferred)) return "owner:" + preferred;
    if (fs::is_regular_file(owner_root / fallback)) return "owner:" + fallback;
    return "missing:" + preferred + "|" + fallback;
}

void require_reference_file(
    const fs::path& path,
    std::uintmax_t expected_size,
    const std::string& expected_sha256) {
    if (!fs::is_regular_file(path)) {
        throw std::runtime_error("missing preserved reference " + path.filename().string());
    }
    if (fs::file_size(path) != expected_size) {
        throw std::runtime_error("preserved reference has wrong size: " + path.filename().string());
    }
    const std::string actual_sha256 = sha256_file(path);
    if (actual_sha256 != expected_sha256) {
        throw std::runtime_error(
            "preserved reference has wrong hash: " + path.filename().string() +
            " (actual " + actual_sha256 + ")");
    }
}

void write_text(const fs::path& path, const std::string& text) {
    write_bytes(path, std::vector<std::uint8_t>(text.begin(), text.end()));
}

json load_and_rewrite_config(const fs::path& owner_root) {
    std::ifstream stream(owner_root / "regression-config.json");
    if (!stream) throw std::runtime_error("owner root lacks regression-config.json");
    json config;
    stream >> config;
    auto& settings = config.at("settings").at(0);
    settings["paths"]["gameFolder"] = (owner_root / "GAME/NETHERW").string();
    settings["paths"]["cdFolder"] = (owner_root / "CD_Files").string();
    settings["sound"]["hqSound"] = false;
    settings["sound"]["oggMusic"] = false;
    settings["graphics"]["windowResWidth"] = 640;
    settings["graphics"]["windowResHeight"] = 480;
    settings["graphics"]["gameDetail"]["gameResWidth"] = 320;
    settings["graphics"]["gameDetail"]["gameResHeight"] = 200;
    settings["graphics"]["threading"]["isActive"] = false;
    return config;
}

struct EngineCleanup {
    bool support_started = false;
    bool manual_engine_cleanup = false;
    ~EngineCleanup() {
        StopProgrammableIntervalTimer();
        if (support_started && manual_engine_cleanup) {
            try { sub_5BC20(); } catch (...) {}
            try { sub_56730_clean_memory(); } catch (...) {}
            if (EventDispatcher::I != nullptr) {
                delete EventDispatcher::I;
                EventDispatcher::I = nullptr;
            }
            try { VGA_close(); } catch (...) {}
        }
        if (support_started) {
            support_end();
        }
    }
};

struct StagingCleanup {
    fs::path path;
    bool committed = false;
    ~StagingCleanup() {
        if (!committed && !path.empty()) {
            std::error_code error;
            fs::remove_all(path, error);
        }
    }
};

json type_str_164_auxiliary_layout() {
    return {
        {"schema", "mc2-type-str-164-auxiliary/v1"},
        {"record_offset", kLegacyPlayerRecordBytes},
        {"record_bytes", kTypeStr164AuxiliaryBytes},
        {"source_struct", "Type_str_164"},
        {"source_struct_bytes", 1136},
        {"descriptor_bounds", {
            {"source_min_inclusive", 38},
            {"source_max_exclusive", 580},
            {"record_min_inclusive", 0},
            {"record_max_exclusive", kTypeStr164AuxiliaryBytes}
        }},
        {"fields", json::array({
            json{
                {"name", "word_0x26_38"}, {"source_offset", 38},
                {"record_offset", 0}, {"count", 8}, {"element_bytes", 2},
                {"bytes", 16}, {"encoding", "i16_le"}
            },
            json{
                {"name", "byte_0x154_340"}, {"source_offset", 340},
                {"record_offset", 16}, {"count", 1}, {"element_bytes", 1},
                {"bytes", 1}, {"encoding", "u8"}
            },
            json{
                {"name", "dword_0x16D_365"}, {"source_offset", 365},
                {"record_offset", 17}, {"count", 1}, {"element_bytes", 4},
                {"bytes", 4}, {"encoding", "i32_le"}
            },
            json{
                {"name", "word_0x1A2_418"}, {"source_offset", 418},
                {"record_offset", 21}, {"count", 1}, {"element_bytes", 2},
                {"bytes", 2}, {"encoding", "i16_le"}
            },
            json{
                {"name", "array_0x1FC_508"}, {"source_offset", 508},
                {"record_offset", 23}, {"count", 32}, {"element_bytes", 2},
                {"bytes", 64}, {"encoding", "u16_le"}
            },
            json{
                {"name", "word_0x242_578"}, {"source_offset", 578},
                {"record_offset", 87}, {"count", 1}, {"element_bytes", 2},
                {"bytes", 2}, {"encoding", "i16_le"}
            }
        })}
    };
}

json type_str_164_auxiliary_reference() {
    return {
        {"schema", "mc2-type-str-164-auxiliary/v1"},
        {"source_struct", "Type_str_164"},
        {"descriptor_table", "engine_support.cpp:665-707"},
        {"initialization", {
            {"word_0x26_38", json::array({"global_types.h:236", "engine_support_converts.cpp:59-61"})},
            {"byte_0x154_340", json::array({"EventsFunctions.cpp:5424-5425"})},
            {"dword_0x16D_365", json::array({"EventsFunctions.cpp:43760"})},
            {"word_0x1A2_418", json::array({"EventsFunctions.cpp:6811-6815"})},
            {"array_0x1FC_508", json::array({"EventsFunctions.cpp:43886-43897"})},
            {"word_0x242_578", json::array({"EventsFunctions.cpp:43811"})}
        }},
        {"consumers", {
            {"word_0x26_38", json::array({"engine_support.cpp:665-666"})},
            {"byte_0x154_340", json::array({"EventsFunctions.cpp:55909-55910", "EventsFunctions.cpp:56193-56194"})},
            {"dword_0x16D_365", json::array({"EventsFunctions.cpp:5436-5437"})},
            {"word_0x1A2_418", json::array({"EventsFunctions.cpp:5358-5359", "EventsFunctions.cpp:5956", "EventsFunctions.cpp:6805-6818"})},
            {"array_0x1FC_508", json::array({"EventsFunctions.cpp:5377-5392", "EventsFunctions.cpp:5968", "EventsFunctions.cpp:6201-6206"})},
            {"word_0x242_578", json::array({"EventsFunctions.cpp:5379", "EventsFunctions.cpp:6201-6206", "EventsFunctions.cpp:6263-6266"})}
        }}
    };
}

json banishment_state_layout() {
    return {
        {"schema", "mc2-player-banishment-state/v1"},
        {"record_offset", kLegacyPlayerRecordBytes + kTypeStr164AuxiliaryBytes},
        {"record_bytes", kLevel2BanishmentStateBytes},
        {"source_struct", "type_str_0x2BDE"},
        {"source_struct_bytes", 2124},
        {"source_offset", 6},
        {"source_field", "byte_0x006_2BE4_11236"},
        {"encoding", "u8"}
    };
}

json banishment_state_reference() {
    return {
        {"schema", "mc2-player-banishment-state/v1"},
        {"source_struct", "type_str_0x2BDE"},
        {"source_field", "byte_0x006_2BE4_11236"},
        {"source_offset", 6},
        {"descriptor_table", "engine_support.cpp:601-613"},
        {"serialization", "engine_support_converts.cpp:239-249"},
        {"consumer", "EventsFunctions.cpp:40781-40785"}
    };
}

int run(const Arguments& arguments) {
    const bool level_one = arguments.level.external == 1;
    const fs::path owner_root = fs::canonical(arguments.owner_root);
    if (!fs::is_directory(owner_root / "GAME/NETHERW") ||
        !fs::is_directory(owner_root / "CD_Files") ||
        (level_one && !fs::is_directory(owner_root / "memimages/regressions"))) {
        throw std::runtime_error(
            level_one
                ? "owner root must contain GAME/NETHERW, CD_Files, and memimages/regressions"
                : "owner root must contain GAME/NETHERW and CD_Files");
    }
    if (level_one) {
        require_reference_file(
            owner_root / "memimages/regressions/sequence-002285FF-00356038.bin",
            4495800,
            "021ab6f582cae920546dc3bcf55b06d00937fea5ddaabfeae079c0368b314e7f");
        require_reference_file(
            owner_root / "memimages/regressions/sequence-002285FF-002DC4E0.bin",
            9633792,
            "16f0f778b120ddaab042f28b8a322efb37940bb68b4527b7d12dac82101db88f");
    }

    const fs::path output = validated_output(arguments.output);
    const fs::path staging = output.string() + ".staging";
    if (fs::exists(staging)) throw std::runtime_error("staging directory already exists");
    fs::create_directory(staging);
    fs::permissions(staging, fs::perms::owner_all, fs::perm_options::replace);
    StagingCleanup staging_cleanup{staging};

    const fs::path runtime_config = staging / "runtime-config.json";
    write_text(runtime_config, load_and_rewrite_config(owner_root).dump(2) + "\n");

    SDL_setenv("SDL_VIDEODRIVER", "dummy", 1);
    SDL_setenv("SDL_AUDIODRIVER", "dummy", 1);

    std::vector<std::string> engine_args = {
        "remc2-level-exporter",
        "--mode_test_regressions", "0",
        "--text_output_to_console",
        "--set_level", std::to_string(arguments.level.internal),
        "--config_file_path", runtime_config.string(),
        "--set_max_regressions_steps", "0"
    };
    std::vector<char*> engine_argv;
    for (auto& value : engine_args) engine_argv.push_back(value.data());
    char* environment[] = {nullptr};

    for (auto& entry : compstr) {
        entry.adress = 0;
        entry.index = 0;
    }
    unitTests = true;
    unitTestsPath = (owner_root / "memimages/regressions").string();
    int end_code = 0;
    endTestsCode = &end_code;
    CommandLineParams.Init(static_cast<int>(engine_argv.size()), engine_argv.data());

    EngineCleanup engine_cleanup;
    support_begin();
    engine_cleanup.support_started = true;
    x_BYTE_D4B80 = 0;
    CleanF5538_716A0();
    for (auto& sprite : str_DWORD_F66F0x) sprite = nullptr;
    x_DWORD_E9C28_str = nullptr;

    const int sub_main_result =
        sub_main(static_cast<int>(engine_argv.size()), engine_argv.data(), environment);

    // This join is deliberately the first operation after the engine returns.
    StopProgrammableIntervalTimer();
    engine_cleanup.manual_engine_cleanup = end_code == 20 || sub_main_result != 0;
    if (sub_main_result != 0 || end_code != 20) {
        throw std::runtime_error(
            "remc2 did not stop at the requested frame-zero regression seam for external level " +
            std::to_string(arguments.level.external));
    }

    const bool non_cave_map = D41A0_0.terrain_2FECE.MapType != MapType_t::Cave;
    Bytes terrain = capture_terrain();
    if (non_cave_map) normalize_non_cave_inactive_terrain_plane(terrain);
    const std::string terrain_type_sha =
        sha256_range(terrain.data, 0x00000, 0x10000);
    const std::string terrain_height_sha =
        sha256_range(terrain.data, 0x10000, 0x10000);
    const std::string terrain_shade_sha =
        sha256_range(terrain.data, 0x20000, 0x10000);
    const std::string terrain_angle_sha =
        sha256_range(terrain.data, 0x30000, 0x10000);
    const std::string terrain_secondary_sha =
        sha256_range(terrain.data, 0x40000, 0x10000);
    const std::string terrain_entity_sha =
        sha256_range(terrain.data, 0x50000, 0x20000);
    if (level_one && (terrain_type_sha != kExpectedTerrainTypeSha256 ||
        terrain_height_sha != kExpectedTerrainHeightSha256 ||
        terrain_shade_sha != kExpectedTerrainShadeSha256 ||
        terrain_angle_sha != kExpectedTerrainAngleSha256 ||
        terrain_entity_sha != kExpectedTerrainEntitySha256)) {
        throw std::runtime_error(
            "one or more stock-regression-compared terrain planes differ from the preserved frame-zero snapshot");
    }
    if (non_cave_map && terrain_secondary_sha != kNormalizedInactivePlaneSha256) {
        throw std::runtime_error("inactive secondary terrain plane was not normalized");
    }
    std::uint32_t active_entities = 0;
    std::uint16_t entity_record_size = 0;
    Bytes entities = capture_entities(active_entities, entity_record_size);
    if (level_one && active_entities != 177) {
        throw std::runtime_error(
            "live entity count does not match the preserved frame-zero snapshot (actual " +
            std::to_string(active_entities) + ")");
    }
    if (arguments.level.external == 2 && active_entities != 114) {
        throw std::runtime_error(
            "external Level 2 active entity count differs from the preserved Day reset census (actual " +
            std::to_string(active_entities) + ")");
    }
    if (level_one && (D41A0_0.LevelIndex_0xc != 0 || D41A0_0.NumberOfPlayers_0xe != 1)) {
        throw std::runtime_error("live LevelIndex/NumberOfPlayers do not match the preserved frame-zero snapshot");
    }
    if (arguments.level.external == 2 &&
        (D41A0_0.LevelIndex_0xc != 0 || D41A0_0.NumberOfPlayers_0xe != 2)) {
        throw std::runtime_error(
            "external Level 2 must start with local player 0 and exactly one AI opponent");
    }
    std::uint16_t player_record_size = 0;
    Bytes players = capture_players(!level_one, player_record_size);
    const auto expected_player_record_size =
        level_one ? kLegacyPlayerRecordBytes : kLevel2PlayerRecordBytes;
    if (player_record_size != expected_player_record_size) {
        throw std::runtime_error("player record size does not match the selected profile");
    }
    const auto local_player_slot = static_cast<std::size_t>(D41A0_0.LevelIndex_0xc);
    const auto& local_state =
        D41A0_0.array_0x2BDE[local_player_slot].dword_0x3E6_2BE4_12228;
    const std::int32_t ambient_loop_5_distance = local_state.dword_0x19A_410;
    const std::int32_t ambient_loop_31_distance = local_state.maxDistance_0x19E_414;
    if (level_one && (local_state.dword_0x19A_410 != 2048 ||
        local_state.maxDistance_0x19E_414 != 1746)) {
        throw std::runtime_error(
            "local player ambient-distance fields differ from preserved DOS frames 0..19");
    }
    json runtime_layout;
    Bytes runtime = capture_runtime(runtime_layout);
    Bytes presentation = capture_presentation();

    // sub_4A1E0 consumes authored feature types in the live structure. Re-load
    // a pristine local copy only after all live state above has been copied.
    Type_Level_2FECE authored{};
    if (!LevelDecompress_533B0(arguments.level.internal, &authored, "")) {
        throw std::runtime_error(
            "stock remc2 LevelDecompress_533B0 failed for external level " +
            std::to_string(arguments.level.external));
    }
    Bytes authored_bytes = serialize_authored_level(authored);
    const MapTypeProfile map_profile = map_type_profile(D41A0_0.terrain_2FECE.MapType);
    if (authored.MapType != D41A0_0.terrain_2FECE.MapType) {
        throw std::runtime_error("authored and live map types disagree at the frame-zero seam");
    }
    if (level_one && authored.MapType != MapType_t::Night) {
        throw std::runtime_error("Level 1 MapType is not Night (1)");
    }
    if (arguments.level.external == 2 && authored.MapType != MapType_t::Day) {
        throw std::runtime_error("Level 2 MapType is not Day (0)");
    }
    if (level_one && x_WORD_180660_VGA_type_resolution != 1) {
        throw std::runtime_error("Level 1 isolated feature seam is not using VGA selector 1");
    }
    std::vector<std::uint16_t> feature_building_selectors;
    Bytes feature_construction_inputs =
        capture_feature_construction_inputs(authored, feature_building_selectors, level_one);
    CollisionMaskCapture collision_masks;
    if (!level_one) {
        collision_masks = capture_collision_masks(owner_root, feature_building_selectors);
    }
    const Type_Level_2FECE feature_expectation = authored;
    const auto feature_allocator_trace = level_one
        ? expected_feature_allocator_trace(feature_expectation)
        : std::vector<FeatureAllocatorTrace>{};
    GenerateLevelMap_43830(&authored);
    Bytes terrain_pre_features = capture_terrain();
    if (non_cave_map) normalize_non_cave_inactive_terrain_plane(terrain_pre_features);
    if (!std::all_of(
            terrain_pre_features.data.begin() + 0x50000,
            terrain_pre_features.data.end(),
            [](std::uint8_t value) { return value == 0; })) {
        throw std::runtime_error("post-GenerateLevelMap entity-index plane is not zero");
    }
    Bytes feature_start = capture_feature_boundary("MFS3");
    if (level_one && (rand2_17B4E0 != kFeatureStartTerrainRng ||
        D41A0_0.rand_0x8 != kFeatureStartGlobalRng)) {
        throw std::runtime_error("isolated feature start RNGs differ from pristine Level 1");
    }
    std::memset(
        D41A0_0.struct_0x6E8E,
        0,
        sizeof(D41A0_0.struct_0x6E8E));
    sub_49F30();
    sub_49270_generate_level_features(&authored);
    if (level_one) validate_feature_house_fixture(feature_expectation);
    Bytes terrain_post_features = capture_terrain();
    if (non_cave_map) normalize_non_cave_inactive_terrain_plane(terrain_post_features);
    Bytes feature_end = capture_feature_boundary("MFE3");
    std::uint32_t feature_active_entities = 0;
    std::uint16_t feature_entity_record_size = 0;
    Bytes feature_entities = capture_normalized_feature_entities(
        feature_active_entities,
        feature_entity_record_size);
    Bytes feature_allocator = capture_feature_allocator(feature_expectation, level_one);
    if (level_one && feature_allocator_trace.size() != 51) {
        throw std::runtime_error("immutable feature allocator expectation changed after generation");
    }
    if ((!level_one && feature_entity_record_size != entity_record_size) ||
        (level_one && (feature_active_entities != 47 || feature_entity_record_size != entity_record_size))) {
        throw std::runtime_error(
            "isolated load-feature entity arena is not 47 source-compatible records");
    }

    fs::remove(runtime_config);

    std::vector<std::pair<const char*, const Bytes*>> files;
    files.reserve(level_one ? 13 : 14);
    files.emplace_back("authored-level.bin", &authored_bytes);
    if (!level_one) files.emplace_back("collision-masks.bin", &collision_masks.bytes);
    files.emplace_back("entities-post-features.bin", &feature_entities);
    files.emplace_back("entities.bin", &entities);
    files.emplace_back("feature-allocator.bin", &feature_allocator);
    files.emplace_back("feature-construction-inputs.bin", &feature_construction_inputs);
    files.emplace_back("feature-end.bin", &feature_end);
    files.emplace_back("feature-start.bin", &feature_start);
    files.emplace_back("players.bin", &players);
    files.emplace_back("presentation.bin", &presentation);
    files.emplace_back("runtime.bin", &runtime);
    files.emplace_back("terrain-post-features.bin", &terrain_post_features);
    files.emplace_back("terrain-pre-features.bin", &terrain_pre_features);
    files.emplace_back("terrain.bin", &terrain);
    json file_manifest = json::object();
    std::string identity_input;
    for (const auto& [name, content] : files) {
        write_bytes(staging / name, content->data);
        const std::string digest = sha256(content->data);
        file_manifest[name] = {{"bytes", content->data.size()}, {"sha256", digest}};
        identity_input += std::string(name);
        identity_input.push_back('\0');
        identity_input += digest;
        identity_input.push_back('\0');
        identity_input += std::to_string(content->data.size());
        identity_input.push_back('\n');
    }
    Sha256 package_hash;
    package_hash.update(identity_input);

    const std::string map_code(1, map_profile.asset_code);
    const std::string terrain_bank_name =
        "TMAPS" + std::to_string(map_profile.id) + "-0";
    const std::string medium_sprite_bank_name = "MSPR" + map_code + "0-0";
    const std::string high_sprite_bank_name = "HSPR" + map_code + "0-0";
    const std::string medium_web_bank_name = "MWEB" + map_code + "0-0";
    const std::string high_web_bank_name = "HWEB" + map_code + "0-0";
    const std::string palette_bank_name = "PAL" + map_code + "-0";
    const std::string sky_bank_name = "SKY" + map_code + "0-0";
    const std::string sprite_bank_name =
        x_WORD_180660_VGA_type_resolution == 1 ? medium_sprite_bank_name : high_sprite_bank_name;
    const std::string web_bank_name =
        x_WORD_180660_VGA_type_resolution == 1 ? medium_web_bank_name : high_web_bank_name;

    json manifest = {
        {"schema", level_one ? "mc2-owner-level-export/v3" : "mc2-owner-level-export/v5"},
        {"byte_order", "little-endian"},
        {"level", {
            {"external", arguments.level.external},
            {"internal", arguments.level.internal},
            {"map_type", {{"id", map_profile.id}, {"name", map_profile.name}}}
        }},
        {"capture", {{"site", "0x002285ff"}, {"frame", 0}}},
        {"feature_generation_oracle", {
            {"provenance", level_one
                ? "pristine authored Level 1 rerun after live frame-zero outputs were copied"
                : "authored level rerun after live frame-zero outputs were copied"},
            {"start_phase", "after-GenerateLevelMap_43830-before-sub_49F30-sub_49270"},
            {"end_phase", "after-sub_49270-before-group0"},
            {"vga_selector", x_WORD_180660_VGA_type_resolution},
            {"global_rng_barriers", level_one ? json{
                kFeatureGlobalRngBarriers[0],
                kFeatureGlobalRngBarriers[1],
                kFeatureGlobalRngBarriers[2],
                kFeatureGlobalRngBarriers[3],
                kFeatureGlobalRngBarriers[4],
                kFeatureGlobalRngBarriers[5],
                kFeatureGlobalRngBarriers[6]
            } : json::array()},
            {"start_record", {
                {"file", "feature-start.bin"},
                {"magic", "MFS3"},
                {"version", 1},
                {"bytes", kFeatureStartBytes},
                {"terrain_rng", {{"offset", 8}, {"encoding", "u16_le"}}},
                {"global_rng", {{"offset", 12}, {"encoding", "u32_le"}}}
            }},
            {"end_record", {
                {"file", "feature-end.bin"},
                {"magic", "MFE3"},
                {"version", 1},
                {"bytes", kFeatureStartBytes},
                {"terrain_rng", {{"offset", 8}, {"encoding", "u16_le"}}},
                {"global_rng", {{"offset", 12}, {"encoding", "u32_le"}}}
            }},
            {"pre_terrain", {
                {"file", "terrain-pre-features.bin"},
                {"bytes", kTerrainBytes},
                {"inactive_secondary_plane", non_cave_map ? "zero-normalized-non-cave" : "preserved-live-plane"}
            }},
            {"post_terrain", {
                {"file", "terrain-post-features.bin"},
                {"bytes", kTerrainBytes},
                {"inactive_secondary_plane", non_cave_map ? "zero-normalized-non-cave" : "preserved-live-plane"}
            }},
            {"post_entities", {
                {"file", "entities-post-features.bin"},
                {"magic", "MCE3"},
                {"version", 1},
                {"record_bytes", feature_entity_record_size},
                {"slots", kEntitySlots},
                {"active", feature_active_entities},
                {"inactive_payload", "zero-normalized-after-index-active-prefix"},
                {"map_heads", "terrain-post-features.bin entity-index plane"},
                {"map_links", "oldMapEntity/nextEntity in complete active records"}
            }},
            {"allocator", {
                {"file", "feature-allocator.bin"},
                {"magic", "MCA3"},
                {"version", 1},
                {"header_bytes", kFeatureAllocatorHeaderBytes},
                {"relation_count", kFeatureAllocatorRelationCount},
                {"trace_record_bytes", kFeatureAllocatorTraceRecordBytes},
                {"trace_provenance", level_one
                    ? "source-derived expected operations; counters and relations captured live"
                    : "live allocator counters and relations; no Level 1 trace oracle applied"}
            }},
            {"construction_inputs", {
                {"file", "feature-construction-inputs.bin"},
                {"magic", "MCI3"},
                {"version", 1},
                {"record_count", feature_building_selectors.size()},
                {"selectors", feature_building_selectors},
                {"source", "live pinned BLDGPRM rows plus BUILD footprint bytes"}
            }}
        }},
        {"dimensions", {{"terrain", {256, 256}}}},
        {"counts", {
            {"authored_features", kAuthoredFeatures},
            {"entity_slots", kEntitySlots},
            {"active_entities", active_entities},
            {"players", 8},
            {"particle_parameters", 347},
            {"level_sprite_selectors", 504},
            {"entity_parameter_records", 157}
        }},
        {"record_sizes", {
            {"entity", entity_record_size},
            {"player", player_record_size}
        }},
        {"record_layouts", {
            {"entity", {
                {"index_offset", 0},
                {"active_offset", 2},
                {"next_relation_offset", 4},
                {"parameter_relation_offset", 164},
                {"special_domain_offset", 168},
                {"special_relation_offset", 172}
            }},
            {"player", {
                {"index_offset", 0},
                {"normalized_host_time_offset", 50},
                {"legacy_v1_bytes", 565},
                {"ambient_loop_5_distance_offset", 565},
                {"ambient_loop_31_distance_offset", 569},
                {"record_bytes", player_record_size}
            }},
            {"runtime", runtime_layout},
            {"runtime_tagged_relations", {
                {"stages", {
                    {"domain_offset_in_record", 6},
                    {"relation_offset_in_record", 10},
                    {"domains", {
                        {"0", "scalar"},
                        {"1", "authored_feature_one_based"},
                        {"2", "live_entity_one_based"},
                        {"3", "live_entity_zero_based_index"}
                    }}
                }},
                {"stage_variables", {
                    {"domain_offset_in_record", 4},
                    {"relation_offset_in_record", 8},
                    {"domains", {{"0", "axis_scalar"}, {"1", "live_entity_one_based"}}}
                }},
                {"events", {
                    {"domain_offset_in_record", 10},
                    {"relation_offset_in_record", 14},
                    {"domains", {{"0", "null"}, {"1", "live_entity_one_based"}}}
                }}
            }}
        }},
        {"normalization", {
            {"native_pointers", "stable tagged relations; live/authored pools are one-based and zero is null"},
            {"volatile_host_state", {
                    non_cave_map
                        ? "inactive non-Cave secondary terrain plane zeroed; player time_393 host-clock sample zeroed"
                        : "player time_393 host-clock sample zeroed; secondary terrain plane preserved"
            }},
            {"integer_encoding", "explicit fixed-width little-endian semantic fields"}
        }},
        {"reset_projection", {
            {"schema", "mc2-flight-reset-projection/v1"},
            {"source", {
                {"capture_frame", 0},
                {"local_player_slot", {
                    {"file", "runtime.bin"}, {"offset", 16},
                    {"encoding", "i16_le"}, {"minimum", 0}, {"maximum", 7}
                }},
                {"entity_relation",
                    "players.bin local-player record offset 4 is the zero-based entities.bin record index"}
            }},
            {"field_mappings", {
                {"entity_index", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 4}, {"encoding", "u16_le"}
                }},
                {"position.x", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 80}, {"encoding", "u16_le"}
                }},
                {"position.y", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 82}, {"encoding", "u16_le"}
                }},
                {"position.z", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 84}, {"encoding", "i16_le"}
                }},
                {"yaw", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 32}, {"encoding", "i16_le"}
                }},
                {"display_pitch", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 34}, {"encoding", "i16_le"}
                }},
                {"effective_pitch", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 38}, {"encoding", "i16_le"}
                }},
                {"roll_accumulator", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 46}, {"encoding", "i16_le"}
                }},
                {"pitch_accumulator", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 48}, {"encoding", "i16_le"}
                }},
                {"target_speed", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 20}, {"encoding", "i16_le"}
                }},
                {"speed_changed_flag", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"actual_speed", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 134}, {"encoding", "i16_le"}
                }},
                {"strafe_speed", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 22}, {"encoding", "i16_le"}
                }},
                {"roll_delta", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 16}, {"encoding", "i16_le"}
                }},
                {"pitch_delta", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 18}, {"encoding", "i16_le"}
                }},
                {"recoil_speed", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 32}, {"encoding", "i16_le"}
                }},
                {"recoil_yaw", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 34}, {"encoding", "i16_le"}
                }},
                {"impulse_x", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 54}, {"encoding", "i16_le"}
                }},
                {"impulse_y", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 56}, {"encoding", "i16_le"}
                }},
                {"impulse_z", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 58}, {"encoding", "i16_le"}
                }},
                {"entity_fov", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 38}, {"encoding", "i16_le"}
                }},
                {"water_counter", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"speed_modifier", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 44}, {"encoding", "u8"}
                }},
                {"speed_modifier_counter", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 45}, {"encoding", "i8"}
                }},
                {"immobilize_counter", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"immobilize_subcounter", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"tether_prior_distance", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"tether_target_index", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"tether_holder_index", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"tether_tier", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"speed_spell_holder_index", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"entity_phase", {
                    {"kind", "entity_relation_field"}, {"file", "entities.bin"},
                    {"offset", 66}, {"encoding", "u8"}
                }},
                {"ambient_loop_5_distance", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 565}, {"encoding", "i32_le"},
                    {"source_field", "dword_0x19A_410"}
                }},
                {"ambient_loop_31_distance", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 569}, {"encoding", "i32_le"},
                    {"source_field", "maxDistance_0x19E_414"}
                }},
                {"player_color_index", {
                    {"kind", "exported_field"}, {"file", "players.bin"},
                    {"record", "local_player_slot"}, {"offset", 40},
                    {"encoding", "u8_low_of_i16_le"}
                }},
                {"music_countdown", {{"kind", "zero_at_seam"}, {"value", 0}}},
                {"recovery_latch", {{"kind", "zero_at_seam"}, {"value", false}}},
                {"skip_flight_once", {
                    {"kind", "entity_relation_bit"}, {"file", "entities.bin"},
                    {"offset", 17}, {"bit", 3}
                }},
                {"tick", {{"kind", "zero_at_seam"}, {"value", 0}}}
            }},
            {"expected_state", {
                {"entity_index", 152},
                {"position", {{"x", 19840}, {"y", 56960}, {"z", 5024}}},
                {"yaw", 0}, {"display_pitch", 0}, {"effective_pitch", 0},
                {"roll_accumulator", 0}, {"pitch_accumulator", 0},
                {"target_speed", 0}, {"speed_changed_flag", 0},
                {"actual_speed", 0}, {"strafe_speed", 0},
                {"roll_delta", 0}, {"pitch_delta", 0},
                {"recoil_speed", 0}, {"recoil_yaw", 0},
                {"impulse_x", 0}, {"impulse_y", 0}, {"impulse_z", 0},
                {"entity_fov", 0}, {"water_counter", 0},
                {"speed_modifier", 0}, {"speed_modifier_counter", 0},
                {"immobilize_counter", 0}, {"immobilize_subcounter", 0},
                {"tether_prior_distance", 0}, {"tether_target_index", 0},
                {"tether_holder_index", 0}, {"tether_tier", 0},
                {"speed_spell_holder_index", 0}, {"entity_phase", 153},
                {"ambient_loop_5_distance", 2048},
                {"ambient_loop_31_distance", 1746},
                {"player_color_index", 0}, {"music_countdown", 0},
                {"recovery_latch", false}, {"skip_flight_once", false},
                {"tick", 0}
            }},
            {"state_hash", {
                {"api", "mc_sim_reset"},
                {"schema", "mc2-flight-state-hash/fnv1a64-v1"},
                {"algorithm", "FNV-1a 64-bit"},
                {"byte_order", "little-endian"},
                {"canonical_bytes", 81},
                {"ordered_fields", {
                    "entity_index:u16", "position.x:u16", "position.y:u16", "position.z:i16",
                    "yaw:i16", "display_pitch:i16", "effective_pitch:i16",
                    "roll_accumulator:i16", "pitch_accumulator:i16", "target_speed:i16",
                    "speed_changed_flag:i16", "actual_speed:i16", "strafe_speed:i16",
                    "roll_delta:i16", "pitch_delta:i16", "recoil_speed:i16",
                    "recoil_yaw:i16", "impulse_x:i16", "impulse_y:i16", "impulse_z:i16",
                    "entity_fov:i16", "water_counter:i8", "speed_modifier:u8",
                    "speed_modifier_counter:i8", "immobilize_counter:i8",
                    "immobilize_subcounter:i8", "tether_prior_distance:i32",
                    "tether_target_index:u16", "tether_holder_index:u16", "tether_tier:u16",
                    "speed_spell_holder_index:u16", "entity_phase:u8",
                    "ambient_loop_5_distance:i32", "ambient_loop_31_distance:i32",
                    "player_color_index:u8", "music_countdown:i16", "recovery_latch:u8",
                    "skip_flight_once:u8", "tick:u64"
                }},
                {"hex", "9da4bc91af58f3ce"}
            }},
            {"conformance_hash", {
                {"schema", "mc2-flight-conformance-hash/fnv1a64-v1"},
                {"context", "NativeSim::owner_level_one"},
                {"hex", "77b96b82492d6f4b"},
                {"derivation",
                    "sim-core conformance_hash over reset FlightState and sim-capi ABI v1 owner-level-one FlightContext"}
            }}
        }},
        {"presentation_dependencies", {
            {"map_type", map_profile.name},
            {"vga_resolution_selector", x_WORD_180660_VGA_type_resolution},
            {"texture_size", x_BYTE_D41B5_texture_size},
            {"terrain_graphics_flags", D41A0_0.terrain_2FECE.byte_0x2FED2},
            {"terrain_graphics_flag_2", (D41A0_0.terrain_2FECE.byte_0x2FED2 & 2) != 0},
            {"terrain_bank", terrain_bank_name},
            {"sprite_bank", sprite_bank_name},
            {"web_bank", web_bank_name},
            {"block_bank",
                std::string("BL") + std::to_string(x_BYTE_D41B5_texture_size) +
                ((D41A0_0.terrain_2FECE.byte_0x2FED2 & 2) ? "F0-0" : "N0-0")},
            {"palette",
                (D41A0_0.terrain_2FECE.byte_0x2FED2 & 2) ? "PALF-0" : palette_bank_name},
            {"sky_bank", x_BYTE_D41B5_texture_size == 32 ? sky_bank_name : ""},
            {"resolved_files", {
                {"level_records", resolved_owner_asset(
                    owner_root,
                    "GAME/NETHERW/CLEVELS/LEVELS.DAT",
                    "CD_Files/LEVELS/LEVELS.DAT")},
                {"terrain_bank", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/") + terrain_bank_name + ".DAT",
                    std::string("CD_Files/DATA/") + terrain_bank_name + ".DAT")},
                {"sprite_bank", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/") +
                        sprite_bank_name + ".DAT",
                    std::string("CD_Files/DATA/") +
                        sprite_bank_name + ".DAT")},
                {"web_bank", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/") +
                        web_bank_name + ".DAT",
                    std::string("CD_Files/DATA/") +
                        web_bank_name + ".DAT")},
                {"block_bank", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/BL") +
                        std::to_string(x_BYTE_D41B5_texture_size) +
                        ((D41A0_0.terrain_2FECE.byte_0x2FED2 & 2) ? "F0-0.DAT" : "N0-0.DAT"),
                    std::string("CD_Files/DATA/BL") +
                        std::to_string(x_BYTE_D41B5_texture_size) +
                        ((D41A0_0.terrain_2FECE.byte_0x2FED2 & 2) ? "F0-0.DAT" : "N0-0.DAT"))},
                {"palette", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/") +
                        ((D41A0_0.terrain_2FECE.byte_0x2FED2 & 2)
                             ? "PALF-0.DAT" : palette_bank_name + ".DAT"),
                    std::string("CD_Files/DATA/") +
                        ((D41A0_0.terrain_2FECE.byte_0x2FED2 & 2)
                             ? "PALF-0.DAT" : palette_bank_name + ".DAT"))},
                {"sky_bank", resolved_owner_asset(
                    owner_root,
                    std::string("GAME/NETHERW/CDATA/") + sky_bank_name + ".DAT",
                    std::string("CD_Files/DATA/") + sky_bank_name + ".DAT")},
                {"building_bank", resolved_owner_asset(
                    owner_root,
                    "GAME/NETHERW/CDATA/BUILD0-0.DAT",
                    "CD_Files/DATA/BUILD0-0.DAT")},
                {"spell_table", resolved_owner_asset(
                    owner_root,
                    "GAME/NETHERW/CDATA/SPELLS.DAT",
                    "CD_Files/DATA/SPELLS.DAT")},
                {"font_bank", resolved_owner_asset(
                    owner_root,
                    "GAME/NETHERW/CDATA/FONT1.DAT",
                    "CD_Files/DATA/FONT1.DAT")},
                {"language", resolved_owner_asset(
                    owner_root,
                    "GAME/NETHERW/LANGUAGE/L1.TXT",
                    "CD_Files/LANGUAGE/L1.TXT")}
            }},
            {"sound_bank", 1},
            {"cursor", 9}
        }},
        {"source", {
            {"upstream_commit", MC2_UPSTREAM_COMMIT},
            {"upstream_worktree_diff_sha256", MC2_UPSTREAM_DIFF_SHA256},
            {"compiled_source_sha256", MC2_COMPILED_SOURCE_SHA256},
            {"modified_paths", json::parse(MC2_UPSTREAM_STATUS_JSON)},
            {"modified_files", json::parse(MC2_MODIFIED_FILE_HASHES_JSON)},
            {"compiled_files", json::parse(MC2_SOURCE_INVENTORY_JSON)},
            {"owner_files", make_owner_sources(owner_root, map_profile, !level_one)}
        }},
        {"reference", {
            {"terrain_frame0_sha256", level_one ? kExpectedTerrainSha256 : sha256(terrain.data)},
            {"terrain_full_match", level_one ? json(false) : json(nullptr)},
            {"runtime_scalar_divergences", level_one ? json{
                {
                    {"field", "maptypeMusic_0x235"},
                    {"original_state_offset", "0x235"},
                    {"runtime_bin_offset", 32},
                    {"exported_remc2_value", 1},
                    {"preserved_dos_frames", "0..19"},
                    {"preserved_dos_value", 0},
                    {"status", "presentation_environment_divergence"},
                    {"evidence",
                        "native remc2 selects the Night music bank when music is enabled; the oracle capture had music inactive"}
                }
            } : json::array()},
            {"terrain_planes", {
                {
                    {"name", "type"}, {"offset", 0x00000}, {"bytes", 0x10000},
                    {"stock_regression_compared", true},
                    {"exported_sha256", terrain_type_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainTypeSha256) : json(nullptr)},
                    {"match", level_one}
                },
                {
                    {"name", "height"}, {"offset", 0x10000}, {"bytes", 0x10000},
                    {"stock_regression_compared", true},
                    {"exported_sha256", terrain_height_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainHeightSha256) : json(nullptr)},
                    {"match", level_one}
                },
                {
                    {"name", "shade"}, {"offset", 0x20000}, {"bytes", 0x10000},
                    {"stock_regression_compared", true},
                    {"exported_sha256", terrain_shade_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainShadeSha256) : json(nullptr)},
                    {"match", level_one}
                },
                {
                    {"name", "angle"}, {"offset", 0x30000}, {"bytes", 0x10000},
                    {"stock_regression_compared", true},
                    {"exported_sha256", terrain_angle_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainAngleSha256) : json(nullptr)},
                    {"match", level_one}
                },
                {
                    {"name", "secondary_cave"}, {"offset", 0x40000}, {"bytes", 0x10000},
                    {"stock_regression_compared", false},
                    {"source_status", non_cave_map ? "inactive_uninitialized_host_state" : "live_plane"},
                    {"normalization", non_cave_map ? "inactive_non_regression_compared_zeroed" : "preserved"},
                    {"exported_sha256", terrain_secondary_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainSecondarySha256) : json(nullptr)},
                    {"match", false}
                },
                {
                    {"name", "entity_indices"}, {"offset", 0x50000}, {"bytes", 0x20000},
                    {"stock_regression_compared", true},
                    {"exported_sha256", terrain_entity_sha},
                    {"oracle_sha256", level_one ? json(kExpectedTerrainEntitySha256) : json(nullptr)},
                    {"match", level_one}
                }
            }},
            {"state_sequence", {
                {"frames", level_one ? 20 : 0},
                {"bytes_per_frame", 224790},
                {"sha256", level_one
                    ? "021ab6f582cae920546dc3bcf55b06d00937fea5ddaabfeae079c0368b314e7f"
                    : ""}
            }},
            {"ambient_distance_sequence", {
                {"capture_site", "0x002285ff"},
                {"frames", level_one
                    ? json{{"first", 0}, {"last", 19}, {"count", 20}}
                    : json{{"first", 0}, {"last", 0}, {"count", 1}}},
                {"local_player_slot", 0},
                {"dword_0x19A_410", {
                    {"semantic", "ambient_loop_5_distance"},
                    {"value", ambient_loop_5_distance}, {"invariant", level_one}
                }},
                {"maxDistance_0x19E_414", {
                    {"semantic", "ambient_loop_31_distance"},
                    {"value", ambient_loop_31_distance}, {"invariant", level_one}
                }}
            }},
            {"terrain_sequence", {
                {"frames", level_one ? 21 : 1},
                {"bytes_per_frame", kTerrainBytes},
                {"sha256", level_one
                    ? "16f0f778b120ddaab042f28b8a322efb37940bb68b4527b7d12dac82101db88f"
                    : sha256(terrain.data)}
            }}
        }},
        {"files", file_manifest},
        {"package_sha256", package_hash.finish()}
    };
    if (!level_one) {
        manifest["record_layouts"]["player"]["type_str_164_auxiliary"] =
            type_str_164_auxiliary_layout();
        manifest["reference"]["type_str_164_auxiliary"] =
            type_str_164_auxiliary_reference();
        manifest["record_layouts"]["player"]["banishment_state_0x006"] =
            banishment_state_layout();
        manifest["reference"]["banishment_state_0x006"] =
            banishment_state_reference();
        manifest["collision_masks"] = collision_masks.metadata;
    }
    // The reset projection is a sealed Level 1 contract.  Do not copy its
    // Night-specific expected state into an open Level 2 package; Level 2 gets
    // its own reset seal only when that corpus is qualified.
    if (!level_one) manifest.erase("reset_projection");
    write_text(staging / "manifest.json", manifest.dump(2) + "\n");

    fs::rename(staging, output);
    staging_cleanup.committed = true;
    std::cout << "exported deterministic private external Level "
              << arguments.level.external << " package: " << output << "\n";
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    try {
        return run(parse_arguments(argc, argv));
    } catch (const std::exception& error) {
        std::cerr << "remc2-level-exporter: " << error.what() << "\n";
        return 1;
    }
}
