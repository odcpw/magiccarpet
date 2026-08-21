    #[test]
    fn level_two_castle_defender_cadence_keeps_fixed_source_v21_after_dead_slot() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("fixed-v21 defender cadence session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("fixed-v21 defender cadence Castle");
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 4; // sub_60400 returns fixed v21 = 2.
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 78_600;
        }
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let (dead_slot, live_slot) = {
            let castle = session.economy().castle().expect("Castle");
            (castle.defender_slots[0], castle.defender_slots[1])
        };
        assert_ne!(dead_slot, 0);
        assert_ne!(live_slot, 0);
        let sentinel_target = 0xdead;
        {
            let economy = session.economy_mut();
            economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == dead_slot)
                .expect("slot-0 defender")
                .life = -1;
            let live = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == live_slot)
                .expect("slot-1 defender");
            live.source_byte_3e = 1;
            live.target_sphere_slot = sentinel_target;
        }

        // The dead source slot is cleared first, but v21 remains two. A
        // compact defenders.len()==1 loop would turn byte_0x3E==1 into a
        // modulo-one hit and overwrite the live slot's sentinel target.
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let live = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == live_slot)
            .copied()
            .expect("live slot-1 defender remains indexed");
        assert_eq!(live.source_byte_3e, 1);
        assert_eq!(live.target_sphere_slot, sentinel_target);
        let castle = session.economy().castle().expect("Castle");
        assert_eq!(castle.defender_slots, [0, live_slot, 0]);
        assert_eq!(castle.defender_count, 1);
    }

    #[test]
    fn level_two_castle_outer_turn_sub5ff50_transforms_dead_excess_and_reclaims_following_pass() {
        let package = level_two_fixture_path();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let baseline_hash = session.economy().reset_hash();
        let _castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("sub_5FF50 Castle");
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 5_000;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let defender_slot = session.economy().castle().expect("Castle").defender_slots[0];
        assert_ne!(defender_slot, 0);
        let dead_position = Position {
            x: 4_096,
            y: 0,
            z: 0,
        };
        let dead_rng = {
            let economy = session.economy_mut();
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("live Castle defender");
            defender.life = -1;
            defender.mana = 321;
            defender.position = dead_position;
            defender.target_sphere_slot = 0;
            defender.entity_rng
        };
        session.economy_mut().castle.as_mut().expect("Castle").phase = 0;
        let global_before_death = session.economy().global_rng;
        let free_before_death = session.economy().free_slot_count();

        // The root's ordinary action-4 callback reaches sub_5FF50. A valid
        // dead holder becomes one normal TransformEntityToManaSphere(false)
        // sphere, clears its raw relation, and remains hidden but allocated
        // for the rest of this source pass.
        session.step_outer_turn(PlayerCommand::default(), 0);
        let hidden = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .copied()
            .expect("dead defender remains until following source boundary");
        assert!(hidden.active);
        assert_ne!(hidden.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden.player_entity_index, 0);
        assert_eq!(hidden.entity_rng, level2_next_rng16(dead_rng));
        assert!(!session.economy().free_slots.contains(&defender_slot));
        assert!(session.economy().map_chains().is_linked(defender_slot));
        assert_eq!(
            session.economy().free_slot_count(),
            free_before_death.saturating_sub(1),
            "the transformed sphere consumes one slot while the hidden holder remains allocated"
        );
        assert_eq!(session.economy().source_owner(defender_slot), 0);
        assert_eq!(
            session.economy().castle().expect("Castle").defender_slots[0],
            0
        );
        let transformed = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| {
                sphere.active
                    && sphere.source_entity_index != defender_slot
                    && sphere.owner_entity_index == 111
                    && sphere.mana == 321
            })
            .copied()
            .expect("dead defender mana sphere");
        let global_after_death = level2_next_rng32(global_before_death);
        let sphere_seed = transformed
            .source_entity_index
            .wrapping_add(global_after_death as u16);
        assert_eq!(
            transformed.entity_rng,
            level2_next_rng16(level2_next_rng16(sphere_seed)),
            "TransformEntityToManaSphere keeps its death-path two-draw sphere RNG"
        );
        assert!(session.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    mana: 321,
                    ..
                } if *source_entity_index == defender_slot
            )
        }));

        // Force the next ordinary root visit to the stage-0 target so the
        // following source-boundary reclaim is observed without immediately
        // allocating a replacement holder into the just-freed slot.
        session.economy_mut().castle.as_mut().expect("Castle").level = 0;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != defender_slot)
        );
        assert!(session.economy().free_slots.contains(&defender_slot));
        assert!(!session.economy().map_chains().is_linked(defender_slot));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // A zero-mana holder takes the same valid dead-holder branch, but the
        // pinned TransformEntityToManaSphere(false) returns before its raw
        // player relation write. The relation therefore remains nonzero for
        // the hidden interval; exact typed/owner/id identity, rather than a
        // cleared relation, drives the following source-boundary reclaim.
        let mut zero_mana = inspection
            .level2_reset_session()
            .expect("qualified zero-mana Level 2 reset session");
        let zero_castle_slot = zero_mana
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("zero-mana Castle");
        {
            let castle = zero_mana.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 0;
            castle.mana_capacity = 5_000;
        }
        zero_mana.select_level2_campaign_entry_stage();
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        let zero_slot = zero_mana
            .economy()
            .castle()
            .expect("zero-mana Castle")
            .defender_slots[0];
        let (zero_rng, zero_player_relation, zero_free_slots, zero_event_count) = {
            let economy = zero_mana.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == zero_slot)
                .expect("zero-mana defender");
            child.life = -1;
            child.mana = 0;
            child.position = Position {
                x: 4_096,
                y: 0,
                z: 0,
            };
            child.target_sphere_slot = 0;
            (
                child.entity_rng,
                child.player_entity_index,
                economy.free_slot_count(),
                economy.events.len(),
            )
        };
        assert_ne!(zero_player_relation, 0);
        zero_mana
            .economy_mut()
            .castle
            .as_mut()
            .expect("zero-mana Castle")
            .phase = 0;
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        let hidden_zero = zero_mana
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == zero_slot)
            .copied()
            .expect("zero-mana holder remains hidden until following source boundary");
        assert!(hidden_zero.active);
        assert_ne!(hidden_zero.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden_zero.player_entity_index, zero_player_relation);
        assert_eq!(hidden_zero.entity_rng, zero_rng);
        assert_eq!(zero_mana.economy().free_slot_count(), zero_free_slots);
        assert!(zero_mana.economy().map_chains().is_linked(zero_slot));
        assert_eq!(
            zero_mana
                .economy()
                .castle()
                .expect("zero-mana Castle")
                .defender_slots[0],
            0
        );
        assert!(
            zero_mana.economy().events()[zero_event_count..]
                .iter()
                .all(|event| {
                    !matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned {
                            source_entity_index,
                            ..
                        } if *source_entity_index == zero_slot
                    )
                })
        );

        zero_mana
            .economy_mut()
            .castle
            .as_mut()
            .expect("zero-mana Castle")
            .level = 0;
        zero_mana.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            zero_mana
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != zero_slot)
        );
        assert!(zero_mana.economy().free_slots.contains(&zero_slot));
        assert!(!zero_mana.economy().map_chains().is_linked(zero_slot));
        assert!(zero_mana.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == zero_castle_slot
        )));

        // The unused relation tail takes the same valid-holder transform
        // branch. Build two source defenders through the ordinary root route,
        // then lower the stage so the second relation becomes excess.
        let mut excess = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let excess_castle_slot = excess
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("excess-holder Castle");
        {
            let castle = excess.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 4;
            castle.phase = 0;
        }
        excess.select_level2_campaign_entry_stage();
        excess.step_outer_turn(PlayerCommand::default(), 0);
        let (primary_excess_slot, excess_slot) = {
            let castle = excess.economy().castle().expect("excess-holder Castle");
            (castle.defender_slots[0], castle.defender_slots[1])
        };
        assert_ne!(primary_excess_slot, 0);
        assert_ne!(excess_slot, 0);
        {
            let economy = excess.economy_mut();
            let castle = economy.castle.as_mut().expect("excess-holder Castle");
            castle.level = 1;
            castle.phase = 0;
            let holder = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == excess_slot)
                .expect("valid excess holder");
            holder.mana = 222;
            holder.position = Position {
                x: 4_096,
                y: 0,
                z: 0,
            };
            holder.target_sphere_slot = 0;
        }
        excess.step_outer_turn(PlayerCommand::default(), 0);
        let hidden_excess = excess
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == excess_slot)
            .copied()
            .expect("excess holder remains until following source boundary");
        assert!(hidden_excess.active);
        assert_ne!(hidden_excess.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert_eq!(hidden_excess.player_entity_index, 0);
        assert!(excess.economy().map_chains().is_linked(excess_slot));
        assert_eq!(
            excess
                .economy()
                .castle()
                .expect("excess-holder Castle")
                .defender_slots,
            [primary_excess_slot, 0, 0]
        );
        assert!(excess.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    mana: 222,
                    ..
                } if *source_entity_index == excess_slot
            )
        }));
        excess
            .economy_mut()
            .castle
            .as_mut()
            .expect("excess-holder Castle")
            .level = 0;
        excess.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            excess
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != excess_slot)
        );
        assert!(excess.economy().free_slots.contains(&excess_slot));
        assert!(excess.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == excess_castle_slot
        )));

        // A wrong relation must not release a live non-defender row. The
        // relation slot is authoritative for selection, but the class/model/
        // action predicate is authoritative for the Transform branch.
        let mut wrong = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let wrong_castle_slot = wrong
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("wrong-relation Castle");
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("wrong-relation Castle")
            .level = 1;
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("wrong-relation Castle")
            .phase = 0;
        wrong.select_level2_campaign_entry_stage();
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        let wrong_slot = wrong
            .economy()
            .castle()
            .expect("wrong-relation Castle")
            .defender_slots[0];
        {
            let economy = wrong.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == wrong_slot)
                .expect("wrong-relation holder");
            child.source_class = 10;
            child.model = 42;
            child.action = 44;
            economy
                .castle
                .as_mut()
                .expect("wrong-relation Castle")
                .phase = 0;
        }
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        let wrong_child = wrong
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == wrong_slot)
            .copied()
            .expect("wrongly related live row is preserved");
        assert!(wrong_child.active);
        assert_eq!(
            (
                wrong_child.source_class,
                wrong_child.model,
                wrong_child.action
            ),
            (10, 42, 44)
        );
        assert_ne!(
            wrong
                .economy()
                .castle()
                .expect("wrong-relation Castle")
                .defender_slots[0],
            wrong_slot
        );
        assert!(!wrong.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    ..
                } if *source_entity_index == wrong_slot
            )
        }));

        // A stale slot is cleared/replaced without dereferencing or freeing
        // an unrelated live row.
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("stale-relation Castle")
            .defender_slots[0] = 0xdead;
        wrong
            .economy_mut()
            .castle
            .as_mut()
            .expect("stale-relation Castle")
            .phase = 0;
        wrong.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !wrong
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.source_entity_index == 0xdead)
        );
        assert!(
            wrong
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.source_entity_index == wrong_slot && child.active)
        );
        assert!(wrong.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == wrong_castle_slot
        )));
    }

    #[test]
    fn level_two_castle_defender_addballon_class3_threshold_terrain_and_life() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let setup = || {
            let mut session = inspection
                .level2_reset_session()
                .expect("Castle defender AddBallon session");
            let baseline_hash = session.economy().reset_hash();
            let castle_position = Position::default();
            let castle_slot = session
                .economy_mut()
                .build_castle(castle_position, 111)
                .expect("Castle defender AddBallon Castle");
            session
                .economy_mut()
                .castle
                .as_mut()
                .expect("Castle defender AddBallon Castle")
                .level = 1;
            session.economy_mut().maintain_castle_defenders(castle_slot);
            let defender_slot = session
                .economy()
                .castle()
                .expect("Castle defender AddBallon Castle")
                .defender_slots[0];
            let holder_position = Position {
                x: castle_position.x.wrapping_add(16),
                y: castle_position.y,
                z: 0,
            };
            let holder_ground = session.economy().terrain.ground_height(holder_position);
            (
                session,
                baseline_hash,
                castle_slot,
                defender_slot,
                castle_position,
                holder_position,
                holder_ground,
            )
        };

        // The source distance helper is XY-only. A holder below the local
        // terrain plane still transfers at exactly speed*target-dword10.
        let (
            mut session,
            baseline_hash,
            castle_slot,
            defender_slot,
            castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.z = holder_ground.saturating_sub(100);
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        let map_before = session.economy().map_chains().linked_entity_indices();
        let free_before_transfer = session.economy().free_slot_count();
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let castle = session.economy().castle().expect("Castle target");
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(castle.position, castle_position);
        assert_eq!(castle.mana, 7);
        assert_eq!(
            (child.position.x, child.position.y),
            (castle_position.x, castle_position.y)
        );
        assert_eq!(child.mana, 0);
        assert_eq!(child.player_entity_index, child.id_entity_index);
        assert_eq!(child.life, child.max_life);
        assert_eq!(
            session
                .economy()
                .events()
                .iter()
                .filter(|event| matches!(event, Level2EconomyEvent::CastleDefenderArrived { .. }))
                .count(),
            1
        );
        assert_eq!(
            session.economy().free_slot_count(),
            free_before_transfer,
            "the typed transfer does not allocate or reclaim a source row"
        );
        assert!(map_before.iter().all(|slot| {
            session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(slot)
        }));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // One unit beyond the pinned source speed range moves the holder and
        // leaves mana/life untouched.
        let (
            mut session,
            _baseline_hash,
            castle_slot,
            defender_slot,
            castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.x = holder_position.x.wrapping_add(49);
        holder_position.z = holder_ground;
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(child.mana, 7);
        assert_eq!(child.life, 1);
        assert_ne!(child.position.x, holder_position.x);
        assert_eq!(
            session.economy().castle().expect("Castle target").position,
            castle_position
        );

        // At the same XY threshold, a holder above the source's +512 terrain
        // allowance fails the exact altitude gate and does not transfer.
        let (
            mut session,
            _baseline_hash,
            castle_slot,
            defender_slot,
            _castle_position,
            mut holder_position,
            holder_ground,
        ) = setup();
        holder_position.z = holder_ground.saturating_add(513);
        {
            let economy = session.economy_mut();
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            child.position = holder_position;
            child.target_sphere_slot = castle_slot;
            child.mana = 7;
            child.life = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("model-3 defender");
        assert_eq!(child.mana, 7);
        assert_eq!(child.life, 1);
        assert_eq!(child.position.x, holder_position.x);
        assert_eq!(child.position.z, holder_ground.saturating_add(509));
    }

    #[test]
    fn level_two_castle_defender_sphere_target_consumes_through_outer_dispatch() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let setup = || {
            let mut session = inspection
                .level2_reset_session()
                .expect("Castle defender sphere session");
            let baseline_hash = session.economy().reset_hash();
            let castle_slot = session
                .economy_mut()
                .build_castle(Position::default(), 111)
                .expect("Castle defender sphere Castle");
            session
                .economy_mut()
                .castle
                .as_mut()
                .expect("Castle defender sphere Castle")
                .level = 1;
            session.economy_mut().maintain_castle_defenders(castle_slot);
            let defender_slot = session
                .economy()
                .castle()
                .expect("Castle defender sphere Castle")
                .defender_slots[0];
            let holder_position = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Castle defender sphere holder")
                .position;
            let holder = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Castle defender sphere holder");
            assert_eq!(
                (holder.max_life, holder.max_mana, holder.speed),
                (
                    LEVEL2_CASTLE_DEFENDER_MAX_LIFE,
                    LEVEL2_CASTLE_DEFENDER_MAX_MANA,
                    LEVEL2_CASTLE_DEFENDER_SPEED
                )
            );
            (
                session,
                baseline_hash,
                castle_slot,
                defender_slot,
                holder_position,
            )
        };

        // Positive production route: source arbitration installs the sphere
        // target, then one outer turn visits the sphere and holder in source
        // order.  The holder receives the sphere payload and the sphere is
        // disabled/unlinked before the retained arena is reclaimed.
        let (mut session, baseline_hash, castle_slot, defender_slot, holder_position) = setup();
        let target_position = Position {
            x: holder_position.x.wrapping_add(100),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, target_position, 0, 0x1234, 321, 111)
            .expect("defender target sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .velocity_x = 0;
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .velocity_y = 0;
        session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("defender target sphere")
            .vertical_velocity = 0;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("defender holder")
                .target_sphere_slot,
            sphere_slot
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("live defender holder");
        assert_eq!(child.mana, 321);
        assert_eq!(child.player_entity_index, child.id_entity_index);
        assert_eq!(child.target_sphere_slot, 0);
        assert_ne!(child.position.x, target_position.x);
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .all(|sphere| sphere.source_entity_index != sphere_slot)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&sphere_slot)
        );
        let economy_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Economy {
                        event: Level2EconomyEvent::CastleDefenderCollectedSphere {
                            defender_slot: event_defender,
                            sphere_id: event_sphere,
                            mana,
                        },
                        ..
                    } if *event_defender == defender_slot
                        && *event_sphere == sphere_id
                        && *mana == 321
                )
            })
            .expect("defender sphere economy dispatch");
        let defender_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot: event_slot,
                        kind: Level2DynamicArenaKind::CastleDefender,
                        ..
                    } if *event_slot == defender_slot
                )
            })
            .expect("defender dynamic dispatch");
        assert!(economy_index < defender_index);
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // Wrong player relation is rejected even when the target slot is
        // installed manually, matching AddBallon's class-10 owner gate.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let wrong_owner_position = Position {
            x: holder_position.x.wrapping_add(64),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, wrong_owner_position, 0, 0x1234, 321, 222)
            .expect("wrong-owner sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder")
            .target_sphere_slot = sphere_slot;
        let wrong_owner_before = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder")
            .position;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let wrong_owner_after = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("wrong-owner holder");
        assert_eq!(wrong_owner_after.mana, 0);
        assert_eq!(wrong_owner_after.position.x, wrong_owner_before.x);
        assert_eq!(wrong_owner_after.position.y, wrong_owner_before.y);
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot && sphere.active)
        );
        // The Day attraction threshold is strict on the positive side: a
        // sphere at 1025 source units is not consumed and clears byte-0 bit
        // 0x40 before the holder moves toward it.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let far_position = Position {
            x: holder_position.x.wrapping_add(1_025),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, far_position, 0, 0x1234, 321, 111)
            .expect("far sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("far holder")
            .target_sphere_slot = sphere_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let sphere = session
            .economy()
            .spheres()
            .iter()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("far sphere remains");
        assert!(sphere.active);
        assert_eq!(sphere.control_flags & 0x40, 0);

        // `sub_106C0` is strict.  Place a size-0 sphere exactly at the
        // defender's X boundary (13 + 272); equality must not consume it.
        let (mut session, _, _, defender_slot, holder_position) = setup();
        let boundary_position = Position {
            x: holder_position.x.wrapping_add(13 + 272),
            ..holder_position
        };
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, boundary_position, 0, 0x1234, 1, 111)
            .expect("boundary sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("boundary holder")
            .target_sphere_slot = sphere_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot && sphere.active)
        );
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("boundary holder")
                .mana,
            0
        );

        // The source stores Z as signed 16-bit coordinates before the AABB
        // arithmetic.  Equal high-bit coordinates must therefore retain the
        // same overlap result as zero-based coordinates (a raw-u16 compare
        // would incorrectly see a ~65k-unit separation).
        let (mut session, _, _, _defender_slot, holder_position) = setup();
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(77, holder_position, 0, 0x1234, 1, 111)
            .expect("signed-Z sphere");
        let sphere_slot = u16::try_from(sphere_id).expect("sphere slot fits");
        let sphere = session
            .economy_mut()
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == sphere_slot)
            .expect("signed-Z sphere");
        sphere.position.z = i16::MIN;
        let defender_position = Position {
            z: i16::MIN,
            ..holder_position
        };
        assert!(level2_castle_defender_sphere_overlap(
            defender_position,
            sphere
        ));
    }

    #[test]
    fn level_two_castle_defender_cave_collision_matches_source_control_tail() {
        // The qualified Level 2 package is Day data; this deliberately
        // synthetic Cave-route test exercises the production defender method
        // after swapping only the terrain mode/planes. It is route coverage,
        // not owner-data Cave qualification.
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Castle defender Cave session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("Cave defender Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("Cave defender Castle")
            .level = 1;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let defender_slot = session
            .economy()
            .castle()
            .expect("Cave defender Castle")
            .defender_slots[0];
        let (holder_position, ground) = {
            let child = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Cave defender holder");
            (
                child.position,
                session.economy().terrain.ground_height(child.position),
            )
        };
        let tile = crate::Tile {
            x: holder_position.x.to_be_bytes()[0],
            y: holder_position.y.to_be_bytes()[0],
        };
        let terrain_index = terrain_index(tile.x, tile.y);

        // Angle bit 8 blocks the holder. The source toggles bit 0, selects
        // speed 48, emits sound 22 on the uncooldown transition, and applies
        // sub_580E0's ground+512 floor after the -16 step.
        {
            let economy = session.economy_mut();
            economy.terrain.map_type = MapType::Cave;
            economy.terrain.planes.angles.fill(0);
            economy.terrain.planes.secondary_heights.fill(255);
            economy.terrain.planes.angles[terrain_index] = 8;
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("Cave defender holder");
            child.target_sphere_slot = 0;
            child.position = Position {
                z: ground.saturating_add(520),
                ..holder_position
            };
            child.control_flags = 0;
            child.cave_collision_cooldown = 0;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("blocked Cave defender holder");
        assert_ne!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED);
        assert_eq!(
            child.position.z,
            ground + LEVEL2_CASTLE_DEFENDER_CAVE_FLOOR_CLEARANCE
        );
        assert_eq!(
            child.cave_collision_cooldown,
            LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN
        );
        assert!(session.economy().events().iter().any(|event| matches!(
            event,
            Level2EconomyEvent::CastleDefenderCaveSound {
                defender_slot: event_slot,
                sound_id: LEVEL2_CASTLE_DEFENDER_CAVE_SOUND,
            } if *event_slot == defender_slot
        )));

        // Clearing the angle and using a high ceiling opens the route. A
        // cooldown of one decrements to zero before the transition, so the
        // source emits the same sound and reloads 32 exactly once.
        session.economy_mut().events.clear();
        {
            let economy = session.economy_mut();
            economy.terrain.planes.angles[terrain_index] = 0;
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("open Cave defender holder");
            child.position.z = ground.saturating_add(700);
            child.control_flags = 1;
            child.cave_collision_cooldown = 1;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("open Cave defender holder");
        assert_eq!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_OPEN_SPEED);
        assert_eq!(child.position.z, ground + 700 - 16);
        assert_eq!(
            child.cave_collision_cooldown,
            LEVEL2_CASTLE_DEFENDER_CAVE_SOUND_COOLDOWN
        );
        assert!(session.economy().events().iter().any(|event| matches!(
            event,
            Level2EconomyEvent::CastleDefenderCaveSound {
                defender_slot: event_slot,
                sound_id: LEVEL2_CASTLE_DEFENDER_CAVE_SOUND,
            } if *event_slot == defender_slot
        )));

        // The ceiling half of sub_11E70 independently blocks the route when
        // the angle flag is clear.
        session.economy_mut().events.clear();
        {
            let economy = session.economy_mut();
            economy.terrain.planes.secondary_heights.fill(20);
            let child = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("ceiling-blocked Cave defender holder");
            child.position.z = ground.saturating_add(520);
            child.control_flags = 0;
            child.cave_collision_cooldown = 0;
        }
        assert!(
            session
                .economy_mut()
                .step_castle_defender_slot(defender_slot)
        );
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .expect("ceiling-blocked Cave defender holder");
        assert_ne!(child.control_flags & 1, 0);
        assert_eq!(child.speed, LEVEL2_CASTLE_DEFENDER_CAVE_BLOCKED_SPEED);

        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_teardown_without_free_arena_only_restores_action() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("no-free teardown session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-free teardown Castle");
        let before_sphere_count = session.economy().spheres().len();
        let before_linked_entities = session.economy().map_chains().linked_entity_indices();
        let before_ground = session.economy().terrain.planes.ground_heights.clone();
        let before_rng = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        {
            let economy = session.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("no-free teardown Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session.economy().castle().expect("gated Castle");
        assert_eq!((castle.action, castle.level), (4, 2));
        assert_eq!(
            session.economy().source_rng.get(&castle_slot).copied(),
            Some(before_rng)
        );
        assert_eq!(
            session.economy().spheres().len(),
            before_sphere_count,
            "no-free teardown cannot manufacture an overflow sphere"
        );
        assert_eq!(
            session.economy().map_chains().linked_entity_indices(),
            before_linked_entities,
            "no-free teardown cannot mutate the source map arena"
        );
        assert_eq!(
            session.economy().terrain.planes.ground_heights,
            before_ground
        );
        assert!(
            !session
                .economy()
                .events()
                .iter()
                .any(|event| matches!(event, Level2EconomyEvent::CastleStageDestroyed { .. }))
        );
    }

    #[test]
    fn level_two_castle_teardown_and_allocator_failure_are_exact() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("no-free teardown session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-free teardown Castle");
        let before_ground = session.economy().terrain.planes.ground_heights.clone();
        let before_rng = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        let before_phase = session.economy().castle().expect("Castle").phase;
        {
            let economy = session.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("no-free teardown Castle");
            castle.action = 6;
            castle.level = 2;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let castle = session.economy().castle().expect("gated Castle");
        assert_eq!((castle.action, castle.level), (4, 2));
        assert_eq!(castle.phase, before_phase.wrapping_add(1));
        assert_eq!(
            session.economy().source_rng.get(&castle_slot).copied(),
            Some(before_rng)
        );
        assert_eq!(
            session.economy().terrain.planes.ground_heights,
            before_ground
        );
        assert!(!session.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::CastleDestructionSound { source_entity_index, .. }
                    | Level2EconomyEvent::CastleStageDestroyed { source_entity_index, .. }
                    | Level2EconomyEvent::CastleDestroyed { source_entity_index, .. }
                    if *source_entity_index == castle_slot
            )
        }));

        // `sub_49F90` rebuilds the free arena when an inactive typed row was
        // omitted from the fast free stack.  The retry must reuse that row,
        // keep the live Castle distinct, and leave exactly one committed
        // overflow sphere rather than leaking or colliding with a slot.
        let mut rebuild = inspection
            .level2_reset_session()
            .expect("allocator-rebuild session");
        let rebuild_castle_slot = rebuild
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("allocator-rebuild Castle");
        let stale_slot = {
            let economy = rebuild.economy_mut();
            let stale_id = economy
                .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 1, 111)
                .expect("stale typed sphere");
            let stale_slot = u16::try_from(stale_id).expect("stale sphere source slot");
            let stale = economy
                .spheres
                .iter_mut()
                .find(|sphere| sphere.source_entity_index == stale_slot)
                .expect("stale typed sphere row");
            stale.active = false;
            let stale_position = stale.position;
            economy
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: stale_slot,
                    active: false,
                    x: stale_position.x,
                    y: stale_position.y,
                })
                .expect("unlink stale typed sphere");
            stale_slot
        };
        {
            let economy = rebuild.economy_mut();
            economy.free_slots.clear();
            let castle = economy.castle.as_mut().expect("allocator-rebuild Castle");
            castle.mana = 2_000;
            castle.mana_capacity = 1_000;
            castle.action = 4;
            castle.phase = 0;
        }
        rebuild.select_level2_campaign_entry_stage();
        rebuild.step_outer_turn(PlayerCommand::default(), 0);
        assert!(rebuild.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == rebuild_castle_slot
            )
        }));
        assert_ne!(stale_slot, rebuild_castle_slot);
        assert!(
            rebuild
                .economy()
                .spheres
                .iter()
                .any(|sphere| sphere.active && sphere.source_entity_index == stale_slot)
        );
        assert!(
            rebuild.economy().map_chains().is_linked(stale_slot),
            "sub_5FD00 CopyEntityPosition relinks a rebuilt overflow sphere"
        );
        assert!(!rebuild.economy().free_slots.contains(&stale_slot));
        assert!(!rebuild.economy().free_slots.contains(&rebuild_castle_slot));
        assert_eq!(
            rebuild
                .economy()
                .spheres
                .iter()
                .filter(|sphere| sphere.active && sphere.source_entity_index == stale_slot)
                .count(),
            1
        );

        // `sub_613D0` follows only the old controller successor chain.  A
        // planted defender and an auxiliary tail row with the same parent
        // must survive into the existing post-downgrade maintenance walk.
        let mut chain = inspection
            .level2_reset_session()
            .expect("controller-chain teardown session");
        chain.select_level2_campaign_entry_stage();
        let castle_family_counts = |economy: &Level2EconomyState| {
            (
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.model == 79)
                    .count(),
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.model == 3 && child.action == 9)
                    .count(),
                economy
                    .castle_children()
                    .iter()
                    .filter(|child| child.active && child.source_class == 5 && child.model == 15)
                    .count(),
            )
        };
        let baseline_hash = chain.campaign_state_hash();
        let baseline_terrain_hash = chain.terrain_hash();
        let baseline_free_count = chain.economy().free_slots.len();
        let baseline_family_counts = castle_family_counts(chain.economy());
        let starting_holder_slots =
            chain.shared_spells().local_player.enabled_spell_holders[..4].to_vec();
        assert_eq!(
            starting_holder_slots,
            vec![115, 116, 117, 118],
            "qualified direct-start materialization must retain source holder order"
        );
        assert!(
            starting_holder_slots
                .iter()
                .all(|slot| !chain.economy().free_slots.contains(slot)),
            "shared-spell holder slots are baseline-owned and cannot re-enter the economy free stack"
        );
        let allocator_baseline = chain.economy().free_slots.clone();
        let chain_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller-chain Castle");
        assert_eq!(
            Some(chain_castle_slot),
            allocator_baseline.last().copied(),
            "Castle allocation must consume the lowest post-holder source slot"
        );
        assert!(
            chain
                .economy_mut()
                .start_castle_construction(chain_castle_slot)
        );
        assert!(chain.economy_mut().step_castle_action(chain_castle_slot));
        {
            let economy = chain.economy_mut();
            assert!(economy.set_castle_controller_coefficient(2, 0x2a));
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 1;
            for child in &mut economy.castle_children {
                if child.parent_entity_index == chain_castle_slot && child.model == 42 {
                    child.active = false;
                }
            }
        }
        assert!(chain.economy_mut().step_castle_action(chain_castle_slot));
        let old_controller_slots = {
            let castle = chain.economy().castle().expect("stage-two Castle");
            let count = usize::from(castle.controller_chain_count);
            assert_eq!(count, 4);
            castle.controller_chain_slots[..count].to_vec()
        };
        let old_controller_generations = old_controller_slots
            .iter()
            .map(|slot| (*slot, chain.economy().slot_generation(*slot)))
            .collect::<BTreeMap<_, _>>();
        assert!(
            old_controller_slots
                .iter()
                .all(|slot| allocator_baseline.contains(slot)
                    && !starting_holder_slots.contains(slot)),
            "controller rows must come from the economy baseline, never from holder slots"
        );
        assert_eq!(
            chain
                .economy()
                .castle_children()
                .iter()
                .filter(|child| {
                    child.active
                        && child.model == 79
                        && child.source_word_4a == 2
                        && old_controller_slots.contains(&child.source_entity_index)
                })
                .count(),
            4
        );
        chain.select_level2_campaign_entry_stage();
        {
            let economy = chain.economy_mut();
            let castle = economy.castle.as_mut().expect("stage-two Castle");
            // Keep the real stage-two controller chain while selecting the
            // source maintenance tail that owns model-15 rows. This is only
            // branch setup; the production maintenance helper creates the
            // rows here, while teardown alone uses the ordinary Level 2
            // source cursor, which also observes terminal auxiliary cleanup.
            castle.level = 3;
            castle.action = 4;
            castle.timer = 0;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
            castle.mana = castle.mana_capacity + 1_000;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(chain_castle_slot);
        let (defender_slots, auxiliary_slots) = {
            let castle = chain.economy().castle().expect("stage-two Castle");
            let defender_count = usize::from(castle.defender_count);
            let auxiliary_count = usize::from(castle.auxiliary_count);
            (
                castle.defender_slots[..defender_count]
                    .iter()
                    .copied()
                    .filter(|slot| *slot != 0)
                    .collect::<Vec<_>>(),
                castle.auxiliary_slots[..auxiliary_count]
                    .iter()
                    .copied()
                    .filter(|slot| *slot != 0)
                    .collect::<Vec<_>>(),
            )
        };
        assert!(!defender_slots.is_empty());
        assert!(!auxiliary_slots.is_empty());
        let auxiliary_generations = auxiliary_slots
            .iter()
            .map(|slot| (*slot, chain.economy().slot_generation(*slot)))
            .collect::<BTreeMap<_, _>>();
        for slot in defender_slots.iter().chain(auxiliary_slots.iter()) {
            assert!(chain.economy().map_chains().is_linked(*slot));
        }
        {
            let castle = chain
                .economy_mut()
                .castle
                .as_mut()
                .expect("stage-two Castle");
            castle.action = 6;
            castle.level = 2;
            castle.life = -1;
        }
        chain.step_outer_turn(PlayerCommand::default(), 0);
        let castle = chain.economy().castle().expect("downgraded Castle");
        assert_eq!(castle.level, 1);
        assert!(
            chain
                .economy()
                .castle_children
                .iter()
                .all(|child| { !(child.active && child.model == 79 && child.source_word_4a == 2) })
        );
        assert!(defender_slots.iter().all(|slot| {
            chain
                .economy()
                .castle_children()
                .iter()
                .any(|child| child.active && child.source_entity_index == *slot)
                && chain.economy().map_chains().is_linked(*slot)
        }));
        for slot in &auxiliary_slots {
            assert!(chain.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: Level2DynamicArenaKind::CastleAuxiliary,
                        ..
                    } if *source_slot == *slot
                )
            }));
            assert!(
                chain
                    .economy()
                    .castle_children()
                    .iter()
                    .all(|child| child.source_entity_index != *slot || !child.active)
            );
            let live_row = chain
                .economy()
                .live_arena_cursor_rows()
                .into_iter()
                .find(|row| row.source_slot == *slot);
            match live_row {
                None => {
                    assert!(!chain.economy().map_chains().is_linked(*slot));
                    assert!(chain.economy().free_slots.contains(slot));
                }
                Some(row) => {
                    // The source reclaim is immediately eligible for a
                    // later constructor in this same outer pass. A reused
                    // slot must carry a new generation and a coherent live
                    // map row; it is not an orphaned auxiliary entity.
                    assert_ne!(
                        row.generation,
                        auxiliary_generations
                            .get(slot)
                            .copied()
                            .expect("auxiliary generation")
                    );
                    assert_eq!(row.kind, Level2DynamicArenaKind::ManaAttraction);
                    assert!(!chain.economy().free_slots.contains(slot));
                    assert!(chain.economy().map_chains().is_linked(*slot));
                }
            }
        }
        let live_rows = chain.economy().live_arena_cursor_rows();
        let old_slot_states = old_controller_slots
            .iter()
            .map(|slot| {
                (
                    *slot,
                    old_controller_generations
                        .get(slot)
                        .copied()
                        .expect("old controller generation"),
                    live_rows
                        .iter()
                        .find(|row| row.source_slot == *slot)
                        .map(|row| (row.generation, row.kind)),
                    chain.economy().free_slots.contains(slot),
                    chain.economy().map_chains().is_linked(*slot),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            old_slot_states
                .iter()
                .all(|(_, old_generation, live, free, linked)| match live {
                    None => *free && !*linked,
                    Some((generation, kind)) => {
                        *generation != *old_generation
                            && !*free
                            && (*linked || matches!(kind, Level2DynamicArenaKind::CastleChild))
                    }
                }),
            "retired controller slots must be free or source-reused with a new generation: {old_slot_states:?}"
        );
        assert_ne!(chain.campaign_state_hash(), baseline_hash);
        assert_ne!(chain.economy().free_slots.len(), baseline_free_count);
        assert_ne!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );
        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );

        // Exhaust every ordinary arena row while retaining two flagged rows.
        // All constructors use the global `NewEvent_4A050` chokepoint; the
        // generic death-burst producer is the discriminating consumer here.
        chain.select_level2_campaign_entry_stage();
        let retry_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("retry allocator Castle");
        {
            let economy = chain.economy_mut();
            let castle = economy.castle.as_mut().expect("retry allocator Castle");
            castle.level = 3;
            castle.action = 4;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(retry_castle_slot);
        let retry_slot = chain
            .economy()
            .castle()
            .expect("retry allocator Castle")
            .auxiliary_slots[0];
        assert_ne!(retry_slot, 0);
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == retry_slot
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                && child.parent_entity_index == 0
        }));
        let preexisting_death_burst_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(retry_castle_slot, 111, Position::default())
            .expect("second flagged retry row");
        assert!(retry_slot < preexisting_death_burst_slot);
        assert_eq!(
            chain
                .economy()
                .castle_children()
                .iter()
                .filter(|child| {
                    child.active
                        && child.source_class == 5
                        && child.model == 15
                        && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                })
                .map(|child| child.source_entity_index)
                .chain(
                    chain
                        .economy()
                        .death_bursts()
                        .iter()
                        .filter(|burst| {
                            burst.active && burst.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                        })
                        .map(|burst| burst.source_entity_index),
                )
                .collect::<Vec<_>>(),
            vec![retry_slot, preexisting_death_burst_slot]
        );
        let ordinary_slots = chain.economy().free_slots.len();
        let filled_rows = {
            let economy = chain.economy_mut();
            for _ in 0..ordinary_slots {
                assert!(
                    economy
                        .spawn_mana_sphere_from_death(52, Position::default(), 0, 0x9876, 1, 111)
                        .is_some()
                );
            }
            ordinary_slots
        };
        assert_eq!(filled_rows, ordinary_slots);
        assert_eq!(filled_rows + 4, baseline_free_count);
        assert!(chain.economy().free_slots.is_empty());
        let death_burst_count = chain.economy().death_bursts().len();
        let retry_generation_before = chain.economy().slot_generation(retry_slot);
        let retry_global_rng = chain.economy().global_rng;
        let consumed_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(retry_castle_slot, 111, Position::default())
            .expect("global retry allocator consumes Castle model-15 row");
        assert_eq!(consumed_slot, retry_slot);
        assert_eq!(chain.economy().death_bursts().len(), death_burst_count + 1);
        let replacement_burst = chain
            .economy()
            .death_bursts()
            .iter()
            .find(|burst| burst.active && burst.source_entity_index == retry_slot)
            .expect("replacement death burst");
        assert_eq!(
            replacement_burst.entity_rng,
            retry_slot.wrapping_add(retry_global_rng as u16)
        );
        assert_ne!(
            chain.economy().slot_generation(retry_slot),
            retry_generation_before
        );
        assert!(!chain.economy().slot_rng.contains_key(&retry_slot));
        assert!(!chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == retry_slot
                && child.source_class == 5
                && child.model == 15
        }));
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("retry allocator Castle")
                .auxiliary_slots[0],
            retry_slot
        );
        assert!(chain.economy().map_chains().is_linked(retry_slot));
        assert!(chain.economy().free_slots.is_empty());

        // `sub_5FF50` owns the stale Castle relation cleanup; allocation must
        // leave it visible until that production maintenance walk runs.
        chain
            .economy_mut()
            .maintain_castle_defenders(retry_castle_slot);
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("retry allocator Castle")
                .auxiliary_slots[0],
            0
        );

        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );

        // Source `sub_49F90` orders every flagged live row by arena slot, not
        // by producer family. A lower authored model-6 row must therefore
        // beat the later Castle model-15 auxiliary when the generic death
        // burst constructor reaches the shared NewEvent fallback.
        chain.select_level2_campaign_entry_stage();
        let lower_model6 = chain
            .economy_mut()
            .allocate_authored_model6(361, Position::default())
            .expect("lower flagged authored model-6 row");
        let lower_model6_slot = lower_model6.source_entity_index;
        let lower_model6_rng = chain
            .economy()
            .source_rng
            .get(&lower_model6_slot)
            .copied()
            .expect("authored model-6 source RNG");
        assert_eq!(
            chain.economy().source_rng.get(&lower_model6_slot),
            Some(&lower_model6_rng)
        );
        let lower_model6_generation = chain.economy().slot_generation(lower_model6_slot);
        let lower_castle_slot = chain
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("lower-slot discriminator Castle");
        {
            let economy = chain.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("lower-slot discriminator Castle");
            castle.level = 3;
            castle.action = 4;
            castle.phase &= !1;
            castle.auxiliary_cooldown = 0;
        }
        chain
            .economy_mut()
            .maintain_castle_defenders(lower_castle_slot);
        let lower_case_auxiliary_slot = chain
            .economy()
            .castle()
            .expect("lower-slot discriminator Castle")
            .auxiliary_slots[0];
        assert_ne!(lower_case_auxiliary_slot, 0);
        assert!(lower_model6_slot < lower_case_auxiliary_slot);
        assert_eq!(
            chain
                .economy()
                .authored_model6
                .iter()
                .filter(|source| {
                    source.active
                        && source.source_entity_index == lower_model6_slot
                        && source.control_flags & SOURCE_REPLACEABLE_FLAG != 0
                })
                .count(),
            1
        );
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == lower_case_auxiliary_slot
                && child.source_class == 5
                && child.model == 15
                && child.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        }));
        chain.economy_mut().free_slots.clear();
        let replacement_global_rng = chain.economy().global_rng;
        let consumed_slot = chain
            .economy_mut()
            .spawn_death_burst_from_death_with_id(lower_castle_slot, 111, Position::default())
            .expect("global fallback consumes lower authored model-6 row");
        assert_eq!(consumed_slot, lower_model6_slot);
        assert_eq!(
            chain
                .economy()
                .authored_model6
                .iter()
                .filter(|source| source.source_entity_index == lower_model6_slot)
                .count(),
            0,
            "typed authored model-6 row was evicted in place"
        );
        assert!(chain.economy().castle_children().iter().any(|child| {
            child.active
                && child.source_entity_index == lower_case_auxiliary_slot
                && child.source_class == 5
                && child.model == 15
        }));
        let replacement_burst = chain
            .economy()
            .death_bursts()
            .iter()
            .find(|burst| burst.active && burst.source_entity_index == lower_model6_slot)
            .expect("replacement death burst at authored model-6 slot");
        assert_eq!(
            replacement_burst.entity_rng,
            lower_model6_slot.wrapping_add(replacement_global_rng as u16)
        );
        assert_eq!(
            chain.economy().source_rng.get(&lower_model6_slot),
            None,
            "eviction clears the old authored model-6 source RNG"
        );
        assert_eq!(
            chain.economy().slot_rng.get(&lower_model6_slot),
            None,
            "eviction clears the old slot RNG"
        );
        assert_ne!(
            chain.economy().slot_generation(lower_model6_slot),
            lower_model6_generation
        );
        assert!(chain.economy().map_chains().is_linked(lower_model6_slot));
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("lower-slot discriminator Castle")
                .auxiliary_slots[0],
            lower_case_auxiliary_slot,
            "Castle auxiliary relation remains until sub_5FF50 maintenance"
        );
        chain
            .economy_mut()
            .maintain_castle_defenders(lower_castle_slot);
        assert_eq!(
            chain
                .economy()
                .castle()
                .expect("lower-slot discriminator Castle")
                .auxiliary_slots[0],
            lower_case_auxiliary_slot,
            "maintenance preserves the still-live higher Castle auxiliary"
        );
        chain.reset();
        assert_eq!(chain.campaign_state_hash(), baseline_hash);
        assert_eq!(chain.terrain_hash(), baseline_terrain_hash);
        assert_eq!(chain.economy().free_slots.len(), baseline_free_count);
        assert_eq!(
            castle_family_counts(chain.economy()),
            baseline_family_counts
        );
    }

    #[test]
    fn level_two_mana_sphere_vertical_velocity_uses_signed_source_adjustment() {
        assert_eq!(level2_mana_sphere_vertical_velocity(0), 127);
        assert_eq!(level2_mana_sphere_vertical_velocity(1_024), 0);
        assert_eq!(level2_mana_sphere_vertical_velocity(2_048), -128);
    }

    #[test]
    fn level_two_castle_spawn_carrier_accepts_source_launch_payload_and_hashes_relation() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Castle source-payload session");
        session.select_level2_campaign_entry_stage();
        let baseline_hash = session.campaign_state_hash();
        let payload = Level2CastleProjectileLaunchPayload {
            position: Position {
                x: 8_192,
                y: 8_448,
                z: 256,
            },
            yaw: 1_792,
            pitch: -321,
            source_id_entity_index: 111,
            source_subspell_index: 2,
            source_mana: 2_400,
            source_a4_byte_154: 7,
        };
        let flyer_slot = session
            .economy_mut()
            .spawn_castle_projectile_with_payload(payload, 0, 37)
            .expect("source-payload Castle allocation");
        let flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("source-payload Castle carrier");
        assert_eq!(flyer.position, payload.position);
        assert_eq!(flyer.yaw, payload.yaw);
        assert_eq!(flyer.pitch, payload.pitch);
        assert_eq!(flyer.owner_entity_index, payload.source_id_entity_index);
        assert_eq!(flyer.actual_speed, 384_i16.wrapping_add(37));
        assert_eq!(flyer.minimum_speed, 384);
        assert_eq!(
            session.economy().source_owner(flyer_slot),
            payload.source_id_entity_index,
            "the source id relation is carried by the hashed economy owner map"
        );
        assert!(session.economy().map_chains().is_linked(flyer_slot));
        assert_ne!(session.campaign_state_hash(), baseline_hash);

        session
            .economy_mut()
            .disable_castle_projectile(flyer_slot, true);
        session
            .economy_mut()
            .reclaim_hidden_castle_rows_at_source_boundary();
        assert_eq!(session.economy().source_owner(flyer_slot), 0);
        assert!(!session.economy().map_chains().is_linked(flyer_slot));
    }

    #[test]
    fn level_two_castle_flyer_lands_on_post_move_terrain_intercept() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 terrain-intercept session");
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(
            PlayerCommand {
                opcode: 6,
                argument_1: 2,
                flags: 0x40,
                ..PlayerCommand::default()
            },
            0,
        );
        let flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.active)
            .copied()
            .expect("new-Castle flyer remains live after its same-pass gate-only callback");
        let ground = session.terrain.ground_height(flyer.position);
        let blocker_index = session
            .entities
            .iter()
            .position(|entity| !entity.active && entity.index != 0)
            .expect("free source row for prior-landing gate blocker");
        let blocker = &mut session.entities[blocker_index];
        blocker.active = true;
        blocker.life = -1;
        blocker.class = 10;
        blocker.model = 45;
        blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
        blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
        blocker.set_position(flyer.position);
        {
            let projectile = session
                .economy_mut()
                .castle_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
                .expect("live flyer row");
            projectile.position.z = ground + 1;
            projectile.axis_position = Position {
                x: projectile.position.x,
                y: projectile.position.y,
                z: ground - 1,
            };
            projectile.life = projectile.max_life;
        }
        let before_landing_yaw = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            .expect("terrain-intercept flyer")
            .yaw;
        let entities = session.entities.clone();
        assert!(
            !session
                .economy_mut()
                .step_castle_projectile_slot(flyer.source_entity_index, &entities),
            "post-move ground interception lands the flyer before life expiry"
        );
        let root = session
            .economy()
            .castle()
            .expect("terrain landing allocates the selected class-3/model-2 root");
        assert_eq!((root.source_class, root.source_model), (3, 2));
        assert!(
            session
                .economy()
                .castle_projectiles()
                .iter()
                .all(|projectile| !projectile.active),
            "terrain landing consumes the flyer after 4A190 succeeds"
        );
        let consumed_flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            .expect("consumed terrain-intercept flyer");
        assert_eq!(
            consumed_flyer.yaw, before_landing_yaw,
            "prior terrain landing does not enter the later 11CB0 bounce branch"
        );
    }

    #[test]
    fn level_two_castle_flyer_later_site_gate_bounces_and_retries_root() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 later-gate session");
        session.economy_mut().terrain.planes.angles.fill(0);
        let initial = Position {
            x: 8_192,
            y: 8_192,
            z: 10_000,
        };
        let flyer_slot = session
            .economy_mut()
            .spawn_castle_projectile(initial, 111, 0, 0, 0, 0)
            .expect("new-Castle flyer");
        assert!(
            session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &[])
        );
        {
            let projectile = session
                .economy_mut()
                .castle_projectiles
                .iter_mut()
                .find(|projectile| projectile.source_entity_index == flyer_slot)
                .expect("gate-initialized flyer");
            projectile.position = initial;
            projectile.axis_position = Position {
                x: 8_400,
                y: initial.y,
                z: initial.z,
            };
            projectile.yaw = 512;
            projectile.life = projectile.max_life;
        }
        let blocker_index = session
            .entities
            .iter()
            .position(|entity| !entity.active && entity.index != 0)
            .expect("free source row for later gate blocker");
        let blocker = &mut session.entities[blocker_index];
        blocker.active = true;
        blocker.life = -1;
        blocker.class = 10;
        blocker.model = 45;
        blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
        blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
        blocker.set_position(initial);
        let entities = session.entities.clone();
        let free_slots = session.economy().free_slots.clone();
        session.economy_mut().free_slots.clear();
        let before_failed_retry = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("live flyer before pure gate failure");
        assert!(before_failed_retry.position.z > session.terrain.ground_height(initial));
        assert_eq!(before_failed_retry.life, before_failed_retry.max_life);
        assert!(
            session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &entities)
        );
        let bounced = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.source_entity_index == flyer_slot)
            .copied()
            .expect("allocation failure leaves bounced flyer live");
        assert!(bounced.active);
        assert_ne!(bounced.position, before_failed_retry.position);
        assert_eq!(bounced.yaw, 1_536);
        assert_eq!(bounced.life, bounced.max_life - 1);
        assert!(session.economy().castle().is_none());

        session.economy_mut().free_slots = free_slots;
        assert!(
            !session
                .economy_mut()
                .step_castle_projectile_slot(flyer_slot, &entities)
        );
        let root = session
            .economy()
            .castle()
            .expect("later gate retry allocates the Castle root");
        assert_eq!((root.source_class, root.source_model), (3, 2));
        assert_ne!(root.position, initial);
        assert!(
            session
                .economy()
                .castle_projectiles()
                .iter()
                .all(|projectile| !projectile.active)
        );
    }

    #[test]
    fn level_two_castle_site_gate_scans_live_castles_and_model45_rows() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 site-gate session");
        session.select_level2_campaign_entry_stage();
        let request = PlayerCommand {
            opcode: 6,
            argument_1: 2,
            flags: 0x40,
            ..PlayerCommand::default()
        };
        for (class, model) in [(3_u8, 2_u8), (10_u8, 45_u8)] {
            session.reset();
            session.select_level2_campaign_entry_stage();
            let blocker_index = session
                .entities
                .iter()
                .position(|entity| !entity.active && entity.index != 0)
                .expect("free source row for site-gate blocker");
            let blocker = &mut session.entities[blocker_index];
            blocker.active = true;
            blocker.life = if class == 10 && model == 45 { -1 } else { 8 };
            blocker.class = class;
            blocker.model = model;
            blocker.record[88..90].copy_from_slice(&0_i16.to_le_bytes());
            blocker.record[90..92].copy_from_slice(&0_i16.to_le_bytes());
            blocker.set_position(session.state.position);

            // The source cursor visits the newly allocated flyer after the
            // holder in this same request pass. Install the live blocker
            // before that request so the first 11CB0 gate rejects the flyer;
            // inserting it after the request would probe the later landing
            // branch and incorrectly permit a Castle root.
            session.step_outer_turn(request, 0);
            assert!(session.economy().castle().is_none());
            let flyer = session
                .economy()
                .castle_projectiles()
                .iter()
                .find(|projectile| projectile.site_rejected && !projectile.active)
                .copied()
                .expect("same-pass site gate hides the blocked Castle flyer");
            assert_eq!((flyer.source_class, flyer.source_model), (9, 10));
            let flyer_generation = session.economy().slot_generation(flyer.source_entity_index);
            assert!(flyer.drawing_disabled);
            assert!(
                session
                    .economy()
                    .map_chains()
                    .is_linked(flyer.source_entity_index)
            );
            assert!(
                !session
                    .economy()
                    .free_slots
                    .contains(&flyer.source_entity_index)
            );
            assert!(!session.dispatch_events().iter().any(|event| matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    kind: Level2DynamicArenaKind::CastleProjectile,
                    ..
                }
            )));

            // DisableEntityDrawing keeps the row linked and reserved until
            // the next generic hidden-row prepass, where the source frees it.
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !session
                    .economy()
                    .castle_projectiles()
                    .iter()
                    .any(|projectile| projectile.source_entity_index == flyer.source_entity_index)
            );
            assert!(
                session
                    .economy()
                    .free_slots
                    .contains(&flyer.source_entity_index)
            );
            assert!(
                !session
                    .economy()
                    .map_chains()
                    .is_linked(flyer.source_entity_index)
            );
            assert_eq!(
                session.economy().slot_generation(flyer.source_entity_index),
                flyer_generation
            );
        }
    }

    #[test]
    fn level_two_economy_sound_receipts_pin_source_owner_channel_and_negatives() {
        let mut session = level_two_possession_lifecycle_session();

        // AUD-L2-036: the qualified Quickfair callback latches sound before
        // its three shape-cell RNG branches. Drive the real row-214 ->
        // group-20 source chain; no allocator or narrative helper is used.
        let authored_features = session.mission.authored_features().to_vec();
        let feature_position = |index: u16| {
            authored_features
                .iter()
                .copied()
                .find(|feature| feature.index == index)
                .expect("qualified Quickfair source-chain row")
                .world_position(0)
        };
        session.state.position = feature_position(11);
        session.runtime.objectives[0].stage_selectors[1] = 2;
        session.runtime.objectives[0].stage_selectors[2] = 2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.runtime.objectives[0].stage_selectors[3] = 2;
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.authored_index == 214)
        );
        session.state.position = feature_position(214);
        let expected_group20 = [
            216_u16, 217, 218, 219, 220, 221, 222, 223, 232, 233, 234, 235,
        ];
        let mut activated = false;
        for _ in 0..8 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        group: 20,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == expected_group20
                )
            }) {
                activated = true;
                break;
            }
        }
        assert!(activated, "row 214 must activate group 20 in production");
        let quickfair = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.authored_index == 216)
            .copied()
            .expect("production group-20 Quickfair source");
        assert_eq!(
            session
                .dispatch_events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2DispatchEvent::Economy {
                            source_slot,
                            event: Level2EconomyEvent::PreparedSound {
                                source_entity_index,
                                entity_index,
                                player_index: -1,
                                sound_id: 3,
                            },
                            ..
                        } if *source_slot == quickfair.source_entity_index
                            && *source_entity_index == quickfair.source_entity_index
                            && *entity_index == quickfair.source_entity_index
                    )
                })
                .count(),
            1,
            "the production Quickfair callback emits one sound-3 receipt"
        );

        // A following callback does not duplicate the sound latch.
        session.economy_mut().begin_turn();
        assert!(
            session
                .economy_mut()
                .step_authored_model1_slot(quickfair.source_entity_index)
        );
        assert!(session.economy().events().is_empty());

        // The negative life guard precedes the sound latch and all Quickfair
        // source draws.
        session.economy_mut().begin_turn();
        let before = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .copied()
            .expect("Quickfair remains live for negative branch");
        session
            .economy_mut()
            .authored_model1
            .iter_mut()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .expect("Quickfair negative branch source")
            .life = -1;
        assert!(
            session
                .economy_mut()
                .step_authored_model1_slot(quickfair.source_entity_index)
        );
        let after = session
            .economy()
            .authored_model1()
            .iter()
            .find(|source| source.source_entity_index == quickfair.source_entity_index)
            .copied()
            .expect("Quickfair remains inspectable after negative branch");
        assert!(session.economy().events().is_empty());
        assert_eq!(after.entity_rng, before.entity_rng);

        // AUD-L2-056/057: both accepted possession branches carry the house
        // source as the handler slot and v7/v8 as the exact owner argument.
        session.reset();
        let blocked_house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.parameter.possession_blocked())
            .expect("pinned BLDGPRM possession negative");
        let blocked_owner = session.state.entity_index;
        assert!(
            session
                .ordinary_mut()
                .submit_house_possession(blocked_house.entity_index, blocked_owner, 1)
                .is_err()
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                }
            )
        }));
        session.reset();
        let house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.parameter.produces_villagers())
            .expect("qualified Level 2 possession house");
        let owner = session.state.entity_index;
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, owner, 1)
            .expect("payload-one house possession mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound {
                        source_entity_index,
                        entity_index,
                        player_index: -1,
                        sound_id: 4,
                    },
                    ..
                } if *source_slot == house.entity_index
                    && *source_entity_index == house.entity_index
                    && *entity_index == owner
            )
        }));
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                } if *source_slot == house.entity_index
            )
        }));
        session.reset();
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, owner, 0)
            .expect("payload-zero house possession mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound {
                        source_entity_index,
                        entity_index,
                        player_index: -1,
                        sound_id: 4,
                    },
                    ..
                } if *source_slot == house.entity_index
                    && *source_entity_index == house.entity_index
                    && *entity_index == owner
            )
        }));
        session.reset();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 4, .. },
                    ..
                }
            )
        }));

        // AUD-L2-080: InitSwitchChainZaxisAndSound resolves the first
        // overlapping model-0 owner (the local player here), then emits
        // before the group-10 model-76 allocation is announced.
        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("qualified imported model-0 switch 25");
        session.state.position = switch.position;
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 remains live")
            .phase = 0;
        let owner = session.state.entity_index;
        session.step_outer_turn(PlayerCommand::default(), 0);
        let sound_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Economy {
                        pass: 0,
                        source_slot,
                        event: Level2EconomyEvent::PreparedSound {
                            source_entity_index,
                            entity_index,
                            player_index: -1,
                            sound_id: 41,
                        },
                        ..
                    } if *source_slot == switch.source_slot
                        && *source_entity_index == switch.source_slot
                        && *entity_index == owner
                )
            })
            .expect("source pass emits switch sound 41");
        let group_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        source_slot,
                        group: 10,
                        ..
                    } if *source_slot == switch.source_slot
                )
            })
            .expect("source pass announces switch group 10");
        assert!(sound_index < group_index, "sound precedes group activation");
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group: 10,
                    ..
                } if *source_slot == switch.source_slot
            )
        }));
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    source_slot,
                    event: Level2EconomyEvent::PreparedSound { sound_id: 41, .. },
                    ..
                } if *source_slot == switch.source_slot
            )
        }));

        // Wrong overlap and source-arena exhaustion consume no forbidden
        // sound receipt and leave the switch available for a later source
        // turn.
        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("switch 25 after reset");
        session.state.position = Position {
            x: switch.position.x.wrapping_add(0x4000),
            ..switch.position
        };
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 non-overlap state")
            .phase = 0;
        session.step_level2_group10_switch25(0);
        assert!(session.dispatch_events().is_empty());
        assert!(session.level2_group10_switch25.is_some());

        session.reset();
        let switch = session
            .level2_group10_switch25
            .expect("switch 25 exhaustion state");
        session.state.position = switch.position;
        session
            .level2_group10_switch25
            .as_mut()
            .expect("switch 25 exhaustion phase")
            .phase = 0;
        session.economy_mut().free_slots.clear();
        session.step_level2_group10_switch25(0);
        assert!(!session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Economy {
                    event: Level2EconomyEvent::PreparedSound { sound_id: 41, .. },
                    ..
                }
            )
        }));
    }
