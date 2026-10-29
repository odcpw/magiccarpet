//! Current source inventory relations used by GameUI's visibility consumers.

use sha2::{Digest, Sha256};

use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::spell_activation::BeyondSightState;

use super::{
    EntitySnapshot, EntitySpecialRelation, Level2DynamicArenaKind, Level2NyphurVisibility,
    PlayerSnapshot, RivalWizardRoster,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HolderBinding {
    owner: SourceEntityHandle,
    holder: SourceEntityHandle,
}

/// The local inventory needs the same retained generation relation that the
/// rival roster already owns. Only constructors/acquisition may bind it;
/// observing a reused numeric slot must never adopt its new generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlayerVisibility {
    local_beyond_sight: Option<HolderBinding>,
}

fn held_spell<'a>(
    arena: &'a SourceArena,
    player: &PlayerSnapshot,
    family: usize,
) -> Option<(HolderBinding, &'a EntitySnapshot)> {
    let owner = arena.handle(player.entity_index)?;
    let actor = arena.entities.get(usize::from(player.entity_index))?;
    if !actor.active
        || actor.class != 3
        || actor.special_relation != EntitySpecialRelation::PlayerState(player.slot)
    {
        return None;
    }
    let slot = *player.enabled_spell_holders.get(family)?;
    if slot == 0 {
        return None;
    }
    let holder = arena.handle(slot)?;
    match arena.resolve(holder)? {
        SourceEntityView::Imported(_) => (),
        SourceEntityView::Dynamic(
            Level2DynamicArenaKind::SpellPickup | Level2DynamicArenaKind::SpellReplacement,
        ) => (),
        _ => return None,
    }
    let row = arena.entities.get(usize::from(slot))?;
    (row.active
        && row.index == slot
        && row.class == 15
        && usize::from(row.model) == family
        && usize::from(row.action) == family * 3
        && row.source_spell_parent_entity_index() == player.entity_index
        && row.life >= 0)
        .then_some((HolderBinding { owner, holder }, row))
}

impl PlayerVisibility {
    pub(crate) fn from_reset(arena: &SourceArena, local: &PlayerSnapshot) -> Self {
        Self {
            local_beyond_sight: held_spell(arena, local, 12).map(|(binding, _)| binding),
        }
    }

    /// Called after the genuine holder constructor/acquisition is committed.
    pub(crate) fn bind_beyond_sight(&mut self, arena: &SourceArena, local: &PlayerSnapshot) {
        self.local_beyond_sight = held_spell(arena, local, 12).map(|(binding, _)| binding);
    }

    pub(crate) fn local_holder<'a>(
        &self,
        arena: &'a SourceArena,
        local: &PlayerSnapshot,
    ) -> Option<&'a EntitySnapshot> {
        held_spell(arena, local, 12)
            .filter(|(binding, _)| Some(*binding) == self.local_beyond_sight)
            .map(|(_, row)| row)
    }

    pub(crate) fn status_snapshot(
        &self,
        arena: &SourceArena,
        local: &PlayerSnapshot,
        cached: Option<BeyondSightState>,
    ) -> Option<BeyondSightState> {
        let holder = self.local_holder(arena, local)?;
        let tier @ 0..=2 = u8::try_from(holder.level_gate).ok()? else {
            return None;
        };
        let parent = arena.entities.get(usize::from(local.entity_index))?;
        let mut state = cached
            .filter(|state| {
                state.holder_entity_index == holder.index
                    && state.parent_entity_index == parent.index
            })
            .or_else(|| {
                BeyondSightState::new(tier, holder.index, parent.index, parent.mana, parent.life)
            })?;
        state.tier = tier;
        state.action = holder.action;
        state.active_value = holder.active_value;
        state.activation_value = holder.activation_value.cast_signed();
        state.mana_cost = holder.max_mana;
        state.mana_regen_requirement = holder.mana_regen;
        state.countdown = i16::from_le_bytes([holder.record[58], holder.record[59]]);
        state.parent_mana = parent.mana;
        state.parent_life = parent.life;
        state.parent_mana_regen = parent.mana_regen;
        Some(state)
    }

    pub(crate) fn snapshot(
        &self,
        arena: &SourceArena,
        rivals: &RivalWizardRoster,
        local: &PlayerSnapshot,
        target: Option<&PlayerSnapshot>,
    ) -> Level2NyphurVisibility {
        let holder = self.local_holder(arena, local);
        let tier = holder.and_then(|row| u8::try_from(row.level_gate).ok());
        let target = target.filter(|player| {
            // GameUI visits the current player-count range and living source
            // actors; it does not gate names on the lobby active-index byte.
            arena
                .entities
                .get(usize::from(player.entity_index))
                .is_some_and(|row| {
                    row.active
                        && row.index == player.entity_index
                        && row.class == 3
                        && row.special_relation == EntitySpecialRelation::PlayerState(player.slot)
                })
                && (!player.is_ai || rivals.get_in_arena(arena, player.entity_index).is_some())
        });
        let owner_visibility_mode = target.map_or(0, |player| player.visibility_mode);
        let family4_gate_active = target.is_some_and(|player| {
            held_spell(arena, player, 4).is_some_and(|(binding, row)| {
                row.active_value != 0
                    && (!player.is_ai
                        || rivals.bound_live_holder(binding.owner, player, arena, 4)
                            == Some(binding.holder))
            })
        });
        // sub_63570 intentionally tests the held tier, not word_2E. Passive
        // reveal persists after expiry; the separate label/mine pass does not.
        let visible_to_local_player = target.is_some()
            && match tier {
                None => true,
                Some(0) => owner_visibility_mode == 0 && !family4_gate_active,
                Some(1) => !family4_gate_active,
                Some(2..) => true,
            };
        Level2NyphurVisibility {
            owner_visibility_mode,
            beyond_sight_holder_present: holder.is_some(),
            beyond_sight_active: holder.is_some_and(|row| row.active_value != 0),
            beyond_sight_tier: tier,
            family4_gate_active,
            visible_to_local_player,
        }
    }

    pub(crate) fn hash_into(
        &self,
        hasher: &mut Sha256,
        arena: &SourceArena,
        local: &PlayerSnapshot,
    ) {
        // A matching relation is reconstructible from the already-hashed arena
        // and player. Only a retained stale/missing binding adds information.
        if self.local_beyond_sight == held_spell(arena, local, 12).map(|(binding, _)| binding) {
            return;
        }
        hasher.update(b"source-local-visibility-binding-v1");
        if let Some(binding) = self.local_beyond_sight {
            hasher.update([1]);
            hasher.update(binding.owner.source_slot.to_le_bytes());
            hasher.update(binding.owner.generation.to_le_bytes());
            hasher.update(binding.holder.source_slot.to_le_bytes());
            hasher.update(binding.holder.generation.to_le_bytes());
        } else {
            hasher.update([0]);
        }
    }
}
