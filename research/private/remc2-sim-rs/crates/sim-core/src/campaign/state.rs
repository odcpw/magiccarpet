//! CampaignState carrier for the campaign fold seam.
//!
//! CF-04 introduced this carrier as a read-only projection. CF-05 moved terrain,
//! CF-06 established the canonical source arena, and CF-07 moves flight,
//! players, global runtime, context, and source-RNG receipts without changing
//! the source schedule. Remaining domains stay projections until their
//! sequential cutover leaves land.

use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use super::{journal::EffectJournal, source_arena::SourceArena};

use crate::{
    FlightContext, FlightEvent, FlightProfile, FlightState, SpellFireContext, SpellFireSource,
    TetherTarget,
    goat::GoatState,
    mission_content::MissionContentSession,
    native_world::{
        Level2BadStone, Level2NyphurDispatchTrace, Level2OrdinaryResetSession, Level2ResetSession,
        Level2SharedSpellResetSession, PlayerSnapshot, RuntimeSnapshot, SessionTerrain,
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

/// Immutable receipts for the canonical live `SourceArena`.
///
/// The live records and allocator are not cloned into this reset observer;
/// doing so would recreate the shadow authority that CF-06 removes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArenaState {
    arena_receipt: CanonicalDigest,
    economy_reset_hash: String,
    source_rng_receipt: CanonicalDigest,
}

/// Typed sidecars that currently have public read-only projections.  Their
/// internal writers remain in the old Level2ResetSession implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSidecarsState {
    pub shared_spells: Level2SharedSpellResetSession,
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

/// The complete resettable live carrier, grouped according to the census.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetState {
    pub core: CanonicalRuntimeState,
    pub source_arena: SourceArenaState,
    pub terrain: SessionTerrain,
    pub source_sidecars: SourceSidecarsState,
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
    baseline_core: Arc<CanonicalRuntimeState>,
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
    pub(crate) fn from_level2_with_terrain(
        session: &Level2ResetSession,
        terrain: &SessionTerrain,
    ) -> Self {
        let live = ResetState::from_level2_with_terrain(session, terrain);
        Self {
            baseline_core: Arc::new(live.core.clone()),
            persistent: PersistentState,
            live,
            scratch: ScratchState::from_level2(session, 0),
        }
    }

    #[must_use]
    pub(crate) fn core(&self) -> &CanonicalRuntimeState {
        &self.live.core
    }

    pub(crate) fn core_mut(&mut self) -> &mut CanonicalRuntimeState {
        &mut self.live.core
    }

    /// Restore the migrated domains from one immutable aggregate baseline.
    pub(crate) fn reset_core(&mut self) {
        let next_journal_generation = self.live.core.effect_journal.generation().wrapping_add(1);
        self.live.core = (*self.baseline_core).clone();
        self.live
            .core
            .effect_journal
            .reset_to_generation(next_journal_generation);
        self.scratch.invalidate();
    }

    /// Seal an intentional pre-generation configuration into the one reset
    /// baseline before a session handle is exposed.
    pub(crate) fn rebase_core(&mut self) {
        self.baseline_core = Arc::new(self.live.core.clone());
    }

    /// Borrow the one mutable terrain carrier used by the production route.
    #[must_use]
    pub(crate) fn terrain(&self) -> &SessionTerrain {
        &self.live.terrain
    }

    /// Mutably borrow the one terrain carrier used by the production route.
    pub(crate) fn terrain_mut(&mut self) -> &mut SessionTerrain {
        &mut self.live.terrain
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
        let mut baseline = ResetState::from_level2(&baseline_authority);
        baseline
            .core
            .effect_journal
            .reset_to_generation(session.effect_journal().generation());
        let baseline = Arc::new(baseline);
        let live = ResetState::from_level2(session);
        let definition = Arc::new(CampaignDefinition::from_level2(session));
        let baseline_hash = ResetReceipt::from_state(&baseline);
        let baseline_core = Arc::new(baseline.core.clone());
        let scratch = ScratchState::from_level2(session, 0);
        Self {
            definition,
            baseline,
            baseline_hash,
            state: CampaignState {
                baseline_core,
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
        let next_journal_generation = self
            .state
            .live
            .core
            .effect_journal
            .generation()
            .wrapping_add(1);
        self.state.live = (*self.baseline).clone();
        self.state
            .live
            .core
            .effect_journal
            .reset_to_generation(next_journal_generation);
        self.state.baseline_core = Arc::new(self.baseline.core.clone());
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
        Self::from_level2_with_terrain(session, session.canonical_terrain())
    }

    #[must_use]
    pub(crate) fn from_level2_with_terrain(
        session: &Level2ResetSession,
        terrain: &SessionTerrain,
    ) -> Self {
        let sidecars = SourceSidecarsState {
            shared_spells: session.shared_spells().clone(),
            ordinary: session.ordinary().clone(),
            goats: session.goats().to_vec().into_boxed_slice(),
            bad_stones: session.bad_stones().to_vec().into_boxed_slice(),
            mission: session.mission().clone(),
            nyphur: NyphurState {
                state_byte: session.nyphur_dispatch_state_byte(),
                traces: session.nyphur_dispatch_traces().to_vec().into_boxed_slice(),
                state_hash: session.nyphur_state_hash().ok(),
            },
            legacy_campaign_state_hash: session.campaign_state_hash_with_terrain(terrain),
        };
        Self {
            core: CanonicalRuntimeState {
                state: session.state,
                players: session.players().to_vec().into_boxed_slice(),
                runtime: session.runtime().clone(),
                context: session.context.clone(),
                effect_journal: session.effect_journal().clone(),
            },
            source_arena: SourceArenaState {
                arena_receipt: digest_live_source_arena(session.source_arena()),
                economy_reset_hash: session.economy().reset_hash_with_terrain(terrain),
                source_rng_receipt: digest_source_rng_inputs(
                    session.source_global_rng(),
                    terrain,
                    session.source_arena(),
                ),
            },
            terrain: terrain.clone(),
            source_sidecars: sidecars,
        }
    }
}

impl ScratchState {
    #[must_use]
    fn from_level2(session: &Level2ResetSession, generation: u64) -> Self {
        Self {
            validity_epoch: generation,
            valid: true,
            dispatch_receipt: session.effect_journal().receipt(),
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
        state.source_arena.arena_receipt,
        digest_terrain(&state.terrain),
        digest_players_and_runtime(&state.core),
        digest_source_sidecars(
            &state.source_sidecars,
            &state.source_arena.economy_reset_hash,
            &state.terrain,
        ),
        state.source_arena.source_rng_receipt,
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
fn digest_source_sidecars(
    sidecars: &SourceSidecarsState,
    economy_reset_hash: &str,
    terrain: &SessionTerrain,
) -> CanonicalDigest {
    digest_with(|hasher| {
        append_string(
            hasher,
            &sidecars.shared_spells.reset_hash_with_terrain(terrain),
        );
        append_string(hasher, economy_reset_hash);
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

#[must_use]
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
        assert_eq!(observer.state.live.core.effect_journal.generation(), 0);
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
        assert_ne!(
            observer.baseline_hash,
            ResetReceipt::from_state(&observer.state.live),
            "reset generation must distinguish the new cursor lifetime"
        );
        assert_eq!(observer.state.live.core.effect_journal.generation(), 1);
        observer.observe_level2(&session);
        assert!(observer.state.scratch.valid);
        assert_dual_observer_equal(&session);
    }

    #[test]
    fn cf04_omitted_field_is_detected() {
        let session = level_two_session();
        let old = LegacyResetObservation::from_level2(&session);
        assert_ne!(
            old.reset.core.effect_journal.receipt(),
            CanonicalDigest::zero()
        );

        let mut omitted = encode_reset_v1(&old.reset);
        let journal_component_offset = RESET_V1_ENCODED_LEN - 32;
        omitted[journal_component_offset..].fill(0);
        assert_ne!(
            old.receipt.bytes(),
            omitted,
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
        assert_dual_observer_equal(&session);

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
        assert_dual_observer_equal(&session);

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
        std::mem::swap(
            &mut swapped.source_arena.arena_receipt,
            &mut swapped.source_arena.source_rng_receipt,
        );
        assert_ne!(encode_reset_v1(&swapped), accepted_bytes);
        let mut dropped = accepted.clone();
        dropped.source_arena.source_rng_receipt = CanonicalDigest::zero();
        assert_ne!(encode_reset_v1(&dropped), accepted_bytes);
        let mut duplicated = accepted.clone();
        let rng_receipt = duplicated.source_arena.source_rng_receipt;
        duplicated.source_arena.source_rng_receipt = digest_with(|hasher| {
            hasher.update(rng_receipt.as_bytes());
            hasher.update(rng_receipt.as_bytes());
        });
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
}
