//! Consumers for the complete Magic Mine field visit.
//!
//! The source action calls `sub_6DCA0` for each captured-spell request, feeds
//! the guidance yaw back into the mine before any fanout turn, awards family-23
//! experience before allocation, and creates an Impact/Ash row on expiry.
//! Shared scheduling and the mine's hide/reclaim tail remain with the caller.

use super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload;
use super::{
    EntitySnapshot, FireballProjectileOrigin, Level2SharedSpellResetSession, ParticleParameter,
    SessionTerrain,
};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::EffectKind;
use crate::world::fields::magic_mine::{self as field, CapturedSpell, ProjectileRequest, Visit};
use crate::{Position, world::effects};

const MAGIC_MINE_EXPERIENCE_FAMILY: u8 = 23;
const UNCARGED_FAMILY: u16 = u16::MAX;

/// Owner `SPELLS.DAT` life bytes, before LevelInit's map-specific first-tier
/// edits to families 4 and 19. Those two families are source-null in
/// `sub_6DCA0`, so their life byte is not consumed by a Mine discharge.
/// Families with dedicated typed rules below are checked against this table.
const SOURCE_SPELL_LIFE: [[u8; 3]; 26] = [
    [0, 1, 2],
    [0, 1, 2],
    [0, 1, 2],
    [0, 0, 0],
    [19, 25, 16],
    [0, 0, 0],
    [0, 0, 1],
    [0, 1, 2],
    [0, 0, 1],
    [2, 5, 10],
    [0, 1, 2],
    [1, 2, 3],
    [0, 0, 0],
    [0, 0, 1],
    [0, 1, 2],
    [60, 80, 120],
    [6, 12, 24],
    [16, 32, 64],
    [7, 9, 11],
    [19, 25, 16],
    [16, 26, 40],
    [5, 10, 10],
    [0, 1, 2],
    [0, 1, 2],
    [16, 26, 32],
    [0, 1, 2],
];

fn typed_spell_life(family: u8, tier: u8) -> Option<u8> {
    let tier = usize::from(tier);
    match family {
        7 => Some(crate::level_definition::LIGHTNING_PROPERTIES.tiers[tier].product),
        9 => Some(crate::level_definition::METEOR_PROPERTIES.tiers[tier].impact_duration),
        13 => Some(crate::level_definition::STEAL_MANA_TIERS[tier].product),
        14 => Some(crate::level_definition::DUEL_TIERS[tier].product),
        15..=18 => {
            let properties = match family {
                15 => crate::level_definition::TREMOR_PROPERTIES,
                16 => crate::level_definition::CRATER_PROPERTIES,
                17 => crate::level_definition::EARTHQUAKE_PROPERTIES,
                _ => crate::level_definition::VOLCANO_PROPERTIES,
            };
            Some(properties.tiers[tier].impact_parameter)
        }
        20 => Some(crate::level_definition::GRAVITY_WELL_TIERS[tier].effect_parameter),
        21 => Some(crate::level_definition::WHIRLWIND_TIERS[tier].effect_parameter),
        22..=24 => {
            use crate::level_definition::late_spells::{
                ALLIANCE_PROPERTIES, FOOLS_MANA_PROPERTIES, MAGIC_MINE_PROPERTIES,
            };
            let properties = match family {
                22 => FOOLS_MANA_PROPERTIES,
                23 => MAGIC_MINE_PROPERTIES,
                _ => ALLIANCE_PROPERTIES,
            };
            Some(properties.tiers[tier].life)
        }
        _ => None,
    }
}

/// Resolve the captured source words through the installed spell rules.
/// `None` means only that the mine still has its 0xffff uncaptured sentinel;
/// a recorded but unsupported or malformed profile is an error.
pub(crate) fn captured_profile(mine: &EntitySnapshot) -> Result<Option<CapturedSpell>, String> {
    if mine.class != 10 || mine.model != 78 {
        return Err("captured spell profile needs a class10/model78 Mine row".to_owned());
    }
    let family_word = mine
        .record
        .get(58..60)
        .ok_or("Magic Mine family source word is missing")?;
    let family_word = u16::from_le_bytes([family_word[0], family_word[1]]);
    if family_word == UNCARGED_FAMILY {
        return Ok(None);
    }
    let family = u8::try_from(family_word)
        .ok()
        .filter(|family| *family < 26)
        .ok_or("Magic Mine captured family is outside SPELLS.DAT")?;
    let tier_word = mine
        .record
        .get(56..58)
        .ok_or("Magic Mine tier source word is missing")?;
    let tier_word = u16::from_le_bytes([tier_word[0], tier_word[1]]);
    let tier = u8::try_from(tier_word)
        .ok()
        .filter(|tier| *tier < 3)
        .ok_or("Magic Mine captured tier is outside SPELLS.DAT")?;

    let set_spell = crate::spell_activation::level2_set_spell_profile(family, tier)
        .ok_or_else(|| format!("no typed SetSpell profile for family {family}, tier {tier}"))?;
    let source_life = SOURCE_SPELL_LIFE[usize::from(family)][usize::from(tier)];
    let life = typed_spell_life(family, tier).unwrap_or(source_life);
    if life != source_life {
        return Err(format!(
            "typed Mine life disagrees with installed SPELLS.DAT for family {family}, tier {tier}"
        ));
    }

    // The installed owner SPELLS.DAT row 0/tier 2 is 180. The compiled
    // Spells.cpp fallback used by the generic SetSpell table has 80 there.
    let damage = if family == 0 && tier == 2 {
        180
    } else {
        set_spell.subspell_index
    };
    Ok(Some(CapturedSpell {
        family,
        tier,
        damage,
        // SetSpell retains exactly the bit tested by sub_3A8B0 when it maps
        // fontType to the Mine's 1-or-6 charge state.
        font_type: u8::from(set_spell.byte_0x3b == 0),
        life,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceProduct {
    Fireball { terminal_model: u8 },
    LightningRay,
    LightningStormShot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ConsumeOutcome {
    /// True when a real projectile guidance yaw exhausted the mine's charge.
    pub charge_finished: bool,
    /// Live class-9 products in source allocation order. The scheduler can
    /// admit each into the remainder of the current ascending pass.
    pub projectiles: Vec<u16>,
    /// A real model-0/model-5 expiry effect, if the source allocator succeeded.
    pub expiry_effect: Option<u16>,
    /// The field visit already disabled drawing; the common scheduler owns
    /// following-boundary reclaim.
    pub hidden: bool,
}

fn source_product(
    request: ProjectileRequest,
    captured: Option<CapturedSpell>,
) -> Result<Option<SourceProduct>, String> {
    let Some(captured) =
        captured.filter(|spell| spell.family == request.family && spell.tier == request.tier)
    else {
        return Err("Magic Mine projectile request has no matching captured spell".to_owned());
    };
    if captured.damage != request.damage {
        return Err("Magic Mine projectile request changed captured spell damage".to_owned());
    }

    let product = match request.family {
        0 if captured.life < 2 => SourceProduct::Fireball { terminal_model: 0 },
        0 => {
            return Err(
                "Magic Mine family-0 subtype 28 has no current typed constructor".to_owned(),
            );
        }
        7 if captured.life == 0 => SourceProduct::LightningRay,
        7 if captured.life <= 2 => SourceProduct::LightningStormShot,
        7 => return Ok(None),
        // These branches of sub_6DCA0 unconditionally reach LABEL_60 without
        // invoking a type-9 source constructor. Preserve their real null.
        1..=6 | 8 | 10..=12 | 14 | 23 => return Ok(None),
        // These source branches do allocate. Their ordinary Rust launch APIs
        // still require an active player holder, which a captured mine is not.
        9 | 13 | 15 => {
            return Err(format!(
                "Magic Mine family-{} needs its bare source constructor",
                request.family
            ));
        }
        family => {
            return Err(format!(
                "Magic Mine family {family} is outside the first-16 source dispatch"
            ));
        }
    };
    Ok(Some(product))
}

fn source_selector(product: SourceProduct) -> (u8, u8) {
    match product {
        SourceProduct::Fireball { terminal_model } => (10, terminal_model),
        SourceProduct::LightningRay => (10, 23),
        SourceProduct::LightningStormShot => (9, 9),
    }
}

fn source_sound(product: SourceProduct) -> u16 {
    match product {
        SourceProduct::Fireball { .. } | SourceProduct::LightningStormShot => 9,
        SourceProduct::LightningRay => 23,
    }
}

fn launch_position(mine: &EntitySnapshot, altitude_offset: i16) -> Position {
    Position {
        z: mine.position.z.wrapping_add(altitude_offset),
        ..mine.position
    }
}

fn base_angles(request: ProjectileRequest, origin: Position) -> (i16, i16) {
    crate::level1_mission::projectile_guidance_angles(origin, request.target_position)
}

fn construct_request(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    mine_slot: u16,
    request: ProjectileRequest,
    captured: Option<CapturedSpell>,
) -> Result<Option<(u16, i16)>, String> {
    let Some(product) = source_product(request, captured)? else {
        return Ok(None);
    };
    let mine = arena
        .entities
        .get(usize::from(mine_slot))
        .filter(|mine| {
            mine.index == mine_slot
                && mine.class == 10
                && mine.model == 78
                && mine.record.get(74).is_some_and(|phase| *phase == 5)
        })
        .cloned()
        .ok_or("Magic Mine projectile request is outside phase five")?;
    if mine.word_32() != request.owner_slot || mine.source_word_96() != request.target_slot {
        return Err("Magic Mine projectile request no longer matches its source links".to_owned());
    }
    let target = arena
        .entities
        .get(usize::from(request.target_slot))
        .filter(|target| target.index == request.target_slot)
        .ok_or("Magic Mine target address is outside the source arena")?;
    if target.class != 3 || target.model > 1 {
        return Err("Magic Mine target no longer matches its class-3 selector".to_owned());
    }
    let (source_yaw, pitch) = base_angles(request, mine.position);
    let yaw = source_yaw.wrapping_add(request.yaw_offset) & 2047;
    let position = launch_position(&mine, request.altitude_offset);
    let (terminal_class, terminal_model) = source_selector(product);

    match product {
        SourceProduct::Fireball { .. } => {
            // The ordinary family-0 initializer is the same typed Fireball
            // carrier used by the player spell; the direct target link keeps
            // this mine shot on the source's already-selected target.
            let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                return Ok(None);
            };
            let payload = Level2FireballProjectileLaunchPayload {
                origin: FireballProjectileOrigin::PlayerSpell,
                position: mine.position,
                yaw,
                pitch,
                axis_position: request.target_position,
                source_id_entity_index: request.owner_slot,
                holder_entity_index: 0,
                source_subspell_index: request.damage,
                source_mana: 50,
                source_dword_0x10: 0,
                target_source_entity_index: request.target_slot,
                target_class: terminal_class,
                target_model: terminal_model,
            };
            let Some(projectile_slot) = arena
                .economy
                .initialize_fireball_projectile(slot, payload, 0)
            else {
                return Ok(None);
            };
            arena.economy.prepare_sound(
                projectile_slot,
                request.owner_slot,
                -1,
                source_sound(product),
            );
            let projectile = arena
                .economy
                .fireball_projectiles
                .iter_mut()
                .find(|projectile| {
                    projectile.active && projectile.source_entity_index == projectile_slot
                })
                .ok_or("family-0 constructor returned without a live Fireball carrier")?;
            projectile.owner_entity_index = request.owner_slot;
            projectile.source_subspell_index = request.damage;
            projectile.target_source_entity_index = request.target_slot;
            projectile.target_class = terminal_class;
            projectile.target_model = terminal_model;
            projectile.position = position;
            projectile.yaw = yaw;
            projectile.roll = source_yaw;
            projectile.pitch = pitch;
            projectile.fov = pitch;
            Ok(Some((projectile_slot, source_yaw)))
        }
        SourceProduct::LightningRay | SourceProduct::LightningStormShot => {
            use super::lightning::{self, Product};
            let lightning_product = match product {
                SourceProduct::LightningRay => Product::Ray,
                SourceProduct::LightningStormShot => Product::StormShot,
                SourceProduct::Fireball { .. } => {
                    return Err("Fireball reached the Lightning constructor".to_owned());
                }
            };
            let Some(projectile_slot) =
                lightning::construct(arena, terrain, particles, lightning_product, mine.position)?
            else {
                return Ok(None);
            };
            arena.economy.prepare_sound(
                projectile_slot,
                request.owner_slot,
                -1,
                source_sound(product),
            );
            let projectile = arena
                .entities
                .get_mut(usize::from(projectile_slot))
                .filter(|projectile| projectile.index == projectile_slot && projectile.active)
                .ok_or("Lightning constructor returned without a live source row")?;
            let expected_model = match product {
                SourceProduct::LightningRay => 9,
                SourceProduct::LightningStormShot => 12,
                SourceProduct::Fireball { .. } => {
                    return Err("Fireball reached the Lightning constructor".to_owned());
                }
            };
            if (projectile.class, projectile.model) != (9, expected_model) {
                return Err("Lightning constructor returned the wrong family product".to_owned());
            }
            projectile
                .record
                .get_mut(71..73)
                .ok_or("Lightning product source selector fields are missing")?
                .copy_from_slice(&[terminal_class, terminal_model]);
            projectile.set_subspell_index(request.damage);
            projectile.source_id = request.owner_slot;
            projectile
                .record
                .get_mut(30..32)
                .ok_or("Lightning product owner link fields are missing")?
                .copy_from_slice(&request.owner_slot.to_le_bytes());
            projectile.set_source_word_96(request.target_slot);
            projectile.set_position(position);
            projectile.set_source_roll(source_yaw);
            projectile.set_fov(pitch);
            projectile.set_yaw(yaw);
            projectile.set_pitch(pitch);
            let completion_yaw = projectile.source_roll();
            let entity_rng = projectile.entity_rng;
            arena.economy.source_rng.insert(projectile_slot, entity_rng);
            crate::world::effects::sync_position(
                arena,
                terrain,
                projectile_slot,
                position,
                true,
                false,
            )?;
            Ok(Some((projectile_slot, completion_yaw)))
        }
    }
}

fn award_source_experience(
    shared: &mut Level2SharedSpellResetSession,
    arena: &SourceArena,
    experience: Option<(u16, u8, u8)>,
) -> Result<(), String> {
    let Some((owner_slot, family, amount)) = experience else {
        return Ok(());
    };
    if family != MAGIC_MINE_EXPERIENCE_FAMILY {
        return Err("Magic Mine Visit requested experience for a non-source family".to_owned());
    }
    let is_player = arena
        .entities
        .get(usize::from(owner_slot))
        .is_some_and(|owner| owner.index == owner_slot && owner.class == 3 && owner.model == 0)
        && shared
            .players()
            .iter()
            .any(|player| player.entity_index == owner_slot);
    if is_player {
        shared.award_spell_experience(owner_slot, family, i32::from(amount));
    }
    Ok(())
}

fn allocate_expiry_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    mana_drop: Option<field::ManaDrop>,
) -> Result<Option<u16>, String> {
    let Some(drop) = mana_drop else {
        return Ok(None);
    };
    let kind = match drop.model {
        0 => EffectKind::Impact,
        5 => EffectKind::Ash,
        model => return Err(format!("Magic Mine expiry requested source model {model}")),
    };
    effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(kind),
        drop.position,
        particles,
    )
}

/// Consume one complete model78 visit after `magic_mine_spell::step_field`.
/// Requests stay in source order; each successful constructor contributes its
/// live guidance roll, while a real null leaves the mine's charge untouched.
pub(crate) fn consume_visit(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    mine_slot: u16,
    captured: Option<CapturedSpell>,
    visit: &Visit,
) -> Result<ConsumeOutcome, String> {
    award_source_experience(shared, arena, visit.spell_experience)?;

    let mut charge_finished = visit.charge_finished;
    let mut projectiles = Vec::with_capacity(visit.projectiles.len());
    if !visit.projectiles.is_empty() {
        let mut completion_yaws = Vec::with_capacity(visit.projectiles.len());
        for request in visit.projectiles.iter().copied() {
            let spawned =
                construct_request(arena, terrain, particles, mine_slot, request, captured)?;
            if let Some((slot, yaw)) = spawned {
                projectiles.push(slot);
                // Source sub_3A8B0 writes the mine's yaw from the base
                // guidance result before applying a fanout-only shot offset.
                completion_yaws.push(Some(yaw));
            } else {
                completion_yaws.push(None);
            }
        }
        charge_finished |= super::magic_mine_spell::complete_field_projectiles(
            arena,
            mine_slot,
            &completion_yaws,
        )?;
    } else if visit.spell_experience.is_some() {
        return Err(
            "Magic Mine firing visit awarded experience without a target request".to_owned(),
        );
    }

    let expiry_effect = allocate_expiry_effect(arena, terrain, particles, visit.mana_drop)?;
    Ok(ConsumeOutcome {
        charge_finished,
        projectiles,
        expiry_effect,
        hidden: visit.hide,
    })
}

#[cfg(test)]
mod tests {
    use super::{SourceProduct, captured_profile, source_product};
    use crate::Position;
    use crate::world::fields::magic_mine::{CapturedSpell, ProjectileRequest};

    fn mine_row(family: u16, tier: u16) -> super::EntitySnapshot {
        let mut mine = crate::world::terrain::cave_features::new_event(3, 1, Position::default());
        mine.class = 10;
        mine.model = 78;
        mine.record[58..60].copy_from_slice(&family.to_le_bytes());
        mine.record[56..58].copy_from_slice(&tier.to_le_bytes());
        mine
    }

    fn request(family: u8, tier: u8) -> ProjectileRequest {
        ProjectileRequest {
            owner_slot: 1,
            target_slot: 2,
            target_position: Position::default(),
            family,
            tier,
            damage: 100,
            yaw_offset: 0,
            altitude_offset: 0,
        }
    }

    fn captured(family: u8, tier: u8, life: u8) -> CapturedSpell {
        CapturedSpell {
            family,
            tier,
            damage: 100,
            font_type: 1,
            life,
        }
    }

    #[test]
    fn source_dispatch_keeps_null_families_null_and_non_null_gaps_explicit() {
        assert_eq!(
            source_product(request(1, 0), Some(captured(1, 0, 0))),
            Ok(None)
        );
        assert_eq!(
            source_product(request(23, 0), Some(captured(23, 0, 0))),
            Ok(None)
        );
        assert_eq!(
            source_product(request(7, 0), Some(captured(7, 0, 3))),
            Ok(None)
        );
        assert!(source_product(request(9, 0), Some(captured(9, 0, 0))).is_err());
        assert_eq!(
            source_product(request(7, 2), Some(captured(7, 2, 2))),
            Ok(Some(SourceProduct::LightningStormShot))
        );
        assert!(
            source_product(
                request(7, 2),
                Some(CapturedSpell {
                    damage: 101,
                    ..captured(7, 2, 2)
                })
            )
            .is_err()
        );
    }

    #[test]
    fn captured_profile_resolves_owner_spell_rows_and_rejects_invalid_words() {
        let lightning = captured_profile(&mine_row(7, 2)).unwrap().unwrap();
        assert_eq!(
            lightning,
            CapturedSpell {
                family: 7,
                tier: 2,
                damage: 800,
                font_type: 0,
                life: 2,
            }
        );

        let source_owner_fireball = captured_profile(&mine_row(0, 2)).unwrap().unwrap();
        assert_eq!(source_owner_fireball.damage, 180);
        assert_eq!(source_owner_fireball.life, 2);

        let genuinely_null = captured_profile(&mine_row(23, 1)).unwrap().unwrap();
        assert_eq!(genuinely_null.damage, 5_000);
        let mut null_request = request(23, 1);
        null_request.damage = genuinely_null.damage;
        assert_eq!(source_product(null_request, Some(genuinely_null)), Ok(None));

        let unsupported_non_null = captured_profile(&mine_row(9, 0)).unwrap().unwrap();
        let mut meteor_request = request(9, 0);
        meteor_request.damage = unsupported_non_null.damage;
        assert!(source_product(meteor_request, Some(unsupported_non_null)).is_err());

        assert_eq!(captured_profile(&mine_row(u16::MAX, u16::MAX)), Ok(None));
        assert!(captured_profile(&mine_row(26, 0)).is_err());
        assert!(captured_profile(&mine_row(7, 3)).is_err());
    }
}
