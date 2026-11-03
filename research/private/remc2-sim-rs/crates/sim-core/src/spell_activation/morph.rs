//! Ordered scalar transitions for the acquired MC2 Morph holder.

/// Scalar authority consumed by one family-4/action-12 holder visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MorphSpellStepInput {
    pub active_value: i16,
    pub activation_value: i16,
    pub presentation_timer: u16,
    pub parent_valid: bool,
    pub source_gate_passed: bool,
    pub parent_mana_regen: i32,
    pub mana_cost: i32,
}

/// Ordered scalar result from remc2 `sub_6A030`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MorphSpellStepOutcome {
    pub active_value: i16,
    pub presentation_timer: u16,
    pub first_pulse: bool,
    pub award_experience: bool,
    pub request_creature: bool,
    pub parent_mana_regen: Option<i32>,
    pub expired: bool,
}

/// Advance one Morph holder without performing arena allocation or audio.
/// Allocation failure deliberately does not alter this scalar outcome: the
/// source awards experience, charges regeneration, and consumes the pulse
/// before it knows whether `NewEvent` found a slot.
#[must_use]
pub const fn step_morph_spell(input: MorphSpellStepInput) -> MorphSpellStepOutcome {
    let mut outcome = MorphSpellStepOutcome {
        active_value: input.active_value,
        presentation_timer: input.presentation_timer.saturating_sub(1),
        first_pulse: false,
        award_experience: false,
        request_creature: false,
        parent_mana_regen: None,
        expired: false,
    };
    if input.active_value <= 0 || !input.parent_valid {
        return outcome;
    }
    if input.source_gate_passed {
        outcome.first_pulse = input.active_value == input.activation_value;
        outcome.award_experience = outcome.first_pulse;
        outcome.request_creature = outcome.first_pulse;
        outcome.parent_mana_regen = if outcome.first_pulse {
            Some(if input.parent_mana_regen >= 0 {
                input.mana_cost.wrapping_neg()
            } else {
                input.parent_mana_regen.wrapping_sub(input.mana_cost)
            })
        } else if input.parent_mana_regen > 0 {
            Some(0)
        } else {
            None
        };
    } else {
        outcome.active_value = 1;
    }
    outcome.active_value = outcome.active_value.wrapping_sub(1);
    outcome.expired = outcome.active_value == 0;
    outcome
}

/// Source remote-owner flicker, before the active timer is decremented.
/// None retains the middle interval's existing visibility bits.
pub(crate) const fn remote_body_hidden(active: i16, duration: i16) -> Option<bool> {
    if active > duration.wrapping_sub(7) {
        Some((duration as u8).wrapping_sub(active as u8) & 1 != 0)
    } else if active < 7 {
        Some(active & 1 != 0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_parent_freezes_active_but_ticks_presentation_and_remote_flicker_is_ordered() {
        let outcome = step_morph_spell(MorphSpellStepInput {
            active_value: 201,
            activation_value: 201,
            presentation_timer: 2,
            parent_valid: false,
            source_gate_passed: false,
            parent_mana_regen: 0,
            mana_cost: 5000,
        });
        assert_eq!((outcome.active_value, outcome.presentation_timer), (201, 1));
        assert!(!outcome.expired && !outcome.request_creature);
        assert_eq!(remote_body_hidden(201, 201), Some(false));
        assert_eq!(remote_body_hidden(200, 201), Some(true));
        assert_eq!(remote_body_hidden(194, 201), None);
        assert_eq!(remote_body_hidden(6, 201), Some(false));
        assert_eq!(remote_body_hidden(1, 201), Some(true));
    }
}
