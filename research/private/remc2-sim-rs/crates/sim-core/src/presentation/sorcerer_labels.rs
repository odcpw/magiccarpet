//! Indexed DrawSorcererNameAndHealthBar at the primary sprite boundary.

use crate::{
    classic_hd::ClassicHdViewport,
    game_runtime::preferences::installed_locale::SourceFont,
    goat_sprite_raster::Mode20SpritePlacement,
    indexed_overlay::draw_selected_source_font,
    native_world::{CampaignPlayerLabel, MapType, campaign_hud_palette_indices},
    SIN_16_16,
};

/// Source coordinates are the low-resolution DrawWorld projection. The
/// labels use the separate doubled UI coordinate convention before clipping.
pub(super) fn draw(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    viewport: ClassicHdViewport,
    placement: Mode20SpritePlacement,
    roll: i16,
    map_type: MapType,
    empty_bar: u8,
    font: &SourceFont,
    label: CampaignPlayerLabel,
) -> Result<(), String> {
    if pixels.len() != width.checked_mul(height).ok_or("label target extent overflow")?
        || u64::from(viewport.x) + u64::from(viewport.width) > width as u64
        || u64::from(viewport.y) + u64::from(viewport.height) > height as u64
        || usize::from(label.name_len) >= label.name_bytes.len()
    {
        return Err("sorcerer label has an invalid target or name".into());
    }
    let (background, light, dark) = match map_type {
        MapType::Day => (0xaa, 0x63, 0x0d),
        MapType::Night => (0x33, 0x11, 0x3b),
        MapType::Cave => (0x33, 0x88, 0x3b),
    };
    let owner = campaign_hud_palette_indices(label.player_color_index, map_type)
        .map_err(|error| error.to_string())?[0];
    let angle = usize::from((roll as u16) & 2047);
    let sin = SIN_16_16[angle];
    let cos = SIN_16_16[512 + angle];
    let sprite_x = placement.bottom_x.wrapping_sub(
        ((cos.wrapping_mul(placement.projected_width) >> 1)
            .wrapping_add(sin.wrapping_mul(placement.projected_height))) >> 16);
    let sprite_y = placement.bottom_y.wrapping_sub(
        (cos.wrapping_mul(placement.projected_height)
            .wrapping_sub(sin.wrapping_mul(placement.projected_width) >> 1)) >> 16);
    let view_x = viewport.x as i32;
    let view_y = viewport.y as i32;
    let right = view_x + viewport.width as i32 - 4;
    let bottom = view_y + viewport.height as i32 - 22;
    // DrawSprite's screen coordinates are local to its viewport buffer. This
    // port's placement is target-relative, so remove the origin first.
    let center = (sprite_x - view_x).wrapping_add(placement.projected_width >> 1) as i16;
    let x = view_x + i32::from(center.wrapping_mul(2));
    let y = view_y + (sprite_y - view_y).wrapping_mul(2) - 20;
    if x < view_x || (y as i16) < view_y as i16 || x >= right || (y as i16) >= bottom as i16 {
        return Ok(());
    }
    let mut count = usize::from(label.name_len);
    let mut box_width = 8 * count as i32 + 4;
    if x + box_width > right {
        box_width = right - x;
        count = usize::try_from((box_width - 4) / 8).unwrap_or(0);
    }
    if count == 0 { return Ok(()); }
    let mut rect = |x: i32, y: i32, w: i32, h: i32, color: u8| {
        if w <= 0 || h <= 0 { return; }
        // DrawLineLowRes receives source UI coordinates and halves each
        // position/extent before writing to the 320x200 indexed target.
        let x0 = x >> 1;
        let y0 = y >> 1;
        let x1 = x0 + (w >> 1);
        let y1 = y0 + (h >> 1);
        let left = x0.max(0);
        let top = y0.max(0);
        let right = x1.min(width as i32);
        let bottom = y1.min(height as i32);
        for yy in top..bottom {
            for xx in left..right { pixels[yy as usize * width + xx as usize] = color; }
        }
    };
    rect(x, y, box_width + 2, 18, background);
    rect(x, y, box_width + 2, 2, light);
    rect(x, y + 16, box_width + 2, 2, dark);
    rect(x, y, 2, 16, light);
    rect(x + box_width, y, 2, 18, dark);
    drop(rect);
    draw_selected_source_font(font, x + 4, y, &label.name_bytes[..count], |px, py| {
        if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 { return false; }
        pixels[py as usize * width + px as usize] = owner;
        true
    }).map_err(|error| error.to_string())?;
    let mut rect = |x: i32, y: i32, w: i32, color: u8| {
        if w <= 0 { return; }
        let yy = y >> 1;
        let start = (x >> 1).max(0);
        let end = ((x >> 1) + (w >> 1)).min(width as i32);
        if yy >= 0 && yy < height as i32 {
            for xx in start..end { pixels[yy as usize * width + xx as usize] = color; }
        }
    };
    rect(x + 2, y + 14, box_width - 2, empty_bar);
    if label.max_life != 0 {
        let life_width = label.life.wrapping_mul(box_width - 2)
            .checked_div(label.max_life).ok_or("sorcerer health division overflow")?;
        let life_width = life_width.min(right - 2 - (x + 2));
        if life_width > 0 { rect(x + 2, y + 14, life_width, owner); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_runtime::preferences::installed_locale::SourceGlyph;

    fn font() -> SourceFont {
        let glyphs = (0..273).map(|index| SourceGlyph {
            index, offset: index * 24, length: 24, width: 4, height: 6, sha256: String::new(),
        }).collect::<Vec<_>>();
        SourceFont { glyphs: glyphs.into_boxed_slice(), masks: vec![1; 273 * 24].into_boxed_slice() }
    }

    fn label(life: i32, max_life: i32) -> CampaignPlayerLabel {
        let mut name_bytes = [0; 32];
        name_bytes[..6].copy_from_slice(b"Nyphur");
        CampaignPlayerLabel { player_color_index: 1, name_bytes, name_len: 6, life, max_life }
    }

    fn placement(x: i32, y: i32) -> Mode20SpritePlacement {
        Mode20SpritePlacement {
            selector: 1, orientation_sector: 0, mirrored: false, lateral: 0,
            forward_depth: 1, distance_squared: 0, bottom_x: x, bottom_y: y,
            projected_width: 20, projected_height: 20, brightness: 0x2000,
            quarter_sum_anchor: false, shadow_quarter_sum_anchor: false,
        }
    }

    #[test]
    fn source_label_clips_name_and_preserves_signed_life_bar() {
        let font = font();
        let viewport = ClassicHdViewport::full(320, 200);
        let mut pixels = vec![9; 320 * 200];
        draw(&mut pixels, 320, 200, viewport, placement(30, 60), 0, MapType::Night,
            5, &font, label(50, 100)).unwrap();
        assert_eq!(pixels[30 * 320 + 30], 0x11); // top/left border
        assert_eq!(pixels[30 * 320 + 32], 0x77); // actual owner's glyph
        assert_eq!(pixels[37 * 320 + 31], 0x77);
        assert_eq!(pixels[37 * 320 + 42], 0x77);
        assert_eq!(pixels[37 * 320 + 43], 5);
        assert_eq!(pixels[38 * 320 + 30], 0x3b); // lower border
        let mut empty = vec![9; 320 * 200];
        draw(&mut empty, 320, 200, viewport, placement(30, 60), 0, MapType::Night,
            5, &font, label(-1, 100)).unwrap();
        assert_eq!(empty[37 * 320 + 31], 5);
        let mut zero = vec![9; 320 * 200];
        draw(&mut zero, 320, 200, viewport, placement(30, 60), 0, MapType::Night,
            5, &font, label(50, 0)).unwrap();
        assert_eq!(zero[37 * 320 + 31], 5);
        let mut edge = vec![9; 320 * 200];
        draw(&mut edge, 320, 200, viewport, placement(155, 60), 0, MapType::Night,
            5, &font, label(50, 100)).unwrap();
        assert_eq!(edge, vec![9; 320 * 200], "fewer than one source character fits");
        for size in [39, 17] {
            let view = crate::terrain_frame::source_viewport(size).unwrap();
            let mut inset = vec![9; 320 * 200];
            let actor = placement(view.x as i32 + 1, view.y as i32 + 40);
            draw(&mut inset, 320, 200, view, actor, 0, MapType::Night,
                5, &font, label(50, 100)).unwrap();
            // Source adds the physical-size-derived viewport origin BEFORE
            // the low-res writer halves UI coordinates.
            let x = ((view.x + 2) / 2) as usize;
            let y = ((view.y + 20) / 2) as usize;
            assert_eq!(inset[y * 320 + x], 0x11, "size {size} uses source origin");
            assert!(x < view.x as usize, "control reaches the inset surround");
        }
    }
}
