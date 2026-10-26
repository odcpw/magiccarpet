/// The source `Type_str_164::word_0x26_38` matrix for one player slot.
///
/// Level 1's sealed player record omits this embedded field. A newly
/// initialized remc2 player slot still has an authoritative value: `sub_53160`
/// calls `clean_x_D41A0_BYTEARRAY_0_0x2BDE`, whose 2,124-byte `memset` clears
/// the complete player row, including the matrix. This carrier represents
/// that runtime state; it is not a projection fallback for an unknown row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceKillScoreMatrix {
    words: [i16; 8],
}

impl SourceKillScoreMatrix {
    /// Build the omitted matrix from remc2's fresh player-slot constructor.
    #[must_use]
    pub(super) const fn from_new_source_player_slot() -> Self {
        Self { words: [0; 8] }
    }

    #[must_use]
    pub(super) const fn words(&self) -> &[i16; 8] {
        &self.words
    }

    /// Apply remc2's signed 16-bit increment for the victim's color.
    /// Returns false for colors outside the eight source player slots.
    pub(super) fn increment_for_color(&mut self, color: usize) -> bool {
        let Some(score) = self.words.get_mut(color) else {
            return false;
        };
        *score = score.wrapping_add(1);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sha2::{Digest, Sha256};

    use super::super::{
        OwnerPackageProfile, OwnerPackageSchema, parse_entities, parse_players, parse_runtime,
    };
    use super::SourceKillScoreMatrix;

    #[test]
    fn fresh_source_player_matrix_counts_valid_colors_and_rejects_out_of_range() {
        let mut scores = SourceKillScoreMatrix::from_new_source_player_slot();
        assert_eq!(scores.words(), &[0; 8]);

        assert!(scores.increment_for_color(3));
        assert_eq!(scores.words()[3], 1);

        let before = *scores.words();
        assert!(!scores.increment_for_color(8));
        assert_eq!(*scores.words(), before);
    }

    #[test]
    fn level_one_campaign_snapshot_has_initialized_scores_on_active_local_player() {
        let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-v3-current")
            },
            PathBuf::from,
        );
        let players = parse_players(
            &std::fs::read(package.join("players.bin")).expect("Level 1 players.bin"),
            OwnerPackageProfile::Level1,
            OwnerPackageSchema::V3,
        )
        .expect("actual ordinary Level 1 player rows");
        let entities = parse_entities(
            &std::fs::read(package.join("entities.bin")).expect("Level 1 entities.bin"),
        )
        .expect("actual ordinary Level 1 entities");
        let runtime = parse_runtime(
            &std::fs::read(package.join("runtime.bin")).expect("Level 1 runtime.bin"),
        )
        .expect("actual ordinary Level 1 runtime");
        let local_player = &players[usize::from(runtime.local_player_slot)];
        let local_entity = &entities[usize::from(local_player.entity_index)];
        assert!(!local_player.is_ai);
        assert!(local_entity.active);
        assert_eq!(local_entity.class, 3);
        assert_eq!(local_player.type_str_164_auxiliary(), None);
        assert_eq!(local_player.source_kill_scores(), Some([0; 8]));

        let mut before_hasher = Sha256::new();
        local_player.append_omitted_state_hash(&mut before_hasher);
        let mut scored_player = local_player.clone();
        assert!(scored_player.increment_kill_score_for_color(2));
        assert_eq!(
            scored_player.source_kill_scores(),
            Some([0, 0, 1, 0, 0, 0, 0, 0])
        );
        let mut after_hasher = Sha256::new();
        scored_player.append_omitted_state_hash(&mut after_hasher);
        assert_ne!(before_hasher.finalize(), after_hasher.finalize());
        assert_eq!(
            scored_player.clone().source_kill_scores(),
            scored_player.source_kill_scores()
        );
    }
}
