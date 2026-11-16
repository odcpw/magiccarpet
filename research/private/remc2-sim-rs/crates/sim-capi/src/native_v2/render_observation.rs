//! Cheap, identity-bound campaign presentation reads for the runtime client.
use super::*;
use serde::Serialize;

#[derive(Serialize)]
struct RenderObservation {
    contract: &'static str,
    identity: sim_session::SessionIdentity,
    // This counter is meaningful only together with the session generation.
    terrain_mutation_sequence: String,
    value: sim_core::campaign::backend::CampaignRenderObservation,
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_render_observation_json_v1(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        with_runtime(handle, |record| {
            let session = record.session()?;
            let backend = session.backend();
            let mut value = backend.render_observation();
            value.spell_menu_panels_opaque = record.runtime.identity().display_settings.panels_opaque;
            record.apply_local_presentation_name(
                &mut value.source_scoreboard,
                &mut value.participant_notifications,
            )?;
            let observation = RenderObservation {
                contract: "carpet-flight-campaign-render-observation/v1",
                identity: session.identity(),
                terrain_mutation_sequence: backend
                    .campaign_presentation_source()
                    .simulation()
                    .canonical_terrain()
                    .mutation_sequence()
                    .to_string(),
                value,
            };
            super::super::game_session_abi::write_json(&observation, buffer, capacity, out_length)
        })
    })
}

/// Copy a coherent, guarded MCPO v3 packet to a reusable caller-owned buffer.
/// A short buffer receives no bytes and `out_written` receives the exact size.
#[unsafe(no_mangle)]
pub extern "C" fn mc_game_runtime_render_observation_copy_v1(
    handle: u64,
    expected_runtime_generation: u64,
    expected_session_generation: u64,
    expected_tick: u64,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "render publication out_written")?;
        with_runtime(handle, |record| {
            let (runtime_id, runtime_generation, mode) = record.runtime.presentation_identity();
            if runtime_generation != expected_runtime_generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale render runtime generation",
                ));
            }
            let session = record.session()?;
            let identity = session.identity();
            if identity.generation != expected_session_generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "stale render session generation",
                ));
            }
            if identity.tick != expected_tick {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "stale render tick",
                ));
            }
            let backend = session.backend();
            let mut value = backend.render_observation();
            value.spell_menu_panels_opaque = record.runtime.identity().display_settings.panels_opaque;
            let local_slot = backend
                .campaign_presentation_source()
                .simulation()
                .shared_spells()
                .local_player()
                .slot;
            let local_name = record.runtime.presentation_player_name();
            if let Some(local) = value
                .source_scoreboard
                .players
                .iter_mut()
                .find(|player| player.source_slot == local_slot)
            {
                local.display_name = local_name.to_owned();
            }
            for notice in &mut value.participant_notifications {
                if notice.is_local {
                    notice.display_name = local_name.to_owned();
                }
            }
            let packet = sim_session::render_publication::RenderPublication {
                runtime_id,
                runtime_generation,
                paused: mode == RuntimeMode::Paused,
                identity: &identity,
                terrain_revision: backend
                    .campaign_presentation_source()
                    .simulation()
                    .canonical_terrain()
                    .mutation_sequence(),
                tick: value.tick,
                effect_head: value.effect_head,
                player_pose: &value.player_pose,
                player_motion: &value.player_motion,
                player_vitals: &value.player_vitals,
                source_menu_admission: value.source_menu_admission.as_ref(),
                return_to_title_requested: value.return_to_title_requested,
                source_scoreboard: &value.source_scoreboard,
                participant_notifications: &value.participant_notifications,
                player_wall_contact_counter: value.player_wall_contact_counter,
                player_immobilized_ticks: value.player_immobilized_ticks,
                source_camera_pose: &value.source_camera_pose,
                source_camera_fov: value.source_camera_fov,
                spell_menu_has_any_spell: value.spell_menu_has_any_spell,
                spell_menu_wizard_colors: value.spell_menu_wizard_colors,
                spell_menu_panels_opaque: value.spell_menu_panels_opaque,
                player_spells: &value.player_spells,
                objective_progress: &value.objective_progress,
                objective_textbox_ticks: value.objective_textbox_ticks,
                objective_secret_announcement_record: value.objective_secret_announcement_record,
                exit_sequence_active: value.exit_sequence_active,
                presentation_entities: &value.presentation_entities,
            };
            let length = packet
                .encoded_len()
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "render publication exceeds u32"))?;
            write_output(out_written, length);
            if capacity < length {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    "render publication buffer is too small",
                ));
            }
            if buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "render publication buffer is null",
                ));
            }
            let slice_length = usize::try_from(length)
                .ok()
                .filter(|length| *length <= isize::MAX as usize)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "render publication exceeds addressable slice size",
                    )
                })?;
            // SAFETY: the caller supplies a writable buffer of `capacity` bytes;
            // length was checked before any packet bytes are written.
            let bytes = unsafe { std::slice::from_raw_parts_mut(buffer, slice_length) };
            packet
                .encode_into(bytes)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "cannot encode render publication"))?;
            Ok(())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_v2::game_session_abi::{
        mc_game_session_snapshot_json_v1, mc_game_session_step_json_v1,
    };
    use std::path::PathBuf;

    fn package_set() -> Vec<u8> {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let paths = [
            std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
                || generated.join("mc-bhq8-level1-collision-v5-night"),
                PathBuf::from,
            ),
            std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
                || generated.join("mc-cf02-campaign-package-v2-b/qualified-level2"),
                PathBuf::from,
            ),
            std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
                || generated.join("mc-cf14-campaign-package-v1-a/qualified-level3"),
                PathBuf::from,
            ),
        ];
        serde_json::to_vec(&serde_json::json!({
            "request_contract": INSTALLED_PACKAGE_SET_CONTRACT,
            "packages": paths.iter().map(|path| serde_json::json!({"package_dir": path, "owner_cache_path": null})).collect::<Vec<_>>()
        })).unwrap()
    }

    fn identity(handle: u64) -> (u64, u64, u64, u64, RuntimeMode) {
        with_runtime(handle, |record| {
            let (id, generation, mode) = record.runtime.presentation_identity();
            let full = record.runtime.identity();
            assert_eq!(
                (id, generation, mode),
                (full.runtime_id, full.generation, full.mode)
            );
            assert_eq!(
                record.runtime.presentation_player_name(),
                full.player_preferences.preferences.player_name
            );
            let session = record.session()?.identity();
            Ok((id, generation, session.generation, session.tick, mode))
        })
        .unwrap()
    }

    fn packet(handle: u64) -> Vec<u8> {
        let (id, generation, session_generation, tick, mode) = identity(handle);
        let mut length = 0;
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                handle,
                generation,
                session_generation,
                tick,
                std::ptr::null_mut(),
                0,
                &raw mut length
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert!(length > 29);
        let mut short = vec![0xa5; length as usize];
        let mut required = 0;
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                handle,
                generation,
                session_generation,
                tick,
                short.as_mut_ptr(),
                length - 1,
                &raw mut required
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(required, length);
        assert!(short.iter().all(|b| *b == 0xa5));
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                handle,
                generation,
                session_generation,
                tick,
                std::ptr::null_mut(),
                length,
                &raw mut required
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                handle,
                generation,
                session_generation,
                tick,
                short.as_mut_ptr(),
                length,
                std::ptr::null_mut()
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                handle,
                generation,
                session_generation,
                tick,
                short.as_mut_ptr(),
                length,
                &raw mut required
            ),
            MC_STATUS_OK
        );
        assert_eq!(&short[..4], b"MCPO");
        assert_eq!(u32::from_le_bytes(short[4..8].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(short[8..12].try_into().unwrap()), length);
        assert_eq!(u64::from_le_bytes(short[12..20].try_into().unwrap()), id);
        assert_eq!(
            u64::from_le_bytes(short[20..28].try_into().unwrap()),
            generation
        );
        assert_eq!(short[28], u8::from(mode == RuntimeMode::Paused));
        short
    }

    fn snapshot(handle: u64) -> Vec<u8> {
        let mut length = 0;
        assert_eq!(
            mc_game_session_snapshot_json_v1(handle, std::ptr::null_mut(), 0, &raw mut length),
            MC_STATUS_BOUNDS
        );
        let mut out = vec![0; length as usize];
        assert_eq!(
            mc_game_session_snapshot_json_v1(handle, out.as_mut_ptr(), length, &raw mut length),
            MC_STATUS_OK
        );
        out
    }

    // Read just the panel bit from the public MCPO v3 packet. Walk the
    // variable-sized prefix instead of assuming a particular player name or
    // notice count, so this exercises the actual encoder and buffer path.
    fn packet_panels_opaque(packet: &[u8]) -> bool {
        struct Cursor<'a> { bytes: &'a [u8], at: usize }
        impl Cursor<'_> {
            fn skip(&mut self, n: usize) { self.at += n; assert!(self.at <= self.bytes.len()); }
            fn byte(&mut self) -> u8 { let b = self.bytes[self.at]; self.skip(1); b }
            fn count(&mut self) -> usize {
                let n = u32::from_le_bytes(self.bytes[self.at..self.at + 4].try_into().unwrap());
                self.skip(4); n as usize
            }
            fn string(&mut self) { let n = self.count(); self.skip(n); }
        }
        let mut c = Cursor { bytes: packet, at: 29 };
        for _ in 0..3 { c.string(); }
        c.skip(7 * 8 + 14 + 6 + 16);
        if c.byte() != 0 { c.skip(13); }
        c.skip(1); // return-to-title
        for _ in 0..c.count() {
            c.skip(1); c.string(); c.skip(2 + 4 + 16);
        }
        if c.byte() != 0 { c.string(); }
        for _ in 0..c.count() {
            c.skip(8 + 1); c.string(); c.string(); c.skip(2);
        }
        c.skip(1 + 2 + 14 + 1 + 2);
        match c.byte() { 0 => false, 1 => true, value => panic!("invalid MCPO panel bit {value}") }
    }

    #[test]
    fn binary_publication_guards_purity_and_lifecycle() {
        let mut handle = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut handle), MC_STATUS_OK);
        let packages = package_set();
        assert_eq!(
            mc_game_runtime_bootstrap_package_set_json_v1(
                handle,
                1,
                packages.as_ptr(),
                packages.len() as u32
            ),
            MC_STATUS_OK
        );
        assert_eq!(mc_game_runtime_open_level_v1(handle, 2), MC_STATUS_OK);
        assert!(!packet_panels_opaque(&packet(handle)));
        let (_, generation, session_generation, tick, _) = identity(handle);
        assert_eq!(mc_game_runtime_toggle_panel_transparency_v1(
            handle, generation, session_generation, tick
        ), MC_STATUS_OK);
        assert!(packet_panels_opaque(&packet(handle)));
        let before = snapshot(handle);
        let initial = packet(handle);
        assert_eq!(packet(handle), initial);
        assert_eq!(snapshot(handle), before);
        let (_, generation, session_generation, tick, _) = identity(handle);
        let mut changed = 9;
        assert_eq!(
            mc_game_runtime_finish_mission_if_ready_result_v1(
                handle,
                generation,
                session_generation,
                tick,
                &raw mut changed
            ),
            MC_STATUS_OK
        );
        assert_eq!(changed, 0);
        assert_eq!(packet(handle), initial);
        changed = 9;
        assert_eq!(
            mc_game_runtime_finish_mission_if_ready_result_v1(
                handle,
                generation - 1,
                session_generation,
                tick,
                &raw mut changed
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(changed, 0);
        assert_eq!(
            mc_game_runtime_finish_mission_if_ready_result_v1(
                handle,
                generation,
                session_generation,
                tick,
                std::ptr::null_mut()
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(packet(handle), initial);
        let mut sentinel = [0xa5; 64];
        let mut written = 77;
        for (outer, nested, expected_tick, status) in [
            (
                generation - 1,
                session_generation,
                tick,
                MC_STATUS_GENERATION_MISMATCH,
            ),
            (
                generation,
                session_generation + 1,
                tick,
                MC_STATUS_GENERATION_MISMATCH,
            ),
            (
                generation,
                session_generation,
                tick + 1,
                MC_STATUS_INVALID_ARGUMENT,
            ),
        ] {
            assert_eq!(
                mc_game_runtime_render_observation_copy_v1(
                    handle,
                    outer,
                    nested,
                    expected_tick,
                    sentinel.as_mut_ptr(),
                    sentinel.len() as u32,
                    &raw mut written
                ),
                status
            );
            assert_eq!(written, 77);
            assert_eq!(sentinel, [0xa5; 64]);
        }
        let invalid = br#"{"look_yaw":"invalid"}"#;
        assert_ne!(
            mc_game_session_step_json_v1(
                handle,
                session_generation,
                tick,
                invalid.as_ptr(),
                invalid.len() as u32
            ),
            MC_STATUS_OK
        );
        assert_eq!(packet(handle), initial);
        let command = br#"{"longitudinal":"accelerate","look_yaw":64}"#;
        assert_eq!(
            mc_game_session_step_json_v1(
                handle,
                session_generation,
                tick,
                command.as_ptr(),
                command.len() as u32
            ),
            MC_STATUS_OK
        );
        let stepped = packet(handle);
        assert_ne!(initial, stepped);
        assert_eq!(mc_game_runtime_pause_v1(handle, generation), MC_STATUS_OK);
        let (_, paused_generation, paused_session, paused_tick, _) = identity(handle);
        let paused = packet(handle);
        assert_ne!(paused, stepped);
        assert_eq!(
            mc_game_runtime_paused_frame_v1(handle, paused_generation, paused_session, paused_tick),
            MC_STATUS_OK
        );
        let paused_before = snapshot(handle);
        packet(handle);
        assert_eq!(snapshot(handle), paused_before);
        let save = with_runtime(handle, |record| {
            record.save_replay(handle, paused_generation, paused_session, paused_tick)
        })
        .unwrap();
        let mut restored = 0;
        assert_eq!(mc_game_runtime_create_v1(&raw mut restored), MC_STATUS_OK);
        assert_eq!(
            mc_game_runtime_load_replay_json_v1(
                restored,
                1,
                packages.as_ptr(),
                packages.len() as u32,
                save.as_ptr(),
                save.len() as u32
            ),
            MC_STATUS_OK
        );
        let (_, restored_generation, restored_session, restored_tick, restored_mode) =
            identity(restored);
        assert_eq!(restored_mode, RuntimeMode::Paused);
        assert_eq!(restored_tick, paused_tick);
        packet(restored);
        assert_eq!(
            mc_game_runtime_render_observation_copy_v1(
                restored,
                restored_generation,
                restored_session + 1,
                restored_tick,
                sentinel.as_mut_ptr(),
                sentinel.len() as u32,
                &raw mut written
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_game_runtime_destroy_v1(restored), MC_STATUS_OK);
        assert_eq!(mc_game_runtime_destroy_v1(handle), MC_STATUS_OK);
    }
}
