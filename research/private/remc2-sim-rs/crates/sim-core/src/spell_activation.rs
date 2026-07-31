//! Exact renderer-free reconstruction of remc2 `sub_5F660` and its two small
//! state helpers (`sub_5F7B0`, `sub_5F7E0`).

#[cfg(test)]
use crate::MissionAudioSite;
use crate::{FlightEvent, SourceAudioCallTag, mark_source_audio_call};

const DIRECTION_BITS: u32 = 0x0300;
const POINTER_ACTION_BIT: u32 = 0x0020;
const NOT_ENOUGH_MANA_SOUND: u16 = 29;
const NOT_ENOUGH_MANA_HINT: u16 = 41;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpellCasterState {
    pub entity_index: u16,
    pub model: u8,
    pub mana: i32,
    pub control_flags: u32,
    pub player_color_index: u8,
    /// Player-state byte `0x1BF`; this controls whether activation clears the
    /// source pointer/action bit.
    pub control_mode: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpellActivationHolder {
    pub entity_index: u16,
    pub model: u8,
    /// Signed source field `byte_0x46_70`.
    pub level_gate: i8,
    /// Signed active timer/counter `word_0x2E_46`.
    pub active_value: i16,
    /// Unsigned reset value `word_0x30_48`, copied bit-for-bit into the signed
    /// active field.
    pub activation_value: u16,
    /// Source field `maxMana_0x8C_140`, used here as the activation cost.
    pub mana_cost: i32,
    /// Source field `byte_0x3C_60`, set by the active model-1 path.
    pub action_marker: i8,
}

/// Apply the exact bounded state transition of `sub_5F660`.
///
/// `None` represents the source sentinel/invalid-holder condition. The return
/// value is the original routine's success byte. Presentation calls are
/// journaled in source order instead of being executed by the simulation.
pub fn activate_spell(
    caster: &mut SpellCasterState,
    holder: Option<&mut SpellActivationHolder>,
    direction_mask: u16,
    help_enabled: bool,
    events: &mut Vec<FlightEvent>,
) -> bool {
    let Some(holder) = holder else {
        return false;
    };
    let special_caster = caster.model == 1;
    let direction_mask = if special_caster { 0 } else { direction_mask };

    match holder.model {
        0 if holder.level_gate >= 2 && holder.active_value != 0 => return false,
        1 if holder.active_value > 0 => {
            holder.action_marker = 1;
            set_cast_direction(caster, direction_mask);
            clear_pointer_action(caster, holder.model);
            return true;
        }
        2 if holder.active_value > 0 => {
            if !special_caster {
                journal_mana_sound(caster, events, SourceAudioCallTag::NotEnoughManaImmediate);
            }
            return false;
        }
        4 | 6 | 8 | 0x0b | 0x0c | 0x0e if caster.model == 0 && holder.active_value > 0 => {
            holder.active_value = if holder.model == 4 { 7 } else { 1 };
            return false;
        }
        4 | 6 | 8 | 0x0b | 0x0c | 0x0e if caster.model != 0 && holder.active_value != 0 => {
            return false;
        }
        7 if holder.level_gate >= 1 && holder.active_value != 0 => return false,
        9 | 0x0a | 0x0d | 0x0f..=0x18 if holder.active_value != 0 => return false,
        _ => {}
    }

    if caster.mana < holder.mana_cost {
        if !special_caster {
            if help_enabled {
                events.push(FlightEvent::ShowHelpHint {
                    hint_index: NOT_ENOUGH_MANA_HINT,
                    state: 6,
                });
            }
            journal_mana_sound(caster, events, SourceAudioCallTag::NotEnoughManaFallback);
        }
        return false;
    }

    holder.active_value = holder.activation_value.cast_signed();
    set_cast_direction(caster, direction_mask);
    clear_pointer_action(caster, holder.model);
    true
}

fn set_cast_direction(caster: &mut SpellCasterState, direction_mask: u16) {
    caster.control_flags = (caster.control_flags & !DIRECTION_BITS) | u32::from(direction_mask);
}

fn clear_pointer_action(caster: &mut SpellCasterState, holder_model: u8) {
    if caster.control_mode < 2 || (caster.control_mode == 2 && holder_model != 1) {
        caster.control_flags &= !POINTER_ACTION_BIT;
    }
}

fn journal_mana_sound(
    caster: &SpellCasterState,
    events: &mut Vec<FlightEvent>,
    source_call: SourceAudioCallTag,
) {
    mark_source_audio_call(source_call);
    events.push(FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: MissionAudioSite::NotEnoughMana,
        entity_index: 0,
        player_index: i16::from(caster.player_color_index),
        sound_id: NOT_ENOUGH_MANA_SOUND,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caster() -> SpellCasterState {
        SpellCasterState {
            mana: 100,
            control_flags: 0xffff_ffff,
            player_color_index: 3,
            ..SpellCasterState::default()
        }
    }

    fn holder(model: u8) -> SpellActivationHolder {
        SpellActivationHolder {
            model,
            activation_value: 23,
            mana_cost: 50,
            ..SpellActivationHolder::default()
        }
    }

    #[test]
    fn invalid_holder_is_the_source_sentinel_no_op() {
        let mut caster = caster();
        let before = caster;
        assert!(!activate_spell(
            &mut caster,
            None,
            256,
            true,
            &mut Vec::new()
        ));
        assert_eq!(caster, before);
    }

    #[test]
    fn cold_activation_sets_timer_direction_and_pointer_state() {
        let mut caster = caster();
        let mut holder = holder(3);
        let mut events = Vec::new();

        assert!(activate_spell(
            &mut caster,
            Some(&mut holder),
            256,
            true,
            &mut events
        ));

        assert_eq!(holder.active_value, 23);
        assert_eq!(caster.control_flags & DIRECTION_BITS, 256);
        assert_eq!(caster.control_flags & POINTER_ACTION_BIT, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn active_model_one_retriggers_and_returns_success_without_mana() {
        let mut caster = caster();
        caster.mana = 0;
        let mut holder = holder(1);
        holder.active_value = 4;
        let mut events = Vec::new();

        assert!(activate_spell(
            &mut caster,
            Some(&mut holder),
            512,
            true,
            &mut events
        ));

        assert_eq!(holder.active_value, 4);
        assert_eq!(holder.action_marker, 1);
        assert_eq!(caster.control_flags & DIRECTION_BITS, 512);
        assert!(events.is_empty());
    }

    #[test]
    fn active_model_two_reports_sound_but_not_success_or_help() {
        let mut caster = caster();
        let mut holder = holder(2);
        holder.active_value = 1;
        let mut events = Vec::new();

        assert!(!activate_spell(
            &mut caster,
            Some(&mut holder),
            256,
            true,
            &mut events
        ));
        assert_eq!(
            events,
            vec![FlightEvent::PrepareSound {
                audio_site: MissionAudioSite::NotEnoughMana,
                entity_index: 0,
                player_index: 3,
                sound_id: 29,
            }]
        );
    }

    #[test]
    fn active_rearm_models_preserve_their_exact_timer_values() {
        for model in [4, 6, 8, 0x0b, 0x0c, 0x0e] {
            let mut caster = caster();
            let mut holder = holder(model);
            holder.active_value = 9;

            assert!(!activate_spell(
                &mut caster,
                Some(&mut holder),
                256,
                false,
                &mut Vec::new()
            ));
            assert_eq!(holder.active_value, if model == 4 { 7 } else { 1 });
        }
    }

    #[test]
    fn every_active_blocker_model_returns_without_mana_path() {
        let cases = [
            (0, 2),
            (7, 1),
            (9, 0),
            (0x0a, 0),
            (0x0d, 0),
            (0x0f, 0),
            (0x10, 0),
            (0x11, 0),
            (0x12, 0),
            (0x13, 0),
            (0x14, 0),
            (0x15, 0),
            (0x16, 0),
            (0x17, 0),
            (0x18, 0),
        ];
        for (model, level_gate) in cases {
            let mut caster = caster();
            let mut holder = holder(model);
            holder.level_gate = level_gate;
            holder.active_value = 5;
            holder.activation_value = 99;

            assert!(!activate_spell(
                &mut caster,
                Some(&mut holder),
                256,
                false,
                &mut Vec::new()
            ));
            assert_eq!(holder.active_value, 5, "model {model}");
        }
    }

    #[test]
    fn below_threshold_and_inactive_cases_reach_cold_activation() {
        let cases = [(0, 1, 5), (1, 0, 0), (2, 0, 0), (7, 0, 5), (9, 0, 0)];
        for (model, level_gate, active_value) in cases {
            let mut caster = caster();
            let mut holder = holder(model);
            holder.level_gate = level_gate;
            holder.active_value = active_value;

            assert!(activate_spell(
                &mut caster,
                Some(&mut holder),
                256,
                false,
                &mut Vec::new()
            ));
            assert_eq!(holder.active_value, 23, "model {model}");
        }
    }

    #[test]
    fn insufficient_mana_emits_help_then_sound_but_special_caster_suppresses_both() {
        let mut caster = caster();
        caster.mana = 49;
        let mut holder = holder(3);
        let mut events = Vec::new();

        assert!(!activate_spell(
            &mut caster,
            Some(&mut holder),
            256,
            true,
            &mut events
        ));
        assert_eq!(
            events,
            vec![
                FlightEvent::ShowHelpHint {
                    hint_index: 41,
                    state: 6,
                },
                FlightEvent::PrepareSound {
                    audio_site: MissionAudioSite::NotEnoughMana,
                    entity_index: 0,
                    player_index: 3,
                    sound_id: 29,
                },
            ]
        );

        caster.model = 1;
        events.clear();
        assert!(!activate_spell(
            &mut caster,
            Some(&mut holder),
            256,
            true,
            &mut events
        ));
        assert!(events.is_empty());
    }

    #[test]
    fn special_caster_forces_zero_direction_and_pointer_clear_matches_source_matrix() {
        for (control_mode, model, pointer_clears) in [
            (0, 1, true),
            (1, 1, true),
            (2, 1, false),
            (2, 3, true),
            (3, 3, false),
        ] {
            let mut caster = caster();
            caster.control_mode = control_mode;
            caster.model = u8::from(control_mode == 3);
            let mut holder = holder(model);

            assert!(activate_spell(
                &mut caster,
                Some(&mut holder),
                512,
                false,
                &mut Vec::new()
            ));
            let expected_direction = if caster.model == 1 { 0 } else { 512 };
            assert_eq!(caster.control_flags & DIRECTION_BITS, expected_direction);
            assert_eq!(
                caster.control_flags & POINTER_ACTION_BIT == 0,
                pointer_clears
            );
        }
    }
}
