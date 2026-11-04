//! Self-targeted healing, reflection and travel. The MC2 adapter supplies the
//! tier rules and retains this state in the existing authoritative holder.

use crate::{Position, move_entity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReboundMode {
    Reflect,
    DoubleDamage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeleportMode {
    Castle,
    Return,
    CycleCastles,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportSpellRule {
    Heal { percent: u16 },
    Rebound(ReboundMode),
    Teleport(TeleportMode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportSpellHolder {
    pub active: i16,
    pub duration: i16,
    pub presentation_timer: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    pub teleport_selector: u16,
    pub return_position: Position,
    pub rng: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportSpellPlayer {
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub flags: u32,
    pub position: Position,
    pub yaw: i16,
    pub castle_mana: Option<i32>,
    pub color: u8,
}

/// Source player-color order, with absent players/castles represented by None.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TeleportCastles {
    /// The caster's own A4 address is distinct from the global player table.
    pub own_position: Option<Position>,
    pub positions: [Option<Position>; 8],
    pub player_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportSpellSound {
    Heal,
    Teleport,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SupportSpellStep {
    pub award_experience: bool,
    pub cleanup: bool,
    pub stop_player: bool,
    pub sound: Option<SupportSpellSound>,
}

impl SupportSpellHolder {
    fn admitted(&self, player: &SupportSpellPlayer) -> bool {
        player.life >= 0
            && player.mana >= 0
            && (self.castle_mana_required == 0
                || player
                    .castle_mana
                    .is_some_and(|mana| mana >= self.castle_mana_required))
            && (self.active != self.duration || player.mana >= self.mana_cost)
    }

    fn charge(&self, player: &mut SupportSpellPlayer) {
        if self.active == self.duration {
            self.charge_pulse(player);
        } else if self.active != 0 && player.mana_regen > 0 {
            player.mana_regen = 0;
        }
    }

    fn charge_pulse(&self, player: &mut SupportSpellPlayer) {
        player.mana_regen = if player.mana_regen >= 0 {
            self.mana_cost.wrapping_neg()
        } else {
            player.mana_regen.wrapping_sub(self.mana_cost)
        };
    }

    /// One enabled holder callback, including word-36 cooldown. Parent loss
    /// deliberately differs between the source's three callbacks: Heal still
    /// decrements W; Rebound and Teleport leave W untouched.
    pub fn step(
        &mut self,
        rule: SupportSpellRule,
        player: Option<&mut SupportSpellPlayer>,
        castles: &TeleportCastles,
    ) -> SupportSpellStep {
        let mut result = SupportSpellStep::default();
        match rule {
            SupportSpellRule::Heal { percent } if self.active > 0 => {
                if let Some(player) = player {
                    if self.admitted(player) && player.mana >= self.mana_cost {
                        let first = self.active == self.duration;
                        if first {
                            result.sound = Some(SupportSpellSound::Heal);
                        }
                        if player.life < player.max_life {
                            result.award_experience = first;
                            player.life = player.life.wrapping_add(
                                player.max_life.wrapping_mul(i32::from(percent)) / 100,
                            );
                            if player.life > player.max_life {
                                player.life = player.max_life;
                            }
                            self.charge_pulse(player);
                        }
                    } else {
                        self.active = 1;
                    }
                }
                self.active = self.active.wrapping_sub(1);
                result.cleanup = self.active == 0;
            }
            SupportSpellRule::Rebound(mode) => {
                if let Some(player) = player {
                    if self.active <= 0 {
                        // sub_6AA00 clears these only on the following inactive
                        // visit, preserving all unrelated flags.
                        player.flags &= !0x8010;
                    } else {
                        if self.admitted(player) {
                            player.flags |= match mode {
                                ReboundMode::Reflect => 0x8000,
                                ReboundMode::DoubleDamage => 0x10,
                            };
                            self.charge(player);
                        } else {
                            self.active = 1;
                        }
                        self.active = self.active.wrapping_sub(1);
                        result.cleanup = self.active == 0;
                    }
                }
            }
            SupportSpellRule::Teleport(mode) if self.active > 0 => {
                if let Some(player) = player {
                    if self.admitted(player) {
                        if self.active == self.duration {
                            result.award_experience = true;
                            let own_castle = castles.own_position;
                            let mut destination = None;
                            match mode {
                                TeleportMode::Castle => {
                                    self.teleport_selector = 0;
                                    destination = own_castle.map(|p| castle_exit(p, player.yaw));
                                }
                                TeleportMode::Return => {
                                    if let Some(castle) = own_castle {
                                        if self.teleport_selector == 1 {
                                            destination = Some(self.return_position);
                                            self.teleport_selector = 0;
                                        } else {
                                            self.return_position = player.position;
                                            destination = Some(castle_exit(castle, player.yaw));
                                            self.teleport_selector = 1;
                                        }
                                    }
                                }
                                TeleportMode::CycleCastles => {
                                    for _ in 0..9 {
                                        let target = match self.teleport_selector {
                                            1 => None,
                                            2..=9 => {
                                                let color = (self.teleport_selector - 2) as u8;
                                                if color != player.color
                                                    && color < castles.player_count
                                                {
                                                    castles.positions[usize::from(color)]
                                                } else {
                                                    None
                                                }
                                            }
                                            _ => own_castle,
                                        };
                                        self.teleport_selector =
                                            self.teleport_selector.wrapping_add(1);
                                        if self.teleport_selector >= 9 {
                                            self.teleport_selector = 0;
                                        }
                                        if let Some(target) = target {
                                            destination = Some(castle_exit(target, player.yaw));
                                            break;
                                        }
                                    }
                                }
                            }
                            if let Some(destination) = destination {
                                player.position = destination;
                                result.sound = Some(SupportSpellSound::Teleport);
                            } else {
                                self.return_position = player.position;
                                self.rng = self.rng.wrapping_mul(9_377).wrapping_add(9_439);
                                move_entity(
                                    &mut self.return_position,
                                    (self.rng & 0x7ff) as i16,
                                    0,
                                    0x4000,
                                );
                                player.position = self.return_position;
                                self.teleport_selector = 0;
                            }
                            result.stop_player = true;
                        }
                        self.charge(player);
                    } else {
                        self.active = 1;
                    }
                    self.active = self.active.wrapping_sub(1);
                    result.cleanup = self.active == 0;
                    result.stop_player |= result.cleanup;
                }
            }
            _ => {}
        }
        if self.presentation_timer != 0 {
            self.presentation_timer = self.presentation_timer.wrapping_sub(1);
        }
        result
    }
}

fn castle_exit(mut position: Position, yaw: i16) -> Position {
    move_entity(&mut position, yaw.wrapping_sub(204) & 0x7ff, 0, -448);
    position
}

/// Mutable projectile fields consumed by the source's reflection operation.
/// Collision membership and whether the product is reflectable belong to the
/// projectile owner; no allocation or synthetic impact is performed here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReboundProjectile {
    pub owner: u16,
    pub target: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub damage: u16,
    pub mana: i32,
    pub life: i32,
    pub max_life: i32,
    pub rng: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReboundTarget {
    pub source_id: u16,
    pub flags: u32,
    pub mana: i32,
    pub position: Position,
    pub fov: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReboundSpread {
    Narrow,
    Wide,
}

/// `sub_68740` after the projectile adapter has checked its source product
/// selectors. A rejected reflection leaves both participants unchanged.
#[must_use]
pub fn reflect_projectile(
    projectile: &mut ReboundProjectile,
    target: ReboundTarget,
    spread: ReboundSpread,
) -> Option<i32> {
    let cost = projectile.mana / 4;
    if target.flags & 0x8010 == 0 || cost > target.mana {
        return None;
    }
    projectile.roll = projectile.yaw.wrapping_add(1024) & 0x7ff;
    projectile.pitch = projectile.pitch.wrapping_neg() & 0x7ff;
    projectile.fov = projectile.pitch;
    if target.flags & 0x10 != 0 {
        projectile.yaw = projectile.roll;
        projectile.damage = projectile.damage.wrapping_mul(2);
    } else {
        let (scatter_span, scatter_offset) = match spread {
            ReboundSpread::Narrow => (45, 22),
            ReboundSpread::Wide => (91, 45),
        };
        projectile.rng = projectile.rng.wrapping_mul(9_377).wrapping_add(9_439);
        projectile.yaw = projectile
            .roll
            .wrapping_add((projectile.rng % scatter_span) as i16)
            .wrapping_sub(scatter_offset);
    }
    projectile.target = projectile.owner;
    projectile.owner = target.source_id;
    projectile.life = projectile.max_life;
    projectile.position = target.position;
    projectile.position.z = projectile.position.z.wrapping_add(target.fov);
    Some(target.mana.wrapping_sub(cost))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holder() -> SupportSpellHolder {
        SupportSpellHolder {
            active: 11,
            duration: 11,
            presentation_timer: 64,
            mana_cost: 500,
            castle_mana_required: 0,
            teleport_selector: 0,
            return_position: Position::default(),
            rng: 17,
        }
    }

    fn player() -> SupportSpellPlayer {
        SupportSpellPlayer {
            life: 990,
            max_life: 1_000,
            mana: 10_000,
            mana_regen: 30,
            flags: 0x400_0400,
            position: Position {
                x: 123,
                y: 456,
                z: 789,
            },
            yaw: 204,
            castle_mana: Some(20_000),
            color: 0,
        }
    }

    #[test]
    fn support_spell_pulses_preserve_source_cost_caps_flags_and_parent_loss() {
        let castles = TeleportCastles::default();
        let mut h = holder();
        let mut p = player();
        let first = h.step(
            SupportSpellRule::Heal { percent: 10 },
            Some(&mut p),
            &castles,
        );
        assert_eq!(
            (p.life, p.mana_regen, h.active, h.presentation_timer),
            (1_000, -500, 10, 63)
        );
        assert!(first.award_experience);
        assert_eq!(first.sound, Some(SupportSpellSound::Heal));
        let full = h.step(
            SupportSpellRule::Heal { percent: 10 },
            Some(&mut p),
            &castles,
        );
        assert!(!full.award_experience);
        assert_eq!(
            p.mana_regen, -500,
            "full health does not pay for another Heal pulse"
        );
        p.life = 900;
        p.mana = 499;
        assert!(
            h.step(
                SupportSpellRule::Heal { percent: 10 },
                Some(&mut p),
                &castles
            )
            .cleanup
        );
        assert_eq!(p.life, 900);
        h = holder();
        h.step(SupportSpellRule::Heal { percent: 1 }, None, &castles);
        assert_eq!(h.active, 10);

        for (mode, bit) in [
            (ReboundMode::Reflect, 0x8000),
            (ReboundMode::DoubleDamage, 0x10),
        ] {
            h = holder();
            h.active = 1;
            p = player();
            let flags = p.flags;
            assert!(
                h.step(SupportSpellRule::Rebound(mode), Some(&mut p), &castles)
                    .cleanup
            );
            assert_eq!(p.flags, flags | bit, "last active visit retains reflection");
            h.step(SupportSpellRule::Rebound(mode), Some(&mut p), &castles);
            assert_eq!(
                p.flags, flags,
                "inactive visit clears only source Rebound bits"
            );
            h = holder();
            h.step(SupportSpellRule::Rebound(mode), None, &castles);
            assert_eq!(h.active, 11);
        }
    }

    #[test]
    fn teleport_retains_return_and_cycle_state_and_fallback_rng() {
        let mut castles = TeleportCastles {
            player_count: 3,
            ..TeleportCastles::default()
        };
        castles.positions[0] = Some(Position {
            x: 5_000,
            y: 6_000,
            z: 1_000,
        });
        castles.own_position = castles.positions[0];
        castles.positions[2] = Some(Position {
            x: 20_000,
            y: 22_000,
            z: 1_500,
        });
        let mut h = holder();
        let mut p = player();
        let start = p.position;
        let first = h.step(
            SupportSpellRule::Teleport(TeleportMode::Return),
            Some(&mut p),
            &castles,
        );
        assert!(first.stop_player && first.award_experience);
        assert_eq!(
            p.position,
            castle_exit(castles.positions[0].unwrap(), p.yaw)
        );
        assert_eq!((h.return_position, h.teleport_selector), (start, 1));
        h.active = h.duration;
        h.step(
            SupportSpellRule::Teleport(TeleportMode::Return),
            Some(&mut p),
            &castles,
        );
        assert_eq!((p.position, h.teleport_selector), (start, 0));
        h.active = h.duration;
        h.teleport_selector = 1;
        h.step(
            SupportSpellRule::Teleport(TeleportMode::CycleCastles),
            Some(&mut p),
            &castles,
        );
        assert_eq!(
            p.position,
            castle_exit(castles.positions[2].unwrap(), p.yaw)
        );
        assert_eq!(h.teleport_selector, 5);

        h.active = h.duration;
        let old_rng = h.rng;
        let old_position = p.position;
        let fallback = h.step(
            SupportSpellRule::Teleport(TeleportMode::Castle),
            Some(&mut p),
            &TeleportCastles::default(),
        );
        assert_eq!(h.rng, old_rng.wrapping_mul(9_377).wrapping_add(9_439));
        assert_ne!(p.position, old_position);
        assert_eq!(fallback.sound, None);
        assert!(fallback.award_experience && fallback.stop_player);
        h.active = h.duration;
        h.step(
            SupportSpellRule::Teleport(TeleportMode::Castle),
            None,
            &castles,
        );
        assert_eq!(h.active, h.duration);
    }

    #[test]
    fn reflection_transfers_owner_cost_and_target_with_tier_damage() {
        let mut projectile = ReboundProjectile {
            owner: 11,
            target: 0,
            position: Position::default(),
            yaw: 500,
            pitch: 256,
            roll: 0,
            fov: 0,
            damage: 0x9000,
            mana: 101,
            life: 1,
            max_life: 100,
            rng: 31,
        };
        let target = ReboundTarget {
            source_id: 22,
            flags: 0x10,
            mana: 24,
            position: Position {
                x: 33,
                y: 44,
                z: 55,
            },
            fov: 100,
        };
        let before = projectile;
        assert_eq!(
            reflect_projectile(&mut projectile, target, ReboundSpread::Narrow),
            None
        );
        assert_eq!(projectile, before);
        assert_eq!(
            reflect_projectile(
                &mut projectile,
                ReboundTarget { mana: 25, ..target },
                ReboundSpread::Narrow
            ),
            Some(0)
        );
        assert_eq!(
            (projectile.owner, projectile.target, projectile.life),
            (22, 11, 100)
        );
        assert_eq!(
            (projectile.yaw, projectile.pitch, projectile.damage),
            (1524, 1792, 0x2000)
        );
        assert_eq!(projectile.position.z, 155);
        assert_eq!(
            projectile.rng, 31,
            "precise reflection does not consume RNG"
        );
    }
}
