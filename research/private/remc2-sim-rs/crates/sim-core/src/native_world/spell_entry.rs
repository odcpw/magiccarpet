//! Source InitialiseSpells/sub_5CF40 admission before local holder births.

use super::*;

impl Level2SharedSpellResetSession {
    /// Ordinary single-player continuation uses retained ownership, not the
    /// destination map's direct-start gifts. A blocked family stays learned
    /// while receiving no holder in this mission.
    pub(crate) fn materialize_continuation_spell_entry(
        &mut self,
        economy: &mut Level2EconomyState,
        learned: &[u8],
        blocked: &[u8; PLAYER_SPELL_COUNT],
    ) -> Result<()> {
        let mut ownership = [0_u8; PLAYER_SPELL_COUNT];
        for &family in learned {
            let Some(flag) = ownership.get_mut(usize::from(family)) else {
                return Err(NativeWorldError::new("entry spell family is out of range"));
            };
            if *flag != 0 {
                return Err(NativeWorldError::new("entry spell families are duplicated"));
            }
            *flag = 1;
        }
        if self
            .local_player
            .enabled_spell_holders
            .iter()
            .any(|index| *index != 0)
        {
            return Err(NativeWorldError::new(
                "continuation admission must run before local spell-holder allocation",
            ));
        }
        let player_index = self
            .players
            .iter()
            .position(|player| player.entity_index == self.local_player.entity_index)
            .ok_or_else(|| NativeWorldError::new("entry local player row is absent"))?;

        // InitialiseSpells_54A50 clears mission acquisitions and selected
        // tiers before sub_5CF40 allocates in numeric family order.
        let player = &mut self.local_player;
        player.spell_enabled_flags = ownership;
        player.acquired_spell_flags.fill(0);
        player.family_selected_tiers.fill(0);
        player.left_spell_slot_raw = -1;
        player.right_spell_slot_raw = -1;
        player.left_spell_slot = 0;
        player.right_spell_slot = 0;
        player.left_subspell_index = 0;
        player.right_subspell_index = 0;
        player.selected_subspell_index = 0;
        player.record[450..476].copy_from_slice(&ownership);
        player.record[476..502].fill(0);
        player.record[528..554].fill(0);
        player.record[554..558].fill(0xff);
        player.record[558..560].fill(0);
        player.record[562] = 0;
        self.players[player_index].clone_from(player);
        let admitted = (0..PLAYER_SPELL_COUNT)
            .filter(|family| ownership[*family] != 0 && blocked[*family] == 0)
            .map(|family| (family as u8, 0))
            .collect::<Vec<_>>();
        self.materialize_carried_spell_families(economy, &admitted)?;

        // Hand selection is the menu-order loop in InitialiseSpells, distinct
        // from numeric holder construction order in sub_5CF40.
        let mut hands = SOURCE_SPELL_INDEX_MAP.iter().copied().filter(|family| {
            let index = usize::from(*family);
            ownership[index] != 0 && blocked[index] == 0
        });
        let left = hands.next().map_or(-1, i16::from);
        let right = hands.next().map_or(-1, i16::from);
        let player = &mut self.local_player;
        player.left_spell_slot_raw = left;
        player.right_spell_slot_raw = right;
        player.left_spell_slot = left.max(0) as u8;
        player.right_spell_slot = right.max(0) as u8;
        player.acquired_spell_flags.fill(0);
        player.record[476..502].fill(0);
        player.record[554..556].copy_from_slice(&left.to_le_bytes());
        player.record[556..558].copy_from_slice(&right.to_le_bytes());
        if let Some(ui_index) = SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| i16::from(*family) == left)
        {
            player.selected_spell_ui_index = ui_index as u8;
            player.record[561] = ui_index as u8;
        }
        for state in &mut self.acquisitions {
            state.learned = 0;
        }
        self.players[player_index].clone_from(player);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{
        CampaignSession,
        dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
        spec::{CampaignDefinition, CampaignSelection},
    };

    fn level_two_package() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-block32-campaign-v2")
    }

    #[test]
    fn continuation_admits_only_learned_unblocked_spells_before_allocation() {
        let definition = CampaignDefinition::preflight(
            level_two_package(),
            CampaignSelection::LEVEL2,
            &CAMPAIGN_BEHAVIOR_REGISTRY,
        )
        .unwrap();
        let inspection = NativeWorld::inspect_campaign_definition(&definition).unwrap();
        let mut shared = Level2SharedSpellResetSession::from_campaign_parts(
            inspection.entities(),
            inspection.players(),
            inspection.runtime(),
        )
        .unwrap();
        let mut economy = Level2EconomyState::from_parts(
            SessionTerrain::from_inspection(&inspection),
            inspection.entities(),
            shared.player_entity.mana,
            shared.player_entity.model,
            0,
            0,
            inspection.runtime().random_state,
        )
        .unwrap();
        let mut blocked = [0; PLAYER_SPELL_COUNT];
        blocked[6] = 1;
        shared
            .materialize_continuation_spell_entry(&mut economy, &[12, 0, 6, 11], &blocked)
            .unwrap();

        let player = shared.local_player();
        assert_eq!(
            player.spell_enabled_flags[6], 1,
            "blocked ownership survives"
        );
        assert_eq!(
            player.enabled_spell_holders[6], 0,
            "blocked spell has no holder"
        );
        assert_eq!(
            player.enabled_spell_holders[2], 0,
            "map Castle gift is not carried"
        );
        assert_eq!(player.acquired_spell_flags, [0; PLAYER_SPELL_COUNT]);
        assert_eq!(
            (player.left_spell_slot_raw, player.right_spell_slot_raw),
            (0, 11)
        );
        assert_eq!(
            shared
                .dynamic_pickups
                .iter()
                .map(|row| row.model)
                .collect::<Vec<_>>(),
            [0, 11, 12]
        );
        assert!(shared.dynamic_pickups.iter().all(|row| {
            row.actual_speed == 16
                && row.special_relation == EntitySpecialRelation::GlobalSpecialState
                && row.record_bytes()[97] == 1
                && row.record_bytes()[168..176] == [2, 0, 0, 0, 1, 0, 0, 0]
        }));
    }

    #[test]
    fn direct_starting_holders_publish_source_cold_state_consistently() {
        let campaign =
            CampaignSession::open(level_two_package(), CampaignSelection::LEVEL2).unwrap();
        let shared = campaign.simulation().shared_spells();
        assert!(!shared.dynamic_pickups.is_empty());
        for holder in &shared.dynamic_pickups {
            assert_eq!(holder.actual_speed, 16);
            assert_eq!(holder.record_bytes()[134..136], [16, 0]);
            assert_eq!(holder.record_bytes()[97], 1);
            assert_eq!(
                holder.special_relation,
                EntitySpecialRelation::GlobalSpecialState
            );
            assert_eq!(holder.record_bytes()[168..176], [2, 0, 0, 0, 1, 0, 0, 0]);
        }
    }
}
