#include <cstdint>
#include <fstream>
#include <iterator>
#include <vector>

#include "../../../upstream/magic-carpet-2-hd/remc2/portability/xmi2mid.h"

int main(int argc, char** argv)
{
    if (argc != 3)
        return 2;
    std::ifstream input(argv[1], std::ios::binary);
    std::vector<unsigned char> source{
        std::istreambuf_iterator<char>(input),
        std::istreambuf_iterator<char>()};
    if (!input.good() && !input.eof())
        return 3;
    std::size_t outputSize = 0;
    unsigned char* output = TranscodeXmiToMid(
        source.data(), source.size(), &outputSize);
    if (output == nullptr || outputSize == 0)
        return 4;
    std::ofstream target(argv[2], std::ios::binary | std::ios::trunc);
    target.write(reinterpret_cast<const char*>(output), outputSize);
    delete[] output;
    return target.good() ? 0 : 5;
}
