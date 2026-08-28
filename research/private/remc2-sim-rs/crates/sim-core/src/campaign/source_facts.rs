//! Immutable, hash-bound campaign inputs admitted by package schema v1.
//!
//! These are data facts, not presentation resources and not executable
//! package content. The validated campaign profile determines which of the
//! closed fact kinds have real package carriers.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Stable schema-v1 SourceFact discriminants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum SourceFactKind {
    AuthoredLevelRecords = 1,
    FeatureConstructionInputs = 2,
    CollisionMask = 3,
    BuildingParameters = 4,
}

impl SourceFactKind {
    fn from_u64(value: u64) -> Result<Self, String> {
        match value {
            1 => Ok(Self::AuthoredLevelRecords),
            2 => Ok(Self::FeatureConstructionInputs),
            3 => Ok(Self::CollisionMask),
            4 => Ok(Self::BuildingParameters),
            _ => Err(format!("unknown SourceFact kind {value}")),
        }
    }
}

/// Closed source-field vocabulary for schema-v1 selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceField {
    Par1_14,
}

/// One authored selector that chose a bounded immutable fact row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorProvenance {
    pub authored_record: u16,
    pub source_class: u8,
    pub source_model: u8,
    pub source_subtype: u8,
    pub source_field: SourceField,
    pub selector_value: u32,
}

/// Owner artifact identity behind an immutable fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArtifact {
    pub logical_path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Fully validated schema-v1 SourceFact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFact {
    pub fact_id: String,
    pub kind: SourceFactKind,
    pub version: u16,
    pub member: String,
    pub member_sha256: String,
    pub selector_provenance: Vec<SelectorProvenance>,
    pub source_artifacts: Vec<SourceArtifact>,
}

pub(crate) struct SourceFactContext<'a> {
    pub files: &'a BTreeMap<String, (u64, String)>,
    pub members: &'a BTreeMap<String, Vec<u8>>,
    pub authored_collision_provenance: &'a [SelectorProvenance],
    pub building_objective_provenance: &'a [SelectorProvenance],
    /// Census-profiled identity of the collision bank when the selected owner
    /// package actually carries one.
    pub collision_bank: Option<(usize, &'a str)>,
    pub has_building_parameters: bool,
}

const SOURCE_ARTIFACTS: &[(&str, u64, &str)] = &[
    (
        "CD_Files/DATA/BLDGPRM.DAT",
        304,
        "1c4a7fdef5d4b811f871441f0380faaacd6e5e689e73fc4628bf8c5b51ea6f00",
    ),
    (
        "CD_Files/DATA/BUILD0-0.DAT",
        80_064,
        "3db5d9a763dedafbe3077eeeef47431da0b5d397e311f6ee46719a0fc51e8e5d",
    ),
    (
        "CD_Files/DATA/BUILD0-0.TAB",
        462,
        "e064a1520e96f65c13fde9a05918d240ee4f708ab6aefc793ec8b391f220c05e",
    ),
    (
        "CD_Files/DATA/MSPRD0-0.DAT",
        49_202,
        "a81783458f1b81da262d60bc1f4bc28012137437f2fcedb75af441a4fc777656",
    ),
    (
        "CD_Files/DATA/MSPRD0-0.TAB",
        1_572,
        "47fe03bbd99e1975755a8e47401977965b5ef63fe427a755a2f481ed393117ea",
    ),
    (
        "CD_Files/DATA/SPELLS.DAT",
        2_080,
        "7b426f0104f282d91bf7da904ed978e809135e12e0dfebd845e4a582d52b51cc",
    ),
    (
        "CD_Files/DATA/TABLESN.DAT",
        83_456,
        "f88974baf68f728fb534a4dab35397b9cb3ca2f424019bbde36858eebbf97495",
    ),
    (
        "CD_Files/LEVELS/LEVELS.DAT",
        3_063_856,
        "3b7fae5d7c350486a4f62ad4b2e9fa2e4e52023a216b8a7f043ce02439b50222",
    ),
    (
        "CD_Files/LEVELS/LEVELS.TAB",
        4_000,
        "179a65f04d4aa7bc147cfd56ffb63a0da400dfd85542eb0fbda10265d6bc1bea",
    ),
    (
        "GAME/NETHERW/CLEVELS/LEVELS.DAT",
        3_063_856,
        "3b7fae5d7c350486a4f62ad4b2e9fa2e4e52023a216b8a7f043ce02439b50222",
    ),
    (
        "GAME/NETHERW/CLEVELS/LEVELS.TAB",
        4_000,
        "179a65f04d4aa7bc147cfd56ffb63a0da400dfd85542eb0fbda10265d6bc1bea",
    ),
];

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} is not an object"))?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{label} field set differs"));
    }
    Ok(object)
}

fn bounded_u64(value: &Value, maximum: u64, label: &str) -> Result<u64, String> {
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{label} is not an unsigned integer"))?;
    if value > maximum {
        return Err(format!("{label} exceeds its bound"));
    }
    Ok(value)
}

fn string<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} is not a string"))
}

fn lower_sha256(value: &Value, label: &str) -> Result<String, String> {
    let value = string(value, label)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} is not a lowercase SHA-256"));
    }
    Ok(value.to_owned())
}

fn normalized_source_path(value: &Value, label: &str) -> Result<String, String> {
    let value = string(value, label)?;
    if value.is_empty()
        || !value.is_ascii()
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(format!("{label} is absolute, traversing, or noncanonical"));
    }
    Ok(value.to_owned())
}

fn read_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16, String> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{label} is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn parse_selector_provenance(value: &Value, label: &str) -> Result<SelectorProvenance, String> {
    let object = exact_object(
        value,
        &[
            "authored_record",
            "selector_value",
            "source_class",
            "source_field",
            "source_model",
            "source_subtype",
        ],
        label,
    )?;
    if string(&object["source_field"], &format!("{label}.source_field"))? != "par1_14" {
        return Err(format!("{label} uses an unknown source field"));
    }
    Ok(SelectorProvenance {
        authored_record: u16::try_from(bounded_u64(
            &object["authored_record"],
            u64::from(u16::MAX),
            &format!("{label}.authored_record"),
        )?)
        .expect("bounded u16"),
        source_class: u8::try_from(bounded_u64(
            &object["source_class"],
            u64::from(u8::MAX),
            &format!("{label}.source_class"),
        )?)
        .expect("bounded u8"),
        source_model: u8::try_from(bounded_u64(
            &object["source_model"],
            u64::from(u8::MAX),
            &format!("{label}.source_model"),
        )?)
        .expect("bounded u8"),
        source_subtype: u8::try_from(bounded_u64(
            &object["source_subtype"],
            u64::from(u8::MAX),
            &format!("{label}.source_subtype"),
        )?)
        .expect("bounded u8"),
        source_field: SourceField::Par1_14,
        selector_value: u32::try_from(bounded_u64(
            &object["selector_value"],
            u64::from(u32::MAX),
            &format!("{label}.selector_value"),
        )?)
        .expect("bounded u32"),
    })
}

fn parse_artifact(value: &Value, label: &str) -> Result<SourceArtifact, String> {
    let object = exact_object(value, &["bytes", "logical_path", "sha256"], label)?;
    let artifact = SourceArtifact {
        logical_path: normalized_source_path(
            &object["logical_path"],
            &format!("{label}.logical_path"),
        )?,
        bytes: bounded_u64(&object["bytes"], u64::MAX, &format!("{label}.bytes"))?,
        sha256: lower_sha256(&object["sha256"], &format!("{label}.sha256"))?,
    };
    let expected = SOURCE_ARTIFACTS
        .iter()
        .find(|(path, _, _)| *path == artifact.logical_path)
        .ok_or_else(|| format!("{label} names an unproven owner artifact"))?;
    if artifact.bytes != expected.1 || artifact.sha256 != expected.2 {
        return Err(format!("{label} owner artifact identity changed"));
    }
    Ok(artifact)
}

fn decode_feature_construction_inputs(bytes: &[u8]) -> Result<Vec<u16>, String> {
    if bytes.len() < 16
        || bytes.get(..4) != Some(b"MCI3")
        || read_u16(bytes, 4, "feature-input version")? != 1
        || read_u16(bytes, 6, "feature-input header")? != 16
        || read_u16(bytes, 10, "feature-input encoding")? != 1
        || read_u32(bytes, 12, "feature-input reserved")? != 0
    {
        return Err("feature construction input header changed".to_owned());
    }
    let count = usize::from(read_u16(bytes, 8, "feature-input count")?);
    let mut selectors = Vec::with_capacity(count);
    let mut cursor = 16_usize;
    for _ in 0..count {
        let selector = read_u16(bytes, cursor, "feature-input selector")?;
        let width = *bytes
            .get(cursor + 6)
            .ok_or_else(|| "feature-input width is truncated".to_owned())?;
        let height = *bytes
            .get(cursor + 7)
            .ok_or_else(|| "feature-input height is truncated".to_owned())?;
        let extent = usize::try_from(read_u32(bytes, cursor + 8, "feature-input extent")?)
            .expect("u32 fits usize");
        if selectors
            .last()
            .is_some_and(|previous| *previous >= selector)
            || width == 0
            || height == 0
            || extent != usize::from(width) * usize::from(height) * 2
        {
            return Err("feature construction selector/order/extent changed".to_owned());
        }
        cursor = cursor
            .checked_add(12 + extent)
            .ok_or_else(|| "feature construction extent overflow".to_owned())?;
        if cursor > bytes.len() {
            return Err("feature construction payload is truncated".to_owned());
        }
        selectors.push(selector);
    }
    if cursor != bytes.len() {
        return Err("feature construction member has trailing bytes".to_owned());
    }
    Ok(selectors)
}

fn decode_collision_masks(bytes: &[u8], expected: &[u16]) -> Result<(), String> {
    if bytes.len() < 20
        || bytes.get(..4) != Some(b"MCM1")
        || read_u16(bytes, 4, "collision version")? != 1
        || read_u16(bytes, 6, "collision header")? != 20
        || read_u16(bytes, 8, "collision VGA selector")? != 1
        || read_u16(bytes, 10, "collision reserved")? != 0
        || read_u32(bytes, 16, "collision record header")? != 12
    {
        return Err("collision-mask header changed".to_owned());
    }
    let count = usize::try_from(read_u32(bytes, 12, "collision count")?).expect("u32 fits usize");
    let mut selectors = Vec::with_capacity(count);
    let mut cursor = 20_usize;
    for _ in 0..count {
        let selector = read_u16(bytes, cursor, "collision selector")?;
        let width = read_u16(bytes, cursor + 2, "collision width")?;
        let height = read_u16(bytes, cursor + 4, "collision height")?;
        let reserved = read_u16(bytes, cursor + 6, "collision reserved")?;
        let extent = usize::try_from(read_u32(bytes, cursor + 8, "collision extent")?)
            .expect("u32 fits usize");
        if selectors
            .last()
            .is_some_and(|previous| *previous >= selector)
            || reserved != 0
            || width == 0
            || height == 0
            || extent != usize::from(width) * usize::from(height) * 2
        {
            return Err("collision-mask selector/order/extent changed".to_owned());
        }
        cursor = cursor
            .checked_add(12 + extent)
            .ok_or_else(|| "collision-mask extent overflow".to_owned())?;
        if cursor > bytes.len() {
            return Err("collision-mask payload is truncated".to_owned());
        }
        selectors.push(selector);
    }
    if cursor != bytes.len() {
        return Err("collision-mask member has trailing bytes".to_owned());
    }
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    if selectors != expected
        || (expected.is_empty() && (count != 0 || bytes.len() != 20))
        || (!expected.is_empty() && count == 0)
    {
        return Err(
            "collision-mask empty/nonempty carrier differs from authored selectors".to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn decode_building_parameters(
    bytes: &[u8],
) -> Result<BTreeMap<u8, crate::house::HouseParameter>, String> {
    const RECORD_COUNT: usize = 76;
    const RECORD_BYTES: usize = 4;
    if bytes.len() != RECORD_COUNT * RECORD_BYTES {
        return Err("building parameter owner extent changed".to_owned());
    }
    let mut result = BTreeMap::new();
    for index in 0..RECORD_COUNT {
        let offset = index * RECORD_BYTES;
        let selector = u8::try_from(index).expect("BLDGPRM selector fits u8");
        let parameter = crate::house::HouseParameter::from_source_record(
            u16::from_le_bytes([bytes[offset], bytes[offset + 1]]),
            bytes[offset + 2],
            bytes[offset + 3],
        );
        result.insert(selector, parameter);
    }
    Ok(result)
}

pub(crate) fn building_parameter_chain(
    parameters: &BTreeMap<u8, crate::house::HouseParameter>,
    root: u8,
) -> Result<Box<[u8]>, String> {
    let mut selector = root;
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    while selector != 0 {
        if chain.len() == 8 || !seen.insert(selector) {
            return Err("BLDGPRM chain cycles or exceeds eight rows".to_owned());
        }
        chain.push(selector);
        selector = parameters
            .get(&selector)
            .ok_or_else(|| format!("BLDGPRM chain references missing row {selector}"))?
            .destruction_successor;
    }
    if chain.is_empty() {
        return Err("BLDGPRM chain is empty".to_owned());
    }
    Ok(chain.into_boxed_slice())
}

/// Parse and validate the complete, sorted SourceFact set for the package's
/// real immutable carriers.
pub(crate) fn validate_source_facts(
    value: &Value,
    context: &SourceFactContext<'_>,
) -> Result<Vec<SourceFact>, String> {
    let rows = value
        .as_array()
        .ok_or_else(|| "source_facts is not an array".to_owned())?;
    if context.has_building_parameters
        && !matches!(
            context.building_objective_provenance,
            [SelectorProvenance {
                authored_record: 153,
                source_class: 10,
                source_model: 21,
                source_subtype: 45,
                source_field: SourceField::Par1_14,
                selector_value: 21,
            }]
        )
    {
        return Err(
            "standing-building objective provenance differs from the qualified Level 2 binding"
                .to_owned(),
        );
    }
    let mut expected = Vec::new();
    if context.has_building_parameters {
        expected.push((
            "building-parameters",
            SourceFactKind::BuildingParameters,
            "building-parameters.bin",
            &["CD_Files/DATA/BLDGPRM.DAT"][..],
        ));
    }
    if context.collision_bank.is_some() {
        expected.push((
            "collision-mask-bank",
            SourceFactKind::CollisionMask,
            "collision-masks.bin",
            &["CD_Files/DATA/MSPRD0-0.DAT", "CD_Files/DATA/MSPRD0-0.TAB"][..],
        ));
    }
    expected.extend([
        (
            "feature-construction-inputs",
            SourceFactKind::FeatureConstructionInputs,
            "feature-construction-inputs.bin",
            &[
                "CD_Files/DATA/BUILD0-0.DAT",
                "CD_Files/DATA/BUILD0-0.TAB",
                "CD_Files/DATA/SPELLS.DAT",
                "CD_Files/DATA/TABLESN.DAT",
            ][..],
        ),
        (
            "level-records",
            SourceFactKind::AuthoredLevelRecords,
            "authored-level.bin",
            &[
                "GAME/NETHERW/CLEVELS/LEVELS.DAT",
                "GAME/NETHERW/CLEVELS/LEVELS.TAB",
            ][..],
        ),
    ]);
    if rows.len() != expected.len() {
        return Err(format!(
            "campaign profile requires exactly {} SourceFacts",
            expected.len()
        ));
    }
    let mut result = Vec::with_capacity(rows.len());
    let mut member_claims = BTreeMap::<String, String>::new();
    for (index, (raw, expected)) in rows.iter().zip(expected).enumerate() {
        let label = format!("source_facts[{index}]");
        let object = exact_object(
            raw,
            &[
                "fact_id",
                "kind",
                "lifetime",
                "member",
                "member_sha256",
                "selector_provenance",
                "source_artifacts",
                "version",
            ],
            &label,
        )?;
        let fact_id = string(&object["fact_id"], &format!("{label}.fact_id"))?;
        if fact_id != expected.0
            || fact_id.is_empty()
            || !fact_id.is_ascii()
            || !fact_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(format!("{label} id/order differs"));
        }
        let kind = SourceFactKind::from_u64(bounded_u64(
            &object["kind"],
            u64::from(u16::MAX),
            &format!("{label}.kind"),
        )?)?;
        let version = bounded_u64(
            &object["version"],
            u64::from(u16::MAX),
            &format!("{label}.version"),
        )?;
        if kind != expected.1
            || version != 1
            || string(&object["lifetime"], &format!("{label}.lifetime"))?
                != "immutable-through-session"
        {
            return Err(format!("{label} kind/version/lifetime differs"));
        }
        let member = string(&object["member"], &format!("{label}.member"))?;
        if member != expected.2 {
            return Err(format!("{label} member differs"));
        }
        let member_sha256 =
            lower_sha256(&object["member_sha256"], &format!("{label}.member_sha256"))?;
        let outer_hash = context
            .files
            .get(member)
            .ok_or_else(|| format!("{label} references an undeclared member"))?
            .1
            .as_str();
        if member_sha256 != outer_hash {
            return Err(format!("{label} member hash differs from outer file map"));
        }
        if let Some(previous) = member_claims.insert(member.to_owned(), member_sha256.clone())
            && previous != member_sha256
        {
            return Err(format!("{label} conflicts with another member claim"));
        }

        let provenance_values = object["selector_provenance"]
            .as_array()
            .ok_or_else(|| format!("{label}.selector_provenance is not an array"))?;
        let selector_provenance = provenance_values
            .iter()
            .enumerate()
            .map(|(row, value)| {
                parse_selector_provenance(value, &format!("{label}.selector_provenance[{row}]"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected_provenance = match kind {
            SourceFactKind::AuthoredLevelRecords => &[][..],
            SourceFactKind::BuildingParameters => context.building_objective_provenance,
            SourceFactKind::FeatureConstructionInputs | SourceFactKind::CollisionMask => {
                context.authored_collision_provenance
            }
        };
        if selector_provenance != expected_provenance {
            return Err(format!(
                "{label} selector provenance differs from authored decode"
            ));
        }

        let artifact_values = object["source_artifacts"]
            .as_array()
            .ok_or_else(|| format!("{label}.source_artifacts is not an array"))?;
        let source_artifacts = artifact_values
            .iter()
            .enumerate()
            .map(|(row, value)| parse_artifact(value, &format!("{label}.source_artifacts[{row}]")))
            .collect::<Result<Vec<_>, _>>()?;
        let actual_paths = source_artifacts
            .iter()
            .map(|artifact| artifact.logical_path.as_str())
            .collect::<Vec<_>>();
        if actual_paths != expected.3 {
            return Err(format!("{label} owner artifact set/order differs"));
        }
        result.push(SourceFact {
            fact_id: fact_id.to_owned(),
            kind,
            version: 1,
            member: member.to_owned(),
            member_sha256,
            selector_provenance,
            source_artifacts,
        });
    }

    let expected_selectors = context
        .authored_collision_provenance
        .iter()
        .map(|row| u16::try_from(row.selector_value).expect("Level 2 selector is u8"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let feature_inputs = context
        .members
        .get("feature-construction-inputs.bin")
        .ok_or_else(|| "feature construction member is absent".to_owned())?;
    if decode_feature_construction_inputs(feature_inputs)? != expected_selectors {
        return Err("feature construction selectors differ from authored provenance".to_owned());
    }
    if let Some((collision_bank_bytes, collision_bank_sha256)) = context.collision_bank {
        let collision = context
            .members
            .get("collision-masks.bin")
            .ok_or_else(|| "collision-mask member is absent".to_owned())?;
        decode_collision_masks(collision, &expected_selectors)?;
        if collision.len() != collision_bank_bytes
            || super::spec::sha256_hex(collision) != collision_bank_sha256
        {
            return Err("collision SourceFact differs from the census-qualified bytes".to_owned());
        }
    }
    if context.has_building_parameters {
        let bytes = context
            .members
            .get("building-parameters.bin")
            .ok_or_else(|| "building parameter member is absent".to_owned())?;
        let parameters = decode_building_parameters(bytes)?;
        if super::spec::sha256_hex(bytes)
            != "1c4a7fdef5d4b811f871441f0380faaacd6e5e689e73fc4628bf8c5b51ea6f00"
            || parameters.get(&21).is_none_or(|row| {
                row.life_mana_scalar != 6 || row.flags != 8 || row.destruction_successor != 54
            })
            || parameters.get(&54).is_none_or(|row| {
                row.life_mana_scalar != 8 || row.flags != 8 || row.destruction_successor != 0
            })
        {
            return Err(
                "building parameter owner bytes or qualified objective chain changed".to_owned(),
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{building_parameter_chain, decode_building_parameters, decode_collision_masks};

    #[test]
    fn empty_collision_carrier_matches_zero_authored_rows() {
        let bytes = [
            b'M', b'C', b'M', b'1', 1, 0, 20, 0, 1, 0, 0, 0, 0, 0, 0, 0, 12, 0, 0, 0,
        ];
        decode_collision_masks(&bytes, &[]).expect("explicitly empty typed carrier is valid");
    }

    #[test]
    fn nonempty_collision_carrier_rejects_zero_authored_rows() {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            std::path::PathBuf::from,
        );
        let bytes = std::fs::read(package.join("collision-masks.bin"))
            .expect("qualified CF-02 package exists");
        let error = decode_collision_masks(&bytes, &[]).expect_err("nonempty/zero rejects");
        assert!(error.contains("empty/nonempty"));
    }

    #[test]
    fn level_two_building_parameter_chain_is_owner_bound_and_cycle_closed() {
        let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
            },
            std::path::PathBuf::from,
        );
        let bytes = std::fs::read(package.join("building-parameters.bin"))
            .expect("qualified Level 2 BLDGPRM carrier exists");
        let parameters = decode_building_parameters(&bytes).expect("owner carrier decodes");
        assert_eq!(
            &*building_parameter_chain(&parameters, 21).expect("chain closes"),
            &[21, 54]
        );

        let mut cyclic = parameters;
        cyclic
            .get_mut(&54)
            .expect("qualified successor row exists")
            .destruction_successor = 21;
        assert!(
            building_parameter_chain(&cyclic, 21)
                .expect_err("cycle must fail closed")
                .contains("cycles")
        );
    }
}
