//! Source state outside the compact owner-package player record.

use super::*;

/// The shared `unk_F42B0x` A4 target installed by `NewEvent_4A050` for
/// nonplayer entities. Ordinary spell acquisition and carry do not rebind it
/// to the holder's parent. Player A4 fields have their own distinct owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSpecialState {
    pub protection_ticks: u16,
    pub visibility_mode: u8,
}

impl GlobalSpecialState {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub(super) fn append_hash(self, hasher: &mut Sha256) {
        if self != Self::default() {
            hasher.update(b"source-global-special-state-v1");
            hasher.update(self.protection_ticks.to_le_bytes());
            hasher.update([self.visibility_mode]);
        }
    }
}

impl Level2SharedSpellResetSession {
    #[must_use]
    pub fn global_special_state(&self) -> GlobalSpecialState {
        self.global_special_state
    }

    pub(crate) fn apply_source_global_context(&mut self, context: GlobalSpecialState) {
        self.global_special_state = context;
    }

    pub(super) fn physical_spell_profile(
        &self,
        family: u8,
        tier: u8,
        owner: u16,
        castle_stage: Option<i32>,
    ) -> Option<Level2SetSpellProfile> {
        let mut profile = level2_set_spell_profile(family, tier)?;
        if family == CASTLE_SPELL_FAMILY {
            let destroyed = self
                .players
                .iter()
                .find(|player| player.entity_index == owner)
                .is_some_and(|player| player.castle_destroyed != 0);
            profile.max_mana = castle_mana_cost(profile.max_mana, tier, castle_stage, destroyed);
            profile.mana = if profile.word_0x30 != 0 {
                profile.max_mana / i32::from(profile.word_0x30)
            } else {
                profile.max_mana
            };
        }
        if self.free_spell_usage {
            profile.mana_regen = 0;
            profile.mana = 1;
        }
        Some(profile)
    }

    pub(super) fn castle_stage_for_owner(economy: &Level2EconomyState, owner: u16) -> Option<i32> {
        economy
            .castle()
            .filter(|castle| {
                castle.owner_entity_index == owner
                    && economy.player_castle_entity_index() == castle.source_entity_index
            })
            .map(|castle| i32::from(castle.level))
    }

    pub(super) fn reprice_acquired_castle_holder(
        &mut self,
        economy: &Level2EconomyState,
        outcome: SpellHolderAcquisitionOutcome,
    ) {
        if outcome.family != CASTLE_SPELL_FAMILY {
            return;
        }
        let Some(holder) = self.pickup(outcome.source_slot) else {
            return;
        };
        let tier = holder.level_gate.cast_unsigned();
        let stage = Self::castle_stage_for_owner(economy, outcome.parent_entity_index);
        let Some(profile) =
            self.physical_spell_profile(2, tier, outcome.parent_entity_index, stage)
        else {
            return;
        };
        if let Some(holder) = self.pickup_mut(outcome.source_slot) {
            holder.apply_nyphur_spell_profile(profile, tier);
        }
        for state in self
            .acquisitions
            .iter_mut()
            .filter(|state| state.source_slot == outcome.source_slot)
        {
            state.max_mana = profile.max_mana;
            state.mana = profile.mana;
            state.mana_regen = profile.mana_regen;
        }
    }
}

/// Level.cpp GetSpellManaCost_6D710. A missing Castle uses the table cost;
/// a linked stage-zero Castle follows the stage table and Q8 scaling instead.
fn castle_mana_cost(base: i32, tier: u8, stage: Option<i32>, destroyed: bool) -> i32 {
    let Some(stage) = stage else {
        return base.wrapping_add(if destroyed { 3_000 } else { 0 });
    };
    let mut cost: i32 = match stage {
        0 => 1_000,
        1 => 10_000,
        2 => 20_000,
        3 => 40_000,
        4 => 80_000,
        5 => 160_000,
        6 => 320_000,
        _ => 300_000_000,
    };
    if stage >= 7 {
        return cost;
    }
    let scale = match tier {
        1 => 320_i32,
        2 => 384_i32,
        _ => return cost.wrapping_add(if stage == 0 && destroyed { 3_000 } else { 0 }),
    };
    let product = cost.wrapping_mul(scale);
    // my_sign32 returns -1 or 0, not Rust's -1/0/1 signum.
    let sign = product >> 31;
    cost = product.wrapping_sub(sign << 8).wrapping_add(sign) >> 8;
    cost.wrapping_add(if stage == 0 && destroyed { 3_000 } else { 0 })
}

impl Mc2LiveState {
    pub(crate) fn source_spell_profile(
        &self,
        family: u8,
        tier: u8,
        owner: u16,
    ) -> Option<Level2SetSpellProfile> {
        let stage =
            Level2SharedSpellResetSession::castle_stage_for_owner(&self.economy.economy, owner)
                .or_else(|| {
                    let player = self
                        .shared_spells
                        .players
                        .iter()
                        .find(|player| player.entity_index == owner)?;
                    let slot = u16::from_le_bytes([player.record[42], player.record[43]]);
                    self.economy
                        .entities
                        .get(usize::from(slot))
                        .filter(|row| slot != 0 && row.active)
                        .map(EntitySnapshot::source_dword_10)
                });
        self.shared_spells
            .physical_spell_profile(family, tier, owner, stage)
    }

    pub(crate) fn set_player_castle_destroyed(&mut self, owner: u16, value: u8) {
        for player in self
            .shared_spells
            .players
            .iter_mut()
            .chain(self.core.players.iter_mut())
        {
            if player.entity_index == owner {
                player.castle_destroyed = value;
            }
        }
        if self.shared_spells.local_player.entity_index == owner {
            self.shared_spells.local_player.castle_destroyed = value;
        }
    }

    /// sub60780 forces a cold SetSpell while retaining W; sub6D8B0 uses
    /// the ordinary SetSpell queue branch when the holder remains active.
    pub(crate) fn refresh_castle_spell_profile(
        &mut self,
        owner: u16,
        force: bool,
        stage_override: Option<i32>,
    ) {
        // sub60810 passes no holder to sub60780 while the owner is dying
        // or exiting. sub6D8B0's ordinary SetSpell path has no such guard.
        if force
            && self
                .economy
                .entities
                .get(usize::from(owner))
                .is_none_or(|owner| owner.action > 1)
        {
            return;
        }
        let Some(player) = self
            .shared_spells
            .players
            .iter()
            .find(|player| player.entity_index == owner)
        else {
            return;
        };
        let slot = player.enabled_spell_holders[2];
        let tier = player.family_selected_tiers[2].clamp(0, 2).cast_unsigned();
        let profile = if let Some(stage) = stage_override {
            self.shared_spells
                .physical_spell_profile(2, tier, owner, Some(stage))
        } else {
            self.source_spell_profile(2, tier, owner)
        };
        let Some(profile) = profile else {
            return;
        };
        let apply = |holder: &mut EntitySnapshot| {
            let active = holder.active_value;
            let word = [holder.record[50], holder.record[51]];
            if force {
                holder.active_value = 0;
                holder.record[50..52].fill(0);
            }
            holder.apply_nyphur_spell_profile(profile, tier);
            if force {
                holder.active_value = active;
                holder.record[50..52].copy_from_slice(&word);
            }
        };
        if let Some(holder) = self
            .shared_spells
            .pickup_mut(slot)
            .filter(|holder| holder.active && holder.model == 2)
        {
            apply(holder);
        }
        if let Some(holder) = self
            .economy
            .entities
            .get_mut(usize::from(slot))
            .filter(|holder| slot != 0 && holder.active && holder.class == 15 && holder.model == 2)
        {
            apply(holder);
        }
    }

    /// PlayerEvents opcode42 runs before UpdateEntities. The Castle's normal
    /// callback, same-pass publication and following-pass reclaim still own
    /// destruction; this command only makes its source life negative.
    pub(crate) fn request_castle_stage_removal(&mut self) {
        let owner = self.core.state.entity_index;
        let Some(castle) = self.economy.castle().filter(|castle| {
            castle.active
                && castle.owner_entity_index == owner
                && self.economy.player_castle_entity_index() == castle.source_entity_index
        }) else {
            return;
        };
        if castle.level == 1 {
            self.set_player_castle_destroyed(owner, 1);
        }
        self.economy
            .economy
            .castle
            .as_mut()
            .expect("validated owned Castle")
            .life = -1;
        self.economy.entities[usize::from(castle.source_entity_index)].set_life(-1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn castle_cost_uses_source_stage_scaling_and_destruction_surcharge() {
        assert_eq!(castle_mana_cost(2_000, 2, None, false), 2_000);
        assert_eq!(castle_mana_cost(2_000, 2, None, true), 5_000);
        assert_eq!(castle_mana_cost(2_000, 2, Some(0), true), 4_500);
        assert_eq!(castle_mana_cost(2_000, 2, Some(1), true), 15_000);
        assert_eq!(castle_mana_cost(1_250, 1, Some(1), false), 12_500);
        assert_eq!(castle_mana_cost(1_000, 0, Some(6), false), 320_000);
        assert_eq!(castle_mana_cost(2_000, 2, Some(7), true), 300_000_000);
    }
}
