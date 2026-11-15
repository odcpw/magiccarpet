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
        actions.sound_id = (input.objective_index != 0).then_some(61);
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
            actions.sound_id = (input.objective_index != 0).then_some(61);
            let segment = if input.level_ended {
                9
            } else {
                input.objective_index.wrapping_add(1)
            };
            actions.speech = if (30..=34).contains(&input.source_level) {
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

    fn level_three_input() -> ObjectivePresentationInput {
        ObjectivePresentationInput {
            palette_mod: 3,
            objective_index: 0,
            level_ended: false,
            speech_enabled: true,
            show_foreign_text: false,
            source_level: 2,
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
