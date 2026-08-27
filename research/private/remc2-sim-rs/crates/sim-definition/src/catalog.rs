//! Stable semantic descriptors and their definition read bindings.
//!
//! The catalog describes game meaning. Package/source selectors are kept in
//! adapters and never become public IDs. A descriptor becomes definition-
//! writable only when all four authoring callbacks exist. That alone does not
//! prove a live gameplay consumer uses the published value.

use super::{DefinitionError, DefinitionErrorCode, GameDefinition, Result};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SemanticId(String);

impl<'de> Deserialize<'de> for SemanticId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl SemanticId {
    /// Parse a lowercase dotted semantic ID.
    ///
    /// # Errors
    ///
    /// Returns an error for source-shaped, package-shaped, empty, uppercase,
    /// or otherwise noncanonical IDs.
    pub fn parse(value: &str) -> Result<Self> {
        let segments = value.split('.').collect::<Vec<_>>();
        if segments.len() < 2
            || segments.iter().any(|segment| {
                segment.is_empty()
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    || !segment
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_lowercase)
            })
            || value.contains("level1")
            || value.contains("level2")
            || value.contains("source_slot")
        {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidSemanticId,
                value,
                "semantic ID must be lowercase dotted game meaning and must not expose level/source layout",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum SemanticValue {
    Integer(i64),
    Enum(String),
    Reference(String),
}

impl SemanticValue {
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Integer(value) => json!({"type": "integer", "value": value}),
            Self::Enum(value) => json!({"type": "enum", "value": value}),
            Self::Reference(value) => json!({"type": "reference", "value": value}),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorKind {
    Tuning,
    AssetSlot,
    Rule,
    Behavior,
}

impl DescriptorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tuning => "tuning",
            Self::AssetSlot => "asset_slot",
            Self::Rule => "rule",
            Self::Behavior => "behavior",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Integer,
    Enum,
    Reference,
}

impl ValueType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Enum => "enum",
            Self::Reference => "reference",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    ReadOnly,
    Writable,
}

impl Access {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Writable => "writable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationTiming {
    SessionReset,
    PackageReload,
}

impl MutationTiming {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionReset => "session_reset",
            Self::PackageReload => "package_reload",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalContract {
    pub dispatch_phase: String,
    pub dispatch_key: String,
    pub ordinary_producer: String,
    pub handler: String,
    pub same_pass_epilogue: String,
    pub following_pass_cleanup: String,
    pub authoritative_reads: Vec<String>,
    pub authoritative_writes: Vec<String>,
    pub rng_and_event_effects: String,
}

impl TemporalContract {
    fn to_json(&self) -> Value {
        json!({
            "dispatch_phase": self.dispatch_phase,
            "dispatch_key": self.dispatch_key,
            "ordinary_producer": self.ordinary_producer,
            "handler": self.handler,
            "same_pass_epilogue": self.same_pass_epilogue,
            "following_pass_cleanup": self.following_pass_cleanup,
            "authoritative_reads": self.authoritative_reads,
            "authoritative_writes": self.authoritative_writes,
            "rng_and_event_effects": self.rng_and_event_effects,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDescriptor {
    pub id: SemanticId,
    pub descriptor_version: u16,
    pub kind: DescriptorKind,
    pub value_type: ValueType,
    pub unit: Option<String>,
    pub default_value: SemanticValue,
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub enum_values: Vec<String>,
    pub access: Access,
    pub mutation_timing: MutationTiming,
    pub determinism_role: String,
    pub dependencies: Vec<String>,
    pub side_effects: Vec<String>,
    pub scope: String,
    pub provenance_origin: String,
    pub provenance_content_hash: String,
    pub temporal_contract: Option<TemporalContract>,
    pub summary: String,
}

impl SemanticDescriptor {
    #[must_use]
    pub fn to_json(&self, effective_value: Option<&SemanticValue>) -> Value {
        json!({
            "id": self.id.as_str(),
            "descriptor_version": self.descriptor_version,
            "kind": self.kind.as_str(),
            "value_type": self.value_type.as_str(),
            "unit": self.unit,
            "default_value": self.default_value.to_json(),
            "effective_value": effective_value.map(SemanticValue::to_json),
            "bounds_or_enum": {
                "minimum": self.minimum,
                "maximum": self.maximum,
                "values": self.enum_values,
            },
            "access": self.access.as_str(),
            "mutation_timing": self.mutation_timing.as_str(),
            "determinism_role": self.determinism_role,
            "dependencies": self.dependencies,
            "side_effects": self.side_effects,
            "scope": self.scope,
            "provenance": {
                "origin": self.provenance_origin,
                "content_hash": self.provenance_content_hash,
            },
            "temporal_contract": self.temporal_contract.as_ref().map(TemporalContract::to_json),
            "deprecation": Value::Null,
            "summary": self.summary,
            "examples": [self.default_value.to_json()],
        })
    }

    pub(crate) fn validate_value(&self, value: &SemanticValue, path: &str) -> Result<()> {
        match (self.value_type, value) {
            (ValueType::Integer, SemanticValue::Integer(value)) => {
                if self.minimum.is_some_and(|minimum| *value < minimum)
                    || self.maximum.is_some_and(|maximum| *value > maximum)
                {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::OutOfRange,
                        path,
                        "integer semantic value is outside descriptor bounds",
                    ));
                }
            }
            (ValueType::Enum, SemanticValue::Enum(value)) => {
                if !self.enum_values.iter().any(|candidate| candidate == value) {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::OutOfRange,
                        path,
                        "enum semantic value is not declared by its descriptor",
                    ));
                }
            }
            (ValueType::Reference, SemanticValue::Reference(value)) => {
                super::lower_content_hash(value, path)?;
            }
            _ => {
                return Err(DefinitionError::new(
                    DefinitionErrorCode::TypeMismatch,
                    path,
                    "semantic value type does not match descriptor",
                ));
            }
        }
        Ok(())
    }
}

pub type ReadBinding = fn(&GameDefinition, &SemanticId) -> Result<SemanticValue>;
pub type ValidateBinding = fn(&GameDefinition, &SemanticId, &SemanticValue) -> Result<()>;
pub type ApplyBinding =
    fn(&GameDefinition, &SemanticId, &SemanticValue, u64) -> Result<GameDefinition>;
pub type ReadBackBinding = fn(&GameDefinition, &SemanticId) -> Result<SemanticValue>;

#[derive(Clone, Debug)]
pub struct SemanticBinding {
    pub id: SemanticId,
    pub read: Option<ReadBinding>,
    pub validate: Option<ValidateBinding>,
    pub apply: Option<ApplyBinding>,
    pub read_back: Option<ReadBackBinding>,
}

impl SemanticBinding {
    #[cfg(test)]
    fn read_only(id: &str) -> Result<Self> {
        Ok(Self {
            id: SemanticId::parse(id)?,
            read: Some(read_definition_value),
            validate: None,
            apply: None,
            read_back: None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SemanticCatalog {
    descriptors: BTreeMap<SemanticId, SemanticDescriptor>,
    bindings: BTreeMap<SemanticId, SemanticBinding>,
    catalog_hash: String,
}

impl SemanticCatalog {
    /// Validate descriptors and bindings as two independent sets.
    ///
    /// # Errors
    ///
    /// Duplicate IDs, missing rows, catalog-only rows, undiscoverable
    /// setters, and incomplete writable callback sets fail closed.
    pub fn try_from_parts(
        descriptors: Vec<SemanticDescriptor>,
        bindings: Vec<SemanticBinding>,
    ) -> Result<Self> {
        let descriptor_count = descriptors.len();
        let binding_count = bindings.len();
        let descriptors = descriptors
            .into_iter()
            .map(|descriptor| (descriptor.id.clone(), descriptor))
            .collect::<BTreeMap<_, _>>();
        let bindings = bindings
            .into_iter()
            .map(|binding| (binding.id.clone(), binding))
            .collect::<BTreeMap<_, _>>();
        if descriptors.len() != descriptor_count {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DuplicateDescriptor,
                "catalog.descriptors",
                "semantic catalog contains a duplicate descriptor ID",
            ));
        }
        if bindings.len() != binding_count {
            return Err(DefinitionError::new(
                DefinitionErrorCode::DuplicateBinding,
                "catalog.bindings",
                "semantic catalog contains a duplicate binding ID",
            ));
        }
        let descriptor_ids = descriptors.keys().collect::<BTreeSet<_>>();
        let binding_ids = bindings.keys().collect::<BTreeSet<_>>();
        if descriptor_ids != binding_ids {
            return Err(DefinitionError::new(
                DefinitionErrorCode::CatalogBindingMismatch,
                "catalog",
                "descriptor and binding ID sets differ",
            ));
        }
        for (id, descriptor) in &descriptors {
            let binding = &bindings[id];
            let read_present = binding.read.is_some();
            let write_complete = binding.validate.is_some()
                && binding.apply.is_some()
                && binding.read_back.is_some();
            match descriptor.access {
                Access::ReadOnly
                    if read_present
                        && !write_complete
                        && binding.validate.is_none()
                        && binding.apply.is_none()
                        && binding.read_back.is_none() => {}
                Access::Writable if read_present && write_complete => {}
                Access::ReadOnly => {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::UndiscoverableSetter,
                        id.as_str(),
                        "read-only descriptor has a setter or lacks its read binding",
                    ));
                }
                Access::Writable => {
                    return Err(DefinitionError::new(
                        DefinitionErrorCode::IncompleteWritableBinding,
                        id.as_str(),
                        "writable descriptor requires read, validate, apply, and read-back bindings",
                    ));
                }
            }
        }
        let canonical = Value::Array(
            descriptors
                .values()
                .map(|descriptor| descriptor.to_json(None))
                .collect(),
        );
        let catalog_hash = format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(&canonical).expect("catalog JSON is serializable"))
        );
        Ok(Self {
            descriptors,
            bindings,
            catalog_hash,
        })
    }

    #[must_use]
    pub fn catalog_hash(&self) -> &str {
        &self.catalog_hash
    }

    #[must_use]
    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &SemanticDescriptor> {
        self.descriptors.values()
    }

    #[must_use]
    pub fn descriptor(&self, id: &SemanticId) -> Option<&SemanticDescriptor> {
        self.descriptors.get(id)
    }

    /// Read one effective value through its registered binding.
    ///
    /// # Errors
    ///
    /// Returns a typed unknown-ID or missing-binding error.
    pub fn read(&self, definition: &GameDefinition, id: &SemanticId) -> Result<SemanticValue> {
        let binding = self.bindings.get(id).ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "semantic ID is not present in the binding catalog",
            )
        })?;
        binding.read.ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::CatalogBindingMismatch,
                id.as_str(),
                "semantic binding has no read implementation",
            )
        })?(definition, id)
    }

    pub(crate) fn validate_replacement(
        &self,
        definition: &GameDefinition,
        id: &SemanticId,
        value: &SemanticValue,
    ) -> Result<()> {
        self.validate_replacement_shape(definition, id, value)?;
        let binding = &self.bindings[id];
        binding.validate.ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::IncompleteWritableBinding,
                id.as_str(),
                "writable descriptor lacks its validator",
            )
        })?(definition, id, value)
    }

    pub(crate) fn validate_replacement_shape(
        &self,
        definition: &GameDefinition,
        id: &SemanticId,
        value: &SemanticValue,
    ) -> Result<()> {
        let descriptor = self.descriptor(id).ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "semantic ID is not present in the catalog",
            )
        })?;
        if descriptor.access != Access::Writable {
            return Err(DefinitionError::new(
                DefinitionErrorCode::InvalidRequest,
                id.as_str(),
                "semantic descriptor is read-only at this engine revision",
            ));
        }
        descriptor.validate_value(value, id.as_str())?;
        if definition.value(id).is_none() {
            return Err(DefinitionError::new(
                DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "active definition contains no value for writable descriptor",
            ));
        }
        Ok(())
    }

    pub(crate) fn apply_replacement(
        &self,
        definition: &GameDefinition,
        id: &SemanticId,
        value: &SemanticValue,
        revision: u64,
    ) -> Result<GameDefinition> {
        let binding = self.bindings.get(id).ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::UnknownSemanticId,
                id.as_str(),
                "semantic ID is not present in the binding catalog",
            )
        })?;
        let candidate = binding.apply.ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::IncompleteWritableBinding,
                id.as_str(),
                "writable descriptor lacks its apply binding",
            )
        })?(definition, id, value, revision)?;
        let read_back = binding.read_back.ok_or_else(|| {
            DefinitionError::new(
                DefinitionErrorCode::IncompleteWritableBinding,
                id.as_str(),
                "writable descriptor lacks its read-back binding",
            )
        })?(&candidate, id)?;
        if &read_back != value {
            return Err(DefinitionError::new(
                DefinitionErrorCode::CatalogBindingMismatch,
                id.as_str(),
                "writable binding read-back differs from the requested replacement",
            ));
        }
        Ok(candidate)
    }
}

fn read_definition_value(definition: &GameDefinition, id: &SemanticId) -> Result<SemanticValue> {
    definition.value(id).cloned().ok_or_else(|| {
        DefinitionError::new(
            DefinitionErrorCode::UnknownSemanticId,
            id.as_str(),
            "definition contains no effective value for semantic ID",
        )
    })
}

fn validate_definition_replacement(
    _: &GameDefinition,
    _: &SemanticId,
    _: &SemanticValue,
) -> Result<()> {
    Ok(())
}

fn validate_player_max_forward_speed(
    _: &GameDefinition,
    id: &SemanticId,
    value: &SemanticValue,
) -> Result<()> {
    const SOURCE_SPEED_STEP: i64 = 16;
    let SemanticValue::Integer(value) = value else {
        return Err(DefinitionError::new(
            DefinitionErrorCode::TypeMismatch,
            id.as_str(),
            "player maximum forward speed is not an integer",
        ));
    };
    if value % SOURCE_SPEED_STEP != 0 {
        return Err(DefinitionError::new(
            DefinitionErrorCode::OutOfRange,
            id.as_str(),
            "player maximum forward speed must be a multiple of the source speed step (16)",
        ));
    }
    Ok(())
}

fn validate_presentation_asset_replacement(
    definition: &GameDefinition,
    id: &SemanticId,
    value: &SemanticValue,
) -> Result<()> {
    definition.validate_presentation_asset_value(id, value)
}

fn apply_definition_replacement(
    definition: &GameDefinition,
    id: &SemanticId,
    value: &SemanticValue,
    revision: u64,
) -> Result<GameDefinition> {
    definition.with_replacement(id, value.clone(), revision)
}

fn writable_definition_binding(id: &str) -> Result<SemanticBinding> {
    Ok(SemanticBinding {
        id: SemanticId::parse(id)?,
        read: Some(read_definition_value),
        validate: Some(validate_definition_replacement),
        apply: Some(apply_definition_replacement),
        read_back: Some(read_definition_value),
    })
}

fn writable_player_max_forward_speed_binding() -> Result<SemanticBinding> {
    Ok(SemanticBinding {
        id: SemanticId::parse("flight.player.max_forward_speed")?,
        read: Some(read_definition_value),
        validate: Some(validate_player_max_forward_speed),
        apply: Some(apply_definition_replacement),
        read_back: Some(read_definition_value),
    })
}

fn writable_presentation_asset_binding(id: &str) -> Result<SemanticBinding> {
    Ok(SemanticBinding {
        id: SemanticId::parse(id)?,
        read: Some(read_definition_value),
        validate: Some(validate_presentation_asset_replacement),
        apply: Some(apply_definition_replacement),
        read_back: Some(read_definition_value),
    })
}

fn descriptor(
    id: &str,
    kind: DescriptorKind,
    value_type: ValueType,
    unit: Option<&str>,
    default_value: SemanticValue,
    bounds: (Option<i64>, Option<i64>),
    enum_values: &[&str],
    timing: MutationTiming,
    origin: &str,
    origin_hash: &str,
    temporal_contract: Option<TemporalContract>,
    summary: &str,
) -> Result<SemanticDescriptor> {
    Ok(SemanticDescriptor {
        id: SemanticId::parse(id)?,
        descriptor_version: 1,
        kind,
        value_type,
        unit: unit.map(str::to_owned),
        default_value,
        minimum: bounds.0,
        maximum: bounds.1,
        enum_values: enum_values
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        access: Access::ReadOnly,
        mutation_timing: timing,
        determinism_role: "identity".to_owned(),
        dependencies: Vec::new(),
        side_effects: vec!["existing_sessions_require_explicit_reset_or_reopen".to_owned()],
        scope: if kind == DescriptorKind::AssetSlot {
            "presentation"
        } else {
            "package"
        }
        .to_owned(),
        provenance_origin: origin.to_owned(),
        provenance_content_hash: origin_hash.to_owned(),
        temporal_contract,
        summary: summary.to_owned(),
    })
}

/// Build the bounded F2 catalog.
///
/// # Errors
///
/// Returns an error if the static descriptor/binding relationship is invalid.
pub fn default_catalog() -> Result<SemanticCatalog> {
    let mut descriptors = vec![
        descriptor(
            "ai.enemy.default.policy",
            DescriptorKind::Behavior,
            ValueType::Enum,
            None,
            SemanticValue::Enum("classic_goat_v1".to_owned()),
            (None, None),
            &["classic_goat_v1", "vigilant_goat_v1"],
            MutationTiming::SessionReset,
            "sim-core/goat-config-and-source-scheduler",
            "sha256:8d57a0ecf38907f10c619f4e0948a3e81c5ab5ec0a06f3ff8ddce9f572605e01",
            Some(TemporalContract {
                dispatch_phase: "update_entities_source_pass".to_owned(),
                dispatch_key: "enemy_creature_policy".to_owned(),
                ordinary_producer: "validated creature reset rows".to_owned(),
                handler: "perceive_then_decide_then_act in ascending source-slot order".to_owned(),
                same_pass_epilogue: "phase advance and source audio/event tail".to_owned(),
                following_pass_cleanup: "drawing-disabled reclaim before next dispatch".to_owned(),
                authoritative_reads: vec![
                    "rust_entity_state".to_owned(),
                    "rust_player_targets".to_owned(),
                    "rust_world_materials".to_owned(),
                ],
                authoritative_writes: vec![
                    "rust_entity_action_target_motion".to_owned(),
                    "rust_event_journal".to_owned(),
                ],
                rng_and_event_effects: "ordered per-entity RNG draws and semantic event/audio rows"
                    .to_owned(),
            }),
            "Current bounded enemy policy implemented by the ordinary creature lifecycle.",
        )?,
        descriptor(
            "flight.player.max_forward_speed",
            DescriptorKind::Tuning,
            ValueType::Integer,
            Some("source_velocity_units_per_entity_pass"),
            SemanticValue::Integer(80),
            (Some(0), Some(32_752)),
            &[],
            MutationTiming::SessionReset,
            "sim-core/flight-source-pin",
            "sha256:171fed6decdb5bf26b89d8067ed6ace8912e6e3baee7353e98f9cb1ec9955be3",
            Some(TemporalContract {
                dispatch_phase: "player_input_decode".to_owned(),
                dispatch_key: "forward_motion".to_owned(),
                ordinary_producer: "latched player movement command".to_owned(),
                handler: "decode movement target-speed clamp".to_owned(),
                same_pass_epilogue: "entity flight integration".to_owned(),
                following_pass_cleanup: "next command latch or reset".to_owned(),
                authoritative_reads: vec!["rust_player_flight_state".to_owned()],
                authoritative_writes: vec!["rust_target_speed".to_owned()],
                rng_and_event_effects: "none".to_owned(),
            }),
            "Maximum forward target speed consumed by the ordinary player flight path.",
        )?,
        descriptor(
            "presentation.spell.fireball.projectile_asset",
            DescriptorKind::AssetSlot,
            ValueType::Reference,
            None,
            SemanticValue::Reference(
                "sha256:f6755ec0f9931b6748c32893cc11838fb398043dafdc962eb5e288105cf0c88d"
                    .to_owned(),
            ),
            (None, None),
            &[],
            MutationTiming::PackageReload,
            "sim-core/typed-indexed-presentation-asset",
            "sha256:9efbccbc4aa610289331328dc899eba0f9f9c2c2d18c5e56c3cf8396e64c0024",
            None,
            "Indexed projectile frame selected by the ordinary presentation binding.",
        )?,
        descriptor(
            "spell.fireball.mana_cost",
            DescriptorKind::Rule,
            ValueType::Integer,
            Some("mana_units"),
            SemanticValue::Integer(100),
            (Some(0), Some(i64::from(i32::MAX))),
            &[],
            MutationTiming::SessionReset,
            "sim-core/spell-holder-activation",
            "sha256:c2b6726b9d3325bc5287b7c262bb7559e0862286ed0a884c65c804417d1ae3ca",
            Some(TemporalContract {
                dispatch_phase: "spell_activation".to_owned(),
                dispatch_key: "spell_family_fireball".to_owned(),
                ordinary_producer: "owned player spell holder".to_owned(),
                handler: "shared mana gate then ordinary cast dispatch".to_owned(),
                same_pass_epilogue: "accepted activation enters projectile constructor".to_owned(),
                following_pass_cleanup: "holder countdown and projectile lifecycle".to_owned(),
                authoritative_reads: vec![
                    "rust_caster_mana".to_owned(),
                    "rust_holder_cost".to_owned(),
                ],
                authoritative_writes: vec![
                    "rust_holder_activation".to_owned(),
                    "rust_event_journal".to_owned(),
                ],
                rng_and_event_effects: "no RNG in cost gate; accepted cast emits ordered effects"
                    .to_owned(),
            }),
            "Mana affordability threshold consumed by the ordinary cast path.",
        )?,
    ];
    let flight_id = SemanticId::parse("flight.player.max_forward_speed")?;
    let enemy_policy_id = SemanticId::parse("ai.enemy.default.policy")?;
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.id == enemy_policy_id)
        .expect("static enemy-policy descriptor exists")
        .access = Access::Writable;
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.id == flight_id)
        .expect("static flight descriptor exists")
        .access = Access::Writable;
    let fireball_cost_id = SemanticId::parse("spell.fireball.mana_cost")?;
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.id == fireball_cost_id)
        .expect("static Fireball cost descriptor exists")
        .access = Access::Writable;
    let projectile_asset_id = SemanticId::parse("presentation.spell.fireball.projectile_asset")?;
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.id == projectile_asset_id)
        .expect("static projectile-asset descriptor exists")
        .access = Access::Writable;
    let bindings = vec![
        writable_definition_binding("ai.enemy.default.policy")?,
        writable_player_max_forward_speed_binding()?,
        writable_presentation_asset_binding("presentation.spell.fireball.projectile_asset")?,
        writable_definition_binding("spell.fireball.mana_cost")?,
    ];
    SemanticCatalog::try_from_parts(descriptors, bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_validate(_: &GameDefinition, _: &SemanticId, _: &SemanticValue) -> Result<()> {
        Ok(())
    }

    fn noop_apply(
        definition: &GameDefinition,
        _: &SemanticId,
        _: &SemanticValue,
        _: u64,
    ) -> Result<GameDefinition> {
        Ok(definition.clone())
    }

    #[test]
    fn catalog_rejects_duplicate_missing_and_undiscoverable_bindings() {
        let catalog = default_catalog().expect("valid bounded catalog");
        assert_eq!(catalog.descriptors().len(), 4);

        let descriptor = catalog
            .descriptors()
            .next()
            .expect("one descriptor")
            .clone();
        let duplicate = SemanticCatalog::try_from_parts(
            vec![descriptor.clone(), descriptor.clone()],
            vec![SemanticBinding::read_only(descriptor.id.as_str()).unwrap()],
        );
        assert_eq!(
            duplicate.unwrap_err().code,
            DefinitionErrorCode::DuplicateDescriptor
        );

        let missing = SemanticCatalog::try_from_parts(vec![descriptor.clone()], Vec::new());
        assert_eq!(
            missing.unwrap_err().code,
            DefinitionErrorCode::CatalogBindingMismatch
        );

        let mut read_only_descriptor = descriptor;
        read_only_descriptor.access = Access::ReadOnly;
        let mut read_only_with_setter =
            SemanticBinding::read_only(read_only_descriptor.id.as_str()).unwrap();
        read_only_with_setter.validate = Some(noop_validate);
        read_only_with_setter.apply = Some(noop_apply);
        read_only_with_setter.read_back = Some(read_definition_value);
        let undiscoverable = SemanticCatalog::try_from_parts(
            vec![read_only_descriptor],
            vec![read_only_with_setter],
        );
        assert_eq!(
            undiscoverable.unwrap_err().code,
            DefinitionErrorCode::UndiscoverableSetter
        );
    }
}
