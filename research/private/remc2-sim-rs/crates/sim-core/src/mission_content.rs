//! Shared campaign-level content and reset seams.
//!
//! The Level 1 mission kernel predates the multi-level owner package.  This
//! module supplies the small typed seam needed to select a level from validated
//! metadata and to retain the complete authored/reset denominator before a
//! level-specific dispatcher is implemented.  Level 1 continues to use its
//! established [`Level1MissionTemplate`] constructors; Level 2 is deliberately
//! fail-closed at dispatch boundaries.

use std::{collections::BTreeSet, fmt};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    FlightWorld,
    campaign::spec::{BehaviorSelector, CampaignDefinition, CampaignSelection},
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
    /// Descriptor-selected campaign content. The external/internal identity
    /// remains data on [`MissionContentProfile`]; adding another owner level
    /// never adds another Rust enum variant or dispatcher.
    Descriptor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissionMap {
    Night,
    Day,
    Cave,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetConstructor {
    Level1FrameZero,
    Level2QualifiedPackage,
    DescriptorPackage,
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

/// One source `Type_WizardMapSettings_0x360D2` record from the authored
/// Level 2 map. These records are reset data, not Rust tuning defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionWizardMapSettings {
    pub aggression: i16,
    pub reflexes: i16,
    pub perception: i16,
    pub starting_spells: [u8; 26],
    pub spell_levels: [u8; 26],
    pub blocked_spells: [u8; 26],
    /// Source `Life_0x3612F`; remc2 applies this Q8 value at wizard creation.
    pub life: i16,
    record: Box<[u8]>,
}

impl MissionWizardMapSettings {
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }
}

pub const LEVEL2_WIZARD_MAP_SETTINGS_COUNT: usize = 8;
pub const LEVEL2_WIZARD_MAP_SETTINGS_BYTES: usize = 110;
pub const LEVEL2_WIZARD_MAP_SETTINGS_OFFSET: usize = 25_092;

// `Type_MapData` stores seven 8-byte stage records between the eight
// WizardMapSettings rows and its eleven authored StageVar rows.  These are
// authored records, not the reduced runtime StageVars view: the authored
// selector is necessary for `sub_12100`'s direct/subtype matching.
const LEVEL2_AUTHORED_STAGE_VARIABLE_OFFSET: usize = 26_028;
const LEVEL2_AUTHORED_STAGE_VARIABLE_COUNT: usize = 11;
const LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES: usize = 8;

/// One decoded `terrain_2FECE.StageVars_0x3647A` entry retained exactly where
/// remc2 needs it: before `InitStageVars_11EE0` reduces it into runtime state.
///
/// `selector` is an authored-feature index. For a subtype-match row, it names
/// the exemplar whose subtype is compared by `sub_12100`; it is not itself a
/// Bee-only or class-restricted relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Level2AuthoredStageVariable {
    pub index: u8,
    pub raw_type: u8,
    pub kind: u8,
    pub flags: u8,
    pub stage: u8,
    pub selector: u16,
    /// The complete authored union word at +4. For direct entity routes it
    /// is an authored row index, rather than the low-byte point coordinate.
    pub relation_word: u16,
    pub point_x: u8,
    pub point_y: u8,
}

impl Level2AuthoredStageVariable {
    const SUBTYPE_MATCH_FLAG: u8 = 1;

    #[must_use]
    pub const fn subtype_match(self) -> bool {
        self.flags & Self::SUBTYPE_MATCH_FLAG != 0
    }

    #[must_use]
    pub fn point(self) -> (u16, u16) {
        (u16::from(self.point_x) << 8, u16::from(self.point_y) << 8)
    }

    #[must_use]
    pub const fn references_authored_entity(self, authored_index: u16) -> bool {
        matches!(self.kind, 3 | 4 | 5 | 8 | 9)
            && self.flags & 2 == 0
            && self.relation_word == authored_index
    }
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

const LEVEL2_ACTIVATION_CLOSURE_JSON: &str =
    include_str!("../../../level2-conformance/activation/level2-activation-closure.json");

/// One source-backed stage-tag edge in the bounded Level 2 activation closure.
/// The producer row and source/target groups are read from the immutable
/// activation artifact; no showcase route is encoded in the dispatcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Level2ActivationEdge {
    pub source_group: u16,
    pub target_group: u16,
    pub producer_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Level2ActivationGroup {
    pub id: u16,
    pub rows: Box<[u16]>,
}

/// The validated artifact view consumed by the Level 2 source dispatcher.
/// Membership is retained in artifact/source order and is hashed through the
/// generic mission session so reset/replay identity includes the carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Level2ActivationPlan {
    groups: Box<[Level2ActivationGroup]>,
    owned_rows: Box<[u16]>,
    edges: Box<[Level2ActivationEdge]>,
    artifact_sha256: Box<[u8]>,
}

impl Level2ActivationPlan {
    #[must_use]
    pub(crate) fn group_rows(&self, group: u16) -> Option<&[u16]> {
        self.groups
            .iter()
            .find(|candidate| candidate.id == group)
            .map(|candidate| candidate.rows.as_ref())
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn owned_rows(&self) -> &[u16] {
        &self.owned_rows
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn edges(&self) -> &[Level2ActivationEdge] {
        &self.edges
    }
}

fn artifact_index(value: &Value, field: &str) -> Result<u16, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| format!("activation artifact field {field} is not a u16"))
}

fn artifact_authored_index(value: &Value, field: &str) -> Result<u16, String> {
    let authored_id = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("activation artifact field {field} is not an authored id"))?;
    authored_id
        .strip_prefix("AUT-L2-")
        .ok_or_else(|| format!("activation artifact authored id {authored_id} is malformed"))?
        .parse::<u16>()
        .map_err(|error| format!("activation artifact authored id {authored_id}: {error}"))
}

fn activation_artifact_array<'a>(root: &'a Value, field: &str) -> Result<&'a [Value], String> {
    root.get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("activation artifact field {field} is not an array"))
}

fn activation_artifact_authored_rows(root: &Value) -> Result<&[Value], String> {
    root.get("authored_rows")
        .and_then(|rows| rows.get("rows"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "activation artifact field authored_rows.rows is not an array".to_owned())
}

/// Validate the checked-in closure against the decoded owner authored table.
/// This is also the negative-test seam: changing a member, group, or
/// producer edge fails closed before a Level 2 reset session is constructed.
pub(crate) fn validate_level2_activation_closure_json(
    closure_json: &str,
    authored_features: &[AuthoredFeature],
) -> Result<Level2ActivationPlan, String> {
    const FIRST_OWNED_GROUP: u16 = 1;
    const LAST_OWNED_GROUP: u16 = 18;
    const OWNED_GROUP_COUNT: usize = 18;

    let root: Value = serde_json::from_str(closure_json)
        .map_err(|error| format!("invalid Level 2 activation artifact JSON: {error}"))?;
    let groups = activation_artifact_array(&root, "groups")?;
    let authored_rows = activation_artifact_authored_rows(&root)?;
    let mut owned_rows = Vec::new();
    let mut seen_rows = BTreeSet::new();
    let mut parsed_groups = Vec::new();

    for group_id in FIRST_OWNED_GROUP..=LAST_OWNED_GROUP {
        let group = groups
            .iter()
            .find(|candidate| {
                candidate.get("group").and_then(Value::as_u64) == Some(u64::from(group_id))
            })
            .ok_or_else(|| format!("activation artifact is missing group {group_id}"))?;
        let expected_count = artifact_index(group, "dispatchable_row_count")?;
        let authored_ids = activation_artifact_array(group, "authored_ids")?;
        if usize::from(expected_count) != authored_ids.len() {
            return Err(format!(
                "activation group {group_id} count {} does not match authored-id list {}",
                expected_count,
                authored_ids.len()
            ));
        }
        let expected_owner = format!("group_{group_id}_dispatch");
        let mut rows = Vec::with_capacity(authored_ids.len());
        for authored_id in authored_ids {
            let authored_index = authored_id
                .as_str()
                .ok_or_else(|| "activation artifact authored_ids contains a non-string".to_owned())?
                .strip_prefix("AUT-L2-")
                .ok_or_else(|| "activation artifact authored id is malformed".to_owned())?
                .parse::<u16>()
                .map_err(|error| format!("activation artifact authored id parse: {error}"))?;
            if !seen_rows.insert(authored_index) {
                return Err(format!(
                    "activation authored row {authored_index} appears more than once"
                ));
            }
            let row = authored_rows
                .iter()
                .find(|candidate| {
                    candidate.get("index").and_then(Value::as_u64)
                        == Some(u64::from(authored_index))
                })
                .ok_or_else(|| {
                    format!("activation artifact is missing authored row {authored_index}")
                })?;
            if row.get("activation_group").and_then(Value::as_i64) != Some(i64::from(group_id))
                || row.get("closure_state").and_then(Value::as_str)
                    != Some("conditionally_reachable")
                || row.get("planned_owner").and_then(Value::as_str) != Some(expected_owner.as_str())
            {
                return Err(format!(
                    "activation authored row {authored_index} has mismatched group/state/owner"
                ));
            }
            let feature = authored_features
                .iter()
                .find(|feature| feature.index == authored_index)
                .ok_or_else(|| format!("owner authored table is missing row {authored_index}"))?;
            if feature.activation_group != i16::try_from(group_id).expect("bounded group id") {
                return Err(format!(
                    "owner authored row {authored_index} belongs to group {}, expected {group_id}",
                    feature.activation_group
                ));
            }
            rows.push(authored_index);
            owned_rows.push(authored_index);
        }
        parsed_groups.push(Level2ActivationGroup {
            id: group_id,
            rows: rows.into_boxed_slice(),
        });
    }

    let producer_edges = activation_artifact_array(&root, "producer_edges")?;
    let mut edges = Vec::new();
    let mut edge_targets = BTreeSet::new();
    for edge in producer_edges {
        let source_group = artifact_index(edge, "source_group")?;
        let target_group = artifact_index(edge, "target_group")?;
        if source_group > LAST_OWNED_GROUP
            || !(FIRST_OWNED_GROUP..=LAST_OWNED_GROUP).contains(&target_group)
        {
            continue;
        }
        if edge.get("producer").and_then(Value::as_str) != Some("class11.stageTag") {
            return Err(format!(
                "activation edge {source_group}->{target_group} is not a class11.stageTag producer"
            ));
        }
        let producer_index = artifact_authored_index(edge, "producer_row")?;
        let producer = authored_features
            .iter()
            .find(|feature| feature.index == producer_index)
            .ok_or_else(|| format!("activation producer row {producer_index} is absent"))?;
        if producer.activation_group != i16::try_from(source_group).expect("bounded group id")
            || producer.kind != 11
            || producer.stage_tag != i16::try_from(target_group).expect("bounded group id")
        {
            return Err(format!(
                "activation producer row {producer_index} does not carry {source_group}->{target_group}"
            ));
        }
        if !edge_targets.insert(target_group) {
            return Err(format!(
                "activation target group {target_group} has duplicate edges"
            ));
        }
        edges.push(Level2ActivationEdge {
            source_group,
            target_group,
            producer_index,
        });
    }
    if edge_targets.len() != OWNED_GROUP_COUNT {
        return Err(format!(
            "activation closure has {} of {OWNED_GROUP_COUNT} owned producer targets",
            edge_targets.len()
        ));
    }

    Ok(Level2ActivationPlan {
        groups: parsed_groups.into_boxed_slice(),
        owned_rows: owned_rows.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        artifact_sha256: Sha256::digest(closure_json.as_bytes())
            .to_vec()
            .into_boxed_slice(),
    })
}

pub(crate) fn checked_in_level2_activation_plan(
    authored_features: &[AuthoredFeature],
) -> Result<Level2ActivationPlan, String> {
    validate_level2_activation_closure_json(LEVEL2_ACTIVATION_CLOSURE_JSON, authored_features)
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
    /// Legacy compatibility metadata. Descriptor-backed sessions derive the
    /// authoritative player count, local slot, life, mana and music directly
    /// from the validated runtime/player/entity members instead.
    pub wizard: Option<MissionWizardSettings>,
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
        wizard: Some(MissionWizardSettings {
            player_count: 1,
            local_player_slot: 0,
            ai_player_slot: None,
            map_music: 1,
            initial_life: 1000,
            initial_mana: 1000,
        }),
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
        wizard: Some(MissionWizardSettings {
            player_count: 2,
            local_player_slot: 0,
            ai_player_slot: Some(1),
            map_music: 2,
            initial_life: 1000,
            initial_mana: 1000,
        }),
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

    /// Select the shared authored/reset decoder from a fully validated
    /// campaign definition. External/internal level numbers remain data; the
    /// decoder is selected by the member schema and compiled behavior
    /// closure, never by adding a friendly Level-N implementation branch.
    pub fn from_campaign_definition(
        definition: &CampaignDefinition,
    ) -> Result<Self, MissionContentError> {
        let authored = &definition.campaign_spec.authored_records;
        if authored.count != Some(AUTHORED_FEATURE_COUNT as u64)
            || authored.record_bytes != Some(AUTHORED_FEATURE_BYTES as u64)
            || !definition.campaign_spec.initial_activation.is_empty()
        {
            return Err(MissionContentError::InvalidProfile(
                "campaign definition is not compatible with the shared authored/reset decoder"
                    .to_owned(),
            ));
        }
        let map = match definition
            .campaign_spec
            .presentation_recipe
            .map_type
            .as_str()
        {
            "Day" => MissionMap::Day,
            "Night" => MissionMap::Night,
            "Cave" => MissionMap::Cave,
            other => {
                return Err(MissionContentError::InvalidProfile(format!(
                    "campaign definition has unsupported map type {other}"
                )));
            }
        };
        Ok(Self {
            spec_commit: REMC2_SPEC_COMMIT,
            level: CampaignLevel::Descriptor,
            external_level: definition.identity.external_level,
            internal_level: definition.identity.internal_level,
            map,
            map_type_id: definition.identity.map_type_id,
            authored_record_count: AUTHORED_FEATURE_COUNT,
            authored_record_bytes: AUTHORED_FEATURE_BYTES,
            reset_constructor: ResetConstructor::DescriptorPackage,
            wizard: None,
            mappings: MissionMappings {
                initial_activation_group: 0,
                player_marker_index: 1,
                objective_selectors: LEVEL2_OBJECTIVE_SELECTORS,
            },
        })
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
    wizard_map_settings: Box<[MissionWizardMapSettings]>,
    level2_authored_stage_variables: Box<[Level2AuthoredStageVariable]>,
    level2_activation_plan: Option<Level2ActivationPlan>,
    initial_global_rng: u32,
}

impl MissionContentTemplate {
    /// Decode through the validated descriptor rather than a friendly
    /// per-level profile selected by the caller.
    pub fn decode_campaign_definition(
        definition: &CampaignDefinition,
        authored: &[u8],
        reset_source_slots: &[Level1ResetSourceSlot],
        initial_global_rng: u32,
    ) -> Result<Self, MissionContentError> {
        let profile = MissionContentProfile::from_campaign_definition(definition)?;
        let mut template = Self::decode(profile, authored, reset_source_slots, initial_global_rng)?;

        // Stage variables are part of the shared authored map layout, not a
        // friendly Level-2 runtime profile. Descriptor campaigns retain them
        // so only source-reachable families consult them.
        template.level2_authored_stage_variables =
            parse_level2_authored_stage_variables(authored, &template.authored_features)
                .map_err(MissionContentError::Decode)?
                .into_boxed_slice();

        // Wizard reset settings are shared by missions with this behavior.
        // They do not identify the mission whose authored rows are bound by
        // the checked-in Level-2 activation artifact.
        let requires_wizard_settings = definition.required_behavior_keys().any(|key| {
            key.dispatch_site == 2
                && matches!(
                    key.selector,
                    BehaviorSelector::ClassAction {
                        class: 3,
                        action: 1
                    }
                )
        });
        if requires_wizard_settings {
            template.wizard_map_settings = parse_level2_wizard_map_settings(authored)
                .map_err(MissionContentError::Decode)?
                .into_boxed_slice();
        }
        if definition.identity == CampaignSelection::LEVEL2 {
            template.level2_activation_plan = Some(
                checked_in_level2_activation_plan(&template.authored_features)
                    .map_err(MissionContentError::InvalidReset)?,
            );
        }
        Ok(template)
    }

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
        let wizard_map_settings = if profile.is_level2() {
            parse_level2_wizard_map_settings(authored).map_err(MissionContentError::Decode)?
        } else {
            Vec::new()
        };
        let level2_authored_stage_variables = if profile.is_level2() {
            parse_level2_authored_stage_variables(authored, &features)
                .map_err(MissionContentError::Decode)?
        } else {
            Vec::new()
        };
        let level2_activation_plan = if profile.is_level2() {
            Some(
                checked_in_level2_activation_plan(&features)
                    .map_err(MissionContentError::InvalidReset)?,
            )
        } else {
            None
        };
        Ok(Self {
            profile,
            authored_features: features.into_boxed_slice(),
            reset_source_slots: reset_source_slots.to_vec().into_boxed_slice(),
            wizard_map_settings: wizard_map_settings.into_boxed_slice(),
            level2_authored_stage_variables: level2_authored_stage_variables.into_boxed_slice(),
            level2_activation_plan,
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
    pub fn wizard_map_settings(&self) -> &[MissionWizardMapSettings] {
        &self.wizard_map_settings
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn level2_authored_stage_variables(&self) -> &[Level2AuthoredStageVariable] {
        &self.level2_authored_stage_variables
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

fn parse_level2_wizard_map_settings(
    authored: &[u8],
) -> Result<Vec<MissionWizardMapSettings>, String> {
    let required = LEVEL2_WIZARD_MAP_SETTINGS_OFFSET
        + LEVEL2_WIZARD_MAP_SETTINGS_COUNT * LEVEL2_WIZARD_MAP_SETTINGS_BYTES;
    if authored.len() < required {
        return Err(format!(
            "authored level too short for Level 2 wizard settings: {} < {required}",
            authored.len()
        ));
    }
    let mut settings = Vec::with_capacity(LEVEL2_WIZARD_MAP_SETTINGS_COUNT);
    for index in 0..LEVEL2_WIZARD_MAP_SETTINGS_COUNT {
        let offset = LEVEL2_WIZARD_MAP_SETTINGS_OFFSET + index * LEVEL2_WIZARD_MAP_SETTINGS_BYTES;
        let record = authored[offset..offset + LEVEL2_WIZARD_MAP_SETTINGS_BYTES].to_vec();
        let mut starting_spells = [0_u8; 26];
        starting_spells.copy_from_slice(&record[15..41]);
        let mut spell_levels = [0_u8; 26];
        spell_levels.copy_from_slice(&record[41..67]);
        let mut blocked_spells = [0_u8; 26];
        blocked_spells.copy_from_slice(&record[67..93]);
        settings.push(MissionWizardMapSettings {
            aggression: i16::from_le_bytes([record[3], record[4]]),
            reflexes: i16::from_le_bytes([record[7], record[8]]),
            perception: i16::from_le_bytes([record[11], record[12]]),
            starting_spells,
            spell_levels,
            blocked_spells,
            life: i16::from_le_bytes([record[93], record[94]]),
            record: record.into_boxed_slice(),
        });
    }
    Ok(settings)
}

fn parse_level2_authored_stage_variables(
    authored: &[u8],
    features: &[AuthoredFeature],
) -> Result<Vec<Level2AuthoredStageVariable>, String> {
    let required = LEVEL2_AUTHORED_STAGE_VARIABLE_OFFSET
        + LEVEL2_AUTHORED_STAGE_VARIABLE_COUNT * LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES;
    if authored.len() < required {
        return Err(format!(
            "authored level too short for Level 2 StageVars: {} < {required}",
            authored.len()
        ));
    }

    // `InitStageVars_11EE0` determines the last live entry from indices 1..10
    // and ignores the row-zero sentinel. Preserve both that bound and the
    // source order used by `sub_12100`'s two matching passes.
    let record = |index: usize| {
        let offset =
            LEVEL2_AUTHORED_STAGE_VARIABLE_OFFSET + index * LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES;
        &authored[offset..offset + LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES]
    };
    let count = (1..LEVEL2_AUTHORED_STAGE_VARIABLE_COUNT)
        .rev()
        .find(|&index| record(index)[0] & 0x0f != 0)
        .unwrap_or(0);
    let mut stage_variables = Vec::with_capacity(count);
    for index in 1..=count {
        let bytes = record(index);
        let raw_type = bytes[0];
        let kind = raw_type & 0x0f;
        if kind == 0 {
            continue;
        }
        let selector = u16::from_le_bytes([bytes[2], bytes[3]]);
        if usize::from(selector) >= features.len() {
            return Err(format!(
                "Level 2 StageVar {index} selector {selector} is outside authored feature table"
            ));
        }
        let flags = u8::from(raw_type & 0x80 != 0)
            | (u8::from(raw_type & 0x40 != 0) << 1)
            | if raw_type & 0x10 != 0 { 0x20 } else { 0 }
            | if raw_type & 0x20 != 0 { 0x40 } else { 0 };
        stage_variables.push(Level2AuthoredStageVariable {
            index: u8::try_from(index).expect("bounded StageVar index fits u8"),
            raw_type,
            kind,
            flags,
            stage: bytes[1],
            selector,
            relation_word: u16::from_le_bytes([bytes[4], bytes[5]]),
            point_x: bytes[4],
            point_y: bytes[6],
        });
    }
    Ok(stage_variables)
}

/// The deterministic reset/session state for the generic seam.  Level 2
/// source dispatch is intentionally not approximated here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissionContentSession {
    profile: MissionContentProfile,
    authored_features: Box<[AuthoredFeature]>,

    source_slots: Box<[Level1ResetSourceSlot]>,
    wizard_map_settings: Box<[MissionWizardMapSettings]>,
    level2_authored_stage_variables: Box<[Level2AuthoredStageVariable]>,
    level2_activation_plan: Option<Level2ActivationPlan>,
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
    fn actual_source4_day_template_uses_its_authored_rows_without_level2_plan() {
        use crate::campaign::dispatch::CAMPAIGN_BEHAVIOR_REGISTRY;

        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-paj2-source4-campaign-package");
        let selection = CampaignDefinition::declared_selection(&package).unwrap();
        assert_eq!(
            (
                selection.external_level,
                selection.internal_level,
                selection.map_type_id
            ),
            (5, 4, 0)
        );
        let definition =
            CampaignDefinition::preflight(&package, selection, &CAMPAIGN_BEHAVIOR_REGISTRY)
                .expect("genuine source4 campaign preflight");
        let inspection = NativeWorld::inspect_campaign_definition(&definition)
            .expect("genuine source4 inspection");
        let template = inspection
            .mission_content_template_from_definition(&definition)
            .expect("Day wizard settings do not select another mission's activation plan");
        assert_eq!(template.profile().level, CampaignLevel::Descriptor);
        assert_eq!(template.profile().map, MissionMap::Day);
        assert_eq!(
            template.wizard_map_settings().len(),
            LEVEL2_WIZARD_MAP_SETTINGS_COUNT
        );
        assert_eq!(template.authored_features().len(), AUTHORED_FEATURE_COUNT);
        assert_eq!(template.authored_features()[12].activation_group, 0);
        assert!(template.session().level2_activation_plan().is_none());
        assert!(checked_in_level2_activation_plan(template.authored_features()).is_err());
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
        let reset = session.clone();
        let baseline = session.state_hash();
        let error = session
            .dispatch_source_slot(1)
            .expect_err("unimplemented Level 2 dispatch must fail closed");
        assert!(error.to_string().contains("L2-DISPATCH-AUTHORED-RESET"));
        session
            .set_source_slot_active(1, false)
            .expect("valid source slot mutation");
        assert_ne!(session.state_hash(), baseline);
        session.clone_from(&reset);
        assert_eq!(session.state_hash(), baseline);
    }

    #[test]
    fn authored_stage_entity_references_retain_high_word_and_exclude_model_routes() {
        let authored = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b/level14/authored-level.bin"),
        )
        .expect("real Level 14 authored map");
        let features = parse_authored_features(&authored).unwrap();
        let variables = parse_level2_authored_stage_variables(&authored, &features).unwrap();
        let direct = variables.iter().find(|stage| stage.index == 5).unwrap();
        // The source names creature row 0x138, not its low-byte alias 0x38.
        assert_eq!((features[312].kind, features[312].subtype), (5, 18));
        assert_eq!(direct.relation_word, 312);
        assert!(direct.references_authored_entity(312));
        assert!(!direct.references_authored_entity(56));
        let model = variables.iter().find(|stage| stage.index == 6).unwrap();
        assert_eq!(model.relation_word, 518);
        assert!(
            !model.references_authored_entity(518),
            "model routes retain their exemplar instead of binding one live entity"
        );
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.mission_content_template().unwrap().session();
        session.level2_authored_stage_variables = variables.into_boxed_slice();
        // Sparse source indices remain indices, even when the active count
        // cuts off the later direct route or is cleared without erasing rows.
        assert!(
            session
                .level2_authored_stage_variable_references_to(312, 5)
                .any(|i| i == 5)
        );
        assert!(
            !session
                .level2_authored_stage_variable_references_to(312, 4)
                .any(|i| i == 5)
        );
        assert_eq!(
            session
                .level2_authored_stage_variable_references_to(312, 0)
                .count(),
            0
        );
    }

    #[test]
    fn authored_goat_constructor_publishes_direct_reference_without_own_route() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::RuntimeRelation;
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3");
        let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3).unwrap();
        // Exercise a direct route to an actual group-5 Goat using the real
        // producer. The original row is a model route to another family.
        let authored_index = 166;
        let live = &mut campaign.simulation_mut().session.state.live;
        let authored = live
            .mission
            .level2_authored_stage_variables
            .iter_mut()
            .find(|stage| stage.index == 1)
            .unwrap();
        authored.flags &= !2;
        authored.raw_type &= !0x40;
        authored.relation_word = authored_index;
        let runtime = &mut live.core.runtime;
        let original_counter = runtime.stage_variables[1].word_2;
        runtime.stage_variables[1].flags = (runtime.stage_variables[1].flags & !2) | 4;
        campaign
            .simulation_mut()
            .activate_authored_group_prerequisite_for_test(5)
            .unwrap();
        let session = campaign.simulation();
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == authored_index)
            .unwrap();
        let goat = session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == source.source_entity_index)
            .unwrap();
        assert_eq!((goat.action, goat.stage_var_1), (9, 0));
        let runtime = session.runtime();
        assert_eq!(
            runtime.stage_variables[1].relation,
            RuntimeRelation::LiveEntity(source.source_entity_index)
        );
        assert_eq!(runtime.stage_variables[1].flags & 4, 0);
        assert_eq!(runtime.stage_variables[1].word_2, original_counter);
        let bytes = &runtime.bytes()[404..416];
        assert_eq!(bytes[1], runtime.stage_variables[1].flags as u8);
        assert_eq!(bytes[4..8], [1, 0, 0, 0]);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            u32::from(source.source_entity_index) + 1
        );
    }

    #[test]
    fn level_two_authored_stagevar_l2_06_subtype_match_maps_all_day_bees() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 owner package");
        let template = inspection
            .mission_content_template()
            .expect("Level 2 content template");
        let l2_06 = template
            .level2_authored_stage_variables()
            .iter()
            .copied()
            .find(|stage_var| stage_var.index == 6)
            .expect("authored L2-06 StageVar");
        assert_eq!(
            (
                l2_06.raw_type,
                l2_06.kind,
                l2_06.flags,
                l2_06.stage,
                l2_06.selector,
                l2_06.point(),
            ),
            (0x82, 2, 1, 0, 231, (0x6500, 0x1000))
        );

        let session = template.session();
        let qualified_rows = [
            210_u16, 211, 212, 229, 230, 231, 236, 237, 257, 307, 308, 312,
        ];
        for authored_index in qualified_rows {
            let feature = session
                .authored_features()
                .iter()
                .copied()
                .find(|feature| feature.index == authored_index)
                .expect("qualified Day-Bee authored row");
            assert_eq!((feature.kind, feature.subtype), (5, 2));
            assert_eq!(
                session.level2_authored_stage_variable_for(feature, 10),
                Some(l2_06)
            );
            assert_eq!(session.level2_authored_stage_variable_for(feature, 5), None);
            assert_eq!(session.level2_authored_stage_variable_for(feature, 0), None);
        }

        // Decode against the real owner package, then corrupt only the
        // authored selector in memory: a missing selector must fail before
        // any production session can infer a substitute mapping.
        let mut malformed = std::fs::read(level_two_fixture_path().join("authored-level.bin"))
            .expect("qualified Level 2 authored bytes");
        let selector_offset =
            LEVEL2_AUTHORED_STAGE_VARIABLE_OFFSET + 6 * LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES + 2;
        malformed[selector_offset..selector_offset + 2].copy_from_slice(&1_200_u16.to_le_bytes());
        assert!(
            MissionContentTemplate::decode(
                MissionContentProfile::LEVEL2,
                &malformed,
                template.reset_source_slots(),
                template.initial_global_rng(),
            )
            .is_err()
        );

        // Removing just L2-06 is a valid authored decode but removes the
        // subtype-2 route; it never falls back to a friendly Bee default.
        let mut missing = std::fs::read(level_two_fixture_path().join("authored-level.bin"))
            .expect("qualified Level 2 authored bytes");
        missing[LEVEL2_AUTHORED_STAGE_VARIABLE_OFFSET + 6 * LEVEL2_AUTHORED_STAGE_VARIABLE_BYTES] =
            0;
        let missing_session = MissionContentTemplate::decode(
            MissionContentProfile::LEVEL2,
            &missing,
            template.reset_source_slots(),
            template.initial_global_rng(),
        )
        .expect("missing L2-06 remains a structurally valid authored table")
        .session();
        let row_229 = missing_session
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == 229)
            .expect("row 229");
        assert!(
            missing_session
                .level2_authored_stage_variable_for(row_229, 10)
                .is_none()
        );
    }

    #[test]
    fn level_two_activation_plan_binds_full_denominator_and_rejects_tampered_edges() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 owner package");
        let template = inspection
            .mission_content_template()
            .expect("Level 2 activation plan");
        let session = template.session();
        let plan = session
            .level2_activation_plan()
            .expect("Level 2 activation plan is present");
        assert_eq!(plan.owned_rows().len(), 151);
        assert_eq!(plan.edges().len(), 18);

        let mut member_tamper: Value = serde_json::from_str(LEVEL2_ACTIVATION_CLOSURE_JSON)
            .expect("checked-in activation artifact JSON");
        let group = member_tamper["groups"]
            .as_array_mut()
            .expect("activation groups")
            .iter_mut()
            .find(|group| group["group"] == 9)
            .expect("group 9");
        group["authored_ids"]
            .as_array_mut()
            .expect("group 9 authored ids")
            .pop();
        assert!(
            validate_level2_activation_closure_json(
                &serde_json::to_string(&member_tamper).expect("member tamper JSON"),
                template.authored_features(),
            )
            .is_err()
        );

        let mut edge_tamper: Value = serde_json::from_str(LEVEL2_ACTIVATION_CLOSURE_JSON)
            .expect("checked-in activation artifact JSON");
        let edges = edge_tamper["producer_edges"]
            .as_array_mut()
            .expect("producer edges");
        let owned_edge = edges
            .iter()
            .position(|edge| edge["target_group"] == 18)
            .expect("owned target group 18 producer edge");
        edges.remove(owned_edge);
        assert!(
            validate_level2_activation_closure_json(
                &serde_json::to_string(&edge_tamper).expect("edge tamper JSON"),
                template.authored_features(),
            )
            .is_err()
        );
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

            source_slots: template.reset_source_slots.clone(),
            wizard_map_settings: template.wizard_map_settings.clone(),
            level2_authored_stage_variables: template.level2_authored_stage_variables.clone(),
            level2_activation_plan: template.level2_activation_plan.clone(),
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

    /// Source linked terrain constructors consume stage tags while following
    /// predecessor/successor references in this session's actual row table.
    pub(crate) fn authored_features_mut(&mut self) -> &mut [AuthoredFeature] {
        &mut self.authored_features
    }

    /// Move one package-authored feature before its production constructor
    /// consumes it. The private MC2 adapter has already proven the authored
    /// record/kind relation; this seam changes only its tile coordinates.
    pub(crate) fn set_authored_feature_tile(
        &mut self,
        authored_record: u16,
        tile_x: u16,
        tile_y: u16,
    ) -> Result<(), MissionContentError> {
        let Some(feature) = self
            .authored_features
            .iter_mut()
            .find(|feature| feature.index == authored_record)
        else {
            return Err(MissionContentError::InvalidReset(format!(
                "authored feature {authored_record} is absent"
            )));
        };
        feature.tile_x = tile_x;
        feature.tile_y = tile_y;
        Ok(())
    }

    #[must_use]
    pub fn source_slots(&self) -> &[Level1ResetSourceSlot] {
        &self.source_slots
    }

    #[must_use]
    pub fn wizard_map_settings(&self) -> &[MissionWizardMapSettings] {
        &self.wizard_map_settings
    }

    /// Source-ordered `sub_12100` lookup: direct authored-row selectors have
    /// precedence; only then may the first subtype-match StageVar select the
    /// exemplar's subtype. The caller owns its class/model producer gate.
    #[must_use]
    pub(crate) fn level2_authored_stage_variable_for(
        &self,
        feature: AuthoredFeature,
        active_count: u8,
    ) -> Option<Level2AuthoredStageVariable> {
        self.level2_authored_stage_variables
            .iter()
            .copied()
            .find(|stage_var| {
                stage_var.index <= active_count
                    && !stage_var.subtype_match()
                    && stage_var.selector == feature.index
            })
            .or_else(|| {
                self.level2_authored_stage_variables
                    .iter()
                    .copied()
                    .find(|stage_var| {
                        stage_var.index <= active_count
                            && stage_var.subtype_match()
                            && self
                                .authored_features
                                .get(usize::from(stage_var.selector))
                                .is_some_and(|exemplar| exemplar.subtype == feature.subtype)
                    })
            })
    }

    /// `sub_12100`'s second pass publishes the constructed creature into
    /// every direct route that names it, independently of its own AI route.
    pub(crate) fn level2_authored_stage_variable_references_to(
        &self,
        authored_index: u16,
        active_count: u8,
    ) -> impl Iterator<Item = u8> + '_ {
        self.level2_authored_stage_variables
            .iter()
            .filter(move |stage| {
                stage.index <= active_count && stage.references_authored_entity(authored_index)
            })
            .map(|stage| stage.index)
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn level2_activation_plan(&self) -> Option<&Level2ActivationPlan> {
        self.level2_activation_plan.as_ref()
    }

    #[must_use]
    pub(crate) fn level2_dispatch_group_rows(&self, group: u16) -> Option<&[u16]> {
        self.level2_activation_plan
            .as_ref()
            .and_then(|plan| plan.group_rows(group))
    }

    #[must_use]
    pub const fn global_rng(&self) -> u32 {
        self.global_rng
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
        for settings in &self.wizard_map_settings {
            hasher.update(settings.record_bytes());
        }
        if !self.level2_authored_stage_variables.is_empty() {
            hasher.update(b"level2-authored-stage-vars-v2");
            for stage_var in &self.level2_authored_stage_variables {
                hasher.update([
                    stage_var.index,
                    stage_var.raw_type,
                    stage_var.kind,
                    stage_var.flags,
                    stage_var.stage,
                    stage_var.point_x,
                    stage_var.point_y,
                ]);
                hasher.update(stage_var.selector.to_le_bytes());
                hasher.update(stage_var.relation_word.to_le_bytes());
            }
        }
        if let Some(plan) = &self.level2_activation_plan {
            hasher.update(b"level2-activation-plan-v1");
            hasher.update(plan.artifact_sha256.as_ref());
            for group in plan.groups.iter() {
                hasher.update(group.id.to_le_bytes());
                for authored_index in &group.rows {
                    hasher.update(authored_index.to_le_bytes());
                }
            }
            for edge in plan.edges.iter() {
                hasher.update(edge.source_group.to_le_bytes());
                hasher.update(edge.target_group.to_le_bytes());
                hasher.update(edge.producer_index.to_le_bytes());
            }
        }
        format!("{:x}", hasher.finalize())
    }

    /// Explicit fail-closed boundary for unimplemented Level 2 behavior.
    /// A valid source slot still returns an error; it is never silently
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
