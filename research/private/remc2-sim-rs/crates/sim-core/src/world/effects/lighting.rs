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
        crate::world::effects::sync_position(
            arena,
            terrain,
            slot,
            projectile.position,
            false,
            false,
        )?;
        let row = &mut arena.entities[usize::from(slot)];
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
        row.set_control_flags(projectile.control_flags | 0x400);
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
        if projectile.origin == crate::native_world::FireballProjectileOrigin::Creature27 {
            // sub_2A7F0 binds descriptor 106. The typed origin drives its
            // guidance while live; retain the source pointer on reclaim so a
            // later fixed-address read observes the same residual bytes.
            let descriptor = crate::world::effects::MODEL27_GUIDANCE;
            row.parameter_relation = Some(descriptor.source_index);
            row.record[164..168]
                .copy_from_slice(&(u32::from(descriptor.source_index) + 1).to_le_bytes());
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
