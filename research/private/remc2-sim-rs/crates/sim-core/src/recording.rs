use crate::PlayerCommand;

const SIGNATURE: &[u8; 16] = b"MC2-HD-Recording";
const SPELL_COUNT: usize = 26;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputRecording<'a> {
    pub levels: Vec<RecordedLevel<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedLevel<'a> {
    pub level: u16,
    pub players: Vec<RecordedPlayer<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedPlayer<'a> {
    pub player_index: u16,
    pub spells_enabled: [i16; SPELL_COUNT],
    pub spell_indexes: [u8; SPELL_COUNT],
    pub spell_levels: [u8; SPELL_COUNT],
    pub spells_experience: [i32; SPELL_COUNT],
    pub turns: Vec<RecordedTurn<'a>>,
}

impl RecordedPlayer<'_> {
    #[must_use]
    pub fn turn(&self, turn: u32) -> Option<&RecordedTurn<'_>> {
        self.turns
            .binary_search_by_key(&turn, |candidate| candidate.turn)
            .ok()
            .map(|index| &self.turns[index])
    }

    /// Decode a ten-byte player input payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested turn is missing or its payload is
    /// not exactly the original ten-byte input structure.
    pub fn command(&self, turn: u32) -> Result<PlayerCommand, String> {
        let recorded = self
            .turn(turn)
            .ok_or_else(|| format!("recording has no turn {turn}"))?;
        let bytes: [u8; 10] = recorded.payload.try_into().map_err(|_| {
            format!(
                "recorded turn {turn} has {} bytes, expected 10",
                recorded.payload.len()
            )
        })?;
        Ok(PlayerCommand::from_bytes(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordedTurn<'a> {
    pub turn: u32,
    pub payload: &'a [u8],
}

impl<'a> InputRecording<'a> {
    /// Parse the current remc2 `MC2-HD-Recording` format with spell metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for a bad signature, a truncated field, an impossible
    /// payload length, duplicate/unsorted turns, or trailing malformed data.
    pub fn parse_with_spell_metadata(bytes: &'a [u8]) -> Result<Self, String> {
        Self::parse_format(bytes, true)
    }

    /// Parse the older regression-corpus variant without spell metadata.
    ///
    /// It uses the same signature but begins turn records immediately after
    /// each player header. Spell arrays in the returned view are zero.
    ///
    /// # Errors
    ///
    /// Returns the same structural errors as
    /// [`Self::parse_with_spell_metadata`].
    pub fn parse_legacy_regression(bytes: &'a [u8]) -> Result<Self, String> {
        Self::parse_format(bytes, false)
    }

    fn parse_format(bytes: &'a [u8], has_spell_metadata: bool) -> Result<Self, String> {
        let mut reader = Reader::new(bytes);
        if reader.take(SIGNATURE.len())? != SIGNATURE {
            return Err("input recording signature mismatch".to_string());
        }

        let mut levels = Vec::new();
        while !reader.is_empty() {
            let level = reader.u16()?;
            let player_count = usize::from(reader.u16()?);
            let mut players = Vec::with_capacity(player_count);
            for _ in 0..player_count {
                let player_index = reader.u16()?;
                let turn_count = usize::try_from(reader.u32()?)
                    .map_err(|_| "turn count does not fit this host".to_string())?;
                let mut spells_enabled = [0_i16; SPELL_COUNT];
                let mut spell_indexes = [0_u8; SPELL_COUNT];
                let mut spell_levels = [0_u8; SPELL_COUNT];
                let mut spells_experience = [0_i32; SPELL_COUNT];
                if has_spell_metadata {
                    for value in &mut spells_enabled {
                        *value = reader.i16()?;
                    }
                    spell_indexes.copy_from_slice(reader.take(SPELL_COUNT)?);
                    spell_levels.copy_from_slice(reader.take(SPELL_COUNT)?);
                    for value in &mut spells_experience {
                        *value = reader.i32()?;
                    }
                }

                let mut turns = Vec::with_capacity(turn_count);
                let mut previous = None;
                for _ in 0..turn_count {
                    let turn = reader.u32()?;
                    if previous.is_some_and(|value| turn <= value) {
                        return Err(format!(
                            "turns for level {level} player {player_index} are not strictly ordered"
                        ));
                    }
                    previous = Some(turn);
                    let size = usize::try_from(reader.u32()?)
                        .map_err(|_| "turn payload length does not fit this host".to_string())?;
                    turns.push(RecordedTurn {
                        turn,
                        payload: reader.take(size)?,
                    });
                }
                players.push(RecordedPlayer {
                    player_index,
                    spells_enabled,
                    spell_indexes,
                    spell_levels,
                    spells_experience,
                    turns,
                });
            }
            levels.push(RecordedLevel { level, players });
        }
        Ok(Self { levels })
    }

    #[must_use]
    pub fn player(&self, level: u16, player_index: u16) -> Option<&RecordedPlayer<'_>> {
        self.levels
            .iter()
            .find(|candidate| candidate.level == level)?
            .players
            .iter()
            .find(|candidate| candidate.player_index == player_index)
    }
}

#[derive(Clone, Copy, Debug)]
struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn is_empty(self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        if count > self.remaining.len() {
            return Err(format!(
                "input recording is truncated: need {count} bytes, have {}",
                self.remaining.len()
            ));
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("fixed u16 span"),
        ))
    }

    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()?.cast_signed())
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("fixed u32 span"),
        ))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()?.cast_signed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNED_ENRICHED_RECORDING: &[u8] = include_bytes!(
        "../../../../wasm-probes/input-recording-parser/artifacts/recording-probe.data"
    );
    const OWNED_LEGACY_REGRESSION: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/afterloadtest2/Levels-1-5-Recording.bin"
    );

    #[test]
    fn owned_recording_matches_the_existing_native_wasm_probe() {
        let recording =
            InputRecording::parse_with_spell_metadata(OWNED_ENRICHED_RECORDING).unwrap();
        let player = recording.player(0, 0).unwrap();
        assert_eq!(player.turns.len(), 272);
        assert_eq!(player.turns.first().unwrap().turn, 1);
        assert_eq!(player.turns.last().unwrap().turn, 272);
        assert_eq!(
            player
                .turns
                .iter()
                .map(|turn| turn.payload.len())
                .sum::<usize>(),
            2_720
        );
        assert_eq!(probe_hash(0, player), 0x2282_f2c4_ee21_151f);
    }

    #[test]
    fn owned_legacy_regression_recording_has_explicit_bare_layout() {
        let recording = InputRecording::parse_legacy_regression(OWNED_LEGACY_REGRESSION).unwrap();
        let player = recording.player(0, 0).unwrap();
        assert_eq!(player.turns.len(), 2_932);
        assert_eq!(player.turns.first().unwrap().turn, 1);
        assert_eq!(player.turns.last().unwrap().turn, 2_932);
        assert!(player.turns.iter().all(|turn| turn.payload.len() == 10));
        assert!(
            InputRecording::parse_with_spell_metadata(OWNED_LEGACY_REGRESSION).is_err(),
            "the two same-signature layouts must never be silently conflated"
        );
    }

    fn probe_hash(level: u16, player: &RecordedPlayer<'_>) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for &byte in bytes {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        feed(&level.to_le_bytes());
        feed(&player.player_index.to_le_bytes());
        for index in 0..SPELL_COUNT {
            feed(&player.spells_enabled[index].to_le_bytes());
            feed(&[player.spell_indexes[index]]);
            feed(&[player.spell_levels[index]]);
            feed(&player.spells_experience[index].to_le_bytes());
        }
        for turn in &player.turns {
            feed(&turn.turn.to_le_bytes());
            feed(
                &u32::try_from(turn.payload.len())
                    .expect("fixture payload length")
                    .to_le_bytes(),
            );
            feed(turn.payload);
        }
        hash
    }
}
