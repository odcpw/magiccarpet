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
    /// Publish a source award at the same boundary as its authoritative
    /// progression change, before later callbacks read core or fire state.
    pub(crate) fn publish_spell_progression_family(
        &self,
        players: &mut [PlayerSnapshot],
        fire: Option<&mut SpellFireContext>,
        owner: u16,
        family: u8,
    ) {
        if let Some(source) = self
            .players
            .iter()
            .find(|player| player.entity_index == owner)
        {
            for player in players
                .iter_mut()
                .filter(|player| player.entity_index == owner)
            {
                player.sync_spell_progression_family_from(source, usize::from(family));
            }
        }
        if let Some(fire) = fire
            && fire.caster.entity_index == owner
        {
            fire.spell_experience[usize::from(family)] = self.spell_experience[usize::from(family)];
            fire.spell_levels[usize::from(family)] = self.spell_levels[usize::from(family)];
        }
    }

    /// Source A4 feedback for accepted damage, Duel and resolved Steal.
    pub(super) fn set_player_hit_feedback(&mut self) {
        self.set_player_hit_frame_time(4);
        self.player_life_regen_cooldown = 16;
        self.player_hit_effect_cooldown = 64;
    }

    /// Apply the directional mailbox feedback to the authoritative local row
    /// and its shared spell projections before continuing ordinary upkeep.
    pub(super) fn apply_directional_feedback(
        &mut self,
        actor: &mut EntitySnapshot,
        duel_consumed: bool,
        steal_consumed: bool,
        steal_attacker_resolved: bool,
    ) -> bool {
        if !actor.active
            || actor.class != 3
            || actor.index != self.player_entity.index
            || actor.life < 0
        {
            return false;
        }
        if duel_consumed || (steal_consumed && steal_attacker_resolved) {
            self.set_player_hit_feedback();
        }
        if duel_consumed || steal_consumed {
            actor.apply_source_sub_5ef70();
            self.sync_player_entity_from_authority(actor);
            self.sync_typed_spell_parent_caches_from_player_entity();
        }
        true
    }

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
            .castle_for_owner(owner)
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
    /// Project only generation-validated Castle links into the compact A4
    /// records read by player and rival spell consumers.
    pub(crate) fn sync_castle_player_relations(&mut self) {
        let economy = &self.economy.economy;
        let entities = &self.economy.entities;
        for player in self
            .core
            .players
            .iter_mut()
            .chain(self.shared_spells.players.iter_mut())
            .chain(std::iter::once(&mut self.shared_spells.local_player))
        {
            let linked_slot = economy
                .castle_for_owner(player.entity_index)
                .map(|castle| castle.source_entity_index);
            // Ordinary packages can already contain a prebuilt Castle.
            // Preserve its real imported relation until the Castle owner
            // adopts that source carrier; dynamic roots never use a raw row.
            let imported_slot = i16::from_le_bytes([player.record[42], player.record[43]]);
            let imported = u16::try_from(imported_slot).ok().filter(|slot| {
                *slot != 0
                    && entities.get(usize::from(*slot)).is_some_and(|row| {
                        row.active
                            && (row.class, row.model) == (3, 2)
                            && row.source_id == player.entity_index
                    })
            });
            let slot = linked_slot.or(imported).unwrap_or(0);
            player.record[42..44].copy_from_slice(&slot.to_le_bytes());
        }
    }

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
                    let slot =
                        u16::try_from(i16::from_le_bytes([player.record[42], player.record[43]]))
                            .ok()?;
                    self.economy
                        .entities
                        .get(usize::from(slot))
                        .filter(|row| {
                            slot != 0
                                && row.active
                                && (row.class, row.model) == (3, 2)
                                && row.source_id == owner
                        })
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

    /// sub_60780 reapplies the physical holder's current tier, independently
    /// of the human menu selector. Resolve the same live owner/holder pair
    /// that ordinary AI inventory lookups consume.
    fn current_castle_holder_tier(&self, owner: u16) -> Option<u8> {
        let player = self.shared_spells.players.iter()
            .find(|player| player.entity_index == owner)?;
        let slot = player.enabled_spell_holders[2];
        let owner_handle = self.economy.handle(owner)?;
        let holder_handle = self.economy.handle(slot)?;
        if player.is_ai
            && self.nyphur_dispatcher.bound_live_holder(owner_handle, player, &self.economy, 2)
                != Some(holder_handle)
        {
            return None;
        }
        let holder = self.economy.entities.get(usize::from(slot))
            .filter(|holder| holder.active && holder.class == 15 && holder.model == 2
                && holder.source_spell_parent_entity_index() == owner)?;
        let tier = u8::try_from(holder.level_gate).ok()?;
        (tier <= 2).then_some(tier)
    }

    /// Run the Castle callback with its actual player inventory consumer at
    /// `sub_605E0`, before the root is hidden and the remaining handler tail.
    pub(crate) fn step_castle_action_with_player_state(
        &mut self,
        castle_slot: u16,
        reset_free_slots: &[u16],
        remove_rival_castle_spell: bool,
    ) -> bool {
        let final_profile = self.economy.castle_at(castle_slot).and_then(|castle| {
            if castle.level == 0 {
                return None;
            }
            let owner = castle.source_id_entity_index;
            if self.economy.entities.get(usize::from(owner))?.action > 1 {
                return None;
            }
            let tier = self.current_castle_holder_tier(owner)?;
            self.shared_spells
                .physical_spell_profile(2, tier, owner, Some(0))
                .map(|profile| (tier, profile))
        });
        let Self {
            economy: arena,
            core,
            terrain,
            shared_spells,
            nyphur_dispatcher,
            ..
        } = self;
        let crate::campaign::source_arena::SourceArena {
            economy, entities, ..
        } = arena;
        let mut consumer = CastlePlayerConsumer {
            entities,
            shared_spells,
            core,
            nyphur_dispatcher,
        };
        economy.step_castle_action_with_final_loss(
            castle_slot,
            terrain,
            reset_free_slots,
            &mut |economy, castle| {
                consumer.final_loss(economy, castle, remove_rival_castle_spell, final_profile);
            },
        )
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
        let tier = if force {
            let Some(tier) = self.current_castle_holder_tier(owner) else {
                return;
            };
            tier
        } else {
            // sub_6D8B0 refreshes the human owner's selected tier through
            // ordinary SetSpell, including its active-holder queue branch.
            player.family_selected_tiers[2].clamp(0, 2).cast_unsigned()
        };
        let profile = if let Some(stage) = stage_override {
            self.shared_spells
                .physical_spell_profile(2, tier, owner, Some(stage))
        } else {
            self.source_spell_profile(2, tier, owner)
        };
        let Some(profile) = profile else {
            return;
        };
        apply_castle_spell_profile(
            &mut self.economy.entities,
            &mut self.shared_spells,
            owner,
            force,
            tier,
            profile,
        );
    }

    /// PlayerEvents opcode42 runs before UpdateEntities. The Castle's normal
    /// callback, same-pass publication and following-pass reclaim still own
    /// destruction; this command only makes its source life negative.
    pub(crate) fn request_castle_stage_removal(&mut self) {
        let owner = self.core.state.entity_index;
        let Some(castle) = self.economy.castle_for_owner(owner).copied() else {
            return;
        };
        if castle.level == 1 {
            self.set_player_castle_destroyed(owner, 1);
        }
        self.economy
            .economy
            .castle_at_mut(castle.source_entity_index)
            .expect("validated owned Castle")
            .life = -1;
        self.economy.entities[usize::from(castle.source_entity_index)].set_life(-1);
    }
}

/// Borrowed owners needed at the Castle's inventory boundary. The economy
/// passes its current state into the hook; no second arena or inventory is held.
struct CastlePlayerConsumer<'a> {
    entities: &'a mut [EntitySnapshot],
    shared_spells: &'a mut Level2SharedSpellResetSession,
    core: &'a mut crate::campaign::state::CanonicalRuntimeState,
    nyphur_dispatcher: &'a mut level2_nyphur::RivalWizardRoster,
}

impl CastlePlayerConsumer<'_> {
    fn final_loss(
        &mut self,
        economy: &Level2EconomyState,
        castle: Level2CastleState,
        remove_rival_castle_spell: bool,
        final_profile: Option<(u8, Level2SetSpellProfile)>,
    ) {
        let owner = castle.source_id_entity_index;
        if owner == 0
            || economy.slot_generation(owner) != castle.owner_slot_generation
            || self
                .entities
                .get(usize::from(owner))
                .is_none_or(|row| !row.active || row.class != 3)
        {
            return;
        }
        // sub_60810 refreshes the final stage's holder before sub_605E0
        // retires it. Keep that profile write ahead of inventory removal.
        if let Some((tier, profile)) = final_profile {
            apply_castle_spell_profile(
                self.entities,
                self.shared_spells,
                owner,
                true,
                tier,
                profile,
            );
        }
        let remove_spell =
            remove_rival_castle_spell && self.entities[usize::from(owner)].model == 1;
        let holder_slot = self
            .shared_spells
            .players
            .iter()
            .find(|player| player.entity_index == owner)
            .map_or(0, |player| player.enabled_spell_holders[2]);
        if remove_spell
            && let Some(holder) = self
                .entities
                .get_mut(usize::from(holder_slot))
                .filter(|holder| {
                    holder_slot != 0
                        && holder.active
                        && (holder.class, holder.model) == (15, 2)
                        && holder.source_spell_parent_entity_index() == owner
                })
        {
            holder.disable_drawing();
            self.shared_spells.sync_pickup_from_authority(holder);
        }
        for player in self
            .shared_spells
            .players
            .iter_mut()
            .chain(self.core.players.iter_mut())
            .chain(std::iter::once(&mut self.shared_spells.local_player))
            .filter(|player| player.entity_index == owner)
        {
            player.record[42..44].fill(0);
            if remove_spell {
                player.enabled_spell_holders[2] = 0;
                player.record[272..274].fill(0);
            }
        }
        if remove_spell {
            for state in &mut self.shared_spells.acquisitions {
                if state.owner_entity_index == owner && state.family == 2 {
                    state.holder_relation = 0;
                }
            }
            if self.shared_spells.local_player.entity_index == owner
                && let Some(fire) = self.core.context.spell_fire.as_mut()
            {
                fire.enabled_holder_indices = self.shared_spells.enabled_spell_holder_indices();
            }
        }
        let handle = crate::campaign::source_arena::SourceEntityHandle {
            source_slot: owner,
            generation: economy.slot_generation(owner),
        };
        self.nyphur_dispatcher.with_owner_mut(handle, |dispatcher| {
            dispatcher.apply_final_castle_loss(owner, remove_spell);
        });
    }
}

fn apply_castle_spell_profile(
    entities: &mut [EntitySnapshot],
    shared_spells: &mut Level2SharedSpellResetSession,
    owner: u16,
    force: bool,
    tier: u8,
    profile: Level2SetSpellProfile,
) {
    let Some(player) = shared_spells
        .players
        .iter()
        .find(|player| player.entity_index == owner)
    else {
        return;
    };
    let slot = player.enabled_spell_holders[2];
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
    if let Some(holder) = shared_spells
        .pickup_mut(slot)
        .filter(|holder| holder.active && holder.model == 2)
    {
        apply(holder);
    }
    if let Some(holder) = entities
        .get_mut(usize::from(slot))
        .filter(|holder| slot != 0 && holder.active && holder.class == 15 && holder.model == 2)
    {
        apply(holder);
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
