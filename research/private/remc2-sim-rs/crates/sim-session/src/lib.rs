//! Transport-neutral, definition-bound game session authority.
//!
//! This module owns the lifetime relationship between an immutable
//! [`GameDefinition`], one session identity, and one gameplay backend. It does
//! not expose package-specific level names or transport layouts. Publishing a
//! new definition cannot affect an existing session; only an explicit reset or
//! reopen operation may advance that session's generation.

use serde::{Deserialize, Serialize};
use sim_definition::{BoundPresentationAsset, DefinitionError, GameDefinition, SemanticId};
use std::{error::Error, fmt, num::NonZeroU64, sync::Arc};

pub mod render_publication;

pub const GAME_SESSION_CONTRACT: &str = "carpet-flight-game-session/v6";

fn serialize_u64_decimal<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn deserialize_u64_decimal<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    let value = text.parse::<u64>().map_err(serde::de::Error::custom)?;
    if value == 0 || value.to_string() != text {
        return Err(serde::de::Error::custom("invalid opaque actor identity"));
    }
    Ok(value)
}

fn deserialize_optional_actor<'de, D>(deserializer: D) -> Result<Option<RuntimeActorId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let text = Option::<String>::deserialize(deserializer)?;
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let value = text.parse::<u64>().map_err(serde::de::Error::custom)?;
    if value == 0 || value.to_string() != text {
        return Err(serde::de::Error::custom("invalid opaque actor identity"));
    }
    Ok(Some(RuntimeActorId(value)))
}

fn serialize_u64_hex<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{value:016x}"))
}

/// Transport-independent player pose in the engine's deterministic fixed
/// coordinate system. Unit metadata is discoverable from the semantic
/// catalog; source entity or slot identifiers are deliberately absent.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PlayerPose {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub display_pitch: i16,
    pub effective_pitch: i16,
    pub roll: i16,
}

/// Renderer view after the game adapter applies its view transform. It is
/// distinct from movement angles and never feeds back into player physics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CameraPose {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PlayerMotion {
    pub target_speed: i16,
    pub actual_speed: i16,
    pub strafe_speed: i16,
}

/// Authoritative local-player resource state for presentation and automation.
///
/// These values are observations of the live Rust-owned player carrier. They
/// are not client-owned gauges or a second gameplay authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PlayerVitals {
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub max_mana: i32,
}

/// A live participant notice, independent of the participant's current actor.
/// The backend owns its frame clock; rendering this observation never ages it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ParticipantNotification {
    pub participant: RuntimeParticipantId,
    pub is_local: bool,
    pub display_name: String,
    pub kind: SemanticId,
    /// Zero is the last visible frame, rather than an inactive notice.
    pub remaining_frames: i16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceSorcererScore {
    pub source_slot: u8,
    pub display_name: String,
    pub outline_palette_index: u8,
    pub text_palette_index: u8,
    pub max_mana: i32,
    pub kills8: [i16; 8],
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SourceScoreboard {
    pub players: Vec<SourceSorcererScore>,
    pub projection_error: Option<String>,
}

/// Current hand assignment; an acquired spell may also be selected without
/// occupying either hand. This is observed authority, not a client preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellHand {
    None,
    Left,
    Right,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellAvailability {
    Ready,
    NotAcquired,
    InvalidHolder,
    InvalidCaster,
    Active,
    InsufficientMana,
}

/// One supported semantic action at this observation's generation and tick.
/// Tier/cost describe the live acquired holder (zero when absent). Readiness
/// uses the gameplay activation gate and known emission prerequisites at this
/// boundary, before the next source pass; it does not promise a future target,
/// allocation or hit will succeed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SpellObservation {
    pub id: SemanticId,
    pub display_name: String,
    pub acquired: bool,
    /// Opaque live-holder identity, interpreted together with the enclosing
    /// session generation. It changes when the source slot is reused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_actor: Option<RuntimeActorId>,
    pub selected: bool,
    pub tier: i8,
    pub unlocked_tier: u8,
    pub menu_slot: u8,
    /// Indexed sprites in the retained spell-menu artwork bank. A client
    /// consumes these bindings without interpreting the semantic spell ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_artwork: Option<SpellMenuArtwork>,
    /// Bound tier text and authoritative menu-help predicates. Presenters
    /// select/layout the source hints without reimplementing casting rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_help: Option<SpellMenuHelp>,
    pub mana_cost: i32,
    pub hand: SpellHand,
    pub marked_hand: SpellMarkHand,
    pub availability: SpellAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SpellMenuArtwork {
    pub grid_icon: u16,
    pub tier_icons: [u16; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SpellMenuHelp {
    pub is_standard: bool,
    pub selected_tier: i8,
    pub title_records: [u16; 3],
    pub castle_blocked: [bool; 3],
    pub mana_unavailable: bool,
    pub definition_enabled: bool,
    pub blink_hidden: bool,
    pub discovered: bool,
    pub tier_mana_costs: [i32; 3],
    pub experience: i32,
    pub experience_thresholds: [i32; 3],
}

/// Authoritative progress through the bound level's semantic objectives.
///
/// The optional current objective is the semantic identity selected by the
/// live Rust runtime. Clients may present this projection but do not infer or
/// advance objective state themselves.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ObjectiveProgress {
    pub current_objective: Option<SemanticId>,
    pub completed_count: u32,
    pub total_count: u32,
    pub all_objectives_satisfied: bool,
}

/// Package-neutral pose for one live presentation entity. Coordinates use
/// the same deterministic world units as [`PlayerPose`]; adapter-private
/// source slots and render selectors never cross this projection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PresentationPose {
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
}

/// One live semantic presentation entity bound to an immutable package asset.
/// The opaque actor ID is stable only for the current session generation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresentationEntity {
    pub actor: RuntimeActorId,
    pub kind: SemanticId,
    pub pose: PresentationPose,
    pub logical_height: u16,
    pub asset: GameAssetReference,
}

/// Canonical bounded session observation used by adapters and transports.
///
/// Compatibility-only profile, level, source-slot, class/model/action, and
/// raw spell-holder selectors remain private to the MC2-owned adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GameSnapshot {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub tick: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub event_head: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub state_hash: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub conformance_hash: u64,
    pub player_pose: PlayerPose,
    pub player_motion: PlayerMotion,
    pub caster_mana: i32,
    pub objective_count: u32,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub action_sequence: u64,
}

/// Canonical bounded observation for descriptor-backed campaign execution.
///
/// Campaign preservation adapters bind exact retained production identities
/// as content digests. Compatibility-only schedule keys, source slots, and
/// class/model/action selectors never cross this projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CampaignSnapshot {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub tick: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub effect_head: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub state_hash: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub terrain_hash: u64,
    /// SHA-256 receipt of the complete retained production-effect journal.
    pub effect_journal_sha256: String,
    pub player_pose: PlayerPose,
    pub player_motion: PlayerMotion,
    pub player_vitals: PlayerVitals,
    /// Admission of the source menu command applied on this exact tick, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_menu_admission: Option<SourceMenuAdmission>,
    /// The ordinary player sequence requested a return to the frontend.
    /// This is a live result, distinct from completion or a secret return.
    pub return_to_title_requested: bool,
    pub source_scoreboard: SourceScoreboard,
    pub participant_notifications: Vec<ParticipantNotification>,
    /// Source player contact counter used by the ordinary wall-help poll.
    pub player_wall_contact_counter: i8,
    /// Source Spider immobilization countdown; presentation must not invent one.
    pub player_immobilized_ticks: u16,
    pub source_camera_pose: CameraPose,
    pub source_camera_fov: i16,
    pub spell_menu_has_any_spell: bool,
    pub spell_menu_wizard_colors: [u8; 2],
    pub spell_menu_panels_opaque: bool,
    /// Source global left/right hand-selection visibility, decremented by
    /// PlayerEvents before accepted hand assignment retriggers eight updates.
    pub player_hand_selection_flash_ticks: [u8; 2],
    pub player_spells: Vec<SpellObservation>,
    pub objective_progress: ObjectiveProgress,
    /// Authoritative objective-panel countdown; clients never invent a timer.
    pub objective_textbox_ticks: u8,
    /// Selected owner-language record for the source special announcement.
    /// Absent means that the ordinary objective binding remains authoritative.
    pub objective_secret_announcement_record: Option<u16>,
    pub exit_sequence_active: bool,
    pub presentation_entities: Vec<PresentationEntity>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LongitudinalControl {
    #[default]
    Coast,
    Accelerate,
    Decelerate,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LateralControl {
    #[default]
    Neutral,
    Negative,
    Positive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellAssignmentHand {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpellAssignment {
    pub spell_id: SemanticId,
    pub tier: u8,
    pub hand: SpellAssignmentHand,
    /// Optional observation binding; omitted by semantic-only clients.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_actor",
        skip_serializing_if = "Option::is_none"
    )]
    pub holder_actor: Option<RuntimeActorId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellMarkHand {
    None,
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpellMark {
    pub spell_id: SemanticId,
    pub hand: SpellMarkHand,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_actor",
        skip_serializing_if = "Option::is_none"
    )]
    pub holder_actor: Option<RuntimeActorId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellCycleDirection {
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MarkedSpellCycle {
    pub hand: SpellAssignmentHand,
    pub direction: SpellCycleDirection,
}

/// Semantic menu input; the game adapter owns the menu state and decides
/// whether the corresponding source action can be admitted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbandonAction {
    ToggleGameplayMap,
    OpenAbandonConfirmation,
    ConfirmAbandon,
    CancelAbandonConfirmation,
    OpenSpellMenu,
    CloseSpellMenu,
    ShowPlayerInformation,
    HidePlayerInformation,
    CloseGameplayMenus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SourceMenuAdmission {
    pub action: AbandonAction,
    pub admitted: bool,
    pub map_open: bool,
    pub spells_open: bool,
    pub player_information_open: bool,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub tick: u64,
}

/// Portable player commands. Game adapters own their admission and rules;
/// no remc2 opcode or bitfield crosses this boundary.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameCommand {
    pub longitudinal: LongitudinalControl,
    pub lateral: LateralControl,
    pub roll_axis: i8,
    pub pitch_axis: i8,
    pub cast_left: bool,
    pub cast_right: bool,
    pub cast_selected: bool,
    /// Rising dead-player recovery request; the simulation admits it only
    /// when the source player has a surviving linked Castle.
    #[serde(skip_serializing_if = "is_false")]
    pub recover_player: bool,
    /// Remove one stage of the player's linked Castle through its ordinary
    /// lifecycle. The game adapter owns admission and the source command.
    #[serde(skip_serializing_if = "is_false")]
    pub remove_castle_stage: bool,
    /// Monotonic elapsed source clock input, recorded with the command so
    /// completion time remains deterministic through replay and save/load.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub elapsed_centiseconds: u32,
    /// Select one acquired spell through its package-neutral semantic ID
    /// before this command's cast flags are consumed.
    pub select_spell: Option<SemanticId>,
    /// Configure an acquired spell's unlocked tier and casting hand. The
    /// adapter validates the choice before executing the ordinary game turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assign_spell: Option<SpellAssignment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_spell: Option<SpellMark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_marked_spell: Option<MarkedSpellCycle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abandon_action: Option<AbandonAction>,
    #[serde(skip_serializing_if = "is_false")]
    pub recenter: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub full_stop: bool,
    /// Keep the current objective visible for this input frame. The game
    /// adapter owns its presentation lifetime and source frame ordering.
    #[serde(skip_serializing_if = "is_false")]
    pub show_objective: bool,
    pub look_yaw: i16,
    pub look_pitch: i16,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeActorId(
    #[serde(
        serialize_with = "serialize_u64_decimal",
        deserialize_with = "deserialize_u64_decimal"
    )]
    pub u64,
);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeParticipantId(#[serde(serialize_with = "serialize_u64_decimal")] pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GameAssetReference {
    pub id: String,
    pub content_address: String,
}

/// One ordered semantic event. Adapter-private numeric kinds, payload slots,
/// and source identities are projected to a semantic ID, package asset
/// identity, and opaque runtime handles. Event-specific semantic payloads may
/// be added as typed domain fields; raw compatibility slots never cross this
/// contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GameEvent {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub sequence: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub state_hash: u64,
    pub event_id: SemanticId,
    pub actor: Option<RuntimeActorId>,
    pub participant: Option<RuntimeParticipantId>,
    pub asset: Option<GameAssetReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventPage {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub requested_cursor: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub next_cursor: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub event_head: u64,
    pub has_more: bool,
    pub events: Vec<GameEvent>,
    /// Optional opaque compatibility receipt emitted by preservation adapters.
    /// It proves exact retained event/audio identities without exposing the
    /// adapter's source selectors as the engine's public vocabulary.
    pub compatibility: Option<CompatibilityReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompatibilityReceipt {
    pub contract: String,
    pub event_count: u32,
    pub event_content_sha256: String,
    pub event_lifecycle_sha256: String,
    pub audio_count: u32,
    pub audio_content_sha256: String,
    pub audio_lifecycle_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAction {
    Prepared,
    Start,
    Restart,
    SuppressExisting,
    SuppressNoVoice,
    SuppressDistanceOrVolume,
    SuppressPendingReplaced,
    UpdateExisting,
    EndLoop,
    SuppressNoExisting,
}

/// A logical audio command joined to its event and package asset identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GameAudioCommand {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub event_sequence: u64,
    #[serde(serialize_with = "serialize_u64_hex")]
    pub state_hash: u64,
    pub asset: GameAssetReference,
    pub relative_position: [i32; 3],
    pub playback_rate_hz: u32,
    pub actor: Option<RuntimeActorId>,
    pub participant: Option<RuntimeParticipantId>,
    pub sound_flags: i16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub rate_delta_percent: i16,
    pub voice_index: u16,
    pub play_type: u8,
    pub loop_count: i8,
    pub action: AudioAction,
    pub pooled_flush_rank: u8,
}

/// Identity guard supplied with every state-changing session command.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SessionGuard {
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub session_id: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub generation: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub expected_tick: u64,
}

/// Canonical domain identity carried by observations and receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionIdentity {
    pub engine_contract: &'static str,
    pub package_graph_hash: String,
    pub definition_hash: String,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub definition_revision: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub session_id: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub generation: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub tick: u64,
}

impl SessionIdentity {
    #[must_use]
    pub const fn guard(&self) -> SessionGuard {
        SessionGuard {
            session_id: self.session_id,
            generation: self.generation,
            expected_tick: self.tick,
        }
    }
}

/// One backend projection bound to the exact identity at which it was read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionObservation<T> {
    pub identity: SessionIdentity,
    pub value: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionGuardErrorCode {
    InvalidSessionId,
    SessionMismatch,
    GenerationMismatch,
    TickMismatch,
    GenerationExhausted,
}

impl SessionGuardErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSessionId => "INVALID_SESSION_ID",
            Self::SessionMismatch => "SESSION_MISMATCH",
            Self::GenerationMismatch => "GENERATION_MISMATCH",
            Self::TickMismatch => "TICK_MISMATCH",
            Self::GenerationExhausted => "GENERATION_EXHAUSTED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionGuardError {
    pub code: SessionGuardErrorCode,
    pub expected: SessionIdentity,
    pub actual: SessionGuard,
}

impl fmt::Display for SessionGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: expected session/generation/tick {}/{}/{}, received {}/{}/{}",
            self.code.as_str(),
            self.expected.session_id,
            self.expected.generation,
            self.expected.tick,
            self.actual.session_id,
            self.actual.generation,
            self.actual.expected_tick
        )
    }
}

impl Error for SessionGuardError {}

#[derive(Debug)]
pub enum GameSessionError<E> {
    Guard(SessionGuardError),
    Backend(E),
}

impl<E: fmt::Display> fmt::Display for GameSessionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Guard(error) => error.fmt(formatter),
            Self::Backend(error) => {
                write!(formatter, "session backend rejected operation: {error}")
            }
        }
    }
}

impl<E: Error + 'static> Error for GameSessionError<E> {}

#[derive(Debug)]
pub enum GameSessionAssetError {
    Guard(SessionGuardError),
    Definition(DefinitionError),
}

impl fmt::Display for GameSessionAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Guard(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
        }
    }
}

impl Error for GameSessionAssetError {}

/// Package-independent gameplay backend contract.
///
/// Implementations own live gameplay state, ordering, RNG, event/audio
/// journals, presentation lifecycle, reset, and replay. A rejected `step` or
/// `reset_for_definition` must leave that backend byte-for-byte unchanged.
pub trait GameSessionBackend {
    type Command;
    type Snapshot;
    type Error;

    fn tick(&self) -> u64;
    fn completion_status(&self) -> GameCompletionStatus;
    fn snapshot(&self) -> Self::Snapshot;
    /// Whether the authoritative game menu excludes idle flight assistance.
    fn flight_assistance_menu_open(&self) -> bool {
        false
    }
    fn events_after(&self, cursor: u64, limit: usize) -> Result<EventPage, Self::Error>;
    fn audio_for_event(&self, event_sequence: u64) -> Result<GameAudioCommand, Self::Error>;
    fn step(&mut self, command: Self::Command) -> Result<(), Self::Error>;
    /// Apply package-declared presentation changes at an accepted pause edge.
    /// This cannot advance gameplay or fail after lifecycle publication.
    fn enter_pause(&mut self) {}
    /// Apply one admitted presentation frame without advancing gameplay. This
    /// work is infallible after the exact session guard has been validated.
    fn presentation_frame(&mut self, _notifications_visible: bool) {}
    /// Service deterministic backend work that the bound package declares
    /// before its gameplay pause gate. Implementations must not advance the
    /// gameplay tick or execute a gameplay command.
    fn paused_frame(&mut self) -> Result<(), Self::Error>;
    fn paused_frame_timed(
        &mut self,
        _elapsed_centiseconds: u32,
        _show_objective: bool,
    ) -> Result<(), Self::Error> {
        self.paused_frame()
    }
    fn reset_for_definition(&mut self, definition: &GameDefinition) -> Result<(), Self::Error>;
}

/// Package-neutral successful gameplay completion observed by the outer
/// runtime. Backends publish this only from their persistent authoritative
/// completion carrier, never from client input or an intermediate objective.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GameCompletion {
    #[serde(serialize_with = "serialize_u64_hex")]
    pub state_hash: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<GameCompletionStatistics>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameCompletionStatistics {
    pub spells_found_percent: i32,
    pub hit_accuracy_percent: i32,
    pub creatures_killed_percent: i32,
    pub mana_collected_percent: i32,
    pub time_taken_seconds: u32,
}

/// Closed neutral projection of a backend's authoritative completion carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GameCompletionStatus {
    Unsupported,
    InProgress,
    Completed { completion: GameCompletion },
    Invalid { message: String },
}

/// Adapter that constructs one concrete backend from a neutral definition.
pub trait GameAdapter {
    type Backend: GameSessionBackend;
    type OpenInput;
    type Error;

    fn open_backend(
        &self,
        definition: &GameDefinition,
        input: Self::OpenInput,
    ) -> Result<Self::Backend, Self::Error>;
}

/// Permanent neutral owner of one definition-bound gameplay backend.
#[derive(Debug)]
pub struct GameSession<B> {
    definition: Arc<GameDefinition>,
    session_id: u64,
    generation: u64,
    backend: B,
}

impl<B: GameSessionBackend> GameSession<B> {
    /// Open a definition-bound session through a concrete adapter.
    ///
    /// # Errors
    ///
    /// Returns `INVALID_SESSION_ID` before adapter execution for identifier
    /// zero, or the adapter's typed open error.
    pub fn open<A>(
        adapter: &A,
        definition: Arc<GameDefinition>,
        session_id: u64,
        input: A::OpenInput,
    ) -> Result<Self, OpenSessionError<A::Error>>
    where
        A: GameAdapter<Backend = B>,
    {
        Self::open_with_generation(adapter, definition, session_id, NonZeroU64::MIN, input)
    }

    /// Open a definition-bound session at a runtime-owned nonzero lifecycle
    /// generation. The outer runtime uses this when moving between levels so
    /// a stale guard from an earlier level cannot collide with a fresh
    /// session on the same handle.
    #[doc(hidden)]
    pub fn open_with_generation<A>(
        adapter: &A,
        definition: Arc<GameDefinition>,
        session_id: u64,
        initial_generation: NonZeroU64,
        input: A::OpenInput,
    ) -> Result<Self, OpenSessionError<A::Error>>
    where
        A: GameAdapter<Backend = B>,
    {
        if session_id == 0 {
            return Err(OpenSessionError::InvalidSessionId);
        }
        let backend = adapter
            .open_backend(&definition, input)
            .map_err(OpenSessionError::Adapter)?;
        Ok(Self {
            definition,
            session_id,
            generation: initial_generation.get(),
            backend,
        })
    }

    #[must_use]
    pub fn definition(&self) -> &Arc<GameDefinition> {
        &self.definition
    }

    /// Borrow the bound backend for a transport-owned compatibility
    /// projection. Gameplay mutation must still cross [`Self::step`],
    /// [`Self::reset`], or [`Self::reopen`].
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Mutably borrow the backend for an explicitly classified transport
    /// sidecar such as presentation input or audio acknowledgement. This is
    /// not a second gameplay authority and must not be exposed as a general
    /// client mutation surface.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    #[must_use]
    pub fn identity(&self) -> SessionIdentity {
        SessionIdentity {
            engine_contract: GAME_SESSION_CONTRACT,
            package_graph_hash: self.definition.package_graph_hash().to_owned(),
            definition_hash: self.definition.definition_hash().to_owned(),
            definition_revision: self.definition.revision(),
            session_id: self.session_id,
            generation: self.generation,
            tick: self.backend.tick(),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionObservation<B::Snapshot> {
        SessionObservation {
            identity: self.identity(),
            value: self.backend.snapshot(),
        }
    }

    /// Observe authoritative gameplay completion without advancing the
    /// session or consuming its event journal.
    pub fn completion_status(
        &self,
        guard: SessionGuard,
    ) -> Result<SessionObservation<GameCompletionStatus>, SessionGuardError> {
        self.validate_guard(guard)?;
        Ok(SessionObservation {
            identity: self.identity(),
            value: self.backend.completion_status(),
        })
    }

    /// Read an ordered bounded event page without advancing any session state.
    pub fn events_after(
        &self,
        guard: SessionGuard,
        cursor: u64,
        limit: usize,
    ) -> Result<SessionObservation<EventPage>, GameSessionError<B::Error>> {
        self.validate_guard(guard)
            .map_err(GameSessionError::Guard)?;
        let page = self
            .backend
            .events_after(cursor, limit)
            .map_err(GameSessionError::Backend)?;
        Ok(SessionObservation {
            identity: self.identity(),
            value: page,
        })
    }

    /// Read one logical audio sidecar without consuming or releasing it.
    pub fn audio_for_event(
        &self,
        guard: SessionGuard,
        event_sequence: u64,
    ) -> Result<SessionObservation<GameAudioCommand>, GameSessionError<B::Error>> {
        self.validate_guard(guard)
            .map_err(GameSessionError::Guard)?;
        let command = self
            .backend
            .audio_for_event(event_sequence)
            .map_err(GameSessionError::Backend)?;
        Ok(SessionObservation {
            identity: self.identity(),
            value: command,
        })
    }

    /// Read one typed content-addressed presentation asset without advancing
    /// gameplay or consuming presentation lifecycle state.
    pub fn presentation_asset(
        &self,
        guard: SessionGuard,
        id: &SemanticId,
    ) -> Result<SessionObservation<BoundPresentationAsset>, GameSessionAssetError> {
        self.validate_guard(guard)
            .map_err(GameSessionAssetError::Guard)?;
        let asset = self
            .definition
            .presentation_asset(id)
            .map_err(GameSessionAssetError::Definition)?;
        Ok(SessionObservation {
            identity: self.identity(),
            value: asset,
        })
    }

    /// Apply one typed command after exact session/generation/tick validation.
    ///
    /// # Errors
    ///
    /// A stale guard fails before the backend is called. Backend errors are
    /// returned without changing the definition binding or generation.
    pub fn step(
        &mut self,
        guard: SessionGuard,
        command: B::Command,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.advance(guard, command)?;
        Ok(self.snapshot())
    }

    /// Apply the same guarded step when a transport reads its observation
    /// separately. This does not construct or discard a backend snapshot.
    ///
    /// # Errors
    ///
    /// Returns the same guard and backend errors as [`Self::step`].
    #[doc(hidden)]
    pub fn advance(
        &mut self,
        guard: SessionGuard,
        command: B::Command,
    ) -> Result<(), GameSessionError<B::Error>> {
        self.validate_guard(guard)
            .map_err(GameSessionError::Guard)?;
        self.backend
            .step(command)
            .map_err(GameSessionError::Backend)
    }

    /// Service one package-declared paused frame after exact nested guard
    /// validation. The session generation and gameplay tick remain fixed;
    /// only deterministic pre-gate backend carriers may change.
    pub fn paused_frame(
        &mut self,
        guard: SessionGuard,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.paused_frame_timed(guard, 0, false)
    }

    pub fn paused_frame_timed(
        &mut self,
        guard: SessionGuard,
        elapsed_centiseconds: u32,
        show_objective: bool,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.validate_guard(guard)
            .map_err(GameSessionError::Guard)?;
        self.backend
            .paused_frame_timed(elapsed_centiseconds, show_objective)
            .map_err(GameSessionError::Backend)?;
        Ok(self.snapshot())
    }

    /// Advance package-owned presentation state through its own clock. An
    /// observation or repeated raster copy must never perform this mutation.
    pub fn presentation_frame(
        &mut self,
        guard: SessionGuard,
        notifications_visible: bool,
    ) -> Result<(), SessionGuardError> {
        self.validate_guard(guard)?;
        self.backend.presentation_frame(notifications_visible);
        Ok(())
    }

    /// Reset against the already-bound immutable definition.
    pub fn reset(
        &mut self,
        guard: SessionGuard,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.reset_or_reopen(guard, Arc::clone(&self.definition))
    }

    /// Explicitly reset and bind this session, and only this session, to a new
    /// immutable definition.
    pub fn reopen(
        &mut self,
        guard: SessionGuard,
        definition: Arc<GameDefinition>,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.reset_or_reopen(guard, definition)
    }

    /// Advance only the session lifecycle generation for an outer runtime
    /// boundary such as pause or resume.
    ///
    /// The definition, backend, tick, journals, RNG, and snapshot payload are
    /// unchanged. This method is intentionally not a gameplay command and is
    /// not exposed by the neutral session ABI; it lets the sole outer runtime
    /// invalidate commands captured before a lifecycle boundary.
    ///
    /// # Errors
    ///
    /// Rejects a stale guard or generation overflow without changing the
    /// session.
    #[doc(hidden)]
    pub fn advance_lifecycle_generation(
        &mut self,
        guard: SessionGuard,
    ) -> Result<SessionObservation<B::Snapshot>, SessionGuardError> {
        self.validate_guard(guard)?;
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| SessionGuardError {
                code: SessionGuardErrorCode::GenerationExhausted,
                expected: self.identity(),
                actual: guard,
            })?;
        self.generation = next_generation;
        Ok(self.snapshot())
    }

    fn reset_or_reopen(
        &mut self,
        guard: SessionGuard,
        definition: Arc<GameDefinition>,
    ) -> Result<SessionObservation<B::Snapshot>, GameSessionError<B::Error>> {
        self.validate_guard(guard)
            .map_err(GameSessionError::Guard)?;
        let next_generation = self.generation.checked_add(1).ok_or_else(|| {
            GameSessionError::Guard(SessionGuardError {
                code: SessionGuardErrorCode::GenerationExhausted,
                expected: self.identity(),
                actual: guard,
            })
        })?;
        self.backend
            .reset_for_definition(&definition)
            .map_err(GameSessionError::Backend)?;
        self.definition = definition;
        self.generation = next_generation;
        Ok(self.snapshot())
    }

    fn validate_guard(&self, guard: SessionGuard) -> Result<(), SessionGuardError> {
        let expected = self.identity();
        let code = if guard.session_id == 0 {
            Some(SessionGuardErrorCode::InvalidSessionId)
        } else if guard.session_id != expected.session_id {
            Some(SessionGuardErrorCode::SessionMismatch)
        } else if guard.generation != expected.generation {
            Some(SessionGuardErrorCode::GenerationMismatch)
        } else if guard.expected_tick != expected.tick {
            Some(SessionGuardErrorCode::TickMismatch)
        } else {
            None
        };
        match code {
            Some(code) => Err(SessionGuardError {
                code,
                expected,
                actual: guard,
            }),
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub enum OpenSessionError<E> {
    InvalidSessionId,
    Adapter(E),
}

impl<E: fmt::Display> fmt::Display for OpenSessionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSessionId => formatter.write_str("INVALID_SESSION_ID: session ID is zero"),
            Self::Adapter(error) => write!(formatter, "session adapter rejected open: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for OpenSessionError<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_definition::GamePackage;
    use sim_definition::{
        ChangeSetBuilder, IntegerParameter, PublicationFault, RequestedTiming, WorkspaceStore,
        default_catalog, engine_capabilities,
    };
    use std::{cell::Cell, convert::Infallible, path::PathBuf};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ProbeSnapshot {
        tick: u64,
        definition_hash: String,
        command_total: i64,
    }

    #[derive(Debug)]
    struct ProbeBackend {
        tick: u64,
        definition_hash: String,
        command_total: i64,
        reject_next_reset: bool,
        reject_steps: bool,
        step_calls: usize,
        snapshot_reads: Cell<usize>,
    }

    impl GameSessionBackend for ProbeBackend {
        type Command = i64;
        type Snapshot = ProbeSnapshot;
        type Error = &'static str;

        fn tick(&self) -> u64 {
            self.tick
        }

        fn completion_status(&self) -> GameCompletionStatus {
            GameCompletionStatus::InProgress
        }

        fn snapshot(&self) -> Self::Snapshot {
            self.snapshot_reads.set(self.snapshot_reads.get() + 1);
            ProbeSnapshot {
                tick: self.tick,
                definition_hash: self.definition_hash.clone(),
                command_total: self.command_total,
            }
        }

        fn events_after(&self, cursor: u64, _limit: usize) -> Result<EventPage, Self::Error> {
            Ok(EventPage {
                requested_cursor: cursor,
                next_cursor: cursor,
                event_head: cursor,
                has_more: false,
                events: Vec::new(),
                compatibility: None,
            })
        }

        fn audio_for_event(&self, _event_sequence: u64) -> Result<GameAudioCommand, Self::Error> {
            Err("probe has no audio")
        }

        fn step(&mut self, command: Self::Command) -> Result<(), Self::Error> {
            self.step_calls += 1;
            if self.reject_steps {
                return Err("injected step rejection");
            }
            self.command_total += command;
            self.tick += 1;
            Ok(())
        }

        fn paused_frame(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn reset_for_definition(&mut self, definition: &GameDefinition) -> Result<(), Self::Error> {
            if self.reject_next_reset {
                self.reject_next_reset = false;
                return Err("injected reset rejection");
            }
            self.tick = 0;
            self.command_total = 0;
            self.definition_hash = definition.definition_hash().to_owned();
            Ok(())
        }
    }

    struct ProbeAdapter;

    impl GameAdapter for ProbeAdapter {
        type Backend = ProbeBackend;
        type OpenInput = ();
        type Error = Infallible;

        fn open_backend(
            &self,
            definition: &GameDefinition,
            (): Self::OpenInput,
        ) -> Result<Self::Backend, Self::Error> {
            Ok(ProbeBackend {
                tick: 0,
                definition_hash: definition.definition_hash().to_owned(),
                command_total: 0,
                reject_next_reset: false,
                reject_steps: false,
                step_calls: 0,
                snapshot_reads: Cell::new(0),
            })
        }
    }

    fn definition() -> GameDefinition {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../sim-definition/tests/fixtures/custom-synthetic-v1");
        GameDefinition::from_package(
            GamePackage::load(root).unwrap(),
            &engine_capabilities().unwrap(),
            &default_catalog().unwrap(),
        )
        .unwrap()
    }

    fn guard(session: &GameSession<ProbeBackend>) -> SessionGuard {
        let identity = session.identity();
        SessionGuard {
            session_id: identity.session_id,
            generation: identity.generation,
            expected_tick: identity.tick,
        }
    }

    #[test]
    fn advance_matches_step_without_constructing_an_observation() {
        let definition = Arc::new(definition());
        let mut observed =
            GameSession::open(&ProbeAdapter, Arc::clone(&definition), 7, ()).unwrap();
        let mut advanced = GameSession::open(&ProbeAdapter, definition, 7, ()).unwrap();
        for command in [4, -2, 9] {
            let reads_before = advanced.backend.snapshot_reads.get();
            let stepped = observed.step(guard(&observed), command).unwrap();
            advanced.advance(guard(&advanced), command).unwrap();
            assert_eq!(advanced.backend.snapshot_reads.get(), reads_before);
            assert_eq!(
                observed.backend.snapshot_reads.get(),
                observed.backend.step_calls
            );
            assert_eq!(advanced.backend.step_calls, observed.backend.step_calls);
            assert_eq!(advanced.snapshot(), stepped);
        }
    }

    #[test]
    fn advance_and_step_reject_before_observation_and_preserve_state() {
        for observe in [false, true] {
            let mut session =
                GameSession::open(&ProbeAdapter, Arc::new(definition()), 7, ()).unwrap();
            let before = session.snapshot();
            let valid = guard(&session);
            for stale in [
                SessionGuard {
                    session_id: 8,
                    ..valid
                },
                SessionGuard {
                    generation: valid.generation + 1,
                    ..valid
                },
                SessionGuard {
                    expected_tick: valid.expected_tick + 1,
                    ..valid
                },
            ] {
                let result = if observe {
                    session.step(stale, 4).map(|_| ())
                } else {
                    session.advance(stale, 4)
                };
                assert!(matches!(result, Err(GameSessionError::Guard(_))));
                assert_eq!(session.backend.step_calls, 0);
                assert_eq!(session.backend.snapshot_reads.get(), 1);
            }
            session.backend.reject_steps = true;
            let result = if observe {
                session.step(valid, 4).map(|_| ())
            } else {
                session.advance(valid, 4)
            };
            assert!(matches!(
                result,
                Err(GameSessionError::Backend("injected step rejection"))
            ));
            assert_eq!(session.backend.step_calls, 1);
            assert_eq!(session.backend.snapshot_reads.get(), 1);
            assert_eq!(session.snapshot(), before);
        }
    }

    #[test]
    fn publishing_a_definition_does_not_rebind_existing_sessions() {
        let original = definition();
        let root =
            std::env::temp_dir().join(format!("carpet-flight-f4-binding-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = WorkspaceStore::create(&root, "binding-workspace", original.clone()).unwrap();
        let mut first =
            GameSession::open(&ProbeAdapter, Arc::new(original.clone()), 1, ()).unwrap();
        let second = GameSession::open(&ProbeAdapter, Arc::new(original.clone()), 2, ()).unwrap();
        let catalog = default_catalog().unwrap();
        let change_set = ChangeSetBuilder::new(
            "binding-workspace",
            &original,
            "publish-speed",
            RequestedTiming::SessionReset,
        )
        .set_integer(IntegerParameter::PlayerMaxForwardSpeed, 96)
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change_set.clone()).unwrap();
        assert!(validation.accepted);
        store
            .apply(
                &catalog,
                change_set,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let published = Arc::new(store.active_definition().unwrap());

        assert_eq!(first.identity().definition_hash, original.definition_hash());
        assert_eq!(
            second.identity().definition_hash,
            original.definition_hash()
        );
        assert_eq!(first.identity().generation, 1);
        assert_eq!(second.identity().generation, 1);
        let reopened = first.reopen(guard(&first), Arc::clone(&published)).unwrap();
        assert_eq!(
            reopened.identity.definition_hash,
            published.definition_hash()
        );
        assert_eq!(reopened.identity.definition_revision, 2);
        assert_eq!(reopened.identity.generation, 2);
        assert_eq!(
            second.identity().definition_hash,
            original.definition_hash()
        );
        assert_eq!(second.identity().generation, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_guards_and_backend_rejection_preserve_binding_and_observation() {
        let original = Arc::new(definition());
        let mut session = GameSession::open(&ProbeAdapter, Arc::clone(&original), 7, ()).unwrap();
        let before = session.snapshot();
        let stale = SessionGuard {
            expected_tick: 1,
            ..guard(&session)
        };
        let error = session.step(stale, 4).unwrap_err();
        assert!(matches!(
            error,
            GameSessionError::Guard(SessionGuardError {
                code: SessionGuardErrorCode::TickMismatch,
                ..
            })
        ));
        assert_eq!(session.snapshot(), before);

        session.backend.reject_next_reset = true;
        let before_rejected_reset = session.snapshot();
        let error = session.reset(guard(&session)).unwrap_err();
        assert!(matches!(error, GameSessionError::Backend(_)));
        assert_eq!(session.identity(), before_rejected_reset.identity);
        assert_eq!(session.backend.snapshot(), before_rejected_reset.value);

        let stepped = session.step(guard(&session), 4).unwrap();
        assert_eq!(stepped.identity.tick, 1);
        assert_eq!(stepped.value.command_total, 4);
        let reset = session.reset(guard(&session)).unwrap();
        assert_eq!(reset.identity.generation, 2);
        assert_eq!(reset.identity.tick, 0);
        assert_eq!(reset.value.command_total, 0);
    }
}
