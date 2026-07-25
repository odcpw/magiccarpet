#include <cstddef>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>
#include <type_traits>

#include "portability/InputRecorder.h"

namespace {

constexpr std::uint64_t kFnvOffset = 14695981039346656037ULL;
constexpr std::uint64_t kFnvPrime = 1099511628211ULL;

void HashByte(std::uint64_t& hash, std::uint8_t value)
{
	hash ^= value;
	hash *= kFnvPrime;
}

template <typename T>
void HashLittleEndian(std::uint64_t& hash, T value)
{
	using Unsigned = std::make_unsigned_t<T>;
	Unsigned unsignedValue = static_cast<Unsigned>(value);
	for (std::size_t index = 0; index < sizeof(T); ++index)
	{
		HashByte(hash, static_cast<std::uint8_t>(unsignedValue & 0xffU));
		unsignedValue >>= 8U;
	}
}

std::string Hex64(std::uint64_t value)
{
	std::ostringstream stream;
	stream << std::hex << std::setfill('0') << std::setw(16) << value;
	return stream.str();
}

} // namespace

int main(int argc, char** argv)
{
	if (argc != 2)
	{
		std::cerr << "usage: recording_probe RECORDING\n";
		return 2;
	}

	InputRecorder recorder(argv[1]);
	if (!recorder.StartPlayback())
	{
		std::cerr << "recording load failed\n";
		return 3;
	}

	constexpr std::uint16_t level = 0;
	constexpr std::uint16_t playerIndex = 0;
	RecordedEventPlayer* player = recorder.GetCurrentPlayer(level, playerIndex);
	if (player == nullptr || player->Turns == nullptr || player->Turns->empty())
	{
		std::cerr << "level 0 player 0 has no turns\n";
		return 4;
	}

	std::uint64_t hash = kFnvOffset;
	HashLittleEndian(hash, level);
	HashLittleEndian(hash, playerIndex);

	for (std::size_t index = 0; index < 26; ++index)
	{
		HashLittleEndian(hash, player->SpellsEnabled[index]);
		HashByte(hash, player->SpellIndexes[index]);
		HashByte(hash, player->SpellLevels[index]);
		HashLittleEndian(hash, player->SpellsExperience[index]);
	}

	std::uint64_t payloadBytes = 0;
	for (const auto& [turnNumber, turn] : *player->Turns)
	{
		if (turn == nullptr || turn->Bytes == nullptr)
		{
			std::cerr << "recording contains a null turn payload\n";
			return 5;
		}
		HashLittleEndian(hash, turnNumber);
		HashLittleEndian(hash, turn->SizeBytes);
		for (std::uint32_t index = 0; index < turn->SizeBytes; ++index)
		{
			HashByte(hash, turn->Bytes[index]);
		}
		payloadBytes += turn->SizeBytes;
	}

	const auto firstTurn = player->Turns->begin()->first;
	const auto lastTurn = player->Turns->rbegin()->first;
	std::cout
		<< "{\"loaded\":true"
		<< ",\"level\":" << level
		<< ",\"player\":" << playerIndex
		<< ",\"declared_turns\":" << player->TurnCount
		<< ",\"parsed_turns\":" << player->Turns->size()
		<< ",\"first_turn\":" << firstTurn
		<< ",\"last_turn\":" << lastTurn
		<< ",\"payload_bytes\":" << payloadBytes
		<< ",\"fnv1a64\":\"" << Hex64(hash) << "\"}\n";
	return 0;
}
