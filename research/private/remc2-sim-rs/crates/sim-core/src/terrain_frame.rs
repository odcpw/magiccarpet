//! Exact isolated Level 1 outdoor terrain frame.
//!
//! This pipeline initializes the original 320×200 indexed sky, forms and
//! projects the 40×21 outdoor lattice, submits the complete reflected pass
//! before the complete primary pass, and executes mode 5 or mode 26 into the
//! indexed framebuffer. It deliberately does not claim a complete original
//! world frame: MC2 interleaves sprites and particles with terrain, while this
//! module returns their terrain-only coverage separately.

use crate::{
    SIN_16_16,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    combat_tmap::{CombatTmapBank, FireballTmapClock, PossessionTmapClock},
    economy_tmap::{CastleTmapClock, EconomyTmapBank},
    firefly_tmap::{FireflyTmapBank, FireflyTmapClocks},
    frozen_actor_tmap::{
        DOLMEN_LOGICAL_HEIGHT, DOLMEN_RENDER_MODE, DOLMEN_RENDER_PARAMETER_INDEX, DOLMEN_SELECTOR,
        FrozenActorTmapBank, VillagerTmapClocks, villager_selector_mask,
    },
    goat::{GOAT_RENDER_PARAMETER_INDEX, GoatState},
    goat_draw_admission::outdoor_billboard_tile,
    goat_sprite_raster::{
        Mode20SpriteCamera, Mode20SpriteDefinition, Mode20SpriteEntity, Mode20SpriteRasterStats,
        mode17_selector_for_base, mode19_selector_for_base, mode20_selector,
        mode20_selector_for_base, project_fixed_sprite_with_definition,
        project_mode17_sprite_with_definition, project_mode19_sprite_with_definition,
        project_mode20_sprite, project_mode20_sprite_with_definition, project_sprite_shadow,
        rasterize_mode20_sprite, rasterize_possession_sprite,
        rasterize_reflected_possession_sprite, rasterize_reflected_sprite, rasterize_shadow_sprite,
        reproject_sprite_altitude,
    },
    goat_tmap::GoatTmapClocks,
    legacy_sqrt,
    legacy_terrain::interpolate_ground,
    level1_mission::{
        ARCHER_ARROW_BASE_TMAP, ARCHER_ARROW_LOGICAL_HEIGHT, ARCHER_ARROW_RENDER_INDEX,
        ARCHER_ATTACK_BASE_TMAP_A, ARCHER_ATTACK_BASE_TMAP_B, ARCHER_ATTACK_LOGICAL_HEIGHT_B,
        ARCHER_ATTACK_RENDER_INDEX_A, ARCHER_ATTACK_RENDER_INDEX_B, ARCHER_BASE_TMAP,
        ARCHER_LOGICAL_HEIGHT, ARCHER_RENDER_INDEX, ARCHER_RENDER_MODE,
        CASTLE_PROJECTILE_BASE_TMAP, CASTLE_PROJECTILE_LOGICAL_HEIGHT,
        CASTLE_PROJECTILE_RENDER_INDEX, CASTLE_ROOT_BASE_TMAP, CASTLE_ROOT_LOGICAL_HEIGHT,
        CASTLE_ROOT_RENDER_INDEX, CHECKPOINT_SMOKE_BASE_TMAP, CHECKPOINT_SMOKE_FIRST_RENDER_INDEX,
        CHECKPOINT_SMOKE_MODEL, DynamicResidentKind, DynamicResidentState, EXIT_TARGET_BASE_TMAP,
        EXIT_TARGET_LOGICAL_HEIGHT, EXIT_TARGET_RENDER_INDEX, EXIT_TARGET_RENDER_MODE,
        EXPERIENCE_SCROLL_BASE_TMAP, EXPERIENCE_SCROLL_LOGICAL_HEIGHT,
        EXPERIENCE_SCROLL_RENDER_INDEX, EXPERIENCE_SCROLL_RENDER_MODE, FIREBALL_BASE_TMAP,
        FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_LOGICAL_HEIGHT, FIREBALL_IMPACT_RENDER_INDEX,
        FIREBALL_LOGICAL_HEIGHT, FIREBALL_RENDER_INDEX, FIREFLY_BASE_TMAP, FIREFLY_LOGICAL_HEIGHT,
        FIREFLY_RENDER_INDEX, FIREFLY_RENDER_MODE, FireballImpact, GROUND_FIRE_BASE_TMAP,
        GROUND_FIRE_MODEL, Level1Actor, Level1ActorKind, Level1AmbientEntity,
        Level1MultipartSegment, Level1PossessedBuilding, Level1Projectile, MANA_CARRIER_BASE_TMAP,
        MANA_CARRIER_LOGICAL_HEIGHT, MANA_CARRIER_RENDER_INDEX, MANA_SPHERE_NEUTRAL_BASE_TMAP,
        MANA_SPHERE_NEUTRAL_RENDER_BASE, MANA_SPHERE_PLAYER_ONE_BASE_TMAP,
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE, MULTIPART_HEAD_BASE_TMAP,
        MULTIPART_HEAD_LOGICAL_HEIGHT, MULTIPART_HEAD_RENDER_INDEX, MULTIPART_HEAD_RENDER_MODE,
        MULTIPART_SEGMENT_BASE_TMAP, MULTIPART_SEGMENT_RENDER_MODE, ManaCarrier, ManaSphere,
        POSSESSION_IMPACT_BASE_TMAP, POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_MODEL,
        POSSESSION_IMPACT_RENDER_INDEX, POSSESSION_PROJECTILE_BASE_TMAP,
        POSSESSION_PROJECTILE_LOGICAL_HEIGHT, POSSESSION_PROJECTILE_RENDER_INDEX,
        POSSESSION_STRONG_IMPACT_MODEL, SPELL_JAR_BASE_TMAP, SPELL_JAR_LOGICAL_HEIGHT,
        SPELL_JAR_RENDER_INDEX, SPELL_JAR_RENDER_MODE, STONE_BASE_TMAP, STONE_LOGICAL_HEIGHT,
        STONE_RENDER_INDEX, STONE_RENDER_MODE, TREE_BASE_TMAP_A, TREE_BASE_TMAP_B,
        TREE_DEAD_BASE_TMAP_A, TREE_DEAD_BASE_TMAP_B, TREE_DEAD_RENDER_INDEX_A,
        TREE_DEAD_RENDER_INDEX_B, TREE_LOGICAL_HEIGHT, TREE_RENDER_INDEX_A, TREE_RENDER_INDEX_B,
        TREE_RENDER_MODE, WATER_SPLASH_BASE_TMAP, WATER_SPLASH_LOGICAL_HEIGHT,
        WATER_SPLASH_RENDER_INDEX, ground_fire_presentation, impact_is_presentational,
        mana_sphere_size_ordinal,
    },
    mode5_raster::{Mode5Vertex, rasterize_mode5_triangle_from_atlas},
    mode26_raster::{NIGHT_TABLES_BYTES, rasterize_mode26_triangle_from_atlas_into_framebuffer},
    native_world::ParticleParameter,
    renderable_map_chain::RenderableMapChains,
    source_sprite_tmap::{
        SourceSpriteDisposition, SourceSpriteSubmission, SourceSpriteWorldResources,
    },
    static_world_tmap::StaticWorldTmapBank,
    terrain_grid::{OutdoorGridInput, OutdoorGridVertex, outdoor_grid},
    terrain_reflection::{
        MODE5_TERRAIN_SELECTOR, MODE26_REFLECTIVE_TERRAIN_SELECTOR, OutdoorProjection,
        OutdoorReflectionVertex, TERRAIN_FEATURE_REFLECTIVE_MATERIAL, TerrainPass,
        TerrainTriangleSubmission, project_outdoor_reflection_vertex, reflection_texture_cell,
        submit_outdoor_grid, terrain_material_blocks_shadow, terrain_row_anchor_order,
    },
};

pub const LEVEL1_VIEW_WIDTH: usize = 320;
pub const LEVEL1_VIEW_HEIGHT: usize = 200;
pub const LEVEL1_VIEW_PIXELS: usize = LEVEL1_VIEW_WIDTH * LEVEL1_VIEW_HEIGHT;
pub const LEVEL1_TERRAIN_ATLAS_WIDTH: usize = 256;
pub const LEVEL1_TERRAIN_TILE_SIZE: usize = 32;
pub const LEVEL1_TERRAIN_TILE_COLUMNS: usize = 8;
pub const LEVEL1_TERRAIN_TILE_COUNT: usize = 152;
pub const LEVEL1_TERRAIN_ATLAS_BYTES: usize =
    LEVEL1_TERRAIN_TILE_COUNT * LEVEL1_TERRAIN_TILE_SIZE * LEVEL1_TERRAIN_TILE_SIZE;
pub const LEVEL1_SKY_BYTES: usize = 256 * 256;

const UV_EDGE: i32 = 0x001f_ffff;
const BASE_UV_ROWS: [[[i32; 2]; 4]; 8] = [
    [[0, 0], [UV_EDGE, 0], [UV_EDGE, UV_EDGE], [0, UV_EDGE]],
    [[UV_EDGE, 0], [UV_EDGE, UV_EDGE], [0, UV_EDGE], [0, 0]],
    [[UV_EDGE, UV_EDGE], [0, UV_EDGE], [0, 0], [UV_EDGE, 0]],
    [[0, UV_EDGE], [0, 0], [UV_EDGE, 0], [UV_EDGE, UV_EDGE]],
    [[UV_EDGE, 0], [0, 0], [0, UV_EDGE], [UV_EDGE, UV_EDGE]],
    [[0, 0], [0, UV_EDGE], [UV_EDGE, UV_EDGE], [UV_EDGE, 0]],
    [[0, UV_EDGE], [UV_EDGE, UV_EDGE], [UV_EDGE, 0], [0, 0]],
    [[UV_EDGE, UV_EDGE], [UV_EDGE, 0], [0, 0], [0, UV_EDGE]],
];
const UV_ROW_ORDER: [usize; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 6, 7, 4, 5, 2, 3, 0, 1, 5, 6, 7, 4, 3, 0, 1, 2, 1, 2, 3, 0, 7, 4, 5, 6,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1TerrainFrameCamera {
    pub render_x: u16,
    pub render_y: u16,
    pub eye_z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub source_turn: i32,
    pub reflections: bool,
}

pub struct Level1TerrainFrameResources<'a> {
    pub terrain_types: &'a [u8],
    pub ground_heights: &'a [u8],
    pub shading: &'a [u8],
    pub angles: &'a [u8],
    /// Original 256-stride BL32N atlas, 8 tiles per row.
    pub terrain_atlas: &'a [u8],
    pub tablesn: &'a [u8],
    pub sky: &'a [u8],
}

pub struct Level1WorldFrameResources<'a> {
    pub local_player_entity_index: u16,
    pub goats: &'a [GoatState],
    pub frozen_actors: &'a [Level1FrozenActorRender],
    pub ambient_entities: &'a [Level1AmbientEntity],
    pub frozen_actor_tmap_bank: Option<&'a FrozenActorTmapBank>,
    pub villager_tmap_clocks: Option<&'a VillagerTmapClocks>,
    pub mission_actors: &'a [Level1Actor],
    pub dynamic_residents: &'a [DynamicResidentState],
    pub mission_segments: &'a [Level1MultipartSegment],
    pub castle_root: Option<Level1CastleRootRender>,
    pub mana_spheres: &'a [ManaSphere],
    pub mana_carrier: Option<&'a ManaCarrier>,
    pub map_chains: &'a RenderableMapChains,
    pub goat_tmap_clocks: &'a GoatTmapClocks,
    pub archer_tmap_bank: &'a ArcherTmapBank,
    pub archer_tmap_clocks: &'a ArcherTmapClocks,
    pub firefly_tmap_bank: &'a FireflyTmapBank,
    pub firefly_tmap_clocks: &'a FireflyTmapClocks,
    pub projectiles: &'a [Level1Projectile],
    pub impacts: &'a [FireballImpact],
    pub possessed_buildings: &'a [Level1PossessedBuilding],
    pub combat_tmap_bank: &'a CombatTmapBank,
    pub static_world_tmap_bank: &'a StaticWorldTmapBank,
    pub economy_tmap_bank: &'a EconomyTmapBank,
    pub fireball_tmap_clock: &'a FireballTmapClock,
    pub possession_tmap_clock: &'a PossessionTmapClock,
    pub castle_tmap_clock: &'a CastleTmapClock,
    /// Exact 347-row visual parameter table used by byte-10 shadow gates.
    pub particle_parameters: &'a [ParticleParameter],
    /// Complete source-arena flags used by the common visibility/shadow gates.
    pub source_slots: &'a [crate::level1_mission::Level1ResetSourceSlot],
    pub night: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1FrozenActorRender {
    pub source_entity_index: u16,
    pub position: crate::Position,
    pub yaw: i16,
    pub render_parameter_index: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1CastleRootRender {
    pub source_entity_index: u16,
    pub position: crate::Position,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerrainFrameStats {
    pub reflected_triangles: u32,
    pub primary_mode5_triangles: u32,
    pub primary_mode26_triangles: u32,
    pub terrain_pixel_writes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level1TerrainFrame {
    pub width: u16,
    pub height: u16,
    /// Complete indexed sky plus isolated terrain result.
    pub indices: Vec<u8>,
    /// 255 where this isolated terrain pipeline wrote at least once.
    pub terrain_coverage: Vec<u8>,
    pub index_hash: u64,
    pub coverage_hash: u64,
    pub stats: TerrainFrameStats,
    /// Aggregate source-indexed Goat raster accounting. Zero for the stable
    /// isolated terrain-only entry point.
    pub goat_raster_stats: Mode20SpriteRasterStats,
    /// Selector-global draw-attempt mask for 152..154.
    pub goat_selector_mask: u8,
    /// True only when this frame evaluated and composited the supported Goat
    /// family in source terrain-cell order.
    pub goats_composited: bool,
    /// Aggregate reset-frozen villager/dolmen raster accounting.
    pub frozen_actor_raster_stats: Mode20SpriteRasterStats,
    /// Selector-global draw-attempt mask for villager selectors
    /// 155..157, 180..185, and 199..201.
    pub frozen_actor_selector_mask: u16,
    /// True only when the exact 31 reset-frozen Level 1 actors were evaluated
    /// in the shared source map-cell chains.
    pub frozen_actors_composited: bool,
    /// Aggregate source-indexed village fire/smoke raster accounting.
    pub ambient_raster_stats: Mode20SpriteRasterStats,
    /// True when the live ambient projection participated in map-chain draw.
    pub ambient_entities_composited: bool,
    /// Aggregate source-indexed Archer raster accounting.
    pub archer_raster_stats: Mode20SpriteRasterStats,
    /// Selector-global draw-attempt mask for animated selectors 8..10.
    pub archer_selector_mask: u8,
    /// True only when every live Archer was evaluated in the shared source
    /// map-cell chain with its exact qualified mode-20 resources.
    pub archers_composited: bool,
    /// Aggregate source-indexed Firefly raster accounting.
    pub firefly_raster_stats: Mode20SpriteRasterStats,
    /// Selector-global draw-attempt mask for animated selectors 279..286.
    pub firefly_selector_mask: u8,
    /// True only when every live Firefly was evaluated in the shared source
    /// map-cell chain with its exact qualified mode-17 resources.
    pub fireflies_composited: bool,
    /// Aggregate source-indexed projectile/effect raster accounting.
    pub combat_raster_stats: Mode20SpriteRasterStats,
    /// True when selector 463 reached source draw-attempt latching.
    pub fireball_draw_attempted: bool,
    /// True when selector 144 reached source draw-attempt latching.
    pub possession_draw_attempted: bool,
    /// True when the supported Fireball/Possession/Arrow/Castle/Impact/Splash families
    /// were evaluated in the shared source map chains.
    pub combat_composited: bool,
    /// Aggregate source-indexed fixed objective-prop raster accounting.
    pub objective_prop_raster_stats: Mode20SpriteRasterStats,
    /// True when the exact jar/exit fixed-selector slice was evaluated in the
    /// shared source map chains.
    pub objective_props_composited: bool,
    /// Aggregate source-indexed castle/mana-economy raster accounting.
    pub economy_raster_stats: Mode20SpriteRasterStats,
    /// True when the exact Level 1 castle root, local-color mana spheres, and
    /// mana carrier were evaluated in the shared source map chains.
    pub economy_composited: bool,
    /// True when castle selector 96 reached source draw-attempt latching.
    pub castle_draw_attempted: bool,
    /// Aggregate `sub_3FD60` reflected-sprite raster accounting.
    pub reflected_sprite_raster_stats: Mode20SpriteRasterStats,
    /// Aggregate `DrawSprite_41BD3(0)` terrain-shadow raster accounting.
    pub shadow_sprite_raster_stats: Mode20SpriteRasterStats,
    pub reflected_sprites_composited: bool,
    pub sprite_shadows_composited: bool,
    /// Present only when the cache-backed typed source compositor completed
    /// its actual sky, reflected, and primary execution sequence.
    source_execution_order_hash: Option<u64>,
}

pub const SOURCE_RENDER_REQUIRED_FAMILY_MASK: u16 = 0x01ff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceRenderExecutionReceipt {
    pub family_mask: u16,
    pub order_hash: u64,
    pub index_hash: u64,
    pub coverage_hash: u64,
    pub terrain_pixel_writes: u64,
    pub sky_seeded: bool,
    pub has_coverage: bool,
    pub reflected_sprites_composited: bool,
}

impl SourceRenderExecutionReceipt {
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.family_mask == SOURCE_RENDER_REQUIRED_FAMILY_MASK
            && self.order_hash == source_render_order_hash()
            && self.sky_seeded
            && self.has_coverage
    }
}

impl Level1TerrainFrame {
    pub fn source_execution_receipt(
        &self,
    ) -> Result<SourceRenderExecutionReceipt, TerrainFrameError> {
        if self.indices.len() != LEVEL1_VIEW_PIXELS
            || self.terrain_coverage.len() != LEVEL1_VIEW_PIXELS
            || byte_hash(&self.indices) != self.index_hash
            || byte_hash(&self.terrain_coverage) != self.coverage_hash
        {
            return Err(TerrainFrameError(
                "source renderer output/hash extent mismatch".to_owned(),
            ));
        }
        let family_mask = u16::from(self.goats_composited)
            | u16::from(self.frozen_actors_composited) << 1
            | u16::from(self.ambient_entities_composited) << 2
            | u16::from(self.archers_composited) << 3
            | u16::from(self.fireflies_composited) << 4
            | u16::from(self.combat_composited) << 5
            | u16::from(self.objective_props_composited) << 6
            | u16::from(self.economy_composited) << 7
            | u16::from(self.sprite_shadows_composited) << 8;
        let order_hash = self.source_execution_order_hash.ok_or_else(|| {
            TerrainFrameError(
                "source-render receipt requested from a frame without typed execution".to_owned(),
            )
        })?;
        let receipt = SourceRenderExecutionReceipt {
            family_mask,
            order_hash,
            index_hash: self.index_hash,
            coverage_hash: self.coverage_hash,
            terrain_pixel_writes: self.stats.terrain_pixel_writes,
            sky_seeded: true,
            has_coverage: true,
            reflected_sprites_composited: self.reflected_sprites_composited,
        };
        if !receipt.is_complete() {
            return Err(TerrainFrameError(format!(
                "source renderer receipt family mask {family_mask:#06x} does not close {SOURCE_RENDER_REQUIRED_FAMILY_MASK:#06x}"
            )));
        }
        Ok(receipt)
    }
}

#[must_use]
pub fn source_render_order_hash() -> u64 {
    byte_hash(b"mc2-source-render-execution-v2\0\x01\x02\x03")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainFrameError(pub String);

impl std::fmt::Display for TerrainFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TerrainFrameError {}

/// Render one exact isolated Level 1 terrain frame.
///
/// The only intentional omission is non-terrain framebuffer writers
/// (particles, sprites, carpet, HUD and effects). The returned coverage makes
/// that boundary explicit to clients.
pub fn render_level1_terrain_frame(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    render_level1_frame(camera, resources, None, None, false)
}

/// Render the bounded Level 1 world frame with source-ordered mode-20 Goats.
///
/// Other sprites, particles, carpet, HUD, and effects remain explicitly
/// omitted.
pub fn render_level1_world_frame(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
    world: Level1WorldFrameResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    render_level1_frame(camera, resources, Some(world), None, false)
}

/// Render the same source-ordered world compositor while requiring a typed
/// CP1 disposition for every linked source row. This is the production
/// fail-closed entry used by the complete presentation path.
pub fn render_level1_source_world_frame(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
    world: Level1WorldFrameResources<'_>,
    source_sprites: SourceSpriteWorldResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    render_level1_frame(camera, resources, Some(world), Some(source_sprites), false)
}

/// Receipt for a source-ordered world-sprite pass rendered into a caller-owned
/// profile-sized indexed plane.
///
/// The terrain and sky are intentionally outside this helper. Classic HD
/// supplies those from its resolution-aware terrain graph, then uses this
/// adapter for the complete source compositor so legacy families retain their
/// existing clocks, banks, painter order, reflection gates, and shadow rules.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Level1SourceWorldSpriteRasterReceipt {
    pub primary: Mode20SpriteRasterStats,
    pub reflected: Mode20SpriteRasterStats,
    pub shadows: Mode20SpriteRasterStats,
    pub reflection_pass_evaluated: bool,
    pub shadow_pass_evaluated: bool,
}

/// Render the complete source-ordered Level 1 world-sprite compositor into a
/// profile-sized indexed plane.
///
/// This is deliberately additive to `render_level1_source_world_frame`: the
/// latter remains the sealed 320x200 Exact path. The caller supplies a
/// profile-sized projection and matching output plane; the source world grid
/// still owns chain traversal and painter order, while the existing sprite
/// kernels write at the supplied projection extent.
pub fn raster_level1_source_world_sprites(
    camera: Level1TerrainFrameCamera,
    projection: OutdoorProjection,
    resources: Level1TerrainFrameResources<'_>,
    world: Level1WorldFrameResources<'_>,
    source_sprites: SourceSpriteWorldResources<'_>,
    indices: &mut [u8],
) -> Result<Level1SourceWorldSpriteRasterReceipt, TerrainFrameError> {
    let width = usize::try_from(projection.viewport_width).map_err(|_| {
        TerrainFrameError("profile-sized source sprite viewport width is negative".to_owned())
    })?;
    let height = usize::try_from(projection.viewport_height).map_err(|_| {
        TerrainFrameError("profile-sized source sprite viewport height is negative".to_owned())
    })?;
    let expected = width.checked_mul(height).ok_or_else(|| {
        TerrainFrameError("profile-sized source sprite extent overflow".to_owned())
    })?;
    if width == 0 || height == 0 || indices.len() != expected {
        return Err(TerrainFrameError(format!(
            "profile-sized source sprite plane requires {width}x{height}={expected} bytes, got {}",
            indices.len()
        )));
    }
    let expected_plane = 256 * 256;
    for (name, length) in [
        ("terrain_types", resources.terrain_types.len()),
        ("ground_heights", resources.ground_heights.len()),
        ("shading", resources.shading.len()),
        ("angles", resources.angles.len()),
    ] {
        if length != expected_plane {
            return Err(TerrainFrameError(format!(
                "profile-sized source sprite {name} plane requires {expected_plane} bytes, got {length}"
            )));
        }
    }

    let lattice = outdoor_grid(OutdoorGridInput {
        render_x: camera.render_x,
        render_y: camera.render_y,
        yaw: camera.yaw,
        turn: camera.source_turn,
        shading: resources.shading,
        angles: resources.angles,
    });
    let mut goat_raster_stats = Mode20SpriteRasterStats::default();
    let mut goat_selector_mask = 0_u8;
    let mut frozen_actor_raster_stats = Mode20SpriteRasterStats::default();
    let mut frozen_actor_selector_mask = 0_u16;
    let mut ambient_raster_stats = Mode20SpriteRasterStats::default();
    let mut archer_raster_stats = Mode20SpriteRasterStats::default();
    let mut archer_selector_mask = 0_u8;
    let mut firefly_raster_stats = Mode20SpriteRasterStats::default();
    let mut firefly_selector_mask = 0_u8;
    let mut combat_raster_stats = Mode20SpriteRasterStats::default();
    let mut objective_prop_raster_stats = Mode20SpriteRasterStats::default();
    let mut economy_raster_stats = Mode20SpriteRasterStats::default();
    let mut reflected_sprite_raster_stats = Mode20SpriteRasterStats::default();
    let mut shadow_sprite_raster_stats = Mode20SpriteRasterStats::default();
    let mut fireball_draw_attempted = false;
    let mut possession_draw_attempted = false;
    let mut castle_draw_attempted = false;

    if camera.reflections && camera.eye_z < crate::terrain_reflection::REFLECTION_EYE_Z_LIMIT {
        for owner_index in primary_owner_schedule(&lattice) {
            raster_world_chain(
                WorldChainPass::Reflection,
                owner_index,
                camera,
                projection,
                &lattice,
                &resources,
                &world,
                Some(&source_sprites),
                indices,
                &mut goat_raster_stats,
                &mut goat_selector_mask,
                &mut frozen_actor_raster_stats,
                &mut frozen_actor_selector_mask,
                &mut ambient_raster_stats,
                &mut archer_raster_stats,
                &mut archer_selector_mask,
                &mut firefly_raster_stats,
                &mut firefly_selector_mask,
                &mut combat_raster_stats,
                &mut objective_prop_raster_stats,
                &mut economy_raster_stats,
                &mut reflected_sprite_raster_stats,
                &mut shadow_sprite_raster_stats,
                &mut fireball_draw_attempted,
                &mut possession_draw_attempted,
                &mut castle_draw_attempted,
            )?;
        }
    }

    let mut primary_raster_stats = Mode20SpriteRasterStats::default();
    for owner_index in primary_owner_schedule(&lattice) {
        let before = [
            goat_raster_stats,
            frozen_actor_raster_stats,
            ambient_raster_stats,
            archer_raster_stats,
            firefly_raster_stats,
            combat_raster_stats,
            objective_prop_raster_stats,
            economy_raster_stats,
        ];
        raster_world_chain(
            WorldChainPass::PrimaryWithShadow,
            owner_index,
            camera,
            projection,
            &lattice,
            &resources,
            &world,
            Some(&source_sprites),
            indices,
            &mut goat_raster_stats,
            &mut goat_selector_mask,
            &mut frozen_actor_raster_stats,
            &mut frozen_actor_selector_mask,
            &mut ambient_raster_stats,
            &mut archer_raster_stats,
            &mut archer_selector_mask,
            &mut firefly_raster_stats,
            &mut firefly_selector_mask,
            &mut combat_raster_stats,
            &mut objective_prop_raster_stats,
            &mut economy_raster_stats,
            &mut reflected_sprite_raster_stats,
            &mut shadow_sprite_raster_stats,
            &mut fireball_draw_attempted,
            &mut possession_draw_attempted,
            &mut castle_draw_attempted,
        )?;
        let after = [
            goat_raster_stats,
            frozen_actor_raster_stats,
            ambient_raster_stats,
            archer_raster_stats,
            firefly_raster_stats,
            combat_raster_stats,
            objective_prop_raster_stats,
            economy_raster_stats,
        ];
        for (before, after) in before.into_iter().zip(after) {
            primary_raster_stats.candidate_pixels = primary_raster_stats
                .candidate_pixels
                .wrapping_add(after.candidate_pixels.wrapping_sub(before.candidate_pixels));
            primary_raster_stats.transparent_pixels =
                primary_raster_stats.transparent_pixels.wrapping_add(
                    after
                        .transparent_pixels
                        .wrapping_sub(before.transparent_pixels),
                );
            primary_raster_stats.clipped_pixels = primary_raster_stats
                .clipped_pixels
                .wrapping_add(after.clipped_pixels.wrapping_sub(before.clipped_pixels));
            primary_raster_stats.framebuffer_writes =
                primary_raster_stats.framebuffer_writes.wrapping_add(
                    after
                        .framebuffer_writes
                        .wrapping_sub(before.framebuffer_writes),
                );
        }
    }

    Ok(Level1SourceWorldSpriteRasterReceipt {
        primary: primary_raster_stats,
        reflected: reflected_sprite_raster_stats,
        shadows: shadow_sprite_raster_stats,
        reflection_pass_evaluated: camera.reflections,
        shadow_pass_evaluated: true,
    })
}

/// Render the map branch's real second `DrawWorld_411A0` call.  The fixed
/// profile viewport is source `(384, 0, 256, 400)`, therefore VGA1 pixels
/// occupy columns 192..319 with a 128 by 200 projection.  The returned frame
/// keeps the normal 320 by 200 backing so the overlay writer can copy that
/// exact viewport after the map clear/terrain/entity passes.
pub fn render_level1_source_world_map_frame(
    mut camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
    world: Level1WorldFrameResources<'_>,
    source_sprites: SourceSpriteWorldResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    // The map branch calls DrawWorld with the live player Z plus 128 before
    // installing the fixed right-hand viewport.
    camera.eye_z = camera.eye_z.wrapping_add(128);
    render_level1_source_world_map_frame_composed(camera, resources, world, source_sprites)
}

/// Render the map branch from the already-composed camera consumed by
/// `DrawWorld_411A0`.
///
/// This entry keeps the map Z bias inside the immutable presentation input
/// projection rather than applying a hidden renderer-side transformation.
pub fn render_level1_source_world_map_frame_composed(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
    world: Level1WorldFrameResources<'_>,
    source_sprites: SourceSpriteWorldResources<'_>,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    render_level1_frame(camera, resources, Some(world), Some(source_sprites), true)
}

fn render_level1_frame(
    camera: Level1TerrainFrameCamera,
    resources: Level1TerrainFrameResources<'_>,
    world: Option<Level1WorldFrameResources<'_>>,
    source_sprites: Option<SourceSpriteWorldResources<'_>>,
    map_right_viewport: bool,
) -> Result<Level1TerrainFrame, TerrainFrameError> {
    validate_resources(&resources)?;
    let projection = if map_right_viewport {
        level1_map_projection(camera)
    } else {
        level1_projection(camera)
    };
    let lattice = outdoor_grid(OutdoorGridInput {
        render_x: camera.render_x,
        render_y: camera.render_y,
        yaw: camera.yaw,
        turn: camera.source_turn,
        shading: resources.shading,
        angles: resources.angles,
    });
    let projected: [Option<OutdoorReflectionVertex>; 840] = std::array::from_fn(|index| {
        let source = lattice[index];
        let map_index = usize::from(source.tile_x) + usize::from(source.tile_y) * 256;
        let (texture_index, texture_uv_row) = reflection_texture_cell(
            source.tile_x,
            source.tile_y,
            camera.yaw,
            resources.terrain_types,
            resources.angles,
        );
        let mut projected = project_outdoor_reflection_vertex(
            source,
            resources.ground_heights[map_index],
            camera.eye_z,
            texture_index,
            texture_uv_row,
            false,
            projection,
        );
        if !camera.reflections
            && let Some(vertex) = projected.as_mut()
        {
            vertex.feature_flags &= !TERRAIN_FEATURE_REFLECTIVE_MATERIAL;
        }
        projected
    });
    let eye_for_submission = if camera.reflections {
        camera.eye_z
    } else {
        i32::MAX
    };
    let plan = submit_outdoor_grid(eye_for_submission, &lattice, &projected);
    let mut indices = if map_right_viewport {
        draw_level1_map_sky(camera, resources.sky)?
    } else {
        draw_level1_sky(camera, resources.sky)?
    };
    let mut source_execution = source_sprites
        .as_ref()
        .map(|_| b"mc2-source-render-execution-v2\0".to_vec());
    if let Some(execution) = source_execution.as_mut() {
        execution.push(1);
    }
    let mut terrain_coverage = vec![0; LEVEL1_VIEW_PIXELS];
    let mut stats = TerrainFrameStats::default();
    let mut goat_raster_stats = Mode20SpriteRasterStats::default();
    let mut goat_selector_mask = 0_u8;
    let mut frozen_actor_raster_stats = Mode20SpriteRasterStats::default();
    let mut frozen_actor_selector_mask = 0_u16;
    let mut ambient_raster_stats = Mode20SpriteRasterStats::default();
    let mut archer_raster_stats = Mode20SpriteRasterStats::default();
    let mut archer_selector_mask = 0_u8;
    let mut firefly_raster_stats = Mode20SpriteRasterStats::default();
    let mut firefly_selector_mask = 0_u8;
    let mut combat_raster_stats = Mode20SpriteRasterStats::default();
    let mut objective_prop_raster_stats = Mode20SpriteRasterStats::default();
    let mut economy_raster_stats = Mode20SpriteRasterStats::default();
    let mut fireball_draw_attempted = false;
    let mut possession_draw_attempted = false;
    let mut castle_draw_attempted = false;
    let mut reflected_sprite_raster_stats = Mode20SpriteRasterStats::default();
    let mut shadow_sprite_raster_stats = Mode20SpriteRasterStats::default();

    if let Some(world) = world.as_ref()
        && camera.reflections
        && camera.eye_z < crate::terrain_reflection::REFLECTION_EYE_Z_LIMIT
    {
        let mut reflected_index = 0;
        for owner_index in primary_owner_schedule(&lattice) {
            while let Some(submission) = plan.reflected.get(reflected_index) {
                if submission.owner_index != owner_index {
                    break;
                }
                raster_submission(
                    *submission,
                    &lattice,
                    &projected,
                    &resources,
                    &mut indices,
                    &mut terrain_coverage,
                    &mut stats,
                )?;
                reflected_index += 1;
            }
            raster_world_chain(
                WorldChainPass::Reflection,
                owner_index,
                camera,
                projection,
                &lattice,
                &resources,
                world,
                source_sprites.as_ref(),
                &mut indices,
                &mut goat_raster_stats,
                &mut goat_selector_mask,
                &mut frozen_actor_raster_stats,
                &mut frozen_actor_selector_mask,
                &mut ambient_raster_stats,
                &mut archer_raster_stats,
                &mut archer_selector_mask,
                &mut firefly_raster_stats,
                &mut firefly_selector_mask,
                &mut combat_raster_stats,
                &mut objective_prop_raster_stats,
                &mut economy_raster_stats,
                &mut reflected_sprite_raster_stats,
                &mut shadow_sprite_raster_stats,
                &mut fireball_draw_attempted,
                &mut possession_draw_attempted,
                &mut castle_draw_attempted,
            )?;
        }
        if reflected_index != plan.reflected.len() {
            return Err(TerrainFrameError(format!(
                "reflected terrain schedule consumed {reflected_index} of {} submissions",
                plan.reflected.len()
            )));
        }
    } else {
        for submission in &plan.reflected {
            raster_submission(
                *submission,
                &lattice,
                &projected,
                &resources,
                &mut indices,
                &mut terrain_coverage,
                &mut stats,
            )?;
        }
    }
    if let Some(execution) = source_execution.as_mut() {
        execution.push(2);
    }
    if let Some(world) = world.as_ref() {
        let mut primary_index = 0;
        for owner_index in primary_owner_schedule(&lattice) {
            while let Some(submission) = plan.primary.get(primary_index) {
                if submission.owner_index != owner_index {
                    break;
                }
                raster_submission(
                    *submission,
                    &lattice,
                    &projected,
                    &resources,
                    &mut indices,
                    &mut terrain_coverage,
                    &mut stats,
                )?;
                primary_index += 1;
            }
            raster_world_chain(
                WorldChainPass::PrimaryWithShadow,
                owner_index,
                camera,
                projection,
                &lattice,
                &resources,
                world,
                source_sprites.as_ref(),
                &mut indices,
                &mut goat_raster_stats,
                &mut goat_selector_mask,
                &mut frozen_actor_raster_stats,
                &mut frozen_actor_selector_mask,
                &mut ambient_raster_stats,
                &mut archer_raster_stats,
                &mut archer_selector_mask,
                &mut firefly_raster_stats,
                &mut firefly_selector_mask,
                &mut combat_raster_stats,
                &mut objective_prop_raster_stats,
                &mut economy_raster_stats,
                &mut reflected_sprite_raster_stats,
                &mut shadow_sprite_raster_stats,
                &mut fireball_draw_attempted,
                &mut possession_draw_attempted,
                &mut castle_draw_attempted,
            )?;
        }
        if primary_index != plan.primary.len() {
            return Err(TerrainFrameError(format!(
                "primary terrain schedule consumed {primary_index} of {} submissions",
                plan.primary.len()
            )));
        }
    } else {
        for submission in &plan.primary {
            raster_submission(
                *submission,
                &lattice,
                &projected,
                &resources,
                &mut indices,
                &mut terrain_coverage,
                &mut stats,
            )?;
        }
    }
    if let Some(execution) = source_execution.as_mut() {
        execution.push(3);
    }
    let source_execution_order_hash = source_execution.as_deref().map(byte_hash);

    Ok(Level1TerrainFrame {
        width: LEVEL1_VIEW_WIDTH as u16,
        height: LEVEL1_VIEW_HEIGHT as u16,
        index_hash: byte_hash(&indices),
        coverage_hash: byte_hash(&terrain_coverage),
        indices,
        terrain_coverage,
        stats,
        goat_raster_stats,
        goat_selector_mask,
        goats_composited: world.is_some(),
        frozen_actor_raster_stats,
        frozen_actor_selector_mask,
        frozen_actors_composited: world.is_some(),
        ambient_raster_stats,
        ambient_entities_composited: world.is_some(),
        archer_raster_stats,
        archer_selector_mask,
        archers_composited: world.is_some(),
        firefly_raster_stats,
        firefly_selector_mask,
        fireflies_composited: world.is_some(),
        combat_raster_stats,
        fireball_draw_attempted,
        possession_draw_attempted,
        combat_composited: world.is_some(),
        objective_prop_raster_stats,
        objective_props_composited: world.is_some(),
        economy_raster_stats,
        economy_composited: world.is_some(),
        castle_draw_attempted,
        reflected_sprite_raster_stats,
        shadow_sprite_raster_stats,
        reflected_sprites_composited: world.is_some()
            && camera.reflections
            && camera.eye_z < crate::terrain_reflection::REFLECTION_EYE_Z_LIMIT,
        sprite_shadows_composited: world.is_some(),
        source_execution_order_hash,
    })
}

fn primary_owner_schedule(lattice: &[OutdoorGridVertex; 840]) -> Vec<usize> {
    let mut schedule = Vec::with_capacity(20 * 39);
    for row in (1..=20).rev() {
        let row_start = row * 40;
        let row_flags = std::array::from_fn(|column| lattice[row_start + column].feature_flags);
        schedule.extend(terrain_row_anchor_order(row_start, &row_flags));
    }
    schedule
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorldChainPass {
    Reflection,
    PrimaryWithShadow,
}

fn reflection_chain_entity_admitted(entity_index: u16) -> bool {
    // sub_3FD60 only reflects entries from the original 1,000-row entity
    // arena. Higher sentinel/extension indices are skipped before any entity
    // record is read.
    entity_index < 1_000
}

fn shadow_surface_allows(
    owner_tile_x: u8,
    owner_tile_y: u8,
    camera_yaw: i16,
    terrain_types: &[u8],
    angles: &[u8],
) -> bool {
    let (shadow_texture, _) = reflection_texture_cell(
        owner_tile_x,
        owner_tile_y,
        camera_yaw,
        terrain_types,
        angles,
    );
    !terrain_material_blocks_shadow(shadow_texture)
}

const fn impact_presentation_discriminants(model: u8) -> (bool, bool) {
    let possession = matches!(
        model,
        POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
    );
    let fireball = !possession
        && model != crate::level1_mission::WATER_SPLASH_MODEL
        && model != CHECKPOINT_SMOKE_MODEL;
    (possession, fireball)
}

#[allow(clippy::too_many_arguments)]
fn raster_world_chain(
    pass: WorldChainPass,
    owner_index: usize,
    camera: Level1TerrainFrameCamera,
    projection: OutdoorProjection,
    lattice: &[OutdoorGridVertex; 840],
    resources: &Level1TerrainFrameResources<'_>,
    world_resources: &Level1WorldFrameResources<'_>,
    source_sprites: Option<&SourceSpriteWorldResources<'_>>,
    indices: &mut [u8],
    goat_aggregate: &mut Mode20SpriteRasterStats,
    goat_selector_mask: &mut u8,
    frozen_actor_aggregate: &mut Mode20SpriteRasterStats,
    frozen_actor_selector_mask: &mut u16,
    ambient_aggregate: &mut Mode20SpriteRasterStats,
    archer_aggregate: &mut Mode20SpriteRasterStats,
    archer_selector_mask: &mut u8,
    firefly_aggregate: &mut Mode20SpriteRasterStats,
    firefly_selector_mask: &mut u8,
    combat_aggregate: &mut Mode20SpriteRasterStats,
    objective_prop_aggregate: &mut Mode20SpriteRasterStats,
    economy_aggregate: &mut Mode20SpriteRasterStats,
    reflected_aggregate: &mut Mode20SpriteRasterStats,
    shadow_aggregate: &mut Mode20SpriteRasterStats,
    fireball_draw_attempted: &mut bool,
    possession_draw_attempted: &mut bool,
    castle_draw_attempted: &mut bool,
) -> Result<(), TerrainFrameError> {
    let Some((tile_x, tile_y)) = outdoor_billboard_tile(lattice, owner_index, camera.yaw) else {
        return Ok(());
    };
    // remc2 latches `textAtyp_43` from the terrain texture lookup cell
    // before advancing to the distinct entity-head cell.  Reusing
    // `(tile_x, tile_y)` here would therefore classify the wrong material in
    // three of the four camera quadrants.
    let owner = lattice[owner_index];
    let shadow_surface = shadow_surface_allows(
        owner.tile_x,
        owner.tile_y,
        camera.yaw,
        resources.terrain_types,
        resources.angles,
    );
    for entity_index in world_resources.map_chains.entity_chain(tile_x, tile_y) {
        if pass == WorldChainPass::Reflection && !reflection_chain_entity_admitted(entity_index) {
            continue;
        }
        let source_slot = world_resources
            .source_slots
            .get(usize::from(entity_index))
            .ok_or_else(|| {
                TerrainFrameError(format!(
                    "renderable map-chain entity {entity_index} has no source-arena row"
                ))
            })?;
        if let Some(source_sprites) = source_sprites {
            let disposition =
                source_sprites
                    .dispositions
                    .iter()
                    .find(|disposition| match disposition {
                        SourceSpriteDisposition::Render(submission) => {
                            submission.source_entity_index == entity_index
                        }
                        SourceSpriteDisposition::Legacy {
                            source_entity_index,
                            ..
                        }
                        | SourceSpriteDisposition::Excluded {
                            source_entity_index,
                            ..
                        } => *source_entity_index == entity_index,
                    });
            match disposition {
                Some(SourceSpriteDisposition::Excluded { .. }) => continue,
                Some(SourceSpriteDisposition::Render(submission)) => {
                    raster_source_sprite_submission(
                        pass,
                        *submission,
                        camera,
                        projection,
                        resources,
                        world_resources,
                        source_sprites,
                        indices,
                        shadow_surface,
                        combat_aggregate,
                        reflected_aggregate,
                        shadow_aggregate,
                    )?;
                    continue;
                }
                Some(SourceSpriteDisposition::Legacy { .. }) => {}
                None => {
                    return Err(TerrainFrameError(format!(
                        "unknown Level 1 source sprite tuple: slot={entity_index} class={} model={} action={}",
                        source_slot.class, source_slot.model, source_slot.action
                    )));
                }
            }
        }
        let control_flags = source_slot.control_flags;
        if source_sprites.is_none() && control_flags & 0x21 != 0 {
            continue;
        }
        if let Some(ambient) = world_resources
            .ambient_entities
            .iter()
            .find(|ambient| ambient.source_entity_index == entity_index)
        {
            if !ambient.visible() {
                continue;
            }
            let parameter = world_resources
                .particle_parameters
                .get(usize::from(ambient.render_parameter_index))
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "ambient entity {entity_index} parameter {} is absent",
                        ambient.render_parameter_index
                    ))
                })?;
            let expected = match (ambient.model, ambient.action) {
                (0, 0) => (7, 38, 36),
                (1, 1) => (41, 54, 21),
                (14, 14) => (9, 63, 21),
                _ => {
                    return Err(TerrainFrameError(format!(
                        "ambient entity {entity_index} has unknown source tuple class={} model={} action={}",
                        source_slot.class, source_slot.model, source_slot.action
                    )));
                }
            };
            if (
                ambient.render_parameter_index,
                parameter.base_tmap,
                parameter.render_mode,
            ) != expected
            {
                return Err(TerrainFrameError(format!(
                    "ambient entity {entity_index} render tuple changed: {}/{}/{}",
                    ambient.render_parameter_index, parameter.base_tmap, parameter.render_mode
                )));
            }
            let frame = world_resources
                .static_world_tmap_bank
                .frame(parameter.base_tmap)
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "ambient selector {} has no qualified frame",
                        parameter.base_tmap
                    ))
                })?;
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: parameter.base_tmap,
                    logical_height: i32::from(parameter.rotation_speed),
                },
                None,
                false,
                sprite_camera,
                Mode20SpriteEntity {
                    x: ambient.position.x,
                    y: ambient.position.y,
                    z: ambient.position.z,
                    yaw: ambient.yaw,
                },
                frame.width,
                frame.height,
            );
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                ambient.position,
                ambient.render_parameter_index,
                interpolate_ground(
                    resources.ground_heights,
                    ambient.position.x,
                    ambient.position.y,
                ),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                ambient_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }
        if let Some(building) = world_resources
            .possessed_buildings
            .iter()
            .find(|building| building.entity_index == entity_index && building.active)
        {
            let frame = world_resources
                .combat_tmap_bank
                .static_frame(building.base_tmap)
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "possessed-building selector {} has no qualified frame",
                        building.base_tmap
                    ))
                })?;
            let parameter = world_resources
                .particle_parameters
                .get(usize::from(building.render_parameter_index))
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "possessed-building parameter {} is absent",
                        building.render_parameter_index
                    ))
                })?;
            if parameter.base_tmap != building.base_tmap
                || parameter.render_mode != building.render_mode
                || parameter.rotation_speed != building.logical_height
            {
                return Err(TerrainFrameError(format!(
                    "possessed-building render tuple changed: {parameter:?}"
                )));
            }
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: building.base_tmap,
                    logical_height: i32::from(building.logical_height),
                },
                None,
                false,
                sprite_camera,
                Mode20SpriteEntity {
                    x: building.position.x,
                    y: building.position.y,
                    z: building.position.z,
                    yaw: 0,
                },
                frame.width,
                frame.height,
            );
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                building.position,
                building.render_parameter_index,
                interpolate_ground(
                    resources.ground_heights,
                    building.position.x,
                    building.position.y,
                ),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                combat_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }
        if let Some(goat) = world_resources
            .goats
            .iter()
            .find(|goat| goat.entity_index == entity_index)
        {
            if !goat.active || goat.render_parameter_index != GOAT_RENDER_PARAMETER_INDEX {
                continue;
            }
            let (_, selector, _) = mode20_selector(goat.yaw, camera.yaw);
            let Some(clock) = world_resources
                .goat_tmap_clocks
                .states()
                .iter()
                .find(|clock| clock.selector() == selector)
            else {
                return Err(TerrainFrameError(format!(
                    "Goat selector {selector} has no live TMAP clock"
                )));
            };
            let placement = project_mode20_sprite(
                Mode20SpriteCamera {
                    render_x: camera.render_x,
                    render_y: camera.render_y,
                    eye_z: camera.eye_z,
                    yaw: camera.yaw,
                    projection,
                },
                Mode20SpriteEntity {
                    x: goat.position.x,
                    y: goat.position.y,
                    z: goat.position.z,
                    yaw: goat.yaw,
                },
                clock.width(),
                clock.height(),
            );
            if placement.is_ok()
                || matches!(
                    placement,
                    Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                )
            {
                // remc2 latches the selected TMAP after depth/cutoff
                // admission, before projected-size and viewport rejection.
                *goat_selector_mask |= 1 << (selector - 152);
            }
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                Mode20SpriteCamera {
                    render_x: camera.render_x,
                    render_y: camera.render_y,
                    eye_z: camera.eye_z,
                    yaw: camera.yaw,
                    projection,
                },
                goat.position,
                GOAT_RENDER_PARAMETER_INDEX,
                interpolate_ground(resources.ground_heights, goat.position.x, goat.position.y),
                clock.width(),
                clock.height(),
                clock.pixels(),
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                goat_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        if let Some(resident) = world_resources
            .dynamic_residents
            .iter()
            .find(|resident| resident.source_entity_index == entity_index)
        {
            if !resident.active || !resident.visible || resident.life < 0 {
                continue;
            }
            match resident.kind {
                DynamicResidentKind::Archer => {
                    let exact_pose = matches!(
                        (
                            resident.render_parameter_index,
                            resident.base_tmap,
                            resident.render_mode,
                            resident.logical_height,
                        ),
                        (
                            ARCHER_RENDER_INDEX,
                            ARCHER_BASE_TMAP,
                            ARCHER_RENDER_MODE,
                            ARCHER_LOGICAL_HEIGHT
                        ) | (
                            ARCHER_ATTACK_RENDER_INDEX_A,
                            ARCHER_ATTACK_BASE_TMAP_A,
                            ARCHER_RENDER_MODE,
                            ARCHER_LOGICAL_HEIGHT
                        ) | (
                            ARCHER_ATTACK_RENDER_INDEX_B,
                            ARCHER_ATTACK_BASE_TMAP_B,
                            ARCHER_RENDER_MODE,
                            ARCHER_ATTACK_LOGICAL_HEIGHT_B
                        )
                    );
                    if !exact_pose {
                        return Err(TerrainFrameError(format!(
                            "house Archer entity {entity_index} has unsupported render tuple {}/{}/{}/{}",
                            resident.render_parameter_index,
                            resident.base_tmap,
                            resident.render_mode,
                            resident.logical_height,
                        )));
                    }
                    let (_, selector, _) =
                        mode20_selector_for_base(resident.base_tmap, resident.yaw, camera.yaw);
                    let (width, height, pixels) = if let Some(clock) =
                        world_resources.archer_tmap_clocks.animated_frame(selector)
                    {
                        (clock.width(), clock.height(), clock.pixels())
                    } else if let Some(frame) =
                        world_resources.archer_tmap_bank.static_frame(selector)
                    {
                        (frame.width, frame.height, frame.pixels.as_ref())
                    } else {
                        return Err(TerrainFrameError(format!(
                            "house Archer selector {selector} has no qualified TMAP frame"
                        )));
                    };
                    let sprite_camera = Mode20SpriteCamera {
                        render_x: camera.render_x,
                        render_y: camera.render_y,
                        eye_z: camera.eye_z,
                        yaw: camera.yaw,
                        projection,
                    };
                    let placement = project_mode20_sprite_with_definition(
                        Mode20SpriteDefinition {
                            base_selector: resident.base_tmap,
                            logical_height: i32::from(resident.logical_height),
                        },
                        sprite_camera,
                        Mode20SpriteEntity {
                            x: resident.position.x,
                            y: resident.position.y,
                            z: resident.position.z,
                            yaw: resident.yaw,
                        },
                        width,
                        height,
                    );
                    if placement.is_ok()
                        || matches!(
                            placement,
                            Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                        )
                    {
                        if selector >= ARCHER_BASE_TMAP && selector <= ARCHER_BASE_TMAP + 2 {
                            *archer_selector_mask |= 1 << (selector - ARCHER_BASE_TMAP);
                        }
                    }
                    let Ok(placement) = placement else { continue };
                    raster_world_sprite_pass(
                        pass,
                        indices,
                        placement,
                        sprite_camera,
                        resident.position,
                        resident.render_parameter_index,
                        interpolate_ground(
                            resources.ground_heights,
                            resident.position.x,
                            resident.position.y,
                        ),
                        width,
                        height,
                        pixels,
                        resources.tablesn,
                        camera.roll,
                        control_flags,
                        shadow_surface,
                        world_resources.night,
                        false,
                        world_resources.particle_parameters,
                        archer_aggregate,
                        reflected_aggregate,
                        shadow_aggregate,
                    )
                    .map_err(TerrainFrameError)?;
                }
                DynamicResidentKind::Villager => {
                    let bank = world_resources.frozen_actor_tmap_bank.ok_or_else(|| {
                        TerrainFrameError(
                            "house villagers have no qualified owner TMAP bank".to_owned(),
                        )
                    })?;
                    let parameter = world_resources
                        .particle_parameters
                        .get(usize::from(resident.render_parameter_index))
                        .ok_or_else(|| {
                            TerrainFrameError(format!(
                                "house villager {entity_index} render parameter {} is absent",
                                resident.render_parameter_index
                            ))
                        })?;
                    if parameter.base_tmap != resident.base_tmap
                        || parameter.render_mode != resident.render_mode
                        || parameter.rotation_speed != resident.logical_height
                    {
                        return Err(TerrainFrameError(format!(
                            "house villager entity {entity_index} render tuple changed: {parameter:?}"
                        )));
                    }
                    let (_, selector, _) =
                        mode20_selector_for_base(resident.base_tmap, resident.yaw, camera.yaw);
                    let clocks = world_resources.villager_tmap_clocks.ok_or_else(|| {
                        TerrainFrameError("house villagers have no live selector clocks".to_owned())
                    })?;
                    let clock = clocks.frame(selector).ok_or_else(|| {
                        TerrainFrameError(format!(
                            "house villager entity {entity_index} selector {selector} has no qualified clock"
                        ))
                    })?;
                    // The qualified villager bank owns every selector used by
                    // the four source render rows. Keep the lookup explicit so
                    // a cache regression cannot silently fall through to a
                    // generic billboard.
                    if !bank
                        .villagers()
                        .iter()
                        .any(|selector_bank| selector_bank.selector == selector)
                    {
                        return Err(TerrainFrameError(format!(
                            "house villager selector {selector} is absent from the qualified bank"
                        )));
                    }
                    let sprite_camera = Mode20SpriteCamera {
                        render_x: camera.render_x,
                        render_y: camera.render_y,
                        eye_z: camera.eye_z,
                        yaw: camera.yaw,
                        projection,
                    };
                    let placement = project_mode20_sprite_with_definition(
                        Mode20SpriteDefinition {
                            base_selector: resident.base_tmap,
                            logical_height: i32::from(resident.logical_height),
                        },
                        sprite_camera,
                        Mode20SpriteEntity {
                            x: resident.position.x,
                            y: resident.position.y,
                            z: resident.position.z,
                            yaw: resident.yaw,
                        },
                        clock.width(),
                        clock.height(),
                    );
                    if placement.is_ok()
                        || matches!(
                            placement,
                            Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                        )
                    {
                        *frozen_actor_selector_mask |=
                            villager_selector_mask(selector).ok_or_else(|| {
                                TerrainFrameError(format!(
                                    "house villager selector {selector} is outside the qualified Level 1 set"
                                ))
                            })?;
                    }
                    let Ok(placement) = placement else { continue };
                    raster_world_sprite_pass(
                        pass,
                        indices,
                        placement,
                        sprite_camera,
                        resident.position,
                        resident.render_parameter_index,
                        interpolate_ground(
                            resources.ground_heights,
                            resident.position.x,
                            resident.position.y,
                        ),
                        clock.width(),
                        clock.height(),
                        clock.pixels(),
                        resources.tablesn,
                        camera.roll,
                        control_flags,
                        shadow_surface,
                        world_resources.night,
                        false,
                        world_resources.particle_parameters,
                        frozen_actor_aggregate,
                        reflected_aggregate,
                        shadow_aggregate,
                    )
                    .map_err(TerrainFrameError)?;
                }
                DynamicResidentKind::Builder | DynamicResidentKind::Trader => {
                    // Their exact constructor/state rows are live, collision
                    // capable and source ordered, but owner TMAP selectors
                    // 138..143 are not in the current qualified cache. Keep
                    // them absent instead of substituting a wrong sprite.
                }
            }
            continue;
        }

        if let Some(actor) = world_resources
            .frozen_actors
            .iter()
            .find(|actor| actor.source_entity_index == entity_index)
            .filter(|actor| {
                if actor.render_parameter_index == DOLMEN_RENDER_PARAMETER_INDEX {
                    source_slot.active
                        && source_slot.class == 2
                        && source_slot.model == 2
                        && source_slot.action == 6
                } else {
                    source_slot.active && source_slot.class == 5 && source_slot.model == 13
                }
            })
        {
            let bank = world_resources.frozen_actor_tmap_bank.ok_or_else(|| {
                TerrainFrameError(
                    "frozen Level 1 actors have no qualified owner TMAP bank".to_owned(),
                )
            })?;
            let parameter = world_resources
                .particle_parameters
                .get(usize::from(actor.render_parameter_index))
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "frozen actor {entity_index} render parameter {} is absent",
                        actor.render_parameter_index
                    ))
                })?;
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let sprite_entity = Mode20SpriteEntity {
                x: actor.position.x,
                y: actor.position.y,
                z: actor.position.z,
                yaw: actor.yaw,
            };
            let (placement, width, height, pixels) =
                if actor.render_parameter_index == DOLMEN_RENDER_PARAMETER_INDEX {
                    if (
                        parameter.base_tmap,
                        parameter.rotation_speed,
                        parameter.render_mode,
                    ) != (DOLMEN_SELECTOR, DOLMEN_LOGICAL_HEIGHT, DOLMEN_RENDER_MODE)
                    {
                        return Err(TerrainFrameError(format!(
                            "dolmen entity {entity_index} render tuple changed: {parameter:?}"
                        )));
                    }
                    let frame = bank.dolmen();
                    (
                        project_fixed_sprite_with_definition(
                            Mode20SpriteDefinition {
                                base_selector: DOLMEN_SELECTOR,
                                logical_height: i32::from(DOLMEN_LOGICAL_HEIGHT),
                            },
                            None,
                            false,
                            sprite_camera,
                            sprite_entity,
                            frame.width,
                            frame.height,
                        ),
                        frame.width,
                        frame.height,
                        frame.pixels.as_ref(),
                    )
                } else {
                    if parameter.render_mode != 20 {
                        return Err(TerrainFrameError(format!(
                            "villager entity {entity_index} render mode changed: {parameter:?}"
                        )));
                    }
                    let (_, selector, _) =
                        mode20_selector_for_base(parameter.base_tmap, actor.yaw, camera.yaw);
                    let clocks = world_resources.villager_tmap_clocks.ok_or_else(|| {
                        TerrainFrameError(
                            "frozen Level 1 villagers have no live selector clocks".to_owned(),
                        )
                    })?;
                    let clock = clocks.frame(selector).ok_or_else(|| {
                    TerrainFrameError(format!(
                        "villager entity {entity_index} selector {selector} has no qualified clock"
                    ))
                })?;
                    let placement = project_mode20_sprite_with_definition(
                        Mode20SpriteDefinition {
                            base_selector: parameter.base_tmap,
                            logical_height: i32::from(parameter.rotation_speed),
                        },
                        sprite_camera,
                        sprite_entity,
                        clock.width(),
                        clock.height(),
                    );
                    if placement.is_ok()
                        || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ) {
                        *frozen_actor_selector_mask |=
                        villager_selector_mask(selector).ok_or_else(|| {
                            TerrainFrameError(format!(
                                "villager selector {selector} is outside the qualified Level 1 set"
                            ))
                        })?;
                    }
                    (placement, clock.width(), clock.height(), clock.pixels())
                };
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                actor.position,
                actor.render_parameter_index,
                interpolate_ground(resources.ground_heights, actor.position.x, actor.position.y),
                width,
                height,
                pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                frozen_actor_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        if let Some(projectile) = world_resources
            .projectiles
            .iter()
            .find(|projectile| projectile.source_entity_index() == entity_index)
        {
            if !projectile.active() {
                continue;
            }
            let (
                position,
                yaw,
                render_index,
                base,
                logical_height,
                frame,
                quarter_anchor,
                fireball,
                possession,
            ) = match projectile {
                Level1Projectile::PlayerFireball(value) => (
                    value.position,
                    value.yaw,
                    FIREBALL_RENDER_INDEX,
                    FIREBALL_BASE_TMAP,
                    FIREBALL_LOGICAL_HEIGHT,
                    None,
                    true,
                    true,
                    false,
                ),
                Level1Projectile::FireflyFireball(value) => (
                    value.position,
                    value.yaw,
                    FIREBALL_RENDER_INDEX,
                    FIREBALL_BASE_TMAP,
                    FIREBALL_LOGICAL_HEIGHT,
                    None,
                    true,
                    true,
                    false,
                ),
                Level1Projectile::ArcherArrow(value) => (
                    value.position,
                    value.yaw,
                    ARCHER_ARROW_RENDER_INDEX,
                    ARCHER_ARROW_BASE_TMAP,
                    ARCHER_ARROW_LOGICAL_HEIGHT,
                    None,
                    false,
                    false,
                    false,
                ),
                Level1Projectile::CastleBuild(value) => (
                    value.position,
                    value.yaw,
                    CASTLE_PROJECTILE_RENDER_INDEX,
                    CASTLE_PROJECTILE_BASE_TMAP,
                    CASTLE_PROJECTILE_LOGICAL_HEIGHT,
                    None,
                    true,
                    false,
                    false,
                ),
                Level1Projectile::Possession(value) => (
                    value.position,
                    value.yaw,
                    POSSESSION_PROJECTILE_RENDER_INDEX,
                    POSSESSION_PROJECTILE_BASE_TMAP,
                    POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
                    None,
                    true,
                    false,
                    true,
                ),
            };
            let selector = if fireball {
                FIREBALL_BASE_TMAP
            } else if base == ARCHER_ARROW_BASE_TMAP {
                mode19_selector_for_base(base, yaw, camera.yaw).1
            } else {
                base
            };
            let (width, height, pixels) = if fireball {
                let value = world_resources.fireball_tmap_clock.frame();
                (value.width, value.height, value.pixels.as_ref())
            } else if possession {
                let value = world_resources.possession_tmap_clock.frame();
                (value.width, value.height, value.pixels.as_ref())
            } else {
                let value = world_resources
                    .combat_tmap_bank
                    .static_frame(selector)
                    .ok_or_else(|| {
                        TerrainFrameError(format!(
                            "combat projectile selector {selector} has no qualified frame"
                        ))
                    })?;
                (value.width, value.height, value.pixels.as_ref())
            };
            let definition = Mode20SpriteDefinition {
                base_selector: base,
                logical_height: i32::from(logical_height),
            };
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let sprite_entity = Mode20SpriteEntity {
                x: position.x,
                y: position.y,
                z: position.z,
                yaw,
            };
            let placement = if base == ARCHER_ARROW_BASE_TMAP {
                project_mode19_sprite_with_definition(
                    definition,
                    sprite_camera,
                    sprite_entity,
                    width,
                    height,
                )
            } else {
                project_fixed_sprite_with_definition(
                    definition,
                    frame,
                    quarter_anchor,
                    sprite_camera,
                    sprite_entity,
                    width,
                    height,
                )
            };
            if fireball
                && (placement.is_ok()
                    || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ))
            {
                *fireball_draw_attempted = true;
            }
            if possession
                && (placement.is_ok()
                    || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ))
            {
                *possession_draw_attempted = true;
            }
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                position,
                render_index,
                interpolate_ground(resources.ground_heights, position.x, position.y),
                width,
                height,
                pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                possession,
                world_resources.particle_parameters,
                combat_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        if let Some(impact) = world_resources
            .impacts
            .iter()
            .find(|impact| impact.source_entity_index == entity_index)
        {
            if !impact.active {
                continue;
            }
            if !impact_is_presentational(impact.model) {
                continue;
            }
            let splash = impact.model == crate::level1_mission::WATER_SPLASH_MODEL;
            let ground_fire = impact.model == GROUND_FIRE_MODEL;
            let checkpoint_smoke = impact.model == CHECKPOINT_SMOKE_MODEL;
            let (possession, fireball) = impact_presentation_discriminants(impact.model);
            let (render_index, base, logical_height, quarter_anchor, animate) = if splash {
                (
                    WATER_SPLASH_RENDER_INDEX,
                    WATER_SPLASH_BASE_TMAP,
                    WATER_SPLASH_LOGICAL_HEIGHT,
                    false,
                    true,
                )
            } else if ground_fire {
                let (render_index, logical_height) = ground_fire_presentation(impact.phase);
                (
                    render_index,
                    GROUND_FIRE_BASE_TMAP,
                    logical_height,
                    false,
                    false,
                )
            } else if possession {
                // Parameter 41 is render mode 21: GameRenderOriginal selects
                // fixed base TMAP 54 while enabling its special primary
                // quarter-sum pivot. The internal animation counter advances,
                // but only render modes 22..36 add it to the selector.
                (
                    POSSESSION_IMPACT_RENDER_INDEX,
                    POSSESSION_IMPACT_BASE_TMAP,
                    POSSESSION_IMPACT_LOGICAL_HEIGHT,
                    true,
                    false,
                )
            } else if checkpoint_smoke {
                let render_index = u16::from(impact.animation_frame);
                (
                    render_index,
                    CHECKPOINT_SMOKE_BASE_TMAP,
                    render_index
                        .saturating_sub(CHECKPOINT_SMOKE_FIRST_RENDER_INDEX)
                        .saturating_add(1)
                        * 50,
                    true,
                    false,
                )
            } else {
                debug_assert!(fireball);
                (
                    FIREBALL_IMPACT_RENDER_INDEX,
                    FIREBALL_IMPACT_BASE_TMAP,
                    FIREBALL_IMPACT_LOGICAL_HEIGHT,
                    true,
                    true,
                )
            };
            let selector = if animate {
                base + u16::from(impact.animation_frame)
            } else {
                base
            };
            let (value_width, value_height, value_pixels) = if checkpoint_smoke {
                let value = world_resources
                    .static_world_tmap_bank
                    .frame(selector)
                    .ok_or_else(|| {
                        TerrainFrameError(format!(
                            "checkpoint smoke selector {selector} has no qualified frame"
                        ))
                    })?;
                (value.width, value.height, value.pixels.as_ref())
            } else {
                let value = world_resources
                    .combat_tmap_bank
                    .static_frame(selector)
                    .ok_or_else(|| {
                        TerrainFrameError(format!(
                            "combat impact selector {selector} has no qualified frame"
                        ))
                    })?;
                (value.width, value.height, value.pixels.as_ref())
            };
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: base,
                    logical_height: i32::from(logical_height),
                },
                animate.then_some(impact.animation_frame),
                quarter_anchor,
                sprite_camera,
                Mode20SpriteEntity {
                    x: impact.position.x,
                    y: impact.position.y,
                    z: impact.position.z,
                    yaw: 0,
                },
                value_width,
                value_height,
            );
            if fireball
                && (placement.is_ok()
                    || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ))
            {
                *fireball_draw_attempted = true;
            }
            if possession
                && (placement.is_ok()
                    || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ))
            {
                *possession_draw_attempted = true;
            }
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                impact.position,
                render_index,
                interpolate_ground(
                    resources.ground_heights,
                    impact.position.x,
                    impact.position.y,
                ),
                value_width,
                value_height,
                value_pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                combat_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        let economy_sprite = if world_resources
            .castle_root
            .is_some_and(|castle| castle.source_entity_index == entity_index)
        {
            let castle = world_resources.castle_root.expect("matched castle root");
            if !castle.active {
                continue;
            }
            Some((
                castle.position,
                0_i16,
                CASTLE_ROOT_RENDER_INDEX,
                CASTLE_ROOT_BASE_TMAP,
                CASTLE_ROOT_LOGICAL_HEIGHT,
                false,
                true,
            ))
        } else if let Some(sphere) = world_resources
            .mana_spheres
            .iter()
            .find(|sphere| sphere.source_entity_index == entity_index)
        {
            if !sphere.active || sphere.pending_delete {
                continue;
            }
            let selector = if sphere.owner_entity_index == 0 {
                MANA_SPHERE_NEUTRAL_BASE_TMAP
            } else if sphere.owner_entity_index == world_resources.local_player_entity_index {
                MANA_SPHERE_PLAYER_ONE_BASE_TMAP
            } else {
                return Err(TerrainFrameError(format!(
                    "mana sphere entity {entity_index} owner {} has no qualified Level 1 color selector",
                    sphere.owner_entity_index
                )));
            };
            let size_ordinal = mana_sphere_size_ordinal(sphere.mana);
            let render_index = if sphere.owner_entity_index == 0 {
                MANA_SPHERE_NEUTRAL_RENDER_BASE + u16::from(size_ordinal)
            } else {
                MANA_SPHERE_PLAYER_ONE_RENDER_BASE + u16::from(size_ordinal)
            };
            Some((
                sphere.position,
                sphere.yaw,
                render_index,
                selector,
                u16::from(size_ordinal + 1) * 50,
                true,
                false,
            ))
        } else if world_resources
            .mana_carrier
            .is_some_and(|carrier| carrier.source_entity_index == entity_index)
        {
            let carrier = world_resources.mana_carrier.expect("matched mana carrier");
            if !carrier.active {
                continue;
            }
            if carrier.owner_entity_index != world_resources.local_player_entity_index {
                return Err(TerrainFrameError(format!(
                    "mana carrier entity {entity_index} owner {} has no qualified Level 1 color selector",
                    carrier.owner_entity_index
                )));
            }
            Some((
                carrier.position,
                carrier.yaw,
                MANA_CARRIER_RENDER_INDEX,
                MANA_CARRIER_BASE_TMAP,
                MANA_CARRIER_LOGICAL_HEIGHT,
                false,
                false,
            ))
        } else {
            None
        };
        if let Some((
            position,
            yaw,
            render_index,
            selector,
            logical_height,
            quarter_anchor,
            castle,
        )) = economy_sprite
        {
            let frame = if castle {
                world_resources.castle_tmap_clock.frame()
            } else {
                world_resources
                    .economy_tmap_bank
                    .frame(selector)
                    .ok_or_else(|| {
                        TerrainFrameError(format!(
                            "economy selector {selector} has no qualified frame"
                        ))
                    })?
            };
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: selector,
                    logical_height: i32::from(logical_height),
                },
                None,
                quarter_anchor,
                sprite_camera,
                Mode20SpriteEntity {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                    yaw,
                },
                frame.width,
                frame.height,
            );
            if castle
                && (placement.is_ok()
                    || matches!(
                        placement,
                        Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                    ))
            {
                *castle_draw_attempted = true;
            }
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                position,
                render_index,
                interpolate_ground(resources.ground_heights, position.x, position.y),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                economy_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        if let Some(segment) = world_resources
            .mission_segments
            .iter()
            .find(|segment| segment.source_entity_index == entity_index)
        {
            if !segment.active || !segment.visible || segment.life < 0 {
                continue;
            }
            if segment.base_tmap != MULTIPART_SEGMENT_BASE_TMAP
                || segment.render_mode != MULTIPART_SEGMENT_RENDER_MODE
            {
                return Err(TerrainFrameError(format!(
                    "multipart segment {entity_index} has unsupported render tuple {}/{}/{}",
                    segment.render_parameter_index, segment.base_tmap, segment.render_mode
                )));
            }
            let frame = world_resources
                .static_world_tmap_bank
                .frame(MULTIPART_SEGMENT_BASE_TMAP)
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "multipart segment selector {MULTIPART_SEGMENT_BASE_TMAP} has no qualified frame"
                    ))
                })?;
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: MULTIPART_SEGMENT_BASE_TMAP,
                    logical_height: i32::from(segment.logical_height),
                },
                None,
                false,
                sprite_camera,
                Mode20SpriteEntity {
                    x: segment.position.x,
                    y: segment.position.y,
                    z: segment.position.z,
                    yaw: segment.yaw,
                },
                frame.width,
                frame.height,
            );
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                segment.position,
                segment.render_parameter_index,
                interpolate_ground(
                    resources.ground_heights,
                    segment.position.x,
                    segment.position.y,
                ),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                firefly_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        let Some(actor) = world_resources
            .mission_actors
            .iter()
            .find(|actor| actor.source_entity_index == entity_index)
        else {
            return Err(TerrainFrameError(format!(
                "unknown Level 1 source sprite tuple: slot={entity_index} class={} model={} action={}",
                source_slot.class, source_slot.model, source_slot.action
            )));
        };
        if !actor.active || !actor.visible || actor.life < 0 {
            continue;
        }
        if matches!(
            actor.kind,
            Level1ActorKind::CastleSpellJar
                | Level1ActorKind::SpeedSpellJar
                | Level1ActorKind::ExitTarget
                | Level1ActorKind::ExperienceScroll
                | Level1ActorKind::Tree
                | Level1ActorKind::Stone
        ) {
            let (expected_render_index, expected_selector, expected_mode, expected_height) =
                match actor.kind {
                    Level1ActorKind::CastleSpellJar | Level1ActorKind::SpeedSpellJar => (
                        SPELL_JAR_RENDER_INDEX,
                        SPELL_JAR_BASE_TMAP,
                        SPELL_JAR_RENDER_MODE,
                        SPELL_JAR_LOGICAL_HEIGHT,
                    ),
                    Level1ActorKind::ExitTarget => (
                        EXIT_TARGET_RENDER_INDEX,
                        EXIT_TARGET_BASE_TMAP,
                        EXIT_TARGET_RENDER_MODE,
                        EXIT_TARGET_LOGICAL_HEIGHT,
                    ),
                    Level1ActorKind::ExperienceScroll => (
                        EXPERIENCE_SCROLL_RENDER_INDEX,
                        EXPERIENCE_SCROLL_BASE_TMAP,
                        EXPERIENCE_SCROLL_RENDER_MODE,
                        EXPERIENCE_SCROLL_LOGICAL_HEIGHT,
                    ),
                    Level1ActorKind::Tree => {
                        match (actor.render_parameter_index, actor.base_tmap) {
                            (Some(TREE_RENDER_INDEX_A), Some(TREE_BASE_TMAP_A)) => (
                                TREE_RENDER_INDEX_A,
                                TREE_BASE_TMAP_A,
                                TREE_RENDER_MODE,
                                TREE_LOGICAL_HEIGHT,
                            ),
                            (Some(TREE_RENDER_INDEX_B), Some(TREE_BASE_TMAP_B)) => (
                                TREE_RENDER_INDEX_B,
                                TREE_BASE_TMAP_B,
                                TREE_RENDER_MODE,
                                TREE_LOGICAL_HEIGHT,
                            ),
                            (Some(TREE_DEAD_RENDER_INDEX_A), Some(TREE_DEAD_BASE_TMAP_A)) => (
                                TREE_DEAD_RENDER_INDEX_A,
                                TREE_DEAD_BASE_TMAP_A,
                                TREE_RENDER_MODE,
                                TREE_LOGICAL_HEIGHT,
                            ),
                            (Some(TREE_DEAD_RENDER_INDEX_B), Some(TREE_DEAD_BASE_TMAP_B)) => (
                                TREE_DEAD_RENDER_INDEX_B,
                                TREE_DEAD_BASE_TMAP_B,
                                TREE_RENDER_MODE,
                                TREE_LOGICAL_HEIGHT,
                            ),
                            _ => {
                                return Err(TerrainFrameError(format!(
                                    "tree entity {entity_index} has unsupported render tuple {:?}/{:?}",
                                    actor.render_parameter_index, actor.base_tmap,
                                )));
                            }
                        }
                    }
                    Level1ActorKind::Stone => (
                        STONE_RENDER_INDEX,
                        STONE_BASE_TMAP,
                        STONE_RENDER_MODE,
                        STONE_LOGICAL_HEIGHT,
                    ),
                    _ => unreachable!("fixed objective-prop branch filtered actor kind"),
                };
            if (
                actor.render_parameter_index,
                actor.base_tmap,
                actor.render_mode,
                actor.logical_height,
            ) != (
                Some(expected_render_index),
                Some(expected_selector),
                Some(expected_mode),
                Some(expected_height),
            ) {
                return Err(TerrainFrameError(format!(
                    "objective prop entity {entity_index} has unsupported render tuple {:?}/{:?}/{:?}/{:?}",
                    actor.render_parameter_index,
                    actor.base_tmap,
                    actor.render_mode,
                    actor.logical_height,
                )));
            }
            let frame = world_resources
                .static_world_tmap_bank
                .frame(expected_selector)
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "objective prop selector {expected_selector} has no qualified frame"
                    ))
                })?;
            if frame.render_mode != expected_mode {
                return Err(TerrainFrameError(format!(
                    "objective prop selector {expected_selector} mode {} disagrees with actor mode {expected_mode}",
                    frame.render_mode
                )));
            }
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_fixed_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: expected_selector,
                    logical_height: i32::from(expected_height),
                },
                None,
                false,
                sprite_camera,
                Mode20SpriteEntity {
                    x: actor.position.x,
                    y: actor.position.y,
                    z: actor.position.z,
                    yaw: actor.yaw,
                },
                frame.width,
                frame.height,
            );
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                actor.position,
                expected_render_index,
                interpolate_ground(resources.ground_heights, actor.position.x, actor.position.y),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                objective_prop_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }
        if actor.kind == Level1ActorKind::ExitSwitch {
            return Err(TerrainFrameError(format!(
                "sprite-less ExitSwitch entity {entity_index} entered the renderable map chain"
            )));
        }
        if actor.kind == Level1ActorKind::MultipartHead {
            let exact_tuple = (
                actor.render_parameter_index,
                actor.base_tmap,
                actor.render_mode,
                actor.logical_height,
            ) == (
                Some(MULTIPART_HEAD_RENDER_INDEX),
                Some(MULTIPART_HEAD_BASE_TMAP),
                Some(MULTIPART_HEAD_RENDER_MODE),
                Some(MULTIPART_HEAD_LOGICAL_HEIGHT),
            );
            if !exact_tuple {
                return Err(TerrainFrameError(format!(
                    "multipart head entity {entity_index} has unsupported render tuple {:?}/{:?}/{:?}/{:?}",
                    actor.render_parameter_index,
                    actor.base_tmap,
                    actor.render_mode,
                    actor.logical_height,
                )));
            }
            let (_, selector, _) =
                mode17_selector_for_base(MULTIPART_HEAD_BASE_TMAP, actor.yaw, camera.yaw);
            let frame = world_resources
                .static_world_tmap_bank
                .frame(selector)
                .ok_or_else(|| {
                    TerrainFrameError(format!(
                        "multipart head selector {selector} has no qualified reset frame"
                    ))
                })?;
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_mode17_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: MULTIPART_HEAD_BASE_TMAP,
                    logical_height: i32::from(MULTIPART_HEAD_LOGICAL_HEIGHT),
                },
                sprite_camera,
                Mode20SpriteEntity {
                    x: actor.position.x,
                    y: actor.position.y,
                    z: actor.position.z,
                    yaw: actor.yaw,
                },
                frame.width,
                frame.height,
            );
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                actor.position,
                MULTIPART_HEAD_RENDER_INDEX,
                interpolate_ground(resources.ground_heights, actor.position.x, actor.position.y),
                frame.width,
                frame.height,
                &frame.pixels,
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                firefly_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }
        if actor.kind == Level1ActorKind::Firefly {
            let exact_tuple = (
                actor.render_parameter_index,
                actor.base_tmap,
                actor.render_mode,
                actor.logical_height,
            ) == (
                Some(FIREFLY_RENDER_INDEX),
                Some(FIREFLY_BASE_TMAP),
                Some(FIREFLY_RENDER_MODE),
                Some(FIREFLY_LOGICAL_HEIGHT),
            );
            if !exact_tuple {
                return Err(TerrainFrameError(format!(
                    "Firefly entity {entity_index} has unsupported render tuple {:?}/{:?}/{:?}/{:?}",
                    actor.render_parameter_index,
                    actor.base_tmap,
                    actor.render_mode,
                    actor.logical_height,
                )));
            }
            let (_, selector, _) =
                mode17_selector_for_base(FIREFLY_BASE_TMAP, actor.yaw, camera.yaw);
            let Some(clock) = world_resources.firefly_tmap_clocks.frame(selector) else {
                return Err(TerrainFrameError(format!(
                    "Firefly selector {selector} has no live TMAP clock"
                )));
            };
            let qualified_bank = world_resources
                .firefly_tmap_bank
                .animated()
                .iter()
                .any(|bank| bank.selector == selector);
            if !qualified_bank {
                return Err(TerrainFrameError(format!(
                    "Firefly selector {selector} is absent from the qualified bank"
                )));
            }
            let sprite_camera = Mode20SpriteCamera {
                render_x: camera.render_x,
                render_y: camera.render_y,
                eye_z: camera.eye_z,
                yaw: camera.yaw,
                projection,
            };
            let placement = project_mode17_sprite_with_definition(
                Mode20SpriteDefinition {
                    base_selector: FIREFLY_BASE_TMAP,
                    logical_height: i32::from(FIREFLY_LOGICAL_HEIGHT),
                },
                sprite_camera,
                Mode20SpriteEntity {
                    x: actor.position.x,
                    y: actor.position.y,
                    z: actor.position.z,
                    yaw: actor.yaw,
                },
                clock.width(),
                clock.height(),
            );
            if placement.is_ok()
                || matches!(
                    placement,
                    Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
                )
            {
                *firefly_selector_mask |= 1 << (selector - FIREFLY_BASE_TMAP);
            }
            let Ok(placement) = placement else { continue };
            raster_world_sprite_pass(
                pass,
                indices,
                placement,
                sprite_camera,
                actor.position,
                FIREFLY_RENDER_INDEX,
                interpolate_ground(resources.ground_heights, actor.position.x, actor.position.y),
                clock.width(),
                clock.height(),
                clock.pixels(),
                resources.tablesn,
                camera.roll,
                control_flags,
                shadow_surface,
                world_resources.night,
                false,
                world_resources.particle_parameters,
                firefly_aggregate,
                reflected_aggregate,
                shadow_aggregate,
            )
            .map_err(TerrainFrameError)?;
            continue;
        }

        let (render_index, base_tmap, render_mode, logical_height) = (
            actor.render_parameter_index,
            actor.base_tmap,
            actor.render_mode,
            actor.logical_height,
        );
        let exact_pose = matches!(
            (render_index, base_tmap, render_mode, logical_height),
            (
                Some(ARCHER_RENDER_INDEX),
                Some(ARCHER_BASE_TMAP),
                Some(ARCHER_RENDER_MODE),
                Some(ARCHER_LOGICAL_HEIGHT)
            ) | (
                Some(ARCHER_ATTACK_RENDER_INDEX_A),
                Some(ARCHER_ATTACK_BASE_TMAP_A),
                Some(ARCHER_RENDER_MODE),
                Some(ARCHER_LOGICAL_HEIGHT)
            ) | (
                Some(ARCHER_ATTACK_RENDER_INDEX_B),
                Some(ARCHER_ATTACK_BASE_TMAP_B),
                Some(ARCHER_RENDER_MODE),
                Some(ARCHER_ATTACK_LOGICAL_HEIGHT_B)
            )
        );
        if !exact_pose {
            return Err(TerrainFrameError(format!(
                "Archer entity {entity_index} has unsupported render tuple {render_index:?}/{base_tmap:?}/{render_mode:?}/{logical_height:?}"
            )));
        }
        let base_tmap = base_tmap.expect("exact Archer pose has base TMAP");
        let logical_height = logical_height.expect("exact Archer pose has logical height");
        let render_index = render_index.expect("exact Archer pose has render parameter");
        let (_, selector, _) = mode20_selector_for_base(base_tmap, actor.yaw, camera.yaw);
        let (width, height, pixels) =
            if let Some(clock) = world_resources.archer_tmap_clocks.animated_frame(selector) {
                (clock.width(), clock.height(), clock.pixels())
            } else if let Some(frame) = world_resources.archer_tmap_bank.static_frame(selector) {
                (frame.width, frame.height, frame.pixels.as_ref())
            } else {
                return Err(TerrainFrameError(format!(
                    "Archer selector {selector} has no qualified TMAP frame"
                )));
            };
        let sprite_camera = Mode20SpriteCamera {
            render_x: camera.render_x,
            render_y: camera.render_y,
            eye_z: camera.eye_z,
            yaw: camera.yaw,
            projection,
        };
        let placement = project_mode20_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: base_tmap,
                logical_height: i32::from(logical_height),
            },
            sprite_camera,
            Mode20SpriteEntity {
                x: actor.position.x,
                y: actor.position.y,
                z: actor.position.z,
                yaw: actor.yaw,
            },
            width,
            height,
        );
        if placement.is_ok()
            || matches!(
                placement,
                Err(crate::goat_sprite_raster::Mode20SpriteReject::NonPositiveProjection)
            )
        {
            if selector >= ARCHER_BASE_TMAP && selector <= ARCHER_BASE_TMAP + 2 {
                *archer_selector_mask |= 1 << (selector - ARCHER_BASE_TMAP);
            }
        }
        let Ok(placement) = placement else { continue };
        raster_world_sprite_pass(
            pass,
            indices,
            placement,
            sprite_camera,
            actor.position,
            render_index,
            interpolate_ground(resources.ground_heights, actor.position.x, actor.position.y),
            width,
            height,
            pixels,
            resources.tablesn,
            camera.roll,
            control_flags,
            shadow_surface,
            world_resources.night,
            false,
            world_resources.particle_parameters,
            archer_aggregate,
            reflected_aggregate,
            shadow_aggregate,
        )
        .map_err(TerrainFrameError)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn raster_source_sprite_submission(
    pass: WorldChainPass,
    submission: SourceSpriteSubmission,
    camera: Level1TerrainFrameCamera,
    projection: OutdoorProjection,
    resources: &Level1TerrainFrameResources<'_>,
    world_resources: &Level1WorldFrameResources<'_>,
    source_sprites: &SourceSpriteWorldResources<'_>,
    indices: &mut [u8],
    shadow_surface: bool,
    aggregate: &mut Mode20SpriteRasterStats,
    reflected_aggregate: &mut Mode20SpriteRasterStats,
    shadow_aggregate: &mut Mode20SpriteRasterStats,
) -> Result<(), TerrainFrameError> {
    if pass == WorldChainPass::Reflection && !submission.reflection_eligible {
        return Ok(());
    }
    let selector = match submission.render_mode {
        17 => mode17_selector_for_base(submission.base_tmap, submission.yaw, camera.yaw).1,
        19 => mode19_selector_for_base(submission.base_tmap, submission.yaw, camera.yaw).1,
        20 => mode20_selector_for_base(submission.base_tmap, submission.yaw, camera.yaw).1,
        0 | 1 | 11 | 21 | 36 => {
            if submission.selected_frame == 0 {
                submission.base_tmap
            } else {
                submission.selected_frame
            }
        }
        mode => {
            return Err(TerrainFrameError(format!(
                "source sprite slot {} uses unsupported render mode {mode}",
                submission.source_entity_index
            )));
        }
    };
    let frame = source_sprites.tmap_bank.frame(selector).ok_or_else(|| {
        TerrainFrameError(format!(
            "source sprite slot {} selector {selector} has no qualified TMAP frame",
            submission.source_entity_index
        ))
    })?;
    let sprite_camera = Mode20SpriteCamera {
        render_x: camera.render_x,
        render_y: camera.render_y,
        eye_z: camera.eye_z,
        yaw: camera.yaw,
        projection,
    };
    let entity = Mode20SpriteEntity {
        x: submission.position.x,
        y: submission.position.y,
        z: submission.position.z,
        yaw: submission.yaw,
    };
    let definition = Mode20SpriteDefinition {
        base_selector: selector,
        logical_height: i32::from(submission.logical_height),
    };
    let placement = match submission.render_mode {
        17 => project_mode17_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: submission.base_tmap,
                logical_height: definition.logical_height,
            },
            sprite_camera,
            entity,
            frame.width,
            frame.height,
        ),
        19 => project_mode19_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: submission.base_tmap,
                logical_height: definition.logical_height,
            },
            sprite_camera,
            entity,
            frame.width,
            frame.height,
        ),
        20 => project_mode20_sprite_with_definition(
            Mode20SpriteDefinition {
                base_selector: submission.base_tmap,
                logical_height: definition.logical_height,
            },
            sprite_camera,
            entity,
            frame.width,
            frame.height,
        ),
        _ => project_fixed_sprite_with_definition(
            definition,
            None,
            matches!(submission.render_mode, 21 | 36),
            sprite_camera,
            entity,
            frame.width,
            frame.height,
        ),
    };
    let Ok(placement) = placement else {
        return Ok(());
    };
    let control_flags = if submission.shadow_eligible {
        submission.control_flags
    } else {
        submission.control_flags | 0x0808_0000
    };
    raster_world_sprite_pass(
        pass,
        indices,
        placement,
        sprite_camera,
        submission.position,
        submission.render_parameter_index,
        interpolate_ground(
            resources.ground_heights,
            submission.position.x,
            submission.position.y,
        ),
        frame.width,
        frame.height,
        &frame.pixels,
        resources.tablesn,
        camera.roll.wrapping_add(submission.roll),
        control_flags,
        shadow_surface,
        world_resources.night,
        submission.render_parameter_index == POSSESSION_PROJECTILE_RENDER_INDEX,
        world_resources.particle_parameters,
        aggregate,
        reflected_aggregate,
        shadow_aggregate,
    )
    .map_err(TerrainFrameError)
}

fn accumulate_sprite_stats(
    aggregate: &mut Mode20SpriteRasterStats,
    stats: Mode20SpriteRasterStats,
) {
    aggregate.candidate_pixels = aggregate
        .candidate_pixels
        .wrapping_add(stats.candidate_pixels);
    aggregate.transparent_pixels = aggregate
        .transparent_pixels
        .wrapping_add(stats.transparent_pixels);
    aggregate.clipped_pixels = aggregate.clipped_pixels.wrapping_add(stats.clipped_pixels);
    aggregate.framebuffer_writes = aggregate
        .framebuffer_writes
        .wrapping_add(stats.framebuffer_writes);
}

#[allow(clippy::too_many_arguments)]
fn raster_world_sprite_pass(
    pass: WorldChainPass,
    indices: &mut [u8],
    placement: crate::goat_sprite_raster::Mode20SpritePlacement,
    sprite_camera: Mode20SpriteCamera,
    position: crate::Position,
    render_parameter_index: u16,
    terrain_altitude: i32,
    source_width: u16,
    source_height: u16,
    source_pixels: &[u8],
    tablesn: &[u8],
    roll: i16,
    control_flags: u32,
    shadow_surface: bool,
    night: bool,
    possession: bool,
    particle_parameters: &[ParticleParameter],
    primary_aggregate: &mut Mode20SpriteRasterStats,
    reflected_aggregate: &mut Mode20SpriteRasterStats,
    shadow_aggregate: &mut Mode20SpriteRasterStats,
) -> Result<(), String> {
    let output_width = usize::try_from(sprite_camera.projection.viewport_width)
        .map_err(|_| "sprite projection viewport width is negative".to_owned())?;
    let output_height = usize::try_from(sprite_camera.projection.viewport_height)
        .map_err(|_| "sprite projection viewport height is negative".to_owned())?;
    let expected_extent = output_width
        .checked_mul(output_height)
        .ok_or_else(|| "sprite projection viewport extent overflow".to_owned())?;
    if output_width == 0 || output_height == 0 || indices.len() != expected_extent {
        return Err(format!(
            "sprite projection viewport requires {output_width}x{output_height}={expected_extent} bytes, got {}",
            indices.len()
        ));
    }
    match pass {
        WorldChainPass::Reflection => {
            let reflected =
                reproject_sprite_altitude(placement, sprite_camera, -i32::from(position.z));
            let stats = if possession {
                rasterize_reflected_possession_sprite(
                    indices,
                    output_width,
                    output_height,
                    reflected,
                    source_width,
                    source_height,
                    source_pixels,
                    tablesn,
                    roll,
                )
            } else {
                rasterize_reflected_sprite(
                    indices,
                    output_width,
                    output_height,
                    reflected,
                    source_width,
                    source_height,
                    source_pixels,
                    tablesn,
                    roll,
                )
            }?;
            accumulate_sprite_stats(reflected_aggregate, stats);
        }
        WorldChainPass::PrimaryWithShadow => {
            let parameter = particle_parameters
                .get(usize::from(render_parameter_index))
                .ok_or_else(|| {
                    format!(
                        "render parameter {render_parameter_index} is absent from the qualified particle table"
                    )
                })?;
            if shadow_surface
                && control_flags & 0x0808_0000 == 0
                && parameter.byte_10 == 0
                && let Ok(shadow) =
                    project_sprite_shadow(placement, sprite_camera, terrain_altitude, night)
            {
                let stats = rasterize_shadow_sprite(
                    indices,
                    output_width,
                    output_height,
                    shadow,
                    source_width,
                    source_height,
                    source_pixels,
                    tablesn,
                    roll,
                )?;
                accumulate_sprite_stats(shadow_aggregate, stats);
            }
            let stats = if possession {
                rasterize_possession_sprite(
                    indices,
                    output_width,
                    output_height,
                    placement,
                    source_width,
                    source_height,
                    source_pixels,
                    tablesn,
                    roll,
                )
            } else {
                rasterize_mode20_sprite(
                    indices,
                    output_width,
                    output_height,
                    placement,
                    source_width,
                    source_height,
                    source_pixels,
                    tablesn,
                    roll,
                )
            }?;
            accumulate_sprite_stats(primary_aggregate, stats);
        }
    }
    Ok(())
}

#[must_use]
pub fn level1_projection(camera: Level1TerrainFrameCamera) -> OutdoorProjection {
    let diagonal_squared =
        (LEVEL1_VIEW_WIDTH * LEVEL1_VIEW_WIDTH + LEVEL1_VIEW_HEIGHT * LEVEL1_VIEW_HEIGHT) as u32;
    let diagonal = legacy_sqrt(diagonal_squared) as i32;
    OutdoorProjection {
        scale: 7_i32
            .wrapping_mul(diagonal)
            .wrapping_mul(i32::from(camera.fov))
            >> 11,
        pitch_offset: i32::from(camera.pitch).wrapping_mul(LEVEL1_VIEW_WIDTH as i32) >> 8,
        roll: camera.roll,
        center_x: LEVEL1_VIEW_WIDTH as i32 / 2,
        center_y: LEVEL1_VIEW_HEIGHT as i32 / 2,
        viewport_width: LEVEL1_VIEW_WIDTH as i32,
        viewport_height: LEVEL1_VIEW_HEIGHT as i32,
    }
}

#[must_use]
fn level1_map_projection(camera: Level1TerrainFrameCamera) -> OutdoorProjection {
    const WIDTH: i32 = 128;
    const HEIGHT: i32 = 200;
    let diagonal = legacy_sqrt(u32::try_from(WIDTH * WIDTH + HEIGHT * HEIGHT).unwrap()) as i32;
    OutdoorProjection {
        scale: 7_i32
            .wrapping_mul(diagonal)
            .wrapping_mul(i32::from(camera.fov))
            >> 11,
        pitch_offset: i32::from(camera.pitch).wrapping_mul(WIDTH) >> 8,
        roll: camera.roll,
        center_x: 192 + WIDTH / 2,
        center_y: HEIGHT / 2,
        // Rasterizers target the shared 320x200 backing.  Pixels left of the
        // fixed map viewport are discarded by the later exact viewport copy.
        viewport_width: LEVEL1_VIEW_WIDTH as i32,
        viewport_height: HEIGHT,
    }
}

/// Reproduce `DrawSky_40950` for the qualified 320×200 Level 1 viewport.
pub fn draw_level1_sky(
    camera: Level1TerrainFrameCamera,
    sky: &[u8],
) -> Result<Vec<u8>, TerrainFrameError> {
    draw_level1_sky_viewport(camera, sky, LEVEL1_VIEW_WIDTH, 0)
}

fn draw_level1_map_sky(
    camera: Level1TerrainFrameCamera,
    sky: &[u8],
) -> Result<Vec<u8>, TerrainFrameError> {
    draw_level1_sky_viewport(camera, sky, 128, 192)
}

fn draw_level1_sky_viewport(
    camera: Level1TerrainFrameCamera,
    sky: &[u8],
    viewport_width: usize,
    viewport_x: usize,
) -> Result<Vec<u8>, TerrainFrameError> {
    if sky.len() != LEVEL1_SKY_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 sky must contain {LEVEL1_SKY_BYTES} bytes, got {}",
            sky.len()
        )));
    }
    if viewport_width == 0 || viewport_x + viewport_width > LEVEL1_VIEW_WIDTH {
        return Err(TerrainFrameError(format!(
            "Level 1 sky viewport {viewport_x}+{viewport_width} is outside {LEVEL1_VIEW_WIDTH}"
        )));
    }
    let projection = if viewport_width == LEVEL1_VIEW_WIDTH {
        level1_projection(camera)
    } else {
        level1_map_projection(camera)
    };
    let roll = i32::from(camera.roll) & 0x07ff;
    let cos = SIN_16_16[usize::try_from(512 + roll).expect("masked sky cosine index")];
    let sin = SIN_16_16[usize::try_from(roll).expect("masked sky sine index")];
    let cos_step = cos.wrapping_shl(8)
        / i32::try_from(viewport_width).expect("fixed sky viewport width fits i32");
    let sin_step = sin.wrapping_shl(8)
        / i32::try_from(viewport_width).expect("fixed sky viewport width fits i32");

    let mut x_deltas = vec![0_u8; viewport_width];
    let mut y_deltas = vec![0_u8; viewport_width];
    let mut error_x = 0_i32;
    let mut error_y = 0_i32;
    let mut previous_x = 0_u8;
    let mut previous_y = 0_u8;
    for column in 0..viewport_width {
        let current_x = byte2(error_x);
        let current_y = byte2(error_y);
        x_deltas[column] = current_x.wrapping_sub(previous_x);
        y_deltas[column] = current_y.wrapping_sub(previous_y);
        previous_x = current_x;
        previous_y = current_y;
        error_x = error_x.wrapping_add(cos_step);
        error_y = error_y.wrapping_add(sin_step);
    }

    let origin_x = (sin.wrapping_mul(projection.pitch_offset).wrapping_neg() >> 16)
        .wrapping_add(projection.center_x);
    let origin_y = projection
        .center_y
        .wrapping_sub(cos.wrapping_mul(projection.pitch_offset) >> 16);
    let rotated_x = origin_x
        .wrapping_mul(cos_step)
        .wrapping_sub(origin_y.wrapping_mul(sin_step));
    let rotated_y = cos_step
        .wrapping_mul(origin_y)
        .wrapping_add(sin_step.wrapping_mul(origin_x));
    let mut row_u = (i32::from(camera.yaw as u16) << 15).wrapping_sub(rotated_x);
    let mut row_v = rotated_y.wrapping_neg();
    let mut frame = vec![0; LEVEL1_VIEW_PIXELS];

    for row in 0..LEVEL1_VIEW_HEIGHT {
        let mut u = byte2(row_u);
        let mut v = byte2(row_v);
        for column in 0..viewport_width {
            frame[row * LEVEL1_VIEW_WIDTH + viewport_x + column] =
                sky[(usize::from(v) << 8) | usize::from(u)];
            u = u.wrapping_add(x_deltas[column]);
            v = v.wrapping_add(y_deltas[column]);
        }
        row_u = row_u.wrapping_sub(sin_step);
        row_v = row_v.wrapping_add(cos_step);
    }
    Ok(frame)
}

fn raster_submission(
    submission: TerrainTriangleSubmission,
    lattice: &[OutdoorGridVertex; 840],
    projected: &[Option<OutdoorReflectionVertex>; 840],
    resources: &Level1TerrainFrameResources<'_>,
    indices: &mut [u8],
    coverage: &mut [u8],
    stats: &mut TerrainFrameStats,
) -> Result<(), TerrainFrameError> {
    let vertices = triangle_vertices(submission, lattice, projected)?;
    let texture_base = terrain_texture_base(submission.texture_index)?;
    match submission.raster_selector {
        MODE5_TERRAIN_SELECTOR => {
            let pixels = rasterize_mode5_triangle_from_atlas(
                vertices,
                LEVEL1_VIEW_WIDTH as i32,
                LEVEL1_VIEW_HEIGHT as i32,
                resources.terrain_atlas,
                texture_base,
                &resources.tablesn[..65_536],
            );
            for pixel in pixels {
                let index = usize::try_from(pixel.y).expect("mode-5 Y is clipped")
                    * LEVEL1_VIEW_WIDTH
                    + usize::try_from(pixel.sample.x).expect("mode-5 X is clipped");
                indices[index] = pixel.sample.output_index;
                coverage[index] = 255;
                stats.terrain_pixel_writes = stats.terrain_pixel_writes.wrapping_add(1);
            }
            match submission.pass {
                TerrainPass::Reflected => {
                    stats.reflected_triangles = stats.reflected_triangles.wrapping_add(1);
                }
                TerrainPass::Primary => {
                    stats.primary_mode5_triangles = stats.primary_mode5_triangles.wrapping_add(1);
                }
            }
        }
        MODE26_REFLECTIVE_TERRAIN_SELECTOR => {
            if submission.pass != TerrainPass::Primary {
                return Err(TerrainFrameError(
                    "mode 26 is valid only for the Level 1 primary terrain pass".to_owned(),
                ));
            }
            let pixels = rasterize_mode26_triangle_from_atlas_into_framebuffer(
                vertices,
                LEVEL1_VIEW_WIDTH as i32,
                LEVEL1_VIEW_HEIGHT as i32,
                resources.terrain_atlas,
                texture_base,
                resources.tablesn,
                indices,
            );
            for pixel in pixels {
                let index = usize::try_from(pixel.y).expect("mode-26 Y is clipped")
                    * LEVEL1_VIEW_WIDTH
                    + usize::try_from(pixel.sample.x).expect("mode-26 X is clipped");
                coverage[index] = 255;
                stats.terrain_pixel_writes = stats.terrain_pixel_writes.wrapping_add(1);
            }
            stats.primary_mode26_triangles = stats.primary_mode26_triangles.wrapping_add(1);
        }
        selector => {
            return Err(TerrainFrameError(format!(
                "unsupported Level 1 terrain raster selector {selector}; no approximation allowed"
            )));
        }
    }
    Ok(())
}

fn triangle_vertices(
    submission: TerrainTriangleSubmission,
    lattice: &[OutdoorGridVertex; 840],
    projected: &[Option<OutdoorReflectionVertex>; 840],
) -> Result<[Mode5Vertex; 3], TerrainFrameError> {
    let uv = source_uv_row(submission.texture_uv_row)?;
    let mut result = [Mode5Vertex::default(); 3];
    for (destination, source_index) in result.iter_mut().zip(submission.vertex_indices) {
        let projected_vertex = projected[source_index].ok_or_else(|| {
            TerrainFrameError(format!(
                "submitted terrain vertex {source_index} has no projected value"
            ))
        })?;
        let corner = cell_corner(submission.owner_index, source_index)?;
        let screen = match submission.pass {
            TerrainPass::Reflected => projected_vertex.reflected_screen,
            TerrainPass::Primary => projected_vertex.primary_screen,
        };
        *destination = Mode5Vertex {
            x: screen.x,
            y: screen.y,
            u: uv[corner][0],
            v: uv[corner][1],
            pnt5: lattice[source_index].pnt5.ok_or_else(|| {
                TerrainFrameError(format!(
                    "submitted terrain vertex {source_index} has no pnt5"
                ))
            })?,
        };
    }
    Ok(result)
}

fn cell_corner(owner: usize, vertex: usize) -> Result<usize, TerrainFrameError> {
    if vertex == owner {
        Ok(0)
    } else if vertex == owner + 1 {
        Ok(1)
    } else if vertex + 39 == owner {
        Ok(2)
    } else if vertex + 40 == owner {
        Ok(3)
    } else {
        Err(TerrainFrameError(format!(
            "terrain vertex {vertex} does not belong to owner cell {owner}"
        )))
    }
}

fn source_uv_row(row: u8) -> Result<[[i32; 2]; 4], TerrainFrameError> {
    let index = usize::from(row);
    let base = *UV_ROW_ORDER
        .get(index)
        .ok_or_else(|| TerrainFrameError(format!("terrain UV row {row} is out of bounds")))?;
    Ok(BASE_UV_ROWS[base])
}

fn terrain_texture_base(selector: u8) -> Result<usize, TerrainFrameError> {
    let selector = usize::from(selector);
    if selector >= LEVEL1_TERRAIN_TILE_COUNT {
        return Err(TerrainFrameError(format!(
            "terrain selector {selector} is outside the qualified Level 1 BL32N bank"
        )));
    }
    Ok((selector / LEVEL1_TERRAIN_TILE_COLUMNS)
        * LEVEL1_TERRAIN_TILE_SIZE
        * LEVEL1_TERRAIN_ATLAS_WIDTH
        + (selector % LEVEL1_TERRAIN_TILE_COLUMNS) * LEVEL1_TERRAIN_TILE_SIZE)
}

fn validate_resources(
    resources: &Level1TerrainFrameResources<'_>,
) -> Result<(), TerrainFrameError> {
    for (name, values) in [
        ("terrain types", resources.terrain_types),
        ("ground heights", resources.ground_heights),
        ("terrain shading", resources.shading),
        ("terrain angles", resources.angles),
    ] {
        if values.len() != 65_536 {
            return Err(TerrainFrameError(format!(
                "{name} must contain 65,536 bytes, got {}",
                values.len()
            )));
        }
    }
    if resources.terrain_atlas.len() != LEVEL1_TERRAIN_ATLAS_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 BL32N atlas must contain {LEVEL1_TERRAIN_ATLAS_BYTES} bytes, got {}",
            resources.terrain_atlas.len()
        )));
    }
    if resources.tablesn.len() != NIGHT_TABLES_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 TABLESN must contain {NIGHT_TABLES_BYTES} bytes, got {}",
            resources.tablesn.len()
        )));
    }
    if resources.sky.len() != LEVEL1_SKY_BYTES {
        return Err(TerrainFrameError(format!(
            "Level 1 SKYN must contain {LEVEL1_SKY_BYTES} bytes, got {}",
            resources.sky.len()
        )));
    }
    Ok(())
}

#[must_use]
pub fn byte_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

const fn byte2(value: i32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Position,
        frozen_actor_tmap::{LEVEL1_FROZEN_ACTOR_ENTITY_INDICES, VILLAGER_ANIMATED_SELECTORS},
        goat_tmap::GoatTmapClocks,
        native_world::{NativeSourceFrameResources, NativeWorld},
    };
    use std::path::PathBuf;

    #[test]
    fn reflection_arena_limit_and_shadow_material_cell_match_source_walk() {
        assert!(reflection_chain_entity_admitted(999));
        assert!(!reflection_chain_entity_admitted(1_000));
        assert!(!reflection_chain_entity_admitted(u16::MAX));

        let mut terrain_types = [0_u8; 65_536];
        let angles = [0_u8; 65_536];
        // Quadrant 1 reads the shadow classifier from (x-1,y), while the
        // subsequent entity-chain head is (x,y). Put a shadow-blocking
        // material only in the former so the two source cells cannot be
        // accidentally conflated again.
        terrain_types[99 + 100 * 256] = 8;
        terrain_types[100 + 100 * 256] = 0;
        assert!(!shadow_surface_allows(
            100,
            100,
            512,
            &terrain_types,
            &angles
        ));
        assert!(!terrain_material_blocks_shadow(
            terrain_types[100 + 100 * 256]
        ));
    }

    #[test]
    fn source_uv_table_preserves_all_32_rows_and_inclusive_fixed_edges() {
        assert_eq!(source_uv_row(0).unwrap(), BASE_UV_ROWS[0]);
        assert_eq!(source_uv_row(8).unwrap(), BASE_UV_ROWS[6]);
        assert_eq!(source_uv_row(16).unwrap(), BASE_UV_ROWS[5]);
        assert_eq!(source_uv_row(24).unwrap(), BASE_UV_ROWS[1]);
        assert_eq!(source_uv_row(31).unwrap(), BASE_UV_ROWS[6]);
        assert_eq!(source_uv_row(0).unwrap()[1][0], 0x001f_ffff);
        assert!(source_uv_row(32).is_err());
    }

    #[test]
    fn atlas_pointer_uses_source_256_stride_and_reaches_last_tile() {
        assert_eq!(terrain_texture_base(0).unwrap(), 0);
        assert_eq!(terrain_texture_base(7).unwrap(), 224);
        assert_eq!(terrain_texture_base(8).unwrap(), 8_192);
        assert_eq!(terrain_texture_base(151).unwrap(), 147_680);
        assert_eq!(
            terrain_texture_base(151).unwrap() + 31 * 256 + 31,
            LEVEL1_TERRAIN_ATLAS_BYTES - 1
        );
        assert!(terrain_texture_base(152).is_err());
    }

    #[test]
    fn qualified_projection_is_164_at_default_fov() {
        let camera = Level1TerrainFrameCamera {
            render_x: 0,
            render_y: 0,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        assert_eq!(level1_projection(camera).scale, 164);
    }

    #[test]
    fn zero_sky_is_stable_and_complete() {
        let camera = Level1TerrainFrameCamera {
            render_x: 0,
            render_y: 0,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        let frame = draw_level1_sky(camera, &[0; LEVEL1_SKY_BYTES]).unwrap();
        assert_eq!(frame.len(), LEVEL1_VIEW_PIXELS);
        assert!(frame.iter().all(|value| *value == 0));
    }

    #[test]
    fn qualified_owner_reset_and_low_water_frames_are_deterministic() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
        .expect("validated Level 1 owner package");
        let atlas = std::fs::read(
            private.join("generated/level1-unity-asset-cache/terrain/bl32n.atlas.indices.u8"),
        )
        .expect("qualified Level 1 source-layout terrain atlas");
        let tablesn =
            std::fs::read(private.join("generated/level1-unity-asset-cache/palette/tablesn.u8"))
                .expect("qualified Level 1 TABLESN");
        let sky =
            std::fs::read(private.join("generated/level1-unity-asset-cache/sky/skyn.indices.u8"))
                .expect("qualified Level 1 SKYN");
        let planes = world.terrain_planes();
        let camera = Level1TerrainFrameCamera {
            render_x: 19_841,
            render_y: 56_958,
            eye_z: 5_152,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 8,
            reflections: true,
        };
        let reset_lattice = outdoor_grid(OutdoorGridInput {
            render_x: camera.render_x,
            render_y: camera.render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let mut visible = 0;
        let mut culled = 0;
        let mut near = 0;
        let mut fog_ramp = 0;
        let mut fog_zero = 0;
        let mut parity = [0; 2];
        let mut d4_by_band = [0_u8; 3];
        for vertex in reset_lattice {
            if vertex.feature_flags & 0x0002 != 0 {
                culled += 1;
                continue;
            }
            visible += 1;
            parity[usize::from(vertex.feature_flags & 1)] += 1;
            let band = if vertex.distance_squared <= 14_745_600 {
                near += 1;
                0
            } else if vertex.distance_squared < 23_658_496 {
                fog_ramp += 1;
                1
            } else {
                fog_zero += 1;
                2
            };
            let map_index = usize::from(vertex.tile_x) + usize::from(vertex.tile_y) * 256;
            d4_by_band[band] |= 1 << ((planes.angles[map_index] >> 4) & 7);
        }
        assert_eq!((visible, culled), (672, 168));
        assert_eq!((near, fog_ramp, fog_zero), (388, 212, 72));
        assert_eq!(parity, [336, 336]);
        assert_eq!(d4_by_band, [0xff, 0xff, 0xff]);
        let render = || {
            render_level1_terrain_frame(
                camera,
                Level1TerrainFrameResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    terrain_atlas: &atlas,
                    tablesn: &tablesn,
                    sky: &sky,
                },
            )
            .expect("qualified isolated Level 1 terrain frame")
        };
        let first = render();
        let second = render();
        assert_eq!(first, second);
        assert_eq!(first.index_hash, 0x2f38_beef_8e4a_62fb);
        assert_eq!(first.coverage_hash, 0x65fa_674a_e4af_efec);
        assert_eq!(
            first.stats,
            TerrainFrameStats {
                reflected_triangles: 0,
                primary_mode5_triangles: 578,
                primary_mode26_triangles: 0,
                terrain_pixel_writes: 37_174,
            }
        );
        assert!(first.stats.primary_mode5_triangles > 0);
        assert!(first.terrain_coverage.contains(&255));

        let light_position = crate::Position {
            x: camera.render_x & 0xff00 | 0x80,
            y: (camera.render_y & 0xff00)
                .wrapping_sub(10 << 8)
                .wrapping_add(0x80),
            z: i16::try_from(interpolate_ground(
                &planes.ground_heights,
                camera.render_x & 0xff00 | 0x80,
                (camera.render_y & 0xff00)
                    .wrapping_sub(10 << 8)
                    .wrapping_add(0x80),
            ))
            .expect("qualified Level 1 light altitude"),
        };
        let mut lighting = crate::dynamic_lighting::DynamicLightingState::reset();
        assert_eq!(
            lighting.admit(true, true, 152, light_position, 128, 7, false),
            Some(0)
        );
        let mut lit_shading = planes.shading.to_vec();
        lighting.service_recompute(
            &mut lit_shading,
            true,
            |_| {
                Some(crate::dynamic_lighting::DynamicLightLinkedEntity {
                    position: light_position,
                    life: 31,
                    disabled: false,
                })
            },
            |x, y| {
                i16::try_from(interpolate_ground(&planes.ground_heights, x, y))
                    .expect("qualified Level 1 ground altitude")
            },
        );
        let lighting_hash = lighting.hash_v1();
        let lit_render = || {
            render_level1_terrain_frame(
                camera,
                Level1TerrainFrameResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &lit_shading,
                    angles: &planes.angles,
                    terrain_atlas: &atlas,
                    tablesn: &tablesn,
                    sky: &sky,
                },
            )
            .expect("qualified dynamically lit terrain frame")
        };
        let lit_first = lit_render();
        let lit_repeated = lit_render();
        assert_eq!(lit_repeated, lit_first);
        assert_eq!(lighting.hash_v1(), lighting_hash);
        assert_ne!(lit_first.index_hash, first.index_hash);
        assert_eq!(
            (
                light_position,
                lighting.rng(),
                lighting.slots()[0].history,
                byte_hash(&lit_shading),
                lit_first.index_hash,
            ),
            (
                crate::Position {
                    x: 19_840,
                    y: 54_400,
                    z: 4_896,
                },
                0x84ca_7cd2,
                [
                    0, 0, 3, 0, 0, 0, 0, 24, 0, 0, 0, 3, 31, 0, 0, 0, 0, 24, 17, 0, 0, 0, 0, 0, 0,
                ],
                0xeb19_63c7_e822_72e3,
                0x65ba_6566_350d_f608,
            ),
            "exact 25-cell shade bytes reach exact final production terrain pixels"
        );

        let low_camera = Level1TerrainFrameCamera {
            render_x: 16 << 8,
            render_y: 0,
            eye_z: 2_000,
            ..camera
        };
        let low = render_level1_terrain_frame(
            low_camera,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified low-altitude reflection frame");
        assert_eq!(low.index_hash, 0x9721_91f6_b54d_1e2a);
        assert_eq!(low.coverage_hash, 0x5abf_682e_9728_07ca);
        assert_eq!(
            low.stats,
            TerrainFrameStats {
                reflected_triangles: 36,
                primary_mode5_triangles: 0,
                primary_mode26_triangles: 300,
                terrain_pixel_writes: 7_885,
            }
        );
        assert!(low.stats.reflected_triangles > 0);
        assert!(low.stats.primary_mode26_triangles > 0);

        let moving_low = render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                source_turn: low_camera.source_turn + 1,
                ..low_camera
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified moving low-altitude reflection frame");
        assert_eq!(moving_low.index_hash, 0x9992_4581_ea78_82b0);
        assert_eq!(moving_low.coverage_hash, 0x1424_ba7c_1c11_d071);
        assert_eq!(
            moving_low.stats,
            TerrainFrameStats {
                reflected_triangles: 36,
                primary_mode5_triangles: 0,
                primary_mode26_triangles: 300,
                terrain_pixel_writes: 7_932,
            }
        );

        for seam_y in [u16::MAX, 0] {
            let seam = render_level1_terrain_frame(
                Level1TerrainFrameCamera {
                    render_y: seam_y,
                    ..camera
                },
                Level1TerrainFrameResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    terrain_atlas: &atlas,
                    tablesn: &tablesn,
                    sky: &sky,
                },
            )
            .expect("qualified map-seam terrain frame");
            let seam_lattice = outdoor_grid(OutdoorGridInput {
                render_x: camera.render_x,
                render_y: seam_y,
                yaw: camera.yaw,
                turn: camera.source_turn,
                shading: &planes.shading,
                angles: &planes.angles,
            });
            assert!(seam_lattice.iter().any(|vertex| vertex.tile_y == 0));
            assert!(seam_lattice.iter().any(|vertex| vertex.tile_y == 255));
            let expected = if seam_y == u16::MAX {
                (0xe519_7093_29e4_23e8, 0xd33b_c3da_61c8_1217, 68_931)
            } else {
                (0x0025_703f_712a_3129, 0x8210_0660_1a3c_8f55, 68_908)
            };
            assert_eq!(
                (
                    seam.index_hash,
                    seam.coverage_hash,
                    seam.stats.terrain_pixel_writes
                ),
                expected
            );
            assert_eq!(seam.stats.reflected_triangles, 0);
            assert_eq!(seam.stats.primary_mode5_triangles, 604);
            assert_eq!(seam.stats.primary_mode26_triangles, 0);
        }

        let no_reflections = render_level1_terrain_frame(
            Level1TerrainFrameCamera {
                reflections: false,
                ..low_camera
            },
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &atlas,
                tablesn: &tablesn,
                sky: &sky,
            },
        )
        .expect("qualified reflection-disabled terrain frame");
        assert_eq!(no_reflections.stats.reflected_triangles, 0);
        assert_eq!(no_reflections.stats.primary_mode26_triangles, 0);
        assert!(no_reflections.stats.primary_mode5_triangles > 0);
    }

    #[test]
    fn owner_cache_goat_world_frame_is_interleaved_and_deterministic() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let source_cache = std::env::var_os("MC_PLAN28_SOURCE_CACHE_PATH").map(PathBuf::from);
        let cache = source_cache
            .clone()
            .unwrap_or_else(|| private.join("generated/level1-unity-asset-cache"));
        let world = NativeWorld::load_with_owner_cache(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
            &cache,
        )
        .expect("validated Level 1 owner package and cache");
        let atlas = std::fs::read(cache.join("terrain/bl32n.atlas.indices.u8"))
            .expect("qualified source-layout terrain atlas");
        let tablesn = std::fs::read(cache.join("palette/tablesn.u8")).expect("qualified TABLESN");
        let sky = std::fs::read(cache.join("sky/skyn.indices.u8")).expect("qualified SKYN");
        let planes = world.terrain_planes();
        let goat = &world.reset_live_world().goats()[0];
        let camera = Level1TerrainFrameCamera {
            render_x: goat.position.x,
            render_y: goat.position.y.wrapping_add(1_000),
            eye_z: i32::from(goat.position.z) + 100,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        let render_resources = || Level1TerrainFrameResources {
            terrain_types: &planes.terrain_types,
            ground_heights: &planes.ground_heights,
            shading: &planes.shading,
            angles: &planes.angles,
            terrain_atlas: &atlas,
            tablesn: &tablesn,
            sky: &sky,
        };
        let terrain = render_level1_terrain_frame(camera, render_resources())
            .expect("isolated terrain frame");
        let clocks =
            GoatTmapClocks::from_bank(world.goat_tmap_bank().expect("qualified Goat TMAP bank"));
        let animated = [8_u16, 9, 10].map(|selector| {
            std::fs::read(cache.join(format!("terrain/tmaps1/{selector:03}.decoded.bin")))
                .expect("qualified animated Archer TMAP")
        });
        let statics = [11_u16, 12, 13, 121, 122, 123].map(|selector| {
            std::fs::read(cache.join(format!("terrain/tmaps1/{selector:03}.indices.u8")))
                .expect("qualified static Archer TMAP")
        });
        let archer_bank = ArcherTmapBank::decode_qualified(
            [&animated[0], &animated[1], &animated[2]],
            [
                &statics[0],
                &statics[1],
                &statics[2],
                &statics[3],
                &statics[4],
                &statics[5],
            ],
        )
        .expect("qualified Archer TMAP bank");
        let archer_clocks = ArcherTmapClocks::from_bank(&archer_bank);
        let firefly_payloads = std::array::from_fn::<_, 8, _>(|ordinal| {
            let selector = 279 + ordinal as u16;
            std::fs::read(cache.join(format!("terrain/tmaps1/{selector:03}.decoded.bin")))
                .expect("qualified animated Firefly TMAP")
        });
        let firefly_bank = FireflyTmapBank::decode_qualified([
            &firefly_payloads[0],
            &firefly_payloads[1],
            &firefly_payloads[2],
            &firefly_payloads[3],
            &firefly_payloads[4],
            &firefly_payloads[5],
            &firefly_payloads[6],
            &firefly_payloads[7],
        ])
        .expect("qualified Firefly TMAP bank");
        let firefly_clocks = FireflyTmapClocks::from_bank(&firefly_bank);
        let fireball_payload = std::fs::read(cache.join("terrain/tmaps1/463.decoded.bin"))
            .expect("qualified animated Fireball TMAP");
        let possession_payload = std::fs::read(cache.join("terrain/tmaps1/144.decoded.bin"))
            .expect("qualified animated Possession TMAP");
        let combat_static = crate::combat_tmap::COMBAT_STATIC_SELECTORS.map(|selector| {
            std::fs::read(cache.join(format!("terrain/tmaps1/{selector:03}.indices.u8")))
                .expect("qualified static combat TMAP")
        });
        let combat_bank = CombatTmapBank::decode_qualified(
            &fireball_payload,
            &possession_payload,
            combat_static.each_ref().map(Vec::as_slice),
        )
        .expect("qualified combat TMAP bank");
        let fireball_clock = FireballTmapClock::from_bank(&combat_bank);
        let possession_clock = PossessionTmapClock::from_bank(&combat_bank);
        let static_world_bank = StaticWorldTmapBank::decode_qualified(
            &std::fs::read(cache.join("terrain/tmaps1/059.decoded.bin"))
                .expect("qualified spell-jar TMAP"),
            &std::fs::read(cache.join("terrain/tmaps1/461.decoded.bin"))
                .expect("qualified exit-target TMAP"),
        )
        .expect("qualified static world TMAP bank");
        let economy_bank = EconomyTmapBank::decode_qualified(
            &std::fs::read(cache.join("terrain/tmaps1/058.decoded.bin"))
                .expect("qualified neutral mana TMAP"),
            &std::fs::read(cache.join("terrain/tmaps1/067.decoded.bin"))
                .expect("qualified owned mana TMAP"),
            &std::fs::read(cache.join("terrain/tmaps1/088.decoded.bin"))
                .expect("qualified mana-carrier TMAP"),
            &std::fs::read(cache.join("terrain/tmaps1/096.decoded.bin"))
                .expect("qualified castle-root TMAP"),
        )
        .expect("qualified economy TMAP bank");
        let castle_clock = CastleTmapClock::from_bank(&economy_bank);
        let dolmen_payload = std::fs::read(cache.join("terrain/tmaps1/079.decoded.bin"))
            .expect("qualified dolmen TMAP");
        let villager_payloads = VILLAGER_ANIMATED_SELECTORS.map(|selector| {
            std::fs::read(cache.join(format!("terrain/tmaps1/{selector:03}.decoded.bin")))
                .expect("qualified animated villager TMAP")
        });
        let frozen_actor_bank = FrozenActorTmapBank::decode_qualified(
            &dolmen_payload,
            std::array::from_fn(|ordinal| villager_payloads[ordinal].as_slice()),
        )
        .expect("qualified frozen actor TMAP bank");
        let villager_clocks = VillagerTmapClocks::from_bank(&frozen_actor_bank);
        let seeds = world
            .reset_live_world()
            .goats()
            .iter()
            .map(|goat| crate::renderable_map_chain::RenderableMapSeed {
                entity_index: goat.entity_index,
                active: goat.active,
                x: goat.position.x,
                y: goat.position.y,
            })
            .collect::<Vec<_>>();
        let map_chains =
            RenderableMapChains::from_reset(&seeds, &planes.entity_indices, |entity_index| {
                world
                    .entities()
                    .get(usize::from(entity_index))
                    .map(crate::native_world::EntitySnapshot::old_map_entity)
            })
            .expect("qualified shared renderable map chains");
        let player_life = world.entities()[usize::from(world.reset_state().entity_index)].life;
        let mission = world.level1_mission().session(player_life, 1_000);
        let frozen_actors = LEVEL1_FROZEN_ACTOR_ENTITY_INDICES.map(|source_entity_index| {
            let entity = &world.entities()[usize::from(source_entity_index)];
            Level1FrozenActorRender {
                source_entity_index,
                position: entity.position,
                yaw: entity.yaw,
                render_parameter_index: entity
                    .particle_parameter_index
                    .expect("qualified frozen actor render parameter"),
            }
        });
        let frozen_seeds =
            frozen_actors.map(|actor| crate::renderable_map_chain::RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: true,
                x: actor.position.x,
                y: actor.position.y,
            });
        let frozen_map_chains = RenderableMapChains::from_reset(
            &frozen_seeds,
            &planes.entity_indices,
            |entity_index| {
                world
                    .entities()
                    .get(usize::from(entity_index))
                    .map(crate::native_world::EntitySnapshot::old_map_entity)
            },
        )
        .expect("qualified frozen-actor map chains");
        let frozen_subject = frozen_actors[0];
        let mut visible_frozen = None;
        for yaw in (0_i16..2_048_i16).step_by(256) {
            let frozen_camera = Level1TerrainFrameCamera {
                render_x: frozen_subject.position.x.wrapping_sub(1_024),
                render_y: frozen_subject.position.y,
                eye_z: i32::from(frozen_subject.position.z) + 256,
                yaw,
                pitch: 0,
                roll: 0,
                fov: 128,
                source_turn: 0,
                reflections: true,
            };
            let frame = render_level1_world_frame(
                frozen_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: &[],
                    frozen_actors: &frozen_actors,
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: Some(&frozen_actor_bank),
                    villager_tmap_clocks: Some(&villager_clocks),
                    mission_actors: &[],
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &frozen_map_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("frozen-actor-composited world frame");
            if frame.frozen_actor_raster_stats.framebuffer_writes > 0
                && frame.frozen_actor_selector_mask != 0
            {
                visible_frozen = Some((frozen_camera, frame));
                break;
            }
        }
        let (frozen_camera, frozen_frame) =
            visible_frozen.expect("a cardinal camera must rasterize frozen villager 51");
        let repeated_frozen = render_level1_world_frame(
            frozen_camera,
            render_resources(),
            Level1WorldFrameResources {
                local_player_entity_index: world.reset_state().entity_index,
                goats: &[],
                frozen_actors: &frozen_actors,
                ambient_entities: &[],
                frozen_actor_tmap_bank: Some(&frozen_actor_bank),
                villager_tmap_clocks: Some(&villager_clocks),
                mission_actors: &[],
                dynamic_residents: &[],
                mission_segments: &[],
                castle_root: None,
                mana_spheres: &[],
                mana_carrier: None,
                map_chains: &frozen_map_chains,
                goat_tmap_clocks: &clocks,
                archer_tmap_bank: &archer_bank,
                archer_tmap_clocks: &archer_clocks,
                firefly_tmap_bank: &firefly_bank,
                firefly_tmap_clocks: &firefly_clocks,
                projectiles: &[],
                impacts: &[],
                possessed_buildings: &[],
                combat_tmap_bank: &combat_bank,
                static_world_tmap_bank: &static_world_bank,
                economy_tmap_bank: &economy_bank,
                fireball_tmap_clock: &fireball_clock,
                possession_tmap_clock: &possession_clock,
                castle_tmap_clock: &castle_clock,
                particle_parameters: &world.presentation().particles,
                source_slots: mission.source_slots(),
                night: true,
            },
        )
        .expect("repeated frozen-actor-composited world frame");
        assert_eq!(repeated_frozen, frozen_frame);
        assert!(
            frozen_frame
                .reflected_sprite_raster_stats
                .framebuffer_writes
                > 0
        );
        assert!(frozen_frame.shadow_sprite_raster_stats.framebuffer_writes > 0);
        let render_world = || {
            render_level1_world_frame(
                camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: &[],
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &map_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("Goat-composited world frame")
        };
        let first = render_world();
        let repeated = render_world();
        assert_eq!(first, repeated);
        assert!(first.goats_composited);
        assert_ne!(first.goat_selector_mask, 0);
        assert!(first.goat_raster_stats.framebuffer_writes > 0);
        assert!(first.reflected_sprites_composited);
        // This camera puts Goat 50 behind intervening terrain. The Goat pass
        // writes pixels at its owner cell, and the later near cells overwrite
        // all of them. This is the source painter behavior that a Unity
        // billboard/depth-less terrain composite could not reproduce.
        assert_eq!(first.indices, terrain.indices);
        assert_eq!(first.terrain_coverage, terrain.terrain_coverage);
        assert_eq!(first.coverage_hash, terrain.coverage_hash);

        let close_camera = Level1TerrainFrameCamera {
            render_y: goat.position.y.wrapping_add(300),
            eye_z: i32::from(goat.position.z),
            ..camera
        };
        let close_terrain = render_level1_terrain_frame(close_camera, render_resources())
            .expect("close isolated terrain frame");
        let close_world = render_level1_world_frame(
            close_camera,
            render_resources(),
            Level1WorldFrameResources {
                local_player_entity_index: world.reset_state().entity_index,
                goats: world.reset_live_world().goats(),
                frozen_actors: &[],
                ambient_entities: &[],
                frozen_actor_tmap_bank: None,
                villager_tmap_clocks: None,
                mission_actors: &[],
                dynamic_residents: &[],
                mission_segments: &[],
                castle_root: None,
                mana_spheres: &[],
                mana_carrier: None,
                map_chains: &map_chains,
                goat_tmap_clocks: &clocks,
                archer_tmap_bank: &archer_bank,
                archer_tmap_clocks: &archer_clocks,
                firefly_tmap_bank: &firefly_bank,
                firefly_tmap_clocks: &firefly_clocks,
                projectiles: &[],
                impacts: &[],
                possessed_buildings: &[],
                combat_tmap_bank: &combat_bank,
                static_world_tmap_bank: &static_world_bank,
                economy_tmap_bank: &economy_bank,
                fireball_tmap_clock: &fireball_clock,
                possession_tmap_clock: &possession_clock,
                castle_tmap_clock: &castle_clock,
                particle_parameters: &world.presentation().particles,
                source_slots: mission.source_slots(),
                night: true,
            },
        )
        .expect("close Goat-composited world frame");
        assert!(close_world.goat_raster_stats.framebuffer_writes > 0);
        assert!(
            close_world.shadow_sprite_raster_stats.framebuffer_writes > 0,
            "night TABLESN ground shadow writes immediately before the Goat"
        );
        assert_ne!(close_world.indices, close_terrain.indices);
        assert_eq!(close_world.terrain_coverage, close_terrain.terrain_coverage);

        let mut archer = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Archer)
            .copied()
            .expect("qualified Level 1 Archer");
        archer.source_entity_index = 999;
        archer.active = true;
        archer.visible = true;
        archer.life = archer.max_life;
        archer.position = goat.position;
        archer.yaw = 0;
        let mut archer_chains = map_chains.clone();
        archer_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: archer.source_entity_index,
                active: true,
                x: archer.position.x,
                y: archer.position.y,
            })
            .expect("synthetic Archer head insertion");
        let archer_camera = Level1TerrainFrameCamera {
            render_x: archer.position.x,
            render_y: archer.position.y.wrapping_add(300),
            eye_z: i32::from(archer.position.z),
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        let mut pose_hashes = Vec::new();
        for (render_index, base_tmap, logical_height, expected_animated_mask) in [
            (
                ARCHER_RENDER_INDEX,
                ARCHER_BASE_TMAP,
                ARCHER_LOGICAL_HEIGHT,
                1,
            ),
            (
                ARCHER_ATTACK_RENDER_INDEX_A,
                ARCHER_ATTACK_BASE_TMAP_A,
                ARCHER_LOGICAL_HEIGHT,
                0,
            ),
            (
                ARCHER_ATTACK_RENDER_INDEX_B,
                ARCHER_ATTACK_BASE_TMAP_B,
                ARCHER_ATTACK_LOGICAL_HEIGHT_B,
                0,
            ),
        ] {
            archer.render_parameter_index = Some(render_index);
            archer.base_tmap = Some(base_tmap);
            archer.render_mode = Some(ARCHER_RENDER_MODE);
            archer.logical_height = Some(logical_height);
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&archer),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &archer_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("Archer-composited source world frame");
            assert!(frame.archers_composited);
            assert!(frame.archer_raster_stats.framebuffer_writes > 0);
            assert_eq!(frame.archer_selector_mask, expected_animated_mask);
            pose_hashes.push(frame.index_hash);
        }
        pose_hashes.sort_unstable();
        pose_hashes.dedup();
        assert_eq!(
            pose_hashes.len(),
            3,
            "all Archer poses alter the indexed frame"
        );

        let mut resident_mission = mission.clone();
        let resident_source_slot = resident_mission
            .spawn_house_resident(goat.position, 4, 33, 0)
            .expect("qualified house Archer constructor")
            .expect("qualified source arena has a resident slot");
        let mut resident = resident_mission
            .dynamic_residents()
            .copied()
            .find(|resident| resident.source_entity_index == resident_source_slot)
            .expect("constructed house Archer is registered");
        let mut resident_chains = map_chains.clone();
        resident_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })
            .expect("house Archer source-map insertion");
        let mut resident_pose_hashes = Vec::new();
        for (render_index, base_tmap, logical_height) in [
            (ARCHER_RENDER_INDEX, ARCHER_BASE_TMAP, ARCHER_LOGICAL_HEIGHT),
            (
                ARCHER_ATTACK_RENDER_INDEX_A,
                ARCHER_ATTACK_BASE_TMAP_A,
                ARCHER_LOGICAL_HEIGHT,
            ),
            (
                ARCHER_ATTACK_RENDER_INDEX_B,
                ARCHER_ATTACK_BASE_TMAP_B,
                ARCHER_ATTACK_LOGICAL_HEIGHT_B,
            ),
        ] {
            resident.render_parameter_index = render_index;
            resident.base_tmap = base_tmap;
            resident.logical_height = logical_height;
            let resident_frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: Some(&frozen_actor_bank),
                    villager_tmap_clocks: Some(&villager_clocks),
                    mission_actors: &[],
                    dynamic_residents: std::slice::from_ref(&resident),
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &resident_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: resident_mission.source_slots(),
                    night: true,
                },
            )
            .expect("every qualified house Archer pose reaches the source compositor");
            assert!(resident_frame.archers_composited);
            assert!(resident_frame.archer_raster_stats.framebuffer_writes > 0);
            resident_pose_hashes.push(resident_frame.index_hash);
        }
        resident_pose_hashes.sort_unstable();
        resident_pose_hashes.dedup();
        assert_eq!(
            resident_pose_hashes.len(),
            3,
            "all house Archer poses alter the indexed frame"
        );

        let mut firefly = mission
            .actors()
            .iter()
            .find(|actor| actor.kind == Level1ActorKind::Firefly)
            .copied()
            .expect("qualified Level 1 Firefly");
        firefly.source_entity_index = 998;
        firefly.active = true;
        firefly.visible = true;
        firefly.life = firefly.max_life;
        firefly.position = goat.position;
        let mut firefly_chains = map_chains.clone();
        firefly_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: firefly.source_entity_index,
                active: true,
                x: firefly.position.x,
                y: firefly.position.y,
            })
            .expect("synthetic Firefly head insertion");
        for sector in 0..16 {
            firefly.yaw = i16::try_from(sector << 7).expect("Firefly sector yaw fits i16");
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&firefly),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &firefly_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("Firefly-composited source world frame");
            let selector_offset = if sector < 8 { sector } else { 15 - sector };
            assert!(frame.fireflies_composited);
            assert!(frame.firefly_raster_stats.framebuffer_writes > 0);
            assert_eq!(frame.firefly_selector_mask, 1 << selector_offset);
        }

        for (ordinal, kind) in [
            Level1ActorKind::CastleSpellJar,
            Level1ActorKind::SpeedSpellJar,
            Level1ActorKind::ExitTarget,
        ]
        .into_iter()
        .enumerate()
        {
            let mut prop = mission
                .actors()
                .iter()
                .find(|actor| actor.kind == kind)
                .copied()
                .expect("qualified Level 1 objective prop");
            prop.source_entity_index = 997 - u16::try_from(ordinal).expect("bounded prop ordinal");
            prop.active = true;
            prop.visible = true;
            prop.collected = false;
            prop.position = goat.position;
            let mut prop_chains = map_chains.clone();
            prop_chains
                .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                    entity_index: prop.source_entity_index,
                    active: true,
                    x: prop.position.x,
                    y: prop.position.y,
                })
                .expect("synthetic objective-prop head insertion");
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&prop),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &prop_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("fixed objective prop composites");
            assert!(frame.objective_props_composited);
            assert!(
                frame.objective_prop_raster_stats.framebuffer_writes > 0,
                "{kind:?} writes indexed pixels"
            );

            prop.visible = false;
            let hidden = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&prop),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &prop_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("hidden objective prop remains chain-safe");
            assert_eq!(
                hidden.objective_prop_raster_stats,
                Mode20SpriteRasterStats::default()
            );

            prop.visible = true;
            prop.render_mode = Some(99);
            let invalid = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: world.reset_state().entity_index,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&prop),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &prop_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            );
            assert!(
                matches!(invalid, Err(TerrainFrameError(message)) if message.contains("unsupported render tuple"))
            );
        }

        let local_player = world.reset_state().entity_index;
        let castle = Level1CastleRootRender {
            source_entity_index: 994,
            position: goat.position,
            active: true,
        };
        let neutral_sphere = ManaSphere {
            id: 1,
            source_entity_index: 993,
            entity_rng: 0,
            position: goat.position,
            velocity_x: 0,
            velocity_y: 0,
            vertical_velocity: 0,
            yaw: 0,
            mana: 256,
            owner_entity_index: 0,
            player_entity_index: 0,
            incoming_owner_entity_index: 0,
            incoming_possession_payload: 0,
            attraction_source_entity_index: 0,
            attraction_step: 0,
            attraction_distance_high: 0,
            assigned_carrier_id: 0,
            assigned_defender_slot: 0,
            control_flags: 0,
            phase: 0,
            active: true,
            pending_delete: false,
        };
        let carrier = ManaCarrier {
            id: 992,
            source_entity_index: 992,
            entity_rng: 0,
            owner_entity_index: local_player,
            position: goat.position,
            yaw: 0,
            pitch: 0,
            speed: 48,
            life: 10_000,
            max_life: 10_000,
            max_mana: 10_000,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: true,
        };
        for (source_entity_index, castle_root, spheres, mana_carrier, expected_castle_latch) in [
            (
                castle.source_entity_index,
                Some(castle),
                &[][..],
                None,
                true,
            ),
            (
                neutral_sphere.source_entity_index,
                None,
                std::slice::from_ref(&neutral_sphere),
                None,
                false,
            ),
            (
                carrier.source_entity_index,
                None,
                &[][..],
                Some(&carrier),
                false,
            ),
        ] {
            let mut economy_chains = map_chains.clone();
            economy_chains
                .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                    entity_index: source_entity_index,
                    active: true,
                    x: goat.position.x,
                    y: goat.position.y,
                })
                .expect("synthetic economy head insertion");
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: local_player,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: &[],
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root,
                    mana_spheres: spheres,
                    mana_carrier,
                    map_chains: &economy_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: mission.source_slots(),
                    night: true,
                },
            )
            .expect("castle/mana economy composites");
            assert!(frame.economy_composited);
            assert!(frame.economy_raster_stats.framebuffer_writes > 0);
            assert_eq!(frame.castle_draw_attempted, expected_castle_latch);
        }

        let mut marker_chains = archer_chains.clone();
        marker_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: 991,
                active: true,
                x: archer.position.x,
                y: archer.position.y,
            })
            .expect("dedicated Possession marker insertion");
        let mut marker_source_slots = mission.source_slots().to_vec();
        marker_source_slots[991].control_flags = 0;
        let mut invisible_impact_hashes = Vec::new();
        for model in [
            None,
            Some(POSSESSION_IMPACT_MODEL),
            Some(crate::level1_mission::POSSESSION_ATTRACTION_MODEL),
            Some(POSSESSION_STRONG_IMPACT_MODEL),
        ] {
            if let Some(model) = model {
                assert!(
                    !impact_is_presentational(model),
                    "models 12, 54, and 70 carry gameplay state but the source renderer hides them"
                );
                if matches!(
                    model,
                    POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
                ) {
                    assert_eq!(
                        impact_presentation_discriminants(model),
                        (true, false),
                        "models 12/70 remain possession, never fireball, even though drawing is suppressed"
                    );
                }
            }
            let marker = model.map(|model| FireballImpact {
                id: u32::from(model),
                source_entity_index: 991,
                source_projectile_id: 998,
                owner_entity_index: local_player,
                external_target_entity_index: 0,
                model,
                position: goat.position,
                payload: i32::from(model == POSSESSION_STRONG_IMPACT_MODEL),
                life_ticks: 8,
                entity_rng: 0,
                vertical_velocity: 0,
                phase: if model == POSSESSION_IMPACT_MODEL {
                    1
                } else {
                    200
                },
                animation_frame: 1,
                damage_submitted: false,
                active: true,
            });
            let impact_chains = if marker.is_some() {
                &marker_chains
            } else {
                &archer_chains
            };
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: local_player,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&archer),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: impact_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: marker.as_slice(),
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: &marker_source_slots,
                    night: true,
                },
            )
            .expect("source-hidden class-10 gameplay impact");
            assert!(!frame.possession_draw_attempted);
            assert!(!frame.fireball_draw_attempted);
            invisible_impact_hashes.push(byte_hash(&frame.indices));
        }
        assert!(
            invisible_impact_hashes
                .iter()
                .all(|hash| *hash == invisible_impact_hashes[0]),
            "models 12, 54, and 70 must not alter the native terrain framebuffer"
        );

        assert_eq!(
            (0_u32..=7)
                .map(ground_fire_presentation)
                .collect::<Vec<_>>(),
            [
                (228, 50),
                (229, 150),
                (230, 300),
                (231, 450),
                (232, 600),
                (233, 700),
                (234, 800),
                (235, 950),
            ],
            "sub_31760's phase selects exact parameter rows 228..235"
        );
        let mut ground_fire_hashes = Vec::new();
        for phase in [0, 7] {
            let marker = FireballImpact {
                id: 6,
                source_entity_index: 991,
                source_projectile_id: 998,
                owner_entity_index: local_player,
                external_target_entity_index: 0,
                model: GROUND_FIRE_MODEL,
                position: goat.position,
                payload: 50,
                life_ticks: 130,
                entity_rng: 0,
                vertical_velocity: 0,
                phase,
                animation_frame: 0,
                damage_submitted: false,
                active: true,
            };
            let frame = render_level1_world_frame(
                archer_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: local_player,
                    goats: world.reset_live_world().goats(),
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: std::slice::from_ref(&archer),
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &marker_chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: std::slice::from_ref(&marker),
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: &marker_source_slots,
                    night: true,
                },
            )
            .expect("phase-driven ground-fire presentation");
            assert!(frame.fireball_draw_attempted);
            assert!(!frame.possession_draw_attempted);
            ground_fire_hashes.push(byte_hash(&frame.indices));
        }
        assert_ne!(
            ground_fire_hashes[0], ground_fire_hashes[1],
            "fixed selector 77 scales differently for source phases 0 and 7"
        );

        let hostile_sphere = ManaSphere {
            owner_entity_index: local_player.wrapping_add(1),
            ..neutral_sphere
        };
        let mut hostile_chains = map_chains.clone();
        hostile_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: hostile_sphere.source_entity_index,
                active: true,
                x: hostile_sphere.position.x,
                y: hostile_sphere.position.y,
            })
            .expect("synthetic unsupported-color sphere insertion");
        let unsupported_color = render_level1_world_frame(
            archer_camera,
            render_resources(),
            Level1WorldFrameResources {
                local_player_entity_index: local_player,
                goats: world.reset_live_world().goats(),
                frozen_actors: &[],
                ambient_entities: &[],
                frozen_actor_tmap_bank: None,
                villager_tmap_clocks: None,
                mission_actors: &[],
                dynamic_residents: &[],
                mission_segments: &[],
                castle_root: None,
                mana_spheres: std::slice::from_ref(&hostile_sphere),
                mana_carrier: None,
                map_chains: &hostile_chains,
                goat_tmap_clocks: &clocks,
                archer_tmap_bank: &archer_bank,
                archer_tmap_clocks: &archer_clocks,
                firefly_tmap_bank: &firefly_bank,
                firefly_tmap_clocks: &firefly_clocks,
                projectiles: &[],
                impacts: &[],
                possessed_buildings: &[],
                combat_tmap_bank: &combat_bank,
                static_world_tmap_bank: &static_world_bank,
                economy_tmap_bank: &economy_bank,
                fireball_tmap_clock: &fireball_clock,
                possession_tmap_clock: &possession_clock,
                castle_tmap_clock: &castle_clock,
                particle_parameters: &world.presentation().particles,
                source_slots: mission.source_slots(),
                night: true,
            },
        );
        assert!(
            matches!(unsupported_color, Err(TerrainFrameError(message)) if message.contains("no qualified Level 1 color selector"))
        );

        use crate::source_sprite_tmap::{
            CASTLE_DEFENSE_SHOT_PROFILES, SourceSpriteAnimationOwner, SourceSpriteExclusionReason,
            SourceSpriteLegacyFamily, SourceSpritePaletteOwner, SourceSpriteTemplate,
            SourceSpriteTmapBank, SourceSpriteTmapFrame,
        };
        let source_frame = |selector: u16, color: u8| SourceSpriteTmapFrame {
            selector,
            width: 16,
            height: 16,
            pixels: vec![color; 16 * 16].into_boxed_slice(),
        };
        let patterned_source_frame =
            |selector: u16, width: u16, height: u16| SourceSpriteTmapFrame {
                selector,
                width,
                height,
                pixels: (0..usize::from(width) * usize::from(height))
                    .map(|index| {
                        u8::try_from(index % 31)
                            .expect("bounded synthetic source index")
                            .wrapping_add(1)
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            };
        let source_bank = SourceSpriteTmapBank::new(vec![
            source_frame(54, 17),
            source_frame(64, 19),
            source_frame(80, 21),
            source_frame(81, 23),
            source_frame(138, 25),
            source_frame(139, 25),
            source_frame(140, 25),
            source_frame(141, 27),
            source_frame(142, 27),
            source_frame(143, 27),
            source_frame(151, 29),
            source_frame(463, 31),
            patterned_source_frame(300, 15, 16),
            patterned_source_frame(301, 16, 15),
            patterned_source_frame(310, 15, 16),
            patterned_source_frame(311, 15, 16),
            patterned_source_frame(312, 15, 16),
        ])
        .expect("synthetic exact-selector CP2 TMAP bank");
        let source_template = |parameter, selector, mode, height| SourceSpriteTemplate {
            render_parameter_index: parameter,
            base_tmap: selector,
            render_mode: mode,
            logical_height: height,
            animation_owner: SourceSpriteAnimationOwner::SourceEntity,
            palette_owner: SourceSpritePaletteOwner::NightLevel,
            shadow_eligible: true,
            reflection_eligible: true,
        };
        let make_submission =
            |slot, position, class, model, action, parameter, selector, mode, height| {
                SourceSpriteSubmission::from_template(
                    slot,
                    position,
                    0,
                    0,
                    0,
                    0,
                    class,
                    model,
                    action,
                    0,
                    0,
                    source_template(parameter, selector, mode, height),
                    selector,
                )
            };
        let source_camera = Level1TerrainFrameCamera {
            reflections: false,
            ..archer_camera
        };
        let source_terrain = render_level1_terrain_frame(source_camera, render_resources())
            .expect("CP2 source-compositor terrain baseline");
        let mut source_slots = mission.source_slots().to_vec();
        let mut family_submissions = vec![
            (
                "local-player",
                make_submission(980, goat.position, 3, 0, 0, 66, 80, 0, 200),
            ),
            (
                "death-burst",
                make_submission(980, goat.position, 10, 1, 1, 41, 54, 21, 250),
            ),
            (
                "death-marker",
                make_submission(980, goat.position, 10, 40, 42, 65, 81, 0, 200),
            ),
            (
                "castle-association",
                make_submission(980, goat.position, 10, 43, 45, 41, 54, 21, 250),
            ),
            (
                "castle-defense-controller",
                make_submission(980, goat.position, 10, 79, 86, 66, 80, 0, 200),
            ),
            (
                "resident-builder",
                make_submission(980, goat.position, 5, 12, 97, 221, 141, 20, 250),
            ),
            (
                "resident-trader",
                make_submission(980, goat.position, 5, 14, 113, 219, 138, 20, 250),
            ),
        ];
        family_submissions.extend(
            CASTLE_DEFENSE_SHOT_PROFILES
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, profile)| {
                    (
                        match ordinal {
                            0 => "castle-shot-fireball",
                            1 => "castle-shot-arrow",
                            2 => "castle-shot-arrow-return",
                            3 => "castle-shot-fireball-alternate",
                            4 => "castle-shot-lightning",
                            _ => unreachable!(),
                        },
                        profile.submission(980, goat.position, 0, 0, 0, 0),
                    )
                }),
        );
        let mut family_hashes = Vec::new();
        let lighting_hash_before_source_render = mission.dynamic_lighting_hash_v1();
        for (ordinal, (family, mut submission)) in family_submissions.into_iter().enumerate() {
            submission.position.x = submission
                .position
                .x
                .wrapping_add(u16::try_from(ordinal * 7).unwrap());
            source_slots[980] = crate::level1_mission::Level1ResetSourceSlot {
                index: 980,
                active: true,
                class: submission.class,
                model: submission.model,
                action: submission.action,
                phase: 0,
                entity_rng: 0,
                control_flags: 0,
            };
            let mut source_chain = RenderableMapChains::empty();
            source_chain
                .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                    entity_index: 980,
                    active: true,
                    x: submission.position.x,
                    y: submission.position.y,
                })
                .unwrap();
            let frame = render_level1_source_world_frame(
                source_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: local_player,
                    goats: &[],
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: &[],
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &source_chain,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: &source_slots,
                    night: true,
                },
                SourceSpriteWorldResources {
                    dispositions: &[SourceSpriteDisposition::Render(submission)],
                    tmap_bank: &source_bank,
                },
            )
            .unwrap_or_else(|error| panic!("{family}: {error}"));
            assert!(
                frame.combat_raster_stats.framebuffer_writes > 0,
                "{family} must reach the production indexed compositor"
            );
            assert_ne!(
                frame.indices, source_terrain.indices,
                "{family} must write nonzero production pixels"
            );
            family_hashes.push((family, frame.index_hash));
        }
        assert_eq!(family_hashes.len(), 12);
        assert_eq!(
            family_hashes
                .iter()
                .map(|(_, hash)| *hash)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            family_hashes.len(),
            "every supported family must produce a distinguishing indexed framebuffer"
        );

        let overlap_position = goat.position;
        let front = make_submission(980, overlap_position, 3, 0, 0, 66, 80, 0, 200);
        let back = make_submission(982, overlap_position, 10, 40, 42, 65, 81, 0, 200);
        source_slots[980] = crate::level1_mission::Level1ResetSourceSlot {
            index: 980,
            active: true,
            class: front.class,
            model: front.model,
            action: front.action,
            phase: 0,
            entity_rng: 0,
            control_flags: 0,
        };
        source_slots[981] = crate::level1_mission::Level1ResetSourceSlot {
            index: 981,
            active: true,
            class: 11,
            model: 0,
            action: 0,
            phase: 0,
            entity_rng: 0,
            control_flags: 0,
        };
        source_slots[982] = crate::level1_mission::Level1ResetSourceSlot {
            index: 982,
            active: true,
            class: back.class,
            model: back.model,
            action: back.action,
            phase: 0,
            entity_rng: 0,
            control_flags: 0,
        };
        let order_dispositions = [
            SourceSpriteDisposition::Render(front),
            SourceSpriteDisposition::Excluded {
                source_entity_index: 981,
                reason: SourceSpriteExclusionReason::SpriteLessSwitch,
            },
            SourceSpriteDisposition::Render(back),
        ];
        let neighbour_dispositions = [order_dispositions[0], order_dispositions[2]];
        let render_order_case = |insertion_order: &[u16], include_excluded: bool| {
            let mut chains = RenderableMapChains::empty();
            for entity_index in insertion_order {
                chains
                    .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: *entity_index,
                        active: true,
                        x: overlap_position.x,
                        y: overlap_position.y,
                    })
                    .unwrap();
            }
            render_level1_source_world_frame(
                source_camera,
                render_resources(),
                Level1WorldFrameResources {
                    local_player_entity_index: local_player,
                    goats: &[],
                    frozen_actors: &[],
                    ambient_entities: &[],
                    frozen_actor_tmap_bank: None,
                    villager_tmap_clocks: None,
                    mission_actors: &[],
                    dynamic_residents: &[],
                    mission_segments: &[],
                    castle_root: None,
                    mana_spheres: &[],
                    mana_carrier: None,
                    map_chains: &chains,
                    goat_tmap_clocks: &clocks,
                    archer_tmap_bank: &archer_bank,
                    archer_tmap_clocks: &archer_clocks,
                    firefly_tmap_bank: &firefly_bank,
                    firefly_tmap_clocks: &firefly_clocks,
                    projectiles: &[],
                    impacts: &[],
                    possessed_buildings: &[],
                    combat_tmap_bank: &combat_bank,
                    static_world_tmap_bank: &static_world_bank,
                    economy_tmap_bank: &economy_bank,
                    fireball_tmap_clock: &fireball_clock,
                    possession_tmap_clock: &possession_clock,
                    castle_tmap_clock: &castle_clock,
                    particle_parameters: &world.presentation().particles,
                    source_slots: &source_slots,
                    night: true,
                },
                SourceSpriteWorldResources {
                    dispositions: if include_excluded {
                        &order_dispositions
                    } else {
                        &neighbour_dispositions
                    },
                    tmap_bank: &source_bank,
                },
            )
            .expect("typed source-order overlap frame")
        };
        let excluded_middle = render_order_case(&[980, 981, 982], true);
        let same_neighbours = render_order_case(&[980, 982], false);
        let reversed_neighbours = render_order_case(&[982, 980], false);
        assert_eq!(
            excluded_middle.indices, same_neighbours.indices,
            "an excluded middle raw node cannot reorder its renderable neighbours"
        );
        assert_ne!(
            excluded_middle.index_hash, reversed_neighbours.index_hash,
            "overlapping indexed pixels, not a trace alone, discriminate source chain order"
        );
        assert_eq!(
            (
                excluded_middle.index_hash,
                same_neighbours.index_hash,
                reversed_neighbours.index_hash,
                excluded_middle
                    .shadow_sprite_raster_stats
                    .framebuffer_writes,
            ),
            (
                0xd680_f6a6_6f8c_4d56,
                0xd680_f6a6_6f8c_4d56,
                0x32d2_dd45_0a14_76aa,
                5_886,
            ),
            "raw-chain neighbour order and immediately-before-primary shadow writes are framebuffer-pinned"
        );

        let mut unknown_chain = RenderableMapChains::empty();
        unknown_chain
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: 981,
                active: true,
                x: overlap_position.x,
                y: overlap_position.y,
            })
            .unwrap();
        let unknown = render_level1_source_world_frame(
            source_camera,
            render_resources(),
            Level1WorldFrameResources {
                local_player_entity_index: local_player,
                goats: &[],
                frozen_actors: &[],
                ambient_entities: &[],
                frozen_actor_tmap_bank: None,
                villager_tmap_clocks: None,
                mission_actors: &[],
                dynamic_residents: &[],
                mission_segments: &[],
                castle_root: None,
                mana_spheres: &[],
                mana_carrier: None,
                map_chains: &unknown_chain,
                goat_tmap_clocks: &clocks,
                archer_tmap_bank: &archer_bank,
                archer_tmap_clocks: &archer_clocks,
                firefly_tmap_bank: &firefly_bank,
                firefly_tmap_clocks: &firefly_clocks,
                projectiles: &[],
                impacts: &[],
                possessed_buildings: &[],
                combat_tmap_bank: &combat_bank,
                static_world_tmap_bank: &static_world_bank,
                economy_tmap_bank: &economy_bank,
                fireball_tmap_clock: &fireball_clock,
                possession_tmap_clock: &possession_clock,
                castle_tmap_clock: &castle_clock,
                particle_parameters: &world.presentation().particles,
                source_slots: &source_slots,
                night: true,
            },
            SourceSpriteWorldResources {
                dispositions: &[],
                tmap_bank: &source_bank,
            },
        );
        assert!(
            matches!(unknown, Err(TerrainFrameError(message)) if message.contains("slot=981 class=11 model=0 action=0"))
        );

        let source_schedule = primary_owner_schedule(&outdoor_grid(OutdoorGridInput {
            render_x: source_camera.render_x,
            render_y: source_camera.render_y,
            yaw: source_camera.yaw,
            turn: source_camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        }));
        assert_eq!(source_schedule.len(), 20 * 39);
        for (chunk, expected_row) in source_schedule.chunks_exact(39).zip((1..=20).rev()) {
            assert!(
                chunk
                    .iter()
                    .all(|owner| *owner >= expected_row * 40 && *owner < (expected_row + 1) * 40),
                "source raw-chain traversal must complete row {expected_row}, including its seam, before the next row"
            );
        }

        let render_source_vector =
            |camera: Level1TerrainFrameCamera, submission: SourceSpriteSubmission| {
                let mut vector_slots = mission.source_slots().to_vec();
                vector_slots[usize::from(submission.source_entity_index)] =
                    crate::level1_mission::Level1ResetSourceSlot {
                        index: submission.source_entity_index,
                        active: true,
                        class: submission.class,
                        model: submission.model,
                        action: submission.action,
                        phase: 0,
                        entity_rng: 0,
                        control_flags: submission.control_flags,
                    };
                let mut vector_chain = RenderableMapChains::empty();
                vector_chain
                    .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: submission.source_entity_index,
                        active: true,
                        x: submission.position.x,
                        y: submission.position.y,
                    })
                    .expect("synthetic hotspot source-chain insertion");
                render_level1_source_world_frame(
                    camera,
                    render_resources(),
                    Level1WorldFrameResources {
                        local_player_entity_index: local_player,
                        goats: &[],
                        frozen_actors: &[],
                        ambient_entities: &[],
                        frozen_actor_tmap_bank: None,
                        villager_tmap_clocks: None,
                        mission_actors: &[],
                        dynamic_residents: &[],
                        mission_segments: &[],
                        castle_root: None,
                        mana_spheres: &[],
                        mana_carrier: None,
                        map_chains: &vector_chain,
                        goat_tmap_clocks: &clocks,
                        archer_tmap_bank: &archer_bank,
                        archer_tmap_clocks: &archer_clocks,
                        firefly_tmap_bank: &firefly_bank,
                        firefly_tmap_clocks: &firefly_clocks,
                        projectiles: &[],
                        impacts: &[],
                        possessed_buildings: &[],
                        combat_tmap_bank: &combat_bank,
                        static_world_tmap_bank: &static_world_bank,
                        economy_tmap_bank: &economy_bank,
                        fireball_tmap_clock: &fireball_clock,
                        possession_tmap_clock: &possession_clock,
                        castle_tmap_clock: &castle_clock,
                        particle_parameters: &world.presentation().particles,
                        source_slots: &vector_slots,
                        night: true,
                    },
                    SourceSpriteWorldResources {
                        dispositions: &[SourceSpriteDisposition::Render(submission)],
                        tmap_bank: &source_bank,
                    },
                )
                .expect("qualified CP2 hotspot vector")
            };
        if source_cache.is_some() {
            let mut production_session = world.session();
            let production_player = production_session.state;
            assert_ne!(
                production_session
                    .mission
                    .player_source_control_flags(production_player.entity_index)
                    & 0x21,
                0,
                "reset self-view is source-hidden before the production family probe"
            );
            production_session
                .mission
                .reveal_player_source_sprite_for_test(production_player.entity_index);
            let offset_position = |offset_x: i16, offset_y: i16| Position {
                x: production_player.position.x.wrapping_add_signed(offset_x),
                y: production_player.position.y.wrapping_add_signed(offset_y),
                z: production_player.position.z,
            };
            let production_burst = production_session
                .mission
                .spawn_death_burst_from_death(
                    production_player.entity_index,
                    offset_position(-210, 0),
                )
                .expect("production death-burst submission");
            let production_marker = production_session
                .mission
                .spawn_player_death_marker_for_test(&production_player)
                .expect("production death-marker constructor");
            let production_association = production_session
                .mission
                .spawn_castle_association_for_test(
                    production_player.entity_index,
                    offset_position(-140, 30),
                )
                .expect("production Castle-association constructor");
            let production_builder = production_session
                .mission
                .spawn_house_resident(offset_position(-70, 60), 12, 97, 0)
                .expect("production Builder constructor")
                .expect("production Builder source slot");
            let production_trader = production_session
                .mission
                .spawn_house_resident(offset_position(70, 90), 14, 113, 0)
                .expect("production Trader constructor")
                .expect("production Trader source slot");
            let target = production_session.first_goat_source_view_for_test();
            let mut actual_sources = vec![
                ("production-local-player", production_player.entity_index),
                ("production-death-burst", production_burst),
                ("production-death-marker", production_marker),
                ("production-castle-association", production_association),
                ("production-builder", production_builder),
                ("production-trader", production_trader),
            ];
            for (ordinal, profile) in CASTLE_DEFENSE_SHOT_PROFILES.iter().copied().enumerate() {
                let offset = i16::try_from(ordinal).expect("five shot rows fit i16");
                let controller = production_session
                    .mission
                    .spawn_castle_defense_controller(
                        offset_position(140 + offset * 55, 120 + offset * 35),
                        production_player.entity_index,
                    )
                    .expect("production defense-controller source slot");
                if ordinal == 0 {
                    actual_sources.push(("production-defense-controller", controller));
                }
                let shot = production_session
                    .mission
                    .fire_castle_defense_shot_for_test(
                        &production_player,
                        controller,
                        target,
                        profile.spell_family,
                        profile.subspell,
                    )
                    .expect("production action-86 shot allocation");
                actual_sources.push((
                    match ordinal {
                        0 => "production-shot-fireball",
                        1 => "production-shot-arrow",
                        2 => "production-shot-arrow-return",
                        3 => "production-shot-fireball-alternate",
                        4 => "production-shot-lightning",
                        _ => unreachable!(),
                    },
                    shot,
                ));
            }
            let production_dispositions = production_session
                .source_sprite_dispositions()
                .expect("complete production typed source sprites");
            let production_submissions = actual_sources
                .iter()
                .map(|(family, source_entity_index)| {
                    let submission = production_dispositions
                    .iter()
                    .find_map(|disposition| match disposition {
                        SourceSpriteDisposition::Render(submission)
                            if submission.source_entity_index == *source_entity_index =>
                        {
                            Some(*submission)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{family} slot {source_entity_index} has no production typed submission; source={:?}, disposition={:?}",
                            production_session
                                .mission
                                .source_slots()
                                .get(usize::from(*source_entity_index)),
                            production_dispositions.iter().find(|disposition| {
                                match disposition {
                                    SourceSpriteDisposition::Render(submission) => {
                                        submission.source_entity_index == *source_entity_index
                                    }
                                    SourceSpriteDisposition::Legacy {
                                        source_entity_index: index,
                                        ..
                                    }
                                    | SourceSpriteDisposition::Excluded {
                                        source_entity_index: index,
                                        ..
                                    } => *index == *source_entity_index,
                                }
                            })
                        )
                    });
                    (*family, *source_entity_index, submission)
                })
                .collect::<Vec<_>>();
            let production_camera = Level1TerrainFrameCamera {
                render_x: production_player.position.x.wrapping_add(75),
                render_y: production_player.position.y.wrapping_add(430),
                eye_z: i32::from(production_player.position.z),
                reflections: false,
                ..source_camera
            };
            let production_resources = || NativeSourceFrameResources {
                frozen_actor_tmap_bank: Some(&frozen_actor_bank),
                villager_tmap_clocks: Some(&villager_clocks),
                archer_tmap_bank: &archer_bank,
                archer_tmap_clocks: &archer_clocks,
                firefly_tmap_bank: &firefly_bank,
                firefly_tmap_clocks: &firefly_clocks,
                combat_tmap_bank: &combat_bank,
                static_world_tmap_bank: &static_world_bank,
                economy_tmap_bank: &economy_bank,
                fireball_tmap_clock: &fireball_clock,
                possession_tmap_clock: &possession_clock,
                castle_tmap_clock: &castle_clock,
            };
            let mut production_deltas = Vec::new();
            for (family, source_entity_index, submission) in production_submissions {
                let mut source_chain = production_session.mission.source_map_chains().clone();
                source_chain
                    .retain_entities(&[])
                    .expect("start an exact per-source attribution chain");
                source_chain
                    .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: source_entity_index,
                        active: true,
                        x: submission.position.x,
                        y: submission.position.y,
                    })
                    .expect("actual production source slot fits the attribution chain");
                let mut visible_session = production_session.clone_for_source_frame_test();
                visible_session.mission.set_source_map_chains(source_chain);
                let production_frame = visible_session
                    .render_source_world_frame(
                        production_camera,
                        render_resources(),
                        production_resources(),
                    )
                    .unwrap_or_else(|error| panic!("{family}: {error}"));
                let no_draw_frame = visible_session
                    .render_source_world_frame_with_typed_no_draw_for_test(
                        production_camera,
                        render_resources(),
                        production_resources(),
                        source_entity_index,
                    )
                    .unwrap_or_else(|error| panic!("{family}: {error}"));
                let mut delta_bytes = Vec::new();
                for (index, (visible, no_draw)) in production_frame
                    .indices
                    .iter()
                    .zip(&no_draw_frame.indices)
                    .enumerate()
                {
                    if visible == no_draw {
                        continue;
                    }
                    delta_bytes.extend_from_slice(
                        &u32::try_from(index)
                            .expect("source frame index fits u32")
                            .to_le_bytes(),
                    );
                    delta_bytes.push(*visible);
                    delta_bytes.push(*no_draw);
                }
                let delta_count = delta_bytes.len() / 6;
                assert!(
                    delta_count > 0,
                    "{family} slot {source_entity_index} must change visible pixels against its otherwise-identical typed no-draw baseline"
                );
                production_deltas.push((family, delta_count, byte_hash(&delta_bytes)));
            }
            assert_eq!(production_deltas.len(), 12);
            assert_eq!(
                production_deltas.as_slice(),
                &[
                    ("production-local-player", 5_254, 0xb87d_8215_f6f4_e988),
                    ("production-death-burst", 7_820, 0xc376_917b_edea_b473),
                    ("production-death-marker", 10_714, 0x8be3_f05b_eb2f_8254),
                    (
                        "production-castle-association",
                        7_353,
                        0xc324_4182_a670_4375,
                    ),
                    ("production-builder", 2_383, 0xa5a9_6cbd_45c8_8cf0),
                    ("production-trader", 4_067, 0x9846_bd8e_67d3_329e),
                    (
                        "production-defense-controller",
                        7_734,
                        0x7c55_a9b3_6ca7_42bc,
                    ),
                    ("production-shot-fireball", 1_894, 0xb19a_5ef6_2cf0_c3a2),
                    ("production-shot-arrow", 504, 0x3c9e_d920_cced_2b78),
                    ("production-shot-arrow-return", 678, 0xeb33_807f_e7ba_4b26,),
                    (
                        "production-shot-fireball-alternate",
                        653,
                        0xc4ea_b5dc_4b7f_4aa5,
                    ),
                    ("production-shot-lightning", 380, 0xb0cd_3914_6a6a_b7b9),
                ],
                "production source pixel-delta count/hash vector changed"
            );
            assert_eq!(
                production_deltas
                    .iter()
                    .map(|(_, _, hash)| *hash)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                12,
                "all twelve actual producers must have distinct target-only pixel deltas: {production_deltas:?}"
            );
            let imported_slot = production_dispositions
                .iter()
                .find_map(|disposition| match disposition {
                    SourceSpriteDisposition::Legacy {
                        source_entity_index,
                        family: SourceSpriteLegacyFamily::ImportedReset,
                    } => Some(*source_entity_index),
                    _ => None,
                })
                .expect("the complete reset chain contains a typed imported row");
            production_session
                .mission
                .set_source_no_draw_for_test(imported_slot);
            assert!(
                production_session
                    .source_sprite_dispositions()
                    .expect("known live tuple with a source no-draw bit remains typed")
                    .iter()
                    .any(|disposition| {
                        matches!(
                            disposition,
                            SourceSpriteDisposition::Excluded {
                                source_entity_index,
                                reason: SourceSpriteExclusionReason::SourceNoDrawFlags,
                            } if *source_entity_index == imported_slot
                        )
                    }),
                "a recognized live tuple may become a typed source no-draw exclusion"
            );
            production_session
                .mission
                .replace_source_tuple_for_test(imported_slot, 254, 253, 252);
            let error = production_session
                .source_sprite_dispositions()
                .expect_err("an unknown live raw-chain tuple must fail before rasterization");
            assert!(
                error.starts_with(&format!(
                    "unknown Level 1 source sprite tuple: slot={imported_slot} class=254 model=253 action=252"
                )),
                "production fail-closed diagnostic must carry the live slot/class/model/action tuple: {error}"
            );
        }

        let normal = make_submission(979, goat.position, 3, 0, 0, 66, 300, 0, 200);
        let special = make_submission(979, goat.position, 10, 1, 1, 41, 300, 21, 200);
        let normal_frame = render_source_vector(source_camera, normal);
        let special_frame = render_source_vector(source_camera, special);
        assert_ne!(
            normal_frame.index_hash, special_frame.index_hash,
            "normal and quarter-sum special hotspots must produce different indexed pixels"
        );
        let mut rolled = normal;
        rolled.roll = 96;
        let rolled_frame = render_source_vector(source_camera, rolled);
        assert_ne!(
            normal_frame.index_hash, rolled_frame.index_hash,
            "entity roll must reach the production source raster"
        );

        let unmirrored = make_submission(979, goat.position, 5, 12, 97, 221, 310, 20, 250);
        let mut mirrored = unmirrored;
        mirrored.yaw = 1_024;
        let unmirrored_frame = render_source_vector(source_camera, unmirrored);
        let mirrored_frame = render_source_vector(source_camera, mirrored);
        assert_ne!(
            unmirrored_frame.index_hash, mirrored_frame.index_hash,
            "mode-20 mirror selection must preserve asymmetric source pixels"
        );

        let odd_width = normal;
        let mut odd_height = normal;
        odd_height.base_tmap = 301;
        odd_height.selected_frame = 301;
        let odd_width_frame = render_source_vector(source_camera, odd_width);
        let odd_height_frame = render_source_vector(source_camera, odd_height);
        assert_ne!(
            odd_width_frame.index_hash, odd_height_frame.index_hash,
            "odd/even source extents must retain distinct hotspot arithmetic"
        );
        assert_eq!(
            (
                normal_frame.index_hash,
                special_frame.index_hash,
                rolled_frame.index_hash,
                unmirrored_frame.index_hash,
                mirrored_frame.index_hash,
                odd_width_frame.index_hash,
                odd_height_frame.index_hash,
            ),
            (
                0xe01c_88b1_1836_ddfe,
                0x23ca_6253_9d54_0943,
                0xfead_34af_a907_ba86,
                0xdddd_e4a9_2d70_9b2d,
                0x5c2d_84bb_ed97_a9c9,
                0xe01c_88b1_1836_ddfe,
                0x705a_7344_080d_4656,
            ),
            "normal/special hotspot, roll, mirror, and odd/even extent vectors are exact"
        );

        let mut reflected = normal;
        reflected.position = frozen_subject.position;
        let reflected_frame = render_source_vector(frozen_camera, reflected);
        assert!(
            reflected_frame
                .reflected_sprite_raster_stats
                .framebuffer_writes
                > 0
        );
        assert!(reflected_frame.combat_raster_stats.framebuffer_writes > 0);
        assert!(
            reflected_frame
                .shadow_sprite_raster_stats
                .framebuffer_writes
                > 0
        );
        assert_ne!(
            reflected_frame.index_hash, normal_frame.index_hash,
            "complete reflection sweep, then terrain, shadow and primary writes are framebuffer-visible"
        );
        assert_eq!(
            (
                reflected_frame.index_hash,
                reflected_frame.reflected_sprite_raster_stats,
                reflected_frame.shadow_sprite_raster_stats,
                reflected_frame.combat_raster_stats,
            ),
            (
                0xf100_ad2c_4b0d_4a6a,
                Mode20SpriteRasterStats {
                    candidate_pixels: 2_068,
                    transparent_pixels: 0,
                    clipped_pixels: 1_247,
                    framebuffer_writes: 731,
                },
                Mode20SpriteRasterStats {
                    candidate_pixels: 516,
                    transparent_pixels: 0,
                    clipped_pixels: 275,
                    framebuffer_writes: 187,
                },
                Mode20SpriteRasterStats {
                    candidate_pixels: 2_068,
                    transparent_pixels: 0,
                    clipped_pixels: 1_196,
                    framebuffer_writes: 782,
                },
            ),
            "the complete reflected sweep precedes the exact shadow/primary source writes"
        );

        let mut clip_vectors = Vec::new();
        for (label, delta_x, delta_z) in [
            ("left", -285_i32, 0_i32),
            ("right", 285, 0),
            ("top", 0, 100),
            ("bottom", 0, -300),
        ] {
            let mut clipped = normal;
            clipped.position.x = u16::try_from(i32::from(clipped.position.x).wrapping_add(delta_x))
                .expect("bounded CP2 clipping x");
            clipped.position.z = i16::try_from(i32::from(clipped.position.z).wrapping_add(delta_z))
                .expect("bounded CP2 clipping altitude");
            let frame = render_source_vector(source_camera, clipped);
            assert!(
                frame.combat_raster_stats.clipped_pixels > 0
                    && frame.combat_raster_stats.framebuffer_writes > 0,
                "{label} vector must partially clip, not reject, through the production raster: {:?}",
                frame.combat_raster_stats
            );
            clip_vectors.push((
                label,
                frame.index_hash,
                frame.combat_raster_stats.clipped_pixels,
                frame.combat_raster_stats.framebuffer_writes,
            ));
        }
        assert_eq!(
            clip_vectors,
            [
                ("left", 0xfd9f_dafc_d6e9_25e5, 5_630, 5_700),
                ("right", 0x0030_baa6_4431_99df, 5_730, 5_600),
                ("top", 0x72bd_ab3b_7f84_6772, 6_592, 4_738),
                ("bottom", 0x658c_b8c7_2f1f_58ba, 6_592, 4_738),
            ],
            "four-side clipping pins left/top source-phase advance and right/bottom bounds"
        );
        assert_eq!(
            render_source_vector(source_camera, normal),
            normal_frame,
            "repeated production rendering is pure and byte-deterministic"
        );
        assert_eq!(
            render_order_case(&[980, 981, 982], true),
            excluded_middle,
            "overlap ordering and shadow/primary placement are stable across repeated draws"
        );
        assert_eq!(
            mission.dynamic_lighting_hash_v1(),
            lighting_hash_before_source_render,
            "rendering cannot advance lighting RNG or mutate retained light history"
        );
    }
}
