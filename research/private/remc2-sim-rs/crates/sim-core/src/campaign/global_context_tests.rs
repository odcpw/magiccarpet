use super::*;
use crate::game_runtime::{CampaignMenuCommand, GameRuntime, RuntimeMode};

fn resolver() -> Mc2RuntimePackageResolver {
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    Mc2RuntimePackageResolver::load([
        Mc2CampaignInputs::new(generated.join("mc-bhq8-level1-collision-v5-night")),
        Mc2CampaignInputs::new(generated.join("mc-cf02-campaign-package-v2-b/qualified-level2")),
        Mc2CampaignInputs::new(generated.join("mc-cf14-campaign-package-v1-a/qualified-level3")),
    ])
    .unwrap()
}

fn step(runtime: &mut GameRuntime<Mc2CampaignBackend>) {
    let session = runtime.session_mut().unwrap();
    session
        .step(session.identity().guard(), GameCommand::default())
        .unwrap();
}

#[test]
fn warm_source_context_survives_reset_detach_and_replay_after_load() {
    let resolver = resolver();
    let mut original = GameRuntime::<Mc2CampaignBackend>::title(81801).unwrap();
    let guard = original.identity().guard();
    let loading = resolver.new_game(&mut original, guard).unwrap();
    resolver
        .open_pending(&mut original, loading.guard(), 81801)
        .unwrap();
    // Controlled source-process context fixture. Ordinary Invisibility's
    // holder-A4 producer has its separate active-lifecycle regression.
    let first = GlobalSpecialState {
        protection_ticks: 0,
        visibility_mode: 1,
    };
    original
        .session_mut()
        .unwrap()
        .backend_mut()
        .install_initial_source_context(first)
        .unwrap();
    step(&mut original);
    let saved = resolver.save_replay(&original).unwrap();

    let mut cold = GameRuntime::<Mc2CampaignBackend>::title(81802).unwrap();
    let guard = cold.identity().guard();
    resolver
        .load_replay(&mut cold, guard, 81802, &saved)
        .unwrap();
    assert_eq!(
        cold.session().unwrap().backend().snapshot(),
        original.session().unwrap().backend().snapshot()
    );
    assert_eq!(resolver.save_replay(&cold).unwrap(), saved);

    let warm = GlobalSpecialState {
        protection_ticks: 0,
        visibility_mode: 2,
    };
    let mut loaded = GameRuntime::<Mc2CampaignBackend>::title(81803).unwrap();
    let guard = loaded.identity().guard();
    resolver
        .load_replay_with_source_context(&mut loaded, guard, 81803, &saved, warm)
        .unwrap();
    assert_eq!(resolver.source_global_context(&loaded).unwrap(), warm);
    let after_load = resolver.save_replay(&loaded).unwrap();
    let payload: Mc2CampaignReplaySave = serde_json::from_slice(&after_load).unwrap();
    assert_eq!(payload.initial_global_context, first);
    assert_eq!(
        payload.global_context_changes,
        [Mc2GlobalContextChange {
            before_operation: payload.operations.len(),
            context: warm
        }]
    );
    assert_ne!(
        payload.expected_state_hash,
        serde_json::from_slice::<Mc2CampaignReplaySave>(&saved)
            .unwrap()
            .expected_state_hash
    );
    step(&mut loaded);
    let continued_save = resolver.save_replay(&loaded).unwrap();
    let mut restored = GameRuntime::<Mc2CampaignBackend>::title(81804).unwrap();
    let guard = restored.identity().guard();
    resolver
        .load_replay(&mut restored, guard, 81804, &continued_save)
        .unwrap();
    assert_eq!(
        restored.session().unwrap().backend().snapshot(),
        loaded.session().unwrap().backend().snapshot()
    );
    assert_eq!(resolver.save_replay(&restored).unwrap(), continued_save);
    assert!(
        !resolver
            .detached_source_contexts
            .lock()
            .unwrap()
            .contains_key(&81803)
    );
    assert!(
        !resolver
            .detached_source_contexts
            .lock()
            .unwrap()
            .contains_key(&81804)
    );

    let session = restored.session_mut().unwrap();
    session.reset(session.identity().guard()).unwrap();
    assert_eq!(resolver.source_global_context(&restored).unwrap(), warm);
    assert!(
        restored
            .session()
            .unwrap()
            .backend()
            .global_context_changes
            .is_empty()
    );
    assert_eq!(
        restored
            .session()
            .unwrap()
            .backend()
            .package_campaign
            .source_global_context(),
        warm
    );

    let guard = loaded.identity().guard();
    let session_guard = loaded.session().unwrap().identity().guard();
    resolver
        .close_to_title(&mut loaded, guard, session_guard)
        .unwrap();
    assert_eq!(loaded.identity().mode, RuntimeMode::Title);
    assert_eq!(resolver.source_global_context(&loaded).unwrap(), warm);
    let guard = loaded.identity().guard();
    let loading = resolver.new_game(&mut loaded, guard).unwrap();
    let map = resolver
        .show_new_game_map(&mut loaded, loading.guard())
        .unwrap();
    let map_save = resolver.save_replay(&loaded).unwrap();
    let map_payload: serde_json::Value = serde_json::from_slice(&map_save).unwrap();
    assert!(map_payload.get("initial_global_context").is_none());
    assert!(map_payload.get("global_context_changes").is_none());
    let loading = resolver
        .campaign_menu_command(&mut loaded, map.guard(), CampaignMenuCommand::StartMission)
        .unwrap();
    resolver
        .open_pending(&mut loaded, loading.guard(), 81805)
        .unwrap();
    assert_eq!(resolver.source_global_context(&loaded).unwrap(), warm);
    assert!(
        !resolver
            .detached_source_contexts
            .lock()
            .unwrap()
            .contains_key(&81803)
    );
    assert_eq!(
        resolver.source_global_context(&original).unwrap(),
        first,
        "separate live game instances do not share context"
    );

    let mut malformed: Mc2CampaignReplaySave = serde_json::from_slice(&continued_save).unwrap();
    malformed.global_context_changes[0].before_operation = malformed.operations.len() + 1;
    malformed.receipt_sha256 = malformed.compute_receipt_sha256();
    let mut rejected = GameRuntime::<Mc2CampaignBackend>::title(81806).unwrap();
    let guard = rejected.identity().guard();
    assert!(
        resolver
            .load_replay(
                &mut rejected,
                guard,
                81806,
                &serde_json::to_vec(&malformed).unwrap()
            )
            .is_err()
    );
    assert_eq!(rejected.identity().mode, RuntimeMode::Title);
}
