#include <adlmidi.h>

#include <cstdint>
#include <fstream>
#include <iterator>
#include <vector>

static void put16(std::ofstream& stream, std::uint16_t value)
{
    stream.put(static_cast<char>(value));
    stream.put(static_cast<char>(value >> 8));
}

static void put32(std::ofstream& stream, std::uint32_t value)
{
    put16(stream, static_cast<std::uint16_t>(value));
    put16(stream, static_cast<std::uint16_t>(value >> 16));
}

int main(int argc, char** argv)
{
    if (argc != 3)
        return 2;
    std::ifstream input(argv[1], std::ios::binary);
    std::vector<unsigned char> midi{
        std::istreambuf_iterator<char>(input),
        std::istreambuf_iterator<char>()};
    if (!input.good() && !input.eof())
        return 3;
    constexpr long sampleRate = 44100;
    ADL_MIDIPlayer* player = adl_init(sampleRate);
    if (player == nullptr)
        return 4;
    adl_setNumChips(player, 2);
    adl_setLoopEnabled(player, 0);
    adl_setBank(player, 59);
    if (adl_openData(player, midi.data(), midi.size()) < 0)
    {
        adl_close(player);
        return 5;
    }
    std::vector<std::int16_t> pcm;
    std::vector<std::int16_t> block(4096);
    while (!adl_atEnd(player))
    {
        const int samples = adl_play(player, block.size(), block.data());
        if (samples < 0)
        {
            adl_close(player);
            return 6;
        }
        pcm.insert(pcm.end(), block.begin(), block.begin() + samples);
    }
    adl_close(player);
    if (pcm.empty() || (pcm.size() & 1) != 0)
        return 7;

    std::ofstream output(argv[2], std::ios::binary | std::ios::trunc);
    const std::uint32_t dataBytes =
        static_cast<std::uint32_t>(pcm.size() * sizeof(std::int16_t));
    output.write("RIFF", 4);
    put32(output, 36 + dataBytes);
    output.write("WAVEfmt ", 8);
    put32(output, 16);
    put16(output, 1);
    put16(output, 2);
    put32(output, sampleRate);
    put32(output, sampleRate * 4);
    put16(output, 4);
    put16(output, 16);
    output.write("data", 4);
    put32(output, dataBytes);
    output.write(
        reinterpret_cast<const char*>(pcm.data()),
        static_cast<std::streamsize>(dataBytes));
    return output.good() ? 0 : 8;
}
