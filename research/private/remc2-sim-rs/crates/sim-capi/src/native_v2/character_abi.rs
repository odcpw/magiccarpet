//! Read-only character animation inputs from the same authoritative campaign.
//! An arrow row is a successful native allocation, not an animation timer.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McCampaignCharacterStateV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub slot_generation: u32,
    pub parent_generation: u32,
    pub entity_index: u16,
    pub source_id: u16,
    pub parent_index: u16,
    pub parent_id: u16,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub active: u8,
    pub has_target: u8,
    pub has_parent: u8,
    pub stance: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub life: i32,
    pub target_index: u16,
    pub target_x: u16,
    pub target_y: u16,
    pub target_z: i16,
    pub control_flags: u32,
    pub age_ticks: u16,
    /// Source appearance parameter for class-5/model-13; zero otherwise.
    pub render_parameter: u16,
}

/// A retained native death, tied to the world/session and slot incarnation.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McCampaignVoxelDeathV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub sequence: u64,
    pub slot_generation: u32,
    pub entity_index: u16,
    pub source_id: u16,
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_voxel_deaths_copy(
    handle: u64,
    generation: u64,
    tick: u64,
    after_sequence: u64,
    buffer: *mut McCampaignVoxelDeathV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignVoxelDeathV1>(element_size, "voxel death")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                return Err(AbiError::new(MC_STATUS_INVALID_ARGUMENT, "voxel deaths require a descriptor campaign"));
            };
            let terrain = reset.canonical_terrain();
            let world = terrain.voxel_world().ok_or_else(||
                AbiError::new(MC_STATUS_INVALID_ARGUMENT, "voxel geometry is disabled"))?;
            // The command/session generation can advance on pause and resume
            // while the same world and its death journal remain alive. Guard
            // the query with current session identity; label rows with the
            // stable world epoch so that rebinds do not replay old deaths.
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(MC_STATUS_GENERATION_MISMATCH, format!(
                    "stale voxel death query: requested generation {generation} tick {tick}, session generation {} tick {}",
                    session.generation, reset.campaign_tick())));
            }
            let journal = terrain.voxel_deaths().expect("voxel world has a death journal");
            let rows = journal.rows();
            if after_sequence != 0 && rows.front().is_some_and(|first| after_sequence < first.sequence - 1) {
                return Err(AbiError::new(MC_STATUS_BOUNDS, "voxel death cursor fell behind retained history"));
            }
            if after_sequence > rows.back().map_or(0, |last| last.sequence) {
                return Err(AbiError::new(MC_STATUS_INVALID_ARGUMENT, "voxel death cursor is ahead of history"));
            }
            let count = rows.iter().filter(|row| row.sequence > after_sequence).count();
            if count > capacity as usize {
                return Err(AbiError::new(MC_STATUS_BOUNDS, "voxel death buffer is too small"));
            }
            if count != 0 && buffer.is_null() {
                return Err(AbiError::new(MC_STATUS_NULL_POINTER, "voxel death buffer is null"));
            }
            // SAFETY: the caller supplies writable space for capacity elements;
            // element size, produced count and non-null output were checked.
            unsafe {
                for (index, row) in rows.iter().filter(|row| row.sequence > after_sequence).enumerate() {
                    buffer.add(index).write(McCampaignVoxelDeathV1 {
                        abi_version: MC_NATIVE_ABI_VERSION, struct_size: struct_size::<McCampaignVoxelDeathV1>(),
                        generation: world.generation(), sequence: row.sequence, slot_generation: row.slot_generation,
                        entity_index: row.slot, source_id: row.source_id,
                    });
                }
                out_written.write(count as u32);
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_character_states_copy(
    handle: u64,
    generation: u64,
    tick: u64,
    buffer: *mut McCampaignCharacterStateV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignCharacterStateV1>(element_size, "character state")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "character state requires a descriptor campaign",
                ));
            };
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale character state query",
                ));
            }
            let mut rows = Vec::new();
            let base = McCampaignCharacterStateV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignCharacterStateV1>(),
                generation,
                tick,
                ..Default::default()
            };
            let mut targets = reset
                .entities()
                .iter()
                .filter(|s| s.active)
                .map(|s| (s.index, s.position))
                .collect::<BTreeMap<_, _>>();
            let player = reset.shared_spells().player_entity();
            targets.insert(player.index, player.position);
            for source in reset
                .economy()
                .authored_class5()
                .iter()
                .filter(|s| s.active)
            {
                targets.insert(source.source_entity_index, source.position);
            }
            for goat in reset.goats().iter().filter(|s| s.active) {
                targets.insert(goat.entity_index, goat.position);
            }
            let live_slots = reset.economy().live_arena_slots().into_iter().collect::<BTreeMap<_, _>>();
            // Imported slots retain generation zero until NewEvent replaces
            // them; dynamically authored goats use the same arena identity.
            for goat in reset.goats().iter().filter(|s| s.active) {
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: reset.economy().slot_generation(goat.entity_index),
                    entity_index: goat.entity_index,
                    source_id: goat.source_id,
                    class_id: 5,
                    model: 1,
                    action: goat.action,
                    phase: goat.phase,
                    active: u8::from(goat.active),
                    x: goat.position.x,
                    y: goat.position.y,
                    z: goat.position.z,
                    yaw: goat.yaw,
                    pitch: goat.pitch,
                    speed: goat.actual_speed,
                    life: goat.life,
                    control_flags: goat.control_flags,
                    ..base
                });
            }
            // Reset villagers and house-born residents have distinct source
            // owners. An arena replacement must never inherit a retained row.
            for villager in reset.ordinary().villagers().iter().filter(|s|
                s.active && !live_slots.contains_key(&s.entity_index))
            {
                let target = villager.target_entity_index.and_then(|index| targets.get(&index));
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: reset.economy().slot_generation(villager.entity_index),
                    entity_index: villager.entity_index, source_id: villager.source_id,
                    class_id: 5, model: 13, action: villager.action, phase: villager.phase,
                    active: 1, x: villager.position.x, y: villager.position.y, z: villager.position.z,
                    yaw: villager.yaw, pitch: villager.pitch, speed: villager.actual_speed,
                    life: villager.life, control_flags: villager.control_flags,
                    has_target: u8::from(target.is_some()),
                    target_index: villager.target_entity_index.unwrap_or(0),
                    target_x: target.map_or(0, |p| p.x), target_y: target.map_or(0, |p| p.y),
                    target_z: target.map_or(0, |p| p.z),
                    render_parameter: villager.render_parameter_index,
                    ..base
                });
            }
            for resident in reset.economy().house_residents().iter().filter(|s|
                s.active && s.model == 13 && live_slots.get(&s.source_entity_index)
                    == Some(&sim_core::native_world::Level2DynamicArenaKind::HouseResident))
            {
                let target = targets.get(&resident.target_entity_index);
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: reset.economy().slot_generation(resident.source_entity_index),
                    entity_index: resident.source_entity_index, source_id: resident.source_id,
                    class_id: 5, model: 13, action: resident.action, phase: resident.phase,
                    active: u8::from(resident.drawing_enabled),
                    x: resident.position.x, y: resident.position.y, z: resident.position.z,
                    yaw: resident.yaw, speed: resident.actual_speed, life: resident.life,
                    control_flags: resident.control_flags, has_target: u8::from(target.is_some()),
                    target_index: resident.target_entity_index,
                    target_x: target.map_or(0, |p| p.x), target_y: target.map_or(0, |p| p.y),
                    target_z: target.map_or(0, |p| p.z), render_parameter: resident.render_parameter_index,
                    ..base
                });
            }
            for archer in reset.economy().authored_class5_archer() {
                let Some(source) =
                    reset.economy().authored_class5().iter().find(|s| {
                        s.source_entity_index == archer.source_entity_index && s.model == 4
                    })
                else {
                    continue;
                };
                // Archer action34 selects word96; word26 is its attacker.
                let target = targets.get(&source.word_0x96_150);
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: archer.slot_generation,
                    entity_index: source.source_entity_index,
                    source_id: archer.entity_id,
                    class_id: 5,
                    model: 4,
                    action: source.action,
                    phase: source.source_byte_3e,
                    active: u8::from(source.active),
                    x: source.position.x,
                    y: source.position.y,
                    z: source.position.z,
                    yaw: source.yaw,
                    pitch: source.pitch,
                    speed: source.actual_speed,
                    life: source.life,
                    control_flags: archer.control_flags,
                    // Source attack visual206 is the kneeling family. Keep
                    // this observation independent of camera draw admission.
                    stance: u8::from(source.particle_index == 206),
                    has_target: u8::from(target.is_some()),
                    target_index: source.word_0x96_150,
                    target_x: target.map_or(0, |p| p.x),
                    target_y: target.map_or(0, |p| p.y),
                    target_z: target.map_or(0, |p| p.z),
                    ..base
                });
            }
            // The live arena is authoritative when a source address has been
            // reused; retained projectile carriers must not claim the new row.
            for projectile in reset.economy().fireball_projectiles().iter().filter(|p|
                p.active && !p.drawing_disabled && p.source_class == 9 && matches!(p.source_model, 0 | 28) &&
                live_slots.get(&p.source_entity_index) == Some(&sim_core::native_world::Level2DynamicArenaKind::FireballProjectile))
            {
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: reset.economy().slot_generation(projectile.source_entity_index),
                    entity_index: projectile.source_entity_index,
                    source_id: projectile.owner_entity_index,
                    class_id: projectile.source_class,
                    model: projectile.source_model,
                    action: projectile.action,
                    phase: projectile.phase,
                    active: 1,
                    x: projectile.position.x,
                    y: projectile.position.y,
                    z: projectile.position.z,
                    yaw: projectile.yaw,
                    pitch: projectile.pitch,
                    speed: projectile.actual_speed,
                    life: i32::from(projectile.life),
                    control_flags: projectile.control_flags,
                    age_ticks: u16::try_from((i32::from(projectile.max_life) - i32::from(projectile.life)).max(0))
                        .unwrap_or(u16::MAX),
                    ..base
                });
            }
            for child in reset
                .economy()
                .authored_class5_archer_children()
                .iter()
                .filter(|s| s.active && s.slot_generation == reset.economy().slot_generation(s.source_entity_index) &&
                    live_slots.get(&s.source_entity_index) == Some(&sim_core::native_world::Level2DynamicArenaKind::AuthoredClass5ArcherChild))
            {
                let parent = reset.economy().authored_class5_archer().iter().find(|p| {
                    p.source_entity_index == child.parent_entity_index
                        && p.entity_id == child.parent_entity_id
                });
                let parent_generation = parent.map_or(0, |p| p.slot_generation);
                rows.push(McCampaignCharacterStateV1 {
                    slot_generation: child.slot_generation,
                    parent_generation,
                    has_parent: u8::from(parent.is_some()),
                    entity_index: child.source_entity_index,
                    source_id: child.entity_id,
                    parent_index: child.parent_entity_index,
                    parent_id: child.parent_entity_id,
                    class_id: child.source_class,
                    model: child.model,
                    action: child.action,
                    phase: child.source_byte_3e,
                    active: u8::from(child.drawing_enabled),
                    x: child.position.x,
                    y: child.position.y,
                    z: child.position.z,
                    yaw: child.yaw,
                    pitch: child.pitch,
                    speed: child.actual_speed,
                    life: i32::from(child.life),
                    control_flags: child.control_flags,
                    age_ticks: u16::try_from(
                        (i32::from(child.max_life) - i32::from(child.life)).max(0),
                    )
                    .unwrap_or(u16::MAX),
                    ..base
                });
            }
            rows.sort_by_key(|s| s.entity_index);
            if rows
                .windows(2)
                .any(|pair| pair[0].entity_index == pair[1].entity_index)
            {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "duplicate character source slot",
                ));
            }
            if rows.len() > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "character state buffer is too small",
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "character state buffer is null",
                ));
            }
            // SAFETY: caller supplies writable storage for capacity elements;
            // element size, non-null pointers and produced count were checked.
            unsafe {
                for (index, row) in rows.iter().enumerate() {
                    buffer.add(index).write(*row);
                }
                out_written.write(rows.len() as u32);
            }
            Ok(())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn character_layout_and_invalid_queries() {
        assert_eq!(size_of::<McCampaignVoxelDeathV1>(), 32);
        assert_eq!(std::mem::offset_of!(McCampaignVoxelDeathV1, sequence), 16);
        assert_eq!(size_of::<McCampaignCharacterStateV1>(), 80);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV1, x), 48);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV1, life), 60);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV1, render_parameter), 78);
        let mut written = 19;
        assert_ne!(
            mc_native_campaign_character_states_copy(
                0,
                0,
                0,
                std::ptr::null_mut(),
                0,
                79,
                &mut written
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            written, 19,
            "failure must not manufacture a successful batch"
        );
        assert_ne!(
            mc_native_campaign_character_states_copy(
                0,
                0,
                0,
                std::ptr::null_mut(),
                0,
                80,
                &mut written
            ),
            MC_STATUS_OK
        );
    }

    #[test]
    fn level_one_villagers_are_observed_from_live_native_state() {
        use super::super::game_session_abi::mc_game_session_step_json_v1;
        use super::super::game_runtime_abi::{mc_game_runtime_bootstrap_package_set_json_v1,
            mc_game_runtime_create_v1, mc_game_runtime_destroy_v1, mc_game_runtime_open_level_v1};
        let generated = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let request = serde_json::to_vec(&serde_json::json!({
            "request_contract": "carpet-flight-installed-package-set-bootstrap/v1",
            "packages": [
                {"package_dir":generated.join("mc-bhq8-level1-collision-v5-night"),"owner_cache_path":generated.join("mc-oi62-environment-level1-v2")},
                {"package_dir":generated.join("mc-cf02-campaign-package-v2-b/qualified-level2"),"owner_cache_path":null},
                {"package_dir":generated.join("mc-cf14-campaign-package-v1-a/qualified-level3"),"owner_cache_path":null}
            ]
        })).unwrap();
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&mut handle), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_bootstrap_package_set_json_v1(handle, 1, request.as_ptr(), request.len() as u32), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let read = |tick| {
            let mut rows = vec![McCampaignCharacterStateV1::default(); 1000];
            let mut written = 0;
            assert_eq!(mc_native_campaign_character_states_copy(handle, 1, tick, rows.as_mut_ptr(), 1000, 80, &mut written), MC_STATUS_OK);
            rows.truncate(written as usize);
            rows
        };
        let initial = read(0);
        let villagers: Vec<_> = initial.iter().filter(|r| (r.class_id, r.model) == (5, 13)).collect();
        assert_eq!(villagers.len(), 28);
        assert!(villagers.iter().all(|r| matches!(r.render_parameter, 239 | 241 | 242 | 271)));
        assert!(initial.iter().filter(|r| (r.class_id, r.model) != (5, 13)).all(|r| r.render_parameter == 0));
        for row in villagers.iter().filter(|r| r.render_parameter == 241) {
            println!("reset 241 slot={} source={} xyz={},{},{} speed={} action={}", row.entity_index, row.source_id, row.x, row.y, row.z, row.speed, row.action);
        }
        for tick in 0..20 {
            assert_eq!(mc_game_session_step_json_v1(handle, 1, tick, b"{}".as_ptr(), 2), MC_STATUS_OK);
        }
        let later = read(20);
        let moved: Vec<_> = villagers.iter().filter_map(|a| later.iter().find(|b| b.entity_index == a.entity_index && b.source_id == a.source_id && b.slot_generation == a.slot_generation && (a.x, a.y) != (b.x, b.y)).map(|b| (*a,b))).collect();
        assert!(!moved.is_empty());
        for (a,b) in moved.iter().filter(|(a,_)| a.render_parameter == 241) {
            println!("tick20 241 slot={} xyz={},{},{} delta={},{} speed={} action={}", b.entity_index,b.x,b.y,b.z, i32::from(b.x)-i32::from(a.x), i32::from(b.y)-i32::from(a.y),b.speed,b.action);
        }
        let mut written = 0;
        assert_eq!(mc_native_campaign_character_states_copy(handle, 1, 0, std::ptr::null_mut(), 0, 80, &mut written), MC_STATUS_GENERATION_MISMATCH);
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }
}
