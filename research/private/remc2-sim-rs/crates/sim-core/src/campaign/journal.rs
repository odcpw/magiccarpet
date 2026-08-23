//! Ordered production effects for descriptor-backed campaign sessions.
//!
//! The journal owns cross-domain ordering and cursor lifetime.  Existing
//! Level 2 event slices remain compatibility projections until CF-12/CF-13;
//! production append sites write this journal first and then update those
//! projections.  Diagnostic parity traces are deliberately excluded.

use sha2::{Digest as _, Sha256};

use crate::{
    FlightEvent,
    native_world::{Level2DispatchEvent, Level2SharedSpellEvent},
};

use super::state::CanonicalDigest;

const MAX_RETAINED_EFFECTS: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignEffect {
    Dispatch(Level2DispatchEvent),
    SharedSpell(Level2SharedSpellEvent),
    Flight(FlightEvent),
    SceneChange { scene: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRecord {
    pub sequence: u64,
    pub turn: u32,
    pub effect: CampaignEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectCursor {
    pub generation: u64,
    pub next_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectPage {
    pub records: Box<[EffectRecord]>,
    pub next_cursor: EffectCursor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectPageError {
    StaleGeneration {
        expected: u64,
        actual: u64,
    },
    ExpiredCursor {
        first_available: u64,
        requested: u64,
    },
    FutureCursor {
        next_sequence: u64,
        requested: u64,
    },
    OutputTooSmall {
        pending: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectJournal {
    first_sequence: u64,
    next_sequence: u64,
    records: Vec<EffectRecord>,
}

impl Default for EffectJournal {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectJournal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            first_sequence: 0,
            next_sequence: 0,
            records: Vec::new(),
        }
    }

    #[must_use]
    pub const fn cursor(&self, generation: u64) -> EffectCursor {
        EffectCursor {
            generation,
            next_sequence: self.next_sequence,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[EffectRecord] {
        &self.records
    }

    pub(crate) fn append_dispatch(&mut self, turn: u32, event: Level2DispatchEvent) {
        self.append(turn, CampaignEffect::Dispatch(event));
    }

    pub(crate) fn append_shared_spell(&mut self, turn: u32, event: Level2SharedSpellEvent) {
        self.append(turn, CampaignEffect::SharedSpell(event));
    }

    pub(crate) fn append_flight(&mut self, turn: u32, event: FlightEvent) {
        self.append(turn, CampaignEffect::Flight(event));
    }

    pub(crate) fn append_scene_change(&mut self, turn: u32, scene: u8) {
        self.append(turn, CampaignEffect::SceneChange { scene });
    }

    fn append(&mut self, turn: u32, effect: CampaignEffect) {
        self.records.push(EffectRecord {
            sequence: self.next_sequence,
            turn,
            effect,
        });
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.records.len() > MAX_RETAINED_EFFECTS {
            let expired = self.records.len() - MAX_RETAINED_EFFECTS;
            self.records.drain(..expired);
            self.first_sequence = self
                .records
                .first()
                .map_or(self.next_sequence, |record| record.sequence);
        }
    }

    pub fn page(
        &self,
        generation: u64,
        cursor: EffectCursor,
        capacity: usize,
    ) -> Result<EffectPage, EffectPageError> {
        if cursor.generation != generation {
            return Err(EffectPageError::StaleGeneration {
                expected: generation,
                actual: cursor.generation,
            });
        }
        if cursor.next_sequence < self.first_sequence {
            return Err(EffectPageError::ExpiredCursor {
                first_available: self.first_sequence,
                requested: cursor.next_sequence,
            });
        }
        if cursor.next_sequence > self.next_sequence {
            return Err(EffectPageError::FutureCursor {
                next_sequence: self.next_sequence,
                requested: cursor.next_sequence,
            });
        }
        let offset = usize::try_from(cursor.next_sequence - self.first_sequence)
            .expect("retained effect offset fits usize")
            .min(self.records.len());
        let pending = self.records.len() - offset;
        if pending > 0 && capacity == 0 {
            return Err(EffectPageError::OutputTooSmall { pending });
        }
        let count = pending.min(capacity);
        let records = self.records[offset..offset + count]
            .to_vec()
            .into_boxed_slice();
        let next_cursor = EffectCursor {
            generation,
            next_sequence: cursor.next_sequence + u64::try_from(count).expect("count fits u64"),
        };
        Ok(EffectPage {
            records,
            next_cursor,
        })
    }

    #[must_use]
    pub fn receipt(&self) -> CanonicalDigest {
        let mut hasher = Sha256::new();
        hasher.update(b"carpet-flight-effect-journal-v1");
        hasher.update(self.first_sequence.to_le_bytes());
        hasher.update(self.next_sequence.to_le_bytes());
        for record in &self.records {
            hasher.update(record.sequence.to_le_bytes());
            hasher.update(record.turn.to_le_bytes());
            match &record.effect {
                CampaignEffect::Dispatch(event) => {
                    hasher.update([1]);
                    let (pass, kind, family, flags, source_slot) = event.campaign_projection();
                    hasher.update([pass, kind, family, flags]);
                    hasher.update(source_slot.to_le_bytes());
                }
                CampaignEffect::SharedSpell(event) => {
                    hasher.update([2]);
                    hash_shared_spell(&mut hasher, *event);
                }
                CampaignEffect::Flight(event) => {
                    hasher.update([3]);
                    hash_flight(&mut hasher, *event);
                }
                CampaignEffect::SceneChange { scene } => {
                    hasher.update([4, *scene]);
                }
            }
        }
        CanonicalDigest::from_bytes(hasher.finalize().into())
    }
}

fn hash_shared_spell(hasher: &mut Sha256, event: Level2SharedSpellEvent) {
    match event {
        Level2SharedSpellEvent::SpellCollected {
            source_slot,
            player_entity_index,
            family,
        } => {
            hasher.update([1, family]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update(player_entity_index.to_le_bytes());
        }
        Level2SharedSpellEvent::SpellAcquired {
            source_slot,
            player_entity_index,
            family,
            holder_action,
        } => {
            hasher.update([2, family, holder_action]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update(player_entity_index.to_le_bytes());
        }
        Level2SharedSpellEvent::ShieldDamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage,
        } => {
            hasher.update([3]);
            hasher.update(target_entity_index.to_le_bytes());
            hasher.update(attacker_entity_index.to_le_bytes());
            hasher.update(damage.raw_damage.to_le_bytes());
            hasher.update(damage.applied_damage.to_le_bytes());
            hasher.update(damage.mana_drained.to_le_bytes());
            hasher.update(damage.experience_awarded.to_le_bytes());
            hasher.update(damage.status_before.to_le_bytes());
            hasher.update(damage.status_after.to_le_bytes());
        }
        Level2SharedSpellEvent::ExperienceScrollCollected {
            source_slot,
            player_entity_index,
        } => {
            hasher.update([4]);
            hasher.update(source_slot.to_le_bytes());
            hasher.update(player_entity_index.to_le_bytes());
        }
        Level2SharedSpellEvent::AbilityImproved {
            source_family,
            tier,
        } => hasher.update([5, source_family, tier.cast_unsigned()]),
    }
}

fn hash_flight(hasher: &mut Sha256, event: FlightEvent) {
    match event {
        FlightEvent::PrepareSound {
            entity_index,
            player_index,
            sound_id,
            ..
        } => {
            hasher.update([1]);
            hasher.update(entity_index.to_le_bytes());
            hasher.update(player_index.to_le_bytes());
            hasher.update(sound_id.to_le_bytes());
        }
        FlightEvent::EndLoop {
            entity_index,
            player_index,
            sound_id,
            ..
        } => {
            hasher.update([2]);
            hasher.update(entity_index.to_le_bytes());
            hasher.update(player_index.to_le_bytes());
            hasher.update(sound_id.to_le_bytes());
        }
        FlightEvent::UpdateMusic {
            mode,
            timer_multiplier,
        } => hasher.update([3, mode, timer_multiplier]),
        FlightEvent::RequestSpellActivation {
            source,
            holder_entity_index,
            direction_mask,
        } => {
            let source_tag = match source {
                crate::SpellFireSource::Left => 1,
                crate::SpellFireSource::Right => 2,
                crate::SpellFireSource::Selected => 3,
            };
            hasher.update([4, source_tag]);
            hasher.update(holder_entity_index.to_le_bytes());
            hasher.update(direction_mask.to_le_bytes());
        }
        FlightEvent::ShowHelpHint { hint_index, state } => {
            hasher.update([5, state]);
            hasher.update(hint_index.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::{PlayerCommand, Position, native_world::NativeWorld};

    fn level_two_session() -> crate::native_world::Level2ResetSession {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        NativeWorld::inspect_level2(package)
            .expect("qualified Level 2 package")
            .level2_reset_session()
            .expect("qualified Level 2 session")
    }

    #[test]
    fn cf08_objectives_and_effect_journal_preserve_source_order() {
        let mut session = level_two_session();
        let pickup = session
            .shared_spells()
            .pickups()
            .iter()
            .find(|pickup| pickup.index == 75)
            .expect("qualified Speed pickup")
            .position;
        session.state.position = Position {
            z: pickup.z,
            ..pickup
        };
        session.step_outer_turn(PlayerCommand::default(), 0);

        let effects = session.effect_journal().records();
        let collected = effects
            .iter()
            .position(|record| {
                matches!(
                    record.effect,
                    CampaignEffect::SharedSpell(Level2SharedSpellEvent::SpellCollected {
                        source_slot: 75,
                        ..
                    })
                )
            })
            .expect("ordinary source route journals the Speed collection");
        assert!(matches!(
            effects.get(collected + 1).map(|record| &record.effect),
            Some(CampaignEffect::Flight(FlightEvent::PrepareSound {
                sound_id: 18,
                ..
            }))
        ));
        assert!(matches!(
            effects.get(collected + 2).map(|record| &record.effect),
            Some(CampaignEffect::Dispatch(
                Level2DispatchEvent::SharedPickup {
                    source_slot: 75,
                    ..
                }
            ))
        ));
        assert!(
            effects
                .windows(2)
                .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        );

        let ordered_receipt = session.effect_journal().receipt();
        let mut swapped = session.effect_journal().clone();
        swapped.records.swap(collected, collected + 1);
        let mut duplicated = session.effect_journal().clone();
        duplicated
            .records
            .insert(collected + 1, duplicated.records[collected].clone());
        assert_ne!(swapped.receipt(), ordered_receipt);
        assert_ne!(duplicated.receipt(), ordered_receipt);

        let mut replay = level_two_session();
        replay.state.position = Position {
            z: pickup.z,
            ..pickup
        };
        replay.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(replay.effect_journal(), session.effect_journal());
    }

    #[test]
    fn cf08_cursor_generation_and_failed_capacity_are_atomic() {
        let mut journal = EffectJournal::new();
        journal.append_flight(
            1,
            FlightEvent::UpdateMusic {
                mode: 2,
                timer_multiplier: 3,
            },
        );
        let cursor = EffectCursor {
            generation: 7,
            next_sequence: 0,
        };
        let before = journal.clone();
        assert_eq!(
            journal.page(7, cursor, 0),
            Err(EffectPageError::OutputTooSmall { pending: 1 })
        );
        assert_eq!(journal, before);
        assert!(matches!(
            journal.page(
                7,
                EffectCursor {
                    generation: 6,
                    next_sequence: 0,
                },
                1,
            ),
            Err(EffectPageError::StaleGeneration { .. })
        ));

        assert!(matches!(
            journal.page(8, cursor, 1),
            Err(EffectPageError::StaleGeneration { .. })
        ));

        let mut expiring = EffectJournal::new();
        for turn in 0..=MAX_RETAINED_EFFECTS {
            expiring.append_flight(
                u32::try_from(turn).expect("bounded turn"),
                FlightEvent::UpdateMusic {
                    mode: 2,
                    timer_multiplier: 3,
                },
            );
        }
        assert!(matches!(
            expiring.page(
                9,
                EffectCursor {
                    generation: 9,
                    next_sequence: 0,
                },
                1,
            ),
            Err(EffectPageError::ExpiredCursor {
                first_available: 1,
                requested: 0
            })
        ));

        let mut session = level_two_session();
        let generation = session.session.generation;
        let before = session.clone();
        let mut undersized = [];
        assert!(matches!(
            session.step_outer_turn_transactional(
                generation,
                PlayerCommand::default(),
                0,
                &mut undersized,
            ),
            Err(super::super::state::CampaignStepError::Encode(_))
        ));
        assert_eq!(session, before);
        assert!(matches!(
            session.step_outer_turn_transactional(
                generation.wrapping_add(1),
                PlayerCommand::default(),
                0,
                &mut [0; 512],
            ),
            Err(super::super::state::CampaignStepError::StaleGeneration { .. })
        ));
        assert_eq!(session, before);

        let old_cursor = session.effect_journal().cursor(session.session.generation);
        session.reset();
        assert!(matches!(
            session
                .effect_journal()
                .page(session.session.generation, old_cursor, 1),
            Err(EffectPageError::StaleGeneration { .. })
        ));
    }
}
