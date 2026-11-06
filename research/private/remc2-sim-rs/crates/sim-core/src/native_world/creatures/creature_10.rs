//! Source class5/model10 final-encounter actor.
//!
//! The MC2 adapter translates this family into the existing source-row
//! carrier. Mission objectives, allocation and the following-pass reclaim
//! remain owned by the campaign orchestrator.

use crate::Position;

pub(crate) mod mc2;
pub(crate) mod protection;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 10;
pub(crate) const SOURCE_INITIAL_ACTION: u8 = 80;
pub(crate) const SOURCE_DESCRIPTOR: u16 = 107;
pub(crate) const SOURCE_PARTICLE: u16 = 341;
pub(crate) const SOURCE_FEATURE_MASK: u8 = 2;
pub(crate) const SOURCE_INITIAL_LIFE: i32 =
    crate::level_definition::FINAL_BOSS_CLASS_PROPERTIES.max_life;
// NewEvent_4A050 zeroes mana, and sub_4BD00 does not replace it.
pub(crate) const SOURCE_INITIAL_MANA: i32 =
    crate::level_definition::FINAL_BOSS_CLASS_PROPERTIES.initial_mana;

/// Frame-local view of the boss fields which `sub_21030`, `sub_22190` and
/// `sub_21AB0` mutate. The source arena row remains authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Actor {
    pub slot: u16,
    pub source_id: u16,
    pub action: u8,
    pub phase: u8,
    pub flags: u32,
    pub damage_enabled: bool,
    pub position: Position,
    pub rng: u16,
    pub yaw: i16,
    pub roll: i16,
    pub life: i32,
    pub max_life: i32,
    pub timer: i32,
    pub attack_remaining: u16,
    pub source_animation: u16,
    pub subspell_index: u16,
    pub summon_mode: u8,
    pub summon_variant: u8,
    pub summon_angle_step: u16,
    pub target: u16,
    pub visual: u16,
    pub animation_frame: u8,
    pub animation_limit: u8,
    pub current_attacker: u16,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub source_counter: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DamageResult {
    NoHit,
    Hit {
        applied: i32,
        lethal_threshold: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SummonKind {
    Dragon,
    Model21,
    Model25,
    Firefly,
    Class9Projectile { subtype: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SummonRequest {
    pub kind: SummonKind,
    pub position: Position,
    /// Post-constructor source writes; `None` preserves the constructor field.
    pub yaw: Option<i16>,
    pub pitch: Option<i16>,
    pub roll: Option<i16>,
    /// Source `id_0x1A_26`: the boss owner, not the player target.
    pub owner: u16,
    /// Type-5 child rows store the target player slot directly. Class-9
    /// projectiles instead retain target class/model in their source fields.
    pub target_player_slot: Option<u16>,
    pub target_player_class: Option<u8>,
    pub target_player_model: Option<u8>,
    pub parent_slot: Option<u16>,
    pub source_stage_variable: Option<u8>,
    pub source_timer: Option<u16>,
    pub actual_speed: Option<i16>,
    pub action: u8,
    pub source_type: u8,
    pub source_subtype: u8,
    pub source_variant: u8,
    pub descriptor: Option<u16>,
    pub subspell_index: Option<u16>,
    pub life: Option<i32>,
    pub max_life: Option<i32>,
    /// The source plays these sounds only after the requested child allocates.
    pub success_sound: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceEffectRequest {
    pub source_type: u8,
    pub source_subtype: u8,
    pub position: Position,
    pub post_position: Option<Position>,
    pub owner_source_id: u16,
    pub life: Option<i32>,
    pub max_life: Option<i32>,
}

/// Direct player displacement from source summon mode 7. The source arena
/// admits `proposed_position` through its existing move test before applying
/// terrain clearance; this request is not an allocated entity or a claim that
/// the move was accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlayerPressureEffect {
    pub player_slot: u16,
    pub speed: i16,
    pub heading: i16,
    pub proposed_position: Position,
    /// `sub_5C800(player, 6)` changes the local palette only on attack start.
    pub palette_submod: Option<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    pub animation: Option<u16>,
    pub sounds: Vec<u8>,
    pub summons: Vec<SummonRequest>,
    pub source_effects: Vec<SourceEffectRequest>,
    pub player_pressure: Option<PlayerPressureEffect>,
    pub request_approach_step: bool,
    pub kill_all_creatures: bool,
    pub refresh_creatures: bool,
    /// Source `sub_21490` world-wide byte mutations, for typed rows that are
    /// not mirrored in SourceArena.entities. Values are the source v29 mode.
    pub approach_creature_mode: Option<u8>,
    /// Source `countStageVars_0x36E00 = 0` from the stage-4 approach path.
    /// The campaign integration applies this to its authoritative StageVar
    /// count carrier; the family does not replace the decoded table.
    pub clear_stage_variable_count: bool,
    /// A source phase15 10/9 attempt is pending; the caller consumes this
    /// marker on both allocation success and failure.
    pub final_death_reward_pending: bool,
    /// Slot of the actual constructed 10/9 reward volcano. This is not an
    /// objective or victory signal.
    pub final_death_reward_slot: Option<u16>,
    /// The source word36548 lifecycle write, applied by the shared dispatcher
    /// after this callback while retaining the entity through objective reads.
    pub final_boss_active_counter: Option<i16>,
    /// True when the one-shot phase15 volcano constructor was attempted.
    pub final_death_reward_attempted: bool,
    /// Source kill credit returned by the shared class-5 pre-kill chain walk.
    pub kill_credit: Option<u16>,
    pub hide: bool,
}

/// Per-pass world facts consumed by the source boss callback. Population
/// limits come from the live shared source arena, so summon decisions cannot
/// drift into a family-owned counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Frame {
    pub player_slot: u16,
    pub player_source_id: u16,
    pub player_position: Position,
    pub player_alive: bool,
    pub close_creature: bool,
    pub model0_count: u16,
    pub model19_count: u16,
    pub model21_count: u16,
    pub model25_count: u16,
    pub source_setting: u16,
    /// The optional literal source animation-table read for this callback.
    /// Unloaded assets leave the source's already assigned timer intact.
    pub source_animation_timer: Option<u16>,
    /// Source global `word_0x36546`, shared by the mode-7 push callback.
    pub pressure_speed: i16,
    pub player_class: u8,
    pub player_model: u8,
}

const FINAL_REWARD_PHASE: u8 = 15;

fn next_rng(actor: &mut Actor) -> u16 {
    actor.rng = actor.rng.wrapping_mul(9_377).wrapping_add(9_439);
    actor.rng
}

/// `sub_22190`: only a nonzero damage-owner word is a queued hit. Incoming
/// damage is clamped to 1..=300, then life below ten is pinned to eight so the
/// normal phase-three transition starts the death sequence on its next pass.
pub(crate) fn consume_damage(actor: &mut Actor) -> DamageResult {
    if !actor.damage_enabled {
        return DamageResult::NoHit;
    }
    if actor.pending_attacker == 0 {
        actor.current_attacker = 0;
        return DamageResult::NoHit;
    }
    let applied = actor.pending_damage.clamp(1, 300);
    actor.life = actor.life.wrapping_sub(applied);
    actor.current_attacker = actor.pending_attacker;
    actor.pending_attacker = 0;
    if actor.life < 10 {
        actor.life = 8;
        DamageResult::Hit {
            applied,
            lethal_threshold: true,
        }
    } else {
        DamageResult::Hit {
            applied,
            lethal_threshold: false,
        }
    }
}

fn request_source_effect(
    effects: &mut Effects,
    source_subtype: u8,
    position: Position,
    owner_source_id: u16,
    post_position: Option<Position>,
    life: Option<i32>,
    max_life: Option<i32>,
) {
    effects.source_effects.push(SourceEffectRequest {
        source_type: 10,
        source_subtype,
        position,
        post_position,
        owner_source_id,
        life,
        max_life,
    });
    if source_subtype == 9 {
        effects.final_death_reward_pending = true;
    }
}

fn step_phase_three(actor: &mut Actor, frame: Frame) {
    if actor.life < 10 {
        actor.phase = 12;
    } else if actor.subspell_index & 1 != 0 {
        actor.phase = 6;
    } else {
        actor.timer = actor.timer.wrapping_sub(1);
        if frame.player_alive
            && distance(actor.position, frame.player_position) < 0x2000
            && actor.timer <= 0
        {
            actor.phase = if next_rng(actor) % 12 < 9 { 4 } else { 6 };
        }
    }
}

fn step_phase_nine(actor: &mut Actor, frame: Frame, effects: &mut Effects) {
    step_summon(actor, frame, effects);
    actor.timer = actor.timer.wrapping_sub(1);
    if actor.timer <= 0 {
        actor.phase = 10;
    }
}

/// One UpdateEntities callback for source action80. Child constructors and
/// final objective signaling are returned as ordered requests for the shared
/// source arena to allocate before its same-pass tail.
pub(crate) fn step(
    actor: &mut Actor,
    frame: Frame,
    particles: &[crate::native_world::ParticleParameter],
) -> Result<Effects, String> {
    if actor.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT != 0 {
        return Ok(Effects::default());
    }
    match actor.action {
        SOURCE_INITIAL_ACTION => {}
        81..=83 | 86..=87 => {
            // Source callbacks 0x51..0x53 and 0x56..0x57 only restore action80.
            actor.action = SOURCE_INITIAL_ACTION;
            return Ok(Effects::default());
        }
        action => {
            return Err(format!(
                "unsupported final encounter action{action} in update callback"
            ));
        }
    }
    let mut effects = Effects::default();
    if frame.close_creature {
        actor.subspell_index |= 1;
    }
    if (2..12).contains(&actor.phase) && actor.life >= 10 {
        let _ = consume_damage(actor);
    }

    match actor.phase {
        0 => {
            effects.final_boss_active_counter = Some(1);
            actor.phase = 1;
            actor.subspell_index = 8;
            actor.timer = 15;
            actor.source_animation = 22;
            actor.target = frame.player_slot;
            effects.request_approach_step = true;
        }
        1 => effects.request_approach_step = true,
        2 => {
            let rng = next_rng(actor);
            let remaining = (26_i64 * i64::from(actor.life) / i64::from(actor.max_life.max(1)))
                - i64::from(rng & 7);
            actor.phase = 3;
            actor.timer = remaining.clamp(3, 26) as i32;
            actor.source_animation = 22;
            actor.summon_variant = 0;
            set_animation(actor, &mut effects, particles, 341)?;
            step_phase_three(actor, frame);
        }
        3 => step_phase_three(actor, frame),
        4 => {
            actor.phase = 5;
            actor.timer = 6;
            actor.source_animation = 113;
            actor.summon_variant = 2;
            actor.timer = actor.timer.wrapping_sub(1);
        }
        5 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 6;
            }
        }
        6 => {
            actor.phase = 7;
            actor.timer = 16;
            actor.source_animation = 113;
            actor.summon_variant = 0;
            set_animation(actor, &mut effects, particles, 343)?;
            if let Some(timer) = frame.source_animation_timer {
                actor.timer = i32::from(timer);
            }
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 8;
            }
        }
        7 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 8;
            }
        }
        8 => {
            actor.phase = 9;
            actor.timer = 0;
            actor.source_animation = 22;
            actor.summon_variant = 3;
            set_animation(actor, &mut effects, particles, 342)?;
            select_attack(actor, frame);
            step_phase_nine(actor, frame, &mut effects);
        }
        9 => step_phase_nine(actor, frame, &mut effects),
        10 => {
            actor.phase = 11;
            actor.timer = 16;
            actor.source_animation = 22;
            set_animation(actor, &mut effects, particles, 344)?;
            if let Some(timer) = frame.source_animation_timer {
                actor.timer = i32::from(timer);
            }
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 2;
            }
        }
        11 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 2;
            }
        }
        12 => {
            actor.phase = 13;
            actor.timer = 32;
            request_source_effect(
                &mut effects,
                17,
                actor.position,
                actor.source_id,
                Some(Position {
                    z: 0,
                    ..actor.position
                }),
                Some(70),
                Some(70),
            );
            actor.timer = actor.timer.wrapping_sub(1);
        }
        13 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = 14;
                actor.timer = 32;
                set_animation(actor, &mut effects, particles, 345)?;
                if let Some(timer) = frame.source_animation_timer {
                    actor.timer = i32::from(timer);
                }
            }
        }
        14 => {
            effects.sounds.push(10);
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                actor.phase = FINAL_REWARD_PHASE;
                actor.timer = 60;
                actor.life = -1;
                actor.flags |= 1;
                effects.kill_all_creatures = true;
                effects.refresh_creatures = true;
            }
        }
        FINAL_REWARD_PHASE => {
            effects.kill_all_creatures = true;
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                request_source_effect(
                    &mut effects,
                    9,
                    actor.position,
                    actor.source_id,
                    None,
                    Some(32),
                    Some(11),
                );
                actor.flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
                effects.final_boss_active_counter = Some(0);
                effects.hide = true;
            }
        }
        phase => return Err(format!("unsupported final encounter phase{phase}")),
    }
    Ok(effects)
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn set_animation(
    actor: &mut Actor,
    effects: &mut Effects,
    particles: &[crate::native_world::ParticleParameter],
    animation: u16,
) -> Result<(), String> {
    let parameter = particles
        .get(usize::from(animation))
        .ok_or_else(|| format!("final encounter requires retained particle{animation}"))?;
    let limit = parameter
        .animation_limit()
        .ok_or_else(|| format!("final encounter particle{animation} has invalid animation mode"))?;
    actor.visual = animation;
    actor.animation_frame = 0;
    actor.animation_limit = limit;
    effects.animation = Some(animation);
    Ok(())
}

fn select_attack(actor: &mut Actor, frame: Frame) {
    let mut flags = actor.subspell_index & !2;
    actor.subspell_index = flags;
    if flags & 1 != 0 {
        flags &= !1;
        actor.subspell_index = flags;
        if actor.damage_enabled {
            actor.subspell_index = flags | 2;
            actor.summon_mode = 7;
            actor.attack_remaining = 24;
            actor.timer = 32;
        }
        return;
    }

    actor.subspell_index = flags | 2;
    let mut low_roll = next_rng(actor) % 0x46;
    actor.rng = actor.rng.wrapping_add(frame.source_setting);
    if actor.subspell_index as u8 & 0x80 != 0 {
        low_roll = 0;
        actor.subspell_index &= !0x80;
    }

    if (3..=6).contains(&low_roll) {
        actor.summon_mode = 7;
        actor.attack_remaining = 24;
        actor.timer = 32;
        return;
    }

    let mut secondary = false;
    if low_roll < 40 {
        secondary = true;
    } else if low_roll <= 48 {
        actor.timer = 8;
        actor.attack_remaining = 8;
        actor.summon_angle_step = 256;
        if frame.model19_count < 28 {
            actor.summon_mode = 6;
        } else {
            secondary = true;
        }
    } else if low_roll <= 58 {
        actor.timer = 8;
        actor.attack_remaining = 3;
        actor.summon_angle_step = 682;
        if frame.model0_count < 4 {
            actor.summon_mode = 3;
        } else {
            secondary = true;
        }
    } else if low_roll <= 68 {
        actor.timer = 8;
        actor.attack_remaining = 3;
        actor.summon_angle_step = 682;
        if frame.model25_count < 6 {
            actor.summon_mode = 5;
        } else {
            secondary = true;
        }
    } else {
        actor.timer = 8;
        actor.attack_remaining = 3;
        actor.summon_angle_step = 682;
        if frame.model21_count < 12 {
            actor.summon_mode = 4;
        } else {
            secondary = true;
        }
    }

    if secondary {
        let roll = u32::from(next_rng(actor) % 0x1d);
        actor.rng = actor.rng.wrapping_add(frame.source_setting);
        match roll {
            0..=7 => {
                actor.summon_mode = 1;
                actor.attack_remaining = 10;
                actor.timer = 10;
            }
            8..=17 => {
                actor.summon_mode = 2;
                actor.attack_remaining = 8;
                actor.timer = 8;
            }
            18..=25 => {
                actor.summon_mode = 9;
                actor.attack_remaining = 1;
                actor.timer = 5;
            }
            26..=27 => {
                actor.summon_mode = 8;
                actor.attack_remaining = 1;
                actor.timer = 5;
            }
            _ => {
                actor.summon_mode = 7;
                actor.attack_remaining = 24;
                actor.timer = 32;
            }
        }
    }
    actor.subspell_index |= 2;
}

fn summon_position(actor: Actor, distance: i16) -> Position {
    let mut position = actor.position;
    crate::move_entity(&mut position, actor.yaw, 0, distance);
    position.z = position.z.wrapping_add(768);
    position
}

fn step_summon(actor: &mut Actor, frame: Frame, effects: &mut Effects) {
    if !frame.player_alive || actor.attack_remaining == 0 {
        return;
    }
    actor.attack_remaining = actor.attack_remaining.wrapping_sub(1);
    let mut position = summon_position(*actor, 640);
    let mut success_sound = None;
    let mut action = 0;
    let mut kind = None;
    let mut source_type = 9;
    let mut source_subtype = 0;
    let mut source_variant = 0;
    let mut spawn_yaw = actor.yaw;
    let mut yaw = None;
    let mut pitch = None;
    let mut roll = None;
    let mut parent_slot = None;
    let mut target_player_slot = None;
    let mut target_player_class = None;
    let mut target_player_model = None;
    let mut source_stage_variable = None;
    let mut source_timer = None;
    let mut actual_speed = None;
    let mut descriptor = None;
    let mut subspell_index = None;
    let mut life = None;
    let mut max_life = None;

    match actor.summon_mode {
        1 => {
            kind = Some(SummonKind::Class9Projectile { subtype: 0 });
            source_subtype = 0;
            descriptor = Some(62);
            subspell_index = Some(800);
            success_sound = Some(15);
        }
        2 => {
            kind = Some(SummonKind::Class9Projectile { subtype: 9 });
            source_subtype = 9;
            source_variant = 23;
            descriptor = Some(62);
            subspell_index = Some(800);
            success_sound = Some(23);
        }
        3..=6 => {
            source_type = 5;
            let angle = (u32::from(actor.summon_angle_step)
                .wrapping_mul(u32::from(actor.attack_remaining))
                .wrapping_add(u32::from(actor.yaw as u16)))
                & 0x7ff;
            crate::move_entity(&mut position, angle as i16, 0, 1_792);
            spawn_yaw = angle as i16;
            yaw = Some(spawn_yaw);
            roll = Some(spawn_yaw);
            parent_slot = Some(actor.slot);
            target_player_slot = Some(frame.player_slot);
            source_stage_variable = Some(17);
            source_timer = Some(250);
            actual_speed = Some(320);
            match actor.summon_mode {
                3 => {
                    kind = Some(SummonKind::Dragon);
                    action = 7;
                    success_sound = Some(8);
                }
                4 => {
                    kind = Some(SummonKind::Model21);
                    action = 175;
                    success_sound = Some(42);
                }
                5 => {
                    kind = Some(SummonKind::Model25);
                    action = 207;
                    success_sound = Some(37);
                }
                _ => {
                    kind = Some(SummonKind::Firefly);
                    action = 159;
                    success_sound = Some(44);
                }
            }
        }
        7 => {
            let starts_pressure = actor.subspell_index & 2 != 0;
            let palette_submod = if starts_pressure {
                actor.subspell_index &= !2;
                effects.sounds.push(19);
                Some(6)
            } else {
                None
            };
            let initial_speed = if starts_pressure {
                1_024
            } else {
                frame.pressure_speed
            };
            let speed = initial_speed.wrapping_sub(80).clamp(10, 1_024);
            let heading = crate::level1_mission::projectile_guidance_angles(
                actor.position,
                frame.player_position,
            )
            .0;
            let mut proposed_position = frame.player_position;
            crate::move_entity(&mut proposed_position, heading, 0, speed);
            effects.player_pressure = Some(PlayerPressureEffect {
                player_slot: frame.player_slot,
                speed,
                heading,
                proposed_position,
                palette_submod,
            });
            return;
        }
        8 => {
            kind = Some(SummonKind::Class9Projectile { subtype: 26 });
            source_subtype = 26;
            source_variant = 22;
            subspell_index = Some(20);
            action = 3;
            success_sound = Some(15);
        }
        9 => {
            kind = Some(SummonKind::Class9Projectile { subtype: 3 });
            source_subtype = 3;
            source_variant = 17;
            subspell_index = Some(6_000);
            action = 10;
            success_sound = Some(15);
        }
        _ => return,
    }

    if let Some(kind) = kind {
        if source_type == 9 {
            let (aim_yaw, aim_pitch) = crate::level1_mission::projectile_guidance_angles(
                actor.position,
                frame.player_position,
            );
            yaw = Some(aim_yaw);
            pitch = Some(aim_pitch);
            target_player_class = Some(frame.player_class);
            target_player_model = Some(frame.player_model);
        }
        effects.summons.push(SummonRequest {
            kind,
            position,
            yaw,
            pitch,
            roll,
            owner: actor.source_id,
            target_player_slot,
            target_player_class,
            target_player_model,
            parent_slot,
            source_stage_variable,
            source_timer,
            actual_speed,
            action,
            source_type,
            source_subtype,
            source_variant,
            descriptor,
            subspell_index,
            life,
            max_life,
            success_sound,
        });
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            player_slot: 0,
            player_source_id: 0,
            player_position: Position::default(),
            player_alive: false,
            close_creature: false,
            model0_count: 0,
            model19_count: 0,
            model21_count: 0,
            model25_count: 0,
            source_setting: 0,
            source_animation_timer: None,
            pressure_speed: 0,
            player_class: 0,
            player_model: 0,
        }
    }
}

/// Actions 80..87 are the source family callbacks. An unsupported action must
/// stay outside this owner and must not be treated as a successful empty step.
pub(crate) const fn supports_action(action: u8) -> bool {
    matches!(action, 80..=87)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressure_actor() -> Actor {
        Actor {
            slot: 4,
            source_id: 44,
            action: SOURCE_INITIAL_ACTION,
            phase: 9,
            flags: 0,
            damage_enabled: true,
            position: Position {
                x: 2_000,
                y: 3_000,
                z: 4_000,
            },
            rng: 0,
            yaw: 0,
            roll: 0,
            life: SOURCE_INITIAL_LIFE,
            max_life: SOURCE_INITIAL_LIFE,
            timer: 8,
            attack_remaining: 2,
            source_animation: 22,
            subspell_index: 2,
            summon_mode: 7,
            summon_variant: 0,
            summon_angle_step: 0,
            target: 0,
            visual: SOURCE_PARTICLE,
            animation_frame: 0,
            animation_limit: 1,
            current_attacker: 0,
            pending_damage: 0,
            pending_attacker: 0,
            source_counter: 0,
        }
    }

    #[test]
    fn source_attack_selection_preserves_fallback_draws_and_population_limits() {
        fn draw(value: u16) -> u16 {
            value.wrapping_mul(9_377).wrapping_add(9_439)
        }

        const SETTING: u16 = 17;
        fn seed_for(low: u16, secondary: Option<u16>) -> u16 {
            (0..=u16::MAX)
                .find(|&seed| {
                    let first = draw(seed);
                    first % 70 == low
                        && secondary.is_none_or(|roll| {
                            draw(first.wrapping_add(SETTING)) % 29 == roll
                        })
                })
                .expect("source rolls have an available seed")
        }
        let frame = Frame {
            source_setting: SETTING,
            model19_count: 27,
            model0_count: 3,
            model25_count: 5,
            model21_count: 11,
            ..Frame::default()
        };

        // Non-special low rolls must take the second RNG draw and preserve
        // its source-setting update. Exercise each secondary interval.
        for (low, roll, mode, remaining, timer) in [
            (0, 0, 1, 10, 10),
            (2, 8, 2, 8, 8),
            (7, 18, 9, 1, 5),
            (39, 26, 8, 1, 5),
            (39, 28, 7, 24, 32),
        ] {
            let mut actor = pressure_actor();
            actor.rng = seed_for(low, Some(roll));
            let first = draw(actor.rng).wrapping_add(SETTING);
            let expected_rng = draw(first).wrapping_add(SETTING);
            select_attack(&mut actor, frame);
            assert_eq!(
                (actor.summon_mode, actor.attack_remaining, actor.timer),
                (mode, remaining, timer)
            );
            assert_eq!(actor.rng, expected_rng);
            assert_eq!(actor.subspell_index & 2, 2);
        }

        // Rolls 3 through 6 take the pressure branch without a second draw.
        for low in 3..=6 {
            let mut actor = pressure_actor();
            actor.rng = seed_for(low, None);
            let expected_rng = draw(actor.rng).wrapping_add(SETTING);
            select_attack(&mut actor, frame);
            assert_eq!(
                (actor.summon_mode, actor.attack_remaining, actor.timer),
                (7, 24, 32)
            );
            assert_eq!(actor.rng, expected_rng);
        }

        // Each population cap admits its source branch below the cap; at the
        // cap, the very same first roll proceeds through the second draw.
        for (low, admitted_mode) in [
            (40, 6), (48, 6), (49, 3), (58, 3), (59, 5), (68, 5), (69, 4),
        ] {
            let seed = seed_for(low, Some(18));
            let first = draw(seed).wrapping_add(SETTING);
            let mut admitted = pressure_actor();
            admitted.rng = seed;
            select_attack(&mut admitted, frame);
            assert_eq!(admitted.summon_mode, admitted_mode, "first roll {low}");
            assert_eq!(admitted.rng, first);

            let capped = Frame {
                model19_count: 28,
                model0_count: 4,
                model25_count: 6,
                model21_count: 12,
                ..frame
            };
            let mut rejected = pressure_actor();
            rejected.rng = seed;
            select_attack(&mut rejected, capped);
            assert_eq!(
                (rejected.summon_mode, rejected.attack_remaining, rejected.timer),
                (9, 1, 5)
            );
            assert_eq!(rejected.rng, draw(first).wrapping_add(SETTING));
        }

        // The source high-bit override forces the non-special low path.
        let mut overridden = pressure_actor();
        overridden.rng = seed_for(69, Some(28));
        overridden.subspell_index = 0x80;
        let first = draw(overridden.rng).wrapping_add(SETTING);
        select_attack(&mut overridden, frame);
        assert_eq!(
            (overridden.summon_mode, overridden.attack_remaining, overridden.timer),
            (7, 24, 32)
        );
        assert_eq!(overridden.rng, draw(first).wrapping_add(SETTING));
        assert_eq!(overridden.subspell_index & 0x82, 2);
    }

    #[test]
    fn source_animation_timer_controls_phase_tails_only_when_read() {
        let particle = crate::native_world::ParticleParameter {
            base_tmap: 0,
            word_2: 0,
            word_4: 0,
            speed: 0,
            rotation_speed: 0,
            byte_10: 0,
            byte_11: 0,
            render_mode: 17,
            byte_13: 0,
        };
        let particles = vec![particle; 346];
        // None is the unloaded path; Some is the value read from the
        // source animation record, not the animation's frame count.
        for (phase, read, next_phase, timer, visual) in [
            (6, None, 7, 15, 343),
            (6, Some(5), 7, 4, 343),
            (6, Some(1), 8, 0, 343),
            (6, Some(0), 8, -1, 343),
            (10, None, 11, 15, 344),
            (10, Some(5), 11, 4, 344),
            (10, Some(1), 2, 0, 344),
            (10, Some(0), 2, -1, 344),
            (13, None, 14, 32, 345),
            (13, Some(5), 14, 5, 345),
            (13, Some(0), 14, 0, 345),
        ] {
            let mut actor = pressure_actor();
            actor.phase = phase;
            actor.timer = 1;
            let frame = Frame {
                source_animation_timer: read,
                ..Frame::default()
            };
            let effects = step(&mut actor, frame, &particles).unwrap();
            assert_eq!((actor.phase, actor.timer), (next_phase, timer));
            assert_eq!((actor.visual, effects.animation), (visual, Some(visual)));
        }

        let mut actor = pressure_actor();
        actor.phase = 13;
        actor.timer = 2;
        let frame = Frame {
            source_animation_timer: Some(0),
            ..Frame::default()
        };
        let effects = step(&mut actor, frame, &particles).unwrap();
        assert_eq!((actor.phase, actor.timer), (13, 1));
        assert_eq!(effects.animation, None);
    }

    #[test]
    fn pressure_mode_pushes_live_player_without_allocating_a_wave() {
        let mut actor = pressure_actor();
        let mut frame = Frame {
            player_slot: 7,
            player_position: Position {
                x: 12_000,
                y: 9_000,
                z: 128,
            },
            player_alive: true,
            pressure_speed: 0,
            ..Frame::default()
        };
        let mut effects = Effects::default();

        step_summon(&mut actor, frame, &mut effects);

        let push = effects
            .player_pressure
            .expect("mode7 emits direct player push");
        assert_eq!(push.player_slot, frame.player_slot);
        assert_eq!(push.speed, 944);
        assert_eq!(push.palette_submod, Some(6));
        assert_ne!(push.proposed_position, frame.player_position);
        assert_eq!(actor.subspell_index & 2, 0);
        assert_eq!(effects.sounds.as_slice(), &[19]);
        assert!(effects.summons.is_empty());

        actor.attack_remaining = 2;
        frame.pressure_speed = push.speed;
        let mut next_effects = Effects::default();
        step_summon(&mut actor, frame, &mut next_effects);
        let next_push = next_effects
            .player_pressure
            .expect("active pressure continues direct player push");
        assert_eq!(next_push.speed, 864);
        assert_eq!(next_push.palette_submod, None);
        assert!(next_effects.sounds.is_empty());
        assert!(next_effects.summons.is_empty());
    }

    #[test]
    fn pressure_mode_with_no_live_player_leaves_the_attack_unconsumed() {
        let mut actor = pressure_actor();
        let frame = Frame {
            player_alive: false,
            pressure_speed: 944,
            ..Frame::default()
        };
        let mut effects = Effects::default();

        step_summon(&mut actor, frame, &mut effects);

        assert_eq!(actor.attack_remaining, 2);
        assert_eq!(actor.subspell_index & 2, 2);
        assert!(effects.player_pressure.is_none());
        assert!(effects.summons.is_empty());
        assert!(effects.sounds.is_empty());
    }

    #[test]
    fn dragon7_request_preserves_source_parent_stage_and_target_fields() {
        let mut actor = pressure_actor();
        actor.summon_mode = 3;
        actor.attack_remaining = 3;
        actor.summon_angle_step = 682;
        actor.subspell_index = 0;
        let frame = Frame {
            player_slot: 7,
            player_source_id: 99,
            player_position: Position {
                x: 12_000,
                y: 9_000,
                z: 128,
            },
            player_alive: true,
            player_class: 3,
            player_model: 0,
            ..Frame::default()
        };
        let mut effects = Effects::default();

        step_summon(&mut actor, frame, &mut effects);

        let request = effects.summons.first().expect("mode3 constructs a Dragon");
        assert_eq!(request.kind, SummonKind::Dragon);
        assert_eq!((request.source_type, request.source_subtype), (5, 0));
        assert_eq!(request.action, 7);
        assert_eq!(request.owner, actor.source_id);
        assert_eq!(request.target_player_slot, Some(frame.player_slot));
        assert_eq!(request.target_player_class, None);
        assert_eq!(request.target_player_model, None);
        assert_eq!(request.parent_slot, Some(actor.slot));
        assert_eq!(request.source_stage_variable, Some(17));
        assert_eq!(request.source_timer, Some(250));
        assert_eq!(request.actual_speed, Some(320));
        assert_eq!(request.yaw, request.roll);
        assert_eq!(request.pitch, None, "the constructor's pitch is retained");
    }

    #[test]
    fn class9_projectile_requests_keep_exact_selector_and_boss_owner() {
        for (mode, subtype, variant, subspell, action) in [
            (1, 0, 0, Some(800), 0),
            (2, 9, 23, Some(800), 0),
            (8, 26, 22, Some(20), 3),
            (9, 3, 17, Some(6_000), 10),
        ] {
            let mut actor = pressure_actor();
            actor.summon_mode = mode;
            actor.attack_remaining = 2;
            actor.subspell_index = 0;
            let frame = Frame {
                player_slot: 7,
                player_source_id: 99,
                player_position: Position {
                    x: 12_000,
                    y: 9_000,
                    z: 128,
                },
                player_alive: true,
                player_class: 3,
                player_model: 0,
                ..Frame::default()
            };
            let mut effects = Effects::default();

            step_summon(&mut actor, frame, &mut effects);

            let request = effects.summons.first().expect("mode emits class9 product");
            assert_eq!(request.kind, SummonKind::Class9Projectile { subtype });
            assert_eq!((request.source_type, request.source_subtype), (9, subtype));
            assert_eq!(request.source_variant, variant);
            assert_eq!(request.subspell_index, subspell);
            assert_eq!(request.action, action);
            assert_eq!(request.owner, actor.source_id);
            assert_eq!(request.target_player_slot, None);
            assert_eq!(request.target_player_class, Some(frame.player_class));
            assert_eq!(request.target_player_model, Some(frame.player_model));
            assert!(request.yaw.is_some());
            assert!(request.pitch.is_some());
        }
    }

    #[test]
    fn damage_mailbox_caps_hits_and_ignores_missing_attacker() {
        let mut actor = pressure_actor();
        actor.life = 1_000;
        actor.pending_damage = 500;
        actor.pending_attacker = 9;

        assert_eq!(
            consume_damage(&mut actor),
            DamageResult::Hit {
                applied: 300,
                lethal_threshold: false,
            }
        );
        assert_eq!(actor.life, 700);
        assert_eq!(actor.current_attacker, 9);
        assert_eq!(actor.pending_attacker, 0);

        actor.pending_damage = 700;
        assert_eq!(consume_damage(&mut actor), DamageResult::NoHit);
        assert_eq!(actor.life, 700);
        assert_eq!(actor.current_attacker, 0);

        actor.life = 20;
        actor.pending_damage = 15;
        actor.pending_attacker = 10;
        assert_eq!(
            consume_damage(&mut actor),
            DamageResult::Hit {
                applied: 15,
                lethal_threshold: true,
            }
        );
        assert_eq!(actor.life, 8);
    }

    #[test]
    fn first_callback_activates_the_source_boss_counter() {
        let mut actor = pressure_actor();
        actor.phase = 0;
        let effects = step(&mut actor, Frame::default(), &[])
            .expect("phase0 constructor handoff is supported");

        assert_eq!(actor.phase, 1);
        assert_eq!(effects.final_boss_active_counter, Some(1));
        assert!(effects.request_approach_step);
    }

    #[test]
    fn source_reentry_aliases_restore_action80_without_running_the_update() {
        for action in [81, 82, 83, 86, 87] {
            let mut actor = pressure_actor();
            actor.action = action;
            let before = actor;

            let effects = step(&mut actor, Frame::default(), &[])
                .expect("source action alias is a valid family callback");

            assert_eq!(actor.action, SOURCE_INITIAL_ACTION);
            assert_eq!(actor.phase, before.phase);
            assert_eq!(actor.timer, before.timer);
            assert_eq!(actor.rng, before.rng);
            assert_eq!(effects, Effects::default());
        }
    }

    #[test]
    fn phase15_death_reward_is_requested_only_after_terminal_delay() {
        let mut actor = pressure_actor();
        let actor_source_id = actor.source_id;
        actor.phase = 14;
        actor.timer = 1;
        actor.life = 8;
        let mut effects =
            step(&mut actor, Frame::default(), &[]).expect("phase14 death transition is supported");
        assert_eq!(actor.phase, FINAL_REWARD_PHASE);
        assert_eq!(actor.timer, 60);
        assert_eq!(actor.life, -1);
        assert_ne!(actor.flags & 1, 0);
        assert!(effects.kill_all_creatures);
        assert!(effects.refresh_creatures);
        assert_eq!(effects.final_boss_active_counter, None);
        assert!(effects.source_effects.is_empty());

        actor.timer = 1;
        let frame = Frame::default();
        effects = step(&mut actor, frame, &[]).expect("expired final reward phase is supported");
        let reward = effects
            .source_effects
            .first()
            .expect("expiry requests the source Volcano constructor");
        assert_eq!((reward.source_type, reward.source_subtype), (10, 9));
        assert_eq!(reward.owner_source_id, actor_source_id);
        assert_eq!(reward.life, Some(32));
        assert_eq!(reward.max_life, Some(11));
        assert!(effects.final_death_reward_pending);
        assert!(!effects.final_death_reward_attempted);
        assert_eq!(effects.final_boss_active_counter, Some(0));
        assert!(effects.hide);
        assert!(effects.final_death_reward_slot.is_none());

        let hidden = step(&mut actor, frame, &[])
            .expect("the hidden final boss does not re-enter its one-shot callback");
        assert!(hidden.source_effects.is_empty());
        assert!(!hidden.final_death_reward_pending);
        assert_eq!(hidden.final_boss_active_counter, None);
        assert!(!hidden.hide);

        let mut unexpired_actor = pressure_actor();
        unexpired_actor.phase = FINAL_REWARD_PHASE;
        unexpired_actor.timer = 2;
        unexpired_actor.life = -1;
        let before_expiry = step(&mut unexpired_actor, frame, &[])
            .expect("unexpired final reward phase is supported");
        assert!(before_expiry.source_effects.is_empty());
        assert!(!before_expiry.final_death_reward_pending);
        assert!(!before_expiry.hide);
    }
}
