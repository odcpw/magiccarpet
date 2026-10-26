//! Settlement population state and source-ordered Builder, Villager and Trader decisions.
//!
//! This owner borrows only population, source records, source views, RNG and feedback.
//! Arena allocation and footprint application are immediate effects applied by the caller;
//! the family cannot reach unrelated mission, spell, player or scheduler state.

pub(crate) mod runtime;
pub(crate) mod stage;
pub use stage::SettlementStageLinks;

use crate::house::{HOUSE_CLASS, HOUSE_MODEL};
use crate::level_definition::{
    BUILDER_CLASS_PROPERTIES, TRADER_CLASS_PROPERTIES, VILLAGER_CLASS_PROPERTIES,
};
use crate::level1_mission::{
    GlobalRngSite, Level1SourceEffect, Level1SourceEntityView, SOURCE_DISABLED_FLAG,
};
use crate::villager::{
    VillagerBehaviorConfig, VillagerState, VillagerStepEffect, step_villager_state_with_stage,
};
use crate::{FlightWorld, Position, distance_3d_wrapped, heading_for_positions};
use sim_definition::adapter::{
    BUILDER_APPEARANCE, BUILDER_SOURCE_BINDING, TRADER_APPEARANCE, TRADER_SOURCE_BINDING,
    VILLAGER_APPEARANCES,
};

pub const BUILDER_MODEL: u8 = BUILDER_SOURCE_BINDING.model;
pub const BUILDER_SEARCH_ACTION: u8 = 96;
pub const BUILDER_ROAM_ACTION: u8 = 97;
pub const BUILDER_APPROACH_ACTION: u8 = 98;
pub const BUILDER_FIND_HOUSE_ACTION: u8 = 99;
pub const BUILDER_PRE_KILL_ACTION: u8 = 100;
pub const BUILDER_KILL_ACTION: u8 = 101;
pub const BUILDER_HIT_ACTION: u8 = 102;
pub const BUILDER_RESET_ACTION: u8 = 103;
pub const BUILDER_POST_BUILD_ACTION: u8 = 105;
pub const BUILDER_DECISION_CADENCE: u8 = BUILDER_CLASS_PROPERTIES.scan_period;
pub const BUILDER_GROUND_CLEARANCE: i16 = BUILDER_CLASS_PROPERTIES.ground_clearance;
pub const BUILDER_DESCENT: i16 = BUILDER_CLASS_PROPERTIES.descent;
pub const BUILDER_TURN_LIMIT: i16 = BUILDER_CLASS_PROPERTIES.turn_limit;
pub const BUILDER_TERRAIN_MASK: u32 = BUILDER_CLASS_PROPERTIES.terrain_mask;
pub const BUILDER_MAX_TILE_SLOPE: i32 = BUILDER_CLASS_PROPERTIES.maximum_slope;
pub const BUILDER_HOUSE_APPROACH_RANGE: u32 = 0x0a00;
pub const BUILDER_HIT_RELEASE_RANGE: u32 = 0x0e00;
pub const BUILDER_BUILD_SOUND: u16 = 10;
pub const TRADER_MODEL: u8 = TRADER_SOURCE_BINDING.model;
pub const TRADER_INITIAL_ACTION: u8 = 112;
pub const TRADER_FREE_ACTION: u8 = 113;
pub const TRADER_FREE_ALIAS_ACTION_A: u8 = 114;
pub const TRADER_FREE_ALIAS_ACTION_B: u8 = 115;
pub const TRADER_PRE_KILL_ACTION: u8 = 116;
pub const TRADER_KILL_ACTION: u8 = 117;
pub const TRADER_HIT_ACTION: u8 = 118;
pub const TRADER_STAGED_ACTION: u8 = 119;
pub const TRADER_DECISION_CADENCE: u8 = TRADER_CLASS_PROPERTIES.scan_period;
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
    pub stage: SettlementStageLinks,
    pub stage_var_1: i8,
    pub source_byte_39: u8,
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

pub(crate) struct SettlementContext<'a, 'stage> {
    pub source_control_flags: &'a mut u32,
    pub dynamic_residents: &'a mut [Option<DynamicResidentState>],
    pub class_5_census: &'a mut u32,
    pub player_archer_target_ticks: &'a mut u16,
    pub global_rng: &'a mut u32,
    pub building_source_order: &'a [u16],
    pub house_request: Option<BuilderHouseRequest>,
    pub death_bursts: Vec<(u16, Position)>,
    pub linked_kills: Vec<u16>,
    pub selection_rng_draw: Option<(u32, u32)>,
    pub stage: Option<stage::StageContext<'stage>>,
}

impl SettlementContext<'_, '_> {
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
                return self.step_dynamic_resident_body(world, source_views, resident, 112);
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
                        for candidate in self
                            .building_source_order
                            .iter()
                            .filter_map(|slot| source_view(source_views, *slot))
                            .filter(|candidate| candidate.villager_entry_eligible)
                        {
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
                                && self.stage.as_ref().map_or(
                                    candidate.class == resident.class
                                        && candidate.model == resident.model
                                        && candidate.life >= 0,
                                    |context| {
                                        context.models[usize::from(resident.model)]
                                            .contains(&candidate.source_entity_index)
                                    },
                                )
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
                    (view.life, view.current_attacker, view.linked_entity_index),
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
            #[cfg(test)]
            trace_builder_site(resident, building_index, candidate, "water");
            resident.reset_counter = 2;
            resident.target_entity_index = 0;
            resident.action = BUILDER_ROAM_ACTION;
            return;
        }

        let width_tiles = i16::from(spec.sprite_width) / 2 + 3;
        let height_tiles = i16::from(spec.sprite_height) / 2 + 3;
        if !builder_site_height_is_eligible(world, candidate, width_tiles, height_tiles) {
            #[cfg(test)]
            trace_builder_site(resident, building_index, candidate, "corner range");
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
            #[cfg(test)]
            trace_builder_site(resident, building_index, candidate, "source overlap");
            return;
        }

        self.house_request = Some(BuilderHouseRequest {
            candidate,
            building_index,
            spec,
        });
        #[cfg(test)]
        trace_builder_site(resident, building_index, candidate, "accepted");
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
        match consume_builder_mailbox(resident, source_views) {
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
        match consume_builder_mailbox(resident, source_views) {
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
        match consume_builder_mailbox(resident, source_views) {
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
        for candidate in self
            .building_source_order
            .iter()
            .filter_map(|slot| source_view(source_views, *slot))
        {
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
        match consume_builder_mailbox(resident, source_views) {
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
                                && self.stage.as_ref().map_or(
                                    candidate.class == resident.class
                                        && candidate.model == resident.model
                                        && candidate.life >= 0,
                                    |context| {
                                        context.models[usize::from(resident.model)]
                                            .contains(&candidate.source_entity_index)
                                    },
                                )
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
        if *self.source_control_flags & 0x800 != 0 {
            *self.source_control_flags &= !0x800;
            return;
        }
        let was_alive = resident.life >= 0;
        let transition = crate::ground_creature_motion::step_ground_creature_motion(
            resident.position,
            resident.yaw,
            resident.desired_heading,
            resident.actual_speed,
            world,
            crate::ground_creature_motion::GroundCreatureMotionConfig {
                half_width: resident.collision_half_width,
                half_height: resident.collision_half_height,
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
        self.step_dynamic_resident_body(world, source_views, resident, 104)
    }

    fn step_dynamic_resident_body<W: FlightWorld>(
        &mut self,
        world: &W,
        source_views: &[Level1SourceEntityView],
        resident: &mut DynamicResidentState,
        base: u8,
    ) -> Vec<Level1SourceEffect> {
        let mut control_flags = *self.source_control_flags;
        if resident.suppress_death_burst {
            control_flags |= 0x0010_0000;
        } else {
            control_flags &= !0x0010_0000;
        }
        let mut villager = VillagerState {
            entity_index: resident.source_entity_index,
            source_id: resident.source_id,
            model: resident.model,
            linked_entity_index: resident.linked_entity_index,
            stage: resident.stage,
            source_byte_39: resident.source_byte_39,
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
            stage_var_1: resident.stage_var_1,
            stage_var_2: resident.stage_var_2,
            actual_speed: resident.actual_speed,
            minimum_speed: resident.minimum_speed,
            maximum_speed: resident.maximum_speed,
            mana: resident.mana,
            owner_entity_index: resident.owner_entity_index,
            target_entity_index: (resident.target_entity_index != 0)
                .then_some(resident.target_entity_index),
        };
        let villager_effects = if base == 112 {
            let mut effects = Vec::new();
            if let Some(stage) = self.stage.as_mut() {
                stage::step(
                    &mut villager,
                    world,
                    VillagerBehaviorConfig::LEVEL_ONE,
                    base,
                    stage,
                    &mut effects,
                );
            } else {
                assert!(
                    !matches!(villager.stage_var_2, 1..=10 | 12..=14 | 16 | 17),
                    "scripted Trader needs its source StageVar context"
                );
            }
            villager.actual_speed = if villager.action == base + 6 {
                villager.minimum_speed
            } else {
                villager.maximum_speed
            };
            effects
        } else {
            step_villager_state_with_stage(
                &mut villager,
                world,
                source_views,
                VillagerBehaviorConfig::LEVEL_ONE,
                self.stage.as_mut(),
            )
        };

        // The population carrier owns an allocated source slot. A callback
        // hide retires drawing now; 57F20 retires the allocation next pass.
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
        resident.stage_var_1 = villager.stage_var_1;
        resident.stage = villager.stage;
        resident.source_byte_39 = villager.source_byte_39;
        resident.actual_speed = villager.actual_speed;
        resident.minimum_speed = villager.minimum_speed;
        resident.maximum_speed = villager.maximum_speed;
        resident.mana = villager.mana;
        resident.owner_entity_index = villager.owner_entity_index;
        resident.target_entity_index = villager.target_entity_index.unwrap_or(0);
        resident.movement_blocked = villager.control_flags & 0x0004_0000 != 0;
        resident.suppress_death_burst = villager.control_flags & 0x0010_0000 != 0;
        *self.source_control_flags = villager.control_flags;

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
#[cfg(test)]
fn trace_builder_site(
    resident: &DynamicResidentState,
    building: u8,
    position: Position,
    result: &str,
) {
    if std::env::var_os("MC_SETTLEMENT_TRACE").is_some() {
        eprintln!(
            "Builder {} phase {} target {} side {} BUILD {} at {:?}: {}",
            resident.source_entity_index,
            resident.phase,
            resident.target_entity_index,
            resident.reset_counter,
            building,
            position,
            result
        );
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

fn consume_builder_mailbox(
    resident: &mut DynamicResidentState,
    source_views: &[Level1SourceEntityView],
) -> BuilderMailboxResult {
    let mut hit = crate::combat::actors::consume_damage(
        &mut resident.life,
        &mut resident.pending_damage,
        &mut resident.pending_attacker,
        &mut resident.current_attacker,
    )
    .map(|hit| hit.attacker);
    let mut linked = resident.linked_entity_index;
    let mut remaining = source_views.len();
    while linked != 0 && remaining != 0 {
        remaining -= 1;
        let Some(child) = source_view(source_views, linked) else {
            break;
        };
        if crate::combat::actors::inherit_linked_life(
            &mut resident.life,
            &mut resident.current_attacker,
            child.life,
            child.current_attacker,
        ) {
            hit = Some(child.current_attacker);
            break;
        }
        linked = child.linked_entity_index;
    }
    if resident.life < 0 {
        resident.kill_credit_entity = resident.current_attacker;
        BuilderMailboxResult::Dead
    } else {
        hit.map_or(BuilderMailboxResult::None, BuilderMailboxResult::Hit)
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
    let properties = match kind {
        DynamicResidentKind::Archer => None,
        DynamicResidentKind::Builder => Some(BUILDER_CLASS_PROPERTIES),
        DynamicResidentKind::Villager => Some(VILLAGER_CLASS_PROPERTIES),
        DynamicResidentKind::Trader => Some(TRADER_CLASS_PROPERTIES),
    };
    let (minimum_speed, actual_speed, maximum_speed, reset_counter) = match properties {
        None => (30, 30, 0, i32::from(source_entity_index % 100)),
        Some(properties) => (
            properties.minimum_speed,
            properties.maximum_speed,
            properties.maximum_speed,
            2,
        ),
    };
    let (render_parameter_index, base_tmap, logical_height) = match kind {
        DynamicResidentKind::Archer => (0, 8, 250),
        DynamicResidentKind::Builder => (
            BUILDER_APPEARANCE.particle,
            BUILDER_APPEARANCE.base_tmap,
            BUILDER_APPEARANCE.logical_height,
        ),
        DynamicResidentKind::Trader => (
            TRADER_APPEARANCE.particle,
            TRADER_APPEARANCE.base_tmap,
            TRADER_APPEARANCE.logical_height,
        ),
        DynamicResidentKind::Villager => {
            entity_rng = next_rng16(entity_rng);
            let appearance = VILLAGER_APPEARANCES[usize::from(entity_rng % 9)];
            (
                appearance.particle,
                appearance.base_tmap,
                appearance.logical_height,
            )
        }
    };
    let collision_half_height = if kind == DynamicResidentKind::Archer {
        256
    } else {
        properties
            .expect("settlement constructor properties")
            .collision_half_height
    };
    DynamicResidentState {
        source_entity_index,
        source_id: source_entity_index,
        kind,
        class: 5,
        model,
        action,
        position,
        max_life: properties.map_or(1_000, |properties| properties.max_life),
        life: properties.map_or(1_000, |properties| properties.max_life),
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
        stage: SettlementStageLinks::default(),
        stage_var_1: 0,
        source_byte_39: 64,
        owner_entity_index: 0,
        reset_counter,
        mana: properties.map_or(0, |properties| properties.mana),
        stage_var_2: 0,
        builder_placement_latch: 0,
        movement_blocked: false,
        suppress_death_burst: false,
        phase,
        collision_half_width: properties.map_or(128, |properties| properties.collision_half_width),
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

/// Complete source carrier for house-created population. Common arena identity and
/// additional behavior state have one storage location; callbacks use temporary views.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2HouseResidentSource {
    pub source_entity_index: u16,
    /// Reset rows retain their package origin when adopted into this owner.
    pub initial_package: bool,
    pub entity_rng: u16,
    pub parent_house_entity_index: u16,
    pub owner_entity_index: u16,
    pub attacker_entity_index: u16,
    pub semantic_kind: crate::level_definition::CreatureKind,
    pub position: Position,
    pub model: u8,
    pub action: u8,
    pub life: i32,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
    pub source_id: u16,
    pub max_life: i32,
    pub pending_damage: i32,
    pub current_attacker: u16,
    pub kill_credit_entity: u16,
    pub yaw: i16,
    pub desired_heading: i16,
    pub minimum_speed: i16,
    pub actual_speed: i16,
    pub maximum_speed: i16,
    pub target_entity_index: u16,
    pub linked_entity_index: u16,
    pub stage: SettlementStageLinks,
    pub stage_var_1: i8,
    pub source_byte_39: u8,
    pub reset_counter: i32,
    pub mana: i32,
    pub stage_var_2: i8,
    pub builder_placement_latch: u8,
    pub movement_blocked: bool,
    pub suppress_death_burst: bool,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub render_mode: u8,
    pub logical_height: u16,
    pub control_flags: u32,
}

pub const SETTLEMENT_MAX_LIFE: i32 = 1_000;

impl Level2HouseResidentSource {
    /// Adopt the already-constructed source packet. This performs no NewEvent,
    /// constructor draw, population increment, or mutation of the reset arena.
    fn from_reset_snapshot(source: &crate::native_world::EntitySnapshot) -> Option<Self> {
        if !source.active || source.class != 5 {
            return None;
        }
        let (semantic_kind, appearance) = match source.model {
            BUILDER_MODEL => (
                crate::level_definition::CreatureKind::Builder,
                BUILDER_APPEARANCE,
            ),
            TRADER_MODEL => (
                crate::level_definition::CreatureKind::Trader,
                TRADER_APPEARANCE,
            ),
            _ => return None,
        };
        let bytes = source.record_bytes();
        let word = |offset| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let signed = |offset| i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let dword = |offset| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        Some(Self {
            source_entity_index: source.index,
            initial_package: true,
            entity_rng: source.entity_rng,
            parent_house_entity_index: 0,
            owner_entity_index: word(152),
            attacker_entity_index: word(102),
            semantic_kind,
            position: source.position,
            model: source.model,
            action: source.action,
            life: source.life,
            phase: source.phase,
            drawing_enabled: !source.drawing_disabled(),
            active: source.active,
            source_id: source.source_id,
            max_life: dword(8),
            pending_damage: dword(98),
            current_attacker: word(42),
            kill_credit_entity: word(40),
            yaw: source.yaw,
            desired_heading: signed(36),
            minimum_speed: signed(136),
            actual_speed: signed(134),
            maximum_speed: signed(138),
            target_entity_index: word(154),
            linked_entity_index: word(56),
            stage: SettlementStageLinks {
                parent: word(44),
                remaining: signed(50),
                duration: signed(52),
                relation: word(78),
            },
            stage_var_1: bytes[76] as i8,
            source_byte_39: bytes[61],
            reset_counter: dword(20),
            mana: source.mana,
            stage_var_2: bytes[77] as i8,
            builder_placement_latch: bytes[65],
            movement_blocked: source.control_flags & 0x40000 != 0,
            suppress_death_burst: source.control_flags & 0x100000 != 0,
            collision_half_width: signed(88),
            collision_vertical_offset: signed(86),
            collision_half_height: signed(92),
            render_parameter_index: word(94),
            base_tmap: appearance.base_tmap,
            render_mode: 20,
            logical_height: appearance.logical_height,
            control_flags: source.control_flags,
        })
    }

    pub(crate) fn publish_source(&self, source: &mut crate::native_world::EntitySnapshot) {
        debug_assert_eq!(source.index, self.source_entity_index);
        source.active = self.active;
        source.record[2] = u8::from(self.active);
        source.source_id = self.source_id;
        source.record[30..32].copy_from_slice(&self.source_id.to_le_bytes());
        source.class = 5;
        source.model = self.model;
        source.record[67] = 5;
        source.record[68] = self.model;
        source.set_position(self.position);
        source.set_yaw(self.yaw);
        source.record[36..38].copy_from_slice(&self.desired_heading.to_le_bytes());
        source.set_max_life(self.max_life);
        source.set_life(self.life);
        source.set_control_flags(self.control_flags);
        source.set_entity_rng(self.entity_rng);
        source.set_actual_speed(self.actual_speed);
        let descriptor = if self.model == BUILDER_MODEL {
            BUILDER_SOURCE_BINDING.descriptor
        } else {
            TRADER_SOURCE_BINDING.descriptor
        };
        source.parameter_relation = Some(descriptor);
        source.record[164..168].copy_from_slice(&(u32::from(descriptor) + 1).to_le_bytes());
        source.action = self.action;
        source.record[73] = self.action;
        source.phase = self.phase;
        source.record[66] = self.phase;
        source.record[20..24].copy_from_slice(&self.reset_counter.to_le_bytes());
        source.record[40..42].copy_from_slice(&self.kill_credit_entity.to_le_bytes());
        source.record[42..44].copy_from_slice(&self.current_attacker.to_le_bytes());
        source.record[44..46].copy_from_slice(&self.stage.parent.to_le_bytes());
        source.record[50..52].copy_from_slice(&self.stage.remaining.to_le_bytes());
        source.record[52..54].copy_from_slice(&self.stage.duration.to_le_bytes());
        source.record[56..58].copy_from_slice(&self.linked_entity_index.to_le_bytes());
        source.record[61] = self.source_byte_39;
        source.font_type_index = self.builder_placement_latch;
        source.record[65] = self.builder_placement_latch;
        source.record[76] = self.stage_var_1 as u8;
        source.record[77] = self.stage_var_2 as u8;
        source.record[78..80].copy_from_slice(&self.stage.relation.to_le_bytes());
        for (offset, value) in [
            (86, self.collision_vertical_offset),
            (88, self.collision_half_width),
            (90, self.collision_half_width),
            (92, self.collision_half_height),
            (136, self.minimum_speed),
            (138, self.maximum_speed),
        ] {
            source.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        source.particle_parameter_index = Some(self.render_parameter_index);
        source.record[94..96].copy_from_slice(&self.render_parameter_index.to_le_bytes());
        source.record[98..102].copy_from_slice(&self.pending_damage.to_le_bytes());
        source.record[102..104].copy_from_slice(&self.attacker_entity_index.to_le_bytes());
        source.mana = self.mana;
        source.record[148..152].copy_from_slice(&self.mana.to_le_bytes());
        source.record[152..154].copy_from_slice(&self.owner_entity_index.to_le_bytes());
        source.record[154..156].copy_from_slice(&self.target_entity_index.to_le_bytes());
    }

    pub(crate) fn from_resident(
        state: DynamicResidentState,
        parent_house_entity_index: u16,
        semantic_kind: crate::level_definition::CreatureKind,
    ) -> Self {
        Self {
            source_entity_index: state.source_entity_index,
            initial_package: false,
            entity_rng: state.entity_rng,
            parent_house_entity_index: parent_house_entity_index,
            owner_entity_index: state.owner_entity_index,
            attacker_entity_index: state.pending_attacker,
            semantic_kind: semantic_kind,
            position: state.position,
            model: state.model,
            action: state.action,
            life: state.life,
            phase: state.phase,
            drawing_enabled: state.visible,
            active: state.active,
            source_id: state.source_id,
            max_life: state.max_life,
            pending_damage: state.pending_damage,
            current_attacker: state.current_attacker,
            kill_credit_entity: state.kill_credit_entity,
            yaw: state.yaw,
            desired_heading: state.desired_heading,
            minimum_speed: state.minimum_speed,
            actual_speed: state.actual_speed,
            maximum_speed: state.maximum_speed,
            target_entity_index: state.target_entity_index,
            linked_entity_index: state.linked_entity_index,
            stage: state.stage,
            stage_var_1: state.stage_var_1,
            source_byte_39: state.source_byte_39,
            reset_counter: state.reset_counter,
            mana: state.mana,
            stage_var_2: state.stage_var_2,
            builder_placement_latch: state.builder_placement_latch,
            movement_blocked: state.movement_blocked,
            suppress_death_burst: state.suppress_death_burst,
            collision_half_width: state.collision_half_width,
            collision_vertical_offset: state.collision_vertical_offset,
            collision_half_height: state.collision_half_height,
            render_parameter_index: state.render_parameter_index,
            base_tmap: state.base_tmap,
            render_mode: state.render_mode,
            logical_height: state.logical_height,
            control_flags: 8,
        }
    }

    pub(crate) fn population_state(self) -> DynamicResidentState {
        DynamicResidentState {
            source_entity_index: self.source_entity_index,
            source_id: self.source_id,
            kind: match self.semantic_kind {
                crate::level_definition::CreatureKind::Builder => DynamicResidentKind::Builder,
                crate::level_definition::CreatureKind::Villager => DynamicResidentKind::Villager,
                crate::level_definition::CreatureKind::Trader => DynamicResidentKind::Trader,
                _ => unreachable!("Archer has its own arena carrier"),
            },
            class: 5,
            model: self.model,
            action: self.action,
            position: self.position,
            max_life: self.max_life,
            life: self.life,
            pending_damage: self.pending_damage,
            pending_attacker: self.attacker_entity_index,
            current_attacker: self.current_attacker,
            kill_credit_entity: self.kill_credit_entity,
            entity_rng: self.entity_rng,
            yaw: self.yaw,
            desired_heading: self.desired_heading,
            minimum_speed: self.minimum_speed,
            actual_speed: self.actual_speed,
            maximum_speed: self.maximum_speed,
            target_entity_index: self.target_entity_index,
            linked_entity_index: self.linked_entity_index,
            stage: self.stage,
            stage_var_1: self.stage_var_1,
            source_byte_39: self.source_byte_39,
            owner_entity_index: self.owner_entity_index,
            reset_counter: self.reset_counter,
            mana: self.mana,
            stage_var_2: self.stage_var_2,
            builder_placement_latch: self.builder_placement_latch,
            movement_blocked: self.movement_blocked,
            suppress_death_burst: self.suppress_death_burst,
            phase: self.phase,
            collision_half_width: self.collision_half_width,
            collision_vertical_offset: self.collision_vertical_offset,
            collision_half_height: self.collision_half_height,
            render_parameter_index: self.render_parameter_index,
            base_tmap: self.base_tmap,
            render_mode: self.render_mode,
            logical_height: self.logical_height,
            active: self.active,
            visible: self.drawing_enabled,
        }
    }

    pub(crate) fn commit_population(&mut self, state: DynamicResidentState) {
        self.source_entity_index = state.source_entity_index;
        self.entity_rng = state.entity_rng;
        self.owner_entity_index = state.owner_entity_index;
        self.attacker_entity_index = state.pending_attacker;
        self.position = state.position;
        self.model = state.model;
        self.action = state.action;
        self.life = state.life;
        self.phase = state.phase;
        self.drawing_enabled = state.visible;
        self.source_id = state.source_id;
        self.max_life = state.max_life;
        self.pending_damage = state.pending_damage;
        self.current_attacker = state.current_attacker;
        self.kill_credit_entity = state.kill_credit_entity;
        self.yaw = state.yaw;
        self.desired_heading = state.desired_heading;
        self.minimum_speed = state.minimum_speed;
        self.actual_speed = state.actual_speed;
        self.maximum_speed = state.maximum_speed;
        self.target_entity_index = state.target_entity_index;
        self.linked_entity_index = state.linked_entity_index;
        self.reset_counter = state.reset_counter;
        self.mana = state.mana;
        self.stage_var_2 = state.stage_var_2;
        self.stage = state.stage;
        self.stage_var_1 = state.stage_var_1;
        self.source_byte_39 = state.source_byte_39;
        self.builder_placement_latch = state.builder_placement_latch;
        self.movement_blocked = state.movement_blocked;
        self.suppress_death_burst = state.suppress_death_burst;
        self.collision_half_width = state.collision_half_width;
        self.collision_vertical_offset = state.collision_vertical_offset;
        self.collision_half_height = state.collision_half_height;
        self.render_parameter_index = state.render_parameter_index;
        self.base_tmap = state.base_tmap;
        self.render_mode = state.render_mode;
        self.logical_height = state.logical_height;
        // Hide retains the numeric row until following-pass sub_57F20.
        self.drawing_enabled &= state.active;
    }

    pub(crate) fn digest_behavior(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        if self.initial_package {
            hasher.update(b"settlement-reset-origin-v1\0");
        }
        hasher.update(self.source_id.to_le_bytes());
        hasher.update(self.max_life.to_le_bytes());
        hasher.update(self.pending_damage.to_le_bytes());
        hasher.update(self.current_attacker.to_le_bytes());
        hasher.update(self.kill_credit_entity.to_le_bytes());
        hasher.update(self.yaw.to_le_bytes());
        hasher.update(self.desired_heading.to_le_bytes());
        hasher.update(self.minimum_speed.to_le_bytes());
        hasher.update(self.actual_speed.to_le_bytes());
        hasher.update(self.maximum_speed.to_le_bytes());
        hasher.update(self.target_entity_index.to_le_bytes());
        hasher.update(self.linked_entity_index.to_le_bytes());
        hasher.update(self.reset_counter.to_le_bytes());
        hasher.update(self.mana.to_le_bytes());
        hasher.update(self.stage_var_2.to_le_bytes());
        if self.stage_var_1 != 0 || self.stage != Default::default() || self.source_byte_39 != 64 {
            hasher.update(b"settlement-stage-v1\0");
            hasher.update(self.stage_var_1.to_le_bytes());
            hasher.update(self.stage.parent.to_le_bytes());
            hasher.update(self.stage.relation.to_le_bytes());
            hasher.update(self.stage.remaining.to_le_bytes());
            hasher.update(self.stage.duration.to_le_bytes());
            hasher.update([self.source_byte_39]);
        }
        hasher.update(self.builder_placement_latch.to_le_bytes());
        hasher.update([u8::from(self.movement_blocked)]);
        hasher.update([u8::from(self.suppress_death_burst)]);
        hasher.update(self.collision_half_width.to_le_bytes());
        hasher.update(self.collision_vertical_offset.to_le_bytes());
        hasher.update(self.collision_half_height.to_le_bytes());
        hasher.update(self.render_parameter_index.to_le_bytes());
        hasher.update(self.base_tmap.to_le_bytes());
        hasher.update(self.render_mode.to_le_bytes());
        hasher.update(self.logical_height.to_le_bytes());
        hasher.update(self.control_flags.to_le_bytes());
    }
}

/// Constructor counters and residents are one reset/save/hash-owned population.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthoredTraderBehavior {
    pub source_id: u16,
    pub control_flags: u32,
    pub owner: u16,
    pub index: i8,
    pub mode: i8,
    pub links: SettlementStageLinks,
}

impl AuthoredTraderBehavior {
    pub(crate) fn new(slot: u16) -> Self {
        Self {
            source_id: slot,
            control_flags: 8,
            owner: 0,
            index: 0,
            mode: 0,
            links: SettlementStageLinks::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettlementPopulation {
    residents: Vec<Level2HouseResidentSource>,
    builder_phase: u8,
    villager_phase: u8,
    /// Dynamic settlement rows admitted by the current sub_60F00 boundary.
    /// Generic economy mutations must retain these same generation-bound rows.
    mana_sources: Vec<(u16, u32)>,
    pub(crate) traders: std::collections::BTreeMap<u16, AuthoredTraderBehavior>,
}
impl SettlementPopulation {
    pub(crate) fn new(builder_phase: u8, villager_phase: u8) -> Self {
        Self {
            residents: Vec::new(),
            builder_phase,
            villager_phase,
            mana_sources: Vec::new(),
            traders: std::collections::BTreeMap::new(),
        }
    }
    pub(crate) fn take_constructor_phase(&mut self, kind: DynamicResidentKind) -> u8 {
        let phase = match kind {
            DynamicResidentKind::Builder => &mut self.builder_phase,
            DynamicResidentKind::Villager => &mut self.villager_phase,
            _ => unreachable!("Archer/Trader constructor counters have their existing owner"),
        };
        let current = *phase;
        *phase = phase.wrapping_add(1);
        current
    }
    pub(crate) fn digest_counters(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        hasher.update([self.builder_phase, self.villager_phase]);
        if !self.traders.is_empty() {
            hasher.update(b"settlement-authored-trader-v1\0");
            for (slot, state) in &self.traders {
                hasher.update(slot.to_le_bytes());
                hasher.update(state.source_id.to_le_bytes());
                hasher.update(state.control_flags.to_le_bytes());
                hasher.update(state.owner.to_le_bytes());
                hasher.update([state.index as u8, state.mode as u8]);
                hasher.update(state.links.parent.to_le_bytes());
                hasher.update(state.links.relation.to_le_bytes());
                hasher.update(state.links.remaining.to_le_bytes());
                hasher.update(state.links.duration.to_le_bytes());
            }
        }
        if !self.mana_sources.is_empty() {
            hasher.update(b"settlement-mana-sources-v1\0");
            for (slot, generation) in &self.mana_sources {
                hasher.update(slot.to_le_bytes());
                hasher.update(generation.to_le_bytes());
            }
        }
    }
}
impl std::ops::Deref for SettlementPopulation {
    type Target = Vec<Level2HouseResidentSource>;
    fn deref(&self) -> &Self::Target {
        &self.residents
    }
}
impl std::ops::DerefMut for SettlementPopulation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.residents
    }
}
impl<'a> IntoIterator for &'a SettlementPopulation {
    type Item = &'a Level2HouseResidentSource;
    type IntoIter = std::slice::Iter<'a, Level2HouseResidentSource>;
    fn into_iter(self) -> Self::IntoIter {
        self.residents.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettlementPopulationStep {
    pub feedback_entity: Option<u16>,
    pub spell_awards: Vec<(u16, u8)>,
}

pub(crate) struct SettlementVillagerStep {
    pub effects: Vec<VillagerStepEffect>,
    pub spell_awards: Vec<(u16, u8)>,
}
impl crate::native_world::Level2EconomyState {
    pub(crate) fn settlement_mana_source_slots(&self) -> impl Iterator<Item = u16> + '_ {
        self.house_residents
            .mana_sources
            .iter()
            .filter_map(|&(slot, generation)| {
                (self.slot_generation(slot) == generation).then_some(slot)
            })
    }

    pub(crate) fn rebuild_settlement_mana_rows(&mut self, houses: &[crate::house::HouseState]) {
        // Derived source-boundary projection only. The House and resident
        // carriers remain the sole mutable gameplay authority.
        self.owner_mana_rows.retain(|row| {
            !houses
                .iter()
                .any(|house| house.entity_index == row.source_entity_index)
        });
        for house in houses.iter().filter(|house| house.active) {
            let mana = if house.parameter.flags & 8 == 0 {
                (1_000_i32 * i32::from(house.parameter.life_mana_scalar)) >> 7
            } else {
                0
            };
            self.install_owner_mana_row(house.entity_index, 10, 45, house.owner_entity_index, mana);
        }
        let residents = self
            .house_residents
            .iter()
            .filter(|row| row.active)
            .copied()
            .collect::<Vec<_>>();
        for row in residents {
            self.install_owner_mana_row(
                row.source_entity_index,
                5,
                row.model,
                row.owner_entity_index,
                row.mana,
            );
        }
        let mut sources = houses
            .iter()
            .filter(|house| house.active)
            .map(|house| house.entity_index)
            .chain(
                self.house_residents
                    .iter()
                    .filter(|row| row.active)
                    .map(|row| row.source_entity_index),
            )
            .filter_map(|slot| {
                self.slot_generations
                    .get(&slot)
                    .map(|&generation| (slot, generation))
            })
            .collect::<Vec<_>>();
        sources.sort_unstable();
        sources.dedup();
        self.house_residents.mana_sources = sources;
        self.publish_owner_mana_projection();
    }

    /// RemoveCastleStage writes this after each constructor, even if the
    /// demolishing source has no owner. Ordinary population births retain zero.
    pub(crate) fn initialize_demolition_resident_mailbox(&mut self, slot: u16, attacker: u16) {
        if let Some(row) = self
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
        {
            row.pending_damage = 1;
            row.attacker_entity_index = attacker;
        } else if let Some(row) = self
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot && row.model == 4)
        {
            row.pending_word_0x62 = attacker;
            if let Some(state) = self
                .authored_class5_archer
                .iter_mut()
                .find(|state| state.source_entity_index == slot)
            {
                state.pending_damage = 1;
            }
        }
    }

    pub(crate) fn allocate_settlement_resident(
        &mut self,
        parent_house_entity_index: u16,
        _house_owner_entity_index: u16,
        request: crate::level1_mission::HouseResidentRequest,
        terrain: &crate::native_world::SessionTerrain,
    ) -> Option<u16> {
        if request.model == 4 {
            return self.allocate_house_archer(parent_house_entity_index, 0, request, terrain);
        }
        crate::campaign::mc2_level_definition::Mc2LevelDefinition::creature_for_house_resident(
            request.model,
            request.action,
        )?;
        let slot = self.allocate_slot()?;
        self.initialize_settlement_resident(slot, parent_house_entity_index, request)
    }

    /// Constructor tail after the canonical SourceArena has assigned a slot.
    pub(crate) fn initialize_settlement_resident(
        &mut self,
        slot: u16,
        parent_house_entity_index: u16,
        request: crate::level1_mission::HouseResidentRequest,
    ) -> Option<u16> {
        let semantic_kind =
            crate::campaign::mc2_level_definition::Mc2LevelDefinition::creature_for_house_resident(
                request.model,
                request.action,
            )?;
        let kind = match semantic_kind {
            crate::level_definition::CreatureKind::Builder => DynamicResidentKind::Builder,
            crate::level_definition::CreatureKind::Villager => DynamicResidentKind::Villager,
            crate::level_definition::CreatureKind::Trader => DynamicResidentKind::Trader,
            _ => return None,
        };
        let initial_rng = self.new_event_rng(slot);
        let phase = if kind == DynamicResidentKind::Trader {
            let phase = self.class5_model14_constructor_phase;
            self.class5_model14_constructor_phase = phase.wrapping_add(1);
            phase
        } else {
            self.house_residents.take_constructor_phase(kind)
        };
        let mut state = resident_constructor(
            slot,
            initial_rng,
            phase,
            request.position,
            kind,
            request.model,
            request.action,
            request.attacker_entity_index,
        );
        // Ordinary births have no damage payload. Demolition callers write
        // their explicit payload after construction, including ownerless bursts.
        state.pending_damage = 0;
        // GetRandManaSphere and the family constructors do not copy the
        // possessing house's player relation into the new neutral resident.
        let source = Level2HouseResidentSource::from_resident(
            state,
            parent_house_entity_index,
            semantic_kind,
        );
        if self
            .map_chains
            .sync_entity(crate::renderable_map_chain::RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: state.position.x,
                y: state.position.y,
            })
            .is_err()
        {
            self.release_slot(slot, Some(state.entity_rng));
            return None;
        }
        self.source_rng.insert(slot, state.entity_rng);
        self.source_owner.insert(slot, state.owner_entity_index);
        self.owner_class.insert(slot, 5);
        self.house_residents.push(source);
        Some(slot)
    }

    /// Construction burial writes life and both credit words directly;
    /// it neither consumes nor synthesizes an ordinary damage mailbox.
    pub(crate) fn apply_settlement_construction_kill(&mut self, slot: u16, source_id: u16) -> bool {
        let Some(row) = self
            .house_residents
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        else {
            return false;
        };
        row.life = -1;
        row.kill_credit_entity = source_id;
        row.current_attacker = source_id;
        true
    }

    pub(crate) fn submit_settlement_damage(
        &mut self,
        slot: u16,
        damage: i32,
        attacker: u16,
        writer: crate::combat::actors::DamageWriter,
    ) -> bool {
        let Some(row) = self
            .house_residents
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        else {
            return false;
        };
        crate::combat::actors::DamageMailboxMut::new(
            &mut row.pending_damage,
            &mut row.attacker_entity_index,
        )
        .submit(damage, attacker, writer)
    }
}

impl crate::campaign::source_arena::SourceArena {
    /// Reset Builders/Traders use the same callbacks and lifetime carrier as
    /// later population births. Imported Villagers retain their existing owner.
    pub(crate) fn adopt_reset_settlement_sources(&mut self) {
        for source in &self.entities {
            if self
                .economy
                .house_residents
                .iter()
                .any(|row| row.source_entity_index == source.index)
                || self
                    .economy
                    .authored_class5
                    .iter()
                    .any(|row| row.source_entity_index == source.index)
            {
                continue;
            }
            if let Some(resident) = Level2HouseResidentSource::from_reset_snapshot(source) {
                self.economy.house_residents.push(resident);
            }
        }
    }

    /// sub_23340/sub_237B0 mutate the live household regardless of whether
    /// it came from reset data, a Builder, or an authored group constructor.
    pub(crate) fn try_enter_settlement_house(
        &mut self,
        houses: &mut crate::house::HouseFamily,
        slot: u16,
    ) -> crate::house::HouseResidentEntry {
        use crate::house::HouseResidentEntry;
        let result = houses.try_enter_resident(slot);
        if result != HouseResidentEntry::NotLive {
            if result == HouseResidentEntry::Entered
                && let Some(house) = houses
                    .houses()
                    .iter()
                    .find(|house| house.entity_index == slot)
            {
                house.publish_source(&mut self.entities[usize::from(slot)]);
            }
            return result;
        }
        let Some(house) = self.economy.authored_model45.iter_mut().find(|house| {
            house.active
                && house.source_entity_index == slot
                && house.source_class == 10
                && house.model == 45
        }) else {
            return HouseResidentEntry::NotLive;
        };
        if house.dword_0x10 >= i32::from(house.minimum_speed) {
            return HouseResidentEntry::Full;
        }
        house.dword_0x10 = house.dword_0x10.wrapping_add(1);
        let occupants = house.dword_0x10;
        self.entities[usize::from(slot)].set_source_dword_10(occupants);
        HouseResidentEntry::Entered
    }

    pub(crate) fn initialize_demolition_resident_mailbox(&mut self, slot: u16, attacker: u16) {
        self.economy
            .initialize_demolition_resident_mailbox(slot, attacker);
        if let Some(row) = self
            .house_residents
            .iter()
            .find(|row| row.source_entity_index == slot)
            .copied()
        {
            row.publish_source(&mut self.entities[usize::from(slot)]);
        }
    }

    /// Ordinary House births use the same frozen NewEvent allocator as all
    /// other source constructors, before entering their family constructor.
    pub(crate) fn allocate_house_resident(
        &mut self,
        parent_house: u16,
        _owner: u16,
        request: crate::level1_mission::HouseResidentRequest,
        terrain: &mut crate::native_world::SessionTerrain,
    ) -> Option<u16> {
        if request.model == 4 {
            let slot = self.allocate_source_slot_with_terrain(terrain)?;
            self.economy
                .initialize_house_archer(slot, parent_house, request, terrain)?;
            let position = self
                .authored_class5
                .iter()
                .find(|row| row.source_entity_index == slot)?
                .position;
            crate::world::effects::sync_position(self, terrain, slot, position, true, false)
                .ok()?;
            return Some(slot);
        }
        crate::campaign::mc2_level_definition::Mc2LevelDefinition::creature_for_house_resident(
            request.model,
            request.action,
        )?;
        let slot = self.allocate_source_slot_with_terrain(terrain)?;
        self.economy
            .initialize_settlement_resident(slot, parent_house, request)?;
        let source = *self
            .house_residents
            .iter()
            .find(|row| row.source_entity_index == slot)?;
        self.entities[usize::from(slot)] = crate::native_world::cave_features::new_event(
            slot,
            slot.wrapping_add(self.global_rng as u16),
            source.position,
        );
        let raw = &mut self.entities[usize::from(slot)];
        raw.set_subspell_index(crate::native_world::level2_class5::trader::TRADER_SUBSPELL_INDEX);
        raw.pitch = source.yaw;
        raw.record[34..36].copy_from_slice(&source.yaw.to_le_bytes());
        raw.record[60] = 1;
        raw.record[69] = 3;
        raw.record[97] = 1;
        source.publish_source(&mut self.entities[usize::from(slot)]);
        crate::world::effects::sync_position(self, terrain, slot, source.position, true, false)
            .ok()?;
        Some(slot)
    }

    /// An ephemeral callback view over actual source, house and population
    /// carriers. Dynamic occupants replace stale reset records by source slot.
    pub(crate) fn settlement_source_views(
        &self,
        houses: &crate::house::HouseFamily,
        villagers: &[crate::villager::VillagerState],
        parameters: &std::collections::BTreeMap<u8, crate::house::HouseParameter>,
    ) -> Vec<crate::level1_mission::Level1SourceEntityView> {
        use crate::level1_mission::{Level1ResetSourceSlot, Level1SourceEntityView};
        let mut views = std::collections::BTreeMap::new();
        for row in self
            .entities
            .iter()
            .filter(|row| row.active && row.index != 0)
        {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.index,
                active: true,
                class: row.class,
                model: row.model,
                action: row.action,
                phase: row.phase,
                entity_rng: row.entity_rng,
                control_flags: row.control_flags,
            });
            view.source_id = row.source_id;
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.owner_entity_index = row.source_player_entity_index();
            view.current_attacker =
                u16::from_le_bytes([row.record_bytes()[42], row.record_bytes()[43]]);
            view.linked_entity_index = row.word_34();
            view.collision_half_width = row.array_pitch();
            view.collision_half_depth = row.array_roll();
            view.collision_vertical_offset = row.array_yaw();
            view.collision_half_height = row.array_fov();
            views.insert(row.index, view);
        }
        for row in self.authored_model45.iter().filter(|row| row.active) {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.source_entity_index,
                active: true,
                class: row.source_class,
                model: row.model,
                action: row.action,
                phase: row.source_byte_3e,
                entity_rng: row.entity_rng,
                control_flags: row.control_flags,
            });
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.collision_half_width = row.view_pitch;
            view.collision_half_depth = row.view_roll;
            view.collision_vertical_offset = row.view_yaw;
            view.collision_half_height = row.view_fov;
            view.villager_entry_eligible = parameters
                .get(&row.byte_0x46)
                .is_some_and(|parameter| parameter.produces_villagers());
            views.insert(row.source_entity_index, view);
        }
        for row in self.authored_class5.iter().filter(|row| row.active) {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.source_entity_index,
                active: true,
                class: 5,
                model: row.model,
                action: row.action,
                phase: row.source_byte_3e,
                entity_rng: row.entity_rng,
                control_flags: if row.drawing_enabled { 8 } else { 0x408 },
            });
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.target_entity_index = row.word_0x96_150;
            view.collision_half_width = row.shift_rot_x;
            view.collision_half_depth = row.shift_rot_x;
            view.collision_half_height = row.shift_rot_y;
            views.insert(row.source_entity_index, view);
        }
        for row in houses.houses().iter().filter(|row| {
            row.source_retained()
                && (row.active
                    || self.authored_house_record(row.entity_index).is_some()
                    || self
                        .entities
                        .get(usize::from(row.entity_index))
                        .is_some_and(|raw| raw.active && raw.class == 10 && raw.model == 45))
        }) {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.entity_index,
                active: row.source_retained(),
                class: 10,
                model: 45,
                action: row.action,
                phase: row.phase,
                entity_rng: row.rng,
                control_flags: row.control_flags,
            });
            view.source_id = row.source_id;
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.owner_entity_index = row.owner_entity_index;
            view.collision_half_width = row.collision_half_width;
            view.collision_half_depth = row.collision_half_depth;
            view.collision_vertical_offset = row.collision_vertical_offset;
            view.collision_half_height = row.collision_half_height;
            view.villager_entry_eligible = row.parameter.produces_villagers();
            views.insert(row.entity_index, view);
        }
        for row in villagers.iter().filter(|row| row.source_retained()) {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.entity_index,
                active: true,
                class: 5,
                model: row.model,
                action: row.action,
                phase: row.phase,
                entity_rng: row.rng,
                control_flags: row.control_flags,
            });
            view.source_id = row.source_id;
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.owner_entity_index = row.owner_entity_index;
            view.linked_entity_index = row.linked_entity_index;
            view.current_attacker = row.current_attacker.cast_unsigned();
            view.collision_half_width = row.collision_half_width;
            view.collision_half_depth = row.collision_half_width;
            view.collision_vertical_offset = row.collision_vertical_offset;
            view.collision_half_height = row.collision_half_height;
            views.insert(row.entity_index, view);
        }
        for row in self.house_residents.iter().filter(|row| row.active) {
            let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
                index: row.source_entity_index,
                active: true,
                class: 5,
                model: row.model,
                action: row.action,
                phase: row.phase,
                entity_rng: row.entity_rng,
                control_flags: row.control_flags,
            });
            view.source_id = row.source_id;
            view.position = row.position;
            view.life = row.life;
            view.max_life = row.max_life;
            view.owner_entity_index = row.owner_entity_index;
            view.target_entity_index = row.target_entity_index;
            view.linked_entity_index = row.linked_entity_index;
            view.collision_half_width = row.collision_half_width;
            view.collision_half_depth = row.collision_half_width;
            view.collision_vertical_offset = row.collision_vertical_offset;
            view.collision_half_height = row.collision_half_height;
            view.builder_placement_latch = row.builder_placement_latch;
            view.current_attacker = row.current_attacker;
            views.insert(row.source_entity_index, view);
        }
        views.into_values().collect()
    }

    pub(crate) fn step_settlement_resident_slot(
        &mut self,
        slot: u16,
        terrain: &mut crate::native_world::SessionTerrain,
        houses: &mut crate::house::HouseFamily,
        villagers: &mut crate::villager::VillagerFamily,
        building_source_order: &[u16],
        parameters: &std::collections::BTreeMap<u8, crate::house::HouseParameter>,
        footprints: &std::collections::BTreeMap<u8, crate::level1_mission::CastleBuildFootprint>,
        runtime: &mut crate::native_world::RuntimeSnapshot,
        models: &[Vec<u16>; 29],
        players: &[u16],
        particles: &[crate::native_world::ParticleParameter],
    ) -> Result<Option<SettlementPopulationStep>, String> {
        let Some(index) = self
            .house_residents
            .iter()
            .position(|row| row.source_entity_index == slot && row.active && row.drawing_enabled)
        else {
            return Ok(None);
        };
        let mut views = self.settlement_source_views(houses, villagers.villagers(), parameters);
        for view in &mut views {
            view.villager_entry_eligible &=
                building_source_order.contains(&view.source_entity_index);
        }
        let mut source = self.house_residents[index];
        let mut resident = source.population_state();
        let targets = stage::targets(self, &views, villagers.villagers());
        let mut linked_residents = Vec::new();
        if resident.linked_entity_index != 0 {
            linked_residents.resize(1000, None);
            for row in self.house_residents.iter() {
                linked_residents[usize::from(row.source_entity_index)] =
                    Some(row.population_state());
            }
        }
        let economy = &mut self.economy;
        let mut feedback = 0;
        let mut context = SettlementContext {
            stage: Some(stage::StageContext {
                runtime: Some(runtime),
                targets: &targets,
                models,
                players,
                effects: Vec::new(),
            }),
            source_control_flags: &mut source.control_flags,
            dynamic_residents: &mut linked_residents,
            class_5_census: &mut economy.source_creature_population,
            player_archer_target_ticks: &mut feedback,
            global_rng: &mut economy.global_rng,
            building_source_order,
            house_request: None,
            death_bursts: Vec::new(),
            linked_kills: Vec::new(),
            selection_rng_draw: None,
        };
        let effects = match resident.kind {
            DynamicResidentKind::Builder => {
                context.step_dynamic_builder(terrain, &views, &mut resident)
            }
            DynamicResidentKind::Villager => {
                context.step_dynamic_villager(terrain, &views, &mut resident)
            }
            DynamicResidentKind::Trader => {
                context.step_dynamic_trader(terrain, &views, &mut resident)
            }
            DynamicResidentKind::Archer => unreachable!("Archer has its own source owner"),
        };
        let request = context.house_request;
        let bursts = std::mem::take(&mut context.death_bursts);
        let linked_kills = std::mem::take(&mut context.linked_kills);
        let stage_effects = context
            .stage
            .as_mut()
            .map(|stage| std::mem::take(&mut stage.effects))
            .unwrap_or_default();
        drop(context);
        for effect in effects {
            match effect {
                Level1SourceEffect::TryEnterVillager {
                    house_source_entity_index,
                    ..
                } => {
                    if self.try_enter_settlement_house(houses, house_source_entity_index)
                        != crate::house::HouseResidentEntry::Entered
                    {
                        resident.reset_counter = 0;
                        resident.target_entity_index = 0;
                        resident.actual_speed = resident.maximum_speed;
                        resident.action = if resident.kind == DynamicResidentKind::Trader {
                            TRADER_FREE_ACTION
                        } else {
                            105
                        };
                    }
                }
                Level1SourceEffect::DisableSourceSlot { .. } => resident.visible = false,
                _ => return Err("unexpected settlement family effect".to_owned()),
            }
        }
        for linked in linked_kills {
            if let Some(row) = self
                .house_residents
                .iter_mut()
                .find(|row| row.source_entity_index == linked)
            {
                row.action = 117;
            }
        }
        for (dead_slot, position) in bursts {
            let _ =
                self.spawn_death_burst_from_death_with_id(dead_slot, resident.source_id, position);
        }
        if let Some(request) = request {
            let parameter = *parameters
                .get(&request.building_index)
                .ok_or_else(|| format!("Builder BLDGPRM row {} absent", request.building_index))?;
            let footprint = footprints
                .get(&request.building_index)
                .ok_or_else(|| format!("Builder BUILD row {} absent", request.building_index))?;
            if let Some(house_slot) = self.allocate_source_slot_with_terrain(terrain) {
                let rng = self.new_event_rng(house_slot);
                let position = builder_house_position(terrain, request.candidate, request.spec);
                let mut house = crate::house::HouseState::builder_from_source(
                    house_slot,
                    rng,
                    position,
                    request.building_index,
                    footprint.width,
                    footprint.height,
                    parameter,
                );
                houses
                    .insert_builder_house(house)
                    .map_err(|error| format!("Builder house insert: {error}"))?;
                crate::mark_source_audio_call(crate::SourceAudioCallTag::BuilderBuildAction96);
                self.prepare_sound(slot, resident.source_id, -1, BUILDER_BUILD_SOUND);
                house.action = crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
                houses.initialize_builder_house(house)?;
                self.source_rng.insert(house_slot, rng);
                self.source_owner.insert(house_slot, 0);
                self.owner_class.insert(house_slot, 10);
                let mut raw =
                    crate::native_world::cave_features::new_event(house_slot, rng, position);
                crate::house::bind_startup_construction(
                    &mut raw,
                    request.building_index,
                    parameter,
                    footprint,
                    u16::from(footprint.width) * u16::from(footprint.height) * 4,
                );
                house.publish_source(&mut raw);
                self.entities[usize::from(house_slot)] = raw;
                crate::world::effects::sync_position(
                    self, terrain, house_slot, position, true, false,
                )?;
            }
        }
        resident.phase = resident.phase.wrapping_add(1);
        source.commit_population(resident);
        if !source.drawing_enabled {
            source.control_flags |= crate::level1_mission::SOURCE_DISABLED_FLAG;
        }
        self.house_residents[index] = source;
        self.source_rng.insert(slot, source.entity_rng);
        self.source_owner.insert(slot, source.owner_entity_index);
        source.publish_source(&mut self.entities[usize::from(slot)]);
        crate::world::effects::sync_position(
            self,
            terrain,
            slot,
            source.position,
            source.active,
            false,
        )?;
        let spell_awards =
            self::runtime::apply_effects(self, villagers, terrain, particles, stage_effects)?;
        Ok(Some(SettlementPopulationStep {
            feedback_entity: (feedback != 0).then_some(
                if matches!(source.action, 101 | 109 | 117) {
                    source.kill_credit_entity
                } else {
                    source.current_attacker
                },
            ),
            spell_awards,
        }))
    }
}

impl crate::native_world::Level2OrdinaryResetSession {
    /// Keep current House state visible to generic arena consumers, including
    /// callback turns that emit no terrain event.
    pub(crate) fn publish_house_source(
        &self,
        arena: &mut crate::campaign::source_arena::SourceArena,
        slot: u16,
    ) {
        if let Some(house) = self
            .houses
            .houses()
            .iter()
            .find(|house| house.entity_index == slot)
        {
            house.publish_source(&mut arena.entities[usize::from(slot)]);
        }
    }

    /// The family-owned part of 57F20, shared by UpdateEntities and 49F90.
    /// Retire the authoritative row before a free address can be reused.
    pub(crate) fn reclaim_for_source_boundary(
        &mut self,
        arena: &mut crate::campaign::source_arena::SourceArena,
        terrain: &mut crate::native_world::SessionTerrain,
    ) -> Result<(), String> {
        let mut retired = self
            .houses
            .reclaim_inactive()
            .into_iter()
            .filter_map(|house| {
                let imported = arena
                    .entities
                    .get(usize::from(house.entity_index))
                    .is_some_and(|raw| {
                        raw.active && raw.class == 10 && raw.model == 45 && raw.drawing_disabled()
                    });
                let constructed = arena.economy.owner_class.get(&house.entity_index) == Some(&10)
                    && arena.economy.source_owner.contains_key(&house.entity_index);
                (imported
                    || constructed
                    || arena.authored_house_record(house.entity_index).is_some())
                .then_some((house.entity_index, house.rng, house.position, true))
            })
            .collect::<Vec<_>>();
        retired.extend(self.villagers.villagers().iter().filter_map(|villager| {
            (!villager.active
                && arena
                    .entities
                    .get(usize::from(villager.entity_index))
                    .is_some_and(|raw| {
                        raw.active
                            && raw.class == 5
                            && raw.model == villager.model
                            && raw.drawing_disabled()
                    }))
            .then_some((
                villager.entity_index,
                villager.rng,
                villager.position,
                false,
            ))
        }));
        self.villagers.reclaim_inactive();
        retired.extend(arena.house_residents.iter().filter_map(|resident| {
            (!resident.active || !resident.drawing_enabled).then_some((
                resident.source_entity_index,
                resident.entity_rng,
                resident.position,
                false,
            ))
        }));
        arena
            .house_residents
            .retain(|resident| resident.active && resident.drawing_enabled);
        retired.sort_unstable_by_key(|row| row.0);
        for (slot, rng, position, house) in retired {
            crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
            if let Some(raw) = arena
                .entities
                .get_mut(usize::from(slot))
                .filter(|raw| raw.active && raw.drawing_disabled())
            {
                raw.reclaim_source_row();
            }
            if house {
                arena
                    .economy
                    .reclaim_house_source_slot_at_source_boundary(slot, rng);
            } else {
                arena.economy.source_owner.remove(&slot);
                arena.economy.owner_class.remove(&slot);
                arena.economy.source_rng.remove(&slot);
                arena.economy.release_slot(slot, Some(rng));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod production_tests {
    use super::*;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use std::{collections::BTreeMap, path::PathBuf};

    fn ordinary_level_two() -> crate::native_world::Mc2Simulation {
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE")
            .map(PathBuf::from)
            .unwrap_or_else(|| private.join("generated/mc-1pb-16-level2-full-build-v2"));
        // The current packet carries the complete retained BUILD bank. The
        // normal simulation entry needs no separately bound presentation or
        // Castle cache; an explicitly supplied cache still gets its full
        // identity validation through the existing cached entry point.
        let session = if let Some(cache) = std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5") {
            CampaignSession::open_with_owner_cache(
                package,
                PathBuf::from(cache),
                CampaignSelection::LEVEL2,
            )
        } else {
            CampaignSession::open(package, CampaignSelection::LEVEL2)
        };
        session
            .expect("actual Level2 package with its retained owner BUILD bank")
            .simulation()
            .clone()
    }

    fn ordinary_level_seven() -> crate::native_world::Mc2Simulation {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261024-b/level7");
        CampaignSession::open(package, CampaignSelection::ordinary(7).unwrap())
            .expect("actual ordinary Level7 package")
            .simulation()
            .clone()
    }

    #[test]
    fn house_born_villager_death_feedback_uses_kill_credit() {
        let mut session = ordinary_level_two();
        let player = session.state.entity_index;
        let live = &mut session.session.state.live;
        let house = live.ordinary.houses.houses()[0];
        let slot = live
            .economy
            .allocate_house_resident(
                house.entity_index,
                0,
                crate::level1_mission::HouseResidentRequest {
                    position: house.position,
                    model: 13,
                    action: 105,
                    attacker_entity_index: 0,
                },
                &mut live.terrain,
            )
            .unwrap();
        let resident = live
            .economy
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        // The combat mailbox has already selected action108 and kill credit.
        // Current attacker deliberately differs to check the returned consumer key.
        resident.action = 108;
        resident.reset_counter = 0;
        resident.kill_credit_entity = player;
        resident.current_attacker = 0;
        let resident = *resident;
        resident.publish_source(&mut live.economy.entities[usize::from(slot)]);
        let models = std::array::from_fn(|_| Vec::new());
        let step = live
            .ordinary
            .step_settlement_source(
                &mut live.economy,
                &mut live.terrain,
                slot,
                &[],
                &mut live.core.runtime,
                &models,
                &[player],
                &session.session.definition.presentation.particles,
            )
            .unwrap()
            .unwrap();
        assert_eq!(step.feedback_entity, Some(player));
        assert_eq!(live.economy.entities[usize::from(slot)].action, 109);
    }

    #[test]
    fn house_born_settlements_consume_forced_motion_latch_once() {
        let mut session = ordinary_level_two();
        let live = &mut session.session.state.live;
        let house = live.ordinary.houses.houses()[0];
        let models = std::array::from_fn(|_| Vec::new());
        for (model, action) in [(12, 97), (13, 105), (14, 113)] {
            let slot = live
                .economy
                .allocate_house_resident(
                    house.entity_index,
                    0,
                    crate::level1_mission::HouseResidentRequest {
                        position: house.position,
                        model,
                        action,
                        attacker_entity_index: 0,
                    },
                    &mut live.terrain,
                )
                .unwrap();
            let resident = live
                .economy
                .house_residents
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            resident.phase = 1;
            // Controlled output of sub_33340: the wind has already supplied
            // this position/yaw and requests skipping one ground move.
            resident.control_flags |= 0x800;
            let before = *resident;
            before.publish_source(&mut live.economy.entities[usize::from(slot)]);
            live.ordinary
                .step_settlement_source(
                    &mut live.economy,
                    &mut live.terrain,
                    slot,
                    &[],
                    &mut live.core.runtime,
                    &models,
                    &[],
                    &session.session.definition.presentation.particles,
                )
                .unwrap()
                .expect("ordinary settlement callback");
            let after = live
                .economy
                .house_residents
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                (after.position, after.yaw),
                (before.position, before.yaw),
                "model {model}"
            );
            assert_eq!(after.control_flags & 0x800, 0, "model {model}");
            assert_eq!(after.phase, 2, "the callback epilogue still advances");
            assert_eq!(
                live.economy.entities[usize::from(slot)].control_flags & 0x800,
                0
            );
        }
    }

    #[test]
    fn ordinary_level_seven_reset_traders_preserve_packets_and_reach_callbacks() {
        let mut session = ordinary_level_seven();
        let before = session
            .session
            .state
            .live
            .economy
            .reset_hash_with_terrain(&session.session.state.live.terrain);
        session
            .session
            .state
            .live
            .economy
            .adopt_reset_settlement_sources();
        assert_eq!(
            session
                .session
                .state
                .live
                .economy
                .reset_hash_with_terrain(&session.session.state.live.terrain,),
            before,
            "reset adoption must be idempotent, without RNG or allocator changes"
        );
        let rows = session
            .economy()
            .house_residents()
            .iter()
            .filter(|row| row.initial_package && row.model == TRADER_MODEL)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter()
                .map(|row| row.source_entity_index)
                .collect::<Vec<_>>(),
            [101, 102, 103, 104]
        );
        for row in &rows {
            assert_eq!((row.action, row.stage_var_1, row.stage_var_2), (113, 0, 0));
            let original = &session.entities()[usize::from(row.source_entity_index)];
            let mut projected = original.clone();
            row.publish_source(&mut projected);
            assert_eq!(
                projected.record_bytes(),
                original.record_bytes(),
                "adopted Trader {} changed its source packet",
                row.source_entity_index
            );
        }
        session.step_level2_source_pass(0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "ordinary Level7 source dispatch failed: {:?}",
            session.dispatch_events()
        );
        for old in rows {
            let current = session
                .economy()
                .house_residents()
                .iter()
                .find(|row| row.source_entity_index == old.source_entity_index)
                .unwrap();
            assert_eq!(current.phase, old.phase.wrapping_add(1));
            assert_eq!(
                session.entities()[usize::from(current.source_entity_index)].phase,
                current.phase
            );
            eprintln!(
                "actual Level7 reset Trader {} -> action {} phase {} target {} at {:?}",
                current.source_entity_index,
                current.action,
                current.phase,
                current.target_entity_index,
                current.position
            );
        }
    }

    fn village_placement_through_authoring() -> crate::native_world::Mc2Simulation {
        use crate::campaign::backend::load_mc2_campaign;
        use crate::game_definition::{
            ChangeSetBuilder, PublicationFault, RequestedTiming, WorkspaceStore, default_catalog,
        };
        use crate::level_definition::TilePosition;
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE")
            .map(PathBuf::from)
            .unwrap_or_else(|| private.join("generated/mc-1pb-16-level2-full-build-v2"));
        let catalog = default_catalog().unwrap();
        let base = load_mc2_campaign(&package, &catalog).unwrap();
        let id = sim_definition::adapter::stable_entity_id("creature.villager.at_123_179").unwrap();
        let workspace = std::env::temp_dir().join(format!(
            "mc-1pb-16-settlement-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store =
            WorkspaceStore::create(&workspace, "settlement-placement", base.clone()).unwrap();
        let change = ChangeSetBuilder::new(
            "settlement-placement",
            &base,
            "villager-near-real-house",
            RequestedTiming::SessionReset,
        )
        .move_entity_from(
            id,
            TilePosition { x: 123, y: 179 },
            // Retained terrain tile 72/84 is clear ground (type 2), outside
            // House 12's BUILD footprint and within its entry range. Slot 78
            // has no reset target; slot 77 already targets distant House 21.
            TilePosition { x: 72, y: 84 },
        )
        .build()
        .unwrap();
        let validation = store.validate(&catalog, change.clone()).unwrap();
        assert!(validation.accepted, "{:?}", validation.diagnostics);
        store
            .apply(
                &catalog,
                change,
                &validation.receipt_hash,
                PublicationFault::None,
            )
            .unwrap();
        let mut session = CampaignSession::open(&package, CampaignSelection::LEVEL2).unwrap();
        let published = store.active_definition().unwrap();
        session
            .apply_semantic_level_variant(published.level_definition().unwrap())
            .unwrap();
        eprintln!(
            "actual Level2 typed placement workspace {}",
            workspace.display()
        );
        session.simulation().clone()
    }

    fn ordinary_group_three_through_controls(session: &mut crate::native_world::Mc2Simulation) {
        session.select_level2_campaign_entry_stage();
        let features = session.mission().authored_features().to_vec();
        let point = |index| {
            features
                .iter()
                .find(|row| row.index == index)
                .unwrap()
                .world_position(0)
        };
        let player = session.state.entity_index;
        let step_at = |session: &mut crate::native_world::Mc2Simulation, position, turns| {
            for _ in 0..turns {
                session
                    .set_level2_entity_position(player, position)
                    .unwrap();
                session.step_outer_turn(crate::PlayerCommand::default(), 0);
            }
        };
        step_at(session, point(11), 1);
        if !session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|row| row.authored_index == 42)
        {
            let row = session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|row| row.authored_index == 41)
                .copied()
                .expect("ordinary row11 produced area-exit41");
            let mut outside = row.position;
            outside.x = outside.x.wrapping_add(0x8000);
            step_at(session, outside, 16);
        }
        step_at(session, point(42), 16);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .filter(|row| row.model == 14 && (125..=151).contains(&row.authored_index))
                .count(),
            27
        );
    }

    fn household_counts(session: &crate::native_world::Mc2Simulation) -> BTreeMap<u16, i32> {
        let live = &session.session.state.live;
        live.economy
            .settlement_source_views(
                &live.ordinary.houses,
                live.ordinary.villagers.villagers(),
                &live.ordinary.house_parameters,
            )
            .iter()
            .filter(|row| (row.class, row.model) == (10, 45))
            .filter_map(|row| {
                crate::native_world::level2_class5::trader::target_in_arena(
                    &live.economy,
                    &live.ordinary.houses,
                    row.source_entity_index,
                )
                .map(|target| (target.entity_index, target.dword_0x10))
            })
            .collect()
    }

    #[test]
    fn authored_villager_placement_joins_the_authoritative_household() {
        let mut session = village_placement_through_authoring();
        let slot = 78;
        let placed = session
            .ordinary()
            .villagers()
            .iter()
            .find(|row| row.entity_index == slot)
            .unwrap();
        assert_eq!((placed.position.x >> 8, placed.position.y >> 8), (72, 84));
        assert_eq!(placed.target_entity_index, None);
        assert_eq!((placed.stage_var_1, placed.stage_var_2), (0, 0));
        // Two ordinary 40-phase decisions select and enter the nearby home.
        for pass in 0..128_u16 {
            let before = household_counts(&session);
            session.step_level2_source_pass((pass % 3) as u8);
            let villager = session
                .ordinary()
                .villagers()
                .iter()
                .find(|row| row.entity_index == slot)
                .unwrap_or_else(|| {
                    panic!(
                        "authored Villager {slot} reclaimed before household entry at pass {pass}"
                    )
                });
            assert!(
                villager.life >= 0,
                "authored Villager died at pass {pass}: {villager:?}"
            );
            if villager.action == 108 && villager.reset_counter == 1 {
                let target = villager
                    .target_entity_index
                    .expect("accepted household remains the target");
                assert_eq!(target, 12);
                let occupants = household_counts(&session)[&target];
                assert_eq!(occupants, before[&target] + 1);
                assert_eq!(
                    session.entities()[usize::from(target)].source_dword_10(),
                    occupants
                );
                eprintln!(
                    "actual Level2 Villager {} -> House {} occupants {} at source pass {}",
                    villager.entity_index, target, occupants, pass
                );
                session.step_level2_source_pass(0);
                assert!(session.entities()[usize::from(slot)].drawing_disabled());
                session.step_level2_source_pass(1);
                assert_eq!(session.entities()[usize::from(slot)].class, 0);
                return;
            }
        }
        panic!(
            "ordinary imported villagers never joined a household; live rows {:?}; eligible houses {:?}",
            session.ordinary().villagers(),
            session
                .ordinary()
                .houses()
                .iter()
                .filter(|house| house.parameter.produces_villagers())
                .map(|house| (
                    house.entity_index,
                    house.position,
                    house.occupants,
                    house.capacity
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn ordinary_authored_traders_follow_their_source_stage_route() {
        let mut session = ordinary_level_two();
        ordinary_group_three_through_controls(&mut session);
        use crate::native_world::creatures::archer::ArcherStageRuntime;
        let route = |index| session.runtime().descriptor(index).unwrap();
        assert_eq!(
            (
                route(1).kind,
                route(1).next,
                route(1).point.x,
                route(1).point.y
            ),
            (1, 2, 116 << 8, 13 << 8)
        );
        assert_eq!(
            (route(2).kind, route(2).next, route(2).scalar),
            (6, 3, 2000)
        );
        assert_eq!(
            (
                route(3).kind,
                route(3).next,
                route(3).point.x,
                route(3).point.y
            ),
            (1, 4, 2 << 8, 153 << 8)
        );
        assert_eq!(
            (route(4).kind, route(4).next, route(4).scalar),
            (6, 5, 2020)
        );
        assert_eq!((route(5).kind, route(5).next), (1, 0));
        let before = *session
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.authored_index == 125)
            .unwrap();
        for pass in 0..64 {
            session.step_level2_source_pass(pass % 3);
        }
        let row = session
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == before.source_entity_index)
            .unwrap();
        let behavior =
            session.session.state.live.economy.house_residents.traders[&row.source_entity_index];
        // These actors follow three waypoints and 4020 waiting passes before
        // free trade. Bypassing their StageVars must not satisfy this gate.
        assert_eq!((row.action, behavior.index, behavior.mode), (119, 1, 1));
        assert_eq!(row.word_0x96_150, 0);
        assert_eq!(row.source_byte_3e, before.source_byte_3e.wrapping_add(64));
        assert_ne!(row.position, before.position);
        let raw = &session.entities()[usize::from(row.source_entity_index)];
        assert_eq!(raw.position, row.position);
        assert_eq!((raw.action, raw.phase), (row.action, row.source_byte_3e));
        assert_eq!((raw.record_bytes()[76], raw.record_bytes()[77]), (1, 1));
        assert!(
            session
                .economy()
                .map_chains
                .is_linked(row.source_entity_index)
        );
        eprintln!(
            "actual group3 Trader {} follows source route 1 in action119 at {:?}",
            row.source_entity_index, row.position
        );
    }

    #[test]
    fn ordinary_level_seven_reset_trader_joins_the_authoritative_household() {
        let mut session = ordinary_level_seven();
        for pass in 0..1024_u16 {
            let before = household_counts(&session);
            session.step_level2_source_pass((pass % 3) as u8);
            if pass % 64 == 0 && std::env::var_os("MC_SETTLEMENT_TRACE").is_some() {
                let trader = session
                    .economy()
                    .house_residents()
                    .iter()
                    .find(|row| row.initial_package && row.model == TRADER_MODEL);
                let target = trader.and_then(|row| {
                    crate::native_world::level2_class5::trader::target_in_arena(
                        &session.session.state.live.economy,
                        &session.session.state.live.ordinary.houses,
                        row.target_entity_index,
                    )
                });
                eprintln!("ordinary Level7 Trader pass {pass}: {trader:?}; target {target:?}");
            }
            if let Some(row) = session.economy().house_residents().iter().find(|row| {
                row.initial_package
                    && row.model == TRADER_MODEL
                    && row.action == 116
                    && row.reset_counter == 1
            }) {
                let slot = row.source_entity_index;
                let target = row.target_entity_index;
                let occupants = household_counts(&session)[&target];
                assert!(occupants > before[&target]);
                assert_eq!(
                    session.entities()[usize::from(target)].source_dword_10(),
                    occupants
                );
                eprintln!(
                    "actual Level7 reset Trader {} -> House {} occupants {} at source pass {}",
                    slot, target, occupants, pass
                );
                session.step_level2_source_pass(0);
                assert!(session.entities()[usize::from(slot)].drawing_disabled());
                session.step_level2_source_pass(1);
                assert!(
                    !session
                        .economy()
                        .house_residents()
                        .iter()
                        .any(|row| row.source_entity_index == slot)
                );
                return;
            }
        }
        panic!(
            "ordinary free Level7 Traders never joined a household; final rows {:?}; dispatch {:?}",
            session
                .economy()
                .house_residents()
                .iter()
                .filter(|row| row.model == TRADER_MODEL)
                .collect::<Vec<_>>(),
            session.dispatch_events()
        );
    }

    #[test]
    fn authored_trader_household_write_survives_the_next_house_callback() {
        let mut session = ordinary_level_two();
        let house = *session
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.active && house.parameter.produces_villagers())
            .unwrap();
        let live = &mut session.session.state.live;
        let source = live
            .economy
            .economy
            .allocate_authored_trader(
                125,
                house.position,
                TRADER_CLASS_PROPERTIES.max_life,
                &live.terrain,
            )
            .unwrap();
        let slot = source.source_entity_index;
        let source = live
            .economy
            .economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == slot)
            .unwrap();
        source.word_0x96_150 = house.entity_index;
        source.source_byte_3e = 0;
        // The source control input is a selected near household. No house
        // capacity or population is changed to arrange a successful result.
        session.step_level2_source_pass(0);
        let owned = *session
            .ordinary()
            .houses()
            .iter()
            .find(|candidate| candidate.entity_index == house.entity_index)
            .unwrap();
        assert_eq!(owned.occupants, house.occupants + 1);
        assert_eq!(
            session.entities()[usize::from(house.entity_index)].source_dword_10(),
            owned.occupants
        );
        let trader = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == slot)
            .unwrap();
        assert_eq!((trader.action, trader.dword_0x10), (116, 1));
        session.step_level2_source_pass(1);
        assert!(session.entities()[usize::from(slot)].drawing_disabled());
        assert!(
            session
                .ordinary()
                .houses()
                .iter()
                .find(|candidate| candidate.entity_index == house.entity_index)
                .unwrap()
                .occupants
                >= owned.occupants
        );
        session.step_level2_source_pass(2);
        assert!(
            !session
                .economy()
                .authored_class5()
                .iter()
                .any(|source| source.source_entity_index == slot)
        );
        assert!(
            !session
                .session
                .state
                .live
                .economy
                .economy
                .house_residents
                .traders
                .contains_key(&slot)
        );
    }

    #[test]
    fn staged_settlement_expiry_reaches_effect_and_following_pass_reclaim() {
        let mut session = ordinary_level_two();
        let slot = session.ordinary().villagers()[0].entity_index;
        let parent = session.state.entity_index;
        let live = &mut session.session.state.live;
        let actor = live
            .ordinary
            .villagers
            .villagers_mut()
            .iter_mut()
            .find(|row| row.entity_index == slot)
            .unwrap();
        // Controlled regression input to the real scheduled callback, distinct
        // from the ordinary owner-data household proof above.
        actor.stage_var_2 = 16;
        actor.stage.parent = parent;
        actor.stage.remaining = 0;
        actor.action = 105;
        actor.publish_source(&mut live.economy.entities[usize::from(slot)]);
        let effect_position = actor.position;
        let effects_before = live
            .economy
            .entities
            .iter()
            .filter(|row| row.active && (row.class, row.model) == (10, 0))
            .map(|row| row.index)
            .collect::<std::collections::BTreeSet<_>>();
        session.step_level2_source_pass(0);
        assert!(session.entities()[usize::from(slot)].drawing_disabled());
        assert_eq!(session.entities()[usize::from(slot)].class, 5);
        assert!(session.entities().iter().any(|row| row.active
            && (row.class, row.model) == (10, 0)
            && !effects_before.contains(&row.index)
            && row.position.x == effect_position.x
            && row.position.y == effect_position.y
            && row.subspell_index == 400
            && row.array_pitch() == 128
            && row.array_fov() == 128));
        session.step_level2_source_pass(1);
        assert_eq!(session.entities()[usize::from(slot)].class, 0);
        assert!(
            !session
                .ordinary()
                .villagers()
                .iter()
                .any(|row| row.entity_index == slot)
        );
    }

    #[test]
    fn settlement_contact_preserves_selected_target_and_source_mailbox_rules() {
        let mut session = ordinary_level_two();
        let live = &mut session.session.state.live;
        let house = live.ordinary.houses.houses()[0];
        let slot = house.entity_index;
        assert!(
            !live
                .ordinary
                .houses
                .submit_source_damage(slot, 12, house.source_id)
        );
        assert!(
            live.ordinary
                .houses
                .submit_source_contact(slot, 12, house.source_id)
        );
        assert!(live.ordinary.houses.submit_source_contact(slot, 7, 0));
        assert_eq!(live.ordinary.houses.houses()[0].pending_damage, 7);
        assert!(
            live.ordinary
                .houses
                .submit_source_contact(slot, 5, house.source_id)
        );
        assert_eq!(live.ordinary.houses.houses()[0].pending_damage, 12);
        let villager = live.ordinary.villagers.villagers()[0].clone();
        let slot = villager.entity_index;
        assert!(
            !live
                .ordinary
                .villagers
                .submit_source_damage(slot, 12, villager.source_id)
        );
        assert!(
            live.ordinary
                .villagers
                .submit_source_contact(slot, 12, villager.source_id)
        );
        assert!(live.ordinary.villagers.submit_source_contact(slot, 7, 0));
        assert_eq!(live.ordinary.villagers.villagers()[0].pending_damage, 7);
        assert!(
            live.ordinary
                .villagers
                .submit_source_contact(slot, 5, villager.source_id)
        );
        assert_eq!(live.ordinary.villagers.villagers()[0].pending_damage, 12);
    }

    #[test]
    fn ordinary_level_two_villager_death_publishes_and_reclaims_the_actual_target_row() {
        let mut session = ordinary_level_two();
        let initial = session
            .ordinary()
            .villagers()
            .iter()
            .find(|row| row.active && row.action == 105)
            .unwrap()
            .clone();
        let slot = initial.entity_index;
        let attacker = session.state.entity_index;
        session
            .ordinary_mut()
            .inject_villager_damage(slot, initial.life + 1, attacker.cast_signed())
            .unwrap();
        session.step_level2_source_pass(0);
        let row = session
            .ordinary()
            .villagers()
            .iter()
            .find(|row| row.entity_index == slot)
            .unwrap();
        let raw = &session.entities()[usize::from(slot)];
        assert_eq!(row.life, -1);
        assert_eq!(raw.life, row.life);
        assert_eq!(raw.position, row.position);
        assert_eq!(raw.action, row.action);
        assert_eq!(raw.phase, row.phase);
        assert_eq!(
            u16::from_le_bytes([raw.record_bytes()[42], raw.record_bytes()[43]]),
            attacker
        );
        assert_eq!(row.action, 108);
        for pass in 0..16 {
            session.step_level2_source_pass(pass % 3);
            if let Some(row) = session
                .ordinary()
                .villagers()
                .iter()
                .find(|row| row.entity_index == slot)
                && !row.active
            {
                assert!(
                    session.entities()[usize::from(slot)].active,
                    "hidden row is retained in this pass"
                );
                assert!(session.entities()[usize::from(slot)].drawing_disabled());
                session.step_level2_source_pass((pass + 1) % 3);
                assert!(
                    !session
                        .ordinary()
                        .villagers()
                        .iter()
                        .any(|row| row.entity_index == slot)
                );
                assert_eq!(
                    session.entities()[usize::from(slot)].class,
                    0,
                    "following pass retires the targeting header"
                );
                return;
            }
        }
        panic!("ordinary imported Villager did not reach its phase-gated removal");
    }

    #[test]
    fn ordinary_demolition_resident_publishes_and_reclaims_its_actual_arena_row() {
        let mut session = ordinary_level_two();
        let victim = *session
            .ordinary()
            .houses()
            .iter()
            .find(|row| row.entity_index == 10)
            .unwrap();
        let attacker = session.state.entity_index;
        session
            .ordinary_mut()
            .inject_house_damage(victim.entity_index, victim.life + 1, attacker.cast_signed())
            .unwrap();
        session.step_level2_source_pass(0);
        session.step_level2_source_pass(1);
        let builder = *session
            .economy()
            .house_residents()
            .iter()
            .find(|row| {
                row.parent_house_entity_index == victim.entity_index && row.model == BUILDER_MODEL
            })
            .unwrap();
        let slot = builder.source_entity_index;
        let raw = &session.entities()[usize::from(slot)];
        assert_eq!(
            (
                raw.active, raw.class, raw.model, raw.action, raw.life, raw.phase
            ),
            (
                true,
                5,
                BUILDER_MODEL,
                builder.action,
                builder.life,
                builder.phase
            )
        );
        assert_eq!(raw.position, builder.position);
        assert_eq!(raw.entity_rng, builder.entity_rng);
        assert_eq!(
            raw.particle_parameter_index,
            Some(BUILDER_APPEARANCE.particle)
        );
        assert_eq!(raw.source_player_entity_index(), builder.owner_entity_index);
        assert!(session.economy().map_chains.is_linked(slot));

        assert!(session.session.state.live.economy.submit_settlement_damage(
            slot,
            builder.life + 1,
            attacker,
            crate::combat::actors::DamageWriter::Ordinary
        ));
        let mut hidden = false;
        for pass in 0..24 {
            session.step_level2_source_pass(pass % 3);
            if let Some(row) = session
                .economy()
                .house_residents()
                .iter()
                .find(|row| row.source_entity_index == slot)
                && !row.drawing_enabled
            {
                assert!(session.entities()[usize::from(slot)].active);
                assert!(session.entities()[usize::from(slot)].drawing_disabled());
                hidden = true;
                break;
            }
        }
        assert!(
            hidden,
            "ordinary Builder death must reach its phase-gated hide"
        );
        session.step_level2_source_pass(0);
        assert!(
            !session
                .economy()
                .house_residents()
                .iter()
                .any(|row| row.source_entity_index == slot)
        );
        assert_eq!(session.entities()[usize::from(slot)].class, 0);
        assert!(!session.economy().map_chains.is_linked(slot));
        let live = &mut session.session.state.live;
        let free = live.economy.free_slot_count();
        live.ordinary
            .reclaim_for_source_boundary(&mut live.economy, &mut live.terrain)
            .unwrap();
        assert_eq!(
            live.economy.free_slot_count(),
            free,
            "retired family rows cannot free a reused address again"
        );
    }

    #[test]
    fn ordinary_level_two_demolition_builder_constructs_a_live_house() {
        let mut session = ordinary_level_two();
        let initial_slots = session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active)
            .map(|house| {
                (
                    house.entity_index,
                    session.economy().slot_generation(house.entity_index),
                )
            })
            .collect::<BTreeMap<_, _>>();
        // This is a resident-producing House, with buildable land beside
        // surviving neighbour 10. Scenery rows 1..9 do not spawn a Builder;
        // demolishing shoreline House 10 remains the separate reclaim case.
        let victim = *session
            .ordinary()
            .houses()
            .iter()
            .find(|house| house.entity_index == 12)
            .unwrap();
        assert!(victim.parameter.produces_villagers());
        assert_eq!(victim.parameter.destruction_successor, 0);
        let attacker = session.state.entity_index;
        session
            .ordinary_mut()
            .inject_house_damage(victim.entity_index, victim.life + 1, attacker.cast_signed())
            .unwrap();
        session.step_level2_source_pass(0);
        session.step_level2_source_pass(1);
        let builder = *session
            .economy()
            .house_residents()
            .iter()
            .find(|row| {
                row.parent_house_entity_index == victim.entity_index && row.model == BUILDER_MODEL
            })
            .expect("ordinary RemoveCastleStage produced its final Builder");
        assert_eq!(
            builder.pending_damage, 1,
            "demolition constructor retains its source damage residue"
        );
        assert_eq!(
            builder.semantic_kind,
            crate::level_definition::CreatureKind::Builder
        );
        assert!(
            !session
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == victim.entity_index)
                .unwrap()
                .active
        );
        let terrain_before_build = session.terrain_hash();
        if std::env::var_os("MC_SETTLEMENT_TRACE").is_some() {
            eprintln!("ordinary Builder birth: {builder:?}");
        }
        let mut born = None;
        for pass in 0..4096_u16 {
            session.step_level2_source_pass((pass % 3) as u8);
            if pass % 256 == 0 && std::env::var_os("MC_SETTLEMENT_TRACE").is_some() {
                eprintln!(
                    "ordinary Builder pass {pass}: {:?}",
                    session
                        .economy()
                        .house_residents()
                        .iter()
                        .find(|row| row.source_entity_index == builder.source_entity_index)
                );
            }
            if let Some(house) = session.ordinary().houses().iter().find(|house| {
                house.active
                    && initial_slots
                        .get(&house.entity_index)
                        .is_none_or(|generation| {
                            *generation != session.economy().slot_generation(house.entity_index)
                        })
            }) {
                born = Some(*house);
                if house.action == crate::house::HOUSE_LIVE_ACTION {
                    break;
                }
            }
        }
        let house = born.unwrap_or_else(|| {
            panic!(
                "no Builder house; current residents: {:?}",
                session.economy().house_residents()
            )
        });
        assert_eq!(house.action, crate::house::HOUSE_LIVE_ACTION);
        assert!(house.capacity > 0 && house.occupants >= 2);
        assert!(house.collision_half_width > 0 && house.collision_half_depth > 0);
        let raw = &session.entities()[usize::from(house.entity_index)];
        assert_eq!(
            (raw.class, raw.model, raw.action, raw.life, raw.phase),
            (10, 45, house.action, house.life, house.phase)
        );
        assert_eq!(raw.position, house.position);
        assert!(raw.active && session.economy().map_chains.is_linked(house.entity_index));
        assert_ne!(session.terrain_hash(), terrain_before_build);
        // Neutral source houses retain bit 0 and render through their BUILD
        // terrain. Possession clears that bit through the ordinary callback,
        // binds ownership/mana, and admits particle 177 to sprite consumers.
        session
            .ordinary_mut()
            .submit_house_possession(house.entity_index, attacker, 1)
            .unwrap();
        session.step_level2_source_pass(0);
        assert_eq!(session.economy().source_owner(house.entity_index), attacker);
        assert_eq!(
            session
                .economy()
                .owner_mana_rows
                .iter()
                .find(|row| row.source_entity_index == house.entity_index)
                .unwrap()
                .player_entity_index,
            0,
            "a same-pass refresh must retain the pre-possession boundary owner"
        );
        session.step_level2_source_pass(1);
        let owned = session
            .ordinary()
            .houses()
            .iter()
            .find(|row| row.entity_index == house.entity_index)
            .copied()
            .unwrap();
        assert_eq!(owned.owner_entity_index, attacker);
        let mana = session
            .economy()
            .owner_mana_rows
            .iter()
            .find(|row| row.source_entity_index == house.entity_index)
            .unwrap();
        assert_eq!(mana.player_entity_index, attacker);
        assert_eq!(
            mana.mana,
            (1_000 * i32::from(owned.parameter.life_mana_scalar)) >> 7
        );
        assert!(
            session
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|row| row.entity_index == house.entity_index)
        );
        eprintln!(
            "actual Level2 House {} demolition -> Builder {} action {} -> House {} BUILD {} action {} occupants {}/{} at {:?}",
            victim.entity_index,
            builder.source_entity_index,
            builder.action,
            house.entity_index,
            house.building_index,
            house.action,
            house.occupants,
            house.capacity,
            house.position
        );

        // Exercise the real teardown/rebuild/constructor sequence which can
        // reuse a settlement address during a later authored group callback.
        session
            .ordinary_mut()
            .inject_house_damage(owned.entity_index, owned.life + 1, attacker.cast_signed())
            .unwrap();
        session.step_level2_source_pass(0);
        session.step_level2_source_pass(1);
        assert!(session.entities()[usize::from(owned.entity_index)].drawing_disabled());
        let live = &mut session.session.state.live;
        let generation = live.economy.slot_generation(owned.entity_index);
        assert!(
            live.economy
                .settlement_mana_source_slots()
                .any(|slot| slot == owned.entity_index)
        );
        live.ordinary
            .reclaim_for_source_boundary(&mut live.economy, &mut live.terrain)
            .unwrap();
        live.economy
            .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
            .unwrap();
        // 49F90 sorts all existing free addresses, including older holes.
        // Consume those normally rather than forcing the desired slot first.
        let mut reused = None;
        for _ in 1..1000 {
            let slot = live
                .economy
                .allocate_house_resident(
                    owned.entity_index,
                    0,
                    crate::level1_mission::HouseResidentRequest {
                        position: owned.position,
                        model: 13,
                        action: 105,
                        attacker_entity_index: 0,
                    },
                    &mut live.terrain,
                )
                .expect("ordinary free arena retains reclaimed House address");
            if slot == owned.entity_index {
                reused = Some(slot);
                break;
            }
        }
        let reused = reused.expect("canonical allocation must revisit the reclaimed House address");
        assert_ne!(live.economy.slot_generation(reused), generation);
        assert!(
            !live
                .economy
                .settlement_mana_source_slots()
                .any(|slot| slot == reused)
        );
        live.economy.refresh_owner_mana_rows();
        assert!(
            !live
                .economy
                .owner_mana_rows
                .iter()
                .any(|row| row.source_entity_index == reused
                    && row.source_class == 10
                    && row.source_model == 45)
        );
    }
}
