//! Wyvern sub_1D5D0(128) routes over the same authoritative actor record.

use super::{Actor, Context, Effects, RULES, SOURCE_MODEL, Target, consume, distance,
    heading_for_positions, jitter, living, move_actor, settle, xy};
use crate::Position;

fn near(a:Position,b:Position,radius:u32,signed:bool)->bool {
    let (x,y)=if signed {
        (i32::from(a.x as i16).abs_diff(i32::from(b.x as i16)),i32::from(a.y as i16).abs_diff(i32::from(b.y as i16)))
    } else {(u32::from(a.x.abs_diff(b.x)),u32::from(a.y.abs_diff(b.y)))};
    x<radius && y<radius
}
fn follow(actor:&mut Actor<'_>,ctx:&Context<'_>,point:Position,signed:bool,width:Option<i16>) {
    if actor.row.phase&7!=0 {return;}
    if actor.row.control_flags&0x40000==0 {
        actor.row.set_source_roll(heading_for_positions(actor.row.position,point) as i16);
        if actor.row.phase&63==0 {jitter(actor);}
    }
    if let Some(peer)=ctx.family.iter().filter_map(|s|ctx.target(*s)).find(|t|t.source_id!=actor.row.source_id
        && near(actor.row.position,t.position,u32::from(actor.word(88)),signed)) {
        actor.row.set_source_roll(heading_for_positions(peer.position,actor.row.position) as i16);
    } else if let Some(width)=width && near(actor.row.position,point,(i32::from(actor.word(88))+i32::from(width)) as u32,false) {
        actor.row.set_source_roll(heading_for_positions(point,actor.row.position) as i16);
    }
}
fn idle(actor:&mut Actor<'_>) {
    // sub_1E1C0 and the missing-target tail of sub_1D8C0 draw once
    // every sixteen visits. Point/target following uses the separate two-draw
    // sixty-four-visit jitter above.
    if actor.row.phase&15==0 && actor.row.control_flags&0x40000==0 {
        let rng=super::level2_next_rng16(actor.row.entity_rng);
        actor.row.set_entity_rng(rng);
        actor.row.set_source_roll(actor.row.source_roll().wrapping_add((rng%113+142) as i16)&2047);
    }
}

fn selected(actor:&mut Actor<'_>,ctx:&Context<'_>,route:super::super::archer::ArcherStageDescriptor)->Option<Target> {
    if route.flags&2==0 {return ctx.target(route.actor);}
    let mut stage=actor.stage();
    if stage.relation==0 {
        // sub_1E3E0 first inherits the same-route family relation when bit1
        // permits it, then selects the closest member of the requested frozen
        // model list. Current life/draw flags do not refilter that source list.
        let shared=if route.flags&1!=0 {
            ctx.family.iter().filter_map(|s|ctx.target(*s)).find_map(|peer|
                (peer.stage.index==stage.index && peer.stage.relation!=0).then_some(peer.stage.relation))
        } else {None};
        stage.relation=shared.unwrap_or_else(|| {
            let mut shortest=u32::MAX;
            let mut selected=0;
            let slots=ctx.families.get(usize::from(route.model)).map(Vec::as_slice).unwrap_or(&[]);
            for target in slots.iter().filter_map(|s|ctx.target(*s)) {
                let d=xy(actor.row.position,target.position);
                if d<shortest {shortest=d;selected=target.slot;}
            }
            selected
        });
        actor.set_stage(stage);
    }
    ctx.target(stage.relation)
}
fn blink(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects,clear:bool) {
    let stage=actor.stage();
    let phase=if clear {0} else if stage.remaining<=stage.duration.wrapping_sub(16) && stage.remaining>=16 {
        if actor.row.phase&0x1c==16 {actor.row.phase&3} else {0}
    } else if actor.row.phase&1!=0 {0} else {2};
    let bits=match phase {1|3=>0x02000000,2=>0x04000000,_=>0};
    actor.row.set_control_flags((actor.row.control_flags&!0x06000000)|bits);
    let mut next=actor.word(56);
    for _ in 0..ctx.targets.len() {
        let Some(mut child)=ctx.target(next) else {break};
        next=child.next;
        child.control_flags=(child.control_flags&!0x06000000)|bits;
        effects.linked_updates.push(child);
    }
}
pub(super) fn alliance_target(actor:&mut Actor<'_>,ctx:&Context<'_>,target:Target,effects:&mut Effects)->Option<Target> {
    let mut stage=actor.stage();
    if stage.mode!=14 {return Some(target);}
    blink(actor,ctx,effects,false);
    stage.remaining=stage.remaining.wrapping_sub(1);actor.set_stage(stage);
    if stage.remaining<=0 {return None;}
    if let Some(parent)=ctx.target(stage.parent) {
        let selected=if parent.target!=0 {parent.target} else {parent.attacker};
        if parent.slot==target.slot || (selected!=0 && selected!=target.slot) {return None;}
    }
    Some(target)
}
fn summon_follow(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    let hit=consume(actor,ctx,true);
    if hit==2 {let mut stage=actor.stage();stage.remaining=1;actor.set_stage(stage);return;}
    move_actor(actor,ctx,effects);
    if hit==1 {
        if actor.attacker()!=actor.stage().parent && ctx.target(actor.attacker()).is_none_or(|t|t.class!=5||t.model!=SOURCE_MODEL) {
            actor.set_target(actor.attacker());actor.set_action(130);
            if actor.stage().mode == 14 {
                effects.earlier_feedback=Some((actor.stage().parent,19));
            } else {
                effects.feedback=Some((actor.stage().parent,19));
            }
        }
    } else if let Some(target)=ctx.target(actor.target()) {follow(actor,ctx,target.position,false,None);}
}
fn summoned(actor:&mut Actor<'_>,ctx:&Context<'_>,effects:&mut Effects) {
    let mut stage=actor.stage();
    let parent=ctx.target(stage.parent);
    if stage.mode==14 {
        stage.remaining=stage.remaining.wrapping_sub(1);actor.set_stage(stage);
        if stage.remaining>0 {blink(actor,ctx,effects,false);}
        if stage.remaining<=0 || parent.is_none_or(|t|t.life<=0) {
            stage.mode=10;stage.parent=0;stage.index=stage.relation as u8;
            actor.set_stage(stage);actor.set_target(0);blink(actor,ctx,effects,true);return;
        }
        let parent=parent.expect("living Alliance parent");
        let previous=actor.target();actor.set_target(stage.parent);
        summon_follow(actor,ctx,effects);
        let inherited=if parent.target!=0 {parent.target} else {parent.attacker};
        actor.set_target(if inherited!=0 {inherited} else {previous});
        if let Some(target)=ctx.target(actor.target()) {
            if (target.stage.parent==stage.parent && target.stage.mode==14)||target.life<=0||!living(target) {actor.set_target(0);}
            else if distance(actor.row.position,target.position)<RULES.target_range {
                actor.set_action(130);effects.feedback=Some((stage.parent,24));
            }
        }
        return;
    }
    if parent.is_none_or(|t|!living(t)) {stage.remaining=0;}
    else if stage.mode==13 {stage.remaining=stage.remaining.wrapping_sub(1);}
    actor.set_stage(stage);
    if stage.remaining<=0 {effects.expire=true;effects.summon_expire=stage.mode==13;return;}
    let parent=parent.expect("living summoned parent");
    let mut target=ctx.target(actor.target());
    if target.is_some_and(|t|t.life<=0||!living(t)) {actor.set_target(0);target=None;}
    if target.is_none() && actor.row.phase&7==0 {
        let mut shortest=u32::MAX;
        for candidate in ctx.class3.iter().filter_map(|s|ctx.target(*s)).filter(|t|matches!(t.model,0|1)&&t.source_id!=parent.source_id) {
            let d=xy(parent.position,candidate.position);
            if d<shortest {shortest=d;target=Some(candidate);}
        }
        actor.set_target(target.map_or(0,|t|t.slot));
    }
    if let Some(target)=target {
        summon_follow(actor,ctx,effects);
        if actor.row.phase&7==0 && distance(actor.row.position,target.position)<RULES.target_range {actor.set_action(130);}
    } else {
        actor.set_target(stage.parent);summon_follow(actor,ctx,effects);actor.set_target(0);
        let mut stage=actor.stage();stage.remaining=stage.remaining.wrapping_sub(4);actor.set_stage(stage);
    }
}

pub(super) fn step(actor:&mut Actor<'_>,ctx:&mut Context<'_>,effects:&mut Effects) {
    let stage=actor.stage();
    let mode=stage.mode;
    match mode {
        10=>actor.set_action(130),
        12=>{
            if let Some(parent)=ctx.target(stage.parent) {
                let mut position=parent.position;
                // sub_1E4D0's model16 body sits 896 below its caster.
                position.z=(i32::from(position.z)-896).max(0) as i16;
                actor.row.set_position(position);actor.row.set_yaw(parent.yaw as i16);actor.row.set_source_roll(parent.roll as i16);
                if parent.life<0 {actor.row.disable_drawing();}
            } else {actor.row.set_position(Position::default());actor.row.set_yaw(0);actor.row.set_source_roll(0);actor.row.disable_drawing();}
        },
        13|14|16=>summoned(actor,ctx,effects),
        17=>{
            move_actor(actor,ctx,effects);
            if actor.row.life<0 {actor.row.disable_drawing();}
            else if let Some(target)=ctx.target(actor.target()).filter(|t|living(*t)) {
                actor.row.set_source_roll(heading_for_positions(actor.row.position,target.position) as i16);
                actor.row.set_actual_speed(actor.row.actual_speed.wrapping_sub(8));
                if actor.row.actual_speed<=16 {let mut s=actor.stage();s.mode=16;actor.set_stage(s);}
            } else {let mut s=actor.stage();s.mode=16;actor.set_stage(s);}
        },
        1..=9=>{
            let route=ctx.runtime.descriptor(stage.index);
            let point=route.is_some_and(|r|mode==1||(mode==2&&!near(actor.row.position,r.point,3073,true)));
            match consume(actor,ctx,!point) {
                2=>actor.set_action(132),
                1=>{
                    if ctx.target(actor.attacker()).is_none_or(|t|t.class!=5||t.model!=SOURCE_MODEL) {
                        actor.set_target(actor.attacker());let mut s=actor.stage();s.mode=10;actor.set_stage(s);actor.set_action(130);
                    }
                    settle(actor,ctx);
                },
                _=>{
                    let wander=matches!(mode,6..=9)||(mode==2&&!point);
                    if !wander||RULES.behavior_flags&2==0 {
                        move_actor(actor,ctx,effects);
                        if point {
                            if let Some(route)=route {follow(actor,ctx,route.point,true,None);}
                        } else if matches!(mode,3..=5)&&actor.row.phase&7==0 {
                            if let Some(route)=route {
                                if let Some(target)=selected(actor,ctx,route) {follow(actor,ctx,target.position,false,Some(target.half_width));}
                                else {idle(actor);}
                            }
                        } else if wander {idle(actor);}
                    }
                }
            }
            if matches!(mode,3|4)&&actor.row.phase&7==0&&let Some(route)=route {
                let slot=if route.flags&2!=0 {actor.stage().relation} else {route.actor};
                if let Some(target)=ctx.target(slot) {
                    let target=if mode==4 {ctx.target(target.target)} else {Some(target)};
                    if let Some(target)=target.filter(|t|t.slot!=0xae02)&&distance(actor.row.position,target.position)<=RULES.target_range {
                        actor.set_target(target.slot);let mut s=actor.stage();s.mode=10;actor.set_stage(s);actor.set_action(130);
                    }
                }
            }
            if mode==2&&actor.row.action==135&&actor.stage().mode==2&&actor.row.record[61]!=0&&actor.row.phase%RULES.scan_period==0 {
                let mut shortest=u32::MAX;let mut selected=None;
                for target in ctx.class3.iter().filter_map(|s|ctx.target(*s)).filter(|t|t.control_flags&0x20==0) {
                    // sub1DBF0 promotes X but retains signed-short Y.
                    let x=i32::from(target.position.x)-i32::from(actor.row.position.x);
                    let y=i32::from(target.position.y.wrapping_sub(actor.row.position.y) as i16);
                    let d=x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32;
                    if d<=RULES.target_range*RULES.target_range&&d<shortest&&super::angle_distance(i32::from(actor.row.yaw),heading_for_positions(actor.row.position,target.position))<RULES.target_half_angle {
                        shortest=d;selected=Some(target);
                    }
                }
                if let Some(target)=selected {actor.set_target(target.slot);let mut s=actor.stage();s.mode=10;actor.set_stage(s);actor.set_action(130);}
            }
        },
        _=>{},
    }
}
