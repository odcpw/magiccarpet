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
    let sky = std::fs::read(private.join("generated/level1-unity-asset-cache/sky/skyn.indices.u8"))
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
    let terrain =
        render_level1_terrain_frame(camera, render_resources()).expect("isolated terrain frame");
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
    let dolmen_payload =
        std::fs::read(cache.join("terrain/tmaps1/079.decoded.bin")).expect("qualified dolmen TMAP");
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
    let frozen_seeds = frozen_actors.map(|actor| crate::renderable_map_chain::RenderableMapSeed {
        entity_index: actor.source_entity_index,
        active: true,
        x: actor.position.x,
        y: actor.position.y,
    });
    let frozen_map_chains =
        RenderableMapChains::from_reset(&frozen_seeds, &planes.entity_indices, |entity_index| {
            world
                .entities()
                .get(usize::from(entity_index))
                .map(crate::native_world::EntitySnapshot::old_map_entity)
        })
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
        max_life: 300,
        life: 300,
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
    let patterned_source_frame = |selector: u16, width: u16, height: u16| SourceSpriteTmapFrame {
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

    let render_source_vector = |camera: Level1TerrainFrameCamera,
                                submission: SourceSpriteSubmission| {
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
            .spawn_death_burst_from_death(production_player.entity_index, offset_position(-210, 0))
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
