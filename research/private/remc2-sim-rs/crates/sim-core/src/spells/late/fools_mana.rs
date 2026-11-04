//! Fool's Mana holder, sphere and retaliation rules.
//!
//! The campaign adapter owns source slots, selectors and scheduled phase
//! advancement. This module owns the state transitions for the holder and its
//! emitted spheres, including the Rapid Fire and Lightning Fire tier paths.

use crate::Position;
use crate::level_definition::late_spells::{FOOLS_MANA_PROPERTIES, LateSpellTier};

const SPHERES_PER_CAST: u8 = 6;

/// Owner SPELLS.DAT family22, in source tier order. Spells.cpp defaults are
/// not the live runtime values after the owner table load.
const fn rule(tier: LateSpellTier) -> Rule {
    Rule {
        mana_cost: tier.mana_cost,
        castle_mana_required: tier.runtime_castle_mana_limit,
        projectile_damage: tier.subspell_index as u16,
        sphere_tier: tier.life,
        duration: tier.cooldown,
    }
}

pub(crate) const SOURCE_RULES: [Rule; 3] = [
    rule(FOOLS_MANA_PROPERTIES.tiers[0]),
    rule(FOOLS_MANA_PROPERTIES.tiers[1]),
    rule(FOOLS_MANA_PROPERTIES.tiers[2]),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rule {
    /// Mana drained by one successful holder pulse (`manaCost_6`).
    pub mana_cost: i32,
    /// Minimum current castle mana (`maxManaLimit_A`); zero disables the gate.
    pub castle_mana_required: i32,
    /// Payload copied to each generated sphere and retaliation projectile.
    pub projectile_damage: u16,
    /// Source `life_0x1A` byte copied into each sphere.
    pub sphere_tier: u8,
    /// Source `word_0x18`, the holder pulse interval.
    pub duration: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Caster {
    pub entity_index: u16,
    pub position: Position,
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub castle_mana: Option<i32>,
    pub speed: i16,
    pub yaw: i16,
    pub next_entity: u16,
    pub entity_index2: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Holder {
    pub active: i16,
    pub duration: i16,
    pub cooldown: i16,
    /// Source rand_0x14_20 is a 16-bit LCG state.
    pub rng: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderVisit {
    /// The source checks for more than six available slots once, then makes
    /// six independent allocation attempts. A failed attempt does not cancel
    /// later attempts or refund the pulse.
    pub sphere_allocation_attempts: u8,
    pub mana_pulse: bool,
    pub denied: bool,
    pub cleanup: bool,
}

impl Holder {
    /// Initialize the holder's duration from its typed source/profile rule.
    pub(crate) const fn new(rule: Rule, active: i16, cooldown: i16, rng: u16) -> Self {
        Self {
            active,
            duration: rule.duration,
            cooldown,
            rng,
        }
    }

    /// One source holder callback, excluding the shared scheduler's phase tail.
    ///
    /// A missing parent still counts down the holder. A rejected live parent
    /// collapses the remaining timer to one before this visit's decrement,
    /// which requests the source reset/hide tail. Reclaim remains owned by the
    /// shared following-pass lifecycle.
    pub(crate) fn visit(
        &mut self,
        rule: Rule,
        parent: Option<&mut Caster>,
        available_slots: usize,
    ) -> HolderVisit {
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
                    if available_slots > usize::from(SPHERES_PER_CAST) {
                        result.sphere_allocation_attempts = SPHERES_PER_CAST;
                    }
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

    /// Apply post-constructor fields only after a child allocation succeeded.
    /// The source advances the holder RNG twice per successfully created child;
    /// failed allocations consume no holder RNG.
    pub(crate) fn sphere_parameters(
        &mut self,
        caster: Caster,
        rule: Rule,
        initial_pitch: i16,
    ) -> SphereParameters {
        let speed_base = (i32::from(caster.speed).wrapping_mul(4)).clamp(140, 280);
        self.rng = next_rng(self.rng);
        let speed = speed_base + (self.rng & 0x7f) as i32;
        self.rng = next_rng(self.rng);
        let yaw = (i32::from(caster.yaw) - 85
            + i32::from(self.rng % 0xaa)
            + i32::from(caster.next_entity))
            & 0x7ff;

        SphereParameters {
            position: caster.position,
            parent_entity: caster.entity_index,
            damage: rule.projectile_damage,
            tier: rule.sphere_tier,
            speed: speed as i16,
            yaw: yaw as i16,
            pitch: (caster.entity_index2 as i16).wrapping_add(initial_pitch),
            player_entity: if rule.sphere_tier >= 3 {
                caster.entity_index
            } else {
                0
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SphereParameters {
    /// The allocator initializes the sphere at this position before applying
    /// the source's shared linked-child positioning rule.
    pub position: Position,
    pub parent_entity: u16,
    pub damage: u16,
    pub tier: u8,
    pub speed: i16,
    pub yaw: i16,
    pub pitch: i16,
    /// Source rendering uses the parent player only for sphere tier3+.
    pub player_entity: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectileKind {
    RapidFire,
    LightningFire,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RetaliationVisit {
    pub projectile: Option<ProjectileKind>,
    pub target_entity: Option<u16>,
    pub award_spell_experience: bool,
    pub consume_sphere: bool,
    pub clear_target_link: bool,
}

/// One live Fool's Mana sphere's owned state. The source entity row remains the
/// authoritative carrier; the adapter maps these values in and out per visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Sphere {
    pub entity_index: u16,
    pub parent_entity: u16,
    pub player_entity: u16,
    pub target_entity: u16,
    pub target_aux: u32,
    pub tier: u8,
    pub damage: u16,
    pub mana: i32,
    pub retaliation_ticks: i32,
    /// Source rand_0x14_20 is a 16-bit LCG state.
    pub rng: u16,
}

impl Sphere {
    /// Source sub_36680. A zero or self-owned target does not produce a spell.
    /// Projectiles are requests: the caller attempts the ordinary allocation,
    /// then reports XP/consumption exactly as the source callback does.
    pub(crate) fn visit_retaliation(&mut self) -> RetaliationVisit {
        if self.target_entity == 0 {
            return RetaliationVisit::default();
        }
        if self.parent_entity == self.target_entity {
            self.target_entity = 0;
            self.target_aux = 0;
            return RetaliationVisit {
                clear_target_link: true,
                ..RetaliationVisit::default()
            };
        }

        let mut result = RetaliationVisit {
            target_entity: Some(self.target_entity),
            ..RetaliationVisit::default()
        };
        match self.tier {
            0 => {
                result.projectile = Some(ProjectileKind::RapidFire);
                result.award_spell_experience = true;
                result.consume_sphere = true;
            }
            1 => {
                let previous = self.retaliation_ticks;
                self.retaliation_ticks = previous.wrapping_add(1);
                if previous >= 8 {
                    result.award_spell_experience = true;
                    result.consume_sphere = true;
                } else if self.retaliation_ticks & 1 == 0 {
                    result.projectile = Some(ProjectileKind::RapidFire);
                }
            }
            2 | 3 => {
                let previous = self.retaliation_ticks;
                self.retaliation_ticks = previous.wrapping_add(1);
                if previous == 0 {
                    result.projectile = Some(ProjectileKind::LightningFire);
                } else if previous > 1 {
                    result.award_spell_experience = true;
                    result.consume_sphere = true;
                }
            }
            _ => {}
        }
        result
    }

    /// Source sub_36F30: choose the higher tier, randomly choose ownership on
    /// equal-or-higher tiers from distinct parents, add mana, and reclaim the
    /// absorbed slot immediately through sub_57F20 in this same callback.
    pub(crate) fn merge(&mut self, absorbed: Sphere) -> SphereMerge {
        let adopt_absorbed = if self.tier < absorbed.tier {
            true
        } else if self.parent_entity != absorbed.parent_entity {
            self.rng = next_rng(self.rng);
            self.rng & 1 != 0
        } else {
            false
        };

        if adopt_absorbed {
            self.parent_entity = absorbed.parent_entity;
            self.player_entity = absorbed.player_entity;
            self.tier = absorbed.tier;
            self.damage = absorbed.damage;
        }
        self.mana = self.mana.wrapping_add(absorbed.mana);
        SphereMerge {
            absorbed_entity: absorbed.entity_index,
            adopted_owner_and_tier: adopt_absorbed,
            mana_after_merge: self.mana,
            reclaim_absorbed_immediately: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SphereMerge {
    pub absorbed_entity: u16,
    pub adopted_owner_and_tier: bool,
    pub mana_after_merge: i32,
    /// Source calls sub_57F20 here: unlink from map, clear class and return the
    /// slot to the free list before this scheduled callback finishes.
    pub reclaim_absorbed_immediately: bool,
}

fn next_rng(rng: u16) -> u16 {
    (u32::from(rng).wrapping_mul(9_377).wrapping_add(9_439)) as u16
}

#[cfg(test)]
mod tests {
    use super::{Caster, Holder, ProjectileKind, Rule, SOURCE_RULES, Sphere};
    use crate::Position;

    fn caster() -> Caster {
        Caster {
            entity_index: 10,
            position: Position { x: 1, y: 2, z: 3 },
            life: 100,
            mana: 20_000,
            mana_regen: 12,
            castle_mana: Some(30_000),
            speed: 32,
            yaw: 100,
            next_entity: 7,
            entity_index2: 9,
        }
    }

    fn rule(tier: u8) -> Rule {
        let mut rule = SOURCE_RULES[usize::from(tier.min(2))];
        rule.sphere_tier = tier;
        rule
    }

    fn sphere(tier: u8, parent: u16, target: u16) -> Sphere {
        Sphere {
            entity_index: 40,
            parent_entity: parent,
            player_entity: parent,
            target_entity: target,
            target_aux: 77,
            tier,
            damage: 125,
            mana: 350,
            retaliation_ticks: 0,
            rng: 123,
        }
    }

    #[test]
    fn holder_pulse_charges_even_when_the_slot_gate_prevents_all_children() {
        let mut holder = Holder {
            active: SOURCE_RULES[0].duration,
            duration: SOURCE_RULES[0].duration,
            cooldown: 3,
            rng: 99,
        };
        let mut parent = caster();
        let visit = holder.visit(rule(0), Some(&mut parent), 6);
        assert_eq!(visit.sphere_allocation_attempts, 0);
        assert!(visit.mana_pulse);
        assert_eq!(parent.mana_regen, -15_000);
        assert_eq!(
            (holder.active, holder.cooldown),
            (SOURCE_RULES[0].duration - 1, 2)
        );
        assert!(!visit.cleanup);
    }

    #[test]
    fn insufficient_castle_mana_expires_holder_without_charging_or_spawning() {
        let mut holder = Holder {
            active: 2,
            duration: SOURCE_RULES[0].duration,
            cooldown: 2,
            rng: 99,
        };
        let mut parent = caster();
        parent.castle_mana = Some(4_999);
        let visit = holder.visit(rule(0), Some(&mut parent), 50);
        assert!(visit.denied);
        assert!(visit.cleanup);
        assert!(!visit.mana_pulse);
        assert_eq!(visit.sphere_allocation_attempts, 0);
        assert_eq!(parent.mana_regen, 12);
        assert_eq!((holder.active, holder.cooldown), (0, 1));
    }

    #[test]
    fn successful_source_tier_two_allocations_advance_rng_without_a_player_color_owner() {
        let mut holder = Holder {
            active: 1,
            duration: 1,
            cooldown: 0,
            rng: 99,
        };
        let first = holder.sphere_parameters(caster(), rule(2), 0);
        assert_eq!(first.parent_entity, 10);
        assert_eq!(first.player_entity, 0);
        assert_eq!(first.damage, 2_000);
        assert_eq!(first.tier, 2);
        assert!((140..=407).contains(&first.speed));
        assert_ne!(holder.rng, 99);
    }

    #[test]
    fn owner_tier_zero_pulse_requests_six_spheres_at_the_live_castle_gate() {
        let rule = SOURCE_RULES[0];
        let mut holder = Holder::new(rule, rule.duration, 0, 0x1234);
        let mut parent = Caster {
            mana: 20_000,
            castle_mana: Some(30_000),
            ..caster()
        };

        let visit = holder.visit(rule, Some(&mut parent), 7);

        assert_eq!(visit.sphere_allocation_attempts, 6);
        assert!(visit.mana_pulse);
        assert!(!visit.denied);
        assert_eq!(parent.mana_regen, -15_000);
        assert_eq!(holder.active, rule.duration - 1);
    }

    #[test]
    fn later_admitted_countdown_preserves_positive_regeneration() {
        let rule = SOURCE_RULES[0];
        let mut holder = Holder::new(rule, rule.duration - 1, 2, 0x1234);
        let mut parent = Caster { mana: 20_000, mana_regen: 123,
            castle_mana: Some(30_000), ..caster() };
        let visit = holder.visit(rule, Some(&mut parent), 7);
        assert!(!visit.denied && !visit.mana_pulse);
        assert_eq!(visit.sphere_allocation_attempts, 0);
        assert_eq!(parent.mana_regen, 123);
        assert_eq!((holder.active, holder.cooldown), (rule.duration - 2, 1));
    }

    #[test]
    fn rapid_fire_tier_emits_four_projectiles_before_experience_and_consumption() {
        let mut sphere = sphere(1, 10, 20);
        let visits = (0..9)
            .map(|_| sphere.visit_retaliation())
            .collect::<Vec<_>>();
        assert_eq!(
            visits
                .iter()
                .filter(|visit| visit.projectile == Some(ProjectileKind::RapidFire))
                .count(),
            4
        );
        assert!(!visits[7].consume_sphere);
        assert!(visits[8].award_spell_experience && visits[8].consume_sphere);
    }

    #[test]
    fn self_owned_contact_clears_relation_without_firing_or_experience() {
        let mut sphere = sphere(2, 10, 10);
        let visit = sphere.visit_retaliation();
        assert!(visit.clear_target_link);
        assert_eq!((sphere.target_entity, sphere.target_aux), (0, 0));
        assert_eq!(visit.projectile, None);
        assert!(!visit.award_spell_experience && !visit.consume_sphere);
    }

    #[test]
    fn lightning_fire_tier_fires_once_then_ends_after_the_source_delay() {
        let mut sphere = sphere(2, 10, 20);
        let first = sphere.visit_retaliation();
        let second = sphere.visit_retaliation();
        let third = sphere.visit_retaliation();
        assert_eq!(first.projectile, Some(ProjectileKind::LightningFire));
        assert_eq!(second.projectile, None);
        assert!(!second.consume_sphere);
        assert!(third.award_spell_experience && third.consume_sphere);
    }

    #[test]
    fn merge_preserves_stronger_tier_and_reclaims_absorbed_sphere_immediately() {
        let mut stronger = sphere(2, 10, 0);
        let weaker = Sphere {
            entity_index: 41,
            tier: 1,
            mana: 200,
            ..sphere(1, 20, 0)
        };
        let merged = stronger.merge(weaker);
        assert!(!merged.adopted_owner_and_tier);
        assert_eq!((stronger.parent_entity, stronger.tier), (10, 2));
        assert_eq!(merged.mana_after_merge, 550);
        assert_eq!(merged.absorbed_entity, 41);
        assert!(merged.reclaim_absorbed_immediately);
    }
}
