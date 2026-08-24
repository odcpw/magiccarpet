use super::*;

/// A fail-closed package or closure validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeWorldError(String);

impl NativeWorldError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NativeWorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NativeWorldError {}

pub(super) type Result<T> = std::result::Result<T, NativeWorldError>;

/// The owner-package profile selects both the schema member set and the
/// external/internal level identity. It is deliberately typed so a Level 2
/// package cannot flow through the sealed Level 1 loader by accident.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerPackageProfile {
    Level1,
    Level2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerPackageSchema {
    V2,
    V3,
    V4,
    V5,
}

/// Resolve the single Level 1 presentation-cache root used by Rust fixtures.
///
/// `private_root` is the repository's `research/private` directory.  The
/// environment override is intentionally common to sim-core and sim-capi;
/// without it, the sealed PLAN29 final cache is selected.
#[must_use]
pub fn resolve_level1_presentation_cache_root(private_root: impl AsRef<Path>) -> PathBuf {
    std::env::var_os("MC_OWNER_ASSET_CACHE").map_or_else(
        || {
            private_root
                .as_ref()
                .join("generated")
                .join(FINAL_LEVEL1_CACHE_DIRECTORY)
        },
        PathBuf::from,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapType {
    Day,
    Night,
    Cave,
}

impl MapType {
    pub(super) fn decode(id: u8, name: &str) -> Result<Self> {
        match (id, name) {
            (0, "Day") => Ok(Self::Day),
            (1, "Night") => Ok(Self::Night),
            (2, "Cave") => Ok(Self::Cave),
            _ => Err(NativeWorldError::new(format!(
                "unsupported or inconsistent map type {id}/{name}"
            ))),
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        match self {
            Self::Day => 0,
            Self::Night => 1,
            Self::Cave => 2,
        }
    }

    #[must_use]
    pub const fn profile(self) -> FlightProfile {
        match self {
            Self::Day | Self::Night => FlightProfile::OUTDOOR,
            Self::Cave => FlightProfile::CAVE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelMetadata {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: MapType,
    pub capture_site: String,
    pub capture_frame: u32,
    pub vga_resolution_selector: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAuthority {
    Inactive,
    FrozenObserved,
    PlayerFlightLive,
    SpellActivationLive,
    GoatLive,
    VillagerLive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub index: u16,
    /// Original `id_0x1A_26`, used as the sample arbitration flag for
    /// positional sound IDs 37 and 39. This is distinct from the pool slot.
    pub source_id: u16,
    pub authority: EntityAuthority,
    pub active: bool,
    pub max_life: i32,
    pub life: i32,
    pub next_relation: Option<u16>,
    pub class: u8,
    pub model: u8,
    pub control_flags: u32,
    pub entity_rng: u16,
    pub action: u8,
    pub action_marker: i8,
    pub level_gate: i8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub fov: i16,
    pub phase: u8,
    /// Source `subSpellIndex_0x2A_42` (the low word exported at bytes
    /// 46..48 after the four-byte entity header). `SetSpell_6D5E0` carries
    /// this into allocated spell holders.
    pub subspell_index: u16,
    /// Source `byte_0x3B_59`, exported at byte 63 after the four-byte entity
    /// header and written by `SetSpell_6D5E0`.
    pub byte_3b: u8,
    /// Source `byte_0x3C_60`, reset by `SetSpell_6D5E0`.
    pub byte_3c: u8,
    /// Source `fontTypeIndex_0x3D_61`, reset by `SetSpell_6D5E0`.
    pub font_type_index: u8,
    pub active_value: i16,
    pub activation_value: u16,
    /// Source `manaRegen_0x88_136`, exported at bytes 140..144 after the
    /// four-byte entity header.
    pub mana_regen: i32,
    pub actual_speed: i16,
    /// Source `maxMana_0x8C_140`, exported at bytes 144..148. `mana_cost`
    /// remains as the historical activation-cost name for callers that use
    /// the lossless entity projection.
    pub max_mana: i32,
    pub mana_cost: i32,
    pub mana: i32,
    /// Signed source field at record offset 94. Active renderable rows use a
    /// non-negative index into the 347-row particle presentation table.
    pub particle_parameter_index: Option<u16>,
    pub parameter_relation: Option<u16>,
    pub special_relation: EntitySpecialRelation,
    pub(super) record: Box<[u8]>,
}

impl EntitySnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Raw source `playerEntityIndex_0x94_148`, exported four bytes after the
    /// original source offset because the package record carries a four-byte
    /// header.  This relation is distinct from `parentId_0x28` and from the
    /// presentation `source_id`.
    #[must_use]
    pub fn source_player_entity_index(&self) -> u16 {
        u16::from_le_bytes([self.record[152], self.record[153]])
    }

    /// Keep the source `playerEntityIndex_0x94_148` relation in the lossless
    /// record when an authoritative callback claims a dynamic target.
    pub(super) fn set_source_player_entity_index(&mut self, player_entity_index: u16) {
        self.record[152..154].copy_from_slice(&player_entity_index.to_le_bytes());
    }

    /// Return the source draw-disable state written by
    /// `DisableEntityDrawing04_57F10`. This is byte 17 bit 2 (the 0x400 bit
    /// of the control dword), not byte 16 bit 0 used by pickup ownership.
    #[must_use]
    pub const fn drawing_disabled(&self) -> bool {
        self.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
    }

    /// Apply the source draw-disable write to both the typed control dword
    /// and its lossless exported record bytes.
    pub(super) fn disable_drawing(&mut self) {
        self.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        self.record[16..20].copy_from_slice(&self.control_flags.to_le_bytes());
    }

    /// Apply the source `sub_57F20` class-zero transition after a row was
    /// draw-disabled during the preceding UpdateEntities pass.
    pub(super) fn reclaim_source_row(&mut self) {
        self.active = false;
        self.authority = EntityAuthority::Inactive;
        self.class = 0;
        self.record[2] = 0;
        self.record[67] = 0;
    }

    /// Update the source position while keeping the lossless record projection
    /// in lockstep.  Level 2 switch tests and the future objective bridge use
    /// this seam instead of mutating a typed cache and leaving stale bytes.
    pub(super) fn set_position(&mut self, position: Position) {
        self.position = position;
        self.record[80..82].copy_from_slice(&position.x.to_le_bytes());
        self.record[82..84].copy_from_slice(&position.y.to_le_bytes());
        self.record[84..86].copy_from_slice(&position.z.to_le_bytes());
    }

    /// Keep the source orientation field in lockstep with a callback write.
    pub(super) fn set_yaw(&mut self, yaw: i16) {
        self.yaw = yaw;
        self.record[32..34].copy_from_slice(&yaw.to_le_bytes());
    }

    /// Keep the source entity pitch in lockstep with a callback write.
    pub(super) fn set_pitch(&mut self, pitch: i16) {
        self.pitch = pitch;
        self.record[34..36].copy_from_slice(&pitch.to_le_bytes());
    }

    /// Keep the source `roll_0x20_32` field in lockstep with the Nyphur
    /// movement helper's target-heading write.
    pub(super) fn set_source_roll(&mut self, roll: i16) {
        self.record[36..38].copy_from_slice(&roll.to_le_bytes());
    }

    /// Raw source `roll_0x20_32`, kept separate from the exported fov word.
    pub(super) fn source_roll(&self) -> i16 {
        i16::from_le_bytes([self.record[36], self.record[37]])
    }

    /// Keep the source entity FOV word in lockstep with a callback write.
    pub(super) fn set_fov(&mut self, fov: i16) {
        self.fov = fov;
        self.record[38..40].copy_from_slice(&fov.to_le_bytes());
    }

    /// Keep the source callback speed field in lockstep with its typed cache.
    pub(super) fn set_actual_speed(&mut self, actual_speed: i16) {
        self.actual_speed = actual_speed;
        self.record[134..136].copy_from_slice(&actual_speed.to_le_bytes());
    }

    /// Raw source `minSpeed_0x84_132`, exported four bytes after the source
    /// offset and consumed by the Nyphur movement callbacks.
    pub(super) fn minimum_speed(&self) -> i16 {
        i16::from_le_bytes([self.record[136], self.record[137]])
    }

    /// Keep the source life field in lockstep with a common AI upkeep write.
    pub(super) fn set_life(&mut self, life: i32) {
        self.life = life;
        self.record[12..16].copy_from_slice(&life.to_le_bytes());
    }

    /// Keep the source mana field in lockstep with a common AI upkeep write.
    pub(super) fn set_mana(&mut self, mana: i32) {
        self.mana = mana;
        self.record[148..152].copy_from_slice(&mana.to_le_bytes());
    }

    /// Keep the source mana-regeneration field in lockstep with a common AI
    /// upkeep write.
    pub(super) fn set_mana_regen(&mut self, mana_regen: i32) {
        self.mana_regen = mana_regen;
        self.record[140..144].copy_from_slice(&mana_regen.to_le_bytes());
    }

    /// Borrow the source `str_0x5E_94` payload cleared by `sub_12A70` while
    /// the AI cooldown carrier is active.
    pub(super) fn source_upkeep_mailbox(&self) -> &[u8] {
        &self.record[98..134]
    }

    pub(super) fn source_upkeep_word_7a(&self) -> u16 {
        u16::from_le_bytes([self.record[126], self.record[127]])
    }

    pub(super) fn source_upkeep_word_62(&self) -> u16 {
        u16::from_le_bytes([self.record[102], self.record[103]])
    }

    pub(super) fn source_upkeep_payload(&self) -> i32 {
        i32::from_le_bytes([
            self.record[98],
            self.record[99],
            self.record[100],
            self.record[101],
        ])
    }

    pub(super) fn set_source_upkeep_payload(&mut self, value: i32) {
        self.record[98..102].copy_from_slice(&value.to_le_bytes());
    }

    pub(super) fn clear_source_upkeep_owner(&mut self) {
        self.record[102..104].fill(0);
    }

    /// Apply the source `memset(str_0x5E_94, 0, 36)` operation.
    pub(super) fn clear_source_upkeep_mailbox(&mut self) {
        self.record[98..134].fill(0);
    }

    /// Apply the source `sub_5EF70` post-contact write.  The qualified
    /// Nyphur row is model 1, so this is deliberately a typed source guard,
    /// not a generic timer write on every AI entity.
    pub(super) fn apply_source_sub_5ef70(&mut self) {
        if self.index != 0 && self.class == 3 && self.model == 0 {
            self.record[58..60].copy_from_slice(&100_i16.to_le_bytes());
        }
    }

    /// Apply the already-qualified `SetSpell_6D5E0` profile to an AI holder.
    /// The active-holder branch only queues `tier + 1` in word 0x2C; the
    /// cold branch carries the complete authored subspell/mana payload.
    pub(super) fn apply_nyphur_spell_profile(&mut self, profile: Level2SetSpellProfile, tier: u8) {
        let active_word_2e = u16::from_le_bytes([self.record[50], self.record[51]]);
        if active_word_2e != 0 {
            self.record[48..50].copy_from_slice(&u16::from(tier + 1).to_le_bytes());
            return;
        }
        self.subspell_index = profile.subspell_index;
        self.record[46..48].copy_from_slice(&profile.subspell_index.to_le_bytes());
        self.activation_value = profile.word_0x30;
        self.record[52..54].copy_from_slice(&profile.word_0x30.to_le_bytes());
        self.byte_3b = profile.byte_0x3b;
        self.record[63] = profile.byte_0x3b;
        self.byte_3c = 0;
        self.record[64] = 0;
        self.action_marker = 0;
        self.record[65] = 0;
        self.font_type_index = 0;
        self.mana_regen = profile.mana_regen;
        self.record[140..144].copy_from_slice(&profile.mana_regen.to_le_bytes());
        self.max_mana = profile.max_mana;
        self.mana_cost = profile.max_mana;
        self.record[144..148].copy_from_slice(&profile.max_mana.to_le_bytes());
        self.mana = profile.mana;
        self.record[148..152].copy_from_slice(&profile.mana.to_le_bytes());
        self.level_gate = tier.cast_signed();
        self.record[74] = tier;
    }

    /// Keep the source entity RNG in lockstep with a callback draw.
    pub(super) fn set_entity_rng(&mut self, entity_rng: u16) {
        self.entity_rng = entity_rng;
        self.record[24..26].copy_from_slice(&entity_rng.to_le_bytes());
    }

    /// Keep the source control dword in lockstep with a callback flag write.
    pub(super) fn set_control_flags(&mut self, control_flags: u32) {
        self.control_flags = control_flags;
        self.record[16..20].copy_from_slice(&control_flags.to_le_bytes());
    }

    /// Raw source `word_0x30_48` writer used by the wind target callback.
    pub(super) fn set_source_word_30(&mut self, value: u16) {
        self.record[48..50].copy_from_slice(&value.to_le_bytes());
    }

    /// Keep the lossless source cadence byte in sync with a typed callback
    /// projection that reuses an imported frame-zero entity slot.
    pub(super) fn set_phase(&mut self, phase: u8) {
        self.phase = phase;
        self.record[66] = phase;
    }

    /// Keep the low source word of `dword_0x10` in sync with a callback
    /// cooldown projected onto an imported class-11 root.
    pub(super) fn set_source_word_10(&mut self, value: u16) {
        self.record[20..22].copy_from_slice(&value.to_le_bytes());
    }

    /// Raw source `dword_0x10_16`, used by the Trader when it reserves a
    /// class-10/model-45 building target.
    pub(super) fn source_dword_10(&self) -> i32 {
        i32::from_le_bytes([
            self.record[20],
            self.record[21],
            self.record[22],
            self.record[23],
        ])
    }

    /// Keep all four bytes of source `dword_0x10_16` in lockstep with a
    /// Trader target-reservation write.
    pub(super) fn set_source_dword_10(&mut self, value: i32) {
        self.record[20..24].copy_from_slice(&value.to_le_bytes());
    }

    /// Keep the local-player damage/killer words in the lossless source
    /// record when `sub_5EFA0` applies a contact mailbox.
    pub(super) fn set_damage_result_words(&mut self, killer: u16, attacker: u16) {
        self.record[40..42].copy_from_slice(&killer.to_le_bytes());
        self.record[42..44].copy_from_slice(&attacker.to_le_bytes());
    }

    /// Apply the source death action and clear word-0x2C when the local
    /// player crosses below zero life in `AddPlayer03_00_5E010`.
    pub(super) fn set_player_death_state(&mut self) {
        self.action = 2;
        self.record[48..50].fill(0);
        self.record[73] = 2;
    }

    pub(super) fn source_word_2c(&self) -> i16 {
        i16::from_le_bytes([self.record[48], self.record[49]])
    }

    pub(super) fn set_source_word_2c(&mut self, value: i16) {
        self.record[48..50].copy_from_slice(&value.to_le_bytes());
    }

    pub(super) fn set_source_action(&mut self, action: u8) {
        self.action = action;
        self.record[73] = action;
    }

    /// Set source control byte[0] bit 0x20. This is deliberately not the
    /// 0x400 bit installed by `DisableEntityDrawing04_57F10`.
    pub(super) fn set_player_death_hidden_bit(&mut self) {
        self.set_control_flags(self.control_flags | 0x20);
    }

    #[cfg(test)]
    pub(crate) fn with_record_byte(mut self, offset: usize, value: u8) -> Self {
        self.record[offset] = value;
        self
    }

    /// Raw zero-terminated `oldMapEntity_0x16_22` link used by the outdoor
    /// terrain-cell painter traversal.
    #[must_use]
    pub fn old_map_entity(&self) -> u16 {
        // The private package prepends the four-byte exported entity header
        // (slot, active, padding) before the remc2 record body.
        u16::from_le_bytes([self.record[26], self.record[27]])
    }

    /// Raw zero-terminated `nextEntity_0x18_24` link toward the terrain-cell
    /// head. Together with `oldMapEntity`, this validates exact map surgery.
    #[must_use]
    pub fn next_entity(&self) -> u16 {
        u16::from_le_bytes([self.record[28], self.record[29]])
    }

    /// Raw source `dword_0x10` constructor field. Scenery constructors use
    /// this as their deterministic slot modulo eleven value.
    #[must_use]
    pub fn source_word_10(&self) -> u16 {
        u16::from_le_bytes([self.record[20], self.record[21]])
    }

    /// Raw source `word_0x96_150`, the model-4 special-spawn relation used
    /// by `sub_68E50`.
    #[must_use]
    pub fn source_word_96(&self) -> u16 {
        u16::from_le_bytes([self.record[154], self.record[155]])
    }

    /// Raw source `word_0x98_152`, the identity checksum paired with
    /// `word_0x96_150` by the Nyphur target selectors.
    pub(super) fn source_word_98(&self) -> i16 {
        i16::from_le_bytes([self.record[156], self.record[157]])
    }

    /// Install the two-word source relation written by the pinned target
    /// selectors. The relation stays in the lossless entity record; the
    /// common carrier does not invent a second target identity.
    pub(super) fn set_source_relation(&mut self, target: &EntitySnapshot) {
        self.record[154..156].copy_from_slice(&target.index.to_le_bytes());
        self.record[156..158].copy_from_slice(&level2_nyphur_source_identity(target).to_le_bytes());
    }

    /// Apply the source relation clear used by `sub_5EFA0` when a previous
    /// target is no longer a live contact.
    pub(super) fn clear_source_word_96(&mut self) {
        self.record[154..156].fill(0);
    }

    /// Source `array_0x52_82.yaw`, distinct from the entity orientation yaw.
    #[must_use]
    pub(super) fn array_yaw(&self) -> i16 {
        i16::from_le_bytes([self.record[86], self.record[87]])
    }

    /// Raw source `array_0x52_82.pitch` consumed by the model-76 target
    /// follower when it retunes the root oscillation envelope from a live
    /// target row.
    #[must_use]
    pub(super) fn array_pitch(&self) -> i16 {
        i16::from_le_bytes([self.record[88], self.record[89]])
    }

    /// Raw source `array_0x52_82.roll`, the Y half-width used by
    /// `CompareAxisWithShift_10750`.
    #[must_use]
    pub(super) fn array_roll(&self) -> i16 {
        i16::from_le_bytes([self.record[90], self.record[91]])
    }

    /// Raw source `array_0x52_82.fov`, used by the cave ceiling clamp in
    /// `sub_33340` after the target movement branch.
    #[must_use]
    pub(super) fn array_fov(&self) -> i16 {
        i16::from_le_bytes([self.record[92], self.record[93]])
    }

    /// Raw source `word_0x24_36` targeted by the action-51 first pass.
    #[must_use]
    pub fn word_24(&self) -> u16 {
        u16::from_le_bytes([self.record[40], self.record[41]])
    }

    /// Raw source `word_0x26_38` targeted by the action-51 first pass.
    #[must_use]
    pub fn word_26(&self) -> u16 {
        u16::from_le_bytes([self.record[42], self.record[43]])
    }

    /// Raw source `word_0x32_50`, the peer-link field consumed by the
    /// class-5 Archer action-35 callback.
    #[must_use]
    pub fn word_32(&self) -> u16 {
        u16::from_le_bytes([self.record[54], self.record[55]])
    }

    /// Raw source `word_0x34_52`, the head/next link used by class-5 child
    /// chains. This is distinct from the class-list `next_relation` field.
    #[must_use]
    pub fn word_34(&self) -> u16 {
        u16::from_le_bytes([self.record[56], self.record[57]])
    }

    /// Raw source `word_0x2E_46` used by `sub_5F890` for the spell-holder
    /// action/cooldown counter.  This is deliberately kept separate from the
    /// public entity action byte at record offset 73.
    #[must_use]
    pub fn source_word_2e(&self) -> u16 {
        u16::from_le_bytes([self.record[46], self.record[47]])
    }

    /// Raw source `word_0x30_48`, the spell-holder counter copied by
    /// `sub_5F890(a1, 1)` before a Castle stage is reinstalled.
    #[must_use]
    pub fn source_word_30(&self) -> u16 {
        u16::from_le_bytes([self.record[48], self.record[49]])
    }

    /// Exported `word_0x30_48` from the spell-holder source struct. The
    /// entities export prepends a four-byte slot/active header, so this
    /// source field is at record bytes 52..54. Keep this helper separate from
    /// the legacy `source_word_30` projection used by older economy rows.
    #[must_use]
    pub fn source_spell_word_30(&self) -> u16 {
        u16::from_le_bytes([self.record[52], self.record[53]])
    }

    /// Source SetSpell parent relation written at the holder record's
    /// `word_0x28_40` slot. This is distinct from the terrain oldMapEntity
    /// link, which remains zero for the acquired source pickup.
    #[must_use]
    pub fn source_spell_parent_entity_index(&self) -> u16 {
        u16::from_le_bytes([self.record[44], self.record[45]])
    }

    /// Raw source `dword_0x5E_94`, the spell payload mailbox consumed by
    /// `sub_11900` and the wind parent pulse helper.
    #[must_use]
    pub(super) fn source_wind_spell_payload(&self) -> i32 {
        i32::from_le_bytes(
            self.record[98..102]
                .try_into()
                .expect("entity record payload"),
        )
    }

    /// Raw source `word_0x62_98`, the last source entity that wrote the spell
    /// payload mailbox.
    #[must_use]
    pub(super) fn source_wind_spell_owner(&self) -> u16 {
        u16::from_le_bytes([self.record[102], self.record[103]])
    }

    /// Apply `sub_11900(a1, this, 0, payload)` to a class-10/model-45 row.
    pub(super) fn apply_wind_model45_mailbox(&mut self, parent_entity_id: u16, payload: u16) {
        let next_payload = if self.source_wind_spell_owner() != 0 {
            i32::from(payload)
        } else {
            self.source_wind_spell_payload()
                .wrapping_add(i32::from(payload))
        };
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&parent_entity_id.to_le_bytes());
    }

    /// Apply the model-2 player branch of the wind pulse helper. This writes
    /// the thirty-tick mailbox, source parent relation, and additive payload
    /// exactly where `sub_33710` stores them in the imported entity record.
    pub(super) fn apply_wind_model2_mailbox(
        &mut self,
        parent_source_slot: u16,
        parent_entity_id: u16,
        payload: u16,
    ) {
        self.activation_value = 30;
        self.record[52..54].copy_from_slice(&30_u16.to_le_bytes());
        self.record[42..44].copy_from_slice(&parent_source_slot.to_le_bytes());
        let next_payload = self
            .source_wind_spell_payload()
            .wrapping_add(i32::from(payload));
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&parent_entity_id.to_le_bytes());
    }

    /// Apply one `sub_10C80(a2 = 0)` Fire-Spheres mailbox write. The model-2,
    /// model-45, and generic map scans share this exact write after their
    /// distinct admission predicates: replace an ownerless payload, otherwise
    /// add, then record the colliding child identity.
    pub(super) fn apply_model76_mailbox(&mut self, owner_entity_id: u16, payload: u16) {
        let next_payload = if self.source_wind_spell_owner() != 0 {
            self.source_wind_spell_payload()
                .wrapping_add(i32::from(payload))
        } else {
            i32::from(payload)
        };
        self.record[98..102].copy_from_slice(&next_payload.to_le_bytes());
        self.record[102..104].copy_from_slice(&owner_entity_id.to_le_bytes());
    }

    /// Raw source `byte_0x38_56`, exported four bytes later in the lossless
    /// entity record. Bit zero admits the `sub_10C80(a2 = 0)` generic scan.
    pub(super) fn source_byte_38(&self) -> u8 {
        self.record[60]
    }

    /// Raw source `word_0x4A_74`, the return-side hand override consumed by
    /// `sub_68FF0` before it installs a holder.
    #[must_use]
    pub fn source_word_4a(&self) -> i16 {
        i16::from_le_bytes([self.record[78], self.record[79]])
    }

    /// Raw reset `animationFrame_0x60_96` used by fixed and base-plus-frame
    /// sprite families.
    #[must_use]
    pub fn animation_frame(&self) -> i8 {
        i8::from_le_bytes([self.record[96]])
    }

    #[must_use]
    pub(super) fn castle_placement_blocker(&self) -> Level1CastlePlacementBlocker {
        Level1CastlePlacementBlocker {
            source_entity_index: self.index,
            position: self.position,
            // Exported axis_4d pitch/roll retain original record offsets
            // 0x58/0x5a. They are distinct from the orientation fields.
            half_x: i16::from_le_bytes([self.record[88], self.record[89]]),
            half_y: i16::from_le_bytes([self.record[90], self.record[91]]),
        }
    }

    #[must_use]
    pub fn collision_target(&self) -> Level1CollisionTarget {
        Level1CollisionTarget {
            entity_index: self.index,
            position: self.position,
            collision_vertical_offset: i16::from_le_bytes([self.record[86], self.record[87]]),
            collision_half_width: i16::from_le_bytes([self.record[88], self.record[89]]),
            collision_half_height: i16::from_le_bytes([self.record[92], self.record[93]]),
            active: self.active,
        }
    }

    pub(super) fn ambient_entity(&self) -> Option<Level1AmbientEntity> {
        if !self.active
            || self.class != 10
            || !matches!(
                (self.model, self.action),
                (0, 0) | (1, 1) | (14, 14) | (60, 65)
            )
        {
            return None;
        }
        let read_u16 = |offset| u16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_i16 = |offset| i16::from_le_bytes([self.record[offset], self.record[offset + 1]]);
        let read_u32 = |offset| {
            u32::from_le_bytes([
                self.record[offset],
                self.record[offset + 1],
                self.record[offset + 2],
                self.record[offset + 3],
            ])
        };
        Some(Level1AmbientEntity {
            source_entity_index: self.index,
            parent_entity_index: 0,
            source_id: self.source_id,
            position: self.position,
            max_life: self.max_life,
            life: self.life,
            control_flags: self.control_flags,
            search_radius: read_u32(20),
            entity_rng: read_u16(24),
            class: self.class,
            model: self.model,
            action: self.action,
            phase: self.phase,
            yaw: self.yaw,
            // Particle `array_0x52` axes, distinct from orientation yaw/pitch.
            pitch: read_i16(88),
            fov: read_i16(92),
            vertical_velocity: read_i16(48),
            actual_speed: self.actual_speed,
            max_speed: read_i16(138),
            render_parameter_index: self.particle_parameter_index.unwrap_or(0),
            animation_frame: self.animation_frame(),
            animation_limit: i8::from_le_bytes([self.record[97]]),
            active: true,
            map_linked: self.control_flags & 4 != 0,
            last_material_effect: None,
            collision_probe_ran: false,
        })
    }
}

/// One Level 2 class-2/model-3/action-9 bad stone.  The package already
/// contains the post-`GenerateEvents_49290` reset rows; this wrapper keeps the
/// source row lossless while giving the production Level 2 session the exact
/// action-9 terrain-snap seam from remc2 `sub_65110`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2BadStone {
    pub(super) baseline: EntitySnapshot,
    pub(super) entity: EntitySnapshot,
    pub(super) map_cell: (u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2BadStoneStep {
    pub source_slot: u16,
    pub previous_position: Position,
    pub position: Position,
    pub previous_control_flags: u32,
    pub control_flags: u32,
    pub previous_phase: u8,
    pub phase: u8,
}

impl Level2BadStone {
    pub(super) fn from_entity(entity: &EntitySnapshot, planes: &TerrainPlanes) -> Result<Self> {
        let Some(ordinal) = LEVEL2_BAD_STONE_SLOTS
            .iter()
            .position(|slot| *slot == entity.index)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone source slot {} is not one of 33..=36",
                entity.index
            )));
        };
        if !entity.active
            || entity.class != LEVEL2_BAD_STONE_CLASS
            || entity.model != LEVEL2_BAD_STONE_MODEL
            || entity.action != LEVEL2_BAD_STONE_ACTION
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} has class/model/action {}/{}/{}",
                entity.index, entity.class, entity.model, entity.action
            )));
        }
        if entity.max_life != 300
            || entity.life != 300
            || entity.actual_speed != 16
            || entity.source_word_10() != entity.index % 11
            || entity.control_flags & 4 == 0
            || entity.control_flags & 0x0002_0000 == 0
            || entity.particle_parameter_index != Some(LEVEL2_BAD_STONE_PARTICLE)
            || entity.parameter_relation != Some(LEVEL2_BAD_STONE_PARAMETER - 1)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} constructor fields do not match pinned sub_4AE80",
                entity.index
            )));
        }
        let map_cell = (
            (entity.position.x >> 8) as u8,
            (entity.position.y >> 8) as u8,
        );
        let map_index = terrain_index(map_cell.0, map_cell.1);
        if planes.entity_indices[map_index] != entity.index as i16 {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} is not the head of its authored terrain map cell",
                entity.index
            )));
        }
        let record_hash = sha256_hex(entity.record_bytes());
        if record_hash != LEVEL2_BAD_STONE_RECORD_SHA256[ordinal] {
            return Err(NativeWorldError::new(format!(
                "Level 2 bad stone {} record hash changed: {record_hash}",
                entity.index
            )));
        }
        Ok(Self {
            baseline: entity.clone(),
            entity: entity.clone(),
            map_cell,
        })
    }

    #[must_use]
    pub fn entity(&self) -> &EntitySnapshot {
        &self.entity
    }

    #[must_use]
    pub const fn map_cell(&self) -> (u8, u8) {
        self.map_cell
    }

    #[must_use]
    pub fn source_record_sha256(&self) -> String {
        sha256_hex(self.entity.record_bytes())
    }

    /// Apply the exact class-2 action-9 body: set source control byte 2 bit 1,
    /// snap Z to the shared terrain query, then advance the source action
    /// cadence byte. No movement, RNG draw, or allocation is performed.
    pub fn step_action_9(&mut self, terrain: &SessionTerrain) -> Level2BadStoneStep {
        let previous_position = self.entity.position;
        let previous_control_flags = self.entity.control_flags;
        let previous_phase = self.entity.phase;
        self.entity.record[18] |= 2;
        self.entity.control_flags = u32::from_le_bytes([
            self.entity.record[16],
            self.entity.record[17],
            self.entity.record[18],
            self.entity.record[19],
        ]);
        self.entity.position.z = FlightWorld::ground_height(terrain, self.entity.position);
        self.entity.record[84..86].copy_from_slice(&self.entity.position.z.to_le_bytes());
        self.entity.phase = self.entity.phase.wrapping_add(1);
        self.entity.record[66] = self.entity.phase;
        Level2BadStoneStep {
            source_slot: self.entity.index,
            previous_position,
            position: self.entity.position,
            previous_control_flags,
            control_flags: self.entity.control_flags,
            previous_phase,
            phase: self.entity.phase,
        }
    }

    pub fn reset(&mut self) {
        self.entity = self.baseline.clone();
    }
}

/// One Level 2 class-2/model-2/action-6 dolmen.  `sub_4ADF0` installs the
/// same map-linked constructor state as the bad-stone family, while
/// `AddDolmen02_02_65080` marks player proximity and snaps the entity to the
/// shared terrain altitude.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level2Dolmen {
    pub(super) baseline: EntitySnapshot,
    pub(super) entity: EntitySnapshot,
    pub(super) map_cell: (u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2DolmenStep {
    pub source_slot: u16,
    pub previous_position: Position,
    pub position: Position,
    pub previous_control_flags: u32,
    pub control_flags: u32,
    pub previous_phase: u8,
    pub phase: u8,
}

pub(super) fn level2_source_intersects(left: &EntitySnapshot, right: &EntitySnapshot) -> bool {
    let i16_at = |entity: &EntitySnapshot, offset: usize| {
        i16::from_le_bytes([entity.record[offset], entity.record[offset + 1]])
    };
    let dx = i32::from(right.position.x.wrapping_sub(left.position.x).cast_signed());
    let dy = i32::from(right.position.y.wrapping_sub(left.position.y).cast_signed());
    let dz = i32::from(left.position.z) + i32::from(i16_at(left, 90))
        - i32::from(right.position.z)
        - i32::from(i16_at(right, 90));
    dx.abs() < i32::from(i16_at(left, 86)) + i32::from(i16_at(right, 86))
        && dy.abs() < i32::from(i16_at(left, 88)) + i32::from(i16_at(right, 88))
        && dz.abs() < i32::from(i16_at(left, 92)) + i32::from(i16_at(right, 92))
}

pub(super) fn sync_level2_spell_candidate_projection(
    target: &mut EntitySnapshot,
    source: &EntitySnapshot,
) {
    target.active = source.active;
    target.model = source.model;
    target.life = source.life;
    target.position = source.position;
    target.record[2] = source.record[2];
    target.record[12..16].copy_from_slice(&source.record[12..16]);
    target.record[67..69].copy_from_slice(&source.record[67..69]);
    target.record[80..86].copy_from_slice(&source.record[80..86]);
}

pub(super) fn level2_experience_scroll_overlap(scroll: Position, player: Position) -> bool {
    let dx = i32::from(player.x.wrapping_sub(scroll.x).cast_signed()).abs();
    let dy = i32::from(player.y.wrapping_sub(scroll.y).cast_signed()).abs();
    let scroll_z = scroll
        .z
        .wrapping_add(LEVEL2_SCROLL_COLLISION_VERTICAL_OFFSET);
    let player_z = player
        .z
        .wrapping_add(LEVEL2_PLAYER_COLLISION_VERTICAL_OFFSET);
    let dz = i32::from(player_z.wrapping_sub(scroll_z)).abs();
    dx < i32::from(LEVEL2_SCROLL_COLLISION_HALF_WIDTH + LEVEL2_PLAYER_COLLISION_HALF_WIDTH)
        && dy < i32::from(LEVEL2_SCROLL_COLLISION_HALF_WIDTH + LEVEL2_PLAYER_COLLISION_HALF_WIDTH)
        && dz < i32::from(LEVEL2_SCROLL_COLLISION_HALF_HEIGHT + LEVEL2_PLAYER_COLLISION_HALF_HEIGHT)
}

pub(super) fn level2_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

/// Explicit signed-16 subtraction used by remc2's `sub_58440` and
/// `sub_10630`. Position Z is already stored as `i16`, but spelling out the
/// source truncation keeps the high-bit boundary contract visible.
pub(super) fn level2_signed16_difference(left: i16, right: i16) -> i32 {
    i32::from(i16::from_le_bytes(right.wrapping_sub(left).to_le_bytes()))
}

/// Pinned `sub_58440`: Castle defender sphere arbitration compares the
/// signed-16 X/Y/Z deltas as a squared 3-D distance, not the XY-only arrival
/// metric used by `AddBallon_60AB0`.
pub(super) fn level2_wrapping_source_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i64::from(i32::from(right.x.wrapping_sub(left.x).cast_signed()));
    let dy = i64::from(i32::from(right.y.wrapping_sub(left.y).cast_signed()));
    let dz = i64::from(level2_signed16_difference(left.z, right.z));
    u32::try_from(dx * dx + dy * dy + dz * dz).unwrap_or(u32::MAX)
}

/// `sub_5F810` tests byte 1 bit `0x20` of the source control dword. The
/// little-endian entity export places that bit at dword mask `0x2000`.
pub(super) const LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT: u32 = 0x0000_2000;

/// Pinned signed-16 XY distance used by `Maths::EuclideanDistXYZ_58490` in
/// `AddBallon_60AB0` before its class-3 target gate.  The helper's recovered
/// cutoff path ignores Z; the holder's Z participates only in the later
/// terrain gate.
pub(super) fn level2_wrapping_target_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .cast_unsigned(),
    )
}

pub(super) fn level2_mana_sphere_dimension(mana: i32) -> i16 {
    MANA_SPHERE_COLLISION_DIMENSIONS[usize::from(mana_sphere_size_ordinal(mana))]
}

/// `sub_4ABA0` installs the defender's source collision axes through
/// `SetEntityIndexAndRot_49CD0(169)`.  The qualified particle row 169 is
/// speed 545 / rotation 800, so the source stores 272 for X/Y and 400 for
/// the vertical offset/extent.  Keep these separate from the holder's
/// movement speed: `sub_106C0` consumes the collision axes, not `actSpeed`.
pub(super) const LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH: i16 = 272;
pub(super) const LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET: i16 = 400;

/// Pinned `sub_106C0(sphere, defender)` strict AABB.  All source axis fields
/// are signed 16-bit values, while the additions are performed in the
/// promoted integer domain before the strict `<` comparison.
pub(super) fn level2_castle_defender_sphere_overlap(
    defender_position: Position,
    sphere: &ManaSphere,
) -> bool {
    let sphere_extent = i32::from(level2_mana_sphere_dimension(sphere.mana));
    let defender_width = i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_HALF_WIDTH);
    let dx = i32::from(
        sphere
            .position
            .x
            .wrapping_sub(defender_position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        sphere
            .position
            .y
            .wrapping_sub(defender_position.y)
            .cast_signed(),
    );
    let dz = i32::from(sphere.position.z) + sphere_extent
        - i32::from(defender_position.z)
        - i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET);
    dx.abs() < sphere_extent + defender_width
        && dy.abs() < sphere_extent + defender_width
        && dz.abs() < sphere_extent + i32::from(LEVEL2_CASTLE_DEFENDER_COLLISION_VERTICAL_OFFSET)
}

/// Pinned remc2 `sub_10A50` SEARCH.DAT walk for model-39 mana spheres. The
/// four class-0 cells precede the twelve class-1 cells; candidates are read
/// from the live map chain rather than the allocation vector.
pub(super) const LEVEL2_MANA_SPHERE_SEARCH_OFFSETS: [(i8, i8); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (1, 1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (2, -1),
    (-1, 0),
    (2, 0),
    (-1, 1),
    (2, 1),
    (-1, 2),
    (0, 2),
    (1, 2),
    (2, 2),
];

/// Pinned `sub_108B0` uses `AddE7EE0x_10080(0, ...)` for the tier-0
/// Possession flyer.  That class-0 query is only the first four SEARCH.DAT
/// cells; the remaining twelve cells belong to the wider class-1 sphere walk.
pub(super) const LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS: [(i8, i8); 4] =
    [(0, 0), (1, 0), (0, 1), (1, 1)];

pub(super) fn level2_first_colliding_mana_sphere_slot(
    map_chains: &RenderableMapChains,
    spheres: &[ManaSphere],
    source_slot: u16,
    sphere: ManaSphere,
) -> Option<u16> {
    let tile_x = ((u32::from(sphere.position.x) + 128) >> 8) as u8;
    let tile_y = ((u32::from(sphere.position.y) + 128) >> 8) as u8;
    for (delta_x, delta_y) in LEVEL2_MANA_SPHERE_SEARCH_OFFSETS {
        let search_x = tile_x.wrapping_add_signed(delta_x);
        let search_y = tile_y.wrapping_add_signed(delta_y);
        for candidate_slot in map_chains.entity_chain(search_x, search_y) {
            if candidate_slot == source_slot {
                continue;
            }
            let Some(candidate) = spheres.iter().find(|candidate| {
                candidate.active && candidate.source_entity_index == candidate_slot
            }) else {
                continue;
            };
            let left = level2_mana_sphere_dimension(sphere.mana);
            let right = level2_mana_sphere_dimension(candidate.mana);
            if crate::level1_mission::source_entities_overlap(
                sphere.position,
                left,
                left,
                left,
                candidate.position,
                right,
                right,
                right,
            ) {
                return Some(candidate_slot);
            }
        }
    }
    None
}

/// Pinned `sub_108B0` map-chain walk for the ordinary tier-0 Possession
/// flyer. The source returns the first eligible strict-overlap model-39 row;
/// allocation order never substitutes for the chain order.
pub(super) fn level2_first_possession_target_sphere_slot(
    map_chains: &RenderableMapChains,
    spheres: &[ManaSphere],
    projectile: Level2PossessionProjectile,
) -> Option<u16> {
    for candidate_slot in source_map_search_slots(
        map_chains,
        projectile.position,
        &LEVEL2_POSSESSION_PROJECTILE_SEARCH_OFFSETS,
    ) {
        let Some(candidate) = spheres.iter().find(|candidate| {
            candidate.active
                && !candidate.pending_delete
                && candidate.source_entity_index == candidate_slot
                && u16::try_from(candidate.id).ok() != Some(projectile.source_id_entity_index)
                && candidate.player_entity_index != projectile.source_id_entity_index
        }) else {
            continue;
        };
        let dimension = level2_mana_sphere_dimension(candidate.mana);
        if crate::level1_mission::source_entities_overlap(
            projectile.position,
            POSSESSION_PROJECTILE_COLLISION_HALF_WIDTH,
            POSSESSION_PROJECTILE_COLLISION_VERTICAL_OFFSET,
            POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT,
            candidate.position,
            dimension,
            dimension,
            dimension,
        ) {
            return Some(candidate_slot);
        }
    }
    None
}

pub(super) fn level2_possession_guidance_target_top(sphere: &ManaSphere) -> Position {
    Position {
        z: sphere
            .position
            .z
            .wrapping_add(level2_mana_sphere_dimension(sphere.mana)),
        ..sphere.position
    }
}

/// Qualified Level-2 subset of `sub_67CB0`'s model-1/0x11 acquisition list.
/// The represented arena currently carries model-39 rows; source rebuilds
/// their special list in ascending arena-slot order and strict-lower score
/// replacement preserves the earlier row on ties.
pub(super) fn level2_possession_guidance_target(
    spheres: &[ManaSphere],
    source_slots: &[u16],
    projectile: Level2PossessionProjectile,
) -> Option<(u16, Position)> {
    let mut selected = None;
    let mut selected_score = u32::MAX;
    for source_slot in source_slots {
        let Some(sphere) = spheres.iter().find(|sphere| {
            sphere.source_entity_index == *source_slot
                && (sphere.active || sphere.pending_delete)
                && sphere.player_entity_index != projectile.source_id_entity_index
        }) else {
            continue;
        };
        let target_top = level2_possession_guidance_target_top(sphere);
        let Some(score) = projectile_target_score(
            projectile.position,
            projectile.yaw,
            projectile.pitch,
            target_top,
        ) else {
            continue;
        };
        if score < selected_score {
            selected = Some((sphere.source_entity_index, target_top));
            selected_score = score;
        }
    }
    selected
}

/// Resolve an already-installed source slot without a generation guard.
/// If a model-39 row reuses that slot, source follows the reused row; if the
/// slot is absent, the one-shot acquisition bit prevents retargeting.
pub(super) fn level2_possession_guidance_relation(
    spheres: &[ManaSphere],
    possession_projectiles: &[Level2PossessionProjectile],
    stale_source_records: &BTreeMap<u16, Position>,
    target_source_entity_index: u16,
) -> Option<Position> {
    spheres
        .iter()
        .find(|sphere| sphere.source_entity_index == target_source_entity_index)
        .map(level2_possession_guidance_target_top)
        .or_else(|| {
            possession_projectiles
                .iter()
                .find(|projectile| projectile.source_entity_index == target_source_entity_index)
                // A freshly constructed model-1 row has source array-yaw 0,
                // so `sub_65580` uses its stored position without a Z offset.
                .map(|projectile| projectile.position)
        })
        .or_else(|| {
            stale_source_records
                .get(&target_source_entity_index)
                .copied()
        })
}

pub(super) fn level2_carrier_sphere_overlap(carrier: Position, sphere: &ManaSphere) -> bool {
    let extent = level2_mana_sphere_dimension(sphere.mana);
    let dx = i32::from(sphere.position.x.wrapping_sub(carrier.x).cast_signed());
    let dy = i32::from(sphere.position.y.wrapping_sub(carrier.y).cast_signed());
    // `sub_106C0(sphere, carrier)` compares the source sphere axis (including
    // its mana-sized yaw/fov extent) against the carrier's 272/400/400
    // collision axes.  Keep the strict signed source expression rather than
    // treating the sphere as a point at its centre.
    let dz = i32::from(sphere.position.z) + i32::from(extent)
        - i32::from(carrier.z)
        - i32::from(MANA_CARRIER_COLLISION_VERTICAL_OFFSET);
    dx.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dy.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_WIDTH + extent)
        && dz.abs() < i32::from(MANA_CARRIER_COLLISION_HALF_HEIGHT + extent)
}

/// `AddBallon_60AB0` compares a class-10 target's raw
/// `playerEntityIndex_0x94` with the carrier's raw `id_0x1A` relation. The
/// Level 2 projection stores that raw carrier relation in
/// `owner_entity_index`; `ManaCarrier::id` remains the presentation/source
/// slot identity and must not be used for this gate.
pub(super) fn level2_carrier_sphere_relation_matches(
    carrier: &ManaCarrier,
    sphere: &ManaSphere,
) -> bool {
    sphere.owner_entity_index == carrier.owner_entity_index
}

pub(super) fn level2_castle_sphere_overlap(castle: Position, sphere: &ManaSphere) -> bool {
    let extent = level2_mana_sphere_dimension(sphere.mana);
    let dx = i32::from(sphere.position.x.wrapping_sub(castle.x).cast_signed());
    let dy = i32::from(sphere.position.y.wrapping_sub(castle.y).cast_signed());
    dx.abs() < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + extent)
        && dy.abs() < i32::from(CASTLE_STAGE_ONE_COLLISION_HALF_WIDTH + extent)
}

/// `sub_60810`'s stage table with the imported player spell coefficient
/// normalised to 256 (the Level 2 economy projection has no live spell-tier
/// mutator yet). The returned pair is `(max_life, mana_capacity)`.
pub(super) fn level2_castle_stage_stats(level: u8) -> (i32, i32) {
    match level {
        0 => (0, 5_000),
        1 => (20_000, 8_500),
        2 => (40_000, 18_000),
        3 => (40_000, 38_800),
        4 => (60_000, 78_600),
        5 => (60_000, 158_200),
        6 => (80_000, 317_400),
        _ => (80_000, 300_000_000),
    }
}

/// Source `my_sign32` vertical launch calculation shared by
/// `TransformEntityToManaSphere_36BA0` and `sub_5FD00`:
/// `(1024 - height - sign(1024 - height) * 7) >> 3`.
#[must_use]
pub(super) fn level2_mana_sphere_vertical_velocity(height_above_ground: i32) -> i16 {
    let delta = 1_024_i32.saturating_sub(height_above_ground);
    let sign = match delta.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    i16::try_from((delta - sign * 7) >> 3).expect("mana sphere vertical velocity fits i16")
}

/// `sub_60400`'s model-3 action-9 defender-holder count consumed by
/// `sub_5FF50`. The source's stage-0/default path has no children to maintain.
pub(super) const fn level2_castle_defender_count(level: u8) -> u8 {
    match level {
        1 | 2 | 3 => 1,
        4 | 5 => 2,
        6 | 7 => 3,
        _ => 0,
    }
}

/// `sub_60400`'s `array_0x5C_92` model-15 tail count consumed by
/// `sub_5FF50`.  The source has no tail entries for stages 1/2, then grows
/// the authored rows as 4/6/14/18/34 through stage 7.
pub(super) const fn level2_castle_auxiliary_count(level: u8) -> u8 {
    match level {
        3 => 4,
        4 => 6,
        5 => 14,
        6 => 18,
        7 => 34,
        _ => 0,
    }
}

/// `sub_60400`'s stage-controller counts and the corresponding model-79
/// placement points from `x_BYTE_DB038` (pinned `EventsFunctions.cpp`).  The
/// source stores this as one compact count/offset table; spelling it out here
/// keeps source order stable and makes allocation failure observable after any
/// committed prefix.
pub(super) fn level2_castle_controller_points(level: u8) -> &'static [(i8, i8)] {
    match level {
        1 => &[(4, 4)],
        2 | 3 => &[(3, 3), (17, 3), (3, 17), (17, 17)],
        4 | 5 => &[(3, 3), (31, 3), (3, 31), (31, 31)],
        6 | 7 => &[
            (3, 3),
            (24, 3),
            (45, 3),
            (3, 24),
            (45, 24),
            (3, 45),
            (24, 45),
            (45, 45),
        ],
        _ => &[],
    }
}

/// BUILD00DATTAB dimensions consumed by `sub_613D0` on the fixed 320x200
/// path.  The source TAB bytes are `[8, 21, 21, 35, 35, 48, 48]`; the
/// runtime loader doubles them and the Castle controller halves them again
/// under VGA selector 1, yielding the same effective values.  Keep the
/// asymmetrical 21/35 entries even though integer centring shares coordinates
/// with 20/34.
pub(super) fn level2_castle_controller_dimensions(level: u8) -> (u8, u8) {
    match level {
        1 => (8, 8),
        2 | 3 => (21, 21),
        4 | 5 => (35, 35),
        6 | 7 => (48, 48),
        _ => (0, 0),
    }
}

/// Effective BUILD dimensions after the source VGA selector-1 half-size
/// conversion used by `RemoveCastleStage_385C0`.  The pinned runtime records
/// are 16/42/42/70/70/96/96, yielding the same effective values as the
/// controller-offset table after the single source half-size conversion.
pub(super) const fn level2_castle_removal_dimensions(level: u8) -> (u8, u8) {
    match level {
        1 => (8, 8),
        2 | 3 => (21, 21),
        4 | 5 => (35, 35),
        6 | 7 => (48, 48),
        _ => (0, 0),
    }
}

/// Source `sub_613D0` backs down from the current Castle level while the
/// player's `array_0x24E[9 + level]` coefficient is zero. The selected level,
/// rather than the requested level, controls both DB038 geometry and model-79
/// word `0x4A`; an all-zero prefix emits no controllers.
pub(super) fn level2_castle_controller_selection(
    level: u8,
    coefficients: &[u8; 8],
) -> Option<(u8, u8)> {
    let mut selected = level.min(7);
    while selected > 0 {
        let coefficient = coefficients[usize::from(selected)];
        if coefficient != 0 {
            return Some((selected, coefficient));
        }
        selected -= 1;
    }
    None
}

/// `SetShiftByCastle_49EC0`'s fixed 320x200/VGA selector-1 result. The
/// controller dimensions are already the effective post-VGA values, so the
/// source half-size must not be applied a second time here.
pub(super) fn level2_castle_shift_axes(level: u8) -> (i16, i16) {
    let (width, height) = level2_castle_controller_dimensions(level);
    let width = i32::from(width);
    let height = i32::from(height);
    (
        i16::try_from(((width << 8) + 1_280) >> 1).expect("Castle shift pitch fits i16"),
        i16::try_from(((height << 8) + 1_280) >> 1).expect("Castle shift roll fits i16"),
    )
}

/// Port of `sub_4AA40`'s Castle root/`axis_0x9A_154x` placement split. The
/// caller supplies the validated owner BUILD footprint; Level 2's legacy
/// compatibility `build_castle` path still uses the projectile position as
/// both values when no footprint is available.
#[must_use]
pub fn level2_castle_root_positions<W: FlightWorld>(
    world: &W,
    footprint: &CastleBuildFootprint,
    projectile_position: Position,
) -> (Position, Position) {
    // `CastleBuildFootprint` is already the consumer-effective 8x8 BUILD
    // footprint (the raw/runtime descriptor widths are normalized at load),
    // so do not apply the source VGA half-size a second time here.
    level2_castle_root_positions_with_resolution(world, footprint, projectile_position, false)
}

/// Derive the Castle root and `axis_0x9A_154x` anchor exactly as
/// `sub_4AA40` does, including the source's VGA-size split. The explicit
/// selector is retained for callers that still provide raw BUILD dimensions;
/// production Level 2 calls pass `false` because the validated footprint is
/// already normalized to consumer-effective dimensions.
#[must_use]
pub fn level2_castle_root_positions_with_resolution<W: FlightWorld>(
    world: &W,
    footprint: &CastleBuildFootprint,
    projectile_position: Position,
    low_resolution: bool,
) -> (Position, Position) {
    let mut root_position = Position {
        x: projectile_position.x & 0xff00,
        y: projectile_position.y & 0xff00,
        z: world.ground_height(projectile_position),
    };
    if ((root_position.x >> 8) + (root_position.y >> 8)) & 1 != 0 {
        root_position.x = root_position.x.wrapping_add(256);
    }
    let center_x = (root_position.x >> 8) as u8;
    let center_y = (root_position.y >> 8) as u8;
    let width = if low_resolution {
        footprint.width >> 1
    } else {
        footprint.width
    };
    let height = if low_resolution {
        footprint.height >> 1
    } else {
        footprint.height
    };
    let start_x = center_x.wrapping_sub(width / 2);
    let start_y = center_y.wrapping_sub(height / 2);
    let mut minimum = 250_u8;
    let mut sample = |x: u8, y: u8| {
        let height = world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        });
        // `sub_48E60` scans the byte height plane, not the interpolated
        // world-height result.  At an exact tile corner the interpolation
        // is exactly `32 * mapHeightmap`; retain the source byte boundary
        // while making the conversion explicit for generic FlightWorlds.
        let cell_height = (i32::from(height) >> 5).clamp(0, i32::from(u8::MAX)) as u8;
        minimum = minimum.min(cell_height);
    };
    for offset in 0..width {
        let x = start_x.wrapping_add(offset);
        sample(x, start_y);
        sample(x, start_y.wrapping_add(height));
    }
    for offset in 0..height {
        let y = start_y.wrapping_add(offset);
        sample(start_x.wrapping_add(width), y);
        sample(start_x, y);
    }
    let construction_anchor = Position {
        z: i16::from(minimum) * 32,
        ..root_position
    };
    (root_position, construction_anchor)
}

impl Level2Dolmen {
    pub(super) fn from_entity(entity: &EntitySnapshot, planes: &TerrainPlanes) -> Result<Self> {
        let Some(ordinal) = [108_u16, 109, 110]
            .iter()
            .position(|slot| *slot == entity.index)
        else {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen source slot {} is not one of 108..=110",
                entity.index
            )));
        };
        if !entity.active
            || entity.class != 2
            || entity.model != 2
            || entity.action != 6
            || entity.max_life != 300
            || entity.life != 300
            || entity.actual_speed != 16
            || entity.source_word_10() != entity.index % 11
            || entity.control_flags & 4 == 0
            || entity.particle_parameter_index != Some(39)
            || entity.parameter_relation != Some(59)
            || entity.special_relation != EntitySpecialRelation::GlobalSpecialState
            || entity.old_map_entity() != 0
            || entity.next_entity() != 0
        {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} constructor fields do not match pinned sub_4ADF0",
                entity.index
            )));
        }
        let map_cell = (
            (entity.position.x >> 8) as u8,
            (entity.position.y >> 8) as u8,
        );
        if planes.entity_indices[terrain_index(map_cell.0, map_cell.1)] != entity.index as i16 {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} is not the head of its authored terrain map cell",
                entity.index
            )));
        }
        let record_hash = sha256_hex(entity.record_bytes());
        if record_hash != LEVEL2_DOLMEN_RECORD_SHA256[ordinal] {
            return Err(NativeWorldError::new(format!(
                "Level 2 dolmen {} record hash changed: {record_hash}",
                entity.index
            )));
        }
        Ok(Self {
            baseline: entity.clone(),
            entity: entity.clone(),
            map_cell,
        })
    }

    #[must_use]
    pub fn entity(&self) -> &EntitySnapshot {
        &self.entity
    }

    #[must_use]
    pub const fn map_cell(&self) -> (u8, u8) {
        self.map_cell
    }

    /// Apply `AddDolmen02_02_65080`: the source mutates a nearby player
    /// entity's byte-1 interaction flag, then only snaps the dolmen Z to the
    /// shared terrain altitude.  It does not advance the dolmen phase or set
    /// any dolmen control bit.
    pub fn step_action_6(&mut self, terrain: &SessionTerrain) -> Level2DolmenStep {
        let previous_position = self.entity.position;
        let previous_control_flags = self.entity.control_flags;
        let previous_phase = self.entity.phase;
        self.entity.position.z = FlightWorld::ground_height(terrain, self.entity.position);
        self.entity.record[84..86].copy_from_slice(&self.entity.position.z.to_le_bytes());
        Level2DolmenStep {
            source_slot: self.entity.index,
            previous_position,
            position: self.entity.position,
            previous_control_flags,
            control_flags: self.entity.control_flags,
            previous_phase,
            phase: self.entity.phase,
        }
    }

    pub fn reset(&mut self) {
        self.entity = self.baseline.clone();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitySpecialRelation {
    None,
    PlayerState(u8),
    GlobalSpecialState,
}

/// The exact fixed-width Type_str_164 fields needed by the open Level 2
/// Nyphur/common-upkeep and terminal seams.  This is a carrier only: policy
/// remains source-backed work for a later slice, and no defaults are generated
/// when the owner package omits these bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeStr164Auxiliary {
    pub word_0x26_38: [i16; 8],
    pub byte_0x154_340: u8,
    pub dword_0x16D_365: i32,
    pub word_0x1A2_418: i16,
    pub array_0x1FC_508: [u16; 32],
    pub word_0x242_578: i16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerSnapshot {
    pub slot: u8,
    pub is_ai: bool,
    pub entity_index: u16,
    pub active_player_index: i16,
    pub turn: i32,
    pub left_spell_slot: u8,
    pub right_spell_slot: u8,
    /// Raw signed selectors. Level 2 uses -1/-1 for an empty local hand;
    /// Level 1's normalized selectors remain available through the legacy
    /// `left_spell_slot`/`right_spell_slot` fields.
    pub left_spell_slot_raw: i16,
    pub right_spell_slot_raw: i16,
    pub left_subspell_index: i8,
    pub right_subspell_index: i8,
    pub selected_spell_ui_index: u8,
    pub selected_subspell_index: i8,
    pub enabled_spell_holders: [u16; PLAYER_SPELL_COUNT],
    pub spell_hand_eligibility: [u8; PLAYER_SPELL_COUNT],
    /// Source `array_0x403` learned/acquired flags (record 424..450), kept
    /// distinct from the hand-side bytes at record 398..424.
    pub learned_spell_flags: [u8; PLAYER_SPELL_COUNT],
    /// Source `array_0x3E9`/`SpellsEnabled` projection (record 450..476).
    /// This is distinct from the hand-side eligibility bytes at record
    /// 398..424 and the holder relations above.
    pub spell_enabled_flags: [u8; PLAYER_SPELL_COUNT],
    pub family_selected_tiers: [i8; PLAYER_SPELL_COUNT],
    /// Source `SpellExperience_0x263_611x`, stored after the 60-byte player
    /// prefix. This persistent row is distinct from the runtime family-12
    /// award buffer below.
    pub persistent_spell_experience: [i32; PLAYER_SPELL_COUNT],
    /// Source `spellsExperience_0x2CB_715x`, the runtime experience buffer.
    pub runtime_spell_experience: [i32; PLAYER_SPELL_COUNT],
    /// Source `SpellLevels_0x41D_1053z`, recomputed from persistent plus
    /// runtime experience at the family threshold table.
    pub spell_unlocked_levels: [i8; PLAYER_SPELL_COUNT],
    /// Source-exact Level 2 Type_str_164 carrier. Level 1 deliberately has no
    /// corresponding field because its sealed player record remains v2.
    pub type_str_164_auxiliary: Option<TypeStr164Auxiliary>,
    /// Source `array_0x2BDE[playerColorIndex].byte_0x006_2BE4_11236`.
    ///
    /// This mutable Level 2 carrier is intentionally separate from the
    /// compact ActPlayerIndex record at bytes 6..8.  Objective kind 3 reads
    /// this authoritative value; v3/v4 inspection rows retain `None` and
    /// cannot construct a qualified mutable Level 2 session.
    pub banishment_state_0x006: Option<u8>,
    pub(super) record: Box<[u8]>,
}

impl PlayerSnapshot {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    #[must_use]
    pub(super) fn legacy_record_bytes(&self) -> &[u8] {
        &self.record[..PLAYER_LEGACY_RECORD_BYTES]
    }

    #[must_use]
    pub const fn type_str_164_auxiliary(&self) -> Option<&TypeStr164Auxiliary> {
        self.type_str_164_auxiliary.as_ref()
    }

    /// Source `playerColorIndex_0x38_56` from the linked player-state record.
    /// `sub_68FF0` reads this dword_0xA4 field while walking model-0 entities;
    /// it is not the entity's unrelated byte at record offset 60.
    #[must_use]
    pub fn player_color_index(&self) -> u8 {
        self.record[40]
    }

    /// Source `array_0x2BDE[color].byte_0x006`, consumed by objective kind 3
    /// (the banished-player predicate). This is distinct from both the linked
    /// entity's life/drawing state and ActPlayerIndex record[6..8].
    #[must_use]
    pub(super) fn objective_player_state(&self) -> Option<u8> {
        self.banishment_state_0x006
    }

    /// Source `roll_0x155_341`, exported at the compact player-state offset
    /// used by the Level 2 reset projection.
    pub(super) fn roll_0x155(&self) -> i16 {
        i16::from_le_bytes([self.record[46], self.record[47]])
    }

    /// Keep the linked player-state roll feedback in lockstep with source
    /// `sub_33340` writes.
    pub(super) fn set_roll_0x155(&mut self, value: i16) {
        self.record[46..48].copy_from_slice(&value.to_le_bytes());
    }

    /// Apply the linked player-state writes made by the human banished
    /// camera: clear moveBoost, roll feedback, and pitch feedback.
    pub(super) fn clear_banished_camera_feedback(&mut self) {
        self.record[32..34].fill(0);
        self.record[46..48].fill(0);
        self.record[48..50].fill(0);
    }

    pub(super) fn increment_kill_score_for_color(&mut self, color: usize) -> bool {
        let Some(auxiliary) = self.type_str_164_auxiliary.as_mut() else {
            return false;
        };
        let Some(score) = auxiliary.word_0x26_38.get_mut(color) else {
            return false;
        };
        *score = score.wrapping_add(1);
        let offset = PLAYER_LEGACY_RECORD_BYTES + color * 2;
        self.record[offset..offset + 2].copy_from_slice(&score.to_le_bytes());
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainPlanes {
    pub terrain_types: Box<[u8]>,
    pub ground_heights: Box<[u8]>,
    pub shading: Box<[u8]>,
    pub angles: Box<[u8]>,
    pub secondary_heights: Box<[u8]>,
    pub entity_indices: Box<[i16]>,
}

/// Mutable terrain owned by exactly one play session.
///
/// `NativeWorld` remains immutable and shareable. Castle construction changes
/// these planes immediately after the action-44 source slot so subsequent
/// source entities, flight, collision, and rendering all read the same state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionTerrain {
    pub(super) map_type: MapType,
    pub(super) planes: TerrainPlanes,
    pub(super) terrain_rng: u16,
    pub(super) mutation_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HouseRemovalResident {
    pub position: Position,
    pub model: u8,
    pub action: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseRemovalOutcome {
    pub final_rng: u16,
    pub final_occupants: i32,
    pub residents: Vec<HouseRemovalResident>,
}

impl SessionTerrain {
    #[must_use]
    pub fn from_inspection(inspection: &NativeWorldInspection) -> Self {
        Self {
            map_type: inspection.metadata.map_type,
            planes: inspection.terrain_planes.clone(),
            // Level 2 preserves the post-feature terrain RNG boundary.
            terrain_rng: inspection.terrain_rng,
            mutation_sequence: 0,
        }
    }

    #[must_use]
    pub fn from_world(world: &NativeWorld) -> Self {
        Self {
            map_type: world.metadata.map_type,
            planes: world.terrain_planes.clone(),
            // Exact post-generation `rand2_17B4E0` observed at the qualified
            // Package-M export seam. Castle topology variants continue this
            // independent 16-bit terrain stream.
            terrain_rng: 39_663,
            mutation_sequence: 0,
        }
    }

    #[must_use]
    pub const fn planes(&self) -> &TerrainPlanes {
        &self.planes
    }

    #[must_use]
    pub fn ground_heights(&self) -> &[u8] {
        &self.planes.ground_heights
    }

    /// Borrow the immutable ground plane and mutable shading plane together
    /// for the source outer-turn dynamic-light subtraction/service boundary.
    pub fn dynamic_lighting_planes(&mut self) -> (&[u8], &mut [u8]) {
        (
            self.planes.ground_heights.as_ref(),
            self.planes.shading.as_mut(),
        )
    }

    #[must_use]
    pub const fn terrain_rng(&self) -> u16 {
        self.terrain_rng
    }

    #[must_use]
    pub const fn mutation_sequence(&self) -> u64 {
        self.mutation_sequence
    }

    /// Canonical content hash for the six mutable source terrain planes.
    ///
    /// FNV-1a is applied to the schema domain, little-endian dimensions, five
    /// byte planes, then every signed entity index in little-endian form.
    /// The mutation sequence is intentionally separate from content identity.
    #[must_use]
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut append = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        append(b"mc2-session-terrain-v1\0");
        append(&256_u16.to_le_bytes());
        append(&256_u16.to_le_bytes());
        append(&self.planes.terrain_types);
        append(&self.planes.ground_heights);
        append(&self.planes.shading);
        append(&self.planes.angles);
        append(&self.planes.secondary_heights);
        for entity_index in &self.planes.entity_indices {
            append(&entity_index.to_le_bytes());
        }
        hash
    }

    /// Apply one source-ordered BUILD[1] terrain update.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed timer. Mutation is transactional: an
    /// error leaves all six terrain planes, the terrain RNG, and the mutation
    /// sequence unchanged.
    pub fn apply_castle_update(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if update.is_ambient() {
            return self.apply_ambient_update(update);
        }
        let mut staged = self.clone();
        staged.apply_castle_update_inner(update, footprint)?;
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    pub fn apply_ambient_update(
        &mut self,
        update: CastleTerrainUpdate,
    ) -> std::result::Result<(), String> {
        let mut staged = self.clone();
        match update {
            CastleTerrainUpdate::AmbientMaterial {
                position,
                in_type,
                target_type,
            } => {
                let x = position.x.wrapping_add(128).to_be_bytes()[0];
                let y = position.y.wrapping_add(128).to_be_bytes()[0];
                apply_ambient_material(
                    &mut staged.planes,
                    x,
                    y,
                    in_type,
                    target_type,
                    staged.map_type,
                )?;
            }
            CastleTerrainUpdate::AmbientCollision {
                position,
                height_delta,
            } => apply_ambient_collision(
                &mut staged.planes,
                position,
                height_delta,
                staged.map_type,
                &mut staged.terrain_rng,
            ),
            _ => {
                return Err("non-ambient update requires a Castle BUILD footprint".to_owned());
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    /// Remove one source action-53 house BUILD footprint.
    ///
    /// Resident selection and height restoration share the house RNG and must
    /// therefore run in the same row-major loop. Terrain topology retains its
    /// independent session RNG. The returned resident requests are already in
    /// exact allocation order.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed footprint. Mutation is transactional.
    pub fn apply_house_removal(
        &mut self,
        position: Position,
        mut house_rng: u16,
        mut occupants: i32,
        produces_residents: bool,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<HouseRemovalOutcome, String> {
        let expected = usize::from(footprint.width) * usize::from(footprint.height);
        if footprint.cells().len() != expected {
            return Err(format!(
                "house BUILD {} has {} cells, expected {expected}",
                footprint.index,
                footprint.cells().len()
            ));
        }
        let mut staged = self.clone();
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);
        let corner_height =
            |x: u8, y: u8| i16::from(staged.planes.ground_heights[terrain_index(x, y)]);
        let z_coefficient = (corner_height(start_x, start_y)
            + corner_height(start_x.wrapping_add(footprint.width), start_y)
            + corner_height(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )
            + corner_height(start_x, start_y.wrapping_add(footprint.height)))
            >> 2;
        let mut residents = Vec::new();
        let mut nonempty_ordinal = 0_u32;
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell == [0xff, 0xff] {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                nonempty_ordinal = nonempty_ordinal.wrapping_add(1);
                let resident_z = if nonempty_ordinal & 7 == 0 {
                    z_coefficient.wrapping_sub(10).wrapping_mul(32)
                } else {
                    z_coefficient.wrapping_mul(32)
                };
                if occupants > 0 {
                    occupants -= 1;
                    if produces_residents {
                        let (model, action) = if occupants == 0 {
                            (12, 97)
                        } else if occupants < 4 {
                            (4, 33)
                        } else {
                            house_rng = next_house_rng(house_rng);
                            match house_rng % 12 {
                                0 | 1 => (4, 33),
                                2 | 3 => (14, 113),
                                4..=8 => (13, 105),
                                9..=11 => (12, 97),
                                _ => unreachable!("modulo twelve"),
                            }
                        };
                        residents.push(HouseRemovalResident {
                            position: Position {
                                x: u16::from(x) << 8,
                                y: u16::from(y) << 8,
                                z: resident_z,
                            },
                            model,
                            action,
                        });
                    }
                }
                let index = terrain_index(x, y);
                staged.planes.angles[index] = (staged.planes.angles[index] & 0x70) | 1;
                add_building_to_terrain(
                    &mut staged.planes,
                    x,
                    y,
                    staged.map_type,
                    &mut staged.terrain_rng,
                );
                if cell[1] == 0xff {
                    continue;
                }
                let height = staged.planes.ground_heights[index];
                staged.planes.ground_heights[index] =
                    reduce_house_height(height, cell[1], &mut house_rng);
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                smooth_building_height(
                    &mut staged.planes,
                    start_x.wrapping_add(column),
                    start_y.wrapping_add(row),
                    staged.map_type,
                );
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(HouseRemovalOutcome {
            final_rng: house_rng,
            final_occupants: occupants,
            residents,
        })
    }

    /// Apply one source action-51 BUILD terrain event transactionally.
    pub fn apply_house_construction(
        &mut self,
        position: Position,
        remaining: Option<i32>,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        let expected = usize::from(footprint.width) * usize::from(footprint.height);
        if footprint.cells().len() != expected {
            return Err(format!(
                "house BUILD {} has {} cells, expected {expected}",
                footprint.index,
                footprint.cells().len()
            ));
        }
        let mut staged = self.clone();
        match remaining {
            Some(remaining) => {
                let remaining = u8::try_from(remaining).map_err(|_| {
                    format!("house action-51 remaining count {remaining} does not fit u8")
                })?;
                if remaining == 0 {
                    return Err(
                        "house action-51 construction pass requires remaining > 0".to_owned()
                    );
                }
                apply_level1_load_house_sweep(
                    &mut staged.planes,
                    &mut staged.terrain_rng,
                    position,
                    footprint,
                    remaining,
                );
            }
            None => {
                apply_level1_load_house_sweep(
                    &mut staged.planes,
                    &mut staged.terrain_rng,
                    position,
                    footprint,
                    0,
                );
                apply_level1_load_house_edge_smoothing(&mut staged.planes, position, footprint);
            }
        }
        staged.mutation_sequence = staged.mutation_sequence.wrapping_add(1);
        *self = staged;
        Ok(())
    }

    pub(super) fn apply_castle_update_inner(
        &mut self,
        update: CastleTerrainUpdate,
        footprint: &CastleBuildFootprint,
    ) -> std::result::Result<(), String> {
        if let CastleTerrainUpdate::AmbientMaterial {
            position,
            in_type,
            target_type,
        } = update
        {
            let x = position.x.wrapping_add(128).to_be_bytes()[0];
            let y = position.y.wrapping_add(128).to_be_bytes()[0];
            apply_ambient_material(&mut self.planes, x, y, in_type, target_type, self.map_type)?;
            return Ok(());
        }
        if let CastleTerrainUpdate::AmbientCollision {
            position,
            height_delta,
        } = update
        {
            apply_ambient_collision(
                &mut self.planes,
                position,
                height_delta,
                self.map_type,
                &mut self.terrain_rng,
            );
            return Ok(());
        }
        let position = match update {
            CastleTerrainUpdate::ConstructionPass { position, .. }
            | CastleTerrainUpdate::ConstructionCompleted { position }
            | CastleTerrainUpdate::StageDestroyed { position, .. } => position,
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        };
        let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
        let start_x = center_x.wrapping_sub(footprint.width / 2);
        let start_y = center_y.wrapping_sub(footprint.height / 2);

        match update {
            CastleTerrainUpdate::ConstructionPass { remaining, .. } => {
                if !(1..=18).contains(&remaining) {
                    return Err(format!(
                        "Castle construction remaining count {remaining} is outside 1..=18"
                    ));
                }
                let construction_base = i16::from(position.z >> 5);
                for row in 0..footprint.height {
                    for column in 0..footprint.width {
                        let cell = footprint.cells()
                            [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                        let x = start_x.wrapping_add(column);
                        let y = start_y.wrapping_add(row);
                        let index = terrain_index(x, y);
                        if (remaining % 7 == 0 || remaining == 1) && cell[0] != 0xff {
                            apply_castle_material(&mut self.planes, x, y, cell[0], self.map_type);
                        }
                        if cell[1] == 0xff {
                            continue;
                        }
                        let current = i16::from(self.planes.ground_heights[index]);
                        let target = construction_base + i16::from(cell[1]);
                        let delta = target - current;
                        if delta == 0 {
                            continue;
                        }
                        if current == 0
                            || building_topology_rebuild_required(self.planes.terrain_types[index])
                        {
                            self.planes.angles[index] = (self.planes.angles[index] & 0xf8) | 1;
                            add_building_to_terrain(
                                &mut self.planes,
                                x,
                                y,
                                self.map_type,
                                &mut self.terrain_rng,
                            );
                        }
                        let step = delta / remaining;
                        self.planes.ground_heights[index] = current.wrapping_add(step) as u8;
                        if remaining == 1 && self.planes.angles[index].cast_signed() < 0 {
                            self.planes.angles[index] &= 0x7f;
                            if self.map_type != MapType::Cave {
                                self.planes.angles[index] |= 0x08;
                            }
                        }
                        if self.map_type == MapType::Cave {
                            let desired =
                                target.max(i16::from(self.planes.ground_heights[index])) + 100;
                            let desired = desired.min(255);
                            let secondary = i16::from(self.planes.secondary_heights[index]);
                            if desired > secondary {
                                self.planes.secondary_heights[index] =
                                    secondary.wrapping_add((desired - secondary) / remaining) as u8;
                            }
                            normalize_cave_clearance(&mut self.planes, index);
                        } else if remaining == 2 {
                            self.planes.angles[index] &= 0xf7;
                        }
                    }
                }
            }
            CastleTerrainUpdate::ConstructionCompleted { .. } => {
                if self.map_type != MapType::Cave {
                    for row in 0..footprint.height {
                        for column in 0..footprint.width {
                            let index = terrain_index(
                                start_x.wrapping_add(column),
                                start_y.wrapping_add(row),
                            );
                            if self.planes.angles[index] & 0x08 != 0 {
                                self.planes.angles[index] |= 0x80;
                                self.planes.angles[index] &= 0xf7;
                            }
                        }
                    }
                }
            }
            CastleTerrainUpdate::StageDestroyed { entity_rng, .. } => {
                remove_castle_stage(
                    &mut self.planes,
                    start_x,
                    start_y,
                    footprint,
                    self.map_type,
                    &mut self.terrain_rng,
                    entity_rng,
                );
            }
            CastleTerrainUpdate::AmbientMaterial { .. }
            | CastleTerrainUpdate::AmbientCollision { .. } => {
                unreachable!("ambient update returned above")
            }
        }
        Ok(())
    }
}

pub(crate) fn reduce_house_height(height: u8, reduction: u8, house_rng: &mut u16) -> u8 {
    if reduction >= height {
        return 0;
    }
    *house_rng = next_house_rng(*house_rng);
    if *house_rng % 0x32 <= 20 {
        height.wrapping_sub(reduction)
    } else {
        *house_rng = next_house_rng(*house_rng);
        height
            .wrapping_sub(reduction)
            .wrapping_add((*house_rng % 0x14) as u8)
    }
}

impl FlightWorld for SessionTerrain {
    fn ground_height(&self, position: Position) -> i16 {
        interpolate_ground(&self.planes.ground_heights, position.x, position.y) as i16
    }

    fn ceiling_height(&self, position: Position) -> Option<i16> {
        (self.map_type == MapType::Cave).then(|| {
            interpolate_cave(&self.planes.secondary_heights, position.x, position.y) as i16
        })
    }

    fn material_mask(&self, position: Position) -> u32 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        material_mask(self.planes.terrain_types[terrain_index(x, y)])
    }

    fn terrain_type(&self, position: Position) -> u8 {
        let x = position.x.to_be_bytes()[0];
        let y = position.y.to_be_bytes()[0];
        self.planes.terrain_types[terrain_index(x, y)]
    }

    fn angle_flags(&self, tile: crate::Tile) -> u8 {
        self.planes.angles[terrain_index(tile.x, tile.y)]
    }
}

pub(super) const TERRAIN_GENERATION_TABLE_PREFIX: [u8; 0x70] = [
    0x1b, 0x00, 0x1b, 0x50, 0x1b, 0x30, 0x1b, 0x60, 0x1a, 0x00, 0x1a, 0x50, 0x1a, 0x30, 0x1a, 0x60,
    0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60, 0x0a, 0x00, 0x0a, 0x50, 0x0a, 0x30, 0x0a, 0x60,
    0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60, 0x0b, 0x00, 0x0b, 0x50, 0x0b, 0x30, 0x0b, 0x60,
    0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60, 0x0c, 0x00, 0x0c, 0x50, 0x0c, 0x30, 0x0c, 0x60,
    0x15, 0x00, 0x15, 0x50, 0x15, 0x30, 0x15, 0x60, 0x16, 0x00, 0x16, 0x50, 0x16, 0x30, 0x16, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x18, 0x50, 0x18, 0x30, 0x18, 0x60,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x17, 0x50, 0x17, 0x30, 0x17, 0x60,
];

pub(super) fn terrain_index(x: u8, y: u8) -> usize {
    usize::from(x) + usize::from(y) * 256
}

pub(super) fn building_topology_rebuild_required(terrain_type: u8) -> bool {
    matches!(
        terrain_type,
        0 | 0x25 | 0x26 | 0x2c..=0x2f | 0x51 | 0x53 | 0x68 | 0x69 | 0x6d | 0x72 | 0x74
    )
}

pub(super) fn add_building_to_terrain(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    // `AddBuildingToTerrain_46570` unconditionally marks the selected cell
    // and its west, north-west, and north neighbors before rebuilding their
    // topology. Unlike `sub_462A0`, bit 0x80 never suppresses these writes.
    for (mark_x, mark_y) in [
        (x, y),
        (x.wrapping_sub(1), y),
        (x.wrapping_sub(1), y.wrapping_sub(1)),
        (x, y.wrapping_sub(1)),
    ] {
        planes.terrain_types[terrain_index(mark_x, mark_y)] = 1;
    }
    rebuild_marked_building_topology(planes, x, y, 1, 1, map_type, terrain_rng);
}

pub(super) fn rebuild_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    end_x: u8,
    end_y: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let width = end_x.wrapping_sub(start_x).wrapping_add(1);
    let height = end_y.wrapping_sub(start_y).wrapping_add(1);

    // `sub_462A0` marks every selected cell plus its west, north-west, and
    // north neighbors only when the signed angle byte is non-negative.
    // Material BUILD cells retain bit 0x80 and must not be rewritten here.
    for row in 0..height {
        for column in 0..width {
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            for (mark_x, mark_y) in [
                (x, y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
            ] {
                let index = terrain_index(mark_x, mark_y);
                if planes.angles[index].cast_signed() >= 0 {
                    planes.terrain_types[index] = 1;
                }
            }
        }
    }

    rebuild_marked_building_topology(
        planes,
        start_x,
        start_y,
        width,
        height,
        map_type,
        terrain_rng,
    );
}

pub(super) fn rebuild_marked_building_topology(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    width: u8,
    height: u8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let topology_width = width.wrapping_add(1);
    let topology_height = height.wrapping_add(1);
    let topology_start_x = start_x.wrapping_sub(1);
    let topology_start_y = start_y.wrapping_sub(1);

    for row in 0..topology_height {
        for column in 0..topology_width {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let index = terrain_index(tile_x, tile_y);
            if planes.terrain_types[index] != 1 {
                continue;
            }
            let topology_index = 343 * usize::from(planes.angles[index] & 7)
                + 49 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y)] & 7,
                )
                + 7 * usize::from(
                    planes.angles[terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))]
                        & 7,
                )
                + usize::from(planes.angles[terrain_index(tile_x, tile_y.wrapping_add(1))] & 7);
            let [terrain_type, rotation] = building_topology_entry(topology_index);
            planes.terrain_types[index] = terrain_type;
            if terrain_type >= 8 {
                // The decompiled source writes `(angle & 0x87) + rotation`.
                planes.angles[index] = (planes.angles[index] & 0x87).wrapping_add(rotation);
            } else {
                *terrain_rng = terrain_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.angles[index] =
                    (planes.angles[index] & 0x87).wrapping_add(16 * (*terrain_rng % 7) as u8);
            }
        }
    }

    // Finally recalculate the one-cell-expanded shading neighborhood in the
    // literal source order. The signed eight-bit temporary deliberately wraps.
    for row in 0..topology_height.wrapping_add(1) {
        for column in 0..topology_width.wrapping_add(1) {
            let tile_x = topology_start_x.wrapping_add(column);
            let tile_y = topology_start_y.wrapping_add(row);
            let down_right = planes.ground_heights
                [terrain_index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))];
            let up_left = planes.ground_heights
                [terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))];
            let mut shade = (32_i16 - i16::from(down_right)) as i8;
            shade = shade.wrapping_add(up_left as i8);
            shade = if shade >= 28 {
                if shade > 40 {
                    (shade & 7).wrapping_add(40)
                } else {
                    shade
                }
            } else {
                (shade & 3).wrapping_add(28)
            };
            let index = terrain_index(tile_x, tile_y);
            planes.shading[index] = if map_type == MapType::Day {
                shade as u8
            } else {
                64_i8.wrapping_sub(shade) as u8
            };
            if map_type == MapType::Cave {
                normalize_cave_clearance(planes, index);
            } else {
                planes.angles[index] &= 0xf7;
            }
        }
    }
}

pub(crate) fn apply_level1_load_path_point(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
    rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
}

pub(crate) fn level1_load_house_base(
    planes: &TerrainPlanes,
    tile_x: u8,
    tile_y: u8,
    footprint: &CastleBuildFootprint,
) -> (u8, u8, i16) {
    let mut center_x = tile_x;
    let center_y = tile_y;
    let mut start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    if start_x.wrapping_add(start_y) & 1 != 0 {
        center_x = center_x.wrapping_add(1);
        start_x = start_x.wrapping_add(1);
    }
    let corner_sum = u16::from(planes.ground_heights[terrain_index(start_x, start_y)])
        + u16::from(
            planes.ground_heights[terrain_index(start_x.wrapping_add(footprint.width), start_y)],
        )
        + u16::from(
            planes.ground_heights[terrain_index(
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(footprint.height),
            )],
        )
        + u16::from(
            planes.ground_heights[terrain_index(start_x, start_y.wrapping_add(footprint.height))],
        );
    (
        center_x,
        center_y,
        i16::try_from((corner_sum >> 2) * 32).expect("average u8 corner height fits i16"),
    )
}

pub(crate) fn apply_level1_load_house_sweep(
    planes: &mut TerrainPlanes,
    terrain_rng: &mut u16,
    position: Position,
    footprint: &CastleBuildFootprint,
    remaining: u8,
) {
    let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
    let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
    let start_x = center_x.wrapping_sub(footprint.width / 2);
    let start_y = center_y.wrapping_sub(footprint.height / 2);
    let construction_base = position.z >> 5;

    if remaining == 0 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    rebuild_building_topology(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        MapType::Night,
                        terrain_rng,
                    );
                }
            }
        }
        return;
    }

    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            let current = i16::from(planes.ground_heights[index]);
            let target = construction_base + i16::from(cell[1]);
            let next = current.wrapping_add((target - current) / i16::from(remaining));
            planes.ground_heights[index] = next.to_le_bytes()[0];
            if planes.angles[index].trailing_zeros() >= 3 {
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
            }
        }
    }

    if remaining.is_multiple_of(5) || remaining == 1 {
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] == 0xff {
                    continue;
                }
                let x = start_x.wrapping_add(column);
                let y = start_y.wrapping_add(row);
                let index = terrain_index(x, y);
                planes.angles[index] = (planes.angles[index] & 0xf0) | 1;
                planes.terrain_types[index] = 1;
            }
        }
        for row in 0..footprint.height {
            for column in 0..footprint.width {
                let cell = footprint.cells()
                    [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
                if cell[0] != 0xff {
                    apply_level1_house_material(
                        planes,
                        start_x.wrapping_add(column),
                        start_y.wrapping_add(row),
                        column,
                        cell[0],
                        terrain_rng,
                    );
                }
            }
        }
    }
}

pub(crate) fn apply_level1_load_house_edge_smoothing(
    planes: &mut TerrainPlanes,
    position: Position,
    footprint: &CastleBuildFootprint,
) {
    let center_x = position.x.wrapping_add(128).to_be_bytes()[0];
    let center_y = position.y.wrapping_add(128).to_be_bytes()[0];
    let footprint_start_x = center_x.wrapping_sub(footprint.width / 2);
    let footprint_start_y = center_y.wrapping_sub(footprint.height / 2);
    let half_width = footprint.width / 2;
    let half_height = footprint.height / 2;
    for radius in [2_u8, 5] {
        smooth_level1_house_edges(
            planes,
            footprint_start_x,
            footprint_start_y,
            half_width,
            half_height,
            radius,
        );
    }
}

pub(super) fn smooth_level1_house_edges(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    half_width: u8,
    half_height: u8,
    radius: u8,
) {
    let boundary_x = start_x.wrapping_sub(half_width);
    let boundary_y = start_y.wrapping_sub(half_height);
    for row in 0..half_height.wrapping_mul(2) {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
            smooth_level1_house_height(
                planes,
                boundary_x
                    .wrapping_add(half_width.wrapping_mul(2))
                    .wrapping_add(inset),
                boundary_y.wrapping_add(row),
            );
        }
    }
    for column in 0..half_width
        .wrapping_mul(2)
        .wrapping_add(radius.wrapping_mul(2))
    {
        for inset in 0..=radius {
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y.wrapping_sub(radius).wrapping_add(inset),
            );
            smooth_level1_house_height(
                planes,
                boundary_x.wrapping_sub(radius).wrapping_add(column),
                boundary_y
                    .wrapping_add(half_height.wrapping_mul(2))
                    .wrapping_add(inset),
            );
        }
    }
}

pub(super) fn smooth_level1_house_height(planes: &mut TerrainPlanes, tile_x: u8, tile_y: u8) {
    let index = terrain_index(tile_x, tile_y);
    if planes.angles[index].trailing_zeros() >= 3
        || planes.ground_heights[index] == 0
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x, tile_y.wrapping_sub(1))],
        )
        || !level1_house_smoothing_type(
            planes.terrain_types[terrain_index(tile_x.wrapping_sub(1), tile_y)],
        )
        || !level1_house_smoothing_type(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row in 0..3_u8 {
        for column in 0..3_u8 {
            let neighbor = terrain_index(
                tile_x.wrapping_sub(1).wrapping_add(column),
                tile_y.wrapping_sub(1).wrapping_add(row),
            );
            if level1_house_smoothing_type(planes.terrain_types[neighbor]) {
                height_sum += u32::from(planes.ground_heights[neighbor]);
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] =
            u8::try_from(height_sum / height_count).expect("average of u8 heights fits u8");
    }
}

pub(super) const fn level1_house_smoothing_type(terrain_type: u8) -> bool {
    terrain_type <= 5 || terrain_type > 0x22
}

pub(super) fn apply_level1_house_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    column: u8,
    material: u8,
    terrain_rng: &mut u16,
) {
    let index = terrain_index(x, y);
    if material < 8 {
        planes.angles[index] = (planes.angles[index] & 0xf0) | material;
        rebuild_building_topology(planes, x, y, x, y, MapType::Night, terrain_rng);
        return;
    }
    match material {
        8 => planes.terrain_types[index] = 8,
        9 => planes.terrain_types[index] = 9,
        16..=19 => {
            let corner = classify_castle_corner(planes, x, y, column);
            let table_offset = match material {
                16 => 2 * corner,
                17 => 0x40 + 2 * corner,
                18 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                19 => 0x60 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
                _ => unreachable!(),
            };
            let preserve = material == 16 && matches!(planes.terrain_types[index], 10..=12);
            if !preserve {
                let table = terrain_generation_table();
                planes.terrain_types[index] = table[table_offset];
                planes.angles[index] = (planes.angles[index] & 0x8f) | table[table_offset + 1];
            }
        }
        _ => return,
    }
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
    }
}

pub(crate) fn level1_feature_ground_height(planes: &TerrainPlanes, x: u16, y: u16) -> i16 {
    let bytes = interpolate_ground(&planes.ground_heights, x, y).to_le_bytes();
    i16::from_le_bytes([bytes[0], bytes[1]])
}

pub(super) fn terrain_generation_table() -> &'static [u8; 0x120] {
    static TABLE: OnceLock<[u8; 0x120]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let source = include_str!(
            "../../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D4A30.cpp"
        );
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-generation initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-generation terminator")
            .0;
        let initializer = initializer
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(values, _)| values))
            .collect::<String>();
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(0x120);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain-generation byte");
                values.push(
                    u8::from_str_radix(hex, 16).expect("terrain-generation hexadecimal byte"),
                );
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 0x120);
        values
            .try_into()
            .expect("terrain-generation table has fixed extent")
    })
}

pub(super) fn building_topology_entry(index: usize) -> [u8; 2] {
    debug_assert!(index < 7_usize.pow(4));
    let catalog = terrain_corner_catalog();
    for (terrain_type, corners) in catalog.iter().enumerate() {
        if corners.iter().any(|corner| *corner < 0) {
            continue;
        }
        let c = corners.map(|corner| corner as usize);
        for (candidate, rotation) in [
            (343 * c[0] + 49 * c[1] + 7 * c[2] + c[3], 0),
            (343 * c[1] + 49 * c[0] + 7 * c[3] + c[2], 16),
            (343 * c[2] + 49 * c[3] + 7 * c[0] + c[1], 48),
            (343 * c[3] + 49 * c[2] + 7 * c[1] + c[0], 32),
            (343 * c[1] + 49 * c[2] + 7 * c[3] + c[0], 96),
            (343 * c[2] + 49 * c[1] + 7 * c[0] + c[3], 112),
            (343 * c[3] + 49 * c[0] + 7 * c[1] + c[2], 80),
            (343 * c[0] + 49 * c[3] + 7 * c[2] + c[1], 64),
        ] {
            if candidate == index {
                return [terrain_type as u8, rotation];
            }
        }
    }
    [1, 0]
}

pub(super) fn terrain_corner_catalog() -> &'static [[i8; 4]; 148] {
    static CATALOG: OnceLock<[[i8; 4]; 148]> = OnceLock::new();
    CATALOG.get_or_init(|| {
        // remc2 keeps this functional terrain-topology catalog in a dedicated
        // source table. Parsing its hexadecimal initializer here avoids
        // duplicating a second hand-maintained 592-byte private table.
        let source = include_str!(
            "../../../../../../../upstream/magic-carpet-2-hd/remc2/engine/Unk_D47E0.cpp"
        );
        let initializer = source
            .split_once("{//terrain gen")
            .expect("remc2 terrain-corner initializer")
            .1
            .split_once("};")
            .expect("remc2 terrain-corner terminator")
            .0;
        let bytes = initializer.as_bytes();
        let mut values = Vec::with_capacity(148 * 4);
        let mut cursor = 0;
        while cursor + 3 < bytes.len() {
            if bytes[cursor] == b'0'
                && bytes[cursor + 1] == b'x'
                && bytes[cursor + 2].is_ascii_hexdigit()
                && bytes[cursor + 3].is_ascii_hexdigit()
            {
                let hex = std::str::from_utf8(&bytes[cursor + 2..cursor + 4])
                    .expect("ASCII hexadecimal terrain corner");
                values.push(u8::from_str_radix(hex, 16).expect("terrain corner byte") as i8);
                cursor += 4;
            } else {
                cursor += 1;
            }
        }
        assert_eq!(values.len(), 148 * 4);
        let mut result = [[0_i8; 4]; 148];
        for (row, values) in result.iter_mut().zip(values.chunks_exact(4)) {
            row.copy_from_slice(values);
        }
        result
    })
}

pub(super) fn classify_castle_corner(planes: &TerrainPlanes, x: u8, y: u8, fallback: u8) -> usize {
    let heights = [
        planes.ground_heights[terrain_index(x, y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y)],
        planes.ground_heights[terrain_index(x.wrapping_add(1), y.wrapping_add(1))],
        planes.ground_heights[terrain_index(x, y.wrapping_add(1))],
    ];
    let mut maximum = 0_u8;
    let mut maximum_corner = 0_usize;
    let mut minimum = u8::MAX;
    for (corner, height) in heights.into_iter().enumerate() {
        if height > maximum {
            maximum = height;
            maximum_corner = corner;
        }
        minimum = minimum.min(height);
    }
    let mut second_maximum = 0_u8;
    let mut second_corner = usize::from(fallback);
    for (corner, height) in heights.into_iter().enumerate() {
        if corner != maximum_corner && height > second_maximum {
            second_maximum = height;
            second_corner = corner;
        }
    }
    if maximum.wrapping_sub(minimum) >= 8 && maximum.wrapping_sub(second_maximum) >= 8 {
        return maximum_corner;
    }
    match maximum_corner {
        0 => {
            if second_corner == 1 {
                4
            } else {
                7
            }
        }
        1 => {
            if second_corner == 2 {
                5
            } else {
                4
            }
        }
        2 => {
            if second_corner == 3 {
                6
            } else {
                5
            }
        }
        3 => {
            if second_corner != 0 {
                6
            } else {
                7
            }
        }
        _ => 0,
    }
}

pub(super) fn apply_castle_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    material: u8,
    map_type: MapType,
) {
    let index = terrain_index(x, y);
    let preserve_existing_material =
        material == 0x10 && matches!(planes.terrain_types[index], 10 | 11 | 12);
    if !preserve_existing_material {
        let corner = classify_castle_corner(planes, x, y, 7);
        let table_offset = match material {
            0x10 => 2 * corner,
            0x11 => 0x40 + 2 * corner,
            0x12 => 0x50 + 2 * (8 * usize::from(x.wrapping_add(y) & 1) + corner),
            _ => {
                // Level 2's qualified BUILD stages use only the admitted
                // 0x10, 0x11, and 0x12 controls here.
                return;
            }
        };
        planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
        planes.angles[index] =
            (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    }
    planes.angles[index] |= 0x80;
    if map_type == MapType::Cave {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            normalize_cave_clearance(planes, terrain_index(corner_x, corner_y));
        }
    } else {
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            planes.angles[terrain_index(corner_x, corner_y)] &= 0xf7;
        }
    }
}

pub(super) fn apply_ambient_material(
    planes: &mut TerrainPlanes,
    x: u8,
    y: u8,
    in_type: u8,
    target_type: u8,
    map_type: MapType,
) -> std::result::Result<(), String> {
    let base_offset = match target_type {
        20 => 0x10,
        21 => 0x20,
        22 => 0x30,
        _ => {
            return Err(format!(
                "ambient terrain target {target_type} is outside 20..=22"
            ));
        }
    };
    let corner = classify_castle_corner(planes, x, y, in_type);
    let table_offset = base_offset + 2 * corner;
    let index = terrain_index(x, y);
    planes.terrain_types[index] = TERRAIN_GENERATION_TABLE_PREFIX[table_offset];
    planes.angles[index] =
        (planes.angles[index] & 0x8f) | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1];
    planes.angles[index] |= 0x80;
    for (corner_x, corner_y) in [
        (x, y),
        (x.wrapping_add(1), y),
        (x.wrapping_add(1), y.wrapping_add(1)),
        (x, y.wrapping_add(1)),
    ] {
        let index = terrain_index(corner_x, corner_y);
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        } else {
            planes.angles[index] &= 0xf7;
        }
    }
    Ok(())
}

pub(super) fn apply_ambient_collision(
    planes: &mut TerrainPlanes,
    position: Position,
    height_delta: i8,
    map_type: MapType,
    terrain_rng: &mut u16,
) {
    let base_x = (position.x as i16).wrapping_add(128) >> 8;
    let base_y = (position.y as i16).wrapping_add(128) >> 8;
    for (offset_x, offset_y) in [(0_i16, 0_i16), (1, 0), (0, 1), (1, 1)] {
        let signed_x = base_x.wrapping_add(offset_x);
        let signed_y = base_y.wrapping_add(offset_y);
        let x = signed_x as u8;
        let y = signed_y as u8;
        let index = terrain_index(x, y);
        if planes.angles[index] & 0x80 != 0 {
            break;
        }
        let unclamped = i16::from(planes.ground_heights[index]) + i16::from(height_delta);
        let height = unclamped.clamp(0, 200) as u8;
        planes.ground_heights[index] = height;
        if map_type == MapType::Cave {
            let secondary = i16::from(planes.secondary_heights[index]) - i16::from(height_delta);
            planes.secondary_heights[index] = if secondary >= 255 {
                u8::MAX
            } else {
                secondary as u8
            };
        }
        if height != 0 {
            planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
        } else {
            let allowed = [
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
                (x.wrapping_add(1), y.wrapping_sub(1)),
                (x.wrapping_add(1), y),
                (x.wrapping_sub(1), y),
                (x.wrapping_sub(1), y.wrapping_add(1)),
                (x, y.wrapping_add(1)),
                (x.wrapping_add(1), y.wrapping_add(1)),
            ]
            .into_iter()
            .all(|(x, y)| {
                let angle = planes.angles[terrain_index(x, y)] & 7;
                !matches!(angle, 2 | 3 | 5)
            });
            if allowed {
                planes.angles[index] &= 0xf0;
            }
        }
        add_building_to_terrain(planes, x, y, map_type, terrain_rng);
        if !(0..=200).contains(&unclamped) && signed_x == 0 && signed_y == 0 {
            break;
        }
    }
}

pub(super) fn normalize_cave_clearance(planes: &mut TerrainPlanes, index: usize) {
    if planes.secondary_heights[index] > planes.ground_heights[index] {
        planes.angles[index] &= 0xf7;
    } else {
        planes.secondary_heights[index] = planes.ground_heights[index].wrapping_sub(1);
        planes.angles[index] |= 0x08;
    }
}

pub(super) fn remove_castle_stage(
    planes: &mut TerrainPlanes,
    start_x: u8,
    start_y: u8,
    footprint: &CastleBuildFootprint,
    map_type: MapType,
    terrain_rng: &mut u16,
    mut entity_rng: u16,
) -> u16 {
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            if cell[0] == 0xff && cell[1] == 0xff {
                continue;
            }
            let x = start_x.wrapping_add(column);
            let y = start_y.wrapping_add(row);
            let index = terrain_index(x, y);
            planes.angles[index] = (planes.angles[index] & 0x70) | 1;
            add_building_to_terrain(planes, x, y, map_type, terrain_rng);
            if cell[1] == 0xff {
                continue;
            }
            let offset = cell[1];
            let height = planes.ground_heights[index];
            if offset >= height {
                planes.ground_heights[index] = 0;
                continue;
            }
            entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
            if entity_rng % 50 <= 20 {
                planes.ground_heights[index] = height.wrapping_sub(offset);
            } else {
                entity_rng = entity_rng.wrapping_mul(9_377).wrapping_add(9_439);
                planes.ground_heights[index] = height
                    .wrapping_sub(offset)
                    .wrapping_add((entity_rng % 20) as u8);
            }
        }
    }
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            smooth_building_height(
                planes,
                start_x.wrapping_add(column),
                start_y.wrapping_add(row),
                map_type,
            );
        }
    }
    entity_rng
}

pub(super) const fn next_house_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

pub(super) fn smooth_building_height(planes: &mut TerrainPlanes, x: u8, y: u8, map_type: MapType) {
    let index = terrain_index(x, y);
    let ordinary = |terrain_type: u8| terrain_type <= 5 || terrain_type > 0x22;
    if planes.angles[index] & 7 == 0
        || planes.ground_heights[index] == 0
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x, y.wrapping_sub(1))])
        || !ordinary(planes.terrain_types[terrain_index(x.wrapping_sub(1), y)])
        || !ordinary(planes.terrain_types[index])
    {
        return;
    }
    let mut height_sum = 0_u32;
    let mut height_count = 0_u32;
    for row_offset in [u8::MAX, 0, 1] {
        for column_offset in [u8::MAX, 0, 1] {
            let neighbor = terrain_index(x.wrapping_add(column_offset), y.wrapping_add(row_offset));
            if ordinary(planes.terrain_types[neighbor]) {
                height_sum = height_sum.wrapping_add(u32::from(planes.ground_heights[neighbor]));
                height_count += 1;
            }
        }
    }
    if height_count != 0 {
        planes.ground_heights[index] = (height_sum / height_count) as u8;
        if map_type == MapType::Cave {
            normalize_cave_clearance(planes, index);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRelation {
    Scalar(u32),
    AuthoredFeature(u16),
    LiveEntity(u16),
    LiveEntityIndex(u16),
    AxisScalar(u32),
    Null,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStage {
    pub kind: i8,
    pub flags: i8,
    pub axis_x: i16,
    pub axis_y: i16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeStageVariable {
    pub kind: i8,
    pub flags: i8,
    pub word_2: u16,
    pub relation: RuntimeRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub global_turn: u32,
    pub random_state: u32,
    pub local_player_slot: u8,
    pub player_count: u8,
    pub maptype_music: i32,
    pub objective_state: crate::campaign::objectives::ObjectiveState,
    pub stages: Box<[RuntimeStage]>,
    pub stage_variables: Box<[RuntimeStageVariable]>,
    pub player_inputs: [PlayerCommand; PLAYER_COUNT],
    pub(super) bytes: Box<[u8]>,
}

impl std::ops::Deref for RuntimeSnapshot {
    type Target = crate::campaign::objectives::ObjectiveState;

    fn deref(&self) -> &Self::Target {
        &self.objective_state
    }
}

impl std::ops::DerefMut for RuntimeSnapshot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.objective_state
    }
}

impl RuntimeSnapshot {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// `sub_12330` advances only the high byte of the selected runtime
    /// StageVar's word before applying its thinning rule. Keep the decoded
    /// record and the lossless runtime byte carrier synchronized so reset and
    /// campaign hashing observe the same construction cadence.
    pub(super) fn advance_level2_stage_variable_counter(&mut self, stage_var_index: u8) -> bool {
        let Some(stage_var) = self.stage_variables.get_mut(usize::from(stage_var_index)) else {
            return false;
        };
        let counter = (stage_var.word_2 >> 8) as u8;
        stage_var.word_2 = (stage_var.word_2 & 0x00ff) | (u16::from(counter.wrapping_add(1)) << 8);
        let offset = 392 + usize::from(stage_var_index) * 12 + 2;
        let Some(bytes) = self.bytes.get_mut(offset..offset + 2) else {
            return false;
        };
        bytes.copy_from_slice(&stage_var.word_2.to_le_bytes());
        true
    }

    pub(super) fn set_level2_objective_stage_selector_for(
        &mut self,
        objective_index: usize,
        stage_index: usize,
        state: u8,
    ) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            if let Some(selector) = objective.stage_selectors.get_mut(stage_index) {
                *selector = state;
                self.bytes[304 + objective_index * 11 + stage_index + 3] = state;
            }
        }
    }

    pub(super) fn set_level2_objective_stage_selector(&mut self, stage_index: usize, state: u8) {
        self.set_level2_objective_stage_selector_for(0, stage_index, state);
    }

    #[cfg(test)]
    pub(super) fn set_level2_objective_text_selector(&mut self, stage_index: usize) {
        self.set_level2_objective_text_selector_for(0, stage_index);
    }

    pub(super) fn set_level2_objective_text_selector_for(
        &mut self,
        objective_index: usize,
        stage_index: usize,
    ) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            if let Ok(selector) = u8::try_from(stage_index) {
                objective.text_selector = selector;
                self.bytes[304 + objective_index * 11 + 1] = selector;
            }
        }
    }

    pub(super) fn set_level2_objective_stage_state(&mut self, state: u8) {
        self.set_level2_objective_stage_selector(LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX, state);
    }

    pub(super) fn set_level2_objective_level_end_for(&mut self, objective_index: usize, state: u8) {
        if let Some(objective) = self.objectives.get_mut(objective_index) {
            objective.is_level_end = state;
            self.bytes[304 + objective_index * 11] = state;
        }
    }

    /// Bind an authored event into the stage table as remc2 `sub_58DA0` does
    /// for stage kinds 1/2/4/6.  The exported runtime uses an authored-feature
    /// relation for the pre-construction pointer; after construction kinds
    /// 1/2/4 retain a one-based live-entity relation while kind 6 stores the
    /// zero-based entity index.  The source also sets the stage's bound flag.
    pub(super) fn bind_authored_stage_entity(
        &mut self,
        authored_index: u16,
        source_slot: u16,
    ) -> usize {
        let mut bound = 0;
        for (stage_index, stage) in self.stages.iter_mut().enumerate() {
            if !matches!(stage.kind, 1 | 2 | 4 | 6)
                || stage.relation != RuntimeRelation::AuthoredFeature(authored_index)
            {
                continue;
            }
            stage.relation = if stage.kind == 6 {
                RuntimeRelation::LiveEntityIndex(source_slot)
            } else {
                RuntimeRelation::LiveEntity(source_slot)
            };
            stage.flags |= 1;
            let offset = 192 + stage_index * 14;
            self.bytes[offset + 1] = stage.flags as u8;
            if stage.kind == 6 {
                self.bytes[offset + 6] = 3;
                self.bytes[offset + 10..offset + 14]
                    .copy_from_slice(&u32::from(source_slot).to_le_bytes());
            } else {
                self.bytes[offset + 6] = 2;
                self.bytes[offset + 10..offset + 14]
                    .copy_from_slice(&u32::from(source_slot).wrapping_add(1).to_le_bytes());
            }
            bound += 1;
        }
        bound
    }

    pub(super) fn mark_level2_objective_done_for(&mut self, player_color_index: u8) {
        let index = usize::from(player_color_index);
        if let Some(objective) = self.objectives.get_mut(index) {
            objective.done = 1;
            self.bytes[304 + index * 11 + 2] = 1;
        }
    }

    #[cfg(test)]
    pub(super) fn set_level2_is_level_end(&mut self, state: u8) {
        if let Some(objective) = self.objectives.get_mut(0) {
            objective.is_level_end = state;
            self.bytes[304] = state;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeObjective {
    pub is_level_end: u8,
    pub text_selector: u8,
    pub done: u8,
    pub stage_selectors: [u8; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticleParameter {
    pub base_tmap: u16,
    pub word_2: u16,
    pub word_4: u16,
    pub speed: u16,
    pub rotation_speed: u16,
    pub byte_10: u8,
    pub byte_11: i8,
    pub render_mode: u8,
    pub byte_13: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSelectors {
    pub particles: Box<[ParticleParameter]>,
    pub level_sprite_selectors: Box<[u8]>,
    pub entity_parameters: Box<[[u8; ENTITY_PARAMETER_BYTES]]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameObservation {
    pub ordinal: u16,
    pub category: String,
    pub hook: String,
    pub stage: Option<String>,
    pub selector: Value,
    pub event_sha256: String,
    pub event: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    pub asset_id: String,
    pub content_address: String,
    pub owner_asset_ids: Box<[String]>,
    pub observation: FrameObservation,
}

/// An immutable, validated owner-data world.
#[derive(Clone, Debug)]
pub struct NativeWorld {
    pub(super) metadata: LevelMetadata,
    pub(super) package_sha256: String,
    pub(super) closure_sha256: String,
    pub(super) profile: FlightProfile,
    pub(super) terrain_planes: TerrainPlanes,
    pub(super) terrain: LegacyTerrain,
    pub(super) entities: Box<[EntitySnapshot]>,
    pub(super) players: Box<[PlayerSnapshot]>,
    pub(super) runtime: RuntimeSnapshot,
    pub(super) presentation: PresentationSelectors,
    pub(super) asset_refs: Box<[AssetRef]>,
    pub(super) sound_asset_refs: BTreeMap<u16, u32>,
    pub(super) music_asset_ref: Option<u32>,
    pub(super) goat_tmap_bank: Option<GoatTmapBank>,
    pub(super) source_sprite_tmap_bank: Option<SourceSpriteTmapBank>,
    pub(super) reset_source_map_chains: RenderableMapChains,
    pub(super) raw_source_entity_indices: Box<[u16]>,
    pub(super) house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
    pub(super) reset_state: FlightState,
    pub(super) reset_context: FlightContext,
    pub(super) goat_stage_point: Position,
    pub(super) goat_config: GoatBehaviorConfig,
    pub(super) reset_live_world: LiveWorldState,
    pub(super) level1_mission: Level1MissionTemplate,
}

/// Common owner data decoded for a profile before mission-specific behavior is
/// constructed. Level 2 uses this inspection seam while its mission dispatch
/// remains intentionally unsupported; it shares the same typed component
/// arenas and parsers as Level 1.
#[derive(Clone, Debug)]
pub struct NativeWorldInspection {
    pub(super) profile: OwnerPackageProfile,
    pub(super) schema: OwnerPackageSchema,
    pub(super) metadata: LevelMetadata,
    pub(super) package_sha256: String,
    pub(super) terrain_planes: TerrainPlanes,
    pub(super) terrain: LegacyTerrain,
    pub(super) entities: Box<[EntitySnapshot]>,
    pub(super) players: Box<[PlayerSnapshot]>,
    pub(super) runtime: RuntimeSnapshot,
    pub(super) presentation: PresentationSelectors,
    pub(super) terrain_rng: u16,
    pub(super) reset_state: Option<FlightState>,
    pub(super) authored_level: Box<[u8]>,
    pub(super) collision_masks: Option<CollisionMaskBank>,
    pub(super) level2_castle_stage_footprints: BTreeMap<u8, CastleBuildFootprint>,
    pub(super) level2_house_build_footprints: BTreeMap<u8, CastleBuildFootprint>,
}

impl NativeWorldInspection {
    #[must_use]
    pub const fn schema(&self) -> OwnerPackageSchema {
        self.schema
    }

    #[must_use]
    pub const fn profile(&self) -> OwnerPackageProfile {
        self.profile
    }

    #[must_use]
    pub const fn metadata(&self) -> &LevelMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn package_sha256(&self) -> &str {
        self.package_sha256.as_str()
    }

    #[must_use]
    pub const fn terrain_planes(&self) -> &TerrainPlanes {
        &self.terrain_planes
    }

    #[must_use]
    pub const fn terrain(&self) -> &LegacyTerrain {
        &self.terrain
    }

    #[must_use]
    pub fn entities(&self) -> &[EntitySnapshot] {
        &self.entities
    }

    #[must_use]
    pub fn players(&self) -> &[PlayerSnapshot] {
        &self.players
    }

    #[must_use]
    pub const fn runtime(&self) -> &RuntimeSnapshot {
        &self.runtime
    }

    #[must_use]
    pub const fn presentation(&self) -> &PresentationSelectors {
        &self.presentation
    }

    #[must_use]
    pub const fn terrain_rng(&self) -> u16 {
        self.terrain_rng
    }

    #[must_use]
    pub const fn reset_state(&self) -> Option<&FlightState> {
        self.reset_state.as_ref()
    }

    /// Borrow the immutable source-exact Level 2 collision bank. Historical
    /// v3 packages intentionally return `None`; they are not collision
    /// authority and cannot be upgraded by inference.
    #[must_use]
    pub const fn collision_mask_bank(&self) -> Option<&CollisionMaskBank> {
        self.collision_masks.as_ref()
    }

    /// Query remc2 model-45 occupancy through the validated Level 2 carrier.
    pub fn model45_occupancy(&self, selector: u16, x: i32, y: i32) -> Result<bool> {
        let bank = self.collision_mask_bank().ok_or_else(|| {
            NativeWorldError::new("Level 2 collision occupancy requires a qualified v5 package")
        })?;
        bank.model45_occupancy(selector, x, y)
            .map_err(|error: CollisionMaskLookupError| {
                NativeWorldError::new(format!("model-45 occupancy lookup failed: {error:?}"))
            })
    }

    /// Select the typed mission profile from metadata already checked by the
    /// owner-package loader. Unknown metadata cannot fall back to Level 1.
    pub fn mission_content_profile(&self) -> Result<MissionContentProfile> {
        MissionContentProfile::from_metadata(
            self.metadata.external_level,
            self.metadata.internal_level,
            self.metadata.map_type.id(),
        )
        .map_err(|error| NativeWorldError::new(error.to_string()))
    }

    /// Build the generic authored/reset template from the qualified package.
    /// This retains all 1,200 authored records and all 1,000 reset slots before
    /// any Level 2 behavior dispatcher is selected.
    pub fn mission_content_template(&self) -> Result<MissionContentTemplate> {
        let profile = self.mission_content_profile()?;
        let slots = self.mission_reset_slots();
        MissionContentTemplate::decode(
            profile,
            &self.authored_level,
            &slots,
            self.runtime.random_state,
        )
        .map_err(|error| NativeWorldError::new(error.to_string()))
    }

    /// Build the mission template selected by a validated campaign
    /// definition. The descriptor identity and package hash, rather than the
    /// compatibility `OwnerPackageProfile`, choose the compiled decoder.
    pub(crate) fn mission_content_template_from_definition(
        &self,
        definition: &ValidatedCampaignDefinition,
    ) -> Result<MissionContentTemplate> {
        self.validate_campaign_definition_binding(definition)?;
        MissionContentTemplate::decode_campaign_definition(
            definition,
            &self.authored_level,
            &self.mission_reset_slots(),
            self.runtime.random_state,
        )
        .map_err(|error| NativeWorldError::new(error.to_string()))
    }

    fn mission_reset_slots(&self) -> Vec<Level1ResetSourceSlot> {
        self.entities
            .iter()
            .map(|entity| Level1ResetSourceSlot {
                index: entity.index,
                active: entity.active,
                class: entity.class,
                model: entity.model,
                action: entity.action,
                phase: entity.phase,
                entity_rng: entity.entity_rng,
                control_flags: entity.control_flags,
            })
            .collect()
    }

    fn validate_campaign_definition_binding(
        &self,
        definition: &ValidatedCampaignDefinition,
    ) -> Result<()> {
        let owner_export_package_sha256 = definition
            .owner_export_package_sha256()
            .map_err(|error| NativeWorldError::new(error.to_string()))?;
        if self.package_sha256 != owner_export_package_sha256
            || self.metadata.external_level != definition.identity.external_level
            || self.metadata.internal_level != definition.identity.internal_level
            || self.metadata.map_type.id() != definition.identity.map_type_id
        {
            return Err(NativeWorldError::new(
                "campaign definition identity/package does not match decoded inspection",
            ));
        }
        Ok(())
    }

    /// Construct an isolated deterministic session from the generic profile.
    pub fn mission_content_session(&self) -> Result<MissionContentSession> {
        Ok(self.mission_content_template()?.session())
    }

    /// Build the profile-specific Level 2 frame-zero session. All mutable
    /// state is cloned from this validated inspection; no implementation
    /// fixture or stale Level 1 reset projection is consulted.
    pub fn level2_reset_session(&self) -> Result<Level2ResetSession> {
        let mission = self.mission_content_template()?.session();
        self.build_level2_reset_session(mission)
    }

    /// Construct the Level 2 compatibility session beneath the neutral,
    /// descriptor-backed campaign owner.
    pub(crate) fn level2_reset_session_from_definition(
        &self,
        definition: &ValidatedCampaignDefinition,
    ) -> Result<Level2ResetSession> {
        let mission = self
            .mission_content_template_from_definition(definition)?
            .session();
        self.build_level2_reset_session(mission)
    }

    fn build_level2_reset_session(
        &self,
        mission: MissionContentSession,
    ) -> Result<Level2ResetSession> {
        if self.profile != OwnerPackageProfile::Level2 {
            return Err(NativeWorldError::new(
                "Level 2 reset session requested for a non-Level 2 package",
            ));
        }
        // Inspection deliberately accepts historical v3/v4 package rows, but
        // they have no source-backed type-3 banishment carrier.  Reject before
        // constructing any mutable mission/session state rather than creating
        // a `None` session that merely suppresses objective completion.
        let stage_count = usize::from(self.runtime.stage_index).min(self.runtime.stages.len());
        if self
            .runtime
            .stages
            .iter()
            .take(stage_count)
            .any(|stage| stage.kind == 3)
            && self
                .players
                .iter()
                .any(|player| player.banishment_state_0x006.is_none())
        {
            return Err(NativeWorldError::new(
                "Level 2 type-3 objective requires a source banishment-state carrier before session creation",
            ));
        }
        let state = self
            .reset_state
            .ok_or_else(|| NativeWorldError::new("validated Level 2 package has no reset state"))?;
        let nyphur_dispatcher = Level2NyphurDispatcher::from_qualified_reset(
            &self.players,
            &self.entities,
            &self.runtime,
            &mission,
        )?;
        let bad_stones = level2_bad_stones(&self.entities, &self.terrain_planes)?;
        let mut economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
            SessionTerrain::from_inspection(self),
            &self.entities,
            self.entities[usize::from(self.players[0].entity_index)].mana,
            self.entities[usize::from(self.players[0].entity_index)].model,
            0,
            self.players[0].enabled_spell_holders[2],
            self.runtime.random_state,
            self.level2_castle_stage_footprints.clone(),
        )?;
        let wizard = mission
            .wizard_map_settings()
            .first()
            .ok_or_else(|| NativeWorldError::new("Level 2 Wizard 0 settings are absent"))?;
        let shared_spells = Level2SharedSpellResetSession::from_parts_with_starting_spells(
            SessionTerrain::from_inspection(self),
            &self.entities,
            &self.players,
            &self.presentation,
            Some((&mut economy, wizard)),
        )?;
        let castle_holder = shared_spells
            .dynamic_pickups
            .iter()
            .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
            .ok_or_else(|| NativeWorldError::new("Level 2 family-2 starting holder is absent"))?;
        economy.initialize_player_spell_cleanup_context(
            self.entities[usize::from(self.players[0].entity_index)].model,
            0,
            castle_holder.index,
            castle_holder.active_value.cast_unsigned(),
            castle_holder.activation_value,
        );
        let ordinary = Level2OrdinaryResetSession::from_parts(
            SessionTerrain::from_inspection(self),
            self.entities.clone(),
            self.players.clone(),
            self.presentation.clone(),
            self.level2_house_build_footprints.clone(),
        )?;
        let goat_config = GoatBehaviorConfig::from_level_two_parameter_record(
            self.presentation
                .entity_parameters
                .get(98)
                .ok_or_else(|| NativeWorldError::new("Level 2 Goat parameter row 98 is absent"))?,
        )
        .map_err(NativeWorldError::new)?;
        let goats =
            GoatFamily::from_level_two_entities(&self.entities, GOAT_STAGE_POINT, goat_config)
                .map_err(NativeWorldError::new)?;
        Ok(Level2ResetSession::new(
            self.metadata.map_type.profile(),
            self.package_sha256.clone(),
            state,
            SessionTerrain::from_inspection(self),
            self.entities.clone(),
            self.players.clone(),
            self.runtime.clone(),
            self.presentation.clone(),
            bad_stones,
            shared_spells,
            economy,
            ordinary,
            goats,
            mission,
            nyphur_dispatcher,
            self.collision_masks.clone(),
        ))
    }

    /// Mission construction is intentionally fail-closed until the Level 2
    /// content profile is implemented by the later mission beads.
    pub fn mission_unimplemented(&self) -> Result<()> {
        Err(NativeWorldError::new(format!(
            "mission behavior is unsupported for {:?} owner profile",
            self.profile
        )))
    }
}

/// Production Level 2 reset projection for the ordinary resident/static
/// families.  It owns the shared `HouseFamily`/`VillagerFamily` behavior and
/// the three typed dolmens; no Level 1 slot assumptions are consulted.
pub(super) const LEVEL2_SHARED_SPELL_PICKUP_SLOTS: [u16; 2] = [38, 75];
pub(super) const LEVEL2_SHARED_SPELL_HOLDER_SLOTS: [u16; 2] = [113, 114];
/// Authored Level 2 spell pickups which are materialised by the source event
/// constructors rather than occupying a frame-zero reset slot.  The indices
/// are the pinned authored-level rows (155/156); row 38 is the active reset
/// pickup for family 12.  Keeping these source rows explicit lets the generic
/// `sub_68FF0` acquisition seam be reached from the production reset session
/// without pretending that the authored rows are already live entities.
pub(super) const LEVEL2_GENERIC_SPELL_PICKUPS: [(u8, u16, u8); 3] =
    [(6, 155, 19), (11, 156, 34), (12, 38, 37)];
pub(super) const LEVEL2_SHARED_SPELL_ENTITY_SHA256: [&str; 4] = [
    "21f016732b7a687ac53cd9fc452a148dce204e4944b30755abc2f312c5460182",
    "899b6f9f0c614cdb77c1c7c3e7922ac6eadd03391fdd252f347b29d93fee3e81",
    "9312c195108896b32d553bdce73a43484bf80e010fcf762fc7c01d7f7ecca954",
    "fedeb2b2fbe40c6ec72614bc4a971e2e826d52a8348156040e872c14a7c0a80e",
];
pub(super) const LEVEL2_SHARED_SPELL_PLAYER_ENTITY_SHA256: &str =
    "b5234d405349902b3ef653ed52ac175c929b6024a19869a503fbe65f190d3834";
pub(super) const LEVEL2_NYPHUR_ENTITY_INDEX: u16 = 112;
pub(super) const LEVEL2_NYPHUR_FIREBALL_HOLDER_INDEX: u16 = 113;
pub(super) const LEVEL2_NYPHUR_POSSESSION_HOLDER_INDEX: u16 = 114;
/// The qualified Level 2 objective ledger selects stage index 3 for the
/// first type-5 point objective.  Model-32's switch consumes this exact
/// stage byte after the objective evaluator writes the source completion
/// sentinel.
pub(super) const LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX: usize = 3;
#[cfg(test)]
pub(super) const LEVEL2_BEE_OBJECTIVE_STAGE_INDEX: usize = 2;
pub(super) const LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX: u16 = 229;
/// `str_D7BD6[73]` values consumed by `sub_1BF90`'s class-3 target scan.
pub(super) const LEVEL2_BEE_AWARENESS_RADIUS: i64 = 0x1400;
pub(super) const LEVEL2_BEE_AWARENESS_HALF_ANGLE: i32 = 0x02AA;
/// `sub_1CE80` accepts a strict 3-D contact only below 1024 source units.
pub(super) const LEVEL2_BEE_CONTACT_RADIUS: u32 = 1024;
/// Row-229's constructor subspell (`subSpellIndex_0x2A_42`) is 200 in the
/// qualified Day package; the live source field remains authoritative below.
#[cfg(test)]
pub(super) const LEVEL2_BEE_CONTACT_SUBSPELL: i32 = 200;
/// Descriptor-73 movement values consumed by `sub_1B8C0` before the action-17
/// cadence scan. These are source fields, not tuning constants: the Bee
/// settles to 51 units above terrain, relaxes by -32 while airborne, probes
/// a 128-unit collision footprint, and rejects a tile slope of 20 or more.
pub(super) const LEVEL2_BEE_GROUND_CLEARANCE: i16 = 0x0033;
pub(super) const LEVEL2_BEE_GROUND_RELAXATION: i16 = -32;
pub(super) const LEVEL2_BEE_COLLISION_HALF_EXTENT: i16 = 128;
/// `sub_106C0` Fireball source dimensions.  These are kept local to the Day
/// producer because the Level 1 projectile helper owns the same values for a
/// different arena, while this route must not create a second shared
/// projectile framework.
pub(super) const LEVEL2_FIREBALL_COLLISION_HALF_WIDTH: i16 = 48;
pub(super) const LEVEL2_FIREBALL_COLLISION_VERTICAL_OFFSET: i16 = 50;
pub(super) const LEVEL2_FIREBALL_COLLISION_HALF_HEIGHT: i16 = 50;
pub(super) const LEVEL2_BEE_TERRAIN_THRESHOLD: i32 = 20;
pub(super) const LEVEL2_BEE_TURN_LIMIT: i16 = 0x0071;
pub(super) const LEVEL2_BEE_MOVEMENT_SKIP_BIT: u32 = 0x0000_0800;
pub(super) const LEVEL2_BEE_OBSTRUCTION_BIT: u32 = 0x0004_0000;
pub(super) const LEVEL2_OBJECTIVE_POINT_RADIUS: i32 = 768;
pub(super) const LEVEL2_NYPHUR_ENTITY_SHA256: &str =
    "a240c8711868a249b2e3ebb43c59613dc88d979a07de279ece869f14ffcf888e";
/// Qualified frame-zero `byte_0x3E_62` for the Nyphur entity. The source
/// dispatcher increments this byte after its callback, so the census hash
/// below normalizes only this known mutable field.
pub(super) const LEVEL2_NYPHUR_INITIAL_PHASE: u8 = 113;
pub(super) const LEVEL2_NYPHUR_PLAYER_SHA256: &str =
    "3859adcd13d8f32f27220c29e678581f4563fd051275ecc8b48390352dda5159";
pub(super) const LEVEL2_NYPHUR_FIREBALL_SHA256: &str =
    "9312c195108896b32d553bdce73a43484bf80e010fcf762fc7c01d7f7ecca954";
pub(super) const LEVEL2_NYPHUR_POSSESSION_SHA256: &str =
    "fedeb2b2fbe40c6ec72614bc4a971e2e826d52a8348156040e872c14a7c0a80e";
