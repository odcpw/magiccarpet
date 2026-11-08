//! Source selector eligibility without the indexed renderer's player-view clip.
//! Independent 3D cameras apply their own spatial culling after this query.
use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_entity_presentations_copy(
    handle: u64,
    generation: u64,
    tick: u64,
    camera_yaw: i16,
    buffer: *mut McCampaignEntityPresentationV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignEntityPresentationV2>(
            element_size,
            "world entity presentation element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "world entity presentation requires a descriptor campaign session",
                ));
            };
            if generation != session.generation || tick != reset.campaign_tick() {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale world entity presentation query",
                ));
            }
            // Preserve native family lifecycle, hide flags and particle-table
            // selection. Only the legacy source-camera near/range clip is absent.
            let rows = reset
                .campaign_entity_presentations(camera_yaw)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
            if rows.len() > capacity as usize {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!(
                        "world entity presentation needs {} rows, capacity is {capacity}",
                        rows.len()
                    ),
                ));
            }
            if !rows.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "world entity presentation buffer is null",
                ));
            }
            for (ordinal, row) in rows.iter().enumerate() {
                let value = McCampaignEntityPresentationV2 {
                    abi_version: MC_NATIVE_ABI_VERSION,
                    struct_size: struct_size::<McCampaignEntityPresentationV2>(),
                    generation,
                    tick,
                    entity_index: row.entity_index,
                    source_id: row.source_id,
                    render_parameter_index: row.render_parameter_index,
                    base_tmap: row.base_tmap,
                    selected_tmap: row.selected_tmap,
                    logical_height: row.logical_height,
                    render_mode: row.render_mode,
                    orientation_sector: row.orientation_sector,
                    mirrored: u8::from(row.mirrored),
                    reserved_0: 0,
                    x: row.position.x,
                    y: row.position.y,
                    z: row.position.z,
                    yaw: row.yaw,
                    pixel_mode: row.pixel_mode,
                    owner_color: row.owner_color,
                    brightness: row.brightness,
                    draw_order: row.draw_order,
                    control_flags: row.control_flags,
                    primary_depth_adjustment: row.primary_depth_adjustment,
                    reserved: 0,
                };
                unsafe { buffer.add(ordinal).write_unaligned(value) };
            }
            write_output(
                out_written,
                u32::try_from(rows.len()).expect("entity arena fits u32"),
            );
            Ok(())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_presentation_rejects_invalid_boundary_without_writing_count() {
        let mut written = u32::MAX;
        assert_eq!(
            mc_native_campaign_world_entity_presentations_copy(
                u64::MAX,
                1,
                0,
                0,
                std::ptr::null_mut(),
                0,
                64,
                std::ptr::null_mut()
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(
            mc_native_campaign_world_entity_presentations_copy(
                u64::MAX,
                1,
                0,
                0,
                std::ptr::null_mut(),
                0,
                63,
                &mut written
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(written, u32::MAX);
        assert_eq!(
            mc_native_campaign_world_entity_presentations_copy(
                u64::MAX,
                1,
                0,
                0,
                std::ptr::null_mut(),
                0,
                64,
                &mut written
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(written, u32::MAX);
    }
}
