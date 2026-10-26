//! Authoritative per-player result word produced by source PlayerEvents.
//!
//! The remc2 `dw_w_b.word[1]` result is a 16-bit field. Source action 29
//! reaches PlayerEvents case 0x1D, which writes 8 and sets bit 0x10 in the
//! low result byte only when the internal source level is greater than 24.
//! Keep this carrier separate from `LevelExitSequence`: action 11 completes
//! that sequence with result 0x1A, while ordinary action-12 exit is 0x0A.

const PLAYER_EVENT_1D_RESULT: u16 = 0x0008;
const SECRET_RESULT_BIT: u16 = 0x0010;
const SECRET_RESULT_LEVEL_THRESHOLD: u16 = 24;
const SECRET_BANISHMENT_RESULT: u16 = PLAYER_EVENT_1D_RESULT | SECRET_RESULT_BIT;

/// The source player's live `dw_w_b.word[1]` result, scoped to one player in
/// one live mission session. The owning session carries its runtime identity;
/// cloning preserves the result through replay without inventing a second
/// generation counter here. The owning campaign hash must include its slot
/// and raw word. `source_player_slot` is the player-array slot, not the
/// player's entity index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourcePlayerResult {
    source_player_slot: u16,
    result_word: u16,
}

impl SourcePlayerResult {
    /// Initialize the source result word at the real start of a level.
    pub(crate) const fn source_level_start(source_player_slot: u16) -> Self {
        Self {
            source_player_slot,
            result_word: 0,
        }
    }

    /// Record the result only when the scheduled player event 0x1D actually
    /// runs for this carrier's source player. The scheduler must call this
    /// from the already-admitted action-29 handler, after source action-slot
    /// occupancy has accepted 29. A different player identity is rejected
    /// without changing the authoritative word.
    pub(crate) fn record_player_event_0x1d(
        &mut self,
        source_player_slot: u16,
        internal_level: u16,
    ) -> bool {
        if self.source_player_slot != source_player_slot {
            return false;
        }

        self.result_word = PLAYER_EVENT_1D_RESULT
            | if internal_level > SECRET_RESULT_LEVEL_THRESHOLD {
                SECRET_RESULT_BIT
            } else {
                0
            };
        true
    }

    pub(crate) const fn source_player_slot(&self) -> u16 {
        self.source_player_slot
    }

    /// Return the raw source-width word, preserving ordinary result 0x0008.
    pub(crate) const fn raw_word(&self) -> u16 {
        self.result_word
    }

    /// Match only the exact secret result for the same source player. The
    /// guarded campaign consumer must additionally validate the active secret
    /// frame and runtime/session generation before returning from it.
    pub(crate) const fn is_secret_banishment_for(&self, source_player_slot: u16) -> bool {
        self.source_player_slot == source_player_slot
            && matches!(self.result_word, SECRET_BANISHMENT_RESULT)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PLAYER_EVENT_1D_RESULT, SECRET_BANISHMENT_RESULT, SECRET_RESULT_LEVEL_THRESHOLD,
        SourcePlayerResult,
    };

    #[test]
    fn source_event_keeps_the_full_word_and_distinguishes_ordinary_from_secret() {
        let mut ordinary = SourcePlayerResult::source_level_start(3);
        assert_eq!(ordinary.raw_word(), 0);
        assert!(ordinary.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD));
        assert_eq!(ordinary.raw_word(), PLAYER_EVENT_1D_RESULT);
        assert!(!ordinary.is_secret_banishment_for(3));

        let mut secret = SourcePlayerResult::source_level_start(3);
        assert!(secret.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(secret.raw_word(), SECRET_BANISHMENT_RESULT);
        assert!(secret.is_secret_banishment_for(3));
        assert!(secret.is_secret_banishment_for(3), "query does not consume");
        assert!(!secret.is_secret_banishment_for(4));
    }

    #[test]
    fn source_event_rejects_a_different_player_without_mutating_its_result() {
        let mut result = SourcePlayerResult::source_level_start(3);
        assert!(result.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert!(!result.record_player_event_0x1d(4, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(result.source_player_slot(), 3);
        assert_eq!(result.raw_word(), SECRET_BANISHMENT_RESULT);
        assert!(result.is_secret_banishment_for(3));
    }

    #[test]
    fn new_level_reset_is_zero_and_clones_preserve_the_source_word() {
        let mut source_session = SourcePlayerResult::source_level_start(1);
        assert!(source_session.record_player_event_0x1d(1, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(source_session, source_session.clone());

        let next_source_session = SourcePlayerResult::source_level_start(1);
        assert_eq!(next_source_session.raw_word(), 0);
        assert_eq!(source_session.raw_word(), SECRET_BANISHMENT_RESULT);
    }
}
