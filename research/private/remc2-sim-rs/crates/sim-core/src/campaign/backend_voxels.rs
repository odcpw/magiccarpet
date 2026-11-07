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
