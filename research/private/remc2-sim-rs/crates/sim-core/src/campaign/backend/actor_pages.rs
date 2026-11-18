//! Join current creature observations to session-scoped actor IDs.

use super::Mc2CampaignBackend;
use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
use crate::campaign::spec::Mc2EntityRuntimeBinding;
use crate::game_definition::SemanticId;
use crate::game_session::{ActorObservation, SessionGuard};
use crate::native_world::source_target::PAGED_CREATURES;

impl Mc2CampaignBackend {
    /// Semantic kinds actually admitted by the guarded campaign read.
    pub fn actor_page_kinds() -> impl Iterator<Item = String> {
        PAGED_CREATURES
            .into_iter()
            .map(|kind| format!("creature.{}", kind.as_str()))
    }
}

pub(super) fn observe(
    backend: &Mc2CampaignBackend,
    guard: SessionGuard,
    offset: usize,
    count: usize,
) -> Vec<ActorObservation> {
    let definition = backend.campaign.definition();
    let authored_ids = definition.authored_creature_entity_bindings();
    let actor = |handle| backend.actor_lifetime_id(guard, handle);
    backend
        .campaign
        .simulation()
        .creature_page_sources()
        .skip(offset)
        .take(count)
        .map(|row| {
            let identity = row.identity;
            let entity_id = if identity.role != CreatureRuntimeRole::Root {
                None
            } else {
                match identity.origin {
                    CreatureRootOrigin::InitialPackage => definition
                        .level_definition()
                        .entities()
                        .iter()
                        .find(|entity| {
                            matches!(
                                definition.movable_entity_binding(entity.id()),
                                Some(Mc2EntityRuntimeBinding::InitialSource { initial_source_slot, .. })
                                    if *initial_source_slot == identity.handle.source_slot
                            )
                        })
                        .map(|entity| entity.id().clone()),
                    CreatureRootOrigin::AuthoredRecord { authored_record } => {
                        authored_ids.get(&authored_record).cloned()
                    }
                    _ => None,
                }
            };
            ActorObservation {
                actor: actor(identity.handle),
                kind: SemanticId::parse(&format!("creature.{}", identity.kind.as_str()))
                    .expect("static creature kind is a semantic ID"),
                entity_id,
                pose: row.pose,
                roll: row.roll,
                life: row.health.map(|(life, _)| life),
                max_life: row.health.map(|(_, max_life)| max_life),
                target: row.target.map(actor),
                parent: row.parent.map(actor),
                lifecycle: row.lifecycle,
            }
        })
        .collect()
}
