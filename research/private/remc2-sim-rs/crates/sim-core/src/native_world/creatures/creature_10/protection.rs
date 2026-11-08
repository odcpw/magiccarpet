//! Source `sub_21F60` projectile protection for the final boss and registered
//! special building.

use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    Level2SharedSpellResetSession, ParticleParameter, SessionTerrain, source_target,
};

const PROTECTED_PROJECTILE_MODELS: [u8; 8] = [2, 4, 5, 10, 22, 23, 25, 30];
const SOURCE_AXIS_HALF_EXTENT: i16 = 5_120;
const BOSS_PROJECTILE_RADIUS: u32 = 0x0c00;

#[derive(Clone, Copy)]
struct Projectile {
    slot: u16,
    model: u8,
    position: crate::Position,
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
    particles: &[ParticleParameter],
    class9_slots_in_source_order: &[u16],
) -> Result<bool, String> {
    let protected = current(arena, shared_spells, particles, terrain, protected_slot)?;
    let boss = protected.class == 5 && protected.model == 10;
    let mut close = false;

    let members = class9_slots_in_source_order
        .iter()
        .map(|&slot| (slot, arena.economy.slot_generation(slot)))
        .collect::<Vec<_>>();
    for (candidate_slot, generation) in members {
        let Some(candidate) =
            source_projectile(arena, shared_spells, particles, terrain, candidate_slot)?
        else {
            continue;
        };
        if !PROTECTED_PROJECTILE_MODELS.contains(&candidate.model) {
            // A NewEvent inside this scan resets the replaced row's next
            // pointer. It may truncate the chain at a later member too.
            if arena.economy.slot_generation(candidate_slot) != generation {
                break;
            }
            continue;
        }
        let protected = current(arena, shared_spells, particles, terrain, protected_slot)?;
        let shifts = protected.collision.ok_or_else(|| {
            format!("source protection row {protected_slot} has no current collision axes")
        })?;

        // Only the Castle projectile (model10) substitutes the player's
        // Castle position and shifts. The other admitted models jump to
        // LABEL_19 in sub_21F60 and use their own position.
        let (overlap, closes_boss) = if candidate.model == 10 {
            let (target_position, target_half_x, target_half_y) = player_castle_geometry(arena)
                .unwrap_or((
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
                        shifts.half_width
                    },
                    if boss {
                        SOURCE_AXIS_HALF_EXTENT
                    } else {
                        shifts.half_depth
                    },
                    target_position,
                    target_half_x,
                    target_half_y,
                ),
                boss,
            )
        } else if boss {
            let overlap = crate::native_world::level2_wrapping_target_distance(protected.position, candidate.position)
                <= BOSS_PROJECTILE_RADIUS;
            // Both the boss distance branch and its Castle-projectile axis
            // branch set v19; special-building protection ignores the flag.
            (overlap, overlap)
        } else {
            (
                crate::native_world::level2_compare_axis_with_shift(
                    protected.position,
                    shifts.half_width,
                    shifts.half_depth,
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

        let effect_slot = crate::world::effects::allocate_from_source_position(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            candidate.slot,
            candidate.position,
            particles,
        )?;
        if let Some(slot) = effect_slot {
            // The source reads the protected owner's id AFTER NewEvent.
            let owner =
                current(arena, shared_spells, particles, terrain, protected_slot)?.source_id;
            let effect = &mut arena.entities[usize::from(slot)];
            effect.source_id = owner;
            effect.record[30..32].copy_from_slice(&owner.to_le_bytes());
        }
        if !crate::world::effects::hide_birth_replacement(arena, candidate.slot, effect_slot)? {
            hide_candidate(arena, candidate.slot)?;
        }
        if arena.economy.slot_generation(candidate_slot) != generation {
            break;
        }
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
        holder.record[50..52].fill(0);
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
            row.record[50..52].fill(0);
        }
    } else if let Some(holder) = arena.entities.get_mut(usize::from(holder_slot)) {
        holder.record[50..52].fill(0);
    } else {
        return Err(format!("source spell-2 holder row {holder_slot} is absent"));
    }
    Ok(())
}

fn current(
    arena: &SourceArena,
    spells: &Level2SharedSpellResetSession,
    particles: &[ParticleParameter],
    terrain: &SessionTerrain,
    slot: u16,
) -> Result<source_target::View, String> {
    source_target::read(
        arena,
        source_target::Context {
            spells: Some(spells),
            particles: Some(particles),
            map_type: Some(terrain.map_type()),
            ..source_target::Context::default()
        },
        slot,
    )?
    .ok_or_else(|| format!("source protection address {slot} is absent"))
}

fn source_projectile(
    arena: &SourceArena,
    spells: &Level2SharedSpellResetSession,
    particles: &[ParticleParameter],
    terrain: &SessionTerrain,
    slot: u16,
) -> Result<Option<Projectile>, String> {
    if slot == 0 {
        return Ok(None);
    }
    // Membership was frozen before callbacks. Read the current address even
    // when an earlier callback changed its class, life, or drawing flags.
    let row = current(arena, spells, particles, terrain, slot)?;
    Ok(Some(Projectile {
        slot,
        model: row.model,
        position: row.position,
    }))
}

fn hide_candidate(arena: &mut SourceArena, slot: u16) -> Result<(), String> {
    use crate::native_world::Level2DynamicArenaKind as Kind;
    match arena
        .economy
        .live_arena_cursor_row(slot)
        .map(|row| row.kind)
    {
        Some(Kind::CastleProjectile) => {
            let row = arena
                .economy
                .castle_projectiles
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or_else(|| format!("source Castle projectile {slot} has no carrier"))?;
            row.active = false;
            row.drawing_disabled = true;
            row.control_flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
        }
        Some(Kind::FireballProjectile) => {
            let row = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or_else(|| format!("source Fireball projectile {slot} has no carrier"))?;
            row.active = false;
            row.drawing_disabled = true;
            row.control_flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
        }
        Some(Kind::PossessionProjectile) => {
            let row = arena
                .economy
                .possession_projectiles
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .ok_or_else(|| format!("source Possession projectile {slot} has no carrier"))?;
            row.active = false;
        }
        Some(kind) => {
            return Err(format!(
                "source protection has no current hide writer for {kind:?} at {slot}"
            ));
        }
        None => arena.entities[usize::from(slot)].disable_drawing(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_castle_projectiles_use_the_boss_distance_boundary() {
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-1pb-16-level2-full-build-v2");
        let campaign = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::LEVEL2,
        )
        .expect("actual Level2 packet retains source constructor counters");
        let session = campaign.simulation();
        let live = &session.session.state.live;
        {
            let mut arena = live.economy.clone();
            let mut spells = live.shared_spells.clone();
            let holder = spells.local_player().enabled_spell_holders[2];
            let row = spells
                .pickup_mut(holder)
                .expect("actual Castle spell holder");
            row.set_subspell_index(93);
            row.record[50..52].copy_from_slice(&7_u16.to_le_bytes());
            clear_spell_two_holder(&mut arena, &mut spells).unwrap();
            let row = spells.pickup(holder).unwrap();
            assert_eq!(row.source_word_2e(), 0, "sub_21F60 clears source word2E");
            assert_eq!(row.subspell_index, 93, "source word2A is a separate field");
        }
        for model in [2_u8, 4, 5, 22, 23, 25, 30] {
            for (distance, altitude_delta) in [(3_072_u16, 0_i16), (3_073, 0), (3_072, 16_000), (3_073, 16_000)] {
                let mut arena = live.economy.clone();
                let mut terrain = live.terrain.clone();
                let mut runtime = live.core.runtime.clone();
                let mut spells = live.shared_spells.clone();
                let boss = super::super::mc2::allocate_authored(
                    &mut arena,
                    &mut runtime,
                    &mut terrain,
                    &session.session.definition.presentation.particles,
                    live.core.state.position,
                    2,
                )
                .unwrap()
                .unwrap();
                let mut position = arena.entities[usize::from(boss)].position;
                position.x = position.x.wrapping_add(distance);
                position.z = position.z.wrapping_add(altitude_delta);
                // A diagnostic raw class9 row isolates sub_21F60's model
                // selection; this does not claim an ordinary spell producer.
                let projectile = arena
                    .allocate_source_slot_with_terrain(&mut terrain)
                    .unwrap();
                let row = &mut arena.entities[usize::from(projectile)];
                row.active = true;
                row.record[2] = 1;
                row.class = 9;
                row.model = model;
                row.action = model;
                row.record[67] = 9;
                row.record[68] = model;
                row.record[73] = model;
                row.set_control_flags(0);
                row.set_position(position);
                crate::world::effects::sync_position(
                    &mut arena,
                    &mut terrain,
                    projectile,
                    position,
                    true,
                    false,
                )
                .unwrap();

                let alias = (model == 2 && distance == 3_072 && altitude_delta == 0)
                    .then(|| (arena.clone(), terrain.clone(), spells.clone()));
                apply(
                    &mut arena,
                    &mut terrain,
                    boss,
                    &mut spells,
                    &session.session.definition.presentation.particles,
                    &[projectile],
                )
                .unwrap();
                assert_eq!(
                    arena.entities[usize::from(projectile)].drawing_disabled(),
                    distance == 3_072,
                    "model{model} at XY distance{distance}, Z delta{altitude_delta} uses source58490",
                );
                if let Some((mut arena, mut terrain, mut spells)) = alias {
                    for row in &mut arena.entities {
                        row.set_control_flags(
                            row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                        );
                    }
                    arena.entities[usize::from(projectile)]
                        .set_control_flags(crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
                    arena.rebuild_source_free_arena();
                    arena.economy.exhaust_new_event_capacity_for_test();
                    let previous = arena.handle(projectile).unwrap();
                    let owner = arena.entities[usize::from(boss)].source_id;
                    apply(
                        &mut arena,
                        &mut terrain,
                        boss,
                        &mut spells,
                        &session.session.definition.presentation.particles,
                        &[projectile],
                    )
                    .unwrap();
                    let row = &arena.entities[usize::from(projectile)];
                    assert_eq!((row.class, row.model, row.action), (10, 0, 0));
                    assert_eq!(row.position, crate::Position::default());
                    assert_eq!(row.source_id, owner);
                    assert!(
                        row.drawing_disabled(),
                        "the current newborn receives the source hide"
                    );
                    assert!(arena.resolve(previous).is_none());
                    assert!(arena.economy.map_chains.is_linked(projectile));
                    arena
                        .rebuild_source_arena_with_terrain(&mut terrain, &[])
                        .unwrap();
                    assert!(!arena.economy.map_chains.is_linked(projectile));
                    assert!(arena.economy.free_source_slots().contains(&projectile));
                }
            }
        }
    }
}
