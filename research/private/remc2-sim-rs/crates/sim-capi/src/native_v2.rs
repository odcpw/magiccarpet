//! Package-backed, handle-based ABI v2 for native presentation clients.
//!
//! All paths and data are supplied explicitly by the caller. This module does
//! not use the embedded owner regression bytes retained by the legacy v1 ABI.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    fs,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    slice, str,
    sync::{Arc, Mutex, OnceLock},
};

use sha2::{Digest, Sha256};
use sim_core::{
    FlightContext, FlightEvent, FlightState, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING, PlayerCommand,
    Position, PositionalSoundCommand, StepEvidence, conformance_hash,
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GoatPlayerTarget, GoatState, GoatStepEvent,
        LEVEL_ONE_GOAT_INDICES, LiveWorldState,
    },
    native_world::{AssetRef, EntityAuthority, EntitySnapshot, NativeWorld, TerrainPlanes},
    positional_sound_command, positional_sound_should_replace,
    render_camera::{TerrainSlopeCamera, TerrainSlopeCameraSample},
    source_projection_hash, state_hash, step_outer_turn_with_context,
    terrain_frame::{
        LEVEL1_SKY_BYTES, LEVEL1_TERRAIN_ATLAS_BYTES, LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS,
        LEVEL1_VIEW_WIDTH, Level1TerrainFrame, Level1TerrainFrameCamera,
        Level1TerrainFrameResources, render_level1_terrain_frame,
    },
    terrain_grid::{OUTDOOR_GRID_VERTICES, OutdoorGridInput, outdoor_grid},
    terrain_presentation::outdoor_terrain_pnt5,
};

pub const MC_NATIVE_ABI_VERSION: u32 = 2;

pub const MC_STATUS_OK: i32 = 0;
pub const MC_STATUS_NULL_POINTER: i32 = 1;
pub const MC_STATUS_INVALID_UTF8: i32 = 2;
pub const MC_STATUS_INVALID_ARGUMENT: i32 = 3;
pub const MC_STATUS_UNSUPPORTED_VERSION: i32 = 4;
pub const MC_STATUS_STRUCT_TOO_SMALL: i32 = 5;
pub const MC_STATUS_RESERVED_NONZERO: i32 = 6;
pub const MC_STATUS_IO_ERROR: i32 = 7;
pub const MC_STATUS_VALIDATION_FAILED: i32 = 8;
pub const MC_STATUS_UNKNOWN_HANDLE: i32 = 9;
pub const MC_STATUS_BOUNDS: i32 = 10;
pub const MC_STATUS_BUFFER_TOO_SMALL: i32 = 11;
pub const MC_STATUS_GENERATION_MISMATCH: i32 = 12;
pub const MC_STATUS_CURSOR_EXPIRED: i32 = 13;
pub const MC_STATUS_INTERNAL_PANIC: i32 = 14;
pub const MC_STATUS_LOCK_FAILED: i32 = 15;

pub const MC_PROFILE_OUTDOOR: u8 = 0;
pub const MC_PROFILE_CAVE: u8 = 1;

pub const MC_AUTHORITY_INACTIVE: u8 = 0;
pub const MC_AUTHORITY_FROZEN_OBSERVED: u8 = 1;
pub const MC_AUTHORITY_PLAYER_FLIGHT_LIVE: u8 = 2;
pub const MC_AUTHORITY_SPELL_ACTIVATION_LIVE: u8 = 3;
pub const MC_AUTHORITY_GOAT_LIVE: u8 = 4;

pub const MC_TERRAIN_TYPES_U8: u32 = 0;
pub const MC_TERRAIN_HEIGHT_U8: u32 = 1;
pub const MC_TERRAIN_SHADE_U8: u32 = 2;
pub const MC_TERRAIN_ANGLE_U8: u32 = 3;
pub const MC_TERRAIN_SECONDARY_U8: u32 = 4;
pub const MC_TERRAIN_ENTITY_INDEX_I16: u32 = 5;

pub const MC_WORLD_STRING_SCHEMA: u32 = 0;
pub const MC_WORLD_STRING_PACKAGE_SHA256: u32 = 1;
pub const MC_WORLD_STRING_CLOSURE_SHA256: u32 = 2;
pub const MC_WORLD_STRING_STATE_HASH_SCHEMA: u32 = 3;
pub const MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA: u32 = 4;
pub const MC_WORLD_STRING_CONTEXT_SEAM: u32 = 5;

pub const MC_ASSET_STRING_ID: u32 = 0;
pub const MC_ASSET_STRING_CONTENT_ADDRESS: u32 = 1;
pub const MC_ASSET_STRING_CATEGORY: u32 = 2;
pub const MC_ASSET_STRING_HOOK: u32 = 3;
pub const MC_ASSET_STRING_STAGE: u32 = 4;
pub const MC_ASSET_STRING_OWNER_ID: u32 = 5;

pub const MC_EVENT_MOVEMENT_RESOLUTION: u32 = 0;
pub const MC_EVENT_PREPARE_SOUND: u32 = 1;
pub const MC_EVENT_END_LOOP: u32 = 2;
pub const MC_EVENT_UPDATE_MUSIC: u32 = 3;
pub const MC_EVENT_REQUEST_SPELL_ACTIVATION: u32 = 4;
pub const MC_EVENT_SHOW_HELP_HINT: u32 = 5;
pub const MC_EVENT_START_MUSIC: u32 = 6;
pub const MC_EVENT_GOAT_SOUND_REQUEST: u32 = 7;
pub const MC_EVENT_GOAT_MANA_CONVERTED: u32 = 8;
pub const MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST: u32 = 9;
pub const MC_EVENT_GOAT_REMOVAL_QUEUED: u32 = 10;
pub const MC_EVENT_GOAT_RECLAIMED: u32 = 11;
pub const MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY: u32 = 12;

pub const MC_AUDIO_ACTION_START: u8 = 1;
pub const MC_AUDIO_ACTION_RESTART: u8 = 2;
pub const MC_AUDIO_ACTION_SUPPRESS_EXISTING: u8 = 3;
pub const MC_AUDIO_ACTION_SUPPRESS_NO_VOICE: u8 = 4;
pub const MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME: u8 = 5;
pub const MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED: u8 = 6;
pub const MC_AUDIO_ACTION_UPDATE_EXISTING: u8 = 7;
pub const MC_AUDIO_ACTION_END_LOOP: u8 = 8;
pub const MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING: u8 = 9;

pub const MC_ASSET_REF_NONE: u32 = u32::MAX;
pub const MC_PARAMETER_RECORD_NONE: u16 = u16::MAX;
pub const MC_REQUESTED_SPELL_NONE: i16 = -1;
pub const MC_ENTITY_INDEX_NONE: u16 = u16::MAX;
pub const MC_AUDIO_VOICE_NONE: u16 = u16::MAX;
pub const MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS: u8 = 0x01;
pub const MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128: u8 = 0x02;
pub const MC_RENDER_CAMERA_TERRAIN_SLOPE_XY: u8 = 0x01;
pub const MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN: u32 = 0x0001;
pub const MC_TERRAIN_FRAME_SKY_SEEDED: u32 = 0x0002;
pub const MC_TERRAIN_FRAME_HAS_COVERAGE: u32 = 0x0004;
pub const MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED: u32 = 0x0008;

const EVENT_CAPACITY: usize = 4_096;
const AUDIO_VOICE_COUNT: usize = 10;
const TERRAIN_CELLS: usize = 256 * 256;
const POSITIONAL_QUALIFICATION_VECTOR_COUNT: u32 = 23;
const TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT: u32 = 13;
const TERRAIN_ATLAS_SHA256: &str =
    "d4d69babd648008aac40413e6089e27bbe32e6b2a5be4311eb10b1aaa33fc5de";
const TABLESN_SHA256: &str = "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495";
const SKYN_SHA256: &str = "944112ef6eb54e042889c9cc841ed1d7bdb129c38c1c61ce4c8b8e81589f466b";

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McWorldInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: u8,
    pub profile: u8,
    pub reserved_0: u16,
    pub entity_count: u32,
    pub active_entity_count: u32,
    pub player_count: u32,
    pub asset_ref_count: u32,
    pub terrain_width: u16,
    pub terrain_height: u16,
    pub reset_state_hash: u64,
    pub reset_conformance_hash: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub opcode: i8,
    pub argument_1: i8,
    pub argument_2: i8,
    pub roll: i8,
    pub pitch: i8,
    pub flags: i8,
    pub requested_spell: i16,
    pub reserved: [u32; 4],
}

/// Additive tail for the two 16-bit view fields in the original ten-byte
/// player input record. `McCommandV2` remains byte-for-byte stable.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McCameraInputV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub look_yaw: u16,
    pub look_pitch: u16,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McSessionSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub event_head: u64,
    pub state_hash: u64,
    pub conformance_hash: u64,
    pub entity_index: u16,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub display_pitch: i16,
    pub effective_pitch: i16,
    pub roll_accumulator: i16,
    pub target_speed: i16,
    pub actual_speed: i16,
    pub strafe_speed: i16,
    pub phase: u8,
    pub last_resolution: u8,
    pub profile: u8,
    pub local_player_slot: u8,
    pub left_spell_slot: u8,
    pub right_spell_slot: u8,
    pub selected_spell_ui_index: u8,
    /// Validated reset observation. ABI v2 does not advance this presentation
    /// countdown; `objectives_authority` is therefore frozen-observed.
    pub objective_textbox_ticks: u8,
    pub objective_count: u8,
    pub objectives_authority: u8,
    pub reserved_1: [u8; 2],
    pub left_spell_holder: u16,
    pub right_spell_holder: u16,
    pub selected_spell_holder: u16,
    pub reserved_0: u16,
    pub caster_mana: i32,
    pub maptype_music: i32,
    pub fire_sequence: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McObjectiveSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub slot: u8,
    pub is_level_end: u8,
    pub text_selector: u8,
    pub done: u8,
    pub authority: u8,
    pub stage_selectors: [u8; 8],
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEntitySnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub entity_index: u16,
    pub parameter_record_index: u16,
    /// The player-flight row merges current session pose/flight fields into
    /// the validated reset observation. Spell holder timer/action-marker
    /// mutations are authoritative in the core but are not fields in v2.
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub has_parameter_record: u8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub actual_speed: i16,
    pub reserved: [u32; 4],
}

/// Additive Rust-owned Level 1 Goat row. The existing 52-byte entity snapshot
/// remains stable; presentation clients should fetch this family in one bulk
/// call when they need gameplay and render fields beyond pose.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatSnapshotV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub entity_index: u16,
    pub target_entity_index: u16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub authority: u8,
    pub active: u8,
    pub class_id: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub proximity_countdown: i8,
    pub proximity_subcounter: i8,
    /// Qualified reset observation only. Rust does not advance the unresolved
    /// selector-global animation clock in this milestone.
    pub animation_frame: i8,
    pub reserved_0: u8,
    pub x: u16,
    pub y: u16,
    pub z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub rng: u16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub pending_damage: i32,
    pub control_flags: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatBatchV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub live_world_hash: u64,
    pub start_ordinal: u32,
    pub written: u32,
    pub total_count: u32,
    pub has_more: u8,
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

/// Private qualification-only pending-damage injection. This writes the
/// source boundary consumed on the following native step; Unity never owns or
/// directly mutates Goat state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McGoatDamageCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub entity_index: u16,
    pub reserved_0: u16,
    pub damage: i32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McAssetRefInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub asset_index: u32,
    pub trace_ordinal: u16,
    pub owner_asset_count: u16,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventQueryV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub cursor: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventBatchV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub next_cursor: u64,
    pub event_head: u64,
    pub written: u32,
    pub has_more: u8,
    pub reserved_0: [u8; 3],
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McEventV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub sequence: u64,
    pub state_hash: u64,
    pub kind: u32,
    pub code: u32,
    pub entity_index: u16,
    pub player_index: i16,
    pub value_0: i32,
    pub value_1: i32,
    pub asset_ref_index: u32,
    pub reserved: [u32; 4],
}

/// Additive original-stereo sidecar. `McEventV2` remains byte-for-byte stable;
/// clients request this record only for a positional `PrepareSound` sequence.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McAudioCommandV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub event_sequence: u64,
    pub state_hash: u64,
    pub asset_ref_index: u32,
    pub relative_x: i32,
    pub relative_y: i32,
    pub relative_z: i32,
    pub playback_rate_hz: u32,
    pub sound_id: u16,
    pub entity_index: u16,
    pub player_index: i16,
    pub sound_flags: i16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub rate_delta_percent: i16,
    pub voice_index: u16,
    pub play_type: u8,
    pub loop_count: i8,
    pub action: u8,
    pub reserved_0: u8,
    pub reserved: [u32; 4],
}

/// Additive source-presentation state. Existing snapshots and hashes remain
/// byte-for-byte unchanged.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McPresentationStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_turn: i32,
    pub camera_x: u16,
    pub camera_y: u16,
    pub camera_z: i16,
    pub camera_yaw: i16,
    pub camera_pitch: i16,
    pub camera_roll: i16,
    pub camera_fov: i16,
    pub camera_authority: u8,
    pub camera_flags: u8,
    pub reserved_0: u32,
    pub reserved: [u32; 4],
}

/// Additive final outdoor-render camera transform. The raw stored history
/// remains available through `McPresentationStateV2`; this sidecar adds the
/// stateful X/Y slope target and easing used by `DrawWorld_411A0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McRenderCameraStateV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub source_turn: i32,
    pub history_x: u16,
    pub history_y: u16,
    pub eye_z: i16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub slope_target_x: i16,
    pub slope_target_y: i16,
    pub render_offset_x: i16,
    pub render_offset_y: i16,
    pub authority: u8,
    pub flags: u8,
    pub reserved_0: u16,
    pub reserved: [u32; 4],
}

/// One hard pnt5 qualification vector produced by the Rust source model.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainPnt5VectorV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub turn: i32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub distance_squared: u32,
    pub shade: u8,
    pub angle: u8,
    pub visible: u8,
    pub tablesn_row: u8,
    pub wave: i32,
    pub land_displacement: i32,
    pub pnt5: i32,
    pub view_y: i32,
    pub lighting_wave: i32,
    pub reserved: [u32; 2],
}

/// One row-major vertex of the source 40 by 21 outdoor terrain lattice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainGridVertexV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub index: u16,
    pub row: u8,
    pub column: u8,
    pub tile_x: u8,
    pub tile_y: u8,
    pub shade: u8,
    pub angle: u8,
    pub view_x: i32,
    pub view_depth: i32,
    pub distance_squared: u32,
    pub pnt5: i32,
    pub raw_wave: i32,
    pub surface_displacement: i32,
    pub lighting_wave: i32,
    pub feature_flags: u16,
    pub tablesn_row: u8,
    pub pnt5_written: u8,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainFrameQueryV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    /// Presentation/render-camera tick, not the post-simulation snapshot tick.
    pub tick: u64,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct McTerrainFrameInfoV2 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub generation: u64,
    pub tick: u64,
    pub width: u16,
    pub height: u16,
    pub stride: u16,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub reserved_0: u16,
    pub index_bytes: u32,
    pub coverage_bytes: u32,
    pub flags: u32,
    pub reflected_triangles: u32,
    pub primary_mode5_triangles: u32,
    pub primary_mode26_triangles: u32,
    pub index_hash: u64,
    pub coverage_hash: u64,
    pub terrain_pixel_writes: u64,
    pub reserved: [u32; 2],
}

#[derive(Debug)]
struct AbiError {
    status: i32,
    message: String,
}

impl AbiError {
    fn new(status: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

type AbiResult<T> = Result<T, AbiError>;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CameraHistory {
    tick: u64,
    source_turn: i32,
    x: u16,
    y: u16,
    z: i16,
    yaw: i16,
    pitch: i16,
    roll: i16,
    fov: i16,
}

#[derive(Debug)]
struct TerrainFrameAssets {
    terrain_atlas: Box<[u8]>,
    tablesn: Box<[u8]>,
    sky: Box<[u8]>,
}

impl CameraHistory {
    fn capture(
        state: &FlightState,
        source_turn: i32,
        fov: i16,
        look_yaw: i16,
        look_pitch: i16,
    ) -> Self {
        let boost = i32::from(state.recoil_speed);
        let turn_phase = source_turn & 1;
        let pitch = i32::from(state.pitch_accumulator) / 2 + turn_phase * (boost / 16) - boost / 8
            + i32::from(look_pitch);
        Self {
            tick: state.tick,
            source_turn,
            x: state.position.x,
            y: state.position.y,
            z: state.position.z.wrapping_add(128),
            // PlayerEvents stores the raw wrapping i16 sum. DrawWorld masks
            // this value to 11 bits only when building the render transform.
            yaw: state.yaw.wrapping_add(look_yaw),
            pitch: pitch as i16,
            roll: state.roll_accumulator,
            fov,
        }
    }
}

#[derive(Debug)]
struct SessionRecord {
    world: Arc<NativeWorld>,
    state: FlightState,
    context: FlightContext,
    live_world: LiveWorldState,
    generation: u64,
    event_head: u64,
    events: VecDeque<McEventV2>,
    audio_commands: BTreeMap<u64, McAudioCommandV2>,
    audio_voices: [Option<AudioVoice>; AUDIO_VOICE_COUNT],
    music_start_armed: bool,
    fire_sequence: u32,
    last_resolution: u8,
    source_turn: i32,
    camera_fov: i16,
    camera_look_yaw: i16,
    camera_look_pitch: i16,
    camera_history: CameraHistory,
    camera_slope: TerrainSlopeCamera,
    camera_slope_sample: TerrainSlopeCameraSample,
    terrain_frame_assets: Option<Arc<TerrainFrameAssets>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AudioVoice {
    sound_id: u16,
    sound_flags: i16,
    event_sequence: u64,
}

impl SessionRecord {
    fn new(world: NativeWorld) -> AbiResult<Self> {
        let terrain_frame_assets = load_optional_terrain_frame_assets()?.map(Arc::new);
        let world = Arc::new(world);
        let player = &world.players()[usize::from(world.runtime().local_player_slot)];
        let record = player.record_bytes();
        let source_turn = player.turn;
        let camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        let camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        let state = *world.reset_state();
        let camera_fov = 128;
        let camera_history = CameraHistory::capture(
            &state,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
        );
        let mut camera_slope = TerrainSlopeCamera::default();
        let camera_slope_sample = camera_slope.update(
            world.terrain().ground_heights(),
            camera_history.x,
            camera_history.y,
        );
        Ok(Self {
            state,
            context: world.session_context(),
            live_world: world.reset_live_world().clone(),
            world,
            generation: 1,
            event_head: 0,
            events: VecDeque::new(),
            audio_commands: BTreeMap::new(),
            audio_voices: [None; AUDIO_VOICE_COUNT],
            music_start_armed: true,
            fire_sequence: 0,
            last_resolution: 0,
            source_turn,
            camera_fov,
            camera_look_yaw,
            camera_look_pitch,
            camera_history,
            camera_slope,
            camera_slope_sample,
            terrain_frame_assets,
        })
    }

    fn reset(&mut self) {
        self.state = *self.world.reset_state();
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world().clone();
        self.generation = next_nonzero(self.generation);
        self.event_head = 0;
        self.events.clear();
        self.audio_commands.clear();
        self.audio_voices = [None; AUDIO_VOICE_COUNT];
        self.music_start_armed = true;
        self.fire_sequence = 0;
        self.last_resolution = 0;
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let record = player.record_bytes();
        self.source_turn = player.turn;
        self.camera_fov = 128;
        self.camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        self.camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope = TerrainSlopeCamera::default();
        self.camera_slope_sample = self.camera_slope.update(
            self.world.terrain().ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
    }

    fn presentation_state(&self) -> McPresentationStateV2 {
        let camera = self.camera_history;
        McPresentationStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            camera_x: camera.x,
            camera_y: camera.y,
            camera_z: camera.z,
            camera_yaw: camera.yaw,
            camera_pitch: camera.pitch,
            camera_roll: camera.roll,
            camera_fov: camera.fov,
            camera_authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            camera_flags: MC_PRESENTATION_CAMERA_EXACT_V2_INPUTS
                | MC_PRESENTATION_CAMERA_EYE_Z_PLUS_128,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    fn render_camera_state(&self) -> McRenderCameraStateV2 {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        McRenderCameraStateV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McRenderCameraStateV2>(),
            generation: self.generation,
            tick: camera.tick,
            source_turn: camera.source_turn,
            history_x: camera.x,
            history_y: camera.y,
            eye_z: camera.z,
            yaw: camera.yaw & 0x07ff,
            pitch: camera.pitch,
            roll: camera.roll,
            fov: camera.fov,
            slope_target_x: slope.target_x,
            slope_target_y: slope.target_y,
            render_offset_x: slope.offset_x,
            render_offset_y: slope.offset_y,
            authority: MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            flags: MC_RENDER_CAMERA_TERRAIN_SLOPE_XY,
            reserved_0: 0,
            reserved: [0; 4],
        }
    }

    fn terrain_grid(&self) -> [McTerrainGridVertexV2; OUTDOOR_GRID_VERTICES] {
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.world.terrain_planes();
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
            render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        std::array::from_fn(|index| {
            let source = grid[index];
            McTerrainGridVertexV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainGridVertexV2>(),
                index: u16::try_from(index).expect("terrain grid index fits u16"),
                row: u8::try_from(index / 40).expect("terrain grid row fits u8"),
                column: u8::try_from(index % 40).expect("terrain grid column fits u8"),
                tile_x: source.tile_x,
                tile_y: source.tile_y,
                shade: source.shade,
                angle: source.angle,
                view_x: source.view_x,
                view_depth: source.view_depth,
                distance_squared: source.distance_squared,
                pnt5: source.pnt5.unwrap_or(0),
                raw_wave: source.raw_wave,
                surface_displacement: source.surface_displacement,
                lighting_wave: source.lighting_wave,
                feature_flags: source.feature_flags,
                tablesn_row: source.vertex_tablesn_row.unwrap_or(0),
                pnt5_written: u8::from(source.pnt5.is_some()),
                reserved: [0; 4],
            }
        })
    }

    fn terrain_frame(&self) -> AbiResult<Level1TerrainFrame> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "isolated terrain frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.world.terrain_planes();
        render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                render_x: camera.x.wrapping_add(slope.offset_x.cast_unsigned()),
                render_y: camera.y.wrapping_add(slope.offset_y.cast_unsigned()),
                eye_z: i32::from(camera.z),
                yaw: camera.yaw,
                pitch: camera.pitch,
                roll: camera.roll,
                fov: camera.fov,
                source_turn: camera.source_turn,
                reflections: true,
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    fn snapshot(&self) -> McSessionSnapshotV2 {
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let fire = self.context.spell_fire.as_ref();
        let selected = fire.map_or(0, |fire| fire.selected_spell_ui_index);
        let left_holder = fire.map_or(0, |fire| fire.left_holder_index);
        let right_holder = fire.map_or(0, |fire| fire.right_holder_index);
        let selected_holder = fire.map_or(0, |fire| fire.selected_holder_index);
        let caster_mana = fire.map_or(0, |fire| fire.caster.mana);
        McSessionSnapshotV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSessionSnapshotV2>(),
            generation: self.generation,
            tick: self.state.tick,
            event_head: self.event_head,
            state_hash: state_hash(&self.state),
            conformance_hash: conformance_hash(&self.state, &self.context),
            entity_index: self.state.entity_index,
            x: self.state.position.x,
            y: self.state.position.y,
            z: self.state.position.z,
            yaw: self.state.yaw,
            display_pitch: self.state.display_pitch,
            effective_pitch: self.state.effective_pitch,
            roll_accumulator: self.state.roll_accumulator,
            target_speed: self.state.target_speed,
            actual_speed: self.state.actual_speed,
            strafe_speed: self.state.strafe_speed,
            phase: self.state.entity_phase,
            last_resolution: self.last_resolution,
            profile: profile_code(self.world.profile().cave_mode),
            local_player_slot: self.world.runtime().local_player_slot,
            left_spell_slot: player.left_spell_slot,
            right_spell_slot: player.right_spell_slot,
            selected_spell_ui_index: selected,
            objective_textbox_ticks: self.world.runtime().objective_textbox_ticks,
            objective_count: u8::try_from(self.world.runtime().objectives.len())
                .expect("validated objective count fits u8"),
            objectives_authority: MC_AUTHORITY_FROZEN_OBSERVED,
            reserved_1: [0; 2],
            left_spell_holder: left_holder,
            right_spell_holder: right_holder,
            selected_spell_holder: selected_holder,
            reserved_0: 0,
            caster_mana,
            maptype_music: self.world.runtime().maptype_music,
            fire_sequence: self.fire_sequence,
            reserved: [0; 4],
        }
    }

    fn append_event(&mut self, event: McEventV2) {
        if self.events.len() == EVENT_CAPACITY
            && let Some(expired) = self.events.pop_front()
        {
            self.audio_commands.remove(&expired.sequence);
        }
        self.events.push_back(event);
    }

    fn next_event_base(&mut self, kind: u32, state_hash: u64) -> McEventV2 {
        self.event_head = next_nonzero(self.event_head);
        McEventV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventV2>(),
            generation: self.generation,
            sequence: self.event_head,
            state_hash,
            kind,
            code: 0,
            entity_index: 0,
            player_index: -1,
            value_0: 0,
            value_1: 0,
            asset_ref_index: MC_ASSET_REF_NONE,
            reserved: [0; 4],
        }
    }

    fn step(&mut self, command: PlayerCommand) {
        self.source_turn = self.source_turn.wrapping_add(1);
        if command.opcode == 8 {
            self.camera_fov = self.camera_fov.wrapping_add(i16::from(command.argument_1));
        }
        self.camera_look_yaw = command.argument_6.cast_signed();
        self.camera_look_pitch = command.argument_8.cast_signed();
        // `PlayerEvents_51BB0` stores this camera record after applying the
        // input/view fields but before `UpdateEntities_57730` advances flight.
        self.camera_history = CameraHistory::capture(
            &self.state,
            self.source_turn,
            self.camera_fov,
            self.camera_look_yaw,
            self.camera_look_pitch,
        );
        self.camera_slope_sample = self.camera_slope.update(
            self.world.terrain().ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.context.events.clear();
        let world = Arc::clone(&self.world);
        let evidence = step_outer_turn_with_context(
            &mut self.state,
            command,
            0,
            world.profile(),
            world.as_ref(),
            &mut self.context,
        );
        let player_control_flags = world
            .entities()
            .get(usize::from(self.state.entity_index))
            .map_or(0, |entity| entity.control_flags);
        let goat_events = self.live_world.step_with_players(
            world.as_ref(),
            &[GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: player_control_flags,
            }],
        );
        let state_hash = state_hash(&self.state);
        let typed_events = self.context.events.clone();
        for event in typed_events {
            let record = self.encode_flight_event(event, state_hash);
            self.append_event(record);
        }
        let live_world_hash = self.live_world.hash_v1();
        for event in goat_events {
            let record = self.encode_goat_event(event, live_world_hash);
            self.append_event(record);
        }
        self.drain_pending_audio_commands();
        for pass in evidence {
            let mut record = self.next_event_base(MC_EVENT_MOVEMENT_RESOLUTION, pass.state_hash);
            record.code = resolution_code(&pass);
            self.last_resolution = u8::try_from(record.code).unwrap_or(u8::MAX);
            self.append_event(record);
        }
        // The original loop stops music after its first DrawAndEvents pass,
        // then starts Night logical track 1 after the second pass while its
        // local `gameTurn` is 1. Keep that process-loop event distinct from
        // the per-player UpdateMusic envelope command.
        if self.music_start_armed && self.source_turn == 2 {
            let mut record = self.next_event_base(MC_EVENT_START_MUSIC, state_hash);
            record.code = u32::try_from(self.world.runtime().maptype_music)
                .expect("qualified Level 1 music selector is non-negative");
            record.value_0 = 0;
            record.value_1 = 1;
            record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
            self.append_event(record);
            self.music_start_armed = false;
        }
    }

    fn drain_pending_audio_commands(&mut self) {
        let unresolved = self
            .audio_commands
            .iter()
            .filter_map(|(sequence, command)| {
                (command.action == 0).then_some((*sequence, *command))
            })
            .collect::<Vec<_>>();
        let mut pending = BTreeMap::<u16, u64>::new();
        for (sequence, command) in unresolved {
            if let Some(current_sequence) = pending.get(&command.sound_id).copied() {
                let current = self.audio_commands[&current_sequence];
                if positional_sound_should_replace(
                    i32::from(command.volume_q15),
                    i32::from(current.volume_q15),
                ) {
                    self.audio_commands
                        .get_mut(&current_sequence)
                        .expect("pending command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                    pending.insert(command.sound_id, sequence);
                } else {
                    self.audio_commands
                        .get_mut(&sequence)
                        .expect("new command remains present")
                        .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                }
            } else {
                pending.insert(command.sound_id, sequence);
            }
        }
        for sequence in pending.into_values() {
            let mut command = self.audio_commands[&sequence];
            let (action, voice_index) = self.allocate_audio_voice(command);
            command.action = if matches!(command.sound_id, 1 | 2 | 5 | 31)
                && action == MC_AUDIO_ACTION_SUPPRESS_EXISTING
            {
                MC_AUDIO_ACTION_UPDATE_EXISTING
            } else {
                action
            };
            command.voice_index = voice_index;
            self.audio_commands.insert(sequence, command);
        }
    }

    fn allocate_audio_voice(&mut self, command: McAudioCommandV2) -> (u8, u16) {
        let matching = self.audio_voices.iter().position(|voice| {
            voice.is_some_and(|voice| {
                voice.sound_id == command.sound_id && voice.sound_flags == command.sound_flags
            })
        });
        if command.play_type == POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING
            && let Some(index) = matching
        {
            return (
                MC_AUDIO_ACTION_SUPPRESS_EXISTING,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        if command.play_type == 3
            && let Some(index) = matching
        {
            self.audio_voices[index] = Some(AudioVoice {
                sound_id: command.sound_id,
                sound_flags: command.sound_flags,
                event_sequence: command.event_sequence,
            });
            return (
                MC_AUDIO_ACTION_RESTART,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        let Some(index) = self.audio_voices.iter().position(Option::is_none) else {
            return (MC_AUDIO_ACTION_SUPPRESS_NO_VOICE, MC_AUDIO_VOICE_NONE);
        };
        self.audio_voices[index] = Some(AudioVoice {
            sound_id: command.sound_id,
            sound_flags: command.sound_flags,
            event_sequence: command.event_sequence,
        });
        (
            MC_AUDIO_ACTION_START,
            u16::try_from(index).expect("ten voices fit u16"),
        )
    }

    fn encode_flight_event(&mut self, event: FlightEvent, hash: u64) -> McEventV2 {
        match event {
            FlightEvent::PrepareSound {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if let Some(command) = self.prepare_audio_command(&record) {
                    self.audio_commands.insert(record.sequence, command);
                }
                record
            }
            FlightEvent::EndLoop {
                entity_index,
                player_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_END_LOOP, hash);
                record.entity_index = entity_index;
                record.player_index = player_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                if matches!(sound_id, 1 | 2 | 5 | 31) {
                    let matching = self.audio_voices.iter().position(|voice| {
                        voice.is_some_and(|voice| {
                            voice.sound_id == sound_id && voice.sound_flags == 0
                        })
                    });
                    let mut command = local_audio_command_record(
                        &record,
                        sound_id,
                        0,
                        POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
                    );
                    if let Some(index) = matching {
                        command.action = MC_AUDIO_ACTION_END_LOOP;
                        command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                    } else {
                        command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                    }
                    self.audio_commands.insert(record.sequence, command);
                }
                record
            }
            FlightEvent::UpdateMusic {
                mode,
                timer_multiplier,
            } => {
                let mut record = self.next_event_base(MC_EVENT_UPDATE_MUSIC, hash);
                record.code = u32::from(mode);
                record.value_0 = i32::from(timer_multiplier);
                record.value_1 = self.source_turn;
                record.asset_ref_index = self.world.music_asset_ref().unwrap_or(MC_ASSET_REF_NONE);
                record
            }
            FlightEvent::RequestSpellActivation {
                source,
                holder_entity_index,
                direction_mask,
            } => {
                self.fire_sequence = self.fire_sequence.wrapping_add(1);
                let mut record = self.next_event_base(MC_EVENT_REQUEST_SPELL_ACTIVATION, hash);
                record.entity_index = holder_entity_index;
                record.code = match source {
                    sim_core::SpellFireSource::Left => 0,
                    sim_core::SpellFireSource::Right => 1,
                    sim_core::SpellFireSource::Selected => 2,
                };
                record.value_0 = i32::from(direction_mask);
                record
            }
            FlightEvent::ShowHelpHint { hint_index, state } => {
                let mut record = self.next_event_base(MC_EVENT_SHOW_HELP_HINT, hash);
                record.code = u32::from(hint_index);
                record.value_0 = i32::from(state);
                record
            }
        }
    }

    fn encode_goat_event(&mut self, event: GoatStepEvent, live_hash: u64) -> McEventV2 {
        match event {
            GoatStepEvent::Bleat {
                entity_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_SOUND_REQUEST, live_hash);
                record.entity_index = entity_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                record
            }
            GoatStepEvent::ManaConverted { entity_index, mana } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_MANA_CONVERTED, live_hash);
                record.entity_index = entity_index;
                record.value_0 = mana;
                record
            }
            GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                let mut record =
                    self.next_event_base(MC_EVENT_GOAT_EXTRA_MANA_SPHERE_REQUEST, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::RemovalQueued { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_REMOVAL_QUEUED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::Reclaimed { entity_index } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_RECLAIMED, live_hash);
                record.entity_index = entity_index;
                record
            }
            GoatStepEvent::EngagementBoundaryReached {
                entity_index,
                target_entity_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_ENGAGEMENT_BOUNDARY, live_hash);
                record.entity_index = entity_index;
                record.value_0 = i32::from(target_entity_index);
                record
            }
        }
    }

    fn prepare_audio_command(&self, event: &McEventV2) -> Option<McAudioCommandV2> {
        let sound_id = u16::try_from(event.code).ok()?;
        if matches!(sound_id, 1 | 2 | 5 | 31) {
            return Some(local_audio_command_record(
                event,
                sound_id,
                0,
                POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
            ));
        }
        if sound_id == 29 {
            return Some(local_audio_command_record(event, sound_id, 0, 3));
        }
        if !matches!(sound_id, 37 | 39 | 43) {
            return None;
        }
        let source = self.world.entities().get(usize::from(event.entity_index))?;
        let source_position = if event.entity_index == self.state.entity_index {
            self.state.position
        } else {
            source.position
        };
        let random_state = self
            .context
            .effects
            .as_ref()
            .map_or(self.world.runtime().random_state, |effects| {
                effects.global_rng
            });
        let command = positional_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source.source_id,
        )?;
        Some(audio_command_record(
            event.generation,
            event.sequence,
            event.state_hash,
            event.asset_ref_index,
            sound_id,
            event.entity_index,
            event.player_index,
            command,
        ))
    }
}

fn local_audio_command_record(
    event: &McEventV2,
    sound_id: u16,
    sound_flags: i16,
    play_type: u8,
) -> McAudioCommandV2 {
    let loop_count = if matches!(sound_id, 1 | 2 | 5 | 31) {
        -1
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation: event.generation,
        event_sequence: event.sequence,
        state_hash: event.state_hash,
        asset_ref_index: event.asset_ref_index,
        playback_rate_hz: 22_050,
        sound_id,
        entity_index: event.entity_index,
        player_index: event.player_index,
        sound_flags,
        volume_q15: if sound_id == 29 { 0x7fff } else { 0 },
        pan_q16: 0x7fff,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type,
        loop_count,
        action: 0,
        reserved: [0; 4],
        ..McAudioCommandV2::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn audio_command_record(
    generation: u64,
    event_sequence: u64,
    state_hash: u64,
    asset_ref_index: u32,
    sound_id: u16,
    entity_index: u16,
    player_index: i16,
    command: PositionalSoundCommand,
) -> McAudioCommandV2 {
    let playback_rate_hz = if command.audible {
        let percent = u32::try_from(100 + i32::from(command.rate_delta_percent))
            .expect("bounded playback percentage is positive");
        22_050 * percent / 100
    } else {
        0
    };
    McAudioCommandV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McAudioCommandV2>(),
        generation,
        event_sequence,
        state_hash,
        asset_ref_index,
        relative_x: command.relative_x,
        relative_y: command.relative_y,
        relative_z: command.relative_z,
        playback_rate_hz,
        sound_id,
        entity_index,
        player_index,
        sound_flags: command.sound_flags,
        volume_q15: command.volume_q15,
        pan_q16: command.pan_q16,
        rate_delta_percent: command.rate_delta_percent,
        voice_index: MC_AUDIO_VOICE_NONE,
        play_type: POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
        loop_count: 0,
        action: if command.audible {
            0
        } else {
            MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME
        },
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn positional_qualification_command(index: u32) -> AbiResult<McAudioCommandV2> {
    let (dx, dy, sound_id, random_state) = match index {
        0 | 22 => (0, -1_024, 43, 28),
        1 => (512, -512, 37, 0),
        2 => (1_024, 0, 39, 0),
        3 => (512, 512, 37, 0),
        4 => (0, 1_024, 37, 0),
        5 => (-512, 512, 37, 0),
        6 => (-1_024, 0, 37, 0),
        7 => (-512, -512, 37, 0),
        8 => (320, 0, 37, 0),
        9 => (321, 0, 37, 0),
        10 => (0, -12_095, 37, 0),
        11 => (0, -12_096, 37, 0),
        12 => (9_071, 0, 37, 0),
        13 => (9_072, 0, 37, 0),
        14 => (0, 6_047, 37, 0),
        15 => (0, 6_048, 37, 0),
        16 => (12_288, 0, 37, 0),
        17 => (12_288, 1, 37, 0),
        18 => (0, 0, 43, 13),
        19 => (0, 0, 43, 28),
        20 => (0, 0, 43, 20),
        21 => (0, -6_000, 43, 28),
        _ => {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                format!("positional qualification vector {index} is out of bounds"),
            ));
        }
    };
    let listener = Position {
        x: 32_768,
        y: 32_768,
        z: 1_000,
    };
    let source = Position {
        x: u16::try_from(i32::from(listener.x) + dx).expect("qualification x remains in range"),
        y: u16::try_from(i32::from(listener.y) + dy).expect("qualification y remains in range"),
        z: listener.z,
    };
    let command = positional_sound_command(listener, 0, source, sound_id, random_state, 321)
        .expect("qualification vector uses a proven Level 1 positional sound");
    let asset_ref_index = match sound_id {
        37 => 82,
        39 => 83,
        43 => 84,
        _ => unreachable!("qualification sound is bounded"),
    };
    let mut record = audio_command_record(
        1,
        u64::from(index) + 1,
        0,
        asset_ref_index,
        sound_id,
        321,
        -1,
        command,
    );
    if record.action == 0 {
        record.action = MC_AUDIO_ACTION_START;
        record.voice_index = 0;
    }
    if index == 22 {
        record.action = MC_AUDIO_ACTION_SUPPRESS_EXISTING;
        record.voice_index = 0;
    }
    Ok(record)
}

#[derive(Default)]
struct Registry {
    next_handle: u64,
    sessions: BTreeMap<u64, Arc<Mutex<SessionRecord>>>,
}

impl Registry {
    fn insert(&mut self, session: SessionRecord) -> AbiResult<u64> {
        for _ in 0..u16::MAX {
            self.next_handle = next_nonzero(self.next_handle);
            if !self.sessions.contains_key(&self.next_handle) {
                let handle = self.next_handle;
                self.sessions.insert(handle, Arc::new(Mutex::new(session)));
                return Ok(handle);
            }
        }
        Err(AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "session handle space is unavailable",
        ))
    }
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn boundary(operation: impl FnOnce() -> AbiResult<()>) -> i32 {
    LAST_ERROR.with(|error| error.borrow_mut().clear());
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => {
            set_last_error("panic contained at mc_native ABI boundary");
            MC_STATUS_INTERNAL_PANIC
        }
    }
}

fn boundary_value<T: Copy>(
    output: *mut T,
    output_size: u32,
    operation: impl FnOnce() -> AbiResult<T>,
) -> i32 {
    boundary(|| {
        require_struct_output::<T>(output, output_size, "output")?;
        let value = operation()?;
        // SAFETY: the caller supplied a non-null writable output pointer per
        // the ABI contract; unaligned writes support conservative P/Invoke.
        unsafe { output.write_unaligned(value) };
        Ok(())
    })
}

fn set_last_error(message: &str) {
    LAST_ERROR.with(|error| {
        let mut error = error.borrow_mut();
        error.clear();
        error.push_str(message);
    });
}

fn with_registry<T>(operation: impl FnOnce(&Registry) -> AbiResult<T>) -> AbiResult<T> {
    let guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&guard)
}

fn with_registry_mut<T>(operation: impl FnOnce(&mut Registry) -> AbiResult<T>) -> AbiResult<T> {
    let mut guard = registry()
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session registry lock is poisoned"))?;
    operation(&mut guard)
}

fn session_arc(registry: &Registry, handle: u64) -> AbiResult<Arc<Mutex<SessionRecord>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown session handle {handle}"),
        )
    })
}

fn with_session<T>(
    handle: u64,
    operation: impl FnOnce(&SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_registry(|registry| session_arc(registry, handle))?;
    let guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
    operation(&guard)
}

fn with_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut SessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_registry(|registry| session_arc(registry, handle))?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "session lock is poisoned"))?;
    operation(&mut guard)
}

#[unsafe(no_mangle)]
/// Mechanically infallible constant expression: this export touches no memory,
/// performs no allocation and invokes no panic-capable operation.
pub extern "C" fn mc_native_abi_version() -> u32 {
    MC_NATIVE_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    out_handle: *mut u64,
) -> i32 {
    boundary_value(out_handle, struct_size::<u64>(), || {
        let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
        let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
        if package.is_empty() || closure.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "package_dir and closure_path must be non-empty",
            ));
        }
        let world = NativeWorld::load(&package, &closure).map_err(|error| {
            let message = error.to_string();
            let status = if message.starts_with("cannot read ") {
                MC_STATUS_IO_ERROR
            } else {
                MC_STATUS_VALIDATION_FAILED
            };
            AbiError::new(status, message)
        })?;
        let session = SessionRecord::new(world)?;
        with_registry_mut(|registry| registry.insert(session))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_destroy(handle: u64) -> i32 {
    boundary(|| {
        with_registry_mut(|registry| {
            if handle == 0 || registry.sessions.remove(&handle).is_none() {
                Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown session handle {handle}"),
                ))
            } else {
                Ok(())
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_reset(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session_mut(handle, |session| {
            session.reset();
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_snapshot(
    handle: u64,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(session.snapshot()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_source_projection_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(source_projection_hash(&session.state)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_presentation_state(
    handle: u64,
    out_state: *mut McPresentationStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.presentation_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_render_camera_state(
    handle: u64,
    out_state: *mut McRenderCameraStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(session.render_camera_state()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector_count() -> u32 {
    TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_pnt5_vector(
    index: u32,
    out_vector: *mut McTerrainPnt5VectorV2,
    out_vector_size: u32,
) -> i32 {
    boundary_value(out_vector, out_vector_size, || {
        let (turn, tile_x, tile_y, shade, angle, view_y, distance_squared) = match index {
            0 => (0, 4, 4, 0, 8, 128, 0),
            1 => (0, 12, 4, 40, 8, 128, 0),
            2 => (1, 0, 0, 40, 8, 128, 0),
            3 => (1, 1, 1, 40, 8, 128, 14_745_600),
            4 => (1, 2, 3, 40, 8, 128, 14_745_601),
            5 => (1, 5, 7, 40, 8, 128, 19_202_048),
            6 => (1, 255, 255, 40, 8, 128, 23_658_495),
            7 => (1, 255, 255, 40, 8, 128, 23_658_496),
            8 => (1, 255, 255, 40, 8, 128, 26_214_399),
            9 => (1, 255, 255, 40, 8, 128, 26_214_400),
            10 => (1, 42, 99, 56, 8, 128, 0),
            11 => (65_535, 256, -1, 55, 8, 128, 14_757_945),
            12 => (1, 0, 0, 40, 8, -256, 0),
            _ => {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("terrain pnt5 qualification vector {index} is out of bounds"),
                ));
            }
        };
        let value =
            outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
        Ok(McTerrainPnt5VectorV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainPnt5VectorV2>(),
            turn,
            tile_x,
            tile_y,
            distance_squared,
            shade,
            angle,
            visible: u8::from(value.visible),
            tablesn_row: value.tablesn_row,
            wave: value.wave,
            land_displacement: value.land_displacement,
            pnt5: value.pnt5,
            view_y,
            lighting_wave: value.lighting_wave,
            reserved: [0; 2],
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_grid_vertex_count() -> u32 {
    u32::try_from(OUTDOOR_GRID_VERTICES).expect("terrain grid vertex count fits u32")
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_grid_copy(
    handle: u64,
    buffer: *mut McTerrainGridVertexV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McTerrainGridVertexV2>(element_size, "terrain grid vertex")?;
        with_session(handle, |session| {
            let grid = session.terrain_grid();
            copy_slice(&grid, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step(
    handle: u64,
    command: *const McCommandV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let command = player_command(command)?;
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_step_with_camera(
    handle: u64,
    command: *const McCommandV2,
    camera_input: *const McCameraInputV2,
    out_snapshot: *mut McSessionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        let command = read_versioned_input(command, "command")?;
        validate_command(&command)?;
        let camera_input = read_versioned_input(camera_input, "camera input")?;
        validate_camera_input(&camera_input)?;
        let mut command = player_command(command)?;
        command.argument_6 = camera_input.look_yaw;
        command.argument_8 = camera_input.look_pitch;
        with_session_mut(handle, |session| {
            session.step(command);
            Ok(session.snapshot())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_frame_copy(
    handle: u64,
    query: *const McTerrainFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    coverage_buffer: *mut u8,
    coverage_capacity: u32,
    out_info: *mut McTerrainFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "terrain frame query")?;
        validate_reserved(&query.reserved, "terrain frame query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "terrain frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified terrain frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "terrain frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "terrain frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.terrain_frame()?;
            // SAFETY: both caller buffers were checked non-null and have at
            // least the exact 64,000-byte frame capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                coverage_buffer
                    .copy_from_nonoverlapping(frame.terrain_coverage.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            Ok(McTerrainFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McTerrainFrameInfoV2>(),
                generation: session.generation,
                tick: session.camera_history.tick,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_x: 0,
                viewport_y: 0,
                viewport_width: LEVEL1_VIEW_WIDTH as u16,
                viewport_height: LEVEL1_VIEW_HEIGHT as u16,
                reserved_0: 0,
                index_bytes: required,
                coverage_bytes: required,
                flags: MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                    | MC_TERRAIN_FRAME_SKY_SEEDED
                    | MC_TERRAIN_FRAME_HAS_COVERAGE
                    | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED,
                reflected_triangles: frame.stats.reflected_triangles,
                primary_mode5_triangles: frame.stats.primary_mode5_triangles,
                primary_mode26_triangles: frame.stats.primary_mode26_triangles,
                index_hash: frame.index_hash,
                coverage_hash: frame.coverage_hash,
                terrain_pixel_writes: frame.stats.terrain_pixel_writes,
                reserved: [0; 2],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_info(
    handle: u64,
    out_info: *mut McWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let world = &session.world;
            let metadata = world.metadata();
            Ok(McWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: profile_code(world.profile().cave_mode),
                reserved_0: 0,
                entity_count: u32::try_from(world.entities().len())
                    .expect("validated entity count fits u32"),
                active_entity_count: u32::try_from(
                    world
                        .entities()
                        .iter()
                        .filter(|entity| entity.active)
                        .count(),
                )
                .expect("validated active count fits u32"),
                player_count: u32::try_from(world.players().len())
                    .expect("validated player count fits u32"),
                asset_ref_count: u32::try_from(world.asset_refs().len())
                    .expect("validated asset count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: state_hash(world.reset_state()),
                reset_conformance_hash: conformance_hash(
                    world.reset_state(),
                    world.reset_context(),
                ),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_length(
    handle: u64,
    kind: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            string_length(value)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_world_string_copy(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let value = world_string(&session.world, kind)?;
            copy_string(value, buffer, capacity, out_length)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_length(
    handle: u64,
    kind: u32,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let world = &session.world;
            validate_plane_kind(world.terrain_planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_plane_copy_u8(
    handle: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let world = &session.world;
            let values = terrain_u8_plane(world.terrain_planes(), kind)?;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_terrain_entity_indices_copy(
    handle: u64,
    buffer: *mut i16,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_session(handle, |session| {
            let values = &session.world.terrain_planes().entity_indices;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.entities().len())
                .expect("validated entity count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.runtime().objectives.len())
                .expect("validated objective count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "objective element")?;
        with_session(handle, |session| {
            let objectives = &session.world.runtime().objectives;
            let start = usize::try_from(start_slot)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective start exceeds usize"))?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "objective start is beyond objective count",
                ));
            }
            let remaining = &objectives[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity exceeds usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "objective output buffer is null",
                ));
            }
            for (offset, objective) in remaining.iter().take(count).enumerate() {
                let slot = start + offset;
                let snapshot = objective_snapshot(
                    u8::try_from(slot).expect("validated objective slot fits u8"),
                    objective,
                );
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records of the exact ABI stride.
                unsafe { buffer.add(offset).write_unaligned(snapshot) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McEntitySnapshotV2>(element_size, "entity element")?;
        with_session(handle, |session| {
            let entities = session.world.entities();
            let start = usize::try_from(start_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity start does not fit usize"))?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "entity start is beyond entity count",
                ));
            }
            let remaining = &entities[start..];
            let count = remaining.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "entity output buffer is null",
                ));
            }
            for (index, entity) in remaining.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe {
                    buffer.add(index).write_unaligned(entity_snapshot(
                        entity,
                        &session.state,
                        &session.live_world,
                    ));
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok::<(), AbiError>(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McGoatSnapshotV2>(element_size, "Goat element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = session.live_world.goats();
            let start = usize::try_from(start_ordinal)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat start exceeds usize"))?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "Goat capacity exceeds usize"))?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                // SAFETY: the caller supplied exact element stride and
                // writable capacity for `count` records.
                unsafe {
                    buffer.add(offset).write_unaligned(goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world.hash_v1(),
                start_ordinal,
                written: u32::try_from(count).expect("33 Goats fit u32"),
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_inject_damage(
    handle: u64,
    command: *const McGoatDamageCommandV2,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "Goat damage command")?;
        validate_reserved(&command.reserved, "Goat damage command")?;
        if command.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Goat damage command reserved_0 must be zero",
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat damage generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            session
                .live_world
                .inject_damage(
                    command.entity_index,
                    command.damage,
                    session.state.entity_index.cast_signed(),
                )
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: session.live_world.outer_tick().into(),
                live_world_hash: session.live_world.hash_v1(),
                start_ordinal: 0,
                written: 0,
                total_count: u32::try_from(LEVEL_ONE_GOAT_INDICES.len()).expect("33 Goats fit u32"),
                has_more: 0,
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(u32::try_from(session.world.asset_refs().len())
                .expect("validated asset count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_ref_info(
    handle: u64,
    asset_index: u32,
    out_info: *mut McAssetRefInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            Ok(McAssetRefInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McAssetRefInfoV2>(),
                asset_index,
                trace_ordinal: asset.observation.ordinal,
                owner_asset_count: u16::try_from(asset.owner_asset_ids.len()).map_err(|_| {
                    AbiError::new(MC_STATUS_BOUNDS, "owner asset count exceeds u16")
                })?,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_length(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    out_length: *mut u32,
) -> i32 {
    boundary_value(out_length, struct_size::<u32>(), || {
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            string_length(asset_string(asset, kind, owner_index)?)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_asset_string_copy(
    handle: u64,
    asset_index: u32,
    kind: u32,
    owner_index: u32,
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_length, "out_length")?;
        with_session(handle, |session| {
            let asset = asset_ref(&session.world, asset_index)?;
            copy_string(
                asset_string(asset, kind, owner_index)?,
                buffer,
                capacity,
                out_length,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_events_copy_since(
    handle: u64,
    query: *const McEventQueryV2,
    buffer: *mut McEventV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McEventBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McEventV2>(element_size, "event element")?;
        let query = read_versioned_input(query, "event query")?;
        validate_reserved(&query.reserved, "event query")?;
        with_session(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "event query generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.cursor > session.event_head {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "event cursor is beyond current event head",
                ));
            }
            if let Some(oldest) = session.events.front()
                && query.cursor.saturating_add(1) < oldest.sequence
            {
                return Err(AbiError::new(
                    MC_STATUS_CURSOR_EXPIRED,
                    "event cursor predates the retained journal",
                ));
            }
            let available = session
                .events
                .iter()
                .filter(|event| event.sequence > query.cursor)
                .collect::<Vec<_>>();
            let count = available.len().min(
                usize::try_from(capacity)
                    .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "capacity does not fit usize"))?,
            );
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "event output buffer is null",
                ));
            }
            for (index, event) in available.iter().take(count).enumerate() {
                // SAFETY: buffer is non-null for count records and the caller
                // promises capacity writable records.
                unsafe { buffer.add(index).write_unaligned(**event) };
            }
            let next_cursor = if count == 0 {
                query.cursor
            } else {
                available[count - 1].sequence
            };
            Ok(McEventBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventBatchV2>(),
                generation: session.generation,
                next_cursor,
                event_head: session.event_head,
                written: u32::try_from(count).expect("event batch count fits u32"),
                has_more: u8::from(count < available.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_for_event(
    handle: u64,
    generation: u64,
    event_sequence: u64,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio command generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_voice_complete(
    handle: u64,
    generation: u64,
    voice_index: u16,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio completion generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let index = usize::from(voice_index);
            let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} is out of bounds"),
                )
            })?;
            if !voice.is_some_and(|voice| voice.event_sequence == event_sequence) {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("audio voice {voice_index} does not own event {event_sequence}"),
                ));
            }
            *voice = None;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub const extern "C" fn mc_native_audio_qualification_vector_count() -> u32 {
    POSITIONAL_QUALIFICATION_VECTOR_COUNT
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_vector(
    index: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary_value(out_command, out_command_size, || {
        positional_qualification_command(index)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_qualification_submit(
    handle: u64,
    event_kind: u32,
    sound_id: u16,
    sound_flags: i16,
    volume_q15: u16,
    pan_q16: u16,
    play_type: u8,
    out_event: *mut McEventV2,
    out_event_size: u32,
    out_command: *mut McAudioCommandV2,
    out_command_size: u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McEventV2>(out_event, out_event_size, "audio event")?;
        require_struct_output::<McAudioCommandV2>(out_command, out_command_size, "audio command")?;
        if !matches!(event_kind, MC_EVENT_PREPARE_SOUND | MC_EVENT_END_LOOP)
            || !matches!(sound_id, 1 | 2 | 5 | 29 | 31 | 37 | 39 | 43)
            || !matches!(play_type, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING | 3)
            || volume_q15 > 0x7fff
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "qualification audio submission is outside the proven effect contract",
            ));
        }
        let (event, command) = with_session_mut(handle, |session| {
            let mut event = session.next_event_base(event_kind, state_hash(&session.state));
            event.code = u32::from(sound_id);
            event.entity_index = session.state.entity_index;
            event.player_index = -1;
            event.asset_ref_index = session.world.sound_asset_ref(sound_id).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("sound {sound_id} has no qualified owner asset"),
                )
            })?;
            let mut command = local_audio_command_record(&event, sound_id, sound_flags, play_type);
            command.volume_q15 = volume_q15;
            command.pan_q16 = pan_q16;
            if event_kind == MC_EVENT_END_LOOP {
                let matching = session.audio_voices.iter().position(|voice| {
                    voice.is_some_and(|voice| {
                        voice.sound_id == sound_id && voice.sound_flags == sound_flags
                    })
                });
                if let Some(index) = matching {
                    command.action = MC_AUDIO_ACTION_END_LOOP;
                    command.voice_index = u16::try_from(index).expect("ten voices fit u16");
                } else {
                    command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                }
            } else {
                let (action, voice_index) = session.allocate_audio_voice(command);
                command.action = if matches!(sound_id, 1 | 2 | 5 | 31)
                    && action == MC_AUDIO_ACTION_SUPPRESS_EXISTING
                {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
            }
            session.audio_commands.insert(event.sequence, command);
            session.append_event(event);
            Ok((event, command))
        })?;
        write_output(out_event, event);
        write_output(out_command, command);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_length(out_length: *mut u32) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        require_output(out_length, "out_length")?;
        LAST_ERROR.with(|error| {
            let length = string_length(&error.borrow())?;
            write_output(out_length, length);
            Ok::<(), AbiError>(())
        })
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        Ok(Err(error)) => {
            set_last_error(&error.message);
            error.status
        }
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_last_error_copy(
    buffer: *mut u8,
    capacity: u32,
    out_length: *mut u32,
) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        let message = LAST_ERROR.with(|error| error.borrow().clone());
        require_output(out_length, "out_length")?;
        copy_string(&message, buffer, capacity, out_length)
    })) {
        Ok(Ok(())) => MC_STATUS_OK,
        // Error retrieval must not destroy the diagnostic being retrieved.
        Ok(Err(error)) => error.status,
        Err(_) => MC_STATUS_INTERNAL_PANIC,
    }
}

fn validate_command(command: &McCommandV2) -> AbiResult<()> {
    validate_reserved(&command.reserved, "command")?;
    if command.requested_spell != MC_REQUESTED_SPELL_NONE
        && !(0..=28).contains(&command.requested_spell)
    {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            "requested spell must be -1 or 0..=28",
        ));
    }
    Ok(())
}

fn validate_camera_input(camera_input: &McCameraInputV2) -> AbiResult<()> {
    validate_reserved(&camera_input.reserved, "camera input")
}

fn validate_reserved(reserved: &[u32], name: &str) -> AbiResult<()> {
    if reserved.iter().any(|value| *value != 0) {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            format!("{name} reserved fields must be zero"),
        ));
    }
    Ok(())
}

fn player_command(command: McCommandV2) -> AbiResult<PlayerCommand> {
    let (opcode, argument_1) = if command.requested_spell == MC_REQUESTED_SPELL_NONE {
        (command.opcode, command.argument_1)
    } else {
        (
            6,
            i8::try_from(command.requested_spell)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "requested spell exceeds i8"))?,
        )
    };
    Ok(PlayerCommand {
        opcode,
        argument_1,
        argument_2: command.argument_2,
        roll: command.roll,
        pitch: command.pitch,
        flags: command.flags,
        ..PlayerCommand::default()
    })
}

fn world_string(world: &NativeWorld, kind: u32) -> AbiResult<&str> {
    match kind {
        MC_WORLD_STRING_SCHEMA => Ok("mc2-owner-level-export/v2"),
        MC_WORLD_STRING_PACKAGE_SHA256 => Ok(world.package_sha256()),
        MC_WORLD_STRING_CLOSURE_SHA256 => Ok(world.closure_sha256()),
        MC_WORLD_STRING_STATE_HASH_SCHEMA => Ok("mc2-flight-state-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONFORMANCE_HASH_SCHEMA => Ok("mc2-flight-conformance-hash/fnv1a64-v1"),
        MC_WORLD_STRING_CONTEXT_SEAM => {
            Ok("Level1 control_mode=0; regression-proven, not serialized by package v2")
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown world string kind {kind}"),
        )),
    }
}

fn validate_plane_kind(planes: &TerrainPlanes, kind: u32) -> AbiResult<()> {
    match kind {
        MC_TERRAIN_TYPES_U8
        | MC_TERRAIN_HEIGHT_U8
        | MC_TERRAIN_SHADE_U8
        | MC_TERRAIN_ANGLE_U8
        | MC_TERRAIN_SECONDARY_U8 => {
            terrain_u8_plane(planes, kind)?;
            Ok(())
        }
        MC_TERRAIN_ENTITY_INDEX_I16 => Ok(()),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown terrain plane kind {kind}"),
        )),
    }
}

fn terrain_u8_plane(planes: &TerrainPlanes, kind: u32) -> AbiResult<&[u8]> {
    match kind {
        MC_TERRAIN_TYPES_U8 => Ok(&planes.terrain_types),
        MC_TERRAIN_HEIGHT_U8 => Ok(&planes.ground_heights),
        MC_TERRAIN_SHADE_U8 => Ok(&planes.shading),
        MC_TERRAIN_ANGLE_U8 => Ok(&planes.angles),
        MC_TERRAIN_SECONDARY_U8 => Ok(&planes.secondary_heights),
        MC_TERRAIN_ENTITY_INDEX_I16 => Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "entity-index terrain plane requires the i16 copy API",
        )),
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown u8 terrain plane kind {kind}"),
        )),
    }
}

fn entity_snapshot(
    entity: &EntitySnapshot,
    state: &FlightState,
    live_world: &LiveWorldState,
) -> McEntitySnapshotV2 {
    let (has_parameter_record, parameter_record_index) = entity
        .parameter_relation
        .map_or((0, MC_PARAMETER_RECORD_NONE), |index| (1, index));
    let mut snapshot = McEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McEntitySnapshotV2>(),
        entity_index: entity.index,
        parameter_record_index,
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        has_parameter_record,
        reserved_0: 0,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
        reserved: [0; 4],
    };
    if entity.authority == EntityAuthority::PlayerFlightLive && entity.index == state.entity_index {
        snapshot.x = state.position.x;
        snapshot.y = state.position.y;
        snapshot.z = state.position.z;
        snapshot.yaw = state.yaw;
        snapshot.pitch = state.display_pitch;
        snapshot.fov = state.entity_fov;
        snapshot.actual_speed = state.actual_speed;
        snapshot.phase = state.entity_phase;
    } else if entity.authority == EntityAuthority::GoatLive
        && let Some(goat) = live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_GOAT_LIVE;
        snapshot.active = u8::from(goat.active);
        snapshot.action = goat.action;
        snapshot.phase = goat.phase;
        snapshot.x = goat.position.x;
        snapshot.y = goat.position.y;
        snapshot.z = goat.position.z;
        snapshot.yaw = goat.yaw;
        snapshot.pitch = goat.pitch;
        snapshot.actual_speed = goat.actual_speed;
    }
    snapshot
}

fn goat_snapshot(goat: &GoatState) -> McGoatSnapshotV2 {
    McGoatSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McGoatSnapshotV2>(),
        entity_index: goat.entity_index,
        target_entity_index: goat.target_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        render_parameter_index: goat.render_parameter_index,
        base_tmap: GOAT_BASE_TMAP,
        authority: MC_AUTHORITY_GOAT_LIVE,
        active: u8::from(goat.active),
        class_id: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        stage_var_1: goat.stage_var_1,
        stage_var_2: goat.stage_var_2,
        proximity_countdown: goat.proximity_countdown,
        proximity_subcounter: goat.proximity_subcounter,
        animation_frame: goat.animation_frame,
        reserved_0: 0,
        x: goat.position.x,
        y: goat.position.y,
        z: goat.position.z,
        yaw: goat.yaw,
        pitch: goat.pitch,
        roll: goat.roll,
        actual_speed: goat.actual_speed,
        minimum_speed: goat.minimum_speed,
        maximum_speed: goat.maximum_speed,
        rng: goat.rng,
        life: goat.life,
        max_life: goat.max_life,
        mana: goat.mana,
        pending_damage: goat.pending_damage,
        control_flags: goat.control_flags,
        reserved: [0; 4],
    }
}

fn objective_snapshot(
    slot: u8,
    objective: &sim_core::native_world::RuntimeObjective,
) -> McObjectiveSnapshotV2 {
    McObjectiveSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McObjectiveSnapshotV2>(),
        slot,
        is_level_end: objective.is_level_end,
        text_selector: objective.text_selector,
        done: objective.done,
        authority: MC_AUTHORITY_FROZEN_OBSERVED,
        stage_selectors: objective.stage_selectors,
        reserved_0: [0; 3],
        reserved: [0; 4],
    }
}

fn asset_ref(world: &NativeWorld, index: u32) -> AbiResult<&AssetRef> {
    let index = usize::try_from(index)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "asset index does not fit usize"))?;
    world.asset_refs().get(index).ok_or_else(|| {
        AbiError::new(
            MC_STATUS_BOUNDS,
            format!("asset index {index} is out of bounds"),
        )
    })
}

fn asset_string(asset: &AssetRef, kind: u32, owner_index: u32) -> AbiResult<&str> {
    match kind {
        MC_ASSET_STRING_ID => Ok(&asset.asset_id),
        MC_ASSET_STRING_CONTENT_ADDRESS => Ok(&asset.content_address),
        MC_ASSET_STRING_CATEGORY => Ok(&asset.observation.category),
        MC_ASSET_STRING_HOOK => Ok(&asset.observation.hook),
        MC_ASSET_STRING_STAGE => Ok(asset.observation.stage.as_deref().unwrap_or("")),
        MC_ASSET_STRING_OWNER_ID => {
            let owner_index = usize::try_from(owner_index)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "owner index does not fit usize"))?;
            asset
                .owner_asset_ids
                .get(owner_index)
                .map(String::as_str)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("owner asset index {owner_index} is out of bounds"),
                    )
                })
        }
        _ => Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("unknown asset string kind {kind}"),
        )),
    }
}

fn resolution_code(evidence: &StepEvidence) -> u32 {
    match evidence.movement_resolution {
        "accepted" => 0,
        "redirected" => 1,
        "rejected" => 2,
        "rejected-and-stopped" => 3,
        "recovered-after-reject" => 4,
        "skipped" => 5,
        _ => u32::MAX,
    }
}

fn profile_code(cave_mode: bool) -> u8 {
    if cave_mode {
        MC_PROFILE_CAVE
    } else {
        MC_PROFILE_OUTDOOR
    }
}

fn next_nonzero(value: u64) -> u64 {
    let next = value.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

fn load_optional_terrain_frame_assets() -> AbiResult<Option<TerrainFrameAssets>> {
    let Some(root) = std::env::var_os("MC_ASSET_CACHE_PATH") else {
        return Ok(None);
    };
    let root = PathBuf::from(root);
    let terrain_atlas = read_qualified_cache_file(
        &root,
        "terrain/bl32n.atlas.indices.u8",
        LEVEL1_TERRAIN_ATLAS_BYTES,
        TERRAIN_ATLAS_SHA256,
    )?;
    let tablesn = read_qualified_cache_file(
        &root,
        "palette/tablesn.u8",
        sim_core::mode26_raster::NIGHT_TABLES_BYTES,
        TABLESN_SHA256,
    )?;
    let sky =
        read_qualified_cache_file(&root, "sky/skyn.indices.u8", LEVEL1_SKY_BYTES, SKYN_SHA256)?;
    Ok(Some(TerrainFrameAssets {
        terrain_atlas: terrain_atlas.into_boxed_slice(),
        tablesn: tablesn.into_boxed_slice(),
        sky: sky.into_boxed_slice(),
    }))
}

fn read_qualified_cache_file(
    root: &Path,
    relative: &str,
    expected_len: usize,
    expected_sha256: &str,
) -> AbiResult<Vec<u8>> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!(
                "cannot resolve owner asset cache {}: {error}",
                root.display()
            ),
        )
    })?;
    if fs::symlink_metadata(&canonical_root)
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_IO_ERROR,
                format!(
                    "cannot inspect owner asset cache {}: {error}",
                    canonical_root.display()
                ),
            )
        })?
        .file_type()
        .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            "owner asset cache root must not be a symlink",
        ));
    }
    let candidate = canonical_root.join(relative);
    let canonical_candidate = fs::canonicalize(&candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot resolve owner cache file {relative}: {error}"),
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_root)
        || fs::symlink_metadata(&candidate)
            .map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("cannot inspect owner cache file {relative}: {error}"),
                )
            })?
            .file_type()
            .is_symlink()
    {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} escapes the cache or is a symlink"),
        ));
    }
    let bytes = fs::read(&canonical_candidate).map_err(|error| {
        AbiError::new(
            MC_STATUS_IO_ERROR,
            format!("cannot read owner cache file {relative}: {error}"),
        )
    })?;
    if bytes.len() != expected_len {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!(
                "owner cache file {relative} has {} bytes, expected {expected_len}",
                bytes.len()
            ),
        ));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if actual_sha256 != expected_sha256 {
        return Err(AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            format!("owner cache file {relative} hash mismatch"),
        ));
    }
    Ok(bytes)
}

fn struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("ABI struct size fits u32")
}

fn string_length(value: &str) -> AbiResult<u32> {
    u32::try_from(value.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "string length exceeds u32"))
}

fn copy_string(value: &str, buffer: *mut u8, capacity: u32, out_length: *mut u32) -> AbiResult<()> {
    let payload_length = string_length(value)?;
    write_output(out_length, payload_length);
    let required = payload_length
        .checked_add(1)
        .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "string capacity overflow"))?;
    if capacity < required {
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("string buffer requires {required} bytes, got {capacity}"),
        ));
    }
    if buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "string output buffer is null",
        ));
    }
    // SAFETY: the caller supplied `capacity >= value.len() + 1` writable bytes.
    unsafe {
        buffer.copy_from_nonoverlapping(value.as_ptr(), value.len());
        *buffer.add(value.len()) = 0;
    }
    Ok(())
}

fn copy_slice<T: Copy>(
    values: &[T],
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> AbiResult<()> {
    let required = u32::try_from(values.len())
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "slice length exceeds u32"))?;
    if capacity < required {
        write_output(out_written, 0);
        return Err(AbiError::new(
            MC_STATUS_BUFFER_TOO_SMALL,
            format!("copy requires {required} elements, got {capacity}"),
        ));
    }
    if !values.is_empty() && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "copy output buffer is null",
        ));
    }
    // SAFETY: the caller supplied enough writable T elements and source and
    // destination cannot overlap across the FFI ownership boundary.
    unsafe { buffer.copy_from_nonoverlapping(values.as_ptr(), values.len()) };
    write_output(out_written, required);
    Ok(())
}

fn utf8_input(pointer: *const u8, length: u32, name: &str) -> AbiResult<String> {
    let length = usize::try_from(length)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{name} length exceeds usize")))?;
    if length > 0 && pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    if length == 0 {
        return Ok(String::new());
    }
    // SAFETY: the caller promises `length` readable bytes at the non-null
    // pointer for the duration of this call.
    let bytes = unsafe { slice::from_raw_parts(pointer, length) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|error| {
        AbiError::new(
            MC_STATUS_INVALID_UTF8,
            format!("{name} is not valid UTF-8: {error}"),
        )
    })
}

fn read_versioned_input<T: Copy>(pointer: *const T, name: &str) -> AbiResult<T> {
    if pointer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ));
    }
    // SAFETY: every versioned ABI input begins with two u32 fields. Reading
    // only that fixed header lets us reject short callers without first
    // reading the full current Rust structure.
    let bytes = pointer.cast::<u8>();
    let abi_version = unsafe { bytes.cast::<u32>().read_unaligned() };
    let declared_size = unsafe { bytes.add(size_of::<u32>()).cast::<u32>().read_unaligned() };
    if abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!("{name} ABI version {abi_version} is unsupported"),
        ));
    }
    let required = struct_size::<T>();
    if declared_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} struct_size {declared_size} is smaller than {required}"),
        ));
    }
    // SAFETY: after declaring at least the current prefix size, the caller is
    // required by the ABI contract to provide that readable prefix.
    Ok(unsafe { pointer.read_unaligned() })
}

fn require_output<T>(pointer: *mut T, name: &str) -> AbiResult<()> {
    if pointer.is_null() {
        Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{name} pointer is null"),
        ))
    } else {
        Ok(())
    }
}

fn require_struct_output<T>(pointer: *mut T, output_size: u32, name: &str) -> AbiResult<()> {
    require_output(pointer, name)?;
    let required = struct_size::<T>();
    if output_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {output_size} is smaller than {required}"),
        ));
    }
    Ok(())
}

fn require_element_size<T>(element_size: u32, name: &str) -> AbiResult<()> {
    let required = struct_size::<T>();
    if element_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!("{name} size {element_size} is smaller than {required}"),
        ));
    }
    if element_size > required {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("{name} size {element_size} must equal {required}"),
        ));
    }
    Ok(())
}

fn write_output<T>(pointer: *mut T, value: T) {
    // SAFETY: all callers validated the output pointer before reaching this
    // helper. Unaligned writes support conservative P/Invoke marshaling.
    unsafe { pointer.write_unaligned(value) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::FlightWorld;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[repr(C, align(8))]
    struct AlignedAbiHeader {
        abi_version: u32,
        struct_size: u32,
    }

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
    }

    fn open_fixture() -> u64 {
        let (package, closure) = fixture_paths();
        open_paths(&package, &closure).expect("package M opens")
    }

    fn open_fixture_with_terrain_frame_assets() -> u64 {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("package M loads");
        let mut session = SessionRecord::new(world).expect("fixture session constructs");
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        session.terrain_frame_assets = Some(Arc::new(TerrainFrameAssets {
            terrain_atlas: fs::read(
                private.join("generated/level1-unity-asset-cache/terrain/bl32n.atlas.indices.u8"),
            )
            .unwrap()
            .into_boxed_slice(),
            tablesn: fs::read(
                private.join("generated/level1-unity-asset-cache/palette/tablesn.u8"),
            )
            .unwrap()
            .into_boxed_slice(),
            sky: fs::read(private.join("generated/level1-unity-asset-cache/sky/skyn.indices.u8"))
                .unwrap()
                .into_boxed_slice(),
        }));
        with_registry_mut(|registry| registry.insert(session)).expect("fixture session registers")
    }

    fn open_paths(package: &Path, closure: &Path) -> Result<u64, i32> {
        let package = package.to_string_lossy();
        let closure = closure.to_string_lossy();
        let mut handle = 0_u64;
        let status = mc_native_session_open(
            package.as_ptr(),
            u32::try_from(package.len()).unwrap(),
            closure.as_ptr(),
            u32::try_from(closure.len()).unwrap(),
            &raw mut handle,
        );
        if status == MC_STATUS_OK {
            Ok(handle)
        } else {
            Err(status)
        }
    }

    fn valid_command() -> McCommandV2 {
        McCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCommandV2>(),
            requested_spell: MC_REQUESTED_SPELL_NONE,
            ..McCommandV2::default()
        }
    }

    fn valid_camera_input() -> McCameraInputV2 {
        McCameraInputV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McCameraInputV2>(),
            ..McCameraInputV2::default()
        }
    }

    #[test]
    fn layouts_and_versions_are_stable() {
        assert_eq!(mc_native_abi_version(), 2);
        assert_eq!(size_of::<McWorldInfoV2>(), 72);
        assert_eq!(size_of::<McCommandV2>(), 32);
        assert_eq!(size_of::<McCameraInputV2>(), 28);
        assert_eq!(size_of::<McSessionSnapshotV2>(), 120);
        assert_eq!(size_of::<McObjectiveSnapshotV2>(), 40);
        assert_eq!(size_of::<McEntitySnapshotV2>(), 52);
        assert_eq!(size_of::<McGoatSnapshotV2>(), 84);
        assert_eq!(size_of::<McGoatBatchV2>(), 64);
        assert_eq!(size_of::<McGoatDamageCommandV2>(), 40);
        assert_eq!(size_of::<McAssetRefInfoV2>(), 32);
        assert_eq!(size_of::<McEventQueryV2>(), 40);
        assert_eq!(size_of::<McEventBatchV2>(), 56);
        assert_eq!(size_of::<McEventV2>(), 72);
        assert_eq!(size_of::<McAudioCommandV2>(), 88);
        assert_eq!(size_of::<McPresentationStateV2>(), 64);
        assert_eq!(size_of::<McRenderCameraStateV2>(), 72);
        assert_eq!(size_of::<McTerrainPnt5VectorV2>(), 56);
        assert_eq!(size_of::<McTerrainGridVertexV2>(), 64);
        assert_eq!(size_of::<McTerrainFrameQueryV2>(), 40);
        assert_eq!(size_of::<McTerrainFrameInfoV2>(), 96);
        assert_eq!(std::mem::offset_of!(McWorldInfoV2, reset_state_hash), 40);
        assert_eq!(std::mem::offset_of!(McCommandV2, requested_spell), 14);
        assert_eq!(std::mem::offset_of!(McCameraInputV2, look_yaw), 8);
        assert_eq!(std::mem::offset_of!(McCameraInputV2, reserved), 12);
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, generation), 8);
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, entity_index), 48);
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, left_spell_holder),
            82
        );
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, objective_textbox_ticks),
            77
        );
        assert_eq!(
            std::mem::offset_of!(McSessionSnapshotV2, objectives_authority),
            79
        );
        assert_eq!(std::mem::offset_of!(McSessionSnapshotV2, reserved), 104);
        assert_eq!(
            std::mem::offset_of!(McObjectiveSnapshotV2, stage_selectors),
            13
        );
        assert_eq!(std::mem::offset_of!(McObjectiveSnapshotV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McEntitySnapshotV2, reserved), 36);
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, life), 48);
        assert_eq!(std::mem::offset_of!(McGoatSnapshotV2, reserved), 68);
        assert_eq!(std::mem::offset_of!(McGoatBatchV2, reserved), 48);
        assert_eq!(std::mem::offset_of!(McGoatDamageCommandV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McEventV2, state_hash), 24);
        assert_eq!(std::mem::offset_of!(McEventV2, reserved), 56);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, playback_rate_hz), 48);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, sound_id), 52);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, volume_q15), 60);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, play_type), 68);
        assert_eq!(std::mem::offset_of!(McAudioCommandV2, reserved), 72);
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, source_turn), 24);
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, camera_x), 28);
        assert_eq!(
            std::mem::offset_of!(McPresentationStateV2, camera_authority),
            42
        );
        assert_eq!(std::mem::offset_of!(McPresentationStateV2, reserved), 48);
        assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, history_x), 28);
        assert_eq!(
            std::mem::offset_of!(McRenderCameraStateV2, slope_target_x),
            42
        );
        assert_eq!(
            std::mem::offset_of!(McRenderCameraStateV2, render_offset_x),
            46
        );
        assert_eq!(std::mem::offset_of!(McRenderCameraStateV2, reserved), 56);
        assert_eq!(std::mem::offset_of!(McTerrainPnt5VectorV2, pnt5), 36);
        assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, view_x), 16);
        assert_eq!(
            std::mem::offset_of!(McTerrainGridVertexV2, feature_flags),
            44
        );
        assert_eq!(std::mem::offset_of!(McTerrainGridVertexV2, reserved), 48);
        assert_eq!(std::mem::offset_of!(McTerrainFrameQueryV2, reserved), 24);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, width), 24);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_bytes), 40);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, index_hash), 64);
        assert_eq!(std::mem::offset_of!(McTerrainFrameInfoV2, reserved), 88);
    }

    #[test]
    fn camera_history_preserves_raw_wrapping_yaw_until_render_mapping() {
        let mut state = FlightState {
            yaw: 2_040,
            ..FlightState::default()
        };
        let positive = CameraHistory::capture(&state, 0, 128, 32, 0);
        assert_eq!(positive.yaw, 2_072);
        assert_eq!(positive.yaw & 0x07ff, 24);

        state.yaw = 8;
        let negative = CameraHistory::capture(&state, 0, 128, -32, 0);
        assert_eq!(negative.yaw, -24);
        assert_eq!(negative.yaw & 0x07ff, 2_024);

        state.yaw = i16::MAX;
        let signed_overflow = CameraHistory::capture(&state, 0, 128, 1, 0);
        assert_eq!(signed_overflow.yaw, i16::MIN);
        assert_eq!(signed_overflow.yaw & 0x07ff, 0);
    }

    #[test]
    fn render_camera_sidecar_exposes_stateful_slope_offset_and_reset() {
        let handle = open_fixture();
        let read = || {
            let mut render = McRenderCameraStateV2::default();
            assert_eq!(
                mc_native_render_camera_state(
                    handle,
                    &raw mut render,
                    struct_size::<McRenderCameraStateV2>(),
                ),
                MC_STATUS_OK
            );
            render
        };
        let initial = read();
        let (initial_history, initial_slope) = with_session(handle, |session| {
            Ok((session.camera_history, session.camera_slope_sample))
        })
        .unwrap();
        assert_eq!(
            (
                initial.history_x,
                initial.history_y,
                initial.eye_z,
                initial.slope_target_x,
                initial.slope_target_y,
                initial.render_offset_x,
                initial.render_offset_y,
            ),
            (
                initial_history.x,
                initial_history.y,
                initial_history.z,
                initial_slope.target_x,
                initial_slope.target_y,
                initial_slope.offset_x,
                initial_slope.offset_y,
            )
        );
        assert_eq!(initial.authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
        assert_eq!(initial.flags, MC_RENDER_CAMERA_TERRAIN_SLOPE_XY);

        let command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let camera_input = valid_camera_input();
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera_input,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let advanced = read();
        let advanced_slope =
            with_session(handle, |session| Ok(session.camera_slope_sample)).unwrap();
        assert_eq!(
            (
                advanced.slope_target_x,
                advanced.slope_target_y,
                advanced.render_offset_x,
                advanced.render_offset_y,
            ),
            (
                advanced_slope.target_x,
                advanced_slope.target_y,
                advanced_slope.offset_x,
                advanced_slope.offset_y,
            )
        );

        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let reset = read();
        assert_eq!(
            (
                reset.history_x,
                reset.history_y,
                reset.eye_z,
                reset.slope_target_x,
                reset.slope_target_y,
                reset.render_offset_x,
                reset.render_offset_y,
            ),
            (
                initial.history_x,
                initial.history_y,
                initial.eye_z,
                initial.slope_target_x,
                initial.slope_target_y,
                initial.render_offset_x,
                initial.render_offset_y,
            ),
            "reset zeros the persistent accumulators before the initial draw update"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_is_the_pre_entity_update_history_record() {
        let handle = open_fixture();
        let (before_state, before_turn) =
            with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
        let command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let camera_input = McCameraInputV2 {
            look_yaw: 64,
            look_pitch: (-32_i16).cast_unsigned(),
            ..valid_camera_input()
        };
        let expected =
            CameraHistory::capture(&before_state, before_turn.wrapping_add(1), 128, 64, -32);
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const command,
                &raw const camera_input,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut presentation = McPresentationStateV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(presentation.tick, expected.tick);
        assert_eq!(presentation.source_turn, expected.source_turn);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
                presentation.camera_fov,
            ),
            (
                expected.x,
                expected.y,
                expected.z,
                expected.yaw,
                expected.pitch,
                expected.roll,
                expected.fov,
            )
        );
        assert_eq!(snapshot.tick, before_state.tick + 1);
        assert_ne!(
            (snapshot.x, snapshot.y, snapshot.z, snapshot.yaw),
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
                presentation.camera_yaw,
            ),
            "movement advances after the stored camera record"
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_history_tracks_consecutive_turns_and_reset() {
        let handle = open_fixture();
        let first_command = McCommandV2 {
            roll: 38,
            flags: 1,
            ..valid_command()
        };
        let first_camera = McCameraInputV2 {
            look_yaw: 64,
            look_pitch: (-32_i16).cast_unsigned(),
            ..valid_camera_input()
        };
        let mut first_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const first_command,
                &raw const first_camera,
                &raw mut first_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );

        let (before_second, before_second_turn) =
            with_session(handle, |session| Ok((session.state, session.source_turn))).unwrap();
        let second_command = McCommandV2 {
            roll: -19,
            flags: 1,
            ..valid_command()
        };
        let second_camera = McCameraInputV2 {
            look_yaw: 21,
            look_pitch: 9,
            ..valid_camera_input()
        };
        let expected_second = CameraHistory::capture(
            &before_second,
            before_second_turn.wrapping_add(1),
            128,
            21,
            9,
        );
        let mut second_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step_with_camera(
                handle,
                &raw const second_command,
                &raw const second_camera,
                &raw mut second_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut presentation = McPresentationStateV2::default();
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (
                presentation.tick,
                presentation.source_turn,
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
            ),
            (
                expected_second.tick,
                expected_second.source_turn,
                expected_second.x,
                expected_second.y,
                expected_second.z,
                expected_second.yaw,
                expected_second.pitch,
                expected_second.roll,
            )
        );
        assert_eq!(presentation.tick, first_snapshot.tick);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
            ),
            (first_snapshot.x, first_snapshot.y, first_snapshot.z)
        );
        assert_eq!(second_snapshot.tick, first_snapshot.tick + 1);

        let mut reset_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                handle,
                &raw mut reset_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_presentation_state(
                handle,
                &raw mut presentation,
                struct_size::<McPresentationStateV2>(),
            ),
            MC_STATUS_OK
        );
        let reset_history = with_session(handle, |session| Ok(session.camera_history)).unwrap();
        assert_eq!(presentation.generation, reset_snapshot.generation);
        assert_eq!(presentation.tick, reset_snapshot.tick);
        assert_eq!(
            (
                presentation.camera_x,
                presentation.camera_y,
                presentation.camera_z.wrapping_sub(128),
            ),
            (reset_snapshot.x, reset_snapshot.y, reset_snapshot.z)
        );
        assert_eq!(
            (
                presentation.source_turn,
                presentation.camera_yaw,
                presentation.camera_pitch,
                presentation.camera_roll,
                presentation.camera_fov,
            ),
            (
                reset_history.source_turn,
                reset_history.yaw,
                reset_history.pitch,
                reset_history.roll,
                reset_history.fov,
            )
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn presentation_camera_base_preserves_source_ground_clearance() {
        let handle = open_fixture();
        for turn in 0..192 {
            let command = McCommandV2 {
                roll: match turn % 4 {
                    0 => 38,
                    1 => 0,
                    2 => -38,
                    _ => 0,
                },
                flags: 1,
                ..valid_command()
            };
            let camera_input = valid_camera_input();
            let mut snapshot = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_step_with_camera(
                    handle,
                    &raw const command,
                    &raw const camera_input,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let mut presentation = McPresentationStateV2::default();
            assert_eq!(
                mc_native_presentation_state(
                    handle,
                    &raw mut presentation,
                    struct_size::<McPresentationStateV2>(),
                ),
                MC_STATUS_OK
            );
            let base = Position {
                x: presentation.camera_x,
                y: presentation.camera_y,
                z: presentation.camera_z.wrapping_sub(128),
            };
            let (ground, clearance) = with_session(handle, |session| {
                Ok((
                    session.world.ground_height(base),
                    session.world.profile().ground_clearance,
                ))
            })
            .unwrap();
            assert!(
                i32::from(base.z) >= i32::from(ground) + i32::from(clearance),
                "turn {turn}: camera base z={} is below ground {} + clearance {}",
                base.z,
                ground,
                clearance
            );
        }
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    #[cfg(not(windows))]
    fn c_header_compiles_and_locks_the_rust_layout() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mc-sim-header-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("layout.c");
        let object = directory.join("layout.o");
        fs::write(
            &source,
            r#"
#include <stddef.h>
#include "mc_sim.h"

_Static_assert(sizeof(McWorldInfoV2) == 72, "McWorldInfoV2 size");
_Static_assert(sizeof(McCommandV2) == 32, "McCommandV2 size");
_Static_assert(sizeof(McCameraInputV2) == 28, "McCameraInputV2 size");
_Static_assert(sizeof(McSessionSnapshotV2) == 120, "McSessionSnapshotV2 size");
_Static_assert(sizeof(McObjectiveSnapshotV2) == 40, "McObjectiveSnapshotV2 size");
_Static_assert(sizeof(McEntitySnapshotV2) == 52, "McEntitySnapshotV2 size");
_Static_assert(sizeof(McGoatSnapshotV2) == 84, "McGoatSnapshotV2 size");
_Static_assert(sizeof(McGoatBatchV2) == 64, "McGoatBatchV2 size");
_Static_assert(sizeof(McGoatDamageCommandV2) == 40, "McGoatDamageCommandV2 size");
_Static_assert(sizeof(McAssetRefInfoV2) == 32, "McAssetRefInfoV2 size");
_Static_assert(sizeof(McEventQueryV2) == 40, "McEventQueryV2 size");
_Static_assert(sizeof(McEventBatchV2) == 56, "McEventBatchV2 size");
_Static_assert(sizeof(McEventV2) == 72, "McEventV2 size");
_Static_assert(sizeof(McAudioCommandV2) == 88, "McAudioCommandV2 size");
_Static_assert(sizeof(McPresentationStateV2) == 64, "McPresentationStateV2 size");
_Static_assert(sizeof(McRenderCameraStateV2) == 72, "McRenderCameraStateV2 size");
_Static_assert(sizeof(McTerrainPnt5VectorV2) == 56, "McTerrainPnt5VectorV2 size");
_Static_assert(sizeof(McTerrainGridVertexV2) == 64, "McTerrainGridVertexV2 size");
_Static_assert(sizeof(McTerrainFrameQueryV2) == 40, "McTerrainFrameQueryV2 size");
_Static_assert(sizeof(McTerrainFrameInfoV2) == 96, "McTerrainFrameInfoV2 size");
_Static_assert(offsetof(McWorldInfoV2, reset_state_hash) == 40, "world hash offset");
_Static_assert(offsetof(McCommandV2, requested_spell) == 14, "spell offset");
_Static_assert(offsetof(McCameraInputV2, look_yaw) == 8, "camera yaw offset");
_Static_assert(offsetof(McCameraInputV2, reserved) == 12, "camera reserved offset");
_Static_assert(offsetof(McSessionSnapshotV2, entity_index) == 48, "entity offset");
_Static_assert(offsetof(McSessionSnapshotV2, objective_textbox_ticks) == 77, "objective ticks offset");
_Static_assert(offsetof(McSessionSnapshotV2, objectives_authority) == 79, "objective authority offset");
_Static_assert(offsetof(McSessionSnapshotV2, left_spell_holder) == 82, "holder offset");
_Static_assert(offsetof(McSessionSnapshotV2, reserved) == 104, "snapshot reserved offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, stage_selectors) == 13, "objective stages offset");
_Static_assert(offsetof(McObjectiveSnapshotV2, reserved) == 24, "objective reserved offset");
_Static_assert(offsetof(McEntitySnapshotV2, reserved) == 36, "entity reserved offset");
_Static_assert(offsetof(McGoatSnapshotV2, life) == 48, "Goat life offset");
_Static_assert(offsetof(McGoatSnapshotV2, reserved) == 68, "Goat reserved offset");
_Static_assert(offsetof(McGoatBatchV2, reserved) == 48, "Goat batch reserved offset");
_Static_assert(offsetof(McGoatDamageCommandV2, reserved) == 24, "Goat damage reserved offset");
_Static_assert(offsetof(McEventV2, state_hash) == 24, "event hash offset");
_Static_assert(offsetof(McEventV2, reserved) == 56, "event reserved offset");
_Static_assert(offsetof(McAudioCommandV2, playback_rate_hz) == 48, "audio rate offset");
_Static_assert(offsetof(McAudioCommandV2, sound_id) == 52, "audio sound offset");
_Static_assert(offsetof(McAudioCommandV2, volume_q15) == 60, "audio volume offset");
_Static_assert(offsetof(McAudioCommandV2, play_type) == 68, "audio type offset");
_Static_assert(offsetof(McAudioCommandV2, reserved) == 72, "audio reserved offset");
_Static_assert(offsetof(McPresentationStateV2, source_turn) == 24, "presentation turn offset");
_Static_assert(offsetof(McPresentationStateV2, camera_x) == 28, "presentation camera offset");
_Static_assert(offsetof(McPresentationStateV2, camera_authority) == 42, "presentation authority offset");
_Static_assert(offsetof(McPresentationStateV2, reserved) == 48, "presentation reserved offset");
_Static_assert(offsetof(McRenderCameraStateV2, history_x) == 28, "render camera history offset");
_Static_assert(offsetof(McRenderCameraStateV2, slope_target_x) == 42, "render camera slope target offset");
_Static_assert(offsetof(McRenderCameraStateV2, render_offset_x) == 46, "render camera offset");
_Static_assert(offsetof(McRenderCameraStateV2, reserved) == 56, "render camera reserved offset");
_Static_assert(offsetof(McTerrainPnt5VectorV2, pnt5) == 36, "terrain pnt5 offset");
_Static_assert(offsetof(McTerrainGridVertexV2, view_x) == 16, "terrain grid view offset");
_Static_assert(offsetof(McTerrainGridVertexV2, feature_flags) == 44, "terrain grid flags offset");
_Static_assert(offsetof(McTerrainGridVertexV2, reserved) == 48, "terrain grid reserved offset");
_Static_assert(offsetof(McTerrainFrameQueryV2, reserved) == 24, "terrain frame query reserved offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, width) == 24, "terrain frame width offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_bytes) == 40, "terrain frame bytes offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, index_hash) == 64, "terrain frame hash offset");
_Static_assert(offsetof(McTerrainFrameInfoV2, reserved) == 88, "terrain frame reserved offset");
"#,
        )
        .unwrap();
        let include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include");
        let output = std::process::Command::new("cc")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
            .arg(include)
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&directory);
        assert!(
            output.status.success(),
            "C header layout probe failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn terrain_pnt5_qualification_vectors_cross_the_typed_abi_exactly() {
        assert_eq!(
            mc_native_terrain_pnt5_vector_count(),
            TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT
        );
        let inputs = [
            (0, 4, 4, 0, 8, 128, 0),
            (0, 12, 4, 40, 8, 128, 0),
            (1, 0, 0, 40, 8, 128, 0),
            (1, 1, 1, 40, 8, 128, 14_745_600),
            (1, 2, 3, 40, 8, 128, 14_745_601),
            (1, 5, 7, 40, 8, 128, 19_202_048),
            (1, 255, 255, 40, 8, 128, 23_658_495),
            (1, 255, 255, 40, 8, 128, 23_658_496),
            (1, 255, 255, 40, 8, 128, 26_214_399),
            (1, 255, 255, 40, 8, 128, 26_214_400),
            (1, 42, 99, 56, 8, 128, 0),
            (65_535, 256, -1, 55, 8, 128, 14_757_945),
            (1, 0, 0, 40, 8, -256, 0),
        ];
        for (index, (turn, tile_x, tile_y, shade, angle, view_y, distance_squared)) in
            inputs.into_iter().enumerate()
        {
            let expected =
                outdoor_terrain_pnt5(turn, tile_x, tile_y, shade, angle, view_y, distance_squared);
            let mut vector = McTerrainPnt5VectorV2::default();
            assert_eq!(
                mc_native_terrain_pnt5_vector(
                    u32::try_from(index).unwrap(),
                    &raw mut vector,
                    struct_size::<McTerrainPnt5VectorV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(vector.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(
                (
                    vector.struct_size,
                    vector.turn,
                    vector.tile_x,
                    vector.tile_y,
                    vector.shade,
                    vector.angle,
                    vector.view_y,
                    vector.distance_squared,
                ),
                (
                    56,
                    turn,
                    tile_x,
                    tile_y,
                    shade,
                    angle,
                    view_y,
                    distance_squared,
                )
            );
            assert_eq!(vector.visible, u8::from(expected.visible));
            assert_eq!(vector.tablesn_row, expected.tablesn_row);
            assert_eq!(vector.wave, expected.wave);
            assert_eq!(vector.land_displacement, expected.land_displacement);
            assert_eq!(vector.pnt5, expected.pnt5);
            assert_eq!(vector.lighting_wave, expected.lighting_wave);
            assert_eq!(vector.reserved, [0; 2]);
        }
        let mut out_of_bounds = McTerrainPnt5VectorV2::default();
        assert_eq!(
            mc_native_terrain_pnt5_vector(
                TERRAIN_PNT5_QUALIFICATION_VECTOR_COUNT,
                &raw mut out_of_bounds,
                struct_size::<McTerrainPnt5VectorV2>(),
            ),
            MC_STATUS_BOUNDS
        );
    }

    #[test]
    fn owner_level_one_terrain_grid_crosses_the_bulk_abi_exactly() {
        let handle = open_fixture();
        assert_eq!(mc_native_terrain_grid_vertex_count(), 840);

        let mut too_small = vec![McTerrainGridVertexV2::default(); 839];
        let mut written = 99;
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                too_small.as_mut_ptr(),
                839,
                struct_size::<McTerrainGridVertexV2>(),
                &raw mut written,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(written, 0);

        let mut vertices = vec![McTerrainGridVertexV2::default(); 840];
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                vertices.as_mut_ptr(),
                840,
                struct_size::<McTerrainGridVertexV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 840);
        let direct = with_session(handle, |session| Ok(session.terrain_grid())).unwrap();
        assert_eq!(vertices.as_slice(), direct.as_slice());
        for (index, vertex) in vertices.iter().enumerate() {
            assert_eq!(vertex.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(vertex.struct_size, 64);
            assert_eq!(usize::from(vertex.index), index);
            assert_eq!(usize::from(vertex.row), index / 40);
            assert_eq!(usize::from(vertex.column), index % 40);
            assert_eq!(vertex.pnt5_written, u8::from(vertex.feature_flags & 2 == 0));
            if vertex.pnt5_written != 0 {
                assert_eq!(vertex.tablesn_row, ((vertex.pnt5 >> 16) & 0xff) as u8);
            }
            assert_eq!(vertex.reserved, [0; 4]);
        }
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn isolated_terrain_frame_crosses_generation_tick_checked_bulk_abi() {
        let handle = open_fixture_with_terrain_frame_assets();
        let mut camera = McRenderCameraStateV2::default();
        assert_eq!(
            mc_native_render_camera_state(
                handle,
                &raw mut camera,
                struct_size::<McRenderCameraStateV2>(),
            ),
            MC_STATUS_OK
        );
        let query = McTerrainFrameQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McTerrainFrameQueryV2>(),
            generation: camera.generation,
            tick: camera.tick,
            reserved: [0; 4],
        };
        let mut indices = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut coverage = vec![0_u8; LEVEL1_VIEW_PIXELS];
        let mut info = McTerrainFrameInfoV2::default();
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut info,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            (info.width, info.height, info.stride),
            (
                LEVEL1_VIEW_WIDTH as u16,
                LEVEL1_VIEW_HEIGHT as u16,
                LEVEL1_VIEW_WIDTH as u16,
            )
        );
        assert_eq!(
            (info.index_bytes, info.coverage_bytes),
            (LEVEL1_VIEW_PIXELS as u32, LEVEL1_VIEW_PIXELS as u32)
        );
        assert_eq!(
            info.flags,
            MC_TERRAIN_FRAME_ISOLATED_SKY_TERRAIN
                | MC_TERRAIN_FRAME_SKY_SEEDED
                | MC_TERRAIN_FRAME_HAS_COVERAGE
                | MC_TERRAIN_FRAME_NON_TERRAIN_OMITTED
        );
        assert_eq!(
            sim_core::terrain_frame::byte_hash(&indices),
            info.index_hash
        );
        assert_eq!(
            sim_core::terrain_frame::byte_hash(&coverage),
            info.coverage_hash
        );
        assert!(coverage.contains(&255));

        let first_indices = indices.clone();
        let first_coverage = coverage.clone();
        let mut repeated = McTerrainFrameInfoV2::default();
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(info, repeated);
        assert_eq!(indices, first_indices);
        assert_eq!(coverage, first_coverage);

        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const query,
                indices.as_mut_ptr(),
                (LEVEL1_VIEW_PIXELS - 1) as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        let stale_tick = McTerrainFrameQueryV2 {
            tick: query.tick.wrapping_add(1),
            ..query
        };
        assert_eq!(
            mc_native_session_terrain_frame_copy(
                handle,
                &raw const stale_tick,
                indices.as_mut_ptr(),
                indices.len() as u32,
                coverage.as_mut_ptr(),
                coverage.len() as u32,
                &raw mut repeated,
                struct_size::<McTerrainFrameInfoV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn positional_audio_qualification_vectors_cross_the_typed_sidecar_exactly() {
        assert_eq!(
            mc_native_audio_qualification_vector_count(),
            POSITIONAL_QUALIFICATION_VECTOR_COUNT
        );
        let expected_octants = [
            (43, 1_024, 30_036, 32_767, 0, 22_050),
            (37, 724, 30_560, 49_151, 0, 22_050),
            (39, 1_024, 29_126, 65_535, 0, 22_050),
            (37, 724, 29_678, 49_151, 0, 22_050),
            (37, 1_024, 27_305, 32_767, 0, 22_050),
            (37, 724, 29_678, 16_383, 0, 22_050),
            (37, 1_024, 29_126, 0, 0, 22_050),
            (37, 724, 30_560, 16_383, 0, 22_050),
        ];
        for (index, (sound, distance, volume, pan, rate_delta, rate_hz)) in
            expected_octants.into_iter().enumerate()
        {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    u32::try_from(index).unwrap(),
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(command.struct_size, 88);
            assert_eq!(command.sound_id, sound);
            assert_eq!(
                sim_core::positional_sound_command(
                    Position {
                        x: 32_768,
                        y: 32_768,
                        z: 1_000,
                    },
                    0,
                    Position {
                        x: u16::try_from(32_768_i32 + command.relative_x).unwrap(),
                        y: u16::try_from(32_768_i32 + command.relative_y).unwrap(),
                        z: i16::try_from(1_000_i32 + command.relative_z).unwrap(),
                    },
                    command.sound_id,
                    if index == 0 { 28 } else { 0 },
                    321,
                )
                .unwrap()
                .distance_xy,
                distance
            );
            assert_eq!(command.volume_q15, volume);
            assert_eq!(command.pan_q16, pan);
            assert_eq!(command.rate_delta_percent, rate_delta);
            assert_eq!(command.playback_rate_hz, rate_hz);
            assert_eq!(command.play_type, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING);
            assert_eq!(command.loop_count, 0);
            assert_eq!(command.action, MC_AUDIO_ACTION_START);
            assert_eq!(command.voice_index, 0);
            assert_eq!(command.reserved, [0; 4]);
        }

        for (index, expected_volume, expected_action) in [
            (10, 514, MC_AUDIO_ACTION_START),
            (11, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (12, 515, MC_AUDIO_ACTION_START),
            (13, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (14, 517, MC_AUDIO_ACTION_START),
            (15, 511, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (16, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
            (17, 0, MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME),
        ] {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    index,
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.volume_q15, expected_volume);
            assert_eq!(command.action, expected_action);
        }

        for (index, delta, hz) in [(18, -15, 18_742), (19, 0, 22_050), (20, 14, 25_137)] {
            let mut command = McAudioCommandV2::default();
            assert_eq!(
                mc_native_audio_qualification_vector(
                    index,
                    &raw mut command,
                    struct_size::<McAudioCommandV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(command.sound_flags, 0);
            assert_eq!(command.rate_delta_percent, delta);
            assert_eq!(command.playback_rate_hz, hz);
        }

        let mut duplicate = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                22,
                &raw mut duplicate,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(duplicate.sound_id, 43);
        assert_eq!(duplicate.action, MC_AUDIO_ACTION_SUPPRESS_EXISTING);
        assert_eq!(duplicate.voice_index, 0);

        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_qualification_vector(
                POSITIONAL_QUALIFICATION_VECTOR_COUNT,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_BOUNDS
        );
    }

    #[test]
    fn audio_sidecars_encode_original_loop_count_by_sound_class() {
        let handle = open_fixture();
        with_session_mut(handle, |session| {
            let entity_index = session.state.entity_index;
            let hash = state_hash(&session.state);
            for sound_id in [1, 2, 5, 31] {
                session.audio_commands.clear();
                session.audio_voices.fill(None);

                let start_event = session.encode_flight_event(
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                session.drain_pending_audio_commands();
                let start = session.audio_commands[&start_event.sequence];
                assert_eq!(start.loop_count, -1);
                assert_eq!(start.action, MC_AUDIO_ACTION_START);

                let update_event = session.encode_flight_event(
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                session.drain_pending_audio_commands();
                let update = session.audio_commands[&update_event.sequence];
                assert_eq!(update.loop_count, -1);
                assert_eq!(update.action, MC_AUDIO_ACTION_UPDATE_EXISTING);
                assert_eq!(update.voice_index, start.voice_index);

                let end_event = session.encode_flight_event(
                    FlightEvent::EndLoop {
                        entity_index,
                        player_index: -1,
                        sound_id,
                    },
                    hash,
                );
                let end = session.audio_commands[&end_event.sequence];
                assert_eq!(end.loop_count, -1);
                assert_eq!(end.action, MC_AUDIO_ACTION_END_LOOP);
                assert_eq!(end.voice_index, start.voice_index);
            }

            let synthetic_event = McEventV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McEventV2>(),
                ..McEventV2::default()
            };
            assert_eq!(
                local_audio_command_record(&synthetic_event, 29, 0, 3,).loop_count,
                0
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn session_audio_sidecar_is_keyed_to_event_generation_and_cleared_on_reset() {
        let handle = open_fixture();
        let snapshot = with_session_mut(handle, |session| {
            session.state.entity_phase = 64;
            session
                .context
                .effects
                .as_mut()
                .expect("session has effects")
                .periodic_spell_effect = Some(0x13);
            session.step(PlayerCommand::default());
            Ok(session.snapshot())
        })
        .unwrap();
        let sequence = with_session(handle, |session| {
            session
                .events
                .iter()
                .find(|event| event.kind == MC_EVENT_PREPARE_SOUND && event.code == 43)
                .map(|event| event.sequence)
                .ok_or_else(|| AbiError::new(MC_STATUS_BOUNDS, "missing positional event"))
        })
        .unwrap();
        let mut command = McAudioCommandV2::default();
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                snapshot.generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(command.event_sequence, sequence);
        assert_eq!(command.state_hash, snapshot.state_hash);
        assert_eq!(command.sound_id, 43);
        assert_eq!(command.entity_index, 152);
        assert_eq!(command.sound_flags, 0);
        assert_eq!(
            (command.relative_x, command.relative_y, command.relative_z),
            (0, 0, 0)
        );
        assert_eq!(command.volume_q15, 0x7fff);
        assert_eq!(command.pan_q16, 0x7fff);
        assert_eq!(command.loop_count, 0);
        assert_eq!(command.action, MC_AUDIO_ACTION_START);
        assert_eq!(command.voice_index, 1);
        with_session(handle, |session| {
            assert_eq!(
                session.audio_voices[0].map(|voice| voice.sound_id),
                Some(2),
                "the local ambient loop owns shared voice zero first"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation.wrapping_add(1),
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_native_audio_voice_complete(handle, snapshot.generation, 0, command.event_sequence,),
            MC_STATUS_BOUNDS
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence.wrapping_add(1),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_audio_voice_complete(
                handle,
                snapshot.generation,
                command.voice_index,
                command.event_sequence,
            ),
            MC_STATUS_BOUNDS
        );

        let old_generation = snapshot.generation;
        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                old_generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            mc_native_audio_command_for_event(
                handle,
                reset.generation,
                sequence,
                &raw mut command,
                struct_size::<McAudioCommandV2>(),
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn pending_slot_and_ten_voice_lifecycle_match_original_arbitration() {
        fn command(
            sequence: u64,
            sound_id: u16,
            sound_flags: i16,
            volume_q15: u16,
        ) -> McAudioCommandV2 {
            McAudioCommandV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: 88,
                generation: 1,
                event_sequence: sequence,
                sound_id,
                sound_flags,
                volume_q15,
                play_type: POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
                voice_index: MC_AUDIO_VOICE_NONE,
                ..McAudioCommandV2::default()
            }
        }

        let handle = open_fixture();
        with_session_mut(handle, |session| {
            session.audio_commands.insert(1, command(1, 37, 20, 1_000));
            session.audio_commands.insert(2, command(2, 37, 21, 992));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&1].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );
            assert_eq!(session.audio_commands[&2].action, MC_AUDIO_ACTION_START);
            assert_eq!(session.audio_commands[&2].voice_index, 0);

            session.audio_commands.insert(3, command(3, 39, 30, 1_000));
            session.audio_commands.insert(4, command(4, 39, 31, 991));
            session.drain_pending_audio_commands();
            assert_eq!(session.audio_commands[&3].action, MC_AUDIO_ACTION_START);
            assert_eq!(
                session.audio_commands[&4].action,
                MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED
            );

            for index in 2..AUDIO_VOICE_COUNT {
                let sequence = u64::try_from(index + 10).unwrap();
                let flags = i16::try_from(index + 100).unwrap();
                session
                    .audio_commands
                    .insert(sequence, command(sequence, 37, flags, 2_000));
                session.drain_pending_audio_commands();
                assert_eq!(
                    session.audio_commands[&sequence].voice_index,
                    u16::try_from(index).unwrap()
                );
            }
            assert!(session.audio_voices.iter().all(Option::is_some));

            let full_sequence = 100;
            session
                .audio_commands
                .insert(full_sequence, command(full_sequence, 37, 999, 2_000));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&full_sequence].action,
                MC_AUDIO_ACTION_SUPPRESS_NO_VOICE
            );
            assert_eq!(
                session.audio_commands[&full_sequence].voice_index,
                MC_AUDIO_VOICE_NONE
            );

            let duplicate_sequence = 101;
            session.audio_commands.insert(
                duplicate_sequence,
                command(duplicate_sequence, 37, 21, 2_000),
            );
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&duplicate_sequence].action,
                MC_AUDIO_ACTION_SUPPRESS_EXISTING
            );
            assert_eq!(session.audio_commands[&duplicate_sequence].voice_index, 0);

            session.audio_voices[0] = None;
            let reused_sequence = 102;
            session
                .audio_commands
                .insert(reused_sequence, command(reused_sequence, 37, 999, 2_000));
            session.drain_pending_audio_commands();
            assert_eq!(
                session.audio_commands[&reused_sequence].action,
                MC_AUDIO_ACTION_START
            );
            assert_eq!(session.audio_commands[&reused_sequence].voice_index, 0);

            session.reset();
            assert!(session.audio_voices.iter().all(Option::is_none));
            assert!(session.audio_commands.is_empty());
            Ok(())
        })
        .unwrap();
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn versioned_struct_boundaries_reject_short_buffers_and_bad_strides() {
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2 {
            abi_version: 77,
            ..McSessionSnapshotV2::default()
        };
        let unchanged = snapshot;
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>() - 1,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(snapshot, unchanged);

        let short_command_header = [MC_NATIVE_ABI_VERSION, 8];
        assert_eq!(
            mc_native_session_step(
                handle,
                short_command_header.as_ptr().cast::<McCommandV2>(),
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );

        let mut written = 99;
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McEntitySnapshotV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McEntitySnapshotV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                std::ptr::null_mut(),
                0,
                struct_size::<McTerrainGridVertexV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_session_terrain_grid_copy(
                handle,
                std::ptr::null_mut(),
                0,
                struct_size::<McTerrainGridVertexV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );

        let short_query_header = AlignedAbiHeader {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: 8,
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                (&raw const short_query_header).cast::<McEventQueryV2>(),
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn bounded_event_journal_reports_expired_cursors() {
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..=EVENT_CAPACITY {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: snapshot.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_CURSOR_EXPIRED
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn panic_is_contained_at_the_native_boundary() {
        assert_eq!(
            boundary(|| -> AbiResult<()> { panic!("intentional ABI containment probe") }),
            MC_STATUS_INTERNAL_PANIC
        );
        LAST_ERROR.with(|error| {
            assert!(error.borrow().contains("panic contained"));
        });
    }

    #[test]
    fn zero_capacity_event_read_preserves_the_cursor() {
        let handle = open_fixture();
        let mut command = valid_command();
        command.flags = 0x10;
        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: snapshot.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                std::ptr::null_mut(),
                0,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(batch.written, 0);
        assert_eq!(batch.next_cursor, query.cursor);
        assert_eq!(batch.has_more, 1);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn null_utf8_version_size_reserved_and_unknown_handle_are_distinct() {
        let mut handle = 0_u64;
        assert_eq!(
            mc_native_session_open(std::ptr::null(), 1, std::ptr::null(), 1, &raw mut handle),
            MC_STATUS_NULL_POINTER
        );
        let bad_utf8 = [0xff_u8];
        assert_eq!(
            mc_native_session_open(bad_utf8.as_ptr(), 1, bad_utf8.as_ptr(), 1, &raw mut handle),
            MC_STATUS_INVALID_UTF8
        );
        let handle = open_fixture();
        let mut snapshot = McSessionSnapshotV2::default();
        let mut command = valid_command();
        command.abi_version = 99;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_UNSUPPORTED_VERSION
        );
        command = valid_command();
        command.struct_size = 4;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        command = valid_command();
        command.reserved[0] = 1;
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_UNKNOWN_HANDLE
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_UNKNOWN_HANDLE);
    }

    #[test]
    fn package_m_world_reset_and_copy_surfaces_are_exact() {
        let handle = open_fixture();
        assert_ne!(handle, 0);
        let mut info = McWorldInfoV2::default();
        assert_eq!(
            mc_native_world_info(handle, &raw mut info, struct_size::<McWorldInfoV2>()),
            MC_STATUS_OK
        );
        assert_eq!((info.external_level, info.internal_level), (1, 0));
        assert_eq!(info.map_type, 1);
        assert_eq!(info.profile, MC_PROFILE_OUTDOOR);
        assert_eq!(info.entity_count, 1_000);
        assert_eq!(info.active_entity_count, 177);
        assert_eq!(info.player_count, 8);
        assert_eq!(info.asset_ref_count, 76);
        assert_eq!(info.reset_state_hash, 0x9da4_bc91_af58_f3ce);
        assert_eq!(info.reset_conformance_hash, 0x77b9_6b82_492d_6f4b);

        let mut snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(snapshot.state_hash, info.reset_state_hash);
        assert_eq!(
            snapshot.conformance_hash, 0x00cc_7b79_9d69_17fc,
            "session conformance includes the additive Level 1 effects journal"
        );
        assert_ne!(snapshot.conformance_hash, info.reset_conformance_hash);
        assert_eq!(
            (snapshot.x, snapshot.y, snapshot.z),
            (19_840, 56_960, 5_024)
        );
        assert_eq!(snapshot.objective_textbox_ticks, 33);
        assert_eq!(snapshot.objective_count, 8);
        assert_eq!(snapshot.objectives_authority, MC_AUTHORITY_FROZEN_OBSERVED);

        let mut count = 0;
        assert_eq!(
            mc_native_terrain_plane_length(handle, MC_TERRAIN_TYPES_U8, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 65_536);
        let mut too_small = vec![0_u8; 1];
        let mut written = 99;
        assert_eq!(
            mc_native_terrain_plane_copy_u8(
                handle,
                MC_TERRAIN_TYPES_U8,
                too_small.as_mut_ptr(),
                1,
                &raw mut written,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(written, 0);
        let mut terrain = vec![0_u8; 65_536];
        assert_eq!(
            mc_native_terrain_plane_copy_u8(
                handle,
                MC_TERRAIN_TYPES_U8,
                terrain.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 65_536);
        let mut entity_indices = vec![0_i16; 65_536];
        assert_eq!(
            mc_native_terrain_entity_indices_copy(
                handle,
                entity_indices.as_mut_ptr(),
                65_536,
                &raw mut written,
            ),
            MC_STATUS_OK
        );

        let mut entities = vec![McEntitySnapshotV2::default(); 1_000];
        assert_eq!(
            mc_native_entities_copy(
                handle,
                0,
                entities.as_mut_ptr(),
                1_000,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1_000);
        assert_eq!(entities[152].authority, MC_AUTHORITY_PLAYER_FLIGHT_LIVE);
        assert_eq!(entities[153].authority, MC_AUTHORITY_SPELL_ACTIVATION_LIVE);
        for index in LEVEL_ONE_GOAT_INDICES {
            assert_eq!(
                entities[usize::from(index)].authority,
                MC_AUTHORITY_GOAT_LIVE
            );
        }
        assert_eq!(
            entities
                .iter()
                .filter(|entity| entity.authority == MC_AUTHORITY_FROZEN_OBSERVED)
                .count(),
            142
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn goat_bulk_hash_damage_lifecycle_reset_and_session_isolation_are_atomic() {
        fn session_snapshot(handle: u64) -> McSessionSnapshotV2 {
            let mut snapshot = McSessionSnapshotV2::default();
            assert_eq!(
                mc_native_session_snapshot(
                    handle,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            snapshot
        }

        fn goats(handle: u64, generation: u64) -> (Vec<McGoatSnapshotV2>, McGoatBatchV2) {
            let mut rows = vec![McGoatSnapshotV2::default(); LEVEL_ONE_GOAT_INDICES.len()];
            let mut batch = McGoatBatchV2::default();
            assert_eq!(
                mc_native_goats_copy(
                    handle,
                    generation,
                    0,
                    rows.as_mut_ptr(),
                    u32::try_from(rows.len()).unwrap(),
                    struct_size::<McGoatSnapshotV2>(),
                    &raw mut batch,
                    struct_size::<McGoatBatchV2>(),
                ),
                MC_STATUS_OK
            );
            assert_eq!(batch.written, 33);
            assert_eq!(batch.total_count, 33);
            assert_eq!(batch.has_more, 0);
            (rows, batch)
        }

        let first = open_fixture();
        let second = open_fixture();
        let first_session = session_snapshot(first);
        let second_session = session_snapshot(second);
        let (reset_rows, reset_batch) = goats(first, first_session.generation);
        let (second_rows, second_batch) = goats(second, second_session.generation);
        assert_eq!(reset_rows, second_rows);
        assert_eq!(reset_batch.live_world_hash, second_batch.live_world_hash);
        assert_eq!(reset_batch.tick, 0);
        assert_eq!(reset_rows[0].entity_index, 50);
        assert_eq!(reset_rows[0].authority, MC_AUTHORITY_GOAT_LIVE);
        assert_eq!(reset_rows[0].render_parameter_index, 238);
        assert_eq!(reset_rows[0].base_tmap, 152);
        assert_eq!(reset_rows[0].life, 600);
        assert_eq!(reset_rows[0].max_life, 600);

        let mut stepped = McSessionSnapshotV2::default();
        let idle_command = valid_command();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const idle_command,
                &raw mut stepped,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let (_, advanced_batch) = goats(first, stepped.generation);
        let (_, untouched_batch) = goats(second, second_session.generation);
        assert_eq!(advanced_batch.tick, 1);
        assert_ne!(advanced_batch.live_world_hash, reset_batch.live_world_hash);
        assert_eq!(untouched_batch.live_world_hash, reset_batch.live_world_hash);

        let damage = McGoatDamageCommandV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McGoatDamageCommandV2>(),
            generation: stepped.generation,
            entity_index: 50,
            reserved_0: 0,
            damage: 601,
            reserved: [0; 4],
        };
        let mut damage_batch = McGoatBatchV2::default();
        let mut invalid_damage = damage;
        invalid_damage.reserved[0] = 1;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.reserved_0 = 1;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_RESERVED_NONZERO
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.damage = 0;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.entity_index = 999;
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        invalid_damage = damage;
        invalid_damage.generation = invalid_damage.generation.wrapping_add(1);
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const invalid_damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(
            goats(first, stepped.generation).1.live_world_hash,
            advanced_batch.live_world_hash
        );
        assert_eq!(
            mc_native_session_goat_inject_damage(
                first,
                &raw const damage,
                &raw mut damage_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(damage_batch.live_world_hash, advanced_batch.live_world_hash);

        for _ in 0..12 {
            assert_eq!(
                mc_native_session_step(
                    first,
                    &raw const idle_command,
                    &raw mut stepped,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
            let (rows, _) = goats(first, stepped.generation);
            if rows[0].active == 0 {
                break;
            }
            if rows[0].action == 12 {
                let kill_credit = with_session(first, |session| {
                    Ok(session.live_world.goats()[0].kill_credit_entity)
                })
                .unwrap();
                assert_eq!(
                    kill_credit,
                    i16::try_from(stepped.entity_index).unwrap(),
                    "qualification damage is always attributed to the local player"
                );
            }
        }
        let (dead_rows, _) = goats(first, stepped.generation);
        assert!(
            dead_rows[0].active == 0,
            "Goat must reach next-pass reclamation"
        );

        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: stepped.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 256];
        let mut event_batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                first,
                &raw const query,
                events.as_mut_ptr(),
                u32::try_from(events.len()).unwrap(),
                struct_size::<McEventV2>(),
                &raw mut event_batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        events.truncate(usize::try_from(event_batch.written).unwrap());
        assert!(events.iter().any(|event| {
            event.kind == MC_EVENT_GOAT_MANA_CONVERTED
                && event.entity_index == 50
                && event.value_0 == 300
        }));
        assert!(events.iter().any(|event| {
            event.kind == MC_EVENT_GOAT_REMOVAL_QUEUED && event.entity_index == 50
        }));
        assert!(
            events
                .iter()
                .any(|event| { event.kind == MC_EVENT_GOAT_RECLAIMED && event.entity_index == 50 })
        );

        let mut reset_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(
                first,
                &raw mut reset_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut rejected_batch = McGoatBatchV2::default();
        assert_eq!(
            mc_native_goats_copy(
                first,
                stepped.generation,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McGoatSnapshotV2>(),
                &raw mut rejected_batch,
                struct_size::<McGoatBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        let (restored_rows, restored_batch) = goats(first, reset_snapshot.generation);
        assert_eq!(restored_rows, reset_rows);
        assert_eq!(restored_batch.live_world_hash, reset_batch.live_world_hash);
        assert_eq!(restored_batch.tick, 0);

        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn objective_catalog_is_complete_frozen_and_bounds_checked() {
        let handle = open_fixture();
        let mut count = 0;
        assert_eq!(
            mc_native_objective_count(handle, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 8);

        let mut objectives = [McObjectiveSnapshotV2::default(); 8];
        let mut written = 0;
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                objectives.as_mut_ptr(),
                8,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 8);
        for (slot, objective) in objectives.iter().enumerate() {
            assert_eq!(usize::from(objective.slot), slot);
            assert_eq!(objective.authority, MC_AUTHORITY_FROZEN_OBSERVED);
            assert_eq!(objective.abi_version, MC_NATIVE_ABI_VERSION);
            assert_eq!(
                objective.struct_size,
                struct_size::<McObjectiveSnapshotV2>()
            );
        }
        assert_eq!(
            (
                objectives[0].is_level_end,
                objectives[0].text_selector,
                objectives[0].done,
            ),
            (0, 0, 0)
        );
        assert_eq!(objectives[0].stage_selectors, [1, 1, 1, 1, 1, 0, 0, 0]);

        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>() - 1,
                &raw mut written,
            ),
            MC_STATUS_STRUCT_TOO_SMALL
        );
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                0,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>() + 1,
                &raw mut written,
            ),
            MC_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            mc_native_objectives_copy(
                handle,
                9,
                std::ptr::null_mut(),
                0,
                struct_size::<McObjectiveSnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn two_sessions_are_independent_and_repeat_deterministically() {
        let first = open_fixture();
        let second = open_fixture();
        assert_ne!(first, second);
        let mut command = valid_command();
        command.flags = 1;
        let mut first_snapshot = McSessionSnapshotV2::default();
        let mut second_snapshot = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                first,
                &raw const command,
                &raw mut first_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_session_snapshot(
                second,
                &raw mut second_snapshot,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert_ne!(first_snapshot.state_hash, second_snapshot.state_hash);
        let mut player_entity = McEntitySnapshotV2::default();
        let mut written = 0;
        assert_eq!(
            mc_native_entities_copy(
                first,
                u32::from(first_snapshot.entity_index),
                &raw mut player_entity,
                1,
                struct_size::<McEntitySnapshotV2>(),
                &raw mut written,
            ),
            MC_STATUS_OK
        );
        assert_eq!(written, 1);
        assert_eq!(
            (player_entity.x, player_entity.y, player_entity.z),
            (first_snapshot.x, first_snapshot.y, first_snapshot.z)
        );
        assert_eq!(player_entity.yaw, first_snapshot.yaw);
        assert_eq!(player_entity.pitch, first_snapshot.display_pitch);
        assert_eq!(player_entity.actual_speed, first_snapshot.actual_speed);
        assert_eq!(player_entity.phase, first_snapshot.phase);
        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(first, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(reset.state_hash, second_snapshot.state_hash);
        assert_ne!(reset.generation, second_snapshot.generation);
        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn source_projection_hash_normalizes_only_the_additive_session_tick() {
        let handle = open_fixture();
        let command = valid_command();
        let mut snapshot = McSessionSnapshotV2::default();
        for _ in 0..8 {
            assert_eq!(
                mc_native_session_step(
                    handle,
                    &raw const command,
                    &raw mut snapshot,
                    struct_size::<McSessionSnapshotV2>(),
                ),
                MC_STATUS_OK
            );
        }
        let mut projected_hash = 0_u64;
        assert_eq!(
            mc_native_session_source_projection_hash(handle, &raw mut projected_hash),
            MC_STATUS_OK
        );
        assert_eq!(snapshot.state_hash, 0x061b_7ffc_8213_1a7e);
        assert_eq!(projected_hash, 0x0e45_47b4_2a98_c976);
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn step_event_order_and_copy_since_survive_reset_generation() {
        let handle = open_fixture();
        let mut before = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_snapshot(
                handle,
                &raw mut before,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        let mut command = valid_command();
        command.flags = 0x10;
        let mut after = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_step(
                handle,
                &raw const command,
                &raw mut after,
                struct_size::<McSessionSnapshotV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(after.event_head >= 2);
        let query = McEventQueryV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McEventQueryV2>(),
            generation: after.generation,
            cursor: 0,
            reserved: [0; 4],
        };
        let mut events = vec![McEventV2::default(); 32];
        let mut batch = McEventBatchV2::default();
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert!(batch.written >= 2);
        let written = usize::try_from(batch.written).unwrap();
        for pair in events[..written].windows(2) {
            assert_eq!(pair[1].sequence, pair[0].sequence + 1);
        }
        assert!(
            events[..written]
                .iter()
                .any(|event| event.kind == MC_EVENT_REQUEST_SPELL_ACTIVATION)
        );
        assert!(
            events[..written - 1]
                .iter()
                .any(|event| event.kind == MC_EVENT_MOVEMENT_RESOLUTION)
        );
        let music_start = events[written - 1];
        assert_eq!(music_start.kind, MC_EVENT_START_MUSIC);
        assert_eq!(music_start.code, 1);
        assert_eq!(music_start.value_0, 0);
        assert_eq!(music_start.value_1, 1);

        let next_query = McEventQueryV2 {
            cursor: batch.next_cursor,
            ..query
        };
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const next_query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_OK
        );
        assert_eq!(batch.written, 0);

        let mut reset = McSessionSnapshotV2::default();
        assert_eq!(
            mc_native_session_reset(handle, &raw mut reset, struct_size::<McSessionSnapshotV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(
            mc_native_events_copy_since(
                handle,
                &raw const query,
                events.as_mut_ptr(),
                32,
                struct_size::<McEventV2>(),
                &raw mut batch,
                struct_size::<McEventBatchV2>(),
            ),
            MC_STATUS_GENERATION_MISMATCH
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn level_one_music_starts_once_after_the_second_source_pass_and_rearms_on_reset() {
        let first = open_fixture();
        let second = open_fixture();

        for handle in [first, second] {
            with_session_mut(handle, |session| {
                assert_eq!(session.source_turn, 1);
                assert!(session.music_start_armed);
                assert!(
                    !session
                        .events
                        .iter()
                        .any(|event| event.kind == MC_EVENT_START_MUSIC)
                );
                session.step(PlayerCommand::default());
                assert_eq!(session.source_turn, 2);
                let starts = session
                    .events
                    .iter()
                    .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                    .collect::<Vec<_>>();
                assert_eq!(starts.len(), 1);
                let start = starts[0];
                assert_eq!(start.code, 1);
                assert_eq!(start.value_0, 0);
                assert_eq!(start.value_1, 1);
                assert_eq!(start.asset_ref_index, MC_ASSET_REF_NONE);
                assert_eq!(start.generation, 1);
                assert_eq!(session.events.back().unwrap().kind, MC_EVENT_START_MUSIC);
                assert!(!session.music_start_armed);
                session.step(PlayerCommand::default());
                assert_eq!(
                    session
                        .events
                        .iter()
                        .filter(|event| event.kind == MC_EVENT_START_MUSIC)
                        .count(),
                    1
                );
                Ok(())
            })
            .unwrap();
        }

        with_session_mut(first, |session| {
            session.reset();
            assert_eq!(session.generation, 2);
            assert!(session.music_start_armed);
            assert!(session.events.is_empty());
            session.step(PlayerCommand::default());
            let start = session
                .events
                .iter()
                .find(|event| event.kind == MC_EVENT_START_MUSIC)
                .unwrap();
            assert_eq!(start.generation, 2);
            assert_eq!(start.value_1, 1);
            Ok(())
        })
        .unwrap();

        assert_eq!(mc_native_session_destroy(first), MC_STATUS_OK);
        assert_eq!(mc_native_session_destroy(second), MC_STATUS_OK);
    }

    #[test]
    fn asset_strings_last_error_and_bounds_have_copy_contracts() {
        let handle = open_fixture();
        let mut count = 0;
        assert_eq!(
            mc_native_asset_ref_count(handle, &raw mut count),
            MC_STATUS_OK
        );
        assert_eq!(count, 76);
        let mut info = McAssetRefInfoV2::default();
        assert_eq!(
            mc_native_asset_ref_info(handle, 0, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
            MC_STATUS_OK
        );
        assert_eq!(info.asset_index, 0);
        let mut length = 0;
        assert_eq!(
            mc_native_asset_string_length(
                handle,
                0,
                MC_ASSET_STRING_CONTENT_ADDRESS,
                0,
                &raw mut length,
            ),
            MC_STATUS_OK
        );
        assert_eq!(length, 71);
        let mut tiny = [0_u8; 1];
        assert_eq!(
            mc_native_asset_string_copy(
                handle,
                0,
                MC_ASSET_STRING_CONTENT_ADDRESS,
                0,
                tiny.as_mut_ptr(),
                1,
                &raw mut length,
            ),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        let mut error_length = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut error_length),
            MC_STATUS_OK
        );
        assert!(error_length > 0);
        let mut error = vec![0_u8; usize::try_from(error_length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                error.as_mut_ptr(),
                u32::try_from(error.len()).unwrap(),
                &raw mut error_length,
            ),
            MC_STATUS_OK
        );
        assert!(
            str::from_utf8(&error[..usize::try_from(error_length).unwrap()])
                .unwrap()
                .contains("requires")
        );
        assert_eq!(
            mc_native_asset_ref_info(handle, 76, &raw mut info, struct_size::<McAssetRefInfoV2>(),),
            MC_STATUS_BOUNDS
        );
        assert_eq!(mc_native_session_destroy(handle), MC_STATUS_OK);
    }

    #[test]
    fn last_error_copy_can_resize_and_retry_without_losing_the_diagnostic() {
        let mut info = McWorldInfoV2::default();
        assert_eq!(
            mc_native_world_info(u64::MAX, &raw mut info, struct_size::<McWorldInfoV2>(),),
            MC_STATUS_UNKNOWN_HANDLE
        );
        let mut original_length = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut original_length),
            MC_STATUS_OK
        );
        assert!(original_length > 0);

        let mut tiny = [0_u8; 1];
        let mut reported_length = 0;
        assert_eq!(
            mc_native_last_error_copy(tiny.as_mut_ptr(), 1, &raw mut reported_length),
            MC_STATUS_BUFFER_TOO_SMALL
        );
        assert_eq!(reported_length, original_length);

        assert_eq!(
            mc_native_last_error_copy(
                std::ptr::null_mut(),
                original_length + 1,
                &raw mut reported_length,
            ),
            MC_STATUS_NULL_POINTER
        );
        assert_eq!(reported_length, original_length);

        let mut length_after_failure = 0;
        assert_eq!(
            mc_native_last_error_length(&raw mut length_after_failure),
            MC_STATUS_OK
        );
        assert_eq!(length_after_failure, original_length);
        let mut message = vec![0_u8; usize::try_from(original_length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                message.as_mut_ptr(),
                u32::try_from(message.len()).unwrap(),
                &raw mut reported_length,
            ),
            MC_STATUS_OK
        );
        assert_eq!(reported_length, original_length);
        let message =
            str::from_utf8(&message[..usize::try_from(reported_length).unwrap()]).unwrap();
        assert!(message.contains("unknown session handle"));
        assert!(!message.contains("requires"));
    }

    #[test]
    fn corrupt_package_propagates_validation_status_and_useful_error() {
        let (package, closure) = fixture_paths();
        let temp = TempPackage::copy_from(&package, &closure);
        let path = temp.package.join("entities.bin");
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 0xff;
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            open_paths(&temp.package, &temp.closure),
            Err(MC_STATUS_VALIDATION_FAILED)
        );
        let mut length = 0;
        assert_eq!(mc_native_last_error_length(&raw mut length), MC_STATUS_OK);
        let mut error = vec![0_u8; usize::try_from(length).unwrap() + 1];
        assert_eq!(
            mc_native_last_error_copy(
                error.as_mut_ptr(),
                u32::try_from(error.len()).unwrap(),
                &raw mut length,
            ),
            MC_STATUS_OK
        );
        let error = str::from_utf8(&error[..usize::try_from(length).unwrap()]).unwrap();
        assert!(error.contains("hash mismatch"));
    }

    struct TempPackage {
        root: PathBuf,
        package: PathBuf,
        closure: PathBuf,
    }

    impl TempPackage {
        fn copy_from(package: &Path, closure: &Path) -> Self {
            let root = std::env::temp_dir().join(format!(
                "mc2-capi-v2-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let output = root.join("package");
            fs::create_dir_all(&output).unwrap();
            fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
            for name in [
                "authored-level.bin",
                "entities.bin",
                "players.bin",
                "presentation.bin",
                "runtime.bin",
                "terrain.bin",
            ] {
                fs::copy(package.join(name), output.join(name)).unwrap();
            }
            let closure_output = root.join("closure.json");
            fs::copy(closure, &closure_output).unwrap();
            Self {
                root,
                package: output,
                closure: closure_output,
            }
        }
    }

    impl Drop for TempPackage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos()
    }
}
