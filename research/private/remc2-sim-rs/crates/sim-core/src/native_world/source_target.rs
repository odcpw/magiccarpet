//! Current fixed-address target reads across arena and external actor owners.
//!
//! These values are borrowed for one read. They are neither an entity mirror
//! nor a generation-bound lifetime handle: NewEvent can replace the address
//! between two reads made by the same callback.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::combat::area::{Actor, Shape};
use crate::goat::GoatFamily;
use crate::house::HouseFamily;
use crate::villager::VillagerFamily;

use super::{EntitySnapshot, Level2DynamicArenaKind as Kind, ParticleParameter, area_damage};

fn status_kind(arena: &SourceArena, slot: u16) -> Option<Kind> {
    if let Some(kind) = arena.economy.live_arena_kind(slot) {
        return Some(kind);
    }
    if arena.economy.free_source_slots().contains(&slot) {
        return None;
    }
    // Match read_current's retained flyer owners until the next reclaim
    // prologue. Hidden drawing does not invalidate a source parent pointer.
    if arena
        .economy
        .fireball_projectiles
        .iter()
        .any(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
    {
        Some(Kind::FireballProjectile)
    } else if arena
        .economy
        .castle_projectiles
        .iter()
        .any(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
    {
        Some(Kind::CastleProjectile)
    } else if arena
        .economy
        .possession_projectiles
        .iter()
        .any(|row| row.source_entity_index == slot && !row.active)
    {
        Some(Kind::PossessionProjectile)
    } else {
        None
    }
}

/// Status callbacks dereference a fixed source address, including a retained
/// free row. Fields without a typed owner (A4 and the charge accumulator)
/// remain in the current incarnation's NewEvent record.
pub(crate) fn status_parent(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<EntitySnapshot>, String> {
    let Some(view) = read(arena, context, slot)? else {
        return Ok(None);
    };
    let mut parent = arena.entities[usize::from(slot)].clone();
    let kind = status_kind(arena, slot);
    let retained = kind.is_some() && arena.economy.live_arena_kind(slot).is_none();
    if matches!(kind, Some(Kind::SpellPickup | Kind::SpellReplacement)) {
        parent = context
            .spells
            .and_then(|shared| shared.pickup(slot))
            .cloned()
            .ok_or_else(|| format!("status parent {slot} lost its current spell record"))?;
    }
    parent.active = view.active;
    parent.class = view.class;
    parent.model = view.model;
    parent.source_id = view.source_id;
    parent.position = view.position;
    parent.set_life(view.life);
    parent.set_control_flags(view.control_flags);
    if let Some(yaw) = view.yaw {
        parent.set_yaw(yaw);
    }
    if let Some(shape) = view.collision {
        for (offset, value) in [
            (86, shape.half_width),
            (88, shape.half_depth),
            (90, shape.vertical_offset),
            (92, shape.half_height),
        ] {
            parent.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    if let Some(kind) = kind {
        let economy = &arena.economy;
        // NewEvent initializes mana to zero. Only these typed carriers own a
        // mutable mana field; another carrier cannot inherit the old player.
        let mana = match kind {
            Kind::FireballProjectile => economy
                .fireball_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| row.source_mana),
            Kind::CastleProjectile => economy
                .castle_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| row.source_mana),
            Kind::ManaSphere => economy
                .spheres
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::ManaCarrier => economy
                .carriers
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.carried_mana),
            Kind::CastleRoot => economy.castle_at(slot).map(|row| row.mana),
            Kind::CastleChild | Kind::CastleController | Kind::CastleDefender => economy
                .castle_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::CastleAuxiliary => economy
                .castle_defenders
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::AuthoredClass5 => economy
                .authored_class5
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::AuthoredDragonPart => economy
                .authored_dragon_parts
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::AuthoredClass5Model3Child => economy
                .authored_class5_model3_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::AuthoredModel45 => economy
                .authored_model45
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::HouseResident => economy
                .house_residents
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::BeeCreature => economy
                .bee_creatures
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.mana),
            Kind::SpellPickup | Kind::SpellReplacement => Some(parent.mana),
            _ => Some(0),
        };
        parent.set_mana(
            mana.ok_or_else(|| format!("status parent {slot} lost its current mana owner"))?,
        );
        let speed = match kind {
            Kind::FireballProjectile => economy
                .fireball_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::CastleProjectile => economy
                .castle_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::PossessionProjectile => economy
                .possession_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::AuthoredClass5 => economy
                .authored_class5
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::AuthoredClass5Model3Child => economy
                .authored_class5_model3_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::AuthoredClass5ArcherChild => economy
                .authored_class5_archer_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::BeeCreature => economy
                .bee_creatures
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::HouseResident => economy
                .house_residents
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::AuthoredModel45 => economy
                .authored_model45
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::AuthoredModel76 => economy
                .authored_model76
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::Model76Child => economy
                .model76_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.minimum_speed, row.actual_speed)),
            Kind::Model6SpawnedChild => economy
                .model6_spawned_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (row.min_speed, row.actual_speed)),
            _ => None,
        };
        if let Some((minimum, actual)) = speed {
            parent.record[136..138].copy_from_slice(&minimum.to_le_bytes());
            parent.set_actual_speed(actual);
        }
        let pitch = match kind {
            Kind::FireballProjectile => economy
                .fireball_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::CastleProjectile => economy
                .castle_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::PossessionProjectile => economy
                .possession_projectiles
                .iter()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::AuthoredClass5 => economy
                .authored_class5
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::AuthoredClass5Model3Child => economy
                .authored_class5_model3_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::AuthoredClass5ArcherChild => economy
                .authored_class5_archer_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::BeeCreature => economy
                .bee_creatures
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::AuthoredModel76 => economy
                .authored_model76
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            Kind::Model76Child => economy
                .model76_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.pitch),
            _ => None,
        };
        if let Some(pitch) = pitch {
            parent.set_pitch(pitch);
        }
    }
    let external = context
        .goats
        .and_then(|family| {
            family.goats().iter().find(|row| {
                row.active
                    && row.entity_index == slot
                    && (view.class, view.model) == (super::GOAT_CLASS, super::GOAT_MODEL)
            })
        })
        .map(|row| (row.mana, row.minimum_speed, row.actual_speed))
        .or_else(|| {
            context
                .villagers
                .and_then(|family| {
                    family.villagers().iter().find(|row| {
                        row.source_retained()
                            && row.entity_index == slot
                            && (view.class, view.model) == (5, row.model)
                    })
                })
                .map(|row| (row.mana, row.minimum_speed, row.actual_speed))
        });
    if let Some((mana, minimum, actual)) = external {
        parent.set_mana(mana);
        parent.record[136..138].copy_from_slice(&minimum.to_le_bytes());
        parent.set_actual_speed(actual);
    }
    if let Some(villager) = context.villagers.and_then(|family| {
        family.villagers().iter().find(|row| {
            row.source_retained()
                && row.entity_index == slot
                && (view.class, view.model) == (5, row.model)
        })
    }) {
        parent.set_pitch(villager.pitch);
    }
    Ok(Some(parent))
}

/// Resolve the current typed occupant of an address used by a rival's frozen
/// class-5 list or direct target pointer. Membership remains the caller's
/// pass-start source list; this reads the occupant at the time of the visit.
/// Imported rows return `None` and continue to use their physical record.
pub(crate) fn rival_target_record(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<EntitySnapshot>, String> {
    let Some(cursor) = arena.economy.live_arena_cursor_row(slot) else {
        return Ok(None);
    };
    let (class, action) = arena.economy.live_arena_class_action(slot, cursor.kind)
        .ok_or_else(|| format!("rival target {slot} lost its current action"))?;
    let mut target = status_parent(arena, context, slot)?
        .ok_or_else(|| format!("rival target {slot} lost its current record"))?;
    if target.class != class {
        return Err(format!("rival target {slot} class changed during current read"));
    }
    // The inherited raw row is a cold NewEvent record for this address, not
    // the actor authority. Publish the fields the rival reads into the local
    // callback view; the caller must never copy this typed view back to it.
    target.authority = super::EntityAuthority::FrozenObserved;
    target.record[0..2].copy_from_slice(&slot.to_le_bytes());
    target.record[2] = u8::from(target.active);
    target.record[30..32].copy_from_slice(&target.source_id.to_le_bytes());
    target.record[67] = target.class;
    target.record[68] = target.model;
    target.set_position(target.position);
    target.set_source_action(action);
    target.set_source_player_entity_index(rival_target_relation(arena, context, slot, cursor.kind));
    Ok(Some(target))
}

/// The source's playerEntityIndex word may belong to a typed carrier rather
/// than its cold raw packet. This is a physical source relation, not an
/// instruction to claim the target or alter its other ownership state.
fn rival_target_relation(arena: &SourceArena, context: Context<'_>, slot: u16, kind: Kind) -> u16 {
    let economy = &arena.economy;
    let typed = match kind {
        Kind::ManaSphere => economy.spheres.iter().find(|row| row.active && row.source_entity_index == slot)
            .map(|row| row.player_entity_index),
        Kind::CastleRoot => economy.castle_at(slot).map(|row| row.player_entity_index),
        Kind::CastleChild | Kind::CastleController | Kind::CastleDefender => economy.castle_children.iter()
            .find(|row| row.active && row.source_entity_index == slot).map(|row| row.player_entity_index),
        Kind::CastleAuxiliary => economy.castle_defenders.iter()
            .find(|row| row.active && row.source_entity_index == slot).map(|row| row.player_entity_index),
        Kind::PlayerDeathMarker => economy.player_death_markers.iter()
            .find(|row| row.active && row.source_entity_index == slot).map(|row| row.player_entity_index),
        Kind::HouseResident => economy.house_residents.iter()
            .find(|row| row.active && row.source_entity_index == slot).map(|row| row.owner_entity_index),
        Kind::AuthoredClass5Model3Child => economy.authored_class5_model3_children.iter()
            .find(|row| row.active && row.source_entity_index == slot).map(|row| row.player_owner_entity_index),
        Kind::AuthoredModel45 => context.houses.and_then(|houses| houses.houses().iter()
            .find(|row| row.source_retained() && row.entity_index == slot).map(|row| row.owner_entity_index)),
        Kind::AuthoredClass5 => context.villagers.and_then(|villagers| villagers.villagers().iter()
            .find(|row| row.source_retained() && row.entity_index == slot
                && economy.authored_class5.iter().any(|source| source.active
                    && source.source_entity_index == slot && source.model == row.model))
            .map(|row| row.owner_entity_index)),
        _ => None,
    };
    if let Some(relation) = typed {
        relation
    } else if matches!(kind, Kind::AuthoredClass5 | Kind::BeeCreature | Kind::AuthoredDragonPart | Kind::HouseResident) {
        // These carriers treat a missing owner-map entry as the source's
        // neutral zero, never as permission to read an old raw projection.
        economy.source_owner(slot)
    } else {
        economy.source_owner.get(&slot).copied()
            .unwrap_or_else(|| arena.entities[usize::from(slot)].source_player_entity_index())
    }
}

/// Commit only sub_135C0's word94 write to the same typed generation that
/// supplied its callback view. A replaced address must not receive the write.
pub(crate) fn commit_rival_target_relation(
    arena: &mut SourceArena,
    ordinary: &mut super::Level2OrdinaryResetSession,
    handle: crate::campaign::source_arena::SourceEntityHandle,
    owner: u16,
) -> Result<bool, String> {
    let slot = handle.source_slot;
    if arena.handle(slot) != Some(handle) {
        return Ok(false);
    }
    let Some(kind) = arena.economy.live_arena_kind(slot) else {
        return Ok(false);
    };
    match kind {
        Kind::ManaSphere => {
            let row = arena.economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its mana sphere"))?;
            row.player_entity_index = owner;
        }
        Kind::CastleRoot => arena.economy.castle_at_mut(slot)
            .ok_or_else(|| format!("rival target {slot} lost its Castle"))?.player_entity_index = owner,
        Kind::CastleChild | Kind::CastleController | Kind::CastleDefender => {
            arena.economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its Castle child"))?.player_entity_index = owner;
        }
        Kind::CastleAuxiliary => {
            arena.economy.castle_defenders.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its Castle auxiliary"))?.player_entity_index = owner;
        }
        Kind::PlayerDeathMarker => {
            arena.economy.player_death_markers.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its death marker"))?.player_entity_index = owner;
        }
        Kind::HouseResident => {
            arena.economy.house_residents.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its resident"))?.owner_entity_index = owner;
        }
        Kind::AuthoredClass5Model3Child => {
            arena.economy.authored_class5_model3_children.iter_mut().find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its creature child"))?.player_owner_entity_index = owner;
        }
        Kind::AuthoredModel45 => {
            if let Some(house) = ordinary.houses.house_mut(slot) {
                house.owner_entity_index = owner;
            }
        }
        Kind::AuthoredClass5 => {
            let model = arena.economy.authored_class5.iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("rival target {slot} lost its creature"))?.model;
            if let Some(villager) = ordinary.villagers.villagers_mut().iter_mut()
                .find(|row| row.source_retained() && row.entity_index == slot && row.model == model)
            {
                villager.owner_entity_index = owner;
            }
        }
        _ => {}
    }
    if matches!(kind, Kind::AuthoredClass5 | Kind::BeeCreature | Kind::AuthoredDragonPart | Kind::HouseResident) {
        // These families read word94 through this map, including constructors
        // which leave its neutral zero implicit until the first write.
        arena.economy.source_owner.insert(slot, owner);
    } else if let Some(relation) = arena.economy.source_owner.get_mut(&slot) {
        *relation = owner;
    }
    arena.entities[usize::from(slot)].set_source_player_entity_index(owner);
    Ok(true)
}

pub(crate) fn commit_status_speed(
    arena: &mut SourceArena,
    domains: Option<area_damage::ActorDomains<'_>>,
    parent: &EntitySnapshot,
    speed: i16,
) {
    let slot = parent.index;
    if let Some(mut domains) = domains {
        domains.set_status_speed(slot, parent.class, parent.model, speed);
    }
    let kind = status_kind(arena, slot);
    let retained = kind.is_some() && arena.economy.live_arena_kind(slot).is_none();
    let economy = &mut arena.economy;
    macro_rules! set_speed {
        ($rows:expr) => {
            if let Some(row) = $rows
                .iter_mut()
                .find(|row| (row.active || retained) && row.source_entity_index == slot)
            {
                row.actual_speed = speed;
            }
        };
    }
    match kind {
        Some(Kind::FireballProjectile) => set_speed!(economy.fireball_projectiles),
        Some(Kind::CastleProjectile) => set_speed!(economy.castle_projectiles),
        Some(Kind::PossessionProjectile) => set_speed!(economy.possession_projectiles),
        Some(Kind::BeeCreature) => set_speed!(economy.bee_creatures),
        Some(Kind::AuthoredClass5) => set_speed!(economy.authored_class5),
        Some(Kind::AuthoredClass5Model3Child) => {
            set_speed!(economy.authored_class5_model3_children)
        }
        Some(Kind::AuthoredClass5ArcherChild) => {
            set_speed!(economy.authored_class5_archer_children)
        }
        Some(Kind::HouseResident) => set_speed!(economy.house_residents),
        Some(Kind::AuthoredModel45) => set_speed!(economy.authored_model45),
        Some(Kind::AuthoredModel76) => set_speed!(economy.authored_model76),
        Some(Kind::Model76Child) => set_speed!(economy.model76_children),
        Some(Kind::Model13SpawnedChild) => set_speed!(economy.model13_spawned_children),
        Some(Kind::Model6SpawnedChild) => set_speed!(economy.model6_spawned_children),
        Some(Kind::AuthoredModel59) => set_speed!(economy.authored_model59),
        Some(Kind::Model59SmokeChild) => set_speed!(economy.model59_smoke_children),
        Some(Kind::AuthoredModel53) => set_speed!(economy.authored_model53),
        Some(Kind::BeyondSightPulseEffect) => set_speed!(economy.beyond_sight_effects),
        _ => {}
    }
    if let Some(raw) = arena.entities.get_mut(usize::from(slot)) {
        raw.set_actual_speed(speed);
    }
}

pub(crate) fn status_castle_mana(
    arena: &SourceArena,
    shared: &super::Level2SharedSpellResetSession,
    parent: &EntitySnapshot,
) -> Result<Option<i32>, String> {
    let slot = match parent.special_relation {
        super::EntitySpecialRelation::PlayerState(index) => {
            let player = shared.players.get(usize::from(index)).ok_or_else(|| {
                format!("status parent {} has no PlayerState {index}", parent.index)
            })?;
            u16::from_le_bytes([player.record[42], player.record[43]])
        }
        super::EntitySpecialRelation::GlobalSpecialState => {
            shared.global_special_state.castle_entity_index
        }
        _ => 0,
    };
    Ok(status_parent(
        arena,
        Context {
            spells: Some(shared),
            ..Context::default()
        },
        slot,
    )?
    .map(|castle| castle.mana))
}

/// Commit just the two parent fields written by 6A480/6B1C0/6B310. In
/// particular a typed replacement must not receive the old player's pose,
/// life, identity or cache when a holder still points at its numeric slot.
pub(crate) fn commit_status_parent(
    arena: &mut SourceArena,
    shared: &mut super::Level2SharedSpellResetSession,
    domains: Option<&mut area_damage::ActorDomains<'_>>,
    before: &EntitySnapshot,
    after: &EntitySnapshot,
) -> Result<(), String> {
    let slot = before.index;
    if slot == 0 || slot != after.index || usize::from(slot) >= arena.entities.len() {
        return Err("status parent publication changed its fixed source address".into());
    }
    let mask = before.control_flags ^ after.control_flags;
    let flags = after.control_flags;
    let kind = status_kind(arena, slot);
    let retained = kind.is_some() && arena.economy.live_arena_kind(slot).is_none();
    if matches!(kind, Some(Kind::SpellPickup | Kind::SpellReplacement)) {
        let row = shared
            .pickup_mut(slot)
            .ok_or_else(|| format!("status parent {slot} lost its spell owner"))?;
        row.set_control_flags((row.control_flags & !mask) | (flags & mask));
        row.set_mana_regen(after.mana_regen);
    } else if mask != 0
        && !domains.is_some_and(|domains| {
            domains.set_status_flags(slot, before.class, before.model, mask, flags)
        })
    {
        let economy = &mut arena.economy;
        macro_rules! set_flags {
            ($rows:expr) => {
                if let Some(row) = $rows
                    .iter_mut()
                    .find(|row| (row.active || retained) && row.source_entity_index == slot)
                {
                    row.control_flags = (row.control_flags & !mask) | (flags & mask);
                }
            };
        }
        macro_rules! set_state_flags {
            ($rows:expr) => {
                if let Some(row) = $rows.iter_mut().find(|row| row.source_entity_index == slot) {
                    row.control_flags = (row.control_flags & !mask) | (flags & mask);
                }
            };
        }
        match kind {
            Some(Kind::FireballProjectile) => set_flags!(economy.fireball_projectiles),
            Some(Kind::CastleProjectile) => set_flags!(economy.castle_projectiles),
            Some(Kind::CastleRoot) => {
                if let Some(row) = economy.castle_at_mut(slot) {
                    row.control_flags = (row.control_flags & !mask) | (flags & mask);
                }
            }
            Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender) => {
                set_flags!(economy.castle_children)
            }
            Some(Kind::CastleAuxiliary) => set_flags!(economy.castle_defenders),
            Some(Kind::ManaSphere) => set_flags!(economy.spheres),
            Some(Kind::BeeCreature) => set_flags!(economy.bee_creatures),
            Some(Kind::AuthoredDragonPart) => set_flags!(economy.authored_dragon_parts),
            Some(Kind::AuthoredClass5Model3Child) => {
                set_flags!(economy.authored_class5_model3_children)
            }
            Some(Kind::AuthoredClass5ArcherChild) => {
                set_flags!(economy.authored_class5_archer_children)
            }
            Some(Kind::AuthoredClass5) => {
                set_state_flags!(economy.authored_class5_archer);
                set_state_flags!(economy.authored_fireflies);
                set_state_flags!(economy.authored_dragons);
                set_state_flags!(economy.authored_class5_model3);
            }
            Some(Kind::HouseResident) => set_flags!(economy.house_residents),
            Some(Kind::DeathBurst) => set_flags!(economy.death_bursts),
            Some(Kind::PlayerDeathMarker) => set_flags!(economy.player_death_markers),
            Some(Kind::SpeedTrail) => set_flags!(economy.speed_trails),
            Some(Kind::TerrainStripCloseTrigger) => {
                set_flags!(economy.terrain_strip_close_triggers)
            }
            Some(Kind::AuthoredModel1) => set_flags!(economy.authored_model1),
            Some(Kind::AuthoredModel45) => set_flags!(economy.authored_model45),
            Some(Kind::AuthoredModel13) => set_flags!(economy.authored_model13),
            Some(Kind::Model13SpawnedChild) => set_flags!(economy.model13_spawned_children),
            Some(Kind::AuthoredModel6) => set_flags!(economy.authored_model6),
            Some(Kind::Model6SpawnedChild) => set_flags!(economy.model6_spawned_children),
            Some(Kind::AuthoredModel17) => set_flags!(economy.authored_model17),
            Some(Kind::Model17SpawnedChild) => set_flags!(economy.model17_spawned_children),
            Some(Kind::AuthoredModel22) => set_flags!(economy.fields.authored_model22),
            Some(Kind::AuthoredModel76) => set_flags!(economy.authored_model76),
            Some(Kind::Model76Child) => set_flags!(economy.model76_children),
            Some(Kind::Model76TailSpawn) => set_flags!(economy.model76_tail_spawns),
            Some(Kind::AuthoredModel32) => set_flags!(economy.authored_model32),
            Some(Kind::AuthoredModel59) => set_flags!(economy.authored_model59),
            Some(Kind::Model59SmokeChild) => set_flags!(economy.model59_smoke_children),
            Some(Kind::AuthoredModel53) => set_flags!(economy.authored_model53),
            Some(Kind::AuthoredModel5WaterSplash) => {
                set_flags!(economy.authored_model5_water_splashes)
            }
            // These carriers do not model mutable protection flags. Their
            // NewEvent scalar record remains the owner of those status bits.
            _ => {}
        }
    }
    let raw = &mut arena.entities[usize::from(slot)];
    raw.set_control_flags((raw.control_flags & !mask) | (flags & mask));
    raw.set_mana_regen(after.mana_regen);
    Ok(())
}
pub(crate) fn bind_castle_upgrade_record(
    arena: &mut SourceArena,
    slot: u16,
    particle: &ParticleParameter,
) -> Result<(), String> {
    let limit = super::campaign_shared_families::particle_animation_limit(particle.render_mode)
        .ok_or_else(|| format!("Castle particle41 render mode {} has no source animation limit", particle.render_mode))?;
    let raw = &mut arena.entities[usize::from(slot)];
    raw.record[94..96].copy_from_slice(&41_u16.to_le_bytes());
    raw.record[96] = 0;
    raw.record[97] = limit;
    Ok(())
}

pub(crate) fn advance_castle_upgrade_frame(arena: &mut SourceArena, slot: u16) {
    let raw = &mut arena.entities[usize::from(slot)];
    if raw.record[96].cast_signed() < raw.record[97].cast_signed() {
        raw.record[96] = raw.record[96].wrapping_add(1);
    }
}

/// 389F0 includes Entities[0] in its direct Castle pointer dereference.
pub(crate) fn castle_upgrade_target(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<(Position, Shape)>, String> {
    let view = if slot == 0 {
        arena.entities.first().map(View::raw)
    } else { read(arena, context, slot)? };
    view.map(|view| {
        view.collision.map(|shape| (view.position, shape))
            .ok_or_else(|| format!("Castle upgrade target {slot} lacks current source axes"))
    }).transpose()
}

pub(crate) fn collector_target(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>, String> {
    let Some(view) = read(arena, context, slot)? else { return Ok(None) };
    let raw = &arena.entities[usize::from(slot)];
    let kind = arena.economy.live_arena_kind(slot);
    let mut mana = if kind.is_none() { raw.mana } else { 0 };
    let mut relation = if kind.is_none() { raw.source_player_entity_index() } else { 0 };
    let mut stage = if kind.is_none() { raw.source_dword_10() } else { 0 };
    let mut shape = view.collision;
    if let Some(row) = arena.economy.spheres.iter()
        .find(|row| kind == Some(Kind::ManaSphere) && row.active && row.source_entity_index == slot)
    {
        mana = row.mana;
        relation = row.player_entity_index;
    } else if kind == Some(Kind::CastleRoot) && let Some(row) = arena.economy.castle_at(slot) {
        mana = row.mana;
        relation = row.player_entity_index;
        stage = i32::from(row.level);
    } else if let Some(row) = arena.economy.castle_children.iter()
        .find(|row| row.active && row.source_entity_index == slot
            && matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary)))
    {
        mana = row.mana;
        relation = row.player_entity_index;
        shape = shape.or_else(|| context.particle_shape(row.visual_index));
    } else if let Some(row) = arena.economy.carriers.iter()
        .find(|row| kind == Some(Kind::ManaCarrier) && row.active && row.source_entity_index == slot)
    {
        mana = row.carried_mana;
        relation = row.owner_entity_index;
    }
    // Zero-relation class10 and nonclass10 branches never test these axes.
    let shape = if view.class == 10 && relation != 0 {
        shape.ok_or_else(|| format!("Castle collector target {slot} lacks current collision axes"))?
    } else { shape.unwrap_or(Shape { half_width: 0, half_depth: 0, vertical_offset: 0, half_height: 0 }) };
    Ok(Some(super::level2_dynamic_economy::Level2CastleCollectorTarget {
        source_slot: slot, source_class: view.class, source_model: view.model, position: view.position,
        player_entity_index: relation, mana, source_dword_10: stage,
        collision_yaw: shape.vertical_offset, collision_pitch: shape.half_width,
        collision_roll: shape.half_depth, collision_fov: shape.half_height,
    }))
}

/// sub_36AE0 rewrites the current word94 at every fixed address, including
/// retained rows. Semantic IDs and sphere/Castle ownership are distinct.
pub(crate) fn transfer_possession_player_relations(
    arena: &mut SourceArena,
    houses: &mut HouseFamily,
    villagers: &mut VillagerFamily,
    from: u16,
    to: u16,
) -> Result<usize, String> {
    let mut changed = 0;
    for index in 1..arena.entities.len().min(1_000) {
        let slot = index as u16;
        let Some(view) = read(
            arena,
            Context {
                houses: Some(houses),
                villagers: Some(villagers),
                ..Context::default()
            },
            slot,
        )?
        else {
            continue;
        };
        let kind = arena.economy.live_arena_kind(slot);
        let raw_relation = arena.entities[index].source_player_entity_index();
        let relation = match kind {
            Some(Kind::ManaSphere) => arena
                .economy
                .spheres
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.player_entity_index),
            Some(Kind::CastleRoot) => arena
                .economy
                .castle_at(slot)
                .map(|row| row.player_entity_index),
            Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender) => arena
                .economy
                .castle_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.player_entity_index),
            Some(Kind::CastleAuxiliary) => arena
                .economy
                .castle_defenders
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.player_entity_index),
            Some(Kind::PlayerDeathMarker) => arena
                .economy
                .player_death_markers
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| row.player_entity_index),
            _ => None,
        }
        .or_else(|| {
            (view.class == 10 && view.model == 45)
                .then(|| {
                    houses
                        .houses()
                        .iter()
                        .find(|row| row.source_retained() && row.entity_index == slot)
                        .map(|row| row.owner_entity_index)
                })
                .flatten()
        })
        .or_else(|| {
            (view.class == 5)
                .then(|| {
                    villagers
                        .villagers()
                        .iter()
                        .find(|row| {
                            row.source_retained()
                                && row.entity_index == slot
                                && row.model == view.model
                        })
                        .map(|row| row.owner_entity_index)
                })
                .flatten()
        })
        .unwrap_or(raw_relation);
        if relation != from {
            continue;
        }
        arena.entities[index].set_source_player_entity_index(to);
        match kind {
            Some(Kind::ManaSphere) => {
                if let Some(row) = arena
                    .economy
                    .spheres
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                {
                    row.player_entity_index = to;
                }
            }
            Some(Kind::CastleRoot) => {
                if let Some(row) = arena.economy.castle_at_mut(slot) {
                    row.player_entity_index = to;
                }
            }
            Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender) => {
                if let Some(row) = arena
                    .economy
                    .castle_children
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                {
                    row.player_entity_index = to;
                }
            }
            Some(Kind::CastleAuxiliary) => {
                if let Some(row) = arena
                    .economy
                    .castle_defenders
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                {
                    row.player_entity_index = to;
                }
            }
            Some(Kind::PlayerDeathMarker) => {
                if let Some(row) = arena
                    .economy
                    .player_death_markers
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                {
                    row.player_entity_index = to;
                }
            }
            _ => {}
        }
        if view.class == 10
            && view.model == 45
            && let Some(row) = houses.house_mut(slot)
        {
            row.owner_entity_index = to;
        }
        if view.class == 5
            && let Some(row) = villagers.villagers_mut().iter_mut().find(|row| {
                row.source_retained() && row.entity_index == slot && row.model == view.model
            })
        {
            row.owner_entity_index = to;
        }
        changed += 1;
    }
    Ok(changed)
}

/// A callback-local pointer view plus the raw record domain. Economy-owned
/// mutations go to their current carrier; retained free rows stay physical.
pub(crate) struct CastleCollectorConsumer<'a> {
    pub(crate) target: Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>,
    pub(crate) entities: &'a mut [EntitySnapshot],
    pub(crate) shared: &'a mut super::Level2SharedSpellResetSession,
}

pub(crate) fn publish_collector_player_resources(
    arena: &SourceArena,
    shared: &mut super::Level2SharedSpellResetSession,
    rivals: &mut super::level2_nyphur::RivalWizardRoster,
    slot: u16,
) {
    if arena.economy.live_arena_kind(slot).is_some() { return; }
    let Some(row) = arena.entities.get(usize::from(slot))
        .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1))
    else { return };
    if let Some(handle) = arena.handle(slot) && let Some(rival) = rivals.get_mut(handle) {
        rival.sync_spell_target_resources(row);
    }
    shared.sync_player_entity_from_authority(row);
}

impl super::level2_dynamic_economy::Level2CastleCollectorConsumer for CastleCollectorConsumer<'_> {
    fn record_damage(&mut self) {
        self.shared.record_castle_collector_damage();
    }
    fn read(&self, _economy: &super::Level2EconomyState, slot: u16)
        -> Option<super::level2_dynamic_economy::Level2CastleCollectorTarget>
    {
        self.target.filter(|row| row.source_slot == slot)
    }

    fn set_attraction(&mut self, economy: &mut super::Level2EconomyState, slot: u16, in_range: bool, holder: u16) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::ManaSphere) && let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            if in_range { row.control_flags |= 0x40; row.assigned_defender_slot = holder; }
            else { row.control_flags &= !0x40; }
        } else if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) {
            if in_range { row.control_flags |= 0x40; } else { row.control_flags &= !0x40; }
        } else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            if in_range { row.control_flags |= 0x40; row.target_sphere_slot = holder; }
            else { row.control_flags &= !0x40; }
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            raw.set_control_flags(if in_range { raw.control_flags | 0x40 } else { raw.control_flags & !0x40 });
            if in_range { raw.set_source_word_96(holder); }
        }
    }

    fn add_mana(&mut self, economy: &mut super::Level2EconomyState, slot: u16, delta: i32) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) { row.mana = row.mana.wrapping_add(delta); }
        else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.mana = row.mana.wrapping_add(delta);
        } else if kind == Some(Kind::ManaCarrier) && let Some(row) = economy.carriers.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.carried_mana = row.carried_mana.wrapping_add(delta);
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) {
            let previous = self.target.filter(|row| row.source_slot == slot).map_or(raw.mana, |row| row.mana);
            raw.set_mana(previous.wrapping_add(delta));
        }
    }

    fn hide(&mut self, economy: &mut super::Level2EconomyState, slot: u16) {
        let kind = economy.live_arena_kind(slot);
        if kind == Some(Kind::ManaSphere) && let Some(row) = economy.spheres.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.pending_delete = true;
            row.control_flags |= 0x400;
        } else if kind == Some(Kind::CastleRoot) && let Some(row) = economy.castle_at_mut(slot) { row.control_flags |= 0x400; }
        else if matches!(kind, Some(Kind::CastleChild | Kind::CastleController | Kind::CastleDefender | Kind::CastleAuxiliary))
            && let Some(row) = economy.castle_children.iter_mut().find(|row| row.active && row.source_entity_index == slot) {
            row.control_flags |= 0x400;
        }
        if let Some(raw) = self.entities.get_mut(usize::from(slot)) { raw.disable_drawing(); }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Context<'a> {
    pub(crate) goats: Option<&'a GoatFamily>,
    pub(crate) houses: Option<&'a HouseFamily>,
    pub(crate) villagers: Option<&'a VillagerFamily>,
    pub(crate) spells: Option<&'a super::Level2SharedSpellResetSession>,
    pub(crate) player: Option<(u16, Position, i32)>,
    pub(crate) player_orientation: Option<(i16, i16)>,
    pub(crate) particles: Option<&'a [ParticleParameter]>,
    pub(crate) map_type: Option<super::MapType>,
    pub(crate) house_parameters:
        Option<&'a std::collections::BTreeMap<u8, crate::house::HouseParameter>>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        goats: &'a GoatFamily,
        houses: &'a HouseFamily,
        villagers: &'a VillagerFamily,
    ) -> Self {
        Self {
            goats: Some(goats),
            houses: Some(houses),
            villagers: Some(villagers),
            ..Self::default()
        }
    }

    /// Split the ordinary owner at its privacy boundary; the returned context
    /// retains only the actual actor domains required by current-slot reads.
    pub(crate) fn from_ordinary(
        goats: &'a GoatFamily,
        ordinary: &'a super::Level2OrdinaryResetSession,
    ) -> Self {
        Self {
            house_parameters: Some(&ordinary.house_parameters),
            ..Self::new(goats, &ordinary.houses, &ordinary.villagers)
        }
    }

    fn external(self, slot: u16) -> Option<View> {
        if let Some(row) = self.goats.and_then(|family| {
            family
                .goats()
                .iter()
                .find(|row| row.active && row.entity_index == slot)
        }) {
            let mut view = View::external(super::goat_source_entity_view(row));
            view.yaw = Some(row.yaw);
            view.roll = Some(row.roll);
            view.relations.parent = Some(row.parent_entity_index);
            view.relations.stage_mode = Some(row.stage_var_2 as u8);
            view.relations.stage_index = Some(row.stage_var_1 as u8);
            view.relations.stage_relation = Some(row.stage_target);
            return Some(view);
        }
        if let Some(row) = self.houses.and_then(|family| {
            family
                .houses()
                .iter()
                .find(|row| row.source_retained() && row.entity_index == slot)
        }) {
            return Some(View::external(super::house_source_entity_view(row)));
        }
        self.villagers
            .and_then(|family| {
                family
                    .villagers()
                    .iter()
                    .find(|row| row.source_retained() && row.entity_index == slot)
            })
            .map(|row| {
                let mut view = View::external(super::villager_source_entity_view(row));
                view.yaw = Some(row.yaw);
                view.roll = Some(row.roll);
                view.relations.parent = Some(row.stage.parent);
                view.relations.stage_mode = Some(row.stage_var_2 as u8);
                view.relations.stage_index = Some(row.stage_var_1 as u8);
                view.relations.stage_relation = Some(row.stage.relation);
                view.relations.next = Some(row.linked_entity_index);
                view
            })
    }

    fn particle_shape(self, index: u16) -> Option<Shape> {
        self.particles
            .and_then(|rows| rows.get(usize::from(index)))
            .map(|particle| Shape {
                half_width: (particle.speed / 2) as i16,
                half_depth: (particle.speed / 2) as i16,
                vertical_offset: (particle.rotation_speed / 2) as i16,
                half_height: (particle.rotation_speed / 2) as i16,
            })
    }
}

/// One eligible current occupant for `sub_108B0`. Search order belongs to
/// the live map chain; this callback-local frame supplies only current fields.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PossessionCollisionTarget {
    pub(crate) slot: u16,
    pub(crate) source_id: u16,
    pub(crate) model: u8,
    pub(crate) position: Position,
    pub(crate) shape: Shape,
}

pub(crate) struct PossessionCollisionFrame {
    pub(crate) targets: Vec<PossessionCollisionTarget>,
    pub(crate) guidance: Option<Position>,
    pub(crate) guidance_source_id: Option<u16>,
    pub(crate) acquisition_candidates: Vec<(u16, Position)>,
}

pub(crate) fn possession_collision_frame(
    arena: &SourceArena,
    context: Context<'_>,
    projectile_slot: u16,
    terrain: &super::SessionTerrain,
    lists: Option<&area_damage::EffectLists>,
) -> Result<PossessionCollisionFrame, String> {
    let Some(projectile) = arena
        .economy
        .possession_projectiles
        .iter()
        .find(|row| row.active && row.source_entity_index == projectile_slot)
    else {
        return Ok(PossessionCollisionFrame {
            targets: Vec::new(),
            guidance: None,
            guidance_source_id: None,
            acquisition_candidates: Vec::new(),
        });
    };
    let owner = projectile.source_id_entity_index;
    let guidance_view = if projectile.target_source_entity_index == 0 {
        None
    } else {
        read(arena, context, projectile.target_source_entity_index)?
    };
    let guidance_source_id = guidance_view.map(|view| view.source_id);
    let guidance = guidance_view.map(|view| Position {
        z: view.position.z.wrapping_add(if view.model == 2 {
            0
        } else {
            view.collision.map_or(0, |shape| shape.vertical_offset)
        }),
        ..view.position
    });
    let (mana_slots, house_slots, model22_slots) = lists.map_or_else(
        || {
            (
                arena.economy.possession_guidance_source_slots.clone(),
                Vec::new(),
                Vec::new(),
            )
        },
        |lists| lists.possession_guidance_slots(arena),
    );
    let mut acquisition_candidates = Vec::new();
    let top = |view: View| Position {
        z: view.position.z.wrapping_add(if view.model == 2 {
            0
        } else {
            view.collision.map_or(0, |shape| shape.vertical_offset)
        }),
        ..view.position
    };
    for slot in mana_slots {
        let Some(view) = read(arena, context, slot)? else {
            continue;
        };
        let raw = &arena.entities[usize::from(slot)];
        let relation = if view.model == 39 {
            arena
                .economy
                .spheres
                .iter()
                .find(|row| row.source_entity_index == slot && row.active)
                .map_or(raw.source_player_entity_index(), |row| {
                    row.player_entity_index
                })
        } else if view.model == 57 {
            raw.source_spell_parent_entity_index()
        } else {
            continue;
        };
        if raw.record[61] != 0 && relation != owner {
            acquisition_candidates.push((slot, top(view)));
        }
    }
    for &slot in &house_slots {
        let Some(view) = read(arena, context, slot)? else {
            continue;
        };
        let raw = &arena.entities[usize::from(slot)];
        if raw.record[61] == 0 {
            continue;
        }
        let house = context.houses.and_then(|family| {
            family.houses().iter().find(|row| {
                row.source_retained()
                    && row.entity_index == slot
                    && view.class == 10
                    && view.model == 45
            })
        });
        let (relation, blocked) = if let Some(house) = house {
            (
                house.owner_entity_index,
                house.parameter.possession_blocked(),
            )
        } else {
            let selector = arena
                .economy
                .authored_model45
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map_or(raw.record[74], |row| row.byte_0x46);
            let blocked = context
                .house_parameters
                .and_then(|parameters| parameters.get(&selector))
                .ok_or_else(|| {
                    format!("Possession guidance slot {slot} lacks current BLDGPRM {selector}")
                })?
                .possession_blocked();
            (raw.source_player_entity_index(), blocked)
        };
        if relation != owner && !blocked {
            acquisition_candidates.push((slot, top(view)));
        }
    }
    for parent_slot in model22_slots {
        let raw = &arena.entities[usize::from(parent_slot)];
        if raw.record[61] == 0 {
            continue;
        }
        let Some(parent) = read(arena, context, parent_slot)? else {
            continue;
        };
        let mut slot = parent.relations.next.unwrap_or(0);
        let mut visited = std::collections::BTreeSet::new();
        while slot != 0 && visited.insert(slot) {
            let Some(view) = read(arena, context, slot)? else {
                break;
            };
            acquisition_candidates.push((slot, top(view)));
            slot = view.relations.next.unwrap_or(0);
        }
    }
    let moved = arena
        .economy
        .possession_projectile_moved(
            projectile_slot,
            terrain,
            context.houses.map_or(&[], |family| family.houses()),
            &house_slots,
            &arena.entities,
            Some(guidance),
            Some(&acquisition_candidates),
        )
        .ok_or("Possession collision frame lost its current flyer")?;
    let mut targets = Vec::new();
    for slot in crate::level1_mission::source_map_search_slots(
        &arena.economy.map_chains,
        moved.position,
        &super::LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
    ) {
        let index = usize::from(slot);
        let kind = arena.economy.live_arena_kind(slot);
        let Some(raw) = arena.entities.get(index) else {
            continue;
        };
        // Most slots cannot satisfy sub_108B0's narrow class/model gate.
        // Inspect the current typed header before projecting any actor, so
        // unrelated families need no collision binding for this callback.
        let candidate_header = match kind {
            Some(
                Kind::ManaSphere
                | Kind::ManaCarrier
                | Kind::PlayerDeathMarker
                | Kind::AuthoredModel45,
            ) => true,
            Some(Kind::AuthoredClass5) => arena
                .economy
                .authored_class5
                .iter()
                .any(|row| row.active && row.source_entity_index == slot && row.model == 22),
            Some(_) => false,
            None => matches!((raw.class, raw.model), (5, 22) | (10, 39 | 40 | 45 | 57)),
        };
        if !candidate_header {
            continue;
        }
        let Some(view) = read(arena, context, slot)? else {
            continue;
        };
        if view.control_flags & 8 == 0 {
            continue;
        }
        let mut relation = raw.source_player_entity_index();
        let eligible = match (view.class, view.model) {
            (5, 22) => true,
            (10, 39) => {
                if let Some(row) = arena.economy.spheres.iter().find(|row| {
                    kind == Some(Kind::ManaSphere) && row.active && row.source_entity_index == slot
                }) {
                    relation = row.player_entity_index;
                }
                true
            }
            (10, 40) => {
                if let Some(row) = arena.economy.player_death_markers.iter().find(|row| {
                    kind == Some(Kind::PlayerDeathMarker)
                        && row.active
                        && row.source_entity_index == slot
                }) {
                    relation = row.player_entity_index;
                } else if let Some(row) = arena.economy.carriers.iter().find(|row| {
                    kind == Some(Kind::ManaCarrier) && row.active && row.source_entity_index == slot
                }) {
                    relation = row.owner_entity_index;
                }
                true
            }
            (10, 45) => {
                if let Some(house) = context.houses.and_then(|houses| {
                    houses
                        .houses()
                        .iter()
                        .find(|house| house.entity_index == slot && house.source_retained())
                }) {
                    relation = house.owner_entity_index;
                    !house.parameter.possession_blocked()
                } else {
                    let selector = arena
                        .economy
                        .authored_model45
                        .iter()
                        .find(|row| {
                            kind == Some(Kind::AuthoredModel45)
                                && row.active
                                && row.source_entity_index == slot
                        })
                        .map_or(raw.record[74], |row| row.byte_0x46);
                    !context
                        .house_parameters
                        .and_then(|parameters| parameters.get(&selector))
                        .ok_or_else(|| {
                            format!(
                                "Possession model45 slot {slot} lacks current BLDGPRM {selector}"
                            )
                        })?
                        .possession_blocked()
                }
            }
            (10, 57) => {
                view.relations
                    .parent
                    .unwrap_or_else(|| raw.source_spell_parent_entity_index())
                    != owner
            }
            _ => false,
        };
        if !eligible
            || (view.model != 57 || view.class != 10)
                && (view.source_id == owner || relation == owner)
        {
            continue;
        }
        let shape = view
            .collision
            .ok_or_else(|| format!("Possession target {slot} lacks current collision axes"))?;
        targets.push(PossessionCollisionTarget {
            slot,
            source_id: view.source_id,
            model: view.model,
            position: view.position,
            shape,
        });
    }
    Ok(PossessionCollisionFrame {
        targets,
        guidance,
        guidance_source_id,
        acquisition_candidates,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Relations {
    pub(crate) target: Option<u16>,
    pub(crate) attacker: Option<u16>,
    pub(crate) parent: Option<u16>,
    pub(crate) stage_mode: Option<u8>,
    pub(crate) stage_index: Option<u8>,
    pub(crate) stage_relation: Option<u16>,
    /// Source word0x34 child chain, distinct from word0x32 peer following.
    pub(crate) next: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct View {
    pub(crate) slot: u16,
    pub(crate) source_id: u16,
    pub(crate) active: bool,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) position: Position,
    pub(crate) life: i32,
    pub(crate) control_flags: u32,
    pub(crate) yaw: Option<i16>,
    pub(crate) roll: Option<i16>,
    pub(crate) relations: Relations,
    /// Collision axes are present only when the owning carrier or retained
    /// particle binding supplies them. Missing axes never come from old raw
    /// bytes left at a typed occupant's address.
    pub(crate) collision: Option<Shape>,
}

impl View {
    pub(crate) const fn drawing_disabled(self) -> bool {
        self.control_flags & 0x400 != 0
    }

    fn raw(row: &EntitySnapshot) -> Self {
        Self {
            slot: row.index,
            source_id: row.source_id,
            active: row.active,
            class: row.class,
            model: row.model,
            position: row.position,
            life: row.life,
            control_flags: row.control_flags,
            yaw: Some(row.yaw),
            roll: Some(row.source_roll()),
            relations: Relations {
                target: Some(u16::from_le_bytes([row.record[154], row.record[155]])),
                attacker: Some(row.word_26()),
                parent: Some(row.source_spell_parent_entity_index()),
                stage_mode: Some(row.record[77]),
                stage_index: Some(row.record[76]),
                stage_relation: Some(u16::from_le_bytes([row.record[78], row.record[79]])),
                next: Some(u16::from_le_bytes([row.record[56], row.record[57]])),
            },
            collision: Some(Shape {
                half_width: row.array_pitch(),
                half_depth: row.array_roll(),
                vertical_offset: row.array_yaw(),
                half_height: row.array_fov(),
            }),
        }
    }

    fn actor(slot: u16, actor: Actor, life: i32) -> Self {
        Self {
            slot,
            source_id: actor.source_id,
            active: true,
            class: actor.class,
            model: actor.model,
            position: actor.position,
            life,
            control_flags: actor.control_flags,
            yaw: None,
            roll: None,
            relations: Relations::default(),
            collision: Some(actor.shape),
        }
    }

    fn external(row: crate::level1_mission::Level1SourceEntityView) -> Self {
        Self {
            slot: row.source_entity_index,
            source_id: row.source_id,
            active: row.active,
            class: row.class,
            model: row.model,
            position: row.position,
            life: row.life,
            control_flags: row.control_flags,
            yaw: None,
            roll: None,
            relations: Relations {
                target: Some(row.target_entity_index),
                attacker: Some(row.current_attacker),
                ..Relations::default()
            },
            collision: Some(Shape {
                half_width: row.collision_half_width,
                half_depth: row.collision_half_depth,
                vertical_offset: row.collision_vertical_offset,
                half_height: row.collision_half_height,
            }),
        }
    }
}

/// Alliance directly dereferences these addresses even after reclaim. The
/// result supplements pointer reads without changing frozen search lists.
pub(crate) fn alliance_direct_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    if source.record[77] != 14 {
        return Ok(std::collections::BTreeSet::new());
    }
    direct_creature_references(arena, source)
}

/// sub_1E580 and sub_1E320 read fixed parent/selected-target records in the
/// Army follow and pursuit modes, independently of frozen search membership.
pub(crate) fn summoned_direct_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    if !matches!(source.record[77], 13 | 16 | 17) {
        return Ok(std::collections::BTreeSet::new());
    }
    direct_creature_references(arena, source)
}

fn direct_creature_references(
    arena: &SourceArena,
    source: &EntitySnapshot,
) -> Result<std::collections::BTreeSet<u16>, String> {
    let mut referenced = std::collections::BTreeSet::new();
    let mut pending = vec![
        source.source_spell_parent_entity_index(),
        source.source_word_96(),
        source.word_26(),
        u16::from_le_bytes([source.record[102], source.record[103]]),
        u16::from_le_bytes([source.record[56], source.record[57]]),
    ];
    while let Some(slot) = pending.pop() {
        if slot == 0 || referenced.contains(&slot) {
            continue;
        }
        let Some(target) = read(arena, Context::default(), slot)? else {
            continue;
        };
        referenced.insert(slot);
        pending.extend([
            target.relations.target.unwrap_or(0),
            target.relations.attacker.unwrap_or(0),
            target.relations.next.unwrap_or(0),
        ]);
    }
    Ok(referenced)
}

/// Current sub_10780 map-chain targets for one arrow. Ordinary actor domains
/// and typed arena carriers are resolved at this callback, not at reset.
pub(crate) fn arrow_collision_frame(
    arena: &SourceArena,
    slot: u16,
    context: Context<'_>,
) -> Result<super::creatures::archer::ArrowCollisionFrame, String> {
    use super::creatures::archer::{ArrowCollisionFrame, ArrowCollisionTarget};
    let arrow = arena.economy.authored_class5_archer_children.iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .ok_or_else(|| format!("Arrow {slot} lost its current carrier"))?;
    if arrow.action != super::creatures::archer::ARCHER_CHILD_ACTION {
        return Ok(ArrowCollisionFrame { targets: Vec::new(), owner_class_model: None });
    }
    let mut targets = Vec::new();
    let offsets = super::guided_projectile::collision_search_offsets(arrow.collision_half_width);
    for target_slot in crate::level1_mission::source_map_search_slots(&arena.map_chains, arrow.position, &offsets) {
        let Some(view) = read(arena, context, target_slot)? else { continue; };
        if view.control_flags & 8 == 0 { continue; }
        let shape = view.collision.ok_or_else(|| format!("Arrow target {target_slot} has no current axes"))?;
        let mana = match arena.economy.archer_arrow_target_properties(target_slot, &arena.entities) {
            Some((_, _, _, mana)) => mana,
            None if view.control_flags & 0x8010 == 0 => 0,
            None => return Err(format!("Protected arrow target {target_slot} has no current mana")),
        };
        targets.push(ArrowCollisionTarget {
            target: super::level2_dynamic_economy::Level2FireballCollisionTarget {
                source_slot: target_slot, source_id: view.source_id,
                class: view.class, model: view.model, position: view.position,
                half_width: shape.half_width, vertical_offset: shape.vertical_offset,
                half_height: shape.half_height,
            },
            control_flags: view.control_flags,
            mana,
        });
    }
    let owner_class_model = read(arena, context, arrow.entity_id)?.map(|view| (view.class, view.model));
    Ok(ArrowCollisionFrame { targets, owner_class_model })
}

/// Read the current fixed-address occupant; allocation is separate from a
/// source pointer's validity.
pub(crate) fn read(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<View>, String> {
    let mut view = read_current(arena, context, slot)?;
    if let Some(view) = view.as_mut() {
        view.control_flags = status_flags(arena, slot, view.control_flags);
    }
    Ok(view)
}

/// Carriers with no mutable flag field retain these source scalar fields in
/// their cold NewEvent record. Allocation retires that record before reuse.
pub(crate) fn status_flags(arena: &SourceArena, slot: u16, flags: u32) -> u32 {
    let raw_owned = match status_kind(arena, slot) {
        Some(Kind::ManaCarrier | Kind::PossessionProjectile | Kind::PossessionEffect
            | Kind::AuthoredSwitchProducer | Kind::AuthoredClass14
            | Kind::Model22SpawnedChild | Kind::ManaAttraction | Kind::BeyondSightPulseEffect) => true,
        Some(Kind::AuthoredClass5) => arena.economy.authored_class5.iter()
            .any(|row| row.active && row.source_entity_index == slot && matches!(row.model, 9 | 14)),
        _ => false,
    };
    if !raw_owned { return flags; }
    flags | arena.entities.get(usize::from(slot)).map_or(0, |row| {
        row.control_flags & (crate::spell_activation::SHIELD_MANA_STATUS_BIT
            | crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT
            | crate::spell_activation::INVISIBLE_STATUS_BIT)
    })
}

fn read_current(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<View>, String> {
    if slot == 0 {
        return Ok(None);
    }
    let Some(raw) = arena.entities.get(usize::from(slot)) else {
        return Ok(None);
    };
    if let Some(cursor) = arena.economy.live_arena_cursor_row(slot) {
        // These descriptor rows lend their mutable actor state to the three
        // external families. A different replacement kind cannot inherit it.
        if matches!(cursor.kind, Kind::AuthoredClass5 | Kind::AuthoredModel45)
            && let Some(external) = context.external(slot)
            && arena
                .economy
                .live_arena_class_action(slot, cursor.kind)
                .is_some_and(|(class, _)| class == external.class)
            && match cursor.kind {
                Kind::AuthoredClass5 => arena.economy.authored_class5.iter().any(|row| {
                    row.active && row.source_entity_index == slot && row.model == external.model
                }),
                Kind::AuthoredModel45 => external.model == 45,
                _ => false,
            }
        {
            let mut external = external;
            if let Some(state) = arena
                .economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                external.relations.next = Some(state.linked_head);
            }
            return Ok(Some(external));
        }
        return dynamic(arena, context, slot, cursor.kind).map(Some);
    }
    // The numeric dispatcher stops visiting a hidden typed flyer, but its
    // fixed row is still allocated until the following reclaim prologue.
    // Light and pointer consumers must see that hidden incarnation meanwhile.
    if !arena.economy.free_source_slots().contains(&slot)
        && let Some(projectile) = arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
    {
        return Ok(Some(fireball(projectile, context)));
    }
    if !arena.economy.free_source_slots().contains(&slot) {
        if let Some(row) = arena
            .economy
            .castle_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active && row.drawing_disabled)
        {
            return Ok(Some(castle_projectile(row)));
        }
        if let Some(row) = arena
            .economy
            .possession_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot && !row.active)
        {
            return Ok(Some(possession_projectile(row)));
        }
    }
    let mut view = if raw.active
        && let Some(external) = context.external(slot)
        && (raw.class, raw.model) == (external.class, external.model)
    {
        external
    } else {
        View::raw(raw)
    };
    if raw.active
        && raw.class == 3
        && let Some((player_slot, position, life)) = context.player
        && player_slot == slot
    {
        view.position = position;
        view.life = life;
        if let Some((yaw, roll)) = context.player_orientation {
            view.yaw = Some(yaw);
            view.roll = Some(roll);
        }
    }
    Ok(Some(view))
}

/// The type-2 objective reads this byte only after the current target dies.
/// Resolve it from the same owner as `read`, never a typed row's raw shadow.
pub(crate) fn font_type_index(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
) -> Result<Option<u8>, String> {
    let Some(current) = read(arena, context, slot)? else {
        return Ok(None);
    };
    if (current.class, current.model) == (10, 45)
        && let Some(house) = context.houses.and_then(|family| {
            family
                .houses()
                .iter()
                .find(|row| row.source_retained() && row.entity_index == slot)
        })
    {
        return Ok(Some(house.parameter.destruction_successor));
    }
    let Some(cursor) = arena.economy.live_arena_cursor_row(slot) else {
        return Ok(Some(arena.entities[usize::from(slot)].font_type_index));
    };
    match cursor.kind {
        // These constructors retain NewEvent's zero byte, and their source
        // callbacks do not write fontTypeIndex (4AC40, 4E5F0, 500C0,
        // 64E20, 31760, 30F60, 35940).
        Kind::ManaSphere | Kind::AuthoredTree | Kind::AuthoredModel6 => Ok(Some(0)),
        Kind::AuthoredModel76 => arena
            .economy
            .authored_model76
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map(|row| Some(row.font_type_index as u8))
            .ok_or_else(|| format!("objective target {slot} lost its Fire Spheres carrier")),
        Kind::SpellPickup | Kind::SpellReplacement => context
            .spells
            .and_then(|spells| spells.pickup(slot))
            .filter(|row| (row.class, row.model) == (current.class, current.model))
            .map(|row| Some(row.font_type_index))
            .ok_or_else(|| format!("objective target {slot} lost its spell carrier")),
        kind => Err(format!(
            "objective target {slot} has no font-type binding for current {kind:?}"
        )),
    }
}

fn dynamic(
    arena: &SourceArena,
    context: Context<'_>,
    slot: u16,
    kind: Kind,
) -> Result<View, String> {
    let economy = &arena.economy;
    if matches!(
        kind,
        Kind::ManaSphere
            | Kind::AuthoredTree
            | Kind::CastleRoot
            | Kind::CastleAuxiliary
            | Kind::BeeCreature
            | Kind::HouseResident
            | Kind::AuthoredDragonPart
            | Kind::AuthoredClass5Model3Child
            | Kind::AuthoredClass5
    ) {
        let actor = area_damage::read_arena_actor(arena, slot)
            .ok_or_else(|| format!("target slot {slot} lost its current {kind:?} actor"))?;
        let life = area_damage::read_arena_life(arena, slot)
            .ok_or_else(|| format!("target slot {slot} lost its current {kind:?} life"))?;
        let mut view = View::actor(slot, actor, life);
        (view.yaw, view.roll) = orientation(arena, kind, slot);
        view.relations = relations(arena, kind, slot);
        return Ok(view);
    }
    macro_rules! carrier {
        ($rows:expr, $row:ident, $class:expr, $model:expr, $id:expr, $life:expr, $flags:expr) => {{
            let $row = $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost current {kind:?} carrier"))?;
            View {
                slot,
                source_id: $id,
                active: true,
                class: $class,
                model: $model,
                position: $row.position,
                life: i32::from($life),
                control_flags: $flags,
                yaw: None,
                roll: None,
                relations: Relations::default(),
                collision: None,
            }
        }};
    }
    let mut view = match kind {
        Kind::FireballProjectile => fireball(
            economy
                .fireball_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Fireball carrier"))?,
            context,
        ),
        Kind::CastleProjectile => castle_projectile(
            economy
                .castle_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Castle flyer"))?,
        ),
        Kind::PossessionProjectile => possession_projectile(
            economy
                .possession_projectiles
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its current Possession flyer"))?,
        ),
        Kind::PossessionEffect => carrier!(
            economy.possession_effects,
            row,
            row.source_class,
            row.source_model,
            row.source_id_entity_index,
            row.life,
            // NewAdd0A0C clears bits 0 and 3, then sets bits 0 and map-link 2.
            5
        ),
        Kind::ManaCarrier => {
            let mut view = carrier!(
                economy.carriers,
                row,
                crate::level1_mission::MANA_CARRIER_CLASS,
                crate::level1_mission::MANA_CARRIER_MODEL,
                row.owner_entity_index,
                row.life,
                12
            );
            view.collision = context.map_type.and_then(|map_type| {
                context.particle_shape(169).map(|mut shape| {
                    if map_type == super::MapType::Cave {
                        shape.half_width = 256;
                        shape.half_depth = 256;
                        shape.half_height = 768;
                    }
                    shape
                })
            });
            view
        }
        Kind::SpellPickup | Kind::SpellReplacement => {
            // The typed arena row owns allocation/dispatch; the shared spell
            // owner retains this live record's mutable life, flags and axes.
            // The raw arena copy need not have been published yet.
            let family = match kind {
                Kind::SpellPickup => economy
                    .spell_pickups
                    .iter()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .map(|row| row.family),
                _ => economy
                    .spell_replacements
                    .iter()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .map(|row| row.family),
            }
            .ok_or_else(|| format!("target slot {slot} lost its current spell carrier"))?;
            let row = context
                .spells
                .and_then(|spells| spells.pickup(slot))
                .filter(|row| row.active && row.class == 15 && row.model == family)
                .ok_or_else(|| {
                    format!("target slot {slot} lacks its current shared spell record")
                })?;
            View::raw(row)
        }
        Kind::CastleChild | Kind::CastleController | Kind::CastleDefender => {
            let mut view = carrier!(
                economy.castle_children,
                row,
                row.source_class,
                row.model,
                row.id_entity_index,
                row.life,
                row.control_flags
            );
            let row = economy
                .castle_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its Castle child"))?;
            if row.model == 43 && row.action == 45 {
                view.yaw = Some(0);
                view.roll = Some(0);
                view.collision = Some(Shape {
                    vertical_offset: row.yaw.cast_signed(),
                    half_width: row.roll.cast_signed(),
                    half_depth: row.roll.cast_signed(),
                    half_height: row.roll.cast_signed(),
                });
                return Ok(view);
            }
            view.yaw = Some(row.yaw as i16);
            view.roll = Some(row.roll as i16);
            view.relations.target = Some(row.target_sphere_slot);
            view.relations.attacker = Some(row.damage_attacker_entity_index);
            view.relations.parent = Some(row.parent_entity_index);
            if kind == Kind::CastleDefender {
                // This is sub_4ABA0's real class3/model3 balloon carrier;
                // the separate class5/model15 owner is CastleAuxiliary.
                view.collision = context.map_type.and_then(|map_type| {
                    context.particle_shape(row.visual_index).map(|mut shape| {
                        if map_type == super::MapType::Cave {
                            shape.half_width = 256;
                            shape.half_depth = 256;
                            shape.half_height = 768;
                        }
                        shape
                    })
                });
            }
            view
        }
        Kind::DeathBurst => carrier!(
            economy.death_bursts,
            row,
            10,
            1,
            row.id_entity_index,
            row.life_ticks,
            row.control_flags
        ),
        Kind::PlayerDeathMarker => {
            let row = economy
                .player_death_markers
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its player death marker"))?;
            let mut view = carrier!(
                economy.player_death_markers,
                marker,
                10,
                40,
                slot,
                marker.life,
                marker.control_flags
            );
            view.collision = context.particle_shape(row.particle_parameter_index);
            view
        }
        Kind::SpeedTrail => carrier!(
            economy.speed_trails,
            row,
            row.class,
            row.model,
            row.source_id,
            row.life,
            row.control_flags
        ),
        Kind::TerrainStripCloseTrigger => carrier!(
            economy.terrain_strip_close_triggers,
            row,
            row.class,
            row.model,
            slot,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredSwitchProducer => carrier!(
            economy.authored_switch_producers,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            // AddSwitchXX copies NewEvent's maxLife; visits change dword10,
            // not life. Its collision bit is cleared by the constructor.
            300,
            1 | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredClass14 => {
            let row = economy
                .authored_class14
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost its class14 carrier"))?;
            let mut view = carrier!(
                economy.authored_class14,
                row,
                14,
                row.model,
                slot,
                // sub_514E0 copies NewEvent's maxLife before SetHalfSpeed.
                300,
                12 | u32::from(row.control_flags) | if row.drawing_enabled { 0 } else { 0x400 }
            );
            view.collision = context.particle_shape(row.particle).map(|mut shape| {
                if row.model == 5 {
                    shape.half_width = row.shift_rot_x as i16;
                    shape.half_depth = row.shift_rot_x as i16;
                    shape.half_height = row.shift_rot_y as i16;
                }
                shape
            });
            view
        }
        Kind::ManaAttraction => carrier!(
            economy.mana_attraction_fields,
            row,
            10,
            54,
            slot,
            row.life,
            // AddAuxiliary_50500 clears bits 0 and 3, then sets bit 0.
            1
        ),
        Kind::AuthoredModel1 => carrier!(
            economy.authored_model1,
            row,
            10,
            row.model,
            slot,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel45 => {
            let mut view = carrier!(
                economy.authored_model45,
                row,
                row.source_class,
                row.model,
                slot,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            let row = economy
                .authored_model45
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("current model45 carrier was resolved above");
            view.collision = Some(Shape {
                half_width: row.view_pitch,
                half_depth: row.view_roll,
                vertical_offset: row.view_yaw,
                half_height: row.view_fov,
            });
            view
        }
        Kind::AuthoredModel13 => carrier!(
            economy.authored_model13,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model13SpawnedChild => {
            let mut view = carrier!(
                economy.model13_spawned_children,
                row,
                row.source_class,
                row.model,
                row.parent_entity_id,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            let row = economy
                .model13_spawned_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("the current smoke14 carrier was just read");
            view.yaw = Some(row.yaw);
            view.collision = context.particle_shape(row.particle_index);
            view
        }
        Kind::AuthoredModel6 => carrier!(
            economy.authored_model6,
            row,
            10,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags
        ),
        Kind::Model6SpawnedChild => carrier!(
            economy.model6_spawned_children,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredModel17 => {
            let row = economy
                .authored_model17
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("target slot {slot} lost current {kind:?} carrier"))?;
            let mut view = carrier!(
                economy.authored_model17,
                row,
                row.source_class,
                row.model,
                row.entity_id,
                row.life,
                row.control_flags
            );
            view.yaw = Some(row.yaw);
            view.relations.target = Some(row.target_source_entity_index);
            view
        }
        Kind::Model17SpawnedChild => carrier!(
            economy.model17_spawned_children,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredModel22 | Kind::Model22SpawnedChild => economy
            .fields
            .source_rows()
            .iter()
            .find(|row| row.index == slot)
            .map(View::raw)
            .ok_or_else(|| format!("target slot {slot} lost its Wind carrier"))?,
        Kind::AuthoredModel76 => carrier!(
            economy.authored_model76,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model76Child => carrier!(
            economy.model76_children,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model76TailSpawn => carrier!(
            economy.model76_tail_spawns,
            row,
            row.source_class,
            row.model,
            row.parent_entity_id,
            row.life,
            row.control_flags
        ),
        Kind::AuthoredClass5ArcherChild => {
            let row = economy.authored_class5_archer_children.iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("Arrow {slot} lost its current collision axes"))?;
            let mut view = carrier!(
                economy.authored_class5_archer_children,
                row,
                row.source_class,
                row.model,
                row.entity_id,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            view.collision = Some(Shape {
                half_width: row.collision_half_width,
                half_depth: row.collision_half_width,
                vertical_offset: row.collision_vertical_offset,
                half_height: row.collision_half_height,
            });
            view
        }
        Kind::AuthoredModel32 => carrier!(
            economy.authored_model32,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel59 => carrier!(
            economy.authored_model59,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::Model59SmokeChild => {
            let mut view = carrier!(
                economy.model59_smoke_children,
                row,
                row.source_class,
                row.model,
                slot,
                row.life,
                row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
            );
            let row = economy
                .model59_smoke_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .expect("the current smoke13 carrier was just read");
            view.yaw = Some(row.yaw);
            view.collision = context.particle_shape(row.particle_index);
            view
        }
        Kind::AuthoredModel53 => carrier!(
            economy.authored_model53,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags | if row.drawing_enabled { 0 } else { 0x400 }
        ),
        Kind::AuthoredModel5WaterSplash => carrier!(
            economy.authored_model5_water_splashes,
            row,
            row.source_class,
            row.model,
            row.entity_id,
            row.life,
            row.control_flags
        ),
        _ => {
            return Err(format!(
                "target slot {slot} has no current projection for {kind:?}"
            ));
        }
    };
    let (yaw, roll) = orientation(arena, kind, slot);
    if yaw.is_some() {
        view.yaw = yaw;
    }
    if roll.is_some() {
        view.roll = roll;
    }
    let current_relations = relations(arena, kind, slot);
    view.relations = Relations {
        target: current_relations.target.or(view.relations.target),
        attacker: current_relations.attacker.or(view.relations.attacker),
        parent: current_relations.parent.or(view.relations.parent),
        stage_mode: current_relations.stage_mode.or(view.relations.stage_mode),
        stage_index: current_relations.stage_index.or(view.relations.stage_index),
        stage_relation: current_relations
            .stage_relation
            .or(view.relations.stage_relation),
        next: current_relations.next.or(view.relations.next),
    };
    Ok(view)
}

fn fireball(row: &super::Level2FireballProjectile, context: Context<'_>) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.owner_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: i32::from(row.life),
        control_flags: row.control_flags | if row.drawing_disabled { 0x400 } else { 0 },
        yaw: Some(row.yaw),
        roll: Some(row.roll),
        relations: Relations {
            target: Some(row.target_source_entity_index),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: context.particle_shape(340),
    }
}

fn castle_projectile(row: &super::Level2CastleProjectile) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.owner_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: row.life,
        control_flags: row.control_flags | if row.drawing_disabled { 0x400 } else { 0 },
        yaw: Some(row.yaw),
        roll: Some(row.source_roll),
        relations: Relations {
            target: Some(row.source_word_96),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: Some(row.collision_shape),
    }
}

fn possession_projectile(row: &super::Level2PossessionProjectile) -> View {
    View {
        slot: row.source_entity_index,
        source_id: row.source_id_entity_index,
        active: row.active,
        class: row.source_class,
        model: row.source_model,
        position: row.position,
        life: i32::from(row.life),
        control_flags: 4
            | u32::from(row.acquisition_attempted) * 2
            | u32::from(row.threat_accounted) * 0x2000
            | if row.active { 0 } else { 0x400 },
        yaw: Some(row.yaw),
        roll: Some(row.roll),
        relations: Relations {
            target: Some(row.target_source_entity_index),
            attacker: Some(row.holder_entity_index),
            ..Relations::default()
        },
        collision: None,
    }
}

fn relations(arena: &SourceArena, kind: Kind, slot: u16) -> Relations {
    let economy = &arena.economy;
    match kind {
        Kind::BeeCreature => economy
            .bee_creatures
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_entity_index.unwrap_or(0)),
                attacker: Some(row.damage_attacker_entity_index),
                parent: Some(row.parent_entity_index),
                stage_mode: Some(row.stage_var2),
                stage_index: Some(row.stage_var1),
                stage_relation: Some(row.stage_relation),
                next: Some(row.linked_child_entity_index),
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5 => {
            let mut relation = economy
                .authored_class5
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| Relations {
                    target: Some(row.word_0x96_150),
                    attacker: Some(row.word_0x26),
                    ..Relations::default()
                })
                .unwrap_or_default();
            if let Some(state) = economy
                .authored_class5_runtime
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.next = Some(state.linked_head);
            }
            if let Some(state) = economy
                .authored_class5_archer
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.stage.parent);
                relation.stage_mode = Some(state.stage.mode);
                relation.stage_index = Some(state.stage.index);
                relation.stage_relation = Some(state.stage.relation);
                relation.next = Some(state.word_0x34);
            } else if let Some(state) = economy
                .authored_fireflies
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
            } else if let Some(state) = economy
                .authored_dragons
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
            } else if let Some(state) = economy
                .authored_class5_model3
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                relation.parent = Some(state.parent_entity_index);
                relation.stage_mode = Some(state.stage_var2);
                relation.stage_index = Some(state.stage_var1);
                relation.stage_relation = Some(state.stage_relation);
                relation.next = Some(state.word_0x34);
            }
            relation
        }
        Kind::HouseResident => economy
            .house_residents
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_entity_index),
                attacker: Some(row.current_attacker),
                parent: Some(row.stage.parent),
                stage_mode: Some(row.stage_var_2 as u8),
                stage_index: Some(row.stage_var_1 as u8),
                stage_relation: Some(row.stage.relation),
                next: Some(row.linked_entity_index),
            })
            .unwrap_or_default(),
        Kind::AuthoredDragonPart => economy
            .authored_dragon_parts
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.next_entity_index),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5Model3Child => economy
            .authored_class5_model3_children
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.word_0x96),
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.word_0x34),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::FireballProjectile => economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.target_source_entity_index),
                attacker: Some(row.holder_entity_index),
                ..Relations::default()
            })
            .unwrap_or_default(),
        Kind::AuthoredClass5ArcherChild => economy
            .authored_class5_archer_children
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| Relations {
                target: Some(row.word_0x96),
                attacker: Some(row.word_0x26),
                parent: Some(row.parent_entity_index),
                next: Some(row.word_0x34),
                ..Relations::default()
            })
            .unwrap_or_default(),
        _ => Relations::default(),
    }
}

fn orientation(arena: &SourceArena, kind: Kind, slot: u16) -> (Option<i16>, Option<i16>) {
    macro_rules! angles {
        ($rows:expr) => {
            $rows
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .map(|row| (Some(row.yaw as i16), Some(row.roll as i16)))
                .unwrap_or((None, None))
        };
    }
    match kind {
        Kind::ManaSphere => arena.economy.spheres.iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| (Some(row.yaw), None))
            .unwrap_or((None, None)),
        Kind::FireballProjectile => angles!(arena.economy.fireball_projectiles),
        Kind::BeeCreature => angles!(arena.economy.bee_creatures),
        Kind::AuthoredClass5 => angles!(arena.economy.authored_class5),
        Kind::AuthoredDragonPart => angles!(arena.economy.authored_dragon_parts),
        Kind::AuthoredClass5ArcherChild => angles!(arena.economy.authored_class5_archer_children),
        Kind::AuthoredModel22 => angles!(arena.economy.fields.authored_model22),
        Kind::Model22SpawnedChild => angles!(arena.economy.fields.model22_spawned_children),
        Kind::Model76Child => angles!(arena.economy.model76_children),
        // AddTree and actions 0..2 retain NewEvent's pose. The typed Tree
        // yaw/roll fields are array_0x52 collision axes, not pose angles.
        Kind::AuthoredTree => (Some(0), Some(0)),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_publication_does_not_write_an_old_external_actor_at_a_reused_address() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let mut goats = session.session.state.live.goats.clone();
        let mut ordinary = session.session.state.live.ordinary.clone();
        let mut shared = session.session.state.live.shared_spells.clone();
        let old = goats.goats().iter().find(|row| row.active).unwrap().clone();
        let old_generation = arena.economy.slot_generation(old.entity_index);
        let old_handle = arena.handle(old.entity_index).unwrap();
        assert!(rival_target_record(
            &arena,
            Context::from_ordinary(&goats, &ordinary),
            old.entity_index,
        ).unwrap().is_none(), "imported actors retain their physical raw row");
        // Retain the old external projection as an explicit boundary control.
        // Current arena metadata, not that stale family row, owns the address.
        crate::world::terrain::cave_features::reclaim(&mut arena, &mut terrain, old.entity_index);
        let slot = arena
            .economy
            .spawn_fireball_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: super::super::FireballProjectileOrigin::PlayerSpell,
                    position: old.position,
                    yaw: 0,
                    pitch: 0,
                    axis_position: Position::default(),
                    source_id_entity_index: session.state.entity_index,
                    holder_entity_index: 0,
                    source_subspell_index: 100,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                },
                0,
            )
            .unwrap();
        assert_eq!(slot, old.entity_index);
        arena.retire_previous_source_projection(slot);
        assert_ne!(arena.economy.slot_generation(slot), old_generation);
        // A poisoned retired packet cannot stand in for the replacement.
        let raw = &mut arena.entities[usize::from(slot)];
        raw.class = 5;
        raw.model = 1;
        raw.set_position(Position { x: 1, y: 2, z: 3 });
        raw.set_mana(999);
        raw.set_source_action(99);
        let target = rival_target_record(&arena, Context::from_ordinary(&goats, &ordinary), slot)
            .unwrap().unwrap();
        assert_eq!((target.class, target.model, target.action, target.mana), (9, 0, 0, 50));
        assert_eq!(target.source_id, session.state.entity_index);
        assert_eq!(target.position, old.position);
        assert!(!arena.entities[usize::from(slot)].active,
            "a callback-local target view must not resurrect the raw shadow");
        let current_handle = arena.handle(slot).unwrap();
        assert!(!commit_rival_target_relation(&mut arena, &mut ordinary, old_handle, 777).unwrap());
        assert_eq!(arena.entities[usize::from(slot)].source_player_entity_index(), 0,
            "a stale generation cannot write the replacement");
        assert!(commit_rival_target_relation(&mut arena, &mut ordinary, current_handle, 777).unwrap());
        assert_eq!(rival_target_record(&arena, Context::from_ordinary(&goats, &ordinary), slot)
            .unwrap().unwrap().source_player_entity_index(), 777);
        let context = Context::from_ordinary(&goats, &ordinary);
        let parent = status_parent(&arena, context, slot).unwrap().unwrap();
        assert_eq!((parent.class, parent.model), (9, 0));
        let mut after = parent.clone();
        after.set_control_flags(parent.control_flags | crate::spell_activation::SHIELD_MANA_STATUS_BIT);
        let mut domains = area_damage::ActorDomains::new(&mut goats, &mut ordinary);
        commit_status_parent(&mut arena, &mut shared, Some(&mut domains), &parent, &after).unwrap();
        commit_status_speed(&mut arena, Some(domains), &parent, -13);
        assert_eq!(
            goats
                .goats()
                .iter()
                .find(|row| row.entity_index == slot)
                .unwrap(),
            &old
        );
        let current = status_parent(&arena, Context::from_ordinary(&goats, &ordinary), slot)
            .unwrap()
            .unwrap();
        assert_ne!(
            current.control_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
            0
        );
        assert_eq!(current.actual_speed, -13);

        // The model-9 constructor leaves a neutral owner-map entry implicit.
        // A real word94 write must create it for later creature consumers.
        let skeleton = arena.economy.allocate_authored_class5(900, 9, old.position, &terrain)
            .expect("ordinary model-9 constructor");
        let skeleton_slot = skeleton.source_entity_index;
        arena.retire_previous_source_projection(skeleton_slot);
        assert!(!arena.economy.source_owner.contains_key(&skeleton_slot));
        arena.entities[usize::from(skeleton_slot)].set_source_player_entity_index(444);
        assert_eq!(rival_target_record(&arena, Context::from_ordinary(&goats, &ordinary), skeleton_slot)
            .unwrap().unwrap().source_player_entity_index(), 0,
            "an implicit neutral owner must not borrow a stale raw word");
        let skeleton_handle = arena.handle(skeleton_slot).unwrap();
        assert!(commit_rival_target_relation(&mut arena, &mut ordinary, skeleton_handle, 777).unwrap());
        assert_eq!(arena.economy.source_owner(skeleton_slot), 777);
        assert_eq!(rival_target_record(&arena, Context::from_ordinary(&goats, &ordinary), skeleton_slot)
            .unwrap().unwrap().source_player_entity_index(), 777);
    }

    #[test]
    fn status_parent_reads_and_writes_hidden_flyers_until_their_allocator_release() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut shared = session.session.state.live.shared_spells.clone();
        let fireball = arena
            .economy
            .spawn_fireball_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: super::super::FireballProjectileOrigin::PlayerSpell,
                    position: session.state.position,
                    yaw: 21,
                    pitch: 22,
                    axis_position: Position::default(),
                    source_id_entity_index: session.state.entity_index,
                    holder_entity_index: 0,
                    source_subspell_index: 100,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                },
                0,
            )
            .unwrap();
        let castle = arena
            .economy
            .spawn_castle_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
                    position: session.state.position,
                    axis_position: session.state.position,
                    yaw: 21,
                    pitch: 22,
                    source_id_entity_index: session.state.entity_index,
                    source_subspell_index: 0,
                    source_mana: 70,
                    source_a4_byte_154: 0,
                },
                0,
                0,
                &session.session.state.live.terrain,
            )
            .unwrap();
        let possession = arena
            .economy
            .spawn_possession_projectile(
                session.state.position,
                session.state.entity_index,
                0,
                21,
                22,
                0,
                session.state.position,
            )
            .unwrap();
        for (slot, mana) in [(fireball, 50), (castle, 70), (possession, 0)] {
            arena.retire_previous_source_projection(slot);
            let before = status_parent(&arena, Context::default(), slot)
                .unwrap()
                .unwrap();
            assert_eq!(before.mana, mana);
            let mut protected = before.clone();
            protected.set_control_flags(
                before.control_flags | crate::spell_activation::SHIELD_MANA_STATUS_BIT,
            );
            commit_status_parent(&mut arena, &mut shared, None, &before, &protected).unwrap();
            if slot == fireball {
                let row = arena
                    .economy
                    .fireball_projectiles
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
                    .unwrap();
                row.active = false;
                row.drawing_disabled = true;
            } else if slot == castle {
                let row = arena
                    .economy
                    .castle_projectiles
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
                    .unwrap();
                row.active = false;
                row.drawing_disabled = true;
            } else {
                arena
                    .economy
                    .possession_projectiles
                    .iter_mut()
                    .find(|row| row.source_entity_index == slot)
                    .unwrap()
                    .active = false;
            }
            assert!(arena.economy.live_arena_kind(slot).is_none());
            let hidden = status_parent(&arena, Context::default(), slot)
                .unwrap()
                .unwrap();
            assert!(!hidden.active);
            assert_eq!(
                hidden.mana, mana,
                "hidden source parent retains its actual mana"
            );
            assert_eq!(hidden.minimum_speed(), before.minimum_speed());
            assert_ne!(
                hidden.control_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
                0
            );
            let mut expired = hidden.clone();
            expired.set_control_flags(
                hidden.control_flags & !crate::spell_activation::SHIELD_MANA_STATUS_BIT,
            );
            commit_status_parent(&mut arena, &mut shared, None, &hidden, &expired).unwrap();
            commit_status_speed(&mut arena, None, &hidden, -13);
            let current = status_parent(&arena, Context::default(), slot)
                .unwrap()
                .unwrap();
            assert_eq!(
                current.control_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
                0
            );
            assert_eq!(current.actual_speed, -13);
            assert_eq!(current.position, before.position);
            // Controlled allocator release, not a complete hide/reclaim
            // scenario: after release the fixed physical row is authoritative.
            arena.economy.release_slot(slot, None);
            let released = status_parent(&arena, Context::default(), slot)
                .unwrap()
                .unwrap();
            assert_eq!(released.mana, arena.entities[usize::from(slot)].mana);
            assert_eq!(released.class, arena.entities[usize::from(slot)].class);
        }
    }
    #[test]
    fn status_publication_uses_current_typed_fields_and_new_event_scalar_retirement() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut shared = session.session.state.live.shared_spells.clone();
        let cached_player = shared.player_entity.clone();
        let slot = arena.economy.spawn_possession_projectile(
            session.state.position, session.state.entity_index, 0, 21, 0, 0, session.state.position,
        ).expect("ordinary Possession NewEvent");
        arena.retire_previous_source_projection(slot);
        let parent = status_parent(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((parent.class, parent.model), (9, 1));
        assert_eq!(parent.special_relation, super::super::EntitySpecialRelation::GlobalSpecialState);
        let pose = parent.position;
        let mut after = parent.clone();
        after.set_control_flags(parent.control_flags | crate::spell_activation::SHIELD_MANA_STATUS_BIT);
        after.set_mana_regen(-77);
        commit_status_parent(&mut arena, &mut shared, None, &parent, &after).unwrap();
        let read_back = status_parent(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!(read_back.control_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
            crate::spell_activation::SHIELD_MANA_STATUS_BIT);
        assert_eq!((read_back.position, read_back.mana_regen), (pose, -77));
        assert_eq!(shared.player_entity, cached_player);
        assert_eq!(status_castle_mana(&arena, &shared, &read_back).unwrap(), None);
        let mut expired = read_back.clone();
        expired.set_control_flags(expired.control_flags & !crate::spell_activation::SHIELD_MANA_STATUS_BIT);
        commit_status_parent(&mut arena, &mut shared, None, &read_back, &expired).unwrap();
        assert_eq!(read(&arena, Context::default(), slot).unwrap().unwrap().control_flags
            & crate::spell_activation::SHIELD_MANA_STATUS_BIT, 0);

        let bee = arena.economy.allocate_authored_bee(1, pose, None, 3000, &session.session.state.live.terrain)
            .expect("ordinary Bee NewEvent");
        let bee_slot = bee.source_entity_index;
        arena.retire_previous_source_projection(bee_slot);
        let parent = status_parent(&arena, Context::default(), bee_slot).unwrap().unwrap();
        let mut protected = parent.clone();
        protected.set_control_flags(protected.control_flags | crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT);
        commit_status_parent(&mut arena, &mut shared, None, &parent, &protected).unwrap();
        assert_ne!(area_damage::read_arena_actor(&arena, bee_slot).unwrap().control_flags
            & crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT, 0);
        let actual = arena.economy.bee_creatures.iter().find(|row| row.source_entity_index == bee_slot).unwrap();
        assert_ne!(actual.control_flags & crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT, 0);
        let mut replacement = parent.clone();
        replacement.set_control_flags(parent.control_flags);
        commit_status_parent(&mut arena, &mut shared, None, &protected, &replacement).unwrap();
        assert_eq!(area_damage::read_arena_actor(&arena, bee_slot).unwrap().control_flags
            & crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT, 0);
        // The same NewEvent retirement used by actual slot replacement drops
        // scalar status even for a carrier without a mutable flag field.
        arena.retire_previous_source_projection(slot);
        let cold = status_parent(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!(cold.mana_regen, 0);
        assert_eq!(cold.control_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT, 0);
    }

    #[test]
    fn current_typed_creature_overrides_retained_raw_target_bytes() {
        let mut session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let position = session.state.position;
        let bee = arena
            .economy
            .allocate_authored_bee(1, position, None, 3000, &session.session.state.live.terrain)
            .expect("ordinary authored Bee constructor");
        let slot = bee.source_entity_index;
        let raw = &mut arena.entities[usize::from(slot)];
        raw.class = 9;
        raw.model = 9;
        raw.set_position(Position { x: 3, y: 4, z: 5 });
        raw.set_life(-400);
        raw.set_control_flags(0x400);
        raw.record[154..156].copy_from_slice(&944_u16.to_le_bytes());
        raw.record[76] = 71;
        raw.record[78..80].copy_from_slice(&988_u16.to_le_bytes());
        raw.record[56..58].copy_from_slice(&977_u16.to_le_bytes());
        let current = arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        current.target_entity_index = Some(111);
        current.damage_attacker_entity_index = 222;
        current.parent_entity_index = 333;
        current.stage_var2 = 14;
        current.stage_var1 = 6;
        current.stage_relation = 444;
        current.linked_child_entity_index = 555;

        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model), (bee.source_class, bee.model));
        assert_eq!((target.position, target.life), (bee.position, bee.life));
        assert_eq!(target.control_flags, bee.control_flags);
        assert!(target.collision.is_some());
        assert_eq!(
            target.relations,
            Relations {
                target: Some(111),
                attacker: Some(222),
                parent: Some(333),
                stage_mode: Some(14),
                stage_index: Some(6),
                stage_relation: Some(444),
                next: Some(555),
            }
        );

        // The objective consumer must use this same current carrier. A dead
        // retained packet cannot kill a live Bee; a live packet cannot hide
        // the current Bee's death from a type-1 objective.
        use crate::campaign::objectives::target_death::{Form, achieved_in_arena};
        let stage = super::super::RuntimeStage {
            kind: 1,
            flags: 1,
            axis_x: 0,
            axis_y: 0,
            relation: super::super::RuntimeRelation::LiveEntity(slot),
        };
        assert!(!achieved_in_arena(Form::TargetDeath, &stage, &arena, Context::default()).unwrap());
        arena
            .economy
            .bee_creatures
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap()
            .life = -1;
        arena.entities[usize::from(slot)].set_life(400);
        assert!(achieved_in_arena(Form::TargetDeath, &stage, &arena, Context::default()).unwrap());
        arena.entities[usize::from(slot)].set_life(-400);

        // Removing the typed occupant changes the same address read. No old
        // value or lifetime handle can keep selecting the former Bee.
        arena
            .economy
            .bee_creatures
            .retain(|row| row.source_entity_index != slot);
        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model, target.life), (9, 9, -400));
        assert!(target.drawing_disabled());
        assert_eq!(target.relations.target, Some(944));
        assert_eq!(target.relations.stage_index, Some(71));
        assert_eq!(target.relations.stage_relation, Some(988));
        assert_eq!(target.relations.next, Some(977));

        // A death can allocate a sphere at a Dragon predecessor address.
        // The following segment must read that new carrier's actual yaw,
        // not the old fixed-address bytes left by its predecessor.
        let sphere_slot = arena.economy.spawn_mana_sphere_from_death(
            999, position, 57, 0x1234, 500, 0, &session.session.state.live.terrain,
        ).expect("real death constructor allocates a mana sphere") as u16;
        let sphere = arena.economy.spheres.iter_mut()
            .find(|row| row.active && row.source_entity_index == sphere_slot).unwrap();
        sphere.yaw = 753;
        arena.entities[usize::from(sphere_slot)].set_yaw(-321);
        let target = read(&arena, Context::default(), sphere_slot).unwrap().unwrap();
        assert_eq!((target.class, target.model, target.yaw), (10, 39, Some(753)));
        arena.economy.spheres.iter_mut().find(|row| row.source_entity_index == sphere_slot).unwrap().yaw = 754;
        assert_eq!(read(&arena, Context::default(), sphere_slot).unwrap().unwrap().yaw, Some(754));

        let mut sphere_stage = stage.clone();
        sphere_stage.kind = 2;
        sphere_stage.relation = super::super::RuntimeRelation::LiveEntity(sphere_slot);
        arena.entities[usize::from(sphere_slot)].font_type_index = 7;
        arena.entities[usize::from(sphere_slot)].set_life(400);
        arena
            .economy
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .unwrap()
            .life = -1;
        assert!(
            achieved_in_arena(
                Form::TargetDeathWithFontTypeZero,
                &sphere_stage,
                &arena,
                Context::default(),
            )
            .unwrap(),
            "the current sphere retains its constructor's zero font byte"
        );
        arena
            .economy
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere_slot)
            .unwrap()
            .life = 0;
        assert!(
            !achieved_in_arena(
                Form::TargetDeathWithFontTypeZero,
                &sphere_stage,
                &arena,
                Context::default(),
            )
            .unwrap(),
            "zero life is not source death"
        );

        let tree = arena
            .economy
            .allocate_authored_tree(260, position, &session.session.definition.presentation.particles)
            .expect("ordinary Tree constructor");
        let current_tree = arena
            .economy
            .authored_tree_mut(tree.source_entity_index)
            .unwrap();
        current_tree.yaw = 612;
        current_tree.roll = 345;
        arena.entities[usize::from(tree.source_entity_index)].set_yaw(999);
        arena.entities[usize::from(tree.source_entity_index)].set_source_roll(-111);
        let target = read(&arena, Context::default(), tree.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.yaw, target.roll), (Some(0), Some(0)));
        let shape = target.collision.expect("current Tree collision axes");
        assert_eq!((shape.vertical_offset, shape.half_depth), (612, 345));

        let live = &session.session.state.live;
        let house = live
            .ordinary
            .houses()
            .iter()
            .find(|row| row.source_retained())
            .expect("ordinary Level2 House");
        arena.entities[usize::from(house.entity_index)].font_type_index =
            house.parameter.destruction_successor.wrapping_add(1);
        assert_eq!(
            font_type_index(
                &arena,
                Context::from_ordinary(&live.goats, &live.ordinary),
                house.entity_index,
            )
            .unwrap(),
            Some(house.parameter.destruction_successor)
        );
        let goat = live
            .goats
            .goats()
            .iter()
            .find(|goat| goat.active)
            .expect("ordinary Level2 Goat");
        let raw = &mut arena.entities[usize::from(goat.entity_index)];
        raw.record[76] = (goat.stage_var_1 as u8).wrapping_add(1);
        raw.record[78..80].copy_from_slice(&goat.stage_target.wrapping_add(1).to_le_bytes());
        raw.set_yaw(goat.yaw.wrapping_add(1));
        raw.set_source_roll(goat.roll.wrapping_add(1));
        let target = read(
            &arena,
            Context::from_ordinary(&live.goats, &live.ordinary),
            goat.entity_index,
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.yaw, Some(goat.yaw));
        assert_eq!(target.roll, Some(goat.roll));
        assert_eq!(target.relations.stage_index, Some(goat.stage_var_1 as u8));
        assert_eq!(target.relations.stage_relation, Some(goat.stage_target));
        assert_eq!(target.relations.parent, Some(goat.parent_entity_index));

        // Castle model3 defenders are castle_children, whereas model15
        // auxiliaries use the separate castle_defenders collection.
        let owner = session.state.entity_index;
        let castle = session.economy_mut().build_castle(position, owner).unwrap();
        session.economy_mut().maintain_castle_defenders(castle);
        let defender = session
            .economy()
            .castle_children()
            .iter()
            .find(|row| row.active && (row.source_class, row.model) == (3, 3))
            .copied()
            .expect("ordinary Castle maintenance creates a model3 defender");
        let mut arena = session.source_arena().clone();
        let raw = &mut arena.entities[usize::from(defender.source_entity_index)];
        raw.active = true;
        raw.class = 9;
        raw.model = 9;
        raw.set_life(-400);
        raw.record[76] = 88;
        let context = Context {
            particles: Some(&session.session.definition.presentation.particles),
            map_type: Some(super::super::MapType::Day),
            ..Context::default()
        };
        let target = read(&arena, context, defender.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.class, target.model), (3, 3));
        assert_eq!(
            (target.source_id, target.life),
            (defender.id_entity_index, defender.life)
        );
        assert_eq!(target.position, defender.position);
        assert!(target.collision.is_some());

        assert_eq!(
            target.relations.stage_index, None,
            "an unavailable typed stage never inherits an old raw byte"
        );

        arena
            .economy
            .castle_children
            .iter_mut()
            .find(|row| row.source_entity_index == defender.source_entity_index)
            .unwrap()
            .active = false;
        let target = read(&arena, context, defender.source_entity_index)
            .unwrap()
            .unwrap();
        assert_eq!((target.class, target.model, target.life), (9, 9, -400));

        // A House birth retains its provenance after a StageVar/Alliance
        // parent changes. Only the actual source parent belongs in this view.
        let live = &mut session.session.state.live;
        let parent = live.core.state.entity_index;
        let resident_slot = live
            .economy
            .allocate_house_resident(
                10,
                0,
                crate::level1_mission::HouseResidentRequest {
                    position,
                    model: 13,
                    action: 105,
                    attacker_entity_index: 0,
                },
                &mut live.terrain,
            )
            .expect("ordinary Villager constructor");
        live.economy
            .economy
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == resident_slot)
            .unwrap()
            .stage
            .parent = parent;
        live.economy.entities[usize::from(resident_slot)].record[44..46]
            .copy_from_slice(&900_u16.to_le_bytes());
        let target = read(&live.economy, Context::default(), resident_slot)
            .unwrap()
            .unwrap();
        assert_eq!(target.relations.parent, Some(parent));
        let resident = live
            .economy
            .economy
            .house_residents
            .iter_mut()
            .find(|row| row.source_entity_index == resident_slot)
            .unwrap();
        assert_eq!(resident.parent_house_entity_index, 10);
        resident.stage.parent = 0;
        let target = read(&live.economy, Context::default(), resident_slot)
            .unwrap()
            .unwrap();
        assert_eq!(target.relations.parent, Some(0));
    }

    #[test]
    fn current_spell_carrier_uses_shared_owner_before_raw_publication() {
        let mut session = super::super::tests::level_two_campaign_session();
        let feature = *session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 156)
            .expect("retained Level2 family11 authored producer");
        assert_eq!((feature.kind, feature.subtype), (15, 11));
        session
            .activate_authored_group_prerequisite_for_test(
                u16::try_from(feature.activation_group).unwrap(),
            )
            .unwrap();
        let parent = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|row| row.authored_index == feature.index)
            .expect("real group constructor publishes family11")
            .source_entity_index;
        assert_eq!(session.shared_spells().pickup(parent).unwrap().model, 11);
        let replacement = session
            .allocate_level2_spell_replacement(11, parent, session.state.position)
            .expect("ordinary persistent spell replacement constructor");
        let slot = replacement.source_entity_index;
        let mut arena = session.source_arena().clone();
        let mut spells = session.shared_spells().clone();
        let current = spells.pickup_mut(slot).unwrap();
        current.set_position(Position {
            x: 111,
            y: 222,
            z: 333,
        });
        current.set_life(-7);
        current.set_control_flags(0x404);
        current.record[154..156].copy_from_slice(&944_u16.to_le_bytes());
        current.record[44..46].copy_from_slice(&parent.to_le_bytes());
        let expected = View::raw(current);

        // Even a plausible same-family arena copy is not the shared owner's
        // current mutable carrier before publication to the next dispatch.
        let raw = &mut arena.entities[usize::from(slot)];
        raw.class = 15;
        raw.model = 11;
        raw.set_position(Position::default());
        raw.set_life(700);
        raw.set_control_flags(8);
        let context = Context {
            spells: Some(&spells),
            ..Context::default()
        };
        assert_eq!(read(&arena, context, slot).unwrap(), Some(expected));
        assert!(read(&arena, Context::default(), slot).is_err());

        // A record for another family at the same numeric address cannot
        // satisfy this typed occupant's current-owner requirement.
        spells.pickup_mut(slot).unwrap().model = 12;
        let context = Context {
            spells: Some(&spells),
            ..Context::default()
        };
        assert!(read(&arena, context, slot).is_err());
    }

    #[test]
    fn hidden_fireball_remains_current_until_the_source_reclaim_boundary() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = arena
            .economy
            .spawn_fireball_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
                    origin: super::super::FireballProjectileOrigin::PlayerSpell,
                    position: session.state.position,
                    yaw: 21,
                    pitch: 22,
                    axis_position: Position::default(),
                    source_id_entity_index: session.state.entity_index,
                    holder_entity_index: 0,
                    source_subspell_index: 100,
                    source_mana: 50,
                    source_dword_0x10: 0,
                    target_source_entity_index: 0,
                    target_class: 0,
                    target_model: 0,
                },
                0,
            )
            .expect("ordinary Fireball constructor");
        assert!(
            !read(&arena, Context::default(), slot)
                .unwrap()
                .unwrap()
                .drawing_disabled()
        );
        let row = arena
            .economy
            .fireball_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        row.active = false;
        row.drawing_disabled = true;
        assert!(arena.economy.live_arena_cursor_row(slot).is_none());
        let target = read(&arena, Context::default(), slot).unwrap().unwrap();
        assert_eq!((target.class, target.model), (9, 0));
        assert_eq!(target.position, session.state.position);
        assert!(!target.active && target.drawing_disabled());

        let castle = arena
            .economy
            .spawn_castle_projectile_with_payload(
                super::super::level2_dynamic_economy::Level2CastleProjectileLaunchPayload {
                    position: session.state.position,
                    axis_position: session.state.position,
                    yaw: session.state.yaw,
                    pitch: 0,
                    source_id_entity_index: session.state.entity_index,
                    source_subspell_index: 0,
                    source_mana: session.shared_spells().player_mana(),
                    source_a4_byte_154: 0,
                },
                0,
                0,
                &session.session.state.live.terrain,
            )
            .expect("ordinary Castle flyer constructor");
        let possession = arena
            .economy
            .spawn_possession_projectile(
                session.state.position,
                session.state.entity_index,
                0,
                session.state.yaw,
                0,
                0,
                session.state.position,
            )
            .expect("ordinary Possession flyer constructor");
        let row = arena
            .economy
            .castle_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == castle)
            .unwrap();
        row.active = false;
        row.drawing_disabled = true;
        arena
            .economy
            .possession_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == possession)
            .unwrap()
            .active = false;
        for (slot, model) in [(castle, 10), (possession, 1)] {
            // A retained packet at the same address cannot resurrect the
            // previous occupant while its typed flyer awaits reclaim.
            let raw = &mut arena.entities[usize::from(slot)];
            raw.class = 2;
            raw.model = 7;
            raw.set_life(-900);
            let current = read(&arena, Context::default(), slot).unwrap().unwrap();
            assert_eq!((current.class, current.model), (9, model));
            assert_eq!(current.position, session.state.position);
            assert_eq!(current.source_id, session.state.entity_index);
            assert!(!current.active && current.drawing_disabled());
        }
        arena.economy.reclaim_inactive_slots_at_source_boundary();
        for slot in [castle, possession] {
            assert!(arena.economy.free_source_slots().contains(&slot));
            let retained = read(&arena, Context::default(), slot).unwrap().unwrap();
            assert_eq!(
                (retained.class, retained.model, retained.life),
                (2, 7, -900)
            );
        }
    }

    #[test]
    fn live_player_override_does_not_hide_retained_free_address_bytes() {
        let session = super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let slot = session.state.entity_index;
        let current = Position {
            x: 1000,
            y: 2000,
            z: 300,
        };
        let context = Context {
            player: Some((slot, current, 17)),
            player_orientation: Some((21, 22)),
            ..Context::default()
        };
        let target = read(&arena, context, slot).unwrap().unwrap();
        assert_eq!((target.position, target.life), (current, 17));
        assert_eq!((target.yaw, target.roll), (Some(21), Some(22)));
        let raw = &mut arena.entities[usize::from(slot)];
        let retained = (raw.position, raw.life);
        let retained_orientation = (Some(raw.yaw), Some(raw.source_roll()));
        raw.active = false;
        let target = read(&arena, context, slot).unwrap().unwrap();
        assert!(!target.active);
        assert_eq!((target.position, target.life), retained);
        assert_eq!((target.yaw, target.roll), retained_orientation);
        assert!(read(&arena, context, 0).unwrap().is_none());
        assert!(read(&arena, context, u16::MAX).unwrap().is_none());
    }

    #[test]
    fn possession_frame_uses_current_sphere_and_model57_parent_instead_of_raw_predecessor() {
        let session = super::super::tests::level_two_campaign_session();
        let live = &session.session.state.live;
        let mut arena = session.source_arena().clone();
        let owner = session.state.entity_index;
        let position = session.state.position;
        let sphere = arena
            .economy
            .spawn_mana_sphere_from_death(999, position, 0, 0x1234, 512, 0, &live.terrain)
            .expect("ordinary sphere constructor") as u16;
        let mut terrain = live.terrain.clone();
        crate::world::effects::sync_position(
            &mut arena,
            &mut terrain,
            sphere,
            position,
            true,
            false,
        )
        .unwrap();
        let projectile = arena
            .economy
            .spawn_possession_projectile(position, owner, 116, 0, 0, 0, position)
            .expect("ordinary Possession constructor");
        arena
            .economy
            .possession_projectiles
            .iter_mut()
            .find(|row| row.source_entity_index == projectile)
            .unwrap()
            .target_source_entity_index = sphere;
        // The current sphere is authoritative even when an old wizard record
        // survives at the same physical address before publication.
        let raw = &mut arena.entities[usize::from(sphere)];
        raw.class = 3;
        raw.model = 1;
        raw.source_id = owner;
        raw.set_position(Position { x: 1, y: 2, z: 3 });
        let context = Context::from_ordinary(&live.goats, &live.ordinary);
        let frame =
            possession_collision_frame(&arena, context, projectile, &terrain, None).unwrap();
        let target = frame.targets.iter().find(|row| row.slot == sphere).unwrap();
        assert_eq!(target.model, 39);
        assert_eq!(target.source_id, sphere);
        assert_eq!(target.position, position);
        assert_eq!(frame.guidance_source_id, Some(sphere));
        assert_eq!(frame.guidance.unwrap().x, position.x);
        assert!(!frame.targets.iter().any(|row| row.slot == owner));

        let decoy = arena
            .allocate_source_slot()
            .expect("ordinary free source address");
        let row = &mut arena.entities[usize::from(decoy)];
        row.active = true;
        row.class = 10;
        row.model = 57;
        row.set_control_flags(8);
        row.set_position(position);
        row.source_id = owner;
        row.record[152..154].copy_from_slice(&owner.to_le_bytes());
        row.record[44..46].copy_from_slice(&owner.to_le_bytes());
        crate::world::effects::sync_position(
            &mut arena,
            &mut terrain,
            decoy,
            position,
            true,
            false,
        )
        .unwrap();
        let frame =
            possession_collision_frame(&arena, context, projectile, &terrain, None).unwrap();
        assert!(
            !frame.targets.iter().any(|row| row.slot == decoy),
            "own Fool's Mana is excluded by parent"
        );
        arena.entities[usize::from(decoy)].record[44..46].copy_from_slice(&999_u16.to_le_bytes());
        let frame =
            possession_collision_frame(&arena, context, projectile, &terrain, None).unwrap();
        assert!(
            frame.targets.iter().any(|row| row.slot == decoy),
            "foreign Fool's Mana ignores ordinary id/player exclusions"
        );
    }

    #[test]
    fn possession_launch_reads_current_bee_pose_and_shape_over_old_raw_bytes() {
        let mut session = super::super::tests::level_two_campaign_session();
        let holder_slot = session.shared_spells().local_player.enabled_spell_holders[1];
        let origin = session.state.position;
        let (bee_slot, position, height) = {
            let live = &mut session.session.state.live;
            let bee = live
                .economy
                .economy
                .allocate_authored_bee(1, origin, None, 3_000, &live.terrain)
                .expect("ordinary Bee constructor");
            let slot = bee.source_entity_index;
            live.economy.retire_previous_source_projection(slot);
            let current = live
                .economy
                .economy
                .bee_creatures
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            current.position.z = current.position.z.wrapping_add(2_000);
            current.yaw = 128;
            current.pitch = 64;
            let position = current.position;
            let shape = read(&live.economy, Context::default(), slot)
                .unwrap()
                .unwrap()
                .collision
                .expect("current Bee collision shape");
            let raw = &mut live.economy.entities[usize::from(slot)];
            raw.set_yaw(999);
            raw.set_pitch(-999);
            raw.record[92..94].copy_from_slice(&1_i16.to_le_bytes());
            live.shared_spells.global_special_state.input_yaw = 0;
            live.shared_spells.global_special_state.input_pitch = 0;
            let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
            holder.record[44..46].copy_from_slice(&slot.to_le_bytes());
            holder.active_value = holder.activation_value.cast_signed();
            (slot, position, shape.half_height)
        };
        let born = session
            .step_level2_possession_holder_source_slot(holder_slot)
            .expect("real holder launches from current typed Bee parent");
        let flyer = session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|row| row.source_entity_index == born)
            .unwrap();
        assert_eq!(flyer.source_id_entity_index, bee_slot);
        assert_eq!((flyer.yaw, flyer.pitch), (128, 64));
        assert_eq!(
            flyer.position,
            Position {
                z: position.z.wrapping_add(height),
                ..position
            }
        );
    }

    #[test]
    fn fireball_launch_reuses_current_parent_or_holder_records_after_new_event() {
        use crate::level1_mission::SOURCE_REPLACEABLE_FLAG;
        for replace_holder in [false, true] {
            let mut session = super::super::tests::level_two_campaign_session();
            let player = session.state.entity_index;
            let holder_slot = session.shared_spells().local_player.enabled_spell_holders[0];
            let victim = if replace_holder { holder_slot } else { player };
            let old_position = Position {
                x: 40_000,
                y: 41_000,
                z: 2_000,
            };
            let cost = {
                let live = &mut session.session.state.live;
                live.economy.economy.exhaust_new_event_capacity_for_test();
                for row in &mut live.economy.entities {
                    row.set_control_flags(row.control_flags & !SOURCE_REPLACEABLE_FLAG);
                }
                let parent = &mut live.economy.entities[usize::from(player)];
                parent.set_position(old_position);
                parent.set_actual_speed(71);
                parent.set_mana(10_000);
                parent.set_mana_regen(17);
                parent.set_life(1_000);
                let holder = live.shared_spells.pickup_mut(holder_slot).unwrap();
                holder.active_value = holder.activation_value.cast_signed();
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                let cost = holder.max_mana;
                if replace_holder {
                    holder.set_control_flags(holder.control_flags | SOURCE_REPLACEABLE_FLAG);
                }
                live.economy.entities[usize::from(holder_slot)] = holder.clone();
                let raw = &mut live.economy.entities[usize::from(victim)];
                raw.set_control_flags(raw.control_flags | SOURCE_REPLACEABLE_FLAG);
                live.economy.rebuild_source_free_arena();
                live.economy.economy.free_slots.clear();
                cost
            };
            let born = session
                .step_level2_fireball_holder_source_slot(holder_slot)
                .unwrap();
            assert_eq!(born, victim);
            let flyer = session
                .economy()
                .fireball_projectiles()
                .iter()
                .find(|row| row.source_entity_index == born)
                .unwrap();
            assert_eq!(
                flyer.actual_speed,
                384 + 71,
                "speed argument is evaluated before NewEvent"
            );
            if replace_holder {
                assert_eq!((flyer.source_subspell_index, flyer.source_mana), (100, 50));
                assert_eq!(
                    session.entities()[usize::from(holder_slot)].active_value,
                    -1
                );
                assert_eq!(session.entities()[usize::from(player)].mana_regen, 0);
            } else {
                assert_eq!(
                    flyer.position,
                    Position {
                        z: 50,
                        ..Position::default()
                    }
                );
                assert_ne!(flyer.position, old_position);
                assert_eq!(session.entities()[usize::from(player)].mana_regen, -cost);
            }
        }
    }

    #[test]
    fn acquired_fireball_tiers_reach_real_damage_death_and_natural_descendant_cleanup() {
        use crate::native_world::simulation::CarriedSpellProgress;
        use crate::{FlightWorld as _, PlayerCommand};

        for (tier, configured) in [(0_u8, false), (1, false), (2, false), (0, true)] {
            let mut session = super::super::tests::level_two_campaign_session();
            if configured {
                let original = session.clone();
                let before = session.session.definition.identity_receipt;
                session.apply_fireball_projectile_speed(512);
                session.session.rebase();
                assert_ne!(session.session.definition.identity_receipt, before);
                assert_eq!(original.session.definition.identity_receipt, before);
                assert_eq!(
                    original
                        .session
                        .definition
                        .fireball_properties
                        .projectile_speed,
                    384
                );
            }
            session.select_level2_campaign_entry_stage();
            let player = session.state.entity_index;
            let holder = session.shared_spells().local_player.enabled_spell_holders[0];
            assert_ne!(holder, 0, "ordinary startup acquires Fireball");
            assert_eq!(
                session
                    .shared_spells()
                    .pickup(holder)
                    .unwrap()
                    .source_spell_parent_entity_index(),
                player
            );
            // Declared objective/progression/resources/positions select this
            // scenario. The actual authored producer, acquired holder, public
            // cast, flight, impact, damage/death and cleanup run normally.
            let producer = session
                .entities()
                .iter()
                .find(|row| row.active && row.class == 11 && row.model == 32 && row.source_id == 21)
                .unwrap();
            let stage = usize::try_from(producer.level_gate).unwrap();
            for objective in 0..session.runtime.objectives.len() {
                session
                    .runtime
                    .set_level2_objective_stage_selector_for(objective, stage, 2);
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().any(|event| matches!(
                event,
                super::super::Mc2DispatchEvent::AuthoredGroupActivated { group: 21, .. }
            )));
            let bee_slot = session
                .economy()
                .bee_creatures()
                .iter()
                .find(|row| row.authored_index().is_some())
                .unwrap()
                .source_entity_index;
            for _ in 0..160 {
                let bee = *session
                    .economy()
                    .bee_creatures()
                    .iter()
                    .find(|row| row.source_entity_index == bee_slot)
                    .unwrap();
                if bee.action == 17 {
                    break;
                }
                let mut position = bee.position;
                if bee.action == 23 {
                    crate::move_entity(&mut position, bee.yaw, 0, 1_000);
                } else {
                    position.x = position.x.wrapping_add(20_000);
                    position.y = position.y.wrapping_add(20_000);
                }
                session
                    .set_level2_entity_position(player, position)
                    .unwrap();
                session.step_outer_turn(PlayerCommand::default(), 0);
            }
            let bee = *session
                .economy()
                .bee_creatures()
                .iter()
                .find(|row| row.source_entity_index == bee_slot)
                .unwrap();
            assert_eq!(bee.action, 17);
            let initial_life = bee.life;
            let kill_count = session.economy().authored_bee_kill_census(player);
            let castle = session
                .economy_mut()
                .build_castle(
                    Position {
                        x: bee.position.x.wrapping_add(0x4000),
                        y: bee.position.y.wrapping_add(0x4000),
                        ..bee.position
                    },
                    player,
                )
                .unwrap();
            assert!(
                session
                    .economy_mut()
                    .set_castle_mana_prerequisite_for_test(castle, 10_000)
            );
            {
                let live = &mut session.session.state.live;
                live.economy.refresh_owner_mana_rows();
                let parent = &mut live.economy.entities[usize::from(player)];
                parent.set_mana(15_000);
                parent.set_mana_regen(0);
                let snapshot = parent.clone();
                live.shared_spells
                    .sync_player_entity_from_authority(&snapshot);
                live.shared_spells.set_local_player_mana_for_test(15_000);
                live.shared_spells.local_player.record[42..44]
                    .copy_from_slice(&castle.to_le_bytes());
                for state in live
                    .shared_spells
                    .players
                    .iter_mut()
                    .chain(live.core.players.iter_mut())
                    .filter(|state| state.entity_index == player)
                {
                    state.record[42..44].copy_from_slice(&castle.to_le_bytes());
                }
            }
            session
                .apply_carried_spell_progression(&[CarriedSpellProgress {
                    family: 0,
                    persistent_experience: super::super::LEVEL2_SPELL_TIER_THRESHOLDS[0]
                        [usize::from(tier)],
                    runtime_experience: 0,
                    unlocked_tier: tier as i8,
                    selected_tier: tier as i8,
                }])
                .unwrap();
            assert_eq!(
                session.shared_spells().pickup(holder).unwrap().level_gate,
                tier as i8
            );
            let baseline = session
                .entities()
                .iter()
                .map(|row| (row.index, session.economy().slot_generation(row.index)))
                .collect::<std::collections::BTreeMap<_, _>>();
            let mut descendants = std::collections::BTreeMap::new();
            let mut speeds = std::collections::BTreeMap::new();
            let mut saw_flyer = false;
            let mut saw_impact = false;
            let mut saw_damage = false;
            let mut saw_sectors = false;
            let mut saw_tail = false;
            let mut terminal_witness = None;
            let mut saw_presentation = false;
            let mut casts = 0;
            for turn in 0..320 {
                assert!(
                    !session
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                    "tier{tier}: {:?}",
                    session.dispatch_events()
                );
                let current = session
                    .economy()
                    .bee_creatures()
                    .iter()
                    .find(|row| row.source_entity_index == bee_slot)
                    .copied();
                saw_damage |= current.is_some_and(|row| {
                    row.life < initial_life && row.damage_attacker_entity_index == player
                });
                let killed = session.economy().authored_bee_kill_census(player) > kill_count;
                let alive_descendants = descendants.iter().any(|(&slot, &generation)| {
                    session.economy().slot_generation(slot) == generation
                        && session.source_arena().handle(slot).is_some()
                });
                if saw_flyer
                    && saw_impact
                    && saw_damage
                    && !alive_descendants
                    && session.shared_spells().pickup(holder).unwrap().active_value == 0
                    && (tier < 2 || (killed && saw_tail))
                {
                    break;
                }
                let should_cast = casts == 0
                    || (tier == 2
                        && !killed
                        && !alive_descendants
                        && session.shared_spells().pickup(holder).unwrap().active_value == 0
                        && casts < 4);
                let command = if should_cast {
                    let target = current.expect("live target until source death").position;
                    let position = Position {
                        x: target.x.wrapping_add(256),
                        y: target.y.wrapping_add(1_024),
                        z: target.z.wrapping_add(512),
                    };
                    session
                        .set_level2_entity_position(player, position)
                        .unwrap();
                    session.state.yaw = 0;
                    session.state.display_pitch = 0;
                    session.state.effective_pitch = 0;
                    session.state.actual_speed = 0;
                    session.state.target_speed = 0;
                    let launch = Position {
                        x: position.x.wrapping_sub(256),
                        z: position
                            .z
                            .wrapping_add(session.entities()[usize::from(player)].array_fov()),
                        ..position
                    };
                    let (_, pitch) =
                        crate::level1_mission::projectile_guidance_angles(launch, target);
                    casts += 1;
                    PlayerCommand {
                        opcode: 6,
                        argument_1: 0,
                        flags: 0x40,
                        argument_8: pitch.cast_unsigned(),
                        ..PlayerCommand::default()
                    }
                } else {
                    PlayerCommand::default()
                };
                let roots_before = session
                    .economy()
                    .authored_model76()
                    .iter()
                    .filter(|root| {
                        root.entity_id == player
                            && root.active
                            && root.drawing_enabled
                            && root.control_flags & 0x400 == 0
                    })
                    .map(|root| (root.source_entity_index, root.position))
                    .collect::<Vec<_>>();
                let generations_before = if roots_before.is_empty() {
                    Vec::new()
                } else {
                    session
                        .entities()
                        .iter()
                        .map(|row| session.economy().slot_generation(row.index))
                        .collect::<Vec<_>>()
                };
                session.step_outer_turn(command, 0);
                for flyer in session
                    .economy()
                    .fireball_projectiles()
                    .iter()
                    .filter(|row| {
                        row.owner_entity_index == player && row.holder_entity_index == holder
                    })
                {
                    saw_flyer = true;
                    assert_eq!(
                        (flyer.source_model, flyer.action),
                        if tier == 2 { (28, 29) } else { (0, 0) }
                    );
                    assert_eq!(flyer.minimum_speed, if configured { 512 } else { 384 });
                    assert_eq!(
                        *speeds
                            .entry(flyer.source_entity_index)
                            .or_insert(flyer.actual_speed),
                        flyer.actual_speed,
                        "sub65C20 retains inherited speed"
                    );
                    descendants.insert(
                        flyer.source_entity_index,
                        session.economy().slot_generation(flyer.source_entity_index),
                    );
                    if flyer.active {
                        let view = session
                            .campaign_entity_presentations(0)
                            .unwrap()
                            .into_iter()
                            .find(|view| view.entity_index == flyer.source_entity_index)
                            .unwrap();
                        assert_eq!((view.source_id, view.render_parameter_index), (player, 42));
                        saw_presentation = true;
                    }
                }
                for row in session.entities().iter().filter(|row| {
                    row.active
                        && row.class == 10
                        && row.model == 0
                        && row.source_id == player
                        && baseline.get(&row.index).copied().unwrap_or_default()
                            != session.economy().slot_generation(row.index)
                }) {
                    saw_impact = true;
                    descendants.insert(row.index, session.economy().slot_generation(row.index));
                }
                for root in session
                    .economy()
                    .authored_model76()
                    .iter()
                    .filter(|row| row.entity_id == player)
                {
                    saw_impact = true;
                    assert_eq!((root.model, root.action, root.max_life), (76, 83, 30));
                    descendants.insert(
                        root.source_entity_index,
                        session.economy().slot_generation(root.source_entity_index),
                    );
                    let children = session
                        .economy()
                        .model76_children()
                        .iter()
                        .filter(|child| child.parent_entity_index == root.source_entity_index)
                        .collect::<Vec<_>>();
                    if children.len() == 25 {
                        saw_sectors = true;
                    }
                    for child in children {
                        assert_eq!(
                            (child.model, child.action, child.visual_index),
                            (77, 84, 42)
                        );
                        descendants.insert(
                            child.source_entity_index,
                            session.economy().slot_generation(child.source_entity_index),
                        );
                    }
                }
                // The shrinking root emits a NewEvent-owned model0 tail,
                // whose self-ID is deliberately distinct from the caster.
                for (root_slot, position) in roots_before {
                    let after = session
                        .economy()
                        .authored_model76()
                        .iter()
                        .find(|root| root.source_entity_index == root_slot);
                    if after.is_none_or(|root| {
                        !root.active || !root.drawing_enabled || root.control_flags & 0x400 != 0
                    }) {
                        let position = after.map_or(position, |root| root.position);
                        terminal_witness = Some((
                            root_slot,
                            position,
                            session
                                .entities()
                                .iter()
                                .filter(|row| {
                                    row.active
                                        && row.class == 10
                                        && row.model == 0
                                        && session.economy().slot_generation(row.index)
                                            != generations_before[usize::from(row.index)]
                                })
                                .map(|row| {
                                    (
                                        row.index,
                                        row.source_id,
                                        row.position,
                                        session.economy().slot_generation(row.index),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ));
                        for tail in session.entities().iter().filter(|row| {
                            row.active
                                && row.class == 10
                                && row.model == 0
                                && row.source_id == row.index
                                && row.position.x == position.x
                                && row.position.y == position.y
                                && session.economy().slot_generation(row.index)
                                    != generations_before[usize::from(row.index)]
                        }) {
                            let visited = tail.index > root_slot;
                            assert_eq!(
                                (tail.life, tail.phase),
                                (
                                    if visited { 7 } else { 8 },
                                    (tail.index as u8).wrapping_add(u8::from(visited))
                                ),
                                "the fresh tail runs only after the root's current numeric cursor"
                            );
                            let mut expected_z = position.z;
                            if visited {
                                let ground = session.canonical_terrain().ground_height(position);
                                if expected_z > ground {
                                    expected_z = expected_z.wrapping_add(tail.source_word_2c());
                                }
                                if expected_z <= ground {
                                    expected_z = ground;
                                }
                                if let Some(ceiling) =
                                    session.canonical_terrain().ceiling_height(position)
                                {
                                    let limit = ceiling.wrapping_sub(tail.array_fov());
                                    if expected_z > limit {
                                        expected_z = limit;
                                    }
                                }
                            }
                            assert_eq!(
                                tail.position.z, expected_z,
                                "the actual first sub30D50 visit owns vertical settling"
                            );
                            saw_tail = true;
                            descendants
                                .insert(tail.index, session.economy().slot_generation(tail.index));
                        }
                    }
                }
                if turn == 319 {
                    let live = &session.session.state.live;
                    let context = Context {
                        spells: Some(&live.shared_spells),
                        particles: Some(&session.session.definition.presentation.particles),
                        ..Context::from_ordinary(&live.goats, &live.ordinary)
                    };
                    let survivors = descendants
                        .iter()
                        .filter(|(slot, generation)| {
                            live.economy.economy.slot_generation(**slot) == **generation
                                && live.economy.handle(**slot).is_some()
                        })
                        .map(|(&slot, &generation)| {
                            (
                                slot,
                                generation,
                                live.economy.economy.live_arena_kind(slot),
                                read(&live.economy, context, slot).map(|row| {
                                    row.map(|row| {
                                        (
                                            row.class,
                                            row.model,
                                            row.control_flags,
                                            row.life,
                                            row.active,
                                            row.position,
                                        )
                                    })
                                }),
                            )
                        })
                        .collect::<Vec<_>>();
                    panic!(
                        "tier{tier}: casts={casts}, flyer={saw_flyer}, impact={saw_impact}, damage={saw_damage}, sectors={saw_sectors}, tail={saw_tail}, killed={}/{kill_count}, holder_active={}, survivors={survivors:?}, terminal={terminal_witness:?}, bee={current:?}",
                        session.economy().authored_bee_kill_census(player),
                        session.shared_spells().pickup(holder).unwrap().active_value
                    );
                }
            }
            assert!(saw_flyer && saw_impact && saw_damage && saw_presentation);
            assert_eq!(saw_sectors, tier == 2);
            assert_eq!(
                saw_tail,
                tier == 2,
                "the actual root terminal tail is observed and reclaimed"
            );
            if tier == 2 {
                assert!(
                    session.economy().authored_bee_kill_census(player) > kill_count,
                    "tiered damage reaches actual attributed phase-gated death"
                );
            }
            assert!(
                descendants.iter().all(|(&slot, &generation)| session
                    .economy()
                    .slot_generation(slot)
                    != generation
                    || session.source_arena().handle(slot).is_none()),
                "natural following-pass reclaim"
            );
        }
    }

    #[test]
    fn acquired_possession_all_tiers_reach_real_ownership_and_auxiliary_consumers() {
        use crate::native_world::simulation::CarriedSpellProgress;
        use crate::{FlightWorld as _, PlayerCommand};

        for (tier, configured) in [(0_u8, false), (1, false), (2, false), (0, true)] {
            let mut session = super::super::tests::level_two_campaign_session();
            if configured {
                let original = session.clone();
                let before = session.session.definition.identity_receipt;
                session.apply_possession_projectile_speed(512);
                session.session.rebase();
                assert_ne!(session.session.definition.identity_receipt, before);
                assert_eq!(original.session.definition.identity_receipt, before);
                assert_eq!(
                    original
                        .session
                        .definition
                        .possession_properties
                        .projectile_speed,
                    384
                );
            }
            session.select_level2_campaign_entry_stage();
            let player = session.state.entity_index;
            let holder = session.shared_spells().local_player.enabled_spell_holders[1];
            assert_ne!(holder, 0, "ordinary startup acquires Possession");
            assert_eq!(
                session
                    .shared_spells()
                    .pickup(holder)
                    .unwrap()
                    .source_spell_parent_entity_index(),
                player
            );
            // These are explicit progression/resource/position controls. The
            // held source, public request, constructor, map search, contact,
            // House mailbox, auxiliary and cleanup all run normally.
            session
                .apply_carried_spell_progression(&[CarriedSpellProgress {
                    family: 1,
                    persistent_experience: super::super::LEVEL2_SPELL_TIER_THRESHOLDS[1]
                        [usize::from(tier)],
                    runtime_experience: 0,
                    unlocked_tier: tier as i8,
                    selected_tier: tier as i8,
                }])
                .unwrap();
            let switch = session
                .mission()
                .authored_features()
                .iter()
                .find(|feature| feature.index == 11)
                .unwrap()
                .world_position(256);
            session.set_level2_entity_position(player, switch).unwrap();
            let mut house = None;
            for _ in 0..16 {
                session.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    !session
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                    "{:?}",
                    session.dispatch_events()
                );
                house = session
                    .ordinary()
                    .houses()
                    .iter()
                    .filter(|house| {
                        house.active
                            && house.action == crate::house::HOUSE_LIVE_ACTION
                            && !house.parameter.possession_blocked()
                            && house.owner_entity_index == 0
                    })
                    .min_by_key(|house| {
                        session
                            .economy()
                            .spheres()
                            .iter()
                            .filter(|sphere| sphere.active && sphere.player_entity_index == 0)
                            .map(|sphere| {
                                super::super::level2_wrapping_xy_distance(
                                    house.position,
                                    sphere.position,
                                )
                            })
                            .min()
                            .unwrap_or(u32::MAX)
                    })
                    .copied();
                if house.is_some() {
                    break;
                }
            }
            let house = house.expect("actual authored group produces a neutral live House");
            let neutral_spheres = session
                .economy()
                .spheres()
                .iter()
                .filter(|sphere| sphere.active && sphere.player_entity_index == 0)
                .map(|sphere| sphere.id)
                .collect::<std::collections::BTreeSet<_>>();
            let sphere_target = (tier > 0).then(|| {
                session
                    .economy()
                    .spheres()
                    .iter()
                    .filter(|sphere| sphere.active && neutral_spheres.contains(&sphere.id))
                    .min_by_key(|sphere| {
                        (
                            sphere.id != 141,
                            i32::from(sphere.velocity_x).abs() + i32::from(sphere.velocity_y).abs(),
                            sphere.id,
                        )
                    })
                    .copied()
                    .expect("real authored neutral mana target")
            });
            let target_position = sphere_target.map_or(house.position, |sphere| sphere.position);
            let target_offset = sphere_target.map_or(house.collision_vertical_offset, |sphere| {
                sphere.collision_yaw
            });
            let origin = Position {
                y: target_position.y.wrapping_add(1_024),
                z: target_position.z.wrapping_add(256),
                ..target_position
            };
            let castle_position = Position {
                x: origin.x.wrapping_add(0x4000),
                y: origin.y.wrapping_add(0x4000),
                ..origin
            };
            let castle = session
                .economy_mut()
                .build_castle(castle_position, player)
                .unwrap();
            assert!(
                session
                    .economy_mut()
                    .set_castle_mana_prerequisite_for_test(castle, 2_000)
            );
            {
                let live = &mut session.session.state.live;
                live.economy.refresh_owner_mana_rows();
                let raw = &mut live.economy.entities[usize::from(player)];
                raw.set_mana(2_000);
                raw.set_mana_regen(0);
                let snapshot = raw.clone();
                live.shared_spells
                    .sync_player_entity_from_authority(&snapshot);
                for state in live
                    .shared_spells
                    .players
                    .iter_mut()
                    .chain(live.core.players.iter_mut())
                    .filter(|state| state.entity_index == player)
                {
                    state.record[42..44].copy_from_slice(&castle.to_le_bytes());
                }
            }
            session.set_level2_entity_position(player, origin).unwrap();
            let target_top = Position {
                z: target_position.z.wrapping_add(target_offset),
                ..target_position
            };
            let launch = Position {
                x: origin.x.wrapping_sub(256),
                z: origin
                    .z
                    .wrapping_add(session.entities()[usize::from(player)].array_fov()),
                ..origin
            };
            let (yaw, pitch) =
                crate::level1_mission::projectile_guidance_angles(launch, target_top);
            session.set_level2_player_aim_for_test(yaw, pitch);
            let mut saw_flyer = false;
            let mut saw_flash = false;
            let mut saw_auxiliary = false;
            let mut saw_ownership_callback = false;
            let mut saw_presentation = false;
            let mut saw_attraction = false;
            let mut captured_slots = std::collections::BTreeMap::new();
            session.step_outer_turn(
                PlayerCommand {
                    opcode: 6,
                    argument_1: 1,
                    flags: 0x40,
                    ..PlayerCommand::default()
                },
                0,
            );
            // The 128-life auxiliary gets its final producer visit at zero,
            // hides on the next visit and reclaims at the following boundary.
            let cleanup_bound = 128
                + usize::try_from(
                    session
                        .session
                        .definition
                        .possession_properties
                        .projectile_lifetime,
                )
                .unwrap()
                + 3;
            for _ in 0..cleanup_bound {
                assert!(
                    !session
                        .dispatch_events()
                        .iter()
                        .any(|event| matches!(event, super::super::Mc2DispatchEvent::Error { .. })),
                    "tier {tier}: {:?}",
                    session.dispatch_events()
                );
                for row in session
                    .economy()
                    .possession_projectiles()
                    .iter()
                    .filter(|row| row.source_id_entity_index == player)
                {
                    saw_flyer = true;
                    captured_slots.insert(
                        row.source_entity_index,
                        session.economy().slot_generation(row.source_entity_index),
                    );
                    assert_eq!(
                        (row.source_class, row.source_model, row.action),
                        if tier == 0 { (9, 1, 1) } else { (9, 17, 18) }
                    );
                    assert_eq!(row.minimum_speed, if configured { 512 } else { 384 });
                    if row.active {
                        let presentation = session
                            .campaign_entity_presentations(0)
                            .unwrap()
                            .into_iter()
                            .find(|view| view.entity_index == row.source_entity_index)
                            .expect("the real flyer reaches its presentation binding");
                        assert_eq!(
                            (presentation.source_id, presentation.render_parameter_index),
                            (player, 209)
                        );
                        saw_presentation = true;
                    }
                }
                for row in session
                    .economy()
                    .possession_effects()
                    .iter()
                    .filter(|row| row.source_id_entity_index == player)
                {
                    saw_flash = true;
                    captured_slots.insert(
                        row.source_entity_index,
                        session.economy().slot_generation(row.source_entity_index),
                    );
                    assert_eq!(
                        (row.source_model, row.action),
                        if tier == 2 { (70, 77) } else { (12, 12) }
                    );
                }
                for row in session.economy().mana_attraction_fields() {
                    if session.economy().source_owner(row.source_entity_index) == player {
                        saw_auxiliary = true;
                        captured_slots.insert(
                            row.source_entity_index,
                            session.economy().slot_generation(row.source_entity_index),
                        );
                        assert_eq!((row.action, row.max_life), (59, 128));
                        if !saw_attraction
                            && let Some(sphere) =
                                session.economy().spheres().iter().find(|sphere| {
                                    sphere.active
                                        && sphere.attraction_source_entity_index
                                            == row.source_entity_index
                                        && super::super::level2_wrapping_xy_distance(
                                            row.position,
                                            sphere.position,
                                        ) < row.radius_squared
                                        && super::super::level2_wrapping_xy_distance(
                                            row.position,
                                            sphere.position,
                                        ) != 0
                                })
                        {
                            // The source consumes and clears this mailbox in
                            // the same pass. Compare real scheduled movement
                            // against an otherwise identical zero-radius
                            // control. An earlier authored field can win the
                            // mailbox, so select this actual field's recipient;
                            // no target, projectile or outcome is injected.
                            let sphere_slot = sphere.source_entity_index;
                            let generation = session.economy().slot_generation(sphere_slot);
                            let mut actual = session.clone();
                            let mut control = session.clone();
                            // Both snapshots can inherit this producer's
                            // already queued move. Clear it in both probe
                            // initial states so the next ordinary field visit
                            // is the sole source of the measured displacement.
                            for branch in [&mut actual, &mut control] {
                                for sphere in &mut branch.session.state.live.economy.spheres {
                                    if sphere.attraction_source_entity_index
                                        == row.source_entity_index
                                    {
                                        sphere.attraction_source_entity_index = 0;
                                        sphere.attraction_step = 0;
                                        sphere.attraction_distance_high = 0;
                                    }
                                }
                            }
                            control
                                .session
                                .state
                                .live
                                .economy
                                .mana_attraction_fields
                                .iter_mut()
                                .find(|field| field.source_entity_index == row.source_entity_index)
                                .unwrap()
                                .radius_squared = 0;
                            for _ in 0..2 {
                                actual.step_outer_turn(PlayerCommand::default(), 0);
                                control.step_outer_turn(PlayerCommand::default(), 0);
                                for branch in [&actual, &control] {
                                    assert!(
                                        !branch.dispatch_events().iter().any(|event| matches!(
                                            event,
                                            super::super::Mc2DispatchEvent::Error { .. }
                                        )),
                                        "{:?}",
                                        branch.dispatch_events()
                                    );
                                }
                                let a = actual.economy().spheres().iter().find(|sphere| {
                                    sphere.active && sphere.source_entity_index == sphere_slot
                                });
                                let b = control.economy().spheres().iter().find(|sphere| {
                                    sphere.active && sphere.source_entity_index == sphere_slot
                                });
                                if actual.economy().slot_generation(sphere_slot) == generation
                                    && control.economy().slot_generation(sphere_slot) == generation
                                    && a.zip(b).is_some_and(|(a, b)| {
                                        (a.position.x, a.position.y, a.yaw)
                                            != (b.position.x, b.position.y, b.yaw)
                                    })
                                {
                                    saw_attraction = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                saw_ownership_callback |= session.dispatch_events().iter().any(|event| match event {
                    super::super::Mc2DispatchEvent::House { events, .. } => events.iter().any(|event| matches!(event,
                        crate::house::HouseStepEvent::Possessed { entity_index, owner_entity_index, .. }
                            if tier == 0 && *entity_index == house.entity_index && *owner_entity_index == player)),
                    super::super::Mc2DispatchEvent::Economy { event: super::super::Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }, .. } =>
                        tier > 0 && *owner_entity_index == player && neutral_spheres.contains(sphere_id),
                    _ => false,
                });
                if saw_ownership_callback
                    && session.shared_spells().pickup(holder).unwrap().active_value == 0
                    && captured_slots.iter().all(|(&slot, &generation)| {
                        session.economy().slot_generation(slot) != generation
                            || !matches!(
                                session.economy().live_arena_kind(slot),
                                Some(
                                    Kind::PossessionProjectile
                                        | Kind::PossessionEffect
                                        | Kind::ManaAttraction
                                )
                            )
                    })
                {
                    break;
                }
                session.step_outer_turn(PlayerCommand::default(), 0);
            }
            let owned = session
                .ordinary()
                .houses()
                .iter()
                .find(|row| row.entity_index == house.entity_index)
                .unwrap();
            if tier == 0 {
                assert_eq!(
                    owned.owner_entity_index,
                    player,
                    "tier{tier}, configured={configured}, flyer={saw_flyer}, flash={saw_flash}, auxiliary={saw_auxiliary}, house={owned:?}, dispatch={:?}",
                    session.dispatch_events()
                );
            } else {
                assert!(
                    session
                        .economy()
                        .spheres()
                        .iter()
                        .any(|sphere| sphere.active && sphere.player_entity_index == player),
                    "the actual claimed mana has a live player-owned consumer"
                );
            }
            assert!(
                saw_flyer && saw_flash && saw_ownership_callback && saw_presentation,
                "tier {tier}: flyer={saw_flyer}, flash={saw_flash}, ownership={saw_ownership_callback}, presentation={saw_presentation}"
            );
            assert_eq!(saw_auxiliary, tier > 0);
            assert_eq!(
                saw_attraction,
                tier > 0,
                "tier {tier}: higher-tier auxiliary reaches actual authored mana movement"
            );
            assert_eq!(
                session.shared_spells().pickup(holder).unwrap().active_value,
                0
            );
            assert!(
                captured_slots.into_iter().all(|(slot, generation)| session
                    .economy()
                    .slot_generation(slot)
                    != generation
                    || !matches!(
                        session.economy().live_arena_kind(slot),
                        Some(
                            Kind::PossessionProjectile
                                | Kind::PossessionEffect
                                | Kind::ManaAttraction
                        )
                    )),
                "tier {tier} leaves no current generation of its flyer, flash or auxiliary allocated"
            );
        }
    }

    #[test]
    fn marker_transfer_updates_current_word94_and_retained_rows_without_changing_ids() {
        let mut session = super::super::tests::level_two_campaign_session();
        let position = session.state.position;
        let owner = session.state.entity_index;
        let live = &mut session.session.state.live;
        let from = 999;
        let to = 998;
        let sphere = live
            .economy
            .spawn_mana_sphere_from_death(991, position, 0, 0x1234, 512, owner, &live.terrain)
            .unwrap() as u16;
        let castle = live
            .economy
            .build_castle(position, owner, &live.terrain)
            .unwrap();
        live.economy
            .spheres
            .iter_mut()
            .find(|row| row.source_entity_index == sphere)
            .unwrap()
            .player_entity_index = from;
        live.economy
            .castle_at_mut(castle)
            .unwrap()
            .player_entity_index = from;
        // Old raw projections must neither prevent the typed transfer nor
        // replace the current source's separate ID and semantic owner.
        live.economy.entities[usize::from(sphere)].set_source_player_entity_index(123);
        live.economy.entities[usize::from(castle)].set_source_player_entity_index(123);
        let inactive = live.economy.allocate_source_slot().unwrap();
        live.economy.entities[usize::from(inactive)].active = false;
        live.economy.entities[usize::from(inactive)].set_source_player_entity_index(from);
        let castle_before = *live.economy.castle_at(castle).unwrap();
        assert!(
            transfer_possession_player_relations(
                &mut live.economy,
                &mut live.ordinary.houses,
                &mut live.ordinary.villagers,
                from,
                to,
            )
            .unwrap()
                >= 3
        );
        let sphere_after = live
            .economy
            .spheres
            .iter()
            .find(|row| row.source_entity_index == sphere)
            .unwrap();
        assert_eq!(
            (
                sphere_after.player_entity_index,
                sphere_after.owner_entity_index
            ),
            (to, owner)
        );
        let castle_after = live.economy.castle_at(castle).unwrap();
        assert_eq!(castle_after.player_entity_index, to);
        assert_eq!(
            (
                castle_after.source_id_entity_index,
                castle_after.owner_entity_index
            ),
            (
                castle_before.source_id_entity_index,
                castle_before.owner_entity_index
            )
        );
        for slot in [sphere, castle, inactive] {
            assert_eq!(
                live.economy.entities[usize::from(slot)].source_player_entity_index(),
                to
            );
        }
        assert!(!live.economy.entities[usize::from(inactive)].active);
    }
}
