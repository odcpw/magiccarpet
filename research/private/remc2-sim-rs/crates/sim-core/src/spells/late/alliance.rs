//! Alliance spell holder and area effect.
//!
//! Holder pulses request the ordinary class9/model25 projectile. Its impact
//! writes Alliance state into each eligible class5 source row, which the
//! existing creature action/StageVar consumers read on their normal visits.

use crate::level_definition::late_spells::{ALLIANCE_PROPERTIES, LateSpellTier};
use crate::{Position, native_world::EntitySnapshot};

const fn rule(tier: LateSpellTier, index: u8) -> Rule {
    Rule {
        mana_cost: tier.mana_cost,
        castle_mana_required: tier.runtime_castle_mana_limit,
        damage: tier.subspell_index as u16,
        radius_tiles: tier.life,
        tier: index,
        duration: tier.cooldown,
    }
}

/// Owner SPELLS.DAT family24, in source tier order.
pub(crate) const SOURCE_RULES: [Rule; 3] = [
    rule(ALLIANCE_PROPERTIES.tiers[0], 0),
    rule(ALLIANCE_PROPERTIES.tiers[1], 1),
    rule(ALLIANCE_PROPERTIES.tiers[2], 2),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rule {
    pub mana_cost: i32,
    pub castle_mana_required: i32,
    /// Source `subSpellIndex_2`, copied as the alliance lifetime/duration.
    pub damage: u16,
    /// Source `life_0x1A`, the square tile radius around the impact target.
    pub radius_tiles: u8,
    /// The 0-based typed profile row retained for catalog lookup.
    pub tier: u8,
    /// Source `word_0x18`, the holder pulse interval.
    pub duration: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Caster {
    pub entity_index: u16,
    pub source_id: u16,
    pub position: Position,
    pub life: i32,
    pub mana: i32,
    pub mana_regen: i32,
    pub castle_mana: Option<i32>,
    pub yaw: i16,
    pub pitch: i16,
    pub next_entity: u16,
    pub entity_index2: u16,
    pub height_offset: i16,
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

    /// One source sub_6CD20 holder callback; scheduler tail/reclaim stay shared.
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
                } else if parent.mana_regen > 0 {
                    parent.mana_regen = 0;
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HolderVisit {
    pub projectile_allocation_attempts: u8,
    pub mana_pulse: bool,
    pub denied: bool,
    pub cleanup: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectileParameters {
    pub source_family: u8,
    pub owner_entity: u16,
    pub holder_entity: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub tier: u8,
    pub damage: u16,
    pub radius_tiles: u8,
    pub pending_cast_auxiliary: u8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub sound: u16,
}

impl Rule {
    /// Fields copied by sub_6CD20 after the regular model25 constructor.
    pub(crate) fn projectile_parameters(
        self,
        caster: Caster,
        holder_entity: u16,
    ) -> ProjectileParameters {
        let yaw = caster.yaw.wrapping_add(caster.next_entity as i16);
        let pitch = caster.pitch.wrapping_add(caster.entity_index2 as i16);
        let mut position = Position {
            z: caster.position.z.wrapping_add(caster.height_offset),
            ..caster.position
        };
        crate::move_entity(&mut position, yaw, pitch, 16_384);
        ProjectileParameters {
            source_family: 24,
            owner_entity: caster.source_id,
            holder_entity,
            target_class: 10,
            target_model: 74,
            tier: self.tier,
            damage: self.damage,
            radius_tiles: self.radius_tiles,
            pending_cast_auxiliary: caster.pending_cast_auxiliary,
            position,
            yaw,
            pitch,
            sound: 9,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Impact {
    pub changed_targets: u16,
    pub affected_entities: Vec<u16>,
    pub sounds: Vec<(u16, u16)>,
    pub hide_effect: bool,
}

/// Source eligibility from sub_3A7F0. The numeric action and StageVar values
/// remain in the entity row because creature owners consume them directly.
pub(crate) fn eligible_target(target: &EntitySnapshot) -> bool {
    if target.class != 5 {
        return false;
    }
    let model = target.model;
    let excluded_model =
        matches!(model, 12..=15 | 22..=23 | 26..=27) || (model == 25 && target.record[74] != 0);
    let stage = target.record[77];
    !excluded_model && !matches!(stage, 13 | 14 | 16 | 17) && target.action != 232
}

/// sub_3A650. `map_order` is the source's row-major cell scan with each cell's
/// own map-chain order. The handler writes StageVar2=14 and the parent/timer
/// words consumed by the ordinary creature callback, then hides the impact.
pub(crate) fn impact(
    rows_by_slot: &mut [EntitySnapshot],
    effect_slot: u16,
    map_order: &[u16],
) -> Impact {
    let mut result = Impact::default();
    let Some(effect) = rows_by_slot.get(usize::from(effect_slot)) else {
        return result;
    };
    // Source IfSubtypeCallCreatingManaSphere_4A190 dispatches Alliance
    // collisions to the separate bare (10,74) terminal constructor. The
    // ordinary (10,25) carrier is not itself an impact consumer.
    if effect.class != 10 || effect.model != 74 {
        return result;
    }
    result.hide_effect = true;
    let target_slot = effect.source_word_96();
    let Some(center) = rows_by_slot
        .get(usize::from(target_slot))
        .filter(|row| target_slot != 0 && row.index == target_slot && row.active)
    else {
        rows_by_slot[usize::from(effect_slot)].disable_drawing();
        return result;
    };
    let center_position = center.position;
    let target_class = center.class;
    let target_model = center.model;
    let radius = usize::from(effect.record[74]);
    let width = radius.saturating_mul(2);
    if width == 0 || width > 510 {
        rows_by_slot[usize::from(effect_slot)].disable_drawing();
        return result;
    }
    let center_x = center_position.x.wrapping_add(128).to_le_bytes()[1];
    let center_y = center_position.y.wrapping_add(128).to_le_bytes()[1];
    let start_x = center_x.wrapping_sub(radius as u8);
    let start_y = center_y.wrapping_sub(radius as u8);
    let owner_id = effect.source_id;
    let duration = effect.subspell_index;
    let sound_entity = effect.index;

    // `map_order` is already the source's row-major scan result, including
    // each visited oldMapEntity chain. sub_3A650 never re-derives an entity's
    // cell from its current position after it was reached through that chain.
    for &slot in map_order {
        if slot == 0 {
            continue;
        }
        let Some(target) = rows_by_slot.get(usize::from(slot)) else {
            continue;
        };
        if target.index != slot
            || target.class != target_class
            || target.model != target_model
            || !eligible_target(target)
        {
            continue;
        }
        let target = &mut rows_by_slot[usize::from(slot)];
        let previous_stage = i16::from(target.record[76] as i8);
        target.record[77] = 14;
        target.record[76] = 0;
        target.record[78..80].copy_from_slice(&previous_stage.to_le_bytes());
        target.record[44..46].copy_from_slice(&owner_id.to_le_bytes());
        target.record[50..52].copy_from_slice(&duration.to_le_bytes());
        target.record[52..54].copy_from_slice(&duration.to_le_bytes());
        if target.action & 7 == 2 {
            target.record[154..156].fill(0);
        } else {
            target.set_source_action(target.model.wrapping_mul(8).wrapping_add(7));
        }
        result.changed_targets = result.changed_targets.saturating_add(1);
        result.affected_entities.push(slot);
        result.sounds.push((sound_entity, 6));
    }
    rows_by_slot[usize::from(effect_slot)].disable_drawing();
    result
}

#[cfg(test)]
mod tests {
    use super::{Caster, Holder, SOURCE_RULES, impact};
    use crate::Position;
    use crate::native_world::EntitySnapshot;

    fn caster() -> Caster {
        Caster {
            entity_index: 10,
            source_id: 10,
            position: Position { x: 1, y: 2, z: 3 },
            life: 100,
            mana: 100_000,
            mana_regen: 100,
            castle_mana: Some(500_000),
            yaw: 100,
            pitch: 20,
            next_entity: 5,
            entity_index2: 7,
            height_offset: 64,
            pending_cast_auxiliary: 9,
        }
    }

    fn row(index: u16, class: u8, model: u8, position: Position) -> EntitySnapshot {
        let mut row = crate::world::terrain::cave_features::new_event(index, 1, position);
        row.class = class;
        row.model = model;
        row.record[67] = class;
        row.record[68] = model;
        row.active = true;
        row.set_life(100);
        row
    }

    #[test]
    fn tier_two_cast_requests_real_area_projectile_and_pulse_cost() {
        let rule = SOURCE_RULES[2];
        let mut holder = Holder::new(rule, rule.duration, 1);
        let mut parent = caster();
        let visit = holder.visit(rule, Some(&mut parent));
        let projectile = rule.projectile_parameters(parent, 40);
        assert_eq!(visit.projectile_allocation_attempts, 1);
        assert_eq!(parent.mana_regen, -30_000);
        assert_eq!(
            (projectile.source_family, projectile.target_model),
            (24, 74)
        );
        assert_eq!((projectile.damage, projectile.radius_tiles), (2_710, 32));
        assert_eq!(projectile.sound, 9);
    }

    #[test]
    fn expired_holder_rejects_cast_without_charging_or_projectile() {
        let rule = SOURCE_RULES[0];
        let mut holder = Holder::new(rule, rule.duration, 0);
        let mut parent = caster();
        parent.castle_mana = Some(19_999);
        let visit = holder.visit(rule, Some(&mut parent));
        assert!(visit.denied && visit.cleanup);
        assert_eq!(visit.projectile_allocation_attempts, 0);
        assert_eq!(parent.mana_regen, 100);
    }

    #[test]
    fn zero_castle_requirement_uses_the_physical_holder_cost() {
        let rule = super::Rule {
            mana_cost: 1_234,
            castle_mana_required: 0,
            ..SOURCE_RULES[0]
        };
        let mut parent = caster();
        parent.castle_mana = None;
        parent.mana = rule.mana_cost;
        let mut holder = Holder::new(rule, rule.duration, 0);
        let visit = holder.visit(rule, Some(&mut parent));
        assert!(!visit.denied);
        assert_eq!(visit.projectile_allocation_attempts, 1);
        assert_eq!(parent.mana_regen, -1_234);

        let mut short = caster();
        short.castle_mana = None;
        short.mana = rule.mana_cost - 1;
        let mut holder = Holder::new(rule, rule.duration, 0);
        let denied = holder.visit(rule, Some(&mut short));
        assert!(denied.denied);
        assert_eq!(denied.projectile_allocation_attempts, 0);
        assert_eq!(short.mana_regen, 100);
    }

    #[test]
    fn impact_writes_alliance_state_consumed_by_the_creature_action() {
        let mut rows = vec![
            row(0, 0, 0, Position::default()),
            // The actual consumer centers on word96's target, not the bare
            // terminal row's default position.
            // The source map-chain visit is authoritative. This entity's
            // current rounded coordinate lies outside the scanned cells, but
            // it was reached from a cell in the row-major chain supplied to
            // the handler.
            row(1, 5, 0, Position { x: 255, y: 0, z: 0 }),
            row(
                2,
                5,
                0,
                Position {
                    // Source u16 coordinate representation of signed -128.
                    x: u16::MAX - 127,
                    y: 0,
                    z: 0,
                },
            ),
            row(3, 10, 74, Position { x: 0, y: 0, z: 0 }),
        ];
        rows[1].record[76] = 4;
        rows[3].source_id = 10;
        rows[3].set_subspell_index(1_100);
        rows[3].record[74] = 1;
        rows[3].record[154..156].copy_from_slice(&2_u16.to_le_bytes());

        let impact = impact(&mut rows, 3, &[1]);

        assert_eq!(impact.changed_targets, 1);
        assert_eq!(impact.affected_entities, [1]);
        assert_eq!(rows[1].record[77], 14);
        assert_eq!(rows[1].record[76], 0);
        assert_eq!(
            i16::from_le_bytes([rows[1].record[78], rows[1].record[79]]),
            4
        );
        assert_eq!(
            u16::from_le_bytes([rows[1].record[44], rows[1].record[45]]),
            10
        );
        assert_eq!(
            u16::from_le_bytes([rows[1].record[50], rows[1].record[51]]),
            1_100
        );
        assert_eq!(
            u16::from_le_bytes([rows[1].record[52], rows[1].record[53]]),
            1_100
        );
        assert_eq!(rows[1].action, 7);
        assert!(impact.hide_effect && rows[3].drawing_disabled());
    }

    #[test]
    fn impact_does_not_reapply_to_a_creature_already_in_alliance_mode() {
        let mut rows = vec![
            row(0, 0, 0, Position::default()),
            row(
                1,
                5,
                0,
                Position {
                    x: 8_192,
                    y: 0,
                    z: 0,
                },
            ),
            row(
                2,
                5,
                0,
                Position {
                    x: 8_192,
                    y: 0,
                    z: 0,
                },
            ),
            row(3, 10, 74, Position::default()),
        ];
        rows[1].record[77] = 14;
        rows[3].record[74] = 16;
        rows[3].record[154..156].copy_from_slice(&2_u16.to_le_bytes());

        let impact = impact(&mut rows, 3, &[1]);

        assert!(impact.affected_entities.is_empty());
        assert_eq!(rows[1].record[77], 14);
        assert!(rows[3].drawing_disabled());
    }

    #[test]
    fn legacy_model25_carrier_is_not_an_alliance_terminal() {
        let mut rows = vec![
            row(0, 0, 0, Position::default()),
            row(
                1,
                5,
                0,
                Position {
                    x: 8_192,
                    y: 0,
                    z: 0,
                },
            ),
            row(
                2,
                5,
                0,
                Position {
                    x: 8_192,
                    y: 0,
                    z: 0,
                },
            ),
            row(3, 10, 25, Position::default()),
        ];
        rows[1].record[76] = 4;
        rows[3].record[74] = 16;
        rows[3].record[154..156].copy_from_slice(&2_u16.to_le_bytes());

        let impact = impact(&mut rows, 3, &[1]);

        assert!(impact.affected_entities.is_empty());
        assert!(!impact.hide_effect);
        assert_eq!(rows[1].record[77], 0);
        assert_eq!(rows[1].record[76], 4);
        assert!(!rows[3].drawing_disabled());
    }
}
