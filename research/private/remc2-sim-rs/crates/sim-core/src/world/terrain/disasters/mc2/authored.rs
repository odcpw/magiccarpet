//! Authored terrain chains retain their real predecessor/successor traversal.
//! The caller supplies the mutable mission rows and authoritative scratch Z;
//! descendants enter the ordinary arena and numeric scheduler immediately.

use std::collections::BTreeSet;

use crate::{
    Position, campaign::source_arena::SourceArena, legacy_atan, legacy_sqrt,
    level1_mission::AuthoredFeature, native_world::SessionTerrain,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Prepared {
    pub(crate) source_slots: Vec<u16>,
    pub(crate) consumed_rows: Vec<u16>,
}

fn chain(features: &[AuthoredFeature], mut index: u16) -> Result<Vec<AuthoredFeature>, String> {
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(index) {
            return Err("terrain chain predecessor cycle".into());
        }
        let row = features
            .get(usize::from(index))
            .ok_or("terrain predecessor is absent")?;
        if row.parameter_1 == 0 {
            break;
        }
        index = row.parameter_1;
    }
    seen.clear();
    let mut rows = Vec::new();
    loop {
        if !seen.insert(index) {
            return Err("terrain chain successor cycle".into());
        }
        let row = *features
            .get(usize::from(index))
            .ok_or("terrain successor is absent")?;
        rows.push(row);
        if row.parameter_2 == 0 {
            return Ok(rows);
        }
        index = row.parameter_2;
    }
}

pub(crate) fn prepare_chain(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    features: &mut [AuthoredFeature],
    index: u16,
    retained_scratch_z: i16,
) -> Result<Prepared, String> {
    let producer = *features
        .get(usize::from(index))
        .ok_or("terrain producer row is absent")?;
    if producer.kind != 10 || !matches!(producer.subtype, 28 | 29 | 31 | 50) {
        return Err("authored terrain constructor is not a linked chain".into());
    }
    if producer.stage_tag == 0 {
        return Ok(Prepared::default());
    }
    let rows = chain(features, index)?;
    if producer.subtype == 28 {
        return Ok(Prepared {
            source_slots: super::ribbon::prepare_authored(arena, terrain, features, index)?,
            consumed_rows: rows.iter().map(|row| row.index).collect(),
        });
    }
    let mut result = Prepared::default();
    for (ordinal, row) in rows.iter().enumerate() {
        features[usize::from(row.index)].stage_tag = 0;
        result.consumed_rows.push(row.index);
        let Some(next) = rows.get(ordinal + 1) else {
            break;
        };
        if producer.subtype == 29 {
            for segment in
                super::super::paths::segments(row.tile_x, row.tile_y, next.tile_x, next.tile_y)
            {
                if let Some(slot) =
                    super::paths::construct(arena, terrain, segment, retained_scratch_z)?
                {
                    result.source_slots.push(slot);
                }
            }
            continue;
        }
        let x = row.tile_x << 8;
        let y = row.tile_y << 8;
        let end_x = next.tile_x << 8;
        let end_y = next.tile_y << 8;
        let dx = end_x.wrapping_sub(x) as i16;
        let dy = end_y.wrapping_sub(y) as i16;
        let distance = legacy_sqrt(
            (i32::from(dx) * i32::from(dx)) as u32 + (i32::from(dy) * i32::from(dy)) as u32,
        ) as u16;
        let yaw = legacy_atan(dx, dy);
        let height = terrain.planes().ground_heights
            [usize::from(row.tile_x as u8) + 256 * usize::from(row.tile_y as u8)];
        let (kind, height_scale) = if producer.subtype == 31 {
            (super::travelling::Kind::CraterTrail, 32)
        } else {
            (super::travelling::Kind::RisingTrail, 16)
        };
        let position = Position {
            x,
            y,
            z: i16::from(height) * height_scale,
        };
        if let Some(slot) = super::travelling::construct(arena, terrain, position, kind) {
            let source = &mut arena.entities[usize::from(slot)];
            source.yaw = yaw;
            source.record[32..34].copy_from_slice(&yaw.to_le_bytes());
            if producer.subtype == 31 {
                source.set_life(i32::from(distance >> 8));
                source.record[74] = match row.parameter_3 {
                    0 => 2,
                    1 => 6,
                    2 => 16,
                    3 => 32,
                    value => value as u8,
                };
            } else {
                source.set_life(i32::from(distance) / i32::from(source.actual_speed));
            }
            result.source_slots.push(slot);
        }
    }
    Ok(result)
}
