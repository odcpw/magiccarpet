//! Current typed economy assets, scoped to their real arena lifetimes.

use super::Mc2CampaignBackend;
use crate::Position;
use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::game_session::{
    ActorLifecycle, CastleStatus, EconomyAsset, EconomyObservation, EconomyPosition, SessionGuard,
};
use crate::native_world::{Level2CastleChild, Level2DynamicArenaKind as Kind, source_target};

pub(super) fn observe(
    backend: &Mc2CampaignBackend,
    guard: SessionGuard,
    offset: usize,
    count: usize,
) -> Vec<EconomyObservation> {
    let simulation = backend.campaign.simulation();
    let arena = simulation.source_arena();
    let economy = &arena.economy;
    let actor = |handle| backend.actor_lifetime_id(guard, handle);
    let owner = |slot| {
        if slot == 0
            || !simulation
                .players()
                .iter()
                .any(|player| player.entity_index == slot && player.active_player_index >= 0)
        {
            return None;
        }
        match arena.resolve(arena.handle(slot)?)? {
            SourceEntityView::Imported(row) if row.class == 3 && matches!(row.model, 0 | 1) => {
                backend.participant(u64::from(slot))
            }
            _ => None,
        }
    };

    let castles = economy.live_castles().filter_map(|row| {
        let slot = row.source_entity_index;
        let handle = current_handle(arena, slot, Kind::CastleRoot)?;
        let view = source_target::read(arena, source_target::Context::default(), slot).ok()??;
        let status = match row.action {
            4 => CastleStatus::Standing,
            5 => CastleStatus::Constructing,
            6 => CastleStatus::Destroying,
            _ => return None,
        };
        let owner_handle = arena.handle(row.owner_entity_index);
        let owner = owner_handle
            .filter(|handle| handle.generation == row.owner_slot_generation)
            .and_then(|handle| owner(handle.source_slot));
        Some(EconomyObservation {
            actor: actor(handle),
            owner,
            position: position(row.position),
            lifecycle: lifecycle(view.drawing_disabled(), row.life),
            asset: EconomyAsset::Castle {
                stage: row.level,
                status,
                life: row.life,
                max_life: row.max_life,
                mana: row.mana,
                mana_capacity: row.mana_capacity,
            },
        })
    });
    let collectors = economy.castle_children().iter().filter_map(|row| {
        if !is_collector(row) {
            return None;
        }
        let slot = row.source_entity_index;
        let handle = current_handle(arena, slot, Kind::CastleDefender)?;
        // The ordered root stock, not the child's unused parent address,
        // establishes this relation. The owner index carries both generations.
        let home = economy
            .castle_for_owner(row.owner_entity_index)
            .and_then(|root| {
                let order = root
                    .defender_slots
                    .iter()
                    .position(|&holder| holder == slot)?;
                Some((
                    current_handle(arena, root.source_entity_index, Kind::CastleRoot)?,
                    u8::try_from(order).expect("Castle stock has three positions"),
                ))
            });
        let target_slot = row.target_sphere_slot;
        let target = home
            .filter(|(root, _)| root.source_slot == target_slot)
            .map(|(root, _)| root)
            .or_else(|| {
                economy
                    .spheres()
                    .iter()
                    .find(|sphere| {
                        sphere.active
                            && !sphere.pending_delete
                            && sphere.source_entity_index == target_slot
                            && sphere.player_entity_index == row.id_entity_index
                    })
                    .and_then(|_| current_handle(arena, target_slot, Kind::ManaSphere))
            });
        Some(EconomyObservation {
            actor: actor(handle),
            owner: owner(row.owner_entity_index),
            position: position(row.position),
            lifecycle: lifecycle(row.drawing_disabled(), row.life),
            asset: EconomyAsset::Collector {
                castle: home.map(|(root, _)| actor(root)),
                castle_order: home.map(|(_, order)| order),
                target: target.map(actor),
                life: row.life,
                max_life: row.max_life,
                mana: row.mana,
                mana_capacity: row.max_mana,
            },
        })
    });
    let spheres = economy.spheres().iter().filter_map(|row| {
        let slot = row.source_entity_index;
        let handle = current_handle(arena, slot, Kind::ManaSphere)?;
        let assigned_collector = economy
            .castle_children()
            .iter()
            .find(|collector| {
                is_collector(collector)
                    && !collector.drawing_disabled()
                    && collector.source_entity_index == row.assigned_defender_slot
                    && collector.target_sphere_slot == slot
                    && collector.id_entity_index == row.player_entity_index
            })
            .and_then(|collector| {
                current_handle(arena, collector.source_entity_index, Kind::CastleDefender)
            });
        Some(EconomyObservation {
            actor: actor(handle),
            owner: owner(row.owner_entity_index),
            position: position(row.position),
            lifecycle: lifecycle(row.pending_delete, row.life),
            asset: EconomyAsset::ManaSphere {
                mana: row.mana,
                assigned_collector: assigned_collector.map(actor),
            },
        })
    });
    castles
        .chain(collectors)
        .chain(spheres)
        .skip(offset)
        .take(count)
        .collect()
}

fn current_handle(arena: &SourceArena, slot: u16, kind: Kind) -> Option<SourceEntityHandle> {
    if slot == 0 {
        return None;
    }
    let handle = arena.handle(slot)?;
    matches!(arena.resolve(handle), Some(SourceEntityView::Dynamic(current)) if current == kind)
        .then_some(handle)
}

fn is_collector(row: &Level2CastleChild) -> bool {
    row.active && (row.source_class, row.model, row.action) == (3, 3, 9)
}

fn position(value: Position) -> EconomyPosition {
    EconomyPosition {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn lifecycle(hidden: bool, life: i32) -> ActorLifecycle {
    if hidden {
        ActorLifecycle::Hidden
    } else if life < 0 {
        ActorLifecycle::Dying
    } else {
        ActorLifecycle::Alive
    }
}
