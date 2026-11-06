//! Source-shaped callbacks shared by descriptor-backed campaign packages.
//!
//! These helpers operate on the lossless imported entity row. They deliberately
//! stop at the two engine-owned boundaries which cannot be row-local:
//! allocation is supplied by the caller and player/stage side effects are
//! returned as typed outcomes. The ordinary numeric-slot scheduler remains
//! responsible for the same-pass phase epilogue and following-pass reclaim.

use super::*;

const TREE_CLASS: u8 = 2;
const TREE_MODEL: u8 = 0;
const TREE_ACTION: u8 = 0;
const STONE_CLASS: u8 = 2;
const STONE_MODEL: u8 = 1;
const STONE_ACTION: u8 = 3;
const STONE_PARTICLE: u16 = 79;
const MODEL9_CLASS: u8 = 5;
const MODEL9_MODEL: u8 = 9;
const MODEL9_ACTION_COUNTDOWN: u8 = 72;
const MODEL9_ACTION_MOVEMENT: u8 = 73;
const MODEL9_ACTION_REACTION: u8 = 74;
const MODEL9_ACTION_PRE_KILL: u8 = 76;
const MODEL9_ACTION_RAW_MOVEMENT: u8 = 79;
const MODEL9_MOVEMENT_COUNTDOWN: i32 = 400;
/// Source selectors for the MC2 Skeleton adapter; the family and constructor
/// consume the same binding when creating the root and its arrow.
pub(super) struct Mc2SkeletonAssets {
    pub(super) descriptor: u16,
    pub(super) initial_particle: u16,
    pub(super) movement_particle: u16,
    pub(super) reaction_particle: u16,
    pub(super) arrow_particle: u16,
}

pub(super) const MC2_SKELETON_ASSETS: Mc2SkeletonAssets = Mc2SkeletonAssets {
    descriptor: 80,
    initial_particle: 220,
    movement_particle: 201,
    reaction_particle: 202,
    arrow_particle: 203,
};
const MODEL9_MOVEMENT_PARTICLE: u16 = MC2_SKELETON_ASSETS.movement_particle;
// Pinned remc2 `str_D7BD6[80]`, installed by `sub_4BBB0`.
const MODEL9_TURN_LIMIT: i16 = 0x16;
const MODEL9_SLOPE_LIMIT: i32 = 0x14;
const MODEL9_MATERIAL_MASK: u32 = 0xfff0_80fe;
const MODEL9_BLOCKED_BIT: u32 = 0x0004_0000;
const MODEL9_SKIP_MOVEMENT_BIT: u32 = 0x0000_0800;
const PICKUP_CLASS: u8 = 15;

// Pinned remc2 `x_BYTE_D8A2E`. `SetEntityIndexAndRot_49CD0` indexes this
// table with the package-derived particle render mode.
const PARTICLE_ANIMATION_LIMITS: [u8; 38] = [
    1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8,
    9, 10, 11, 12, 13, 14, 15, 16, 0,
];

pub(super) fn particle_animation_limit(render_mode: u8) -> Option<u8> {
    PARTICLE_ANIMATION_LIMITS
        .get(usize::from(render_mode))
        .copied()
}

const CONTROL_MAP_LINKED_BIT: u32 = 0x0000_0004;
const CONTROL_TREE_CALLBACK_BIT: u32 = 0x0002_0000;
const CONTROL_PICKUP_OWNED_BIT: u32 = 0x0000_0001;
const CONTROL_PICKUP_BYTE3_BIT1: u32 = 0x0200_0000;

/// Construct the lossless source row produced by remc2 `AddStone_4AD70`.
///
/// `NewEvent_4A050` supplies the common row defaults. The stone constructor
/// clears the initial low-byte bit 3, binds the slot-modulo-eleven word,
/// inserts the row into the map chain, copies max life to life, and installs
/// particle 79. The numeric scheduler owns the later action-3 callback and
/// common phase epilogue.
pub(super) fn construct_stone_source(
    source_slot: u16,
    entity_rng: u16,
    position: Position,
    map_links: (u16, u16),
    particle_pose: ParticlePose,
) -> EntitySnapshot {
    let mut record = vec![0_u8; 176].into_boxed_slice();
    record[0..2].copy_from_slice(&source_slot.to_le_bytes());
    record[2] = 1;
    record[8..12].copy_from_slice(&300_i32.to_le_bytes());
    record[12..16].copy_from_slice(&300_i32.to_le_bytes());
    record[16..20].copy_from_slice(&CONTROL_MAP_LINKED_BIT.to_le_bytes());
    record[20..24].copy_from_slice(&i32::from(source_slot % 11).to_le_bytes());
    record[24..26].copy_from_slice(&entity_rng.to_le_bytes());
    record[26..28].copy_from_slice(&map_links.0.to_le_bytes());
    record[28..30].copy_from_slice(&map_links.1.to_le_bytes());
    record[30..32].copy_from_slice(&source_slot.to_le_bytes());
    record[46..48].copy_from_slice(&100_u16.to_le_bytes());
    record[61] = (-6_i8).to_ne_bytes()[0];
    record[66] = source_slot as u8;
    record[67] = STONE_CLASS;
    record[68] = STONE_MODEL;
    record[69] = (-1_i8).to_ne_bytes()[0];
    record[70] = (-1_i8).to_ne_bytes()[0];
    record[71] = 10;
    record[73] = STONE_ACTION;
    record[80..82].copy_from_slice(&position.x.to_le_bytes());
    record[82..84].copy_from_slice(&position.y.to_le_bytes());
    record[84..86].copy_from_slice(&position.z.to_le_bytes());
    record[86..88].copy_from_slice(&particle_pose.array_yaw.to_le_bytes());
    record[88..90].copy_from_slice(&particle_pose.array_pitch.to_le_bytes());
    record[90..92].copy_from_slice(&particle_pose.array_roll.to_le_bytes());
    record[92..94].copy_from_slice(&particle_pose.array_fov.to_le_bytes());
    record[94..96].copy_from_slice(&STONE_PARTICLE.to_le_bytes());
    record[97] = particle_pose.animation_limit;
    record[134..136].copy_from_slice(&16_i16.to_le_bytes());
    // NewEvent binds source parameter row 59 and the global special-state
    // carrier. The pointer-free export stores both as one-based relations.
    record[164..168].copy_from_slice(&60_u32.to_le_bytes());
    record[168] = 2;
    record[172..176].copy_from_slice(&1_u32.to_le_bytes());

    EntitySnapshot {
        index: source_slot,
        source_id: source_slot,
        authority: EntityAuthority::FrozenObserved,
        active: true,
        max_life: 300,
        life: 300,
        next_relation: None,
        class: STONE_CLASS,
        model: STONE_MODEL,
        control_flags: CONTROL_MAP_LINKED_BIT,
        entity_rng,
        action: STONE_ACTION,
        action_marker: 0,
        level_gate: 0,
        position,
        yaw: 0,
        pitch: 0,
        fov: 0,
        phase: source_slot as u8,
        subspell_index: 100,
        byte_3b: 0,
        byte_3c: 0,
        font_type_index: 0,
        active_value: 0,
        activation_value: 0,
        mana_regen: 0,
        actual_speed: 16,
        max_mana: 0,
        mana_cost: 0,
        mana: 0,
        particle_parameter_index: Some(STONE_PARTICLE),
        parameter_relation: Some(59),
        special_relation: EntitySpecialRelation::GlobalSpecialState,
        record,
    }
}

/// `AddDolmen_4ADF0` shares NewEvent and map defaults with the stone, then
/// selects action 6, particle 39 and the fixed 1024-unit collision extents.
pub(super) fn construct_dolmen_source(
    source_slot: u16,
    entity_rng: u16,
    position: Position,
    particle_pose: ParticlePose,
) -> EntitySnapshot {
    let mut row = construct_stone_source(source_slot, entity_rng, position, (0, 0), particle_pose);
    row.model = 2;
    row.action = 6;
    row.particle_parameter_index = Some(39);
    row.record[68] = 2;
    row.record[73] = 6;
    row.record[94..96].copy_from_slice(&39_i16.to_le_bytes());
    row
}

/// Lossless `sub_4BBB0` row, after NewEvent and before its placement probe.
pub(super) fn construct_skeleton_source(
    source_slot: u16,
    initial_rng: u16,
    position: Position,
    phase: u8,
    scan_period: u16,
    particle_pose: ParticlePose,
) -> EntitySnapshot {
    let properties = crate::level_definition::SKELETON_CLASS_PROPERTIES;
    let assets = MC2_SKELETON_ASSETS;
    let mana = properties.max_life >> 1;
    let entity_rng = next_source_rng16(initial_rng);
    let angle = (entity_rng % 0x832) as i16 - 1;
    let control_flags = CONTROL_MAP_LINKED_BIT | 8;
    let mut record = vec![0_u8; 176].into_boxed_slice();
    record[0..2].copy_from_slice(&source_slot.to_le_bytes());
    record[2] = 1;
    record[8..12].copy_from_slice(&properties.max_life.to_le_bytes());
    record[12..16].copy_from_slice(&properties.max_life.to_le_bytes());
    record[16..20].copy_from_slice(&control_flags.to_le_bytes());
    record[20..24].copy_from_slice(&16_i32.to_le_bytes());
    record[24..26].copy_from_slice(&entity_rng.to_le_bytes());
    record[30..32].copy_from_slice(&source_slot.to_le_bytes());
    record[32..34].copy_from_slice(&angle.to_le_bytes());
    record[34..36].copy_from_slice(&angle.to_le_bytes());
    record[36..38].copy_from_slice(&angle.to_le_bytes());
    record[46..48].copy_from_slice(&properties.initial_subspell.to_le_bytes());
    record[60] = 1;
    record[61] = (scan_period - u16::from(phase) % scan_period).wrapping_add(4) as u8;
    record[66] = phase;
    record[67] = 5;
    record[68] = 9;
    record[69] = 3;
    record[70] = 255;
    record[71] = 10;
    record[73] = 72;
    record[80..82].copy_from_slice(&position.x.to_le_bytes());
    record[82..84].copy_from_slice(&position.y.to_le_bytes());
    record[84..86].copy_from_slice(&position.z.to_le_bytes());
    record[86..88].copy_from_slice(&particle_pose.array_yaw.to_le_bytes());
    for offset in [88, 90, 92] {
        record[offset..offset + 2].copy_from_slice(&128_i16.to_le_bytes());
    }
    record[94..96].copy_from_slice(&assets.initial_particle.to_le_bytes());
    record[97] = particle_pose.animation_limit;
    record[134..136].copy_from_slice(&properties.minimum_speed.to_le_bytes());
    record[136..138].copy_from_slice(&properties.minimum_speed.to_le_bytes());
    record[148..152].copy_from_slice(&mana.to_le_bytes());
    record[164..168].copy_from_slice(&(u32::from(assets.descriptor) + 1).to_le_bytes());
    record[168] = 2;
    record[172..176].copy_from_slice(&1_u32.to_le_bytes());
    EntitySnapshot {
        index: source_slot,
        source_id: source_slot,
        authority: EntityAuthority::FrozenObserved,
        active: true,
        max_life: properties.max_life,
        life: properties.max_life,
        next_relation: None,
        class: 5,
        model: 9,
        control_flags,
        entity_rng,
        action: 72,
        action_marker: 0,
        level_gate: 0,
        position,
        yaw: angle,
        pitch: angle,
        fov: 0,
        phase,
        subspell_index: properties.initial_subspell,
        byte_3b: 0,
        byte_3c: 0,
        font_type_index: 0,
        active_value: 0,
        activation_value: 0,
        mana_regen: 0,
        actual_speed: properties.minimum_speed,
        max_mana: 0,
        mana_cost: 0,
        mana,
        particle_parameter_index: Some(assets.initial_particle),
        parameter_relation: Some(assets.descriptor),
        special_relation: EntitySpecialRelation::GlobalSpecialState,
        record,
    }
}

/// `sub_102D0(..., 1)` visits only the origin for the constructor's 128-unit
/// half extents. Its movement-only slope test must not reject placement.
pub(super) fn skeleton_placement_blocked<W: FlightWorld>(
    world: &W,
    position: Position,
    material_mask: u32,
    ground_clearance: i16,
) -> bool {
    world.material_mask(position) & !material_mask != 0
        || world.cave_collision_blocked(position)
        || world.ceiling_height(position).is_some_and(|ceiling| {
            i32::from(world.ground_height(position)) + 128 + i32::from(ground_clearance)
                > i32::from(ceiling)
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CampaignSharedFamilyError {
    pub source_slot: u16,
    pub expected: (u8, u8, u8),
    pub actual: (u8, u8, u8),
}

impl fmt::Display for CampaignSharedFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "campaign source slot {} expected class/model/action {}/{}/{}, found {}/{}/{}",
            self.source_slot,
            self.expected.0,
            self.expected.1,
            self.expected.2,
            self.actual.0,
            self.actual.1,
            self.actual.2
        )
    }
}

impl std::error::Error for CampaignSharedFamilyError {}

fn require_selector(
    entity: &EntitySnapshot,
    expected: (u8, u8, u8),
) -> std::result::Result<(), CampaignSharedFamilyError> {
    let actual = (entity.class, entity.model, entity.action);
    if actual == expected {
        Ok(())
    } else {
        Err(CampaignSharedFamilyError {
            source_slot: entity.index,
            expected,
            actual,
        })
    }
}

pub(super) fn next_source_rng16(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScenerySpawnKind {
    /// `IfSubtypeCallCreatingManaSphere_4A190(position, 10, 6)`.
    TreeDeathSphere,
    /// `IfSubtypeCallCreatingManaSphere_4A190(position, 10, 5)`.
    WaterSplash,
}

/// Complete post-constructor writes required by a scenery callback.
///
/// `inherit_source_id_from` is a live entity slot whose source id must be
/// copied onto the allocated row. A value of `None` means the scenery row's
/// own source id is used. The allocator returns the allocated source slot;
/// `None` is the source's ordinary exhausted-arena result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScenerySpawnRequest {
    pub kind: ScenerySpawnKind,
    pub source_position: Position,
    pub inherit_source_id_from: Option<u16>,
    pub fallback_source_id: u16,
    pub word_0x2c: i16,
    pub position_z: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TreeAction0Outcome {
    pub damage_applied: i32,
    pub death_sphere_slot: Option<u16>,
    pub water_splash_slot: Option<u16>,
    pub rng_draws: u8,
    pub drawing_disabled: bool,
}

/// Port remc2 `AddTree02_00_64E20` against an imported class-2 tree row.
///
/// The allocation callback is intentionally invoked in source order. A failed
/// death-sphere allocation clears the damage owner mailbox but consumes no RNG
/// and leaves the negative life/action untouched. Water always hides the tree,
/// even when its splash allocation fails.
pub(super) fn step_tree_action0<F>(
    entity: &mut EntitySnapshot,
    ground_height: i16,
    is_water: bool,
    mut allocate: F,
) -> std::result::Result<TreeAction0Outcome, CampaignSharedFamilyError>
where
    F: FnMut(ScenerySpawnRequest) -> Option<u16>,
{
    require_selector(entity, (TREE_CLASS, TREE_MODEL, TREE_ACTION))?;
    let mut outcome = TreeAction0Outcome::default();
    entity.set_control_flags(entity.control_flags | CONTROL_TREE_CALLBACK_BIT);

    let damage_owner = entity.source_upkeep_word_62();
    if damage_owner != 0 {
        let damage = entity.source_upkeep_payload();
        outcome.damage_applied = damage;
        entity.set_life(entity.life.wrapping_sub(damage));
        if entity.life < 0 {
            let word_0x2c = ((i32::from(entity.fov) * 3) >> 2) as i16;
            let position_z = if entity.position.z <= 128 {
                0
            } else {
                entity.position.z.wrapping_sub(128)
            };
            let request = ScenerySpawnRequest {
                kind: ScenerySpawnKind::TreeDeathSphere,
                source_position: entity.position,
                inherit_source_id_from: Some(damage_owner),
                fallback_source_id: entity.source_id,
                word_0x2c,
                position_z,
            };
            if let Some(source_slot) = allocate(request) {
                outcome.death_sphere_slot = Some(source_slot);
                let rng = next_source_rng16(entity.entity_rng);
                entity.set_entity_rng(rng);
                entity.set_life(i32::from(rng % 0x3c) + 130);
                entity.set_control_flags(
                    (entity.control_flags & !0x0002_0008) | CONTROL_TREE_CALLBACK_BIT,
                );
                entity.set_source_action(1);
                outcome.rng_draws = 1;
            }
        }
        entity.clear_source_upkeep_owner();
    }

    let mut position = entity.position;
    position.z = ground_height;
    entity.set_position(position);
    if is_water {
        outcome.water_splash_slot = allocate(ScenerySpawnRequest {
            kind: ScenerySpawnKind::WaterSplash,
            source_position: position,
            inherit_source_id_from: None,
            fallback_source_id: entity.source_id,
            word_0x2c: 0,
            position_z: position.z,
        });
        entity.disable_drawing();
        outcome.drawing_disabled = true;
    }
    Ok(outcome)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoneAction3Outcome {
    pub previous_position: Position,
    pub position: Position,
    pub previous_control_flags: u32,
    pub control_flags: u32,
}

/// Port remc2 `AddStatue02_01_65040` for class-2/model-1/action-3.
///
/// Despite the constructor name, dispatch is selected by action: this callback
/// only sets source control byte 2 bit 1 and snaps Z. It performs no water
/// query, allocation, RNG draw, or draw-disable transition.
pub(super) fn step_stone_action3(
    entity: &mut EntitySnapshot,
    ground_height: i16,
) -> std::result::Result<StoneAction3Outcome, CampaignSharedFamilyError> {
    require_selector(entity, (STONE_CLASS, STONE_MODEL, STONE_ACTION))?;
    let previous_position = entity.position;
    let previous_control_flags = entity.control_flags;
    entity.set_control_flags(entity.control_flags | CONTROL_TREE_CALLBACK_BIT);
    let mut position = entity.position;
    position.z = ground_height;
    entity.set_position(position);
    Ok(StoneAction3Outcome {
        previous_position,
        position,
        previous_control_flags,
        control_flags: entity.control_flags,
    })
}

/// Package-derived particle row needed by `SetEntityIndexAndRot_49CD0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ParticlePose {
    pub animation_limit: u8,
    pub array_yaw: i16,
    pub array_pitch: i16,
    pub array_roll: i16,
    pub array_fov: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Model9Action72Outcome {
    pub transitioned: bool,
    pub animation_advanced: bool,
    /// `sub_122A0` advances this source StageVars2 relation after transition.
    pub stage_relation_to_advance: Option<u16>,
}

/// Port remc2 `sub_20370`, including its pre-decrement zero test.
pub(super) fn step_model9_action72(
    entity: &mut EntitySnapshot,
    movement_pose: ParticlePose,
) -> std::result::Result<Model9Action72Outcome, CampaignSharedFamilyError> {
    require_selector(
        entity,
        (MODEL9_CLASS, MODEL9_MODEL, MODEL9_ACTION_COUNTDOWN),
    )?;
    let previous_countdown = entity.source_dword_10();
    let next_countdown = previous_countdown.wrapping_sub(1);
    entity.set_source_dword_10(next_countdown);
    let mut outcome = Model9Action72Outcome::default();

    if previous_countdown != 0 {
        if next_countdown < 16 && next_countdown % 2 == 0 && entity.record[96] < entity.record[97] {
            entity.record[96] = entity.record[96].wrapping_add(1);
            outcome.animation_advanced = true;
        }
        return Ok(outcome);
    }

    entity.set_actual_speed(entity.minimum_speed());
    entity.particle_parameter_index = Some(MODEL9_MOVEMENT_PARTICLE);
    entity.record[94..96].copy_from_slice(&MODEL9_MOVEMENT_PARTICLE.to_le_bytes());
    entity.record[96] = 0;
    entity.record[97] = movement_pose.animation_limit;
    entity.record[86..88].copy_from_slice(&movement_pose.array_yaw.to_le_bytes());
    entity.record[88..90].copy_from_slice(&movement_pose.array_pitch.to_le_bytes());
    entity.record[90..92].copy_from_slice(&movement_pose.array_roll.to_le_bytes());
    entity.record[92..94].copy_from_slice(&movement_pose.array_fov.to_le_bytes());
    entity.record[69] = 3;
    entity.record[70] = (-1_i8).to_ne_bytes()[0];
    entity.set_source_action(MODEL9_ACTION_MOVEMENT);
    entity.set_source_dword_10(MODEL9_MOVEMENT_COUNTDOWN);
    entity.level_gate = 0;
    entity.record[74] = 0;
    outcome.transitioned = true;
    let stage_relation = u16::from_le_bytes([entity.record[78], entity.record[79]]);
    outcome.stage_relation_to_advance = (stage_relation != 0).then_some(stage_relation);
    Ok(outcome)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Model9Action79Outcome {
    pub population_decrement: bool,
    pub moved: bool,
    pub movement_attempts: u8,
    pub steered: bool,
    pub rng_draws: u8,
    pub peer_avoidance_slot: Option<u16>,
    pub damage_transition: Option<Model9DamageTransition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Model9DamageTransition {
    Reaction { attacker_slot: u16 },
    PreKill { attacker_slot: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Model9ReactionCandidate {
    pub source_slot: u16,
    pub position: Position,
}

/// Read-only attacker identity at the callback's source-cursor boundary.
/// Dynamic creature rows need not exist in the imported reset table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Model9DamageTarget {
    pub source_slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Model9ReactionOutcome {
    pub population_decrement: bool,
    pub prey_to_consume: Option<u16>,
    pub inherit_identity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Model9Action79Error {
    Selector(CampaignSharedFamilyError),
    InvalidChildChain {
        source_slot: u16,
        child_slot: u16,
    },
    InvalidDamageTarget {
        source_slot: u16,
        target_slot: u16,
    },
    PeerRead {
        source_slot: u16,
        peer_slot: u16,
        reason: String,
        /// Movement has completed; its map/population writes still apply.
        completed: Model9Action79Outcome,
    },
}

impl fmt::Display for Model9Action79Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selector(error) => error.fmt(formatter),
            Self::InvalidChildChain {
                source_slot,
                child_slot,
            } => write!(
                formatter,
                "model-9 source slot {source_slot} has invalid child-chain slot {child_slot}"
            ),
            Self::InvalidDamageTarget {
                source_slot,
                target_slot,
            } => write!(
                formatter,
                "model-9 source slot {source_slot} has invalid damage target slot {target_slot}"
            ),
            Self::PeerRead {
                source_slot,
                peer_slot,
                reason,
                ..
            } => write!(
                formatter,
                "model-9 source slot {source_slot} cannot read current peer slot {peer_slot}: {reason}"
            ),
        }
    }
}

fn settle_model9_after_damage<W: FlightWorld>(entity: &mut EntitySnapshot, world: &W) {
    let ground = world.ground_height(entity.position);
    if entity.position.z > ground {
        entity.position.z = entity.position.z.wrapping_sub(128);
    }
    if entity.position.z <= ground {
        entity.position.z = ground;
    }
    entity.set_position(entity.position);
}

fn apply_model9_reaction_pose(
    entity: &mut EntitySnapshot,
    target: Model9DamageTarget,
    reaction_pose: ParticlePose,
) {
    if entity.source_id == target.source_id {
        entity.set_source_action(MODEL9_ACTION_MOVEMENT);
        return;
    }
    entity.set_actual_speed(0);
    entity.particle_parameter_index = Some(MC2_SKELETON_ASSETS.reaction_particle);
    entity.record[94..96].copy_from_slice(&MC2_SKELETON_ASSETS.reaction_particle.to_le_bytes());
    entity.record[96] = 0;
    entity.record[97] = reaction_pose.animation_limit;
    entity.record[86..88].copy_from_slice(&reaction_pose.array_yaw.to_le_bytes());
    entity.record[88..90].copy_from_slice(&reaction_pose.array_pitch.to_le_bytes());
    entity.record[90..92].copy_from_slice(&reaction_pose.array_roll.to_le_bytes());
    entity.record[92..94].copy_from_slice(&reaction_pose.array_fov.to_le_bytes());
    entity.record[69] = target.class;
    entity.record[70] = target.model;
}

fn consume_model9_damage<W: FlightWorld>(
    entity: &mut EntitySnapshot,
    world: &W,
    entities: &[EntitySnapshot],
    dynamic_targets: &[Model9DamageTarget],
    reaction_pose: ParticlePose,
) -> std::result::Result<Option<Model9DamageTransition>, Model9Action79Error> {
    let mut damage_owner = entity.source_upkeep_word_62();
    let mut damage_seen = damage_owner != 0;
    if damage_seen {
        entity.set_life(entity.life.wrapping_sub(entity.source_upkeep_payload()));
        entity.clear_source_upkeep_owner();
    } else {
        damage_owner = 0;
    }
    entity.set_damage_result_words(0, damage_owner);

    let mut child_slot = entity.word_34();
    let mut visited = 0_usize;
    while child_slot != 0 {
        let child = entities
            .get(usize::from(child_slot))
            .filter(|child| child.active && child.index == child_slot)
            .ok_or(Model9Action79Error::InvalidChildChain {
                source_slot: entity.index,
                child_slot,
            })?;
        if child.life < entity.life {
            entity.set_life(child.life);
            damage_owner = child.word_26();
            entity.set_damage_result_words(0, damage_owner);
            damage_seen = true;
            break;
        }
        child_slot = child.word_34();
        visited += 1;
        if visited >= entities.len() {
            return Err(Model9Action79Error::InvalidChildChain {
                source_slot: entity.index,
                child_slot,
            });
        }
    }

    let transition = if entity.life < 0 {
        entity.set_damage_result_words(damage_owner, damage_owner);
        entity.set_source_action(MODEL9_ACTION_PRE_KILL);
        Some(Model9DamageTransition::PreKill {
            attacker_slot: damage_owner,
        })
    } else if damage_seen {
        entity.record[154..156].copy_from_slice(&damage_owner.to_le_bytes());
        entity.set_source_action(MODEL9_ACTION_REACTION);
        Some(Model9DamageTransition::Reaction {
            attacker_slot: damage_owner,
        })
    } else {
        None
    };

    if transition.is_some() {
        settle_model9_after_damage(entity, world);
    }
    if matches!(transition, Some(Model9DamageTransition::Reaction { .. })) {
        let target = dynamic_targets
            .iter()
            .copied()
            .find(|target| target.source_slot == damage_owner)
            .or_else(|| {
                entities
                    .get(usize::from(damage_owner))
                    .filter(|target| target.active && target.index == damage_owner)
                    .map(|target| Model9DamageTarget {
                        source_slot: target.index,
                        source_id: target.source_id,
                        class: target.class,
                        model: target.model,
                    })
            })
            .ok_or(Model9Action79Error::InvalidDamageTarget {
                source_slot: entity.index,
                target_slot: damage_owner,
            })?;
        apply_model9_reaction_pose(entity, target, reaction_pose);
    }
    Ok(transition)
}

impl std::error::Error for Model9Action79Error {}

impl From<CampaignSharedFamilyError> for Model9Action79Error {
    fn from(value: CampaignSharedFamilyError) -> Self {
        Self::Selector(value)
    }
}

fn model9_retry_headings(yaw: i16) -> [i16; 4] {
    let yaw = i32::from(yaw);
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & 0x07ff;
    let fourth = ((((yaw + 0x400) as u16) & 0x0700) | (yaw as u8 as u16)) & 0x07ff;
    [
        i16::try_from(yaw & 0x07ff).expect("11-bit model-9 heading"),
        i16::try_from((yaw + 341) & 0x07ff).expect("11-bit model-9 heading"),
        i16::try_from(third).expect("11-bit model-9 heading"),
        i16::try_from(fourth).expect("11-bit model-9 heading"),
    ]
}

fn model9_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
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

fn model9_candidate_blocked<W: FlightWorld>(
    entity: &EntitySnapshot,
    candidate: Position,
    world: &W,
) -> bool {
    let material_blocked = world.material_mask(candidate) & !MODEL9_MATERIAL_MASK != 0;
    let cave_ceiling_blocked = world.ceiling_height(candidate).is_some_and(|ceiling| {
        i32::from(world.ground_height(candidate)) + i32::from(entity.array_fov())
            > i32::from(ceiling)
    });
    material_blocked
        || world.cave_collision_blocked(candidate)
        || cave_ceiling_blocked
        || model9_slope_measure(candidate, world) >= MODEL9_SLOPE_LIMIT
}

pub(super) fn step_model9_source_movement<W: FlightWorld>(
    entity: &mut EntitySnapshot,
    world: &W,
) -> std::result::Result<(bool, u8, bool), Model9Action79Error> {
    if entity.control_flags & MODEL9_SKIP_MOVEMENT_BIT != 0 {
        entity.set_control_flags(entity.control_flags & !MODEL9_SKIP_MOVEMENT_BIT);
        return Ok((false, 0, false));
    }

    let before = entity.position;
    let headings = model9_retry_headings(entity.yaw);
    for (attempt, heading) in headings.into_iter().enumerate() {
        entity.set_yaw(heading);
        let mut candidate = before;
        let ground_height = world.ground_height(candidate);
        if candidate.z > ground_height {
            candidate.z = candidate.z.wrapping_sub(128);
        }
        if candidate.z <= ground_height {
            candidate.z = ground_height;
        }
        crate::move_entity(&mut candidate, heading, 0, entity.actual_speed);
        let crossed_tile = before.x >> 8 != candidate.x >> 8 || before.y >> 8 != candidate.y >> 8;
        if (attempt == 0 && !crossed_tile) || !model9_candidate_blocked(entity, candidate, world) {
            entity.set_position(candidate);
            let yaw = (i32::from(heading)
                + i32::from(crate::turn_toward(
                    heading,
                    entity.source_roll(),
                    MODEL9_TURN_LIMIT,
                )))
                & 0x07ff;
            entity.set_yaw(i16::try_from(yaw).expect("11-bit model-9 heading"));
            if attempt == 0 {
                entity.set_control_flags(entity.control_flags & !MODEL9_BLOCKED_BIT);
            }
            return Ok((
                true,
                u8::try_from(attempt + 1).expect("four movement attempts"),
                false,
            ));
        }
        if attempt == 0 {
            entity.set_control_flags(entity.control_flags | MODEL9_BLOCKED_BIT);
        }
    }

    entity.set_life(-1);
    // Descriptor80 has the fatal-blockage bit. sub_1B8C0 decrements the
    // population and returns normally; the next callback sees negative life.
    Ok((false, 4, true))
}

/// Current fields at a frozen model-list address, without reapplying the
/// eligibility rules that were used when the list was built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Model9Peer {
    pub source_slot: u16,
    pub source_id: u16,
    pub position: Position,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) struct Model9PointStageVisit {
    pub before: EntitySnapshot,
    pub after: EntitySnapshot,
    pub stage_target: Position,
    pub frozen_model9_slots: Vec<u16>,
    pub visited_model9_slots: Vec<u16>,
    pub peers: Vec<Model9Peer>,
    pub outcome: Model9Action79Outcome,
}

#[cfg(test)]
std::thread_local! {
    static MODEL9_POINT_STAGE_VISITS:
        std::cell::RefCell<Option<Vec<Model9PointStageVisit>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn start_model9_point_stage_trace() {
    MODEL9_POINT_STAGE_VISITS.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(super) fn take_model9_point_stage_trace() -> Vec<Model9PointStageVisit> {
    MODEL9_POINT_STAGE_VISITS.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
fn record_model9_point_stage_visit(
    before: Option<EntitySnapshot>,
    after: &EntitySnapshot,
    stage_target: Position,
    frozen_model9_slots: &[u16],
    visited_model9_slots: &[u16],
    peers: &[Model9Peer],
    outcome: Model9Action79Outcome,
) {
    let Some(before) = before else { return };
    MODEL9_POINT_STAGE_VISITS.with(|trace| {
        if let Some(visits) = trace.borrow_mut().as_mut()
            && visits.len() < 64
        {
            visits.push(Model9PointStageVisit {
                before,
                after: after.clone(),
                stage_target,
                frozen_model9_slots: frozen_model9_slots.to_vec(),
                visited_model9_slots: visited_model9_slots.to_vec(),
                peers: peers.to_vec(),
                outcome,
            });
        }
    });
}

/// Port the ordinary StageVar-kind-1 path of remc2
/// `sub_20FC0 -> sub_1D5D0(a2 = 72) -> sub_1DDA0`.
///
/// `entities` is the immutable callback-start snapshot for the source child
/// chain. After damage-free movement and the phase gate, peer avoidance reads
/// current occupants lazily in frozen pass-start model order, stopping at the
/// first overlap. The numeric
/// scheduler still owns the phase epilogue. Damage
/// enters the real reaction/pre-kill selector before movement. The caller
/// applies the population write returned by fatal terrain blockage.
pub(super) fn step_model9_action79<W: FlightWorld>(
    entity: &mut EntitySnapshot,
    world: &W,
    stage_target: Position,
    entities: &[EntitySnapshot],
    frozen_model9_slots: &[u16],
    mut peer_at: impl FnMut(u16) -> std::result::Result<Option<Model9Peer>, String>,
    dynamic_targets: &[Model9DamageTarget],
    reaction_pose: ParticlePose,
) -> std::result::Result<Model9Action79Outcome, Model9Action79Error> {
    require_selector(
        entity,
        (MODEL9_CLASS, MODEL9_MODEL, MODEL9_ACTION_RAW_MOVEMENT),
    )?;
    #[cfg(test)]
    let before =
        MODEL9_POINT_STAGE_VISITS.with(|trace| trace.borrow().as_ref().map(|_| entity.clone()));

    if let Some(damage_transition) =
        consume_model9_damage(entity, world, entities, dynamic_targets, reaction_pose)?
    {
        let outcome = Model9Action79Outcome {
            damage_transition: Some(damage_transition),
            ..Model9Action79Outcome::default()
        };
        #[cfg(test)]
        record_model9_point_stage_visit(
            before,
            entity,
            stage_target,
            frozen_model9_slots,
            &[],
            &[],
            outcome,
        );
        return Ok(outcome);
    }

    let (moved, movement_attempts, population_decrement) =
        step_model9_source_movement(entity, world)?;
    let mut outcome = Model9Action79Outcome {
        population_decrement,
        moved,
        movement_attempts,
        ..Model9Action79Outcome::default()
    };
    if entity.phase & 7 != 0 {
        #[cfg(test)]
        record_model9_point_stage_visit(
            before,
            entity,
            stage_target,
            frozen_model9_slots,
            &[],
            &[],
            outcome,
        );
        return Ok(outcome);
    }

    if entity.control_flags & MODEL9_BLOCKED_BIT == 0 {
        let mut roll = i16::try_from(crate::heading_for_positions(entity.position, stage_target))
            .expect("11-bit model-9 target heading");
        entity.set_source_roll(roll);
        outcome.steered = true;
        if entity.phase & 0x3f == 0 {
            let first = next_source_rng16(entity.entity_rng);
            let second = next_source_rng16(first);
            entity.set_entity_rng(second);
            let sign = 2 * (i32::from(first % 0x9d) / 79) - 1;
            roll =
                i16::try_from((i32::from(roll) + sign * (i32::from(second & 0xff) + 85)) & 0x07ff)
                    .expect("11-bit model-9 jitter heading");
            entity.set_source_roll(roll);
            outcome.rng_draws = 2;
        }
    }

    let peer_radius = i32::from(entity.array_pitch());
    #[cfg(test)]
    let mut visited_model9_slots = Vec::new();
    #[cfg(test)]
    let mut visited_peers = Vec::new();
    for &peer_slot in frozen_model9_slots {
        #[cfg(test)]
        if before.is_some() {
            visited_model9_slots.push(peer_slot);
        }
        let Some(peer) = peer_at(peer_slot).map_err(|reason| Model9Action79Error::PeerRead {
            source_slot: entity.index,
            peer_slot,
            reason,
            completed: outcome,
        })?
        else {
            continue;
        };
        #[cfg(test)]
        if before.is_some() {
            visited_peers.push(peer);
        }
        if peer.source_id == entity.source_id {
            continue;
        }
        let delta_x = (i32::from(entity.position.x.cast_signed())
            - i32::from(peer.position.x.cast_signed()))
        .abs();
        let delta_y = (i32::from(entity.position.y.cast_signed())
            - i32::from(peer.position.y.cast_signed()))
        .abs();
        if delta_x < peer_radius && delta_y < peer_radius {
            let roll = i16::try_from(crate::heading_for_positions(peer.position, entity.position))
                .expect("11-bit model-9 peer heading");
            entity.set_source_roll(roll);
            outcome.peer_avoidance_slot = Some(peer_slot);
            break;
        }
    }
    #[cfg(test)]
    record_model9_point_stage_visit(
        before,
        entity,
        stage_target,
        frozen_model9_slots,
        &visited_model9_slots,
        &visited_peers,
        outcome,
    );
    Ok(outcome)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Model9ScanTarget {
    pub identity: Model9DamageTarget,
    pub position: Position,
    pub half_width: i16,
    pub control_flags: u32,
}

/// `sub_203D0`: the ordinary post-constructor movement and acquisition pass.
/// Target rows are observations in frozen class-list order, never new authority.
pub(super) fn step_model9_action73<W: FlightWorld>(
    entity: &mut EntitySnapshot,
    world: &W,
    entities: &[EntitySnapshot],
    dynamic_targets: &[Model9DamageTarget],
    movement_pose: ParticlePose,
    reaction_pose: ParticlePose,
    scan_period: u16,
    awareness_radius: i32,
    sight_angle: u16,
    targets: &[Model9ScanTarget],
) -> std::result::Result<Model9ReactionOutcome, Model9Action79Error> {
    require_selector(entity, (5, 9, 73))?;
    let timer = entity.source_dword_10();
    if timer > 0 {
        entity.set_source_dword_10(timer - 1);
        if timer == 1 {
            entity.particle_parameter_index = Some(MC2_SKELETON_ASSETS.movement_particle);
            entity.record[94..96]
                .copy_from_slice(&MC2_SKELETON_ASSETS.movement_particle.to_le_bytes());
            entity.record[96] = 0;
            entity.record[97] = movement_pose.animation_limit;
            entity.record[86..88].copy_from_slice(&movement_pose.array_yaw.to_le_bytes());
            entity.record[88..90].copy_from_slice(&movement_pose.array_pitch.to_le_bytes());
            entity.record[90..92].copy_from_slice(&movement_pose.array_roll.to_le_bytes());
            entity.record[92..94].copy_from_slice(&movement_pose.array_fov.to_le_bytes());
            entity.level_gate = 1;
            entity.record[74] = 1;
        }
    }
    if entity.record[74] != 0 {
        if entity.record[74] != 1 {
            return Ok(Model9ReactionOutcome::default());
        }
        let candidates = targets
            .iter()
            .filter(|target| {
                target.identity.class == 5
                    && target.identity.model
                        == [4, 12, 13][usize::from((u16::from(entity.phase) / scan_period) % 3)]
            })
            .map(|target| Model9ReactionCandidate {
                source_slot: target.identity.source_slot,
                position: target.position,
            })
            .collect::<Vec<_>>();
        // The source invokes sub_20940 without changing the outer action.
        return step_model9_action74(
            entity,
            world,
            entities,
            dynamic_targets,
            movement_pose,
            reaction_pose,
            scan_period,
            awareness_radius,
            &candidates,
        );
    }
    if entity.record[61] != 0 {
        entity.set_source_dword_10(400);
    }
    if consume_model9_damage(entity, world, entities, dynamic_targets, reaction_pose)?.is_some() {
        return Ok(Model9ReactionOutcome::default());
    }
    let (_, _, population_decrement) = step_model9_source_movement(entity, world)?;
    if u16::from(entity.phase) % scan_period != 0 {
        return Ok(Model9ReactionOutcome {
            population_decrement,
            ..Default::default()
        });
    }
    let distance2 = |position: Position| {
        let dx = i32::from(position.x.wrapping_sub(entity.position.x) as i16);
        let dy = i32::from(position.y.wrapping_sub(entity.position.y) as i16);
        dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
    };
    let nearest = |predicate: &dyn Fn(&Model9ScanTarget) -> bool| {
        let mut selected = None;
        let mut best = u32::MAX;
        for target in targets {
            let squared = distance2(target.position);
            if predicate(target) && squared < best {
                best = squared;
                selected = Some(*target);
            }
        }
        selected
    };
    let opponent = nearest(&|target| {
        target.identity.class == 3
            && target.identity.model == 2
            && target.identity.source_id != entity.source_id
    });
    let awareness_squared = awareness_radius.wrapping_mul(awareness_radius) as u32;
    let cone = if entity.record[61] != 0 {
        nearest(&|target| {
            let heading = crate::heading_for_positions(entity.position, target.position) as i16;
            let delta = (i32::from(heading) - i32::from(entity.yaw)) & 2047;
            target.identity.class == 3
                && target.identity.source_id != entity.source_id
                && target.control_flags & 0x20 == 0
                && distance2(target.position) <= awareness_squared
                && delta.min(2048 - delta) < i32::from(sight_angle)
        })
    } else {
        None
    };
    let model = [4, 12, 13][usize::from((u16::from(entity.phase) / scan_period) % 3)];
    let prey = nearest(&|target| {
        target.identity.class == 5
            && target.identity.model == model
            && distance2(target.position) <= awareness_squared
    });
    let distance3 = |position: Position| {
        let dz = i32::from(position.z.wrapping_sub(entity.position.z));
        distance2(position).wrapping_add(dz.wrapping_mul(dz) as u32)
    };
    let opponent_in_reach = opponent.filter(|target| {
        let reach = awareness_radius.wrapping_add(i32::from(target.half_width)) as u32;
        u64::from(distance3(target.position)) < u64::from(reach + 1).pow(2)
    });
    let prey_in_reach = prey.filter(|target| distance3(target.position) < 1537_u32.pow(2));
    if let Some(opponent) = opponent {
        entity.set_source_roll(
            crate::heading_for_positions(entity.position, opponent.position) as i16,
        );
    } else {
        let first = next_source_rng16(entity.entity_rng);
        let second = next_source_rng16(first);
        entity.set_entity_rng(second);
        let sign = 2 * (i32::from(first % 157) / 79) - 1;
        entity.set_source_roll(
            ((i32::from(entity.source_roll()) + sign * (i32::from(second & 255) + 85)) & 2047)
                as i16,
        );
    }
    if let Some(target) = opponent_in_reach.or(cone) {
        entity.record[154..156].copy_from_slice(&target.identity.source_slot.to_le_bytes());
        entity.set_source_action(74);
        apply_model9_reaction_pose(entity, target.identity, reaction_pose);
        return Ok(Model9ReactionOutcome {
            population_decrement,
            ..Default::default()
        });
    }
    Ok(Model9ReactionOutcome {
        population_decrement,
        prey_to_consume: prey_in_reach.map(|target| target.identity.source_slot),
        inherit_identity: false,
    })
}

/// Port remc2 `sub_20940` through its frozen model-list selection. The caller
/// commits the hide/allocation effect through the shared source arena.
pub(super) fn step_model9_action74<W: FlightWorld>(
    entity: &mut EntitySnapshot,
    world: &W,
    entities: &[EntitySnapshot],
    dynamic_targets: &[Model9DamageTarget],
    movement_pose: ParticlePose,
    reaction_pose: ParticlePose,
    scan_period: u16,
    awareness_radius: i32,
    candidates: &[Model9ReactionCandidate],
) -> std::result::Result<Model9ReactionOutcome, Model9Action79Error> {
    let callback_action = if entity.action == 73 && entity.record[74] == 1 {
        73
    } else {
        MODEL9_ACTION_REACTION
    };
    require_selector(entity, (MODEL9_CLASS, MODEL9_MODEL, callback_action))?;
    if consume_model9_damage(entity, world, entities, dynamic_targets, reaction_pose)?.is_some() {
        return Ok(Model9ReactionOutcome::default());
    }

    let timer = entity.source_dword_10();
    if timer < 0 {
        entity.set_source_dword_10(timer.wrapping_add(1));
        if timer == -1 {
            entity.particle_parameter_index = Some(MODEL9_MOVEMENT_PARTICLE);
            entity.record[94..96].copy_from_slice(&MODEL9_MOVEMENT_PARTICLE.to_le_bytes());
            entity.record[96] = 0;
            entity.record[97] = movement_pose.animation_limit;
            entity.record[86..88].copy_from_slice(&movement_pose.array_yaw.to_le_bytes());
            entity.record[88..90].copy_from_slice(&movement_pose.array_pitch.to_le_bytes());
            entity.record[90..92].copy_from_slice(&movement_pose.array_roll.to_le_bytes());
            entity.record[92..94].copy_from_slice(&movement_pose.array_fov.to_le_bytes());
            entity.record[74] = 0;
            entity.set_source_dword_10(MODEL9_MOVEMENT_COUNTDOWN);
        }
        return Ok(Model9ReactionOutcome::default());
    }
    if entity.record[61] != 0 {
        entity.set_source_dword_10(-50);
        return Ok(Model9ReactionOutcome::default());
    }
    if scan_period == 0 || u16::from(entity.phase) % scan_period != 0 {
        return Ok(Model9ReactionOutcome::default());
    }

    let awareness_squared = awareness_radius.wrapping_mul(awareness_radius) as u32;
    let mut nearest = None;
    let mut nearest_squared = u32::MAX;
    for candidate in candidates {
        let dx = i32::from(candidate.position.x.wrapping_sub(entity.position.x) as i16);
        let dy = i32::from(candidate.position.y.wrapping_sub(entity.position.y) as i16);
        let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if squared <= awareness_squared && squared < nearest_squared {
            nearest = Some(*candidate);
            nearest_squared = squared;
        }
    }

    let prey_to_consume = nearest.and_then(|candidate| {
        let dx = i32::from(candidate.position.x.wrapping_sub(entity.position.x) as i16);
        let dy = i32::from(candidate.position.y.wrapping_sub(entity.position.y) as i16);
        let dz = i32::from(candidate.position.z.wrapping_sub(entity.position.z));
        let squared = dx
            .wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .wrapping_add(dz.wrapping_mul(dz)) as u32;
        // `sub_7277A_radix_3d(...) <= 0x600` is an integer-root check, so
        // every squared distance below 1537^2 is admitted.
        (squared < 1_537_u32.pow(2)).then_some(candidate.source_slot)
    });
    Ok(Model9ReactionOutcome {
        prey_to_consume,
        inherit_identity: true,
        population_decrement: false,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PickupPlayerCandidate {
    pub entity_index: u16,
    pub model: u8,
    pub life: i32,
    pub player_color_index: u8,
    pub overlaps: bool,
    pub holder_entity_index: u16,
    pub subspell_tier: u8,
    pub left_hand_family: i16,
    pub right_hand_family: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PickupHand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PickupAcquisition {
    pub source_slot: u16,
    pub player_entity_index: u16,
    pub family: u8,
    pub holder_action: u8,
    pub subspell_tier: u8,
    pub hand: PickupHand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PickupActionOutcome {
    pub acquisition: Option<PickupAcquisition>,
    pub drawing_disabled: bool,
    pub internal_owner_flag_set: bool,
}

/// Row-local portion of `sub_692A0 -> sub_68FF0`, shared by every ordinary
/// spell-pickup action (`3 * family + 1`).
///
/// The returned acquisition is the caller's instruction to update the player
/// holder/hand arrays, apply the already validated `SetSpell_6D5E0` profile,
/// and emit sound 18. Candidate order must be the rebuilt source player-list
/// order. The scheduler advances `phase` after this function returns.
pub(super) fn step_ordinary_pickup(
    entity: &mut EntitySnapshot,
    ground_height: i16,
    internal_level_color: u8,
    candidates: &[PickupPlayerCandidate],
) -> std::result::Result<PickupActionOutcome, CampaignSharedFamilyError> {
    let family = entity.model;
    let expected_action = family.wrapping_mul(3).wrapping_add(1);
    require_selector(entity, (PICKUP_CLASS, family, expected_action))?;
    let mut outcome = PickupActionOutcome::default();

    if entity.life != 0 {
        let previous_life = entity.life;
        entity.set_life(previous_life.wrapping_sub(1));
        if previous_life == 1 {
            entity.disable_drawing();
            outcome.drawing_disabled = true;
        }
        return Ok(outcome);
    }

    let mut position = entity.position;
    if position.z > ground_height {
        position.z = position.z.wrapping_sub(128);
    }
    if position.z <= ground_height {
        position.z = ground_height;
    }
    entity.set_position(position);
    if entity.phase & 3 != 0 {
        return Ok(outcome);
    }

    for candidate in candidates {
        if candidate.model != 0 || candidate.life < 0 {
            continue;
        }
        if candidate.player_color_index == internal_level_color
            && entity.control_flags & CONTROL_PICKUP_OWNED_BIT == 0
            && candidate.holder_entity_index != 0
        {
            entity.set_control_flags(entity.control_flags | CONTROL_PICKUP_OWNED_BIT);
            outcome.internal_owner_flag_set = true;
        }
        if !candidate.overlaps || candidate.holder_entity_index != 0 {
            continue;
        }

        entity.set_control_flags(
            (entity.control_flags | CONTROL_PICKUP_OWNED_BIT) & !CONTROL_PICKUP_BYTE3_BIT1,
        );
        entity.record[44..46].copy_from_slice(&candidate.entity_index.to_le_bytes());
        let holder_action = expected_action.wrapping_sub(1);
        entity.set_source_action(holder_action);
        entity.record[58..60].copy_from_slice(&64_u16.to_le_bytes());
        let explicit_hand = u16::from_le_bytes([entity.record[78], entity.record[79]]);
        let hand = if explicit_hand != 0 {
            entity.record[78..80].fill(0);
            if explicit_hand == 2 {
                PickupHand::Left
            } else {
                PickupHand::Right
            }
        } else if candidate.left_hand_family == -1 || candidate.right_hand_family != -1 {
            PickupHand::Left
        } else {
            PickupHand::Right
        };
        outcome.acquisition = Some(PickupAcquisition {
            source_slot: entity.index,
            player_entity_index: candidate.entity_index,
            family,
            holder_action,
            subspell_tier: candidate.subspell_tier,
            hand,
        });
        break;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tile;

    #[derive(Clone, Copy, Debug, Default)]
    struct Model9World;

    impl FlightWorld for Model9World {
        fn ground_height(&self, _position: Position) -> i16 {
            256
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

    fn entity(class: u8, model: u8, action: u8) -> EntitySnapshot {
        let mut record = vec![0_u8; 176].into_boxed_slice();
        record[2] = 1;
        record[8..12].copy_from_slice(&300_i32.to_le_bytes());
        record[12..16].copy_from_slice(&300_i32.to_le_bytes());
        record[16..20].copy_from_slice(&CONTROL_MAP_LINKED_BIT.to_le_bytes());
        record[24..26].copy_from_slice(&0x1234_u16.to_le_bytes());
        record[30..32].copy_from_slice(&77_u16.to_le_bytes());
        record[38..40].copy_from_slice(&400_i16.to_le_bytes());
        record[66] = 4;
        record[67] = class;
        record[68] = model;
        record[73] = action;
        record[80..82].copy_from_slice(&0x1200_u16.to_le_bytes());
        record[82..84].copy_from_slice(&0x3400_u16.to_le_bytes());
        record[84..86].copy_from_slice(&512_i16.to_le_bytes());
        record[136..138].copy_from_slice(&20_i16.to_le_bytes());
        EntitySnapshot {
            index: 93,
            source_id: 77,
            authority: EntityAuthority::FrozenObserved,
            active: true,
            max_life: 300,
            life: 300,
            next_relation: None,
            class,
            model,
            control_flags: CONTROL_MAP_LINKED_BIT,
            entity_rng: 0x1234,
            action,
            action_marker: 0,
            level_gate: 0,
            position: Position {
                x: 0x1200,
                y: 0x3400,
                z: 512,
            },
            yaw: 0,
            pitch: 0,
            fov: 400,
            phase: 4,
            subspell_index: 0,
            byte_3b: 0,
            byte_3c: 0,
            font_type_index: 0,
            active_value: 0,
            activation_value: 0,
            mana_regen: 0,
            actual_speed: 0,
            max_mana: 0,
            mana_cost: 0,
            mana: 0,
            particle_parameter_index: None,
            parameter_relation: None,
            special_relation: EntitySpecialRelation::None,
            record,
        }
    }

    #[test]
    fn tree_allocation_success_is_the_only_death_rng_path() {
        let mut failed = entity(TREE_CLASS, TREE_MODEL, TREE_ACTION);
        failed.record[98..102].copy_from_slice(&301_i32.to_le_bytes());
        failed.record[102..104].copy_from_slice(&12_u16.to_le_bytes());
        let failed_before_rng = failed.entity_rng;
        let outcome = step_tree_action0(&mut failed, 64, false, |_| None).unwrap();
        assert_eq!(outcome.rng_draws, 0);
        assert_eq!(failed.entity_rng, failed_before_rng);
        assert_eq!(failed.life, -1);
        assert_eq!(failed.action, TREE_ACTION);
        assert_eq!(failed.source_upkeep_word_62(), 0);

        let mut succeeded = entity(TREE_CLASS, TREE_MODEL, TREE_ACTION);
        succeeded.record[98..102].copy_from_slice(&301_i32.to_le_bytes());
        succeeded.record[102..104].copy_from_slice(&12_u16.to_le_bytes());
        let expected_rng = next_source_rng16(succeeded.entity_rng);
        let mut request = None;
        let outcome = step_tree_action0(&mut succeeded, 64, false, |value| {
            request = Some(value);
            Some(700)
        })
        .unwrap();
        assert_eq!(outcome.death_sphere_slot, Some(700));
        assert_eq!(outcome.rng_draws, 1);
        assert_eq!(succeeded.entity_rng, expected_rng);
        assert_eq!(succeeded.life, i32::from(expected_rng % 60) + 130);
        assert_eq!(succeeded.action, 1);
        assert_eq!(request.unwrap().word_0x2c, 300);
    }

    #[test]
    fn stone_action3_sets_callback_bit_and_snaps_without_hiding() {
        let mut stone = entity(STONE_CLASS, STONE_MODEL, STONE_ACTION);
        let outcome = step_stone_action3(&mut stone, 96).unwrap();
        assert_eq!(stone.position.z, 96);
        assert_eq!(outcome.previous_position.z, 512);
        assert_eq!(outcome.position.z, 96);
        assert_eq!(outcome.previous_control_flags, CONTROL_MAP_LINKED_BIT);
        assert_eq!(
            outcome.control_flags,
            CONTROL_MAP_LINKED_BIT | CONTROL_TREE_CALLBACK_BIT
        );
        assert!(!stone.drawing_disabled());
    }

    #[test]
    fn skeleton_constructor_uses_typed_defaults_and_source_mana_order() {
        let properties = crate::level_definition::SKELETON_CLASS_PROPERTIES;
        let pose = ParticlePose {
            animation_limit: 9,
            array_yaw: 101,
            array_pitch: 102,
            array_roll: 103,
            array_fov: 104,
        };
        let mut row = construct_skeleton_source(7, 0x1357, Position::default(), 0, 30, pose);
        assert_eq!(
            (row.max_life, row.life),
            (properties.max_life, properties.max_life)
        );
        assert_eq!(row.mana, properties.max_life >> 1);
        assert_eq!(row.actual_speed, properties.minimum_speed);
        assert_eq!(row.subspell_index, properties.initial_subspell);
        assert_eq!(row.parameter_relation, Some(MC2_SKELETON_ASSETS.descriptor));
        assert_eq!(
            row.particle_parameter_index,
            Some(MC2_SKELETON_ASSETS.initial_particle)
        );
        assert_eq!(row.entity_rng, next_source_rng16(0x1357));
        // Authored semantic health is bound after the source constructor;
        // SetEvent144 is not called a second time by that later binding.
        row.bind_frame_zero_max_life(1234);
        assert_eq!(
            (row.max_life, row.life, row.mana),
            (1234, 1234, properties.max_life >> 1)
        );
    }

    #[test]
    fn model9_uses_predecrement_transition_and_even_animation_cadence() {
        let pose = ParticlePose {
            animation_limit: 9,
            array_yaw: 101,
            array_pitch: 102,
            array_roll: 103,
            array_fov: 104,
        };
        let mut countdown = entity(MODEL9_CLASS, MODEL9_MODEL, MODEL9_ACTION_COUNTDOWN);
        countdown.set_source_dword_10(15);
        countdown.record[96] = 2;
        countdown.record[97] = 9;
        let outcome = step_model9_action72(&mut countdown, pose).unwrap();
        assert!(outcome.animation_advanced);
        assert_eq!(countdown.record[96], 3);
        assert_eq!(countdown.source_dword_10(), 14);
        assert_eq!(countdown.action, MODEL9_ACTION_COUNTDOWN);

        countdown.set_source_dword_10(0);
        countdown.record[78..80].copy_from_slice(&6_u16.to_le_bytes());
        let outcome = step_model9_action72(&mut countdown, pose).unwrap();
        assert!(outcome.transitioned);
        assert_eq!(outcome.stage_relation_to_advance, Some(6));
        assert_eq!(countdown.action, MODEL9_ACTION_MOVEMENT);
        assert_eq!(countdown.source_dword_10(), MODEL9_MOVEMENT_COUNTDOWN);
        assert_eq!(countdown.actual_speed, 20);
        assert_eq!(countdown.particle_parameter_index, Some(201));
        assert_eq!(countdown.record[69..71], [3, 0xff]);
        assert_eq!(particle_animation_limit(20), Some(1));
        assert_eq!(particle_animation_limit(37), Some(0));
        assert_eq!(particle_animation_limit(38), None);
    }

    #[test]
    fn skeleton_blocked_movement_returns_population_effect_then_pre_kill() {
        struct Blocked;
        impl FlightWorld for Blocked {
            fn ground_height(&self, _: Position) -> i16 {
                256
            }
            fn ceiling_height(&self, _: Position) -> Option<i16> {
                None
            }
            fn material_mask(&self, _: Position) -> u32 {
                u32::MAX
            }
            fn angle_flags(&self, _: Tile) -> u8 {
                0
            }
            fn cave_collision_blocked(&self, _: Position) -> bool {
                true
            }
        }
        let pose = ParticlePose {
            animation_limit: 1,
            array_yaw: 0,
            array_pitch: 128,
            array_roll: 128,
            array_fov: 128,
        };
        let mut source = entity(5, 9, 79);
        source.set_position(Position {
            x: 0x12ff,
            y: 0x34ff,
            z: 256,
        });
        source.set_actual_speed(512);
        let phase = source.phase;
        let result = step_model9_action79(
            &mut source,
            &Blocked,
            Position::default(),
            &[],
            &[],
            |_| Ok(None),
            &[],
            pose,
        )
        .unwrap();
        assert_eq!(
            (
                result.moved,
                result.movement_attempts,
                result.population_decrement
            ),
            (false, 4, true)
        );
        assert_eq!((source.life, source.action, source.phase), (-1, 79, phase));
        let result = step_model9_action79(
            &mut source,
            &Blocked,
            Position::default(),
            &[],
            &[],
            |_| Ok(None),
            &[],
            pose,
        )
        .unwrap();
        assert_eq!(source.action, 76);
        assert!(!result.population_decrement);
    }

    #[test]
    fn skeleton_constructor_enters_player_scan_without_confusing_bees_with_players() {
        let pose = ParticlePose {
            animation_limit: 9,
            array_yaw: 62,
            array_pitch: 74,
            array_roll: 74,
            array_fov: 128,
        };
        let position = Position {
            x: 8000,
            y: 8000,
            z: 256,
        };
        let mut source = construct_skeleton_source(7, 0x1357, position, 0, 30, pose);
        let rng = 0x1357_u16.wrapping_mul(9377).wrapping_add(9439);
        let angle = (rng % 0x832) as i16 - 1;
        assert_eq!(
            (
                source.entity_rng,
                source.yaw,
                source.pitch,
                source.source_roll(),
                source.fov
            ),
            (rng, angle, angle, angle, 0)
        );
        assert_eq!(
            (
                source.life,
                source.mana,
                source.control_flags,
                source.record[61]
            ),
            (1000, 500, 12, 34)
        );
        assert_eq!((source.array_pitch(), source.array_fov()), (128, 128));
        for _ in 0..17 {
            step_model9_action72(&mut source, pose).unwrap();
        }
        assert_eq!((source.action, source.source_dword_10()), (73, 400));
        let target = Model9ScanTarget {
            identity: Model9DamageTarget {
                source_slot: 8,
                source_id: 8,
                class: 5,
                model: 2,
            },
            position: Position {
                x: 8100,
                ..position
            },
            half_width: 128,
            control_flags: 8,
        };
        let mut negative = source.clone();
        let outcome = step_model9_action73(
            &mut negative,
            &Model9World,
            &[],
            &[],
            pose,
            pose,
            30,
            2000,
            1024,
            &[target],
        )
        .unwrap();
        assert_eq!(
            negative.action, 73,
            "a model-2 Bee is not a class-3 opponent"
        );
        assert_eq!(outcome.prey_to_consume, None);
        assert_ne!(negative.position, position);
        let player_target = Model9ScanTarget {
            identity: Model9DamageTarget {
                class: 3,
                ..target.identity
            },
            ..target
        };
        step_model9_action73(
            &mut source,
            &Model9World,
            &[],
            &[],
            pose,
            pose,
            30,
            2000,
            1024,
            &[player_target],
        )
        .unwrap();
        assert_eq!(
            (
                source.action,
                source.actual_speed,
                source.record[69],
                source.record[70]
            ),
            (74, 0, 3, 2)
        );
        assert_eq!(
            u16::from_le_bytes([source.record[154], source.record[155]]),
            8
        );
    }

    #[test]
    fn model9_action79_moves_then_peer_overwrites_jitter_without_phase_or_global_rng() {
        let mut source = entity(MODEL9_CLASS, MODEL9_MODEL, MODEL9_ACTION_RAW_MOVEMENT);
        source.set_phase(0);
        source.set_actual_speed(source.minimum_speed());
        source.record[88..92].copy_from_slice(&74_i16.to_le_bytes().repeat(2));
        source.set_source_roll(0);
        let before = source.position;
        let before_phase = source.phase;
        let first_rng = next_source_rng16(source.entity_rng);
        let second_rng = next_source_rng16(first_rng);
        let mut moved = before;
        moved.z = moved.z.wrapping_sub(128);
        crate::move_entity(&mut moved, source.yaw, 0, source.actual_speed);

        let mut peer = entity(MODEL9_CLASS, MODEL9_MODEL, MODEL9_ACTION_RAW_MOVEMENT);
        peer.index = 94;
        peer.source_id = 78;
        peer.set_position(Position {
            x: moved.x.wrapping_add(1),
            y: moved.y.wrapping_add(1),
            z: moved.z,
        });
        let peer_index = peer.index;
        let expected_peer_roll = i16::try_from(crate::heading_for_positions(peer.position, moved))
            .expect("11-bit peer heading");
        let mut entities = vec![entity(0, 0, 0); 95];
        entities[usize::from(source.index)] = source.clone();
        entities[usize::from(peer_index)] = peer;
        let mut attacker = entity(3, 0, 0);
        attacker.index = 44;
        attacker.source_id = 44;
        entities[44] = attacker;

        let frozen_model9_slots = [source.index, peer_index];
        let peers = frozen_model9_slots.map(|source_slot| {
            let peer = &entities[usize::from(source_slot)];
            Model9Peer {
                source_slot,
                source_id: peer.source_id,
                position: peer.position,
            }
        });

        let outcome = step_model9_action79(
            &mut source,
            &Model9World,
            Position {
                x: moved.x.wrapping_add(1_000),
                y: moved.y,
                z: moved.z,
            },
            &entities,
            &frozen_model9_slots,
            |slot| Ok(peers.iter().copied().find(|peer| peer.source_slot == slot)),
            &[],
            ParticlePose {
                animation_limit: 1,
                array_yaw: 0,
                array_pitch: 0,
                array_roll: 0,
                array_fov: 0,
            },
        )
        .unwrap();
        assert_eq!(source.position, moved);
        assert_eq!(source.phase, before_phase);
        assert_eq!(source.entity_rng, second_rng);
        assert_eq!(source.source_roll(), expected_peer_roll);
        assert_eq!(
            outcome,
            Model9Action79Outcome {
                population_decrement: false,
                moved: true,
                movement_attempts: 1,
                steered: true,
                rng_draws: 2,
                peer_avoidance_slot: Some(94),
                damage_transition: None,
            }
        );

        let mut damaged = entities[usize::from(source.index)].clone();
        damaged.record[98..102].copy_from_slice(&7_i32.to_le_bytes());
        damaged.record[102..104].copy_from_slice(&44_u16.to_le_bytes());
        let outcome = step_model9_action79(
            &mut damaged,
            &Model9World,
            Position { x: 0, y: 0, z: 0 },
            &entities,
            &frozen_model9_slots,
            |slot| Ok(peers.iter().copied().find(|peer| peer.source_slot == slot)),
            &[],
            ParticlePose {
                animation_limit: 1,
                array_yaw: 0,
                array_pitch: 0,
                array_roll: 0,
                array_fov: 0,
            },
        )
        .unwrap();
        assert_eq!(
            outcome.damage_transition,
            Some(Model9DamageTransition::Reaction { attacker_slot: 44 })
        );
        assert_eq!(damaged.life, 293);
        assert_eq!(damaged.source_upkeep_word_62(), 0);
        assert_eq!(damaged.word_26(), 44);
        assert_eq!(damaged.action, MODEL9_ACTION_REACTION);
    }

    #[test]
    fn configured_model9_point_peers_use_frozen_membership_and_current_fields() {
        // This configures the pass boundary explicitly. It is not an ordinary
        // newborn/alias producer: the unmodified Level3 gate covers emergence.
        let mut source = entity(5, 9, 79);
        source.set_phase(8);
        source.record[88..90].copy_from_slice(&74_i16.to_le_bytes());
        let mut entities = vec![entity(0, 0, 0); 96];
        entities[usize::from(source.index)] = source.clone();
        let target = Position {
            x: source.position.x.wrapping_add(1000),
            y: source.position.y,
            z: 0,
        };
        let pose = ParticlePose {
            animation_limit: 1,
            array_yaw: 0,
            array_pitch: 0,
            array_roll: 0,
            array_fov: 0,
        };
        let mut frozen_peer = entity(5, 9, 79);
        frozen_peer.index = 94;
        frozen_peer.source_id = 78;
        frozen_peer.set_position(Position {
            x: source.position.x.wrapping_add(500),
            y: source.position.y.wrapping_add(500),
            z: source.position.z,
        });
        entities[94] = frozen_peer;
        let frozen_model9_slots = [93, 94];

        // A current model9 outside the frozen list is nearby. The old numeric
        // scan would steer away from this address on this callback.
        let mut newborn = entity(5, 9, 79);
        newborn.index = 95;
        newborn.source_id = 79;
        newborn.set_position(Position {
            x: source.position.x.wrapping_sub(1),
            y: source.position.y.wrapping_add(1),
            z: source.position.z,
        });
        entities[95] = newborn;
        let observe = |entities: &[EntitySnapshot], source_slot| {
            let row = &entities[usize::from(source_slot)];
            Model9Peer {
                source_slot,
                source_id: row.source_id,
                position: row.position,
            }
        };
        let mut without_newborn = source.clone();
        let result = step_model9_action79(
            &mut without_newborn,
            &Model9World,
            target,
            &entities,
            &frozen_model9_slots,
            |slot| Ok(Some(observe(&entities, slot))),
            &[],
            pose,
        )
        .unwrap();
        assert!(result.moved && result.steered);
        assert_eq!(result.rng_draws, 0);
        assert_eq!(result.peer_avoidance_slot, None);
        assert_eq!(
            i32::from(without_newborn.source_roll()),
            crate::heading_for_positions(without_newborn.position, target)
        );
        assert_ne!(
            i32::from(without_newborn.source_roll()),
            crate::heading_for_positions(entities[95].position, without_newborn.position)
        );

        // A frozen member now has different current ID/pose and no longer
        // satisfies current active/class/model/life eligibility. Source1DDA0
        // still reads those fields at the retained address without filtering.
        entities[94].active = false;
        entities[94].class = 0;
        entities[94].model = 0;
        entities[94].set_life(-1);
        entities[94].source_id = 80;
        entities[94].set_position(Position {
            x: source.position.x.wrapping_add(1),
            y: source.position.y.wrapping_add(1),
            z: source.position.z,
        });
        let result = step_model9_action79(
            &mut source,
            &Model9World,
            target,
            &entities,
            &frozen_model9_slots,
            |slot| Ok(Some(observe(&entities, slot))),
            &[],
            pose,
        )
        .unwrap();
        assert_eq!(result.peer_avoidance_slot, Some(94));
        assert_eq!(result.rng_draws, 0);
        assert_eq!(
            i32::from(source.source_roll()),
            crate::heading_for_positions(entities[94].position, source.position)
        );
        assert_ne!(source.source_roll(), without_newborn.source_roll());
        assert_eq!(source.position, without_newborn.position);
        assert_eq!(source.phase, 8);
        assert_eq!(source.entity_rng, 0x1234);

        let mut same_id = entities[93].clone();
        entities[94].source_id = same_id.source_id;
        let result = step_model9_action79(
            &mut same_id,
            &Model9World,
            target,
            &entities,
            &frozen_model9_slots,
            |slot| Ok(Some(observe(&entities, slot))),
            &[],
            pose,
        )
        .unwrap();
        assert_eq!(result.peer_avoidance_slot, None);
        assert_eq!(same_id.source_roll(), without_newborn.source_roll());
    }

    #[test]
    fn configured_model9_point_peer_reader_respects_source_gates_and_first_overlap() {
        let mut base = entity(5, 9, 79);
        base.set_phase(8);
        base.record[88..90].copy_from_slice(&74_i16.to_le_bytes());
        let frozen = [93, 94, 95, 96];
        let target = Position {
            x: base.position.x.wrapping_add(1000),
            ..base.position
        };
        let pose = ParticlePose {
            animation_limit: 1,
            array_yaw: 0,
            array_pitch: 0,
            array_roll: 0,
            array_fov: 0,
        };
        let attacker = Model9DamageTarget {
            source_slot: 44,
            source_id: 44,
            class: 3,
            model: 0,
        };
        for hit in [false, true] {
            let mut source = base.clone();
            if hit {
                source.record[98..102].copy_from_slice(&7_i32.to_le_bytes());
                source.record[102..104].copy_from_slice(&44_u16.to_le_bytes());
            } else {
                source.set_phase(9);
            }
            let mut reads = Vec::new();
            start_model9_point_stage_trace();
            let result = step_model9_action79(
                &mut source,
                &Model9World,
                target,
                &[],
                &frozen,
                |slot| {
                    reads.push(slot);
                    Err("reader must stay gated".into())
                },
                &[attacker],
                pose,
            )
            .unwrap();
            assert!(reads.is_empty());
            assert_eq!(result.damage_transition.is_some(), hit);
            assert_eq!(result.moved, !hit);
            let visits = take_model9_point_stage_trace();
            assert_eq!(visits.len(), 1);
            assert_eq!(visits[0].frozen_model9_slots, frozen);
            assert!(visits[0].visited_model9_slots.is_empty());
            assert!(visits[0].peers.is_empty());
        }

        let mut source = base.clone();
        let mut reads = Vec::new();
        start_model9_point_stage_trace();
        let result = step_model9_action79(
            &mut source,
            &Model9World,
            target,
            &[],
            &frozen,
            |slot| {
                reads.push(slot);
                match slot {
                    93 => Ok(Some(Model9Peer {
                        source_slot: slot,
                        source_id: base.source_id,
                        position: base.position,
                    })),
                    94 => Ok(None),
                    95 => Ok(Some(Model9Peer {
                        source_slot: slot,
                        source_id: 78,
                        position: Position {
                            x: base.position.x.wrapping_add(1),
                            y: base.position.y.wrapping_add(1),
                            ..base.position
                        },
                    })),
                    _ => Err("unvisited suffix reader error".into()),
                }
            },
            &[],
            pose,
        )
        .unwrap();
        assert_eq!(result.peer_avoidance_slot, Some(95));
        assert_eq!(reads, [93, 94, 95]);
        let visits = take_model9_point_stage_trace();
        assert_eq!(visits.len(), 1);
        assert_eq!(visits[0].visited_model9_slots, reads);
        assert_eq!(
            visits[0]
                .peers
                .iter()
                .map(|peer| peer.source_slot)
                .collect::<Vec<_>>(),
            [93, 95]
        );

        let mut source = base.clone();
        let mut reads = Vec::new();
        let error = step_model9_action79(
            &mut source,
            &Model9World,
            target,
            &[],
            &frozen,
            |slot| {
                reads.push(slot);
                if slot == 93 {
                    Ok(None)
                } else {
                    Err("current carrier error".into())
                }
            },
            &[],
            pose,
        )
        .unwrap_err();
        assert_eq!(reads, [93, 94]);
        assert_eq!(
            error,
            Model9Action79Error::PeerRead {
                source_slot: 93,
                peer_slot: 94,
                reason: "current carrier error".into(),
                completed: Model9Action79Outcome {
                    moved: true,
                    movement_attempts: 1,
                    steered: true,
                    ..Model9Action79Outcome::default()
                },
            }
        );
        assert_ne!(source.position, base.position);
        assert_eq!(
            i32::from(source.source_roll()),
            crate::heading_for_positions(source.position, target)
        );
        assert_eq!((source.phase, source.entity_rng), (8, base.entity_rng));
    }

    #[test]
    fn ordinary_pickup_carries_source_order_owner_and_hand_rules() {
        let mut pickup = entity(PICKUP_CLASS, 6, 19);
        pickup.set_life(0);
        pickup.record[78..80].copy_from_slice(&2_u16.to_le_bytes());
        let candidates = [
            PickupPlayerCandidate {
                entity_index: 10,
                model: 0,
                life: 100,
                player_color_index: 3,
                overlaps: false,
                holder_entity_index: 88,
                subspell_tier: 1,
                left_hand_family: -1,
                right_hand_family: -1,
            },
            PickupPlayerCandidate {
                entity_index: 11,
                model: 0,
                life: 100,
                player_color_index: 1,
                overlaps: true,
                holder_entity_index: 0,
                subspell_tier: 2,
                left_hand_family: 4,
                right_hand_family: -1,
            },
        ];
        let outcome = step_ordinary_pickup(&mut pickup, 128, 3, &candidates).unwrap();
        assert!(outcome.internal_owner_flag_set);
        assert_eq!(pickup.position.z, 384);
        assert_eq!(pickup.action, 18);
        assert_eq!(
            u16::from_le_bytes([pickup.record[44], pickup.record[45]]),
            11
        );
        assert_eq!(
            u16::from_le_bytes([pickup.record[58], pickup.record[59]]),
            64
        );
        assert_eq!(
            u16::from_le_bytes([pickup.record[78], pickup.record[79]]),
            0
        );
        assert_eq!(outcome.acquisition.unwrap().hand, PickupHand::Left);

        let mut expiring = entity(PICKUP_CLASS, 11, 34);
        expiring.set_life(1);
        let outcome = step_ordinary_pickup(&mut expiring, 0, 0, &[]).unwrap();
        assert!(outcome.drawing_disabled);
        assert_eq!(expiring.life, 0);
        assert_eq!(expiring.position.z, 512);
    }
}
