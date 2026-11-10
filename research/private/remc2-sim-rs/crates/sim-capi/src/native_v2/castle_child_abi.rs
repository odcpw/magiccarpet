//! Read-only Castle balloon observations. The economy remains authoritative.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McCampaignCastleChildV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub slot_generation: u32,
    pub castle_generation: u32,
    pub target_generation: u32,
    pub entity_index: u16,
    pub source_id: u16,
    pub owner_entity_index: u16,
    pub player_entity_index: u16,
    pub castle_index: u16,
    pub target_index: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub speed: i16,
    pub render_parameter: u16,
    pub render_height: u16,
    pub reserved_0: u16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub control_flags: u32,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub active: u8,
    pub drawing: u8,
    pub owner_player_color: u8,
    pub relation_flags: u8,
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_castle_children_copy_v1(
    handle: u64,
    generation: u64,
    tick: u64,
    buffer: *mut McCampaignCastleChildV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignCastleChildV1>(element_size, "Castle child")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign {
                reset, inspection, ..
            } = &session.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Castle children require a descriptor campaign",
                ));
            };
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale Castle child query",
                ));
            }
            let economy = reset.economy();
            let live = economy
                .live_arena_slots()
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            let castle = economy.castle();
            let mut rows = BTreeMap::new();
            for child in economy.castle_children().iter().filter(|child| {
                child.active && child.source_class == 3 && child.model == 3 && child.action == 9
            }) {
                if live.get(&child.source_entity_index)
                    != Some(&sim_core::native_world::Level2DynamicArenaKind::CastleDefender)
                {
                    continue;
                }
                let mut owners = reset
                    .shared_spells()
                    .players()
                    .iter()
                    .filter(|player| player.entity_index == child.id_entity_index);
                let owner_color = owners.next().and_then(|player| {
                    owners.next().is_none().then(|| player.player_color_index())
                });
                let current_castle = castle.filter(|root| {
                    root.owner_entity_index == child.owner_entity_index
                        && live.get(&root.source_entity_index)
                            == Some(&sim_core::native_world::Level2DynamicArenaKind::CastleRoot)
                });
                let target = economy.spheres().iter().find(|sphere| {
                    sphere.active
                        && sphere.source_entity_index == child.target_sphere_slot
                        && live.get(&sphere.source_entity_index)
                            == Some(&sim_core::native_world::Level2DynamicArenaKind::ManaSphere)
                });
                let mut relation_flags = 0;
                if owner_color.is_some() {
                    relation_flags |= 1;
                }
                if current_castle.is_some() {
                    relation_flags |= 2;
                }
                if target.is_some() {
                    relation_flags |= 4;
                }
                let height = inspection
                    .presentation()
                    .particles
                    .get(usize::from(child.visual_index))
                    .map_or(0, |particle| particle.rotation_speed);
                let row = McCampaignCastleChildV1 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCampaignCastleChildV1>(),
                    generation,
                    tick,
                    slot_generation: economy.slot_generation(child.source_entity_index),
                    castle_generation: current_castle
                        .map_or(0, |root| economy.slot_generation(root.source_entity_index)),
                    target_generation: target.map_or(0, |sphere| {
                        economy.slot_generation(sphere.source_entity_index)
                    }),
                    entity_index: child.source_entity_index,
                    source_id: child.id_entity_index,
                    owner_entity_index: child.owner_entity_index,
                    player_entity_index: child.player_entity_index,
                    castle_index: current_castle.map_or(0, |root| root.source_entity_index),
                    target_index: child.target_sphere_slot,
                    x: child.position.x,
                    y: child.position.y,
                    z: child.position.z,
                    yaw: child.yaw as i16,
                    speed: child.speed,
                    render_parameter: child.visual_index,
                    render_height: height,
                    life: child.life,
                    max_life: child.max_life,
                    mana: child.mana,
                    max_mana: child.max_mana,
                    control_flags: child.control_flags,
                    class_id: child.source_class,
                    model: child.model,
                    action: child.action,
                    phase: child.source_byte_3e,
                    active: 1,
                    // remc2 DisableEntityDrawing04 flag; typed child remains
                    // in the arena until a later reclaim pass.
                    drawing: u8::from(child.control_flags & 0x0000_0400 == 0),
                    owner_player_color: owner_color.unwrap_or(0),
                    relation_flags,
                    ..Default::default()
                };
                if rows.insert(child.source_entity_index, row).is_some() {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "duplicate live Castle child incarnation",
                    ));
                }
            }
            if rows.len() > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Castle child buffer is too small",
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Castle child buffer is null",
                ));
            }
            // SAFETY: capacity, element size and non-null storage were checked.
            unsafe {
                for (index, row) in rows.values().enumerate() {
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
    fn castle_child_layout_is_exact() {
        assert_eq!(size_of::<McCampaignCastleChildV1>(), 96);
        assert_eq!(std::mem::offset_of!(McCampaignCastleChildV1, life), 64);
        assert_eq!(
            std::mem::offset_of!(McCampaignCastleChildV1, control_flags),
            80
        );
        assert_eq!(
            std::mem::offset_of!(McCampaignCastleChildV1, owner_player_color),
            90
        );
    }
}
