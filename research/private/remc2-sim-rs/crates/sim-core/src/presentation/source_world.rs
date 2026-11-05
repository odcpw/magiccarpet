//! Source-ordered MC2 world sprites, shared by Night, Day and Cave.
//!
//! Terrain calls this owner after each cell. Reflected sprites therefore
//! exist before primary water consumes them; shadows consume the current
//! destination immediately before their own primary sprite.

use std::collections::BTreeMap;

use crate::{
    campaign::sprite_render::{source_sprite_pixel_metadata, source_sprite_pixel_mode},
    classic_hd_raster::ClassicHdRasterError,
    goat_sprite_raster::{
        Mode20SpriteCamera, Mode20SpriteDefinition, Mode20SpriteEntity, Mode20SpritePlacement,
        SourceSpritePass, project_fixed_sprite_with_depth_adjustment, project_sprite_shadow,
        rasterize_indexed_source_sprite, reproject_sprite_altitude,
    },
    legacy_terrain::interpolate_ground,
    native_world::{CampaignEntityPresentation, MapType, ParticleParameter},
    source_sprite_tmap::SourceSpriteTmapBank,
    terrain_frame::Level1TerrainFrameCamera,
    terrain_reflection::{
        OutdoorProjection, REFLECTION_EYE_Z_LIMIT, TerrainPass, terrain_material_blocks_shadow,
    },
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceWorldSpriteStats {
    pub primary_calls: u32,
    pub reflection_calls: u32,
    pub shadow_calls: u32,
    pub primary_writes: u64,
    pub reflection_writes: u64,
    pub shadow_writes: u64,
}

struct Sprite {
    source: CampaignEntityPresentation,
    placement: Option<Mode20SpritePlacement>,
    primary: Option<Mode20SpritePlacement>,
    primary_pixel_mode: u8,
    reflection_pixel_mode: u8,
    shadow: Option<Mode20SpritePlacement>,
}

pub(crate) struct SourceWorldSprites<'a> {
    camera: Mode20SpriteCamera,
    bank: &'a SourceSpriteTmapBank,
    buckets: BTreeMap<u16, Vec<Sprite>>,
    reflections: bool,
    previous_pixel_mode: u8,
    stats: SourceWorldSpriteStats,
    primary_visibility_completions: Vec<u16>,
}

impl<'a> SourceWorldSprites<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        camera: Level1TerrainFrameCamera,
        projection: OutdoorProjection,
        map_type: MapType,
        rows: Vec<CampaignEntityPresentation>,
        particles: &[ParticleParameter],
        bank: &'a SourceSpriteTmapBank,
        cell_orders: &[u16],
        ground_heights: &[u8],
        previous_pixel_mode: u8,
        shadows: bool,
    ) -> Result<Self, ClassicHdRasterError> {
        if cell_orders.len() != 65536 || ground_heights.len() != 65536 {
            return Err(ClassicHdRasterError(
                "source world map planes are incomplete".into(),
            ));
        }
        let sprite_camera = Mode20SpriteCamera {
            render_x: camera.render_x,
            render_y: camera.render_y,
            eye_z: camera.eye_z,
            yaw: camera.yaw,
            projection,
        };
        let mut buckets: BTreeMap<u16, Vec<Sprite>> = BTreeMap::new();
        for source in rows {
            let rank = cell_orders
                [usize::from(source.position.y >> 8) * 256 + usize::from(source.position.x >> 8)];
            if rank == 0 || source.draw_order == u32::MAX {
                continue;
            }
            let frame = bank.frame(source.selected_tmap).ok_or_else(|| {
                ClassicHdRasterError(format!(
                    "source sprite {} TMAP {} is absent",
                    source.entity_index, source.selected_tmap
                ))
            })?;
            let particle = particles
                .get(usize::from(source.render_parameter_index))
                .ok_or_else(|| ClassicHdRasterError("source sprite particle is absent".into()))?;
            // The authority already selected the live animation/orientation
            // frame. Project that immutable frame through the shared source
            // geometry, retaining each mode's distinct primary/shadow pivot.
            let project = |depth_adjustment| {
                project_fixed_sprite_with_depth_adjustment(
                    Mode20SpriteDefinition {
                        base_selector: source.selected_tmap,
                        logical_height: i32::from(source.logical_height),
                    },
                    matches!(source.render_mode, 11 | 36).then_some(0),
                    matches!(source.render_mode, 21 | 36),
                    sprite_camera,
                    Mode20SpriteEntity {
                        x: source.position.x,
                        y: source.position.y,
                        z: source.position.z,
                        yaw: source.yaw,
                    },
                    frame.width,
                    frame.height,
                    depth_adjustment,
                )
                .ok()
                .map(|mut placement| {
                    placement.mirrored = source.mirrored;
                    placement.orientation_sector = source.orientation_sector;
                    placement
                })
            };
            let placement = project(0);
            let primary = if source.primary_depth_adjustment == 0 {
                placement
            } else {
                project(source.primary_depth_adjustment)
            };
            if placement.is_none() && primary.is_none() {
                continue;
            }
            let pixel_mode = |placement: Option<Mode20SpritePlacement>| {
                placement.map_or(Ok(0), |placement| {
                    source_sprite_pixel_metadata(particle.byte_10, placement.distance_squared)
                        .map(|metadata| metadata.pixel_mode)
                        .map_err(|error| ClassicHdRasterError(error.into()))
                })
            };
            let primary_pixel_mode = pixel_mode(primary)?;
            let reflection_pixel_mode = pixel_mode(placement)?;
            let shadow = if shadows
                && particle.byte_10 == 0
                && (source.control_flags >> 16) & 0x0808 == 0
            {
                placement.and_then(|placement| {
                    project_sprite_shadow(
                        placement,
                        sprite_camera,
                        interpolate_ground(ground_heights, source.position.x, source.position.y),
                        map_type != MapType::Day,
                    )
                    .ok()
                })
            } else {
                None
            };
            buckets.entry(rank).or_default().push(Sprite {
                source,
                placement,
                primary,
                primary_pixel_mode,
                reflection_pixel_mode,
                shadow,
            });
        }
        for bucket in buckets.values_mut() {
            bucket.sort_unstable_by_key(|sprite| sprite.source.draw_order);
        }
        Ok(Self {
            camera: sprite_camera,
            bank,
            buckets,
            reflections: map_type != MapType::Cave
                && camera.reflections
                && camera.eye_z < REFLECTION_EYE_Z_LIMIT,
            previous_pixel_mode,
            stats: SourceWorldSpriteStats::default(),
            primary_visibility_completions: Vec::new(),
        })
    }

    pub(crate) fn reflections_enabled(&self) -> bool {
        self.reflections
    }
    pub(crate) fn stats(&self) -> SourceWorldSpriteStats {
        self.stats
    }
    pub(crate) fn last_pixel_mode(&self) -> u8 {
        self.previous_pixel_mode
    }

    /// Source DrawSprite's primary rotated branch sets subSpellIndex bit 6
    /// after the call for entities whose control byte 3 has bit 6. The result
    /// identifies source slots in this frame; the session owner binds their
    /// allocator generations before applying the feedback to live state.
    pub(crate) fn primary_visibility_completions(&self) -> &[u16] {
        &self.primary_visibility_completions
    }

    pub(crate) fn draw_cell(
        &mut self,
        pass: TerrainPass,
        rank: u16,
        texture: u8,
        width: usize,
        height: usize,
        indices: &mut [u8],
        tables: &[u8],
    ) -> Result<(), ClassicHdRasterError> {
        let Some(bucket) = self.buckets.get(&rank) else {
            return Ok(());
        };
        for sprite in bucket {
            let source = sprite.source;
            let frame = self
                .bank
                .frame(source.selected_tmap)
                .expect("source frame retained from validated planning");
            let mut draw = |placement, pixel_mode, kind| {
                rasterize_indexed_source_sprite(
                    indices,
                    width,
                    height,
                    placement,
                    frame.width,
                    frame.height,
                    &frame.pixels,
                    tables,
                    self.camera.projection.roll,
                    pixel_mode,
                    source.owner_color,
                    kind,
                )
                .map_err(ClassicHdRasterError)
            };
            if pass == TerrainPass::Reflected {
                if !self.reflections {
                    continue;
                }
                let Some(placement) = sprite.placement else {
                    continue;
                };
                let reflected = reproject_sprite_altitude(
                    placement,
                    self.camera,
                    -i32::from(source.position.z),
                );
                self.previous_pixel_mode = sprite.reflection_pixel_mode;
                let result = draw(
                    reflected,
                    sprite.reflection_pixel_mode,
                    SourceSpritePass::Reflection,
                )?;
                self.stats.reflection_calls += 1;
                self.stats.reflection_writes += u64::from(result.framebuffer_writes);
                continue;
            }
            if !terrain_material_blocks_shadow(texture) {
                if let Some(shadow) = sprite.shadow {
                    self.previous_pixel_mode = 8;
                    let result = draw(shadow, 8, SourceSpritePass::Shadow)?;
                    self.stats.shadow_calls += 1;
                    self.stats.shadow_writes += u64::from(result.framebuffer_writes);
                }
            }
            let Some(primary) = sprite.primary else {
                continue;
            };
            let mode = source_sprite_pixel_mode(
                sprite.primary_pixel_mode,
                source.control_flags,
                self.previous_pixel_mode,
            );
            self.previous_pixel_mode = mode;
            let result = draw(primary, mode, SourceSpritePass::Primary)?;
            self.stats.primary_calls += 1;
            self.stats.primary_writes += u64::from(result.framebuffer_writes);
            if !primary.quarter_sum_anchor && source.control_flags & 0x4000_0000 != 0 {
                // In the source this write follows the primary rotated draw
                // branch, including its internal clip exits. It is not
                // conditioned on a nontransparent framebuffer write.
                self.primary_visibility_completions.push(source.entity_index);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile},
        classic_hd_raster::{CLASSIC_HD_TERRAIN_ATLAS_BYTES, rasterize_classic_hd_mode26_triangle},
        mode5_raster::Mode5Vertex,
        source_sprite_tmap::SourceSpriteTmapFrame,
    };

    #[test]
    fn reflected_sprite_is_consumed_by_water_before_shadow_and_primary_calls() {
        let camera = Level1TerrainFrameCamera {
            render_x: 1000,
            render_y: 1000,
            eye_z: 0,
            yaw: 0,
            pitch: 0,
            roll: 0,
            fov: 128,
            source_turn: 0,
            reflections: true,
        };
        let row = CampaignEntityPresentation {
            entity_index: 1,
            source_id: 1,
            render_parameter_index: 0,
            base_tmap: 1,
            selected_tmap: 1,
            logical_height: 128,
            render_mode: 0,
            orientation_sector: 0,
            mirrored: false,
            position: crate::Position {
                x: 1000,
                y: 488,
                z: 64,
            },
            yaw: 0,
            pixel_mode: 0,
            owner_color: 123,
            brightness: 8192,
            draw_order: 0,
            control_flags: 0x4000_0000,
            primary_depth_adjustment: 0,
        };
        let particle = ParticleParameter {
            base_tmap: 1,
            word_2: 0,
            word_4: 0,
            speed: 0,
            rotation_speed: 128,
            byte_10: 0,
            byte_11: 0,
            render_mode: 0,
            byte_13: 0,
        };
        let bank = SourceSpriteTmapBank::new(vec![SourceSpriteTmapFrame {
            selector: 1,
            width: 2,
            height: 2,
            pixels: vec![2, 2, 3, 3].into_boxed_slice(),
        }])
        .unwrap();
        let mut cells = vec![0; 65536];
        cells[259] = 1;
        let ground = vec![0; 65536];
        let mut world = SourceWorldSprites::new(
            camera,
            crate::terrain_frame::level1_projection(camera),
            MapType::Night,
            vec![row],
            &[particle],
            &bank,
            &cells,
            &ground,
            0,
            true,
        )
        .unwrap();
        let mut indices = vec![77; 320 * 200];
        let mut tables = vec![0; 83456];
        for previous in 0..256 {
            tables[0x4000 + (previous << 8) + 5] = (previous as u8).wrapping_add(100);
            tables[0x1800 + previous] = previous as u8 / 2;
        }
        tables[5] = 5;
        world
            .draw_cell(
                TerrainPass::Reflected,
                1,
                0,
                320,
                200,
                &mut indices,
                &tables,
            )
            .unwrap();
        assert!(world.primary_visibility_completions().is_empty(),
            "a reflected draw cannot publish primary visibility");
        let reflection = indices[140 * 320 + 160];
        assert!(
            matches!(reflection, 2 | 3),
            "reflected source samples must already exist before water"
        );
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let mut provenance = vec![0; indices.len()];
        let mut writer =
            ClassicHdFrameWriter::from_planes(&profile, &mut indices, &mut provenance, 1, 0, 0, 0)
                .unwrap();
        let point = |x, y| Mode5Vertex {
            x,
            y,
            u: 0,
            v: 0,
            pnt5: 0,
        };
        let atlas = vec![5; CLASSIC_HD_TERRAIN_ATLAS_BYTES];
        for triangle in [
            [point(0, 0), point(320, 0), point(320, 200)],
            [point(0, 0), point(320, 200), point(0, 200)],
        ] {
            rasterize_classic_hd_mode26_triangle(&mut writer, triangle, &atlas, 0, &tables, 1)
                .unwrap();
        }
        writer.finish();
        assert_eq!(
            indices[140 * 320 + 160],
            reflection + 100,
            "mode26 must use the reflected sprite's index as its destination operand"
        );
        world
            .draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables)
            .unwrap();
        assert_eq!(indices[140 * 320 + 160], reflection + 100);
        assert_eq!(world.stats().reflection_calls, 1);
        assert_eq!(world.stats().shadow_calls, 1);
        assert_eq!(world.stats().primary_calls, 1);
        assert!(world.stats().shadow_writes > 0);
        assert_eq!(world.primary_visibility_completions(), &[1]);
    }

    #[test]
    fn primary_completion_follows_source_admission_even_when_internally_clipped() {
        let camera = Level1TerrainFrameCamera {
            render_x: 1000, render_y: 1000, eye_z: 0, yaw: 0, pitch: 0,
            roll: 0, fov: 128, source_turn: 0, reflections: true,
        };
        let row = CampaignEntityPresentation {
            entity_index: 31, source_id: 31, render_parameter_index: 0,
            base_tmap: 1, selected_tmap: 1, logical_height: 128, render_mode: 17,
            orientation_sector: 0, mirrored: false,
            position: crate::Position { x: 3000, y: 488, z: 64 },
            yaw: 0, pixel_mode: 0, owner_color: 123, brightness: 8192,
            draw_order: 0, control_flags: 0x4000_0000, primary_depth_adjustment: 0,
        };
        let particle = ParticleParameter {
            base_tmap: 1, word_2: 0, word_4: 0, speed: 0, rotation_speed: 128,
            byte_10: 0, byte_11: 0, render_mode: 17, byte_13: 0,
        };
        let bank = SourceSpriteTmapBank::new(vec![SourceSpriteTmapFrame {
            selector: 1, width: 2, height: 2, pixels: vec![2; 4].into_boxed_slice(),
        }]).unwrap();
        let ground = vec![0; 65536];
        let mut cells = vec![0; 65536];
        cells[usize::from(row.position.x >> 8) + 256 * usize::from(row.position.y >> 8)] = 1;
        let mut indices = vec![0; 320 * 200];
        let tables = vec![0; 83456];
        let particles = [particle];
        let create = |row, cells: &[u16]| SourceWorldSprites::new(
            camera, crate::terrain_frame::level1_projection(camera), MapType::Night,
            vec![row], &particles, &bank, cells, &ground, 0, true,
        ).unwrap();

        let mut world = create(row, &cells);
        world.draw_cell(TerrainPass::Reflected, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert!(world.primary_visibility_completions().is_empty());
        world.draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert_eq!(world.stats().primary_calls, 1);
        assert_eq!(world.stats().primary_writes, 0, "sprite lies outside the viewport");
        assert_eq!(world.primary_visibility_completions(), &[31],
            "source primary draw completion is not gated on framebuffer writes");

        let mut unvisited = create(row, &vec![0; 65536]);
        unvisited.draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert!(unvisited.primary_visibility_completions().is_empty());

        let mut unflagged = row;
        unflagged.control_flags = 0;
        let mut no_feedback = create(unflagged, &cells);
        no_feedback.draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert_eq!(no_feedback.stats().primary_calls, 1);
        assert!(no_feedback.primary_visibility_completions().is_empty());

        let mut behind = row;
        behind.position.y = 1128;
        let mut behind_cells = vec![0; 65536];
        behind_cells[usize::from(behind.position.x >> 8)
            + 256 * usize::from(behind.position.y >> 8)] = 1;
        let mut rejected = create(behind, &behind_cells);
        rejected.draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert!(rejected.primary_visibility_completions().is_empty(),
            "a near-plane rejected sprite never calls DrawSprite");

        let mut special = row;
        special.render_mode = 21;
        let mut special_world = create(special, &cells);
        special_world.draw_cell(TerrainPass::Primary, 1, 0, 320, 200, &mut indices, &tables).unwrap();
        assert_eq!(special_world.stats().primary_calls, 1);
        assert!(special_world.primary_visibility_completions().is_empty(),
            "the source special-pivot branch has no primary visibility write");
    }
}
