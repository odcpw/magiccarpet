//! Exact horizontal kernel for the original terrain mode-5 rasterizer.
//!
//! Mode 5 is affine in screen space, but it is not equivalent to merely
//! applying a GPU `noperspective` qualifier. The source advances wrapped
//! 16.16 integers, uses truncating signed division, samples before advancing,
//! and extracts address bytes from the stepped integer state.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode5Span {
    /// Inclusive integer screen X from the scanline's left 16.16 edge.
    pub left_x: i32,
    /// Exclusive integer screen X from the scanline's right 16.16 edge.
    pub right_x: i32,
    pub left_u: i32,
    pub left_v: i32,
    pub left_pnt5: i32,
    pub du: i32,
    pub dv: i32,
    pub dpnt5: i32,
}

/// One projected source vertex as consumed by mode 5.
///
/// X and Y are integer screen coordinates. U, V and pnt5 are signed wrapped
/// 16.16 values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode5Vertex {
    pub x: i32,
    pub y: i32,
    pub u: i32,
    pub v: i32,
    pub pnt5: i32,
}

/// The 20-byte logical scanline record built by the original triangle setup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode5Scanline {
    pub y: i32,
    pub left_x_16_16: i32,
    pub right_x_16_16: i32,
    pub left_u_16_16: i32,
    pub left_v_16_16: i32,
    pub left_pnt5_16_16: i32,
}

/// Exact mode-5 triangle setup result before horizontal clipping and sampling.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mode5TriangleSetup {
    pub du: i32,
    pub dv: i32,
    pub dpnt5: i32,
    pub scanlines: Vec<Mode5Scanline>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode5Pixel {
    pub x: i32,
    pub u: i32,
    pub v: i32,
    pub pnt5: i32,
    pub texture_address: u16,
    pub tablesn_address: u16,
    pub source_index: u8,
    pub output_index: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode5FramePixel {
    pub y: i32,
    pub sample: Mode5Pixel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode5TextureAddressing {
    Full256,
    Tile32,
}

pub(crate) const fn tile_texture_address(u: i32, v: i32) -> Option<u16> {
    // The qualified HD source checks the raw packed address against the
    // 32-pixel tile span before dereferencing it. Do not wrap or clamp a
    // stepped value: an invalid sample terminates the current span.
    let address = raw_texture_address(u, v);
    if address > (32_u16 << 8) {
        None
    } else {
        Some(address)
    }
}

const fn raw_texture_address(u: i32, v: i32) -> u16 {
    let u_byte = fixed_byte(u);
    let v_byte = fixed_byte(v);
    ((v_byte as u16) << 8) | u_byte as u16
}

/// Source horizontal attribute increment.
///
/// The rasterizer divides by `integer_width + 1`, not by the number of pixels
/// subsequently written. Signed division truncates toward zero.
#[must_use]
pub fn mode5_horizontal_increment(left: i32, right: i32, integer_width: i32) -> i32 {
    let divisor = integer_width.wrapping_add(1);
    assert_ne!(divisor, 0, "mode-5 span divisor must be nonzero");
    right.wrapping_sub(left) / divisor
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TriangleShape {
    /// Top, middle-right, bottom. The long edge is the left edge.
    MiddleRight {
        top: Mode5Vertex,
        middle: Mode5Vertex,
        bottom: Mode5Vertex,
    },
    /// Top, bottom, middle-left. The segmented edge is the left edge.
    MiddleLeft {
        top: Mode5Vertex,
        middle: Mode5Vertex,
        bottom: Mode5Vertex,
    },
    FlatTop {
        left: Mode5Vertex,
        right: Mode5Vertex,
        bottom: Mode5Vertex,
    },
    FlatBottom {
        top: Mode5Vertex,
        left: Mode5Vertex,
        right: Mode5Vertex,
    },
}

/// Build the source's mode-5 scanline records.
///
/// This ports the original vertex-order branches rather than sorting and
/// applying a barycentric fill rule. That distinction preserves its winding
/// rejection, flat-edge rules, signed fixed-point divisions, exact middle-row
/// edge reset, and top-inclusive/bottom-exclusive coverage.
#[must_use]
pub fn setup_mode5_triangle(
    vertex1: Mode5Vertex,
    vertex2: Mode5Vertex,
    vertex3: Mode5Vertex,
    viewport_height: i32,
) -> Option<Mode5TriangleSetup> {
    assert!(
        viewport_height >= 0,
        "mode-5 viewport height must be non-negative"
    );
    let shape = classify_triangle(vertex1, vertex2, vertex3)?;
    let (du, dv, dpnt5) = horizontal_gradients(shape)?;
    let (top_y, bottom_y) = shape_y_bounds(shape);
    if top_y >= viewport_height || bottom_y <= 0 {
        return None;
    }
    let start_y = top_y.max(0);
    let end_y = bottom_y.min(viewport_height);
    let mut scanlines = Vec::with_capacity(
        usize::try_from(end_y - start_y).expect("positive triangle height fits usize"),
    );

    for y in start_y..end_y {
        let (left_x, right_x, left_u, left_v, left_pnt5) = scanline_at(shape, y);
        scanlines.push(Mode5Scanline {
            y,
            left_x_16_16: left_x,
            right_x_16_16: right_x,
            left_u_16_16: left_u,
            left_v_16_16: left_v,
            left_pnt5_16_16: left_pnt5,
        });
    }
    if scanlines.is_empty() {
        None
    } else {
        Some(Mode5TriangleSetup {
            du,
            dv,
            dpnt5,
            scanlines,
        })
    }
}

/// Setup and shade one complete mode-5 triangle.
///
/// Returned samples remain in source scanline and left-to-right submission
/// order. A caller emulating the framebuffer can therefore apply multiple
/// triangle results sequentially with the same last-write-wins behavior.
#[must_use]
pub fn rasterize_mode5_triangle(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture: &[u8],
    tablesn: &[u8],
) -> Vec<Mode5FramePixel> {
    assert_eq!(texture.len(), 256 * 256, "mode-5 texture must be 256x256");
    rasterize_mode5_triangle_from_atlas(
        vertices,
        viewport_width,
        viewport_height,
        texture,
        0,
        tablesn,
    )
}

/// Setup and shade one complete mode-5 triangle through a source
/// 256-stride texture-atlas pointer.
#[must_use]
pub fn rasterize_mode5_triangle_from_atlas(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
) -> Vec<Mode5FramePixel> {
    let mut frame_pixels = Vec::new();
    visit_mode5_triangle_from_atlas(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        |pixel| frame_pixels.push(pixel),
    );
    frame_pixels
}

#[doc(hidden)]
pub fn rasterize_mode5_triangle_from_tile_atlas(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
) -> Vec<Mode5FramePixel> {
    let mut frame_pixels = Vec::new();
    visit_mode5_triangle_from_tile_atlas(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        |pixel| frame_pixels.push(pixel),
    );
    frame_pixels
}

/// Visit complete mode-5 triangle samples without allocating a frame-sized
/// sample vector. The source traversal and sample order are identical to
/// [`rasterize_mode5_triangle_from_atlas`]; Classic HD uses this presentation
/// seam to write directly into its profile-sized frame while retaining the
/// sealed Exact path's existing vector-returning API.
pub(crate) fn visit_mode5_triangle_from_atlas<F>(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    visitor: F,
) where
    F: FnMut(Mode5FramePixel),
{
    visit_mode5_triangle_from_atlas_with_addressing(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        Mode5TextureAddressing::Full256,
        visitor,
    );
}

#[doc(hidden)]
pub fn visit_mode5_triangle_from_tile_atlas<F>(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    visitor: F,
) where
    F: FnMut(Mode5FramePixel),
{
    visit_mode5_triangle_from_atlas_with_addressing(
        vertices,
        viewport_width,
        viewport_height,
        texture_atlas,
        texture_base,
        tablesn,
        Mode5TextureAddressing::Tile32,
        visitor,
    );
}

fn visit_mode5_triangle_from_atlas_with_addressing<F>(
    vertices: [Mode5Vertex; 3],
    viewport_width: i32,
    viewport_height: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    addressing: Mode5TextureAddressing,
    mut visitor: F,
) where
    F: FnMut(Mode5FramePixel),
{
    assert!(
        viewport_width >= 0,
        "mode-5 viewport width must be non-negative"
    );
    assert!(
        texture_base < texture_atlas.len(),
        "mode-5 texture base must point inside its atlas"
    );
    let Some(setup) = setup_mode5_triangle(vertices[0], vertices[1], vertices[2], viewport_height)
    else {
        return;
    };
    for row in setup.scanlines {
        let Some(span) = span_from_scanline(row, setup.du, setup.dv, setup.dpnt5, viewport_width)
        else {
            continue;
        };
        visit_mode5_span_from_atlas(
            span,
            viewport_width,
            texture_atlas,
            texture_base,
            tablesn,
            addressing,
            |sample| visitor(Mode5FramePixel { y: row.y, sample }),
        );
    }
}

pub(crate) fn span_from_scanline(
    row: Mode5Scanline,
    du: i32,
    dv: i32,
    dpnt5: i32,
    viewport_width: i32,
) -> Option<Mode5Span> {
    let left_word = (row.left_x_16_16 >> 16) as u16;
    let right_word = (row.right_x_16_16 >> 16) as u16;
    let left = i32::from(left_word as i16);
    let right = if left < 0 {
        // The left-clipped source branch reinterprets right as signed i16.
        let signed = i32::from(right_word as i16);
        (signed > 0).then_some(signed)?
    } else {
        // The nonnegative-left branch keeps right as u16 until its viewport
        // clamp. This asymmetry is observable at high-word 0x8000.
        i32::from(right_word).min(viewport_width)
    };
    Some(Mode5Span {
        left_x: left,
        right_x: right,
        left_u: row.left_u_16_16,
        left_v: row.left_v_16_16,
        left_pnt5: row.left_pnt5_16_16,
        du,
        dv,
        dpnt5,
    })
}

fn classify_triangle(
    vertex1: Mode5Vertex,
    vertex2: Mode5Vertex,
    vertex3: Mode5Vertex,
) -> Option<TriangleShape> {
    let (v6, v7, v8) = (vertex1.y, vertex2.y, vertex3.y);
    if v6 == v7 {
        if v6 == v8 {
            return None;
        }
        if v6 >= v8 {
            if vertex1.x <= vertex2.x {
                return None;
            }
            return Some(TriangleShape::FlatBottom {
                top: vertex3,
                left: vertex2,
                right: vertex1,
            });
        }
        if vertex2.x <= vertex1.x {
            return None;
        }
        return Some(TriangleShape::FlatTop {
            left: vertex1,
            right: vertex2,
            bottom: vertex3,
        });
    }
    if v6 <= v7 {
        if v6 != v8 {
            if v6 >= v8 {
                return middle_right(vertex3, vertex1, vertex2);
            }
            if v7 != v8 {
                if v7 <= v8 {
                    return middle_right(vertex1, vertex2, vertex3);
                }
                return middle_left(vertex1, vertex3, vertex2);
            }
            if vertex2.x <= vertex3.x {
                return None;
            }
            return Some(TriangleShape::FlatBottom {
                top: vertex1,
                left: vertex3,
                right: vertex2,
            });
        }
        if vertex1.x <= vertex3.x {
            return None;
        }
        return Some(TriangleShape::FlatTop {
            left: vertex3,
            right: vertex1,
            bottom: vertex2,
        });
    }
    if v6 == v8 {
        if vertex3.x <= vertex1.x {
            return None;
        }
        return Some(TriangleShape::FlatBottom {
            top: vertex2,
            left: vertex1,
            right: vertex3,
        });
    }
    if v6 < v8 {
        return middle_left(vertex2, vertex1, vertex3);
    }
    if v7 == v8 {
        if vertex3.x <= vertex2.x {
            return None;
        }
        return Some(TriangleShape::FlatTop {
            left: vertex2,
            right: vertex3,
            bottom: vertex1,
        });
    }
    if v7 < v8 {
        return middle_right(vertex2, vertex3, vertex1);
    }
    middle_left(vertex3, vertex2, vertex1)
}

fn middle_right(
    top: Mode5Vertex,
    middle: Mode5Vertex,
    bottom: Mode5Vertex,
) -> Option<TriangleShape> {
    let middle_slope = x_slope(top, middle);
    let long_slope = x_slope(top, bottom);
    (middle_slope > long_slope).then_some(TriangleShape::MiddleRight {
        top,
        middle,
        bottom,
    })
}

fn middle_left(
    top: Mode5Vertex,
    middle: Mode5Vertex,
    bottom: Mode5Vertex,
) -> Option<TriangleShape> {
    let middle_slope = x_slope(top, middle);
    let long_slope = x_slope(top, bottom);
    (long_slope > middle_slope).then_some(TriangleShape::MiddleLeft {
        top,
        middle,
        bottom,
    })
}

fn horizontal_gradients(shape: TriangleShape) -> Option<(i32, i32, i32)> {
    let (left, right, width, plus_one) = match shape {
        TriangleShape::MiddleRight {
            top,
            middle,
            bottom,
        } => {
            let elapsed = middle.y.wrapping_sub(top.y);
            let height = bottom.y.wrapping_sub(top.y);
            let term = mul_div(elapsed, top.x.wrapping_sub(bottom.x), height);
            let delta = middle.x.wrapping_sub(top.x);
            let (width, overflow) = term.overflowing_add(delta);
            if signed_add_is_negative(width, overflow) {
                return None;
            }
            if width == 0 {
                return Some((0, 0, 0));
            }
            let divisor = width.wrapping_add(1);
            assert_ne!(divisor, 0, "source-valid mode-5 divisor must be nonzero");
            let numerator = |top_value: i32, middle_value: i32, bottom_value: i32| {
                middle_value
                    .wrapping_add(mul_div(
                        elapsed,
                        top_value.wrapping_sub(bottom_value),
                        height,
                    ))
                    .wrapping_sub(top_value)
            };
            return Some((
                numerator(top.u, middle.u, bottom.u) / divisor,
                numerator(top.v, middle.v, bottom.v) / divisor,
                numerator(top.pnt5, middle.pnt5, bottom.pnt5) / divisor,
            ));
        }
        TriangleShape::MiddleLeft {
            top,
            middle,
            bottom,
        } => {
            let at_middle = interpolate_vertex_integer(top, bottom, middle.y.wrapping_sub(top.y));
            let term = mul_div(
                middle.y.wrapping_sub(top.y),
                bottom.x.wrapping_sub(top.x),
                bottom.y.wrapping_sub(top.y),
            );
            let delta = top.x.wrapping_sub(middle.x);
            let (width, overflow) = term.overflowing_add(delta);
            if signed_add_is_negative(width, overflow) {
                return None;
            }
            (middle, at_middle, width, true)
        }
        TriangleShape::FlatTop { left, right, .. }
        | TriangleShape::FlatBottom { left, right, .. } => {
            (left, right, right.x.wrapping_sub(left.x), false)
        }
    };
    if width == 0 {
        return Some((0, 0, 0));
    }
    let divisor = if plus_one {
        width.wrapping_add(1)
    } else {
        width
    };
    assert_ne!(divisor, 0, "source-valid mode-5 divisor must be nonzero");
    Some((
        right.u.wrapping_sub(left.u) / divisor,
        right.v.wrapping_sub(left.v) / divisor,
        right.pnt5.wrapping_sub(left.pnt5) / divisor,
    ))
}

const fn signed_add_is_negative(wrapped_result: i32, overflow: bool) -> bool {
    (wrapped_result < 0) ^ overflow
}

fn shape_y_bounds(shape: TriangleShape) -> (i32, i32) {
    match shape {
        TriangleShape::MiddleRight { top, bottom, .. }
        | TriangleShape::MiddleLeft { top, bottom, .. }
        | TriangleShape::FlatTop {
            left: top, bottom, ..
        }
        | TriangleShape::FlatBottom {
            top, left: bottom, ..
        } => (top.y, bottom.y),
    }
}

fn scanline_at(shape: TriangleShape, y: i32) -> (i32, i32, i32, i32, i32) {
    match shape {
        TriangleShape::MiddleRight {
            top,
            middle,
            bottom,
        } => {
            let left = interpolate_vertex_fixed(top, bottom, y.wrapping_sub(top.y));
            let right = if y < middle.y {
                interpolate_vertex_fixed(top, middle, y.wrapping_sub(top.y))
            } else {
                interpolate_vertex_fixed(middle, bottom, y.wrapping_sub(middle.y))
            };
            (left.x, right.x, left.u, left.v, left.pnt5)
        }
        TriangleShape::MiddleLeft {
            top,
            middle,
            bottom,
        } => {
            let left = if y < middle.y {
                interpolate_vertex_fixed(top, middle, y.wrapping_sub(top.y))
            } else {
                interpolate_segmented_left_fixed(top, middle, bottom, y.wrapping_sub(middle.y))
            };
            let right = interpolate_vertex_fixed(top, bottom, y.wrapping_sub(top.y));
            (left.x, right.x, left.u, left.v, left.pnt5)
        }
        TriangleShape::FlatTop {
            left,
            right,
            bottom,
        } => {
            let left_edge = interpolate_vertex_fixed(left, bottom, y.wrapping_sub(left.y));
            let right_edge = interpolate_vertex_fixed(right, bottom, y.wrapping_sub(right.y));
            (
                left_edge.x,
                right_edge.x,
                left_edge.u,
                left_edge.v,
                left_edge.pnt5,
            )
        }
        TriangleShape::FlatBottom { top, left, right } => {
            let left_edge = interpolate_vertex_fixed(top, left, y.wrapping_sub(top.y));
            let right_edge = interpolate_vertex_fixed(top, right, y.wrapping_sub(top.y));
            (
                left_edge.x,
                right_edge.x,
                left_edge.u,
                left_edge.v,
                left_edge.pnt5,
            )
        }
    }
}

fn interpolate_segmented_left_fixed(
    top: Mode5Vertex,
    middle: Mode5Vertex,
    bottom: Mode5Vertex,
    lower_elapsed_y: i32,
) -> Mode5Vertex {
    let upper_height = middle.y.wrapping_sub(top.y);
    let lower_height = bottom.y.wrapping_sub(middle.y);
    assert!(
        upper_height > 0 && lower_height > 0,
        "mode-5 segmented edge heights must be positive"
    );
    let accumulated = |top_value: i32, middle_value: i32, bottom_value: i32| {
        let at_middle = top_value.wrapping_add(
            middle_value
                .wrapping_sub(top_value)
                .wrapping_div(upper_height)
                .wrapping_mul(upper_height),
        );
        at_middle.wrapping_add(
            bottom_value
                .wrapping_sub(middle_value)
                .wrapping_div(lower_height)
                .wrapping_mul(lower_elapsed_y),
        )
    };
    Mode5Vertex {
        // The source resets only segmented X to the exact middle coordinate.
        // Attribute accumulators deliberately retain upper-edge truncation.
        x: fixed_x(middle.x).wrapping_add(x_slope(middle, bottom).wrapping_mul(lower_elapsed_y)),
        y: middle.y.wrapping_add(lower_elapsed_y),
        u: accumulated(top.u, middle.u, bottom.u),
        v: accumulated(top.v, middle.v, bottom.v),
        pnt5: accumulated(top.pnt5, middle.pnt5, bottom.pnt5),
    }
}

fn interpolate_vertex_integer(start: Mode5Vertex, end: Mode5Vertex, elapsed_y: i32) -> Mode5Vertex {
    let height = end.y.wrapping_sub(start.y);
    assert!(height > 0, "mode-5 interpolation edge must descend");
    Mode5Vertex {
        x: start
            .x
            .wrapping_add(mul_div(elapsed_y, end.x.wrapping_sub(start.x), height)),
        y: start.y.wrapping_add(elapsed_y),
        u: start
            .u
            .wrapping_add(mul_div(elapsed_y, end.u.wrapping_sub(start.u), height)),
        v: start
            .v
            .wrapping_add(mul_div(elapsed_y, end.v.wrapping_sub(start.v), height)),
        pnt5: start.pnt5.wrapping_add(mul_div(
            elapsed_y,
            end.pnt5.wrapping_sub(start.pnt5),
            height,
        )),
    }
}

fn interpolate_vertex_fixed(start: Mode5Vertex, end: Mode5Vertex, elapsed_y: i32) -> Mode5Vertex {
    let height = end.y.wrapping_sub(start.y);
    assert!(height > 0, "mode-5 interpolation edge must descend");
    Mode5Vertex {
        x: fixed_x(start.x).wrapping_add(x_slope(start, end).wrapping_mul(elapsed_y)),
        y: start.y.wrapping_add(elapsed_y),
        u: start.u.wrapping_add(
            end.u
                .wrapping_sub(start.u)
                .wrapping_div(height)
                .wrapping_mul(elapsed_y),
        ),
        v: start.v.wrapping_add(
            end.v
                .wrapping_sub(start.v)
                .wrapping_div(height)
                .wrapping_mul(elapsed_y),
        ),
        pnt5: start.pnt5.wrapping_add(
            end.pnt5
                .wrapping_sub(start.pnt5)
                .wrapping_div(height)
                .wrapping_mul(elapsed_y),
        ),
    }
}

fn x_slope(start: Mode5Vertex, end: Mode5Vertex) -> i32 {
    let height = end.y.wrapping_sub(start.y);
    assert!(height > 0, "mode-5 X edge must descend");
    end.x
        .wrapping_sub(start.x)
        .wrapping_shl(16)
        .wrapping_div(height)
}

fn fixed_x(x: i32) -> i32 {
    x.wrapping_shl(16)
}

fn mul_div(left: i32, right: i32, divisor: i32) -> i32 {
    let product = i64::from(left) * i64::from(right);
    (product / i64::from(divisor)) as i32
}

/// Rasterize one mode-5 scanline span into exact indexed samples.
///
/// Left clipping advances U, V and pnt5 by the clipped pixel count before the
/// first sample. Right clipping only shortens the exclusive endpoint.
///
/// # Panics
///
/// Panics unless texture and TABLESN each contain exactly 65,536 bytes or the
/// viewport width is negative.
#[must_use]
pub fn rasterize_mode5_span(
    span: Mode5Span,
    viewport_width: i32,
    texture: &[u8],
    tablesn: &[u8],
) -> Vec<Mode5Pixel> {
    assert_eq!(texture.len(), 256 * 256, "mode-5 texture must be 256x256");
    rasterize_mode5_span_from_atlas(span, viewport_width, texture, 0, tablesn)
}

/// Rasterize one mode-5 span through a source 256-stride texture-atlas
/// pointer.
#[must_use]
pub fn rasterize_mode5_span_from_atlas(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
) -> Vec<Mode5Pixel> {
    let mut pixels = Vec::new();
    visit_mode5_span_from_atlas(
        span,
        viewport_width,
        texture_atlas,
        texture_base,
        tablesn,
        Mode5TextureAddressing::Full256,
        |pixel| pixels.push(pixel),
    );
    pixels
}

#[cfg(test)]
pub(crate) fn rasterize_mode5_span_from_tile_atlas(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
) -> Vec<Mode5Pixel> {
    let mut pixels = Vec::new();
    visit_mode5_span_from_atlas(
        span,
        viewport_width,
        texture_atlas,
        texture_base,
        tablesn,
        Mode5TextureAddressing::Tile32,
        |pixel| pixels.push(pixel),
    );
    pixels
}

fn visit_mode5_span_from_atlas<F>(
    span: Mode5Span,
    viewport_width: i32,
    texture_atlas: &[u8],
    texture_base: usize,
    tablesn: &[u8],
    addressing: Mode5TextureAddressing,
    mut visitor: F,
) where
    F: FnMut(Mode5Pixel),
{
    assert!(
        texture_base < texture_atlas.len(),
        "mode-5 texture base must point inside its atlas"
    );
    assert_eq!(tablesn.len(), 256 * 256, "TABLESN must be 256x256");
    assert!(viewport_width >= 0, "viewport width must be non-negative");

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
            Mode5TextureAddressing::Full256 => raw_texture_address(u, v),
            Mode5TextureAddressing::Tile32 => {
                let Some(texture_address) = tile_texture_address(u, v) else {
                    break;
                };
                texture_address
            }
        };
        let atlas_address = texture_base
            .checked_add(usize::from(texture_address))
            .expect("mode-5 atlas address fits usize");
        let source_index = match addressing {
            Mode5TextureAddressing::Full256 => *texture_atlas
                .get(atlas_address)
                .expect("mode-5 stepped texture address remains inside its source atlas"),
            Mode5TextureAddressing::Tile32 => {
                let Some(source_index) = texture_atlas.get(atlas_address) else {
                    break;
                };
                *source_index
            }
        };
        let tablesn_address = (u16::from(fixed_byte(pnt5)) << 8) | u16::from(source_index);
        let output_index = tablesn[usize::from(tablesn_address)];
        visitor(Mode5Pixel {
            x,
            u,
            v,
            pnt5,
            texture_address,
            tablesn_address,
            source_index,
            output_index,
        });
        u = u.wrapping_add(span.du);
        v = v.wrapping_add(span.dv);
        pnt5 = pnt5.wrapping_add(span.dpnt5);
    }
}

const fn fixed_byte(value: i32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn banks() -> (Vec<u8>, Vec<u8>) {
        let mut texture = vec![0; 256 * 256];
        let mut tablesn = vec![0; 256 * 256];
        for address in 0..=u16::MAX {
            texture[usize::from(address)] = address.to_le_bytes()[0];
            let [source, row] = address.to_le_bytes();
            tablesn[usize::from(address)] = source.wrapping_add(row);
        }
        (texture, tablesn)
    }

    fn vertex(x: i32, y: i32, u: i32, v: i32, pnt5: i32) -> Mode5Vertex {
        Mode5Vertex { x, y, u, v, pnt5 }
    }

    #[test]
    fn ordinary_middle_right_triangle_matches_source_edge_records() {
        let top = vertex(2, 0, 0, 10 << 16, 20 << 16);
        let middle = vertex(8, 2, 8 << 16, 18 << 16, 32 << 16);
        let bottom = vertex(4, 4, 4 << 16, 14 << 16, 24 << 16);
        let setup = setup_mode5_triangle(top, middle, bottom, 200).unwrap();
        assert_eq!(
            (setup.du, setup.dv, setup.dpnt5),
            (1 << 16, 1 << 16, 109_226),
            "ordinary split width uses width plus one"
        );
        assert_eq!(
            setup
                .scanlines
                .iter()
                .map(|row| (row.y, row.left_x_16_16, row.right_x_16_16, row.left_u_16_16,))
                .collect::<Vec<_>>(),
            vec![
                (0, 2 << 16, 2 << 16, 0),
                (1, (2 << 16) + 0x8000, 5 << 16, 1 << 16),
                (2, 3 << 16, 8 << 16, 2 << 16),
                (3, (3 << 16) + 0x8000, 6 << 16, 3 << 16),
            ]
        );
    }

    #[test]
    fn middle_right_preserves_source_operand_orientation_at_int_min() {
        let top = vertex(0, 0, 0, 0, 0);
        let middle = vertex(2, 1, 0, 0, 0);
        let bottom = vertex(0, 2, i32::MIN, 0, 0);
        let setup = setup_mode5_triangle(top, middle, bottom, 8).unwrap();
        assert_eq!(setup.du, -357_913_941);
    }

    #[test]
    fn cyclic_vertex_orders_match_and_reversed_winding_is_rejected() {
        let top = vertex(2, 0, 0, 0, 0);
        let middle = vertex(8, 2, 8 << 16, 4 << 16, 2 << 16);
        let bottom = vertex(4, 4, 4 << 16, 8 << 16, 4 << 16);
        let expected = setup_mode5_triangle(top, middle, bottom, 200).unwrap();
        assert_eq!(
            setup_mode5_triangle(middle, bottom, top, 200),
            Some(expected.clone())
        );
        assert_eq!(
            setup_mode5_triangle(bottom, top, middle, 200),
            Some(expected)
        );
        assert_eq!(setup_mode5_triangle(top, bottom, middle, 200), None);
        assert_eq!(setup_mode5_triangle(middle, top, bottom, 200), None);
        assert_eq!(setup_mode5_triangle(bottom, middle, top, 200), None);
    }

    #[test]
    fn flat_top_and_bottom_use_exact_width_without_plus_one() {
        let left = vertex(1, 0, 0, 0, 0);
        let right = vertex(5, 0, 8 << 16, 12 << 16, 16 << 16);
        let bottom = vertex(3, 2, 4 << 16, 6 << 16, 8 << 16);
        let flat_top = setup_mode5_triangle(left, right, bottom, 200).unwrap();
        assert_eq!(
            (flat_top.du, flat_top.dv, flat_top.dpnt5),
            (2 << 16, 3 << 16, 4 << 16)
        );
        assert_eq!(
            flat_top
                .scanlines
                .iter()
                .map(|row| (row.y, row.left_x_16_16, row.right_x_16_16, row.left_u_16_16,))
                .collect::<Vec<_>>(),
            vec![(0, 1 << 16, 5 << 16, 0), (1, 2 << 16, 4 << 16, 2 << 16),]
        );

        let top = vertex(3, 0, 4 << 16, 6 << 16, 8 << 16);
        let bottom_left = vertex(1, 2, 0, 0, 0);
        let bottom_right = vertex(5, 2, 8 << 16, 12 << 16, 16 << 16);
        let flat_bottom = setup_mode5_triangle(top, bottom_right, bottom_left, 200).unwrap();
        assert_eq!(
            (flat_bottom.du, flat_bottom.dv, flat_bottom.dpnt5),
            (2 << 16, 3 << 16, 4 << 16)
        );
        assert_eq!(
            flat_bottom
                .scanlines
                .iter()
                .map(|row| (row.y, row.left_x_16_16, row.right_x_16_16, row.left_u_16_16,))
                .collect::<Vec<_>>(),
            vec![
                (0, 3 << 16, 3 << 16, 4 << 16),
                (1, 2 << 16, 4 << 16, 2 << 16),
            ]
        );
    }

    #[test]
    fn vertical_clipping_advances_edges_and_keeps_middle_reset() {
        let top = vertex(2, -2, 0, 0, 0);
        let middle = vertex(8, 1, 9 << 16, 12 << 16, 15 << 16);
        let bottom = vertex(4, 4, 6 << 16, 12 << 16, 18 << 16);
        let setup = setup_mode5_triangle(top, middle, bottom, 3).unwrap();
        assert_eq!(
            setup.scanlines.iter().map(|row| row.y).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(setup.scanlines[1].right_x_16_16, 8 << 16);
        assert_eq!(setup.scanlines[1].left_u_16_16, 3 << 16);
        assert_eq!(setup.scanlines[1].right_x_16_16, 8 << 16);
    }

    #[test]
    fn horizontal_and_fully_offscreen_degenerate_triangles_are_rejected() {
        let horizontal = [
            vertex(1, 4, 0, 0, 0),
            vertex(3, 4, 0, 0, 0),
            vertex(5, 4, 0, 0, 0),
        ];
        assert_eq!(
            setup_mode5_triangle(horizontal[0], horizontal[1], horizontal[2], 8),
            None
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(1, -8, 0, 0, 0),
                vertex(5, -6, 0, 0, 0),
                vertex(3, -4, 0, 0, 0),
                8,
            ),
            None
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(1, 8, 0, 0, 0),
                vertex(5, 10, 0, 0, 0),
                vertex(3, 12, 0, 0, 0),
                8,
            ),
            None
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(0, 0, 0, 0, 0),
                vertex(1, 100_000, 0, 0, 0),
                vertex(2, 200_001, 0, 0, 0),
                1,
            ),
            None,
            "distinct geometric edges that quantize to one 16.16 slope are rejected"
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(0, 0, 0, 0, 0),
                vertex(0, 0, 0, 0, 0),
                vertex(2, 2, 0, 0, 0),
                8,
            ),
            None
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(4, 0, 0, 0, 0),
                vertex(0, 0, 0, 0, 0),
                vertex(2, 2, 0, 0, 0),
                8,
            ),
            None,
            "reversed flat top is rejected"
        );
        assert_eq!(
            setup_mode5_triangle(
                vertex(2, 0, 0, 0, 0),
                vertex(0, 2, 0, 0, 0),
                vertex(4, 2, 0, 0, 0),
                8,
            ),
            None,
            "reversed flat bottom submission is rejected"
        );
    }

    #[test]
    fn middle_left_vector_switches_edge_slopes_at_the_split() {
        let top = vertex(4, 0, 15 << 16, 0, 15 << 16);
        let bottom = vertex(4, 4, 15 << 16, 0, 15 << 16);
        let middle = vertex(0, 2, 10 << 16, 0, 10 << 16);
        let setup = setup_mode5_triangle(top, bottom, middle, 8).unwrap();
        assert_eq!((setup.du, setup.dpnt5), (1 << 16, 1 << 16));
        assert_eq!(
            setup
                .scanlines
                .iter()
                .map(|row| (
                    row.y,
                    row.left_x_16_16 >> 16,
                    row.right_x_16_16 >> 16,
                    row.left_pnt5_16_16 >> 16,
                ))
                .collect::<Vec<_>>(),
            vec![(0, 4, 4, 15), (1, 2, 4, 12), (2, 0, 4, 10), (3, 2, 4, 12),]
        );
    }

    #[test]
    fn middle_left_retains_upper_attribute_division_remainder() {
        let top = vertex(4, 0, 0, 0, 0);
        let bottom = vertex(4, 4, 10, 10, 10);
        let middle = vertex(0, 2, 5, 5, 5);
        let setup = setup_mode5_triangle(top, bottom, middle, 8).unwrap();
        assert_eq!(
            setup
                .scanlines
                .iter()
                .map(|row| (
                    row.y,
                    row.left_x_16_16 >> 16,
                    row.left_u_16_16,
                    row.left_v_16_16,
                    row.left_pnt5_16_16,
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, 4, 0, 0, 0),
                (1, 2, 2, 2, 2),
                (2, 0, 4, 4, 4),
                (3, 2, 6, 6, 6),
            ],
            "only X resets to the exact middle; Q accumulators keep the lost unit"
        );
    }

    #[test]
    fn triangle_left_clip_advances_attributes_across_byte_wrap() {
        let (texture, tablesn) = banks();
        let pixels = rasterize_mode5_triangle(
            [
                vertex(-2, 0, 0x00fe_0000, 0, 0x00fe_0000),
                vertex(2, 0, 0x0102_0000, 0, 0x0102_0000),
                vertex(-2, 2, 0x00fe_0000, 0, 0x00fe_0000),
            ],
            4,
            4,
            &texture,
            &tablesn,
        );
        assert_eq!(
            pixels
                .iter()
                .map(|pixel| (
                    pixel.sample.x,
                    pixel.y,
                    fixed_byte(pixel.sample.u),
                    fixed_byte(pixel.sample.pnt5),
                ))
                .collect::<Vec<_>>(),
            vec![(0, 0, 0, 0), (1, 0, 1, 1)]
        );
    }

    #[test]
    fn nonnegative_left_branch_reads_right_high_word_as_unsigned() {
        let (texture, tablesn) = banks();
        let pixels = rasterize_mode5_triangle(
            [
                vertex(0, 0, 0, 0, 0),
                vertex(32_768, 0, 0, 0, 0),
                vertex(0, 1, 0, 0, 0),
            ],
            320,
            2,
            &texture,
            &tablesn,
        );
        assert_eq!(pixels.len(), 320);
        assert_eq!((pixels[0].sample.x, pixels[0].y), (0, 0));
        assert_eq!((pixels[319].sample.x, pixels[319].y), (319, 0));
    }

    #[test]
    fn vertical_and_right_clipping_preserve_source_coverage() {
        let (texture, tablesn) = banks();
        let vertical = rasterize_mode5_triangle(
            [
                vertex(0, -2, 0, 0, 0),
                vertex(4, -2, 0, 0, 0),
                vertex(0, 2, 0, 0, 0),
            ],
            4,
            2,
            &texture,
            &tablesn,
        );
        assert_eq!(
            vertical
                .iter()
                .map(|pixel| (pixel.sample.x, pixel.y))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0), (0, 1)]
        );

        let right = rasterize_mode5_triangle(
            [
                vertex(0, 0, 0, 0, 0),
                vertex(6, 0, 0, 0, 0),
                vertex(0, 2, 0, 0, 0),
            ],
            4,
            4,
            &texture,
            &tablesn,
        );
        assert_eq!(
            right
                .iter()
                .map(|pixel| (pixel.sample.x, pixel.y))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (1, 1), (2, 1),]
        );
    }

    #[test]
    fn both_checkerboard_diagonals_cover_a_quad_once_without_a_seam() {
        use std::collections::BTreeMap;

        let (texture, tablesn) = banks();
        let p0 = vertex(0, 0, 0, 0, 0);
        let p6 = vertex(4, 0, 0, 0, 0);
        let p12 = vertex(4, 4, 0, 0, 0);
        let p18 = vertex(0, 4, 0, 0, 0);
        let parity_one = [[p18, p0, p12], [p0, p6, p12]];
        let parity_zero = [[p18, p6, p12], [p18, p0, p6]];

        for triangles in [parity_one, parity_zero] {
            let mut coverage = BTreeMap::new();
            for triangle in triangles {
                for pixel in rasterize_mode5_triangle(triangle, 4, 4, &texture, &tablesn) {
                    *coverage.entry((pixel.sample.x, pixel.y)).or_insert(0_u8) += 1;
                }
            }
            assert_eq!(coverage.len(), 16);
            assert!(coverage.values().all(|count| *count == 1));
        }
    }

    #[test]
    fn increment_uses_width_plus_one_and_signed_truncation() {
        assert_eq!(mode5_horizontal_increment(0, 65_536, 3), 16_384);
        assert_eq!(mode5_horizontal_increment(0, -65_535, 3), -16_383);
        assert_eq!(
            mode5_horizontal_increment(i32::MAX, i32::MIN, 1),
            0,
            "source subtraction wraps before division"
        );
        assert_eq!(
            mode5_horizontal_increment(0, i32::MIN, i32::MAX),
            1,
            "source width-plus-one and attribute subtraction both wrap"
        );
        assert!(!signed_add_is_negative(i32::MIN, true));
        assert!(signed_add_is_negative(i32::MAX, true));
    }

    #[test]
    fn left_clip_advances_all_attributes_before_the_first_sample() {
        let (texture, tablesn) = banks();
        let pixels = rasterize_mode5_span(
            Mode5Span {
                left_x: -2,
                right_x: 3,
                left_u: 0,
                left_v: 4 << 16,
                left_pnt5: 10 << 16,
                du: 1 << 16,
                dv: 2 << 16,
                dpnt5: 1 << 16,
            },
            8,
            &texture,
            &tablesn,
        );
        assert_eq!(pixels.len(), 3);
        assert_eq!(
            pixels
                .iter()
                .map(|pixel| (
                    pixel.x,
                    pixel.texture_address,
                    pixel.tablesn_address,
                    pixel.output_index,
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, 0x0802, 0x0c02, 14),
                (1, 0x0a03, 0x0d03, 16),
                (2, 0x0c04, 0x0e04, 18),
            ]
        );
    }

    #[test]
    fn tile_atlas_addressing_aborts_span_before_out_of_range_dereference() {
        let mut atlas = vec![0; 152 * 32 * 32];
        let base = (151 / 8) * 32 * 256 + (151 % 8) * 32;
        atlas[base + 0x0010] = 17;
        let (_, tablesn) = banks();

        let pixels = rasterize_mode5_span_from_tile_atlas(
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
        );
        assert_eq!(pixels.len(), 1);
        assert_eq!(pixels[0].texture_address, 0x0010);
        assert_eq!(pixels[0].source_index, 17);
        assert_eq!(tile_texture_address(16 << 16, 0), Some(0x0010));
        assert_eq!(tile_texture_address(16 << 16, -(1 << 16)), None);
    }

    #[test]
    fn right_clip_is_exclusive_and_does_not_change_initial_attributes() {
        let (texture, tablesn) = banks();
        let pixels = rasterize_mode5_span(
            Mode5Span {
                left_x: 6,
                right_x: 10,
                left_u: 7 << 16,
                left_v: 9 << 16,
                left_pnt5: 11 << 16,
                du: 1 << 16,
                dv: 0,
                dpnt5: 1 << 16,
            },
            8,
            &texture,
            &tablesn,
        );
        assert_eq!(pixels.len(), 2);
        assert_eq!((pixels[0].x, pixels[0].texture_address), (6, 0x0907));
        assert_eq!((pixels[1].x, pixels[1].texture_address), (7, 0x0908));
    }

    #[test]
    fn wrapped_16_16_steps_cross_texture_and_tablesn_byte_edges() {
        let (texture, tablesn) = banks();
        let pixels = rasterize_mode5_span(
            Mode5Span {
                left_x: 0,
                right_x: 3,
                left_u: 0x00ff_0000,
                left_v: 0x00fe_0000,
                left_pnt5: 0x00ff_0000,
                du: 0x0001_0000,
                dv: 0x0001_0000,
                dpnt5: 0x0001_0000,
            },
            3,
            &texture,
            &tablesn,
        );
        assert_eq!(
            pixels
                .iter()
                .map(|pixel| (pixel.texture_address, pixel.tablesn_address))
                .collect::<Vec<_>>(),
            vec![(0xfeff, 0xffff), (0xff00, 0x0000), (0x0001, 0x0101)]
        );
    }

    #[test]
    fn fractional_carry_and_borrow_change_address_bytes_after_sampling() {
        let (texture, tablesn) = banks();
        for (start, step, expected) in [
            (0x0010_fff0, 0x20, vec![0x10, 0x11, 0x11, 0x11]),
            (0x0011_0010, -0x20, vec![0x11, 0x10, 0x10, 0x10]),
        ] {
            let pixels = rasterize_mode5_span(
                Mode5Span {
                    left_x: 0,
                    right_x: 4,
                    left_u: start,
                    left_v: 0,
                    left_pnt5: start,
                    du: step,
                    dv: 0,
                    dpnt5: step,
                },
                4,
                &texture,
                &tablesn,
            );
            assert_eq!(
                pixels
                    .iter()
                    .map(|pixel| fixed_byte(pixel.u))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                pixels
                    .iter()
                    .map(|pixel| fixed_byte(pixel.pnt5))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn empty_and_fully_clipped_spans_write_no_pixels() {
        let (texture, tablesn) = banks();
        for (left_x, right_x) in [(4, 4), (5, 2), (-8, -1), (8, 12)] {
            assert!(
                rasterize_mode5_span(
                    Mode5Span {
                        left_x,
                        right_x,
                        ..Mode5Span::default()
                    },
                    8,
                    &texture,
                    &tablesn,
                )
                .is_empty()
            );
        }
    }
}
