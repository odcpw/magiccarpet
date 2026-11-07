//! Raw descriptor-campaign callbacks for Worm roots and shared Dragon/Worm limbs.
//!
//! This module deliberately mutates the imported [`EntitySnapshot`] records in
//! place.  It does not allocate a parallel creature, rebuild reset state, or
//! own the common `UpdateEntities_57730` epilogue.  The enclosing campaign
//! scheduler advances byte `0x3e`/`phase` and performs following-pass reclaim.
//!
//! Source anchors (pinned remc2 `6806c720`):
//! - `sub_1B6B0`, `EventsFunctions.cpp:8696-8732` (action 232);
//! - `sub_1D5D0 -> sub_1D700 -> sub_1D8C0`, lines 9977-10246;
//! - `sub_1EEE0`, lines 11172-11186; and
//! - `sub_1FA50`, lines 11619-11622 (action 31, base action 24).

use crate::{FlightWorld, Position, Tile};

use super::super::super::{
    EntitySnapshot, FireballProjectileOrigin,
    level2_dynamic_economy::Level2FireballProjectileLaunchPayload,
};

const MULTIPART_CLASS: u8 = 5;
const MULTIPART_MODEL: u8 = 3;
const MULTIPART_STAGE_ACTION: u8 = 31;
const MULTIPART_CHILD_ACTION: u8 = 232;
const MULTIPART_ATTACK_ACTION: u8 = 26;
const MULTIPART_PRE_KILL_ACTION: u8 = 28;
const MULTIPART_KILL_ACTION: u8 = 29;
const MULTIPART_IDLE_ACTION: u8 = 25;
const MULTIPART_LINKED_ACTION: u8 = 27;
const MULTIPART_DESCRIPTOR_RANGE: u32 = crate::level_definition::WORM_PROPERTIES.target_range;
// sub_58350 ignores the nominal word 34 and clamps to descriptor subtype 5.
const MULTIPART_DESCRIPTOR_TURN_LIMIT: i16 = crate::level_definition::WORM_PROPERTIES.turn_response;
const MULTIPART_DESCRIPTOR_SLOPE_LIMIT: i32 = crate::level_definition::WORM_PROPERTIES.maximum_slope;
const MULTIPART_DESCRIPTOR_MATERIAL_MASK: u32 = crate::level_definition::WORM_PROPERTIES.terrain_mask;
const MULTIPART_DESCRIPTOR_SETTLE_STEP: i16 = crate::level_definition::WORM_PROPERTIES.descent;
const MULTIPART_MOTION_COLLISION_BIT: u32 = 0x0004_0000;
// `str_D7BD6[74].byte_0x20 == 1`: fatal all-directions blockage is
// enabled, while the alternate base+6 hit action is not.
const MULTIPART_DESCRIPTOR_FLAGS: u8 = crate::level_definition::WORM_PROPERTIES.behavior_flags;
const STAGE_RELATION_SENTINEL: u16 = 0xae02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultipartStageTarget {
    /// The resolved source StageVars2 entry has no live target pointer.
    None,
    /// The caller resolved the StageVars2 pointer/word-0x4a selector to this
    /// raw entity slot.  Keeping resolution explicit prevents this callback
    /// from inventing a level-specific target lookup.
    Entity(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultipartCallbackError {
    SourceSlotOutOfBounds {
        source_slot: u16,
    },
    UnexpectedCarrier {
        source_slot: u16,
        class: u8,
        model: u8,
        action: u8,
    },
    UnexpectedStageMode {
        source_slot: u16,
        stage_mode: u8,
    },
    RelationOutOfBounds {
        source_slot: u16,
        field: &'static str,
        related_slot: u16,
    },
    ChildChainCycle {
        source_slot: u16,
    },
}

impl std::fmt::Display for MultipartCallbackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::SourceSlotOutOfBounds { source_slot } => {
                write!(
                    formatter,
                    "multipart source slot {source_slot} is out of bounds"
                )
            }
            Self::UnexpectedCarrier {
                source_slot,
                class,
                model,
                action,
            } => write!(
                formatter,
                "multipart source slot {source_slot} has unexpected carrier {class}/{model}/{action}"
            ),
            Self::UnexpectedStageMode {
                source_slot,
                stage_mode,
            } => write!(
                formatter,
                "multipart action-31 source slot {source_slot} has unsupported StageVar2 {stage_mode}"
            ),
            Self::RelationOutOfBounds {
                source_slot,
                field,
                related_slot,
            } => write!(
                formatter,
                "multipart source slot {source_slot} {field} relation {related_slot} is out of bounds"
            ),
            Self::ChildChainCycle { source_slot } => write!(
                formatter,
                "multipart source slot {source_slot} word-0x34 child chain cycles"
            ),
        }
    }
}

impl std::error::Error for MultipartCallbackError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultipartRootDisposition {
    Stage,
    Attack { target_slot: u16 },
    PreKill { killer_slot: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultipartMotionResult {
    FirstVisitFlag,
    SameTile,
    Direct,
    Redirected,
    Blocked,
    NotRun,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultipartRootExternalEffect {
    None,
    DecrementLiveEntityCounter,
}

#[must_use = "the campaign scheduler must apply external_effect and preserve the reported disposition"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultipartRootStep {
    pub source_slot: u16,
    pub disposition: MultipartRootDisposition,
    pub motion: MultipartMotionResult,
    pub rng_draws: u8,
    /// `sub_1D700` changed StageVar2 to 10 and selected base action + 2.
    pub completed_stage_target: bool,
    /// `sub_1B8C0` may mutate `D41A0.str_0x364D2`, which is deliberately not
    /// reconstructed in this raw callback. The enclosing campaign state must
    /// apply this effect before advancing the common phase.
    pub external_effect: MultipartRootExternalEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultipartChildStep {
    pub source_slot: u16,
    pub predecessor_slot: u16,
    pub followed_predecessor: bool,
    /// Source predictedAxis write, present only for the moving branch.
    pub predicted_position: Option<Position>,
    pub consumed_mailbox_owner: Option<u16>,
    pub drawing_disabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultipartAttackStep {
    pub source_slot: u16,
    pub launch: Option<Level2FireballProjectileLaunchPayload>,
    pub external_effect: MultipartRootExternalEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultipartPreKillStep {
    pub source_slot: u16,
    pub killer_slot: u16,
    pub child_count: u8,
}

#[derive(Clone, Copy)]
struct RawEntityView {
    slot: u16,
    source_id: u16,
    active: bool,
    class: u8,
    model: u8,
    action: u8,
    position: Position,
    yaw: i16,
    life: i32,
    word_26: u16,
    word_34: u16,
    word_96: u16,
    array_pitch: i16,
}

impl RawEntityView {
    fn read(entity: &EntitySnapshot) -> Self {
        Self {
            slot: entity.index,
            source_id: entity.source_id,
            active: entity.active,
            class: entity.class,
            model: entity.model,
            action: entity.action,
            position: entity.position,
            yaw: entity.yaw,
            life: entity.life,
            word_26: read_u16(entity, 42),
            word_34: read_u16(entity, 56),
            word_96: read_u16(entity, 154),
            array_pitch: read_i16(entity, 88),
        }
    }
}

#[derive(Clone, Copy)]
struct RootPrelude {
    life: i32,
    attacker: u16,
    mailbox_owner: u16,
    state: u8,
}

/// Execute the current descriptor-campaign action-31 callback directly on the
/// raw class-5/model-3 root.
///
/// `stage_target` is the already-resolved StageVars2 target.  The callback
/// consumes no allocator/RNG state except the source-local entity RNG draws
/// explicitly reported in [`MultipartRootStep`].  It neither advances the
/// common phase nor dispatches action 26/28 in the same pass.
pub(crate) fn step_raw_multipart_action31<W: FlightWorld>(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
    world: &W,
    stage_target: MultipartStageTarget,
) -> Result<MultipartRootStep, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(source_before, MULTIPART_STAGE_ACTION)?;
    let stage_mode = entities[source_index].record_bytes()[77];
    if stage_mode != 4 {
        return Err(MultipartCallbackError::UnexpectedStageMode {
            source_slot,
            stage_mode,
        });
    }

    // Precompute the source prelude without allocating or mutating so a bad
    // relation fails atomically instead of leaving a half-consumed mailbox.
    let mut prelude = RootPrelude {
        life: source_before.life,
        attacker: 0,
        mailbox_owner: read_u16(&entities[source_index], 102),
        state: 0,
    };
    if prelude.mailbox_owner != 0 {
        prelude.life = prelude
            .life
            .wrapping_sub(read_i32(&entities[source_index], 98));
        prelude.attacker = prelude.mailbox_owner;
        prelude.state = 1;
    }
    let mut child_slot = source_before.word_34;
    let mut visited = 0_usize;
    while child_slot != 0 {
        let child_index = checked_relation(entities, source_slot, "word-0x34", child_slot)?;
        let child = RawEntityView::read(&entities[child_index]);
        if child.life < prelude.life {
            prelude.life = child.life;
            prelude.attacker = child.word_26;
            prelude.state = 1;
            break;
        }
        child_slot = child.word_34;
        visited += 1;
        if visited > entities.len() {
            return Err(MultipartCallbackError::ChildChainCycle { source_slot });
        }
    }
    if prelude.life < 0 {
        prelude.state = 2;
    }

    if prelude.state == 1 {
        let _ = checked_relation(entities, source_slot, "word-0x26", prelude.attacker)?;
    }
    let stage_target_view = match stage_target {
        MultipartStageTarget::None => None,
        MultipartStageTarget::Entity(target_slot) => {
            let target_index =
                checked_relation(entities, source_slot, "StageVars2 target", target_slot)?;
            Some(RawEntityView::read(&entities[target_index]))
        }
    };
    if entities[source_index].phase & 7 == 0
        && let Some(target) = stage_target_view
        && target.word_96 != 0
        && target.word_96 != STAGE_RELATION_SENTINEL
    {
        let _ = checked_relation(
            entities,
            source_slot,
            "StageVars2 target word-0x96",
            target.word_96,
        )?;
    }

    {
        let source = &mut entities[source_index];
        source.set_life(prelude.life);
        write_u16(source, 42, prelude.attacker);
        if prelude.mailbox_owner != 0 {
            // sub_1D8C0 clears only word 0x62; the payload dword persists.
            source.clear_source_upkeep_owner();
        }
    }

    let mut motion = MultipartMotionResult::NotRun;
    let mut rng_draws = 0_u8;
    match prelude.state {
        2 => {
            let source = &mut entities[source_index];
            write_u16(source, 40, prelude.attacker);
            source.set_source_action(MULTIPART_PRE_KILL_ACTION);
        }
        1 => {
            let attacker = RawEntityView::read(
                &entities[checked_relation(entities, source_slot, "word-0x26", prelude.attacker)?],
            );
            let source = &mut entities[source_index];
            if attacker.class != source.class || attacker.model != source.model {
                write_u16(source, 154, prelude.attacker);
                source.record[77] = 10;
                source.set_source_action(if MULTIPART_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    MULTIPART_ATTACK_ACTION
                });
            }
            settle_to_terrain(source, world);
        }
        _ => {
            motion = step_root_motion(&mut entities[source_index], world);
            let root_after_motion = RawEntityView::read(&entities[source_index]);
            if entities[source_index].phase & 7 == 0 {
                match stage_target_view {
                    Some(target) if target.slot != 0 => {
                        if entities[source_index].control_flags & MULTIPART_MOTION_COLLISION_BIT
                            == 0
                        {
                            set_roll(
                                &mut entities[source_index],
                                heading(root_after_motion.position, target.position),
                            );
                            if entities[source_index].phase & 0x3f == 0 {
                                let first = next_rng(entities[source_index].entity_rng);
                                let second = next_rng(first);
                                entities[source_index].set_entity_rng(second);
                                rng_draws = 2;
                                let direction = 2 * (i32::from(first % 157) / 79) - 1;
                                let delta = direction * (i32::from(second & 0xff) + 85);
                                let roll = (i32::from(read_i16(&entities[source_index], 36))
                                    + delta)
                                    & 0x07ff;
                                set_roll(
                                    &mut entities[source_index],
                                    i16::try_from(roll).expect("11-bit roll"),
                                );
                            }
                        }

                        if let Some(peer) =
                            first_nearby_same_model(entities, source_slot, root_after_motion)
                        {
                            set_roll(
                                &mut entities[source_index],
                                heading(peer.position, root_after_motion.position),
                            );
                        }
                        let threshold = i32::from(root_after_motion.array_pitch)
                            + i32::from(target.array_pitch);
                        if wrapped_axis_distance(root_after_motion.position.x, target.position.x)
                            < threshold
                            && wrapped_axis_distance(
                                root_after_motion.position.y,
                                target.position.y,
                            ) < threshold
                        {
                            set_roll(
                                &mut entities[source_index],
                                heading(target.position, root_after_motion.position),
                            );
                        }
                    }
                    _ if entities[source_index].phase & 0x0f == 0
                        && entities[source_index].control_flags
                            & MULTIPART_MOTION_COLLISION_BIT
                            == 0 =>
                    {
                        let next = next_rng(entities[source_index].entity_rng);
                        entities[source_index].set_entity_rng(next);
                        rng_draws = 1;
                        let roll = (i32::from(read_i16(&entities[source_index], 36))
                            + i32::from(next % 113)
                            + 142)
                            & 0x07ff;
                        set_roll(
                            &mut entities[source_index],
                            i16::try_from(roll).expect("11-bit roll"),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // StageVar2==4 is sub_1D700: after sub_1D8C0 it performs one additional
    // eight-phase completion check through the resolved stage target.
    let mut completed_stage_target = false;
    if entities[source_index].phase & 7 == 0
        && let Some(target) = stage_target_view
        && target.slot != 0
        && target.word_96 != 0
        && target.word_96 != STAGE_RELATION_SENTINEL
    {
        let related_index = checked_relation(
            entities,
            source_slot,
            "StageVars2 target word-0x96",
            target.word_96,
        )?;
        if wrapped_distance_3d(
            entities[source_index].position,
            entities[related_index].position,
        ) <= MULTIPART_DESCRIPTOR_RANGE
        {
            let source = &mut entities[source_index];
            write_u16(source, 154, target.word_96);
            source.record[77] = 10;
            source.set_source_action(MULTIPART_ATTACK_ACTION);
            completed_stage_target = true;
        }
    }

    let source = &entities[source_index];
    let external_effect = if motion == MultipartMotionResult::Blocked && source.life == -1 {
        MultipartRootExternalEffect::DecrementLiveEntityCounter
    } else {
        MultipartRootExternalEffect::None
    };
    let disposition = match source.action {
        MULTIPART_PRE_KILL_ACTION => MultipartRootDisposition::PreKill {
            killer_slot: read_u16(source, 40),
        },
        MULTIPART_ATTACK_ACTION => MultipartRootDisposition::Attack {
            target_slot: read_u16(source, 154),
        },
        _ => MultipartRootDisposition::Stage,
    };
    Ok(MultipartRootStep {
        source_slot,
        disposition,
        motion,
        rng_draws,
        completed_stage_target,
        external_effect,
    })
}

/// Alliance's action31 follows the fixed parent record through sub_1E9C0.
/// Return ordered spell feedback for the scheduler's existing reward owner.
pub(crate) fn step_raw_alliance_stage<W: FlightWorld>(
    entities: &mut [EntitySnapshot], source_slot: u16, world: &W,
) -> Result<(MultipartRootExternalEffect, Vec<(u16, u8)>), MultipartCallbackError> {
    step_raw_alliance_stage_inner(entities, source_slot, world, None)
}

fn step_raw_alliance_stage_inner<W: FlightWorld>(
    entities: &mut [EntitySnapshot], source_slot: u16, world: &W, peers: Option<&[u16]>,
) -> Result<(MultipartRootExternalEffect, Vec<(u16, u8)>), MultipartCallbackError> {
    let index = checked_slot(entities, source_slot)?;
    validate_carrier(RawEntityView::read(&entities[index]), MULTIPART_STAGE_ACTION)?;
    let remaining = (read_u16(&entities[index], 50) as i16).wrapping_sub(1);
    write_u16(&mut entities[index], 50, remaining as u16);
    let parent_slot = read_u16(&entities[index], 44);
    let parent = if parent_slot != 0 && remaining > 0 {
        Some(checked_relation(entities, source_slot, "Alliance parent", parent_slot)?)
    } else { None };
    let alive = parent.is_some_and(|parent| entities[parent].life > 0);
    if remaining <= 0 || !alive {
        raw_alliance_blink(entities, source_slot, true)?;
        let source = &mut entities[index];
        source.record[77] = 10;
        source.record[76] = read_u16(source, 78) as u8;
        write_u16(source, 44, 0); write_u16(source, 154, 0);
        return Ok((MultipartRootExternalEffect::None, Vec::new()));
    }
    raw_alliance_blink(entities, source_slot, false)?;
    let previous = read_u16(&entities[index], 154);
    write_u16(&mut entities[index], 154, parent_slot);
    let prelude = read_root_prelude(entities, source_slot)?;
    apply_root_prelude(&mut entities[index], prelude);
    let mut feedback = Vec::new();
    let mut external = MultipartRootExternalEffect::None;
    if prelude.state == 2 {
        write_u16(&mut entities[index], 40, prelude.attacker);
        write_u16(&mut entities[index], 50, 1);
    } else {
        let motion = step_root_motion(&mut entities[index], world);
        if motion == MultipartMotionResult::Blocked && entities[index].life == -1 {
            external = MultipartRootExternalEffect::DecrementLiveEntityCounter;
        }
        if prelude.state == 1 {
            let attacker = &entities[checked_relation(entities, source_slot, "Alliance attacker", prelude.attacker)?];
            if prelude.attacker != parent_slot && (attacker.class, attacker.model) != (MULTIPART_CLASS, MULTIPART_MODEL) {
                write_u16(&mut entities[index], 154, prelude.attacker);
                entities[index].set_source_action(MULTIPART_ATTACK_ACTION);
                feedback.push((parent_slot, 19));
            }
        } else if entities[index].phase & 7 == 0 {
            let parent_position = entities[parent.unwrap()].position;
            if entities[index].control_flags & MULTIPART_MOTION_COLLISION_BIT == 0 {
                let position = entities[index].position;
                set_roll(&mut entities[index], heading(position, parent_position));
                if entities[index].phase & 63 == 0 {
                    let first = next_rng(entities[index].entity_rng); let second = next_rng(first);
                    entities[index].set_entity_rng(second);
                    let delta = (2 * (i32::from(first % 157) / 79) - 1) * (i32::from(second & 255) + 85);
                    let roll = (i32::from(entities[index].source_roll()) + delta) & 2047;
                    set_roll(&mut entities[index], roll as i16);
                }
            }
            let source = RawEntityView::read(&entities[index]);
            if let Some(peer) = first_nearby_same_model_in(entities, source_slot, source, peers) {
                set_roll(&mut entities[index], heading(peer.position, source.position));
            }
        }
    }
    let parent = parent.unwrap();
    let inherited = match read_u16(&entities[parent], 154) { 0 => read_u16(&entities[parent], 42), selected => selected };
    let selected = if inherited == 0 { previous } else { inherited };
    write_u16(&mut entities[index], 154, selected);
    if selected != 0 {
        let target = checked_relation(entities, source_slot, "Alliance target", selected)?;
        if (read_u16(&entities[target], 44) == parent_slot && entities[target].record[77] == 14)
            || entities[target].life <= 0 || entities[target].drawing_disabled() {
            write_u16(&mut entities[index], 154, 0);
        } else if wrapped_distance_3d(entities[index].position, entities[target].position) < MULTIPART_DESCRIPTOR_RANGE {
            entities[index].set_source_action(MULTIPART_ATTACK_ACTION);
            feedback.push((parent_slot, 24));
        }
    }
    Ok((external, feedback))
}

fn raw_alliance_blink(entities: &mut [EntitySnapshot], source_slot: u16, clear: bool) -> Result<(), MultipartCallbackError> {
    let index = checked_slot(entities, source_slot)?;
    let bits = crate::native_world::level2_class5::firefly::alliance_blink_bits(
        read_u16(&entities[index], 50), read_u16(&entities[index], 52), entities[index].phase, clear,
    );
    let mut child = read_u16(&entities[index], 56);
    let flags = entities[index].control_flags;
    entities[index].set_control_flags((flags & !0x0600_0000) | bits);
    let mut remaining = entities.len();
    while child != 0 {
        if remaining == 0 { return Err(MultipartCallbackError::ChildChainCycle { source_slot }); }
        remaining -= 1;
        let child_index = checked_relation(entities, source_slot, "Alliance child", child)?;
        child = read_u16(&entities[child_index], 56);
        let flags = entities[child_index].control_flags;
        entities[child_index].set_control_flags((flags & !0x0600_0000) | bits);
    }
    Ok(())
}

/// Resolve only the direct Alliance pointer graph and the frozen model list
/// into callback-local records. Their owning carriers remain authoritative.
fn projected_alliance_rows(
    arena: &crate::campaign::source_arena::SourceArena,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<Vec<EntitySnapshot>, String> {
    let source = arena.entities.get(usize::from(slot)).ok_or("Worm source address is absent")?;
    let mut pointers = crate::native_world::source_target::alliance_direct_references(arena, source)?;
    pointers.extend(peers.iter().copied());
    let mut rows = arena.entities.to_vec();
    for pointer in pointers {
        if pointer == slot { continue; }
        let Some(view) = crate::native_world::source_target::read(arena, context, pointer)? else { continue };
        let row = &mut rows[usize::from(pointer)];
        row.active = view.active; row.source_id = view.source_id;
        row.class = view.class; row.model = view.model;
        row.set_position(view.position); row.set_life(view.life); row.set_control_flags(view.control_flags);
        write_u16(row, 42, view.relations.attacker.unwrap_or(0));
        write_u16(row, 44, view.relations.parent.unwrap_or(0));
        write_u16(row, 56, view.relations.next.unwrap_or(0));
        write_u16(row, 154, view.relations.target.unwrap_or(0));
        row.record[77] = view.relations.stage_mode.unwrap_or(0);
    }
    Ok(rows)
}

pub(crate) fn step_imported_alliance_stage(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &super::super::super::SessionTerrain,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(MultipartRootExternalEffect, Vec<(u16, u8)>), String> {
    let mut rows = projected_alliance_rows(arena, slot, peers, context)?;
    let outcome = step_raw_alliance_stage_inner(&mut rows, slot, terrain, Some(peers)).map_err(|error| error.to_string())?;
    let root = &rows[usize::from(slot)];
    let flags = root.control_flags & 0x0600_0000;
    let child = read_u16(root, 56);
    crate::native_world::level2_class5::firefly::alliance_blink_chain(arena, child, flags, context)?;
    arena.entities[usize::from(slot)] = root.clone();
    Ok(outcome)
}

pub(crate) fn step_imported_alliance_attack(
    arena: &mut crate::campaign::source_arena::SourceArena,
    terrain: &super::super::super::SessionTerrain,
    slot: u16,
    peers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<MultipartAttackStep, String> {
    let mut rows = projected_alliance_rows(arena, slot, peers, context)?;
    let prelude = read_root_prelude(&rows, slot).map_err(|error| error.to_string())?;
    let outcome = step_raw_multipart_action26_inner(&mut rows, slot, terrain, Some(peers)).map_err(|error| error.to_string())?;
    let root = &rows[usize::from(slot)];
    if prelude.state == 0 {
        let flags = root.control_flags & 0x0600_0000;
        let child = read_u16(root, 56);
        crate::native_world::level2_class5::firefly::alliance_blink_chain(arena, child, flags, context)?;
    }
    arena.entities[usize::from(slot)] = root.clone();
    Ok(outcome)
}

/// Execute the package-reachable action-26 attack callback. Allocation,
/// target feedback and sound remain outer-session effects so a failed
/// `NewEvent` cannot be mistaken for a successful Fireball launch.
pub(crate) fn step_raw_multipart_action26<W: FlightWorld>(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
    world: &W,
) -> Result<MultipartAttackStep, MultipartCallbackError> {
    step_raw_multipart_action26_inner(entities, source_slot, world, None)
}

fn step_raw_multipart_action26_inner<W: FlightWorld>(
    entities: &mut [EntitySnapshot], source_slot: u16, world: &W, peers: Option<&[u16]>,
) -> Result<MultipartAttackStep, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(source_before, MULTIPART_ATTACK_ACTION)?;

    let prelude = read_root_prelude(entities, source_slot)?;
    apply_root_prelude(&mut entities[source_index], prelude);
    if prelude.life < 0 {
        write_u16(&mut entities[source_index], 40, prelude.attacker);
        entities[source_index].set_source_action(MULTIPART_PRE_KILL_ACTION);
        return Ok(MultipartAttackStep {
            source_slot,
            launch: None,
            external_effect: MultipartRootExternalEffect::None,
        });
    }
    if prelude.state == 1 {
        write_u16(&mut entities[source_index], 154, prelude.attacker);
        settle_to_terrain(&mut entities[source_index], world);
        return Ok(MultipartAttackStep {
            source_slot,
            launch: None,
            external_effect: MultipartRootExternalEffect::None,
        });
    }

    let motion = step_root_motion(&mut entities[source_index], world);
    let external_effect =
        if motion == MultipartMotionResult::Blocked && entities[source_index].life < 0 {
            MultipartRootExternalEffect::DecrementLiveEntityCounter
        } else {
            MultipartRootExternalEffect::None
        };
    let target_slot = read_u16(&entities[source_index], 154);
    let allied = entities[source_index].record[77] == 14;
    if allied {
        raw_alliance_blink(entities, source_slot, false)?;
        let remaining = (read_u16(&entities[source_index], 50) as i16).wrapping_sub(1);
        write_u16(&mut entities[source_index], 50, remaining as u16);
        let parent = read_u16(&entities[source_index], 44);
        let selected = if parent == 0 { 0 } else {
            let index = checked_relation(entities, source_slot, "Alliance parent", parent)?;
            match read_u16(&entities[index], 154) { 0 => read_u16(&entities[index], 42), selected => selected }
        };
        if remaining <= 0 || (parent != 0 && (target_slot == parent || (selected != 0 && selected != target_slot))) {
            entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
            return Ok(MultipartAttackStep { source_slot, launch: None, external_effect });
        }
    }
    let target = if target_slot == 0 {
        None
    } else {
        let target_index = checked_relation(entities, source_slot, "word-0x96", target_slot)?;
        Some(RawEntityView::read(&entities[target_index]))
    };
    let Some(target) = target.filter(|target| {
        (allied || target.active)
            && target.life >= 0
            && !entities[usize::from(target.slot)].drawing_disabled()
            && (allied || wrapped_distance_3d(entities[source_index].position, target.position)
                < MULTIPART_DESCRIPTOR_RANGE)
    }) else {
        entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
        return Ok(MultipartAttackStep {
            source_slot,
            launch: None,
            external_effect,
        });
    };

    if entities[source_index].phase & 3 == 0 {
        let source_position = entities[source_index].position;
        set_roll(
            &mut entities[source_index],
            heading(source_position, target.position),
        );
        if let Some(peer) = first_nearby_same_model_in(
            entities,
            source_slot,
            RawEntityView::read(&entities[source_index]),
            peers,
        ) {
            set_roll(
                &mut entities[source_index],
                heading(peer.position, source_position),
            );
        }
    }

    let launch = if entities[source_index].phase % crate::level_definition::WORM_PROPERTIES.scan_period == 0 {
        if allied && wrapped_distance_3d(entities[source_index].position, target.position) >= MULTIPART_DESCRIPTOR_RANGE {
            entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
            return Ok(MultipartAttackStep { source_slot, launch: None, external_effect });
        }
        let position = entities[source_index].position;
        let (yaw, pitch) =
            crate::level1_mission::projectile_guidance_angles(position, target.position);
        let mut launch_position = position;
        launch_position.z = launch_position.z.wrapping_add(entities[source_index].fov);
        Some(Level2FireballProjectileLaunchPayload {
            origin: FireballProjectileOrigin::Worm,
            position: launch_position,
            yaw,
            pitch,
            axis_position: target.position,
            source_id_entity_index: entities[source_index].source_id,
            holder_entity_index: 0,
            source_subspell_index: 500,
            source_mana: 0,
            source_dword_0x10: 0,
            target_source_entity_index: target.slot,
            target_class: target.class,
            target_model: target.model,
        })
    } else {
        None
    };
    Ok(MultipartAttackStep {
        source_slot,
        launch,
        external_effect,
    })
}

/// Execute package-reachable free-roam action 25. The pass-start imported
/// entity slice is already in numeric source order, so the first eligible
/// player/peer match preserves remc2 tie ordering.
pub(crate) fn step_raw_multipart_action25<W: FlightWorld>(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
    world: &W,
) -> Result<MultipartRootExternalEffect, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(source_before, MULTIPART_IDLE_ACTION)?;
    let prelude = read_root_prelude(entities, source_slot)?;
    apply_root_prelude(&mut entities[source_index], prelude);
    if prelude.life < 0 {
        write_u16(&mut entities[source_index], 40, prelude.attacker);
        entities[source_index].set_source_action(MULTIPART_PRE_KILL_ACTION);
        return Ok(MultipartRootExternalEffect::None);
    }
    if prelude.state == 1 {
        write_u16(&mut entities[source_index], 154, prelude.attacker);
        entities[source_index].set_source_action(MULTIPART_ATTACK_ACTION);
        settle_to_terrain(&mut entities[source_index], world);
        return Ok(MultipartRootExternalEffect::None);
    }
    let motion = step_root_motion(&mut entities[source_index], world);
    let external = if motion == MultipartMotionResult::Blocked && entities[source_index].life < 0 {
        MultipartRootExternalEffect::DecrementLiveEntityCounter
    } else {
        MultipartRootExternalEffect::None
    };
    if entities[source_index].phase % crate::level_definition::WORM_PROPERTIES.scan_period != 0 {
        return Ok(external);
    }
    let first = next_rng(entities[source_index].entity_rng);
    let second = next_rng(first);
    entities[source_index].set_entity_rng(second);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    let delta = direction * (i32::from(second & 0xff) + 85);
    let roll = (i32::from(entities[source_index].source_roll()) + delta) & 0x07ff;
    set_roll(
        &mut entities[source_index],
        i16::try_from(roll).expect("11-bit Worm roll"),
    );
    if entities[source_index].record_bytes()[61] == 0 {
        return Ok(external);
    }
    let source = RawEntityView::read(&entities[source_index]);
    let target = entities
        .iter()
        .map(RawEntityView::read)
        .filter(|candidate| {
            candidate.slot != 0
                && candidate.active
                && candidate.class == 3
                && entities[usize::from(candidate.slot)].control_flags & 0x20 == 0
                && wrapped_distance_2d(source.position, candidate.position)
                    <= MULTIPART_DESCRIPTOR_RANGE
                && crate::angle_distance(
                    i32::from(source.yaw),
                    i32::from(heading(source.position, candidate.position)),
                ) < crate::level_definition::WORM_PROPERTIES.target_half_angle
        })
        .min_by_key(|candidate| wrapped_distance_2d(source.position, candidate.position));
    if let Some(target) = target {
        write_u16(&mut entities[source_index], 154, target.slot);
        entities[source_index].set_source_action(MULTIPART_ATTACK_ACTION);
        return Ok(external);
    }
    if let Some(peer) = entities
        .iter()
        .map(RawEntityView::read)
        .filter(|candidate| {
            candidate.slot != source_slot
                && candidate.active
                && candidate.class == MULTIPART_CLASS
                && candidate.model == MULTIPART_MODEL
                && !matches!(candidate.action, MULTIPART_CHILD_ACTION | 180 | 234)
                && read_u16(&entities[usize::from(candidate.slot)], 54) == 0
                && wrapped_distance_2d(source.position, candidate.position)
                    <= MULTIPART_DESCRIPTOR_RANGE
                && crate::angle_distance(
                    i32::from(source.yaw),
                    i32::from(heading(candidate.position, source.position)),
                ) < crate::level_definition::WORM_PROPERTIES.target_half_angle
        })
        .min_by_key(|candidate| wrapped_distance_2d(source.position, candidate.position))
    {
        write_u16(&mut entities[source_index], 54, peer.slot);
        entities[source_index].set_source_action(MULTIPART_LINKED_ACTION);
    }
    Ok(external)
}

/// Execute package-reachable peer-follow action 27.
pub(crate) fn step_raw_multipart_action27<W: FlightWorld>(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
    world: &W,
) -> Result<MultipartRootExternalEffect, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(source_before, MULTIPART_LINKED_ACTION)?;
    let peer_slot = read_u16(&entities[source_index], 54);
    if peer_slot == 0 {
        entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
        return Ok(MultipartRootExternalEffect::None);
    }
    let peer_index = checked_relation(entities, source_slot, "word-0x32 peer", peer_slot)?;
    let peer = RawEntityView::read(&entities[peer_index]);
    let peer_valid = peer.active
        && peer.life >= 0
        && peer.class == MULTIPART_CLASS
        && peer.model == MULTIPART_MODEL
        && !entities[peer_index].drawing_disabled();
    let prelude = read_root_prelude(entities, source_slot)?;
    apply_root_prelude(&mut entities[source_index], prelude);
    if prelude.life < 0 {
        if peer_valid {
            write_u16(&mut entities[peer_index], 154, prelude.attacker);
            write_u16(&mut entities[peer_index], 54, 0);
            entities[peer_index].set_source_action(MULTIPART_ATTACK_ACTION);
        }
        write_u16(&mut entities[source_index], 54, 0);
        write_u16(&mut entities[source_index], 40, prelude.attacker);
        entities[source_index].set_source_action(MULTIPART_PRE_KILL_ACTION);
        return Ok(MultipartRootExternalEffect::None);
    }
    if prelude.state == 1 {
        if peer_valid {
            write_u16(&mut entities[peer_index], 154, prelude.attacker);
            write_u16(&mut entities[peer_index], 54, 0);
            entities[peer_index].set_source_action(MULTIPART_ATTACK_ACTION);
        }
        write_u16(&mut entities[source_index], 54, 0);
        write_u16(&mut entities[source_index], 154, prelude.attacker);
        entities[source_index].set_source_action(MULTIPART_ATTACK_ACTION);
        settle_to_terrain(&mut entities[source_index], world);
        return Ok(MultipartRootExternalEffect::None);
    }
    let motion = step_root_motion(&mut entities[source_index], world);
    let external = if motion == MultipartMotionResult::Blocked && entities[source_index].life < 0 {
        MultipartRootExternalEffect::DecrementLiveEntityCounter
    } else {
        MultipartRootExternalEffect::None
    };
    if !peer_valid {
        write_u16(&mut entities[source_index], 54, 0);
        entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
        return Ok(external);
    }
    if entities[source_index].phase % crate::level_definition::WORM_PROPERTIES.scan_period != 0 {
        return Ok(external);
    }
    let mut followed = None;
    match peer.action.wrapping_sub(24) {
        0 | 1 => followed = Some(peer),
        2 => {
            write_u16(&mut entities[source_index], 154, peer.word_96);
            write_u16(&mut entities[source_index], 54, 0);
            entities[source_index].set_source_action(MULTIPART_ATTACK_ACTION);
        }
        3 => {
            let peer_relation = read_u16(&entities[peer_index], 54);
            write_u16(&mut entities[source_index], 54, peer_relation);
            if peer_relation == 0 {
                followed = Some(RawEntityView::read(&entities[0]));
            } else {
                let relation_index = checked_relation(
                    entities,
                    source_slot,
                    "chained word-0x32 peer",
                    peer_relation,
                )?;
                followed = Some(RawEntityView::read(&entities[relation_index]));
            }
        }
        _ => {
            write_u16(&mut entities[source_index], 54, 0);
            entities[source_index].set_source_action(MULTIPART_IDLE_ACTION);
        }
    }
    if let Some(followed) = followed {
        let source_position = entities[source_index].position;
        set_roll(
            &mut entities[source_index],
            heading(source_position, followed.position),
        );
        if let Some(nearby) = first_nearby_same_model(
            entities,
            source_slot,
            RawEntityView::read(&entities[source_index]),
        ) {
            set_roll(
                &mut entities[source_index],
                heading(nearby.position, source_position),
            );
        }
        let peer_maximum_speed = read_i16(&entities[peer_index], 138);
        entities[source_index]
            .set_actual_speed(peer_maximum_speed.wrapping_add(entities[peer_index].actual_speed));
    }
    Ok(external)
}

/// Execute `PreKillEntity_1C890` for one imported Worm root. The root and all
/// sixteen body rows retain their fixed source slots and enter action 29; the
/// scheduler owns their independent later visits and epilogues.
pub(crate) fn step_raw_multipart_action28(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
) -> Result<MultipartPreKillStep, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(source_before, MULTIPART_PRE_KILL_ACTION)?;
    let mut killer_slot = read_u16(&entities[source_index], 40);
    let mut child_slot = source_before.word_34;
    let mut child_count = 0_u8;
    while child_slot != 0 {
        let child_index = checked_relation(entities, source_slot, "word-0x34", child_slot)?;
        if entities[child_index].class != MULTIPART_CLASS
            || entities[child_index].model != MULTIPART_MODEL
        {
            return Err(MultipartCallbackError::UnexpectedCarrier {
                source_slot: child_slot,
                class: entities[child_index].class,
                model: entities[child_index].model,
                action: entities[child_index].action,
            });
        }
        let next = read_u16(&entities[child_index], 56);
        let child_killer = read_u16(&entities[child_index], 40);
        if child_killer != 0 {
            killer_slot = child_killer;
        }
        entities[child_index].set_source_action(MULTIPART_KILL_ACTION);
        child_count = child_count.saturating_add(1);
        if usize::from(child_count) > entities.len() {
            return Err(MultipartCallbackError::ChildChainCycle { source_slot });
        }
        child_slot = next;
    }
    write_u16(&mut entities[source_index], 40, killer_slot);
    entities[source_index].set_source_action(MULTIPART_KILL_ACTION);
    Ok(MultipartPreKillStep {
        source_slot,
        killer_slot,
        child_count,
    })
}

fn read_root_prelude(
    entities: &[EntitySnapshot],
    source_slot: u16,
) -> Result<RootPrelude, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let source = RawEntityView::read(&entities[source_index]);
    let mailbox_owner = read_u16(&entities[source_index], 102);
    let mut prelude = RootPrelude {
        life: source.life,
        attacker: 0,
        mailbox_owner,
        state: 0,
    };
    if mailbox_owner != 0 {
        prelude.life = prelude
            .life
            .wrapping_sub(read_i32(&entities[source_index], 98));
        prelude.attacker = mailbox_owner;
        prelude.state = 1;
    }
    let mut child_slot = source.word_34;
    let mut visited = 0_usize;
    while child_slot != 0 {
        let child_index = checked_relation(entities, source_slot, "word-0x34", child_slot)?;
        let child = RawEntityView::read(&entities[child_index]);
        if child.life < prelude.life {
            prelude.life = child.life;
            prelude.attacker = child.word_26;
            prelude.state = 1;
            break;
        }
        child_slot = child.word_34;
        visited += 1;
        if visited > entities.len() {
            return Err(MultipartCallbackError::ChildChainCycle { source_slot });
        }
    }
    if prelude.life < 0 {
        prelude.state = 2;
    }
    if prelude.attacker != 0 {
        let _ = checked_relation(entities, source_slot, "word-0x26", prelude.attacker)?;
    }
    Ok(prelude)
}

fn apply_root_prelude(source: &mut EntitySnapshot, prelude: RootPrelude) {
    source.set_life(prelude.life);
    write_u16(source, 42, prelude.attacker);
    if prelude.mailbox_owner != 0 {
        source.clear_source_upkeep_owner();
    }
}

/// Execute `sub_1B6B0` for one raw action-232 child.
///
/// Common phase advancement and following-pass reclamation remain outside
/// this callback.  The source routine performs no RNG draw, allocation, or
/// audio call, and neither does this port.
pub(crate) fn step_raw_multipart_action232(
    entities: &mut [EntitySnapshot],
    source_slot: u16,
) -> Result<MultipartChildStep, MultipartCallbackError> {
    let source_index = checked_slot(entities, source_slot)?;
    let child_before = RawEntityView::read(&entities[source_index]);
    validate_carrier(child_before, MULTIPART_CHILD_ACTION)?;
    let predecessor_slot = read_u16(&entities[source_index], 54);
    let predecessor_index = checked_relation(
        entities,
        source_slot,
        "word-0x32 predecessor",
        predecessor_slot,
    )?;
    let predecessor = RawEntityView::read(&entities[predecessor_index]);
    let byte_39 = entities[source_index].record_bytes()[61];
    let phase = entities[source_index].phase;
    let word_36 = read_u16(&entities[source_index], 58);
    let mailbox_owner = read_u16(&entities[source_index], 102);
    let mailbox_payload = read_i32(&entities[source_index], 98);

    let drawing_disabled = predecessor.class != MULTIPART_CLASS;
    let mut followed_predecessor = false;
    let mut predicted_position = None;
    let mut consumed_mailbox_owner = None;
    let child = &mut entities[source_index];
    if drawing_disabled {
        child.disable_drawing();
    }
    if byte_39 != 0 {
        let yaw = heading(child_before.position, predecessor.position);
        let horizontal = wrapped_distance_2d(child_before.position, predecessor.position);
        let pitch = crate::pitch_for_delta(
            i32::try_from(horizontal).expect("wrapped XY distance fits i32"),
            i32::from(predecessor.position.z.wrapping_sub(child_before.position.z)),
        );
        let mut predicted = predecessor.position;
        crate::move_entity(
            &mut predicted,
            yaw,
            pitch,
            i16::from_ne_bytes(word_36.to_ne_bytes()).wrapping_neg(),
        );
        predicted_position = Some(predicted);
        child.set_yaw(yaw);
        child.set_pitch(pitch);
        child.set_position(predicted);
        followed_predecessor = true;
        if mailbox_owner != 0 {
            child.set_life(child.life.wrapping_sub(mailbox_payload));
            child.clear_source_upkeep_owner();
            write_u16(child, 42, mailbox_owner);
            consumed_mailbox_owner = Some(mailbox_owner);
        } else {
            write_u16(child, 42, 0);
        }
    } else if phase & 3 == 0 {
        child.set_position(predecessor.position);
        child.set_yaw(predecessor.yaw);
        followed_predecessor = true;
    }

    Ok(MultipartChildStep {
        source_slot,
        predecessor_slot,
        followed_predecessor,
        predicted_position,
        consumed_mailbox_owner,
        drawing_disabled,
    })
}

fn validate_carrier(
    entity: RawEntityView,
    expected_action: u8,
) -> Result<(), MultipartCallbackError> {
    // Source action232 calls sub_1B6B0 for both Dragon and Worm segments.
    // The root callbacks below remain specific to the Worm's model3.
    let accepted_model = entity.model == MULTIPART_MODEL
        || (expected_action == MULTIPART_CHILD_ACTION && entity.model == 0);
    if entity.slot == 0
        || !entity.active
        || entity.class != MULTIPART_CLASS
        || !accepted_model
        || entity.action != expected_action
    {
        return Err(MultipartCallbackError::UnexpectedCarrier {
            source_slot: entity.slot,
            class: entity.class,
            model: entity.model,
            action: entity.action,
        });
    }
    Ok(())
}

fn checked_slot(
    entities: &[EntitySnapshot],
    source_slot: u16,
) -> Result<usize, MultipartCallbackError> {
    let index = usize::from(source_slot);
    if index >= entities.len() || entities[index].index != source_slot {
        return Err(MultipartCallbackError::SourceSlotOutOfBounds { source_slot });
    }
    Ok(index)
}

fn checked_relation(
    entities: &[EntitySnapshot],
    source_slot: u16,
    field: &'static str,
    related_slot: u16,
) -> Result<usize, MultipartCallbackError> {
    let index = usize::from(related_slot);
    if index >= entities.len() || entities[index].index != related_slot {
        return Err(MultipartCallbackError::RelationOutOfBounds {
            source_slot,
            field,
            related_slot,
        });
    }
    Ok(index)
}

fn first_nearby_same_model(
    entities: &[EntitySnapshot],
    source_slot: u16,
    source: RawEntityView,
) -> Option<RawEntityView> {
    first_nearby_same_model_in(entities, source_slot, source, None)
}

fn first_nearby_same_model_in(
    entities: &[EntitySnapshot], source_slot: u16, source: RawEntityView, peers: Option<&[u16]>,
) -> Option<RawEntityView> {
    entities.iter().find_map(|candidate| {
        let candidate = RawEntityView::read(candidate);
        (candidate.slot != source_slot
            && if let Some(peers) = peers { peers.contains(&candidate.slot) } else {
                candidate.active && candidate.life >= 0 && candidate.class == MULTIPART_CLASS
                    && candidate.model == source.model
            // `UpdateEntities_57730` omits these teardown/presentation
            // actions from `bytearray_38403x[model]`; notably this keeps the
            // action-232 multipart children out of the peer scan.
                    && !matches!(candidate.action, 0xb4 | 0xe8 | 0xea)
            }
            && candidate.source_id != source.source_id
            && wrapped_axis_distance(source.position.x, candidate.position.x)
                < i32::from(source.array_pitch)
            && wrapped_axis_distance(source.position.y, candidate.position.y)
                < i32::from(source.array_pitch))
        .then_some(candidate)
    })
}

fn step_root_motion<W: FlightWorld>(
    source: &mut EntitySnapshot,
    world: &W,
) -> MultipartMotionResult {
    if source.record[17] & 8 != 0 {
        source.set_control_flags(source.control_flags & !0x0000_0800);
        return MultipartMotionResult::FirstVisitFlag;
    }

    let before = source.position;
    let mut predicted = before;
    settle_position(&mut predicted, world);
    crate::move_entity(&mut predicted, source.yaw, 0, source.actual_speed);
    world.set_source_predicted_position(predicted);
    let crossed_tile = before.x >> 8 != predicted.x >> 8 || before.y >> 8 != predicted.y >> 8;
    if !crossed_tile {
        source.set_position(predicted);
        source.set_control_flags(source.control_flags & !MULTIPART_MOTION_COLLISION_BIT);
        turn_toward_roll(source);
        return MultipartMotionResult::SameTile;
    }
    if !candidate_blocked(source, predicted, source.yaw, world) {
        source.set_position(predicted);
        source.set_control_flags(source.control_flags & !MULTIPART_MOTION_COLLISION_BIT);
        turn_toward_roll(source);
        return MultipartMotionResult::Direct;
    }

    source.set_control_flags(source.control_flags | MULTIPART_MOTION_COLLISION_BIT);
    let original_yaw = source.yaw;
    for heading in retry_headings(original_yaw) {
        source.set_yaw(heading);
        let mut candidate = before;
        settle_position(&mut candidate, world);
        crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
        world.set_source_predicted_position(candidate);
        if !candidate_blocked(source, candidate, heading, world) {
            source.set_position(candidate);
            turn_toward_roll(source);
            return MultipartMotionResult::Redirected;
        }
    }
    if MULTIPART_DESCRIPTOR_FLAGS & 1 != 0 {
        source.set_life(-1);
    }
    MultipartMotionResult::Blocked
}

/// The source Stage31 helpers share the same descriptor74 movement.
/// The boolean reports the source all-directions-blocked population effect.
pub(super) fn move_stage_actor<W: FlightWorld>(source: &mut EntitySnapshot, world: &W) -> bool {
    step_root_motion(source, world) == MultipartMotionResult::Blocked
}

pub(super) fn settle_stage_actor<W: FlightWorld>(source: &mut EntitySnapshot, world: &W) {
    settle_to_terrain(source, world);
}

fn settle_to_terrain<W: FlightWorld>(source: &mut EntitySnapshot, world: &W) {
    let mut position = source.position;
    settle_position(&mut position, world);
    source.set_position(position);
}

fn settle_position<W: FlightWorld>(position: &mut Position, world: &W) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(MULTIPART_DESCRIPTOR_SETTLE_STEP);
    }
    if position.z <= ground {
        position.z = ground;
    }
}

fn candidate_blocked<W: FlightWorld>(
    source: &EntitySnapshot,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    probe_blocked(source, candidate, heading, world)
        || slope_measure(candidate, world) >= MULTIPART_DESCRIPTOR_SLOPE_LIMIT
}

fn probe_blocked<W: FlightWorld>(
    source: &EntitySnapshot,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    let maximum_extent = i32::from(read_i16(source, 88).max(read_i16(source, 90))).max(0);
    let mut probe = candidate;
    let mut distance = 0_i32;
    while distance <= maximum_extent {
        if world.material_mask(probe) & !MULTIPART_DESCRIPTOR_MATERIAL_MASK != 0
            || world.cave_collision_blocked(probe)
        {
            return true;
        }
        if let Some(ceiling) = world.ceiling_height(probe) {
            let tile = Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            };
            if world.angle_flags(tile) & 8 != 0
                || i32::from(source.fov) + i32::from(world.ground_height(probe))
                    > i32::from(ceiling)
            {
                return true;
            }
        }
        distance += 256;
        crate::move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let tile_x = position.x.to_be_bytes()[0];
    let tile_y = position.y.to_be_bytes()[0];
    let height = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    (point_1 + point_4 - point_2 - point_3)
        .abs()
        .max((point_1 + point_2 - point_4 - point_3).abs())
}

fn retry_headings(yaw: i16) -> [i16; 3] {
    let original = u16::from_ne_bytes(yaw.to_ne_bytes());
    let second = original.wrapping_add(341) & 0x07ff;
    let third = (original.wrapping_sub(85) & 0x00ff) | ((original.wrapping_sub(341) >> 8 & 7) << 8);
    let fourth = (original.wrapping_add(0x0400) & 0x0700) | (original & 0x00ff);
    [
        i16::try_from(second).expect("11-bit retry heading"),
        i16::try_from(third).expect("11-bit retry heading"),
        i16::try_from(fourth).expect("11-bit retry heading"),
    ]
}

fn turn_toward_roll(source: &mut EntitySnapshot) {
    let desired = read_i16(source, 36);
    let mut delta = (i32::from(desired) & 0x07ff) - (i32::from(source.yaw) & 0x07ff);
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    let turn = delta.abs().min(i32::from(MULTIPART_DESCRIPTOR_TURN_LIMIT)) * delta.signum();
    source.set_yaw(i16::try_from((i32::from(source.yaw) + turn) & 0x07ff).expect("11-bit yaw"));
}

fn heading(from: Position, to: Position) -> i16 {
    i16::try_from(crate::heading_for_positions(from, to)).expect("source heading is 11-bit")
}

fn wrapped_axis_distance(left: u16, right: u16) -> i32 {
    i32::from(right.wrapping_sub(left).cast_signed()).abs()
}

fn wrapped_distance_2d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    crate::legacy_sqrt(square_i16(dx).wrapping_add(square_i16(dy)))
}

fn wrapped_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    crate::legacy_sqrt(
        square_i16(dx)
            .wrapping_add(square_i16(dy))
            .wrapping_add(square_i16(dz)),
    )
}

fn square_i16(value: i32) -> u32 {
    let value = i32::from(value as i16);
    (value * value) as u32
}

fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

fn read_u16(entity: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([
        entity.record_bytes()[offset],
        entity.record_bytes()[offset + 1],
    ])
}

fn read_i16(entity: &EntitySnapshot, offset: usize) -> i16 {
    i16::from_le_bytes([
        entity.record_bytes()[offset],
        entity.record_bytes()[offset + 1],
    ])
}

fn read_i32(entity: &EntitySnapshot, offset: usize) -> i32 {
    i32::from_le_bytes(
        entity.record_bytes()[offset..offset + 4]
            .try_into()
            .expect("fixed entity record field"),
    )
}

fn write_u16(entity: &mut EntitySnapshot, offset: usize, value: u16) {
    entity.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_roll(entity: &mut EntitySnapshot, roll: i16) {
    entity.record[36..38].copy_from_slice(&roll.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::{EntityAuthority, EntitySpecialRelation};

    #[derive(Clone, Copy)]
    struct FlatWorld;

    impl FlightWorld for FlatWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            0
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    #[derive(Clone, Copy)]
    struct BlockedWorld;

    impl FlightWorld for BlockedWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            u32::MAX
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    fn entity(slot: u16, class: u8, model: u8, action: u8, position: Position) -> EntitySnapshot {
        let mut record = vec![0_u8; 176].into_boxed_slice();
        record[0..2].copy_from_slice(&slot.to_le_bytes());
        record[2] = u8::from(slot != 0);
        record[8..12].copy_from_slice(&9_000_i32.to_le_bytes());
        record[12..16].copy_from_slice(&9_000_i32.to_le_bytes());
        record[30..32].copy_from_slice(&slot.to_le_bytes());
        record[67] = class;
        record[68] = model;
        record[73] = action;
        record[77] = 4;
        record[80..82].copy_from_slice(&position.x.to_le_bytes());
        record[82..84].copy_from_slice(&position.y.to_le_bytes());
        record[84..86].copy_from_slice(&position.z.to_le_bytes());
        record[88..90].copy_from_slice(&414_i16.to_le_bytes());
        record[90..92].copy_from_slice(&414_i16.to_le_bytes());
        record[134..136].copy_from_slice(&30_i16.to_le_bytes());
        EntitySnapshot {
            index: slot,
            source_id: slot,
            authority: if slot == 0 {
                EntityAuthority::Inactive
            } else {
                EntityAuthority::FrozenObserved
            },
            active: slot != 0,
            max_life: 9_000,
            life: 9_000,
            next_relation: None,
            class,
            model,
            control_flags: 0,
            entity_rng: 1,
            action,
            action_marker: 0,
            level_gate: 0,
            position,
            yaw: 0,
            pitch: 0,
            fov: 0,
            phase: 1,
            subspell_index: 0,
            byte_3b: 0,
            byte_3c: 0,
            font_type_index: 0,
            active_value: 0,
            activation_value: 0,
            mana_regen: 0,
            actual_speed: 30,
            max_mana: 0,
            mana_cost: 0,
            mana: 0,
            particle_parameter_index: None,
            parameter_relation: None,
            special_relation: EntitySpecialRelation::None,
            record,
        }
    }

    fn arena(count: u16) -> Vec<EntitySnapshot> {
        (0..count)
            .map(|slot| entity(slot, 0, 0, 0, Position { x: 0, y: 0, z: 0 }))
            .collect()
    }

    #[test]
    fn alliance_stage_and_attack_use_current_fixed_parent_record() {
        let mut entities = arena(5);
        let pos = Position { x: 1000, y: 1000, z: 0 };
        entities[1] = entity(1, 5, 3, 31, pos);
        entities[2] = entity(2, 3, 0, 0, pos);
        entities[3] = entity(3, 5, 21, 0, pos);
        entities[4] = entity(4, 5, 3, 232, pos);
        entities[1].record[76] = 0; entities[1].record[77] = 14;
        write_u16(&mut entities[1], 44, 2); write_u16(&mut entities[1], 50, 40);
        write_u16(&mut entities[1], 52, 40); write_u16(&mut entities[1], 56, 4);
        write_u16(&mut entities[2], 154, 3);
        entities[2].reclaim_source_row();
        let (effect, feedback) = step_raw_alliance_stage(&mut entities, 1, &FlatWorld).unwrap();
        assert_eq!(effect, MultipartRootExternalEffect::None);
        assert_eq!(feedback, vec![(2, 24)]);
        assert_eq!((entities[1].action, read_u16(&entities[1], 154)), (26, 3));
        write_u16(&mut entities[1], 154, 2);
        step_raw_multipart_action26(&mut entities, 1, &FlatWorld).unwrap();
        assert_eq!(entities[1].action, 25, "the parent address is rejected in attack26");
        entities[1].set_source_action(26);
        write_u16(&mut entities[1], 154, 3);
        entities[3].set_position(Position { x: 20_000, ..pos });
        entities[1].set_phase(1);
        assert!(step_raw_multipart_action26(&mut entities, 1, &FlatWorld).unwrap().launch.is_none());
        assert_eq!(entities[1].action, 26, "range is not tested between source cadence ticks");
        entities[1].set_phase(0);
        assert!(step_raw_multipart_action26(&mut entities, 1, &FlatWorld).unwrap().launch.is_none());
        assert_eq!(entities[1].action, 25, "the cadence visit rejects a distant target");
        entities[1].set_source_action(31);
        entities[2].set_life(0);
        step_raw_alliance_stage(&mut entities, 1, &FlatWorld).unwrap();
        assert_eq!((entities[1].record[77], read_u16(&entities[1], 44)), (10, 0));
        assert_eq!(entities[4].control_flags & 0x0600_0000, 0);
    }

    #[test]
    fn action232_accepts_dragon_limbs_without_admitting_other_models_or_dragon_roots() {
        let mut entities = arena(3);
        entities[1] = entity(
            1,
            5,
            0,
            1,
            Position {
                x: 700,
                y: 800,
                z: 90,
            },
        );
        entities[1].set_yaw(777);
        entities[2] = entity(2, 5, 0, 232, Position { x: 1, y: 2, z: 3 });
        write_u16(&mut entities[2], 54, 1);
        entities[2].set_phase(0);
        let step = step_raw_multipart_action232(&mut entities, 2).unwrap();
        assert!(step.followed_predecessor);
        assert_eq!(step.predicted_position, None, "the copy branch does not write predictedAxis");
        assert_eq!(
            (entities[2].position, entities[2].yaw),
            (entities[1].position, 777)
        );

        entities[2].model = 27;
        assert!(matches!(
            step_raw_multipart_action232(&mut entities, 2),
            Err(MultipartCallbackError::UnexpectedCarrier { model: 27, .. })
        ));
        entities[1].set_source_action(25);
        assert!(matches!(
            step_raw_multipart_action25(&mut entities, 1, &FlatWorld),
            Err(MultipartCallbackError::UnexpectedCarrier { model: 0, .. })
        ));
    }

    #[test]
    fn action232_follows_immediate_predecessor_and_consumes_only_mailbox_owner() {
        let mut entities = arena(4);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 10_000,
                y: 20_000,
                z: 300,
            },
        );
        entities[2] = entity(
            2,
            5,
            3,
            232,
            Position {
                x: 9_800,
                y: 20_000,
                z: 300,
            },
        );
        write_u16(&mut entities[2], 54, 1);
        write_u16(&mut entities[2], 58, 100);
        entities[2].record[61] = 1;
        entities[2].record[98..102].copy_from_slice(&250_i32.to_le_bytes());
        write_u16(&mut entities[2], 102, 3);
        let before_rng = entities[2].entity_rng;
        let before_phase = entities[2].phase;

        let step = step_raw_multipart_action232(&mut entities, 2).expect("valid child");

        assert!(step.followed_predecessor);
        assert_eq!(step.predicted_position, Some(entities[2].position));
        assert_eq!(step.consumed_mailbox_owner, Some(3));
        assert_eq!(entities[2].life, 8_750);
        assert_eq!(read_i32(&entities[2], 98), 250);
        assert_eq!(read_u16(&entities[2], 102), 0);
        assert_eq!(read_u16(&entities[2], 42), 3);
        assert_eq!(
            (entities[2].entity_rng, entities[2].phase),
            (before_rng, before_phase)
        );
        assert_eq!(entities[2].action, 232);
    }

    #[test]
    fn action232_hides_on_non_class5_parent_but_still_runs_callback_body() {
        let mut entities = arena(3);
        entities[1] = entity(
            1,
            7,
            0,
            0,
            Position {
                x: 1_000,
                y: 1_000,
                z: 0,
            },
        );
        entities[2] = entity(
            2,
            5,
            3,
            232,
            Position {
                x: 900,
                y: 1_000,
                z: 0,
            },
        );
        write_u16(&mut entities[2], 54, 1);
        write_u16(&mut entities[2], 58, 50);
        entities[2].record[61] = 1;

        let step = step_raw_multipart_action232(&mut entities, 2).expect("valid child");

        assert!(step.drawing_disabled);
        assert!(step.followed_predecessor);
        assert!(entities[2].drawing_disabled());
        assert_ne!(
            entities[2].position,
            Position {
                x: 900,
                y: 1_000,
                z: 0
            }
        );
    }

    #[test]
    fn action232_zero_byte39_copies_only_on_four_phase_cadence() {
        let mut entities = arena(3);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 7_000,
                y: 8_000,
                z: 90,
            },
        );
        entities[1].set_yaw(777);
        entities[2] = entity(2, 5, 3, 232, Position { x: 1, y: 2, z: 3 });
        write_u16(&mut entities[2], 54, 1);
        entities[2].record[61] = 0;
        entities[2].set_phase(3);
        let before = entities[2].position;
        assert!(
            !step_raw_multipart_action232(&mut entities, 2)
                .expect("valid child")
                .followed_predecessor
        );
        assert_eq!(entities[2].position, before);

        entities[2].set_phase(4);
        assert!(
            step_raw_multipart_action232(&mut entities, 2)
                .expect("valid child")
                .followed_predecessor
        );
        assert_eq!(
            (entities[2].position, entities[2].yaw),
            (entities[1].position, 777)
        );
    }

    #[test]
    fn action25_selects_nearest_source_eligible_player() {
        let source_position = Position {
            x: 10_000,
            y: 10_000,
            z: 1,
        };
        let mut entities = arena(5);
        entities[1] = entity(1, 5, 3, 25, source_position);
        entities[1].set_phase(0);
        entities[1].set_actual_speed(0);
        entities[1].set_yaw(512);
        set_roll(&mut entities[1], 512);
        entities[1].record[61] = 1;

        entities[2] = entity(
            2,
            3,
            0,
            0,
            Position {
                x: 11_000,
                ..source_position
            },
        );
        entities[2].set_life(-1);
        entities[2].disable_drawing();
        entities[3] = entity(
            3,
            3,
            0,
            0,
            Position {
                x: 10_100,
                ..source_position
            },
        );
        let disabled_flags = entities[3].control_flags | 0x20;
        entities[3].set_control_flags(disabled_flags);
        entities[4] = entity(
            4,
            3,
            0,
            0,
            Position {
                x: 10_500,
                ..source_position
            },
        );

        step_raw_multipart_action25(&mut entities, 1, &FlatWorld).expect("valid idle Worm");

        assert_eq!(entities[1].action, 26);
        assert_eq!(read_u16(&entities[1], 154), 4);
    }

    #[test]
    fn action27_propagates_peer_state_and_honors_entry_zero_relation() {
        let mut entities = arena(4);
        entities[1] = entity(
            1,
            5,
            3,
            27,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[1].set_phase(0);
        entities[1].set_actual_speed(0);
        write_u16(&mut entities[1], 54, 2);
        entities[2] = entity(
            2,
            5,
            3,
            27,
            Position {
                x: 20_000,
                y: 10_000,
                z: 0,
            },
        );
        write_u16(&mut entities[2], 54, 0);
        entities[2].record[138..140].copy_from_slice(&40_i16.to_le_bytes());
        entities[2].set_actual_speed(7);

        step_raw_multipart_action27(&mut entities, 1, &FlatWorld).expect("valid linked Worm");

        assert_eq!(read_u16(&entities[1], 54), 0);
        assert_eq!(entities[1].action, 27);
        assert_eq!(entities[1].actual_speed, 47);
        assert_eq!(
            read_i16(&entities[1], 36),
            heading(entities[1].position, entities[0].position)
        );

        entities[1].set_source_action(27);
        write_u16(&mut entities[1], 54, 0);
        entities[1].record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        write_u16(&mut entities[1], 102, 3);
        let life_before = entities[1].life;
        step_raw_multipart_action27(&mut entities, 1, &FlatWorld)
            .expect("zero relation returns to idle before the prelude");
        assert_eq!(entities[1].action, 25);
        assert_eq!(entities[1].life, life_before);
        assert_eq!(read_u16(&entities[1], 102), 3);
    }

    #[test]
    fn action25_nearest_peer_flows_through_action27_to_attack() {
        let source_position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        let mut entities = arena(5);
        entities[1] = entity(1, 5, 3, 25, source_position);
        entities[1].set_phase(0);
        entities[1].set_actual_speed(0);
        entities[1].set_yaw(1536);
        set_roll(&mut entities[1], 512);
        entities[1].record[61] = 1;
        entities[2] = entity(
            2,
            5,
            3,
            25,
            Position {
                x: 11_000,
                ..source_position
            },
        );
        entities[3] = entity(
            3,
            5,
            3,
            26,
            Position {
                x: 10_500,
                ..source_position
            },
        );
        write_u16(&mut entities[3], 154, 4);

        step_raw_multipart_action25(&mut entities, 1, &FlatWorld).expect("valid idle Worm");
        assert_eq!((entities[1].action, read_u16(&entities[1], 54)), (27, 3));

        step_raw_multipart_action27(&mut entities, 1, &FlatWorld).expect("valid linked Worm");
        assert_eq!(entities[1].action, 26);
        assert_eq!(read_u16(&entities[1], 54), 0);
        assert_eq!(read_u16(&entities[1], 154), 4);
    }

    #[test]
    fn action31_adopts_first_lower_child_and_enters_source_pre_kill() {
        let mut entities = arena(5);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 1_000,
                y: 1_000,
                z: 100,
            },
        );
        write_u16(&mut entities[1], 56, 2);
        entities[2] = entity(2, 5, 3, 232, entities[1].position);
        entities[2].set_life(8_500);
        write_u16(&mut entities[2], 42, 4);
        write_u16(&mut entities[2], 56, 3);
        entities[3] = entity(3, 5, 3, 232, entities[1].position);
        entities[3].set_life(-10);
        write_u16(&mut entities[3], 42, 4);
        // Source stops at the first lower child, so make that first child the
        // lethal one for this discriminator.
        entities[2].set_life(-5);

        let step =
            step_raw_multipart_action31(&mut entities, 1, &FlatWorld, MultipartStageTarget::None)
                .expect("valid root");

        assert_eq!(
            step.disposition,
            MultipartRootDisposition::PreKill { killer_slot: 4 }
        );
        assert_eq!(entities[1].life, -5);
        assert_eq!(
            (read_u16(&entities[1], 40), read_u16(&entities[1], 42)),
            (4, 4)
        );
        assert_eq!(entities[1].action, 28);
        assert_eq!(step.rng_draws, 0);
    }

    #[test]
    fn action31_mailbox_hit_clears_owner_retains_payload_and_settles() {
        let mut entities = arena(4);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 1_000,
                y: 1_000,
                z: 100,
            },
        );
        entities[3] = entity(3, 3, 0, 0, entities[1].position);
        entities[1].record[98..102].copy_from_slice(&250_i32.to_le_bytes());
        write_u16(&mut entities[1], 102, 3);

        let step =
            step_raw_multipart_action31(&mut entities, 1, &FlatWorld, MultipartStageTarget::None)
                .expect("valid root");

        assert_eq!(
            step.disposition,
            MultipartRootDisposition::Attack { target_slot: 3 }
        );
        assert_eq!(entities[1].life, 8_750);
        assert_eq!(read_i32(&entities[1], 98), 250);
        assert_eq!(read_u16(&entities[1], 102), 0);
        assert_eq!(entities[1].position.z, 36);
        assert_eq!(step.motion, MultipartMotionResult::NotRun);
    }

    #[test]
    fn action31_phase64_target_path_consumes_exactly_two_local_rng_draws() {
        let mut entities = arena(4);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[1].set_phase(64);
        entities[1].set_actual_speed(0);
        entities[1].set_entity_rng(0x1234);
        entities[2] = entity(
            2,
            3,
            0,
            0,
            Position {
                x: 20_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[2].record[88..90].copy_from_slice(&10_i16.to_le_bytes());
        let first = next_rng(0x1234);
        let second = next_rng(first);

        let step = step_raw_multipart_action31(
            &mut entities,
            1,
            &FlatWorld,
            MultipartStageTarget::Entity(2),
        )
        .expect("valid root");

        assert_eq!(step.rng_draws, 2);
        assert_eq!(entities[1].entity_rng, second);
        assert_eq!(entities[1].action, 31);
        assert!(!step.completed_stage_target);
    }

    #[test]
    fn action31_peer_scan_excludes_action232_presentation_children() {
        let mut entities = arena(4);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[1].set_phase(8);
        entities[1].set_actual_speed(0);
        entities[2] = entity(
            2,
            3,
            0,
            0,
            Position {
                x: 20_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[3] = entity(
            3,
            5,
            3,
            232,
            Position {
                x: 10_000,
                y: 10_100,
                z: 0,
            },
        );
        let expected_target_heading = heading(entities[1].position, entities[2].position);

        let step = step_raw_multipart_action31(
            &mut entities,
            1,
            &FlatWorld,
            MultipartStageTarget::Entity(2),
        )
        .expect("valid root");

        assert_eq!(read_i16(&entities[1], 36), expected_target_heading);
        assert_eq!(step.external_effect, MultipartRootExternalEffect::None);
    }

    #[test]
    fn action31_blocked_motion_reports_external_live_counter_decrement() {
        let mut entities = arena(2);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[1].set_actual_speed(512);

        let step = step_raw_multipart_action31(
            &mut entities,
            1,
            &BlockedWorld,
            MultipartStageTarget::None,
        )
        .expect("valid root");

        assert_eq!(step.motion, MultipartMotionResult::Blocked);
        assert_eq!(entities[1].life, -1);
        assert_eq!(
            step.external_effect,
            MultipartRootExternalEffect::DecrementLiveEntityCounter
        );
        // This movement death becomes action 28 only on the following root
        // callback, after the scheduler-owned common phase.
        assert_eq!(entities[1].action, 31);
    }

    #[test]
    fn action31_stage4_completion_uses_targets_word96_relation() {
        let mut entities = arena(5);
        entities[1] = entity(
            1,
            5,
            3,
            31,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        );
        entities[1].set_phase(8);
        entities[1].set_actual_speed(0);
        entities[2] = entity(2, 11, 0, 0, entities[1].position);
        write_u16(&mut entities[2], 154, 3);
        entities[3] = entity(
            3,
            3,
            0,
            0,
            Position {
                x: 10_100,
                y: 10_000,
                z: 0,
            },
        );

        let step = step_raw_multipart_action31(
            &mut entities,
            1,
            &FlatWorld,
            MultipartStageTarget::Entity(2),
        )
        .expect("valid root");

        assert!(step.completed_stage_target);
        assert_eq!(entities[1].record_bytes()[77], 10);
        assert_eq!(entities[1].action, 26);
        assert_eq!(read_u16(&entities[1], 154), 3);
    }

    #[test]
    fn malformed_child_chain_fails_before_consuming_root_mailbox() {
        let mut entities = arena(4);
        entities[1] = entity(1, 5, 3, 31, Position { x: 0, y: 0, z: 0 });
        write_u16(&mut entities[1], 56, 2);
        entities[1].record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        write_u16(&mut entities[1], 102, 3);
        entities[2] = entity(2, 5, 3, 232, entities[1].position);
        write_u16(&mut entities[2], 56, 99);

        let before = entities[1].clone();
        let error =
            step_raw_multipart_action31(&mut entities, 1, &FlatWorld, MultipartStageTarget::None)
                .expect_err("bad relation must fail");

        assert!(matches!(
            error,
            MultipartCallbackError::RelationOutOfBounds {
                field: "word-0x34",
                related_slot: 99,
                ..
            }
        ));
        assert_eq!(entities[1], before);
    }
}
