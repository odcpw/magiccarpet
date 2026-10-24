//! Settlement population state and source-ordered Builder, Villager and Trader decisions.
//!
//! This owner borrows only population, source records, source views, RNG and feedback.
//! Arena allocation and footprint application are immediate effects applied by the caller;
//! the family cannot reach unrelated mission, spell, player or scheduler state.

use crate::house::{HOUSE_CLASS, HOUSE_MODEL};
use crate::level1_mission::{
    GlobalRngSite, Level1ResetSourceSlot, Level1SourceEffect, Level1SourceEntityView,
    SOURCE_DISABLED_FLAG,
};
use crate::villager::{
    VillagerBehaviorConfig, VillagerState, VillagerStepEffect, step_villager_state,
};
use crate::{FlightWorld, Position, distance_3d_wrapped, heading_for_positions};

pub const BUILDER_MODEL: u8 = 12;
pub const BUILDER_SEARCH_ACTION: u8 = 96;
pub const BUILDER_ROAM_ACTION: u8 = 97;
pub const BUILDER_APPROACH_ACTION: u8 = 98;
pub const BUILDER_FIND_HOUSE_ACTION: u8 = 99;
pub const BUILDER_PRE_KILL_ACTION: u8 = 100;
pub const BUILDER_KILL_ACTION: u8 = 101;
pub const BUILDER_HIT_ACTION: u8 = 102;
pub const BUILDER_RESET_ACTION: u8 = 103;
pub const BUILDER_POST_BUILD_ACTION: u8 = 105;
pub const BUILDER_DECISION_CADENCE: u8 = 40;
pub const BUILDER_GROUND_CLEARANCE: i16 = 0;
pub const BUILDER_DESCENT: i16 = -256;
pub const BUILDER_TURN_LIMIT: i16 = 22;
pub const BUILDER_TERRAIN_MASK: u32 = 0xffff_fefe;
pub const BUILDER_MAX_TILE_SLOPE: i32 = 15;
pub const BUILDER_HOUSE_APPROACH_RANGE: u32 = 0x0a00;
pub const BUILDER_HIT_RELEASE_RANGE: u32 = 0x0e00;
pub const BUILDER_BUILD_SOUND: u16 = 10;
pub const TRADER_MODEL: u8 = 14;
pub const TRADER_INITIAL_ACTION: u8 = 112;
pub const TRADER_FREE_ACTION: u8 = 113;
pub const TRADER_FREE_ALIAS_ACTION_A: u8 = 114;
pub const TRADER_FREE_ALIAS_ACTION_B: u8 = 115;
pub const TRADER_PRE_KILL_ACTION: u8 = 116;
pub const TRADER_KILL_ACTION: u8 = 117;
pub const TRADER_HIT_ACTION: u8 = 118;
pub const TRADER_STAGED_ACTION: u8 = 119;
pub const TRADER_DECISION_CADENCE: u8 = 40;
pub const TRADER_HOUSE_APPROACH_RANGE: u32 = 0x0800;
pub const TRADER_HIT_RELEASE_RANGE: u32 = 0x0e00;
pub const TRADER_HOUSE_SEARCH_MIN_DISTANCE_SQUARED: u32 = 0x0e10_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DynamicResidentKind {
    Archer = 1,
    Builder = 2,
    Villager = 3,
    Trader = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicResidentState {
    pub source_entity_index: u16,
    pub source_id: u16,
    pub kind: DynamicResidentKind,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub position: Position,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub current_attacker: u16,
    pub kill_credit_entity: u16,
    pub entity_rng: u16,
    pub yaw: i16,
    pub desired_heading: i16,
    pub minimum_speed: i16,
    pub actual_speed: i16,
    pub maximum_speed: i16,
    pub target_entity_index: u16,
    /// Source `word_0x32_50`: another model-4 resident followed by action 35.
    pub linked_entity_index: u16,
    /// Source `playerEntityIndex_0x94_148`, transferred to the death mana.
    pub owner_entity_index: u16,
    pub reset_counter: i32,
    pub mana: i32,
    pub stage_var_2: i8,
    /// Source `fontTypeIndex_0x3D_61`: Builder action 96 sets this after an
    /// accepted site, even when the class-10 allocation fails. Action 97 is
    /// the only ordinary body that clears it.
    pub builder_placement_latch: u8,
    /// Source movement collision latch (`struct_byte_0xc_12_15.byte[2] & 4`).
    pub movement_blocked: bool,
    /// Source death-burst suppression (`struct_byte_0xc_12_15.byte[2] & 0x10`).
    pub suppress_death_burst: bool,
    pub phase: u8,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub render_mode: u8,
    pub logical_height: u16,
    pub active: bool,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BuilderHouseRequest {
    pub candidate: Position,
    pub building_index: u8,
    pub spec: BuilderBuildingSpec,
}

pub(crate) struct SettlementContext<'a> {
    pub source_slots: &'a mut [Level1ResetSourceSlot],
    pub dynamic_residents: &'a mut [Option<DynamicResidentState>],
    pub class_5_census: &'a mut u32,
    pub player_archer_target_ticks: &'a mut u16,
    pub global_rng: &'a mut u32,
    pub building_source_order: &'a [u16],
    pub house_request: Option<BuilderHouseRequest>,
    pub death_bursts: Vec<(u16, Position)>,
    pub linked_kills: Vec<u16>,
    pub selection_rng_draw: Option<(u32, u32)>,
}

impl SettlementContext<'_> {
    fn advance_global_rng(&mut self, _source_slot: u16, site: GlobalRngSite) {
        debug_assert_eq!(site, GlobalRngSite::BuilderSelection);
        let before = *self.global_rng;
        *self.global_rng = before.wrapping_mul(9_377).wrapping_add(9_439);
        self.selection_rng_draw = Some((before, *self.global_rng));
    }
    fn spawn_death_burst_from_death(&mut self, slot: u16, position: Position) {
        self.death_bursts.push((slot, position));
    }
    pub(crate) fn step_dynamic_trader<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) -> Vec<Level1SourceEffect> {
        let mut effects = Vec::new();
        match resident.action {
            TRADER_INITIAL_ACTION | TRADER_FREE_ALIAS_ACTION_A | TRADER_FREE_ALIAS_ACTION_B => {
                // `sub_23790`, `sub_23AC0`, and `sub_23AE0` tail directly
                // into `sub_237B0`; the free body runs in this dispatch.
                resident.action = TRADER_FREE_ACTION;
                self.step_dynamic_trader_free(world, source_views, resident, &mut effects);
            }
            TRADER_FREE_ACTION => {
                self.step_dynamic_trader_free(world, source_views, resident, &mut effects);
            }
            TRADER_PRE_KILL_ACTION => {
                if resident.reset_counter != 0 {
                    effects.push(Level1SourceEffect::DisableSourceSlot {
                        source_entity_index: resident.source_entity_index,
                    });
                } else {
                    self.pre_kill_dynamic_trader(source_views, resident);
                }
            }
            TRADER_KILL_ACTION => {
                // `sub_23B30` performs this player feedback before the
                // phase-gated generic kill/reclaim helper.
                self.set_trader_player_hit_feedback(source_views, resident.kill_credit_entity);
                if resident.phase.trailing_zeros() >= 3 {
                    // Level 1 house-created Traders carry zero mana. The
                    // optional death burst is therefore the only allocation.
                    if !resident.suppress_death_burst {
                        self.spawn_death_burst_from_death(
                            resident.source_entity_index,
                            resident.position,
                        );
                    }
                    effects.push(Level1SourceEffect::DisableSourceSlot {
                        source_entity_index: resident.source_entity_index,
                    });
                }
            }
            TRADER_HIT_ACTION => {
                self.step_dynamic_trader_hit(world, source_views, resident);
            }
            TRADER_STAGED_ACTION => {
                // Every Level 1 reset/import and house constructor supplies
                // StageVar2 zero, for which `sub_1D5D0(entity, 112)` is an
                // exact no-op. Preserve the wrapper's speed selection.
                resident.actual_speed = if resident.action == TRADER_HIT_ACTION {
                    resident.minimum_speed
                } else {
                    resident.maximum_speed
                };
            }
            _ => {}
        }
        effects
    }

    fn step_dynamic_trader_free<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
        effects: &mut Vec<Level1SourceEffect>,
    ) {
        match self.consume_trader_mailbox(source_views, resident) {
            TraderMailboxResult::Hit(attacker) => {
                self.set_trader_player_hit_feedback(source_views, attacker);
                resident.target_entity_index = attacker;
                resident.action = TRADER_HIT_ACTION;
            }
            TraderMailboxResult::Dead => {
                resident.action = TRADER_PRE_KILL_ACTION;
            }
            TraderMailboxResult::None => {
                self.move_dynamic_builder(world, resident);
                if resident.phase.is_multiple_of(TRADER_DECISION_CADENCE) {
                    if resident.target_entity_index != 0 {
                        let target = source_view(source_views, resident.target_entity_index)
                            .filter(|target| {
                                target.active
                                    && target.class == HOUSE_CLASS
                                    && target.model == HOUSE_MODEL
                            });
                        if let Some(target) = target {
                            if distance_3d_wrapped(resident.position, target.position)
                                > TRADER_HOUSE_APPROACH_RANGE
                            {
                                resident.desired_heading = i16::try_from(heading_for_positions(
                                    resident.position,
                                    target.position,
                                ))
                                .expect("Trader house heading is 11-bit");
                            } else {
                                // The house effect sink performs the sole
                                // authoritative capacity check/increment.
                                // Rejection restores action 113 immediately;
                                // success retains this terminal marker for
                                // action 116's drawing-disable branch.
                                resident.reset_counter = 1;
                                resident.action = TRADER_PRE_KILL_ACTION;
                                effects.push(Level1SourceEffect::TryEnterVillager {
                                    resident_source_entity_index: resident.source_entity_index,
                                    house_source_entity_index: target.source_entity_index,
                                });
                            }
                        } else {
                            reject_trader_house_target(resident);
                        }
                    } else {
                        apply_builder_wander(resident);
                        let mut nearest = None;
                        let mut nearest_distance = u32::MAX;
                        for candidate in source_views.iter().copied().filter(|candidate| {
                            candidate.active
                                && candidate.class == HOUSE_CLASS
                                && candidate.model == HOUSE_MODEL
                                && candidate.villager_entry_eligible
                        }) {
                            let distance =
                                squared_wrapping_xy_distance(resident.position, candidate.position);
                            if distance < nearest_distance
                                && distance > TRADER_HOUSE_SEARCH_MIN_DISTANCE_SQUARED
                            {
                                nearest = Some(candidate.source_entity_index);
                                nearest_distance = distance;
                            }
                        }
                        if let Some(target) = nearest {
                            resident.target_entity_index = target;
                            resident.actual_speed = resident.maximum_speed.wrapping_add(12);
                        }
                    }
                }
            }
        }
        if resident.action == TRADER_HIT_ACTION {
            resident.actual_speed = resident.minimum_speed;
        }
    }

    fn step_dynamic_trader_hit<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        match self.consume_trader_mailbox(source_views, resident) {
            TraderMailboxResult::Hit(attacker) => {
                resident.target_entity_index = attacker;
                self.set_trader_player_hit_feedback(source_views, attacker);
                relax_to_terrain(
                    &mut resident.position,
                    world,
                    BUILDER_GROUND_CLEARANCE,
                    BUILDER_DESCENT,
                );
            }
            TraderMailboxResult::Dead => {
                resident.action = TRADER_PRE_KILL_ACTION;
            }
            TraderMailboxResult::None => {
                self.move_dynamic_builder(world, resident);
                let target =
                    source_view(source_views, resident.target_entity_index).filter(|target| {
                        target.active
                            && target.life >= 0
                            && target.control_flags & SOURCE_DISABLED_FLAG == 0
                    });
                if let Some(target) = target {
                    if resident.phase.trailing_zeros() >= 2 {
                        let heading = i16::try_from(heading_for_positions(
                            resident.position,
                            target.position,
                        ))
                        .expect("Trader hit target heading is 11-bit");
                        resident.desired_heading = heading.wrapping_add(0x0400) & 0x07ff;
                        if let Some(peer) = source_views.iter().copied().find(|candidate| {
                            candidate.active
                                && candidate.class == resident.class
                                && candidate.model == resident.model
                                && candidate.source_id != resident.source_id
                                && (i32::from(resident.position.x)
                                    - i32::from(candidate.position.x))
                                .abs()
                                    < i32::from(resident.collision_half_width)
                                && (i32::from(resident.position.y)
                                    - i32::from(candidate.position.y))
                                .abs()
                                    < i32::from(resident.collision_half_width)
                        }) {
                            resident.desired_heading = i16::try_from(heading_for_positions(
                                peer.position,
                                resident.position,
                            ))
                            .expect("Trader separation heading is 11-bit");
                        }
                    }
                    if !resident.phase.is_multiple_of(TRADER_DECISION_CADENCE)
                        || distance_3d_wrapped(resident.position, target.position)
                            < TRADER_HIT_RELEASE_RANGE
                    {
                        return;
                    }
                }
                resident.action = TRADER_FREE_ACTION;
            }
        }
        if resident.action != TRADER_HIT_ACTION {
            resident.target_entity_index = 0;
            resident.actual_speed = resident.maximum_speed;
        }
    }

    fn consume_trader_mailbox(
        &self,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) -> TraderMailboxResult {
        let mut result = crate::combat::actors::consume_damage(
            &mut resident.life,
            &mut resident.pending_damage,
            &mut resident.pending_attacker,
            &mut resident.current_attacker,
        )
        .map_or(TraderMailboxResult::None, |hit| {
            TraderMailboxResult::Hit(hit.attacker)
        });

        let mut linked = resident.linked_entity_index;
        while linked != 0 {
            let Some(view) = source_view(source_views, linked) else {
                break;
            };
            let (life, current_attacker, next) = self
                .dynamic_residents
                .get(usize::from(linked))
                .copied()
                .flatten()
                .map_or(
                    (view.life, view.owner_entity_index, view.linked_entity_index),
                    |linked_resident| {
                        (
                            linked_resident.life,
                            linked_resident.current_attacker,
                            linked_resident.linked_entity_index,
                        )
                    },
                );
            if crate::combat::actors::inherit_linked_life(
                &mut resident.life,
                &mut resident.current_attacker,
                life,
                current_attacker,
            ) {
                result = TraderMailboxResult::Hit(current_attacker);
                break;
            }
            linked = next;
        }
        if resident.life < 0 {
            resident.kill_credit_entity = resident.current_attacker;
            TraderMailboxResult::Dead
        } else {
            result
        }
    }

    fn pre_kill_dynamic_trader(
        &mut self,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        let mut linked = resident.linked_entity_index;
        while linked != 0 {
            let Some(view) = source_view(source_views, linked) else {
                break;
            };
            let next = self
                .dynamic_residents
                .get(usize::from(linked))
                .copied()
                .flatten()
                .map_or(view.linked_entity_index, |linked_resident| {
                    linked_resident.linked_entity_index
                });
            self.linked_kills.push(linked);
            if let Some(linked_resident) = self
                .dynamic_residents
                .get_mut(usize::from(linked))
                .and_then(Option::as_mut)
            {
                linked_resident.action = TRADER_KILL_ACTION;
                if linked_resident.kill_credit_entity != 0 {
                    resident.kill_credit_entity = linked_resident.kill_credit_entity;
                }
            }
            linked = next;
        }
        // Model 14 is explicitly excluded from `creaturesKilledPercent`.
        resident.action = TRADER_KILL_ACTION;
    }

    fn set_trader_player_hit_feedback(
        &mut self,
        source_views: &[Level1SourceEntityView],
        attacker: u16,
    ) {
        if source_view(source_views, attacker)
            .is_some_and(|attacker| matches!(attacker.model, 0 | 1))
        {
            *self.player_archer_target_ticks = 200;
        }
    }

    pub(crate) fn step_dynamic_builder<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) -> Vec<Level1SourceEffect> {
        let mut effects = Vec::new();
        match resident.action {
            BUILDER_SEARCH_ACTION => {
                self.step_dynamic_builder_placement(world, source_views, resident);
            }
            BUILDER_ROAM_ACTION => {
                self.step_dynamic_builder_roam(world, source_views, resident);
            }
            BUILDER_APPROACH_ACTION => {
                self.step_dynamic_builder_approach(world, source_views, resident);
            }
            BUILDER_FIND_HOUSE_ACTION => {
                self.step_dynamic_builder_find_house(source_views, resident);
            }
            BUILDER_PRE_KILL_ACTION => {
                // Builder constructors have no child chain. `PreKillEntity`
                // therefore performs only this action transition; model 12
                // is explicitly excluded from the player creature census.
                resident.action = BUILDER_KILL_ACTION;
            }
            BUILDER_KILL_ACTION => {
                self.set_builder_player_hit_feedback(source_views, resident.kill_credit_entity);
                if resident.phase & 7 == 0 {
                    // Builder mana is zero, so
                    // `TransformEntityToManaSphere(false)` performs no RNG
                    // draw and no allocation. The optional death burst is the
                    // sole allocation before disable/unlink.
                    if !resident.suppress_death_burst {
                        self.spawn_death_burst_from_death(
                            resident.source_entity_index,
                            resident.position,
                        );
                    }
                    effects.push(Level1SourceEffect::DisableSourceSlot {
                        source_entity_index: resident.source_entity_index,
                    });
                }
            }
            BUILDER_HIT_ACTION => {
                self.step_dynamic_builder_hit(world, source_views, resident);
            }
            BUILDER_RESET_ACTION => {
                resident.reset_counter = 5;
                resident.action = BUILDER_ROAM_ACTION;
                self.step_dynamic_builder_roam(world, source_views, resident);
            }
            // Action 96 deliberately hands the Builder to the already-closed
            // townie/building-entry band at 105. Keep that shared table body
            // live for model 12 rather than freezing a successful or
            // allocation-exhausted Builder at the ownership boundary.
            104..=111 => {
                return self.step_dynamic_villager(world, source_views, resident);
            }
            _ => {}
        }
        effects
    }

    fn step_dynamic_builder_placement<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        let Some(target) = source_view(source_views, resident.target_entity_index)
            .filter(|target| target.class != 0 && target.model == HOUSE_MODEL)
        else {
            resident.reset_counter = 5;
            resident.target_entity_index = 0;
            resident.action = BUILDER_ROAM_ACTION;
            return;
        };

        let attempt = resident.reset_counter;
        resident.reset_counter = resident.reset_counter.wrapping_add(1);
        if attempt >= 4 {
            resident.reset_counter = 1;
            resident.target_entity_index = 0;
            resident.action = BUILDER_ROAM_ACTION;
            return;
        }

        self.advance_global_rng(
            resident.source_entity_index,
            GlobalRngSite::BuilderSelection,
        );
        let building_index = select_builder_building_index(*self.global_rng);
        let spec = builder_building_spec(building_index)
            .expect("pinned eligible/fallback BLDGPRM rows have BUILD/MSPRD data");
        let extent_x = i16::from(spec.sprite_width)
            .wrapping_mul(128)
            .wrapping_add(768);
        let extent_y = i16::from(spec.sprite_height)
            .wrapping_mul(128)
            .wrapping_add(768);
        let mut candidate = target.position;

        resident.entity_rng = next_rng16(resident.entity_rng);
        let jitter_x = i16::try_from((resident.entity_rng % 3) << 8)
            .expect("Builder three-way X jitter fits i16");
        resident.entity_rng = next_rng16(resident.entity_rng);
        let jitter_y = i16::try_from((resident.entity_rng % 3) << 8)
            .expect("Builder three-way Y jitter fits i16");
        match resident.reset_counter {
            1 => {
                candidate.x = candidate.x.wrapping_add_signed(
                    jitter_x
                        .wrapping_add(target.collision_half_width)
                        .wrapping_add(extent_x)
                        .wrapping_add(256),
                );
                candidate.y = candidate
                    .y
                    .wrapping_add_signed(jitter_y.wrapping_sub(1_280));
            }
            2 => {
                candidate.x = candidate.x.wrapping_sub_signed(
                    jitter_x
                        .wrapping_add(target.collision_half_width)
                        .wrapping_add(extent_x)
                        .wrapping_add(256),
                );
                candidate.y = candidate
                    .y
                    .wrapping_add_signed(jitter_y.wrapping_sub(1_280));
            }
            3 => {
                candidate.x = candidate
                    .x
                    .wrapping_add_signed(jitter_x.wrapping_sub(1_280));
                candidate.y = candidate.y.wrapping_add_signed(
                    jitter_y
                        .wrapping_add(target.collision_half_depth)
                        .wrapping_add(extent_y)
                        .wrapping_add(256),
                );
            }
            4 => {
                candidate.x = candidate
                    .x
                    .wrapping_add_signed(jitter_x.wrapping_sub(1_280));
                candidate.y = candidate.y.wrapping_sub_signed(
                    target
                        .collision_half_depth
                        .wrapping_add(extent_y)
                        .wrapping_add(jitter_y)
                        .wrapping_add(256),
                );
            }
            _ => unreachable!("post-increment Builder placement attempts are one through four"),
        }

        // `sub_104D0 == 1` is exactly terrain material/type zero. This branch
        // abandons the target immediately rather than trying the next side.
        if world.terrain_type(candidate) == 0 {
            resident.reset_counter = 2;
            resident.target_entity_index = 0;
            resident.action = BUILDER_ROAM_ACTION;
            return;
        }

        let width_tiles = i16::from(spec.sprite_width) / 2 + 3;
        let height_tiles = i16::from(spec.sprite_height) / 2 + 3;
        if !builder_site_height_is_eligible(world, candidate, width_tiles, height_tiles) {
            return;
        }

        let building_source_order = self.building_source_order;
        if builder_placement_blocker_in_routes(
            source_views,
            &building_source_order,
            candidate,
            extent_x,
            extent_y,
        )
        .is_some()
        {
            return;
        }

        self.house_request = Some(BuilderHouseRequest {
            candidate,
            building_index,
            spec,
        });
        // The source does not roll back this state when NewEvent exhausts.
        resident.target_entity_index = 0;
        resident.builder_placement_latch = 1;
        resident.action = BUILDER_POST_BUILD_ACTION;
    }

    fn step_dynamic_builder_roam<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        resident.builder_placement_latch = 0;
        match consume_builder_mailbox(resident) {
            BuilderMailboxResult::Dead => {
                resident.action = BUILDER_PRE_KILL_ACTION;
                return;
            }
            BuilderMailboxResult::Hit(attacker) => {
                self.set_builder_player_hit_feedback(source_views, attacker);
                resident.target_entity_index = attacker;
                resident.action = BUILDER_HIT_ACTION;
                resident.actual_speed = resident.minimum_speed;
                return;
            }
            BuilderMailboxResult::None => {}
        }

        self.move_dynamic_builder(world, resident);
        if resident.phase % BUILDER_DECISION_CADENCE == 0 {
            apply_builder_wander(resident);
            let previous = resident.reset_counter;
            resident.reset_counter = resident.reset_counter.wrapping_sub(1);
            if previous == 0 {
                resident.reset_counter = 1;
                resident.action = BUILDER_FIND_HOUSE_ACTION;
            }
        }
    }

    fn step_dynamic_builder_approach<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        match consume_builder_mailbox(resident) {
            BuilderMailboxResult::Dead => {
                resident.action = BUILDER_PRE_KILL_ACTION;
                return;
            }
            BuilderMailboxResult::Hit(attacker) => {
                self.set_builder_player_hit_feedback(source_views, attacker);
                resident.target_entity_index = attacker;
                resident.action = BUILDER_HIT_ACTION;
                resident.actual_speed = resident.minimum_speed;
                return;
            }
            BuilderMailboxResult::None => {}
        }

        self.move_dynamic_builder(world, resident);
        if (resident.phase % BUILDER_DECISION_CADENCE) / 2 != 0 {
            return;
        }
        let previous = resident.reset_counter;
        resident.reset_counter = resident.reset_counter.wrapping_sub(1);
        let target = source_view(source_views, resident.target_entity_index);
        if previous == 0 || target.is_none_or(|target| target.class == 0) {
            resident.reset_counter = 5;
            resident.action = BUILDER_ROAM_ACTION;
            return;
        }
        let target_position =
            target.map_or(Position { x: 0, y: 0, z: 0 }, |target| target.position);
        resident.desired_heading =
            i16::try_from(heading_for_positions(resident.position, target_position))
                .expect("Builder target heading is 11-bit");
        if distance_3d_wrapped(resident.position, target_position) < BUILDER_HOUSE_APPROACH_RANGE {
            resident.reset_counter = 0;
            resident.action = BUILDER_SEARCH_ACTION;
        }
    }

    fn step_dynamic_builder_find_house(
        &mut self,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        match consume_builder_mailbox(resident) {
            BuilderMailboxResult::Dead => {
                resident.action = BUILDER_PRE_KILL_ACTION;
                return;
            }
            BuilderMailboxResult::Hit(attacker) => {
                self.set_builder_player_hit_feedback(source_views, attacker);
                resident.target_entity_index = attacker;
                resident.action = BUILDER_HIT_ACTION;
                resident.actual_speed = resident.minimum_speed;
                return;
            }
            BuilderMailboxResult::None => {}
        }

        let mut nearest = None;
        let mut nearest_distance = u32::MAX;
        for candidate in source_views.iter().copied().filter(|candidate| {
            candidate.active && candidate.class == HOUSE_CLASS && candidate.model == HOUSE_MODEL
        }) {
            let distance = distance_3d_wrapped(resident.position, candidate.position);
            if distance != 0 && distance < nearest_distance {
                nearest = Some(candidate.source_entity_index);
                nearest_distance = distance;
            }
        }
        if let Some(target) = nearest {
            resident.target_entity_index = target;
            resident.reset_counter = 10;
            resident.action = BUILDER_APPROACH_ACTION;
        } else {
            resident.reset_counter = 5;
            resident.action = BUILDER_ROAM_ACTION;
        }
    }

    fn step_dynamic_builder_hit<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) {
        match consume_builder_mailbox(resident) {
            BuilderMailboxResult::Dead => {
                resident.action = BUILDER_PRE_KILL_ACTION;
            }
            BuilderMailboxResult::Hit(attacker) => {
                resident.target_entity_index = attacker;
                relax_to_terrain(
                    &mut resident.position,
                    world,
                    BUILDER_GROUND_CLEARANCE,
                    BUILDER_DESCENT,
                );
            }
            BuilderMailboxResult::None => {
                self.move_dynamic_builder(world, resident);
                let target =
                    source_view(source_views, resident.target_entity_index).filter(|target| {
                        target.active
                            && target.life >= 0
                            && target.control_flags & SOURCE_DISABLED_FLAG == 0
                    });
                if let Some(target) = target {
                    if resident.phase & 3 == 0 {
                        let heading = i16::try_from(heading_for_positions(
                            resident.position,
                            target.position,
                        ))
                        .expect("Builder hit target heading is 11-bit");
                        resident.desired_heading = heading.wrapping_add(0x0400) & 0x07ff;
                        if let Some(peer) = source_views.iter().copied().find(|candidate| {
                            candidate.active
                                && candidate.life >= 0
                                && candidate.class == resident.class
                                && candidate.model == resident.model
                                && candidate.source_id != resident.source_id
                                && (i32::from(resident.position.x)
                                    - i32::from(candidate.position.x))
                                .abs()
                                    < i32::from(resident.collision_half_width)
                                && (i32::from(resident.position.y)
                                    - i32::from(candidate.position.y))
                                .abs()
                                    < i32::from(resident.collision_half_width)
                        }) {
                            resident.desired_heading = i16::try_from(heading_for_positions(
                                peer.position,
                                resident.position,
                            ))
                            .expect("Builder separation heading is 11-bit");
                        }
                    }
                    if resident.phase % BUILDER_DECISION_CADENCE != 0
                        || distance_3d_wrapped(resident.position, target.position)
                            < BUILDER_HIT_RELEASE_RANGE
                    {
                        return;
                    }
                }
                resident.action = BUILDER_ROAM_ACTION;
            }
        }

        // `sub_23260` deliberately normalizes every non-102 result,
        // including a freshly selected action 100, through action 97.
        if resident.action != BUILDER_HIT_ACTION {
            resident.reset_counter = 5;
            resident.target_entity_index = 0;
            resident.action = BUILDER_ROAM_ACTION;
            resident.actual_speed = resident.maximum_speed;
        }
    }

    fn set_builder_player_hit_feedback(
        &mut self,
        source_views: &[Level1SourceEntityView],
        attacker: u16,
    ) {
        if source_view(source_views, attacker)
            .is_some_and(|attacker| matches!(attacker.model, 0 | 1))
        {
            *self.player_archer_target_ticks = 200;
        }
    }

    fn move_dynamic_builder<W: FlightWorld>(
        &mut self,
        world: &W,
        resident: &mut DynamicResidentState,
    ) {
        let was_alive = resident.life >= 0;
        let transition = crate::ground_creature_motion::step_ground_creature_motion(
            resident.position,
            resident.yaw,
            resident.desired_heading,
            resident.actual_speed,
            world,
            crate::ground_creature_motion::GroundCreatureMotionConfig {
                ground_clearance: BUILDER_GROUND_CLEARANCE,
                descent: BUILDER_DESCENT,
                turn_limit: BUILDER_TURN_LIMIT,
                terrain_mask: BUILDER_TERRAIN_MASK,
                maximum_tile_slope: BUILDER_MAX_TILE_SLOPE,
            },
        );
        resident.position = transition.position;
        resident.yaw = transition.yaw;
        resident.movement_blocked = transition.movement_blocked;
        if transition.killed {
            resident.life = -1;
        }
        if was_alive && resident.life == -1 && *self.class_5_census != 0 {
            *self.class_5_census -= 1;
        }
    }

    pub(crate) fn step_dynamic_villager<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
    ) -> Vec<Level1SourceEffect> {
        let source_index = usize::from(resident.source_entity_index);
        let mut control_flags = self.source_slots[source_index].control_flags;
        if resident.suppress_death_burst {
            control_flags |= 0x0010_0000;
        } else {
            control_flags &= !0x0010_0000;
        }
        let mut villager = VillagerState {
            entity_index: resident.source_entity_index,
            source_id: resident.source_id,
            active: resident.active,
            control_flags,
            reset_counter: resident.reset_counter,
            max_life: resident.max_life,
            life: resident.life,
            pending_damage: resident.pending_damage,
            pending_attacker: resident.pending_attacker.cast_signed(),
            current_attacker: resident.current_attacker.cast_signed(),
            kill_credit_entity: resident.kill_credit_entity.cast_signed(),
            rng: resident.entity_rng,
            yaw: resident.yaw,
            pitch: 0,
            roll: resident.desired_heading,
            position: resident.position,
            collision_half_width: resident.collision_half_width,
            collision_vertical_offset: resident.collision_vertical_offset,
            collision_half_height: resident.collision_half_height,
            render_parameter_index: resident.render_parameter_index,
            animation_frame: 0,
            action: resident.action,
            phase: resident.phase,
            stage_var_1: 0,
            stage_var_2: resident.stage_var_2,
            actual_speed: resident.actual_speed,
            minimum_speed: resident.minimum_speed,
            maximum_speed: resident.maximum_speed,
            mana: resident.mana,
            owner_entity_index: resident.owner_entity_index,
            target_entity_index: (resident.target_entity_index != 0)
                .then_some(resident.target_entity_index),
        };
        let villager_effects = step_villager_state(
            &mut villager,
            world,
            source_views,
            VillagerBehaviorConfig::LEVEL_ONE,
        );

        resident.active = villager.active;
        resident.visible &= villager.active;
        resident.reset_counter = villager.reset_counter;
        resident.max_life = villager.max_life;
        resident.life = villager.life;
        resident.pending_damage = villager.pending_damage;
        resident.pending_attacker = villager.pending_attacker.cast_unsigned();
        resident.current_attacker = villager.current_attacker.cast_unsigned();
        resident.kill_credit_entity = villager.kill_credit_entity.cast_unsigned();
        resident.entity_rng = villager.rng;
        resident.yaw = villager.yaw;
        resident.desired_heading = villager.roll;
        resident.position = villager.position;
        resident.action = villager.action;
        resident.stage_var_2 = villager.stage_var_2;
        resident.actual_speed = villager.actual_speed;
        resident.minimum_speed = villager.minimum_speed;
        resident.maximum_speed = villager.maximum_speed;
        resident.mana = villager.mana;
        resident.owner_entity_index = villager.owner_entity_index;
        resident.target_entity_index = villager.target_entity_index.unwrap_or(0);
        resident.movement_blocked = villager.control_flags & 0x0004_0000 != 0;
        resident.suppress_death_burst = villager.control_flags & 0x0010_0000 != 0;
        self.source_slots[source_index].control_flags = villager.control_flags;
        self.source_slots[source_index].entity_rng = villager.rng;

        let mut effects = Vec::with_capacity(villager_effects.len());
        for effect in villager_effects {
            match effect {
                VillagerStepEffect::TryEnterBuilding {
                    resident_source_entity_index,
                    house_source_entity_index,
                } => effects.push(Level1SourceEffect::TryEnterVillager {
                    resident_source_entity_index,
                    house_source_entity_index,
                }),
                VillagerStepEffect::TargetPlayer { .. } => {
                    *self.player_archer_target_ticks = 200;
                }
                VillagerStepEffect::DeathBurst {
                    source_entity_index,
                    position,
                } => {
                    self.spawn_death_burst_from_death(source_entity_index, position);
                }
                VillagerStepEffect::DisableSourceSlot {
                    source_entity_index,
                } => effects.push(Level1SourceEffect::DisableSourceSlot {
                    source_entity_index,
                }),
                VillagerStepEffect::PopulationCensusDecrement { .. } => {
                    if *self.class_5_census != 0 {
                        *self.class_5_census -= 1;
                    }
                }
            }
        }
        effects
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuilderMailboxResult {
    None,
    Hit(u16),
    Dead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraderMailboxResult {
    None,
    Hit(u16),
    Dead,
}

fn consume_builder_mailbox(resident: &mut DynamicResidentState) -> BuilderMailboxResult {
    let hit = crate::combat::actors::consume_damage(
        &mut resident.life,
        &mut resident.pending_damage,
        &mut resident.pending_attacker,
        &mut resident.current_attacker,
    );
    if resident.life < 0 {
        resident.kill_credit_entity = resident.current_attacker;
        BuilderMailboxResult::Dead
    } else {
        hit.map_or(BuilderMailboxResult::None, |hit| {
            BuilderMailboxResult::Hit(hit.attacker)
        })
    }
}

pub(crate) fn reject_trader_house_target(resident: &mut DynamicResidentState) {
    resident.reset_counter = 0;
    resident.target_entity_index = 0;
    resident.action = TRADER_FREE_ACTION;
    resident.actual_speed = resident.maximum_speed;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BuilderBuildingSpec {
    pub(crate) sprite_width: u8,
    pub(crate) sprite_height: u8,
    pub(crate) footprint_width: u8,
    pub(crate) footprint_height: u8,
}

pub(crate) fn builder_building_spec(building_index: u8) -> Option<BuilderBuildingSpec> {
    let (sprite_width, sprite_height, footprint_width, footprint_height) = match building_index {
        // Row 17 is the source fallback when the bounded eligibility scan
        // exhausts; its BLDGPRM flags do not make it a normal candidate.
        17 => (2, 1, 48, 48),
        19 => (2, 1, 6, 4),
        22 => (2, 1, 4, 4),
        26 => (2, 1, 9, 11),
        29 => (2, 1, 13, 11),
        30 => (5, 7, 13, 15),
        31 => (5, 7, 23, 11),
        32 => (5, 7, 39, 27),
        33 => (5, 7, 17, 17),
        34 => (5, 7, 22, 22),
        35 => (5, 7, 15, 15),
        39 => (5, 7, 25, 9),
        40 => (62, 19, 21, 23),
        42 => (8, 22, 11, 11),
        47 => (17, 22, 11, 11),
        50 => (17, 22, 7, 7),
        62 => (4, 5, 27, 25),
        74 => (15, 9, 15, 15),
        _ => return None,
    };
    Some(BuilderBuildingSpec {
        sprite_width,
        sprite_height,
        footprint_width,
        footprint_height,
    })
}

fn builder_building_is_eligible(building_index: u8) -> bool {
    matches!(
        building_index,
        19 | 22 | 26 | 29 | 30 | 31 | 32 | 33 | 34 | 35 | 39 | 40 | 42 | 47 | 50 | 62 | 74
    )
}

pub(crate) fn select_builder_building_index(global_rng: u32) -> u8 {
    let mut building_index = ((global_rng % 60) + 17) as u8;
    let mut probes = 0_u8;
    while !builder_building_is_eligible(building_index) && probes < 77 {
        building_index = building_index.wrapping_add(1);
        probes = probes.wrapping_add(1);
        if building_index >= 76 {
            building_index = 17;
        }
    }
    if probes >= 77 { 17 } else { building_index }
}

pub(crate) fn builder_site_height_is_eligible<W: FlightWorld>(
    world: &W,
    candidate: Position,
    width_tiles: i16,
    height_tiles: i16,
) -> bool {
    let mut start_x = (candidate.x >> 8) as u8;
    start_x = start_x.wrapping_sub((width_tiles >> 1) as u8);
    let start_y = ((candidate.y >> 8) as u8).wrapping_sub((height_tiles >> 1) as u8);
    if start_x.wrapping_add(start_y) & 1 != 0 {
        start_x = start_x.wrapping_add(1);
    }
    let width = width_tiles as u8;
    let height = height_tiles as u8;
    let terrain_height = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) / 32
    };
    let heights = [
        terrain_height(start_x, start_y),
        terrain_height(start_x.wrapping_add(width), start_y),
        terrain_height(start_x.wrapping_add(width), start_y.wrapping_add(height)),
        terrain_height(start_x, start_y.wrapping_add(height)),
    ];
    let range = heights.iter().max().expect("four Builder site corners")
        - heights.iter().min().expect("four Builder site corners");
    // For every BUILD/MSPRD row in this band, the source boolean expression
    // `((width >> 7) + (height >> 7) > 4) + 15` is exactly 16.
    range < 16
}

fn builder_site_overlaps(
    candidate: Position,
    extent_x: i16,
    extent_y: i16,
    other: Position,
    other_extent_x: i16,
    other_extent_y: i16,
) -> bool {
    let delta_x = other.x.wrapping_sub(candidate.x).cast_signed();
    let delta_y = other.y.wrapping_sub(candidate.y).cast_signed();
    i32::from(delta_x).abs() <= i32::from(other_extent_x.wrapping_add(extent_x))
        && i32::from(delta_y).abs() <= i32::from(other_extent_y.wrapping_add(extent_y))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuilderPlacementBlocker {
    Building(u16),
    Model2(u16),
    Model67(u16),
    BuilderLatch(u16),
}

pub(crate) fn builder_placement_blocker_in_routes(
    source_views: &[Level1SourceEntityView],
    building_source_order: &[u16],
    candidate: Position,
    extent_x: i16,
    extent_y: i16,
) -> Option<BuilderPlacementBlocker> {
    let overlaps = |view: &Level1SourceEntityView| {
        builder_site_overlaps(
            candidate,
            extent_x,
            extent_y,
            view.position,
            view.collision_half_width,
            view.collision_half_depth,
        )
    };

    // The pinned turn-start list builder admits only class-10/model-45 rows
    // to `dword_38527`; action 96 traverses that frozen complete list before
    // either class-3 subtype scan without re-checking the current tuple.
    for source_entity_index in building_source_order {
        if let Some(view) = source_view(source_views, *source_entity_index).filter(overlaps) {
            return Some(BuilderPlacementBlocker::Building(view.source_entity_index));
        }
    }
    if let Some(view) = source_views.iter().find(|view| {
        view.active && view.life >= 0 && view.class == 3 && view.model == 2 && overlaps(view)
    }) {
        return Some(BuilderPlacementBlocker::Model2(view.source_entity_index));
    }
    if let Some(view) = source_views.iter().find(|view| {
        view.active && view.life >= 0 && view.class == 3 && view.model == 67 && overlaps(view)
    }) {
        return Some(BuilderPlacementBlocker::Model67(view.source_entity_index));
    }
    source_views
        .iter()
        .find(|view| {
            view.active
                && view.life >= 0
                && view.class == 5
                && view.model == BUILDER_MODEL
                && !matches!(view.action, 180 | 232 | 234)
                && view.builder_placement_latch != 0
        })
        .map(|view| BuilderPlacementBlocker::BuilderLatch(view.source_entity_index))
}

pub(crate) fn builder_house_position<W: FlightWorld>(
    world: &W,
    candidate: Position,
    spec: BuilderBuildingSpec,
) -> Position {
    let mut position = Position {
        x: candidate.x & 0xff00,
        y: candidate.y & 0xff00,
        z: candidate.z,
    };
    let mut start_x = (position.x >> 8) as u8;
    start_x = start_x.wrapping_sub(spec.footprint_width / 2);
    let start_y = ((position.y >> 8) as u8).wrapping_sub(spec.footprint_height / 2);
    if start_x.wrapping_add(start_y) & 1 != 0 {
        position.x = position.x.wrapping_add(256);
        start_x = start_x.wrapping_add(1);
    }
    let corner_height = |x: u8, y: u8| {
        let height = world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        });
        (i32::from(height) / 32) & 0xff
    };
    let sum = corner_height(start_x, start_y)
        + corner_height(start_x.wrapping_add(spec.footprint_width), start_y)
        + corner_height(
            start_x.wrapping_add(spec.footprint_width),
            start_y.wrapping_add(spec.footprint_height),
        )
        + corner_height(start_x, start_y.wrapping_add(spec.footprint_height));
    position.z = i16::try_from((sum >> 2) * 32).expect("Builder house terrain Z fits i16");
    position
}

fn apply_builder_wander(resident: &mut DynamicResidentState) {
    resident.entity_rng = next_rng16(resident.entity_rng);
    let selector = resident.entity_rng;
    resident.entity_rng = next_rng16(resident.entity_rng);
    let sign = if selector % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((resident.entity_rng & 0x00ff) + 85).expect("Builder wander delta fits i16");
    resident.desired_heading = resident.desired_heading.wrapping_add(sign * delta) & 0x07ff;
}

fn source_view(views: &[Level1SourceEntityView], slot: u16) -> Option<Level1SourceEntityView> {
    views
        .binary_search_by_key(&slot, |view| view.source_entity_index)
        .ok()
        .map(|index| views[index])
}
const fn next_rng16(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}
fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}
fn relax_to_terrain<W: FlightWorld>(
    position: &mut Position,
    world: &W,
    clearance: i16,
    descent: i16,
) {
    crate::ground_creature_motion::relax_to_terrain(position, world, clearance, descent);
}

/// Source AddArchers/sub_4BDF0/AddVilliger/AddTrader payload, after arena allocation.
/// The one event RNG draw, optional Villager appearance draw and model phase
/// are retained by the caller's source carrier; no second family arena exists.
pub(crate) fn resident_constructor(
    source_entity_index: u16,
    initial_rng: u16,
    phase: u8,
    position: Position,
    kind: DynamicResidentKind,
    model: u8,
    action: u8,
    attacker_entity_index: u16,
) -> DynamicResidentState {
    let mut entity_rng = next_rng16(initial_rng);
    let yaw = i16::try_from(entity_rng & 0x07ff)
        .expect("11-bit heading fits i16")
        .wrapping_sub(1);
    let (minimum_speed, actual_speed, maximum_speed, reset_counter) = match kind {
        DynamicResidentKind::Archer => (30, 30, 0, i32::from(source_entity_index % 100)),
        DynamicResidentKind::Builder => (54, 24, 24, 2),
        DynamicResidentKind::Villager | DynamicResidentKind::Trader => (54, 18, 18, 2),
    };
    let (render_parameter_index, base_tmap, logical_height) = match kind {
        DynamicResidentKind::Archer => (0, 8, 250),
        DynamicResidentKind::Builder => (221, 141, 250),
        DynamicResidentKind::Trader => (219, 138, 250),
        DynamicResidentKind::Villager => {
            entity_rng = next_rng16(entity_rng);
            match entity_rng % 9 {
                0..=2 => (242, 183, 250),
                3..=5 => (271, 199, 300),
                6..=7 => (241, 180, 250),
                8 => (239, 155, 200),
                _ => unreachable!("modulo-nine branch"),
            }
        }
    };
    let collision_half_height = if kind == DynamicResidentKind::Archer {
        256
    } else {
        128
    };
    DynamicResidentState {
        source_entity_index,
        source_id: source_entity_index,
        kind,
        class: 5,
        model,
        action,
        position,
        max_life: 1_000,
        life: 1_000,
        pending_damage: 1,
        pending_attacker: attacker_entity_index,
        current_attacker: 0,
        kill_credit_entity: 0,
        entity_rng,
        yaw,
        desired_heading: yaw,
        minimum_speed,
        actual_speed,
        maximum_speed,
        target_entity_index: 0,
        linked_entity_index: 0,
        owner_entity_index: 0,
        reset_counter,
        mana: 0,
        stage_var_2: 0,
        builder_placement_latch: 0,
        movement_blocked: false,
        suppress_death_burst: false,
        phase,
        collision_half_width: 128,
        collision_vertical_offset: 0,
        collision_half_height,
        render_parameter_index,
        base_tmap,
        render_mode: 20,
        logical_height,
        active: true,
        visible: true,
    }
}
