fn sphere_lineage_descendants(
    source_id: u32,
    merge_lineage: &BTreeSet<(u32, u32)>,
) -> BTreeSet<u32> {
    let mut descendants = BTreeSet::from([source_id]);
    loop {
        let previous_len = descendants.len();
        for (merged_source, surviving_target) in merge_lineage {
            if descendants.contains(merged_source) {
                descendants.insert(*surviving_target);
            }
        }
        if descendants.len() == previous_len {
            return descendants;
        }
    }
}

fn sphere_has_source_ancestor(
    sphere_id: u32,
    source_ids: &BTreeSet<u32>,
    merge_lineage: &BTreeSet<(u32, u32)>,
) -> bool {
    source_ids
        .iter()
        .any(|source_id| sphere_lineage_descendants(*source_id, merge_lineage).contains(&sphere_id))
}

fn campaign_economy_events_with_mana_lineage(
    handle: u64,
    merge_lineage: &mut BTreeSet<(u32, u32)>,
    castle_collected_sphere_ids: &mut BTreeSet<u32>,
) -> Vec<McCampaignEconomyEventV2> {
    let events = campaign_economy_events_public(handle);
    for event in &events {
        if event.code == MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED {
            merge_lineage.insert((event.secondary_id, event.primary_id));
        } else if event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE {
            castle_collected_sphere_ids.insert(event.secondary_id);
        }
    }
    events
}

fn retain_castle_defender_receipts(
    events: &[McCampaignEconomyEventV2],
    retained: &mut Vec<McCampaignEconomyEventV2>,
) {
    retained.extend(events.iter().copied().filter(|event| {
        matches!(
            event.code,
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE
                | MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED
        )
    }));
}

fn assert_no_level2_dispatch_error(handle: u64, events: &[McCampaignEventV2], context: &str) {
    if !events.iter().any(|event| event.kind == 255) {
        return;
    }
    let diagnostic = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let error_slots = reset
            .dispatch_events()
            .iter()
            .filter_map(|event| match event {
                Mc2DispatchEvent::Error { source_slot, .. } => Some(*source_slot),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let arena_rows = reset
            .economy()
            .live_arena_slots()
            .into_iter()
            .filter(|(source_slot, _)| error_slots.contains(source_slot))
            .collect::<Vec<_>>();
        Ok(format!(
            "events={:?}, live_arena_rows={arena_rows:?}",
            reset.dispatch_events()
        ))
    })
    .expect("read the source scheduler error diagnostic");
    panic!("{context} encountered a source scheduler error: {diagnostic}");
}

fn level2_source_valid_castle_route(handle: u64) -> (Position, (u16, u16)) {
    with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Castle-site selection requires a Level 2 campaign session",
            ));
        };
        let group1_center = reset
            .mission()
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 19)
            .map(|feature| feature.world_position(0))
            .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing authored row 19"))?;
        let auxiliary = reset.shared_spells().beyond_sight_auxiliary();
        if reset.state.yaw != 0 || auxiliary.next_entity != 0 {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "the package-backed Castle route requires the qualified yaw-zero launch",
            ));
        }
        let terrain = reset.canonical_terrain();
        let angles = &terrain.planes().angles;
        let site_is_source_valid = |position: Position| {
            let tile_x = (position.x >> 8) as u8;
            let tile_y = (position.y >> 8) as u8;
            let terrain_clear = (0..8_u8).all(|row| {
                (0..8_u8).all(|column| {
                    let x = tile_x.wrapping_sub(8).wrapping_add(column);
                    let y = tile_y.wrapping_sub(8).wrapping_add(row);
                    angles[usize::from(x) + usize::from(y) * 256].cast_signed() >= 0
                })
            });
            let source_lists_clear = reset.entities().iter().all(|entity| {
                let blocks = entity.active
                    && ((entity.class == 3 && entity.model == 2 && entity.life >= 0)
                        || (entity.class == 10 && entity.model == 45));
                if !blocks {
                    return true;
                }
                let record = entity.record_bytes();
                let half_x = i16::from_le_bytes([record[88], record[89]]);
                let half_y = i16::from_le_bytes([record[90], record[91]]);
                let dx =
                    i16::from_ne_bytes(entity.position.x.wrapping_sub(position.x).to_ne_bytes());
                let dy =
                    i16::from_ne_bytes(entity.position.y.wrapping_sub(position.y).to_ne_bytes());
                i32::from(dx).abs() > i32::from(half_x) + 2_560
                    || i32::from(dy).abs() > i32::from(half_y) + 2_560
            });
            terrain_clear && source_lists_clear
        };
        (0..=u8::MAX)
            .flat_map(|tile_y| (0..=u8::MAX).map(move |tile_x| (tile_x, tile_y)))
            .filter_map(|(tile_x, tile_y)| {
                let target_position = Position {
                    x: (u16::from(tile_x) << 8).wrapping_add(128),
                    y: (u16::from(tile_y) << 8).wrapping_add(128),
                    z: terrain.ground_height(Position {
                        x: (u16::from(tile_x) << 8).wrapping_add(128),
                        y: (u16::from(tile_y) << 8).wrapping_add(128),
                        z: 0,
                    }),
                };
                let launch_position = Position {
                    y: target_position.y.wrapping_add(4_096),
                    z: target_position.z.wrapping_add(4_096),
                    ..target_position
                };
                let mut flyer_position = Position {
                    x: launch_position.x.wrapping_sub(256),
                    ..launch_position
                };
                if terrain.ground_height(flyer_position) > flyer_position.z {
                    flyer_position = launch_position;
                }
                if !site_is_source_valid(flyer_position) || !site_is_source_valid(target_position) {
                    return None;
                }
                let mut root_x = u16::from(tile_x) << 8;
                let root_y = u16::from(tile_y) << 8;
                if (u16::from(tile_x) + u16::from(tile_y)) & 1 != 0 {
                    root_x = root_x.wrapping_add(256);
                }
                Some((launch_position, (root_x, root_y)))
            })
            .min_by_key(|(_, (root_x, root_y))| {
                let dx = i16::from_ne_bytes(root_x.wrapping_sub(group1_center.x).to_ne_bytes());
                let dy = i16::from_ne_bytes(root_y.wrapping_sub(group1_center.y).to_ne_bytes());
                (
                    i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy),
                    *root_y,
                    *root_x,
                )
            })
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 2 has no source-valid Castle site near group 1",
                )
            })
    })
    .expect("derive the package-backed Castle launch and root")
}

#[test]
fn level2_campaign_public_tier0_possession_claims_group1_mana_from_strict_target_flight() {
    let handle = open_level2_campaign_handle_with_asset_cache(&qualified_level2_day_owner_cache());
    let row11_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    assert!(
        group_events.iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_AUTHORED_GROUP
                && event.source_slot == 23
                && event.family == 1
                && event.flags == 34
        }),
        "row-11 must publish the complete 34-row group-1 scan"
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
    assert!(
        group1_spheres.len() >= 6,
        "ordinary row-11 production exposes the real group-1 model-39 population"
    );
    let group1_sphere_ids = group1_spheres
        .iter()
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    let target = group1_spheres
        .iter()
        .copied()
        .find(|sphere| sphere.sphere_id == 141)
        .expect("the pinned public group-1 target is materialized");
    let selected_target_sphere_id = target.sphere_id;

    let Level2PublicPossessionClaimRoute {
        latest_snapshot,
        player_entity_index,
        new_claimed_group1_sphere_ids,
        merge_lineage,
        ..
    } = drive_level2_public_group1_possession_claims(
        handle,
        input,
        selected_target_sphere_id,
        &group1_sphere_ids,
    );
    assert_eq!(
        new_claimed_group1_sphere_ids.len(),
        4,
        "the package-backed target-141 callback claims its four live overlaps"
    );
    let live_owned_sphere_ids = campaign_mana_spheres_public(handle, latest_snapshot.generation)
        .into_iter()
        .filter(|sphere| {
            sphere.active != 0
                && sphere.model == 39
                && sphere.owner_entity_index == player_entity_index
        })
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    assert!(
        new_claimed_group1_sphere_ids.iter().all(|source_id| {
            !sphere_lineage_descendants(*source_id, &merge_lineage)
                .is_disjoint(&live_owned_sphere_ids)
        }),
        "every public Possession receipt has a live player-owned sphere descendant"
    );
    assert!(latest_snapshot.revision > 1);
    assert_eq!(mc_native_campaign_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn level2_campaign_public_invisible_casts_after_castle_reaches_live_mana_gate() {
    // Use the package/cache pair installed by the current player. The old
    // aggregate cache has a stale audio receipt and cannot open even under
    // the pre-change native library.
    let open = || {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = generated
            .join("mc-level2-vault-successor-closure-20261025")
            .to_string_lossy()
            .into_owned();
        let cache = generated
            .join("mc-level2-vault-successor-environment-20261025")
            .to_string_lossy()
            .into_owned();
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
            mc_native_campaign_session_open_with_asset_cache(
                package.as_ptr(),
                package.len() as u32,
                std::ptr::null(),
                0,
                cache.as_ptr(),
                cache.len() as u32,
                &raw const selection,
                &raw mut handle
            ),
            MC_STATUS_OK
        );
        handle
    };
    let handle = open();
    let (castle_launch_position, _) = level2_source_valid_castle_route(handle);
    let isolated_handle = open();
    let initial_hash = campaign_world_hash_public(handle);
    assert_eq!(campaign_world_hash_public(isolated_handle), initial_hash);
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        "ordinary row-11 production exposes the real group-1 model-39 population"
    );
    let group1_sphere_ids = group1_spheres
        .iter()
        .map(|sphere| sphere.sphere_id)
        .collect::<BTreeSet<_>>();
    let target = group1_spheres
        .iter()
        .copied()
        .find(|sphere| sphere.sphere_id == 141)
        .expect("the pinned public group-1 target is materialized");

    let Level2PublicPossessionClaimRoute {
        mut input,
        mut latest_snapshot,
        mut player_entity_index,
        mut new_claimed_group1_sphere_ids,
        mut merge_lineage,
        castle_defender_events: mut defender_events,
    } = drive_level2_public_group1_possession_claims(
        handle,
        input,
        target.sphere_id,
        &group1_sphere_ids,
    );
    let mut castle_collected_sphere_ids = BTreeSet::new();

    // Build the ordinary Castle that owns the Invisible predicate's mana
    // relation. The public route intentionally uses the same selected
    // spell command as the managed bridge.
    input.player_x = castle_launch_position.x;
    input.player_y = castle_launch_position.y;
    input.player_z = castle_launch_position.z;
    input.command = valid_command();
    let castle_launch_state = || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign Castle diagnostics")
            };
            let shared = reset.shared_spells();
            let slot = shared.local_player().enabled_spell_holders[2];
            let holder = shared
                .dynamic_pickups()
                .iter()
                .chain(shared.holders())
                .chain(shared.pickups())
                .find(|holder| holder.index == slot)
                .map(|holder| {
                    (
                        holder.index,
                        holder.action,
                        holder.active_value,
                        holder.activation_value,
                        holder.max_mana,
                        holder.mana,
                        holder.mana_regen,
                        holder.level_gate,
                    )
                });
            Ok((
                shared.player_entity().index,
                shared.player_entity().life,
                shared.player_entity().mana,
                shared.player_entity().mana_regen,
                slot,
                holder,
                reset
                    .economy()
                    .castle_projectiles()
                    .iter()
                    .map(|flyer| {
                        (
                            flyer.source_entity_index,
                            flyer.holder_entity_index,
                            flyer.active,
                            flyer.life,
                            flyer.position,
                            flyer.site_rejected,
                        )
                    })
                    .collect::<Vec<_>>(),
            ))
        })
        .expect("read authoritative Castle admission/slot state")
    };
    let castle_can_arm = || {
        let state = castle_launch_state();
        state.5.is_some_and(|holder| {
            holder.2 == 0 && holder.3 > 0 && state.1 >= 0 && state.2 >= holder.4 && state.3 >= 0
        })
    };
    for _ in 0..source_pass_bound {
        if castle_can_arm() {
            break;
        }
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        assert_no_level2_dispatch_error(handle, &events, "Castle holder charge/admission");
        latest_snapshot = snapshot;
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
    }
    let castle_before = castle_launch_state();
    assert!(
        castle_can_arm(),
        "ordinary source upkeep admits the actual cold Castle holder: {castle_before:?}"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 2,
        ..McCommandV2::default()
    };
    let (castle_armed_snapshot, castle_arm_events) = step_level2_campaign_public(handle, input);
    let economy_events = campaign_economy_events_with_mana_lineage(
        handle,
        &mut merge_lineage,
        &mut castle_collected_sphere_ids,
    );
    retain_castle_defender_receipts(&economy_events, &mut defender_events);
    let castle_after = castle_launch_state();
    let births = castle_after
        .6
        .iter()
        .filter(|flyer| flyer.1 == castle_before.4 && flyer.2)
        .collect::<Vec<_>>();
    assert_eq!(
        births.len(),
        1,
        "accepted Castle request allocates one real flyer; before={castle_before:?}; after={castle_after:?}"
    );
    let flyer_slot = births[0].0;
    let casting_holder_slot = births[0].1;
    assert_eq!(casting_holder_slot, castle_before.4);
    assert_eq!(
        castle_after.5.unwrap().2,
        castle_before.5.unwrap().3.cast_signed() - 1
    );
    assert!(
        !births[0].5,
        "the real flyer was admitted by the source site predicate"
    );
    assert_eq!(
        castle_arm_events
            .iter()
            .filter(|event| event.pass == 0 && event.kind == 8 && event.family == 38)
            .count(),
        usize::from(flyer_slot > casting_holder_slot),
        "ascending source dispatch visits a newborn only when its slot follows the CASTING HOLDER; before={castle_before:?}; after={castle_after:?}; revision={}; events={:?}",
        castle_armed_snapshot.revision,
        castle_arm_events
            .iter()
            .map(|event| (
                event.kind,
                event.pass,
                event.source_slot,
                event.family,
                event.flags
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        castle_arm_events
            .iter()
            .all(|event| !(event.kind == 8 && event.family == 1))
    );
    input.command = valid_command();
    let mut castle_snapshot = castle_armed_snapshot;
    if flyer_slot < casting_holder_slot {
        let (snapshot, events) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        assert_no_level2_dispatch_error(handle, &events, "deferred Castle flyer visit");
        assert_eq!(
            events
                .iter()
                .filter(|event| {
                    event.kind == 8 && event.family == 38 && event.source_slot == flyer_slot
                })
                .count(),
            1,
            "the exact lower-slot newborn receives its first callback on the following source pass"
        );
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
    }
    let mut castle_construction_started = false;
    for _ in 0..source_pass_bound {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
        castle_construction_started = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.active && child.model == 42 && child.action == 44))
        })
        .expect("read the ordinary Castle construction authority");
        if castle_construction_started {
            break;
        }
    }
    assert!(
        castle_construction_started,
        "the public Castle flyer reaches its real model-42 construction child"
    );

    // The source BUILD callback initializes at 19, then retains a -25 hold,
    // so 64 ordinary child visits is a conservative bound from the observed
    // production constructor rather than from the earlier flyer launch.
    let mut castle_defender_ready = false;
    for _ in 0..64 {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
        castle_defender_ready = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let Some(castle) = reset.economy().castle() else {
                return Ok(false);
            };
            Ok(castle.action == 4
                && reset.economy().castle_children().iter().any(|child| {
                    child.active
                        && child.source_class == 3
                        && child.model == 3
                        && child.action == 9
                        && castle.defender_slots.contains(&child.source_entity_index)
                }))
        })
        .expect("read the ordinary Castle defender authority");
        if castle_defender_ready {
            break;
        }
    }
    let castle_readiness_state = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok((
            reset.economy().castle().map(|castle| {
                (
                    castle.action,
                    castle.build_state,
                    castle.phase,
                    castle.defender_slots,
                )
            }),
            reset
                .economy()
                .castle_children()
                .iter()
                .filter(|child| child.active)
                .map(|child| {
                    (
                        child.source_entity_index,
                        child.source_class,
                        child.model,
                        child.action,
                        child.mana,
                    )
                })
                .collect::<Vec<_>>(),
        ))
    })
    .expect("read Castle readiness diagnostics");
    assert!(
        castle_defender_ready,
        "the public Castle flyer reaches stable action 4 and creates its real defender; state={castle_readiness_state:?}"
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
    let economy_events = campaign_economy_events_with_mana_lineage(
        handle,
        &mut merge_lineage,
        &mut castle_collected_sphere_ids,
    );
    retain_castle_defender_receipts(&economy_events, &mut defender_events);
    let (pickup_snapshot, _) = step_level2_campaign_public(handle, input);
    let economy_events = campaign_economy_events_with_mana_lineage(
        handle,
        &mut merge_lineage,
        &mut castle_collected_sphere_ids,
    );
    retain_castle_defender_receipts(&economy_events, &mut defender_events);
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
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
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

    let castle_mana_before_funding = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.economy().castle().map_or(0, |castle| castle.mana))
    })
    .expect("read the pre-funding Castle mana carrier");
    assert!(
        castle_mana_before_funding < 2_000,
        "the planted negative starts below Invisible's strict Castle-mana gate"
    );
    input.command = McCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCommandV2>(),
        flags: 0x40,
        requested_spell: 11,
        ..McCommandV2::default()
    };
    let (rejected_snapshot, _) = step_level2_campaign_public(handle, input);
    let castle_mana_after_rejection = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok(reset.economy().castle().map_or(0, |castle| castle.mana))
    })
    .expect("read Castle mana after the planted negative");
    assert!(
        castle_mana_after_rejection < 2_000,
        "the rejected spell visit remains below the authoritative Castle-mana threshold"
    );
    let rejected = campaign_invisible_state_public(handle, rejected_snapshot.generation);
    assert_eq!(
        (
            rejected.active_value,
            rejected.parent_visibility_mode,
            rejected.parent_mana_regen,
        ),
        (
            cold_invisible.active_value,
            cold_invisible.parent_visibility_mode,
            cold_invisible.parent_mana_regen,
        ),
        "a real selected-spell request remains cold while the live Castle is underfunded"
    );

    // Continue ordinary Possession requests against the still-live package
    // group-1 population until Castle mana plus live owned descendants can
    // satisfy Invisible's 2,000-mana Castle predicate.
    input.command = valid_command();
    let mut attempted_funding_targets = BTreeSet::new();
    loop {
        let (castle_mana, eligible_owned_mana) = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let castle_mana = reset.economy().castle().map_or(0, |castle| castle.mana);
            let eligible_owned_mana = reset
                .economy()
                .spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere.player_entity_index == player_entity_index
                        && sphere.control_flags & 0x2000 == 0
                        && sphere_has_source_ancestor(
                            sphere.id,
                            &new_claimed_group1_sphere_ids,
                            &merge_lineage,
                        )
                })
                .map(|sphere| sphere.mana)
                .sum::<i32>();
            Ok((castle_mana, eligible_owned_mana))
        })
        .expect("read the exact Castle-eligible mana relation while funding Invisible");
        if castle_mana.saturating_add(eligible_owned_mana) >= 2_000 {
            break;
        }
        let target = campaign_mana_spheres_public(handle, latest_snapshot.generation)
            .into_iter()
            .find(|sphere| {
                sphere.active != 0
                    && sphere.model == 39
                    && sphere.owner_entity_index == 0
                    && group1_sphere_ids.contains(&sphere.sphere_id)
                    && !attempted_funding_targets.contains(&sphere.sphere_id)
            })
            .expect("the package group-1 population retains a neutral funding target");
        attempted_funding_targets.insert(target.sphere_id);
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
        new_claimed_group1_sphere_ids.extend(route.new_claimed_group1_sphere_ids);
        merge_lineage.extend(route.merge_lineage);
        defender_events.extend(route.castle_defender_events);
    }

    input.player_x = castle_launch_position.x;
    input.player_y = castle_launch_position.y;
    input.player_z = castle_launch_position.z;
    input.command = valid_command();
    input.speed_index = 1;
    let mut live_castle_mana = 0;
    let mut stable_defender_return = None;
    for _ in 0..source_pass_bound.div_ceil(4) {
        let (snapshot, _) = step_level2_campaign_public(handle, input);
        castle_snapshot = snapshot;
        let economy_events = campaign_economy_events_with_mana_lineage(
            handle,
            &mut merge_lineage,
            &mut castle_collected_sphere_ids,
        );
        retain_castle_defender_receipts(&economy_events, &mut defender_events);
        let (castle_mana, stable_return) = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            let Some(castle) = reset.economy().castle() else {
                return Ok((0, None));
            };
            let stable_return = reset
                .economy()
                .castle_children()
                .iter()
                .find(|child| {
                    child.active
                        && child.source_class == 3
                        && child.model == 3
                        && child.action == 9
                        && child.mana >= 2_000
                        && child.target_sphere_slot == castle.source_entity_index
                        && !reset.economy().spheres().iter().any(|sphere| {
                            sphere.active
                                && !sphere.pending_delete
                                && sphere.player_entity_index == child.id_entity_index
                                && sphere.control_flags & 0x2000 == 0
                        })
                })
                .map(|child| {
                    (
                        castle.source_entity_index,
                        child.source_entity_index,
                        child.mana,
                    )
                });
            Ok((castle.mana, stable_return))
        })
        .expect("read the authoritative Castle and defender return carriers");
        live_castle_mana = castle_mana;
        stable_defender_return = stable_return;
        if live_castle_mana >= 2_000 || stable_defender_return.is_some() {
            break;
        }
    }
    let mut matching_arrival = None;
    if live_castle_mana < 2_000 {
        let (castle_slot, defender_slot, carried_mana) = stable_defender_return
            .expect("the exhausted eligible sphere set leaves the funded defender returning");
        let lineage_collection_mana = defender_events
            .iter()
            .filter(|event| {
                event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE
                    && event.source_slot == defender_slot
                    && event.primary_id == u32::from(defender_slot)
                    && event.entity_0 == defender_slot
                    && sphere_has_source_ancestor(
                        event.secondary_id,
                        &new_claimed_group1_sphere_ids,
                        &merge_lineage,
                    )
            })
            .map(|event| event.value_0)
            .sum::<i32>();
        assert_eq!(
            lineage_collection_mana, carried_mana,
            "the stable defender's carried mana is accounted for by its exact package lineage collection receipts"
        );
        // Once the Castle is the stable target, signed-wrapping XY has a
        // maximum diagonal of 46,340 source units. At speed 48 the pinned
        // movement reaches the Castle in at most 993 visits; speed index 1
        // executes four complete source passes per outer turn.
        for _ in 0..1_000_usize.div_ceil(4) {
            let (snapshot, dispatch_events) = step_level2_campaign_public(handle, input);
            assert_no_level2_dispatch_error(
                handle,
                &dispatch_events,
                "stable Castle defender return",
            );
            castle_snapshot = snapshot;
            let economy_events = campaign_economy_events_with_mana_lineage(
                handle,
                &mut merge_lineage,
                &mut castle_collected_sphere_ids,
            );
            if let Some(arrival) = economy_events.iter().copied().find(|event| {
                event.code == MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED
                    && event.source_slot == defender_slot
                    && event.primary_id == u32::from(defender_slot)
                    && event.entity_0 == castle_slot
                    && event.value_0 == carried_mana
            }) {
                matching_arrival = Some(arrival);
            }
            retain_castle_defender_receipts(&economy_events, &mut defender_events);
            live_castle_mana = with_campaign_session(handle, |session| {
                let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                    unreachable!("campaign fixture opened as Level 2")
                };
                Ok(reset.economy().castle().map_or(0, |castle| castle.mana))
            })
            .expect("read Castle mana during the stable defender return");
            if live_castle_mana >= 2_000 {
                break;
            }
        }
    }
    input.speed_index = 0;
    let castle_funding_state = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        Ok((
            reset.economy().castle().map(|castle| {
                (
                    castle.source_entity_index,
                    castle.mana,
                    castle.position,
                    castle.level,
                    castle.defender_slots,
                    castle.defender_total_mana,
                    castle.defender_target_spheres,
                )
            }),
            reset
                .economy()
                .castle_children()
                .iter()
                .filter(|child| child.active && child.source_class == 3 && child.model == 3)
                .map(|child| {
                    (
                        child.source_entity_index,
                        child.action,
                        child.mana,
                        child.player_entity_index,
                        child.target_sphere_slot,
                        child.position,
                        child.speed,
                        child.control_flags,
                        reset.canonical_terrain().ground_height(child.position),
                    )
                })
                .collect::<Vec<_>>(),
            reset
                .economy()
                .spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active
                        && !sphere.pending_delete
                        && sphere_has_source_ancestor(
                            sphere.id,
                            &new_claimed_group1_sphere_ids,
                            &merge_lineage,
                        )
                })
                .map(|sphere| {
                    (
                        sphere.id,
                        sphere.mana,
                        sphere.owner_entity_index,
                        sphere.player_entity_index,
                        sphere.control_flags,
                        sphere.position,
                    )
                })
                .collect::<Vec<_>>(),
        ))
    })
    .expect("read Castle funding diagnostics");
    assert!(
        live_castle_mana >= 2_000,
        "ordinary Castle economy satisfies Invisible's live mana gate; defender_events={defender_events:?}, stable_return={stable_defender_return:?}, matching_arrival={matching_arrival:?}, attempted={attempted_funding_targets:?}, collected_ids={castle_collected_sphere_ids:?}, state={castle_funding_state:?}"
    );
    if stable_defender_return.is_some() {
        assert!(
            matching_arrival.is_some(),
            "the same funded defender deposits into its stable Castle target"
        );
    }
    let mana_window_hud =
        campaign_day_hud_public(handle, castle_snapshot.generation, castle_snapshot.revision);
    assert!(
        mana_window_hud.local_player_mana >= 2_000,
        "the ordinary Castle economy satisfies Invisible's live mana gate"
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
        (1, 0, 33, 180, 181, 2_000, 2_000, 0),
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
    assert_eq!(invisible_first.parent_visibility_mode, 0);
    let global_visibility = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
            unreachable!("campaign fixture opened as a campaign")
        };
        Ok(reset.shared_spells().global_special_state().visibility_mode)
    })
    .expect("read the source Invisible holder's global A4 recipient");
    assert_eq!(
        global_visibility, 1,
        "NewEvent binds the holder A4 to GlobalSpecialState; its first pulse does not rewrite the parent player's A4"
    );
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
    let level2_package = qualified_level2_campaign_package();
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    let level2_package = qualified_level2_campaign_package();
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
    // sub_63570 admits the opponent without a Beyond Sight holder;
    // the separate active-spell map gate still suppresses its player dot.
    assert_eq!(initial_state.visibility_flags, 3);
    assert!(initial_state.entity_count > 0);
    // Approved native arrow contract: one objective outline plus seven vertices.
    assert_eq!(initial_state.mark_count, 8);

    let (local_player_slot, local_objective, live_player) =
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    assert!(initial_marks[1..].iter().all(|mark| mark.kind == 3));
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

    let family12_position = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    .expect("read authored family-12 source position");
    let mut input = McCampaignStepInputV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignStepInputV2>(),
        generation: 0,
        command: valid_command(),
        player_slot: 0,
        speed_index: 0,
        player_x: family12_position.x,
        player_y: family12_position.y,
        player_z: family12_position.z,
        ..McCampaignStepInputV2::default()
    };
    let mut snapshot = McCampaignStepSnapshotV2::default();
    let mut events = [McCampaignEventV2::default(); 1024];
    let mut event_count: u32;
    let mut acquired_family12 = false;
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
        acquired_family12 |= events[..event_count as usize].iter().any(|event| {
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
        "the real slot-38 source visit, not a flight-step magic number, acquires family 12"
    );
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
    assert_eq!(acquired_state.mark_count, 8);
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    assert!(
        acquired_marks[1..]
            .iter()
            .all(|mark| mark.kind == 3 && mark.visible == 0)
    );
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
    input.player_x = family12_position.x;
    input.player_y = family12_position.y;
    input.player_z = family12_position.z;
    let mut replay_acquired_family12 = false;
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
        replay_acquired_family12 |= events[..event_count as usize].iter().any(|event| {
            event.kind == MC_CAMPAIGN_EVENT_SPELL_ACQUISITION
                && event.family == 12
                && event.source_slot == 38
                && event.flags == 1
        });
        if replay_acquired_family12 {
            break;
        }
    }
    assert!(replay_acquired_family12);
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
        .join("../../../generated/mc-level2-vault-successor-closure-20261025");
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    // The real objective producer builds the BUILD21 vaults; its reward is
    // not an immediate family-11 pickup. Use the complete owner BUILD closure
    // and let those vaults finish their normal construction before supplying
    // a controlled destruction mailbox. This checks acquisition/API guards,
    // not earned combat or mission completion.
    let mut vaults = Vec::new();
    for _ in 0..64 {
        vaults = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset
                .ordinary()
                .houses()
                .iter()
                .filter(|house| house.active && house.building_index == 21)
                .copied()
                .collect::<Vec<_>>())
        })
        .expect("read actual objective-produced vaults");
        if !vaults.is_empty()
            && vaults.iter().all(|house| house.action == sim_core::house::HOUSE_LIVE_ACTION)
        {
            break;
        }
        let _ = step_level2_campaign_public(handle, input);
    }
    assert!(!vaults.is_empty(), "the real objective must construct its vaults");
    assert!(vaults.iter().all(|house| house.action == sim_core::house::HOUSE_LIVE_ACTION));
    with_campaign_session_mut(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let attacker = reset.state.entity_index as i16;
        for house in &vaults {
            reset
                .ordinary_mut()
                .inject_house_damage(house.entity_index, house.life + 1, attacker)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))?;
        }
        Ok(())
    })
    .expect("submit controlled damage to the real live vaults");
    // RemoveCastleStage constructs BUILD54 before publishing the vault's
    // reward group. That actual successor must also finish and be destroyed.
    let mut successors = Vec::new();
    for _ in 0..64 {
        successors = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset
                .ordinary()
                .houses()
                .iter()
                .filter(|house| {
                    house.active
                        && vaults.iter().any(|vault| {
                            house.building_index == vault.parameter.destruction_successor
                                && house.position.x == vault.position.x
                                && house.position.y == vault.position.y
                        })
                })
                .copied()
                .collect::<Vec<_>>())
        })
        .expect("read real vault demolition successors");
        if successors.len() == vaults.len()
            && successors.iter().all(|house| house.action == sim_core::house::HOUSE_LIVE_ACTION)
        {
            break;
        }
        let _ = step_level2_campaign_public(handle, input);
    }
    assert_eq!(successors.len(), vaults.len());
    assert!(successors.iter().all(|house| {
        house.action == sim_core::house::HOUSE_LIVE_ACTION
            && house.parameter.destruction_successor == 0
    }));
    with_campaign_session_mut(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let attacker = reset.state.entity_index as i16;
        for house in &successors {
            reset
                .ordinary_mut()
                .inject_house_damage(house.entity_index, house.life + 1, attacker)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))?;
        }
        Ok(())
    })
    .expect("submit controlled damage to the actual final vault successors");
    for _ in 0..64 {
        let ready = with_campaign_session(handle, |session| {
            let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
                unreachable!("campaign fixture opened as Level 2")
            };
            Ok(reset.economy().spell_pickups().iter().any(|source| source.authored_index == 156))
        })
        .expect("observe the real vault reward producer");
        if ready {
            break;
        }
        let _ = step_level2_campaign_public(handle, input);
    }
    let source = with_campaign_session(handle, |session| {
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
    assert_eq!(opponent.visibility_flags, 3);

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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
            let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
            let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &session.kind else {
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
        let CampaignSessionKind::Campaign { reset, .. } = &mut session.kind else {
            unreachable!("campaign fixture opened as Level 2")
        };
        let terrain = reset.canonical_terrain().clone();
        let sphere_id = reset
            .economy_mut()
            .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 111, 222, &terrain)
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
