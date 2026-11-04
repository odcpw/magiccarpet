//! Family-16 Crater projectile and its terrain-ring impact.
//!
//! The holder in `disaster_spells` owns charge/countdown. This leaf owns only
//! the source 9/5/5 flight callback and construction of its 10/11 child; the
//! child then runs through the ordinary terrain-disaster scheduler.

use super::{CastInput, EntitySnapshot, ParticleParameter, SessionTerrain};
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    level_definition::{CRATER_PROPERTIES as RULES, TerrainDisasterKind},
    move_entity,
    native_world::guided_projectile::{self, Guidance, StepOutcome},
};

pub(crate) const SOURCE_CONSTRUCTOR: guided_projectile::Constructor = guided_projectile::Constructor {
    model: 5, action: 5, parameter: 60, particle: 211,
    speed: RULES.projectile_speed, maximum_speed: RULES.maximum_launch_speed,
    life: RULES.projectile_range / RULES.projectile_speed as i32, mana: 50,
};

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put(row, 30, owner);
}

/// sub_6BAB0 and the family-16 branch of sub_6DCA0.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let (parent_position, inherited_speed) = {
        let parent = arena
            .entities
            .get(usize::from(parent_slot))
            .ok_or("Crater parent address is outside source arena")?;
        (parent.position, parent.actual_speed)
    };
    let holder_tier = arena
        .entities
        .get(usize::from(holder_slot))
        .filter(|row| row.class == 15 && row.model == 16)
        .ok_or("Crater holder is absent or has a different source family")?
        .level_gate;
    let tier = crate::spell_activation::level2_applied_tier(holder_tier)
        .ok_or("Crater holder has a negative source tier")?;
    let rule = RULES
        .tiers
        .get(usize::from(tier))
        .copied()
        .ok_or("Crater tier is outside its source table")?;
    let Some(slot) = guided_projectile::construct(
        arena, terrain, particles, parent_position, Some(parent_slot), inherited_speed,
        SOURCE_CONSTRUCTOR,
    )? else {
        // The shared holder still charges and advances its countdown after
        // this source NewEvent failure.
        return Ok(None);
    };
    let row = &mut arena.entities[usize::from(slot)];
    row.record[71] = 10;
    row.record[72] = 11;
    put(row, 42, holder_slot);
    // sub_4D620's constructor value is overwritten from the live holder after
    // NewEvent by sub_6BAB0. This matters when allocation aliases that address.
    row.set_mana(50);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_parameter);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));


    // sub_6BAB0 reads these fixed addresses after NewEvent. Allocation can
    // replace either one, so reborrow their current occupants before applying
    // holder mana, owner, hand offset, orientation and target axis.
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
        .ok_or("Crater parent address disappeared after NewEvent")?;
    let holder_mana = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Crater holder address disappeared after NewEvent")?
        .mana;

    let mut launch_position = arena.entities[usize::from(slot)].position;
    let hand_yaw = if parent_control_flags & 0x100 != 0 {
        Some(parent_yaw.wrapping_sub(512) & 2047)
    } else if parent_control_flags & 0x200 != 0 {
        Some(parent_yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand_yaw {
        move_entity(&mut launch_position, yaw, 0, 256);
        if terrain.ground_height(launch_position) > launch_position.z {
            launch_position = arena.entities[usize::from(slot)].position;
        }
    }
    launch_position.z = launch_position.z.wrapping_add(parent_fov);

    let yaw = input.aim_yaw.wrapping_add(parent_yaw);
    let pitch = input.aim_pitch.wrapping_add(parent_pitch);
    let mut aim = parent_position;
    move_entity(&mut aim, yaw, 0, RULES.aim_distance);
    aim.z = terrain.ground_height(aim);

    let projectile = &mut arena.entities[usize::from(slot)];
    set_owner(projectile, parent_source_id);
    projectile.set_mana(holder_mana);
    projectile.set_position(launch_position);
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);
    projectile.record[158..160].copy_from_slice(&aim.x.to_le_bytes());
    projectile.record[160..162].copy_from_slice(&aim.y.to_le_bytes());
    projectile.record[162..164].copy_from_slice(&aim.z.to_le_bytes());
    crate::world::effects::sync_position(arena, terrain, slot, launch_position, true, false)?;
    Ok(Some(slot))
}

/// sub_66280: run shared sub_65820, then apply Crater's child-life override.
/// Phase advancement and following-pass reclaim remain with the scheduler.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&guided_projectile::Targets<'_>>,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<StepOutcome, String> {
    let outcome = guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
        context,
    )?;
    if let Some(impact_slot) = outcome.impact_slot {
        // sub_66280 reads byte46 after sub_65820 returns. The fixed source
        // address may now carry payload written during impact construction.
        let impact_life = arena
            .entities
            .get(usize::from(slot))
            .ok_or_else(|| format!("Crater projectile address left the arena at slot {slot}"))?
            .record[74];
        let impact = arena
            .entities
            .get_mut(usize::from(impact_slot))
            .ok_or("Crater impact address is outside source arena")?;
        // sub_66280 writes life only; model11 keeps its constructor maxLife40.
        impact.set_life(i32::from(impact_life));
    }
    Ok(outcome)
}

/// Family-16 child selector (10,11): construct the source-default LoweringRing
/// through the existing callback/kernel; the shared impact path copies the
/// projectile payload only after this bare constructor has returned.
pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    if (projectile.class, projectile.model, projectile.action) != (9, 5, 5)
        || (projectile.record[71], projectile.record[72]) != (10, 11)
    {
        return Err("Crater impact constructor received a different source selector".into());
    }
    let definition =
        sim_definition::adapter::mc2_terrain_disaster_definition(TerrainDisasterKind::LoweringRing);
    let Some(slot) = crate::world::terrain::disasters::mc2::runtime::construct_defined(
        arena,
        terrain,
        projectile.position,
        definition,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_disaster_pickups_and_public_hands_reach_terrain_and_damage() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, source_target};
        use crate::level_definition::{CRATER_PROPERTIES, EARTHQUAKE_PROPERTIES, VOLCANO_PROPERTIES};

        let private = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let package = std::env::var_os("MC_DISASTER_LEVEL15_PACKAGE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| private.join("generated/mc-bhq6-ordinary-20261025-zclu5fjj/level15"));
        let mut source = CampaignSession::open(package, CampaignSelection::ordinary(15).unwrap())
            .expect("unfiltered Level15 owner campaign opens");
        let player = source.simulation().state.entity_index;
        let initial = [(129_u16, 16_u8, 50_u8), (128, 17, 53), (125, 18, 56)];
        for (slot, family, action) in initial {
            let pickup = source.simulation().shared_spells().pickup(slot)
                .filter(|row| (row.class, row.model, row.action) == (15, family, action))
                .expect("actual initial Level15 pickup");
            assert_eq!(pickup.source_spell_parent_entity_index(), 0);
        }
        let mut acquired = std::collections::BTreeSet::new();
        for (slot, family, _) in initial {
            let mut visits = Vec::new();
            for _ in 0..16 {
                let simulation = source.simulation_mut();
                if simulation.shared_spells().local_player().enabled_spell_holders[usize::from(family)] != 0 {
                    break;
                }
                let position = simulation.entities()[usize::from(slot)].position;
                simulation.set_level2_entity_position(player, position).unwrap();
                simulation.step_outer_turn(PlayerCommand::default(), 0);
                for event in simulation.dispatch_events() {
                    if let Mc2DispatchEvent::SpellAcquisition { source_slot, family, result: Some(outcome), .. } = event
                        && outcome.parent_entity_index == player
                    {
                        acquired.insert((*source_slot, *family));
                    }
                }
                let current = simulation.shared_spells().pickup(slot).unwrap();
                visits.push((position, simulation.state.position, current.position, current.phase,
                    current.control_flags, current.action, current.life,
                    simulation.shared_spells().player_entity().life,
                    simulation.dispatch_events().iter().filter(|event| matches!(event,
                        Mc2DispatchEvent::SpellAcquisition { source_slot, .. } if *source_slot == slot
                    )).cloned().collect::<Vec<_>>()));
                assert!(simulation.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                    "Level15 actual pickup dispatch: {:?}", simulation.dispatch_events());
            }
            assert_ne!(source.simulation().shared_spells().local_player().enabled_spell_holders[usize::from(family)], 0,
                "ordinary overlap acquires family{family} from its actual Level15 pickup; visits={visits:?}");
            assert!(acquired.contains(&(slot, family)),
                "the actual initial slot{slot} must acquire family{family}: {acquired:?}");
        }
        let learned = source.simulation().shared_spells().local_player().enabled_spell_holders
            .iter().enumerate().filter_map(|(family, holder)| (*holder != 0).then_some(family as u8))
            .collect::<Vec<_>>();
        let level2 = private.join("generated/ribbon-20261104-integration/group-zero-owner-package-v4/level2");
        for (family, properties, flyer_model, flyer_action, impact_model, impact_action, particle) in [
            (16_u8, CRATER_PROPERTIES, 5_u8, 5_u8, 11_u8, 11_u8, 211_u16),
            (17, EARTHQUAKE_PROPERTIES, 2, 2, 15, 15, 211),
            (18, VOLCANO_PROPERTIES, 4, 4, 9, 9, 210),
        ] {
            for tier in 0_u8..=2 {
                let mut campaign = CampaignSession::open(&level2, CampaignSelection::LEVEL2)
                    .expect("unfiltered Level2 target campaign opens");
                assert_eq!(campaign.rebuild_for_spell_carry(&learned, Some(family)).unwrap(), Some(family));
                let simulation = campaign.simulation_mut();
                let caster = simulation.state.entity_index;
                let rule = properties.tiers[usize::from(tier)];
                for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
                    simulation.economy_mut().set_castle_stage_footprint(stage, footprint);
                }
                let castle_site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
                    x: x * 2048, y: y * 2048, z: 0,
                })).find(|position| !simulation.economy().castle_projectile_site_blocked(
                    *position, simulation.entities(), simulation.canonical_terrain()))
                    .expect("owner terrain has a normal Castle site");
                let castle = simulation.economy_mut().build_castle(castle_site, caster)
                    .expect("normal Castle constructor");
                assert!(simulation.economy_mut().set_castle_mana_prerequisite_for_test(
                    castle, rule.castle_mana_required + 50_000));
                simulation.shared_spells_mut().award_spell_experience(caster, family, 5_000);
                {
                    let live = &mut simulation.session.state.live;
                    let parent = &mut live.economy.entities[usize::from(caster)];
                    parent.max_mana = rule.mana_cost + 10_000;
                    parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
                    parent.set_mana(parent.max_mana);
                    parent.set_mana_regen(0);
                    let parent = parent.clone();
                    live.shared_spells.sync_player_entity_from_authority(&parent);
                    live.shared_spells.player_mana = parent.mana;
                    live.shared_spells.caster.mana = parent.mana;
                    live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
                    if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
                }
                let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|value| *value == family).unwrap() as i8;
                simulation.step_outer_turn(PlayerCommand { opcode: 31, argument_1: ui,
                    argument_2: tier as i8, ..Default::default() }, 0);
                let holder = simulation.shared_spells().local_player().enabled_spell_holders[usize::from(family)];
                assert_ne!(holder, 0);
                assert_eq!(simulation.shared_spells().pickup(holder).unwrap().level_gate, tier as i8);

                let (target, origin, yaw, aim) = simulation.goats().iter().filter(|goat| goat.life > 0)
                    .flat_map(|goat| [0_i16, 512, 1024, 1536].into_iter().map(move |yaw| (goat, yaw)))
                    .find_map(|(goat, yaw)| {
                        let mut origin = goat.position;
                        crate::move_entity(&mut origin, yaw.wrapping_add(1024) & 2047, 0, properties.aim_distance);
                        let terrain = simulation.canonical_terrain();
                        let ground = terrain.ground_height(goat.position);
                        let maximum = (0..=16).map(|step| {
                            let mut point = origin;
                            crate::move_entity(&mut point, yaw, 0, step * 256);
                            terrain.ground_height(point)
                        }).max().unwrap();
                        if terrain.material_mask(goat.position) == 1 || maximum > ground.wrapping_add(1024) {
                            return None;
                        }
                        origin.z = maximum.wrapping_add(768);
                        Some((goat.entity_index, origin, yaw, Position { z: ground, ..goat.position }))
                    }).expect("an actual Goat has an open ground approach");
                let target_handle = simulation.source_arena().handle(target).unwrap();
                simulation.set_level2_entity_position(caster, origin).unwrap();
                simulation.state.actual_speed = 0;
                simulation.state.target_speed = 0;
                let (_, pitch) = crate::level1_mission::projectile_guidance_angles(origin, aim);
                simulation.set_level2_player_aim_for_test(yaw, pitch);
                let life_before = {
                    let live = &simulation.session.state.live;
                    source_target::read(&live.economy,
                        source_target::Context::from_ordinary(&live.goats, &live.ordinary), target)
                        .unwrap().unwrap().life
                };
                let xp_before = simulation.shared_spells().spell_experience()[usize::from(family)];
                let mut prior_mana = simulation.entities()[usize::from(caster)].mana;
                let mut charged = false;
                let mut flyer = None;
                let mut impact = None;
                let mut hidden = [false; 2];
                let mut reclaimed = [false; 2];
                let mut previous_phase = [None; 2];
                let mut phase_advanced = [false; 2];
                let mut presented = false;
                let mut damage = false;
                let mut mutated = false;
                let mut launch_sound = false;
                let mut descendant = false;
                for turn in 0..160 {
                    let before = simulation.canonical_terrain().planes().ground_heights.clone();
                    simulation.step_outer_turn(if turn == 0 {
                        PlayerCommand { flags: 0x10, ..Default::default() }
                    } else { PlayerCommand::default() }, 0);
                    assert!(simulation.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                        "family{family} tier{tier} dispatch: {:?}", simulation.dispatch_events());
                    let live = &simulation.session.state.live;
                    let mana = live.economy.entities[usize::from(caster)].mana;
                    charged |= prior_mana - mana == rule.mana_cost;
                    prior_mana = mana;
                    for row in live.economy.entities.iter().filter(|row| row.active && row.source_id == caster) {
                        if (row.class, row.model, row.action) == (9, flyer_model, flyer_action) {
                            flyer.get_or_insert((row.index, live.economy.slot_generation(row.index)));
                            assert_eq!(row.subspell_index, rule.damage);
                        }
                        if (row.class, row.model, row.action) == (10, impact_model, impact_action) {
                            impact.get_or_insert((row.index, live.economy.slot_generation(row.index)));
                            assert_eq!(row.subspell_index, rule.damage);
                        }
                        descendant |= match family {
                            17 => (row.class, row.model, row.action) == (10, 11, 11),
                            18 => (row.class, row.model, row.action) == (10, 18, 18),
                            _ => false,
                        };
                    }
                    for (i, saved) in [flyer, impact].into_iter().enumerate() {
                        if let Some((slot, generation)) = saved {
                            let same = live.economy.slot_generation(slot) == generation;
                            let row = &live.economy.entities[usize::from(slot)];
                            hidden[i] |= same && row.active && row.drawing_disabled();
                            if same && row.active {
                                if let Some(previous) = previous_phase[i] {
                                    assert_eq!(row.phase, u8::wrapping_add(previous, 1),
                                        "family{family} tier{tier} carrier{i} source phase");
                                    phase_advanced[i] = true;
                                }
                                previous_phase[i] = Some(row.phase);
                            }
                            reclaimed[i] |= !same || live.economy.free_source_slots().contains(&slot);
                        }
                    }
                    if let Some((slot, generation)) = impact
                        && live.economy.slot_generation(slot) == generation
                    {
                        let center = live.economy.entities[usize::from(slot)].position;
                        let x = i32::from(center.x >> 8);
                        let y = i32::from(center.y >> 8);
                        mutated |= (-8_i32..=8).flat_map(|dx| (-8_i32..=8).map(move |dy| (dx, dy)))
                            .any(|(dx, dy)| {
                                let tile = usize::from((x + dx) as u8) + 256 * usize::from((y + dy) as u8);
                                before[tile] != simulation.canonical_terrain().planes().ground_heights[tile]
                            });
                    }
                    if live.economy.resolve(target_handle).is_some() {
                        let view = source_target::read(&live.economy,
                            source_target::Context::from_ordinary(&live.goats, &live.ordinary), target)
                            .unwrap().unwrap();
                        damage |= view.life < life_before && simulation.goats().iter().any(|goat| {
                            goat.entity_index == target && (goat.current_attacker == caster as i16
                                || goat.kill_credit_entity == caster as i16)
                        });
                    }
                    for event in &live.shared_spells.events {
                        if let crate::FlightEvent::PrepareSound { entity_index, sound_id, .. } = event {
                            launch_sound |= *entity_index == caster && *sound_id == 15;
                        }
                    }
                    for view in simulation.campaign_entity_presentations(0).unwrap() {
                        if flyer.is_some_and(|(slot, generation)| view.entity_index == slot
                            && live.economy.slot_generation(slot) == generation) {
                            assert_eq!((view.source_id, view.render_parameter_index), (caster, particle));
                            assert_eq!(view.base_tmap,
                                simulation.session.definition.presentation.particles[usize::from(particle)].base_tmap);
                            presented = true;
                        }
                    }
                    if reclaimed.iter().all(|done| *done) { break; }
                }
                assert!(charged && presented && launch_sound && mutated && damage,
                    "family{family} tier{tier}: charge={charged} sprite={presented} sound={launch_sound} terrain={mutated} damage={damage} flyer={flyer:?} impact={impact:?} descendant={descendant}");
                assert!(simulation.shared_spells().spell_experience()[usize::from(family)] > xp_before,
                    "family{family} actual damage awards experience");
                if family != 16 { assert!(descendant, "family{family} schedules its actual child"); }
                assert_eq!(hidden, [true; 2], "family{family} flyer and impact hide before reclaim");
                assert_eq!(reclaimed, [true; 2], "family{family} flyer and impact reach following reclaim");
                assert_eq!(phase_advanced, [true; 2], "family{family} carriers advance through normal visits");
            }
        }
    }

    #[test]
    fn disaster_holder_adapter_preserves_failed_allocation_and_missing_parent() {
        use crate::level_definition::{CRATER_PROPERTIES, EARTHQUAKE_PROPERTIES, VOLCANO_PROPERTIES};
        for (family, properties) in [(16_u8, CRATER_PROPERTIES), (17, EARTHQUAKE_PROPERTIES), (18, VOLCANO_PROPERTIES)] {
            for parent_missing in [false, true] {
                let mut simulation = super::super::super::tests::level_two_campaign_session();
                let particles = simulation.session.definition.presentation.particles.clone();
                let live = &mut simulation.session.state.live;
                let parent = live.core.state.entity_index;
                let slot = live.shared_spells.pickups().iter().find(|row| row.model == 3).unwrap().index;
                let rule = properties.tiers[0];
                let holder = live.shared_spells.pickup_mut(slot).unwrap();
                holder.model = family;
                holder.record[68] = family;
                holder.set_source_action(family * 3);
                holder.active_value = 0;
                holder.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
                holder.apply_nyphur_spell_profile(
                    crate::spell_activation::level2_set_spell_profile(family, 0).unwrap(), 0);
                holder.active_value = holder.activation_value as i16;
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                holder.record[44..46].copy_from_slice(&(if parent_missing { 0 } else { parent }).to_le_bytes());
                holder.record[58..60].copy_from_slice(&7_u16.to_le_bytes());
                let start = holder.active_value;
                let phase = holder.phase;
                live.economy.entities[usize::from(parent)].set_mana(rule.mana_cost);
                live.economy.entities[usize::from(parent)].set_mana_regen(0);
                if !parent_missing {
                    // Controlled allocator exhaustion exercises source NewEvent failure;
                    // it is separate from the ordinary acquired-cast proof above.
                    for row in &mut live.economy.entities {
                        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
                    }
                    live.economy.economy.exhaust_new_event_capacity_for_test();
                    live.economy.rebuild_source_free_arena();
                }
                let outcome = super::super::step_holder(
                    &mut live.shared_spells, &mut live.economy, &mut live.terrain, &particles, slot,
                    super::super::CastInput { castle_mana: Some(rule.castle_mana_required),
                        auxiliary_byte: 3, ..Default::default() },
                ).unwrap();
                assert_eq!(outcome, super::super::HolderOutcome::default());
                assert_eq!(live.economy.entities[usize::from(parent)].mana_regen,
                    if parent_missing { 0 } else { -rule.mana_cost },
                    "family{family} charges an admitted failed birth, but not a missing parent");
                let holder = live.shared_spells.pickup(slot).unwrap();
                assert_eq!(holder.active_value, start - 1);
                assert_eq!(u16::from_le_bytes([holder.record[58], holder.record[59]]), 6);
                assert_eq!(holder.phase, phase, "scheduler owns the common phase tail");
            }
        }
    }

    #[test]
    fn guided_crater_impact_uses_current_byte46_and_preserves_mutated_payload() {
        let mut session = super::super::super::tests::level_two_campaign_session();
        let particles = session.session.definition.presentation.particles.clone();
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let holder = live
            .shared_spells
            .pickups()
            .iter()
            .find(|row| row.model == 3)
            .expect("Level2 source holder fixture")
            .index;
        let mut row = live
            .shared_spells
            .pickup(holder)
            .expect("Level2 source holder row")
            .clone();
        row.model = 16;
        row.record[68] = 16;
        row.set_source_action(48);
        row.apply_nyphur_spell_profile(
            crate::spell_activation::level2_set_spell_profile(16, 0)
                .expect("Crater level-zero profile"),
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
        .expect("source Crater projectile allocation");
        let source = &live.economy.entities[usize::from(flyer)];
        assert_eq!((source.class, source.model, source.action), (9, 5, 5));
        assert_eq!(
            (source.particle_parameter_index, source.parameter_relation),
            (Some(211), Some(60))
        );
        assert_eq!(
            u32::from_le_bytes(source.record[164..168].try_into().unwrap()),
            61
        );
        let real_owner = source.source_id;
        let mut scheduled = session.clone();
        let live = &mut session.session.state.live;

        let mut position = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|position| live.terrain.material_mask(*position) != 1)
            .expect("Level2 fixture has a dry terrain cell");
        assert_ne!(live.terrain.material_mask(position), 1);
        position.z = live.terrain.ground_height(position).wrapping_sub(1);

        const MUTATED_OWNER: u16 = 0x4567;
        const MUTATED_DAMAGE: u16 = 4321;
        const MUTATED_BYTE46: u8 = 61;
        let source = &mut live.economy.entities[usize::from(flyer)];
        source.set_position(position);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        source.source_id = MUTATED_OWNER;
        source.record[30..32].copy_from_slice(&MUTATED_OWNER.to_le_bytes());
        source.set_subspell_index(MUTATED_DAMAGE);
        source.set_source_byte_46(MUTATED_BYTE46);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            position,
            true,
            false,
        )
        .unwrap();

        let outcome = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let child_slot = outcome
            .impact_slot
            .expect("dry terrain collision constructs Crater");
        let current_flyer_byte46 = live.economy.entities[usize::from(flyer)].record[74];
        let child = &live.economy.entities[usize::from(child_slot)];
        assert_eq!((child.class, child.model, child.action), (10, 11, 11));
        assert_eq!(child.life, i32::from(current_flyer_byte46));
        assert_eq!(child.max_life, 40, "bare LoweringRing maxLife is retained");
        assert_eq!(child.record[74], current_flyer_byte46);
        assert_eq!(child.subspell_index, MUTATED_DAMAGE);
        assert_eq!(child.source_id, MUTATED_OWNER);

        // The same flyer on retained water follows the source splash path,
        // without producing a terrain-lowering ring.
        let mut water_case = scheduled.clone();
        let water = &mut water_case.session.state.live;
        let mut water_point = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|point| water.terrain.material_mask(*point) == 1)
            .expect("retained Level2 terrain has water");
        water_point.z = water.terrain.ground_height(water_point).wrapping_sub(1);
        let source = &mut water.economy.entities[usize::from(flyer)];
        source.set_position(water_point);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        crate::world::effects::sync_position(
            &mut water.economy,
            &mut water.terrain,
            flyer,
            water_point,
            true,
            false,
        )
        .unwrap();
        let water_impact = step_projectile(
            &mut water.economy,
            &mut water.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(
                &water.goats,
                &water.ordinary,
            ),
        )
        .unwrap();
        assert!(water_impact.impact_slot.is_none());
        assert!(water.economy.entities[usize::from(flyer)].drawing_disabled());

        // Keep the source-owner branch separate from the payload mutation
        // control above, then follow the real impact into its scheduled
        // model11 callback and the flight world's current ground height.
        let live = &mut scheduled.session.state.live;
        let mut point = (0..65536_u32)
            .map(|tile| Position {
                x: ((tile & 255) << 8) as u16,
                y: ((tile >> 8) << 8) as u16,
                z: 0,
            })
            .find(|point| {
                let x = usize::from(point.x >> 8);
                let y = usize::from(point.y >> 8);
                (2..=253).contains(&x)
                    && (2..=253).contains(&y)
                    && live.terrain.material_mask(*point) != 1
                    && (-2_isize..=2).all(|dx| {
                        (-2_isize..=2).all(|dy| {
                            let index =
                                (x as isize + dx) as usize + 256 * (y as isize + dy) as usize;
                            live.terrain.planes().angles[index] & 15 != 0
                                && live.terrain.planes().ground_heights[index] > 4
                        })
                    })
            })
            .expect("retained Level2 terrain has elevated dry ground");
        point.z = live.terrain.ground_height(point).wrapping_sub(1);
        let source = &mut live.economy.entities[usize::from(flyer)];
        source.set_position(point);
        source.set_actual_speed(0);
        source.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer,
            point,
            true,
            false,
        )
        .unwrap();
        let outcome = step_projectile(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer,
            None,
            crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary),
        )
        .unwrap();
        let ring_slot = outcome
            .impact_slot
            .expect("dry Crater impact creates its ring");
        let ring = &live.economy.entities[usize::from(ring_slot)];
        assert_eq!(
            (ring.class, ring.model, ring.action, ring.source_id),
            (10, 11, 11, real_owner)
        );
        let handle = live.economy.handle(ring_slot).unwrap();
        let center = ring.position;
        let phase = ring.phase;
        let baseline = live.terrain.clone();
        let mut lowered = false;
        // The source ring advances its radius once every three phases.
        for visit in 0..3 {
            scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(
                !scheduled.dispatch_events().iter().any(|event| matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "scheduled Crater ring must not roll back: {:?}",
                scheduled.dispatch_events()
            );
            let live = &scheduled.session.state.live;
            let ring = &live.economy.entities[usize::from(ring_slot)];
            if visit == 0 {
                assert_eq!(
                    live.economy.economy.slot_generation(ring_slot),
                    handle.generation
                );
                assert_eq!(ring.phase, phase.wrapping_add(1));
            }
            let x = i32::from(center.x >> 8);
            let y = i32::from(center.y >> 8);
            lowered |= (-3_i32..=3)
                .flat_map(|dx| (-3_i32..=3).map(move |dy| (dx, dy)))
                .any(|(dx, dy)| {
                    let point = Position {
                        x: ((x + dx) as u8 as u16) << 8,
                        y: ((y + dy) as u8 as u16) << 8,
                        z: 0,
                    };
                    live.terrain.ground_height(point) < baseline.ground_height(point)
                });
            if lowered {
                break;
            }
        }
        assert!(lowered, "Crater ring lowers the live flight ground");
    }
}
