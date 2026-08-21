use super::{
    BTreeSet, Digest, EntitySnapshot, FlightWorld, LEVEL2_NYPHUR_ENTITY_INDEX,
    LEVEL2_NYPHUR_SOURCE_SPELL_TIER_COUNT, LEVEL2_NYPHUR_SPELL_COOLDOWNS, Level2SetSpellProfile,
    MapType, MissionContentSession, NativeWorldError, PLAYER_SPELL_COUNT, PlayerSnapshot, Position,
    PresentationSelectors, Result, RuntimeRelation, RuntimeSnapshot, SessionTerrain, Sha256,
    SourceAudioCallTag, angle_distance, heading_for_positions, level2_applied_tier,
    level2_direct_starting_spell_profile, level2_nyphur_participant, level2_set_spell_profile,
    mark_source_audio_call, terrain_index,
};
#[cfg(test)]
use super::{Level2DispatchEvent, NativeWorld, PlayerCommand};

/// Pinned `sub_15790` spell-family order, including its zero-terminated tail
/// and the source's unsigned `0xff` sentinel. These rows are deliberately
/// separate from the later `sub_15910` order: the first helper has one
/// optional family-7 RNG branch before this scan.
const LEVEL2_NYPHUR_SPELL_SCAN_15790: [u8; 9] =
    [0x10, 0x12, 0x09, 0x07, 0x14, 0x15, 0x13, 0x00, 0xff];
const LEVEL2_NYPHUR_SPELL_SCAN_15910: [u8; 8] = [0x10, 0x12, 0x07, 0x09, 0x11, 0x14, 0x00, 0xff];

/// Pinned `unk_D3F91x` order consumed by `sub_15FC0`.  These are class-5
/// model buckets, not spell-family rows; the source maps their selected model
/// to a spell id on the family-4 holder after the nearest-candidate scan.
const LEVEL2_NYPHUR_SUB_15FC0_BUCKETS: [u8; 4] = [2, 0x13, 0x19, 0x10];
const LEVEL2_NYPHUR_MODE14_PRIMARY_MAX_DISTANCE: u32 = 0x1400;
const LEVEL2_NYPHUR_MODE14_ATTACK_MIN_DISTANCE: u32 = 0x0a00;

#[inline]
const fn level2_nyphur_mode14_primary_admitted(distance: u32) -> bool {
    distance <= LEVEL2_NYPHUR_MODE14_PRIMARY_MAX_DISTANCE
}

#[inline]
const fn level2_nyphur_mode14_bucket_admitted(distance: u32) -> bool {
    distance < LEVEL2_NYPHUR_MODE14_PRIMARY_MAX_DISTANCE
}

#[inline]
const fn level2_nyphur_mode14_attack_band(distance: u32) -> bool {
    distance > LEVEL2_NYPHUR_MODE14_ATTACK_MIN_DISTANCE
        && distance < LEVEL2_NYPHUR_MODE14_PRIMARY_MAX_DISTANCE
}

/// Source `Spells.cpp` family-4 rows used by `SetSpell_6D5E0` when
/// `sub_15FC0` selects one of the four bucket models.  The generic Level 2
/// profile table intentionally does not admit family 4 on the fresh route;
/// this narrow mode-14 projection is only reachable through a validated
/// source-owned family-4 holder.
const fn level2_nyphur_family4_spell_profile(tier: u8) -> Option<Level2SetSpellProfile> {
    Some(match tier {
        0 => Level2SetSpellProfile {
            subspell_index: 100,
            word_0x30: 201,
            byte_0x3b: 1,
            mana_regen: 20_000,
            max_mana: 5_000,
            mana: 24,
        },
        1 => Level2SetSpellProfile {
            subspell_index: 100,
            word_0x30: 301,
            byte_0x3b: 1,
            mana_regen: 20_000,
            max_mana: 7_500,
            mana: 24,
        },
        2 => Level2SetSpellProfile {
            subspell_index: 100,
            word_0x30: 455,
            byte_0x3b: 1,
            mana_regen: 25_000,
            max_mana: 15_000,
            mana: 32,
        },
        _ => return None,
    })
}

/// Source-owned Level 2 AI participant/holder chain at reset.
///
/// The participant rows remain part of the lossless entity/player arrays. This
/// typed view only exposes the constructor relations needed by later AI and
/// presentation consumers, keeping those consumers from re-decoding raw
/// records or silently accepting a cross-player holder chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2NyphurParticipant {
    pub player_slot: u8,
    pub entity_index: u16,
    pub is_ai: bool,
    pub color_index: u8,
    pub entity_class: u8,
    pub entity_model: u8,
    pub entity_action: u8,
    pub entity_source_id: u16,
    pub entity_next_relation: Option<u16>,
    pub entity_next_entity: u16,
    pub entity_control_flags: u32,
    pub entity_life: i32,
    pub entity_max_life: i32,
    pub entity_mana: i32,
    pub entity_max_mana: i32,
    pub fireball_holder_index: u16,
    pub fireball_action: u8,
    pub fireball_parent_entity_index: u16,
    pub fireball_next_entity: u16,
    pub possession_holder_index: u16,
    pub possession_action: u8,
    pub possession_parent_entity_index: u16,
    pub possession_next_entity: u16,
    pub objective_stage_relation: RuntimeRelation,
}

/// The mode-specific helper selected by `sub_12910`'s one-byte state field.
///
/// Keep the source symbol on each variant: the later mode slices implement the
/// helper bodies, while this dispatcher owns the exact selection and common
/// tail order.  The two memory helpers are distinct source calls even though
/// both are weak allocation-failure hooks in the pinned C++ source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level2NyphurDispatchHelper {
    Sub12ff0,
    Nmemneed,
    Sub13100,
    Sub131f0,
    Nmemneed0,
    Sub135c0,
    Sub13710,
    Sub13830,
    Sub13870,
    Sub133b0,
    Sub13270,
    Sub13850,
    Sub161a0,
}

/// One call in the pinned `sub_12910` sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level2NyphurDispatchCall {
    Sub12a70,
    Helper(Level2NyphurDispatchHelper),
    Sub12e70,
}

impl Level2NyphurDispatchCall {
    #[must_use]
    pub const fn hash_tag(self) -> u8 {
        match self {
            Self::Sub12a70 => 1,
            Self::Helper(helper) => match helper {
                Level2NyphurDispatchHelper::Sub12ff0 => 2,
                Level2NyphurDispatchHelper::Nmemneed => 3,
                Level2NyphurDispatchHelper::Sub13100 => 4,
                Level2NyphurDispatchHelper::Sub131f0 => 5,
                Level2NyphurDispatchHelper::Nmemneed0 => 6,
                Level2NyphurDispatchHelper::Sub135c0 => 7,
                Level2NyphurDispatchHelper::Sub13710 => 8,
                Level2NyphurDispatchHelper::Sub13830 => 9,
                Level2NyphurDispatchHelper::Sub13870 => 10,
                Level2NyphurDispatchHelper::Sub133b0 => 11,
                Level2NyphurDispatchHelper::Sub13270 => 12,
                Level2NyphurDispatchHelper::Sub13850 => 13,
                Level2NyphurDispatchHelper::Sub161a0 => 14,
            },
            Self::Sub12e70 => 15,
        }
    }
}

/// Observable call order for one production `sub_12910` dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2NyphurDispatchTrace {
    pub pass: u8,
    pub source_slot: u16,
    /// Source `Type_str_164::byte_0x1C1_449`, preserved at signed one-byte
    /// width.  The owner package's model-1 reset writes this byte to zero.
    pub state_byte: i8,
    pub calls: Box<[Level2NyphurDispatchCall]>,
}

impl Level2NyphurDispatchTrace {
    #[must_use]
    pub fn tail_call_count(&self) -> usize {
        self.calls
            .iter()
            .filter(|call| matches!(call, Level2NyphurDispatchCall::Sub12e70))
            .count()
    }
}

/// The source-owned mutable fields consumed by `sub_12A70`.
///
/// `Type_str_164` is not serialized as a standalone owner-package member.
/// These fields therefore live beside, rather than inside, the lossless
/// entity/player records.  Their reset values come from the qualified AI
/// player and `MissionWizardMapSettings`; the entity remains authoritative for
/// the source position/life/mana projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Level2NyphurCommonUpkeep {
    player_slot: u8,
    player_color_index: u8,
    pub(super) source_slot: u16,
    source_list_model_zero: Option<u16>,
    castle_entity_index: u16,
    speed: i16,
    strafe_speed: i16,
    word_0xe_14: i16,
    byte_0x45c: i8,
    byte_0x45d: i8,
    byte_0x45e: u8,
    move_boost: i16,
    move_speed: u8,
    move_speed_flag: u8,
    player_hit_frame_time: u8,
    dword_0x18d: i32,
    palette_sub_mod: u8,
    palette_count: u16,
    palette_modification_enabled: bool,
    contact_target_entity: Option<u16>,
    contact_distance: u32,
    contact_word_0x14a: i16,
    yaw_0x1e_30: i16,
    fov_0x22_34: i16,
    word_0x1a2: i16,
    /// Source `Type_str_164::word_0x1A4_420`, the mana-starvation latch
    /// shared by `sub_15790` and `sub_15910`.
    word_0x1a4: i16,
    byte_0x154: u8,
    word_0x159: i16,
    life_regen: i16,
    dword_0x16d: i32,
    source_mailbox: [u8; 36],
    /// The source declaration is exactly `int16_t[32]` at 0x1fc. The final
    /// threshold guard reads two words from the six bytes of compiler
    /// padding before word 0x242; carry those bytes separately instead of
    /// inventing two array members.
    array_0x1fc: [u16; 32],
    padding_0x23c: [u8; 6],
    word_0x242: i16,
    word_0x244: i16,
    word_0x246: i16,
    word_0x248: i16,
    word_0x24a: i16,
    word_0x24c: i16,
    spell_enabled: [i16; PLAYER_SPELL_COUNT],
    spell_holder_indices: [u16; PLAYER_SPELL_COUNT],
    spell_levels: [i8; PLAYER_SPELL_COUNT],
    owner_control_mode: u8,
    mana_regen: i32,
    mana: i32,
    life: i32,
    max_mana: i32,
    max_life: i32,
    high_upkeep: bool,
    map_angle_allowed: bool,
    cadence_eligible: bool,
    cadence_candidate: Option<u16>,
    cadence_action: u8,
    return_value: i8,
    /// The pinned Watcom `watcomrand` stream is process-global in remc2.  A
    /// session-local carrier keeps its source draw order while preserving the
    /// Rust session's reset and replay isolation.
    watcom_rand_state: u32,
}

/// Source `sub_12A70` state made available to the next AI leaves and to
/// deterministic Rust tests.  The arrays retain their source widths and
/// ordering; no local-player projection is used for AI-owned values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2NyphurCommonUpkeepSnapshot {
    pub player_slot: u8,
    pub player_color_index: u8,
    pub source_slot: u16,
    pub source_list_model_zero: Option<u16>,
    pub castle_entity_index: u16,
    pub phase: u8,
    pub speed: i16,
    pub strafe_speed: i16,
    pub word_0xe_14: i16,
    pub byte_0x45c: i8,
    pub byte_0x45d: i8,
    pub byte_0x45e: u8,
    pub move_boost: i16,
    pub move_speed: u8,
    pub move_speed_flag: u8,
    pub player_hit_frame_time: u8,
    pub dword_0x18d: i32,
    pub palette_sub_mod: u8,
    pub palette_count: u16,
    pub palette_modification_enabled: bool,
    pub contact_target_entity: Option<u16>,
    pub contact_distance: u32,
    pub contact_word_0x14a: i16,
    pub yaw_0x1e_30: i16,
    pub fov_0x22_34: i16,
    pub word_0x1a2: i16,
    /// Source `Type_str_164::word_0x1A4_420`, the shared mana-starvation
    /// latch consumed by the two source spell selectors.
    pub word_0x1a4: i16,
    pub byte_0x154: u8,
    pub word_0x159: i16,
    pub life_regen: i16,
    pub dword_0x16d: i32,
    pub source_mailbox: [u8; 36],
    pub array_0x1fc: [u16; 32],
    pub padding_0x23c: [u8; 6],
    pub word_0x242: i16,
    pub word_0x244: i16,
    pub word_0x246: i16,
    pub word_0x248: i16,
    pub word_0x24a: i16,
    pub word_0x24c: i16,
    pub spell_enabled: [i16; PLAYER_SPELL_COUNT],
    pub spell_holder_indices: [u16; PLAYER_SPELL_COUNT],
    pub spell_levels: [i8; PLAYER_SPELL_COUNT],
    pub owner_control_mode: u8,
    pub mana_regen: i32,
    pub mana: i32,
    pub life: i32,
    pub max_mana: i32,
    pub max_life: i32,
    pub high_upkeep: bool,
    pub map_angle_allowed: bool,
    pub cadence_eligible: bool,
    pub cadence_candidate: Option<u16>,
    pub cadence_action: u8,
    pub return_value: i8,
}

impl Level2NyphurCommonUpkeep {
    fn from_qualified_reset(
        participant: Level2NyphurParticipant,
        players: &[PlayerSnapshot],
        entities: &[EntitySnapshot],
        mission: &MissionContentSession,
    ) -> Result<Self> {
        let settings = mission
            .wizard_map_settings()
            .get(usize::from(participant.color_index))
            .ok_or_else(|| {
                NativeWorldError::new(format!(
                    "Level 2 Nyphur wizard settings row {} is absent",
                    participant.color_index
                ))
            })?;
        let player = players
            .get(usize::from(participant.player_slot))
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur AI player is absent"))?;
        let entity = entities
            .get(usize::from(participant.entity_index))
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur AI entity is absent"))?;
        let auxiliary = player.type_str_164_auxiliary().ok_or_else(|| {
            NativeWorldError::new("Level 2 Nyphur AI player Type_str_164 carrier is absent")
        })?;
        let mut array_0x1fc = auxiliary.array_0x1FC_508;
        let mut padding_0x23c = [0_u8; 6];
        for index in 0..8 {
            let threshold = 4 * index + 4;
            if threshold < array_0x1fc.len() {
                // Model-1 reset writes the source sentinel only when the
                // captured owner row has not already supplied that slot.
                if array_0x1fc[threshold] == 0 {
                    array_0x1fc[threshold] = 24_607;
                }
            } else {
                // The eighth logical threshold is array index 32, which is
                // the first uint16 in the separate Watcom padding boundary.
                padding_0x23c[..2].copy_from_slice(&24_607_u16.to_le_bytes());
            }
        }
        let mut spell_enabled = [0_i16; PLAYER_SPELL_COUNT];
        spell_enabled[2] = i16::from(participant.color_index).wrapping_mul(4);
        let spell_holder_indices = player.enabled_spell_holders;
        let spell_levels = settings.spell_levels.map(u8::cast_signed);
        let castle_entity_index = entities
            .iter()
            .find(|candidate| {
                candidate.active
                    && candidate.class == 2
                    && candidate.source_id == participant.entity_source_id
            })
            .map_or(0, |candidate| candidate.index);
        let mut source_mailbox = [0_u8; 36];
        source_mailbox.copy_from_slice(entity.source_upkeep_mailbox());
        Ok(Self {
            player_slot: player.slot,
            player_color_index: participant.color_index,
            source_slot: participant.entity_index,
            source_list_model_zero: level2_nyphur_class3_source_order(entities)
                .into_iter()
                .filter(|index| entities[usize::from(*index)].model == 0)
                .last(),
            castle_entity_index,
            speed: 0,
            strafe_speed: 0,
            word_0xe_14: 0,
            byte_0x45c: 0,
            byte_0x45d: 0,
            byte_0x45e: 0,
            move_boost: 0,
            move_speed: 0,
            move_speed_flag: 0,
            player_hit_frame_time: 0,
            dword_0x18d: 0,
            palette_sub_mod: 0,
            palette_count: 256,
            palette_modification_enabled: false,
            contact_target_entity: None,
            contact_distance: 0,
            contact_word_0x14a: 0,
            yaw_0x1e_30: 0,
            fov_0x22_34: 0,
            word_0x1a2: auxiliary.word_0x1A2_418,
            word_0x1a4: 0,
            byte_0x154: auxiliary.byte_0x154_340,
            word_0x159: 100,
            life_regen: 0,
            dword_0x16d: auxiliary.dword_0x16D_365,
            source_mailbox,
            array_0x1fc,
            padding_0x23c,
            word_0x242: auxiliary.word_0x242_578,
            word_0x244: settings.perception,
            word_0x246: settings.reflexes,
            word_0x248: 0,
            word_0x24a: if settings.life != 0 {
                settings.life
            } else {
                256
            },
            word_0x24c: 0,
            spell_enabled,
            spell_holder_indices,
            spell_levels,
            owner_control_mode: player.record_bytes()[447],
            mana_regen: entity.mana_regen,
            mana: entity.mana,
            life: entity.life,
            max_mana: entity.max_mana,
            max_life: entity.max_life,
            high_upkeep: false,
            map_angle_allowed: false,
            cadence_eligible: false,
            cadence_candidate: None,
            cadence_action: 0,
            return_value: 0,
            watcom_rand_state: 1,
        })
    }

    fn snapshot(&self, phase: u8) -> Level2NyphurCommonUpkeepSnapshot {
        Level2NyphurCommonUpkeepSnapshot {
            player_slot: self.player_slot,
            player_color_index: self.player_color_index,
            source_slot: self.source_slot,
            source_list_model_zero: self.source_list_model_zero,
            castle_entity_index: self.castle_entity_index,
            phase,
            speed: self.speed,
            strafe_speed: self.strafe_speed,
            word_0xe_14: self.word_0xe_14,
            byte_0x45c: self.byte_0x45c,
            byte_0x45d: self.byte_0x45d,
            byte_0x45e: self.byte_0x45e,
            move_boost: self.move_boost,
            move_speed: self.move_speed,
            move_speed_flag: self.move_speed_flag,
            player_hit_frame_time: self.player_hit_frame_time,
            dword_0x18d: self.dword_0x18d,
            palette_sub_mod: self.palette_sub_mod,
            palette_count: self.palette_count,
            palette_modification_enabled: self.palette_modification_enabled,
            contact_target_entity: self.contact_target_entity,
            contact_distance: self.contact_distance,
            contact_word_0x14a: self.contact_word_0x14a,
            yaw_0x1e_30: self.yaw_0x1e_30,
            fov_0x22_34: self.fov_0x22_34,
            word_0x1a2: self.word_0x1a2,
            word_0x1a4: self.word_0x1a4,
            byte_0x154: self.byte_0x154,
            word_0x159: self.word_0x159,
            life_regen: self.life_regen,
            dword_0x16d: self.dword_0x16d,
            source_mailbox: self.source_mailbox,
            array_0x1fc: self.array_0x1fc,
            padding_0x23c: self.padding_0x23c,
            word_0x242: self.word_0x242,
            word_0x244: self.word_0x244,
            word_0x246: self.word_0x246,
            word_0x248: self.word_0x248,
            word_0x24a: self.word_0x24a,
            word_0x24c: self.word_0x24c,
            spell_enabled: self.spell_enabled,
            spell_holder_indices: self.spell_holder_indices,
            spell_levels: self.spell_levels,
            owner_control_mode: self.owner_control_mode,
            mana_regen: self.mana_regen,
            mana: self.mana,
            life: self.life,
            max_mana: self.max_mana,
            max_life: self.max_life,
            high_upkeep: self.high_upkeep,
            map_angle_allowed: self.map_angle_allowed,
            cadence_eligible: self.cadence_eligible,
            cadence_candidate: self.cadence_candidate,
            cadence_action: self.cadence_action,
            return_value: self.return_value,
        }
    }

    pub(super) fn hash_into(&self, hasher: &mut Sha256, phase: u8) {
        hasher.update(b"level2-nyphur-common-upkeep-v1");
        hasher.update([
            self.player_slot,
            self.player_color_index,
            phase,
            u8::from(self.high_upkeep),
            u8::from(self.map_angle_allowed),
            u8::from(self.cadence_eligible),
            self.return_value.cast_unsigned(),
        ]);
        hasher.update(self.source_slot.to_le_bytes());
        match self.source_list_model_zero {
            Some(index) => {
                hasher.update([1]);
                hasher.update(index.to_le_bytes());
            }
            None => hasher.update([0]),
        }
        hasher.update(self.castle_entity_index.to_le_bytes());
        hasher.update(self.speed.to_le_bytes());
        hasher.update(self.strafe_speed.to_le_bytes());
        hasher.update(self.word_0xe_14.to_le_bytes());
        hasher.update([
            self.byte_0x45c.cast_unsigned(),
            self.byte_0x45d.cast_unsigned(),
        ]);
        hasher.update([self.byte_0x45e]);
        hasher.update(self.move_boost.to_le_bytes());
        hasher.update([
            self.move_speed,
            self.move_speed_flag,
            self.player_hit_frame_time,
        ]);
        hasher.update(self.dword_0x18d.to_le_bytes());
        hasher.update([self.palette_sub_mod]);
        hasher.update(self.palette_count.to_le_bytes());
        hasher.update([u8::from(self.palette_modification_enabled)]);
        match self.contact_target_entity {
            Some(index) => {
                hasher.update([1]);
                hasher.update(index.to_le_bytes());
            }
            None => hasher.update([0]),
        }
        hasher.update(self.contact_distance.to_le_bytes());
        hasher.update(self.contact_word_0x14a.to_le_bytes());
        hasher.update(self.yaw_0x1e_30.to_le_bytes());
        hasher.update(self.fov_0x22_34.to_le_bytes());
        hasher.update(self.word_0x1a2.to_le_bytes());
        hasher.update(self.word_0x1a4.to_le_bytes());
        hasher.update([self.byte_0x154]);
        hasher.update(self.word_0x159.to_le_bytes());
        hasher.update(self.life_regen.to_le_bytes());
        hasher.update(self.dword_0x16d.to_le_bytes());
        hasher.update(self.source_mailbox);
        for value in self.array_0x1fc {
            hasher.update(value.to_le_bytes());
        }
        hasher.update(self.padding_0x23c);
        for value in [
            self.word_0x242,
            self.word_0x244,
            self.word_0x246,
            self.word_0x248,
            self.word_0x24a,
            self.word_0x24c,
        ] {
            hasher.update(value.to_le_bytes());
        }
        for value in self.spell_enabled {
            hasher.update(value.to_le_bytes());
        }
        for value in self.spell_holder_indices {
            hasher.update(value.to_le_bytes());
        }
        for value in self.spell_levels {
            hasher.update(value.to_le_bytes());
        }
        hasher.update([self.owner_control_mode]);
        hasher.update(self.mana_regen.to_le_bytes());
        hasher.update(self.mana.to_le_bytes());
        hasher.update(self.life.to_le_bytes());
        hasher.update(self.max_mana.to_le_bytes());
        hasher.update(self.max_life.to_le_bytes());
        match self.cadence_candidate {
            Some(index) => {
                hasher.update([1]);
                hasher.update(index.to_le_bytes());
            }
            None => hasher.update([0]),
        }
        hasher.update([self.cadence_action]);
        hasher.update(self.watcom_rand_state.to_le_bytes());
    }

    /// Advance the pinned Watcom C runtime stream and expose its 15-bit
    /// result.  The source helper advances before applying `% 255`.
    fn step_watcomrand(&mut self) -> u16 {
        self.watcom_rand_state = self
            .watcom_rand_state
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12_345);
        ((self.watcom_rand_state >> 16) & 0x7fff) as u16
    }

    fn cadence_period(&self) -> u16 {
        let sign = i32::from(self.word_0x246).signum();
        let adjusted = (i32::from(self.word_0x246) - (sign << 2) + sign) >> 2;
        let period = 64 - adjusted;
        u16::try_from(period).expect("qualified Nyphur reflexes produce a positive cadence")
    }

    fn auxiliary_word(&self, index: usize) -> u16 {
        if index < self.array_0x1fc.len() {
            self.array_0x1fc[index]
        } else {
            let offset = (index - self.array_0x1fc.len()) * 2;
            u16::from_le_bytes([self.padding_0x23c[offset], self.padding_0x23c[offset + 1]])
        }
    }

    fn set_auxiliary_word(&mut self, index: usize, value: u16) {
        if index < self.array_0x1fc.len() {
            self.array_0x1fc[index] = value;
        } else {
            let offset = (index - self.array_0x1fc.len()) * 2;
            self.padding_0x23c[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn source_spell_holder_slot(&self, family: u8) -> Option<u16> {
        // The source `sub_146C0` table is `str_611.SpellsEnabled`, which is
        // distinct from `array_0x367.SpellEnabled` (`spell_enabled` here),
        // the cooldown words decremented by `sub_12A70`.
        self.spell_holder_indices
            .get(usize::from(family))
            .copied()
            .filter(|slot| *slot != 0)
    }

    /// Resolve the source-owned family-4 holder consumed by `sub_15FC0` and
    /// `sub_161A0`.  `sub_146C0` itself only follows the player relation; the
    /// mode-14 path additionally rejects a cross-owner, wrong-class, wrong-
    /// model, or wrong-action carrier instead of treating an unrelated live
    /// row as the advanced-mode holder.
    fn source_family4_holder<'a>(
        &self,
        entities: &'a [EntitySnapshot],
    ) -> Option<&'a EntitySnapshot> {
        self.sub_146c0(4, entities).filter(|holder| {
            holder.class == 15
                && holder.model == 4
                && holder.action == 12
                && holder.source_spell_parent_entity_index() == self.source_slot
        })
    }

    /// Pinned `sub_146C0`: resolve one source-slot/list relation against the
    /// live entity pool. The C++ sentinel comparison is represented by the
    /// explicit index, active, and nonnegative-life checks; malformed or
    /// missing carriers therefore return the source null result instead of
    /// indexing a neighboring row.
    fn sub_146c0<'a>(
        &self,
        family: u8,
        entities: &'a [EntitySnapshot],
    ) -> Option<&'a EntitySnapshot> {
        let slot = self.source_spell_holder_slot(family)?;
        Self::sub_146c0_slot(slot, entities)
    }

    fn sub_146c0_slot<'a>(slot: u16, entities: &'a [EntitySnapshot]) -> Option<&'a EntitySnapshot> {
        if slot == 0 {
            return None;
        }
        let holder = entities.get(usize::from(slot))?;
        (holder.index == slot && holder.active && holder.life >= 0).then_some(holder)
    }

    /// Pinned `sub_15730`: the AI's maximum mana is compared with the
    /// resolved holder's maximum mana using the source's inclusive boundary.
    /// `sub_146C0` owns the invalid-carrier failure path.
    fn sub_15730(&self, family: u8, entities: &[EntitySnapshot]) -> bool {
        self.sub_146c0(family, entities)
            .is_some_and(|holder| self.max_mana >= holder.max_mana)
    }

    /// Resolve the family-8 carrier from the entity currently named by source
    /// word 0x96. The pinned call is `sub_15760(Entities[target], 8)`: the
    /// target itself must be a player/wizard carrier whose `entity_index`
    /// names that row. It does not follow the target's own 0x94 relation and
    /// does not consult this AI's common-upkeep holder table.
    fn target_sub_15760_family8(
        &self,
        target_slot: u16,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> bool {
        let Some(target) = entities.get(usize::from(target_slot)) else {
            return false;
        };
        if target.index != target_slot {
            return false;
        }
        let Some(owner) = players
            .iter()
            .find(|player| player.entity_index == target_slot)
        else {
            return false;
        };
        let holder_slot = owner.enabled_spell_holders[8];
        Self::sub_146c0_slot(holder_slot, entities).is_some_and(|holder| holder.active_value > 0)
    }

    /// Pinned `sub_15760`: return a holder only while its source word 0x2E is
    /// positive. A cold holder remains a real source slot but is not an
    /// active target for this predicate.
    fn sub_15760<'a>(
        &self,
        family: u8,
        entities: &'a [EntitySnapshot],
    ) -> Option<&'a EntitySnapshot> {
        self.sub_146c0(family, entities)
            .filter(|holder| holder.active_value > 0)
    }

    /// Pinned `sub_156F0`: the bounded family input is part of the helper's
    /// contract; values outside the 26 source spell rows fail closed.
    fn sub_156f0(&self, family: u8, entities: &[EntitySnapshot]) -> bool {
        family < PLAYER_SPELL_COUNT as u8 && self.sub_15760(family, entities).is_some()
    }

    /// Pinned `sub_15E60`: report whether any of the seven source families
    /// that can provide a targeting/ability carrier has a holder slot.
    fn sub_15e60(&self) -> bool {
        [0_u8, 7, 0x12, 0x10, 0x14, 0x15, 9]
            .into_iter()
            .any(|family| self.source_spell_holder_slot(family).is_some())
    }

    /// Pinned `sub_13DC0`: the damaged-source Castle relation is tested
    /// before the cadence gate. A missing, inactive, or mismatched Castle
    /// carrier is the source null result.
    fn step_sub_13dc0(&self, source: &mut EntitySnapshot, castle: Option<&EntitySnapshot>) -> bool {
        if self.max_life / 2 <= self.life || self.castle_entity_index == 0 {
            return false;
        }
        let Some(castle) = castle else {
            return false;
        };
        if castle.index != self.castle_entity_index || !castle.active || castle.life < 0 {
            return false;
        }
        source.set_source_relation(castle);
        true
    }

    /// Pinned `sub_13C50` entry gates. Its final `sub_155E0` eligibility
    /// carrier belongs to another Bead, so a qualified holder is not
    /// substituted with a generic cast: the dependent branch fails closed.
    fn step_sub_13c50(
        &self,
        source: &EntitySnapshot,
        castle: Option<&EntitySnapshot>,
        entities: &[EntitySnapshot],
    ) -> bool {
        let Some(castle) = castle else {
            return false;
        };
        if castle.index != self.castle_entity_index
            || !castle.active
            || castle.life < 0
            || castle.action != 4
            || castle.activation_value != 0
        {
            return false;
        }
        let Some(_family_two_holder) = self.sub_146c0(2, entities) else {
            return false;
        };
        if !self.sub_15730(2, entities) {
            return false;
        }

        // `sub_155E0` owns the remaining Castle aperture/cooldown/heading
        // decision. It is intentionally not reimplemented in this lane.
        let _ = source;
        false
    }

    /// Pinned `sub_13CE0`: family 1 is the first carrier gate, followed by
    /// the inclusive family-2 maximum-mana comparison and the source-ordered
    /// `sub_148E0` target search. The returned slot is the only write carrier
    /// for the caller's source `word_0x96_150`/`word_0x98_152` pair; this
    /// helper itself reads the indexed rows, players, and the common Watcom
    /// stream, and does not mutate the source entity.
    fn step_sub_13ce0(
        &mut self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> Option<u16> {
        if self.sub_146c0(1, entities).is_none() {
            return None;
        }
        if let Some(family_two_holder) = self.sub_146c0(2, entities)
            && self.max_mana > family_two_holder.max_mana
        {
            return None;
        }

        self.sub_148e0_target(source, entities, players)
    }

    /// The qualified fall-through of `sub_12E70` reaches `sub_14630`. Keep
    /// its castle/relation write here so the post-mode tail owns the source
    /// slot and identity together. Mode-specific selector helpers remain in
    /// their packets; this is only the common-tail operation itself.
    fn step_sub_14630(&self, source: &mut EntitySnapshot, castle: Option<&EntitySnapshot>) -> i8 {
        if self.life >= self.max_life || self.castle_entity_index == 0 {
            return 12;
        }
        let Some(castle) = castle else {
            return 12;
        };
        if castle.index != self.castle_entity_index || !castle.active || castle.life < 0 {
            return 12;
        }
        source.set_source_relation(castle);
        11
    }

    fn source_spell_profile(family: u8, raw_tier: i8) -> Option<(u8, Level2SetSpellProfile)> {
        let tier = level2_applied_tier(raw_tier)?;
        let profile = match family {
            0 | 1 | 3 => level2_direct_starting_spell_profile(family, tier),
            4 => level2_nyphur_family4_spell_profile(tier),
            _ => level2_set_spell_profile(family, tier),
        }?;
        Some((tier, profile))
    }

    /// Apply the source `SetSpell_6D5E0` write without running the later
    /// affordability predicate.  `sub_15FC0` calls SetSpell directly after
    /// selecting a class-5 bucket; the following `sub_161A0` activation owns
    /// the separate `sub_15170`/mana/cooldown gate.
    fn step_set_spell_profile(
        &self,
        family: u8,
        raw_tier: i8,
        entities: &mut [EntitySnapshot],
    ) -> bool {
        let Some(holder_slot) = self.source_spell_holder_slot(family) else {
            return false;
        };
        let Some((tier, profile)) = Self::source_spell_profile(family, raw_tier) else {
            return false;
        };
        let Some(holder) = entities.get_mut(usize::from(holder_slot)) else {
            return false;
        };
        if holder.index != holder_slot || !holder.active || holder.life < 0 {
            return false;
        }
        holder.apply_nyphur_spell_profile(profile, tier);
        true
    }

    /// The source `sub_15170` availability predicate for the source-backed
    /// profiles: direct family 0/1/3 rows from `Spells.cpp`, plus the generic
    /// family 6/11/12 table. Families without one of those qualified rows
    /// fail closed; they are not coerced into a neighboring profile.
    fn source_spell_available(
        &self,
        family: u8,
        holder: &EntitySnapshot,
        profile: Level2SetSpellProfile,
        entities: &[EntitySnapshot],
    ) -> bool {
        if self.max_mana < profile.max_mana
            || self.mana < profile.mana
            || self.spell_enabled[usize::from(family)] != 0
            || self.mana < holder.max_mana
        {
            return false;
        }
        if matches!(family, 4 | 6 | 8 | 11) && self.sub_156f0(family, entities) {
            return false;
        }
        true
    }

    /// Execute the source `sub_15F20`: SetSpell happens before the
    /// availability check, including on the not-enough-mana result.
    fn step_sub_15f20(&mut self, family: u8, raw_tier: i8, entities: &mut [EntitySnapshot]) -> i8 {
        if usize::from(family) >= PLAYER_SPELL_COUNT {
            return 0;
        }
        if !(0..LEVEL2_NYPHUR_SOURCE_SPELL_TIER_COUNT).contains(&raw_tier) {
            return 0;
        }
        let Some(holder_slot) = self.sub_146c0(family, entities).map(|holder| holder.index) else {
            return 0;
        };
        let Some((_, profile)) = Self::source_spell_profile(family, raw_tier) else {
            return 0;
        };
        let Some(holder) = entities.get_mut(usize::from(holder_slot)) else {
            return 0;
        };
        let tier = level2_applied_tier(raw_tier).expect("non-negative Nyphur spell tier");
        holder.apply_nyphur_spell_profile(profile, tier);
        let Some(holder) = entities.get(usize::from(holder_slot)) else {
            return 0;
        };
        if !self.source_spell_available(family, holder, profile, entities) {
            return if self.max_mana < profile.max_mana || self.mana < profile.mana {
                -1
            } else {
                0
            };
        }
        family.cast_signed()
    }

    /// The pinned `sub_15790` quarter expression. `my_sign32` is zero for
    /// every nonnegative carrier, including max mana 1000.
    fn sub_15790_mana_quarter(max_mana: i32) -> i32 {
        let sign = if max_mana < 0 { -1 } else { 0 };
        max_mana.wrapping_sub(sign << 2).wrapping_add(sign) >> 2
    }

    /// Model the x86 carry flag returned by a left shift. `__CFSHL__(x, 2)`
    /// is the final bit shifted out, not the shifted value; for the source's
    /// arithmetic sign carrier (`0` or `-1`) it is respectively `0` or `1`.
    fn source_cfs_hl(value: i32, shift: u32) -> i32 {
        debug_assert!((1..=32).contains(&shift));
        i32::from(((value.cast_unsigned() >> (32 - shift)) & 1) != 0)
    }

    /// The pinned `sub_15910` quarter expression. For the qualified
    /// nonnegative max-mana carrier this is exactly `max_mana >> 2` (250 at
    /// max mana 1000); retain the source sign carrier for negative inputs.
    fn sub_15910_mana_quarter(max_mana: i32) -> i32 {
        let sign_bit = max_mana >> 31;
        let carry = Self::source_cfs_hl(sign_bit, 2);
        let correction = carry.wrapping_add(sign_bit.wrapping_mul(4));
        max_mana.wrapping_sub(correction) >> 2
    }

    /// Apply the shared source `word_0x1A4` mana latch after the caller has
    /// supplied its helper-specific quarter expression. All arithmetic stays
    /// in the source's signed 32-bit carrier; the latch clears at the
    /// inclusive recovery boundary.
    fn update_spell_mana_latch_with_quarter(&mut self, quarter: i32) -> bool {
        if quarter <= self.mana {
            if self.word_0x1a4 != 0 {
                let mut recovery = quarter.wrapping_add(6_000);
                if recovery >= self.max_mana {
                    recovery = self.max_mana / 2;
                }
                if recovery <= self.mana {
                    self.word_0x1a4 = 0;
                }
            }
        } else {
            self.word_0x1a4 = 1;
        }
        self.word_0x1a4 == 0
    }

    fn update_spell_mana_latch_15790(&mut self) -> bool {
        self.update_spell_mana_latch_with_quarter(Self::sub_15790_mana_quarter(self.max_mana))
    }

    fn update_spell_mana_latch_15910(&mut self) -> bool {
        self.update_spell_mana_latch_with_quarter(Self::sub_15910_mana_quarter(self.max_mana))
    }

    /// Pinned `sub_15790`: optional family-7 selection consumes one Watcom
    /// draw only when the current target owner's family-8 holder is active,
    /// then the sentinel-table scan runs in source order with descending
    /// tiers. The family-19 target model guard is retained even though its
    /// authored profile is currently outside the qualified Rust spell table.
    fn step_sub_15790(
        &mut self,
        target_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> i8 {
        if !self.update_spell_mana_latch_15790() {
            return -1;
        }

        if self.target_sub_15760_family8(target_slot, entities, players) {
            let roll = self.step_watcomrand() % 255;
            if i32::from(roll) < i32::from(self.word_0x244) {
                let mut tier = self.spell_levels[7];
                while tier >= 0 {
                    if self.step_sub_15f20(7, tier, entities) == 7 {
                        return 7;
                    }
                    tier -= 1;
                }
            }
        }

        for &family in &LEVEL2_NYPHUR_SPELL_SCAN_15790 {
            if family == u8::MAX {
                break;
            }
            let mut tier = self.spell_levels[usize::from(family)];
            while tier >= 0 {
                if self.step_sub_15f20(family, tier, entities) == family.cast_signed() {
                    if family != 19 {
                        return family.cast_signed();
                    }
                    let Some(target) = entities.get(usize::from(target_slot)) else {
                        break;
                    };
                    if target.index == target_slot
                        && target.active
                        && target.life >= 0
                        && matches!(target.model, 0 | 1)
                    {
                        return family.cast_signed();
                    }
                }
                tier -= 1;
            }
        }
        -1
    }

    /// Pinned `sub_15910`: it shares only the mana latch with `sub_15790`;
    /// this source order has no optional family-7 branch and therefore never
    /// consumes the Watcom stream itself.
    fn step_sub_15910(&mut self, entities: &mut [EntitySnapshot]) -> i8 {
        if !self.update_spell_mana_latch_15910() {
            return -1;
        }
        for &family in &LEVEL2_NYPHUR_SPELL_SCAN_15910 {
            if family == u8::MAX {
                break;
            }
            let mut tier = self.spell_levels[usize::from(family)];
            while tier >= 0 {
                let result = self.step_sub_15f20(family, tier, entities);
                if result == family.cast_signed() {
                    return result;
                }
                tier -= 1;
            }
        }
        -1
    }

    /// Pinned `sub_5F7E0`: the owner visibility mode is read as an unsigned
    /// byte, while the suppression write targets byte zero of the source
    /// control dword.
    fn step_sub_5f7e0(&self, holder_model: u8, source_control_flags: &mut u32) {
        if self.owner_control_mode < 2 || (self.owner_control_mode <= 2 && holder_model != 1) {
            *source_control_flags &= !0x0000_0020;
        }
    }

    /// Pinned `sub_5F7B0`: copy holder word 0x30 into word 0x2E, clear only
    /// byte-one bits 0/1 of the source control dword, preserve the caller's
    /// activation mask, then apply `sub_5F7E0`.
    fn step_sub_5f7b0(
        &self,
        holder: &mut EntitySnapshot,
        source_control_flags: &mut u32,
        activation_flags: u32,
    ) {
        holder.active_value = holder.activation_value.cast_signed();
        holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
        *source_control_flags &= !0x0000_0300;
        *source_control_flags |= activation_flags;
        self.step_sub_5f7e0(holder.model, source_control_flags);
    }

    /// Execute the source `sub_5F660` switch, including its model-specific
    /// direct-success cases and its allocation/mana fallback.  The source
    /// model is passed explicitly because these helpers modify the owner
    /// control dword, not the local-player projection.
    fn step_sub_5f660(
        &mut self,
        source_model: u8,
        source_control_flags: &mut u32,
        holder: &mut EntitySnapshot,
        activation_flags: u32,
    ) -> bool {
        let source_model_one = source_model == 1;
        let activation_flags = if source_model_one {
            0
        } else {
            activation_flags
        };
        let mut direct_success = false;
        let mut use_mana_fallback = false;

        match holder.model {
            0 => {
                // Both the byte<2 break and the byte>=2 jump land in the
                // source allocation/mana fallback after the switch.
                use_mana_fallback = true;
            }
            1 => {
                if holder.active_value > 0 {
                    holder.byte_3c = 1;
                    holder.record[64] = 1;
                    *source_control_flags &= !0x0000_0300;
                    *source_control_flags |= activation_flags;
                    self.step_sub_5f7e0(holder.model, source_control_flags);
                    direct_success = true;
                } else {
                    use_mana_fallback = true;
                }
            }
            2 => {
                if holder.active_value > 0 {
                    // The source prepares sound 29 here for a non-model-1
                    // caster. The common carrier has no event journal for
                    // this unowned mode branch, but retain the source audio
                    // call identity rather than fabricating a typed event.
                    if !source_model_one {
                        mark_source_audio_call(SourceAudioCallTag::NotEnoughManaImmediate);
                    }
                    direct_success = false;
                } else {
                    use_mana_fallback = true;
                }
            }
            4 | 6 | 8 | 11 | 12 | 14 => {
                if source_model == 0 && holder.active_value > 0 {
                    holder.active_value = if holder.model == 4 { 7 } else { 1 };
                    holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                    // The pinned branch jumps to LABEL_23 with v7 still
                    // zero. It mutates the holder but does not report a
                    // successful activation or enter the mana fallback.
                } else {
                    use_mana_fallback = true;
                }
            }
            7 => {
                if holder.byte_3b >= 1 && holder.active_value > 0 {
                    // LABEL_23 returns the switch's initial zero result.
                } else {
                    use_mana_fallback = true;
                }
            }
            9 | 10 | 13 | 15..=24 => {
                if holder.active_value > 0 {
                    // LABEL_23 returns the switch's initial zero result.
                } else {
                    use_mana_fallback = true;
                }
            }
            _ => {
                // The source default also reaches the shared mana/allocation
                // tail for a live holder; it is not an implicit refusal.
                use_mana_fallback = true;
            }
        }

        if direct_success {
            return true;
        }
        if !use_mana_fallback {
            return false;
        }
        if self.mana < holder.max_mana {
            // AI_HELPER-0076 (`sub_88B60`) and its notification storage are
            // owned by the later mode/caster carrier. The pinned source calls
            // it only at EventsFunctions.cpp:61006 from the insufficient-
            // mana branch. Preserve only the source audio call when this
            // common primitive reaches that branch; do not invent a help UI
            // state here.
            if !source_model_one {
                mark_source_audio_call(SourceAudioCallTag::NotEnoughManaFallback);
            }
            return false;
        }
        self.step_sub_5f7b0(holder, source_control_flags, activation_flags);
        true
    }

    /// Execute the source `sub_14E10` activation tail and install its exact
    /// family cooldown.  No event/action is fabricated for an unsupported
    /// spell table row.
    fn step_sub_14e10(
        &mut self,
        family: u8,
        source_model: u8,
        source_control_flags: &mut u32,
        entities: &mut [EntitySnapshot],
    ) -> bool {
        if family == 4 && self.source_family4_holder(entities).is_none() {
            return false;
        }
        let Some(holder_slot) = self.source_spell_holder_slot(family) else {
            return false;
        };
        let Some(holder_before) = entities.get(usize::from(holder_slot)) else {
            return false;
        };
        let Some((_, profile)) = Self::source_spell_profile(family, holder_before.level_gate)
        else {
            return false;
        };
        if !self.source_spell_available(family, holder_before, profile, entities) {
            return false;
        }
        let Some(holder) = entities.get_mut(usize::from(holder_slot)) else {
            return false;
        };
        if !self.step_sub_5f660(source_model, source_control_flags, holder, 0) {
            return false;
        }
        self.spell_enabled[usize::from(family)] =
            LEVEL2_NYPHUR_SPELL_COOLDOWNS[usize::from(family)];
        true
    }

    /// Source `sub_15D20`: cadence contact arms the fixed 80-unit strafe
    /// only when the source latch is clear.
    fn step_sub_15d20(&mut self) {
        if self.byte_0x45e == 0 {
            self.strafe_speed = 80;
        }
    }

    fn cadence_candidate(
        &self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
    ) -> Option<u16> {
        let mut nearest = None;
        let mut nearest_distance = u32::MAX;
        for candidate_index in level2_nyphur_class9_source_order(entities) {
            let candidate = &entities[usize::from(candidate_index)];
            if candidate.index == source.index || candidate.source_word_96() != source.source_id {
                continue;
            }
            let left_x = i16::from_le_bytes(source.position.x.to_le_bytes());
            let right_x = i16::from_le_bytes(candidate.position.x.to_le_bytes());
            let left_y = i16::from_le_bytes(source.position.y.to_le_bytes());
            let right_y = i16::from_le_bytes(candidate.position.y.to_le_bytes());
            let dx = i32::from(right_x.wrapping_sub(left_x));
            let dy = i32::from(right_y.wrapping_sub(left_y));
            let distance = crate::legacy_sqrt(
                u32::try_from(dx * dx + dy * dy)
                    .expect("source XY distance fits the pinned uint32 carrier"),
            );
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest = Some(candidate.index);
            }
        }
        (nearest_distance < 0x0190_0000)
            .then_some(nearest)
            .flatten()
    }

    fn step_sub_15d40(
        &mut self,
        source: &EntitySnapshot,
        candidate: &EntitySnapshot,
        source_model: u8,
        source_control_flags: &mut u32,
        entities: &mut [EntitySnapshot],
    ) -> u8 {
        let dx = i32::from(i16::from_ne_bytes(
            candidate
                .position
                .x
                .wrapping_sub(source.position.x)
                .to_ne_bytes(),
        ));
        let dy = i32::from(i16::from_ne_bytes(
            candidate
                .position
                .y
                .wrapping_sub(source.position.y)
                .to_ne_bytes(),
        ));
        let distance = crate::legacy_sqrt(
            u32::try_from(dx * dx + dy * dy)
                .expect("source XY distance fits the pinned uint32 carrier"),
        );
        if distance >= 0x0010_0000 {
            return distance.to_le_bytes()[0];
        }
        match candidate.model {
            0 => 0,
            1..=2 => candidate.model,
            4 => {
                let mut result = distance.to_le_bytes()[0];
                let mut tier = self.spell_levels[6];
                while tier >= 0 {
                    let attempt = self.step_sub_15f20(6, tier, entities);
                    result = attempt.cast_unsigned();
                    if attempt == 6 {
                        result =
                            if self.step_sub_14e10(6, source_model, source_control_flags, entities)
                            {
                                6
                            } else {
                                0
                            };
                        break;
                    }
                    tier -= 1;
                }
                result
            }
            3 => {
                let mut result = distance.to_le_bytes()[0];
                let mut tier = self.spell_levels[8];
                let mut family_8_succeeded = false;
                while tier >= 0 {
                    let attempt = self.step_sub_15f20(8, tier, entities);
                    result = attempt.cast_unsigned();
                    if attempt == 8 {
                        family_8_succeeded = true;
                        result =
                            if self.step_sub_14e10(8, source_model, source_control_flags, entities)
                            {
                                8
                            } else {
                                0
                            };
                        break;
                    }
                    tier -= 1;
                }
                if !family_8_succeeded {
                    let mut tier = self.spell_levels[6];
                    while tier >= 0 {
                        let attempt = self.step_sub_15f20(6, tier, entities);
                        result = attempt.cast_unsigned();
                        if attempt == 6 {
                            result = if self.step_sub_14e10(
                                6,
                                source_model,
                                source_control_flags,
                                entities,
                            ) {
                                6
                            } else {
                                0
                            };
                            break;
                        }
                        tier -= 1;
                    }
                }
                result
            }
            _ => 0,
        }
    }

    /// Pinned `sub_5C800`: the global palette sub-mod is written only for the
    /// source whose player color is the active level color.
    fn step_sub_5c800(&mut self, entity_color_index: u8, active_level_color_index: u8) {
        if entity_color_index == active_level_color_index {
            self.palette_sub_mod = 2;
        }
    }

    /// Execute the source `sub_5EFA0` mailbox consumer.  The owner mailbox is
    /// part of the imported entity record; the remaining `Type_str_164`
    /// fields are carried here so a contact cannot silently become a
    /// local-player-only side effect.
    fn step_sub_5efa0(
        &mut self,
        source: &mut EntitySnapshot,
        previous_relation_live: bool,
        contact_target: Option<(u16, Position)>,
        attacker_target: Option<(Position, bool)>,
        move_speed_flag: u8,
        active_level_color_index: u8,
    ) -> i8 {
        source.record[42..44].fill(0);
        self.return_value = 0;
        if source.source_word_96() != 0 && !previous_relation_live {
            source.clear_source_word_96();
        }
        if self.life < 0 {
            self.return_value = 2;
            source.action = 2;
            source.record[73] = 2;
            return self.return_value;
        }

        let contact_word = source.source_upkeep_word_7a();
        if contact_word != 0 {
            if let Some((target_index, target_position)) = contact_target {
                self.contact_target_entity = Some(target_index);
                self.contact_distance =
                    level2_nyphur_source_distance_3d(source.position, target_position);
                self.contact_word_0x14a =
                    i16::from_le_bytes([source.record[122], source.record[123]]);
            }
            source.record[126..128].fill(0);
            self.player_hit_frame_time = 4;
            self.dword_0x18d = 16;
            self.word_0x24c = 64;
            source.apply_source_sub_5ef70();
        }

        // SetPaletteModification_5C830 is gated by the source global flag and
        // only affects an owner whose color is the active level.  Both inputs
        // arrive from the ordinary session context; the AI does not borrow
        // the local player's move-speed state as a shortcut.
        self.move_speed_flag = move_speed_flag;
        if self.move_speed != 0
            && move_speed_flag != 0
            && self.player_color_index == active_level_color_index
        {
            self.palette_modification_enabled = true;
            self.palette_sub_mod = 3;
            self.palette_count = u16::try_from(171_i32 * i32::from(self.move_speed) / 3 + 85)
                .expect("source palette count fits its uint16 carrier");
        }

        let attacker = source.source_upkeep_word_62();
        if attacker == 0 {
            return self.return_value;
        }
        source.record[42..44].copy_from_slice(&attacker.to_le_bytes());
        let mut control_flags = source.control_flags;
        if control_flags & 0x0000_4000 != 0 || control_flags & 0x0040_0000 != 0 {
            if control_flags & 0x0000_4000 != 0 {
                let mana_delta = source.source_upkeep_payload() >> 2;
                self.mana = self.mana.wrapping_sub(mana_delta);
                source.set_source_upkeep_payload(mana_delta);
                control_flags &= !0x0000_4000;
            } else {
                control_flags &= !0x0040_0000;
                source.set_source_upkeep_payload(0);
                control_flags |= 0x0000_4000;
            }
            source.set_control_flags(control_flags);
        }
        let damage = source.source_upkeep_payload();
        self.life = self.life.wrapping_sub(damage);
        self.move_boost = (damage / 10).clamp(0, 80) as i16;
        if let Some((target_position, _)) = attacker_target {
            self.yaw_0x1e_30 =
                i16::try_from(heading_for_positions(target_position, source.position))
                    .expect("source contact heading fits signed 16 bits");
            let dx = i32::from(i16::from_ne_bytes(
                source
                    .position
                    .x
                    .wrapping_sub(target_position.x)
                    .to_ne_bytes(),
            ));
            let dy = i32::from(i16::from_ne_bytes(
                source
                    .position
                    .y
                    .wrapping_sub(target_position.y)
                    .to_ne_bytes(),
            ));
            let horizontal = crate::legacy_sqrt(
                u32::try_from(dx * dx + dy * dy)
                    .expect("source contact horizontal distance fits uint32"),
            );
            self.fov_0x22_34 = crate::pitch_for_delta(
                i32::try_from(horizontal).expect("source contact distance fits signed 32 bits"),
                i32::from(source.position.z) - i32::from(target_position.z),
            );
        }
        self.step_sub_5c800(self.player_color_index, active_level_color_index);
        self.player_hit_frame_time = 4;
        self.dword_0x18d = 16;
        self.word_0x24c = 64;
        mark_source_audio_call(SourceAudioCallTag::PlayerPain);
        source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
        if self.life < 0 {
            let killer = if attacker_target.is_some_and(|(_, model67)| model67) {
                0
            } else {
                attacker
            };
            source.record[40..42].copy_from_slice(&killer.to_le_bytes());
            self.return_value = 2;
        } else {
            source.apply_source_sub_5ef70();
            source.clear_source_upkeep_owner();
            self.return_value = 1;
        }
        self.return_value
    }

    fn step(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
        move_speed_flag: u8,
        active_level_color_index: u8,
    ) -> Result<Level2NyphurCommonUpkeepSnapshot> {
        let entity_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur common source row is absent"))?;
        if source_slot != self.source_slot {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur common source slot changed",
            ));
        }
        let mut source_entity = entities[entity_index].clone();

        // sub_15EE0 runs before every other common-upkeep write and retains
        // the last model-0 source encountered in dword_38519, the rebuilt
        // class-3 linked list. Pool order is not source order.
        self.source_list_model_zero = level2_nyphur_class3_source_order(entities)
            .into_iter()
            .filter(|index| entities[usize::from(*index)].model == 0)
            .last();
        if self.word_0x1a2 < 0 {
            self.word_0x1a2 = self.word_0x1a2.wrapping_add(1);
        }
        for enabled in &mut self.spell_enabled {
            if *enabled > 0 {
                *enabled -= 1;
            }
        }
        for index in 0..8 {
            let threshold = 4 * index + 4;
            if self.auxiliary_word(threshold) < 0x601f {
                let next =
                    i32::from(self.word_0x242) + 1 + i32::from(self.auxiliary_word(4 * index));
                self.set_auxiliary_word(threshold, next as u16);
                if self.auxiliary_word(threshold) > 0x601f {
                    self.set_auxiliary_word(threshold, 0x601f);
                }
            }
            if self.auxiliary_word(threshold) > 0x601f {
                if self.auxiliary_word(threshold + 1) == 0 {
                    let decrement = (256_i32 - i32::from(self.word_0x242)) as u16;
                    self.set_auxiliary_word(
                        threshold,
                        self.auxiliary_word(threshold).wrapping_sub(decrement),
                    );
                }
                if self.auxiliary_word(threshold) < 0x601f {
                    self.set_auxiliary_word(threshold, 0x601f);
                }
            }
        }

        let castle = (self.castle_entity_index != 0)
            .then(|| entities.get(usize::from(self.castle_entity_index)))
            .flatten();
        let castle_overlaps =
            castle.is_some_and(|castle| level2_nyphur_strict_overlap(&source_entity, castle));
        self.high_upkeep = castle_overlaps || source_entity.control_flags & 0x1000 != 0;
        if castle_overlaps {
            self.word_0x159 = 2;
        }
        if self.word_0x159 != 0 {
            entities[entity_index].clear_source_upkeep_mailbox();
            self.source_mailbox = [0; 36];
            self.word_0x159 = self.word_0x159.wrapping_sub(1);
        } else {
            let relation_live = source_entity.source_word_96() == 0
                || entities
                    .get(usize::from(source_entity.source_word_96()))
                    .is_some_and(|target| {
                        target.life > 0
                            && target.control_flags & 0x0400 == 0
                            && level2_nyphur_source_identity_matches(&source_entity, target)
                    });
            let contact_target = entities
                .get(usize::from(source_entity.source_upkeep_word_7a()))
                .filter(|_| source_entity.source_upkeep_word_7a() != 0)
                .map(|target| (target.index, target.position));
            let attacker_target = entities
                .get(usize::from(source_entity.source_upkeep_word_62()))
                .filter(|_| source_entity.source_upkeep_word_62() != 0)
                .map(|target| (target.position, target.class == 10 && target.model == 67));
            self.return_value = self.step_sub_5efa0(
                &mut entities[entity_index],
                relation_live,
                contact_target,
                attacker_target,
                move_speed_flag,
                active_level_color_index,
            );
            self.source_mailbox
                .copy_from_slice(entities[entity_index].source_upkeep_mailbox());
            source_entity = entities[entity_index].clone();
            if self.return_value == 2 {
                return Ok(self.snapshot(source_entity.phase));
            }
        }

        let parameter_index = source_entity.parameter_relation.ok_or_else(|| {
            NativeWorldError::new("Level 2 Nyphur common source parameter is absent")
        })?;
        let parameter = presentation
            .entity_parameters
            .get(usize::from(parameter_index))
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur common parameter row is absent")
            })?;

        // Source sub_146F0 first consumes the movement latch. Otherwise it
        // predicts forward and strafe movement, eases actual speed, then
        // turns toward the source roll using the parameter row's 0x02/0x04
        // limits. The imported row has zero speed, but keeping the complete
        // carrier prevents a later source consumer from replacing this with
        // a position-preserving shortcut.
        if entities[entity_index].control_flags & 0x0800 != 0 {
            let control_flags = entities[entity_index].control_flags & !0x0800;
            entities[entity_index].set_control_flags(control_flags);
        } else {
            let word_160_e = i16::from_le_bytes([parameter[14], parameter[15]]);
            let lower_clearance = i16::from_le_bytes([parameter[12], parameter[13]]);
            let subtype_160_2 = i16::from_le_bytes([parameter[2], parameter[3]]);
            let word_160_4 = i16::from_le_bytes([parameter[4], parameter[5]]);
            let mut predicted = entities[entity_index].position;
            let ground = terrain.ground_height(predicted);
            level2_nyphur_sub_580e0(&mut predicted, ground, lower_clearance, word_160_e);
            let yaw = entities[entity_index].yaw;
            crate::move_entity(&mut predicted, yaw, 0, entities[entity_index].actual_speed);
            crate::move_entity(&mut predicted, yaw.wrapping_add(512), 0, self.strafe_speed);
            let strafe_sign = self.strafe_speed.signum();
            self.strafe_speed = self.strafe_speed.wrapping_sub(strafe_sign.wrapping_mul(4));
            entities[entity_index].set_position(predicted);

            let speed_delta = self.speed.wrapping_sub(entities[entity_index].actual_speed);
            if speed_delta != 0 {
                entities[entity_index].set_actual_speed(
                    entities[entity_index]
                        .actual_speed
                        .wrapping_add(speed_delta.signum().wrapping_mul(16)),
                );
            }
            let roll = entities[entity_index].source_roll();
            let angle = angle_distance(i32::from(yaw), i32::from(roll & 0x07ff));
            let reflex_gap = 255_i32 - i32::from(self.word_0x246);
            let reflex_sign = reflex_gap.signum();
            let denominator = ((reflex_gap - (reflex_sign << 4) + reflex_sign) >> 4) + 8;
            let turn = i32::from(angle) / denominator;
            let turn = if turn <= i32::from(subtype_160_2) {
                turn.max(i32::from(word_160_4))
            } else {
                i32::from(subtype_160_2)
            };
            let next_yaw = yaw.wrapping_add(level2_nyphur_turn_direction(yaw, roll).wrapping_mul(
                i16::try_from(turn).expect("Nyphur turn carrier fits signed 16 bits"),
            )) & 0x07ff;
            let crossed_roll = (yaw.cast_unsigned() < roll.cast_unsigned()
                && next_yaw.cast_unsigned() > roll.cast_unsigned())
                || (yaw.cast_unsigned() > roll.cast_unsigned()
                    && roll.cast_unsigned() > next_yaw.cast_unsigned());
            entities[entity_index].set_yaw(if crossed_roll { roll } else { next_yaw });
        }
        self.map_angle_allowed =
            level2_nyphur_map_angle_allowed(terrain, entities, entities[entity_index].position);
        if self.byte_0x154 < 200 {
            self.byte_0x154 += 1;
        }
        self.mana = self.mana.wrapping_add(self.mana_regen);
        self.life = self.life.wrapping_add(i32::from(self.life_regen));
        if self.life < -1 {
            self.life = -1;
        }
        if self.life > self.max_life {
            self.life = self.max_life;
        }
        if self.dword_0x16d != 0 {
            self.dword_0x16d = self.dword_0x16d.wrapping_sub(1);
        }
        if self.high_upkeep {
            self.mana_regen = (self.max_mana / 200).max(1_000);
            self.life_regen = (self.max_life / 200) as i16;
            let control_flags = entities[entity_index].control_flags & !0x1000;
            entities[entity_index].set_control_flags(control_flags);
        } else {
            self.mana_regen = (self.max_mana / 2_000).max(100);
            self.life_regen = (self.max_life / 500) as i16;
        }
        self.mana = self.mana.clamp(0, self.max_mana);
        let period = self.cadence_period();
        self.cadence_eligible = u16::from(source_entity.phase) % period == 0;
        self.cadence_candidate = if self.cadence_eligible {
            self.cadence_candidate(&source_entity, entities)
        } else {
            None
        };
        self.cadence_action = 0;
        let mut source_control_flags = entities[entity_index].control_flags;
        if let Some(candidate_index) = self.cadence_candidate {
            if let Some(candidate) = entities.get(usize::from(candidate_index)).cloned() {
                self.step_sub_15d20();
                self.cadence_action = self.step_sub_15d40(
                    &source_entity,
                    &candidate,
                    source_entity.model,
                    &mut source_control_flags,
                    entities,
                );
            }
        }
        if self.cadence_eligible && self.life < self.max_life {
            let mut tier = self.spell_levels[5];
            while tier >= 0 {
                if self.step_sub_15f20(5, tier, entities) == 5 {
                    let _ = self.step_sub_14e10(
                        5,
                        source_entity.model,
                        &mut source_control_flags,
                        entities,
                    );
                    break;
                }
                tier -= 1;
            }
        }
        entities[entity_index].set_control_flags(source_control_flags);

        let upper_clearance = i16::from_le_bytes([parameter[10], parameter[11]]);
        let lower_clearance = i16::from_le_bytes([parameter[12], parameter[13]]);
        let ground = terrain.ground_height(entities[entity_index].position);
        let upper = ground.wrapping_add(upper_clearance);
        let lower = ground.wrapping_add(lower_clearance);
        let mut position = entities[entity_index].position;
        if position.z > upper {
            position.z = upper;
        }
        if position.z < lower {
            position.z = lower;
        }
        if position != entities[entity_index].position {
            entities[entity_index].set_position(position);
        }
        entities[entity_index].set_mana(self.mana);
        entities[entity_index].set_life(self.life);
        entities[entity_index].set_mana_regen(self.mana_regen);
        // `sub_12A70` returns one after the complete common tail.  The
        // mailbox helper's zero/one result is only an early-branch carrier;
        // it must not replace the common helper's final return value.
        self.return_value = 1;
        Ok(self.snapshot(entities[entity_index].phase))
    }

    /// Pinned `sub_148E0`: scan the rebuilt class-10 target list in source
    /// order, preserve its model-57 perception draw and early break, then
    /// fall back to the class-5 model-22 bucket only when no primary target
    /// survived. The helper only returns a slot; the caller performs the
    /// single authoritative source relation write.
    fn sub_148e0_target(
        &mut self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> Option<u16> {
        const CLASS3_TARGET_PRESSURE: i32 = 50_000;
        const CLASS3_TARGET_DISTANCE: u32 = 0x0190_0000;

        let mut best = None;
        let mut best_distance = u32::MAX;
        for candidate_index in level2_nyphur_class10_target_source_order(entities) {
            let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                continue;
            };
            let owner_index = candidate.source_player_entity_index();
            if owner_index == source.index {
                continue;
            }

            // The source draws before resolving the model-57 owner row. A
            // successful perception branch breaks the primary list and then
            // still permits the model-22 fallback below.
            if candidate.model == 57 {
                let perception_roll = self.step_watcomrand() % 255;
                if i32::from(perception_roll) < i32::from(self.word_0x244) {
                    break;
                }
            }

            let Some(owner) = entities.get(usize::from(owner_index)) else {
                continue;
            };
            let distance = if owner.class == 3 {
                if owner_index == source.source_id {
                    continue;
                }
                let Some(owner_player) = players
                    .iter()
                    .find(|player| player.entity_index == owner_index)
                else {
                    continue;
                };
                let threshold_index = 4 * usize::from(owner_player.player_color_index()) + 4;
                if threshold_index > 32 {
                    continue;
                }
                let threshold = self.auxiliary_word(threshold_index);
                let pressure =
                    CLASS3_TARGET_PRESSURE - owner.max_mana / 10 * i32::from(self.word_0x242) / 255;
                if pressure >= i32::from(threshold) {
                    let Some(nearest_model01) =
                        level2_nyphur_sub_16fc0_target(candidate, source, entities)
                    else {
                        continue;
                    };
                    if source.source_id != nearest_model01.source_id
                        && level2_nyphur_source_distance_xy_euclidean_sq(
                            candidate.position,
                            nearest_model01.position,
                        ) <= CLASS3_TARGET_DISTANCE
                    {
                        continue;
                    }
                    let nearest_model2 = if self.castle_entity_index == 0 {
                        level2_nyphur_sub_14b10_or_14bd0_target(
                            candidate,
                            &[candidate.source_id],
                            entities,
                        )
                    } else {
                        let Some(castle) = entities.get(usize::from(self.castle_entity_index))
                        else {
                            continue;
                        };
                        level2_nyphur_sub_14b10_or_14bd0_target(
                            candidate,
                            &[candidate.source_id, castle.source_id],
                            entities,
                        )
                    };
                    if nearest_model2
                        .is_some_and(|peer| level2_nyphur_strict_overlap(candidate, peer))
                    {
                        continue;
                    }
                    level2_nyphur_source_distance_xy_euclidean_sq(
                        source.position,
                        candidate.position,
                    )
                } else {
                    let castle_position = if self.castle_entity_index == 0 {
                        let Some(sentinel) = entities.first() else {
                            continue;
                        };
                        sentinel.position
                    } else {
                        let Some(castle) = entities.get(usize::from(self.castle_entity_index))
                        else {
                            continue;
                        };
                        castle.position
                    };
                    level2_nyphur_source_distance_xy_euclidean_sq(
                        castle_position,
                        candidate.position,
                    )
                }
            } else {
                level2_nyphur_source_distance_xy_euclidean_sq(source.position, candidate.position)
            };
            if distance < best_distance {
                best = Some(candidate.index);
                best_distance = distance;
            }
        }

        if best.is_none() {
            for candidate_index in level2_nyphur_class5_bucket_order(entities)
                .into_iter()
                .filter(|index| {
                    entities
                        .get(usize::from(*index))
                        .is_some_and(|candidate| candidate.model == 22)
                })
            {
                let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                    continue;
                };
                if candidate.model == 57
                    || candidate.source_player_entity_index() == source.source_id
                {
                    continue;
                }
                let distance = level2_nyphur_source_distance_xy_euclidean_sq(
                    source.position,
                    candidate.position,
                );
                if distance < best_distance {
                    best = Some(candidate.index);
                    best_distance = distance;
                }
            }
        }
        best
    }
}

fn level2_nyphur_strict_overlap(left: &EntitySnapshot, right: &EntitySnapshot) -> bool {
    let left_x = i16::from_le_bytes(left.position.x.to_le_bytes());
    let right_x = i16::from_le_bytes(right.position.x.to_le_bytes());
    let left_y = i16::from_le_bytes(left.position.y.to_le_bytes());
    let right_y = i16::from_le_bytes(right.position.y.to_le_bytes());
    let dx = i32::from(right_x.wrapping_sub(left_x)).abs();
    let dy = i32::from(right_y.wrapping_sub(left_y)).abs();
    let dz = i32::from(
        left.position
            .z
            .wrapping_add(left.array_yaw())
            .wrapping_sub(right.position.z.wrapping_add(right.array_yaw())),
    )
    .abs();
    dx < i32::from(left.array_pitch()) + i32::from(right.array_pitch())
        && dy < i32::from(left.array_roll()) + i32::from(right.array_roll())
        && dz < i32::from(left.array_fov()) + i32::from(right.array_fov())
}

fn level2_nyphur_linked_source_order(
    entities: &[EntitySnapshot],
    include: impl Fn(&EntitySnapshot) -> bool,
) -> Vec<u16> {
    let eligible = entities
        .iter()
        .filter(|entity| include(entity))
        .map(|entity| entity.index)
        .collect::<BTreeSet<_>>();
    let linked = eligible
        .iter()
        .filter_map(|index| entities.get(usize::from(*index)))
        .filter_map(|entity| entity.next_relation)
        .filter(|index| eligible.contains(index))
        .collect::<BTreeSet<_>>();
    let Some(mut current) = eligible
        .iter()
        .copied()
        .find(|index| !linked.contains(index))
    else {
        return Vec::new();
    };
    let mut visited = BTreeSet::new();
    let mut order = Vec::with_capacity(eligible.len());
    while current != 0 && visited.insert(current) {
        let Some(entity) = entities.get(usize::from(current)) else {
            break;
        };
        order.push(current);
        current = entity.next_relation.unwrap_or(0);
    }
    order
}

fn level2_nyphur_class3_source_order(entities: &[EntitySnapshot]) -> Vec<u16> {
    level2_nyphur_linked_source_order(entities, |entity| {
        entity.active && entity.life >= 0 && entity.class == 3
    })
}

/// Rebuild the source `dword_38523` list used by `sub_148E0`. `UpdateEntities`
/// appends class-10 models 39/40/57 in source-slot order and links that list
/// independently from the model-45 building list. Walking the encoded links
/// is therefore required; a pool-order sweep can change both strict ties and
/// the position of the model-57 perception draw.
fn level2_nyphur_class10_target_source_order(entities: &[EntitySnapshot]) -> Vec<u16> {
    level2_nyphur_linked_source_order(entities, |entity| {
        entity.active && entity.class == 10 && matches!(entity.model, 39 | 40 | 57)
    })
}

/// Rebuild the source `bytearray_38403x[0..29]` class-5 buckets in the same
/// pool order as `UpdateEntities_57730`. The source excludes only the three
/// action sentinels that are being torn down or already consumed.
fn level2_nyphur_class5_bucket_order(entities: &[EntitySnapshot]) -> Vec<u16> {
    let mut order = Vec::new();
    for model in 0..29_u8 {
        order.extend(
            entities
                .iter()
                .filter(|entity| {
                    entity.active
                        && entity.life >= 0
                        && entity.class == 5
                        && entity.model == model
                        && !matches!(entity.action, 0xb4 | 0xe8 | 0xea)
                })
                .map(|entity| entity.index),
        );
    }
    order
}

/// Rebuild only the four source buckets walked by `sub_15FC0`.  The pinned
/// `UpdateEntities_57730` pass visits entity slots in ascending order and
/// appends each live class-5 row to its model bucket, so this preserves both
/// bucket order and same-bucket ties without substituting a global class-5
/// sweep.
fn level2_nyphur_sub_15fc0_class5_source_order(entities: &[EntitySnapshot]) -> Vec<(u16, u8)> {
    let mut order = Vec::new();
    for model in LEVEL2_NYPHUR_SUB_15FC0_BUCKETS {
        order.extend(
            entities
                .iter()
                .filter(|entity| {
                    entity.active
                        && entity.life >= 0
                        && entity.class == 5
                        && entity.model == model
                        && !matches!(entity.action, 0xb4 | 0xe8 | 0xea)
                })
                .map(|entity| (entity.index, model)),
        );
    }
    order
}

fn level2_nyphur_class45_source_order(entities: &[EntitySnapshot]) -> Vec<u16> {
    level2_nyphur_linked_source_order(entities, |entity| {
        entity.active && entity.life >= 0 && entity.class == 10 && entity.model == 45
    })
}

fn level2_nyphur_class9_source_order(entities: &[EntitySnapshot]) -> Vec<u16> {
    level2_nyphur_linked_source_order(entities, |entity| {
        entity.active && entity.life >= 0 && entity.class == 9
    })
}

/// Pinned `sub_16FC0`: return the nearest live class-3 model-0/1 row while
/// excluding the candidate and Nyphur source identities. The strict update
/// retains the first row in the rebuilt class-3 linked list on ties.
fn level2_nyphur_sub_16fc0_target<'a>(
    candidate: &EntitySnapshot,
    source: &EntitySnapshot,
    entities: &'a [EntitySnapshot],
) -> Option<&'a EntitySnapshot> {
    let mut nearest = None;
    let mut nearest_distance = u32::MAX;
    for index in level2_nyphur_class3_source_order(entities) {
        let Some(peer) = entities.get(usize::from(index)) else {
            continue;
        };
        if peer.source_id == candidate.source_id
            || peer.source_id == source.source_id
            || !matches!(peer.model, 0 | 1)
        {
            continue;
        }
        let distance =
            level2_nyphur_source_distance_xy_euclidean_sq(candidate.position, peer.position);
        if distance < nearest_distance {
            nearest = Some(peer);
            nearest_distance = distance;
        }
    }
    nearest
}

/// Pinned `sub_14B10`/`sub_14BD0`: return the nearest live class-3 model-2
/// row while excluding the candidate and, when present, the Castle identity.
fn level2_nyphur_sub_14b10_or_14bd0_target<'a>(
    candidate: &EntitySnapshot,
    excluded_source_ids: &[u16],
    entities: &'a [EntitySnapshot],
) -> Option<&'a EntitySnapshot> {
    let mut nearest = None;
    let mut nearest_distance = u32::MAX;
    for index in level2_nyphur_class3_source_order(entities) {
        let Some(peer) = entities.get(usize::from(index)) else {
            continue;
        };
        if peer.model != 2
            || excluded_source_ids
                .iter()
                .any(|source_id| *source_id == peer.source_id)
        {
            continue;
        }
        let distance =
            level2_nyphur_source_distance_xy_euclidean_sq(candidate.position, peer.position);
        if distance < nearest_distance {
            nearest = Some(peer);
            nearest_distance = distance;
        }
    }
    nearest
}

fn level2_nyphur_sub_11c80(terrain: &SessionTerrain, x: u8, y: u8) -> bool {
    let angle = i8::from_ne_bytes([terrain.planes.angles[terrain_index(x, y)]]);
    angle >= 0 && (terrain.map_type != MapType::Cave || angle & 8 == 0)
}

fn level2_nyphur_turn_direction(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut difference = to - from;
    if difference.unsigned_abs() > 1_024 {
        difference += if difference >= 0 { -2_048 } else { 2_048 };
    }
    difference.signum() as i16
}

/// Pinned `sub_14C40`: the source identity is a signed 16-bit sum of the
/// entity id, model, and class discriminator.  The exported entity fields are
/// unsigned carriers, so perform the source-width wrap before interpreting
/// the checksum as signed.
pub(super) fn level2_nyphur_source_identity(entity: &EntitySnapshot) -> i16 {
    let value = entity
        .source_id
        .wrapping_add(u16::from(entity.model))
        .wrapping_add(u16::from(entity.class).wrapping_shl(7));
    i16::from_le_bytes(value.to_le_bytes())
}

/// Pinned `sub_14C60`: compare the stored target identity, not just its slot.
fn level2_nyphur_source_identity_matches(source: &EntitySnapshot, target: &EntitySnapshot) -> bool {
    source.source_word_98() == level2_nyphur_source_identity(target)
}

/// Read the source `axis_0x9A_154x` triple from the lossless entity record.
/// The exported record carries a four-byte package header, so source offsets
/// 154/156/158 are record offsets 158/160/162 respectively.
fn level2_nyphur_axis_position(source: &EntitySnapshot) -> Result<Position> {
    let axis = source
        .record
        .get(158..164)
        .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur axis_0x9A_154x carrier is absent"))?;
    Ok(Position {
        x: u16::from_le_bytes([axis[0], axis[1]]),
        y: u16::from_le_bytes([axis[2], axis[3]]),
        z: i16::from_le_bytes([axis[4], axis[5]]),
    })
}

/// Keep the source `axis_0x9A_154x` triple in the lossless entity record.
/// The typed entity projection has no duplicate axis cache, so this is the
/// single authoritative write used by the `sub_13B00` selector.
fn level2_nyphur_set_axis_position(source: &mut EntitySnapshot, axis: Position) {
    source.record[158..160].copy_from_slice(&axis.x.to_le_bytes());
    source.record[160..162].copy_from_slice(&axis.y.to_le_bytes());
    source.record[162..164].copy_from_slice(&axis.z.to_le_bytes());
}

/// Pinned signed-grid quotient used by `sub_13B00`.  The recovered source
/// applies `my_sign16` before shifting, so the unsigned world coordinate must
/// first be reinterpreted at its original signed 16-bit width.
fn level2_nyphur_sub_13b00_grid_quadrant(coordinate: u16) -> u8 {
    let coordinate = i32::from(i16::from_ne_bytes(coordinate.to_ne_bytes()));
    let sign = if coordinate < 0 { -1_i32 } else { 0_i32 };
    ((coordinate - (sign << 14) - sign) >> 14) as u8
}

/// Pinned `sub_580E0`. The fourth source argument is intentionally absent:
/// the recovered helper never reads it. The Z write and comparison retain the
/// source `i16` carrier at both boundaries.
fn level2_nyphur_sub_580e0(position: &mut Position, ground: i16, lower: i16, step: i16) {
    if position.z > ground {
        position.z = position.z.wrapping_add(step);
    }
    if i32::from(position.z) <= i32::from(ground) + i32::from(lower) {
        position.z = ground.wrapping_add(lower);
    }
}

/// Pinned `sub_583B0`: a non-wrapping Chebyshev distance over X/Y. This is a
/// separate probe from the signed-16 Euclidean helper below.
fn level2_nyphur_source_distance_xy_max(left: Position, right: Position) -> i32 {
    let dx = (i32::from(right.x) - i32::from(left.x)).abs();
    let dy = (i32::from(right.y) - i32::from(left.y)).abs();
    dx.max(dy)
}

/// Pinned `Maths::EuclideanDistXY_584D0`: each coordinate delta narrows to a
/// signed 16-bit value before the signed 32-bit square/add carrier is exposed
/// as `uint32_t`.
fn level2_nyphur_source_distance_xy_euclidean_sq(left: Position, right: Position) -> u32 {
    let dx = i32::from(i16::from_ne_bytes(
        right.x.wrapping_sub(left.x).to_ne_bytes(),
    ));
    let dy = i32::from(i16::from_ne_bytes(
        right.y.wrapping_sub(left.y).to_ne_bytes(),
    ));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

const LEVEL2_NYPHUR_SUB_133B0_CASTLE_DISTANCE_SQ: u32 = 0x0640_0000;

/// The production `sub_133B0` branch predicate. Keep the source threshold in
/// one carrier so the equality boundary is testable without duplicating an
/// unused expected literal.
fn level2_nyphur_sub_133b0_castle_is_far(source: Position, castle: Position) -> bool {
    level2_nyphur_source_distance_xy_euclidean_sq(source, castle)
        > LEVEL2_NYPHUR_SUB_133B0_CASTLE_DISTANCE_SQ
}

/// Pinned `watcominitrandnext`: the source helper returns the address of its
/// process-local 32-bit state. Passing the state explicitly keeps that same
/// pointer contract without introducing a second global simulation stream.
#[cfg(test)]
fn level2_nyphur_watcominitrandnext(state: &mut u32) -> &mut u32 {
    state
}

/// Pinned Watcom `rand()` step used by the source AI closure. The state is a
/// wrapping 32-bit carrier; the returned value is the source's 15-bit slice.
#[cfg(test)]
fn level2_nyphur_watcomrand(state: &mut u32) -> u16 {
    let state = level2_nyphur_watcominitrandnext(state);
    *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
    ((*state >> 16) & 0x7fff) as u16
}

/// Pinned `sub_583F0_distance_3d`: the three coordinate differences are
/// signed 16-bit before the legacy integer square-root pass.
fn level2_nyphur_source_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(i16::from_ne_bytes(
        right.x.wrapping_sub(left.x).to_ne_bytes(),
    ));
    let dy = i32::from(i16::from_ne_bytes(
        right.y.wrapping_sub(left.y).to_ne_bytes(),
    ));
    let dz = i32::from(right.z.wrapping_sub(left.z));
    let squared = dx
        .wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .wrapping_add(dz.wrapping_mul(dz));
    crate::legacy_sqrt(squared as u32)
}

/// Pinned `sub_11CB0`: inspect the two source linked lists first, then the
/// eight-by-eight map-angle aperture. The source uses the lists rebuilt by
/// `UpdateEntities_57730`; a vector-order sweep is a different operation.
fn level2_nyphur_map_angle_allowed(
    terrain: &SessionTerrain,
    entities: &[EntitySnapshot],
    position: Position,
) -> bool {
    let blocked_by = |candidate: &EntitySnapshot| {
        let dx = i16::from_ne_bytes(candidate.position.x.wrapping_sub(position.x).to_ne_bytes());
        let dy = i16::from_ne_bytes(candidate.position.y.wrapping_sub(position.y).to_ne_bytes());
        i32::from(dx).abs() <= i32::from(candidate.array_pitch()) + 2_560
            && i32::from(dy).abs() <= i32::from(candidate.array_roll()) + 2_560
    };
    if level2_nyphur_class3_source_order(entities)
        .into_iter()
        .filter_map(|index| entities.get(usize::from(index)))
        .filter(|entity| entity.model == 2)
        .any(blocked_by)
    {
        return false;
    }
    if level2_nyphur_class45_source_order(entities)
        .into_iter()
        .filter_map(|index| entities.get(usize::from(index)))
        .any(blocked_by)
    {
        return false;
    }
    let start_x = position.x.to_be_bytes()[0].wrapping_sub(8);
    let start_y = position.y.to_be_bytes()[0].wrapping_sub(8);
    for row in 0..8_u8 {
        for column in 0..8_u8 {
            if !level2_nyphur_sub_11c80(
                terrain,
                start_x.wrapping_add(column),
                start_y.wrapping_add(row),
            ) {
                return false;
            }
        }
    }
    true
}

/// Production carrier for the source AI dispatch state byte and common
/// `sub_12A70` state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Level2NyphurDispatcher {
    state_byte: i8,
    pub(super) common_upkeep: Level2NyphurCommonUpkeep,
}

impl Level2NyphurDispatcher {
    /// `sub_5C950`/the model-1 reset path clears byte `0x1C1` at the pinned
    /// `EventsFunctions.cpp:43893` seam.  The qualified package does not
    /// serialize the 1,136-byte `Type_str_164`; derive only this reset value
    /// after validating the source-bound participant chain.
    pub(super) fn from_qualified_reset(
        players: &[PlayerSnapshot],
        entities: &[EntitySnapshot],
        runtime: &RuntimeSnapshot,
        mission: &MissionContentSession,
    ) -> Result<Self> {
        let ai_player = players
            .get(1)
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur AI player row 1 is absent"))?;
        if ai_player.type_str_164_auxiliary().is_none() {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur AI player Type_str_164 carrier is absent",
            ));
        }
        let baseline_entity = entities
            .get(usize::from(LEVEL2_NYPHUR_ENTITY_INDEX))
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur entity row is absent"))?;
        let participant = level2_nyphur_participant(players, entities, runtime, baseline_entity)?;
        if participant.entity_model != 1 || participant.entity_action != 1 {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur dispatcher requires the qualified model-1 action-1 row",
            ));
        }
        Ok(Self {
            state_byte: 0,
            common_upkeep: Level2NyphurCommonUpkeep::from_qualified_reset(
                participant,
                players,
                entities,
                mission,
            )?,
        })
    }

    #[must_use]
    pub(super) const fn state_byte(&self) -> i8 {
        self.state_byte
    }

    /// The source `Type_str_164::byte_0x154_340` carrier is advanced by the
    /// common callback and consumed by the two ordinary spell producers.
    pub(super) const fn source_byte_0x154(&self) -> u8 {
        self.common_upkeep.byte_0x154
    }

    /// Clear the source byte only after a producer has committed its shared
    /// class-9 allocation and launch event.
    pub(super) const fn clear_source_byte_0x154(&mut self) {
        self.common_upkeep.byte_0x154 = 0;
    }

    /// Apply the class-5 contact feedback written by source `sub_237B0` to
    /// the one player record owned by this dispatcher. Non-player entity
    /// indices are deliberately ignored rather than creating a second
    /// authority for the player field in the class-5 arena.
    pub(super) fn apply_class5_player_feedback(&mut self, entity_index: u16) -> bool {
        if self.common_upkeep.source_slot != entity_index {
            return false;
        }
        self.common_upkeep.word_0x248 = 200;
        true
    }

    /// Execute the production `sub_12910` carrier. The selected state is
    /// latched after the common callback, exactly where the source switch
    /// reads it; source case zero invokes the tail twice even if the first
    /// tail call changes the next-turn state byte.
    /// The owned mode wrappers run between the common callback and the shared
    /// tail; their exact source symbol and call position are retained in the
    /// returned trace.
    pub(super) fn step_dispatch(
        &mut self,
        pass: u8,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
        move_speed_flag: u8,
        active_level_color_index: u8,
    ) -> Result<(Level2NyphurCommonUpkeepSnapshot, Level2NyphurDispatchTrace)> {
        let snapshot = self.common_upkeep.step(
            source_slot,
            entities,
            terrain,
            presentation,
            move_speed_flag,
            active_level_color_index,
        )?;
        // `sub_12910` reads byte 0x1C1 only after `sub_12A70` returns. The
        // source-state latch therefore belongs between the common callback
        // and the mode switch, not before the common callback.
        let selected_state = self.state_byte;
        match selected_state {
            1 => self.step_sub_12ff0(source_slot, entities, terrain)?,
            // The pinned program contains this structural `_nmemneed` case,
            // but no assignment of byte 0x1C1 to value 2. Preserve the call
            // in the trace map without inventing a production transition.
            2 => Self::step_nmemneed(),
            3 => self.step_sub_13100(source_slot, entities, terrain)?,
            4 => self.step_sub_131f0(source_slot, entities, terrain)?,
            5 => Self::step_nmemneed0(),
            8 => self.step_sub_13830(source_slot, entities, players, terrain, presentation)?,
            9 => self.step_sub_13870(source_slot, entities, players, terrain, presentation)?,
            11 => self.step_sub_133b0(source_slot, entities, terrain)?,
            12 => self.step_sub_13270(source_slot, entities, terrain)?,
            13 => self.step_sub_13850(source_slot, entities, players, terrain, presentation)?,
            14 => self.step_sub_161a0(source_slot, entities, players, terrain)?,
            _ => {}
        }
        let tail_calls = if selected_state == 0 { 2 } else { 1 };
        for _ in 0..tail_calls {
            self.step_sub_12e70(source_slot, entities, players)?;
        }
        Ok((
            snapshot,
            Self::trace_for_state(selected_state, pass, source_slot),
        ))
    }

    /// The pinned case-5 helper is `_nmemneed_0`, a weak stub that returns zero
    /// after its diagnostic hook. Its result is ignored by `sub_12910`; the
    /// observable production behavior is therefore the following shared tail,
    /// with no AI, movement, or RNG write at this call site.
    const fn step_nmemneed0() {}

    /// The pinned case-2 helper is the other weak memory hook. The complete
    /// source state-byte assignment set never writes value 2, so this body is
    /// retained only for an externally reconstructed structural trace.
    const fn step_nmemneed() {}

    /// Pinned `sub_12FF0` (AI_HELPER-0011, source line 5579). The selected
    /// target must still satisfy the paired slot/identity relation. Its
    /// inclusive 512 near gate, exclusive 2048 far gate, family-2 tier scan,
    /// activation polarity, signed vertical step, and `sub_16580` tail remain
    /// in source order.
    fn step_sub_12ff0(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_12FF0 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let target = entities
            .get(usize::from(source.source_word_96()))
            .cloned()
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_12FF0 target row is absent")
            })?;
        if !level2_nyphur_source_identity_matches(&source, &target) {
            return self.step_sub_16580(&source, terrain);
        }

        let roll = i16::try_from(heading_for_positions(source.position, target.position))
            .expect("Nyphur sub_12FF0 roll fits the signed source angle carrier");
        entities[source_index].set_source_roll(roll);
        if !self.step_sub_14c90(source_index, Some(&target), None, 512, 2_048, entities)? {
            return self.step_sub_16580(&entities[source_index], terrain);
        }

        let mut tier = self.common_upkeep.spell_levels[2];
        while tier >= 0 {
            if self.common_upkeep.step_sub_15f20(2, tier, entities) == 2 {
                break;
            }
            tier -= 1;
        }
        let mut source_control_flags = entities[source_index].control_flags;
        let activated = self.common_upkeep.step_sub_14e10(
            2,
            entities[source_index].model,
            &mut source_control_flags,
            entities,
        );
        entities[source_index].set_control_flags(source_control_flags);
        if !activated {
            self.common_upkeep.speed = 0;
            let vertical_delta =
                i32::from(entities[source_index].position.z) - (i32::from(target.position.z) + 512);
            let direction = if vertical_delta < 0 {
                -1_i16
            } else if vertical_delta > 0 {
                1_i16
            } else {
                0_i16
            };
            let mut position = entities[source_index].position;
            position.z = position
                .z
                .wrapping_add(direction.wrapping_mul(self.common_upkeep.word_0xe_14));
            entities[source_index].set_position(position);
        }
        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// Execute the pinned `sub_14C90` distance gate used by the four mode
    /// bodies owned here.  The source clears `word_0xe_14` before either the
    /// target or axis comparison; a far target first offers family 3, and
    /// only the failed offer falls through to minimum-speed movement.
    fn step_sub_14c90(
        &mut self,
        source_index: usize,
        target: Option<&EntitySnapshot>,
        axis: Option<Position>,
        near_distance: u32,
        far_distance: u32,
        entities: &mut [EntitySnapshot],
    ) -> Result<bool> {
        let source = entities
            .get(source_index)
            .cloned()
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur distance source row is absent"))?;
        let comparison_position = match (target, axis) {
            (Some(target), None) => target.position,
            (None, Some(axis)) => axis,
            _ => {
                return Err(NativeWorldError::new(
                    "Level 2 Nyphur distance gate has an invalid comparison carrier",
                ));
            }
        };
        self.common_upkeep.word_0xe_14 = 0;
        let distance = level2_nyphur_source_distance_3d(source.position, comparison_position);
        if distance <= near_distance {
            self.common_upkeep.speed = 0;
            self.common_upkeep.word_0xe_14 = 1;
            return Ok(true);
        }

        if distance > far_distance && self.source_spell_ready(3, entities) {
            if !self.common_upkeep.sub_156f0(3, entities) {
                let mut source_control_flags = source.control_flags;
                let _ = self.common_upkeep.step_sub_14e10(
                    3,
                    source.model,
                    &mut source_control_flags,
                    entities,
                );
                entities[source_index].set_control_flags(source_control_flags);
            }
        } else {
            self.common_upkeep.speed = source.minimum_speed();
            self.common_upkeep.word_0xe_14 = 1;
        }
        Ok(false)
    }

    /// Pinned `sub_15FC0`: admit advanced mode only through the source-owned
    /// family-4 holder, preserve the existing target relation when its active
    /// holder already owns it, otherwise select the nearest class-3/model-0/1
    /// primary and then the nearest candidate in `unk_D3F91x` bucket order.
    /// The direct SetSpell write precedes the source slot/identity write just
    /// as in the C++ helper; every malformed carrier fails closed.
    fn step_sub_15fc0(&mut self, source_slot: u16, entities: &mut [EntitySnapshot]) -> bool {
        let Some(source_index) = entities
            .iter()
            .position(|entity| entity.index == source_slot)
        else {
            return false;
        };
        let source = entities[source_index].clone();
        if self.common_upkeep.source_family4_holder(entities).is_none() {
            return false;
        }
        let target_matches = entities
            .get(usize::from(source.source_word_96()))
            .is_some_and(|target| level2_nyphur_source_identity_matches(&source, target));
        if self.common_upkeep.sub_156f0(4, entities) && target_matches {
            return true;
        }

        let mut nearest_primary = None;
        let mut nearest_primary_distance = u32::MAX;
        for candidate_index in level2_nyphur_class3_source_order(entities) {
            let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                continue;
            };
            if candidate.source_id == source.source_id || !matches!(candidate.model, 0 | 1) {
                continue;
            }
            let distance = level2_nyphur_source_distance_3d(source.position, candidate.position);
            if distance < nearest_primary_distance {
                nearest_primary = Some(candidate.index);
                nearest_primary_distance = distance;
            }
        }
        // The source accepts equality at 0x1400 for the primary class-3
        // target, but the selected class-5 bucket is strict below 0x1400.
        if !level2_nyphur_mode14_primary_admitted(nearest_primary_distance) {
            nearest_primary = None;
        }
        let Some(primary_index) = nearest_primary else {
            return false;
        };
        let Some(primary) = entities.get(usize::from(primary_index)) else {
            return false;
        };

        let mut nearest_bucket = None;
        let mut nearest_bucket_distance = u32::MAX;
        for (candidate_index, bucket_model) in level2_nyphur_sub_15fc0_class5_source_order(entities)
        {
            let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                continue;
            };
            if candidate.source_id == source.source_id {
                continue;
            }
            let distance = level2_nyphur_source_distance_3d(primary.position, candidate.position);
            if distance < nearest_bucket_distance {
                nearest_bucket = Some((candidate.index, bucket_model));
                nearest_bucket_distance = distance;
            }
        }
        if !level2_nyphur_mode14_bucket_admitted(nearest_bucket_distance) {
            return false;
        }
        let Some((target_index, bucket_model)) = nearest_bucket else {
            return false;
        };
        let spell_tier = match bucket_model {
            2 | 0x13 => 0,
            0x19 => 1,
            0x10 => 2,
            _ => return false,
        };
        if !self
            .common_upkeep
            .step_set_spell_profile(4, spell_tier, entities)
        {
            return false;
        }
        let Some(target) = entities.get(usize::from(target_index)).cloned() else {
            return false;
        };
        entities[source_index].set_source_relation(&target);
        true
    }

    /// Pinned `sub_161A0`: activate the family-4 handoff, perform the two
    /// source vertical corrections and family-4 random roll perturbation,
    /// then choose the strict `(0xA00, 0x1400)` class-3 attack band before
    /// falling through to the nearest-target approach branch and `sub_16580`.
    fn step_sub_161a0(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_161A0 source row is absent")
            })?;
        let source = entities[source_index].clone();
        if self.common_upkeep.source_family4_holder(entities).is_none() {
            return self.step_sub_16580(&source, terrain);
        }
        let Some(target) = entities.get(usize::from(source.source_word_96())).cloned() else {
            return self.step_sub_16580(&source, terrain);
        };
        if !level2_nyphur_source_identity_matches(&source, &target) {
            return self.step_sub_16580(&source, terrain);
        }

        let mut source_control_flags = source.control_flags;
        let _ =
            self.common_upkeep
                .step_sub_14e10(4, source.model, &mut source_control_flags, entities);
        entities[source_index].set_control_flags(source_control_flags);

        let initial_roll = i16::try_from(heading_for_positions(source.position, target.position))
            .expect("Nyphur sub_161A0 roll fits the signed source angle carrier");
        entities[source_index].set_source_roll(initial_roll);
        for _ in 0..2 {
            let current = entities[source_index].position;
            let vertical_delta = i32::from(current.z) - (i32::from(target.position.z) + 512);
            let direction = vertical_delta.signum() as i16;
            let mut position = current;
            position.z = position
                .z
                .wrapping_add(direction.wrapping_mul(self.common_upkeep.word_0xe_14));
            entities[source_index].set_position(position);
            let roll = i16::try_from(heading_for_positions(position, target.position))
                .expect("Nyphur sub_161A0 roll fits the signed source angle carrier");
            entities[source_index].set_source_roll(roll);
        }

        let family4_holder = self.common_upkeep.source_family4_holder(entities).cloned();
        if family4_holder
            .as_ref()
            .is_some_and(|holder| holder.level_gate == 0)
        {
            let next_rng = entities[source_index]
                .entity_rng
                .wrapping_mul(9_377)
                .wrapping_add(9_439);
            entities[source_index].set_entity_rng(next_rng);
            let roll_sign = 2 * (i32::from(next_rng % 0x9d) / 79);
            let next_rng = next_rng.wrapping_mul(9_377).wrapping_add(9_439);
            entities[source_index].set_entity_rng(next_rng);
            let current_roll = i32::from(entities[source_index].source_roll());
            let perturbed_roll =
                (current_roll + (roll_sign - 1) * i32::from(next_rng % 0x55)) & 0x07ff;
            entities[source_index].set_source_roll(
                i16::try_from(perturbed_roll).expect("masked Nyphur roll fits i16"),
            );
        }

        let mut nearest_primary = None;
        let mut nearest_primary_distance = u32::MAX;
        for candidate_index in level2_nyphur_class3_source_order(entities) {
            let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                continue;
            };
            if candidate.source_id == source.source_id || !matches!(candidate.model, 0 | 1) {
                continue;
            }
            let distance = level2_nyphur_source_distance_3d(
                entities[source_index].position,
                candidate.position,
            );
            if distance < nearest_primary_distance {
                nearest_primary = Some(candidate.index);
                nearest_primary_distance = distance;
            }
        }

        if let Some(candidate_index) =
            nearest_primary.filter(|_| level2_nyphur_mode14_attack_band(nearest_primary_distance))
        {
            let candidate = entities
                .get(usize::from(candidate_index))
                .cloned()
                .ok_or_else(|| {
                    NativeWorldError::new("Level 2 Nyphur sub_161A0 target row is absent")
                })?;
            entities[source_index].set_source_relation(&candidate);
            let family = self
                .common_upkeep
                .step_sub_15790(candidate.index, entities, players);
            self.common_upkeep.speed = 0;
            self.common_upkeep.word_0xe_14 = 1;
            if family >= 0
                && self.common_upkeep.step_sub_14e10(
                    family.cast_unsigned(),
                    source.model,
                    &mut source_control_flags,
                    entities,
                )
            {
                entities[source_index].set_control_flags(source_control_flags);
                return self.step_sub_16580(&entities[source_index], terrain);
            }
        } else {
            let target_position = nearest_primary
                .and_then(|index| {
                    entities
                        .get(usize::from(index))
                        .map(|entity| entity.position)
                })
                .unwrap_or_default();
            let roll = i16::try_from(heading_for_positions(
                entities[source_index].position,
                target_position,
            ))
            .expect("Nyphur sub_161A0 roll fits the signed source angle carrier");
            entities[source_index].set_source_roll(roll);
            let vertical_delta =
                i32::from(entities[source_index].position.z) - (i32::from(target_position.z) + 512);
            let direction = vertical_delta.signum() as i16;
            let mut position = entities[source_index].position;
            position.z = position
                .z
                .wrapping_add(direction.wrapping_mul(self.common_upkeep.word_0xe_14));
            entities[source_index].set_position(position);
        }

        if let Some(holder) = family4_holder {
            let speed = if holder.level_gate != 0 {
                entities[source_index].minimum_speed()
            } else {
                entities[source_index].minimum_speed().wrapping_mul(3)
            };
            entities[source_index].set_actual_speed(speed);
        }
        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// The bounded Level 2 carrier for the source `sub_15170` predicate.
    /// Unsupported spell families remain absent instead of borrowing a
    /// neighboring profile.
    fn source_spell_ready(&self, family: u8, entities: &[EntitySnapshot]) -> bool {
        let Some(holder) = self.common_upkeep.sub_146c0(family, entities) else {
            return false;
        };
        let Some((_, profile)) =
            Level2NyphurCommonUpkeep::source_spell_profile(family, holder.level_gate)
        else {
            return false;
        };
        self.common_upkeep
            .source_spell_available(family, holder, profile, entities)
    }

    /// Pinned `sub_13100` (AI_HELPER-0012, source line 5620).  Keep its
    /// source roll, 2048/4096 gate, descending family-2 tier loop, activation
    /// attempt, and signed vertical step in source order.
    fn step_sub_13100(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_13100 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let axis = level2_nyphur_axis_position(&source)?;
        let roll = i16::try_from(heading_for_positions(source.position, axis))
            .expect("Nyphur sub_13100 roll fits the signed source angle carrier");
        entities[source_index].set_source_roll(roll);

        if !self.step_sub_14c90(source_index, None, Some(axis), 2_048, 4_096, entities)? {
            return self.step_sub_16580(&entities[source_index], terrain);
        }

        let mut tier = self.common_upkeep.spell_levels[2];
        while tier >= 0 {
            if self.common_upkeep.step_sub_15f20(2, tier, entities) == 2 {
                break;
            }
            tier -= 1;
        }
        let mut source_control_flags = entities[source_index].control_flags;
        if self.common_upkeep.step_sub_14e10(
            2,
            entities[source_index].model,
            &mut source_control_flags,
            entities,
        ) {
            entities[source_index].set_control_flags(source_control_flags);
            return self.step_sub_16580(&entities[source_index], terrain);
        }
        entities[source_index].set_control_flags(source_control_flags);

        self.common_upkeep.speed = 0;
        let vertical_delta = i32::from(source.position.z) - (i32::from(axis.z) + 512);
        let direction = if vertical_delta < 0 {
            -1_i16
        } else if vertical_delta > 0 {
            1_i16
        } else {
            0_i16
        };
        let mut position = entities[source_index].position;
        position.z = position
            .z
            .wrapping_add(direction.wrapping_mul(self.common_upkeep.word_0xe_14));
        entities[source_index].set_position(position);
        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// Pinned `sub_131F0` (AI_HELPER-0013, source line 5658).  Its roll is
    /// deliberately computed against the source position itself; this keeps
    /// the source's unusual zero-heading expression rather than substituting
    /// the stored target heading.
    fn step_sub_131f0(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_131F0 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let target = entities
            .get(usize::from(source.source_word_96()))
            .cloned()
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_131F0 target row is absent")
            })?;
        if !level2_nyphur_source_identity_matches(&source, &target) {
            // The pinned source treats a stale identity as the false result
            // of sub_14C60: it skips the roll/distance work, tails once
            // through sub_16580, and returns zero.  Only an absent indexed
            // carrier above is a package-boundary error.
            self.step_sub_16580(&source, terrain)?;
            return Ok(());
        }
        let roll = i16::try_from(heading_for_positions(source.position, source.position))
            .expect("Nyphur sub_131F0 roll fits the signed source angle carrier");
        entities[source_index].set_source_roll(roll);
        self.step_sub_14c90(source_index, Some(&target), None, 256, 2_048, entities)?;
        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// Pinned `sub_13270` (AI_HELPER-0014, source line 5680).  The family-22
    /// probe intentionally passes the constant spell-level carrier `v5`, not
    /// the loop variable; the source's draw and branch order are preserved.
    fn step_sub_13270(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_13270 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let perception = self.common_upkeep.word_0x244;
        let perception_roll = self.common_upkeep.step_watcomrand() % 255;
        let mut source_control_flags = source.control_flags;
        if i32::from(perception_roll) < i32::from(perception) {
            let castle = if self.common_upkeep.castle_entity_index == 0 {
                None
            } else {
                Some(
                    entities
                        .get(usize::from(self.common_upkeep.castle_entity_index))
                        .cloned()
                        .ok_or_else(|| {
                            NativeWorldError::new("Level 2 Nyphur sub_13270 castle row is absent")
                        })?,
                )
            };
            let castle_overlap = castle.as_ref().is_some_and(|castle| {
                super::level2_model76_sub106c0_matches(
                    source.position,
                    source.array_yaw(),
                    source.array_pitch(),
                    source.array_roll(),
                    source.array_fov(),
                    castle.position,
                    castle.array_yaw(),
                    castle.array_pitch(),
                    castle.array_roll(),
                    castle.array_fov(),
                )
            });
            let family_two_tier = self.common_upkeep.spell_levels[2];
            if !castle_overlap {
                let mut tier = family_two_tier;
                while tier >= 0 {
                    // This is intentionally `family_two_tier`, matching the
                    // pinned `sub_13270` body rather than the apparent loop typo.
                    if self
                        .common_upkeep
                        .step_sub_15f20(22, family_two_tier, entities)
                        == 22
                    {
                        let _ = self.common_upkeep.step_sub_14e10(
                            22,
                            source.model,
                            &mut source_control_flags,
                            entities,
                        );
                        entities[source_index].set_control_flags(source_control_flags);
                        return self.step_sub_16580(&entities[source_index], terrain);
                    }
                    tier -= 1;
                }
            }
        }

        if self.source_spell_ready(3, entities) {
            if !self.common_upkeep.sub_156f0(3, entities) {
                let _ = self.common_upkeep.step_sub_14e10(
                    3,
                    source.model,
                    &mut source_control_flags,
                    entities,
                );
            }
            entities[source_index].set_control_flags(source_control_flags);
            return self.step_sub_16580(&entities[source_index], terrain);
        }

        self.step_sub_16580(&source, terrain)?;
        if self.common_upkeep.sub_15760(3, entities).is_some() {
            return Ok(());
        }
        self.common_upkeep.speed = source.minimum_speed();
        self.common_upkeep.word_0xe_14 = 1;
        Ok(())
    }

    /// Pinned `sub_133B0` (AI_HELPER-0015, source line 5745).  The no-castle
    /// and castle branches retain their independent descending spell scans,
    /// strict identity check, squared XY threshold, and post-helper terrain
    /// call order.
    fn step_sub_133b0(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        terrain: &SessionTerrain,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_133B0 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let mut source_control_flags = source.control_flags;
        let castle = if self.common_upkeep.castle_entity_index == 0 {
            None
        } else {
            Some(
                entities
                    .get(usize::from(self.common_upkeep.castle_entity_index))
                    .cloned()
                    .ok_or_else(|| {
                        NativeWorldError::new("Level 2 Nyphur sub_133B0 castle row is absent")
                    })?,
            )
        };

        let Some(castle) = castle else {
            let mut tier = self.common_upkeep.spell_levels[11];
            while tier >= 0 {
                if self.common_upkeep.step_sub_15f20(11, tier, entities) == 11 {
                    let _ = self.common_upkeep.step_sub_14e10(
                        11,
                        source.model,
                        &mut source_control_flags,
                        entities,
                    );
                    break;
                }
                tier -= 1;
            }

            let mut tier = self.common_upkeep.spell_levels[3];
            while tier >= 0 {
                if self.common_upkeep.step_sub_15f20(3, tier, entities) == 3 {
                    let _ = self.common_upkeep.step_sub_14e10(
                        3,
                        source.model,
                        &mut source_control_flags,
                        entities,
                    );
                    entities[source_index].set_control_flags(source_control_flags);
                    self.step_sub_16580(&entities[source_index], terrain)?;
                    return Ok(());
                }
                tier -= 1;
            }
            entities[source_index].set_control_flags(source_control_flags);
            if self.common_upkeep.sub_15760(3, entities).is_some() {
                return self.step_sub_16580(&entities[source_index], terrain);
            }
            self.common_upkeep.speed = source.minimum_speed();
            self.common_upkeep.word_0xe_14 = 1;
            return self.step_sub_16580(&entities[source_index], terrain);
        };

        if level2_nyphur_sub_133b0_castle_is_far(source.position, castle.position) {
            let mut tier = self.common_upkeep.spell_levels[1];
            while tier >= 0 {
                if self.common_upkeep.step_sub_15f20(1, tier, entities) == 1
                    && self.common_upkeep.step_sub_14e10(
                        1,
                        source.model,
                        &mut source_control_flags,
                        entities,
                    )
                {
                    entities[source_index].set_control_flags(source_control_flags);
                    self.step_sub_16580(&entities[source_index], terrain)?;
                    return Ok(());
                }
                tier -= 1;
            }
        }

        let mut tier = self.common_upkeep.spell_levels[11];
        while tier >= 0 {
            if self.common_upkeep.step_sub_15f20(11, tier, entities) == 11 {
                let _ = self.common_upkeep.step_sub_14e10(
                    11,
                    source.model,
                    &mut source_control_flags,
                    entities,
                );
                break;
            }
            tier -= 1;
        }

        if level2_nyphur_source_identity_matches(&source, &castle) {
            let roll = i16::try_from(heading_for_positions(source.position, castle.position))
                .expect("Nyphur sub_133B0 roll fits the signed source angle carrier");
            entities[source_index].set_source_roll(roll);
            // Materialize every preceding sub_14E10 write before the
            // distance helper snapshots the source.  A later family-3
            // activation must extend, rather than be overwritten by, the
            // family-11 local flag carrier.
            entities[source_index].set_control_flags(source_control_flags);
            let close =
                self.step_sub_14c90(source_index, Some(&castle), None, 256, 2_048, entities)?;
            source_control_flags = entities[source_index].control_flags;
            entities[source_index].set_control_flags(source_control_flags);
            self.step_sub_16580(&entities[source_index], terrain)?;
            if close {
                return Ok(());
            }
            return Ok(());
        }

        entities[source_index].set_control_flags(source_control_flags);
        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// `sub_13830` is a source wrapper around `sub_13890`.
    fn step_sub_13830(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
    ) -> Result<()> {
        self.step_sub_13890(source_slot, entities, players, terrain, presentation)
    }

    /// `sub_13870` is a source wrapper around `sub_13890`; its nullsub return
    /// adaptation does not add a simulation-side write.
    fn step_sub_13870(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
    ) -> Result<()> {
        self.step_sub_13890(source_slot, entities, players, terrain, presentation)
    }

    /// `sub_13850` is a source wrapper around `sub_13890`.
    fn step_sub_13850(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
    ) -> Result<()> {
        self.step_sub_13890(source_slot, entities, players, terrain, presentation)
    }

    /// Execute the shared body reached by pinned `sub_13830`, `sub_13870`,
    /// and `sub_13850` in source order:
    /// `sub_13890 -> sub_14C90/sub_15170 -> sub_16580`, then let
    /// `step_dispatch` enter the shared `sub_12E70` tail. The qualified
    /// Level 2 package has no family-3 holder/carrier, while direct family-0/1
    /// rows and the pinned family-3 Speed Up row are source-backed by the
    /// fresh-start profiles. A newly live unsupported family still fails
    /// closed rather than borrowing an unrelated spell profile.
    fn step_sub_13890(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
        terrain: &SessionTerrain,
        presentation: &PresentationSelectors,
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_13890 source row is absent")
            })?;
        let source = entities[source_index].clone();
        let target = entities
            .get(usize::from(source.source_word_96()))
            .cloned()
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur state-13 target row is absent"))?;
        if !level2_nyphur_source_identity_matches(&source, &target) {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur sub_13890 target identity does not match",
            ));
        }

        let roll = i16::try_from(crate::heading_for_positions(
            source.position,
            target.position,
        ))
        .expect("Nyphur sub_13890 roll fits the signed source angle carrier");
        entities[source_index].set_source_roll(roll);

        // `sub_14C90` returns one only for distance <= a3.  Its 3328..4608
        // branch writes minimum speed and returns zero; the >4608 branch
        // offers family-3 Speed Up before the same zero return.  Only the
        // returned-one path reaches this caller's family-0/1 selection.
        let close =
            self.step_sub_14c90(source_index, Some(&target), None, 3_328, 4_608, entities)?;
        if !close {
            return self.step_sub_16580(&entities[source_index], terrain);
        }
        if self.common_upkeep.word_0x1a2 >= 0 {
            let family =
                self.common_upkeep
                    .step_sub_15790(source.source_word_96(), entities, players);
            if family >= 0 && self.source_spell_ready(family.cast_unsigned(), entities) {
                let mut source_control_flags = source.control_flags;
                if self.common_upkeep.step_sub_14e10(
                    family.cast_unsigned(),
                    source.model,
                    &mut source_control_flags,
                    entities,
                ) {
                    entities[source_index].set_control_flags(source_control_flags);
                    return self.step_sub_16580(&entities[source_index], terrain);
                }
            }
        }

        if matches!(target.model, 0 | 1) {
            let counter = self.common_upkeep.byte_0x45d;
            if counter < 3 || counter >= 5 {
                if counter < 5 || counter >= 20 {
                    if counter < 20 {
                        if counter != 0 {
                            let turn = if self.common_upkeep.byte_0x45c == 1 {
                                -512
                            } else {
                                512
                            };
                            let source_roll =
                                entities[source_index].source_roll().wrapping_add(turn) & 0x07ff;
                            entities[source_index].set_source_roll(source_roll);
                            let actual_speed = (i32::from(entities[source_index].minimum_speed())
                                * 3
                                * i32::from(self.common_upkeep.word_0x246)
                                / 255) as i16;
                            entities[source_index].set_actual_speed(actual_speed);
                            self.common_upkeep.word_0xe_14 = 1;
                        } else {
                            let next_rng = entities[source_index]
                                .entity_rng
                                .wrapping_mul(9_377)
                                .wrapping_add(9_439);
                            entities[source_index].set_entity_rng(next_rng);
                            if next_rng % 0xFF >= 127 {
                                self.common_upkeep.byte_0x45c = 2;
                                let yaw = entities[source_index].yaw.wrapping_add(512) & 0x07ff;
                                entities[source_index].set_yaw(yaw);
                            } else {
                                self.common_upkeep.byte_0x45c = 1;
                                let yaw = entities[source_index].yaw.wrapping_sub(512) & 0x07ff;
                                entities[source_index].set_yaw(yaw);
                            }
                        }
                        self.common_upkeep.byte_0x45d = counter.wrapping_add(1);
                    } else {
                        self.common_upkeep.byte_0x45d = 0;
                    }
                } else {
                    self.common_upkeep.byte_0x45d = counter.wrapping_add(1);
                }
            } else {
                self.common_upkeep.byte_0x45d = counter.wrapping_add(1);
            }
        }

        let parameter_index = entities[source_index].parameter_relation.ok_or_else(|| {
            NativeWorldError::new("Level 2 Nyphur sub_13890 source parameter is absent")
        })?;
        let parameter = presentation
            .entity_parameters
            .get(usize::from(parameter_index))
            .ok_or_else(|| {
                NativeWorldError::new("Level 2 Nyphur sub_13890 parameter row is absent")
            })?;
        let vertical_step = i16::from_le_bytes([parameter[14], parameter[15]]);
        let vertical_delta =
            i32::from(entities[source_index].position.z) - (i32::from(target.position.z) + 512);
        let vertical_direction = vertical_delta.signum() as i16;
        let mut position = entities[source_index].position;
        position.z = position
            .z
            .wrapping_add(vertical_direction.wrapping_mul(vertical_step));
        entities[source_index].set_position(position);

        self.step_sub_16580(&entities[source_index], terrain)
    }

    /// Execute the qualified clear-terrain result of pinned `sub_16580` and
    /// its `sub_169C0` probe. The source's first `sub_13890` dispatch has zero
    /// `byte_0x45E` and no terrain-type-8 cardinal blocker, so both
    /// `sub_16730` probes return zero and `sub_169C0` leaves the carrier at
    /// zero. Preserve the source's exact byte-carrier reset/increment cases;
    /// a blocked path still fails closed because its later pathfinding
    /// carrier is not present in the qualified package.
    fn step_sub_16580(&mut self, source: &EntitySnapshot, terrain: &SessionTerrain) -> Result<()> {
        if self.common_upkeep.byte_0x45e > 2 && self.common_upkeep.byte_0x45e < 8 {
            self.common_upkeep.byte_0x45e = self.common_upkeep.byte_0x45e.wrapping_add(1);
            return Ok(());
        }
        if self.common_upkeep.byte_0x45e >= 8 {
            self.common_upkeep.byte_0x45e = 0;
            return Ok(());
        }

        let map_x = source.position.x.to_be_bytes()[0];
        let map_y = source.position.y.to_be_bytes()[0];
        let terrain_is_blocked =
            |x: u8, y: u8| terrain.planes.terrain_types[terrain_index(x, y)] == 8;
        let blocked = terrain_is_blocked(map_x, map_y.wrapping_sub(1))
            || terrain_is_blocked(map_x, map_y.wrapping_add(1))
            || terrain_is_blocked(map_x.wrapping_sub(1), map_y)
            || terrain_is_blocked(map_x.wrapping_add(1), map_y);
        if blocked {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur sub_16580 sub_169C0 reached an unsupported terrain turn",
            ));
        }
        Ok(())
    }

    /// Execute the common-tail selector owned by this packet. The qualified
    /// frame-zero route has no family-2 holder and no castle damage branch;
    /// when its source cadence gate is not due, the pinned helper is a true
    /// no-op. At a due cadence the owned `sub_14530` selector either chooses
    /// the exact class-5 target or the shared `sub_14630` state/relation
    /// fall-through; later mode bodies are deliberately not reconstructed
    /// here.
    fn step_sub_12e70(
        &mut self,
        source_slot: u16,
        entities: &mut [EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> Result<()> {
        let source_index = entities
            .iter()
            .position(|entity| entity.index == source_slot)
            .ok_or_else(|| NativeWorldError::new("Level 2 Nyphur tail source row is absent"))?;

        // sub_13B00 is the first selector in sub_12E70 and is intentionally
        // outside the cadence gate.  On success the source writes state 3,
        // skips every later selector, and returns through the common result.
        let source_snapshot = entities[source_index].clone();
        if let Some(axis) = self.sub_13b00_axis(&source_snapshot, entities) {
            level2_nyphur_set_axis_position(&mut entities[source_index], axis);
            self.state_byte = 3;
            return Ok(());
        }

        let castle = (self.common_upkeep.castle_entity_index != 0)
            .then(|| entities.get(usize::from(self.common_upkeep.castle_entity_index)))
            .flatten()
            .cloned();
        if self
            .common_upkeep
            .step_sub_13dc0(&mut entities[source_index], castle.as_ref())
        {
            self.state_byte = 11;
            return Ok(());
        }

        let source_phase = entities[source_index].phase;
        let period = self.common_upkeep.cadence_period();
        if u16::from(source_phase) % period != 0 {
            return Ok(());
        }

        // The Castle action-4 selector is the first cadence-gated owned row.
        // Its final `sub_155E0` dependency fails closed when no owned carrier
        // is available, without consuming a source relation or RNG draw.
        if self
            .common_upkeep
            .step_sub_13c50(&entities[source_index], castle.as_ref(), entities)
        {
            self.state_byte = 1;
            return Ok(());
        }

        // The selector bodies between `sub_13C50` and `sub_13CE0` are owned
        // by other Beads. This gate is source-shaped rather than a generic
        // fallback: it rejects the tail unless every unowned prerequisite is
        // mechanically absent for the current indexed Level 2 rows.
        if !self.owned_tail_fallthrough_is_safe(&entities[source_index], entities, players) {
            return Err(NativeWorldError::new(
                "Level 2 Nyphur tail reached an unowned selector prerequisite",
            ));
        }
        // Pinned `sub_15FC0` follows the model-2/model-3 selectors and
        // precedes `sub_13CE0`. Its family-4 carrier and target bucket are
        // handled by this lane; absent or malformed carriers fail closed.
        if self.step_sub_15fc0(source_slot, entities) {
            self.state_byte = 14;
            return Ok(());
        }
        if let Some(target_index) =
            self.common_upkeep
                .step_sub_13ce0(&entities[source_index], entities, players)
        {
            let target = entities
                .get(usize::from(target_index))
                .cloned()
                .ok_or_else(|| NativeWorldError::new("Level 2 sub_148E0 target row is absent"))?;
            entities[source_index].set_source_relation(&target);
            self.state_byte = 6;
            return Ok(());
        }
        let source_snapshot = entities[source_index].clone();
        if let Some(target_index) = self.sub_14530_target(&source_snapshot, entities) {
            let target = entities
                .get(usize::from(target_index))
                .cloned()
                .ok_or_else(|| NativeWorldError::new("Level 2 sub_14530 target row is absent"))?;
            entities[source_index].set_source_relation(&target);
            self.state_byte = 13;
            return Ok(());
        }
        let state = self
            .common_upkeep
            .step_sub_14630(&mut entities[source_index], castle.as_ref());
        self.state_byte = state;
        Ok(())
    }

    /// Pinned `sub_13B00`: when the AI has no Castle and its family-2 holder
    /// is affordable, scan the four-by-four wrapped arena in source order.
    /// Each cell is accepted only when the nearest live class-3/model-2 row
    /// with a different source id is absent or strictly farther than 0x3000
    /// by `sub_583B0`'s Chebyshev distance.
    fn sub_13b00_axis(
        &self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
    ) -> Option<Position> {
        if self.common_upkeep.castle_entity_index != 0 {
            return None;
        }
        if !self.common_upkeep.sub_15730(2, entities) {
            return None;
        }

        let sentinel_z = entities.first()?.position.z;
        let start_x = level2_nyphur_sub_13b00_grid_quadrant(source.position.x);
        let start_y = level2_nyphur_sub_13b00_grid_quadrant(source.position.y);
        let model_two = level2_nyphur_class3_source_order(entities)
            .into_iter()
            .filter_map(|index| entities.get(usize::from(index)))
            .filter(|candidate| candidate.source_id != source.source_id && candidate.model == 2)
            .collect::<Vec<_>>();

        for row in 0..4_u8 {
            for column in 0..4_u8 {
                let position = Position {
                    x: u16::from(start_x.wrapping_add(column) & 3) << 14,
                    y: u16::from(start_y.wrapping_add(row) & 3) << 14,
                    z: sentinel_z,
                };
                let mut nearest = None;
                let mut nearest_distance = u32::MAX;
                for candidate in &model_two {
                    let distance =
                        level2_nyphur_source_distance_xy_euclidean_sq(position, candidate.position);
                    if distance < nearest_distance {
                        nearest = Some(*candidate);
                        nearest_distance = distance;
                    }
                }
                if nearest.is_none_or(|candidate| {
                    level2_nyphur_source_distance_xy_max(candidate.position, position) > 0x3000
                }) {
                    return Some(position);
                }
            }
        }
        None
    }

    /// Pinned `sub_14530` is the last owned selector before `sub_14630`.
    /// `UpdateEntities_57730` rebuilds twenty-nine class-5 model buckets in
    /// pool order; a strict source-order nearest search then stores both the
    /// target slot and its `sub_14C40` identity. The later mode body remains
    /// outside this packet, but the shared tail must select this target before
    /// it can fall through.
    fn sub_14530_target(
        &self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
    ) -> Option<u16> {
        if !self.common_upkeep.sub_15e60() {
            return None;
        }
        let anchor = if self.common_upkeep.castle_entity_index != 0 {
            entities
                .get(usize::from(self.common_upkeep.castle_entity_index))
                .unwrap_or(source)
        } else {
            source
        };
        let mut best = None;
        let mut best_distance = u32::MAX;
        for candidate_index in level2_nyphur_class5_bucket_order(entities) {
            let Some(candidate) = entities.get(usize::from(candidate_index)) else {
                continue;
            };
            if candidate.source_id == source.source_id || candidate.mana <= 0 {
                continue;
            }
            let distance =
                level2_nyphur_source_distance_xy_euclidean_sq(anchor.position, candidate.position);
            if distance < best_distance {
                best = Some(candidate.index);
                best_distance = distance;
            }
        }
        best
    }

    /// Prove the fall-through branch of pinned `sub_12E70` without executing
    /// any mode-owned selector. Each condition mirrors a source selector's
    /// entry gate in source order; an unknown or newly live carrier fails
    /// closed instead of being silently sent through the common tail.
    fn owned_tail_fallthrough_is_safe(
        &self,
        source: &EntitySnapshot,
        entities: &[EntitySnapshot],
        players: &[PlayerSnapshot],
    ) -> bool {
        // `sub_13DC0` and `sub_13C50` have already run in source order. The
        // family-2 arena search is handled above by `sub_13B00`; the
        // remaining predicates below cover only selectors owned elsewhere.

        let source_order = level2_nyphur_class3_source_order(entities);
        // sub_13E40 and sub_14250 scan the source class-3 list for model 2 and
        // model 3 targets. They are not present in the qualified indexed
        // list, so their mode packets cannot be selected here.
        if source_order.iter().any(|index| {
            entities
                .get(usize::from(*index))
                .is_some_and(|entity| matches!(entity.model, 2 | 3))
        }) {
            return false;
        }

        // sub_14030 can select a class-3 model 0/1 target. Evaluate its
        // source threshold and family-2 carrier gates against the actual
        // player rows instead of assuming that a model-shaped row is inert.
        if self.common_upkeep.sub_15e60() {
            for candidate in source_order.iter().filter_map(|index| {
                entities.get(usize::from(*index)).filter(|candidate| {
                    candidate.index != source.index
                        && candidate.source_id != source.source_id
                        && matches!(candidate.model, 0 | 1)
                })
            }) {
                let Some(owner) = players
                    .iter()
                    .find(|player| player.entity_index == candidate.index)
                else {
                    return false;
                };
                let family_11_active = owner.enabled_spell_holders[11] != 0
                    && entities
                        .get(usize::from(owner.enabled_spell_holders[11]))
                        .is_some_and(|holder| holder.active_value > 0);
                if family_11_active {
                    continue;
                }
                let color = usize::from(owner.player_color_index());
                let threshold_index = 4 * color + 4;
                let Some(threshold) = (threshold_index < self.common_upkeep.array_0x1fc.len())
                    .then(|| self.common_upkeep.auxiliary_word(threshold_index))
                else {
                    return false;
                };
                if threshold == 1 {
                    return false;
                }
                let pressure = 50_000_i32
                    - candidate.max_mana / 10 * self.common_upkeep.word_0x242 as i32 / 255;
                if pressure <= i32::from(threshold) {
                    return false;
                }
                // The final sub_14030 alternative is gated by the candidate's
                // family-2 holder. A nonzero relation is enough to reject the
                // common fall-through; its mode packet owns the later action.
                if owner.enabled_spell_holders[2] != 0 {
                    return false;
                }
            }
        }

        // AI_HELPER-0075 (`sub_6DC40`) is mechanically excluded from this
        // tail: the pinned source calls it only from `sub_6D9C0` at
        // EventsFunctions.cpp:43950 and `sub_6DAD0` at :43998, the spell-level
        // transition handlers. This indexed common-tail route has no such
        // notification carrier, so it must not synthesize that later-mode
        // side effect.
        //
        // sub_15FC0 is handled immediately after this boundary. In
        // particular, a model-57 row must remain visible here so its
        // sub_148E0 Watcom draw occurs at the source call site.
        true
    }

    #[must_use]
    pub(super) fn common_upkeep(&self, phase: u8) -> Level2NyphurCommonUpkeepSnapshot {
        self.common_upkeep.snapshot(phase)
    }

    /// Pure selector used by the production carrier and boundary tests. It
    /// has no mutable state of its own, so later mode slices can reuse the
    /// exact branch map without introducing a test-only state setter.
    #[must_use]
    fn trace_for_state(state_byte: i8, pass: u8, source_slot: u16) -> Level2NyphurDispatchTrace {
        let mut calls = Vec::with_capacity(3);
        calls.push(Level2NyphurDispatchCall::Sub12a70);
        match state_byte {
            0 => {
                // Source case 0 calls sub_12E70 once for its side effect and
                // once more for the returned result.
                calls.push(Level2NyphurDispatchCall::Sub12e70);
                calls.push(Level2NyphurDispatchCall::Sub12e70);
            }
            1 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub12ff0,
            )),
            2 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Nmemneed,
            )),
            3 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13100,
            )),
            4 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub131f0,
            )),
            5 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Nmemneed0,
            )),
            6 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub135c0,
            )),
            7 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13710,
            )),
            8 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13830,
            )),
            9 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13870,
            )),
            11 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub133b0,
            )),
            12 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13270,
            )),
            13 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub13850,
            )),
            14 => calls.push(Level2NyphurDispatchCall::Helper(
                Level2NyphurDispatchHelper::Sub161a0,
            )),
            i8::MIN..=-1 | 10 | 15..=i8::MAX => {
                // Pinned default: no mode helper, then the common tail.
            }
        }
        if !matches!(state_byte, 0) {
            calls.push(Level2NyphurDispatchCall::Sub12e70);
        }
        Level2NyphurDispatchTrace {
            pass,
            source_slot,
            state_byte,
            calls: calls.into_boxed_slice(),
        }
    }
}

/// Source visibility decision for the authored Level 2 Nyphur participant.
///
/// `sub_63570` consumes the target owner's `byte_0x1BF_447` and the local
/// family-12 holder tier.  Keep the raw owner mode and the local holder gate
/// visible so presentation/AI consumers cannot silently replace the source
/// predicate with a generic "has spell" check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2NyphurVisibility {
    pub owner_visibility_mode: u8,
    pub beyond_sight_holder_present: bool,
    pub beyond_sight_active: bool,
    pub beyond_sight_tier: Option<u8>,
    pub family4_gate_active: bool,
    pub visible_to_local_player: bool,
}

#[cfg(test)]
#[path = "level2_nyphur_tests.rs"]
mod tests;
