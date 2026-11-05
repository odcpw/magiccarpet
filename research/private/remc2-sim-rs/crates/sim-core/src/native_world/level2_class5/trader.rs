//! The reachable Level 2 class-5/model-14 Trader family.
//!
//! `AddTrader_4C0B0` and `sub_237B0` are deliberately kept apart from the
//! other class-5 rows.  The Trader constructor has a different source packet
//! (notably its model counter, descriptor, and particle row), and action 113
//! owns a real mailbox/target/building decision closure rather than the
//! creature callbacks used by models 3 and 9.

use crate::level_definition::TRADER_CLASS_PROPERTIES as PROPERTIES;
use crate::native_world::creatures::settlements::{AuthoredTraderBehavior, runtime, stage};
use crate::native_world::{
    Level2AuthoredClass5RuntimeState, Level2AuthoredClass5Source, Level2EconomyState,
    SessionTerrain, level2_next_rng16,
};
use crate::renderable_map_chain::RenderableMapSeed;
use crate::{FlightWorld, Position};
use sim_definition::adapter::{TRADER_APPEARANCE, TRADER_SOURCE_BINDING};

/// The group-3 Trader rows, in the order in which the Level 2 dispatch
/// artifact presents them.
pub(crate) const TRADER_AUTHORED_ROWS: [u16; 27] = [
    125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
];

pub(crate) const TRADER_CLASS: u8 = 5;
pub(crate) const TRADER_MODEL: u8 = TRADER_SOURCE_BINDING.model;
pub(crate) const TRADER_ACTION: u8 = TRADER_SOURCE_BINDING.action;
pub(crate) const TRADER_HIT_ACTION: u8 = 118;
pub(crate) const TRADER_PRE_KILL_ACTION: u8 = 116;
pub(crate) const TRADER_MINIMUM_SPEED: i16 = PROPERTIES.minimum_speed;
pub(crate) const TRADER_MAXIMUM_SPEED: i16 = PROPERTIES.maximum_speed;
pub(crate) const TRADER_MAX_LIFE: i32 = PROPERTIES.max_life;
pub(crate) const TRADER_MANA: i32 = PROPERTIES.mana;
pub(crate) const TRADER_MAX_MANA: i32 = 0;
pub(crate) const TRADER_SUBSPELL_INDEX: u16 = 500;
pub(crate) const TRADER_DESCRIPTOR_INDEX: u16 = TRADER_SOURCE_BINDING.descriptor;
pub(crate) const TRADER_DESCRIPTOR_CADENCE: u8 = PROPERTIES.scan_period;
pub(crate) const TRADER_BYTE_0X38: u8 = 1;
pub(crate) const TRADER_XTYPE: u8 = 3;
pub(crate) const TRADER_SOURCE_BYTE_39: u8 = 64;
pub(crate) const TRADER_PARTICLE_INDEX: u16 = TRADER_APPEARANCE.particle;
pub(crate) const TRADER_SHIFT_ROT_X: i16 = PROPERTIES.collision_half_width;
pub(crate) const TRADER_SHIFT_ROT_Y: i16 = PROPERTIES.collision_half_height;
pub(crate) const TRADER_BUILDING_DISTANCE_MIN: u32 = 0x0e10_0000;
pub(crate) const TRADER_TARGET_RANGE: u32 = 0x0800;

#[must_use]
pub(crate) fn is_authored_trader_row(authored_index: u16) -> bool {
    TRADER_AUTHORED_ROWS.contains(&authored_index)
}

/// The exact constructor packet before it is projected into the existing
/// class-5 arena carrier.  The carrier predates this family and does not yet
/// expose `byte_0x38`, `xtype`, the descriptor pointer, or the first value
/// written to `dword_0x10`; retaining them here prevents the integration seam
/// from silently dropping source state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderConstructorPayload {
    pub(crate) source_entity_index: u16,
    pub(crate) entity_rng: u16,
    pub(crate) authored_index: u16,
    pub(crate) source_class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) minimum_speed: i16,
    pub(crate) maximum_speed: i16,
    pub(crate) actual_speed: i16,
    pub(crate) max_life: i32,
    pub(crate) life: i32,
    pub(crate) mana: i32,
    pub(crate) max_mana: i32,
    pub(crate) subspell_index: u16,
    pub(crate) initial_dword_0x10: i32,
    pub(crate) dword_0x10: i32,
    pub(crate) byte_0x38: u8,
    pub(crate) descriptor_index: u16,
    pub(crate) source_byte_39: u8,
    pub(crate) xtype: u8,
    pub(crate) source_byte_3e: u8,
    pub(crate) particle_index: u16,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) roll: i16,
    pub(crate) fov: i16,
    pub(crate) shift_rot_x: i16,
    pub(crate) shift_rot_y: i16,
    pub(crate) position: Position,
    pub(crate) phase: u8,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

/// Reproduce the ordered `AddTrader_4C0B0` writes after `NewEvent_4A050`.
/// `initial_rng` is the value installed by `NewEvent`; this function performs
/// the constructor's one LCG draw and no other random operation.
#[must_use]
pub(crate) fn trader_constructor_payload(
    source_entity_index: u16,
    initial_rng: u16,
    authored_index: u16,
    source_byte_3e: u8,
    position: Position,
    max_life: i32,
) -> TraderConstructorPayload {
    let entity_rng = level2_next_rng16(initial_rng);
    let angle = i16::try_from(entity_rng & 0x07ff).expect("Trader angle fits") - 1;
    TraderConstructorPayload {
        source_entity_index,
        entity_rng,
        authored_index,
        source_class: TRADER_CLASS,
        model: TRADER_MODEL,
        action: TRADER_ACTION,
        minimum_speed: TRADER_MINIMUM_SPEED,
        maximum_speed: TRADER_MAXIMUM_SPEED,
        actual_speed: TRADER_MAXIMUM_SPEED,
        max_life,
        life: max_life,
        mana: TRADER_MANA,
        max_mana: TRADER_MAX_MANA,
        subspell_index: TRADER_SUBSPELL_INDEX,
        initial_dword_0x10: i32::from(source_entity_index % 100),
        dword_0x10: 2,
        byte_0x38: TRADER_BYTE_0X38,
        descriptor_index: TRADER_DESCRIPTOR_INDEX,
        source_byte_39: TRADER_SOURCE_BYTE_39,
        xtype: TRADER_XTYPE,
        source_byte_3e,
        particle_index: TRADER_PARTICLE_INDEX,
        yaw: angle,
        pitch: angle,
        roll: angle,
        fov: 0,
        shift_rot_x: TRADER_SHIFT_ROT_X,
        shift_rot_y: TRADER_SHIFT_ROT_Y,
        position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

impl TraderConstructorPayload {
    /// Project the source packet into the carrier fields that existed before
    /// the Trader slice.  The full payload remains available to the caller so
    /// the serialized integration can extend that carrier without changing
    /// constructor order or values.
    #[must_use]
    fn into_class5_source(self) -> Level2AuthoredClass5Source {
        Level2AuthoredClass5Source {
            source_entity_index: self.source_entity_index,
            entity_rng: self.entity_rng,
            authored_index: self.authored_index,
            model: self.model,
            action: self.action,
            minimum_speed: self.minimum_speed,
            maximum_speed: self.maximum_speed,
            actual_speed: self.actual_speed,
            max_life: self.max_life,
            life: self.life,
            mana: self.mana,
            max_mana: self.max_mana,
            subspell_index: self.subspell_index,
            dword_0x10: self.dword_0x10,
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: self.particle_index,
            source_byte_3e: self.source_byte_3e,
            source_byte_39: self.source_byte_39,
            yaw: self.yaw,
            pitch: self.pitch,
            roll: self.roll,
            fov: self.fov,
            shift_rot_x: self.shift_rot_x,
            shift_rot_y: self.shift_rot_y,
            position: self.position,
            phase: self.phase,
            drawing_enabled: self.drawing_enabled,
            active: self.active,
        }
    }
}

impl Level2EconomyState {
    /// `sub_57F20` runs at the following source-pass boundary. Hide leaves
    /// the row and map-chain membership intact until this owner is called.
    pub(crate) fn reclaim_hidden_trader_rows_at_source_boundary(&mut self) {
        let mut hidden = self
            .authored_class5
            .iter()
            .filter(|row| row.active && row.model == TRADER_MODEL && !row.drawing_enabled)
            .map(|row| (row.source_entity_index, row.entity_rng))
            .collect::<Vec<_>>();
        hidden.sort_unstable_by_key(|(slot, _)| *slot);
        self.authored_class5
            .retain(|row| row.model != TRADER_MODEL || row.drawing_enabled);
        self.authored_class5_runtime.retain(|row| {
            !hidden
                .iter()
                .any(|(slot, _)| *slot == row.source_entity_index)
        });
        for (slot, rng) in hidden {
            self.house_residents.traders.remove(&slot);
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(rng));
        }
    }

    #[cfg(test)]
    pub(crate) fn stage_trader_mailbox_damage_for_test(
        &mut self,
        source_slot: u16,
        attacker_slot: u16,
        damage: i32,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.source_entity_index == source_slot
                && source.model == TRADER_MODEL
                && source.action == TRADER_ACTION
        }) else {
            return false;
        };
        let Some(runtime) = self
            .authored_class5_runtime
            .iter_mut()
            .find(|runtime| runtime.source_entity_index == source_slot)
        else {
            return false;
        };
        source.pending_word_0x62 = attacker_slot;
        runtime.pending_damage = damage;
        true
    }

    /// Allocate one authored Trader using the real source free stack, event
    /// RNG, model-14 counter, terrain/map projection, and reset-owned source
    /// arena.  The caller owns the surrounding group transaction.
    pub(crate) fn allocate_authored_trader(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if max_life <= 0 {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let source_byte_3e = self.class5_model14_constructor_phase;
        self.class5_model14_constructor_phase = source_byte_3e.wrapping_add(1);

        let mut position = position;
        position.z = terrain.ground_height(position);
        let payload = trader_constructor_payload(
            source_entity_index,
            initial_rng,
            authored_index,
            source_byte_3e,
            position,
            max_life,
        );
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = payload.into_class5_source();
        self.house_residents.traders.insert(
            source_entity_index,
            AuthoredTraderBehavior::new(source_entity_index),
        );
        self.source_rng
            .insert(source_entity_index, source.entity_rng);
        self.authored_class5.push(source);
        self.authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index,
                byte_0x38: payload.byte_0x38,
                xtype: payload.xtype,
                descriptor_index: payload.descriptor_index,
                pending_damage: 0,
                linked_head: 0,
            });
        Some(source)
    }
}

impl crate::campaign::source_arena::SourceArena {
    /// Advance one live Trader through its source callback and direct
    /// closure.  The context is deliberately explicit: the current generic
    /// class-5 carrier does not own the Trader mailbox damage amount, linked
    /// child chain, target `dword_0x10`, or ordered building candidates.
    pub(crate) fn step_authored_trader_slot(
        &mut self,
        source_slot: u16,
        target: Option<TraderTarget>,
        pending_damage: i32,
        linked_head: u16,
        linked: &[TraderLinkedState],
        buildings: &[TraderBuildingCandidate],
        terrain: &mut SessionTerrain,
        runtime: &mut crate::native_world::RuntimeSnapshot,
        models: &[Vec<u16>; 29],
        players: &[u16],
        particles: &[crate::native_world::ParticleParameter],
        villagers: &mut crate::villager::VillagerFamily,
    ) -> Result<Option<TraderAction113Step>, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == TRADER_MODEL
                && source.drawing_enabled
                && matches!(source.action, 112..=119)
        }) else {
            return Ok(None);
        };
        let mut source = self.authored_class5[source_index];
        let mut context = TraderAction113Context {
            pending_damage,
            linked_head,
            linked,
            target,
            buildings,
        };
        let linked_killers = linked
            .iter()
            .map(|child| {
                (
                    child.entity_index,
                    self.entities
                        .get(usize::from(child.entity_index))
                        .map_or(0, |row| row.word_24()),
                )
            })
            .collect::<Vec<_>>();
        let views = self
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| {
                let mut view = crate::level1_mission::Level1SourceEntityView::imported_record(
                    crate::level1_mission::Level1ResetSourceSlot {
                        index: row.index,
                        active: row.active,
                        class: row.class,
                        model: row.model,
                        action: row.action,
                        phase: row.phase,
                        entity_rng: row.entity_rng,
                        control_flags: row.control_flags,
                    },
                );
                view.position = row.position;
                view.life = row.life;
                view.source_id = row.source_id;
                view
            })
            .collect::<Vec<_>>();
        let targets = stage::targets(self, &views, villagers.villagers());
        let mut behavior = self
            .house_residents
            .traders
            .get(&source_slot)
            .copied()
            .unwrap_or_else(|| AuthoredTraderBehavior::new(source_slot));
        let mut effects = step_trader_lifecycle(
            &mut source,
            &mut context,
            terrain,
            &views,
            &linked_killers,
            &mut behavior,
            Some(stage::StageContext {
                runtime: Some(runtime),
                targets: &targets,
                models,
                players,
                effects: Vec::new(),
            }),
        );
        if effects.prekill_linked {
            for child in linked {
                if let Some(row) = self.entities.get_mut(usize::from(child.entity_index)) {
                    row.set_source_action(117);
                }
                if let Some(row) = self
                    .authored_class5
                    .iter_mut()
                    .find(|row| row.source_entity_index == child.entity_index)
                {
                    row.action = 117;
                }
            }
        }
        if effects.death_burst {
            let _ = self.spawn_death_burst_from_death_with_id(
                source_slot,
                source_slot,
                source.position,
            );
        }
        if effects.population_decrement != 0 {
            self.decrement_source_creature_population();
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.house_residents.traders.insert(source_slot, behavior);
        publish_source(
            &source,
            behavior,
            &mut self.entities[usize::from(source_slot)],
        );
        self.entities[usize::from(source_slot)].record[98..102]
            .copy_from_slice(&pending_damage.to_le_bytes());
        self.entities[usize::from(source_slot)].record[56..58]
            .copy_from_slice(&linked_head.to_le_bytes());
        let map_synced = self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok();
        let stage_effects = std::mem::take(&mut effects.stage_effects);
        effects.spell_awards =
            self::runtime::apply_effects(self, villagers, terrain, particles, stage_effects)?;
        Ok(Some(TraderAction113Step {
            effects,
            target: context.target,
            map_synced,
        }))
    }
}

/// Publish the current family carrier, including the stage words absent from
/// the common class-5 row, for collision, targeting and presentation consumers.
pub(crate) fn publish_source(
    source: &Level2AuthoredClass5Source,
    state: AuthoredTraderBehavior,
    raw: &mut crate::native_world::EntitySnapshot,
) {
    if (raw.class, raw.model) != (5, source.model) {
        *raw = crate::native_world::cave_features::new_event(
            source.source_entity_index,
            source.entity_rng,
            source.position,
        );
    }
    raw.active = source.active;
    raw.record[2] = u8::from(source.active);
    raw.class = 5;
    raw.model = source.model;
    raw.record[67] = 5;
    raw.record[68] = source.model;
    raw.source_id = state.source_id;
    raw.record[30..32].copy_from_slice(&state.source_id.to_le_bytes());
    raw.set_control_flags(state.control_flags | if source.drawing_enabled { 0 } else { 0x400 });
    raw.set_position(source.position);
    raw.set_yaw(source.yaw);
    raw.pitch = source.pitch;
    raw.record[34..36].copy_from_slice(&source.pitch.to_le_bytes());
    raw.record[36..38].copy_from_slice(&source.roll.to_le_bytes());
    raw.set_max_life(source.max_life);
    raw.set_life(source.life);
    raw.set_entity_rng(source.entity_rng);
    raw.set_source_action(source.action);
    raw.phase = source.source_byte_3e;
    raw.record[66] = source.source_byte_3e;
    raw.set_source_dword_10(source.dword_0x10);
    raw.record[40..42].copy_from_slice(&source.word_0x24.to_le_bytes());
    raw.record[42..44].copy_from_slice(&source.word_0x26.to_le_bytes());
    raw.record[44..46].copy_from_slice(&state.links.parent.to_le_bytes());
    raw.record[50..52].copy_from_slice(&state.links.remaining.to_le_bytes());
    raw.record[52..54].copy_from_slice(&state.links.duration.to_le_bytes());
    raw.record[60] = TRADER_BYTE_0X38;
    raw.record[61] = source.source_byte_39;
    raw.record[69] = TRADER_XTYPE;
    raw.record[76] = state.index as u8;
    raw.record[77] = state.mode as u8;
    raw.record[78..80].copy_from_slice(&state.links.relation.to_le_bytes());
    raw.record[88..90].copy_from_slice(&source.shift_rot_x.to_le_bytes());
    raw.record[90..92].copy_from_slice(&source.shift_rot_x.to_le_bytes());
    raw.record[92..94].copy_from_slice(&source.shift_rot_y.to_le_bytes());
    raw.record[94..96].copy_from_slice(&source.particle_index.to_le_bytes());
    raw.particle_parameter_index = Some(source.particle_index);
    raw.record[97] = 1;
    raw.record[102..104].copy_from_slice(&source.pending_word_0x62.to_le_bytes());
    raw.set_subspell_index(source.subspell_index);
    raw.parameter_relation = Some(TRADER_DESCRIPTOR_INDEX);
    raw.record[164..168].copy_from_slice(&(u32::from(TRADER_DESCRIPTOR_INDEX) + 1).to_le_bytes());
    raw.set_actual_speed(source.actual_speed);
    raw.record[136..138].copy_from_slice(&source.minimum_speed.to_le_bytes());
    raw.record[138..140].copy_from_slice(&source.maximum_speed.to_le_bytes());
    raw.mana = source.mana;
    raw.record[148..152].copy_from_slice(&source.mana.to_le_bytes());
    raw.record[152..154].copy_from_slice(&state.owner.to_le_bytes());
    raw.record[154..156].copy_from_slice(&source.word_0x96_150.to_le_bytes());
}

/// The ordinary authored constructor's sub_12100 route assignment.
pub(crate) fn bind_authored_route(
    arena: &mut crate::campaign::source_arena::SourceArena,
    runtime: &mut crate::native_world::RuntimeSnapshot,
    slot: u16,
    index: u8,
) {
    let economy = &mut arena.economy;
    let Some(source) = economy
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == slot && source.model == TRADER_MODEL)
    else {
        return;
    };
    let state = economy
        .house_residents
        .traders
        .entry(slot)
        .or_insert_with(|| AuthoredTraderBehavior::new(slot));
    crate::native_world::creatures::stage_route::Route {
        model: TRADER_MODEL,
        position: source.position,
        action: &mut source.action,
        index: &mut state.index,
        mode: &mut state.mode,
        relation: &mut state.links.relation,
    }
    .bind(runtime, index);
    publish_source(source, *state, &mut arena.entities[usize::from(slot)]);
}

/// A linked class-5 child as observed by `sub_237B0`'s `word_0x34` walk.
pub(crate) fn target_in_arena(
    arena: &crate::campaign::source_arena::SourceArena,
    houses: &crate::house::HouseFamily,
    slot: u16,
) -> Option<TraderTarget> {
    if slot == 0 {
        return None;
    }
    if let Some(house) = houses
        .houses()
        .iter()
        .find(|house| house.entity_index == slot && house.source_retained())
    {
        return Some(TraderTarget {
            entity_index: slot,
            class: 10,
            model: 45,
            active: house.source_retained(),
            minimum_speed: house.capacity,
            dword_0x10: house.occupants,
            position: house.position,
        });
    }
    if let Some(source) = arena
        .authored_model45()
        .iter()
        .find(|source| source.source_entity_index == slot && source.active)
    {
        return Some(TraderTarget {
            entity_index: slot,
            class: source.source_class,
            model: source.model,
            active: source.active,
            minimum_speed: source.minimum_speed,
            dword_0x10: source.dword_0x10,
            position: source.position,
        });
    }
    if let Some(source) = arena
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == slot && source.active)
    {
        return Some(TraderTarget {
            entity_index: slot,
            class: 5,
            model: source.model,
            active: source.active,
            minimum_speed: source.minimum_speed,
            dword_0x10: source.dword_0x10,
            position: source.position,
        });
    }
    let raw = arena
        .entities
        .get(usize::from(slot))
        .filter(|raw| raw.active)?;
    Some(TraderTarget {
        entity_index: slot,
        class: raw.class,
        model: raw.model,
        active: raw.active,
        minimum_speed: raw.minimum_speed(),
        dword_0x10: raw.source_dword_10(),
        position: raw.position,
    })
}

pub(crate) fn apply_target_in_arena(
    arena: &mut crate::campaign::source_arena::SourceArena,
    houses: &mut crate::house::HouseFamily,
    target: TraderTarget,
) {
    if let Some(house) = houses.commit_resident_count(target.entity_index, target.dword_0x10) {
        house.publish_source(&mut arena.entities[usize::from(target.entity_index)]);
    } else {
        if let Some(source) = arena
            .economy
            .authored_model45
            .iter_mut()
            .find(|source| source.source_entity_index == target.entity_index && source.active)
        {
            source.dword_0x10 = target.dword_0x10;
        }
        if let Some(raw) = arena
            .entities
            .get_mut(usize::from(target.entity_index))
            .filter(|raw| raw.active)
        {
            raw.set_source_dword_10(target.dword_0x10);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderLinkedState {
    pub(crate) entity_index: u16,
    pub(crate) next_entity_index: u16,
    pub(crate) life: i32,
    pub(crate) current_attacker: u16,
    pub(crate) model: u8,
}

/// The target fields directly read by action 113.  `dword_0x10` is included
/// because the close class-10/model-45 branch mutates the target before the
/// source callback returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderTarget {
    pub(crate) entity_index: u16,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) active: bool,
    pub(crate) minimum_speed: i16,
    pub(crate) dword_0x10: i32,
    pub(crate) position: Position,
}

/// An ordered source candidate from the live building map walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderBuildingCandidate {
    pub(crate) entity_index: u16,
    pub(crate) position: Position,
    pub(crate) eligible: bool,
}

pub(crate) struct TraderAction113Context<'a> {
    pub(crate) pending_damage: i32,
    pub(crate) linked_head: u16,
    pub(crate) linked: &'a [TraderLinkedState],
    pub(crate) target: Option<TraderTarget>,
    pub(crate) buildings: &'a [TraderBuildingCandidate],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraderAction113Effects {
    pub(crate) player_feedback_entity: Option<u16>,
    pub(crate) selected_target: Option<u16>,
    pub(crate) released_target: bool,
    pub(crate) moved: bool,
    pub(crate) hidden: bool,
    pub(crate) death_burst: bool,
    pub(crate) prekill_linked: bool,
    pub(crate) population_decrement: u32,
    pub(crate) stage_effects: Vec<stage::StageEffect>,
    pub(crate) spell_awards: Vec<(u16, u8)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraderAction113Step {
    pub(crate) effects: TraderAction113Effects,
    pub(crate) target: Option<TraderTarget>,
    pub(crate) map_synced: bool,
}

/// Translate the MC2 carrier into the canonical settlement population callback.
/// All state written back below belongs to the same source slot. The views are
/// temporary reads, never another mutable authority or scheduled actor.
fn step_trader_lifecycle<W: crate::FlightWorld>(
    source: &mut Level2AuthoredClass5Source,
    context: &mut TraderAction113Context<'_>,
    world: &W,
    extra_views: &[crate::level1_mission::Level1SourceEntityView],
    linked_killers: &[(u16, u16)],
    behavior: &mut AuthoredTraderBehavior,
    stage: Option<stage::StageContext<'_>>,
) -> TraderAction113Effects {
    use crate::level1_mission::{
        Level1ResetSourceSlot, Level1SourceEffect, Level1SourceEntityView,
    };
    use crate::native_world::creatures::settlements::{
        self, DynamicResidentKind, SettlementContext,
    };

    let old_target = source.word_0x96_150;
    let old_action = source.action;
    let had_mailbox = source.pending_word_0x62 != 0;
    let mut resident = settlements::resident_constructor(
        source.source_entity_index,
        source.entity_rng,
        source.source_byte_3e,
        source.position,
        DynamicResidentKind::Trader,
        TRADER_MODEL,
        source.action,
        0,
    );
    resident.entity_rng = source.entity_rng;
    resident.yaw = source.yaw;
    resident.desired_heading = source.roll;
    resident.life = source.life;
    resident.max_life = source.max_life;
    resident.pending_damage = context.pending_damage;
    resident.pending_attacker = source.pending_word_0x62;
    resident.current_attacker = source.word_0x26;
    resident.kill_credit_entity = source.word_0x24;
    resident.reset_counter = source.dword_0x10;
    resident.minimum_speed = source.minimum_speed;
    resident.maximum_speed = source.maximum_speed;
    resident.actual_speed = source.actual_speed;
    resident.target_entity_index = source.word_0x96_150;
    resident.linked_entity_index = context.linked_head;
    resident.mana = source.mana;
    resident.visible = source.drawing_enabled;
    resident.source_id = behavior.source_id;
    resident.stage = behavior.links;
    resident.stage_var_1 = behavior.index;
    resident.stage_var_2 = behavior.mode;
    resident.owner_entity_index = behavior.owner;
    resident.source_byte_39 = source.source_byte_39;
    resident.movement_blocked = behavior.control_flags & 0x40000 != 0;
    resident.suppress_death_burst = behavior.control_flags & 0x100000 != 0;

    let view = |index, class, model, active, position| {
        let mut view = Level1SourceEntityView::imported_record(Level1ResetSourceSlot {
            index,
            class,
            model,
            active,
            action: 0,
            phase: 0,
            entity_rng: 0,
            control_flags: 0,
        });
        view.position = position;
        view.life = 1;
        view
    };
    let mut views = extra_views.to_vec();
    for building in context.buildings {
        let mut candidate = view(building.entity_index, 10, 45, true, building.position);
        candidate.villager_entry_eligible = building.eligible;
        views.push(candidate);
    }
    if let Some(target) = context.target {
        let mut target_view = view(
            target.entity_index,
            target.class,
            target.model,
            target.active,
            target.position,
        );
        if let Some(current) = extra_views
            .iter()
            .find(|row| row.source_entity_index == target.entity_index)
        {
            target_view.life = current.life;
            target_view.control_flags = current.control_flags;
        }
        target_view.villager_entry_eligible = true;
        views.push(target_view);
    }
    let mut linked_residents = if context.linked.is_empty() {
        Vec::new()
    } else {
        vec![None; 1000]
    };
    for linked in context.linked {
        let mut child_view = view(linked.entity_index, 5, linked.model, true, source.position);
        child_view.life = linked.life;
        child_view.owner_entity_index = linked.current_attacker;
        child_view.linked_entity_index = linked.next_entity_index;
        views.push(child_view);
        let mut child = resident;
        child.source_entity_index = linked.entity_index;
        child.life = linked.life;
        child.current_attacker = linked.current_attacker;
        child.linked_entity_index = linked.next_entity_index;
        child.kill_credit_entity = linked_killers
            .iter()
            .find(|(slot, _)| *slot == linked.entity_index)
            .map_or(0, |(_, killer)| *killer);
        if let Some(slot) = linked_residents.get_mut(usize::from(linked.entity_index)) {
            *slot = Some(child);
        }
    }
    // Stable sorting keeps the current explicit target ahead of stale package
    // rows for the same arena identity.
    views.reverse();
    views.sort_by_key(|row| row.source_entity_index);
    views.dedup_by_key(|row| row.source_entity_index);
    let mut census = 1;
    let mut feedback = 0;
    let mut unused_global_rng = 0;
    let building_source_order = context
        .buildings
        .iter()
        .map(|row| row.entity_index)
        .collect::<Vec<_>>();
    let mut step = SettlementContext {
        stage,
        source_control_flags: &mut behavior.control_flags,
        dynamic_residents: &mut linked_residents,
        class_5_census: &mut census,
        player_archer_target_ticks: &mut feedback,
        global_rng: &mut unused_global_rng,
        building_source_order: &building_source_order,
        house_request: None,
        death_bursts: Vec::new(),
        linked_kills: Vec::new(),
        selection_rng_draw: None,
    };
    let outputs = step.step_dynamic_trader(world, &views, &mut resident);
    let death_burst = !step.death_bursts.is_empty();
    let prekill_linked = !step.linked_kills.is_empty();
    let stage_effects = step
        .stage
        .as_mut()
        .map(|stage| std::mem::take(&mut stage.effects))
        .unwrap_or_default();
    drop(step);
    let population_decrement = u32::from(census == 0);
    let mut hidden = false;
    for effect in outputs {
        match effect {
            Level1SourceEffect::TryEnterVillager {
                house_source_entity_index,
                ..
            } => {
                if let Some(target) = context
                    .target
                    .as_mut()
                    .filter(|target| target.entity_index == house_source_entity_index)
                    && i32::from(target.minimum_speed) > target.dword_0x10
                {
                    target.dword_0x10 = target.dword_0x10.wrapping_add(1);
                } else {
                    settlements::reject_trader_house_target(&mut resident);
                }
            }
            Level1SourceEffect::DisableSourceSlot { .. } => hidden = true,
            _ => unreachable!("Trader emits only household entry or hide"),
        }
    }
    source.entity_rng = resident.entity_rng;
    source.action = resident.action;
    source.life = resident.life;
    source.pending_word_0x62 = resident.pending_attacker;
    source.word_0x26 = resident.current_attacker;
    source.word_0x24 = resident.kill_credit_entity;
    source.word_0x96_150 = resident.target_entity_index;
    source.dword_0x10 = resident.reset_counter;
    source.position = resident.position;
    source.yaw = resident.yaw;
    source.roll = resident.desired_heading;
    source.actual_speed = resident.actual_speed;
    source.source_byte_39 = resident.source_byte_39;
    source.mana = resident.mana;
    behavior.index = resident.stage_var_1;
    behavior.mode = resident.stage_var_2;
    behavior.links = resident.stage;
    behavior.owner = resident.owner_entity_index;
    behavior.control_flags = (behavior.control_flags & !0x0014_0000)
        | if resident.movement_blocked {
            0x0004_0000
        } else {
            0
        }
        | if resident.suppress_death_burst {
            0x0010_0000
        } else {
            0
        };
    if hidden {
        source.drawing_enabled = false;
    }
    TraderAction113Effects {
        player_feedback_entity: (feedback != 0).then_some(if old_action == 117 {
            source.word_0x24
        } else {
            source.word_0x26
        }),
        selected_target: (source.word_0x96_150 != 0 && source.word_0x96_150 != old_target)
            .then_some(source.word_0x96_150),
        released_target: old_target != 0 && source.word_0x96_150 == 0,
        moved: !had_mailbox && source.life >= 0 && matches!(old_action, 112..=115 | 118),
        hidden,
        death_burst,
        prekill_linked,
        population_decrement,
        stage_effects,
        spell_awards: Vec::new(),
    }
}

#[cfg(test)]
pub(crate) fn step_trader_action113(
    source: &mut Level2AuthoredClass5Source,
    context: &mut TraderAction113Context<'_>,
) -> TraderAction113Effects {
    struct Ground(i16);
    impl crate::FlightWorld for Ground {
        fn ground_height(&self, _: Position) -> i16 {
            self.0
        }
        fn ceiling_height(&self, _: Position) -> Option<i16> {
            None
        }
        fn material_mask(&self, _: Position) -> u32 {
            0
        }
        fn angle_flags(&self, _: crate::Tile) -> u8 {
            0
        }
    }
    let mut behavior = AuthoredTraderBehavior::new(source.source_entity_index);
    step_trader_lifecycle(
        source,
        context,
        &Ground(source.position.z),
        &[],
        &[],
        &mut behavior,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::level_two_fixture_path;
    use super::*;
    use crate::native_world::{Level2DynamicArenaKind, NativeWorld};

    #[test]
    fn level_twenty_two_reset_adoption_and_actual_row_constructor_are_distinct() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        let package = std::env::var_os("MC_LEVEL22_SETTLEMENT_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                    "../../../generated/settlement-20261105-integration/owner-current/level22",
                )
            },
            std::path::PathBuf::from,
        );
        let mut campaign =
            CampaignSession::open(&package, CampaignSelection::ordinary(22).unwrap())
                .expect("qualified current whole Level22 owner package");
        let session = campaign.simulation_mut();
        let feature = *session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 237)
            .unwrap();
        assert_eq!(
            (feature.kind, feature.subtype, feature.activation_group),
            (5, 14, 0)
        );
        let captured = session.entities()[147].clone();
        assert_eq!((captured.class, captured.model), (5, 14));
        assert_eq!(
            (captured.position.x >> 8, captured.position.y >> 8),
            (feature.tile_x, feature.tile_y)
        );
        let captured_handle = session.source_arena().handle(147).unwrap();
        assert_eq!(
            session
                .economy()
                .house_residents()
                .iter()
                .filter(|row| row.initial_package && row.model == TRADER_MODEL)
                .count(),
            1
        );
        let baseline = session.campaign_state_hash();
        session
            .session
            .state
            .live
            .economy
            .adopt_reset_settlement_sources();
        session
            .session
            .state
            .live
            .economy
            .adopt_reset_settlement_sources();
        assert_eq!(
            session.campaign_state_hash(),
            baseline,
            "adoption cannot allocate or rerun construction"
        );
        assert_eq!(session.entities()[147], captured);
        assert_eq!(session.source_arena().handle(147), Some(captured_handle));

        // Separate constructor control using the actual qualified authored row.
        // The captured group0 actor above is already present; this is not an
        // ordinary second birth or a claim that reset lost its Trader.
        let max_life = session
            .session
            .definition
            .authored_creature_max_life(feature.index)
            .unwrap() as i32;
        let live = &mut session.session.state.live;
        let before_phase = live.economy.class5_model14_constructor_phase;
        let free = live.economy.free_slot_count();
        for invalid in [0, -1] {
            assert!(
                live.economy
                    .allocate_authored_trader(
                        feature.index,
                        feature.world_position(0),
                        invalid,
                        &live.terrain
                    )
                    .is_none()
            );
            assert_eq!(live.economy.free_slot_count(), free);
            assert_eq!(live.economy.class5_model14_constructor_phase, before_phase);
        }
        let source = live
            .economy
            .allocate_authored_trader(
                feature.index,
                feature.world_position(0),
                max_life,
                &live.terrain,
            )
            .expect("shared typed Trader constructor admits real non-Level2 row237");
        assert_ne!(source.source_entity_index, captured_handle.source_slot);
        assert_eq!(source.authored_index, feature.index);
        assert_eq!(
            (source.model, source.action, source.life, source.max_life),
            (TRADER_MODEL, TRADER_ACTION, max_life, max_life)
        );
        assert_eq!(
            live.economy.class5_model14_constructor_phase,
            before_phase.wrapping_add(1)
        );
        assert_eq!(live.economy.free_slot_count(), free - 1);
        let child = live.economy.handle(source.source_entity_index).unwrap();
        assert!(
            live.economy.resolve(child).is_some(),
            "actual typed arena publication"
        );
        assert!(live.economy.map_chains.is_linked(child.source_slot));
        assert_eq!(live.economy.handle(147), Some(captured_handle));
        assert_eq!(live.economy.entities[147], captured);
    }

    fn trader_position() -> Position {
        Position {
            x: 0x2800,
            y: 0x3100,
            z: 0,
        }
    }

    fn empty_context<'a>() -> TraderAction113Context<'a> {
        TraderAction113Context {
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: None,
            buildings: &[],
        }
    }

    #[test]
    fn trader_rows_are_the_complete_group3_source_order() {
        assert_eq!(TRADER_AUTHORED_ROWS.len(), 27);
        assert_eq!(TRADER_AUTHORED_ROWS[0], 125);
        assert_eq!(TRADER_AUTHORED_ROWS[26], 151);
        assert!(
            TRADER_AUTHORED_ROWS
                .windows(2)
                .all(|rows| rows[1] == rows[0].wrapping_add(1))
        );
        assert!(
            TRADER_AUTHORED_ROWS
                .iter()
                .copied()
                .all(is_authored_trader_row)
        );
        assert!(!is_authored_trader_row(124));
        assert!(!is_authored_trader_row(152));
    }

    #[test]
    fn trader_constructor_payload_is_source_exact_and_rng_discriminating() {
        let slot = 147_u16;
        let initial_rng = 0x1357_u16;
        let entity_rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let angle = i16::try_from(entity_rng & 0x07ff).expect("angle fits") - 1;
        let payload = trader_constructor_payload(
            slot,
            initial_rng,
            125,
            3,
            trader_position(),
            TRADER_MAX_LIFE,
        );

        assert_eq!(payload.entity_rng, entity_rng);
        assert_eq!(payload.initial_dword_0x10, 47);
        assert_eq!(payload.dword_0x10, 2);
        assert_eq!(payload.source_class, 5);
        assert_eq!(payload.model, 14);
        assert_eq!(payload.action, 113);
        assert_eq!(payload.minimum_speed, 54);
        assert_eq!(payload.maximum_speed, 18);
        assert_eq!(payload.actual_speed, 18);
        assert_eq!(payload.max_life, 1_000);
        assert_eq!(payload.life, 1_000);
        assert_eq!(payload.mana, 0);
        assert_eq!(payload.max_mana, 0);
        assert_eq!(payload.subspell_index, 500);
        assert_eq!(payload.byte_0x38, 1);
        assert_eq!(payload.descriptor_index, 100);
        assert_eq!(payload.source_byte_39, 64);
        assert_eq!(payload.xtype, 3);
        assert_eq!(payload.source_byte_3e, 3);
        assert_eq!(payload.particle_index, 219);
        assert_eq!(
            (payload.yaw, payload.pitch, payload.roll),
            (angle, angle, angle)
        );
        assert_eq!(payload.fov, 0);
        assert_eq!((payload.shift_rot_x, payload.shift_rot_y), (128, 128));
        assert!(payload.active && payload.drawing_enabled);
    }

    #[test]
    fn level_two_trader_group3_constructor_and_action113_are_exact() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Trader model-local session");
        let baseline_hash = session.campaign_state_hash();

        let mut rows = Vec::new();
        for authored_index in TRADER_AUTHORED_ROWS {
            rows.push(
                session
                    .economy_mut()
                    .allocate_authored_trader(authored_index, trader_position(), TRADER_MAX_LIFE)
                    .expect("every authored Trader row allocates"),
            );
        }
        assert_eq!(
            rows.iter()
                .map(|source| source.authored_index)
                .collect::<Vec<_>>(),
            TRADER_AUTHORED_ROWS
        );
        assert!(rows.iter().all(|source| {
            source.model == TRADER_MODEL
                && source.action == TRADER_ACTION
                && source.minimum_speed == TRADER_MINIMUM_SPEED
                && source.maximum_speed == TRADER_MAXIMUM_SPEED
                && source.actual_speed == TRADER_MAXIMUM_SPEED
                && source.max_life == TRADER_MAX_LIFE
                && source.life == TRADER_MAX_LIFE
                && source.mana == TRADER_MANA
                && source.max_mana == TRADER_MAX_MANA
                && source.subspell_index == TRADER_SUBSPELL_INDEX
                && source.source_byte_39 == TRADER_SOURCE_BYTE_39
                && source.particle_index == TRADER_PARTICLE_INDEX
                && source.shift_rot_x == TRADER_SHIFT_ROT_X
                && source.shift_rot_y == TRADER_SHIFT_ROT_Y
                && source.active
                && source.drawing_enabled
        }));
        assert_eq!(session.economy().authored_class5_runtime().len(), 27);
        assert!(
            session
                .economy()
                .authored_class5_runtime()
                .iter()
                .all(|state| {
                    state.byte_0x38 == TRADER_BYTE_0X38
                        && state.xtype == TRADER_XTYPE
                        && state.descriptor_index == TRADER_DESCRIPTOR_INDEX
                        && state.pending_damage == 0
                        && state.linked_head == 0
                })
        );

        let no_slot_phase = session.economy().class5_model14_constructor_phase;
        let no_slot_rng = session.economy().source_rng.clone();
        session.economy_mut().free_slots.clear();
        assert!(
            session
                .economy_mut()
                .allocate_authored_trader(151, trader_position(), TRADER_MAX_LIFE)
                .is_none()
        );
        assert!(session.economy().authored_class5().len() >= 27);
        assert!(session.economy().free_slots.is_empty());
        assert_eq!(
            session.economy().class5_model14_constructor_phase,
            no_slot_phase
        );
        assert_eq!(session.economy().source_rng, no_slot_rng);

        let mut source = rows[0];
        let source_before = source;
        let first_rng = source_before
            .entity_rng
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        let second_rng = first_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let expected_roll = i16::try_from(
            (i32::from(source_before.roll)
                + (i32::from(second_rng & 0xff) + 85)
                    * (2 * (i32::from(first_rng % 0x009d) / 79) - 1))
                & 0x07ff,
        )
        .expect("roll fits");
        let mut context = empty_context();
        let effects = step_trader_action113(&mut source, &mut context);
        assert!(effects.moved);
        assert_eq!(source.action, TRADER_ACTION);
        assert_eq!(source.word_0x96_150, 0);
        assert_eq!(source.actual_speed, TRADER_MAXIMUM_SPEED);
        assert_eq!(source.entity_rng, second_rng);
        assert_eq!(source.roll, expected_roll);
        assert!(effects.selected_target.is_none());
        assert!(!effects.released_target);

        let mut replay = source_before;
        let mut replay_context = empty_context();
        let replay_effects = step_trader_action113(&mut replay, &mut replay_context);
        assert_eq!(source, replay);
        assert_eq!(effects, replay_effects);

        let attacker = TraderTarget {
            entity_index: 777,
            class: 3,
            model: 0,
            active: true,
            minimum_speed: 0,
            dword_0x10: 0,
            position: source_before.position,
        };
        let mut hit_source = source_before;
        hit_source.pending_word_0x62 = attacker.entity_index;
        let mut hit_context = TraderAction113Context {
            pending_damage: 100,
            linked_head: 0,
            linked: &[],
            target: Some(attacker),
            buildings: &[],
        };
        let hit_effects = step_trader_action113(&mut hit_source, &mut hit_context);
        assert_eq!(hit_source.life, 900);
        assert_eq!(hit_source.pending_word_0x62, 0);
        assert_eq!(hit_source.word_0x26, attacker.entity_index);
        assert_eq!(hit_source.word_0x96_150, attacker.entity_index);
        assert_eq!(hit_source.action, TRADER_HIT_ACTION);
        assert_eq!(hit_source.actual_speed, TRADER_MINIMUM_SPEED);
        assert_eq!(
            hit_effects.player_feedback_entity,
            Some(attacker.entity_index)
        );

        // Bind the mailbox effect through the real dynamic-arena dispatcher:
        // the Trader consumes its sidecar damage and the existing Nyphur
        // authority receives the source player-feedback word.
        let trader_slot = rows[0].source_entity_index;
        let player_slot = session.nyphur_dispatcher.common_upkeep(0).source_slot;
        session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == trader_slot)
            .expect("production Trader row")
            .pending_word_0x62 = player_slot;
        session
            .economy_mut()
            .authored_class5_runtime_mut(trader_slot)
            .expect("production Trader sidecar")
            .pending_damage = 100;
        assert!(session.step_level2_dynamic_arena(
            0,
            trader_slot,
            Level2DynamicArenaKind::AuthoredClass5,
        ));
        let production_after = session
            .economy()
            .authored_class5()
            .iter()
            .find(|candidate| candidate.source_entity_index == trader_slot)
            .copied()
            .expect("production Trader remains live after contact");
        assert_eq!(production_after.life, 900);
        assert_eq!(production_after.action, TRADER_HIT_ACTION);
        assert_eq!(production_after.word_0x96_150, player_slot);
        assert_eq!(
            session
                .economy()
                .authored_class5_runtime()
                .iter()
                .find(|state| state.source_entity_index == trader_slot)
                .expect("production Trader sidecar remains keyed")
                .pending_damage,
            100
        );
        assert_eq!(session.nyphur_dispatcher.common_upkeep(0).word_0x248, 200);

        session.reset();
        assert_eq!(session.campaign_state_hash(), baseline_hash);
    }
}
