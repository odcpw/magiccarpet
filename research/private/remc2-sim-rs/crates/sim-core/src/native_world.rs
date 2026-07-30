//! Strict owner-package loader for an immutable native simulation world.
//!
//! The loader consumes an explicit level-export directory and an explicit
//! asset-closure path. It never searches for owner data, embeds regression
//! bytes, or treats observed entities as simulated entities.

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    FlightContext, FlightEffectsContext, FlightProfile, FlightState, FlightWorld, PlayerCommand,
    Position, SOURCE_SPELL_INDEX_MAP, SpellFireContext, conformance_hash,
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_RENDER_PARAMETER_INDEX, GOAT_STAGE_POINT,
        GoatBehaviorConfig, GoatPlayerTarget, GoatState, GoatStepEvent, LEVEL_ONE_GOAT_INDICES,
        LiveWorldState,
    },
    goat_tmap::{
        GOAT_TMAP_SELECTORS, GOAT_TMAP_STATE_COUNT, GoatTmapBank, GoatTmapClocks, GoatTmapFrame,
        GoatTmapSelectorBank,
    },
    house::{HOUSE_CLASS, HOUSE_MODEL, HouseResidentEntry, HouseState, HouseStepEvent},
    legacy_terrain::{
        LEGACY_MAP_CELLS, LegacyTerrain, interpolate_cave, interpolate_ground, material_mask,
    },
    level1_mission::{
        ARCHER_BASE_TMAP, ARCHER_LOGICAL_HEIGHT, ARCHER_RENDER_INDEX, ARCHER_RENDER_MODE,
        CASTLE_SPELL_FAMILY, CastleBuildFootprint, CastleTerrainUpdate, FIREBALL_BASE_TMAP,
        FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_LOGICAL_HEIGHT, FIREBALL_IMPACT_RENDER_INDEX,
        FIREBALL_IMPACT_RENDER_MODE, FIREBALL_LOGICAL_HEIGHT, FIREBALL_RENDER_INDEX,
        FIREBALL_RENDER_MODE, FIREFLY_BASE_TMAP, FIREFLY_LOGICAL_HEIGHT, FIREFLY_RENDER_INDEX,
        FIREFLY_RENDER_MODE, GROUND_FIRE_BASE_TMAP, GROUND_FIRE_FIRST_RENDER_INDEX,
        GROUND_FIRE_LOGICAL_HEIGHTS, GROUND_FIRE_PRESENTATION_BYTE_11,
        GROUND_FIRE_PRESENTATION_SPEEDS, GROUND_FIRE_RENDER_MODE, HouseResidentRequest,
        Level1AmbientEntity, Level1CastlePlacementBlocker, Level1CollisionTarget,
        Level1MissionState, Level1MissionTemplate, Level1ResetSourceSlot,
        Level1SourceCollisionTarget, Level1SourceEffect, Level1SourceEntityAuthority,
        Level1SourceEntityView, MANA_CARRIER_BASE_TMAP, MANA_CARRIER_LOGICAL_HEIGHT,
        MANA_CARRIER_RENDER_INDEX, MANA_CARRIER_RENDER_MODE, POSSESSION_ATTRACTION_BASE_TMAP,
        POSSESSION_ATTRACTION_LOGICAL_HEIGHT, POSSESSION_ATTRACTION_RENDER_INDEX,
        POSSESSION_ATTRACTION_RENDER_MODE, POSSESSION_IMPACT_BASE_TMAP,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, SPELL_JAR_BASE_TMAP, SPELL_JAR_LOGICAL_HEIGHT,
        SPELL_JAR_RENDER_INDEX, SPELL_JAR_RENDER_MODE,
    },
    renderable_map_chain::RenderableMapSeed,
    spell_activation::{SpellActivationHolder, SpellCasterState},
    state_hash, step_entity_body_with_context, step_outer_turn_with_context,
    villager::{
        LEVEL_ONE_VILLAGER_INDICES, VILLAGER_CLASS, VILLAGER_MODEL, VillagerBehaviorConfig,
        VillagerState,
    },
};

const MANIFEST_SCHEMA: &str = "mc2-owner-level-export/v2";
const CLOSURE_SCHEMA: &str = "mc2-level-asset-closure/v2";
const AUDIO_CACHE_SCHEMA: &str = "mc2-owner-unity-asset-cache/v1";
const AUDIO_CACHE_MANIFEST_SHA256: &str =
    "e3aaf1cf03e390850c5da44ed4b3880352b425bd063657e3cdbbcb8086a4d7ee";
const AUDIO_CACHE_CONTENT_SHA256: &str =
    "d299990e300117d065eb3a75008f6a69911b1ba356e4e06452c0a826450a01e5";
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
    pub vga_resolution_selector: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAuthority {
    Inactive,
    FrozenObserved,
    PlayerFlightLive,
    SpellActivationLive,
    GoatLive,
    VillagerLive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub index: u16,
    /// Original `id_0x1A_26`, used as the sample arbitration flag for
    /// positional sound IDs 37 and 39. This is distinct from the pool slot.
    pub source_id: u16,
    pub authority: EntityAuthority,
    pub active: bool,
    pub max_life: i32,
    pub life: i32,
    pub next_relation: Option<u16>,
    pub class: u8,
    pub model: u8,
    pub control_flags: u32,
    pub entity_rng: u16,
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
    /// Signed source field at record offset 94. Active renderable rows use a
    /// non-negative index into the 347-row particle presentation table.
    pub particle_parameter_index: Option<u16>,
    pub parameter_relation: Option<u16>,
    pub special_relation: EntitySpecialRelation,
    record: Box<[u8]>,
}

impl EntitySnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn with_record_byte(mut self, offset: usize, value: u8) -> Self {
        self.record[offset] = value;
        self
    }

    /// Raw zero-terminated `oldMapEntity_0x16_22` link used by the outdoor
    /// terrain-cell painter traversal.
    #[must_use]
    pub fn old_map_entity(&self) -> u16 {
        u16::from_le_bytes([self.record[22], self.record[23]])
    }

    /// Raw reset `animationFrame_0x60_96` used by fixed and base-plus-frame
    /// sprite families.
    #[must_use]
    pub fn animation_frame(&self) -> i8 {
        i8::from_le_bytes([self.record[96]])
    }

    #[must_use]
    fn castle_placement_blocker(&self) -> Level1CastlePlacementBlocker {
        Level1CastlePlacementBlocker {
            source_entity_index: self.index,
            position: self.position,
            // Exported axis_4d pitch/roll retain original record offsets
            // 0x58/0x5a. They are distinct from the orientation fields.
            half_x: i16::from_le_bytes([self.record[88], self.record[89]]),
            half_y: i16::from_le_bytes([self.record[90], self.record[91]]),
        }
    }

    #[must_use]
    pub fn collision_target(&self) -> Level1CollisionTarget {
        Level1CollisionTarget {
            entity_index: self.index,
            position: self.position,
            collision_vertical_offset: i16::from_le_bytes([self.record[86], self.record[87]]),
            collision_half_width: i16::from_le_bytes([self.record[88], self.record[89]]),
            collision_half_height: i16::from_le_bytes([self.record[92], self.record[93]]),
            active: self.active,
        }
    }

    fn ambient_entity(&self) -> Option<Level1AmbientEntity> {
        if !self.active
            || self.class != 10
            || !matches!(
                (self.model, self.action),
                (0, 0) | (1, 1) | (14, 14) | (60, 65)
            )
        {
            return None;
        }
        let read_u16 = |offset| u16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_i16 = |offset| i16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_u32 = |offset| {
            u32::from_le_bytes([
                self.record[offset],
                self.record[offset + 1],
                self.record[offset + 2],
                self.record[offset + 3],
            ])
        };
        Some(Level1AmbientEntity {
            source_entity_index: self.index,
            parent_entity_index: 0,
            source_id: self.source_id,
            position: self.position,
            max_life: self.max_life,
            life: self.life,
            control_flags: self.control_flags,
            search_radius: read_u32(20),
            entity_rng: read_u16(24),
            class: self.class,
            model: self.model,
            action: self.action,
            phase: self.phase,
            yaw: self.yaw,
            // Particle `array_0x52` axes, distinct from orientation yaw/pitch.
            pitch: read_i16(88),
            fov: read_i16(92),
            vertical_velocity: read_i16(48),
            actual_speed: self.actual_speed,
            max_speed: read_i16(138),
            render_parameter_index: self.particle_parameter_index.unwrap_or(0),
            animation_frame: self.animation_frame(),
            animation_limit: i8::from_le_bytes([self.record[97]]),
            active: true,
            map_linked: self.control_flags & 4 != 0,
            last_material_effect: None,
            collision_probe_ran: false,
        })
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
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_spell_ui_index: u8,
    pub selected_subspell_index: i8,
    pub enabled_spell_holders: [u16; PLAYER_SPELL_COUNT],
    pub spell_hand_eligibility: [u8; PLAYER_SPELL_COUNT],
    pub family_selected_tiers: [i8; PLAYER_SPELL_COUNT],
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

/// Mutable terrain owned by exactly one play session.
///
/// `NativeWorld` remains immutable and shareable. Castle construction changes
/// these planes immediately after the action-44 source slot so subsequent
/// source entities, flight, collision, and rendering all read the same state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionTerrain {
    map_type: MapType,
    planes: TerrainPlanes,
    terrain_rng: u16,
    mutation_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HouseRemovalResident {
    pub position: Position,
    pub model: u8,
    pub action: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseRemovalOutcome {
    pub final_rng: u16,
    pub final_occupants: i32,
    pub residents: Vec<HouseRemovalResident>,
}

impl SessionTerrain {
    #[must_use]
    pub fn from_world(world: &NativeWorld) -> Self {
        Self {
            map_type: world.metadata.map_type,
            planes: world.terrain_planes.clone(),
            // Exact post-generation `rand2_17B4E0` observed at the qualified
            // Package-M export seam. Castle topology variants continue this
            // independent 16-bit terrain stream.
            terrain_rng: 39_663,
            mutation_sequence: 0,
        }
    }

    #[must_use]
    pub const fn planes(&self) -> &TerrainPlanes {
        &self.planes
    }

    #[must_use]
    pub fn ground_heights(&self) -> &[u8] {
        &self.planes.ground_heights
    }

    #[must_use]
    pub const fn terrain_rng(&self) -> u16 {
        self.terrain_rng
    }

    #[must_use]
    pub const fn mutation_sequence(&self) -> u64 {
        self.mutation_sequence
    }

    /// Canonical content hash for the six mutable source terrain planes.
    ///
    /// FNV-1a is applied to the schema domain, little-endian dimensions, five
    /// byte planes, then every signed entity index in little-endian form.
    /// The mutation sequence is intentionally separate from content identity.
    #[must_use]
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut append = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        append(b"mc2-session-terrain-v1\0");
        append(&256_u16.to_le_bytes());
        append(&256_u16.to_le_bytes());
        append(&self.planes.terrain_types);
        append(&self.planes.ground_heights);
        append(&self.planes.shading);
        append(&self.planes.angles);
        append(&self.planes.secondary_heights);
        for entity_index in &self.planes.entity_indices {
            append(&entity_index.to_le_bytes());
        }
        hash
    }

    /// Apply one source-ordered BUILD[1] terrain update.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed timer. Mutation is transactional: an
    /// error leaves all six terrain planes, the terrain RNG, and the mutation
    /// sequence unchanged.
    pub fn apply_castle_update(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if update.is_ambient() {
            return self.apply_ambient_update(update);
        }
        let mut staged = self.clone();
        staged.apply_castle_update_inner(update, footprint)?;
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    pub fn apply_ambient_update(
        &mut self,
        update: CastleTerrainUpdate,
    ) -> std::result::Result<(), String> {
        let mut staged = self.clone();
        match update {
            CastleTerrainUpdate::AmbientMaterial {
                position,
                in_type,
                target_type,
            } => {
                let x = position.x.wrapping_add(128).to_be_bytes()[0];
                let y = position.y.wrapping_add(128).to_be_bytes()[0];
                apply_ambient_material(
                    &mut staged.planes,
                    x,
                    y,
                    in_type,
                    target_type,
                    staged.map_type,
                )?;
            }
            CastleTerrainUpdate::AmbientCollision {
                position,
                height_delta,
            } => apply_ambient_collision(
                &mut staged.planes,
                position,
                height_delta,
                staged.map_type,
                &mut staged.terrain_rng,
            ),
            _ => {
                return Err("non-ambient update requires a Castle BUILD footprint".to_owned());
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    /// Remove one source action-53 house BUILD footprint.
    ///
    /// Resident selection and height restoration share the house RNG and must
    /// therefore run in the same row-major loop. Terrain topology retains its
    /// independent session RNG. The returned resident requests are already in
    /// exact allocation order.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed footprint. Mutation is transactional.
    pub fn apply_house_removal(
        &mut self,
        position: Position,
        mut house_rng: u16,
        mut occupants: i32,
        produces_residents: bool,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<HouseRemovalOutcome, String> {
        let expected = usize::from(footprint.width) * usize::from(footprint.height);
        if footprint.cells().len() != expected {
            return Err(format!(
                "house BUILD {} has {} cells, expected {expected}",
                footprint.index,
                footprint.cells().len()
            ));
        }
        let mut staged = self.clone();
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);
        let corner_height =
            |x: u8, y: u8| i16::from(staged.planes.ground_heights[terrain_index(x, y)]);
        let z_coefficient = (corner_height(start_x, start_y)
            + corner_height(start_x.wrapping_add(footprint.width), start_y)
            + corner_height(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )
            + corner_height(start_x, start_y.wrapping_add(footprint.height)))
            >> 2;
        let mut residents = Vec::new();
        let mut nonempty_ordinal = 0_u32;
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell == [0xff, 0xff] {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                nonempty_ordinal = nonempty_ordinal.wrapping_add(1);
                let resident_z = if nonempty_ordinal & 7 == 0 {
                    z_coefficient.wrapping_sub(10).wrapping_mul(32)
                } else {
                    z_coefficient.wrapping_mul(32)
                };
                if occupants > 0 {
                    occupants -= 1;
                    if produces_residents {
                        let (model, action) = if occupants == 0 {
                            (12, 97)
                        } else if occupants < 4 {
                            (4, 33)
                        } else {
                            house_rng = next_house_rng(house_rng);
                            match house_rng % 12 {
                                0 | 1 => (4, 33),
                                2 | 3 => (14, 113),
                                4..=8 => (13, 105),
                                9..=11 => (12, 97),
                                _ => unreachable!("modulo twelve"),
                            }
                        };
                        residents.push(HouseRemovalResident {
                            position: Position {
                                x: u16::from(x) << 8,
                                y: u16::from(y) << 8,
                                z: resident_z,
                            },
                            model,
                            action,
                        });
                    }
                }
                let index = terrain_index(x, y);
                staged.planes.angles[index] = (staged.planes.angles[index] & 0x70) | 1;
                add_building_to_terrain(
                    &mut staged.planes,
                    x,
                    y,
                    staged.map_type,
                    &mut staged.terrain_rng,
                );
                if cell[1] == 0xff {
                    continue;
                }
                let height = staged.planes.ground_heights[index];
                staged.planes.ground_heights[index] =
                    reduce_house_height(height, cell[1], &mut house_rng);
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                smooth_building_height(
                    &mut staged.planes,
                    start_x.wrapping_add(column),
                    start_y.wrapping_add(row),
                    staged.map_type,
                );
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(HouseRemovalOutcome {
            final_rng: house_rng,
            final_occupants: occupants,
            residents,
        })
    }

    fn apply_castle_update_inner(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if let CastleTerrainUpdate::AmbientMaterial {
            position,
            in_type,
            target_type,
        } = update
        {
            let x = position.x.wrapping_add(128).to_be_bytes()[0];
            let y = position.y.wrapping_add(128).to_be_bytes()[0];
            apply_ambient_material(&mut self.planes, x, y, in_type, target_type, self.map_type)?;
            return Ok(());
        }
        if let CastleTerrainUpdate::AmbientCollision {
            position,
            height_delta,
        } = update
        {
            apply_ambient_collision(
                &mut self.planes,
                position,
                height_delta,
                self.map_type,
                &mut self.terrain_rng,
            );
            return Ok(());
        }
        let position = match update {
            CastleTerrainUpdate::ConstructionPass { position, .. }
            | CastleTerrainUpdate::ConstructionCompleted { position }
            | CastleTerrainUpdate::StageDestroyed { position, .. } => position,
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        };
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);

        match update {
            CastleTerrainUpdate::ConstructionPass { remaining, .. } => {
                if !(1..=18).contains(&remaining) {
                    return Err(format!(
                        "Castle construction remaining count {remaining} is outside 1..=18"
                    ));
                }
                let construction_base = i16::from(position.z >> 5);
                for row in 0..footprint.height {
                    for column in 0..footprint.width {
                        let cell = footprint.cells()
                            [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                        let x = start_x.wrapping_add(column);
                        let y = start_y.wrapping_add(row);
                        let index = terrain_index(x, y);
                        if (remaining % 7 == 0 || remaining == 1) && cell[0] != 0xff {
                            apply_castle_material(&mut self.planes, x, y, cell[0], self.map_type);
                        }
                        if cell[1] == 0xff {
                            continue;
                        }
                        let current = i16::from(self.planes.ground_heights[index]);
                        let target = construction_base + i16::from(cell[1]);
                        let delta = target - current;
                        if delta == 0 {
                            continue;
                        }
                        if current == 0
                            || building_topology_rebuild_required(self.planes.terrain_types[index])
                        {
                            self.planes.angles[index] = (self.planes.angles[index] & 0xf8) | 1;
                            add_building_to_terrain(
                                &mut self.planes,
                                x,
                                y,
                                self.map_type,
                                &mut self.terrain_rng,
                            );
                        }
                        let step = delta / remaining;
                        self.planes.ground_heights[index] = current.wrapping_add(step) as u8;
                        if remaining == 1 && self.planes.angles[index].cast_signed() < 0 {
                            self.planes.angles[index] &= 0x7f;
                            if self.map_type != MapType::Cave {
                                self.planes.angles[index] |= 0x08;
                            }
                        }
                        if self.map_type == MapType::Cave {
                            let desired =
                                target.max(i16::from(self.planes.ground_heights[index])) + 100;
                            let desired = desired.min(255);
                            let secondary = i16::from(self.planes.secondary_heights[index]);
                            if desired > secondary {
                                self.planes.secondary_heights[index] =
                                    secondary.wrapping_add((desired - secondary) / remaining) as u8;
                            }
                            normalize_cave_clearance(&mut self.planes, index);
                        } else if remaining == 2 {
                            self.planes.angles[index] &= 0xf7;
                        }
                    }
                }
            }
            CastleTerrainUpdate::ConstructionCompleted { .. } => {
                if self.map_type != MapType::Cave {
                    for row in 0..footprint.height {
                        for column in 0..footprint.width {
                            let index = terrain_index(
                                start_x.wrapping_add(column),
                                start_y.wrapping_add(row),
                            );
                            if self.planes.angles[index] & 0x08 != 0 {
                                self.planes.angles[index] |= 0x80;
                                self.planes.angles[index] &= 0xf7;
                            }
                        }
                    }
                }
            }
            CastleTerrainUpdate::StageDestroyed { entity_rng, .. } => {
                remove_castle_stage(
                    &mut self.planes,
                    start_x,
                    start_y,
                    footprint,
                    self.map_type,
                    &mut self.terrain_rng,
                    entity_rng,
                );
            }
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        }
        Ok(())
    }
}

pub(crate) fn reduce_house_height(height: u8, reduction: u8, house_rng: &mut u16) -> u8 {
    if reduction >= height {
        return 0;
    }
    *house_rng = next_house_rng(*house_rng);
    if *house_rng % 0x32 <= 20 {
        height.wrapping_sub(reduction)
    } else {
        *house_rng = next_house_rng(*house_rng);
        height
            .wrapping_sub(reduction)
            .wrapping_add((*house_rng % 0x14) as u8)
    }
}

impl FlightWorld for SessionTerrain {
    fn ground_height(&self, position: Position) -> i16 {
        interpolate_ground(&self.planes.ground_heights, position.x, position.y) as i16
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        (self.map_type == MapType::Cave).then(|| {
            interpolate_cave(&self.planes.secondary_heights, position.x, position.y) as i16
        })
    }

    fn material_mask(&self, position: Position) -> u32 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        material_mask(self.planes.terrain_types[terrain_index(x, y)])
    }

    fn terrain_type(&self, position: Position) -> u8 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        self.planes.terrain_types[terrain_index(x, y)]
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.planes.angles[terrain_index(tile.x, tile.y)]
    }
}

const TERRAIN_GENERATION_TABLE_PREFIX: [u8; 0x70] = [
    0x1b, 0x00, 0x1b, 0x50, 0x1b, 0x30, 0x1b, 0x60, 0x1a, 0x00, 0x1a, 0x50, 0x1a, 0x30, 0x1a, 0x60,
    0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60, 0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60,
    0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60, 0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60,
    0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60, 0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60,
    0x15, 0x00, 0x15, 0x50, 0x15, 0x30, 0x15, 0x60, 0x16, 0x00, 0x16, 0x50, 0x16, 0x30, 0x16, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x18, 0x50, 0x18, 0x30, 0x18, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x17, 0x50, 0x17, 0x30, 0x17, 0x60,
];

fn terrain_index(x: u8, y: u8) -> usize {
    usize::from(x) + usize::from(y) * 256
}

fn building_topology_rebuild_required(terrain_type: u8) -> bool {
    matches!(
        terrain_type,
        0 | 0x25 | 0x26 | 0x2c..=0x2f | 0x51 | 0x53 | 0x68 | 0x69 | 0x6d | 0x72 | 0x74
    )
}

fn add_building_to_terrain(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    // `AddBuildingToTerrain_46570` unconditionally marks the selected cell
    // and its west, north-west, and north neighbors before rebuilding their
    // topology. Unlike `sub_462A0`, bit 0x80 never suppresses these writes.
    for (mark_x, mark_y) in [
        (x, y),
        (x.wrapping_sub(1), y),
        (x.wrapping_sub(1), y.wrapping_sub(1)),
        (x, y.wrapping_sub(1)),
    ] {
        planes.terrain_types[terrain_index(mark_x, mark_y)] = 1;
    }
    rebuild_marked_building_topology(planes, x, y, 1, 1, map_type, terrain_rng);
}

fn rebuild_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    end_x: u8,
    end_y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let width = end_x.wrapping_sub(start_x).wrapping_add(1);
    let height = end_y.wrapping_sub(start_y).wrapping_add(1);

    // `sub_462A0` marks every selected cell plus its west, north-west, and
    // north neighbors only when the signed angle byte is non-negative.
    // Material BUILD cells retain bit 0x80 and must not be rewritten here.
    for row in 0..height {
        for column in 0..width {
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            for (mark_x, mark_y) in [
                (x, y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
            ] {
                let index = terrain_index(mark_x, mark_y);
                if planes.angles[index].cast_signed() >= 0 {
                    planes.terrain_types[index] = 1;
                }
            }
        }
    }

    rebuild_marked_building_topology(
        planes,
        start_x,
        start_y,
        width,
        height,
        map_type,
        terrain_rng,
    );
}

fn rebuild_marked_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    width: u8,
    height: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let topology_width = width.wrapping_add(1);
    let topology_height = height.wrapping_add(1);
    let topology_start_x = start_x.wrapping_sub(1);
    let topology_start_y = start_y.wrapping_sub(1);

    for row in 0..topology_height {
        for column in 0..topology_width {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let index = terrain_index(tile_x, tile_y);
            if planes.terrain_types[index] != 1 {
                continue;
            }
            let topology_index = 343 * usize::from(planes.angles[index] & 7)
                + 49 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y)] & 7,
                )
                + 7 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))]
                        & 7,
                )
                + usize::from(planes.angles[terrain_index(tile_x, tile_y.wrapping_add(1))] & 7);
            let [terrain_type, rotation] = building_topology_entry(topology_index);
            planes.terrain_types[index] = terrain_type;
            if terrain_type >= 8 {
                // The decompiled source writes `(angle & 0x87) + rotation`.
                planes.angles[index] = (planes.angles[index] & 0x87).wrapping_add(rotation);
            } else {
                *terrain_rng = terrain_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.angles[index] =
                    (planes.angles[index] & 0x87).wrapping_add(16 * (*terrain_rng % 7) as u8);
            }
        }
    }

    // Finally recalculate the one-cell-expanded shading neighborhood in the
    // literal source order. The signed eight-bit temporary deliberately wraps.
    for row in 0..topology_height.wrapping_add(1) {
        for column in 0..topology_width.wrapping_add(1) {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let down_right = planes.ground_heights
                [terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))];
            let up_left = planes.ground_heights
                [terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))];
            let mut shade = (32_i16 - i16::from(down_right)) as i8;
            shade = shade.wrapping_add(up_left as i8);
            shade = if shade >= 28 {
                if shade > 40 {
                    (shade & 7).wrapping_add(40)
                } else {
                    shade
                }
            } else {
                (shade & 3).wrapping_add(28)
            };
            let index = terrain_index(tile_x, tile_y);
            planes.shading[index] = if map_type == MapType::Day {
                shade as u8
            } else {
                64_i8.wrapping_sub(shade) as u8
            };
            if map_type == MapType::Cave {
                normalize_cave_clearance(planes, index);
            } else {
                planes.angles[index] &= 0xf7;
            }
        }
    }
}

pub(crate) fn apply_level1_load_path_point(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
    rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
}

pub(crate) fn level1_load_house_base(
    planes: &TerrainPlanes,
    tile_x: u8,
    tile_y: u8,
    footprint: &CastleBuildFootprint,
) -> (u8, u8, i16) {
    let mut center_x = tile_x;
    let center_y = tile_y;
    let mut start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    if start_x.wrapping_add(start_y) & 1 != 0 {
        center_x = center_x.wrapping_add(1);
        start_x = start_x.wrapping_add(1);
    }
    let corner_sum = u16::from(planes.ground_heights[terrain_index(start_x, start_y)])
        + u16::from(
            planes.ground_heights[terrain_index(start_x.wrapping_add(footprint.width), start_y)],
        )
        + u16::from(
            planes.ground_heights[terrain_index(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )],
        )
        + u16::from(
            planes.ground_heights[terrain_index(start_x, start_y.wrapping_add(footprint.height))],
        );
    (
        center_x,
        center_y,
        i16::try_from((corner_sum >> 2) * 32).expect("average u8 corner height fits i16"),
    )
}

pub(crate) fn apply_level1_load_house_sweep(
    planes: &mut TerrainPlanes,
    terrain_rng: &mut u16,
    position: Position,
    footprint: &CastleBuildFootprint,
    remaining: u8,
) {
    let center_x = position.x.to_be_bytes()[0];
    let center_y = position.y.to_be_bytes()[0];
    let start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    let construction_base = position.z >> 5;

    if remaining == 0 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    rebuild_building_topology(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        MapType::Night,
                        terrain_rng,
                    );
                }
            }
        }
        return;
    }

    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            let current = i16::from(planes.ground_heights[index]);
            let target = construction_base + i16::from(cell[1]);
            let next = current.wrapping_add((target - current) / i16::from(remaining));
            planes.ground_heights[index] = next.to_le_bytes()[0];
            if planes.angles[index].trailing_zeros() >= 3 {
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
            }
        }
    }

    if remaining.is_multiple_of(5) || remaining == 1 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] == 0xff {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let index = terrain_index(x, y);
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                planes.terrain_types[index] = 1;
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    apply_level1_house_material(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        column,
                        cell[0],
                        terrain_rng,
                    );
                }
            }
        }
    }
}

pub(crate) fn apply_level1_load_house_edge_smoothing(
    planes: &mut TerrainPlanes,
    position: Position,
    footprint: &CastleBuildFootprint,
) {
    let center_x = position.x.to_be_bytes()[0];
    let center_y = position.y.to_be_bytes()[0];
    let footprint_start_x = center_x.wrapping_sub(footprint.width / 2);
    let footprint_start_y = center_y.wrapping_sub(footprint.height / 2);
    let half_width = footprint.width / 2;
    let half_height = footprint.height / 2;
    for radius in [2_u8, 5] {
        smooth_level1_house_edges(
            planes,
            footprint_start_x,
            footprint_start_y,
            half_width,
            half_height,
            radius,
        );
    }
}

fn smooth_level1_house_edges(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    half_width: u8,
    half_height: u8,
    radius: u8,
) {
    let boundary_x = start_x.wrapping_sub(half_width);
    let boundary_y = start_y.wrapping_sub(half_height);
    for row in 0..half_height.wrapping_mul(2) {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
            smooth_level1_house_height(
                planes,
                boundary_x
                    .wrapping_add(half_width.wrapping_mul(2))
                    .wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
        }
    }
    for column in 0..half_width
        .wrapping_mul(2)
        .wrapping_add(radius.wrapping_mul(2))
    {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y.wrapping_sub(radius).wrapping_add(inset),
            );
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y
                    .wrapping_add(half_height.wrapping_mul(2))
                    .wrapping_add(inset),
            );
        }
    }
}

fn smooth_level1_house_height(planes: &mut TerrainPlanes, tile_x: u8, tile_y: u8) {
    let index = terrain_index(tile_x, tile_y);
    if planes.angles[index].trailing_zeros() >= 3
        || planes.ground_heights[index] == 0
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x, tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y)],
        )
        || !level1_house_smoothing_type(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row in 0..3_u8 {
        for column in 0..3_u8 {
            let neighbor = terrain_index(
                tile_x.wrapping_sub(1).wrapping_add(column),
                tile_y.wrapping_sub(1).wrapping_add(row),
            );
            if level1_house_smoothing_type(planes.terrain_types[neighbor]) {
                height_sum += u32::from(planes.ground_heights[neighbor]);
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] =
            u8::try_from(height_sum / height_count).expect("average of u8 heights fits u8");
    }
}

const fn level1_house_smoothing_type(terrain_type: u8) -> bool {
    terrain_type <= 5 || terrain_type > 0x22
}

fn apply_level1_house_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    column: u8,
    material: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    if material < 8 {
        planes.angles[index] = (planes.angles[index] & 0xf0) | material;
        rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
        return;
    }
    match material {
        8 => planes.terrain_types[index] = 8,
        9 => planes.terrain_types[index] = 9,
        16..=19 => {
            let corner = classify_castle_corner(planes, x, y, column);
            let table_offset = match material {
                16 => 2 * corner,
                17 => 0x40 + 2 * corner,
                18 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                19 => 0x60 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                _ => unreachable!(),
            };
            let preserve = material == 16 && matches!(planes.terrain_types[index], 10..=12);
            if !preserve {
                let table = terrain_generation_table();
                planes.terrain_types[index] = table[table_offset];
                planes.angles[index] = (planes.angles[index] & 0x8f) | table[table_offset + 1];
            }
        }
        _ => return,
    }
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
    }
}

pub(crate) fn level1_feature_ground_height(planes: &TerrainPlanes, x: u16, y: u16) -> i16 {
    let bytes = interpolate_ground(&planes.ground_heights, x, y).to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

fn terrain_generation_table() -> &'static [u8; 0x120] {
    static TABLE: OnceLock<[u8; 0x120]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let source =
            include_str!("../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D4A30.cpp");
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-generation initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-generation terminator")
            .0;
        let initializer = initializer
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(values, _)| values))
            .collect::<String>();
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(0x120);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain-generation byte");
                values.push(
                    u8::from_str_radix(hex, 16).expect("terrain-generation hexadecimal byte"),
                );
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 0x120);
        values
            .try_into()
            .expect("terrain-generation table has fixed extent")
    })
}

fn building_topology_entry(index: usize) -> [u8; 2] {
    debug_assert!(index < 7_usize.pow(4));
    let catalog = terrain_corner_catalog();
    for (terrain_type, corners) in catalog.iter().enumerate() {
        if corners.iter().any(|corner| *corner < 0) {
            continue;
        }
        let c = corners.map(|corner| corner as usize);
        for (candidate, rotation) in [
            (343 * c[0] + 49 * c[1] + 7 * c[2] + c[3], 0),
            (343 * c[1] + 49 * c[0] + 7 * c[3] + c[2], 16),
            (343 * c[2] + 49 * c[3] + 7 * c[0] + c[1], 48),
            (343 * c[3] + 49 * c[2] + 7 * c[1] + c[0], 32),
            (343 * c[1] + 49 * c[2] + 7 * c[3] + c[0], 96),
            (343 * c[2] + 49 * c[1] + 7 * c[0] + c[3], 112),
            (343 * c[3] + 49 * c[0] + 7 * c[1] + c[2], 80),
            (343 * c[0] + 49 * c[3] + 7 * c[2] + c[1], 64),
        ] {
            if candidate == index {
                return [terrain_type as u8, rotation];
            }
        }
    }
    [1, 0]
}

fn terrain_corner_catalog() -> &'static [[i8; 4]; 148] {
    static CATALOG: OnceLock<[[i8; 4]; 148]> = OnceLock::new();
    CATALOG.get_or_init(|| {
        // remc2 keeps this functional terrain-topology catalog in a dedicated
        // source table. Parsing its hexadecimal initializer here avoids
        // duplicating a second hand-maintained 592-byte private table.
        let source =
            include_str!("../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D47E0.cpp");
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-corner initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-corner terminator")
            .0;
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(148 * 4);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain corner");
                values.push(u8::from_str_radix(hex, 16).expect("terrain corner byte") as i8);
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 148 * 4);
        let mut result = [[0_i8; 4]; 148];
        for (row, values) in result.iter_mut().zip(values.chunks_exact(4)) {
            row.copy_from_slice(values);
        }
        result
    })
}

fn classify_castle_corner(planes: &TerrainPlanes, x: u8, y: u8, fallback: u8) -> usize {
    let heights = [
        planes.ground_heights[terrain_index(x, y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
        planes.ground_heights[terrain_index(x, y.wrapping_add(1))],
    ];
    let mut maximum = 0_u8;
    let mut maximum_corner = 0_usize;
    let mut minimum = u8::MAX;
    for (corner, height) in heights.into_iter().enumerate() {
        if height > maximum {
            maximum = height;
            maximum_corner = corner;
        }
        minimum = minimum.min(height);
    }
    let mut second_maximum = 0_u8;
    let mut second_corner = usize::from(fallback);
    for (corner, height) in heights.into_iter().enumerate() {
        if corner != maximum_corner && height > second_maximum {
            second_maximum = height;
            second_corner = corner;
        }
    }
    if maximum.wrapping_sub(minimum) >= 8 && maximum.wrapping_sub(second_maximum) >= 8 {
        return maximum_corner;
    }
    match maximum_corner {
        0 => {
            if second_corner == 1 {
                4
            } else {
                7
            }
        }
        1 => {
            if second_corner == 2 {
                5
            } else {
                4
            }
        }
        2 => {
            if second_corner == 3 {
                6
            } else {
                5
            }
        }
        3 => {
            if second_corner != 0 {
                6
            } else {
                7
            }
        }
        _ => 0,
    }
}

fn apply_castle_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    material: u8,
    map_type: MapType,
) {
    let index = terrain_index(x, y);
    let preserve_existing_material =
        material == 0x10 && matches!(planes.terrain_types[index], 10 | 11 | 12);
    if !preserve_existing_material {
        let corner = classify_castle_corner(planes, x, y, 7);
        let table_offset = match material {
            0x10 => 2 * corner,
            0x12 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
            _ => {
                // BUILD[1] is qualified to contain only 0x10 and 0x12 controls.
                return;
            }
        };
        planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
        planes.angles[index] =
            (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    }
    planes.angles[index] |= 0x80;
    if map_type == MapType::Cave {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            normalize_cave_clearance(planes, terrain_index(corner_x, corner_y));
        }
    } else {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
        }
    }
}

fn apply_ambient_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    in_type: u8,
    target_type: u8,
    map_type: MapType,
) -> std::result::Result<(), String> {
    let base_offset = match target_type {
        20 => 0x10,
        21 => 0x20,
        22 => 0x30,
        _ => {
            return Err(format!(
                "ambient terrain target {target_type} is outside 20..=22"
            ));
        }
    };
    let corner = classify_castle_corner(planes, x, y, in_type);
    let table_offset = base_offset + 2 * corner;
    let index = terrain_index(x, y);
    planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
    planes.angles[index] =
        (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        let index = terrain_index(corner_x, corner_y);
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        } else {
            planes.angles[index] &= 0xf7;
        }
    }
    Ok(())
}

fn apply_ambient_collision(
    planes: &mut TerrainPlanes,
    position: Position,
    height_delta: i8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let base_x = (position.x as i16).wrapping_add(128) >> 8;
    let base_y = (position.y as i16).wrapping_add(128) >> 8;
    for (offset_x, offset_y) in [(0_i16, 0_i16), (1, 0), (0, 1), (1, 1)] {
        let signed_x = base_x.wrapping_add(offset_x);
        let signed_y = base_y.wrapping_add(offset_y);
        let x = signed_x as u8;
        let y = signed_y as u8;
        let index = terrain_index(x, y);
        if planes.angles[index] & 0x80 != 0 {
            break;
        }
        let unclamped = i16::from(planes.ground_heights[index]) + i16::from(height_delta);
        let height = unclamped.clamp(0, 200) as u8;
        planes.ground_heights[index] = height;
        if map_type == MapType::Cave {
            let secondary = i16::from(planes.secondary_heights[index]) - i16::from(height_delta);
            planes.secondary_heights[index] = if secondary >= 255 {
                u8::MAX
            } else {
                secondary as u8
            };
        }
        if height != 0 {
            planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
        } else {
            let allowed = [
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
                (x.wrapping_add(1), y.wrapping_sub(1)),
                (x.wrapping_add(1), y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_add(1)),
                (x, y.wrapping_add(1)),
                (x.wrapping_add(1), y.wrapping_add(1)),
            ]
            .into_iter()
            .all(|(x, y)| {
                let angle = planes.angles[terrain_index(x, y)] & 7;
                !matches!(angle, 2 | 3 | 5)
            });
            if allowed {
                planes.angles[index] &= 0xf0;
            }
        }
        add_building_to_terrain(planes, x, y, map_type, terrain_rng);
        if !(0..=200).contains(&unclamped) && signed_x == 0 && signed_y == 0 {
            break;
        }
    }
}

fn normalize_cave_clearance(planes: &mut TerrainPlanes, index: usize) {
    if planes.secondary_heights[index] > planes.ground_heights[index] {
        planes.angles[index] &= 0xf7;
    } else {
        planes.secondary_heights[index] = planes.ground_heights[index].wrapping_sub(1);
        planes.angles[index] |= 0x08;
    }
}

fn remove_castle_stage(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    footprint: &CastleBuildFootprint,
    map_type: MapType,
    terrain_rng: &mut u16,
    mut entity_rng: u16,
) {
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[0] == 0xff && cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            planes.angles[index] = (planes.angles[index] & 0x70) | 1;
            add_building_to_terrain(planes, x, y, map_type, terrain_rng);
            if cell[1] == 0xff {
                continue;
            }
            let offset = cell[1];
            let height = planes.ground_heights[index];
            if offset >= height {
                planes.ground_heights[index] = 0;
                continue;
            }
            entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
            if entity_rng % 50 <= 20 {
                planes.ground_heights[index] = height.wrapping_sub(offset);
            } else {
                entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.ground_heights[index] = height
                    .wrapping_sub(offset)
                    .wrapping_add((entity_rng % 20) as u8);
            }
        }
    }
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            smooth_building_height(
                planes,
                start_x.wrapping_add(column),
                start_y.wrapping_add(row),
                map_type,
            );
        }
    }
}

const fn next_house_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn smooth_building_height(planes: &mut TerrainPlanes, x: u8, y: u8, map_type: MapType) {
    let index = terrain_index(x, y);
    let ordinary = |terrain_type: u8| terrain_type <= 5 || terrain_type > 0x22;
    if planes.angles[index] & 7 == 0
        || planes.ground_heights[index] == 0
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x, y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y)])
        || !ordinary(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row_offset in [u8::MAX, 0, 1] {
        for column_offset in [u8::MAX, 0, 1] {
            let neighbor = terrain_index(x.wrapping_add(column_offset), y.wrapping_add(row_offset));
            if ordinary(planes.terrain_types[neighbor]) {
                height_sum = height_sum.wrapping_add(u32::from(planes.ground_heights[neighbor]));
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] = (height_sum / height_count) as u8;
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        }
    }
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
    goat_tmap_bank: Option<GoatTmapBank>,
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
    reset_state: FlightState,
    reset_context: FlightContext,
    goat_stage_point: Position,
    goat_config: GoatBehaviorConfig,
    reset_live_world: LiveWorldState,
    level1_mission: Level1MissionTemplate,
}

impl NativeWorld {
    /// Load and validate an export package and its separately supplied closure.
    ///
    /// # Errors
    ///
    /// Returns an error on any schema, identity, size, version, relation,
    /// reset, closure-binding, or frame-resolution mismatch.
    pub fn load(package_dir: impl AsRef<Path>, closure_path: impl AsRef<Path>) -> Result<Self> {
        let owner_cache = std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from);
        Self::load_internal(
            package_dir.as_ref(),
            closure_path.as_ref(),
            owner_cache.as_deref(),
        )
    }

    /// Load with an explicit qualified private owner-asset cache.
    ///
    /// This is the deterministic equivalent of setting `MC_ASSET_CACHE_PATH`
    /// and avoids process-global environment mutation in tools and tests.
    ///
    /// # Errors
    ///
    /// Returns an error on any package, closure, cache identity, binding,
    /// output hash, or qualified descriptor mismatch.
    pub fn load_with_owner_cache(
        package_dir: impl AsRef<Path>,
        closure_path: impl AsRef<Path>,
        owner_cache: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::load_internal(
            package_dir.as_ref(),
            closure_path.as_ref(),
            Some(owner_cache.as_ref()),
        )
    }

    fn load_internal(
        package_dir: &Path,
        closure_path: &Path,
        owner_cache: Option<&Path>,
    ) -> Result<Self> {
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
        validate_level_one_mission_stages(&runtime)?;
        let presentation = parse_presentation(member(&members, "presentation.bin")?)?;
        validate_level_one_mission_presentation(&presentation)?;
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
        VillagerBehaviorConfig::from_parameter_record(
            presentation.entity_parameters.get(100).ok_or_else(|| {
                NativeWorldError::new("Level 1 Villager parameter row 100 is absent")
            })?,
        )
        .map_err(NativeWorldError::new)?;
        let goat_render = presentation
            .particles
            .get(usize::from(GOAT_RENDER_PARAMETER_INDEX))
            .ok_or_else(|| NativeWorldError::new("Level 1 Goat render parameter 238 is absent"))?;
        let exact_goat_render = ParticleParameter {
            base_tmap: GOAT_BASE_TMAP,
            word_2: 8_492,
            word_4: 42,
            speed: 150,
            rotation_speed: 150,
            byte_10: 0,
            byte_11: 1,
            render_mode: 20,
            byte_13: 0,
        };
        if *goat_render != exact_goat_render {
            return Err(NativeWorldError::new(format!(
                "Level 1 Goat render parameter 238 mismatch: got {goat_render:?}, expected {exact_goat_render:?}"
            )));
        }
        let reset_live_world = LiveWorldState::from_entities_with_map(
            &entities,
            &terrain_planes.entity_indices,
            goat_stage_point,
            goat_config,
        )
        .map_err(NativeWorldError::new)?;
        let global_mana_denominator = level_one_global_mana_denominator(&entities)?;
        let initial_source_slots = entities
            .iter()
            .map(|entity| Level1ResetSourceSlot {
                index: entity.index,
                active: entity.active,
                class: entity.class,
                model: entity.model,
                action: entity.action,
                phase: entity.phase,
                entity_rng: entity.entity_rng,
                control_flags: entity.control_flags,
            })
            .collect::<Vec<_>>();
        let mut initial_ambient_entities = entities
            .iter()
            .filter_map(EntitySnapshot::ambient_entity)
            .collect::<Vec<_>>();
        initial_ambient_entities.sort_unstable_by_key(|entity| entity.source_entity_index);
        let emitter_slots = initial_ambient_entities
            .iter()
            .filter(|entity| (entity.model, entity.action) == (60, 65))
            .map(|entity| entity.source_entity_index)
            .collect::<Vec<_>>();
        let mut smoke_ordinal = 0_usize;
        for entity in &mut initial_ambient_entities {
            match (entity.model, entity.action) {
                (14, 14) => {
                    entity.parent_entity_index = emitter_slots[smoke_ordinal];
                    smoke_ordinal += 1;
                }
                (0, 0) => entity.parent_entity_index = entity.source_id,
                _ => {}
            }
        }
        let initial_castle_blockers = entities
            .iter()
            .filter(|entity| {
                entity.active && entity.life >= 0 && entity.class == 3 && entity.model == 2
            })
            .map(EntitySnapshot::castle_placement_blocker)
            .collect::<Vec<_>>();
        let initial_building_blockers = entities
            .iter()
            .filter(|entity| entity.active && entity.class == 10 && entity.model == 45)
            .map(EntitySnapshot::castle_placement_blocker)
            .collect::<Vec<_>>();
        let initial_possession_targets = entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.life >= 0
                    && entity.class == 10
                    && entity.model == 45
                    // remc2 `sub_108B0` rejects model-45 buildings whose
                    // BLDGPRM byte_2 has bit 8 set. In the qualified Level 1
                    // package those are rows 17 and 51..=53.
                    && !matches!(entity.level_gate, 17 | 51 | 52 | 53)
            })
            .map(EntitySnapshot::collision_target)
            .collect::<Vec<_>>();
        let initial_source_collision_targets = entities
            .iter()
            .filter(|entity| entity.active && entity.control_flags & 8 != 0)
            .map(|entity| Level1SourceCollisionTarget {
                target: entity.collision_target(),
                source_id: entity.source_id,
            })
            .collect::<Vec<_>>();
        let (closure_sha256, mut asset_refs) =
            parse_closure(closure_path, &package_sha256, &manifest_bytes, &members)?;
        let cache_assets = match owner_cache {
            Some(cache_path) => append_level_one_cache_assets(
                cache_path,
                &package_sha256,
                &closure_sha256,
                &mut asset_refs,
            )?,
            None => LevelOneCacheAssets::default(),
        };
        let level1_mission =
            Level1MissionTemplate::decode_with_source_slots_and_placement_blockers(
                authored,
                &terrain,
                global_mana_denominator,
                runtime.random_state,
                &initial_source_slots,
                &initial_ambient_entities,
                &initial_castle_blockers,
                &initial_building_blockers,
                &initial_possession_targets,
                &initial_source_collision_targets,
                cache_assets.castle_stage_one_footprint,
            )
            .map_err(NativeWorldError::new)?;

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
            sound_asset_refs: cache_assets.sound_asset_refs,
            music_asset_ref: cache_assets.music_asset_ref,
            goat_tmap_bank: cache_assets.goat_tmap_bank,
            house_build_footprints: cache_assets.house_build_footprints,
            reset_state,
            reset_context,
            goat_stage_point,
            goat_config,
            reset_live_world,
            level1_mission,
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
    pub fn house_build_footprint(&self, building_index: u8) -> Option<&CastleBuildFootprint> {
        self.house_build_footprints.get(&building_index)
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
    pub const fn goat_tmap_bank(&self) -> Option<&GoatTmapBank> {
        self.goat_tmap_bank.as_ref()
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

    #[must_use]
    pub const fn level1_mission(&self) -> &Level1MissionTemplate {
        &self.level1_mission
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
        // The preserved reset conformance projection intentionally contains
        // only the flight-critical left holder. A live Level 1 session needs
        // the complete owned hand state: Fireball in the left hand and
        // Possession in the right, plus the enabled-family lookup used by
        // selected casting.
        let player = &self.players[usize::from(self.runtime.local_player_slot)];
        if let Some(fire) = context.spell_fire.as_mut() {
            let left_slot = usize::from(player.left_spell_slot);
            let right_slot = usize::from(player.right_spell_slot);
            let left_index = player.enabled_spell_holders[left_slot];
            let right_index = player.enabled_spell_holders[right_slot];
            fire.left_holder_index = left_index;
            fire.right_holder_index = right_index;
            fire.left_subspell_index = player.left_subspell_index;
            fire.right_subspell_index = player.right_subspell_index;
            fire.selected_spell_ui_index = player.selected_spell_ui_index;
            fire.selected_subspell_index = player.selected_subspell_index;
            fire.spell_hand_eligibility = player.spell_hand_eligibility;
            fire.family_selected_tiers = player.family_selected_tiers;
            let selected_family =
                SOURCE_SPELL_INDEX_MAP[usize::from(player.selected_spell_ui_index)];
            fire.selected_holder_index = player.enabled_spell_holders[usize::from(selected_family)];
            fire.enabled_holder_indices[left_slot] = left_index;
            fire.enabled_holder_indices[right_slot] = right_index;
            if right_index != 0
                && right_index != left_index
                && let Some(holder) = self.entities.get(usize::from(right_index))
            {
                fire.holders.push(SpellActivationHolder {
                    entity_index: holder.index,
                    model: holder.model,
                    level_gate: holder.level_gate,
                    active_value: holder.active_value,
                    activation_value: holder.activation_value,
                    mana_cost: holder.mana_cost,
                    action_marker: holder.action_marker,
                });
            }
        }
        context
    }

    #[must_use]
    pub fn session(&self) -> NativeSession<'_> {
        let mut mission = self.level1_mission.session(
            self.entities[usize::from(self.reset_state.entity_index)].life,
            1_000,
        );
        mission.set_source_turn_counter(
            self.players[usize::from(self.runtime.local_player_slot)]
                .turn
                .cast_unsigned(),
        );
        NativeSession {
            world: self,
            terrain: SessionTerrain::from_world(self),
            state: self.reset_state,
            context: self.session_context(),
            live_world: self.reset_live_world.clone(),
            goat_tmap_clocks: self.goat_tmap_bank.as_ref().map(GoatTmapClocks::from_bank),
            camera_fov: 128,
            pending_house_effects: Vec::new(),
            mission,
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

    fn terrain_type(&self, position: Position) -> u8 {
        self.terrain.terrain_type(position)
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.terrain.angle_flags(tile)
    }
}

/// Per-play-session mutable state, independent from the immutable world.
#[derive(Debug)]
pub struct NativeSession<'world> {
    world: &'world NativeWorld,
    pub terrain: SessionTerrain,
    pub state: FlightState,
    pub context: FlightContext,
    pub live_world: LiveWorldState,
    pub goat_tmap_clocks: Option<GoatTmapClocks>,
    pub camera_fov: i16,
    pub pending_house_effects: Vec<HouseStepEvent>,
    pub mission: Level1MissionState,
}

fn goat_source_entity_view(goat: &GoatState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: goat.entity_index,
        source_id: goat.source_id,
        authority: Level1SourceEntityAuthority::LiveGoat,
        active: goat.active,
        visible: goat.active,
        class: GOAT_CLASS,
        model: GOAT_MODEL,
        action: goat.action,
        phase: goat.phase,
        entity_rng: goat.rng,
        control_flags: goat.control_flags,
        position: goat.position,
        max_life: goat.max_life,
        life: goat.life,
        pending_damage: goat.pending_damage,
        pending_attacker: goat.pending_attacker.cast_unsigned(),
        owner_entity_index: goat.current_attacker.cast_unsigned(),
        target_entity_index: goat.target_entity_index.unwrap_or(0),
        linked_entity_index: 0,
        actual_speed: goat.actual_speed,
        maximum_speed: goat.maximum_speed,
        collision_half_width: goat.collision_half_width,
        collision_half_depth: goat.collision_half_width,
        collision_vertical_offset: goat.collision_vertical_offset,
        collision_half_height: goat.collision_half_height,
        villager_entry_eligible: false,
        builder_placement_latch: 0,
    }
}

fn house_source_entity_view(house: &HouseState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: house.entity_index,
        source_id: house.source_id,
        authority: Level1SourceEntityAuthority::LiveHouse,
        active: house.active,
        visible: house.active,
        class: HOUSE_CLASS,
        model: HOUSE_MODEL,
        action: house.action,
        phase: house.phase,
        entity_rng: house.rng,
        control_flags: house.control_flags,
        position: house.position,
        max_life: house.max_life,
        life: house.life,
        pending_damage: house.pending_damage,
        pending_attacker: house.pending_attacker.cast_unsigned(),
        owner_entity_index: house.owner_entity_index,
        target_entity_index: 0,
        linked_entity_index: 0,
        actual_speed: 0,
        maximum_speed: 0,
        collision_half_width: house.collision_half_width,
        collision_half_depth: house.collision_half_depth,
        collision_vertical_offset: house.collision_vertical_offset,
        collision_half_height: house.collision_half_height,
        villager_entry_eligible: house.parameter.produces_villagers(),
        builder_placement_latch: 0,
    }
}

fn villager_source_entity_view(villager: &VillagerState) -> Level1SourceEntityView {
    Level1SourceEntityView {
        source_entity_index: villager.entity_index,
        source_id: villager.source_id,
        authority: Level1SourceEntityAuthority::LiveVillager,
        active: villager.active,
        visible: villager.active,
        class: VILLAGER_CLASS,
        model: VILLAGER_MODEL,
        action: villager.action,
        phase: villager.phase,
        entity_rng: villager.rng,
        control_flags: villager.control_flags,
        position: villager.position,
        max_life: villager.max_life,
        life: villager.life,
        pending_damage: villager.pending_damage,
        pending_attacker: villager.pending_attacker.cast_unsigned(),
        owner_entity_index: villager.current_attacker.cast_unsigned(),
        target_entity_index: villager.target_entity_index.unwrap_or(0),
        linked_entity_index: 0,
        actual_speed: villager.actual_speed,
        maximum_speed: villager.maximum_speed,
        collision_half_width: villager.collision_half_width,
        collision_half_depth: villager.collision_half_width,
        collision_vertical_offset: villager.collision_vertical_offset,
        collision_half_height: villager.collision_half_height,
        villager_entry_eligible: false,
        builder_placement_latch: 0,
    }
}

fn current_live_world_source_entity_view(
    live_world: &LiveWorldState,
    source_entity_index: u16,
) -> Option<Level1SourceEntityView> {
    live_world
        .goats()
        .iter()
        .find(|goat| goat.entity_index == source_entity_index)
        .map(goat_source_entity_view)
        .or_else(|| {
            live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == source_entity_index)
                .map(house_source_entity_view)
        })
        .or_else(|| {
            live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_entity_index)
                .map(villager_source_entity_view)
        })
}

/// Update only the current host-owned row after that source action. The
/// turn-start vector remains allocated and ordered once; this does not admit
/// the row into any frozen remc2 seeker list.
pub fn refresh_level1_current_source_view(
    source_views: &mut Vec<Level1SourceEntityView>,
    live_world: &LiveWorldState,
    source_entity_index: u16,
) {
    let Some(current) = current_live_world_source_entity_view(live_world, source_entity_index)
    else {
        return;
    };
    match source_views.binary_search_by_key(&source_entity_index, |view| view.source_entity_index) {
        Ok(index) => source_views[index] = current,
        Err(index) => source_views.insert(index, current),
    }
}

/// Build the production turn-start source snapshot from the existing typed
/// authorities. The returned values live only on the caller's stack/heap for
/// the duration of this source turn.
#[must_use]
pub fn level1_turn_start_source_views(
    mission: &Level1MissionState,
    player: &FlightState,
    live_world: &LiveWorldState,
) -> Vec<Level1SourceEntityView> {
    let mut imported = Vec::with_capacity(
        live_world.goats().len() + live_world.houses().len() + live_world.villagers().len(),
    );
    imported.extend(live_world.goats().iter().map(goat_source_entity_view));
    imported.extend(live_world.houses().iter().map(house_source_entity_view));
    imported.extend(
        live_world
            .villagers()
            .iter()
            .map(villager_source_entity_view),
    );
    mission.source_entity_views(player, &imported)
}

/// Apply one resident dispatch's effects after its mission borrow has ended.
///
/// House entry stages both authorities and commits them together. Rejected
/// full/dead entries leave both hashes unchanged. Returned seeds let the C API
/// update its presentation chain from the same applied effect; the direct
/// session already receives the mission-owned chain update.
pub fn apply_level1_source_effects(
    mission: &mut Level1MissionState,
    live_world: &mut LiveWorldState,
    effects: &[Level1SourceEffect],
) -> std::result::Result<Vec<RenderableMapSeed>, String> {
    if effects.is_empty() {
        return Ok(Vec::new());
    }

    let mut staged_mission = mission.clone();
    let mut staged_world = live_world.clone();
    let mut render_updates = Vec::new();
    let applied = (|| {
        for effect in effects.iter().copied() {
            match effect {
                Level1SourceEffect::CreateBuilderHouse { house } => {
                    staged_world.insert_builder_house(house)?;
                    staged_mission.update_external_collision_target(house_collision_target(&house));
                    if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                        render_updates.push(seed);
                    }
                }
                Level1SourceEffect::InitializeBuilderHouse { house } => {
                    staged_world.initialize_builder_house(house)?;
                    staged_mission.update_external_collision_target(house_collision_target(&house));
                    staged_mission.apply_local_source_effect(effect)?;
                }
                Level1SourceEffect::SetBuilderHouseAction {
                    source_entity_index,
                    action,
                } => {
                    staged_world.set_builder_house_action(source_entity_index, action)?;
                    staged_mission.apply_local_source_effect(effect)?;
                }
                Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index,
                    house_source_entity_index,
                } => {
                    if staged_mission.owns_authored_house(house_source_entity_index) {
                        if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                            render_updates.push(seed);
                        }
                        continue;
                    }
                    if staged_world.try_enter_resident(house_source_entity_index)
                        == HouseResidentEntry::Entered
                    {
                        if let Some(seed) = staged_mission.apply_local_source_effect(
                            Level1SourceEffect::DisableSourceSlot {
                                source_entity_index: resident_source_entity_index,
                            },
                        )? {
                            render_updates.push(seed);
                        }
                    }
                }
                Level1SourceEffect::TryEnterVillager {
                    resident_source_entity_index,
                    house_source_entity_index,
                } => {
                    if staged_mission.owns_authored_house(house_source_entity_index) {
                        staged_mission.apply_local_source_effect(effect)?;
                        continue;
                    }
                    if staged_world.try_enter_resident(house_source_entity_index)
                        != HouseResidentEntry::Entered
                    {
                        staged_mission
                            .reject_dynamic_villager_entry(resident_source_entity_index)?;
                    }
                }
                _ => {
                    if let Some(seed) = staged_mission.apply_local_source_effect(effect)? {
                        render_updates.push(seed);
                    }
                }
            }
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = applied {
        if let Some(source_entity_index) = effects.iter().find_map(|effect| match effect {
            Level1SourceEffect::CreateBuilderHouse { house } => Some(house.entity_index),
            _ => None,
        }) {
            let _ = mission.rollback_builder_house_allocation(source_entity_index);
        }
        return Err(error);
    }
    *mission = staged_mission;
    *live_world = staged_world;
    Ok(render_updates)
}

fn goat_collision_target(goat: &GoatState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: goat.entity_index,
        position: goat.position,
        collision_half_width: goat.collision_half_width,
        collision_vertical_offset: goat.collision_vertical_offset,
        collision_half_height: goat.collision_half_height,
        active: goat.active,
    }
}

fn villager_collision_target(villager: &VillagerState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: villager.entity_index,
        position: villager.position,
        collision_half_width: villager.collision_half_width,
        collision_vertical_offset: villager.collision_vertical_offset,
        collision_half_height: villager.collision_half_height,
        active: villager.active,
    }
}

fn house_collision_target(house: &HouseState) -> Level1CollisionTarget {
    Level1CollisionTarget {
        entity_index: house.entity_index,
        position: house.position,
        collision_half_width: house.collision_half_width,
        collision_vertical_offset: house.collision_vertical_offset,
        collision_half_height: house.collision_half_height,
        active: house.active,
    }
}

impl NativeSession<'_> {
    /// Compose the legacy typed live-world hash with the authoritative
    /// mission-owned global RNG under the explicit v2 domain.
    #[must_use]
    pub fn live_world_hash_v2(&self) -> u64 {
        self.live_world.hash_v2(self.mission.global_rng())
    }

    pub fn reset(&mut self) {
        self.terrain = SessionTerrain::from_world(self.world);
        self.state = self.world.reset_state;
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world.clone();
        self.goat_tmap_clocks = self
            .world
            .goat_tmap_bank
            .as_ref()
            .map(GoatTmapClocks::from_bank);
        self.camera_fov = 128;
        self.pending_house_effects.clear();
        self.mission = self.world.level1_mission.session(
            self.world.entities[usize::from(self.world.reset_state.entity_index)].life,
            1_000,
        );
        self.mission.set_source_turn_counter(
            self.world.players[usize::from(self.world.runtime.local_player_slot)]
                .turn
                .cast_unsigned(),
        );
    }

    pub fn step(&mut self, command: PlayerCommand) -> Vec<crate::StepEvidence> {
        self.mission.finish_hud_draw(self.state.tick);
        if let (Some(clocks), Some(bank)) = (
            self.goat_tmap_clocks.as_mut(),
            self.world.goat_tmap_bank.as_ref(),
        ) {
            clocks.service_unpaused_outer_turn(bank);
        }
        self.mission.advance_exit_palette_frame();
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let holders_before = self
            .context
            .spell_fire
            .as_ref()
            .map_or_else(Vec::new, |fire| fire.holders.clone());
        self.context.events.clear();
        let initial_target = [GoatPlayerTarget {
            entity_index: self.state.entity_index,
            position: self.state.position,
            control_flags: 0,
        }];
        self.mission.set_external_collision_targets(
            self.live_world
                .goats()
                .iter()
                .map(goat_collision_target)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .map(villager_collision_target),
                ),
        );
        self.mission.begin_source_turn();
        self.reclaim_imported_source_slots();
        if self.mission.take_group_7_villager_release() {
            let released = self.live_world.release_villager_stage_group_7();
            debug_assert_eq!(released, 28);
        }
        let begin_goat_events = self.live_world.begin_source_turn(&initial_target);
        self.process_goat_events(&begin_goat_events);
        let mut source_views =
            level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
        let mut evidence = Vec::new();
        for source_slot in 1..ENTITY_COUNT {
            let source_slot = u16::try_from(source_slot).expect("source cursor fits u16");
            if let Some((owner_entity_index, payload)) =
                self.mission.take_building_possession_mailbox(source_slot)
            {
                self.live_world
                    .submit_house_possession(source_slot, owner_entity_index, payload)
                    .expect("possession mailbox targets a live eligible house");
            }
            let house_state_before_dispatch = self
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == source_slot && house.active)
                .is_some_and(|house| {
                    house.action == crate::house::HOUSE_REMOVE_ACTION
                        || (house.action == crate::house::HOUSE_LIVE_ACTION
                            && house.parameter.produces_villagers()
                            && ((house.pending_attacker != 0 && house.occupants > 2)
                                || (house.phase & 0x1f == 0
                                    && house.capacity > 5
                                    && i32::from(house.capacity) == house.occupants)))
                })
                .then(|| self.live_world.clone());
            let house_events = self
                .live_world
                .step_house_source_slot(source_slot, &self.terrain);
            if !self.process_house_events(&house_events) {
                self.live_world =
                    house_state_before_dispatch.expect("failed house dispatch was staged");
            }
            let player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: 0,
            }];
            let goat_events =
                self.live_world
                    .step_source_slot(source_slot, &self.terrain, &player_target);
            let villager_effects = self.live_world.step_villager_source_slot(
                source_slot,
                &self.terrain,
                &source_views,
            );
            self.mission
                .apply_imported_villager_effects(&villager_effects);
            self.process_goat_events(&goat_events);
            if let Some(house) = self
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(house_collision_target(house));
                self.mission.sync_imported_source_slot(
                    house.entity_index,
                    house.active,
                    house.action,
                    house.phase,
                    house.rng,
                    house.control_flags,
                );
            } else if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(goat_collision_target(goat));
                self.mission.sync_imported_source_slot(
                    goat.entity_index,
                    goat.active,
                    goat.action,
                    goat.phase,
                    goat.rng,
                    goat.control_flags,
                );
            } else if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_slot)
            {
                self.mission
                    .update_external_collision_target(villager_collision_target(villager));
                self.mission.sync_imported_source_slot(
                    villager.entity_index,
                    villager.active,
                    villager.action,
                    villager.phase,
                    villager.rng,
                    villager.control_flags,
                );
            }
            refresh_level1_current_source_view(&mut source_views, &self.live_world, source_slot);
            if let Some(dolmen) =
                self.world
                    .entities
                    .get(usize::from(source_slot))
                    .filter(|entity| {
                        entity.active
                            && entity.class == 2
                            && entity.model == 2
                            && entity.action == 6
                    })
            {
                self.mission.step_dolmen_source_slot(
                    &self.state,
                    source_slot,
                    dolmen.collision_target(),
                );
            }

            let (fireball_activated, possession_activated, castle_cast_yaw) = if source_slot
                == self.state.entity_index
            {
                self.mission
                    .prepare_player_action_before_flight(&mut self.state);
                evidence = if self.mission.exit_sequence_owns_player_action() {
                    self.state.tick = self.state.tick.wrapping_add(1);
                    Vec::new()
                } else {
                    match self.mission.player_action {
                        crate::level1_mission::PLAYER_ACTION_ACTIVE => {
                            step_outer_turn_with_context(
                                &mut self.state,
                                command,
                                0,
                                self.world.profile,
                                &self.terrain,
                                &mut self.context,
                            )
                        }
                        crate::level1_mission::PLAYER_ACTION_DYING => {
                            vec![step_entity_body_with_context(
                                &mut self.state,
                                self.world.profile,
                                &self.terrain,
                                &mut self.context,
                            )]
                        }
                        _ => Vec::new(),
                    }
                };
                if !self.mission.exit_sequence_owns_player_action() {
                    let flight_proposed = evidence
                        .last()
                        .map_or(self.state.position, |pass| pass.proposed);
                    self.mission
                        .advance_player_lifecycle_after_flight_with_proposed(
                            &self.terrain,
                            &mut self.state,
                            &mut self.context,
                            flight_proposed,
                        );
                }
                let activated_model = |model| {
                    self.context.spell_fire.as_ref().is_some_and(|fire| {
                        fire.holders.iter().any(|holder| {
                            holder.model == model
                                && holder.activation_value != 0
                                && holder.active_value == holder.activation_value.cast_signed()
                                && holders_before
                                    .iter()
                                    .find(|before| before.entity_index == holder.entity_index)
                                    .is_none_or(|before| before.active_value != holder.active_value)
                        })
                    })
                };
                let fireball = activated_model(0);
                let possession = activated_model(1);
                let castle = activated_model(CASTLE_SPELL_FAMILY);
                self.mission
                    .capture_spell_holder_active_values(&self.context);
                (fireball, possession, castle.then_some(self.state.yaw))
            } else {
                (false, false, None)
            };
            let mission_event_start = self.mission.events().len();
            if !self.mission.exit_sequence_owns_player_action()
                && self.mission.player_action == crate::level1_mission::PLAYER_ACTION_ACTIVE
            {
                self.mission.dispatch_speed_holder_slot(
                    source_slot,
                    &mut self.state,
                    &mut self.context,
                );
            }
            let source_effects = self.mission.dispatch_source_slot_with_views_and_look(
                &self.terrain,
                &self.state,
                &source_views,
                source_slot,
                fireball_activated,
                possession_activated,
                castle_cast_yaw,
                0,
                0,
            );
            self.mission
                .apply_pending_player_transients(&mut self.state);
            apply_level1_source_effects(&mut self.mission, &mut self.live_world, &source_effects)
                .expect("resident source effects apply after slot dispatch");
            if source_slot == self.state.entity_index && self.mission.exit_sequence_active {
                self.mission.advance_exit_sequence(
                    &self.terrain,
                    &mut self.state,
                    &mut self.context,
                    &mut self.camera_fov,
                );
            }
            self.apply_castle_terrain_updates();
            self.inject_new_fireball_impacts(mission_event_start);
            self.reclaim_imported_source_slots();
        }
        self.live_world.finish_source_turn();
        self.mission.finish_source_turn(&self.state);
        if let Some(fire) = self.context.spell_fire.as_mut() {
            // The class-15 handlers consume one active tick after their
            // same-turn projectile factories: model 0 in `sub_693F0` and
            // model 1 in `sub_69640`. Keep the direct Rust session in
            // lockstep with the C ABI production session so both spells can
            // return to their inactive, retriggerable state.
            for holder in &mut fire.holders {
                if matches!(holder.model, 0 | 1) && holder.active_value > 0 {
                    holder.active_value -= 1;
                }
            }
        }
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        evidence
    }

    fn apply_castle_terrain_updates(&mut self) {
        let updates = self
            .mission
            .drain_castle_terrain_updates()
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return;
        }
        for update in updates {
            if update.is_ambient() {
                self.terrain
                    .apply_ambient_update(update)
                    .expect("qualified Level 1 ambient terrain update");
            } else {
                let footprint = self
                    .mission
                    .castle_stage_one_footprint()
                    .expect("Castle terrain updates require the validated BUILD[1] footprint");
                self.terrain
                    .apply_castle_update(update, footprint)
                    .expect("qualified Level 1 Castle terrain update");
            }
        }
    }

    fn process_goat_events(&mut self, events: &[GoatStepEvent]) {
        for event in events {
            match *event {
                GoatStepEvent::ManaConverted { entity_index, mana } => {
                    if let Some((position, yaw, rng)) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| (goat.position, goat.yaw, goat.rng))
                    {
                        self.mission.spawn_mana_sphere_from_death(
                            &self.terrain,
                            entity_index,
                            position,
                            yaw,
                            rng,
                            mana,
                            0,
                        );
                        self.mission.disable_imported_source_slot(entity_index);
                    }
                }
                GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
                    if let Some(position) = self
                        .live_world
                        .goats()
                        .iter()
                        .find(|goat| goat.entity_index == entity_index)
                        .map(|goat| goat.position)
                    {
                        self.mission
                            .spawn_death_burst_from_death(entity_index, position);
                    }
                }
                GoatStepEvent::Reclaimed { .. } => {}
                _ => {}
            }
        }
    }

    fn process_house_events(&mut self, events: &[HouseStepEvent]) -> bool {
        let allocation_transaction = events.iter().any(|event| {
            matches!(
                event,
                HouseStepEvent::ResidentRequested { .. } | HouseStepEvent::RemoveFootprint { .. }
            )
        });
        let terrain_before_dispatch = allocation_transaction.then(|| self.terrain.clone());
        let mission_before_dispatch = allocation_transaction.then(|| self.mission.clone());
        let pending_effect_count = self.pending_house_effects.len();
        for event in events {
            match *event {
                HouseStepEvent::Possessed {
                    entity_index,
                    owner_entity_index,
                    payload,
                } => self.mission.apply_building_possession(
                    &self.state,
                    entity_index,
                    owner_entity_index,
                    payload,
                ),
                HouseStepEvent::RemoveFootprint {
                    entity_index,
                    attacker_entity_index,
                    position,
                    building_index,
                    occupants,
                    rng,
                    xtype,
                    ..
                } => {
                    let produces_residents = self
                        .live_world
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == entity_index)
                        .is_some_and(|house| house.parameter.produces_villagers());
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .cloned()
                        .expect("Level 1 action-53 house BUILD footprint");
                    let outcome = self
                        .terrain
                        .apply_house_removal(
                            position,
                            rng,
                            occupants,
                            produces_residents,
                            &footprint,
                        )
                        .expect("qualified Level 1 house terrain removal");
                    self.live_world
                        .commit_house_removal_state(
                            entity_index,
                            outcome.final_rng,
                            outcome.final_occupants,
                        )
                        .expect("commit action-53 house state");
                    let requests = outcome
                        .residents
                        .iter()
                        .map(|resident| HouseResidentRequest {
                            position: resident.position,
                            model: resident.model,
                            action: resident.action,
                            attacker_entity_index,
                        })
                        .collect::<Vec<_>>();
                    let Some(_order) = self
                        .mission
                        .commit_imported_house_demolition(entity_index, xtype, &requests)
                        .expect("qualified action-53 resident/xtype transaction")
                    else {
                        self.terrain = terrain_before_dispatch
                            .expect("action-53 terrain transaction was staged");
                        self.mission = mission_before_dispatch
                            .expect("action-53 mission transaction was staged");
                        self.pending_house_effects.truncate(pending_effect_count);
                        return false;
                    };
                    for resident in outcome.residents {
                        self.pending_house_effects
                            .push(HouseStepEvent::ResidentRequested {
                                entity_index,
                                attacker_entity_index,
                                position: resident.position,
                                model: resident.model,
                                action: resident.action,
                            });
                    }
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ResidentRequested {
                    attacker_entity_index,
                    position,
                    model,
                    action,
                    ..
                } => {
                    let request = HouseResidentRequest {
                        position,
                        model,
                        action,
                        attacker_entity_index,
                    };
                    let Some(_) = self
                        .mission
                        .spawn_house_residents_atomic(&[request])
                        .expect("qualified action-52 resident transaction")
                    else {
                        self.terrain = terrain_before_dispatch
                            .expect("action-52 terrain transaction was staged");
                        self.mission = mission_before_dispatch
                            .expect("action-52 mission transaction was staged");
                        self.pending_house_effects.truncate(pending_effect_count);
                        return false;
                    };
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {
                    self.pending_house_effects.push(*event);
                }
            }
        }
        true
    }

    fn reclaim_imported_source_slots(&mut self) {
        for source_slot in self.mission.take_reclaimed_imported_source_slots() {
            let _ = self.live_world.reclaim_source_slot(source_slot);
        }
    }

    fn inject_new_fireball_impacts(&mut self, event_start: usize) {
        let impacts = self.mission.events()[event_start..]
            .iter()
            .filter_map(|event| {
                let crate::level1_mission::Level1MissionEvent::FireballImpacted {
                    impact_id,
                    position,
                    ..
                } = event
                else {
                    return None;
                };
                let payload = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == *impact_id)
                    .map(|impact| {
                        (
                            impact.payload,
                            impact.owner_entity_index,
                            impact.external_target_entity_index,
                        )
                    })?;
                Some((*position, payload.0, payload.1, payload.2))
            })
            .collect::<Vec<_>>();
        for (position, payload, owner_entity_index, direct_target_entity_index) in impacts {
            let targets = self
                .live_world
                .goats()
                .iter()
                .filter(|goat| {
                    goat.active
                        && goat.entity_index != owner_entity_index
                        && (goat.entity_index == direct_target_entity_index
                            || crate::level1_mission::source_entities_overlap(
                                position,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                goat.position,
                                goat.collision_half_width,
                                goat.collision_vertical_offset,
                                goat.collision_half_height,
                            ))
                })
                .map(|goat| goat.entity_index)
                .chain(
                    self.live_world
                        .villagers()
                        .iter()
                        .filter(|villager| {
                            villager.active
                                && villager.entity_index != owner_entity_index
                                && (villager.entity_index == direct_target_entity_index
                                    || crate::level1_mission::source_entities_overlap(
                                        position,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
                                        villager.position,
                                        villager.collision_half_width,
                                        villager.collision_vertical_offset,
                                        villager.collision_half_height,
                                    ))
                        })
                        .map(|villager| villager.entity_index),
                )
                .chain(
                    self.live_world
                        .houses()
                        .iter()
                        .filter(|house| {
                            house.active
                                && house.entity_index != owner_entity_index
                                && (house.entity_index == direct_target_entity_index
                                    || crate::level1_mission::source_entities_overlap_xy(
                                        position,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        crate::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        house.position,
                                        house.collision_half_width,
                                        house.collision_half_depth,
                                    ))
                        })
                        .map(|house| house.entity_index),
                )
                .collect::<Vec<_>>();
            for target in targets {
                let _ = self.live_world.inject_damage(
                    target,
                    payload,
                    owner_entity_index.cast_signed(),
                );
            }
        }
    }
}

fn validate_level_one_mission_stages(runtime: &RuntimeSnapshot) -> Result<()> {
    let expected = [
        (5, 29_440_u16, 54_272_u16, 0_u32),
        (5, 49_664_u16, 54_528_u16, 0_u32),
        (7, 0, 0, 4),
        (0, 0, 0, 15),
        (7, 0, 0, 19),
    ];
    if runtime.stage_index != 5 {
        return Err(NativeWorldError::new(format!(
            "Level 1 must expose five mission stages, got {}",
            runtime.stage_index
        )));
    }
    for (index, (kind, x, y, scalar)) in expected.into_iter().enumerate() {
        let stage = &runtime.stages[index];
        if stage.kind != kind
            || stage.axis_x.cast_unsigned() != x
            || stage.axis_y.cast_unsigned() != y
            || stage.relation != RuntimeRelation::Scalar(scalar)
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 mission stage {index} mismatch: {stage:?}"
            )));
        }
    }
    Ok(())
}

fn validate_level_one_mission_presentation(presentation: &PresentationSelectors) -> Result<()> {
    let expected = [
        (
            ARCHER_RENDER_INDEX,
            ARCHER_BASE_TMAP,
            ARCHER_RENDER_MODE,
            ARCHER_LOGICAL_HEIGHT,
            133_u16,
            1_i8,
        ),
        (
            FIREFLY_RENDER_INDEX,
            FIREFLY_BASE_TMAP,
            FIREFLY_RENDER_MODE,
            FIREFLY_LOGICAL_HEIGHT,
            210,
            1,
        ),
        (
            FIREBALL_RENDER_INDEX,
            FIREBALL_BASE_TMAP,
            FIREBALL_RENDER_MODE,
            FIREBALL_LOGICAL_HEIGHT,
            97,
            1,
        ),
        (
            FIREBALL_IMPACT_RENDER_INDEX,
            FIREBALL_IMPACT_BASE_TMAP,
            FIREBALL_IMPACT_RENDER_MODE,
            FIREBALL_IMPACT_LOGICAL_HEIGHT,
            256,
            -1,
        ),
        (
            POSSESSION_IMPACT_RENDER_INDEX,
            POSSESSION_IMPACT_BASE_TMAP,
            POSSESSION_IMPACT_RENDER_MODE,
            POSSESSION_IMPACT_LOGICAL_HEIGHT,
            264,
            -1,
        ),
        (
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_ATTRACTION_BASE_TMAP,
            POSSESSION_ATTRACTION_RENDER_MODE,
            POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            133,
            1,
        ),
        (
            SPELL_JAR_RENDER_INDEX,
            SPELL_JAR_BASE_TMAP,
            SPELL_JAR_RENDER_MODE,
            SPELL_JAR_LOGICAL_HEIGHT,
            184,
            -1,
        ),
        (
            MANA_CARRIER_RENDER_INDEX,
            MANA_CARRIER_BASE_TMAP,
            MANA_CARRIER_RENDER_MODE,
            MANA_CARRIER_LOGICAL_HEIGHT,
            545,
            -16,
        ),
    ];
    for (index, base_tmap, render_mode, logical_height, speed, byte_11) in expected {
        let parameter = presentation
            .particles
            .get(usize::from(index))
            .ok_or_else(|| {
                NativeWorldError::new(format!("Level 1 render parameter {index} is absent"))
            })?;
        if parameter.base_tmap != base_tmap
            || parameter.word_2 != 8_492
            || parameter.word_4 != 42
            || parameter.speed != speed
            || parameter.rotation_speed != logical_height
            || parameter.byte_10 != 0
            || parameter.byte_11 != byte_11
            || parameter.render_mode != render_mode
            || parameter.byte_13 != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 render parameter {index} mismatch: {parameter:?}"
            )));
        }
    }
    for (phase, ((logical_height, speed), byte_11)) in GROUND_FIRE_LOGICAL_HEIGHTS
        .into_iter()
        .zip(GROUND_FIRE_PRESENTATION_SPEEDS)
        .zip(GROUND_FIRE_PRESENTATION_BYTE_11)
        .enumerate()
    {
        let index = GROUND_FIRE_FIRST_RENDER_INDEX
            + u16::try_from(phase).expect("eight ground-fire phases fit u16");
        let parameter = presentation
            .particles
            .get(usize::from(index))
            .ok_or_else(|| {
                NativeWorldError::new(format!("Level 1 render parameter {index} is absent"))
            })?;
        if parameter.base_tmap != GROUND_FIRE_BASE_TMAP
            || parameter.word_2 != 8_492
            || parameter.word_4 != 42
            || parameter.speed != speed
            || parameter.rotation_speed != logical_height
            || parameter.byte_10 != 0
            || parameter.byte_11 != byte_11
            || parameter.render_mode != GROUND_FIRE_RENDER_MODE
            || parameter.byte_13 != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 1 render parameter {index} mismatch: {parameter:?}"
            )));
        }
    }
    Ok(())
}

fn level_one_global_mana_denominator(entities: &[EntitySnapshot]) -> Result<i32> {
    let mut denominator = 1_i32;
    for entity in entities.iter().skip(1).filter(|entity| entity.active) {
        let contributes = entity.class == 5
            || entity.class == 10 && matches!(entity.model, 39 | 45 | 58)
            || entity.class == 3 && matches!(entity.model, 2 | 3);
        if contributes {
            denominator = denominator
                .checked_add(entity.mana)
                .ok_or_else(|| NativeWorldError::new("Level 1 global mana denominator overflow"))?;
        }
    }
    if denominator <= 0 {
        return Err(NativeWorldError::new(
            "Level 1 global mana denominator is not positive",
        ));
    }
    Ok(denominator)
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
        vga_resolution_selector: as_u8(
            require_value(
                require_object(manifest, "presentation_dependencies", "manifest")?,
                "vga_resolution_selector",
                "manifest.presentation_dependencies",
            )?,
            "manifest.presentation_dependencies.vga_resolution_selector",
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
        let particle_parameter_raw = read_i16(record, 94, "entity particle parameter index")?;
        let particle_parameter_index =
            (particle_parameter_raw >= 0).then(|| particle_parameter_raw.cast_unsigned());
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
            max_life: read_i32(record, 8, "entity max life")?,
            life: read_i32(record, 12, "entity life")?,
            next_relation: next,
            class: read_u8(record, 67, "entity class")?,
            model: read_u8(record, 68, "entity model")?,
            control_flags: read_u32(record, 16, "entity control flags")?,
            entity_rng: read_u16(record, 20, "entity RNG")?,
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
            particle_parameter_index,
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
        let left_subspell_index = read_u8(record, 558, "left subspell index")?.cast_signed();
        let right_subspell_index = read_u8(record, 559, "right subspell index")?.cast_signed();
        let selected_spell_ui_index = read_u8(record, 561, "selected spell UI index")?;
        let selected_subspell_index =
            read_u8(record, 562, "selected subspell index")?.cast_signed();
        if !(0..=2).contains(&left_subspell_index)
            || !(0..=2).contains(&right_subspell_index)
            || usize::from(selected_spell_ui_index) >= SOURCE_SPELL_INDEX_MAP.len()
            || !(0..=2).contains(&selected_subspell_index)
        {
            return Err(NativeWorldError::new(format!(
                "player {slot} subspell/menu selection is out of bounds"
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
        let mut spell_hand_eligibility = [0_u8; PLAYER_SPELL_COUNT];
        spell_hand_eligibility.copy_from_slice(&record[398..424]);
        let mut family_selected_tiers = [0_i8; PLAYER_SPELL_COUNT];
        for (tier, value) in family_selected_tiers.iter_mut().zip(&record[528..554]) {
            *tier = value.cast_signed();
        }
        players.push(PlayerSnapshot {
            slot: u8::try_from(slot).expect("eight player slots"),
            is_ai: ai == 1,
            entity_index,
            active_player_index: read_i16(record, 6, "active player index")?,
            turn: read_i32(record, 8, "player turn")?,
            left_spell_slot: u8::try_from(left).expect("validated spell slot"),
            right_spell_slot: u8::try_from(right).expect("validated spell slot"),
            left_subspell_index,
            right_subspell_index,
            selected_spell_ui_index,
            selected_subspell_index,
            enabled_spell_holders: holders,
            spell_hand_eligibility,
            family_selected_tiers,
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
        stage_index: read_u8(bytes, 65, "runtime stage index")?,
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
            left_subspell_index: player.left_subspell_index,
            right_subspell_index: player.right_subspell_index,
            selected_spell_ui_index: player.selected_spell_ui_index,
            selected_subspell_index: player.selected_subspell_index,
            spell_hand_eligibility: player.spell_hand_eligibility,
            family_selected_tiers: player.family_selected_tiers,
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
    let actual_conformance = conformance_hash(state, context);
    if actual_conformance != expected_conformance {
        return Err(NativeWorldError::new(format!(
            "reset conformance hash mismatch: expected {expected_conformance:016x}, got {actual_conformance:016x}"
        )));
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
    for index in LEVEL_ONE_VILLAGER_INDICES {
        let villager = entities.get_mut(usize::from(index)).ok_or_else(|| {
            NativeWorldError::new(format!("Villager entity {index} is out of bounds"))
        })?;
        if !villager.active || villager.class != VILLAGER_CLASS || villager.model != VILLAGER_MODEL
        {
            return Err(NativeWorldError::new(format!(
                "Villager authority row {index} is not active class {VILLAGER_CLASS}/model {VILLAGER_MODEL}"
            )));
        }
        villager.authority = EntityAuthority::VillagerLive;
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

fn read_owner_cache_output(root: &Path, relative: &str) -> Result<Vec<u8>> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(NativeWorldError::new(
            "owner cache output path is not a confined relative path",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve owner cache: {error}")))?;
    let candidate = root.join(relative_path);
    if fs::symlink_metadata(&candidate)
        .map_err(|error| NativeWorldError::new(format!("cannot inspect owner output: {error}")))?
        .file_type()
        .is_symlink()
    {
        return Err(NativeWorldError::new("owner cache output is a symlink"));
    }
    let resolved = candidate
        .canonicalize()
        .map_err(|error| NativeWorldError::new(format!("cannot resolve owner output: {error}")))?;
    if !resolved.starts_with(&root) {
        return Err(NativeWorldError::new("owner cache output escapes its root"));
    }
    read_file(&resolved)
}

#[derive(Debug, Default)]
struct LevelOneCacheAssets {
    sound_asset_refs: BTreeMap<u16, u32>,
    music_asset_ref: Option<u32>,
    goat_tmap_bank: Option<GoatTmapBank>,
    castle_stage_one_footprint: Option<CastleBuildFootprint>,
    house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
}

fn append_level_one_cache_assets(
    root: &Path,
    package_sha256: &str,
    closure_sha256: &str,
    refs: &mut Vec<AssetRef>,
) -> Result<LevelOneCacheAssets> {
    let manifest_bytes = read_owner_cache_output(root, "manifest.json")?;
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
    let castle_asset = require_object(assets, "level1_castle_stage", "owner cache assets")?;
    require_exact_u64(castle_asset, "build_index", 1, "Castle cache asset")?;
    require_exact_u64(castle_asset, "effective_width", 8, "Castle cache asset")?;
    require_exact_u64(castle_asset, "effective_height", 8, "Castle cache asset")?;
    let castle_descriptor_path = require_string(castle_asset, "path", "Castle cache asset")?;
    let castle_cells_path = require_string(castle_asset, "cells_path", "Castle cache asset")?;
    let house_assets = require_object(assets, "level1_house_footprints", "owner cache assets")?;
    let house_descriptor_path = require_string(house_assets, "path", "house BUILD cache asset")?;
    let house_cells_path = require_string(house_assets, "cells_path", "house BUILD cache asset")?;
    let audio = require_object(assets, "level1_audio", "audio cache assets")?;
    let sound_path = require_string(audio, "sound_descriptor_path", "audio cache Level 1 assets")?;
    let music_path = require_string(audio, "music_descriptor_path", "audio cache Level 1 assets")?;
    let load_output = |path: &str| -> Result<Vec<u8>> {
        let (expected_hash, expected_bytes) = outputs
            .get(path)
            .ok_or_else(|| NativeWorldError::new(format!("audio output is undeclared: {path}")))?;
        let bytes = read_owner_cache_output(root, path)?;
        if bytes.len() as u64 != *expected_bytes || sha256_hex(&bytes) != *expected_hash {
            return Err(NativeWorldError::new(format!(
                "audio output hash/size mismatch: {path}"
            )));
        }
        Ok(bytes)
    };

    let castle_descriptor_bytes = load_output(castle_descriptor_path)?;
    let castle_descriptor: Value =
        serde_json::from_slice(&castle_descriptor_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid Castle BUILD descriptor: {error}"))
        })?;
    let castle_descriptor = as_object(&castle_descriptor, "Castle BUILD descriptor")?;
    if require_string(castle_descriptor, "schema", "Castle BUILD descriptor")?
        != "mc2-level1-castle-build-footprint/v1"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD descriptor schema mismatch",
        ));
    }
    require_exact_u64(
        castle_descriptor,
        "build_index",
        1,
        "Castle BUILD descriptor",
    )?;
    require_exact_u64(
        castle_descriptor,
        "vga_resolution_selector",
        1,
        "Castle BUILD descriptor",
    )?;
    let dimensions = require_object(castle_descriptor, "dimensions", "Castle BUILD descriptor")?;
    let raw = require_object(dimensions, "raw_tab", "Castle BUILD dimensions")?;
    let runtime = require_object(dimensions, "loader_runtime", "Castle BUILD dimensions")?;
    let effective = require_object(
        dimensions,
        "castle_consumer_effective",
        "Castle BUILD dimensions",
    )?;
    for (row, width, height, label) in [
        (raw, 8, 8, "raw Castle BUILD dimensions"),
        (runtime, 16, 16, "runtime Castle BUILD dimensions"),
        (effective, 8, 8, "effective Castle BUILD dimensions"),
    ] {
        require_exact_u64(row, "width", width, label)?;
        require_exact_u64(row, "height", height, label)?;
    }
    if require_hash_string(raw, "tab_record_sha256", "Castle BUILD raw dimensions")?
        != "27a9da49ded1a0aa1a4e01dbaab8634404f5bf9d6789a5785884e94e35e2f804"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD TAB record identity changed",
        ));
    }
    let cells = require_object(castle_descriptor, "cells", "Castle BUILD descriptor")?;
    if require_string(cells, "path", "Castle BUILD cells")? != castle_cells_path
        || require_hash_string(cells, "sha256", "Castle BUILD cells")?
            != "f9fb9d8a5c6d2841849bba03479e6503966fa4ec14020508c343296602aa8ae3"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD descriptor cell binding changed",
        ));
    }
    require_exact_u64(cells, "bytes", 128, "Castle BUILD cells")?;
    require_exact_u64(cells, "stride_bytes", 2, "Castle BUILD cells")?;
    let castle_cells = load_output(castle_cells_path)?;
    if sha256_hex(&castle_cells)
        != "f9fb9d8a5c6d2841849bba03479e6503966fa4ec14020508c343296602aa8ae3"
    {
        return Err(NativeWorldError::new(
            "Castle BUILD materialized cells changed",
        ));
    }
    let parameters = require_object(
        castle_descriptor,
        "building_parameters",
        "Castle BUILD descriptor",
    )?;
    require_exact_u64(parameters, "record_index", 1, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "word_0", 2, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "flags", 3, "Castle BUILD parameters")?;
    require_exact_u64(parameters, "byte_3", 0, "Castle BUILD parameters")?;
    if require_hash_string(parameters, "record_sha256", "Castle BUILD parameters")?
        != "7d6e4762a22170651b49bef86a5bb9d5de987f5d5f3e546901291fd24c411d39"
    {
        return Err(NativeWorldError::new(
            "Castle BLDGPRM record identity changed",
        ));
    }
    let castle_stage_one_footprint = CastleBuildFootprint::from_owner_cells(1, 8, 8, &castle_cells)
        .map_err(NativeWorldError::new)?;

    let house_descriptor_bytes = load_output(house_descriptor_path)?;
    let house_descriptor: Value =
        serde_json::from_slice(&house_descriptor_bytes).map_err(|error| {
            NativeWorldError::new(format!("invalid house BUILD descriptor: {error}"))
        })?;
    let house_descriptor = as_object(&house_descriptor, "house BUILD descriptor")?;
    if require_string(house_descriptor, "schema", "house BUILD descriptor")?
        != "mc2-level1-house-build-footprints/v1"
    {
        return Err(NativeWorldError::new(
            "house BUILD descriptor schema mismatch",
        ));
    }
    require_exact_u64(
        house_descriptor,
        "vga_resolution_selector",
        1,
        "house BUILD descriptor",
    )?;
    let house_cells_descriptor =
        require_object(house_descriptor, "cells", "house BUILD descriptor")?;
    if require_string(house_cells_descriptor, "path", "house BUILD cells")? != house_cells_path {
        return Err(NativeWorldError::new(
            "house BUILD descriptor cell path mismatch",
        ));
    }
    require_exact_u64(
        house_cells_descriptor,
        "stride_bytes",
        2,
        "house BUILD cells",
    )?;
    let house_cells = load_output(house_cells_path)?;
    if require_hash_string(house_cells_descriptor, "sha256", "house BUILD cells")?
        != sha256_hex(&house_cells)
        || require_value(house_cells_descriptor, "bytes", "house BUILD cells")?.as_u64()
            != Some(house_cells.len() as u64)
    {
        return Err(NativeWorldError::new(
            "house BUILD descriptor cell binding changed",
        ));
    }
    let expected_house_indices = [17_u8, 26, 29, 30, 31, 34, 42, 47, 50, 51, 52, 53];
    let house_rows = require_array(house_descriptor, "footprints", "house BUILD descriptor")?;
    if house_rows.len() != expected_house_indices.len() {
        return Err(NativeWorldError::new(
            "house BUILD descriptor row count changed",
        ));
    }
    let mut house_build_footprints = BTreeMap::new();
    for (row, expected_index) in house_rows.iter().zip(expected_house_indices) {
        let row = as_object(row, "house BUILD row")?;
        let build_index = as_u8(
            require_value(row, "build_index", "house BUILD row")?,
            "house BUILD index",
        )?;
        if build_index != expected_index {
            return Err(NativeWorldError::new(
                "house BUILD descriptor ordering changed",
            ));
        }
        let width = as_u8(
            require_value(row, "width", "house BUILD row")?,
            "house BUILD width",
        )?;
        let height = as_u8(
            require_value(row, "height", "house BUILD row")?,
            "house BUILD height",
        )?;
        let offset = usize::try_from(
            require_value(row, "blob_offset", "house BUILD row")?
                .as_u64()
                .ok_or_else(|| NativeWorldError::new("house BUILD offset is not u64"))?,
        )
        .map_err(|_| NativeWorldError::new("house BUILD offset exceeds usize"))?;
        let length = usize::try_from(
            require_value(row, "bytes", "house BUILD row")?
                .as_u64()
                .ok_or_else(|| NativeWorldError::new("house BUILD length is not u64"))?,
        )
        .map_err(|_| NativeWorldError::new("house BUILD length exceeds usize"))?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= house_cells.len())
            .ok_or_else(|| NativeWorldError::new("house BUILD row escapes cell blob"))?;
        let payload = &house_cells[offset..end];
        if length != usize::from(width) * usize::from(height) * 2
            || require_hash_string(row, "sha256", "house BUILD row")? != sha256_hex(payload)
        {
            return Err(NativeWorldError::new(format!(
                "house BUILD row {build_index} binding changed"
            )));
        }
        let footprint = CastleBuildFootprint::from_owner_cells(build_index, width, height, payload)
            .map_err(NativeWorldError::new)?;
        house_build_footprints.insert(build_index, footprint);
    }

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
    let expected_sound_ids = [
        1_u16, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37, 39, 40,
        41, 43, 44, 46, 54, 55, 56, 57, 63,
    ];
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
    let terrain_assets = require_object(assets, "terrain_materials", "owner cache assets")?;
    let materials_path = require_string(
        terrain_assets,
        "path",
        "owner cache terrain-material assets",
    )?;
    let materials_bytes = load_output(materials_path)?;
    let materials: Value = serde_json::from_slice(&materials_bytes)
        .map_err(|error| NativeWorldError::new(format!("invalid terrain materials: {error}")))?;
    let materials = as_object(&materials, "terrain materials")?;
    let tmaps1 = require_object(materials, "tmaps1", "terrain materials")?;
    let golden = require_object(tmaps1, "goat_animation_golden", "TMAPS1 materials")?;
    if require_hash_string(golden, "file_sha256", "Goat TMAP golden")?
        != "a5c3d430ba22c3503453124d76119ed7b93ff6ea8445e23cd616faa1015b5f64"
        || require_hash_string(golden, "source_oracle_manifest_sha256", "Goat TMAP golden")?
            != "cad16d830ece1f6422ec4f3f6a0a5ab4954fe6c20344e2e03c6ce1f0bc41e73b"
        || require_string(golden, "selector_state_scope", "Goat TMAP golden")?
            != "selector_global_shared_TMAP_state_not_per_entity"
    {
        return Err(NativeWorldError::new(
            "Goat TMAP golden identity or selector scope changed",
        ));
    }
    let textures = require_array(tmaps1, "textures", "TMAPS1 materials")?;
    let expected_cursors = [
        [490_u32, 818, 1146, 1472, 1812],
        [710_u32, 1196, 1700, 2202, 2724],
        [490_u32, 822, 1152, 1482, 1816],
    ];
    let expected_hashes = [
        [
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
            "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
            "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
            "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
        ],
        [
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
            "4068c077eb4f66e39ae3bd13202462b477a418aa7781822d09f0feb3337765c7",
            "d5d9ccc838119171cb7ac06315e6e6994e72c058cf40b3b9a9b6fb6709e8931f",
            "f0f656d86c51628b5f0aac90932b79b448f64ef1ef38b41fcffc9d275df71fb1",
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
        ],
        [
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
            "897331a6a567a6f64be563755ba33a5ab1e75e0a1d77d5a1fc9a67ca8657d44a",
            "22d9b6b2bad1a7fef49afcb05284c513f14200852c5cbaad097f684a0197f940",
            "752525ceb8c9a097f0676def3bdeb58258bd093ce7efacf3fe170dc7ec4cbc84",
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
        ],
    ];
    let expected_payload_bytes = [1818_u32, 2730, 1822];
    let expected_state_names = [
        "reset-base",
        "frame-01",
        "frame-02",
        "frame-03",
        "frame-04",
        "closure-frame-01",
    ];
    let mut selector_rows = Vec::with_capacity(GOAT_TMAP_SELECTORS.len());
    for selector_ordinal in 0..GOAT_TMAP_SELECTORS.len() {
        let selector_row = (|| -> Result<_> {
            let expected_selector = GOAT_TMAP_SELECTORS[selector_ordinal];
            let texture = textures
                .iter()
                .find(|texture| {
                    texture
                        .as_object()
                        .and_then(|object| object.get("index"))
                        .and_then(Value::as_u64)
                        == Some(u64::from(expected_selector))
                })
                .ok_or_else(|| {
                    NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} is absent"
                    ))
                })?;
            let texture = as_object(texture, "Goat TMAP texture")?;
            if require_value(texture, "animated", "Goat TMAP texture")?.as_bool() != Some(true)
                || require_exact_u64(texture, "frame_count", 4, "Goat TMAP texture").is_err()
                || require_exact_u64(texture, "header_word", 5123, "Goat TMAP texture").is_err()
                || require_exact_u64(
                    texture,
                    "animation_or_allocation_group",
                    152,
                    "Goat TMAP texture",
                )
                .is_err()
                || require_string(texture, "animation_materialization", "Goat TMAP texture")?
                    != "exact_qualified_Goat_SS2_four_transition_states; selector_global_shared_TMAP_state_not_per_entity"
            {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} descriptor changed"
                )));
            }
            let width = as_u16(
                require_value(texture, "width", "Goat TMAP texture")?,
                "Goat TMAP width",
            )?;
            let height = as_u16(
                require_value(texture, "height", "Goat TMAP texture")?,
                "Goat TMAP height",
            )?;
            let expected_width = if expected_selector == 153 { 32 } else { 22 };
            if width != expected_width || height != 22 {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} dimensions changed"
                )));
            }
            let decoded_path =
                require_string(texture, "decoded_payload_path", "Goat TMAP texture")?;
            let decoded_payload = load_output(decoded_path)?;
            if decoded_payload.len() != expected_payload_bytes[selector_ordinal] as usize {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} payload extent changed"
                )));
            }
            let rows = require_array(texture, "animation_states", "Goat TMAP texture")?;
            if rows.len() != expected_state_names.len() {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} state count changed"
                )));
            }
            let mut runtime_states = Vec::with_capacity(GOAT_TMAP_STATE_COUNT);
            for (state_ordinal, row) in rows.iter().take(GOAT_TMAP_STATE_COUNT).enumerate() {
                let row = as_object(row, "Goat TMAP animation state")?;
                if require_string(row, "state", "Goat TMAP animation state")?
                    != expected_state_names[state_ordinal]
                    || as_u32(
                        require_value(row, "stream_cursor", "Goat TMAP animation state")?,
                        "Goat TMAP stream cursor",
                    )? != expected_cursors[selector_ordinal][state_ordinal]
                    || as_u8(
                        require_value(row, "frame_index", "Goat TMAP animation state")?,
                        "Goat TMAP frame index",
                    )? != state_ordinal as u8 + 1
                    || as_u16(
                        require_value(row, "width", "Goat TMAP animation state")?,
                        "Goat TMAP state width",
                    )? != width
                    || as_u16(
                        require_value(row, "height", "Goat TMAP animation state")?,
                        "Goat TMAP state height",
                    )? != height
                    || require_hash_string(row, "sha256", "Goat TMAP animation state")?
                        != expected_hashes[selector_ordinal][state_ordinal]
                {
                    return Err(NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} state {state_ordinal} changed"
                    )));
                }
                let path = require_string(row, "indices_path", "Goat TMAP animation state")?;
                let pixels = load_output(path)?;
                let declared_bytes = require_value(row, "bytes", "Goat TMAP animation state")?
                    .as_u64()
                    .ok_or_else(|| {
                        NativeWorldError::new("Goat TMAP state byte count is not u64")
                    })?;
                if pixels.len() as u64 != declared_bytes
                    || sha256_hex(&pixels) != expected_hashes[selector_ordinal][state_ordinal]
                {
                    return Err(NativeWorldError::new(format!(
                        "Goat TMAP selector {expected_selector} state payload changed"
                    )));
                }
                runtime_states.push(GoatTmapFrame {
                    state_ordinal: state_ordinal as u8,
                    frame_index: state_ordinal as u8 + 1,
                    stream_cursor: expected_cursors[selector_ordinal][state_ordinal],
                    width,
                    height,
                    sha256: expected_hashes[selector_ordinal][state_ordinal].to_owned(),
                    pixels: pixels.into_boxed_slice(),
                });
            }
            let closure = as_object(&rows[5], "Goat TMAP closure state")?;
            if require_string(closure, "state", "Goat TMAP closure state")?
                != expected_state_names[5]
                || as_u32(
                    require_value(closure, "stream_cursor", "Goat TMAP closure state")?,
                    "Goat TMAP closure cursor",
                )? != expected_cursors[selector_ordinal][1]
                || as_u8(
                    require_value(closure, "frame_index", "Goat TMAP closure state")?,
                    "Goat TMAP closure frame index",
                )? != 2
                || require_hash_string(closure, "sha256", "Goat TMAP closure state")?
                    != expected_hashes[selector_ordinal][1]
                || require_string(closure, "indices_path", "Goat TMAP closure state")?
                    != require_string(
                        as_object(&rows[1], "Goat TMAP frame-01 state")?,
                        "indices_path",
                        "Goat TMAP frame-01 state",
                    )?
            {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} closure changed"
                )));
            }
            if runtime_states[4].pixels != runtime_states[0].pixels {
                return Err(NativeWorldError::new(format!(
                    "Goat TMAP selector {expected_selector} frame 4 does not restore reset pixels"
                )));
            }
            let states: [GoatTmapFrame; GOAT_TMAP_STATE_COUNT] = runtime_states
                .try_into()
                .map_err(|_| NativeWorldError::new("Goat TMAP runtime state count changed"))?;
            Ok(GoatTmapSelectorBank {
                selector: expected_selector,
                stream_start: expected_cursors[selector_ordinal][0],
                payload_bytes: expected_payload_bytes[selector_ordinal],
                states,
            })
        })()?;
        selector_rows.push(selector_row);
    }
    let selectors: [GoatTmapSelectorBank; GOAT_TMAP_SELECTORS.len()] = selector_rows
        .try_into()
        .map_err(|_| NativeWorldError::new("Goat TMAP selector count changed"))?;
    let bank = GoatTmapBank::new(selectors).map_err(NativeWorldError::new)?;
    Ok(LevelOneCacheAssets {
        sound_asset_refs: sound_refs,
        music_asset_ref: Some(music_index),
        goat_tmap_bank: Some(bank),
        castle_stage_one_footprint: Some(castle_stage_one_footprint),
        house_build_footprints,
    })
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
        assert_eq!(
            world.entities.iter().filter(|entity| entity.active).count(),
            177
        );
        assert_eq!(
            world
                .entities
                .iter()
                .filter(|entity| {
                    entity.active
                        && entity.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG
                            != 0
                })
                .count(),
            31
        );
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
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719"
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
        assert_eq!(
            world.presentation.particles[usize::from(MANA_CARRIER_RENDER_INDEX)],
            ParticleParameter {
                base_tmap: MANA_CARRIER_BASE_TMAP,
                word_2: 8_492,
                word_4: 42,
                speed: 545,
                rotation_speed: MANA_CARRIER_LOGICAL_HEIGHT,
                byte_10: 0,
                byte_11: -16,
                render_mode: MANA_CARRIER_RENDER_MODE,
                byte_13: 0,
            }
        );
        let (castle_blockers, building_blockers) = world.level1_mission.castle_placement_blockers();
        assert!(castle_blockers.is_empty());
        assert_eq!(building_blockers.len(), 47);
        let expected_buildings = world
            .entities
            .iter()
            .filter(|entity| entity.active && entity.class == 10 && entity.model == 45);
        for (blocker, entity) in building_blockers.iter().zip(expected_buildings) {
            assert_eq!(*blocker, entity.castle_placement_blocker());
        }
        let possession_targets = world.level1_mission.possession_targets();
        assert_eq!(possession_targets.len(), 41);
        assert!(possession_targets.iter().all(|target| {
            let entity = &world.entities[usize::from(target.entity_index)];
            !matches!(entity.level_gate, 17 | 51 | 52 | 53) && *target == entity.collision_target()
        }));
    }

    #[test]
    fn owner_level_one_has_a_qualified_castle_site_near_the_stage_three_route() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let (castles, buildings) = world.level1_mission.castle_placement_blockers();
        let former_site = Position {
            x: 50_048,
            y: 51_584,
            z: 1_200,
        };
        let qualified_site = Position {
            x: 49_920,
            y: 43_776,
            z: 1_200,
        };
        assert!(!crate::level1_mission::castle_placement_valid(
            &world,
            former_site,
            castles,
            buildings
        ));
        assert!(crate::level1_mission::castle_placement_valid(
            &world,
            qualified_site,
            castles,
            buildings
        ));
    }

    #[test]
    fn qualified_level_one_castle_build_reaches_every_owner_height_transactionally() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let guard_x = 20_u8;
        let guard_y = 21_u8;
        let mut unconditional = world.terrain_planes().clone();
        let mut signed = unconditional.clone();
        for (x, y, angle) in [
            (guard_x, guard_y, 0x80),
            (guard_x.wrapping_sub(1), guard_y, 0x81),
            (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1), 0x82),
            (guard_x, guard_y.wrapping_sub(1), 0x83),
        ] {
            let index = terrain_index(x, y);
            unconditional.terrain_types[index] = 42;
            unconditional.angles[index] = angle;
            signed.terrain_types[index] = 42;
            signed.angles[index] = angle;
        }
        let mut unconditional_rng = 9_775;
        let mut signed_rng = 9_775;
        add_building_to_terrain(
            &mut unconditional,
            guard_x,
            guard_y,
            MapType::Night,
            &mut unconditional_rng,
        );
        apply_level1_load_path_point(&mut signed, guard_x, guard_y, &mut signed_rng);
        for (x, y) in [
            (guard_x, guard_y),
            (guard_x.wrapping_sub(1), guard_y),
            (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1)),
            (guard_x, guard_y.wrapping_sub(1)),
        ] {
            let index = terrain_index(x, y);
            assert_ne!(
                unconditional.terrain_types[index], 42,
                "AddBuildingToTerrain must mark bit-0x80 cell ({x},{y})"
            );
            assert_eq!(
                signed.terrain_types[index], 42,
                "sub_462A0 must preserve bit-0x80 cell ({x},{y})"
            );
        }
        let footprint = world
            .level1_mission()
            .castle_stage_one_footprint()
            .expect("qualified BUILD[1]");
        let candidate = Position {
            x: 49_920,
            y: 43_776,
            z: 1_200,
        };
        let mut root = Position {
            x: candidate.x & 0xff00,
            y: candidate.y & 0xff00,
            z: world.ground_height(candidate),
        };
        if ((root.x >> 8) + (root.y >> 8)) & 1 != 0 {
            root.x = root.x.wrapping_add(256);
        }
        let start_x = (root.x >> 8) as u8 - footprint.width / 2;
        let start_y = (root.y >> 8) as u8 - footprint.height / 2;
        let mut base = u8::MAX;
        for offset in 0..footprint.width {
            for (x, y) in [
                (start_x.wrapping_add(offset), start_y),
                (
                    start_x.wrapping_add(offset),
                    start_y.wrapping_add(footprint.height),
                ),
            ] {
                base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
            }
        }
        for offset in 0..footprint.height {
            for (x, y) in [
                (
                    start_x.wrapping_add(footprint.width),
                    start_y.wrapping_add(offset),
                ),
                (start_x, start_y.wrapping_add(offset)),
            ] {
                base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
            }
        }
        let anchor = Position {
            z: i16::from(base) * 32,
            ..root
        };
        let mut terrain = SessionTerrain::from_world(&world);
        let unchanged = terrain.clone();
        let error = terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 0,
                },
                footprint,
            )
            .expect_err("a malformed construction timer must fail closed");
        assert!(error.contains("outside 1..=18"));
        assert_eq!(
            terrain, unchanged,
            "a rejected update must not mutate planes, RNG, or sequence"
        );

        let mut first_session = world.session();
        let mut second_session = world.session();
        first_session
            .terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 18,
                },
                footprint,
            )
            .expect("qualified first construction pass");
        assert_ne!(first_session.terrain, second_session.terrain);
        assert_eq!(second_session.terrain.mutation_sequence(), 0);
        first_session.reset();
        second_session.reset();
        assert_eq!(first_session.terrain, second_session.terrain);
        assert_eq!(first_session.terrain, SessionTerrain::from_world(&world));

        for remaining in (1..=18).rev() {
            terrain
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position: anchor,
                        remaining,
                    },
                    footprint,
                )
                .expect("qualified Level 1 footprint does not require an unsupported topology");
        }
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionCompleted { position: anchor },
                footprint,
            )
            .expect("qualified completion");
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                assert_ne!(cell[1], 0xff);
                assert_eq!(
                    terrain.planes().ground_heights
                        [terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row))],
                    base.wrapping_add(cell[1])
                );
            }
        }
        assert_eq!(terrain.terrain_rng(), 37_256);
        assert_eq!(terrain.mutation_sequence(), 19);
        assert_eq!(
            sha256_hex(&terrain.planes().terrain_types),
            "6cf4397c008d4ea16e37340bbe819f6d58235e61f8d669260ee9f0ac41e94e16"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().ground_heights),
            "7eff800ac2a60ccf3314bd917408c65a11c1a5cbe59b52d9670b4ed76b3c4533"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().shading),
            "ad91ad8c250c0cd4e3bd9248ae9fa7bf95cb72d056921e7f0dbff03c7fdf7ec7"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().angles),
            "79962c82610ea6abf727a11f82f5351b69355915f11aba9738e4eaca8d18484e"
        );
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::StageDestroyed {
                    position: anchor,
                    entity_rng: 100,
                },
                footprint,
            )
            .expect("qualified Level 1 teardown");
        assert_eq!(terrain.terrain_rng(), 33_028);
        assert_eq!(terrain.mutation_sequence(), 20);
        assert_eq!(
            sha256_hex(&terrain.planes().terrain_types),
            "5793a3375673e55b74dd3b62159ff6b239241035a3100d599987350b6faa24ae"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().ground_heights),
            "50f3a45214f435ac3b8423254a69b7cc87b80a8039085c25a0fe2033eed1ecba"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().shading),
            "f1a63a2c818a1e16f8e781dd8ef0777833b5194ac8f69a604723408c23e486a8"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().angles),
            "4c72d5ae2beee363ffa2b51ef0b181bcd8c8d5f05ac4a801735ec2722aad3891"
        );
        assert_eq!(
            sha256_hex(&terrain.planes().secondary_heights),
            "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31"
        );
        assert_eq!(
            terrain.planes().entity_indices,
            world.terrain_planes().entity_indices
        );

        let mut ambient = SessionTerrain::from_world(&world);
        let material_x = 200_u8;
        let material_y = 201_u8;
        for (x, y) in [
            (material_x, material_y),
            (material_x.wrapping_add(1), material_y),
            (material_x.wrapping_add(1), material_y.wrapping_add(1)),
            (material_x, material_y.wrapping_add(1)),
        ] {
            ambient.planes.ground_heights[terrain_index(x, y)] = 50;
            ambient.planes.angles[terrain_index(x, y)] = 0;
        }
        ambient
            .apply_castle_update(
                CastleTerrainUpdate::AmbientMaterial {
                    position: Position {
                        x: u16::from(material_x) << 8,
                        y: u16::from(material_y) << 8,
                        z: 1_600,
                    },
                    in_type: 2,
                    target_type: 20,
                },
                footprint,
            )
            .expect("source action-0 terrain material update");
        assert_eq!(
            ambient.planes.terrain_types[terrain_index(material_x, material_y)],
            TERRAIN_GENERATION_TABLE_PREFIX[0x18]
        );
        assert_eq!(
            ambient.planes.angles[terrain_index(material_x, material_y)],
            (TERRAIN_GENERATION_TABLE_PREFIX[0x19] | 0x80) & 0xf7
        );

        let collision_x = 210_u8;
        let collision_y = 211_u8;
        for (x, y) in [
            (collision_x, collision_y),
            (collision_x.wrapping_add(1), collision_y),
            (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
            (collision_x, collision_y.wrapping_add(1)),
        ] {
            ambient.planes.ground_heights[terrain_index(x, y)] = 100;
            ambient.planes.angles[terrain_index(x, y)] = 0;
        }
        ambient
            .apply_castle_update(
                CastleTerrainUpdate::AmbientCollision {
                    position: Position {
                        x: u16::from(collision_x) << 8,
                        y: u16::from(collision_y) << 8,
                        z: 3_200,
                    },
                    height_delta: -3,
                },
                footprint,
            )
            .expect("source action-0 SEARCH terrain deformation");
        for (x, y) in [
            (collision_x, collision_y),
            (collision_x.wrapping_add(1), collision_y),
            (collision_x, collision_y.wrapping_add(1)),
            (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
        ] {
            assert_eq!(ambient.planes.ground_heights[terrain_index(x, y)], 97);
        }
        assert_eq!(ambient.mutation_sequence(), 2);
    }

    #[test]
    fn qualified_level_one_house_footprints_drive_transactional_action_53_terrain() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let expected = [
            (17, 48, 48),
            (26, 9, 11),
            (29, 13, 11),
            (30, 13, 15),
            (31, 23, 11),
            (34, 22, 22),
            (42, 11, 11),
            (47, 11, 11),
            (50, 7, 7),
            (51, 4, 12),
            (52, 12, 4),
            (53, 8, 8),
        ];
        for (index, width, height) in expected {
            let footprint = world
                .house_build_footprint(index)
                .expect("qualified Level 1 house BUILD row");
            assert_eq!(
                (footprint.index, footprint.width, footprint.height),
                (index, width, height)
            );
            assert_eq!(
                footprint.cells().len(),
                usize::from(width) * usize::from(height)
            );
        }

        let house = &world.entities()[11];
        assert_eq!(house.level_gate, 29);
        let footprint = world
            .house_build_footprint(house.level_gate.cast_unsigned())
            .expect("house 11 BUILD[29]");
        let mut terrain = SessionTerrain::from_world(&world);
        let before = terrain.deterministic_hash();
        let outcome = terrain
            .apply_house_removal(house.position, house.entity_rng, 2, true, footprint)
            .expect("action-53 terrain mutation");
        assert_eq!(outcome.final_occupants, 0);
        assert_eq!(outcome.final_rng, 2);
        assert_eq!(
            outcome
                .residents
                .iter()
                .map(|resident| (resident.model, resident.action))
                .collect::<Vec<_>>(),
            [(4, 33), (12, 97)]
        );
        assert_eq!(terrain.mutation_sequence(), 1);
        assert_ne!(terrain.deterministic_hash(), before);

        let mut session = world.session();
        let empty_mission_before = session.mission.clone();
        let empty_live_world_before = session.live_world.clone();
        let empty_global_rng = session.mission.global_rng();
        let empty_events = session.mission.events().to_vec();
        let empty_arena_hash = session.mission.source_arena_hash_v1();
        let empty_live_hash = session.live_world_hash_v2();
        let actor_storage = session.mission.actors().as_ptr();
        let goat_storage = session.live_world.goats().as_ptr();
        assert!(
            apply_level1_source_effects(&mut session.mission, &mut session.live_world, &[],)
                .expect("empty effect batch is an exact no-op")
                .is_empty()
        );
        assert_eq!(session.mission, empty_mission_before);
        assert_eq!(session.live_world, empty_live_world_before);
        assert_eq!(session.mission.global_rng(), empty_global_rng);
        assert_eq!(session.mission.events(), empty_events);
        assert_eq!(session.mission.source_arena_hash_v1(), empty_arena_hash);
        assert_eq!(session.live_world_hash_v2(), empty_live_hash);
        assert_eq!(
            session.mission.actors().as_ptr(),
            actor_storage,
            "empty production dispatch must not clone mission storage"
        );
        assert_eq!(
            session.live_world.goats().as_ptr(),
            goat_storage,
            "empty production dispatch must not clone live-world storage"
        );

        let resident_position = session.live_world.houses()[0].position;
        let resident_slot = session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("qualified source arena has a resident slot");
        let occupants_before = session.live_world.houses()[0].occupants;
        let live_hash_before = session.live_world_hash_v2();
        let arena_hash_before = session.mission.source_arena_hash_v1();
        let updates = apply_level1_source_effects(
            &mut session.mission,
            &mut session.live_world,
            &[Level1SourceEffect::TryEnterResident {
                resident_source_entity_index: resident_slot,
                house_source_entity_index: 1,
            }],
        )
        .expect("atomic resident entry effect");
        assert_eq!(updates.len(), 1);
        assert_eq!(
            session.live_world.houses()[0].occupants,
            occupants_before + 1,
            "successful entry increments the sole house authority exactly once"
        );
        assert!(!session.mission.dynamic_residents().next().unwrap().active);
        assert_ne!(session.live_world_hash_v2(), live_hash_before);
        assert_ne!(session.mission.source_arena_hash_v1(), arena_hash_before);

        while session.live_world.houses()[0].occupants
            < i32::from(session.live_world.houses()[0].capacity)
        {
            assert_eq!(
                session.live_world.try_enter_resident(1),
                HouseResidentEntry::Entered
            );
        }
        let full_resident = session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("second resident slot");
        let full_live_hash = session.live_world_hash_v2();
        let full_arena_hash = session.mission.source_arena_hash_v1();
        assert!(
            apply_level1_source_effects(
                &mut session.mission,
                &mut session.live_world,
                &[Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index: full_resident,
                    house_source_entity_index: 1,
                }],
            )
            .expect("full resident entry rejection")
            .is_empty()
        );
        assert_eq!(session.live_world_hash_v2(), full_live_hash);
        assert_eq!(session.mission.source_arena_hash_v1(), full_arena_hash);
        assert!(
            session
                .mission
                .dynamic_residents()
                .find(|resident| resident.source_entity_index == full_resident)
                .is_some_and(|resident| resident.active)
        );

        let mut dead_session = world.session();
        let lethal = dead_session.live_world.houses()[0].life + 1;
        dead_session
            .live_world
            .inject_damage(1, lethal, 152)
            .expect("lethal house damage");
        let _ = dead_session
            .live_world
            .step_house_source_slot(1, &dead_session.terrain);
        let _ = dead_session
            .live_world
            .step_house_source_slot(1, &dead_session.terrain);
        assert!(!dead_session.live_world.houses()[0].active);
        let dead_resident = dead_session
            .mission
            .spawn_house_resident(resident_position, 4, 33, 0)
            .expect("resident constructor")
            .expect("dead-house resident slot");
        let dead_live_hash = dead_session.live_world_hash_v2();
        let dead_arena_hash = dead_session.mission.source_arena_hash_v1();
        assert!(
            apply_level1_source_effects(
                &mut dead_session.mission,
                &mut dead_session.live_world,
                &[Level1SourceEffect::TryEnterResident {
                    resident_source_entity_index: dead_resident,
                    house_source_entity_index: 1,
                }],
            )
            .expect("dead resident entry rejection")
            .is_empty()
        );
        assert_eq!(dead_session.live_world_hash_v2(), dead_live_hash);
        assert_eq!(dead_session.mission.source_arena_hash_v1(), dead_arena_hash);
    }

    #[test]
    fn house_action_53_interleaves_resident_and_height_rng_in_one_cell() {
        let mut boundary_20_rng = 3;
        assert_eq!(reduce_house_height(100, 10, &mut boundary_20_rng), 90);
        assert_eq!(
            boundary_20_rng, 37_570,
            "`rand % 0x32 == 20` consumes one draw"
        );
        let mut boundary_21_rng = 20;
        assert_eq!(reduce_house_height(100, 10, &mut boundary_21_rng), 108);
        assert_eq!(
            boundary_21_rng, 14_898,
            "`rand % 0x32 == 21` consumes the second draw"
        );
        let mut wrapping_rng = 29;
        assert_eq!(
            reduce_house_height(250, 1, &mut wrapping_rng),
            12,
            "the second branch truncates to the source byte"
        );
        assert_eq!(wrapping_rng, 20_859);
        let mut no_draw_rng = 0xabcd;
        assert_eq!(reduce_house_height(10, 10, &mut no_draw_rng), 0);
        assert_eq!(no_draw_rng, 0xabcd);

        let footprint =
            CastleBuildFootprint::from_owner_cells(99, 1, 1, &[0xff, 10]).expect("one-cell BUILD");
        let mut terrain = SessionTerrain {
            map_type: MapType::Night,
            planes: TerrainPlanes {
                terrain_types: vec![0; 65_536].into_boxed_slice(),
                ground_heights: vec![100; 65_536].into_boxed_slice(),
                shading: vec![0; 65_536].into_boxed_slice(),
                angles: vec![0; 65_536].into_boxed_slice(),
                secondary_heights: vec![0; 65_536].into_boxed_slice(),
                entity_indices: vec![0; 65_536].into_boxed_slice(),
            },
            terrain_rng: 39_663,
            mutation_sequence: 0,
        };
        let outcome = terrain
            .apply_house_removal(Position { x: 0, y: 0, z: 0 }, 2, 5, true, &footprint)
            .expect("synthetic action-53 pass");
        assert_eq!(outcome.final_occupants, 4);
        assert_eq!(outcome.final_rng, 62_591);
        assert_eq!(
            outcome.residents,
            [HouseRemovalResident {
                position: Position {
                    x: 0,
                    y: 0,
                    z: 3_200
                },
                model: 13,
                action: 105,
            }]
        );
        assert_eq!(terrain.ground_heights()[0], 100);
        assert_eq!(terrain.mutation_sequence(), 1);
    }

    #[test]
    fn direct_session_commits_house_action_53_trace_before_reclaim() {
        let (package, closure) = fixture_paths();
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load_with_owner_cache(
            package,
            closure,
            private.join("generated/level1-unity-asset-cache"),
        )
        .expect("validated package M and owner cache");
        let mut session = world.session();
        let initial_terrain_hash = session.terrain.deterministic_hash();
        session
            .live_world
            .inject_damage(11, 28_001, session.state.entity_index.cast_signed())
            .expect("fatal house damage");

        session.step(PlayerCommand::default());
        let dying = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("house 11");
        assert!(dying.active);
        assert_eq!(dying.action, crate::house::HOUSE_REMOVE_ACTION);
        assert_eq!(
            session.terrain.mutation_sequence(),
            2,
            "the first source turn applies two action-0 village terrain effects"
        );

        session.step(PlayerCommand::default());
        let removed = session
            .live_world
            .houses()
            .iter()
            .find(|house| house.entity_index == 11)
            .expect("house 11 retained for deterministic history");
        assert!(!removed.active);
        assert_eq!(removed.occupants, 0);
        assert_eq!(removed.rng, 2);
        assert_eq!(
            session.terrain.mutation_sequence(),
            3,
            "house removal follows the two source-ordered ambient mutations"
        );
        assert_ne!(session.terrain.deterministic_hash(), initial_terrain_hash);
        assert_eq!(
            session
                .pending_house_effects
                .iter()
                .filter_map(|event| match event {
                    HouseStepEvent::ResidentRequested { model, action, .. } =>
                        Some((*model, *action)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [(4, 33), (12, 97)]
        );

        let mut replay = world.session();
        replay
            .live_world
            .inject_damage(11, 28_001, replay.state.entity_index.cast_signed())
            .expect("fatal replay damage");
        replay.step(PlayerCommand::default());
        replay.step(PlayerCommand::default());
        assert_eq!(replay.live_world, session.live_world);
        assert_eq!(replay.terrain, session.terrain);
        assert_eq!(replay.mission, session.mission);
        assert_eq!(replay.pending_house_effects, session.pending_house_effects);

        let mut exhausted = world.session();
        exhausted
            .live_world
            .inject_damage(11, 28_001, exhausted.state.entity_index.cast_signed())
            .expect("fatal house damage");
        let destruction_started = exhausted
            .live_world
            .step_house_source_slot(11, &exhausted.terrain);
        assert!(matches!(
            destruction_started.as_slice(),
            [HouseStepEvent::DestructionStarted { .. }]
        ));
        while exhausted
            .mission
            .spawn_house_resident(Position::default(), 4, 33, 152)
            .unwrap()
            .is_some()
        {}
        let live_before = exhausted.live_world.clone();
        let terrain_before = exhausted.terrain.clone();
        let mission_before = exhausted.mission.clone();
        let effects_before = exhausted.pending_house_effects.clone();
        let removal = exhausted
            .live_world
            .step_house_source_slot(11, &exhausted.terrain);
        assert!(!exhausted.process_house_events(&removal));
        exhausted.live_world = live_before.clone();
        assert_eq!(exhausted.live_world, live_before);
        assert_eq!(exhausted.terrain, terrain_before);
        assert_eq!(exhausted.mission, mission_before);
        assert_eq!(exhausted.pending_house_effects, effects_before);
    }

    #[test]
    fn qualified_owner_cache_adds_descriptor_backed_audio_and_goat_tmap_states() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut refs = Vec::new();
        let cache = append_level_one_cache_assets(
            &private.join("generated/level1-unity-asset-cache"),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
            &mut refs,
        )
        .expect("qualified owner audio cache");
        let sound = cache.sound_asset_refs;
        let music = cache.music_asset_ref;
        let goat_tmaps = cache.goat_tmap_bank;
        let castle = cache
            .castle_stage_one_footprint
            .expect("qualified Castle BUILD footprint");
        assert_eq!((castle.index, castle.width, castle.height), (1, 8, 8));
        assert_eq!(castle.cells().len(), 64);
        assert_eq!(refs.len(), 33);
        assert_eq!(
            sound.keys().copied().collect::<Vec<_>>(),
            [
                1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37, 39,
                40, 41, 43, 44, 46, 54, 55, 56, 57, 63
            ]
        );
        assert_eq!(sound[&12], 8);
        assert_eq!(
            refs[8].asset_id,
            "owner_cache:audio/sound-bank-1-822.json#sound_id=12"
        );
        assert_eq!(music, Some(32));
        assert_eq!(
            refs[32].content_address,
            "sha256:03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
        );
        let goat_tmaps = goat_tmaps.expect("qualified Goat TMAP bank");
        assert_eq!(
            std::array::from_fn(|ordinal| goat_tmaps.selectors()[ordinal].selector),
            GOAT_TMAP_SELECTORS
        );
        assert_eq!(
            std::array::from_fn(|ordinal| {
                goat_tmaps.selectors()[0].states[ordinal].stream_cursor
            }),
            [490, 818, 1146, 1472, 1812]
        );
        assert_eq!(goat_tmaps.selectors()[1].states[0].pixels.len(), 32 * 22);
    }

    #[test]
    fn qualified_goat_tmap_clocks_materialize_every_exact_pixel_state_and_replay() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cache = append_level_one_cache_assets(
            &private.join("generated/level1-unity-asset-cache"),
            "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
            "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
            &mut Vec::new(),
        )
        .expect("qualified owner cache");
        let bank = cache.goat_tmap_bank.expect("qualified Goat TMAP bank");
        let expected_hashes = [
            [
                "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
                "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
                "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
                "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
                "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
            ],
            [
                "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
                "4068c077eb4f66e39ae3bd13202462b477a418aa7781822d09f0feb3337765c7",
                "d5d9ccc838119171cb7ac06315e6e6994e72c058cf40b3b9a9b6fb6709e8931f",
                "f0f656d86c51628b5f0aac90932b79b448f64ef1ef38b41fcffc9d275df71fb1",
                "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
            ],
            [
                "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
                "897331a6a567a6f64be563755ba33a5ab1e75e0a1d77d5a1fc9a67ca8657d44a",
                "22d9b6b2bad1a7fef49afcb05284c513f14200852c5cbaad097f684a0197f940",
                "752525ceb8c9a097f0676def3bdeb58258bd093ce7efacf3fe170dc7ec4cbc84",
                "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
            ],
        ];
        let expected_ordinals = [0_u8, 1, 2, 3, 4, 1];
        let mut clocks = GoatTmapClocks::from_bank(&bank);
        let reset_pixels: [Box<[u8]>; GOAT_TMAP_SELECTORS.len()] =
            std::array::from_fn(|ordinal| clocks.states()[ordinal].pixels().into());

        for (step, expected_ordinal) in expected_ordinals.into_iter().enumerate() {
            for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
                assert_eq!(clock.state_ordinal(), expected_ordinal);
                assert_eq!(
                    sha256_hex(clock.pixels()),
                    expected_hashes[selector_ordinal][usize::from(expected_ordinal)]
                );
            }
            if expected_ordinal == 4 {
                for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
                    assert_eq!(clock.pixels(), reset_pixels[selector_ordinal].as_ref());
                }
            }
            if step + 1 < expected_ordinals.len() {
                clocks
                    .latch_draw_attempts(0b111)
                    .expect("three qualified selectors");
                clocks.service_unpaused_outer_turn(&bank);
            }
        }

        let expected_replay = clocks.clone();
        clocks.reset(&bank);
        for _ in 0..5 {
            clocks
                .latch_draw_attempts(0b111)
                .expect("three qualified selectors");
            clocks.service_unpaused_outer_turn(&bank);
        }
        assert_eq!(clocks, expected_replay);
        assert_eq!(clocks.hash_v1(), expected_replay.hash_v1());
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
            114
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
                .filter(|entity| entity.authority == EntityAuthority::VillagerLive)
                .count(),
            LEVEL_ONE_VILLAGER_INDICES.len()
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
            0xe7ec_6bc1_8b5f_e775,
            "the additive complete Level 1 hand/audio session contract is independently pinned"
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
    fn player_fireball_crosses_the_shared_arena_into_goat_death_and_one_mana_sphere() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let mut session = world.session();
        let goat = session
            .live_world
            .goats()
            .iter()
            .find(|goat| goat.active)
            .expect("Level 1 has an active Goat")
            .clone();

        session.state.position = Position {
            x: goat.position.x,
            y: goat.position.y.wrapping_add(384),
            z: goat.position.z.wrapping_sub(100),
        };
        session.state.yaw = 0;
        session.state.effective_pitch = 0;
        session.mission.has_fireball = true;
        session.mission.player_mana = 1_000;
        for _ in 0..3 {
            assert!(session.mission.spawn_fireball(&session.state));
        }

        for _ in 0..24 {
            session.step(PlayerCommand::default());
            if session
                .live_world
                .goats()
                .iter()
                .find(|candidate| candidate.entity_index == goat.entity_index)
                .is_some_and(|candidate| !candidate.active)
            {
                break;
            }
        }
        let retired = session
            .live_world
            .goats()
            .iter()
            .find(|candidate| candidate.entity_index == goat.entity_index)
            .expect("typed Goat row remains addressable after retirement");
        assert!(!retired.active);
        let sphere_count = session.mission.mana_spheres().len();
        assert_eq!(sphere_count, 1);
        let sphere = session.mission.mana_spheres()[0];
        assert_eq!(sphere.mana, goat.mana);
        assert_eq!(sphere.id, u32::from(sphere.source_entity_index));
        assert_eq!(
            session.mission.source_slots()[usize::from(sphere.source_entity_index)].entity_rng,
            sphere.entity_rng
        );

        for _ in 0..8 {
            session.step(PlayerCommand::default());
        }
        assert_eq!(
            session.mission.mana_spheres().len(),
            sphere_count,
            "retiring the imported Goat must not emit duplicate mana"
        );
        assert!(
            !session
                .mission
                .imported_source_slot_is_live(goat.entity_index)
        );
    }

    #[test]
    fn dolmen_contact_selects_high_recharge_on_the_following_player_action() {
        let (package, closure) = fixture_paths();
        let world = NativeWorld::load(package, closure).expect("validated package M");
        let dolmen = world
            .entities()
            .iter()
            .find(|entity| {
                entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
            })
            .expect("Level 1 dolmen");
        let mut session = world.session();
        session.state.position = dolmen.position;
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        session.mission.player_mana = 0;
        session.mission.player_max_mana = 10_000;

        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN,
            "the dolmen status bit changes the next regen value after this tick's regen"
        );

        session.state.position.x = session.state.position.x.wrapping_add(4_096);
        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN
                + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
            "the latched high recharge value is consumed once after leaving"
        );

        session.step(PlayerCommand::default());
        assert_eq!(
            session.mission.player_mana,
            crate::level1_mission::PLAYER_OUTDOOR_MANA_REGEN * 2
                + crate::level1_mission::PLAYER_RECHARGE_MINIMUM_MANA_REGEN,
            "the following out-of-range tick returns to outdoor recharge"
        );
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
    fn carrier_presentation_extent_mutations_fail_closed() {
        let (package, _) = fixture_paths();
        let original = fs::read(package.join("presentation.bin")).unwrap();
        let row = 12 + usize::from(MANA_CARRIER_RENDER_INDEX) * PARTICLE_RECORD_BYTES;
        for (offset, bytes) in [
            (6_usize, 544_u16.to_le_bytes()),
            (8_usize, 799_u16.to_le_bytes()),
        ] {
            let mut presentation = original.clone();
            presentation[row + offset..row + offset + 2].copy_from_slice(&bytes);
            let parsed = parse_presentation(&presentation).expect("well-formed mutation");
            let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
            assert!(error.to_string().contains("169 mismatch"));
        }
        let mut presentation = original.clone();
        presentation[row + 11] = (-15_i8).cast_unsigned();
        let parsed = parse_presentation(&presentation).expect("well-formed mutation");
        let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
        assert!(error.to_string().contains("169 mismatch"));

        for index in [
            POSSESSION_ATTRACTION_RENDER_INDEX,
            POSSESSION_IMPACT_RENDER_INDEX,
            GROUND_FIRE_FIRST_RENDER_INDEX,
            GROUND_FIRE_FIRST_RENDER_INDEX + 7,
        ] {
            let mut presentation = original.clone();
            let row = 12 + usize::from(index) * PARTICLE_RECORD_BYTES;
            presentation[row] ^= 1;
            let parsed = parse_presentation(&presentation).expect("well-formed mutation");
            let error = validate_level_one_mission_presentation(&parsed).unwrap_err();
            assert!(
                error.to_string().contains(&format!("{index} mismatch")),
                "row {index} mutation reached the exact Level 1 presentation gate: {error}"
            );
        }
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
