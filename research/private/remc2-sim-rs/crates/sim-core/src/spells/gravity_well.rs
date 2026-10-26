//! Gravity Well's ordinary holder pulse and the payload handed to its field
//! impact. The campaign adapter retains these scalars in the source rows;
//! gravity-field motion and actor selection belong to `world::fields`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rule {
    /// Tier `subSpellIndex_2`, copied to the terminal field by the source.
    pub power: u16,
    /// Tier `life_0x1A`, copied to the projectile's byte 0x46.
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

/// Power and life byte attached to the ordinary class-9 projectile. Guided
/// impact copies the power to the shared Gravity Well field; the source wrapper
/// then applies the field lifetime without changing that payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectilePayload {
    pub power: u16,
    pub effect_life: u8,
}

impl ProjectilePayload {
    /// Terminal class-10/model-67 lifetime from `sub_678E0`.
    #[must_use]
    pub const fn field_lifetime(self) -> i16 {
        (self.effect_life as i16).wrapping_mul(8)
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
    /// One `sub_6C3E0` visit, excluding the shared scheduler's phase and
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
                        visit.projectile_payload = Some(ProjectilePayload {
                            power: rule.power,
                            effect_life: rule.effect_life,
                        });
                    }
                    caster.mana_regen = if first {
                        if caster.mana_regen >= 0 {
                            rule.mana_cost.wrapping_neg()
                        } else {
                            caster.mana_regen.wrapping_sub(rule.mana_cost)
                        }
                    } else if self.active != 0 && caster.mana_regen > 0 {
                        0
                    } else {
                        caster.mana_regen
                    };
                    visit.parent_mana_regen = Some(caster.mana_regen);
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

    #[test]
    fn admitted_opening_pulse_preserves_source_power_and_field_lifetime() {
        let rule = Rule {
            power: 750,
            effect_life: 16,
            mana_cost: 15_000,
            castle_mana_required: 300_000,
        };
        let mut holder = Holder {
            active: 7,
            duration: 7,
            cooldown: 11,
        };
        let mut caster = Caster {
            life: 10_000,
            mana: 20_000,
            mana_regen: 80,
            castle_mana: Some(350_000),
        };

        let visit = holder.step(rule, Some(&mut caster));

        assert_eq!(
            visit.projectile_payload,
            Some(ProjectilePayload {
                power: 750,
                effect_life: 16,
            })
        );
        assert_eq!(visit.projectile_payload.unwrap().field_lifetime(), 128);
        assert!(!visit.cleanup);
        assert_eq!(visit.parent_mana_regen, Some(-15_000));
        assert_eq!((holder.active, holder.cooldown), (6, 10));
        assert_eq!(caster.mana_regen, -15_000);
    }

    #[test]
    fn second_tier_life_uses_the_source_eightfold_field_wrapper() {
        let payload = ProjectilePayload {
            power: 1_500,
            effect_life: 26,
        };

        assert_eq!(payload.power, 1_500);
        assert_eq!(payload.field_lifetime(), 208);
    }

    #[test]
    fn failed_cast_admission_emits_nothing_and_runs_holder_cooldown_tail() {
        let rule = Rule {
            power: 750,
            effect_life: 16,
            mana_cost: 15_000,
            castle_mana_required: 300_000,
        };
        let mut holder = Holder {
            active: 7,
            duration: 7,
            cooldown: 11,
        };
        let mut caster = Caster {
            life: 10_000,
            mana: 14_999,
            mana_regen: 80,
            castle_mana: Some(300_000),
        };

        let visit = holder.step(rule, Some(&mut caster));

        assert_eq!(visit.projectile_payload, None);
        assert_eq!(visit.parent_mana_regen, None);
        assert!(visit.cleanup);
        assert_eq!((holder.active, holder.cooldown), (0, 10));
        assert_eq!(caster.mana_regen, 80);
    }
}
