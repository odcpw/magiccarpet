//! Magic Mine holder and the model29 spell that charges an existing mine.
//!
//! The scheduled spell adapter allocates the ordinary class9/model29 flyer
//! and applies its shared relation/movement setup. This leaf owns the family23
//! cost gates, pulse cadence and exact payload copied to that flyer.

use crate::level_definition::late_spells::{LateSpellTier, MAGIC_MINE_PROPERTIES};

const fn rule(tier: LateSpellTier) -> Rule {
    Rule {
        mana_cost: tier.mana_cost,
        castle_mana_required: tier.runtime_castle_mana_limit,
        damage: tier.subspell_index as u16,
        tier: tier.life,
        duration: tier.cooldown,
        repeat_count: match tier.life {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => 0,
        },
    }
}

/// Owner SPELLS.DAT values after SetDefaultSpells_5C0A0's live override.
pub(crate) const SOURCE_RULES: [Rule; 3] = [
    rule(MAGIC_MINE_PROPERTIES.tiers[0]),
    rule(MAGIC_MINE_PROPERTIES.tiers[1]),
    rule(MAGIC_MINE_PROPERTIES.tiers[2]),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rule {
    /// `manaCost_6`, applied to the caster once per holder pulse.
    pub mana_cost: i32,
    /// `maxManaLimit_A`, checked against the caster's current castle mana.
    pub castle_mana_required: i32,
    /// `subSpellIndex_2`, copied into the placed model78 field's max life.
    pub damage: u16,
    /// `life_0x1A`, captured by the model29 projectile.
    pub tier: u8,
    /// `word_0x18`, the source holder's pulse interval.
    pub duration: i16,
    /// Source model78 charge count for this mine tier:1,2,4.
    pub repeat_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Caster {
    pub entity_index: u16,
    pub position: crate::Position,
    pub terrain_altitude: i16,
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub castle_mana: Option<i32>,
    pub yaw: i16,
    pub pitch: i16,
    pub height_offset: i16,
    pub next_entity: u16,
    pub entity_index2: u16,
    /// `Type_str_160.byte_0x154`; the ordinary helper transfers it only when
    /// the projectile allocation succeeds.
    pub pending_cast_auxiliary: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Holder {
    pub active: i16,
    pub duration: i16,
    pub cooldown: i16,
}

impl Holder {
    pub(crate) const fn new(rule: Rule, active: i16, cooldown: i16) -> Self {
        Self {
            active,
            duration: rule.duration,
            cooldown,
        }
    }

    /// One `sub_6CAC0` holder visit. The source makes one constructor attempt
    /// on the pulse frame and still charges if allocation fails.
    pub(crate) fn visit(&mut self, rule: Rule, parent: Option<&mut Caster>) -> HolderVisit {
        let mut result = HolderVisit::default();
        if self.active > 0 {
            if let Some(parent) = parent {
                let admitted = parent.life >= 0
                    && parent.mana >= 0
                    && (rule.castle_mana_required == 0
                        || parent
                            .castle_mana
                            .is_some_and(|mana| mana >= rule.castle_mana_required))
                    && (self.active != self.duration || parent.mana >= rule.mana_cost);
                if !admitted {
                    self.active = 1;
                    result.denied = true;
                } else if self.active == self.duration {
                    result.mana_pulse = true;
                    result.projectile_allocation_attempts = 1;
                    parent.mana_regen = if parent.mana_regen >= 0 {
                        rule.mana_cost.wrapping_neg()
                    } else {
                        parent.mana_regen.wrapping_sub(rule.mana_cost)
                    };
                }
            }
            self.active = self.active.wrapping_sub(1);
            result.cleanup = self.active == 0;
        }
        if self.cooldown != 0 {
            self.cooldown = self.cooldown.wrapping_sub(1);
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderVisit {
    pub projectile_allocation_attempts: u8,
    pub mana_pulse: bool,
    pub denied: bool,
    /// The shared ordinary scheduled tail hides the expired holder; source
    /// arena cleanup then reclaims it at the following pass boundary.
    pub cleanup: bool,
}

/// Fields written by `sub_6CAC0` after the ordinary class9/model29
/// constructor and common linked-child setup succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectileParameters {
    pub source_family: u8,
    pub owner_entity: u16,
    pub holder_entity: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub tier: u8,
    pub source_mana: i32,
    pub pending_cast_auxiliary: u8,
    pub position: crate::Position,
    pub destination: crate::Position,
    pub yaw: i16,
    pub pitch: i16,
    pub sound: u16,
}

impl Rule {
    /// Source family23 holder copies these fields only after model29 allocates.
    pub(crate) fn projectile_parameters(
        self,
        caster: Caster,
        holder_entity: u16,
        holder_mana: i32,
    ) -> ProjectileParameters {
        let mut destination = caster.position;
        crate::move_entity(
            &mut destination,
            caster.yaw.wrapping_add(caster.next_entity as i16),
            0,
            4_096,
        );
        destination.z = caster.terrain_altitude;
        ProjectileParameters {
            source_family: 23,
            owner_entity: caster.entity_index,
            holder_entity,
            target_class: 10,
            target_model: 78,
            tier: self.tier,
            source_mana: holder_mana,
            pending_cast_auxiliary: caster.pending_cast_auxiliary,
            position: crate::Position {
                z: caster.position.z.wrapping_add(caster.height_offset),
                ..caster.position
            },
            destination,
            yaw: caster.yaw.wrapping_add(caster.next_entity as i16),
            pitch: caster.pitch.wrapping_add(caster.entity_index2 as i16),
            sound: 15,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Caster, Holder, SOURCE_RULES};
    use crate::Position;

    fn caster() -> Caster {
        Caster {
            entity_index: 10,
            position: Position { x: 1, y: 2, z: 3 },
            terrain_altitude: 0,
            life: 100,
            mana: 50_000,
            mana_regen: 100,
            castle_mana: Some(100_000),
            yaw: 100,
            pitch: 20,
            height_offset: 150,
            next_entity: 5,
            entity_index2: 7,
            pending_cast_auxiliary: 9,
        }
    }

    #[test]
    fn source_tier_two_pulse_produces_one_mine_targeting_flyer_and_charges() {
        let rule = SOURCE_RULES[2];
        let mut holder = Holder::new(rule, rule.duration, 2);
        let mut parent = caster();
        let visit = holder.visit(rule, Some(&mut parent));
        assert_eq!(visit.projectile_allocation_attempts, 1);
        assert!(visit.mana_pulse);
        assert_eq!(parent.mana_regen, -24_000);
        assert_eq!((holder.active, holder.cooldown), (86, 1));

        let projectile = rule.projectile_parameters(parent, 40, 875);
        assert_eq!((projectile.source_family, projectile.tier), (23, 2));
        assert_eq!((projectile.target_class, projectile.target_model), (10, 78));
        assert_eq!(projectile.source_mana, 875);
        assert_eq!(projectile.sound, 15);
    }

    #[test]
    fn insufficient_castle_mana_expires_without_cost_or_projectile() {
        let rule = SOURCE_RULES[0];
        let mut holder = Holder::new(rule, rule.duration, 0);
        let mut parent = caster();
        parent.castle_mana = Some(19_999);
        let visit = holder.visit(rule, Some(&mut parent));
        assert!(visit.denied && visit.cleanup);
        assert!(!visit.mana_pulse);
        assert_eq!(visit.projectile_allocation_attempts, 0);
        assert_eq!(parent.mana_regen, 100);
    }

    #[test]
    fn later_admitted_countdown_preserves_positive_regeneration() {
        let rule = SOURCE_RULES[0];
        let mut holder = Holder::new(rule, rule.duration - 1, 2);
        let mut parent = caster();
        let visit = holder.visit(rule, Some(&mut parent));
        assert!(!visit.denied && !visit.mana_pulse);
        assert_eq!(visit.projectile_allocation_attempts, 0);
        assert_eq!(parent.mana_regen, 100);
        assert_eq!((holder.active, holder.cooldown), (rule.duration - 2, 1));
    }
}
