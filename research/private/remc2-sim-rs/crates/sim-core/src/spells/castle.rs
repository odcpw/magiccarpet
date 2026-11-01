//! Castle flyer guidance and contact inputs from the source spell behavior.

use crate::{Position, combat::area::Shape, native_world::ParticleParameter};

/// The movement fields written by `sub_656D0` / `sub_66D00` before contact.
/// The caller resolves the current fixed-address target on every visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CastleFlight {
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub speed: i16,
    pub minimum_speed: i16,
}

impl CastleFlight {
    pub(crate) fn advance(&mut self, target: Position) -> (i16, i16) {
        let (desired_yaw, desired_pitch) =
            crate::level1_mission::projectile_guidance_angles(self.position, target);
        // str_D7BD6[60] supplies the 22-unit turn cap on both axes.
        self.yaw = self
            .yaw
            .wrapping_add(crate::turn_toward(self.yaw, desired_yaw, 22))
            & 0x07ff;
        self.pitch = self
            .pitch
            .wrapping_add(crate::turn_toward(self.pitch, desired_pitch, 22))
            & 0x07ff;
        self.speed = self
            .speed
            .wrapping_add(2 * self.minimum_speed.cmp(&self.speed) as i16);
        crate::move_entity(&mut self.position, self.yaw, self.pitch, self.speed);
        (desired_yaw, desired_pitch)
    }
}

/// `sub_4D900` binds particle 18 through `SetEntityIndexAndRot_49CD0`.
pub(crate) fn particle_shape(particle: &ParticleParameter) -> Shape {
    Shape {
        half_width: (particle.speed / 2) as i16,
        half_depth: (particle.speed / 2) as i16,
        vertical_offset: (particle.rotation_speed / 2) as i16,
        half_height: (particle.rotation_speed / 2) as i16,
    }
}

/// Source ground and cave-ceiling tests run after the movement and target
/// collision. The returned height is the position written on contact.
pub(crate) fn terrain_contact(
    position: Position,
    ground: i16,
    cave_ceiling: Option<i16>,
    shape: Shape,
) -> Option<i16> {
    if ground > position.z {
        return Some(ground);
    }
    let ceiling = cave_ceiling?.wrapping_sub(shape.half_height);
    (position.z > ceiling).then_some(ceiling)
}
