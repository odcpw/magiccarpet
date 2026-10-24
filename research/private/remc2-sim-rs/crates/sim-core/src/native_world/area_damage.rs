//! MC2 area writer adapter over the existing arena and family mailboxes.

use std::collections::BTreeSet;

use super::{ENTITY_DRAW_DISABLED_BIT, current_live_world_source_entity_view};
use crate::{
    Position,
    campaign::source_arena::SourceArena,
    combat::{
        actors::{DamageMailboxMut, DamageWriter},
        area::{self, Actor, AreaDamage, AreaWorld, Shape},
    },
    goat::LiveWorldState,
};

pub(crate) fn apply(
    arena: &mut SourceArena,
    live_world: Option<&mut LiveWorldState>,
    player: Option<(u16, Position)>,
    request: AreaDamage,
) -> Result<u16, String> {
    area::apply(
        &mut ArenaTargets {
            arena,
            live_world,
            player,
        },
        request,
    )
}

struct ArenaTargets<'a> {
    arena: &'a mut SourceArena,
    live_world: Option<&'a mut LiveWorldState>,
    player: Option<(u16, Position)>,
}

fn shape(width: i16, depth: i16, offset: i16, height: i16) -> Shape {
    Shape {
        half_width: width,
        half_depth: depth,
        vertical_offset: offset,
        half_height: height,
    }
}

impl AreaWorld for ArenaTargets<'_> {
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
        if let Some(world) = self.live_world.as_deref()
            && let Some(view) = current_live_world_source_entity_view(world, slot)
            && view.active
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
            // SetEvent144 uses source byte38=1 and particle 144's collision
            // axes; the domain's animation shifts are distinct render fields.
            let extent = super::LEVEL2_BEE_COLLISION_HALF_EXTENT;
            return Some(Actor {
                source_id: row.entity_id,
                class: row.source_class,
                model: row.model,
                position: row.position,
                shape: shape(extent, extent, 0, extent),
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
                source_id: slot,
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
        if let Some(world) = self.live_world.as_deref_mut()
            && current_live_world_source_entity_view(world, slot).is_some_and(|view| view.active)
        {
            if !world.submit_source_damage(slot, payload, owner) {
                return Err(format!(
                    "area live-world mailbox {slot} rejected an admitted target"
                ));
            }
            if let Some(goat) = world.goats().iter().find(|row| row.entity_index == slot) {
                self.arena.economy.sync_authored_goat_projection(goat);
            }
            return Ok(());
        }
        let economy = &mut self.arena.economy;
        if let Some(row) = economy
            .castle
            .as_mut()
            .filter(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(
                &mut row.pending_damage,
                &mut row.pending_attacker_entity_index,
            )
            .submit(i32::from(payload), owner, DamageWriter::Ordinary);
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
            .submit(i32::from(payload), owner, DamageWriter::Ordinary);
            return Ok(());
        }
        if economy
            .house_residents
            .iter()
            .any(|row| row.active && row.source_entity_index == slot)
        {
            if !economy.submit_settlement_damage(
                slot,
                i32::from(payload),
                owner,
                DamageWriter::Ordinary,
            ) {
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
                DamageWriter::Ordinary,
            );
            return Ok(());
        }
        if let Some(row) = economy
            .authored_class5_model3_children
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            DamageMailboxMut::new(&mut row.damage_dword_0x5e_94, &mut row.pending_word_0x62)
                .submit(i32::from(payload), owner, DamageWriter::Ordinary);
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
                DamageWriter::Ordinary,
            );
            return Ok(());
        }
        let row = self
            .arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active)
            .ok_or_else(|| format!("area raw slot {slot} no longer live"))?;
        row.apply_model76_mailbox(owner, payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::disasters::{RingProfile, mc2};

    fn ring_near(
        arena: &mut SourceArena,
        terrain: &super::super::SessionTerrain,
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
                        let slot = mc2::construct(arena, position, RingProfile::default())
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
        let mut session = super::super::tests::level_two_campaign_session();
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
        let ring = ring_near(&mut arena, &terrain, position);
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
            .step_authored_firefly_lifecycle_slot(firefly.source_entity_index, &terrain)
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
}
