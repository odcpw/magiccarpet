//! Package-backed, handle-based ABI v2 for native presentation clients.
//!
//! All paths and data are supplied explicitly by the caller. This module does
//! not use the embedded owner regression bytes retained by the legacy v1 ABI.

mod audio_voice_pool;
mod campaign_abi;
mod campaign_audio;
mod campaign_audio_state;
mod campaign_facade;
mod campaign_minimap_labels;
mod events_audio_abi;
mod game_runtime_abi;
mod game_session_abi;
mod level1_abi;
mod session_record;
mod shared_types;
mod validation;
use campaign_abi::*;
pub(crate) use events_audio_abi::*;
pub(crate) use level1_abi::*;
use session_record::*;
pub use shared_types::*;
use validation::*;

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
    campaign_effect_snapshot,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
use sim_core::native_world::resolve_level1_presentation_cache_root;
use sim_core::{
    FlightContext, FlightEvent, FlightState, FlightTuning, FlightWorld,
    POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING, PlayerCommand, Position, PositionalSoundCommand,
    StepEvidence,
    archer_tmap::{ArcherTmapBank, ArcherTmapClocks},
    campaign::{
        CampaignSession as DescriptorCampaignSession,
        journal::{EffectCursor, EffectPageError},
        spec::CampaignSelection,
    },
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
        GOAT_BASE_TMAP, GOAT_CLASS, GOAT_MODEL, GOAT_PATROL_ACTION, GoatBehaviorConfig,
        GoatPlayerTarget, GoatState, GoatStepEvent, LEVEL_ONE_GOAT_INDICES, LEVEL_TWO_GOAT_INDICES,
        LiveWorldState,
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
        Level2EconomyEvent, Mc2DispatchEvent, Mc2Simulation, NativeWorld, NativeWorldInspection,
        ParticleParameter, SessionTerrain, TerrainPlanes, apply_level1_source_effects,
        level1_turn_start_source_views, refresh_level1_current_source_view,
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
    step_entity_body_with_context, step_outer_turn_with_context_and_tuning,
    terrain_frame::{
        LEVEL1_SKY_BYTES, LEVEL1_TERRAIN_ATLAS_BYTES, LEVEL1_VIEW_HEIGHT, LEVEL1_VIEW_PIXELS,
        LEVEL1_VIEW_WIDTH, Level1CastleRootRender, Level1FrozenActorRender, Level1TerrainFrame,
        Level1TerrainFrameCamera, Level1TerrainFrameResources, Level1WorldFrameResources,
        raster_level1_source_world_sprites, render_level1_source_world_frame,
        render_level1_source_world_map_frame_composed, render_level1_terrain_frame,
    },
    terrain_grid::{OUTDOOR_GRID_VERTICES, OutdoorGridInput, outdoor_grid},
    terrain_presentation::outdoor_terrain_pnt5,
    terrain_reflection::OutdoorProjection,
    villager::{VILLAGER_CLASS, VILLAGER_MODEL, VillagerState},
};

use crate::presentation_bridge::{
    LIVE_MENU_AUTH_ALL, LIVE_MENU_AUTH_DEBUG_METRICS, Level1PresentationLive, Level2MinimapEntity,
    Level2MinimapMark, Level2MinimapPlan, LiveDebugEntity, LiveHud, LiveMenuState, LiveSpellMenu,
    LiveSpellSlot, PresentationBridgeQuery, build_level1_final_main, campaign_minimap_plan,
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
struct SessionDefinitionTuning {
    flight: FlightTuning,
    fireball_mana_cost: i32,
    goat_config: GoatBehaviorConfig,
}

impl Default for SessionDefinitionTuning {
    fn default() -> Self {
        Self {
            flight: FlightTuning::CLASSIC,
            fireball_mana_cost: 100,
            goat_config: GoatBehaviorConfig::LEVEL_ONE,
        }
    }
}

#[derive(Debug)]
struct SessionRecord {
    world: Arc<NativeWorld>,
    terrain: SessionTerrain,
    state: FlightState,
    definition_tuning: SessionDefinitionTuning,
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

#[cfg(test)]
#[path = "native_v2/tests_split/mod.rs"]
mod tests;
