//! Source-ordered Level 1 townie/villager simulation.
//!
//! This is the bounded class-5/model-13 path used by Level 1: initial action
//! 104, free-townie aliases 105-107, death actions 108-109, hit action 110,
//! and staged action 111.

use crate::{
    FlightWorld, Position, heading_for_positions,
    house::{HouseFamily, HouseResidentEntry},
    level1_mission::{Level1SourceEntityView, SOURCE_DISABLED_FLAG},
    move_entity,
    native_world::EntitySnapshot,
    turn_toward, wrapping_i16,
};

pub const LEVEL_ONE_VILLAGER_INDICES: [u16; 28] = [
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
    75, 76, 77, 78,
];
/// The Level 2 Day reset contains ten early ordinary villagers, one
/// feature-owned slot at 87, then nineteen more villagers.  The gap is a
/// deliberate source-order hole occupied by a dormant class-11 row.
pub const LEVEL_TWO_VILLAGER_INDICES: [u16; 29] = [
    77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100,
    101, 102, 103, 104, 105, 106,
];
pub const VILLAGER_CLASS: u8 = 5;
pub const VILLAGER_MODEL: u8 = 13;
pub const VILLAGER_INITIAL_ACTION: u8 = 104;
pub const VILLAGER_STAGED_ACTION: u8 = 111;
pub const VILLAGER_FREE_ACTION: u8 = 105;
pub const VILLAGER_FREE_ALIAS_ACTION_A: u8 = 106;
pub const VILLAGER_FREE_ALIAS_ACTION_B: u8 = 107;
pub const VILLAGER_HIT_ACTION: u8 = 110;
pub const VILLAGER_DEATH_ACTION: u8 = 108;
pub const VILLAGER_REMOVAL_ACTION: u8 = 109;
pub const VILLAGER_PARAMETER_INDEX: u16 = 100;
pub const VILLAGER_MAX_LIFE: i32 = 1_000;

const SOURCE_DEATH_BURST_SUPPRESSED_FLAG: u32 = 0x0010_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VillagerBehaviorConfig {
    pub turn_clamp: i16,
    pub ground_clearance: i16,
    pub vertical_relaxation: i16,
    pub terrain_threshold: i16,
    pub allowed_material_mask: u32,
    pub behavior_cadence: u16,
    pub engagement_range: u16,
    pub separation_radius: i16,
    pub behavior_flags: u8,
}

impl VillagerBehaviorConfig {
    pub const LEVEL_ONE: Self = Self {
        turn_clamp: 22,
        ground_clearance: 0,
        vertical_relaxation: -256,
        terrain_threshold: 15,
        allowed_material_mask: 0xffff_fefe,
        behavior_cadence: 40,
        engagement_range: 3_584,
        separation_radius: 512,
        behavior_flags: 9,
    };

    /// Level 2 row 101 is byte-for-byte the ordinary Level 1 movement
    /// parameter.  Naming it explicitly keeps profile validation honest while
    /// allowing the movement/death implementation to remain shared.
    pub const LEVEL_TWO: Self = Self::LEVEL_ONE;

    pub fn from_parameter_record(bytes: &[u8; 34]) -> Result<Self, String> {
        let config = Self {
            turn_clamp: read_i16(bytes, 2),
            ground_clearance: read_i16(bytes, 12),
            vertical_relaxation: read_i16(bytes, 14),
            terrain_threshold: read_i16(bytes, 16),
            allowed_material_mask: read_u32(bytes, 20),
            behavior_cadence: read_u16(bytes, 26),
            engagement_range: read_u16(bytes, 28),
            separation_radius: read_i16(bytes, 30),
            behavior_flags: bytes[32],
        };
        if config != Self::LEVEL_ONE {
            return Err(format!(
                "Level 1 Villager parameter row 100 mismatch: {config:?}"
            ));
        }
        Ok(config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerState {
    pub entity_index: u16,
    pub source_id: u16,
    pub active: bool,
    pub control_flags: u32,
    pub reset_counter: i32,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: i16,
    pub current_attacker: i16,
    pub kill_credit_entity: i16,
    pub rng: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub position: Position,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub render_parameter_index: u16,
    pub animation_frame: i8,
    pub action: u8,
    pub phase: u8,
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub mana: i32,
    pub owner_entity_index: u16,
    pub target_entity_index: Option<u16>,
}

impl VillagerState {
    pub fn from_snapshot(snapshot: &EntitySnapshot) -> Result<Self, String> {
        if snapshot.class != VILLAGER_CLASS || snapshot.model != VILLAGER_MODEL {
            return Err(format!(
                "entity {} is {}/{}, not Villager {VILLAGER_CLASS}/{VILLAGER_MODEL}",
                snapshot.index, snapshot.class, snapshot.model
            ));
        }
        let bytes = snapshot.record_bytes();
        if bytes.len() != 176 {
            return Err(format!(
                "exported Villager entity {} must contain 176 bytes, got {}",
                snapshot.index,
                bytes.len()
            ));
        }
        let render_parameter_index = read_u16(bytes, 94);
        if !matches!(render_parameter_index, 239 | 241 | 242 | 271) {
            return Err(format!(
                "Villager entity {} has unexpected render parameter {render_parameter_index}",
                snapshot.index
            ));
        }
        Ok(Self {
            entity_index: snapshot.index,
            source_id: read_u16(bytes, 30),
            active: snapshot.active,
            control_flags: read_u32(bytes, 16),
            reset_counter: read_i32(bytes, 20),
            max_life: read_i32(bytes, 8),
            life: read_i32(bytes, 12),
            pending_damage: read_i32(bytes, 98),
            pending_attacker: read_i16(bytes, 102),
            current_attacker: read_i16(bytes, 42),
            kill_credit_entity: read_i16(bytes, 40),
            rng: read_u16(bytes, 24),
            yaw: read_i16(bytes, 32),
            pitch: read_i16(bytes, 34),
            roll: read_i16(bytes, 36),
            position: snapshot.position,
            collision_vertical_offset: read_i16(bytes, 86),
            collision_half_width: read_i16(bytes, 88),
            collision_half_height: read_i16(bytes, 92),
            render_parameter_index,
            animation_frame: bytes[96].cast_signed(),
            action: snapshot.action,
            phase: snapshot.phase,
            stage_var_1: bytes[76].cast_signed(),
            stage_var_2: bytes[77].cast_signed(),
            actual_speed: read_i16(bytes, 134),
            minimum_speed: read_i16(bytes, 136),
            maximum_speed: read_i16(bytes, 138),
            mana: read_i32(bytes, 148),
            owner_entity_index: read_u16(bytes, 152),
            target_entity_index: relation_index(read_u16(bytes, 154)),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VillagerStepEffect {
    TryEnterBuilding {
        resident_source_entity_index: u16,
        house_source_entity_index: u16,
    },
    TargetPlayer {
        player_source_entity_index: u16,
    },
    DeathBurst {
        source_entity_index: u16,
        position: Position,
    },
    DisableSourceSlot {
        source_entity_index: u16,
    },
    PopulationCensusDecrement {
        source_entity_index: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerFamily {
    villagers: Box<[VillagerState]>,
    config: VillagerBehaviorConfig,
}

impl VillagerFamily {
    pub(crate) fn empty(config: VillagerBehaviorConfig) -> Self {
        Self {
            villagers: Box::new([]),
            config,
        }
    }
    pub fn from_entities(
        entities: &[EntitySnapshot],
        config: VillagerBehaviorConfig,
    ) -> Result<Self, String> {
        let mut villagers = Vec::with_capacity(LEVEL_ONE_VILLAGER_INDICES.len());
        for expected_index in LEVEL_ONE_VILLAGER_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Villager entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Villager pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            let state = VillagerState::from_snapshot(snapshot)?;
            if state.action != VILLAGER_STAGED_ACTION
                || state.stage_var_1 != 2
                || state.stage_var_2 != 7
            {
                return Err(format!(
                    "Villager entity {expected_index} reset state changed: action={} stage={}/{}",
                    state.action, state.stage_var_1, state.stage_var_2
                ));
            }
            villagers.push(state);
        }

        Ok(Self {
            villagers: villagers.into_boxed_slice(),
            config,
        })
    }

    /// Adopt every live Villager row from a validated campaign reset in
    /// source-slot order.  Population and reset actions are package data;
    /// the shared behavior carrier does not impose a Level-N slot list.
    pub fn from_campaign_entities(
        entities: &[EntitySnapshot],
        config: VillagerBehaviorConfig,
    ) -> Result<Self, String> {
        let villagers = entities
            .iter()
            .filter(|entity| {
                entity.active && entity.class == VILLAGER_CLASS && entity.model == VILLAGER_MODEL
            })
            .map(VillagerState::from_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            villagers: villagers.into_boxed_slice(),
            config,
        })
    }

    /// Decode the Level 2 Day ordinary-villager pool.  Behavior is shared with
    /// Level 1, but the reset action/stage values and source-slot census are
    /// profile data rather than Level 1 assumptions.
    pub fn from_level_two_entities(
        entities: &[EntitySnapshot],
        config: VillagerBehaviorConfig,
    ) -> Result<Self, String> {
        let mut villagers = Vec::with_capacity(LEVEL_TWO_VILLAGER_INDICES.len());
        for expected_index in LEVEL_TWO_VILLAGER_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Level 2 Villager entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Level 2 Villager pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            let state = VillagerState::from_snapshot(snapshot)?;
            let expected_actual_speed = (expected_index == 77).then_some(30).unwrap_or(18);
            if state.action != VILLAGER_FREE_ACTION
                || state.stage_var_1 != 0
                || state.stage_var_2 != 0
                || state.max_life != 1_000
                || state.life != 1_000
                // The qualified frame-zero capture retains a one-row
                // post-reset action-105 actual-speed write (30); all other
                // ordinary rows retain the constructor max speed (18).
                || state.actual_speed != expected_actual_speed
                || state.minimum_speed != 54
                || state.maximum_speed != 18
            {
                return Err(format!(
                    "Level 2 Villager entity {expected_index} reset state changed: action={} stage={}/{} life={}/{} speed={}/{}/{}",
                    state.action,
                    state.stage_var_1,
                    state.stage_var_2,
                    state.life,
                    state.max_life,
                    state.actual_speed,
                    state.minimum_speed,
                    state.maximum_speed,
                ));
            }
            villagers.push(state);
        }
        Ok(Self {
            villagers: villagers.into_boxed_slice(),
            config,
        })
    }

    #[must_use]
    pub fn villagers(&self) -> &[VillagerState] {
        &self.villagers
    }

    /// Bind one reset-live semantic Villager before its first scheduler visit.
    ///
    /// The imported arena and this typed family are parallel projections of
    /// the same source row. Refuse a split package baseline, then move the
    /// typed owner to the authored position after the raw arena/map carrier
    /// has been relocated.
    pub(crate) fn bind_frame_zero_semantic_variant(
        &mut self,
        entity_index: u16,
        package_position: Position,
        authored_position: Position,
        package_max_life: i32,
        authored_max_life: i32,
    ) -> Result<(), String> {
        let villager = self
            .villagers
            .iter_mut()
            .find(|villager| villager.entity_index == entity_index)
            .ok_or_else(|| format!("Villager source slot {entity_index} is absent"))?;
        if villager.position != package_position
            || villager.max_life != package_max_life
            || villager.life != package_max_life
        {
            return Err(format!(
                "Villager source slot {entity_index} diverges from its semantic package baseline"
            ));
        }
        if authored_max_life <= 0 {
            return Err(format!(
                "Villager source slot {entity_index} authored maximum life must be positive"
            ));
        }
        villager.position = authored_position;
        villager.max_life = authored_max_life;
        villager.life = authored_max_life;
        Ok(())
    }

    pub(crate) fn apply_house_construction_kill(
        &mut self,
        source_entity_index: u16,
        source_id: u16,
    ) {
        let Some(villager) = self
            .villagers
            .iter_mut()
            .find(|villager| villager.entity_index == source_entity_index)
        else {
            return;
        };
        villager.life = -1;
        villager.kill_credit_entity = source_id.cast_signed();
        villager.current_attacker = source_id.cast_signed();
    }

    /// `sub_12500` class-5 stage service for Level 1's kind-7 broadcast.
    /// This runs before source slot one and does not consume an entity phase.
    pub fn release_stage_group_7(&mut self) -> usize {
        let mut released = 0;
        for villager in &mut self.villagers {
            if villager.active && villager.action == VILLAGER_STAGED_ACTION {
                villager.action = VILLAGER_FREE_ACTION;
                released += 1;
            }
        }
        released
    }

    pub fn step_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        source_views: &[Level1SourceEntityView],
        houses: &mut HouseFamily,
    ) -> Vec<VillagerStepEffect> {
        let Some(index) = self
            .villagers
            .iter()
            .position(|villager| villager.entity_index == source_slot && villager.active)
        else {
            return Vec::new();
        };
        let mut effects =
            step_villager_state(&mut self.villagers[index], world, source_views, self.config);
        for effect in effects.iter().copied() {
            let VillagerStepEffect::TryEnterBuilding {
                house_source_entity_index,
                ..
            } = effect
            else {
                continue;
            };
            if houses.try_enter_resident(house_source_entity_index) != HouseResidentEntry::Entered {
                reject_building_entry(&mut self.villagers[index]);
            }
        }
        effects.retain(|effect| !matches!(effect, VillagerStepEffect::TryEnterBuilding { .. }));
        self.villagers[index].phase = self.villagers[index].phase.wrapping_add(1);
        effects
    }

    /// Ordinary source writers admit zero payload and an ownerless mailbox.
    /// The source identity exclusion belongs here, before mailbox mutation.
    pub fn submit_source_damage(&mut self, entity_index: u16, payload: u16, owner: u16) -> bool {
        let Some(villager) = self.villagers.iter_mut().find(|villager| {
            villager.active && villager.entity_index == entity_index && villager.source_id != owner
        }) else {
            return false;
        };
        let mut attacker = villager.pending_attacker.cast_unsigned();
        let written = crate::combat::actors::DamageMailboxMut::new(
            &mut villager.pending_damage,
            &mut attacker,
        )
        .submit(
            i32::from(payload),
            owner,
            crate::combat::actors::DamageWriter::Ordinary,
        );
        villager.pending_attacker = attacker.cast_signed();
        written
    }

    pub fn inject_damage(
        &mut self,
        entity_index: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<(), String> {
        if damage <= 0 || attacker == 0 {
            return Err("damage must be positive and attacker must be nonzero".to_owned());
        }
        let villager = self
            .villagers
            .iter_mut()
            .find(|villager| villager.entity_index == entity_index && villager.active)
            .ok_or_else(|| format!("Villager entity {entity_index} is not live"))?;
        villager.pending_damage = villager.pending_damage.wrapping_add(damage);
        villager.pending_attacker = attacker;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VillagerDamageResult {
    None,
    Hit(u16),
    Dead,
}

/// Execute one source-dispatch body for both reset/imported and house-created
/// model-13 residents. The caller owns phase advancement and applies the
/// returned effects immediately after the entity borrow ends.
pub(crate) fn step_villager_state<W: FlightWorld>(
    villager: &mut VillagerState,
    world: &W,
    source_views: &[Level1SourceEntityView],
    config: VillagerBehaviorConfig,
) -> Vec<VillagerStepEffect> {
    let mut effects = Vec::new();
    match villager.action {
        VILLAGER_INITIAL_ACTION | VILLAGER_FREE_ALIAS_ACTION_A | VILLAGER_FREE_ALIAS_ACTION_B => {
            villager.action = VILLAGER_FREE_ACTION;
            step_free_villager(villager, world, source_views, config, &mut effects);
        }
        VILLAGER_FREE_ACTION => {
            step_free_villager(villager, world, source_views, config, &mut effects);
        }
        VILLAGER_DEATH_ACTION => {
            if villager.reset_counter != 0 {
                disable_villager(villager, &mut effects);
            } else {
                // `PreKillEntity_1C890(entity, 104)`: model 13 is excluded
                // from the player creature-kill counter, then enters 109.
                villager.action = VILLAGER_REMOVAL_ACTION;
            }
        }
        VILLAGER_REMOVAL_ACTION => {
            if villager.phase & 7 == 0 {
                // Level 1 townies carry zero mana, so
                // `TransformEntityToManaSphere_36BA0(false)` performs no RNG
                // draw or allocation. The class-10/model-1 burst is attempted
                // before the dying source slot is disabled.
                if villager.control_flags & SOURCE_DEATH_BURST_SUPPRESSED_FLAG == 0 {
                    effects.push(VillagerStepEffect::DeathBurst {
                        source_entity_index: villager.entity_index,
                        position: villager.position,
                    });
                }
                disable_villager(villager, &mut effects);
            }
        }
        VILLAGER_HIT_ACTION => {
            step_hit_villager(villager, world, source_views, config, &mut effects);
        }
        VILLAGER_STAGED_ACTION => {
            step_staged_villager(villager, world, source_views, config, &mut effects);
            villager.actual_speed = if villager.action == VILLAGER_HIT_ACTION {
                villager.minimum_speed
            } else {
                villager.maximum_speed
            };
        }
        _ => {}
    }
    effects
}

fn step_free_villager<W: FlightWorld>(
    villager: &mut VillagerState,
    world: &W,
    source_views: &[Level1SourceEntityView],
    config: VillagerBehaviorConfig,
    effects: &mut Vec<VillagerStepEffect>,
) {
    match consume_villager_mailbox(villager) {
        VillagerDamageResult::Hit(attacker) => {
            if source_view(source_views, attacker)
                .is_some_and(|target| matches!(target.model, 0 | 1))
            {
                effects.push(VillagerStepEffect::TargetPlayer {
                    player_source_entity_index: attacker,
                });
            }
            villager.target_entity_index = Some(attacker);
            villager.action = VILLAGER_HIT_ACTION;
            villager.actual_speed = villager.minimum_speed;
            return;
        }
        VillagerDamageResult::Dead => {
            villager.action = VILLAGER_DEATH_ACTION;
            return;
        }
        VillagerDamageResult::None => {}
    }

    if move_villager(villager, world, config) {
        effects.push(VillagerStepEffect::PopulationCensusDecrement {
            source_entity_index: villager.entity_index,
        });
    }
    if !u16::from(villager.phase).is_multiple_of(config.behavior_cadence) {
        return;
    }

    if let Some(target_index) = villager.target_entity_index {
        let Some(target) = source_view(source_views, target_index).filter(|target| {
            target.active
                && target.life >= 0
                && target.class == 10
                && target.model == 45
                && target.villager_entry_eligible
        }) else {
            reject_building_entry(villager);
            return;
        };
        if wrapped_distance_3d(villager.position, target.position) > 0x800 {
            villager.roll =
                i16::try_from(heading_for_positions(villager.position, target.position))
                    .expect("11-bit heading fits i16");
        } else {
            villager.reset_counter = 1;
            villager.action = VILLAGER_DEATH_ACTION;
            effects.push(VillagerStepEffect::TryEnterBuilding {
                resident_source_entity_index: villager.entity_index,
                house_source_entity_index: target.source_entity_index,
            });
        }
        return;
    }

    let direction_selector = next_rng(villager.rng);
    villager.rng = next_rng(direction_selector);
    let sign = 2 * (i32::from(direction_selector % 157) / 79) - 1;
    let magnitude = i32::from(villager.rng & 0xff) + 85;
    villager.roll = wrapping_i16(i64::from(villager.roll) + i64::from(sign * magnitude)) & 0x07ff;

    let mut best = None;
    let mut best_distance = u32::MAX;
    for candidate in source_views.iter().copied().filter(|candidate| {
        candidate.active
            && candidate.life >= 0
            && candidate.class == 10
            && candidate.model == 45
            && candidate.villager_entry_eligible
    }) {
        let distance = squared_source_xy_distance(villager.position, candidate.position);
        if distance < best_distance {
            best = Some(candidate.source_entity_index);
            best_distance = distance;
        }
    }
    villager.target_entity_index = best;
    if best.is_some() {
        villager.actual_speed = villager.maximum_speed.wrapping_add(12);
    }
}

fn step_hit_villager<W: FlightWorld>(
    villager: &mut VillagerState,
    world: &W,
    source_views: &[Level1SourceEntityView],
    config: VillagerBehaviorConfig,
    effects: &mut Vec<VillagerStepEffect>,
) {
    match consume_villager_mailbox(villager) {
        VillagerDamageResult::Hit(attacker) => {
            villager.target_entity_index = Some(attacker);
            relax_to_terrain(
                &mut villager.position,
                world,
                config.ground_clearance,
                config.vertical_relaxation,
            );
            return;
        }
        VillagerDamageResult::Dead => {
            villager.action = VILLAGER_DEATH_ACTION;
        }
        VillagerDamageResult::None => {
            if move_villager(villager, world, config) {
                effects.push(VillagerStepEffect::PopulationCensusDecrement {
                    source_entity_index: villager.entity_index,
                });
            }
            let target = villager
                .target_entity_index
                .and_then(|target| source_view(source_views, target))
                .filter(|target| {
                    target.active
                        && target.life >= 0
                        && target.control_flags & SOURCE_DISABLED_FLAG == 0
                });
            if let Some(target) = target {
                if villager.phase & 3 == 0 {
                    villager.roll = i16::try_from(
                        (heading_for_positions(villager.position, target.position) + 0x400)
                            & 0x07ff,
                    )
                    .expect("11-bit heading fits i16");
                    for peer in source_views.iter().copied().filter(|peer| {
                        peer.active
                            && peer.class == VILLAGER_CLASS
                            && peer.model == VILLAGER_MODEL
                            && peer.source_id != villager.source_id
                    }) {
                        let dx =
                            (i32::from(villager.position.x) - i32::from(peer.position.x)).abs();
                        let dy =
                            (i32::from(villager.position.y) - i32::from(peer.position.y)).abs();
                        if dx < i32::from(config.separation_radius)
                            && dy < i32::from(config.separation_radius)
                        {
                            villager.roll = i16::try_from(heading_for_positions(
                                peer.position,
                                villager.position,
                            ))
                            .expect("11-bit heading fits i16");
                            break;
                        }
                    }
                }
                if !u16::from(villager.phase).is_multiple_of(config.behavior_cadence)
                    || wrapped_distance_3d(villager.position, target.position)
                        < u32::from(config.engagement_range)
                {
                    return;
                }
            }
            villager.action = VILLAGER_FREE_ACTION;
        }
    }
    if villager.action != VILLAGER_HIT_ACTION {
        villager.target_entity_index = None;
        villager.actual_speed = villager.maximum_speed;
    }
}

fn step_staged_villager<W: FlightWorld>(
    villager: &mut VillagerState,
    world: &W,
    source_views: &[Level1SourceEntityView],
    config: VillagerBehaviorConfig,
    effects: &mut Vec<VillagerStepEffect>,
) {
    match consume_villager_mailbox(villager) {
        VillagerDamageResult::Hit(attacker) => {
            let foreign_attacker = source_view(source_views, attacker).is_none_or(|target| {
                (target.class, target.model) != (VILLAGER_CLASS, VILLAGER_MODEL)
            });
            if foreign_attacker {
                villager.target_entity_index = Some(attacker);
                villager.stage_var_2 = 10;
                villager.action = if config.behavior_flags & 8 != 0 {
                    VILLAGER_HIT_ACTION
                } else {
                    VILLAGER_FREE_ALIAS_ACTION_A
                };
            }
            relax_to_terrain(
                &mut villager.position,
                world,
                config.ground_clearance,
                config.vertical_relaxation,
            );
        }
        VillagerDamageResult::Dead => {
            villager.action = VILLAGER_DEATH_ACTION;
        }
        VillagerDamageResult::None => {
            if config.behavior_flags & 2 == 0 {
                if move_villager(villager, world, config) {
                    effects.push(VillagerStepEffect::PopulationCensusDecrement {
                        source_entity_index: villager.entity_index,
                    });
                }
                if villager.phase & 0x0f == 0 && villager.control_flags & 0x0004_0000 == 0 {
                    villager.rng = next_rng(villager.rng);
                    villager.roll = villager.roll.wrapping_add(
                        i16::try_from(villager.rng % 113 + 142).expect("wander delta fits i16"),
                    ) & 0x07ff;
                }
            }
        }
    }
}

fn consume_villager_mailbox(villager: &mut VillagerState) -> VillagerDamageResult {
    let mut result = VillagerDamageResult::None;
    if villager.pending_attacker != 0 {
        villager.life = villager.life.wrapping_sub(villager.pending_damage);
        villager.current_attacker = villager.pending_attacker;
        villager.pending_attacker = 0;
        result = VillagerDamageResult::Hit(villager.current_attacker.cast_unsigned());
    } else {
        villager.current_attacker = 0;
    }
    if villager.life < 0 {
        villager.kill_credit_entity = villager.current_attacker;
        VillagerDamageResult::Dead
    } else {
        result
    }
}

fn disable_villager(villager: &mut VillagerState, effects: &mut Vec<VillagerStepEffect>) {
    villager.active = false;
    villager.control_flags |= SOURCE_DISABLED_FLAG;
    effects.push(VillagerStepEffect::DisableSourceSlot {
        source_entity_index: villager.entity_index,
    });
}

pub(crate) fn reject_building_entry(villager: &mut VillagerState) {
    villager.reset_counter = 0;
    villager.target_entity_index = None;
    villager.action = VILLAGER_FREE_ACTION;
    villager.actual_speed = villager.maximum_speed;
}

fn move_villager<W: FlightWorld>(
    villager: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
) -> bool {
    let original = villager.position;
    let original_yaw = villager.yaw;
    let headings = [
        original_yaw,
        original_yaw.wrapping_add(341) & 0x07ff,
        original_yaw.wrapping_sub(341) & 0x07ff,
        original_yaw.wrapping_add(1_024) & 0x07ff,
    ];
    for (attempt, heading) in headings.into_iter().enumerate() {
        let mut candidate = original;
        relax_to_terrain(
            &mut candidate,
            world,
            config.ground_clearance,
            config.vertical_relaxation,
        );
        move_entity(&mut candidate, heading, 0, villager.actual_speed);
        let crossed_tile = source_tile(candidate) != source_tile(original);
        let blocked = (attempt != 0 || crossed_tile)
            && (material_probe_blocked(
                candidate,
                heading,
                villager.collision_half_width,
                config.allowed_material_mask,
                world,
            ) || terrain_slope_measure(candidate, world)
                >= i32::from(config.terrain_threshold));
        if blocked {
            villager.control_flags |= 0x0004_0000;
            villager.yaw = heading;
            continue;
        }
        villager.position = candidate;
        villager.yaw =
            heading.wrapping_add(turn_toward(heading, villager.roll, config.turn_clamp)) & 0x07ff;
        if attempt == 0 {
            villager.control_flags &= !0x0004_0000;
        }
        return false;
    }
    if config.behavior_flags & 1 != 0 || world.material_mask(villager.position) == 1 {
        let was_alive = villager.life >= 0;
        villager.life = -1;
        return was_alive;
    }
    false
}

fn source_view(
    source_views: &[Level1SourceEntityView],
    source_entity_index: u16,
) -> Option<Level1SourceEntityView> {
    source_views
        .binary_search_by_key(&source_entity_index, |view| view.source_entity_index)
        .ok()
        .map(|index| source_views[index])
}

fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

const fn relation_index(value: u16) -> Option<u16> {
    if value == 0 { None } else { Some(value) }
}

fn source_tile(position: Position) -> (u8, u8) {
    (position.x.to_be_bytes()[0], position.y.to_be_bytes()[0])
}

fn squared_source_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x) - i32::from(left.x);
    let dy = i32::from(right.y) - i32::from(left.y);
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn wrapped_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .wrapping_add(dz.wrapping_mul(dz))
            .cast_unsigned(),
    )
}

fn material_probe_blocked<W: FlightWorld>(
    candidate: Position,
    heading: i16,
    collision_half_width: i16,
    allowed_material_mask: u32,
    world: &W,
) -> bool {
    let mut probe = candidate;
    let maximum = i32::from(collision_half_width.max(0));
    let mut distance = 0_i32;
    while distance <= maximum {
        if !allowed_material_mask & world.material_mask(probe) != 0 {
            return true;
        }
        distance += 256;
        move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn terrain_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let (tile_x, tile_y) = source_tile(position);
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

fn relax_to_terrain<W: FlightWorld>(
    position: &mut Position,
    world: &W,
    ground_clearance: i16,
    vertical_relaxation: i16,
) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(vertical_relaxation);
    }
    if position.z <= ground.wrapping_add(ground_clearance) {
        position.z = ground.wrapping_add(ground_clearance);
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed span"))
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    read_u16(bytes, offset).cast_signed()
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed span"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed span"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{Tile, level1_mission::Level1SourceEntityAuthority, native_world::NativeWorld};

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
            1
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    fn source_view(
        source_entity_index: u16,
        source_id: u16,
        position: Position,
        class: u8,
        model: u8,
        life: i32,
        villager_entry_eligible: bool,
    ) -> Level1SourceEntityView {
        Level1SourceEntityView {
            source_entity_index,
            source_id,
            authority: Level1SourceEntityAuthority::LiveHouse,
            active: true,
            visible: true,
            class,
            model,
            action: 52,
            phase: 0,
            entity_rng: 0,
            control_flags: 0,
            position,
            max_life: 1_000,
            life,
            pending_damage: 0,
            pending_attacker: 0,
            owner_entity_index: 0,
            target_entity_index: 0,
            linked_entity_index: 0,
            actual_speed: 0,
            maximum_speed: 0,
            collision_half_width: 128,
            collision_half_depth: 128,
            collision_vertical_offset: 0,
            collision_half_height: 128,
            villager_entry_eligible,
            builder_placement_latch: 0,
        }
    }

    fn load_world() -> NativeWorld {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        NativeWorld::load(
            root.join("level1-export-n"),
            root.join("level1-asset-closure.json"),
        )
        .expect("qualified owner package")
    }

    #[test]
    fn exact_level_one_reset_decodes_all_twenty_eight_villagers() {
        let world = load_world();
        let family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        assert_eq!(family.villagers().len(), 28);
        for (ordinal, villager) in family.villagers().iter().enumerate() {
            assert_eq!(villager.entity_index, 51 + ordinal as u16);
            assert_eq!(villager.source_id, 51 + ordinal as u16);
            assert!(villager.active);
            assert_eq!(villager.action, 111);
            assert_eq!(villager.phase, 1 + ordinal as u8);
            assert_eq!((villager.max_life, villager.life), (1_000, 1_000));
            assert_eq!((villager.pending_damage, villager.pending_attacker), (0, 0));
            assert_eq!(villager.reset_counter, 2);
            assert_eq!(villager.actual_speed, 18);
            assert_eq!(villager.minimum_speed, 54);
            assert_eq!(villager.maximum_speed, 18);
            assert_eq!(villager.stage_var_1, 2);
            assert_eq!(villager.stage_var_2, 7);
            assert_eq!((villager.mana, villager.owner_entity_index), (0, 0));
            assert!(matches!(
                villager.render_parameter_index,
                239 | 241 | 242 | 271
            ));
        }
        assert_eq!(
            family
                .villagers()
                .iter()
                .map(|villager| villager.entity_index)
                .collect::<Vec<_>>(),
            LEVEL_ONE_VILLAGER_INDICES
        );
        let family_before_house_entry = family.clone();
        let mut houses = HouseFamily::from_entities(world.entities()).expect("house family");
        let occupants_before = houses.houses()[0].occupants;
        assert_eq!(houses.try_enter_resident(1), HouseResidentEntry::Entered);
        assert_eq!(houses.houses()[0].occupants, occupants_before + 1);
        assert_eq!(
            family, family_before_house_entry,
            "VillagerFamily stores no second house authority or occupancy counter"
        );
    }

    #[test]
    fn row_one_hundred_is_pinned() {
        let bytes = [
            41, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 1, 0, 0, 0, 255, 15, 0, 0, 2, 254, 254, 255, 255,
            0, 9, 40, 0, 0, 14, 0, 2, 9, 0,
        ];
        assert_eq!(
            VillagerBehaviorConfig::from_parameter_record(&bytes),
            Ok(VillagerBehaviorConfig::LEVEL_ONE)
        );
    }

    #[test]
    fn staged_villagers_move_and_advance_in_source_order() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let mut houses = HouseFamily::from_entities(world.entities()).expect("house family");
        let before = family.villagers()[0].clone();
        family.step_source_slot(51, world.terrain(), &[], &mut houses);
        let after = &family.villagers()[0];
        assert_eq!(after.phase, before.phase.wrapping_add(1));
        assert_ne!(after.position, before.position);
        assert_eq!(family.villagers()[1].phase, 2);

        let mut staged = before;
        staged.position = Position {
            x: u16::MAX - 5,
            y: 20_000,
            z: 0,
        };
        staged.yaw = 512;
        staged.roll = 512;
        staged.actual_speed = 100;
        staged.phase = 0;
        let rng_before = staged.rng;
        let effects = step_villager_state(
            &mut staged,
            &FlatWorld,
            &[],
            VillagerBehaviorConfig::LEVEL_ONE,
        );
        assert!(effects.is_empty());
        assert_eq!(staged.position.x, 94, "movement wraps as a source u16");
        assert_eq!(staged.rng, next_rng(rng_before));
        assert_eq!(
            staged.roll,
            512_i16.wrapping_add(i16::try_from(staged.rng % 113 + 142).unwrap()) & 0x07ff
        );

        let mut blocked = staged;
        blocked.action = VILLAGER_FREE_ACTION;
        blocked.position = Position {
            x: 255,
            y: 255,
            z: 0,
        };
        blocked.actual_speed = 300;
        blocked.life = 1_000;
        let effects = step_villager_state(
            &mut blocked,
            &BlockedWorld,
            &[],
            VillagerBehaviorConfig::LEVEL_ONE,
        );
        assert_eq!(blocked.life, -1);
        assert_ne!(blocked.control_flags & 0x0004_0000, 0);
        assert_eq!(
            effects,
            [VillagerStepEffect::PopulationCensusDecrement {
                source_entity_index: blocked.entity_index
            }]
        );
    }

    #[test]
    fn lethal_damage_uses_source_actions_108_then_109_then_phase_gated_removal() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let mut houses = HouseFamily::from_entities(world.entities()).expect("house family");
        // Exercise KillTownie_23680's ordinary combat-death branch rather
        // than its nonzero dword_0x10 immediate-disable branch.
        family.villagers[0].reset_counter = 0;
        family
            .inject_damage(51, family.villagers()[0].life + 1, 152)
            .expect("lethal hit");

        family.step_source_slot(51, world.terrain(), &[], &mut houses);
        assert_eq!(family.villagers()[0].action, VILLAGER_DEATH_ACTION);
        assert!(family.villagers()[0].active);
        assert_eq!(family.villagers()[0].pending_attacker, 0);
        assert_eq!(
            family.villagers()[0].pending_damage,
            family.villagers()[0]
                .life
                .wrapping_neg()
                .wrapping_add(1_000),
            "source mailbox retains its damage dword"
        );
        assert_eq!(family.villagers()[0].kill_credit_entity, 152);

        family.step_source_slot(51, world.terrain(), &[], &mut houses);
        assert_eq!(family.villagers()[0].action, VILLAGER_REMOVAL_ACTION);
        assert!(family.villagers()[0].active);

        let mut removal_effects = Vec::new();
        while family.villagers()[0].active {
            removal_effects = family.step_source_slot(51, world.terrain(), &[], &mut houses);
        }
        assert_eq!(family.villagers()[0].phase & 7, 1);
        assert_ne!(family.villagers()[0].control_flags & 0x0000_0400, 0);
        assert!(matches!(
            removal_effects.as_slice(),
            [
                VillagerStepEffect::DeathBurst {
                    source_entity_index: 51,
                    ..
                },
                VillagerStepEffect::DisableSourceSlot {
                    source_entity_index: 51
                }
            ]
        ));
    }

    #[test]
    fn source_alias_actions_normalize_to_free_townie_action_105() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let mut houses = HouseFamily::from_entities(world.entities()).expect("house family");
        for action in [
            VILLAGER_INITIAL_ACTION,
            VILLAGER_FREE_ALIAS_ACTION_A,
            VILLAGER_FREE_ALIAS_ACTION_B,
        ] {
            family.villagers[0].action = action;
            family.step_source_slot(51, world.terrain(), &[], &mut houses);
            assert_eq!(family.villagers()[0].action, VILLAGER_FREE_ACTION);
        }

        let base = family.villagers()[0].clone();
        let mut death = base.clone();
        death.action = VILLAGER_DEATH_ACTION;
        death.reset_counter = 1;
        let effects = step_villager_state(
            &mut death,
            &FlatWorld,
            &[],
            VillagerBehaviorConfig::LEVEL_ONE,
        );
        assert!(!death.active);
        assert_eq!(
            effects,
            [VillagerStepEffect::DisableSourceSlot {
                source_entity_index: death.entity_index
            }]
        );

        let attacker = source_view(
            152,
            152,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
            3,
            0,
            1_000,
            false,
        );
        let mut staged = base.clone();
        staged.action = VILLAGER_STAGED_ACTION;
        staged.pending_damage = 5;
        staged.pending_attacker = 152;
        staged.life = 1_000;
        step_villager_state(
            &mut staged,
            &FlatWorld,
            &[attacker],
            VillagerBehaviorConfig::LEVEL_ONE,
        );
        assert_eq!(
            (
                staged.action,
                staged.stage_var_2,
                staged.target_entity_index,
                staged.actual_speed,
                staged.life,
                staged.pending_damage,
                staged.pending_attacker,
            ),
            (VILLAGER_HIT_ACTION, 10, Some(152), 54, 995, 5, 0)
        );

        let mut hit = base.clone();
        hit.action = VILLAGER_HIT_ACTION;
        hit.target_entity_index = Some(999);
        hit.phase = 40;
        hit.actual_speed = 54;
        step_villager_state(&mut hit, &FlatWorld, &[], VillagerBehaviorConfig::LEVEL_ONE);
        assert_eq!(
            (hit.action, hit.target_entity_index, hit.actual_speed),
            (VILLAGER_FREE_ACTION, None, hit.maximum_speed)
        );

        let mut selector = base.clone();
        selector.action = VILLAGER_FREE_ACTION;
        selector.phase = 40;
        selector.actual_speed = 0;
        selector.position = Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        };
        let lower = source_view(
            1,
            1,
            Position {
                x: 19_900,
                y: 20_000,
                z: 0,
            },
            10,
            45,
            1_000,
            true,
        );
        let higher = source_view(
            8,
            8,
            Position {
                x: 20_100,
                y: 20_000,
                z: 0,
            },
            10,
            45,
            1_000,
            true,
        );
        let rng_before = selector.rng;
        step_villager_state(
            &mut selector,
            &FlatWorld,
            &[lower, higher],
            VillagerBehaviorConfig::LEVEL_ONE,
        );
        assert_eq!(selector.target_entity_index, Some(1));
        assert_eq!(selector.actual_speed, selector.maximum_speed + 12);
        assert_eq!(selector.rng, next_rng(next_rng(rng_before)));

        let eligible_house = houses
            .houses()
            .iter()
            .find(|house| {
                house.active
                    && house.parameter.produces_villagers()
                    && house.occupants < i32::from(house.capacity)
            })
            .copied()
            .expect("eligible Level 1 Villager house");
        let eligible_view = source_view(
            eligible_house.entity_index,
            eligible_house.source_id,
            eligible_house.position,
            10,
            45,
            eligible_house.life,
            true,
        );
        let occupants_before = houses
            .houses()
            .iter()
            .find(|house| house.entity_index == eligible_house.entity_index)
            .unwrap()
            .occupants;
        family.villagers[0] = base.clone();
        family.villagers[0].action = VILLAGER_FREE_ACTION;
        family.villagers[0].phase = 40;
        family.villagers[0].actual_speed = 0;
        family.villagers[0].position = eligible_house.position;
        family.villagers[0].target_entity_index = Some(eligible_house.entity_index);
        family.step_source_slot(51, &FlatWorld, &[eligible_view], &mut houses);
        assert_eq!(
            houses
                .houses()
                .iter()
                .find(|house| house.entity_index == eligible_house.entity_index)
                .unwrap()
                .occupants,
            occupants_before + 1
        );
        assert_eq!(
            (
                family.villagers()[0].action,
                family.villagers()[0].reset_counter,
                family.villagers()[0].active,
            ),
            (VILLAGER_DEATH_ACTION, 1, true)
        );
        let effects = family.step_source_slot(51, &FlatWorld, &[eligible_view], &mut houses);
        assert_eq!(
            effects,
            [VillagerStepEffect::DisableSourceSlot {
                source_entity_index: 51
            }]
        );
        assert_eq!(
            houses
                .houses()
                .iter()
                .find(|house| house.entity_index == eligible_house.entity_index)
                .unwrap()
                .occupants,
            occupants_before + 1,
            "entry increments the authoritative house exactly once"
        );

        let mut full_houses = HouseFamily::from_entities(world.entities()).expect("house family");
        while full_houses.try_enter_resident(eligible_house.entity_index)
            == HouseResidentEntry::Entered
        {}
        let full_occupants = full_houses
            .houses()
            .iter()
            .find(|house| house.entity_index == eligible_house.entity_index)
            .unwrap()
            .occupants;
        family.villagers[0] = base;
        family.villagers[0].action = VILLAGER_FREE_ACTION;
        family.villagers[0].phase = 40;
        family.villagers[0].actual_speed = 0;
        family.villagers[0].position = eligible_house.position;
        family.villagers[0].target_entity_index = Some(eligible_house.entity_index);
        family.step_source_slot(51, &FlatWorld, &[eligible_view], &mut full_houses);
        assert_eq!(
            (
                family.villagers()[0].action,
                family.villagers()[0].target_entity_index,
                family.villagers()[0].reset_counter,
            ),
            (VILLAGER_FREE_ACTION, None, 0)
        );
        assert_eq!(
            full_houses
                .houses()
                .iter()
                .find(|house| house.entity_index == eligible_house.entity_index)
                .unwrap()
                .occupants,
            full_occupants,
            "full house rejection is a bookkeeping no-op"
        );

        let mut dead_view = eligible_view;
        dead_view.life = -1;
        let occupants_before_dead = full_occupants;
        family.villagers[0].target_entity_index = Some(eligible_house.entity_index);
        family.villagers[0].phase = 40;
        family.step_source_slot(51, &FlatWorld, &[dead_view], &mut full_houses);
        assert_eq!(family.villagers()[0].target_entity_index, None);
        assert_eq!(
            full_houses
                .houses()
                .iter()
                .find(|house| house.entity_index == eligible_house.entity_index)
                .unwrap()
                .occupants,
            occupants_before_dead,
            "dead house rejection is a bookkeeping no-op"
        );
    }

    #[test]
    fn group_7_stage_broadcast_releases_villagers_without_advancing_phase() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let phases = family
            .villagers()
            .iter()
            .map(|villager| villager.phase)
            .collect::<Vec<_>>();

        assert_eq!(family.release_stage_group_7(), 28);
        assert!(
            family
                .villagers()
                .iter()
                .all(|villager| villager.action == VILLAGER_FREE_ACTION)
        );
        assert_eq!(
            family
                .villagers()
                .iter()
                .map(|villager| villager.phase)
                .collect::<Vec<_>>(),
            phases
        );
        assert_eq!(family.release_stage_group_7(), 0);
    }
}
