//! Descriptor-backed campaign session facade.
//!
//! Campaign sessions validate a complete
//! [`spec::CampaignDefinition`] before any mutable inspection/session state is
//! constructed, then delegate the package-present behavior to the shared Rust
//! authority.

use std::{fmt, path::Path, sync::Arc};

use crate::{
    PlayerCommand, Position, StepEvidence,
    level_definition::{LevelDefinition, LevelEntityPositionAccess},
    native_world::{Mc2DispatchEvent, NativeWorld, NativeWorldError, NativeWorldInspection},
};

pub(crate) mod authored_spawn;
pub mod backend;
pub(crate) mod completion_statistics;
pub mod dispatch;
pub mod journal;
pub(crate) mod mc2_level_definition;
pub(crate) mod mc2_secret_flow;
pub mod objective_speech;
pub mod objectives;
pub mod rewards;
pub mod schedule;
#[doc(hidden)]
pub mod source_arena;
pub mod source_facts;
pub(crate) mod source_menus;
pub(crate) mod source_player_result;
pub(crate) mod source_setting;
pub mod spec;
pub(crate) mod sprite_render;
pub mod state;

pub use crate::native_world::Mc2Simulation;

use dispatch::CAMPAIGN_BEHAVIOR_REGISTRY;
use journal::{EffectCursor, EffectPage, EffectPageError};
use spec::{CampaignDefinition, CampaignPackageError, CampaignSelection};

/// Failure to validate or materialize a campaign session. A validation error
/// is always returned before an inspection, reset state, or live session has
/// been constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignOpenError {
    Preflight(CampaignPackageError),
    Materialization(NativeWorldError),
}

impl fmt::Display for CampaignOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(error) => write!(formatter, "{error}"),
            Self::Materialization(error) => write!(formatter, "campaign materialization: {error}"),
        }
    }
}

impl std::error::Error for CampaignOpenError {}

/// A validated mission definition and its one live MC2 simulation.
#[derive(Clone, Debug)]
pub struct CampaignSession {
    definition: Arc<CampaignDefinition>,
    inspection: Arc<NativeWorldInspection>,
    simulation: Mc2Simulation,
    speech_enabled: bool,
    show_foreign_objective_text: bool,
}

impl CampaignSession {
    /// Validate and open a campaign package. Contract v1 admits the
    /// census-profiled selections (qualified Levels 1, 2, and 3); unsupported
    /// identities fail during descriptor preflight rather than selecting a
    /// fallback implementation. Behavior closure validates every live selector
    /// against the shared compiled registry before this materializer runs, so
    /// the folded authority below stays the one live engine and no per-level
    /// route exists.
    pub fn open(
        package_root: impl AsRef<Path>,
        selection: CampaignSelection,
    ) -> Result<Self, CampaignOpenError> {
        Self::open_with_dynamic_lighting(package_root, selection, true)
    }

    /// Bind the source option before startup constructors admit any light.
    pub fn open_with_dynamic_lighting(
        package_root: impl AsRef<Path>,
        selection: CampaignSelection,
        enabled: bool,
    ) -> Result<Self, CampaignOpenError> {
        Self::open_with_materializer(package_root.as_ref(), selection, |definition| {
            let inspection = NativeWorld::inspect_campaign_definition_with_dynamic_lighting(
                definition, None, enabled,
            )?;
            let simulation = inspection.campaign_reset_session_from_definition(definition)?;
            Ok((inspection, simulation))
        })
    }

    /// Open with an explicit private gameplay or environment material component while
    /// keeping owner bytes outside the package and distributable client tree.
    pub fn open_with_owner_cache(
        package_root: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
        selection: CampaignSelection,
    ) -> Result<Self, CampaignOpenError> {
        Self::open_with_owner_cache_and_dynamic_lighting(
            package_root, owner_cache, selection, true,
        )
    }

    /// Open with the actual source option and the existing private asset cache.
    pub fn open_with_owner_cache_and_dynamic_lighting(
        package_root: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
        selection: CampaignSelection,
        enabled: bool,
    ) -> Result<Self, CampaignOpenError> {
        let owner_cache = owner_cache.as_ref();
        Self::open_with_materializer(package_root.as_ref(), selection, |definition| {
            let mut animation_bank = None;
            let inspection = if selection == CampaignSelection::LEVEL1
                || owner_cache.join("materials/descriptor.json").is_file()
            {
                // Material components supply presentation, not legacy BUILD
                // footprints. Validate their package binding without feeding
                // them to the gameplay-component decoder or changing reset state.
                let assets = if selection == CampaignSelection::LEVEL1 {
                    crate::day_indexed_frame::DayIndexedFrameAssets::load_night(owner_cache)
                } else {
                    crate::day_indexed_frame::DayIndexedFrameAssets::load(owner_cache)
                }
                .map_err(|error| NativeWorldError::new(error.to_string()))?;
                let profile = assets.profile();
                // Only sealed Night textures predate campaign binding and
                // deliberately reuse the older export's presentation bytes.
                let legacy_night = selection == CampaignSelection::LEVEL1
                    && assets.campaign_package_sha256().is_none();
                if !legacy_night
                    && (profile.external_level != selection.external_level
                        || profile.internal_level != selection.internal_level
                        || profile.map_type_id != selection.map_type_id
                        || assets.package_sha256()
                            != definition
                                .owner_export_package_sha256()
                                .map_err(|error| NativeWorldError::new(error.to_string()))?
                        || assets
                            .campaign_package_sha256()
                            .is_some_and(|hash| hash != definition.package_sha256))
                {
                    return Err(NativeWorldError::new(
                        "material cache does not match the opened campaign package",
                    ));
                }
                animation_bank = assets.source_animation_bank()
                    .map(|(catalog, payloads)| (catalog, payloads, Arc::<str>::from(assets.cache_content_sha256())));
                NativeWorld::inspect_campaign_definition_with_dynamic_lighting(
                    definition, None, enabled,
                )?
            } else {
                if requires_source_animation_assets(definition) {
                    return Err(missing_source_animation_assets());
                }
                NativeWorld::inspect_campaign_definition_with_dynamic_lighting(
                    definition, Some(owner_cache), enabled,
                )?
            };
            let mut simulation = inspection.campaign_reset_session_from_definition(definition)?;
            if let Some((catalog, payloads, owner)) = animation_bank {
                simulation.bind_source_animation_catalog(catalog, payloads, owner)?;
            }
            Ok((inspection, simulation))
        })
    }

    fn open_with_materializer(
        package_root: &Path,
        selection: CampaignSelection,
        materialize: impl FnOnce(
            &CampaignDefinition,
        )
            -> Result<(NativeWorldInspection, Mc2Simulation), NativeWorldError>,
    ) -> Result<Self, CampaignOpenError> {
        let definition =
            CampaignDefinition::preflight(package_root, selection, &CAMPAIGN_BEHAVIOR_REGISTRY)
                .map_err(CampaignOpenError::Preflight)?;
        let (inspection, simulation) =
            materialize(&definition).map_err(CampaignOpenError::Materialization)?;
        if requires_source_animation_assets(&definition) && !simulation.source_animations.is_bound() {
            return Err(CampaignOpenError::Materialization(missing_source_animation_assets()));
        }
        Ok(Self {
            definition: Arc::new(definition),
            inspection: Arc::new(inspection),
            simulation,
            speech_enabled: true,
            show_foreign_objective_text: false,
        })
    }

    #[must_use]
    pub fn definition(&self) -> &CampaignDefinition {
        &self.definition
    }

    /// Adopt one explicitly authored portal pair before exposing a fresh
    /// session. The validated owner members and their provenance stay intact.
    pub(crate) fn apply_authored_portal_variant(
        &mut self,
        level: &crate::level_definition::LevelDefinition,
    ) -> Result<(), NativeWorldError> {
        let (descriptor, edits) = self
            .definition
            .with_authored_portal_variant(level.clone())
            .map_err(NativeWorldError::new)?;
        let mut features = self
            .simulation
            .session
            .state
            .live
            .mission
            .authored_features()
            .to_vec();
        for (record, group) in edits {
            let row = features
                .get_mut(usize::from(record))
                .ok_or_else(|| NativeWorldError::new("authored portal record is unavailable"))?;
            row.activation_group = group;
        }
        let mut simulation_definition = (*self.simulation.session.definition).clone();
        simulation_definition.adopt_authored_portal_level(level.clone());
        self.definition = Arc::new(descriptor);
        self.simulation.session.definition = Arc::new(simulation_definition);
        self.simulation
            .session
            .state
            .live
            .mission
            .authored_features_mut()
            .copy_from_slice(&features);
        self.simulation.session.rebase();
        Ok(())
    }

    /// Declare a bounded source-language fixture before the first public turn.
    /// Shipped records and production reset admission remain unchanged.
    #[cfg(test)]
    pub(crate) fn configure_source_variant_for_test(
        &mut self,
        features: &[crate::level1_mission::AuthoredFeature],
        bindings: &state::SemanticLevelBindings,
    ) -> Result<(), NativeWorldError> {
        let original = self
            .simulation
            .session
            .state
            .live
            .mission
            .authored_features();
        if features.len() != original.len() {
            return Err(NativeWorldError::new(
                "configured item must preserve the authored extent",
            ));
        }
        let changed = original
            .iter()
            .zip(features)
            .filter(|(before, after)| before != after)
            .collect::<Vec<_>>();
        if changed.len() == 2 {
            let portal = changed
                .iter()
                .find(|(before, _)| (before.kind, before.subtype) == (14, 4));
            let trigger = changed
                .iter()
                .find(|(before, _)| (before.kind, before.subtype) == (11, 31));
            let (Some((old_portal, portal)), Some((old_trigger, trigger))) = (portal, trigger)
            else {
                return Err(NativeWorldError::new(
                    "configured pair requires the original secret portal and controller",
                ));
            };
            let group = portal.activation_group;
            for (before, after) in &changed {
                let mut unchanged = **after;
                unchanged.activation_group = before.activation_group;
                if unchanged != **before {
                    return Err(NativeWorldError::new(
                        "configured portal pair may change only its activation group",
                    ));
                }
            }
            if old_portal.activation_group <= 0
                || old_portal.activation_group != old_trigger.activation_group
                || group <= 0
                || group != trigger.activation_group
                || group == old_portal.activation_group
            {
                return Err(NativeWorldError::new(
                    "configured portal pair must share a new existing dormant group",
                ));
            }
            let portal_id = bindings
                .authored_scenery_entities
                .get(&portal.index)
                .ok_or_else(|| {
                    NativeWorldError::new("configured secret portal lacks its typed identity")
                })?;
            let trigger_id = bindings
                .authored_switch_entities
                .get(&trigger.index)
                .ok_or_else(|| {
                    NativeWorldError::new("configured secret controller lacks its typed identity")
                })?;
            let mut descriptor = (*self.definition).clone();
            descriptor
                .configure_portal_source(
                    portal_id,
                    trigger_id,
                    group as u16,
                    bindings.level.clone(),
                )
                .map_err(NativeWorldError::new)?;
            let mut simulation_definition = (*self.simulation.session.definition).clone();
            simulation_definition
                .configure_portal_source_for_test(bindings.clone())
                .map_err(NativeWorldError::new)?;
            self.definition = Arc::new(descriptor);
            self.simulation.session.definition = Arc::new(simulation_definition);
            self.simulation
                .session
                .state
                .live
                .mission
                .authored_features_mut()
                .copy_from_slice(features);
            self.simulation.session.rebase();
            return Ok(());
        }
        let [(before, after)] = changed.as_slice() else {
            return Err(NativeWorldError::new(
                "configured item must add exactly one dormant authored row",
            ));
        };
        let item_kind = match (after.kind, after.subtype) {
            (14, 0) => crate::level_definition::ItemKind::PassiveItem,
            (10, 91) => crate::level_definition::ItemKind::RewardJar,
            _ => {
                return Err(NativeWorldError::new(
                    "configured item requires an enabled passive-item or reward-jar constructor",
                ));
            }
        };
        if before.kind != 0
            || before.index != after.index
            || after.index == 0
            || after.activation_group <= 0
        {
            return Err(NativeWorldError::new(
                "configured item must use one unused dormant authored row",
            ));
        }
        let id = bindings
            .authored_item_entities
            .get(&after.index)
            .ok_or_else(|| {
                NativeWorldError::new("configured item has no semantic authored identity")
            })?;
        let entity = bindings
            .level
            .entity(id)
            .ok_or_else(|| NativeWorldError::new("configured item identity has no typed entity"))?;
        if bindings.authored_items.get(&after.index) != Some(&item_kind)
            || entity.kind() != crate::level_definition::LevelEntityKind::Item(item_kind)
            || entity.position().x != after.tile_x
            || entity.position().y != after.tile_y
            || entity.activation_group()
                != Some(sim_definition::adapter::activation_group_id(
                    after.activation_group as u16,
                ))
        {
            return Err(NativeWorldError::new(
                "configured item semantic pose/group must match its source record",
            ));
        }
        let mut descriptor = (*self.definition).clone();
        descriptor
            .configure_item_source_for_test(
                after.index,
                id.clone(),
                item_kind,
                bindings.level.clone(),
            )
            .map_err(NativeWorldError::new)?;
        let mut simulation_definition = (*self.simulation.session.definition).clone();
        simulation_definition
            .configure_item_source_for_test(after.index, id.clone(), item_kind, bindings.clone())
            .map_err(NativeWorldError::new)?;
        self.definition = Arc::new(descriptor);
        self.simulation.session.definition = Arc::new(simulation_definition);
        self.simulation
            .session
            .state
            .live
            .mission
            .authored_features_mut()
            .copy_from_slice(features);
        self.simulation.session.rebase();
        Ok(())
    }

    #[must_use]
    pub fn inspection(&self) -> &NativeWorldInspection {
        &self.inspection
    }

    /// Share the immutable owner data with a retained presentation adapter.
    #[must_use]
    pub fn inspection_arc(&self) -> Arc<NativeWorldInspection> {
        Arc::clone(&self.inspection)
    }

    pub(crate) fn speech_enabled(&self) -> bool {
        self.speech_enabled
    }

    pub(crate) fn set_speech_enabled(&mut self, enabled: bool) {
        self.speech_enabled = enabled;
    }

    /// Source AddEvent2 gates new admissions only. Existing footprints and
    /// linked lights keep their ordinary subtraction/recompute lifecycle.
    pub fn set_dynamic_lighting_enabled(&mut self, enabled: bool) {
        self.simulation.campaign_state_mut().live.economy.dynamic_lighting_enabled = enabled;
    }

    pub(crate) fn show_foreign_objective_text(&self) -> bool {
        self.show_foreign_objective_text
    }

    pub(crate) fn set_show_foreign_objective_text(&mut self, enabled: bool) {
        self.show_foreign_objective_text = enabled;
    }

    /// Step the one authoritative Rust state through the source scheduler.
    pub fn step(&mut self, command: PlayerCommand, speed_index: u8) -> Vec<StepEvidence> {
        self.step_with_tuning(command, speed_index, crate::FlightTuning::CLASSIC)
    }

    pub(crate) fn step_with_tuning(
        &mut self,
        command: PlayerCommand,
        speed_index: u8,
        tuning: crate::FlightTuning,
    ) -> Vec<StepEvidence> {
        let evidence = self
            .simulation
            .step_outer_turn_with_tuning(command, speed_index, tuning);
        if !self
            .simulation
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        {
            self.simulation.service_campaign_objective_presentation(
                self.definition.identity.internal_level,
                self.speech_enabled,
                self.show_foreign_objective_text,
            );
        }
        evidence
    }

    /// Service exactly one source-authorized paused frame without advancing
    /// the gameplay command path or tick.
    pub(crate) fn paused_frame(&mut self) {
        self.simulation.service_paused_outer_frame();
        self.simulation.service_campaign_objective_presentation(
            self.definition.identity.internal_level,
            self.speech_enabled,
            self.show_foreign_objective_text,
        );
    }

    pub fn reset(&mut self) {
        self.simulation.reset();
    }

    pub(crate) fn source_global_context(&self) -> crate::native_world::GlobalSpecialState {
        self.simulation.shared_spells().global_special_state()
    }

    pub(crate) fn install_initial_source_global_context(
        &mut self,
        context: crate::native_world::GlobalSpecialState,
    ) -> Result<(), NativeWorldError> {
        if self.tick() != 0 {
            return Err(NativeWorldError::new(
                "initial source context requires an unpublished mission",
            ));
        }
        self.simulation
            .session
            .state
            .live
            .shared_spells
            .apply_source_global_context(context);
        self.simulation.session.rebase();
        Ok(())
    }

    pub(crate) fn apply_warm_source_global_context(
        &mut self,
        context: crate::native_world::GlobalSpecialState,
    ) {
        self.simulation
            .session
            .state
            .live
            .shared_spells
            .apply_source_global_context(context);
    }

    /// Reconstruct an unpublished continuation from the imported source
    /// arena, before destination direct-start spells consume slots or RNG.
    pub(crate) fn rebuild_for_spell_carry(
        &mut self,
        learned: &[u8],
        selected_family: Option<u8>,
    ) -> Result<Option<u8>, NativeWorldError> {
        if self.tick() != 0 {
            return Err(NativeWorldError::new(
                "spell entry can only replace an unpublished initial campaign",
            ));
        }
        if selected_family.is_some_and(|family| !learned.contains(&family)) {
            return Err(NativeWorldError::new(
                "carried selected spell is not learned",
            ));
        }
        let level = self.simulation.semantic_level_definition().cloned();
        let simulation = self
            .inspection
            .campaign_reset_session_with_spell_entry(&self.definition, Some(learned))?;
        let mut candidate = self.clone();
        candidate.simulation = simulation;
        if let (Some(catalog), Some(owner)) = (
            self.simulation.source_animations.catalog(),
            &self.simulation.source_animation_owner,
        ) {
            // Spell entry rematerializes the unpublished world; reinitialize
            // its validated source bank and preload against that new world.
            candidate.simulation.bind_source_animation_catalog(
                catalog.clone(),
                self.simulation.source_animation_payloads.clone(),
                owner.clone(),
            )?;
        }
        candidate.install_initial_source_global_context(self.source_global_context())?;
        for (player_index, objective) in self.simulation.runtime().objectives.iter().enumerate() {
            candidate
                .simulation
                .session
                .state
                .live
                .core
                .runtime
                .set_level2_objective_text_selector_for(
                    player_index,
                    usize::from(objective.text_selector),
                );
        }
        if let Some(level) = level {
            candidate.apply_semantic_level_variant(&level)?;
        }
        let player = candidate.simulation.shared_spells().local_player();
        let active = player
            .enabled_spell_holders
            .iter()
            .enumerate()
            .filter_map(|(family, holder)| (*holder != 0).then_some(family as u8))
            .collect::<Vec<_>>();
        let selected = selected_family
            .filter(|family| active.contains(family))
            .or_else(|| {
                crate::SOURCE_SPELL_INDEX_MAP
                    .get(usize::from(player.selected_spell_ui_index))
                    .copied()
                    .filter(|family| active.contains(family))
            });
        candidate
            .simulation
            .apply_carried_spell_families(&active, selected)?;
        *self = candidate;
        Ok(selected)
    }

    pub(crate) fn apply_semantic_level_variant(
        &mut self,
        level: &LevelDefinition,
    ) -> Result<(), NativeWorldError> {
        level
            .verify_session_reset_variant_of(self.definition.level_definition())
            .map_err(NativeWorldError::new)?;
        for entity in level.entities() {
            let original = self
                .definition
                .level_definition()
                .entity(entity.id())
                .expect("position variant preserves every package entity");
            let position_changed = entity.position() != original.position();
            let health_changed = entity.creature_health().map(|health| health.max_life())
                != original.creature_health().map(|health| health.max_life());
            let overlap_changed = entity
                .switch_overlap()
                .map(|overlap| overlap.overlap_extent_tiles())
                != original
                    .switch_overlap()
                    .map(|overlap| overlap.overlap_extent_tiles());
            if !position_changed && !health_changed && !overlap_changed {
                continue;
            }
            if position_changed
                && entity.position_access() != LevelEntityPositionAccess::SessionReset
            {
                return Err(NativeWorldError::new(format!(
                    "semantic entity {} is not placement-writable",
                    entity.id().as_str()
                )));
            }
            let binding = self
                .definition
                .movable_entity_binding(entity.id())
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "semantic entity {} has no private MC2 runtime binding",
                        entity.id().as_str()
                    ))
                })?;
            match binding {
                spec::Mc2EntityRuntimeBinding::InitialSource {
                    initial_source_slot,
                    kind,
                    source_id,
                } => {
                    if *kind != entity.kind() {
                        return Err(NativeWorldError::new(format!(
                            "semantic entity {} private binding kind diverges",
                            entity.id().as_str()
                        )));
                    }
                    let source = self
                        .simulation
                        .entities()
                        .get(usize::from(*initial_source_slot))
                        .filter(|source| {
                            source.active
                                && source.index == *initial_source_slot
                                && source.source_id == *source_id
                        })
                        .cloned()
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "semantic entity {} lost its frame-zero MC2 carrier",
                                entity.id().as_str()
                            ))
                        })?;
                    let original_tile = original.position();
                    if source.position.x >> 8 != original_tile.x
                        || source.position.y >> 8 != original_tile.y
                    {
                        return Err(NativeWorldError::new(format!(
                            "semantic entity {} private binding position diverges",
                            entity.id().as_str()
                        )));
                    }
                    if position_changed {
                        let position = entity.position();
                        self.simulation.set_level2_entity_position(
                            *initial_source_slot,
                            Position {
                                x: position.x.wrapping_shl(8) | (source.position.x & 0xff),
                                y: position.y.wrapping_shl(8) | (source.position.y & 0xff),
                                z: source.position.z,
                            },
                        )?;
                    }
                    if health_changed {
                        let package_max_life = original
                            .creature_health()
                            .map(|health| health.max_life())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic entity {} package carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?
                            .cast_signed();
                        let authored_max_life = entity
                            .creature_health()
                            .map(|health| health.max_life())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic entity {} authored carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?
                            .cast_signed();
                        if source.max_life != package_max_life || source.life != package_max_life {
                            return Err(NativeWorldError::new(format!(
                                "semantic entity {} frame-zero health diverges from its package definition",
                                entity.id().as_str()
                            )));
                        }
                        let source = &mut self.simulation.session.state.live.economy.entities
                            [usize::from(*initial_source_slot)];
                        source.bind_frame_zero_max_life(authored_max_life);
                    }
                    if overlap_changed {
                        let package_extent = original
                            .switch_overlap()
                            .map(|overlap| overlap.overlap_extent_tiles())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic switch {} package carrier has no overlap binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        let authored_extent = entity
                            .switch_overlap()
                            .map(|overlap| overlap.overlap_extent_tiles())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic switch {} authored carrier has no overlap binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        self.simulation.bind_frame_zero_switch_overlap_variant(
                            *initial_source_slot,
                            package_extent,
                            authored_extent,
                        )?;
                    }
                    if *kind
                        == crate::level_definition::LevelEntityKind::Creature(
                            crate::level_definition::CreatureKind::Goat,
                        )
                    {
                        let package_max_life = original
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic Goat {} package carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        let authored_max_life = entity
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic Goat {} authored carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        let authored_position = self.simulation.session.state.live.economy.entities
                            [usize::from(*initial_source_slot)]
                        .position;
                        self.simulation
                            .session
                            .state
                            .live
                            .goats
                            .bind_frame_zero_semantic_variant(
                                *initial_source_slot,
                                source.position,
                                authored_position,
                                package_max_life,
                                authored_max_life,
                            )
                            .map_err(NativeWorldError::new)?;
                    }
                    if *kind
                        == crate::level_definition::LevelEntityKind::Creature(
                            crate::level_definition::CreatureKind::Villager,
                        )
                    {
                        let authored_position = self.simulation.session.state.live.economy.entities
                            [usize::from(*initial_source_slot)]
                        .position;
                        let package_max_life = original
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic Villager {} package carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        let authored_max_life = entity
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic Villager {} authored carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        self.simulation
                            .session
                            .state
                            .live
                            .ordinary
                            .bind_frame_zero_villager_variant(
                                *initial_source_slot,
                                source.position,
                                authored_position,
                                package_max_life,
                                authored_max_life,
                            )?;
                    }
                    if *kind
                        == crate::level_definition::LevelEntityKind::Building(
                            crate::level_definition::BuildingKind::House,
                        )
                    {
                        let authored_position = self.simulation.session.state.live.economy.entities
                            [usize::from(*initial_source_slot)]
                        .position;
                        self.simulation
                            .session
                            .state
                            .live
                            .ordinary
                            .bind_frame_zero_house_position(
                                *initial_source_slot,
                                source.position,
                                authored_position,
                            )?;
                    }
                }
                spec::Mc2EntityRuntimeBinding::InitialMultipartFamily {
                    root_source_slot,
                    body_source_slots,
                    kind,
                    source_id,
                } => {
                    if *kind != entity.kind() {
                        return Err(NativeWorldError::new(format!(
                            "semantic entity {} private multipart binding kind diverges",
                            entity.id().as_str()
                        )));
                    }
                    let source = self
                        .simulation
                        .entities()
                        .get(usize::from(*root_source_slot))
                        .filter(|source| {
                            source.active
                                && source.index == *root_source_slot
                                && source.source_id == *source_id
                        })
                        .ok_or_else(|| {
                            NativeWorldError::new(format!(
                                "semantic entity {} lost its frame-zero multipart root",
                                entity.id().as_str()
                            ))
                        })?;
                    let original_tile = original.position();
                    if source.position.x >> 8 != original_tile.x
                        || source.position.y >> 8 != original_tile.y
                    {
                        return Err(NativeWorldError::new(format!(
                            "semantic entity {} private multipart binding position diverges",
                            entity.id().as_str()
                        )));
                    }
                    if position_changed || health_changed {
                        let package_max_life = original
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic multipart entity {} package carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        let authored_max_life = entity
                            .creature_health()
                            .map(|health| health.max_life().cast_signed())
                            .ok_or_else(|| {
                                NativeWorldError::new(format!(
                                    "semantic multipart entity {} authored carrier has no health binding",
                                    entity.id().as_str()
                                ))
                            })?;
                        self.simulation.bind_level2_imported_multipart_variant(
                            *root_source_slot,
                            body_source_slots,
                            original_tile,
                            entity.position(),
                            package_max_life,
                            authored_max_life,
                        )?;
                    }
                }
                spec::Mc2EntityRuntimeBinding::AuthoredFeature {
                    authored_record,
                    kind,
                } => {
                    if *kind != entity.kind() {
                        return Err(NativeWorldError::new(format!(
                            "semantic entity {} private authored-feature kind diverges",
                            entity.id().as_str()
                        )));
                    }
                    if position_changed {
                        let position = entity.position();
                        self.simulation
                            .session
                            .state
                            .live
                            .mission
                            .set_authored_feature_tile(*authored_record, position.x, position.y)
                            .map_err(|error| NativeWorldError::new(error.to_string()))?;
                    }
                }
            }
        }
        let active_definition = self
            .definition
            .with_level_definition_variant(level.clone())
            .map_err(NativeWorldError::new)?;
        self.simulation
            .replace_semantic_level_definition(level.clone())
            .map_err(NativeWorldError::new)?;
        self.definition = Arc::new(active_definition);
        self.simulation.session.rebase();
        Ok(())
    }

    pub(crate) fn apply_local_player_fireball_mana_cost(
        &mut self,
        mana_cost: i32,
    ) -> Result<(), NativeWorldError> {
        self.simulation
            .bind_local_player_fireball_mana_cost(mana_cost)
    }

    pub(crate) fn apply_speed_multipliers(&mut self, multipliers: [i32; 3]) {
        self.simulation.apply_speed_multipliers(multipliers);
    }

    pub(crate) fn apply_fireball_launch_sound(&mut self, source_sound_id: u16) {
        self.simulation.bind_fireball_launch_sound(source_sound_id);
    }

    pub(crate) fn apply_enemy_goat_policy(
        &mut self,
        binding: crate::game_definition::DefinitionGoatPolicyBinding,
    ) {
        self.simulation.bind_enemy_goat_policy(binding);
    }

    pub(crate) fn set_runtime_generation(&mut self, generation: u64) {
        self.simulation.session.generation = generation;
        self.simulation.session.state.scratch = self
            .simulation
            .session
            .state
            .live
            .reset_scratch(generation, false);
    }

    /// Current runtime generation of the one folded session authority.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.simulation.session.generation
    }

    /// Current source tick of the one folded session authority.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.simulation.state.tick
    }

    /// Cursor immediately after the last retained production effect.
    #[must_use]
    pub fn effect_cursor(&self) -> EffectCursor {
        self.simulation.effect_journal().cursor(self.generation())
    }

    /// Page the canonical ordered production journal without exposing a
    /// mutable journal or any Level-2-specific event sidecar.
    pub fn effects_after(
        &self,
        cursor: EffectCursor,
        capacity: usize,
    ) -> Result<EffectPage, EffectPageError> {
        self.simulation
            .effect_journal()
            .page(self.generation(), cursor, capacity)
    }

    #[must_use]
    pub fn state_hash(&self) -> u64 {
        self.simulation.campaign_state_hash()
    }

    #[must_use]
    pub fn terrain_hash(&self) -> u64 {
        self.simulation.terrain_hash()
    }

    pub(crate) fn state_and_terrain_hashes(&self) -> (u64, u64) {
        let terrain_hash = self.terrain_hash();
        (
            self.simulation.campaign_state_hash_with_terrain_hash(terrain_hash),
            terrain_hash,
        )
    }

    #[must_use]
    pub fn events(&self) -> &[Mc2DispatchEvent] {
        self.simulation.dispatch_events()
    }

    /// Borrow the simulation shared by every campaign mission.
    #[doc(hidden)]
    #[must_use]
    pub fn simulation(&self) -> &Mc2Simulation {
        &self.simulation
    }

    /// Mutate this campaign session's sole simulation authority.
    #[doc(hidden)]
    pub fn simulation_mut(&mut self) -> &mut Mc2Simulation {
        &mut self.simulation
    }
}

fn requires_source_animation_assets(definition: &CampaignDefinition) -> bool {
    let enabled = definition.campaign_spec.presentation_recipe.terrain_graphics_flags
        & u32::from(crate::native_world::creatures::creature_10::SOURCE_FEATURE_MASK) != 0;
    enabled && definition.creature_bindings().values()
        .chain(definition.initial_creature_bindings().values())
        .any(|kind| *kind == crate::level_definition::CreatureKind::FinalBoss)
}

fn missing_source_animation_assets() -> NativeWorldError {
    NativeWorldError::new("missing gameplay animation assets: enabled final-boss producer requires the complete qualified TMAP catalog; open with its owner material cache")
}

#[cfg(test)]
mod tests {
    use super::{CampaignOpenError, CampaignSelection, CampaignSession, Mc2Simulation};
    use crate::{
        FlightEvent, FlightWorld, PlayerCommand,
        campaign::{
            dispatch::CAMPAIGN_BEHAVIOR_REGISTRY,
            schedule::{ScheduleReceipt, SourcePass, VisitKind},
            spec::{BehaviorSelector, CampaignDefinition, ValidationStage},
        },
        native_world::{
            EntitySpecialRelation, FireballProjectileOrigin, Level2AuthoredClass5Source,
            Level2EconomyEvent, Level2NyphurDispatchCall, Mc2DispatchEvent, NativeWorld,
        },
    };
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
    };

    fn level_two_fixture_path() -> PathBuf {
        std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        )
    }

    fn descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn level_three_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    fn level_one_descriptor_fixture_path() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq8-level1-collision-v5-night")
            },
            PathBuf::from,
        )
    }

    #[test]
    fn opening_objective_speech_is_emitted_by_real_outer_and_paused_frames() {
        use crate::campaign::{journal::CampaignEffect, objective_speech::ordinary_speech_segment};
        for (root, selection) in [
            (
                level_one_descriptor_fixture_path(),
                CampaignSelection::LEVEL1,
            ),
            (descriptor_fixture_path(), CampaignSelection::LEVEL2),
            (
                level_three_descriptor_fixture_path(),
                CampaignSelection::LEVEL3,
            ),
        ] {
            let mut session = CampaignSession::open(root, selection).unwrap();
            let local = usize::from(session.simulation.runtime().local_player_slot);
            let owner = usize::from(session.simulation.players()[local].player_color_index());
            let objective = session.simulation.runtime().objectives[owner].text_selector;
            let expected =
                ordinary_speech_segment(selection.internal_level as u8, objective + 1).unwrap();
            for _ in 0..10 {
                session.step(PlayerCommand::default(), 0);
            }
            let speech: Vec<_> = session
                .simulation
                .effect_journal()
                .records()
                .iter()
                .filter_map(|record| match record.effect {
                    CampaignEffect::ObjectivePresentation(actions) => actions.speech,
                    _ => None,
                })
                .collect();
            assert_eq!(
                speech,
                [expected],
                "ordinary opening emits exactly its source objective segment"
            );
            let palette_sample = |session: &CampaignSession| {
                let mut colors = [0_u8, 0, 0, 4, 8, 12];
                session.simulation.field_palette().apply_rgb8(&mut colors);
                colors
            };
            assert_eq!(
                palette_sample(&session),
                [0, 0, 0, 196, 200, 204],
                "real objective request reaches the indexed RGB6 palette consumer"
            );
            let tick = session.simulation.state.tick;
            let phase = session.simulation.runtime().objective_presentation_phase;
            session.paused_frame();
            assert_eq!(session.simulation.state.tick, tick);
            assert_eq!(
                session.simulation.runtime().objective_presentation_phase,
                phase.wrapping_add(1)
            );
            assert_eq!(
                palette_sample(&session),
                [0, 0, 0, 196, 200, 204],
                "the first case9 call installs the fade's source palette"
            );
            session.paused_frame();
            assert_eq!(session.simulation.state.tick, tick);
            assert_eq!(
                palette_sample(&session),
                [0, 0, 0, 184, 188, 192],
                "paused outer frames advance the same sixteen-step fade"
            );
        }
    }

    #[test]
    fn ordinary_source_twenty_objective_speech_and_textbox_follow_outer_frames() {
        use crate::campaign::{journal::CampaignEffect, objective_speech::ordinary_speech_segment};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ammyridia-source-closure-20261114/level20");
        let mut session =
            CampaignSession::open(root, CampaignSelection::ordinary(20).unwrap()).unwrap();
        session.set_show_foreign_objective_text(true);
        let local = usize::from(session.simulation.runtime().local_player_slot);
        let owner = usize::from(session.simulation.players()[local].player_color_index());
        let objective = session.simulation.runtime().objectives[owner].text_selector;
        let expected = ordinary_speech_segment(19, objective + 1).unwrap();
        let mut heard = false;
        for _ in 0..16 {
            session.step(PlayerCommand::default(), 0);
            assert!(!session
                .simulation
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })));
            if session
                .simulation
                .effect_journal()
                .records()
                .iter()
                .any(|record| {
                    matches!(record.effect, CampaignEffect::ObjectivePresentation(actions) if actions.speech == Some(expected))
                })
            {
                heard = true;
                break;
            }
        }
        assert!(
            heard,
            "the real source 20 opening selects its source objective speech"
        );
        assert_eq!(session.simulation.runtime().objective_textbox_ticks, 200);
        session.step(PlayerCommand::default(), 0);
        assert_eq!(session.simulation.runtime().objective_textbox_ticks, 199);
        let tick = session.simulation.state.tick;
        session.paused_frame();
        assert_eq!(session.simulation.state.tick, tick);
        assert_eq!(session.simulation.runtime().objective_textbox_ticks, 198);
    }

    #[test]
    fn foreign_objective_auto_display_reaches_ordinary_and_paused_presentation() {
        use crate::campaign::journal::CampaignEffect;
        let baseline = CampaignSession::open(
            level_one_descriptor_fixture_path(),
            CampaignSelection::LEVEL1,
        )
        .unwrap();
        assert!(!baseline.show_foreign_objective_text());
        for show_foreign_text in [false, true] {
            for paused in [false, true] {
                let mut session = baseline.clone();
                session.set_show_foreign_objective_text(show_foreign_text);
                let start_tick = session.simulation.state.tick;
                for _ in 0..10 {
                    if paused {
                        session.paused_frame();
                    } else {
                        session.step(PlayerCommand::default(), 0);
                    }
                }
                let speech_textboxes = session
                    .simulation
                    .effect_journal()
                    .records()
                    .iter()
                    .filter_map(|record| match record.effect {
                        CampaignEffect::ObjectivePresentation(actions)
                            if actions.speech.is_some() =>
                        {
                            Some(actions.textbox_ticks)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(speech_textboxes, [show_foreign_text.then_some(200)]);
                if paused {
                    assert_eq!(session.simulation.state.tick, start_tick);
                }
                session.reset();
                assert_eq!(session.show_foreign_objective_text(), show_foreign_text);
            }
        }
    }

    #[test]
    fn night_environment_rejects_other_package_before_materialization() {
        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let cache = generated.join("mc-oi62-environment-level1-v2");
        CampaignSession::open_with_owner_cache(
            level_one_descriptor_fixture_path(),
            &cache,
            CampaignSelection::LEVEL1,
        )
        .expect("matching real Night package opens");
        let other_cache = generated.join("mc-oi62-environment-level25-v2");
        crate::day_indexed_frame::DayIndexedFrameAssets::load_night(&other_cache)
            .expect("other real Night cache is valid on its own");
        let error = CampaignSession::open_with_owner_cache(
            level_one_descriptor_fixture_path(),
            other_cache,
            CampaignSelection::LEVEL1,
        )
        .expect_err("Night cache from another package must not materialize");
        assert!(matches!(error, CampaignOpenError::Materialization(_)));
        assert!(error.to_string().contains("material cache does not match"));
        CampaignSession::open_with_owner_cache(
            level_one_descriptor_fixture_path(),
            generated.join("level1-unity-asset-cache-plan29-final"),
            CampaignSelection::LEVEL1,
        )
        .expect("sealed legacy Night resources retain their deliberate reuse contract");
    }

    #[test]
    fn cf11_level2_opens_steps_resets_through_descriptor() {
        let legacy_inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("legacy Level 2 compatibility package loads");
        let unbound_legacy = legacy_inspection
            .level2_reset_session()
            .expect("legacy Level 2 compatibility session opens");
        let mut campaign =
            CampaignSession::open(descriptor_fixture_path(), CampaignSelection::LEVEL2)
                .expect("qualified campaign definition opens one Rust session");
        // The descriptor boot binds reset-hashed Castle construction data and
        // semantic creature parameters. Compare its folded compatibility path
        // against the same definition, independently decoded from the owner export.
        let mut legacy = legacy_inspection
            .level2_reset_session_from_definition(campaign.definition())
            .expect("legacy inspection binds the qualified campaign definition");
        assert!(unbound_legacy.semantic_level_definition().is_none());
        assert!(!unbound_legacy.economy().has_castle_stage_footprint(1));
        assert_ne!(
            campaign.state_hash(),
            unbound_legacy.campaign_state_hash(),
            "an unbound legacy reset is not the descriptor's gameplay baseline"
        );
        assert_eq!(
            legacy.semantic_level_definition(),
            Some(campaign.definition().level_definition())
        );
        for stage in 1..=3 {
            assert!(legacy.economy().has_castle_stage_footprint(stage));
            assert!(
                campaign
                    .simulation()
                    .economy()
                    .has_castle_stage_footprint(stage)
            );
        }

        assert_eq!(
            campaign.definition().completed_stage,
            ValidationStage::ResetState
        );
        assert_eq!(campaign.definition().identity, CampaignSelection::LEVEL2);
        assert_ne!(
            campaign.definition().package_sha256,
            campaign.inspection().package_sha256(),
            "campaign envelope and wrapped owner export have distinct identities"
        );
        assert_eq!(
            campaign.inspection().package_sha256(),
            legacy_inspection.package_sha256(),
            "materialized world and owner cache bind to the wrapped export identity"
        );
        assert_eq!(campaign.state_hash(), legacy.campaign_state_hash());
        assert_eq!(campaign.terrain_hash(), legacy.terrain_hash());
        let initial_state = campaign.state_hash();
        let initial_terrain = campaign.terrain_hash();

        let command = PlayerCommand::default();
        let descriptor_evidence = campaign.step(command, 0);
        let legacy_evidence = legacy.step_outer_turn(command, 0);
        assert_eq!(descriptor_evidence, legacy_evidence);
        assert_eq!(campaign.events(), legacy.dispatch_events());
        assert_eq!(campaign.state_hash(), legacy.campaign_state_hash());
        assert_eq!(campaign.terrain_hash(), legacy.terrain_hash());
        let stepped_state = campaign.state_hash();
        let stepped_terrain = campaign.terrain_hash();
        let stepped_events = campaign.events().to_vec();

        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_state);
        assert_eq!(campaign.terrain_hash(), initial_terrain);
        assert!(campaign.events().is_empty());

        let replay_evidence = campaign.step(command, 0);
        assert_eq!(replay_evidence, descriptor_evidence);
        assert_eq!(campaign.events(), stepped_events.as_slice());
        assert_eq!(campaign.state_hash(), stepped_state);
        assert_eq!(campaign.terrain_hash(), stepped_terrain);
    }

    #[test]
    fn cf11_descriptor_preflight_fails_before_session() {
        static MATERIALIZER_CALLED: AtomicBool = AtomicBool::new(false);
        MATERIALIZER_CALLED.store(false, Ordering::SeqCst);
        let mismatched_selection = CampaignSelection {
            external_level: 1,
            internal_level: 0,
            map_type_id: 1,
        };
        let result = CampaignSession::open_with_materializer(
            &descriptor_fixture_path(),
            mismatched_selection,
            |_| {
                MATERIALIZER_CALLED.store(true, Ordering::SeqCst);
                panic!("materializer must not run after failed descriptor preflight")
            },
        );
        let CampaignOpenError::Preflight(error) = result.expect_err("identity mismatch must fail")
        else {
            panic!("identity mismatch escaped descriptor preflight");
        };
        assert_eq!(error.stage, ValidationStage::OuterManifest);
        assert!(!MATERIALIZER_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn cf15_level1_real_reset_families_enter_shared_scheduler() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let session = campaign.simulation();

        let raw_slots = |class: u8, model: u8, action: u8| {
            session
                .entities()
                .iter()
                .filter(|row| {
                    row.active && row.class == class && row.model == model && row.action == action
                })
                .map(|row| row.index)
                .collect::<Vec<_>>()
        };
        let goat_slots = raw_slots(5, 1, 15);
        let villager_slots = raw_slots(5, 13, 111);
        let dolmen_slots = raw_slots(2, 2, 6);
        let mut holder_slots = raw_slots(15, 0, 0);
        holder_slots.extend(raw_slots(15, 1, 3));
        holder_slots.sort_unstable();

        assert_eq!(goat_slots.len(), 33);
        assert_eq!(villager_slots.len(), 28);
        assert_eq!(dolmen_slots.len(), 3);
        assert_eq!(holder_slots.len(), 2);
        assert_eq!(session.goats().len(), goat_slots.len());
        assert_eq!(session.ordinary().villagers().len(), villager_slots.len());
        assert_eq!(session.ordinary().dolmens().len(), dolmen_slots.len());
        assert_eq!(session.shared_spells().holders().len(), holder_slots.len());
        assert_eq!(
            session
                .goats()
                .iter()
                .map(|row| row.entity_index)
                .collect::<Vec<_>>(),
            goat_slots
        );
        assert_eq!(
            session
                .ordinary()
                .villagers()
                .iter()
                .map(|row| row.entity_index)
                .collect::<Vec<_>>(),
            villager_slots
        );
        assert_eq!(
            session
                .shared_spells()
                .holders()
                .iter()
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            holder_slots
        );

        let initial_hash = campaign.state_hash();
        let goat_phases = session
            .goats()
            .iter()
            .map(|row| (row.entity_index, row.phase))
            .collect::<Vec<_>>();
        let villager_phases = session
            .ordinary()
            .villagers()
            .iter()
            .map(|row| (row.entity_index, row.phase))
            .collect::<Vec<_>>();
        let holder_phases = session
            .shared_spells()
            .holders()
            .iter()
            .map(|row| (row.index, row.phase))
            .collect::<Vec<_>>();
        let mut hydrated_slots = goat_slots;
        hydrated_slots.extend(villager_slots);
        hydrated_slots.extend(dolmen_slots);
        hydrated_slots.extend(holder_slots);
        hydrated_slots.sort_unstable();

        let _ = campaign.step(PlayerCommand::default(), 0);
        let receipts = campaign.simulation().cf10_production_schedule_receipts();
        for source_slot in hydrated_slots {
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::SlotResolved {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        kind: VisitKind::Callback,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} was not resolved as a callback"
            );
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} did not complete its callback"
            );
            assert!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::EpilogueCompleted {
                        pass: SourcePass::UpdateEntities,
                        scan: 0,
                        slot,
                        ..
                    } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} did not complete its phase epilogue"
            );
            assert!(
                campaign.events().iter().all(|event| !matches!(
                    event,
                    Mc2DispatchEvent::Error { source_slot: slot, .. } if *slot == source_slot
                )),
                "hydrated Level 1 slot {source_slot} emitted a dispatch error"
            );
        }
        let session = campaign.simulation();
        for (slot, phase) in goat_phases {
            assert_eq!(
                session
                    .goats()
                    .iter()
                    .find(|row| row.entity_index == slot)
                    .expect("Goat remains live after one ordinary pass")
                    .phase,
                phase.wrapping_add(1)
            );
        }
        for (slot, phase) in villager_phases {
            assert_eq!(
                session
                    .ordinary()
                    .villagers()
                    .iter()
                    .find(|row| row.entity_index == slot)
                    .expect("Villager remains live after one ordinary pass")
                    .phase,
                phase.wrapping_add(1)
            );
        }
        for (slot, phase) in holder_phases {
            let holder = session
                .shared_spells()
                .holders()
                .iter()
                .find(|row| row.index == slot)
                .expect("initial spell holder remains live after one ordinary pass");
            assert_eq!(holder.phase, phase.wrapping_add(1));
            assert_eq!(session.entities()[usize::from(slot)].phase, holder.phase);
        }

        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_hash);
        assert_eq!(campaign.simulation().goats().len(), 33);
        assert_eq!(campaign.simulation().ordinary().villagers().len(), 28);
        assert_eq!(campaign.simulation().ordinary().dolmens().len(), 3);
        assert_eq!(campaign.simulation().shared_spells().holders().len(), 2);
    }

    #[test]
    fn cf15_level1_group1_materializes_castle_pickup_switches_and_trees_in_source_order() {
        const GROUP_ROWS: [u16; 21] = [
            31, 99, 104, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172,
            173, 174, 175,
        ];
        const TREE_ROWS: [u16; 18] = [
            155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 168, 169, 170, 171, 172, 173, 174,
            175,
        ];
        const PRODUCER_SLOT: u16 = 48;
        const SPELL_SLOT: u16 = 178;
        const FIRST_TREE_SLOT: u16 = 181;

        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let initial_hash = campaign.state_hash();
        let session = campaign.simulation();
        let producer = &session.entities()[usize::from(PRODUCER_SLOT)];
        assert_eq!(
            (
                producer.active,
                producer.class,
                producer.model,
                producer.action,
                producer.source_id,
            ),
            (true, 11, 1, 1, 1),
            "the real package must drive group 1 through its ordinary switch producer"
        );
        let producer_feature = session
            .mission()
            .authored_features()
            .iter()
            .find(|row| row.index == 2)
            .expect("package row 2 is the reset producer source");
        assert_eq!(
            (
                producer_feature.kind,
                producer_feature.subtype,
                producer_feature.activation_group,
                producer_feature.stage_tag,
                producer_feature.word_10,
            ),
            (11, 1, 0, 1, 64)
        );
        let producer_position = producer.position;
        let producer_half_width = i16::from_le_bytes(
            producer.record_bytes()[88..90]
                .try_into()
                .expect("producer render half-width extent"),
        );
        let producer_half_depth = i16::from_le_bytes(
            producer.record_bytes()[90..92]
                .try_into()
                .expect("producer render half-depth extent"),
        );
        assert_eq!((producer_half_width, producer_half_depth), (16_384, 16_384));
        let package_group_rows = session
            .mission()
            .authored_features()
            .iter()
            .filter(|row| row.activation_group == 1 && row.kind != 0)
            .map(|row| row.index)
            .collect::<Vec<_>>();
        assert_eq!(package_group_rows, GROUP_ROWS);
        assert!(session.economy().authored_trees().is_empty());
        assert!(session.economy().spell_pickups().is_empty());

        let player_entity_index = session.state.entity_index;
        let player = &session.entities()[usize::from(player_entity_index)];
        let player_half_width = i16::from_le_bytes(
            player.record_bytes()[88..90]
                .try_into()
                .expect("player render half-width extent"),
        );
        let player_half_depth = i16::from_le_bytes(
            player.record_bytes()[90..92]
                .try_into()
                .expect("player render half-depth extent"),
        );
        let mut inside_evidence = Vec::new();
        for _ in 0..7 {
            inside_evidence.push(campaign.step(PlayerCommand::default(), 0));
            assert!(campaign.simulation().economy().authored_trees().is_empty());
        }
        assert_eq!(
            campaign.simulation().entities()[usize::from(PRODUCER_SLOT)].phase,
            56
        );
        let inside_due_evidence = campaign.step(PlayerCommand::default(), 0);
        assert!(campaign.simulation().economy().authored_trees().is_empty());
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
        )));
        assert!(crate::level1_mission::source_entities_overlap_xy(
            producer_position,
            producer_half_width,
            producer_half_depth,
            campaign.simulation().state.position,
            player_half_width,
            player_half_depth,
        ));

        let travel_command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let mut travel_evidence = Vec::new();
        let mut global_before_producer = None;
        let mut producer_evidence = None;
        for _ in 0..256 {
            let global_before = campaign.simulation().source_global_rng();
            let evidence = campaign.step(travel_command, 0);
            let fired = campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        source_slot: PRODUCER_SLOT,
                        group: 1,
                        ..
                    }
                )
            });
            travel_evidence.push(evidence.clone());
            if fired {
                global_before_producer = Some(global_before);
                producer_evidence = Some(evidence);
                break;
            }
        }
        let global_before_producer =
            global_before_producer.expect("commanded flight reaches the producer non-overlap");
        let producer_evidence = producer_evidence.expect("group 1 fires on a natural phase visit");
        assert!(!crate::level1_mission::source_entities_overlap_xy(
            producer_position,
            producer_half_width,
            producer_half_depth,
            campaign.simulation().state.position,
            player_half_width,
            player_half_depth,
        ));
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(campaign.events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                pass: 0,
                source_slot: PRODUCER_SLOT,
                group: 1,
                authored_indices,
            } if authored_indices.as_ref() == GROUP_ROWS
        )));
        assert!(campaign.events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                pass: 0,
                source_slot: SPELL_SLOT,
                family: 2,
                result: None,
            }
        )));

        let session = campaign.simulation();
        let spell_source = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.authored_index == 31)
            .copied()
            .expect("group-1 row 31 materializes through the shared spell arena");
        assert_eq!(
            (
                spell_source.source_entity_index,
                spell_source.family,
                spell_source.action,
                spell_source.active,
            ),
            (SPELL_SLOT, 2, 7, true)
        );
        let spell_projection = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == SPELL_SLOT)
            .expect("row 31 has a shared class-15 callback projection");
        assert_eq!(
            (
                spell_projection.active,
                spell_projection.source_id,
                spell_projection.class,
                spell_projection.model,
                spell_projection.action,
                spell_projection.yaw,
                spell_projection.pitch,
                spell_projection.fov,
                spell_projection.particle_parameter_index,
                spell_projection.parameter_relation,
                spell_projection.special_relation,
            ),
            (
                true,
                SPELL_SLOT,
                15,
                2,
                7,
                0,
                0,
                0,
                Some(77),
                Some(59),
                EntitySpecialRelation::GlobalSpecialState,
            )
        );
        assert_eq!(
            (
                spell_projection.subspell_index,
                spell_projection.activation_value,
                spell_projection.byte_3b,
                spell_projection.mana_regen,
                spell_projection.max_mana,
                spell_projection.mana_cost,
                spell_projection.mana,
            ),
            (0, 101, 1, 0, 1_000, 1_000, 9)
        );
        assert_eq!(
            &spell_projection.record_bytes()[46..54],
            &[0, 0, 0, 0, 0, 0, 101, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[140..152],
            &[0, 0, 0, 0, 232, 3, 0, 0, 9, 0, 0, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[30..40],
            &[178, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&spell_projection.record_bytes()[44..46], &[0, 0]);
        assert_eq!(
            &spell_projection.record_bytes()[86..96],
            &[75, 0, 0, 3, 0, 3, 0, 5, 77, 0]
        );
        assert_eq!(
            &spell_projection.record_bytes()[164..168],
            &60_u32.to_le_bytes()
        );
        assert_eq!(
            &spell_projection.record_bytes()[168..176],
            &[2, 0, 0, 0, 1, 0, 0, 0]
        );

        let trees = session.economy().authored_trees().to_vec();
        assert_eq!(
            trees
                .iter()
                .map(|row| row.authored_index)
                .collect::<Vec<_>>(),
            TREE_ROWS
        );
        assert!(trees.iter().all(|tree| {
            session.mission().authored_features().iter().any(|feature| {
                feature.index == tree.authored_index
                    && feature.activation_group == 1
                    && (feature.kind, feature.subtype) == (2, 0)
            })
        }));
        let first_tree = trees
            .iter()
            .find(|row| row.authored_index == TREE_ROWS[0])
            .expect("first source-ordered Level 1 tree");
        assert_eq!(first_tree.source_entity_index, FIRST_TREE_SLOT);
        let next_rng16 = |seed: u16| seed.wrapping_mul(9_377).wrapping_add(9_439);
        let pass_global = global_before_producer
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        let life_rng = next_rng16(
            first_tree
                .source_entity_index
                .wrapping_add(pass_global as u16),
        );
        let x_rng = next_rng16(life_rng);
        let y_rng = next_rng16(x_rng);
        let particle_rng = next_rng16(y_rng);
        let feature = session
            .mission()
            .authored_features()
            .iter()
            .find(|row| row.index == TREE_ROWS[0])
            .expect("row 155 remains in the package census");
        let authored_position = feature.world_position(
            session
                .canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let expected_x = authored_position
            .x
            .wrapping_add((x_rng & 0x3f).wrapping_sub(32));
        let expected_y = authored_position
            .y
            .wrapping_add((y_rng & 0x3f).wrapping_sub(32));
        assert_eq!(
            (
                first_tree.source_class,
                first_tree.model,
                first_tree.action,
                first_tree.entity_rng,
                first_tree.particle_index,
                first_tree.control_flags,
                first_tree.position.x,
                first_tree.position.y,
                first_tree.position.z,
            ),
            (
                2,
                0,
                0,
                particle_rng,
                if particle_rng & 1 == 0 { 83 } else { 84 },
                0x0002_000c,
                expected_x,
                expected_y,
                session.canonical_terrain().ground_height(crate::Position {
                    x: expected_x,
                    y: expected_y,
                    z: authored_position.z,
                }),
            )
        );
        assert!(
            session
                .economy()
                .map_chains()
                .entity_chain((expected_x >> 8) as u8, (expected_y >> 8) as u8)
                .contains(&FIRST_TREE_SLOT)
        );

        let receipts = session.cf10_production_schedule_receipts();
        for source_slot in
            std::iter::once(SPELL_SLOT).chain(trees.iter().map(|row| row.source_entity_index))
        {
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::SlotResolved {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    kind: VisitKind::Callback,
                    ..
                } if *slot == source_slot
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    ..
                } if *slot == source_slot
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    ..
                } if *slot == source_slot
            )));
        }

        let producer_events = campaign.events().to_vec();
        let pickup_position = spell_source.position;
        campaign
            .simulation_mut()
            .set_level2_entity_position(player_entity_index, pickup_position)
            .expect("move the real local player through the production position carrier");
        let mut acquisition_evidence = Vec::new();
        let mut acquired = None;
        for attempt in 0..4 {
            acquisition_evidence.push(campaign.step(PlayerCommand::default(), 0));
            assert!(
                campaign
                    .events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
            if attempt == 0 {
                let reclaimed_producer =
                    &campaign.simulation().entities()[usize::from(PRODUCER_SLOT)];
                assert_eq!(
                    (reclaimed_producer.active, reclaimed_producer.class),
                    (false, 0),
                    "the hidden producer is reclaimed on the following source pass"
                );
                assert!(campaign.events().iter().all(|event| !matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
                )));
            }
            acquired = campaign.events().iter().find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    source_slot: SPELL_SLOT,
                    family: 2,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            });
            if acquired.is_some() {
                break;
            }
        }
        let acquired = acquired.expect("the naturally phased family-2 source is acquired");
        assert_eq!(
            (
                acquired.family,
                acquired.source_slot,
                acquired.holder_action,
                acquired.parent_entity_index,
                acquired.presentation_timer,
            ),
            (2, SPELL_SLOT, 6, player_entity_index, 64)
        );
        let session = campaign.simulation();
        let acquired_projection = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == SPELL_SLOT)
            .expect("acquired family-2 projection remains the source holder");
        assert_eq!(acquired_projection.action, 6);
        assert_eq!(
            u16::from_le_bytes(
                acquired_projection.record_bytes()[44..46]
                    .try_into()
                    .expect("source parent relation extent")
            ),
            player_entity_index
        );
        let local_player = &session.shared_spells().players()[0];
        assert_eq!(local_player.enabled_spell_holders[2], SPELL_SLOT);
        assert_eq!(local_player.acquired_spell_flags[2], 1);
        assert_eq!(session.players()[0].enabled_spell_holders[2], SPELL_SLOT);
        assert_eq!(session.players()[0].acquired_spell_flags[2], 1);
        let pickup_sounds = session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id: 18,
                        ..
                    } if *entity_index == player_entity_index
                )
            })
            .count();
        assert_eq!(pickup_sounds, 1);

        let first_final_hash = campaign.state_hash();
        let first_events = campaign.events().to_vec();
        let first_trees = campaign.simulation().economy().authored_trees().to_vec();
        let first_spell_projection = campaign
            .simulation()
            .shared_spells()
            .dynamic_pickups()
            .to_vec();
        campaign.reset();
        assert_eq!(campaign.state_hash(), initial_hash);
        assert!(campaign.simulation().economy().authored_trees().is_empty());
        assert!(
            campaign
                .simulation()
                .shared_spells()
                .dynamic_pickups()
                .is_empty()
        );

        let mut replay_inside = Vec::new();
        for _ in 0..7 {
            replay_inside.push(campaign.step(PlayerCommand::default(), 0));
        }
        let replay_inside_due_evidence = campaign.step(PlayerCommand::default(), 0);
        assert_eq!(replay_inside, inside_evidence);
        assert_eq!(replay_inside_due_evidence, inside_due_evidence);
        let mut replay_travel_evidence = Vec::new();
        for _ in 0..travel_evidence.len() {
            replay_travel_evidence.push(campaign.step(travel_command, 0));
        }
        assert_eq!(replay_travel_evidence, travel_evidence);
        assert_eq!(
            replay_travel_evidence.last(),
            Some(&producer_evidence),
            "replay reaches the same source producer visit"
        );
        assert_eq!(campaign.events(), producer_events);
        campaign
            .simulation_mut()
            .set_level2_entity_position(player_entity_index, pickup_position)
            .expect("replay uses the same production position carrier");
        let mut replay_acquisition_evidence = Vec::new();
        for _ in 0..acquisition_evidence.len() {
            replay_acquisition_evidence.push(campaign.step(PlayerCommand::default(), 0));
        }
        assert_eq!(replay_acquisition_evidence, acquisition_evidence);
        assert_eq!(campaign.events(), first_events);
        assert_eq!(campaign.state_hash(), first_final_hash);
        assert_eq!(
            campaign.simulation().economy().authored_trees(),
            first_trees
        );
        assert_eq!(
            campaign.simulation().shared_spells().dynamic_pickups(),
            first_spell_projection
        );
    }

    #[test]
    fn cf15_level1_real_package_progression_reaches_archers_speed_and_fireflies() {
        const GROUP_ROWS: [u16; 6] = [101, 102, 103, 106, 109, 153];
        const ARCHER_ROWS: [u16; 4] = [101, 102, 103, 106];
        const GROUP13_MODEL5_ROWS: [u16; 9] = [295, 296, 297, 298, 299, 300, 301, 302, 303];

        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let semantic_level = campaign.definition().level_definition();
        assert_eq!(
            semantic_level.authored_creature_count(crate::level_definition::CreatureKind::Archer),
            4
        );
        assert_eq!(
            semantic_level.authored_creature_count(crate::level_definition::CreatureKind::Firefly),
            6
        );
        let reset_hash = campaign.state_hash();
        assert_eq!(
            campaign
                .simulation()
                .mission()
                .authored_features()
                .iter()
                .filter(|row| row.activation_group == 3 && row.kind != 0)
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            GROUP_ROWS
        );

        let step_toward = |campaign: &mut CampaignSession, target: crate::Position| {
            let state = campaign.simulation().state;
            let desired = i16::try_from(crate::heading_for_positions(state.position, target))
                .expect("source heading is 11-bit");
            let correction = crate::turn_toward(state.yaw, desired, 127);
            campaign.step(
                PlayerCommand {
                    roll: correction.clamp(-24, 24) as i8,
                    // Keep the source terrain-relative flight controller
                    // climbing over the ridge between the two real
                    // waypoints instead of repeatedly rejecting the same
                    // horizontal move at ground height.
                    pitch: -127,
                    flags: 1,
                    ..PlayerCommand::default()
                },
                0,
            )
        };
        let stage_position = |campaign: &CampaignSession, stage_index: usize| {
            let stage = &campaign.simulation().runtime().stages[stage_index];
            crate::Position {
                x: stage.axis_x.cast_unsigned(),
                y: stage.axis_y.cast_unsigned(),
                z: campaign.simulation().state.position.z,
            }
        };

        let first_waypoint = stage_position(&campaign, 0);
        let mut saw_group13_model5 = false;
        for _ in 0..512 {
            if campaign.simulation().runtime().objectives[0].stage_selectors[0] == 2 {
                break;
            }
            let _ = step_toward(&mut campaign, first_waypoint);
            let activated_group13 = campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 13,
                        authored_indices,
                        ..
                    } if authored_indices.len() == 33
                        && authored_indices.ends_with(&GROUP13_MODEL5_ROWS)
                )
            });
            if activated_group13 {
                let splashes = campaign
                    .simulation()
                    .economy()
                    .authored_model5_water_splashes();
                assert_eq!(splashes.len(), GROUP13_MODEL5_ROWS.len());
                assert!(splashes.iter().all(|source| {
                    (
                        source.source_class,
                        source.model,
                        source.action,
                        source.max_life,
                        source.life,
                        source.control_flags,
                        source.sprite_index,
                        source.active,
                        source.drawing_enabled,
                    ) == (10, 5, 5, 8, 7, 0x20_002, 244, true, true)
                }));
                saw_group13_model5 = true;
            }
        }
        assert_eq!(
            campaign.simulation().runtime().objectives[0].stage_selectors[0],
            2,
            "public flight must reach the real first waypoint"
        );

        let second_waypoint = stage_position(&campaign, 1);
        let mut activation_evidence = None;
        for _ in 0..512 {
            let evidence = step_toward(&mut campaign, second_waypoint);
            if evidence.is_empty() {
                panic!(
                    "public flight source turn failed at {:?}: {:?}",
                    campaign.simulation().state.position,
                    campaign.events()
                );
            }
            let activated_group13 = campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 13,
                        authored_indices,
                        ..
                    } if authored_indices.len() == 33
                        && authored_indices.ends_with(&GROUP13_MODEL5_ROWS)
                )
            });
            if activated_group13 {
                let splashes = campaign
                    .simulation()
                    .economy()
                    .authored_model5_water_splashes();
                assert_eq!(splashes.len(), GROUP13_MODEL5_ROWS.len());
                assert!(splashes.iter().all(|source| {
                    (
                        source.source_class,
                        source.model,
                        source.action,
                        source.max_life,
                        source.life,
                        source.control_flags,
                        source.sprite_index,
                        source.active,
                        source.drawing_enabled,
                    ) == (10, 5, 5, 8, 7, 0x20_002, 244, true, true)
                }));
                saw_group13_model5 = true;
            }
            if campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 3,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == GROUP_ROWS
                )
            }) {
                activation_evidence = Some(evidence);
                break;
            }
        }
        assert_eq!(
            campaign.simulation().runtime().objectives[0].stage_selectors[1],
            2,
            "public flight ended at {:?} yaw {} speed {}/{} pitch {} without completing the second real waypoint; live model-32 rows {:?}",
            campaign.simulation().state.position,
            campaign.simulation().state.yaw,
            campaign.simulation().state.actual_speed,
            campaign.simulation().state.target_speed,
            campaign.simulation().state.effective_pitch,
            campaign
                .simulation()
                .economy()
                .authored_model32()
                .iter()
                .map(|row| (row.authored_index, row.fired))
                .collect::<Vec<_>>()
        );
        let _activation_evidence = activation_evidence
            .expect("public commanded flight reaches the real objective/model-32 group-3 chain");
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(saw_group13_model5);

        let session = campaign.simulation();
        let archers = session
            .economy()
            .authored_class5()
            .iter()
            .filter(|row| row.model == 4 && ARCHER_ROWS.contains(&row.authored_index))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            archers
                .iter()
                .map(|row| row.authored_index)
                .collect::<Vec<_>>(),
            ARCHER_ROWS
        );
        assert!(archers.iter().all(|row| {
            row.active && row.drawing_enabled && row.life == 1_000 && row.max_life == 1_000
        }));
        assert!(archers.iter().all(|row| {
            session.creature_kind_for_source_slot(row.source_entity_index)
                == Some(crate::level_definition::CreatureKind::Archer)
        }));
        assert_eq!(
            archers.iter().map(|row| row.action).collect::<Vec<_>>(),
            [35, 33, 33, 33],
            "the constructor-phase-zero Archer links its nearest already-materialized Archer peer on its same-pass callback"
        );
        let countdown = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|row| row.authored_index == 109)
            .copied()
            .expect("group-3 row 109 materializes through the shared class-11 arena");
        assert_eq!(
            (
                countdown.model,
                countdown.action,
                countdown.stage_tag,
                countdown.dword_0x10,
                countdown.active,
                countdown.drawing_enabled,
            ),
            (17, 17, 11, 16, true, true)
        );
        let archer_objective_switch = session
            .economy()
            .authored_model32()
            .iter()
            .find(|row| row.authored_index == 153)
            .copied()
            .expect("group-3 row 153 waits for the Archer-absence objective");
        assert_eq!(
            session.runtime().objectives[0].stage_selectors[2],
            1,
            "live package Archers keep the model-4 absence objective open"
        );
        assert_eq!(session.runtime().objectives[0].text_selector, 2);
        assert!(!archer_objective_switch.fired);
        assert!(
            session
                .economy()
                .authored_model32()
                .iter()
                .all(|row| row.authored_index != 190)
        );

        let receipts = session.cf10_production_schedule_receipts();
        for source_slot in archers
            .iter()
            .map(|row| row.source_entity_index)
            .chain(std::iter::once(countdown.source_entity_index))
        {
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == source_slot
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == source_slot
            )));
        }

        let _ = campaign.step(PlayerCommand::default(), 0);
        let frozen = campaign
            .simulation()
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|row| row.authored_index == 109)
            .expect("live Archer membership retains the countdown producer");
        assert_eq!(frozen.dword_0x10, 16);
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 11, .. }
        )));

        // Archer damage and death are not under test here. The real-package
        // route above already proved these exact four roots and the frozen
        // model-4 membership. Stage only their source hide state, then leave
        // cleanup, membership rebuild, countdown, construction, dispatch,
        // epilogue, and reclaim to the ordinary production scheduler.
        assert_eq!(
            campaign
                .simulation_mut()
                .economy_mut()
                .hide_authored_class5_rows_for_test(&ARCHER_ROWS),
            ARCHER_ROWS.len()
        );
        let countdown_slot = countdown.source_entity_index;
        let archer_objective_switch_slot = archer_objective_switch.source_entity_index;
        let mut castle_objective_switch_slot = None;
        let mut archer_objective_switch_phase_before_fire = None;
        let mut firefly_constructor_route = None;

        for expected_timer in (1..=15).rev() {
            let evidence = campaign.step(PlayerCommand::default(), 0);
            assert!(!evidence.is_empty());
            let session = campaign.simulation();
            let countdown = session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|row| row.authored_index == 109)
                .expect("Archer-absence countdown remains live before its fire visit");
            assert_eq!(countdown.dword_0x10, expected_timer);
            assert!(
                session
                    .shared_spells()
                    .authored_pickup_slots()
                    .get(&100)
                    .is_none()
            );
            assert!(campaign.events().iter().all(|event| !matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 11, .. }
            )));
            match expected_timer {
                15 => {
                    assert_eq!(session.runtime().objectives[0].stage_selectors[2], 2);
                    assert_eq!(session.runtime().objectives[0].text_selector, 3);
                    let row153 = session
                        .economy()
                        .authored_model32()
                        .iter()
                        .find(|row| row.authored_index == 153)
                        .expect("objective completion happens after row 153's callback");
                    assert!(row153.active && row153.drawing_enabled && !row153.fired);
                    archer_objective_switch_phase_before_fire = Some(row153.source_byte_3e);
                    assert!(campaign.events().iter().all(|event| !matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated { group: 5, .. }
                    )));
                    assert!(
                        session
                            .economy()
                            .authored_model32()
                            .iter()
                            .all(|row| row.authored_index != 190)
                    );
                }
                14 => {
                    assert!(campaign.events().iter().any(|event| matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated {
                            source_slot,
                            group: 5,
                            authored_indices,
                            ..
                        } if *source_slot == archer_objective_switch_slot
                            && authored_indices.as_ref() == [190]
                    )));
                    let row153 = session
                        .economy()
                        .authored_model32()
                        .iter()
                        .find(|row| row.authored_index == 153)
                        .expect("fired row 153 is retained through its callback pass");
                    assert_eq!(
                        (
                            row153.fired,
                            row153.drawing_enabled,
                            row153.active,
                            row153.control_flags,
                        ),
                        (true, false, false, 0x401)
                    );
                    assert_eq!(
                        row153.source_byte_3e,
                        archer_objective_switch_phase_before_fire
                            .expect("the preceding pass observed row 153")
                            .wrapping_add(1)
                    );
                    let row190 = session
                        .economy()
                        .authored_model32()
                        .iter()
                        .find(|row| row.authored_index == 190)
                        .copied()
                        .expect("group 5 materializes the package castle-objective switch");
                    assert_eq!(
                        (
                            row190.source_class,
                            row190.model,
                            row190.action,
                            row190.stage_tag,
                            row190.objective_stage_index,
                            row190.max_life,
                            row190.life,
                            row190.control_flags,
                            row190.active,
                            row190.drawing_enabled,
                            row190.fired,
                        ),
                        (11, 32, 32, 6, 3, 300, 300, 1, true, true, false)
                    );
                    assert!(row190.source_entity_index < archer_objective_switch_slot);
                    assert_eq!(row190.source_byte_3e, row190.source_entity_index as u8);
                    assert!(session.cf10_production_schedule_receipts().iter().any(
                        |receipt| matches!(
                            receipt,
                            ScheduleReceipt::EpilogueCompleted {
                                pass: SourcePass::UpdateEntities,
                                slot,
                                ..
                            } if *slot == archer_objective_switch_slot
                        )
                    ));
                    assert!(session.cf10_production_schedule_receipts().iter().all(
                        |receipt| !matches!(
                            receipt,
                            ScheduleReceipt::CallbackCompleted { slot, .. }
                                if *slot == row190.source_entity_index
                        )
                    ));
                    assert!(campaign.events().iter().all(|event| !matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated { group: 6, .. }
                    )));
                    castle_objective_switch_slot = Some(row190.source_entity_index);
                }
                13 => {
                    assert!(
                        session
                            .economy()
                            .authored_model32()
                            .iter()
                            .all(|row| row.authored_index != 153)
                    );
                    let row190_slot = castle_objective_switch_slot
                        .expect("the preceding pass materialized row 190");
                    assert!(session.cf10_production_schedule_receipts().iter().any(
                        |receipt| matches!(
                            receipt,
                            ScheduleReceipt::CallbackCompleted {
                                pass: SourcePass::UpdateEntities,
                                slot,
                                ..
                            } if *slot == row190_slot
                        )
                    ));
                    let row190 = session
                        .economy()
                        .authored_model32()
                        .iter()
                        .find(|row| row.source_entity_index == row190_slot)
                        .expect("row 190 remains inert while objective stage 3 is open");
                    assert!(row190.active && row190.drawing_enabled && !row190.fired);
                    assert_eq!(row190.source_byte_3e, (row190_slot as u8).wrapping_add(1));
                    assert!(campaign.events().iter().all(|event| !matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated { group: 6, .. }
                    )));
                    firefly_constructor_route = Some(campaign.clone());
                }
                _ => {}
            }
        }

        // This branch starts from the package-backed row-190 state proven
        // above. Castle construction and mana collection are deliberately not
        // under test: stage only those two upstream authority fields, then
        // leave objective timing, row-190 dispatch, group construction, live
        // numeric callbacks, and phase epilogues to the ordinary scheduler.
        let mut firefly_route = firefly_constructor_route
            .expect("the real row-190 route was captured while stage 3 remained open");
        let mut missing_castle = firefly_route.clone();
        let _ = missing_castle.step(PlayerCommand::default(), 0);
        assert_eq!(
            missing_castle.simulation().runtime().objectives[0].stage_selectors[3],
            1,
            "secured mana cannot satisfy the package objective without a Castle relation"
        );
        assert!(missing_castle.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 6, .. }
        )));

        let player_entity_index = firefly_route.simulation().state.entity_index;
        let player_position = firefly_route.simulation().state.position;
        let castle_slot = firefly_route
            .simulation_mut()
            .economy_mut()
            .build_castle(player_position, player_entity_index)
            .expect("the package arena can stage the Castle objective prerequisite");
        assert!(
            firefly_route
                .simulation_mut()
                .economy_mut()
                .set_castle_mana_prerequisite_for_test(castle_slot, 4_000)
        );

        let _ = firefly_route.step(PlayerCommand::default(), 0);
        assert_eq!(
            firefly_route.simulation().runtime().objectives[0].stage_selectors[3],
            2,
            "kind-0 Castle/mana completion is a post-entity objective phase"
        );
        assert!(firefly_route.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 6, .. }
        )));
        assert!(
            firefly_route
                .simulation()
                .economy()
                .authored_fireflies()
                .is_empty()
        );

        let row190_slot = firefly_route
            .simulation()
            .economy()
            .authored_model32()
            .iter()
            .find(|row| row.authored_index == 190)
            .expect("the package group-6 producer remains live")
            .source_entity_index;
        let firefly_birth_evidence = firefly_route.step(PlayerCommand::default(), 0);
        let firefly_session = firefly_route.simulation();
        assert!(
            firefly_route.events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group: 6,
                    authored_indices,
                    ..
                } if *source_slot == row190_slot
                    && authored_indices.as_ref() == [108, 134, 135, 154, 189, 193]
            )),
            "row 190 did not activate package group 6: evidence={firefly_birth_evidence:?}, events={:?}, row190={:?}, objective={:?}",
            firefly_route.events(),
            firefly_session
                .economy()
                .authored_model32()
                .iter()
                .find(|row| row.source_entity_index == row190_slot),
            firefly_session.runtime().objectives[0],
        );
        assert_eq!(
            firefly_session.runtime().objectives[0].stage_selectors[4],
            1
        );
        let firefly_roots = firefly_session
            .economy()
            .authored_class5()
            .iter()
            .filter(|row| [108, 134, 135, 189, 193].contains(&row.authored_index))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            firefly_roots
                .iter()
                .map(|row| row.authored_index)
                .collect::<Vec<_>>(),
            [108, 134, 135, 189, 193]
        );
        assert_eq!(firefly_session.economy().authored_fireflies().len(), 5);
        assert!(firefly_roots.iter().all(|root| {
            firefly_session.creature_kind_for_source_slot(root.source_entity_index)
                == Some(crate::level_definition::CreatureKind::Firefly)
        }));
        let receipts = firefly_session.cf10_production_schedule_receipts();
        for (ordinal, root) in firefly_roots.iter().enumerate() {
            let callback_on_birth_pass = root.source_entity_index > row190_slot;
            assert_eq!(
                (
                    root.model,
                    root.action,
                    root.minimum_speed,
                    root.maximum_speed,
                    root.actual_speed,
                    root.max_life,
                    root.life,
                    root.mana,
                    root.max_mana,
                    root.subspell_index,
                    root.dword_0x10,
                ),
                (
                    19,
                    159,
                    76,
                    8,
                    76,
                    600,
                    600,
                    300,
                    0,
                    300,
                    i32::from(root.source_entity_index % 100),
                )
            );
            assert_eq!(
                (
                    root.particle_index,
                    root.shift_rot_x,
                    root.shift_rot_y,
                    root.source_byte_3e,
                    root.active,
                    root.drawing_enabled,
                ),
                (
                    287,
                    85,
                    51,
                    u8::try_from(ordinal)
                        .expect("five Fireflies fit u8")
                        .wrapping_add(u8::from(callback_on_birth_pass)),
                    true,
                    true,
                )
            );
            let stage = firefly_session
                .economy()
                .authored_fireflies()
                .iter()
                .find(|stage| stage.source_entity_index == root.source_entity_index)
                .expect("each Firefly root has one stage sidecar");
            assert_eq!(
                (
                    stage.control_flags,
                    stage.byte_0x38,
                    stage.xtype,
                    stage.descriptor_index,
                    stage.stage_var1,
                    stage.stage_var2,
                    stage.stage_target.x,
                    stage.stage_target.y,
                ),
                (8, 1, 3, 88, 8, 1, 242 << 8, 184 << 8)
            );
            assert_eq!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == root.source_entity_index
                )),
                callback_on_birth_pass
            );
            assert_eq!(
                receipts.iter().any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::EpilogueCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == root.source_entity_index
                )),
                callback_on_birth_pass
            );
        }
        let firefly_slots = firefly_roots
            .iter()
            .map(|row| row.source_entity_index)
            .collect::<Vec<_>>();
        let third = firefly_slots[2];
        let fourth = firefly_slots[3];
        assert!(
            firefly_session.entities().iter().any(|row| row.active
                && row.class == 2
                && row.model == 1
                && row.action == 3
                && row.index > third
                && row.index < fourth),
            "package Stone row 154 occupies the fourth group-6 NewEvent allocation"
        );

        let phases_before_following_visit = firefly_roots
            .iter()
            .map(|row| (row.source_entity_index, row.source_byte_3e))
            .collect::<Vec<_>>();
        let _ = firefly_route.step(PlayerCommand::default(), 0);
        let firefly_session = firefly_route.simulation();
        assert_eq!(
            firefly_session.runtime().objectives[0].stage_selectors[4],
            1
        );
        for (slot, phase) in phases_before_following_visit {
            let root = firefly_session
                .economy()
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .expect("stage-patrol Firefly remains live on its following visit");
            assert_eq!(root.source_byte_3e, phase.wrapping_add(1));
            assert!(
                firefly_session
                    .cf10_production_schedule_receipts()
                    .iter()
                    .any(|receipt| matches!(
                        receipt,
                        ScheduleReceipt::CallbackCompleted {
                            pass: SourcePass::UpdateEntities,
                            slot: receipt_slot,
                            ..
                        } if *receipt_slot == slot
                    ))
            );
        }

        // Stage only the already-proven package roots at the model-19 stage
        // target. Natural travel time is not under test; the ordinary
        // PreDispatch arrival, action-153 decision, action-155 visits, and
        // shared numeric epilogues remain authoritative production work.
        let mut charge_route = firefly_route.clone();
        let roam_slot = firefly_slots[0];
        let peer_slot = firefly_slots[1];
        assert!(roam_slot < peer_slot);
        assert!(
            firefly_route
                .simulation_mut()
                .economy_mut()
                .stage_authored_firefly_peer_roam_prerequisite_for_test(roam_slot, peer_slot,)
        );
        let staged_roam = *firefly_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == roam_slot)
            .expect("the staged real Firefly root remains live");
        let motion_config = crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: staged_roam.shift_rot_x,
            half_height: staged_roam.shift_rot_y,
            ground_clearance: 51,
            descent: -8,
            turn_limit: 113,
            terrain_mask: u32::MAX,
            maximum_tile_slope: 20,
        };
        let expected_roam_motion = crate::ground_creature_motion::step_ground_creature_motion(
            staged_roam.position,
            staged_roam.yaw,
            staged_roam.roll,
            staged_roam.actual_speed,
            &firefly_route.simulation().session.state.live.terrain.clone(),
            motion_config,
        );
        let next_rng16 = |seed: u16| seed.wrapping_mul(9_377).wrapping_add(9_439);
        let wander_direction = next_rng16(staged_roam.entity_rng);
        let roam_rng_after_decision = next_rng16(wander_direction);
        let wander_sign = if wander_direction % 157 < 79 { -1 } else { 1 };
        let wander_delta = i16::try_from((roam_rng_after_decision & 0x00ff) + 85)
            .expect("Firefly wander delta fits i16");
        let expected_roam_roll = staged_roam.roll.wrapping_add(wander_sign * wander_delta) & 0x07ff;

        let _ = firefly_route.step(PlayerCommand::default(), 0);
        let firefly_session = firefly_route.simulation();
        let roam = firefly_session
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == roam_slot)
            .expect("the real Firefly survives its roam decision");
        let roam_state = firefly_session
            .economy()
            .authored_fireflies()
            .iter()
            .find(|state| state.source_entity_index == roam_slot)
            .expect("the real Firefly keeps its model-19 sidecar");
        assert_eq!(
            (
                roam.action,
                roam.source_byte_3e,
                roam.entity_rng,
                roam.actual_speed,
                roam.position,
                roam.yaw,
                roam.roll,
                roam_state.stage_var1,
                roam_state.stage_var2,
                roam_state.linked_entity_index,
            ),
            (
                155,
                71,
                roam_rng_after_decision,
                76,
                expected_roam_motion.position,
                expected_roam_motion.yaw,
                expected_roam_roll,
                0,
                0,
                peer_slot,
            ),
            "arrival changes 159->153 before dispatch; action 153 alone consumes two RNG draws and records the real peer"
        );
        assert!(
            firefly_session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == roam_slot
                ))
        );
        assert!(
            firefly_session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::EpilogueCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == roam_slot
                ))
        );
        assert!(
            firefly_route
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        // Action 155 moves on intervening visits but consumes the link only
        // on its own 35-tick decision cadence. This rejects recursive
        // same-visit redispatch after action 153 selects the peer.
        let linked_before = *roam;
        let expected_linked_motion = crate::ground_creature_motion::step_ground_creature_motion(
            linked_before.position,
            linked_before.yaw,
            linked_before.roll,
            linked_before.actual_speed,
            &firefly_route.simulation().session.state.live.terrain.clone(),
            motion_config,
        );
        let mut invalid_link = firefly_route.clone();
        let peer_authored_index = invalid_link
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == peer_slot)
            .expect("the selected peer is one of the real package roots")
            .authored_index;
        assert_eq!(
            invalid_link
                .simulation_mut()
                .economy_mut()
                .hide_authored_class5_rows_for_test(&[peer_authored_index]),
            1
        );
        let _ = invalid_link.step(PlayerCommand::default(), 0);
        let invalid_root = invalid_link
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == roam_slot)
            .expect("an invalid peer does not remove the following Firefly");
        let invalid_state = invalid_link
            .simulation()
            .economy()
            .authored_fireflies()
            .iter()
            .find(|state| state.source_entity_index == roam_slot)
            .expect("the following Firefly retains its sidecar");
        assert_eq!(
            (
                invalid_root.action,
                invalid_root.source_byte_3e,
                invalid_root.position,
                invalid_root.yaw,
                invalid_state.linked_entity_index,
            ),
            (
                153,
                72,
                expected_linked_motion.position,
                expected_linked_motion.yaw,
                0,
            ),
            "an invalid word-0x32 peer is cleared after movement even off cadence"
        );

        let _ = firefly_route.step(PlayerCommand::default(), 0);
        let off_cadence = firefly_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == roam_slot)
            .expect("linked Firefly remains live before its decision cadence");
        assert_eq!(
            (
                off_cadence.action,
                off_cadence.source_byte_3e,
                off_cadence.entity_rng,
                off_cadence.actual_speed,
                off_cadence.position,
                off_cadence.yaw,
            ),
            (
                155,
                72,
                roam_rng_after_decision,
                76,
                expected_linked_motion.position,
                expected_linked_motion.yaw,
            )
        );
        let decision_root_before = *off_cadence;
        let peer_position_before = firefly_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == peer_slot)
            .expect("the linked package peer remains live before the decision visit")
            .position;
        let expected_decision_motion = crate::ground_creature_motion::step_ground_creature_motion(
            decision_root_before.position,
            decision_root_before.yaw,
            decision_root_before.roll,
            decision_root_before.actual_speed,
            &firefly_route.simulation().session.state.live.terrain.clone(),
            motion_config,
        );
        let expected_linked_roll = i16::try_from(crate::heading_for_positions(
            expected_decision_motion.position,
            peer_position_before,
        ))
        .expect("Firefly linked heading fits i16");
        assert!(
            firefly_route
                .simulation_mut()
                .economy_mut()
                .stage_authored_firefly_phase_prerequisite_for_test(roam_slot, 105)
        );
        let _ = firefly_route.step(PlayerCommand::default(), 0);
        let firefly_session = firefly_route.simulation();
        let roam = firefly_session
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == roam_slot)
            .expect("linked Firefly remains live after consuming its peer relation");
        let roam_state = firefly_session
            .economy()
            .authored_fireflies()
            .iter()
            .find(|state| state.source_entity_index == roam_slot)
            .expect("linked Firefly retains its peer relation");
        assert_eq!(
            (
                roam.action,
                roam.source_byte_3e,
                roam.entity_rng,
                roam.actual_speed,
                roam.position,
                roam.yaw,
                roam.roll,
                roam_state.linked_entity_index,
            ),
            (
                155,
                106,
                roam_rng_after_decision,
                84,
                expected_decision_motion.position,
                expected_decision_motion.yaw,
                expected_linked_roll,
                peer_slot,
            ),
            "action 155 follows the real action-153 peer only on its later source cadence"
        );
        assert!(
            firefly_session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == roam_slot
                ))
        );
        assert!(
            firefly_route
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        // Stop the real player through its public command, then stage only a
        // real package Firefly's geometry/cadence/private RNG so the ordinary
        // action-153 scan reaches action 154's charge branch promptly. This
        // proves the production callbacks and visible sound, not the branch's
        // natural random waiting time or its separate fireball lifecycle.
        let _ = charge_route.step(
            PlayerCommand {
                opcode: 0x27,
                argument_1: 1,
                ..PlayerCommand::default()
            },
            0,
        );
        let attack_player = charge_route.simulation().state;
        assert_eq!(
            (attack_player.actual_speed, attack_player.target_speed),
            (0, 0)
        );
        let attack_slot = firefly_slots[0];
        assert!(
            charge_route
                .simulation_mut()
                .economy_mut()
                .stage_authored_firefly_charge_prerequisite_for_test(
                    attack_slot,
                    attack_player.position,
                    attack_player.yaw,
                )
        );
        let charge_cursor = charge_route.effect_cursor();
        let expected_motion = |route: &CampaignSession, root: &Level2AuthoredClass5Source| {
            crate::ground_creature_motion::step_ground_creature_motion(
                root.position,
                root.yaw,
                root.roll,
                root.actual_speed,
                &route.simulation().session.state.live.terrain.clone(),
                motion_config,
            )
        };

        let staged_attack = *charge_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the staged attacker is a real group-6 Firefly");
        let acquisition_motion = expected_motion(&charge_route, &staged_attack);
        let _ = charge_route.step(PlayerCommand::default(), 0);
        let acquired = *charge_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the Firefly survives its real player acquisition");
        assert_eq!(
            (
                acquired.action,
                acquired.word_0x96_150,
                acquired.byte_0x46_70,
                acquired.source_byte_3e,
                acquired.entity_rng,
                acquired.position,
                acquired.yaw,
            ),
            (
                154,
                attack_player.entity_index,
                0,
                71,
                54_209,
                acquisition_motion.position,
                acquisition_motion.yaw,
            ),
            "action 153 moves, consumes exactly two private RNG draws, and selects the real player without recursively dispatching action 154"
        );

        let orbit_motion = expected_motion(&charge_route, &acquired);
        let _ = charge_route.step(PlayerCommand::default(), 0);
        let orbit = *charge_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the Firefly survives its orbit visit");
        assert_eq!(
            (
                orbit.action,
                orbit.byte_0x46_70,
                orbit.source_byte_3e,
                orbit.entity_rng,
                orbit.actual_speed,
                orbit.position,
                orbit.yaw,
            ),
            (
                154,
                2,
                72,
                30_016,
                76,
                orbit_motion.position,
                orbit_motion.yaw,
            ),
            "substates 0 and 1 share one callback and consume the orbit draw"
        );

        let maneuver_motion = expected_motion(&charge_route, &orbit);
        let _ = charge_route.step(PlayerCommand::default(), 0);
        let maneuver_target = charge_route.simulation().state;
        let maneuver = *charge_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the Firefly survives its charge selection");
        let expected_altitude_target = i32::from(maneuver_target.position.z) + 543;
        let mut expected_maneuver_position = maneuver_motion.position;
        expected_maneuver_position.z = expected_maneuver_position.z.wrapping_add(
            if i32::from(expected_maneuver_position.z) <= expected_altitude_target {
                64
            } else {
                -64
            },
        );
        assert_eq!(
            (
                maneuver.action,
                maneuver.byte_0x46_70,
                maneuver.source_byte_3e,
                maneuver.entity_rng,
                maneuver.actual_speed,
                maneuver.dword_0x10,
                maneuver.position,
                maneuver.yaw,
            ),
            (
                154,
                7,
                73,
                46_686,
                8,
                expected_altitude_target,
                expected_maneuver_position,
                maneuver_motion.yaw,
            ),
            "state 2 falls through state 3, preserving the source RNG and rare charge selector order"
        );
        let pre_charge_effects = charge_route
            .effects_after(charge_cursor, 16_384)
            .expect("bounded pre-charge journal page");
        assert!(pre_charge_effects.records.iter().all(|record| !matches!(
            record.effect,
            crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                sound_id: 43 | 44,
                ..
            })
        )));

        let player_record_before_charge = charge_route.simulation().entities()
            [usize::from(attack_player.entity_index)]
        .record_bytes()[98..104]
            .to_vec();
        let charge_motion = expected_motion(&charge_route, &maneuver);
        let _ = charge_route.step(PlayerCommand::default(), 0);
        let charge_target = charge_route.simulation().state;
        let charged = *charge_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the charging Firefly remains live");
        let mut expected_charge_position = charge_motion.position;
        let charge_altitude = i32::from(charge_target.position.z)
            .wrapping_sub(i32::from(expected_charge_position.z))
            .clamp(-64, 64);
        expected_charge_position.z = expected_charge_position
            .z
            .wrapping_add(charge_altitude as i16);
        assert_eq!(
            (
                charged.action,
                charged.byte_0x46_70,
                charged.source_byte_3e,
                charged.entity_rng,
                charged.actual_speed,
                charged.dword_0x10,
                charged.position,
                charged.yaw,
            ),
            (
                154,
                8,
                74,
                3_581,
                228,
                23,
                expected_charge_position,
                charge_motion.yaw,
            ),
            "state 7 emits sound and falls through state 8, so its first completed timer is 23"
        );
        let charge_effects = charge_route
            .effects_after(charge_cursor, 16_384)
            .expect("bounded charge journal page");
        assert_eq!(
            charge_effects
                .records
                .iter()
                .filter(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        FlightEvent::PrepareSound {
                            entity_index,
                            player_index: -1,
                            sound_id: 44,
                            ..
                        }
                    ) if entity_index == attack_slot
                ))
                .count(),
            1
        );
        assert_eq!(
            charge_effects
                .records
                .iter()
                .filter(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        FlightEvent::PrepareSound {
                            entity_index,
                            player_index: -1,
                            sound_id: 43 | 44,
                            ..
                        }
                    ) if entity_index == attack_slot
                ))
                .count(),
            1,
            "the charge visit emits exactly one RNG-selected Firefly sound"
        );
        assert_eq!(
            charge_route.simulation().entities()[usize::from(attack_player.entity_index)]
                .record_bytes()[98..104],
            player_record_before_charge,
            "the staged geometry remains outside the strict contact radius, so this slice does not counterfeit damage"
        );
        assert!(
            charge_route
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        // Continue the same real package root from its proven charge state.
        // Only proximity is staged; action, target, timer, RNG, movement,
        // mailbox publication, player consumption, and source order remain
        // production-owned.
        let mut contact_route = charge_route.clone();
        let contact_player = contact_route.simulation().state;
        let player_life_before_contact = contact_route.simulation().session.state.live.player_life;
        assert!(
            contact_route
                .simulation_mut()
                .economy_mut()
                .stage_authored_firefly_contact_geometry_for_test(
                    attack_slot,
                    contact_player.position,
                )
        );
        let staged_contact = *contact_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the real charging Firefly is staged at contact geometry");
        let contact_motion = expected_motion(&contact_route, &staged_contact);
        let mut expected_contact_position = contact_motion.position;
        let contact_altitude = i32::from(contact_player.position.z)
            .wrapping_sub(i32::from(expected_contact_position.z))
            .clamp(-64, 64);
        expected_contact_position.z = expected_contact_position
            .z
            .wrapping_add(contact_altitude as i16);
        assert!(
            crate::legacy_sqrt({
                let dx = i32::from(
                    contact_player
                        .position
                        .x
                        .wrapping_sub(expected_contact_position.x)
                        .cast_signed(),
                );
                let dy = i32::from(
                    contact_player
                        .position
                        .y
                        .wrapping_sub(expected_contact_position.y)
                        .cast_signed(),
                );
                let dz = i32::from(contact_player.position.z)
                    .wrapping_sub(i32::from(expected_contact_position.z));
                dx.wrapping_mul(dx)
                    .cast_unsigned()
                    .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
                    .wrapping_add(dz.wrapping_mul(dz).cast_unsigned())
            }) < 768,
            "post-movement and post-altitude geometry must cross the strict source contact radius"
        );
        let contact_cursor = contact_route.effect_cursor();
        let _ = contact_route.step(PlayerCommand::default(), 0);
        let contacted_player_state = contact_route.simulation().state;
        let contact_altitude = i32::from(contacted_player_state.position.z)
            .wrapping_sub(i32::from(contact_motion.position.z))
            .clamp(-64, 64);
        expected_contact_position.z = contact_motion
            .position
            .z
            .wrapping_add(contact_altitude as i16);
        let contacted = *contact_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the Firefly survives its charge contact");
        assert_eq!(
            (
                contacted.action,
                contacted.byte_0x46_70,
                contacted.source_byte_3e,
                contacted.entity_rng,
                contacted.actual_speed,
                contacted.dword_0x10,
                contacted.position,
            ),
            (154, 9, 75, 3_581, 228, 22, expected_contact_position,),
            "state 8 moves, decrements, clamps altitude, publishes one contact, then takes one common epilogue"
        );
        assert!(
            contact_route
                .effects_after(contact_cursor, 16_384)
                .expect("bounded contact journal page")
                .records
                .iter()
                .all(|record| !matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        sound_id: 43 | 44,
                        ..
                    })
                )),
            "contact consumes no private RNG and emits no second charge sound"
        );
        if attack_slot > contact_player.entity_index {
            let player_row =
                &contact_route.simulation().entities()[usize::from(contact_player.entity_index)];
            assert_eq!(
                (
                    contact_route.simulation().session.state.live.player_life,
                    i32::from_le_bytes(
                        player_row.record_bytes()[98..102]
                            .try_into()
                            .expect("player damage payload bytes"),
                    ),
                    u16::from_le_bytes(
                        player_row.record_bytes()[102..104]
                            .try_into()
                            .expect("player attacker bytes"),
                    ),
                ),
                (player_life_before_contact, 300, attack_slot),
                "a later Firefly slot leaves contact queued for the next player visit"
            );
            let _ = contact_route.step(PlayerCommand::default(), 0);
        }
        let contact_session = contact_route.simulation();
        let damage_events = contact_route
            .events()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::Economy {
                        event: Level2EconomyEvent::DamageApplied {
                            target_entity_index,
                            attacker_entity_index,
                            damage: 300,
                            lethal: false,
                        },
                        ..
                    } if *target_entity_index == contact_player.entity_index
                        && *attacker_entity_index == attack_slot
                )
            })
            .count();
        assert_eq!(damage_events, 1);
        assert_eq!(
            contact_session.session.state.live.player_life,
            player_life_before_contact - 300
        );
        let contacted_player =
            &contact_session.entities()[usize::from(contact_player.entity_index)];
        let contacted_player_record = contact_session
            .shared_spells()
            .players()
            .iter()
            .find(|player| player.entity_index == contact_player.entity_index)
            .expect("the shared local-player record survives Firefly contact");
        assert_eq!(contacted_player_record.hit_frame_time, 4);
        assert_eq!(
            u16::from_le_bytes(
                contacted_player.record_bytes()[102..104]
                    .try_into()
                    .expect("cleared player attacker bytes"),
            ),
            0,
            "the ordinary nonlethal player visit clears the attacker mailbox"
        );

        // The same real package Firefly also owns action 154's state-5
        // fireball producer. Stage only its package-reachable substate,
        // cadence, and geometry; allocation, target feedback, numeric source
        // scheduling, guidance, and impact handling stay on the production
        // arena path rather than a Firefly-only test path.
        let mut fireball_route = charge_route.clone();
        let fireball_player = fireball_route.simulation().state;
        assert!(
            fireball_route
                .simulation_mut()
                .economy_mut()
                .stage_authored_firefly_fireball_prerequisite_for_test(
                    attack_slot,
                    fireball_player.position,
                )
        );
        let staged_firing = *fireball_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the real Firefly is staged for its state-5 firing cadence");
        let firing_motion = expected_motion(&fireball_route, &staged_firing);

        let mut fireball_failure = fireball_route.clone();
        let target_word36_before = fireball_failure.simulation().entities()
            [usize::from(fireball_player.entity_index)]
        .record_bytes()[58..60]
            .to_vec();
        let _ = fireball_failure
            .simulation_mut()
            .economy_mut()
            .exhaust_new_event_capacity_for_test()
            .expect("the shared arena has capacity to exhaust for the Firefly negative");
        let _ = fireball_failure.step(PlayerCommand::default(), 0);
        assert!(
            fireball_failure
                .simulation()
                .economy()
                .fireball_projectiles()
                .iter()
                .all(|projectile| projectile.origin
                    != crate::native_world::FireballProjectileOrigin::Firefly)
        );
        assert_eq!(
            fireball_failure.simulation().entities()[usize::from(fireball_player.entity_index)]
                .record_bytes()[58..60]
                .to_vec(),
            target_word36_before,
            "failed NewEvent allocation cannot publish Firefly target feedback"
        );
        let failed_firing = fireball_failure
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("allocation failure retains the firing Firefly");
        assert_eq!(
            (failed_firing.action, failed_firing.byte_0x46_70),
            (154, 5),
            "state 5 remains eligible to retry after allocation failure"
        );

        let _ = fireball_route.step(PlayerCommand::default(), 0);
        let fired_player = fireball_route.simulation().state;
        let fired_root = *fireball_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == attack_slot)
            .expect("the firing Firefly remains live");
        assert_eq!(
            (
                fired_root.action,
                fired_root.byte_0x46_70,
                fired_root.source_byte_3e,
                fired_root.position,
            ),
            (154, 5, 141, firing_motion.position),
            "state 5 moves, allocates, and takes one common source epilogue"
        );
        let projectile = *fireball_route
            .simulation()
            .economy()
            .fireball_projectiles()
            .iter()
            .find(|projectile| {
                projectile.origin == crate::native_world::FireballProjectileOrigin::Firefly
                    && projectile.owner_entity_index == attack_slot
            })
            .expect("the real Firefly allocates one shared class-9 projectile");
        assert_eq!(
            (
                projectile.source_class,
                projectile.source_model,
                projectile.action,
                projectile.holder_entity_index,
                projectile.source_subspell_index,
                projectile.payload,
                projectile.target_source_entity_index,
                projectile.actual_speed,
                projectile.life,
            ),
            (
                9,
                0,
                0,
                0,
                500,
                500,
                fireball_player.entity_index,
                384,
                if projectile.source_entity_index > attack_slot {
                    20
                } else {
                    21
                },
            )
        );
        assert_eq!(projectile.yaw, staged_firing.yaw);
        assert_eq!(
            i16::from_le_bytes(
                fireball_route.simulation().entities()[usize::from(fireball_player.entity_index)]
                    .record_bytes()[58..60]
                    .try_into()
                    .expect("Firefly target feedback bytes"),
            ),
            100,
            "sub_5EF70 target feedback follows successful allocation"
        );
        let projectile_callback_on_birth = projectile.source_entity_index > attack_slot;
        let fireball_receipts = fireball_route
            .simulation()
            .cf10_production_schedule_receipts();
        assert_eq!(
            fireball_receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == projectile.source_entity_index
            )),
            projectile_callback_on_birth
        );
        assert_eq!(
            fireball_receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == projectile.source_entity_index
            )),
            projectile_callback_on_birth
        );
        assert!(
            crate::legacy_sqrt({
                let dx = i32::from(
                    fired_player
                        .position
                        .x
                        .wrapping_sub(projectile.position.x)
                        .cast_signed(),
                );
                let dy = i32::from(
                    fired_player
                        .position
                        .y
                        .wrapping_sub(projectile.position.y)
                        .cast_signed(),
                );
                dx.wrapping_mul(dx)
                    .cast_unsigned()
                    .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
            }) > 768,
            "the focused birth remains outside impact range and proves the distinct flyer row"
        );

        // Continue one real package model-19 root through the shared damage
        // mailbox and its ordinary 156/157 death tail. Two source-sized
        // impact payloads are queued before the root's numeric visit, proving
        // the occupied-mailbox additive rule without inventing a test-only
        // creature or action.
        let mut death_route = fireball_route.clone();
        let dying_slot = firefly_slots[1];
        let dying_before = *death_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == dying_slot)
            .expect("the real package Firefly is available for damage");
        death_route
            .simulation_mut()
            .apply_fireball_impact_to_firefly_for_test(
                dying_before.position,
                projectile.owner_entity_index,
                500,
            );
        death_route
            .simulation_mut()
            .apply_fireball_impact_to_firefly_for_test(
                dying_before.position,
                projectile.owner_entity_index,
                500,
            );
        let _ = death_route.step(PlayerCommand::default(), 0);
        let pre_kill = *death_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == dying_slot)
            .expect("lethal Firefly remains allocated for pre-kill");
        assert_eq!(
            (
                pre_kill.action,
                pre_kill.life,
                pre_kill.word_0x26,
                pre_kill.word_0x24,
                pre_kill.pending_word_0x62,
                pre_kill.source_byte_3e,
            ),
            (
                156,
                dying_before.life - 1_000,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                dying_before.source_byte_3e.wrapping_add(1),
            )
        );
        assert!(death_route.events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::Economy {
                event: Level2EconomyEvent::DamageApplied {
                    target_entity_index,
                    attacker_entity_index,
                    damage: 1_000,
                    lethal: true,
                },
                ..
            } if *target_entity_index == dying_slot
                && *attacker_entity_index == projectile.owner_entity_index
        )));

        let _ = death_route.step(PlayerCommand::default(), 0);
        let mut kill_phase = death_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == dying_slot)
            .expect("pre-kill transitions to the retained kill row")
            .source_byte_3e;
        assert_eq!(
            death_route
                .simulation()
                .economy()
                .authored_class5()
                .iter()
                .find(|root| root.source_entity_index == dying_slot)
                .expect("retained kill row")
                .action,
            157
        );
        while kill_phase & 7 != 0 {
            let _ = death_route.step(PlayerCommand::default(), 0);
            kill_phase = death_route
                .simulation()
                .economy()
                .authored_class5()
                .iter()
                .find(|root| root.source_entity_index == dying_slot)
                .expect("off-cadence kill row remains retained")
                .source_byte_3e;
        }
        let death_rng_before = death_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == dying_slot)
            .expect("phase-ready kill row")
            .entity_rng;
        let _ = death_route.step(PlayerCommand::default(), 0);
        let killed = *death_route
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|root| root.source_entity_index == dying_slot)
            .expect("kill callback retains the hidden row through its epilogue");
        assert!(!killed.active);
        assert!(!killed.drawing_enabled);
        assert_eq!(killed.action, 157);
        assert_eq!(killed.source_byte_3e, kill_phase.wrapping_add(1));
        assert_eq!(
            killed.entity_rng,
            death_rng_before.wrapping_mul(9_377).wrapping_add(9_439)
        );
        assert!(
            death_route
                .simulation()
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.mana == 300 && sphere.owner_entity_index == 0)
        );
        assert!(
            death_route
                .simulation()
                .economy()
                .death_bursts()
                .iter()
                .any(|burst| burst.parent_entity_index == dying_slot
                    && burst.id_entity_index == dying_slot)
        );
        let _ = death_route.step(PlayerCommand::default(), 0);
        assert!(
            death_route
                .simulation()
                .economy()
                .authored_class5()
                .iter()
                .all(|root| root.source_entity_index != dying_slot)
        );

        let mut allocation_failure = campaign.clone();
        let restored_slot = allocation_failure
            .simulation_mut()
            .economy_mut()
            .exhaust_new_event_capacity_for_test()
            .expect("the real Level 1 source arena has ordinary NewEvent capacity");

        let before_fire_cursor = campaign.effect_cursor();
        let fire_evidence = campaign.step(PlayerCommand::default(), 0);
        assert!(!fire_evidence.is_empty());
        let session = campaign.simulation();
        assert!(campaign.events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 11,
                authored_indices,
                ..
            } if *source_slot == countdown_slot && authored_indices.as_ref() == [100]
        )));
        let jar_slot = *session
            .shared_spells()
            .authored_pickup_slots()
            .get(&100)
            .expect("group 11 materializes the package Speed jar");
        assert_ne!(jar_slot, countdown_slot);
        let jar_initial_phase = jar_slot.to_le_bytes()[0];
        let jar_callbacks_on_fire_turn = jar_slot > countdown_slot;
        let fire_receipts = session.cf10_production_schedule_receipts();
        assert_eq!(
            fire_receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == jar_slot
            )),
            jar_callbacks_on_fire_turn,
            "a newly allocated row is visible in the current numeric scan exactly when its slot is still ahead of the producer"
        );
        assert_eq!(
            fire_receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == jar_slot
            )),
            jar_callbacks_on_fire_turn
        );
        let jar = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("authored Speed jar has a shared dynamic projection");
        let authored = session
            .mission()
            .authored_features()
            .iter()
            .find(|row| row.index == 100)
            .expect("package Speed row 100");
        let expected_xy = authored.world_position(0);
        let expected_position = authored.world_position(
            session
                .session
                .state
                .live
                .terrain
                .ground_height(expected_xy),
        );
        assert_eq!(
            (
                jar.class,
                jar.model,
                jar.action,
                jar.position,
                jar.subspell_index,
                jar.activation_value,
                jar.byte_3b,
                jar.mana_regen,
                jar.max_mana,
                jar.mana,
                jar.phase,
                jar.active,
            ),
            (
                15,
                3,
                10,
                expected_position,
                2,
                301,
                1,
                0,
                1_000,
                3,
                jar_initial_phase.wrapping_add(u8::from(jar_callbacks_on_fire_turn)),
                true
            )
        );
        assert!(session.economy().spell_pickups().iter().any(|row| {
            (
                row.source_entity_index,
                row.authored_index,
                row.family,
                row.action,
                row.phase,
            ) == (
                jar_slot,
                100,
                3,
                10,
                jar_initial_phase.wrapping_add(u8::from(jar_callbacks_on_fire_turn)),
            )
        }));
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert_eq!(
            campaign
                .events()
                .iter()
                .filter(|event| matches!(
                    event,
                    Mc2DispatchEvent::SpellAcquisition {
                        source_slot,
                        family: 3,
                        result: None,
                        ..
                    } if *source_slot == jar_slot
                ))
                .count(),
            usize::from(jar_callbacks_on_fire_turn)
        );

        let effects = campaign
            .effects_after(before_fire_cursor, 16_384)
            .expect("complete Level 1 countdown journal page");
        let sound_sequence = effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 41,
                    ..
                }) if entity_index == countdown_slot => Some(record.sequence),
                _ => None,
            })
            .expect("action-17 sound 41");
        let activation_sequence = effects
            .records
            .iter()
            .find_map(|record| match &record.effect {
                crate::campaign::journal::CampaignEffect::Dispatch(
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        source_slot,
                        group: 11,
                        authored_indices,
                        ..
                    },
                ) if *source_slot == countdown_slot && authored_indices.as_ref() == [100] => {
                    Some(record.sequence)
                }
                _ => None,
            })
            .expect("group-11 activation receipt");
        assert!(sound_sequence < activation_sequence);

        let _ = campaign.step(PlayerCommand::default(), 0);
        let session = campaign.simulation();
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert_eq!(
            campaign
                .events()
                .iter()
                .filter(|event| matches!(
                    event,
                    Mc2DispatchEvent::SpellAcquisition {
                        source_slot,
                        family: 3,
                        result: None,
                        ..
                    } if *source_slot == jar_slot
                ))
                .count(),
            1
        );
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|row| row.authored_index != 109),
            "the following prepass reclaims the hidden countdown producer"
        );
        let receipts = session.cf10_production_schedule_receipts();
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            ScheduleReceipt::CallbackCompleted {
                pass: SourcePass::UpdateEntities,
                slot,
                ..
            } if *slot == jar_slot
        )));
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            ScheduleReceipt::EpilogueCompleted {
                pass: SourcePass::UpdateEntities,
                slot,
                ..
            } if *slot == jar_slot
        )));
        let jar = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("uncollected Speed jar remains live after its following-pass callback");
        assert_eq!(
            jar.phase,
            jar_initial_phase
                .wrapping_add(1)
                .wrapping_add(u8::from(jar_callbacks_on_fire_turn))
        );

        let failure_evidence = allocation_failure.step(PlayerCommand::default(), 0);
        assert!(!failure_evidence.is_empty());
        let failure_session = allocation_failure.simulation();
        assert!(
            allocation_failure
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(allocation_failure.events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 11,
                authored_indices,
                ..
            } if *source_slot == countdown_slot && authored_indices.as_ref() == [100]
        )));
        assert!(
            failure_session
                .session
                .state
                .live
                .authored_features_consumed
                .contains(&100)
        );
        assert!(
            failure_session
                .shared_spells()
                .authored_pickup_slots()
                .get(&100)
                .is_none()
        );
        assert!(
            failure_session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|row| row.authored_index == 109 && !row.drawing_enabled)
        );
        assert!(
            allocation_failure
                .simulation_mut()
                .economy_mut()
                .restore_new_event_slot_for_test(restored_slot),
            "the retry discriminator restores one real NewEvent slot after the failed group scan"
        );

        let _ = allocation_failure.step(PlayerCommand::default(), 0);
        let failure_session = allocation_failure.simulation();
        assert!(allocation_failure.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::Error { .. }
                | Mc2DispatchEvent::AuthoredGroupActivated { group: 11, .. }
        )));
        assert!(
            failure_session
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|row| row.authored_index != 109)
        );
        assert!(
            failure_session
                .shared_spells()
                .authored_pickup_slots()
                .get(&100)
                .is_none()
        );
        assert!(
            failure_session
                .session
                .state
                .live
                .authored_features_consumed
                .contains(&100)
        );

        // Continue the real row-100 route through ordinary pickup. Public
        // flight, collision, and source cadence must convert the same NewEvent
        // row into the player's holder without substituting the legacy
        // reset-time Speed pickup.
        let acquisition_cursor = campaign.effect_cursor();
        let mut saw_overlapping_off_cadence_refusal = false;
        let mut acquired = None;
        for _ in 0..512 {
            let evidence = step_toward(&mut campaign, expected_position);
            assert!(!evidence.is_empty());
            assert!(
                campaign
                    .events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
            let unacquired_callback = campaign.events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::SpellAcquisition {
                        source_slot,
                        family: 3,
                        result: None,
                        ..
                    } if *source_slot == jar_slot
                )
            });
            acquired = campaign.events().iter().find_map(|event| match event {
                Mc2DispatchEvent::SpellAcquisition {
                    source_slot,
                    family: 3,
                    result: Some(outcome),
                    ..
                } if *source_slot == jar_slot => Some(*outcome),
                _ => None,
            });
            if acquired.is_some() {
                break;
            }

            let session = campaign.simulation();
            let jar = session
                .shared_spells()
                .dynamic_pickups()
                .iter()
                .find(|row| row.index == jar_slot)
                .expect("the uncollected real Speed jar remains live");
            assert_eq!(jar.action, 10);
            assert_eq!(
                u16::from_le_bytes(
                    jar.record_bytes()[44..46]
                        .try_into()
                        .expect("Speed pickup parent relation extent")
                ),
                0
            );
            assert_eq!(session.players()[0].enabled_spell_holders[3], 0);
            let player = &session.entities()[usize::from(session.state.entity_index)];
            let i16_at = |row: &crate::native_world::EntitySnapshot, offset: usize| {
                i16::from_le_bytes(
                    row.record_bytes()[offset..offset + 2]
                        .try_into()
                        .expect("entity collision extent"),
                )
            };
            let overlaps = crate::level1_mission::source_entities_overlap(
                jar.position,
                i16_at(jar, 86),
                i16_at(jar, 90),
                i16_at(jar, 92),
                player.position,
                i16_at(player, 86),
                i16_at(player, 90),
                i16_at(player, 92),
            );
            let callback_phase = jar.phase.wrapping_sub(1);
            if unacquired_callback && overlaps && callback_phase & 3 != 0 {
                saw_overlapping_off_cadence_refusal = true;
            }
        }
        assert!(
            saw_overlapping_off_cadence_refusal,
            "an overlapping off-cadence callback must leave the real jar unowned"
        );
        let acquired = acquired.expect("public flight reaches the real row-100 Speed jar");
        let player_entity_index = campaign.simulation().state.entity_index;
        assert_eq!(
            (
                acquired.family,
                acquired.source_slot,
                acquired.holder_action,
                acquired.parent_entity_index,
                acquired.presentation_timer,
            ),
            (3, jar_slot, 9, player_entity_index, 64)
        );

        let session = campaign.simulation();
        let holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("the acquired Speed holder preserves the row-100 source slot");
        assert_eq!((holder.class, holder.model, holder.action), (15, 3, 9));
        assert_eq!(
            u16::from_le_bytes(
                holder.record_bytes()[44..46]
                    .try_into()
                    .expect("Speed holder parent relation extent")
            ),
            player_entity_index
        );
        assert_eq!(
            u16::from_le_bytes(
                holder.record_bytes()[58..60]
                    .try_into()
                    .expect("Speed holder presentation timer extent")
            ),
            64
        );
        assert!(session.economy().spell_pickups().iter().any(|row| {
            row.source_entity_index == jar_slot
                && row.authored_index == 100
                && row.family == 3
                && row.action == 9
        }));
        let player = &session.players()[0];
        assert_eq!(player.enabled_spell_holders[3], jar_slot);
        assert_eq!(player.acquired_spell_flags[3], 1);
        assert!(
            (player.left_spell_slot == 3 && player.left_subspell_index == 0)
                || (player.right_spell_slot == 3 && player.right_subspell_index == 0)
        );

        let acquisition_effects = campaign
            .effects_after(acquisition_cursor, 16_384)
            .expect("complete Speed acquisition journal page");
        let pickup_sound_sequence = acquisition_effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 18,
                    ..
                }) if entity_index == player_entity_index => Some(record.sequence),
                _ => None,
            })
            .expect("ordinary Speed acquisition sound 18");
        let acquired_sequence = acquisition_effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::SharedSpell(
                    crate::native_world::Level2SharedSpellEvent::SpellAcquired {
                        source_slot,
                        player_entity_index: owner,
                        family: 3,
                        holder_action: 9,
                    },
                ) if source_slot == jar_slot && owner == player_entity_index => {
                    Some(record.sequence)
                }
                _ => None,
            })
            .expect("ordinary Speed holder acquisition event");
        assert!(pickup_sound_sequence < acquired_sequence);

        let acquired_phase = holder.phase;
        let following_evidence = campaign.step(PlayerCommand::default(), 0);
        assert!(!following_evidence.is_empty());
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(campaign.events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                source_slot,
                family: 3,
                result: Some(_),
                ..
            } if *source_slot == jar_slot
        )));
        let session = campaign.simulation();
        assert!(
            session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| {
                    matches!(
                        receipt,
                        ScheduleReceipt::SlotResolved {
                            pass: SourcePass::UpdateEntities,
                            slot,
                            selector,
                            ..
                        } if *slot == jar_slot
                            && selector.registry_key().selector
                                == crate::campaign::spec::BehaviorSelector::ClassAction {
                                    class: 15,
                                    action: 9,
                                }
                    )
                })
        );
        assert!(
            session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| matches!(
                    receipt,
                    ScheduleReceipt::CallbackCompleted {
                        pass: SourcePass::UpdateEntities,
                        slot,
                        ..
                    } if *slot == jar_slot
                ))
        );
        let holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("the following action-9 visit retains the Speed holder");
        let arena_holder = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.source_entity_index == jar_slot)
            .expect("the following visit retains the source-arena row");
        assert_eq!((holder.action, arena_holder.action), (9, 9));
        assert_eq!(
            (holder.phase, arena_holder.phase),
            (
                acquired_phase.wrapping_add(1),
                acquired_phase.wrapping_add(1)
            )
        );

        // Exercise exactly one real tier-0 GetScroll_69DB0 pulse. The player
        // command arms the acquired row during the player visit at source
        // slot 152; Speed behavior is allowed only when the numeric scheduler
        // subsequently reaches the holder's own action-9 slot. Align that visit with the
        // source's phase&3 trail cadence using ordinary no-input turns.
        let player_entity_index = session.state.entity_index;
        let player = session
            .players()
            .iter()
            .find(|player| player.entity_index == player_entity_index)
            .expect("local Level 1 player remains live")
            .clone();
        let (cast_flag, cast_source, cast_direction_mask) = if player.left_spell_slot == 3 {
            (0x10, crate::SpellFireSource::Left, 256)
        } else if player.right_spell_slot == 3 {
            (0x20, crate::SpellFireSource::Right, 512)
        } else {
            panic!("ordinary Speed acquisition must assign the real holder to one hand");
        };

        let no_cast_cursor = campaign.effect_cursor();
        let no_cast_target_speed = session.state.target_speed;
        let no_cast_experience = session.shared_spells().spell_experience()[3];
        let no_cast_trail_count = session.economy().speed_trails().len();
        let _ = campaign.step(PlayerCommand::default(), 0);
        let session = campaign.simulation();
        let holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("no-input turn retains the inactive Speed holder");
        assert_eq!(holder.active_value, 0);
        assert_eq!(session.state.target_speed, no_cast_target_speed);
        assert_eq!(
            session.shared_spells().spell_experience()[3],
            no_cast_experience
        );
        assert_eq!(session.economy().speed_trails().len(), no_cast_trail_count);
        assert!(
            campaign
                .effects_after(no_cast_cursor, 16_384)
                .expect("complete no-cast journal page")
                .records
                .iter()
                .all(|record| !matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        sound_id: 19,
                        ..
                    })
                )),
            "an idle action-9 visit must not run the Speed pulse"
        );

        let desired_precast_phase = if jar_slot > player_entity_index { 0 } else { 3 };
        for _ in 0..3 {
            let phase = campaign
                .simulation()
                .shared_spells()
                .dynamic_pickups()
                .iter()
                .find(|row| row.index == jar_slot)
                .expect("Speed holder remains live while aligning cadence")
                .phase;
            if phase & 3 == desired_precast_phase {
                break;
            }
            let before_trails = campaign.simulation().economy().speed_trails().len();
            let before_experience = campaign.simulation().shared_spells().spell_experience()[3];
            let _ = campaign.step(PlayerCommand::default(), 0);
            let session = campaign.simulation();
            assert_eq!(
                session
                    .shared_spells()
                    .dynamic_pickups()
                    .iter()
                    .find(|row| row.index == jar_slot)
                    .expect("aligned Speed holder")
                    .active_value,
                0
            );
            assert_eq!(session.economy().speed_trails().len(), before_trails);
            assert_eq!(
                session.shared_spells().spell_experience()[3],
                before_experience
            );
        }

        let session = campaign.simulation();
        let pre_cast_holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("cadence-aligned Speed holder")
            .clone();
        assert_eq!(pre_cast_holder.phase & 3, desired_precast_phase);
        assert_eq!(pre_cast_holder.active_value, 0);
        let pre_cast_target_speed = session.state.target_speed;
        let speed_direction = if pre_cast_target_speed < 0 { -1 } else { 1 };
        let expected_launch_speed = speed_direction * 240;
        let pre_cast_player = session
            .entities()
            .get(usize::from(player_entity_index))
            .expect("local player source row before Speed cast")
            .clone();
        assert_eq!(
            (pre_cast_player.mana, pre_cast_player.mana_regen),
            (1_000, 100)
        );
        let pre_cast_experience = session.shared_spells().spell_experience()[3];
        let pre_cast_trail_slots = session
            .economy()
            .speed_trails()
            .iter()
            .map(|trail| trail.source_entity_index)
            .collect::<Vec<_>>();
        let cast_cursor = campaign.effect_cursor();
        let _ = campaign.step(
            PlayerCommand {
                opcode: 6,
                argument_1: 3,
                flags: cast_flag,
                ..PlayerCommand::default()
            },
            0,
        );

        let holder_runs_on_cast_turn = jar_slot > player_entity_index;
        if !holder_runs_on_cast_turn {
            let session = campaign.simulation();
            let armed = session
                .shared_spells()
                .dynamic_pickups()
                .iter()
                .find(|row| row.index == jar_slot)
                .expect("lower-slot Speed holder remains armed after the player visit");
            assert_eq!(armed.active_value, 301);
            assert_eq!(session.state.target_speed, pre_cast_target_speed);
            assert_eq!(
                session
                    .economy()
                    .speed_trails()
                    .iter()
                    .map(|trail| trail.source_entity_index)
                    .collect::<Vec<_>>(),
                pre_cast_trail_slots
            );
            assert!(campaign.events().iter().all(|event| !matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: crate::native_world::Level2DynamicArenaKind::SpeedTrail,
                    ..
                } if *source_slot != 0
            )));
            let _ = campaign.step(PlayerCommand::default(), 0);
        }

        let session = campaign.simulation();
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|row| row.index == jar_slot)
            .expect("first Speed pulse retains the acquired holder");
        let arena_holder = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.source_entity_index == jar_slot)
            .expect("first Speed pulse retains its source-arena projection");
        let holder_visits = 1 + usize::from(!holder_runs_on_cast_turn);
        assert_eq!(holder.active_value, 300);
        assert_eq!(arena_holder.action, 9);
        assert_eq!(holder.control_flags & 0x80, 0x80);
        assert_eq!(
            holder.phase,
            pre_cast_holder
                .phase
                .wrapping_add(u8::try_from(holder_visits).expect("at most two holder visits"))
        );
        assert_eq!(arena_holder.phase, holder.phase);
        assert_eq!(session.state.target_speed, expected_launch_speed);
        assert_eq!(session.state.actual_speed, expected_launch_speed);
        assert_eq!(
            session.shared_spells().spell_experience()[3],
            pre_cast_experience + 1
        );
        assert_eq!(
            session.players()[0].runtime_spell_experience[3],
            pre_cast_experience + 1
        );
        let player_entity = session
            .entities()
            .get(usize::from(player_entity_index))
            .expect("local player source row after first Speed pulse");
        assert_eq!(player_entity.mana, 1_000);
        assert_eq!(player_entity.mana_regen, -1_000);

        let new_trails = session
            .economy()
            .speed_trails()
            .iter()
            .filter(|trail| !pre_cast_trail_slots.contains(&trail.source_entity_index))
            .collect::<Vec<_>>();
        assert_eq!(new_trails.len(), 1);
        let trail = new_trails[0];
        let trail_slot = trail.source_entity_index;
        assert_eq!(trail.source_id, pre_cast_player.source_id);
        assert_eq!(
            (
                trail.class,
                trail.model,
                trail.action,
                trail.max_life,
                trail.control_flags
            ),
            (10, 2, 2, 8, 0x0002_0001)
        );
        assert_eq!(trail.position, session.state.position);
        assert_eq!(trail.life, if trail_slot > jar_slot { 31 } else { 32 });

        let cast_effects = campaign
            .effects_after(cast_cursor, 16_384)
            .expect("complete first Speed pulse journal page");
        assert!(cast_effects.records.iter().any(|record| matches!(
            record.effect,
            crate::campaign::journal::CampaignEffect::Flight(
                FlightEvent::RequestSpellActivation {
                    source,
                    holder_entity_index,
                    direction_mask,
                }
            ) if source == cast_source
                && holder_entity_index == jar_slot
                && direction_mask == cast_direction_mask
        )));
        let speed_sound_sequence = cast_effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                    entity_index,
                    player_index: -1,
                    sound_id: 19,
                    ..
                }) if entity_index == player_entity_index => Some(record.sequence),
                _ => None,
            })
            .expect("first Speed pulse sound 19");
        let holder_receipt_sequence = cast_effects
            .records
            .iter()
            .find_map(|record| match record.effect {
                crate::campaign::journal::CampaignEffect::Dispatch(
                    Mc2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: crate::native_world::Level2DynamicArenaKind::SpellPickup,
                        ..
                    },
                ) if source_slot == jar_slot => Some(record.sequence),
                _ => None,
            })
            .expect("first Speed holder dispatch receipt");
        assert!(speed_sound_sequence < holder_receipt_sequence);

        let receipts = session.cf10_production_schedule_receipts();
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            ScheduleReceipt::CallbackCompleted {
                pass: SourcePass::UpdateEntities,
                slot,
                ..
            } if *slot == jar_slot
        )));
        assert_eq!(
            receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    slot,
                    ..
                } if *slot == trail_slot
            )),
            trail_slot > jar_slot
        );

        // Every real trail receives its class-10/action-2 callback on the
        // following numeric pass, independent of which side of the holder's
        // cursor its allocated slot occupied.
        let trail_life_after_launch = trail.life;
        let _ = campaign.step(PlayerCommand::default(), 0);
        let session = campaign.simulation();
        let trail = session
            .economy()
            .speed_trails()
            .iter()
            .find(|trail| trail.source_entity_index == trail_slot)
            .expect("Speed trail remains live after its following callback");
        assert_eq!(trail.life, trail_life_after_launch - 1);
        assert!(
            session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| {
                    matches!(
                        receipt,
                        ScheduleReceipt::SlotResolved {
                            pass: SourcePass::UpdateEntities,
                            slot,
                            selector,
                            ..
                        } if *slot == trail_slot
                            && selector.registry_key().selector
                                == crate::campaign::spec::BehaviorSelector::ClassAction {
                                    class: 10,
                                    action: 2,
                                }
                    )
                })
        );
        assert!(
            session
                .cf10_production_schedule_receipts()
                .iter()
                .any(|receipt| {
                    matches!(
                        receipt,
                        ScheduleReceipt::CallbackCompleted {
                            pass: SourcePass::UpdateEntities,
                            slot,
                            ..
                        } if *slot == trail_slot
                    )
                })
        );

        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        assert_eq!(
            campaign.simulation().players()[0].enabled_spell_holders[3],
            0
        );
        assert!(
            campaign
                .simulation()
                .shared_spells()
                .dynamic_pickups()
                .is_empty()
        );
        assert!(campaign.simulation().economy().speed_trails().is_empty());
    }

    #[test]
    fn cf15_level1_firefly_extinction_unlocks_the_real_exit_group_next_turn() {
        const GROUP6_ROWS: [u16; 6] = [108, 134, 135, 154, 189, 193];
        const FIREFLY_ROWS: [u16; 5] = [108, 134, 135, 189, 193];
        const GROUP4_ROWS: [u16; 12] = [21, 132, 137, 138, 139, 140, 141, 233, 304, 305, 307, 308];

        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let reset_hash = campaign.state_hash();
        let session = campaign.simulation_mut();

        // The preceding castle objective and row-190 gate are already proven
        // by the real package progression test. Stage only that exact group-6
        // boundary; this test owns the downstream extinction objective and
        // level-end switch chronology.
        session
            .activate_authored_group_prerequisite_for_test(6)
            .expect("real package group 6 materializes through the shared arena");
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                group: 6,
                authored_indices,
                ..
            } if authored_indices.as_ref() == GROUP6_ROWS
        )));
        let fireflies = session
            .economy()
            .authored_class5()
            .iter()
            .filter(|source| FIREFLY_ROWS.contains(&source.authored_index))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(fireflies.len(), FIREFLY_ROWS.len());

        let objective = &mut session.session.state.live.core.runtime.objectives[0];
        objective.stage_selectors[..5].copy_from_slice(&[2, 2, 2, 2, 1]);
        objective.text_selector = 4;
        objective.is_level_end = 0;
        objective.done = 0;

        let exit_gate = session
            .entities()
            .iter()
            .find(|source| {
                source.active
                    && source.class == 11
                    && source.model == 4
                    && source.action == 4
                    && source.source_id == 4
            })
            .map(|source| source.index)
            .expect("the Level 1 reset contains its package level-end gate");

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.runtime().objectives[0].stage_selectors[4], 1);
        assert_eq!(session.runtime().objectives[0].is_level_end, 0);
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 4, .. }
        )));

        // Queue source-sized damage into each real package root. The ordinary
        // numeric callbacks own mailbox consumption and the 156/157 death
        // transitions; no synthetic Firefly or objective row is introduced.
        let attacker = session.state.entity_index;
        for firefly in &fireflies {
            for _ in 0..2 {
                session.apply_fireball_impact_to_firefly_for_test(firefly.position, attacker, 500);
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.runtime().objectives[0].stage_selectors[4], 1);
        assert!(
            session
                .economy()
                .authored_class5()
                .iter()
                .filter(|source| FIREFLY_ROWS.contains(&source.authored_index))
                .all(|source| source.life < 0 && source.action == 156)
        );

        // The next pass-start family rebuild excludes life-negative roots.
        // Objective service runs after the complete numeric pass, so the gate
        // has already missed the newly written level-end state this turn.
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.runtime().objectives[0].stage_selectors[4], 2);
        assert_eq!(session.runtime().objectives[0].is_level_end, 1);
        assert!(!session.entities()[usize::from(exit_gate)].drawing_disabled());
        assert!(session.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated { group: 4, .. }
        )));

        // Only the following ordinary numeric visit may consume group 4.
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 4,
                authored_indices,
                ..
            } if *source_slot == exit_gate && authored_indices.as_ref() == GROUP4_ROWS
        )));
        assert!(session.entities()[usize::from(exit_gate)].drawing_disabled());
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        assert!(
            session
                .economy()
                .authored_class14()
                .iter()
                .any(|source| source.authored_index == 132 && source.model == 3)
        );
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.authored_index == 137 && source.model == 12)
        );
        assert!(
            session
                .economy()
                .authored_class5()
                .iter()
                .any(|source| source.authored_index == 307 && source.model == 3)
        );

        // No-Claim: this does not prove the later group-33/34 switches,
        // multipart combat, exit-trigger overlap, or the full exit animation.
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
    }

    #[test]
    fn cf15_level1_group4_area_trigger_releases_only_its_package_firefly() {
        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let reset_hash = campaign.state_hash();
        let session = campaign.simulation_mut();

        // The level-end objective and gate are proven separately. Stage only
        // their real group-4 boundary; the row-305 trigger and row-306
        // Firefly remain package-authored shared-arena behavior.
        session
            .activate_authored_group_prerequisite_for_test(4)
            .expect("real package group 4 materializes through the shared arena");
        let trigger = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 305)
            .copied()
            .expect("package row 305 is the group-33 area trigger");
        assert_eq!(
            (trigger.model, trigger.action, trigger.stage_tag),
            (0, 0, 33)
        );
        assert!(
            session
                .economy()
                .authored_class5()
                .iter()
                .all(|source| source.authored_index != 306)
        );

        let player_entity_index = session.state.entity_index;
        session
            .set_level2_entity_position(player_entity_index, trigger.position)
            .expect("move the real player into the real package trigger");
        let turns_until_cadence = usize::from((8_u8.wrapping_sub(trigger.phase & 7)) & 7) + 1;
        for _ in 1..turns_until_cadence {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().all(|event| !matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 33, .. }
            )));
        }

        let journal_start = session.effect_journal().records().len();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 33,
                authored_indices,
                ..
            } if *source_slot == trigger.source_entity_index
                && authored_indices.as_ref() == [306]
        )));
        let firefly = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == 306)
            .copied()
            .expect("package row 306 is allocated as the sixth Firefly");
        assert_eq!((firefly.model, firefly.action), (19, 159));
        assert!(
            session.effect_journal().records()[journal_start..]
                .iter()
                .all(|record| !matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        FlightEvent::PrepareSound { entity_index, .. }
                    ) if entity_index == trigger.source_entity_index
                ))
        );
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|source| source.source_entity_index != trigger.source_entity_index)
        );
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
    }

    #[test]
    fn cf15_level1_group4_worm_absence_countdown_releases_group34() {
        const GROUP34_ROWS: [u16; 3] = [309, 310, 311];

        let package = level_one_descriptor_fixture_path();
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL1)
            .expect("real Level 1 descriptor opens through the shared engine");
        let reset_hash = campaign.state_hash();
        campaign
            .simulation_mut()
            .activate_authored_group_prerequisite_for_test(4)
            .expect("real package group 4 materializes through the shared arena");
        let mut allocation_failure = campaign.clone();
        let session = campaign.simulation_mut();

        let first_worm = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == 307)
            .copied()
            .expect("package row 307 is the first Worm root");
        let countdown = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 308)
            .copied()
            .expect("package row 308 is the Worm-absence countdown");
        assert_eq!((first_worm.model, first_worm.action), (3, 25));
        assert_eq!(
            (countdown.model, countdown.action, countdown.stage_tag),
            (16, 16, 34)
        );
        assert_eq!(
            session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == first_worm.source_entity_index)
                .count(),
            16
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == countdown.source_entity_index)
                .expect("live Worm family keeps its countdown")
                .dword_0x10,
            0
        );

        let attacker_slot = session
            .entities()
            .iter()
            .find(|entity| {
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && !entity.drawing_disabled()
            })
            .map(|entity| entity.index)
            .expect("the qualified package retains a real player attacker");

        let projectile_count = session.economy().fireball_projectiles().len();

        // The nearest wrong action-26 implementation clears the target or
        // emits feedback/sound even when NewEvent has no row. Prove failure is
        // silent and retryable on a clone of this exact package state.
        {
            let failure = allocation_failure.simulation_mut();
            assert_eq!(
                failure
                    .economy_mut()
                    .stage_authored_worm_attack_for_test(307, attacker_slot),
                Some(first_worm.source_entity_index)
            );
            assert!(
                failure
                    .economy_mut()
                    .exhaust_new_event_capacity_for_test()
                    .is_some()
            );
            let before_projectiles = failure.economy().fireball_projectiles().len();
            let before_feedback =
                failure.entities()[usize::from(attacker_slot)].record_bytes()[58..60].to_vec();
            failure.step_outer_turn(PlayerCommand::default(), 0);
            let retained = failure
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == 307)
                .expect("allocation failure retains the attacking Worm");
            assert_eq!(
                (retained.action, retained.word_0x96_150),
                (26, attacker_slot)
            );
            assert_eq!(
                failure.economy().fireball_projectiles().len(),
                before_projectiles
            );
            assert_eq!(
                &failure.entities()[usize::from(attacker_slot)].record_bytes()[58..60],
                before_feedback.as_slice(),
                "failed NewEvent must not publish target feedback"
            );
            assert!(failure.dispatch_events().iter().all(|event| !matches!(
                event,
                Mc2DispatchEvent::Economy {
                    event: Level2EconomyEvent::AuthoredClass5Model3Sound { .. },
                    ..
                }
            )));
        }

        assert_eq!(
            session
                .economy_mut()
                .stage_authored_worm_attack_for_test(307, attacker_slot),
            Some(first_worm.source_entity_index)
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        let projectile = session
            .economy()
            .fireball_projectiles()
            .get(projectile_count)
            .expect("ordinary Worm action 26 allocates a real Fireball row");
        assert_eq!(
            (
                projectile.origin,
                projectile.owner_entity_index,
                projectile.source_subspell_index,
                projectile.payload,
                projectile.target_source_entity_index,
            ),
            (
                FireballProjectileOrigin::Worm,
                first_worm.source_entity_index,
                500,
                500,
                attacker_slot,
            )
        );
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::Economy {
                source_slot,
                event: Level2EconomyEvent::AuthoredClass5Model3Sound {
                    source_entity_index,
                    sound_id: 8,
                },
                ..
            } if *source_slot == first_worm.source_entity_index
                && *source_entity_index == first_worm.source_entity_index
        )));
        assert_eq!(
            u16::from_le_bytes(
                session.entities()[usize::from(attacker_slot)].record_bytes()[58..60]
                    .try_into()
                    .unwrap()
            ),
            100,
            "successful Worm Fireball applies source target feedback"
        );

        let child_before = session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .find(|child| {
                child.parent_entity_index == first_worm.source_entity_index
                    && child.particle_index == 101
            })
            .copied()
            .expect("the real Worm owns its thirteenth source-ordered body row");
        let mut family_slots = session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .filter(|child| child.parent_entity_index == first_worm.source_entity_index)
            .map(|child| child.source_entity_index)
            .collect::<Vec<_>>();
        family_slots.push(first_worm.source_entity_index);
        assert_eq!(
            session
                .economy_mut()
                .stage_authored_worm_child_mailbox_for_test(
                    307,
                    12,
                    attacker_slot,
                    child_before.life.saturating_add(1),
                ),
            Some((
                first_worm.source_entity_index,
                child_before.source_entity_index
            ))
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        let child_after = session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .find(|child| child.source_entity_index == child_before.source_entity_index)
            .copied()
            .expect("the lethal body remains through its action-232 epilogue");
        assert!(child_after.life < 0);
        assert_eq!(child_after.pending_word_0x62, 0);
        assert_eq!(child_after.word_0x26, attacker_slot);
        assert_ne!(child_after.position, child_before.position);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == 307)
                .expect("root visits before the later body row")
                .action,
            25
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == 307)
                .expect("root observes the lower-life body on the following pass")
                .action,
            28
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.authored_index == 307)
                .expect("pre-kill retains the root through the pass")
                .action,
            29
        );
        assert!(
            session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == first_worm.source_entity_index)
                .all(|child| child.action == 29)
        );

        let mut saw_mana_sphere = false;
        let mut saw_death_burst = false;
        let mut saw_body_root_identity = false;
        let mut armed = false;
        for _ in 0..10 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            saw_mana_sphere |= session.economy().events().iter().any(|event| {
                matches!(
                    event,
                    Level2EconomyEvent::ManaSphereSpawned {
                        source_entity_index,
                        ..
                    } if family_slots.contains(source_entity_index)
                )
            });
            saw_death_burst |= session.economy().events().iter().any(|event| {
                matches!(
                    event,
                    Level2EconomyEvent::DeathBurstSpawned {
                        parent_entity_index,
                        ..
                    } if family_slots.contains(parent_entity_index)
                )
            });
            saw_body_root_identity |= session.economy().events().iter().any(|event| {
                matches!(
                    event,
                    Level2EconomyEvent::DeathBurstSpawned {
                        parent_entity_index,
                        id_entity_index,
                        ..
                    } if *parent_entity_index == child_before.source_entity_index
                        && *id_entity_index == first_worm.source_entity_index
                )
            });
            let timer = session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == countdown.source_entity_index)
                .expect("countdown remains live while the family dies")
                .dword_0x10;
            if timer == 16 {
                armed = true;
                break;
            }
            assert_eq!(timer, 0, "dying action-29 bodies still block absence");
        }
        assert!(saw_mana_sphere && saw_death_burst);
        assert!(
            saw_body_root_identity,
            "a Worm body death burst retains the root identity copied by qmemcpy"
        );
        assert!(armed, "the following-pass cleanup exposes Worm absence");
        assert_eq!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == countdown.source_entity_index)
                .expect("absence arms the package countdown")
                .dword_0x10,
            16
        );

        for expected in (1..16).rev() {
            let mut observed = None;
            for _ in 0..8 {
                session.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    session
                        .dispatch_events()
                        .iter()
                        .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                    "countdown pass rolled back: {:?}",
                    session.dispatch_events()
                );
                assert!(session.dispatch_events().iter().all(|event| !matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated { group: 34, .. }
                )));
                let timer = session
                    .economy()
                    .authored_switch_producers()
                    .iter()
                    .find(|source| source.source_entity_index == countdown.source_entity_index)
                    .expect("countdown remains live before its fire visit")
                    .dword_0x10;
                if timer == expected {
                    observed = Some(timer);
                    break;
                }
            }
            assert_eq!(
                observed,
                Some(expected),
                "the clear-family countdown must make bounded progress"
            );
        }

        let journal_start = session.effect_journal().records().len();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let new_records = &session.effect_journal().records()[journal_start..];
        let sound = new_records
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(
                        FlightEvent::PrepareSound {
                            entity_index,
                            sound_id: 41,
                            ..
                        }
                    ) if entity_index == countdown.source_entity_index
                )
            })
            .expect("the countdown emits source sound 41");
        let activation = new_records
            .iter()
            .position(|record| {
                matches!(
                    &record.effect,
                    crate::campaign::journal::CampaignEffect::Dispatch(
                        Mc2DispatchEvent::AuthoredGroupActivated {
                            source_slot,
                            group: 34,
                            authored_indices,
                            ..
                        }
                    ) if *source_slot == countdown.source_entity_index
                        && authored_indices.as_ref() == GROUP34_ROWS
                )
            })
            .expect("the countdown releases exact package group 34");
        assert!(sound < activation);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .filter(|source| GROUP34_ROWS.contains(&source.authored_index))
                .count(),
            GROUP34_ROWS.len()
        );
        assert_eq!(
            session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| {
                    session.economy().authored_class5().iter().any(|root| {
                        GROUP34_ROWS.contains(&root.authored_index)
                            && root.source_entity_index == child.parent_entity_index
                    })
                })
                .count(),
            48
        );
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .all(|source| source.source_entity_index != countdown.source_entity_index)
        );
        // No-Claim: this proves the package-present dynamic Worm carrier used
        // by Levels 1/2, not every target geometry or absent action 30.
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
    }

    /// mc-iol7n census receipt: the fifteen live Level 3 reset selectors.
    const CF14_LEVEL3_LIVE_PAIRS: [(u8, u8); 15] = [
        (2, 0),
        (2, 3),
        (2, 21),
        (3, 0),
        (5, 31),
        (5, 72),
        (5, 232),
        (10, 52),
        (11, 0),
        (11, 4),
        (11, 16),
        (11, 32),
        (14, 6),
        (15, 19),
        (15, 34),
    ];

    #[test]
    fn cf14_level3_descriptor_uses_shared_engine() {
        let package = level_three_descriptor_fixture_path();

        // The one shared descriptor engine validates the qualified owner
        // Level 3 package through every data stage (identity, members, typed
        // members, source facts, operations, reset receipt) with no per-level
        // route.  Only the compiled-capability gate is exercised separately
        // below, so this receipt binds stages 1-7 and 9 to the census facts.
        let accept = |_: &super::spec::BehaviorKey| true;
        let definition =
            CampaignDefinition::preflight(&package, CampaignSelection::LEVEL3, &accept)
                .expect("census-profiled Level 3 descriptor validates through the shared engine");
        assert_eq!(definition.completed_stage, ValidationStage::ResetState);
        assert_eq!(definition.identity, CampaignSelection::LEVEL3);
        assert!(definition.reset_state_receipt.structurally_validated);
        assert_eq!(definition.source_facts.len(), 3);
        assert_eq!(definition.campaign_spec.objectives.len(), 5);
        assert_eq!(
            definition
                .campaign_spec
                .reset_sources
                .feature_boundary
                .len(),
            7
        );
        assert!(definition.campaign_spec.initial_activation.is_empty());
        assert_eq!(
            definition.campaign_spec.presentation_recipe.map_type, "Day",
            "Level 3 reuses the shared Day presentation facts"
        );
        let live_pairs = definition
            .required_behavior_keys()
            .map(|key| match key.selector {
                BehaviorSelector::ClassAction { class, action } => (class, action),
                ref other => panic!("live root uses non-ClassAction selector {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            live_pairs,
            CF14_LEVEL3_LIVE_PAIRS.to_vec(),
            "descriptor live roots must equal the census receipt"
        );
        let missing = definition
            .required_behavior_keys()
            .filter(|key| CAMPAIGN_BEHAVIOR_REGISTRY.entry(key).is_none())
            .map(|key| match key.selector {
                BehaviorSelector::ClassAction { class, action } => (class, action),
                ref other => panic!("missing root uses non-ClassAction selector {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            missing,
            Vec::<(u8, u8)>::new(),
            "every census-proven live root must have a compiled capability"
        );

        // The production route consumes the same descriptor and registry,
        // derives the local wizard from runtime -> player -> entity relations,
        // and creates one resettable Rust authority without a Level-3 branch.
        let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3)
            .expect("public Level 3 descriptor opens through the shared materializer");
        assert_eq!(
            campaign
                .inspection()
                .entities()
                .iter()
                .filter(|row| row.active)
                .count(),
            252
        );
        assert_eq!(campaign.simulation().state.entity_index, 252);
        let controller = &campaign.simulation().session.state.live.economy.entities[94];
        assert_eq!(
            (
                controller.class,
                controller.model,
                controller.action,
                controller.life,
                controller.subspell_index,
                i32::from_le_bytes(controller.record_bytes()[20..24].try_into().unwrap()),
                controller.phase,
            ),
            (14, 1, 6, 3, 48, 14, 95),
            "the real Level 3 terrain-strip row must be rehydrated losslessly"
        );
        let reset_state_hash = campaign.state_hash();
        let reset_terrain_hash = campaign.terrain_hash();
        let reset_rows = campaign
            .simulation()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| (row.index, (row.class, row.action)))
            .collect::<BTreeMap<_, _>>();
        let imported_family_before = campaign
            .simulation()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && matches!(
                        (row.class, row.model, row.action),
                        (2, 0, 0) | (2, 1, 3) | (5, 9, 72)
                    )
            })
            .map(|row| {
                (
                    row.index,
                    row.class,
                    row.model,
                    row.phase,
                    i32::from_le_bytes(row.record_bytes()[20..24].try_into().unwrap()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (2, 0))
                .count(),
            37,
            "the production proof must cover every imported Level 3 tree"
        );
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (2, 1))
                .count(),
            12,
            "the production proof must cover every imported Level 3 stone"
        );
        assert_eq!(
            imported_family_before
                .iter()
                .filter(|(_, class, model, _, _)| (*class, *model) == (5, 9))
                .count(),
            11,
            "the production proof must cover every imported Level 3 model-9 row"
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let receipts = campaign.simulation().cf10_production_schedule_receipts();
        let resolved = receipts
            .iter()
            .filter_map(|receipt| match receipt {
                ScheduleReceipt::SlotResolved {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot,
                    selector,
                    kind,
                } => Some((*slot, selector, *kind)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let unexpected_resolved = resolved
            .iter()
            .filter(|(slot, _, _)| !reset_rows.contains_key(slot))
            .map(|(slot, selector, kind)| (*slot, selector.registry_key(), *kind))
            .collect::<Vec<_>>();
        assert_eq!(
            resolved
                .iter()
                .map(|(slot, _, _)| *slot)
                .collect::<Vec<_>>(),
            reset_rows.keys().copied().collect::<Vec<_>>(),
            "the first descriptor pass must visit exactly the 252 reset rows in numeric source order; unexpected visits: {unexpected_resolved:?}"
        );
        assert!(
            resolved
                .iter()
                .all(|(_, _, kind)| *kind == VisitKind::Callback),
            "all fifteen census selector families are real callbacks"
        );
        for (slot, selector, _) in &resolved {
            let (expected_class, expected_action) = reset_rows[slot];
            assert_eq!(
                selector.registry_key().selector,
                BehaviorSelector::ClassAction {
                    class: expected_class,
                    action: expected_action,
                },
                "slot {slot} must be resolved from its lossless reset selector"
            );
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::CallbackCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot: completed_slot,
                    selector: completed,
                } if completed_slot == slot && completed == *selector
            )));
            assert!(receipts.iter().any(|receipt| matches!(
                receipt,
                ScheduleReceipt::EpilogueCompleted {
                    pass: SourcePass::UpdateEntities,
                    scan: 0,
                    slot: completed_slot,
                    original,
                    ..
                } if completed_slot == slot && original == *selector
            )));
        }
        assert!(receipts.iter().all(|receipt| !matches!(
            receipt,
            ScheduleReceipt::CleanupCompleted {
                pass: SourcePass::UpdateEntities,
                scan: 0,
                ..
            }
        )));
        assert!(receipts.iter().any(|receipt| matches!(
            receipt,
            ScheduleReceipt::ScanFinished {
                pass: SourcePass::UpdateEntities,
                scan: 0,
                eligible_visits: 252,
            }
        )));
        assert_eq!(
            receipts.len(),
            763,
            "five phases + scan bounds + 252 resolve/callback/epilogue triples"
        );
        let controller = &campaign.simulation().session.state.live.economy.entities[94];
        assert_eq!(
            (
                controller.life,
                controller.subspell_index,
                i32::from_le_bytes(controller.record_bytes()[20..24].try_into().unwrap()),
                controller.phase,
            ),
            (3, 48, 14, 96),
            "the inert controller callback still receives exactly one UpdateEntities epilogue"
        );
        for (slot, class, model, phase_before, countdown_before) in imported_family_before {
            let row = &campaign.simulation().session.state.live.economy.entities[usize::from(slot)];
            assert_eq!(
                row.phase,
                phase_before.wrapping_add(1),
                "imported {class}/{model} slot {slot} must receive one scheduler epilogue"
            );
            match (class, model) {
                (2, 0 | 1) => {
                    assert_ne!(
                        row.control_flags & 0x0002_0000,
                        0,
                        "class-2 callback must set the source byte-2 lifecycle bit at slot {slot}"
                    );
                }
                (5, 9) => assert_eq!(
                    i32::from_le_bytes(row.record_bytes()[20..24].try_into().unwrap()),
                    countdown_before.wrapping_sub(1),
                    "model-9 action72 must pre-decrement its countdown at slot {slot}"
                ),
                _ => unreachable!(),
            }
        }
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_state_hash);
        assert_eq!(campaign.terrain_hash(), reset_terrain_hash);

        // Selection mismatch fails at the outer manifest before member decode.
        let mismatch = CampaignDefinition::preflight(&package, CampaignSelection::LEVEL2, &accept)
            .expect_err("Level 2 caller against the Level 3 package must fail");
        assert_eq!(mismatch.stage, ValidationStage::OuterManifest);

        // An unprofiled selection can never borrow the Level 3 route.
        let unprofiled = CampaignSelection {
            external_level: 4,
            internal_level: 3,
            map_type_id: 0,
        };
        let unknown = CampaignDefinition::preflight(&package, unprofiled, &accept)
            .expect_err("an unprofiled selection must fail closed");
        assert_eq!(unknown.stage, ValidationStage::OuterManifest);

        // A mutated member fails the hash stage before any typed decode.
        let tampered_root = std::env::temp_dir().join(format!(
            "mc-cf14-tamper-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&tampered_root).expect("create tamper scratch");
        for entry in std::fs::read_dir(&package).expect("enumerate Level 3 package") {
            let entry = entry.expect("inspect Level 3 package entry");
            std::fs::copy(entry.path(), tampered_root.join(entry.file_name()))
                .expect("copy Level 3 member into tamper scratch");
        }
        let terrain_path = tampered_root.join("terrain.bin");
        let mut terrain = std::fs::read(&terrain_path).expect("read tamper terrain");
        terrain[0] ^= 0x01;
        std::fs::write(&terrain_path, terrain).expect("write tampered terrain");
        let tampered =
            CampaignDefinition::preflight(&tampered_root, CampaignSelection::LEVEL3, &accept)
                .expect_err("a mutated member must fail the hash stage");
        assert_eq!(tampered.stage, ValidationStage::MemberHashes);
        std::fs::remove_dir_all(&tampered_root).expect("remove tamper scratch");
    }

    #[test]
    fn cf14_level3_model9_countdown_uses_package_pose_and_stage_handoff() {
        let mut campaign = CampaignSession::open(
            level_three_descriptor_fixture_path(),
            CampaignSelection::LEVEL3,
        )
        .expect("qualified Level 3 descriptor opens");
        let before = campaign.simulation().session.state.live.economy.entities[93].clone();
        let countdown = i32::from_le_bytes(before.record_bytes()[20..24].try_into().unwrap());
        assert_eq!(
            countdown, 15,
            "the qualified descriptor preserves the captured source pre-decrement clock"
        );
        let stage_before = campaign
            .simulation()
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables[2];
        assert_eq!(
            (stage_before.kind, stage_before.flags, stage_before.word_2),
            (1, 1, 0)
        );

        for _ in 0..=countdown {
            let _ = campaign.step(PlayerCommand::default(), 0);
        }

        let after = &campaign.simulation().session.state.live.economy.entities[93];
        assert_eq!(
            after.action, 79,
            "sub_12330 must replace intermediate action 73"
        );
        assert_eq!(
            i32::from_le_bytes(after.record_bytes()[20..24].try_into().unwrap()),
            400
        );
        assert_eq!(after.actual_speed, 20);
        assert_eq!(after.particle_parameter_index, Some(201));
        assert_eq!((after.record_bytes()[96], after.record_bytes()[97]), (0, 1));
        assert_eq!(
            (
                i16::from_le_bytes(after.record_bytes()[86..88].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[88..90].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[90..92].try_into().unwrap()),
                i16::from_le_bytes(after.record_bytes()[92..94].try_into().unwrap()),
            ),
            (128, 74, 74, 128),
            "SetEntityIndexAndRot must use package particle 201 plus remc2's render-mode table"
        );
        assert_eq!(&after.record_bytes()[76..80], &[2, 1, 0, 0]);
        assert_eq!(after.position, before.position);
        assert_eq!(after.entity_rng, before.entity_rng);
        assert_eq!(after.phase, before.phase.wrapping_add(countdown as u8 + 1));
        assert_eq!(
            campaign
                .simulation()
                .session
                .state
                .live
                .core
                .runtime
                .stage_variables[2]
                .word_2,
            0x0b00,
            "all eleven source-ordered model-9 rows advance the shared StageVar once"
        );
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );

        let before_action79 = campaign
            .simulation()
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 9)
            .map(|row| (row.index, row.phase, row.entity_rng, row.position))
            .collect::<Vec<_>>();
        assert_eq!(before_action79.len(), 11);
        assert_eq!(
            before_action79
                .iter()
                .filter(|(_, phase, _, _)| phase & 7 == 0)
                .map(|(slot, _, _, _)| *slot)
                .collect::<Vec<_>>(),
            vec![248],
            "the captured phases make slot 248 the only first-pass steering row"
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let action79_rows = &campaign.simulation().session.state.live.economy.entities;
        for (slot, phase, entity_rng, _) in &before_action79 {
            let row = &action79_rows[usize::from(*slot)];
            assert_eq!(row.action, 79);
            assert_eq!(row.phase, phase.wrapping_add(1));
            assert_eq!(row.entity_rng, *entity_rng);
        }
        let packed_target = match campaign
            .simulation()
            .session
            .state
            .live
            .core
            .runtime
            .stage_variables[2]
            .relation
        {
            crate::native_world::RuntimeRelation::AxisScalar(value) => value,
            relation => panic!("model-9 StageVar target is not an axis: {relation:?}"),
        };
        let target = crate::Position {
            x: packed_target as u16,
            y: (packed_target >> 16) as u16,
            z: 0,
        };
        assert_eq!(
            i16::from_le_bytes(
                action79_rows[248].record_bytes()[36..38]
                    .try_into()
                    .unwrap()
            ),
            crate::heading_for_positions(action79_rows[248].position, target) as i16
        );
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
    }

    #[test]
    fn cf14_level3_multipart_shares_stage_target_on_eight_phase_cadence() {
        const ROOT_SLOTS: [u16; 5] = [133, 150, 167, 184, 201];
        const GROUP11_ROWS: [u16; 10] = [247, 248, 249, 250, 251, 252, 253, 254, 255, 256];
        let mut campaign = CampaignSession::open(
            level_three_descriptor_fixture_path(),
            CampaignSelection::LEVEL3,
        )
        .expect("qualified Level 3 descriptor opens");
        let reset_hash = campaign.state_hash();
        let entities = &campaign.simulation().session.state.live.economy.entities;
        assert_eq!(entities[201].phase, 5);
        assert_eq!(&entities[201].record_bytes()[78..80], &[0, 0]);
        assert_eq!(
            u16::from_le_bytes(entities[133].record_bytes()[78..80].try_into().unwrap()),
            93,
            "the first source-ordered root carries the shared target"
        );

        for _ in 0..3 {
            let _ = campaign.step(PlayerCommand::default(), 0);
        }
        let before = campaign.simulation().session.state.live.economy.entities[201].clone();
        let target_position =
            campaign.simulation().session.state.live.economy.entities[93].position;
        assert_eq!(before.phase, 8);
        assert_eq!(&before.record_bytes()[78..80], &[0, 0]);

        let _ = campaign.step(PlayerCommand::default(), 0);
        let after = &campaign.simulation().session.state.live.economy.entities[201];
        assert_eq!(after.phase, 9);
        assert_eq!(
            u16::from_le_bytes(after.record_bytes()[78..80].try_into().unwrap()),
            93
        );
        assert_ne!(after.position, before.position);
        assert_eq!(after.entity_rng, before.entity_rng);
        assert_eq!(
            i16::from_le_bytes(after.record_bytes()[36..38].try_into().unwrap()),
            crate::heading_for_positions(after.position, target_position) as i16
        );
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );

        let attacker_slot = campaign
            .simulation()
            .entities()
            .iter()
            .find(|entity| {
                entity.active
                    && entity.class == 3
                    && entity.model == 0
                    && !entity.drawing_disabled()
            })
            .map(|entity| entity.index)
            .expect("qualified Level 3 retains a real player attacker");

        let child_before = campaign.simulation().entities()[202].clone();
        assert_eq!(
            campaign
                .simulation_mut()
                .stage_imported_worm_child_mailbox_for_test(201, 0, attacker_slot, 7,),
            Some(202)
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let child_after = &campaign.simulation().entities()[202];
        assert_eq!(child_after.life, child_before.life - 7);
        assert_eq!(
            u16::from_le_bytes(child_after.record_bytes()[102..104].try_into().unwrap()),
            0
        );
        assert_ne!(child_after.position, child_before.position);
        assert_eq!(child_after.phase, child_before.phase.wrapping_add(1));
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );

        assert!(
            campaign
                .simulation_mut()
                .stage_imported_worm_root_mailbox_for_test(201, attacker_slot, false)
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let attacked = &campaign.simulation().entities()[201];
        assert_eq!(attacked.action, 26);
        assert_eq!(attacked.record_bytes()[77], 10);
        assert_eq!(
            u16::from_le_bytes(attacked.record_bytes()[154..156].try_into().unwrap()),
            attacker_slot
        );

        let mut imported_allocation_failure = campaign.clone();
        {
            let failure = imported_allocation_failure.simulation_mut();
            assert!(failure.stage_imported_worm_attack_cadence_for_test(201, attacker_slot));
            assert!(
                failure
                    .economy_mut()
                    .exhaust_new_event_capacity_for_test()
                    .is_some()
            );
            let before_projectiles = failure.economy().fireball_projectiles().len();
            let before_feedback =
                failure.entities()[usize::from(attacker_slot)].record_bytes()[58..60].to_vec();
            let journal_start = failure.effect_journal().records().len();
            failure.step_outer_turn(PlayerCommand::default(), 0);
            let retained = &failure.entities()[201];
            assert_eq!(retained.action, 26);
            assert_eq!(
                u16::from_le_bytes(retained.record_bytes()[154..156].try_into().unwrap()),
                attacker_slot
            );
            assert_eq!(
                failure.economy().fireball_projectiles().len(),
                before_projectiles
            );
            assert_eq!(
                &failure.entities()[usize::from(attacker_slot)].record_bytes()[58..60],
                before_feedback.as_slice()
            );
            assert!(
                failure.effect_journal().records()[journal_start..]
                    .iter()
                    .all(|record| !matches!(
                        record.effect,
                        crate::campaign::journal::CampaignEffect::Flight(
                            FlightEvent::PrepareSound {
                                entity_index: 201,
                                sound_id: 8,
                                ..
                            }
                        )
                    ))
            );
        }

        assert!(
            campaign
                .simulation_mut()
                .stage_imported_worm_attack_cadence_for_test(201, attacker_slot)
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert!(
            campaign
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "imported action-26 pass rolled back: {:?}",
            campaign.events()
        );
        let projectile = campaign
            .simulation()
            .economy()
            .fireball_projectiles()
            .iter()
            .rev()
            .find(|projectile| {
                projectile.origin == FireballProjectileOrigin::Worm
                    && projectile.owner_entity_index == 201
                    && projectile.target_source_entity_index == attacker_slot
            })
            .copied()
            .unwrap_or_else(|| {
                let compatibility = campaign.simulation();
                let root = &compatibility.entities()[201];
                panic!(
                    "imported action 26 did not allocate: root=({},{},{},{:?}) projectiles={:?} events={:?}",
                    root.action,
                    root.phase,
                    u16::from_le_bytes(root.record_bytes()[154..156].try_into().unwrap()),
                    root.position,
                    compatibility.economy().fireball_projectiles(),
                    campaign.events(),
                )
            });
        assert_eq!(
            (
                projectile.origin,
                projectile.owner_entity_index,
                projectile.source_subspell_index,
                projectile.payload,
                projectile.target_source_entity_index,
            ),
            (FireballProjectileOrigin::Worm, 201, 500, 500, attacker_slot)
        );
        assert!(
            campaign
                .simulation()
                .effect_journal()
                .records()
                .iter()
                .any(|record| matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index: 201,
                        sound_id: 8,
                        ..
                    })
                ))
        );

        for root_slot in ROOT_SLOTS {
            assert!(
                campaign
                    .simulation_mut()
                    .stage_imported_worm_root_mailbox_for_test(root_slot, attacker_slot, true,)
            );
        }
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert!(
            ROOT_SLOTS
                .iter()
                .all(|slot| { campaign.simulation().entities()[usize::from(*slot)].action == 28 })
        );
        let _ = campaign.step(PlayerCommand::default(), 0);
        let root_actions = ROOT_SLOTS
            .iter()
            .map(|slot| {
                let row = &campaign.simulation().entities()[usize::from(*slot)];
                (
                    *slot, row.active, row.class, row.model, row.action, row.phase,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            root_actions
                .iter()
                .all(|(_, active, class, model, action, _)| {
                    *active && *class == 5 && *model == 3 && *action == 29
                }),
            "imported pre-kill roots diverged: {root_actions:?}; events={:?}",
            campaign.events()
        );
        for root_slot in ROOT_SLOTS {
            assert!((1..=16).all(|offset| {
                campaign.simulation().entities()[usize::from(root_slot + offset)].action == 29
            }));
        }
        assert_eq!(
            campaign
                .simulation()
                .creature_runtime_census()
                .entries()
                .iter()
                .filter(|identity| {
                    identity.kind == crate::level_definition::CreatureKind::Worm
                        && identity.role
                            == crate::campaign::source_arena::CreatureRuntimeRole::MultipartPart
                })
                .count(),
            80,
            "the semantic census retains every imported body during action 29"
        );

        let family_slots = ROOT_SLOTS
            .iter()
            .flat_map(|root| {
                let root = *root;
                std::iter::once(root).chain((1..=16).map(move |offset| root + offset))
            })
            .collect::<Vec<_>>();
        // Death effects can legitimately reuse a reclaimed fixed address.
        // The obligation is reclaim of these original generations, not that
        // their addresses remain empty after subsequent source NewEvents.
        let family_handles = family_slots.iter().map(|slot| {
            campaign.simulation().source_arena().handle(*slot)
                .expect("original root/body remains current before kill callbacks")
        }).collect::<Vec<_>>();
        let mut saw_mana_sphere = false;
        let mut saw_death_burst = false;
        let mut saw_countdown_armed = false;
        for _ in 0..10 {
            let _ = campaign.step(PlayerCommand::default(), 0);
            assert!(
                campaign
                    .events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
            saw_mana_sphere |= campaign
                .simulation()
                .economy()
                .events()
                .iter()
                .any(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned {
                            source_entity_index,
                            ..
                        } if family_slots.contains(source_entity_index)
                    )
                });
            saw_death_burst |= campaign
                .simulation()
                .economy()
                .events()
                .iter()
                .any(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::DeathBurstSpawned {
                            parent_entity_index,
                            ..
                        } if family_slots.contains(parent_entity_index)
                    )
                });
            saw_countdown_armed |= i32::from_le_bytes(
                campaign.simulation().entities()[247].record_bytes()[20..24]
                    .try_into()
                    .unwrap(),
            ) == 16;
            if family_handles.iter().all(|handle| {
                campaign.simulation().source_arena().resolve(*handle).is_none()
            }) {
                break;
            }
        }
        assert!(saw_mana_sphere && saw_death_burst);
        assert!(family_handles.iter().all(|handle| {
            campaign.simulation().source_arena().resolve(*handle).is_none()
        }), "original Worm generations still resolve after the unchanged cleanup bound: {:?}",
            family_handles.iter().filter_map(|handle| {
                campaign.simulation().source_arena().resolve(*handle)
                    .map(|_| *handle)
            }).collect::<Vec<_>>());

        fn countdown_timer(source: &crate::native_world::EntitySnapshot) -> i32 {
            i32::from_le_bytes(source.record_bytes()[20..24].try_into().unwrap())
        }
        let mut timer = countdown_timer(&campaign.simulation().entities()[247]);
        for _ in 0..8 {
            if saw_countdown_armed {
                break;
            }
            let _ = campaign.step(PlayerCommand::default(), 0);
            timer = countdown_timer(&campaign.simulation().entities()[247]);
            saw_countdown_armed |= timer == 16;
        }
        assert!(
            saw_countdown_armed,
            "the imported absence countdown arms at 16"
        );
        while timer > 1 {
            let expected = timer - 1;
            let _ = campaign.step(PlayerCommand::default(), 0);
            timer = countdown_timer(&campaign.simulation().entities()[247]);
            assert_eq!(timer, expected);
        }
        assert_eq!(timer, 1);
        let journal_start = campaign.simulation().effect_journal().records().len();
        let _ = campaign.step(PlayerCommand::default(), 0);
        let records = &campaign.simulation().effect_journal().records()[journal_start..];
        let sound = records
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index: 247,
                        sound_id: 41,
                        ..
                    })
                )
            })
            .expect("the imported countdown emits sound 41");
        let activation = records
            .iter()
            .position(|record| {
                matches!(
                    &record.effect,
                    crate::campaign::journal::CampaignEffect::Dispatch(
                        Mc2DispatchEvent::AuthoredGroupActivated {
                            source_slot: 247,
                            group: 11,
                            authored_indices,
                            ..
                        }
                    ) if authored_indices.as_ref() == GROUP11_ROWS
                )
            })
            .expect("the imported countdown releases exact group 11");
        assert!(sound < activation);
        let _ = campaign.step(PlayerCommand::default(), 0);
        assert!(!campaign.simulation().entities()[247].active);
        campaign.reset();
        assert_eq!(campaign.state_hash(), reset_hash);
        // No-Claim: this proves the imported Level-3 Worm carrier only; it
        // does not claim absent action 30 or later-level multipart families.
    }

    #[test]
    fn campaign_step_reset_and_replay_preserve_observations() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 Omega fixture loads");
        let mut session: Mc2Simulation = inspection
            .level2_reset_session()
            .expect("qualified Level 2 Omega session opens");
        let initial_hash = session.campaign_state_hash();
        let initial_terrain_hash = session.terrain_hash();

        let first_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        let first_events = session.dispatch_events().to_vec();
        assert!(
            !first_evidence.is_empty(),
            "Omega route must commit one step"
        );
        assert!(
            !first_events.is_empty(),
            "Omega route must emit dispatch rows"
        );
        let first_passes = first_events
            .iter()
            .map(|event| event.campaign_projection().0)
            .collect::<Vec<_>>();
        assert!(
            first_passes.windows(2).all(|window| window[0] <= window[1]),
            "prepass/source/epilogue rows escaped their source-pass order"
        );
        let nyphur_trace = session
            .nyphur_dispatch_traces()
            .first()
            .expect("Omega route emits the ordinary Nyphur source trace");
        assert_eq!(
            (
                nyphur_trace.pass,
                nyphur_trace.source_slot,
                nyphur_trace.state_byte
            ),
            (0, 112, 0),
            "load/prepass must reach the qualified source row before projection"
        );
        assert_eq!(
            nyphur_trace.calls.as_ref(),
            &[
                Level2NyphurDispatchCall::Sub12a70,
                Level2NyphurDispatchCall::Sub12e70,
                Level2NyphurDispatchCall::Sub12e70,
            ],
            "source callback, same-pass epilogue, and commit tail order changed"
        );
        let stepped_hash = session.campaign_state_hash();
        let stepped_terrain_hash = session.terrain_hash();

        session.reset();
        assert_eq!(session.campaign_state_hash(), initial_hash);
        assert_eq!(session.terrain_hash(), initial_terrain_hash);
        assert!(
            session.dispatch_events().is_empty(),
            "reset must clear the committed dispatch projection before replay"
        );

        let replay_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(replay_evidence, first_evidence);
        assert_eq!(session.dispatch_events(), first_events.as_slice());
        assert_eq!(session.campaign_state_hash(), stepped_hash);
        assert_eq!(session.terrain_hash(), stepped_terrain_hash);
    }
}
