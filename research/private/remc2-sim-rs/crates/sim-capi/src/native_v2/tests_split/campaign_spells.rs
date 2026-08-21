#[test]
fn level2_campaign_public_tier0_possession_claims_group1_mana_from_strict_target_flight() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let row11_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Possession proof requires a Level 2 campaign session",
            ));
        };
        reset
            .mission()
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 11)
            .map(|feature| feature.world_position(0))
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing authored row 11"))
    })
    .expect("read Level 2 row-11 objective position");
    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        player_x: row11_position.x,
        player_y: row11_position.y,
        // This is the regular Day flight height at the row-11 objective,
        // not a target-height or mailbox shortcut.
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let baseline_sphere_ids = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| sphere.active != 0 && sphere.model == 39)
        .map(|sphere| sphere.sphere_id)
        .collect::<Vec<_>>();
    let (_, group_events) = step_level2_campaign_public(handle, input);
    assert!(group_events.iter().any(|event| {
        event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
            && event.source_slot == 23
            && event.family == 1
            && event.flags == 33
    }));
    let group1_spheres = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.mana == 512
                && sphere.owner_entity_index == 0
                && !baseline_sphere_ids.contains(&sphere.sphere_id)
        })
        .collect::<Vec<_>>();
    assert!(
        group1_spheres.len() >= 6,
        "ordinary row-11 production exposes at least six neutral group-1 model-39 spheres"
    );
    let group1_sphere_ids = group1_spheres
        .iter()
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    let target = group1_spheres
        .iter()
        .copied()
        .find(|sphere| sphere.sphere_id == 141)
        .expect("the pinned public six-overlap group-1 cluster is materialized");
    let selected_target_sphere_id = target.sphere_id;

    let Level2PublicPossessionClaimRoute {
        mut input,
        mut latest_snapshot,
        ..
    } = drive_level2_public_group1_possession_claims(
        handle,
        input,
        selected_target_sphere_id,
        &group1_sphere_ids,
    );

    // Continue the same public session through the known-valid Castle
    // route.  The six claimed model-39 rows are now live campaign inputs
    // for the Castle defender, not a fixture-only claim assertion.
    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    input.command = valid_command();
    while latest_snapshot.revision < 27 {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
    }
    assert_eq!(
        latest_snapshot.revision, 27,
        "the neutral public route reaches the Castle recovery turn"
    );
    let recovered_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert!(
        recovered_hud.local_player_mana >= 1_000,
        "neutral public turns recover enough Day mana for Castle"
    );

    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    let (castle_armed_snapshot, castle_arm_events) = step_level2_campaign_public(handle, input);
    assert_eq!(castle_armed_snapshot.revision, 28);
    assert_eq!(
        castle_arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        1,
        "the public Castle request dispatches one live flyer callback in its source pass"
    );
    assert!(
        castle_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1))
    );

    input.command = valid_command();
    let (mut castle_snapshot, castle_root_events) = step_level2_campaign_public(handle, input);
    assert_eq!(castle_snapshot.revision, 29);
    assert!(
        castle_root_events
            .iter()
            .any(|event| event.kind == 8 && event.family == 1),
        "the known-valid public Castle route allocates its root"
    );

    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    input.command = valid_command();

    let public_economy_events = || {
        let mut rows = [McCampaignEconomyEventV2::default(); 64];
        let mut count = 0;
        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignEconomyEventV2>(),
                &raw mut count,
            ),
            MC_STATUS_OK
        );
        rows[..count as usize].to_vec()
    };
    let mut defender_collection = None;
    let mut castle_deposit = None;
    while castle_snapshot.revision < 1_024 && castle_deposit.is_none() {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        let economy_events = public_economy_events();
        defender_collection = defender_collection.or_else(|| {
            economy_events
                .iter()
                .find(|event| event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE)
                .map(|event| (castle_snapshot.revision, event.value_0))
        });
        castle_deposit = castle_deposit.or_else(|| {
            economy_events
                .iter()
                .find(|event| event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED)
                .map(|event| (castle_snapshot.revision, event.value_0))
        });
    }
    assert_eq!(
        defender_collection.map(|(_, value)| value),
        Some(2_560),
        "the first public Castle collection receipt carries the five-sphere merged payload"
    );
    assert_eq!(
        castle_deposit.map(|(_, value)| value),
        Some(3_072),
        "the later public Castle arrival deposits all six claimed 512-mana rows"
    );
    assert!(
        defender_collection.expect("bounded collection receipt").0
            < castle_deposit.expect("bounded arrival receipt").0,
        "the public Castle defender collects before its later arrival deposit"
    );

    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    let possession_holder = |hud: McCampaignDayHudSnapshotV2| {
        [hud.left, hud.right]
            .into_iter()
            .find(|slot| slot.raw_spell_selector == 1)
            .expect("Day HUD exposes the Possession holder")
    };
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        pitch: 127,
        flags: 0x41,
        requested_spell: 1,
        ..McCommandV2::default()
    };
    let (rearmed_snapshot, rearm_events) = step_level2_campaign_public(handle, input);
    assert_eq!(
        rearm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 39)
            .count(),
        1,
        "the zero-cooldown request re-arms and dispatches one new Possession flyer in the same pass"
    );
    assert_eq!(
        possession_holder(campaign_day_hud_public(
            handle,
            rearmed_snapshot.generation,
            rearmed_snapshot.revision,
        ))
        .active_value,
        2,
        "the same-pass source callback advances the re-armed holder"
    );
    input.command = valid_command();
    let (_, rearm_launch_events) = step_level2_campaign_public(handle, input);
    assert!(rearm_launch_events.iter().all(|event| {
        !(event.kind == 8 && event.family == 39)
            || event.source_slot
                == rearm_events
                    .iter()
                    .find(|event| event.kind == 8 && event.family == 39)
                    .expect("same-pass re-arm flyer receipt")
                    .source_slot
    }));
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_public_beyond_sight_completes_ordinary_economy_and_reset_route() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let isolated_handle =
        open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let initial_hash = campaign_world_hash_public(handle);
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);
    let initial_player = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("initial public local-player projection is available");
    assert_eq!(
        campaign_players_public(isolated_handle)
            .into_iter()
            .find(|player| player.active != 0 && player.slot == 0)
            .expect("isolated public local-player projection is available"),
        initial_player
    );
    let read_family12_projection = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Beyond Sight XP proof requires a Level 2 campaign session",
                ));
            };
            let player = reset
                .shared_spells()
                .players()
                .iter()
                .find(|player| player.slot == 0)
                .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing local player row"))?;
            Ok((
                player.persistent_spell_experience[12],
                player.runtime_spell_experience[12],
                player.spell_unlocked_levels[12],
            ))
        })
        .expect("read-only family-12 XP/unlocked projection")
    };
    assert_eq!(read_family12_projection(), (0, 0, 0));

    let mut world_info = McCampaignWorldInfoV2::default();
    assert_eq!(
        mc_native_campaign_world_info(
            handle,
            &raw mut world_info,
            struct_size::<McCampaignWorldInfoV2>(),
        ),
        MC_STATUS_OK
    );
    let source_pass_bound = usize::try_from(world_info.entity_count)
        .expect("source entity count fits usize")
        .saturating_mul(8);
    assert!(source_pass_bound > 0);

    let row11_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Beyond Sight proof requires a Level 2 campaign session",
            ));
        };
        reset
            .mission()
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 11)
            .map(|feature| feature.world_position(0))
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing authored row 11"))
    })
    .expect("read Level 2 row-11 objective position");
    let (row42_position, row65_position) = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Beyond Sight proof requires a Level 2 campaign session",
            ));
        };
        let feature_position = |authored_index| {
            reset
                .mission()
                .authored_features()
                .iter()
                .copied()
                .find(|feature| feature.index == authored_index)
                .map(|feature| feature.world_position(0))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("missing authored row {authored_index}"),
                    )
                })
        };
        Ok((feature_position(42)?, feature_position(65)?))
    })
    .expect("read Level 2 switch-chain positions");
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        player_x: row11_position.x,
        player_y: row11_position.y,
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let baseline_sphere_ids = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| sphere.active != 0 && sphere.model == 39)
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    start_campaign_step_recording(handle);

    let mut family12_rows = [McCampaignEntitySnapshotV2::default(); 1];
    let mut family12_written = 0;
    assert_eq!(
        mc_native_campaign_entities_copy(
            handle,
            38,
            family12_rows.as_mut_ptr(),
            1,
            struct_size::<McCampaignEntitySnapshotV2>(),
            &raw mut family12_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(family12_written, 1);
    let family12_source = family12_rows[0];
    assert_eq!(
        (
            family12_source.entity_index,
            family12_source.active,
            family12_source.class_id,
            family12_source.model,
            family12_source.action,
            family12_source.phase,
        ),
        (38, 1, 15, 12, 37, 39),
        "the public entity projection exposes the static family-12 source row"
    );
    let (mut latest_snapshot, group_events) = step_level2_campaign_public(handle, input);
    assert!(
        group_events.iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
                && event.source_slot == 23
                && event.family == 1
                && event.flags == 33
        }),
        "row-11 authored group receipt identifies group 1"
    );
    assert!(
        group_events.iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
                && event.source_slot == 23
                && event.family == 1
                && event.flags == 1
        }),
        "row-11 latent group-1 row remains a separate one-row activation"
    );
    let group1_spheres = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.mana == 512
                && sphere.owner_entity_index == 0
                && !baseline_sphere_ids.contains(&sphere.sphere_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        group1_spheres.len(),
        28,
        "ordinary row-11 production materializes all 28 source group-1 spheres: {group1_spheres:?}"
    );
    let group1_sphere_ids = group1_spheres
        .iter()
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();

    let claim_cluster_priority_ids = [
        (640_u16, 45_440_u16),
        (60_288, 39_552),
        (896, 33_152),
        (60_032, 38_784),
        (6_272, 39_552),
    ]
    .into_iter()
    .map(|(source_x, source_y)| {
        group1_spheres
            .iter()
            .min_by_key(|sphere| {
                u32::from(sphere.x.abs_diff(source_x)) + u32::from(sphere.y.abs_diff(source_y))
            })
            .expect("group-1 source cluster has a public sphere representative")
            .sphere_id
    })
    .collect::<Vec<_>>();
    assert_eq!(
        claim_cluster_priority_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        5,
        "the productive authored overlap-cluster representatives resolve to distinct public identities"
    );

    let mut player_entity_index = latest_snapshot.player_entity_index;
    let mut attempted_target_ids = BTreeSet::new();
    let mut claimed_group1_ids = BTreeSet::new();
    let mut group1_merge_lineage = BTreeSet::new();
    let mut claim_route_deltas = Vec::new();
    // These authored rows are deterministic representatives of productive
    // public overlap clusters. Taking the representatives first
    // avoids spending whole projectile lifetimes on neutral rows already
    // covered by another cluster while grounded spheres continue their
    // ordinary merge lifecycle.
    while attempted_target_ids.len() < group1_sphere_ids.len() {
        let Some(target) = campaign_mana_spheres_public(handle, latest_snapshot.generation)
            .into_iter()
            .filter(|sphere| {
                sphere.active != 0
                    && sphere.model == 39
                    && sphere.owner_entity_index == 0
                    && group1_sphere_ids.contains(&sphere.sphere_id)
                    && !attempted_target_ids.contains(&sphere.sphere_id)
            })
            .min_by_key(|sphere| {
                (
                    claim_cluster_priority_ids
                        .iter()
                        .position(|source_id| *source_id == sphere.sphere_id)
                        .unwrap_or(claim_cluster_priority_ids.len()),
                    sphere.sphere_id,
                )
            })
        else {
            break;
        };
        attempted_target_ids.insert(target.sphere_id);
        assert_eq!(target.owner_entity_index, 0);
        let route = drive_level2_public_group1_possession_claims_with_expected(
            handle,
            input,
            target.sphere_id,
            &group1_sphere_ids,
            None,
        );
        input = route.input;
        latest_snapshot = route.latest_snapshot;
        player_entity_index = route.player_entity_index;
        let new_claimed_group1_sphere_ids = route.new_claimed_group1_sphere_ids;
        group1_merge_lineage.extend(route.merge_lineage);
        claim_route_deltas.push((target.sphere_id, new_claimed_group1_sphere_ids.clone()));
        let claimed = campaign_mana_spheres_public(handle, latest_snapshot.generation)
            .into_iter()
            .filter(|sphere| {
                sphere.active != 0
                    && sphere.model == 39
                    && sphere.owner_entity_index == player_entity_index
                    && group1_sphere_ids.contains(&sphere.sphere_id)
            })
            .map(|sphere| sphere.sphere_id)
            .collect::<BTreeSet<_>>();
        claimed_group1_ids.extend(new_claimed_group1_sphere_ids);
        let mut covered_by_claim_lineage = claimed_group1_ids.clone();
        loop {
            let previous_len = covered_by_claim_lineage.len();
            for (claimed_source, surviving_target) in &group1_merge_lineage {
                if covered_by_claim_lineage.contains(claimed_source) {
                    covered_by_claim_lineage.insert(*surviving_target);
                }
            }
            if covered_by_claim_lineage.len() == previous_len {
                break;
            }
        }
        assert!(
            claimed.is_subset(&covered_by_claim_lineage),
            "every live owned sphere is covered by a public claim or claimed-source -> surviving-target merge lineage"
        );
    }
    assert!(
        claimed_group1_ids.len() >= 23,
        "bounded ordinary Possession flights establish the Bead's public claim floor (attempted={attempted_target_ids:?}, claimed={claimed_group1_ids:?}, deltas={claim_route_deltas:?})"
    );
    assert_ne!(player_entity_index, 0);
    let claimed_spheres = campaign_mana_spheres_public(handle, latest_snapshot.generation)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
                && group1_sphere_ids.contains(&sphere.sphere_id)
        })
        .collect::<Vec<_>>();
    assert!(
        !claimed_spheres.is_empty(),
        "the historical public claims retain live owned mana before Castle collection"
    );
    assert!(
        claimed_spheres
            .iter()
            .map(|sphere| sphere.mana)
            .sum::<i32>()
            >= 10_240,
        "the merge-aware live claim set covers the 10240 Castle-deposit floor (live={}, historical={}, attempted={})",
        claimed_spheres.len(),
        claimed_group1_ids.len(),
        attempted_target_ids.len()
    );

    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    let mut hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    for _ in 0..source_pass_bound {
        if hud.local_player_mana >= 1_000 {
            break;
        }
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        hud = campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    }
    assert!(hud.local_player_mana >= 1_000);

    let castle_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    input.command = castle_command;
    let (castle_request_snapshot, castle_arm_events) = step_level2_campaign_public(handle, input);
    latest_snapshot = castle_request_snapshot;
    assert_eq!(
        castle_arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        1,
        "player slot 111 arms Castle before holder 117 and the live flyer callback publishes once in the same pass"
    );
    input.command = valid_command();
    assert!(
        castle_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1)),
        "the Castle root remains a later dynamic lifecycle receipt"
    );
    let mut castle_root_seen = false;
    for _ in 0..64 {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        castle_root_seen |= events
            .iter()
            .any(|event| event.kind == 8 && event.family == 1);
        if castle_root_seen {
            break;
        }
    }
    assert!(castle_root_seen, "ordinary Castle build allocates its root");

    let read_castle = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Castle read requires a Level 2 campaign session",
                ));
            };
            Ok(reset.economy().castle().map(|castle| {
                (
                    reset.economy().player_castle_stage(),
                    castle.level,
                    castle.action,
                    castle.build_state,
                    castle.mana,
                    castle.defender_count,
                )
            }))
        })
        .expect("read-only public Castle lifecycle state")
    };
    let mut stage_one_ready = false;
    for _ in 0..source_pass_bound {
        stage_one_ready = read_castle()
            .is_some_and(|state| state.0 >= 1 && state.1 == 1 && state.2 == 4 && state.3 == 0);
        if stage_one_ready {
            break;
        }
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
    }
    assert!(
        stage_one_ready,
        "ordinary Castle build reaches stable stage one"
    );

    let mut upgrade_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    for _ in 0..source_pass_bound {
        if upgrade_hud.local_player_mana >= 1_000 {
            break;
        }
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        upgrade_hud =
            campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    }
    assert!(
        upgrade_hud.local_player_mana >= 1_000,
        "ordinary Castle-overlap upkeep recovers the public upgrade mana gate"
    );

    let upgrade_root_slot = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle read requires a Level 2 campaign session",
            ));
        };
        reset
            .economy()
            .castle()
            .map(|castle| castle.source_entity_index)
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing occupied Castle root"))
    })
    .expect("read occupied Castle root identity before recast");

    // Recast the occupied Castle through the ordinary selected-spell
    // command. At this Castle-overlap geometry the higher-slot flyer
    // reaches the occupied root in the same source pass, retires, and
    // dispatches the higher-slot upgrade child. The already-visited root
    // consumes that mailbox through its normal Castle route on the next
    // ascending pass.
    input.command = castle_command;
    let (_upgrade_request_snapshot, upgrade_arm_events) =
        step_level2_campaign_public(handle, input);
    assert_eq!(
        upgrade_arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 2)
            .count(),
        1,
        "the occupied Castle recast dispatches exactly one upgrade-child callback"
    );
    assert!(
        upgrade_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1)),
        "the already-visited lower root cannot run again after its higher-slot upgrade child"
    );
    assert!(
        upgrade_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 38)),
        "the occupied-root hit retires the upgrade flyer before it can publish a live-projectile callback"
    );
    input.command = valid_command();
    let (upgrade_root_snapshot, upgrade_root_events) = step_level2_campaign_public(handle, input);
    latest_snapshot = upgrade_root_snapshot;
    assert!(
        upgrade_root_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 38)),
        "the following pass cannot republish the consumed upgrade flyer"
    );
    assert_eq!(
        upgrade_root_events
            .iter()
            .filter(|event| event.pass == 0
                && event.kind == 7
                && event.source_slot == upgrade_root_slot)
            .count(),
        1,
        "the following ascending pass dispatches the already-live Castle root exactly once"
    );
    let mut stage_two_ready = false;
    for _ in 0..source_pass_bound {
        stage_two_ready = read_castle()
            .is_some_and(|state| state.0 >= 2 && state.1 >= 2 && state.2 == 4 && state.3 == 0);
        if stage_two_ready {
            break;
        }
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
    }
    assert!(
        stage_two_ready,
        "ordinary Castle upgrade reaches stable stage two"
    );

    // The stable root transition can land on an odd source phase. The
    // ordinary action-4 maintenance callback runs on the next eligible
    // even phase and materializes the one stage-two model-3 defender.
    let mut stage_two_defender_ready = false;
    for _ in 0..source_pass_bound {
        stage_two_defender_ready = read_castle().is_some_and(|state| {
            state.0 == 2 && state.1 == 2 && state.2 == 4 && state.3 == 0 && state.5 == 1
        });
        if stage_two_defender_ready {
            break;
        }
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
    }
    assert!(
        stage_two_defender_ready,
        "ordinary stage-two maintenance materializes its one source defender"
    );

    // The economy sidecar is a per-outer-turn receipt, not a cumulative
    // journal. Establish the baseline only after the stage-two defender
    // exists, then accumulate source returns across ordinary turns.
    let _stage_two_economy_baseline = campaign_economy_events_public(handle);
    let mut collected = BTreeMap::<u32, i32>::new();
    let mut stage_two_defender_id = None;
    // Bind the accumulated return to the one stage-two defender rather
    // than inventing multiple defender identities or fixed-size merges.
    let mut arrived = BTreeMap::<u64, McCampaignEconomyEventV2>::new();
    for _ in 0..source_pass_bound {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        let turn_economy_events = campaign_economy_events_public(handle);
        for event in turn_economy_events.iter().filter(|event| {
            event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE
                && group1_sphere_ids.contains(&event.secondary_id)
                && claimed_group1_ids.contains(&event.secondary_id)
        }) {
            if let Some(defender_id) = stage_two_defender_id {
                assert_eq!(event.primary_id, defender_id);
            } else {
                stage_two_defender_id = Some(event.primary_id);
            }
            assert!(
                event.value_0 > 0 && event.value_0 % 512 == 0,
                "each collected claimed aggregate preserves an integral number of authored 512-mana spheres"
            );
            assert!(
                collected
                    .insert(event.secondary_id, event.value_0)
                    .is_none(),
                "each claimed aggregate identity is collected once"
            );
        }
        for event in turn_economy_events
            .iter()
            .filter(|event| event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED)
        {
            assert_eq!(Some(event.primary_id), stage_two_defender_id);
            assert_eq!(event.entity_0, upgrade_root_slot);
            arrived.insert(latest_snapshot.revision, *event);
        }
        if collected.values().copied().sum::<i32>() >= 10_240 && !arrived.is_empty() {
            break;
        }
    }
    let collected_mana = collected.values().copied().sum::<i32>();
    assert!(collected_mana >= 10_240);
    assert!(
        collected_mana / 512 >= 20,
        "the public aggregate receipts account for at least twenty authored 512-mana spheres"
    );
    assert!(!arrived.is_empty());
    assert!(
        arrived
            .values()
            .take(1)
            .map(|event| event.value_0)
            .sum::<i32>()
            >= 10_240,
        "the public Castle defender arrival deposits at least twenty authored sphere-equivalents"
    );
    assert!(
        arrived
            .values()
            .take(1)
            .all(|event| event.value_0 >= 10_240),
        "with zero model-45 aggregate, this stage-two defender returns after reaching its source max-mana threshold"
    );
    let retained_group1 = campaign_mana_spheres_public(handle, latest_snapshot.generation)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
                && group1_sphere_ids.contains(&sphere.sphere_id)
        })
        .map(|sphere| (sphere.sphere_id, sphere.mana))
        .collect::<BTreeMap<_, _>>();
    // Possession effects remain ordinary live callbacks while Castle
    // construction proceeds, so observe any later ownership writes from
    // the final public sphere snapshot rather than freezing the ledger at
    // the helper boundary.
    claimed_group1_ids.extend(retained_group1.keys().copied());
    assert!(claimed_group1_ids.len() >= 23);
    let collected_ids = collected.keys().copied().collect::<BTreeSet<_>>();
    assert!(
        retained_group1
            .keys()
            .all(|sphere_id| !collected_ids.contains(sphere_id)),
        "retained claimed spheres are distinct from the aggregate identities already deposited"
    );
    assert!(
        retained_group1.values().copied().sum::<i32>() >= 1_536,
        "the accumulated group-1 survivor carries the player max-mana reserve"
    );

    // The dense group-1 batch conserves enough mana but source merging
    // and the one stage-two defender can collapse its reserve to a single
    // live identity. Continue the already-authored switch route instead
    // of substituting a mana-equivalent assertion: row 42 produces group
    // 3, its inverse-overlap row 64 produces row 65, and row 65 produces
    // the spatially isolated group-5 model-39 rows used below.
    input.player_x = row42_position.x;
    input.player_y = row42_position.y;
    input.player_z = row42_position.z;
    input.command = valid_command();
    let mut group3_sphere_ids = BTreeSet::new();
    for _ in 0..8 {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        let Some(group3_event) = events.iter().find(|event| {
            event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP && event.family == 3 && event.flags == 22
        }) else {
            continue;
        };
        group3_sphere_ids.extend(
            campaign_economy_events_public(handle)
                .into_iter()
                .filter(|event| {
                    event.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED
                        && event.entity_0 == group3_event.source_slot
                        && event.value_0 == 512
                })
                .map(|event| event.primary_id),
        );
        break;
    }
    assert_eq!(
        group3_sphere_ids.len(),
        20,
        "the ordinary row-42 receipt materializes all twenty group-3 model-39 identities"
    );

    input.player_x = row65_position.x;
    input.player_y = row65_position.y;
    input.player_z = row65_position.z;
    input.command = valid_command();
    let mut group5_sphere_ids = BTreeSet::new();
    for _ in 0..source_pass_bound {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        let Some(group5_event) = events.iter().find(|event| {
            event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP && event.family == 5 && event.flags == 18
        }) else {
            continue;
        };
        group5_sphere_ids.extend(
            campaign_economy_events_public(handle)
                .into_iter()
                .filter(|event| {
                    event.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED
                        && event.entity_0 == group5_event.source_slot
                        && event.value_0 == 512
                })
                .map(|event| event.primary_id),
        );
        break;
    }
    assert_eq!(
        group5_sphere_ids.len(),
        16,
        "the ordinary row-65 receipt materializes all sixteen group-5 model-39 identities"
    );

    // The source model-1 flyer chooses its own eligible model-39 target
    // from the source-pass snapshot before its first move. Do not replace
    // that production decision with a test-side trajectory predictor:
    // drive bounded ordinary requests from public neutral anchors and
    // retain only ownership deltas actually observed through the ABI.
    let mut attempted_group5_anchor_ids = BTreeSet::new();
    let mut claimed_group5_ids = BTreeSet::new();
    for _ in 0..group5_sphere_ids.len() {
        let Some(target) = campaign_mana_spheres_public(handle, latest_snapshot.generation)
            .into_iter()
            .filter(|sphere| {
                sphere.active != 0
                    && sphere.model == 39
                    && sphere.owner_entity_index == 0
                    && group5_sphere_ids.contains(&sphere.sphere_id)
                    && !attempted_group5_anchor_ids.contains(&sphere.sphere_id)
            })
            .min_by_key(|sphere| (sphere.source_entity_index, sphere.sphere_id))
        else {
            break;
        };
        attempted_group5_anchor_ids.insert(target.sphere_id);
        let route = drive_level2_public_group1_possession_claims_with_expected(
            handle,
            input,
            target.sphere_id,
            &group5_sphere_ids,
            None,
        );
        input = route.input;
        latest_snapshot = route.latest_snapshot;
        player_entity_index = route.player_entity_index;
        claimed_group5_ids.extend(route.new_claimed_group1_sphere_ids);
        let live_owned_group5_ids =
            campaign_mana_spheres_public(handle, latest_snapshot.generation)
                .into_iter()
                .filter(|sphere| {
                    sphere.active != 0
                        && sphere.model == 39
                        && sphere.owner_entity_index == player_entity_index
                        && group5_sphere_ids.contains(&sphere.sphere_id)
                })
                .map(|sphere| sphere.sphere_id)
                .collect::<BTreeSet<_>>();
        assert!(
            live_owned_group5_ids.is_subset(&claimed_group5_ids),
            "every live group-5 identity was observed crossing the public ownership boundary"
        );
        if live_owned_group5_ids.len() >= 3 {
            break;
        }
    }
    let retained = campaign_mana_spheres_public(handle, latest_snapshot.generation)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
                && group5_sphere_ids.contains(&sphere.sphere_id)
        })
        .map(|sphere| (sphere.sphere_id, sphere.mana))
        .collect::<BTreeMap<_, _>>();
    assert!(
        retained
            .keys()
            .all(|sphere_id| claimed_group5_ids.contains(sphere_id)),
        "every retained group-5 identity was observed as a public ownership delta"
    );
    assert!(
        retained.len() >= 3,
        "the ordinary Castle/group-5 route retains at least three distinct owned live spheres: {retained:?}"
    );
    assert!(
        retained.values().copied().sum::<i32>() >= 1_536,
        "the retained owned live sphere identities carry at least 1536 source mana"
    );

    // The static family-12 source has zero life and therefore remains
    // available throughout the economy route. Acquire it only after the
    // aggregate-deposit/three-retained window is established so its dynamic
    // holder allocation cannot perturb the earlier sphere-slot identities.
    input.player_x = family12_source.x;
    input.player_y = family12_source.y;
    input.player_z = family12_source.z;
    input.command = valid_command();
    let mut beyond_public_events = Vec::new();
    let mut acquired_family12 = false;
    for _ in 0..8 {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        beyond_public_events.extend(events.iter().copied());
        acquired_family12 |= events.iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 12
                && event.source_slot == 38
                && event.flags == 1
        });
        if acquired_family12 {
            break;
        }
    }
    assert!(
        acquired_family12,
        "ordinary C-ABI overlap acquires static family-12 row 38"
    );
    let cold_beyond = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    assert_eq!(
        (cold_beyond.active, cold_beyond.tier, cold_beyond.action),
        (1, 0, 36)
    );
    assert_eq!(read_family12_projection(), (0, 0, 0));
    let cold_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert!(
        [cold_hud.left, cold_hud.right]
            .into_iter()
            .any(|slot| slot.raw_spell_selector == 12 && slot.tier == 0),
        "the public Day HUD exposes the cold tier-0 family-12 holder"
    );
    let post_arrival_castle_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle position requires a Level 2 campaign session",
            ));
        };
        reset
            .economy()
            .castle()
            .map(|castle| castle.position)
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing stage-two Castle"))
    })
    .expect("read-only post-arrival Castle position");
    input.player_x = post_arrival_castle_position.x;
    input.player_y = post_arrival_castle_position.y;
    input.player_z = post_arrival_castle_position.z;
    input.command = valid_command();
    assert_eq!(
        (input.player_x, input.player_y, input.player_z),
        (
            post_arrival_castle_position.x,
            post_arrival_castle_position.y,
            post_arrival_castle_position.z,
        ),
        "the public XP input is pinned to the live Castle position before the player body runs"
    );

    let post_arrival_castle = read_castle().expect("stage-two Castle remains live");
    assert!(
        post_arrival_castle.4 >= 10_240,
        "stage-two Castle current mana reflects the accumulated public defender return"
    );
    let (castle_overlap_snapshot, castle_overlap_events) =
        step_level2_campaign_public(handle, input);
    latest_snapshot = castle_overlap_snapshot;
    beyond_public_events.extend(castle_overlap_events);
    let mut mana_window_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    for _ in 0..source_pass_bound {
        if mana_window_hud.local_player_max_mana >= 2_536
            && mana_window_hud.local_player_mana >= 2_400
        {
            break;
        }
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
        beyond_public_events.extend(events);
        mana_window_hud =
            campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    }
    assert!(mana_window_hud.local_player_max_mana >= 2_536);
    assert!(mana_window_hud.local_player_mana >= 2_400);
    let read_castle_player_resources = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "Beyond Sight resource proof requires a Level 2 campaign session",
                ));
            };
            let player = reset.shared_spells().player_entity();
            Ok((player.max_mana, player.mana, player.mana_regen))
        })
        .expect("read-only Castle-overlap player resource projection")
    };
    let (player_max_mana, player_mana, player_mana_regen) = read_castle_player_resources();
    assert!(player_max_mana >= 2_536);
    assert!(player_mana >= 2_400);
    assert!(
        player_mana_regen >= 1_000,
        "the source-time Castle overlap selected positive upkeep before the post-body projection moved"
    );

    // Tier zero has no Castle-mana requirement. Repeating the ordinary
    // selected cast terminalizes an active model-12 holder to W=1; the
    // next slot-38 callback cleans it before player slot 111 may re-arm.
    // Re-arm succeeds only when the source player-mana gate has recovered
    // to the physical tier-zero cost, so every transition is checked
    // against the pre-pass public resource state.
    let mut beyond = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    assert_eq!((beyond.tier, beyond.experience_awarded), (0, 0));
    let mut fresh_tier0_activations = 0_usize;
    let mut terminal_repeat_passes = 0_usize;
    let mut xp_passes = 0_usize;
    while beyond.experience_awarded < 360 {
        assert!(
            xp_passes < source_pass_bound,
            "ordinary selected action-36 repeats reach 360 awards within the source arena bound"
        );
        let before = beyond;
        let (_, player_mana_before, _) = read_castle_player_resources();
        input.command = McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 0x40,
            requested_spell: 12,
            ..McCommandV2::default()
        };
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        beyond_public_events.extend(events.iter().copied());
        latest_snapshot = snapshot;
        xp_passes += 1;
        beyond = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
        assert_eq!(
            beyond.tier, 0,
            "all pre-upgrade public activations stay tier zero"
        );
        let (player_max_mana, _, player_mana_regen) = read_castle_player_resources();
        assert!(player_max_mana >= 2_536);
        assert!(
            player_mana_regen == -1_800 || player_mana_regen >= 1_000,
            "source action-36 charge/recharge leaves the expected mana-regeneration state"
        );
        let experience_delta = beyond.experience_awarded - before.experience_awarded;
        assert!(
            experience_delta == 0 || experience_delta == 1,
            "one source action-36 pass contributes at most one XP award"
        );
        if beyond.active_value == beyond.activation_value
            && before.active_value != before.activation_value
        {
            assert!(
                player_mana_before >= 1_800,
                "every cold or post-cleanup re-arm satisfies the source player-mana gate"
            );
            fresh_tier0_activations += 1;
        }
        if before.active_value == before.activation_value {
            assert_eq!(
                experience_delta, 1,
                "the first active action-36 pulse awards exactly once"
            );
        } else if before.active_value == 1 {
            assert_eq!(
                experience_delta, 0,
                "active-repeat terminal pass adds no XP"
            );
            if player_mana_before >= 1_800 {
                assert_eq!(
                    beyond.active_value, beyond.activation_value,
                    "terminal cleanup may re-arm in the same outer turn only when the later player callback passes the mana gate"
                );
            } else {
                assert_eq!(
                    beyond.active_value, 0,
                    "terminal cleanup remains cold while the later player callback lacks mana"
                );
            }
            terminal_repeat_passes += 1;
        } else {
            assert_eq!(experience_delta, 0, "later tier-zero pulses add no XP");
        }
        if experience_delta == 1 {
            assert_eq!(
                beyond.experience_awarded,
                i32::try_from(fresh_tier0_activations).expect("activation count fits i32"),
                "each fresh public action-36 arm contributes one and only one XP award"
            );
        }
    }
    assert_eq!(fresh_tier0_activations, 360);
    assert_eq!(beyond.experience_awarded, 360);
    assert_eq!(terminal_repeat_passes, 359);
    assert!(
        xp_passes > 719,
        "Castle recharge makes the public route longer than the ideal alternating-pass lower bound"
    );
    assert!(
        xp_passes < source_pass_bound,
        "public XP route stays within the source arena bound"
    );
    assert_eq!(read_family12_projection(), (0, 360, 1));
    input.command = valid_command();
    let (neutral_snapshot, neutral_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(neutral_events.iter().copied());
    latest_snapshot = neutral_snapshot;
    let neutral_beyond = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    assert_eq!(neutral_beyond.experience_awarded, 360);
    let mut cleanup_passes = 0;
    while beyond.active_value != 0 {
        assert!(
            cleanup_passes < source_pass_bound,
            "tier-zero terminal cleanup stays within the source arena bound"
        );
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        beyond_public_events.extend(events.iter().copied());
        latest_snapshot = snapshot;
        beyond = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
        assert_eq!(beyond.experience_awarded, 360);
        cleanup_passes += 1;
    }
    assert_eq!(beyond.active_value, 0);

    // The ordinary source menu protocol first selects family 12, then
    // writes the now-unlocked raw tier, and finally assigns that tier to a
    // physical hand holder through SetSpell.
    let player_before_menu = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("public player projection exposes the pre-menu source selection");
    input.command = McCommandV2 {
        opcode: 40,
        argument_1: 12,
        argument_2: 1,
        ..valid_command()
    };
    let (_menu_snapshot, menu_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(menu_events);
    let selected_player = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("public player projection exposes the local source selection");
    assert_eq!(selected_player.selected_spell_ui_index, 12);
    input.command = McCommandV2 {
        opcode: 41,
        argument_1: 1,
        ..valid_command()
    };
    let (_tier_menu_snapshot, tier_menu_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(tier_menu_events);
    let selected_player = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("public player projection exposes the raw tier selection");
    assert_eq!(selected_player.selected_spell_ui_index, 12);
    assert_eq!(selected_player.selected_subspell_index, 1);
    assert!(
        player_before_menu.selected_spell_ui_index != 12
            || player_before_menu.selected_subspell_index != 1,
        "the public opcode-40/41 sequence must visibly change the live Rust selection"
    );
    input.command = McCommandV2 {
        opcode: 31,
        argument_1: 12,
        argument_2: 1,
        ..valid_command()
    };
    let (tier_one_snapshot, tier_one_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(tier_one_events);
    latest_snapshot = tier_one_snapshot;
    let tier_one_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    let tier_one_slot = [tier_one_hud.left, tier_one_hud.right]
        .into_iter()
        .find(|slot| slot.raw_spell_selector == 12)
        .expect("public HUD keeps the family-12 hand after opcode 31");
    assert_eq!(tier_one_slot.tier, 1);
    assert_eq!(tier_one_slot.action, 36);
    assert_eq!(tier_one_slot.holder_entity_index, 38);

    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 12,
        ..McCommandV2::default()
    };
    let (tier_one_arm_snapshot, tier_one_arm_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(tier_one_arm_events);
    latest_snapshot = tier_one_arm_snapshot;
    let tier_one_armed = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    assert_eq!(tier_one_armed.tier, 1);
    assert_eq!(tier_one_armed.mana_cost, 2_400);
    input.command = valid_command();
    let mut tier_one_pulse = None;
    for _ in 0..source_pass_bound {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        beyond_public_events.extend(events);
        latest_snapshot = snapshot;
        let state = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
        if state.experience_awarded > 360 {
            tier_one_pulse = Some(state);
            break;
        }
    }
    let tier_one_pulse = tier_one_pulse.expect("ordinary tier-one cast reaches its first pulse");
    assert_eq!(tier_one_pulse.experience_awarded, 361);
    assert_eq!(tier_one_pulse.tier, 1);
    let active_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert_ne!(active_hud.nyphur.visibility_flags & 0x08, 0);

    let tier_one_activation = usize::try_from(tier_one_pulse.activation_value.max(0))
        .expect("tier-one activation value is non-negative")
        .saturating_add(source_pass_bound);
    let mut expired = tier_one_pulse;
    for _ in 0..tier_one_activation {
        if expired.active_value == 0 {
            break;
        }
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        beyond_public_events.extend(events);
        latest_snapshot = snapshot;
        expired = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    }
    assert_eq!(expired.active_value, 0);
    let expired_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert_eq!(expired_hud.nyphur.visibility_flags & 0x08, 0);
    assert_eq!(expired.experience_awarded, 361);
    let (cleanup_snapshot, cleanup_events) = step_level2_campaign_public(handle, input);
    beyond_public_events.extend(cleanup_events);
    latest_snapshot = cleanup_snapshot;
    let cleaned = campaign_beyond_sight_state_public(handle, latest_snapshot.generation);
    assert_eq!(cleaned.active_value, 0);
    assert_eq!(cleaned.cleanup_requested, 0);
    assert_eq!(cleaned.experience_awarded, 361);
    let cleanup_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert_eq!(cleanup_hud.nyphur.visibility_flags & 0x08, 0);
    assert!(
        beyond_public_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 14)),
        "the complete ordinary family-12 lifecycle never allocates the unrelated model-74/action-72 arena family"
    );
    let positive_stream = finish_campaign_step_recording(handle);
    let positive_stream_len = positive_stream.len();
    assert!(
        positive_stream_len > 720,
        "the replay trace contains the complete public economy, XP, tier, expiry, and cleanup stream"
    );
    assert!(
        positive_stream
            .iter()
            .flat_map(|step| &step.events)
            .all(|event| !(event.kind == 8 && event.family == 14)),
        "no step in the complete public stream allocates model-74/action-72"
    );
    let mut final_beyond = cleaned;
    final_beyond.generation = 0;
    let mut final_hud = cleanup_hud;
    final_hud.generation = 0;
    let final_player = campaign_players_public(handle)
        .into_iter()
        .find(|player| player.active != 0 && player.slot == 0)
        .expect("final public player projection remains available");
    let final_family12_projection = read_family12_projection();
    let final_revision = latest_snapshot.revision;
    let changed_hash = campaign_world_hash_public(handle);
    assert_ne!(changed_hash, initial_hash);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(campaign_world_hash_public(handle), initial_hash);
    let reset_beyond = campaign_beyond_sight_state_public(handle, 1);
    assert_eq!(
        reset_beyond,
        McCampaignBeyondSightSnapshotV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCampaignBeyondSightSnapshotV2>(),
            generation: 1,
            ..McCampaignBeyondSightSnapshotV2::default()
        }
    );
    let reset_hud = campaign_day_hud_public(handle, 1, 0);
    assert_eq!(reset_hud.nyphur.visibility_flags, 2);
    assert_eq!(
        campaign_players_public(handle)
            .into_iter()
            .find(|player| player.active != 0 && player.slot == 0)
            .expect("reset public local-player projection is available"),
        initial_player,
        "reset restores the exact public player hand and raw selection projection"
    );
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);

    for (ordinal, recorded) in positive_stream.into_iter().enumerate() {
        let mut replay_input = recorded.input;
        replay_input.generation = 1;
        let (replay_snapshot, replay_events) = step_level2_campaign_public(handle, replay_input);
        assert_eq!(
            normalize_recorded_campaign_step(replay_snapshot, replay_events),
            normalize_recorded_campaign_step(recorded.snapshot, recorded.events),
            "generation-normalized public state/event trace replays at step {ordinal}"
        );
    }
    assert_eq!(campaign_world_hash_public(handle), changed_hash);
    let mut replay_beyond = campaign_beyond_sight_state_public(handle, 1);
    replay_beyond.generation = 0;
    assert_eq!(replay_beyond, final_beyond);
    let mut replay_hud = campaign_day_hud_public(handle, 1, final_revision);
    replay_hud.generation = 0;
    replay_hud.nyphur.generation = 0;
    assert_eq!(replay_hud, final_hud);
    assert_eq!(
        campaign_players_public(handle)
            .into_iter()
            .find(|player| player.active != 0 && player.slot == 0)
            .expect("replayed public player projection remains available"),
        final_player
    );
    assert_eq!(read_family12_projection(), final_family12_projection);
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_session_destroy(isolated_handle),
        MC_STATUS_OK
    );
}

#[test]
fn level2_campaign_public_invisible_casts_after_first_castle_defender_return() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let isolated_handle =
        open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let initial_hash = campaign_world_hash_public(handle);
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);

    let row11_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Invisible proof requires a Level 2 campaign session",
            ));
        };
        reset
            .mission()
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 11)
            .map(|feature| feature.world_position(0))
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing authored row 11"))
    })
    .expect("read Level 2 row-11 objective position");

    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        player_x: row11_position.x,
        player_y: row11_position.y,
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let baseline_sphere_ids = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| sphere.active != 0 && sphere.model == 39)
        .map(|sphere| sphere.sphere_id)
        .collect::<Vec<_>>();
    let (_, group_events) = step_level2_campaign_public(handle, input);
    assert!(group_events.iter().any(|event| {
        event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
            && event.source_slot == 23
            && event.flags != 0
    }));
    let group1_spheres = campaign_mana_spheres_public(handle, 0)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.mana == 512
                && sphere.owner_entity_index == 0
                && !baseline_sphere_ids.contains(&sphere.sphere_id)
        })
        .collect::<Vec<_>>();
    assert!(
        group1_spheres.len() >= 6,
        "ordinary row-11 production exposes six neutral group-1 model-39 spheres"
    );
    let group1_sphere_ids = group1_spheres
        .iter()
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    let target = group1_spheres
        .iter()
        .copied()
        .find(|sphere| sphere.sphere_id == 141)
        .expect("the pinned public six-overlap group-1 cluster is materialized");

    let Level2PublicPossessionClaimRoute {
        mut input,
        mut latest_snapshot,
        player_entity_index,
        ..
    } = drive_level2_public_group1_possession_claims(
        handle,
        input,
        target.sphere_id,
        &group1_sphere_ids,
    );

    // Build the ordinary Castle that owns the Invisible predicate's mana
    // relation. The public route intentionally uses the same selected
    // spell command as the managed bridge.
    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    input.command = valid_command();
    while latest_snapshot.revision < 27 {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        latest_snapshot = snapshot;
    }
    let recovered_hud =
        campaign_day_hud_public(handle, latest_snapshot.generation, latest_snapshot.revision);
    assert!(
        recovered_hud.local_player_mana >= 1_000,
        "ordinary idle turns recover enough Day mana for the Castle request"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    let (castle_armed_snapshot, castle_arm_events) = step_level2_campaign_public(handle, input);
    assert_eq!(
        castle_arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        1,
        "the public Castle request dispatches its live flyer in the same source pass"
    );
    assert!(
        castle_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1))
    );
    input.command = valid_command();
    let (mut castle_snapshot, castle_root_events) = step_level2_campaign_public(handle, input);
    assert!(
        castle_root_events
            .iter()
            .any(|event| event.kind == 8 && event.family == 1),
        "the ordinary Castle route allocates a root"
    );
    assert_eq!(castle_snapshot.generation, castle_armed_snapshot.generation);

    // Objective stage two materialises the authored row-156 acquisition
    // source. Its dynamic arena slot is intentionally read from the public
    // pickup endpoint rather than treated as a stable test fixture.
    let (objective_x, objective_y) = level2_objective_point(handle);
    input.player_x = objective_x;
    input.player_y = objective_y;
    input.player_z = 256;
    input.command = valid_command();
    let _ = step_level2_campaign_public(handle, input);
    let (pickup_snapshot, _) = step_level2_campaign_public(handle, input);
    let invisible_source = campaign_spell_pickups_public(handle, pickup_snapshot.generation)
        .into_iter()
        .find(|source| source.authored_index == 156)
        .expect("ordinary objective route materializes authored Invisible source 156");
    assert_eq!(
        (
            invisible_source.family,
            invisible_source.action,
            invisible_source.active
        ),
        (11, 34, 1),
        "the public pickup preserves family-11's authored acquisition action"
    );
    input.player_x = invisible_source.x;
    input.player_y = invisible_source.y;
    input.player_z = invisible_source.z;
    let mut invisible_acquired = false;
    for _ in 0..4 {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        invisible_acquired |= events.iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 11
                && event.source_slot == invisible_source.source_entity_index
                && event.flags == 1
        });
        if invisible_acquired {
            break;
        }
    }
    assert!(
        invisible_acquired,
        "ordinary C-ABI flight at the public source position acquires Invisible"
    );
    let cold_invisible = campaign_invisible_state_public(handle, castle_snapshot.generation);
    assert_eq!(
        (
            cold_invisible.active,
            cold_invisible.tier,
            cold_invisible.action,
            cold_invisible.active_value,
            cold_invisible.holder_entity_index,
        ),
        (1, 0, 33, 0, invisible_source.source_entity_index),
        "authored acquisition creates only the cold tier-0 Invisible holder"
    );

    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    input.command = valid_command();
    let mut first_defender_deposit = None;
    for _ in 0..1_024 {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        let mut rows = [McCampaignEconomyEventV2::default(); 64];
        let mut count = 0;
        assert_eq!(
            mc_native_campaign_economy_events(
                handle,
                rows.as_mut_ptr(),
                rows.len() as u32,
                struct_size::<McCampaignEconomyEventV2>(),
                &raw mut count,
            ),
            MC_STATUS_OK
        );
        for event in &rows[..count as usize] {
            if event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED {
                first_defender_deposit = Some(event.value_0);
            }
        }
        if first_defender_deposit.is_some() {
            break;
        }
    }
    assert_eq!(
        first_defender_deposit,
        Some(3_072),
        "the single live Castle defender aggregates all six ordinary 512-mana group-1 spheres before returning"
    );
    let retained_group1_spheres = campaign_mana_spheres_public(handle, castle_snapshot.generation)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
                && group1_sphere_ids.contains(&sphere.sphere_id)
        })
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        retained_group1_spheres.len(),
        0,
        "the first 3072-mana Castle return consumes all six claimed group-1 spheres"
    );
    let mana_window_hud =
        campaign_day_hud_public(handle, castle_snapshot.generation, castle_snapshot.revision);
    assert!(
        mana_window_hud.local_player_mana >= 2_000,
        "the ordinary first Castle defender return satisfies Invisible's live mana gate"
    );
    assert!(
        mana_window_hud.local_player_max_mana >= mana_window_hud.local_player_mana,
        "the live HUD retains a coherent Rust-owned current/max mana relation"
    );

    // This is the positive proof: no activation helper is called. The
    // existing public selected-spell command reaches sub_5F660 at player
    // slot 111, then the higher Invisible holder reaches its first
    // sub_6B1C0 pulse in that same ascending source pass.
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 11,
        ..McCommandV2::default()
    };
    let (invisible_armed_snapshot, _) = step_level2_campaign_public(handle, input);
    let invisible_armed =
        campaign_invisible_state_public(handle, invisible_armed_snapshot.generation);
    assert_eq!(
        (
            invisible_armed.active,
            invisible_armed.tier,
            invisible_armed.action,
            invisible_armed.active_value,
            invisible_armed.activation_value,
            invisible_armed.mana_cost,
            invisible_armed.mana_regen_requirement,
            invisible_armed.parent_visibility_mode,
        ),
        (1, 0, 33, 180, 181, 2_000, 2_000, 1),
        "selected spell 11 arms and first-pulses the existing tier-0 Invisible holder without a helper"
    );
    assert!(
        invisible_armed.parent_mana >= invisible_armed.mana_cost,
        "the request has the source-required current mana"
    );
    assert_eq!(invisible_armed.parent_mana_regen, -2_000);

    input.command = valid_command();
    let (invisible_first_snapshot, _) = step_level2_campaign_public(handle, input);
    let invisible_first =
        campaign_invisible_state_public(handle, invisible_first_snapshot.generation);
    assert_eq!(
        invisible_first.active_value,
        invisible_first.activation_value - 2,
        "the following ordinary source pass executes the second Invisible pulse"
    );
    assert_eq!(invisible_first.parent_visibility_mode, 1);
    assert_eq!(
        invisible_first.parent_mana_regen, 0,
        "the nonfresh higher-slot pulse clears the positive upkeep selected earlier in the pass"
    );
    assert_ne!(invisible_first.spell_status_flags & 0x20, 0);
    assert_eq!(invisible_first.experience_awarded, 1);
    let invisible_hud = campaign_day_hud_public(
        handle,
        invisible_first_snapshot.generation,
        invisible_first_snapshot.revision,
    );
    let invisible_slot = [invisible_hud.left, invisible_hud.right]
        .into_iter()
        .find(|slot| {
            slot.holder_entity_index == invisible_source.source_entity_index
                && slot.raw_spell_selector == 11
        })
        .expect("the Rust Day HUD presents the active Invisible holder");
    assert_eq!(
        (
            invisible_slot.tier,
            invisible_slot.action,
            invisible_slot.active_value,
            invisible_slot.activation_value,
            invisible_slot.max_mana,
            invisible_slot.mana_regen,
        ),
        (0, 33, 179, 181, 2_000, 2_000),
        "the HUD projects the source-held Invisible state without C# gameplay authority"
    );

    // The source active-repeat branch must not refresh duration. It writes
    // a one-tick terminal countdown at player slot 111; the higher holder
    // consumes it and performs normal visibility/status cleanup in the
    // same ascending source pass. A following neutral pass proves the
    // cleaned state remains stable.
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 11,
        ..McCommandV2::default()
    };
    let (repeat_snapshot, _) = step_level2_campaign_public(handle, input);
    let repeated = campaign_invisible_state_public(handle, repeat_snapshot.generation);
    assert_eq!(
        repeated.active_value, 0,
        "an active selected-spell repeat takes the same-pass source terminal path instead of refreshing"
    );
    assert_eq!(repeated.experience_awarded, 1);
    assert_eq!(repeated.parent_visibility_mode, 0);
    assert_eq!(repeated.parent_status_flags & 0x20, 0);
    input.command = valid_command();
    let (expired_snapshot, _) = step_level2_campaign_public(handle, input);
    let expired = campaign_invisible_state_public(handle, expired_snapshot.generation);
    assert_eq!(expired.active_value, 0);
    assert_eq!(expired.parent_visibility_mode, 0);
    assert_eq!(expired.parent_status_flags & 0x20, 0);
    assert_ne!(
        expired.spell_status_flags & 0x20,
        0,
        "terminal cleanup preserves the source spell/owner status asymmetry"
    );

    let unchanged = expired;
    let mut stale = expired;
    assert_eq!(
        mc_native_campaign_invisible_state(
            handle,
            expired_snapshot.generation + 1,
            &raw mut stale,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        stale, unchanged,
        "stale generation fails closed without output mutation"
    );
    let mut rejected_input = input;
    rejected_input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 29,
        ..McCommandV2::default()
    };
    let mut rejected_snapshot = McCampaignStepSnapshotV2::default();
    let mut rejected_events = [McCampaignEventV2::default(); 1];
    let mut rejected_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const rejected_input,
            &raw mut rejected_snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            rejected_events.as_mut_ptr(),
            rejected_events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut rejected_count,
        ),
        MC_STATUS_BOUNDS,
        "unknown requested spell fails closed at the public command boundary"
    );
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(campaign_world_hash_public(handle), initial_hash);
    let reset_invisible = campaign_invisible_state_public(handle, 1);
    assert_eq!(reset_invisible.active, 0);
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_session_destroy(isolated_handle),
        MC_STATUS_OK
    );
}

#[test]
fn level2_campaign_public_possession_reset_replays_hash_without_cross_session_leakage() {
    let first = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let second = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let initial_first_hash = campaign_world_hash_public(first);
    let initial_second_hash = campaign_world_hash_public(second);
    assert_eq!(initial_first_hash, initial_second_hash);

    let possession_active = |handle: u64, generation: u64, tick: u64| {
        [
            campaign_day_hud_public(handle, generation, tick).left,
            campaign_day_hud_public(handle, generation, tick).right,
        ]
        .into_iter()
        .find(|slot| slot.raw_spell_selector == 1)
        .expect("Day HUD exposes the Possession holder")
        .active_value
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            flags: 0x41,
            requested_spell: 1,
            ..McCommandV2::default()
        },
        player_slot: 0,
        speed_index: 0,
        player_x: 384,
        player_y: 62_080,
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let (armed, arm_events) = step_level2_campaign_public(first, input);
    let first_sources = arm_events
        .iter()
        .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 39)
        .map(|event| event.source_slot)
        .collect::<Vec<_>>();
    assert_eq!(
        first_sources.len(),
        1,
        "the public request dispatches one same-pass Possession flyer"
    );
    assert_eq!(
        possession_active(first, armed.generation, armed.revision),
        2,
        "the first session alone owns the same-pass advanced Possession state"
    );
    assert_eq!(
        possession_active(second, 0, 0),
        0,
        "the untouched second session cannot inherit Possession cooldown state"
    );
    assert_ne!(campaign_world_hash_public(first), initial_first_hash);
    assert_eq!(campaign_world_hash_public(second), initial_second_hash);

    input.command = valid_command();
    let (first_launch, first_launch_events) = step_level2_campaign_public(first, input);
    assert!(first_launch_events.iter().all(|event| {
        !(event.kind == 8 && event.family == 39) || first_sources.contains(&event.source_slot)
    }));
    let first_hash = first_launch.state_hash;
    assert_eq!(campaign_world_hash_public(first), first_hash);
    assert_eq!(campaign_world_hash_public(second), initial_second_hash);

    assert_eq!(mc_native_campaign_session_reset(first), MC_STATUS_OK);
    assert_eq!(campaign_world_hash_public(first), initial_first_hash);
    assert_eq!(
        possession_active(first, 1, 0),
        0,
        "reset clears the public Possession cooldown before replay"
    );
    assert_eq!(campaign_world_hash_public(second), initial_second_hash);

    input.generation = 1;
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x41,
        requested_spell: 1,
        ..McCommandV2::default()
    };
    let (rearmed, replay_arm_events) = step_level2_campaign_public(first, input);
    assert_eq!(rearmed.generation, 1);
    let replay_sources = replay_arm_events
        .iter()
        .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 39)
        .map(|event| event.source_slot)
        .collect::<Vec<_>>();
    assert_eq!(replay_sources, first_sources);
    input.command = valid_command();
    let (replay_launch, replay_launch_events) = step_level2_campaign_public(first, input);
    let first_advance_sources = first_launch_events
        .iter()
        .filter(|event| event.kind == 8 && event.family == 39)
        .map(|event| event.source_slot)
        .collect::<Vec<_>>();
    let replay_advance_sources = replay_launch_events
        .iter()
        .filter(|event| event.kind == 8 && event.family == 39)
        .map(|event| event.source_slot)
        .collect::<Vec<_>>();
    assert_eq!(replay_advance_sources, first_advance_sources);
    assert_eq!(replay_launch.state_hash, first_hash);
    assert_eq!(campaign_world_hash_public(first), first_hash);
    assert_eq!(campaign_world_hash_public(second), initial_second_hash);

    assert_eq!(mc_native_campaign_session_destroy(first), MC_STATUS_OK);
    assert_eq!(mc_native_campaign_session_destroy(second), MC_STATUS_OK);
}

#[test]
fn level2_campaign_step_projects_family12_source_slot38() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
    let pickup_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let pickup = reset
            .shared_spells()
            .pickups()
            .iter()
            .find(|pickup| pickup.index == 38)
            .expect("family-12 source slot 38 is present");
        assert_eq!(pickup.action, 37);
        Ok(pickup.position)
    })
    .expect("read family-12 source position");
    assert_eq!(pickup_position.x, 384);
    assert_eq!(pickup_position.y, 39040);
    assert_eq!(pickup_position.z, 16);

    let mut initial_terrain = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut initial_terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut initial_hud = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            0,
            initial_terrain.tick,
            &raw mut initial_hud,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_hud.left.raw_spell_selector, 0);
    assert_eq!(initial_hud.right.raw_spell_selector, 1);
    let mut initial_hud_repeat = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            0,
            initial_terrain.tick,
            &raw mut initial_hud_repeat,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_hud_repeat, initial_hud);
    let mut hud_sentinel = McCampaignDayHudSnapshotV2 {
        global_mana_denominator: 0x1357_9BDF,
        ..McCampaignDayHudSnapshotV2::default()
    };
    let unchanged_hud = hud_sentinel;
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            1,
            initial_terrain.tick,
            &raw mut hud_sentinel,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(hud_sentinel, unchanged_hud);
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            0,
            initial_terrain.tick.wrapping_add(1),
            &raw mut hud_sentinel,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(hud_sentinel, unchanged_hud);

    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        player_x: pickup_position.x,
        player_y: pickup_position.y,
        player_z: pickup_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
    let mut event_count;
    let mut acquired = None;
    // The source cadence is part of the reset-visible row. Walk exactly
    // the four ordinary public turns needed to reach its phase gate
    // instead of mutating private source bytes through the C ABI.
    for _ in 0..4 {
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        acquired = events[..event_count as usize]
            .iter()
            .find(|event| {
                event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                    && event.family == 12
                    && event.source_slot == 38
                    && event.flags == 1
            })
            .copied()
            .or(acquired);
    }
    let acquired = acquired.expect("C ABI exposes family-12 slot-38 acquisition");
    assert_eq!(acquired.pass, 0);
    assert_eq!(acquired.generation, 0);
    assert_eq!(acquired.abi_version, MC_NATIVE_ABI_VERSION);
    let acquired_snapshot = snapshot;

    let mut terrain = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut hud = McCampaignDayHudSnapshotV2::default();
    let hud_status = mc_native_campaign_day_hud_state(
        handle,
        0,
        terrain.tick,
        &raw mut hud,
        struct_size::<McCampaignDayHudSnapshotV2>(),
    );
    assert_eq!(
        hud_status,
        MC_STATUS_OK,
        "Day HUD read failed: {}",
        LAST_ERROR.with(|error| error.borrow().clone())
    );
    assert_eq!(hud.left.holder_entity_index, 38);
    assert_eq!(hud.left.parent_entity_index, 111);
    assert_eq!(hud.left.raw_spell_selector, 12);
    assert_eq!(hud.left.frame_selector, 1);
    assert_eq!(hud.left.icon_selector, 135);
    assert_eq!(hud.left.level_text_selector, 0);
    assert_eq!(hud.left.tier, 0);
    assert_eq!(hud.left.draw_admitted, 1);
    assert_eq!(hud.right.raw_spell_selector, 1);
    assert_eq!(
        (
            hud.life_palette_index,
            hud.mana_palette_index,
            hud.mana_max_palette_index,
        ),
        (0x7B, 0x60, 0x64)
    );
    assert_eq!(hud.player_panel_selector, 41);
    assert_eq!(hud.hit_frame_time_pre_draw, 0);
    let mut hud_repeat = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            0,
            terrain.tick,
            &raw mut hud_repeat,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(hud_repeat, hud);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut reset_terrain = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            1,
            &raw mut reset_terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut reset_hud = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            1,
            reset_terrain.tick,
            &raw mut reset_hud,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_hud.left.raw_spell_selector, 0);
    assert_eq!(reset_hud.right.raw_spell_selector, 1);
    assert_eq!(reset_hud.generation, 1);
    assert_eq!(reset_hud.player_panel_selector, 41);

    input.generation = 1;
    let mut replay = None;
    for _ in 0..4 {
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        replay = events[..event_count as usize]
            .iter()
            .find(|event| {
                event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                    && event.family == 12
                    && event.source_slot == 38
                    && event.flags == 1
            })
            .copied()
            .or(replay);
    }
    let replay = replay.expect("reset replay exposes family-12 slot-38 acquisition");
    assert_eq!(replay.generation, 1);
    assert_eq!(replay.pass, acquired.pass);
    assert_eq!(replay.kind, acquired.kind);
    assert_eq!(replay.family, acquired.family);
    assert_eq!(replay.flags, acquired.flags);
    assert_eq!(replay.source_slot, acquired.source_slot);
    assert_eq!(snapshot.state_hash, acquired_snapshot.state_hash);
    assert_eq!(snapshot.terrain_hash, acquired_snapshot.terrain_hash);
    assert_eq!(snapshot.dispatch_hash, acquired_snapshot.dispatch_hash);
    assert_eq!(snapshot.event_count, acquired_snapshot.event_count);
    assert_eq!(snapshot.event_kind_mask, acquired_snapshot.event_kind_mask);
    let mut replay_terrain = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            1,
            &raw mut replay_terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut replay_hud = McCampaignDayHudSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_day_hud_state(
            handle,
            1,
            replay_terrain.tick,
            &raw mut replay_hud,
            struct_size::<McCampaignDayHudSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_hud.left, hud.left);
    assert_eq!(replay_hud.right, hud.right);
    assert_eq!(replay_hud.mana_palette_index, hud.mana_palette_index);
    assert_eq!(
        replay_hud.mana_max_palette_index,
        hud.mana_max_palette_index
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_minimap_is_source_ordered_visibility_bound_and_replay_safe() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
    let selection = McCampaignSelectionV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSelectionV2>(),
        external_level: 2,
        internal_level: 1,
        map_type: 0,
        ..McCampaignSelectionV2::default()
    };
    let mut handle = 0;
    let open_status = mc_native_campaign_session_open(
        package.as_ptr(),
        package.len() as u32,
        std::ptr::null(),
        0,
        &raw const selection,
        &raw mut handle,
    );
    assert_eq!(
        open_status,
        MC_STATUS_OK,
        "minimap fixture open failed: {}",
        LAST_ERROR.with(|error| error.borrow().clone())
    );

    let mut terrain = McCampaignTerrainStateV2::default();
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut initial_state = McCampaignMinimapStateV2::default();
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            0,
            terrain.tick,
            &raw mut initial_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_state.visibility_flags, 2);
    assert!(initial_state.entity_count > 0);
    assert_eq!(initial_state.mark_count, 1);

    let (local_player_slot, local_objective, live_player) =
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let local_player_slot = reset.runtime().local_player_slot;
            let local_objective = reset.runtime().objectives[usize::from(local_player_slot)];
            Ok((local_player_slot, local_objective, reset.state))
        })
        .expect("read live Level 2 minimap local player state");
    let mut selected_stage = McCampaignObjectiveStageSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_objective_stage_state(
            handle,
            0,
            local_player_slot,
            local_objective.text_selector,
            &raw mut selected_stage,
            struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(selected_stage.stage_kind, 5);
    let project_live_mark = |player: &FlightState| {
        sim_core::level1_mission::level1_minimap_project(
            player,
            Position {
                x: selected_stage.axis_x as u16,
                y: selected_stage.axis_y as u16,
                z: 0,
            },
            128,
            256,
        )
    };
    let initial_expected_mark_xy = project_live_mark(&live_player);

    let mut initial_entities =
        vec![McCampaignMinimapEntityV2::default(); initial_state.entity_count as usize];
    let mut initial_entity_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_entities_copy(
            handle,
            0,
            terrain.tick,
            0,
            initial_entities.as_mut_ptr(),
            initial_entities.len() as u32,
            struct_size::<McCampaignMinimapEntityV2>(),
            &raw mut initial_entity_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_entity_count, initial_state.entity_count);
    assert!(initial_entities.windows(2).all(|rows| {
        rows[0].source_order < rows[1].source_order
            && rows[0].generation == 0
            && rows[0].tick == terrain.tick
    }));
    let initial_palette = |subject_entity_index| {
        initial_entities
            .iter()
            .find(|row| row.subject_entity_index == subject_entity_index)
            .unwrap_or_else(|| {
                panic!("expected live initial minimap subject {subject_entity_index}")
            })
            .palette_index
    };
    assert_eq!(initial_palette(1), 209);
    assert_eq!(initial_palette(39), 12);
    assert_eq!(initial_palette(77), 90);
    assert_eq!(initial_palette(108), 0x1C);
    assert_eq!(initial_palette(38), 123);
    let initial_help = |subject_entity_index| {
        initial_entities
            .iter()
            .find(|row| row.subject_entity_index == subject_entity_index)
            .unwrap_or_else(|| {
                panic!("expected live initial model-45 subject {subject_entity_index}")
            })
            .help_selector
    };
    assert_eq!(initial_help(1), 21);
    assert_eq!(initial_help(2), 0);
    assert!(
        !initial_entities
            .iter()
            .any(|row| row.subject_entity_index == 112),
        "Invisible Nyphur does not reach the minimap entity stream"
    );

    let mut initial_marks =
        vec![McCampaignMinimapMarkV2::default(); initial_state.mark_count as usize];
    let mut initial_mark_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_marks_copy(
            handle,
            0,
            terrain.tick,
            0,
            initial_marks.as_mut_ptr(),
            initial_marks.len() as u32,
            struct_size::<McCampaignMinimapMarkV2>(),
            &raw mut initial_mark_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(initial_mark_count, initial_state.mark_count);
    assert_eq!(initial_marks[0].subject_entity_index, 0);
    assert_eq!(initial_marks[0].kind, 2);
    assert_eq!(
        initial_marks[0].source_order,
        u16::from(local_objective.text_selector)
    );
    assert_eq!(
        (initial_marks[0].x, initial_marks[0].y),
        initial_expected_mark_xy
    );
    assert_eq!(initial_marks[0].palette_index, 188);
    assert_eq!(initial_marks[0].help_selector, 0x51);
    assert_eq!(initial_marks[0].visible, 1);
    assert!(
        !initial_marks
            .iter()
            .any(|row| row.subject_entity_index == 112)
    );
    assert!(initial_marks.windows(2).all(|rows| {
        rows[0].source_order < rows[1].source_order
            && rows[0].generation == 0
            && rows[0].tick == terrain.tick
    }));

    let mut stale_state = McCampaignMinimapStateV2 {
        abi_version: 0xA1B2_C3D4,
        generation: 0x1122_3344_5566_7788,
        tick: 0x8877_6655_4433_2211,
        entity_count: 0x1357_9BDF,
        mark_count: 0x2468_ACE0,
        entity_hash: 0x0123_4567_89AB_CDEF,
        mark_hash: 0xFEDC_BA98_7654_3210,
        ..McCampaignMinimapStateV2::default()
    };
    let unchanged_state = stale_state;
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            1,
            terrain.tick,
            &raw mut stale_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(stale_state, unchanged_state);
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            0,
            terrain.tick.wrapping_add(1),
            &raw mut stale_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(stale_state, unchanged_state);

    let mut stale_entity = McCampaignMinimapEntityV2 {
        subject_entity_index: 0xBEEF,
        source_order: 0x1234,
        ..McCampaignMinimapEntityV2::default()
    };
    let unchanged_entity = stale_entity;
    let mut stale_written = 99;
    assert_eq!(
        mc_native_campaign_minimap_entities_copy(
            handle,
            1,
            terrain.tick,
            0,
            &raw mut stale_entity,
            1,
            struct_size::<McCampaignMinimapEntityV2>(),
            &raw mut stale_written,
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(stale_entity, unchanged_entity);
    assert_eq!(
        mc_native_campaign_minimap_entities_copy(
            handle,
            0,
            terrain.tick,
            u32::MAX,
            &raw mut stale_entity,
            1,
            struct_size::<McCampaignMinimapEntityV2>(),
            &raw mut stale_written,
        ),
        MC_STATUS_BOUNDS
    );
    assert_eq!(stale_entity, unchanged_entity);

    let command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command,
        player_slot: 0,
        speed_index: 0,
        player_x: 384,
        player_y: 62_080,
        player_z: 256,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
    let mut event_count = 0;
    for _ in 0..282 {
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        input.player_x = snapshot.x;
        input.player_y = snapshot.y;
        input.player_z = snapshot.z;
    }
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            0,
            &raw mut terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut acquired_state = McCampaignMinimapStateV2::default();
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            0,
            terrain.tick,
            &raw mut acquired_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(acquired_state.visibility_flags, 6);
    assert_eq!(acquired_state.mark_count, 1);
    let mut acquired_entities =
        vec![McCampaignMinimapEntityV2::default(); acquired_state.entity_count as usize];
    let mut acquired_entity_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_entities_copy(
            handle,
            0,
            terrain.tick,
            0,
            acquired_entities.as_mut_ptr(),
            acquired_entities.len() as u32,
            struct_size::<McCampaignMinimapEntityV2>(),
            &raw mut acquired_entity_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(acquired_entity_count, acquired_state.entity_count);
    let acquired_live_player = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.state)
    })
    .expect("read live acquired Level 2 minimap player state");
    let acquired_expected_mark_xy = project_live_mark(&acquired_live_player);
    assert!(
        !acquired_entities
            .iter()
            .any(|row| row.subject_entity_index == 112)
    );
    let acquired_subject_108 = acquired_entities
        .iter()
        .find(|row| row.subject_entity_index == 108)
        .expect("live acquired class-2/model-2 subject 108 remains admitted");
    assert_eq!(acquired_subject_108.palette_index, 98);
    assert!(acquired_entities.windows(2).all(|rows| {
        rows[0].source_order < rows[1].source_order
            && rows[0].generation == 0
            && rows[0].tick == terrain.tick
    }));
    assert_eq!(
        acquired_state.entity_hash,
        level2_minimap_entity_hash(
            &acquired_entities
                .iter()
                .map(|row| Level2MinimapEntity {
                    source_order: row.source_order,
                    subject_entity_index: row.subject_entity_index,
                    x: row.x,
                    y: row.y,
                    selector: row.selector,
                    palette_index: row.palette_index,
                    kind: row.kind,
                    size: row.size,
                    anchor: row.anchor,
                    help_selector: row.help_selector,
                    visible: row.visible,
                })
                .collect::<Vec<_>>()
        )
    );

    let mut acquired_marks =
        vec![McCampaignMinimapMarkV2::default(); acquired_state.mark_count as usize];
    let mut acquired_mark_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_marks_copy(
            handle,
            0,
            terrain.tick,
            0,
            acquired_marks.as_mut_ptr(),
            acquired_marks.len() as u32,
            struct_size::<McCampaignMinimapMarkV2>(),
            &raw mut acquired_mark_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(acquired_mark_count, acquired_state.mark_count);
    assert_eq!(acquired_marks[0].subject_entity_index, 0);
    assert_eq!(acquired_marks[0].kind, 2);
    assert_eq!(
        acquired_marks[0].source_order,
        u16::from(local_objective.text_selector)
    );
    assert_eq!(
        (acquired_marks[0].x, acquired_marks[0].y),
        acquired_expected_mark_xy
    );
    assert_eq!(acquired_marks[0].palette_index, 188);
    assert_eq!(acquired_marks[0].help_selector, 0x51);
    assert_eq!(acquired_marks[0].visible, 0);
    assert!(
        !acquired_marks
            .iter()
            .any(|row| row.subject_entity_index == 112)
    );
    assert!(acquired_marks.windows(2).all(|rows| {
        rows[0].source_order < rows[1].source_order
            && rows[0].generation == 0
            && rows[0].tick == terrain.tick
    }));
    assert_eq!(
        acquired_state.mark_hash,
        level2_minimap_mark_hash(
            &acquired_marks
                .iter()
                .map(|row| Level2MinimapMark {
                    source_order: row.source_order,
                    subject_entity_index: row.subject_entity_index,
                    x: row.x,
                    y: row.y,
                    palette_index: row.palette_index,
                    kind: row.kind,
                    help_selector: row.help_selector,
                    visible: row.visible,
                })
                .collect::<Vec<_>>()
        )
    );

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    let mut reset_state = McCampaignMinimapStateV2::default();
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            1,
            0,
            &raw mut reset_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_state.visibility_flags, initial_state.visibility_flags);
    assert_eq!(reset_state.mark_count, initial_state.mark_count);
    assert_eq!(reset_state.entity_hash, initial_state.entity_hash);
    assert_eq!(reset_state.mark_hash, initial_state.mark_hash);
    let mut reset_marks = vec![McCampaignMinimapMarkV2::default(); reset_state.mark_count as usize];
    let mut reset_mark_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_marks_copy(
            handle,
            1,
            0,
            0,
            reset_marks.as_mut_ptr(),
            reset_marks.len() as u32,
            struct_size::<McCampaignMinimapMarkV2>(),
            &raw mut reset_mark_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(reset_mark_count, reset_state.mark_count);
    assert_eq!(reset_marks[0].visible, 1);
    input.generation = 1;
    input.player_x = 384;
    input.player_y = 62_080;
    input.player_z = 256;
    for _ in 0..282 {
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        input.player_x = snapshot.x;
        input.player_y = snapshot.y;
        input.player_z = snapshot.z;
    }
    assert_eq!(
        mc_native_campaign_terrain_state(
            handle,
            1,
            &raw mut terrain,
            struct_size::<McCampaignTerrainStateV2>(),
        ),
        MC_STATUS_OK
    );
    let mut replay_state = McCampaignMinimapStateV2::default();
    assert_eq!(
        mc_native_campaign_minimap_state(
            handle,
            1,
            terrain.tick,
            &raw mut replay_state,
            struct_size::<McCampaignMinimapStateV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_state.entity_hash, acquired_state.entity_hash);
    assert_eq!(replay_state.mark_hash, acquired_state.mark_hash);
    assert_eq!(
        replay_state.visibility_flags,
        acquired_state.visibility_flags
    );
    let mut replay_marks =
        vec![McCampaignMinimapMarkV2::default(); replay_state.mark_count as usize];
    let mut replay_mark_count = 0;
    assert_eq!(
        mc_native_campaign_minimap_marks_copy(
            handle,
            1,
            terrain.tick,
            0,
            replay_marks.as_mut_ptr(),
            replay_marks.len() as u32,
            struct_size::<McCampaignMinimapMarkV2>(),
            &raw mut replay_mark_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(replay_mark_count, replay_state.mark_count);
    assert_eq!(replay_marks[0].visible, 0);
    assert_eq!(
        replay_marks[0].palette_index,
        acquired_marks[0].palette_index
    );
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_invisible_state_and_activation_guard_are_cabi_reachable() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
    let initial_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.state.position)
    })
    .expect("read initial Level 2 position");
    let (objective_x, objective_y) = level2_objective_point(handle);
    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        player_x: objective_x,
        player_y: objective_y,
        player_z: initial_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
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
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    assert!(event_count > 0, "initial Level 2 source pass emits events");
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    let source = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == 156)
            .copied()
            .expect("authored family-11 source"))
    })
    .expect("read authored family-11 source");
    input.player_x = source.position.x;
    input.player_y = source.position.y;
    input.player_z = source.position.z;
    let mut acquired = false;
    for _ in 0..4 {
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        acquired |= events[..event_count as usize].iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 11
                && event.source_slot == source.source_entity_index
                && event.flags == 1
        });
        if acquired {
            break;
        }
    }
    assert!(acquired, "C ABI exposes family-11 acquisition");
    let mut invisible = McCampaignInvisibleSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_invisible_state(
            handle,
            0,
            &raw mut invisible,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(invisible.active, 1);
    assert_eq!(invisible.action, 33);
    assert_eq!(invisible.active_value, 0);
    let unchanged = invisible;
    assert_eq!(
        mc_native_campaign_invisible_state(
            handle,
            1,
            &raw mut invisible,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(invisible, unchanged);
    assert_eq!(
        mc_native_campaign_invisible_activate(
            handle,
            0,
            256,
            0,
            &raw mut invisible,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(invisible, unchanged);
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_invisible_state(
            handle,
            1,
            &raw mut invisible,
            struct_size::<McCampaignInvisibleSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(invisible.generation, 1);
    assert_eq!(invisible.active, 0);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_beyond_sight_state_and_activation_guard_are_cabi_reachable() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
    let initial_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.state.position)
    })
    .expect("read initial Level 2 position");
    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        player_x: initial_position.x,
        player_y: initial_position.y,
        player_z: initial_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
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
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    let source_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset
            .shared_spells()
            .pickups()
            .iter()
            .find(|source| source.index == 38)
            .expect("authored family-12 source")
            .position)
    })
    .expect("read authored family-12 source");
    input.player_x = source_position.x;
    input.player_y = source_position.y;
    input.player_z = source_position.z;
    let mut acquired = false;
    for _ in 0..4 {
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        acquired |= events[..event_count as usize].iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 12
                && event.source_slot == 38
                && event.flags == 1
        });
        if acquired {
            break;
        }
    }
    assert!(acquired, "C ABI exposes family-12 acquisition");
    let mut beyond_sight = McCampaignBeyondSightSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_beyond_sight_state(
            handle,
            0,
            &raw mut beyond_sight,
            struct_size::<McCampaignBeyondSightSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(beyond_sight.active, 1);
    assert_eq!(beyond_sight.action, 36);
    assert_eq!(beyond_sight.active_value, 0);
    assert_eq!(beyond_sight.experience_awarded, 0);
    let unchanged = beyond_sight;
    assert_eq!(
        mc_native_campaign_beyond_sight_state(
            handle,
            1,
            &raw mut beyond_sight,
            struct_size::<McCampaignBeyondSightSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(beyond_sight, unchanged);
    assert_eq!(
        mc_native_campaign_beyond_sight_activate(
            handle,
            0,
            256,
            0,
            &raw mut beyond_sight,
            struct_size::<McCampaignBeyondSightSnapshotV2>(),
        ),
        MC_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(beyond_sight, unchanged);
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_beyond_sight_state(
            handle,
            1,
            &raw mut beyond_sight,
            struct_size::<McCampaignBeyondSightSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(beyond_sight.generation, 1);
    assert_eq!(beyond_sight.active, 0);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_opponent_state_is_cabi_reachable_and_generation_bound() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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

    let mut opponent = McCampaignOpponentSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_opponent_state(
            handle,
            0,
            &raw mut opponent,
            struct_size::<McCampaignOpponentSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(opponent.active, 1);
    assert_eq!(opponent.player_slot, 1);
    assert_eq!(opponent.is_ai, 1);
    assert_eq!(opponent.color_index, 1);
    assert_eq!(opponent.entity_index, 112);
    assert_eq!(opponent.entity_class, 3);
    assert_eq!(opponent.entity_model, 1);
    assert_eq!(opponent.entity_action, 1);
    assert_eq!(opponent.fireball_holder_index, 113);
    assert_eq!(opponent.fireball_action, 0);
    assert_eq!(opponent.possession_holder_index, 114);
    assert_eq!(opponent.possession_action, 3);
    assert_eq!(opponent.objective_relation_domain, 2);
    assert_eq!(opponent.objective_relation_value, 112);
    assert_eq!(opponent.visibility_flags, 2);

    let unchanged = opponent;
    assert_eq!(
        mc_native_campaign_opponent_state(
            handle,
            1,
            &raw mut opponent,
            struct_size::<McCampaignOpponentSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(opponent, unchanged);
    assert_eq!(
        mc_native_campaign_opponent_state(
            handle,
            0,
            &raw mut opponent,
            struct_size::<McCampaignOpponentSnapshotV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(opponent, unchanged);

    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    assert_eq!(
        mc_native_campaign_opponent_state(
            handle,
            1,
            &raw mut opponent,
            struct_size::<McCampaignOpponentSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(opponent.generation, 1);
    assert_eq!(opponent.active, 1);
    assert_eq!(opponent.entity_index, 112);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_shield_state_activation_and_damage_are_cabi_reachable() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
    let initial_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.state.position)
    })
    .expect("read Level 2 player position");
    let (objective_x, objective_y) = level2_objective_point(handle);
    let input_command = valid_command();
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: input_command,
        player_slot: 0,
        player_x: objective_x,
        player_y: objective_y,
        player_z: initial_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
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
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    let pickup_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == 155)
            .expect("authored family-6 source is materialized")
            .position)
    })
    .expect("read authored family-6 source position");
    input.player_x = pickup_position.x;
    input.player_y = pickup_position.y;
    input.player_z = pickup_position.z;
    let mut acquired = false;
    for _ in 0..4 {
        event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                handle,
                &raw const input,
                &raw mut snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                events.as_mut_ptr(),
                events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut event_count,
            ),
            MC_STATUS_OK
        );
        acquired |= events[..event_count as usize].iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 6
                && event.flags == 1
        });
        if acquired {
            break;
        }
    }
    assert!(acquired, "C ABI exposes family-6 acquisition");
    let mut shield = McCampaignShieldSnapshotV2::default();
    assert_eq!(
        mc_native_campaign_shield_state(
            handle,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(shield.active, 1);
    assert_eq!(shield.action, 18);
    let unchanged_shield = shield;
    assert_eq!(
        mc_native_campaign_shield_state(
            handle,
            1,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(shield, unchanged_shield);
    assert_eq!(
        mc_native_campaign_shield_state(
            handle,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>() - 1,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(shield, unchanged_shield);
    assert_eq!(
        mc_native_campaign_shield_activate(
            handle,
            0,
            256,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(shield.active_value, shield.activation_value);
    assert_eq!(
        mc_native_campaign_shield_activate(
            handle,
            1,
            256,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_GENERATION_MISMATCH
    );
    assert_eq!(
        mc_native_campaign_shield_damage_submit(handle, 1, shield.parent_entity_index, 100, 152),
        MC_STATUS_GENERATION_MISMATCH
    );
    event_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_campaign_shield_state(
            handle,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_ne!(
        shield.parent_status_flags & sim_core::spell_activation::SHIELD_MANA_STATUS_BIT,
        0
    );
    let life_before = shield.parent_life;
    assert_eq!(
        mc_native_campaign_shield_damage_submit(handle, 0, shield.parent_entity_index, 100, 152,),
        MC_STATUS_OK
    );
    event_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            events.as_mut_ptr(),
            events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut event_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_campaign_shield_state(
            handle,
            0,
            &raw mut shield,
            struct_size::<McCampaignShieldSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(shield.parent_life, life_before - 25);
    assert_eq!(shield.experience_awarded, 1);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_step_projects_persistent_replacement_success_and_no_free_slot_failure() {
    let open_level2 = || {
        let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/l2-debug3.fJ3PaM/level2");
        let package = level2_package.to_string_lossy();
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
        handle
    };
    let command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };

    let success_handle = open_level2();
    let (success_source_slot, success_position, success_player_slot) =
        with_campaign_session_mut(success_handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let player_entity_index = reset.state.entity_index;
            let position = reset
                .entities()
                .get(usize::from(player_entity_index))
                .expect("qualified Level 2 player entity")
                .position;
            let replacement = reset
                .allocate_level2_spell_replacement(11, player_entity_index, position)
                .expect("qualified persistent replacement allocation");
            Ok((
                replacement.source_entity_index,
                replacement.position,
                reset.runtime().local_player_slot,
            ))
        })
        .expect("install success-vector replacement");
    let success_count_before = with_campaign_session(success_handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.economy().spell_replacements().len())
    })
    .expect("read success-vector baseline");
    let mut success_input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command,
        player_slot: success_player_slot,
        player_x: success_position.x,
        player_y: success_position.y,
        player_z: success_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut success_snapshot = McCampaignStepSnapshotV2::default();
    let mut success_events = [McCampaignEventV2::default(); 4096];
    let mut success_acquired = false;
    for _ in 0..4 {
        let mut success_event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                success_handle,
                &raw const success_input,
                &raw mut success_snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                success_events.as_mut_ptr(),
                success_events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut success_event_count,
            ),
            MC_STATUS_OK
        );
        success_acquired |= success_events[..success_event_count as usize]
            .iter()
            .any(|event| {
                event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                    && event.family == 11
                    && event.source_slot == success_source_slot
                    && event.flags == 1
            });
        if success_acquired {
            break;
        }
    }
    assert!(success_acquired);
    let success_count_after = with_campaign_session(success_handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.economy().spell_replacements().len())
    })
    .expect("read success-vector result");
    assert_eq!(success_count_after, success_count_before + 1);
    assert_eq!(
        mc_native_campaign_session_destroy(success_handle),
        MC_STATUS_OK
    );

    let failure_handle = open_level2();
    let (failure_source_slot, failure_position, failure_player_slot, allocated_count) =
        with_campaign_session_mut(failure_handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let player_entity_index = reset.state.entity_index;
            let position = reset
                .entities()
                .get(usize::from(player_entity_index))
                .expect("qualified Level 2 player entity")
                .position;
            let first = reset
                .allocate_level2_spell_replacement(11, player_entity_index, position)
                .expect("qualified failure-vector seed replacement");
            let mut allocated = 1_usize;
            while reset.economy().free_slot_count() != 0 {
                reset
                    .allocate_level2_spell_replacement(11, player_entity_index, Position::default())
                    .expect("shared arena should materialize every qualified slot");
                allocated += 1;
            }
            Ok((
                first.source_entity_index,
                first.position,
                reset.runtime().local_player_slot,
                allocated,
            ))
        })
        .expect("exhaust shared arena for failure vector");
    success_input.generation = 0;
    success_input.player_slot = failure_player_slot;
    success_input.player_x = failure_position.x;
    success_input.player_y = failure_position.y;
    success_input.player_z = failure_position.z;
    let mut failure_snapshot = McCampaignStepSnapshotV2::default();
    let mut failure_events = [McCampaignEventV2::default(); 4096];
    let mut failure_acquired = false;
    for _ in 0..4 {
        let mut failure_event_count = 0;
        assert_eq!(
            mc_native_campaign_session_step(
                failure_handle,
                &raw const success_input,
                &raw mut failure_snapshot,
                struct_size::<McCampaignStepSnapshotV2>(),
                failure_events.as_mut_ptr(),
                failure_events.len() as u32,
                struct_size::<McCampaignEventV2>(),
                &raw mut failure_event_count,
            ),
            MC_STATUS_OK
        );
        failure_acquired |= failure_events[..failure_event_count as usize]
            .iter()
            .any(|event| {
                event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                    && event.family == 11
                    && event.source_slot == failure_source_slot
                    && event.flags == 1
            });
        if failure_acquired {
            break;
        }
    }
    assert!(failure_acquired);
    with_campaign_session(failure_handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        assert_eq!(reset.economy().free_slot_count(), 0);
        assert_eq!(reset.economy().spell_replacements().len(), allocated_count);
        assert_eq!(
            reset
                .economy()
                .spell_replacements()
                .iter()
                .find(|replacement| replacement.source_entity_index == failure_source_slot)
                .expect("failure-vector source remains live")
                .action,
            33
        );
        Ok(())
    })
    .expect("read no-free replacement result");
    assert_eq!(
        mc_native_campaign_session_destroy(failure_handle),
        MC_STATUS_OK
    );
}

#[test]
fn level2_campaign_economy_sidecar_copies_typed_payload_without_touching_legacy_rows() {
    let level2_package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/l2-debug3.fJ3PaM/level2");
    let package = level2_package.to_string_lossy();
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
    let (sphere_id, carrier_slot) = with_campaign_session_mut(handle, |session| {
        let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let sphere_id = reset
            .economy_mut()
            .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 111, 222)
            .expect("test sphere allocation");
        let carrier_slot = reset
            .economy_mut()
            .spawn_mana_carrier(Position::default(), 222, sphere_id)
            .expect("test carrier allocation");
        Ok((sphere_id, carrier_slot))
    })
    .expect("install source-derived carrier target before outer turn");

    let input_command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 1,
        requested_spell: MC_REQUESTED_SPELL_NONE,
        ..McCommandV2::default()
    };
    let input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        command: input_command,
        player_x: 123,
        player_y: 456,
        player_z: 7,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut generic_events = [McCampaignEventV2::default(); 1024];
    let mut generic_count = 0;
    assert_eq!(
        mc_native_campaign_session_step(
            handle,
            &raw const input,
            &raw mut snapshot,
            struct_size::<McCampaignStepSnapshotV2>(),
            generic_events.as_mut_ptr(),
            generic_events.len() as u32,
            struct_size::<McCampaignEventV2>(),
            &raw mut generic_count,
        ),
        MC_STATUS_OK
    );
    let mut sidecars = [McCampaignEconomyEventV2::default(); 1024];
    let mut sidecar_count = 0;
    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            sidecars.as_mut_ptr(),
            sidecars.len() as u32,
            struct_size::<McCampaignEconomyEventV2>(),
            &raw mut sidecar_count,
        ),
        MC_STATUS_OK
    );
    let row = sidecars[..sidecar_count as usize]
        .iter()
        .find(|row| {
            row.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED && row.primary_id == sphere_id
        })
        .copied()
        .expect("source carrier collection is published through sidecar");
    assert_eq!(row.generation, snapshot.generation);
    assert_eq!(row.family, 1);
    assert_eq!(row.source_slot, carrier_slot);
    assert_eq!(row.secondary_id, u32::from(carrier_slot));
    assert_eq!(row.entity_0, 0);
    assert_eq!(row.value_0, 111);
    let generic = generic_events
        .get(row.ordinal as usize)
        .expect("sidecar ordinal is in generic event stream");
    assert_eq!(generic.kind, MC_CAMPAIGN_EVENT_ECONOMY);
    assert_eq!(generic.source_slot, row.source_slot);

    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            sidecars.as_mut_ptr(),
            sidecars.len() as u32,
            struct_size::<McCampaignEconomyEventV2>() - 1,
            &raw mut sidecar_count,
        ),
        MC_STATUS_STRUCT_TOO_SMALL
    );
    assert_eq!(mc_native_campaign_session_reset(handle), MC_STATUS_OK);
    sidecar_count = 99;
    assert_eq!(
        mc_native_campaign_economy_events(
            handle,
            sidecars.as_mut_ptr(),
            sidecars.len() as u32,
            struct_size::<McCampaignEconomyEventV2>(),
            &raw mut sidecar_count,
        ),
        MC_STATUS_OK
    );
    assert_eq!(sidecar_count, 0, "reset clears the retained sidecar stream");
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}
