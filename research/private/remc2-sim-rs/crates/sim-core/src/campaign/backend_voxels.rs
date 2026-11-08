//! Optional voxel configuration belongs to the same semantic replay boundary
//! as the native inputs. Native geometry and BUILD bytes are revalidated.
use super::*;
use crate::voxel_world::VoxelWorldSeed;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VoxelModeChange {
    pub(super) before_operation: usize,
    pub(super) parent_terrain_hash: u64,
    pub(super) seed: VoxelWorldSeed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_session::GameSession;

    fn open() -> GameSession<Mc2CampaignBackend> {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let root = generated.join("mc-bhq8-level1-collision-v5-night");
        let cache = generated.join("mc-oi62-environment-level1-v2");
        let definition =
            load_mc2_campaign_with_owner_cache(&root, &cache, &default_catalog().unwrap()).unwrap();
        GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(definition),
            970,
            Mc2CampaignInputs::with_owner_cache(root, cache),
        )
        .unwrap()
    }

    fn samples() -> Vec<u32> {
        vec![50_f32.to_bits(); 17 * 21 + 256 * 320]
    }

    #[test]
    fn public_crater_changes_composed_voxel_ground_and_keeps_prior_edits() {
        use crate::voxel_world::{Point, VolumeId};
        use crate::{FlightWorld, PlayerCommand, Position};

        // This complete source-reset package admits carry before holder
        // construction. The historical Level1 snapshot used by the replay
        // checks below already contains allocated local holders.
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let root = generated.join("ribbon-20261104-integration/group-zero-owner-package-v4/level2");
        let definition = load_mc2_campaign(&root, &default_catalog().unwrap()).unwrap();
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            Arc::new(definition),
            970,
            Mc2CampaignInputs::new(root),
        )
        .unwrap();
        const HOUSE: u16 = 12;
        let backend = session.backend_mut();
        let mut carry = backend.semantic_spell_carry().unwrap();
        if !carry.acquired.contains(&SpellKind::Crater) {
            carry.acquired.push(SpellKind::Crater);
        }
        carry.selected = Some(SpellKind::Crater);
        backend.apply_semantic_spell_carry(&carry).unwrap();
        let rule = crate::level_definition::CRATER_PROPERTIES.tiers[0];
        // Declared carry, Castle/player funding and pre-cast pose controls.
        // No terrain, actor, source clock, RNG or lifetime is forced.
        {
            let sim = backend.campaign.simulation_mut();
            let player = sim.state.entity_index;
            for (stage, footprint) in
                crate::native_world::campaign_castle_stage_footprints().unwrap()
            {
                sim.economy_mut()
                    .set_castle_stage_footprint(stage, footprint);
            }
            let castle_site = (1_u16..32)
                .flat_map(|y| {
                    (1_u16..32).map(move |x| Position {
                        x: x * 2048,
                        y: y * 2048,
                        z: 0,
                    })
                })
                .find(|position| {
                    !sim.economy().castle_projectile_site_blocked(
                        *position,
                        sim.entities(),
                        sim.canonical_terrain(),
                    )
                })
                .expect("source-valid Castle funding site");
            let castle = sim.economy_mut().build_castle(castle_site, player).unwrap();
            assert!(
                sim.economy_mut().set_castle_mana_prerequisite_for_test(
                    castle,
                    rule.castle_mana_required + 50_000,
                )
            );
            sim.stage_player_mana_for_test(rule.mana_cost + 10_000)
                .unwrap();
        }
        let ui = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| *family == 16)
            .unwrap() as i8;
        backend.campaign.step(
            PlayerCommand {
                opcode: 31,
                argument_1: ui,
                argument_2: 0,
                ..Default::default()
            },
            0,
        );
        assert!(
            backend
                .campaign
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        backend.enable_voxel_world(HOUSE, samples(), 970).unwrap();
        let (point, prior_edit) = {
            let sim = backend.campaign.simulation();
            let terrain = sim.canonical_terrain();
            let world = terrain.voxel_world().unwrap();
            let volume = world
                .volumes()
                .iter()
                .find(|v| v.id == VolumeId::Terrain)
                .unwrap();
            let grid = volume.grid;
            let point = (32..usize::from(grid.dimensions[1]) - 32)
                .step_by(32)
                .flat_map(|y| {
                    (32..usize::from(grid.dimensions[0]) - 32)
                        .step_by(32)
                        .map(move |x| (x, y))
                })
                .map(|(x, y)| Position {
                    x: (grid.origin.x + x as i32 * i32::from(grid.cell_size) + 4) as u16,
                    y: (grid.origin.y + y as i32 * i32::from(grid.cell_size) + 4) as u16,
                    z: 0,
                })
                .find(|p| {
                    let tile = usize::from(p.x >> 8) + 256 * usize::from(p.y >> 8);
                    terrain.planes().angles[tile] & 0x8f != 0
                        && terrain.planes().angles[tile] & 0x80 == 0
                        && terrain.material_mask(*p) != 1
                })
                .expect("enabled real patch has unprotected source ground for Crater");
            let x = grid.origin.x + 4;
            let y = grid.origin.y + 4;
            let z = world.terrain_height(x, y).unwrap() - 4;
            (
                Position {
                    z: terrain.ground_height(point),
                    ..point
                },
                Point { x, y, z },
            )
        };
        {
            let sim = backend.campaign.simulation_mut();
            // One declared prior voxel edit, through the canonical typed
            // impact journal. The following Crater must retain this edit.
            sim.session
                .state
                .terrain_mut()
                .voxel_impact(
                    point,
                    prior_edit,
                    Point {
                        x: 0,
                        y: 0,
                        z: -384,
                    },
                    Point { x: 0, y: 0, z: 1 },
                    0,
                    0,
                    0,
                    None,
                )
                .unwrap();
            let player = sim.state.entity_index;
            let fov = sim.entities()[usize::from(player)].array_fov();
            let hand = Position {
                z: point.z + 1024,
                ..point
            };
            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(hand, point);
            sim.set_level2_player_aim_for_test(yaw, pitch);
            let player_yaw =
                yaw.wrapping_sub(sim.shared_spells().beyond_sight_auxiliary().next_entity) & 2047;
            let mut origin = Position {
                z: hand.z - fov,
                ..hand
            };
            crate::move_entity(
                &mut origin,
                player_yaw.wrapping_sub(512).wrapping_add(1024),
                0,
                256,
            );
            sim.set_level2_entity_position(player, origin).unwrap();
        }
        let baseline = backend.campaign.clone();
        let mut game = baseline.clone();
        let mut no_cast = baseline.clone();
        let before = baseline.simulation().canonical_terrain();
        let initial_ground = before.ground_height(point);
        let initial_source =
            crate::legacy_terrain::interpolate_ground(before.ground_heights(), point.x, point.y);
        let initial_world = before.voxel_world().unwrap();
        let building = initial_world
            .volumes()
            .iter()
            .find(|v| v.id == VolumeId::Building(HOUSE))
            .unwrap()
            .clone();
        let removed_world_poses = |world: &crate::voxel_world::VoxelWorld| {
            world
                .removed_cells()
                .map(|cell| {
                    let volume = world
                        .volumes()
                        .iter()
                        .find(|v| v.id == cell.volume)
                        .unwrap();
                    (
                        cell.volume,
                        cell.x,
                        cell.y,
                        volume.grid.origin.z + i32::from(cell.z) * i32::from(volume.grid.cell_size),
                    )
                })
                .collect::<Vec<_>>()
        };
        let prior_removed = removed_world_poses(initial_world);
        assert!(!prior_removed.is_empty());
        let prior_impacts = before.voxel_impacts().to_vec();
        let mut commands = Vec::new();
        let mut flyer = None;
        let mut ring = None;
        let mut paid = false;
        let mut composed_changed = false;
        let mut hidden = false;
        let mut reclaimed = false;
        let mut previous_mana =
            game.simulation().entities()[usize::from(game.simulation().state.entity_index)].mana;
        for turn in 0..128 {
            let command = if turn == 0 {
                PlayerCommand {
                    flags: 0x10,
                    ..Default::default()
                }
            } else {
                PlayerCommand::default()
            };
            commands.push(command);
            game.step(command, 0);
            no_cast.step(PlayerCommand::default(), 0);
            let sim = game.simulation();
            assert!(
                sim.dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "{:#?}",
                sim.dispatch_events()
            );
            let player = sim.state.entity_index;
            let mana = sim.entities()[usize::from(player)].mana;
            paid |= previous_mana - mana == rule.mana_cost;
            previous_mana = mana;
            for row in sim
                .entities()
                .iter()
                .filter(|row| row.active && row.source_id == player)
            {
                if (row.class, row.model, row.action) == (9, 5, 5) {
                    flyer.get_or_insert(sim.source_arena().handle(row.index).unwrap());
                }
                if (row.class, row.model, row.action) == (10, 11, 11) {
                    ring.get_or_insert(sim.source_arena().handle(row.index).unwrap());
                }
            }
            let terrain = sim.canonical_terrain();
            let source = crate::legacy_terrain::interpolate_ground(
                terrain.ground_heights(),
                point.x,
                point.y,
            );
            if source != initial_source {
                let world = terrain.voxel_world().unwrap();
                assert_ne!(world.base_hash(), initial_world.base_hash());
                assert!(world.revision() > initial_world.revision());
                assert_eq!(
                    terrain.ground_height(point),
                    initial_ground + (source - initial_source).div_euclid(8) as i16 * 8,
                    "source change reaches composed collision before the next public input"
                );
                assert_eq!(
                    world.terrain_height(i32::from(point.x), i32::from(point.y)),
                    Some(i32::from(terrain.ground_height(point)))
                );
                assert_eq!(
                    world
                        .volumes()
                        .iter()
                        .find(|v| v.id == VolumeId::Building(HOUSE)),
                    Some(&building)
                );
                assert_eq!(removed_world_poses(world), prior_removed);
                assert_eq!(
                    terrain.voxel_impacts(),
                    prior_impacts,
                    "source projection does not invent/replay impact events"
                );
                composed_changed = true;
            }
            if let Some(handle) = ring {
                match sim.source_arena().resolve(handle) {
                    Some(_) => {
                        hidden |= sim.entities()[usize::from(handle.source_slot)].drawing_disabled()
                    }
                    None => {
                        reclaimed = true;
                        break;
                    }
                }
            }
        }
        assert!(
            paid && flyer.is_some() && ring.is_some() && composed_changed && hidden && reclaimed,
            "paid={paid} flyer={flyer:?} ring={ring:?} changed={composed_changed} hidden={hidden} reclaimed={reclaimed}"
        );
        assert_eq!(
            no_cast
                .simulation()
                .canonical_terrain()
                .ground_height(point),
            initial_ground
        );
        assert_eq!(
            no_cast
                .simulation()
                .canonical_terrain()
                .voxel_world()
                .unwrap(),
            initial_world
        );
        // Replay serialized public inputs from the identical declared controlled
        // baseline. Fresh-loader reconstruction of those raw funding/pose
        // controls is not claimed by this gate.
        let encoded = serde_json::to_vec(
            &commands
                .into_iter()
                .map(PlayerCommand::to_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let commands: Vec<[u8; 10]> = serde_json::from_slice(&encoded).unwrap();
        let mut replay = baseline;
        for command in commands {
            replay.step(PlayerCommand::from_bytes(command), 0);
        }
        assert_eq!(
            replay.simulation().canonical_terrain(),
            game.simulation().canonical_terrain()
        );
    }

    #[test]
    fn voxel_configuration_checks_native_seed_and_exact_operation_boundary() {
        let mut session = open();
        session
            .step(super::super::tests::guard(&session), GameCommand::default())
            .unwrap();
        let generation = session.identity().generation;
        session
            .backend_mut()
            .enable_voxel_world(38, samples(), generation)
            .unwrap();
        let change = session.backend().voxel_mode_changes[0].clone();
        assert_eq!(change.before_operation, 1);
        let expected = session
            .backend()
            .campaign
            .simulation()
            .canonical_terrain()
            .voxel_world()
            .unwrap()
            .deterministic_hash();
        let mut replay = open();
        let unchanged = replay.backend().snapshot();
        assert!(
            replay
                .backend_mut()
                .apply_voxel_mode_change(change.clone(), generation)
                .is_err()
        );
        assert_eq!(replay.backend().snapshot(), unchanged);
        replay
            .step(super::super::tests::guard(&replay), GameCommand::default())
            .unwrap();
        let mut forged = change.clone();
        forged.seed.houses[0].cells[0][0] ^= 1;
        let unchanged = replay.backend().snapshot();
        assert!(
            replay
                .backend_mut()
                .apply_voxel_mode_change(forged, generation)
                .is_err()
        );
        assert_eq!(replay.backend().snapshot(), unchanged);
        replay
            .backend_mut()
            .apply_voxel_mode_change(change, generation + 7)
            .unwrap();
        let world = replay
            .backend()
            .campaign
            .simulation()
            .canonical_terrain()
            .voxel_world()
            .unwrap();
        assert_eq!(world.generation(), generation + 7);
        assert_eq!(world.deterministic_hash(), expected);
        assert_eq!(replay.backend().snapshot(), session.backend().snapshot());
    }

    #[test]
    fn voxel_configuration_roundtrips_at_nonzero_replay_boundary() {
        let mut session = open();
        session
            .step(super::super::tests::guard(&session), GameCommand::default())
            .unwrap();
        let generation = session.identity().generation;
        session
            .backend_mut()
            .enable_voxel_world(38, samples(), generation)
            .unwrap();
        session
            .step(super::super::tests::guard(&session), GameCommand::default())
            .unwrap();
        let mut save = Mc2CampaignReplaySave::new(
            false,
            session.definition().as_ref().clone(),
            session.backend().initial_semantic_spell_carry().unwrap(),
            Vec::new(),
            session.backend().replay_operations.clone(),
            &session.backend().snapshot(),
        );
        // These are configured inputs to the real replay loader; no combat or
        // destruction is claimed by this configuration-boundary regression.
        save.voxel_mode_changes = session.backend().voxel_mode_changes.clone();
        save.receipt_sha256 = save.compute_receipt_sha256();
        let bytes = serde_json::to_vec(&save).unwrap();
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let resolver = Mc2RuntimePackageResolver::load([
            Mc2CampaignInputs::with_owner_cache(
                generated.join("mc-bhq8-level1-collision-v5-night"),
                generated.join("mc-oi62-environment-level1-v2"),
            ),
            Mc2CampaignInputs::new(
                generated.join("mc-cf02-campaign-package-v2-b/qualified-level2"),
            ),
            Mc2CampaignInputs::with_owner_cache(
                generated.join("level3-day-block32-campaign-recipe-v2"),
                generated.join("mc-level3-environment-exit-20261025"),
            ),
        ])
        .unwrap();
        let mut runtime = crate::game_runtime::GameRuntime::title(971).unwrap();
        let guard = runtime.identity().guard();
        resolver
            .load_replay(&mut runtime, guard, 972, &bytes)
            .unwrap();
        assert_eq!(
            runtime.session().unwrap().backend().snapshot(),
            session.backend().snapshot()
        );
        assert_eq!(resolver.save_replay(&runtime).unwrap(), bytes);
        // An absent extension keeps the original v7 receipt representation.
        let old = open();
        let save = Mc2CampaignReplaySave::new(
            false,
            old.definition().as_ref().clone(),
            old.backend().initial_semantic_spell_carry().unwrap(),
            Vec::new(),
            Vec::new(),
            &old.backend().snapshot(),
        );
        assert!(
            !serde_json::to_string(&save)
                .unwrap()
                .contains("voxel_mode_changes")
        );
        assert!(save.verify());
    }
}

impl Mc2CampaignBackend {
    pub fn enable_voxel_world(
        &mut self,
        selected_slot: u16,
        modern_heights: Vec<u32>,
        generation: u64,
    ) -> std::result::Result<(), String> {
        if self
            .campaign
            .simulation()
            .canonical_terrain()
            .voxel_world()
            .is_some()
        {
            return Err("voxel geometry is already enabled".to_owned());
        }
        let seed = VoxelWorldSeed::from_inspection(
            self.campaign.inspection(),
            self.campaign
                .simulation()
                .canonical_terrain()
                .ground_heights(),
            selected_slot,
            modern_heights,
        )
        .map_err(str::to_owned)?;
        let change = VoxelModeChange {
            before_operation: self.replay_operations.len(),
            parent_terrain_hash: self
                .campaign
                .simulation()
                .canonical_terrain()
                .deterministic_hash(),
            seed,
        };
        self.campaign
            .simulation_mut()
            .install_voxel_seed(&change.seed, generation)?;
        self.voxel_mode_changes.push(change);
        Ok(())
    }

    pub(super) fn apply_voxel_mode_change(
        &mut self,
        change: VoxelModeChange,
        generation: u64,
    ) -> std::result::Result<(), String> {
        if self.replay_operations.len() != change.before_operation
            || self
                .campaign
                .simulation()
                .canonical_terrain()
                .deterministic_hash()
                != change.parent_terrain_hash
        {
            return Err(
                "voxel replay configuration has a stale operation/terrain boundary".to_owned(),
            );
        }
        let expected = VoxelWorldSeed::from_inspection(
            self.campaign.inspection(),
            self.campaign
                .simulation()
                .canonical_terrain()
                .ground_heights(),
            change.seed.selected_slot,
            change.seed.modern_heights.clone(),
        )
        .map_err(str::to_owned)?;
        if expected != change.seed {
            return Err("voxel replay seed differs from native owner geometry".to_owned());
        }
        self.enable_voxel_world(
            change.seed.selected_slot,
            change.seed.modern_heights,
            generation,
        )
    }
}
