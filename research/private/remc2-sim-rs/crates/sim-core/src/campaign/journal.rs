//! Ordered production effects for descriptor-backed campaign sessions.
//!
//! The journal owns cross-domain ordering and cursor lifetime.  Existing
//! Level 2 event slices remain compatibility projections until CF-12/CF-13;
//! production append sites write this journal first and then update those
//! projections.  Diagnostic parity traces are deliberately excluded.

use sha2::{Digest as _, Sha256};
use std::{fmt, sync::OnceLock};

use crate::{
    FlightEvent,
    native_world::{Level2SharedSpellEvent, Mc2DispatchEvent},
};

use super::state::CanonicalDigest;

const MAX_RETAINED_EFFECTS: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignEffect {
    Dispatch(Mc2DispatchEvent),
    SharedSpell(Level2SharedSpellEvent),
    Flight(FlightEvent),
    SceneChange { scene: u8 },
    ObjectivePresentation(super::objective_speech::ObjectivePresentationActions),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRecord {
    pub sequence: u64,
    pub turn: u32,
    pub effect: CampaignEffect,
    /// PrepareEventSound inputs are resolved at the producer boundary, before
    /// the source slot can move, be reclaimed, or be reused by another entity.
    pub audio: Option<crate::PreparedEventSoundCommand>,
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

pub struct EffectJournal {
    first_sequence: u64,
    next_sequence: u64,
    records: Vec<EffectRecord>,
    // Keep the retained window contiguous for existing readers, but reclaim
    // its expired prefix in batches instead of moving it on every append.
    retained_start: usize,
    // Observations of one unchanged journal share its exact receipt. This is
    // derived data, excluded from canonical equality and invalidated on append.
    receipt: OnceLock<CanonicalDigest>,
}

impl Clone for EffectJournal {
    fn clone(&self) -> Self {
        Self {
            first_sequence: self.first_sequence,
            next_sequence: self.next_sequence,
            records: self.records().to_vec(),
            retained_start: 0,
            receipt: self.receipt.clone(),
        }
    }
}

impl fmt::Debug for EffectJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectJournal")
            .field("first_sequence", &self.first_sequence)
            .field("next_sequence", &self.next_sequence)
            .field("records", &self.records())
            .field("receipt", &self.receipt)
            .finish()
    }
}

impl PartialEq for EffectJournal {
    fn eq(&self, other: &Self) -> bool {
        self.first_sequence == other.first_sequence
            && self.next_sequence == other.next_sequence
            && self.records() == other.records()
    }
}

impl Eq for EffectJournal {}

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
            retained_start: 0,
            receipt: OnceLock::new(),
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
        &self.records[self.retained_start..]
    }

    pub(crate) fn append_dispatch(&mut self, turn: u32, event: Mc2DispatchEvent) {
        self.append(turn, CampaignEffect::Dispatch(event));
    }

    pub(crate) fn append_shared_spell(&mut self, turn: u32, event: Level2SharedSpellEvent) {
        self.append(turn, CampaignEffect::SharedSpell(event));
    }

    pub(crate) fn append_flight(&mut self, turn: u32, event: FlightEvent) {
        self.append(turn, CampaignEffect::Flight(event));
    }

    pub(crate) fn append_prepared_flight(
        &mut self,
        turn: u32,
        event: FlightEvent,
        audio: Option<crate::PreparedEventSoundCommand>,
    ) {
        self.append(turn, CampaignEffect::Flight(event));
        if let Some(record) = self.records.last_mut() {
            record.audio = audio;
        }
    }

    pub(crate) fn append_scene_change(&mut self, turn: u32, scene: u8) {
        self.append(turn, CampaignEffect::SceneChange { scene });
    }

    pub(crate) fn append_objective_presentation(
        &mut self,
        turn: u32,
        actions: super::objective_speech::ObjectivePresentationActions,
    ) {
        self.append(turn, CampaignEffect::ObjectivePresentation(actions));
    }

    fn append(&mut self, turn: u32, effect: CampaignEffect) {
        self.receipt.take();
        if self.retained_start > 0 && self.records.len() == self.records.capacity() {
            self.records.drain(..self.retained_start);
            self.retained_start = 0;
        }
        // Partial-length clones may have non-power-of-two capacities. Reserve
        // a full spare window once retention starts, avoiding repeated tiny
        // compactions when such a capacity is only slightly above the limit.
        if self.records().len() == MAX_RETAINED_EFFECTS
            && self.records.capacity() < MAX_RETAINED_EFFECTS * 2
        {
            self.records
                .reserve_exact(MAX_RETAINED_EFFECTS * 2 - self.records.len());
        }
        self.records.push(EffectRecord {
            sequence: self.next_sequence,
            turn,
            effect,
            audio: None,
        });
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.records().len() > MAX_RETAINED_EFFECTS {
            // Release boxed event payloads at logical expiry, as before. This
            // private retired slot is excluded from every observation and hash.
            let retired = &mut self.records[self.retained_start];
            retired.effect = CampaignEffect::SceneChange { scene: 0 };
            retired.audio = None;
            self.retained_start += 1;
            self.first_sequence = self.records[self.retained_start].sequence;
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
        let retained = self.records();
        let offset = usize::try_from(cursor.next_sequence - self.first_sequence)
            .expect("retained effect offset fits usize")
            .min(retained.len());
        let pending = retained.len() - offset;
        if pending > 0 && capacity == 0 {
            return Err(EffectPageError::OutputTooSmall { pending });
        }
        let count = pending.min(capacity);
        let records = retained[offset..offset + count].to_vec().into_boxed_slice();
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
        *self.receipt.get_or_init(|| self.calculate_receipt())
    }

    fn calculate_receipt(&self) -> CanonicalDigest {
        let mut hasher = Sha256::new();
        hasher.update(b"carpet-flight-effect-journal-v1");
        hasher.update(self.first_sequence.to_le_bytes());
        hasher.update(self.next_sequence.to_le_bytes());
        for record in self.records() {
            hasher.update(record.sequence.to_le_bytes());
            hasher.update(record.turn.to_le_bytes());
            if let Some(audio) = record.audio {
                let command = audio.command;
                hasher.update([1, audio.play_type, u8::from(command.audible)]);
                hasher.update(command.relative_x.to_le_bytes());
                hasher.update(command.relative_y.to_le_bytes());
                hasher.update(command.relative_z.to_le_bytes());
                hasher.update(command.distance_xy.to_le_bytes());
                hasher.update(command.relative_yaw.to_le_bytes());
                hasher.update(command.volume_q15.to_le_bytes());
                hasher.update(command.pan_q16.to_le_bytes());
                hasher.update(command.rate_delta_percent.to_le_bytes());
                hasher.update(command.sound_flags.to_le_bytes());
            } else {
                hasher.update([0]);
            }
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
                CampaignEffect::ObjectivePresentation(actions) => {
                    hasher.update([
                        5,
                        u8::from(actions.palette_flash),
                        u8::from(actions.duck_audio),
                    ]);
                    hasher.update(actions.sound_id.unwrap_or(0).to_le_bytes());
                    hasher.update([actions.textbox_ticks.unwrap_or(0)]);
                    if let Some(speech) = actions.speech {
                        hasher.update([1, speech.track, speech.segment]);
                        hasher.update(speech.start_ms.to_le_bytes());
                        hasher.update(speech.length_ms.to_le_bytes());
                    } else {
                        hasher.update([0]);
                    }
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

    fn level_two_session() -> crate::native_world::Mc2Simulation {
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
            Some(CampaignEffect::Dispatch(Mc2DispatchEvent::SharedPickup {
                source_slot: 75,
                ..
            }))
        ));
        assert!(
            effects
                .windows(2)
                .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        );

        let ordered_receipt = session.effect_journal().receipt();
        let mut swapped = session.effect_journal().clone();
        swapped.receipt.take();
        swapped.records.swap(collected, collected + 1);
        let mut duplicated = session.effect_journal().clone();
        duplicated.receipt.take();
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
    fn receipt_cache_preserves_append_retention_clone_and_concurrent_observation() {
        let mut journal = EffectJournal::new();
        let empty = journal.receipt();
        assert_eq!(empty, journal.calculate_receipt());
        assert_eq!(journal, EffectJournal::new());
        for turn in 0..MAX_RETAINED_EFFECTS {
            journal.append_scene_change(turn as u32, (turn % 3) as u8);
        }
        assert!(journal.receipt.get().is_none());
        let retained = journal.receipt();
        assert_eq!(retained, journal.calculate_receipt());
        assert_ne!(retained, empty);
        let mut clone = journal.clone();
        assert_eq!(clone.receipt(), retained);
        clone.append_scene_change(MAX_RETAINED_EFFECTS as u32, 2);
        assert!(clone.receipt.get().is_none());
        assert_eq!(clone.first_sequence, 1);
        // Retention is the public logical window, not private spare storage.
        assert_eq!(clone.records().len(), MAX_RETAINED_EFFECTS);
        assert_eq!(clone.receipt(), clone.calculate_receipt());
        assert_ne!(clone.receipt(), retained);
        assert_eq!(journal.receipt(), retained);
        assert_ne!(journal, clone);
        journal.receipt.take();
        std::thread::scope(|scope| {
            let journal = &journal;
            for _ in 0..4 {
                scope.spawn(move || assert_eq!(journal.receipt(), retained));
            }
        });
        assert_eq!(journal.receipt.get(), Some(&retained));
        journal = EffectJournal::new();
        assert_eq!(journal.receipt(), empty);
    }

    #[test]
    fn batched_retention_matches_eager_order_audio_pages_and_receipts() {
        use std::collections::VecDeque;

        let mut journal = EffectJournal::new();
        let mut eager = VecDeque::new();
        let checkpoints = [
            0,
            MAX_RETAINED_EFFECTS - 1,
            MAX_RETAINED_EFFECTS,
            MAX_RETAINED_EFFECTS + 1,
            MAX_RETAINED_EFFECTS * 2 - 1,
            MAX_RETAINED_EFFECTS * 2,
            MAX_RETAINED_EFFECTS * 2 + 1,
            MAX_RETAINED_EFFECTS * 3,
        ];
        for turn in 0..=MAX_RETAINED_EFFECTS * 3 {
            let turn = u32::try_from(turn).expect("bounded test turn");
            let audio = crate::PreparedEventSoundCommand {
                play_type: (turn % 4) as u8,
                command: crate::PositionalSoundCommand {
                    relative_x: turn as i32,
                    relative_y: -(turn as i32),
                    relative_z: 17,
                    distance_xy: turn,
                    relative_yaw: (turn % 2048) as u16,
                    volume_q15: 8192,
                    pan_q16: 32768,
                    rate_delta_percent: -7,
                    sound_flags: 3,
                    audible: turn % 2 == 0,
                },
            };
            let (effect, prepared) = match turn % 3 {
                0 => (
                    CampaignEffect::SceneChange {
                        scene: (turn % 7) as u8,
                    },
                    None,
                ),
                1 => (
                    CampaignEffect::Dispatch(Mc2DispatchEvent::AuthoredGroupActivated {
                        pass: 1,
                        source_slot: 7,
                        group: 3,
                        authored_indices: vec![turn as u16, 23].into_boxed_slice(),
                    }),
                    None,
                ),
                _ => (
                    CampaignEffect::Flight(FlightEvent::UpdateMusic {
                        mode: 2,
                        timer_multiplier: 3,
                    }),
                    Some(audio),
                ),
            };
            match effect.clone() {
                CampaignEffect::SceneChange { scene } => journal.append_scene_change(turn, scene),
                CampaignEffect::Dispatch(event) => journal.append_dispatch(turn, event),
                CampaignEffect::Flight(event) => {
                    journal.append_prepared_flight(turn, event, prepared)
                }
                _ => unreachable!("three test producers"),
            }
            eager.push_back(EffectRecord {
                sequence: u64::from(turn),
                turn,
                effect,
                audio: prepared,
            });
            if eager.len() > MAX_RETAINED_EFFECTS {
                eager.pop_front();
            }
            assert!(journal.records().len() <= MAX_RETAINED_EFFECTS);
            assert!(journal.records.len() <= MAX_RETAINED_EFFECTS * 2);
            assert!(journal.records.capacity() <= MAX_RETAINED_EFFECTS * 2);
            if !checkpoints.contains(&(turn as usize)) {
                continue;
            }

            // Independent eager queue supplies the exact public sequence; no
            // retired prefix exists in this reference representation.
            let reference = EffectJournal {
                first_sequence: eager.front().expect("nonempty").sequence,
                next_sequence: u64::from(turn) + 1,
                records: eager.iter().cloned().collect(),
                retained_start: 0,
                receipt: OnceLock::new(),
            };
            assert_eq!(journal.records(), reference.records());
            assert_eq!(journal, reference);
            assert_eq!(journal.receipt(), reference.receipt());
            let clone = journal.clone();
            assert_eq!(clone.retained_start, 0);
            assert_eq!(clone.records.len(), reference.records.len());
            assert_eq!(clone, journal);
            assert_eq!(clone.receipt(), journal.receipt());
            assert_eq!(format!("{clone:?}"), format!("{journal:?}"));
            assert!(
                journal.records[..journal.retained_start]
                    .iter()
                    .all(|record| matches!(
                        record.effect,
                        CampaignEffect::SceneChange { scene: 0 }
                    ) && record.audio.is_none())
            );

            let first = reference.first_sequence;
            let head = reference.next_sequence;
            for cursor in [
                first.saturating_sub(1),
                first,
                (first + 17).min(head),
                head - 1,
                head,
                head + 1,
            ] {
                for capacity in [0, 1, 17, MAX_RETAINED_EFFECTS] {
                    let cursor = EffectCursor {
                        generation: 7,
                        next_sequence: cursor,
                    };
                    assert_eq!(
                        journal.page(7, cursor, capacity),
                        reference.page(7, cursor, capacity)
                    );
                    assert_eq!(
                        journal.page(8, cursor, capacity),
                        reference.page(8, cursor, capacity)
                    );
                }
            }
        }
    }

    #[test]
    fn partial_clone_retention_has_a_full_spare_window_and_amortized_compaction() {
        for initial in [
            MAX_RETAINED_EFFECTS / 2 + 1,
            MAX_RETAINED_EFFECTS - 1,
            MAX_RETAINED_EFFECTS,
        ] {
            let mut original = EffectJournal::new();
            for turn in 0..initial {
                original.append_scene_change(turn as u32, 1);
            }
            let mut journal = original.clone();
            let mut compactions = 0;
            for turn in initial..initial + MAX_RETAINED_EFFECTS * 4 {
                let before = journal.retained_start;
                journal.append_scene_change(turn as u32, 2);
                if journal.retained_start < before {
                    compactions += 1;
                }
                assert!(journal.records.capacity() <= MAX_RETAINED_EFFECTS * 2);
                assert!(journal.records.len() <= MAX_RETAINED_EFFECTS * 2);
                assert_eq!(
                    journal.records().len(),
                    (turn + 1).min(MAX_RETAINED_EFFECTS)
                );
            }
            assert!(
                compactions <= 4,
                "must not move the full window every few appends"
            );
            assert_eq!(original.records().len(), initial);
            assert!(
                original.records().iter().all(|record| matches!(
                    record.effect,
                    CampaignEffect::SceneChange { scene: 1 }
                ))
            );
        }
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
