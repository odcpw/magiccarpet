//! Read-only CampaignState carrier for the CF-04 fold seam.
//!
//! `Level2ResetSession` remains the only production authority.  This module
//! deliberately projects that authority into the carrier shape described by
//! the campaign seam contract; it does not provide a scheduler or a writer
//! for any of the projected fields.

use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::{
    FlightContext, FlightEvent, FlightProfile, FlightState, SpellFireContext, SpellFireSource,
    TetherTarget,
    goat::GoatState,
    mission_content::MissionContentSession,
    native_world::{
        EntitySnapshot, Level2BadStone, Level2EconomyState, Level2NyphurDispatchTrace,
        Level2OrdinaryResetSession, Level2ResetSession, Level2SharedSpellResetSession,
        PlayerSnapshot, RuntimeObjective, RuntimeSnapshot, SessionTerrain,
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

/// Resettable source arena projection.  The source records remain the
/// lossless row authority during this preparation leaf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArenaState {
    pub entities: Box<[EntitySnapshot]>,
    pub topology: SourceTopology,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayersAndRuntimeState {
    pub players: Box<[PlayerSnapshot]>,
    pub runtime: RuntimeSnapshot,
    pub context: FlightContext,
}

/// Typed sidecars that currently have public read-only projections.  Their
/// internal writers remain in the old Level2ResetSession implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSidecarsState {
    pub shared_spells: Level2SharedSpellResetSession,
    pub economy: Level2EconomyState,
    pub ordinary: Level2OrdinaryResetSession,
    pub goats: Box<[GoatState]>,
    pub bad_stones: Box<[Level2BadStone]>,
    pub mission: MissionContentSession,
    pub nyphur: NyphurState,
    /// Receipt for outer-session cells that are not yet exposed as public
    /// typed borrows (mailbox/HUD/activation sidecars).  It is an observer
    /// anchor, not a new writer or a claim that those cells have migrated.
    pub legacy_campaign_state_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NyphurState {
    pub state_byte: i8,
    pub traces: Box<[Level2NyphurDispatchTrace]>,
    pub state_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRngState {
    pub global_state: u32,
    pub terrain_rng: u16,
    pub entity_streams: Box<[u16]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectiveState {
    pub stage_index: u8,
    pub objectives: Box<[RuntimeObjective]>,
}

/// The ordered journal receipt is resettable live state.  Scratch traces and
/// presentation buffers are intentionally not part of this carrier.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectJournalState {
    pub receipt: CanonicalDigest,
}

/// The complete resettable live carrier, grouped according to the census.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetState {
    pub flight: FlightState,
    pub source_arena: SourceArenaState,
    pub terrain: SessionTerrain,
    pub players_and_runtime: PlayersAndRuntimeState,
    pub source_sidecars: SourceSidecarsState,
    pub source_rng: SourceRngState,
    pub objectives: ObjectiveState,
    pub effect_journal: EffectJournalState,
}

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
}

impl ScratchState {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            validity_epoch: 0,
            valid: false,
            dispatch_receipt: CanonicalDigest::zero(),
        }
    }

    pub fn invalidate(&mut self) {
        self.validity_epoch = self.validity_epoch.wrapping_add(1);
        self.valid = false;
        self.dispatch_receipt = CanonicalDigest::zero();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignState {
    pub persistent: PersistentState,
    pub live: ResetState,
    pub scratch: ScratchState,
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

/// A read-only observation of the old authority.  It is kept separate from
/// `CampaignSession` so equality tests compare an old projection with the
/// shell projection rather than accidentally comparing a shell to itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyResetObservation {
    pub reset: ResetState,
    pub receipt: ResetReceipt,
    pub campaign_state_hash: u64,
    pub terrain_hash: u64,
}

impl LegacyResetObservation {
    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        let reset = ResetState::from_level2(session);
        let receipt = ResetReceipt::from_state(&reset);
        Self {
            reset,
            receipt,
            campaign_state_hash: session.campaign_state_hash(),
            terrain_hash: session.terrain_hash(),
        }
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
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        let mut baseline_authority = session.clone();
        baseline_authority.reset();
        let baseline = Arc::new(ResetState::from_level2(&baseline_authority));
        let live = ResetState::from_level2(session);
        let definition = Arc::new(CampaignDefinition::from_level2(session));
        let baseline_hash = ResetReceipt::from_state(&baseline);
        let scratch = ScratchState::from_level2(session, 0);
        Self {
            definition,
            baseline,
            baseline_hash,
            state: CampaignState {
                persistent: PersistentState,
                live,
                scratch,
            },
            generation: 0,
        }
    }

    /// Refresh the observer from the old authority without changing that
    /// authority.  This is the only bridge needed until a later cutover leaf.
    pub fn observe_level2(&mut self, session: &Level2ResetSession) {
        self.state.live = ResetState::from_level2(session);
        self.state.scratch = ScratchState::from_level2(session, self.generation);
    }

    /// Shell-only reset semantics; this does not reset the old authority.
    pub fn reset(&mut self) {
        self.state.live = (*self.baseline).clone();
        self.state.scratch.invalidate();
        self.generation = self.generation.wrapping_add(1);
    }
}

impl CampaignDefinition {
    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        let package_sha256 = session.package_sha256().to_owned();
        let collision_masks_present = session.collision_mask_bank().is_some();
        let identity_receipt = digest_with(|hasher| {
            hasher.update(package_sha256.as_bytes());
            hasher.update([0]);
            hasher.update([u8::from(collision_masks_present)]);
            let profile = session.profile();
            hasher.update([u8::from(profile.cave_mode)]);
            hasher.update(profile.terrain_reference_height.to_le_bytes());
            hasher.update(profile.ground_clearance.to_le_bytes());
            hasher.update(profile.ceiling_clearance.to_le_bytes());
            hasher.update(profile.passive_vertical_step.to_le_bytes());
            hasher.update(profile.raw_word_0x10.to_le_bytes());
            hasher.update(profile.raw_word_0x12.to_le_bytes());
        });
        Self {
            profile: session.profile(),
            package_sha256,
            collision_masks_present,
            identity_receipt,
        }
    }
}

impl ResetState {
    #[must_use]
    pub fn from_level2(session: &Level2ResetSession) -> Self {
        let entities = session.entities().to_vec().into_boxed_slice();
        let links = entities
            .iter()
            .map(|entity| SourceLink {
                source_slot: entity.index,
                old_map_entity: entity.old_map_entity(),
                next_entity: entity.next_entity(),
                active: entity.active,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let topology = SourceTopology {
            receipt: digest_topology(&links),
            links,
        };
        let runtime = session.runtime().clone();
        let players = session.players().to_vec().into_boxed_slice();
        let sidecars = SourceSidecarsState {
            shared_spells: session.shared_spells().clone(),
            economy: session.economy().clone(),
            ordinary: session.ordinary().clone(),
            goats: session.goats().to_vec().into_boxed_slice(),
            bad_stones: session.bad_stones().to_vec().into_boxed_slice(),
            mission: session.mission().clone(),
            nyphur: NyphurState {
                state_byte: session.nyphur_dispatch_state_byte(),
                traces: session.nyphur_dispatch_traces().to_vec().into_boxed_slice(),
                state_hash: session.nyphur_state_hash().ok(),
            },
            legacy_campaign_state_hash: session.campaign_state_hash(),
        };
        let effect_journal = EffectJournalState {
            receipt: digest_effect_journal(session),
        };
        Self {
            flight: session.state,
            source_arena: SourceArenaState { entities, topology },
            terrain: session.terrain.clone(),
            players_and_runtime: PlayersAndRuntimeState {
                players,
                runtime: runtime.clone(),
                context: session.context.clone(),
            },
            source_rng: SourceRngState {
                global_state: runtime.random_state,
                terrain_rng: session.terrain.terrain_rng(),
                entity_streams: session
                    .entities()
                    .iter()
                    .map(|entity| entity.entity_rng)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            objectives: ObjectiveState {
                stage_index: runtime.stage_index,
                objectives: runtime.objectives.clone(),
            },
            source_sidecars: sidecars,
            effect_journal,
        }
    }
}

impl ScratchState {
    #[must_use]
    fn from_level2(session: &Level2ResetSession, generation: u64) -> Self {
        Self {
            validity_epoch: generation,
            valid: true,
            dispatch_receipt: digest_effect_journal(session),
        }
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

#[must_use]
fn reset_component_digests(
    state: &ResetState,
) -> [CanonicalDigest; RESET_V1_COMPONENT_COUNT as usize] {
    [
        digest_flight(&state.flight),
        digest_source_arena(&state.source_arena),
        digest_terrain(&state.terrain),
        digest_players_and_runtime(&state.players_and_runtime),
        digest_source_sidecars(&state.source_sidecars),
        digest_source_rng(&state.source_rng),
        digest_objectives(&state.objectives),
        state.effect_journal.receipt,
    ]
}

#[must_use]
fn digest_flight(flight: &FlightState) -> CanonicalDigest {
    digest_with(|hasher| hasher.update(state_hash(flight).to_le_bytes()))
}

#[must_use]
fn digest_source_arena(arena: &SourceArenaState) -> CanonicalDigest {
    digest_with(|hasher| {
        append_len(hasher, arena.entities.len());
        for entity in &arena.entities {
            append_len(hasher, entity.record_bytes().len());
            hasher.update(entity.record_bytes());
        }
        hasher.update(arena.topology.receipt.as_bytes());
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
fn digest_players_and_runtime(state: &PlayersAndRuntimeState) -> CanonicalDigest {
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
fn digest_source_sidecars(sidecars: &SourceSidecarsState) -> CanonicalDigest {
    digest_with(|hasher| {
        append_string(hasher, &sidecars.shared_spells.reset_hash());
        append_string(hasher, &sidecars.economy.reset_hash());
        append_string(hasher, &sidecars.ordinary.reset_hash());
        append_string(hasher, &sidecars.mission.state_hash());
        hasher.update(sidecars.legacy_campaign_state_hash.to_le_bytes());
        append_len(hasher, sidecars.goats.len());
        append_len(hasher, sidecars.bad_stones.len());
        hasher.update([sidecars.nyphur.state_byte.cast_unsigned()]);
        append_string_option(hasher, sidecars.nyphur.state_hash.as_deref());
        append_len(hasher, sidecars.nyphur.traces.len());
    })
}

#[must_use]
fn digest_source_rng(rng: &SourceRngState) -> CanonicalDigest {
    digest_with(|hasher| {
        hasher.update(rng.global_state.to_le_bytes());
        hasher.update(rng.terrain_rng.to_le_bytes());
        append_len(hasher, rng.entity_streams.len());
        for stream in &rng.entity_streams {
            hasher.update(stream.to_le_bytes());
        }
    })
}

#[must_use]
fn digest_objectives(objectives: &ObjectiveState) -> CanonicalDigest {
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
fn digest_topology(links: &[SourceLink]) -> CanonicalDigest {
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

#[must_use]
fn digest_effect_journal(session: &Level2ResetSession) -> CanonicalDigest {
    digest_with(|hasher| {
        append_string(hasher, &session.dispatch_observability_hash());
        append_len(hasher, session.dispatch_events().len());
        append_len(hasher, session.economy().events().len());
        append_len(hasher, session.shared_spells().activation_events().len());
        append_len(hasher, session.shared_spells().pickup_events().len());
        append_len(hasher, session.context.events.len());
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

fn append_string_option(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            append_string(hasher, value);
        }
        None => hasher.update([0]),
    }
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
    use crate::PlayerCommand;
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

    fn assert_dual_observer_equal(session: &Level2ResetSession) {
        let public_hash = session.campaign_state_hash();
        let terrain_hash = session.terrain_hash();
        let old = LegacyResetObservation::from_level2(session);
        let shell = CampaignSession::from_level2(session);
        assert_eq!(old.reset, shell.state.live);
        assert_eq!(old.receipt.bytes(), encode_reset_v1(&shell.state.live));
        assert_eq!(old.receipt, ResetReceipt::from_state(&shell.state.live));
        assert_eq!(session.campaign_state_hash(), public_hash);
        assert_eq!(session.terrain_hash(), terrain_hash);
    }

    #[test]
    fn cf04_dual_observer_matches_frame_step_cleanup_reset() {
        let mut session = level_two_session();
        let frame_zero_hash = session.campaign_state_hash();
        let frame_zero_terrain_hash = session.terrain_hash();
        let mut observer = CampaignSession::from_level2(&session);
        assert!(observer.state.scratch.valid);
        assert_eq!(
            observer.baseline_hash,
            ResetReceipt::from_state(&observer.state.live)
        );
        assert_dual_observer_equal(&session);

        session.step_outer_turn(
            PlayerCommand {
                flags: 1,
                ..PlayerCommand::default()
            },
            0,
        );
        let stepped_hash = session.campaign_state_hash();
        assert_ne!(stepped_hash, frame_zero_hash);
        observer.observe_level2(&session);
        assert!(observer.state.scratch.valid);
        assert_eq!(
            observer.state.live,
            LegacyResetObservation::from_level2(&session).reset
        );
        assert_dual_observer_equal(&session);

        // The second ordinary turn reaches the following-pass cleanup/reclaim
        // boundary while the old session remains the only writer.
        session.step_outer_turn(PlayerCommand::default(), 0);
        observer.observe_level2(&session);
        assert_dual_observer_equal(&session);

        session.reset();
        assert_eq!(session.campaign_state_hash(), frame_zero_hash);
        assert_eq!(session.terrain_hash(), frame_zero_terrain_hash);
        observer.reset();
        assert!(!observer.state.scratch.valid);
        assert_eq!(
            observer.state.live,
            LegacyResetObservation::from_level2(&session).reset
        );
        assert_eq!(
            observer.baseline_hash,
            ResetReceipt::from_state(&observer.state.live)
        );
        observer.observe_level2(&session);
        assert!(observer.state.scratch.valid);
        assert_dual_observer_equal(&session);
    }

    #[test]
    fn cf04_omitted_field_is_detected() {
        let session = level_two_session();
        let old = LegacyResetObservation::from_level2(&session);
        assert_ne!(old.reset.effect_journal.receipt, CanonicalDigest::zero());

        let mut omitted = old.reset.clone();
        omitted.effect_journal.receipt = CanonicalDigest::zero();
        assert_ne!(
            old.receipt.bytes(),
            encode_reset_v1(&omitted),
            "omitting the effect-journal field must change the canonical receipt"
        );
    }

    #[test]
    fn cf04_failed_capacity_does_not_change_observer() {
        let session = level_two_session();
        let before = LegacyResetObservation::from_level2(&session);
        let mut undersized = vec![0_u8; RESET_V1_ENCODED_LEN - 1];
        assert_eq!(
            encode_reset_v1_into(&before.reset, &mut undersized),
            Err(EncodeResetError::OutputTooSmall {
                required: RESET_V1_ENCODED_LEN,
                capacity: RESET_V1_ENCODED_LEN - 1,
            })
        );
        assert_eq!(session.campaign_state_hash(), before.campaign_state_hash);
        assert_eq!(session.terrain_hash(), before.terrain_hash);
        assert_dual_observer_equal(&session);
    }
}
