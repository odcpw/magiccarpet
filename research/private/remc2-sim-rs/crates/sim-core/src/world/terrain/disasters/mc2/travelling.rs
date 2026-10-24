//! MC2 moving-disaster constructors and their immediate ring descendants.

use super::{AreaDamage, RingProfile, construct as construct_ring};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{SessionTerrain, cave_features::new_event},
    world::terrain::disasters::{
        deform_band,
        travelling::{self, Controller},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Impact,
    Earthquake,
    CraterTrail,
    RisingTrail,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) child: Option<u16>,
    pub(crate) sound_id: Option<u16>,
    pub(crate) hit_count: u16,
}

fn word(bytes: &[u8], index: usize) -> i16 {
    i16::from_le_bytes([bytes[index], bytes[index + 1]])
}

pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    kind: Kind,
) -> Option<u16> {
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 10;
    source.record[67] = 10;
    let (model, action, life) = match kind {
        Kind::Impact => (10, 10, 1),
        Kind::Earthquake => (15, 15, 128),
        Kind::CraterTrail => (32, 34, 0),
        Kind::RisingTrail => (51, 55, 0),
    };
    source.model = model;
    source.record[68] = model;
    source.set_source_action(action);
    source.set_max_life(life);
    source.set_life(life);
    source.set_control_flags(source.control_flags & !8);
    let (width, height): (i16, i16) = match kind {
        Kind::Impact => {
            source.set_subspell_index(100);
            source.set_control_flags((source.control_flags & 0xfffd_ffff) | 0x20000);
            (128, 128)
        }
        Kind::Earthquake => {
            source.set_subspell_index(100);
            source.set_actual_speed(256);
            source.set_entity_rng(source.entity_rng.wrapping_mul(9377).wrapping_add(9439));
            source.yaw = (source.entity_rng & 2047) as i16;
            source.record[32..34].copy_from_slice(&source.yaw.to_le_bytes());
            (1024, 16384)
        }
        Kind::CraterTrail => {
            source.set_actual_speed(256);
            source.record[74] = 2;
            (0, 0)
        }
        Kind::RisingTrail => {
            source.set_actual_speed(1024);
            source.set_source_dword_10(256);
            (768, 768)
        }
    };
    source.record[88..90].copy_from_slice(&width.to_le_bytes());
    source.record[90..92].copy_from_slice(&width.to_le_bytes());
    source.record[92..94].copy_from_slice(&height.to_le_bytes());
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    Some(slot)
}

/// Only the rising trail calls 10C80 here. Its callback is distinct from the
/// lowering ring's 116A0 rectangle writer despite sharing the shape carrier.
pub(crate) fn step<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    mut projectile_damage: F,
) -> Result<Visit, String>
where
    F: FnMut(&mut SourceArena, AreaDamage) -> Result<u16, String>,
{
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 10)
        .ok_or("moving disaster source is not live")?;
    let kind = match (source.model, source.action) {
        (10, 10) => Kind::Impact,
        (15, 15) => Kind::Earthquake,
        (32, 34) => Kind::CraterTrail,
        (51, 55) => Kind::RisingTrail,
        selector => return Err(format!("unsupported moving disaster {selector:?}")),
    };
    let width = word(&source.record, 88);
    let depth = word(&source.record, 90);
    let height = word(&source.record, 92);
    let owner = source.source_id;
    if kind == Kind::Impact {
        let index = usize::from(source.position.x >> 8) + 256 * usize::from(source.position.y >> 8);
        if terrain.planes().angles[index] & 15 != 0 {
            source.set_entity_rng(source.entity_rng.wrapping_mul(9377).wrapping_add(9439));
            deform_band(
                terrain,
                source.position,
                width,
                0,
                width / 256,
                -((source.entity_rng % 7) as i16),
                true,
            );
        }
        source.disable_drawing();
        return Ok(Visit {
            hide: true,
            ..Visit::default()
        });
    }
    let mut controller = Controller {
        position: source.position,
        life: source.life,
        water_clock: i32::from_le_bytes(source.record[20..24].try_into().expect("source dword10")),
        yaw: source.yaw,
        rng: source.entity_rng,
        speed: source.actual_speed,
    };
    let mut hit_count = 0;
    let mut sound_id = None;
    let visit = match kind {
        Kind::Earthquake => travelling::earthquake(terrain, &mut controller),
        Kind::CraterTrail => travelling::crater_trail(terrain, &mut controller),
        Kind::RisingTrail => {
            if let Some(delta) = travelling::prepare_rising_trail(terrain, &mut controller) {
                source.set_life(controller.life);
                source.set_entity_rng(controller.rng);
                let request = AreaDamage {
                    source_slot: slot,
                    position: controller.position,
                    owner,
                    payload: source.subspell_index,
                    half_width: width,
                    half_depth: depth,
                    vertical_offset: word(&source.record, 86),
                    half_height: height,
                    experience_spell: None,
                };
                if !deform_band(terrain, controller.position, width, 0, 1024, delta, false) {
                    hit_count = projectile_damage(arena, request)?;
                    sound_id = Some(10);
                }
                travelling::Visit::default()
            } else {
                travelling::Visit {
                    hide: true,
                    child_position: None,
                }
            }
        }
        Kind::Impact => unreachable!("handled above"),
    };
    let source = &mut arena.entities[usize::from(slot)];
    source.set_life(controller.life);
    source.set_source_dword_10(controller.water_clock);
    source.set_position(controller.position);
    source.set_entity_rng(controller.rng);
    source.yaw = controller.yaw;
    source.record[32..34].copy_from_slice(&controller.yaw.to_le_bytes());
    if visit.hide {
        source.disable_drawing();
    }
    let mut child = None;
    if let Some(position) = visit.child_position {
        let mut profile = RingProfile::default();
        profile.half_height = height;
        profile.lifetime = if kind == Kind::Earthquake {
            10
        } else {
            i32::from(source.record[74])
        };
        if kind == Kind::Earthquake {
            profile.half_width = width;
        }
        child = construct_ring(arena, terrain, position, profile);
        if let Some(child_slot) = child {
            let row = &mut arena.entities[usize::from(child_slot)];
            row.source_id = owner;
            row.record[30..32].copy_from_slice(&owner.to_le_bytes());
            if kind == Kind::Earthquake {
                row.record[90..92].copy_from_slice(&depth.to_le_bytes());
                row.record[42..44].copy_from_slice(&15_u16.to_le_bytes());
            }
        }
    }
    if !visit.hide && matches!(kind, Kind::CraterTrail | Kind::RisingTrail) {
        // NewEvent may have replaced this address. Move the actual current
        // occupant after the child/damage boundary, as the source pointer does.
        let source = &mut arena.entities[usize::from(slot)];
        let mut position = source.position;
        crate::move_entity(&mut position, source.yaw, 0, source.actual_speed);
        source.set_position(position);
    }
    Ok(Visit {
        hide: visit.hide,
        child,
        sound_id,
        hit_count,
    })
}
