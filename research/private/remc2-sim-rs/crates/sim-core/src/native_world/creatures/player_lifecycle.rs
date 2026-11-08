//! Source player death and banishment transitions.

use crate::Position;
use crate::level_definition::{PlayerKind, PlayerLifecycleProperties};
use crate::native_world::{EntitySnapshot, EntitySpecialRelation, PlayerSnapshot};

pub(crate) mod notifications;

/// Project the same current marker carriers stepped by the source scheduler.
/// Dynamic slots are excluded from the imported-row presentation fallback.
pub(crate) fn presentation_candidates(
    markers: &[crate::native_world::Level2PlayerDeathMarker],
) -> impl Iterator<Item = (u16, u16, u16, i16, Position, u32)> + '_ {
    markers
        .iter()
        .filter(|marker| marker.active && marker.control_flags & 0x421 == 0)
        .map(|marker| {
            (
                marker.source_entity_index,
                marker.source_entity_index,
                marker.particle_parameter_index,
                0,
                marker.position,
                marker.control_flags,
            )
        })
}

/// The eight mutable spawn XY pairs written by constructors 4A820..4A900.
/// PlayerEvents indexes them by the physical player-array row. Source 5C950
/// replaces the stored Z with the current terrain height before using it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerSpawnPositions([(u16, u16); 8]);

impl PlayerSpawnPositions {
    pub(crate) fn write_source_constructor(&mut self, subtype: u16, position: Position) -> bool {
        let Some(index) = subtype.checked_sub(4).filter(|index| *index < 8) else {
            return false;
        };
        self.0[usize::from(index)] = (position.x, position.y);
        true
    }

    pub(crate) fn get(&self, player_row: u16) -> Option<Position> {
        self.0
            .get(usize::from(player_row))
            .map(|&(x, y)| Position { x, y, z: 0 })
    }

    pub(crate) fn append_hash(&self, hash: &mut sha2::Sha256) {
        use sha2::Digest;
        hash.update(b"player-source-spawns-v1");
        for (x, y) in self.0 {
            hash.update(x.to_le_bytes());
            hash.update(y.to_le_bytes());
        }
    }
}

/// Resolve the semantic role from the actual current player A4 carrier.
pub(crate) fn kind(player: &PlayerSnapshot) -> PlayerKind {
    if player.is_ai {
        PlayerKind::Rival
    } else {
        PlayerKind::Human
    }
}

/// The source dying burst uses the global predicted-axis value, not a
/// borrowed player-position address. NewEvent must see the full source arena.
pub(crate) fn spawn_dying_burst(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    player_slot: u16,
    predicted_position: Position,
) -> Result<Option<u16>, String> {
    if usize::from(player_slot) >= arena.entities.len() {
        return Err(format!(
            "dying player address {player_slot} is outside the arena"
        ));
    }
    let Some(slot) = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(
            crate::level_definition::EffectKind::RadialExplosion,
        ),
        predicted_position,
        particles,
    )?
    else {
        return Ok(None);
    };
    // The source reads the player ID after NewEvent, from its current fixed
    // address, and then writes it and the player-death light bit to the burst.
    let player_id = arena.entities[usize::from(player_slot)].source_id;
    let burst = &mut arena.entities[usize::from(slot)];
    burst.source_id = player_id;
    burst.record[30..32].copy_from_slice(&player_id.to_le_bytes());
    burst.set_control_flags(burst.control_flags | 0x80);
    crate::world::effects::register_death_burst(arena, slot, player_slot)?;
    Ok(Some(slot))
}

/// Source 10/40 constructor. Its position argument points into the current
/// player's record across NewEvent, so a replacement of that same physical
/// address observes NewEvent's cleared position.
pub(crate) fn construct_death_marker(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &mut crate::native_world::SessionTerrain,
    player_slot: u16,
    player_position: Position,
) -> Result<Option<crate::native_world::Level2PlayerDeathMarker>, String> {
    if usize::from(player_slot) >= arena.entities.len() {
        return Err(format!(
            "dying player address {player_slot} is outside the arena"
        ));
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if slot == player_slot {
        Position::default()
    } else {
        player_position
    };
    // The marker owns a typed carrier; NewEvent must also erase the old raw
    // projection before any current-address reader can see this birth.
    arena.retire_previous_source_projection(slot);
    let entity_rng = arena.economy.new_event_rng(slot);
    let marker = crate::native_world::Level2PlayerDeathMarker {
        source_entity_index: slot,
        entity_rng,
        trigger_player_entity_index: player_slot,
        player_entity_index: 0,
        pending_possession_owner: 0,
        pending_possession_payload: 0,
        position,
        dispatch_phase: slot as u8,
        constructor_phase: u32::from(slot % 11),
        byte_38: crate::level1_mission::PLAYER_DEATH_MARKER_BYTE_38,
        max_life: crate::level1_mission::PLAYER_DEATH_MARKER_LIFE,
        life: crate::level1_mission::PLAYER_DEATH_MARKER_LIFE,
        control_flags: 12,
        particle_parameter_index: crate::level1_mission::PLAYER_DEATH_MARKER_PARTICLE_PARAMETER,
        active: true,
    };
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
    {
        arena.economy.release_slot(slot, None);
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, entity_rng);
    arena.economy.source_owner.insert(slot, 0);
    arena.economy.player_death_markers.push(marker);
    Ok(Some(marker))
}

/// A failed marker birth leaves released-holder sentinel1 in the A4 array.
/// On the next grounded visit, source follows physical slot1 again. When the
/// current slot is a House, its family owns the action, flags, pose and life;
/// writing only the imported row would leave that actual carrier stale.
pub(crate) fn scatter_current_sentinel_house(
    arena: &mut crate::campaign::source_arena::SourceArena,
    houses: &mut crate::house::HouseFamily,
    terrain: &mut crate::native_world::SessionTerrain,
    player_slot: u16,
    player_position: Position,
    rng: &mut u16,
) -> Result<bool, String> {
    let Some(raw) = arena.entities.get(1) else {
        return Ok(false);
    };
    let kind = arena.economy.live_arena_kind(1);
    if (raw.class, raw.model) != (10, 45)
        || kind.is_some_and(|kind| {
            kind != crate::native_world::Level2DynamicArenaKind::AuthoredModel45
        })
    {
        return Ok(false);
    }
    let Some(house) = houses.house_mut(1).filter(|house| house.source_retained()) else {
        return Ok(false);
    };
    house.control_flags &= !1;
    house.action = house.action.wrapping_add(1);
    *rng = crate::native_world::level2_next_rng16(*rng);
    arena.entities[usize::from(player_slot)].set_entity_rng(*rng);
    let x = (*rng & 0x01ff) as i16 - 256;
    *rng = crate::native_world::level2_next_rng16(*rng);
    arena.entities[usize::from(player_slot)].set_entity_rng(*rng);
    let y = (*rng & 0x01ff) as i16 - 256;
    let position = Position {
        x: player_position.x.wrapping_add_signed(x),
        y: player_position.y.wrapping_add_signed(y),
        z: player_position.z,
    };
    house.position = position;
    house.publish_source(&mut arena.entities[1]);
    if let Some(row) = arena
        .economy
        .authored_model45
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == 1)
    {
        row.control_flags = house.control_flags;
        row.action = house.action;
        row.position = position;
    }
    crate::world::effects::sync_position(arena, terrain, 1, position, true, false)?;
    *rng = crate::native_world::level2_next_rng16(*rng);
    arena.entities[usize::from(player_slot)].set_entity_rng(*rng);
    house.life = i32::from(*rng % 90) + 200;
    house.publish_source(&mut arena.entities[1]);
    if let Some(row) = arena
        .economy
        .authored_model45
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == 1)
    {
        row.life = house.life;
    }
    Ok(true)
}

/// Position and vertical velocity after the falling part of `sub_5E310`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Fall {
    pub position: Position,
    pub next_velocity: i16,
    pub grounded: bool,
}

/// Source player pose after the human's banished-camera visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Camera {
    pub position: Position,
    pub yaw: i16,
    pub entity_roll: Option<i16>,
    pub entity_fov: Option<i16>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CameraReceipt {
    pub player: crate::campaign::source_arena::SourceEntityHandle,
    pub killer_slot: u16,
    pub player_position: Position,
    pub player_yaw: i16,
    pub target_position: Option<Position>,
    pub altitude: i16,
    pub result: Camera,
}

#[cfg(test)]
thread_local! {
    static CAMERA_TRACE: std::cell::RefCell<Option<(crate::campaign::source_arena::SourceEntityHandle, Vec<CameraReceipt>)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_camera_trace(player: crate::campaign::source_arena::SourceEntityHandle) {
    CAMERA_TRACE.with(|trace| *trace.borrow_mut() = Some((player, Vec::new())));
}

#[cfg(test)]
pub(crate) fn record_camera_visit(receipt: CameraReceipt) {
    CAMERA_TRACE.with(|trace| {
        if let Some((wanted, entries)) = trace.borrow_mut().as_mut()
            && *wanted == receipt.player
            && entries.len() < 64
        {
            entries.push(receipt);
        }
    });
}

#[cfg(test)]
pub(crate) fn take_camera_trace() -> Vec<CameraReceipt> {
    CAMERA_TRACE.with(|trace| {
        trace
            .borrow_mut()
            .take()
            .map_or_else(Vec::new, |(_, entries)| entries)
    })
}

/// Run `sub_5E6C0` with the already-resolved current killer position. A
/// nonzero unavailable source address is an error at the caller, not None.
pub(crate) fn camera(
    mut position: Position,
    yaw: i16,
    target: Option<Position>,
    terrain_altitude: i16,
) -> Camera {
    if let Some(target) = target {
        let heading = crate::heading_for_positions(position, target) as i16;
        let dx = i32::from(target.x.wrapping_sub(position.x).cast_signed());
        let dy = i32::from(target.y.wrapping_sub(position.y).cast_signed());
        let horizontal = crate::legacy_sqrt(
            dx.wrapping_mul(dx)
                .cast_unsigned()
                .wrapping_add(dy.wrapping_mul(dy).cast_unsigned()),
        ) as i32;
        let pitch =
            crate::pitch_for_delta(horizontal, i32::from(target.z.wrapping_sub(position.z)));
        position.z = terrain_altitude;
        Camera {
            position,
            yaw: yaw.wrapping_add(crate::turn_toward(yaw, heading, 22)) & 0x07ff,
            entity_roll: Some(heading),
            entity_fov: Some(pitch),
        }
    } else {
        position.z = terrain_altitude.wrapping_add(256);
        Camera {
            position,
            yaw: yaw.wrapping_add(5) & 0x07ff,
            entity_roll: None,
            entity_fov: None,
        }
    }
}

/// Run after the player's flight body and before its source-position copy and
/// dying burst. The altitude plus descriptor clearance is compared as a
/// promoted signed integer, as in the source, before a position is narrowed.
pub(crate) fn fall(
    rules: PlayerLifecycleProperties,
    mut position: Position,
    old_velocity: i16,
    terrain_altitude: i16,
    clearance: i16,
) -> Fall {
    position.z = position.z.wrapping_add(old_velocity);
    let next_velocity = old_velocity
        .wrapping_sub(rules.fall_acceleration)
        .clamp(rules.terminal_fall_velocity, 0);
    let ground = i32::from(terrain_altitude) + i32::from(clearance);
    if i32::from(position.z) < ground {
        position.z = ground as i16;
    }
    Fall {
        position,
        next_velocity,
        grounded: i32::from(position.z) == ground,
    }
}

/// Consequence of a reached AI action-3 visit. The caller performs the
/// source-addressed Castle revival or publishes the player's notification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AiBanishment {
    Waiting,
    Revive { castle_slot: u16 },
    Banished { announce: bool },
}

/// `sub_5E7C0` reads this actor's current A4 Castle word on every visit.
/// A nonzero word remains a source address; resolving its current occupant
/// and position is deferred until the zero-timer revival branch is reached.
pub(crate) fn step_ai_banished(
    actor: &mut EntitySnapshot,
    player: &PlayerSnapshot,
) -> Result<AiBanishment, String> {
    if kind(player) != PlayerKind::Rival
        || actor.special_relation != EntitySpecialRelation::PlayerState(player.slot)
    {
        return Err(format!(
            "player {} has no matching AI A4 carrier",
            actor.index
        ));
    }
    let castle_slot = u16::from_le_bytes([player.record[42], player.record[43]]);
    if castle_slot != 0 {
        let timer = actor.source_dword_10();
        if timer != 0 {
            actor.set_source_dword_10(timer.wrapping_sub(1));
            return Ok(AiBanishment::Waiting);
        }
        return Ok(AiBanishment::Revive { castle_slot });
    }
    let state = player
        .banishment_state_0x006
        .ok_or_else(|| format!("player {} has no banishment-state carrier", actor.index))?;
    Ok(AiBanishment::Banished {
        announce: state != 0,
    })
}

/// Called after the reached banishment announcement has been published.
pub(crate) fn finish_ai_banishment(player: &mut PlayerSnapshot) -> Result<(), String> {
    if !player.set_banishment_state(0) {
        return Err(format!(
            "player {} has no banishment-state carrier",
            player.entity_index
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AiBanishment, camera, fall, finish_ai_banishment, scatter_current_sentinel_house,
        step_ai_banished,
    };
    use crate::Position;
    use crate::level_definition::PLAYER_LIFECYCLE_PROPERTIES as RULES;

    #[test]
    fn source_spawn_constructors_preserve_row_order_and_later_writes() {
        let mut positions = super::PlayerSpawnPositions::default();
        let first = Position {
            x: 128,
            y: 384,
            z: 11,
        };
        let later = Position {
            x: 640,
            y: 896,
            z: -20,
        };
        assert!(positions.write_source_constructor(4, first));
        assert!(positions.write_source_constructor(11, later));
        assert!(positions.write_source_constructor(4, later));
        assert!(!positions.write_source_constructor(3, first));
        assert!(!positions.write_source_constructor(12, first));
        assert_eq!(positions.get(0), Some(Position { z: 0, ..later }));
        assert_eq!(positions.get(7), Some(Position { z: 0, ..later }));
        assert_eq!(positions.get(1), Some(Position::default()));
        assert_eq!(positions.get(8), None);
    }

    #[test]
    fn falling_uses_source_velocity_and_promoted_ground_comparison() {
        let pose = Position {
            x: 10,
            y: 20,
            z: 100,
        };
        let airborne = fall(RULES, pose, -10, 70, 10);
        assert_eq!(
            (
                airborne.position.z,
                airborne.next_velocity,
                airborne.grounded
            ),
            (90, -12, false)
        );
        let landed = fall(RULES, pose, -25, 70, 10);
        assert_eq!(
            (landed.position.z, landed.next_velocity, landed.grounded),
            (80, -27, true)
        );
        // The source compares against 32_768 before narrowing its position;
        // this cannot be represented by i16 and is not a grounded equality.
        let edge = fall(RULES, pose, 0, i16::MAX, 1);
        assert_eq!(
            (edge.position.z, edge.next_velocity, edge.grounded),
            (i16::MIN, -2, false)
        );
        assert_eq!(fall(RULES, pose, -256, 0, 0).next_velocity, -256);
        let changed = crate::level_definition::PlayerLifecycleProperties {
            fall_acceleration: 5,
            ..RULES
        };
        assert_eq!(fall(changed, pose, -10, 70, 10).next_velocity, -15);
    }

    #[test]
    fn banished_camera_distinguishes_zero_killer_from_current_target() {
        let pose = Position {
            x: 100,
            y: 200,
            z: 400,
        };
        let orbit = camera(pose, 2_046, None, 50);
        assert_eq!((orbit.position.z, orbit.yaw), (306, 3));
        assert_eq!((orbit.entity_roll, orbit.entity_fov), (None, None));
        let target = camera(pose, 0, Some(pose), 50);
        assert_eq!((target.position.z, target.yaw), (50, 0));
        assert_eq!((target.entity_roll, target.entity_fov), (Some(0), Some(0)));
    }

    #[test]
    fn configured_ai_banishment_reads_current_a4_and_preserves_source_order() {
        let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/creatures-20261104-integration/current-owner-package/level23",
            )
            },
            std::path::PathBuf::from,
        );
        let campaign = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
        )
        .expect("qualified whole Mission 23");
        let session = campaign.simulation();
        let mut actor = session.entities()[425].clone();
        let mut player = session
            .players()
            .iter()
            .find(|player| player.entity_index == 425)
            .expect("actual rival A4 carrier")
            .clone();
        let castle = u16::from_le_bytes([player.record[42], player.record[43]]);
        assert_ne!(castle, 0, "real rival has a current Castle address");
        actor.set_source_action(3);
        actor.set_source_dword_10(1_200);
        assert_eq!(
            step_ai_banished(&mut actor, &player).unwrap(),
            AiBanishment::Waiting
        );
        assert_eq!(actor.source_dword_10(), 1_199);
        actor.set_source_dword_10(i32::MIN);
        assert_eq!(
            step_ai_banished(&mut actor, &player).unwrap(),
            AiBanishment::Waiting
        );
        assert_eq!(actor.source_dword_10(), i32::MAX);
        actor.set_source_dword_10(0);
        assert_eq!(
            step_ai_banished(&mut actor, &player).unwrap(),
            AiBanishment::Revive {
                castle_slot: castle
            }
        );

        // Configured current-A4 exception: the old typed owner cache can still
        // contain the Castle, while the reached player word has become zero.
        assert!(session.economy().castle_for_owner(425).is_some());
        player.record[42..44].fill(0);
        assert!(player.set_banishment_state(1));
        assert_eq!(
            step_ai_banished(&mut actor, &player).unwrap(),
            AiBanishment::Banished { announce: true }
        );
        assert_eq!(
            player.banishment_state_0x006,
            Some(1),
            "notice precedes the source clear"
        );
        finish_ai_banishment(&mut player).unwrap();
        assert_eq!(player.banishment_state_0x006, Some(0));
        assert_eq!(
            step_ai_banished(&mut actor, &player).unwrap(),
            AiBanishment::Banished { announce: false }
        );
    }

    #[test]
    fn configured_marker_failure_retry_scatters_actual_slot_one_house_in_order() {
        let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../../../generated/creatures-20261104-integration/current-owner-package/level23",
            )
            },
            std::path::PathBuf::from,
        );
        let mut campaign = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::ordinary(23).unwrap(),
        )
        .expect("qualified whole Mission 23");
        let live = &mut campaign.simulation_mut().session.state.live;
        let player_slot = 425_u16;
        let position = live.economy.entities[usize::from(player_slot)].position;
        let mut rng = live.economy.entities[usize::from(player_slot)].entity_rng;
        let mut expected_rng = rng;
        let original = *live
            .ordinary
            .houses
            .houses()
            .iter()
            .find(|house| house.entity_index == 1)
            .expect("the actual source slot1 is a House");
        for visit in 0..26_u8 {
            let draw = |value: u16| value.wrapping_mul(9_377).wrapping_add(9_439);
            expected_rng = draw(expected_rng);
            let x = (expected_rng & 0x01ff) as i16 - 256;
            expected_rng = draw(expected_rng);
            let y = (expected_rng & 0x01ff) as i16 - 256;
            expected_rng = draw(expected_rng);
            assert!(
                scatter_current_sentinel_house(
                    &mut live.economy,
                    &mut live.ordinary.houses,
                    &mut live.terrain,
                    player_slot,
                    position,
                    &mut rng
                )
                .unwrap()
            );
            let house = live
                .ordinary
                .houses
                .houses()
                .iter()
                .find(|house| house.entity_index == 1)
                .unwrap();
            let expected_position = Position {
                x: position.x.wrapping_add_signed(x),
                y: position.y.wrapping_add_signed(y),
                z: position.z,
            };
            assert_eq!(
                (
                    rng,
                    live.economy.entities[usize::from(player_slot)].entity_rng
                ),
                (expected_rng, expected_rng)
            );
            assert_eq!(
                (
                    house.action,
                    house.control_flags & 1,
                    house.position,
                    house.life
                ),
                (
                    original.action.wrapping_add(visit + 1),
                    0,
                    expected_position,
                    i32::from(expected_rng % 90) + 200
                )
            );
            let raw = &live.economy.entities[1];
            assert_eq!(
                (raw.action, raw.control_flags & 1, raw.position, raw.life),
                (house.action, 0, house.position, house.life)
            );
            assert!(
                live.economy
                    .map_chains
                    .entity_chain(
                        (expected_position.x >> 8) as u8,
                        (expected_position.y >> 8) as u8
                    )
                    .contains(&1)
            );
        }
    }
}
