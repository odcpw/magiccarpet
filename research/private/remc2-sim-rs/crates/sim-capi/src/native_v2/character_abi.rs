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
    pub reserved_1: u16,
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
            for child in reset
                .economy()
                .authored_class5_archer_children()
                .iter()
                .filter(|s| s.active)
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
        assert_eq!(size_of::<McCampaignCharacterStateV1>(), 80);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV1, x), 48);
        assert_eq!(std::mem::offset_of!(McCampaignCharacterStateV1, life), 60);
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
}
