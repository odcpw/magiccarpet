//! Cave collapse state and rules. The adapter retains these fields in the
//! authoritative source row; terrain writes go to the live session planes.

use crate::SIN_16_16;
use crate::legacy_sqrt;
use crate::{Position, move_entity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaveInRule {
    pub power: u16,
    pub radius_tier: u8,
}

impl CaveInRule {
    pub const fn projectile_power(self) -> u16 {
        if self.radius_tier == 0 {
            self.power
        } else {
            self.power / self.radius_tier as u16
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaveInHolder {
    pub active: i16,
    pub duration: i16,
    pub cooldown: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaveInCaster {
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub castle_mana: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HolderVisit {
    pub launch: bool,
    pub cleanup: bool,
}

impl CaveInHolder {
    /// sub_6CFA0: charge even when the optional projectile allocation fails.
    /// Parent loss still counts down in a Cave; outside a Cave only word36 runs.
    pub fn step(&mut self, cave: bool, caster: Option<&mut CaveInCaster>) -> HolderVisit {
        let mut visit = HolderVisit::default();
        if cave && self.active > 0 {
            if let Some(caster) = caster {
                let admitted = caster.life >= 0
                    && caster.mana >= 0
                    && (self.castle_mana_required == 0
                        || caster
                            .castle_mana
                            .is_some_and(|mana| mana >= self.castle_mana_required))
                    && (self.active != self.duration || caster.mana >= self.mana_cost);
                if !admitted {
                    self.active = 1;
                } else if self.active == self.duration {
                    visit.launch = true;
                    caster.mana_regen = if caster.mana_regen >= 0 {
                        self.mana_cost.wrapping_neg()
                    } else {
                        caster.mana_regen.wrapping_sub(self.mana_cost)
                    };
                }
            }
            self.active = self.active.wrapping_sub(1);
            visit.cleanup = self.active == 0;
        }
        if self.cooldown != 0 {
            self.cooldown = self.cooldown.wrapping_sub(1);
        }
        visit
    }
}

/// The terrain owner implements these writes using its canonical topology and
/// material updates. Heights are source bytes, positions are wrapping world units.
pub trait CaveTerrain {
    fn perimeter(&self, x: u8, y: u8, diameter: u16) -> (i32, i32);
    fn floor(&self, x: u8, y: u8) -> u8;
    fn ceiling(&self, x: u8, y: u8) -> u8;
    fn write_floor(&mut self, x: u8, y: u8, height: i32, force_material: bool);
    fn write_ceiling(&mut self, x: u8, y: u8, height: u8);
    fn normalize(&mut self, x: u8, y: u8);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaveInEffect {
    pub position: Position,
    pub radius_tier: u8,
    pub stage: u8,
    pub life: i32,
    pub wave_phase: i16,
    pub smoke_emitted: bool,
    pub floor: i16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectVisit {
    pub hide: bool,
    pub terrain_mutated: bool,
    pub smoke: Vec<(Position, i16)>,
}

impl CaveInEffect {
    /// sub_311E0, including the six overlapping rings, wizard clearance bubbles,
    /// one smoke ring, and the final *following* hide visit.
    pub fn step(&mut self, terrain: &mut impl CaveTerrain, wizards: &[Position]) -> EffectVisit {
        let mut visit = EffectVisit::default();
        if self.stage == 1 {
            return visit;
        }
        if self.stage > 2 {
            visit.hide = self.stage == 3;
            return visit;
        }
        let base = match self.radius_tier {
            1 => 5,
            2 => 7,
            _ => 3,
        };
        let maximum_radius = base + 12;
        let center_x = (self.position.x.wrapping_add(128) >> 8) as u8;
        let center_y = (self.position.y.wrapping_add(128) >> 8) as u8;
        if self.stage == 0 {
            let (floor, ceiling) = terrain.perimeter(
                center_x.wrapping_sub(maximum_radius as u8),
                center_y.wrapping_sub(maximum_radius as u8),
                2 * maximum_radius as u16,
            );
            self.position.z = ceiling as i16;
            self.floor = floor as i16;
            self.life = 40;
            self.wave_phase = 227;
            self.smoke_emitted = false;
            self.stage = 2;
        }
        let ceiling = i32::from(self.position.z);
        let floor = i32::from(self.floor);
        let span = ((ceiling - floor) >> 1) + ceiling - floor;
        let mut previous_radius = 0;
        let mut last_tile = (0, 0);
        for ring in 0..6 {
            let radius = base + 2 * ring;
            let outer = radius << 8;
            let inner = (previous_radius - 1024).clamp(0, maximum_radius << 8);
            previous_radius = outer;
            let phase = i32::from(self.wave_phase) - 68 * ring;
            if phase <= 0 || phase > 512 {
                continue;
            }
            let x = center_x.wrapping_sub(radius as u8);
            let y = center_y.wrapping_sub(radius as u8);
            for row in 0..2 * radius {
                for column in 0..2 * radius {
                    let tx = x.wrapping_add(column as u8);
                    let ty = y.wrapping_add(row as u8);
                    let position = Position {
                        x: u16::from(tx) << 8,
                        y: u16::from(ty) << 8,
                        z: 0,
                    };
                    let distance = distance(self.position, position);
                    if distance < outer && distance >= inner {
                        let weight =
                            (65_536 + SIN_16_16[512 + ((distance << 10) / outer) as usize]) >> 1;
                        let pulse = ((span.wrapping_mul(weight) >> 16)
                            .wrapping_mul(65_536 - SIN_16_16[512 + phase as usize]));
                        let target = (pulse >> 18) + floor;
                        if i32::from(terrain.floor(tx, ty)) < target {
                            terrain.write_floor(tx, ty, target, true);
                        }
                        let target = (ceiling - (pulse >> 16)).max(0);
                        if i32::from(terrain.ceiling(tx, ty)) > target {
                            terrain.write_ceiling(tx, ty, target as u8);
                        }
                        for &wizard in wizards {
                            let dx = i32::from(position.x.wrapping_sub(wizard.x) as i16);
                            let dy = i32::from(position.y.wrapping_sub(wizard.y) as i16);
                            let square = (dx * dx) as u32 + (dy * dy) as u32;
                            if square <= 0x64000 {
                                let radius = (legacy_sqrt(0x64000 - square) >> 5) as i32;
                                let center = i32::from(wizard.z) >> 5;
                                let target = (center - radius).clamp(0, 254);
                                if i32::from(terrain.floor(tx, ty)) > target {
                                    terrain.write_floor(tx, ty, target, false);
                                }
                                let target = (center + radius).clamp(0, 254);
                                if i32::from(terrain.ceiling(tx, ty)) < target {
                                    terrain.write_ceiling(tx, ty, target as u8);
                                }
                            }
                        }
                    }
                    terrain.normalize(tx, ty);
                }
            }
            // Literal ix after the outer source loop increments its high byte.
            last_tile = (
                x.wrapping_add((2 * radius - 1) as u8),
                y.wrapping_add((2 * radius) as u8),
            );
        }
        visit.terrain_mutated = true;
        if !self.smoke_emitted && self.wave_phase > 455 {
            self.smoke_emitted = true;
            let radius = ((base << 8) - 768).clamp(256, 8192);
            let smoke_z = i16::from(terrain.floor(last_tile.0, last_tile.1)).wrapping_mul(32);
            for yaw in (0..2048).step_by(28) {
                let mut position = self.position;
                move_entity(&mut position, yaw, 0, radius as i16);
                position.z = smoke_z;
                visit.smoke.push((position, yaw));
            }
        }
        self.life = self.life.wrapping_add(4);
        self.wave_phase = self.wave_phase.wrapping_add(22);
        if self.wave_phase > 1024 {
            self.stage = 3;
        }
        visit
    }
}

fn distance(a: Position, b: Position) -> i32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    legacy_sqrt((dx * dx) as u32 + (dy * dy) as u32) as i32
}

/// Source sub_68490 uses wrapped XY distance. Vertical separation enters the
/// pitch angle, rather than replacing that distance with a three-axis radius.
pub fn target_score(from: Position, yaw: i16, pitch: i16, target: Position) -> Option<u32> {
    let (target_yaw, target_pitch) =
        crate::level1_mission::projectile_guidance_angles(from, target);
    let yaw = crate::angle_distance(i32::from(yaw), i32::from(target_yaw)) as usize;
    let pitch = crate::angle_distance(i32::from(pitch), i32::from(target_pitch)) as usize;
    let distance = distance(from, target);
    if yaw > 113 || pitch > 113 || distance > 5120 {
        return None;
    }
    let yaw_forward = distance.wrapping_mul(SIN_16_16[512 + yaw]) >> 16;
    let pitch_forward = distance.wrapping_mul(SIN_16_16[512 + pitch]) >> 16;
    let yaw_side = distance.wrapping_mul(SIN_16_16[yaw]).wrapping_mul(4) >> 16;
    let pitch_side = distance.wrapping_mul(SIN_16_16[pitch]).wrapping_mul(4) >> 16;
    Some(
        yaw_forward
            .wrapping_mul(yaw_forward)
            .wrapping_add(pitch_forward.wrapping_mul(pitch_forward))
            .wrapping_add(yaw_side.wrapping_mul(yaw_side))
            .wrapping_add(pitch_side.wrapping_mul(pitch_side)) as u32,
    )
}

/// Ordinary CaveIn flyer (sub_67910 -> sub_65820). Map scanning, allocation
/// and source identities stay in the adapter; flight/lifetime live here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaveInProjectile {
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub speed: i16,
    pub min_speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub flags: u32,
    pub target: u16,
}

impl CaveInProjectile {
    pub fn advance(&mut self, guidance: Option<(u16, Position)>) {
        if self.target != 0 {
            if let Some((_, position)) = guidance {
                (self.roll, self.fov) =
                    crate::level1_mission::projectile_guidance_angles(self.position, position);
                // str_D7BD6[60]: both source sub_58350 limits are22.
                self.yaw = self
                    .yaw
                    .wrapping_add(crate::turn_toward(self.yaw, self.roll, 22))
                    & 0x7ff;
                self.pitch = self
                    .pitch
                    .wrapping_add(crate::turn_toward(self.pitch, self.fov, 22))
                    & 0x7ff;
            }
        } else if self.flags & 2 == 0 {
            self.flags |= 2;
            if let Some((slot, position)) = guidance {
                self.target = slot;
                (self.roll, self.fov) =
                    crate::level1_mission::projectile_guidance_angles(self.position, position);
                self.yaw = self.roll;
                self.pitch = self.fov;
            } else {
                self.roll = self.yaw;
                self.fov = self.pitch;
            }
        }
        self.speed = self
            .speed
            .wrapping_add(2 * self.min_speed.cmp(&self.speed) as i16);
        move_entity(&mut self.position, self.yaw, self.pitch, self.speed);
    }

    pub fn contact(
        &mut self,
        ground: i16,
        ceiling_limit: Option<i16>,
        target: Option<Position>,
    ) -> bool {
        if let Some(position) = target {
            self.position = position;
            return true;
        }
        if ground > self.position.z {
            self.position.z = ground;
            return true;
        }
        if let Some(ceiling) = ceiling_limit
            && self.position.z > ceiling
        {
            self.position.z = ceiling;
            return true;
        }
        self.life = self.life.wrapping_sub(1);
        self.life < 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cave_gate_parent_loss_and_null_allocation_keep_source_charge_and_timers() {
        let initial = CaveInHolder {
            active: 31,
            duration: 31,
            cooldown: 64,
            mana_cost: 11_000,
            castle_mana_required: 100_000,
        };
        let mut holder = initial;
        let mut caster = CaveInCaster {
            life: 100,
            mana: 11_000,
            mana_regen: 1,
            castle_mana: Some(100_000),
        };
        assert_eq!(
            holder.step(false, Some(&mut caster)),
            HolderVisit::default()
        );
        assert_eq!(
            (holder.active, holder.cooldown, caster.mana_regen),
            (31, 63, 1)
        );
        let visit = holder.step(true, Some(&mut caster));
        assert!(visit.launch && !visit.cleanup);
        // Source has no refund for NewEvent returning null after this request.
        assert_eq!((holder.active, caster.mana_regen), (30, -11_000));
        holder = initial;
        holder.step(true, None);
        assert_eq!(holder.active, 30);
        caster.castle_mana = Some(99_999);
        holder = initial;
        assert!(holder.step(true, Some(&mut caster)).cleanup);
        assert_eq!(holder.active, 0);
        let target = Position {
            x: 0,
            y: 0_u16.wrapping_sub(5_000),
            z: 1_600,
        };
        assert!(
            target_score(Position::default(), 0, 0, target).is_some(),
            "source XY range admits this angled target"
        );
        assert!(
            crate::level1_mission::projectile_target_score(Position::default(), 0, 0, target)
                .is_none(),
            "XYZ range would reject the same source-admitted target"
        );
        assert_eq!(
            CaveInRule {
                power: 480,
                radius_tier: 1
            }
            .projectile_power(),
            480
        );
        assert_eq!(
            CaveInRule {
                power: 1200,
                radius_tier: 2
            }
            .projectile_power(),
            600
        );
    }
}
