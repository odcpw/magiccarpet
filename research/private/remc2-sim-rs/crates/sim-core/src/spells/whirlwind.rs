//! Whirlwind's ordinary holder pulse and source-scaled projectile payload.
//! Kinematics, actor admission and live actor writes stay with the existing
//! `world::fields::wind` producer/consumer path.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rule {
    /// Tier `subSpellIndex_2` before the source divides by life.
    pub power: u16,
    /// Tier `life_0x1A`, copied to projectile byte 0x46.
    pub effect_life: u8,
    /// One successful source pulse's mana cost.
    pub mana_cost: i32,
    /// Minimum current castle mana; zero disables this gate.
    pub castle_mana_required: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Holder {
    pub active: i16,
    pub duration: i16,
    pub cooldown: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Caster {
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub castle_mana: Option<i32>,
}

/// The source tier values and the subspell value stored on the ordinary
/// model-26 projectile. `sub_6DCA0`/`sub_6C620` divide once before guided
/// flight; the impact wrapper copies this value and only sets field lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectilePayload {
    /// Raw tier `subSpellIndex_2` from SPELLS.DAT.
    pub power: u16,
    /// Value written to the projectile and copied by guided impact.
    pub projectile_power: u16,
    pub effect_life: u8,
}

impl ProjectilePayload {
    /// Terminal class-10/model-22 lifetime from `sub_678E0`.
    #[must_use]
    pub const fn field_lifetime(self) -> i16 {
        (self.effect_life as i16).wrapping_mul(8)
    }
}

impl Rule {
    #[must_use]
    pub const fn projectile_payload(self) -> ProjectilePayload {
        ProjectilePayload {
            power: self.power,
            projectile_power: if self.effect_life == 0 {
                self.power
            } else {
                self.power / self.effect_life as u16
            },
            effect_life: self.effect_life,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Visit {
    /// Present only on the opening pulse after the live parent passes the
    /// source mana, life and castle gates. The adapter attempts allocation;
    /// the source charges even when that allocation fails.
    pub projectile_payload: Option<ProjectilePayload>,
    /// The source writes parent mana regeneration only after admission passes.
    pub parent_mana_regen: Option<i32>,
    pub cleanup: bool,
}

impl Holder {
    /// One `sub_6C620` visit, excluding the shared scheduler's phase and
    /// deferred reset/reclaim tails.
    pub fn step(&mut self, rule: Rule, caster: Option<&mut Caster>) -> Visit {
        let mut visit = Visit::default();
        if self.active > 0 {
            if let Some(caster) = caster {
                let first = self.active == self.duration;
                let admitted = caster.life >= 0
                    && caster.mana >= 0
                    && (rule.castle_mana_required == 0
                        || caster
                            .castle_mana
                            .is_some_and(|mana| mana >= rule.castle_mana_required))
                    && (!first || caster.mana >= rule.mana_cost);
                if admitted {
                    if first {
                        visit.projectile_payload = Some(rule.projectile_payload());
                        caster.mana_regen = if caster.mana_regen >= 0 {
                            rule.mana_cost.wrapping_neg()
                        } else {
                            caster.mana_regen.wrapping_sub(rule.mana_cost)
                        };
                        visit.parent_mana_regen = Some(caster.mana_regen);
                    }
                } else {
                    self.active = 1;
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

#[cfg(test)]
mod tests {
    use super::{Caster, Holder, ProjectilePayload, Rule};

    fn rule(power: u16, effect_life: u8) -> Rule {
        Rule {
            power,
            effect_life,
            mana_cost: 2_000,
            castle_mana_required: 60_000,
        }
    }

    #[test]
    fn admitted_opening_pulse_divides_projectile_power_once_before_impact() {
        let mut holder = Holder {
            active: 5,
            duration: 5,
            cooldown: 7,
        };
        let mut caster = Caster {
            life: 10_000,
            mana: 4_000,
            mana_regen: -20,
            castle_mana: Some(60_000),
        };

        let visit = holder.step(rule(100, 5), Some(&mut caster));

        assert_eq!(
            visit.projectile_payload,
            Some(ProjectilePayload {
                power: 100,
                projectile_power: 20,
                effect_life: 5,
            })
        );
        assert_eq!(visit.projectile_payload.unwrap().field_lifetime(), 40);
        assert_eq!(visit.parent_mana_regen, Some(-2_020));
        assert!(!visit.cleanup);
        assert_eq!((holder.active, holder.cooldown), (4, 6));
        assert_eq!(caster.mana_regen, -2_020);
    }

    #[test]
    fn zero_life_uses_the_source_raw_payload_branch() {
        let payload = rule(180, 0).projectile_payload();
        assert_eq!(
            payload,
            ProjectilePayload {
                power: 180,
                projectile_power: 180,
                effect_life: 0,
            }
        );
        assert_eq!(payload.field_lifetime(), 0);
    }

    #[test]
    fn later_admitted_pulse_leaves_positive_mana_regeneration_unchanged() {
        let mut holder = Holder {
            active: 4,
            duration: 5,
            cooldown: 3,
        };
        let mut caster = Caster {
            life: 10_000,
            mana: 4_000,
            mana_regen: 80,
            castle_mana: Some(60_000),
        };

        let visit = holder.step(rule(100, 5), Some(&mut caster));

        assert_eq!(visit.projectile_payload, None);
        assert_eq!(visit.parent_mana_regen, None);
        assert_eq!(caster.mana_regen, 80);
        assert_eq!((holder.active, holder.cooldown), (3, 2));
    }

    #[test]
    fn third_tier_divides_once_before_guided_impact_copies_projectile_power() {
        let payload = rule(240, 10).projectile_payload();

        assert_eq!(payload.power, 240);
        assert_eq!(payload.projectile_power, 24);
        assert_eq!(payload.effect_life, 10);
        assert_eq!(payload.field_lifetime(), 80);
    }

    #[test]
    fn insufficient_mana_does_not_create_projectile_but_holder_still_counts_down() {
        let mut holder = Holder {
            active: 1,
            duration: 1,
            cooldown: 1,
        };
        let mut caster = Caster {
            life: 10_000,
            mana: 1_999,
            mana_regen: 80,
            castle_mana: Some(60_000),
        };

        let visit = holder.step(rule(100, 5), Some(&mut caster));

        assert_eq!(visit.projectile_payload, None);
        assert_eq!(visit.parent_mana_regen, None);
        assert!(visit.cleanup);
        assert_eq!((holder.active, holder.cooldown), (0, 0));
        assert_eq!(caster.mana_regen, 80);
    }
}
