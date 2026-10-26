//! Source split map: live terrain, a narrow world viewport and shared map rows.

use super::*;
use sim_core::presentation::world_map::{compose_full_map, render_full_map_terrain};

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_map_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    indices: *mut u8,
    index_capacity: u32,
    palette: *mut u8,
    palette_capacity: u32,
    entities: *mut McCampaignMinimapEntityV2,
    entity_capacity: u32,
    marks: *mut McCampaignMinimapMarkV2,
    mark_capacity: u32,
    out_state: *mut McCampaignMinimapStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        require_output(indices, "world map index buffer")?;
        require_output(palette, "world map palette buffer")?;
        with_campaign_presentation_session_mut(handle, |record, _| {
            let (plan, local_player) = projection(record, generation, expected_tick)?;
            if index_capacity < 64000
                || palette_capacity < 768
                || entity_capacity < plan.entities.len() as u32
                || mark_capacity < plan.marks.len() as u32
            {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    "world map output buffers are too small",
                ));
            }
            if !plan.entities.is_empty() {
                require_output(entities, "world map entity buffer")?;
            }
            if !plan.marks.is_empty() {
                require_output(marks, "world map mark buffer")?;
            }
            let CampaignSessionKind::Campaign {
                inspection,
                reset,
                day_frame_assets,
                day_frame_state,
            } = &mut record.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "world map needs a campaign",
                ));
            };
            let assets = day_frame_assets.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "world map needs the selected environment assets",
                )
            })?;
            let materials = assets.environment_resources();
            let planes = reset.canonical_terrain().planes();
            let map = render_full_map_terrain(
                reset.world_map_camera().into(),
                &planes.terrain_types,
                &planes.shading,
                &planes.angles,
                reset.definition_identity().map_type_id == 2,
                assets.tablesd(),
            )
            .map_err(map_error)?;
            let world = day_frame_state
                .render_map_world(inspection, reset, assets, generation)
                .map_err(map_error)?;
            let frame = compose_full_map(&map, world, materials.colors()[0]).map_err(map_error)?;
            let colors = day_frame_state
                .map_world_palette()
                .ok_or_else(|| map_error("map palette was not committed"))?;
            let entity_rows = plan
                .entities
                .iter()
                .copied()
                .map(|row| campaign_minimap_entity_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            let mark_rows = plan
                .marks
                .iter()
                .copied()
                .map(|row| campaign_minimap_mark_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            // SAFETY: all capacities and non-null outputs were checked before
            // rendering or publishing. The caller owns disjoint output arrays.
            unsafe {
                indices.copy_from_nonoverlapping(frame.as_ptr(), frame.len());
                palette.copy_from_nonoverlapping(colors.as_ptr(), 768);
                if !entity_rows.is_empty() {
                    entities.copy_from_nonoverlapping(entity_rows.as_ptr(), entity_rows.len());
                }
                if !mark_rows.is_empty() {
                    marks.copy_from_nonoverlapping(mark_rows.as_ptr(), mark_rows.len());
                }
            }
            Ok(campaign_minimap_state_snapshot(
                generation,
                expected_tick,
                local_player,
                &plan,
            ))
        })
    })
}

fn map_error(error: impl std::fmt::Display) -> AbiError {
    AbiError::new(
        MC_STATUS_VALIDATION_FAILED,
        format!("source world map: {error}"),
    )
}

fn projection(
    record: &CampaignSessionRecord,
    generation: u64,
    tick: u64,
) -> AbiResult<(Level2MinimapPlan, u16)> {
    // Reuse the canonical identity/local-player guard. Geometry and visibility
    // are then produced together for the full map rather than scaling dots.
    let (_, local_player) = campaign_minimap_projection(record, generation, tick)?;
    let CampaignSessionKind::Campaign { reset, .. } = &record.kind else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "world map needs a campaign",
        ));
    };
    let plan = crate::presentation_bridge::campaign_world_map_plan(
        reset,
        reset.definition_identity(),
        reset.world_map_camera(),
    )
    .map_err(map_error)?;
    Ok((plan, local_player))
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_map_labels_json(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_campaign_session(handle, |record| {
            let (plan, _) = projection(record, generation, expected_tick)?;
            game_session_abi::write_json(
                &serde_json::json!({
                    "schema": "mc2-campaign-minimap-labels/v1",
                    "generation": generation, "tick": expected_tick, "labels": plan.labels,
                }),
                buffer,
                capacity,
                out_length,
            )
        })
    })
}
