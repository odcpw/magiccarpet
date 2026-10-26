//! MC2 area writer adapter over the existing arena and family mailboxes.

use std::collections::BTreeSet;

use super::{ENTITY_DRAW_DISABLED_BIT, Level2OrdinaryResetSession};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    combat::{
        actors::{DamageMailboxMut, DamageWriter},
        area::{self, Actor, AreaDamage, AreaWorld, Shape},
    },
    goat::GoatFamily,
    house::HouseFamily,
    villager::VillagerFamily,
};

/// Borrow the three actual scheduler-owned actor domains. The ordinary
/// session is split at construction; no unrelated session state is retained.
pub(crate) struct ActorDomains<'a> {
    goats: &'a mut GoatFamily,
    houses: &'a mut HouseFamily,
    villagers: &'a mut VillagerFamily,
}

impl<'a> ActorDomains<'a> {
    pub(crate) fn new(
        goats: &'a mut GoatFamily,
        ordinary: &'a mut Level2OrdinaryResetSession,
    ) -> Self {
        Self {
            goats,
            houses: &mut ordinary.houses,
            villagers: &mut ordinary.villagers,
        }
    }

    pub(crate) fn reborrow(&mut self) -> ActorDomains<'_> {
        ActorDomains {
            goats: &mut *self.goats,
            houses: &mut *self.houses,
            villagers: &mut *self.villagers,
        }
    }

    fn source_view(&self, slot: u16) -> Option<crate::level1_mission::Level1SourceEntityView> {
        self.goats
            .goats()
            .iter()
            .find(|row| row.active && row.entity_index == slot)
            .map(super::goat_source_entity_view)
            .or_else(|| {
                self.houses
                    .houses()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(super::house_source_entity_view)
            })
            .or_else(|| {
                self.villagers
                    .villagers()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(super::villager_source_entity_view)
            })
    }

    fn submit(&mut self, slot: u16, payload: u16, owner: u16, writer: DamageWriter) -> bool {
        if self
            .houses
            .houses()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            match writer {
                DamageWriter::Ordinary => self.houses.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.houses.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        } else if self
            .villagers
            .villagers()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            match writer {
                DamageWriter::Ordinary => self.villagers.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.villagers.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        } else {
            match writer {
                DamageWriter::Ordinary => self.goats.submit_source_damage(slot, payload, owner),
                DamageWriter::Contact => self.goats.submit_source_contact(slot, payload, owner),
                DamageWriter::FirstWriter => false,
            }
        }
    }
}

pub(crate) fn apply(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    player: Option<(u16, Position)>,
    request: AreaDamage,
) -> Result<u16, String> {
    area::apply(
        &mut ArenaTargets {
            arena,
            actors,
            player,
        },
        request,
    )
}

/// `sub_11900` mode zero after the caller selected its actual target. Contact
/// replaces an owned payload and adds to ownerless residue; it does not repeat
/// the producer's distance, geometry, class or identity admission.
pub(crate) fn contact(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
    payload: u16,
    owner: u16,
) -> Result<(), String> {
    ArenaTargets {
        arena,
        actors,
        player: None,
    }
    .submit_damage(slot, payload, owner, DamageWriter::Contact)
}

/// Read the same current carrier used by damage writers. Terrain owns cell
/// traversal and admission; it must not consult a previous raw occupant.
pub(crate) fn source_actor(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
) -> Option<Actor> {
    ArenaTargets {
        arena,
        actors,
        player: None,
    }
    .actor(slot)
}

/// Read life from the same current carrier as source_actor, including dead
/// retained targets. A typed occupant never falls back to stale raw bytes.
pub(crate) fn source_life(
    arena: &SourceArena,
    actors: Option<ActorDomains<'_>>,
    slot: u16,
) -> Option<i32> {
    if let Some(domains) = actors.as_ref()
        && let Some(view) = domains.source_view(slot)
    {
        return Some(view.life);
    }
    let economy = &arena.economy;
    macro_rules! row_life {
        ($rows:expr) => {
            if let Some(row) = $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                return Some(i32::from(row.life));
            }
        };
    }
    row_life!(economy.spheres);
    row_life!(economy.castle_defenders);
    row_life!(economy.bee_creatures);
    row_life!(economy.house_residents);
    row_life!(economy.authored_dragon_parts);
    row_life!(economy.authored_class5_model3_children);
    row_life!(economy.authored_class5);
    if let Some(row) = economy
        .castle
        .filter(|row| row.active && row.source_entity_index == slot)
    {
        return Some(row.life);
    }
    if economy.live_arena_cursor_row(slot).is_some() {
        return None;
    }
    arena
        .entities
        .get(usize::from(slot))
        .filter(|s| s.active)
        .map(|s| s.life)
}

/// The direct life/credit write made by source 57390 after terrain admission.
/// This leaves ordinary pending damage intact for the actual family callback.
pub(crate) fn apply_construction_kill(
    arena: &mut SourceArena,
    mut actors: Option<ActorDomains<'_>>,
    slot: u16,
    source_id: u16,
) -> Result<bool, String> {
    let Some(target) = source_actor(arena, actors.as_mut().map(ActorDomains::reborrow), slot)
    else {
        return Ok(false);
    };
    if target.class != 5 || target.source_id == source_id {
        return Ok(false);
    }
    if let Some(domains) = actors.as_mut() {
        if domains
            .goats
            .goats()
            .iter()
            .any(|row| row.active && row.entity_index == slot)
        {
            domains.goats.apply_house_construction_kill(slot, source_id);
            let goat = domains
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
                .ok_or_else(|| format!("construction burial lost Goat carrier {slot}"))?;
            arena.economy.sync_authored_goat_projection(goat);
            return Ok(true);
        }
        if domains
            .villagers
            .villagers()
            .iter()
            .any(|row| row.source_retained() && row.entity_index == slot)
        {
            domains
                .villagers
                .apply_house_construction_kill(slot, source_id);
            return Ok(true);
        }
    }
    let economy = &mut arena.economy;
    if economy.apply_settlement_construction_kill(slot, source_id) {
        return Ok(true);
    }
    if let Some(row) = economy
        .castle_defenders
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.apply_construction_kill(source_id);
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_class5_model3_children
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        row.word_0x26 = source_id;
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_dragon_parts
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        row.word_0x26 = source_id;
        return Ok(true);
    }
    if let Some(row) = economy
        .authored_class5
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.life = -1;
        row.word_0x24 = source_id;
        row.word_0x26 = source_id;
        return Ok(true);
    }
    if economy.live_arena_cursor_row(slot).is_some() {
        return Err(format!(
            "construction burial has no direct-life writer for live carrier {slot}"
        ));
    }
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5)
        .ok_or_else(|| format!("construction burial lost admitted class5 slot {slot}"))?;
    row.set_life(-1);
    row.set_damage_result_words(source_id, source_id);
    Ok(true)
}

struct ArenaTargets<'arena, 'actors> {
    arena: &'arena mut SourceArena,
    actors: Option<ActorDomains<'actors>>,
    player: Option<(u16, Position)>,
}

/// The source class/model heads are frozen before the numeric Update scan.
/// A fallback NewEvent clears those heads; map-chain membership stays live.
#[derive(Clone, Debug, Default)]
pub(crate) struct EffectLists {
    class3: Vec<u16>,
    model45: Vec<u16>,
    epoch: u32,
}

impl EffectLists {
    pub(crate) fn is_current(&self, arena: &SourceArena) -> bool {
        self.epoch == arena.economy.replacement_list_epoch
    }

    pub(crate) fn class3_slots(&self, arena: &SourceArena) -> &[u16] {
        if self.is_current(arena) {
            &self.class3
        } else {
            &[]
        }
    }

    pub(crate) fn model45_slots(&self, arena: &SourceArena) -> &[u16] {
        if self.is_current(arena) {
            &self.model45
        } else {
            &[]
        }
    }
}

pub(crate) fn capture_effect_lists(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
) -> EffectLists {
    let targets = ArenaTargets {
        arena,
        actors,
        player: None,
    };
    let mut model45 = targets
        .arena
        .live_model45_buildings()
        .into_iter()
        .map(|(slot, _, _)| slot)
        .collect::<Vec<_>>();
    if let Some(domains) = targets.actors.as_ref() {
        model45.extend(
            domains
                .houses
                .houses()
                .iter()
                .filter(|row| row.source_retained())
                .map(|row| row.entity_index),
        );
    }
    model45.sort_unstable();
    model45.dedup();
    EffectLists {
        class3: targets.class3_slots(),
        model45,
        epoch: targets.arena.economy.replacement_list_epoch,
    }
}

/// Adapt the same live actor carriers to the distinct 10C80/11400 policies.
/// No effect-owned victim mirror or second damage mailbox is introduced.
pub(crate) fn apply_effect(
    arena: &mut SourceArena,
    actors: Option<ActorDomains<'_>>,
    player: Option<(u16, Position)>,
    masks: Option<&super::CollisionMaskBank>,
    lists: &EffectLists,
    request: crate::world::effects::DamagePulse,
) -> Result<u16, String> {
    crate::world::effects::apply_damage(
        &mut EffectTargets {
            targets: ArenaTargets {
                arena,
                actors,
                player,
            },
            masks,
            lists,
        },
        request,
    )
}

struct EffectTargets<'arena, 'actors, 'assets> {
    targets: ArenaTargets<'arena, 'actors>,
    masks: Option<&'assets super::CollisionMaskBank>,
    lists: &'assets EffectLists,
}

impl AreaWorld for EffectTargets<'_, '_, '_> {
    fn class3_slots(&self) -> Vec<u16> {
        self.lists.class3_slots(self.targets.arena).to_vec()
    }

    fn cell_slots(&self, x: u8, y: u8) -> Vec<u16> {
        self.targets.cell_slots(x, y)
    }
    fn actor(&self, slot: u16) -> Option<Actor> {
        self.targets.actor(slot)
    }
    fn mark_model2(&mut self, slot: u16) -> Result<(), String> {
        self.targets.mark_model2(slot)
    }
    fn ordinary_damage(&mut self, slot: u16, payload: u16, owner: u16) -> Result<(), String> {
        self.targets.ordinary_damage(slot, payload, owner)
    }
}

impl crate::world::effects::DamageWorld for EffectTargets<'_, '_, '_> {
    fn model45_slots(&self) -> Vec<u16> {
        self.lists.model45_slots(self.targets.arena).to_vec()
    }

    fn model45_contact(
        &self,
        request: crate::world::effects::DamagePulse,
        slot: u16,
    ) -> Result<bool, String> {
        let Some(target) = self.targets.actor(slot) else {
            return Ok(false);
        };
        if !super::level2_compare_axis_with_shift(
            request.position,
            request.half_width,
            request.half_width,
            target.position,
            target.shape.half_width,
            target.shape.half_depth,
        ) {
            return Ok(false);
        }
        let selector = self
            .targets
            .actors
            .as_ref()
            .and_then(|domains| {
                domains
                    .houses
                    .houses()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
                    .map(|row| row.building_index)
            })
            .or_else(|| {
                self.targets
                    .arena
                    .live_model45_buildings()
                    .into_iter()
                    .find(|(id, _, _)| *id == slot)
                    .map(|(_, selector, _)| selector)
            })
            .ok_or_else(|| {
                format!("effect model45 slot {slot} lost its retained building selector")
            })?;
        let bank = self.masks.ok_or_else(|| {
            "effect model45 contact requires the retained collision-mask bank".to_owned()
        })?;
        Ok(super::level2_model76_model45_occupancy(
            bank,
            u16::from(selector),
            request.position,
        ))
    }
}

fn shape(width: i16, depth: i16, offset: i16, height: i16) -> Shape {
    Shape {
        half_width: width,
        half_depth: depth,
        vertical_offset: offset,
        half_height: height,
    }
}

impl AreaWorld for ArenaTargets<'_, '_> {
    fn class3_slots(&self) -> Vec<u16> {
        let mut slots = self
            .arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 3 && row.index != 0)
            .map(|row| row.index)
            .collect::<BTreeSet<_>>();
        if let Some(castle) = self.arena.economy.castle.filter(|row| row.active) {
            slots.insert(castle.source_entity_index);
        }
        // Preserve the imported intrusive class list, including the retained
        // topology of a Castle whose state has moved into its typed carrier.
        let linked = slots
            .iter()
            .filter_map(|slot| self.arena.entities.get(usize::from(*slot))?.next_relation)
            .collect::<BTreeSet<_>>();
        let mut order = Vec::new();
        let mut visited = BTreeSet::new();
        for head in slots
            .iter()
            .filter(|slot| !linked.contains(slot))
            .chain(slots.iter())
        {
            let mut current = *head;
            while slots.contains(&current) && visited.insert(current) {
                order.push(current);
                current = self
                    .arena
                    .entities
                    .get(usize::from(current))
                    .and_then(|row| row.next_relation)
                    .unwrap_or(0);
            }
        }
        order
    }

    fn cell_slots(&self, x: u8, y: u8) -> Vec<u16> {
        self.arena.economy.map_chains.entity_chain(x, y)
    }

    fn actor(&self, slot: u16) -> Option<Actor> {
        if slot == 0 {
            return None;
        }
        let economy = &self.arena.economy;
        if let Some(actors) = self.actors.as_ref()
            && let Some(view) = actors.source_view(slot)
        {
            let mask = economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
                .map(|row| row.byte_0x38)
                .or_else(|| {
                    economy
                        .authored_model45
                        .iter()
                        .find(|row| row.source_entity_index == slot)
                        .map(|row| row.source_byte_38)
                })
                .unwrap_or_else(|| self.arena.entities[usize::from(slot)].source_byte_38());
            return Some(Actor {
                source_id: view.source_id,
                class: view.class,
                model: view.model,
                position: view.position,
                shape: shape(
                    view.collision_half_width,
                    view.collision_half_depth,
                    view.collision_vertical_offset,
                    view.collision_half_height,
                ),
                damage_mask: mask,
                control_flags: view.control_flags,
            });
        }
        if let Some(row) = economy
            .spheres
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Some(Actor {
                source_id: row.source_entity_index,
                class: 10,
                model: 39,
                position: row.position,
                shape: shape(
                    row.collision_pitch,
                    row.collision_roll,
                    row.collision_yaw,
                    row.collision_fov,
                ),
                damage_mask: 3,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .castle_defenders
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Some(Actor {
                source_id: row.id_entity_index,
                class: 5,
                model: 15,
                position: row.position,
                shape: shape(row.shift_rot_x, row.shift_rot_y, 125, row.fov),
                damage_mask: row.byte_38,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .castle
            .filter(|row| row.active && row.source_entity_index == slot)
        {
            return Some(Actor {
                source_id: row.source_id_entity_index,
                class: row.source_class,
                model: row.source_model,
                position: row.position,
                shape: shape(
                    row.shift_pitch,
                    row.shift_roll,
                    row.shift_yaw,
                    row.shift_fov,
                ),
                damage_mask: row.source_byte_38,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .bee_creatures
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            // Particle 3 supplies array.yaw; SetEntityShiftRot replaces only
            // pitch/roll/fov. Read the current carrier after either operation.
            return Some(Actor {
                source_id: row.entity_id,
                class: row.source_class,
                model: row.model,
                position: row.position,
                shape: shape(
                    row.shift_rot_x as i16,
                    row.shift_rot_x as i16,
                    row.collision_vertical_offset,
                    row.shift_rot_y as i16,
                ),
                damage_mask: 1,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .house_residents
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            // AddVilliger/AddBuilder/AddTrader set byte38=1. Their current
            // domain does not mutate that constructor-owned mask.
            return Some(Actor {
                source_id: row.source_id,
                class: 5,
                model: row.model,
                position: row.position,
                shape: shape(
                    row.collision_half_width,
                    row.collision_half_width,
                    row.collision_vertical_offset,
                    row.collision_half_height,
                ),
                damage_mask: 1,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .authored_dragon_parts
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            let root = economy
                .authored_dragons
                .iter()
                .find(|root| root.source_entity_index == row.parent_entity_index);
            let mask = economy
                .authored_class5_runtime
                .iter()
                .find(|root| root.source_entity_index == row.parent_entity_index)
                .map_or(1, |root| root.byte_0x38);
            return Some(Actor {
                source_id: row.entity_id,
                class: 5,
                model: row.model,
                position: row.position,
                shape: shape(
                    row.render_pitch,
                    row.render_roll,
                    row.render_yaw,
                    row.render_fov,
                ),
                damage_mask: mask,
                control_flags: root.map_or(8, |root| root.control_flags),
            });
        }
        if let Some(row) = economy
            .authored_class5_model3_children
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            let mask = economy
                .authored_class5_runtime
                .iter()
                .find(|root| root.source_entity_index == row.parent_entity_index)
                .map_or(1, |root| root.byte_0x38);
            return Some(Actor {
                // sub_4B6F0 qmemcpy copies the root's id into every body.
                // The immutable constructor parent retains that id even if
                // the root's numeric slot is later reclaimed and reused.
                source_id: row.parent_entity_index,
                class: 5,
                model: row.model,
                position: row.position,
                shape: shape(
                    row.render_pitch,
                    row.render_roll,
                    row.render_yaw,
                    row.render_fov,
                ),
                damage_mask: mask,
                control_flags: row.control_flags,
            });
        }
        if let Some(row) = economy
            .authored_class5
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            let runtime = economy
                .authored_class5_runtime
                .iter()
                .find(|state| state.source_entity_index == slot);
            let archer = economy
                .authored_class5_archer
                .iter()
                .find(|state| state.source_entity_index == slot);
            let firefly = economy
                .authored_fireflies
                .iter()
                .find(|state| state.source_entity_index == slot);
            let dragon = economy
                .authored_dragons
                .iter()
                .find(|state| state.source_entity_index == slot);
            let model3 = economy
                .authored_class5_model3
                .iter()
                .find(|state| state.source_entity_index == slot);
            let flags = archer
                .map(|state| state.control_flags)
                .or_else(|| firefly.map(|state| state.control_flags))
                .or_else(|| dragon.map(|state| state.control_flags))
                .or_else(|| model3.map(|state| state.control_flags))
                .or_else(|| match row.model {
                    // These two current adapters keep NewEvent's fixed
                    // collision bit: model9 never writes flags, and Trader
                    // explicitly starts each canonical callback with 8.
                    // Do not admit another family through a generic default.
                    9 | 14 => Some(8),
                    _ => None,
                })?;
            let dimensions = if let Some(state) = dragon {
                shape(
                    state.render_pitch,
                    state.render_roll,
                    state.render_yaw,
                    state.render_fov,
                )
            } else if let Some(state) = model3 {
                shape(
                    state.render_pitch,
                    state.render_roll,
                    state.render_yaw,
                    state.render_fov,
                )
            } else {
                let offset = if row.model == 4 {
                    super::level2_class5::archer::ARCHER_COLLISION_VERTICAL_OFFSET
                } else {
                    0
                };
                shape(row.shift_rot_x, row.shift_rot_x, offset, row.shift_rot_y)
            };
            return Some(Actor {
                source_id: archer.map_or(slot, |state| state.entity_id),
                class: 5,
                model: row.model,
                position: row.position,
                shape: dimensions,
                damage_mask: archer
                    .map(|state| state.byte_0x38)
                    .or_else(|| runtime.map(|state| state.byte_0x38))
                    .unwrap_or(1),
                control_flags: flags
                    | if row.drawing_enabled {
                        0
                    } else {
                        ENTITY_DRAW_DISABLED_BIT
                    },
            });
        }
        // A dynamic occupant cannot expose a previous raw occupant's mailbox.
        if economy.live_arena_cursor_row(slot).is_some() {
            return None;
        }
        let row = self
            .arena
            .entities
            .get(usize::from(slot))
            .filter(|row| row.active)?;
        Some(Actor {
            source_id: row.source_id,
            class: row.class,
            model: row.model,
            position: self
                .player
                .filter(|(index, _)| *index == slot)
                .map_or(row.position, |(_, position)| position),
            shape: shape(
                row.array_pitch(),
                row.array_roll(),
                row.array_yaw(),
                row.array_fov(),
            ),
            damage_mask: row.source_byte_38(),
            control_flags: row.control_flags,
        })
    }

    fn mark_model2(&mut self, slot: u16) -> Result<(), String> {
        if let Some(row) = self
            .arena
            .economy
            .castle
            .as_mut()
            .filter(|row| row.active && row.source_entity_index == slot)
        {
            row.source_word_30 = 30;
        } else {
            let row = self
                .arena
                .entities
                .get_mut(usize::from(slot))
                .filter(|row| row.active)
                .ok_or_else(|| format!("area model2 slot {slot} no longer live"))?;
            // Export header precedes the source word by four bytes. The
            // older source_word_30 helper addresses a different legacy view.
            row.activation_value = 30;
            row.record[52..54].copy_from_slice(&30_u16.to_le_bytes());
        }
        Ok(())
    }

    fn ordinary_damage(&mut self, slot: u16, payload: u16, owner: u16) -> Result<(), String> {
        self.submit_damage(slot, payload, owner, DamageWriter::Ordinary)
    }
}

impl ArenaTargets<'_, '_> {
    fn submit_damage(
        &mut self,
        slot: u16,
        payload: u16,
        owner: u16,
        writer: DamageWriter,
    ) -> Result<(), String> {
        if let Some(actors) = self.actors.as_mut()
            && actors.source_view(slot).is_some()
        {
            if !actors.submit(slot, payload, owner, writer) {
                return Err(format!(
                    "area live-world mailbox {slot} rejected an admitted target"
                ));
            }
            if let Some(goat) = actors
                .goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
            {
                self.arena.economy.sync_authored_goat_projection(goat);
            }
            return Ok(());
        }
        let economy = &mut self.arena.economy;
        if let Some(row) = economy
            .spheres
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(
                &mut row.ordinary_damage_payload,
                &mut row.ordinary_damage_owner,
            )
            .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if let Some(row) = economy
            .castle_defenders
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            )
            .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if let Some(row) = economy
            .castle
            .as_mut()
            .filter(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            )
            .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if let Some(row) = economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            )
            .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if economy
            .house_residents
            .iter()
            .any(|row| row.active && row.source_entity_index == slot)
        {
            if !economy.submit_settlement_damage(slot, i32::from(payload), owner, writer) {
                return Err(format!("area settlement slot {slot} rejected its mailbox"));
            }
            return Ok(());
        }
        if let Some(row) = economy
            .authored_dragon_parts
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(&mut row.pending_damage, &mut row.pending_word_0x62).submit(
                i32::from(payload),
                owner,
                writer,
            );
            return Ok(());
        }
        if let Some(row) = economy
            .authored_class5_model3_children
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(&mut row.damage_dword_0x5e_94, &mut row.pending_word_0x62)
                .submit(i32::from(payload), owner, writer);
            return Ok(());
        }
        if let Some(row) = economy
            .authored_class5
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            if row.model == 9
                && !economy
                    .authored_class5_runtime
                    .iter()
                    .any(|state| state.source_entity_index == slot)
            {
                // The existing model9 constructor omits an empty ordinary
                // mailbox sidecar. Materialize that canonical mailbox on its
                // first real write, retaining the pinned constructor header.
                economy
                    .authored_class5_runtime
                    .push(super::Level2AuthoredClass5RuntimeState {
                        source_entity_index: slot,
                        byte_0x38: 1,
                        xtype: 3,
                        descriptor_index: 80,
                        pending_damage: 0,
                        linked_head: 0,
                    });
            }
            let damage = if let Some(state) = economy
                .authored_class5_archer
                .iter_mut()
                .find(|state| state.source_entity_index == slot)
            {
                &mut state.pending_damage
            } else if let Some(state) = economy
                .authored_class5_model3
                .iter_mut()
                .find(|state| state.source_entity_index == slot)
            {
                &mut state.damage_dword_0x5e_94
            } else {
                &mut economy
                    .authored_class5_runtime
                    .iter_mut()
                    .find(|state| state.source_entity_index == slot)
                    .ok_or_else(|| format!("area class5 slot {slot} has no ordinary mailbox"))?
                    .pending_damage
            };
            DamageMailboxMut::new(damage, &mut row.pending_word_0x62).submit(
                i32::from(payload),
                owner,
                writer,
            );
            return Ok(());
        }
        if economy.live_arena_cursor_row(slot).is_some() {
            return Err(format!(
                "damage slot {slot} has no canonical mailbox writer"
            ));
        }
        let row = self
            .arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active)
            .ok_or_else(|| format!("area raw slot {slot} no longer live"))?;
        let mut damage = row.source_wind_spell_payload();
        let mut attacker = row.source_wind_spell_owner();
        DamageMailboxMut::new(&mut damage, &mut attacker).submit(i32::from(payload), owner, writer);
        row.record[98..102].copy_from_slice(&damage.to_le_bytes());
        row.record[102..104].copy_from_slice(&attacker.to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::disasters::{RingProfile, mc2};

    #[test]
    fn selected_contact_uses_authored_bee_geometry_and_mailbox() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let bee = arena
            .economy
            .allocate_authored_bee(
                1,
                session.state.position,
                None,
                3000,
                &session.session.state.live.terrain,
            )
            .expect("authored Day Bee birth");
        let slot = bee.source_entity_index;
        let target = source_actor(&mut arena, None, slot).expect("current Bee carrier");
        assert_eq!(target.shape.half_width, 128);
        assert_eq!(target.shape.half_height, 128);
        assert_eq!(target.shape.vertical_offset, 75);
        contact(&mut arena, None, slot, 200, session.state.entity_index).unwrap();
        let current = arena
            .economy
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(current.pending_damage, 200);
        assert_eq!(
            current.pending_attacker_entity_index,
            session.state.entity_index
        );
        assert_eq!(current.life, bee.life);
    }

    #[test]
    fn selected_contact_preserves_raw_mailbox_rules_without_readmission() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let row = &mut arena.entities[usize::from(slot)];
        let owner = row.source_id;
        // An already-selected target may share source identity. An ownerless
        // payload is residue, not an empty mailbox, and addition wraps.
        row.record[98..102].copy_from_slice(&i32::MAX.to_le_bytes());
        row.record[102..104].copy_from_slice(&0_u16.to_le_bytes());
        contact(&mut arena, None, slot, 1, owner).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            i32::MIN
        );
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_owner(),
            owner
        );
        contact(&mut arena, None, slot, 200, owner).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            200
        );
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 8, owner)
        .unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            208
        );
        contact(&mut arena, None, slot, 0, 0).unwrap();
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_payload(),
            0
        );
        assert_eq!(
            arena.entities[usize::from(slot)].source_wind_spell_owner(),
            0
        );
        assert!(contact(&mut arena, None, u16::MAX, 200, owner).is_err());
    }

    #[test]
    fn selected_contact_updates_actual_sphere_carrier_not_shadow_record() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let sphere = arena
            .economy
            .allocate_authored_model39(1, session.state.entity_index, session.state.position)
            .expect("ordinary authored sphere birth");
        let slot = sphere.source_entity_index;
        let shadow = arena.entities[usize::from(slot)].clone();
        // Ordinary damage first owns the retained typed mailbox. Contact
        // replaces that hit; another ordinary hit then adds to Contact.
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 123, session.state.entity_index)
        .unwrap();
        contact(&mut arena, None, slot, 200, session.state.entity_index).unwrap();
        ArenaTargets {
            arena: &mut arena,
            actors: None,
            player: None,
        }
        .ordinary_damage(slot, 8, session.state.entity_index)
        .unwrap();
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (
                current.ordinary_damage_payload,
                current.ordinary_damage_owner
            ),
            (208, session.state.entity_index)
        );
        assert_eq!(current.mana, sphere.mana);
        assert_eq!(arena.entities[usize::from(slot)].record, shadow.record);
    }

    fn ring_near(
        arena: &mut SourceArena,
        terrain: &mut super::super::SessionTerrain,
        target: Position,
    ) -> u16 {
        // Choose an actual eligible package tile within the default ring's
        // extent. No terrain, victim position, life or mailbox is staged.
        for radius in 0_i16..=7 {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let position = Position {
                        x: target.x.wrapping_add_signed(x * 256),
                        y: target.y.wrapping_add_signed(y * 256),
                        z: target.z,
                    };
                    let tile = usize::from(position.x >> 8) + 256 * usize::from(position.y >> 8);
                    if terrain.planes().angles[tile] & 15 != 0 {
                        let slot = mc2::construct(arena, terrain, position, RingProfile::default())
                            .expect("ordinary ring allocation");
                        let source = &arena.entities[usize::from(slot)];
                        assert_eq!((source.class, source.model, source.action), (10, 11, 11));
                        assert_eq!(source.record[67], 10, "the dispatch class is lossless");
                        return slot;
                    }
                }
            }
        }
        panic!("owner-data target has no eligible ring tile within range");
    }

    fn request(arena: &SourceArena, damage: mc2::AreaDamage) -> AreaDamage {
        let source = &arena.entities[usize::from(damage.source_slot)];
        AreaDamage {
            position: damage.position,
            owner: damage.owner,
            payload: damage.payload,
            shape: shape(
                damage.half_width,
                damage.half_depth,
                damage.vertical_offset,
                damage.half_height,
            ),
            selector: (source.record[69], source.record[70]),
            mode: 0,
        }
    }

    #[test]
    fn owner_package_ring_hits_player_creature_and_castle_before_their_consumers() {
        // The Level-2 start is sea: its first lowering clears the center's
        // angle and correctly terminates the next visit. Use the existing
        // Level-3 land start to exercise both ordinary payloads, preserving
        // the real package pose and terrain rather than forcing either.
        let root = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            std::path::PathBuf::from,
        );
        let mut session = crate::campaign::CampaignSession::open(
            root,
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .expect("qualified Level-3 land boundary")
        .simulation()
        .clone();
        let player = session.state.entity_index;
        let position = session.state.position;
        let initial_player_life = session.canonical_player_life_for_test();
        let mut terrain = session.canonical_terrain().clone();
        let mut arena = session.source_arena().clone();
        let castle_slot = arena
            .economy
            .build_castle(position, player, &terrain)
            .expect("ordinary Castle constructor");
        let initial_castle_life = arena.economy.castle().unwrap().life;
        let firefly = arena
            .economy
            .allocate_authored_firefly(
                1,
                position,
                super::super::level2_class5::firefly::FireflyStageRoute {
                    stage_var1: 1,
                    stage_var2: 0,
                    target: position,
                },
                super::super::level2_class5::firefly::FIREFLY_LIFE as u32,
                &terrain,
            )
            .expect("ordinary Firefly constructor");
        let ring = ring_near(&mut arena, &mut terrain, position);
        let owner = arena.entities[usize::from(ring)].source_id;
        for expected_payload in [200, 8] {
            let source = &arena.entities[usize::from(ring)];
            let tile =
                usize::from(source.position.x >> 8) + 256 * usize::from(source.position.y >> 8);
            let before = (
                source.life,
                source.position,
                terrain.planes().angles[tile],
                terrain.planes().ground_heights[tile],
            );
            let outcome = mc2::step(&mut arena, &mut terrain, ring, |arena, damage| {
                assert_eq!(damage.payload, expected_payload);
                let request = request(arena, damage);
                let count = apply(arena, None, Some((player, position)), request)?;
                assert!(count >= 3, "the player, Firefly and Castle are recipients");
                // This callback is synchronous, before the new terrain band.
                assert_eq!(arena.economy.castle().unwrap().source_word_30, 30);
                assert_eq!(
                    arena.entities[usize::from(player)].source_wind_spell_owner(),
                    owner
                );
                Ok(count)
            })
            .unwrap();
            assert!(
                outcome.terrain_mutated,
                "ring payload {expected_payload}: before={before:?}, outcome={outcome:?}, after_angle={}, after_height={}, player_pending={}",
                terrain.planes().angles[tile],
                terrain.planes().ground_heights[tile],
                arena.entities[usize::from(player)].source_wind_spell_payload()
            );
            assert!(outcome.hit_count >= 3);
        }
        assert_eq!(arena.economy.castle().unwrap().pending_damage, 208);
        assert_eq!(
            arena.entities[usize::from(player)].source_wind_spell_payload(),
            208
        );
        let firefly_lifecycle = arena
            .economy
            .step_authored_firefly_lifecycle_slot(firefly.source_entity_index, &[], &terrain)
            .unwrap();
        assert!(firefly_lifecycle.handled);
        let after = arena
            .economy
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == firefly.source_entity_index)
            .unwrap();
        assert_eq!(after.life, firefly.life - 208);
        assert_eq!(after.word_0x26, owner);
        *session.source_arena_mut() = arena;
        *session.canonical_terrain_mut() = terrain;
        assert_eq!(
            session.consume_level2_player_damage(),
            Some((208, owner, false))
        );
        assert_eq!(
            session.canonical_player_life_for_test(),
            initial_player_life - 208
        );
        assert_eq!(
            session
                .source_arena_mut()
                .economy
                .resolve_damage(castle_slot),
            Some(false)
        );
        assert_eq!(
            session.economy().castle().unwrap().life,
            initial_castle_life - 208
        );
    }

    #[test]
    fn death_sphere_contact_geometry_changes_only_at_its_color_callback() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let id = arena
            .economy
            .spawn_mana_sphere_from_death(
                session.state.entity_index,
                session.state.position,
                0,
                0x1234,
                8000,
                0,
                terrain,
            )
            .expect("ordinary death-to-sphere producer");
        let slot = u16::try_from(id).unwrap();
        let newborn = source_actor(&mut arena, None, slot).unwrap();
        assert_eq!(
            newborn.shape,
            shape(28, 28, 28, 28),
            "copied death mana must not resize constructor geometry before its visit"
        );
        assert!(arena.economy.step_mana_sphere_slot(slot, terrain, |_| 52));
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .copied()
            .unwrap();
        assert!(current.active);
        assert_ne!(current.particle_index, 53);
        assert_ne!(current.collision_pitch, 28);
        let visited = source_actor(&mut arena, None, slot).unwrap();
        assert_eq!(
            visited.shape,
            shape(
                current.collision_pitch,
                current.collision_roll,
                current.collision_yaw,
                current.collision_fov,
            ),
            "contact admission follows the callback's retained geometry"
        );
    }

    #[test]
    fn ordinary_sphere_birth_retains_area_mailbox_and_hit_count_without_mana_loss() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let sphere = arena
            .economy
            .allocate_authored_model39(1, session.state.entity_index, session.state.position)
            .expect("ordinary authored mana-sphere constructor");
        assert_eq!(
            sphere.control_flags & 8,
            8,
            "NewEvent's damage admission bit"
        );
        assert_eq!(
            (sphere.ordinary_damage_payload, sphere.ordinary_damage_owner),
            (0, 0)
        );
        let mut request = AreaDamage {
            position: sphere.position,
            shape: shape(1, 1, sphere.collision_yaw, 1),
            owner: sphere.source_entity_index,
            payload: 200,
            selector: (10, 39),
            mode: 0,
        };
        assert_eq!(apply(&mut arena, None, None, request).unwrap(), 0);
        let zero_hash = arena.economy.reset_hash();
        request.owner = session.state.entity_index;
        for (payload, total) in [(200, 200), (8, 208)] {
            request.payload = payload;
            assert_eq!(apply(&mut arena, None, None, request).unwrap(), 1);
            let current = arena
                .economy
                .spheres()
                .iter()
                .find(|row| row.source_entity_index == sphere.source_entity_index)
                .unwrap();
            assert_eq!(
                (
                    current.ordinary_damage_payload,
                    current.ordinary_damage_owner
                ),
                (total, request.owner)
            );
            assert_eq!(current.mana, sphere.mana);
            assert_eq!(current.owner_entity_index, sphere.owner_entity_index);
            assert_eq!(current.incoming_possession_payload, 0);
            assert_eq!(current.incoming_owner_entity_index, 0);
        }
        assert_ne!(arena.economy.reset_hash(), zero_hash);
        assert!(
            arena
                .economy
                .step_mana_sphere_slot(sphere.source_entity_index, terrain, |_| 52)
        );
        let current = arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == sphere.source_entity_index)
            .unwrap();
        assert!(current.active);
        assert_eq!(
            (
                current.ordinary_damage_payload,
                current.ordinary_damage_owner
            ),
            (208, request.owner),
            "the actual Sphere callback retains ordinary damage words"
        );
        assert_eq!(current.mana, sphere.mana);
        let mut cleared = arena.economy.clone();
        let current = cleared
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere.source_entity_index)
            .unwrap();
        current.ordinary_damage_payload = 0;
        current.ordinary_damage_owner = 0;
        assert_ne!(
            cleared.reset_hash(),
            arena.economy.reset_hash(),
            "the authoritative retained mailbox participates in the source hash"
        );
    }

    #[test]
    fn buried_dragon_parts_transfer_last_nonzero_killer_before_root_credit() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let player = session.state.entity_index;
        let root = arena
            .economy
            .allocate_authored_class5(1, 0, session.state.position, terrain)
            .expect("ordinary Dragon constructor and copied body chain");
        let earlier_owner = arena
            .entities
            .iter()
            .find(|row| {
                row.active
                    && row.source_id != 0
                    && row.source_id != player
                    && row.source_id != root.source_entity_index
            })
            .expect("another actual source identity from the owner package")
            .source_id;
        let mut slot = arena
            .economy
            .authored_class5_runtime
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap()
            .linked_head;
        let mut chain = Vec::new();
        while slot != 0 {
            let part = arena
                .economy
                .authored_dragon_parts
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                part.word_0x24, root.word_0x24,
                "the body copied its root killer word"
            );
            chain.push(slot);
            slot = part.next_entity_index;
        }
        assert_eq!(chain.len(), 16);
        assert!(apply_construction_kill(&mut arena, None, chain[0], earlier_owner).unwrap());
        assert!(apply_construction_kill(&mut arena, None, chain[15], player).unwrap());
        assert!(
            arena
                .economy
                .step_authored_dragon_roam_slot(root.source_entity_index, &[], &[], terrain, &[])
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.word_0x24, source.action),
            (-1, earlier_owner, 4),
            "minimum life selects the first lower child before PreKill"
        );
        let transition = arena
            .economy
            .step_authored_dragon_lifecycle_slot(root.source_entity_index, terrain)
            .unwrap();
        assert!(transition.handled);
        assert_eq!(
            transition.pre_kill_credit,
            Some(player),
            "PreKill resolves the last nonzero child killer before the counter consumer"
        );
        assert!(
            arena
                .economy
                .authored_dragon_parts
                .iter()
                .filter(|row| chain.contains(&row.source_entity_index))
                .all(|row| row.action == 5)
        );
        let direct_root = arena
            .economy
            .allocate_authored_class5(2, 0, session.state.position, terrain)
            .expect("a second ordinary Dragon constructor");
        assert!(
            apply_construction_kill(&mut arena, None, direct_root.source_entity_index, player)
                .unwrap()
        );
        assert!(
            arena
                .economy
                .step_authored_dragon_roam_slot(
                    direct_root.source_entity_index,
                    &[],
                    &[],
                    terrain,
                    &[]
                )
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == direct_root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.action),
            (-1, 4),
            "direct negative life reaches death without a fresh mailbox or lower child"
        );
        assert_eq!(
            source.word_0x24, 0,
            "source upkeep clears the ownerless current attacker before assigning root credit"
        );
        let transition = arena
            .economy
            .step_authored_dragon_lifecycle_slot(direct_root.source_entity_index, terrain)
            .unwrap();
        assert_eq!(transition.pre_kill_credit, None);
    }

    #[test]
    fn raw_model2_timer_precedes_owner_gate_and_does_not_touch_legacy_word() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let row = &mut arena.entities[usize::from(slot)];
        // Contract test only: exercise the source's special model-2 branch
        // independently of the package's ordinary player model.
        row.model = 2;
        row.record[68] = 2;
        let before_legacy_word = row.record[48..50].to_vec();
        let request = AreaDamage {
            position: row.position,
            shape: shape(1, 1, 0, 1),
            owner: row.source_id,
            payload: 0,
            selector: (5, 4),
            mode: 0,
        };
        assert_eq!(apply(&mut arena, None, None, request).unwrap(), 0);
        let row = &arena.entities[usize::from(slot)];
        assert_eq!(row.source_spell_word_30(), 30);
        assert_eq!(&row.record[48..50], before_legacy_word.as_slice());
        assert_eq!(row.source_wind_spell_owner(), 0);
    }

    #[test]
    fn worm_area_owner_exclusion_uses_copied_root_identity() {
        let session = super::super::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let mut arena = session.source_arena().clone();
        let root = arena
            .economy
            .allocate_authored_class5(1, 3, session.state.position, terrain)
            .expect("ordinary Worm root and body constructor");
        let body_slots = arena
            .economy
            .authored_class5_model3_children
            .iter()
            .filter(|body| body.parent_entity_index == root.source_entity_index)
            .map(|body| body.source_entity_index)
            .collect::<Vec<_>>();
        assert_eq!(body_slots.len(), 16);
        let mut request = AreaDamage {
            position: root.position,
            shape: shape(2304, 2304, 0, 8192),
            owner: root.source_entity_index,
            payload: 200,
            selector: (5, 3),
            mode: 0,
        };
        assert_eq!(
            apply(&mut arena, None, None, request).unwrap(),
            0,
            "the root id excludes its whole copied chain"
        );
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|body| body_slots.contains(&body.source_entity_index))
                .all(|body| body.damage_dword_0x5e_94 == 0 && body.pending_word_0x62 == 0)
        );
        request.owner = session.state.entity_index;
        assert_eq!(
            apply(&mut arena, None, None, request).unwrap(),
            17,
            "a hostile field reaches the root and all sixteen real bodies"
        );
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|body| body_slots.contains(&body.source_entity_index))
                .all(|body| body.damage_dword_0x5e_94 == 200
                    && body.pending_word_0x62 == request.owner)
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(source.pending_word_0x62, request.owner);
        assert_eq!(
            arena
                .economy
                .authored_class5_model3
                .iter()
                .find(|row| row.source_entity_index == root.source_entity_index)
                .unwrap()
                .damage_dword_0x5e_94,
            200
        );
        let child_slot = body_slots[0];
        assert!(
            !apply_construction_kill(&mut arena, None, child_slot, root.source_entity_index)
                .unwrap()
        );
        assert!(apply_construction_kill(&mut arena, None, child_slot, request.owner).unwrap());
        let child = arena
            .economy
            .authored_class5_model3_children
            .iter()
            .find(|row| row.source_entity_index == child_slot)
            .unwrap();
        assert_eq!(
            (child.life, child.word_0x24, child.word_0x26),
            (-1, request.owner, request.owner)
        );
        assert_eq!(
            (child.damage_dword_0x5e_94, child.pending_word_0x62),
            (200, request.owner),
            "burial preserves the real ordinary mailbox"
        );
        assert!(arena.economy.step_authored_class5_model3_slot_with_target(
            root.source_entity_index,
            None,
            terrain
        ));
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert_eq!(
            (source.life, source.word_0x24, source.action),
            (-1, request.owner, 28),
            "the root consumes its buried child's actual life and credit"
        );
        assert!(arena.economy.step_authored_class5_model3_slot_with_target(
            root.source_entity_index,
            None,
            terrain
        ));
        assert!(
            arena
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|row| body_slots.contains(&row.source_entity_index))
                .all(|row| row.action == 29),
            "the next root callback propagates the linked kill tail"
        );
    }
}
