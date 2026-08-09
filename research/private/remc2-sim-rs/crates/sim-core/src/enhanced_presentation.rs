//! Generation-bound, Rust-owned state for the Unity Enhanced presentation.
//!
//! The ledger is deliberately renderer-neutral.  It turns typed Level 1
//! state into immutable, source-ordered records and event pages.  Unity never
//! writes to the ledger, advances it, or supplies collision/light/gameplay
//! decisions.  A frame is staged transactionally and either commits as one
//! snapshot or leaves the previous snapshot untouched.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use crate::classic_hd::byte_hash;

pub const ENHANCED_ENTITY_ACTIVE: u32 = 1 << 0;
pub const ENHANCED_ENTITY_VISIBLE: u32 = 1 << 1;
pub const ENHANCED_EVENT_PRESENTATION_ONLY: u32 = 1 << 0;
pub const ENHANCED_MAX_EVENT_BACKLOG: usize = 16_384;
pub const ENHANCED_PROFILE_UNITY: u8 = 2;
pub const ENHANCED_PROFILE_REVISION: u64 = 1;
pub const ENHANCED_MAX_DIRTY_REGIONS: usize = 64;
pub const ENHANCED_MAX_MINIMAP_MARKS: usize = 128;
pub const ENHANCED_MAX_AUDIO_REFERENCES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub enum EnhancedFamily {
    Reserved = 0,
    ImportedResetTuple = 1,
    AuthoredActivationTuple = 2,
    AuthoredTerrainFeature = 3,
    AuthoredPlayerMarker = 4,
    AuthoredSwitch = 5,
    PlayerFireball = 6,
    ArcherArrow = 7,
    FireflyFireball = 8,
    CastleBuild = 9,
    CastleRoot = 10,
    CastleConstruction = 11,
    CastleDefenseController = 12,
    CastleDefenseShot = 13,
    CastleAssociation = 14,
    Possession = 15,
    PossessionImpact = 16,
    GroundFire = 17,
    PossessionAttraction = 18,
    StrongPossessionImpact = 19,
    FireballImpact = 20,
    WaterSplash = 21,
    ManaSphere = 22,
    ManaCarrier = 23,
    DeathBurst = 24,
    PlayerDeathMarker = 25,
    SpeedTrail = 26,
    CheckpointSmoke = 27,
    SpellPickup = 28,
    Ambient = 29,
    MultipartSegment = 30,
    ResidentArcher = 31,
    ResidentBuilder = 32,
    ResidentVillager = 33,
    ResidentTrader = 34,
    Goat = 35,
}

pub const ENHANCED_FAMILY_COUNT: usize = 36;

impl EnhancedFamily {
    pub const ALL: [Self; ENHANCED_FAMILY_COUNT] = [
        Self::Reserved,
        Self::ImportedResetTuple,
        Self::AuthoredActivationTuple,
        Self::AuthoredTerrainFeature,
        Self::AuthoredPlayerMarker,
        Self::AuthoredSwitch,
        Self::PlayerFireball,
        Self::ArcherArrow,
        Self::FireflyFireball,
        Self::CastleBuild,
        Self::CastleRoot,
        Self::CastleConstruction,
        Self::CastleDefenseController,
        Self::CastleDefenseShot,
        Self::CastleAssociation,
        Self::Possession,
        Self::PossessionImpact,
        Self::GroundFire,
        Self::PossessionAttraction,
        Self::StrongPossessionImpact,
        Self::FireballImpact,
        Self::WaterSplash,
        Self::ManaSphere,
        Self::ManaCarrier,
        Self::DeathBurst,
        Self::PlayerDeathMarker,
        Self::SpeedTrail,
        Self::CheckpointSmoke,
        Self::SpellPickup,
        Self::Ambient,
        Self::MultipartSegment,
        Self::ResidentArcher,
        Self::ResidentBuilder,
        Self::ResidentVillager,
        Self::ResidentTrader,
        Self::Goat,
    ];

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn is_presentable(self) -> bool {
        !matches!(
            self,
            Self::Reserved
                | Self::CastleConstruction
                | Self::PossessionImpact
                | Self::PossessionAttraction
                | Self::StrongPossessionImpact
                | Self::SpeedTrail
        )
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "Reserved",
            Self::ImportedResetTuple => "ImportedResetTuple",
            Self::AuthoredActivationTuple => "AuthoredActivationTuple",
            Self::AuthoredTerrainFeature => "AuthoredTerrainFeature",
            Self::AuthoredPlayerMarker => "AuthoredPlayerMarker",
            Self::AuthoredSwitch => "AuthoredSwitch",
            Self::PlayerFireball => "PlayerFireball",
            Self::ArcherArrow => "ArcherArrow",
            Self::FireflyFireball => "FireflyFireball",
            Self::CastleBuild => "CastleBuild",
            Self::CastleRoot => "CastleRoot",
            Self::CastleConstruction => "CastleConstruction",
            Self::CastleDefenseController => "CastleDefenseController",
            Self::CastleDefenseShot => "CastleDefenseShot",
            Self::CastleAssociation => "CastleAssociation",
            Self::Possession => "Possession",
            Self::PossessionImpact => "PossessionImpact",
            Self::GroundFire => "GroundFire",
            Self::PossessionAttraction => "PossessionAttraction",
            Self::StrongPossessionImpact => "StrongPossessionImpact",
            Self::FireballImpact => "FireballImpact",
            Self::WaterSplash => "WaterSplash",
            Self::ManaSphere => "ManaSphere",
            Self::ManaCarrier => "ManaCarrier",
            Self::DeathBurst => "DeathBurst",
            Self::PlayerDeathMarker => "PlayerDeathMarker",
            Self::SpeedTrail => "SpeedTrail",
            Self::CheckpointSmoke => "CheckpointSmoke",
            Self::SpellPickup => "SpellPickup",
            Self::Ambient => "Ambient",
            Self::MultipartSegment => "MultipartSegment",
            Self::ResidentArcher => "ResidentArcher",
            Self::ResidentBuilder => "ResidentBuilder",
            Self::ResidentVillager => "ResidentVillager",
            Self::ResidentTrader => "ResidentTrader",
            Self::Goat => "Goat",
        }
    }

    pub fn try_from_code(code: u8) -> Result<Self, EnhancedPresentationError> {
        Self::ALL.get(usize::from(code)).copied().ok_or_else(|| {
            EnhancedPresentationError(format!("unknown Enhanced family code {code}"))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EnhancedEventKind {
    Spawn = 1,
    Update = 2,
    Despawn = 3,
    Reset = 4,
    Spell = 5,
    Impact = 6,
    Light = 7,
    Sound = 8,
    Weather = 9,
    Objective = 10,
    Tombstone = 11,
}

impl EnhancedEventKind {
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub fn try_from_code(code: u8) -> Result<Self, EnhancedPresentationError> {
        match code {
            1 => Ok(Self::Spawn),
            2 => Ok(Self::Update),
            3 => Ok(Self::Despawn),
            4 => Ok(Self::Reset),
            5 => Ok(Self::Spell),
            6 => Ok(Self::Impact),
            7 => Ok(Self::Light),
            8 => Ok(Self::Sound),
            9 => Ok(Self::Weather),
            10 => Ok(Self::Objective),
            11 => Ok(Self::Tombstone),
            _ => Err(EnhancedPresentationError(format!(
                "unknown Enhanced event kind code {code}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EnhancedAuthority {
    Rust = 0,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedTransform {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedProfileIdentity {
    pub mode_id: u8,
    pub asset_tier: u8,
    pub quality_flags: u16,
    pub profile_revision: u64,
    pub internal_width: u16,
    pub internal_height: u16,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub viewport_width: u16,
    pub viewport_height: u16,
}

impl EnhancedProfileIdentity {
    #[must_use]
    pub const fn unity_default() -> Self {
        Self {
            mode_id: ENHANCED_PROFILE_UNITY,
            asset_tier: 1,
            quality_flags: 0,
            profile_revision: ENHANCED_PROFILE_REVISION,
            internal_width: 1920,
            internal_height: 1080,
            viewport_x: 0,
            viewport_y: 0,
            viewport_width: 1920,
            viewport_height: 1080,
        }
    }

    pub fn validate(&self) -> Result<(), EnhancedPresentationError> {
        if self.mode_id != ENHANCED_PROFILE_UNITY {
            return Err(EnhancedPresentationError(format!(
                "unknown Enhanced presentation profile {}",
                self.mode_id
            )));
        }
        if self.profile_revision == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced profile revision zero is invalid".to_owned(),
            ));
        }
        if self.internal_width == 0
            || self.internal_height == 0
            || self.viewport_width == 0
            || self.viewport_height == 0
        {
            return Err(EnhancedPresentationError(
                "Enhanced profile dimensions must be non-zero".to_owned(),
            ));
        }
        let viewport_right = u32::from(self.viewport_x) + u32::from(self.viewport_width);
        let viewport_bottom = u32::from(self.viewport_y) + u32::from(self.viewport_height);
        if viewport_right > u32::from(self.internal_width)
            || viewport_bottom > u32::from(self.internal_height)
        {
            return Err(EnhancedPresentationError(
                "Enhanced profile viewport exceeds internal dimensions".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedDirtyRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub kind: u8,
    pub flags: u8,
    pub reserved: u16,
    pub source_revision: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedCameraPose {
    pub transform: EnhancedTransform,
    pub fov: i16,
    pub eye_height: i16,
    pub slope_target_x: i16,
    pub slope_target_y: i16,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedHudState {
    pub life_percent: u8,
    pub mana_percent: u8,
    pub selected_spell: u8,
    pub selected_tier: u8,
    pub left_spell: u8,
    pub right_spell: u8,
    pub objective_selector: u8,
    pub objective_textbox_ticks: u8,
    pub flags: u32,
    pub cursor_x: i16,
    pub cursor_y: i16,
    pub menu_state: u8,
    pub reserved: [u8; 3],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedMinimapMark {
    pub stable_id: u64,
    pub x: i16,
    pub y: i16,
    pub colour: u8,
    pub selector: u8,
    pub flags: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnhancedAudioReference {
    pub event_id: u64,
    pub sound_id: u16,
    pub entity_index: u16,
    pub volume_q15: u16,
    pub pan_q16: u16,
    pub playback_rate_hz: u32,
    pub position: EnhancedTransform,
    pub play_type: u8,
    pub loop_count: i8,
    pub flags: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedFrameState {
    pub generation: u64,
    pub tick: u64,
    pub profile: EnhancedProfileIdentity,
    pub terrain_revision: u64,
    pub terrain_hash: u64,
    pub dirty_regions: Box<[EnhancedDirtyRegion]>,
    pub camera: EnhancedCameraPose,
    pub player: EnhancedTransform,
    pub hud: EnhancedHudState,
    pub minimap_marks: Box<[EnhancedMinimapMark]>,
    pub audio_references: Box<[EnhancedAudioReference]>,
}

impl EnhancedFrameState {
    #[must_use]
    pub fn for_frame(generation: u64, tick: u64) -> Self {
        Self {
            generation,
            tick,
            profile: EnhancedProfileIdentity::unity_default(),
            terrain_revision: 0,
            terrain_hash: 0,
            dirty_regions: Box::new([]),
            camera: EnhancedCameraPose::default(),
            player: EnhancedTransform::default(),
            hud: EnhancedHudState::default(),
            minimap_marks: Box::new([]),
            audio_references: Box::new([]),
        }
    }

    pub fn validate(&self) -> Result<(), EnhancedPresentationError> {
        self.profile.validate()?;
        if self.dirty_regions.len() > ENHANCED_MAX_DIRTY_REGIONS {
            return Err(EnhancedPresentationError(format!(
                "Enhanced frame has {} dirty regions; maximum is {ENHANCED_MAX_DIRTY_REGIONS}",
                self.dirty_regions.len()
            )));
        }
        if self.minimap_marks.len() > ENHANCED_MAX_MINIMAP_MARKS {
            return Err(EnhancedPresentationError(format!(
                "Enhanced frame has {} minimap marks; maximum is {ENHANCED_MAX_MINIMAP_MARKS}",
                self.minimap_marks.len()
            )));
        }
        if self.audio_references.len() > ENHANCED_MAX_AUDIO_REFERENCES {
            return Err(EnhancedPresentationError(format!(
                "Enhanced frame has {} audio references; maximum is {ENHANCED_MAX_AUDIO_REFERENCES}",
                self.audio_references.len()
            )));
        }
        if self.hud.life_percent > 100 || self.hud.mana_percent > 100 {
            return Err(EnhancedPresentationError(
                "Enhanced HUD percentages must be <= 100".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnhancedEntityRecord {
    pub generation: u64,
    pub stable_id: u64,
    pub source_entity_index: u16,
    pub family: EnhancedFamily,
    pub authority: EnhancedAuthority,
    pub flags: u32,
    pub transform: EnhancedTransform,
    pub selector: i16,
    pub animation_clock: u32,
    pub owner_colour: u8,
    pub life_percent: u8,
    pub presentation_flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnhancedEventRecord {
    pub generation: u64,
    pub tick: u64,
    pub event_id: u64,
    pub kind: EnhancedEventKind,
    pub family: EnhancedFamily,
    pub source_entity_index: u16,
    pub transform: EnhancedTransform,
    pub value_0: i32,
    pub value_1: i32,
    pub flags: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedSnapshot {
    pub generation: u64,
    pub tick: u64,
    pub reset_epoch: u64,
    pub frame: EnhancedFrameState,
    pub entities: Box<[EnhancedEntityRecord]>,
    pub events: Box<[EnhancedEventRecord]>,
    pub state_hash: u64,
    pub event_hash: u64,
    pub receipt_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedSnapshotPage {
    pub generation: u64,
    pub tick: u64,
    pub reset_epoch: u64,
    pub frame: EnhancedFrameState,
    pub entity_cursor: u32,
    pub event_cursor: u32,
    pub next_entity_cursor: u32,
    pub next_event_cursor: u32,
    pub entities: Box<[EnhancedEntityRecord]>,
    pub events: Box<[EnhancedEventRecord]>,
    pub state_hash: u64,
    pub event_hash: u64,
    pub receipt_hash: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedPresentationError(pub String);

impl fmt::Display for EnhancedPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EnhancedPresentationError {}

#[derive(Clone, Debug)]
pub struct EnhancedPresentationLedger {
    generation: u64,
    tick: Option<u64>,
    reset_epoch: u64,
    frame_state: Option<EnhancedFrameState>,
    entities: BTreeMap<u64, EnhancedEntityRecord>,
    tombstones: BTreeSet<u64>,
    event_backlog: VecDeque<EnhancedEventRecord>,
    current_events: Vec<EnhancedEventRecord>,
    max_event_backlog: usize,
    staged_entities: Option<BTreeMap<u64, EnhancedEntityRecord>>,
    staged_tombstones: Option<BTreeSet<u64>>,
    staged_frame_state: Option<EnhancedFrameState>,
    staged_events: Vec<EnhancedEventRecord>,
    touched_entity_ids: BTreeSet<u64>,
    touched_event_ids: BTreeSet<u64>,
}

impl EnhancedPresentationLedger {
    pub fn new(
        generation: u64,
        max_event_backlog: usize,
    ) -> Result<Self, EnhancedPresentationError> {
        if generation == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced presentation generation must be non-zero".to_owned(),
            ));
        }
        if max_event_backlog == 0 || max_event_backlog > ENHANCED_MAX_EVENT_BACKLOG {
            return Err(EnhancedPresentationError(format!(
                "Enhanced event backlog must be between 1 and {ENHANCED_MAX_EVENT_BACKLOG}"
            )));
        }
        Ok(Self {
            generation,
            tick: None,
            reset_epoch: 0,
            frame_state: None,
            entities: BTreeMap::new(),
            tombstones: BTreeSet::new(),
            event_backlog: VecDeque::new(),
            current_events: Vec::new(),
            max_event_backlog,
            staged_entities: None,
            staged_tombstones: None,
            staged_frame_state: None,
            staged_events: Vec::new(),
            touched_entity_ids: BTreeSet::new(),
            touched_event_ids: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn tick(&self) -> Option<u64> {
        self.tick
    }

    #[must_use]
    pub const fn reset_epoch(&self) -> u64 {
        self.reset_epoch
    }

    #[must_use]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    #[must_use]
    pub fn event_backlog_len(&self) -> usize {
        self.event_backlog.len()
    }

    #[must_use]
    pub fn is_frame_open(&self) -> bool {
        self.staged_entities.is_some()
    }

    pub fn begin_frame(
        &mut self,
        generation: u64,
        tick: u64,
    ) -> Result<(), EnhancedPresentationError> {
        if self.is_frame_open() {
            return Err(EnhancedPresentationError(
                "Enhanced presentation frame is already open".to_owned(),
            ));
        }
        self.validate_generation(generation)?;
        let expected_tick = self
            .tick
            .map_or(0, |value| value.checked_add(1).unwrap_or(u64::MAX));
        if tick != expected_tick {
            return Err(EnhancedPresentationError(format!(
                "Enhanced tick {tick} is out of order; expected {expected_tick}"
            )));
        }
        self.staged_entities = Some(self.entities.clone());
        self.staged_tombstones = Some(self.tombstones.clone());
        let mut frame_state = self
            .frame_state
            .clone()
            .unwrap_or_else(|| EnhancedFrameState::for_frame(generation, tick));
        frame_state.generation = generation;
        frame_state.tick = tick;
        frame_state.dirty_regions = Box::new([]);
        frame_state.audio_references = Box::new([]);
        self.staged_frame_state = Some(frame_state);
        self.staged_events.clear();
        self.touched_entity_ids.clear();
        self.touched_event_ids.clear();
        Ok(())
    }

    pub fn set_frame_state(
        &mut self,
        frame_state: EnhancedFrameState,
    ) -> Result<(), EnhancedPresentationError> {
        self.validate_open_frame(frame_state.generation)?;
        let expected_tick = self.staged_tick()?;
        if frame_state.tick != expected_tick {
            return Err(EnhancedPresentationError(format!(
                "Enhanced frame-state tick {} does not match open frame {expected_tick}",
                frame_state.tick
            )));
        }
        frame_state.validate()?;
        self.staged_frame_state = Some(frame_state);
        Ok(())
    }

    pub fn upsert_entity(
        &mut self,
        entity: EnhancedEntityRecord,
    ) -> Result<(), EnhancedPresentationError> {
        self.validate_open_frame(entity.generation)?;
        if entity.stable_id == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced stable identity zero is invalid".to_owned(),
            ));
        }
        if entity.source_entity_index == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced source slot zero is reserved".to_owned(),
            ));
        }
        if !entity.family.is_presentable() {
            return Err(EnhancedPresentationError(format!(
                "Enhanced family {} is an explicit source exclusion, not an entity",
                entity.family.as_str()
            )));
        }
        if entity.authority != EnhancedAuthority::Rust {
            return Err(EnhancedPresentationError(
                "Enhanced entity authority must remain Rust".to_owned(),
            ));
        }
        if entity.life_percent > 100 {
            return Err(EnhancedPresentationError(
                "Enhanced entity life percentage must be <= 100".to_owned(),
            ));
        }
        let tombstones = self
            .staged_tombstones
            .as_ref()
            .expect("validate_open_frame proves staged tombstones exist");
        if tombstones.contains(&entity.stable_id) {
            return Err(EnhancedPresentationError(format!(
                "Enhanced stable identity {} is tombstoned and cannot be reused",
                entity.stable_id
            )));
        }
        if self.touched_entity_ids.contains(&entity.stable_id) {
            return Err(EnhancedPresentationError(format!(
                "Enhanced stable identity {} was submitted more than once in one frame",
                entity.stable_id
            )));
        }
        let staged = self
            .staged_entities
            .as_ref()
            .expect("validate_open_frame proves staged entities exist");
        if staged.iter().any(|(stable_id, existing)| {
            *stable_id != entity.stable_id
                && existing.source_entity_index == entity.source_entity_index
        }) {
            return Err(EnhancedPresentationError(format!(
                "Enhanced source slot {} has duplicate stable identities",
                entity.source_entity_index
            )));
        }
        self.touched_entity_ids.insert(entity.stable_id);
        self.staged_entities
            .as_mut()
            .expect("staged entities remain open")
            .insert(entity.stable_id, entity);
        Ok(())
    }

    pub fn despawn(
        &mut self,
        generation: u64,
        stable_id: u64,
    ) -> Result<(), EnhancedPresentationError> {
        self.validate_open_frame(generation)?;
        if stable_id == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced despawn identity zero is invalid".to_owned(),
            ));
        }
        if self.touched_entity_ids.contains(&stable_id) {
            return Err(EnhancedPresentationError(format!(
                "Enhanced stable identity {stable_id} was submitted more than once in one frame"
            )));
        }
        let staged = self
            .staged_entities
            .as_mut()
            .expect("validate_open_frame proves staged entities exist");
        if staged.remove(&stable_id).is_none() {
            return Err(EnhancedPresentationError(format!(
                "Enhanced despawn identity {stable_id} is not active"
            )));
        }
        self.touched_entity_ids.insert(stable_id);
        self.staged_tombstones
            .as_mut()
            .expect("staged tombstones remain open")
            .insert(stable_id);
        Ok(())
    }

    pub fn push_event(
        &mut self,
        event: EnhancedEventRecord,
    ) -> Result<(), EnhancedPresentationError> {
        self.validate_open_frame(event.generation)?;
        // Events belong to the frame currently being staged.  Using the last
        // committed tick here would reject every event after frame zero.
        let tick = self.staged_tick()?;
        if event.tick != tick {
            return Err(EnhancedPresentationError(format!(
                "Enhanced event tick {} does not match open frame {tick}",
                event.tick
            )));
        }
        if event.event_id == 0 {
            return Err(EnhancedPresentationError(
                "Enhanced event identity zero is invalid".to_owned(),
            ));
        }
        EnhancedEventKind::try_from_code(event.kind.code())?;
        EnhancedFamily::try_from_code(event.family.code())?;
        if self.touched_event_ids.contains(&event.event_id)
            || self
                .event_backlog
                .iter()
                .any(|existing| existing.event_id == event.event_id)
        {
            return Err(EnhancedPresentationError(format!(
                "Enhanced event identity {} is duplicated",
                event.event_id
            )));
        }
        if self.event_backlog.len() + self.staged_events.len() >= self.max_event_backlog {
            return Err(EnhancedPresentationError(
                "Enhanced event backlog is full".to_owned(),
            ));
        }
        self.touched_event_ids.insert(event.event_id);
        self.staged_events.push(event);
        Ok(())
    }

    /// Discard the open frame without changing the last committed snapshot.
    pub fn abort_frame(&mut self) -> Result<(), EnhancedPresentationError> {
        if !self.is_frame_open() {
            return Err(EnhancedPresentationError(
                "Enhanced presentation frame is not open".to_owned(),
            ));
        }
        self.staged_entities = None;
        self.staged_tombstones = None;
        self.staged_frame_state = None;
        self.staged_events.clear();
        self.touched_entity_ids.clear();
        self.touched_event_ids.clear();
        Ok(())
    }

    pub fn commit(&mut self) -> Result<EnhancedSnapshot, EnhancedPresentationError> {
        let Some(staged_entities) = self.staged_entities.take() else {
            return Err(EnhancedPresentationError(
                "Enhanced presentation frame is not open".to_owned(),
            ));
        };
        let staged_tombstones = self
            .staged_tombstones
            .take()
            .expect("staged tombstones exist when entities are staged");
        let staged_frame_state = self
            .staged_frame_state
            .take()
            .expect("staged frame state exists when entities are staged");
        let tick = self.staged_tick()?;
        self.entities = staged_entities;
        self.tombstones = staged_tombstones;
        self.frame_state = Some(staged_frame_state);
        self.tick = Some(tick);
        self.current_events = std::mem::take(&mut self.staged_events);
        for event in &self.current_events {
            self.event_backlog.push_back(*event);
        }
        while self.event_backlog.len() > self.max_event_backlog {
            self.event_backlog.pop_front();
        }
        self.touched_entity_ids.clear();
        self.touched_event_ids.clear();
        self.snapshot()
    }

    pub fn reset(&mut self, generation: u64) -> Result<(), EnhancedPresentationError> {
        if self.is_frame_open() {
            return Err(EnhancedPresentationError(
                "Enhanced reset cannot interrupt an open frame".to_owned(),
            ));
        }
        if generation <= self.generation {
            return Err(EnhancedPresentationError(format!(
                "Enhanced reset generation {generation} must be newer than {}",
                self.generation
            )));
        }
        self.generation = generation;
        self.tick = None;
        self.reset_epoch = self.reset_epoch.wrapping_add(1);
        self.entities.clear();
        self.tombstones.clear();
        self.event_backlog.clear();
        self.current_events.clear();
        self.frame_state = None;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<EnhancedSnapshot, EnhancedPresentationError> {
        if self.is_frame_open() {
            return Err(EnhancedPresentationError(
                "Enhanced snapshot cannot observe an uncommitted frame".to_owned(),
            ));
        }
        let tick = self.tick.ok_or_else(|| {
            EnhancedPresentationError(
                "Enhanced snapshot is unavailable before the first frame".to_owned(),
            )
        })?;
        let entities = self.ordered_entities();
        let events = self.current_events.clone().into_boxed_slice();
        let frame = self.frame_state.clone().ok_or_else(|| {
            EnhancedPresentationError("Enhanced frame state is unavailable".to_owned())
        })?;
        let state_hash = hash_entities(self.generation, tick, self.reset_epoch, &frame, &entities);
        let event_hash = hash_events(&events);
        let receipt_hash = hash_receipt(state_hash, event_hash, entities.len(), events.len());
        Ok(EnhancedSnapshot {
            generation: self.generation,
            tick,
            reset_epoch: self.reset_epoch,
            frame,
            entities,
            events,
            state_hash,
            event_hash,
            receipt_hash,
        })
    }

    pub fn snapshot_page(
        &self,
        generation: u64,
        tick: u64,
        entity_cursor: u32,
        event_cursor: u32,
        max_entities: u32,
        max_events: u32,
    ) -> Result<EnhancedSnapshotPage, EnhancedPresentationError> {
        self.validate_generation(generation)?;
        if self.tick != Some(tick) {
            return Err(EnhancedPresentationError(format!(
                "Enhanced snapshot tick {tick} is stale; current tick is {:?}",
                self.tick
            )));
        }
        let snapshot = self.snapshot()?;
        let entity_cursor = validate_cursor(entity_cursor, snapshot.entities.len(), "entity")?;
        let event_cursor = validate_cursor(event_cursor, snapshot.events.len(), "event")?;
        let entity_end = checked_page_end(
            entity_cursor,
            max_entities,
            snapshot.entities.len(),
            "entity",
        )?;
        let event_end = checked_page_end(event_cursor, max_events, snapshot.events.len(), "event")?;
        let next_entity_cursor = u32::try_from(entity_end).expect("entity page cursor fits u32");
        let next_event_cursor = u32::try_from(event_end).expect("event page cursor fits u32");
        Ok(EnhancedSnapshotPage {
            generation: snapshot.generation,
            tick: snapshot.tick,
            reset_epoch: snapshot.reset_epoch,
            frame: snapshot.frame.clone(),
            entity_cursor: u32::try_from(entity_cursor).expect("entity cursor fits u32"),
            event_cursor: u32::try_from(event_cursor).expect("event cursor fits u32"),
            next_entity_cursor,
            next_event_cursor,
            entities: snapshot.entities[entity_cursor..entity_end]
                .to_vec()
                .into_boxed_slice(),
            events: snapshot.events[event_cursor..event_end]
                .to_vec()
                .into_boxed_slice(),
            state_hash: snapshot.state_hash,
            event_hash: snapshot.event_hash,
            receipt_hash: snapshot.receipt_hash,
        })
    }

    fn validate_generation(&self, generation: u64) -> Result<(), EnhancedPresentationError> {
        if generation != self.generation {
            return Err(EnhancedPresentationError(format!(
                "Enhanced generation {generation} is stale; current generation is {}",
                self.generation
            )));
        }
        Ok(())
    }

    fn validate_open_frame(&self, generation: u64) -> Result<(), EnhancedPresentationError> {
        self.validate_generation(generation)?;
        if !self.is_frame_open() {
            return Err(EnhancedPresentationError(
                "Enhanced operation requires an open frame".to_owned(),
            ));
        }
        Ok(())
    }

    fn staged_tick(&self) -> Result<u64, EnhancedPresentationError> {
        self.tick.map_or(Ok(0), |value| {
            value
                .checked_add(1)
                .ok_or_else(|| EnhancedPresentationError("Enhanced tick overflow".to_owned()))
        })
    }

    fn ordered_entities(&self) -> Box<[EnhancedEntityRecord]> {
        let mut entities: Vec<_> = self.entities.values().copied().collect();
        entities.sort_unstable_by_key(|entity| (entity.source_entity_index, entity.stable_id));
        entities.into_boxed_slice()
    }
}

fn validate_cursor(
    cursor: u32,
    length: usize,
    kind: &str,
) -> Result<usize, EnhancedPresentationError> {
    let cursor = usize::try_from(cursor).expect("u32 cursor fits usize");
    if cursor > length {
        return Err(EnhancedPresentationError(format!(
            "Enhanced {kind} cursor {cursor} exceeds page length {length}"
        )));
    }
    Ok(cursor)
}

fn checked_page_end(
    cursor: usize,
    count: u32,
    length: usize,
    kind: &str,
) -> Result<usize, EnhancedPresentationError> {
    let count = usize::try_from(count).expect("u32 page count fits usize");
    cursor.checked_add(count).map_or_else(
        || {
            Err(EnhancedPresentationError(format!(
                "Enhanced {kind} page cursor overflow"
            )))
        },
        |end| Ok(end.min(length)),
    )
}

fn hash_entities(
    generation: u64,
    tick: u64,
    reset_epoch: u64,
    frame: &EnhancedFrameState,
    entities: &[EnhancedEntityRecord],
) -> u64 {
    let mut bytes = Vec::with_capacity(32 + entities.len() * 64);
    bytes.extend_from_slice(&generation.to_le_bytes());
    bytes.extend_from_slice(&tick.to_le_bytes());
    bytes.extend_from_slice(&reset_epoch.to_le_bytes());
    bytes.push(frame.profile.mode_id);
    bytes.push(frame.profile.asset_tier);
    bytes.extend_from_slice(&frame.profile.quality_flags.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.profile_revision.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.internal_width.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.internal_height.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.viewport_x.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.viewport_y.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.viewport_width.to_le_bytes());
    bytes.extend_from_slice(&frame.profile.viewport_height.to_le_bytes());
    bytes.extend_from_slice(&frame.terrain_revision.to_le_bytes());
    bytes.extend_from_slice(&frame.terrain_hash.to_le_bytes());
    for region in &frame.dirty_regions {
        bytes.extend_from_slice(&region.x.to_le_bytes());
        bytes.extend_from_slice(&region.y.to_le_bytes());
        bytes.extend_from_slice(&region.width.to_le_bytes());
        bytes.extend_from_slice(&region.height.to_le_bytes());
        bytes.push(region.kind);
        bytes.push(region.flags);
        bytes.extend_from_slice(&region.reserved.to_le_bytes());
        bytes.extend_from_slice(&region.source_revision.to_le_bytes());
    }
    for value in [
        frame.camera.transform.x,
        frame.camera.transform.y,
        frame.camera.transform.z,
        i32::from(frame.camera.transform.yaw),
        i32::from(frame.camera.transform.pitch),
        i32::from(frame.camera.transform.roll),
        i32::from(frame.camera.fov),
        i32::from(frame.camera.eye_height),
        i32::from(frame.camera.slope_target_x),
        i32::from(frame.camera.slope_target_y),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&frame.camera.flags.to_le_bytes());
    for value in [
        frame.player.x,
        frame.player.y,
        frame.player.z,
        i32::from(frame.player.yaw),
        i32::from(frame.player.pitch),
        i32::from(frame.player.roll),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.push(frame.hud.life_percent);
    bytes.push(frame.hud.mana_percent);
    bytes.push(frame.hud.selected_spell);
    bytes.push(frame.hud.selected_tier);
    bytes.push(frame.hud.left_spell);
    bytes.push(frame.hud.right_spell);
    bytes.push(frame.hud.objective_selector);
    bytes.push(frame.hud.objective_textbox_ticks);
    bytes.extend_from_slice(&frame.hud.flags.to_le_bytes());
    bytes.extend_from_slice(&frame.hud.cursor_x.to_le_bytes());
    bytes.extend_from_slice(&frame.hud.cursor_y.to_le_bytes());
    bytes.push(frame.hud.menu_state);
    for mark in &frame.minimap_marks {
        bytes.extend_from_slice(&mark.stable_id.to_le_bytes());
        bytes.extend_from_slice(&mark.x.to_le_bytes());
        bytes.extend_from_slice(&mark.y.to_le_bytes());
        bytes.push(mark.colour);
        bytes.push(mark.selector);
        bytes.extend_from_slice(&mark.flags.to_le_bytes());
    }
    for audio in &frame.audio_references {
        bytes.extend_from_slice(&audio.event_id.to_le_bytes());
        bytes.extend_from_slice(&audio.sound_id.to_le_bytes());
        bytes.extend_from_slice(&audio.entity_index.to_le_bytes());
        bytes.extend_from_slice(&audio.volume_q15.to_le_bytes());
        bytes.extend_from_slice(&audio.pan_q16.to_le_bytes());
        bytes.extend_from_slice(&audio.playback_rate_hz.to_le_bytes());
        bytes.extend_from_slice(&audio.position.x.to_le_bytes());
        bytes.extend_from_slice(&audio.position.y.to_le_bytes());
        bytes.extend_from_slice(&audio.position.z.to_le_bytes());
        bytes.extend_from_slice(&audio.position.yaw.to_le_bytes());
        bytes.extend_from_slice(&audio.position.pitch.to_le_bytes());
        bytes.extend_from_slice(&audio.position.roll.to_le_bytes());
        bytes.push(audio.play_type);
        bytes.push(audio.loop_count as u8);
        bytes.extend_from_slice(&audio.flags.to_le_bytes());
    }
    for entity in entities {
        bytes.extend_from_slice(&entity.generation.to_le_bytes());
        bytes.extend_from_slice(&entity.stable_id.to_le_bytes());
        bytes.extend_from_slice(&entity.source_entity_index.to_le_bytes());
        bytes.push(entity.family.code());
        bytes.push(entity.authority as u8);
        bytes.extend_from_slice(&entity.flags.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.x.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.y.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.z.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.yaw.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.pitch.to_le_bytes());
        bytes.extend_from_slice(&entity.transform.roll.to_le_bytes());
        bytes.extend_from_slice(&entity.selector.to_le_bytes());
        bytes.extend_from_slice(&entity.animation_clock.to_le_bytes());
        bytes.push(entity.owner_colour);
        bytes.push(entity.life_percent);
        bytes.extend_from_slice(&entity.presentation_flags.to_le_bytes());
    }
    byte_hash(&bytes)
}

fn hash_events(events: &[EnhancedEventRecord]) -> u64 {
    let mut bytes = Vec::with_capacity(32 + events.len() * 64);
    for event in events {
        bytes.extend_from_slice(&event.generation.to_le_bytes());
        bytes.extend_from_slice(&event.tick.to_le_bytes());
        bytes.extend_from_slice(&event.event_id.to_le_bytes());
        bytes.push(event.kind.code());
        bytes.push(event.family.code());
        bytes.extend_from_slice(&event.source_entity_index.to_le_bytes());
        bytes.extend_from_slice(&event.transform.x.to_le_bytes());
        bytes.extend_from_slice(&event.transform.y.to_le_bytes());
        bytes.extend_from_slice(&event.transform.z.to_le_bytes());
        bytes.extend_from_slice(&event.transform.yaw.to_le_bytes());
        bytes.extend_from_slice(&event.transform.pitch.to_le_bytes());
        bytes.extend_from_slice(&event.transform.roll.to_le_bytes());
        bytes.extend_from_slice(&event.value_0.to_le_bytes());
        bytes.extend_from_slice(&event.value_1.to_le_bytes());
        bytes.extend_from_slice(&event.flags.to_le_bytes());
    }
    byte_hash(&bytes)
}

fn hash_receipt(state_hash: u64, event_hash: u64, entity_count: usize, event_count: usize) -> u64 {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(&state_hash.to_le_bytes());
    bytes.extend_from_slice(&event_hash.to_le_bytes());
    bytes.extend_from_slice(&(entity_count as u64).to_le_bytes());
    bytes.extend_from_slice(&(event_count as u64).to_le_bytes());
    byte_hash(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(generation: u64, stable_id: u64, source_entity_index: u16) -> EnhancedEntityRecord {
        EnhancedEntityRecord {
            generation,
            stable_id,
            source_entity_index,
            family: EnhancedFamily::Goat,
            authority: EnhancedAuthority::Rust,
            flags: ENHANCED_ENTITY_ACTIVE | ENHANCED_ENTITY_VISIBLE,
            transform: EnhancedTransform {
                x: i32::from(source_entity_index),
                ..EnhancedTransform::default()
            },
            selector: 152,
            animation_clock: 4,
            owner_colour: 0,
            life_percent: 100,
            presentation_flags: 0,
        }
    }

    fn event(generation: u64, tick: u64, event_id: u64) -> EnhancedEventRecord {
        EnhancedEventRecord {
            generation,
            tick,
            event_id,
            kind: EnhancedEventKind::Spell,
            family: EnhancedFamily::Goat,
            source_entity_index: 7,
            transform: EnhancedTransform::default(),
            value_0: 1,
            value_1: 2,
            flags: ENHANCED_EVENT_PRESENTATION_ONLY,
        }
    }

    #[test]
    fn source_ordered_snapshot_hashes_are_deterministic_across_submission_order() {
        let mut first = EnhancedPresentationLedger::new(4, 8).expect("ledger");
        let mut second = EnhancedPresentationLedger::new(4, 8).expect("ledger");
        first.begin_frame(4, 0).expect("frame");
        second.begin_frame(4, 0).expect("frame");
        first.upsert_entity(entity(4, 11, 20)).expect("entity");
        first.upsert_entity(entity(4, 12, 10)).expect("entity");
        first.push_event(event(4, 0, 101)).expect("event");
        second.upsert_entity(entity(4, 12, 10)).expect("entity");
        second.upsert_entity(entity(4, 11, 20)).expect("entity");
        second.push_event(event(4, 0, 101)).expect("event");
        let a = first.commit().expect("commit");
        let b = second.commit().expect("commit");
        assert_eq!(a.entities, b.entities);
        assert_eq!(a.state_hash, b.state_hash);
        assert_eq!(a.event_hash, b.event_hash);
        assert_eq!(a.receipt_hash, b.receipt_hash);
        assert_eq!(a.entities[0].source_entity_index, 10);
    }

    #[test]
    fn stale_generation_duplicate_identity_and_unknown_family_fail_before_commit() {
        let mut ledger = EnhancedPresentationLedger::new(1, 8).expect("ledger");
        assert!(ledger.begin_frame(2, 0).is_err());
        ledger.begin_frame(1, 0).expect("frame");
        assert!(ledger.upsert_entity(entity(2, 9, 9)).is_err());
        ledger.upsert_entity(entity(1, 9, 9)).expect("entity");
        assert!(ledger.upsert_entity(entity(1, 9, 9)).is_err());
        assert!(EnhancedFamily::try_from_code(255).is_err());
        ledger.abort_frame().expect("discard invalid frame");
        assert!(!ledger.is_frame_open());
        assert_eq!(ledger.entity_count(), 0);
    }

    #[test]
    fn despawn_tombstone_and_reset_fence_identity_reuse() {
        let mut ledger = EnhancedPresentationLedger::new(3, 8).expect("ledger");
        ledger.begin_frame(3, 0).expect("frame");
        ledger.upsert_entity(entity(3, 42, 4)).expect("entity");
        ledger.commit().expect("commit");
        ledger.begin_frame(3, 1).expect("frame");
        ledger.despawn(3, 42).expect("despawn");
        ledger.commit().expect("commit");
        ledger.begin_frame(3, 2).expect("frame");
        assert!(ledger.upsert_entity(entity(3, 42, 4)).is_err());
        ledger.abort_frame().expect("discard invalid frame");
        assert!(
            ledger.reset(4).is_ok(),
            "reset is allowed after the failed frame is explicitly discarded"
        );
        // A new generation clears tombstones, so stable identity reuse is
        // allowed only after the explicit reset fence.
        let mut reset = EnhancedPresentationLedger::new(3, 8).expect("ledger");
        reset.begin_frame(3, 0).expect("frame");
        reset.upsert_entity(entity(3, 42, 4)).expect("entity");
        reset.commit().expect("commit");
        reset.reset(4).expect("new generation reset");
        reset.begin_frame(4, 0).expect("new generation frame");
        reset
            .upsert_entity(entity(4, 42, 4))
            .expect("identity may be reused after reset");
        reset.commit().expect("commit");
    }

    #[test]
    fn snapshot_pages_validate_boundaries_and_keep_hashes_equal_to_full_snapshot() {
        let mut ledger = EnhancedPresentationLedger::new(5, 8).expect("ledger");
        ledger.begin_frame(5, 0).expect("frame");
        for index in 1_u32..=3 {
            ledger
                .upsert_entity(entity(5, u64::from(index), index as u16))
                .expect("entity");
        }
        for id in 1..=2 {
            ledger.push_event(event(5, 0, id)).expect("event");
        }
        let full = ledger.commit().expect("commit");
        let page = ledger.snapshot_page(5, 0, 1, 1, 1, 1).expect("page");
        assert_eq!(page.entities.len(), 1);
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.next_entity_cursor, 2);
        assert_eq!(page.next_event_cursor, 2);
        assert_eq!(page.state_hash, full.state_hash);
        assert_eq!(page.event_hash, full.event_hash);
        assert!(ledger.snapshot_page(5, 0, 4, 0, 1, 1).is_err());
        assert!(ledger.snapshot_page(4, 0, 0, 0, 1, 1).is_err());
        assert!(ledger.snapshot_page(5, 1, 0, 0, 1, 1).is_err());
    }

    #[test]
    fn event_backlog_and_frame_transaction_are_bounded() {
        let mut ledger = EnhancedPresentationLedger::new(6, 1).expect("ledger");
        ledger.begin_frame(6, 0).expect("frame");
        ledger.push_event(event(6, 0, 1)).expect("first event");
        assert!(ledger.push_event(event(6, 0, 2)).is_err());
        ledger.commit().expect("commit");
        assert_eq!(ledger.event_backlog_len(), 1);
        ledger.begin_frame(6, 1).expect("frame");
        assert!(ledger.push_event(event(6, 1, 1)).is_err());
        ledger.abort_frame().expect("discard duplicate event frame");
        assert!(!ledger.is_frame_open());
    }

    #[test]
    fn excluded_families_are_known_but_cannot_be_entities() {
        let mut ledger = EnhancedPresentationLedger::new(7, 8).expect("ledger");
        ledger.begin_frame(7, 0).expect("frame");
        let mut excluded = entity(7, 8, 8);
        excluded.family = EnhancedFamily::SpeedTrail;
        assert!(ledger.upsert_entity(excluded).is_err());
        ledger.abort_frame().expect("discard excluded entity frame");
        assert!(!ledger.is_frame_open());
        assert!(EnhancedFamily::try_from_code(EnhancedFamily::SpeedTrail.code()).is_ok());
    }

    #[test]
    fn frame_state_is_generation_bound_and_hashes_all_presentation_inputs() {
        let mut ledger = EnhancedPresentationLedger::new(8, 8).expect("ledger");
        ledger.begin_frame(8, 0).expect("frame");
        let mut frame = EnhancedFrameState::for_frame(9, 0);
        assert!(ledger.set_frame_state(frame.clone()).is_err());
        frame.generation = 8;
        frame.terrain_revision = 4;
        frame.terrain_hash = 0xdead_beef;
        frame.dirty_regions = vec![EnhancedDirtyRegion {
            width: 16,
            height: 16,
            ..EnhancedDirtyRegion::default()
        }]
        .into_boxed_slice();
        ledger.set_frame_state(frame).expect("frame state");
        let first = ledger.commit().expect("commit");
        assert_eq!(first.frame.terrain_revision, 4);
        assert_eq!(first.frame.dirty_regions.len(), 1);
        assert_ne!(first.state_hash, 0);

        ledger.begin_frame(8, 1).expect("next frame");
        let second = ledger.commit().expect("commit");
        assert_eq!(second.frame.profile, first.frame.profile);
        assert!(second.frame.dirty_regions.is_empty());
        assert!(second.frame.audio_references.is_empty());
        assert_ne!(second.state_hash, first.state_hash);
    }
}
