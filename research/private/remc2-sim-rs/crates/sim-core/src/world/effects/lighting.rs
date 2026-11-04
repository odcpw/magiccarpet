//! Source projectile lights over the authoritative arena and terrain planes.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::dynamic_lighting::{DynamicLightLinkedEntity, DynamicLightingState};
use crate::native_world::{MapType, SessionTerrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceLight {
    Fireball,
    Lightning,
    Impact,
    GroundFire,
}

fn night_or_cave(terrain: &SessionTerrain) -> bool {
    matches!(terrain.map_type(), MapType::Night | MapType::Cave)
}

/// The caller invokes this at the constructor's AddEvent2 site, before any
/// subsequent source callback can allocate another light. A successful
/// admission must set the source row's light-linked flag at that same site.
pub(crate) fn admit(
    lighting: &mut DynamicLightingState,
    enabled: bool,
    terrain: &SessionTerrain,
    slot: u16,
    position: Position,
    kind: SourceLight,
) -> bool {
    let (intensity, jitter, life_scaled) = match kind {
        SourceLight::Fireball => (128, 1, false),
        SourceLight::Lightning => (128, 9, false),
        SourceLight::Impact => (128, 7, true),
        SourceLight::GroundFire => (80, 11, true),
    };
    lighting
        .admit(
            enabled,
            night_or_cave(terrain),
            slot,
            position,
            intensity,
            jitter,
            life_scaled,
        )
        .is_some()
}

/// Admit immediately after the source constructor has mapped and initialized
/// the row. The light state is separate from the effects state borrowed by
/// the scheduler while it visits an effect.
pub(crate) fn admit_source(
    arena: &mut SourceArena,
    terrain: &SessionTerrain,
    slot: u16,
    position: Position,
    kind: SourceLight,
) -> bool {
    let admitted = admit(
        &mut arena.dynamic_lighting,
        arena.dynamic_lighting_enabled,
        terrain,
        slot,
        position,
        kind,
    );
    if admitted && !arena.economy.mark_fireball_light_linked(slot) {
        if let Some(child) = arena
            .economy
            .effect_sphere_children_mut()
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            child.control_flags |= 0x0008_0000;
        } else if let Some(fire) = arena.economy.authored_model6_mut(slot) {
            fire.control_flags |= 0x0008_0000;
        } else if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
            row.set_control_flags(row.control_flags | 0x0008_0000);
        }
    }
    admitted
}

/// Remove the previous footprint before the first UpdateEntities pass.
pub(crate) fn before_updates(lighting: &DynamicLightingState, terrain: &mut SessionTerrain) {
    let active_map = night_or_cave(terrain);
    let (_, shading) = terrain.dynamic_lighting_planes();
    lighting.subtract_previous(shading, active_map);
}

/// Publish the last typed Fireball bytes before the following-pass reclaim
/// removes its carrier. Source fixed-address readers can still observe those
/// bytes until NewEvent reuses the slot.
pub(crate) fn prepare_fireball_reclaim(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> Result<(), String> {
    let retiring = arena
        .economy
        .fireball_projectiles()
        .iter()
        .filter(|row| !row.active && row.drawing_disabled)
        .copied()
        .collect::<Vec<_>>();
    for projectile in retiring {
        let slot = projectile.source_entity_index;
        let links = if arena.economy.map_chains.is_linked(slot) {
            (arena.economy.map_chains.old_map_entity(slot),
                arena.economy.map_chains.next_entity(slot))
        } else {
            let row = &arena.entities[usize::from(slot)];
            (u16::from_le_bytes([row.record[26], row.record[27]]),
                u16::from_le_bytes([row.record[28], row.record[29]]))
        };
        crate::world::effects::sync_position(
            arena,
            terrain,
            slot,
            projectile.position,
            false,
            false,
        )?;
        let row = &mut arena.entities[usize::from(slot)];
        row.set_map_links(links.0, links.1);
        row.reclaim_source_row();
        row.model = projectile.source_model;
        row.record[68] = projectile.source_model;
        row.source_id = projectile.owner_entity_index;
        row.record[30..32].copy_from_slice(&projectile.owner_entity_index.to_le_bytes());
        row.set_source_action(projectile.action);
        row.set_position(projectile.position);
        row.set_life(i32::from(projectile.life));
        row.set_max_life(i32::from(projectile.max_life));
        row.set_entity_rng(projectile.entity_rng);
        row.set_control_flags((projectile.control_flags | 0x400) & !4);
        row.set_subspell_index(projectile.source_subspell_index);
        row.set_yaw(projectile.yaw);
        row.pitch = projectile.pitch;
        row.record[34..36].copy_from_slice(&projectile.pitch.to_le_bytes());
        row.record[36..38].copy_from_slice(&projectile.roll.to_le_bytes());
        row.fov = projectile.fov;
        row.record[38..40].copy_from_slice(&projectile.fov.to_le_bytes());
        row.record[69] = projectile.target_class;
        row.record[70] = projectile.target_model;
        row.record[154..156].copy_from_slice(&projectile.target_source_entity_index.to_le_bytes());
        let descriptor = match projectile.origin {
            crate::native_world::FireballProjectileOrigin::Creature27 => {
                Some(crate::world::effects::MODEL27_GUIDANCE.source_index)
            }
            crate::native_world::FireballProjectileOrigin::Firefly
            | crate::native_world::FireballProjectileOrigin::Dragon => Some(65),
            _ => None,
        };
        if let Some(descriptor) = descriptor {
            // The origin drives live guidance; retain its source descriptor
            // in the current address after the typed carrier is reclaimed.
            row.parameter_relation = Some(descriptor);
            row.record[164..168].copy_from_slice(&(u32::from(descriptor) + 1).to_le_bytes());
        }
    }
    Ok(())
}

/// Recompute after the last pass. Fixed addresses resolve through the same
/// current-occupant projection as targeting, including recycled carriers.
pub(crate) fn after_updates(
    lighting: &mut DynamicLightingState,
    arena: &SourceArena,
    terrain: &mut SessionTerrain,
    targets: crate::native_world::source_target::Context<'_>,
) -> Result<(), String> {
    if !night_or_cave(terrain) {
        return Ok(());
    }
    let mut linked = std::collections::BTreeMap::new();
    for (_, light) in lighting.active_slots().filter(|(_, light)| light.linked) {
        let slot = light.source_entity_index;
        let current = crate::native_world::source_target::read(arena, targets, slot)?.map(|row| {
            DynamicLightLinkedEntity {
                position: row.position,
                life: row.life,
                disabled: row.drawing_disabled(),
            }
        });
        linked.insert(slot, current);
    }
    let (ground, shading) = terrain.dynamic_lighting_planes();
    lighting.service_recompute(
        shading,
        true,
        |slot| linked.get(&slot).copied().flatten(),
        |x, y| crate::legacy_terrain::interpolate_ground(ground, x, y) as i16,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlightWorld;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::{lightning, test_guided_boss as boss};
    use std::path::PathBuf;

    #[test]
    fn authored_volcano_debris_obeys_current_fire_proximity_and_runs_smoke_lifetime() {
        use crate::level_definition::EffectKind;
        use crate::native_world::Mc2DispatchEvent;

        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level18");
        let mut session = CampaignSession::open(package, CampaignSelection::ordinary(18).unwrap())
            .expect("actual ordinary Level 18 owner package").simulation().clone();
        let feature = session.mission().authored_features().iter()
            .find(|row| row.index == 422 && row.kind == 10 && row.subtype == 9)
            .expect("the actual group-20 Volcano feature");
        assert_eq!(feature.activation_group, 20);
        let prior = session.entities().iter().filter(|row| row.active && (row.class, row.model) == (10, 9))
            .filter_map(|row| session.source_arena().handle(row.index)).collect::<Vec<_>>();
        let switch_position = session.mission().authored_features().iter()
            .find(|row| row.index == 371 && row.kind == 11 && row.subtype == 4)
            .expect("the actual authored completion switch").world_position(0);
        let switch = session.entities().iter().find(|row| row.active
            && (row.class, row.model, row.action, row.source_id) == (11, 4, 4, 20)
            && (row.position.x, row.position.y) == (switch_position.x, switch_position.y))
            .expect("the actual imported group-20 completion switch is live").index;
        let color = session.shared_spells().local_player().player_color_index();
        // Declare the completion prerequisite; the actual model-4 source
        // publishes the complete authored group through the scheduler.
        session.session.state.live.core.runtime.objectives[usize::from(color)].is_level_end = 1;
        for _ in 0..8 {
            session.step_outer_turn(crate::PlayerCommand::default(), 0);
            if session.dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 20, .. }
                    if *source_slot == switch)) { break; }
        }
        assert!(session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 20, .. }
                if *source_slot == switch)), "the real completion switch must publish group 20");
        let volcano = session.entities().iter().find(|row| row.active
            && (row.class, row.model, row.action) == (10, 9, 9)
            && session.source_arena().handle(row.index).is_some_and(|handle| !prior.contains(&handle)))
            .expect("group 20 constructs its actual Volcano");
        let owner = volcano.source_id;
        let volcano_slot = volcano.index;
        let original_volcano_position = volcano.position;
        // The real group also publishes lowering rings. On the original
        // terrain an intervening ring changes the floor during the Eruption's
        // first visit; sub_32880 must hide it without producing debris.
        let mut cancellation = session.clone();
        let mut cancelled = None;
        for _ in 0..32 {
            let ground_before = cancellation.canonical_terrain().ground_height(original_volcano_position);
            cancellation.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(!cancellation.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            if let Some(row) = cancellation.entities().iter().find(|row| row.active
                && (row.class, row.model, row.action) == (10, 18, 18)
                && row.source_id == owner && row.drawing_disabled())
            {
                assert_eq!(row.source_dword_10(), 0, "the changed-floor branch returns before age advances");
                assert_eq!(row.position.z, cancellation.canonical_terrain().ground_height(row.position));
                assert_ne!(ground_before, row.position.z, "the real group changed the floor during the source pass");
                assert!(!cancellation.entities().iter().any(|row| row.active
                    && (row.class, row.model, row.action) == (10, 16, 16) && row.source_id == owner),
                    "the cancelled first Eruption does not create debris");
                cancelled = cancellation.source_arena().handle(row.index);
                break;
            }
        }
        let cancelled = cancelled.expect("the unmodified authored route observes the actual changed-floor cancellation");
        cancellation.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert_ne!(cancellation.source_arena().handle(cancelled.source_slot), Some(cancelled),
            "the cancelled Eruption crosses the next source reclaim boundary");

        // Move only the real authored Volcano to stable owner terrain away
        // from those rings. Its remaining lifetime and every descendant are
        // still advanced by the ordinary scheduler.
        let volcano_position = Position {
            x: original_volcano_position.x.wrapping_add(16_384),
            y: original_volcano_position.y.wrapping_add(16_384),
            z: original_volcano_position.z,
        };
        assert!(!session.source_arena().economy.map_chains.is_linked(volcano_slot),
            "the source Volcano constructor does not map-link its controller");
        session.session.state.live.economy.entities[usize::from(volcano_slot)]
            .set_position(volcano_position);
        let baseline = session.entities().iter().filter(|row| row.active && (row.class, row.model) == (10, 16))
            .filter_map(|row| session.source_arena().handle(row.index)).collect::<Vec<_>>();
        let mut eruption_seen = false;
        let mut debris = None;
        let mut producer_trace = Vec::new();
        for turn in 0..128 {
            session.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(!session.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            if turn < 32 || turn == 127 {
                let row = &session.entities()[usize::from(volcano_slot)];
                let descendants = session.entities().iter().filter(|row| row.active
                    && row.class == 10 && matches!(row.model, 18 | 16)).take(12)
                    .map(|row| (row.index, row.model, row.action, row.source_id, row.position,
                        row.life, row.source_dword_10(), row.control_flags)).collect::<Vec<_>>();
                producer_trace.push((turn, (row.active, row.class, row.model, row.action,
                    row.source_id, row.life, row.record[74], row.phase, row.position, row.control_flags),
                    session.canonical_terrain().ground_height(row.position), descendants));
            }
            eruption_seen |= session.entities().iter().any(|row|
                row.active && (row.class, row.model, row.action) == (10, 18, 18) && row.source_id == owner);
            debris = session.entities().iter().find(|row| row.active
                && (row.class, row.model, row.action) == (10, 16, 16) && row.source_id == owner
                && session.source_arena().handle(row.index).is_some_and(|handle| !baseline.contains(&handle)))
                .map(|row| row.index);
            if debris.is_some() { break; }
        }
        let debris = debris.unwrap_or_else(|| panic!("the authored Volcano's scheduled Eruption produces real burning debris: owner={owner} eruption_seen={eruption_seen} trace={producer_trace:#?}"));
        assert!(eruption_seen, "the actual Eruption must be observed before its debris");
        let debris_handle = session.source_arena().handle(debris).unwrap();

        // Separately observe an unforced debris trajectory landing and
        // creating its real GroundFire; the cases below control geometry only
        // to discriminate the two sub_10B70 boundary rules.
        let mut natural = session.clone();
        let mut natural_fire = false;
        for _ in 0..128 {
            let landings = natural.entities().iter().filter(|row| row.active
                && (row.class, row.model, row.action) == (10, 16, 16) && row.source_id == owner
                && natural.canonical_terrain().terrain_type(row.position) != 0)
                .filter_map(|row| {
                    let vx = i16::from_le_bytes([row.record[158], row.record[159]]).clamp(-80, 80);
                    let vy = i16::from_le_bytes([row.record[160], row.record[161]]).clamp(-80, 80);
                    let vz = i16::from_le_bytes([row.record[48], row.record[49]]);
                    let mut next = Position { x: row.position.x.wrapping_add(vx as u16),
                        y: row.position.y.wrapping_add(vy as u16), z: row.position.z.wrapping_add(vz) };
                    let ground = natural.canonical_terrain().ground_height(next);
                    if ground <= next.z { return None; }
                    next.z = ground;
                    Some(next)
                }).collect::<Vec<_>>();
            let before = natural.entities().iter().filter_map(|row|
                natural.source_arena().handle(row.index).map(|handle| (row.index, handle.generation)))
                .collect::<std::collections::BTreeMap<_, _>>();
            natural.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(!natural.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            natural_fire = natural.entities().iter().any(|row| row.active
                && (row.class, row.model, row.action) == (10, 6, 6) && row.source_id == owner
                && landings.iter().any(|landing| (row.position.x, row.position.y) == (landing.x, landing.y))
                && natural.source_arena().handle(row.index).is_some_and(|handle|
                    before.get(&row.index) != Some(&handle.generation)));
            if natural_fire { break; }
        }
        assert!(natural_fire, "a real Eruption debris trajectory must land and create GroundFire");

        // Explicit landing and reference-fire positions isolate source10B70's
        // tile boundary and distance; no target, fire birth or callback is faked.
        for (cross_boundary, expected_new_fire) in [(true, false), (false, true)] {
            let mut case = session.clone();
            let landing = (1_u16..255).flat_map(|y| (1_u16..255).map(move |x| (x, y)))
                .map(|(x, y)| Position { x: (x << 8) + if cross_boundary { 250 } else { 20 },
                    y: (y << 8) + 128, z: 0 })
                .find_map(|mut p| {
                    p.z = case.canonical_terrain().ground_height(p);
                    let ref_x = p.x.wrapping_add(if cross_boundary { 10 } else { 170 });
                    let other = Position { x: ref_x, ..p };
                    if case.canonical_terrain().terrain_type(p) == 0
                        || case.canonical_terrain().terrain_type(other) == 0
                        || case.canonical_terrain().ground_height(other) != p.z
                        || crate::distance_3d_wrapped(p, volcano_position) < 8_192
                        || crate::distance_3d_wrapped(p, original_volcano_position) < 8_192 { return None; }
                    let arena = case.source_arena();
                    let clear = !super::super::nearby_ground_fire(arena, p)
                        && !super::super::nearby_ground_fire(arena, other);
                    clear.then_some(p)
                }).expect("owner terrain has a dry location away from existing fires");
            let reference = {
                let particles = case.session.definition.presentation.particles.clone();
                let live = &mut case.session.state.live;
                let mut other = Position { x: landing.x.wrapping_add(if cross_boundary { 10 } else { 170 }), ..landing };
                other.z = live.terrain.ground_height(other);
                let slot = crate::world::effects::allocate(&mut live.economy, &mut live.terrain,
                    sim_definition::adapter::mc2_effect_definition(EffectKind::GroundFire), other, &particles)
                    .unwrap().expect("ordinary reference GroundFire constructor");
                // Equal altitude isolates horizontal distance; the position is
                // a declared control on the actual constructed reference.
                live.economy.entities[usize::from(slot)].set_position(Position { z: landing.z, ..other });
                crate::world::effects::sync_position(&mut live.economy, &mut live.terrain, slot,
                    Position { z: landing.z, ..other }, true, false).unwrap();
                let row = &mut live.economy.entities[usize::from(debris)];
                row.set_position(landing);
                row.set_source_word_2c(-1);
                row.record[158..162].fill(0);
                crate::world::effects::sync_position(&mut live.economy, &mut live.terrain,
                    debris, landing, true, false).unwrap();
                slot
            };
            assert_eq!(case.source_arena().handle(debris), Some(debris_handle));
            assert_eq!(super::super::nearby_ground_fire(case.source_arena(), landing), cross_boundary,
                "the current map admits the controlled near reference and rejects the distant one");
            let phase_before = case.entities()[usize::from(debris)].phase;
            let before = case.entities().iter().filter_map(|row|
                case.source_arena().handle(row.index).map(|handle| (row.index, handle.generation)))
                .collect::<std::collections::BTreeMap<_, _>>();
            case.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(!case.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            assert_eq!(case.source_arena().handle(debris), Some(debris_handle),
                "the controlled debris remains the same current occupant");
            let landed = &case.entities()[usize::from(debris)];
            assert_eq!(case.canonical_terrain().ground_height(landing), landing.z,
                "other authored effects must not change this declared landing floor during the pass");
            assert_eq!((landed.position, landed.phase), (landing, phase_before.wrapping_add(1)),
                "the actual debris callback must visit and land at the controlled point");
            let fire = case.entities().iter().find(|row| row.active && row.index != reference
                && (row.class, row.model, row.action) == (10, 6, 6) && row.source_id == owner
                && row.position.x == landing.x && row.position.y == landing.y
                && case.source_arena().handle(row.index).is_some_and(|handle|
                    before.get(&row.index) != Some(&handle.generation)))
                .map(|row| row.index);
            assert_eq!(fire.is_some(), expected_new_fire,
                "cross-boundary near fire suppresses; distant same-tile fire does not: landing={landing:?} owner={owner} debris={debris} reference={reference} fires={:?}",
                case.entities().iter().filter(|row| row.active && (row.class, row.model) == (10, 6))
                    .map(|row| (row.index, row.action, row.source_id, row.position, row.control_flags,
                        case.source_arena().handle(row.index), before.get(&row.index)))
                    .collect::<Vec<_>>());
            if let Some(fire) = fire {
                let fire_handle = case.source_arena().handle(fire).unwrap();
                // The real debris-born fire is retained. Its late-life and
                // allocator pressure are explicit controls for the source
                // 31760 -> smoke14 same-address continuation.
                let mut replaced = case.clone();
                let reset = replaced.session.baseline.economy.economy.free_source_slots().to_vec();
                let source_seed = (0..=u16::MAX)
                    .find(|seed| seed.wrapping_mul(9_377).wrapping_add(9_439) % 7 == 0)
                    .unwrap();
                let mut exhausted = case.clone();
                {
                    let live = &mut exhausted.session.state.live;
                    for row in &mut live.economy.entities {
                        row.set_control_flags(row.control_flags & !0x0002_0000);
                    }
                    let row = &mut live.economy.entities[usize::from(fire)];
                    row.set_life(11);
                    row.set_source_dword_10(1);
                    row.set_entity_rng(source_seed);
                    live.economy.economy.exhaust_new_event_capacity_for_test();
                    live.economy.rebuild_source_arena_with_terrain(&mut live.terrain, &reset).unwrap();
                    live.economy.economy.exhaust_new_event_capacity_for_test();
                }
                let original_particle = exhausted.entities()[usize::from(fire)].particle_parameter_index.unwrap();
                let original_phase = exhausted.entities()[usize::from(fire)].phase;
                exhausted.step_level2_effect_source_slot_for_test(fire).unwrap();
                assert!(!exhausted.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
                let current = &exhausted.entities()[usize::from(fire)];
                assert_eq!(exhausted.source_arena().handle(fire), Some(fire_handle));
                assert_eq!((current.class, current.model, current.life, current.source_dword_10(),
                    current.particle_parameter_index, current.phase),
                    (10, 6, 10, 0, Some(original_particle - 1), original_phase.wrapping_add(1)),
                    "failed NewEvent leaves the real fire and completes its source tail");
                {
                    let live = &mut replaced.session.state.live;
                    for row in &mut live.economy.entities {
                        if row.index != fire {
                            row.set_control_flags(row.control_flags & !0x0002_0000);
                        }
                    }
                    let row = &mut live.economy.entities[usize::from(fire)];
                    assert_ne!(row.control_flags & 0x0002_0000, 0);
                    row.set_life(11);
                    row.set_source_dword_10(1);
                    row.set_entity_rng(source_seed);
                    live.economy.economy.exhaust_new_event_capacity_for_test();
                    live.economy.rebuild_source_arena_with_terrain(&mut live.terrain, &reset).unwrap();
                    live.economy.economy.exhaust_new_event_capacity_for_test();
                    assert!(live.economy.economy.free_source_slots().is_empty());
                }
                let prior_generation = replaced.economy().slot_generation(fire);
                let generations = replaced.entities().iter().map(|row|
                    replaced.economy().slot_generation(row.index)).collect::<Vec<_>>();
                replaced.step_level2_effect_source_slot_for_test(fire).unwrap();
                assert!(!replaced.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
                let row = &replaced.entities()[usize::from(fire)];
                assert_ne!(replaced.economy().slot_generation(fire), prior_generation,
                    "the source allocator reuses the actual GroundFire address");
                assert_eq!((row.class, row.model, row.action), (10, 14, 14),
                    "fire={fire} current_kind={:?} handle={:?} changed={:?}",
                    replaced.economy().live_arena_kind(fire), replaced.source_arena().handle(fire),
                    replaced.entities().iter().filter(|candidate|
                        replaced.economy().slot_generation(candidate.index) != generations[usize::from(candidate.index)])
                        .map(|candidate| (candidate.index, candidate.active, candidate.class, candidate.model,
                            candidate.action, replaced.economy().live_arena_kind(candidate.index)))
                        .collect::<Vec<_>>());
                assert_eq!((row.position.x, row.position.y), (0, 0),
                    "smoke constructor reads the cleared parent position address");
                assert_eq!(row.position.z, replaced.canonical_terrain().ground_height(row.position));
                assert_eq!((row.source_id, row.life, row.source_dword_10(), row.particle_parameter_index),
                    (fire, 15, 100, Some(11)),
                    "31760 continues on the new smoke, including its own particle and ID");
                assert_eq!((row.subspell_index, row.record[69], row.record[70], row.control_flags & 0x0001_0000),
                    (100, 10, 14, 0), "the continuing damage tail reads the newborn smoke payload and selector");
                assert_eq!(row.phase, (fire as u8).wrapping_add(1),
                    "the common epilogue advances the replacement; action14 waits for its next visit");
                let replacement = replaced.source_arena().handle(fire).unwrap();
                if !row.drawing_disabled() {
                    let observer = Position { z: row.position.z.wrapping_add(1_024), ..row.position };
                    replaced.set_level2_entity_position(replaced.state.entity_index, observer).unwrap();
                    let dto = replaced.campaign_entity_presentations(0).unwrap().into_iter()
                        .find(|view| view.entity_index == fire).expect("replacement smoke is presented");
                    assert_eq!((dto.source_id, dto.render_parameter_index), (fire, 11));
                    for _ in 0..32 {
                        if replaced.entities()[usize::from(fire)].drawing_disabled() { break; }
                        replaced.step_level2_source_pass(0);
                    }
                    assert!(replaced.entities()[usize::from(fire)].drawing_disabled(),
                        "replacement smoke naturally reaches its hidden state");
                }
                replaced.step_level2_source_pass(0);
                assert_ne!(replaced.source_arena().handle(fire), Some(replacement),
                    "the actual smoke is reclaimed on the following pass");
                let player = case.state.entity_index;
                case.set_level2_entity_position(player, Position { z: landing.z.wrapping_add(512), ..landing }).unwrap();
                let mut saw_presentation = false;
                let mut smoke = None;
                let mut saw_smoke_hidden = false;
                let mut saw_smoke_presentation = false;
                for _ in 0..96 {
                    let presentations = case.campaign_entity_presentations(0).unwrap();
                    saw_presentation |= presentations.iter().any(|view|
                        view.entity_index == fire && view.source_id == owner
                            && (228..=235).contains(&view.render_parameter_index));
                    if smoke.is_none() {
                        smoke = case.entities().iter().find(|row| row.active
                            && (row.class, row.model, row.action) == (10, 14, 14) && row.source_id == owner
                            && crate::distance_3d_wrapped(landing, row.position) < 1_024
                            && case.source_arena().handle(row.index).is_some_and(|handle|
                                before.get(&row.index) != Some(&handle.generation)))
                            .and_then(|row| case.source_arena().handle(row.index));
                    }
                    if let Some(handle) = smoke {
                        saw_smoke_presentation |= presentations.iter().any(|view|
                            view.entity_index == handle.source_slot && view.source_id == owner
                                && (9..=16).contains(&view.render_parameter_index));
                        let row = &case.entities()[usize::from(handle.source_slot)];
                        saw_smoke_hidden |= case.source_arena().handle(handle.source_slot) == Some(handle)
                            && row.drawing_disabled();
                        if saw_smoke_hidden && case.source_arena().handle(handle.source_slot) != Some(handle)
                            && case.source_arena().handle(fire) != Some(fire_handle) { break; }
                    }
                    case.step_outer_turn(crate::PlayerCommand::default(), 0);
                    assert!(!case.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
                }
                let smoke = smoke.expect("the real scheduled fire produces DarkSmoke14");
                assert!(saw_presentation && saw_smoke_presentation && saw_smoke_hidden,
                    "actual fire/smoke presentation and smoke hide are observed");
                assert_ne!(case.source_arena().handle(smoke.source_slot), Some(smoke), "smoke crosses source reclaim");
                assert_ne!(case.source_arena().handle(fire), Some(fire_handle), "GroundFire crosses source reclaim");
            }
        }

        // Exercise NewEvent's real exhausted fallback on the same produced
        // debris. The cloned arena and landing are explicit exhaustion and
        // geometry controls; the source callback and allocation are unchanged.
        let mut alias = session.clone();
        let particles = alias.session.definition.presentation.particles.clone();
        let live = &mut alias.session.state.live;
        let landing = (1_u16..255).flat_map(|y| (1_u16..255).map(move |x| (x, y)))
            .find_map(|(x, y)| {
                let index = usize::from(y) * 256 + usize::from(x);
                let heights = live.terrain.ground_heights();
                if heights[index] != heights[index + 1] || heights[index] != heights[index + 256]
                    || heights[index] != heights[index + 257] { return None; }
                let mut start = Position { x: (x << 8) + 64, y: (y << 8) + 128, z: 0 };
                start.z = live.terrain.ground_height(start);
                let end = Position { x: start.x.wrapping_add(80), ..start };
                (live.terrain.terrain_type(start) != 0
                    && live.terrain.ground_height(end) == start.z
                    && !super::super::nearby_ground_fire(&live.economy, end)).then_some((start, end))
            }).expect("owner terrain has a dry, flat unburned landing");
        let row = &mut live.economy.entities[usize::from(debris)];
        assert_ne!(row.control_flags & 0x0002_0000, 0, "ordinary debris is source-replaceable");
        row.set_position(landing.0);
        row.set_source_word_2c(-1);
        row.record[158..160].copy_from_slice(&80_i16.to_le_bytes());
        row.record[160..162].copy_from_slice(&0_i16.to_le_bytes());
        crate::world::effects::sync_position(&mut live.economy, &mut live.terrain,
            debris, landing.0, true, false).unwrap();
        live.economy.rebuild_source_free_arena();
        while live.economy.economy.free_slot_count() != 0 {
            live.economy.allocate_source_slot_with_terrain(&mut live.terrain)
                .expect("exhaust the actual free stack");
        }
        let probe_position = Position { x: landing.0.x.wrapping_add(8_192), ..landing.0 };
        loop {
            let mut probe = live.economy.clone();
            let mut probe_terrain = live.terrain.clone();
            let candidate = crate::world::effects::allocate(&mut probe, &mut probe_terrain,
                sim_definition::adapter::mc2_effect_definition(EffectKind::ShortVisual),
                probe_position, &particles).unwrap().expect("replacement cursor reaches the debris");
            if candidate == debris { break; }
            assert_eq!(crate::world::effects::allocate(&mut live.economy, &mut live.terrain,
                sim_definition::adapter::mc2_effect_definition(EffectKind::ShortVisual),
                probe_position, &particles).unwrap(), Some(candidate));
        }
        let previous = live.economy.handle(debris).unwrap();
        let mut state = std::mem::take(&mut live.economy.effects);
        crate::world::effects::visit(&mut live.economy, &mut live.terrain, debris,
            &particles, &mut state, |_, _| unreachable!("debris submits no direct damage"))
            .expect("the real debris callback reaches the current replacement");
        live.economy.effects = state;
        let replacement = &live.economy.entities[usize::from(debris)];
        assert_ne!(live.economy.handle(debris), Some(previous));
        assert_eq!((replacement.class, replacement.model, replacement.action), (10, 6, 6));
        assert_eq!((replacement.position.x, replacement.position.y), (landing.1.x, landing.1.y));
        assert_eq!(&replacement.record[158..162], &[0, 0, 0, 0],
            "the post-allocation tail uses the newborn fire's velocity, not the old debris velocity");
    }

    #[test]
    fn typed_ground_fire_light_and_current_action_reach_the_source_scheduler() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        for (enabled, full) in [(true, false), (false, false), (true, true)] {
            let mut game = CampaignSession::open(&package, CampaignSelection::ordinary(4).unwrap())
                .expect("real Cave4 owner package");
            let (slot, expected_admission, initial_phase) = {
                let live = &mut game.simulation_mut().session.state.live;
                assert_eq!(live.terrain.map_type(), MapType::Cave);
                let tile = live
                    .terrain
                    .planes()
                    .shading
                    .iter()
                    .position(|shade| *shade < 32)
                    .expect("owner cave has a lightable tile");
                let mut position = Position {
                    x: ((tile & 255) as u16).wrapping_shl(8).wrapping_add(128),
                    y: ((tile >> 8) as u16).wrapping_shl(8).wrapping_add(128),
                    z: 0,
                };
                position.z = live.terrain.ground_height(position);
                live.economy.dynamic_lighting_enabled = enabled;
                if full {
                    while live.economy.dynamic_lighting.active_count() < 50 {
                        assert!(
                            live.economy
                                .dynamic_lighting
                                .admit(
                                    true,
                                    true,
                                    live.core.state.entity_index,
                                    position,
                                    128,
                                    1,
                                    false
                                )
                                .is_some()
                        );
                    }
                }
                let before = live.economy.dynamic_lighting.active_count();
                let slot = live
                    .economy
                    .allocate_source_slot_with_terrain(&mut live.terrain)
                    .expect("ground fire source slot");
                let fire = live
                    .economy
                    .economy
                    .initialize_authored_model6(slot, u16::MAX, position, &live.terrain)
                    .expect("real typed ground fire constructor");
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    slot,
                    fire.position,
                    true,
                    false,
                )
                .unwrap();
                let admitted = admit_source(
                    &mut live.economy,
                    &live.terrain,
                    slot,
                    fire.position,
                    SourceLight::GroundFire,
                );
                assert_eq!(admitted, enabled && !full);
                assert_eq!(
                    live.economy.dynamic_lighting.active_count(),
                    before + u8::from(admitted)
                );
                let fire = live.economy.economy.authored_model6_mut(slot).unwrap();
                assert_eq!(fire.control_flags & 0x0008_0000 != 0, admitted);
                // A Tree that aliases its child address leaves this same
                // model-6 carrier on action 1. This is a controlled dispatch
                // check; the Tree producer owns its own integration control.
                fire.action = 1;
                (slot, admitted, fire.phase)
            };
            game.step(crate::PlayerCommand::default(), 0);
            assert!(
                game.events().iter().all(|event| !matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "outer turn errors: {:?}",
                game.events()
            );
            assert!(
                game.events().iter().any(|event| matches!(event,
                    crate::native_world::Mc2DispatchEvent::DynamicArena { source_slot, .. }
                        if *source_slot == slot
                )),
                "the current typed carrier was not visited"
            );
            let live = &game.simulation().session.state.live;
            let fire = live
                .economy
                .economy
                .authored_model6()
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("ground fire remains live after its first visit");
            assert_eq!(
                (fire.action, fire.life, fire.phase),
                (1, 239, initial_phase.wrapping_add(1))
            );
            assert_eq!(
                fire.control_flags & 2,
                2,
                "action1 runs its sound-once tail"
            );
            assert_eq!(fire.control_flags & 0x0008_0000 != 0, expected_admission);
            if expected_admission {
                let light = live
                    .economy
                    .dynamic_lighting
                    .active_slots()
                    .map(|(_, light)| light)
                    .find(|light| light.source_entity_index == slot)
                    .expect("ground fire links its actual source address");
                assert_eq!(
                    (light.intensity, light.jitter_range, light.life_scaled),
                    (80, 11, true)
                );
            }
        }
    }

    #[test]
    fn owner_cave_projectile_constructors_obey_light_option_and_capacity() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let campaign = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("real Cave4 owner package");
        let baseline = campaign.simulation();
        assert_eq!(
            baseline.session.state.live.terrain.map_type(),
            MapType::Cave
        );
        let particles = baseline.session.definition.presentation.particles.clone();

        for fireball in [true, false] {
            for (enabled, full) in [(true, false), (false, false), (true, true)] {
                let mut session = baseline.clone();
                let live = &mut session.session.state.live;
                let owner = live.core.state.entity_index;
                let tile = live
                    .terrain
                    .planes()
                    .shading
                    .iter()
                    .position(|shade| *shade < 32)
                    .expect("owner cave has a lightable tile");
                let mut position = Position {
                    x: ((tile & 255) as u16).wrapping_shl(8).wrapping_add(128),
                    y: ((tile >> 8) as u16).wrapping_shl(8).wrapping_add(128),
                    z: 0,
                };
                position.z = live.terrain.ground_height(position);
                live.economy.dynamic_lighting_enabled = enabled;
                if full {
                    while live.economy.dynamic_lighting.active_count() < 50 {
                        assert!(
                            live.economy
                                .dynamic_lighting
                                .admit(true, true, owner, position, 128, 1, false,)
                                .is_some()
                        );
                    }
                }
                let before = live.economy.dynamic_lighting.active_count();
                let slot = if fireball {
                    boss::construct(
                        &mut live.economy,
                        &mut live.terrain,
                        &particles,
                        boss::Launch {
                            product: boss::Product::FinalBossFireball,
                            position,
                            owner,
                            yaw: 0,
                            pitch: 0,
                            target_class: 3,
                            target_model: 0,
                            target: 0,
                            damage: 800,
                            effect_life: 0,
                            descriptor: None,
                        },
                    )
                    .unwrap()
                    .expect("real Fireball constructor allocates")
                } else {
                    lightning::construct(
                        &mut live.economy,
                        &mut live.terrain,
                        &particles,
                        lightning::Product::Ray,
                        position,
                    )
                    .unwrap()
                    .expect("real Lightning constructor allocates")
                };
                assert_eq!(
                    live.terrain.planes().entity_indices[tile],
                    slot as i16,
                    "constructor publishes the new source map head"
                );
                let admitted = enabled && !full;
                assert_eq!(
                    live.economy.dynamic_lighting.active_count(),
                    before + u8::from(admitted)
                );
                let flags = if fireball {
                    live.economy
                        .economy
                        .fireball_projectiles()
                        .iter()
                        .find(|row| row.active && row.source_entity_index == slot)
                        .expect("live typed Fireball")
                        .control_flags
                } else {
                    live.economy.entities[usize::from(slot)].control_flags
                };
                assert_eq!(flags & 0x0008_0000 != 0, admitted);
                if admitted {
                    let light = live
                        .economy
                        .dynamic_lighting
                        .active_slots()
                        .map(|(_, light)| light)
                        .find(|light| light.source_entity_index == slot)
                        .expect("constructor linked its source address");
                    assert_eq!(
                        (light.intensity, light.jitter_range, light.life_scaled),
                        (128, if fireball { 1 } else { 9 }, false)
                    );
                    assert_eq!(light.position, position);

                    before_updates(&live.economy.dynamic_lighting, &mut live.terrain);
                    let unlit = live.terrain.planes().shading.to_vec();
                    let mut lights = std::mem::take(&mut live.economy.dynamic_lighting);
                    let targets = crate::native_world::source_target::Context {
                        player: Some((owner, live.core.state.position, live.player_life)),
                        particles: Some(&particles),
                        ..crate::native_world::source_target::Context::from_ordinary(
                            &live.goats,
                            &live.ordinary,
                        )
                    };
                    after_updates(&mut lights, &live.economy, &mut live.terrain, targets)
                        .expect("live source light resolves its current occupant");
                    let light = lights
                        .active_slots()
                        .map(|(_, light)| light)
                        .find(|light| light.source_entity_index == slot)
                        .expect("projectile light remains active");
                    assert!(light.history.iter().any(|value| *value != 0));
                    let (_, shading) = live.terrain.dynamic_lighting_planes();
                    lights.subtract_previous(shading, true);
                    assert_eq!(live.terrain.planes().shading.as_ref(), unlit.as_slice());
                    live.economy.dynamic_lighting = lights;
                }
            }
        }
    }

    #[test]
    fn owner_cave_source_position_reuse_maps_and_lights_the_new_address() {
        use crate::level_definition::EffectKind;
        use crate::native_world::Level2EconomyEvent;

        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let campaign = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("real Cave4 owner package");
        let session = &campaign.simulation().session;
        let particles = &session.definition.presentation.particles;
        let mut arena = session.state.live.economy.clone();
        let mut terrain = session.state.live.terrain.clone();
        assert_eq!(terrain.map_type(), MapType::Cave);
        let source_position = Position {
            x: 0x2000,
            y: 0x3000,
            z: 1000,
        };
        arena.dynamic_lighting_enabled = false;
        let parent = crate::world::effects::allocate(
            &mut arena,
            &mut terrain,
            sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
            source_position,
            particles,
        )
        .unwrap()
        .expect("real impact is replaceable");
        let owner = session.state.live.core.state.entity_index;
        let source = &mut arena.entities[usize::from(parent)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        let previous = arena.handle(parent).unwrap();
        arena.rebuild_source_free_arena();
        // The real frozen fallback cursor is retained; exhaust the free
        // stack so the next NewEvent must use that source cursor.
        while arena.economy.free_slot_count() != 0 {
            arena
                .allocate_source_slot_with_terrain(&mut terrain)
                .expect("consume a real free-stack slot");
        }
        loop {
            let mut probe = arena.clone();
            let mut probe_terrain = terrain.clone();
            let candidate = crate::world::effects::allocate(
                &mut probe,
                &mut probe_terrain,
                sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
                source_position,
                particles,
            )
            .unwrap()
            .expect("fallback cursor reaches the existing impact");
            if candidate == parent {
                break;
            }
            assert_eq!(
                crate::world::effects::allocate(
                    &mut arena,
                    &mut terrain,
                    sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
                    source_position,
                    particles,
                )
                .unwrap(),
                Some(candidate)
            );
        }
        arena.dynamic_lighting_enabled = true;
        assert!(arena.dynamic_lighting.active_count() < 50);
        let mut mana_arena = arena.clone();
        let mut mana_terrain = terrain.clone();
        let before_lights = mana_arena.dynamic_lighting.active_count();
        let first_rng = mana_arena
            .economy
            .new_event_rng(parent)
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        let second_rng = first_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let mana_slot = crate::world::effects::spawn_death_mana_sphere(
            &mut mana_arena,
            &mut mana_terrain,
            parent,
            source_position,
            700,
            20_000,
            owner,
        )
        .unwrap()
        .expect("the real mana constructor reuses the source address");
        assert_eq!(mana_slot, parent);
        assert!(mana_arena.resolve(previous).is_none());
        let sphere = mana_arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.active && row.source_entity_index == parent)
            .unwrap();
        assert_eq!(sphere.position, Position::default());
        assert_eq!(
            (
                sphere.mana,
                sphere.player_entity_index,
                sphere.owner_entity_index
            ),
            (512, 0, 0)
        );
        assert_eq!(
            sphere.yaw,
            ((i32::from(first_rng % 113) - 56) & 2047) as i16
        );
        assert_eq!(sphere.entity_rng, second_rng);
        assert_eq!(mana_terrain.planes().entity_indices[0], parent as i16);
        assert_eq!(mana_arena.dynamic_lighting.active_count(), before_lights);
        assert!(
            crate::world::effects::clear_replacement_mana_owner(&mut mana_arena, parent).unwrap()
        );
        assert!(
            crate::world::effects::hide_birth_replacement(&mut mana_arena, parent, None).unwrap()
        );
        let sphere = mana_arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.active && row.source_entity_index == parent)
            .unwrap();
        assert!(sphere.pending_delete && sphere.control_flags & 0x400 != 0);

        for kind in [EffectKind::Impact, EffectKind::RadialExplosion] {
            let mut current = arena.clone();
            let mut current_terrain = terrain.clone();
            let before = current.dynamic_lighting.active_count();
            let slot = if kind == EffectKind::RadialExplosion {
                crate::world::effects::spawn_death_burst(
                    &mut current,
                    &mut current_terrain,
                    particles,
                    parent,
                    source_position,
                    owner,
                )
                .unwrap()
            } else {
                crate::world::effects::allocate_from_source_position(
                    &mut current,
                    &mut current_terrain,
                    sim_definition::adapter::mc2_effect_definition(kind),
                    parent,
                    source_position,
                    particles,
                )
                .unwrap()
            }
            .expect("the real fallback reuses the source address");
            assert_eq!(slot, parent);
            assert!(current.resolve(previous).is_none());
            let row = &current.entities[usize::from(slot)];
            assert_eq!(row.position, Position::default());
            assert_eq!(current_terrain.planes().entity_indices[0], slot as i16);
            assert_eq!(current.dynamic_lighting.active_count(), before + 1);
            assert!(
                current
                    .dynamic_lighting
                    .active_slots()
                    .any(|(_, light)| light.source_entity_index == slot
                        && light.position == Position::default())
            );
            if kind == EffectKind::RadialExplosion {
                assert_eq!(row.source_id, slot, "source id is read after NewEvent");
                assert!(current.economy.events().iter().any(|event| matches!(
                    event, Level2EconomyEvent::DeathBurstSpawned {
                        source_entity_index, parent_entity_index, id_entity_index,
                    } if *source_entity_index == slot && *parent_entity_index == parent
                        && *id_entity_index == slot
                )));
            }
            assert!(
                crate::world::effects::hide_birth_replacement(&mut current, parent, Some(slot),)
                    .unwrap()
            );
            assert!(current.entities[usize::from(slot)].drawing_disabled());
        }
        arena.finish_source_group();
        let before_lights = arena.dynamic_lighting.active_count();
        let before_events = arena.economy.events().len();
        assert!(
            crate::world::effects::spawn_death_mana_sphere(
                &mut arena,
                &mut terrain,
                parent,
                source_position,
                700,
                20_000,
                owner,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            crate::world::effects::spawn_death_burst(
                &mut arena,
                &mut terrain,
                particles,
                parent,
                source_position,
                owner,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(arena.dynamic_lighting.active_count(), before_lights);
        assert_eq!(arena.economy.events().len(), before_events);
        assert!(arena.resolve(previous).is_some());
    }

    #[test]
    fn owner_cave_outer_turn_services_and_retires_real_impact_light() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let mut game = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("real Cave4 owner package");
        let slot = {
            let session = &mut game.simulation_mut().session;
            let particles = &session.definition.presentation.particles;
            let live = &mut session.state.live;
            assert_eq!(live.terrain.map_type(), MapType::Cave);
            let tile = live
                .terrain
                .planes()
                .shading
                .iter()
                .position(|shade| *shade < 32)
                .expect("owner cave has a lightable tile");
            let mut position = Position {
                x: ((tile & 255) as u16).wrapping_shl(8).wrapping_add(128),
                y: ((tile >> 8) as u16).wrapping_shl(8).wrapping_add(128),
                z: 0,
            };
            position.z = live.terrain.ground_height(position);
            crate::world::effects::allocate(
                &mut live.economy,
                &mut live.terrain,
                sim_definition::adapter::mc2_effect_definition(
                    crate::level_definition::EffectKind::Impact,
                ),
                position,
                particles,
            )
            .unwrap()
            .expect("ordinary impact constructor allocates")
        };
        game.step(crate::PlayerCommand::default(), 0);
        assert!(
            game.events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "outer turn must complete: {:?}",
            game.events()
        );
        let light = game
            .simulation()
            .session
            .state
            .live
            .economy
            .dynamic_lighting
            .active_slots()
            .map(|(_, light)| light)
            .find(|light| light.source_entity_index == slot)
            .expect("scheduled outer turn retains the impact light");
        assert!(
            light.history.iter().any(|value| *value != 0),
            "outer turn applies the footprint"
        );

        game.simulation_mut().session.state.live.economy.entities[usize::from(slot)]
            .disable_drawing();
        game.step(crate::PlayerCommand::default(), 0);
        assert!(
            game.events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "following outer turn must complete: {:?}",
            game.events()
        );
        assert!(
            game.simulation()
                .session
                .state
                .live
                .economy
                .dynamic_lighting
                .active_slots()
                .all(|(_, light)| light.source_entity_index != slot),
            "disabled source retires its linked light after the ordinary update"
        );
    }

    #[test]
    fn owner_cave_open_applies_initial_light_setting_before_construction() {
        let mut checked = false;
        for mission in [4, 6, 8, 12, 15, 16, 21, 24] {
            let relative = if mission == 8 {
                "../../../generated/mc-bhq5-cave-reset-v2-level8".to_owned()
            } else if matches!(mission, 4 | 6) {
                format!("../../../generated/mc-bhq6-ordinary-20261024-b/level{mission}")
            } else {
                format!("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level{mission}")
            };
            let package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
            let selection = CampaignSelection::ordinary(mission).unwrap();
            let mut enabled =
                CampaignSession::open_with_dynamic_lighting(&package, selection, true)
                    .expect("owner cave opens with source lights enabled");
            let live = &enabled.simulation().session.state.live;
            if live.economy.dynamic_lighting.active_count() == 0 {
                continue;
            }
            let disabled = CampaignSession::open_with_dynamic_lighting(&package, selection, false)
                .expect("same owner cave opens with source lights disabled");
            let other = &disabled.simulation().session.state.live;
            assert!(!other.economy.dynamic_lighting_enabled);
            assert_eq!(other.economy.dynamic_lighting.active_count(), 0);
            let mut unlit = live.terrain.planes().shading.to_vec();
            live.economy
                .dynamic_lighting
                .subtract_previous(&mut unlit, true);
            assert_eq!(unlit.as_slice(), other.terrain.planes().shading.as_ref());
            for (_, light) in live.economy.dynamic_lighting.active_slots() {
                let slot = usize::from(light.source_entity_index);
                assert_ne!(live.economy.entities[slot].control_flags & 0x0008_0000, 0);
                assert_eq!(other.economy.entities[slot].control_flags & 0x0008_0000, 0);
            }
            let prior = live.economy.dynamic_lighting.clone();
            enabled.set_dynamic_lighting_enabled(false);
            let live = &enabled.simulation().session.state.live;
            assert!(!live.economy.dynamic_lighting_enabled);
            assert_eq!(
                live.economy.dynamic_lighting, prior,
                "toggle leaves existing lights intact"
            );
            checked = true;
            break;
        }
        assert!(checked, "owner caves must exercise a startup light");
    }
}
