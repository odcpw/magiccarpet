#[test]
fn level_one_mission_and_fireball_cross_the_generation_checked_abi() {
    let handle = open_fixture();
    let mut mission = McLevel1MissionSnapshotV2::default();
    assert_eq!(
        mc_native_level1_mission_snapshot(
            handle,
            &raw mut mission,
            struct_size::<McLevel1MissionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(mission.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
    assert_eq!((mission.current_stage, mission.text_selector), (0, 48));
    assert_eq!((mission.player_life, mission.player_mana), (10_000, 1_000));
    assert_eq!(mission.actor_count, 45);
    assert_eq!((mission.projectile_count, mission.impact_count), (0, 0));
    assert_eq!(mission.tasks_completed, 0);
    assert_eq!(mission.exit_sequence_active, 0);
    assert_eq!(mission.exit_sequence_complete, 0);
    assert_eq!(mission.exit_result_code, 0);
    assert_eq!(mission.exit_sequence_phase, 0);
    assert_eq!(mission.exit_sequence_timer, 0);
    let mut hud = McLevel1HudSnapshotV2::default();
    assert_eq!(
        mc_native_level1_hud_snapshot(handle, &raw mut hud, struct_size::<McLevel1HudSnapshotV2>(),),
        MC_STATUS_OK
    );
    assert_eq!(
        (hud.generation, hud.tick),
        (mission.generation, mission.tick)
    );
    assert_eq!(hud.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
    assert_eq!(
        (
            hud.mana_color_index,
            hud.mana_max_color_index,
            hud.life_color_index,
            hud.color_phase_1,
            hud.color_phase_2,
        ),
        (0xA4, 0xAA, 0x7B, 0, 0)
    );
    let mut spell_menu = McLevel1SpellMenuSnapshotV2::default();
    assert_eq!(
        mc_native_level1_spell_menu_snapshot(
            handle,
            &raw mut spell_menu,
            struct_size::<McLevel1SpellMenuSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (spell_menu.generation, spell_menu.tick),
        (mission.generation, mission.tick)
    );
    assert_eq!(spell_menu.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
    assert_eq!(
        (
            spell_menu.selected_ui_index,
            spell_menu.selected_tier,
            spell_menu.left_family,
            spell_menu.left_tier,
            spell_menu.right_family,
            spell_menu.right_tier,
        ),
        (0, 0, 0, 0, 1, 0)
    );
    assert_eq!(spell_menu.family_enabled, [1, 1, 0, 0]);
    assert_eq!(spell_menu.family_unlocked_tier, [0; 4]);
    assert_eq!(spell_menu.family_selected_tier, [0; 4]);
    assert_eq!(spell_menu.family_hand_eligibility, [0; 4]);
    assert_ne!(spell_menu.holder_entity_index[0], 0);
    assert_ne!(spell_menu.holder_entity_index[1], 0);
    assert!(hud.global_mana_denominator > 0);
    assert_eq!((hud.secured_mana_component, hud.goal_percent), (0, 0));
    assert_eq!(
        (
            hud.left.holder_entity_index,
            hud.left.parent_entity_index,
            hud.left.model,
            hud.left.tier,
            hud.left.active_value,
            hud.left.max_mana,
            hud.left.caster_mana,
            hud.left.frame_selector,
            hud.left.icon_selector,
        ),
        (153, 152, 0, 0, 0, 100, 1_000, 1, 123)
    );
    assert_eq!(
        (
            hud.right.holder_entity_index,
            hud.right.model,
            hud.right.active_value,
            hud.right.max_mana,
            hud.right.frame_selector,
            hud.right.icon_selector,
        ),
        (154, 1, 0, 100, 1, 124)
    );
    assert_eq!(
        (
            hud.selected_holder_entity_index,
            hud.selected_model,
            hud.selected_tier,
            hud.selected_frame_selector,
            hud.selected_icon_selector,
        ),
        (153, 0, 0, 1, 123)
    );
    let mut minimap = McLevel1MinimapMarksV2::default();
    assert_eq!(
        mc_native_level1_minimap_marks(
            handle,
            &raw mut minimap,
            struct_size::<McLevel1MinimapMarksV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        (minimap.generation, minimap.tick),
        (mission.generation, mission.tick)
    );
    assert_eq!(
        (
            minimap.mark_count,
            minimap.mark_pulse,
            minimap.arrow_valid,
            minimap.arrow_visible,
            minimap.mark_colour[0],
            minimap.mark_help_selector[0],
        ),
        (1, 1, 1, 0, 184, 0x51)
    );
    assert_eq!(minimap.reserved, [0; 4]);
    let expected_minimap_commands = with_session(handle, |session| {
        Ok(level1_minimap_entity_commands(session))
    })
    .unwrap();
    assert!(!expected_minimap_commands.is_empty());
    assert!(expected_minimap_commands.windows(2).all(|pair| {
        pair[0].kind == LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL
            || pair[0].source_slot <= pair[1].source_slot
    }));
    let civilian_slots = with_session(handle, |session| {
        Ok(session
            .mission
            .source_slots()
            .iter()
            .filter(|slot| slot.active && slot.class == 5 && (12..=14).contains(&slot.model))
            .map(|slot| slot.index)
            .collect::<Vec<_>>())
    })
    .unwrap();
    assert_eq!(civilian_slots.len(), 28);
    for source_slot in civilian_slots {
        assert!(expected_minimap_commands.iter().any(|command| {
            command.source_slot == source_slot
                && command.kind == sim_core::level1_mission::LEVEL1_MINIMAP_COMMAND_DOT
                && command.palette_index == 23
        }));
    }
    let mut entity_commands =
        [McLevel1MinimapEntityCommandV2::default(); LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS];
    let mut entity_info = McLevel1MinimapEntitiesInfoV2::default();
    assert_eq!(
        mc_native_level1_minimap_entities_copy(
            handle,
            mission.generation,
            mission.tick,
            entity_commands.as_mut_ptr(),
            entity_commands.len() as u32,
            struct_size::<McLevel1MinimapEntityCommandV2>(),
            &raw mut entity_info,
            struct_size::<McLevel1MinimapEntitiesInfoV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        entity_info.total_count as usize,
        expected_minimap_commands.len()
    );
    assert_eq!(
        entity_info.command_hash,
        level1_minimap_command_hash(&expected_minimap_commands)
    );
    assert_eq!(
        (
            entity_info.cross_x,
            entity_info.cross_y,
            entity_info.cross_colour,
            entity_info.cross_arm_length,
            entity_info.authority,
        ),
        (63, 64, 176, 10, MC_AUTHORITY_LEVEL1_MISSION_LIVE)
    );
    assert_eq!(
        &entity_commands[..expected_minimap_commands.len()],
        expected_minimap_commands.as_slice()
    );
    assert_eq!(entity_info.reserved_0, [0; 4]);
    assert_eq!(entity_info.reserved, [0; 4]);
    assert_eq!(
        mission.fidelity_gap_flags & MC_LEVEL1_GAP_CASTLE_BUILD_FOOTPRINT,
        0
    );
    assert_eq!(
        mission.fidelity_gap_flags & MC_LEVEL1_GAP_CASTLE_CONSTRUCTION_TIMING,
        0
    );
    assert_eq!(
        mission.fidelity_gap_flags & MC_LEVEL1_GAP_MANA_SPHERE_ALLOCATION_RNG,
        0
    );
    assert_eq!(
        mission.fidelity_gap_flags & MC_LEVEL1_GAP_MANA_CARRIER_FLIGHT,
        0
    );
    assert_eq!(mission.fidelity_gap_flags, 0);

    let mut actors = [McLevel1ActorSnapshotV2::default(); 45];
    let mut written = 0;
    assert_eq!(
        mc_native_level1_actors_copy(
            handle,
            mission.generation,
            mission.tick,
            0,
            actors.as_mut_ptr(),
            actors.len() as u32,
            struct_size::<McLevel1ActorSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, 45);
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::Archer as u8
            })
            .count(),
        4
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::Firefly as u8
            })
            .count(),
        6
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::MultipartHead as u8
            })
            .count(),
        4
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::ExperienceScroll as u8
            })
            .count(),
        5
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::Tree as u8
                    && actor.render_parameter_index == sim_core::level1_mission::TREE_RENDER_INDEX_A
                    && actor.base_tmap == sim_core::level1_mission::TREE_BASE_TMAP_A
            })
            .count(),
        18
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::Stone as u8
                    && actor.render_parameter_index == sim_core::level1_mission::STONE_RENDER_INDEX
                    && actor.base_tmap == sim_core::level1_mission::STONE_BASE_TMAP
            })
            .count(),
        1
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind
                    == sim_core::level1_mission::Level1ActorKind::CheckpointSmokeEmitter as u8
                    && actor.visible == 0
                    && actor.render_parameter_index == u16::MAX
            })
            .count(),
        3
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::ExitTarget as u8
                    && actor.visible == 0
                    && actor.base_tmap == 461
            })
            .count(),
        1
    );
    assert_eq!(
        actors
            .iter()
            .filter(|actor| {
                actor.kind == sim_core::level1_mission::Level1ActorKind::ExitSwitch as u8
                    && actor.visible == 0
                    && actor.render_parameter_index == u16::MAX
            })
            .count(),
        1
    );
    assert!(actors.iter().all(|actor| actor.active == 0));

    let fire = McCommandV2 {
        flags: 0x10,
        ..valid_command()
    };
    let mut flight = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const fire,
            &raw mut flight,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(
        mc_native_level1_mission_snapshot(
            handle,
            &raw mut mission,
            struct_size::<McLevel1MissionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(mission.tick, flight.tick);
    assert!(mission.projectile_count + mission.impact_count > 0);

    if mission.projectile_count != 0 {
        let mut projectile = McFireballProjectileSnapshotV2::default();
        assert_eq!(
            mc_native_fireball_projectiles_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                &raw mut projectile,
                1,
                struct_size::<McFireballProjectileSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        assert_eq!(projectile.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_eq!(projectile.projectile_kind, 1);
        assert_eq!(projectile.render_index, FIREBALL_RENDER_INDEX);
        assert_ne!(
            projectile.reserved[0], 0,
            "reserved word 0 preserves source-slot identity independently of projectile_id"
        );
    } else {
        let mut impact = McFireballImpactSnapshotV2::default();
        assert_eq!(
            mc_native_fireball_impacts_copy(
                handle,
                mission.generation,
                mission.tick,
                0,
                &raw mut impact,
                1,
                struct_size::<McFireballImpactSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        assert_eq!(impact.authority, MC_AUTHORITY_LEVEL1_MISSION_LIVE);
        assert_ne!(
            impact.reserved[0], 0,
            "reserved word 0 preserves source-slot identity independently of impact_id"
        );
    }

    assert_eq!(
        mc_native_level1_actors_copy(
            handle,
            mission.generation,
            mission.tick.saturating_sub(1),
            0,
            std::ptr::null_mut(),
            0,
            struct_size::<McLevel1ActorSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_GENERATION_MISMATCH
    );

    let mut reset = McSessionSnapshotV2::default();
    assert_eq!(
        mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
        MC_STATUS_OK
    );
    let idle = valid_command();
    let (tree_source_slot, tree_authored_index, tree_position) =
        with_session_mut(handle, |session| {
            let activation_target = session
                .world
                .level1_mission()
                .possession_targets()
                .first()
                .map(|target| target.entity_index)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "qualified Level 1 has no imported house activation target",
                    )
                })?;
            session
                .mission
                .commit_imported_house_demolition(activation_target, 1, &[])
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "production xtype-one activation failed",
                    )
                })?;
            session
                .mission
                .actors()
                .iter()
                .find(|actor| actor.kind == Level1ActorKind::Tree && actor.active)
                .map(|actor| {
                    (
                        actor.source_entity_index,
                        actor.authored_index,
                        actor.position,
                    )
                })
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "group-one activation produced no live tree",
                    )
                })
        })
        .unwrap();

    for shot in 0..2 {
        with_session_mut(handle, |session| {
            session.state.position = Position {
                x: tree_position.x.wrapping_sub(512),
                y: tree_position.y,
                z: tree_position.z.wrapping_add(100),
            };
            session.state.yaw = 512;
            session.state.pitch_accumulator = 0;
            session.state.effective_pitch = 0;
            session.state.target_speed = 0;
            session.state.actual_speed = 0;
            session.state.strafe_speed = 0;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const fire,
                &raw mut flight,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut reached = false;
        for _ in 0..24 {
            reached = with_session(handle, |session| {
                let tree = session
                    .mission
                    .actors()
                    .iter()
                    .find(|actor| actor.source_entity_index == tree_source_slot)
                    .ok_or_else(|| {
                        AbiError::new(
                            MC_STATUS_VALIDATION_FAILED,
                            "production tree source row disappeared",
                        )
                    })?;
                Ok(if shot == 0 {
                    tree.life < sim_core::level1_mission::TREE_LIFE
                } else {
                    session
                        .mission
                        .impacts()
                        .iter()
                        .any(|impact| impact.model == GROUND_FIRE_MODEL && impact.active)
                })
            })
            .unwrap();
            if reached {
                break;
            }
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const idle,
                    &raw mut flight,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        assert!(
            reached,
            "production shot {shot} reaches the real tree lifecycle"
        );
    }

    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: flight.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut events = vec![McEventV2::default(); 256];
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            events.as_mut_ptr(),
            u32::try_from(events.len()).unwrap(),
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    events.truncate(usize::try_from(batch.written).unwrap());
    assert!(events.iter().any(|event| {
        event.kind == MC_EVENT_LEVEL1_ACTOR_DAMAGED
            && event.entity_index == tree_authored_index
            && event.code == u32::from(flight.entity_index)
    }));

    let first_ground_fire = read_level1_impacts(handle)
        .into_iter()
        .find(|impact| impact.model == GROUND_FIRE_MODEL && impact.active != 0)
        .expect("the second production hit creates live class-10/model-6 ground fire");
    assert_eq!(
        (
            first_ground_fire.class_id,
            first_ground_fire.render_parameter_index,
            first_ground_fire.base_tmap,
            first_ground_fire.render_mode,
            first_ground_fire.logical_height,
            first_ground_fire.animation_frame,
            first_ground_fire.authority,
        ),
        (
            FIREBALL_IMPACT_CLASS,
            228,
            77,
            1,
            50,
            0,
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        )
    );
    assert!(
        first_ground_fire.reserved[0] > 0 && first_ground_fire.reserved[0] < 1_000,
        "the production ground fire retains its real source-arena slot"
    );
    with_session_mut(handle, |session| {
        assert!(
            session
                .renderable_map_chains
                .tracks(u16::try_from(first_ground_fire.reserved[0]).unwrap()),
            "model 6 follows NewAdd0A06's AddEventToMap path"
        );
        session.state.position = Position {
            x: tree_position.x.wrapping_add(16_384),
            y: tree_position.y.wrapping_add(16_384),
            z: tree_position.z.wrapping_add(2_000),
        };
        session.state.target_speed = 0;
        session.state.actual_speed = 0;
        session.state.strafe_speed = 0;
        session.mission.player_life = 1_000_000;
        Ok(())
    })
    .unwrap();

    let mut changed_rows = Vec::new();
    for _ in 0..256 {
        let Some(ground_fire) = read_level1_impacts(handle)
            .into_iter()
            .find(|impact| impact.impact_id == first_ground_fire.impact_id && impact.active != 0)
        else {
            break;
        };
        let phase = usize::from(ground_fire.render_parameter_index.saturating_sub(228));
        let expected_heights = [50, 150, 300, 450, 600, 700, 800, 950];
        assert!(
            phase < expected_heights.len(),
            "ground-fire row {} lies outside 228..235",
            ground_fire.render_parameter_index
        );
        assert_eq!(
            (
                ground_fire.base_tmap,
                ground_fire.render_mode,
                ground_fire.logical_height,
                ground_fire.animation_frame,
            ),
            (77, 1, expected_heights[phase], 0)
        );
        if changed_rows.last().copied() != Some(ground_fire.render_parameter_index) {
            changed_rows.push(ground_fire.render_parameter_index);
        }
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut flight,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    assert_eq!(
        changed_rows,
        [
            228, 229, 230, 231, 232, 233, 234, 235, 234, 233, 232, 231, 230, 229, 228
        ],
        "the real tree fire expands and contracts through the source phase table"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

fn configure_shared_possession_holder(handle: u64) {
    with_session_mut(handle, |session| {
        session.mission.possession_experience = 4_800;
        session.mission.possession_tier = 2;
        let fire = session
            .context
            .spell_fire
            .as_mut()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        fire.selected_holder_index = 153;
        fire.selected_spell_ui_index = 0;
        fire.selected_subspell_index = 0;
        fire.spell_levels[1] = 2;
        fire.family_selected_tiers[1] = 2;
        fire.holders
            .iter_mut()
            .find(|holder| holder.entity_index == 154)
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent"))?
            .level_gate = 2;
        Ok(())
    })
    .unwrap();
}

fn submit_spell_menu_command(
    handle: u64,
    snapshot: &mut McSessionSnapshotV2,
    opcode: i8,
    argument_1: i8,
    argument_2: i8,
) {
    let command = McCommandV2 {
        opcode,
        argument_1,
        argument_2,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const command,
            &raw mut *snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
}

#[test]
fn sealed_level1_raw_opcode41_preserves_requested_tier_before_hand_assignment() {
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2::default();
    let physical_before = with_session(handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_ref()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        let holder = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == 154)
            .ok_or_else(|| {
                AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
            })?;
        Ok((holder.level_gate, holder.activation_value))
    })
    .unwrap();

    submit_spell_menu_command(handle, &mut snapshot, 40, 1, 1);
    submit_spell_menu_command(handle, &mut snapshot, 41, 2, 0);

    let menu = read_level1_spell_menu(handle);
    assert_eq!(
        (
            menu.selected_ui_index,
            menu.selected_tier,
            menu.right_family,
            menu.right_tier,
            menu.family_selected_tier[1],
        ),
        (1, 2, 1, 0, 2),
        "sealed Level 1 preserves opcode-41's raw intermediate selector"
    );
    let physical_after = with_session(handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_ref()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        let holder = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == 154)
            .ok_or_else(|| {
                AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent")
            })?;
        Ok((holder.level_gate, holder.activation_value))
    })
    .unwrap();
    assert_eq!(physical_after, physical_before);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

fn read_level1_hud(handle: u64) -> McLevel1HudSnapshotV2 {
    let mut hud = McLevel1HudSnapshotV2::default();
    assert_eq!(
        mc_native_level1_hud_snapshot(handle, &raw mut hud, struct_size::<McLevel1HudSnapshotV2>(),),
        MC_STATUS_OK
    );
    hud
}

fn read_level1_spell_menu(handle: u64) -> McLevel1SpellMenuSnapshotV2 {
    let mut menu = McLevel1SpellMenuSnapshotV2::default();
    assert_eq!(
        mc_native_level1_spell_menu_snapshot(
            handle,
            &raw mut menu,
            struct_size::<McLevel1SpellMenuSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    menu
}

fn read_level1_impacts(handle: u64) -> Vec<McFireballImpactSnapshotV2> {
    let mut mission = McLevel1MissionSnapshotV2::default();
    assert_eq!(
        mc_native_level1_mission_snapshot(
            handle,
            &raw mut mission,
            struct_size::<McLevel1MissionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let mut impacts =
        vec![McFireballImpactSnapshotV2::default(); usize::try_from(mission.impact_count).unwrap()];
    let mut written = 0;
    assert_eq!(
        mc_native_fireball_impacts_copy(
            handle,
            mission.generation,
            mission.tick,
            0,
            impacts.as_mut_ptr(),
            mission.impact_count,
            struct_size::<McFireballImpactSnapshotV2>(),
            &raw mut written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(written, mission.impact_count);
    impacts
}

fn assert_raw_tier_and_family_minus_one(handle: u64, snapshot: &mut McSessionSnapshotV2) {
    submit_spell_menu_command(handle, snapshot, 40, 0, 1);
    let cursor_before_raw = with_session(handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_ref()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        Ok((
            fire.selected_spell_ui_index,
            fire.selected_subspell_index,
            fire.selected_holder_index,
        ))
    })
    .unwrap();
    submit_spell_menu_command(handle, snapshot, 31, 1, 7);
    let menu = read_level1_spell_menu(handle);
    assert_eq!((menu.left_family, menu.left_tier), (1, 7));
    assert_eq!(menu.family_selected_tier[1], 7);
    let (cursor_after_raw, holder_tier) = with_session(handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_ref()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        let holder_tier = fire
            .holders
            .iter()
            .find(|holder| holder.entity_index == 154)
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent"))?
            .level_gate;
        Ok((
            (
                fire.selected_spell_ui_index,
                fire.selected_subspell_index,
                fire.selected_holder_index,
            ),
            holder_tier,
        ))
    })
    .unwrap();
    assert_eq!(cursor_after_raw, cursor_before_raw);
    assert_eq!(holder_tier, 2);

    submit_spell_menu_command(handle, snapshot, 40, 1, 1);
    let reopened = read_level1_spell_menu(handle);
    assert_eq!(
        (
            reopened.selected_ui_index,
            reopened.selected_tier,
            reopened.family_selected_tier[1],
        ),
        (1, 7, 7),
        "family press exposes the raw current tier before held selection clamps it"
    );
    submit_spell_menu_command(handle, snapshot, 41, 2, 0);
    let before_no_op = read_level1_spell_menu(handle);
    assert_eq!(
        (
            before_no_op.selected_tier,
            before_no_op.family_selected_tier[1],
        ),
        (2, 2)
    );
    submit_spell_menu_command(handle, snapshot, 32, -1, 7);
    let menu = read_level1_spell_menu(handle);
    assert_eq!(
        (
            menu.selected_ui_index,
            menu.selected_tier,
            menu.left_family,
            menu.left_tier,
            menu.right_family,
            menu.right_tier,
            menu.family_selected_tier,
            menu.holder_entity_index,
        ),
        (
            before_no_op.selected_ui_index,
            before_no_op.selected_tier,
            before_no_op.left_family,
            before_no_op.left_tier,
            before_no_op.right_family,
            before_no_op.right_tier,
            before_no_op.family_selected_tier,
            before_no_op.holder_entity_index,
        )
    );
}

fn assert_spell_menu_cast_routes(handle: u64, snapshot: &mut McSessionSnapshotV2) {
    submit_spell_menu_command(handle, snapshot, 31, 1, 0);
    submit_spell_menu_command(handle, snapshot, 32, 0, 0);
    submit_spell_menu_command(handle, snapshot, 38, 0, 2);
    let menu = read_level1_spell_menu(handle);
    assert_eq!(menu.family_hand_eligibility, [2, 0, 0, 0]);
    for (flags, expected_mask) in [(0x10_i8, 256_u32), (0x20_i8, 512_u32)] {
        let command = McCommandV2 {
            flags,
            ..valid_command()
        };
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut *snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (left_holder, right_holder, direction_mask) = with_session(handle, |session| {
            let fire =
                session.context.spell_fire.as_ref().ok_or_else(|| {
                    AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent")
                })?;
            Ok((
                fire.left_holder_index,
                fire.right_holder_index,
                fire.caster.control_flags & 0x0300,
            ))
        })
        .unwrap();
        assert_eq!((left_holder, right_holder), (154, 153));
        assert_eq!(direction_mask, expected_mask);
    }
}

#[test]
fn top_hud_uses_shared_holder_tier_while_menu_preserves_per_hand_tiers() {
    let handle = open_fixture();
    configure_shared_possession_holder(handle);

    let mut snapshot = McSessionSnapshotV2::default();
    assert_eq!(read_level1_hud(handle).selected_model, 0);
    submit_spell_menu_command(handle, &mut snapshot, 40, 1, 1);
    let pressed_hud = read_level1_hud(handle);
    assert_eq!(
        (pressed_hud.selected_model, pressed_hud.selected_tier),
        (1, 2)
    );
    let selected_fire = McCommandV2 {
        flags: 0x40,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const selected_fire,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let selected_route = with_session(handle, |session| {
        let fire = session
            .context
            .spell_fire
            .as_ref()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        Ok((
            fire.selected_holder_index,
            fire.accepted_models.clone(),
            fire.caster.control_flags & 0x0300,
        ))
    })
    .unwrap();
    assert_eq!(selected_route, (154, vec![1], 256));
    submit_spell_menu_command(handle, &mut snapshot, 41, 2, 0);
    submit_spell_menu_command(handle, &mut snapshot, 31, 1, 2);
    submit_spell_menu_command(handle, &mut snapshot, 40, 1, 2);
    submit_spell_menu_command(handle, &mut snapshot, 41, 1, 0);
    submit_spell_menu_command(handle, &mut snapshot, 32, 1, 1);

    let hud = read_level1_hud(handle);
    let menu = read_level1_spell_menu(handle);
    assert_eq!((hud.left.model, hud.left.tier), (1, 1));
    assert_eq!((hud.right.model, hud.right.tier), (1, 1));
    assert_eq!((hud.selected_model, hud.selected_tier), (1, 1));
    assert_eq!((menu.left_family, menu.left_tier), (1, 2));
    assert_eq!((menu.right_family, menu.right_tier), (1, 1));
    assert_eq!(menu.family_selected_tier[1], 1);

    assert_raw_tier_and_family_minus_one(handle, &mut snapshot);
    assert_spell_menu_cast_routes(handle, &mut snapshot);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn castle_holder_reset_is_not_overwritten_by_player_turn() {
    let handle = open_fixture();
    let idle = valid_command();
    let mut snapshot = McSessionSnapshotV2::default();
    for position in [
        Position { x: 29_440, y: 54_272, z: 0 },
        Position { x: 36_000, y: 54_272, z: 4_896 },
        Position { x: 49_664, y: 54_528, z: 0 },
    ] {
        with_session_mut(handle, |session| {
            session.state.position = position;
            session.state.actual_speed = 0;
            Ok(())
        }).unwrap();
        for _ in 0..16 {
            assert_eq!(mc_native_session_step(handle, &raw const idle,
                &raw mut snapshot, struct_size::<McSessionSnapshotV2>()), MC_STATUS_OK);
        }
    }
    with_session_mut(handle, |session| {
        assert!(session.mission.has_castle_spell, "ordinary pickup route");
        session.mission.begin_source_turn();
        let slot = session.live_world.goats().iter()
            .find(|goat| goat.entity_index < session.state.entity_index && goat.active)
            .expect("lower source-slot creature").entity_index;
        assert!(session.mission.disable_imported_source_slot(slot));
        session.mission.begin_source_turn();
        session.state.position = Position { x: 42_213, y: 56_833, z: 256 };
        session.mission.player_mana = 1_000;
        assert!(session.mission.cast_initial_castle(&session.terrain, &session.state, 0));
        let projectile = session.mission.projectiles().iter().find_map(|p| {
            if let Level1Projectile::CastleBuild(p) = p { Some(p) } else { None }
        }).unwrap();
        assert!(projectile.source_entity_index < session.state.entity_index,
            "projectile {} must end before player {} captures holder state",
            projectile.source_entity_index, session.state.entity_index);
        Ok(())
    }).unwrap();
    for _ in 0..120 {
        assert_eq!(mc_native_session_step(handle, &raw const idle,
            &raw mut snapshot, struct_size::<McSessionSnapshotV2>()), MC_STATUS_OK);
    }
    with_session(handle, |session| {
        assert!(!session.mission.castle_exists, "blocked placement must reject");
        assert!(!session.mission.projectiles().iter().any(|p| matches!(p, Level1Projectile::CastleBuild(_))));
        assert_eq!(session.mission.castle_holder_active_value(), 0);
        Ok(())
    }).unwrap();
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn selected_fireball_retriggers_through_the_public_command_path() {
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2::default();
    let left_fire = McCommandV2 {
        flags: 0x10,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const left_fire,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let fireball_launch_count = |session: &SessionRecord| {
        session
            .mission
            .projectiles()
            .iter()
            .filter(|projectile| matches!(projectile, Level1Projectile::PlayerFireball(_)))
            .count()
            + session
                .mission
                .impacts()
                .iter()
                .filter(|impact| impact.model == FIREBALL_IMPACT_MODEL)
                .count()
    };
    let first_count = with_session(handle, |session| Ok(fireball_launch_count(session))).unwrap();
    assert_eq!(first_count, 1);

    let selected_fire = McCommandV2 {
        flags: 0x40,
        requested_spell: 0,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const selected_fire,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let second_count = with_session(handle, |session| Ok(fireball_launch_count(session))).unwrap();
    assert_eq!(second_count, 2);
    assert_eq!(snapshot.selected_spell_ui_index, 0);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn right_hand_possession_countdown_retriggers_through_the_public_command_path() {
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2::default();
    let right_possession = McCommandV2 {
        flags: 0x20,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const right_possession,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let holder_active = |expected| {
        with_session(handle, |session| {
            let holder = session
                .context
                .spell_fire
                .as_ref()
                .and_then(|fire| {
                    fire.holders
                        .iter()
                        .find(|holder| holder.entity_index == 154)
                })
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "Possession holder 154 is absent",
                    )
                })?;
            Ok(holder.active_value == expected)
        })
        .unwrap()
    };
    assert!(
        holder_active(2),
        "sub_69640 consumes the activation turn from 3 to 2"
    );

    let idle = valid_command();
    for expected in [1, 0] {
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(holder_active(expected));
    }

    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const right_possession,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert!(
        holder_active(2),
        "the second right-hand cast starts and consumes a fresh activation"
    );

    let query = McEventQueryV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEventQueryV2>(),
        generation: snapshot.generation,
        cursor: 0,
        reserved: [0; 4],
    };
    let mut events = vec![McEventV2::default(); 256];
    let mut batch = McEventBatchV2::default();
    assert_eq!(
        mc_native_events_copy_since(
            handle,
            &raw const query,
            events.as_mut_ptr(),
            u32::try_from(events.len()).unwrap(),
            struct_size::<McEventV2>(),
            &raw mut batch,
            struct_size::<McEventBatchV2>(),
        ),
        MC_STATUS_OK
    );
    events.truncate(usize::try_from(batch.written).unwrap());
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == MC_EVENT_LEVEL1_POSSESSION_SPAWNED)
            .count(),
        2,
        "both public right-hand casts reach the Possession projectile factory"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn real_level_one_right_hand_possession_claims_and_composites_a_building() {
    let handle = open_fixture_with_owner_cache_and_terrain_frame_assets();
    let target = with_session_mut(handle, |session| {
        session.mission.possession_experience = 600;
        session.mission.possession_tier = 1;
        let fire = session
            .context
            .spell_fire
            .as_mut()
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "spell state absent"))?;
        fire.spell_levels[1] = 1;
        fire.family_selected_tiers[1] = 1;
        fire.holders
            .iter_mut()
            .find(|holder| holder.entity_index == 154)
            .ok_or_else(|| AbiError::new(MC_STATUS_VALIDATION_FAILED, "possession holder absent"))?
            .level_gate = 1;
        let target = *session
            .world
            .level1_mission()
            .possession_targets()
            .first()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "qualified Level 1 has no possession target",
                )
            })?;
        session.state.position = Position {
            x: target.position.x,
            y: target.position.y.wrapping_add(384),
            z: target.position.z.wrapping_sub(50),
        };
        session.state.yaw = 0;
        session.state.pitch_accumulator = 0;
        session.state.effective_pitch = 0;
        session.state.target_speed = 0;
        session.state.actual_speed = 0;
        session.state.strafe_speed = 0;
        session.camera_history = CameraHistory::capture(
            &session.state,
            session.source_turn,
            session.camera_fov,
            0,
            0,
        );
        Ok(target)
    })
    .unwrap();

    let mut snapshot = McSessionSnapshotV2::default();
    let right_possession = McCommandV2 {
        flags: 0x20,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const right_possession,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    let idle = valid_command();
    let mut mission = McLevel1MissionSnapshotV2::default();
    for _ in 0..12 {
        assert_eq!(
            mc_native_level1_mission_snapshot(
                handle,
                &raw mut mission,
                struct_size::<McLevel1MissionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        if mission.possessed_building_count != 0 {
            break;
        }
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    assert_eq!(mission.possessed_building_count, 1);
    assert_eq!(mission.last_possessed_building_entity, target.entity_index);
    assert_eq!(mission.last_possessed_building_owner, snapshot.entity_index);
    assert_eq!(mission.possession_experience, 601);
    assert_eq!(mission.possession_tier, 1);
    with_session(handle, |session| {
        let inputs = level1_minimap_entity_inputs(session);
        let owned = inputs[usize::from(target.entity_index)];
        assert_eq!(owned.player_link, snapshot.entity_index);
        assert!(owned.player_link_is_local_player);
        let command = level1_minimap_entity_command(&session.state, owned).unwrap();
        assert!(matches!(command.palette_index, 164 | 170));
        let mut unowned = owned;
        unowned.player_link = 0;
        unowned.player_link_is_player = false;
        unowned.player_link_is_local_player = false;
        assert_eq!(
            level1_minimap_entity_command(&session.state, unowned).unwrap().palette_index,
            215
        );
        Ok(())
    }).unwrap();
    let mut impacts =
        vec![McFireballImpactSnapshotV2::default(); usize::try_from(mission.impact_count).unwrap()];
    let mut impact_written = 0;
    assert_eq!(
        mc_native_fireball_impacts_copy(
            handle,
            mission.generation,
            mission.tick,
            0,
            impacts.as_mut_ptr(),
            mission.impact_count,
            struct_size::<McFireballImpactSnapshotV2>(),
            &raw mut impact_written,
        ),
        MC_STATUS_OK
    );
    assert_eq!(impact_written, mission.impact_count);
    let attraction = impacts
        .iter()
        .find(|impact| impact.model == POSSESSION_ATTRACTION_MODEL)
        .expect("tier-one production collision retains its model-54 gameplay row");
    assert_eq!(
        (
            attraction.class_id,
            attraction.render_parameter_index,
            attraction.base_tmap,
            attraction.render_mode,
            attraction.logical_height,
            attraction.animation_frame,
            attraction.authority,
            attraction.active,
        ),
        (
            FIREBALL_IMPACT_CLASS,
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_ATTRACTION_BASE_TMAP,
            POSSESSION_ATTRACTION_RENDER_MODE,
            POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            0,
            MC_AUTHORITY_LEVEL1_MISSION_LIVE,
            1,
        )
    );
    with_session(handle, |session| {
        let house = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == target.entity_index)
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "possessed house is absent from the canonical house family",
                )
            })?;
        assert_eq!(house.owner_entity_index, snapshot.entity_index);
        assert_eq!(house.pending_possession_owner, 0);
        assert_eq!(house.pending_possession_payload, 0);
        assert!(
            !session
                .renderable_map_chains
                .tracks(u16::try_from(attraction.reserved[0]).unwrap()),
            "model 54 remains in gameplay snapshots without an AddEventToMap chain"
        );
        Ok(())
    })
    .unwrap();

    // Let the temporary model-12 impact finish so any combat raster writes
    // below can only come from the persistent parameter-177 building.
    for _ in 0..12 {
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const idle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
    }
    let writes = with_session_mut(handle, |session| {
        assert!(
            session
                .mission
                .projectiles()
                .iter()
                .all(|projectile| !projectile.active())
        );
        assert!(
            session
                .mission
                .impacts()
                .iter()
                .filter(|impact| impact.model != POSSESSION_ATTRACTION_MODEL)
                .all(|impact| !impact.active)
        );
        assert!(
            session
                .mission
                .impacts()
                .iter()
                .any(|impact| { impact.model == POSSESSION_ATTRACTION_MODEL && impact.active }),
            "the invisible model-54 gameplay auxiliary outlives its short model-12 sibling"
        );
        let building = session
            .mission
            .possessed_buildings()
            .iter()
            .find(|building| building.entity_index == target.entity_index)
            .copied()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "possessed Level 1 building disappeared",
                )
            })?;
        assert_eq!(building.owner_entity_index, snapshot.entity_index);
        assert_eq!(building.render_parameter_index, 177);
        assert_eq!(building.base_tmap, 96);
        assert_eq!(building.render_mode, 0);
        assert_eq!(building.logical_height, 400);

        session.state.position = Position {
            x: target.position.x,
            y: target.position.y.wrapping_add(1_000),
            z: target.position.z,
        };
        session.state.yaw = 0;
        session.state.pitch_accumulator = 0;
        session.state.effective_pitch = 0;
        session.camera_history = CameraHistory::capture(
            &session.state,
            session.source_turn,
            session.camera_fov,
            0,
            0,
        );
        session
            .sync_supported_renderable_records()
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let frame = session.world_frame()?;
        Ok(frame.combat_raster_stats.framebuffer_writes)
    })
    .unwrap();
    assert!(
        writes > 0,
        "the persistent remc2 parameter-177/TMAP-96 building reaches the indexed compositor"
    );
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn requested_spell_without_cast_flag_preserves_selection_and_spawns_no_projectile() {
    let handle = open_fixture();
    let mut snapshot = McSessionSnapshotV2::default();
    let select_possession = McCommandV2 {
        flags: 0,
        requested_spell: 1,
        ..valid_command()
    };
    assert_eq!(
        mc_native_session_step(
            handle,
            &raw const select_possession,
            &raw mut snapshot,
            struct_size::<McSessionSnapshotV2>(),
        ),
        MC_STATUS_OK
    );
    assert_eq!(snapshot.selected_spell_ui_index, 0);
    assert_eq!(snapshot.selected_spell_holder, 153);
    let projectile_count =
        with_session(handle, |session| Ok(session.mission.projectiles().len())).unwrap();
    assert_eq!(projectile_count, 0);
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn ability_improved_event_encodes_the_exact_notification_contract() {
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        let source_family = 16;
        let event = session.encode_mission_event(
            Level1MissionEvent::AbilityImproved {
                source_family,
                notification_template_selector: ABILITY_IMPROVED_TEMPLATE_SELECTOR,
                ability_name_selector: ABILITY_IMPROVED_NAME_SELECTOR_BASE
                    + u16::from(source_family),
                notification_state: ABILITY_IMPROVED_NOTIFICATION_STATE,
                notification_timer: ABILITY_IMPROVED_NOTIFICATION_TIMER,
            },
            state_hash(&session.state),
        );
        assert_eq!(event.kind, MC_EVENT_LEVEL1_ABILITY_IMPROVED);
        assert_eq!(event.code, u32::from(source_family));
        assert_eq!(
            event.entity_index,
            ABILITY_IMPROVED_NAME_SELECTOR_BASE + u16::from(source_family)
        );
        assert_eq!(
            (event.value_0, event.value_1),
            (
                i32::from(ABILITY_IMPROVED_NOTIFICATION_STATE),
                i32::from(ABILITY_IMPROVED_NOTIFICATION_TIMER),
            )
        );
        assert_eq!(
            event.reserved,
            [u32::from(ABILITY_IMPROVED_TEMPLATE_SELECTOR), 0, 0, 0]
        );
        Ok(())
    })
    .expect("ability-improved C ABI event");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn selected_castle_sound_event_and_sidecar_share_the_owner_asset_reference() {
    let handle = open_fixture_with_owner_cache();
    with_session_mut(handle, |session| {
        let position = session.state.position;
        let source_entity_index = session.state.entity_index;
        let random_state = session.mission.global_rng();
        // The projectile may already be gone when the end-of-turn event
        // journal is encoded (for example, an immediate blocked
        // placement). The spawn event must therefore carry its complete
        // audio source rather than looking the projectile up again.
        assert!(session.mission.projectiles().is_empty());
        let event = session.encode_mission_event(
            Level1MissionEvent::CastleProjectileSpawned {
                projectile_id: 1,
                sound_id: CASTLE_PROJECTILE_SOUND,
                source_entity_index,
                position,
                random_state,
            },
            state_hash(&session.state),
        );
        let expected_asset_ref = session
            .world
            .sound_asset_ref(CASTLE_PROJECTILE_SOUND)
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "Castle sound has no owner asset reference",
                )
            })?;
        assert_eq!(event.code, u32::from(CASTLE_PROJECTILE_SOUND));
        assert_eq!(expected_asset_ref, 86);
        assert_eq!(event.asset_ref_index, expected_asset_ref);
        let command = session
            .audio_commands
            .get(&event.sequence)
            .copied()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "Castle sound event has no audio sidecar",
                )
            })?;
        assert_eq!(command.sound_id, CASTLE_PROJECTILE_SOUND);
        assert_eq!(command.asset_ref_index, expected_asset_ref);
        assert_eq!(command.event_sequence, event.sequence);
        Ok(())
    })
    .expect("selected Castle audio contract");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}

#[test]
fn water_splash_event_preserves_impact_identity_after_audio_sidecar_attachment() {
    let handle = open_fixture();
    with_session_mut(handle, |session| {
        let splash_id = 1_234_u32;
        let position = session.state.position;
        let event = session.encode_mission_event(
            Level1MissionEvent::WaterSplashed {
                splash_id,
                projectile_id: 77,
                position,
                sound_id: WATER_SPLASH_SOUND,
            },
            state_hash(&session.state),
        );
        assert_eq!(event.kind, MC_EVENT_LEVEL1_WATER_SPLASHED);
        assert_eq!(event.code, u32::from(WATER_SPLASH_SOUND));
        assert_eq!(event.entity_index, splash_id as u16);
        assert_eq!(event.value_0, 77);
        assert_eq!(event.value_1, i32::from(position.z));
        let command = session
            .audio_commands
            .get(&event.sequence)
            .copied()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "water-splash event has no audio sidecar",
                )
            })?;
        assert_eq!(command.sound_id, WATER_SPLASH_SOUND);
        assert_eq!(command.event_sequence, event.sequence);
        assert_eq!(command.entity_index, event.entity_index);
        assert_eq!(command.generation, event.generation);
        assert_eq!(command.state_hash, event.state_hash);
        assert_eq!(command.asset_ref_index, event.asset_ref_index);
        assert_eq!(command.player_index, event.player_index);
        let overflow_event = session.encode_mission_event(
            Level1MissionEvent::WaterSplashed {
                splash_id: u32::from(u16::MAX) + 1,
                projectile_id: 78,
                position,
                sound_id: WATER_SPLASH_SOUND,
            },
            state_hash(&session.state),
        );
        assert_eq!(overflow_event.entity_index, u16::MAX);
        assert_eq!(
            session.audio_commands[&overflow_event.sequence].entity_index,
            overflow_event.entity_index
        );
        Ok(())
    })
    .expect("water-splash C ABI event");
    assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
}
