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

/// Narrow owner inputs for source directional spell constructors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectileSpellCastInput {
    pub aim_yaw: i16,
    pub aim_pitch: i16,
    pub auxiliary_byte: u8,
    pub castle_mana: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectileSpellHolderOutcome {
    pub clear_player_aux: Option<u16>,
    /// First successful product; additional linked shots remain in the arena.
    pub projectile: Option<u16>,
}

/// Renderer-free input for one `GetScroll_69DB0` Speed-holder visit.
///
/// The caller owns source-slot scheduling, player lookup, experience, audio,
/// and `NewEvent` allocation. This kernel owns only the ordered scalar writes
/// shared by the sealed Level 1 adapter and descriptor campaign sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedSpellStepInput {
    pub active_value: i16,
    pub activation_value: i16,
    pub holder_phase: u8,
    pub holder_control_flags: u32,
    pub presentation_timer: u16,
    pub parent_valid: bool,
    pub source_gate_passed: bool,
    pub speed_changed: bool,
    pub direction: i16,
    pub minimum_speed: i16,
    pub first_multiplier: i16,
    pub sustain_multiplier: i16,
    pub parent_mana_regen: i32,
    pub mana_cost: i32,
    /// Model-0 players clear the sound latch two active visits after launch;
    /// model-1 players use the source's 64-visit offset.
    pub latch_clear_offset: i16,
}

/// Ordered scalar result from one Speed-holder visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedSpellStepOutcome {
    pub active_value: i16,
    pub holder_control_flags: u32,
    pub presentation_timer: u16,
    pub requested_speed: Option<i16>,
    pub actual_speed: Option<i16>,
    pub parent_mana_regen: Option<i32>,
    pub prepare_sound_19: bool,
    pub award_experience: bool,
    pub request_trail: bool,
    pub expired: bool,
}

const SPEED_SOUND_LATCH: u32 = 0x80;

/// Apply the source `GetScroll_69DB0` scalar transition for one numeric
/// holder visit.
///
/// Allocation failure is intentionally outside this function: the source
/// still applies sound, speed, mana-regeneration, and countdown writes when
/// the optional class-10/model-2 constructor returns null.
#[must_use]
pub fn step_speed_spell(input: SpeedSpellStepInput) -> SpeedSpellStepOutcome {
    let mut outcome = SpeedSpellStepOutcome {
        active_value: input.active_value,
        holder_control_flags: input.holder_control_flags,
        presentation_timer: input.presentation_timer.saturating_sub(1),
        requested_speed: None,
        actual_speed: None,
        parent_mana_regen: None,
        prepare_sound_19: false,
        award_experience: false,
        request_trail: false,
        expired: false,
    };
    if input.active_value <= 0 || !input.parent_valid {
        return outcome;
    }

    if input.speed_changed {
        outcome.active_value = 1;
    } else if input.source_gate_passed {
        let first_active_tick = input.active_value == input.activation_value;
        if first_active_tick {
            if outcome.holder_control_flags & SPEED_SOUND_LATCH == 0 {
                outcome.holder_control_flags |= SPEED_SOUND_LATCH;
                outcome.prepare_sound_19 = true;
            }
            outcome.award_experience = true;
        }
        if input.active_value
            == input
                .activation_value
                .wrapping_sub(input.latch_clear_offset)
        {
            outcome.holder_control_flags &= !SPEED_SOUND_LATCH;
        }
        let multiplier = if first_active_tick {
            input.first_multiplier
        } else {
            input.sustain_multiplier
        };
        let speed = input
            .direction
            .wrapping_mul(input.minimum_speed)
            .wrapping_mul(multiplier);
        outcome.requested_speed = Some(speed);
        outcome.actual_speed = Some(speed);
        outcome.request_trail = input.holder_phase & 3 == 0;
        outcome.parent_mana_regen = if first_active_tick {
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
    }

    outcome.active_value = outcome.active_value.wrapping_sub(1);
    if outcome.active_value == 0 {
        let speed = input.direction.wrapping_mul(input.minimum_speed);
        outcome.requested_speed = Some(speed);
        outcome.actual_speed = Some(speed);
        outcome.holder_control_flags &= !SPEED_SOUND_LATCH;
        outcome.expired = true;
    }
    outcome
}

/// Result of one `CastSpeedSpell_31100` trail visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedTrailStepOutcome {
    pub life: i32,
    pub disable_drawing: bool,
}

/// Apply the signed class-10/model-2 trail lifetime transition.
///
/// The row is hidden only after the decrement crosses below `-1`; its owner
/// retains the disabled row until the following source-boundary reclaim.
#[must_use]
pub const fn step_speed_trail(life: i32) -> SpeedTrailStepOutcome {
    let life = life.wrapping_sub(1);
    SpeedTrailStepOutcome {
        life,
        disable_drawing: life < -1,
    }
}

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

/// The four source action roles used by the 26-family physical spell table.
///
/// `sub_692A0` passes `action - 1` to the common acquisition helper, while
/// `sub_69250` passes `action - 2` and creates a replacement persistent row.
/// Keeping the roles explicit prevents the Level 2 pickup seam from silently
/// reusing a Level 1 family-specific action constant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalSpellActionRole {
    Active,
    OrdinaryPickup,
    PersistentPickup,
    DetachedReturn,
}

/// Return the source action for one of remc2's 26 physical spell families.
///
/// This is the direct data-driven relation used by the source constructors:
/// active `3 * family`, ordinary pickup `3 * family + 1`, persistent pickup
/// `3 * family + 2`, and detached return `78`. The detached action is shared
/// by every family, but the family is still validated so unauthored values
/// cannot enter the generic Level 2 seam.
#[must_use]
pub const fn physical_spell_action(family: u8, role: PhysicalSpellActionRole) -> Option<u8> {
    if family >= 26 {
        return None;
    }
    let base = family.saturating_mul(3);
    Some(match role {
        PhysicalSpellActionRole::Active => base,
        PhysicalSpellActionRole::OrdinaryPickup => base.saturating_add(1),
        PhysicalSpellActionRole::PersistentPickup => base.saturating_add(2),
        PhysicalSpellActionRole::DetachedReturn => 78,
    })
}

/// Apply the upper-bound clamp performed by `SetSpell_6D5E0` to the holder
/// payload while retaining the raw non-negative player selector separately.
/// The source only clamps the value used to index `SPELLS.DAT`; the selected
/// hand/subspell record keeps the caller's raw selector.
#[must_use]
pub const fn level2_applied_tier(raw_tier: i8) -> Option<u8> {
    if raw_tier < 0 {
        None
    } else if raw_tier > 2 {
        Some(2)
    } else {
        Some(raw_tier as u8)
    }
}

/// The player-visible hand selected by the source acquisition helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellHand {
    Left,
    Right,
}

/// Mutable, renderer-free state for the generic Level 2 holder acquisition
/// transition. It mirrors only the source fields touched by `sub_68FF0` and
/// `sub_69250`/`sub_692A0`; active Shield/Invisible/Beyond Sight lifecycles
/// are implemented by their respective runtime handlers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpellHolderAcquisitionState {
    pub family: u8,
    pub source_slot: u16,
    /// Authored feature index which materialised this live source row. Zero
    /// denotes a reset-time source (for example family 12 at slot 38).
    pub authored_source_index: u16,
    pub owner_entity_index: u16,
    pub source_action: u8,
    pub holder_action: u8,
    pub replacement_action: Option<u8>,
    pub parent_entity_index: u16,
    /// Source `word_0x36_54`, set to 64 by `sub_68FF0`.
    pub presentation_timer: u16,
    /// Player `SpellEnabled[family]` relation.
    pub holder_relation: u16,
    /// Player `array_0x3B5` hand-side byte (1 = left, 2 = right).
    pub eligibility: u8,
    /// Player `array_0x403` learned/acquired byte at compact record 476+family,
    /// kept separate from the `array_0x3B5` hand-side projection.
    pub learned: u8,
    pub left_spell_slot: i16,
    pub right_spell_slot: i16,
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_tier: i8,
    /// Lossless `SetSpell_6D5E0` source projection. These fields mirror the
    /// authored entity bytes which are copied into the holder record when the
    /// pickup is acquired (subSpellIndex, word_0x30, byte_0x3B, and mana
    /// bookkeeping respectively).
    pub subspell_index: u16,
    pub word_0x30: u16,
    pub byte_0x3b: u8,
    pub byte_0x3c: u8,
    pub font_type_index: u8,
    pub mana_regen: i32,
    pub max_mana: i32,
    pub mana: i32,
    /// Source pickup life/cadence state consumed by `sub_68FF0` before the
    /// player scan.  Any nonzero value is decremented and suppresses the scan
    /// for that pulse; reaching zero disables drawing while retaining the
    /// source entity in the arena for the next source pulse.
    pub source_life: i32,
    pub source_draw_enabled: bool,
    /// Source `word_0x4A_74`: a non-zero value requests the return-side hand
    /// (1 = right, 2 = left) and is cleared by acquisition.
    pub return_side: i16,
}

impl SpellHolderAcquisitionState {
    #[must_use]
    pub const fn new(
        family: u8,
        source_slot: u16,
        owner_entity_index: u16,
        source_action: u8,
        selected_tier: i8,
    ) -> Self {
        Self {
            family,
            source_slot,
            authored_source_index: 0,
            owner_entity_index,
            source_action,
            holder_action: 0,
            replacement_action: None,
            parent_entity_index: 0,
            presentation_timer: 0,
            holder_relation: 0,
            eligibility: 0,
            learned: 0,
            left_spell_slot: -1,
            right_spell_slot: -1,
            left_subspell_index: 0,
            right_subspell_index: 0,
            selected_tier,
            subspell_index: 0,
            word_0x30: 0,
            byte_0x3b: 0,
            byte_0x3c: 0,
            font_type_index: 0,
            mana_regen: 0,
            max_mana: 0,
            mana: 0,
            source_life: 0,
            source_draw_enabled: true,
            return_side: 0,
        }
    }

    /// Apply the life/drawing prefix of `sub_68FF0`.  This intentionally
    /// leaves a zero-life source eligible for the following pulse, matching
    /// the source's `if (life) ... else scan` branch.
    pub fn step_source_life(&mut self) -> bool {
        if self.source_life != 0 {
            self.source_life -= 1;
            if self.source_life == 0 {
                self.source_draw_enabled = false;
            }
            return false;
        }
        true
    }
}

/// The source row projection consumed by `SetSpell_6D5E0`. Tier changes must
/// apply the selected rules rather than reuse frame-zero pickup bytes. The
/// focused family definitions below retain loaded owner SPELLS.DAT values
/// and applicable SetDefaultSpells overrides.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2SetSpellProfile {
    pub subspell_index: u16,
    pub word_0x30: u16,
    pub byte_0x3b: u8,
    pub mana_regen: i32,
    pub max_mana: i32,
    pub mana: i32,
}

/// Return the pinned `Spells.cpp` profile for a physical spell family/tier.
/// This is the table cost before GetSpellManaCost_6D710 follows the owner's
/// Castle relation. The runtime resolves that dynamic cost at SetSpell.
#[must_use]
pub const fn level2_set_spell_profile(family: u8, tier: u8) -> Option<Level2SetSpellProfile> {
    if family == 19 && tier < 3 {
        use crate::level_definition::{MC2_SOURCE_SUMMON_ARMY_PROPERTIES, SummonArmyTier};
        let source_tier = match SummonArmyTier::from_level_gate(tier as i8) {
            Some(value) => value,
            None => return None,
        };
        let rule = MC2_SOURCE_SUMMON_ARMY_PROPERTIES.tier_properties(source_tier);
        return Some(Level2SetSpellProfile {
            subspell_index: rule.power,
            word_0x30: rule.cooldown,
            byte_0x3b: 1,
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if family >= 22 && family <= 24 && tier < 3 {
        use crate::level_definition::late_spells::{
            ALLIANCE_PROPERTIES, FOOLS_MANA_PROPERTIES, MAGIC_MINE_PROPERTIES,
        };
        let properties = match family {
            22 => FOOLS_MANA_PROPERTIES,
            23 => MAGIC_MINE_PROPERTIES,
            _ => ALLIANCE_PROPERTIES,
        };
        let rule = properties.tiers[tier as usize];
        return Some(Level2SetSpellProfile {
            subspell_index: rule.subspell_index as u16,
            word_0x30: rule.cooldown as u16,
            byte_0x3b: ((rule.font_type & 1) == 0) as u8,
            mana_regen: rule.runtime_castle_mana_limit,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if (family == 20 || family == 21) && tier < 3 {
        let rule = if family == 20 {
            crate::level_definition::GRAVITY_WELL_TIERS[tier as usize]
        } else {
            crate::level_definition::WHIRLWIND_TIERS[tier as usize]
        };
        return Some(Level2SetSpellProfile {
            subspell_index: rule.power,
            word_0x30: rule.cooldown,
            byte_0x3b: 1,
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if (family == 13 || family == 14) && tier < 3 {
        let rule = if family == 13 {
            crate::level_definition::STEAL_MANA_TIERS[tier as usize]
        } else {
            crate::level_definition::DUEL_TIERS[tier as usize]
        };
        return Some(Level2SetSpellProfile {
            subspell_index: rule.power,
            word_0x30: rule.cooldown,
            byte_0x3b: 1,
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if family >= 15 && family <= 18 && tier < 3 {
        let properties = match family {
            15 => crate::level_definition::TREMOR_PROPERTIES,
            16 => crate::level_definition::CRATER_PROPERTIES,
            17 => crate::level_definition::EARTHQUAKE_PROPERTIES,
            _ => crate::level_definition::VOLCANO_PROPERTIES,
        };
        let rule = properties.tiers[tier as usize];
        return Some(Level2SetSpellProfile {
            subspell_index: rule.damage,
            word_0x30: rule.cooldown,
            byte_0x3b: 1,
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if family == 7 && tier < 3 {
        let rule = crate::level_definition::LIGHTNING_PROPERTIES.tiers[tier as usize];
        return Some(Level2SetSpellProfile {
            subspell_index: rule.damage,
            word_0x30: rule.cooldown,
            // SetDefaultSpells sets fontType bit0 for Lightning tier0.
            byte_0x3b: if tier == 0 { 0 } else { 1 },
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if family == 9 && tier < 3 {
        let rule = crate::level_definition::METEOR_PROPERTIES.tiers[tier as usize];
        return Some(Level2SetSpellProfile {
            subspell_index: rule.damage,
            word_0x30: rule.cooldown,
            byte_0x3b: 1,
            mana_regen: rule.castle_mana_required,
            max_mana: rule.mana_cost,
            mana: rule.mana_cost / rule.cooldown as i32,
        });
    }
    if family == 25 && tier < 3 {
        let tier = tier as usize;
        let cost = [11_000, 13_000, 26_000][tier];
        let duration = [31, 41, 51][tier];
        return Some(Level2SetSpellProfile {
            subspell_index: [100, 480, 1_200][tier],
            word_0x30: duration,
            byte_0x3b: 1,
            mana_regen: [100_000, 150_000, 250_000][tier],
            max_mana: cost,
            mana: cost / duration as i32,
        });
    }
    if let Some(profile) = support_spell_profile(family, tier) {
        return Some(profile);
    }
    let rows: &[[Level2SetSpellProfile; 3]] = &[
        [
            Level2SetSpellProfile {
                subspell_index: 250,
                word_0x30: 5,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 100,
                mana: 20,
            },
            Level2SetSpellProfile {
                subspell_index: 160,
                word_0x30: 11,
                byte_0x3b: 0,
                mana_regen: 0,
                max_mana: 250,
                mana: 22,
            },
            Level2SetSpellProfile {
                subspell_index: 80,
                word_0x30: 15,
                byte_0x3b: 1,
                mana_regen: 2_500,
                max_mana: 1_500,
                mana: 100,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 10,
                word_0x30: 3,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 100,
                mana: 33,
            },
            Level2SetSpellProfile {
                subspell_index: 15,
                word_0x30: 41,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 250,
                mana: 6,
            },
            Level2SetSpellProfile {
                subspell_index: 20,
                word_0x30: 51,
                byte_0x3b: 1,
                mana_regen: 1_000,
                max_mana: 1_000,
                mana: 19,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_000,
                mana: 9,
            },
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_250,
                mana: 12,
            },
            Level2SetSpellProfile {
                subspell_index: 1,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 2_000,
                mana: 19,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 2,
                word_0x30: 301,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_000,
                mana: 3,
            },
            Level2SetSpellProfile {
                subspell_index: 3,
                word_0x30: 451,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 2_500,
                mana: 5,
            },
            Level2SetSpellProfile {
                subspell_index: 4,
                word_0x30: 501,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 5_000,
                mana: 9,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 201,
                byte_0x3b: 1,
                mana_regen: 20_000,
                max_mana: 5_000,
                mana: 24,
            },
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 301,
                byte_0x3b: 1,
                mana_regen: 20_000,
                max_mana: 7_500,
                mana: 24,
            },
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 455,
                byte_0x3b: 1,
                mana_regen: 25_000,
                max_mana: 15_000,
                mana: 32,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 101,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_000,
                mana: 9,
            },
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 201,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_500,
                mana: 7,
            },
            Level2SetSpellProfile {
                subspell_index: 100,
                word_0x30: 301,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 10_000,
                mana: 33,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 181,
                byte_0x3b: 1,
                mana_regen: 2_000,
                max_mana: 2_000,
                mana: 11,
            },
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 183,
                byte_0x3b: 1,
                mana_regen: 3_000,
                max_mana: 2_500,
                mana: 13,
            },
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 183,
                byte_0x3b: 1,
                mana_regen: 4_000,
                max_mana: 3_000,
                mana: 16,
            },
        ],
        [
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 151,
                byte_0x3b: 1,
                mana_regen: 0,
                max_mana: 1_800,
                mana: 11,
            },
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 261,
                byte_0x3b: 1,
                mana_regen: 10_000,
                max_mana: 2_400,
                mana: 9,
            },
            Level2SetSpellProfile {
                subspell_index: 0,
                word_0x30: 361,
                byte_0x3b: 1,
                mana_regen: 10_000,
                max_mana: 5_000,
                mana: 13,
            },
        ],
    ];
    let row = match family {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        6 => 5,
        11 => 6,
        12 => 7,
        _ => return None,
    };
    if tier > 2 {
        return None;
    }
    Some(rows[row][tier as usize])
}

/// MC2 SPELLS.DAT defaults for the self-targeted support families. Numeric
/// identities stay at this compatibility boundary; behavior uses typed rules.
#[must_use]
pub const fn support_spell_profile(family: u8, tier: u8) -> Option<Level2SetSpellProfile> {
    if tier > 2 {
        return None;
    }
    let tier = tier as usize;
    let (power, duration, cost, castle) = match family {
        5 => (
            [1, 1, 10][tier],
            [11, 21, 41][tier],
            [500, 10_000, 50_000][tier],
            [0, 40_000, 160_000][tier],
        ),
        8 => (
            0,
            [125, 251, 125][tier],
            [5_000, 15_000, 25_000][tier],
            [40_000, 80_000, 160_000][tier],
        ),
        10 => (
            0,
            11,
            [5_000, 20_000, 40_000][tier],
            [5_000, 40_000, 60_000][tier],
        ),
        _ => return None,
    };
    Some(Level2SetSpellProfile {
        subspell_index: power,
        word_0x30: duration,
        byte_0x3b: 1,
        mana_regen: castle,
        max_mana: cost,
        mana: cost / duration as i32,
    })
}

#[must_use]
pub const fn support_spell_rule(
    family: u8,
    tier: u8,
    power: u16,
) -> Option<crate::spells::support::SupportSpellRule> {
    use crate::spells::support::{ReboundMode, SupportSpellRule, TeleportMode};
    if tier > 2 {
        return None;
    }
    match family {
        5 => Some(SupportSpellRule::Heal { percent: power }),
        8 => Some(SupportSpellRule::Rebound(if tier == 2 {
            ReboundMode::DoubleDamage
        } else {
            ReboundMode::Reflect
        })),
        10 => Some(SupportSpellRule::Teleport(match tier {
            0 => TeleportMode::Castle,
            1 => TeleportMode::Return,
            _ => TeleportMode::CycleCastles,
        })),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellHolderAcquisitionFailure {
    UnsupportedFamily,
    InvalidSourceSlot,
    InvalidOwner,
    InvalidSourceAction,
    InvalidTier,
    NoOverlap,
    AlreadyOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpellHolderAcquisitionOutcome {
    pub family: u8,
    pub source_slot: u16,
    pub holder_action: u8,
    pub replacement_action: Option<u8>,
    pub parent_entity_index: u16,
    pub presentation_timer: u16,
    pub hand: SpellHand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellHolderAcquisitionResult {
    Acquired(SpellHolderAcquisitionOutcome),
    Rejected(SpellHolderAcquisitionFailure),
}

/// Apply remc2's generic physical holder acquisition transition.
///
/// The source has no family-specific branches in `sub_68FF0`; every family
/// admitted by the pinned profile table uses the same relation/hand/timer
/// writes. The caller supplies the source overlap predicate because its
/// collision helper (`sub_106C0`) belongs to the owning entity dispatcher.
pub fn acquire_level2_spell_holder(
    state: &mut SpellHolderAcquisitionState,
    role: PhysicalSpellActionRole,
    overlaps: bool,
) -> SpellHolderAcquisitionResult {
    acquire_level2_spell_holder_with_return_side(state, role, overlaps, state.return_side)
}

/// `sub_68FF0` with its source return-side override made explicit for the
/// production entity dispatcher.  The compatibility wrapper above preserves
/// the original renderer-free test seam while this path carries
/// `word_0x4A_74` losslessly.
pub fn acquire_level2_spell_holder_with_return_side(
    state: &mut SpellHolderAcquisitionState,
    role: PhysicalSpellActionRole,
    overlaps: bool,
    return_side: i16,
) -> SpellHolderAcquisitionResult {
    if !matches!(state.family, 0..=25) {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::UnsupportedFamily,
        );
    }
    if state.source_slot == 0 {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::InvalidSourceSlot,
        );
    }
    let Some(applied_tier) = level2_applied_tier(state.selected_tier) else {
        return SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::InvalidTier);
    };
    if state.owner_entity_index == 0 {
        return SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::InvalidOwner);
    }
    let Some(expected_source_action) = physical_spell_action(state.family, role) else {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::UnsupportedFamily,
        );
    };
    if state.source_action != expected_source_action {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::InvalidSourceAction,
        );
    }
    if !overlaps {
        return SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::NoOverlap);
    }
    if state.holder_relation != 0 {
        return SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::AlreadyOwned);
    }

    let Some(profile) = level2_set_spell_profile(state.family, applied_tier) else {
        return SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::InvalidTier);
    };
    state.subspell_index = profile.subspell_index;
    state.word_0x30 = profile.word_0x30;
    state.byte_0x3b = profile.byte_0x3b;
    state.byte_0x3c = 0;
    state.font_type_index = 0;
    state.mana_regen = profile.mana_regen;
    state.max_mana = profile.max_mana;
    state.mana = profile.mana;

    // `sub_68FF0` writes the active action and owner relation first. The
    // persistent path then allocates an action-3f+2 replacement in source
    // order; allocation itself remains owned by the arena dispatcher.
    let holder_action = physical_spell_action(state.family, PhysicalSpellActionRole::Active)
        .expect("validated Level 2 family has an active action");
    state.source_action = holder_action;
    state.holder_action = holder_action;
    state.replacement_action =
        matches!(role, PhysicalSpellActionRole::PersistentPickup).then(|| {
            physical_spell_action(state.family, PhysicalSpellActionRole::PersistentPickup)
                .expect("validated Level 2 family has a persistent action")
        });
    state.parent_entity_index = state.owner_entity_index;
    state.presentation_timer = 64;
    state.holder_relation = state.source_slot;
    state.learned = 1;

    // Exact source hand order: a remembered return-side flag wins and is
    // consumed.  Otherwise choose left when it is empty or the right hand is
    // already occupied; otherwise choose right.  This deliberately preserves
    // the source's full-hand overwrite behavior.
    let hand = if return_side == 2
        || (return_side == 0 && (state.left_spell_slot < 0 || state.right_spell_slot >= 0))
    {
        state.left_spell_slot = i16::from(state.family);
        state.left_subspell_index = state.selected_tier;
        SpellHand::Left
    } else {
        state.right_spell_slot = i16::from(state.family);
        state.right_subspell_index = state.selected_tier;
        SpellHand::Right
    };
    state.eligibility = match hand {
        SpellHand::Left => 1,
        SpellHand::Right => 2,
    };
    state.return_side = 0;

    SpellHolderAcquisitionResult::Acquired(SpellHolderAcquisitionOutcome {
        family: state.family,
        source_slot: state.source_slot,
        holder_action,
        replacement_action: state.replacement_action,
        parent_entity_index: state.parent_entity_index,
        presentation_timer: state.presentation_timer,
        hand,
    })
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

/// Source family-6 (Shield) tier data from the pinned `Spells.cpp` table.
///
/// `word_0x18` is copied to the holder's `word_0x30_48` by `SetSpell_6D5E0`;
/// `life_0x1A` selects the two status-bit branches in `sub_6A480`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShieldTierProfile {
    pub mana_cost: i32,
    pub activation_ticks: i16,
    pub life_mode: u8,
    pub subspell_index: i32,
}

const SHIELD_TIERS: [ShieldTierProfile; 3] = [
    ShieldTierProfile {
        mana_cost: 1_000,
        activation_ticks: 101,
        life_mode: 0,
        subspell_index: 100,
    },
    ShieldTierProfile {
        mana_cost: 1_500,
        activation_ticks: 201,
        life_mode: 0,
        subspell_index: 100,
    },
    ShieldTierProfile {
        mana_cost: 10_000,
        activation_ticks: 301,
        life_mode: 1,
        subspell_index: 100,
    },
];

pub const SHIELD_FAMILY: u8 = 6;
pub const SHIELD_ACTIVE_ACTION: u8 = 18;
pub const SHIELD_PERSISTENT_ACTION: u8 = 20;
pub const SHIELD_MANA_STATUS_BIT: u32 = 0x0000_4000;
pub const SHIELD_INVULNERABLE_STATUS_BIT: u32 = 0x0040_0000;

#[must_use]
pub const fn shield_tier_profile(tier: u8) -> Option<ShieldTierProfile> {
    match tier {
        0..=2 => Some(SHIELD_TIERS[tier as usize]),
        _ => None,
    }
}

/// The renderer-free production seam for class-15/model-6 Shield action 18.
///
/// Holder acquisition is separate from this state machine.
/// Once a holder exists, this state owns the exact source countdown/status
/// transition (`sub_6A480`), and the damage consumer's two Shield branches.
/// `parent_status_flags` uses the source dword layout: byte 1 bit 6 is the
/// quarter-damage/mana-drain phase and byte 2 bit 6 is the full-invulnerability
/// phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShieldState {
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub tier: u8,
    pub action: u8,
    pub active_value: i16,
    pub activation_value: i16,
    pub mana_cost: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub parent_status_flags: u32,
    pub countdown: i16,
    pub experience_awarded: i32,
    pub cleanup_requested: bool,
}

impl ShieldState {
    /// Construct the source family-6 holder after `SetSpell_6D5E0`.
    #[must_use]
    pub fn new(
        tier: u8,
        holder_entity_index: u16,
        parent_entity_index: u16,
        parent_mana: i32,
        parent_life: i32,
    ) -> Option<Self> {
        let profile = shield_tier_profile(tier)?;
        Some(Self {
            holder_entity_index,
            parent_entity_index,
            tier,
            action: SHIELD_ACTIVE_ACTION,
            active_value: 0,
            activation_value: profile.activation_ticks,
            mana_cost: profile.mana_cost,
            parent_mana,
            parent_life,
            parent_mana_regen: 0,
            parent_status_flags: 0,
            // `sub_68FF0` installs the post-acquisition presentation cadence.
            countdown: 64,
            experience_awarded: 0,
            cleanup_requested: false,
        })
    }

    /// Activate through the shared `sub_5F660` reconstruction, retaining the
    /// source model-6 active/rearm and mana-gate behavior.
    pub fn activate(
        &mut self,
        direction_mask: u16,
        help_enabled: bool,
        events: &mut Vec<FlightEvent>,
    ) -> bool {
        let mut caster = SpellCasterState {
            entity_index: self.parent_entity_index,
            mana: self.parent_mana,
            control_flags: self.parent_status_flags,
            ..SpellCasterState::default()
        };
        let mut holder = SpellActivationHolder {
            entity_index: self.holder_entity_index,
            model: SHIELD_FAMILY,
            level_gate: self.tier as i8,
            active_value: self.active_value,
            activation_value: self.activation_value as u16,
            mana_cost: self.mana_cost,
            ..SpellActivationHolder::default()
        };
        let accepted = activate_spell(
            &mut caster,
            Some(&mut holder),
            direction_mask,
            help_enabled,
            events,
        );
        self.parent_mana = caster.mana;
        self.parent_status_flags = caster.control_flags & !0x3;
        self.active_value = holder.active_value;
        if accepted {
            self.action = SHIELD_ACTIVE_ACTION;
            self.cleanup_requested = false;
        }
        accepted
    }

    /// Advance one source `sub_6A480` pulse, including the unconditional
    /// `word_0x36_54` countdown decrement.
    pub fn step(&mut self) -> ShieldStep {
        let active_before = self.active_value;
        let status_before = self.parent_status_flags;
        let countdown_before = self.countdown;
        let profile = shield_tier_profile(self.tier).expect("Shield tier remains 0..=2");
        let mut sustained = false;

        if self.parent_entity_index != 0 && self.active_value > 0 {
            // `sub_68D50` only rechecks the parent's mana on the first pulse;
            // subsequent pulses are allowed to consume the active countdown.
            let mana_ok =
                self.active_value != self.activation_value || self.parent_mana >= self.mana_cost;
            if self.parent_life >= 0 && self.parent_mana >= 0 && mana_ok {
                sustained = true;
                if profile.life_mode == 0 {
                    self.parent_status_flags |= SHIELD_MANA_STATUS_BIT;
                } else if self.active_value == self.activation_value {
                    self.parent_status_flags |= SHIELD_INVULNERABLE_STATUS_BIT;
                }
                // sub_6A480 charges life-mode 0 before its decrement. Only
                // life-mode 1 decrements before calling sub_68DE0.
                if profile.life_mode == 1 {
                    self.active_value -= 1;
                }
                if self.active_value == self.activation_value {
                    self.parent_mana_regen = if self.parent_mana_regen >= 0 {
                        self.mana_cost.wrapping_neg()
                    } else {
                        self.parent_mana_regen.wrapping_sub(self.mana_cost)
                    };
                } else if self.active_value != 0 && self.parent_mana_regen > 0 {
                    self.parent_mana_regen = 0;
                }
                if profile.life_mode == 0 {
                    self.active_value -= 1;
                }
            } else {
                self.active_value = 0;
            }
        }

        if self.parent_entity_index != 0 && active_before > 0 && self.active_value == 0 {
            self.parent_status_flags &= !(SHIELD_MANA_STATUS_BIT | SHIELD_INVULNERABLE_STATUS_BIT);
            self.cleanup_requested = true;
        }
        if self.countdown > 0 {
            self.countdown -= 1;
        }
        ShieldStep {
            active_before,
            active_after: self.active_value,
            status_before,
            status_after: self.parent_status_flags,
            countdown_before,
            countdown_after: self.countdown,
            sustained,
            cleanup_requested: self.cleanup_requested,
        }
    }

    /// Apply the source damage consumer's Shield status branches.
    ///
    /// Callers should invoke this only after the source hit mailbox has
    /// identified a real incoming hit; non-positive values are a no-op. The
    /// mailbox/attacker identity remains owned by the mission dispatcher.
    pub fn absorb_damage(&mut self, raw_damage: i32) -> ShieldDamage {
        let status_before = self.parent_status_flags;
        let mut applied_damage = raw_damage.max(0);
        let mut mana_drained = 0;
        if applied_damage > 0
            && status_before & (SHIELD_MANA_STATUS_BIT | SHIELD_INVULNERABLE_STATUS_BIT) != 0
        {
            self.experience_awarded = self.experience_awarded.wrapping_add(1);
            if status_before & SHIELD_MANA_STATUS_BIT != 0 {
                mana_drained = applied_damage / 4;
                self.parent_mana = self.parent_mana.wrapping_sub(mana_drained);
                applied_damage = mana_drained;
                self.parent_status_flags &= !SHIELD_MANA_STATUS_BIT;
            } else {
                applied_damage = 0;
                self.parent_status_flags &= !SHIELD_INVULNERABLE_STATUS_BIT;
                self.parent_status_flags |= SHIELD_MANA_STATUS_BIT;
            }
        }
        self.parent_life = self.parent_life.wrapping_sub(applied_damage);
        ShieldDamage {
            raw_damage,
            applied_damage,
            mana_drained,
            experience_awarded: self.experience_awarded,
            status_before,
            status_after: self.parent_status_flags,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShieldStep {
    pub active_before: i16,
    pub active_after: i16,
    pub status_before: u32,
    pub status_after: u32,
    pub countdown_before: i16,
    pub countdown_after: i16,
    pub sustained: bool,
    pub cleanup_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShieldDamage {
    pub raw_damage: i32,
    pub applied_damage: i32,
    pub mana_drained: i32,
    pub experience_awarded: i32,
    pub status_before: u32,
    pub status_after: u32,
}

/// Source family-11 (Invisible) tier data from the pinned `Spells.cpp` table.
///
/// The three records are the family-11 rows at `Spells.cpp:15`: the holder
/// consumes 2,000/2,500/3,000 mana, its source countdown is 181/183/183
/// ticks, and `life_0x1A` writes 1/2/3 to the owner's visibility byte.  The
/// `mana_regen_requirement` is the source `maxManaLimit_A` value checked by
/// `sub_68D50` while an Invisible entity is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvisibleTierProfile {
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub activation_ticks: i16,
    pub owner_visibility_mode: u8,
    pub experience_award: i32,
}

const INVISIBLE_TIERS: [InvisibleTierProfile; 3] = [
    InvisibleTierProfile {
        mana_cost: 2_000,
        mana_regen_requirement: 2_000,
        activation_ticks: 181,
        owner_visibility_mode: 1,
        experience_award: 1,
    },
    InvisibleTierProfile {
        mana_cost: 2_500,
        mana_regen_requirement: 3_000,
        activation_ticks: 183,
        owner_visibility_mode: 2,
        experience_award: 1,
    },
    InvisibleTierProfile {
        mana_cost: 3_000,
        mana_regen_requirement: 4_000,
        activation_ticks: 183,
        owner_visibility_mode: 3,
        experience_award: 1,
    },
];

pub const INVISIBLE_FAMILY: u8 = 11;
pub const INVISIBLE_ACTIVE_ACTION: u8 = 33;
/// `struct_byte_0xc_12_15.byte[0]` bit set on the spell entity at first
/// activation.  The source clears this bit on the parent at termination,
/// which is intentionally represented separately below.
pub const INVISIBLE_STATUS_BIT: u32 = 0x20;

#[must_use]
pub const fn invisible_tier_profile(tier: u8) -> Option<InvisibleTierProfile> {
    match tier {
        0..=2 => Some(INVISIBLE_TIERS[tier as usize]),
        _ => None,
    }
}

/// Renderer-free state for source family-11/model-11 action 33
/// (`sub_6B1C0`, `AddSpell11_33_51300`).
///
/// The state deliberately keeps owner and spell-entity flags separate.  The
/// pinned source sets the spell entity's bit 0x20 when the first pulse starts,
/// but clears bit 0x20 on the owner when the pulse countdown terminates.  This
/// asymmetry is observable and must not be normalized away by a presentation
/// layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvisibleState {
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub tier: u8,
    pub action: u8,
    pub active_value: i16,
    pub activation_value: i16,
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub parent_status_flags: u32,
    pub spell_status_flags: u32,
    pub countdown: i16,
    pub experience_awarded: i32,
    pub cleanup_requested: bool,
}

impl InvisibleState {
    /// Construct an uncast family-11 holder after `SetSpell_6D5E0`.
    #[must_use]
    pub fn new(
        tier: u8,
        holder_entity_index: u16,
        parent_entity_index: u16,
        parent_mana: i32,
        parent_life: i32,
    ) -> Option<Self> {
        let profile = invisible_tier_profile(tier)?;
        Some(Self {
            holder_entity_index,
            parent_entity_index,
            tier,
            action: INVISIBLE_ACTIVE_ACTION,
            active_value: 0,
            activation_value: profile.activation_ticks,
            mana_cost: profile.mana_cost,
            mana_regen_requirement: profile.mana_regen_requirement,
            parent_mana,
            parent_life,
            parent_mana_regen: 0,
            parent_status_flags: 0,
            spell_status_flags: 0,
            // `sub_68FF0` installs the post-acquisition presentation cadence.
            countdown: 64,
            experience_awarded: 0,
            cleanup_requested: false,
        })
    }

    /// Activate through the shared `sub_5F660` reconstruction.
    pub fn activate(
        &mut self,
        direction_mask: u16,
        help_enabled: bool,
        events: &mut Vec<FlightEvent>,
    ) -> bool {
        let mut caster = SpellCasterState {
            entity_index: self.parent_entity_index,
            mana: self.parent_mana,
            control_flags: self.parent_status_flags,
            ..SpellCasterState::default()
        };
        let mut holder = SpellActivationHolder {
            entity_index: self.holder_entity_index,
            model: INVISIBLE_FAMILY,
            level_gate: self.tier as i8,
            active_value: self.active_value,
            activation_value: self.activation_value as u16,
            mana_cost: self.mana_cost,
            ..SpellActivationHolder::default()
        };
        let accepted = activate_spell(
            &mut caster,
            Some(&mut holder),
            direction_mask,
            help_enabled,
            events,
        );
        self.parent_mana = caster.mana;
        self.parent_status_flags = caster.control_flags & !0x3;
        self.active_value = holder.active_value;
        if accepted {
            self.action = INVISIBLE_ACTIVE_ACTION;
            self.cleanup_requested = false;
        }
        accepted
    }

    /// The source `sub_68D50` predicate.  `None` means that the parent has no
    /// valid castle entity; a non-zero `maxManaLimit_A` then fails closed just
    /// as the original `!CastleEntityIndex` branch does.
    #[must_use]
    pub fn authoritative_predicate(&self, parent_castle_mana: Option<i32>) -> bool {
        if self.parent_mana < 0 || self.parent_life < 0 {
            return false;
        }
        if self.mana_regen_requirement != 0 {
            let Some(castle_mana) = parent_castle_mana else {
                return false;
            };
            if self.mana_regen_requirement > castle_mana {
                return false;
            }
        }
        self.active_value != self.activation_value || self.parent_mana >= self.mana_cost
    }

    /// Advance one source `sub_6B1C0` pulse.  The caller supplies the current
    /// castle mana because the source predicate follows the parent's castle
    /// relation rather than reading a global wallet.
    pub fn step(&mut self, parent_castle_mana: Option<i32>) -> InvisibleStep {
        let active_before = self.active_value;
        let owner_flags_before = self.parent_status_flags;
        let spell_flags_before = self.spell_status_flags;
        let countdown_before = self.countdown;
        let mut sustained = false;
        let mut experience_awarded = 0;
        let mut special_state_write = None;

        if self.active_value > 0 {
            if self.parent_entity_index != 0 {
                if self.authoritative_predicate(parent_castle_mana) {
                    sustained = true;
                    if self.active_value == self.activation_value {
                        // `sub_6D8B0(parent, 0xB, 1)` awards Invisible
                        // experience exactly once per fresh activation.
                        let profile =
                            invisible_tier_profile(self.tier).expect("validated Invisible tier");
                        self.experience_awarded = self
                            .experience_awarded
                            .wrapping_add(profile.experience_award);
                        experience_awarded = profile.experience_award;
                        self.spell_status_flags |= INVISIBLE_STATUS_BIT;
                        // NewEvent leaves this nonplayer holder's A4 pointing
                        // at unk_F42B0x, not at its parent's player state.
                        special_state_write = Some(InvisibleSpecialWrite {
                            clear_protection: true,
                            visibility_mode: profile.owner_visibility_mode,
                        });
                    } else if self.spell_status_flags & INVISIBLE_STATUS_BIT == 0 {
                        // Source forces a one-tick cooldown if the spell entity
                        // never reached its initial visibility transition.
                        self.active_value = 1;
                    }
                    // `sub_68DE0`: charge the owner through its regen field;
                    // the source does not immediately subtract parent mana.
                    if self.active_value == self.activation_value {
                        self.parent_mana_regen = if self.parent_mana_regen >= 0 {
                            self.mana_cost.wrapping_neg()
                        } else {
                            self.parent_mana_regen.wrapping_sub(self.mana_cost)
                        };
                    } else if self.active_value != 0 && self.parent_mana_regen > 0 {
                        self.parent_mana_regen = 0;
                    }
                } else {
                    self.active_value = 1;
                }
            }

            self.active_value -= 1;
            if self.active_value == 0 {
                // Preserve the source asymmetry: termination clears the owner
                // bit and holder A4 life mode; the spell entity bit is left as-is
                // until the entity is removed by `sub_6D880`.
                if self.parent_entity_index != 0 {
                    self.parent_status_flags &= !INVISIBLE_STATUS_BIT;
                }
                special_state_write = Some(InvisibleSpecialWrite {
                    clear_protection: special_state_write
                        .is_some_and(|write| write.clear_protection),
                    visibility_mode: 0,
                });
                self.cleanup_requested = true;
            }
        }

        if self.countdown > 0 {
            self.countdown -= 1;
        }
        InvisibleStep {
            active_before,
            active_after: self.active_value,
            owner_flags_before,
            owner_flags_after: self.parent_status_flags,
            spell_flags_before,
            spell_flags_after: self.spell_status_flags,
            special_state_write,
            countdown_before,
            countdown_after: self.countdown,
            experience_awarded,
            sustained,
            cleanup_requested: self.cleanup_requested,
        }
    }

    /// Reset active state after a source-level entity removal/rebuild.
    pub fn reset(&mut self) {
        self.action = INVISIBLE_ACTIVE_ACTION;
        self.active_value = 0;
        self.parent_mana_regen = 0;
        self.parent_status_flags = 0;
        self.spell_status_flags = 0;
        self.countdown = 64;
        self.experience_awarded = 0;
        self.cleanup_requested = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvisibleSpecialWrite {
    pub clear_protection: bool,
    pub visibility_mode: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvisibleStep {
    pub active_before: i16,
    pub active_after: i16,
    pub owner_flags_before: u32,
    pub owner_flags_after: u32,
    pub spell_flags_before: u32,
    pub spell_flags_after: u32,
    /// Immediate write through the holder's A4. The caller resolves that
    /// pointer; this result is not a second persistent visibility authority.
    pub special_state_write: Option<InvisibleSpecialWrite>,
    pub countdown_before: i16,
    pub countdown_after: i16,
    pub experience_awarded: i32,
    pub sustained: bool,
    pub cleanup_requested: bool,
}

/// Source family-12 (Beyond Sight) tier data from the pinned `Spells.cpp`
/// table.  The active callback is deliberately separate from Invisible:
/// Beyond Sight does not set the Invisible parent/entity status bits, but it
/// does charge the same owner/castle predicate and awards family-12
/// experience on its first pulse (`sub_6B310`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeyondSightTierProfile {
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub activation_ticks: i16,
    pub experience_award: i32,
}

const BEYOND_SIGHT_TIERS: [BeyondSightTierProfile; 3] = [
    BeyondSightTierProfile {
        mana_cost: 1_800,
        mana_regen_requirement: 0,
        activation_ticks: 151,
        experience_award: 1,
    },
    BeyondSightTierProfile {
        mana_cost: 2_400,
        mana_regen_requirement: 10_000,
        activation_ticks: 261,
        experience_award: 1,
    },
    BeyondSightTierProfile {
        mana_cost: 5_000,
        mana_regen_requirement: 10_000,
        activation_ticks: 361,
        experience_award: 1,
    },
];

pub const BEYOND_SIGHT_FAMILY: u8 = 12;
pub const BEYOND_SIGHT_ACTIVE_ACTION: u8 = 36;
pub const BEYOND_SIGHT_XP_THRESHOLDS: [i32; 3] = [0, 360, 1_080];

#[must_use]
pub fn beyond_sight_unlocked_tier(total_experience: i32) -> i8 {
    BEYOND_SIGHT_XP_THRESHOLDS
        .iter()
        .rposition(|threshold| total_experience >= *threshold)
        .unwrap_or(0) as i8
}

#[must_use]
pub const fn beyond_sight_tier_profile(tier: u8) -> Option<BeyondSightTierProfile> {
    match tier {
        0..=2 => Some(BEYOND_SIGHT_TIERS[tier as usize]),
        _ => None,
    }
}

/// Renderer-free state for source class-15/family-12 action 36
/// (`sub_6B310`).  Class-10/model-74/action-72 is a separate source path and
/// is never a consumer or reveal effect of this state. This state owns the
/// exact holder, owner/castle predicate, charge, experience, countdown and
/// SetSpell cleanup transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeyondSightState {
    pub holder_entity_index: u16,
    pub parent_entity_index: u16,
    pub tier: u8,
    pub action: u8,
    pub active_value: i16,
    pub activation_value: i16,
    pub mana_cost: i32,
    pub mana_regen_requirement: i32,
    pub parent_mana: i32,
    pub parent_life: i32,
    pub parent_mana_regen: i32,
    pub countdown: i16,
    pub experience_awarded: i32,
    pub cleanup_requested: bool,
}

impl BeyondSightState {
    #[must_use]
    pub fn new(
        tier: u8,
        holder_entity_index: u16,
        parent_entity_index: u16,
        parent_mana: i32,
        parent_life: i32,
    ) -> Option<Self> {
        let profile = beyond_sight_tier_profile(tier)?;
        Some(Self {
            holder_entity_index,
            parent_entity_index,
            tier,
            action: BEYOND_SIGHT_ACTIVE_ACTION,
            active_value: 0,
            activation_value: profile.activation_ticks,
            mana_cost: profile.mana_cost,
            mana_regen_requirement: profile.mana_regen_requirement,
            parent_mana,
            parent_life,
            parent_mana_regen: 0,
            countdown: 64,
            experience_awarded: 0,
            cleanup_requested: false,
        })
    }

    pub fn activate(
        &mut self,
        direction_mask: u16,
        help_enabled: bool,
        events: &mut Vec<FlightEvent>,
    ) -> bool {
        let mut caster = SpellCasterState {
            entity_index: self.parent_entity_index,
            mana: self.parent_mana,
            ..SpellCasterState::default()
        };
        let mut holder = SpellActivationHolder {
            entity_index: self.holder_entity_index,
            model: BEYOND_SIGHT_FAMILY,
            level_gate: self.tier as i8,
            active_value: self.active_value,
            activation_value: self.activation_value as u16,
            mana_cost: self.mana_cost,
            ..SpellActivationHolder::default()
        };
        let accepted = activate_spell(
            &mut caster,
            Some(&mut holder),
            direction_mask,
            help_enabled,
            events,
        );
        self.parent_mana = caster.mana;
        self.active_value = holder.active_value;
        if accepted {
            self.action = BEYOND_SIGHT_ACTIVE_ACTION;
            self.cleanup_requested = false;
        }
        accepted
    }

    #[must_use]
    pub fn authoritative_predicate(&self, parent_castle_mana: Option<i32>) -> bool {
        if self.parent_mana < 0 || self.parent_life < 0 {
            return false;
        }
        if self.mana_regen_requirement != 0 {
            let Some(castle_mana) = parent_castle_mana else {
                return false;
            };
            if self.mana_regen_requirement > castle_mana {
                return false;
            }
        }
        self.active_value != self.activation_value || self.parent_mana >= self.mana_cost
    }

    /// Apply the source `sub_6D8B0(parent, 12, 1)` first-pulse award. The
    /// caller may publish the owning player's XP/level projection before the
    /// later `sub_68D50` predicate runs.
    pub fn award_experience_if_fresh_pulse(&mut self) -> i32 {
        if self.parent_entity_index == 0 || self.active_value != self.activation_value {
            return 0;
        }
        let profile = beyond_sight_tier_profile(self.tier).expect("validated Beyond Sight tier");
        self.experience_awarded = self
            .experience_awarded
            .wrapping_add(profile.experience_award);
        profile.experience_award
    }

    pub fn step(&mut self, parent_castle_mana: Option<i32>) -> BeyondSightStep {
        let experience_awarded = self.award_experience_if_fresh_pulse();
        self.step_after_experience_award(parent_castle_mana, experience_awarded)
    }

    /// Complete one source pulse after the ordered XP award has been applied.
    pub fn step_after_experience_award(
        &mut self,
        parent_castle_mana: Option<i32>,
        experience_awarded: i32,
    ) -> BeyondSightStep {
        let active_before = self.active_value;
        let countdown_before = self.countdown;
        let mut sustained = false;
        if self.active_value > 0 {
            if self.parent_entity_index != 0 {
                if self.authoritative_predicate(parent_castle_mana) {
                    sustained = true;
                    if self.active_value == self.activation_value {
                        self.parent_mana_regen = if self.parent_mana_regen >= 0 {
                            self.mana_cost.wrapping_neg()
                        } else {
                            self.parent_mana_regen.wrapping_sub(self.mana_cost)
                        };
                    } else if self.active_value != 0 && self.parent_mana_regen > 0 {
                        self.parent_mana_regen = 0;
                    }
                } else {
                    self.active_value = 1;
                }
            }
            self.active_value -= 1;
            if self.active_value == 0 {
                self.cleanup_requested = true;
            }
        }
        if self.countdown > 0 {
            self.countdown -= 1;
        }
        BeyondSightStep {
            active_before,
            active_after: self.active_value,
            countdown_before,
            countdown_after: self.countdown,
            experience_awarded,
            sustained,
            cleanup_requested: self.cleanup_requested,
        }
    }

    pub fn reset(&mut self) {
        self.action = BEYOND_SIGHT_ACTIVE_ACTION;
        self.active_value = 0;
        self.parent_mana_regen = 0;
        self.countdown = 64;
        self.experience_awarded = 0;
        self.cleanup_requested = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeyondSightStep {
    pub active_before: i16,
    pub active_after: i16,
    pub countdown_before: i16,
    pub countdown_after: i16,
    pub experience_awarded: i32,
    pub sustained: bool,
    pub cleanup_requested: bool,
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

    fn assert_source_audio_binding(tag: SourceAudioCallTag, expected_hash: &str) {
        assert_eq!(
            crate::SOURCE_AUDIO_CALL_BINDINGS
                .iter()
                .find(|(candidate, _)| *candidate == tag)
                .map(|(_, hash)| *hash),
            Some(expected_hash)
        );
    }

    #[test]
    fn morph_first_pulse_requests_day_bee_and_preserves_allocation_failure_asymmetry() {
        let outcome = step_morph_spell(MorphSpellStepInput {
            active_value: 201,
            activation_value: 201,
            presentation_timer: 64,
            parent_valid: true,
            source_gate_passed: true,
            parent_mana_regen: 100,
            mana_cost: 5_000,
        });

        assert_eq!(
            outcome,
            MorphSpellStepOutcome {
                active_value: 200,
                presentation_timer: 63,
                first_pulse: true,
                award_experience: true,
                request_creature: true,
                parent_mana_regen: Some(-5_000),
                expired: false,
            }
        );
    }

    #[test]
    fn morph_failed_source_gate_collapses_to_expiry_without_requesting_a_creature() {
        let outcome = step_morph_spell(MorphSpellStepInput {
            active_value: 201,
            activation_value: 201,
            presentation_timer: 64,
            parent_valid: true,
            source_gate_passed: false,
            parent_mana_regen: 100,
            mana_cost: 5_000,
        });

        assert_eq!(
            outcome,
            MorphSpellStepOutcome {
                active_value: 0,
                presentation_timer: 63,
                first_pulse: false,
                award_experience: false,
                request_creature: false,
                parent_mana_regen: None,
                expired: true,
            }
        );
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
        let caster_before = caster;
        let holder_before = holder;
        let mut events = Vec::new();

        // AUD-L2-113: EventsFunctions.cpp:60952, the active model-2
        // sub_5F660 branch prepares local-player sound 29 and returns before
        // the shared mana/help tail.
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
        assert_eq!(caster, caster_before);
        assert_eq!(holder, holder_before);
        assert_source_audio_binding(
            SourceAudioCallTag::NotEnoughManaImmediate,
            "467e5c361e5973cedc57a380dbcf17ea8f8be70f4ee3a5890529e88176a3c58c",
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
        let caster_before = caster;
        let holder_before = holder;
        let mut events = Vec::new();

        // AUD-L2-114: EventsFunctions.cpp:61007, the shared insufficient-mana
        // tail prepares the same local-player sound only after sub_88B60's
        // optional help notification.
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
        assert_eq!(caster, caster_before);
        assert_eq!(holder, holder_before);
        assert_source_audio_binding(
            SourceAudioCallTag::NotEnoughManaFallback,
            "d82a33b9eb1aeff3b62a746b4e8b2e1baf614d561f4b3037ac82d5117bef3e36",
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
    fn level2_mana_audio_negatives_reject_without_forbidden_emission() {
        let mut missing_holder_caster = caster();
        let missing_holder_before = missing_holder_caster;
        let mut missing_holder_events = Vec::new();
        assert!(!activate_spell(
            &mut missing_holder_caster,
            None,
            256,
            true,
            &mut missing_holder_events
        ));
        assert!(missing_holder_events.is_empty());
        assert_eq!(missing_holder_caster, missing_holder_before);

        // A cold holder with enough mana succeeds through the normal
        // activation transition; neither 113 nor 114 is admitted.
        let mut enough_mana_caster = caster();
        let mut cold_holder = holder(3);
        let cold_holder_before = cold_holder;
        let mut enough_mana_events = Vec::new();
        assert!(activate_spell(
            &mut enough_mana_caster,
            Some(&mut cold_holder),
            256,
            true,
            &mut enough_mana_events
        ));
        assert!(enough_mana_events.is_empty());
        assert_ne!(cold_holder, cold_holder_before);

        // The active model-1 retrigger is a direct-success branch and never
        // falls through to either forbidden mana sound.
        let mut active_model_one_caster = caster();
        active_model_one_caster.mana = 0;
        let mut active_model_one = holder(1);
        active_model_one.active_value = 4;
        let mut active_model_one_events = Vec::new();
        assert!(activate_spell(
            &mut active_model_one_caster,
            Some(&mut active_model_one),
            512,
            true,
            &mut active_model_one_events
        ));
        assert!(active_model_one_events.is_empty());

        // A model-2 immediate branch on the special caster is source-silent,
        // just as the fallback branch is; it must not consume AUD-L2-113.
        let mut special_caster = caster();
        special_caster.model = 1;
        let mut active_model_two = holder(2);
        active_model_two.active_value = 1;
        let mut special_events = Vec::new();
        assert!(!activate_spell(
            &mut special_caster,
            Some(&mut active_model_two),
            256,
            true,
            &mut special_events
        ));
        assert!(special_events.is_empty());
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

    #[test]
    fn shield_family6_source_tiers_pin_cost_life_and_activation_values() {
        assert_eq!(
            [0, 1, 2].map(|tier| shield_tier_profile(tier).expect("source Shield tier")),
            [
                ShieldTierProfile {
                    mana_cost: 1_000,
                    activation_ticks: 101,
                    life_mode: 0,
                    subspell_index: 100,
                },
                ShieldTierProfile {
                    mana_cost: 1_500,
                    activation_ticks: 201,
                    life_mode: 0,
                    subspell_index: 100,
                },
                ShieldTierProfile {
                    mana_cost: 10_000,
                    activation_ticks: 301,
                    life_mode: 1,
                    subspell_index: 100,
                },
            ]
        );
        assert!(shield_tier_profile(3).is_none());
    }

    #[test]
    fn shield_activation_uses_model6_mana_gate_and_source_direction() {
        let mut shield = ShieldState::new(0, 113, 2, 999, 100).expect("Shield tier");
        let mut events = Vec::new();
        assert!(!shield.activate(512, true, &mut events));
        assert_eq!(shield.active_value, 0);
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
                    player_index: 0,
                    sound_id: 29,
                },
            ]
        );

        shield.parent_mana = 1_000;
        events.clear();
        assert!(shield.activate(512, true, &mut events));
        assert_eq!(shield.active_value, 101);
        assert_eq!(shield.parent_status_flags & DIRECTION_BITS, 512);
        assert!(events.is_empty());
    }

    #[test]
    fn shield_tier0_pulses_quarter_damage_phase_then_cleans_up() {
        let mut shield = ShieldState::new(0, 113, 2, 1_000, 100).expect("Shield tier");
        shield.active_value = 2;
        shield.activation_value = 2;
        let first = shield.step();
        assert!(first.sustained);
        assert_eq!(first.active_after, 1);
        assert_eq!(
            first.status_after & SHIELD_MANA_STATUS_BIT,
            SHIELD_MANA_STATUS_BIT
        );
        assert_eq!(shield.countdown, 63);

        let damage = shield.absorb_damage(100);
        assert_eq!(damage.applied_damage, 25);
        assert_eq!(damage.mana_drained, 25);
        assert_eq!(damage.experience_awarded, 1);
        assert_eq!(damage.status_after & SHIELD_MANA_STATUS_BIT, 0);
        assert_eq!(shield.parent_life, 75);
        assert_eq!(shield.parent_mana, 975);

        let expired = shield.step();
        assert!(expired.cleanup_requested);
        assert_eq!(expired.active_after, 0);
        assert_eq!(
            expired.status_after & (SHIELD_MANA_STATUS_BIT | SHIELD_INVULNERABLE_STATUS_BIT),
            0
        );
        // Action-20 persistence/acquisition is owned by the separate holder
        // index; action-18 cleanup itself does not rewrite it.
        assert_eq!(shield.action, SHIELD_ACTIVE_ACTION);
    }

    #[test]
    fn shield_tier2_starts_full_invulnerability_then_transitions_to_mana_phase() {
        let mut shield = ShieldState::new(2, 113, 2, 10_000, 100).expect("Shield tier");
        shield.active_value = 2;
        shield.activation_value = 2;
        let first = shield.step();
        assert_eq!(
            first.status_after & SHIELD_INVULNERABLE_STATUS_BIT,
            SHIELD_INVULNERABLE_STATUS_BIT
        );
        let blocked = shield.absorb_damage(100);
        assert_eq!(blocked.applied_damage, 0);
        assert_eq!(shield.parent_life, 100);
        assert_eq!(
            blocked.status_after & SHIELD_MANA_STATUS_BIT,
            SHIELD_MANA_STATUS_BIT
        );

        let reduced = shield.absorb_damage(100);
        assert_eq!(reduced.applied_damage, 25);
        assert_eq!(reduced.mana_drained, 25);
        assert_eq!(shield.parent_life, 75);
    }

    #[test]
    fn shield_charge_order_preserves_each_source_life_mode() {
        for (tier, cost) in [(0, 1_000), (1, 1_500)] {
            for old_regen in [100, -250] {
                let mut shield = ShieldState::new(tier, 113, 2, 20_000, 100).unwrap();
                shield.active_value = shield.activation_value;
                shield.parent_mana_regen = old_regen;
                let first = shield.step();
                assert!(first.sustained);
                assert_eq!(first.active_after, shield.activation_value - 1);
                assert_eq!(shield.parent_mana_regen, old_regen.min(0) - cost);
                shield.parent_mana_regen = 100;
                shield.step();
                assert_eq!(shield.parent_mana_regen, 0);
            }
        }

        // The source's invulnerability branch calls sub_68DE0 after W--,
        // so changing the ordinary branches must retain its no-charge result.
        let mut shield = ShieldState::new(2, 113, 2, 20_000, 100).unwrap();
        shield.active_value = shield.activation_value;
        shield.parent_mana_regen = 100;
        shield.step();
        assert_eq!(shield.parent_mana_regen, 0);
    }

    #[test]
    fn shield_step_rejects_dead_or_missing_parent_without_leaking_status() {
        let mut dead = ShieldState::new(0, 113, 2, 1_000, -1).expect("Shield tier");
        dead.active_value = 3;
        dead.parent_status_flags = SHIELD_MANA_STATUS_BIT | SHIELD_INVULNERABLE_STATUS_BIT;
        let result = dead.step();
        assert!(!result.sustained);
        assert_eq!(result.active_after, 0);
        assert_eq!(
            result.status_after & (SHIELD_MANA_STATUS_BIT | SHIELD_INVULNERABLE_STATUS_BIT),
            0
        );

        let mut missing = ShieldState::new(0, 113, 0, 1_000, 100).expect("Shield tier");
        missing.active_value = 3;
        missing.parent_status_flags = SHIELD_MANA_STATUS_BIT;
        let result = missing.step();
        assert!(!result.sustained);
        assert_eq!(result.active_after, 3);
        assert_eq!(result.countdown_after, 63);
        assert_eq!(result.status_after, SHIELD_MANA_STATUS_BIT);
    }

    #[test]
    fn level_two_physical_spell_actions_match_source_formula() {
        assert_eq!(
            physical_spell_action(6, PhysicalSpellActionRole::Active),
            Some(18)
        );
        assert_eq!(
            physical_spell_action(6, PhysicalSpellActionRole::OrdinaryPickup),
            Some(19)
        );
        assert_eq!(
            physical_spell_action(11, PhysicalSpellActionRole::PersistentPickup),
            Some(35)
        );
        assert_eq!(
            physical_spell_action(12, PhysicalSpellActionRole::OrdinaryPickup),
            Some(37)
        );
        assert_eq!(
            physical_spell_action(12, PhysicalSpellActionRole::DetachedReturn),
            Some(78)
        );
        assert_eq!(
            physical_spell_action(26, PhysicalSpellActionRole::Active),
            None
        );
    }

    #[test]
    fn level_two_setspell_profiles_follow_pinned_spells_rows() {
        let expected = [
            (0, 0, 250, 5, 1, 0, 100, 20),
            (0, 1, 160, 11, 0, 0, 250, 22),
            (0, 2, 80, 15, 1, 2_500, 1_500, 100),
            (1, 0, 10, 3, 1, 0, 100, 33),
            (1, 1, 15, 41, 1, 0, 250, 6),
            (1, 2, 20, 51, 1, 1_000, 1_000, 19),
            (2, 0, 0, 101, 1, 0, 1_000, 9),
            (2, 1, 0, 101, 1, 0, 1_250, 12),
            (2, 2, 1, 101, 1, 0, 2_000, 19),
            (3, 0, 2, 301, 1, 0, 1_000, 3),
            (3, 1, 3, 451, 1, 0, 2_500, 5),
            (3, 2, 4, 501, 1, 0, 5_000, 9),
            (4, 0, 100, 201, 1, 20_000, 5_000, 24),
            (4, 1, 100, 301, 1, 20_000, 7_500, 24),
            (4, 2, 100, 455, 1, 25_000, 15_000, 32),
            (5, 0, 1, 11, 1, 0, 500, 45),
            (5, 1, 1, 21, 1, 40_000, 10_000, 476),
            (5, 2, 10, 41, 1, 160_000, 50_000, 1_219),
            (6, 0, 100, 101, 1, 0, 1_000, 9),
            (6, 1, 100, 201, 1, 0, 1_500, 7),
            (6, 2, 100, 301, 1, 0, 10_000, 33),
            (8, 0, 0, 125, 1, 40_000, 5_000, 40),
            (8, 1, 0, 251, 1, 80_000, 15_000, 59),
            (8, 2, 0, 125, 1, 160_000, 25_000, 200),
            // Loaded owner SPELLS.DAT, not the compiled fallback Spells.cpp.
            (7, 0, 200, 5, 0, 30_000, 1_000, 200),
            (7, 1, 300, 35, 1, 90_000, 10_000, 285),
            (7, 2, 800, 45, 1, 120_000, 20_000, 444),
            (9, 0, 4_000, 3, 1, 40_000, 6_000, 2_000),
            (9, 1, 8_000, 7, 1, 80_000, 10_000, 1_428),
            (9, 2, 16_000, 11, 1, 150_000, 20_000, 1_818),
            (13, 0, 2000, 5, 1, 10000, 10000, 2000),
            (13, 1, 4000, 7, 1, 16000, 20000, 2857),
            (13, 2, 10, 21, 1, 32000, 30000, 1428),
            (14, 0, 5170, 195, 1, 10000, 10000, 51),
            (14, 1, 7720, 395, 1, 16000, 20000, 50),
            (14, 2, 7720, 603, 1, 36000, 40000, 66),
            (15, 0, 200, 31, 1, 80_000, 4_000, 129),
            (15, 1, 300, 41, 1, 140_000, 6_000, 146),
            (15, 2, 800, 61, 1, 180_000, 10_000, 163),
            (16, 0, 250, 31, 1, 100_000, 6_000, 193),
            (16, 1, 400, 41, 1, 150_000, 9_000, 219),
            (16, 2, 900, 51, 1, 200_000, 12_000, 235),
            (17, 0, 300, 21, 1, 150_000, 10_000, 476),
            (17, 1, 500, 31, 1, 200_000, 12_000, 387),
            (17, 2, 1_000, 41, 1, 250_000, 15_000, 365),
            (18, 0, 400, 23, 1, 200_000, 12_000, 521),
            (18, 1, 800, 33, 1, 250_000, 15_000, 454),
            (18, 2, 1_200, 43, 1, 350_000, 18_000, 418),
            (19, 0, 100, 299, 1, 150_000, 10_000, 33),
            (19, 1, 100, 399, 1, 200_000, 20_000, 50),
            (19, 2, 100, 499, 1, 300_000, 40_000, 80),
            (20, 0, 750, 11, 1, 300_000, 15_000, 1_363),
            (20, 1, 1_500, 21, 1, 350_000, 20_000, 952),
            (20, 2, 3_000, 27, 1, 400_000, 30_000, 1_111),
            (21, 0, 100, 7, 1, 60_000, 2_000, 285),
            (21, 1, 180, 11, 1, 90_000, 2_500, 227),
            (21, 2, 240, 21, 1, 120_000, 5_000, 238),
            (22, 0, 500, 25, 1, 30_000, 15_000, 600),
            (22, 1, 1_000, 25, 1, 40_000, 20_000, 800),
            (22, 2, 2_000, 25, 1, 50_000, 25_000, 1_000),
            // SetDefaultSpells overrides family23's castle gate, retaining
            // the owner table's payload, cost and cooldown.
            (23, 0, 1_000, 47, 1, 50_000, 10_000, 212),
            (23, 1, 5_000, 67, 1, 70_000, 12_000, 179),
            (23, 2, 10_000, 87, 1, 90_000, 24_000, 275),
            (24, 0, 610, 23, 1, 200_000, 10_000, 434),
            (24, 1, 1_100, 29, 1, 300_000, 18_000, 620),
            (24, 2, 2_710, 63, 1, 400_000, 30_000, 476),
            (10, 0, 0, 11, 1, 5_000, 5_000, 454),
            (10, 1, 0, 11, 1, 40_000, 20_000, 1_818),
            (10, 2, 0, 11, 1, 60_000, 40_000, 3_636),
            (11, 0, 0, 181, 1, 2_000, 2_000, 11),
            (11, 1, 0, 183, 1, 3_000, 2_500, 13),
            (11, 2, 0, 183, 1, 4_000, 3_000, 16),
            (12, 0, 0, 151, 1, 0, 1_800, 11),
            (12, 1, 0, 261, 1, 10_000, 2_400, 9),
            (12, 2, 0, 361, 1, 10_000, 5_000, 13),
        ];
        for (family, tier, subspell_index, word_0x30, byte_0x3b, mana_regen, max_mana, mana) in
            expected
        {
            assert_eq!(
                level2_set_spell_profile(family, tier),
                Some(Level2SetSpellProfile {
                    subspell_index,
                    word_0x30,
                    byte_0x3b,
                    mana_regen,
                    max_mana,
                    mana,
                })
            );
        }
        assert_eq!(level2_set_spell_profile(6, 3), None);
        assert_eq!(level2_set_spell_profile(7, 3), None);
    }

    #[test]
    fn level_two_generic_acquisition_binds_beyond_sight_reset_row_and_hand() {
        // Reset census slot 38 is class-15/model-12/action-37, i.e. the
        // ordinary-pickup row for family 12.
        let mut state = SpellHolderAcquisitionState::new(12, 38, 111, 37, 2);
        let result =
            acquire_level2_spell_holder(&mut state, PhysicalSpellActionRole::OrdinaryPickup, true);
        let SpellHolderAcquisitionResult::Acquired(outcome) = result else {
            panic!("source row must acquire");
        };
        assert_eq!(outcome.holder_action, 36);
        assert_eq!(outcome.replacement_action, None);
        assert_eq!(outcome.parent_entity_index, 111);
        assert_eq!(outcome.presentation_timer, 64);
        assert_eq!(outcome.hand, SpellHand::Left);
        assert_eq!(state.source_action, 36);
        assert_eq!(state.holder_relation, 38);
        assert_eq!(state.eligibility, 1);
        assert_eq!(state.learned, 1);
        assert_eq!(state.left_spell_slot, 12);
        assert_eq!(state.left_subspell_index, 2);
    }

    #[test]
    fn level_two_persistent_acquisition_reports_replacement_and_right_hand() {
        let mut state = SpellHolderAcquisitionState::new(6, 200, 111, 20, 1);
        state.left_spell_slot = 3;
        state.right_spell_slot = -1;
        let result = acquire_level2_spell_holder(
            &mut state,
            PhysicalSpellActionRole::PersistentPickup,
            true,
        );
        let SpellHolderAcquisitionResult::Acquired(outcome) = result else {
            panic!("persistent source row must acquire");
        };
        assert_eq!(outcome.holder_action, 18);
        assert_eq!(outcome.replacement_action, Some(20));
        assert_eq!(outcome.hand, SpellHand::Right);
        assert_eq!(state.right_spell_slot, 6);
        assert_eq!(state.right_subspell_index, 1);
    }

    #[test]
    fn level_two_generic_acquisition_rejects_out_of_range_duplicate_and_no_overlap() {
        // The generic action relation admits all 26 physical source families;
        // the next family has no authored action or SetSpell profile.
        let mut out_of_range = SpellHolderAcquisitionState::new(26, 200, 111, 79, 0);
        let out_of_range_before = out_of_range;
        assert_eq!(
            acquire_level2_spell_holder(
                &mut out_of_range,
                PhysicalSpellActionRole::OrdinaryPickup,
                true,
            ),
            SpellHolderAcquisitionResult::Rejected(
                SpellHolderAcquisitionFailure::UnsupportedFamily
            )
        );
        assert_eq!(out_of_range, out_of_range_before);

        let mut no_overlap = SpellHolderAcquisitionState::new(11, 201, 111, 34, 0);
        assert_eq!(
            acquire_level2_spell_holder(
                &mut no_overlap,
                PhysicalSpellActionRole::OrdinaryPickup,
                false,
            ),
            SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::NoOverlap)
        );
        assert_eq!(no_overlap.holder_relation, 0);

        let mut duplicate = SpellHolderAcquisitionState::new(11, 202, 111, 34, 0);
        duplicate.holder_relation = 77;
        assert_eq!(
            acquire_level2_spell_holder(
                &mut duplicate,
                PhysicalSpellActionRole::OrdinaryPickup,
                true,
            ),
            SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::AlreadyOwned)
        );
        assert_eq!(duplicate.left_spell_slot, -1);
    }

    #[test]
    fn level_two_generic_acquisition_rejects_tampered_source_action_without_mutation() {
        let mut state = SpellHolderAcquisitionState::new(12, 38, 111, 38, 0);
        let before = state;
        assert_eq!(
            acquire_level2_spell_holder(&mut state, PhysicalSpellActionRole::OrdinaryPickup, true,),
            SpellHolderAcquisitionResult::Rejected(
                SpellHolderAcquisitionFailure::InvalidSourceAction
            )
        );
        assert_eq!(state, before);
    }

    #[test]
    fn level_two_generic_acquisition_rejects_invalid_tier_without_mutation() {
        let mut state = SpellHolderAcquisitionState::new(12, 38, 111, 37, -1);
        let before = state;
        assert_eq!(
            acquire_level2_spell_holder(&mut state, PhysicalSpellActionRole::OrdinaryPickup, true,),
            SpellHolderAcquisitionResult::Rejected(SpellHolderAcquisitionFailure::InvalidTier)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn source_life_treats_negative_values_as_nonzero_source_prefix() {
        let mut state = SpellHolderAcquisitionState::new(12, 38, 111, 37, 0);
        state.source_life = -1;
        assert!(!state.step_source_life());
        assert_eq!(state.source_life, -2);
        assert!(state.source_draw_enabled);
    }

    #[test]
    fn invisible_family11_source_tiers_pin_cost_timer_life_and_experience() {
        assert_eq!(
            [0, 1, 2].map(|tier| invisible_tier_profile(tier).expect("source Invisible tier")),
            [
                InvisibleTierProfile {
                    mana_cost: 2_000,
                    mana_regen_requirement: 2_000,
                    activation_ticks: 181,
                    owner_visibility_mode: 1,
                    experience_award: 1,
                },
                InvisibleTierProfile {
                    mana_cost: 2_500,
                    mana_regen_requirement: 3_000,
                    activation_ticks: 183,
                    owner_visibility_mode: 2,
                    experience_award: 1,
                },
                InvisibleTierProfile {
                    mana_cost: 3_000,
                    mana_regen_requirement: 4_000,
                    activation_ticks: 183,
                    owner_visibility_mode: 3,
                    experience_award: 1,
                },
            ]
        );
        assert!(invisible_tier_profile(3).is_none());
    }

    #[test]
    fn invisible_authoritative_predicate_matches_source_mana_life_castle_branches() {
        let mut invisible = InvisibleState::new(0, 513, 7, 2_000, 100).expect("tier");
        invisible.active_value = invisible.activation_value;
        assert!(invisible.authoritative_predicate(Some(2_000)));
        assert!(!invisible.authoritative_predicate(Some(1_999)));
        assert!(!invisible.authoritative_predicate(None));

        // The source only checks the parent's mana against maxMana on the
        // first pulse; subsequent pulses remain authoritative with less mana.
        invisible.active_value -= 1;
        invisible.parent_mana = 0;
        assert!(invisible.authoritative_predicate(Some(2_000)));

        invisible.parent_life = -1;
        assert!(!invisible.authoritative_predicate(Some(2_000)));
    }

    #[test]
    fn invisible_activation_obeys_family11_mana_gate_and_repeated_termination() {
        let mut invisible = InvisibleState::new(1, 513, 7, 2_499, 100).expect("tier");
        let mut events = Vec::new();
        assert!(!invisible.activate(256, true, &mut events));
        assert_eq!(invisible.active_value, 0);
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
                    player_index: 0,
                    sound_id: 29,
                },
            ]
        );

        invisible.parent_mana = 2_500;
        events.clear();
        assert!(invisible.activate(256, true, &mut events));
        assert_eq!(invisible.active_value, 183);
        assert!(events.is_empty());

        // A missing castle makes sub_68D50 fail at the authoritative first
        // pulse, forcing a one-tick cooldown and immediate cleanup.
        let expired = invisible.step(None);
        assert_eq!(expired.active_before, 183);
        assert_eq!(expired.active_after, 0);
        assert!(!expired.sustained);
        assert!(expired.cleanup_requested);
        assert_eq!(invisible.countdown, 63);

        // Repeated termination is idempotent: no second experience award or
        // status mutation is emitted while the removed entity is still inert.
        let repeated = invisible.step(None);
        assert_eq!(repeated.active_before, 0);
        assert_eq!(repeated.active_after, 0);
        assert_eq!(repeated.experience_awarded, 0);
        assert!(repeated.cleanup_requested);
        assert_eq!(invisible.countdown, 62);
    }

    #[test]
    fn invisible_first_pulse_sets_spell_flag_awards_experience_and_charges_regen() {
        let mut invisible = InvisibleState::new(2, 513, 7, 3_000, 100).expect("tier");
        invisible.active_value = invisible.activation_value;
        invisible.parent_mana_regen = 777;
        invisible.parent_status_flags = INVISIBLE_STATUS_BIT | 0x400;

        let first = invisible.step(Some(4_000));
        assert!(first.sustained);
        assert_eq!(first.active_after, 182);
        assert_eq!(first.experience_awarded, 1);
        assert_eq!(invisible.experience_awarded, 1);
        assert_eq!(
            first.spell_flags_after & INVISIBLE_STATUS_BIT,
            INVISIBLE_STATUS_BIT
        );
        assert_eq!(first.special_state_write.unwrap().visibility_mode, 3);
        assert_eq!(invisible.parent_mana_regen, -3_000);
        assert_eq!(invisible.countdown, 63);

        // Exhausting the short countdown clears only the owner bit.  The
        // spell-entity bit remains set until source `sub_6D880` removes it.
        invisible.active_value = 1;
        let end = invisible.step(Some(4_000));
        assert_eq!(end.active_after, 0);
        assert_eq!(end.owner_flags_after & INVISIBLE_STATUS_BIT, 0);
        assert_eq!(end.owner_flags_after & 0x400, 0x400);
        assert_eq!(
            end.spell_flags_after & INVISIBLE_STATUS_BIT,
            INVISIBLE_STATUS_BIT
        );
        assert_eq!(end.special_state_write.unwrap().visibility_mode, 0);
    }

    #[test]
    fn beyond_sight_first_pulse_matches_castle_gate_and_regen_charge() {
        let mut beyond_sight = BeyondSightState::new(1, 513, 7, 2_400, 100).expect("tier");
        beyond_sight.active_value = beyond_sight.activation_value;
        beyond_sight.parent_mana_regen = 777;
        assert!(!beyond_sight.authoritative_predicate(None));
        assert!(!beyond_sight.authoritative_predicate(Some(9_999)));
        assert!(beyond_sight.authoritative_predicate(Some(10_000)));

        let first = beyond_sight.step(Some(10_000));
        assert!(first.sustained);
        assert_eq!(first.active_after, 260);
        assert_eq!(first.experience_awarded, 1);
        assert_eq!(beyond_sight.experience_awarded, 1);
        assert_eq!(beyond_sight.parent_mana_regen, -2_400);
        assert_eq!(beyond_sight.countdown, 63);

        beyond_sight.active_value = 1;
        let end = beyond_sight.step(Some(10_000));
        assert_eq!(end.active_after, 0);
        assert!(end.cleanup_requested);
    }

    #[test]
    fn beyond_sight_first_pulse_awards_before_failed_predicate_without_downstream_effect() {
        let mut beyond_sight = BeyondSightState::new(1, 513, 7, 2_400, 100).expect("tier");
        beyond_sight.active_value = beyond_sight.activation_value;

        let first = beyond_sight.step(Some(9_999));

        assert!(!first.sustained);
        assert_eq!(first.experience_awarded, 1);
        assert_eq!(beyond_sight.experience_awarded, 1);
        assert_eq!(first.active_after, 0);
        assert!(first.cleanup_requested);
    }

    #[test]
    fn beyond_sight_xp_projection_uses_wrapping_total_and_exact_thresholds() {
        assert_eq!(beyond_sight_unlocked_tier(0), 0);
        assert_eq!(beyond_sight_unlocked_tier(359), 0);
        assert_eq!(beyond_sight_unlocked_tier(360), 1);
        assert_eq!(beyond_sight_unlocked_tier(1_079), 1);
        assert_eq!(beyond_sight_unlocked_tier(1_080), 2);
        assert_eq!(
            beyond_sight_unlocked_tier(i32::MAX.wrapping_add(1)),
            0,
            "the source i32 sum wraps before threshold selection"
        );
    }

    #[test]
    fn invisible_reset_restores_cold_holder_without_touching_identity_or_tier() {
        let mut invisible = InvisibleState::new(0, 513, 7, 2_000, 100).expect("tier");
        invisible.active_value = invisible.activation_value;
        let _ = invisible.step(Some(2_000));
        invisible.parent_status_flags |= 0x400;
        invisible.reset();

        assert_eq!(invisible.holder_entity_index, 513);
        assert_eq!(invisible.parent_entity_index, 7);
        assert_eq!(invisible.tier, 0);
        assert_eq!(invisible.action, INVISIBLE_ACTIVE_ACTION);
        assert_eq!(invisible.active_value, 0);
        assert_eq!(invisible.parent_mana_regen, 0);
        assert_eq!(invisible.parent_status_flags, 0);
        assert_eq!(invisible.spell_status_flags, 0);
        assert_eq!(invisible.countdown, 64);
        assert_eq!(invisible.experience_awarded, 0);
        assert!(!invisible.cleanup_requested);
    }
}
