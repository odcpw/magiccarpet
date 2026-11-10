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
    /// Source appearance parameter for settlement humans (models 12–14).
    pub render_parameter: u16,
}

/// V1 remains an 80-byte observation for existing clients. V2 additionally
/// publishes real 3D pose and multipart geometry; no gameplay field is reused.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McCampaignCharacterStateV2 {
    pub base: McCampaignCharacterStateV1,
    pub roll: i16,
    /// Worm's source collision-array half extent; zero for other families.
    pub source_half_width: u16,
    /// Particle-table rotation_speed, used for the original projected height.
    pub render_height: u16,
    pub previous_index: u16,
    pub next_index: u16,
    pub part_index: u16,
    /// Bit0: positive-life model13 action108 consumed into a house.
    pub lifecycle_flags: u32,
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
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel deaths require a descriptor campaign",
                ));
            };
            let terrain = reset.canonical_terrain();
            let world = terrain.voxel_world().ok_or_else(|| {
                AbiError::new(MC_STATUS_INVALID_ARGUMENT, "voxel geometry is disabled")
            })?;
            // The command/session generation can advance on pause and resume
            // while the same world and its death journal remain alive. Guard
            // the query with current session identity; label rows with the
            // stable world epoch so that rebinds do not replay old deaths.
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "stale voxel death query: requested generation {generation} tick {tick}, session generation {} tick {}",
                        session.generation,
                        reset.campaign_tick()
                    ),
                ));
            }
            let journal = terrain
                .voxel_deaths()
                .expect("voxel world has a death journal");
            let rows = journal.rows();
            if after_sequence != 0
                && rows
                    .front()
                    .is_some_and(|first| after_sequence < first.sequence - 1)
            {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "voxel death cursor fell behind retained history",
                ));
            }
            if after_sequence > rows.back().map_or(0, |last| last.sequence) {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel death cursor is ahead of history",
                ));
            }
            let count = rows
                .iter()
                .filter(|row| row.sequence > after_sequence)
                .count();
            if count > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "voxel death buffer is too small",
                ));
            }
            if count != 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "voxel death buffer is null",
                ));
            }
            // SAFETY: the caller supplies writable space for capacity elements;
            // element size, produced count and non-null output were checked.
            unsafe {
                for (index, row) in rows
                    .iter()
                    .filter(|row| row.sequence > after_sequence)
                    .enumerate()
                {
                    buffer.add(index).write(McCampaignVoxelDeathV1 {
                        abi_version: MC_NATIVE_ABI_VERSION,
                        struct_size: struct_size::<McCampaignVoxelDeathV1>(),
                        generation: world.generation(),
                        sequence: row.sequence,
                        slot_generation: row.slot_generation,
                        entity_index: row.slot,
                        source_id: row.source_id,
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
            let rows = character_states_v1(session, generation, tick)?;
            write_character_rows(&rows, buffer, capacity, out_written)
        })
    })
}

fn character_states_v1(
    session: &CampaignSessionRecord,
    generation: u64,
    tick: u64,
) -> Result<Vec<McCampaignCharacterStateV1>, AbiError> {
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
    let live_slots = reset
        .economy()
        .live_arena_slots()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
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
    for villager in reset
        .ordinary()
        .villagers()
        .iter()
        .filter(|s| s.active && !live_slots.contains_key(&s.entity_index))
    {
        let target = villager
            .target_entity_index
            .and_then(|index| targets.get(&index));
        rows.push(McCampaignCharacterStateV1 {
            slot_generation: reset.economy().slot_generation(villager.entity_index),
            entity_index: villager.entity_index,
            source_id: villager.source_id,
            class_id: 5,
            model: 13,
            action: villager.action,
            phase: villager.phase,
            active: 1,
            x: villager.position.x,
            y: villager.position.y,
            z: villager.position.z,
            yaw: villager.yaw,
            pitch: villager.pitch,
            speed: villager.actual_speed,
            life: villager.life,
            control_flags: villager.control_flags,
            has_target: u8::from(target.is_some()),
            target_index: villager.target_entity_index.unwrap_or(0),
            target_x: target.map_or(0, |p| p.x),
            target_y: target.map_or(0, |p| p.y),
            target_z: target.map_or(0, |p| p.z),
            render_parameter: villager.render_parameter_index,
            ..base
        });
    }
    for resident in reset.economy().house_residents().iter().filter(|s| {
        s.active
            && matches!(s.model, 12..=14)
            && live_slots.get(&s.source_entity_index)
                == Some(&sim_core::native_world::Level2DynamicArenaKind::HouseResident)
    }) {
        let target = targets.get(&resident.target_entity_index);
        rows.push(McCampaignCharacterStateV1 {
            slot_generation: reset
                .economy()
                .slot_generation(resident.source_entity_index),
            entity_index: resident.source_entity_index,
            source_id: resident.source_id,
            class_id: 5,
            model: resident.model,
            action: resident.action,
            phase: resident.phase,
            active: u8::from(resident.drawing_enabled),
            x: resident.position.x,
            y: resident.position.y,
            z: resident.position.z,
            yaw: resident.yaw,
            speed: resident.actual_speed,
            life: resident.life,
            control_flags: resident.control_flags,
            has_target: u8::from(target.is_some()),
            target_index: resident.target_entity_index,
            target_x: target.map_or(0, |p| p.x),
            target_y: target.map_or(0, |p| p.y),
            target_z: target.map_or(0, |p| p.z),
            render_parameter: resident.render_parameter_index,
            ..base
        });
    }
    for archer in reset.economy().authored_class5_archer() {
        let Some(source) = reset
            .economy()
            .authored_class5()
            .iter()
            .find(|s| s.source_entity_index == archer.source_entity_index && s.model == 4)
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
    for projectile in reset.economy().fireball_projectiles().iter().filter(|p| {
        p.active
            && !p.drawing_disabled
            && p.source_class == 9
            && matches!(p.source_model, 0 | 28)
            && live_slots.get(&p.source_entity_index)
                == Some(&sim_core::native_world::Level2DynamicArenaKind::FireballProjectile)
    }) {
        rows.push(McCampaignCharacterStateV1 {
            slot_generation: reset
                .economy()
                .slot_generation(projectile.source_entity_index),
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
            age_ticks: u16::try_from(
                (i32::from(projectile.max_life) - i32::from(projectile.life)).max(0),
            )
            .unwrap_or(u16::MAX),
            ..base
        });
    }
    for child in reset
        .economy()
        .authored_class5_archer_children()
        .iter()
        .filter(|s| {
            s.active
                && s.slot_generation == reset.economy().slot_generation(s.source_entity_index)
                && live_slots.get(&s.source_entity_index)
                    == Some(
                        &sim_core::native_world::Level2DynamicArenaKind::AuthoredClass5ArcherChild,
                    )
        })
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
            age_ticks: u16::try_from((i32::from(child.max_life) - i32::from(child.life)).max(0))
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
    Ok(rows)
}

fn write_character_rows<T: Copy>(
    rows: &[T],
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> Result<(), AbiError> {
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
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_character_states_copy_v2(
    handle: u64,
    generation: u64,
    tick: u64,
    buffer: *mut McCampaignCharacterStateV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignCharacterStateV2>(element_size, "character state v2")?;
        with_campaign_session(handle, |session| {
            // One lock and one authoritative tick cover both the existing
            // families and the multipart observations.
            let previous = character_states_v1(session, generation, tick)?;
            let CampaignSessionKind::Campaign {
                reset, inspection, ..
            } = &session.kind
            else {
                unreachable!()
            };
            let logical_height = |parameter: u16| -> Result<u16, AbiError> {
                inspection
                    .presentation()
                    .particles
                    .get(usize::from(parameter))
                    .map(|row| row.rotation_speed)
                    .filter(|height| *height > 0)
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "creature lacks source visual height",
                        )
                    })
            };
            let entries: BTreeSet<_> = reset
                .ordinary()
                .villagers()
                .iter()
                .filter(|row| {
                    row.active && row.life >= 0 && row.action == 108 && row.reset_counter != 0
                })
                .map(|row| row.entity_index)
                .chain(
                    reset
                        .economy()
                        .house_residents()
                        .iter()
                        .filter(|row| {
                            row.active
                                && row.model == 13
                                && row.life >= 0
                                && row.action == 108
                                && row.reset_counter != 0
                        })
                        .map(|row| row.source_entity_index),
                )
                .collect();
            let mut rows: Vec<_> = previous
                .into_iter()
                .map(|mut base| {
                    base.struct_size = struct_size::<McCampaignCharacterStateV2>();
                    McCampaignCharacterStateV2 {
                        lifecycle_flags: u32::from(
                            base.class_id == 5
                                && base.model == 13
                                && entries.contains(&base.entity_index),
                        ),
                        base,
                        ..Default::default()
                    }
                })
                .collect();
            let economy = reset.economy();
            let slots = economy
                .live_arena_slots()
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            use sim_core::native_world::Level2DynamicArenaKind as Kind;
            let base = McCampaignCharacterStateV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignCharacterStateV2>(),
                generation,
                tick,
                class_id: 5,
                ..Default::default()
            };
            for source in economy.authored_class5().iter().filter(|s| {
                s.active
                    && matches!(s.model, 3 | 19)
                    && slots.get(&s.source_entity_index) == Some(&Kind::AuthoredClass5)
            }) {
                let (source_id, control_flags, width, height, previous_index, next_index) =
                    if source.model == 3 {
                        let state = economy
                            .authored_class5_model3()
                            .iter()
                            .find(|s| s.source_entity_index == source.source_entity_index)
                            .ok_or_else(|| {
                                AbiError::new(
                                    MC_STATUS_VALIDATION_FAILED,
                                    "current Worm lacks its typed state",
                                )
                            })?;
                        (
                            state.entity_id,
                            state.control_flags,
                            state.render_pitch,
                            logical_height(source.particle_index)?,
                            state.word_0x32,
                            state.word_0x34,
                        )
                    } else {
                        let state = economy
                            .authored_fireflies()
                            .iter()
                            .find(|s| s.source_entity_index == source.source_entity_index)
                            .ok_or_else(|| {
                                AbiError::new(
                                    MC_STATUS_VALIDATION_FAILED,
                                    "current Firefly lacks its typed state",
                                )
                            })?;
                        let height = logical_height(source.particle_index)?;
                        (state.entity_id, state.control_flags, 0, height, 0, 0)
                    };
                if source.model == 3 && width <= 0 || height == 0 {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "creature render dimensions must be positive",
                    ));
                }
                rows.push(McCampaignCharacterStateV2 {
                    base: McCampaignCharacterStateV1 {
                        slot_generation: economy.slot_generation(source.source_entity_index),
                        entity_index: source.source_entity_index,
                        source_id,
                        model: source.model,
                        action: source.action,
                        phase: source.phase,
                        active: u8::from(source.drawing_enabled),
                        x: source.position.x,
                        y: source.position.y,
                        z: source.position.z,
                        yaw: source.yaw,
                        pitch: source.pitch,
                        speed: source.actual_speed,
                        life: source.life,
                        control_flags,
                        render_parameter: source.particle_index,
                        ..base
                    },
                    roll: source.roll,
                    source_half_width: width as u16,
                    render_height: height,
                    previous_index,
                    next_index,
                    ..Default::default()
                });
            }
            for child in economy
                .authored_class5_model3_children()
                .iter()
                .filter(|s| {
                    s.active
                        && slots.get(&s.source_entity_index)
                            == Some(&Kind::AuthoredClass5Model3Child)
                })
            {
                // The head can disappear before its children complete cleanup.
                // Never attach those retained children to a reused head slot.
                let parent = economy.authored_class5().iter().find(|p| {
                    p.active
                        && p.model == 3
                        && p.source_entity_index == child.parent_entity_index
                        && p.authored_index == child.root_authored_index
                        && slots.get(&p.source_entity_index) == Some(&Kind::AuthoredClass5)
                });
                let parent_id = parent
                    .and_then(|p| {
                        economy
                            .authored_class5_model3()
                            .iter()
                            .find(|s| s.source_entity_index == p.source_entity_index)
                    })
                    .filter(|head| {
                        // Prove membership in the current head's actual chain,
                        // not merely an equal recycled numeric parent address.
                        let mut previous = head.source_entity_index;
                        let mut next = head.word_0x34;
                        for index in 0..16 {
                            let Some(part) =
                                economy
                                    .authored_class5_model3_children()
                                    .iter()
                                    .find(|part| {
                                        part.active
                                            && part.source_entity_index == next
                                            && part.word_0x32 == previous
                                            && part.parent_entity_index == head.source_entity_index
                                            && part.source_byte_3e == index
                                            && part.root_authored_index == child.root_authored_index
                                            && slots.get(&part.source_entity_index)
                                                == Some(&Kind::AuthoredClass5Model3Child)
                                    })
                            else {
                                return false;
                            };
                            if part.source_entity_index == child.source_entity_index {
                                return true;
                            }
                            previous = next;
                            next = part.word_0x34;
                        }
                        false
                    })
                    .map(|s| s.entity_id);
                if child.render_pitch <= 0 || child.render_fov <= 0 || child.source_byte_3e >= 16 {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "invalid native Worm part dimensions/index",
                    ));
                }
                rows.push(McCampaignCharacterStateV2 {
                    base: McCampaignCharacterStateV1 {
                        slot_generation: economy.slot_generation(child.source_entity_index),
                        // qmemcpy retains the root's birth ID, independently
                        // of authored-row provenance and later parent reuse.
                        entity_index: child.source_entity_index,
                        source_id: child.parent_entity_index,
                        parent_index: child.parent_entity_index,
                        parent_id: parent_id.unwrap_or(0),
                        parent_generation: if parent_id.is_some() {
                            economy.slot_generation(child.parent_entity_index)
                        } else {
                            0
                        },
                        has_parent: u8::from(parent_id.is_some()),
                        model: 3,
                        action: child.action,
                        phase: child.phase,
                        active: u8::from(child.drawing_enabled),
                        x: child.position.x,
                        y: child.position.y,
                        z: child.position.z,
                        yaw: child.yaw,
                        pitch: child.pitch,
                        speed: child.actual_speed,
                        life: child.life,
                        control_flags: child.control_flags,
                        render_parameter: child.particle_index,
                        ..base
                    },
                    roll: child.roll,
                    source_half_width: child.render_pitch as u16,
                    render_height: logical_height(child.particle_index)?,
                    part_index: u16::from(child.source_byte_3e),
                    previous_index: child.word_0x32,
                    next_index: child.word_0x34,
                    ..Default::default()
                });
            }
            // Imported package creatures are authoritative raw carriers. They
            // are distinct from later NewEvent typed families; never publish a
            // stale raw packet underneath a currently allocated typed slot.
            let raw = reset.entities();
            for source in raw.iter().filter(|s| {
                s.active
                    && s.class == 5
                    && matches!(s.model, 3 | 19)
                    && !slots.contains_key(&s.index)
            }) {
                let bytes = source.record_bytes();
                if bytes.len() < 158 {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "truncated imported creature packet",
                    ));
                }
                let word = |offset| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                let parameter = source.particle_parameter_index.ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "imported creature lacks its appearance",
                    )
                })?;
                let is_part = source.model == 3 && (89..=104).contains(&parameter);
                if source.model == 3 && parameter != 88 && !is_part
                    || source.model == 19 && parameter != 287
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "invalid imported creature appearance",
                    ));
                }
                let width = if source.model == 19 { 0 } else { word(88) };
                let height = logical_height(parameter)?;
                if source.model == 3 && width == 0 || width > i16::MAX as u16 {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "invalid imported creature dimensions",
                    ));
                }
                let parent = raw
                    .iter()
                    .filter(|head| {
                        is_part
                            && head.active
                            && head.class == 5
                            && head.model == 3
                            && head.particle_parameter_index == Some(88)
                            && !slots.contains_key(&head.index)
                    })
                    .find(|head| {
                        let mut previous = head.index;
                        let mut next = head.word_34();
                        for index in 0..16 {
                            let Some(part) = raw.get(usize::from(next)).filter(|part| {
                                part.active
                                    && part.class == 5
                                    && part.model == 3
                                    && part.particle_parameter_index == Some(89 + index)
                                    && part.word_32() == previous
                                    && !slots.contains_key(&part.index)
                            }) else {
                                return false;
                            };
                            if part.index == source.index {
                                return true;
                            }
                            previous = next;
                            next = part.word_34();
                        }
                        false
                    });
                rows.push(McCampaignCharacterStateV2 {
                    base: McCampaignCharacterStateV1 {
                        slot_generation: economy.slot_generation(source.index),
                        entity_index: source.index,
                        source_id: source.source_id,
                        parent_index: parent.map_or(0, |p| p.index),
                        parent_id: parent.map_or(0, |p| p.source_id),
                        parent_generation: parent.map_or(0, |p| economy.slot_generation(p.index)),
                        has_parent: u8::from(parent.is_some()),
                        model: source.model,
                        action: source.action,
                        phase: source.phase,
                        active: u8::from(!source.drawing_disabled()),
                        x: source.position.x,
                        y: source.position.y,
                        z: source.position.z,
                        yaw: source.yaw,
                        pitch: source.pitch,
                        speed: source.actual_speed,
                        life: source.life,
                        control_flags: source.control_flags,
                        render_parameter: parameter,
                        ..base
                    },
                    roll: word(36) as i16,
                    source_half_width: width,
                    render_height: height,
                    previous_index: if source.model == 3 {
                        source.word_32()
                    } else {
                        0
                    },
                    next_index: if source.model == 3 {
                        source.word_34()
                    } else {
                        0
                    },
                    part_index: if is_part { parameter - 89 } else { 0 },
                    ..Default::default()
                });
            }
            rows.sort_by_key(|s| s.base.entity_index);
            if rows
                .windows(2)
                .any(|p| p[0].base.entity_index == p[1].base.entity_index)
            {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "duplicate character source slot in v2",
                ));
            }
            write_character_rows(&rows, buffer, capacity, out_written)
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
        assert_eq!(
            std::mem::offset_of!(McCampaignCharacterStateV1, render_parameter),
            78
        );
        assert_eq!(size_of::<McCampaignCharacterStateV2>(), 96);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV2, roll), 80);
        assert_eq!(
            std::mem::offset_of!(McCampaignCharacterStateV2, part_index),
            90
        );
        assert_eq!(
            std::mem::offset_of!(McCampaignCharacterStateV2, lifecycle_flags),
            92
        );
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
            mc_native_campaign_character_states_copy_v2(
                0,
                0,
                0,
                std::ptr::null_mut(),
                0,
                95,
                &mut written
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 19);
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
    fn current_creatures_publish_native_pose_and_worm_chain() {
        use super::super::game_session_abi::{mc_game_session_destroy_v1, mc_game_session_open_v1};
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        // Genuine opening constructors in these other ordinary maps exercise
        // the shared family observation. This is not Level1 activation proof.
        for (package, model, expected) in [
            ("mc-cf14-campaign-package-v1-a/qualified-level3", 3, 85),
            ("mc-bhq8-level4-collision-v2-cave", 19, 34),
        ] {
            let path = generated.join(package).to_string_lossy().into_owned();
            let mut handle = 0;
            assert_eq!(
                mc_game_session_open_v1(
                    path.as_ptr(),
                    path.len() as u32,
                    b"".as_ptr(),
                    0,
                    b"".as_ptr(),
                    0,
                    &mut handle
                ),
                MC_STATUS_OK
            );
            let mut rows = vec![McCampaignCharacterStateV2::default(); 1000];
            let mut written = 0;
            assert_eq!(
                mc_native_campaign_character_states_copy_v2(
                    handle,
                    1,
                    0,
                    rows.as_mut_ptr(),
                    1000,
                    96,
                    &mut written
                ),
                MC_STATUS_OK
            );
            rows.truncate(written as usize);
            let family: Vec<_> = rows
                .iter()
                .filter(|r| (r.base.class_id, r.base.model) == (5, model))
                .collect();
            assert_eq!(family.len(), expected);
            assert!(
                family
                    .iter()
                    .all(|r| (model == 19 || r.source_half_width > 0)
                        && r.render_height > 0
                        && r.lifecycle_flags == 0)
            );
            if model == 19 {
                assert!(
                    family
                        .iter()
                        .all(|r| r.base.render_parameter == 287 && r.render_height == 200)
                );
            } else {
                let heads: Vec<_> = family
                    .iter()
                    .filter(|r| r.base.render_parameter == 88)
                    .collect();
                assert_eq!(heads.len(), 5);
                for head in heads {
                    assert_eq!(head.render_height, 180);
                    assert_eq!(head.source_half_width, 414);
                    let parts: Vec<_> = family
                        .iter()
                        .filter(|r| r.base.parent_index == head.base.entity_index)
                        .collect();
                    assert_eq!(parts.len(), 16);
                    for part in parts {
                        assert_eq!(part.base.has_parent, 1);
                        assert_eq!(part.base.parent_generation, head.base.slot_generation);
                        assert_eq!(part.base.parent_id, head.base.source_id);
                        assert_eq!(part.part_index, part.base.render_parameter - 89);
                    }
                }
            }
            with_campaign_session(handle, |session| {
                let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                    unreachable!()
                };
                for row in &family {
                    let source = &reset.entities()[usize::from(row.base.entity_index)];
                    let bytes = source.record_bytes();
                    let CampaignSessionKind::Campaign { inspection, .. } = &session.kind else {
                        unreachable!()
                    };
                    assert_eq!(
                        row.render_height,
                        inspection.presentation().particles[usize::from(row.base.render_parameter)]
                            .rotation_speed
                    );
                    assert_eq!(
                        (row.base.x, row.base.y, row.base.z),
                        (source.position.x, source.position.y, source.position.z)
                    );
                    assert_eq!(
                        (row.base.yaw, row.base.pitch, row.roll),
                        (
                            source.yaw,
                            source.pitch,
                            i16::from_le_bytes([bytes[36], bytes[37]])
                        )
                    );
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(mc_game_session_destroy_v1(handle), MC_STATUS_OK);
        }
    }

    #[test]
    fn level_one_villagers_are_observed_from_live_native_state() {
        use super::super::game_runtime_abi::{
            mc_game_runtime_bootstrap_package_set_json_v1, mc_game_runtime_create_v1,
            mc_game_runtime_destroy_v1, mc_game_runtime_open_level_v1,
        };
        use super::super::game_session_abi::mc_game_session_step_json_v1;
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
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
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                handle,
                1,
                request.as_ptr(),
                request.len() as u32
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        let read = |tick| {
            let mut rows = vec![McCampaignCharacterStateV1::default(); 1000];
            let mut written = 0;
            assert_eq!(
                mc_native_campaign_character_states_copy(
                    handle,
                    1,
                    tick,
                    rows.as_mut_ptr(),
                    1000,
                    80,
                    &mut written
                ),
                MC_STATUS_OK
            );
            rows.truncate(written as usize);
            rows
        };
        let initial = read(0);
        let mut creatures = vec![McCampaignCharacterStateV2::default(); 1000];
        let mut count = 0;
        assert_eq!(
            mc_native_campaign_character_states_copy_v2(
                handle,
                1,
                0,
                creatures.as_mut_ptr(),
                1000,
                96,
                &mut count
            ),
            MC_STATUS_OK
        );
        creatures.truncate(count as usize);
        assert!(
            creatures
                .iter()
                .all(|r| r.base.struct_size == 96 && r.lifecycle_flags == 0)
        );
        // These authored families are deferred until native group6/33 and
        // group4/34 activation. The opening query must not invent them from
        // the map census. Positive producer coverage uses their actual birth.
        assert!(
            creatures
                .iter()
                .all(|r| r.base.class_id != 5 || !matches!(r.base.model, 3 | 19))
        );
        assert_eq!(creatures.len(), initial.len());
        // A capacity failure must not publish a partial batch or count.
        let mut sentinel = McCampaignCharacterStateV2::default();
        sentinel.lifecycle_flags = 0x11223344;
        count = 777;
        assert_eq!(
            mc_native_campaign_character_states_copy_v2(
                handle,
                1,
                0,
                &mut sentinel,
                1,
                96,
                &mut count
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(count, 777);
        assert_eq!(sentinel.lifecycle_flags, 0x11223344);
        let villagers: Vec<_> = initial
            .iter()
            .filter(|r| (r.class_id, r.model) == (5, 13))
            .collect();
        assert_eq!(villagers.len(), 28);
        assert!(
            villagers
                .iter()
                .all(|r| matches!(r.render_parameter, 239 | 241 | 242 | 271))
        );
        assert!(
            initial
                .iter()
                .filter(|r| (r.class_id, r.model) != (5, 13))
                .all(|r| r.render_parameter == 0)
        );
        for row in villagers.iter().filter(|r| r.render_parameter == 241) {
            println!(
                "reset 241 slot={} source={} xyz={},{},{} speed={} action={}",
                row.entity_index, row.source_id, row.x, row.y, row.z, row.speed, row.action
            );
        }
        for tick in 0..20 {
            assert_eq!(
                mc_game_session_step_json_v1(handle, 1, tick, b"{}".as_ptr(), 2),
                MC_STATUS_OK
            );
        }
        let later = read(20);
        let moved: Vec<_> = villagers
            .iter()
            .filter_map(|a| {
                later
                    .iter()
                    .find(|b| {
                        b.entity_index == a.entity_index
                            && b.source_id == a.source_id
                            && b.slot_generation == a.slot_generation
                            && (a.x, a.y) != (b.x, b.y)
                    })
                    .map(|b| (*a, b))
            })
            .collect();
        assert!(!moved.is_empty());
        for (a, b) in moved.iter().filter(|(a, _)| a.render_parameter == 241) {
            println!(
                "tick20 241 slot={} xyz={},{},{} delta={},{} speed={} action={}",
                b.entity_index,
                b.x,
                b.y,
                b.z,
                i32::from(b.x) - i32::from(a.x),
                i32::from(b.y) - i32::from(a.y),
                b.speed,
                b.action
            );
        }
        let mut written = 0;
        assert_eq!(
            mc_native_campaign_character_states_copy(
                handle,
                1,
                0,
                std::ptr::null_mut(),
                0,
                80,
                &mut written
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }
}
