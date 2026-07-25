//! Strict owner-package loader for an immutable native simulation world.
//!
//! The loader consumes an explicit level-export directory and an explicit
//! asset-closure path. It never searches for owner data, embeds regression
//! bytes, or treats observed entities as simulated entities.

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Component, Path},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    FlightContext, FlightEffectsContext, FlightProfile, FlightState, FlightWorld, PlayerCommand,
    Position, SpellFireContext, conformance_hash,
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_RENDER_PARAMETER_INDEX, GOAT_STAGE_POINT,
        GoatBehaviorConfig, GoatPlayerTarget, LEVEL_ONE_GOAT_INDICES, LiveWorldState,
    },
    legacy_terrain::{LEGACY_MAP_CELLS, LegacyTerrain},
    spell_activation::{SpellActivationHolder, SpellCasterState},
    state_hash, step_outer_turn_with_context,
};

const MANIFEST_SCHEMA: &str = "mc2-owner-level-export/v2";
const CLOSURE_SCHEMA: &str = "mc2-level-asset-closure/v2";
const AUDIO_CACHE_SCHEMA: &str = "mc2-owner-unity-asset-cache/v1";
const AUDIO_CACHE_MANIFEST_SHA256: &str =
    "2c3845fff2b35dea336662de9d636e5c620271eeea4ac19a6acb898f62714d51";
const AUDIO_CACHE_CONTENT_SHA256: &str =
    "a5fdd9961ebd61566b0230e2dc1b468219dae74536279b51d89d9c2d6cc0ce81";
const RESET_SCHEMA: &str = "mc2-flight-reset-projection/v1";
const ENTITY_COUNT: usize = 1_000;
const ENTITY_COUNT_U32: u32 = 1_000;
const ENTITY_RECORD_BYTES: usize = 176;
const ENTITY_RECORD_BYTES_U16: u16 = 176;
const PLAYER_COUNT: usize = 8;
const PLAYER_COUNT_I16: i16 = 8;
const PLAYER_COUNT_U32: u32 = 8;
const PLAYER_RECORD_BYTES: usize = 573;
const PLAYER_RECORD_BYTES_U16: u16 = 573;
const AUTHORED_LEVEL_BYTES: usize = 0x6604;
const TERRAIN_BYTES: usize = LEGACY_MAP_CELLS * 7;
const RUNTIME_BYTES: usize = 2_754;
const PARTICLE_COUNT: usize = 347;
const PARTICLE_COUNT_U32: u32 = 347;
const PARTICLE_RECORD_BYTES: usize = 14;
const LEVEL_SPRITE_SELECTOR_COUNT: usize = 504;
const LEVEL_SPRITE_SELECTOR_COUNT_U32: u32 = 504;
const ENTITY_PARAMETER_COUNT: usize = 157;
const ENTITY_PARAMETER_COUNT_U32: u32 = 157;
const ENTITY_PARAMETER_BYTES: usize = 34;
const PLAYER_SPELL_COUNT: usize = 26;
const PLAYER_SPELL_COUNT_I16: i16 = 26;
const FRAME_ASSET_COUNT: usize = 76;
const LEVEL_ONE_CONTROL_MODE_SEAM_DEFAULT: u8 = 0;

const PACKAGE_MEMBERS: [(&str, usize); 6] = [
    ("authored-level.bin", AUTHORED_LEVEL_BYTES),
    ("entities.bin", 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES),
    ("players.bin", 12 + PLAYER_COUNT * PLAYER_RECORD_BYTES),
    ("presentation.bin", 10_720),
    ("runtime.bin", RUNTIME_BYTES),
    ("terrain.bin", TERRAIN_BYTES),
];

/// A fail-closed package or closure validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeWorldError(String);

impl NativeWorldError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NativeWorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NativeWorldError {}

type Result<T> = std::result::Result<T, NativeWorldError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapType {
    Day,
    Night,
    Cave,
}

impl MapType {
    fn decode(id: u8, name: &str) -> Result<Self> {
        match (id, name) {
            (0, "Day") => Ok(Self::Day),
            (1, "Night") => Ok(Self::Night),
            (2, "Cave") => Ok(Self::Cave),
            _ => Err(NativeWorldError::new(format!(
                "unsupported or inconsistent map type {id}/{name}"
            ))),
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        match self {
            Self::Day => 0,
            Self::Night => 1,
            Self::Cave => 2,
        }
    }

    #[must_use]
    pub const fn profile(self) -> FlightProfile {
        match self {
            Self::Day | Self::Night => FlightProfile::OUTDOOR,
            Self::Cave => FlightProfile::CAVE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelMetadata {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: MapType,
    pub capture_site: String,
    pub capture_frame: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAuthority {
    Inactive,
    FrozenObserved,
    PlayerFlightLive,
    SpellActivationLive,
    GoatLive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub index: u16,
    /// Original `id_0x1A_26`, used as the sample arbitration flag for
    /// positional sound IDs 37 and 39. This is distinct from the pool slot.
    pub source_id: u16,
    pub authority: EntityAuthority,
    pub active: bool,
    pub next_relation: Option<u16>,
    pub class: u8,
    pub model: u8,
    pub control_flags: u32,
    pub action: u8,
    pub action_marker: i8,
    pub level_gate: i8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub phase: u8,
    pub active_value: i16,
    pub activation_value: u16,
    pub actual_speed: i16,
    pub mana_cost: i32,
    pub mana: i32,
    pub parameter_relation: Option<u16>,
    pub special_relation: EntitySpecialRelation,
    record: Box<[u8]>,
}

impl EntitySnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitySpecialRelation {
    None,
    PlayerState(u8),
    GlobalSpecialState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerSnapshot {
    pub slot: u8,
    pub is_ai: bool,
    pub entity_index: u16,
    pub active_player_index: i16,
    pub turn: i32,
    pub left_spell_slot: u8,
    pub right_spell_slot: u8,
    pub enabled_spell_holders: [u16; PLAYER_SPELL_COUNT],
    record: Box<[u8]>,
}

impl PlayerSnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainPlanes {
    pub terrain_types: Box<[u8]>,
    pub ground_heights: Box<[u8]>,
    pub shading: Box<[u8]>,
    pub angles: Box<[u8]>,
    pub secondary_heights: Box<[u8]>,
    pub entity_indices: Box<[i16]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRelation {
    Scalar(u32),
    AuthoredFeature(u16),
    LiveEntity(u16),
    LiveEntityIndex(u16),
    AxisScalar(u32),
    Null,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStage {
    pub kind: i8,
    pub flags: i8,
    pub axis_x: i16,
    pub axis_y: i16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStageVariable {
    pub kind: i8,
    pub flags: i8,
    pub word_2: u16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub global_turn: u32,
    pub random_state: u32,
    pub local_player_slot: u8,
    pub player_count: u8,
    pub maptype_music: i32,
    pub stage_index: u8,
    /// Remaining objective-textbox presentation ticks. The atlas identifies
    /// source byte `byte_counter_current_objective_box_0x36E04` as a countdown,
    /// not an objective row selector.
    pub objective_textbox_ticks: u8,
    pub stages: Box<[RuntimeStage]>,
    pub stage_variables: Box<[RuntimeStageVariable]>,
    pub objectives: Box<[RuntimeObjective]>,
    pub player_inputs: [PlayerCommand; PLAYER_COUNT],
    bytes: Box<[u8]>,
}

impl RuntimeSnapshot {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeObjective {
    pub is_level_end: u8,
    pub text_selector: u8,
    pub done: u8,
    pub stage_selectors: [u8; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticleParameter {
    pub base_tmap: u16,
    pub word_2: u16,
    pub word_4: u16,
    pub speed: u16,
    pub rotation_speed: u16,
    pub byte_10: u8,
    pub byte_11: i8,
    pub render_mode: u8,
    pub byte_13: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSelectors {
    pub particles: Box<[ParticleParameter]>,
    pub level_sprite_selectors: Box<[u8]>,
    pub entity_parameters: Box<[[u8; ENTITY_PARAMETER_BYTES]]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameObservation {
    pub ordinal: u16,
    pub category: String,
    pub hook: String,
    pub stage: Option<String>,
    pub selector: Value,
    pub event_sha256: String,
    pub event: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    pub asset_id: String,
    pub content_address: String,
    pub owner_asset_ids: Box<[String]>,
    pub observation: FrameObservation,
}

/// An immutable, validated owner-data world.
#[derive(Clone, Debug)]
pub struct NativeWorld {
    metadata: LevelMetadata,
    package_sha256: String,
    closure_sha256: String,
    profile: FlightProfile,
    terrain_planes: TerrainPlanes,
    terrain: LegacyTerrain,
    entities: Box<[EntitySnapshot]>,
    players: Box<[PlayerSnapshot]>,
    runtime: RuntimeSnapshot,
    presentation: PresentationSelectors,
    asset_refs: Box<[AssetRef]>,
    sound_asset_refs: BTreeMap<u16, u32>,
    music_asset_ref: Option<u32>,
    reset_state: FlightState,
    reset_context: FlightContext,
    goat_stage_point: Position,
    goat_config: GoatBehaviorConfig,
    reset_live_world: LiveWorldState,
}

impl NativeWorld {
    /// Load and validate an export package and its separately supplied closure.
    ///
    /// # Errors
    ///
    /// Returns an error on any schema, identity, size, version, relation,
    /// reset, closure-binding, or frame-resolution mismatch.
    pub fn load(package_dir: impl AsRef<Path>, closure_path: impl AsRef<Path>) -> Result<Self> {
        let package_dir = package_dir.as_ref();
        let manifest_path = package_dir.join("manifest.json");
        let manifest_bytes = read_file(&manifest_path)?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid manifest JSON: {error}")))?;
        let object = as_object(&manifest, "manifest")?;
        require_string(object, "schema", "manifest")?
            .eq(MANIFEST_SCHEMA)
            .then_some(())
            .ok_or_else(|| {
                NativeWorldError::new("manifest schema must be mc2-owner-level-export/v2")
            })?;
        require_string(object, "byte_order", "manifest")?
            .eq("little-endian")
            .then_some(())
            .ok_or_else(|| NativeWorldError::new("manifest byte_order must be little-endian"))?;

        let members = validate_package(package_dir, object)?;
        let package_sha256 = require_hash_string(object, "package_sha256", "manifest")?.to_owned();
        let metadata = parse_metadata(object)?;
        let counts = require_object(object, "counts", "manifest")?;
        validate_manifest_counts(counts)?;
        validate_manifest_layouts(object)?;

        let authored = member(&members, "authored-level.bin")?;
        if authored[6] != metadata.map_type.id() {
            return Err(NativeWorldError::new(
                "authored-level.bin map type does not match manifest",
            ));
        }

        let terrain_planes = parse_terrain(member(&members, "terrain.bin")?)?;
        let terrain = LegacyTerrain::new(
            terrain_planes.terrain_types.to_vec(),
            terrain_planes.ground_heights.to_vec(),
            (metadata.map_type == MapType::Cave).then(|| terrain_planes.secondary_heights.to_vec()),
            terrain_planes.angles.to_vec(),
        )
        .map_err(NativeWorldError::new)?;

        let mut entities = parse_entities(member(&members, "entities.bin")?)?;
        let players = parse_players(member(&members, "players.bin")?)?;
        let runtime = parse_runtime(member(&members, "runtime.bin")?)?;
        let presentation = parse_presentation(member(&members, "presentation.bin")?)?;
        validate_manifest_relations(counts, &entities, &players, &runtime, &presentation)?;

        let reset_state = parse_reset_state(
            require_object(object, "reset_projection", "manifest")?,
            member(&members, "entities.bin")?,
            member(&members, "players.bin")?,
            member(&members, "runtime.bin")?,
        )?;
        let reset_context = build_level_one_context(&metadata, &reset_state, &players, &entities)?;
        validate_reset_contract(
            require_object(object, "reset_projection", "manifest")?,
            &reset_state,
            &reset_context,
        )?;
        apply_authority(&mut entities, &reset_state, &reset_context)?;
        let goat_stage_point = decode_level_one_goat_stage_point(&runtime)?;
        let goat_config = GoatBehaviorConfig::from_parameter_record(
            presentation
                .entity_parameters
                .get(98)
                .ok_or_else(|| NativeWorldError::new("Level 1 Goat parameter row 98 is absent"))?,
        )
        .map_err(NativeWorldError::new)?;
        let goat_render = presentation
            .particles
            .get(usize::from(GOAT_RENDER_PARAMETER_INDEX))
            .ok_or_else(|| NativeWorldError::new("Level 1 Goat render parameter 238 is absent"))?;
        if goat_render.base_tmap != GOAT_BASE_TMAP {
            return Err(NativeWorldError::new(format!(
                "Level 1 Goat render parameter 238 mismatch: base TMAP {}",
                goat_render.base_tmap
            )));
        }
        let reset_live_world =
            LiveWorldState::from_entities(&entities, goat_stage_point, goat_config)
                .map_err(NativeWorldError::new)?;

        let (closure_sha256, mut asset_refs) = parse_closure(
            closure_path.as_ref(),
            &package_sha256,
            &manifest_bytes,
            &members,
        )?;
        let (sound_asset_refs, music_asset_ref) = match std::env::var_os("MC_ASSET_CACHE_PATH") {
            Some(cache_path) => append_level_one_audio_refs(
                Path::new(&cache_path),
                &package_sha256,
                &closure_sha256,
                &mut asset_refs,
            )?,
            None => (BTreeMap::new(), None),
        };

        let profile = metadata.map_type.profile();
        Ok(Self {
            metadata,
            package_sha256,
            closure_sha256,
            profile,
            terrain_planes,
            terrain,
            entities: entities.into_boxed_slice(),
            players: players.into_boxed_slice(),
            runtime,
            presentation,
            asset_refs: asset_refs.into_boxed_slice(),
            sound_asset_refs,
            music_asset_ref,
            reset_state,
            reset_context,
            goat_stage_point,
            goat_config,
            reset_live_world,
        })
    }

    #[must_use]
    pub const fn metadata(&self) -> &LevelMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &str {
        self.package_sha256.as_str()
    }

    #[must_use]
    pub const fn closure_sha256(&self) -> &str {
        self.closure_sha256.as_str()
    }

    #[must_use]
    pub const fn profile(&self) -> FlightProfile {
        self.profile
    }

    #[must_use]
    pub const fn terrain_planes(&self) -> &TerrainPlanes {
        &self.terrain_planes
    }

    #[must_use]
    pub const fn terrain(&self) -> &LegacyTerrain {
        &self.terrain
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    #[must_use]
    pub const fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    #[must_use]
    pub const fn presentation(&self) -> &PresentationSelectors {
        &self.presentation
    }

    #[must_use]
    pub fn asset_refs(&self) -> &[AssetRef] {
        &self.asset_refs
    }

    #[must_use]
    pub fn sound_asset_ref(&self, sound_id: u16) -> Option<u32> {
        self.sound_asset_refs.get(&sound_id).copied()
    }

    #[must_use]
    pub const fn music_asset_ref(&self) -> Option<u32> {
        self.music_asset_ref
    }

    #[must_use]
    pub const fn reset_state(&self) -> &FlightState {
        &self.reset_state
    }

    #[must_use]
    pub const fn reset_context(&self) -> &FlightContext {
        &self.reset_context
    }

    #[must_use]
    pub const fn goat_stage_point(&self) -> Position {
        self.goat_stage_point
    }

    #[must_use]
    pub const fn goat_config(&self) -> GoatBehaviorConfig {
        self.goat_config
    }

    #[must_use]
    pub fn reset_live_world(&self) -> &LiveWorldState {
        &self.reset_live_world
    }

    /// Build the mutable per-session context.
    ///
    /// The package's reset projection predates typed presentation events, so
    /// `reset_context` must remain byte-for-byte compatible with its preserved
    /// conformance hash. Level 1 effects are an additive session surface.
    #[must_use]
    pub fn session_context(&self) -> FlightContext {
        let mut context = self.reset_context.clone();
        context.effects = Some(FlightEffectsContext {
            local_player_index: self.runtime.local_player_slot,
            periodic_spell_effect: None,
            ..FlightEffectsContext::default()
        });
        context
    }

    #[must_use]
    pub fn session(&self) -> NativeSession<'_> {
        NativeSession {
            world: self,
            state: self.reset_state,
            context: self.session_context(),
            live_world: self.reset_live_world.clone(),
        }
    }
}

impl FlightWorld for NativeWorld {
    fn ground_height(&self, position: Position) -> i16 {
        self.terrain.ground_height(position)
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        self.terrain.ceiling_height(position)
    }

    fn material_mask(&self, position: Position) -> u32 {
        self.terrain.material_mask(position)
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.terrain.angle_flags(tile)
    }
}

/// Per-play-session mutable state, independent from the immutable world.
#[derive(Debug)]
pub struct NativeSession<'world> {
    world: &'world NativeWorld,
    pub state: FlightState,
    pub context: FlightContext,
    pub live_world: LiveWorldState,
}

impl NativeSession<'_> {
    pub fn reset(&mut self) {
        self.state = self.world.reset_state;
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world.clone();
    }

    pub fn step(&mut self, command: PlayerCommand) -> Vec<crate::StepEvidence> {
        self.context.events.clear();
        let evidence = step_outer_turn_with_context(
            &mut self.state,
            command,
            0,
            self.world.profile,
            self.world,
            &mut self.context,
        );
        self.live_world.step_with_players(
            self.world,
            &[GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: 0,
            }],
        );
        evidence
    }
}

fn decode_level_one_goat_stage_point(runtime: &RuntimeSnapshot) -> Result<Position> {
    let stage = runtime
        .stage_variables
        .get(3)
        .ok_or_else(|| NativeWorldError::new("Level 1 Goat stage slot 3 is absent"))?;
    let RuntimeRelation::AxisScalar(axis) = stage.relation else {
        return Err(NativeWorldError::new(
            "Level 1 Goat stage slot 3 is not an axis payload",
        ));
    };
    let point = Position {
        x: axis as u16,
        y: (axis >> 16) as u16,
        z: 0,
    };
    if stage.kind != 2 || stage.flags & 1 == 0 || point != GOAT_STAGE_POINT {
        return Err(NativeWorldError::new(format!(
            "Level 1 Goat stage slot 3 mismatch: kind {}, flags {}, point ({}, {})",
            stage.kind, stage.flags, point.x, point.y
        )));
    }
    Ok(point)
}

fn parse_metadata(manifest: &serde_json::Map<String, Value>) -> Result<LevelMetadata> {
    let level = require_object(manifest, "level", "manifest")?;
    let map_type = require_object(level, "map_type", "manifest.level")?;
    let capture = require_object(manifest, "capture", "manifest")?;
    let dimensions = require_object(manifest, "dimensions", "manifest")?;
    let terrain_dimensions = require_array(dimensions, "terrain", "manifest.dimensions")?;
    if terrain_dimensions.as_slice() != [json!(256), json!(256)] {
        return Err(NativeWorldError::new(
            "manifest terrain dimensions must be 256x256",
        ));
    }
    Ok(LevelMetadata {
        external_level: as_u16(
            require_value(level, "external", "manifest.level")?,
            "manifest.level.external",
        )?,
        internal_level: as_u16(
            require_value(level, "internal", "manifest.level")?,
            "manifest.level.internal",
        )?,
        map_type: MapType::decode(
            as_u8(
                require_value(map_type, "id", "manifest.level.map_type")?,
                "manifest.level.map_type.id",
            )?,
            require_string(map_type, "name", "manifest.level.map_type")?,
        )?,
        capture_site: require_string(capture, "site", "manifest.capture")?.to_owned(),
        capture_frame: as_u32(
            require_value(capture, "frame", "manifest.capture")?,
            "manifest.capture.frame",
        )?,
    })
}

fn validate_manifest_counts(counts: &serde_json::Map<String, Value>) -> Result<()> {
    for (name, expected) in [
        ("authored_features", 1_200_u64),
        ("entity_slots", 1_000),
        ("active_entities", 177),
        ("players", 8),
        ("particle_parameters", 347),
        ("level_sprite_selectors", 504),
        ("entity_parameter_records", 157),
    ] {
        let actual = require_value(counts, name, "manifest.counts")?
            .as_u64()
            .ok_or_else(|| NativeWorldError::new(format!("manifest count {name} is not u64")))?;
        if actual != expected {
            return Err(NativeWorldError::new(format!(
                "manifest count {name} must be {expected}, got {actual}"
            )));
        }
    }
    Ok(())
}

fn validate_manifest_layouts(manifest: &serde_json::Map<String, Value>) -> Result<()> {
    let sizes = require_object(manifest, "record_sizes", "manifest")?;
    require_exact_u64(sizes, "entity", ENTITY_RECORD_BYTES as u64, "record_sizes")?;
    require_exact_u64(sizes, "player", PLAYER_RECORD_BYTES as u64, "record_sizes")?;
    let layouts = require_object(manifest, "record_layouts", "manifest")?;
    let entity = require_object(layouts, "entity", "record_layouts")?;
    for (name, expected) in [
        ("index_offset", 0),
        ("active_offset", 2),
        ("next_relation_offset", 4),
        ("parameter_relation_offset", 164),
        ("special_domain_offset", 168),
        ("special_relation_offset", 172),
    ] {
        require_exact_u64(entity, name, expected, "record_layouts.entity")?;
    }
    let player = require_object(layouts, "player", "record_layouts")?;
    for (name, expected) in [
        ("index_offset", 0),
        ("normalized_host_time_offset", 50),
        ("legacy_v1_bytes", 565),
        ("ambient_loop_5_distance_offset", 565),
        ("ambient_loop_31_distance_offset", 569),
        ("record_bytes", 573),
    ] {
        require_exact_u64(player, name, expected, "record_layouts.player")?;
    }
    let runtime = require_object(layouts, "runtime", "record_layouts")?;
    require_exact_u64(
        runtime,
        "scalar_prefix_bytes",
        192,
        "record_layouts.runtime",
    )?;
    require_exact_u64(
        runtime,
        "bytes",
        RUNTIME_BYTES as u64,
        "record_layouts.runtime",
    )?;
    for (name, offset, count, bytes) in [
        ("stages", 192, 8, 14),
        ("objectives", 304, 8, 11),
        ("stage_variables", 392, 11, 12),
        ("events", 524, 50, 43),
        ("player_inputs", 2_674, 8, 10),
    ] {
        let section = require_object(runtime, name, "record_layouts.runtime")?;
        require_exact_u64(section, "offset", offset, name)?;
        require_exact_u64(section, "count", count, name)?;
        require_exact_u64(section, "record_bytes", bytes, name)?;
    }
    Ok(())
}

fn validate_package(
    package_dir: &Path,
    manifest: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let files = require_object(manifest, "files", "manifest")?;
    if files.len() != PACKAGE_MEMBERS.len() {
        return Err(NativeWorldError::new(
            "manifest files map must contain exactly six package members",
        ));
    }
    let mut members = BTreeMap::new();
    let mut identity = Vec::new();
    for (name, expected_size) in PACKAGE_MEMBERS {
        let entry = require_object(files, name, "manifest.files")?;
        require_exact_u64(entry, "bytes", expected_size as u64, name)?;
        let expected_hash = require_hash_string(entry, "sha256", name)?;
        let bytes = read_file(&package_dir.join(name))?;
        if bytes.len() != expected_size {
            return Err(NativeWorldError::new(format!(
                "{name} has {} bytes, expected {expected_size}",
                bytes.len()
            )));
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != expected_hash {
            return Err(NativeWorldError::new(format!("{name} hash mismatch")));
        }
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(actual_hash.as_bytes());
        identity.push(0);
        identity.extend_from_slice(expected_size.to_string().as_bytes());
        identity.push(b'\n');
        members.insert(name.to_owned(), bytes);
    }
    let actual_package_hash = sha256_hex(&identity);
    let expected_package_hash = require_hash_string(manifest, "package_sha256", "manifest")?;
    if actual_package_hash != expected_package_hash {
        return Err(NativeWorldError::new("package_sha256 mismatch"));
    }
    Ok(members)
}

fn parse_terrain(bytes: &[u8]) -> Result<TerrainPlanes> {
    if bytes.len() != TERRAIN_BYTES {
        return Err(NativeWorldError::new("terrain.bin has an invalid extent"));
    }
    let plane = |index: usize| {
        bytes[index * LEGACY_MAP_CELLS..(index + 1) * LEGACY_MAP_CELLS]
            .to_vec()
            .into_boxed_slice()
    };
    let entity_bytes = &bytes[5 * LEGACY_MAP_CELLS..];
    let mut entity_indices = Vec::with_capacity(LEGACY_MAP_CELLS);
    for pair in entity_bytes.chunks_exact(2) {
        entity_indices.push(i16::from_le_bytes([pair[0], pair[1]]));
    }
    Ok(TerrainPlanes {
        terrain_types: plane(0),
        ground_heights: plane(1),
        shading: plane(2),
        angles: plane(3),
        secondary_heights: plane(4),
        entity_indices: entity_indices.into_boxed_slice(),
    })
}

fn parse_entities(bytes: &[u8]) -> Result<Vec<EntitySnapshot>> {
    validate_header(
        bytes,
        *b"MCE1",
        1,
        ENTITY_RECORD_BYTES_U16,
        ENTITY_COUNT_U32,
        "entities.bin",
    )?;
    let mut entities = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let record = record(bytes, 12, ENTITY_RECORD_BYTES, index, "entities.bin")?;
        let stored_index = read_u16(record, 0, "entity index")?;
        if usize::from(stored_index) != index {
            return Err(NativeWorldError::new(format!(
                "entity slot/index mismatch at {index}: {stored_index}"
            )));
        }
        let active_byte = read_u8(record, 2, "entity active")?;
        if active_byte > 1 {
            return Err(NativeWorldError::new(format!(
                "entity {index} has invalid active byte {active_byte}"
            )));
        }
        let next = relation_one_based(
            read_u32(record, 4, "entity next relation")?,
            ENTITY_COUNT,
            "entity next relation",
        )?;
        let parameter_relation = relation_one_based(
            read_u32(record, 164, "entity parameter relation")?,
            ENTITY_PARAMETER_COUNT,
            "entity parameter relation",
        )?;
        let domain = read_u8(record, 168, "entity special domain")?;
        if record[169..172] != [0, 0, 0] {
            return Err(NativeWorldError::new(format!(
                "entity {index} has non-zero special relation padding"
            )));
        }
        let special_value = read_u32(record, 172, "entity special relation")?;
        let special_relation = match (domain, special_value) {
            (0, 0) => EntitySpecialRelation::None,
            (1, 1..=8) => EntitySpecialRelation::PlayerState(
                u8::try_from(special_value - 1).expect("bounded player slot"),
            ),
            (2, 1) => EntitySpecialRelation::GlobalSpecialState,
            _ => {
                return Err(NativeWorldError::new(format!(
                    "entity {index} has invalid special relation {domain}/{special_value}"
                )));
            }
        };
        let active = active_byte == 1;
        entities.push(EntitySnapshot {
            index: stored_index,
            source_id: read_u16(record, 30, "entity source id")?,
            authority: if active {
                EntityAuthority::FrozenObserved
            } else {
                EntityAuthority::Inactive
            },
            active,
            next_relation: next,
            class: read_u8(record, 67, "entity class")?,
            model: read_u8(record, 68, "entity model")?,
            control_flags: read_u32(record, 16, "entity control flags")?,
            action: read_u8(record, 73, "entity action")?,
            action_marker: read_i8(record, 64, "entity action marker")?,
            level_gate: read_i8(record, 74, "entity level gate")?,
            position: Position {
                x: read_u16(record, 80, "entity x")?,
                y: read_u16(record, 82, "entity y")?,
                z: read_i16(record, 84, "entity z")?,
            },
            yaw: read_i16(record, 32, "entity yaw")?,
            pitch: read_i16(record, 34, "entity pitch")?,
            fov: read_i16(record, 38, "entity fov")?,
            phase: read_u8(record, 66, "entity phase")?,
            active_value: read_i16(record, 50, "entity active value")?,
            activation_value: read_u16(record, 52, "entity activation value")?,
            actual_speed: read_i16(record, 134, "entity actual speed")?,
            mana_cost: read_i32(record, 144, "entity mana cost")?,
            mana: read_i32(record, 148, "entity mana")?,
            parameter_relation,
            special_relation,
            record: record.to_vec().into_boxed_slice(),
        });
    }
    Ok(entities)
}

fn parse_players(bytes: &[u8]) -> Result<Vec<PlayerSnapshot>> {
    validate_header(
        bytes,
        *b"MCP1",
        2,
        PLAYER_RECORD_BYTES_U16,
        PLAYER_COUNT_U32,
        "players.bin",
    )?;
    let mut players = Vec::with_capacity(PLAYER_COUNT);
    for slot in 0..PLAYER_COUNT {
        let record = record(bytes, 12, PLAYER_RECORD_BYTES, slot, "players.bin")?;
        if usize::from(read_u16(record, 0, "player index")?) != slot {
            return Err(NativeWorldError::new(format!(
                "player slot/index mismatch at {slot}"
            )));
        }
        let ai = read_u8(record, 2, "player ai flag")?;
        if ai > 1 || record[3] != 0 {
            return Err(NativeWorldError::new(format!(
                "player {slot} has invalid flags"
            )));
        }
        let entity_index = read_u16(record, 4, "player entity index")?;
        if usize::from(entity_index) >= ENTITY_COUNT {
            return Err(NativeWorldError::new(format!(
                "player {slot} entity relation is out of bounds"
            )));
        }
        let left = read_i16(record, 554, "left spell slot")?;
        let right = read_i16(record, 556, "right spell slot")?;
        if !(0..PLAYER_SPELL_COUNT_I16).contains(&left)
            || !(0..PLAYER_SPELL_COUNT_I16).contains(&right)
        {
            return Err(NativeWorldError::new(format!(
                "player {slot} spell selection is out of bounds"
            )));
        }
        let mut holders = [0_u16; PLAYER_SPELL_COUNT];
        for (index, holder) in holders.iter_mut().enumerate() {
            *holder = read_u16(record, 268 + index * 2, "enabled spell holder")?;
            if usize::from(*holder) >= ENTITY_COUNT && *holder != 0 {
                return Err(NativeWorldError::new(format!(
                    "player {slot} spell holder relation is out of bounds"
                )));
            }
        }
        players.push(PlayerSnapshot {
            slot: u8::try_from(slot).expect("eight player slots"),
            is_ai: ai == 1,
            entity_index,
            active_player_index: read_i16(record, 6, "active player index")?,
            turn: read_i32(record, 8, "player turn")?,
            left_spell_slot: u8::try_from(left).expect("validated spell slot"),
            right_spell_slot: u8::try_from(right).expect("validated spell slot"),
            enabled_spell_holders: holders,
            record: record.to_vec().into_boxed_slice(),
        });
    }
    Ok(players)
}

#[allow(clippy::too_many_lines)]
fn parse_runtime(bytes: &[u8]) -> Result<RuntimeSnapshot> {
    if bytes.len() != RUNTIME_BYTES
        || bytes.get(..4) != Some(b"MCR1")
        || read_u16(bytes, 4, "runtime version")? != 1
        || read_u16(bytes, 6, "runtime reserved")? != 0
    {
        return Err(NativeWorldError::new(
            "runtime.bin header/version/extent mismatch",
        ));
    }
    let local_player = read_i16(bytes, 16, "runtime local player slot")?;
    let player_count = read_i16(bytes, 18, "runtime player count")?;
    if !(0..PLAYER_COUNT_I16).contains(&local_player)
        || !(1..=PLAYER_COUNT_I16).contains(&player_count)
    {
        return Err(NativeWorldError::new(
            "runtime player slot/count is out of bounds",
        ));
    }
    let mut stages = Vec::with_capacity(8);
    for index in 0..8 {
        let stage = record(bytes, 192, 14, index, "runtime stages")?;
        let domain = read_u8(stage, 6, "runtime stage relation domain")?;
        if stage[7..10] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime stage relation padding is non-zero",
            ));
        }
        let value = read_u32(stage, 10, "runtime stage relation")?;
        let relation = match domain {
            0 => RuntimeRelation::Scalar(value),
            1 => RuntimeRelation::AuthoredFeature(one_based(
                value,
                1_200,
                "runtime authored-feature relation",
            )?),
            2 => RuntimeRelation::LiveEntity(one_based(
                value,
                ENTITY_COUNT,
                "runtime live-entity relation",
            )?),
            3 if value < ENTITY_COUNT_U32 => {
                RuntimeRelation::LiveEntityIndex(u16::try_from(value).expect("entity index bound"))
            }
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime stage {index} relation {domain}/{value} is invalid"
                )));
            }
        };
        stages.push(RuntimeStage {
            kind: read_i8(stage, 0, "runtime stage type")?,
            flags: read_i8(stage, 1, "runtime stage flags")?,
            axis_x: read_i16(stage, 2, "runtime stage x")?,
            axis_y: read_i16(stage, 4, "runtime stage y")?,
            relation,
        });
    }
    let mut objectives = Vec::with_capacity(8);
    for index in 0..8 {
        let objective = record(bytes, 304, 11, index, "runtime objectives")?;
        objectives.push(RuntimeObjective {
            is_level_end: objective[0],
            text_selector: objective[1],
            done: objective[2],
            stage_selectors: objective[3..11]
                .try_into()
                .expect("validated objective record width"),
        });
    }
    let mut stage_variables = Vec::with_capacity(11);
    for index in 0..11 {
        let stage_var = record(bytes, 392, 12, index, "runtime stage variables")?;
        let domain = read_u8(stage_var, 4, "runtime stage-variable domain")?;
        if stage_var[5..8] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime stage-variable relation padding is non-zero",
            ));
        }
        let value = read_u32(stage_var, 8, "runtime stage-variable relation")?;
        let relation = match domain {
            0 => RuntimeRelation::AxisScalar(value),
            1 => RuntimeRelation::LiveEntity(one_based(
                value,
                ENTITY_COUNT,
                "runtime stage-variable entity relation",
            )?),
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime stage-variable {index} domain is invalid"
                )));
            }
        };
        stage_variables.push(RuntimeStageVariable {
            kind: read_i8(stage_var, 0, "runtime stage-variable type")?,
            flags: read_i8(stage_var, 1, "runtime stage-variable flags")?,
            word_2: read_u16(stage_var, 2, "runtime stage-variable word 2")?,
            relation,
        });
    }
    for index in 0..50 {
        let event = record(bytes, 524, 43, index, "runtime events")?;
        let domain = read_u8(event, 10, "runtime event domain")?;
        if event[11..14] != [0, 0, 0] {
            return Err(NativeWorldError::new(
                "runtime event relation padding is non-zero",
            ));
        }
        let value = read_u32(event, 14, "runtime event relation")?;
        match (domain, value) {
            (0, 0) => {}
            (1, _) => {
                one_based(value, ENTITY_COUNT, "runtime event entity relation")?;
            }
            _ => {
                return Err(NativeWorldError::new(format!(
                    "runtime event {index} relation is invalid"
                )));
            }
        }
    }
    let mut player_inputs = [PlayerCommand::default(); PLAYER_COUNT];
    for (index, command) in player_inputs.iter_mut().enumerate() {
        let input = record(bytes, 2_674, 10, index, "runtime player inputs")?;
        *command = PlayerCommand::from_bytes(
            input
                .try_into()
                .map_err(|_| NativeWorldError::new("runtime input extent mismatch"))?,
        );
    }
    Ok(RuntimeSnapshot {
        global_turn: read_u32(bytes, 8, "runtime global turn")?,
        random_state: read_u32(bytes, 12, "runtime random state")?,
        local_player_slot: u8::try_from(local_player).expect("validated player slot"),
        player_count: u8::try_from(player_count).expect("validated player count"),
        maptype_music: read_i32(bytes, 32, "runtime maptype music")?,
        stage_index: read_u8(bytes, 66, "runtime stage index")?,
        objective_textbox_ticks: read_u8(bytes, 70, "runtime objective textbox ticks")?,
        stages: stages.into_boxed_slice(),
        stage_variables: stage_variables.into_boxed_slice(),
        objectives: objectives.into_boxed_slice(),
        player_inputs,
        bytes: bytes.to_vec().into_boxed_slice(),
    })
}

fn parse_presentation(bytes: &[u8]) -> Result<PresentationSelectors> {
    if bytes.len() != 10_720
        || bytes.get(..4) != Some(b"MCV1")
        || read_u16(bytes, 4, "presentation version")? != 1
        || read_u16(bytes, 6, "presentation reserved")? != 0
        || read_u32(bytes, 8, "presentation particle count")? != PARTICLE_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation.bin header/version/count/extent mismatch",
        ));
    }
    let mut particles = Vec::with_capacity(PARTICLE_COUNT);
    for index in 0..PARTICLE_COUNT {
        let particle = record(
            bytes,
            12,
            PARTICLE_RECORD_BYTES,
            index,
            "presentation particles",
        )?;
        let render_mode = read_u8(particle, 12, "particle render mode")?;
        if render_mode > 36 {
            return Err(NativeWorldError::new(format!(
                "particle {index} render mode {render_mode} is invalid"
            )));
        }
        particles.push(ParticleParameter {
            base_tmap: read_u16(particle, 0, "particle base tmap")?,
            word_2: read_u16(particle, 2, "particle word 2")?,
            word_4: read_u16(particle, 4, "particle word 4")?,
            speed: read_u16(particle, 6, "particle speed")?,
            rotation_speed: read_u16(particle, 8, "particle rotation speed")?,
            byte_10: read_u8(particle, 10, "particle byte 10")?,
            byte_11: read_i8(particle, 11, "particle byte 11")?,
            render_mode,
            byte_13: read_u8(particle, 13, "particle byte 13")?,
        });
    }
    let selector_count_offset = 12 + PARTICLE_COUNT * PARTICLE_RECORD_BYTES;
    if read_u32(bytes, selector_count_offset, "presentation selector count")?
        != LEVEL_SPRITE_SELECTOR_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation level-sprite selector count mismatch",
        ));
    }
    let selector_start = selector_count_offset + 4;
    let selector_end = selector_start + LEVEL_SPRITE_SELECTOR_COUNT;
    let parameter_count_offset = selector_end;
    if read_u32(
        bytes,
        parameter_count_offset,
        "presentation parameter count",
    )? != ENTITY_PARAMETER_COUNT_U32
    {
        return Err(NativeWorldError::new(
            "presentation entity-parameter count mismatch",
        ));
    }
    let parameter_start = parameter_count_offset + 4;
    let mut entity_parameters = Vec::with_capacity(ENTITY_PARAMETER_COUNT);
    for index in 0..ENTITY_PARAMETER_COUNT {
        entity_parameters.push(
            record(
                bytes,
                parameter_start,
                ENTITY_PARAMETER_BYTES,
                index,
                "presentation entity parameters",
            )?
            .try_into()
            .map_err(|_| NativeWorldError::new("entity-parameter extent mismatch"))?,
        );
    }
    Ok(PresentationSelectors {
        particles: particles.into_boxed_slice(),
        level_sprite_selectors: bytes[selector_start..selector_end]
            .to_vec()
            .into_boxed_slice(),
        entity_parameters: entity_parameters.into_boxed_slice(),
    })
}

fn validate_manifest_relations(
    counts: &serde_json::Map<String, Value>,
    entities: &[EntitySnapshot],
    players: &[PlayerSnapshot],
    runtime: &RuntimeSnapshot,
    presentation: &PresentationSelectors,
) -> Result<()> {
    let active_count = entities.iter().filter(|entity| entity.active).count();
    let expected_active = require_value(counts, "active_entities", "manifest.counts")?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new("active_entities is not u64"))?;
    if active_count as u64 != expected_active {
        return Err(NativeWorldError::new(format!(
            "active entity count mismatch: {active_count}/{expected_active}"
        )));
    }
    if players.len() != PLAYER_COUNT
        || runtime.stages.len() != 8
        || runtime.stage_variables.len() != 11
        || runtime.objectives.len() != 8
        || presentation.particles.len() != PARTICLE_COUNT
        || presentation.level_sprite_selectors.len() != LEVEL_SPRITE_SELECTOR_COUNT
        || presentation.entity_parameters.len() != ENTITY_PARAMETER_COUNT
    {
        return Err(NativeWorldError::new(
            "decoded component count does not match manifest",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_reset_state(
    reset: &serde_json::Map<String, Value>,
    entities: &[u8],
    players: &[u8],
    runtime: &[u8],
) -> Result<FlightState> {
    if require_string(reset, "schema", "reset_projection")? != RESET_SCHEMA {
        return Err(NativeWorldError::new(
            "reset projection schema must be mc2-flight-reset-projection/v1",
        ));
    }
    let source = require_object(reset, "source", "reset_projection")?;
    require_exact_u64(source, "capture_frame", 0, "reset_projection.source")?;
    let local_mapping = require_object(source, "local_player_slot", "reset_projection.source")?;
    require_mapping_scalar(
        local_mapping,
        "runtime.bin",
        16,
        "i16_le",
        "local_player_slot",
    )?;
    require_exact_i64(local_mapping, "minimum", 0, "local_player_slot")?;
    require_exact_i64(local_mapping, "maximum", 7, "local_player_slot")?;
    let slot = read_i16(runtime, 16, "local player slot")?;
    if !(0..PLAYER_COUNT_I16).contains(&slot) {
        return Err(NativeWorldError::new(
            "reset local-player slot is out of bounds",
        ));
    }
    let player = record(
        players,
        12,
        PLAYER_RECORD_BYTES,
        usize::try_from(slot).expect("validated player slot"),
        "players.bin",
    )?;
    let entity_index = read_u16(player, 4, "reset player entity relation")?;
    let entity = record(
        entities,
        12,
        ENTITY_RECORD_BYTES,
        usize::from(entity_index),
        "entities.bin",
    )?;
    let mappings = require_object(reset, "field_mappings", "reset_projection")?;

    let exported_i16 = |name: &str, offset: usize| -> Result<i16> {
        require_mapping(
            mappings,
            name,
            "exported_field",
            "players.bin",
            Some("local_player_slot"),
            offset,
            "i16_le",
        )?;
        read_i16(player, offset, name)
    };
    let signed_entity_field = |name: &str, offset: usize| -> Result<i16> {
        require_mapping(
            mappings,
            name,
            "entity_relation_field",
            "entities.bin",
            None,
            offset,
            "i16_le",
        )?;
        read_i16(entity, offset, name)
    };
    let unsigned_entity_field = |name: &str, offset: usize| -> Result<u16> {
        require_mapping(
            mappings,
            name,
            "entity_relation_field",
            "entities.bin",
            None,
            offset,
            "u16_le",
        )?;
        read_u16(entity, offset, name)
    };
    require_mapping(
        mappings,
        "entity_index",
        "exported_field",
        "players.bin",
        Some("local_player_slot"),
        4,
        "u16_le",
    )?;
    let state = FlightState {
        entity_index,
        position: Position {
            x: unsigned_entity_field("position.x", 80)?,
            y: unsigned_entity_field("position.y", 82)?,
            z: signed_entity_field("position.z", 84)?,
        },
        yaw: signed_entity_field("yaw", 32)?,
        display_pitch: signed_entity_field("display_pitch", 34)?,
        effective_pitch: exported_i16("effective_pitch", 38)?,
        roll_accumulator: exported_i16("roll_accumulator", 46)?,
        pitch_accumulator: exported_i16("pitch_accumulator", 48)?,
        target_speed: exported_i16("target_speed", 20)?,
        speed_changed_flag: zero_i16(mappings, "speed_changed_flag")?,
        actual_speed: signed_entity_field("actual_speed", 134)?,
        strafe_speed: exported_i16("strafe_speed", 22)?,
        roll_delta: exported_i16("roll_delta", 16)?,
        pitch_delta: exported_i16("pitch_delta", 18)?,
        recoil_speed: exported_i16("recoil_speed", 32)?,
        recoil_yaw: exported_i16("recoil_yaw", 34)?,
        impulse_x: exported_i16("impulse_x", 54)?,
        impulse_y: exported_i16("impulse_y", 56)?,
        impulse_z: exported_i16("impulse_z", 58)?,
        entity_fov: signed_entity_field("entity_fov", 38)?,
        water_counter: zero_i8(mappings, "water_counter")?,
        speed_modifier: {
            require_mapping(
                mappings,
                "speed_modifier",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                44,
                "u8",
            )?;
            read_u8(player, 44, "speed modifier")?
        },
        speed_modifier_counter: {
            require_mapping(
                mappings,
                "speed_modifier_counter",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                45,
                "i8",
            )?;
            read_i8(player, 45, "speed modifier counter")?
        },
        immobilize_counter: zero_i8(mappings, "immobilize_counter")?,
        immobilize_subcounter: zero_i8(mappings, "immobilize_subcounter")?,
        tether_prior_distance: zero_i32(mappings, "tether_prior_distance")?,
        tether_target_index: zero_u16(mappings, "tether_target_index")?,
        tether_holder_index: zero_u16(mappings, "tether_holder_index")?,
        tether_tier: zero_u16(mappings, "tether_tier")?,
        speed_spell_holder_index: zero_u16(mappings, "speed_spell_holder_index")?,
        entity_phase: {
            require_mapping(
                mappings,
                "entity_phase",
                "entity_relation_field",
                "entities.bin",
                None,
                66,
                "u8",
            )?;
            read_u8(entity, 66, "entity phase")?
        },
        ambient_loop_5_distance: mapped_player_i32(
            mappings,
            player,
            "ambient_loop_5_distance",
            565,
        )?,
        ambient_loop_31_distance: mapped_player_i32(
            mappings,
            player,
            "ambient_loop_31_distance",
            569,
        )?,
        player_color_index: {
            require_mapping(
                mappings,
                "player_color_index",
                "exported_field",
                "players.bin",
                Some("local_player_slot"),
                40,
                "u8_low_of_i16_le",
            )?;
            read_u8(player, 40, "player color index")?
        },
        music_countdown: zero_i16(mappings, "music_countdown")?,
        recovery_latch: zero_bool(mappings, "recovery_latch")?,
        skip_flight_once: {
            let mapping = require_object(mappings, "skip_flight_once", "field_mappings")?;
            require_string(mapping, "kind", "skip_flight_once")?
                .eq("entity_relation_bit")
                .then_some(())
                .ok_or_else(|| NativeWorldError::new("skip_flight_once kind mismatch"))?;
            require_string(mapping, "file", "skip_flight_once")?
                .eq("entities.bin")
                .then_some(())
                .ok_or_else(|| NativeWorldError::new("skip_flight_once file mismatch"))?;
            require_exact_u64(mapping, "offset", 17, "skip_flight_once")?;
            require_exact_u64(mapping, "bit", 3, "skip_flight_once")?;
            read_u8(entity, 17, "skip flight entity flags")? & 8 != 0
        },
        tick: u64::try_from(zero_i64(mappings, "tick")?)
            .map_err(|_| NativeWorldError::new("tick seam default is negative"))?,
    };
    if mappings.len() != 39 {
        return Err(NativeWorldError::new(format!(
            "reset projection must map exactly 39 fields, got {}",
            mappings.len()
        )));
    }
    Ok(state)
}

fn build_level_one_context(
    metadata: &LevelMetadata,
    state: &FlightState,
    players: &[PlayerSnapshot],
    entities: &[EntitySnapshot],
) -> Result<FlightContext> {
    if metadata.external_level != 1 || metadata.internal_level != 0 {
        return Err(NativeWorldError::new(
            "context reconstruction currently supports only export Level 1",
        ));
    }
    let player_slot = players
        .iter()
        .position(|player| player.entity_index == state.entity_index)
        .ok_or_else(|| NativeWorldError::new("reset entity has no player relation"))?;
    let player = &players[player_slot];
    let left_slot = usize::from(player.left_spell_slot);
    let holder_index = player.enabled_spell_holders[left_slot];
    if holder_index == 0 || usize::from(holder_index) >= entities.len() {
        return Err(NativeWorldError::new(
            "Level 1 left-spell activation holder is absent or invalid",
        ));
    }
    let caster = &entities[usize::from(state.entity_index)];
    let holder = &entities[usize::from(holder_index)];
    if !caster.active || !holder.active {
        return Err(NativeWorldError::new(
            "Level 1 flight caster or left-spell holder is inactive",
        ));
    }
    Ok(FlightContext {
        effects: None,
        spell_fire: Some(SpellFireContext {
            left_holder_index: holder_index,
            caster: SpellCasterState {
                entity_index: caster.index,
                model: caster.model,
                mana: caster.mana,
                control_flags: caster.control_flags,
                player_color_index: state.player_color_index,
                // This value is proven for Level 1 by the preserved regression
                // state but is not serialized by package v2.
                control_mode: LEVEL_ONE_CONTROL_MODE_SEAM_DEFAULT,
            },
            holders: vec![SpellActivationHolder {
                entity_index: holder.index,
                model: holder.model,
                level_gate: holder.level_gate,
                active_value: holder.active_value,
                activation_value: holder.activation_value,
                mana_cost: holder.mana_cost,
                action_marker: holder.action_marker,
            }],
            ..SpellFireContext::default()
        }),
        ..FlightContext::default()
    })
}

fn validate_reset_contract(
    reset: &serde_json::Map<String, Value>,
    state: &FlightState,
    context: &FlightContext,
) -> Result<()> {
    let expected_state = require_value(reset, "expected_state", "reset_projection")?;
    if expected_state != &flight_state_json(state) {
        return Err(NativeWorldError::new(
            "decoded reset state does not match manifest expected_state",
        ));
    }
    let hash = require_object(reset, "state_hash", "reset_projection")?;
    for (name, expected) in [
        ("api", "mc_sim_reset"),
        ("schema", "mc2-flight-state-hash/fnv1a64-v1"),
        ("algorithm", "FNV-1a 64-bit"),
        ("byte_order", "little-endian"),
    ] {
        if require_string(hash, name, "reset state_hash")? != expected {
            return Err(NativeWorldError::new(format!(
                "reset state_hash {name} contract mismatch"
            )));
        }
    }
    require_exact_u64(hash, "canonical_bytes", 81, "reset state_hash")?;
    let ordered_fields = require_array(hash, "ordered_fields", "reset state_hash")?;
    let expected_fields = [
        "entity_index:u16",
        "position.x:u16",
        "position.y:u16",
        "position.z:i16",
        "yaw:i16",
        "display_pitch:i16",
        "effective_pitch:i16",
        "roll_accumulator:i16",
        "pitch_accumulator:i16",
        "target_speed:i16",
        "speed_changed_flag:i16",
        "actual_speed:i16",
        "strafe_speed:i16",
        "roll_delta:i16",
        "pitch_delta:i16",
        "recoil_speed:i16",
        "recoil_yaw:i16",
        "impulse_x:i16",
        "impulse_y:i16",
        "impulse_z:i16",
        "entity_fov:i16",
        "water_counter:i8",
        "speed_modifier:u8",
        "speed_modifier_counter:i8",
        "immobilize_counter:i8",
        "immobilize_subcounter:i8",
        "tether_prior_distance:i32",
        "tether_target_index:u16",
        "tether_holder_index:u16",
        "tether_tier:u16",
        "speed_spell_holder_index:u16",
        "entity_phase:u8",
        "ambient_loop_5_distance:i32",
        "ambient_loop_31_distance:i32",
        "player_color_index:u8",
        "music_countdown:i16",
        "recovery_latch:u8",
        "skip_flight_once:u8",
        "tick:u64",
    ];
    if ordered_fields.len() != expected_fields.len()
        || ordered_fields
            .iter()
            .zip(expected_fields)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(NativeWorldError::new(
            "reset state_hash ordered_fields contract mismatch",
        ));
    }
    let expected_hash = parse_u64_hex(require_string(hash, "hex", "reset state_hash")?)?;
    if state_hash(state) != expected_hash {
        return Err(NativeWorldError::new("reset state hash mismatch"));
    }
    let conformance = require_object(reset, "conformance_hash", "reset_projection")?;
    if require_string(conformance, "schema", "reset conformance_hash")?
        != "mc2-flight-conformance-hash/fnv1a64-v1"
        || require_string(conformance, "context", "reset conformance_hash")?
            != "NativeSim::owner_level_one"
    {
        return Err(NativeWorldError::new(
            "reset conformance hash contract mismatch",
        ));
    }
    let expected_conformance = parse_u64_hex(require_string(
        conformance,
        "hex",
        "reset conformance_hash",
    )?)?;
    if conformance_hash(state, context) != expected_conformance {
        return Err(NativeWorldError::new("reset conformance hash mismatch"));
    }
    Ok(())
}

fn apply_authority(
    entities: &mut [EntitySnapshot],
    state: &FlightState,
    context: &FlightContext,
) -> Result<()> {
    let player = entities
        .get_mut(usize::from(state.entity_index))
        .ok_or_else(|| NativeWorldError::new("reset player entity is out of bounds"))?;
    if !player.active {
        return Err(NativeWorldError::new("reset player entity is inactive"));
    }
    player.authority = EntityAuthority::PlayerFlightLive;
    let fire = context
        .spell_fire
        .as_ref()
        .ok_or_else(|| NativeWorldError::new("Level 1 spell context is absent"))?;
    if fire.holders.len() != 1 || fire.holders[0].entity_index != fire.left_holder_index {
        return Err(NativeWorldError::new(
            "Level 1 spell activation authority is not singular",
        ));
    }
    let holder = entities
        .get_mut(usize::from(fire.left_holder_index))
        .ok_or_else(|| NativeWorldError::new("spell holder entity is out of bounds"))?;
    if !holder.active {
        return Err(NativeWorldError::new("spell holder entity is inactive"));
    }
    holder.authority = EntityAuthority::SpellActivationLive;
    for index in LEVEL_ONE_GOAT_INDICES {
        let goat = entities.get_mut(usize::from(index)).ok_or_else(|| {
            NativeWorldError::new(format!("Goat entity {index} is out of bounds"))
        })?;
        if !goat.active || goat.class != GOAT_CLASS || goat.model != GOAT_MODEL {
            return Err(NativeWorldError::new(format!(
                "Goat authority row {index} is not active class {GOAT_CLASS}/model {GOAT_MODEL}"
            )));
        }
        goat.authority = EntityAuthority::GoatLive;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn parse_closure(
    path: &Path,
    package_sha256: &str,
    manifest_bytes: &[u8],
    members: &BTreeMap<String, Vec<u8>>,
) -> Result<(String, Vec<AssetRef>)> {
    let bytes = read_file(path)?;
    let mut closure: Value = serde_json::from_slice(&bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid closure JSON: {error}")))?;
    let closure_object = as_object(&closure, "closure")?;
    if require_string(closure_object, "schema", "closure")? != CLOSURE_SCHEMA {
        return Err(NativeWorldError::new(
            "closure schema must be mc2-level-asset-closure/v2",
        ));
    }
    if require_string(closure_object, "scope", "closure")? != "PRIVATE_TRACK_P_OWNER_DATA" {
        return Err(NativeWorldError::new("closure scope mismatch"));
    }
    let expected_closure_hash =
        require_hash_string(closure_object, "closure_sha256", "closure")?.to_owned();
    as_object_mut(&mut closure, "closure")?.remove("closure_sha256");
    let canonical = canonical_json(&closure)?;
    if sha256_hex(canonical.as_bytes()) != expected_closure_hash {
        return Err(NativeWorldError::new("closure_sha256 mismatch"));
    }

    let object = as_object(
        &serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| NativeWorldError::new(format!("invalid closure JSON: {error}")))?,
        "closure",
    )?
    .clone();
    let source = require_object(&object, "source", "closure")?;
    let level_export = require_object(source, "level_export", "closure.source")?;
    if require_hash_string(level_export, "package_sha256", "closure level_export")?
        != package_sha256
    {
        return Err(NativeWorldError::new(
            "closure level-export package binding mismatch",
        ));
    }
    let trace = require_object(source, "frame_trace", "closure.source")?;
    let bindings = require_object(trace, "bindings", "closure frame_trace")?;
    if require_hash_string(
        bindings,
        "level_export_package_sha256",
        "closure frame bindings",
    )? != package_sha256
    {
        return Err(NativeWorldError::new(
            "closure frame-trace package binding mismatch",
        ));
    }
    require_exact_u64(trace, "canonical_event_count", 76, "closure frame_trace")?;
    if require_value(trace, "byte_identical", "closure frame_trace")? != &Value::Bool(true)
        || require_value(trace, "independent_pass_runs", "closure frame_trace")?
            != &Value::Bool(true)
    {
        return Err(NativeWorldError::new(
            "closure frame trace is not independently byte-identical",
        ));
    }
    let closure_files = require_object(level_export, "files", "closure level_export")?;
    for (name, entry) in closure_files {
        let member_bytes = if name == "manifest.json" {
            manifest_bytes
        } else {
            member(members, name)?
        };
        let expected_size = require_value(
            as_object(entry, "closure file entry")?,
            "size",
            "closure file entry",
        )?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new("closure file size is not u64"))?;
        let expected_hash =
            require_hash_string(as_object(entry, "closure file entry")?, "sha256", name)?;
        if member_bytes.len() as u64 != expected_size || sha256_hex(member_bytes) != expected_hash {
            return Err(NativeWorldError::new(format!(
                "closure file binding mismatch for {name}"
            )));
        }
    }
    for required in [
        "manifest.json",
        "entities.bin",
        "presentation.bin",
        "runtime.bin",
    ] {
        if !closure_files.contains_key(required) {
            return Err(NativeWorldError::new(format!(
                "closure level-export binding omits {required}"
            )));
        }
    }

    let qualification = require_object(&object, "qualification", "closure")?;
    for (name, expected) in [
        ("canonical_frame_events_promoted", 76),
        ("unresolved_frame_used", 0),
        ("ambiguous_frame_used", 0),
    ] {
        require_exact_u64(qualification, name, expected, "closure qualification")?;
    }
    if require_value(
        qualification,
        "zero_unresolved_frame_used_claimed",
        "closure qualification",
    )? != &Value::Bool(true)
        || require_string(qualification, "frame_used", "closure qualification")?
            != "PASS_FINAL_ENTRY_TRACE"
    {
        return Err(NativeWorldError::new(
            "closure qualification does not prove zero unresolved frame use",
        ));
    }
    let sets = require_object(&object, "sets", "closure")?;
    let frame_used = require_object(sets, "frame_used", "closure sets")?;
    if require_string(frame_used, "status", "closure frame_used")? != "qualified_final_entry_trace"
        || !require_array(frame_used, "unresolved", "closure frame_used")?.is_empty()
    {
        return Err(NativeWorldError::new(
            "closure frame-used set is not fully resolved",
        ));
    }
    let assets = require_array(frame_used, "assets", "closure frame_used")?;
    if assets.len() != FRAME_ASSET_COUNT {
        return Err(NativeWorldError::new(format!(
            "closure must contain {FRAME_ASSET_COUNT} frame assets, got {}",
            assets.len()
        )));
    }
    let mut refs = Vec::with_capacity(FRAME_ASSET_COUNT);
    for (index, asset) in assets.iter().enumerate() {
        let asset = as_object(asset, "frame asset")?;
        let ordinal = as_u16(
            require_value(asset, "trace_event_ordinal", "frame asset")?,
            "trace_event_ordinal",
        )?;
        let expected_ordinal = u16::try_from(index + 1).expect("76 assets");
        if ordinal != expected_ordinal {
            return Err(NativeWorldError::new(format!(
                "frame asset ordinal mismatch: {ordinal}/{expected_ordinal}"
            )));
        }
        let event = require_value(asset, "trace_event", "frame asset")?.clone();
        let event_hash = sha256_hex(canonical_json(&event)?.as_bytes());
        let declared_event_hash =
            require_hash_string(asset, "trace_event_sha256", "frame asset")?.to_owned();
        if event_hash != declared_event_hash {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} event hash mismatch"
            )));
        }
        let content_address = require_string(asset, "content_address", "frame asset")?.to_owned();
        validate_content_address(&content_address)?;
        if content_address != format!("sha256:{declared_event_hash}") {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} content address does not bind its event"
            )));
        }
        if require_string(asset, "resolution", "frame asset")? != "unique" {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} is not uniquely resolved"
            )));
        }
        let owner_ids = require_array(asset, "owner_asset_ids", "frame asset")?;
        if owner_ids.is_empty() {
            return Err(NativeWorldError::new(format!(
                "frame asset {ordinal} has no owner asset relation"
            )));
        }
        let event_object = as_object(&event, "trace event")?;
        refs.push(AssetRef {
            asset_id: require_string(asset, "asset_id", "frame asset")?.to_owned(),
            content_address,
            owner_asset_ids: owner_ids
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| NativeWorldError::new("owner asset id is not a string"))
                })
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice(),
            observation: FrameObservation {
                ordinal,
                category: require_string(event_object, "category", "trace event")?.to_owned(),
                hook: require_string(event_object, "hook", "trace event")?.to_owned(),
                stage: event_object
                    .get("stage")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                selector: event_object.get("selector").cloned().unwrap_or(Value::Null),
                event_sha256: declared_event_hash,
                event,
            },
        });
    }
    Ok((expected_closure_hash, refs))
}

fn read_audio_cache_output(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(NativeWorldError::new(
            "audio cache output path is not a confined relative path",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve audio cache: {error}")))?;
    let candidate = root.join(relative_path);
    if fs::symlink_metadata(&candidate)
        .map_err(|error| NativeWorldError::new(format!("cannot inspect audio output: {error}")))?
        .file_type()
        .is_symlink()
    {
        return Err(NativeWorldError::new("audio cache output is a symlink"));
    }
    let resolved = candidate
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve audio output: {error}")))?;
    if !resolved.starts_with(&root) {
        return Err(NativeWorldError::new("audio cache output escapes its root"));
    }
    read_file(&resolved)
}

fn append_level_one_audio_refs(
    root: &Path,
    package_sha256: &str,
    closure_sha256: &str,
    refs: &mut Vec<AssetRef>,
) -> Result<(BTreeMap<u16, u32>, Option<u32>)> {
    let manifest_bytes = read_audio_cache_output(root, "manifest.json")?;
    if sha256_hex(&manifest_bytes) != AUDIO_CACHE_MANIFEST_SHA256 {
        return Err(NativeWorldError::new(
            "owner audio cache manifest is not the qualified Level 1 manifest",
        ));
    }
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid audio manifest JSON: {error}")))?;
    let manifest = as_object(&manifest, "audio cache manifest")?;
    if require_string(manifest, "schema", "audio cache manifest")? != AUDIO_CACHE_SCHEMA
        || require_string(manifest, "scope", "audio cache manifest")?
            != "PRIVATE_TRACK_P_OWNER_DATA"
        || require_hash_string(manifest, "cache_content_sha256", "audio cache manifest")?
            != AUDIO_CACHE_CONTENT_SHA256
    {
        return Err(NativeWorldError::new(
            "owner audio cache identity or private scope mismatch",
        ));
    }
    let bindings = require_object(manifest, "bindings", "audio cache manifest")?;
    if require_hash_string(
        bindings,
        "level_export_package_sha256",
        "audio cache bindings",
    )? != package_sha256
        || require_hash_string(
            bindings,
            "level_asset_closure_sha256",
            "audio cache bindings",
        )? != closure_sha256
    {
        return Err(NativeWorldError::new(
            "owner audio cache package/closure binding mismatch",
        ));
    }
    let mut outputs = BTreeMap::new();
    for output in require_array(manifest, "outputs", "audio cache manifest")? {
        let output = as_object(output, "audio cache output")?;
        let path = require_string(output, "path", "audio cache output")?.to_owned();
        let hash = require_hash_string(output, "sha256", "audio cache output")?.to_owned();
        let bytes = require_value(output, "bytes", "audio cache output")?
            .as_u64()
            .ok_or_else(|| NativeWorldError::new("audio cache output size is not u64"))?;
        if outputs.insert(path, (hash, bytes)).is_some() {
            return Err(NativeWorldError::new(
                "audio cache manifest has duplicate output paths",
            ));
        }
    }
    let assets = require_object(manifest, "assets", "audio cache manifest")?;
    let audio = require_object(assets, "level1_audio", "audio cache assets")?;
    let sound_path = require_string(audio, "sound_descriptor_path", "audio cache Level 1 assets")?;
    let music_path = require_string(audio, "music_descriptor_path", "audio cache Level 1 assets")?;
    let load_output = |path: &str| -> Result<Vec<u8>> {
        let (expected_hash, expected_bytes) = outputs
            .get(path)
            .ok_or_else(|| NativeWorldError::new(format!("audio output is undeclared: {path}")))?;
        let bytes = read_audio_cache_output(root, path)?;
        if bytes.len() as u64 != *expected_bytes || sha256_hex(&bytes) != *expected_hash {
            return Err(NativeWorldError::new(format!(
                "audio output hash/size mismatch: {path}"
            )));
        }
        Ok(bytes)
    };

    let sound_bytes = load_output(sound_path)?;
    let sound: Value = serde_json::from_slice(&sound_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid sound descriptor: {error}")))?;
    let sound = as_object(&sound, "sound descriptor")?;
    if require_string(sound, "schema", "sound descriptor")? != "mc2-level1-sound-bank/v1" {
        return Err(NativeWorldError::new("sound descriptor schema mismatch"));
    }
    require_exact_u64(sound, "bank", 1, "sound descriptor")?;
    require_exact_u64(sound, "quality", 822, "sound descriptor")?;
    require_exact_u64(sound, "quality_index", 3, "sound descriptor")?;
    let sound_rows = require_array(sound, "level1_assets", "sound descriptor")?;
    let expected_sound_ids = [1_u16, 2, 5, 12, 29, 31, 37, 39, 43, 46];
    if sound_rows.len() != expected_sound_ids.len() {
        return Err(NativeWorldError::new(
            "sound descriptor Level 1 mapping count changed",
        ));
    }
    let mut sound_refs = BTreeMap::new();
    for (row, expected_sound_id) in sound_rows.iter().zip(expected_sound_ids) {
        let row = as_object(row, "sound mapping")?;
        let sound_id = as_u16(require_value(row, "sound_id", "sound mapping")?, "sound_id")?;
        if sound_id != expected_sound_id {
            return Err(NativeWorldError::new("sound descriptor ordering changed"));
        }
        let (asset_id, content_hash, mapping_status) = match row.get("asset_path") {
            Some(Value::String(path)) => {
                let payload = load_output(path)?;
                let declared = require_hash_string(row, "asset_sha256", "sound mapping")?;
                if sha256_hex(&payload) != declared {
                    return Err(NativeWorldError::new(
                        "sound mapping does not bind its materialized payload",
                    ));
                }
                (
                    format!("owner_cache:{path}"),
                    declared.to_owned(),
                    "proven_Level1",
                )
            }
            Some(Value::Null) if sound_id == 12 => (
                "owner_cache:audio/sound-bank-1-822.json#sound_id=12".to_owned(),
                require_hash_string(row, "record_sha256", "sound mapping")?.to_owned(),
                "explicit_null",
            ),
            _ => {
                return Err(NativeWorldError::new(
                    "sound mapping has invalid materialization status",
                ));
            }
        };
        if require_string(row, "mapping_status", "sound mapping")? != mapping_status {
            return Err(NativeWorldError::new("sound mapping status changed"));
        }
        let event = json!({
            "category": "audio",
            "hook": "Level1SoundBank",
            "selector": {
                "bank": 1,
                "quality": 822,
                "sound_id": sound_id,
                "mapping_status": mapping_status,
            },
        });
        let event_sha256 = sha256_hex(canonical_json(&event)?.as_bytes());
        let index = u32::try_from(refs.len())
            .map_err(|_| NativeWorldError::new("audio asset-ref index exceeds u32"))?;
        refs.push(AssetRef {
            asset_id,
            content_address: format!("sha256:{content_hash}"),
            owner_asset_ids: vec![format!(
                "owner_data:SOUND/SOUND.DAT#bank=1;quality=822;sound_id={sound_id}"
            )]
            .into_boxed_slice(),
            observation: FrameObservation {
                ordinal: u16::try_from(index + 1)
                    .map_err(|_| NativeWorldError::new("audio ordinal exceeds u16"))?,
                category: "audio".to_owned(),
                hook: "Level1SoundBank".to_owned(),
                stage: Some("session_event".to_owned()),
                selector: event["selector"].clone(),
                event_sha256,
                event,
            },
        });
        sound_refs.insert(sound_id, index);
    }

    let music_bytes = load_output(music_path)?;
    let music: Value = serde_json::from_slice(&music_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid music descriptor: {error}")))?;
    let music = as_object(&music, "music descriptor")?;
    if require_string(music, "schema", "music descriptor")? != "mc2-level1-music-channel/v1" {
        return Err(NativeWorldError::new("music descriptor schema mismatch"));
    }
    require_exact_u64(music, "channel", 0, "music descriptor")?;
    require_exact_u64(music, "driver_index", 2, "music descriptor")?;
    require_exact_u64(music, "logical_track", 1, "music descriptor")?;
    require_exact_u64(music, "playback_slot", 0, "music descriptor")?;
    require_exact_u64(music, "loop_scan_pointer_slot", 1, "music descriptor")?;
    let wave_path = require_string(music, "unity_wave_path", "music descriptor")?;
    let wave = load_output(wave_path)?;
    let wave_hash = require_hash_string(music, "unity_wave_sha256", "music descriptor")?;
    if sha256_hex(&wave) != wave_hash {
        return Err(NativeWorldError::new(
            "music descriptor does not bind its Unity WAVE",
        ));
    }
    let event = json!({
        "category": "audio",
        "hook": "Level1MusicChannel",
        "selector": {
            "channel": 0,
            "driver_type": "f",
            "logical_track": 1,
            "playback_slot": 0,
            "loop_scan_pointer_slot": 1,
        },
    });
    let event_sha256 = sha256_hex(canonical_json(&event)?.as_bytes());
    let music_index = u32::try_from(refs.len())
        .map_err(|_| NativeWorldError::new("music asset-ref index exceeds u32"))?;
    refs.push(AssetRef {
        asset_id: format!("owner_cache:{wave_path}"),
        content_address: format!("sha256:{wave_hash}"),
        owner_asset_ids: vec![
            "owner_data:SOUND/MUSIC.DAT#channel=0;driver=f;logical_track=1".to_owned(),
        ]
        .into_boxed_slice(),
        observation: FrameObservation {
            ordinal: u16::try_from(music_index + 1)
                .map_err(|_| NativeWorldError::new("music ordinal exceeds u16"))?,
            category: "audio".to_owned(),
            hook: "Level1MusicChannel".to_owned(),
            stage: Some("session_event".to_owned()),
            selector: event["selector"].clone(),
            event_sha256,
            event,
        },
    });
    Ok((sound_refs, Some(music_index)))
}

fn flight_state_json(state: &FlightState) -> Value {
    json!({
        "entity_index": state.entity_index,
        "position": {
            "x": state.position.x,
            "y": state.position.y,
            "z": state.position.z,
        },
        "yaw": state.yaw,
        "display_pitch": state.display_pitch,
        "effective_pitch": state.effective_pitch,
        "roll_accumulator": state.roll_accumulator,
        "pitch_accumulator": state.pitch_accumulator,
        "target_speed": state.target_speed,
        "speed_changed_flag": state.speed_changed_flag,
        "actual_speed": state.actual_speed,
        "strafe_speed": state.strafe_speed,
        "roll_delta": state.roll_delta,
        "pitch_delta": state.pitch_delta,
        "recoil_speed": state.recoil_speed,
        "recoil_yaw": state.recoil_yaw,
        "impulse_x": state.impulse_x,
        "impulse_y": state.impulse_y,
        "impulse_z": state.impulse_z,
        "entity_fov": state.entity_fov,
        "water_counter": state.water_counter,
        "speed_modifier": state.speed_modifier,
        "speed_modifier_counter": state.speed_modifier_counter,
        "immobilize_counter": state.immobilize_counter,
        "immobilize_subcounter": state.immobilize_subcounter,
        "tether_prior_distance": state.tether_prior_distance,
        "tether_target_index": state.tether_target_index,
        "tether_holder_index": state.tether_holder_index,
        "tether_tier": state.tether_tier,
        "speed_spell_holder_index": state.speed_spell_holder_index,
        "entity_phase": state.entity_phase,
        "ambient_loop_5_distance": state.ambient_loop_5_distance,
        "ambient_loop_31_distance": state.ambient_loop_31_distance,
        "player_color_index": state.player_color_index,
        "music_countdown": state.music_countdown,
        "recovery_latch": state.recovery_latch,
        "skip_flight_once": state.skip_flight_once,
        "tick": state.tick,
    })
}

fn require_mapping(
    mappings: &serde_json::Map<String, Value>,
    name: &str,
    kind: &str,
    file: &str,
    record_name: Option<&str>,
    offset: usize,
    encoding: &str,
) -> Result<()> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != kind
        || require_string(mapping, "file", name)? != file
        || require_string(mapping, "encoding", name)? != encoding
    {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} kind/file/encoding mismatch"
        )));
    }
    require_exact_u64(mapping, "offset", offset as u64, name)?;
    match record_name {
        Some(expected) if require_string(mapping, "record", name)? != expected => {
            return Err(NativeWorldError::new(format!(
                "reset mapping {name} record selector mismatch"
            )));
        }
        None if mapping.contains_key("record") => {
            return Err(NativeWorldError::new(format!(
                "reset mapping {name} has an unexpected record selector"
            )));
        }
        _ => {}
    }
    Ok(())
}

fn require_mapping_scalar(
    mapping: &serde_json::Map<String, Value>,
    file: &str,
    offset: usize,
    encoding: &str,
    name: &str,
) -> Result<()> {
    if require_string(mapping, "file", name)? != file
        || require_string(mapping, "encoding", name)? != encoding
    {
        return Err(NativeWorldError::new(format!(
            "{name} source mapping mismatch"
        )));
    }
    require_exact_u64(mapping, "offset", offset as u64, name)
}

fn mapped_player_i32(
    mappings: &serde_json::Map<String, Value>,
    player: &[u8],
    name: &str,
    offset: usize,
) -> Result<i32> {
    require_mapping(
        mappings,
        name,
        "exported_field",
        "players.bin",
        Some("local_player_slot"),
        offset,
        "i32_le",
    )?;
    read_i32(player, offset, name)
}

fn zero_i64(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i64> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != "zero_at_seam" {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} must be zero_at_seam"
        )));
    }
    let value = require_value(mapping, "value", name)?
        .as_i64()
        .ok_or_else(|| NativeWorldError::new(format!("{name} seam value is not an integer")))?;
    if value != 0 {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} seam value is not zero"
        )));
    }
    Ok(value)
}

fn zero_i8(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i8> {
    i8::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i8")))
}

fn zero_i16(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i16> {
    i16::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i16")))
}

fn zero_i32(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<i32> {
    i32::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit i32")))
}

fn zero_u16(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<u16> {
    u16::try_from(zero_i64(mappings, name)?)
        .map_err(|_| NativeWorldError::new(format!("{name} does not fit u16")))
}

fn zero_bool(mappings: &serde_json::Map<String, Value>, name: &str) -> Result<bool> {
    let mapping = require_object(mappings, name, "field_mappings")?;
    if require_string(mapping, "kind", name)? != "zero_at_seam"
        || require_value(mapping, "value", name)? != &Value::Bool(false)
    {
        return Err(NativeWorldError::new(format!(
            "reset mapping {name} must be false zero_at_seam"
        )));
    }
    Ok(false)
}

fn validate_header(
    bytes: &[u8],
    magic: [u8; 4],
    version: u16,
    record_size: u16,
    count: u32,
    name: &str,
) -> Result<()> {
    if bytes.len() != 12 + count as usize * usize::from(record_size)
        || bytes.get(..4) != Some(magic.as_slice())
        || read_u16(bytes, 4, "component version")? != version
        || read_u16(bytes, 6, "component record size")? != record_size
        || read_u32(bytes, 8, "component count")? != count
    {
        return Err(NativeWorldError::new(format!(
            "{name} header/version/record-size/count/extent mismatch"
        )));
    }
    Ok(())
}

fn relation_one_based(value: u32, maximum: usize, name: &str) -> Result<Option<u16>> {
    if value == 0 {
        Ok(None)
    } else {
        one_based(value, maximum, name).map(Some)
    }
}

fn one_based(value: u32, maximum: usize, name: &str) -> Result<u16> {
    let maximum_u32 = u32::try_from(maximum)
        .map_err(|_| NativeWorldError::new(format!("{name} maximum does not fit u32")))?;
    if value == 0 || value > maximum_u32 {
        Err(NativeWorldError::new(format!(
            "{name} value {value} is outside 1..={maximum}"
        )))
    } else {
        u16::try_from(value - 1)
            .map_err(|_| NativeWorldError::new(format!("{name} does not fit u16")))
    }
}

fn record<'a>(
    bytes: &'a [u8],
    start: usize,
    size: usize,
    index: usize,
    name: &str,
) -> Result<&'a [u8]> {
    let offset = start
        .checked_add(
            index
                .checked_mul(size)
                .ok_or_else(|| NativeWorldError::new(format!("{name} record offset overflow")))?,
        )
        .ok_or_else(|| NativeWorldError::new(format!("{name} record offset overflow")))?;
    bytes
        .get(offset..offset + size)
        .ok_or_else(|| NativeWorldError::new(format!("{name} record {index} is truncated")))
}

fn member<'a>(members: &'a BTreeMap<String, Vec<u8>>, name: &str) -> Result<&'a [u8]> {
    members
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| NativeWorldError::new(format!("package member {name} is unavailable")))
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path)
        .map_err(|error| NativeWorldError::new(format!("cannot read {}: {error}", path.display())))
}

fn read_u8(bytes: &[u8], offset: usize, name: &str) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))
}

fn read_i8(bytes: &[u8], offset: usize, name: &str) -> Result<i8> {
    read_u8(bytes, offset, name).map(u8::cast_signed)
}

fn read_u16(bytes: &[u8], offset: usize, name: &str) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, name: &str) -> Result<i16> {
    read_u16(bytes, offset, name).map(u16::cast_signed)
}

fn read_u32(bytes: &[u8], offset: usize, name: &str) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_i32(bytes: &[u8], offset: usize, name: &str) -> Result<i32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| NativeWorldError::new(format!("{name} is out of bounds")))?;
    Ok(i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn require_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Value> {
    object
        .get(name)
        .ok_or_else(|| NativeWorldError::new(format!("{context} is missing {name}")))
}

fn require_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    as_object(require_value(object, name, context)?, name)
}

fn require_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Vec<Value>> {
    require_value(object, name, context)?
        .as_array()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not an array")))
}

fn require_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str> {
    require_value(object, name, context)?
        .as_str()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not a string")))
}

fn require_hash_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str> {
    let value = require_string(object, name, context)?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NativeWorldError::new(format!(
            "{context}.{name} is not a SHA-256 hex digest"
        )));
    }
    Ok(value)
}

fn require_exact_u64(
    object: &serde_json::Map<String, Value>,
    name: &str,
    expected: u64,
    context: &str,
) -> Result<()> {
    let actual = require_value(object, name, context)?
        .as_u64()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not u64")))?;
    if actual == expected {
        Ok(())
    } else {
        Err(NativeWorldError::new(format!(
            "{context}.{name} must be {expected}, got {actual}"
        )))
    }
}

fn require_exact_i64(
    object: &serde_json::Map<String, Value>,
    name: &str,
    expected: i64,
    context: &str,
) -> Result<()> {
    let actual = require_value(object, name, context)?
        .as_i64()
        .ok_or_else(|| NativeWorldError::new(format!("{context}.{name} is not i64")))?;
    if actual == expected {
        Ok(())
    } else {
        Err(NativeWorldError::new(format!(
            "{context}.{name} must be {expected}, got {actual}"
        )))
    }
}

fn as_object<'a>(value: &'a Value, context: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not an object")))
}

fn as_object_mut<'a>(
    value: &'a mut Value,
    context: &str,
) -> Result<&'a mut serde_json::Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not an object")))
}

fn as_u8(value: &Value, context: &str) -> Result<u8> {
    value
        .as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u8")))
}

fn as_u16(value: &Value, context: &str) -> Result<u16> {
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u16")))
}

fn as_u32(value: &Value, context: &str) -> Result<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| NativeWorldError::new(format!("{context} is not u32")))
}

fn parse_u64_hex(value: &str) -> Result<u64> {
    u64::from_str_radix(value, 16)
        .map_err(|error| NativeWorldError::new(format!("invalid u64 hex hash: {error}")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("null".to_owned()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => serde_json::to_string(value)
            .map_err(|error| NativeWorldError::new(format!("cannot encode JSON string: {error}"))),
        Value::Array(values) => {
            let encoded = values
                .iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("[{}]", encoded.join(",")))
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut fields = Vec::with_capacity(keys.len());
            for key in keys {
                let encoded_key = serde_json::to_string(key).map_err(|error| {
                    NativeWorldError::new(format!("cannot encode JSON key: {error}"))
                })?;
                fields.push(format!("{encoded_key}:{}", canonical_json(&object[key])?));
            }
            Ok(format!("{{{}}}", fields.join(",")))
        }
    }
}

fn validate_content_address(value: &str) -> Result<()> {
    let Some(hash) = value.strip_prefix("sha256:") else {
        return Err(NativeWorldError::new("asset content address is not sha256"));
    };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(NativeWorldError::new(
            "asset content address has an invalid digest",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{regression_state::RegressionStateSequence, source_projection_hash};
    use std::path::PathBuf;

    const OWNED_LEVEL_ONE_STATE_SEQUENCE: &[u8] = include_bytes!(
        "../../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1/sequence-002285FF-00356038.bin"
    );

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        (
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
    }

    #[test]
    fn package_m_loads_the_exact_native_world() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        assert_eq!(world.metadata.map_type, MapType::Night);
        assert_eq!(world.profile, FlightProfile::OUTDOOR);
        assert_eq!(
            world.terrain.ceiling_height(world.reset_state.position),
            None
        );
        assert_eq!(world.entities.len(), 1_000);
        assert_eq!(world.players.len(), 8);
        assert_eq!(world.asset_refs.len(), 76);
        assert_eq!(world.runtime.objective_textbox_ticks, 33);
        assert_eq!(world.runtime.objectives.len(), 8);
        assert_eq!(
            world.runtime.objectives[0],
            RuntimeObjective {
                is_level_end: 0,
                text_selector: 0,
                done: 0,
                stage_selectors: [1, 1, 1, 1, 1, 0, 0, 0],
            }
        );
        assert_eq!(
            world.package_sha256(),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
        );
        assert_eq!(
            world.closure_sha256(),
            "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12"
        );
        assert_eq!(state_hash(world.reset_state()), 0x9da4_bc91_af58_f3ce);
        assert_eq!(
            conformance_hash(world.reset_state(), world.reset_context()),
            0x77b9_6b82_492d_6f4b
        );
        assert_eq!(
            world.reset_state.position,
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            }
        );
        assert_eq!(world.reset_state.entity_phase, 153);
        assert_eq!(world.reset_state.ambient_loop_5_distance, 2_048);
        assert_eq!(world.reset_state.ambient_loop_31_distance, 1_746);
    }

    #[test]
    fn qualified_owner_cache_adds_only_descriptor_backed_level_one_audio_refs() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut refs = Vec::new();
        let (sound, music) = append_level_one_audio_refs(
            &private.join("generated/level1-unity-asset-cache"),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
            "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12",
            &mut refs,
        )
        .expect("qualified owner audio cache");
        assert_eq!(refs.len(), 11);
        assert_eq!(
            sound.keys().copied().collect::<Vec<_>>(),
            [1, 2, 5, 12, 29, 31, 37, 39, 43, 46]
        );
        assert_eq!(sound[&12], 3);
        assert_eq!(
            refs[3].asset_id,
            "owner_cache:audio/sound-bank-1-822.json#sound_id=12"
        );
        assert_eq!(music, Some(10));
        assert_eq!(
            refs[10].content_address,
            "sha256:03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
        );
    }

    #[test]
    fn authority_is_narrow_and_explicit() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        assert_eq!(
            world.entities[152].authority,
            EntityAuthority::PlayerFlightLive
        );
        assert_eq!(
            world.entities[153].authority,
            EntityAuthority::SpellActivationLive
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::PlayerFlightLive)
                .count(),
            1
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::SpellActivationLive)
                .count(),
            1
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::FrozenObserved)
                .count(),
            142
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::GoatLive)
                .count(),
            LEVEL_ONE_GOAT_INDICES.len()
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| entity.authority == EntityAuthority::Inactive)
                .count(),
            823
        );
    }

    #[test]
    fn native_world_delegates_exact_legacy_terrain_queries() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        for position in [
            Position { x: 0, y: 0, z: 0 },
            Position {
                x: 19_840,
                y: 56_960,
                z: 5_024,
            },
            Position {
                x: 0xffff,
                y: 0xffff,
                z: -1,
            },
            Position {
                x: 0x7f55,
                y: 0x80aa,
                z: 1_234,
            },
        ] {
            assert_eq!(
                world.ground_height(position),
                world.terrain.ground_height(position)
            );
            assert_eq!(
                world.material_mask(position),
                world.terrain.material_mask(position)
            );
        }
    }

    #[test]
    fn sessions_are_independent_and_reset_to_package_projection() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let mut first = world.session();
        let second = world.session();
        assert_eq!(
            conformance_hash(world.reset_state(), world.reset_context()),
            0x77b9_6b82_492d_6f4b,
            "the preserved package reset contract must not absorb presentation state"
        );
        assert_eq!(
            conformance_hash(&first.state, &first.context),
            0x00cc_7b79_9d69_17fc,
            "the additive Level 1 session-audio contract is independently pinned"
        );
        assert_eq!(
            first.context.effects,
            Some(FlightEffectsContext {
                local_player_index: 0,
                periodic_spell_effect: None,
                ..FlightEffectsContext::default()
            })
        );
        first.step(PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        });
        assert_ne!(first.state, second.state);
        first.reset();
        assert_eq!(first.state, *world.reset_state());
        assert_eq!(first.context, world.session_context());
    }

    #[test]
    fn neutral_session_replay_matches_original_idle_source_projection() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let sequence =
            RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).expect("state sequence");
        let mut session = world.session();
        for frame_index in 0..sequence.frame_count() {
            if frame_index > 0 {
                session.step(PlayerCommand::default());
            }
            if matches!(frame_index, 0 | 8 | 19) {
                let frame = sequence.frame(frame_index).expect("captured frame");
                let original = frame.project_player_flight(frame.local_player_slot());
                assert_eq!(
                    source_projection_hash(&session.state),
                    state_hash(&original),
                    "source projection at idle frame {frame_index}"
                );
            }
        }
    }

    #[test]
    fn corrupt_package_hash_version_relation_and_bounds_fail_closed() {
        let (package, closure) = fixture_paths();
        for mutation in [
            Mutation::ManifestSchema,
            Mutation::EntityMagic,
            Mutation::ResetMapping,
        ] {
            let temp = TempPackage::copy_from(&package, &closure);
            temp.mutate(mutation);
            assert!(
                NativeWorld::load(&temp.package, &temp.closure).is_err(),
                "mutation {mutation:?} must fail"
            );
        }
    }

    #[test]
    fn corrupt_component_version_relation_and_bounds_reach_their_parser_gates() {
        let (package, _) = fixture_paths();

        let mut players = fs::read(package.join("players.bin")).unwrap();
        players[4] ^= 0xff;
        assert!(parse_players(&players).is_err());

        let mut entities = fs::read(package.join("entities.bin")).unwrap();
        let relation_offset = 12 + 152 * ENTITY_RECORD_BYTES + 164;
        entities[relation_offset..relation_offset + 4].copy_from_slice(&999_u32.to_le_bytes());
        assert!(parse_entities(&entities).is_err());

        let mut players = fs::read(package.join("players.bin")).unwrap();
        players[16..18].copy_from_slice(&1_000_u16.to_le_bytes());
        assert!(parse_players(&players).is_err());

        let mut runtime = fs::read(package.join("runtime.bin")).unwrap();
        let stage_relation_offset = 192 + 6;
        runtime[stage_relation_offset] = 9;
        assert!(parse_runtime(&runtime).is_err());

        let mut presentation = fs::read(package.join("presentation.bin")).unwrap();
        presentation[4] ^= 0xff;
        assert!(parse_presentation(&presentation).is_err());
    }

    #[test]
    fn corrupt_closure_hash_binding_and_unresolved_frame_use_fail_closed() {
        let (package, closure) = fixture_paths();
        for mutation in [
            ClosureMutation::Hash,
            ClosureMutation::PackageBinding,
            ClosureMutation::Unresolved,
            ClosureMutation::AssetOrdinal,
        ] {
            let temp = TempPackage::copy_from(&package, &closure);
            temp.mutate_closure(mutation);
            assert!(
                NativeWorld::load(&temp.package, &temp.closure).is_err(),
                "closure mutation {mutation:?} must fail"
            );
        }
    }

    #[test]
    fn v1_or_incomplete_reset_is_rejected_instead_of_synthesizing_ambient() {
        let (package, closure) = fixture_paths();
        let temp = TempPackage::copy_from(&package, &closure);
        let manifest_path = temp.package.join("manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["schema"] = json!("mc2-owner-level-export/v1");
        manifest["reset_projection"]["field_mappings"]
            .as_object_mut()
            .unwrap()
            .remove("ambient_loop_31_distance");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert!(NativeWorld::load(&temp.package, &temp.closure).is_err());
    }

    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        ManifestSchema,
        EntityMagic,
        ResetMapping,
    }

    #[derive(Clone, Copy, Debug)]
    enum ClosureMutation {
        Hash,
        PackageBinding,
        Unresolved,
        AssetOrdinal,
    }

    struct TempPackage {
        root: PathBuf,
        package: PathBuf,
        closure: PathBuf,
    }

    impl TempPackage {
        fn copy_from(package: &Path, closure: &Path) -> Self {
            let root = std::env::temp_dir().join(format!(
                "mc2-native-world-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let output = root.join("package");
            fs::create_dir_all(&output).unwrap();
            fs::copy(package.join("manifest.json"), output.join("manifest.json")).unwrap();
            for (name, _) in PACKAGE_MEMBERS {
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

        fn mutate(&self, mutation: Mutation) {
            match mutation {
                Mutation::ManifestSchema | Mutation::ResetMapping => {
                    let path = self.package.join("manifest.json");
                    let mut manifest: Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    if matches!(mutation, Mutation::ManifestSchema) {
                        manifest["schema"] = json!("mc2-owner-level-export/v999");
                    } else {
                        manifest["reset_projection"]["field_mappings"]["ambient_loop_31_distance"]
                            ["offset"] = json!(560);
                    }
                    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
                }
                Mutation::EntityMagic => self.flip_byte("entities.bin", 0),
            }
        }

        fn mutate_closure(&self, mutation: ClosureMutation) {
            let mut closure: Value =
                serde_json::from_slice(&fs::read(&self.closure).unwrap()).unwrap();
            match mutation {
                ClosureMutation::Hash => closure["closure_sha256"] = json!("0".repeat(64)),
                ClosureMutation::PackageBinding => {
                    closure["source"]["level_export"]["package_sha256"] = json!("0".repeat(64));
                }
                ClosureMutation::Unresolved => {
                    closure["sets"]["frame_used"]["unresolved"] = json!(["not-resolved"]);
                }
                ClosureMutation::AssetOrdinal => {
                    closure["sets"]["frame_used"]["assets"][0]["trace_event_ordinal"] = json!(2);
                }
            }
            if !matches!(mutation, ClosureMutation::Hash) {
                closure.as_object_mut().unwrap().remove("closure_sha256");
                closure["closure_sha256"] =
                    json!(sha256_hex(canonical_json(&closure).unwrap().as_bytes()));
            }
            fs::write(&self.closure, serde_json::to_vec_pretty(&closure).unwrap()).unwrap();
        }

        fn flip_byte(&self, name: &str, offset: usize) {
            let path = self.package.join(name);
            let mut bytes = fs::read(&path).unwrap();
            bytes[offset] ^= 0xff;
            fs::write(path, bytes).unwrap();
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
