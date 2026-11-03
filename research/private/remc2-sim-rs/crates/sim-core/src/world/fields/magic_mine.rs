//! Model78 Magic Mine producer, capture collision, and scheduled target/effect path.
//!
//! Mutable mine state stays in its source entity row. The shared scheduler owns
//! dispatch cadence and following-pass reclaim; this owner constructs and
//! updates the row while returning ordinary projectile and pickup requests.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    renderable_map_chain::RenderableMapSeed,
    spells::late::magic_mine::SOURCE_RULES,
};

pub(crate) const MODEL: u8 = 78;
const CLASS: u8 = 10;
const PARTICLE: u16 = 66;
const UNCARGED: u16 = u16::MAX;

/// sub_50840. This is a real map-linked source row, not an auxiliary mine
/// table. Its default subspell profile is tier zero, matching the constructor.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    particle: ParticleParameter,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    source.class = CLASS;
    source.model = MODEL;
    source.record[67] = CLASS;
    source.record[68] = MODEL;
    source.set_source_action(85);
    source.set_control_flags(source.control_flags | 8);
    source.set_subspell_index(0);
    source.set_max_life(1_000);
    source.set_life(1_000);
    source.level_gate = 0;
    source.set_source_byte_46(0);
    source.font_type_index = 1;
    source.record[65] = 1;
    source.record[71] = 1;
    source.record[72] = 0;
    source.record[54..56].copy_from_slice(&slot.to_le_bytes());
    source.record[58..60].copy_from_slice(&UNCARGED.to_le_bytes());
    source.record[158..160].copy_from_slice(&position.x.to_le_bytes());
    source.record[160..162].copy_from_slice(&position.y.to_le_bytes());
    source.record[162..164].copy_from_slice(&position.z.to_le_bytes());
    if let Err(error) =
        crate::world::terrain::cave_features::bind_particle(&mut source, PARTICLE, particle)
    {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    crate::world::terrain::cave_features::synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Capture {
    pub mine_slot: u16,
    pub holder_slot: u16,
    pub mana_sphere_position: Position,
    pub sound_entity: u16,
    pub sound: u16,
}

/// The two source words written after the capture visual's NewEvent. The
/// adapter resolves the current holder first; this owns the mine payload.
pub(crate) fn store_captured_spell(mine: &mut EntitySnapshot, family: u8, tier: u8) {
    mine.record[58..60].copy_from_slice(&u16::from(family).to_le_bytes());
    mine.record[56..58].copy_from_slice(&u16::from(tier).to_le_bytes());
}

/// sub_68AC0. The ordinary collision caller resolves the flyer's holder link
/// before calling this. A successful capture consumes the holder immediately
/// and leaves its family/tier in the mine's source words for the field handler.
pub(crate) fn capture(
    rows: &mut [EntitySnapshot],
    mine_slot: u16,
    flyer_slot: u16,
) -> Option<Capture> {
    let flyer = rows.get(usize::from(flyer_slot))?.clone();
    let captured_holder = flyer.word_26();
    let holder_row = rows.get(usize::from(captured_holder))?;
    let holder_index = holder_row.index;
    let holder_model = holder_row.model;
    let holder_tier = holder_row.level_gate as u8;
    let mine = rows.get(usize::from(mine_slot))?;
    if mine.class != CLASS
        || mine.model != MODEL
        || mine.word_32() != flyer.source_id
        || u16::from_le_bytes([mine.record[58], mine.record[59]]) != UNCARGED
        || holder_index != captured_holder
    {
        return None;
    }
    let mine = &mut rows[usize::from(mine_slot)];
    store_captured_spell(mine, holder_model, holder_tier);
    let holder = &mut rows[usize::from(captured_holder)];
    holder.active_value = 1;
    holder.record[50..52].copy_from_slice(&1_u16.to_le_bytes());
    Some(Capture {
        mine_slot,
        holder_slot: captured_holder,
        mana_sphere_position: flyer.position,
        sound_entity: flyer.index,
        sound: 26,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CapturedSpell {
    pub family: u8,
    pub tier: u8,
    pub damage: u16,
    pub font_type: u8,
    pub life: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectileRequest {
    pub owner_slot: u16,
    pub target_slot: u16,
    pub target_position: Position,
    pub family: u8,
    pub tier: u8,
    pub damage: u16,
    pub yaw_offset: i16,
    pub altitude_offset: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerrainSample {
    pub ground_height: i16,
    pub cave_ceiling: Option<i16>,
    pub water: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManaDrop {
    pub position: Position,
    /// Source calls model5 for water and model0 for land.
    pub model: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub projectiles: Vec<ProjectileRequest>,
    pub spell_experience: Option<(u16, u8, u8)>,
    pub mana_drop: Option<ManaDrop>,
    pub hide: bool,
    pub charge_finished: bool,
}

/// Select the closest class3/model0-or-1 map entry, preserving the source map
/// order for equal distances. The caller supplies that pass's map-chain order.
pub(crate) fn nearest_target(
    mine: &EntitySnapshot,
    owner_slot: u16,
    rows_by_slot: &[EntitySnapshot],
    map_order: &[u16],
) -> Option<u16> {
    let mut nearest = None;
    let mut nearest_distance = 3_584_u32;
    for &slot in map_order {
        if slot == 0 || slot == owner_slot {
            continue;
        }
        let Some(target) = rows_by_slot.get(usize::from(slot)) else {
            continue;
        };
        if target.index != slot || target.class != 3 || target.model > 1 {
            continue;
        }
        let distance = crate::distance_3d_wrapped(mine.position, target.position);
        if distance < nearest_distance {
            nearest = Some(slot);
            nearest_distance = distance;
        }
    }
    nearest
}

/// One source sub_3A8B0 callback. `rows_by_slot` is a short-lived source-row
/// view for target/owner resolution; `map_order` is the current class3 map
/// chain. Projectile requests must be passed to the ordinary spell constructor
/// and followed by `complete_projectiles` with its real allocation results.
pub(crate) fn visit(
    rows: &mut [EntitySnapshot],
    mine_slot: u16,
    map_order: &[u16],
    captured: Option<CapturedSpell>,
    terrain: TerrainSample,
) -> Result<Visit, String> {
    let Some(mine) = rows.get(usize::from(mine_slot)) else {
        return Err("Magic Mine slot is outside the source arena".to_owned());
    };
    if mine.class != CLASS || mine.model != MODEL {
        return Err("Magic Mine visit needs a class10/model78 source row".to_owned());
    }
    let mut result = Visit::default();
    let owner_slot = mine.word_32();
    let Some(owner) = rows.get(usize::from(owner_slot)) else {
        rows[usize::from(mine_slot)].disable_drawing();
        result.hide = true;
        return Ok(result);
    };
    if owner.index != owner_slot || owner.life < 0 || owner.drawing_disabled() {
        rows[usize::from(mine_slot)].disable_drawing();
        result.hide = true;
        return Ok(result);
    }

    // Source ground/cave clamps happen before the phase switch.
    let mine = &mut rows[usize::from(mine_slot)];
    let mut position = mine.position;
    if position.z < terrain.ground_height {
        position.z = terrain.ground_height;
    }
    if let Some(ceiling) = terrain.cave_ceiling {
        let fov = i16::from_le_bytes([mine.record[92], mine.record[93]]);
        let maximum = ceiling.wrapping_sub(fov);
        if position.z > maximum {
            position.z = maximum;
        }
    }
    mine.set_position(position);

    let old_phase = mine.record[74];
    if old_phase != 9 && old_phase != 7 {
        let life = mine.life.wrapping_sub(1);
        mine.set_life(life);
        if life <= 0 {
            mine.record[74] = 6;
        }
    }

    let phase = rows[usize::from(mine_slot)].record[74];
    match phase {
        0 => {
            let mine = &mut rows[usize::from(mine_slot)];
            let rule = SOURCE_RULES
                .get(usize::from(mine.subspell_index))
                .ok_or_else(|| {
                    "Magic Mine constructor tier is outside the source table".to_owned()
                })?;
            mine.set_max_life(i32::from(rule.damage));
            mine.set_life(i32::from(rule.damage));
            mine.record[158..160].copy_from_slice(&mine.position.x.to_le_bytes());
            mine.record[160..162].copy_from_slice(&mine.position.y.to_le_bytes());
            mine.record[162..164].copy_from_slice(&mine.position.z.to_le_bytes());
            mine.record[71] = rule.repeat_count;
            mine.record[74] = 1;
        }
        1 => {
            let mine = &mut rows[usize::from(mine_slot)];
            if u16::from_le_bytes([mine.record[58], mine.record[59]]) != UNCARGED {
                mine.record[74] = 2;
            }
        }
        2 => {
            let mine = &mut rows[usize::from(mine_slot)];
            let family = u16::from_le_bytes([mine.record[58], mine.record[59]]) as u8;
            let tier = u16::from_le_bytes([mine.record[56], mine.record[57]]) as u8;
            let spell = captured
                .filter(|spell| spell.family == family && spell.tier == tier)
                .ok_or_else(|| {
                    "charged Magic Mine has no matching captured spell profile".to_owned()
                })?;
            mine.source_id = owner_slot;
            mine.record[30..32].copy_from_slice(&owner_slot.to_le_bytes());
            mine.font_type_index = if spell.font_type & 1 != 0 { 6 } else { 1 };
            mine.record[65] = mine.font_type_index;
            mine.set_entity_rng(next_rng(mine.entity_rng));
            mine.set_source_dword_10(i32::from(mine.entity_rng % 50) + 16);
            mine.record[74] = 3;
        }
        3 => {
            let mine = &mut rows[usize::from(mine_slot)];
            let timer = mine.source_dword_10().wrapping_sub(1);
            mine.set_source_dword_10(timer);
            if timer == 0 {
                mine.record[74] = 4;
            }
        }
        4 => {
            let should_scan = rows[usize::from(mine_slot)].phase & 0x0f == 0;
            let target = should_scan
                .then(|| nearest_target(&rows[usize::from(mine_slot)], owner_slot, rows, map_order))
                .flatten();
            if let Some(target) = target {
                let mine = &mut rows[usize::from(mine_slot)];
                mine.record[154..156].copy_from_slice(&target.to_le_bytes());
                mine.record[74] = 5;
            }
        }
        5 => {
            let target_slot = rows[usize::from(mine_slot)].source_word_96();
            let target = rows
                .get(usize::from(target_slot))
                .filter(|target| {
                    target_slot != 0
                        && target.index == target_slot
                        && target.life >= 0
                        && !target.drawing_disabled()
                })
                .map(|target| target.position);
            let Some(target_position) = target else {
                finish_charge(&mut rows[usize::from(mine_slot)]);
                result.charge_finished = true;
                return Ok(result);
            };
            let mine = &mut rows[usize::from(mine_slot)];
            let family = u16::from_le_bytes([mine.record[58], mine.record[59]]) as u8;
            let tier = u16::from_le_bytes([mine.record[56], mine.record[57]]) as u8;
            let spell = captured
                .filter(|spell| spell.family == family && spell.tier == tier)
                .ok_or_else(|| {
                    "active Magic Mine has no matching captured spell profile".to_owned()
                })?;
            result.spell_experience = Some((owner_slot, 23, 1));
            let fanout = if spell.family == 7 && spell.life == 2 {
                2
            } else {
                1
            };
            for index in 0..fanout {
                result.projectiles.push(ProjectileRequest {
                    owner_slot,
                    target_slot,
                    target_position,
                    family: spell.family,
                    tier: spell.tier,
                    damage: spell.damage,
                    yaw_offset: if fanout == 2 {
                        if index == 0 { 113 } else { -113 }
                    } else {
                        0
                    },
                    altitude_offset: i16::from_le_bytes([mine.record[86], mine.record[87]]),
                });
            }
        }
        6 => {
            let mine = &mut rows[usize::from(mine_slot)];
            mine.set_control_flags(mine.control_flags & !0x0080_0001);
            if mine.record[72] == 0 {
                mine.record[74] = 7;
                mine.set_source_dword_10(10);
            }
        }
        7 => {
            let mine = &mut rows[usize::from(mine_slot)];
            let timer = mine.source_dword_10().wrapping_sub(1);
            mine.set_source_dword_10(timer);
            if timer == 0 {
                mine.record[74] = 9;
                mine.set_source_dword_10(3);
            }
        }
        8 => {
            let mine = &mut rows[usize::from(mine_slot)];
            mine.record[74] = 9;
            mine.set_source_dword_10(3);
            mine.set_control_flags(mine.control_flags & !0x0080_0001);
        }
        9 => {
            let mine = &mut rows[usize::from(mine_slot)];
            let ticks = mine.source_dword_10().wrapping_add(1);
            mine.set_source_dword_10(ticks);
            let descent = ticks.wrapping_mul(32) as i16;
            let mut position = mine.position;
            position.z = position.z.wrapping_sub(descent);
            if position.z >= terrain.ground_height {
                mine.set_position(position);
            } else {
                position.z = terrain.ground_height;
                mine.set_position(position);
                result.mana_drop = Some(ManaDrop {
                    position,
                    model: if terrain.water { 5 } else { 0 },
                });
                mine.disable_drawing();
                result.hide = true;
            }
        }
        _ => {}
    }
    Ok(result)
}

/// Apply the ordinary constructor outcomes from one active-target visit. A
/// failed allocation leaves the target and charge in place for the next pass.
pub(crate) fn complete_projectiles(
    rows: &mut [EntitySnapshot],
    mine_slot: u16,
    projectile_yaws: &[Option<i16>],
) -> Result<bool, String> {
    let Some(mine) = rows.get(usize::from(mine_slot)) else {
        return Err("Magic Mine slot is outside the source arena".to_owned());
    };
    if mine.class != CLASS || mine.model != MODEL || mine.record[74] != 5 {
        return Err("Magic Mine projectile completion is outside phase five".to_owned());
    }
    let mine = &mut rows[usize::from(mine_slot)];
    let mut charge_finished = false;
    for &yaw in projectile_yaws {
        let Some(yaw) = yaw else {
            continue;
        };
        let raw_yaw = yaw.cast_unsigned();
        let source_yaw = (((raw_yaw >> 8).wrapping_add(4) & 7) << 8) | (raw_yaw & 0xff);
        mine.set_yaw(source_yaw as i16);
        mine.font_type_index = mine.font_type_index.wrapping_sub(1);
        mine.record[65] = mine.font_type_index;
        charge_finished |= mine.font_type_index == 0;
    }
    if charge_finished {
        finish_charge(mine);
    }
    Ok(charge_finished)
}

fn finish_charge(mine: &mut EntitySnapshot) {
    mine.record[154..156].fill(0);
    mine.record[71] = mine.record[71].wrapping_sub(1);
    mine.record[74] = if mine.record[71] != 0 { 2 } else { 6 };
}

fn next_rng(rng: u16) -> u16 {
    (u32::from(rng).wrapping_mul(9_377).wrapping_add(9_439)) as u16
}

#[cfg(test)]
mod tests {
    use super::{CapturedSpell, TerrainSample, capture, complete_projectiles, visit};
    use crate::{Position, native_world::EntitySnapshot};

    fn row(index: u16, class: u8, model: u8, position: Position) -> EntitySnapshot {
        let mut row = crate::world::terrain::cave_features::new_event(index, 17, position);
        row.class = class;
        row.model = model;
        row.record[67] = class;
        row.record[68] = model;
        row.active = true;
        row.set_life(100);
        row
    }

    fn mine() -> EntitySnapshot {
        let mut mine = row(2, 10, 78, Position { x: 0, y: 0, z: 256 });
        mine.set_source_action(85);
        mine.record[54..56].copy_from_slice(&1_u16.to_le_bytes());
        mine.record[56..58].copy_from_slice(&2_u16.to_le_bytes());
        mine.record[58..60].copy_from_slice(&23_u16.to_le_bytes());
        mine.record[65] = 1;
        mine.font_type_index = 1;
        mine.record[71] = 1;
        mine.record[74] = 5;
        mine.record[154..156].copy_from_slice(&3_u16.to_le_bytes());
        mine
    }

    fn rows() -> Vec<EntitySnapshot> {
        vec![
            row(0, 0, 0, Position::default()),
            row(1, 3, 0, Position::default()),
            row(2, 10, 78, Position::default()),
            row(
                3,
                3,
                1,
                Position {
                    x: 500,
                    y: 200,
                    z: 0,
                },
            ),
            row(4, 9, 29, Position::default()),
            row(5, 10, 23, Position::default()),
        ]
    }

    #[test]
    fn matching_projectile_charges_unspent_mine_and_consumes_its_holder() {
        let mut rows = rows();
        rows[2] = mine();
        rows[2].record[58..60].copy_from_slice(&u16::MAX.to_le_bytes());
        rows[4] = row(4, 9, 29, Position { x: 8, y: 9, z: 10 });
        rows[4].source_id = 1;
        rows[4].record[42..44].copy_from_slice(&5_u16.to_le_bytes());
        rows[5] = row(5, 10, 23, Position::default());
        rows[5].level_gate = 2;
        rows[5].record[74] = 2;

        let captured = capture(&mut rows, 2, 4).expect("matching Magic Mine impact");

        assert_eq!(captured.holder_slot, 5);
        assert_eq!(rows[2].word_34(), 2);
        assert_eq!(
            u16::from_le_bytes([rows[2].record[58], rows[2].record[59]]),
            23
        );
        assert_eq!(rows[5].active_value, 1);
        assert_eq!(
            u16::from_le_bytes([rows[5].record[50], rows[5].record[51]]),
            1
        );
        assert!(
            capture(&mut rows, 2, 4).is_none(),
            "charged mine rejects a second payload"
        );
    }

    #[test]
    fn active_mine_requests_ordinary_spell_cast_and_retries_when_allocation_fails() {
        let mut rows = rows();
        rows[2] = mine();
        let visit = visit(
            &mut rows,
            2,
            &[3],
            Some(CapturedSpell {
                family: 23,
                tier: 2,
                damage: 10_000,
                font_type: 1,
                life: 2,
            }),
            TerrainSample {
                ground_height: 0,
                cave_ceiling: None,
                water: false,
            },
        )
        .expect("captured source spell");

        assert_eq!(visit.projectiles.len(), 1);
        assert_eq!(visit.projectiles[0].target_slot, 3);
        assert_eq!(visit.projectiles[0].family, 23);
        assert_eq!(visit.spell_experience, Some((1, 23, 1)));
        assert!(!complete_projectiles(&mut rows, 2, &[None]).expect("allocation failure"));
        assert_eq!(rows[2].record[74], 5);
        assert_eq!(rows[2].source_word_96(), 3);
        assert_eq!(rows[2].font_type_index, 1);
    }

    #[test]
    fn successful_projectile_ends_one_charge_and_clears_its_target() {
        let mut rows = rows();
        rows[2] = mine();
        assert!(complete_projectiles(&mut rows, 2, &[Some(300)]).expect("successful cast"));
        assert_eq!(rows[2].record[74], 6);
        assert_eq!(rows[2].record[71], 0);
        assert_eq!(rows[2].source_word_96(), 0);
        assert_eq!(rows[2].yaw, 300 + 1_024);
    }
}
