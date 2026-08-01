#pragma once

#include <charconv>
#include <cstdint>
#include <stdexcept>
#include <string_view>

namespace mc2_exporter {

struct ExternalLevelSelection {
    std::uint16_t external = 1;
    std::uint16_t internal = 0;
};

// The owner-facing contract is one-based; remc2's --set_level contract is
// zero-based. Keep this conversion in one production helper so every caller
// (CLI and tests) exercises the same validation and mapping.
inline ExternalLevelSelection parse_external_level(std::string_view value) {
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
    };
}

}  // namespace mc2_exporter
