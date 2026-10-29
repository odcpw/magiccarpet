//! Source `sub_21F60` projectile protection for the final boss and registered
//! special building.

use crate::campaign::source_arena::SourceArena;
use crate::native_world::{Level2SharedSpellResetSession, SessionTerrain};

const PROTECTED_PROJECTILE_MODELS: [u8; 8] = [2, 4, 5, 10, 22, 23, 25, 30];
const SOURCE_AXIS_HALF_EXTENT: i16 = 5_120;
const BOSS_PROJECTILE_RADIUS: u32 = 0x0c00;

#[derive(Clone, Copy)]
struct Projectile {
    slot: u16,
    model: u8,
    position: crate::Position,
    carrier: ProjectileCarrier,
}

#[derive(Clone, Copy)]
enum ProjectileCarrier {
    Imported,
    Castle,
    Fireball,
    Possession,
}

/// Walk the live source class-9 chain in its source order, applying the
/// `sub_21F60` collision rules. `class9_slots_in_source_order` is the current
/// `dword_38531` chain, not a slot-sorted census. Every slot is resolved back
/// to its authoritative imported row or typed projectile sidecar here.
///
/// Each successful allocation emits the existing typed class-10/model-0
/// action-0 actor. The common effect projection binds that carrier to
/// particle parameter 7, and this adapter publishes its map links into the
/// session terrain immediately. A failed effect allocation still hides the
/// matched projectile, as the source does. The return value is the source
/// close flag consumed by the boss action callback; callers of the
/// special-building epilogue ignore it.
pub(crate) fn apply(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    protected_slot: u16,
    shared_spells: &mut Level2SharedSpellResetSession,
    class9_slots_in_source_order: &[u16],
) -> Result<bool, String> {
    let protected = arena
        .entities
        .get(usize::from(protected_slot))
        .filter(|row| row.active)
        .cloned()
        .ok_or_else(|| format!("source protection row {protected_slot} is absent"))?;
    let boss = protected.class == 5 && protected.model == 10;
    let castle = player_castle_geometry(arena);
    let mut close = false;

    for &candidate_slot in class9_slots_in_source_order {
        let Some(candidate) = source_projectile(arena, candidate_slot)? else {
            continue;
        };
        if !PROTECTED_PROJECTILE_MODELS.contains(&candidate.model) {
            continue;
        }

        let (overlap, closes_boss) = if candidate.model < 10 {
            let (target_position, target_half_x, target_half_y) = castle.unwrap_or((
                candidate.position,
                SOURCE_AXIS_HALF_EXTENT,
                SOURCE_AXIS_HALF_EXTENT,
            ));
            (
                crate::native_world::level2_compare_axis_with_shift(
                    protected.position,
                    if boss {
                        SOURCE_AXIS_HALF_EXTENT
                    } else {
                        protected.array_pitch()
                    },
                    if boss {
                        SOURCE_AXIS_HALF_EXTENT
                    } else {
                        protected.array_roll()
                    },
                    target_position,
                    target_half_x,
                    target_half_y,
                ),
                boss,
            )
        } else if boss {
            let overlap = crate::distance_3d_wrapped(protected.position, candidate.position)
                <= BOSS_PROJECTILE_RADIUS;
            // Both the boss distance branch and its small-projectile axis
            // branch set v19; special-building protection ignores the flag.
            (overlap, overlap)
        } else {
            (
                crate::native_world::level2_compare_axis_with_shift(
                    protected.position,
                    protected.array_pitch(),
                    protected.array_roll(),
                    candidate.position,
                    SOURCE_AXIS_HALF_EXTENT,
                    SOURCE_AXIS_HALF_EXTENT,
                ),
                false,
            )
        };
        if !overlap {
            continue;
        }

        close |= closes_boss;
        if candidate.model == 10 {
            clear_spell_two_holder(arena, shared_spells)?;
        }

        // NewAdd0A00_4E320 creates the established typed action-0 impact row;
        // the source overwrites its id with the protected entity's id.
        let effect_slot = arena.economy.allocate_class10_model0_effect(
            0,
            protected.source_id,
            0,
            0,
            candidate.position,
            400,
            0,
            0x0002_0000,
            128,
            128,
        );
        let terrain_publish = if let Some(effect_slot) = effect_slot {
            crate::world::effects::sync_position(
                arena,
                terrain,
                effect_slot,
                candidate.position,
                true,
                false,
            )
        } else {
            Ok(())
        };

        hide_candidate(arena, candidate)?;
        terrain_publish.map_err(|error| {
            format!("source protection impact terrain publication failed: {error}")
        })?;
    }

    let rebound_slot = shared_spells.local_player().enabled_spell_holders[8];
    if rebound_slot != 0 {
        let rebound_word = shared_spells
            .pickup(rebound_slot)
            .or_else(|| arena.entities.get(usize::from(rebound_slot)))
            .ok_or_else(|| format!("source spell-8 holder row {rebound_slot} is absent"))?
            .source_word_2e();
        if rebound_word > 0 {
            close = true;
        }
    }
    Ok(close)
}

fn player_castle_geometry(arena: &SourceArena) -> Option<(crate::Position, i16, i16)> {
    let castle_slot = arena.economy.player_castle_entity_index();
    if castle_slot == 0 {
        return None;
    }
    if let Some(castle) = arena
        .economy
        .castle
        .filter(|castle| castle.active && castle.source_entity_index == castle_slot)
    {
        return Some((castle.position, castle.shift_pitch, castle.shift_roll));
    }
    arena
        .entities
        .get(usize::from(castle_slot))
        .filter(|row| row.active)
        .map(|row| (row.position, row.array_pitch(), row.array_roll()))
}

fn clear_spell_two_holder(
    arena: &mut SourceArena,
    shared_spells: &mut Level2SharedSpellResetSession,
) -> Result<(), String> {
    let holder_slot = shared_spells.local_player().enabled_spell_holders[2];
    if let Some(holder) = shared_spells.pickup_mut(holder_slot) {
        holder.record[46..48].fill(0);
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(holder_slot))
            .filter(|row| {
                row.active
                    && row.class == holder.class
                    && row.model == holder.model
                    && row.source_id == holder.source_id
            })
        {
            row.record[46..48].fill(0);
        }
    } else if let Some(holder) = arena.entities.get_mut(usize::from(holder_slot)) {
        holder.record[46..48].fill(0);
    } else {
        return Err(format!("source spell-2 holder row {holder_slot} is absent"));
    }
    Ok(())
}

fn source_projectile(arena: &SourceArena, slot: u16) -> Result<Option<Projectile>, String> {
    if slot == 0 {
        return Ok(None);
    }
    if let Some(row) = arena
        .economy
        .castle_projectiles
        .iter()
        .find(|row| row.active && row.source_class == 9 && row.source_entity_index == slot)
    {
        return Ok(Some(Projectile {
            slot,
            model: row.source_model,
            position: row.position,
            carrier: ProjectileCarrier::Castle,
        }));
    }
    if let Some(row) = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|row| row.active && row.source_class == 9 && row.source_entity_index == slot)
    {
        return Ok(Some(Projectile {
            slot,
            model: row.source_model,
            position: row.position,
            carrier: ProjectileCarrier::Fireball,
        }));
    }
    if let Some(row) = arena
        .economy
        .possession_projectiles
        .iter()
        .find(|row| row.active && row.source_class == 9 && row.source_entity_index == slot)
    {
        return Ok(Some(Projectile {
            slot,
            model: row.source_model,
            position: row.position,
            carrier: ProjectileCarrier::Possession,
        }));
    }
    if let Some(row) = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 9)
    {
        return Ok(Some(Projectile {
            slot,
            model: row.model,
            position: row.position,
            carrier: ProjectileCarrier::Imported,
        }));
    }
    Err(format!(
        "source class-9 chain slot {slot} has no live arena projectile"
    ))
}

fn hide_candidate(arena: &mut SourceArena, candidate: Projectile) -> Result<(), String> {
    match candidate.carrier {
        ProjectileCarrier::Imported => {
            let row = arena
                .entities
                .get_mut(usize::from(candidate.slot))
                .filter(|row| row.active && row.class == 9 && row.model == candidate.model)
                .ok_or_else(|| {
                    format!(
                        "source protection projectile slot {} changed before hide",
                        candidate.slot
                    )
                })?;
            row.disable_drawing();
        }
        ProjectileCarrier::Castle => {
            let row = arena
                .economy
                .castle_projectiles
                .iter_mut()
                .find(|row| {
                    row.active && row.source_class == 9 && row.source_entity_index == candidate.slot
                })
                .ok_or_else(|| {
                    format!(
                        "source Castle projectile slot {} changed before hide",
                        candidate.slot
                    )
                })?;
            row.drawing_disabled = true;
        }
        ProjectileCarrier::Fireball => {
            let row = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| {
                    row.active && row.source_class == 9 && row.source_entity_index == candidate.slot
                })
                .ok_or_else(|| {
                    format!(
                        "source Fireball projectile slot {} changed before hide",
                        candidate.slot
                    )
                })?;
            row.drawing_disabled = true;
        }
        ProjectileCarrier::Possession => {
            return Err(format!(
                "source protection cannot hide typed Possession projectile slot {}",
                candidate.slot
            ));
        }
    }
    Ok(())
}
