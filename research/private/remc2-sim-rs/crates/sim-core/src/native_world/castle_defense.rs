//! Source Castle controllers use the same projectile owners as player spells.
//! The Castle child carries its common fields; this owner retains only the
//! additional words used by sub_3AF00, qualified by the actual arena generation.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::campaign::source_arena::{SourceArena, SourceEntityHandle};
use crate::level1_mission::{source_map_search_slots, source_search_offsets};
use crate::{FlightWorld, Position, move_entity};

use super::{
    FireballProjectileOrigin, Level2CastleChild, ParticleParameter, SessionTerrain,
    level2_dynamic_economy::Level2FireballProjectileLaunchPayload, source_target,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Control {
    pitch: i16,
    orbit: i8,
    rise: i16,
    choice: u16,
    ammunition: i8,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            pitch: 0,
            orbit: 0,
            rise: 0,
            choice: 0,
            ammunition: 1,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State(BTreeMap<u16, (u32, Control)>);

impl State {
    pub(crate) fn forget(&mut self, slot: u16) {
        self.0.remove(&slot);
    }

    pub(crate) fn hash_into(&self, hash: &mut Sha256) {
        if self.0.is_empty() {
            return;
        }
        hash.update(b"castle-defense-controller-state-v1");
        for (&slot, &(generation, state)) in &self.0 {
            hash.update(slot.to_le_bytes());
            hash.update(generation.to_le_bytes());
            hash.update(state.pitch.to_le_bytes());
            hash.update([state.orbit as u8]);
            hash.update(state.rise.to_le_bytes());
            hash.update(state.choice.to_le_bytes());
            hash.update([state.ammunition as u8]);
        }
    }

    fn current(&self, handle: SourceEntityHandle) -> Control {
        self.0
            .get(&handle.source_slot)
            .filter(|(generation, _)| *generation == handle.generation)
            .map_or_else(Control::default, |(_, state)| *state)
    }

    fn store(&mut self, handle: SourceEntityHandle, state: Control) {
        if state == Control::default() {
            self.forget(handle.source_slot);
        } else {
            self.0
                .insert(handle.source_slot, (handle.generation, state));
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Step {
    pub(crate) live: bool,
    /// sub_6DCA0 emits the first successful shot's sound from the Castle.
    pub(crate) sound: Option<(u16, u16)>,
    pub(crate) projectile: Option<u16>,
}

pub(crate) fn is_controller(arena: &SourceArena, slot: u16) -> bool {
    arena.economy.castle_children.iter().any(|row| {
        row.active
            && row.source_entity_index == slot
            && (row.source_class, row.model, row.action) == (10, 79, 86)
    })
}

fn eligible(target: source_target::View, owner: u16) -> bool {
    target.source_id != owner
        && match target.class {
            3 => matches!(target.model, 0 | 1 | 3),
            5 => {
                target.model != 22
                    && !(target.relations.stage_mode == Some(14)
                        && target.relations.parent == Some(owner))
            }
            _ => false,
        }
}

fn find_target(
    arena: &SourceArena,
    context: source_target::Context<'_>,
    child: Level2CastleChild,
) -> Result<Option<u16>, String> {
    // AddE7EE0x(3,12): SEARCH.DAT class order, then the current map chain.
    let lower_classes = source_search_offsets(2).len();
    let mut offsets = source_search_offsets(12);
    // sub_10130 returns 2 for the final coordinate; this caller consumes
    // only return value 1, as do the shared projectile collision searches.
    offsets.pop();
    for slot in source_map_search_slots(
        &arena.economy.map_chains,
        child.position,
        &offsets[lower_classes..],
    ) {
        if let Some(view) = source_target::read(arena, context, slot)?
            && eligible(view, child.id_entity_index)
        {
            return Ok(Some(slot));
        }
    }
    Ok(None)
}

fn aim(target: source_target::View, position: Position) -> Result<(i16, i16), String> {
    let shape = target.collision.ok_or_else(|| {
        format!(
            "Castle defense target {} has no current collision axes",
            target.slot
        )
    })?;
    let center =
        super::guided_projectile::centre(target.position, target.model, shape.vertical_offset);
    Ok(crate::level1_mission::projectile_guidance_angles(
        position, center,
    ))
}

fn shoot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    context: source_target::Context<'_>,
    child: &mut Level2CastleChild,
    control: &mut Control,
    castle_slot: u16,
    local_owner: u16,
) -> Result<Step, String> {
    let target = if child.target_sphere_slot == 0 {
        None
    } else {
        source_target::read(arena, context, child.target_sphere_slot)?
            .filter(|row| row.life >= 0 && !row.drawing_disabled())
    };
    let Some(target) = target else {
        child.target_sphere_slot = 0;
        child.cave_collision_cooldown = 1;
        return Ok(Step {
            live: true,
            ..Step::default()
        });
    };
    let (family, tier) = match control.choice {
        0 => (0, 1),
        1 => (7, 0),
        2 => (7, 1),
        3 => (0, 2),
        4 => (9, 0),
        choice => {
            return Err(format!(
                "Castle controller has invalid shot choice {choice}"
            ));
        }
    };
    let profile = crate::source_sprite_tmap::castle_defense_shot_profile(family, tier)
        .map_err(|error| error.to_string())?;
    let controller_handle = arena.handle(child.source_entity_index);
    let first = child.cave_collision_cooldown == 7;
    let projectile = match family {
        0 => {
            let slot = arena.allocate_source_slot_with_terrain(terrain);
            if let Some(slot) = slot {
                let payload = Level2FireballProjectileLaunchPayload {
                    origin: FireballProjectileOrigin::CastleDefense,
                    position: if slot == child.source_entity_index {
                        Position::default()
                    } else {
                        child.position
                    },
                    yaw: 0,
                    pitch: 0,
                    axis_position: Position::default(),
                    source_id_entity_index: 0,
                    holder_entity_index: 0,
                    source_subspell_index: profile.source_subspell_index,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                };
                let initialized = arena
                    .economy
                    .initialize_fireball_projectile(slot, payload, 0);
                arena
                    .economy
                    .apply_fireball_tier_constructor(slot, tier as i8);
                initialized
            } else {
                None
            }
        }
        7 => {
            let product = if tier == 0 {
                super::lightning::Product::Ray
            } else {
                super::lightning::Product::StormShot
            };
            let slot = super::lightning::construct_at_address(
                arena,
                terrain,
                particles,
                product,
                child.position,
                Some(child.source_entity_index),
            )?;
            slot
        }
        9 => {
            let slot = super::meteor::construct(
                arena,
                terrain,
                particles,
                child.position,
                Some(child.source_entity_index),
                0,
                tier,
            )?;
            slot
        }
        _ => unreachable!("source Castle choices select three spell families"),
    };
    // sub_3AF00 retains addresses across sub_6DCA0, not copies of their
    // occupants. NewEvent may have replaced either address. Read the target
    // and the constructed flyer's position only after the constructor.
    let mut launch_angles = None;
    if let Some(slot) = projectile {
        let current_target = source_target::read(arena, context, target.slot)?
            .ok_or("Castle target address has no current occupant after NewEvent")?;
        let flyer = source_target::read(arena, context, slot)?
            .ok_or("Castle projectile constructor did not publish its current occupant")?;
        let (owner, relation, lift) =
            if arena.handle(child.source_entity_index) == controller_handle {
                (
                    child.id_entity_index,
                    child.target_sphere_slot,
                    (particles
                        .get(66)
                        .ok_or("Castle controller requires retained particle66")?
                        .rotation_speed
                        / 2) as i16,
                )
            } else {
                let current = source_target::read(arena, context, child.source_entity_index)?
                    .ok_or("Castle controller address lost its replacement occupant")?;
                (
                    current.source_id,
                    current.relations.target.unwrap_or(0),
                    current
                        .collision
                        .ok_or("Castle controller replacement has no current collision axes")?
                        .vertical_offset,
                )
            };
        let (yaw, pitch) = aim(current_target, flyer.position)?;
        let position = Position {
            z: flyer.position.z.wrapping_add(lift),
            ..flyer.position
        };
        if family == 0 {
            let row = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or("Castle Fireball constructor lost its current carrier")?;
            row.owner_entity_index = owner;
            row.control_flags |= 4;
            row.target_source_entity_index = relation;
            row.yaw = yaw;
            row.roll = yaw;
            row.pitch = pitch;
            row.fov = pitch;
            if owner == local_owner {
                row.visual_index = 42;
            }
        } else {
            let row = &mut arena.entities[usize::from(slot)];
            row.record[71] = profile.xclass;
            row.record[72] = profile.xmodel;
            row.set_subspell_index(profile.spell_subspell_index);
            row.set_yaw(yaw);
            row.set_pitch(pitch);
            row.record[36..38].copy_from_slice(&yaw.to_le_bytes());
            row.fov = pitch;
            row.record[38..40].copy_from_slice(&pitch.to_le_bytes());
            row.source_id = owner;
            row.record[30..32].copy_from_slice(&owner.to_le_bytes());
            row.record[154..156].copy_from_slice(&relation.to_le_bytes());
        }
        arena.economy.source_owner.insert(slot, owner);
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
        launch_angles = Some((yaw, pitch));
    }
    if first && projectile.is_some() {
        crate::mark_source_audio_call(crate::SourceAudioCallTag::ProjectileDeliveryLaunch);
    }
    if let Some((yaw, pitch)) = launch_angles {
        child.yaw = yaw as u16 & 2047;
        control.pitch = pitch & 2047;
        control.orbit = if control.orbit == 0 {
            1
        } else {
            control.orbit.wrapping_add(1).min(5)
        };
        control.ammunition = control.ammunition.wrapping_sub(1);
        if control.ammunition == 0 {
            child.target_sphere_slot = 0;
            child.cave_collision_cooldown = 1;
        } else {
            child.cave_collision_cooldown = 8;
        }
    } else {
        child.cave_collision_cooldown = 8;
    }
    Ok(Step {
        live: true,
        projectile,
        sound: (first && projectile.is_some()).then_some((castle_slot, profile.sound_id)),
    })
}

/// One ordinary source callback. The caller forwards the returned first-shot
/// audio; this owner commits real projectile births and map/lifetime changes.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    context: source_target::Context<'_>,
    slot: u16,
    local_owner: u16,
) -> Result<Step, String> {
    let handle = arena.handle(slot).ok_or("Castle controller is absent")?;
    let index = arena
        .economy
        .castle_children
        .iter()
        .position(|row| {
            row.active
                && row.source_entity_index == slot
                && (row.source_class, row.model, row.action) == (10, 79, 86)
        })
        .ok_or("Castle controller has no current typed carrier")?;
    let mut child = arena.economy.castle_children[index];
    let mut control = arena.economy.castle_defense.current(handle);
    let castle = arena
        .economy
        .castle_for_owner(child.id_entity_index)
        .copied();
    if child.life < 0 || child.id_entity_index == 0 {
        child.control_flags |= 0x400;
        child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        arena.economy.castle_children[index] = child;
        return Ok(Step {
            live: true,
            ..Step::default()
        });
    }
    if matches!(child.cave_collision_cooldown, 9 | 10) {
        if child.cave_collision_cooldown == 10 {
            crate::world::effects::spawn_death_burst(
                arena,
                terrain,
                particles,
                slot,
                child.position,
                child.id_entity_index,
            )?;
        }
        if arena.handle(slot) == Some(handle) {
            // The typed child owns the retained RNG and position, including
            // the old tile needed by the common fixed-address unlink.
            let row = &mut arena.entities[usize::from(slot)];
            row.entity_rng = child.entity_rng;
            row.record[24..26].copy_from_slice(&child.entity_rng.to_le_bytes());
            row.set_position(child.position);
        }
        arena
            .economy
            .castle_children
            .retain(|row| row.source_entity_index != slot);
        super::cave_features::reclaim(arena, terrain, slot);
        arena.economy.castle_defense.forget(slot);
        return Ok(Step::default());
    }
    if castle.is_none() {
        child.control_flags |= 0x400;
        child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        arena.economy.castle_children[index] = child;
        return Ok(Step {
            live: true,
            ..Step::default()
        });
    }
    let mut result = Step {
        live: true,
        ..Step::default()
    };
    match child.cave_collision_cooldown {
        0 => {
            child.axis_position = child.position;
            child.cave_collision_cooldown = 1;
        }
        1 => {
            child.entity_rng = child.entity_rng.wrapping_mul(9377).wrapping_add(9439);
            child.source_dword_10 = i32::from(child.entity_rng % 48) + 15;
            child.cave_collision_cooldown = 2;
        }
        2 => {
            child.source_dword_10 = child.source_dword_10.wrapping_sub(1);
            if child.source_dword_10 == 0 {
                child.cave_collision_cooldown = 3;
            }
        }
        3 => {
            if child.source_byte_3e & 63 == 0
                && let Some(target) = find_target(arena, context, child)?
            {
                child.target_sphere_slot = target;
                child.cave_collision_cooldown = 4;
            }
        }
        4 | 5 => {
            if child.cave_collision_cooldown == 4 {
                child.cave_collision_cooldown = 5;
                child.source_dword_10 = 4;
            }
            child.source_dword_10 = child.source_dword_10.wrapping_sub(1);
            if child.source_dword_10 == 0 {
                child.cave_collision_cooldown = 6;
                control.rise = 0;
            } else {
                control.rise = control.rise.wrapping_add(160);
            }
        }
        6..=8 => {
            if child.cave_collision_cooldown == 6 {
                child.axis_position.z = child.position.z;
                child.entity_rng = child.entity_rng.wrapping_mul(9377).wrapping_add(9439);
                let selection = child.entity_rng % 100;
                control.choice = if selection == 0 {
                    4
                } else if selection <= 5 {
                    u16::from(child.source_byte_43 == 1) + 2
                } else {
                    u16::from(child.source_byte_43 != 1)
                };
                control.ammunition = if control.choice <= 1 { 6 } else { 1 };
                child.cave_collision_cooldown = 7;
            }
            result = shoot(
                arena,
                terrain,
                particles,
                context,
                &mut child,
                &mut control,
                castle.unwrap().source_entity_index,
                local_owner,
            )?;
        }
        _ => {}
    }
    // NewEvent can replace the callback's own fixed address. Never publish
    // its pre-constructor typed carrier over a different current occupant.
    if arena.handle(slot) != Some(handle) {
        return Ok(result);
    }
    if control.orbit != 0 {
        let radius = match control.orbit.unsigned_abs() {
            1 => 0,
            2 => 115,
            3 => 230,
            4 => 334,
            5 => 368,
            6 => 384,
            value => return Err(format!("Castle controller has invalid orbit {value}")),
        };
        child.position = child.axis_position;
        move_entity(
            &mut child.position,
            child.yaw as i16,
            control.pitch,
            -radius,
        );
        control.orbit = control.orbit.wrapping_add(1);
        if control.orbit > 6 {
            control.orbit = -5;
        }
    }
    let floor = terrain
        .ground_height(child.position)
        .wrapping_add(if child.source_word_4a <= 1 { 384 } else { 224 });
    if child.position.z < floor {
        child.position.z = floor;
    } else if control.rise != 0 {
        child.position.z = floor.wrapping_add(control.rise);
    } else if control.orbit == 0 {
        let delta = i32::from(child.position.z) - i32::from(floor);
        if delta.abs() > 32 {
            child.position.z = child
                .position
                .z
                .wrapping_add(if delta <= 0 { 16 } else { -16 });
        }
        child.position.z = child
            .position
            .z
            .wrapping_add(if child.source_byte_3e & 8 != 0 { 6 } else { -6 });
    }
    child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
    let Some(index) = arena
        .economy
        .castle_children
        .iter()
        .position(|row| row.active && row.source_entity_index == slot)
    else {
        return Ok(result);
    };
    arena.economy.castle_children[index] = child;
    arena.economy.castle_defense.store(handle, control);
    arena.economy.source_rng.insert(slot, child.entity_rng);
    arena.economy.sync_castle_child_map(child);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{Control, eligible, step};
    use crate::native_world::{
        FireballProjectileOrigin, Mc2DispatchEvent, NativeWorld, source_target,
    };
    use crate::{FlightWorld, PlayerCommand, Position};

    #[test]
    fn actual_castle_controller_fires_shared_spell_products_and_reclaims_them() {
        let package = crate::native_world::tests::level_two_fixture_path();
        let cache = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &cache).unwrap();
        let footprint = inspection
            .level2_castle_stage_footprints
            .get(&1)
            .unwrap()
            .clone();
        // Each variant starts at the real Castle/BUILD producer. Only the
        // RNG at the choice boundary is controlled, to cover each source arm.
        for (variant, random_remainder, family, tier) in [
            (1, 6, 0, 1),
            (2, 6, 7, 0),
            (2, 1, 7, 1),
            (1, 1, 0, 2),
            (1, 0, 9, 0),
        ] {
            let mut session = inspection.level2_reset_session().unwrap();
            session.select_level2_campaign_entry_stage();
            assert!(
                session
                    .economy_mut()
                    .set_castle_controller_coefficient(1, variant)
            );
            let castle = session
                .economy_mut()
                .build_castle_with_footprint(
                    Position {
                        x: 0x2000,
                        y: 0x3000,
                        z: 0,
                    },
                    111,
                    &footprint,
                )
                .unwrap();
            session.step_outer_turn(PlayerCommand::default(), 0);
            let controller = session
                .economy()
                .castle_children()
                .iter()
                .find(|row| row.active && row.model == 79 && row.id_entity_index == 111)
                .copied()
                .expect("ordinary Castle stage publishes controller79");
            let mut position = Position {
                x: controller.position.x.wrapping_add(1024),
                y: controller.position.y,
                z: 0,
            };
            position.z = session
                .canonical_terrain()
                .ground_height(position)
                .wrapping_add(128);
            let enemy_source = session
                .economy_mut()
                .allocate_authored_archer(232, position)
                .expect("real authored Archer producer");
            let enemy = enemy_source.source_entity_index;
            let initial_life = enemy_source.life;
            assert!(initial_life > 0);
            for _ in 0..160 {
                let phase = session
                    .economy()
                    .castle_children()
                    .iter()
                    .find(|row| {
                        row.active && row.source_entity_index == controller.source_entity_index
                    })
                    .unwrap()
                    .cave_collision_cooldown;
                if phase == 6 {
                    break;
                }
                assert!(
                    session
                        .economy_mut()
                        .step_castle_child_terminal_slot(controller.source_entity_index)
                );
            }
            {
                let mut view = session.economy_mut();
                let current = view
                    .castle_children
                    .iter_mut()
                    .find(|row| {
                        row.active && row.source_entity_index == controller.source_entity_index
                    })
                    .unwrap();
                assert_eq!(
                    (current.cave_collision_cooldown, current.target_sphere_slot),
                    (6, enemy)
                );
                current.entity_rng = (0..=u16::MAX)
                    .find(|seed| {
                        seed.wrapping_mul(9377).wrapping_add(9439) % 100 == random_remainder
                    })
                    .unwrap();
            }
            let result = {
                let live = &mut session.session.state.live;
                let context = source_target::Context {
                    spells: Some(&live.shared_spells),
                    particles: Some(&session.session.definition.presentation.particles),
                    ..source_target::Context::from_ordinary(&live.goats, &live.ordinary)
                };
                step(
                    &mut live.economy,
                    &mut live.terrain,
                    &session.session.definition.presentation.particles,
                    context,
                    controller.source_entity_index,
                    111,
                )
                .unwrap()
            };
            assert_eq!(result.sound.map(|(slot, _)| slot), Some(castle));
            let projectile = result
                .projectile
                .expect("controller creates a real shared flyer");
            let handle = session.source_arena().handle(projectile).unwrap();
            let presentation = session.campaign_entity_presentations(0).unwrap();
            let visible = presentation
                .iter()
                .find(|row| row.entity_index == projectile)
                .expect("the actual presentation consumer observes the new flyer");
            assert_eq!(
                visible.render_parameter_index,
                match family {
                    0 => 42,
                    7 => 216,
                    9 => 76,
                    _ => unreachable!(),
                }
            );
            if family == 0 {
                let row = session
                    .economy()
                    .fireball_projectiles()
                    .iter()
                    .find(|row| row.active && row.source_entity_index == projectile)
                    .unwrap();
                assert_eq!(row.origin, FireballProjectileOrigin::CastleDefense);
                assert_eq!(
                    (row.source_model, row.action),
                    if tier == 2 { (28, 29) } else { (0, 0) }
                );
                assert_eq!(
                    (
                        row.owner_entity_index,
                        row.target_source_entity_index,
                        row.visual_index
                    ),
                    (111, enemy, 42)
                );
            } else {
                let row = &session.source_arena().entities[usize::from(projectile)];
                assert_eq!(
                    (row.class, row.model, row.action),
                    match (family, tier) {
                        (7, 0) => (9, 9, 9),
                        (7, 1) => (9, 12, 12),
                        (9, 0) => (9, 3, 3),
                        _ => unreachable!(),
                    }
                );
                assert_eq!((row.source_id, row.source_word_96()), (111, enemy));
            }
            let mut saw_damage = false;
            let mut saw_reclaim = false;
            for pass in 0..80 {
                session.step_level2_source_pass(pass & 3);
                assert!(
                    !session
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                    "Castle family {family}/{tier} dispatch: {:?}",
                    session.dispatch_events()
                );
                saw_damage |= session
                    .source_arena()
                    .authored_class5()
                    .iter()
                    .any(|row| row.source_entity_index == enemy && row.life < initial_life);
                saw_reclaim |= session.source_arena().resolve(handle).is_none();
                if saw_damage && saw_reclaim {
                    break;
                }
            }
            assert!(
                saw_damage,
                "Castle family {family}/{tier} must damage the real creature"
            );
            assert!(
                saw_reclaim,
                "Castle family {family}/{tier} must reclaim its original flyer"
            );
            let presentation = session.campaign_entity_presentations(0).unwrap();
            assert!(
                !presentation
                    .iter()
                    .any(|row| row.entity_index == projectile)
                    || session.source_arena().handle(projectile) != Some(handle),
                "reclaimed flyer cannot remain in presentation under its old generation"
            );
        }
    }

    #[test]
    fn shared_target_gate_excludes_current_allies_and_source_identity() {
        let mut view = source_target::View {
            slot: 53,
            source_id: 53,
            active: true,
            class: 5,
            model: 19,
            position: Position::default(),
            life: 600,
            control_flags: 8,
            yaw: Some(0),
            roll: Some(0),
            collision: None,
            relations: source_target::Relations {
                parent: Some(111),
                stage_mode: Some(14),
                ..Default::default()
            },
        };
        assert!(!eligible(view, 111));
        assert!(eligible(view, 112));
        view.relations.stage_mode = Some(10);
        assert!(eligible(view, 111));
        view.source_id = 111;
        assert!(!eligible(view, 111));
        assert_eq!(Control::default().ammunition, 1);
    }
}
