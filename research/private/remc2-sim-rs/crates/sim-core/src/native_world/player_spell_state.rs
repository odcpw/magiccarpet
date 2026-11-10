//! Source state outside the compact owner-package player record.

use super::*;
use crate::campaign::source_arena::SourceArena;

/// The Castle callback borrows source records for its two mana scans and
/// player records for final loss. Keeping those operations in one narrow
/// consumer preserves their source call order without aliasing the arena.
pub(crate) trait CastleSourceConsumer {
    fn mana_rows(&self, economy: &Level2EconomyState)
        -> Vec<crate::campaign::source_arena::SourceManaRow>;
    fn hide_mana_source(&mut self, economy: &mut Level2EconomyState, slot: u16);
    fn final_loss(&mut self, economy: &mut Level2EconomyState, castle: Level2CastleState);
}

// Pure economy checks have no raw actor domain. Their existing callback
// remains a bounded test adapter; production uses CastlePlayerConsumer below.
#[cfg(test)]
impl<F: FnMut(&mut Level2EconomyState, Level2CastleState)> CastleSourceConsumer for F {
    fn mana_rows(&self, economy: &Level2EconomyState)
        -> Vec<crate::campaign::source_arena::SourceManaRow>
    {
        economy.possession_guidance_source_slots.iter().filter_map(|slot| {
            let sphere = economy.spheres().iter()
                .find(|row| row.active && row.source_entity_index == *slot)?;
            Some(crate::campaign::source_arena::SourceManaRow {
                source_slot: *slot, position: sphere.position,
                player_entity_index: sphere.player_entity_index,
                control_flags: sphere.control_flags, mana: sphere.mana,
                collision_yaw: sphere.collision_yaw, collision_pitch: sphere.collision_pitch,
                collision_roll: sphere.collision_roll, collision_fov: sphere.collision_fov,
            })
        }).collect()
    }

    fn hide_mana_source(&mut self, economy: &mut Level2EconomyState, slot: u16) {
        if let Some(sphere) = economy.spheres.iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            sphere.pending_delete = true;
            sphere.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }

    fn final_loss(&mut self, economy: &mut Level2EconomyState, castle: Level2CastleState) {
        self(economy, castle);
    }
}

/// The shared `unk_F42B0x` A4 target installed by `NewEvent_4A050` for
/// nonplayer entities. Ordinary spell acquisition and carry do not rebind it
/// to the holder's parent. Player A4 fields have their own distinct owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalSpecialState {
    pub protection_ticks: u16,
    pub visibility_mode: u8,
    /// Source A4 byte197 written by a nonplayer collector's 60EA0 damage tail.
    /// This global A4 is distinct from the human player's damage feedback.
    #[serde(default)]
    pub damage_flash_ticks: u8,
    /// Aim words on the shared A4 selected by NewEvent, distinct from a player A4.
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_angle")]
    pub input_yaw: i16,
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_angle")]
    pub input_pitch: i16,
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_auxiliary")]
    pub cast_auxiliary: u8,
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_slot")]
    pub castle_entity_index: u16,
    /// Current shared A4 words0xc/0xe, consumed by GetScroll_69DB0.
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_angle")]
    pub speed: i16,
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_angle")]
    pub speed_changed: i16,
    /// Source5EF70 may target a current model0 whose A4 is NewEvent's
    /// shared context, rather than a player-specific A4.
    #[serde(default, skip_serializing_if = "GlobalSpecialState::zero_angle")]
    pub music_countdown: i16,
}

impl GlobalSpecialState {
    fn zero_angle(value: &i16) -> bool { *value == 0 }
    fn zero_auxiliary(value: &u8) -> bool { *value == 0 }
    fn zero_slot(value: &u16) -> bool { *value == 0 }
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub(super) fn append_hash(self, hasher: &mut Sha256) {
        if self.music_countdown != 0 {
            hasher.update(b"source-global-a4-music-countdown-v1");
            hasher.update(self.music_countdown.to_le_bytes());
        }
        if self.speed != 0 || self.speed_changed != 0 {
            hasher.update(b"source-global-speed-input-v1");
            hasher.update(self.speed.to_le_bytes());
            hasher.update(self.speed_changed.to_le_bytes());
        }
        if self.protection_ticks != 0 || self.visibility_mode != 0 {
            hasher.update(b"source-global-special-state-v1");
            hasher.update(self.protection_ticks.to_le_bytes());
            hasher.update([self.visibility_mode]);
        }
        if self.damage_flash_ticks != 0 {
            hasher.update(b"source-global-collector-damage-flash-v1");
            hasher.update([self.damage_flash_ticks]);
        }
        if self.input_yaw != 0 || self.input_pitch != 0 || self.cast_auxiliary != 0 {
            hasher.update(b"source-global-cast-input-v1");
            hasher.update(self.input_yaw.to_le_bytes());
            hasher.update(self.input_pitch.to_le_bytes());
            hasher.update([self.cast_auxiliary]);
        }
        if self.castle_entity_index != 0 {
            hasher.update(b"source-global-castle-relation-v1");
            hasher.update(self.castle_entity_index.to_le_bytes());
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

    pub(crate) fn record_castle_collector_damage(&mut self) {
        self.global_special_state.damage_flash_ticks = 4;
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

    pub(crate) fn source_spell_profile(
        &self,
        arena: &SourceArena,
        family: u8,
        tier: u8,
        owner: u16,
    ) -> Option<Level2SetSpellProfile> {
        let stage = Self::castle_stage_for_owner(&arena.economy, owner).or_else(|| {
            let player = self.players.iter().find(|player| player.entity_index == owner)?;
            let slot = u16::try_from(i16::from_le_bytes([player.record[42], player.record[43]])).ok()?;
            arena.entities.get(usize::from(slot))
                .filter(|row| slot != 0 && row.active && (row.class, row.model) == (3, 2)
                    && row.source_id == owner)
                .map(EntitySnapshot::source_dword_10)
        });
        self.physical_spell_profile(family, tier, owner, stage)
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
    /// `sub_60F00` writes each wizard's maxMana before the ordered entity
    /// callbacks. Claim, transport and deposit only move counted mana; this
    /// publication neither grants current mana nor performs regeneration.
    pub(crate) fn publish_source_mana_capacities(&mut self) {
        for player in &self.core.players {
            let slot = player.entity_index;
            let Some(&capacity) = self.economy.owner_max_mana.get(&slot) else { continue };
            let Some(handle) = self.economy.handle(slot) else { continue };
            let Some(actor) = self.economy.entities.get_mut(usize::from(slot))
                .filter(|actor| actor.active && actor.class == 3 && matches!(actor.model, 0 | 1))
            else { continue };
            actor.max_mana = capacity;
            actor.mana_cost = capacity;
            actor.record[144..148].copy_from_slice(&capacity.to_le_bytes());
            for projected in self.shared_spells.player_entities.iter_mut()
                .chain(std::iter::once(&mut self.shared_spells.player_entity))
                .filter(|projected| projected.index == slot)
            {
                projected.max_mana = capacity;
                projected.mana_cost = capacity;
                projected.record[144..148].copy_from_slice(&capacity.to_le_bytes());
            }
            if let Some(dispatcher) = self.nyphur_dispatcher.get_mut(handle) {
                dispatcher.publish_source_mana_capacity(capacity);
            }
        }
    }

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
        self.shared_spells
            .source_spell_profile(&self.economy, family, tier, owner)
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
        class3_slots: Option<&[u16]>,
        mana_chain: &crate::campaign::source_arena::SourceManaChain,
    ) -> bool {
        let current_class3;
        let class3_slots = if let Some(slots) = class3_slots {
            slots
        } else {
            current_class3 = self.economy.live_castles().map(|castle| castle.source_entity_index)
                .chain(self.economy.entities.iter().filter(|row| row.active && row.class == 3 && row.life >= 0)
                    .map(|row| row.index)).collect::<Vec<_>>();
            &current_class3
        };
        let site_clear = self.economy.castle_at(castle_slot).is_none_or(|castle| {
            castle.action != 5 || castle.build_state != 0 || castle.level == 0
                || self.economy.castle_upgrade_site_checks(castle_slot, &self.terrain, class3_slots, &self.economy.entities)[1]
        });
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
        let prior_castle = self.economy.castle_at(castle_slot).copied();
        let prior_allocations = self.economy.castle_source_allocations();
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
            mana_chain,
            remove_rival_castle_spell,
            final_profile,
        };
        let live = economy.step_castle_action_with_site(
            castle_slot,
            terrain,
            reset_free_slots,
            site_clear,
            &mut consumer,
        );
        // sub_60480/sub_605E0 address the current fixed owner's A4, even
        // when NewEvent has rebound that address to the shared special state.
        if let Some(prior) = prior_castle
            && consumer.entities.get(usize::from(prior.source_id_entity_index))
                .is_some_and(|owner| owner.special_relation == EntitySpecialRelation::GlobalSpecialState)
            && let Some(current) = economy.castle_entry(castle_slot)
        {
            if prior.action == 5 && matches!(prior.build_state, 0 | 3)
                && current.active && current.build_state == 4
            {
                consumer.shared_spells.global_special_state.castle_entity_index = castle_slot;
            }
        }
        let retired_root = economy.castle_entry(castle_slot).copied()
            .filter(|root| !root.active);
        // Construction, controller, overflow and collector constructors all
        // use NewEvent. Retire only their newly allocated generations before
        // the next numeric consumer; existing Castle products stay intact.
        arena.retire_new_castle_source_projections(&prior_allocations);
        if let Some(root) = retired_root {
            arena.retain_castle_target(super::source_target::View {
                slot: root.source_entity_index, source_id: root.source_id_entity_index,
                active: false, class: 0, model: 2, position: root.position,
                life: root.life, control_flags: root.control_flags,
                yaw: None, roll: None, relations: Default::default(),
                collision: Some(crate::combat::area::Shape {
                    vertical_offset: root.shift_yaw, half_width: root.shift_pitch,
                    half_depth: root.shift_roll, half_height: root.shift_fov,
                }),
            });
        }
        live
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
    nyphur_dispatcher: &'a mut creatures::rival_wizards::RivalWizardRoster,
    mana_chain: &'a crate::campaign::source_arena::SourceManaChain,
    remove_rival_castle_spell: bool,
    final_profile: Option<(u8, Level2SetSpellProfile)>,
}

impl CastleSourceConsumer for CastlePlayerConsumer<'_> {
    fn mana_rows(&self, economy: &Level2EconomyState)
        -> Vec<crate::campaign::source_arena::SourceManaRow>
    {
        self.mana_chain.rows(economy, self.entities)
    }

    fn hide_mana_source(&mut self, economy: &mut Level2EconomyState, slot: u16) {
        if economy.live_arena_kind(slot) == Some(Level2DynamicArenaKind::ManaSphere) {
            if let Some(sphere) = economy.spheres.iter_mut()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                sphere.pending_delete = true;
                sphere.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            }
        } else if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            raw.disable_drawing();
        }
    }

    fn final_loss(&mut self, economy: &mut Level2EconomyState, castle: Level2CastleState) {
        if self.entities.get(usize::from(castle.source_id_entity_index))
            .is_some_and(|owner| owner.special_relation == EntitySpecialRelation::GlobalSpecialState)
        {
            // sub_605E0 clears the current A4 unconditionally at final loss.
            self.shared_spells.global_special_state.castle_entity_index = 0;
        }
        self.apply_final_loss(economy, castle, self.remove_rival_castle_spell, self.final_profile);
    }
}

impl CastlePlayerConsumer<'_> {
    fn apply_final_loss(
        &mut self,
        economy: &mut Level2EconomyState,
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
        if self.entities[usize::from(owner)].model != 1 {
            // 605E0's human final-loss branch clears the current addressed
            // holder, then6D880 acts on this Castle caller's own word2C.
            // Unlike construction state2, this root can have word2E=0:
            // SetSpell then cold-applies its clamped family2 profile to the
            // root itself, before 5FCA0's subsequent overflow/cleanup tail.
            let wrote_holder = self.shared_spells.apply_castle_holder_record_update(
                economy, self.entities, &self.core.players,
                self.core.context.spell_fire.as_mut(), owner, false,
            ).ok().flatten().is_some();
            // The addressed holder may alias this root. Read after5F890's
            // word2E write, just as6D880 does through the same fixed address.
            let caller = economy.castle_entry(castle.source_entity_index).copied().unwrap_or(castle);
            if wrote_holder && caller.auxiliary_cooldown != 0 {
                if caller.source_word_2e == 0 {
                    let tier = caller.auxiliary_cooldown.saturating_sub(1).min(2);
                    let parent = self.entities[usize::from(castle.source_entity_index)]
                        .source_spell_parent_entity_index();
                    let stage = Level2SharedSpellResetSession::castle_stage_for_owner(economy, parent);
                    if let Some(profile) = self.shared_spells.physical_spell_profile(2, tier, parent, stage) {
                        let raw = &mut self.entities[usize::from(castle.source_entity_index)];
                        raw.active_value = 0;
                        raw.record[50..52].fill(0);
                        raw.apply_nyphur_spell_profile(profile, tier);
                        if let Some(root) = economy.castle_entry_mut(castle.source_entity_index) {
                            root.mana = profile.mana;
                            root.mana_capacity = profile.max_mana;
                            root.source_word_30 = profile.word_0x30;
                            root.timer = profile.word_0x30;
                        }
                    }
                }
                // If word2E was nonzero, SetSpell only queued on this root;
                // either branch is followed by6D880's unconditional clear.
                self.entities[usize::from(castle.source_entity_index)].record[48..50].fill(0);
                if let Some(root) = economy.castle_entry_mut(castle.source_entity_index) {
                    root.auxiliary_cooldown = 0;
                }
            }
        }
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
    fn castle_final_loss_applies_queued_profile_to_caller_before_reclaim() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
        let campaign = crate::campaign::CampaignSession::open(
            package, crate::campaign::spec::CampaignSelection::LEVEL2,
        ).unwrap();
        let base = campaign.simulation().clone();
        let owner = base.state.entity_index;
        let holder = base.shared_spells().local_player.enabled_spell_holders[2];
        assert_ne!(holder, 0, "the actual owner has its source Castle holder");
        assert!(!base.shared_spells().free_spell_usage);
        let profile = level2_set_spell_profile(2, 2).unwrap();
        for (queued, aliases_caller, caller_word) in [(0, false, 0), (37, false, 0), (37, true, 2)] {
            let mut session = base.clone();
            let position = session.state.position;
            let root = session.economy_mut().build_castle(position, owner).unwrap();
            // Controlled last-stage loss entry. No spell result is injected:
            // the ordinary numeric root callback performs final loss, the
            // current-holder write, caller SetSpell and the overflow tail.
            {
                let mut economy = session.economy_mut();
                let castle = economy.castle_at_mut(root).unwrap();
                castle.level = 1;
                castle.action = 6;
                castle.life = -1;
                castle.timer = 3;
                castle.source_word_2e = caller_word;
                castle.source_word_30 = 3;
                castle.auxiliary_cooldown = queued;
            }
            if aliases_caller {
                let live = &mut session.session.state.live;
                for player in live.core.players.iter_mut()
                    .chain(live.shared_spells.players.iter_mut())
                    .chain(std::iter::once(&mut live.shared_spells.local_player))
                    .filter(|row| row.entity_index == owner)
                {
                    assert!(player.set_enabled_spell_holder_index(2, root));
                }
            }
            let generation = session.economy().slot_generation(root);
            let event_start = session.economy().events().len();
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().all(|event|
                !matches!(event, Mc2DispatchEvent::Error { .. })), "{:?}", session.dispatch_events());
            let raw = &session.entities()[usize::from(root)];
            assert_eq!(session.economy().slot_generation(root), generation);
            assert_eq!(raw.source_word_2c(), 0, "6D880 clears the actual caller's queue");
            assert_eq!(raw.source_word_2e(), 0);
            assert_eq!(raw.source_word_30(), 5,
                "the following 5FCA0 tail overwrites the temporary SetSpell counter");
            let overflow = session.economy().events()[event_start..].iter().filter_map(|event| match event {
                Level2EconomyEvent::ManaSphereSpawned { source_entity_index, mana, .. }
                    if *source_entity_index == root => Some(*mana),
                _ => None,
            }).collect::<Vec<_>>();
            if queued != 0 {
                assert_eq!(raw.level_gate, 2, "SetSpell clamps37-1 to actual Castle tier2");
                assert_eq!((raw.subspell_index, raw.mana_regen, raw.max_mana, raw.mana),
                    (profile.subspell_index, profile.mana_regen, profile.max_mana, 0),
                    "cold SetSpell uses the Castle caller's sentinel parent, not the destroyed owner's price");
                assert_eq!(overflow, [profile.mana],
                    "the following5FD00 must actually construct a sphere from all cold-profile mana at stage0");
            } else {
                assert_eq!(raw.max_mana, 5_000,
                    "source60810 stage0 retains5000 when no caller SetSpell overrides it");
                assert!(overflow.is_empty(), "an empty caller queue creates no cold-profile mana drop");
            }
            if !aliases_caller {
                assert_eq!(session.shared_spells().pickup(holder).unwrap().active_value, 0,
                    "the current addressed holder is released without becoming the profile caller");
            }
            let surviving_root = session.economy().castle_entry(root);
            assert!(surviving_root.is_none_or(|root| !root.active));
        }
    }

    #[test]
    fn alliance_shared_a4_castle_relation_follows_real_build_and_final_loss() {
        let mut session = crate::native_world::tests::level_two_campaign_session();
        let owner = session.state.entity_index;
        let position = session.state.position;
        let root = session.economy_mut().build_castle(position, owner).unwrap();
        {
            let live = &mut session.session.state.live;
            // Controlled current A4 binding; the ordinary Castle callbacks
            // below must publish through that pointer rather than player caches.
            live.economy.entities[usize::from(owner)].special_relation = EntitySpecialRelation::GlobalSpecialState;
            let castle = live.economy.castle_at_mut(root).unwrap();
            castle.action = 5;
            castle.build_state = 3;
        }
        session.step_level2_source_pass(0);
        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "{:?}", session.dispatch_events());
        assert_eq!(session.shared_spells().global_special_state.castle_entity_index, root);
        {
            let live = &mut session.session.state.live;
            live.shared_spells.global_special_state.castle_entity_index = root.wrapping_add(1);
            let castle = live.economy.castle_at_mut(root).unwrap();
            castle.level = 1;
            castle.action = 6;
            castle.life = -1;
            castle.timer = 3;
        }
        session.step_level2_source_pass(0);
        assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })), "{:?}", session.dispatch_events());
        assert_eq!(session.shared_spells().global_special_state.castle_entity_index, 0,
            "final source loss clears the current A4 even when its previous slot differs");
    }

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
