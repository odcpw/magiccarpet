//! Ordinary physical family15 adapter for the shared guided flyer and the
//! scheduled model71 terrain controller.

use super::{CastInput, ParticleParameter, SessionTerrain};
use crate::{
    FlightWorld, Position, campaign::source_arena::SourceArena, move_entity,
    native_world::EntitySnapshot, world::terrain::disasters::mc2::tremor as terrain_tremor,
};

const RULES: sim_definition::level_definition::DisasterSpellProperties =
    sim_definition::level_definition::TREMOR_PROPERTIES;

pub(crate) const SOURCE_CONSTRUCTOR: super::super::guided_projectile::Constructor = super::super::guided_projectile::Constructor {
    model: 23, action: 24, parameter: 60, particle: 211,
    speed: RULES.projectile_speed, maximum_speed: RULES.maximum_launch_speed,
    life: RULES.projectile_range / RULES.projectile_speed as i32, mana: 50,
};

fn tier_rule(tier: u8) -> Result<sim_definition::level_definition::DisasterSpellTier, String> {
    RULES
        .tiers
        .get(usize::from(tier))
        .copied()
        .ok_or_else(|| format!("Tremor tier {tier} is outside its source table"))
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
}

/// sub_6DCA0 family15 allocation, followed by the source hand, aim and speed
/// fields. The shared guided dispatcher owns every movement and collision pass.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let (initial_parent_position, inherited_speed) = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| row.active && parent_slot != 0)
        .map(|row| (row.position, row.actual_speed))
        .ok_or("Tremor cast lost its active parent")?;
    let holder_tier = arena
        .entities
        .get(usize::from(holder_slot))
        .filter(|row| row.active && row.class == 15 && row.model == 15 && row.action == 45)
        .map(|row| row.level_gate)
        .ok_or("Tremor cast lost its family15 active holder")?;
    let tier = crate::spell_activation::level2_applied_tier(holder_tier)
        .ok_or("Tremor holder has an invalid source tier")?;
    let rule = tier_rule(tier)?;
    if RULES.projectile_speed <= 0
        || RULES.maximum_launch_speed < RULES.projectile_speed
        || RULES.projectile_range <= 0
    {
        return Err("Tremor source projectile rules are invalid".to_owned());
    }
    let Some(slot) = super::super::guided_projectile::construct(
        arena, terrain, particles, initial_parent_position, Some(parent_slot), inherited_speed,
        SOURCE_CONSTRUCTOR,
    )? else {
        return Ok(None);
    };
    let row = &mut arena.entities[usize::from(slot)];
    row.record[71] = 10;
    row.record[72] = 71;
    row.set_mana(50);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_parameter);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    row.record[42..44].copy_from_slice(&holder_slot.to_le_bytes());
    row.record[44..46].copy_from_slice(&parent_slot.to_le_bytes());


    // These source pointers are read again after NewEvent. Allocation may
    // have replaced either fixed-address row, so all launch-time parent and
    // holder fields below come from their current occupants.
    let (
        parent_position,
        parent_source_id,
        parent_control_flags,
        parent_yaw,
        parent_pitch,
        parent_fov,
    ) = arena
        .entities
        .get(usize::from(parent_slot))
        .map(|parent| {
            (
                parent.position,
                parent.source_id,
                parent.control_flags,
                parent.yaw,
                parent.pitch,
                parent.array_fov(),
            )
        })
        .ok_or("Tremor parent address disappeared after NewEvent")?;
    let holder_mana = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Tremor holder address disappeared after NewEvent")?
        .mana;

    // sub_68E50's hand offset and ground fallback precede the caster height
    // lift. Aim offsets and the source target axis are then retained on the
    // same fixed-address projectile row.
    let mut position = arena.entities[usize::from(slot)].position;
    let base_launch_position = position;
    let hand = if parent_control_flags & 0x100 != 0 {
        Some(parent_yaw.wrapping_sub(512) & 2047)
    } else if parent_control_flags & 0x200 != 0 {
        Some(parent_yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(hand_yaw) = hand {
        move_entity(&mut position, hand_yaw, 0, 256);
        if terrain.ground_height(position) > position.z {
            position = base_launch_position;
        }
    }
    position.z = position.z.wrapping_add(parent_fov);
    let yaw = input.aim_yaw.wrapping_add(parent_yaw);
    let pitch = input.aim_pitch.wrapping_add(parent_pitch);
    let mut axis = parent_position;
    move_entity(&mut axis, yaw, 0, RULES.aim_distance);
    axis.z = terrain.ground_height(axis);
    let projectile = &mut arena.entities[usize::from(slot)];
    projectile.set_position(position);
    set_owner(projectile, parent_source_id);
    projectile.set_mana(holder_mana);
    projectile.record[158..160].copy_from_slice(&axis.x.to_le_bytes());
    projectile.record[160..162].copy_from_slice(&axis.y.to_le_bytes());
    projectile.record[162..164].copy_from_slice(&axis.z.to_le_bytes());
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);

    arena.economy.source_rng.insert(slot, projectile.entity_rng);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// sub_6DCA0's source action24 impact consumer: convert the live carrier into
/// the scheduled model71 controller at the collision position.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if !projectile.active || (projectile.class, projectile.model) != (9, 23) {
        return Err("Tremor impact is not a live source9/model23 carrier".to_owned());
    }
    terrain_tremor::construct(arena, terrain, projectile.position)
}

/// The source action24 wrapper: shared movement/collision first, then
/// sub_677D0 reads byte46 from the current fixed-address flyer and binds the
/// lifetime onto the already-propagated model71 controller.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::super::guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<super::super::guided_projectile::StepOutcome, String> {
    if !arena
        .entities
        .get(usize::from(slot))
        .is_some_and(|row| row.active && (row.class, row.model, row.action) == (9, 23, 24))
    {
        return Err("Tremor action24 has no live source9/model23 projectile".to_owned());
    }
    let outcome = super::super::guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        super::super::guided_projectile::Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(impact_slot) = outcome.impact_slot {
        let impact_lifetime = arena
            .entities
            .get(usize::from(slot))
            .map(|projectile| projectile.record[74] & 0xf0)
            .ok_or("Tremor flyer address disappeared after guided impact")?;
        let impact = arena
            .entities
            .get_mut(usize::from(impact_slot))
            .filter(|row| row.active && (row.class, row.model, row.action) == (10, 71, 78))
            .ok_or("Tremor guided impact did not publish a live model71 controller")?;
        impact.set_source_byte_46(0);
        impact.set_max_life(i32::from(impact_lifetime));
        impact.set_life(i32::from(impact_lifetime));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_tremor_pickup_and_public_hand_reach_real_terrain_damage_and_reclaim() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, area_damage, source_target};

        let packages = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/group-zero-owner-package-v4");
        let mut source = CampaignSession::open(
            packages.join("level9"),
            CampaignSelection::ordinary(9).unwrap(),
        )
        .expect("unfiltered Level9 campaign");
        // Source RemoveCastleStage follows23→24→28 before publishing xtype2.
        // The campaign must retain that complete owner BUILD input closure;
        // a missing successor is not a successful demolition or reward.
        {
            let ordinary = &source.simulation().session.state.live.ordinary;
            for (selector, scalar, successor) in [(23, 17, 24), (24, 15, 28), (28, 7, 0)] {
                assert_eq!(
                    ordinary.house_parameters.get(&selector),
                    Some(&crate::house::HouseParameter::from_source_record(
                        scalar, 8, successor,
                    )),
                    "Level9 source BUILD{selector} successor input must be retained"
                );
                assert!(
                    ordinary.house_build_footprints.contains_key(&selector),
                    "Level9 source BUILD{selector} footprint must be retained"
                );
            }
        }
        let player = source.simulation().state.entity_index;
        // Qualify this package's actual Creature17 absence sensor. The clone
        // isolates that existing sensor/current text early; other sensors
        // are inactive only in this clone, not the acquisition session. No
        // condition, target, actor action or authored group is replaced. Contacts are explicit
        // lethal-damage controls on the unchanged initial action143 roots.
        {
            use crate::level_definition::{CreatureKind, ObjectiveCondition};
            let mut absence = source.simulation().clone();
            let sensor = absence
                .session
                .definition
                .semantic_level()
                .unwrap()
                .objectives()
                .iter()
                .position(|objective| objective.id().as_str() == "objective.campaign.task_5")
                .unwrap();
            assert_eq!(sensor, 4);
            assert_eq!(
                absence
                    .session
                    .definition
                    .semantic_level()
                    .unwrap()
                    .objectives()[sensor]
                    .condition(),
                ObjectiveCondition::NoLivingCreatures(CreatureKind::Creature17)
            );
            let owner = usize::from(absence.state.player_color_index);
            {
                let runtime = &mut absence.session.state.live.core.runtime;
                for stage in 0..usize::from(runtime.stage_index) {
                    runtime.set_level2_objective_stage_selector_for(
                        owner,
                        stage,
                        u8::from(stage == sensor),
                    );
                }
                runtime.set_level2_objective_text_selector_for(owner, sensor);
            }
            let roots = absence
                .entities()
                .iter()
                .filter(|row| row.active && (row.class, row.model) == (5, 17) && row.life >= 0)
                .map(|row| {
                    assert_eq!(row.action, 143);
                    row.index
                })
                .collect::<Vec<_>>();
            assert_eq!(roots, [67, 68]);
            let survivor = *roots.last().unwrap();
            let survivor_handle = absence.source_arena().handle(survivor).unwrap();
            for &slot in &roots[..roots.len() - 1] {
                let damage = u16::try_from(absence.entities()[usize::from(slot)].life + 1).unwrap();
                area_damage::contact(
                    &mut absence.session.state.live.economy,
                    None,
                    slot,
                    damage,
                    player,
                )
                .unwrap();
            }
            for _ in 0..32 {
                absence.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    !absence
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                    "real Creature17 sensor prerequisite: {:?}",
                    absence.dispatch_events()
                );
                assert_eq!(
                    absence.runtime().objectives[owner].stage_selectors[sensor],
                    1,
                    "one real living root keeps its actual absence objective incomplete"
                );
                if absence.source_arena().rebuilt_class5_model_slots()[17] == [survivor] {
                    break;
                }
            }
            assert_eq!(
                absence.source_arena().rebuilt_class5_model_slots()[17],
                [survivor]
            );
            assert!(absence.entities()[usize::from(survivor)].life > 0);
            let damage = u16::try_from(absence.entities()[usize::from(survivor)].life + 1).unwrap();
            area_damage::contact(
                &mut absence.session.state.live.economy,
                None,
                survivor,
                damage,
                player,
            )
            .unwrap();
            absence.step_outer_turn(PlayerCommand::default(), 0);
            assert!(absence.entities()[usize::from(survivor)].life < 0);
            assert!(
                absence.source_arena().rebuilt_class5_model_slots()[17].is_empty(),
                "a current recensus sees negative life; the objective still uses the captured head"
            );
            assert_eq!(
                absence.runtime().objectives[owner].stage_selectors[sensor],
                1,
                "same-pass negative life does not rewrite the frozen family head"
            );
            absence.step_outer_turn(PlayerCommand::default(), 0);
            assert!(absence.source_arena().rebuilt_class5_model_slots()[17].is_empty());
            assert_eq!(
                absence.runtime().objectives[owner].stage_selectors[sensor],
                2,
                "the next real empty family head completes the actual semantic sensor"
            );
            let mut hidden = false;
            for _ in 0..32 {
                let row = &absence.entities()[usize::from(survivor)];
                hidden |= row.active && row.drawing_disabled();
                if absence.source_arena().resolve(survivor_handle).is_none() {
                    break;
                }
                absence.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    !absence
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                    "real Creature17 following cleanup: {:?}",
                    absence.dispatch_events()
                );
            }
            assert!(
                hidden && absence.source_arena().resolve(survivor_handle).is_none(),
                "the actual initial actor completes prekill/death/hide/following reclaim"
            );
        }
        assert_eq!(
            source
                .simulation()
                .shared_spells()
                .local_player()
                .enabled_spell_holders[15],
            0
        );
        let mut saw_vault_birth = false;
        let mut demolished = std::collections::BTreeSet::new();
        let mut vault_transitions = Vec::new();
        // Root58 is a genuine initial area-exit switch. The initial player is
        // outside it; the ordinary numeric walk creates authored BUILD23/63.
        // Lethal source mailboxes are declared controls on that real vault and
        // any standing demolition successor, never a forced reward-group call.
        for _ in 0..256 {
            let simulation = source.simulation_mut();
            if simulation
                .economy()
                .spell_pickups()
                .iter()
                .any(|row| row.authored_index == 165)
            {
                break;
            }
            if let Some(slot) = simulation.economy().authored_house_source_slot(63) {
                saw_vault_birth = true;
                let generation = simulation.economy().slot_generation(slot);
                if let Some(house) = simulation
                    .ordinary()
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == slot)
                {
                    let transition = (
                        slot,
                        generation,
                        house.active,
                        house.action,
                        house.building_index,
                        house.xtype,
                        house.parameter.destruction_successor,
                    );
                    if vault_transitions.last() != Some(&transition) {
                        vault_transitions.push(transition);
                    }
                }
                let house = simulation
                    .ordinary()
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == slot && house.active && house.action == 52)
                    .copied();
                if let Some(house) = house
                    && demolished.insert((slot, generation, house.building_index))
                {
                    let damage =
                        u16::try_from(house.life + 1).expect("real vault life fits source damage");
                    assert!(
                        simulation
                            .session
                            .state
                            .live
                            .ordinary
                            .houses
                            .submit_source_damage(slot, damage, player)
                    );
                }
            }
            simulation.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                simulation
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "Level9 ordinary vault/reward path: {:?}",
                simulation.dispatch_events()
            );
        }
        assert!(
            saw_vault_birth && !demolished.is_empty(),
            "actual root58 must produce its destructible vault"
        );
        let pickup = source
            .simulation()
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.authored_index == 165)
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "actual vault demolition publishes authored Tremor165/group2: \
                     transitions={vault_transitions:?}, damaged={demolished:?}, current={:?}",
                    source
                        .simulation()
                        .ordinary()
                        .houses()
                        .iter()
                        .filter(|house| matches!(house.building_index, 23 | 24 | 28))
                        .collect::<Vec<_>>()
                )
            });
        for _ in 0..16 {
            let simulation = source.simulation_mut();
            let position = simulation.entities()[usize::from(pickup.source_entity_index)].position;
            simulation
                .set_level2_entity_position(player, position)
                .unwrap();
            simulation.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                simulation
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "{:?}",
                simulation.dispatch_events()
            );
            if simulation
                .shared_spells()
                .local_player()
                .enabled_spell_holders[15]
                != 0
            {
                break;
            }
        }
        assert_eq!(
            source
                .simulation()
                .shared_spells()
                .local_player()
                .enabled_spell_holders[15],
            pickup.source_entity_index,
            "ordinary overlap acquires the genuine Tremor reward"
        );
        let learned = source
            .simulation()
            .shared_spells()
            .local_player()
            .enabled_spell_holders
            .iter()
            .enumerate()
            .filter_map(|(family, holder)| (*holder != 0).then_some(family as u8))
            .collect::<Vec<_>>();

        for tier in 0_u8..=2 {
            let mut campaign =
                CampaignSession::open(packages.join("level2"), CampaignSelection::LEVEL2)
                    .expect("unfiltered destination with real Goat targets");
            assert_eq!(
                campaign
                    .rebuild_for_spell_carry(&learned, Some(15))
                    .unwrap(),
                Some(15)
            );
            let simulation = campaign.simulation_mut();
            let caster = simulation.state.entity_index;
            let rule = RULES.tiers[usize::from(tier)];
            // Castle construction/resources and progression are explicit cast
            // prerequisites. The acquired holder, hand and flyer are ordinary.
            for (stage, footprint) in
                crate::native_world::campaign_castle_stage_footprints().unwrap()
            {
                simulation
                    .economy_mut()
                    .set_castle_stage_footprint(stage, footprint);
            }
            let castle_site = (1_u16..32)
                .flat_map(|y| {
                    (1_u16..32).map(move |x| Position {
                        x: x * 2048,
                        y: y * 2048,
                        z: 0,
                    })
                })
                .find(|position| {
                    !simulation.economy().castle_projectile_site_blocked(
                        *position,
                        simulation.entities(),
                        simulation.canonical_terrain(),
                    )
                })
                .expect("owner terrain has a normal Castle site");
            let castle = simulation
                .economy_mut()
                .build_castle(castle_site, caster)
                .expect("canonical Castle constructor for the funding prerequisite");
            assert!(
                simulation
                    .economy_mut()
                    .set_castle_mana_prerequisite_for_test(
                        castle,
                        rule.castle_mana_required + 50_000
                    )
            );
            simulation
                .shared_spells_mut()
                .award_spell_experience(caster, 15, 5_000);
            {
                let live = &mut simulation.session.state.live;
                let parent = &mut live.economy.entities[usize::from(caster)];
                parent.max_mana = rule.mana_cost + 10_000;
                parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
                parent.set_mana(parent.max_mana);
                parent.set_mana_regen(0);
                let parent = parent.clone();
                live.shared_spells
                    .sync_player_entity_from_authority(&parent);
                live.shared_spells.player_mana = parent.mana;
                live.shared_spells.caster.mana = parent.mana;
                live.shared_spells
                    .sync_typed_spell_parent_caches_from_player_entity();
                if let Some(fire) = live.core.context.spell_fire.as_mut() {
                    fire.caster.mana = parent.mana;
                }
            }
            let ui = crate::SOURCE_SPELL_INDEX_MAP
                .iter()
                .position(|family| *family == 15)
                .unwrap() as i8;
            simulation.step_outer_turn(
                PlayerCommand {
                    opcode: 31,
                    argument_1: ui,
                    argument_2: tier as i8,
                    ..Default::default()
                },
                0,
            );
            let holder = simulation
                .shared_spells()
                .local_player()
                .enabled_spell_holders[15];
            assert_ne!(holder, 0);
            assert_eq!(
                simulation
                    .shared_spells()
                    .pickup(holder)
                    .unwrap()
                    .level_gate,
                tier as i8
            );

            // Choose a real current Goat and a clear cardinal approach from
            // the unchanged owner height field. No actor or terrain is forged.
            let (target, origin, yaw, aim) = simulation
                .goats()
                .iter()
                .filter(|goat| goat.life > 0)
                .flat_map(|goat| {
                    [0_i16, 512, 1024, 1536]
                        .into_iter()
                        .map(move |yaw| (goat, yaw))
                })
                .find_map(|(goat, yaw)| {
                    let mut origin = goat.position;
                    move_entity(
                        &mut origin,
                        yaw.wrapping_add(1024) & 2047,
                        0,
                        RULES.aim_distance,
                    );
                    let terrain = simulation.canonical_terrain();
                    let ground = terrain.ground_height(goat.position);
                    let maximum = (0..=16)
                        .map(|step| {
                            let mut point = origin;
                            move_entity(&mut point, yaw, 0, step * 256);
                            terrain.ground_height(point)
                        })
                        .max()
                        .unwrap();
                    if terrain.material_mask(goat.position) == 1
                        || maximum > ground.wrapping_add(1024)
                    {
                        return None;
                    }
                    origin.z = maximum.wrapping_add(768);
                    Some((
                        goat.entity_index,
                        origin,
                        yaw,
                        Position {
                            z: ground,
                            ..goat.position
                        },
                    ))
                })
                .expect("a real Goat admits an ordinary ground-bound Tremor approach");
            let target_handle = simulation.source_arena().handle(target).unwrap();
            simulation
                .set_level2_entity_position(caster, origin)
                .unwrap();
            simulation.state.actual_speed = 0;
            simulation.state.target_speed = 0;
            let (_, pitch) = crate::level1_mission::projectile_guidance_angles(origin, aim);
            simulation.set_level2_player_aim_for_test(yaw, pitch);
            let life_before = {
                let live = &simulation.session.state.live;
                source_target::read(
                    &live.economy,
                    source_target::Context::from_ordinary(&live.goats, &live.ordinary),
                    target,
                )
                .unwrap()
                .unwrap()
                .life
            };
            let xp_before = simulation.shared_spells().spell_experience()[15];
            let mana_before = simulation.entities()[usize::from(caster)].mana;
            let mut charged = false;
            let mut flyer = None;
            let mut controller = None;
            let mut controller_position = None;
            let mut hidden = [false; 2];
            let mut reclaimed = [false; 2];
            let mut presented = false;
            let mut mutated = false;
            let mut consumed_damage = false;
            let mut controller_damage = false;
            let mut late_clearance = false;
            let mut minimum_life = life_before;
            let mut sound = [false; 2];
            let mut prior_mana = mana_before;
            let clearance_bytes = simulation.session.definition.presentation.entity_parameters[66];
            let clearance = i16::from_le_bytes([clearance_bytes[12], clearance_bytes[13]]);
            area_damage::start_effect_damage_trace(target);
            for turn in 0..(usize::from(rule.impact_parameter & 0xf0) + 40) {
                let clearance_probe = controller
                    .and_then(|(slot, generation)| {
                        let row = &simulation.entities()[usize::from(slot)];
                        (simulation.economy().slot_generation(slot) == generation
                            && row.active
                            && !row.drawing_disabled()
                            && row.life > 0
                            && row.life & 1 != 0)
                            .then_some(row.position)
                    })
                    .filter(|_| !late_clearance);
                let old_probe_ground = clearance_probe
                    .map(|point| simulation.canonical_terrain().ground_height(point));
                if let Some(point) = clearance_probe {
                    // A normal zero-speed player visit precedes this actual
                    // odd-life rising controller. Only pose/flight input is
                    // staged; no terrain or effect callback is forced.
                    simulation
                        .set_level2_entity_position(
                            caster,
                            Position {
                                z: old_probe_ground.unwrap().wrapping_add(clearance),
                                ..point
                            },
                        )
                        .unwrap();
                    let live = &mut simulation.session.state.live;
                    let state = &mut live.core.state;
                    state.actual_speed = 0;
                    state.target_speed = 0;
                    state.strafe_speed = 0;
                    state.impulse_x = 0;
                    state.impulse_y = 0;
                    state.impulse_z = 0;
                    state.recoil_speed = 0;
                    state.display_pitch = 0;
                    state.effective_pitch = 0;
                    state.pitch_accumulator = 0;
                    live.economy.entities[usize::from(caster)].set_pitch(0);
                }
                let before_terrain = simulation
                    .canonical_terrain()
                    .planes()
                    .ground_heights
                    .clone();
                let movement = simulation.step_outer_turn(
                    if turn == 0 {
                        PlayerCommand {
                            flags: 0x10,
                            ..Default::default()
                        }
                    } else {
                        PlayerCommand::default()
                    },
                    0,
                );
                assert!(
                    simulation
                        .dispatch_events()
                        .iter()
                        .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                    "tier{tier} actual Tremor dispatch: {:?}",
                    simulation.dispatch_events()
                );
                let live = &simulation.session.state.live;
                let mana = live.economy.entities[usize::from(caster)].mana;
                charged |= prior_mana - mana == rule.mana_cost;
                prior_mana = mana;
                for row in live
                    .economy
                    .entities
                    .iter()
                    .filter(|row| row.active && row.source_id == caster)
                {
                    if (row.class, row.model, row.action) == (9, 23, 24) {
                        flyer.get_or_insert((row.index, live.economy.slot_generation(row.index)));
                        assert_eq!(row.subspell_index, rule.damage);
                    }
                    if (row.class, row.model, row.action) == (10, 71, 78) {
                        controller
                            .get_or_insert((row.index, live.economy.slot_generation(row.index)));
                        controller_position.get_or_insert(row.position);
                        assert_eq!(row.max_life, i32::from(rule.impact_parameter & 0xf0));
                        assert_eq!(
                            row.phase,
                            (row.index as u8).wrapping_add(
                                (row.max_life - row.life + i32::from(row.drawing_disabled())) as u8
                            )
                        );
                        assert_eq!(
                            row.control_flags & 1,
                            1,
                            "the terrain controller has no sprite"
                        );
                    }
                }
                for (index, saved) in [flyer, controller].into_iter().enumerate() {
                    if let Some((slot, generation)) = saved {
                        let same = live.economy.slot_generation(slot) == generation;
                        let row = &live.economy.entities[usize::from(slot)];
                        hidden[index] |= same && row.active && row.drawing_disabled();
                        reclaimed[index] |=
                            !same || live.economy.free_source_slots().contains(&slot);
                    }
                }
                if live.economy.resolve(target_handle).is_some() {
                    let view = source_target::read(
                        &live.economy,
                        source_target::Context::from_ordinary(&live.goats, &live.ordinary),
                        target,
                    )
                    .unwrap()
                    .unwrap();
                    minimum_life = minimum_life.min(view.life);
                    consumed_damage |= view.life < life_before
                        && simulation.goats().iter().any(|goat| {
                            goat.entity_index == target
                                && (goat.current_attacker == caster as i16
                                    || goat.kill_credit_entity == caster as i16)
                        });
                }
                for event in &simulation.shared_spells().events {
                    if let crate::FlightEvent::PrepareSound {
                        entity_index,
                        sound_id,
                        ..
                    } = event
                    {
                        sound[0] |= *entity_index == caster && *sound_id == 15;
                        sound[1] |= *sound_id == 10
                            && controller.is_some_and(|(slot, _)| slot == *entity_index);
                    }
                }
                for view in simulation.campaign_entity_presentations(0).unwrap() {
                    if flyer.is_some_and(|(slot, generation)| {
                        view.entity_index == slot
                            && live.economy.slot_generation(slot) == generation
                    }) {
                        assert_eq!((view.source_id, view.render_parameter_index), (caster, 211));
                        assert_eq!(
                            view.base_tmap,
                            simulation.session.definition.presentation.particles[211].base_tmap
                        );
                        presented = true;
                    }
                }
                if let Some((slot, generation)) = controller
                    && live.economy.slot_generation(slot) == generation
                {
                    let center = live.economy.entities[usize::from(slot)].position;
                    let tile = usize::from((center.x.wrapping_add(128) >> 8) as u8)
                        + 256 * usize::from((center.y.wrapping_add(128) >> 8) as u8);
                    mutated |= simulation.canonical_terrain().planes().ground_heights[tile]
                        != before_terrain[tile];
                }
                if let Some(point) = clearance_probe {
                    let ground = simulation.canonical_terrain().ground_height(point);
                    if ground > old_probe_ground.unwrap() {
                        let raw = &live.economy.entities[usize::from(caster)];
                        assert_eq!(raw.position.z, ground.wrapping_add(clearance));
                        assert_eq!(simulation.state.position, raw.position);
                        assert_eq!(
                            simulation.shared_spells().player_entity.position,
                            raw.position
                        );
                        assert!(
                            simulation
                                .shared_spells()
                                .player_entities
                                .iter()
                                .filter(|player| player.index == caster)
                                .all(|player| player.position == raw.position)
                        );
                        assert!(
                            movement
                                .iter()
                                .any(|body| body.committed.z < raw.position.z),
                            "source585D0 clamps after the actual earlier player body"
                        );
                        late_clearance = true;
                    }
                }
                if reclaimed.iter().all(|done| *done) {
                    break;
                }
            }
            let damage_receipts = area_damage::take_effect_damage_trace();
            let receipt_observations = damage_receipts
                .iter()
                .map(|receipt| {
                    (
                        receipt.pulse.owner,
                        receipt.pulse.position,
                        receipt.pulse.payload,
                        receipt.mailbox_before,
                        receipt.mailbox_after,
                        receipt.hit_count,
                        receipt.target_slot,
                        receipt.target_in_scanned_cells,
                    )
                })
                .collect::<Vec<_>>();
            for receipt in damage_receipts {
                controller_damage |= receipt.pulse.owner == caster
                    && controller_position == Some(receipt.pulse.position)
                    && receipt.pulse.payload
                        == 4 * (rule.damage / u16::from(rule.impact_parameter & 0xf0))
                    && receipt
                        .mailbox_after
                        .is_some_and(|(damage, owner)| damage > 0 && owner == caster)
                    && receipt.hit_count.is_some_and(|hits| hits > 0);
            }
            assert!(
                charged
                    && presented
                    && mutated
                    && consumed_damage
                    && controller_damage
                    && late_clearance,
                "tier{tier}: charge={charged}, sprite={presented}, terrain={mutated}, damage={consumed_damage}/{controller_damage}, clearance={late_clearance}, targetlife={life_before}->{minimum_life}, flyer={flyer:?}, controller={controller:?}, controller_position={controller_position:?}, receipts={receipt_observations:?}"
            );
            assert!(
                simulation.shared_spells().spell_experience()[15] > xp_before,
                "real scheduled Tremor hits award XP15"
            );
            assert_eq!(
                hidden, [true; 2],
                "both actual generations hide before reclaim"
            );
            assert_eq!(
                reclaimed, [true; 2],
                "both actual generations reach following source reclaim"
            );
            assert_eq!(
                sound, [true; 2],
                "source launch15 and scheduled Tremor10 reach audio requests"
            );
        }
    }

    #[test]
    fn guided_impact_keeps_mutated_payload_and_masks_lifetime() {
        let mut session = super::super::super::tests::level_two_campaign_session();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let holder = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.model == 3)
            .unwrap()
            .index;
        let mut row = live.shared_spells.pickup(holder).unwrap().clone();
        row.model = 15;
        row.record[68] = 15;
        row.set_source_action(45);
        row.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(15, 0).unwrap(),
            0,
        );
        live.economy.entities[usize::from(holder)] = row;
        let flyer = launch(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            holder,
            parent,
            CastInput::default(),
        )
        .unwrap()
        .unwrap();
        let mut position = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|position| {
                live.terrain.material_mask(*position) != 1
                    && live.terrain.ground_height(*position) > 32
            })
            .unwrap();
        position.z = live.terrain.ground_height(position).wrapping_sub(1);
        let row = &mut live.economy.entities[usize::from(flyer)];
        row.set_position(position);
        row.set_actual_speed(0);
        row.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        // A reflected or otherwise mutated payload remains source data;
        // the bare controller constructor must not reject it as a bad tier.
        row.set_subspell_index(4321);
        row.set_source_byte_46(0x9b);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            position,
            true,
            false,
        )
        .unwrap();
        let visit = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let child = visit
            .impact_slot
            .expect("guided ground collision creates Tremor");
        let row = &live.economy.entities[usize::from(child)];
        assert_eq!((row.class, row.model, row.action), (10, 71, 78));
        assert_eq!((row.record[74], row.max_life, row.life), (0, 0x90, 0x90));
        assert_eq!(
            (row.source_id, row.subspell_index),
            (live.economy.entities[usize::from(parent)].source_id, 4321)
        );

        // Continue through the ordinary source scheduler: the resulting
        // controller must change the terrain sampled by flight, not just
        // produce a model71 row.
        let before = session.clone();
        let phase = before.session.state.live.economy.entities[usize::from(child)].phase;
        let center = before.session.state.live.economy.entities[usize::from(child)].position;
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled Tremor must not roll back: {:?}",
            session.dispatch_events()
        );
        let live = &session.session.state.live;
        assert_eq!(
            live.economy.entities[usize::from(child)].phase,
            phase.wrapping_add(1)
        );
        let previous_terrain = &before.session.state.live.terrain;
        let center_x = (center.x.wrapping_add(128) >> 8) as u8;
        let center_y = (center.y.wrapping_add(128) >> 8) as u8;
        let changed = (-2_i8..=2)
            .flat_map(|dy| (-2_i8..=2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                let x = center_x.wrapping_add_signed(dx);
                let y = center_y.wrapping_add_signed(dy);
                Position {
                    x: u16::from(x) << 8,
                    y: u16::from(y) << 8,
                    z: 0,
                }
            })
            .find(|&point| {
                previous_terrain.ground_height(point) != live.terrain.ground_height(point)
            });
        assert!(
            changed.is_some(),
            "scheduled Tremor changes flight terrain sampling"
        );
    }
}
