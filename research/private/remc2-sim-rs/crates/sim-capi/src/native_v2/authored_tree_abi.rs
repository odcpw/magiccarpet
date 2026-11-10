//! Read-only observations of current source tree occupants.
use super::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct McCampaignAuthoredTreeV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub slot_generation: u32,
    pub entity_index: u16,
    pub source_id: u16,
    /// u16::MAX means this occupant was imported at reset.
    pub authored_index: u16,
    pub render_parameter: u16,
    pub render_height: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub life: i32,
    pub control_flags: u32,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub drawing: u8,
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_authored_trees_copy_v1(
    handle: u64,
    generation: u64,
    tick: u64,
    buffer: *mut McCampaignAuthoredTreeV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignAuthoredTreeV1>(element_size, "authored tree")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign {
                reset, inspection, ..
            } = &session.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "trees require a descriptor campaign",
                ));
            };
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale authored tree query",
                ));
            }
            let economy = reset.economy();
            let live = economy
                .live_arena_slots()
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            let height = |particle: u16| {
                inspection
                    .presentation()
                    .particles
                    .get(usize::from(particle))
                    .map_or(0, |p| p.rotation_speed as u16)
            };
            let mut rows = BTreeMap::new();
            // Source-arena rows imported at reset remain authoritative until a
            // dynamic occupant claims their address; never return the shadow.
            for tree in reset.entities().iter().filter(|tree| {
                tree.active && tree.class == 2 && tree.model == 0 && !live.contains_key(&tree.index)
            }) {
                let parameter = tree.particle_parameter_index.unwrap_or(u16::MAX);
                rows.insert(
                    tree.index,
                    McCampaignAuthoredTreeV1 {
                        abi_version: MC_NATIVE_ABI_VERSION,
                        struct_size: struct_size::<McCampaignAuthoredTreeV1>(),
                        generation,
                        tick,
                        slot_generation: economy.slot_generation(tree.index),
                        entity_index: tree.index,
                        source_id: tree.source_id,
                        authored_index: u16::MAX,
                        render_parameter: parameter,
                        render_height: height(parameter),
                        x: tree.position.x,
                        y: tree.position.y,
                        z: tree.position.z,
                        life: tree.life,
                        control_flags: tree.control_flags,
                        class_id: tree.class,
                        model: tree.model,
                        action: tree.action,
                        drawing: u8::from(
                            !tree.drawing_disabled() && tree.control_flags & 0x21 == 0,
                        ),
                    },
                );
            }
            for tree in economy
                .authored_trees()
                .iter()
                .filter(|tree| tree.active && tree.source_class == 2 && tree.model == 0)
            {
                if live.get(&tree.source_entity_index)
                    != Some(&sim_core::native_world::Level2DynamicArenaKind::AuthoredTree)
                {
                    continue;
                }
                let row = McCampaignAuthoredTreeV1 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCampaignAuthoredTreeV1>(),
                    generation,
                    tick,
                    slot_generation: economy.slot_generation(tree.source_entity_index),
                    entity_index: tree.source_entity_index,
                    source_id: tree.entity_id,
                    authored_index: tree.authored_index,
                    render_parameter: tree.particle_index,
                    render_height: height(tree.particle_index),
                    x: tree.position.x,
                    y: tree.position.y,
                    z: tree.position.z,
                    life: tree.life,
                    control_flags: tree.control_flags,
                    class_id: tree.source_class,
                    model: tree.model,
                    action: tree.action,
                    drawing: u8::from(tree.drawing_enabled && tree.control_flags & 0x421 == 0),
                };
                if rows.insert(tree.source_entity_index, row).is_some() {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "duplicate current tree slot",
                    ));
                }
            }
            if rows.len() > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "authored tree buffer is too small",
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "authored tree buffer is null",
                ));
            }
            // SAFETY: element size, capacity and non-null output were checked.
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
    fn level1_opening_tree_carriers() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/settlement-20261105-integration/owner-closed-v6/level1");
        let package = package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 1,
            internal_level: 0,
            map_type: 1,
            ..Default::default()
        };
        let mut handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut handle
            ),
            MC_STATUS_OK
        );
        // Group 1 is fired by the ordinary reset switch only after the player
        // leaves its large overlap box. Reproduce the real source schedule:
        // eight idle turns followed by forward flight, never direct allocation.
        let mut activated = false;
        for turn in 0..264 {
            let position = with_campaign_session(handle, |session| {
                let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                    unreachable!()
                };
                Ok(reset.state.position)
            })
            .unwrap();
            let input = McCampaignStepInputV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignStepInputV2>(),
                generation: 0,
                command: McCommandV2 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCommandV2>(),
                    requested_spell: MC_REQUESTED_SPELL_NONE,
                    flags: i8::from(turn >= 8),
                    ..Default::default()
                },
                player_x: position.x,
                player_y: position.y,
                player_z: position.z,
                ..Default::default()
            };
            let mut snapshot = McCampaignStepSnapshotV2::default();
            let mut events = vec![McCampaignEventV2::default(); 1024];
            let mut event_count = 0;
            assert_eq!(
                mc_native_campaign_session_step(
                    handle,
                    &raw const input,
                    &raw mut snapshot,
                    struct_size::<McCampaignStepSnapshotV2>(),
                    events.as_mut_ptr(),
                    events.len() as u32,
                    struct_size::<McCampaignEventV2>(),
                    &raw mut event_count
                ),
                MC_STATUS_OK,
                "turn {turn}"
            );
            activated = with_campaign_session(handle, |session| {
                let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                    unreachable!()
                };
                Ok(reset
                    .economy()
                    .authored_trees()
                    .iter()
                    .filter(|tree| tree.active)
                    .count()
                    == 18)
            })
            .unwrap();
            if activated {
                break;
            }
        }
        assert!(
            activated,
            "ordinary source switch and forward flight produce all 18 trees"
        );
        let tick = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!()
            };
            Ok(reset.campaign_tick())
        })
        .unwrap();
        assert_eq!(size_of::<McCampaignAuthoredTreeV1>(), 56);
        let mut rows = [McCampaignAuthoredTreeV1::default(); 32];
        let mut written = u32::MAX;
        assert_eq!(
            mc_native_campaign_authored_trees_copy_v1(
                handle,
                0,
                tick,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignAuthoredTreeV1>(),
                &raw mut written
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 18);
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                panic!("not descriptor campaign")
            };
            let live = reset
                .economy()
                .live_arena_slots()
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            let imported = reset
                .entities()
                .iter()
                .filter(|e| {
                    e.active && e.class == 2 && e.model == 0 && !live.contains_key(&e.index)
                })
                .collect::<Vec<_>>();
            let typed = reset
                .economy()
                .authored_trees()
                .iter()
                .filter(|e| {
                    e.active
                        && live.get(&e.source_entity_index)
                            == Some(&sim_core::native_world::Level2DynamicArenaKind::AuthoredTree)
                })
                .collect::<Vec<_>>();
            eprintln!(
                "level1 opening trees: imported={} typed={} imported_slots={:?} typed_slots={:?}",
                imported.len(),
                typed.len(),
                imported
                    .iter()
                    .map(|e| (e.index, e.action, e.particle_parameter_index))
                    .collect::<Vec<_>>(),
                typed
                    .iter()
                    .map(|e| (e.source_entity_index, e.action, e.particle_index))
                    .collect::<Vec<_>>()
            );
            assert_eq!(imported.len() + typed.len(), 18);
            let presentations = reset.campaign_entity_presentations(0).unwrap();
            for observed in &rows[..written as usize] {
                let source = typed
                    .iter()
                    .find(|tree| tree.source_entity_index == observed.entity_index)
                    .unwrap();
                assert_eq!(
                    (
                        observed.source_id,
                        observed.authored_index,
                        observed.slot_generation,
                        observed.action,
                        observed.render_parameter,
                        observed.life,
                        observed.drawing
                    ),
                    (
                        source.entity_id,
                        source.authored_index,
                        reset.economy().slot_generation(source.source_entity_index),
                        source.action,
                        source.particle_index,
                        source.life,
                        1
                    )
                );
                assert_eq!(
                    (observed.x, observed.y, observed.z),
                    (source.position.x, source.position.y, source.position.z)
                );
                assert!(presentations.iter().any(|sprite| sprite.entity_index
                    == observed.entity_index
                    && sprite.source_id == observed.source_id
                    && sprite.render_parameter_index == observed.render_parameter
                    && sprite.logical_height == observed.render_height));
            }
            Ok(())
        })
        .unwrap();
        written = u32::MAX;
        assert_eq!(
            mc_native_campaign_authored_trees_copy_v1(
                handle,
                0,
                tick,
                rows.as_mut_ptr(),
                17,
                struct_size::<McCampaignAuthoredTreeV1>(),
                &raw mut written
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(written, u32::MAX);
        assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
        assert_eq!(
            mc_native_campaign_authored_trees_copy_v1(
                handle,
                0,
                tick,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignAuthoredTreeV1>(),
                &raw mut written
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_native_campaign_authored_trees_copy_v1(
                handle,
                1,
                0,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignAuthoredTreeV1>(),
                &raw mut written
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 0, "reset returns to the pre-group-1 population");
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    }
}
