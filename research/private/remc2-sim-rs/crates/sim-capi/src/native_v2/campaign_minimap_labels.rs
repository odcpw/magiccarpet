//! Beyond Sight's source FONT1 labels, bound to the same minimap projection.

use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_labels_json(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        campaign_abi::with_campaign_session(handle, |session| {
            let (plan, _) =
                campaign_abi::campaign_minimap_projection(session, generation, expected_tick)?;
            game_session_abi::write_json(
                &serde_json::json!({
                    "schema": "mc2-campaign-minimap-labels/v1",
                    "generation": generation,
                    "tick": expected_tick,
                    "labels": plan.labels,
                }),
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_minimap_labels_endpoint_requires_active_spell_and_exact_revision() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-level2-vault-successor-closure-20261025");
        let package = package.to_string_lossy();
        let selection = McCampaignSelectionV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignSelectionV2>(),
            external_level: 2,
            internal_level: 1,
            map_type: 0,
            ..McCampaignSelectionV2::default()
        };
        let mut handle = 0;
        assert_eq!(
            mc_native_campaign_session_open(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                &raw const selection,
                &raw mut handle,
            ),
            MC_STATUS_OK
        );
        let tick = campaign_abi::with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("opening package has a source campaign session")
            };
            assert!(
                reset
                    .shared_spells()
                    .pickups()
                    .iter()
                    .any(|row| row.index == 38 && row.model == 12)
            );
            assert!(
                reset
                    .players()
                    .iter()
                    .any(|player| player.is_ai && player.entity_index == 112)
            );
            Ok(reset.state.tick)
        })
        .expect("actual opening pickup and opponent producers");
        let mut required = 0;
        assert_eq!(
            mc_native_campaign_minimap_labels_json(
                handle,
                0,
                tick,
                std::ptr::null_mut(),
                0,
                &raw mut required
            ),
            MC_STATUS_BOUNDS
        );
        let mut bytes = vec![0; required as usize];
        assert_eq!(
            mc_native_campaign_minimap_labels_json(
                handle,
                0,
                tick,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &raw mut required
            ),
            MC_STATUS_OK
        );
        let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(snapshot["schema"], "mc2-campaign-minimap-labels/v1");
        assert_eq!(snapshot["labels"], serde_json::json!([]));
        for (generation, expected_tick) in [(1, tick), (0, tick + 1)] {
            required = 0;
            assert_eq!(
                mc_native_campaign_minimap_labels_json(
                    handle,
                    generation,
                    expected_tick,
                    bytes.as_mut_ptr(),
                    bytes.len() as u32,
                    &raw mut required
                ),
                MC_STATUS_GENERATION_MISMATCH
            );
            assert_eq!(required, 0, "stale requests do not publish a snapshot");
        }
        // Exercise the actual pickup callback and its numeric publication,
        // without granting a synthetic holder or setting an active timer.
        let acquired_tick = campaign_abi::with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
                unreachable!("opening package has a source campaign session")
            };
            let position = reset
                .shared_spells()
                .pickups()
                .iter()
                .find(|row| row.index == 38)
                .expect("ordinary Beyond Sight pickup")
                .position;
            for _ in 0..4 {
                reset.state.position = position;
                reset.step_outer_turn(PlayerCommand::default(), 0);
                if reset.shared_spells().beyond_sight_state().is_some() {
                    break;
                }
            }
            let holder = reset
                .shared_spells()
                .beyond_sight_state()
                .expect("ordinary overlap acquires the imported family-12 holder");
            assert_eq!(holder.active_value, 0);
            assert_eq!(reset.players()[0].enabled_spell_holders[12], 38);
            Ok(reset.state.tick)
        })
        .expect("actual opening source acquisition passes");
        let mut bytes = vec![0; 8192];
        assert_eq!(
            mc_native_campaign_minimap_labels_json(
                handle,
                0,
                acquired_tick,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &raw mut required,
            ),
            MC_STATUS_OK
        );
        let acquired: serde_json::Value =
            serde_json::from_slice(&bytes[..required as usize]).unwrap();
        assert_eq!(
            acquired["labels"],
            serde_json::json!([]),
            "owning an inactive Beyond Sight holder does not reveal the player label"
        );
        assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    }
}
