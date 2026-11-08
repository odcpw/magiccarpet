//! Read-only voxel projections and explicitly guarded mode configuration.
//! This is a child of game_runtime_abi so it uses the same runtime lock/owner.
use super::*;
use serde::{Deserialize, Serialize};
use sim_core::voxel_world::{VolumeId, VoxelWorld};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McVoxelStateV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub revision: u64,
    pub content_hash: u64,
    pub enabled: u32,
    pub removed_count: u32,
    pub impact_head: u64,
    /// Stable geometry/journal epoch; outer command generations also advance
    /// on pause/resume while retaining this same world and its actor lifetimes.
    pub world_generation: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct McVoxelImpactChildV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub sequence: u64,
    pub slot_generation: u32,
    pub entity_index: u16,
    pub reserved: u16,
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_voxel_impact_children_copy(
    handle: u64,
    generation: u64,
    tick: u64,
    buffer: *mut McVoxelImpactChildV1,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McVoxelImpactChildV1>(element_size, "voxel impact child")?;
        with_runtime(handle, |record| {
            let session = record.runtime.session().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel observation requires gameplay",
                )
            })?;
            let source = session.backend().campaign_presentation_source();
            if session.identity().generation != generation || source.tick() != tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale voxel impact child observation",
                ));
            }
            let sim = source.simulation();
            let terrain = sim.canonical_terrain();
            if terrain.voxel_world().is_none() {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel geometry is disabled",
                ));
            }
            let rows = terrain
                .voxel_impacts()
                .iter()
                .filter_map(|impact| {
                    let (slot, slot_generation) = impact.impact_child?;
                    sim.is_current_voxel_impact_child(slot, slot_generation)
                        .then_some(McVoxelImpactChildV1 {
                            abi_version: MC_NATIVE_ABI_VERSION,
                            struct_size: struct_size::<McVoxelImpactChildV1>(),
                            generation,
                            sequence: impact.sequence,
                            slot_generation,
                            entity_index: slot,
                            reserved: 0,
                        })
                })
                .collect::<Vec<_>>();
            if rows.len() > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    "voxel impact child buffer is too small",
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "voxel impact child buffer is null",
                ));
            }
            // SAFETY: size, capacity and required pointers were checked above.
            unsafe {
                if !rows.is_empty() {
                    std::ptr::copy_nonoverlapping(rows.as_ptr(), buffer, rows.len());
                }
                out_written.write(rows.len() as u32);
            }
            Ok(())
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnableRequest {
    selected_slot: u16,
    modern_heights: Vec<u32>,
}

#[derive(Serialize)]
struct VolumeProjection<'a> {
    kind: u8,
    slot: u16,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cell_size: u16,
    dimensions: [u16; 3],
    terrain_columns: Option<&'a [u16]>,
    building_bytes: Vec<u8>,
}
#[derive(Serialize)]
struct RemovedProjection {
    kind: u8,
    slot: u16,
    x: u16,
    y: u16,
    z: u16,
}

fn volume_identity(id: VolumeId) -> (u8, u16) {
    match id {
        VolumeId::Terrain => (0, 0),
        VolumeId::Building(slot) => (1, slot),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_voxel_enable_json_v1(
    handle: u64,
    generation: u64,
    tick: u64,
    request: *const u8,
    length: u32,
) -> i32 {
    boundary(|| {
        if length > 2 * 1024 * 1024 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "voxel request exceeds bound",
            ));
        }
        let text = utf8_input(request, length, "voxel enable request")?;
        let request: EnableRequest = serde_json::from_str(&text)
            .map_err(|e| AbiError::new(MC_STATUS_INVALID_ARGUMENT, e.to_string()))?;
        with_runtime_mut(handle, |record| {
            let session = record.runtime.session_mut().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel enable requires unpaused gameplay",
                )
            })?;
            if session.identity().generation != generation
                || session.backend().campaign_presentation_source().tick() != tick
            {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale voxel configuration",
                ));
            }
            session
                .backend_mut()
                .enable_voxel_world(request.selected_slot, request.modern_heights, generation)
                .map_err(|e| AbiError::new(MC_STATUS_VALIDATION_FAILED, e))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_voxel_state_v1(
    handle: u64,
    generation: u64,
    tick: u64,
    out: *mut McVoxelStateV1,
    size: u32,
) -> i32 {
    boundary_value(out, size, || {
        with_runtime(handle, |record| {
            let session = record.runtime.session().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel observation requires gameplay",
                )
            })?;
            let source = session.backend().campaign_presentation_source();
            if session.identity().generation != generation || source.tick() != tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale voxel observation",
                ));
            }
            let mut state = McVoxelStateV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McVoxelStateV1>(),
                generation,
                tick,
                ..Default::default()
            };
            let terrain = source.simulation().canonical_terrain();
            state.impact_head = terrain.voxel_impact_head();
            if let Some(world) = terrain.voxel_world() {
                state.enabled = 1;
                state.world_generation = world.generation();
                state.revision = world.revision();
                state.content_hash = world.deterministic_hash();
                state.removed_count = world.removed_count() as u32;
            }
            Ok(state)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_voxel_snapshot_json_v1(
    handle: u64,
    generation: u64,
    tick: u64,
    include_base: u32,
    after_impact: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |record| {
            if include_base > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "invalid voxel base flag",
                ));
            }
            let session = record.runtime.session().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel observation requires gameplay",
                )
            })?;
            let source = session.backend().campaign_presentation_source();
            if session.identity().generation != generation || source.tick() != tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale voxel snapshot",
                ));
            }
            let world: &VoxelWorld = source
                .simulation()
                .canonical_terrain()
                .voxel_world()
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_INVALID_ARGUMENT, "voxel geometry is disabled")
                })?;
            let terrain = source.simulation().canonical_terrain();
            if after_impact > terrain.voxel_impact_head() {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "voxel impact cursor is ahead of the native journal",
                ));
            }
            if after_impact != 0
                && terrain
                    .voxel_impacts()
                    .first()
                    .is_some_and(|row| after_impact.saturating_add(1) < row.sequence)
            {
                return Err(AbiError::new(
                    MC_STATUS_CURSOR_EXPIRED,
                    "voxel impact cursor predates the retained native journal",
                ));
            }
            let volumes = if include_base == 1 {
                world
                    .volumes()
                    .iter()
                    .map(|volume| {
                        let (kind, slot) = volume_identity(volume.id);
                        let grid = volume.grid;
                        VolumeProjection {
                            kind,
                            slot,
                            origin_x: grid.origin.x,
                            origin_y: grid.origin.y,
                            origin_z: grid.origin.z,
                            cell_size: grid.cell_size,
                            dimensions: grid.dimensions,
                            terrain_columns: volume.terrain_columns(),
                            building_bytes: volume
                                .building_bits()
                                .map(|bits| {
                                    bits.iter().flat_map(|word| word.to_le_bytes()).collect()
                                })
                                .unwrap_or_default(),
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let removed = world
                .removed_cells()
                .map(|cell| {
                    let (kind, slot) = volume_identity(cell.volume);
                    RemovedProjection {
                        kind,
                        slot,
                        x: cell.x,
                        y: cell.y,
                        z: cell.z,
                    }
                })
                .collect::<Vec<_>>();
            let terrain = source.simulation().canonical_terrain();
            let impacts=terrain.voxel_impacts().iter().filter(|row|row.sequence>after_impact).map(|row|serde_json::json!({
            "sequence":row.sequence.to_string(),"projectile_slot":row.projectile_slot,"slot_generation":row.slot_generation,"model":row.model,
            "from_x":row.from.x,"from_y":row.from.y,"from_z":row.from.z,"x":row.at.x,"y":row.at.y,"z":row.at.z,
            "velocity_x":row.velocity.x,"velocity_y":row.velocity.y,"velocity_z":row.velocity.z,
            "normal_x":row.normal.x,"normal_y":row.normal.y,"normal_z":row.normal.z,
            "revision":row.revision.to_string(),"target_slot":row.target.unwrap_or(0),"has_target":row.target.is_some(),"geometry_changed":row.geometry_changed,
            "has_impact_child":row.impact_child.is_some(),"impact_child_slot":row.impact_child.map_or(0,|p|p.0),"impact_child_generation":row.impact_child.map_or(0,|p|p.1)
        })).collect::<Vec<_>>();
            let value = serde_json::json!({"generation":generation.to_string(),"tick":tick.to_string(),"revision":world.revision().to_string(),"content_hash":world.deterministic_hash().to_string(),"base_hash":world.base_hash().to_string(),"impact_head":terrain.voxel_impact_head().to_string(),"volumes":volumes,"removed":removed,"impacts":impacts});
            super::super::game_session_abi::write_json(&value, buffer, capacity, out_length)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_death_queries_preserve_world_epoch_across_pause_and_reject_stale_commands() {
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let request=serde_json::to_vec(&serde_json::json!({
            "request_contract":INSTALLED_PACKAGE_SET_CONTRACT,
            "packages":[
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
        let enable=serde_json::to_vec(&serde_json::json!({"selected_slot":38,"modern_heights":vec![50_f32.to_bits();17*21+256*320]})).unwrap();
        assert_eq!(
            mc_game_runtime_voxel_enable_json_v1(
                handle,
                1,
                0,
                enable.as_ptr(),
                enable.len() as u32
            ),
            MC_STATUS_OK
        );
        let mut before = McVoxelStateV1::default();
        assert_eq!(
            mc_game_runtime_voxel_state_v1(handle, 1, 0, &mut before, 64),
            MC_STATUS_OK
        );
        assert_eq!(before.world_generation, 1);
        assert_eq!(mc_game_runtime_pause_v1(handle, 3), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_resume_v1(handle, 4), MC_STATUS_OK);
        let mut after = McVoxelStateV1::default();
        assert_eq!(
            mc_game_runtime_voxel_state_v1(handle, 3, 0, &mut after, 64),
            MC_STATUS_OK
        );
        assert_eq!(
            (after.world_generation, after.content_hash, after.revision),
            (
                before.world_generation,
                before.content_hash,
                before.revision
            )
        );
        let mut written = u32::MAX;
        // The old gate compared command generation3 with world epoch1 and
        // rejected this valid retained world. The external guard still rejects
        // commands captured before pause, and tick validation remains strict.
        let copy = super::super::super::character_abi::mc_native_campaign_voxel_deaths_copy;
        assert_eq!(
            copy(handle, 3, 0, 0, std::ptr::null_mut(), 0, 32, &mut written),
            MC_STATUS_OK
        );
        assert_eq!(written, 0);
        assert_eq!(
            copy(handle, 1, 0, 0, std::ptr::null_mut(), 0, 32, &mut written),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            copy(handle, 3, 1, 0, std::ptr::null_mut(), 0, 32, &mut written),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }

    #[test]
    fn voxel_state_layout_and_invalid_requests_are_contained() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<McVoxelStateV1>(), 64);
        assert_eq!(offset_of!(McVoxelStateV1, generation), 8);
        assert_eq!(offset_of!(McVoxelStateV1, content_hash), 32);
        assert_eq!(offset_of!(McVoxelStateV1, impact_head), 48);
        assert_eq!(offset_of!(McVoxelStateV1, world_generation), 56);
        assert_eq!(
            mc_game_runtime_voxel_state_v1(u64::MAX, 1, 0, std::ptr::null_mut(), 64),
            MC_STATUS_NULL_POINTER
        );
        let mut row = McVoxelStateV1::default();
        assert_eq!(
            mc_game_runtime_voxel_state_v1(u64::MAX, 1, 0, &mut row, 63),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_game_runtime_voxel_state_v1(u64::MAX, 1, 0, &mut row, 64),
            MC_STATUS_UNKNOWN_HANDLE
        );
        let malformed = b"{\"selected_slot\":38,\"modern_heights\":[],\"foreign\":1}";
        assert_eq!(
            mc_game_runtime_voxel_enable_json_v1(
                u64::MAX,
                1,
                0,
                malformed.as_ptr(),
                malformed.len() as u32
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_game_runtime_voxel_enable_json_v1(
                u64::MAX,
                1,
                0,
                std::ptr::null(),
                2 * 1024 * 1024 + 1
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
    }
}
