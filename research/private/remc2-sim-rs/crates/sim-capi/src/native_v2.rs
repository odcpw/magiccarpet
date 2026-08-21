//! Package-backed, handle-based ABI v2 for native presentation clients.
//!
//! All paths and data are supplied explicitly by the caller. This module does
//! not use the embedded owner regression bytes retained by the legacy v1 ABI.

mod campaign_facade;
mod shared_types;
pub use shared_types::*;

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    slice, str,
    sync::{Arc, Mutex, OnceLock},
};

use campaign_facade::{
    CampaignFacade, InternalCampaignAdapter, InternalCampaignOpen, SealedLevel1Adapter,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
use sim_core::native_world::resolve_level1_presentation_cache_root;
use sim_core::{
    FlightContext, FlightEvent, FlightState, FlightWorld, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
    PlayerCommand, Position, PositionalSoundCommand, StepEvidence,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::rasterize_classic_hd_terrain,
    classic_hd_render_graph::{
        ClassicHdBlurBranch, ClassicHdBlurInput, ClassicHdGraphError, ClassicHdGraphInput,
        ClassicHdLightingPath, ClassicHdRenderGraph, ClassicHdWorldReceipt,
    },
    classic_hd_sprites::{ClassicHdSpritePlan, build_classic_hd_sprite_plan},
    classic_hd_terrain::{ClassicHdSkyBank, ClassicHdTerrainResources, build_classic_hd_terrain},
    combat_tmap::{
        COMBAT_STATIC_EXTENTS, COMBAT_STATIC_SELECTORS, COMBAT_STATIC_SHA256, CombatTmapBank,
        FIREBALL_PAYLOAD_SHA256, FireballTmapClock, POSSESSION_PAYLOAD_SHA256, PossessionTmapClock,
    },
    conformance_hash, conformance_hash_v2,
    day_indexed_frame::{DayIndexedFrame, DayIndexedFrameAssets, DayIndexedFrameState},
    dynamic_lighting::DynamicLightingExecutionReceipt,
    economy_tmap::{
        CASTLE_ROOT_PAYLOAD_BYTES, CASTLE_ROOT_PAYLOAD_SHA256, CastleTmapClock, EconomyTmapBank,
        MANA_CARRIER_PAYLOAD_BYTES, MANA_CARRIER_PAYLOAD_SHA256, MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
        MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256, MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
    },
    firefly_tmap::{FireflyTmapBank, FireflyTmapClocks},
    frozen_actor_tmap::{
        DOLMEN_LOGICAL_HEIGHT, DOLMEN_PAYLOAD_BYTES, DOLMEN_PAYLOAD_SHA256, DOLMEN_RENDER_MODE,
        DOLMEN_RENDER_PARAMETER_INDEX, DOLMEN_SELECTOR, FrozenActorTmapBank,
        LEVEL1_FROZEN_ACTOR_ENTITY_INDICES, VILLAGER_ANIMATED_SELECTORS, VILLAGER_PAYLOAD_BYTES,
        VILLAGER_PAYLOAD_SHA256, VillagerTmapClocks,
    },
    goat::{
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_PATROL_ACTION, GoatPlayerTarget, GoatState,
        GoatStepEvent, LEVEL_ONE_GOAT_INDICES, LEVEL_TWO_GOAT_INDICES, LiveWorldState,
    },
    goat_draw_admission::{
        GoatDrawAdmission, GoatDrawCamera, evaluate_goat_draw_admission,
        outdoor_billboard_visit_map,
    },
    goat_tmap::{
        GOAT_TMAP_DRAW_MASK, GOAT_TMAP_SELECTORS, GoatTmapBank, GoatTmapClock, GoatTmapClocks,
    },
    house::{HouseState, HouseStepEvent},
    indexed_overlay::IndexedOverlayAssets,
    indexed_postprocess::{IndexedPostprocessBranch, IndexedPostprocessInput, OVERLAY_PROVENANCE},
    level1_mission::{
        ARCHER_ARROW_BASE_TMAP, ARCHER_ARROW_LOGICAL_HEIGHT, ARCHER_ARROW_PROJECTILE_CLASS,
        ARCHER_ARROW_PROJECTILE_MODEL, ARCHER_ARROW_RENDER_INDEX, ARCHER_ARROW_RENDER_MODE,
        CASTLE_PROJECTILE_BASE_TMAP, CASTLE_PROJECTILE_CLASS, CASTLE_PROJECTILE_LOGICAL_HEIGHT,
        CASTLE_PROJECTILE_MODEL, CASTLE_PROJECTILE_RENDER_INDEX, CASTLE_PROJECTILE_RENDER_MODE,
        CASTLE_ROOT_ACTION, CASTLE_ROOT_BASE_TMAP, CASTLE_ROOT_CLASS, CASTLE_ROOT_LOGICAL_HEIGHT,
        CASTLE_ROOT_MODEL, CASTLE_ROOT_RENDER_INDEX, CASTLE_SPELL_FAMILY,
        CHECKPOINT_SMOKE_BASE_TMAP, CHECKPOINT_SMOKE_CLASS, CHECKPOINT_SMOKE_FIRST_RENDER_INDEX,
        CHECKPOINT_SMOKE_MODEL, CHECKPOINT_SMOKE_RENDER_MODE, DynamicResidentState,
        FIREBALL_BASE_TMAP, FIREBALL_IMPACT_BASE_TMAP, FIREBALL_IMPACT_CLASS,
        FIREBALL_IMPACT_LOGICAL_HEIGHT, FIREBALL_IMPACT_RENDER_INDEX, FIREBALL_IMPACT_RENDER_MODE,
        FIREBALL_LOGICAL_HEIGHT, FIREBALL_PROJECTILE_CLASS, FIREBALL_PROJECTILE_MODEL,
        FIREBALL_RENDER_INDEX, FIREBALL_RENDER_MODE, FireballImpact, GROUND_FIRE_BASE_TMAP,
        GROUND_FIRE_MODEL, GROUND_FIRE_RENDER_MODE, HouseConstructionSourceEffect,
        LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL, LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        LEVEL1_MINIMAP_MAX_MARKS, Level1Actor, Level1ActorKind, Level1AmbientEntity,
        Level1CollisionTarget, Level1MinimapEntityCommand, Level1MinimapEntityInput,
        Level1MinimapMarks, Level1MissionEvent, Level1MissionState, Level1MultipartSegment,
        Level1PossessedBuilding, Level1Projectile, Level1ResetSourceSlot, Level1SourceEntityView,
        MANA_CARRIER_ACTION, MANA_CARRIER_BASE_TMAP, MANA_CARRIER_CLASS,
        MANA_CARRIER_LOGICAL_HEIGHT, MANA_CARRIER_MAX_LIFE, MANA_CARRIER_MAX_MANA,
        MANA_CARRIER_MODEL, MANA_CARRIER_RENDER_INDEX, MANA_CARRIER_SPEED, MANA_SPHERE_ACTION,
        MANA_SPHERE_CLASS, MANA_SPHERE_MODEL, MANA_SPHERE_NEUTRAL_BASE_TMAP,
        MANA_SPHERE_NEUTRAL_RENDER_BASE, MANA_SPHERE_PLAYER_ONE_BASE_TMAP,
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE, ManaCarrier, ManaSphere,
        POSSESSION_ATTRACTION_BASE_TMAP, POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
        POSSESSION_ATTRACTION_MODEL, POSSESSION_ATTRACTION_RENDER_INDEX,
        POSSESSION_ATTRACTION_RENDER_MODE, POSSESSION_IMPACT_BASE_TMAP, POSSESSION_IMPACT_CLASS,
        POSSESSION_IMPACT_LOGICAL_HEIGHT, POSSESSION_IMPACT_MODEL, POSSESSION_IMPACT_RENDER_INDEX,
        POSSESSION_IMPACT_RENDER_MODE, POSSESSION_PROJECTILE_BASE_TMAP,
        POSSESSION_PROJECTILE_CLASS, POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
        POSSESSION_PROJECTILE_MODEL, POSSESSION_PROJECTILE_RENDER_INDEX,
        POSSESSION_PROJECTILE_RENDER_MODE, POSSESSION_STRONG_IMPACT_MODEL, WATER_SPLASH_BASE_TMAP,
        WATER_SPLASH_LOGICAL_HEIGHT, WATER_SPLASH_MODEL, WATER_SPLASH_RENDER_INDEX,
        WATER_SPLASH_RENDER_MODE, ground_fire_presentation, level1_map_castle_rope_pixels,
        level1_map_entity_command, level1_minimap_castle_rope_pixels,
        level1_minimap_entity_command, mana_sphere_size_ordinal,
    },
    music::{DynamicMusicEnvelope, FadeCallback},
    native_world::{
        AssetRef, EntityAuthority, EntitySnapshot, Level2DayHudSnapshot, Level2DayHudSpellSlot,
        Level2DispatchEvent, Level2EconomyEvent, Level2ResetSession, NativeWorld,
        NativeWorldInspection, ParticleParameter, SessionTerrain, TerrainPlanes,
        apply_level1_source_effects, level1_turn_start_source_views,
        refresh_level1_current_source_view,
    },
    positional_sound_should_replace, prepare_event_sound_command,
    presentation_frame::{
        MainHistory, PresentationFrameError, PresentationFrameInput, PresentationFrameState,
        SourceRenderedFrame,
    },
    render_camera::{TerrainSlopeCamera, TerrainSlopeCameraSample},
    renderable_map_chain::{RenderableMapChains, RenderableMapSeed},
    source_projection_hash,
    source_sprite_tmap::{
        ImportedResetSpriteTuple, SourceSpriteContractRoute, SourceSpriteContractRow,
        SourceSpriteDisposition, SourceSpriteLegacyFamily, SourceSpriteTemplateDisposition,
        SourceSpriteWorldResources, imported_reset_sprite_disposition, source_sprite_dispositions,
        unknown_source_sprite_tuple,
    },
    state_hash,
    static_world_tmap::{
        EXIT_TARGET_PAYLOAD_BYTES, EXIT_TARGET_PAYLOAD_SHA256, LEVEL1_AMBIENT_SELECTORS,
        LEVEL1_SCENERY_SELECTORS, SPELL_JAR_PAYLOAD_BYTES, SPELL_JAR_PAYLOAD_SHA256,
        StaticWorldTmapBank,
    },
    step_entity_body_with_context, step_outer_turn_with_context,
    terrain_frame::{
        LEVEL1_SKY_BYTES, LEVEL1_TERRAIN_ATLAS_BYTES, LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS,
        LEVEL1_VIEW_WIDTH, Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        raster_level1_source_world_sprites, render_level1_source_world_frame,
        render_level1_source_world_map_frame_composed, render_level1_terrain_frame,
        render_level1_world_frame,
    },
    terrain_grid::{OUTDOOR_GRID_VERTICES, OutdoorGridInput, outdoor_grid},
    terrain_presentation::outdoor_terrain_pnt5,
    terrain_reflection::OutdoorProjection,
    villager::{VILLAGER_CLASS, VILLAGER_MODEL, VillagerState},
};

use crate::presentation_bridge::{
    LIVE_MENU_AUTH_ALL, LIVE_MENU_AUTH_DEBUG_METRICS, Level1PresentationLive, Level2MinimapEntity,
    Level2MinimapMark, Level2MinimapPlan, LiveDebugEntity, LiveHud, LiveMenuState, LiveSpellMenu,
    LiveSpellSlot, PresentationBridgeQuery, build_level1_final_main, level2_minimap_plan,
};

#[cfg(test)]
use sim_core::level1_mission::FIREBALL_IMPACT_MODEL;

#[cfg(test)]
use sim_core::level1_mission::{CASTLE_PROJECTILE_SOUND, WATER_SPLASH_SOUND};

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
    palette_rgb6: Box<[u8]>,
    archer_tmap_bank: ArcherTmapBank,
    firefly_tmap_bank: FireflyTmapBank,
    frozen_actor_tmap_bank: FrozenActorTmapBank,
    combat_tmap_bank: CombatTmapBank,
    static_world_tmap_bank: StaticWorldTmapBank,
    economy_tmap_bank: EconomyTmapBank,
    indexed_overlays: IndexedOverlayAssets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceRenderInputs {
    map_right_viewport: bool,
    camera: Level1TerrainFrameCamera,
    generation_asset_digest: u64,
    local_player_entity_index: u16,
    goats: Vec<GoatState>,
    frozen_actors: Vec<Level1FrozenActorRender>,
    ambient_entities: Vec<Level1AmbientEntity>,
    mission_actors: Vec<Level1Actor>,
    dynamic_residents: Vec<DynamicResidentState>,
    mission_segments: Vec<Level1MultipartSegment>,
    castle_root: Option<Level1CastleRootRender>,
    mana_spheres: Vec<ManaSphere>,
    mana_carrier: Option<ManaCarrier>,
    map_chains: RenderableMapChains,
    projectiles: Vec<Level1Projectile>,
    impacts: Vec<FireballImpact>,
    possessed_buildings: Vec<Level1PossessedBuilding>,
    source_slots: Vec<Level1ResetSourceSlot>,
    dispositions: Vec<SourceSpriteDisposition>,
    night: bool,
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
    terrain: SessionTerrain,
    state: FlightState,
    context: FlightContext,
    live_world: LiveWorldState,
    mission: Level1MissionState,
    frozen_actors: Box<[Level1FrozenActorRender]>,
    selector_frames: Option<GoatTmapClocks>,
    archer_selector_frames: Option<ArcherTmapClocks>,
    firefly_selector_frames: Option<FireflyTmapClocks>,
    villager_selector_frames: Option<VillagerTmapClocks>,
    fireball_selector_frame: Option<FireballTmapClock>,
    possession_selector_frame: Option<PossessionTmapClock>,
    castle_selector_frame: Option<CastleTmapClock>,
    renderable_map_chains: RenderableMapChains,
    selector_draw_feedback: Option<(u64, u8)>,
    notification_draw_feedback: Option<(u64, bool, u8, i16)>,
    goat_draw_admission: Option<GoatDrawAdmission>,
    generation: u64,
    event_head: u64,
    events: VecDeque<McEventV2>,
    #[cfg(test)]
    event_capacity: usize,
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
    classic_hd_shading: Arc<[u8]>,
    source_render_asset_digest: Option<u64>,
    presentation_frame: PresentationFrameState,
    /// Lazy owner-bound Classic HD state. The fixed presentation graph remains
    /// separate; this graph is rebuilt on profile or generation changes.
    classic_hd_graph: Option<ClassicHdRenderGraph>,
    classic_hd_profile_id: Option<u32>,
    classic_hd_sprite_plan: Option<ClassicHdSpritePlan>,
    classic_hd_last_world_receipt: Option<ClassicHdWorldReceipt>,
    lighting_receipt: DynamicLightingExecutionReceipt,
    presentation_ui: LiveMenuState,
    debug_clock_bucket: Option<u32>,
    debug_previous_turn: i32,
    debug_fps: i32,
    sound_number: i16,
    game_timer_turn: i32,
    debug_heap_size: u32,
    debug_memory_used: u32,
    debug_memory_free: u32,
    game_paused: bool,
    pending_house_effects: Vec<HouseStepEvent>,
    #[cfg(test)]
    immediate_released_holder_handoff: Vec<(u8, u16, u8, u8)>,
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

fn level1_frozen_actor_renders(world: &NativeWorld) -> AbiResult<Box<[Level1FrozenActorRender]>> {
    let mut renders = Vec::with_capacity(LEVEL1_FROZEN_ACTOR_ENTITY_INDICES.len());
    for entity_index in LEVEL1_FROZEN_ACTOR_ENTITY_INDICES {
        let entity = world
            .entities()
            .get(usize::from(entity_index))
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("frozen Level 1 actor row {entity_index} is absent"),
                )
            })?;
        let (expected_class, expected_model, expected_action, expected_relation, render_index) =
            match entity_index {
                51 | 55 | 58 | 59 | 63 | 66 | 70 | 71 | 74 | 78 => (5, 13, 111, 100, 271),
                52 | 56 | 60 | 67 | 68 | 75 => (5, 13, 111, 100, 241),
                53 | 64 | 72 => (5, 13, 111, 100, 239),
                54 | 57 | 61 | 62 | 65 | 69 | 73 | 76 | 77 => (5, 13, 111, 100, 242),
                130..=132 => (2, 2, 6, 59, DOLMEN_RENDER_PARAMETER_INDEX),
                _ => unreachable!("qualified frozen actor index set is exhaustive"),
            };
        let expected_flags = if entity_index >= 130 { 4 } else { 12 };
        if !entity.active
            || (entity_index < 130 && entity.authority != EntityAuthority::VillagerLive)
            || (entity_index >= 130 && entity.authority != EntityAuthority::FrozenObserved)
            || entity.class != expected_class
            || entity.model != expected_model
            || entity.action != expected_action
            || entity.parameter_relation != Some(expected_relation)
            || entity.particle_parameter_index != Some(render_index)
            || entity.animation_frame() != 0
            || entity.control_flags != expected_flags
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "frozen Level 1 actor row {entity_index} changed: authority={:?} active={} class/model/action={}/{}/{} relation={:?} particle={:?} animation={} flags={:#010x}",
                    entity.authority,
                    entity.active,
                    entity.class,
                    entity.model,
                    entity.action,
                    entity.parameter_relation,
                    entity.particle_parameter_index,
                    entity.animation_frame(),
                    entity.control_flags
                ),
            ));
        }
        let parameter = world
            .presentation()
            .particles
            .get(usize::from(render_index))
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    format!("frozen Level 1 render parameter {render_index} is absent"),
                )
            })?;
        let expected_parameter = match render_index {
            239 => ParticleParameter {
                base_tmap: 155,
                word_2: 8_492,
                word_4: 42,
                speed: 90,
                rotation_speed: 200,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            241 => ParticleParameter {
                base_tmap: 180,
                word_2: 8_492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            242 => ParticleParameter {
                base_tmap: 183,
                word_2: 8_492,
                word_4: 42,
                speed: 116,
                rotation_speed: 250,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            271 => ParticleParameter {
                base_tmap: 199,
                word_2: 8_492,
                word_4: 42,
                speed: 96,
                rotation_speed: 300,
                byte_10: 0,
                byte_11: -1,
                render_mode: 20,
                byte_13: 0,
            },
            DOLMEN_RENDER_PARAMETER_INDEX => ParticleParameter {
                base_tmap: DOLMEN_SELECTOR,
                word_2: 8_492,
                word_4: 42,
                speed: 666,
                rotation_speed: DOLMEN_LOGICAL_HEIGHT,
                byte_10: 0,
                byte_11: 1,
                render_mode: DOLMEN_RENDER_MODE,
                byte_13: 0,
            },
            _ => unreachable!("qualified frozen actor render parameters are exhaustive"),
        };
        if *parameter != expected_parameter {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("frozen Level 1 render parameter {render_index} changed: {parameter:?}"),
            ));
        }
        renders.push(Level1FrozenActorRender {
            source_entity_index: entity_index,
            position: entity.position,
            yaw: entity.yaw,
            render_parameter_index: render_index,
        });
    }
    Ok(renders.into_boxed_slice())
}

fn reset_renderable_map_chains(
    world: &NativeWorld,
    frozen_actors: &[Level1FrozenActorRender],
) -> AbiResult<RenderableMapChains> {
    // The same source map is used for painter ordering and gameplay
    // collision. Preserve every reset row whose collision bit is set; the
    // compositor can still skip unsupported draw families without removing
    // the intervening source-order links.
    let mut seeds = world
        .entities()
        .iter()
        .filter(|entity| entity.active && entity.control_flags & 8 != 0)
        .map(|entity| RenderableMapSeed {
            entity_index: entity.index,
            // Some live reset rows are deliberately absent from map chains
            // behind source no-map/no-draw gates. Track them for later
            // insertion without requiring reset membership.
            active: false,
            x: entity.position.x,
            y: entity.position.y,
        })
        .collect::<Vec<_>>();
    seeds.extend(
        world
            .reset_live_world()
            .goats()
            .iter()
            .map(|goat| RenderableMapSeed {
                entity_index: goat.entity_index,
                active: false,
                x: goat.position.x,
                y: goat.position.y,
            }),
    );
    seeds.extend(frozen_actors.iter().map(|actor| RenderableMapSeed {
        entity_index: actor.source_entity_index,
        active: false,
        x: actor.position.x,
        y: actor.position.y,
    }));
    seeds.extend(
        world
            .level1_mission()
            .ambient_entities()
            .filter(|ambient| ambient.map_linked)
            .map(|ambient| RenderableMapSeed {
                entity_index: ambient.source_entity_index,
                active: false,
                x: ambient.position.x,
                y: ambient.position.y,
            }),
    );
    seeds.sort_unstable_by_key(|seed| seed.entity_index);
    seeds.dedup_by_key(|seed| seed.entity_index);
    RenderableMapChains::from_reset(
        &seeds,
        &world.terrain_planes().entity_indices,
        |entity_index| {
            world
                .entities()
                .get(usize::from(entity_index))
                .map(EntitySnapshot::old_map_entity)
        },
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn native_composited_actor_kind(kind: Level1ActorKind) -> bool {
    matches!(
        kind,
        Level1ActorKind::Archer
            | Level1ActorKind::Firefly
            | Level1ActorKind::CastleSpellJar
            | Level1ActorKind::SpeedSpellJar
            | Level1ActorKind::ExitTarget
            | Level1ActorKind::MultipartHead
            | Level1ActorKind::ExperienceScroll
            | Level1ActorKind::Tree
            | Level1ActorKind::Stone
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AudioVoice {
    sound_id: u16,
    sound_flags: i16,
    event_sequence: u64,
}

fn append_source_render_records<T: std::fmt::Debug>(
    bytes: &mut Vec<u8>,
    family: &[u8],
    records: &[T],
) {
    bytes.extend_from_slice(
        &u16::try_from(family.len())
            .expect("source-render family tag fits u16")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(family);
    bytes.extend_from_slice(
        &u32::try_from(records.len())
            .expect("source-render record extent fits u32")
            .to_le_bytes(),
    );
    for record in records {
        // These source projections derive Debug with declaration-order field
        // names. Length-prefixing that lossless representation avoids struct
        // padding/host-endian dependence while covering every mutable field.
        let encoded = format!("{record:?}");
        bytes.extend_from_slice(
            &u32::try_from(encoded.len())
                .expect("source-render record fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(encoded.as_bytes());
    }
}

fn source_render_asset_digest(world: &NativeWorld, assets: &TerrainFrameAssets) -> AbiResult<u64> {
    let source_bank = world.source_sprite_tmap_bank().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            "source-render generation has no CP1 source TMAP bank",
        )
    })?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"mc2-source-render-assets-v1\0");
    for payload in [&assets.terrain_atlas, &assets.tablesn, &assets.sky] {
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("renderer asset payload fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(payload);
    }
    append_source_render_records(
        &mut bytes,
        b"archer-bank",
        std::slice::from_ref(&assets.archer_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"firefly-bank",
        std::slice::from_ref(&assets.firefly_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"frozen-bank",
        std::slice::from_ref(&assets.frozen_actor_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"combat-bank",
        std::slice::from_ref(&assets.combat_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"static-bank",
        std::slice::from_ref(&assets.static_world_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"economy-bank",
        std::slice::from_ref(&assets.economy_tmap_bank),
    );
    append_source_render_records(
        &mut bytes,
        b"source-bank",
        std::slice::from_ref(source_bank),
    );
    append_source_render_records(&mut bytes, b"particles", &world.presentation().particles);
    Ok(sim_core::presentation_frame::byte_hash(&bytes))
}

fn classic_hd_profile_for_id(profile_id: u32) -> AbiResult<ClassicHdRenderProfile> {
    match profile_id {
        0 => Ok(ClassicHdRenderProfile::compatibility_320x200()),
        1 => Ok(ClassicHdRenderProfile::hd_640x480()),
        2 => Ok(ClassicHdRenderProfile::hd_960x720()),
        3 => Ok(ClassicHdRenderProfile::hd_1280x960()),
        _ => Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("unknown Classic HD profile id {profile_id}"),
        )),
    }
}

impl SessionRecord {
    fn set_game_paused(&mut self, paused: bool) {
        if paused && !self.game_paused {
            // Capture the current live player state once at the pause
            // boundary so presentation may continue while entity action
            // dispatch remains held.
            self.camera_history = CameraHistory::capture(
                &self.state,
                self.source_turn,
                self.camera_fov,
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.camera_slope_sample = self.camera_slope.update(
                self.terrain.ground_heights(),
                self.camera_history.x,
                self.camera_history.y,
            );
        }
        self.game_paused = paused;
    }

    fn apply_presentation_input(&mut self, input: McLevel1PresentationInputV2) -> AbiResult<()> {
        validate_reserved(&input.reserved, "Level 1 presentation input")?;
        if input.reserved_0 != 0 || input.reserved_1 != 0 || input.reserved_2 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "Level 1 presentation input reserved fields must be zero",
            ));
        }
        if input.authority_mask & !LIVE_MENU_AUTH_ALL != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "Level 1 presentation authority mask {:#x} has unknown bits",
                    input.authority_mask
                ),
            ));
        }
        if input.debug_heap_size != 0
            || input.debug_memory_used != 0
            || input.debug_memory_free != 0
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 source allocator metrics are simulation-owned and input fields must be zero",
            ));
        }
        for (name, value) in [
            ("pause_menu_suppressed", input.pause_menu_suppressed),
            ("pause_text_suppressed", input.pause_text_suppressed),
            ("level_end_text_suppressed", input.level_end_text_suppressed),
            ("recursive_pause_writer", input.recursive_pause_writer),
            ("load_available", input.load_available),
            ("sound_available", input.sound_available),
            ("music_available", input.music_available),
            ("cursor_visible", input.cursor_visible),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("{name} must be zero or one"),
                ));
            }
        }
        let volume_preview = self.mission.player_menu_state == 10
            && input.volume_target == 1
            && self.presentation_ui.sound_volume != input.sound_volume;
        let chat_lines = std::array::from_fn(|index| {
            let start = index * 64;
            input.chat_lines[start..start + 64]
                .try_into()
                .expect("fixed chat line extent")
        });
        let chat_names = std::array::from_fn(|index| {
            let start = index * 16;
            input.chat_names[start..start + 16]
                .try_into()
                .expect("fixed chat name extent")
        });
        self.presentation_ui = LiveMenuState {
            authority_mask: input.authority_mask,
            selected_menu_item: input.selected_menu_item,
            pause_menu_suppressed: input.pause_menu_suppressed != 0,
            pause_text_suppressed: input.pause_text_suppressed != 0,
            level_end_text_suppressed: input.level_end_text_suppressed != 0,
            recursive_pause_writer: input.recursive_pause_writer != 0,
            load_available: input.load_available != 0,
            sound_available: input.sound_available != 0,
            music_available: input.music_available != 0,
            options_enabled_mask: input.options_enabled_mask,
            options_selected_mask: input.options_selected_mask,
            sound_volume: input.sound_volume,
            music_volume: input.music_volume,
            volume_target: input.volume_target,
            chat_recipient: input.chat_recipient,
            chat_mode: input.chat_mode,
            chat_player_mask: input.chat_player_mask,
            local_player_index: input.local_player_index,
            frame_timing_index: input.frame_timing_index,
            chat_lines,
            chat_names,
            notification_player_name: input.notification_player_name,
            cursor_visible: input.cursor_visible != 0,
            mouse_x: input.mouse_x,
            mouse_y: input.mouse_y,
            joystick: input.joystick,
            text_colour_phase: input.text_colour_phase,
        };
        if input.authority_mask & LIVE_MENU_AUTH_DEBUG_METRICS != 0 {
            if self.debug_clock_bucket != Some(input.debug_clock_bucket) {
                if self.debug_clock_bucket.is_some() {
                    self.debug_fps = self.source_turn.wrapping_sub(self.debug_previous_turn);
                }
                self.debug_clock_bucket = Some(input.debug_clock_bucket);
                self.debug_previous_turn = self.source_turn;
            }
            self.sound_number = input.sound_number;
            self.debug_heap_size = LEVEL1_SOURCE_HEAP_SIZE;
            self.debug_memory_used = LEVEL1_SOURCE_MEMORY_USED;
            self.debug_memory_free = LEVEL1_SOURCE_MEMORY_FREE;
        } else {
            self.debug_clock_bucket = None;
            self.debug_previous_turn = self.source_turn;
            self.debug_fps = 0;
            self.sound_number = 0;
            self.debug_heap_size = 0;
            self.debug_memory_used = 0;
            self.debug_memory_free = 0;
        }
        if volume_preview {
            self.emit_local_flight_sound(sim_core::volume_preview_audio_event(
                self.state.entity_index,
                i16::from(self.world.runtime().local_player_slot),
            ));
            // `AdjustVolume_1A070` synchronously calls PlayEntitySounds after
            // preparing its preview, including while byte_38591 selects the
            // sound slider during pause.
            self.drain_pending_audio_commands();
        }
        Ok(())
    }

    fn emit_local_flight_sound(&mut self, event: FlightEvent) {
        let record = self.encode_flight_event(event, state_hash(&self.state));
        self.append_event(record);
    }

    fn live_world_hash_v2(&self) -> u64 {
        self.live_world.hash_v2(self.mission.global_rng())
    }

    fn source_render_input_hash(&self) -> AbiResult<u64> {
        let map_right_viewport = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let inputs = self.source_render_inputs(map_right_viewport)?;
        self.source_render_input_hash_for(&inputs)
    }

    fn source_render_input_hash_for(&self, inputs: &SourceRenderInputs) -> AbiResult<u64> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"mc2-source-render-input-v1\0");
        bytes.extend_from_slice(&self.terrain.deterministic_hash().to_le_bytes());
        bytes.extend_from_slice(&self.live_world_hash_v2().to_le_bytes());
        bytes.push(u8::from(inputs.map_right_viewport));
        bytes.extend_from_slice(&inputs.camera.render_x.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.render_y.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.eye_z.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.yaw.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.pitch.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.roll.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.fov.to_le_bytes());
        bytes.extend_from_slice(&inputs.camera.source_turn.to_le_bytes());
        bytes.push(u8::from(inputs.camera.reflections));
        bytes.extend_from_slice(&inputs.generation_asset_digest.to_le_bytes());
        bytes.extend_from_slice(&inputs.local_player_entity_index.to_le_bytes());
        for value in [
            self.camera_slope_sample.target_x,
            self.camera_slope_sample.target_y,
            self.camera_slope_sample.offset_x,
            self.camera_slope_sample.offset_y,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(
            &u32::try_from(inputs.frozen_actors.len())
                .expect("frozen render count fits u32")
                .to_le_bytes(),
        );
        for actor in &inputs.frozen_actors {
            bytes.extend_from_slice(&actor.source_entity_index.to_le_bytes());
            bytes.extend_from_slice(&actor.position.x.to_le_bytes());
            bytes.extend_from_slice(&actor.position.y.to_le_bytes());
            bytes.extend_from_slice(&actor.position.z.to_le_bytes());
            bytes.extend_from_slice(&actor.yaw.to_le_bytes());
            bytes.extend_from_slice(&actor.render_parameter_index.to_le_bytes());
        }
        append_source_render_records(&mut bytes, b"goats", &inputs.goats);
        append_source_render_records(&mut bytes, b"ambient", &inputs.ambient_entities);
        append_source_render_records(&mut bytes, b"actors", &inputs.mission_actors);
        append_source_render_records(&mut bytes, b"residents", &inputs.dynamic_residents);
        append_source_render_records(&mut bytes, b"segments", &inputs.mission_segments);
        append_source_render_records(&mut bytes, b"castle", inputs.castle_root.as_slice());
        append_source_render_records(&mut bytes, b"spheres", &inputs.mana_spheres);
        append_source_render_records(&mut bytes, b"carrier", inputs.mana_carrier.as_slice());
        append_source_render_records(&mut bytes, b"projectiles", &inputs.projectiles);
        append_source_render_records(&mut bytes, b"impacts", &inputs.impacts);
        append_source_render_records(&mut bytes, b"possessed", &inputs.possessed_buildings);
        append_source_render_records(&mut bytes, b"source-slots", &inputs.source_slots);
        append_source_render_records(&mut bytes, b"dispositions", &inputs.dispositions);
        let topology = inputs.map_chains.canonical_topology_bytes();
        bytes.extend_from_slice(
            &u32::try_from(topology.len())
                .expect("source map topology fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&topology);
        bytes.push(u8::from(inputs.night));
        let mut frame = |selector: u16,
                         state: u8,
                         frame_index: u8,
                         cursor: u32,
                         width: u16,
                         height: u16,
                         pixels: &[u8]| {
            bytes.extend_from_slice(&selector.to_le_bytes());
            bytes.push(state);
            bytes.push(frame_index);
            bytes.extend_from_slice(&cursor.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&height.to_le_bytes());
            bytes.extend_from_slice(
                &u32::try_from(pixels.len())
                    .expect("TMAP payload fits u32")
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(pixels);
        };
        for clock in self
            .selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Goat TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .archer_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Archer TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .firefly_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no Firefly TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                clock.frame_index(),
                clock.stream_cursor(),
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for clock in self
            .villager_selector_frames
            .as_ref()
            .ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no villager TMAP clocks",
                )
            })?
            .states()
        {
            frame(
                clock.selector(),
                clock.state_ordinal(),
                0,
                0,
                clock.width(),
                clock.height(),
                clock.pixels(),
            );
        }
        for animated in [
            self.fireball_selector_frame
                .as_ref()
                .map(FireballTmapClock::frame),
            self.possession_selector_frame
                .as_ref()
                .map(PossessionTmapClock::frame),
        ] {
            let animated = animated.ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "source-render input has no combat TMAP clock",
                )
            })?;
            frame(
                animated.selector,
                animated.state_ordinal,
                animated.frame_index,
                animated.stream_cursor,
                animated.width,
                animated.height,
                &animated.pixels,
            );
        }
        let castle = self.castle_selector_frame.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render input has no Castle TMAP clock",
            )
        })?;
        let castle_frame = castle.frame();
        frame(
            castle_frame.selector,
            castle.state_ordinal(),
            0,
            0,
            castle_frame.width,
            castle_frame.height,
            &castle_frame.pixels,
        );
        Ok(sim_core::presentation_frame::byte_hash(&bytes))
    }

    fn new(world: NativeWorld) -> AbiResult<Self> {
        let terrain_frame_assets = load_optional_terrain_frame_assets()?.map(Arc::new);
        Self::new_with_terrain_frame_assets(world, terrain_frame_assets)
    }

    fn new_with_terrain_frame_assets(
        world: NativeWorld,
        terrain_frame_assets: Option<Arc<TerrainFrameAssets>>,
    ) -> AbiResult<Self> {
        let world = Arc::new(world);
        let archer_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        let firefly_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        let villager_selector_frames = terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        let fireball_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        let possession_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        let castle_selector_frame = terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        let frozen_actors = level1_frozen_actor_renders(world.as_ref())?;
        let renderable_map_chains =
            reset_renderable_map_chains(world.as_ref(), frozen_actors.as_ref())?;
        let player = &world.players()[usize::from(world.runtime().local_player_slot)];
        let record = player.record_bytes();
        let source_turn = player.turn;
        let camera_look_yaw = i16::from_le_bytes([record[28], record[29]]);
        let camera_look_pitch = i16::from_le_bytes([record[30], record[31]]);
        let state = *world.reset_state();
        let mut mission = world.level1_mission().session(
            world.entities()[usize::from(state.entity_index)].life,
            1_000,
        );
        mission.set_source_turn_counter(source_turn.cast_unsigned());
        mission.set_reset_source_map_chains(world.reset_source_map_chains().clone());
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
        let terrain = SessionTerrain::from_world(world.as_ref());
        let classic_hd_shading = Arc::from(terrain.planes().shading.clone());
        let lighting_receipt = mission
            .dynamic_lighting_execution_receipt(&terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let source_render_asset_digest = terrain_frame_assets
            .as_ref()
            .map(|assets| source_render_asset_digest(world.as_ref(), assets))
            .transpose()?;
        Ok(Self {
            terrain,
            state,
            context: world.session_context(),
            live_world: world.reset_live_world().clone(),
            mission,
            frozen_actors,
            selector_frames: world.goat_tmap_bank().map(GoatTmapClocks::from_bank),
            archer_selector_frames,
            firefly_selector_frames,
            villager_selector_frames,
            fireball_selector_frame,
            possession_selector_frame,
            castle_selector_frame,
            renderable_map_chains,
            selector_draw_feedback: None,
            notification_draw_feedback: None,
            goat_draw_admission: None,
            world,
            generation: 1,
            event_head: 0,
            events: VecDeque::new(),
            #[cfg(test)]
            event_capacity: EVENT_CAPACITY,
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
            classic_hd_shading,
            source_render_asset_digest,
            presentation_frame: PresentationFrameState::new(1),
            classic_hd_graph: None,
            classic_hd_profile_id: None,
            classic_hd_sprite_plan: None,
            classic_hd_last_world_receipt: None,
            lighting_receipt,
            presentation_ui: LiveMenuState::default(),
            debug_clock_bucket: None,
            debug_previous_turn: source_turn,
            debug_fps: 0,
            sound_number: 0,
            game_timer_turn: 0,
            debug_heap_size: 0,
            debug_memory_used: 0,
            debug_memory_free: 0,
            game_paused: false,
            pending_house_effects: Vec::new(),
            #[cfg(test)]
            immediate_released_holder_handoff: Vec::new(),
        })
    }

    fn reset(&mut self) {
        self.terrain = SessionTerrain::from_world(self.world.as_ref());
        Arc::make_mut(&mut self.classic_hd_shading).copy_from_slice(&self.terrain.planes().shading);
        self.state = *self.world.reset_state();
        self.context = self.world.session_context();
        self.live_world = self.world.reset_live_world().clone();
        self.mission = self.world.level1_mission().session(
            self.world.entities()[usize::from(self.state.entity_index)].life,
            1_000,
        );
        self.mission
            .set_reset_source_map_chains(self.world.reset_source_map_chains().clone());
        self.selector_frames = self.world.goat_tmap_bank().map(GoatTmapClocks::from_bank);
        self.archer_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| ArcherTmapClocks::from_bank(&assets.archer_tmap_bank));
        self.firefly_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireflyTmapClocks::from_bank(&assets.firefly_tmap_bank));
        self.villager_selector_frames = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| VillagerTmapClocks::from_bank(&assets.frozen_actor_tmap_bank));
        self.fireball_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| FireballTmapClock::from_bank(&assets.combat_tmap_bank));
        self.possession_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| PossessionTmapClock::from_bank(&assets.combat_tmap_bank));
        self.castle_selector_frame = self
            .terrain_frame_assets
            .as_ref()
            .map(|assets| CastleTmapClock::from_bank(&assets.economy_tmap_bank));
        self.frozen_actors = level1_frozen_actor_renders(self.world.as_ref())
            .expect("validated world reset actor renders remain valid");
        self.renderable_map_chains =
            reset_renderable_map_chains(self.world.as_ref(), self.frozen_actors.as_ref())
                .expect("validated world reset renderable chains remain valid");
        self.selector_draw_feedback = None;
        self.notification_draw_feedback = None;
        self.goat_draw_admission = None;
        self.generation = next_nonzero(self.generation);
        self.presentation_frame.reset(self.generation);
        self.classic_hd_graph = None;
        self.classic_hd_profile_id = None;
        self.classic_hd_sprite_plan = None;
        self.classic_hd_last_world_receipt = None;
        self.event_head = 0;
        self.events.clear();
        self.audio_commands.clear();
        self.audio_voices = [None; AUDIO_VOICE_COUNT];
        self.music_start_armed = true;
        self.fire_sequence = 0;
        self.last_resolution = 0;
        self.presentation_ui = LiveMenuState::default();
        self.debug_clock_bucket = None;
        self.debug_previous_turn = self.source_turn;
        self.debug_fps = 0;
        self.sound_number = 0;
        self.game_timer_turn = 0;
        self.debug_heap_size = 0;
        self.debug_memory_used = 0;
        self.debug_memory_free = 0;
        self.game_paused = false;
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let record = player.record_bytes();
        self.source_turn = player.turn;
        self.mission
            .set_source_turn_counter(self.source_turn.cast_unsigned());
        self.lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .expect("reset dynamic-light state and authored shading have fixed extents");
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
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.pending_house_effects.clear();
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
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
        let planes = self.terrain.planes();
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
        let planes = self.terrain.planes();
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

    fn level1_palette_rgb8(&self) -> AbiResult<[u8; 768]> {
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Level 1 palette unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let original: &[u8; 768] = assets.palette_rgb6.as_ref().try_into().map_err(|_| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "qualified PALN RGB6 palette must contain exactly 768 bytes",
            )
        })?;
        let rgb6 = self.mission.effective_palette_rgb6(original);
        Ok(rgb6.map(|channel| channel.saturating_mul(4)))
    }

    fn render_world_frame(
        &self,
        ambient_entities: &[Level1AmbientEntity],
        map_chains: &RenderableMapChains,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Goat TMAP descriptors")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Archer TMAP descriptors")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Firefly TMAP descriptors")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified villager TMAP descriptors")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Fireball TMAP descriptor")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Possession TMAP descriptor")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("world frame unavailable without qualified Castle TMAP descriptor")
        })?;
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let dynamic_residents = self
            .mission
            .dynamic_residents()
            .copied()
            .collect::<Vec<_>>();
        render_level1_world_frame(
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
            Level1WorldFrameResources {
                local_player_entity_index: self.state.entity_index,
                goats: self.live_world.goats(),
                frozen_actors: &self.frozen_actors,
                ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: self.mission.actors(),
                dynamic_residents: &dynamic_residents,
                mission_segments: self.mission.multipart_segments(),
                castle_root: self
                    .mission
                    .castle_exists
                    .then_some(Level1CastleRootRender {
                        source_entity_index: self.mission.castle_source_entity_index,
                        position: self.mission.castle_position,
                        active: true,
                    }),
                mana_spheres: self.mission.mana_spheres(),
                mana_carrier: self.mission.mana_carrier(),
                map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: self.mission.projectiles(),
                impacts: self.mission.impacts(),
                possessed_buildings: self.mission.possessed_buildings(),
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: self.mission.source_slots(),
                night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    fn source_sprite_dispositions(&self) -> AbiResult<Vec<SourceSpriteDisposition>> {
        let mut dispositions = self
            .mission
            .source_sprite_dispositions(&self.state)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        let mut qualified = dispositions
            .iter()
            .map(source_sprite_disposition_index)
            .collect::<BTreeSet<_>>();
        let legacy = self
            .mission
            .source_sprite_legacy_dispositions(&qualified)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(legacy.iter().map(source_sprite_disposition_index));
        dispositions.extend(legacy);

        let mut imported_rows = Vec::new();
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if qualified.contains(&source_entity_index) {
                continue;
            }
            let entity = self
                .world
                .entities()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let source = self
                .mission
                .source_slots()
                .get(usize::from(source_entity_index))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(source_entity_index, 0, 0, 0, 0),
                    )
                })?;
            let route = if entity.authority == EntityAuthority::GoatLive {
                if (source.class, source.model) != (GOAT_CLASS, GOAT_MODEL)
                    || !matches!(source.action, 9 | 12 | 13 | 14 | GOAT_PATROL_ACTION)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Goat source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Goat)
            } else if entity.authority == EntityAuthority::VillagerLive {
                if (source.class, source.model) != (VILLAGER_CLASS, VILLAGER_MODEL)
                    || !matches!(source.action, 104..=111)
                {
                    return Err(AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!(
                            "live Villager source row mismatch: slot={source_entity_index} source={}/{}/{} phase={} rng={} flags={:#x}",
                            source.class,
                            source.model,
                            source.action,
                            source.phase,
                            source.entity_rng,
                            source.control_flags,
                        ),
                    ));
                }
                SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Resident)
            } else {
                let tuple = ImportedResetSpriteTuple {
                    class: source.class,
                    model: source.model,
                    action: source.action,
                    entity_parameter_index: entity.parameter_relation.unwrap_or(0),
                    particle_parameter_index: entity.particle_parameter_index.unwrap_or(0),
                    animation_frame: entity.animation_frame(),
                    control_flags: entity.control_flags,
                };
                match imported_reset_sprite_disposition(tuple).map_err(|_| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        unknown_source_sprite_tuple(
                            source_entity_index,
                            source.class,
                            source.model,
                            source.action,
                            entity.particle_parameter_index.unwrap_or(0),
                        ),
                    )
                })? {
                    SourceSpriteTemplateDisposition::Render(_) => {
                        SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::ImportedReset)
                    }
                    SourceSpriteTemplateDisposition::Excluded(reason) => {
                        SourceSpriteContractRoute::Excluded(reason)
                    }
                }
            };
            imported_rows.push(SourceSpriteContractRow {
                source_entity_index,
                position: entity.position,
                yaw: entity.yaw,
                pitch: entity.pitch,
                roll: 0,
                control_flags: source.control_flags,
                class: source.class,
                model: source.model,
                action: source.action,
                xclass: 0,
                xmodel: 0,
                route,
            });
        }
        let imported = source_sprite_dispositions(imported_rows)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
        qualified.extend(imported.iter().map(source_sprite_disposition_index));
        dispositions.extend(imported);
        dispositions.sort_unstable_by_key(source_sprite_disposition_index);
        for source_entity_index in self.renderable_map_chains.linked_entity_indices() {
            if !qualified.contains(&source_entity_index) {
                let (class, model, action) = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or((0, 0, 0), |source| {
                        (source.class, source.model, source.action)
                    });
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    unknown_source_sprite_tuple(source_entity_index, class, model, action, 0),
                ));
            }
        }
        Ok(dispositions)
    }

    fn source_render_inputs(&self, map_right_viewport: bool) -> AbiResult<SourceRenderInputs> {
        let dispositions = self.source_sprite_dispositions()?;
        let mut map_chains = self.renderable_map_chains.clone();
        for disposition in &dispositions {
            let SourceSpriteDisposition::Render(submission) = disposition else {
                continue;
            };
            if submission.source_entity_index == self.state.entity_index {
                map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: submission.source_entity_index,
                        active: true,
                        x: submission.position.x,
                        y: submission.position.y,
                    })
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            }
        }
        if self.terrain_frame_assets.is_none() {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render generation has no qualified asset banks",
            ));
        }
        // `TerrainFrameAssets` is held behind an immutable `Arc` for the
        // lifetime of the session. Its digest was computed and stored when
        // the owner cache opened, so recomputing a hash over the terrain,
        // TABLESN, sky, and every TMAP debug record on every HD frame adds
        // tens of milliseconds without providing a possible mutation gate.
        // Reuse the generation-bound receipt instead; cache replacement still
        // requires a session reset/open and therefore cannot silently cross a
        // generation here.
        let generation_asset_digest = self.source_render_asset_digest.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "source-render generation has no asset digest",
            )
        })?;
        let camera = Level1TerrainFrameCamera {
            render_x: self
                .camera_history
                .x
                .wrapping_add(self.camera_slope_sample.offset_x.cast_unsigned()),
            render_y: self
                .camera_history
                .y
                .wrapping_add(self.camera_slope_sample.offset_y.cast_unsigned()),
            eye_z: i32::from(self.camera_history.z).wrapping_add(if map_right_viewport {
                128
            } else {
                0
            }),
            yaw: i16::try_from(self.camera_history.yaw.cast_unsigned() & 0x07ff)
                .expect("masked source yaw fits i16"),
            pitch: self.camera_history.pitch,
            roll: self.camera_history.roll,
            fov: self.camera_history.fov,
            source_turn: self.camera_history.source_turn,
            reflections: true,
        };
        Ok(SourceRenderInputs {
            map_right_viewport,
            camera,
            generation_asset_digest,
            local_player_entity_index: self.state.entity_index,
            goats: self.live_world.goats().to_vec(),
            frozen_actors: self.frozen_actors.to_vec(),
            ambient_entities: self.mission.ambient_entities().copied().collect(),
            mission_actors: self.mission.actors().to_vec(),
            dynamic_residents: self.mission.dynamic_residents().copied().collect(),
            mission_segments: self.mission.multipart_segments().to_vec(),
            castle_root: self
                .mission
                .castle_exists
                .then_some(Level1CastleRootRender {
                    source_entity_index: self.mission.castle_source_entity_index,
                    position: self.mission.castle_position,
                    active: true,
                }),
            mana_spheres: self.mission.mana_spheres().to_vec(),
            mana_carrier: self.mission.mana_carrier().copied(),
            map_chains,
            projectiles: self.mission.projectiles().to_vec(),
            impacts: self.mission.impacts().to_vec(),
            possessed_buildings: self.mission.possessed_buildings().to_vec(),
            source_slots: self.mission.source_slots().to_vec(),
            dispositions,
            night: self.world.metadata().map_type == sim_core::native_world::MapType::Night,
        })
    }

    fn render_source_world_frame(
        &self,
        inputs: &SourceRenderInputs,
    ) -> AbiResult<Level1TerrainFrame> {
        let unavailable = |message| AbiError::new(MC_STATUS_VALIDATION_FAILED, message);
        let assets = self.terrain_frame_assets.as_ref().ok_or_else(|| {
            unavailable(
                "source world frame unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let source_bank = self.world.source_sprite_tmap_bank().ok_or_else(|| {
            unavailable("source world frame unavailable without the CP1 source TMAP bank")
        })?;
        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Goat TMAP clocks")
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Archer TMAP clocks")
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Firefly TMAP clocks")
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without villager TMAP clocks")
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Fireball TMAP clock")
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Possession TMAP clock")
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            unavailable("source world frame unavailable without Castle TMAP clock")
        })?;
        let planes = self.terrain.planes();
        let render = if inputs.map_right_viewport {
            render_level1_source_world_map_frame_composed
        } else {
            render_level1_source_world_frame
        };
        render(
            inputs.camera,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: inputs.local_player_entity_index,
                goats: &inputs.goats,
                frozen_actors: &inputs.frozen_actors,
                ambient_entities: &inputs.ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: &inputs.mission_actors,
                dynamic_residents: &inputs.dynamic_residents,
                mission_segments: &inputs.mission_segments,
                castle_root: inputs.castle_root,
                mana_spheres: &inputs.mana_spheres,
                mana_carrier: inputs.mana_carrier.as_ref(),
                map_chains: &inputs.map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: &inputs.projectiles,
                impacts: &inputs.impacts,
                possessed_buildings: &inputs.possessed_buildings,
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: &inputs.source_slots,
                night: inputs.night,
            },
            SourceSpriteWorldResources {
                dispositions: &inputs.dispositions,
                tmap_bank: source_bank,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))
    }

    /// Render the owner world directly into the profile-sized Classic HD
    /// graph scratch planes. This is deliberately separate from
    /// `render_source_world_frame`: that function remains the sealed 320x200
    /// Exact source compositor and is never used as an upscale source here.
    fn render_classic_hd_world(
        &mut self,
        profile: &ClassicHdRenderProfile,
        indices: &mut [u8],
        provenance: &mut [u8],
        generation: u64,
        tick: u64,
        input_hash: u64,
        history_hash: u64,
    ) -> Result<ClassicHdWorldReceipt, ClassicHdGraphError> {
        let profile_trace = std::env::var_os("MC_CLASSIC_HD_PROFILE_TRACE").is_some();
        let render_started = std::time::Instant::now();
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            ClassicHdGraphError(
                "Classic HD owner render requires MC_ASSET_CACHE_PATH assets".to_owned(),
            )
        })?);
        let inputs_started = std::time::Instant::now();
        let mut inputs = self
            .source_render_inputs(false)
            .map_err(|error| ClassicHdGraphError(error.message))?;
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} source_inputs_ms={} dispositions={}",
                profile.id.as_str(),
                inputs_started.elapsed().as_millis(),
                inputs.dispositions.len(),
            );
        }

        let terrain_build_started = std::time::Instant::now();
        let terrain = {
            let planes = self.terrain.planes();
            build_classic_hd_terrain(
                profile,
                inputs.camera,
                ClassicHdTerrainResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    sky: &assets.sky,
                },
                ClassicHdSkyBank::Base256,
            )
            .map_err(|error| ClassicHdGraphError(error.to_string()))?
        };
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} terrain_build_ms={} lattice_vertices={} submissions_reflected={} submissions_primary={}",
                profile.id.as_str(),
                terrain_build_started.elapsed().as_millis(),
                terrain.lattice.vertex_count(),
                terrain.submissions.reflected.len(),
                terrain.submissions.primary.len(),
            );
        }
        let mut writer = ClassicHdFrameWriter::from_planes(
            profile,
            indices,
            provenance,
            generation,
            tick,
            input_hash,
            history_hash,
        )
        .map_err(ClassicHdGraphError::from)?;
        writer.indices_mut().copy_from_slice(&terrain.sky);
        writer.provenance_mut().fill(0x01);
        let terrain_raster_started = std::time::Instant::now();
        let terrain_receipt = rasterize_classic_hd_terrain(
            &mut writer,
            &terrain,
            &assets.terrain_atlas,
            &assets.tablesn,
            0x10,
        )
        .map_err(|error| ClassicHdGraphError(error.to_string()))?;
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} terrain_raster_ms={} index_hash={:016x}",
                profile.id.as_str(),
                terrain_raster_started.elapsed().as_millis(),
                byte_hash(writer.indices()),
            );
            eprintln!(
                "[CLASSIC HD TRACE] profile={} mode5_triangles={} mode26_triangles={} terrain_source_pixels={} terrain_written_pixels={} terrain_clipped_pixels={}",
                profile.id.as_str(),
                terrain_receipt.mode5_triangles,
                terrain_receipt.mode26_triangles,
                terrain_receipt.source_pixels,
                terrain_receipt.written_pixels,
                terrain_receipt.clipped_pixels,
            );
        }

        let disposition_started = std::time::Instant::now();
        // `source_render_inputs` already validates and orders the complete
        // disposition list. Reusing that owned list avoids rebuilding the
        // BTreeSet/imported-row admission pass a second time on every HD
        // frame; the authoritative state remains the same session snapshot.
        let dispositions = std::mem::take(&mut inputs.dispositions);
        let source_bank = self.world.source_sprite_tmap_bank().ok_or_else(|| {
            ClassicHdGraphError(
                "Classic HD owner render requires the CP1 source TMAP bank".to_owned(),
            )
        })?;
        let ground_heights = &self.terrain.planes().ground_heights;
        let mut plan = self
            .classic_hd_sprite_plan
            .take()
            .unwrap_or_else(|| ClassicHdSpritePlan::with_capacity(1_024));
        let plan_result = build_classic_hd_sprite_plan(
            profile,
            inputs.camera,
            &dispositions,
            source_bank,
            ground_heights,
            inputs.night,
            &mut plan,
        );
        if let Err(error) = plan_result {
            self.classic_hd_sprite_plan = Some(plan);
            return Err(ClassicHdGraphError(error.to_string()));
        }
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} dispositions={} sprite_plan_primary={} sprite_plan_legacy={} disposition_plan_ms={}",
                profile.id.as_str(),
                dispositions.len(),
                plan.primary.len(),
                plan.legacy.len(),
                disposition_started.elapsed().as_millis(),
            );
        }

        let tmap_clocks = self.selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Goat TMAP clocks".to_owned())
        })?;
        let archer_tmap_clocks = self.archer_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Archer TMAP clocks".to_owned())
        })?;
        let firefly_tmap_clocks = self.firefly_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Firefly TMAP clocks".to_owned())
        })?;
        let villager_tmap_clocks = self.villager_selector_frames.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no villager TMAP clocks".to_owned())
        })?;
        let fireball_tmap_clock = self.fireball_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Fireball TMAP clock".to_owned())
        })?;
        let possession_tmap_clock = self.possession_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Possession TMAP clock".to_owned())
        })?;
        let castle_tmap_clock = self.castle_selector_frame.as_ref().ok_or_else(|| {
            ClassicHdGraphError("Classic HD owner render has no Castle TMAP clock".to_owned())
        })?;
        let planes = self.terrain.planes();
        let terrain_projection = terrain.projection;
        let projection = OutdoorProjection {
            scale: terrain_projection.scale,
            pitch_offset: terrain_projection.pitch_offset,
            roll: terrain_projection.roll,
            center_x: terrain_projection.center_x,
            center_y: terrain_projection.center_y,
            viewport_width: terrain_projection.viewport_width,
            viewport_height: terrain_projection.viewport_height,
        };
        let sprite_raster_started = std::time::Instant::now();
        let sprite_receipt = match raster_level1_source_world_sprites(
            inputs.camera,
            projection,
            Level1TerrainFrameResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                terrain_atlas: &assets.terrain_atlas,
                tablesn: &assets.tablesn,
                sky: &assets.sky,
            },
            Level1WorldFrameResources {
                local_player_entity_index: inputs.local_player_entity_index,
                goats: &inputs.goats,
                frozen_actors: &inputs.frozen_actors,
                ambient_entities: &inputs.ambient_entities,
                frozen_actor_tmap_bank: Some(&assets.frozen_actor_tmap_bank),
                villager_tmap_clocks: Some(villager_tmap_clocks),
                mission_actors: &inputs.mission_actors,
                dynamic_residents: &inputs.dynamic_residents,
                mission_segments: &inputs.mission_segments,
                castle_root: inputs.castle_root,
                mana_spheres: &inputs.mana_spheres,
                mana_carrier: inputs.mana_carrier.as_ref(),
                map_chains: &inputs.map_chains,
                goat_tmap_clocks: tmap_clocks,
                archer_tmap_bank: &assets.archer_tmap_bank,
                archer_tmap_clocks,
                firefly_tmap_bank: &assets.firefly_tmap_bank,
                firefly_tmap_clocks,
                projectiles: &inputs.projectiles,
                impacts: &inputs.impacts,
                possessed_buildings: &inputs.possessed_buildings,
                combat_tmap_bank: &assets.combat_tmap_bank,
                static_world_tmap_bank: &assets.static_world_tmap_bank,
                economy_tmap_bank: &assets.economy_tmap_bank,
                fireball_tmap_clock,
                possession_tmap_clock,
                castle_tmap_clock,
                particle_parameters: &self.world.presentation().particles,
                source_slots: &inputs.source_slots,
                night: inputs.night,
            },
            SourceSpriteWorldResources {
                dispositions: &dispositions,
                tmap_bank: source_bank,
            },
            writer.indices_mut(),
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.classic_hd_sprite_plan = Some(plan);
                return Err(ClassicHdGraphError(error.to_string()));
            }
        };
        if profile_trace {
            eprintln!(
                "[CLASSIC HD TRACE] profile={} sprite_raster_ms={} primary_candidates={} primary_writes={} reflected_candidates={} shadow_candidates={} total_ms={}",
                profile.id.as_str(),
                sprite_raster_started.elapsed().as_millis(),
                sprite_receipt.primary.candidate_pixels,
                sprite_receipt.primary.framebuffer_writes,
                sprite_receipt.reflected.candidate_pixels,
                sprite_receipt.shadows.candidate_pixels,
                render_started.elapsed().as_millis(),
            );
        }

        let receipt = ClassicHdWorldReceipt {
            world_hash: byte_hash(writer.indices()),
            provenance_hash: byte_hash(writer.provenance()),
            terrain_rendered: true,
            typed_sprites_rendered: true,
            reflection_pass_rendered: sprite_receipt.reflection_pass_evaluated,
            shadow_pass_rendered: sprite_receipt.shadow_pass_evaluated,
            water_pass_rendered: false,
        };
        self.classic_hd_sprite_plan = Some(plan);
        self.classic_hd_last_world_receipt = Some(receipt);
        Ok(receipt)
    }

    fn validate_classic_hd_frame_request(
        &self,
        profile_id: u32,
        query: &McPresentationFrameQueryV2,
        index_buffer: &[u8],
        provenance_buffer: &[u8],
    ) -> AbiResult<ClassicHdRenderProfile> {
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        if index_buffer.len() != required || provenance_buffer.len() != required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required} index and provenance bytes",
                    profile.id.as_str()
                ),
            ));
        }
        if query.generation != self.generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "Classic HD generation {} does not match {}",
                    query.generation, self.generation
                ),
            ));
        }
        if query.tick != self.camera_history.tick {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "Classic HD tick {} does not match camera history {}",
                    query.tick, self.camera_history.tick
                ),
            ));
        }
        if query.viewport_width != profile.width as u16
            || query.viewport_height != profile.height as u16
            || query.screen_stride != profile.stride as u16
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "Classic HD query viewport {}x{} stride {} does not match profile {}x{} stride {}",
                    query.viewport_width,
                    query.viewport_height,
                    query.screen_stride,
                    profile.width,
                    profile.height,
                    profile.stride
                ),
            ));
        }
        if (query.game_paused != 0) != self.game_paused {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "Classic HD paused query does not match authoritative session state",
            ));
        }
        Ok(profile)
    }

    fn classic_hd_frame_copy(
        &mut self,
        profile_id: u32,
        query: McPresentationFrameQueryV2,
        index_buffer: &mut [u8],
        provenance_buffer: &mut [u8],
    ) -> AbiResult<McPresentationFrameInfoV2> {
        let profile = self.validate_classic_hd_frame_request(
            profile_id,
            &query,
            index_buffer,
            provenance_buffer,
        )?;
        let required = profile.extent();

        let source_input_hash = self.source_render_input_hash()?;
        let mut input_bytes = Vec::with_capacity(96);
        input_bytes.extend_from_slice(b"mc2-classic-hd-owner-input-v1\0");
        input_bytes.extend_from_slice(&source_input_hash.to_le_bytes());
        input_bytes.extend_from_slice(&profile.profile_revision.to_le_bytes());
        input_bytes.extend_from_slice(&query.presentation_revision.to_le_bytes());
        input_bytes.push(query.configured_blur_mode);
        input_bytes.push(query.blur_capability);
        input_bytes.push(query.high_speed_system);
        input_bytes.push(query.scratch_present);
        input_bytes.push(query.vga_resolution);
        input_bytes.push(query.spatial_smoothing);
        input_bytes.push(query.minimap_enabled);
        input_bytes.push(query.top_bar_enabled);
        input_bytes.push(query.menu_overlay);
        input_bytes.push(query.game_paused);
        input_bytes.push(query.level_end);
        input_bytes.push(query.objective_selector);
        input_bytes.push(query.objective_textbox);
        input_bytes.push(query.tutorial_or_briefing);
        input_bytes.push(query.help_popup);
        input_bytes.push(query.debug_text);
        let input_hash = byte_hash(&input_bytes);

        if self.classic_hd_graph.is_none() || self.classic_hd_profile_id != Some(profile_id) {
            self.classic_hd_graph = Some(
                ClassicHdRenderGraph::new(profile.clone(), self.generation)
                    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.0))?,
            );
            self.classic_hd_profile_id = Some(profile_id);
            self.classic_hd_sprite_plan = Some(ClassicHdSpritePlan::with_capacity(1_024));
            self.classic_hd_last_world_receipt = None;
        }

        let mut graph = self.classic_hd_graph.take().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD graph was not initialized",
            )
        })?;
        let had_history = graph.history_key().is_some();
        let generation = self.generation;
        let tick = self.camera_history.tick;
        let blur = ClassicHdBlurInput {
            configured_mode: query.configured_blur_mode,
            blur_capability: query.blur_capability != 0,
            high_speed_system: query.high_speed_system != 0,
            scratch_present: query.scratch_present != 0,
            menu_state: self.mission.player_menu_state,
            actual_speed: self.state.recoil_speed,
            spatial_smoothing: query.spatial_smoothing != 0,
            spatial_radius: 1,
        };
        let shading = Arc::clone(&self.classic_hd_shading);
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner render requires MC_ASSET_CACHE_PATH assets",
            )
        })?);
        let graph_result = graph.render_frame(
            ClassicHdGraphInput {
                generation,
                tick,
                profile_revision: profile.profile_revision,
                input_hash,
                reflections_enabled: true,
                dynamic_lighting_enabled: true,
                lighting_path: ClassicHdLightingPath::OwnerWorldAlreadyLit,
                water_enabled: false,
                blur,
            },
            Some(shading.as_ref()),
            &assets.tablesn,
            |indices, provenance| {
                self.render_classic_hd_world(
                    &profile, indices, provenance, generation, tick, input_hash, 0,
                )
            },
        );
        self.classic_hd_graph = Some(graph);
        let result =
            graph_result.map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.0))?;
        let world_receipt = self.classic_hd_last_world_receipt.ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD graph completed without an owner world receipt",
            )
        })?;
        index_buffer.copy_from_slice(
            self.classic_hd_graph
                .as_ref()
                .expect("Classic HD graph restored")
                .current_indices(),
        );
        provenance_buffer.copy_from_slice(
            self.classic_hd_graph
                .as_ref()
                .expect("Classic HD graph restored")
                .current_provenance(),
        );
        let (overlay_pixels_written, overlay_stage_count, _, _) = self
            .composite_classic_hd_owner_overlays(
                &profile,
                query,
                index_buffer,
                provenance_buffer,
            )?;
        let mut flags = MC_CLASSIC_HD_FRAME_PROFILE_RERENDER
            | MC_PRESENTATION_FRAME_HAS_PROVENANCE
            | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            | MC_PRESENTATION_FRAME_LIGHTING_COMPLETE;
        if world_receipt.typed_sprites_rendered {
            flags |= MC_CLASSIC_HD_FRAME_TYPED_SPRITES;
        }
        if world_receipt.reflection_pass_rendered {
            flags |= MC_CLASSIC_HD_FRAME_REFLECTIONS;
        }
        if world_receipt.shadow_pass_rendered {
            flags |= MC_CLASSIC_HD_FRAME_SHADOWS;
        }
        let effective_postprocess_mode = match result.receipt.blur_branch {
            ClassicHdBlurBranch::Direct { .. } => 0,
            ClassicHdBlurBranch::Temporal { mode } => mode,
        };
        Ok(McPresentationFrameInfoV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameInfoV2>(),
            generation,
            tick,
            presentation_revision: query.presentation_revision,
            immutable_input_hash: input_hash,
            width: profile.width as u16,
            height: profile.height as u16,
            stride: profile.stride as u16,
            viewport_width: profile.width as u16,
            index_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            provenance_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            flags,
            effective_postprocess_mode,
            history_state: u8::from(had_history || result.receipt.prior_main_hash != 0),
            newly_committed: u8::from(result.newly_committed),
            reserved_0: 0,
            index_hash: result.receipt.output_hash,
            provenance_hash: result.receipt.provenance_hash,
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            overlay_pixels_written,
            overlay_stage_count,
            reserved_1: 0,
            reserved: [0; 6],
        })
    }

    /// Copy only the owner-authentic HUD overlay into a profile-sized
    /// transparent surface. Unity Enhanced already renders the world through
    /// URP, so asking the Classic HD graph to redraw terrain and typed sprites
    /// here would duplicate visible work without changing the HUD pixels.
    fn classic_hd_owner_overlay_copy(
        &mut self,
        profile_id: u32,
        query: McPresentationFrameQueryV2,
        index_buffer: &mut [u8],
        provenance_buffer: &mut [u8],
    ) -> AbiResult<McPresentationFrameInfoV2> {
        let profile = self.validate_classic_hd_frame_request(
            profile_id,
            &query,
            index_buffer,
            provenance_buffer,
        )?;
        let required = profile.extent();
        index_buffer.fill(0);
        provenance_buffer.fill(0);
        let (overlay_pixels_written, overlay_stage_count, input_hash, newly_committed) = self
            .composite_classic_hd_owner_overlays(
                &profile,
                query,
                index_buffer,
                provenance_buffer,
            )?;
        Ok(McPresentationFrameInfoV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McPresentationFrameInfoV2>(),
            generation: self.generation,
            tick: self.camera_history.tick,
            presentation_revision: query.presentation_revision,
            immutable_input_hash: input_hash,
            width: profile.width as u16,
            height: profile.height as u16,
            stride: profile.stride as u16,
            viewport_width: profile.width as u16,
            index_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            provenance_bytes: u32::try_from(required).expect("Classic HD extent fits u32"),
            flags: MC_CLASSIC_HD_FRAME_OWNER_OVERLAY_ONLY
                | MC_PRESENTATION_FRAME_HAS_PROVENANCE
                | MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
                | MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE,
            effective_postprocess_mode: 0,
            history_state: 0,
            newly_committed,
            reserved_0: 0,
            index_hash: byte_hash(index_buffer),
            provenance_hash: byte_hash(provenance_buffer),
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            overlay_pixels_written,
            overlay_stage_count,
            reserved_1: 0,
            reserved: [0; 6],
        })
    }

    fn classic_hd_owner_overlay_frame(
        &mut self,
        query: McPresentationFrameQueryV2,
    ) -> AbiResult<sim_core::presentation_frame::PresentationFrameResult> {
        let mut hud_query = query;
        hud_query.viewport_width = LEVEL1_VIEW_WIDTH as u16;
        hud_query.viewport_height = LEVEL1_VIEW_HEIGHT as u16;
        hud_query.screen_stride = LEVEL1_VIEW_WIDTH as u16;
        // The owner HUD is a direct compositor surface. The HD graph owns its
        // separate blur/history decision, so do not blur the source HUD a
        // second time while extracting its marked overlay pixels.
        hud_query.configured_blur_mode = 0;
        hud_query.blur_capability = 1;
        hud_query.high_speed_system = 1;
        hud_query.scratch_present = 1;
        hud_query.vga_resolution = 1;
        hud_query.spatial_smoothing = 0;

        let result = self.presentation_frame(hud_query)?;
        if !result.frame.compositor_complete()
            || result.frame.overlay_pixels_written == 0
            || result.frame.overlay_receipts.is_empty()
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner compositor did not produce a complete HUD overlay",
            ));
        }
        Ok(result)
    }

    /// Composite the owner-authentic fixed-pipeline overlay pixels over the
    /// profile-sized Classic HD world. The HD Rust graph remains authoritative
    /// for terrain and typed sprites; this imports only pixels explicitly
    /// marked as overlays by the existing 320x200 owner compositor, preserving
    /// the original HUD art and layout while fitting it into the profile's
    /// 4:3 surface. The same helper also serves the Enhanced overlay-only
    /// endpoint, where the destination begins transparent instead of carrying
    /// the HD world.
    fn composite_classic_hd_owner_overlays(
        &mut self,
        profile: &ClassicHdRenderProfile,
        query: McPresentationFrameQueryV2,
        indices: &mut [u8],
        provenance: &mut [u8],
    ) -> AbiResult<(u32, u16, u64, u8)> {
        let base_width = LEVEL1_VIEW_WIDTH as u32;
        let base_height = LEVEL1_VIEW_HEIGHT as u32;
        if profile.width < base_width || profile.height < base_height {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "Classic HD profile {} cannot fit the fixed owner HUD",
                    profile.id.as_str()
                ),
            ));
        }
        if profile.stride != profile.width {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!(
                    "Classic HD profile {} has an incompatible owner HUD stride",
                    profile.id.as_str()
                ),
            ));
        }

        let owner_result = self.classic_hd_owner_overlay_frame(query)?;
        let owner_frame = owner_result.frame;

        let base_width = usize::try_from(base_width).expect("base width fits usize");
        let base_height = usize::try_from(base_height).expect("base height fits usize");
        let output_width = usize::try_from(profile.width).expect("Classic HD width fits usize");
        let output_height = usize::try_from(profile.height).expect("Classic HD height fits usize");
        let output_stride = usize::try_from(profile.stride).expect("Classic HD stride fits usize");
        let mut written = 0_u32;
        for source_y in 0..base_height {
            for source_x in 0..base_width {
                let source_offset = source_y * base_width + source_x;
                if owner_frame.provenance[source_offset] & OVERLAY_PROVENANCE == 0 {
                    continue;
                }
                let value = owner_frame.indices[source_offset];
                let output_x = source_x * output_width / base_width;
                let output_x_end = (source_x + 1) * output_width / base_width;
                let output_y = source_y * output_height / base_height;
                let output_y_end = (source_y + 1) * output_height / base_height;
                for y in output_y..output_y_end {
                    let row = y * output_stride;
                    for x in output_x..output_x_end {
                        let output_offset = row + x;
                        indices[output_offset] = value;
                        provenance[output_offset] |= OVERLAY_PROVENANCE;
                        written = written
                            .checked_add(1)
                            .expect("Classic HD overlay pixel count fits u32");
                    }
                }
            }
        }

        if written == 0 {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Classic HD owner compositor marked no pixels for the HUD overlay",
            ));
        }

        Ok((
            written,
            u16::try_from(owner_frame.overlay_receipts.len())
                .expect("Classic HD overlay stage count fits u16"),
            owner_frame.immutable_input_hash,
            u8::from(owner_result.newly_committed),
        ))
    }

    fn world_frame(&mut self) -> AbiResult<Level1TerrainFrame> {
        let ambient_entities = self.mission.ambient_entities().copied().collect::<Vec<_>>();
        let frame = self.render_world_frame(&ambient_entities, &self.renderable_map_chains)?;
        self.commit_world_frame_feedback(&frame)?;
        Ok(frame)
    }

    fn commit_world_frame_feedback(&mut self, frame: &Level1TerrainFrame) -> AbiResult<()> {
        let mask = frame.goat_selector_mask;
        self.selector_frames
            .as_mut()
            .expect("world-frame selector clocks were validated")
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.archer_selector_frames
            .as_mut()
            .expect("world-frame Archer selector clocks were validated")
            .latch_draw_attempts(frame.archer_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.firefly_selector_frames
            .as_mut()
            .expect("world-frame Firefly selector clocks were validated")
            .latch_draw_attempts(frame.firefly_selector_mask);
        self.villager_selector_frames
            .as_mut()
            .expect("world-frame villager selector clocks were validated")
            .latch_draw_attempts(frame.frozen_actor_selector_mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        self.fireball_selector_frame
            .as_mut()
            .expect("world-frame Fireball selector clock was validated")
            .latch_draw_attempt(frame.fireball_draw_attempted);
        self.possession_selector_frame
            .as_mut()
            .expect("world-frame Possession selector clock was validated")
            .latch_draw_attempt(frame.possession_draw_attempted);
        self.castle_selector_frame
            .as_mut()
            .expect("world-frame Castle selector clock was validated")
            .latch_draw_attempt(frame.castle_draw_attempted);
        let feedback_tick = self.state.tick;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == feedback_tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((feedback_tick, combined_mask));
        Ok(())
    }

    fn presentation_frame(
        &mut self,
        query: McPresentationFrameQueryV2,
    ) -> AbiResult<sim_core::presentation_frame::PresentationFrameResult> {
        let assets = Arc::clone(self.terrain_frame_assets.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "presentation frame unavailable without qualified owner cache",
            )
        })?);
        if query.menu_overlay > 6 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "presentation menu overlay {} is not supported",
                    query.menu_overlay
                ),
            ));
        }
        let hud = level1_hud_snapshot(self);
        let spell_menu = level1_spell_menu_snapshot(self);
        let minimap_entities_abi = level1_minimap_entity_commands(self);
        let minimap_entities = minimap_entities_abi
            .iter()
            .map(|command| Level1MinimapEntityCommand {
                source_slot: command.source_slot,
                x: command.x,
                y: command.y,
                selector: command.selector,
                kind: command.kind,
                palette_index: command.palette_index,
                size: command.size,
                anchor: command.anchor,
                help_selector: command.help_selector,
            })
            .collect::<Vec<_>>();
        let map_entities = level1_map_entity_commands(self);
        let spell_slot = |slot: McLevel1HudSpellSlotV2| LiveSpellSlot {
            holder_entity_index: slot.holder_entity_index,
            model: slot.model,
            tier: slot.tier,
            enabled_flags: slot.enabled_flags,
            active_value: slot.active_value,
            max_mana: slot.max_mana,
            mana_regen: slot.mana_regen,
            caster_mana: slot.caster_mana,
            frame_selector: slot.frame_selector,
            icon_selector: slot.icon_selector,
        };
        let fire = self.context.spell_fire.as_ref();
        let mut family_available = std::array::from_fn(|family| {
            u8::from(
                assets
                    .indexed_overlays
                    .spell_menu_profile(family)
                    .expect("qualified cache has all 26 spell families")
                    .family_header_tier_count
                    != 0,
            )
        });
        // DrawBottomSpellsMenu suppresses the cave-only family in the outdoor
        // Level 1 profile even though the source descriptor itself is live.
        family_available[25] = 0;
        let holder_entity_index = fire.map_or([0_u16; 26], |fire| fire.enabled_holder_indices);
        let family_unlocked_tier = fire.map_or([0_i8; 26], |fire| fire.spell_levels);
        let family_selected_tier = fire.map_or([0_i8; 26], |fire| fire.family_selected_tiers);
        let family_hand_eligibility = fire.map_or([0_u8; 26], |fire| fire.spell_hand_eligibility);
        let family_experience = fire.map_or([0_i32; 26], |fire| fire.spell_experience);
        let family_enabled_flags = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .family_flags
        });
        let family_active_value = std::array::from_fn(|family| {
            let holder = holder_entity_index[family];
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|candidate| candidate.entity_index == holder)
            })
            .map_or(0, |holder| holder.active_value)
        });
        let family_mana_cost = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .mana_cost
        });
        let family_max_mana_limit = std::array::from_fn(|family| {
            assets
                .indexed_overlays
                .spell_menu_profile(family)
                .expect("qualified cache has all 26 spell families")
                .max_mana_limit
        });
        let debug_source = &self.world.entities()[usize::from(self.state.entity_index)];
        let debug_record = debug_source.record_bytes();
        let read_i16 =
            |offset: usize| i16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_u16 =
            |offset: usize| u16::from_le_bytes([debug_record[offset], debug_record[offset + 1]]);
        let read_i32 = |offset: usize| {
            i32::from_le_bytes([
                debug_record[offset],
                debug_record[offset + 1],
                debug_record[offset + 2],
                debug_record[offset + 3],
            ])
        };
        let map_mode = matches!(self.mission.player_menu_state, 6 | 7 | 8 | 11 | 12 | 14);
        let bridge = build_level1_final_main(
            &assets.indexed_overlays,
            Level1PresentationLive {
                flight: self.state,
                menu_state: self.mission.player_menu_state,
                player_life: self.mission.player_life,
                player_immobilize_ticks: self.state.immobilize_counter.cast_unsigned().into(),
                terrain: self.terrain.planes(),
                hud: LiveHud {
                    mana_color_index: hud.mana_color_index,
                    mana_max_color_index: hud.mana_max_color_index,
                    life_color_index: hud.life_color_index,
                    color_phase_1: hud.color_phase_1,
                    color_phase_2: hud.color_phase_2,
                    player_hit_frame_ticks: hud.player_hit_frame_ticks,
                    castle_hit_frame_ticks: hud.castle_hit_frame_ticks,
                    global_mana_denominator: hud.global_mana_denominator,
                    secured_mana_component: hud.secured_mana_component,
                    goal_percent: hud.goal_percent,
                    player_life: self.mission.player_life,
                    player_max_life: self.mission.player_max_life,
                    player_mana: self.mission.player_mana,
                    player_max_mana: self.mission.player_max_mana,
                    castle_active: self.mission.castle_exists,
                    castle_level: self.mission.castle_level,
                    castle_life: self.mission.castle_life,
                    castle_max_life: self.mission.castle_max_life,
                    castle_mana: self.mission.castle_mana,
                    castle_mana_capacity: self.mission.castle_mana_capacity,
                    player_status_flags: hud.player_status_flags,
                    player_status_word: u16::from(self.mission.exit_player_status_flags),
                    balloon_level: 0,
                    balloon_life: [0; 3],
                    balloon_max_life: [0; 3],
                    balloon_mana: [0; 3],
                    balloon_max_mana: [0; 3],
                    left: spell_slot(hud.left),
                    right: spell_slot(hud.right),
                },
                spell_menu: LiveSpellMenu {
                    selected_ui_index: spell_menu.selected_ui_index,
                    selected_tier: spell_menu.selected_tier,
                    left_family: spell_menu.left_family,
                    right_family: spell_menu.right_family,
                    family_available,
                    holder_entity_index,
                    family_unlocked_tier,
                    family_selected_tier,
                    family_hand_eligibility,
                    family_enabled_flags,
                    family_active_value,
                    family_experience,
                    family_mana_cost,
                    family_max_mana_limit,
                    castle_mana: self.mission.castle_mana,
                    caster_mana: self.mission.player_mana,
                    player_colour_0: hud.mana_color_index,
                    player_colour_1: hud.mana_max_color_index,
                    active_suppression_phase: hud.color_phase_1 != 0,
                    cursor_visible: self.presentation_ui.cursor_visible
                        && matches!(self.presentation_ui.joystick, 4 | 5 | 6 | 8 | 11 | 12 | 13),
                    cursor_x: self.presentation_ui.mouse_x / 2,
                    cursor_y: self.presentation_ui.mouse_y / 2,
                },
                menu: self.presentation_ui,
                debug_entity: LiveDebugEntity {
                    pointer_value: u32::from_le_bytes([
                        debug_record[0],
                        debug_record[1],
                        debug_record[2],
                        debug_record[3],
                    ]),
                    source_slot: debug_source.index,
                    class: debug_source.class,
                    model: debug_source.model,
                    action: debug_source.action,
                    life: debug_source.life,
                    max_life: debug_source.max_life,
                    actual_speed: debug_source.actual_speed,
                    min_speed: read_i16(132),
                    max_speed: read_i16(134),
                    position: self.state.position,
                    source_id: debug_source.source_id,
                    parent_id: read_u16(40),
                    mana: debug_source.mana,
                    max_mana: read_i32(140),
                },
                minimap_entities: &minimap_entities,
                minimap_marks: self.mission.minimap_marks(&self.state),
                map_entities: &map_entities,
                map_marks: self.mission.map_marks(&self.state),
                objective_selector: self.mission.current_text_selector,
                objective_ticks: u16::from(self.mission.objective_textbox_ticks),
                level_completed: self.mission.level_completed,
                exit_sequence_active: self.mission.exit_sequence_active,
                exit_display_blank: self.mission.exit_display_blank,
                exit_blur_mode: self.mission.exit_blur_mode,
                notification_active: self.mission.player_notification_active,
                notification_id: self.mission.player_notification_id,
                notification_state: self.mission.player_notification_state,
                notification_timer: self.mission.player_notification_timer,
                notification_player_name: self.presentation_ui.notification_player_name,
                level_number: 1,
                player_turn: self.source_turn,
                sound_number: self.sound_number,
                game_timer_turn: self.game_timer_turn,
                debug_fps: self.debug_fps,
                active_entity_count: u16::try_from(
                    self.mission
                        .source_slots()
                        .iter()
                        .filter(|slot| slot.active)
                        .count(),
                )
                .expect("source arena count fits u16"),
                heap_size: self.debug_heap_size,
                memory_used: self.debug_memory_used,
                memory_free: self.debug_memory_free,
            },
            PresentationBridgeQuery {
                minimap_enabled: query.minimap_enabled != 0,
                top_bar_enabled: query.top_bar_enabled != 0,
                menu_overlay: query.menu_overlay,
                game_paused: query.game_paused != 0,
                level_end: query.level_end != 0,
                objective_selector: (query.objective_selector != u8::MAX)
                    .then_some(query.objective_selector),
                objective_textbox: query.objective_textbox != 0,
                tutorial_or_briefing: query.tutorial_or_briefing != 0,
                help_popup: query.help_popup != 0,
                debug_flags: query.debug_text,
            },
        )
        .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        let current_lighting_receipt = self
            .mission
            .dynamic_lighting_execution_receipt(&self.terrain.planes().shading)
            .map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error))?;
        if current_lighting_receipt != self.lighting_receipt {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "dynamic-light state/shading changed without an executed scheduler receipt",
            ));
        }
        let source_render_inputs = self.source_render_inputs(map_mode)?;
        let notification_feedback = bridge.notification_feedback;
        let input = PresentationFrameInput {
            generation: query.generation,
            tick: query.tick,
            presentation_revision: query.presentation_revision,
            state_hash: state_hash(&self.state),
            source_arena_hash: self.mission.source_arena_hash_v2(),
            source_render_input_hash: self.source_render_input_hash_for(&source_render_inputs)?,
            lighting_hash: self.mission.dynamic_lighting_hash_v1(),
            camera_hash: camera_history_hash(self.camera_history),
            source_reflections_expected: i32::from(self.camera_history.z)
                < sim_core::terrain_reflection::REFLECTION_EYE_Z_LIMIT,
            postprocess: IndexedPostprocessInput {
                configured_mode: if self.mission.exit_sequence_active {
                    self.mission.exit_blur_mode
                } else {
                    query.configured_blur_mode
                },
                blur_capability: query.blur_capability != 0,
                high_speed_system: query.high_speed_system != 0,
                scratch_present: query.scratch_present != 0,
                vga_resolution: query.vga_resolution,
                menu_state: self.mission.player_menu_state,
                viewport_width: query.viewport_width,
                viewport_height: query.viewport_height,
                screen_stride: query.screen_stride,
                actual_speed: self.state.actual_speed,
                spatial_smoothing: query.spatial_smoothing != 0,
            },
            overlay: bridge.overlay,
            lighting_receipt: Some(self.lighting_receipt),
        };
        let mut goat_candidate = self.selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Goat selector clocks",
            )
        })?;
        let mut archer_candidate = self.archer_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Archer selector clocks",
            )
        })?;
        let mut firefly_candidate = self.firefly_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Firefly selector clocks",
            )
        })?;
        let mut villager_candidate = self.villager_selector_frames.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no villager selector clocks",
            )
        })?;
        let mut fireball_candidate = self.fireball_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Fireball selector clock",
            )
        })?;
        let mut possession_candidate = self.possession_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Possession selector clock",
            )
        })?;
        let mut castle_candidate = self.castle_selector_frame.clone().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "presentation source renderer has no Castle selector clock",
            )
        })?;
        let mut presentation =
            std::mem::replace(&mut self.presentation_frame, PresentationFrameState::new(0));
        let mut feedback_candidate = None;
        let result =
            presentation.finalize(input, &assets.tablesn, &assets.indexed_overlays, || {
                let frame = self
                    .render_source_world_frame(&source_render_inputs)
                    .map_err(|error| PresentationFrameError(error.message))?;
                let receipt = frame
                    .source_execution_receipt()
                    .map_err(|error| PresentationFrameError(error.0))?;
                goat_candidate
                    .latch_draw_attempts(frame.goat_selector_mask)
                    .map_err(PresentationFrameError)?;
                archer_candidate
                    .latch_draw_attempts(frame.archer_selector_mask)
                    .map_err(PresentationFrameError)?;
                firefly_candidate.latch_draw_attempts(frame.firefly_selector_mask);
                villager_candidate
                    .latch_draw_attempts(frame.frozen_actor_selector_mask)
                    .map_err(PresentationFrameError)?;
                fireball_candidate.latch_draw_attempt(frame.fireball_draw_attempted);
                possession_candidate.latch_draw_attempt(frame.possession_draw_attempted);
                castle_candidate.latch_draw_attempt(frame.castle_draw_attempted);
                feedback_candidate = Some((
                    goat_candidate,
                    archer_candidate,
                    firefly_candidate,
                    villager_candidate,
                    fireball_candidate,
                    possession_candidate,
                    castle_candidate,
                    frame.goat_selector_mask,
                ));
                Ok(SourceRenderedFrame {
                    map_world: map_mode.then(|| frame.indices.clone()),
                    indices: frame.indices,
                    receipt,
                })
            });
        self.presentation_frame = presentation;
        let result =
            result.map_err(|error| AbiError::new(MC_STATUS_GENERATION_MISMATCH, error.0))?;
        if result.newly_committed {
            let (goat, archer, firefly, villager, fireball, possession, castle, goat_mask) =
                feedback_candidate
                    .expect("newly committed presentation owns validated clock candidates");
            self.selector_frames = Some(goat);
            self.archer_selector_frames = Some(archer);
            self.firefly_selector_frames = Some(firefly);
            self.villager_selector_frames = Some(villager);
            self.fireball_selector_frame = Some(fireball);
            self.possession_selector_frame = Some(possession);
            self.castle_selector_frame = Some(castle);
            self.selector_draw_feedback = Some((self.state.tick, goat_mask));
            if let Some((active, state, timer)) = notification_feedback {
                self.notification_draw_feedback = Some((self.state.tick, active, state, timer));
            }
        }
        Ok(result)
    }

    fn selector_frame_batch(
        &self,
        start_ordinal: u32,
        written: u32,
    ) -> AbiResult<McSelectorFrameBatchV2> {
        let frames = self.selector_frames.as_ref().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable: MC_ASSET_CACHE_PATH was not supplied when the session opened",
            )
        })?;
        let (last_draw_tick, last_draw_mask, has_draw_feedback) = self
            .selector_draw_feedback
            .map_or((0, 0, 0), |(tick, mask)| (tick, u64::from(mask), 1));
        let written_usize = usize::try_from(written)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector written count exceeds usize"))?;
        let start_usize = usize::try_from(start_ordinal)
            .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "selector start exceeds usize"))?;
        Ok(McSelectorFrameBatchV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSelectorFrameBatchV2>(),
            generation: self.generation,
            tick: self.state.tick,
            selector_state_hash: selector_state_hash_v1(frames, self.selector_draw_feedback),
            last_draw_tick,
            last_draw_mask,
            valid_draw_mask: u64::from(GOAT_TMAP_DRAW_MASK),
            start_ordinal,
            written,
            total_count: GOAT_TMAP_SELECTORS.len() as u32,
            has_more: u8::from(start_usize + written_usize < GOAT_TMAP_SELECTORS.len()),
            has_draw_feedback,
            reserved_0: 0,
            reserved: [0; 4],
        })
    }

    fn evaluate_goat_draw_admission(&mut self) -> AbiResult<McSelectorFrameBatchV2> {
        if self.world.goat_tmap_bank().is_none() {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "Goat CPU draw admission unavailable without qualified owner-cache descriptors",
            ));
        }
        let camera = self.camera_history;
        let slope = self.camera_slope_sample;
        let planes = self.terrain.planes();
        let render_x = camera.x.wrapping_add(slope.offset_x.cast_unsigned());
        let render_y = camera.y.wrapping_add(slope.offset_y.cast_unsigned());
        let lattice = outdoor_grid(OutdoorGridInput {
            render_x,
            render_y,
            yaw: camera.yaw,
            turn: camera.source_turn,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let visited = outdoor_billboard_visit_map(&lattice, camera.yaw);
        let admission = evaluate_goat_draw_admission(
            self.live_world.goats(),
            GoatDrawCamera {
                render_x,
                render_y,
                yaw: camera.yaw,
            },
            &visited,
            true,
        );
        let mask = admission.selector_mask;
        let frames = self.selector_frames.as_mut().ok_or_else(|| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selector-frame state unavailable without qualified owner cache",
            )
        })?;
        frames
            .latch_draw_attempts(mask)
            .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
        let combined_mask = self
            .selector_draw_feedback
            .filter(|(tick, _)| *tick == self.state.tick)
            .map_or(mask, |(_, prior_mask)| prior_mask | mask);
        self.selector_draw_feedback = Some((self.state.tick, combined_mask));
        self.goat_draw_admission = Some(admission);
        let mut receipt = self.selector_frame_batch(0, 0)?;
        receipt.has_more = 0;
        Ok(receipt)
    }

    fn snapshot(&self) -> McSessionSnapshotV2 {
        let player = &self.world.players()[usize::from(self.world.runtime().local_player_slot)];
        let fire = self.context.spell_fire.as_ref();
        let selected = fire.map_or(0, |fire| fire.selected_spell_ui_index);
        let left_holder = fire.map_or(0, |fire| fire.left_holder_index);
        let right_holder = fire.map_or(0, |fire| fire.right_holder_index);
        let selected_holder = fire.map_or(0, |fire| fire.selected_holder_index);
        let holder_model = |holder_index| {
            fire.and_then(|fire| {
                fire.holders
                    .iter()
                    .find(|holder| holder.entity_index == holder_index)
                    .map(|holder| holder.model)
            })
        };
        let left_spell_slot = holder_model(left_holder).unwrap_or(player.left_spell_slot);
        let right_spell_slot = holder_model(right_holder).unwrap_or(player.right_spell_slot);
        let caster_mana = fire.map_or(0, |fire| fire.caster.mana);
        McSessionSnapshotV2 {
            abi_version: MC_NATIVE_ABI_VERSION,
            struct_size: struct_size::<McSessionSnapshotV2>(),
            generation: self.generation,
            tick: self.state.tick,
            event_head: self.event_head,
            state_hash: state_hash(&self.state),
            conformance_hash: conformance_hash_v2(&self.state, &self.context),
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
            left_spell_slot,
            right_spell_slot,
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

    #[cfg(test)]
    fn event_capacity(&self) -> usize {
        self.event_capacity
    }

    #[cfg(not(test))]
    fn event_capacity(&self) -> usize {
        EVENT_CAPACITY
    }

    fn append_event(&mut self, event: McEventV2) {
        if self.events.len() == self.event_capacity()
            && let Some(expired) = self.events.pop_front()
        {
            // Audio sidecars have an explicit client release lifecycle. They
            // may outlive the bounded event journal while paused or while a
            // client drains a multi-step backlog.
            let _ = expired;
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

    fn step(&mut self, command: PlayerCommand) {
        #[cfg(test)]
        self.immediate_released_holder_handoff.clear();
        if self.game_paused {
            self.mission.begin_paused_outer_presentation();
            // `UpdateEntities_57730` stores its global-RNG draw before testing
            // GAME_PAUSED. Preserve that source-visible prefix while keeping
            // every post-gate entity action and unrelated turn clock frozen.
            self.mission.advance_paused_update_rng();
            self.mission.finish_paused_outer_presentation(&self.state);
            let state_hash = state_hash(&self.state);
            for event in self.mission.events().to_vec() {
                let record = self.encode_mission_event(event, state_hash);
                self.append_event(record);
            }
            // `PlayEntitySounds_6E150` is gated while the selected
            // single-player/no-CD profile is paused (`byte_38591 == 0`).
            // Keep prepared rows unresolved so the first unpaused flush
            // coalesces them with that frame's rows using the source pool.
            return;
        }
        if let Some((_, active, state, timer)) = self.notification_draw_feedback.take() {
            self.mission.player_notification_active = active;
            self.mission.player_notification_state = state;
            self.mission.player_notification_timer = timer;
            if !active {
                self.mission.player_notification_id = 0;
            }
        }
        self.mission.finish_hud_draw(self.state.tick);
        if let (Some(frames), Some(bank)) =
            (self.selector_frames.as_mut(), self.world.goat_tmap_bank())
        {
            frames.service_unpaused_outer_turn(bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.archer_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.archer_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.firefly_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frames.service_unpaused_outer_turn(&assets.firefly_tmap_bank);
        }
        if let (Some(frames), Some(assets)) = (
            self.villager_selector_frames.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            // remc2's C++ transcription omits the clear-store at the common
            // texture clock service. We retain the already accepted
            // executable-order clear-before-advance policy used by Goat and
            // Archer clocks.
            frames.service_unpaused_outer_turn(&assets.frozen_actor_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.fireball_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.possession_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.combat_tmap_bank);
        }
        if let (Some(frame), Some(assets)) = (
            self.castle_selector_frame.as_mut(),
            self.terrain_frame_assets.as_ref(),
        ) {
            frame.service_unpaused_outer_turn(&assets.economy_tmap_bank);
        }
        self.mission.advance_exit_palette_frame();
        self.source_turn = self.source_turn.wrapping_add(1);
        self.game_timer_turn = self.game_timer_turn.wrapping_add(1);
        if !self.mission.exit_sequence_owns_player_action()
            && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
        {
            if command.opcode == 8 {
                self.camera_fov = self.camera_fov.wrapping_add(i16::from(command.argument_1));
            }
            self.camera_look_yaw = command.argument_6.cast_signed();
            self.camera_look_pitch = command.argument_8.cast_signed();
        }
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
            self.terrain.ground_heights(),
            self.camera_history.x,
            self.camera_history.y,
        );
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let holders_before = self
            .context
            .spell_fire
            .as_ref()
            .map_or_else(Vec::new, |fire| fire.holders.clone());
        self.context.events.clear();
        let world = Arc::clone(&self.world);
        let player_control_flags = world
            .entities()
            .get(usize::from(self.state.entity_index))
            .map_or(0, |entity| entity.control_flags);
        let initial_target = [GoatPlayerTarget {
            entity_index: self.state.entity_index,
            position: self.state.position,
            control_flags: player_control_flags,
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
                )
                .chain(self.live_world.houses().iter().map(house_collision_target)),
        );
        {
            let (_, shading) = self.terrain.dynamic_lighting_planes();
            self.mission
                .subtract_dynamic_lighting_before_updates(shading);
        }
        self.mission.begin_source_turn_at(self.state.position);
        let mut goat_events = self
            .reclaim_imported_source_slots()
            .into_iter()
            .map(|event| (event, self.mission.global_rng()))
            .collect::<Vec<_>>();
        if self.mission.take_group_7_villager_release() {
            let released = self.live_world.release_villager_stage_group_7();
            debug_assert_eq!(released, 28);
        }
        let begin_goat_events = self.live_world.begin_source_turn(&initial_target);
        self.apply_goat_event_side_effects(world.as_ref(), &begin_goat_events);
        goat_events.extend(
            begin_goat_events
                .into_iter()
                .map(|event| (event, self.mission.global_rng())),
        );
        let mut source_views =
            level1_turn_start_source_views(&self.mission, &self.state, &self.live_world);
        let frozen_house_source_views = source_views.clone();
        let mut evidence = Vec::new();
        let mut released_holder_handoff = false;
        for source_slot in 1_u16..1_000 {
            if let Some((owner_entity_index, payload)) =
                self.mission.take_building_possession_mailbox(source_slot)
            {
                self.live_world
                    .submit_house_possession(source_slot, owner_entity_index, payload)
                    .expect("possession mailbox targets a live eligible house");
            }
            let action_51_blocked = self
                .mission
                .current_house_action_51_blocked(&frozen_house_source_views, source_slot);
            let house_events = self.live_world.step_house_source_slot_with_action_51_block(
                source_slot,
                &self.terrain,
                action_51_blocked,
            );
            self.apply_house_event_side_effects(&house_events, &frozen_house_source_views);
            let player_target = [GoatPlayerTarget {
                entity_index: self.state.entity_index,
                position: self.state.position,
                control_flags: player_control_flags,
            }];
            let slot_goat_events =
                self.live_world
                    .step_source_slot(source_slot, &self.terrain, &player_target);
            let villager_effects = self.live_world.step_villager_source_slot(
                source_slot,
                &self.terrain,
                &source_views,
            );
            self.mission
                .apply_imported_villager_effects(&villager_effects);
            self.apply_goat_event_side_effects(world.as_ref(), &slot_goat_events);
            goat_events.extend(
                slot_goat_events
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
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
            if let Some(dolmen) = world
                .entities()
                .get(usize::from(source_slot))
                .filter(|entity| {
                    entity.active && entity.class == 2 && entity.model == 2 && entity.action == 6
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
                        sim_core::level1_mission::PLAYER_ACTION_ACTIVE => {
                            step_outer_turn_with_context(
                                &mut self.state,
                                command,
                                0,
                                world.profile(),
                                &self.terrain,
                                &mut self.context,
                            )
                        }
                        sim_core::level1_mission::PLAYER_ACTION_DYING => {
                            vec![step_entity_body_with_context(
                                &mut self.state,
                                world.profile(),
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
                    let lifecycle_action = self.mission.player_action;
                    self.mission
                        .advance_player_lifecycle_after_flight_with_proposed(
                            &self.terrain,
                            &mut self.state,
                            &mut self.context,
                            flight_proposed,
                        );
                    let released_holder_this_pass = lifecycle_action
                        == sim_core::level1_mission::PLAYER_ACTION_DYING
                        && self.mission.player_lifecycle_trace().iter().any(|entry| {
                            entry.step
                                == sim_core::level1_mission::PLAYER_LIFECYCLE_TRACE_HOLDER_FAMILY
                        });
                    released_holder_handoff |= released_holder_this_pass;
                    if released_holder_this_pass {
                        for released in self.mission.released_spell_holders().iter().flatten() {
                            let tile = (
                                released.position.x.to_be_bytes()[0],
                                released.position.y.to_be_bytes()[0],
                            );
                            self.renderable_map_chains
                                .sync_entity(RenderableMapSeed {
                                    entity_index: released.source_entity_index,
                                    active: true,
                                    x: released.position.x,
                                    y: released.position.y,
                                })
                                .expect("qualified immediate spell holder map insertion");
                            debug_assert!(
                                self.renderable_map_chains
                                    .entity_chain(tile.0, tile.1)
                                    .contains(&released.source_entity_index),
                                "released holder must be visible before later source slots dispatch"
                            );
                            #[cfg(test)]
                            self.immediate_released_holder_handoff.push((
                                released.family,
                                released.source_entity_index,
                                tile.0,
                                tile.1,
                            ));
                        }
                    }
                    if let Some(marker) = self
                        .mission
                        .player_death_marker()
                        .filter(|marker| marker.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: marker.source_entity_index,
                                active: true,
                                x: marker.position.x,
                                y: marker.position.y,
                            })
                            .expect("qualified player death marker map insertion");
                    }
                    for burst in self
                        .mission
                        .death_bursts()
                        .iter()
                        .filter(|burst| burst.active)
                    {
                        self.renderable_map_chains
                            .sync_entity(RenderableMapSeed {
                                entity_index: burst.source_entity_index,
                                active: true,
                                x: burst.position.x,
                                y: burst.position.y,
                            })
                            .expect("qualified death burst map insertion");
                    }
                }
                // A multi-pass outer turn clears `accepted_models` when each
                // pass latches the command. Detect the source-visible holder
                // transition instead, matching the direct Rust session.
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
                (
                    fireball,
                    possession,
                    castle.then_some(self.state.yaw.wrapping_add(self.camera_look_yaw) & 0x07ff),
                )
            } else {
                (false, false, None)
            };
            // remc2 updates a moving entity's terrain-cell link during that
            // entity's source action. Goats/villagers have already acted
            // above, so expose their current chain positions before the Rust
            // mission dispatches the same source slot. Mission-owned rows are
            // synchronized again after their action below.
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated pre-dispatch source map update");
            let mission_event_start = self.mission.events().len();
            if !self.mission.exit_sequence_owns_player_action()
                && self.mission.player_action == sim_core::level1_mission::PLAYER_ACTION_ACTIVE
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
                self.camera_look_yaw,
                self.camera_look_pitch,
            );
            self.mission
                .apply_pending_player_transients(&mut self.state);
            for seed in apply_level1_source_effects(
                &mut self.mission,
                &mut self.live_world,
                &source_effects,
            )
            .expect("resident source effects apply after slot dispatch")
            {
                self.renderable_map_chains
                    .sync_entity(seed)
                    .expect("resident source effect updates presentation map chain");
            }
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
            self.sync_source_slot_renderable_records(source_slot)
                .expect("validated source renderable map update");
            goat_events.extend(
                self.reclaim_imported_source_slots()
                    .into_iter()
                    .map(|event| (event, self.mission.global_rng())),
            );
        }
        self.live_world.finish_source_turn();
        {
            let (ground_heights, shading) = self.terrain.dynamic_lighting_planes();
            self.mission.service_dynamic_lighting_after_updates(
                &self.state,
                &source_views,
                ground_heights,
                shading,
            );
            self.lighting_receipt = self
                .mission
                .dynamic_lighting_execution_receipt(shading)
                .expect("qualified C ABI dynamic-light execution has fixed shading extent");
            Arc::make_mut(&mut self.classic_hd_shading).copy_from_slice(shading);
        }
        self.mission.finish_source_turn(&self.state);
        self.sync_supported_renderable_records()
            .expect("validated final source renderable map update");
        if released_holder_handoff {
            for released in self.mission.released_spell_holders().iter().flatten() {
                self.renderable_map_chains
                    .sync_entity(RenderableMapSeed {
                        entity_index: released.source_entity_index,
                        active: true,
                        x: released.position.x,
                        y: released.position.y,
                    })
                    .expect("qualified released spell holder map insertion");
                debug_assert!(
                    self.renderable_map_chains
                        .entity_chain(
                            released.position.x.to_be_bytes()[0],
                            released.position.y.to_be_bytes()[0],
                        )
                        .contains(&released.source_entity_index),
                    "final released holder insertion must restore current map membership"
                );
            }
        }
        self.finalize_supported_renderable_records()
            .expect("validated source renderable active set");
        if let Some(fire) = self.context.spell_fire.as_mut() {
            // The class-15 handlers consume one active tick after their
            // same-turn projectile factories: model 0 in `sub_693F0` and
            // model 1 in `sub_69640`. This lets Fireball and Possession return
            // to their inactive, retriggerable state on the source clock.
            for holder in &mut fire.holders {
                if matches!(holder.model, 0 | 1) && holder.active_value > 0 {
                    holder.active_value -= 1;
                }
            }
        }
        self.mission.sync_collected_spell_holders(&mut self.context);
        self.mission
            .sync_speed_holder_projection(&mut self.state, &mut self.context);
        let state_hash = state_hash(&self.state);
        let typed_events = self.context.events.clone();
        for event in typed_events {
            let record = self.encode_flight_event(event, state_hash);
            self.append_event(record);
        }
        let live_world_hash = self.live_world_hash_v2();
        for (event, random_state) in goat_events {
            let record = self.encode_goat_event(event, live_world_hash, random_state);
            self.append_event(record);
        }
        let mission_events = self.mission.events().to_vec();
        for event in mission_events {
            let record = self.encode_mission_event(event, state_hash);
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

    fn apply_goat_event_side_effects(&mut self, _world: &NativeWorld, events: &[GoatStepEvent]) {
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

    fn apply_house_event_side_effects(
        &mut self,
        events: &[HouseStepEvent],
        frozen_source_views: &[Level1SourceEntityView],
    ) {
        for event in events {
            match *event {
                HouseStepEvent::ConstructionPass {
                    entity_index,
                    source_id,
                    position,
                    building_index,
                    remaining,
                    first_pass,
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    if first_pass {
                        let effect_start = self.mission.house_construction_effects().len();
                        self.mission.apply_house_construction_first_pass(
                            frozen_source_views,
                            entity_index,
                            source_id,
                            position,
                            footprint,
                        );
                        let effects =
                            self.mission.house_construction_effects()[effect_start..].to_vec();
                        self.live_world
                            .apply_house_construction_source_effects(&effects);
                    }
                    self.terrain
                        .apply_house_construction(position, Some(remaining), footprint)
                        .expect("qualified Level 1 action-51 construction pass");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ConstructionCompleted {
                    entity_index,
                    position,
                    building_index,
                    collision_half_width,
                    collision_half_depth,
                    ..
                } => {
                    let footprint = self
                        .world
                        .house_build_footprint(building_index)
                        .expect("Level 1 action-51 house BUILD footprint");
                    self.terrain
                        .apply_house_construction(position, None, footprint)
                        .expect("qualified Level 1 action-51 construction completion");
                    self.mission
                        .apply_house_construction_completion(
                            frozen_source_views,
                            entity_index,
                            position,
                            collision_half_width,
                            collision_half_depth,
                            |index| {
                                self.world
                                    .house_build_footprint(index)
                                    .map(|value| (value.width, value.height))
                            },
                        )
                        .expect("qualified Level 1 action-51 completion links");
                    self.pending_house_effects.push(*event);
                }
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
                    for resident in outcome.residents {
                        self.mission
                            .spawn_house_resident(
                                resident.position,
                                resident.model,
                                resident.action,
                                attacker_entity_index,
                            )
                            .expect("qualified action-53 resident constructor");
                        self.pending_house_effects
                            .push(HouseStepEvent::ResidentRequested {
                                entity_index,
                                attacker_entity_index,
                                position: resident.position,
                                model: resident.model,
                                action: resident.action,
                            });
                    }
                    self.mission.remove_imported_building(entity_index);
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::ResidentRequested {
                    attacker_entity_index,
                    position,
                    model,
                    action,
                    ..
                } => {
                    self.mission
                        .spawn_house_resident(position, model, action, attacker_entity_index)
                        .expect("qualified action-52 resident constructor");
                    self.pending_house_effects.push(*event);
                }
                HouseStepEvent::Damaged { .. } | HouseStepEvent::DestructionStarted { .. } => {
                    self.pending_house_effects.push(*event);
                }
            }
        }
    }

    fn reclaim_imported_source_slots(&mut self) -> Vec<GoatStepEvent> {
        self.mission
            .take_reclaimed_imported_source_slots()
            .into_iter()
            .filter_map(|source_slot| self.live_world.reclaim_source_slot(source_slot))
            .collect()
    }

    fn sync_supported_renderable_records(&mut self) -> Result<(), String> {
        for actor in &mut self.frozen_actors {
            if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == actor.source_entity_index)
            {
                actor.position = villager.position;
                actor.yaw = villager.yaw;
            } else if (130..=132).contains(&actor.source_entity_index) {
                actor.position.z = self.terrain.ground_height(actor.position);
            }
        }
        for goat in self.live_world.goats() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: goat.entity_index,
                active: goat.active,
                x: goat.position.x,
                y: goat.position.y,
            })?;
        }
        for villager in self.live_world.villagers() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: villager.entity_index,
                active: villager.active,
                x: villager.position.x,
                y: villager.position.y,
            })?;
        }
        for actor in self.mission.actors().iter().filter(|actor| {
            native_composited_actor_kind(actor.kind) && actor.source_entity_index != 0
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            })?;
        }
        for resident in self.mission.dynamic_residents() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })?;
        }
        for segment in self.mission.multipart_segments() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })?;
        }
        for ambient in self.mission.ambient_entities() {
            if ambient.map_linked {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            }
        }
        for projectile in self
            .mission
            .projectiles()
            .iter()
            .filter(|projectile| projectile.source_entity_index() != 0)
        {
            let position = match projectile {
                Level1Projectile::PlayerFireball(value) => value.position,
                Level1Projectile::ArcherArrow(value) => value.position,
                Level1Projectile::FireflyFireball(value) => value.position,
                Level1Projectile::CastleBuild(value) => value.position,
                Level1Projectile::Possession(value) => value.position,
            };
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: projectile.source_entity_index(),
                active: projectile.active(),
                x: position.x,
                y: position.y,
            })?;
        }
        for impact in self
            .mission
            .impacts()
            .iter()
            .filter(|impact| impact.model != POSSESSION_ATTRACTION_MODEL)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: impact.source_entity_index,
                active: impact.active,
                x: impact.position.x,
                y: impact.position.y,
            })?;
        }
        for burst in self
            .mission
            .death_bursts()
            .iter()
            .filter(|burst| burst.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: burst.source_entity_index,
                active: true,
                x: burst.position.x,
                y: burst.position.y,
            })?;
        }
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: marker.source_entity_index,
                active: true,
                x: marker.position.x,
                y: marker.position.y,
            })?;
        }
        for building in self.mission.possessed_buildings() {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: building.entity_index,
                active: building.active,
                x: building.position.x,
                y: building.position.y,
            })?;
        }
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        for sphere in self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| sphere.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active && !sphere.pending_delete,
                x: sphere.position.x,
                y: sphere.position.y,
            })?;
        }
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.source_entity_index != 0)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    fn sync_source_slot_renderable_records(&mut self, source_slot: u16) -> Result<(), String> {
        if self.renderable_map_chains.tracks(source_slot) {
            if source_slot == self.state.entity_index {
                let seed = RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.state.position.x,
                    y: self.state.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(goat) = self
                .live_world
                .goats()
                .iter()
                .find(|goat| goat.entity_index == source_slot)
            {
                let seed = RenderableMapSeed {
                    entity_index: goat.entity_index,
                    active: goat.active,
                    x: goat.position.x,
                    y: goat.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(villager) = self
                .live_world
                .villagers()
                .iter()
                .find(|villager| villager.entity_index == source_slot)
            {
                if let Some(actor) = self
                    .frozen_actors
                    .iter_mut()
                    .find(|actor| actor.source_entity_index == source_slot)
                {
                    actor.position = villager.position;
                    actor.yaw = villager.yaw;
                }
                let seed = RenderableMapSeed {
                    entity_index: villager.entity_index,
                    active: villager.active,
                    x: villager.position.x,
                    y: villager.position.y,
                };
                self.mission.sync_source_map_entity(seed)?;
                self.renderable_map_chains.sync_entity(seed)?;
            } else if let Some(actor) = self.mission.actors().iter().find(|actor| {
                native_composited_actor_kind(actor.kind) && actor.source_entity_index == source_slot
            }) {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: actor.source_entity_index,
                    active: actor.active,
                    x: actor.position.x,
                    y: actor.position.y,
                })?;
            } else if let Some(resident) = self
                .mission
                .dynamic_residents()
                .find(|resident| resident.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: resident.source_entity_index,
                    active: resident.active,
                    x: resident.position.x,
                    y: resident.position.y,
                })?;
            } else if let Some(segment) = self
                .mission
                .multipart_segments()
                .iter()
                .find(|segment| segment.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                })?;
            } else if let Some(ambient) = self
                .mission
                .ambient_entities()
                .find(|ambient| ambient.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: ambient.source_entity_index,
                    active: ambient.visible(),
                    x: ambient.position.x,
                    y: ambient.position.y,
                })?;
            } else if let Some(building) = self
                .mission
                .possessed_buildings()
                .iter()
                .find(|building| building.entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: building.entity_index,
                    active: building.active,
                    x: building.position.x,
                    y: building.position.y,
                })?;
            } else if self.mission.castle_exists
                && self.mission.castle_source_entity_index == source_slot
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: true,
                    x: self.mission.castle_position.x,
                    y: self.mission.castle_position.y,
                })?;
            } else if let Some(sphere) = self
                .mission
                .mana_spheres()
                .iter()
                .find(|sphere| sphere.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: sphere.active && !sphere.pending_delete,
                    x: sphere.position.x,
                    y: sphere.position.y,
                })?;
            } else if let Some(carrier) = self
                .mission
                .mana_carrier()
                .filter(|carrier| carrier.source_entity_index == source_slot)
            {
                self.renderable_map_chains.sync_entity(RenderableMapSeed {
                    entity_index: source_slot,
                    active: carrier.active,
                    x: carrier.position.x,
                    y: carrier.position.y,
                })?;
            }
        }
        // Replay each constructor's AddEventToMap order. Ordinary actors map
        // immediately after allocation. The multipart constructor is the
        // exception: it maps segment 1..16 first and its already-allocated
        // head last, yielding head -> segment16 -> ... -> segment1.
        let mut new_actors = self
            .mission
            .actors()
            .iter()
            .filter(|actor| {
                native_composited_actor_kind(actor.kind)
                    && actor.source_entity_index != 0
                    && !self.renderable_map_chains.tracks(actor.source_entity_index)
            })
            .collect::<Vec<_>>();
        new_actors.sort_unstable_by_key(|actor| actor.source_entity_index);
        let mut newly_tracked = Vec::new();
        for actor in new_actors {
            if actor.kind == Level1ActorKind::MultipartHead {
                let mut segments = self
                    .mission
                    .multipart_segments()
                    .iter()
                    .filter(|segment| {
                        segment.head_source_entity_index == actor.source_entity_index
                            && !self
                                .renderable_map_chains
                                .tracks(segment.source_entity_index)
                    })
                    .collect::<Vec<_>>();
                segments.sort_unstable_by_key(|segment| segment.ordinal);
                newly_tracked.extend(segments.into_iter().map(|segment| RenderableMapSeed {
                    entity_index: segment.source_entity_index,
                    active: segment.active,
                    x: segment.position.x,
                    y: segment.position.y,
                }));
            }
            newly_tracked.push(RenderableMapSeed {
                entity_index: actor.source_entity_index,
                active: actor.active,
                x: actor.position.x,
                y: actor.position.y,
            });
        }
        // Preserve partial-construction prefixes if the source arena filled
        // before all children could be allocated.
        let orphan_segments = self
            .mission
            .multipart_segments()
            .iter()
            .filter(|segment| {
                !self
                    .renderable_map_chains
                    .tracks(segment.source_entity_index)
                    && !newly_tracked
                        .iter()
                        .any(|seed| seed.entity_index == segment.source_entity_index)
            })
            .map(|segment| RenderableMapSeed {
                entity_index: segment.source_entity_index,
                active: segment.active,
                x: segment.position.x,
                y: segment.position.y,
            })
            .collect::<Vec<_>>();
        newly_tracked.extend(orphan_segments);
        for seed in newly_tracked {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_residents = self
            .mission
            .dynamic_residents()
            .filter(|resident| {
                resident.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(resident.source_entity_index)
            })
            .map(|resident| RenderableMapSeed {
                entity_index: resident.source_entity_index,
                active: resident.active,
                x: resident.position.x,
                y: resident.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_residents {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        let newly_tracked_ambient = self
            .mission
            .ambient_entities()
            .filter(|ambient| {
                ambient.map_linked
                    && !self
                        .renderable_map_chains
                        .tracks(ambient.source_entity_index)
            })
            .map(|ambient| RenderableMapSeed {
                entity_index: ambient.source_entity_index,
                active: ambient.visible(),
                x: ambient.position.x,
                y: ambient.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_ambient {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if self.mission.castle_exists
            && self.mission.castle_source_entity_index != 0
            && !self
                .renderable_map_chains
                .tracks(self.mission.castle_source_entity_index)
        {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: self.mission.castle_source_entity_index,
                active: true,
                x: self.mission.castle_position.x,
                y: self.mission.castle_position.y,
            })?;
        }
        let newly_tracked_spheres = self
            .mission
            .mana_spheres()
            .iter()
            .filter(|sphere| {
                sphere.source_entity_index != 0
                    && !self
                        .renderable_map_chains
                        .tracks(sphere.source_entity_index)
            })
            .map(|sphere| RenderableMapSeed {
                entity_index: sphere.source_entity_index,
                active: sphere.active,
                x: sphere.position.x,
                y: sphere.position.y,
            })
            .collect::<Vec<_>>();
        for seed in newly_tracked_spheres {
            self.renderable_map_chains.sync_entity(seed)?;
        }
        if let Some(carrier) = self.mission.mana_carrier().filter(|carrier| {
            carrier.source_entity_index != 0
                && !self
                    .renderable_map_chains
                    .tracks(carrier.source_entity_index)
        }) {
            self.renderable_map_chains.sync_entity(RenderableMapSeed {
                entity_index: carrier.source_entity_index,
                active: carrier.active,
                x: carrier.position.x,
                y: carrier.position.y,
            })?;
        }
        Ok(())
    }

    fn finalize_supported_renderable_records(&mut self) -> Result<(), String> {
        let mut active = self
            .world
            .entities()
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.control_flags & 8 != 0
                    && self.mission.imported_source_slot_is_live(entity.index)
            })
            .map(|entity| entity.index)
            .collect::<Vec<_>>();
        active.extend(
            self.live_world
                .goats()
                .iter()
                .filter(|goat| goat.active)
                .map(|goat| goat.entity_index),
        );
        active.extend(
            self.live_world
                .villagers()
                .iter()
                .filter(|villager| villager.active)
                .map(|villager| villager.entity_index),
        );
        active.extend(
            self.frozen_actors
                .iter()
                .filter(|actor| {
                    actor.source_entity_index >= 130
                        && self
                            .mission
                            .imported_source_slot_is_live(actor.source_entity_index)
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .actors()
                .iter()
                .filter(|actor| {
                    native_composited_actor_kind(actor.kind)
                        && actor.source_entity_index != 0
                        && actor.active
                })
                .map(|actor| actor.source_entity_index),
        );
        active.extend(
            self.mission
                .dynamic_residents()
                .filter(|resident| resident.active)
                .map(|resident| resident.source_entity_index),
        );
        active.extend(
            self.mission
                .multipart_segments()
                .iter()
                .filter(|segment| segment.active)
                .map(|segment| segment.source_entity_index),
        );
        active.extend(
            self.mission
                .ambient_entities()
                .filter(|ambient| ambient.visible())
                .map(|ambient| ambient.source_entity_index),
        );
        active.extend(
            self.mission
                .projectiles()
                .iter()
                .filter(|projectile| projectile.active())
                .map(|projectile| projectile.source_entity_index()),
        );
        active.extend(
            self.mission
                .impacts()
                .iter()
                .filter(|impact| impact.active && impact.model != POSSESSION_ATTRACTION_MODEL)
                .map(|impact| impact.source_entity_index),
        );
        active.extend(
            self.mission
                .death_bursts()
                .iter()
                .filter(|burst| burst.active)
                .map(|burst| burst.source_entity_index),
        );
        active.extend(
            self.mission
                .released_spell_holders()
                .iter()
                .flatten()
                .map(|holder| holder.source_entity_index),
        );
        if let Some(marker) = self
            .mission
            .player_death_marker()
            .filter(|marker| marker.active)
        {
            active.push(marker.source_entity_index);
        }
        active.extend(
            self.mission
                .possessed_buildings()
                .iter()
                .filter(|building| building.active)
                .map(|building| building.entity_index),
        );
        if self.mission.castle_exists && self.mission.castle_source_entity_index != 0 {
            active.push(self.mission.castle_source_entity_index);
        }
        active.extend(
            self.mission
                .mana_spheres()
                .iter()
                .filter(|sphere| {
                    sphere.active && !sphere.pending_delete && sphere.source_entity_index != 0
                })
                .map(|sphere| sphere.source_entity_index),
        );
        if let Some(carrier) = self
            .mission
            .mana_carrier()
            .filter(|carrier| carrier.active && carrier.source_entity_index != 0)
        {
            active.push(carrier.source_entity_index);
        }
        // Hidden class-10/model-45 houses remain live collision rows even
        // before Possession reveals their coloured marker. Keeping them in
        // the filtered chain preserves their source painter/collision order.
        active.extend(
            self.mission
                .possession_collision_targets()
                .iter()
                .filter(|target| target.active)
                .map(|target| target.entity_index),
        );
        active.sort_unstable();
        active.dedup();
        self.renderable_map_chains.retain_entities(&active)
    }

    fn inject_new_fireball_impacts(&mut self, event_start: usize) {
        let impacts = self.mission.events()[event_start..]
            .iter()
            .filter_map(|event| {
                let Level1MissionEvent::FireballImpacted {
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
                            || sim_core::level1_mission::source_entities_overlap(
                                position,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
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
                                    || sim_core::level1_mission::source_entities_overlap(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_VERTICAL_OFFSET,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_HEIGHT,
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
                                    || sim_core::level1_mission::source_entities_overlap_xy(
                                        position,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
                                        sim_core::level1_mission::FIREBALL_IMPACT_COLLISION_HALF_WIDTH,
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

    fn drain_pending_audio_commands(&mut self) {
        let unresolved = self
            .audio_commands
            .iter()
            .filter_map(|(sequence, command)| {
                (command.action == 0).then_some((*sequence, *command))
            })
            .collect::<Vec<_>>();
        let pooled_flush_id = unresolved
            .iter()
            .filter(|(_, command)| !matches!(command.sound_id, 1 | 2 | 5 | 31))
            .map(|(sequence, _)| *sequence)
            .min()
            .unwrap_or(0);
        let mut pending = BTreeMap::<u16, u64>::new();
        for (sequence, command) in unresolved {
            if matches!(command.sound_id, 1 | 2 | 5 | 31) {
                let mut command = command;
                let (action, voice_index) = self.allocate_audio_voice(command);
                command.action = if action == MC_AUDIO_ACTION_SUPPRESS_EXISTING {
                    MC_AUDIO_ACTION_UPDATE_EXISTING
                } else {
                    action
                };
                command.voice_index = voice_index;
                command.pooled_flush_rank = 0;
                self.audio_commands.insert(sequence, command);
                continue;
            }
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
        let mut pooled_flush_rank = 1_u8;
        for sequence in pending.into_values() {
            let mut command = self.audio_commands[&sequence];
            let (action, voice_index) = self.allocate_audio_voice(command);
            command.action = action;
            command.voice_index = voice_index;
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START | MC_AUDIO_ACTION_RESTART | MC_AUDIO_ACTION_UPDATE_EXISTING
            ) {
                command.pooled_flush_rank = pooled_flush_rank;
                command.reserved[0] = pooled_flush_id as u32;
                command.reserved[1] = (pooled_flush_id >> 32) as u32;
                pooled_flush_rank = pooled_flush_rank
                    .checked_add(1)
                    .expect("ordinary sound pool has at most 70 winners");
            } else {
                command.pooled_flush_rank = 0;
            }
            self.audio_commands.insert(sequence, command);
        }
    }

    fn insert_prepared_audio_command(&mut self, sequence: u64, mut command: McAudioCommandV2) {
        if self.game_paused
            && command.action == 0
            && !matches!(command.sound_id, 1 | 2 | 5 | 31)
            && let Some(current_sequence) =
                self.audio_commands
                    .iter()
                    .find_map(|(candidate_sequence, candidate)| {
                        (candidate.action == 0 && candidate.sound_id == command.sound_id)
                            .then_some(*candidate_sequence)
                    })
        {
            let current = self.audio_commands[&current_sequence];
            if positional_sound_should_replace(
                i32::from(command.volume_q15),
                i32::from(current.volume_q15),
            ) {
                self.audio_commands
                    .get_mut(&current_sequence)
                    .expect("paused pending command remains present")
                    .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            } else {
                command.action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            }
        }
        self.audio_commands.insert(sequence, command);
    }

    fn allocate_audio_voice(&mut self, command: McAudioCommandV2) -> (u8, u16) {
        let matching = self.audio_voices.iter().position(|voice| {
            voice.is_some_and(|voice| {
                voice.sound_id == command.sound_id && voice.sound_flags == command.sound_flags
            })
        });
        // `play_type` is the queued `EntitySounds.playType_0` value for
        // PrepareEventSound events (1 = restart-or-start, 3 = if-not-playing).
        // The four direct ambience loops retain the lower PlaySample enum
        // value 2 for if-not-playing.
        let is_direct_ambience_if_not_playing = matches!(command.sound_id, 1 | 2 | 5 | 31)
            && command.play_type == POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING;
        if (command.play_type == 3 || is_direct_ambience_if_not_playing)
            && let Some(index) = matching
        {
            return (
                MC_AUDIO_ACTION_SUPPRESS_EXISTING,
                u16::try_from(index).expect("ten voices fit u16"),
            );
        }
        if command.play_type == 1
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
                    self.insert_prepared_audio_command(record.sequence, command);
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

    fn encode_goat_event(
        &mut self,
        event: GoatStepEvent,
        live_hash: u64,
        random_state: u32,
    ) -> McEventV2 {
        match event {
            GoatStepEvent::Bleat {
                entity_index,
                sound_id,
                position,
                action,
            } => {
                let mut record = self.next_event_base(MC_EVENT_GOAT_SOUND_REQUEST, live_hash);
                record.entity_index = entity_index;
                record.code = u32::from(sound_id);
                record.asset_ref_index = self
                    .world
                    .sound_asset_ref(sound_id)
                    .unwrap_or(MC_ASSET_REF_NONE);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    entity_index,
                    position,
                    action,
                    random_state,
                );
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

    fn encode_mission_event(&mut self, event: Level1MissionEvent, hash: u64) -> McEventV2 {
        match event {
            Level1MissionEvent::ObjectiveAdvanced {
                completed_stage,
                text_selector,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_OBJECTIVE_ADVANCED, hash);
                record.code = u32::from(text_selector);
                record.value_0 = i32::from(completed_stage);
                record
            }
            Level1MissionEvent::AbilityImproved {
                source_family,
                notification_template_selector,
                ability_name_selector,
                notification_state,
                notification_timer,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ABILITY_IMPROVED, hash);
                record.code = u32::from(source_family);
                record.entity_index = ability_name_selector;
                record.value_0 = i32::from(notification_state);
                record.value_1 = i32::from(notification_timer);
                record.reserved[0] = u32::from(notification_template_selector);
                record
            }
            Level1MissionEvent::SpellCollected {
                family,
                authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_SPELL_COLLECTED, hash);
                record.code = u32::from(family);
                record.entity_index = authored_index;
                record
            }
            Level1MissionEvent::FireballSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::PossessionSpawned { projectile_id } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_POSSESSION_SPAWNED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_SPAWNED, hash);
                record.code = sphere_id;
                record.entity_index = source_entity_index;
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::ManaSphereClaimed {
                sphere_id,
                owner_entity_index,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_CLAIMED, hash);
                record.code = u32::from(sound_id);
                record.entity_index = owner_entity_index;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                let sphere = self
                    .mission
                    .mana_spheres()
                    .iter()
                    .find(|sphere| sphere.id == sphere_id)
                    .copied();
                let position = if owner_entity_index == self.state.entity_index {
                    self.state.position
                } else {
                    self.mission
                        .actors()
                        .iter()
                        .find(|actor| actor.source_entity_index == owner_entity_index)
                        .map(|actor| actor.position)
                        .or_else(|| {
                            self.live_world
                                .goats()
                                .iter()
                                .find(|goat| goat.entity_index == owner_entity_index)
                                .map(|goat| goat.position)
                        })
                        .or_else(|| sphere.map(|sphere| sphere.position))
                        .unwrap_or(self.state.position)
                };
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    owner_entity_index,
                    position,
                    10,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::ManaSphereCollected {
                sphere_id,
                carrier_id,
                mana,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_MANA_SPHERE_COLLECTED, hash);
                record.code = carrier_id;
                record.value_0 = i32::try_from(sphere_id).unwrap_or(i32::MAX);
                record.value_1 = mana;
                record
            }
            Level1MissionEvent::ArcherArrowSpawned {
                projectile_id,
                archer_authored_index,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = archer_authored_index;
                record
            }
            Level1MissionEvent::ArcherArrowSound {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ARCHER_ARROW_SOUND, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    13,
                    random_state,
                );
                record
            }
            Level1MissionEvent::FireflyFireballSpawned {
                projectile_id,
                firefly_authored_index,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_FIREBALL_SPAWNED, hash);
                record.code = projectile_id;
                record.entity_index = firefly_authored_index;
                record
            }
            Level1MissionEvent::FireflyChargeSound {
                firefly_authored_index,
                sound_id,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREFLY_CHARGE_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = firefly_authored_index;
                if let Some(actor) = self
                    .mission
                    .actors()
                    .iter()
                    .find(|actor| actor.authored_index == firefly_authored_index)
                    .copied()
                {
                    self.attach_mission_sound(
                        &mut record,
                        sound_id,
                        actor.source_entity_index,
                        actor.position,
                        actor.action,
                        random_state,
                    );
                }
                record
            }
            Level1MissionEvent::FireballImpacted {
                projectile_id,
                impact_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_FIREBALL_IMPACTED, hash);
                record.code = impact_id;
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == impact_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    0,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::WaterSplashed {
                projectile_id,
                splash_id,
                position,
                sound_id,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_WATER_SPLASHED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                record.value_1 = i32::from(position.z);
                let source_entity_index = self
                    .mission
                    .impacts()
                    .iter()
                    .find(|impact| impact.id == splash_id)
                    .map_or(0, |impact| impact.source_entity_index);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    5,
                    self.mission.global_rng(),
                );
                // The stable event row exposes the splash impact identity to
                // presentation.  Audio correlation uses the impact's source
                // slot internally, but must not replace that presentation key
                // in the shared C ABI row.
                record.entity_index = u16::try_from(splash_id).unwrap_or(u16::MAX);
                record
            }
            Level1MissionEvent::ActorDamaged {
                authored_index,
                attacker_entity_index,
                damage,
                life,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_DAMAGED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(attacker_entity_index);
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::ActorKilled {
                authored_index,
                kind,
            } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_ACTOR_KILLED, hash);
                record.entity_index = authored_index;
                record.code = u32::from(kind as u8);
                record
            }
            Level1MissionEvent::CastleBuilt { level } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_BUILT, hash);
                record.code = u32::from(level);
                record
            }
            Level1MissionEvent::CastleProjectileSpawned {
                projectile_id,
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.code = u32::from(sound_id);
                record.value_0 = i32::try_from(projectile_id).unwrap_or(i32::MAX);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    20,
                    random_state,
                );
                record
            }
            Level1MissionEvent::CastlePlacementRejected { projectile_id } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PLACEMENT_REJECTED, hash);
                record.code = projectile_id;
                record
            }
            Level1MissionEvent::CastleConstructionSound {
                sound_id,
                source_entity_index,
                position,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_CONSTRUCTION_SOUND, hash);
                record.code = u32::from(sound_id);
                record.entity_index = source_entity_index;
                record.value_0 = i32::from(position.x) | (i32::from(position.y) << 16);
                record.value_1 = i32::from(position.z);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    44,
                    self.mission.global_rng(),
                );
                record
            }
            Level1MissionEvent::CastleDefenseShot {
                controller_entity_index,
                target_entity_index,
                projectile_entity_index,
                spell_family,
                subspell,
                yaw,
                pitch,
                first_shot,
            } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_CASTLE_PROJECTILE_SPAWNED, hash);
                record.entity_index = projectile_entity_index;
                record.code = u32::from(spell_family)
                    | (u32::from(subspell) << 8)
                    | (u32::from(first_shot) << 16);
                record.value_0 = i32::from(target_entity_index);
                record.value_1 = i32::from(controller_entity_index);
                record.reserved[0] = u32::from(yaw.cast_unsigned());
                record.reserved[1] = u32::from(pitch.cast_unsigned());
                record
            }
            Level1MissionEvent::CastleManaChanged { mana } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_CASTLE_MANA_CHANGED, hash);
                record.value_0 = mana;
                record
            }
            Level1MissionEvent::PlayerDamaged { damage, life } => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DAMAGED, hash);
                record.entity_index = self.state.entity_index;
                record.value_0 = damage;
                record.value_1 = life;
                record
            }
            Level1MissionEvent::PlayerDied => {
                let mut record = self.next_event_base(MC_EVENT_LEVEL1_PLAYER_DIED, hash);
                record.entity_index = self.state.entity_index;
                record
            }
            Level1MissionEvent::MissionSound {
                sound_id,
                source_entity_index,
                position,
                random_state,
            } => {
                let mut record = self.next_event_base(MC_EVENT_PREPARE_SOUND, hash);
                record.code = u32::from(sound_id);
                let source_action = self
                    .mission
                    .source_slots()
                    .get(usize::from(source_entity_index))
                    .map_or(0, |source| source.action);
                self.attach_mission_sound(
                    &mut record,
                    sound_id,
                    source_entity_index,
                    position,
                    source_action,
                    random_state,
                );
                record
            }
            Level1MissionEvent::ExitSequenceStarted => {
                self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_STARTED, hash)
            }
            Level1MissionEvent::ExitSequenceCompleted { result_code } => {
                let mut record =
                    self.next_event_base(MC_EVENT_LEVEL1_EXIT_SEQUENCE_COMPLETED, hash);
                record.code = u32::from(result_code);
                record
            }
            Level1MissionEvent::LevelCompleted => {
                self.next_event_base(MC_EVENT_LEVEL1_COMPLETED, hash)
            }
        }
    }

    fn attach_mission_sound(
        &mut self,
        record: &mut McEventV2,
        sound_id: u16,
        source_entity_index: u16,
        source_position: Position,
        source_action: u8,
        random_state: u32,
    ) {
        record.entity_index = source_entity_index;
        record.asset_ref_index = self
            .world
            .sound_asset_ref(sound_id)
            .unwrap_or(MC_ASSET_REF_NONE);
        let source_id = self
            .world
            .entities()
            .get(usize::from(source_entity_index))
            .map_or(source_entity_index, |source| source.source_id);
        if let Some(prepared) = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source_id,
            source_action,
        ) {
            self.insert_prepared_audio_command(
                record.sequence,
                audio_command_record(
                    record.generation,
                    record.sequence,
                    record.state_hash,
                    record.asset_ref_index,
                    sound_id,
                    source_entity_index,
                    record.player_index,
                    prepared.command,
                    prepared.play_type,
                ),
            );
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
        if matches!(sound_id, 14 | 29) {
            // PrepareEventSound's local LevelIndex/-1 branch stores type 1
            // for menu select and can't-use feedback.
            return Some(local_audio_command_record(event, sound_id, 0, 1));
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
        let prepared = prepare_event_sound_command(
            self.state.position,
            self.state.yaw,
            source_position,
            sound_id,
            random_state,
            source.source_id,
            source.action,
        )?;
        Some(audio_command_record(
            event.generation,
            event.sequence,
            event.state_hash,
            event.asset_ref_index,
            sound_id,
            event.entity_index,
            event.player_index,
            prepared.command,
            prepared.play_type,
        ))
    }
}

fn selector_state_hash_v1(frames: &GoatTmapClocks, feedback: Option<(u64, u8)>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in b"mc2-selector-frame-session-v1\0"
        .iter()
        .copied()
        .chain(frames.hash_v1().to_le_bytes())
        .chain(u8::from(feedback.is_some()).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.0).to_le_bytes())
        .chain(feedback.map_or(0, |value| value.1).to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn camera_history_hash(camera: CameraHistory) -> u64 {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"mc2-presentation-camera-v1\0");
    bytes.extend_from_slice(&camera.tick.to_le_bytes());
    bytes.extend_from_slice(&camera.source_turn.to_le_bytes());
    bytes.extend_from_slice(&camera.x.to_le_bytes());
    bytes.extend_from_slice(&camera.y.to_le_bytes());
    bytes.extend_from_slice(&camera.z.to_le_bytes());
    bytes.extend_from_slice(&camera.yaw.to_le_bytes());
    bytes.extend_from_slice(&camera.pitch.to_le_bytes());
    bytes.extend_from_slice(&camera.roll.to_le_bytes());
    bytes.extend_from_slice(&camera.fov.to_le_bytes());
    sim_core::presentation_frame::byte_hash(&bytes)
}

fn selector_frame_state(
    ordinal: usize,
    clock: &GoatTmapClock,
    bank: &GoatTmapBank,
) -> McSelectorFrameStateV2 {
    let selector = &bank.selectors()[ordinal];
    McSelectorFrameStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McSelectorFrameStateV2>(),
        selector: clock.selector(),
        mask_bit: ordinal as u8,
        authority: MC_AUTHORITY_SELECTOR_FRAME_LIVE,
        width: clock.width(),
        height: clock.height(),
        source_frame_index: u16::from(clock.frame_index()),
        frame_count: 4,
        stream_start: selector.stream_start,
        stream_cursor: clock.stream_cursor(),
        payload_bytes: selector.payload_bytes,
        pixel_bytes: u32::try_from(clock.pixels().len())
            .expect("qualified Goat TMAP pixel count fits u32"),
        flags: (MC_SELECTOR_FRAME_ANIMATED | MC_SELECTOR_FRAME_PIXELS_MUTABLE)
            & !MC_SELECTOR_FRAME_GROUPED_UPDATE
            | if clock.used_by_preceding_draw() {
                MC_SELECTOR_FRAME_USED_BY_PRECEDING_DRAW
            } else {
                0
            },
        pixel_hash: clock.pixel_hash_v1(),
        reserved: [0; 4],
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
        volume_q15: if matches!(sound_id, 14 | 29) {
            0x7fff
        } else {
            0
        },
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
    play_type: u8,
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
        play_type,
        loop_count: 0,
        action: if command.audible {
            0
        } else {
            MC_AUDIO_ACTION_SUPPRESS_DISTANCE_OR_VOLUME
        },
        pooled_flush_rank: 0,
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
    let prepared = prepare_event_sound_command(listener, 0, source, sound_id, random_state, 321, 0)
        .expect("qualification vector uses a proven Level 1 positional sound");
    let asset_ref_index = match sound_id {
        37 => 95,
        39 => 96,
        43 => 99,
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
        prepared.command,
        prepared.play_type,
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

#[derive(Debug)]
enum CampaignSessionKind {
    Level1 {
        world: Arc<NativeWorld>,
    },
    Level2 {
        inspection: Arc<NativeWorldInspection>,
        reset: Level2ResetSession,
        day_frame_assets: Option<DayIndexedFrameAssets>,
        day_frame_state: DayIndexedFrameState,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CampaignDayMusicLifecycle {
    envelope: DynamicMusicEnvelope,
    initialized: bool,
    loaded: bool,
    active: bool,
    playing: bool,
    track: u8,
    volume: u8,
    asset_slot: u32,
    start_count: u32,
    update_count: u32,
    fade_callback_count: u32,
    stop_count: u32,
    end_count: u32,
}

impl CampaignDayMusicLifecycle {
    fn start_day() -> Self {
        let mut envelope = DynamicMusicEnvelope::default();
        envelope.start();
        Self {
            envelope,
            initialized: true,
            loaded: true,
            active: true,
            playing: true,
            track: 2,
            volume: 127,
            asset_slot: 0,
            start_count: 1,
            update_count: 0,
            fade_callback_count: 0,
            stop_count: 0,
            end_count: 0,
        }
    }

    fn update(&mut self, mode: u8, timer_multiplier: u8) {
        self.update_count = self.update_count.wrapping_add(1);
        let _ = self.envelope.update(mode, timer_multiplier);
    }

    fn advance(&mut self, callbacks: u32) {
        for _ in 0..callbacks {
            if !self.envelope.timer_active {
                break;
            }
            self.fade_callback_count = self.fade_callback_count.wrapping_add(1);
            debug_assert!(!matches!(self.envelope.callback(), FadeCallback::Inactive));
        }
    }

    fn stop(&mut self) {
        if !self.playing || !self.active {
            return;
        }
        self.stop_count = self.stop_count.wrapping_add(1);
        self.playing = false;
        self.envelope.stop();
    }

    fn end(&mut self) {
        if !self.loaded {
            return;
        }
        self.end_count = self.end_count.wrapping_add(1);
        self.playing = false;
        self.loaded = false;
        self.active = false;
        self.envelope.stop();
    }

    fn state_hash(self) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(b"level2-day-music-lifecycle-v1");
        hasher.update(self.asset_slot.to_le_bytes());
        hasher.update(self.start_count.to_le_bytes());
        hasher.update(self.update_count.to_le_bytes());
        hasher.update(self.fade_callback_count.to_le_bytes());
        hasher.update(self.stop_count.to_le_bytes());
        hasher.update(self.end_count.to_le_bytes());
        hasher.update([
            u8::from(self.initialized),
            u8::from(self.loaded),
            u8::from(self.active),
            u8::from(self.playing),
            self.track,
            self.volume,
            u8::from(self.envelope.updates_enabled),
            u8::from(self.envelope.timer_active),
            self.envelope.target_mode,
            self.envelope.direction.cast_unsigned(),
            self.envelope.current,
            self.envelope.timer_multiplier,
        ]);
        let digest = hasher.finalize();
        u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
    }
}

#[derive(Debug)]
struct CampaignSessionRecord {
    kind: CampaignSessionKind,
    generation: u64,
    day_music: Option<CampaignDayMusicLifecycle>,
}

#[derive(Default)]
struct CampaignRegistry {
    next_handle: u64,
    sessions: BTreeMap<u64, Arc<Mutex<CampaignSessionRecord>>>,
}

impl CampaignRegistry {
    fn insert(&mut self, session: CampaignSessionRecord) -> AbiResult<u64> {
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
            "campaign session handle space is unavailable",
        ))
    }
}

static CAMPAIGN_REGISTRY: OnceLock<Mutex<CampaignRegistry>> = OnceLock::new();

fn campaign_registry() -> &'static Mutex<CampaignRegistry> {
    CAMPAIGN_REGISTRY.get_or_init(|| Mutex::new(CampaignRegistry::default()))
}

fn with_campaign_registry<T>(
    operation: impl FnOnce(&CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&guard)
}

fn with_campaign_registry_mut<T>(
    operation: impl FnOnce(&mut CampaignRegistry) -> AbiResult<T>,
) -> AbiResult<T> {
    let mut guard = campaign_registry().lock().map_err(|_| {
        AbiError::new(
            MC_STATUS_LOCK_FAILED,
            "campaign session registry lock is poisoned",
        )
    })?;
    operation(&mut guard)
}

fn campaign_session_arc(
    registry: &CampaignRegistry,
    handle: u64,
) -> AbiResult<Arc<Mutex<CampaignSessionRecord>>> {
    if handle == 0 {
        return Err(AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            "campaign session handle zero is invalid",
        ));
    }
    registry.sessions.get(&handle).cloned().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_UNKNOWN_HANDLE,
            format!("unknown campaign session handle {handle}"),
        )
    })
}

fn with_campaign_session<T>(
    handle: u64,
    operation: impl FnOnce(&CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| campaign_session_arc(registry, handle))?;
    let guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned"))?;
    operation(&guard)
}

fn with_campaign_session_mut<T>(
    handle: u64,
    operation: impl FnOnce(&mut CampaignSessionRecord) -> AbiResult<T>,
) -> AbiResult<T> {
    let entry = with_campaign_registry(|registry| campaign_session_arc(registry, handle))?;
    let mut guard = entry
        .lock()
        .map_err(|_| AbiError::new(MC_STATUS_LOCK_FAILED, "campaign session lock is poisoned"))?;
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
        let owner_cache = std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from);
        let session = CampaignFacade::<SealedLevel1Adapter>::open(
            SealedLevel1Adapter::KEY,
            InternalCampaignOpen {
                package_dir: Path::new(&package),
                closure_path: Some(Path::new(&closure)),
                owner_cache: owner_cache.as_deref(),
            },
        )?;
        with_registry_mut(|registry| registry.insert(session))
    })
}

/// Open a validated common campaign session. Level 1 uses the same loader as
/// the legacy call; Level 2 uses the typed inspection/content seam and never
/// falls through to the Level 1 mission dispatcher.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_open(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> i32 {
    boundary(|| {
        open_campaign_session(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            None,
            selection,
            out_handle,
        )
    })
}

/// Open a campaign session with an explicit qualified owner-cache root.  The
/// additive entry point keeps the legacy environment-bound open intact while
/// letting managed callers bind the Day cache to this handle without relying
/// on process-global state.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_open_with_asset_cache(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: *const u8,
    asset_cache_path_len: u32,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> i32 {
    boundary(|| {
        require_output(out_handle, "out_handle")?;
        write_output(out_handle, 0);
        let asset_cache_path =
            utf8_input(asset_cache_path, asset_cache_path_len, "asset_cache_path")?;
        open_campaign_session(
            package_dir,
            package_dir_len,
            closure_path,
            closure_path_len,
            (!asset_cache_path.is_empty()).then_some(asset_cache_path),
            selection,
            out_handle,
        )
    })
}

fn open_campaign_session(
    package_dir: *const u8,
    package_dir_len: u32,
    closure_path: *const u8,
    closure_path_len: u32,
    asset_cache_path: Option<String>,
    selection: *const McCampaignSelectionV2,
    out_handle: *mut u64,
) -> AbiResult<()> {
    require_output(out_handle, "out_handle")?;
    write_output(out_handle, 0);
    let selection = read_versioned_input(selection, "campaign selection")?;
    validate_reserved(&selection.reserved, "campaign selection")?;
    if selection.reserved_0 != 0 {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            "campaign selection reserved_0 must be zero",
        ));
    }
    let package = utf8_input(package_dir, package_dir_len, "package_dir")?;
    if package.is_empty() {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "package_dir must be non-empty",
        ));
    }
    let expected = sim_core::mission_content::MissionContentProfile::from_metadata(
        selection.external_level,
        selection.internal_level,
        selection.map_type,
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
    let session = if expected.is_level2() {
        let inspection = match asset_cache_path.as_deref() {
            Some(cache_root) => {
                let root = Path::new(cache_root);
                if root.join("components/gameplay/manifest.json").is_file() {
                    NativeWorld::inspect_level2_with_owner_cache(
                        &package,
                        root.join("components/gameplay"),
                    )
                } else {
                    NativeWorld::inspect_level2(&package)
                }
            }
            None => NativeWorld::inspect_level2(&package),
        }
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        let mut reset = inspection
            .level2_reset_session()
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        reset.select_level2_campaign_entry_stage();
        let day_frame_assets = load_optional_level2_day_frame_assets(asset_cache_path.as_deref())?;
        let day_music = load_optional_level2_day_music(asset_cache_path.as_deref())?;
        CampaignSessionRecord {
            kind: CampaignSessionKind::Level2 {
                inspection: Arc::new(inspection),
                reset,
                day_frame_assets,
                day_frame_state: DayIndexedFrameState::new(0),
            },
            generation: 0,
            day_music,
        }
    } else {
        let closure = utf8_input(closure_path, closure_path_len, "closure_path")?;
        if closure.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 campaign sessions require closure_path",
            ));
        }
        let world = NativeWorld::load(&package, &closure)
            .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
        CampaignSessionRecord {
            kind: CampaignSessionKind::Level1 {
                world: Arc::new(world),
            },
            generation: 0,
            day_music: None,
        }
    };
    let handle = with_campaign_registry_mut(|registry| registry.insert(session))?;
    write_output(out_handle, handle);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_destroy(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_registry_mut(|registry| {
            if registry.sessions.remove(&handle).is_none() {
                return Err(AbiError::new(
                    MC_STATUS_UNKNOWN_HANDLE,
                    format!("unknown campaign session handle {handle}"),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_reset(handle: u64) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            if let CampaignSessionKind::Level2 {
                reset,
                day_frame_state,
                ..
            } = &mut session.kind
            {
                reset.reset();
                session.generation = session.generation.wrapping_add(1);
                day_frame_state.reset(session.generation);
                if session.day_music.is_some() {
                    session.day_music = Some(CampaignDayMusicLifecycle::start_day());
                }
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignShieldSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_shield_snapshot(
                generation,
                reset.shared_spells().shield_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignShieldSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Shield help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_shield(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield holder is not active",
                ));
            }
            Ok(campaign_shield_snapshot(
                generation,
                reset.shared_spells().shield_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_shield_damage_submit(
    handle: u64,
    generation: u64,
    target_entity_index: u16,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield damage requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Shield generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset.shared_spells_mut().submit_shield_damage(
                target_entity_index,
                damage,
                attacker_entity_index,
            ) {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Shield damage mailbox rejected the hit",
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_invisible_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignInvisibleSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Invisible generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_invisible_snapshot(
                generation,
                reset.shared_spells().invisible_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_invisible_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignInvisibleSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Invisible help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Invisible generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_invisible(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Invisible holder is not active",
                ));
            }
            Ok(campaign_invisible_snapshot(
                generation,
                reset.shared_spells().invisible_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_beyond_sight_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignBeyondSightSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Beyond Sight generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            Ok(campaign_beyond_sight_snapshot(
                generation,
                reset.shared_spells().beyond_sight_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_beyond_sight_activate(
    handle: u64,
    generation: u64,
    direction_mask: u16,
    help_enabled: u8,
    out_state: *mut McCampaignBeyondSightSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if help_enabled > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign Beyond Sight help_enabled must be 0 or 1",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight activation requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Beyond Sight generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if !reset
                .shared_spells_mut()
                .activate_beyond_sight(direction_mask, help_enabled != 0)
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Beyond Sight holder is not active",
                ));
            }
            Ok(campaign_beyond_sight_snapshot(
                generation,
                reset.shared_spells().beyond_sight_state(),
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_opponent_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignOpponentSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign opponent state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign opponent generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let participant = reset.nyphur_participant().map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign opponent reset census is invalid: {error}"),
                )
            })?;
            let visibility = reset.nyphur_visibility().map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign opponent visibility is invalid: {error}"),
                )
            })?;
            Ok(campaign_opponent_snapshot(
                generation,
                participant,
                visibility,
            ))
        })
    })
}

/// Read the authoritative Day HUD/status/hand projection for one campaign
/// generation. The projection is reset-owned and therefore unavailable to
/// the legacy Level 1 handle.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_hud_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayHudSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Day HUD state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day HUD generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != reset.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day HUD tick {expected_tick} does not match {}",
                        reset.state.tick
                    ),
                ));
            }
            let snapshot = reset.day_hud().map_err(|error| {
                AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign Day HUD projection is invalid: {error}"),
                )
            })?;
            Ok(campaign_day_hud_snapshot(session.generation, snapshot))
        })
    })
}

fn campaign_day_music_snapshot(
    generation: u64,
    tick: u64,
    music: CampaignDayMusicLifecycle,
) -> McCampaignDayMusicSnapshotV2 {
    McCampaignDayMusicSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayMusicSnapshotV2>(),
        generation,
        tick,
        asset_slot: music.asset_slot,
        start_count: music.start_count,
        update_count: music.update_count,
        fade_callback_count: music.fade_callback_count,
        stop_count: music.stop_count,
        end_count: music.end_count,
        initialized: u8::from(music.initialized),
        loaded: u8::from(music.loaded),
        active: u8::from(music.active),
        playing: u8::from(music.playing),
        track: music.track,
        volume: music.volume,
        updates_enabled: u8::from(music.envelope.updates_enabled),
        timer_active: u8::from(music.envelope.timer_active),
        target_mode: music.envelope.target_mode,
        direction: music.envelope.direction,
        current_units: music.envelope.current,
        timer_multiplier: music.envelope.timer_multiplier,
        timer_hz: DynamicMusicEnvelope::timer_hz(music.envelope.timer_multiplier),
        reserved_0: 0,
        state_hash: music.state_hash(),
        reserved: [0; 2],
    }
}

fn with_level2_campaign_day_music<T>(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
    operation: impl FnOnce(CampaignDayMusicLifecycle) -> AbiResult<T>,
) -> AbiResult<T> {
    let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day music requires a Level 2 campaign session",
        ));
    };
    if generation != session.generation || expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day music generation/tick {generation}/{expected_tick} does not match {}/{}",
                session.generation, reset.state.tick
            ),
        ));
    }
    let music = session.day_music.ok_or_else(|| {
        AbiError::new(
            MC_STATUS_VALIDATION_FAILED,
            "campaign Day music requires the qualified Level 2 owner cache",
        )
    })?;
    operation(music)
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            with_level2_campaign_day_music(session, generation, expected_tick, |music| {
                Ok(campaign_day_music_snapshot(
                    generation,
                    expected_tick,
                    music,
                ))
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_advance(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    callback_count: u32,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        if callback_count > 1_024 {
            return Err(AbiError::new(
                MC_STATUS_BOUNDS,
                "campaign Day music callback count exceeds 1024",
            ));
        }
        with_campaign_session_mut(handle, |session| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.advance(callback_count);
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_stop(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session_mut(handle, |session| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.stop();
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_music_end(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignDayMusicSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session_mut(handle, |session| {
            with_level2_campaign_day_music(session, generation, expected_tick, |_| Ok(()))?;
            let music = session.day_music.as_mut().expect("validated Day music");
            music.end();
            Ok(campaign_day_music_snapshot(
                generation,
                expected_tick,
                *music,
            ))
        })
    })
}

fn campaign_minimap_projection(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
) -> AbiResult<(Level2MinimapPlan, u16)> {
    let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign minimap projection requires a Level 2 campaign session",
        ));
    };
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign minimap generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    if expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign minimap tick {expected_tick} does not match {}",
                reset.state.tick
            ),
        ));
    }
    let local_player_slot = usize::from(reset.runtime().local_player_slot);
    let local_player_entity_index = reset
        .players()
        .get(local_player_slot)
        .ok_or_else(|| {
            AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign minimap local player slot is unavailable",
            )
        })?
        .entity_index;
    let plan = level2_minimap_plan(reset).map_err(|error| {
        AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            format!("campaign minimap projection is invalid: {error}"),
        )
    })?;
    Ok((plan, local_player_entity_index))
}

fn minimap_hash_update(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(bytes);
}

fn level2_minimap_entity_hash(rows: &[Level2MinimapEntity]) -> u64 {
    let mut hasher = Sha256::new();
    for row in rows {
        minimap_hash_update(&mut hasher, &row.source_order.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.subject_entity_index.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.x.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.y.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.selector.to_le_bytes());
        minimap_hash_update(
            &mut hasher,
            &[
                row.palette_index,
                row.kind,
                row.size,
                row.anchor,
                row.help_selector,
                row.visible,
            ],
        );
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
}

fn level2_minimap_mark_hash(rows: &[Level2MinimapMark]) -> u64 {
    let mut hasher = Sha256::new();
    for row in rows {
        minimap_hash_update(&mut hasher, &row.source_order.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.subject_entity_index.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.x.to_le_bytes());
        minimap_hash_update(&mut hasher, &row.y.to_le_bytes());
        minimap_hash_update(
            &mut hasher,
            &[row.palette_index, row.kind, row.help_selector, row.visible],
        );
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 has eight bytes"))
}

fn campaign_minimap_state_snapshot(
    generation: u64,
    tick: u64,
    local_player_entity_index: u16,
    plan: &Level2MinimapPlan,
) -> McCampaignMinimapStateV2 {
    McCampaignMinimapStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapStateV2>(),
        generation,
        tick,
        local_player_entity_index,
        entity_count: u32::try_from(plan.entities.len()).expect("minimap entity count fits u32"),
        mark_count: u32::try_from(plan.marks.len()).expect("minimap mark count fits u32"),
        entity_hash: level2_minimap_entity_hash(&plan.entities),
        mark_hash: level2_minimap_mark_hash(&plan.marks),
        visibility_flags: plan.visibility_flags,
        ..McCampaignMinimapStateV2::default()
    }
}

fn campaign_minimap_entity_snapshot(
    generation: u64,
    tick: u64,
    row: Level2MinimapEntity,
) -> McCampaignMinimapEntityV2 {
    McCampaignMinimapEntityV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapEntityV2>(),
        generation,
        tick,
        source_order: row.source_order,
        subject_entity_index: row.subject_entity_index,
        x: row.x,
        y: row.y,
        selector: row.selector,
        palette_index: row.palette_index,
        kind: row.kind,
        size: row.size,
        anchor: row.anchor,
        help_selector: row.help_selector,
        visible: row.visible,
        ..McCampaignMinimapEntityV2::default()
    }
}

fn campaign_minimap_mark_snapshot(
    generation: u64,
    tick: u64,
    row: Level2MinimapMark,
) -> McCampaignMinimapMarkV2 {
    McCampaignMinimapMarkV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignMinimapMarkV2>(),
        generation,
        tick,
        source_order: row.source_order,
        subject_entity_index: row.subject_entity_index,
        x: row.x,
        y: row.y,
        palette_index: row.palette_index,
        kind: row.kind,
        help_selector: row.help_selector,
        visible: row.visible,
        ..McCampaignMinimapMarkV2::default()
    }
}

fn copy_minimap_rows<T: Copy>(
    rows: &[T],
    start_ordinal: u32,
    buffer: *mut T,
    capacity: u32,
    out_written: *mut u32,
) -> AbiResult<()> {
    let start = usize::try_from(start_ordinal)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "minimap start ordinal exceeds usize"))?;
    if start > rows.len() {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("minimap start ordinal {start} exceeds {}", rows.len()),
        ));
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "minimap capacity exceeds usize"))?;
    let count = rows.len().saturating_sub(start).min(capacity);
    if count != 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            "minimap row output buffer is null",
        ));
    }
    // SAFETY: count is bounded by the caller's capacity and the immutable
    // source slice; the native ABI owns no alias of the destination.
    unsafe { buffer.copy_from_nonoverlapping(rows[start..].as_ptr(), count) };
    write_output(
        out_written,
        u32::try_from(count).expect("minimap row count fits u32"),
    );
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_state(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_state: *mut McCampaignMinimapStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let (plan, local_player_entity_index) =
                campaign_minimap_projection(session, generation, expected_tick)?;
            Ok(campaign_minimap_state_snapshot(
                generation,
                expected_tick,
                local_player_entity_index,
                &plan,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_entities_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignMinimapEntityV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_element_size::<McCampaignMinimapEntityV2>(element_size, "campaign minimap entity")?;
        require_output(out_written, "minimap entity count")?;
        with_campaign_session(handle, |session| {
            let (plan, _) = campaign_minimap_projection(session, generation, expected_tick)?;
            let rows = plan
                .entities
                .iter()
                .copied()
                .map(|row| campaign_minimap_entity_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            copy_minimap_rows(&rows, start_ordinal, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_minimap_marks_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignMinimapMarkV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_element_size::<McCampaignMinimapMarkV2>(element_size, "campaign minimap mark")?;
        require_output(out_written, "minimap mark count")?;
        with_campaign_session(handle, |session| {
            let (plan, _) = campaign_minimap_projection(session, generation, expected_tick)?;
            let rows = plan
                .marks
                .iter()
                .copied()
                .map(|row| campaign_minimap_mark_snapshot(generation, expected_tick, row))
                .collect::<Vec<_>>();
            copy_minimap_rows(&rows, start_ordinal, buffer, capacity, out_written)
        })
    })
}

/// Execute one additive Level 2 outer turn.  This path deliberately owns a
/// separate campaign handle and never enters the legacy Level 1 session
/// dispatcher.  The candidate session is stepped on a clone first so a short
/// event buffer or invalid output cannot partially mutate the live handle.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_session_step(
    handle: u64,
    input: *const McCampaignStepInputV2,
    out_snapshot: *mut McCampaignStepSnapshotV2,
    out_snapshot_size: u32,
    events: *mut McCampaignEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_struct_output::<McCampaignStepSnapshotV2>(
            out_snapshot,
            out_snapshot_size,
            "campaign step snapshot",
        )?;
        require_output(out_event_count, "campaign step event count")?;
        require_element_size::<McCampaignEventV2>(event_element_size, "campaign step event")?;
        let input = read_versioned_input(input, "campaign step input")?;
        validate_reserved(&input.reserved, "campaign step input")?;
        if input.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "campaign step input reserved_0 must be zero",
            ));
        }
        if input.speed_index > 2 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign step speed_index must be 0, 1, or 2",
            ));
        }
        validate_embedded_command(&input.command)?;
        let command = player_command(input.command)?;
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign step requires a Level 2 campaign session",
                ));
            };
            if input.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign step generation {} does not match {}",
                        input.generation, session.generation
                    ),
                ));
            }
            let player = reset
                .players()
                .get(usize::from(input.player_slot))
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "campaign player slot {} is out of bounds",
                            input.player_slot
                        ),
                    )
                })?;
            if player.active_player_index < 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("campaign player slot {} is inactive", input.player_slot),
                ));
            }
            if input.player_slot != reset.runtime().local_player_slot {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!(
                        "campaign step player slot {} is not the local slot {}",
                        input.player_slot,
                        reset.runtime().local_player_slot
                    ),
                ));
            }

            let mut candidate = reset.clone();
            let music_event_start = candidate.context.events.len();
            candidate.state.entity_index = player.entity_index;
            candidate.state.position = Position {
                x: input.player_x,
                y: input.player_y,
                z: input.player_z,
            };
            candidate.step_outer_turn(command, input.speed_index);

            if let Some(day_music) = session.day_music.as_mut() {
                for event in &candidate.context.events[music_event_start..] {
                    if let FlightEvent::UpdateMusic {
                        mode,
                        timer_multiplier,
                    } = *event
                    {
                        day_music.update(mode, timer_multiplier);
                    }
                }
            }

            let event_rows = candidate
                .dispatch_events()
                .iter()
                .enumerate()
                .map(|(ordinal, event)| campaign_event_snapshot(session.generation, ordinal, event))
                .collect::<Vec<_>>();
            copy_slice(&event_rows, events, event_capacity, out_event_count)?;
            let mut event_kind_mask = 0_u32;
            for event in &event_rows {
                if (1..=32).contains(&event.kind) {
                    event_kind_mask |= 1_u32 << u32::from(event.kind - 1);
                }
            }
            let snapshot = McCampaignStepSnapshotV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignStepSnapshotV2>(),
                generation: session.generation,
                revision: candidate.state.tick,
                state_hash: candidate.campaign_state_hash(),
                terrain_hash: candidate.terrain_hash(),
                dispatch_hash: decode_sha256_hex(&candidate.dispatch_observability_hash()),
                event_count: u32::try_from(event_rows.len()).map_err(|_| {
                    AbiError::new(MC_STATUS_BOUNDS, "campaign event count exceeds u32")
                })?,
                event_kind_mask,
                player_slot: input.player_slot,
                reserved_0: [0; 3],
                player_entity_index: candidate.state.entity_index,
                reserved_1: 0,
                x: candidate.state.position.x,
                y: candidate.state.position.y,
                z: candidate.state.position.z,
                reserved: [0; 4],
            };
            *reset = candidate;
            Ok(snapshot)
        })
        .and_then(|snapshot| {
            write_output(out_snapshot, snapshot);
            Ok(())
        })
    })
}

/// Copy the detailed Level 2 economy sidecars from the most recent outer
/// turn.  Generic `McCampaignEventV2` rows remain the stable 48-byte event
/// stream; callers join these rows by `generation + ordinal`.  Level 1
/// campaign handles and reset/initial states have no kind-10 sidecars.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_economy_events(
    handle: u64,
    events: *mut McCampaignEconomyEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign economy event count")?;
        require_element_size::<McCampaignEconomyEventV2>(
            event_element_size,
            "campaign economy event",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign economy events require a Level 2 campaign session",
                ));
            };
            let rows = reset
                .dispatch_events()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, dispatch)| match dispatch {
                    Level2DispatchEvent::Economy {
                        pass,
                        source_slot,
                        event,
                    } => Some(campaign_economy_event_snapshot(
                        session.generation,
                        ordinal,
                        *pass,
                        *source_slot,
                        *event,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

/// Copy the detailed authored-group activation sidecars from the most recent
/// outer turn.  Generic `McCampaignEventV2` rows remain the stable 48-byte
/// event stream; callers join these rows by `generation + ordinal` and use
/// `event_index` to reconstruct the exact source-ordered authored list.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_authored_group_events(
    handle: u64,
    events: *mut McCampaignAuthoredGroupEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign authored-group event count")?;
        require_element_size::<McCampaignAuthoredGroupEventV2>(
            event_element_size,
            "campaign authored-group event",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign authored-group events require a Level 2 campaign session",
                ));
            };
            let mut rows = Vec::new();
            for (ordinal, dispatch) in reset.dispatch_events().iter().enumerate() {
                let Level2DispatchEvent::AuthoredGroupActivated {
                    pass,
                    source_slot,
                    group,
                    authored_indices,
                } = dispatch
                else {
                    continue;
                };
                let flags = u8::try_from(authored_indices.len()).unwrap_or(u8::MAX);
                for (event_index, authored_index) in authored_indices.iter().copied().enumerate() {
                    rows.push(McCampaignAuthoredGroupEventV2 {
                        abi_version: MC_NATIVE_ABI_VERSION,
                        struct_size: struct_size::<McCampaignAuthoredGroupEventV2>(),
                        generation: session.generation,
                        ordinal: u32::try_from(ordinal)
                            .expect("bounded campaign event ordinal fits u32"),
                        pass: *pass,
                        flags,
                        group: *group,
                        source_slot: *source_slot,
                        event_index: u16::try_from(event_index)
                            .expect("bounded authored-group event index fits u16"),
                        authored_index,
                        reserved_0: 0,
                        reserved: [0; 4],
                    });
                }
            }
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

/// Copy the detailed Level 2 Goat sidecars from the most recent outer turn.
/// Generic `McCampaignEventV2` rows remain the stable 48-byte event stream;
/// callers join these rows by `generation + ordinal`, and use `event_index`
/// when one Goat dispatch row emits multiple typed Goat payloads.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goat_events(
    handle: u64,
    events: *mut McCampaignGoatEventV2,
    event_capacity: u32,
    event_element_size: u32,
    out_event_count: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_event_count, "campaign goat event count")?;
        require_element_size::<McCampaignGoatEventV2>(event_element_size, "campaign goat event")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign goat events require a Level 2 campaign session",
                ));
            };
            let mut rows = Vec::new();
            for (ordinal, dispatch) in reset.dispatch_events().iter().enumerate() {
                let Level2DispatchEvent::Goat {
                    pass,
                    source_slot,
                    events,
                } = dispatch
                else {
                    continue;
                };
                for (event_index, event) in events.iter().copied().enumerate() {
                    rows.push(campaign_goat_event_snapshot(
                        session.generation,
                        ordinal,
                        event_index,
                        *pass,
                        *source_slot,
                        event,
                    ));
                }
            }
            copy_slice(&rows, events, event_capacity, out_event_count)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_world_info(
    handle: u64,
    out_info: *mut McCampaignWorldInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_campaign_session(handle, |session| {
            let (metadata, entity_count, active_count, player_count, objective_count, hash) =
                match &session.kind {
                    CampaignSessionKind::Level1 { world } => (
                        world.metadata(),
                        world.entities().len(),
                        world
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        world.players().len(),
                        world.runtime().objectives.len(),
                        state_hash(world.reset_state()),
                    ),
                    CampaignSessionKind::Level2 {
                        inspection, reset, ..
                    } => (
                        inspection.metadata(),
                        inspection.entities().len(),
                        inspection
                            .entities()
                            .iter()
                            .filter(|entity| entity.active)
                            .count(),
                        inspection.players().len(),
                        inspection.runtime().objectives.len(),
                        reset.campaign_state_hash(),
                    ),
                };
            Ok(McCampaignWorldInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignWorldInfoV2>(),
                external_level: metadata.external_level,
                internal_level: metadata.internal_level,
                map_type: metadata.map_type.id(),
                profile: MC_PROFILE_OUTDOOR,
                reserved_0: 0,
                entity_count: u32::try_from(entity_count).expect("entity count fits u32"),
                active_entity_count: u32::try_from(active_count).expect("active count fits u32"),
                player_count: u32::try_from(player_count).expect("player count fits u32"),
                objective_count: u32::try_from(objective_count).expect("objective count fits u32"),
                terrain_width: 256,
                terrain_height: 256,
                reset_state_hash: hash,
                reserved: [0; 4],
            })
        })
    })
}

fn campaign_day_indexed_frame_info(frame: &DayIndexedFrame) -> McCampaignDayIndexedFrameInfoV2 {
    McCampaignDayIndexedFrameInfoV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayIndexedFrameInfoV2>(),
        generation: frame.generation,
        tick: frame.tick,
        width: frame.width,
        height: frame.height,
        stride: frame.width,
        reserved_0: 0,
        index_bytes: u32::try_from(frame.indices.len()).expect("Day frame index extent fits u32"),
        palette_bytes: u32::try_from(frame.palette_rgb8.len())
            .expect("Day frame palette extent fits u32"),
        index_hash: frame.index_hash,
        palette_hash: byte_hash(&frame.palette_rgb8),
        terrain_input_hash: frame.terrain_input_hash,
        terrain_command_hash: frame.terrain_command_hash,
        package_sha256: decode_sha256_hex(&frame.package_sha256),
        cache_content_sha256: decode_sha256_hex(&frame.cache_content_sha256),
        reserved: [0; 4],
    }
}

fn with_level2_campaign_day_frame<T>(
    session: &CampaignSessionRecord,
    generation: u64,
    expected_tick: u64,
    operation: impl FnOnce(&DayIndexedFrame) -> AbiResult<T>,
) -> AbiResult<T> {
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day indexed frame generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    let CampaignSessionKind::Level2 {
        reset,
        day_frame_state,
        ..
    } = &session.kind
    else {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day indexed frame requires a Level 2 campaign session",
        ));
    };
    if expected_tick != reset.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "campaign Day indexed frame tick {expected_tick} does not match {}",
                reset.state.tick
            ),
        ));
    }
    let frame = day_frame_state.current_frame().ok_or_else(|| {
        AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "campaign Day indexed frame must be rendered before copying",
        )
    })?;
    if frame.generation != generation || frame.tick != expected_tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            "campaign Day indexed frame is not bound to the requested generation/tick",
        ));
    }
    operation(frame)
}

/// Render and commit the current qualified Level 2 Day indexed frame.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_indexed_frame_render(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_info: *mut McCampaignDayIndexedFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_campaign_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day indexed frame generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let CampaignSessionKind::Level2 {
                inspection,
                reset,
                day_frame_assets,
                day_frame_state,
            } = &mut session.kind
            else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Day indexed frame requires a Level 2 campaign session",
                ));
            };
            if expected_tick != reset.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Day indexed frame tick {expected_tick} does not match {}",
                        reset.state.tick
                    ),
                ));
            }
            if let Some(frame) = day_frame_state.current_frame()
                && frame.generation == generation
                && frame.tick == expected_tick
            {
                return Ok(campaign_day_indexed_frame_info(frame));
            }
            let assets = day_frame_assets.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "campaign Day indexed frame requires a qualified owner cache",
                )
            })?;
            let frame = day_frame_state
                .render(inspection, reset, assets, generation)
                .map_err(|error| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        format!("campaign Day indexed frame render failed: {error}"),
                    )
                })?;
            Ok(campaign_day_indexed_frame_info(&frame))
        })
    })
}

/// Copy the complete committed Day indexed frame.  Rendering and copying are
/// separate so a pre-render or stale-generation request cannot expose an old
/// frame as current.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_day_indexed_frame_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    index_buffer: *mut u8,
    index_capacity: u32,
    palette_buffer: *mut u8,
    palette_capacity: u32,
    out_index_written: *mut u32,
    out_palette_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_index_written, "Day indexed frame index count")?;
        require_output(out_palette_written, "Day indexed frame palette count")?;
        write_output(out_index_written, 0);
        write_output(out_palette_written, 0);
        with_campaign_session(handle, |session| {
            with_level2_campaign_day_frame(session, generation, expected_tick, |frame| {
                let index_required =
                    u32::try_from(frame.indices.len()).expect("Day frame index extent fits u32");
                let palette_required = u32::try_from(frame.palette_rgb8.len())
                    .expect("Day frame palette extent fits u32");
                if index_capacity < index_required || palette_capacity < palette_required {
                    return Err(AbiError::new(
                        MC_STATUS_BUFFER_TOO_SMALL,
                        format!(
                            "Day indexed frame requires {index_required} index and {palette_required} palette bytes, got {index_capacity} and {palette_capacity}"
                        ),
                    ));
                }
                if index_buffer.is_null() || palette_buffer.is_null() {
                    return Err(AbiError::new(
                        MC_STATUS_NULL_POINTER,
                        "Day indexed frame index and palette buffers must be non-null",
                    ));
                }
                // SAFETY: both caller buffers are non-null and have the exact
                // required capacities checked above; frame storage is immutable.
                unsafe {
                    index_buffer
                        .copy_from_nonoverlapping(frame.indices.as_ptr(), frame.indices.len());
                    palette_buffer.copy_from_nonoverlapping(
                        frame.palette_rgb8.as_ptr(),
                        frame.palette_rgb8.len(),
                    );
                }
                write_output(out_index_written, index_required);
                write_output(out_palette_written, palette_required);
                Ok(())
            })
        })
    })
}

fn with_level2_campaign_terrain<T>(
    handle: u64,
    generation: u64,
    operation: impl FnOnce(&Level2ResetSession) -> AbiResult<T>,
) -> AbiResult<T> {
    with_campaign_session(handle, |session| {
        if generation != session.generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                format!(
                    "campaign terrain generation {} does not match session generation {}",
                    generation, session.generation
                ),
            ));
        }
        let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "campaign terrain requires a Level 2 campaign session",
            ));
        };
        operation(reset)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_state(
    handle: u64,
    generation: u64,
    out_state: *mut McCampaignTerrainStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_level2_campaign_terrain(handle, generation, |reset| {
            Ok(McCampaignTerrainStateV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignTerrainStateV2>(),
                generation,
                tick: reset.state.tick,
                mutation_sequence: reset.terrain.mutation_sequence(),
                terrain_hash: reset.terrain_hash(),
                width: 256,
                height: 256,
                plane_count: 6,
                hash_schema: 1,
                authority: MC_AUTHORITY_SESSION_TERRAIN_LIVE,
                reserved_0: 0,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_plane_length(
    handle: u64,
    generation: u64,
    kind: u32,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_level2_campaign_terrain(handle, generation, |reset| {
            validate_plane_kind(reset.terrain.planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_plane_copy_u8(
    handle: u64,
    generation: u64,
    kind: u32,
    buffer: *mut u8,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_level2_campaign_terrain(handle, generation, |reset| {
            let values = terrain_u8_plane(reset.terrain.planes(), kind)?;
            copy_slice(values, buffer, capacity, out_written)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_terrain_entity_indices_copy(
    handle: u64,
    generation: u64,
    buffer: *mut i16,
    capacity: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        with_level2_campaign_terrain(handle, generation, |reset| {
            copy_slice(
                &reset.terrain.planes().entity_indices,
                buffer,
                capacity,
                out_written,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_player_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.players().len(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.players().len(),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "player count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objective_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.runtime().objectives.len(),
                CampaignSessionKind::Level2 { inspection, .. } => {
                    inspection.runtime().objectives.len()
                }
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "objective count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entity_count(handle: u64, out_count: *mut u32) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_campaign_session(handle, |session| {
            let count = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.entities().len(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.entities().len(),
            };
            u32::try_from(count)
                .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, "entity count exceeds u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_players_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McCampaignPlayerSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignPlayerSnapshotV2>(
            element_size,
            "campaign player element",
        )?;
        with_campaign_session(handle, |session| {
            let (players, participant_count) = match &session.kind {
                CampaignSessionKind::Level1 { world } => {
                    (world.players(), world.runtime().player_count)
                }
                CampaignSessionKind::Level2 {
                    inspection, reset, ..
                } => {
                    // Level 2 menu selection, XP unlock projection, and hand
                    // assignment mutate the live shared player rows.  Return
                    // that authoritative session projection rather than the
                    // immutable owner-package inspection snapshot.
                    (
                        reset.shared_spells().players(),
                        inspection.runtime().player_count,
                    )
                }
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player start exceeds usize")
            })?;
            if start > players.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign player start out of bounds",
                ));
            }
            let count = (players.len() - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign player capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign player buffer is null",
                ));
            }
            for (offset, player) in players.iter().skip(start).take(count).enumerate() {
                let value = campaign_player_snapshot(player, player.slot < participant_count);
                unsafe { buffer.add(offset).write_unaligned(value) };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("player count fits u32"),
            );
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objectives_copy(
    handle: u64,
    start_slot: u32,
    buffer: *mut McObjectiveSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McObjectiveSnapshotV2>(element_size, "campaign objective element")?;
        with_campaign_session(handle, |session| {
            let objectives = match &session.kind {
                CampaignSessionKind::Level1 { world } => &world.runtime().objectives,
                CampaignSessionKind::Level2 { inspection, .. } => &inspection.runtime().objectives,
            };
            let start = usize::try_from(start_slot).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign objective start exceeds usize")
            })?;
            if start > objectives.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign objective start out of bounds",
                ));
            }
            let count =
                (objectives.len() - start).min(usize::try_from(capacity).map_err(|_| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "campaign objective capacity exceeds usize",
                    )
                })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign objective buffer is null",
                ));
            }
            for (offset, objective) in objectives.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer.add(offset).write_unaligned(objective_snapshot(
                        u8::try_from(start + offset).expect("objective slot fits u8"),
                        objective,
                    ))
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("objective count fits u32"),
            );
            Ok(())
        })
    })
}

/// Read one mutable Level 2 objective stage from the generation-bound reset
/// session.  This deliberately does not repurpose the frozen objective-copy
/// contract: Unity needs the authored stage axes and the live selector state
/// that the source model-32 switch consumes.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_objective_stage_state(
    handle: u64,
    generation: u64,
    objective_slot: u8,
    stage_slot: u8,
    out_state: *mut McCampaignObjectiveStageSnapshotV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign objective stage state requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign objective stage generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let runtime = reset.runtime();
            let objective = runtime
                .objectives
                .get(usize::from(objective_slot))
                .ok_or_else(|| {
                    AbiError::new(MC_STATUS_BOUNDS, "campaign objective slot is out of bounds")
                })?;
            let stage = runtime.stages.get(usize::from(stage_slot)).ok_or_else(|| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign stage slot is out of bounds")
            })?;
            let stage_state = objective
                .stage_selectors
                .get(usize::from(stage_slot))
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        "campaign objective stage slot is out of bounds",
                    )
                })?;
            Ok(campaign_objective_stage_snapshot(
                generation,
                objective_slot,
                stage_slot,
                *objective,
                stage,
                stage_state,
            ))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_entities_copy(
    handle: u64,
    start_index: u32,
    buffer: *mut McCampaignEntitySnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McCampaignEntitySnapshotV2>(
            element_size,
            "campaign entity element",
        )?;
        with_campaign_session(handle, |session| {
            let entities = match &session.kind {
                CampaignSessionKind::Level1 { world } => world.entities(),
                CampaignSessionKind::Level2 { inspection, .. } => inspection.entities(),
            };
            let start = usize::try_from(start_index).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity start exceeds usize")
            })?;
            if start > entities.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign entity start out of bounds",
                ));
            }
            let count = (entities.len() - start).min(usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign entity capacity exceeds usize")
            })?);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign entity buffer is null",
                ));
            }
            for (offset, entity) in entities.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_entity_snapshot(entity))
                };
            }
            write_output(
                out_written,
                u32::try_from(count).expect("entity count fits u32"),
            );
            Ok(())
        })
    })
}

/// Copy the live Level 2 Goat family from the campaign reset session.  The
/// legacy `mc_native_goats_copy` remains Level 1-only; this additive route
/// exposes the authored Level 2 rows after each production campaign step.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goats_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignGoatSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McGoatBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McCampaignGoatSnapshotV2>(element_size, "campaign Goat element")?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Goat snapshots require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Goat generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let goats = reset.goats();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat start exceeds usize")
            })?;
            if start > goats.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign Goat start is beyond family count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign Goat capacity exceeds usize")
            })?;
            let count = (goats.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign Goat output buffer is null",
                ));
            }
            for (offset, goat) in goats.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_goat_snapshot(goat));
                }
            }
            Ok(McGoatBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McGoatBatchV2>(),
                generation: session.generation,
                tick: u64::from(reset.goat_outer_tick()),
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 Goat count fits u32"),
                total_count: u32::try_from(LEVEL_TWO_GOAT_INDICES.len())
                    .expect("Level 2 Goat count fits u32"),
                has_more: u8::from(start + count < goats.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Queue one source-authoritative Level 2 Goat hit.  The normal campaign
/// step consumes the pending mailbox and owns the resulting life/death and
/// economy transitions; callers cannot mutate Goat state directly.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_goat_damage_submit(
    handle: u64,
    generation: u64,
    target_entity_index: u16,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_campaign_session_mut(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &mut session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign Goat damage requires a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign Goat damage generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            reset
                .submit_goat_damage(target_entity_index, damage, attacker_entity_index)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error.to_string()))
        })
    })
}

/// Copy the typed Level 2 authored spell-pickup arena. The rows are created
/// by the native authored-group route and remain bound to the shared source
/// allocator; clients use the returned positions to feed the normal campaign
/// step rather than synthesizing a pickup or mutating simulation state.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_spell_pickups_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McCampaignSpellPickupSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McCampaignSpellPickupSnapshotV2>(
            element_size,
            "campaign spell pickup element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign spell pickups require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign spell pickup generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let pickups = reset.economy().spell_pickups();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup start exceeds usize",
                )
            })?;
            if start > pickups.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign spell pickup capacity exceeds usize",
                )
            })?;
            let count = (pickups.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign spell pickup output buffer is null",
                ));
            }
            for (offset, pickup) in pickups.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(campaign_spell_pickup_snapshot(pickup));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 spell pickup count fits u32"),
                total_count: u32::try_from(pickups.len())
                    .expect("Level 2 spell pickup count fits u32"),
                has_more: u8::from(start + count < pickups.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy the live Level 2 mana-sphere arena.  This is deliberately separate
/// from the legacy Level 1 mission endpoint: the rows come from the mutable
/// `Level2EconomyState` arena after the source `sub_5FD00`/`TransformEntityToManaSphere`
/// paths, including retained inactive rows awaiting the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_spheres_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaSphereSnapshotV2>(
            element_size,
            "campaign mana sphere element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana spheres require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana sphere generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let spheres = reset.economy().spheres();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "campaign mana sphere start exceeds usize")
            })?;
            if start > spheres.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana sphere capacity exceeds usize",
                )
            })?;
            let count = (spheres.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana sphere output buffer is null",
                ));
            }
            for (offset, sphere) in spheres.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_sphere_snapshot(sphere));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana sphere count fits u32"),
                total_count: u32::try_from(spheres.len())
                    .expect("Level 2 mana sphere count fits u32"),
                has_more: u8::from(start + count < spheres.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
    })
}

/// Copy every retained Level 2 mana-carrier row in source allocation order.
/// Do not collapse this to `economy().carrier()`: remc2 can retain multiple
/// active/inactive carriers in the shared arena until the reclaim pass.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_campaign_mana_carriers_copy(
    handle: u64,
    generation: u64,
    start_ordinal: u32,
    buffer: *mut McManaCarrierSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McCampaignManaBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McManaCarrierSnapshotV2>(
            element_size,
            "campaign mana carrier element",
        )?;
        with_campaign_session(handle, |session| {
            let CampaignSessionKind::Level2 { reset, .. } = &session.kind else {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    "campaign mana carriers require a Level 2 campaign session",
                ));
            };
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "campaign mana carrier generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let carriers = reset.economy().carriers();
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start exceeds usize",
                )
            })?;
            if start > carriers.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier start is beyond arena count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "campaign mana carrier capacity exceeds usize",
                )
            })?;
            let count = (carriers.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "campaign mana carrier output buffer is null",
                ));
            }
            for (offset, carrier) in carriers.iter().skip(start).take(count).enumerate() {
                unsafe {
                    buffer
                        .add(offset)
                        .write_unaligned(mana_carrier_snapshot_value(
                            carrier,
                            session.generation,
                            reset.state.tick,
                        ));
                }
            }
            Ok(McCampaignManaBatchV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McCampaignManaBatchV2>(),
                generation: session.generation,
                tick: reset.state.tick,
                live_world_hash: reset.campaign_state_hash(),
                start_ordinal,
                written: u32::try_from(count).expect("Level 2 mana carrier count fits u32"),
                total_count: u32::try_from(carriers.len())
                    .expect("Level 2 mana carrier count fits u32"),
                has_more: u8::from(start + count < carriers.len()),
                reserved_0: [0; 3],
                reserved: [0; 4],
            })
        })
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
        with_session_mut(handle, CampaignFacade::<SealedLevel1Adapter>::reset)
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
pub extern "C" fn mc_native_session_source_arena_hash(handle: u64, out_hash: *mut u64) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| Ok(session.mission.source_arena_hash_v1()))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_fallback_state_hash_v1(
    handle: u64,
    out_hash: *mut u64,
) -> i32 {
    boundary_value(out_hash, struct_size::<u64>(), || {
        with_session(handle, |session| {
            Ok(session.mission.action_51_fallback_state_hash_v1())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_resident_observation_v1(
    handle: u64,
    out_observation: *mut McLevel1ResidentObservationV1,
    out_observation_size: u32,
) -> i32 {
    boundary_value(out_observation, out_observation_size, || {
        with_session(handle, |session| {
            Ok(McLevel1ResidentObservationV1 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1ResidentObservationV1>(),
                required_mask: session.mission.required_resident_behavior_mask(),
                observed_mask: session.mission.observed_resident_behavior_mask(),
                remaining_mask: session.mission.remaining_resident_behavior_mask(),
                reserved_0: 0,
                digest: session.mission.resident_behavior_observation_digest_v1(),
                reserved: [0; 4],
            })
        })
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
            CampaignFacade::<SealedLevel1Adapter>::step(session, command)
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

/// Copy the bounded source-ordered Level 1 world frame.
///
/// This preserves the stable terrain-only endpoint while adding the currently
/// qualified mode-20 Goat family. Other non-terrain writers remain explicitly
/// omitted in the returned flags.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_world_frame_copy(
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
        let query = read_versioned_input(query, "world frame query")?;
        validate_reserved(&query.reserved, "world frame query")?;
        with_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "world frame presentation tick {} does not match {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            let required =
                u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified world frame extent fits u32");
            if index_capacity < required || coverage_capacity < required {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!(
                        "world frame requires {required} index and coverage bytes, got {index_capacity} and {coverage_capacity}"
                    ),
                ));
            }
            if index_buffer.is_null() || coverage_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "world frame index and coverage buffers must be non-null",
                ));
            }
            let frame = session.world_frame()?;
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
                flags: world_frame_flags(
                    frame.reflected_sprites_composited,
                    frame.sprite_shadows_composited,
                ),
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

/// Copy the revisioned compositor-complete fixed-profile FINAL MAIN.
///
/// Unlike the stable 96-byte terrain/world endpoints, this additive endpoint
/// owns temporal MAIN history, indexed overlays, and per-byte provenance.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_presentation_frame_copy(
    handle: u64,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "presentation frame query")?;
        validate_reserved(&query.reserved, "presentation frame query")?;
        if query.reserved_0 != 0 {
            return Err(AbiError::new(
                MC_STATUS_RESERVED_NONZERO,
                "presentation frame query reserved_0 must be zero",
            ));
        }
        for (name, value) in [
            ("blur_capability", query.blur_capability),
            ("high_speed_system", query.high_speed_system),
            ("scratch_present", query.scratch_present),
            ("spatial_smoothing", query.spatial_smoothing),
            ("minimap_enabled", query.minimap_enabled),
            ("top_bar_enabled", query.top_bar_enabled),
            ("game_paused", query.game_paused),
            ("level_end", query.level_end),
            ("objective_textbox", query.objective_textbox),
            ("tutorial_or_briefing", query.tutorial_or_briefing),
            ("help_popup", query.help_popup),
        ] {
            if value > 1 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("presentation frame {name} must be 0 or 1"),
                ));
            }
        }
        if query.debug_text & !0x48 != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation frame debug_text accepts source flags 0x08 and 0x40 only",
            ));
        }
        if query.configured_blur_mode > 2
            || query.viewport_width > LEVEL1_VIEW_WIDTH as u16
            || query.viewport_height > LEVEL1_VIEW_HEIGHT as u16
            || query.screen_stride < query.viewport_width
        {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "presentation blur mode or viewport/stride is invalid",
            ));
        }
        let required =
            u32::try_from(LEVEL1_VIEW_PIXELS).expect("qualified presentation extent fits u32");
        if index_capacity < required || provenance_capacity < required {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "presentation frame requires {required} index and provenance bytes, got {index_capacity} and {provenance_capacity}"
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "presentation index and provenance buffers must be non-null",
            ));
        }
        with_session_mut(handle, |session| {
            if query.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation generation {} does not match {}",
                        query.generation, session.generation
                    ),
                ));
            }
            if query.tick != session.camera_history.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "presentation tick {} does not match camera history {}",
                        query.tick, session.camera_history.tick
                    ),
                ));
            }
            if (query.game_paused != 0) != session.game_paused {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    "presentation paused query does not match authoritative session state",
                ));
            }
            let result = session.presentation_frame(query)?;
            let frame = &result.frame;
            // SAFETY: both caller buffers were checked non-null with the
            // exact fixed-profile capacity.
            unsafe {
                index_buffer.copy_from_nonoverlapping(frame.indices.as_ptr(), LEVEL1_VIEW_PIXELS);
                provenance_buffer
                    .copy_from_nonoverlapping(frame.provenance.as_ptr(), LEVEL1_VIEW_PIXELS);
            }
            let effective_postprocess_mode = match frame.postprocess_branch {
                IndexedPostprocessBranch::Direct { .. } => 0,
                IndexedPostprocessBranch::Temporal { mode } => mode,
            };
            let source = frame.source_receipt;
            let family = source.family_mask;
            let source_flags = if source.sky_seeded {
                MC_TERRAIN_FRAME_SKY_SEEDED
            } else {
                0
            } | if source.has_coverage {
                MC_TERRAIN_FRAME_HAS_COVERAGE
            } else {
                0
            } | if family & 1 != 0 {
                MC_TERRAIN_FRAME_GOATS_COMPOSITED
            } else {
                0
            } | if family & (1 << 1) != 0 {
                MC_TERRAIN_FRAME_FROZEN_ACTORS_COMPOSITED
            } else {
                0
            } | if family & (1 << 3) != 0 {
                MC_TERRAIN_FRAME_ARCHERS_COMPOSITED
            } else {
                0
            } | if family & (1 << 4) != 0 {
                MC_TERRAIN_FRAME_FIREFLIES_COMPOSITED
            } else {
                0
            } | if family & (1 << 5) != 0 {
                MC_TERRAIN_FRAME_COMBAT_COMPOSITED
            } else {
                0
            } | if family & (1 << 6) != 0 {
                MC_TERRAIN_FRAME_OBJECTIVE_PROPS_COMPOSITED
            } else {
                0
            } | if family & (1 << 7) != 0 {
                MC_TERRAIN_FRAME_ECONOMY_COMPOSITED
            } else {
                0
            } | if family & (1 << 8) != 0 {
                MC_TERRAIN_FRAME_SPRITE_SHADOWS_COMPOSITED
            } else {
                0
            } | if source.reflected_sprites_composited {
                MC_TERRAIN_FRAME_REFLECTED_SPRITES_COMPOSITED
            } else {
                0
            };
            let presentation_flags = if frame.has_provenance() {
                MC_PRESENTATION_FRAME_HAS_PROVENANCE
            } else {
                0
            } | if frame.compositor_complete() {
                MC_PRESENTATION_FRAME_COMPOSITOR_COMPLETE
            } else {
                0
            } | if frame.overlays_complete() {
                MC_PRESENTATION_FRAME_OVERLAYS_COMPOSITED
            } else {
                0
            } | if frame.postprocess_complete() {
                MC_PRESENTATION_FRAME_POSTPROCESS_COMPLETE
            } else {
                0
            } | if frame.lighting_complete() {
                MC_PRESENTATION_FRAME_LIGHTING_COMPLETE
            } else {
                0
            };
            Ok(McPresentationFrameInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McPresentationFrameInfoV2>(),
                generation: frame.generation,
                tick: frame.tick,
                presentation_revision: frame.presentation_revision,
                immutable_input_hash: frame.immutable_input_hash,
                width: LEVEL1_VIEW_WIDTH as u16,
                height: LEVEL1_VIEW_HEIGHT as u16,
                stride: LEVEL1_VIEW_WIDTH as u16,
                viewport_width: query.viewport_width,
                index_bytes: required,
                provenance_bytes: required,
                flags: source_flags | presentation_flags,
                effective_postprocess_mode,
                history_state: match session.presentation_frame.history() {
                    MainHistory::Unprimed => 0,
                    MainHistory::Primed => 1,
                },
                newly_committed: u8::from(result.newly_committed),
                reserved_0: 0,
                index_hash: frame.index_hash,
                provenance_hash: frame.provenance_hash,
                lighting_hash: session.mission.dynamic_lighting_hash_v1(),
                source_arena_hash: session.mission.source_arena_hash_v2(),
                overlay_pixels_written: frame.overlay_pixels_written,
                overlay_stage_count: u16::try_from(frame.overlay_receipts.len())
                    .expect("fixed overlay stage count fits u16"),
                reserved_1: 0,
                reserved: [0; 6],
            })
        })
    })
}

/// Copy one owner-bound, profile-sized Classic HD frame.
///
/// This additive endpoint never calls the fixed 320x200 presentation
/// compositor. The Rust Classic HD graph rerenders terrain and supported
/// typed sprites into its own profile-sized MAIN/history planes, then copies
/// those planes to the caller. Unsupported source families fail closed.
fn validate_classic_hd_frame_query(query: &McPresentationFrameQueryV2) -> AbiResult<()> {
    validate_reserved(&query.reserved, "Classic HD frame query")?;
    if query.reserved_0 != 0 {
        return Err(AbiError::new(
            MC_STATUS_RESERVED_NONZERO,
            "Classic HD frame query reserved_0 must be zero",
        ));
    }
    for (name, value) in [
        ("blur_capability", query.blur_capability),
        ("high_speed_system", query.high_speed_system),
        ("scratch_present", query.scratch_present),
        ("spatial_smoothing", query.spatial_smoothing),
        ("minimap_enabled", query.minimap_enabled),
        ("top_bar_enabled", query.top_bar_enabled),
        ("game_paused", query.game_paused),
        ("level_end", query.level_end),
        ("objective_textbox", query.objective_textbox),
        ("tutorial_or_briefing", query.tutorial_or_briefing),
        ("help_popup", query.help_popup),
    ] {
        if value > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!("Classic HD frame {name} must be 0 or 1"),
            ));
        }
    }
    if query.debug_text & !0x48 != 0 {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "Classic HD frame debug_text accepts source flags 0x08 and 0x40 only",
        ));
    }
    if query.configured_blur_mode > 2 {
        return Err(AbiError::new(
            MC_STATUS_INVALID_ARGUMENT,
            "Classic HD configured blur mode must be zero, one, or two",
        ));
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_classic_hd_frame_copy(
    handle: u64,
    profile_id: u32,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "Classic HD frame query")?;
        validate_classic_hd_frame_query(&query)?;
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        let required_u32 = u32::try_from(required).expect("Classic HD extent fits u32");
        if index_capacity < required_u32 || provenance_capacity < required_u32 {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required_u32} index and provenance bytes, got {index_capacity} and {provenance_capacity}",
                    profile.id.as_str()
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "Classic HD index and provenance buffers must be non-null",
            ));
        }
        // SAFETY: the capacities were checked against the validated profile
        // extent and the pointers were checked non-null above.
        let indices = unsafe { slice::from_raw_parts_mut(index_buffer, required) };
        let provenance = unsafe { slice::from_raw_parts_mut(provenance_buffer, required) };
        with_session_mut(handle, |session| {
            session.classic_hd_frame_copy(profile_id, query, indices, provenance)
        })
    })
}

/// Copy only the profile-sized owner HUD overlay for Unity Enhanced.
///
/// This endpoint shares the exact owner compositor and profile scaling with
/// Classic HD, but deliberately does not render the Classic HD world graph.
/// The destination is transparent except for provenance-marked HUD pixels;
/// Unity's Enhanced world remains the only world image on that route.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_classic_hd_owner_overlay_copy(
    handle: u64,
    profile_id: u32,
    query: *const McPresentationFrameQueryV2,
    index_buffer: *mut u8,
    index_capacity: u32,
    provenance_buffer: *mut u8,
    provenance_capacity: u32,
    out_info: *mut McPresentationFrameInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        let query = read_versioned_input(query, "Classic HD owner overlay query")?;
        validate_classic_hd_frame_query(&query)?;
        let profile = classic_hd_profile_for_id(profile_id)?;
        let required = profile.extent();
        let required_u32 = u32::try_from(required).expect("Classic HD extent fits u32");
        if index_capacity < required_u32 || provenance_capacity < required_u32 {
            return Err(AbiError::new(
                MC_STATUS_BUFFER_TOO_SMALL,
                format!(
                    "Classic HD profile {} requires {required_u32} owner overlay bytes per plane, got {index_capacity} and {provenance_capacity}",
                    profile.id.as_str()
                ),
            ));
        }
        if index_buffer.is_null() || provenance_buffer.is_null() {
            return Err(AbiError::new(
                MC_STATUS_NULL_POINTER,
                "Classic HD owner overlay index and provenance buffers must be non-null",
            ));
        }
        // SAFETY: the capacities were checked against the validated profile
        // extent and the pointers were checked non-null above.
        let indices = unsafe { slice::from_raw_parts_mut(index_buffer, required) };
        let provenance = unsafe { slice::from_raw_parts_mut(provenance_buffer, required) };
        with_session_mut(handle, |session| {
            session.classic_hd_owner_overlay_copy(profile_id, query, indices, provenance)
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
            validate_plane_kind(session.terrain.planes(), kind)?;
            Ok(u32::try_from(TERRAIN_CELLS).expect("terrain count fits u32"))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_terrain_state(
    handle: u64,
    out_state: *mut McTerrainStateV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(terrain_state(session)))
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
            let values = terrain_u8_plane(session.terrain.planes(), kind)?;
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
            let values = &session.terrain.planes().entity_indices;
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
                        &session.mission,
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
                live_world_hash: session.live_world_hash_v2(),
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
pub extern "C" fn mc_native_level1_mission_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1MissionSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_mission_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1PlayerLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_player_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_menu_state(handle: u64, menu_state: u8) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            let current = session.mission.player_menu_state;
            session.mission.set_player_menu_state(menu_state);
            if current != menu_state && menu_state != 8 && !matches!(current, 5 | 8) {
                session.emit_local_flight_sound(sim_core::menu_cursor_transition_audio_event(
                    session.state.entity_index,
                    i16::from(session.world.runtime().local_player_slot),
                ));
            }
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_game_paused(handle: u64, paused: u8) -> i32 {
    boundary(|| {
        if paused > 1 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "Level 1 paused state must be zero or one",
            ));
        }
        with_session_mut(handle, |session| {
            session.set_game_paused(paused != 0);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_presentation_input(
    handle: u64,
    input: *const McLevel1PresentationInputV2,
) -> i32 {
    boundary(|| {
        let input = read_versioned_input(input, "Level 1 presentation input")?;
        with_session_mut(handle, |session| session.apply_presentation_input(input))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_player_inject_damage_for_qualification(
    handle: u64,
    damage: i32,
    attacker_entity_index: u16,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            session
                .mission
                .submit_player_damage(damage, attacker_entity_index);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_blocker_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    house_action: u8,
) -> i32 {
    boundary(|| {
        if !matches!(
            house_action,
            sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION | u8::MAX
        ) {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                "action-51 qualification house action must be 51 or 255",
            ));
        }
        with_session_mut(handle, |session| {
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = house_action;
            house.max_life = 2;
            house.life = 2;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            mission
                .inject_action_51_blocker_for_qualification(
                    house.position,
                    house.collision_half_width,
                    house.collision_half_depth,
                )
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "action-51 qualification blocker allocation failed",
                    )
                })?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_action_51_completion_setup_for_qualification(
    handle: u64,
    house_entity_index: u16,
    parent_entity_index: u16,
    construction_selector: u8,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if session
                .world
                .house_build_footprint(construction_selector)
                .is_none()
            {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("action-51 qualification BUILD {construction_selector} is absent"),
                ));
            }
            let mut mission = session.mission.clone();
            let mut live_world = session.live_world.clone();
            let views = level1_turn_start_source_views(&mission, &session.state, &live_world);
            let parent = views
                .iter()
                .copied()
                .find(|view| view.source_entity_index == parent_entity_index)
                .filter(|view| view.class == 3 && view.life >= 0)
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "action-51 qualification parent {parent_entity_index} is not a live class-3 row"
                        ),
                    )
                })?;
            let construction_axis = Position {
                x: parent.position.x.wrapping_add(0x0101),
                y: parent.position.y.wrapping_add(0x0202),
                z: parent.position.z.wrapping_add(33),
            };
            if !mission.configure_action_51_completion_parent_for_qualification(
                parent_entity_index,
                construction_selector,
                construction_axis,
            ) {
                return Err(AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "action-51 qualification parent source fields could not be configured",
                ));
            }
            let mut house = live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("action-51 qualification house {house_entity_index} is absent"),
                    )
                })?;
            house.action = sim_core::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
            house.max_life = 1;
            house.life = 1;
            house.owner_entity_index = session.state.entity_index;
            house.control_flags |= 1;
            house.position = parent.position;
            live_world
                .initialize_builder_house(house)
                .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
            session.mission = mission;
            session.live_world = live_world;
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_hud_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1HudSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_hud_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_spell_menu_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1SpellMenuSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_spell_menu_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_marks(
    handle: u64,
    out_state: *mut McLevel1MinimapMarksV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_minimap_marks(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_minimap_entities_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    buffer: *mut McLevel1MinimapEntityCommandV2,
    capacity: u32,
    element_size: u32,
    out_info: *mut McLevel1MinimapEntitiesInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        require_element_size::<McLevel1MinimapEntityCommandV2>(
            element_size,
            "Level 1 minimap entity command",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            let commands = level1_minimap_entity_commands(session);
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity exceeds usize",
                )
            })?;
            if capacity < commands.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "Level 1 minimap command capacity is smaller than the atomic stream",
                ));
            }
            if !commands.is_empty() && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 minimap command buffer is null",
                ));
            }
            for (offset, command) in commands.iter().enumerate() {
                // SAFETY: element size and caller capacity were validated.
                unsafe {
                    buffer.add(offset).write_unaligned(*command);
                }
            }
            Ok(McLevel1MinimapEntitiesInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1MinimapEntitiesInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                total_count: u32::try_from(commands.len())
                    .expect("Level 1 minimap command count fits u32"),
                cross_x: 63,
                cross_y: 64,
                cross_colour: 176,
                cross_arm_length: 10,
                reveal_active: 0,
                authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
                reserved_0: [0; 4],
                command_hash: level1_minimap_command_hash(&commands),
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_palette_copy(
    handle: u64,
    rgb8_buffer: *mut u8,
    rgb8_capacity: u32,
    out_info: *mut McLevel1PaletteInfoV2,
    out_info_size: u32,
) -> i32 {
    boundary_value(out_info, out_info_size, || {
        with_session(handle, |session| {
            if rgb8_buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "Level 1 effective palette buffer must be non-null",
                ));
            }
            if rgb8_capacity < 768 {
                return Err(AbiError::new(
                    MC_STATUS_BUFFER_TOO_SMALL,
                    format!("Level 1 effective palette requires 768 bytes, got {rgb8_capacity}"),
                ));
            }
            let rgb8 = session.level1_palette_rgb8()?;
            // SAFETY: the caller supplied a non-null buffer with at least the
            // checked 768-byte capacity.
            unsafe {
                rgb8_buffer.copy_from_nonoverlapping(rgb8.as_ptr(), rgb8.len());
            }
            let digest = Sha256::digest(rgb8);
            let mut rgb8_sha256 = [0_u8; 32];
            rgb8_sha256.copy_from_slice(&digest);
            Ok(McLevel1PaletteInfoV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1PaletteInfoV2>(),
                generation: session.generation,
                tick: session.state.tick,
                submode: session.mission.exit_palette_submode,
                effect: session.mission.exit_palette_effect,
                weight: session.mission.exit_palette_weight,
                denominator: session.mission.exit_palette_denominator,
                palette_count: session.mission.palette_count(),
                flags: 1,
                rgb8_sha256,
                reserved: [0; 4],
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_exit_presentation(
    handle: u64,
    out_state: *mut McLevel1ExitPresentationV2,
    out_state_size: u32,
) -> i32 {
    boundary_value(out_state, out_state_size, || {
        with_session(handle, |session| Ok(level1_exit_presentation(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(level1_castle_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_castle_lifecycle_snapshot(
    handle: u64,
    out_snapshot: *mut McLevel1CastleLifecycleSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            Ok(level1_castle_lifecycle_snapshot(session))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_count(
    handle: u64,
    out_count: *mut u32,
) -> i32 {
    boundary_value(out_count, struct_size::<u32>(), || {
        with_session(handle, |session| {
            Ok(
                u32::try_from(level1_house_completion_effects(session).len())
                    .expect("completion effect count fits u32"),
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_house_completion_effect_snapshot(
    handle: u64,
    effect_ordinal: u32,
    out_snapshot: *mut McLevel1HouseCompletionEffectV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| {
            level1_house_completion_effects(session)
                .get(usize::try_from(effect_ordinal).expect("u32 ordinal fits usize"))
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!(
                            "Level 1 house-completion effect ordinal {effect_ordinal} is out of bounds"
                        ),
                    )
                })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_carrier_snapshot(
    handle: u64,
    out_snapshot: *mut McManaCarrierSnapshotV2,
    out_snapshot_size: u32,
) -> i32 {
    boundary_value(out_snapshot, out_snapshot_size, || {
        with_session(handle, |session| Ok(mana_carrier_snapshot(session)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_mana_spheres_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McManaSphereSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McManaSphereSnapshotV2>(element_size, "mana sphere element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.mana_spheres(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                mana_sphere_snapshot,
                "mana sphere",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_actors_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1ActorSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1ActorSnapshotV2>(element_size, "Level 1 actor element")?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.actors(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_actor_snapshot,
                "Level 1 actor",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_level1_multipart_segments_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McLevel1MultipartSegmentSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McLevel1MultipartSegmentSnapshotV2>(
            element_size,
            "Level 1 multipart segment element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.multipart_segments(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                level1_multipart_segment_snapshot,
                "Level 1 multipart segment",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_projectiles_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballProjectileSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballProjectileSnapshotV2>(
            element_size,
            "Fireball projectile element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.projectiles(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_projectile_snapshot,
                "Fireball projectile",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_fireball_impacts_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McFireballImpactSnapshotV2,
    capacity: u32,
    element_size: u32,
    out_written: *mut u32,
) -> i32 {
    boundary(|| {
        require_output(out_written, "out_written")?;
        require_element_size::<McFireballImpactSnapshotV2>(
            element_size,
            "Fireball impact element",
        )?;
        with_session(handle, |session| {
            validate_mission_snapshot_cursor(session, generation, expected_tick)?;
            copy_mission_slice(
                session.mission.impacts(),
                start_ordinal,
                buffer,
                capacity,
                out_written,
                fireball_impact_snapshot,
                "Fireball impact",
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_selector_frame_states_copy(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    start_ordinal: u32,
    buffer: *mut McSelectorFrameStateV2,
    capacity: u32,
    element_size: u32,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        require_element_size::<McSelectorFrameStateV2>(element_size, "selector-frame element")?;
        with_session(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector-frame tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            let frames = session.selector_frames.as_ref().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            let bank = session.world.goat_tmap_bank().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame bank unavailable without qualified owner cache",
                )
            })?;
            let start = usize::try_from(start_ordinal).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame start exceeds usize")
            })?;
            if start > GOAT_TMAP_SELECTORS.len() {
                return Err(AbiError::new(
                    MC_STATUS_BOUNDS,
                    "selector-frame start is beyond selector count",
                ));
            }
            let capacity = usize::try_from(capacity).map_err(|_| {
                AbiError::new(MC_STATUS_BOUNDS, "selector-frame capacity exceeds usize")
            })?;
            let count = (GOAT_TMAP_SELECTORS.len() - start).min(capacity);
            if count > 0 && buffer.is_null() {
                return Err(AbiError::new(
                    MC_STATUS_NULL_POINTER,
                    "selector-frame output buffer is null",
                ));
            }
            for (offset, clock) in frames.states().iter().skip(start).take(count).enumerate() {
                // SAFETY: exact element size and writable capacity were
                // supplied for `count` rows.
                unsafe {
                    buffer.add(offset).write_unaligned(selector_frame_state(
                        start + offset,
                        clock,
                        bank,
                    ));
                }
            }
            session.selector_frame_batch(
                start_ordinal,
                u32::try_from(count).expect("three selector frames fit u32"),
            )
        })
    })
}

/// Run the qualified original CPU Goat draw-admission pass for the completed
/// session tick and OR its three-bit result into the selector used latches.
///
/// This function consumes only authoritative Rust simulation, camera-history,
/// terrain-lattice, and owner-cache state.  Managed/GPU visibility is not an
/// input.
#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_goat_draw_admission_evaluate(
    handle: u64,
    generation: u64,
    expected_tick: u64,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            if expected_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "Goat draw-admission tick {expected_tick} does not match {}",
                        session.state.tick
                    ),
                ));
            }
            session.evaluate_goat_draw_admission()
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_session_selector_draw_feedback_submit(
    handle: u64,
    command: *const McSelectorDrawFeedbackV2,
    out_batch: *mut McSelectorFrameBatchV2,
    out_batch_size: u32,
) -> i32 {
    boundary_value(out_batch, out_batch_size, || {
        let command = read_versioned_input(command, "selector draw feedback")?;
        validate_reserved(&command.reserved, "selector draw feedback")?;
        if command.selector_mask & !u64::from(GOAT_TMAP_DRAW_MASK) != 0 {
            return Err(AbiError::new(
                MC_STATUS_INVALID_ARGUMENT,
                format!(
                    "selector draw mask {:#x} has unsupported bits",
                    command.selector_mask
                ),
            ));
        }
        with_session_mut(handle, |session| {
            if command.generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw generation {} does not match {}",
                        command.generation, session.generation
                    ),
                ));
            }
            if command.draw_tick != session.state.tick {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "selector draw tick {} does not match {}",
                        command.draw_tick, session.state.tick
                    ),
                ));
            }
            let mask =
                u8::try_from(command.selector_mask).expect("validated selector draw mask fits u8");
            if let Some((tick, prior_mask)) = session.selector_draw_feedback
                && tick == command.draw_tick
            {
                let combined_mask = prior_mask | mask;
                if combined_mask == prior_mask {
                    let mut receipt = session.selector_frame_batch(0, 0)?;
                    receipt.has_more = 0;
                    return Ok(receipt);
                }
                let frames = session.selector_frames.as_mut().ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_VALIDATION_FAILED,
                        "selector-frame state unavailable without qualified owner cache",
                    )
                })?;
                frames
                    .latch_draw_attempts(mask)
                    .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
                session.selector_draw_feedback = Some((command.draw_tick, combined_mask));
                let mut receipt = session.selector_frame_batch(0, 0)?;
                receipt.has_more = 0;
                return Ok(receipt);
            }
            let frames = session.selector_frames.as_mut().ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_VALIDATION_FAILED,
                    "selector-frame state unavailable without qualified owner cache",
                )
            })?;
            frames
                .latch_draw_attempts(mask)
                .map_err(|error| AbiError::new(MC_STATUS_INVALID_ARGUMENT, error))?;
            session.selector_draw_feedback = Some((command.draw_tick, mask));
            let mut receipt = session.selector_frame_batch(0, 0)?;
            receipt.has_more = 0;
            Ok(receipt)
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
                live_world_hash: session.live_world_hash_v2(),
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
pub extern "C" fn mc_native_audio_command_release(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio release generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session.audio_commands.get(&event_sequence).ok_or_else(|| {
                AbiError::new(
                    MC_STATUS_BOUNDS,
                    format!("event sequence {event_sequence} has no positional audio command"),
                )
            })?;
            if command.action == 0 {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn mc_native_audio_command_discard(
    handle: u64,
    generation: u64,
    event_sequence: u64,
) -> i32 {
    boundary(|| {
        with_session_mut(handle, |session| {
            if generation != session.generation {
                return Err(AbiError::new(
                    MC_STATUS_GENERATION_MISMATCH,
                    format!(
                        "audio discard generation {generation} does not match {}",
                        session.generation
                    ),
                ));
            }
            let command = session
                .audio_commands
                .get(&event_sequence)
                .copied()
                .ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("event sequence {event_sequence} has no positional audio command"),
                    )
                })?;
            if command.action == MC_AUDIO_ACTION_PREPARED_PENDING {
                return Err(AbiError::new(
                    MC_STATUS_INVALID_ARGUMENT,
                    format!("event sequence {event_sequence} is still pending"),
                ));
            }
            if matches!(
                command.action,
                MC_AUDIO_ACTION_START
                    | MC_AUDIO_ACTION_RESTART
                    | MC_AUDIO_ACTION_UPDATE_EXISTING
                    | MC_AUDIO_ACTION_END_LOOP
            ) {
                let index = usize::from(command.voice_index);
                let voice = session.audio_voices.get_mut(index).ok_or_else(|| {
                    AbiError::new(
                        MC_STATUS_BOUNDS,
                        format!("audio voice {} is out of bounds", command.voice_index),
                    )
                })?;
                let owns_voice = voice.is_some_and(|voice| {
                    voice.sound_id == command.sound_id
                        && voice.sound_flags == command.sound_flags
                        && if matches!(
                            command.action,
                            MC_AUDIO_ACTION_UPDATE_EXISTING | MC_AUDIO_ACTION_END_LOOP
                        ) {
                            voice.event_sequence <= command.event_sequence
                        } else {
                            voice.event_sequence == command.event_sequence
                        }
                });
                if owns_voice {
                    *voice = None;
                }
            }
            session.audio_commands.remove(&event_sequence);
            Ok(())
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

fn validate_embedded_command(command: &McCommandV2) -> AbiResult<()> {
    if command.abi_version != MC_NATIVE_ABI_VERSION {
        return Err(AbiError::new(
            MC_STATUS_UNSUPPORTED_VERSION,
            format!(
                "campaign step command ABI version {} is unsupported",
                command.abi_version
            ),
        ));
    }
    let required = struct_size::<McCommandV2>();
    if command.struct_size < required {
        return Err(AbiError::new(
            MC_STATUS_STRUCT_TOO_SMALL,
            format!(
                "campaign step command struct_size {} is smaller than {required}",
                command.struct_size
            ),
        ));
    }
    validate_command(command)
}

fn campaign_event_snapshot(
    generation: u64,
    ordinal: usize,
    event: &Level2DispatchEvent,
) -> McCampaignEventV2 {
    let (pass, projected_kind, family, flags, source_slot) = event.campaign_projection();
    let kind = match event {
        Level2DispatchEvent::Economy { .. } => MC_CAMPAIGN_EVENT_ECONOMY,
        Level2DispatchEvent::Goat { .. } => MC_CAMPAIGN_EVENT_GOAT,
        Level2DispatchEvent::SpellAcquisition { .. } => MC_CAMPAIGN_EVENT_SPELL_ACQUISITION,
        Level2DispatchEvent::AuthoredGroupActivated { .. } => MC_CAMPAIGN_EVENT_AUTHORED_GROUP,
        _ => projected_kind,
    };
    McCampaignEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        kind,
        family,
        flags,
        source_slot,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn campaign_shield_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::ShieldState>,
) -> McCampaignShieldSnapshotV2 {
    let mut snapshot = McCampaignShieldSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignShieldSnapshotV2>(),
        generation,
        ..McCampaignShieldSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.parent_status_flags = state.parent_status_flags;
        snapshot.experience_awarded = state.experience_awarded;
    }
    snapshot
}

fn campaign_invisible_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::InvisibleState>,
) -> McCampaignInvisibleSnapshotV2 {
    let mut snapshot = McCampaignInvisibleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignInvisibleSnapshotV2>(),
        generation,
        ..McCampaignInvisibleSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.parent_visibility_mode = state.parent_visibility_mode;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.mana_regen_requirement = state.mana_regen_requirement;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.parent_status_flags = state.parent_status_flags;
        snapshot.spell_status_flags = state.spell_status_flags;
        snapshot.experience_awarded = state.experience_awarded;
        snapshot.cleanup_requested = u8::from(state.cleanup_requested);
    }
    snapshot
}

fn campaign_beyond_sight_snapshot(
    generation: u64,
    state: Option<sim_core::spell_activation::BeyondSightState>,
) -> McCampaignBeyondSightSnapshotV2 {
    let mut snapshot = McCampaignBeyondSightSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignBeyondSightSnapshotV2>(),
        generation,
        ..McCampaignBeyondSightSnapshotV2::default()
    };
    if let Some(state) = state {
        snapshot.active = 1;
        snapshot.tier = state.tier;
        snapshot.action = state.action;
        snapshot.holder_entity_index = state.holder_entity_index;
        snapshot.parent_entity_index = state.parent_entity_index;
        snapshot.active_value = state.active_value;
        snapshot.activation_value = state.activation_value;
        snapshot.countdown = state.countdown;
        snapshot.mana_cost = state.mana_cost;
        snapshot.mana_regen_requirement = state.mana_regen_requirement;
        snapshot.parent_mana = state.parent_mana;
        snapshot.parent_life = state.parent_life;
        snapshot.parent_mana_regen = state.parent_mana_regen;
        snapshot.experience_awarded = state.experience_awarded;
        snapshot.cleanup_requested = u8::from(state.cleanup_requested);
    }
    snapshot
}

fn campaign_opponent_snapshot(
    generation: u64,
    participant: sim_core::native_world::Level2NyphurParticipant,
    visibility: sim_core::native_world::Level2NyphurVisibility,
) -> McCampaignOpponentSnapshotV2 {
    let (objective_relation_domain, objective_relation_value) =
        match participant.objective_stage_relation {
            sim_core::native_world::RuntimeRelation::LiveEntity(index) => (2, index),
            _ => (0, 0),
        };
    McCampaignOpponentSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignOpponentSnapshotV2>(),
        generation,
        active: 1,
        player_slot: participant.player_slot,
        is_ai: u8::from(participant.is_ai),
        color_index: participant.color_index,
        entity_index: participant.entity_index,
        entity_class: participant.entity_class,
        entity_model: participant.entity_model,
        entity_action: participant.entity_action,
        entity_source_id: participant.entity_source_id,
        entity_next_relation: participant.entity_next_relation.unwrap_or(u16::MAX),
        entity_next_entity: participant.entity_next_entity,
        entity_control_flags: participant.entity_control_flags,
        entity_life: participant.entity_life,
        entity_max_life: participant.entity_max_life,
        entity_mana: participant.entity_mana,
        entity_max_mana: participant.entity_max_mana,
        fireball_holder_index: participant.fireball_holder_index,
        fireball_action: participant.fireball_action,
        fireball_parent_entity_index: participant.fireball_parent_entity_index,
        fireball_next_entity: participant.fireball_next_entity,
        possession_holder_index: participant.possession_holder_index,
        possession_action: participant.possession_action,
        possession_parent_entity_index: participant.possession_parent_entity_index,
        possession_next_entity: participant.possession_next_entity,
        objective_relation_domain,
        objective_relation_value,
        visibility_flags: u32::from(visibility.visible_to_local_player)
            | (u32::from(visibility.owner_visibility_mode != 0) << 1)
            | (u32::from(visibility.beyond_sight_holder_present) << 2)
            | (u32::from(visibility.beyond_sight_active) << 3)
            | (u32::from(visibility.family4_gate_active) << 4),
        ..McCampaignOpponentSnapshotV2::default()
    }
}

fn campaign_day_hud_spell_slot(slot: Level2DayHudSpellSlot) -> McCampaignDayHudSpellSlotV2 {
    McCampaignDayHudSpellSlotV2 {
        holder_entity_index: slot.holder_entity_index,
        parent_entity_index: slot.parent_entity_index,
        raw_spell_selector: slot.raw_spell_selector,
        color_index: slot.color_index,
        model: slot.model,
        tier: slot.tier,
        action: slot.action,
        action_marker: slot.action_marker,
        active_value: slot.active_value,
        activation_value: slot.activation_value,
        control_flags: slot.control_flags,
        max_mana: slot.max_mana,
        mana_regen: slot.mana_regen,
        caster_mana: slot.caster_mana,
        frame_selector: slot.frame_selector,
        icon_selector: slot.icon_selector,
        level_text_selector: slot.level_text_selector,
        draw_admitted: u8::from(slot.draw_admitted),
        reserved: [0; 2],
    }
}

fn campaign_day_hud_snapshot(
    generation: u64,
    snapshot: Level2DayHudSnapshot,
) -> McCampaignDayHudSnapshotV2 {
    let nyphur =
        campaign_opponent_snapshot(generation, snapshot.nyphur, snapshot.nyphur_visibility);
    McCampaignDayHudSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignDayHudSnapshotV2>(),
        generation,
        tick: snapshot.tick,
        local_player_slot: snapshot.local_player_slot,
        local_player_active: u8::from(snapshot.local_player_active),
        local_player_color_index: snapshot.local_player_color_index,
        hit_frame_time_pre_draw: snapshot.hit_frame_time_pre_draw,
        local_player_entity_index: snapshot.local_player_entity_index,
        color_phase_1: snapshot.color_phase_1,
        color_phase_2: snapshot.color_phase_2,
        local_player_control_flags: snapshot.local_player_control_flags,
        local_player_life: snapshot.local_player_life,
        local_player_max_life: snapshot.local_player_max_life,
        local_player_mana: snapshot.local_player_mana,
        local_player_max_mana: snapshot.local_player_max_mana,
        global_mana_denominator: snapshot.global_mana_denominator,
        minimap_panel_selector: snapshot.minimap_panel_selector,
        castle_panel_selector: snapshot.castle_panel_selector,
        balloon_panel_selector: snapshot.balloon_panel_selector,
        player_panel_selector: snapshot.player_panel_selector,
        player_icon_selector: snapshot.player_icon_selector,
        life_mana_icon_selector: snapshot.life_mana_icon_selector,
        life_palette_index: snapshot.life_palette_index,
        mana_palette_index: snapshot.mana_palette_index,
        mana_max_palette_index: snapshot.mana_max_palette_index,
        reserved_0: 0,
        nyphur,
        left: campaign_day_hud_spell_slot(snapshot.left),
        right: campaign_day_hud_spell_slot(snapshot.right),
    }
}

fn campaign_economy_event_snapshot(
    generation: u64,
    ordinal: usize,
    pass: u8,
    source_slot: u16,
    event: Level2EconomyEvent,
) -> McCampaignEconomyEventV2 {
    let (code, primary_id, secondary_id, entity_0, entity_1, value_0, value_1, flags) = match event
    {
        Level2EconomyEvent::ManaSphereSpawned {
            sphere_id,
            source_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_SPAWNED,
            sphere_id,
            0,
            source_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereClaimed {
            sphere_id,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_CLAIMED,
            sphere_id,
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSphereCollected {
            sphere_id,
            carrier_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERE_COLLECTED,
            sphere_id,
            carrier_id,
            0,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaSpheresMerged {
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_SPHERES_MERGED,
            target_sphere_id,
            source_sphere_id,
            owner_entity_index,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierSpawned {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_SPAWNED,
            u32::from(source_entity_index),
            0,
            source_entity_index,
            owner_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::ManaCarrierArrived {
            carrier_id,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_MANA_CARRIER_ARRIVED,
            carrier_id,
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderArrived {
            defender_slot,
            castle_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_ARRIVED,
            u32::from(defender_slot),
            0,
            castle_id,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCollectedSphere {
            defender_slot,
            sphere_id,
            mana,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_COLLECTED_SPHERE,
            u32::from(defender_slot),
            sphere_id,
            defender_slot,
            0,
            mana,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstSpawned {
            source_entity_index,
            parent_entity_index,
            id_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_SPAWNED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            id_entity_index,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::DeathBurstExpired {
            source_entity_index,
            parent_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_DEATH_BURST_EXPIRED,
            u32::from(source_entity_index),
            0,
            parent_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleBuilt {
            source_entity_index,
            owner_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_BUILT,
            u32::from(source_entity_index),
            0,
            owner_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestroyed {
            source_entity_index,
            killer_entity_index,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTROYED,
            u32::from(source_entity_index),
            0,
            killer_entity_index,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleConstructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_CONSTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDestructionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DESTRUCTION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::CastleDefenderCaveSound {
            defender_slot,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_DEFENDER_CAVE_SOUND,
            u32::from(defender_slot),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::FireSphereCollisionSound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_FIRE_SPHERE_COLLISION_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        Level2EconomyEvent::AuthoredClass5Model3Sound {
            source_entity_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND,
            u32::from(source_entity_index),
            0,
            sound_id,
            0,
            0,
            0,
            0,
        ),
        // Keep the generic prepared-sound payload on the existing typed
        // source-audio code; family 8 distinguishes it without allocating a
        // new ABI code.
        Level2EconomyEvent::PreparedSound {
            source_entity_index,
            entity_index,
            player_index,
            sound_id,
        } => (
            MC_CAMPAIGN_ECONOMY_AUTHORED_CLASS5_MODEL3_SOUND,
            u32::from(source_entity_index),
            u32::from(entity_index),
            sound_id,
            0,
            i32::from(player_index),
            0,
            0,
        ),
        Level2EconomyEvent::CastleStageDestroyed {
            source_entity_index,
            previous_level,
            next_level,
        } => (
            MC_CAMPAIGN_ECONOMY_CASTLE_STAGE_DESTROYED,
            u32::from(source_entity_index),
            0,
            0,
            0,
            i32::from(previous_level),
            i32::from(next_level),
            0,
        ),
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage,
            lethal,
        } => (
            MC_CAMPAIGN_ECONOMY_DAMAGE_APPLIED,
            u32::from(target_entity_index),
            0,
            attacker_entity_index,
            0,
            damage,
            0,
            u8::from(lethal),
        ),
    };
    McCampaignEconomyEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEconomyEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign event ordinal fits u32"),
        pass,
        family: sim_core::native_world::level2_economy_event_family(event),
        code,
        flags,
        source_slot,
        reserved_0: 0,
        primary_id,
        secondary_id,
        entity_0,
        entity_1,
        value_0,
        value_1,
        reserved: [0; 4],
    }
}

fn campaign_goat_event_snapshot(
    generation: u64,
    ordinal: usize,
    event_index: usize,
    pass: u8,
    source_slot: u16,
    event: GoatStepEvent,
) -> McCampaignGoatEventV2 {
    let mut row = McCampaignGoatEventV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignGoatEventV2>(),
        generation,
        ordinal: u32::try_from(ordinal).expect("bounded campaign Goat ordinal fits u32"),
        pass,
        source_slot,
        event_index: u16::try_from(event_index)
            .expect("bounded campaign Goat event index fits u16"),
        asset_ref_index: MC_ASSET_REF_NONE,
        ..McCampaignGoatEventV2::default()
    };
    match event {
        GoatStepEvent::Bleat {
            entity_index,
            sound_id,
            position,
            action,
            ..
        } => {
            row.code = MC_CAMPAIGN_GOAT_BLEAT;
            row.entity_0 = entity_index;
            row.sound_id = sound_id;
            row.action = action;
            row.x = position.x;
            row.y = position.y;
            row.z = position.z;
        }
        GoatStepEvent::ManaConverted { entity_index, mana } => {
            row.code = MC_CAMPAIGN_GOAT_MANA_CONVERTED;
            row.entity_0 = entity_index;
            row.value_0 = mana;
        }
        GoatStepEvent::ExtraManaSphereRequested { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_EXTRA_MANA_SPHERE_REQUEST;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::RemovalQueued { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_REMOVAL_QUEUED;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::Reclaimed { entity_index } => {
            row.code = MC_CAMPAIGN_GOAT_RECLAIMED;
            row.entity_0 = entity_index;
        }
        GoatStepEvent::EngagementBoundaryReached {
            entity_index,
            target_entity_index,
        } => {
            row.code = MC_CAMPAIGN_GOAT_ENGAGEMENT_BOUNDARY;
            row.entity_0 = entity_index;
            row.entity_1 = target_entity_index;
        }
    }
    row
}

fn decode_sha256_hex(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    let mut digest = [0_u8; 32];
    if bytes.len() != 64 {
        return digest;
    }
    let mut index = 0;
    while index < 32 {
        let high = hex_nibble(bytes[index * 2]);
        let low = hex_nibble(bytes[index * 2 + 1]);
        let Some(high) = high else { return [0; 32] };
        let Some(low) = low else { return [0; 32] };
        digest[index] = (high << 4) | low;
        index += 1;
    }
    digest
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
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
    mission: &Level1MissionState,
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
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
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
    if snapshot.active != 0 && !mission.imported_source_slot_is_live(entity.index) {
        snapshot.authority = MC_AUTHORITY_INACTIVE;
        snapshot.active = 0;
    }
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
    } else if entity.authority == EntityAuthority::VillagerLive
        && mission.imported_source_slot_is_live(entity.index)
        && let Some(villager) = live_world
            .villagers()
            .iter()
            .find(|villager| villager.entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_VILLAGER_LIVE;
        snapshot.active = u8::from(villager.active);
        snapshot.action = villager.action;
        snapshot.phase = villager.phase;
        snapshot.x = villager.position.x;
        snapshot.y = villager.position.y;
        snapshot.z = villager.position.z;
        snapshot.yaw = villager.yaw;
        snapshot.pitch = villager.pitch;
        snapshot.actual_speed = villager.actual_speed;
    }
    if let Some(resident) = mission
        .dynamic_residents()
        .find(|resident| resident.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(resident.active);
        snapshot.class_id = resident.class;
        snapshot.model = resident.model;
        snapshot.action = resident.action;
        snapshot.phase = resident.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = match resident.kind {
            sim_core::level1_mission::DynamicResidentKind::Archer => 75,
            sim_core::level1_mission::DynamicResidentKind::Builder => 101,
            sim_core::level1_mission::DynamicResidentKind::Villager
            | sim_core::level1_mission::DynamicResidentKind::Trader => 100,
        };
        snapshot.x = resident.position.x;
        snapshot.y = resident.position.y;
        snapshot.z = resident.position.z;
        snapshot.yaw = resident.yaw;
        snapshot.pitch = 0;
        snapshot.actual_speed = resident.actual_speed;
    }
    if let Some(ambient) = mission
        .ambient_entities()
        .find(|ambient| ambient.source_entity_index == entity.index)
    {
        snapshot.authority = MC_AUTHORITY_LEVEL1_MISSION_LIVE;
        snapshot.active = u8::from(ambient.active);
        snapshot.class_id = ambient.class;
        snapshot.model = ambient.model;
        snapshot.action = ambient.action;
        snapshot.phase = ambient.phase;
        snapshot.has_parameter_record = 1;
        snapshot.parameter_record_index = LEVEL1_NEW_EVENT_ENTITY_PARAMETER_RECORD_INDEX;
        snapshot.x = ambient.position.x;
        snapshot.y = ambient.position.y;
        snapshot.z = ambient.position.z;
        snapshot.yaw = ambient.yaw;
        snapshot.pitch = ambient.pitch;
        snapshot.actual_speed = ambient.actual_speed;
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

fn campaign_goat_snapshot(goat: &GoatState) -> McCampaignGoatSnapshotV2 {
    McCampaignGoatSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignGoatSnapshotV2>(),
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
        partner_entity_index: goat.partner_entity_index.unwrap_or(MC_ENTITY_INDEX_NONE),
        reserved_1: 0,
        reserved_2: [0; 3],
    }
}

fn campaign_spell_pickup_snapshot(
    pickup: &sim_core::native_world::Level2SpellPickupSource,
) -> McCampaignSpellPickupSnapshotV2 {
    McCampaignSpellPickupSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignSpellPickupSnapshotV2>(),
        source_entity_index: pickup.source_entity_index,
        authored_index: pickup.authored_index,
        family: pickup.family,
        action: pickup.action,
        phase: pickup.phase,
        active: u8::from(pickup.active),
        x: pickup.position.x,
        y: pickup.position.y,
        z: pickup.position.z,
        entity_rng: pickup.entity_rng,
        reserved_0: 0,
        reserved: [0; 9],
    }
}

fn level1_mission_snapshot(session: &SessionRecord) -> McLevel1MissionSnapshotV2 {
    let mission = &session.mission;
    let possessed_buildings = || {
        mission
            .possessed_buildings()
            .iter()
            .filter(|building| building.active && building.owner_entity_index != 0)
    };
    McLevel1MissionSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MissionSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        current_stage: mission.current_stage,
        text_selector: mission.current_text_selector,
        objective_textbox_ticks: mission.objective_textbox_ticks,
        tasks_completed: u8::from(mission.level_completed),
        player_dead: u8::from(mission.player_dead()),
        has_fireball: u8::from(mission.has_fireball),
        has_possession: u8::from(mission.has_possession),
        has_castle_spell: u8::from(mission.has_castle_spell),
        has_speed_spell: u8::from(mission.has_speed_spell),
        castle_exists: u8::from(mission.castle_exists),
        castle_level: mission.castle_level,
        fidelity_gap_flags: MC_LEVEL1_CURRENT_FIDELITY_GAPS,
        exit_sequence_active: u8::from(mission.exit_sequence_active),
        exit_sequence_complete: u8::from(mission.exit_sequence_complete),
        exit_result_code: mission.exit_result_code,
        player_life: mission.player_life,
        player_max_life: mission.player_max_life,
        player_mana: mission.player_mana,
        player_max_mana: mission.player_max_mana,
        castle_mana: mission.castle_mana,
        secured_mana_percent: mission.secured_mana_percent(),
        actor_count: u32::try_from(mission.actors().len()).expect("Level 1 actor count fits u32"),
        projectile_count: u32::try_from(mission.projectiles().len())
            .expect("Level 1 projectile count fits u32"),
        impact_count: u32::try_from(mission.impacts().len())
            .expect("Level 1 impact count fits u32"),
        exit_sequence_phase: u32::from(mission.exit_sequence_phase),
        exit_sequence_timer: i32::from(mission.exit_sequence_timer),
        possession_experience: mission.possession_experience,
        last_possessed_building_entity: possessed_buildings()
            .last()
            .map_or(0, |building| building.entity_index),
        last_possessed_building_owner: possessed_buildings()
            .last()
            .map_or(0, |building| building.owner_entity_index),
        possessed_building_count: u16::try_from(possessed_buildings().count())
            .expect("Level 1 possessed building count fits u16"),
        possession_tier: u8::try_from(mission.possession_tier)
            .expect("Level 1 possession tier is non-negative"),
        speed_trail_count: u8::try_from(mission.speed_trails().len()).unwrap_or(u8::MAX),
    }
}

fn level1_player_lifecycle_snapshot(session: &SessionRecord) -> McLevel1PlayerLifecycleSnapshotV2 {
    let mission = &session.mission;
    let marker = mission.player_death_marker();
    let player_entity_index = session.state.entity_index;
    let player_control_flags = mission.player_source_control_flags(player_entity_index);
    McLevel1PlayerLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1PlayerLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        lifecycle_hash: mission.player_lifecycle_hash_v2(&session.state, &session.context),
        source_arena_hash: mission.source_arena_hash_v1(),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.player_action,
        player_dead: u8::from(mission.player_dead()),
        input_suppressed: u8::from(
            mission.player_action != sim_core::level1_mission::PLAYER_ACTION_ACTIVE,
        ),
        player_active: u8::from(mission.player_source_active(player_entity_index)),
        player_visible: u8::from(player_control_flags & 0x20 == 0),
        player_targetable: u8::from(
            mission.player_source_active(player_entity_index) && mission.player_life >= 0,
        ),
        menu_close_requested: u8::from(mission.player_menu_close_requested),
        notification_active: u8::from(mission.player_notification_active),
        notification_state: mission.player_notification_state,
        banished_camera_active: u8::from(mission.player_banished_camera_active),
        marker_present: u8::from(marker.is_some_and(|marker| marker.active)),
        fall_velocity: mission.player_fall_velocity,
        notification_timer: mission.player_notification_timer,
        action_timer: mission.player_action_timer,
        replaceable_cursor: mission.source_pool_replaceable_cursor(),
        killer_entity_index: mission.player_killer_entity_index(),
        killer_score_awards: mission.player_score_for(
            mission.player_killer_entity_index(),
            session.state.player_color_index,
        ),
        notification_id: mission.player_notification_id,
        player_entity_rng: mission.player_source_entity_rng(player_entity_index),
        player_control_flags,
        player_entity_roll: mission.player_entity_roll,
        player_yaw: session.state.yaw,
        controller_pitch: session.state.pitch_accumulator,
        controller_roll: session.state.roll_accumulator,
        recoil_speed: session.state.recoil_speed,
        marker_source_entity_index: marker.map_or(0, |marker| marker.source_entity_index),
        marker_x: marker.map_or(0, |marker| marker.position.x),
        marker_y: marker.map_or(0, |marker| marker.position.y),
        marker_z: marker.map_or(0, |marker| marker.position.z),
        marker_trigger_entity_index: marker.map_or(0, |marker| marker.trigger_player_entity_index),
        marker_particle_parameter_index: marker.map_or(0, |marker| marker.particle_parameter_index),
        marker_life: marker.map_or(0, |marker| marker.life),
        marker_max_life: marker.map_or(0, |marker| marker.max_life),
        source_pool_rebuild_count: mission.source_pool_rebuild_count(),
        lifecycle_trace_hash: mission.player_lifecycle_trace_hash(),
        lifecycle_trace_count: u16::try_from(mission.player_lifecycle_trace().len())
            .unwrap_or(u16::MAX),
        marker_dispatch_phase: marker.map_or(0, |marker| marker.dispatch_phase),
        marker_class: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_CLASS),
        marker_model: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_MODEL),
        marker_action: marker.map_or(0, |_| sim_core::level1_mission::PLAYER_DEATH_MARKER_ACTION),
        marker_byte_38: marker.map_or(0, |marker| marker.byte_38),
        source_color_phase_6: u8::from(mission.source_color_phase_6()),
        source_turn_counter: mission.source_turn_counter(),
        menu_state: mission.player_menu_state,
        killer_score_victim_color: session.state.player_color_index.min(7),
        effective_pitch: session.state.effective_pitch,
        marker_constructor_phase: marker.map_or(0, |marker| marker.constructor_phase),
        reserved: [0; 4],
    }
}

fn level1_hud_spell_slot(session: &SessionRecord, holder_index: u16) -> McLevel1HudSpellSlotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1HudSpellSlotV2::default();
    };
    let Some(holder) = fire
        .holders
        .iter()
        .find(|holder| holder.entity_index == holder_index)
    else {
        return McLevel1HudSpellSlotV2::default();
    };
    let enabled_flags = if matches!(holder.model, 3 | 4 | 6 | 8 | 11 | 12 | 14) {
        4
    } else {
        0
    };
    McLevel1HudSpellSlotV2 {
        holder_entity_index: holder.entity_index,
        parent_entity_index: fire.caster.entity_index,
        model: holder.model,
        // DrawSpellIcon_2E260 reads the shared holder level gate, not the
        // selected tier stored independently for each hand.
        tier: holder.level_gate,
        enabled_flags,
        action_marker: holder.action_marker,
        active_value: holder.active_value,
        activation_value: holder.activation_value,
        max_mana: holder.mana_cost,
        // All four holders reachable in Level 1 have zero source manaRegen.
        mana_regen: 0,
        caster_mana: fire.caster.mana,
        frame_selector: if holder.active_value == 0 { 1 } else { 2 },
        icon_selector: 123 + u16::from(holder.model),
        reserved: [0; 2],
    }
}

fn level1_hud_snapshot(session: &SessionRecord) -> McLevel1HudSnapshotV2 {
    let mission = &session.mission;
    let fire = session.context.spell_fire.as_ref();
    let left_index = fire.map_or(0, |fire| fire.left_holder_index);
    let right_index = fire.map_or(0, |fire| fire.right_holder_index);
    let selected_index = fire.map_or(0, |fire| fire.selected_holder_index);
    let selected = level1_hud_spell_slot(session, selected_index);
    let player_color = usize::from(session.state.player_color_index.min(7));
    const NIGHT_COLORS: [[u8; 2]; 8] = [
        [0xA4, 0xAA],
        [0x77, 0x7D],
        [0xC0, 0xC6],
        [0x58, 0x5D],
        [0x97, 0x9D],
        [0xD7, 0xDD],
        [0x69, 0x62],
        [0xC9, 0xCF],
    ];
    McLevel1HudSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1HudSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        mana_color_index: NIGHT_COLORS[player_color][0],
        mana_max_color_index: NIGHT_COLORS[player_color][1],
        life_color_index: 0x7B,
        color_phase_1: u8::from((session.state.tick & 1) != 0),
        color_phase_2: u8::from(((session.state.tick / 2) & 1) != 0),
        player_hit_frame_ticks: mission.player_hit_frame_ticks(),
        castle_hit_frame_ticks: mission.castle_hit_frame_ticks(),
        global_mana_denominator: mission.global_mana_denominator,
        secured_mana_component: mission.secured_mana_component,
        goal_percent: mission.secured_mana_percent(),
        player_status_flags: session
            .context
            .spell_fire
            .as_ref()
            .map_or(0, |fire| fire.caster.control_flags),
        left: level1_hud_spell_slot(session, left_index),
        right: level1_hud_spell_slot(session, right_index),
        selected_holder_entity_index: selected.holder_entity_index,
        selected_icon_selector: selected.icon_selector,
        selected_model: selected.model,
        selected_tier: selected.tier,
        selected_enabled_flags: selected.enabled_flags,
        selected_frame_selector: u8::try_from(selected.frame_selector)
            .expect("spell frame selector is 1 or 2"),
    }
}

fn level1_spell_menu_snapshot(session: &SessionRecord) -> McLevel1SpellMenuSnapshotV2 {
    let Some(fire) = session.context.spell_fire.as_ref() else {
        return McLevel1SpellMenuSnapshotV2::default();
    };
    let holder_model = |holder_index| {
        fire.holders
            .iter()
            .find(|holder| holder.entity_index == holder_index)
            .map_or(0, |holder| holder.model)
    };
    let mut family_enabled = [0_u8; 4];
    let mut family_unlocked_tier = [0_i8; 4];
    let mut family_selected_tier = [0_i8; 4];
    let mut holder_entity_index = [0_u16; 4];
    for family in 0..4 {
        let holder_index = fire.enabled_holder_indices[family];
        holder_entity_index[family] = holder_index;
        family_enabled[family] = u8::from(holder_index != 0);
        family_unlocked_tier[family] = fire.spell_levels[family].clamp(0, 2);
        family_selected_tier[family] = fire.family_selected_tiers[family];
    }
    McLevel1SpellMenuSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1SpellMenuSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        selected_ui_index: fire.selected_spell_ui_index,
        selected_tier: fire.selected_subspell_index,
        left_family: holder_model(fire.left_holder_index),
        left_tier: fire.left_subspell_index,
        right_family: holder_model(fire.right_holder_index),
        right_tier: fire.right_subspell_index,
        reserved_0: 0,
        family_enabled,
        family_unlocked_tier,
        family_selected_tier,
        family_hand_eligibility: std::array::from_fn(|family| fire.spell_hand_eligibility[family]),
        holder_entity_index,
        reserved: [0; 2],
    }
}

fn level1_minimap_marks(session: &SessionRecord) -> McLevel1MinimapMarksV2 {
    let Level1MinimapMarks {
        mark_count,
        mark_pulse,
        arrow_valid,
        arrow_visible,
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
    } = session.mission.minimap_marks(&session.state);
    McLevel1MinimapMarksV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MinimapMarksV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mark_count,
        mark_pulse: u8::from(mark_pulse),
        arrow_valid: u8::from(arrow_valid),
        arrow_visible: u8::from(arrow_visible),
        mark_x,
        mark_y,
        mark_colour,
        mark_help_selector,
        arrow_xy,
        reserved: [0; 4],
    }
}

fn level1_exit_presentation(session: &SessionRecord) -> McLevel1ExitPresentationV2 {
    let mission = &session.mission;
    McLevel1ExitPresentationV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ExitPresentationV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        player_action: mission.exit_player_action,
        action_phase: mission.exit_sequence_phase,
        palette_submode: mission.exit_palette_submode,
        palette_effect: mission.exit_palette_effect,
        palette_weight: mission.exit_palette_weight,
        palette_denominator: mission.exit_palette_denominator,
        movement_resolution: mission.exit_last_movement_resolution,
        target_source_entity_index: mission.exit_target_source_entity_index,
        target_valid: u8::from(mission.exit_target_valid),
        input_suppressed: u8::from(mission.exit_sequence_owns_player_action()),
        camera_fov: session.camera_fov,
        action_timer: mission.exit_sequence_timer,
        player_status_flags: mission.exit_player_status_flags,
        blur_mode: mission.exit_blur_mode,
        display_blank: u8::from(mission.exit_display_blank),
        result_code: mission.exit_result_code,
        active: u8::from(mission.exit_sequence_active),
        complete: u8::from(mission.exit_sequence_complete),
        flags: MC_LEVEL1_EXIT_FLAG_PALETTE_VISUAL_CURRENT
            | MC_LEVEL1_EXIT_FLAG_FOV_POST_ACTION_NEXT_HISTORY
            | if mission.exit_end_sequence_flag {
                MC_LEVEL1_EXIT_FLAG_END_SEQUENCE
            } else {
                0
            },
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn record_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn level1_minimap_entity_inputs(session: &SessionRecord) -> [Level1MinimapEntityInput; 1_000] {
    const SOURCE_EXTENT: usize = 1_000;
    let local_player = session.state.entity_index;
    let mut inputs = [Level1MinimapEntityInput::default(); SOURCE_EXTENT];
    let relation_is_player = |entity_index: u16| {
        session
            .world
            .entities()
            .get(usize::from(entity_index))
            .is_some_and(|entity| entity.active && entity.class == 3)
    };

    for source_slot in 1..SOURCE_EXTENT {
        let slot = session.mission.source_slots()[source_slot];
        let reset = &session.world.entities()[source_slot];
        let bytes = reset.record_bytes();
        let source_id = reset.source_id;
        let player_link = record_u16(bytes, 0x94);
        inputs[source_slot] = Level1MinimapEntityInput {
            source_slot: source_slot as u16,
            class: slot.class,
            model: slot.model,
            action: slot.action,
            phase: slot.phase,
            control_flags: slot.control_flags,
            source_id,
            source_id_is_player: relation_is_player(source_id),
            source_id_is_local_player: source_id == local_player,
            player_link,
            player_link_is_player: relation_is_player(player_link),
            player_link_is_local_player: player_link == local_player,
            parent_id: record_u16(bytes, 0x28),
            stage_var_2: bytes[0x49] as i8,
            word_32: record_u16(bytes, 0x32),
            position: reset.position,
            active: slot.active,
        };
    }

    let mut resolve = |source_slot: u16,
                       position: Position,
                       source_id: u16,
                       player_link: u16,
                       parent_id: u16,
                       stage_var_2: i8| {
        if source_slot == 0 || usize::from(source_slot) >= SOURCE_EXTENT {
            return;
        }
        let input = &mut inputs[usize::from(source_slot)];
        input.position = position;
        input.source_id = source_id;
        input.source_id_is_player = relation_is_player(source_id);
        input.source_id_is_local_player = source_id == local_player;
        input.player_link = player_link;
        input.player_link_is_player = relation_is_player(player_link);
        input.player_link_is_local_player = player_link == local_player;
        input.parent_id = parent_id;
        input.stage_var_2 = stage_var_2;
    };

    resolve(
        local_player,
        session.state.position,
        local_player,
        local_player,
        0,
        0,
    );
    for goat in session.live_world.goats() {
        resolve(
            goat.entity_index,
            goat.position,
            goat.source_id,
            0,
            0,
            goat.stage_var_2,
        );
    }
    for actor in session.mission.actors() {
        resolve(
            actor.source_entity_index,
            actor.position,
            actor.owner_id.cast_unsigned(),
            0,
            0,
            actor.stage_var_2,
        );
    }
    for segment in session.mission.multipart_segments() {
        resolve(
            segment.source_entity_index,
            segment.position,
            segment.owner_id.cast_unsigned(),
            0,
            segment.head_source_entity_index,
            0,
        );
    }
    for projectile in session.mission.projectiles() {
        match projectile {
            Level1Projectile::PlayerFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                0,
                0,
                0,
            ),
            Level1Projectile::ArcherArrow(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::FireflyFireball(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.target_entity_index,
                0,
                0,
            ),
            Level1Projectile::CastleBuild(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
            Level1Projectile::Possession(projectile) => resolve(
                projectile.source_entity_index,
                projectile.position,
                projectile.owner_entity_index,
                projectile.owner_entity_index,
                0,
                0,
            ),
        }
    }
    for impact in session.mission.impacts() {
        resolve(
            impact.source_entity_index,
            impact.position,
            impact.owner_entity_index,
            impact.external_target_entity_index,
            0,
            0,
        );
    }
    for burst in session.mission.death_bursts() {
        resolve(
            burst.source_entity_index,
            burst.position,
            burst.parent_entity_index,
            0,
            burst.parent_entity_index,
            0,
        );
    }
    for sphere in session.mission.mana_spheres() {
        resolve(
            sphere.source_entity_index,
            sphere.position,
            sphere.source_entity_index,
            sphere.owner_entity_index,
            0,
            0,
        );
    }
    if let Some(carrier) = session.mission.mana_carrier() {
        resolve(
            carrier.source_entity_index,
            carrier.position,
            carrier.owner_entity_index,
            carrier.owner_entity_index,
            0,
            0,
        );
    }
    if session.mission.castle_exists {
        resolve(
            session.mission.castle_source_entity_index,
            session.mission.castle_position,
            session.mission.castle_owner_entity_index,
            session.mission.castle_owner_entity_index,
            0,
            0,
        );
    }
    if let Some(construction) = session.mission.castle_construction() {
        if construction.active {
            resolve(
                construction.source_entity_index,
                construction.position,
                construction.parent_entity_index,
                construction.owner_entity_index,
                construction.parent_entity_index,
                0,
            );
        }
    }
    inputs
}

fn level1_minimap_entity_commands(session: &SessionRecord) -> Vec<McLevel1MinimapEntityCommandV2> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        for (order, (x, y)) in
            level1_minimap_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .enumerate()
        {
            commands.push(McLevel1MinimapEntityCommandV2 {
                source_slot: session.mission.castle_source_entity_index,
                x,
                y,
                kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                palette_index: 176,
                anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                order: u16::try_from(order).expect("castle rope command order fits u16"),
                ..McLevel1MinimapEntityCommandV2::default()
            });
        }
    }
    for input in inputs.into_iter().skip(1) {
        let Some(command) = level1_minimap_entity_command(&session.state, input) else {
            continue;
        };
        commands.push(McLevel1MinimapEntityCommandV2 {
            source_slot: command.source_slot,
            x: command.x,
            y: command.y,
            selector: command.selector,
            kind: command.kind,
            palette_index: command.palette_index,
            size: command.size,
            anchor: command.anchor,
            help_selector: command.help_selector,
            flags: 0x07,
            order: u16::try_from(commands.len()).expect("minimap command order fits u16"),
            reserved: [0; 2],
        });
    }
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 minimap command extent"
    );
    commands
}

fn level1_map_entity_commands(session: &SessionRecord) -> Vec<Level1MinimapEntityCommand> {
    let inputs = level1_minimap_entity_inputs(session);
    let mut commands = Vec::with_capacity(LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS);
    if session.mission.castle_exists {
        commands.extend(
            level1_map_castle_rope_pixels(&session.state, session.mission.castle_position)
                .into_iter()
                .map(|(x, y)| Level1MinimapEntityCommand {
                    source_slot: session.mission.castle_source_entity_index,
                    x,
                    y,
                    selector: 0,
                    kind: LEVEL1_MINIMAP_COMMAND_BLEND_PIXEL,
                    palette_index: 176,
                    size: 1,
                    anchor: sim_core::level1_mission::LEVEL1_MINIMAP_ANCHOR_TOP_LEFT,
                    help_selector: 0,
                }),
        );
    }
    commands.extend(
        inputs
            .into_iter()
            .skip(1)
            .filter_map(|input| level1_map_entity_command(&session.state, input)),
    );
    assert!(
        commands.len() <= LEVEL1_MINIMAP_MAX_ENTITY_COMMANDS,
        "Level 1 map entity command extent"
    );
    commands
}

fn level1_minimap_command_hash(commands: &[McLevel1MinimapEntityCommandV2]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut append = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    append(b"mc2-level1-minimap-entities-v1\0");
    append(&(commands.len() as u32).to_le_bytes());
    for command in commands {
        append(&command.source_slot.to_le_bytes());
        append(&command.x.to_le_bytes());
        append(&command.y.to_le_bytes());
        append(&command.selector.to_le_bytes());
        append(&[
            command.kind,
            command.palette_index,
            command.size,
            command.anchor,
            command.help_selector,
            command.flags,
        ]);
        append(&command.order.to_le_bytes());
    }
    hash
}

fn level1_castle_snapshot(session: &SessionRecord) -> McLevel1CastleSnapshotV2 {
    let mission = &session.mission;
    let carrier = mission.mana_carrier();
    McLevel1CastleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(mission.castle_exists),
        level: mission.castle_level,
        class_id: CASTLE_ROOT_CLASS,
        model: CASTLE_ROOT_MODEL,
        action: CASTLE_ROOT_ACTION,
        // TMAP 96 header `03 00` supplies the runtime mode.
        render_mode: 0,
        reserved_0: 0,
        owner_entity_index: carrier.map_or(0, |carrier| carrier.owner_entity_index),
        render_parameter_index: CASTLE_ROOT_RENDER_INDEX,
        x: mission.castle_position.x,
        y: mission.castle_position.y,
        z: mission.castle_position.z,
        base_tmap: CASTLE_ROOT_BASE_TMAP,
        logical_height: CASTLE_ROOT_LOGICAL_HEIGHT,
        reserved_1: 0,
        life: mission.castle_life,
        max_life: mission.castle_max_life,
        mana: mission.castle_mana,
        mana_capacity: mission.castle_mana_capacity,
        carrier_id: carrier.map_or(0, |carrier| carrier.id),
        reserved: [
            u32::from(mission.castle_source_entity_index),
            mission.castle_presentation_id,
            0,
        ],
    }
}

fn terrain_state(session: &SessionRecord) -> McTerrainStateV2 {
    McTerrainStateV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McTerrainStateV2>(),
        generation: session.generation,
        tick: session.state.tick,
        mutation_sequence: session.terrain.mutation_sequence(),
        terrain_hash: session.terrain.deterministic_hash(),
        width: 256,
        height: 256,
        plane_count: 6,
        hash_schema: 1,
        authority: MC_AUTHORITY_SESSION_TERRAIN_LIVE,
        reserved_0: 0,
        reserved: [0; 4],
    }
}

fn level1_castle_lifecycle_snapshot(session: &SessionRecord) -> McLevel1CastleLifecycleSnapshotV2 {
    let mission = &session.mission;
    let construction = mission.castle_construction();
    McLevel1CastleLifecycleSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1CastleLifecycleSnapshotV2>(),
        generation: session.generation,
        tick: session.state.tick,
        root_presentation_id: mission.castle_presentation_id,
        root_source_entity_index: mission.castle_source_entity_index,
        root_owner_entity_index: mission.castle_owner_entity_index,
        root_x: mission.castle_position.x,
        root_y: mission.castle_position.y,
        root_z: mission.castle_position.z,
        root_build_state: mission.castle_build_state,
        root_action: mission.castle_action,
        root_level: mission.castle_level,
        root_active: u8::from(mission.castle_exists),
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        root_life: mission.castle_life,
        construction_id: construction.map_or(0, |child| child.id),
        construction_source_entity_index: construction.map_or(0, |child| child.source_entity_index),
        construction_parent_entity_index: construction.map_or(0, |child| child.parent_entity_index),
        construction_owner_entity_index: construction.map_or(0, |child| child.owner_entity_index),
        construction_x: construction.map_or(0, |child| child.position.x),
        construction_y: construction.map_or(0, |child| child.position.y),
        construction_z: construction.map_or(0, |child| child.position.z),
        construction_timer: construction.map_or(0, |child| child.timer),
        construction_stage: construction.map_or(0, |child| child.stage),
        construction_present: u8::from(construction.is_some()),
        construction_active: u8::from(construction.is_some_and(|child| child.active)),
        construction_initialized: u8::from(construction.is_some_and(|child| child.initialized)),
        reserved_0: [0; 2],
        terrain_mutation_sequence: session.terrain.mutation_sequence(),
    }
}

fn level1_house_completion_effects(
    session: &SessionRecord,
) -> Vec<McLevel1HouseCompletionEffectV2> {
    let completion_effects = session
        .mission
        .house_construction_effects()
        .iter()
        .copied()
        .filter(|effect| {
            matches!(
                effect,
                HouseConstructionSourceEffect::ConstructionAllocated { .. }
                    | HouseConstructionSourceEffect::ConstructionAllocationFailed { .. }
            )
        })
        .collect::<Vec<_>>();
    let effect_count =
        u32::try_from(completion_effects.len()).expect("completion effect count fits u32");
    completion_effects
        .into_iter()
        .enumerate()
        .map(|(ordinal, effect)| {
            let (kind, house_entity_index, parent_entity_index, source_entity_index, selector) =
                match effect {
                    HouseConstructionSourceEffect::ConstructionAllocated {
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATED,
                        house_entity_index,
                        parent_entity_index,
                        source_entity_index,
                        building_index,
                    ),
                    HouseConstructionSourceEffect::ConstructionAllocationFailed {
                        house_entity_index,
                        parent_entity_index,
                        building_index,
                    } => (
                        MC_LEVEL1_HOUSE_COMPLETION_ALLOCATION_FAILED,
                        house_entity_index,
                        parent_entity_index,
                        0,
                        building_index,
                    ),
                    _ => unreachable!("filtered completion effect"),
                };
            let construction = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .house_completion_constructions()
                        .iter()
                        .find(|construction| {
                            construction.source_entity_index == source_entity_index
                        })
                })
                .flatten();
            let source = (source_entity_index != 0)
                .then(|| {
                    session
                        .mission
                        .source_slots()
                        .get(usize::from(source_entity_index))
                })
                .flatten();
            let parent_word_2e = session
                .mission
                .raw_house_construction_state(parent_entity_index)
                .map_or(0, |state| state.4);
            let parent_construction_selector = session
                .mission
                .raw_source_construction_selector(parent_entity_index)
                .unwrap_or_default();
            let house_building_index = session
                .live_world
                .houses()
                .iter()
                .find(|house| house.entity_index == house_entity_index)
                .map(|house| house.building_index)
                .or_else(|| {
                    session
                        .mission
                        .authored_houses()
                        .find(|house| house.entity_index == house_entity_index)
                        .map(|house| house.building_index)
                })
                .unwrap_or_default();
            McLevel1HouseCompletionEffectV2 {
                abi_version: MC_NATIVE_ABI_VERSION,
                struct_size: struct_size::<McLevel1HouseCompletionEffectV2>(),
                generation: session.generation,
                tick: session.state.tick,
                kind,
                house_entity_index,
                parent_entity_index,
                source_entity_index,
                source_id: construction.map_or(0, |value| value.owner_entity_index),
                class_id: source.map_or(0, |value| value.class),
                model: source.map_or(0, |value| value.model),
                action: source.map_or(0, |value| value.action),
                construction_selector: selector,
                house_building_index,
                constructor_latch: construction.map_or(0, |value| value.constructor_latch),
                active: u8::from(construction.is_some_and(|value| value.active)),
                phase: source.map_or(0, |value| value.phase),
                parent_word_2e,
                parent_construction_selector,
                x: construction.map_or(0, |value| value.position.x),
                y: construction.map_or(0, |value| value.position.y),
                z: construction.map_or(0, |value| value.position.z),
                collision_half_width: construction.map_or(0, |value| value.collision_half_width),
                collision_half_depth: construction.map_or(0, |value| value.collision_half_depth),
                yaw: construction.map_or(0, |value| value.yaw),
                fov: construction.map_or(0, |value| value.fov),
                effect_ordinal: u16::try_from(ordinal).expect("completion effect ordinal fits u16"),
                max_life: construction.map_or(0, |value| value.max_life),
                life: construction.map_or(0, |value| value.life),
                control_flags: construction.map_or(0, |value| value.control_flags),
                effect_count,
            }
        })
        .collect()
}

fn mana_sphere_snapshot(sphere: &ManaSphere) -> McManaSphereSnapshotV2 {
    let size = usize::from(mana_sphere_size_ordinal(sphere.mana));
    let render_base = if sphere.owner_entity_index == 0 {
        MANA_SPHERE_NEUTRAL_RENDER_BASE
    } else {
        MANA_SPHERE_PLAYER_ONE_RENDER_BASE
    };
    McManaSphereSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaSphereSnapshotV2>(),
        sphere_id: sphere.id,
        source_entity_index: sphere.source_entity_index,
        owner_entity_index: sphere.owner_entity_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(sphere.active && !sphere.pending_delete),
        class_id: MANA_SPHERE_CLASS,
        model: MANA_SPHERE_MODEL,
        action: MANA_SPHERE_ACTION,
        phase: sphere.phase,
        // TMAPs 58/67 both carry runtime mode 21.
        render_mode: 21,
        reserved_0: 0,
        x: sphere.position.x,
        y: sphere.position.y,
        z: sphere.position.z,
        yaw: sphere.yaw,
        velocity_x: sphere.velocity_x,
        velocity_y: sphere.velocity_y,
        vertical_velocity: sphere.vertical_velocity,
        render_parameter_index: render_base
            + u16::try_from(size).expect("eight mana sphere sizes fit u16"),
        base_tmap: if sphere.owner_entity_index == 0 {
            MANA_SPHERE_NEUTRAL_BASE_TMAP
        } else {
            MANA_SPHERE_PLAYER_ONE_BASE_TMAP
        },
        logical_height: u16::try_from((size + 1) * 50)
            .expect("mana sphere logical height fits u16"),
        mana: sphere.mana,
        assigned_carrier_id: sphere.assigned_carrier_id,
        // Additive Level 2 mailbox projection. The public struct remains the
        // sealed 64-byte record; reserved words carry the source attraction
        // relation and the two source radial words.
        reserved: [
            u32::from(sphere.attraction_source_entity_index),
            u32::from(sphere.attraction_step) | (u32::from(sphere.attraction_distance_high) << 16),
            0,
        ],
    }
}

fn mana_carrier_snapshot(session: &SessionRecord) -> McManaCarrierSnapshotV2 {
    let carrier = session
        .mission
        .mana_carrier()
        .copied()
        .unwrap_or(ManaCarrier {
            id: 0,
            source_entity_index: 0,
            entity_rng: 0,
            owner_entity_index: 0,
            position: Position::default(),
            yaw: 0,
            pitch: 0,
            speed: MANA_CARRIER_SPEED,
            life: 0,
            max_life: MANA_CARRIER_MAX_LIFE,
            max_mana: MANA_CARRIER_MAX_MANA,
            target_sphere_id: 0,
            carried_mana: 0,
            returning_to_castle: false,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            active: false,
        });
    mana_carrier_snapshot_value(&carrier, session.generation, session.state.tick)
}

fn mana_carrier_snapshot_value(
    carrier: &ManaCarrier,
    generation: u64,
    tick: u64,
) -> McManaCarrierSnapshotV2 {
    McManaCarrierSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McManaCarrierSnapshotV2>(),
        generation,
        tick,
        carrier_id: carrier.id,
        target_sphere_id: carrier.target_sphere_id,
        owner_entity_index: carrier.owner_entity_index,
        render_parameter_index: MANA_CARRIER_RENDER_INDEX,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(carrier.active),
        class_id: MANA_CARRIER_CLASS,
        model: MANA_CARRIER_MODEL,
        action: MANA_CARRIER_ACTION,
        returning_to_castle: u8::from(carrier.returning_to_castle),
        // TMAP 88 header `02 00` supplies the runtime mode. This family is
        // fixed-facing; it does not use Firefly's mode-17 yaw selection.
        render_mode: 0,
        reserved_0: 0,
        x: carrier.position.x,
        y: carrier.position.y,
        z: carrier.position.z,
        yaw: carrier.yaw,
        speed: carrier.speed,
        base_tmap: MANA_CARRIER_BASE_TMAP,
        logical_height: MANA_CARRIER_LOGICAL_HEIGHT,
        reserved_1: 0,
        carried_mana: carrier.carried_mana,
        // Carrier identity and every carrier/sphere link are source slots.
        // Keep the redundant reserved projection until the next ABI version
        // so existing desktop clients can enforce exact equality.
        reserved: [u32::from(carrier.source_entity_index), 0, 0],
    }
}

fn level1_actor_snapshot(actor: &Level1Actor) -> McLevel1ActorSnapshotV2 {
    McLevel1ActorSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1ActorSnapshotV2>(),
        authored_index: actor.authored_index,
        activation_group: actor.activation_group,
        kind: actor.kind as u8,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(actor.active),
        collected: u8::from(actor.collected),
        class_id: actor.class,
        model: actor.model,
        action: actor.action,
        render_mode: actor.render_mode.unwrap_or(0),
        x: actor.position.x,
        y: actor.position.y,
        z: actor.position.z,
        render_parameter_index: actor.render_parameter_index.unwrap_or(u16::MAX),
        max_life: actor.max_life,
        life: actor.life,
        pending_damage: actor.pending_damage,
        base_tmap: actor.base_tmap.unwrap_or(u16::MAX),
        logical_height: actor.logical_height.unwrap_or(0),
        yaw: actor.yaw,
        animation_frame: actor.animation_frame,
        visible: u8::from(actor.visible),
        state_value: actor.state_value,
        reserved: 0,
    }
}

fn level1_multipart_segment_snapshot(
    segment: &Level1MultipartSegment,
) -> McLevel1MultipartSegmentSnapshotV2 {
    McLevel1MultipartSegmentSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McLevel1MultipartSegmentSnapshotV2>(),
        source_entity_index: segment.source_entity_index,
        head_authored_index: segment.head_authored_index,
        head_source_entity_index: segment.head_source_entity_index,
        predecessor_source_entity_index: segment.predecessor_source_entity_index,
        successor_source_entity_index: segment.successor_source_entity_index,
        ordinal: segment.ordinal,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(segment.active),
        visible: u8::from(segment.visible),
        class_id: segment.class,
        model: segment.model,
        action: segment.action,
        render_mode: segment.render_mode,
        x: segment.position.x,
        y: segment.position.y,
        z: segment.position.z,
        yaw: segment.yaw,
        pitch: segment.pitch,
        phase: segment.phase,
        reserved_0: 0,
        owner_id: segment.owner_id,
        life: segment.life,
        pending_damage: segment.pending_damage,
        render_parameter_index: segment.render_parameter_index,
        base_tmap: segment.base_tmap,
        logical_height: segment.logical_height,
        collision_half_width: segment.collision_half_width,
        collision_half_height: segment.collision_half_height,
        trailing_distance: segment.trailing_distance,
        reserved: 0,
    }
}

fn fireball_projectile_snapshot(projectile: &Level1Projectile) -> McFireballProjectileSnapshotV2 {
    let (
        projectile_kind,
        id,
        source_entity_index,
        owner_entity_index,
        render_index,
        class_id,
        model,
        position,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        active,
    ) = match projectile {
        Level1Projectile::PlayerFireball(projectile) => (
            1,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::ArcherArrow(projectile) => (
            2,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            ARCHER_ARROW_RENDER_INDEX,
            ARCHER_ARROW_PROJECTILE_CLASS,
            ARCHER_ARROW_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            ARCHER_ARROW_BASE_TMAP,
            projectile.payload,
            ARCHER_ARROW_LOGICAL_HEIGHT,
            ARCHER_ARROW_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::FireflyFireball(projectile) => (
            3,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            FIREBALL_RENDER_INDEX,
            FIREBALL_PROJECTILE_CLASS,
            FIREBALL_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            FIREBALL_BASE_TMAP,
            projectile.payload,
            FIREBALL_LOGICAL_HEIGHT,
            FIREBALL_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::CastleBuild(projectile) => (
            4,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            CASTLE_PROJECTILE_RENDER_INDEX,
            CASTLE_PROJECTILE_CLASS,
            CASTLE_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            CASTLE_PROJECTILE_BASE_TMAP,
            0,
            CASTLE_PROJECTILE_LOGICAL_HEIGHT,
            CASTLE_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
        Level1Projectile::Possession(projectile) => (
            5,
            projectile.id,
            projectile.source_entity_index,
            projectile.owner_entity_index,
            POSSESSION_PROJECTILE_RENDER_INDEX,
            POSSESSION_PROJECTILE_CLASS,
            POSSESSION_PROJECTILE_MODEL,
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            projectile.speed,
            projectile.life_ticks,
            POSSESSION_PROJECTILE_BASE_TMAP,
            0,
            POSSESSION_PROJECTILE_LOGICAL_HEIGHT,
            POSSESSION_PROJECTILE_RENDER_MODE,
            projectile.active,
        ),
    };
    McFireballProjectileSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballProjectileSnapshotV2>(),
        projectile_id: id,
        owner_entity_index,
        render_index,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(active),
        class_id,
        model,
        x: position.x,
        y: position.y,
        z: position.z,
        yaw,
        pitch,
        speed,
        life_ticks,
        base_tmap,
        payload,
        logical_height,
        render_mode,
        animation_frame: 0,
        projectile_kind,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `projectile_id` remains a presentation-stable identifier.
        reserved: [u32::from(source_entity_index), 0],
    }
}

fn fireball_impact_snapshot(impact: &FireballImpact) -> McFireballImpactSnapshotV2 {
    let (base_tmap, render_parameter_index, render_mode, logical_height) =
        if impact.model == WATER_SPLASH_MODEL {
            (
                WATER_SPLASH_BASE_TMAP,
                WATER_SPLASH_RENDER_INDEX,
                WATER_SPLASH_RENDER_MODE,
                WATER_SPLASH_LOGICAL_HEIGHT,
            )
        } else if impact.model == GROUND_FIRE_MODEL {
            let (render_parameter_index, logical_height) = ground_fire_presentation(impact.phase);
            (
                GROUND_FIRE_BASE_TMAP,
                render_parameter_index,
                GROUND_FIRE_RENDER_MODE,
                logical_height,
            )
        } else if impact.model == POSSESSION_ATTRACTION_MODEL {
            (
                POSSESSION_ATTRACTION_BASE_TMAP,
                POSSESSION_ATTRACTION_RENDER_INDEX,
                POSSESSION_ATTRACTION_RENDER_MODE,
                POSSESSION_ATTRACTION_LOGICAL_HEIGHT,
            )
        } else if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            (
                POSSESSION_IMPACT_BASE_TMAP,
                POSSESSION_IMPACT_RENDER_INDEX,
                POSSESSION_IMPACT_RENDER_MODE,
                POSSESSION_IMPACT_LOGICAL_HEIGHT,
            )
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            let render_parameter_index = u16::from(impact.animation_frame);
            (
                CHECKPOINT_SMOKE_BASE_TMAP,
                render_parameter_index,
                CHECKPOINT_SMOKE_RENDER_MODE,
                render_parameter_index
                    .saturating_sub(CHECKPOINT_SMOKE_FIRST_RENDER_INDEX)
                    .saturating_add(1)
                    * 50,
            )
        } else {
            (
                FIREBALL_IMPACT_BASE_TMAP,
                FIREBALL_IMPACT_RENDER_INDEX,
                FIREBALL_IMPACT_RENDER_MODE,
                FIREBALL_IMPACT_LOGICAL_HEIGHT,
            )
        };
    McFireballImpactSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McFireballImpactSnapshotV2>(),
        impact_id: impact.id,
        source_projectile_id: impact.source_projectile_id,
        authority: MC_AUTHORITY_LEVEL1_MISSION_LIVE,
        active: u8::from(impact.active),
        damage_submitted: u8::from(impact.damage_submitted),
        class_id: if matches!(
            impact.model,
            POSSESSION_IMPACT_MODEL | POSSESSION_STRONG_IMPACT_MODEL
        ) {
            POSSESSION_IMPACT_CLASS
        } else if impact.model == CHECKPOINT_SMOKE_MODEL {
            CHECKPOINT_SMOKE_CLASS
        } else {
            FIREBALL_IMPACT_CLASS
        },
        model: impact.model,
        render_mode,
        animation_frame: i8::try_from(impact.animation_frame).unwrap_or(i8::MAX),
        reserved_0: 0,
        x: impact.position.x,
        y: impact.position.y,
        z: impact.position.z,
        life_ticks: impact.life_ticks,
        payload: impact.payload,
        base_tmap,
        render_parameter_index,
        logical_height,
        reserved_1: 0,
        // Reserved word 0 carries the authoritative remc2 source slot while
        // `impact_id` remains a presentation-stable identifier.
        reserved: [u32::from(impact.source_entity_index), 0],
    }
}

fn validate_mission_snapshot_cursor(
    session: &SessionRecord,
    generation: u64,
    expected_tick: u64,
) -> AbiResult<()> {
    if generation != session.generation {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission generation {generation} does not match {}",
                session.generation
            ),
        ));
    }
    if expected_tick != session.state.tick {
        return Err(AbiError::new(
            MC_STATUS_GENERATION_MISMATCH,
            format!(
                "Level 1 mission tick {expected_tick} does not match {}",
                session.state.tick
            ),
        ));
    }
    Ok(())
}

fn copy_mission_slice<T, U>(
    source: &[T],
    start_ordinal: u32,
    buffer: *mut U,
    capacity: u32,
    out_written: *mut u32,
    convert: impl Fn(&T) -> U,
    label: &str,
) -> AbiResult<()> {
    let start = usize::try_from(start_ordinal)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} start exceeds usize")))?;
    if start > source.len() {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("{label} start is beyond count"),
        ));
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| AbiError::new(MC_STATUS_BOUNDS, format!("{label} capacity exceeds usize")))?;
    let count = (source.len() - start).min(capacity);
    if count > 0 && buffer.is_null() {
        return Err(AbiError::new(
            MC_STATUS_NULL_POINTER,
            format!("{label} output buffer is null"),
        ));
    }
    for (offset, item) in source.iter().skip(start).take(count).enumerate() {
        // SAFETY: the caller supplied exact element stride and writable
        // capacity for `count` records.
        unsafe { buffer.add(offset).write_unaligned(convert(item)) };
    }
    write_output(
        out_written,
        u32::try_from(count).expect("mission slice count fits u32"),
    );
    Ok(())
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

fn campaign_objective_stage_snapshot(
    generation: u64,
    objective_slot: u8,
    stage_slot: u8,
    objective: sim_core::native_world::RuntimeObjective,
    stage: &sim_core::native_world::RuntimeStage,
    stage_state: u8,
) -> McCampaignObjectiveStageSnapshotV2 {
    let (relation_domain, relation_value) = match stage.relation {
        sim_core::native_world::RuntimeRelation::Scalar(value) => (
            0,
            u16::try_from(value).expect("objective scalar relation fits u16"),
        ),
        sim_core::native_world::RuntimeRelation::AuthoredFeature(index) => (1, index),
        sim_core::native_world::RuntimeRelation::LiveEntity(index) => (2, index),
        sim_core::native_world::RuntimeRelation::LiveEntityIndex(index) => (3, index),
        sim_core::native_world::RuntimeRelation::AxisScalar(value) => (
            0,
            u16::try_from(value).expect("objective axis relation fits u16"),
        ),
        sim_core::native_world::RuntimeRelation::Null => (0, 0),
    };
    McCampaignObjectiveStageSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignObjectiveStageSnapshotV2>(),
        generation,
        objective_slot,
        stage_slot,
        is_level_end: objective.is_level_end,
        done: objective.done,
        text_selector: objective.text_selector,
        stage_state,
        stage_kind: stage.kind,
        stage_flags: stage.flags,
        axis_x: stage.axis_x,
        axis_y: stage.axis_y,
        relation_domain,
        reserved_0: 0,
        relation_value,
        reserved: [0; 4],
    }
}

fn campaign_player_snapshot(
    player: &sim_core::native_world::PlayerSnapshot,
    active: bool,
) -> McCampaignPlayerSnapshotV2 {
    McCampaignPlayerSnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignPlayerSnapshotV2>(),
        slot: player.slot,
        is_ai: u8::from(player.is_ai),
        active: u8::from(active),
        reserved_0: 0,
        entity_index: player.entity_index,
        active_player_index: player.active_player_index,
        turn: player.turn,
        left_spell_slot_raw: player.left_spell_slot_raw,
        right_spell_slot_raw: player.right_spell_slot_raw,
        left_subspell_index: player.left_subspell_index,
        right_subspell_index: player.right_subspell_index,
        selected_spell_ui_index: player.selected_spell_ui_index,
        selected_subspell_index: player.selected_subspell_index,
        reserved_1: [0; 2],
        reserved: [0; 4],
    }
}

fn campaign_entity_snapshot(entity: &EntitySnapshot) -> McCampaignEntitySnapshotV2 {
    McCampaignEntitySnapshotV2 {
        abi_version: MC_NATIVE_ABI_VERSION,
        struct_size: struct_size::<McCampaignEntitySnapshotV2>(),
        entity_index: entity.index,
        source_id: entity.source_id,
        next_relation: entity.next_relation.unwrap_or(u16::MAX),
        parameter_relation: entity.parameter_relation.unwrap_or(u16::MAX),
        authority: match entity.authority {
            EntityAuthority::Inactive => MC_AUTHORITY_INACTIVE,
            EntityAuthority::FrozenObserved => MC_AUTHORITY_FROZEN_OBSERVED,
            EntityAuthority::PlayerFlightLive => MC_AUTHORITY_PLAYER_FLIGHT_LIVE,
            EntityAuthority::SpellActivationLive => MC_AUTHORITY_SPELL_ACTIVATION_LIVE,
            EntityAuthority::GoatLive => MC_AUTHORITY_GOAT_LIVE,
            EntityAuthority::VillagerLive => MC_AUTHORITY_VILLAGER_LIVE,
        },
        active: u8::from(entity.active),
        class_id: entity.class,
        model: entity.model,
        action: entity.action,
        phase: entity.phase,
        x: entity.position.x,
        y: entity.position.y,
        z: entity.position.z,
        yaw: entity.yaw,
        pitch: entity.pitch,
        fov: entity.fov,
        actual_speed: entity.actual_speed,
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
    load_terrain_frame_assets(&PathBuf::from(root)).map(Some)
}

fn load_optional_level2_day_frame_assets(
    explicit_root: Option<&str>,
) -> AbiResult<Option<DayIndexedFrameAssets>> {
    let root = explicit_root
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from));
    let Some(root) = root else {
        return Ok(None);
    };
    let material_root = if root.join("components/material/manifest.json").is_file() {
        root.join("components/material")
    } else {
        root
    };
    DayIndexedFrameAssets::load(material_root)
        .map(Some)
        .map_err(|error| {
            AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                format!("cannot load qualified Level 2 Day indexed-frame cache: {error}"),
            )
        })
}

fn load_optional_level2_day_music(
    explicit_root: Option<&str>,
) -> AbiResult<Option<CampaignDayMusicLifecycle>> {
    let root = explicit_root
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MC_ASSET_CACHE_PATH").map(PathBuf::from));
    let Some(root) = root else {
        return Ok(None);
    };
    let audio_root = if root.join("components/audio/manifest.json").is_file() {
        root.join("components/audio")
    } else {
        root
    };
    let _ = read_qualified_cache_file(
        &audio_root,
        "audio/descriptor.json",
        171_379,
        "53b04308fe28b218d122fb515c12e902259eab60c1b91bc031bd154ca8aa477a",
    )?;
    let _ = read_qualified_cache_file(
        &audio_root,
        "audio/music/C2GAME1.xmi",
        10_352,
        "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d",
    )?;
    Ok(Some(CampaignDayMusicLifecycle::start_day()))
}

fn load_terrain_frame_assets(root: &Path) -> AbiResult<TerrainFrameAssets> {
    let terrain_atlas = read_qualified_cache_file(
        root,
        "terrain/bl32n.atlas.indices.u8",
        LEVEL1_TERRAIN_ATLAS_BYTES,
        TERRAIN_ATLAS_SHA256,
    )?;
    let tablesn = read_qualified_cache_file(
        root,
        "palette/tablesn.u8",
        sim_core::mode26_raster::NIGHT_TABLES_BYTES,
        TABLESN_SHA256,
    )?;
    let sky =
        read_qualified_cache_file(root, "sky/skyn.indices.u8", LEVEL1_SKY_BYTES, SKYN_SHA256)?;
    let palette_rgb6 = read_qualified_cache_file(root, "palette/paln.rgb6", 768, PALN_RGB6_SHA256)?;
    let archer_tmap_bank = load_qualified_archer_tmap_bank(root)?;
    let firefly_tmap_bank = load_qualified_firefly_tmap_bank(root)?;
    let frozen_actor_tmap_bank = load_qualified_frozen_actor_tmap_bank(root)?;
    let combat_tmap_bank = load_qualified_combat_tmap_bank(root)?;
    let static_world_tmap_bank = load_qualified_static_world_tmap_bank(root)?;
    let economy_tmap_bank = load_qualified_economy_tmap_bank(root)?;
    let indexed_overlays = IndexedOverlayAssets::load(root)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error.to_string()))?;
    Ok(TerrainFrameAssets {
        terrain_atlas: terrain_atlas.into_boxed_slice(),
        tablesn: tablesn.into_boxed_slice(),
        sky: sky.into_boxed_slice(),
        palette_rgb6: palette_rgb6.into_boxed_slice(),
        archer_tmap_bank,
        firefly_tmap_bank,
        frozen_actor_tmap_bank,
        combat_tmap_bank,
        static_world_tmap_bank,
        economy_tmap_bank,
        indexed_overlays,
    })
}

fn load_qualified_frozen_actor_tmap_bank(root: &Path) -> AbiResult<FrozenActorTmapBank> {
    let dolmen = read_qualified_cache_file(
        root,
        "terrain/tmaps1/079.decoded.bin",
        DOLMEN_PAYLOAD_BYTES,
        DOLMEN_PAYLOAD_SHA256,
    )?;
    let payloads = VILLAGER_ANIMATED_SELECTORS
        .iter()
        .enumerate()
        .map(|(ordinal, selector)| {
            read_qualified_cache_file(
                root,
                &format!("terrain/tmaps1/{selector:03}.decoded.bin"),
                VILLAGER_PAYLOAD_BYTES[ordinal],
                VILLAGER_PAYLOAD_SHA256[ordinal],
            )
        })
        .collect::<AbiResult<Vec<_>>>()?;
    FrozenActorTmapBank::decode_qualified(
        &dolmen,
        std::array::from_fn(|ordinal| payloads[ordinal].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_economy_tmap_bank(root: &Path) -> AbiResult<EconomyTmapBank> {
    let neutral_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/058.decoded.bin",
        MANA_SPHERE_NEUTRAL_PAYLOAD_BYTES,
        MANA_SPHERE_NEUTRAL_PAYLOAD_SHA256,
    )?;
    let player_one_mana = read_qualified_cache_file(
        root,
        "terrain/tmaps1/067.decoded.bin",
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_BYTES,
        MANA_SPHERE_PLAYER_ONE_PAYLOAD_SHA256,
    )?;
    let carrier = read_qualified_cache_file(
        root,
        "terrain/tmaps1/088.decoded.bin",
        MANA_CARRIER_PAYLOAD_BYTES,
        MANA_CARRIER_PAYLOAD_SHA256,
    )?;
    let castle = read_qualified_cache_file(
        root,
        "terrain/tmaps1/096.decoded.bin",
        CASTLE_ROOT_PAYLOAD_BYTES,
        CASTLE_ROOT_PAYLOAD_SHA256,
    )?;
    EconomyTmapBank::decode_qualified(&neutral_mana, &player_one_mana, &carrier, &castle)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_static_world_tmap_bank(root: &Path) -> AbiResult<StaticWorldTmapBank> {
    let spell_jar = read_qualified_cache_file(
        root,
        "terrain/tmaps1/059.decoded.bin",
        SPELL_JAR_PAYLOAD_BYTES,
        SPELL_JAR_PAYLOAD_SHA256,
    )?;
    let exit_target = read_qualified_cache_file(
        root,
        "terrain/tmaps1/461.decoded.bin",
        EXIT_TARGET_PAYLOAD_BYTES,
        EXIT_TARGET_PAYLOAD_SHA256,
    )?;
    let secondary = [30_u16, 31, 32, 33, 34, 35, 36, 37, 66, 259]
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let scenery = LEVEL1_SCENERY_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    let ambient = LEVEL1_AMBIENT_SELECTORS
        .map(|selector| {
            let path = root.join(format!("terrain/tmaps1/{selector:03}.decoded.bin"));
            fs::read(&path).map_err(|error| {
                AbiError::new(
                    MC_STATUS_IO_ERROR,
                    format!("failed reading {}: {error}", path.display()),
                )
            })
        })
        .into_iter()
        .collect::<AbiResult<Vec<_>>>()?;
    StaticWorldTmapBank::decode_with_level1_secondary_scenery_and_ambient(
        &spell_jar,
        &exit_target,
        std::array::from_fn(|index| secondary[index].as_slice()),
        std::array::from_fn(|index| scenery[index].as_slice()),
        std::array::from_fn(|index| ambient[index].as_slice()),
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_combat_tmap_bank(root: &Path) -> AbiResult<CombatTmapBank> {
    let fireball = read_qualified_cache_file(
        root,
        "terrain/tmaps1/463.decoded.bin",
        4_346,
        FIREBALL_PAYLOAD_SHA256,
    )?;
    let possession = read_qualified_cache_file(
        root,
        "terrain/tmaps1/144.decoded.bin",
        7_186,
        POSSESSION_PAYLOAD_SHA256,
    )?;
    let mut static_pixels = Vec::with_capacity(COMBAT_STATIC_SELECTORS.len());
    for ordinal in 0..COMBAT_STATIC_SELECTORS.len() {
        let selector = COMBAT_STATIC_SELECTORS[ordinal];
        let (width, height) = COMBAT_STATIC_EXTENTS[ordinal];
        static_pixels.push(read_qualified_cache_file(
            root,
            &format!("terrain/tmaps1/{selector:03}.indices.u8"),
            usize::from(width) * usize::from(height),
            COMBAT_STATIC_SHA256[ordinal],
        )?);
    }
    let references: Vec<&[u8]> = static_pixels.iter().map(Vec::as_slice).collect();
    let references: [&[u8]; COMBAT_STATIC_SELECTORS.len()] = references
        .try_into()
        .map_err(|_| AbiError::new(MC_STATUS_VALIDATION_FAILED, "combat TMAP count changed"))?;
    CombatTmapBank::decode_qualified(&fireball, &possession, references)
        .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
}

fn load_qualified_archer_tmap_bank(root: &Path) -> AbiResult<ArcherTmapBank> {
    let archer_animated = [
        (
            "terrain/tmaps1/008.decoded.bin",
            6_300,
            "084b65352a3123fa3d5dd8fb344ba56bb4e78f387b1026dd3cdfe590b56c0691",
        ),
        (
            "terrain/tmaps1/009.decoded.bin",
            6_958,
            "96747ee124997f694b384bdd1acf7d896f609ef5c5ef6f1eca9fa9875c57c4f1",
        ),
        (
            "terrain/tmaps1/010.decoded.bin",
            6_048,
            "f07373bd03faec45604b1ad16ae016c005cec11584d39fa76a4769e04c940cf7",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_static = [
        (
            "terrain/tmaps1/011.indices.u8",
            1_938,
            "e41cac6d975a7cd02e50a1f63aba2ab51fbf54587f2ddb5af89e9d4257385a1a",
        ),
        (
            "terrain/tmaps1/012.indices.u8",
            2_090,
            "82ad42cd719d21c2422632a44361b6c918a805efbd4bca8bffc998099048f77a",
        ),
        (
            "terrain/tmaps1/013.indices.u8",
            1_760,
            "d81fe313bcc95ea110fee6bfd4e0716a8eef91737d964be3dee173292f87b44e",
        ),
        (
            "terrain/tmaps1/121.indices.u8",
            1_440,
            "da5633de0e9f4a0591c5f11a6648bb0f6c5caf1b586ed1b13b11d00480c35501",
        ),
        (
            "terrain/tmaps1/122.indices.u8",
            1_710,
            "7b0a2db4ae60b748857a356deed89a6698950371ad564fc5b51ff1985da58fc9",
        ),
        (
            "terrain/tmaps1/123.indices.u8",
            1_440,
            "ce6eb45e6e8d07c45de984c1cb78d574a5ae2f4357df0318f3f3ce9bf37bd3df",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(&root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let archer_tmap_bank = ArcherTmapBank::decode_qualified(
        [
            &archer_animated[0],
            &archer_animated[1],
            &archer_animated[2],
        ],
        [
            &archer_static[0],
            &archer_static[1],
            &archer_static[2],
            &archer_static[3],
            &archer_static[4],
            &archer_static[5],
        ],
    )
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))?;
    Ok(archer_tmap_bank)
}

fn load_qualified_firefly_tmap_bank(root: &Path) -> AbiResult<FireflyTmapBank> {
    let payloads = [
        (
            "terrain/tmaps1/279.decoded.bin",
            8_378,
            "d4ffec8ae18edb62f68441ecd29cf375b6873af3cf98a780bc3539f11b5b1387",
        ),
        (
            "terrain/tmaps1/280.decoded.bin",
            9_828,
            "c334d13b8adf0d274452643760d58a2bcb5527241cd5d8a9ed0ae3550a77b7c9",
        ),
        (
            "terrain/tmaps1/281.decoded.bin",
            10_290,
            "fbfb64916d1d8e7b3c460b0235d47f227603e434d8b6bed808d2d4079986a9e7",
        ),
        (
            "terrain/tmaps1/282.decoded.bin",
            10_292,
            "f91014fba453529488ca0e49f3643622eb8485c1e2d45e631a8bfd1d18d0c720",
        ),
        (
            "terrain/tmaps1/283.decoded.bin",
            10_902,
            "3566b01e55bc04862eb40cfd20150e7eb03cf04f842d37078a245521d225df8d",
        ),
        (
            "terrain/tmaps1/284.decoded.bin",
            11_018,
            "8f75ffc75929f75364445a32c59e04a9b335e3f4d9d96388a106939987f0b4d0",
        ),
        (
            "terrain/tmaps1/285.decoded.bin",
            11_344,
            "ae00740ed6c6d188d369aad1ef95d062ec3d301ecdf7f0413d8b24c9086c367b",
        ),
        (
            "terrain/tmaps1/286.decoded.bin",
            11_294,
            "8095182d4842a4b04f2f02e3937225599bcd2fc50d73ef0ea1170024d5b6b283",
        ),
    ]
    .map(|(path, bytes, hash)| read_qualified_cache_file(root, path, bytes, hash))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    FireflyTmapBank::decode_qualified([
        &payloads[0],
        &payloads[1],
        &payloads[2],
        &payloads[3],
        &payloads[4],
        &payloads[5],
        &payloads[6],
        &payloads[7],
    ])
    .map_err(|error| AbiError::new(MC_STATUS_VALIDATION_FAILED, error))
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
#[path = "native_v2/tests_split/mod.rs"]
mod tests;
