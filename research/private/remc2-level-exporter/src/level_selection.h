#pragma once

#include <charconv>
#include <cstdint>
#include <stdexcept>
#include <string_view>

namespace mc2_exporter {

enum class SourceLevelKind : std::uint8_t {
    Ordinary,
    Secret,
};

struct SourceLevelSelection {
    // The package identity keeps the source's one-based external label and
    // zero-based internal source record index.
    std::uint16_t external = 1;
    std::uint16_t internal = 0;
    SourceLevelKind kind = SourceLevelKind::Ordinary;
};

// The owner-facing ordinary contract is one-based; remc2's --set_level
// contract is zero-based. Keep this conversion in one production helper.
inline SourceLevelSelection parse_external_level(std::string_view value) {
    if (value.empty()) {
        throw std::runtime_error("--level requires an unsigned external level");
    }
    // Do not accept signs, whitespace, or a numeric prefix. In particular,
    // std::stoi("2junk") would silently select level 2 and hide a typo.
    for (const char character : value) {
        if (character < '0' || character > '9') {
            throw std::runtime_error("--level must be an integer from 1 through 25");
        }
    }
    unsigned int parsed = 0;
    const auto conversion = std::from_chars(
        value.data(), value.data() + value.size(), parsed, 10);
    if (conversion.ec != std::errc{} || conversion.ptr != value.data() + value.size() ||
        parsed < 1 || parsed > 25) {
        throw std::runtime_error("--level must be an integer from 1 through 25");
    }
    return {
        static_cast<std::uint16_t>(parsed),
        static_cast<std::uint16_t>(parsed - 1),
        SourceLevelKind::Ordinary,
    };
}

// Source indices 30 through 34 are secret destinations reached through the
// source portal associations. Keep this selector separate from ordinary levels.
inline SourceLevelSelection parse_secret_index(std::string_view value) {
    if (value.empty()) {
        throw std::runtime_error("--secret-index requires an unsigned source index");
    }
    for (const char character : value) {
        if (character < '0' || character > '9') {
            throw std::runtime_error("--secret-index must be a source secret index from 30 through 34");
        }
    }
    unsigned int parsed = 0;
    const auto conversion = std::from_chars(
        value.data(), value.data() + value.size(), parsed, 10);
    if (conversion.ec != std::errc{} || conversion.ptr != value.data() + value.size() ||
        parsed < 30 || parsed > 34) {
        throw std::runtime_error("--secret-index must be a source secret index from 30 through 34");
    }
    const auto internal = static_cast<std::uint16_t>(parsed);
    return {
        static_cast<std::uint16_t>(internal + 1),
        internal,
        SourceLevelKind::Secret,
    };
}

}  // namespace mc2_exporter
