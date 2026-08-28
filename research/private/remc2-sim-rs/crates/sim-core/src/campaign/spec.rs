//! Fail-closed campaign package and descriptor preflight for contract v1.
//!
//! CF-02 deliberately stops at a validated data surface.  It does not select
//! a session, install a scheduler, or allocate a handle; CF-11 consumes this
//! receipt after the static registry work lands.

use super::mc2_level_definition::{Mc2LevelDefinition, Mc2MovableRuntimeHint};
use super::source_facts::{
    SelectorProvenance, SourceFact, SourceFactContext, SourceField, decode_building_parameters,
    validate_source_facts,
};
use crate::level_definition::{
    BuildingKind, CreatureKind, ItemKind, LevelDefinition, LevelEntityId, LevelEntityKind,
    LevelObjectiveId, MechanicKind, RivalWizardId, SceneryKind, SpellKind, SwitchKind,
    TilePosition,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

pub const CAMPAIGN_PACKAGE_SCHEMA: &str = "mc2-owner-campaign-package/v1";
pub const LEVEL2_CAMPAIGN_PACKAGE_SCHEMA: &str = "mc2-owner-campaign-package/v2";
pub const LEVEL1_ASSET_CAMPAIGN_PACKAGE_SCHEMA: &str = "mc2-owner-campaign-package/v3";
pub const CAMPAIGN_DEFINITION_SCHEMA: &str = "mc2-campaign-definition/v1";
pub const LEVEL2_CAMPAIGN_DEFINITION_SCHEMA: &str = "mc2-campaign-definition/v2";
pub const LEVEL1_ASSET_CAMPAIGN_DEFINITION_SCHEMA: &str = "mc2-campaign-definition/v3";
pub const LEVEL2_REMC2_COMMIT: &str = "6806c7204d17220ecc0430759f784aea8ee2ddb2";
const FIREBALL_PROJECTILE_BASE_ASSET_SHA256: &str =
    "331df08c5fa8483d5dd75dc10abd9dd76cc26918d6447403b3f3199bc31f521d";
const FIREBALL_PROJECTILE_FADE_ASSET_SHA256: &str =
    "61059124cba22dd3086baab36bb7d12a9ab28ebf37d394827a49bea4f6c96784";

const FEATURE_CAMPAIGN_MEMBERS: [&str; 15] = [
    "authored-level.bin",
    "campaign-definition.json",
    "collision-masks.bin",
    "entities-post-features.bin",
    "entities.bin",
    "feature-allocator.bin",
    "feature-construction-inputs.bin",
    "feature-end.bin",
    "feature-start.bin",
    "players.bin",
    "presentation.bin",
    "runtime.bin",
    "terrain-post-features.bin",
    "terrain-pre-features.bin",
    "terrain.bin",
];

const LEVEL2_CAMPAIGN_MEMBERS: [&str; 16] = [
    "authored-level.bin",
    "building-parameters.bin",
    "campaign-definition.json",
    "collision-masks.bin",
    "entities-post-features.bin",
    "entities.bin",
    "feature-allocator.bin",
    "feature-construction-inputs.bin",
    "feature-end.bin",
    "feature-start.bin",
    "players.bin",
    "presentation.bin",
    "runtime.bin",
    "terrain-post-features.bin",
    "terrain-pre-features.bin",
    "terrain.bin",
];

const LEVEL1_CAMPAIGN_MEMBERS: [&str; 17] = [
    "authored-level.bin",
    "campaign-definition.json",
    "entities-post-features.bin",
    "entities.bin",
    "feature-allocator.bin",
    "feature-construction-inputs.bin",
    "feature-end.bin",
    "feature-start.bin",
    "fireball-projectile-base.indexed-sprite.json",
    "fireball-projectile-fade.indexed-sprite.json",
    "owner-manifest.json",
    "players.bin",
    "presentation.bin",
    "runtime.bin",
    "terrain-post-features.bin",
    "terrain-pre-features.bin",
    "terrain.bin",
];

const AUTHORED_RECORD_OFFSET: usize = 1_091;
const AUTHORED_RECORD_COUNT: usize = 1_200;
const AUTHORED_RECORD_BYTES: usize = 20;
const AUTHORED_STAGE_OFFSET: usize =
    AUTHORED_RECORD_OFFSET + AUTHORED_RECORD_COUNT * AUTHORED_RECORD_BYTES + 1 + 8 * 110;
const ENTITY_COUNT: usize = 1_000;
const ENTITY_RECORD_BYTES: usize = 176;
const PLAYER_COUNT: usize = 8;

#[derive(Debug, Clone, Copy)]
struct CampaignPackageProfile {
    members: &'static [&'static str],
    package_schema: &'static str,
    definition_schema: &'static str,
    player_schema: &'static str,
    player_version: u16,
    player_record_bytes: usize,
    feature_allocator_bytes: usize,
    feature_allocator_trace_count: u32,
    has_collision_bank: bool,
    has_building_parameters: bool,
    has_presentation_assets: bool,
    has_owner_manifest: bool,
}

/// Caller identity that must equal both the outer and descriptor identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignSelection {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type_id: u8,
}

impl CampaignSelection {
    pub const LEVEL1: Self = Self {
        external_level: 1,
        internal_level: 0,
        map_type_id: 1,
    };
    pub const LEVEL2: Self = Self {
        external_level: 2,
        internal_level: 1,
        map_type_id: 0,
    };
    pub const LEVEL3: Self = Self {
        external_level: 3,
        internal_level: 2,
        map_type_id: 0,
    };
}

fn campaign_package_profile(selection: CampaignSelection) -> Option<CampaignPackageProfile> {
    match selection {
        CampaignSelection::LEVEL1 => Some(CampaignPackageProfile {
            members: &LEVEL1_CAMPAIGN_MEMBERS,
            package_schema: LEVEL1_ASSET_CAMPAIGN_PACKAGE_SCHEMA,
            definition_schema: LEVEL1_ASSET_CAMPAIGN_DEFINITION_SCHEMA,
            player_schema: "mc2-player-arena/v2",
            player_version: 2,
            player_record_bytes: 573,
            feature_allocator_bytes: 4_440,
            feature_allocator_trace_count: 51,
            has_collision_bank: false,
            has_building_parameters: false,
            has_presentation_assets: true,
            has_owner_manifest: true,
        }),
        CampaignSelection::LEVEL2 => Some(CampaignPackageProfile {
            members: &LEVEL2_CAMPAIGN_MEMBERS,
            package_schema: LEVEL2_CAMPAIGN_PACKAGE_SCHEMA,
            definition_schema: LEVEL2_CAMPAIGN_DEFINITION_SCHEMA,
            player_schema: "mc2-player-arena/v4",
            player_version: 4,
            player_record_bytes: 663,
            feature_allocator_bytes: 4_032,
            feature_allocator_trace_count: 0,
            has_collision_bank: true,
            has_building_parameters: true,
            has_presentation_assets: false,
            has_owner_manifest: false,
        }),
        CampaignSelection::LEVEL3 => Some(CampaignPackageProfile {
            members: &FEATURE_CAMPAIGN_MEMBERS,
            package_schema: CAMPAIGN_PACKAGE_SCHEMA,
            definition_schema: CAMPAIGN_DEFINITION_SCHEMA,
            player_schema: "mc2-player-arena/v4",
            player_version: 4,
            player_record_bytes: 663,
            feature_allocator_bytes: 4_032,
            feature_allocator_trace_count: 0,
            has_collision_bank: true,
            has_building_parameters: false,
            has_presentation_assets: false,
            has_owner_manifest: false,
        }),
        _ => None,
    }
}

fn building_objective_provenance(
    has_building_parameters: bool,
    authored: &[u8],
    declared_rows: &[AuthoredBehaviorRow],
) -> Result<Vec<SelectorProvenance>> {
    if !has_building_parameters {
        return Ok(Vec::new());
    }
    let offset = AUTHORED_STAGE_OFFSET + 4 * 7;
    let stage = authored[offset] as i8;
    let objective_selector = i16::from_le_bytes([authored[offset + 1], authored[offset + 2]]);
    if stage != 9 || objective_selector != 153 {
        return fail(
            ValidationStage::SourceFacts,
            "standing-building objective operation differs from the qualified Level 2 binding",
        );
    }
    let authored_record = u16::try_from(objective_selector).map_err(|_| CampaignPackageError {
        stage: ValidationStage::SourceFacts,
        message: "standing-building objective selector is outside the authored arena".to_owned(),
    })?;
    Ok(declared_rows
        .iter()
        .filter(|row| {
            row.authored_record == authored_record
                && row.initial_source_slot.is_none()
                && row.class == 10
                && row.subtype == 45
        })
        .map(|row| SelectorProvenance {
            authored_record: row.authored_record,
            source_class: row.class,
            source_model: row.model,
            source_subtype: row.subtype,
            source_field: SourceField::Par1_14,
            selector_value: u32::from(row.model),
        })
        .collect())
}

fn campaign_map_type_name(map_type_id: u8) -> Option<&'static str> {
    match map_type_id {
        0 => Some("Day"),
        1 => Some("Night"),
        2 => Some("Cave"),
        _ => None,
    }
}

/// Monotone validation stages from the contract.  No session exists at any
/// stage returned by [`CampaignDefinition::preflight`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ValidationStage {
    OuterManifest = 1,
    DirectorySet = 2,
    MemberHashes = 3,
    Definition = 4,
    TypedMembers = 5,
    SourceFacts = 6,
    TypedOperations = 7,
    BehaviorClosure = 8,
    ResetState = 9,
}

/// Typed fail-closed error carrying the earliest failed stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignPackageError {
    pub stage: ValidationStage,
    pub message: String,
}

impl fmt::Display for CampaignPackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "campaign preflight stage {}: {}",
            self.stage as u8, self.message
        )
    }
}

impl Error for CampaignPackageError {}

type Result<T> = std::result::Result<T, CampaignPackageError>;

fn fail<T>(stage: ValidationStage, message: impl Into<String>) -> Result<T> {
    Err(CampaignPackageError {
        stage,
        message: message.into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRef {
    pub member: String,
    pub sha256: String,
    pub schema: Option<String>,
    pub count: Option<u64>,
    pub record_bytes: Option<u64>,
}

/// One exact row freshly decoded from the authored/reset members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBehaviorRow {
    pub authored_record: u16,
    pub initial_source_slot: Option<u16>,
    pub class: u8,
    pub action: u8,
    pub model: u8,
    pub subtype: u8,
    pub initial_dispatch_sites: Vec<u16>,
}

/// Stable behavior selector vocabulary.  Addresses and function names are
/// intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BehaviorSelector {
    ClassAction { class: u8, action: u8 },
    ClassModel { class: u8, model: u8 },
    TypeSubtype { type_id: u8, subtype: u8 },
    DirectCallback { stable_callback_id: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BehaviorKey {
    pub dispatch_site: u16,
    pub selector: BehaviorSelector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorProvenance {
    AuthoredRecord {
        authored_record: u16,
        initial_source_slot: Option<u16>,
    },
    Predecessor(BehaviorKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorRequirement {
    pub key: BehaviorKey,
    pub provenance: Vec<BehaviorProvenance>,
}

/// CF-03 supplies the compiled registry.  CF-02 requires it explicitly so a
/// descriptor can never certify its own handlers.
pub trait CampaignCapabilityRegistry {
    fn contains(&self, key: &BehaviorKey) -> bool;
}

impl<F> CampaignCapabilityRegistry for F
where
    F: Fn(&BehaviorKey) -> bool,
{
    fn contains(&self, key: &BehaviorKey) -> bool {
        self(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveOperation {
    pub operation_kind: u16,
    pub operation_version: u16,
    pub objective_slot: u8,
    pub stage_index: i8,
    pub objective_selector: i16,
    pub axis_x: i16,
    pub axis_y: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RngFieldEncoding {
    U16LittleEndian,
    U32LittleEndian,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RngSourceField {
    pub member: String,
    pub sha256: String,
    pub offset: u64,
    pub encoding: RngFieldEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialRng {
    pub global: RngSourceField,
    pub terrain: RngSourceField,
}

/// Schema v1 admits no Level 2 initial-activation operation.  The empty enum
/// keeps the typed union closed while preserving the descriptor field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialActivationOperation {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetSources {
    pub terrain: MemberRef,
    pub entities: MemberRef,
    pub players: MemberRef,
    pub runtime: MemberRef,
    pub feature_boundary: Vec<MemberRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationResolvedFiles {
    pub block_bank: String,
    pub building_bank: String,
    pub font_bank: String,
    pub language: String,
    pub level_records: String,
    pub palette: String,
    pub sky_bank: String,
    pub spell_table: String,
    pub sprite_bank: String,
    pub terrain_bank: String,
    pub web_bank: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRecipe {
    pub member: MemberRef,
    pub block_bank: String,
    pub cursor: u8,
    pub map_type: String,
    pub palette: String,
    pub resolved_files: PresentationResolvedFiles,
    pub sky_bank: String,
    pub sound_bank: u8,
    pub sprite_bank: String,
    pub terrain_bank: String,
    pub terrain_graphics_flag_2: bool,
    pub terrain_graphics_flags: u32,
    pub texture_size: u16,
    pub vga_resolution_selector: u16,
    pub web_bank: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortablePresentationAsset {
    pub(crate) artifact_id: String,
    pub(crate) slot_id: String,
    pub(crate) member: MemberRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignSpec {
    pub authored_records: MemberRef,
    pub authored_behavior_rows: Vec<AuthoredBehaviorRow>,
    pub reset_sources: ResetSources,
    pub initial_rng: InitialRng,
    pub initial_activation: Vec<InitialActivationOperation>,
    pub objectives: Vec<ObjectiveOperation>,
    pub presentation_recipe: PresentationRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetStatePreflightReceipt {
    pub sha256: String,
    pub structurally_validated: bool,
}

/// Fully validated data surface.  Construction of a live session is outside
/// this type and therefore cannot happen on a failed preflight.
#[derive(Debug, Clone)]
pub struct CampaignDefinition {
    pub package_root: PathBuf,
    pub package_sha256: String,
    pub identity: CampaignSelection,
    pub campaign_spec: CampaignSpec,
    presentation_assets: Vec<PortablePresentationAsset>,
    pub source_facts: Vec<SourceFact>,
    pub required_behaviors: Vec<BehaviorRequirement>,
    pub reset_state_receipt: ResetStatePreflightReceipt,
    pub completed_stage: ValidationStage,
    level_definition: LevelDefinition,
    creature_by_authored_record: BTreeMap<u16, CreatureKind>,
    creature_by_initial_source_slot: BTreeMap<u16, CreatureKind>,
    creature_model_by_kind: BTreeMap<CreatureKind, u8>,
    spell_by_authored_record: BTreeMap<u16, SpellKind>,
    spell_family_by_kind: BTreeMap<SpellKind, u8>,
    building_by_authored_record: BTreeMap<u16, BuildingKind>,
    building_by_initial_source_slot: BTreeMap<u16, BuildingKind>,
    scenery_by_authored_record: BTreeMap<u16, SceneryKind>,
    scenery_by_initial_source_slot: BTreeMap<u16, SceneryKind>,
    item_by_authored_record: BTreeMap<u16, ItemKind>,
    switch_by_authored_record: BTreeMap<u16, SwitchKind>,
    switch_by_initial_source_slot: BTreeMap<u16, SwitchKind>,
    mechanic_by_authored_record: BTreeMap<u16, MechanicKind>,
    mechanic_by_initial_source_slot: BTreeMap<u16, MechanicKind>,
    mechanic_entity_by_authored_record: BTreeMap<u16, LevelEntityId>,
    standing_building_chains: BTreeMap<LevelObjectiveId, Box<[u8]>>,
    rival_wizard_by_id: BTreeMap<RivalWizardId, u16>,
    mechanic_entity_by_initial_source_slot: BTreeMap<u16, LevelEntityId>,
    movable_entity_bindings: BTreeMap<LevelEntityId, Mc2EntityRuntimeBinding>,
    local_player_fireball_mana_cost: Option<i32>,
    member_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mc2EntityRuntimeBinding {
    InitialSource {
        initial_source_slot: u16,
        kind: LevelEntityKind,
        source_id: u16,
    },
    AuthoredFeature {
        authored_record: u16,
        kind: LevelEntityKind,
    },
}

impl CampaignDefinition {
    pub(crate) fn with_level_definition_variant(
        &self,
        level: LevelDefinition,
    ) -> std::result::Result<Self, String> {
        level.verify_session_reset_variant_of(&self.level_definition)?;
        let mut candidate = self.clone();
        candidate.level_definition = level;
        Ok(candidate)
    }

    /// Read the bounded selection declared by a campaign package. Full schema,
    /// identity, member, and behavior validation still happens in
    /// [`Self::preflight`]; this helper only lets neutral adapters avoid a
    /// compiled per-level selection.
    pub fn declared_selection(package_root: impl AsRef<Path>) -> Result<CampaignSelection> {
        let manifest_path = package_root.as_ref().join("manifest.json");
        let bytes = fs::read(&manifest_path).map_err(|error| CampaignPackageError {
            stage: ValidationStage::OuterManifest,
            message: format!("cannot read manifest.json: {error}"),
        })?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|error| CampaignPackageError {
                stage: ValidationStage::OuterManifest,
                message: format!("cannot parse manifest.json: {error}"),
            })?;
        let manifest = value.as_object().ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::OuterManifest,
            message: "manifest is not an object".to_owned(),
        })?;
        let level = manifest
            .get("level")
            .and_then(Value::as_object)
            .ok_or_else(|| CampaignPackageError {
                stage: ValidationStage::OuterManifest,
                message: "manifest.level is not an object".to_owned(),
            })?;
        let map_type = level
            .get("map_type")
            .and_then(Value::as_object)
            .ok_or_else(|| CampaignPackageError {
                stage: ValidationStage::OuterManifest,
                message: "manifest.level.map_type is not an object".to_owned(),
            })?;
        Ok(CampaignSelection {
            external_level: bounded_u16(
                level.get("external").unwrap_or(&Value::Null),
                "manifest.level.external",
                ValidationStage::OuterManifest,
            )?,
            internal_level: bounded_u16(
                level.get("internal").unwrap_or(&Value::Null),
                "manifest.level.internal",
                ValidationStage::OuterManifest,
            )?,
            map_type_id: bounded_u8(
                map_type.get("id").unwrap_or(&Value::Null),
                "manifest.level.map_type.id",
                ValidationStage::OuterManifest,
            )?,
        })
    }

    /// Validate stages 1–9 in contract order without allocating a handle or
    /// touching session/global state.
    pub fn preflight(
        package_root: impl AsRef<Path>,
        caller: CampaignSelection,
        capabilities: &dyn CampaignCapabilityRegistry,
    ) -> Result<Self> {
        let package_root = package_root.as_ref();

        // Stage 1: exact outer schema and bounded identities.
        let manifest_bytes =
            fs::read(package_root.join("manifest.json")).map_err(|error| CampaignPackageError {
                stage: ValidationStage::OuterManifest,
                message: format!("cannot read manifest.json: {error}"),
            })?;
        let manifest_value: Value =
            serde_json::from_slice(&manifest_bytes).map_err(|error| CampaignPackageError {
                stage: ValidationStage::OuterManifest,
                message: format!("cannot parse manifest.json: {error}"),
            })?;
        let manifest_fields: &[&str] = if caller == CampaignSelection::LEVEL1 {
            &[
                "byte_order",
                "files",
                "level",
                "owner_export_package_sha256",
                "package_sha256",
                "schema",
            ]
        } else {
            &["byte_order", "files", "level", "package_sha256", "schema"]
        };
        let manifest = exact_object(
            &manifest_value,
            manifest_fields,
            "manifest",
            ValidationStage::OuterManifest,
        )?;
        let Some(profile) = campaign_package_profile(caller) else {
            return fail(
                ValidationStage::OuterManifest,
                format!(
                    "caller selection external {}/internal {}/map {} has no campaign package profile",
                    caller.external_level, caller.internal_level, caller.map_type_id
                ),
            );
        };
        require_string_eq(
            &manifest["schema"],
            profile.package_schema,
            "manifest.schema",
            ValidationStage::OuterManifest,
        )?;
        require_string_eq(
            &manifest["byte_order"],
            "little-endian",
            "manifest.byte_order",
            ValidationStage::OuterManifest,
        )?;
        let level = exact_object(
            &manifest["level"],
            &["external", "internal", "map_type"],
            "manifest.level",
            ValidationStage::OuterManifest,
        )?;
        let map_type = exact_object(
            &level["map_type"],
            &["id", "name"],
            "manifest.level.map_type",
            ValidationStage::OuterManifest,
        )?;
        let outer = CampaignSelection {
            external_level: bounded_u16(
                &level["external"],
                "manifest.level.external",
                ValidationStage::OuterManifest,
            )?,
            internal_level: bounded_u16(
                &level["internal"],
                "manifest.level.internal",
                ValidationStage::OuterManifest,
            )?,
            map_type_id: bounded_u8(
                &map_type["id"],
                "manifest.level.map_type.id",
                ValidationStage::OuterManifest,
            )?,
        };
        let Some(map_type_name) = campaign_map_type_name(outer.map_type_id) else {
            return fail(
                ValidationStage::OuterManifest,
                format!(
                    "outer selection external {}/internal {}/map {} uses an unsupported map type",
                    outer.external_level, outer.internal_level, outer.map_type_id
                ),
            );
        };
        if caller != outer
            || string(
                &map_type["name"],
                "manifest.level.map_type.name",
                ValidationStage::OuterManifest,
            )? != map_type_name
        {
            return fail(
                ValidationStage::OuterManifest,
                "caller/outer selection or map-type name mismatches the campaign profile",
            );
        }
        let files_value = exact_object(
            &manifest["files"],
            profile.members,
            "manifest.files",
            ValidationStage::OuterManifest,
        )?;
        let mut files = BTreeMap::new();
        for &name in profile.members {
            normalized_member_name(name, ValidationStage::OuterManifest)?;
            let metadata = exact_object(
                &files_value[name],
                &["bytes", "sha256"],
                &format!("manifest.files.{name}"),
                ValidationStage::OuterManifest,
            )?;
            let bytes = unsigned(
                &metadata["bytes"],
                &format!("manifest.files.{name}.bytes"),
                ValidationStage::OuterManifest,
            )?;
            let hash = lower_sha256(
                &metadata["sha256"],
                &format!("manifest.files.{name}.sha256"),
                ValidationStage::OuterManifest,
            )?;
            files.insert(name.to_owned(), (bytes, hash));
        }
        let package_sha256 = lower_sha256(
            &manifest["package_sha256"],
            "manifest.package_sha256",
            ValidationStage::OuterManifest,
        )?;

        // Stage 2: exact regular directory set.
        let expected_paths = profile
            .members
            .iter()
            .copied()
            .chain(std::iter::once("manifest.json"))
            .collect::<BTreeSet<_>>();
        let mut actual_paths = BTreeSet::new();
        for entry in fs::read_dir(package_root).map_err(|error| CampaignPackageError {
            stage: ValidationStage::DirectorySet,
            message: format!("cannot enumerate package: {error}"),
        })? {
            let entry = entry.map_err(|error| CampaignPackageError {
                stage: ValidationStage::DirectorySet,
                message: format!("cannot inspect package entry: {error}"),
            })?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| CampaignPackageError {
                    stage: ValidationStage::DirectorySet,
                    message: "package contains a non-UTF-8 member".to_owned(),
                })?;
            let file_type = entry.file_type().map_err(|error| CampaignPackageError {
                stage: ValidationStage::DirectorySet,
                message: format!("cannot inspect package member {name}: {error}"),
            })?;
            if !file_type.is_file() {
                return fail(
                    ValidationStage::DirectorySet,
                    format!("package member {name} is not a regular file"),
                );
            }
            actual_paths.insert(name);
        }
        if actual_paths != expected_paths.into_iter().map(str::to_owned).collect() {
            return fail(
                ValidationStage::DirectorySet,
                "package directory does not contain the exact member set",
            );
        }

        // Stage 3: every member and the sorted member identity.
        let mut members = BTreeMap::new();
        let mut identity_bytes = Vec::new();
        for &name in profile.members {
            let bytes =
                fs::read(package_root.join(name)).map_err(|error| CampaignPackageError {
                    stage: ValidationStage::MemberHashes,
                    message: format!("cannot read package member {name}: {error}"),
                })?;
            let actual_hash = sha256_hex(&bytes);
            let expected = &files[name];
            if u64::try_from(bytes.len()).expect("usize fits u64") != expected.0
                || actual_hash != expected.1
            {
                return fail(
                    ValidationStage::MemberHashes,
                    format!("package member {name} size/hash mismatch"),
                );
            }
            identity_bytes.extend_from_slice(name.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(actual_hash.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(bytes.len().to_string().as_bytes());
            identity_bytes.push(b'\n');
            members.insert(name.to_owned(), bytes);
        }
        if sha256_hex(&identity_bytes) != package_sha256 {
            return fail(
                ValidationStage::MemberHashes,
                "package_sha256 differs from sorted member identity",
            );
        }

        if profile.has_owner_manifest {
            let owner_export_package_sha256 = lower_sha256(
                &manifest["owner_export_package_sha256"],
                "manifest.owner_export_package_sha256",
                ValidationStage::OuterManifest,
            )?;
            validate_owner_manifest(
                &members["owner-manifest.json"],
                outer,
                &files,
                &owner_export_package_sha256,
            )?;
        }

        // Stage 4: canonical definition, source pin, and identity equality.
        let definition_bytes = &members["campaign-definition.json"];
        let definition_value: Value =
            serde_json::from_slice(definition_bytes).map_err(|error| CampaignPackageError {
                stage: ValidationStage::Definition,
                message: format!("cannot parse campaign-definition.json: {error}"),
            })?;
        let mut canonical =
            serde_json::to_vec(&definition_value).map_err(|error| CampaignPackageError {
                stage: ValidationStage::Definition,
                message: format!("cannot canonicalize campaign definition: {error}"),
            })?;
        canonical.push(b'\n');
        if &canonical != definition_bytes {
            return fail(
                ValidationStage::Definition,
                "campaign-definition.json is not canonical sorted UTF-8 JSON",
            );
        }
        let definition_fields: &[&str] = if profile.has_presentation_assets {
            &[
                "campaign_spec",
                "identity",
                "presentation_assets",
                "required_behaviors",
                "schema",
                "source",
                "source_facts",
            ]
        } else {
            &[
                "campaign_spec",
                "identity",
                "required_behaviors",
                "schema",
                "source",
                "source_facts",
            ]
        };
        let definition = exact_object(
            &definition_value,
            definition_fields,
            "campaign definition",
            ValidationStage::Definition,
        )?;
        require_string_eq(
            &definition["schema"],
            profile.definition_schema,
            "campaign definition.schema",
            ValidationStage::Definition,
        )?;
        let source = exact_object(
            &definition["source"],
            &["remc2_commit"],
            "campaign definition.source",
            ValidationStage::Definition,
        )?;
        require_string_eq(
            &source["remc2_commit"],
            LEVEL2_REMC2_COMMIT,
            "campaign definition.source.remc2_commit",
            ValidationStage::Definition,
        )?;
        let definition_identity = exact_object(
            &definition["identity"],
            &["external_level", "internal_level", "map_type_id"],
            "campaign definition.identity",
            ValidationStage::Definition,
        )?;
        let descriptor_selection = CampaignSelection {
            external_level: bounded_u16(
                &definition_identity["external_level"],
                "campaign definition.identity.external_level",
                ValidationStage::Definition,
            )?,
            internal_level: bounded_u16(
                &definition_identity["internal_level"],
                "campaign definition.identity.internal_level",
                ValidationStage::Definition,
            )?,
            map_type_id: bounded_u8(
                &definition_identity["map_type_id"],
                "campaign definition.identity.map_type_id",
                ValidationStage::Definition,
            )?,
        };
        if descriptor_selection != outer || descriptor_selection != caller {
            return fail(
                ValidationStage::Definition,
                "campaign definition/outer/caller identity mismatch",
            );
        }
        let spec_value = exact_object(
            &definition["campaign_spec"],
            &[
                "authored_behavior_rows",
                "authored_records",
                "initial_activation",
                "initial_rng",
                "objectives",
                "presentation_recipe",
                "reset_sources",
            ],
            "campaign_spec",
            ValidationStage::Definition,
        )?;
        let reset_sources = exact_object(
            &spec_value["reset_sources"],
            &[
                "entities",
                "feature_boundary",
                "players",
                "runtime",
                "terrain",
            ],
            "campaign_spec.reset_sources",
            ValidationStage::Definition,
        )?;

        // Stage 5: typed members and cross-member topology.
        let authored_records = parse_member_ref(
            &spec_value["authored_records"],
            &files,
            "campaign_spec.authored_records",
            Some("mc2-authored-level/v1"),
            Some(1_200),
            Some(20),
            ValidationStage::TypedMembers,
        )?;
        let entities_ref = parse_member_ref(
            &reset_sources["entities"],
            &files,
            "campaign_spec.reset_sources.entities",
            Some("mc2-entity-arena/v1"),
            Some(1_000),
            Some(176),
            ValidationStage::TypedMembers,
        )?;
        let players_ref = parse_member_ref(
            &reset_sources["players"],
            &files,
            "campaign_spec.reset_sources.players",
            Some(profile.player_schema),
            Some(8),
            Some(u64::try_from(profile.player_record_bytes).expect("player width fits u64")),
            ValidationStage::TypedMembers,
        )?;
        let terrain_ref = parse_member_ref(
            &reset_sources["terrain"],
            &files,
            "campaign_spec.reset_sources.terrain",
            Some("mc2-terrain/v1"),
            None,
            None,
            ValidationStage::TypedMembers,
        )?;
        let runtime_ref = parse_member_ref(
            &reset_sources["runtime"],
            &files,
            "campaign_spec.reset_sources.runtime",
            Some("mc2-runtime-state/v1"),
            None,
            None,
            ValidationStage::TypedMembers,
        )?;
        let authored = &members[&authored_records.member];
        let entities = &members[&entities_ref.member];
        let expected_rows = decode_behavior_rows(authored, entities, outer.map_type_id)?;
        let declared_rows = parse_behavior_rows(&spec_value["authored_behavior_rows"])?;
        if declared_rows != expected_rows {
            return fail(
                ValidationStage::TypedMembers,
                "authored_behavior_rows differ from fresh authored/reset decode",
            );
        }
        validate_entities(entities)?;
        let runtime_player_count = validate_runtime(&members["runtime.bin"])?;
        validate_players(
            &members[&players_ref.member],
            entities,
            usize::from(runtime_player_count),
            profile.player_version,
            profile.player_record_bytes,
        )?;
        let local_player_fireball_mana_cost =
            validate_local_player_fireball_binding(&members[&players_ref.member], entities)?;
        validate_terrain(&members["terrain.bin"], "terrain.bin")?;
        let feature_boundary = validate_feature_boundary(
            &reset_sources["feature_boundary"],
            &files,
            &members,
            profile.feature_allocator_bytes,
            profile.feature_allocator_trace_count,
        )?;
        let presentation_recipe =
            validate_presentation(&spec_value["presentation_recipe"], &files, &members, outer)?;
        let presentation_assets = validate_portable_presentation_assets(
            definition.get("presentation_assets"),
            &files,
            &members,
            profile.has_presentation_assets,
        )?;

        // Stage 6: SourceFacts.  Context contains no presentation bytes.
        let authored_collision_provenance = declared_rows
            .iter()
            .filter(|row| {
                row.initial_source_slot.is_none()
                    && row.class == 10
                    && row.action == 255
                    && row.subtype == 45
            })
            .map(|row| SelectorProvenance {
                authored_record: row.authored_record,
                source_class: row.class,
                source_model: row.model,
                source_subtype: row.subtype,
                source_field: SourceField::Par1_14,
                selector_value: u32::from(row.model),
            })
            .collect::<Vec<_>>();
        let building_objective_provenance = building_objective_provenance(
            profile.has_building_parameters,
            authored,
            &declared_rows,
        )?;
        let source_facts = validate_source_facts(
            &definition["source_facts"],
            &SourceFactContext {
                files: &files,
                members: &members,
                authored_collision_provenance: &authored_collision_provenance,
                building_objective_provenance: &building_objective_provenance,
                collision_bank: profile.has_collision_bank.then(|| {
                    let collision = &files["collision-masks.bin"];
                    (
                        usize::try_from(collision.0).expect("campaign member size fits usize"),
                        collision.1.as_str(),
                    )
                }),
                has_building_parameters: profile.has_building_parameters,
            },
        )
        .map_err(|message| CampaignPackageError {
            stage: ValidationStage::SourceFacts,
            message,
        })?;

        // Stage 7: the declared operations must match the authored stage row
        // that supplied the Stage-6 SourceFact provenance.
        let (initial_rng, objectives) =
            validate_operations(spec_value, authored, &files, &members)?;
        let building_parameters = profile
            .has_building_parameters
            .then(|| decode_building_parameters(&members["building-parameters.bin"]))
            .transpose()
            .map_err(|message| CampaignPackageError {
                stage: ValidationStage::TypedOperations,
                message,
            })?;
        let semantic_level = Mc2LevelDefinition::from_authored_bytes(
            authored,
            &objectives,
            descriptor_selection,
            building_parameters.as_ref(),
        )
        .map_err(|message| CampaignPackageError {
            stage: ValidationStage::TypedOperations,
            message: format!("cannot derive semantic level content: {message}"),
        })?;
        let creature_by_initial_source_slot = expected_rows
            .iter()
            .filter_map(|row| {
                Some((
                    row.initial_source_slot?,
                    Mc2LevelDefinition::creature_for_reset_selector(
                        row.class, row.model, row.action,
                    )?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let mechanic_by_initial_source_slot = expected_rows
            .iter()
            .filter_map(|row| {
                Some((
                    row.initial_source_slot?,
                    Mc2LevelDefinition::mechanic_for_reset_selector(
                        row.class, row.model, row.action,
                    )?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let mut mechanic_entity_by_initial_source_slot = BTreeMap::new();
        for row in &expected_rows {
            let (Some(source_slot), Some(kind)) = (
                row.initial_source_slot,
                Mc2LevelDefinition::mechanic_for_reset_selector(row.class, row.model, row.action),
            ) else {
                continue;
            };
            // Reset behavior rows use their own constructor provenance, not
            // the authored feature index. Join the private carrier to the
            // semantic package entity by kind and fail closed on ambiguity.
            let matching = semantic_level
                .mechanic_by_authored_record
                .iter()
                .filter(|(_, candidate_kind)| **candidate_kind == kind)
                .filter_map(|(authored_record, _)| {
                    semantic_level
                        .mechanic_entity_by_authored_record
                        .get(authored_record)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "initial mechanic source slot {source_slot} has {} semantic package candidates",
                        matching.len()
                    ),
                );
            }
            mechanic_entity_by_initial_source_slot.insert(source_slot, matching[0].clone());
        }
        let scenery_by_initial_source_slot = expected_rows
            .iter()
            .filter_map(|row| {
                Some((
                    row.initial_source_slot?,
                    Mc2LevelDefinition::scenery_for_reset_selector(
                        row.class, row.model, row.action,
                    )?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let building_by_initial_source_slot = expected_rows
            .iter()
            .filter_map(|row| {
                Some((
                    row.initial_source_slot?,
                    Mc2LevelDefinition::building_for_reset_selector(
                        row.class, row.model, row.action,
                    )?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let switch_by_initial_source_slot = expected_rows
            .iter()
            .filter_map(|row| {
                Some((
                    row.initial_source_slot?,
                    Mc2LevelDefinition::switch_for_reset_selector(
                        row.class, row.model, row.action,
                    )?,
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let mut movable_entity_bindings = BTreeMap::new();
        for (entity_id, hint) in &semantic_level.movable_runtime_hint_by_entity_id {
            let entity =
                semantic_level
                    .semantic
                    .entity(entity_id)
                    .ok_or_else(|| CampaignPackageError {
                        stage: ValidationStage::TypedOperations,
                        message: format!(
                            "movable semantic entity {} is absent from its level definition",
                            entity_id.as_str()
                        ),
                    })?;
            if let Mc2MovableRuntimeHint::AuthoredFeature(authored_record) = hint {
                let carrier_matches = match entity.kind() {
                    LevelEntityKind::Item(expected_kind) => {
                        semantic_level.item_by_authored_record.get(authored_record)
                            == Some(&expected_kind)
                    }
                    LevelEntityKind::Scenery(expected_kind) => {
                        semantic_level
                            .scenery_by_authored_record
                            .get(authored_record)
                            == Some(&expected_kind)
                    }
                    LevelEntityKind::SpellPickup(expected_kind) => {
                        semantic_level.spell_by_authored_record.get(authored_record)
                            == Some(&expected_kind)
                    }
                    LevelEntityKind::Switch(expected_kind) => {
                        semantic_level
                            .switch_by_authored_record
                            .get(authored_record)
                            == Some(&expected_kind)
                    }
                    _ => false,
                };
                if !carrier_matches {
                    return fail(
                        ValidationStage::TypedOperations,
                        format!(
                            "movable semantic entity {} authored-feature identity diverges",
                            entity_id.as_str()
                        ),
                    );
                }
                movable_entity_bindings.insert(
                    entity_id.clone(),
                    Mc2EntityRuntimeBinding::AuthoredFeature {
                        authored_record: *authored_record,
                        kind: entity.kind(),
                    },
                );
                continue;
            }
            let mut matching = Vec::new();
            for row in &expected_rows {
                let Some(source_slot) = row.initial_source_slot else {
                    continue;
                };
                let kind_matches = match entity.kind() {
                    LevelEntityKind::Switch(expected) => {
                        Mc2LevelDefinition::switch_for_reset_selector(
                            row.class, row.model, row.action,
                        ) == Some(expected)
                    }
                    LevelEntityKind::SpellPickup(expected) => {
                        Mc2LevelDefinition::spell_for_reset_selector(
                            row.class, row.model, row.action,
                        ) == Some(expected)
                    }
                    LevelEntityKind::Scenery(expected) => {
                        Mc2LevelDefinition::scenery_for_reset_selector(
                            row.class, row.model, row.action,
                        ) == Some(expected)
                    }
                    _ => false,
                };
                if !kind_matches {
                    continue;
                }
                let identity_matches = match hint {
                    Mc2MovableRuntimeHint::AuthoredRecord(authored_record) => {
                        row.authored_record == *authored_record
                    }
                    Mc2MovableRuntimeHint::AuthoredFeature(_) => unreachable!(
                        "authored-feature bindings are validated before frame-zero matching"
                    ),
                    Mc2MovableRuntimeHint::ResetKindAndPosition => {
                        initial_entity_tile(entities, source_slot)? == entity.position()
                    }
                };
                if identity_matches {
                    matching.push(row);
                }
            }
            if matching.len() != 1 {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "movable semantic entity {} has {} frame-zero MC2 carriers",
                        entity_id.as_str(),
                        matching.len()
                    ),
                );
            }
            let row = matching[0];
            movable_entity_bindings.insert(
                entity_id.clone(),
                Mc2EntityRuntimeBinding::InitialSource {
                    initial_source_slot: row
                        .initial_source_slot
                        .expect("movable carrier is frame-zero"),
                    kind: entity.kind(),
                    source_id: row.authored_record,
                },
            );
        }
        for scenery in scenery_by_initial_source_slot.values() {
            if !semantic_level.semantic.entities().iter().any(|entity| {
                entity.kind() == crate::level_definition::LevelEntityKind::Scenery(*scenery)
            }) {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "reset {} has no matching authored semantic scenery",
                        scenery.as_str()
                    ),
                );
            }
        }
        for building in building_by_initial_source_slot.values() {
            if !semantic_level.semantic.entities().iter().any(|entity| {
                entity.kind() == crate::level_definition::LevelEntityKind::Building(*building)
            }) {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "reset {} has no matching authored semantic building",
                        building.as_str()
                    ),
                );
            }
        }
        for mechanic in mechanic_by_initial_source_slot.values() {
            if !semantic_level.semantic.entities().iter().any(|entity| {
                entity.kind() == crate::level_definition::LevelEntityKind::Mechanic(*mechanic)
            }) {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "reset {} has no matching authored semantic mechanic",
                        mechanic.as_str()
                    ),
                );
            }
        }
        for switch in switch_by_initial_source_slot.values() {
            if !semantic_level.semantic.entities().iter().any(|entity| {
                entity.kind() == crate::level_definition::LevelEntityKind::Switch(*switch)
            }) {
                return fail(
                    ValidationStage::TypedOperations,
                    format!(
                        "reset {} has no matching authored semantic switch",
                        switch.as_str()
                    ),
                );
            }
        }

        // Stage 8: source-derived post-GenerateEvents runtime roots, exact
        // declaration, and compiled keys. Any later selector is independently
        // fail-closed by the source scheduler before callback or epilogue.
        let expected_requirements = expected_behavior_requirements(&declared_rows);
        let declared_requirements = parse_behavior_requirements(&definition["required_behaviors"])?;
        if declared_requirements != expected_requirements {
            return fail(
                ValidationStage::BehaviorClosure,
                "declared required behavior roots differ from the live reset selectors",
            );
        }
        for requirement in &declared_requirements {
            if !capabilities.contains(&requirement.key) {
                return fail(
                    ValidationStage::BehaviorClosure,
                    format!(
                        "compiled registry is missing capability {:?}",
                        requirement.key
                    ),
                );
            }
        }

        // Stage 9: structural reset receipt.  No live state or handle exists.
        let reset_names = [
            "entities.bin",
            "feature-end.bin",
            "feature-start.bin",
            "players.bin",
            "runtime.bin",
            "terrain.bin",
        ];
        let mut reset_identity = Vec::new();
        for name in reset_names {
            reset_identity.extend_from_slice(name.as_bytes());
            reset_identity.push(0);
            reset_identity.extend_from_slice(files[name].1.as_bytes());
            reset_identity.push(b'\n');
        }
        let reset_state_receipt = ResetStatePreflightReceipt {
            sha256: sha256_hex(&reset_identity),
            structurally_validated: true,
        };

        Ok(Self {
            package_root: package_root.to_path_buf(),
            package_sha256,
            identity: descriptor_selection,
            campaign_spec: CampaignSpec {
                authored_records,
                authored_behavior_rows: declared_rows,
                reset_sources: ResetSources {
                    terrain: terrain_ref,
                    entities: entities_ref,
                    players: players_ref,
                    runtime: runtime_ref,
                    feature_boundary,
                },
                initial_rng,
                initial_activation: Vec::new(),
                objectives,
                presentation_recipe,
            },
            presentation_assets,
            source_facts,
            required_behaviors: declared_requirements,
            reset_state_receipt,
            completed_stage: ValidationStage::ResetState,
            level_definition: semantic_level.semantic,
            creature_by_authored_record: semantic_level.creature_by_authored_record,
            creature_by_initial_source_slot,
            creature_model_by_kind: semantic_level.creature_model_by_kind,
            spell_by_authored_record: semantic_level.spell_by_authored_record,
            spell_family_by_kind: semantic_level.spell_family_by_kind,
            building_by_authored_record: semantic_level.building_by_authored_record,
            building_by_initial_source_slot,
            scenery_by_authored_record: semantic_level.scenery_by_authored_record,
            scenery_by_initial_source_slot,
            item_by_authored_record: semantic_level.item_by_authored_record,
            switch_by_authored_record: semantic_level.switch_by_authored_record,
            switch_by_initial_source_slot,
            mechanic_by_authored_record: semantic_level.mechanic_by_authored_record,
            mechanic_by_initial_source_slot,
            mechanic_entity_by_authored_record: semantic_level.mechanic_entity_by_authored_record,
            standing_building_chains: semantic_level.standing_building_chains,
            rival_wizard_by_id: semantic_level.rival_wizard_by_id,
            mechanic_entity_by_initial_source_slot,
            movable_entity_bindings,
            local_player_fireball_mana_cost,
            member_hashes: files
                .into_iter()
                .map(|(name, (_, sha256))| (name, sha256))
                .collect(),
        })
    }

    pub fn required_behavior_keys(&self) -> impl Iterator<Item = &BehaviorKey> {
        self.required_behaviors
            .iter()
            .map(|requirement| &requirement.key)
    }

    #[must_use]
    pub const fn level_definition(&self) -> &LevelDefinition {
        &self.level_definition
    }

    pub(crate) fn presentation_assets(&self) -> &[PortablePresentationAsset] {
        &self.presentation_assets
    }

    pub(crate) fn creature_bindings(&self) -> &BTreeMap<u16, CreatureKind> {
        &self.creature_by_authored_record
    }

    pub(crate) fn initial_creature_bindings(&self) -> &BTreeMap<u16, CreatureKind> {
        &self.creature_by_initial_source_slot
    }

    pub(crate) fn creature_models(&self) -> &BTreeMap<CreatureKind, u8> {
        &self.creature_model_by_kind
    }

    pub(crate) fn spell_bindings(&self) -> &BTreeMap<u16, SpellKind> {
        &self.spell_by_authored_record
    }

    pub(crate) fn spell_families(&self) -> &BTreeMap<SpellKind, u8> {
        &self.spell_family_by_kind
    }

    pub(crate) fn building_bindings(&self) -> &BTreeMap<u16, BuildingKind> {
        &self.building_by_authored_record
    }

    pub(crate) fn initial_building_bindings(&self) -> &BTreeMap<u16, BuildingKind> {
        &self.building_by_initial_source_slot
    }

    pub(crate) fn scenery_bindings(&self) -> &BTreeMap<u16, SceneryKind> {
        &self.scenery_by_authored_record
    }

    pub(crate) fn initial_scenery_bindings(&self) -> &BTreeMap<u16, SceneryKind> {
        &self.scenery_by_initial_source_slot
    }

    pub(crate) fn item_bindings(&self) -> &BTreeMap<u16, ItemKind> {
        &self.item_by_authored_record
    }

    pub(crate) fn switch_bindings(&self) -> &BTreeMap<u16, SwitchKind> {
        &self.switch_by_authored_record
    }

    pub(crate) fn initial_switch_bindings(&self) -> &BTreeMap<u16, SwitchKind> {
        &self.switch_by_initial_source_slot
    }

    pub(crate) fn mechanic_bindings(&self) -> &BTreeMap<u16, MechanicKind> {
        &self.mechanic_by_authored_record
    }

    pub(crate) fn initial_mechanic_bindings(&self) -> &BTreeMap<u16, MechanicKind> {
        &self.mechanic_by_initial_source_slot
    }

    pub(crate) fn mechanic_entity_bindings(&self) -> &BTreeMap<u16, LevelEntityId> {
        &self.mechanic_entity_by_authored_record
    }

    pub(crate) fn initial_mechanic_entity_bindings(&self) -> &BTreeMap<u16, LevelEntityId> {
        &self.mechanic_entity_by_initial_source_slot
    }

    pub(crate) fn standing_building_chains(&self) -> &BTreeMap<LevelObjectiveId, Box<[u8]>> {
        &self.standing_building_chains
    }

    pub(crate) fn rival_wizard_bindings(&self) -> &BTreeMap<RivalWizardId, u16> {
        &self.rival_wizard_by_id
    }

    pub(crate) fn movable_entity_binding(
        &self,
        entity_id: &LevelEntityId,
    ) -> Option<&Mc2EntityRuntimeBinding> {
        self.movable_entity_bindings.get(entity_id)
    }

    /// Package-present local-player family-zero Fireball cost. The private
    /// holder relation and source slot terminate in this adapter.
    #[must_use]
    pub(crate) const fn local_player_fireball_mana_cost(&self) -> Option<i32> {
        self.local_player_fireball_mana_cost
    }

    /// Reconstruct the identity of the wrapped owner-level export.
    ///
    /// Campaign-only definition/compatibility-manifest members are not part
    /// of the wrapped owner export. Owner caches remain bound to the original
    /// owner-member identity, so consumers must keep the two identities
    /// distinct.
    pub(crate) fn owner_export_package_sha256(&self) -> Result<String> {
        let mut identity = Vec::new();
        for (name, expected_sha256) in &self.member_hashes {
            if matches!(
                name.as_str(),
                "building-parameters.bin"
                    | "campaign-definition.json"
                    | "fireball-projectile-base.indexed-sprite.json"
                    | "fireball-projectile-fade.indexed-sprite.json"
                    | "owner-manifest.json"
            ) {
                continue;
            }
            let bytes = self.read_member(name)?;
            identity.extend_from_slice(name.as_bytes());
            identity.push(0);
            identity.extend_from_slice(expected_sha256.as_bytes());
            identity.push(0);
            identity.extend_from_slice(bytes.len().to_string().as_bytes());
            identity.push(b'\n');
        }
        Ok(sha256_hex(&identity))
    }

    /// Re-read one validated package member for a session consumer.
    ///
    /// Preflight validates the complete package before a session can exist,
    /// but the decoder still has to read the bytes afterwards. Rechecking the
    /// descriptor-bound hash here closes that read boundary instead of
    /// trusting a path whose contents may have changed after preflight.
    pub(crate) fn read_member(&self, name: &str) -> Result<Vec<u8>> {
        let Some(expected_sha256) = self.member_hashes.get(name) else {
            return fail(
                ValidationStage::MemberHashes,
                format!("campaign definition does not declare member {name}"),
            );
        };
        let bytes =
            fs::read(self.package_root.join(name)).map_err(|error| CampaignPackageError {
                stage: ValidationStage::MemberHashes,
                message: format!("cannot re-read {name}: {error}"),
            })?;
        let actual_sha256 = sha256_hex(&bytes);
        if &actual_sha256 != expected_sha256 {
            return fail(
                ValidationStage::MemberHashes,
                format!(
                    "campaign member {name} changed after preflight: expected {expected_sha256}, got {actual_sha256}"
                ),
            );
        }
        Ok(bytes)
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_owner_manifest(
    bytes: &[u8],
    selection: CampaignSelection,
    campaign_files: &BTreeMap<String, (u64, String)>,
    expected_owner_package_sha256: &str,
) -> Result<()> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| CampaignPackageError {
        stage: ValidationStage::Definition,
        message: format!("cannot parse owner-manifest.json: {error}"),
    })?;
    let manifest = value.as_object().ok_or_else(|| CampaignPackageError {
        stage: ValidationStage::Definition,
        message: "owner-manifest.json is not an object".to_owned(),
    })?;
    require_string_eq(
        manifest.get("schema").unwrap_or(&Value::Null),
        "mc2-owner-level-export/v3",
        "owner manifest.schema",
        ValidationStage::Definition,
    )?;
    require_string_eq(
        manifest.get("byte_order").unwrap_or(&Value::Null),
        "little-endian",
        "owner manifest.byte_order",
        ValidationStage::Definition,
    )?;
    let level = manifest
        .get("level")
        .and_then(Value::as_object)
        .ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::Definition,
            message: "owner manifest.level is not an object".to_owned(),
        })?;
    let map_type = level
        .get("map_type")
        .and_then(Value::as_object)
        .ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::Definition,
            message: "owner manifest.level.map_type is not an object".to_owned(),
        })?;
    if bounded_u16(
        level.get("external").unwrap_or(&Value::Null),
        "owner manifest.level.external",
        ValidationStage::Definition,
    )? != selection.external_level
        || bounded_u16(
            level.get("internal").unwrap_or(&Value::Null),
            "owner manifest.level.internal",
            ValidationStage::Definition,
        )? != selection.internal_level
        || bounded_u8(
            map_type.get("id").unwrap_or(&Value::Null),
            "owner manifest.level.map_type.id",
            ValidationStage::Definition,
        )? != selection.map_type_id
        || string(
            map_type.get("name").unwrap_or(&Value::Null),
            "owner manifest.level.map_type.name",
            ValidationStage::Definition,
        )? != campaign_map_type_name(selection.map_type_id).expect("profile map type")
    {
        return fail(
            ValidationStage::Definition,
            "owner manifest identity differs from the campaign selection",
        );
    }

    let owner_files = manifest
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::Definition,
            message: "owner manifest.files is not an object".to_owned(),
        })?;
    let expected_names = campaign_files
        .keys()
        .filter(|name| {
            !matches!(
                name.as_str(),
                "campaign-definition.json"
                    | "fireball-projectile-base.indexed-sprite.json"
                    | "fireball-projectile-fade.indexed-sprite.json"
                    | "owner-manifest.json"
            )
        })
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual_names = owner_files
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return fail(
            ValidationStage::Definition,
            "owner manifest file set differs from the wrapped owner export",
        );
    }
    let mut identity = Vec::new();
    for name in expected_names {
        let metadata = exact_object(
            &owner_files[name],
            &["bytes", "sha256"],
            &format!("owner manifest.files.{name}"),
            ValidationStage::Definition,
        )?;
        let declared_bytes = unsigned(
            &metadata["bytes"],
            &format!("owner manifest.files.{name}.bytes"),
            ValidationStage::Definition,
        )?;
        let declared_sha256 = lower_sha256(
            &metadata["sha256"],
            &format!("owner manifest.files.{name}.sha256"),
            ValidationStage::Definition,
        )?;
        if campaign_files[name] != (declared_bytes, declared_sha256.clone()) {
            return fail(
                ValidationStage::Definition,
                format!("owner manifest member {name} differs from the campaign envelope"),
            );
        }
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(declared_sha256.as_bytes());
        identity.push(0);
        identity.extend_from_slice(declared_bytes.to_string().as_bytes());
        identity.push(b'\n');
    }
    let owner_package_sha256 = lower_sha256(
        manifest.get("package_sha256").unwrap_or(&Value::Null),
        "owner manifest.package_sha256",
        ValidationStage::Definition,
    )?;
    if owner_package_sha256 != sha256_hex(&identity)
        || owner_package_sha256 != expected_owner_package_sha256
    {
        return fail(
            ValidationStage::Definition,
            "owner manifest package identity differs from its bound owner members",
        );
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    label: &str,
    stage: ValidationStage,
) -> Result<&'a serde_json::Map<String, Value>> {
    let Some(object) = value.as_object() else {
        return fail(stage, format!("{label} is not an object"));
    };
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return fail(stage, format!("{label} field set differs"));
    }
    Ok(object)
}

fn string<'a>(value: &'a Value, label: &str, stage: ValidationStage) -> Result<&'a str> {
    value.as_str().ok_or_else(|| CampaignPackageError {
        stage,
        message: format!("{label} is not a string"),
    })
}

fn require_string_eq(
    value: &Value,
    expected: &str,
    label: &str,
    stage: ValidationStage,
) -> Result<()> {
    if string(value, label, stage)? != expected {
        return fail(stage, format!("{label} differs"));
    }
    Ok(())
}

fn unsigned(value: &Value, label: &str, stage: ValidationStage) -> Result<u64> {
    value.as_u64().ok_or_else(|| CampaignPackageError {
        stage,
        message: format!("{label} is not an unsigned integer"),
    })
}

fn bounded_u16(value: &Value, label: &str, stage: ValidationStage) -> Result<u16> {
    u16::try_from(unsigned(value, label, stage)?).map_err(|_| CampaignPackageError {
        stage,
        message: format!("{label} exceeds u16"),
    })
}

fn bounded_u8(value: &Value, label: &str, stage: ValidationStage) -> Result<u8> {
    u8::try_from(unsigned(value, label, stage)?).map_err(|_| CampaignPackageError {
        stage,
        message: format!("{label} exceeds u8"),
    })
}

fn signed_i16(value: &Value, label: &str, stage: ValidationStage) -> Result<i16> {
    let Some(value) = value.as_i64() else {
        return fail(stage, format!("{label} is not a signed integer"));
    };
    i16::try_from(value).map_err(|_| CampaignPackageError {
        stage,
        message: format!("{label} exceeds i16"),
    })
}

fn signed_i8(value: &Value, label: &str, stage: ValidationStage) -> Result<i8> {
    let Some(value) = value.as_i64() else {
        return fail(stage, format!("{label} is not a signed integer"));
    };
    i8::try_from(value).map_err(|_| CampaignPackageError {
        stage,
        message: format!("{label} exceeds i8"),
    })
}

fn lower_sha256(value: &Value, label: &str, stage: ValidationStage) -> Result<String> {
    let value = string(value, label, stage)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return fail(stage, format!("{label} is not a lowercase SHA-256"));
    }
    Ok(value.to_owned())
}

fn normalized_member_name(name: &str, stage: ValidationStage) -> Result<()> {
    if name.is_empty()
        || !name.is_ascii()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return fail(stage, format!("member name {name:?} is not normalized"));
    }
    Ok(())
}

fn parse_member_ref(
    value: &Value,
    files: &BTreeMap<String, (u64, String)>,
    label: &str,
    expected_schema: Option<&str>,
    expected_count: Option<u64>,
    expected_record_bytes: Option<u64>,
    stage: ValidationStage,
) -> Result<MemberRef> {
    let mut keys = vec!["member", "sha256"];
    if expected_schema.is_some() {
        keys.push("schema");
    }
    if expected_count.is_some() {
        keys.push("count");
    }
    if expected_record_bytes.is_some() {
        keys.push("record_bytes");
    }
    let object = exact_object(value, &keys, label, stage)?;
    let member = string(&object["member"], &format!("{label}.member"), stage)?.to_owned();
    normalized_member_name(&member, stage)?;
    let sha256 = lower_sha256(&object["sha256"], &format!("{label}.sha256"), stage)?;
    let Some(outer) = files.get(&member) else {
        return fail(
            stage,
            format!("{label} references undeclared member {member}"),
        );
    };
    if sha256 != outer.1 {
        return fail(stage, format!("{label} hash differs from outer file map"));
    }
    let schema = expected_schema.map(str::to_owned);
    if let Some(expected) = expected_schema {
        require_string_eq(
            &object["schema"],
            expected,
            &format!("{label}.schema"),
            stage,
        )?;
    }
    let count = expected_count;
    if let Some(expected) = expected_count
        && unsigned(&object["count"], &format!("{label}.count"), stage)? != expected
    {
        return fail(stage, format!("{label} count differs"));
    }
    let record_bytes = expected_record_bytes;
    if let Some(expected) = expected_record_bytes
        && unsigned(
            &object["record_bytes"],
            &format!("{label}.record_bytes"),
            stage,
        )? != expected
    {
        return fail(stage, format!("{label} record width differs"));
    }
    Ok(MemberRef {
        member,
        sha256,
        schema,
        count,
        record_bytes,
    })
}

fn read_u8(bytes: &[u8], offset: usize, label: &str, stage: ValidationStage) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| CampaignPackageError {
            stage,
            message: format!("{label} is truncated"),
        })
}

fn read_u16(bytes: &[u8], offset: usize, label: &str, stage: ValidationStage) -> Result<u16> {
    let Some(pair) = bytes.get(offset..offset + 2) else {
        return fail(stage, format!("{label} is truncated"));
    };
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_i16(bytes: &[u8], offset: usize, label: &str, stage: ValidationStage) -> Result<i16> {
    Ok(read_u16(bytes, offset, label, stage)?.cast_signed())
}

fn read_u32(bytes: &[u8], offset: usize, label: &str, stage: ValidationStage) -> Result<u32> {
    let Some(value) = bytes.get(offset..offset + 4) else {
        return fail(stage, format!("{label} is truncated"));
    };
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn initial_entity_tile(bytes: &[u8], source_slot: u16) -> Result<TilePosition> {
    let offset = 12 + usize::from(source_slot) * ENTITY_RECORD_BYTES;
    Ok(TilePosition {
        x: read_u16(
            bytes,
            offset + 80,
            "entity x",
            ValidationStage::TypedMembers,
        )? >> 8,
        y: read_u16(
            bytes,
            offset + 82,
            "entity y",
            ValidationStage::TypedMembers,
        )? >> 8,
    })
}

fn decode_behavior_rows(
    authored: &[u8],
    entities: &[u8],
    expected_map_type_id: u8,
) -> Result<Vec<AuthoredBehaviorRow>> {
    if authored.len() != 0x6604 || authored.get(6) != Some(&expected_map_type_id) {
        return fail(
            ValidationStage::TypedMembers,
            "authored-level.bin extent/map identity changed",
        );
    }
    let mut rows = Vec::new();
    for index in 0..AUTHORED_RECORD_COUNT {
        let offset = AUTHORED_RECORD_OFFSET + index * AUTHORED_RECORD_BYTES;
        let record = &authored[offset..offset + AUTHORED_RECORD_BYTES];
        if record.iter().all(|byte| *byte == 0) {
            continue;
        }
        let class = read_u16(
            authored,
            offset,
            "authored class",
            ValidationStage::TypedMembers,
        )?;
        let subtype = read_i16(
            authored,
            offset + 2,
            "authored subtype",
            ValidationStage::TypedMembers,
        )?;
        let action = read_i16(
            authored,
            offset + 8,
            "authored action",
            ValidationStage::TypedMembers,
        )?;
        let model = read_u16(
            authored,
            offset + 14,
            "authored model",
            ValidationStage::TypedMembers,
        )?;
        if class > u16::from(u8::MAX)
            || !(0..=255).contains(&subtype)
            || model > u16::from(u8::MAX)
            || !(-1..=255).contains(&action)
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("authored behavior row {index} exceeds schema-v1 widths"),
            );
        }
        rows.push(AuthoredBehaviorRow {
            authored_record: u16::try_from(index).expect("1,200 rows fit u16"),
            initial_source_slot: None,
            class: u8::try_from(class).expect("bounded class"),
            action: if action == -1 {
                u8::MAX
            } else {
                u8::try_from(action).expect("bounded action")
            },
            model: u8::try_from(model).expect("bounded model"),
            subtype: u8::try_from(subtype).expect("bounded subtype"),
            initial_dispatch_sites: if action == -1 { vec![4] } else { vec![1, 2] },
        });
    }
    validate_entity_header(entities, *b"MCE1", "entities.bin")?;
    for slot in 0..ENTITY_COUNT {
        let offset = 12 + slot * ENTITY_RECORD_BYTES;
        let active = read_u8(
            entities,
            offset + 2,
            "entity active flag",
            ValidationStage::TypedMembers,
        )?;
        if active == 0 {
            continue;
        }
        rows.push(AuthoredBehaviorRow {
            authored_record: read_u16(
                entities,
                offset + 30,
                "entity source id",
                ValidationStage::TypedMembers,
            )?,
            initial_source_slot: Some(u16::try_from(slot).expect("1,000 slots fit u16")),
            class: entities[offset + 67],
            action: entities[offset + 73],
            model: entities[offset + 68],
            subtype: entities[offset + 70],
            initial_dispatch_sites: vec![1, 2, 3],
        });
    }
    rows.sort_by_key(|row| {
        (
            row.authored_record,
            row.initial_source_slot.is_some(),
            row.initial_source_slot.unwrap_or_default(),
        )
    });
    Ok(rows)
}

fn parse_behavior_rows(value: &Value) -> Result<Vec<AuthoredBehaviorRow>> {
    let Some(rows) = value.as_array() else {
        return fail(
            ValidationStage::TypedMembers,
            "authored_behavior_rows is not an array",
        );
    };
    rows.iter()
        .enumerate()
        .map(|(index, value)| {
            let label = format!("authored_behavior_rows[{index}]");
            let object = exact_object(
                value,
                &[
                    "action",
                    "authored_record",
                    "class",
                    "initial_dispatch_sites",
                    "initial_source_slot",
                    "model",
                    "subtype",
                ],
                &label,
                ValidationStage::TypedMembers,
            )?;
            let sites_value = object["initial_dispatch_sites"].as_array().ok_or_else(|| {
                CampaignPackageError {
                    stage: ValidationStage::TypedMembers,
                    message: format!("{label}.initial_dispatch_sites is not an array"),
                }
            })?;
            let sites = sites_value
                .iter()
                .enumerate()
                .map(|(site, value)| {
                    bounded_u16(
                        value,
                        &format!("{label}.initial_dispatch_sites[{site}]"),
                        ValidationStage::TypedMembers,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            if !sites.windows(2).all(|pair| pair[0] < pair[1])
                || sites.iter().any(|site| !(1..=4).contains(site))
            {
                return fail(
                    ValidationStage::TypedMembers,
                    format!("{label}.initial_dispatch_sites is not sorted/compiled"),
                );
            }
            let initial_source_slot = if object["initial_source_slot"].is_null() {
                None
            } else {
                Some(bounded_u16(
                    &object["initial_source_slot"],
                    &format!("{label}.initial_source_slot"),
                    ValidationStage::TypedMembers,
                )?)
            };
            Ok(AuthoredBehaviorRow {
                authored_record: bounded_u16(
                    &object["authored_record"],
                    &format!("{label}.authored_record"),
                    ValidationStage::TypedMembers,
                )?,
                initial_source_slot,
                class: bounded_u8(
                    &object["class"],
                    &format!("{label}.class"),
                    ValidationStage::TypedMembers,
                )?,
                action: bounded_u8(
                    &object["action"],
                    &format!("{label}.action"),
                    ValidationStage::TypedMembers,
                )?,
                model: bounded_u8(
                    &object["model"],
                    &format!("{label}.model"),
                    ValidationStage::TypedMembers,
                )?,
                subtype: bounded_u8(
                    &object["subtype"],
                    &format!("{label}.subtype"),
                    ValidationStage::TypedMembers,
                )?,
                initial_dispatch_sites: sites,
            })
        })
        .collect()
}

fn validate_entity_header(bytes: &[u8], magic: [u8; 4], label: &str) -> Result<()> {
    if bytes.len() != 12 + ENTITY_COUNT * ENTITY_RECORD_BYTES
        || bytes.get(..4) != Some(magic.as_slice())
        || read_u16(bytes, 4, label, ValidationStage::TypedMembers)? != 1
        || read_u16(bytes, 6, label, ValidationStage::TypedMembers)? != 176
        || read_u32(bytes, 8, label, ValidationStage::TypedMembers)? != 1_000
    {
        return fail(
            ValidationStage::TypedMembers,
            format!("{label} header/extent changed"),
        );
    }
    Ok(())
}

fn validate_entities(bytes: &[u8]) -> Result<()> {
    validate_entity_header(bytes, *b"MCE1", "entities.bin")?;
    let mut active_count = 0_usize;
    for index in 0..ENTITY_COUNT {
        let offset = 12 + index * ENTITY_RECORD_BYTES;
        if usize::from(read_u16(
            bytes,
            offset,
            "entity index",
            ValidationStage::TypedMembers,
        )?) != index
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("entity slot/index mismatch at {index}"),
            );
        }
        let active = bytes[offset + 2];
        if active > 1 || (active == 0 && bytes[offset + 67] != 0) {
            return fail(
                ValidationStage::TypedMembers,
                format!("entity {index} active/class invariant changed"),
            );
        }
        active_count += usize::from(active);
        if active == 1
            && usize::from(read_u16(
                bytes,
                offset + 30,
                "entity source id",
                ValidationStage::TypedMembers,
            )?) >= AUTHORED_RECORD_COUNT
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("entity {index} source relation is out of bounds"),
            );
        }
        if read_u32(
            bytes,
            offset + 4,
            "entity next",
            ValidationStage::TypedMembers,
        )? > 1_000
            || read_u32(
                bytes,
                offset + 164,
                "entity parameter",
                ValidationStage::TypedMembers,
            )? > 157
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("entity {index} relation is out of bounds"),
            );
        }
        let domain = bytes[offset + 168];
        let relation = read_u32(
            bytes,
            offset + 172,
            "entity special relation",
            ValidationStage::TypedMembers,
        )?;
        if !matches!((domain, relation), (0, 0) | (1, 1..=8) | (2, 1)) {
            return fail(
                ValidationStage::TypedMembers,
                format!("entity {index} special relation is invalid"),
            );
        }
    }
    if active_count == 0 {
        return fail(
            ValidationStage::TypedMembers,
            "campaign reset contains no active entities",
        );
    }
    Ok(())
}

fn validate_players(
    bytes: &[u8],
    entities: &[u8],
    bound_player_slots: usize,
    expected_version: u16,
    player_record_bytes: usize,
) -> Result<()> {
    if bytes.len() != 12 + PLAYER_COUNT * player_record_bytes
        || bytes.get(..4) != Some(b"MCP1")
        || read_u16(bytes, 4, "players version", ValidationStage::TypedMembers)? != expected_version
        || usize::from(read_u16(
            bytes,
            6,
            "players width",
            ValidationStage::TypedMembers,
        )?) != player_record_bytes
        || read_u32(bytes, 8, "players count", ValidationStage::TypedMembers)? != 8
    {
        return fail(
            ValidationStage::TypedMembers,
            "players.bin header/extent changed",
        );
    }
    for slot in 0..PLAYER_COUNT {
        let offset = 12 + slot * player_record_bytes;
        if usize::from(read_u16(
            bytes,
            offset,
            "player index",
            ValidationStage::TypedMembers,
        )?) != slot
            || bytes[offset + 2] > 1
            || bytes[offset + 3] != 0
            || bytes[offset + 50..offset + 54] != [0; 4]
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("player {slot} index/flags/host normalization changed"),
            );
        }
        let entity = usize::from(read_u16(
            bytes,
            offset + 4,
            "player entity relation",
            ValidationStage::TypedMembers,
        )?);
        if entity >= ENTITY_COUNT
            || (slot < bound_player_slots && entities[12 + entity * ENTITY_RECORD_BYTES + 2] != 1)
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("player {slot} entity relation is not active/bounded"),
            );
        }
    }
    Ok(())
}

fn validate_local_player_fireball_binding(players: &[u8], entities: &[u8]) -> Result<Option<i32>> {
    let player_offset = 12;
    let player_entity = read_u16(
        players,
        player_offset + 4,
        "local player entity",
        ValidationStage::TypedMembers,
    )?;
    let holder = read_u16(
        players,
        player_offset + 268,
        "local player Fireball holder",
        ValidationStage::TypedMembers,
    )?;
    if holder == 0 {
        return Ok(None);
    }
    let holder_index = usize::from(holder);
    if holder_index >= ENTITY_COUNT {
        return fail(
            ValidationStage::TypedMembers,
            "local player Fireball holder relation is out of bounds",
        );
    }
    let offset = 12 + holder_index * ENTITY_RECORD_BYTES;
    let parent = read_u16(
        entities,
        offset + 44,
        "local player Fireball holder parent",
        ValidationStage::TypedMembers,
    )?;
    if entities[offset + 2] != 1
        || entities[offset + 67] != 15
        || entities[offset + 68] != 0
        || entities[offset + 73] != 0
        || parent != player_entity
    {
        return fail(
            ValidationStage::TypedMembers,
            "local player Fireball holder identity/parent relation changed",
        );
    }
    let mana_cost = i32::from_le_bytes(
        entities[offset + 144..offset + 148]
            .try_into()
            .expect("validated entity record extent"),
    );
    if mana_cost != 100 {
        return fail(
            ValidationStage::TypedMembers,
            "local player Fireball holder package default changed from 100 mana units",
        );
    }
    Ok(Some(mana_cost))
}

fn validate_runtime(bytes: &[u8]) -> Result<u16> {
    let player_count = read_u16(
        bytes,
        18,
        "runtime player count",
        ValidationStage::TypedMembers,
    )?;
    if bytes.len() != 2_754
        || bytes.get(..4) != Some(b"MCR1")
        || read_u16(bytes, 4, "runtime version", ValidationStage::TypedMembers)? != 1
        || read_u16(bytes, 6, "runtime reserved", ValidationStage::TypedMembers)? != 0
        || read_u16(
            bytes,
            16,
            "runtime local player",
            ValidationStage::TypedMembers,
        )? != 0
        || player_count == 0
        || usize::from(player_count) > PLAYER_COUNT
    {
        return fail(
            ValidationStage::TypedMembers,
            "runtime.bin header/census profile scalars changed",
        );
    }
    for index in 0..8 {
        let offset = 192 + index * 14;
        let stage_type = bytes[offset];
        let flags = bytes[offset + 1];
        if !matches!(stage_type, 0 | 3 | 5 | 7 | 9) {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime stage {index} has an unproven type"),
            );
        }
        if bytes[offset + 7..offset + 10] != [0; 3] {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime stage {index} relation padding is nonzero"),
            );
        }
        let domain = bytes[offset + 6];
        let relation = read_u32(
            bytes,
            offset + 10,
            "runtime stage relation",
            ValidationStage::TypedMembers,
        )?;
        let expected_domain = u8::from(stage_type == 3 && flags & 1 != 0) * 2;
        if domain != expected_domain || (domain == 2 && !(1..=1_000).contains(&relation)) {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime stage {index} relation is invalid"),
            );
        }
    }
    for index in 0..8 {
        if bytes[304 + index * 11] != 0 {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime objective {index} has an unproven type"),
            );
        }
    }
    for index in 0..11 {
        let offset = 392 + index * 12;
        let variable_type = bytes[offset];
        let flags = bytes[offset + 1];
        let pointer_payload = (matches!(variable_type, 3..=5 | 8..=9)) && flags & 2 == 0;
        let expected_domain = u8::from(pointer_payload);
        if bytes[offset + 5..offset + 8] != [0; 3] {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime stage variable {index} relation padding is nonzero"),
            );
        }
        let domain = bytes[offset + 4];
        let relation = read_u32(
            bytes,
            offset + 8,
            "runtime stage-variable relation",
            ValidationStage::TypedMembers,
        )?;
        if domain != expected_domain || (domain == 1 && !(1..=1_000).contains(&relation)) {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime stage variable {index} relation is invalid"),
            );
        }
    }
    for index in 0..50 {
        let offset = 524 + index * 43;
        if bytes[offset + 11..offset + 14] != [0; 3] {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime event {index} relation padding is nonzero"),
            );
        }
        let domain = bytes[offset + 10];
        let relation = read_u32(
            bytes,
            offset + 14,
            "runtime event relation",
            ValidationStage::TypedMembers,
        )?;
        if domain != u8::from(bytes[offset] != 0)
            || (domain == 0 && relation != 0)
            || (domain == 1 && !(1..=1_000).contains(&relation))
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("runtime event {index} relation is invalid"),
            );
        }
    }
    Ok(player_count)
}

fn validate_terrain(bytes: &[u8], label: &str) -> Result<()> {
    if bytes.len() != 0x70000 || bytes[0x40000..0x50000].iter().any(|byte| *byte != 0) {
        return fail(
            ValidationStage::TypedMembers,
            format!("{label} extent/Day inactive plane changed"),
        );
    }
    Ok(())
}

fn validate_feature_boundary(
    value: &Value,
    files: &BTreeMap<String, (u64, String)>,
    members: &BTreeMap<String, Vec<u8>>,
    expected_allocator_bytes: usize,
    expected_allocator_trace_count: u32,
) -> Result<Vec<MemberRef>> {
    let expected = [
        ("feature-start.bin", "mc2-feature-boundary/v1"),
        ("terrain-pre-features.bin", "mc2-feature-terrain/v1"),
        ("feature-allocator.bin", "mc2-feature-allocator/v1"),
        (
            "feature-construction-inputs.bin",
            "mc2-feature-construction-inputs/v1",
        ),
        ("entities-post-features.bin", "mc2-feature-entity-arena/v1"),
        ("terrain-post-features.bin", "mc2-feature-terrain/v1"),
        ("feature-end.bin", "mc2-feature-boundary/v1"),
    ];
    let Some(rows) = value.as_array() else {
        return fail(
            ValidationStage::TypedMembers,
            "feature_boundary is not an array",
        );
    };
    if rows.len() != expected.len() {
        return fail(
            ValidationStage::TypedMembers,
            "feature_boundary member count differs",
        );
    }
    let mut references = Vec::with_capacity(expected.len());
    for (index, (value, (name, schema))) in rows.iter().zip(expected).enumerate() {
        let reference = parse_member_ref(
            value,
            files,
            &format!("feature_boundary[{index}]"),
            Some(schema),
            None,
            None,
            ValidationStage::TypedMembers,
        )?;
        if reference.member != name {
            return fail(
                ValidationStage::TypedMembers,
                format!("feature_boundary[{index}] order differs"),
            );
        }
        references.push(reference);
    }
    for (name, magic) in [("feature-start.bin", b"MFS3"), ("feature-end.bin", b"MFE3")] {
        let bytes = &members[name];
        if bytes.len() != 16
            || bytes.get(..4) != Some(magic)
            || read_u16(bytes, 4, name, ValidationStage::TypedMembers)? != 1
            || read_u16(bytes, 6, name, ValidationStage::TypedMembers)? != 16
            || read_u16(bytes, 10, name, ValidationStage::TypedMembers)? != 0
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("{name} layout changed"),
            );
        }
    }
    validate_terrain(
        &members["terrain-pre-features.bin"],
        "terrain-pre-features.bin",
    )?;
    validate_terrain(
        &members["terrain-post-features.bin"],
        "terrain-post-features.bin",
    )?;
    validate_entity_header(
        &members["entities-post-features.bin"],
        *b"MCE3",
        "entities-post-features.bin",
    )?;
    let allocator = &members["feature-allocator.bin"];
    if allocator.len() != expected_allocator_bytes
        || allocator.get(..4) != Some(b"MCA3")
        || read_u16(
            allocator,
            4,
            "feature allocator version",
            ValidationStage::TypedMembers,
        )? != 1
        || read_u16(
            allocator,
            6,
            "feature allocator header",
            ValidationStage::TypedMembers,
        )? != 32
        || read_u32(
            allocator,
            20,
            "feature allocator reserved",
            ValidationStage::TypedMembers,
        )? != 0
        || read_u32(
            allocator,
            24,
            "feature allocator trace count",
            ValidationStage::TypedMembers,
        )? != expected_allocator_trace_count
        || read_u16(
            allocator,
            28,
            "feature allocator relation width",
            ValidationStage::TypedMembers,
        )? != 2
        || read_u16(
            allocator,
            30,
            "feature allocator trace width",
            ValidationStage::TypedMembers,
        )? != 8
    {
        return fail(
            ValidationStage::TypedMembers,
            "feature allocator layout changed",
        );
    }
    Ok(references)
}

fn validate_portable_presentation_assets(
    value: Option<&Value>,
    files: &BTreeMap<String, (u64, String)>,
    members: &BTreeMap<String, Vec<u8>>,
    expected: bool,
) -> Result<Vec<PortablePresentationAsset>> {
    if !expected {
        if value.is_some() {
            return fail(
                ValidationStage::Definition,
                "campaign profile does not admit portable presentation assets",
            );
        }
        return Ok(Vec::new());
    }
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == 2)
        .ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::TypedMembers,
            message: "Level 1 presentation asset set differs".to_owned(),
        })?;
    let expected_rows = [
        (
            "presentation.fireball.projectile.base",
            "fireball-projectile-base.indexed-sprite.json",
            FIREBALL_PROJECTILE_BASE_ASSET_SHA256,
            "CD_Files/DATA/PALN-0.DAT",
            "6b55d0714b939e8bd25cef89466806299dbae933f2f2f96e0cf8402fc607d16f",
            "7c4dfbbc6c0f68a99214fe3ca93d7a1cf0dd658e17a40ad7ecb302152264f507",
        ),
        (
            "presentation.fireball.projectile.fade",
            "fireball-projectile-fade.indexed-sprite.json",
            FIREBALL_PROJECTILE_FADE_ASSET_SHA256,
            "CD_Files/DATA/PALF-0.DAT",
            "c83f900a3d91b68e968f20cc484c8e0ca9a2578211c4d058a747d7f8db2880b4",
            "f0091d61149264890604486b1d4aceadddf62c0cc90f0c0083a3cae881240b6f",
        ),
    ];
    let mut result = Vec::with_capacity(rows.len());
    for (ordinal, expected_row) in expected_rows.iter().enumerate() {
        let object = exact_object(
            &rows[ordinal],
            &["artifact_id", "member", "slot_id", "source"],
            "presentation asset",
            ValidationStage::TypedMembers,
        )?;
        require_string_eq(
            &object["artifact_id"],
            expected_row.0,
            "presentation asset.artifact_id",
            ValidationStage::TypedMembers,
        )?;
        require_string_eq(
            &object["slot_id"],
            "presentation.spell.fireball.projectile_asset",
            "presentation asset.slot_id",
            ValidationStage::TypedMembers,
        )?;
        let member = parse_member_ref(
            &object["member"],
            files,
            "presentation asset.member",
            Some("carpet-flight-indexed-sprite/v1"),
            None,
            None,
            ValidationStage::TypedMembers,
        )?;
        if member.member != expected_row.1 || member.sha256 != expected_row.2 {
            return fail(
                ValidationStage::TypedMembers,
                "portable presentation payload differs from the qualified profile",
            );
        }
        let expected_source = json!({
            "decoded_sha256": "cf185f7e35f9103be4b4ecfa2573fc9332ac8ade346b564be3971cbe73fb35ec",
            "frame_index": 1,
            "indices_sha256": "f6755ec0f9931b6748c32893cc11838fb398043dafdc962eb5e288105cf0c88d",
            "owner_artifacts": [
                {"bytes": 1_354_118, "logical_path": "CD_Files/DATA/TMAPS1-0.DAT", "sha256": "b091204ec21c0cf99716f3ae5c426a37113e57dda48a0d4d2eef98167e6ed0ff"},
                {"bytes": 5_050, "logical_path": "CD_Files/DATA/TMAPS1-0.TAB", "sha256": "22afd08167ae415138548f99901ea037bdc2b48f56c6f3b2dbcfe9743d73c8f0"},
                {"bytes": 768, "logical_path": expected_row.3, "sha256": expected_row.4}
            ],
            "palette_rgb8_sha256": expected_row.5,
            "selector": 463,
            "transform": "base_frame_indices_plus_rgb6_times_4_palette_index_zero_transparent"
        });
        if object["source"] != expected_source {
            return fail(
                ValidationStage::TypedMembers,
                "portable presentation source provenance differs",
            );
        }
        let payload = &members[&member.member];
        let payload_value: Value =
            serde_json::from_slice(payload).map_err(|error| CampaignPackageError {
                stage: ValidationStage::TypedMembers,
                message: format!("cannot parse portable presentation payload: {error}"),
            })?;
        let mut canonical =
            serde_json::to_vec(&payload_value).map_err(|error| CampaignPackageError {
                stage: ValidationStage::TypedMembers,
                message: format!("cannot canonicalize portable presentation payload: {error}"),
            })?;
        canonical.push(b'\n');
        if canonical != *payload {
            return fail(
                ValidationStage::TypedMembers,
                "portable presentation payload is not canonical sorted JSON",
            );
        }
        result.push(PortablePresentationAsset {
            artifact_id: expected_row.0.to_owned(),
            slot_id: "presentation.spell.fireball.projectile_asset".to_owned(),
            member,
        });
    }
    Ok(result)
}

fn validate_presentation(
    value: &Value,
    files: &BTreeMap<String, (u64, String)>,
    members: &BTreeMap<String, Vec<u8>>,
    selection: CampaignSelection,
) -> Result<PresentationRecipe> {
    let object = exact_object(
        value,
        &[
            "block_bank",
            "cursor",
            "map_type",
            "member",
            "palette",
            "resolved_files",
            "sky_bank",
            "sound_bank",
            "sprite_bank",
            "terrain_bank",
            "terrain_graphics_flag_2",
            "terrain_graphics_flags",
            "texture_size",
            "vga_resolution_selector",
            "web_bank",
        ],
        "presentation_recipe",
        ValidationStage::TypedMembers,
    )?;
    let reference = parse_member_ref(
        &object["member"],
        files,
        "presentation_recipe.member",
        Some("mc2-presentation-snapshot/v1"),
        None,
        None,
        ValidationStage::TypedMembers,
    )?;
    let expected_strings = match selection {
        CampaignSelection::LEVEL1 => [
            ("block_bank", "BL32N0-0"),
            ("map_type", "Night"),
            ("palette", "PALN-0"),
            ("sky_bank", "SKYN0-0"),
            ("sprite_bank", "MSPRN0-0"),
            ("terrain_bank", "TMAPS1-0"),
            ("web_bank", "MWEBN0-0"),
        ],
        CampaignSelection::LEVEL2 | CampaignSelection::LEVEL3 => [
            ("block_bank", "BL32N0-0"),
            ("map_type", "Day"),
            ("palette", "PALD-0"),
            ("sky_bank", "SKYD0-0"),
            ("sprite_bank", "MSPRD0-0"),
            ("terrain_bank", "TMAPS0-0"),
            ("web_bank", "MWEBD0-0"),
        ],
        _ => unreachable!("selection was profile-validated"),
    };
    for (name, expected) in expected_strings {
        require_string_eq(
            &object[name],
            expected,
            &format!("presentation_recipe.{name}"),
            ValidationStage::TypedMembers,
        )?;
    }
    for (name, expected) in [
        ("cursor", 9_u64),
        ("sound_bank", 1),
        ("texture_size", 32),
        ("vga_resolution_selector", 1),
    ] {
        if unsigned(
            &object[name],
            &format!("presentation_recipe.{name}"),
            ValidationStage::TypedMembers,
        )? != expected
        {
            return fail(
                ValidationStage::TypedMembers,
                format!("presentation_recipe.{name} differs"),
            );
        }
    }
    let terrain_graphics_flag_2 =
        object["terrain_graphics_flag_2"]
            .as_bool()
            .ok_or_else(|| CampaignPackageError {
                stage: ValidationStage::TypedMembers,
                message: "presentation recipe terrain_graphics_flag_2 is not boolean".to_owned(),
            })?;
    let terrain_graphics_flags = u32::try_from(unsigned(
        &object["terrain_graphics_flags"],
        "presentation_recipe.terrain_graphics_flags",
        ValidationStage::TypedMembers,
    )?)
    .map_err(|_| CampaignPackageError {
        stage: ValidationStage::TypedMembers,
        message: "presentation recipe terrain_graphics_flags exceeds u32".to_owned(),
    })?;
    if terrain_graphics_flag_2 || terrain_graphics_flags != 0 {
        return fail(
            ValidationStage::TypedMembers,
            "presentation recipe terrain flags differ from the campaign profile",
        );
    }
    let resolved = exact_object(
        &object["resolved_files"],
        &[
            "block_bank",
            "building_bank",
            "font_bank",
            "language",
            "level_records",
            "palette",
            "sky_bank",
            "spell_table",
            "sprite_bank",
            "terrain_bank",
            "web_bank",
        ],
        "presentation_recipe.resolved_files",
        ValidationStage::TypedMembers,
    )?;
    let expected_resolved = match selection {
        CampaignSelection::LEVEL1 => [
            ("block_bank", "owner:CD_Files/DATA/BL32N0-0.DAT"),
            ("building_bank", "owner:CD_Files/DATA/BUILD0-0.DAT"),
            ("font_bank", "owner:CD_Files/DATA/FONT1.DAT"),
            ("language", "owner:CD_Files/LANGUAGE/L1.TXT"),
            ("level_records", "owner:GAME/NETHERW/CLEVELS/LEVELS.DAT"),
            ("palette", "owner:CD_Files/DATA/PALN-0.DAT"),
            ("sky_bank", "owner:CD_Files/DATA/SKYN0-0.DAT"),
            ("spell_table", "owner:CD_Files/DATA/SPELLS.DAT"),
            ("sprite_bank", "owner:CD_Files/DATA/MSPRN0-0.DAT"),
            ("terrain_bank", "owner:GAME/NETHERW/CDATA/TMAPS1-0.DAT"),
            ("web_bank", "owner:CD_Files/DATA/MWEBN0-0.DAT"),
        ],
        CampaignSelection::LEVEL2 | CampaignSelection::LEVEL3 => [
            ("block_bank", "owner:CD_Files/DATA/BL32N0-0.DAT"),
            ("building_bank", "owner:CD_Files/DATA/BUILD0-0.DAT"),
            ("font_bank", "owner:CD_Files/DATA/FONT1.DAT"),
            ("language", "owner:CD_Files/LANGUAGE/L1.TXT"),
            ("level_records", "owner:GAME/NETHERW/CLEVELS/LEVELS.DAT"),
            ("palette", "owner:CD_Files/DATA/PALD-0.DAT"),
            ("sky_bank", "owner:CD_Files/DATA/SKYD0-0.DAT"),
            ("spell_table", "owner:CD_Files/DATA/SPELLS.DAT"),
            ("sprite_bank", "owner:CD_Files/DATA/MSPRD0-0.DAT"),
            ("terrain_bank", "owner:GAME/NETHERW/CDATA/TMAPS0-0.DAT"),
            ("web_bank", "owner:CD_Files/DATA/MWEBD0-0.DAT"),
        ],
        _ => unreachable!("selection was profile-validated"),
    };
    for (name, expected) in expected_resolved {
        require_string_eq(
            &resolved[name],
            expected,
            &format!("presentation_recipe.resolved_files.{name}"),
            ValidationStage::TypedMembers,
        )?;
    }
    let bytes = &members[&reference.member];
    if bytes.len() != 10_720
        || bytes.get(..4) != Some(b"MCV1")
        || read_u16(
            bytes,
            4,
            "presentation version",
            ValidationStage::TypedMembers,
        )? != 1
        || read_u16(
            bytes,
            6,
            "presentation reserved",
            ValidationStage::TypedMembers,
        )? != 0
        || read_u32(
            bytes,
            8,
            "presentation particle count",
            ValidationStage::TypedMembers,
        )? != 347
    {
        return fail(
            ValidationStage::TypedMembers,
            "presentation snapshot layout changed",
        );
    }
    Ok(PresentationRecipe {
        member: reference,
        block_bank: string(
            &object["block_bank"],
            "presentation_recipe.block_bank",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        cursor: bounded_u8(
            &object["cursor"],
            "presentation_recipe.cursor",
            ValidationStage::TypedMembers,
        )?,
        map_type: string(
            &object["map_type"],
            "presentation_recipe.map_type",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        palette: string(
            &object["palette"],
            "presentation_recipe.palette",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        resolved_files: PresentationResolvedFiles {
            block_bank: string(
                &resolved["block_bank"],
                "presentation_recipe.resolved_files.block_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            building_bank: string(
                &resolved["building_bank"],
                "presentation_recipe.resolved_files.building_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            font_bank: string(
                &resolved["font_bank"],
                "presentation_recipe.resolved_files.font_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            language: string(
                &resolved["language"],
                "presentation_recipe.resolved_files.language",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            level_records: string(
                &resolved["level_records"],
                "presentation_recipe.resolved_files.level_records",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            palette: string(
                &resolved["palette"],
                "presentation_recipe.resolved_files.palette",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            sky_bank: string(
                &resolved["sky_bank"],
                "presentation_recipe.resolved_files.sky_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            spell_table: string(
                &resolved["spell_table"],
                "presentation_recipe.resolved_files.spell_table",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            sprite_bank: string(
                &resolved["sprite_bank"],
                "presentation_recipe.resolved_files.sprite_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            terrain_bank: string(
                &resolved["terrain_bank"],
                "presentation_recipe.resolved_files.terrain_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
            web_bank: string(
                &resolved["web_bank"],
                "presentation_recipe.resolved_files.web_bank",
                ValidationStage::TypedMembers,
            )?
            .to_owned(),
        },
        sky_bank: string(
            &object["sky_bank"],
            "presentation_recipe.sky_bank",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        sound_bank: bounded_u8(
            &object["sound_bank"],
            "presentation_recipe.sound_bank",
            ValidationStage::TypedMembers,
        )?,
        sprite_bank: string(
            &object["sprite_bank"],
            "presentation_recipe.sprite_bank",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        terrain_bank: string(
            &object["terrain_bank"],
            "presentation_recipe.terrain_bank",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
        terrain_graphics_flag_2,
        terrain_graphics_flags,
        texture_size: bounded_u16(
            &object["texture_size"],
            "presentation_recipe.texture_size",
            ValidationStage::TypedMembers,
        )?,
        vga_resolution_selector: bounded_u16(
            &object["vga_resolution_selector"],
            "presentation_recipe.vga_resolution_selector",
            ValidationStage::TypedMembers,
        )?,
        web_bank: string(
            &object["web_bank"],
            "presentation_recipe.web_bank",
            ValidationStage::TypedMembers,
        )?
        .to_owned(),
    })
}

fn validate_operations(
    spec: &serde_json::Map<String, Value>,
    authored: &[u8],
    files: &BTreeMap<String, (u64, String)>,
    members: &BTreeMap<String, Vec<u8>>,
) -> Result<(InitialRng, Vec<ObjectiveOperation>)> {
    let Some(initial_activation) = spec["initial_activation"].as_array() else {
        return fail(
            ValidationStage::TypedOperations,
            "initial_activation is not an array",
        );
    };
    if !initial_activation.is_empty() {
        return fail(
            ValidationStage::TypedOperations,
            "schema-v1 has no proven initial activation operation",
        );
    }
    let rng = exact_object(
        &spec["initial_rng"],
        &["global", "terrain"],
        "initial_rng",
        ValidationStage::TypedOperations,
    )?;
    for (name, encoding, offset) in [("global", "u32_le", 12_u64), ("terrain", "u16_le", 8)] {
        let field = exact_object(
            &rng[name],
            &["encoding", "member", "offset", "sha256"],
            &format!("initial_rng.{name}"),
            ValidationStage::TypedOperations,
        )?;
        if string(
            &field["encoding"],
            &format!("initial_rng.{name}.encoding"),
            ValidationStage::TypedOperations,
        )? != encoding
            || string(
                &field["member"],
                &format!("initial_rng.{name}.member"),
                ValidationStage::TypedOperations,
            )? != "feature-end.bin"
            || unsigned(
                &field["offset"],
                &format!("initial_rng.{name}.offset"),
                ValidationStage::TypedOperations,
            )? != offset
            || lower_sha256(
                &field["sha256"],
                &format!("initial_rng.{name}.sha256"),
                ValidationStage::TypedOperations,
            )? != files["feature-end.bin"].1
        {
            return fail(
                ValidationStage::TypedOperations,
                format!("initial_rng.{name} source field differs"),
            );
        }
    }
    if members["feature-end.bin"].len() != 16 {
        return fail(
            ValidationStage::TypedOperations,
            "initial RNG member extent changed",
        );
    }
    let rng_member_sha256 = files["feature-end.bin"].1.clone();
    let initial_rng = InitialRng {
        global: RngSourceField {
            member: "feature-end.bin".to_owned(),
            sha256: rng_member_sha256.clone(),
            offset: 12,
            encoding: RngFieldEncoding::U32LittleEndian,
        },
        terrain: RngSourceField {
            member: "feature-end.bin".to_owned(),
            sha256: rng_member_sha256,
            offset: 8,
            encoding: RngFieldEncoding::U16LittleEndian,
        },
    };

    let Some(values) = spec["objectives"].as_array() else {
        return fail(
            ValidationStage::TypedOperations,
            "objectives is not an array",
        );
    };
    let mut declared = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let label = format!("objectives[{index}]");
        let object = exact_object(
            value,
            &["operands", "operation_kind", "operation_version"],
            &label,
            ValidationStage::TypedOperations,
        )?;
        let kind = bounded_u16(
            &object["operation_kind"],
            &format!("{label}.operation_kind"),
            ValidationStage::TypedOperations,
        )?;
        let version = bounded_u16(
            &object["operation_version"],
            &format!("{label}.operation_version"),
            ValidationStage::TypedOperations,
        )?;
        if kind != 1 || version != 1 {
            return fail(
                ValidationStage::TypedOperations,
                format!("{label} has an unknown operation kind/version"),
            );
        }
        let operands = exact_object(
            &object["operands"],
            &[
                "axis_x",
                "axis_y",
                "objective_selector",
                "objective_slot",
                "stage_index",
            ],
            &format!("{label}.operands"),
            ValidationStage::TypedOperations,
        )?;
        declared.push(ObjectiveOperation {
            operation_kind: kind,
            operation_version: version,
            objective_slot: bounded_u8(
                &operands["objective_slot"],
                &format!("{label}.operands.objective_slot"),
                ValidationStage::TypedOperations,
            )?,
            stage_index: signed_i8(
                &operands["stage_index"],
                &format!("{label}.operands.stage_index"),
                ValidationStage::TypedOperations,
            )?,
            objective_selector: signed_i16(
                &operands["objective_selector"],
                &format!("{label}.operands.objective_selector"),
                ValidationStage::TypedOperations,
            )?,
            axis_x: signed_i16(
                &operands["axis_x"],
                &format!("{label}.operands.axis_x"),
                ValidationStage::TypedOperations,
            )?,
            axis_y: signed_i16(
                &operands["axis_y"],
                &format!("{label}.operands.axis_y"),
                ValidationStage::TypedOperations,
            )?,
        });
    }
    let mut expected = Vec::new();
    let mut inactive_seen = false;
    for index in 0..8 {
        let offset = AUTHORED_STAGE_OFFSET + index * 7;
        let stage = authored[offset] as i8;
        let objective_selector = read_i16(
            authored,
            offset + 1,
            "authored objective selector",
            ValidationStage::TypedOperations,
        )?;
        let axis_x = read_i16(
            authored,
            offset + 3,
            "authored objective x",
            ValidationStage::TypedOperations,
        )?;
        let axis_y = read_i16(
            authored,
            offset + 5,
            "authored objective y",
            ValidationStage::TypedOperations,
        )?;
        if stage == -1 {
            inactive_seen = true;
            if (objective_selector, axis_x, axis_y) != (0, 0, 0) {
                return fail(
                    ValidationStage::TypedOperations,
                    format!("inactive objective row {index} has payload"),
                );
            }
            continue;
        }
        if inactive_seen || !matches!(stage, 0 | 3 | 5 | 7 | 9) {
            return fail(
                ValidationStage::TypedOperations,
                format!("objective row {index} has an unproven form"),
            );
        }
        expected.push(ObjectiveOperation {
            operation_kind: 1,
            operation_version: 1,
            objective_slot: u8::try_from(index).expect("eight objective rows"),
            stage_index: stage,
            objective_selector,
            axis_x,
            axis_y,
        });
    }
    if declared != expected {
        return fail(
            ValidationStage::TypedOperations,
            "objective operations differ from the authored census-profiled rows",
        );
    }
    Ok((initial_rng, declared))
}

fn expected_behavior_requirements(rows: &[AuthoredBehaviorRow]) -> Vec<BehaviorRequirement> {
    let mut provenance = BTreeMap::<BehaviorKey, BehaviorProvenance>::new();
    for row in rows {
        // GOLDEN-CHANGE (operator-approved, 2026-08-23): schema v1 binds a
        // post-GenerateEvents reset snapshot. Only live reset rows are
        // executable runtime roots. Dormant authored rows remain exact
        // inventory until a later descriptor graph makes them reachable.
        if row.initial_source_slot.is_none() {
            continue;
        }
        let source = BehaviorProvenance::AuthoredRecord {
            authored_record: row.authored_record,
            initial_source_slot: row.initial_source_slot,
        };
        let key = BehaviorKey {
            dispatch_site: 2,
            selector: BehaviorSelector::ClassAction {
                class: row.class,
                action: row.action,
            },
        };
        provenance.entry(key).or_insert(source);
    }
    provenance
        .into_iter()
        .map(|(key, source)| BehaviorRequirement {
            key,
            provenance: vec![source],
        })
        .collect()
}

fn parse_selector(value: &Value, label: &str) -> Result<BehaviorSelector> {
    let object = value.as_object().ok_or_else(|| CampaignPackageError {
        stage: ValidationStage::BehaviorClosure,
        message: format!("{label} is not an object"),
    })?;
    let kind = string(
        object.get("kind").ok_or_else(|| CampaignPackageError {
            stage: ValidationStage::BehaviorClosure,
            message: format!("{label}.kind is absent"),
        })?,
        &format!("{label}.kind"),
        ValidationStage::BehaviorClosure,
    )?;
    match kind {
        "ClassAction" => {
            exact_object(
                value,
                &["action", "class", "kind"],
                label,
                ValidationStage::BehaviorClosure,
            )?;
            Ok(BehaviorSelector::ClassAction {
                class: bounded_u8(
                    &object["class"],
                    &format!("{label}.class"),
                    ValidationStage::BehaviorClosure,
                )?,
                action: bounded_u8(
                    &object["action"],
                    &format!("{label}.action"),
                    ValidationStage::BehaviorClosure,
                )?,
            })
        }
        "ClassModel" => {
            exact_object(
                value,
                &["class", "kind", "model"],
                label,
                ValidationStage::BehaviorClosure,
            )?;
            Ok(BehaviorSelector::ClassModel {
                class: bounded_u8(
                    &object["class"],
                    &format!("{label}.class"),
                    ValidationStage::BehaviorClosure,
                )?,
                model: bounded_u8(
                    &object["model"],
                    &format!("{label}.model"),
                    ValidationStage::BehaviorClosure,
                )?,
            })
        }
        "TypeSubtype" => {
            exact_object(
                value,
                &["kind", "subtype", "type_id"],
                label,
                ValidationStage::BehaviorClosure,
            )?;
            Ok(BehaviorSelector::TypeSubtype {
                type_id: bounded_u8(
                    &object["type_id"],
                    &format!("{label}.type_id"),
                    ValidationStage::BehaviorClosure,
                )?,
                subtype: bounded_u8(
                    &object["subtype"],
                    &format!("{label}.subtype"),
                    ValidationStage::BehaviorClosure,
                )?,
            })
        }
        "DirectCallback" => {
            exact_object(
                value,
                &["kind", "stable_callback_id"],
                label,
                ValidationStage::BehaviorClosure,
            )?;
            Ok(BehaviorSelector::DirectCallback {
                stable_callback_id: bounded_u16(
                    &object["stable_callback_id"],
                    &format!("{label}.stable_callback_id"),
                    ValidationStage::BehaviorClosure,
                )?,
            })
        }
        _ => fail(
            ValidationStage::BehaviorClosure,
            format!("{label} has unknown selector kind {kind}"),
        ),
    }
}

fn parse_behavior_key(value: &Value, label: &str) -> Result<BehaviorKey> {
    let object = exact_object(
        value,
        &["dispatch_site", "selector"],
        label,
        ValidationStage::BehaviorClosure,
    )?;
    let key = BehaviorKey {
        dispatch_site: bounded_u16(
            &object["dispatch_site"],
            &format!("{label}.dispatch_site"),
            ValidationStage::BehaviorClosure,
        )?,
        selector: parse_selector(&object["selector"], &format!("{label}.selector"))?,
    };
    if !matches!(key.dispatch_site, 1..=5) {
        return fail(
            ValidationStage::BehaviorClosure,
            format!("{label} has unknown dispatch site"),
        );
    }
    Ok(key)
}

fn parse_behavior_requirements(value: &Value) -> Result<Vec<BehaviorRequirement>> {
    let Some(rows) = value.as_array() else {
        return fail(
            ValidationStage::BehaviorClosure,
            "required_behaviors is not an array",
        );
    };
    let mut result = Vec::with_capacity(rows.len());
    for (index, value) in rows.iter().enumerate() {
        let label = format!("required_behaviors[{index}]");
        let object = exact_object(
            value,
            &["dispatch_site", "provenance", "selector"],
            &label,
            ValidationStage::BehaviorClosure,
        )?;
        let key = BehaviorKey {
            dispatch_site: bounded_u16(
                &object["dispatch_site"],
                &format!("{label}.dispatch_site"),
                ValidationStage::BehaviorClosure,
            )?,
            selector: parse_selector(&object["selector"], &format!("{label}.selector"))?,
        };
        let Some(provenance_rows) = object["provenance"].as_array() else {
            return fail(
                ValidationStage::BehaviorClosure,
                format!("{label}.provenance is not an array"),
            );
        };
        if provenance_rows.is_empty() {
            return fail(
                ValidationStage::BehaviorClosure,
                format!("{label}.provenance is empty"),
            );
        }
        let mut provenance = Vec::with_capacity(provenance_rows.len());
        for (provenance_index, value) in provenance_rows.iter().enumerate() {
            let provenance_label = format!("{label}.provenance[{provenance_index}]");
            let Some(object) = value.as_object() else {
                return fail(
                    ValidationStage::BehaviorClosure,
                    format!("{provenance_label} is not an object"),
                );
            };
            if object.contains_key("predecessor") {
                exact_object(
                    value,
                    &["predecessor"],
                    &provenance_label,
                    ValidationStage::BehaviorClosure,
                )?;
                provenance.push(BehaviorProvenance::Predecessor(parse_behavior_key(
                    &object["predecessor"],
                    &format!("{provenance_label}.predecessor"),
                )?));
            } else {
                exact_object(
                    value,
                    &["authored_record", "initial_source_slot"],
                    &provenance_label,
                    ValidationStage::BehaviorClosure,
                )?;
                let slot = if object["initial_source_slot"].is_null() {
                    None
                } else {
                    Some(bounded_u16(
                        &object["initial_source_slot"],
                        &format!("{provenance_label}.initial_source_slot"),
                        ValidationStage::BehaviorClosure,
                    )?)
                };
                provenance.push(BehaviorProvenance::AuthoredRecord {
                    authored_record: bounded_u16(
                        &object["authored_record"],
                        &format!("{provenance_label}.authored_record"),
                        ValidationStage::BehaviorClosure,
                    )?,
                    initial_source_slot: slot,
                });
            }
        }
        result.push(BehaviorRequirement { key, provenance });
    }
    if !result.windows(2).all(|rows| rows[0].key < rows[1].key) {
        return fail(
            ValidationStage::BehaviorClosure,
            "required_behaviors is not strictly sorted and unique",
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        AUTHORED_RECORD_BYTES, AUTHORED_RECORD_COUNT, AUTHORED_RECORD_OFFSET,
        AUTHORED_STAGE_OFFSET, AuthoredBehaviorRow, CampaignDefinition, CampaignSelection,
        ValidationStage, building_objective_provenance, decode_behavior_rows,
    };
    use std::{fs, path::PathBuf};

    fn fixture() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            PathBuf::from,
        )
    }

    fn level1_fixture() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf15-level1-package-v3-current")
            },
            PathBuf::from,
        )
    }

    fn level3_fixture() -> PathBuf {
        std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
            },
            PathBuf::from,
        )
    }

    #[test]
    fn semantic_initial_creature_bindings_cover_level_one_two_and_three_roots() {
        use crate::level_definition::CreatureKind;

        let accept = |_: &super::BehaviorKey| true;
        let cases = [
            (
                level1_fixture(),
                CampaignSelection::LEVEL1,
                vec![(CreatureKind::Goat, 33), (CreatureKind::Villager, 28)],
            ),
            (
                fixture(),
                CampaignSelection::LEVEL2,
                vec![(CreatureKind::Goat, 36), (CreatureKind::Villager, 29)],
            ),
            (
                level3_fixture(),
                CampaignSelection::LEVEL3,
                vec![(CreatureKind::Worm, 5), (CreatureKind::Skeleton, 11)],
            ),
        ];

        for (root, selection, expected) in cases {
            let definition = CampaignDefinition::preflight(root, selection, &accept)
                .expect("qualified package preflights");
            let bindings = definition.initial_creature_bindings();
            for (kind, count) in expected {
                assert_eq!(
                    bindings
                        .values()
                        .filter(|candidate| **candidate == kind)
                        .count(),
                    count,
                    "{selection:?} {kind:?} root count"
                );
            }
            assert!(
                definition
                    .campaign_spec
                    .authored_behavior_rows
                    .iter()
                    .all(|row| {
                        row.initial_source_slot
                            .is_none_or(|slot| row.action != 232 || !bindings.contains_key(&slot))
                    })
            );
        }
    }

    #[test]
    fn mc2_reset_creature_selector_includes_house_builder_and_excludes_parts() {
        use crate::{
            campaign::mc2_level_definition::Mc2LevelDefinition, level_definition::CreatureKind,
        };

        assert_eq!(
            Mc2LevelDefinition::creature_for_reset_selector(5, 12, 97),
            Some(CreatureKind::Builder)
        );
        assert_eq!(
            Mc2LevelDefinition::creature_for_reset_selector(5, 3, 232),
            None
        );
        assert_eq!(
            Mc2LevelDefinition::creature_for_house_resident(12, 97),
            Some(CreatureKind::Builder)
        );
        assert_eq!(
            Mc2LevelDefinition::creature_for_house_resident(12, 105),
            None
        );
    }

    #[test]
    #[ignore = "requires a real owner-data Level 1 campaign envelope"]
    fn cf15_level1_definition_preflights_and_inspects_without_collision() {
        let accept = |_: &super::BehaviorKey| true;
        let definition =
            CampaignDefinition::preflight(level1_fixture(), CampaignSelection::LEVEL1, &accept)
                .expect("real Level 1 descriptor preflights");
        assert_eq!(definition.completed_stage, ValidationStage::ResetState);
        assert_eq!(definition.source_facts.len(), 2);
        assert_eq!(
            definition
                .campaign_spec
                .reset_sources
                .players
                .schema
                .as_deref(),
            Some("mc2-player-arena/v2")
        );
        assert_eq!(
            definition.campaign_spec.reset_sources.players.record_bytes,
            Some(573)
        );
        let inspection = crate::native_world::NativeWorld::inspect_campaign_definition(&definition)
            .expect("Level 1 descriptor-backed inspection opens");
        assert_eq!(inspection.players().len(), 8);
        assert!(inspection.collision_mask_bank().is_none());
        assert_eq!(inspection.metadata().external_level, 1);
        assert_eq!(inspection.metadata().internal_level, 0);

        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(level1_fixture().join("manifest.json")).expect("campaign manifest exists"),
        )
        .expect("campaign manifest parses");
        let outer = manifest
            .as_object()
            .expect("campaign manifest is an object");
        assert_eq!(
            definition
                .owner_export_package_sha256()
                .expect("wrapped owner identity reconstructs"),
            outer["owner_export_package_sha256"]
                .as_str()
                .expect("owner export identity is a string")
        );

        let mut file_map = std::collections::BTreeMap::new();
        for (name, metadata) in outer["files"]
            .as_object()
            .expect("campaign file map is an object")
        {
            file_map.insert(
                name.clone(),
                (
                    metadata["bytes"].as_u64().expect("member bytes is u64"),
                    metadata["sha256"]
                        .as_str()
                        .expect("member hash is a string")
                        .to_owned(),
                ),
            );
        }
        let mut tampered_owner: serde_json::Value = serde_json::from_slice(
            &fs::read(level1_fixture().join("owner-manifest.json")).expect("owner manifest exists"),
        )
        .expect("owner manifest parses");
        tampered_owner["package_sha256"] = serde_json::Value::String("0".repeat(64));
        let error = super::validate_owner_manifest(
            &serde_json::to_vec(&tampered_owner).expect("tampered owner manifest serializes"),
            CampaignSelection::LEVEL1,
            &file_map,
            outer["owner_export_package_sha256"]
                .as_str()
                .expect("owner export identity is a string"),
        )
        .expect_err("tampered owner manifest/package binding rejects");
        assert_eq!(error.stage, ValidationStage::Definition);
        assert!(error.message.contains("package identity differs"));

        let mut substituted_files = outer["files"].clone();
        substituted_files
            .as_object_mut()
            .expect("campaign file map is an object")
            .insert("collision-masks.bin".to_owned(), serde_json::Value::Null);
        let profile = super::campaign_package_profile(CampaignSelection::LEVEL1)
            .expect("Level 1 package profile exists");
        let error = super::exact_object(
            &substituted_files,
            profile.members,
            "Level 1 substituted file map",
            ValidationStage::OuterManifest,
        )
        .expect_err("a Level 2 collision member cannot be substituted into Level 1");
        assert_eq!(error.stage, ValidationStage::OuterManifest);

        let level2_players = fs::read(fixture().join("players.bin"))
            .expect("qualified Level 2 players member exists");
        let level1_entities = fs::read(level1_fixture().join("entities.bin"))
            .expect("real Level 1 entities member exists");
        let error = super::validate_players(&level2_players, &level1_entities, 1, 2, 573)
            .expect_err("a Level 2 player layout cannot be substituted into Level 1");
        assert_eq!(error.stage, ValidationStage::TypedMembers);
        assert!(error.message.contains("players.bin header/extent changed"));
    }

    #[test]
    fn cf02_level2_definition_preflights_before_reset_state() {
        let accept = |_: &super::BehaviorKey| true;
        let definition =
            CampaignDefinition::preflight(fixture(), CampaignSelection::LEVEL2, &accept)
                .expect("qualified CF-02 descriptor preflights");
        assert_eq!(definition.completed_stage, ValidationStage::ResetState);
        assert!(definition.reset_state_receipt.structurally_validated);
        assert_eq!(definition.source_facts.len(), 4);
        assert_eq!(
            definition
                .standing_building_chains()
                .get(
                    &crate::level_definition::LevelObjectiveId::parse(
                        "objective.clear.payahandra_vaults"
                    )
                    .expect("stable objective id parses")
                )
                .map(Box::as_ref),
            Some(&[21, 54][..])
        );
        assert_eq!(
            definition
                .campaign_spec
                .reset_sources
                .feature_boundary
                .len(),
            7
        );
        assert!(definition.campaign_spec.initial_activation.is_empty());
        assert_eq!(definition.campaign_spec.initial_rng.global.offset, 12);
        assert_eq!(
            definition
                .campaign_spec
                .presentation_recipe
                .terrain_graphics_flags,
            0
        );
        assert_eq!(definition.campaign_spec.objectives.len(), 5);
        assert!(!definition.required_behaviors.is_empty());
    }

    #[test]
    fn standing_building_source_fact_joins_the_typed_objective_selector() {
        let mut authored = vec![0_u8; AUTHORED_STAGE_OFFSET + 8 * 7];
        let offset = AUTHORED_STAGE_OFFSET + 4 * 7;
        authored[offset] = 9;
        authored[offset + 1..offset + 3].copy_from_slice(&163_i16.to_le_bytes());
        let rows = [AuthoredBehaviorRow {
            authored_record: 163,
            initial_source_slot: None,
            class: 10,
            action: 8,
            model: 26,
            subtype: 45,
            initial_dispatch_sites: vec![1, 2],
        }];
        let error = building_objective_provenance(true, &authored, &rows)
            .expect_err("a different valid building row cannot replace the qualified objective");
        assert_eq!(error.stage, ValidationStage::SourceFacts);
        assert!(error.message.contains("objective operation differs"));
    }

    #[test]
    fn cf12_descriptor_preflight_rejects_an_uncompiled_capability() {
        let reject = |_: &super::BehaviorKey| false;
        let error = CampaignDefinition::preflight(fixture(), CampaignSelection::LEVEL2, &reject)
            .expect_err("an uncompiled descriptor capability must fail before reset state");
        assert_eq!(error.stage, ValidationStage::BehaviorClosure);
        assert!(
            error
                .message
                .contains("compiled registry is missing capability")
        );
    }

    #[test]
    fn cf02_authored_subtype_must_fit_the_schema_v1_u8() {
        let mut authored = fs::read(fixture().join("authored-level.bin"))
            .expect("qualified authored member exists");
        let entities =
            fs::read(fixture().join("entities.bin")).expect("qualified entity member exists");
        let record = (0..AUTHORED_RECORD_COUNT)
            .find(|index| {
                let offset = AUTHORED_RECORD_OFFSET + index * AUTHORED_RECORD_BYTES;
                authored[offset..offset + AUTHORED_RECORD_BYTES]
                    .iter()
                    .any(|byte| *byte != 0)
            })
            .expect("qualified package has an authored behavior row");
        let offset = AUTHORED_RECORD_OFFSET + record * AUTHORED_RECORD_BYTES + 2;
        authored[offset..offset + 2].copy_from_slice(&(-2_i16).to_le_bytes());

        let error =
            decode_behavior_rows(&authored, &entities, CampaignSelection::LEVEL2.map_type_id)
                .expect_err("negative signed subtype cannot narrow into schema-v1 u8");
        assert_eq!(error.stage, ValidationStage::TypedMembers);
        assert!(error.message.contains("schema-v1 widths"));
    }
}
