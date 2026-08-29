//! CampaignState carrier for the campaign fold seam.
//!
//! CF-04 introduced this carrier as a read-only projection. CF-05 moved terrain,
//! CF-06 established the canonical source arena, and CF-07 moves flight,
//! players, global runtime, context, and source-RNG receipts without changing
//! the source schedule. Remaining domains stay projections until their
//! sequential cutover leaves land.

use std::{collections::BTreeMap, sync::Arc};

use sha2::{Digest as _, Sha256};

use super::{journal::EffectJournal, source_arena::SourceArena};

use crate::{
    FlightContext, FlightEvent, FlightProfile, FlightState, SpellFireContext, SpellFireSource,
    TetherTarget,
    collision_mask::CollisionMaskBank,
    level_definition::{
        BuildingKind, CreatureKind, ItemKind, LevelDefinition, LevelEntityId, LevelObjectiveId,
        MechanicKind, RivalWizardId, SceneryKind, SpellKind, SwitchKind,
    },
    native_world::{
        Level2DispatchEvent, Level2LiveState, Level2NyphurDispatchTrace, Level2ResetSession,
        LevelExitSequence, PlayerSnapshot, PresentationSelectors, RuntimeSnapshot, SessionTerrain,
    },
    spell_activation::{SpellActivationHolder, SpellCasterState},
    state_hash,
};

const RESET_V1_MAGIC: [u8; 4] = *b"MCRS";
const RESET_V1_VERSION: u16 = 1;
const RESET_V1_COMPONENT_COUNT: u8 = 8;
const RESET_V1_ENCODED_LEN: usize = 4 + 2 + 1 + (RESET_V1_COMPONENT_COUNT as usize) * (1 + 32);

/// A fixed-size digest used by the carrier for source-observable domains.
///
/// The digest is a receipt, not a replacement authority.  The actual typed
/// projections remain in the carrier structs so the later cutover leaves can
/// replace one domain at a time without changing the receipt boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDigest([u8; 32]);

impl CanonicalDigest {
    #[must_use]
    pub const fn zero() -> Self {
        Self([0; 32])
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    #[must_use]
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl Default for CanonicalDigest {
    fn default() -> Self {
        Self::zero()
    }
}

/// Immutable package identity observed from the existing Level 2 session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignDefinition {
    pub profile: FlightProfile,
    pub package_sha256: String,
    pub collision_masks_present: bool,
    pub identity_receipt: CanonicalDigest,
    semantic_level: Option<LevelDefinition>,
    authored_creature_kinds: BTreeMap<u16, CreatureKind>,
    authored_creature_entities: BTreeMap<u16, LevelEntityId>,
    initial_source_creatures: BTreeMap<u16, CreatureKind>,
    creature_models: BTreeMap<CreatureKind, u8>,
    authored_spells: BTreeMap<u16, SpellKind>,
    spell_families: BTreeMap<SpellKind, u8>,
    authored_buildings: BTreeMap<u16, BuildingKind>,
    initial_source_buildings: BTreeMap<u16, BuildingKind>,
    authored_scenery: BTreeMap<u16, SceneryKind>,
    initial_source_scenery: BTreeMap<u16, SceneryKind>,
    authored_items: BTreeMap<u16, ItemKind>,
    authored_switches: BTreeMap<u16, SwitchKind>,
    initial_source_switches: BTreeMap<u16, SwitchKind>,
    authored_mechanics: BTreeMap<u16, MechanicKind>,
    initial_source_mechanics: BTreeMap<u16, MechanicKind>,
    authored_mechanic_entities: BTreeMap<u16, LevelEntityId>,
    initial_source_mechanic_entities: BTreeMap<u16, LevelEntityId>,
    standing_building_chains: BTreeMap<LevelObjectiveId, Box<[u8]>>,
    rival_wizard_by_id: BTreeMap<RivalWizardId, u16>,
    pub(crate) collision_masks: Option<CollisionMaskBank>,
    pub(crate) presentation: PresentationSelectors,
}

/// Immutable semantic identities derived from one validated owner package.
/// This is the shared binding boundary; raw MC2 selectors remain in the
/// package adapter and live mutable state remains in the existing arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticLevelBindings {
    pub(crate) level: LevelDefinition,
    pub(crate) authored_creature_kinds: BTreeMap<u16, CreatureKind>,
    pub(crate) authored_creature_entities: BTreeMap<u16, LevelEntityId>,
    pub(crate) initial_source_creatures: BTreeMap<u16, CreatureKind>,
    pub(crate) creature_models: BTreeMap<CreatureKind, u8>,
    pub(crate) authored_spells: BTreeMap<u16, SpellKind>,
    pub(crate) spell_families: BTreeMap<SpellKind, u8>,
    pub(crate) authored_buildings: BTreeMap<u16, BuildingKind>,
    pub(crate) initial_source_buildings: BTreeMap<u16, BuildingKind>,
    pub(crate) authored_scenery: BTreeMap<u16, SceneryKind>,
    pub(crate) initial_source_scenery: BTreeMap<u16, SceneryKind>,
    pub(crate) authored_items: BTreeMap<u16, ItemKind>,
    pub(crate) authored_switches: BTreeMap<u16, SwitchKind>,
    pub(crate) initial_source_switches: BTreeMap<u16, SwitchKind>,
    pub(crate) authored_mechanics: BTreeMap<u16, MechanicKind>,
    pub(crate) initial_source_mechanics: BTreeMap<u16, MechanicKind>,
    pub(crate) authored_mechanic_entities: BTreeMap<u16, LevelEntityId>,
    pub(crate) initial_source_mechanic_entities: BTreeMap<u16, LevelEntityId>,
    pub(crate) standing_building_chains: BTreeMap<LevelObjectiveId, Box<[u8]>>,
    pub(crate) rival_wizard_by_id: BTreeMap<RivalWizardId, u16>,
}

/// Source slot/map topology observed alongside the lossless entity records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTopology {
    pub links: Box<[SourceLink]>,
    pub receipt: CanonicalDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLink {
    pub source_slot: u16,
    pub old_map_entity: u16,
    pub next_entity: u16,
    pub active: bool,
}

/// The one live owner for the state domains cut over through CF-07.
///
/// `Level2ResetSession` borrows these fields through its private campaign
/// carrier; it no longer stores parallel flight/player/runtime/RNG fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeState {
    pub state: FlightState,
    pub players: Box<[PlayerSnapshot]>,
    pub runtime: RuntimeSnapshot,
    pub context: FlightContext,
    pub effect_journal: EffectJournal,
}

/// The complete resettable live carrier, grouped according to the census.
///
/// This is the canonical name used by the campaign contract. The concrete
/// carrier lives beside the legacy Level 2 types only so its private typed
/// sidecars do not become a public package contract.
pub type ResetState = Level2LiveState;

/// No current census field is proven to survive `Level2ResetSession::reset`.
/// Keeping this as a distinct unit type prevents accidental admission of a
/// persistent field during the shell phase.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistentState;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScratchState {
    pub validity_epoch: u64,
    pub valid: bool,
    pub dispatch_receipt: CanonicalDigest,
    pub(crate) dispatch_events: Vec<Level2DispatchEvent>,
    pub(crate) economy_event_cursor: usize,
    pub(crate) nyphur_dispatch_traces: Vec<Level2NyphurDispatchTrace>,
    pub(crate) level2_day_hud_color_phase_1: u8,
    pub(crate) level2_day_hud_color_phase_2: u8,
    pub(crate) level2_day_hud_hit_frame_time_pre_draw: u8,
    pub(crate) level2_day_hud_player_panel_selector: u16,
}

impl ScratchState {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            validity_epoch: 0,
            valid: false,
            dispatch_receipt: CanonicalDigest::zero(),
            dispatch_events: Vec::new(),
            economy_event_cursor: 0,
            nyphur_dispatch_traces: Vec::new(),
            level2_day_hud_color_phase_1: 0,
            level2_day_hud_color_phase_2: 0,
            level2_day_hud_hit_frame_time_pre_draw: 0,
            level2_day_hud_player_panel_selector: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.validity_epoch = self.validity_epoch.wrapping_add(1);
        self.valid = false;
        self.dispatch_receipt = CanonicalDigest::zero();
        self.dispatch_events.clear();
        self.economy_event_cursor = 0;
        self.nyphur_dispatch_traces.clear();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignState {
    pub persistent: PersistentState,
    pub live: ResetState,
    pub scratch: ScratchState,
}

impl CampaignState {
    /// Construct the live carrier while the caller is still assembling the
    /// owning Level 2 session. The explicit terrain argument is intentional:
    /// it lets the caller install terrain exactly once without first creating
    /// a second session field that would need copy-back reconciliation.
    #[must_use]
    pub(crate) fn new(live: ResetState) -> Self {
        Self {
            persistent: PersistentState,
            live,
            scratch: ScratchState::empty(),
        }
    }

    #[must_use]
    pub(crate) fn core(&self) -> &CanonicalRuntimeState {
        &self.live.core
    }

    pub(crate) fn core_mut(&mut self) -> &mut CanonicalRuntimeState {
        &mut self.live.core
    }

    #[must_use]
    pub(crate) fn terrain(&self) -> &SessionTerrain {
        &self.live.terrain
    }

    pub(crate) fn terrain_mut(&mut self) -> &mut SessionTerrain {
        &mut self.live.terrain
    }
}

impl std::ops::Deref for CampaignState {
    type Target = ResetState;

    fn deref(&self) -> &Self::Target {
        &self.live
    }
}

impl std::ops::DerefMut for CampaignState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live
    }
}

/// Versioned bytes and digest for a reset receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetReceipt {
    bytes: Box<[u8]>,
    digest: CanonicalDigest,
}

impl ResetReceipt {
    #[must_use]
    pub fn from_state(state: &ResetState) -> Self {
        let bytes = encode_reset_v1(state);
        let digest = digest_bytes(&bytes);
        Self {
            bytes: bytes.into_boxed_slice(),
            digest,
        }
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn digest(&self) -> CanonicalDigest {
        self.digest
    }
}

/// Read-only campaign shell.  `from_level2` clones the old authority only to
/// obtain its frame-zero baseline; no shell method writes the old session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignSession {
    pub definition: Arc<CampaignDefinition>,
    pub baseline: Arc<ResetState>,
    pub baseline_hash: ResetReceipt,
    pub state: CampaignState,
    pub generation: u64,
}

impl CampaignSession {
    #[must_use]
    pub(crate) fn new(definition: CampaignDefinition, live: ResetState) -> Self {
        let definition = Arc::new(definition);
        let baseline = Arc::new(live.clone());
        let baseline_hash = ResetReceipt::from_state(&baseline);
        let scratch = live.reset_scratch(0, true);
        let mut state = CampaignState::new(live);
        state.scratch = scratch;
        Self {
            definition,
            baseline,
            baseline_hash,
            state,
            generation: 0,
        }
    }

    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        session.session.clone()
    }

    /// Shell-only reset semantics; this does not reset the old authority.
    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.state.live.clone_from(&self.baseline);
        self.state.scratch = self.state.live.reset_scratch(self.generation, false);
    }

    /// Seal an intentional pre-generation configuration into the one shared
    /// reset baseline before the session is exposed to a client.
    pub(crate) fn rebase(&mut self) {
        self.baseline = Arc::new(self.state.live.clone());
        self.baseline_hash = ResetReceipt::from_state(&self.baseline);
    }
}

impl CampaignDefinition {
    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        (*session.session.definition).clone()
    }

    #[must_use]
    pub(crate) fn new(
        profile: FlightProfile,
        package_sha256: String,
        collision_masks: Option<CollisionMaskBank>,
        presentation: PresentationSelectors,
    ) -> Self {
        let collision_masks_present = collision_masks.is_some();
        let identity_receipt = digest_with(|hasher| {
            hasher.update(package_sha256.as_bytes());
            hasher.update([0]);
            hasher.update([u8::from(collision_masks_present)]);
            hasher.update([u8::from(profile.cave_mode)]);
            hasher.update(profile.terrain_reference_height.to_le_bytes());
            hasher.update(profile.ground_clearance.to_le_bytes());
            hasher.update(profile.ceiling_clearance.to_le_bytes());
            hasher.update(profile.passive_vertical_step.to_le_bytes());
            hasher.update(profile.raw_word_0x10.to_le_bytes());
            hasher.update(profile.raw_word_0x12.to_le_bytes());
        });
        Self {
            profile,
            package_sha256,
            collision_masks_present,
            identity_receipt,
            semantic_level: None,
            authored_creature_kinds: BTreeMap::new(),
            authored_creature_entities: BTreeMap::new(),
            initial_source_creatures: BTreeMap::new(),
            creature_models: BTreeMap::new(),
            authored_spells: BTreeMap::new(),
            spell_families: BTreeMap::new(),
            authored_buildings: BTreeMap::new(),
            initial_source_buildings: BTreeMap::new(),
            authored_scenery: BTreeMap::new(),
            initial_source_scenery: BTreeMap::new(),
            authored_items: BTreeMap::new(),
            authored_switches: BTreeMap::new(),
            initial_source_switches: BTreeMap::new(),
            authored_mechanics: BTreeMap::new(),
            initial_source_mechanics: BTreeMap::new(),
            authored_mechanic_entities: BTreeMap::new(),
            initial_source_mechanic_entities: BTreeMap::new(),
            standing_building_chains: BTreeMap::new(),
            rival_wizard_by_id: BTreeMap::new(),
            collision_masks,
            presentation,
        }
    }

    pub(crate) fn bind_semantic_level(&mut self, bindings: SemanticLevelBindings) {
        self.semantic_level = Some(bindings.level);
        self.authored_creature_kinds = bindings.authored_creature_kinds;
        self.authored_creature_entities = bindings.authored_creature_entities;
        self.initial_source_creatures = bindings.initial_source_creatures;
        self.creature_models = bindings.creature_models;
        self.authored_spells = bindings.authored_spells;
        self.spell_families = bindings.spell_families;
        self.authored_buildings = bindings.authored_buildings;
        self.initial_source_buildings = bindings.initial_source_buildings;
        self.authored_scenery = bindings.authored_scenery;
        self.initial_source_scenery = bindings.initial_source_scenery;
        self.authored_items = bindings.authored_items;
        self.authored_switches = bindings.authored_switches;
        self.initial_source_switches = bindings.initial_source_switches;
        self.authored_mechanics = bindings.authored_mechanics;
        self.initial_source_mechanics = bindings.initial_source_mechanics;
        self.authored_mechanic_entities = bindings.authored_mechanic_entities;
        self.initial_source_mechanic_entities = bindings.initial_source_mechanic_entities;
        self.standing_building_chains = bindings.standing_building_chains;
        self.rival_wizard_by_id = bindings.rival_wizard_by_id;
    }

    pub(crate) fn replace_semantic_level(
        &mut self,
        level: LevelDefinition,
    ) -> std::result::Result<(), String> {
        let current = self
            .semantic_level
            .as_ref()
            .ok_or_else(|| "runtime definition has no semantic level to replace".to_owned())?;
        level.verify_session_reset_variant_of(current)?;
        self.semantic_level = Some(level);
        Ok(())
    }

    #[must_use]
    pub fn semantic_level(&self) -> Option<&LevelDefinition> {
        self.semantic_level.as_ref()
    }

    pub(crate) fn authored_creature_max_life(&self, authored_record: u16) -> Option<u32> {
        let entity_id = self.authored_creature_entities.get(&authored_record)?;
        self.semantic_level
            .as_ref()?
            .entity(entity_id)?
            .creature_health()
            .map(|health| health.max_life())
    }

    pub(crate) fn standing_building_chain(&self, objective_id: &LevelObjectiveId) -> Option<&[u8]> {
        self.standing_building_chains
            .get(objective_id)
            .map(AsRef::as_ref)
    }

    pub(crate) fn rival_wizard_entity(&self, id: &RivalWizardId) -> Option<u16> {
        self.rival_wizard_by_id.get(id).copied()
    }

    #[cfg(test)]
    pub(crate) fn replace_rival_wizard_binding_for_test(
        &mut self,
        id: RivalWizardId,
        entity_index: u16,
    ) {
        self.rival_wizard_by_id.insert(id, entity_index);
    }

    pub(crate) fn authored_creature_bindings(&self) -> &BTreeMap<u16, CreatureKind> {
        &self.authored_creature_kinds
    }

    pub(crate) fn initial_creature_bindings(&self) -> &BTreeMap<u16, CreatureKind> {
        &self.initial_source_creatures
    }

    pub(crate) fn creature_model(&self, kind: CreatureKind) -> Option<u8> {
        self.creature_models.get(&kind).copied()
    }

    pub(crate) fn spell_for_authored_record(&self, authored_record: u16) -> Option<SpellKind> {
        self.authored_spells.get(&authored_record).copied()
    }

    pub(crate) fn spell_family(&self, kind: SpellKind) -> Option<u8> {
        self.spell_families.get(&kind).copied()
    }

    pub(crate) fn building_for_authored_record(
        &self,
        authored_record: u16,
    ) -> Option<BuildingKind> {
        self.authored_buildings.get(&authored_record).copied()
    }

    pub(crate) fn building_for_initial_source_slot(
        &self,
        source_slot: u16,
    ) -> Option<BuildingKind> {
        self.initial_source_buildings.get(&source_slot).copied()
    }

    pub(crate) fn scenery_for_authored_record(&self, authored_record: u16) -> Option<SceneryKind> {
        self.authored_scenery.get(&authored_record).copied()
    }

    pub(crate) fn scenery_for_initial_source_slot(&self, source_slot: u16) -> Option<SceneryKind> {
        self.initial_source_scenery.get(&source_slot).copied()
    }

    pub(crate) fn item_for_authored_record(&self, authored_record: u16) -> Option<ItemKind> {
        self.authored_items.get(&authored_record).copied()
    }

    pub(crate) fn switch_for_authored_record(&self, authored_record: u16) -> Option<SwitchKind> {
        self.authored_switches.get(&authored_record).copied()
    }

    pub(crate) fn switch_for_initial_source_slot(&self, source_slot: u16) -> Option<SwitchKind> {
        self.initial_source_switches.get(&source_slot).copied()
    }

    pub(crate) fn mechanic_for_authored_record(
        &self,
        authored_record: u16,
    ) -> Option<MechanicKind> {
        self.authored_mechanics.get(&authored_record).copied()
    }

    pub(crate) fn mechanic_for_initial_source_slot(
        &self,
        source_slot: u16,
    ) -> Option<MechanicKind> {
        self.initial_source_mechanics.get(&source_slot).copied()
    }

    pub(crate) fn mechanic_entity_for_authored_record(
        &self,
        authored_record: u16,
    ) -> Option<&LevelEntityId> {
        self.authored_mechanic_entities.get(&authored_record)
    }

    pub(crate) fn mechanic_entity_for_initial_source_slot(
        &self,
        source_slot: u16,
    ) -> Option<&LevelEntityId> {
        self.initial_source_mechanic_entities.get(&source_slot)
    }
}

impl ResetState {
    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        session.campaign_state().live.clone()
    }
}

/// Encode the reset carrier using fixed-width, versioned bytes.  Every field
/// is represented by one of the eight ordered domain receipts; no Rust layout
/// bytes, map iteration order, pointers, or scratch state enter the result.
#[must_use]
pub fn encode_reset_v1(state: &ResetState) -> Vec<u8> {
    let mut bytes = vec![0; RESET_V1_ENCODED_LEN];
    encode_reset_v1_into(state, &mut bytes).expect("fixed reset receipt buffer fits");
    bytes
}

/// Capacity-checked form used by the failure-atomicity gate.  It performs no
/// mutation before proving that the complete fixed-size receipt fits.
pub fn encode_reset_v1_into(
    state: &ResetState,
    output: &mut [u8],
) -> Result<usize, EncodeResetError> {
    if output.len() < RESET_V1_ENCODED_LEN {
        return Err(EncodeResetError::OutputTooSmall {
            required: RESET_V1_ENCODED_LEN,
            capacity: output.len(),
        });
    }
    let mut cursor = 0;
    output[cursor..cursor + RESET_V1_MAGIC.len()].copy_from_slice(&RESET_V1_MAGIC);
    cursor += RESET_V1_MAGIC.len();
    output[cursor..cursor + 2].copy_from_slice(&RESET_V1_VERSION.to_le_bytes());
    cursor += 2;
    output[cursor] = RESET_V1_COMPONENT_COUNT;
    cursor += 1;
    for (tag, digest) in reset_component_digests(state).into_iter().enumerate() {
        output[cursor] = u8::try_from(tag).expect("reset component tag fits u8");
        cursor += 1;
        output[cursor..cursor + 32].copy_from_slice(&digest.as_bytes());
        cursor += 32;
    }
    Ok(cursor)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeResetError {
    OutputTooSmall { required: usize, capacity: usize },
}

/// A rejected transactional outer turn. Every variant is detected before the
/// candidate replaces the live session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CampaignStepError {
    StaleGeneration {
        expected: u64,
        actual: u64,
    },
    InvalidLocalPlayerBinding {
        slot: u8,
        expected_entity: u16,
        actual_entity: Option<u16>,
    },
    Encode(EncodeResetError),
}

#[must_use]
fn reset_component_digests(
    state: &ResetState,
) -> [CanonicalDigest; RESET_V1_COMPONENT_COUNT as usize] {
    [
        digest_flight(&state.core.state),
        digest_live_source_arena(&state.economy),
        digest_terrain(&state.terrain),
        digest_players_and_runtime(&state.core),
        digest_source_sidecars(state),
        digest_source_rng_inputs(state.economy.global_rng, &state.terrain, &state.economy),
        digest_objectives(&state.core.runtime.objective_state),
        state.core.effect_journal.receipt(),
    ]
}

#[must_use]
fn digest_flight(flight: &FlightState) -> CanonicalDigest {
    digest_with(|hasher| hasher.update(state_hash(flight).to_le_bytes()))
}

#[must_use]
fn digest_live_source_arena(arena: &SourceArena) -> CanonicalDigest {
    digest_with(|hasher| {
        append_len(hasher, arena.entities().len());
        for entity in arena.entities() {
            append_len(hasher, entity.record_bytes().len());
            hasher.update(entity.record_bytes());
        }
        hasher.update(arena.topology().receipt.as_bytes());
    })
}

#[must_use]
fn digest_terrain(terrain: &SessionTerrain) -> CanonicalDigest {
    digest_with(|hasher| {
        hasher.update(terrain.deterministic_hash().to_le_bytes());
        hasher.update(terrain.terrain_rng().to_le_bytes());
        hasher.update(terrain.mutation_sequence().to_le_bytes());
    })
}

#[must_use]
fn digest_players_and_runtime(state: &CanonicalRuntimeState) -> CanonicalDigest {
    digest_with(|hasher| {
        append_len(hasher, state.players.len());
        for player in &state.players {
            append_len(hasher, player.record_bytes().len());
            hasher.update(player.record_bytes());
        }
        append_len(hasher, state.runtime.bytes().len());
        hasher.update(state.runtime.bytes());
        hasher.update(digest_flight_context(&state.context).as_bytes());
    })
}

#[must_use]
fn digest_flight_context(context: &FlightContext) -> CanonicalDigest {
    digest_with(|hasher| {
        append_option_spell_holder(hasher, context.tether.holder);
        append_option_tether_target(hasher, context.tether.target);
        append_option_spell_holder(hasher, context.speed_spell_holder);
        append_option_flight_effects(hasher, context.effects.as_ref());
        append_option_spell_fire(hasher, context.spell_fire.as_ref());
        append_len(hasher, context.events.len());
        for event in &context.events {
            append_flight_event(hasher, *event);
        }
        hasher.update([u8::from(context.chat_clear_requested)]);
    })
}

fn append_option_spell_holder(hasher: &mut Sha256, holder: Option<crate::SpellHolder>) {
    match holder {
        Some(holder) => {
            hasher.update([1]);
            append_spell_holder(hasher, holder);
        }
        None => hasher.update([0]),
    }
}

fn append_spell_holder(hasher: &mut Sha256, holder: crate::SpellHolder) {
    hasher.update(holder.entity_index.to_le_bytes());
    hasher.update(holder.active_value.to_le_bytes());
}

fn append_option_tether_target(hasher: &mut Sha256, target: Option<TetherTarget>) {
    match target {
        Some(target) => {
            hasher.update([1]);
            hasher.update(target.entity_index.to_le_bytes());
            append_position(hasher, target.position);
            hasher.update(target.life.to_le_bytes());
            hasher.update(target.mana.to_le_bytes());
            hasher.update(target.mana_regen.to_le_bytes());
            hasher.update(target.life_regen.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn append_option_flight_effects(
    hasher: &mut Sha256,
    effects: Option<&crate::FlightEffectsContext>,
) {
    match effects {
        Some(effects) => {
            hasher.update([1]);
            hasher.update(effects.global_rng.to_le_bytes());
            hasher.update([
                effects.timing_salt,
                effects.local_player_index,
                effects.palette_submode,
                effects.move_speed_flag,
            ]);
            match effects.periodic_spell_effect {
                Some(effect) => hasher.update([1, effect]),
                None => hasher.update([0]),
            }
        }
        None => hasher.update([0]),
    }
}

fn append_option_spell_fire(hasher: &mut Sha256, fire: Option<&SpellFireContext>) {
    match fire {
        Some(fire) => {
            hasher.update([1]);
            hasher.update(fire.left_holder_index.to_le_bytes());
            hasher.update(fire.right_holder_index.to_le_bytes());
            hasher.update(fire.selected_holder_index.to_le_bytes());
            hasher.update([
                fire.left_subspell_index.cast_unsigned(),
                fire.right_subspell_index.cast_unsigned(),
                fire.selected_spell_ui_index,
                fire.selected_subspell_index.cast_unsigned(),
                fire.spell_menu_stage,
            ]);
            for holder in fire.enabled_holder_indices {
                hasher.update(holder.to_le_bytes());
            }
            hasher.update(fire.spell_hand_eligibility);
            hasher.update(fire.spell_toolbar_slots);
            for tier in fire.family_selected_tiers {
                hasher.update([tier.cast_unsigned()]);
            }
            for experience in fire.spell_experience {
                hasher.update(experience.to_le_bytes());
            }
            for level in fire.spell_levels {
                hasher.update([level.cast_unsigned()]);
            }
            append_spell_caster(hasher, fire.caster);
            append_len(hasher, fire.holders.len());
            for holder in &fire.holders {
                append_activation_holder(hasher, *holder);
            }
            append_len(hasher, fire.accepted_models.len());
            hasher.update(&fire.accepted_models);
            hasher.update([u8::from(fire.help_enabled)]);
        }
        None => hasher.update([0]),
    }
}

fn append_spell_caster(hasher: &mut Sha256, caster: SpellCasterState) {
    hasher.update(caster.entity_index.to_le_bytes());
    hasher.update([caster.model]);
    hasher.update(caster.mana.to_le_bytes());
    hasher.update(caster.control_flags.to_le_bytes());
    hasher.update([caster.player_color_index, caster.control_mode]);
}

fn append_activation_holder(hasher: &mut Sha256, holder: SpellActivationHolder) {
    hasher.update(holder.entity_index.to_le_bytes());
    hasher.update([holder.model, holder.level_gate.cast_unsigned()]);
    hasher.update(holder.active_value.to_le_bytes());
    hasher.update(holder.activation_value.to_le_bytes());
    hasher.update(holder.mana_cost.to_le_bytes());
    hasher.update([holder.action_marker.cast_unsigned()]);
}

fn append_flight_event(hasher: &mut Sha256, event: FlightEvent) {
    match event {
        FlightEvent::PrepareSound {
            entity_index,
            player_index,
            sound_id,
            ..
        } => {
            hasher.update([0]);
            hasher.update(entity_index.to_le_bytes());
            hasher.update(player_index.to_le_bytes());
            hasher.update(sound_id.to_le_bytes());
        }
        FlightEvent::EndLoop {
            entity_index,
            player_index,
            sound_id,
            ..
        } => {
            hasher.update([1]);
            hasher.update(entity_index.to_le_bytes());
            hasher.update(player_index.to_le_bytes());
            hasher.update(sound_id.to_le_bytes());
        }
        FlightEvent::UpdateMusic {
            mode,
            timer_multiplier,
        } => hasher.update([2, mode, timer_multiplier]),
        FlightEvent::RequestSpellActivation {
            source,
            holder_entity_index,
            direction_mask,
        } => {
            hasher.update([3, spell_fire_source_tag(source)]);
            hasher.update(holder_entity_index.to_le_bytes());
            hasher.update(direction_mask.to_le_bytes());
        }
        FlightEvent::ShowHelpHint { hint_index, state } => {
            hasher.update([4]);
            hasher.update(hint_index.to_le_bytes());
            hasher.update([state]);
        }
    }
}

const fn spell_fire_source_tag(source: SpellFireSource) -> u8 {
    match source {
        SpellFireSource::Left => 0,
        SpellFireSource::Right => 1,
        SpellFireSource::Selected => 2,
    }
}

fn append_position(hasher: &mut Sha256, position: crate::Position) {
    hasher.update(position.x.to_le_bytes());
    hasher.update(position.y.to_le_bytes());
    hasher.update(position.z.to_le_bytes());
}

#[must_use]
fn digest_source_sidecars(state: &ResetState) -> CanonicalDigest {
    digest_with(|hasher| {
        append_string(
            hasher,
            &state.shared_spells.reset_hash_with_terrain(&state.terrain),
        );
        append_string(
            hasher,
            &state.economy.reset_hash_with_terrain(&state.terrain),
        );
        append_string(hasher, &state.ordinary.reset_hash());
        append_string(hasher, &state.mission.state_hash());
        append_len(hasher, state.goats.goats().len());
        append_len(hasher, state.bad_stones.len());
        hasher.update([state.nyphur_dispatcher.state_byte().cast_unsigned()]);
        digest_outer_reset_state(hasher, state);
    })
}

fn digest_outer_reset_state(hasher: &mut Sha256, state: &ResetState) {
    if state.source_frame_timing_index != 0 {
        hasher.update(b"level2-source-frame-timing-index-v1");
        hasher.update([state.source_frame_timing_index]);
    }
    let rebuilt_model45 = state.economy.rebuilt_model45_building_selectors();
    if state.source_model45_building_selectors != rebuilt_model45 {
        hasher.update(b"level2-source-model45-building-selectors-v1");
        append_len(hasher, state.source_model45_building_selectors.len());
        hasher.update(&state.source_model45_building_selectors);
    }
    hasher.update(state.player_pending_damage.to_le_bytes());
    hasher.update(state.player_pending_attacker.to_le_bytes());
    hasher.update(state.player_life.to_le_bytes());
    hasher.update([state.player_menu_state]);
    hasher.update(b"level2-player-death-notification-v1");
    match state.player_death_notification {
        Some(notification) => {
            hasher.update([1]);
            hasher.update(notification.id.to_le_bytes());
            hasher.update([notification.state]);
            hasher.update(notification.timer.to_le_bytes());
        }
        None => hasher.update([0]),
    }
    digest_level_exit_sequence(hasher, state.level_exit);
    hasher.update([
        u8::from(state.level2_group8_activated),
        state.level2_selected_objective_stage_state,
        state.level2_objective_pause,
        u8::from(state.level2_objective_done),
    ]);
    for set in [
        &state.level2_mana_attraction_activated,
        &state.level2_model13_activated,
        &state.level2_bees_activated,
        &state.level2_switch_producers_activated,
        &state.level2_model32_activated,
        &state.level2_model76_activated,
        &state.level2_model22_activated,
        &state.level2_model39_activated,
        &state.level2_model6_activated,
        &state.level2_model59_activated,
        &state.level2_model53_activated,
        &state.level2_model1_activated,
        &state.level2_model17_activated,
        &state.level2_class14_activated,
        &state.level2_tree_activated,
        &state.level2_class5_activated,
        &state.level2_model45_activated,
        &state.authored_features_consumed,
    ] {
        append_len(hasher, set.len());
        for value in set {
            hasher.update(value.to_le_bytes());
        }
    }
    append_len(hasher, state.level2_imported_switch_roots.len());
    for root in &state.level2_imported_switch_roots {
        hasher.update(root.source_slot.to_le_bytes());
        hasher.update(root.authored_index.to_le_bytes());
        hasher.update([root.model, root.phase, root.cooldown]);
        hasher.update(root.target_group.to_le_bytes());
        hasher.update(root.word_10.to_le_bytes());
        append_position(hasher, root.position);
        hasher.update([u8::from(root.drawing_enabled), u8::from(root.active)]);
    }
    match state.level2_group10_switch25 {
        Some(switch) => {
            hasher.update([1, switch.phase]);
            hasher.update(switch.source_slot.to_le_bytes());
            hasher.update(switch.authored_index.to_le_bytes());
            hasher.update(switch.target_group.to_le_bytes());
            append_position(hasher, switch.position);
        }
        None => hasher.update([0]),
    }
}

pub(crate) fn digest_level_exit_sequence(hasher: &mut Sha256, exit: LevelExitSequence) {
    // Preserve the established default/reset bytes for every package that
    // has not started an exit. The additive tail is present only once the
    // shared action-12 sequence owns non-legacy state.
    hasher.update(b"level-exit-sequence-v1");
    hasher.update([u8::from(exit.active), u8::from(exit.complete), exit.phase]);
    hasher.update(exit.timer.to_le_bytes());
    hasher.update(exit.trigger_entity_index.to_le_bytes());
    hasher.update(exit.portal_entity_index.to_le_bytes());
    let legacy_only = LevelExitSequence {
        active: exit.active,
        complete: exit.complete,
        phase: exit.phase,
        timer: exit.timer,
        trigger_entity_index: exit.trigger_entity_index,
        portal_entity_index: exit.portal_entity_index,
        ..LevelExitSequence::default()
    };
    if exit != legacy_only {
        hasher.update(b"level-exit-sequence-extension-v1");
        hasher.update([
            u8::from(exit.portal_valid),
            u8::from(exit.end_sequence),
            exit.player_status_flags,
            exit.palette_submode,
            exit.last_movement as u8,
            u8::from(exit.display_blank),
            exit.blur_mode,
            exit.result_code,
        ]);
        hasher.update(exit.camera_fov.to_le_bytes());
    }
}

#[must_use]
fn digest_source_rng_inputs(
    global_state: u32,
    terrain: &SessionTerrain,
    arena: &SourceArena,
) -> CanonicalDigest {
    digest_with(|hasher| {
        hasher.update(global_state.to_le_bytes());
        hasher.update(terrain.terrain_rng().to_le_bytes());
        append_len(hasher, arena.entities().len());
        for entity in arena.entities() {
            hasher.update(entity.entity_rng.to_le_bytes());
        }
    })
}

#[must_use]
fn digest_objectives(objectives: &super::objectives::ObjectiveState) -> CanonicalDigest {
    digest_with(|hasher| {
        hasher.update([objectives.stage_index]);
        append_len(hasher, objectives.objectives.len());
        for objective in &objectives.objectives {
            hasher.update([
                objective.is_level_end,
                objective.text_selector,
                objective.done,
            ]);
            hasher.update(objective.stage_selectors);
        }
    })
}

#[must_use]
pub(crate) fn digest_topology(links: &[SourceLink]) -> CanonicalDigest {
    digest_with(|hasher| {
        append_len(hasher, links.len());
        for link in links {
            hasher.update(link.source_slot.to_le_bytes());
            hasher.update(link.old_map_entity.to_le_bytes());
            hasher.update(link.next_entity.to_le_bytes());
            hasher.update([u8::from(link.active)]);
        }
    })
}

fn append_len(hasher: &mut Sha256, length: usize) {
    hasher.update(
        u32::try_from(length)
            .expect("campaign reset carrier length fits u32")
            .to_le_bytes(),
    );
}

fn append_string(hasher: &mut Sha256, value: &str) {
    append_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

#[must_use]
fn digest_with(update: impl FnOnce(&mut Sha256)) -> CanonicalDigest {
    let mut hasher = Sha256::new();
    update(&mut hasher);
    CanonicalDigest(hasher.finalize().into())
}

#[must_use]
fn digest_bytes(bytes: &[u8]) -> CanonicalDigest {
    digest_with(|hasher| hasher.update(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerCommand, Position, level1_mission::CastleTerrainUpdate};
    use std::path::PathBuf;

    fn level_two_fixture_path() -> PathBuf {
        std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        )
    }

    fn level_two_session() -> Level2ResetSession {
        crate::native_world::NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package")
            .level2_reset_session()
            .expect("qualified Level 2 session")
    }

    fn assert_canonical_receipt_matches_live(session: &Level2ResetSession) {
        let public_hash = session.campaign_state_hash();
        let terrain_hash = session.terrain_hash();
        let receipt = ResetReceipt::from_state(&session.session.state.live);
        assert_eq!(
            receipt.bytes(),
            encode_reset_v1(&session.session.state.live)
        );
        assert_eq!(session.campaign_state_hash(), public_hash);
        assert_eq!(session.terrain_hash(), terrain_hash);
    }

    #[test]
    fn cf04_omitted_field_is_detected() {
        let session = level_two_session();
        let reset = ResetState::from_level2(&session);
        let receipt = ResetReceipt::from_state(&reset);
        assert_ne!(reset.core.effect_journal.receipt(), CanonicalDigest::zero());

        let mut omitted = encode_reset_v1(&reset);
        let journal_component_offset = RESET_V1_ENCODED_LEN - 32;
        omitted[journal_component_offset..].fill(0);
        assert_ne!(
            receipt.bytes(),
            omitted,
            "omitting the effect-journal field must change the canonical receipt"
        );
    }

    #[test]
    fn cf04_failed_capacity_does_not_change_observer() {
        let session = level_two_session();
        let reset = ResetState::from_level2(&session);
        let campaign_state_hash = session.campaign_state_hash();
        let terrain_hash = session.terrain_hash();
        let mut undersized = vec![0_u8; RESET_V1_ENCODED_LEN - 1];
        assert_eq!(
            encode_reset_v1_into(&reset, &mut undersized),
            Err(EncodeResetError::OutputTooSmall {
                required: RESET_V1_ENCODED_LEN,
                capacity: RESET_V1_ENCODED_LEN - 1,
            })
        );
        assert_eq!(session.campaign_state_hash(), campaign_state_hash);
        assert_eq!(session.terrain_hash(), terrain_hash);
        assert_canonical_receipt_matches_live(&session);
    }

    #[test]
    fn cf05_terrain_single_owner_preserves_source_order_and_omega() {
        let mut session = level_two_session();
        let mut replay = level_two_session();

        // Choose a real, unmarked source cell with headroom so the collision
        // write is observable. The following material write overlaps it and
        // therefore makes the two source orders distinguishable.
        let (x, y, in_type) = {
            let terrain = session.canonical_terrain();
            let mut candidate = None;
            for y in 0..=u8::MAX {
                for x in 0..=u8::MAX {
                    let index = usize::from(x) + usize::from(y) * 256;
                    if terrain.planes().angles[index] & 0x80 == 0
                        && terrain.planes().ground_heights[index] < 200
                    {
                        candidate = Some((x, y, terrain.planes().terrain_types[index]));
                        break;
                    }
                }
                if candidate.is_some() {
                    break;
                }
            }
            candidate.expect("qualified terrain contains an unmarked cell with headroom")
        };
        let position = Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        };
        let collision = CastleTerrainUpdate::AmbientCollision {
            position,
            height_delta: 1,
        };
        let material = CastleTerrainUpdate::AmbientMaterial {
            position,
            in_type,
            target_type: 20,
        };
        let apply = |session: &mut Level2ResetSession, update: CastleTerrainUpdate| {
            session
                .canonical_terrain_mut()
                .apply_ambient_update(update)
                .expect("qualified ambient terrain update");
        };

        apply(&mut session, collision);
        apply(&mut session, material);
        apply(&mut replay, collision);
        apply(&mut replay, material);
        assert_eq!(session.canonical_terrain(), replay.canonical_terrain());
        assert_eq!(session.terrain_hash(), replay.terrain_hash());
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
        assert_canonical_receipt_matches_live(&session);

        // Reversing overlapping writes is not an equivalent terrain fold:
        // material marks the cell immutable, so a later collision write is
        // rejected by the same source-order guard.
        let mut reordered = level_two_session();
        apply(&mut reordered, material);
        apply(&mut reordered, collision);
        assert_ne!(reordered.canonical_terrain(), session.canonical_terrain());
        assert_ne!(
            reordered.campaign_state_hash(),
            session.campaign_state_hash()
        );

        let frame_zero = level_two_session();
        let frame_zero_hash = frame_zero.campaign_state_hash();
        let frame_zero_terrain_hash = frame_zero.terrain_hash();
        session.step_outer_turn(PlayerCommand::default(), 0);
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.campaign_state_hash(), replay.campaign_state_hash());
        assert_eq!(session.terrain_hash(), replay.terrain_hash());
        session.reset();
        assert_eq!(session.campaign_state_hash(), frame_zero_hash);
        assert_eq!(session.terrain_hash(), frame_zero_terrain_hash);
        assert_eq!(session.canonical_terrain(), frame_zero.canonical_terrain());
    }

    #[test]
    fn cf05_stale_terrain_projection_cannot_feed_simulation() {
        let mut session = level_two_session();
        let mut control = session.clone();
        let baseline_hash = session.campaign_state_hash();
        let baseline_terrain_hash = session.terrain_hash();
        let mut stale_projection = session.canonical_terrain().clone();

        stale_projection
            .apply_ambient_update(CastleTerrainUpdate::AmbientCollision {
                position: Position { x: 0, y: 0, z: 0 },
                height_delta: 127,
            })
            .expect("stale projection mutation is locally valid");
        assert_ne!(stale_projection.deterministic_hash(), baseline_terrain_hash);
        assert_eq!(session.terrain_hash(), baseline_terrain_hash);

        // The stale copy is deliberately mutated but never passed back. The
        // real outer-turn route must read the canonical CampaignState field,
        // so it remains identical to a session that never created the copy.
        let session_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        let control_evidence = control.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session_evidence, control_evidence);
        assert_eq!(session.campaign_state_hash(), control.campaign_state_hash());
        assert_eq!(session.terrain_hash(), control.terrain_hash());
        assert_ne!(session.campaign_state_hash(), baseline_hash);
        assert_canonical_receipt_matches_live(&session);

        session.reset();
        control.reset();
        assert_eq!(session.campaign_state_hash(), baseline_hash);
        assert_eq!(session.terrain_hash(), baseline_terrain_hash);
        assert_eq!(session.canonical_terrain(), control.canonical_terrain());
    }

    #[test]
    fn cf07_rng_flight_player_runtime_cutover_preserves_order() {
        let mut session = level_two_session();
        let mut control = session.clone();
        let before_rng = session.cf07_rng_observation();
        let generation = session.campaign_state().scratch.validity_epoch;
        let command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let mut output = vec![0_u8; RESET_V1_ENCODED_LEN];

        let control_evidence = control.step_outer_turn(command, 1);
        let (evidence, written) = session
            .step_outer_turn_transactional(generation, command, 1, &mut output)
            .expect("bounded candidate commits");
        assert_eq!(written, RESET_V1_ENCODED_LEN);
        assert_eq!(evidence, control_evidence);
        assert_eq!(session.campaign_state_hash(), control.campaign_state_hash());
        assert_eq!(session.state, control.state);
        assert_eq!(session.players(), control.players());
        assert_eq!(session.runtime(), control.runtime());
        assert_eq!(session.context, control.context);
        assert_eq!(output, encode_reset_v1(&ResetState::from_level2(&session)));

        let after_rng = session.cf07_rng_observation();
        let expected_source_rng = (0..crate::entity_passes_for_speed_index(1))
            .fold(before_rng.0, |rng, _| {
                rng.wrapping_mul(9_377).wrapping_add(9_439)
            });
        assert_eq!(after_rng.0, expected_source_rng);
        assert_eq!(
            after_rng.2, before_rng.2,
            "the player/effects stream advanced outside its source callback"
        );
        assert_eq!(
            after_rng.1, before_rng.1,
            "the imported global-runtime seed became a second live RNG writer"
        );

        // The source-schedule RNG and player/effects RNG sites occupy distinct
        // canonical receipt domains. Swapping, dropping, or duplicating an
        // observation cannot alias the accepted post-turn receipt.
        let accepted = ResetState::from_level2(&session);
        let accepted_bytes = encode_reset_v1(&accepted);
        let mut swapped = accepted.clone();
        let effect_rng = swapped
            .core
            .context
            .effects
            .as_ref()
            .expect("qualified effects context")
            .global_rng;
        let source_rng = swapped.economy.global_rng;
        swapped.economy.global_rng = effect_rng;
        swapped
            .core
            .context
            .effects
            .as_mut()
            .expect("qualified effects context")
            .global_rng = source_rng;
        assert_ne!(encode_reset_v1(&swapped), accepted_bytes);
        let mut dropped = accepted.clone();
        dropped.economy.global_rng = 0;
        assert_ne!(encode_reset_v1(&dropped), accepted_bytes);
        let mut duplicated = accepted.clone();
        duplicated.economy.global_rng = duplicated.economy.global_rng.wrapping_mul(2);
        assert_ne!(encode_reset_v1(&duplicated), accepted_bytes);
        let mut wrong_player_site = accepted.clone();
        wrong_player_site
            .core
            .context
            .effects
            .as_mut()
            .expect("qualified effects context")
            .global_rng ^= 1;
        assert_ne!(encode_reset_v1(&wrong_player_site), accepted_bytes);
    }

    #[test]
    fn cf07_failed_step_is_atomic_for_rng_players_and_flight() {
        let mut session = level_two_session();
        let generation = session.campaign_state().scratch.validity_epoch;
        let before = session.clone();
        let mut undersized = vec![0xa5_u8; RESET_V1_ENCODED_LEN - 1];
        let undersized_before = undersized.clone();
        assert_eq!(
            session.step_outer_turn_transactional(
                generation,
                PlayerCommand::default(),
                0,
                &mut undersized,
            ),
            Err(CampaignStepError::Encode(
                EncodeResetError::OutputTooSmall {
                    required: RESET_V1_ENCODED_LEN,
                    capacity: RESET_V1_ENCODED_LEN - 1,
                }
            ))
        );
        assert_eq!(session, before);
        assert_eq!(undersized, undersized_before);

        let mut output = vec![0x5a_u8; RESET_V1_ENCODED_LEN];
        let output_before = output.clone();
        assert_eq!(
            session.step_outer_turn_transactional(
                generation.wrapping_add(1),
                PlayerCommand::default(),
                0,
                &mut output,
            ),
            Err(CampaignStepError::StaleGeneration {
                expected: generation.wrapping_add(1),
                actual: generation,
            })
        );
        assert_eq!(session, before);
        assert_eq!(output, output_before);

        session
            .campaign_state_mut()
            .core_mut()
            .runtime
            .local_player_slot = u8::MAX;
        let invalid_before = session.clone();
        let expected_entity = session.state.entity_index;
        assert_eq!(
            session.step_outer_turn_transactional(
                generation,
                PlayerCommand::default(),
                0,
                &mut output,
            ),
            Err(CampaignStepError::InvalidLocalPlayerBinding {
                slot: u8::MAX,
                expected_entity,
                actual_entity: None,
            })
        );
        assert_eq!(session, invalid_before);
        assert_eq!(output, output_before);
    }

    #[test]
    fn cf07_reset_replay_restores_rng_and_local_player_timing() {
        let mut session = level_two_session();
        let command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let first_generation = session.campaign_state().scratch.validity_epoch;
        let mut first_output = vec![0_u8; RESET_V1_ENCODED_LEN];
        let (first_evidence, first_written) = session
            .step_outer_turn_transactional(first_generation, command, 0, &mut first_output)
            .expect("first generation commits");
        let first_hash = session.campaign_state_hash();
        let first_rng = session.cf07_rng_observation();
        assert_eq!(first_written, RESET_V1_ENCODED_LEN);

        session.reset();
        let second_generation = session.campaign_state().scratch.validity_epoch;
        assert_eq!(second_generation, first_generation.wrapping_add(1));
        let reset_before_stale = session.clone();
        let mut stale_output = vec![0x3c_u8; RESET_V1_ENCODED_LEN];
        let stale_output_before = stale_output.clone();
        assert_eq!(
            session.step_outer_turn_transactional(first_generation, command, 0, &mut stale_output,),
            Err(CampaignStepError::StaleGeneration {
                expected: first_generation,
                actual: second_generation,
            })
        );
        assert_eq!(session, reset_before_stale);
        assert_eq!(stale_output, stale_output_before);

        let mut replay_output = vec![0_u8; RESET_V1_ENCODED_LEN];
        let (replay_evidence, replay_written) = session
            .step_outer_turn_transactional(second_generation, command, 0, &mut replay_output)
            .expect("reset generation commits");
        assert_eq!(replay_written, RESET_V1_ENCODED_LEN);
        assert_eq!(replay_evidence, first_evidence);
        assert_eq!(session.campaign_state_hash(), first_hash);
        assert_eq!(session.cf07_rng_observation(), first_rng);
        assert_eq!(replay_output, first_output);
    }

    #[test]
    fn cf09_reset_hash_isomorphism_and_live_only_transaction_clone() {
        let mut session = level_two_session();
        let baseline = Arc::clone(&session.session.baseline);
        let definition = Arc::clone(&session.session.definition);
        let baseline_receipt = session.session.baseline_hash.clone();
        let baseline_public_hash = session.campaign_state_hash();

        let mut candidate = session.clone();
        assert!(Arc::ptr_eq(&candidate.session.baseline, &baseline));
        assert!(Arc::ptr_eq(&candidate.session.definition, &definition));
        assert_ne!(
            std::ptr::addr_of!(candidate.session.state.live),
            std::ptr::addr_of!(session.session.state.live),
            "transaction candidates must clone live state, not alias it"
        );

        candidate.session.state.live.core.state.position.x ^= 1;
        let candidate_reset_receipt = ResetReceipt::from_state(&candidate.session.state.live);
        candidate.session.state.scratch.dispatch_events.push(
            crate::native_world::Level2DispatchEvent::Error {
                pass: 0,
                source_slot: 0,
                message: "planted scratch mutation".to_owned(),
            },
        );
        assert_eq!(
            ResetReceipt::from_state(&session.session.state.live),
            baseline_receipt
        );
        assert_eq!(session.session.state.live, *baseline);
        assert_ne!(candidate.session.state.live, *baseline);
        assert_eq!(
            ResetReceipt::from_state(&candidate.session.state.live),
            candidate_reset_receipt,
            "scratch is outside the canonical reset image"
        );

        session.session.state.live.core.state.position.x ^= 1;
        session.session.state.scratch.dispatch_events.push(
            crate::native_world::Level2DispatchEvent::Error {
                pass: 0,
                source_slot: 0,
                message: "discarded on reset".to_owned(),
            },
        );
        session.reset();
        assert_eq!(session.session.state.live, *baseline);
        assert_eq!(session.session.baseline_hash, baseline_receipt);
        assert_eq!(session.campaign_state_hash(), baseline_public_hash);
        assert!(session.session.state.scratch.dispatch_events.is_empty());
        assert!(!session.session.state.scratch.valid);
    }

    #[test]
    fn cf09_repeated_reset_generation_and_replay_are_exact() {
        let mut session = level_two_session();
        let baseline_bytes = encode_reset_v1(&session.session.baseline);
        let command = PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        };
        let old_cursor = session.effect_journal().cursor(session.session.generation);

        let first_evidence = session.step_outer_turn(command, 0);
        let first_hash = session.campaign_state_hash();
        let first_receipt = ResetReceipt::from_state(&session.session.state.live);
        let first_effects = session.effect_journal().clone();

        session.reset();
        assert_eq!(session.session.generation, 1);
        assert_eq!(encode_reset_v1(&session.session.state.live), baseline_bytes);
        assert!(matches!(
            session
                .effect_journal()
                .page(session.session.generation, old_cursor, 1),
            Err(super::super::journal::EffectPageError::StaleGeneration { .. })
        ));

        let replay_evidence = session.step_outer_turn(command, 0);
        assert_eq!(replay_evidence, first_evidence);
        assert_eq!(session.campaign_state_hash(), first_hash);
        assert_eq!(
            ResetReceipt::from_state(&session.session.state.live),
            first_receipt
        );
        assert_eq!(session.effect_journal(), &first_effects);

        session.reset();
        assert_eq!(session.session.generation, 2);
        assert_eq!(encode_reset_v1(&session.session.state.live), baseline_bytes);
        assert_eq!(
            ResetReceipt::from_state(&session.session.state.live),
            session.session.baseline_hash
        );
    }
}
