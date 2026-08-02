//! Shared campaign-level content and reset seams.
//!
//! The Level 1 mission kernel predates the multi-level owner package.  This
//! module supplies the small typed seam needed to select a level from validated
//! metadata and to retain the complete authored/reset denominator before a
//! level-specific dispatcher is implemented.  Level 1 continues to use its
//! established [`Level1MissionTemplate`] constructors; Level 2 is deliberately
//! fail-closed at dispatch boundaries.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::{
    FlightWorld,
    level1_mission::{
        AUTHORED_FEATURE_BYTES, AUTHORED_FEATURE_COUNT, AuthoredFeature, Level1MissionTemplate,
        Level1ResetSourceSlot, parse_authored_features,
    },
};

/// The pinned remc2 source used as the specification for every profile.
pub const REMC2_SPEC_COMMIT: &str = "6806c7204d17220ecc0430759f784aea8ee2ddb2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignLevel {
    Level1,
    Level2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionMap {
    Night,
    Day,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetConstructor {
    Level1FrameZero,
    Level2QualifiedPackage,
}

/// Level-specific wizard/player settings that are part of the reset contract,
/// not ad-hoc defaults in a dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionWizardSettings {
    pub player_count: u8,
    pub local_player_slot: u8,
    pub ai_player_slot: Option<u8>,
    pub map_music: i32,
    pub initial_life: i32,
    pub initial_mana: i32,
}

/// Explicit authored/index/objective mappings selected by the validated level
/// metadata.  These are intentionally data, rather than a trait hierarchy or
/// a level-specific branch spread through the 45k-line mission kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionMappings {
    pub initial_activation_group: i16,
    pub player_marker_index: u16,
    pub objective_selectors: &'static [u8],
}

/// The immutable level profile used by both the generic reset seam and the
/// exact Level 1 wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissionContentProfile {
    pub spec_commit: &'static str,
    pub level: CampaignLevel,
    pub external_level: u16,
    pub internal_level: u16,
    pub map: MissionMap,
    pub map_type_id: u8,
    pub authored_record_count: usize,
    pub authored_record_bytes: usize,
    pub reset_constructor: ResetConstructor,
    pub wizard: MissionWizardSettings,
    pub mappings: MissionMappings,
}

const LEVEL1_OBJECTIVE_SELECTORS: &[u8] = &[48, 49, 50, 51, 52, 53];
const LEVEL2_OBJECTIVE_SELECTORS: &[u8] = &[48, 49, 50, 51, 52, 53];

impl MissionContentProfile {
    pub const LEVEL1: Self = Self {
        spec_commit: REMC2_SPEC_COMMIT,
        level: CampaignLevel::Level1,
        external_level: 1,
        internal_level: 0,
        map: MissionMap::Night,
        map_type_id: 1,
        authored_record_count: AUTHORED_FEATURE_COUNT,
        authored_record_bytes: AUTHORED_FEATURE_BYTES,
        reset_constructor: ResetConstructor::Level1FrameZero,
        wizard: MissionWizardSettings {
            player_count: 1,
            local_player_slot: 0,
            ai_player_slot: None,
            map_music: 1,
            initial_life: 1000,
            initial_mana: 1000,
        },
        mappings: MissionMappings {
            initial_activation_group: 0,
            player_marker_index: 1,
            objective_selectors: LEVEL1_OBJECTIVE_SELECTORS,
        },
    };

    pub const LEVEL2: Self = Self {
        spec_commit: REMC2_SPEC_COMMIT,
        level: CampaignLevel::Level2,
        external_level: 2,
        internal_level: 1,
        map: MissionMap::Day,
        map_type_id: 0,
        authored_record_count: AUTHORED_FEATURE_COUNT,
        authored_record_bytes: AUTHORED_FEATURE_BYTES,
        reset_constructor: ResetConstructor::Level2QualifiedPackage,
        wizard: MissionWizardSettings {
            player_count: 2,
            local_player_slot: 0,
            ai_player_slot: Some(1),
            map_music: 2,
            initial_life: 1000,
            initial_mana: 1000,
        },
        mappings: MissionMappings {
            initial_activation_group: 0,
            player_marker_index: 1,
            objective_selectors: LEVEL2_OBJECTIVE_SELECTORS,
        },
    };

    /// Select a profile only after the owner-package metadata has been
    /// validated.  Unknown combinations fail closed rather than falling back
    /// to Level 1.
    pub fn from_metadata(
        external_level: u16,
        internal_level: u16,
        map_type_id: u8,
    ) -> Result<Self, MissionContentError> {
        let profile = match (external_level, internal_level, map_type_id) {
            (1, 0, 1) => Self::LEVEL1,
            (2, 1, 0) => Self::LEVEL2,
            _ => {
                return Err(MissionContentError::InvalidProfile(format!(
                    "unsupported campaign metadata external={external_level} internal={internal_level} map_type_id={map_type_id}"
                )));
            }
        };
        Ok(profile)
    }

    #[must_use]
    pub const fn is_level2(self) -> bool {
        matches!(self.level, CampaignLevel::Level2)
    }

    /// Exact Level 1 compatibility wrapper.  This delegates to the existing
    /// constructor without changing its decode expectations or state shape.
    pub fn decode_level1<W: FlightWorld>(
        self,
        authored: &[u8],
        world: &W,
        global_mana_denominator: i32,
        initial_global_rng: u32,
        initial_source_slots: &[Level1ResetSourceSlot],
    ) -> Result<Level1MissionTemplate, MissionContentError> {
        if self != Self::LEVEL1 {
            return Err(MissionContentError::InvalidProfile(
                "Level 1 decoder selected for a non-Level 1 profile".to_owned(),
            ));
        }
        Level1MissionTemplate::decode_with_source_slots(
            authored,
            world,
            global_mana_denominator,
            initial_global_rng,
            initial_source_slots,
        )
        .map_err(MissionContentError::Decode)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MissionContentError {
    InvalidProfile(String),
    Decode(String),
    InvalidReset(String),
    UnsupportedMissionBehavior { gap: &'static str, source_slot: u16 },
}

impl fmt::Display for MissionContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(message) | Self::Decode(message) | Self::InvalidReset(message) => {
                formatter.write_str(message)
            }
            Self::UnsupportedMissionBehavior { gap, source_slot } => write!(
                formatter,
                "unsupported Level 2 mission behavior gap {gap} at source slot {source_slot}"
            ),
        }
    }
}

impl std::error::Error for MissionContentError {}

/// Generic authored/reset template.  The full 1,200 authored records and
/// 1,000 source slots are retained, even when the level dispatcher does not
/// yet implement a particular family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionContentTemplate {
    profile: MissionContentProfile,
    authored_features: Box<[AuthoredFeature]>,
    reset_source_slots: Box<[Level1ResetSourceSlot]>,
    initial_global_rng: u32,
}

impl MissionContentTemplate {
    pub fn decode(
        profile: MissionContentProfile,
        authored: &[u8],
        reset_source_slots: &[Level1ResetSourceSlot],
        initial_global_rng: u32,
    ) -> Result<Self, MissionContentError> {
        if profile.spec_commit != REMC2_SPEC_COMMIT {
            return Err(MissionContentError::InvalidProfile(
                "mission content profile is not pinned to the remc2 specification".to_owned(),
            ));
        }
        if profile.authored_record_count != AUTHORED_FEATURE_COUNT
            || profile.authored_record_bytes != AUTHORED_FEATURE_BYTES
        {
            return Err(MissionContentError::InvalidProfile(
                "mission authored decode expectations do not match the 1,200 x 20-byte source table"
                    .to_owned(),
            ));
        }
        if authored.len() < profile.authored_record_count * profile.authored_record_bytes {
            return Err(MissionContentError::Decode(format!(
                "authored level is too short for {:?}: {} bytes",
                profile.level,
                authored.len()
            )));
        }
        if reset_source_slots.len() != 1_000 {
            return Err(MissionContentError::InvalidReset(format!(
                "{:?} reset requires 1,000 source slots, got {}",
                profile.level,
                reset_source_slots.len()
            )));
        }
        for (index, slot) in reset_source_slots.iter().enumerate() {
            if usize::from(slot.index) != index {
                return Err(MissionContentError::InvalidReset(format!(
                    "reset source slot order mismatch at {index}: {}",
                    slot.index
                )));
            }
        }
        let features = parse_authored_features(authored).map_err(MissionContentError::Decode)?;
        if features.len() != profile.authored_record_count {
            return Err(MissionContentError::Decode(format!(
                "authored record count mismatch: {} != {}",
                features.len(),
                profile.authored_record_count
            )));
        }
        Ok(Self {
            profile,
            authored_features: features.into_boxed_slice(),
            reset_source_slots: reset_source_slots.to_vec().into_boxed_slice(),
            initial_global_rng,
        })
    }

    #[must_use]
    pub const fn profile(&self) -> MissionContentProfile {
        self.profile
    }

    #[must_use]
    pub fn authored_features(&self) -> &[AuthoredFeature] {
        &self.authored_features
    }

    #[must_use]
    pub fn reset_source_slots(&self) -> &[Level1ResetSourceSlot] {
        &self.reset_source_slots
    }

    #[must_use]
    pub const fn initial_global_rng(&self) -> u32 {
        self.initial_global_rng
    }

    #[must_use]
    pub fn session(&self) -> MissionContentSession {
        MissionContentSession::from_template(self)
    }
}

/// The deterministic reset/session state for the generic seam.  Level 2
/// source dispatch is intentionally not approximated here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionContentSession {
    profile: MissionContentProfile,
    authored_features: Box<[AuthoredFeature]>,
    baseline_slots: Box<[Level1ResetSourceSlot]>,
    source_slots: Box<[Level1ResetSourceSlot]>,
    initial_global_rng: u32,
    global_rng: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::NativeWorld;
    use std::path::PathBuf;

    fn level_two_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/l2-debug3.fJ3PaM/level2")
    }

    #[test]
    fn profile_selection_is_typed_and_does_not_fallback() {
        assert_eq!(
            MissionContentProfile::from_metadata(2, 1, 0).unwrap(),
            MissionContentProfile::LEVEL2
        );
        assert_eq!(
            MissionContentProfile::from_metadata(1, 0, 1).unwrap(),
            MissionContentProfile::LEVEL1
        );
        assert!(MissionContentProfile::from_metadata(1, 0, 0).is_err());
    }

    #[test]
    fn qualified_level_two_template_retains_full_denominators_and_fails_closed() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 owner package");
        let template = inspection
            .mission_content_template()
            .expect("generic Level 2 authored/reset template");
        assert_eq!(template.profile(), MissionContentProfile::LEVEL2);
        assert_eq!(template.authored_features().len(), AUTHORED_FEATURE_COUNT);
        assert_eq!(template.reset_source_slots().len(), 1_000);
        assert_eq!(
            template
                .reset_source_slots()
                .iter()
                .filter(|slot| slot.active)
                .count(),
            114
        );

        let mut session = template.session();
        let baseline = session.state_hash();
        let error = session
            .dispatch_source_slot(1)
            .expect_err("unimplemented Level 2 dispatch must fail closed");
        assert!(error.to_string().contains("L2-DISPATCH-AUTHORED-RESET"));
        session
            .set_source_slot_active(1, false)
            .expect("valid source slot mutation");
        assert_ne!(session.state_hash(), baseline);
        session.reset();
        assert_eq!(session.state_hash(), baseline);
    }

    #[test]
    fn independent_level_two_sessions_do_not_share_reset_state() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 owner package");
        let template = inspection
            .mission_content_template()
            .expect("generic Level 2 authored/reset template");
        let mut first = template.session();
        let second = template.session();
        let baseline = second.state_hash();
        first
            .set_source_slot_active(7, false)
            .expect("valid first-session mutation");
        assert_eq!(second.state_hash(), baseline);
        assert_ne!(first.state_hash(), baseline);
    }
}

impl MissionContentSession {
    fn from_template(template: &MissionContentTemplate) -> Self {
        Self {
            profile: template.profile,
            authored_features: template.authored_features.clone(),
            baseline_slots: template.reset_source_slots.clone(),
            source_slots: template.reset_source_slots.clone(),
            initial_global_rng: template.initial_global_rng,
            global_rng: template.initial_global_rng,
        }
    }

    #[must_use]
    pub const fn profile(&self) -> MissionContentProfile {
        self.profile
    }

    #[must_use]
    pub fn authored_features(&self) -> &[AuthoredFeature] {
        &self.authored_features
    }

    #[must_use]
    pub fn source_slots(&self) -> &[Level1ResetSourceSlot] {
        &self.source_slots
    }

    #[must_use]
    pub const fn global_rng(&self) -> u32 {
        self.global_rng
    }

    /// Reset all session-owned state to the immutable template baseline.
    pub fn reset(&mut self) {
        self.source_slots.clone_from(&self.baseline_slots);
        self.global_rng = self.initial_global_rng;
    }

    /// Mutate one generic reset field through the shared seam.  This is useful
    /// to prove session isolation while remaining independent of any future
    /// level-specific behavior.
    pub fn set_source_slot_active(
        &mut self,
        source_slot: u16,
        active: bool,
    ) -> Result<(), MissionContentError> {
        let Some(slot) = self.source_slots.get_mut(usize::from(source_slot)) else {
            return Err(MissionContentError::InvalidReset(format!(
                "source slot {source_slot} is out of bounds"
            )));
        };
        slot.active = active;
        Ok(())
    }

    /// Stable reset/session identity over the complete authored and reset
    /// projections.  It is deliberately independent of heap addresses.
    #[must_use]
    pub fn state_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(REMC2_SPEC_COMMIT.as_bytes());
        hasher.update([
            self.profile.external_level as u8,
            self.profile.internal_level as u8,
        ]);
        hasher.update(self.global_rng.to_le_bytes());
        for feature in &self.authored_features {
            hasher.update(feature.index.to_le_bytes());
            hasher.update(feature.kind.to_le_bytes());
            hasher.update(feature.subtype.to_le_bytes());
            hasher.update(feature.tile_x.to_le_bytes());
            hasher.update(feature.tile_y.to_le_bytes());
            hasher.update(feature.activation_group.to_le_bytes());
            hasher.update(feature.word_10.to_le_bytes());
            hasher.update(feature.stage_tag.to_le_bytes());
            hasher.update(feature.parameter_1.to_le_bytes());
            hasher.update(feature.parameter_2.to_le_bytes());
            hasher.update(feature.parameter_3.to_le_bytes());
        }
        for slot in &self.source_slots {
            hasher.update(slot.lossless_source_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    /// Explicit fail-closed boundary for behavior not yet owned by a Level 2
    /// bead.  A valid source slot still returns an error; it is never silently
    /// skipped.
    pub fn dispatch_source_slot(&mut self, source_slot: u16) -> Result<(), MissionContentError> {
        if !self.profile.is_level2() {
            return Err(MissionContentError::InvalidProfile(
                "generic mission dispatch is reserved for the Level 2 seam; use the exact Level 1 mission state"
                    .to_owned(),
            ));
        }
        if usize::from(source_slot) >= self.source_slots.len() {
            return Err(MissionContentError::InvalidReset(format!(
                "source slot {source_slot} is out of bounds"
            )));
        }
        Err(MissionContentError::UnsupportedMissionBehavior {
            gap: "L2-DISPATCH-AUTHORED-RESET",
            source_slot,
        })
    }
}
