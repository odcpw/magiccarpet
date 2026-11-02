//! Wyvern bindings to the live source arena, shared route table and assets.

use super::{Actor, Context, DESCRIPTOR, Effects, PARTICLE, RULES, SOURCE_MODEL, Target, put, word};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena, SourceEntityHandle};
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::Position;

const FIREBALL_PARTICLE: usize = 340;

pub(crate) fn construct_row(
    slot:u16,
    _generation:u32,
    seed:u16,
    position:Position,
    counter:&mut u8,
    particles:&[ParticleParameter],
)->Result<EntitySnapshot,String> {
    particles.get(usize::from(PARTICLE)).and_then(ParticleParameter::animation_limit)
        .ok_or_else(||"Wyvern requires retained particle207 with valid animation".to_owned())?;
    let phase=*counter;
    *counter=counter.wrapping_add(1);
    let mut row=crate::native_world::cave_features::new_event(slot,seed,position);
    row.class=5;row.model=SOURCE_MODEL;
    row.record[67]=5;row.record[68]=SOURCE_MODEL;
    row.set_source_action(129);
    row.set_max_life(RULES.max_life);row.set_life(RULES.max_life);row.set_mana(RULES.initial_mana);
    row.set_actual_speed(RULES.minimum_speed);
    put(&mut row,136,RULES.minimum_speed as u16);put(&mut row,138,RULES.maximum_speed as u16);
    let rng=super::level2_next_rng16(seed);row.set_entity_rng(rng);
    let angle=(rng&2047).wrapping_sub(1) as i16;
    row.set_yaw(angle);row.set_pitch(angle);row.set_source_roll(angle);row.set_fov(0);
    row.set_subspell_index(RULES.attack_damage);
    row.record[60]=1;row.record[61]=64;row.record[69]=3;
    row.parameter_relation=Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR)+1).to_le_bytes());
    row.set_source_dword_10(0);row.set_phase(phase);
    crate::world::effects::bind_particle(&mut row,PARTICLE,particles)?;
    put(&mut row,86,RULES.collision_vertical_offset as u16);
    row.set_source_overlap_half_extents(RULES.collision_half_width,RULES.collision_half_width);
    put(&mut row,92,RULES.collision_half_height as u16);
    Ok(row)
}

fn edit_route(row:&mut EntitySnapshot,edit:impl FnOnce(&mut super::super::stage_route::Route<'_>)) {
    let mut action=row.action;
    let mut index=row.record[76] as i8;
    let mut mode=row.record[77] as i8;
    let mut relation=word(row,78);
    edit(&mut super::super::stage_route::Route {model:SOURCE_MODEL,position:row.position,
        action:&mut action,index:&mut index,mode:&mut mode,relation:&mut relation});
    row.set_source_action(action);row.record[76]=index as u8;row.record[77]=mode as u8;put(row,78,relation);
}
pub(crate) fn bind_authored_route(arena:&mut SourceArena,runtime:&mut RuntimeSnapshot,slot:u16,index:u8)->Result<(),String> {
    let row=arena.entities.get_mut(usize::from(slot)).filter(|r|r.active&&r.class==5&&r.model==SOURCE_MODEL)
        .ok_or_else(||format!("Wyvern StageVar bind lost slot{slot}"))?;
    edit_route(row,|route|route.bind(runtime,index));
    Ok(())
}

pub(crate) fn allocate_authored(
    arena:&mut SourceArena,runtime:&mut RuntimeSnapshot,terrain:&mut SessionTerrain,
    particles:&[ParticleParameter],position:Position,max_life:i32,stage_index:Option<u8>,authored_record:u16,
)->Result<Option<u16>,String> {
    particles.get(usize::from(PARTICLE)).and_then(ParticleParameter::animation_limit)
        .ok_or_else(||"Wyvern requires retained particle207 with valid animation".to_owned())?;
    let Some(slot)=arena.allocate_source_slot_with_terrain(terrain) else {return Ok(None)};
    // Ordinary and Morph constructors share source array_0x10[16].
    let mut counter=arena.economy.morph.phases[0];
    let mut row=construct_row(slot,arena.slot_generation(slot),arena.new_event_rng(slot),position,&mut counter,particles)?;
    row.bind_frame_zero_max_life(max_life);
    arena.economy.morph.phases[0]=counter;
    runtime.bytes[crate::native_world::RUNTIME_BYTES+16]=counter;
    arena.economy.source_rng.insert(slot,row.entity_rng);
    arena.entities[usize::from(slot)]=row;
    crate::world::effects::sync_position(arena,terrain,slot,position,true,false)?;
    if let Some(index)=stage_index {bind_authored_route(arena,runtime,slot,index)?;}
    let handle=arena.handle(slot).ok_or_else(||format!("Wyvern constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(handle,crate::level_definition::CreatureKind::Wyvern,CreatureRootOrigin::AuthoredRecord {authored_record}) {
        return Err(format!("Wyvern record{authored_record} lost generation-bound origin"));
    }
    Ok(Some(slot))
}

/// Source `sub_3A5B0`'s ordinary class-5 summon producer. The caller owns the
/// ring lifetime; this adapter constructs the real Wyvern row and records the
/// historical holder as its generation-bound origin.
pub(crate) fn allocate_summoned(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    parent_slot: u16,
    owner_id: u16,
    holder_slot: u16,
) -> Result<Option<SourceEntityHandle>, String> {
    particles
        .get(usize::from(PARTICLE))
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| "Wyvern Army summon requires retained particle207".to_owned())?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };

    // Ordinary, Morph, and Army producers share source array_0x10[16].
    let mut counter = arena.economy.morph.phases[0];
    let mut row = construct_row(
        slot,
        arena.slot_generation(slot),
        arena.new_event_rng(slot),
        position,
        &mut counter,
        particles,
    )?;
    row.set_source_action(8 * SOURCE_MODEL + 7);
    row.record[77] = 13;
    put(&mut row, 44, parent_slot);
    put(&mut row, 50, 250);
    put(&mut row, 154, 0);
    row.set_mana(0);
    row.max_mana = 0;
    row.mana_cost = 0;
    row.record[144..148].fill(0);
    row.source_id = owner_id;
    put(&mut row, 30, owner_id);

    arena.economy.morph.phases[0] = counter;
    runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;

    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Wyvern Army summon lost allocated slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Wyvern,
        CreatureRootOrigin::SummonedSpell { holder_slot },
    ) {
        return Err(format!(
            "Wyvern Army summon at slot{slot} lost generation-bound origin"
        ));
    }
    if let Some(parent) = arena.handle(parent_slot).filter(|parent| *parent != handle) {
        if !arena.bind_creature_parent(handle, parent) {
            return Err(format!("Wyvern Army summon at slot{slot} lost its parent binding"));
        }
    }
    Ok(Some(handle))
}

pub(crate) fn prepare_source_pass(arena:&mut SourceArena,runtime:&mut RuntimeSnapshot,family:&[u16]) {
    let dead=super::super::creature_15::mc2::targets_in_arena(arena).iter()
        .filter(|t|t.life<0||t.control_flags&0x400!=0).map(|t|t.slot).collect::<std::collections::BTreeSet<_>>();
    for &slot in family {
        if let Some(row)=arena.entities.get_mut(usize::from(slot)).filter(|r|r.active&&r.class==5&&r.model==SOURCE_MODEL) {
            edit_route(row,|route|route.prepare(runtime,|target|dead.contains(&target)));
        }
    }
}

fn targets(arena:&SourceArena,direct:&std::collections::BTreeSet<u16>)->Vec<Target> {
    let mut targets=super::super::creature_15::mc2::targets_in_arena(arena);
    let typed=arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|r|(r.active&&r.class!=0||direct.contains(&r.index))&&r.index!=0&&!typed.iter().any(|(s,_)|*s==r.index)) {
        let actor=Target {
            slot:row.index,source_id:row.source_id,class:row.class,model:row.model,action:row.action,life:row.life,
            control_flags:row.control_flags,position:row.position,previous:word(row,54),next:word(row,56),
            attacker:word(row,42),killer:word(row,40),target:word(row,154),yaw:row.yaw as u16,roll:row.source_roll() as u16,
            speed:row.actual_speed,maximum_speed:word(row,138) as i16,half_width:word(row,88) as i16,parent:word(row,44),
            stage:super::StageState {index:row.record[76],mode:row.record[77],relation:word(row,78),parent:word(row,44),remaining:word(row,50) as i16,duration:word(row,52) as i16},
        };
        if let Some(old)=targets.iter_mut().find(|t|t.slot==row.index) {*old=actor;} else {targets.push(actor);}
    }
    targets.sort_unstable_by_key(|t|t.slot);
    targets
}

pub(crate) fn step_row(
    arena:&mut SourceArena,runtime:&mut RuntimeSnapshot,terrain:&mut SessionTerrain,
    particles:&[ParticleParameter],slot:u16,class3:&[u16],families:&[Vec<u16>;29],
)->Result<Effects,String> {
    let mut row=arena.entities.get(usize::from(slot)).filter(|r|r.active&&r.class==5&&r.model==SOURCE_MODEL)
        .ok_or_else(||format!("Wyvern callback lost slot{slot}"))?.clone();
    let direct=crate::native_world::source_target::alliance_direct_references(arena,&row)?;
    let mut targets=targets(arena,&direct);
    let handle = arena.handle(slot).ok_or_else(|| format!("Wyvern lost source slot{slot}"))?;
    if matches!(row.record[77], 13 | 16)
        && arena.has_creature_parent_binding(handle)
        && arena.creature_parent(handle).is_none()
    {
        // A delayed Army child must expire when its original caster was
        // reclaimed, even if a living replacement now occupies that address.
        let parent_slot = word(&row, 44);
        targets.retain(|target| target.slot != parent_slot);
    }
    let mut context=Context {terrain,targets:&targets,class3,family:&families[16],families,runtime};
    let effects=super::step(&mut Actor {row:&mut row},&mut context)?;
    let position=row.position;
    arena.economy.source_rng.insert(slot,row.entity_rng);
    arena.entities[usize::from(slot)]=row;
    for update in &effects.linked_updates {
        if let Some(row)=arena.entities.get_mut(usize::from(update.slot)).filter(|r|r.active&&r.class==5&&r.model==SOURCE_MODEL) {
            row.set_source_action(update.action);row.set_control_flags(update.control_flags);
            put(row,154,update.target);put(row,54,update.previous);
        }
    }
    for _ in 0..effects.population_decrements {arena.economy.decrement_source_creature_population();}
    crate::world::effects::sync_position(arena,terrain,slot,position,true,false)?;
    if let Some(target)=effects.target_music.and_then(|s|arena.entities.get_mut(usize::from(s))) {target.apply_source_sub_5ef70();}
    if let Some(projectile)=effects.projectile {
        launch_projectile(arena,terrain,particles,projectile)?;
    }
    if effects.kill {super::super::creature_24::mc2::kill(arena,terrain,particles,slot)?;}
    if effects.expire {super::super::creature_24::mc2::expire(arena,terrain,particles,slot,effects.summon_expire)?;}
    Ok(effects)
}

/// The source9/0 constructor is shared with ordinary Fireball. Allocation is
/// performed by SourceArena so current hidden-slot replacement has the same
/// fixed-address semantics as the creature and its death products.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProjectileLaunchState {
    pub(super) position: Position,
    pub(super) yaw: i16,
    pub(super) pitch: i16,
    pub(super) owner: u16,
    pub(super) target: u16,
    pub(super) target_class: u8,
    pub(super) target_model: u8,
}

/// Resolve the post-allocation fields read by sub_24510. When NewEvent reuses
/// the firing Wyvern's address, its position pointer aliases the cleared
/// Fireball row, so source computes aim from that row rather than the old pose.
pub(super) fn projectile_launch_state(
    launch: super::Projectile,
    projectile_slot: u16,
    replaced_parent: bool,
    replacement_fov: i16,
) -> ProjectileLaunchState {
    if replaced_parent {
        let aim_origin = Position::default();
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
            aim_origin,
            launch.target_position,
        );
        ProjectileLaunchState {
            position: Position {
                z: replacement_fov.wrapping_mul(6),
                ..aim_origin
            },
            yaw,
            pitch,
            owner: projectile_slot,
            target: 0,
            target_class: launch.target_class,
            target_model: launch.target_model,
        }
    } else {
        ProjectileLaunchState {
            position: launch.position,
            yaw: launch.yaw,
            pitch: launch.pitch,
            owner: launch.owner,
            target: launch.target,
            target_class: launch.target_class,
            target_model: launch.target_model,
        }
    }
}

fn launch_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    launch: super::Projectile,
) -> Result<Option<u16>, String> {
    let source_position = arena
        .entities
        .get(usize::from(launch.parent))
        .ok_or("Wyvern projectile parent address is outside the source arena")?
        .position;
    let replacement_fov = particles
        .get(FIREBALL_PARTICLE)
        .map(|particle| (particle.rotation_speed / 2) as i16)
        .ok_or_else(|| "Wyvern projectile requires retained particle340".to_owned())?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let replaced = slot == launch.parent;
    let state = projectile_launch_state(launch, slot, replaced, replacement_fov);
    let initial_position = if replaced { Position::default() } else { source_position };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Wyvern,
            position: initial_position,
            yaw: state.yaw,
            pitch: state.pitch,
            axis_position: Position::default(),
            source_id_entity_index: state.owner,
            holder_entity_index: 0,
            source_subspell_index: launch.payload,
            source_mana: launch.damage,
            source_dword_0x10: 0,
            target_source_entity_index: state.target,
            target_class: state.target_class,
            target_model: state.target_model,
        };
    let result = arena.economy.initialize_fireball_projectile(slot, payload, 0);
    if result.is_none() {
        return Err(format!(
            "Wyvern projectile map linkage failed at slot{slot}"
        ));
    }
    crate::world::effects::sync_position(arena, terrain, slot, initial_position, true, false)?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        initial_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.source_entity_index == slot)
        .ok_or_else(|| format!("Wyvern Fireball lost allocated slot{slot}"))?
        .position = state.position;
    Ok(result)
}
