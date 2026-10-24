//! MC2 source-record adapter. Numeric selectors and packed fields stay here;
//! terrain rules operate on the canonical SessionTerrain geometry.

use super::{RingProfile, lower_band, tile_radius};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{SessionTerrain, cave_features::new_event},
};

pub(crate) mod damage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AreaDamage {
    pub(crate) source_slot: u16,
    pub(crate) position: Position,
    pub(crate) owner: u16,
    pub(crate) payload: u16,
    pub(crate) half_width: i16,
    pub(crate) half_depth: i16,
    pub(crate) vertical_offset: i16,
    pub(crate) half_height: i16,
    pub(crate) experience_spell: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StepOutcome {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    pub(crate) hit_count: u16,
    pub(crate) sound_id: Option<u16>,
}

impl StepOutcome {
    pub(crate) fn effects(self, source_slot: u16) -> impl Iterator<Item = crate::FlightEvent> {
        self.sound_id
            .map(move |sound_id| {
                crate::mark_source_audio_call(crate::SourceAudioCallTag::TerrainRingRumble);
                crate::FlightEvent::PrepareSound {
                    #[cfg(test)]
                    audio_site: crate::MissionAudioSite::TerrainDisaster,
                    entity_index: source_slot,
                    player_index: -1,
                    sound_id,
                }
            })
            .into_iter()
    }
}

/// NewAdd0A0B_4E840. This controller is intentionally absent from map chains;
/// it is nevertheless an ordinary live source slot with normal RNG/defaults.
pub(crate) fn construct(
    arena: &mut SourceArena,
    position: Position,
    profile: RingProfile,
) -> Option<u16> {
    let slot = arena.economy.allocate_slot()?;
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 10;
    source.record[67] = 10;
    source.model = 11;
    source.record[68] = 11;
    source.set_source_action(11);
    source.set_max_life(profile.max_life);
    source.set_life(profile.lifetime);
    source.set_subspell_index(profile.damage);
    source.set_control_flags((source.control_flags & 0xfffd_fff7) | 0x0002_0000);
    source.record[42..44].copy_from_slice(&11_u16.to_le_bytes());
    source.record[88..90].copy_from_slice(&profile.half_width.to_le_bytes());
    source.record[90..92].copy_from_slice(&profile.half_width.to_le_bytes());
    source.record[92..94].copy_from_slice(&profile.half_height.to_le_bytes());
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    Some(slot)
}

pub(crate) fn supports(class: u8, action: u8) -> bool {
    class == 10 && action == 11
}

/// sub_31FB0. Damage and experience complete synchronously before the first
/// geometry write. Caller publishes sound10 before common phase advancement.
pub(crate) fn step<F>(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    mut apply_damage: F,
) -> Result<StepOutcome, String>
where
    F: FnMut(&mut SourceArena, AreaDamage) -> Result<u16, String>,
{
    let source = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && supports(row.class, row.action))
        .ok_or_else(|| format!("terrain ring source slot {slot} is not live"))?;
    let mut count = i32::from_le_bytes(source.record[20..24].try_into().expect("source dword10"));
    if source.phase % 3 == 0 {
        count = count.wrapping_add(1);
        source.set_source_dword_10(count);
    }
    let old_life = source.life;
    source.set_life(old_life.wrapping_sub(1));
    let index = usize::from(source.position.x >> 8) + 256 * usize::from(source.position.y >> 8);
    if old_life < 0 || terrain.planes().angles[index] & 15 == 0 {
        source.disable_drawing();
        return Ok(StepOutcome {
            hide: true,
            ..StepOutcome::default()
        });
    }
    let seen = source.control_flags & 2 != 0;
    let pitch = i16::from_le_bytes([source.record[88], source.record[89]]);
    let damage = AreaDamage {
        source_slot: slot,
        position: source.position,
        owner: source.source_id,
        payload: if seen {
            source.subspell_index / 25
        } else {
            source.subspell_index
        },
        half_width: pitch,
        half_depth: i16::from_le_bytes([source.record[90], source.record[91]]),
        vertical_offset: i16::from_le_bytes([source.record[86], source.record[87]]),
        half_height: i16::from_le_bytes([source.record[92], source.record[93]]),
        experience_spell: match u16::from_le_bytes([source.record[42], source.record[43]]) {
            11 => Some(16),
            15 => Some(17),
            _ => None,
        },
    };
    let position = source.position;
    let hit_count = apply_damage(arena, damage)?;
    let radius = tile_radius(pitch);
    let mut current = i32::from(count as i16);
    if current > radius - 1 {
        current = radius - 1;
        if !seen {
            lower_band(terrain, position, pitch, radius, radius);
        }
    }
    let source = &mut arena.entities[usize::from(slot)];
    source.set_control_flags(source.control_flags | 2);
    lower_band(terrain, position, pitch, 0, current);
    let (_, _, _, sequence) = terrain.disaster_parts();
    *sequence = sequence.wrapping_add(1);
    Ok(StepOutcome {
        terrain_mutated: true,
        hit_count,
        sound_id: Some(10),
        hide: false,
    })
}
