#include <cstdint>
#include <fstream>
#include <iostream>
#include <vector>

#include "DataFileRNC.h"

int main(int argc, char** argv) {
    if (argc != 3) return 2;
    std::ifstream input(argv[1], std::ios::binary | std::ios::ate);
    if (!input) return 3;
    const auto packed_size = static_cast<std::size_t>(input.tellg());
    input.seekg(0);
    std::vector<std::uint8_t> bytes(packed_size);
    input.read(reinterpret_cast<char*>(bytes.data()), packed_size);
    if (!input || packed_size < 18) return 4;
    const std::size_t unpacked =
        (std::size_t(bytes[4]) << 24) | (std::size_t(bytes[5]) << 16) |
        (std::size_t(bytes[6]) << 8) | std::size_t(bytes[7]);
    bytes.resize(std::max(bytes.size(), unpacked + 18));
    const int result = DataFileRNC::Decompress(bytes.data(), bytes.data());
    if (result < 0 || static_cast<std::size_t>(result) != unpacked) return 5;
    std::ofstream output(argv[2], std::ios::binary | std::ios::trunc);
    output.write(reinterpret_cast<const char*>(bytes.data()), result);
    return output ? 0 : 6;
}
