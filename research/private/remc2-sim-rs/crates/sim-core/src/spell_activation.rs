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

/// The player-visible hand selected by the source acquisition helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellHand {
    Left,
    Right,
}

/// Mutable, renderer-free state for the generic Level 2 holder acquisition
/// transition. It mirrors only the source fields touched by `sub_68FF0` and
/// `sub_69250`/`sub_692A0`; active Shield/Invisible/Beyond Sight lifecycles
/// remain owned by their dedicated beads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpellHolderAcquisitionState {
    pub family: u8,
    pub source_slot: u16,
    pub owner_entity_index: u16,
    pub source_action: u8,
    pub holder_action: u8,
    pub replacement_action: Option<u8>,
    pub parent_entity_index: u16,
    /// Source `word_0x36_54`, set to 64 by `sub_68FF0`.
    pub presentation_timer: u16,
    /// Player `SpellEnabled[family]` relation.
    pub holder_relation: u16,
    /// Player `array_0x403_1027x.SpellIndex[family]` eligibility byte.
    pub eligibility: u8,
    pub left_spell_slot: i16,
    pub right_spell_slot: i16,
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_tier: i8,
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
            owner_entity_index,
            source_action,
            holder_action: 0,
            replacement_action: None,
            parent_entity_index: 0,
            presentation_timer: 0,
            holder_relation: 0,
            eligibility: 0,
            left_spell_slot: -1,
            right_spell_slot: -1,
            left_subspell_index: 0,
            right_subspell_index: 0,
            selected_tier,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellHolderAcquisitionFailure {
    UnsupportedFamily,
    InvalidSourceSlot,
    InvalidOwner,
    InvalidSourceAction,
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

/// Apply remc2's generic Level 2 physical holder acquisition transition.
///
/// The source has no family-specific branches in `sub_68FF0`: family 6,
/// 11, and 12 are admitted through the same relation/hand/timer writes as
/// the existing four families. This seam intentionally admits only those
/// three new Level 2 families and leaves the Level 1 paths untouched. The
/// caller supplies the source overlap predicate because its collision helper
/// (`sub_106C0`) belongs to the owning entity dispatcher.
pub fn acquire_level2_spell_holder(
    state: &mut SpellHolderAcquisitionState,
    role: PhysicalSpellActionRole,
    overlaps: bool,
) -> SpellHolderAcquisitionResult {
    if !matches!(state.family, 6 | 11 | 12) {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::UnsupportedFamily,
        );
    }
    if state.source_slot == 0 {
        return SpellHolderAcquisitionResult::Rejected(
            SpellHolderAcquisitionFailure::InvalidSourceSlot,
        );
    }
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
    state.eligibility = 1;

    // Exact source hand order: a remembered return-side flag is outside this
    // bounded state. For a fresh acquisition, choose left when it is empty or
    // the right hand is already occupied; otherwise choose right.
    let hand = if state.left_spell_slot < 0 || state.right_spell_slot >= 0 {
        state.left_spell_slot = i16::from(state.family);
        state.left_subspell_index = state.selected_tier;
        SpellHand::Left
    } else {
        state.right_spell_slot = i16::from(state.family);
        state.right_subspell_index = state.selected_tier;
        SpellHand::Right
    };

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
/// The holder is still acquired by the separate Level 2 spell-holder bead.
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
                self.active_value -= 1;
                // Preserve the post-decrement comparison/order of sub_68DE0.
                if self.active_value == self.activation_value {
                    self.parent_mana_regen = self.parent_mana_regen.wrapping_sub(self.mana_cost);
                } else if self.active_value != 0 && self.parent_mana_regen > 0 {
                    self.parent_mana_regen = 0;
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
    pub parent_visibility_mode: u8,
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
            parent_visibility_mode: 0,
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
        let visibility_before = self.parent_visibility_mode;
        let countdown_before = self.countdown;
        let mut sustained = false;
        let mut experience_awarded = 0;

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
                        self.parent_visibility_mode = profile.owner_visibility_mode;
                        self.spell_status_flags |= INVISIBLE_STATUS_BIT;
                    } else if self.spell_status_flags & INVISIBLE_STATUS_BIT == 0 {
                        // Source forces a one-tick cooldown if the spell entity
                        // never reached its initial visibility transition.
                        self.active_value = 1;
                    }
                    // `sub_68DE0`: charge the owner through its regen field;
                    // the source does not immediately subtract parent mana.
                    if self.active_value == self.activation_value {
                        self.parent_mana_regen = self
                            .parent_mana_regen
                            .checked_sub(self.mana_cost)
                            .unwrap_or(i32::MIN);
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
                // bit and owner life mode; the spell entity bit is left as-is
                // until the entity is removed by `sub_6D880`.
                if self.parent_entity_index != 0 {
                    self.parent_status_flags &= !INVISIBLE_STATUS_BIT;
                }
                self.parent_visibility_mode = 0;
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
            visibility_before,
            visibility_after: self.parent_visibility_mode,
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
        self.parent_visibility_mode = 0;
        self.countdown = 64;
        self.experience_awarded = 0;
        self.cleanup_requested = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvisibleStep {
    pub active_before: i16,
    pub active_after: i16,
    pub owner_flags_before: u32,
    pub owner_flags_after: u32,
    pub spell_flags_before: u32,
    pub spell_flags_after: u32,
    pub visibility_before: u8,
    pub visibility_after: u8,
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
        // indexing bead; action-18 cleanup itself does not rewrite it.
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
    fn level_two_generic_acquisition_rejects_unauthored_duplicate_and_no_overlap() {
        let mut unauthored = SpellHolderAcquisitionState::new(3, 200, 111, 10, 0);
        assert_eq!(
            acquire_level2_spell_holder(
                &mut unauthored,
                PhysicalSpellActionRole::OrdinaryPickup,
                true,
            ),
            SpellHolderAcquisitionResult::Rejected(
                SpellHolderAcquisitionFailure::UnsupportedFamily
            )
        );

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
        assert_eq!(first.visibility_after, 3);
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
        assert_eq!(end.visibility_after, 0);
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
        assert_eq!(invisible.parent_visibility_mode, 0);
        assert_eq!(invisible.countdown, 64);
        assert_eq!(invisible.experience_awarded, 0);
        assert!(!invisible.cleanup_requested);
    }
}
