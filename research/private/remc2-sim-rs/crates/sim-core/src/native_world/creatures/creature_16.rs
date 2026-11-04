//! Wyvern: ordinary sub_4C310 construction and actions128..135.
//!
//! The callback borrows one arena record plus narrow terrain, target and route
//! views. The MC2 adapter publishes it before allocations; the source scheduler
//! owns the byte3E epilogue and following-pass reclamation.

use super::archer::{ArcherStageRuntime as StageRuntime, ArcherStageState as StageState};
use super::creature_15::DefenderTarget as Target;
use crate::level_definition::WYVERN_CLASS_PROPERTIES as RULES;
use crate::native_world::{EntitySnapshot, SessionTerrain, ENTITY_DRAW_DISABLED_BIT, level2_next_rng16};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
mod tests;

pub(crate) const SOURCE_MODEL: u8 = 16;
pub(crate) const DESCRIPTOR: u16 = 84;
pub(crate) const PARTICLE: u16 = 207;

/// Callback-local state read from and written to the authoritative source row.
/// No independently mutable family mirror survives a callback.
pub(crate) struct Actor<'a> {
    pub row: &'a mut EntitySnapshot,
}

impl Actor<'_> {
    fn word(&self, at: usize) -> u16 { word(self.row, at) }
    fn put(&mut self, at: usize, value: u16) { put(self.row, at, value); }
    fn target(&self) -> u16 { self.word(154) }
    fn set_target(&mut self, slot: u16) { self.put(154, slot); }
    fn attacker(&self) -> u16 { self.word(42) }
    fn set_action(&mut self, action: u8) { self.row.set_source_action(action); }
    fn stage(&self) -> StageState {
        StageState { index: self.row.record[76], mode: self.row.record[77], relation: self.word(78),
            parent: self.word(44), remaining: self.word(50) as i16, duration: self.word(52) as i16 }
    }
    fn set_stage(&mut self, stage: StageState) {
        self.row.record[76] = stage.index;
        self.row.record[77] = stage.mode;
        for (at,v) in [(78,stage.relation),(44,stage.parent),(50,stage.remaining as u16),(52,stage.duration as u16)] { self.put(at,v); }
    }
}

fn word(row: &EntitySnapshot, at: usize) -> u16 { u16::from_le_bytes([row.record[at], row.record[at+1]]) }
fn put(row: &mut EntitySnapshot, at: usize, value: u16) { row.record[at..at+2].copy_from_slice(&value.to_le_bytes()); }

pub(crate) struct Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub family: &'a [u16],
    pub families: &'a [Vec<u16>; 29],
    pub runtime: &'a mut dyn StageRuntime,
}
impl Context<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0).then(|| self.targets.iter().find(|t| t.slot == slot).copied()).flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Projectile {
    pub parent: u16,
    pub owner: u16,
    pub target: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub position: Position,
    pub target_position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub payload: u16,
    pub damage: i32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Effects {
    pub projectile: Option<Projectile>,
    pub target_music: Option<u16>,
    pub kill_credit: Option<u16>,
    pub feedback: Option<(u16,u8)>,
    pub earlier_feedback: Option<(u16,u8)>,
    pub sound: Option<u8>,
    pub linked_updates: Vec<Target>,
    pub population_decrements: u32,
    pub kill: bool,
    pub expire: bool,
    pub summon_expire: bool,
}

fn living(target: Target) -> bool { target.life >= 0 && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0 }
fn xy(a: Position,b: Position)->u32 {
    let x=i32::from(b.x.wrapping_sub(a.x) as i16);
    let y=i32::from(b.y.wrapping_sub(a.y) as i16);
    x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32
}
fn distance(a: Position,b: Position)->u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(a,b))
}
fn settle(actor:&mut Actor<'_>,ctx:&Context<'_>) {
    crate::ground_creature_motion::relax_to_terrain(&mut actor.row.position,ctx.terrain,RULES.ground_clearance,RULES.descent);
    actor.row.set_position(actor.row.position);
}
fn move_actor(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    if actor.row.control_flags & 0x800 != 0 {
        actor.row.set_control_flags(actor.row.control_flags & !0x800);
        return;
    }
    let motion=crate::ground_creature_motion::step_ground_creature_motion(actor.row.position,actor.row.yaw,actor.row.source_roll(),actor.row.actual_speed,ctx.terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: actor.word(88).max(actor.word(90)) as i16,
            half_height:actor.word(92) as i16,
            ground_clearance:RULES.ground_clearance,descent:RULES.descent,turn_limit:RULES.turn_limit,
            terrain_mask:RULES.terrain_mask,maximum_tile_slope:RULES.maximum_slope,
        });
    actor.row.set_position(motion.position);
    actor.row.set_yaw(motion.yaw);
    actor.row.set_control_flags(if motion.movement_blocked {actor.row.control_flags|0x40000} else {actor.row.control_flags & !0x40000});
    if motion.killed {
        actor.row.set_life(-1);
        effects.population_decrements+=1;
    }
}
fn consume(actor:&mut Actor<'_>,ctx:&Context<'_>)->u8 {
    let mut damage=i32::from_le_bytes(actor.row.record[98..102].try_into().expect("damage dword"));
    let mut pending=actor.word(102);
    let mut attacker=actor.attacker();
    let mut life=actor.row.life;
    let mut hit=u8::from(crate::combat::actors::consume_damage(&mut life,&mut damage,&mut pending,&mut attacker).is_some());
    let mut next=actor.word(56);
    for _ in 0..ctx.targets.len() {
        let Some(child)=ctx.target(next) else {break};
        if crate::combat::actors::inherit_linked_life(&mut life,&mut attacker,child.life,child.attacker) {
            hit=1;
            break;
        }
        next=child.next;
    }
    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    actor.put(102,pending);
    actor.put(42,attacker);
    if life<0 {actor.put(40,attacker);2} else {hit}
}
fn jitter(actor:&mut Actor<'_>) {
    let first=level2_next_rng16(actor.row.entity_rng);
    let rng=level2_next_rng16(first);
    actor.row.set_entity_rng(rng);
    let delta=(i32::from(rng & 255)+85)*(2*(i32::from(first%157)/79)-1);
    actor.row.set_source_roll(actor.row.source_roll().wrapping_add(delta as i16)&2047);
}
fn nearest(actor:&Actor<'_>,ctx:&Context<'_>,slots:&[u16],reverse:bool,cone:bool,eligible:impl Fn(Target)->bool)->Option<Target> {
    let mut best=None;
    let mut shortest=u32::MAX;
    for target in slots.iter().filter_map(|s|ctx.target(*s)).filter(|t|eligible(*t)) {
        let d=xy(actor.row.position,target.position);
        let heading=if reverse {heading_for_positions(target.position,actor.row.position)} else {heading_for_positions(actor.row.position,target.position)};
        if d<=RULES.target_range*RULES.target_range && d<shortest && (!cone || angle_distance(i32::from(actor.row.yaw),heading)<RULES.target_half_angle) {
            best=Some(target);shortest=d;
        }
    }
    best
}
fn select_peer(actor:&mut Actor<'_>,ctx:&Context<'_>,reverse:bool) {
    if RULES.behavior_flags & 4 == 0 && let Some(peer)=nearest(actor,ctx,ctx.family,reverse,true,|t|t.slot!=actor.row.index && t.previous==0) {
        actor.put(54,peer.slot);actor.set_action(131);
    }
}
fn damage_action(actor:&mut Actor<'_>,hit:u8) {
    if hit==2 {actor.set_action(132);} else if hit==1 {actor.set_target(actor.attacker());actor.set_action(130);}
}

/// sub_24420/24440: generic stationary and roaming helpers, then the
/// Wyvern-specific cone-free class3 scan every descriptor cadence+1.
fn roam(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    let stationary=actor.row.action==128;
    let hit=consume(actor,ctx);
    if hit!=0 {
        damage_action(actor,hit);
        if stationary || hit==1 {settle(actor,ctx);}
    } else if stationary {
        if actor.row.phase%RULES.scan_period==0 {select_peer(actor,ctx,true);}
        settle(actor,ctx);
    } else {
        move_actor(actor,ctx,effects);
        if actor.row.phase%RULES.scan_period==0 {
            jitter(actor);
            if actor.row.record[61]!=0 {
                if let Some(target)=nearest(actor,ctx,ctx.class3,false,true,|t|t.control_flags & 0x20==0) {
                    actor.set_target(target.slot);actor.set_action(130);
                } else {select_peer(actor,ctx,false);}
            }
        }
    }
    if !stationary && actor.row.action==129 && actor.row.phase%(RULES.scan_period+1)==0
        && let Some(target)=nearest(actor,ctx,ctx.class3,false,false,|_|true) {
        actor.set_target(target.slot);actor.set_action(130);
    }
}

/// sub_24510; damage interrupts the entire attack packet. Burst decrement
/// precedes allocation, and cadence can rearm it after the last launch.
fn attack(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    let hit=consume(actor,ctx);
    if hit!=0 {damage_action(actor,hit);return;}
    move_actor(actor,ctx,effects);
    let Some(target)=ctx.target(actor.target()).and_then(|t|stage::alliance_target(actor,ctx,t,effects)) else {return};
    if actor.row.phase & 7 == 0 && (actor.row.class==3 || distance(actor.row.position,target.position)>=512) {
        actor.row.set_source_roll(heading_for_positions(actor.row.position,target.position) as i16);
    }
    if !living(target) {actor.set_action(129);return;}
    let burst=actor.row.source_dword_10();
    if burst!=0 {
        actor.row.set_source_dword_10(burst.wrapping_sub(1));
        let (yaw,pitch)=crate::level1_mission::projectile_guidance_angles(actor.row.position,target.position);
        effects.projectile=Some(Projectile {
            parent:actor.row.index,owner:actor.row.source_id,target:actor.target(),target_class:target.class,target_model:target.model,
            position:Position {z:actor.row.position.z.wrapping_add((actor.word(92) as i16).wrapping_mul(6)),..actor.row.position},
            target_position:target.position,yaw,pitch,payload:RULES.projectile_payload,damage:RULES.projectile_damage,
        });
    }
    if actor.row.phase%RULES.scan_period==0 {
        if xy(actor.row.position,target.position)<RULES.target_range*RULES.target_range {
            if actor.row.phase%(2*RULES.scan_period)==0 {effects.sound=Some(39);}
            if angle_distance(i32::from(actor.row.yaw),heading_for_positions(actor.row.position,target.position))<227 {
                actor.row.set_source_dword_10(RULES.burst_length);
                effects.target_music=Some(target.slot);
            }
        } else {actor.set_action(129);}
    }
}
fn link(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    let peer=ctx.target(actor.word(54));
    if actor.word(54)==0 {actor.set_action(129);return;}
    let valid=peer.is_some_and(|t|living(t)&&t.class==5&&t.model==SOURCE_MODEL);
    let hit=consume(actor,ctx);
    if hit!=0 {
        if valid && let Some(mut peer)=peer {
            peer.target=actor.attacker();peer.previous=0;peer.action=130;effects.linked_updates.push(peer);
        }
        actor.put(54,0);damage_action(actor,hit);
        if hit==1 {settle(actor,ctx);}
        return;
    }
    move_actor(actor,ctx,effects);
    if !valid {actor.put(54,0);actor.set_action(129);return;}
    if actor.row.phase%RULES.scan_period!=0 {return;}
    let peer=peer.expect("validated link");
    match peer.action.wrapping_sub(128) {
        0|1=>{},
        2|6=>{actor.set_target(peer.target);actor.put(54,0);actor.set_action(peer.action);return;},
        3=>actor.put(54,peer.previous),
        _=>{actor.put(54,0);actor.set_action(129);return;},
    }
    if let Some(target)=ctx.target(actor.word(54)) {
        actor.row.set_source_roll(heading_for_positions(actor.row.position,target.position) as i16);
    }
    if let Some(other)=ctx.family.iter().filter_map(|s|ctx.target(*s)).find(|t|t.source_id!=actor.row.source_id
        && i32::from(actor.row.position.x as i16).abs_diff(i32::from(t.position.x as i16))<256
        && i32::from(actor.row.position.y as i16).abs_diff(i32::from(t.position.y as i16))<256) {
        actor.row.set_source_roll(heading_for_positions(other.position,actor.row.position) as i16);
    }
    actor.row.set_actual_speed((peer.speed as i16).wrapping_add(peer.maximum_speed));
}

pub(crate) fn step(actor:&mut Actor<'_>,ctx:&mut Context<'_>)->Result<Effects,String> {
    let mut effects=Effects::default();
    match actor.row.action {
        128|129=>roam(actor,ctx,&mut effects),
        130=>attack(actor,ctx,&mut effects),
        131=>link(actor,ctx,&mut effects),
        132=>{
            let mut next=actor.word(56);
            let mut killer=actor.word(40);
            for _ in 0..ctx.targets.len() {
                let Some(mut child)=ctx.target(next) else {break};
                next=child.next;child.action=133;
                crate::combat::actors::inherit_killer(&mut killer,child.killer);
                effects.linked_updates.push(child);
            }
            actor.put(40,killer);
            if let Some(killer)=ctx.target(killer) && crate::combat::actors::awards_creature_kill(SOURCE_MODEL,actor.row.source_id,killer.source_id,killer.class,killer.model) {effects.kill_credit=Some(killer.slot);}
            actor.set_action(133);
        },
        133=>effects.kill=actor.row.phase&7==0,
        // str50[134] names205830 but the pinned source has no implementation
        // for that callback. Descriptor84 has no flee flag, so ordinary
        // construction and damage cannot select it.
        134=>return Err("Wyvern action134 has no callback implementation in the pinned source".into()),
        135=>stage::step(actor,ctx,&mut effects),
        action=>return Err(format!("unsupported Wyvern action{action}")),
    }
    Ok(effects)
}
