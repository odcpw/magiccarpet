//! Campaign bindings for remc2 objective CD speech.
//!
//! `PresentObjective_59820` owns the phase gate. The caller retains its existing
//! phase/textbox carriers and services this once per outer frame, including
//! paused frames. Playback, completion, interruption and volume restoration are
//! consumers of the returned actions, not an independent gameplay clock here.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SpeechSegment {
    pub track: u8,
    pub segment: u8,
    pub start_ms: u32,
    pub length_ms: u32,
}

/// Source D41A0 byte36E0B bit1 and the local player's byte3E4. The flag
/// survives inner LevelInitGame; sub_53160 resets the player byte. Only the
/// outer sub_48350 clears the flag. Neither is semantic secret progress.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SecretAnnouncementState {
    pub active: bool,
    pub collected_count: u8,
}

impl SecretAnnouncementState {
    pub(crate) fn is_default(&self) -> bool { *self == Self::default() }

    pub(crate) fn text_record(self) -> Option<u16> {
        self.active.then_some(284 + u16::from(self.collected_count != 0))
    }

    /// Actual CollectLevelStats_5C530 visits, not unique secret completions.
    pub(crate) fn collect_statistics(&mut self, visits: u8) {
        if self.active { self.collected_count = self.collected_count.wrapping_add(visits); }
    }

    pub(crate) fn after_level_init(self) -> Self {
        Self { active: self.active, collected_count: 0 }
    }

    pub(crate) fn append_hash(self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        if !self.is_default() {
            hasher.update(b"source-secret-announcement-v1");
            hasher.update([u8::from(self.active), self.collected_count]);
        }
    }
}

/// The two literal CollectLevelStats calls in source sub_46830. Bit4 retry
/// skips the first call; source-secret result bit0x10 and level>=24 can add
/// the second call. The caller commits this only at an accepted handoff.
pub(crate) const fn source_statistics_visits(result: u8, internal_level: u16) -> u8 {
    if result & 4 != 0 { return 0; } // source rewrites the result to4 first
    (result & 2 != 0) as u8
        + (result & 0x10 != 0 && internal_level >= 24 && result & 2 != 0) as u8
}

/// `sub_88BA0` writes the pending popup selector, then closes the current
/// popup with the eight-frame poll delay. Unmapped stage kinds deliberately
/// retain the presentation owner's pending selector; they do not repeat the
/// most recently displayed row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectiveContextHelp {
    KeepPending,
    Select(u16),
}

impl ObjectiveContextHelp {
    pub(crate) fn for_stage(level_ended: bool, stage: Option<i8>) -> Self {
        let row = match (level_ended, stage) {
            (true, _) | (_, Some(-1)) => 80,
            (_, Some(0)) => 72,
            (_, Some(1)) => 75,
            (_, Some(2)) => 73,
            (_, Some(3)) => 77,
            (_, Some(5)) => 79,
            (_, Some(7)) => 76,
            (_, Some(8)) => 78,
            (_, Some(9)) => 74,
            _ => return Self::KeepPending,
        };
        Self::Select(row)
    }

    pub(crate) fn event(self) -> crate::FlightEvent {
        crate::FlightEvent::ShowHelpHint {
            hint_index: match self {
                Self::KeepPending => 0,
                Self::Select(row) => row,
            },
            // State 8 is a context request, distinct from row promotion (6).
            state: 8,
        }
    }
}

/// Pinned `Type_DB080_CdTrack.h`: 25 ordinary, two secret-announcement,
/// and one further source row. Zero-length segments have no speech.
const SOURCE_SEGMENTS: [[(u16, u16); 10]; 28] = [
    [
        (0x0000, 0x02ee),
        (0x0339, 0x012c),
        (0x04b0, 0x01c2),
        (0x06bd, 0x01c2),
        (0x08ca, 0x0177),
        (0x0a8c, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0c4e, 0x020d),
    ],
    [
        (0x0000, 0x0465),
        (0x04b0, 0x020d),
        (0x0708, 0x0258),
        (0x09ab, 0x012c),
        (0x0b22, 0x020d),
        (0x0d7a, 0x01c2),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0f87, 0x0258),
    ],
    [
        (0x0000, 0x0384),
        (0x03cf, 0x02a3),
        (0x06bd, 0x0177),
        (0x087f, 0x01c2),
        (0x0a8c, 0x0177),
        (0x0c4e, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0dc5, 0x0177),
    ],
    [
        (0x0000, 0x020d),
        (0x0258, 0x012c),
        (0x03cf, 0x01c2),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x05dc, 0x0258),
    ],
    [
        (0x0000, 0x02ee),
        (0x0339, 0x0177),
        (0x04fb, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0672, 0x02a3),
    ],
    [
        (0x0000, 0x0384),
        (0x03cf, 0x0177),
        (0x0591, 0x0177),
        (0x0753, 0x020d),
        (0x09ab, 0x01c2),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0bb8, 0x0177),
    ],
    [
        (0x0000, 0x0465),
        (0x04b0, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0672, 0x020d),
    ],
    [
        (0x0000, 0x0384),
        (0x03cf, 0x01c2),
        (0x05dc, 0x0177),
        (0x079e, 0x012c),
        (0x0915, 0x012c),
        (0x0a8c, 0x0258),
        (0x0d2f, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0f87, 0x012c),
    ],
    [
        (0x0000, 0x03cf),
        (0x041a, 0x0177),
        (0x05dc, 0x0096),
        (0x06bd, 0x00e1),
        (0x07e9, 0x020d),
        (0x0a41, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0c99, 0x020d),
    ],
    [
        (0x0000, 0x02a3),
        (0x02ee, 0x020d),
        (0x0546, 0x0177),
        (0x0708, 0x0258),
        (0x09ab, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0c03, 0x012c),
    ],
    [
        (0x0000, 0x0258),
        (0x02a3, 0x0177),
        (0x0465, 0x020d),
        (0x06bd, 0x0177),
        (0x087f, 0x01c2),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0a8c, 0x012c),
    ],
    [
        (0x0000, 0x0546),
        (0x0591, 0x0258),
        (0x0834, 0x01c2),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0a41, 0x012c),
    ],
    [
        (0x0000, 0x03cf),
        (0x041a, 0x01c2),
        (0x0627, 0x0177),
        (0x07e9, 0x0177),
        (0x09ab, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0b22, 0x01c2),
    ],
    [
        (0x0000, 0x02ee),
        (0x0339, 0x02a3),
        (0x0627, 0x020d),
        (0x087f, 0x01c2),
        (0x0a8c, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0c4e, 0x020d),
    ],
    [
        (0x0000, 0x0465),
        (0x04b0, 0x00e1),
        (0x05dc, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0834, 0x020d),
    ],
    [
        (0x0000, 0x0339),
        (0x0384, 0x020d),
        (0x05dc, 0x00e1),
        (0x0708, 0x00e1),
        (0x0834, 0x0177),
        (0x09f6, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0b6d, 0x0177),
    ],
    [
        (0x0000, 0x0465),
        (0x04b0, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0672, 0x012c),
    ],
    [
        (0x0000, 0x05dc),
        (0x0627, 0x01c2),
        (0x0834, 0x012c),
        (0x09ab, 0x01c2),
        (0x0bb8, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0d7a, 0x012c),
    ],
    [
        (0x0000, 0x020d),
        (0x0258, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x03cf, 0x00e1),
    ],
    [
        (0x0000, 0x0339),
        (0x0384, 0x020d),
        (0x05dc, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0834, 0x01c2),
    ],
    [
        (0x0000, 0x0339),
        (0x0384, 0x01c2),
        (0x0591, 0x0177),
        (0x0753, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x09ab, 0x0177),
    ],
    [
        (0x0000, 0x0465),
        (0x04b0, 0x00e1),
        (0x05dc, 0x020d),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0834, 0x01c2),
    ],
    [
        (0x0000, 0x03cf),
        (0x041a, 0x012c),
        (0x0591, 0x0177),
        (0x0753, 0x020d),
        (0x09ab, 0x012c),
        (0x0b22, 0x012c),
        (0x0c99, 0x01c2),
        (0x0ea6, 0x0177),
        (0x0000, 0x0000),
        (0x1068, 0x012c),
    ],
    [
        (0x0000, 0x0339),
        (0x0384, 0x0177),
        (0x0546, 0x020d),
        (0x079e, 0x012c),
        (0x0915, 0x012c),
        (0x0a8c, 0x012c),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0c03, 0x0177),
    ],
    [
        (0x0000, 0x04b0),
        (0x0000, 0x0000),
        (0x04fb, 0x02a3),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x07e9, 0x0627),
    ],
    [
        (0x0000, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
    ],
    [
        (0x0000, 0x0177),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
        (0x0000, 0x0000),
    ],
    [
        (0x5bd0, 0x0107),
        (0x5be0, 0x0107),
        (0x5bf0, 0x0107),
        (0x5c00, 0x0107),
        (0x5c10, 0x0107),
        (0x5c24, 0x0107),
        (0x5c34, 0x0107),
        (0x5c44, 0x0107),
        (0x5c60, 0x0107),
        (0x5c70, 0x0107),
    ],
];

/// Preserve the floating literal and truncation in
/// `PlayCDTrackSegmentNumber_86EB0`, including its one-millisecond shortfall.
fn source_cd_milliseconds(frames: u16) -> u32 {
    (f64::from(frames) * 13.333_333_333_33) as u32
}

#[must_use]
pub fn ordinary_speech_segment(source_level: u8, segment: u8) -> Option<SpeechSegment> {
    if source_level >= 25 {
        return None;
    }
    source_speech_segment(source_level, segment)
}

fn source_speech_segment(source_level: u8, segment: u8) -> Option<SpeechSegment> {
    let &(start, length) = SOURCE_SEGMENTS
        .get(usize::from(source_level))?
        .get(usize::from(segment))?;
    (length != 0).then(|| SpeechSegment {
        track: source_level + 1,
        segment,
        start_ms: source_cd_milliseconds(start),
        length_ms: source_cd_milliseconds(length),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectivePresentationInput {
    pub palette_mod: u8,
    /// Source `ObjectiveText_1`, a stage ordinal rather than a language ID.
    pub objective_index: u8,
    pub level_ended: bool,
    pub speech_enabled: bool,
    /// Source `autoShowObjectivesForForeignLanguages && SelectedLangIndex != 2`.
    pub show_foreign_text: bool,
    pub source_level: u8,
    /// Some is the source special announcement flag, carrying byte3E4.
    pub secret_announcement_count: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectivePresentationActions {
    pub sound_id: Option<u16>,
    pub palette_flash: bool,
    pub textbox_ticks: Option<u8>,
    pub speech: Option<SpeechSegment>,
    /// `FadeDownSoundVolume_59A50`: divide current sound/music units by three;
    /// a new segment replaces the old CD playback. Completion starts the
    /// source 120 Hz, one-volume-unit-at-a-time restoration.
    pub duck_audio: bool,
}

/// Ordinary single-player branch of `PresentObjective_59820`. The caller
/// decrements an existing textbox before this service and clears pending
/// objective presentation for multiplayer before selecting this branch.
#[must_use]
pub fn service_objective_presentation(
    phase: &mut u8,
    input: ObjectivePresentationInput,
) -> ObjectivePresentationActions {
    let mut actions = ObjectivePresentationActions::default();
    if *phase == 0 || input.palette_mod < 3 {
        return actions;
    }
    if !input.speech_enabled {
        *phase = 0;
        actions.textbox_ticks = Some(200);
        actions.sound_id = if input.secret_announcement_count.is_some() { Some(41) }
            else { (input.objective_index != 0).then_some(61) };
        return actions;
    }
    match *phase {
        7 => {
            *phase = 8;
            actions.palette_flash = true;
            actions.sound_id = Some(41);
        }
        8 => {
            *phase = 9;
            actions.palette_flash = true;
            actions.sound_id = if input.secret_announcement_count.is_some() { None }
                else { (input.objective_index != 0).then_some(61) };
            let segment = if input.level_ended {
                9
            } else {
                input.objective_index.wrapping_add(1)
            };
            actions.speech = if let Some(count) = input.secret_announcement_count {
                // Deliberate owner-media-supported adaptation: 86F20 omits
                // 86EB0's CD-frame conversion, so SDL cuts these 375-frame
                // announcements after375ms. Actual TRACK26/27 speech lasts
                // over4s. Use the same source conversion as ordinary speech
                // (375 frames ->4999ms), retaining its literal/truncation.
                source_speech_segment(25 + u8::from(count != 0), 0)
            } else if (30..=34).contains(&input.source_level) {
                if input.level_ended {
                    source_speech_segment(10, 9)
                } else {
                    source_speech_segment(0, 4)
                }
            } else {
                ordinary_speech_segment(input.source_level, segment)
            };
            actions.duck_audio = true;
            if input.show_foreign_text {
                actions.textbox_ticks = Some(200);
            }
        }
        // The pinned C++ declares v3 unsigned char, then compares it with -56.
        // Integer promotion makes that comparison true even at 200: preserve
        // the compiled source's increment/wrap, not a signed-byte inference.
        _ => *phase = phase.wrapping_add(1),
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_announcement_uses_source_cd_units_statistics_width_and_distinct_reset_boundaries() {
        let mut input = level_three_input();
        input.objective_index = 4;
        input.secret_announcement_count = Some(0);
        let alert = service_objective_presentation(&mut 7, input);
        assert_eq!(alert.sound_id, Some(41));
        assert!(alert.palette_flash);
        let spoken = service_objective_presentation(&mut 8, input);
        assert_eq!(SOURCE_SEGMENTS[25][0], (0, 375), "retain the original CD-frame table");
        assert_eq!(SOURCE_SEGMENTS[26][0], (0, 375));
        assert_eq!(spoken.speech, Some(SpeechSegment { track: 26, segment: 0, start_ms: 0, length_ms: 4999 }));
        assert!(spoken.duck_audio && spoken.palette_flash);
        assert_eq!(spoken.sound_id, None, "special speech does not also emit ordinary objective61");
        input.secret_announcement_count = Some(1);
        assert_eq!(service_objective_presentation(&mut 8, input).speech,
            Some(SpeechSegment { track: 27, segment: 0, start_ms: 0, length_ms: 4999 }));
        input.speech_enabled = false;
        let silent = service_objective_presentation(&mut 7, input);
        assert_eq!((silent.sound_id, silent.textbox_ticks, silent.speech), (Some(41), Some(200), None));
        let mut state = SecretAnnouncementState { active: true, collected_count: 255 };
        state.collect_statistics(source_statistics_visits(0x0a, 11));
        assert_eq!(state.collected_count, 0, "CollectStats uses the source unsigned byte wrap");
        assert_eq!(state.text_record(), Some(284));
        state.collect_statistics(source_statistics_visits(0x1a, 32));
        assert_eq!(state.collected_count, 2, "literal secret-result source branch has two visits");
        assert_eq!(state.text_record(), Some(285));
        assert_eq!(state.after_level_init(), SecretAnnouncementState { active: true, collected_count: 0 });
        assert_eq!(source_statistics_visits(4, 24), 0);
        assert_eq!(source_statistics_visits(0x16, 32), 0,
            "retry rewrites mixed secret/completed flags to4 before the second branch");
        assert_eq!(source_statistics_visits(0x1e, 32), 0);
        assert_eq!(source_statistics_visits(0x18, 32), 0);
        let mut inactive = SecretAnnouncementState::default();
        inactive.collect_statistics(2);
        assert_eq!(inactive, SecretAnnouncementState::default());
        assert_eq!(inactive.text_record(), None);
    }

    #[test]
    fn objective_expiry_context_uses_source_stage_mapping_and_keeps_pending_default() {
        for (stage, row) in [(0, 72), (1, 75), (2, 73), (3, 77), (5, 79), (7, 76), (8, 78), (9, 74), (-1, 80)] {
            assert_eq!(ObjectiveContextHelp::for_stage(false, Some(stage)), ObjectiveContextHelp::Select(row));
            assert_eq!(ObjectiveContextHelp::for_stage(true, Some(stage)), ObjectiveContextHelp::Select(80));
        }
        for stage in [None, Some(4), Some(6), Some(10)] {
            assert_eq!(ObjectiveContextHelp::for_stage(false, stage), ObjectiveContextHelp::KeepPending);
        }
        assert!(matches!(ObjectiveContextHelp::KeepPending.event(), crate::FlightEvent::ShowHelpHint { hint_index: 0, state: 8 }));
    }

    fn level_three_input() -> ObjectivePresentationInput {
        ObjectivePresentationInput {
            palette_mod: 3,
            objective_index: 0,
            level_ended: false,
            speech_enabled: true,
            show_foreign_text: false,
            source_level: 2,
            secret_announcement_count: None,
        }
    }

    #[test]
    fn level_three_first_objective_waits_for_source_phases_and_exact_cd_segment() {
        let mut phase = 1;
        let mut input = level_three_input();
        input.palette_mod = 2;
        assert_eq!(
            service_objective_presentation(&mut phase, input),
            Default::default()
        );
        assert_eq!(phase, 1);
        input.palette_mod = 3;
        for expected_phase in 2..=7 {
            assert_eq!(
                service_objective_presentation(&mut phase, input),
                Default::default()
            );
            assert_eq!(phase, expected_phase);
        }
        let alert = service_objective_presentation(&mut phase, input);
        assert_eq!(phase, 8);
        assert_eq!(alert.sound_id, Some(41));
        assert!(alert.palette_flash);
        assert!(!alert.duck_audio);
        let spoken = service_objective_presentation(&mut phase, input);
        assert_eq!(phase, 9);
        assert_eq!(
            spoken.speech,
            Some(SpeechSegment {
                track: 3,
                segment: 1,
                start_ms: 12_999,
                length_ms: 8_999,
            })
        );
        assert!(spoken.duck_audio);
        assert_eq!(spoken.sound_id, None);
        assert_eq!(spoken.textbox_ticks, None);
    }

    #[test]
    fn completion_foreign_text_and_speech_disabled_keep_source_branches() {
        let mut input = level_three_input();
        input.objective_index = 4;
        input.level_ended = true;
        input.show_foreign_text = true;
        let spoken = service_objective_presentation(&mut 8, input);
        assert_eq!(
            spoken.speech,
            Some(SpeechSegment {
                track: 3,
                segment: 9,
                start_ms: 46_999,
                length_ms: 4_999,
            })
        );
        assert_eq!(spoken.sound_id, Some(61));
        assert_eq!(spoken.textbox_ticks, Some(200));
        input.speech_enabled = false;
        let mut phase = 1;
        let text = service_objective_presentation(&mut phase, input);
        assert_eq!(phase, 0);
        assert_eq!(text.textbox_ticks, Some(200));
        assert_eq!(text.sound_id, Some(61));
        assert_eq!(text.speech, None);
        assert!(!text.duck_audio);
    }

    #[test]
    fn level_four_uses_authored_briefing_two_objectives_and_completion_only() {
        for (segment, start_ms, length_ms) in [
            (0, 0, 6_999),
            (1, 7_999, 3_999),
            (2, 12_999, 5_999),
            (9, 19_999, 7_999),
        ] {
            assert_eq!(
                ordinary_speech_segment(3, segment),
                Some(SpeechSegment {
                    track: 4,
                    segment,
                    start_ms,
                    length_ms
                })
            );
        }
        for segment in 3..=8 {
            assert!(ordinary_speech_segment(3, segment).is_none());
        }
        let mut input = level_three_input();
        input.source_level = 3;
        input.objective_index = 1;
        assert_eq!(
            service_objective_presentation(&mut 8, input).speech,
            ordinary_speech_segment(3, 2)
        );
        input.level_ended = true;
        assert_eq!(
            service_objective_presentation(&mut 8, input).speech,
            ordinary_speech_segment(3, 9)
        );
    }

    #[test]
    fn unsigned_source_tail_wraps_and_unbound_segments_do_not_substitute() {
        let input = level_three_input();
        let mut phase = 200;
        assert_eq!(
            service_objective_presentation(&mut phase, input),
            Default::default()
        );
        assert_eq!(phase, 201);
        phase = 255;
        assert_eq!(
            service_objective_presentation(&mut phase, input),
            Default::default()
        );
        assert_eq!(phase, 0);
        assert!(ordinary_speech_segment(2, 6).is_none());
        assert!(ordinary_speech_segment(2, 10).is_none());
        assert_eq!(ordinary_speech_segment(4, 1).unwrap().track, 5);
        assert!(ordinary_speech_segment(25, 1).is_none());
        assert_eq!(ordinary_speech_segment(0, 1).unwrap().track, 1);
        assert_eq!(ordinary_speech_segment(1, 1).unwrap().track, 2);
    }

    #[test]
    fn all_cd_table_rows_match_the_pinned_source_and_secret_levels_use_source_aliases() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../upstream/magic-carpet-2-hd/remc2/engine/Type_DB080_CdTrack.h"
        ));
        let rows: Vec<Vec<u16>> = source
            .lines()
            .filter(|line| line.starts_with("{0x"))
            .map(|line| {
                line.split(|c: char| !c.is_ascii_hexdigit() && c != 'x')
                    .filter_map(|word| word.strip_prefix("0x"))
                    .map(|word| u16::from_str_radix(word, 16).unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(rows.len(), SOURCE_SEGMENTS.len());
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.len(), 21);
            assert_eq!(usize::from(row[0]), index + 1);
            for segment in 0..10 {
                assert_eq!(
                    SOURCE_SEGMENTS[index][segment],
                    (row[1 + segment * 2], row[2 + segment * 2])
                );
            }
        }
        for source_level in 30..=34 {
            let mut input = level_three_input();
            input.source_level = source_level;
            input.objective_index = 3;
            assert_eq!(
                service_objective_presentation(&mut 8, input).speech,
                source_speech_segment(0, 4)
            );
            input.level_ended = true;
            assert_eq!(
                service_objective_presentation(&mut 8, input).speech,
                source_speech_segment(10, 9)
            );
        }
    }
}
