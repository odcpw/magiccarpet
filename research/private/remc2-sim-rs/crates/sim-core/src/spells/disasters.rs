//! The four terrain spell holders. The adapter keeps these values in the
//! existing source holder; terrain effects retain their own arena lifetimes.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisasterSpell {
    Tremor,
    Crater,
    Earthquake,
    Volcano,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisasterHolder {
    pub active: i16,
    pub duration: i16,
    pub cooldown: u16,
    pub mana_cost: i32,
    pub castle_mana_required: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisasterCaster {
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

impl DisasterHolder {
    /// sub_6B870/6BAB0/6BCF0/6BF30. Allocation follows `launch` and does not
    /// control charging. Missing parents still consume the active countdown.
    pub fn step(
        &mut self,
        spell: DisasterSpell,
        caster: Option<&mut DisasterCaster>,
    ) -> HolderVisit {
        let mut visit = HolderVisit::default();
        if self.active > 0 {
            if let Some(caster) = caster {
                let first = self.active == self.duration;
                let admitted = caster.mana >= 0
                    && caster.life >= 0
                    && (self.castle_mana_required == 0
                        || caster
                            .castle_mana
                            .is_some_and(|mana| mana >= self.castle_mana_required))
                    && (!first || caster.mana >= self.mana_cost);
                if admitted {
                    visit.launch = first;
                    if first {
                        caster.mana_regen = if caster.mana_regen >= 0 {
                            self.mana_cost.wrapping_neg()
                        } else {
                            caster.mana_regen.wrapping_sub(self.mana_cost)
                        };
                    } else if matches!(spell, DisasterSpell::Tremor | DisasterSpell::Crater)
                        && caster.mana_regen > 0
                    {
                        caster.mana_regen = 0;
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
    use super::*;

    #[test]
    fn source_charge_location_parent_loss_and_failed_admission() {
        for spell in [
            DisasterSpell::Tremor,
            DisasterSpell::Crater,
            DisasterSpell::Earthquake,
            DisasterSpell::Volcano,
        ] {
            let mut holder = DisasterHolder {
                active: 3,
                duration: 3,
                cooldown: 64,
                mana_cost: 500,
                castle_mana_required: 1000,
            };
            let mut caster = DisasterCaster {
                life: 100,
                mana: 500,
                mana_regen: -20,
                castle_mana: Some(1000),
            };
            let first = holder.step(spell, Some(&mut caster));
            assert!(first.launch && !first.cleanup);
            // No allocation outcome can refund this source launch charge.
            assert_eq!(
                (caster.mana_regen, holder.active, holder.cooldown),
                (-520, 2, 63)
            );
            caster.mana_regen = 20;
            assert!(!holder.step(spell, Some(&mut caster)).launch);
            assert_eq!(
                caster.mana_regen,
                if matches!(spell, DisasterSpell::Tremor | DisasterSpell::Crater) {
                    0
                } else {
                    20
                }
            );
            assert!(holder.step(spell, None).cleanup);
            assert_eq!((holder.active, holder.cooldown), (0, 61));
            holder.active = 3;
            caster.castle_mana = None;
            let rejected = holder.step(spell, Some(&mut caster));
            assert!(!rejected.launch && rejected.cleanup);
            assert_eq!(holder.active, 0);
        }
    }
}
