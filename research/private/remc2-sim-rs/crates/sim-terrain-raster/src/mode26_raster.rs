//! Exact indexed-framebuffer compositing for the original terrain mode 26.
//!
//! Mode 26 shares mode 5's triangle setup, clipping, affine U/V stepping and
//! pnt5 stepping. Its distinguishing behavior is the water-texture kernel:
//! texture indices 12 and above use the ordinary shade lookup, while indices
//! 0 through 11 shade first and then blend against the indexed framebuffer
//! pixel that is already present. This is why the outdoor reflection pass must
//! complete before primary water triangles are submitted.

use crate::mode5_raster::{
    Mode5Scanline, Mode5Span, Mode5TextureAddressing, Mode5Vertex, setup_mode5_triangle,
    span_from_scanline, tile_texture_address,
};

/// Exact byte count of the qualified MC2 night renderer table resource.
pub const NIGHT_TABLES_BYTES: usize = 83_456;

/// First byte of the framebuffer-dependent blend table inside TABLESN.
pub const MODE26_BLEND_OFFSET: usize = 16_384;

/// A source texture index below this value takes mode 26's blend path.
pub const MODE26_DIRECT_TEXTURE_THRESHOLD: u8 = 12;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode26Pixel {
    pub x: i32,
    pub u: i32,
    pub v: i32,
    pub pnt5: i32,
    pub texture_address: u16,
    pub shade_address: u16,
    pub source_index: u8,
    pub shaded_index: u8,
    pub previous_framebuffer_index: u8,
    pub blend_address: Option<u32>,
    pub output_index: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode26FramePixel {
    pub y: i32,
    pub sample: Mode26Pixel,
}

/// Composite one exact mode-26 scanline into an indexed framebuffer row.
///
/// Returned samples describe the values observed and written in strict
/// left-to-right source order. The framebuffer row is mutated as pixels are
/// produced.
///
/// # Panics
///
/// Panics unless the texture is 256×256, TABLESN is the qualified 83,456-byte
/// night table, the framebuffer row matches the non-negative viewport width,
/// or the viewport width does not fit `usize`.
#[must_use]
pub fn rasterize_mode26_span(
    span: Mode5Span,
    viewport_width: i32,
    texture: &[u8],
    tablesn: &[u8],
    framebuffer_row: &mut [u8],
) -> Vec<Mode26Pixel> {
    assert_eq!(texture.len(), 256 * 256, "mode-26 texture must be 256x256");
    rasterize_mode26_span_from_atlas(span, viewport_width, texture, 0, tablesn, framebuffer_row)
}

/// Composite one exact mode-26 span through a source 256-stride
/// texture-atlas pointer.
#[must_use]
pub fn rasterize_mode26_span_from_atlas(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer_row: &mut [u8],
) -> Vec<Mode26Pixel> {
    let mut pixels = Vec::new();
    visit_mode26_span_from_atlas(
        span,
        viewport_width,
        texture_atlas,
        texture_base,
        tablesn,
        framebuffer_row,
        Mode5TextureAddressing::Full256,
        |pixel| pixels.push(pixel),
    );
    pixels
}

pub(crate) fn rasterize_mode26_span_from_tile_atlas(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer_row: &mut [u8],
) -> Vec<Mode26Pixel> {
    let mut pixels = Vec::new();
    visit_mode26_span_from_atlas(
        span,
        viewport_width,
        texture_atlas,
        texture_base,
        tablesn,
        framebuffer_row,
        Mode5TextureAddressing::Tile32,
        |pixel| pixels.push(pixel),
    );
    pixels
}

fn visit_mode26_span_from_atlas<F>(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer_row: &mut [u8],
    addressing: Mode5TextureAddressing,
    mut visitor: F,
) where
    F: FnMut(Mode26Pixel),
{
    assert!(
        texture_base < texture_atlas.len(),
        "mode-26 texture base must point inside its atlas"
    );
    assert_eq!(
        tablesn.len(),
        NIGHT_TABLES_BYTES,
        "TABLESN must be the qualified 83,456-byte night renderer table"
    );
    assert!(viewport_width >= 0, "viewport width must be non-negative");
    let width = usize::try_from(viewport_width).expect("viewport width fits usize");
    assert_eq!(
        framebuffer_row.len(),
        width,
        "mode-26 framebuffer row must match viewport width"
    );

    let start = span.left_x.max(0);
    let end = span.right_x.min(viewport_width);
    if end <= start {
        return;
    }

    let clipped = start.wrapping_sub(span.left_x);
    let mut u = span.left_u.wrapping_add(span.du.wrapping_mul(clipped));
    let mut v = span.left_v.wrapping_add(span.dv.wrapping_mul(clipped));
    let mut pnt5 = span
        .left_pnt5
        .wrapping_add(span.dpnt5.wrapping_mul(clipped));
    for x in start..end {
        let texture_address = match addressing {
            Mode5TextureAddressing::Full256 => {
                (u16::from(fixed_byte(v)) << 8) | u16::from(fixed_byte(u))
            }
            Mode5TextureAddressing::Tile32 => {
                let Some(texture_address) = tile_texture_address(u, v) else {
                    break;
                };
                texture_address
            }
        };
        let atlas_address = texture_base
            .checked_add(usize::from(texture_address))
            .expect("mode-26 atlas address fits usize");
        let source_index = match addressing {
            Mode5TextureAddressing::Full256 => *texture_atlas
                .get(atlas_address)
                .expect("mode-26 stepped texture address remains inside its source atlas"),
            Mode5TextureAddressing::Tile32 => {
                let Some(source_index) = texture_atlas.get(atlas_address) else {
                    break;
                };
                *source_index
            }
        };
        let shade_address = (u16::from(fixed_byte(pnt5)) << 8) | u16::from(source_index);
        let shaded_index = tablesn[usize::from(shade_address)];
        let frame_index = usize::try_from(x).expect("clipped mode-26 X is non-negative");
        let previous_framebuffer_index = framebuffer_row[frame_index];
        let (blend_address, output_index) = if source_index >= MODE26_DIRECT_TEXTURE_THRESHOLD {
            (None, shaded_index)
        } else {
            let address = MODE26_BLEND_OFFSET
                + (usize::from(previous_framebuffer_index) << 8)
                + usize::from(shaded_index);
            (
                Some(u32::try_from(address).expect("mode-26 table address fits u32")),
                tablesn[address],
            )
        };
        framebuffer_row[frame_index] = output_index;
        visitor(Mode26Pixel {
            x,
            u,
            v,
            pnt5,
            texture_address,
            shade_address,
            source_index,
            shaded_index,
            previous_framebuffer_index,
            blend_address,
            output_index,
        });
        u = u.wrapping_add(span.du);
        v = v.wrapping_add(span.dv);
        pnt5 = pnt5.wrapping_add(span.dpnt5);
    }
}

/// Setup and composite one complete mode-26 triangle into an indexed frame.
///
/// The frame is row-major and exactly `viewport_width * viewport_height`
/// bytes. Samples are returned in source scanline and left-to-right order.
#[must_use]
pub fn rasterize_mode26_triangle_into_framebuffer(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture: &[u8],
    tablesn: &[u8],
    framebuffer: &mut [u8],
) -> Vec<Mode26FramePixel> {
    assert_eq!(texture.len(), 256 * 256, "mode-26 texture must be 256x256");
    rasterize_mode26_triangle_from_atlas_into_framebuffer(
        vertices,
        viewport_width,
        viewport_height,
        texture,
        0,
        tablesn,
        framebuffer,
    )
}

/// Setup and composite a mode-26 triangle through a source 256-stride
/// texture-atlas pointer into an indexed frame.
#[must_use]
pub fn rasterize_mode26_triangle_from_atlas_into_framebuffer(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer: &mut [u8],
) -> Vec<Mode26FramePixel> {
    rasterize_mode26_triangle_from_atlas_with_addressing(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        framebuffer,
        Mode5TextureAddressing::Full256,
    )
}

#[doc(hidden)]
pub fn rasterize_mode26_triangle_from_tile_atlas_into_framebuffer(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer: &mut [u8],
) -> Vec<Mode26FramePixel> {
    rasterize_mode26_triangle_from_atlas_with_addressing(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        framebuffer,
        Mode5TextureAddressing::Tile32,
    )
}

fn rasterize_mode26_triangle_from_atlas_with_addressing(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer: &mut [u8],
    addressing: Mode5TextureAddressing,
) -> Vec<Mode26FramePixel> {
    assert!(
        viewport_width >= 0 && viewport_height >= 0,
        "mode-26 viewport dimensions must be non-negative"
    );
    assert!(
        texture_base < texture_atlas.len(),
        "mode-26 texture base must point inside its atlas"
    );
    let width = usize::try_from(viewport_width).expect("viewport width fits usize");
    let height = usize::try_from(viewport_height).expect("viewport height fits usize");
    assert_eq!(
        framebuffer.len(),
        width
            .checked_mul(height)
            .expect("mode-26 framebuffer extent fits usize"),
        "mode-26 framebuffer must match the viewport"
    );

    let Some(setup) = setup_mode5_triangle(vertices[0], vertices[1], vertices[2], viewport_height)
    else {
        return Vec::new();
    };
    let mut frame_pixels = Vec::new();
    for row in setup.scanlines {
        let Some(span) =
            mode26_span_from_scanline(row, setup.du, setup.dv, setup.dpnt5, viewport_width)
        else {
            continue;
        };
        let row_start = usize::try_from(row.y)
            .expect("mode-26 setup clips Y non-negative")
            .checked_mul(width)
            .expect("mode-26 framebuffer row offset fits usize");
        let row_end = row_start + width;
        let pixels = match addressing {
            Mode5TextureAddressing::Full256 => rasterize_mode26_span_from_atlas(
                span,
                viewport_width,
                texture_atlas,
                texture_base,
                tablesn,
                &mut framebuffer[row_start..row_end],
            ),
            Mode5TextureAddressing::Tile32 => rasterize_mode26_span_from_tile_atlas(
                span,
                viewport_width,
                texture_atlas,
                texture_base,
                tablesn,
                &mut framebuffer[row_start..row_end],
            ),
        };
        frame_pixels.extend(
            pixels
                .into_iter()
                .map(|sample| Mode26FramePixel { y: row.y, sample }),
        );
    }
    frame_pixels
}

/// Visit a complete mode-26 triangle without retaining one `Mode26Pixel`
/// record per sampled pixel. Classic HD uses this streaming seam because
/// large profile-sized water triangles otherwise spend most of their time
/// growing and copying temporary sample vectors.
#[doc(hidden)]
pub fn visit_mode26_triangle_from_tile_atlas_into_framebuffer<F>(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer: &mut [u8],
    visitor: F,
) where
    F: FnMut(Mode26FramePixel),
{
    visit_mode26_triangle_from_atlas_with_addressing(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        framebuffer,
        Mode5TextureAddressing::Tile32,
        visitor,
    );
}

fn visit_mode26_triangle_from_atlas_with_addressing<F>(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    framebuffer: &mut [u8],
    addressing: Mode5TextureAddressing,
    mut visitor: F,
) where
    F: FnMut(Mode26FramePixel),
{
    assert!(
        viewport_width >= 0 && viewport_height >= 0,
        "mode-26 viewport dimensions must be non-negative"
    );
    assert!(
        texture_base < texture_atlas.len(),
        "mode-26 texture base must point inside its atlas"
    );
    let width = usize::try_from(viewport_width).expect("viewport width fits usize");
    let height = usize::try_from(viewport_height).expect("viewport height fits usize");
    assert_eq!(
        framebuffer.len(),
        width
            .checked_mul(height)
            .expect("mode-26 framebuffer extent fits usize"),
        "mode-26 framebuffer must match the viewport"
    );

    let Some(setup) = setup_mode5_triangle(vertices[0], vertices[1], vertices[2], viewport_height)
    else {
        return;
    };
    for row in setup.scanlines {
        let Some(span) =
            mode26_span_from_scanline(row, setup.du, setup.dv, setup.dpnt5, viewport_width)
        else {
            continue;
        };
        let row_start = usize::try_from(row.y)
            .expect("mode-26 setup clips Y non-negative")
            .checked_mul(width)
            .expect("mode-26 framebuffer row offset fits usize");
        let row_end = row_start + width;
        visit_mode26_span_from_atlas(
            span,
            viewport_width,
            texture_atlas,
            texture_base,
            tablesn,
            &mut framebuffer[row_start..row_end],
            addressing,
            |sample| visitor(Mode26FramePixel { y: row.y, sample }),
        );
    }
}

fn mode26_span_from_scanline(
    row: Mode5Scanline,
    du: i32,
    dv: i32,
    dpnt5: i32,
    viewport_width: i32,
) -> Option<Mode5Span> {
    span_from_scanline(row, du, dv, dpnt5, viewport_width)
}

const fn fixed_byte(value: i32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> Vec<u8> {
        let mut table = vec![0; NIGHT_TABLES_BYTES];
        for row in 0..256_usize {
            for source in 0..256_usize {
                table[(row << 8) | source] = (row as u8).wrapping_add(source as u8);
            }
        }
        for framebuffer in 0..256_usize {
            for shaded in 0..256_usize {
                table[MODE26_BLEND_OFFSET + (framebuffer << 8) + shaded] =
                    (framebuffer as u8).wrapping_sub(shaded as u8);
            }
        }
        table
    }

    fn one_pixel_span() -> Mode5Span {
        Mode5Span {
            left_x: 0,
            right_x: 1,
            left_u: 0,
            left_v: 0,
            left_pnt5: 3 << 16,
            du: 0,
            dv: 0,
            dpnt5: 0,
        }
    }

    #[test]
    fn texture_index_twelve_uses_direct_shade_and_ignores_framebuffer() {
        let mut texture = vec![0; 256 * 256];
        texture[0] = 12;
        let tablesn = tables();
        let mut row = vec![231];
        let pixels = rasterize_mode26_span(one_pixel_span(), 1, &texture, &tablesn, &mut row);
        assert_eq!(pixels[0].shade_address, 0x030c);
        assert_eq!(pixels[0].shaded_index, 15);
        assert_eq!(pixels[0].previous_framebuffer_index, 231);
        assert_eq!(pixels[0].blend_address, None);
        assert_eq!(pixels[0].output_index, 15);
        assert_eq!(row, [15]);
    }

    #[test]
    fn texture_index_eleven_shades_then_blends_against_existing_framebuffer() {
        let mut texture = vec![0; 256 * 256];
        texture[0] = 11;
        let tablesn = tables();
        let mut row = vec![231];
        let pixels = rasterize_mode26_span(one_pixel_span(), 1, &texture, &tablesn, &mut row);
        assert_eq!(pixels[0].shade_address, 0x030b);
        assert_eq!(pixels[0].shaded_index, 14);
        assert_eq!(
            pixels[0].blend_address,
            Some((MODE26_BLEND_OFFSET + (231 << 8) + 14) as u32)
        );
        assert_eq!(pixels[0].output_index, 217);
        assert_eq!(row, [217]);
    }

    #[test]
    fn blend_address_uses_shaded_result_as_low_byte_and_reaches_exact_maximum() {
        let mut texture = vec![0; 256 * 256];
        texture[0] = 0;
        let mut tablesn = tables();
        tablesn[0] = 255;
        tablesn[MODE26_BLEND_OFFSET + 65_535] = 73;
        let mut row = vec![255];
        let pixels = rasterize_mode26_span(
            Mode5Span {
                left_pnt5: 0,
                ..one_pixel_span()
            },
            1,
            &texture,
            &tablesn,
            &mut row,
        );
        assert_eq!(pixels[0].blend_address, Some(81_919));
        assert_eq!(pixels[0].output_index, 73);
    }

    #[test]
    fn left_clip_advances_wrapped_attributes_before_sampling() {
        let mut texture = vec![12; 256 * 256];
        texture[0x0201] = 20;
        let tablesn = tables();
        let mut row = vec![0; 2];
        let pixels = rasterize_mode26_span(
            Mode5Span {
                left_x: -1,
                right_x: 1,
                left_u: 0,
                left_v: 0,
                left_pnt5: 0,
                du: 1 << 16,
                dv: 2 << 16,
                dpnt5: 3 << 16,
            },
            2,
            &texture,
            &tablesn,
            &mut row,
        );
        assert_eq!(pixels.len(), 1);
        assert_eq!(
            (pixels[0].x, pixels[0].u, pixels[0].v),
            (0, 1 << 16, 2 << 16)
        );
        assert_eq!(pixels[0].texture_address, 0x0201);
        assert_eq!(pixels[0].shade_address, 0x0314);
    }

    #[test]
    fn tile_atlas_addressing_aborts_span_before_out_of_range_dereference() {
        let mut atlas = vec![0; 152 * 32 * 32];
        let base = (151 / 8) * 32 * 256 + (151 % 8) * 32;
        atlas[base + 0x0010] = 12;
        let tablesn = tables();
        let mut row = vec![77, 77];
        let pixels = rasterize_mode26_span_from_tile_atlas(
            Mode5Span {
                left_x: 0,
                right_x: 2,
                left_u: 16 << 16,
                left_v: 0,
                dv: -(1 << 16),
                ..Mode5Span::default()
            },
            2,
            &atlas,
            base,
            &tablesn,
            &mut row,
        );
        assert_eq!(pixels.len(), 1);
        assert_eq!(
            pixels
                .iter()
                .map(|pixel| pixel.texture_address)
                .collect::<Vec<_>>(),
            vec![0x0010]
        );
        assert_eq!(row, [12, 77]);
    }

    #[test]
    fn complete_triangle_mutates_framebuffer_in_source_order() {
        let mut texture = vec![0; 256 * 256];
        texture[0] = 11;
        let tablesn = tables();
        let mut framebuffer = vec![100; 4 * 4];
        let samples = rasterize_mode26_triangle_into_framebuffer(
            [
                Mode5Vertex {
                    x: 0,
                    y: 0,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
                Mode5Vertex {
                    x: 3,
                    y: 0,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
                Mode5Vertex {
                    x: 0,
                    y: 3,
                    u: 0,
                    v: 0,
                    pnt5: 3 << 16,
                },
            ],
            4,
            4,
            &texture,
            &tablesn,
            &mut framebuffer,
        );
        assert!(!samples.is_empty());
        assert_eq!(samples[0].sample.previous_framebuffer_index, 100);
        assert_eq!(framebuffer[0], 86);
    }

    #[test]
    fn repeated_water_submission_observes_the_previous_write() {
        let mut texture = vec![0; 256 * 256];
        texture[0] = 11;
        let tablesn = tables();
        let mut row = vec![100];

        let first = rasterize_mode26_span(one_pixel_span(), 1, &texture, &tablesn, &mut row);
        let second = rasterize_mode26_span(one_pixel_span(), 1, &texture, &tablesn, &mut row);

        assert_eq!(first[0].previous_framebuffer_index, 100);
        assert_eq!(first[0].output_index, 86);
        assert_eq!(second[0].previous_framebuffer_index, 86);
        assert_eq!(second[0].output_index, 72);
        assert_eq!(row, [72]);
    }

    #[test]
    fn qualified_owner_night_table_pins_direct_and_blended_water_vectors() {
        let private = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let tablesn =
            std::fs::read(private.join("generated/level1-unity-asset-cache/palette/tablesn.u8"))
                .expect("qualified private Level 1 TABLESN cache must exist");
        assert_eq!(tablesn.len(), NIGHT_TABLES_BYTES);

        let mut texture = vec![0; 256 * 256];
        let mut row = vec![77];
        texture[0] = 11;
        let blended = rasterize_mode26_span(
            Mode5Span {
                left_pnt5: 34 << 16,
                ..one_pixel_span()
            },
            1,
            &texture,
            &tablesn,
            &mut row,
        );
        assert_eq!(blended[0].shade_address, 8_715);
        assert_eq!(blended[0].shaded_index, 12);
        assert_eq!(blended[0].blend_address, Some(36_108));
        assert_eq!(blended[0].output_index, 63);

        row[0] = 77;
        texture[0] = 12;
        let direct = rasterize_mode26_span(
            Mode5Span {
                left_pnt5: 34 << 16,
                ..one_pixel_span()
            },
            1,
            &texture,
            &tablesn,
            &mut row,
        );
        assert_eq!(direct[0].shade_address, 8_716);
        assert_eq!(direct[0].shaded_index, 13);
        assert_eq!(direct[0].blend_address, None);
        assert_eq!(direct[0].output_index, 13);
    }
}
