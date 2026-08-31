//! Compatibility facade for the package-neutral outdoor terrain reflection owner.

pub use sim_terrain_reflection::*;

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::{
        native_world::NativeWorld,
        terrain_grid::{OutdoorGridInput, outdoor_grid},
    };
    use std::path::PathBuf;

    #[test]
    fn preserved_level1_rows_pin_owner_heights_materials_and_inverse_altitudes() {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private.join("generated/level1-export-m"),
            private.join("generated/level1-asset-closure.json"),
        )
        .expect("validated Level 1 owner package");
        let planes = world.terrain_planes();
        let grid = outdoor_grid(OutdoorGridInput {
            render_x: 19_841,
            render_y: 56_958,
            yaw: 0,
            turn: 8,
            shading: &planes.shading,
            angles: &planes.angles,
        });
        let projection = OutdoorProjection {
            scale: 164,
            pitch_offset: 0,
            roll: 0,
            center_x: 160,
            center_y: 100,
            viewport_width: 320,
            viewport_height: 200,
        };

        for (
            owner,
            tile,
            selector,
            height,
            shade,
            angle,
            wave,
            primary_altitude,
            inverse_altitude,
            uv,
            primary_selector,
        ) in [
            (
                94,
                (72, 221),
                46,
                149,
                34,
                20,
                -24_832,
                -384,
                -9_694,
                4,
                MODE26_REFLECTIVE_TERRAIN_SELECTOR,
            ),
            (
                135,
                (73, 220),
                0,
                139,
                41,
                64,
                0,
                -704,
                -9_600,
                16,
                MODE26_REFLECTIVE_TERRAIN_SELECTOR,
            ),
            (
                138,
                (76, 220),
                1,
                143,
                58,
                49,
                0,
                -576,
                -9_728,
                12,
                MODE5_TERRAIN_SELECTOR,
            ),
        ] {
            let source = grid[owner];
            assert_eq!((source.tile_x, source.tile_y), tile);
            assert_eq!(
                (source.shade, source.angle, source.raw_wave),
                (shade, angle, wave)
            );
            let map_index = usize::from(source.tile_x) + usize::from(source.tile_y) * 256;
            assert_eq!(planes.ground_heights[map_index], height);
            let (texture_index, texture_uv_row) = reflection_texture_cell(
                source.tile_x,
                source.tile_y,
                0,
                &planes.terrain_types,
                &planes.angles,
            );
            assert_eq!((texture_index, texture_uv_row), (selector, uv));
            let projected = project_outdoor_reflection_vertex(
                source,
                height,
                5_152,
                texture_index,
                texture_uv_row,
                false,
                projection,
            )
            .expect("selected preserved row is eligible");
            assert_eq!(
                (projected.primary_altitude, projected.inverse_altitude),
                (primary_altitude, inverse_altitude)
            );
            let mut submission_vertex = projected;
            submission_vertex.feature_flags &=
                TERRAIN_FEATURE_REFLECTIVE_MATERIAL | TERRAIN_FEATURE_FLAT_SHADE | 1;
            let quad =
                submit_outdoor_quad(800, REFLECTION_EYE_Z_LIMIT, [Some(submission_vertex); 4]);
            assert!(quad.reflected.is_empty());
            assert_eq!(quad.primary[0].raster_selector, primary_selector);
        }
    }
}
