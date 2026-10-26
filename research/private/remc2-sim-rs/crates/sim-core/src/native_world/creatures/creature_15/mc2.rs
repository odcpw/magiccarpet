//! Source-arena adapter for the Castle defender domain.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CastleDefenderState, DefenderContext, DefenderOrigin, DefenderTarget, SOURCE_CLASS,
    SOURCE_MODEL, StageState, advance_proximity, bind_stage, stage_prepass,
};
use crate::Position;

pub(crate) fn targets_in_economy(
    economy: &crate::native_world::Level2EconomyState,
    entities: &[EntitySnapshot],
) -> Vec<DefenderTarget> {
    let replaced = economy
        .live_arena_slots()
        .into_iter()
        .map(|(slot, _)| slot)
        .collect::<BTreeSet<_>>();
    let mut rows = entities
        .iter()
        .filter(|row| {
            row.index != 0
                && row.class != 0
                && !replaced.contains(&row.index)
                && !economy.slot_generations.contains_key(&row.index)
        })
        .map(|row| {
            let bytes = row.record_bytes();
            let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            (
                row.index,
                DefenderTarget {
                    slot: row.index,
                    source_id: row.source_id,
                    class: row.class,
                    model: row.model,
                    action: row.action,
                    life: row.life,
                    control_flags: row.control_flags,
                    position: row.position,
                    previous: row.word_32(),
                    next: word(56),
                    attacker: row.word_26(),
                    killer: row.word_24(),
                    target: row.source_word_96(),
                    yaw: row.yaw as u16,
                    roll: row.source_roll() as u16,
                    speed: row.actual_speed,
                    maximum_speed: word(138) as i16,
                    half_width: row.array_pitch(),
                    parent: word(44),
                    stage: StageState {
                        index: bytes[76],
                        mode: bytes[77],
                        relation: word(78),
                        parent: word(44),
                        remaining: word(50) as i16,
                        duration: word(52) as i16,
                    },
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for actor in economy.castle_defenders().iter().filter(|row| row.active) {
        rows.insert(actor.source_entity_index, actor.target_view());
    }
    if let Some(castle) = economy.castle().filter(|castle| castle.active) {
        rows.insert(
            castle.source_entity_index,
            DefenderTarget {
                slot: castle.source_entity_index,
                source_id: castle.source_id_entity_index,
                class: castle.source_class,
                model: castle.source_model,
                action: castle.action,
                life: castle.life,
                control_flags: castle.control_flags,
                position: castle.position,
                previous: 0,
                next: 0,
                attacker: 0,
                killer: castle.killer_entity_index,
                target: 0,
                yaw: 0,
                roll: 0,
                speed: 0,
                maximum_speed: 0,
                half_width: castle.shift_pitch,
                parent: 0,
                stage: StageState::default(),
            },
        );
    }
    for child in economy.castle_children().iter().filter(|row| row.active) {
        rows.insert(
            child.source_entity_index,
            DefenderTarget {
                slot: child.source_entity_index,
                source_id: child.id_entity_index,
                class: child.source_class,
                model: child.model,
                action: child.action,
                life: child.life,
                control_flags: child.control_flags,
                position: child.position,
                previous: child.chain_previous_slot,
                next: child.chain_next_slot,
                attacker: child.damage_attacker_entity_index,
                killer: child.killer_entity_index,
                target: child.target_sphere_slot,
                yaw: child.yaw,
                roll: child.roll,
                speed: child.speed,
                maximum_speed: 0,
                half_width: 128,
                parent: child.parent_entity_index,
                stage: StageState::default(),
            },
        );
    }
    for carrier in economy.carriers().iter().filter(|row| row.active) {
        rows.insert(
            carrier.source_entity_index,
            DefenderTarget {
                slot: carrier.source_entity_index,
                source_id: carrier.owner_entity_index,
                class: 3,
                model: 3,
                action: 9,
                life: carrier.life,
                control_flags: 0,
                position: carrier.position,
                previous: 0,
                next: 0,
                attacker: 0,
                killer: 0,
                target: carrier.target_sphere_id as u16,
                yaw: carrier.yaw as u16,
                roll: carrier.yaw as u16,
                speed: carrier.speed,
                maximum_speed: 0,
                half_width: 128,
                parent: 0,
                stage: StageState::default(),
            },
        );
    }
    for row in economy.authored_class5().iter().filter(|row| row.active) {
        let archer = economy
            .authored_class5_archer()
            .iter()
            .find(|actor| actor.source_entity_index == row.source_entity_index);
        let stage = archer.map_or(StageState::default(), |actor| actor.stage);
        rows.insert(
            row.source_entity_index,
            DefenderTarget {
                slot: row.source_entity_index,
                source_id: archer.map_or(row.source_entity_index, |actor| actor.entity_id),
                class: 5,
                model: row.model,
                action: row.action,
                life: row.life,
                control_flags: archer.map_or(0, |actor| actor.control_flags)
                    | if row.drawing_enabled {
                        0
                    } else {
                        crate::native_world::ENTITY_DRAW_DISABLED_BIT
                    },
                position: row.position,
                previous: archer.map_or(0, |actor| actor.word_0x32),
                next: archer.map_or(0, |actor| actor.word_0x34),
                attacker: row.word_0x26,
                killer: row.word_0x24,
                target: row.word_0x96_150,
                yaw: row.yaw as u16,
                roll: row.roll as u16,
                speed: row.actual_speed,
                maximum_speed: row.maximum_speed,
                half_width: row.shift_rot_x,
                parent: stage.parent,
                stage,
            },
        );
    }
    for bee in economy.bee_creatures().iter().filter(|row| row.active) {
        rows.insert(
            bee.source_entity_index,
            DefenderTarget {
                slot: bee.source_entity_index,
                source_id: bee.entity_id,
                class: bee.source_class,
                model: bee.model,
                action: bee.action,
                life: i32::from(bee.life),
                control_flags: bee.control_flags,
                position: bee.position,
                attacker: bee.damage_attacker_entity_index,
                killer: bee.kill_credit_entity_index,
                target: bee.target_entity_index.unwrap_or(0),
                yaw: bee.yaw as u16,
                roll: bee.roll as u16,
                speed: bee.actual_speed,
                maximum_speed: bee.maximum_speed,
                half_width: bee.shift_rot_x as i16,
                parent: bee.parent_entity_index,
                stage: StageState {
                    index: bee.stage_var1,
                    mode: bee.stage_var2,
                    parent: bee.parent_entity_index,
                    ..StageState::default()
                },
                ..DefenderTarget::default()
            },
        );
    }
    for resident in economy.house_residents().iter().filter(|row| row.active) {
        rows.insert(
            resident.source_entity_index,
            DefenderTarget {
                slot: resident.source_entity_index,
                source_id: resident.source_id,
                class: 5,
                model: resident.model,
                action: resident.action,
                life: resident.life,
                control_flags: resident.control_flags,
                position: resident.position,
                previous: resident.linked_entity_index,
                attacker: resident.current_attacker,
                killer: resident.kill_credit_entity,
                target: resident.target_entity_index,
                yaw: resident.yaw as u16,
                roll: resident.desired_heading as u16,
                speed: resident.actual_speed,
                maximum_speed: resident.maximum_speed,
                half_width: resident.collision_half_width,
                stage: StageState {
                    mode: resident.stage_var_2 as u8,
                    ..StageState::default()
                },
                ..DefenderTarget::default()
            },
        );
    }
    rows.into_values().collect()
}
use crate::campaign::source_arena::{
    CreatureRootOrigin, CreatureRuntimeIdentity, CreatureRuntimeRole, SourceArena,
    SourceEntityHandle,
};
use crate::level_definition::CreatureKind;
use crate::native_world::{EntitySnapshot, RuntimeSnapshot, SessionTerrain, level2_class5};

/// Imported rows transfer without NewEvent, an RNG draw or a model-counter
/// advance. Their old bytes remain immutable retained package input.
pub(crate) fn adopt_imported(arena: &mut SourceArena) {
    let rows = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && row.class == SOURCE_CLASS
                && row.model == SOURCE_MODEL
                && !arena
                    .castle_defenders()
                    .iter()
                    .any(|owned| owned.source_entity_index == row.index)
        })
        .cloned()
        .collect::<Vec<_>>();
    for row in rows {
        let state = import_row(&row, arena.slot_generation(row.index));
        arena.economy.castle_defenders.push(state);
        if let Some(retained) = arena.entities.get_mut(usize::from(row.index)) {
            retained.active = false;
            retained.record[2] = 0;
        }
    }
}

fn import_row(row: &EntitySnapshot, generation: u32) -> CastleDefenderState {
    let bytes = row.record_bytes();
    let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    CastleDefenderState {
        source_entity_index: row.index,
        slot_generation: generation,
        origin: DefenderOrigin::InitialPackage,
        castle_owner: None,
        entity_rng: row.entity_rng,
        id_entity_index: row.source_id,
        player_entity_index: row.source_player_entity_index(),
        active: row.active,
        control_flags: row.control_flags,
        action: row.action,
        position: row.position,
        yaw: row.yaw as u16,
        pitch: row.pitch,
        orientation_fov: row.fov,
        roll: row.source_roll() as u16,
        speed: row.actual_speed,
        minimum_speed: row.minimum_speed(),
        maximum_speed: word(138) as i16,
        life: row.life,
        max_life: row.max_life,
        mana: row.mana,
        max_mana: row.max_mana,
        target_sphere_slot: row.source_word_96(),
        chain_previous_slot: row.word_32(),
        chain_next_slot: word(56),
        pending_damage: i32::from_le_bytes([bytes[98], bytes[99], bytes[100], bytes[101]]),
        pending_attacker_entity_index: word(102),
        damage_attacker_entity_index: row.word_26(),
        killer_entity_index: row.word_24(),
        source_byte_3e: row.phase,
        source_byte_39: bytes[61],
        source_dword_10: row.source_dword_10(),
        source_subspell_index: row.subspell_index,
        byte_38: bytes[60],
        xtype: bytes[69],
        xsubtype: bytes[70],
        visual_index: word(94),
        shift_rot_x: row.array_pitch(),
        shift_rot_y: row.array_roll(),
        fov: row.array_fov(),
        proximity_delay: bytes[62],
        stage: StageState {
            index: bytes[76],
            mode: bytes[77],
            relation: word(78),
            parent: word(44),
            remaining: word(50) as i16,
            duration: word(52) as i16,
        },
    }
}

pub(crate) fn runtime_identity(
    arena: &SourceArena,
    slot: u16,
    initial: &BTreeMap<u16, CreatureKind>,
    authored: &BTreeMap<u16, CreatureKind>,
    include_retained: bool,
) -> Option<CreatureRuntimeIdentity> {
    let state = arena
        .castle_defenders()
        .iter()
        .find(|state| state.source_entity_index == slot && (include_retained || state.active))?;
    let origin = match state.origin {
        DefenderOrigin::Castle(castle_slot) => CreatureRootOrigin::CastleSpawn { castle_slot },
        DefenderOrigin::InitialPackage => {
            if initial.get(&slot) != Some(&CreatureKind::CastleDefender) {
                return None;
            }
            CreatureRootOrigin::InitialPackage
        }
        DefenderOrigin::AuthoredRecord(authored_record) => {
            if authored.get(&authored_record) != Some(&CreatureKind::CastleDefender) {
                return None;
            }
            CreatureRootOrigin::AuthoredRecord { authored_record }
        }
    };
    Some(CreatureRuntimeIdentity {
        handle: SourceEntityHandle {
            source_slot: slot,
            generation: state.slot_generation,
        },
        kind: CreatureKind::CastleDefender,
        root_slot: slot,
        role: CreatureRuntimeRole::Root,
        origin,
        raw_model: Some(SOURCE_MODEL),
        living: state.active && state.drawing_enabled() && state.life >= 0,
    })
}

/// Live DrawSprites candidates. Particle selection follows the callback's
/// current visual field; descriptor83 is the actor's behavior descriptor.
pub(crate) fn presentation_candidates(
    actors: &[CastleDefenderState],
) -> impl Iterator<Item = (u16, u16, u16, i16, Position, u32)> + '_ {
    actors
        .iter()
        .filter(|actor| actor.active && actor.control_flags & 0x421 == 0)
        .map(|actor| {
            (
                actor.source_entity_index,
                actor.id_entity_index,
                actor.visual_index,
                actor.yaw.cast_signed(),
                actor.position,
                actor.control_flags,
            )
        })
}

/// Resolve all fields from current carriers. Frozen lists contain handles,
/// never frozen poses/mailboxes, and superseded imported rows cannot reappear.
pub(crate) fn targets_in_arena(arena: &SourceArena) -> Vec<DefenderTarget> {
    let relations = level2_class5::archer::relations_in_arena(arena);
    let mut slots = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<BTreeSet<_>>();
    slots.extend(arena.live_arena_slots().into_iter().map(|(slot, _)| slot));
    let mut targets = slots
        .into_iter()
        .filter_map(|slot| {
            if let Some(actor) = arena
                .castle_defenders()
                .iter()
                .find(|actor| actor.active && actor.source_entity_index == slot)
            {
                return Some(actor.target_view());
            }
            if let Some(castle) = arena
                .castle()
                .filter(|castle| castle.active && castle.source_entity_index == slot)
            {
                return Some(DefenderTarget {
                    slot,
                    source_id: castle.source_id_entity_index,
                    class: castle.source_class,
                    model: castle.source_model,
                    action: castle.action,
                    life: castle.life,
                    control_flags: castle.control_flags,
                    position: castle.position,
                    previous: 0,
                    next: 0,
                    attacker: 0,
                    killer: 0,
                    target: 0,
                    yaw: 0,
                    roll: 0,
                    speed: 0,
                    maximum_speed: 0,
                    half_width: castle.shift_pitch,
                    parent: 0,
                    stage: StageState::default(),
                });
            }
            if let Some(child) = arena
                .castle_children()
                .iter()
                .find(|child| child.active && child.source_entity_index == slot)
            {
                return Some(DefenderTarget {
                    slot,
                    source_id: child.id_entity_index,
                    class: child.source_class,
                    model: child.model,
                    action: child.action,
                    life: child.life,
                    control_flags: child.control_flags,
                    position: child.position,
                    previous: child.chain_previous_slot,
                    next: child.chain_next_slot,
                    attacker: child.damage_attacker_entity_index,
                    killer: child.killer_entity_index,
                    target: child.target_sphere_slot,
                    yaw: child.yaw,
                    roll: child.roll,
                    speed: child.speed,
                    maximum_speed: 0,
                    half_width: 128,
                    parent: child.parent_entity_index,
                    stage: StageState::default(),
                });
            }
            if let Some(carrier) = arena
                .carriers()
                .iter()
                .find(|carrier| carrier.active && carrier.source_entity_index == slot)
            {
                return Some(DefenderTarget {
                    slot,
                    source_id: carrier.owner_entity_index,
                    class: 3,
                    model: 3,
                    action: 9,
                    life: carrier.life,
                    control_flags: 0,
                    position: carrier.position,
                    previous: 0,
                    next: 0,
                    attacker: 0,
                    killer: 0,
                    target: 0,
                    yaw: 0,
                    roll: 0,
                    speed: 0,
                    maximum_speed: 0,
                    half_width: 128,
                    parent: 0,
                    stage: StageState::default(),
                });
            }
            let relation = relations.iter().find(|relation| relation.slot == slot)?;
            let row = relation.target;
            Some(DefenderTarget {
                slot,
                source_id: relation.source_id,
                class: row.class,
                model: row.model,
                action: row.action,
                life: row.life,
                control_flags: row.control_flags,
                position: row.position,
                previous: row.word_0x32,
                next: 0,
                attacker: relation.attacker,
                killer: 0,
                target: row.word_0x96_150,
                yaw: relation.yaw as u16,
                roll: relation.roll as u16,
                speed: row.actual_speed,
                maximum_speed: row.maximum_speed,
                half_width: relation.half_width,
                parent: relation.parent,
                stage: StageState {
                    index: relation.stage_index,
                    mode: relation.stage_mode,
                    relation: relation.stage_relation,
                    parent: relation.parent,
                    ..StageState::default()
                },
            })
        })
        .collect::<Vec<_>>();
    let current_slots = targets.iter().map(|row| row.slot).collect::<BTreeSet<_>>();
    targets.extend(
        targets_in_economy(&arena.economy, &arena.entities)
            .into_iter()
            .filter(|row| !current_slots.contains(&row.slot)),
    );
    targets.sort_unstable_by_key(|row| row.slot);
    targets
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &SessionTerrain,
    frozen: &[Vec<u16>; 29],
) {
    adopt_imported(arena);
    let targets = targets_in_arena(arena);
    let player = targets
        .iter()
        .find(|row| row.class == 3 && row.model == 0)
        .map(|row| row.position);
    for slot in &frozen[usize::from(SOURCE_MODEL)] {
        let Some(actor) = arena
            .economy
            .castle_defenders
            .iter_mut()
            .find(|actor| actor.active && actor.source_entity_index == *slot)
        else {
            continue;
        };
        let mut context = DefenderContext {
            terrain,
            targets: &targets,
            class3: &[],
            model15: &frozen[usize::from(SOURCE_MODEL)],
            class5: &[],
            stage_runtime: Some(runtime),
        };
        stage_prepass(actor, &mut context);
    }
    for slot in &frozen[usize::from(SOURCE_MODEL)] {
        let Some(actor) = arena
            .economy
            .castle_defenders
            .iter_mut()
            .find(|actor| actor.active && actor.source_entity_index == *slot)
        else {
            continue;
        };
        let head = actor.chain_next_slot;
        let inherited = player.and_then(|player| advance_proximity(actor, player));
        let Some(countdown) = inherited else {
            continue;
        };
        let mut next = head;
        let mut visited = BTreeSet::new();
        while next != 0 && visited.insert(next) {
            if let Some(child) = arena
                .economy
                .castle_defenders
                .iter_mut()
                .find(|actor| actor.source_entity_index == next)
            {
                child.source_byte_39 = countdown;
                next = child.chain_next_slot;
            } else if let Some(child) = arena
                .entities
                .get_mut(usize::from(next))
                .filter(|row| row.class != 0)
            {
                child.record[61] = countdown;
                next = u16::from_le_bytes([child.record[56], child.record[57]]);
            } else {
                break;
            }
        }
    }
}

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) {
    if let Some(actor) = arena
        .economy
        .castle_defenders
        .iter_mut()
        .find(|actor| actor.source_entity_index == slot)
    {
        bind_stage(actor, index, runtime);
    }
}

/// Apply the owned callback to its arena carrier, then consume its allocation
/// requests in source order. NewEvent may replace the executing fixed row.
pub(crate) fn step_economy(
    economy: &mut crate::native_world::Level2EconomyState,
    source_slot: u16,
    mut context: DefenderContext<'_>,
) -> Option<super::DefenderEffects> {
    use crate::native_world::{Level2EconomyEvent, level2_next_rng16};
    use crate::renderable_map_chain::RenderableMapSeed;

    let index = economy
        .castle_defenders
        .iter()
        .position(|row| row.active && row.source_entity_index == source_slot)?;
    let mut actor = economy.castle_defenders[index];
    let mut effects = super::step(&mut actor, &mut context)?;
    // The replacement scan must observe this callback's writes before it can
    // reclaim its own carrier. Never write the saved actor back afterwards.
    economy.castle_defenders[index] = actor;
    for peer in &effects.linked_updates {
        if let Some(row) = economy
            .castle_defenders
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == peer.slot)
        {
            row.action = peer.action;
            row.target_sphere_slot = peer.target;
            row.chain_previous_slot = peer.previous;
            row.control_flags = peer.control_flags;
        }
    }
    for _ in 0..effects.population_decrements {
        economy.decrement_source_creature_population();
    }
    if let Some(request) = effects.arrow {
        let packet = economy.allocate_castle_auxiliary_projectile_packet(
            crate::native_world::level2_class5::archer::ArcherChildRequest {
                parent_entity_index: 0,
                parent_entity_id: request.owner,
                parent_word_0x96: request.word96,
                parent_roll: 0,
                parent_position: request.position,
                parent_fov: request.fov,
                target_entity_index: request.target.slot,
                target_class: request.target.class,
                target_model: request.target.model,
                target_position: request.target.position,
            },
        );
        if let Some(packet) = packet {
            effects.target_music = (request.target.class == 3 && request.target.model == 0)
                .then_some(request.target.slot);
            if let Some(arrow) = economy
                .authored_class5_archer_children
                .iter_mut()
                .find(|row| row.source_entity_index == packet.source_entity_index)
            {
                arrow.xtype = request.xtype;
                arrow.xsubtype = request.xsubtype;
                arrow.subspell_index =
                    crate::level_definition::CASTLE_DEFENDER_CLASS_PROPERTIES.arrow_damage;
                if packet.source_entity_index == source_slot {
                    super::finish_self_replaced_arrow(
                        arrow,
                        request.target.position,
                        context.terrain,
                    );
                }
            }
        }
    }
    if effects.kill {
        if actor.mana > 0 {
            actor.entity_rng = level2_next_rng16(actor.entity_rng);
            if let Some(row) = economy
                .castle_defenders
                .iter_mut()
                .find(|row| row.source_entity_index == source_slot)
            {
                row.entity_rng = actor.entity_rng;
            }
        }
        let owner = economy.source_owner(source_slot);
        let _ = economy.spawn_mana_sphere_from_death(
            source_slot,
            actor.position,
            actor.yaw as i16,
            actor.entity_rng,
            actor.mana,
            owner,
            context.terrain,
        );
        let replacement = if actor.control_flags & 0x10_0000 == 0 {
            economy.spawn_death_burst_from_death_with_id(
                source_slot,
                actor.id_entity_index,
                actor.position,
            )
        } else {
            None
        };
        if replacement == Some(source_slot) {
            if let Some(burst) = economy
                .death_bursts
                .iter_mut()
                .find(|row| row.source_entity_index == source_slot)
            {
                super::finish_self_replaced_burst(burst);
            }
            if let Some(Level2EconomyEvent::DeathBurstSpawned {
                id_entity_index, ..
            }) = economy.events.iter_mut().rev().find(|event| {
                matches!(event,
                    Level2EconomyEvent::DeathBurstSpawned { source_entity_index, .. }
                    if *source_entity_index == source_slot)
            }) {
                *id_entity_index = source_slot;
            }
            let _ = economy.map_chains.sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: 0,
                y: 0,
            });
        } else if let Some(row) = economy
            .castle_defenders
            .iter_mut()
            .find(|row| row.source_entity_index == source_slot)
        {
            row.hide();
        }
    }
    Some(effects)
}

pub(crate) struct DefenderStepResult {
    pub effects: super::DefenderEffects,
    pub map_synced: bool,
}

pub(crate) fn step_source_runtime(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    slot: u16,
    membership: &crate::native_world::level2_dynamic_economy::Level2CastleAuxiliaryMembership,
) -> Result<DefenderStepResult, String> {
    let position = arena
        .castle_defenders()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .ok_or_else(|| format!("Castle defender slot {slot} has no current actor"))?
        .position;
    let targets = targets_in_arena(arena);
    let (class3, model15, class5) = arena.castle_auxiliary_live_memberships(membership);
    let context = DefenderContext {
        terrain,
        targets: &targets,
        class3: &class3,
        model15: &model15,
        class5: &class5,
        stage_runtime: Some(runtime),
    };
    let effects = arena
        .economy
        .step_castle_defender_with_context(slot, context)
        .ok_or_else(|| format!("Castle defender slot {slot} has an unsupported callback"))?;
    if let Some(target) = effects
        .target_music
        .and_then(|slot| arena.entities.get_mut(usize::from(slot)))
    {
        target.apply_source_sub_5ef70();
    }
    if effects.expire {
        let mut position = arena
            .castle_defenders()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map_or(position, |row| row.position);
        if effects.summon_expire {
            let generation = arena.slot_generation(slot);
            crate::native_world::creatures::goat::runtime::spawn_expiry_visual(
                arena, terrain, position, particles,
            )?;
            if arena.slot_generation(slot) != generation
                && let Some(effect) = arena
                    .entities
                    .get_mut(usize::from(slot))
                    .filter(|row| row.active && row.class == 10 && row.model == 73)
            {
                // 1E580 passed a pointer into the original fixed row. A
                // self-replacing NewEvent zeroed it before 51A00 read it.
                position = Position::default();
                position.z = crate::FlightWorld::ground_height(terrain, position);
                effect.set_position(position);
                arena
                    .economy
                    .map_chains
                    .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: slot,
                        active: true,
                        x: position.x,
                        y: position.y,
                    })
                    .map_err(|error| format!("Castle defender self-expiry map link: {error}"))?;
            }
        }
        if let Some(effect_slot) = arena
            .economy
            .allocate_class10_model0_effect(0, 0, 0, 0, position, 400, 0, 0x20000, 128, 128)
            && let Some(effect) = arena
                .economy
                .model17_spawned_children
                .iter_mut()
                .find(|row| row.source_entity_index == effect_slot)
        {
            effect.parent_entity_id = effect_slot;
            if effect_slot == slot {
                effect.position = Position::default();
                arena
                    .economy
                    .map_chains
                    .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                        entity_index: slot,
                        active: true,
                        x: 0,
                        y: 0,
                    })
                    .map_err(|error| format!("Castle defender self-expiry impact link: {error}"))?;
            }
        }
        // DisableEntityDrawing uses the pointer after both allocation calls.
        if let Some(actor) = arena
            .economy
            .castle_defenders
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            actor.hide();
        } else if let Some(effect) = arena
            .economy
            .model17_spawned_children
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            effect.control_flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
        } else if let Some(effect) = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active && row.class == 10 && row.model == 73)
        {
            effect.disable_drawing();
        }
    }
    let map_synced = if let Some(effect) = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 10 && row.model == 73)
    {
        effect.set_phase(effect.phase.wrapping_add(1));
        arena.economy.source_rng.insert(slot, effect.entity_rng);
        true
    } else {
        arena.economy.finish_castle_auxiliary_source_visit(slot)
    };
    Ok(DefenderStepResult {
        effects,
        map_synced,
    })
}
