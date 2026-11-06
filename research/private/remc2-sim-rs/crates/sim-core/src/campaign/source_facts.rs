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
    pub map_type_id: u8,
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
        "CD_Files/DATA/MSPRN0-0.DAT",
        49_353,
        "2a890388fba9f094604e9f082a1d2dcbd0995a089b39f5442b4841056ed25abc",
    ),
    (
        "CD_Files/DATA/MSPRN0-0.TAB",
        1_572,
        "12f79e74ec7c18c7c42bd3430d80831639f86650efceb2c97ccf8786c035f939",
    ),
    (
        "CD_Files/DATA/MSPRC0-0.DAT",
        49_357,
        "7e8aa789f585bf3cc64d12f44b9268b609d87e5ca6fe5f22f8be90986f42b627",
    ),
    (
        "CD_Files/DATA/MSPRC0-0.TAB",
        1_572,
        "cac2f8c5a7977cfe7303d115e83b7d6b8f44656e33ed8a6cca052c3d32b59c31",
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

pub(crate) fn normalized_source_path(value: &Value, label: &str) -> Result<String, String> {
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
        || !matches!(read_u16(bytes, 4, "feature-input version")?, 1 | 2 | 3 | 4)
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

/// MCI3 v2 retains finite constructors and all authored Houses; v3 additionally
/// retains live reset Houses and their successors. v4 also retains immutable
/// initial-Castle constructor inputs. The legacy collision census and Level 2's
/// separately closed collision profile keep their own contracts.
fn source_building_successor(selector: u16) -> Result<u16, String> {
    // Nonzero byte_3 rows from the unchanged BLDGPRM.DAT whose complete
    // 304-byte owner identity is pinned in SOURCE_ARTIFACTS.
    Ok(match selector {
        15 => 14,
        18 => 21,
        20 => 16,
        21 => 54,
        23 => 24,
        24 => 28,
        27 => 41,
        41 => 49,
        43 => 44,
        44 => 45,
        68 => 69,
        69 => 72,
        72 => 73,
        16 | 25 | 36 | 37 | 38 | 55 | 56 | 57 | 59 | 60 | 63 | 64 | 65 | 66 | 70 | 71 => selector,
        0..76 => 0,
        _ => return Err("house successor BUILD selector is outside the source table".to_owned()),
    })
}

fn close_construction_successors(selectors: &mut BTreeSet<u16>) -> Result<(), String> {
    let mut pending = selectors.iter().copied().collect::<Vec<_>>();
    while let Some(selector) = pending.pop() {
        let successor = source_building_successor(selector)?;
        if successor != 0 && selectors.insert(successor) {
            pending.push(successor);
        }
    }
    Ok(())
}

fn construction_selectors_from_authored(
    authored: &[u8],
    successor_closure: bool,
    reset_entities: Option<&[u8]>,
    initial_castle_inputs: bool,
) -> Result<Vec<u16>, String> {
    let features = crate::level1_mission::parse_authored_features(authored)?;
    let mut selectors = features
        .iter()
        .skip(1)
        .filter(|feature| feature.kind == 10 && feature.subtype == 45)
        .map(|feature| feature.parameter_1)
        .collect::<BTreeSet<_>>();
    if let Some(entities) = reset_entities {
        let record_bytes = usize::from(read_u16(entities, 6, "reset entity record bytes")?);
        let count = usize::try_from(read_u32(entities, 8, "reset entity count")?)
            .map_err(|_| "reset entity count exceeds usize".to_owned())?;
        if entities.get(..4) != Some(b"MCE1")
            || read_u16(entities, 4, "reset entity version")? != 1
            || record_bytes != 176
            || count != 1000
            || entities.len() != 12 + count * record_bytes
        {
            return Err("reset house selector arena header/extent changed".to_owned());
        }
        for row in entities[12..].chunks_exact(record_bytes).skip(1) {
            if row[2] != 0 && (row[67], row[68]) == (10, 45) {
                let selector = u16::from(row[74]);
                if selector >= 76 {
                    return Err("reset house BUILD selector is outside the source table".to_owned());
                }
                selectors.insert(selector);
            }
        }
    }
    let has_settlement = !selectors.is_empty()
        || features
            .iter()
            .skip(1)
            .any(|feature| feature.kind == 5 && feature.subtype == 12);
    if has_settlement {
        selectors.extend(
            (17_u8..76)
                .filter(|selector| {
                    crate::native_world::creatures::settlements::builder_building_spec(*selector)
                        .is_some()
                })
                .map(u16::from),
        );
    }
    if initial_castle_inputs {
        // Authored player_0x2FED9 occupies bytes11..19. sub_5C950 stamps
        // BUILD[0..count), then the first action5 can create BUILD[count].
        // BUILD0 is the owner's zero-sized no-op; BUILD1 also anchors a root.
        for &count in &authored[11..19] {
            if count >= 76 {
                return Err("initial Castle BUILD selector is outside the source table".to_owned());
            }
            if count != 0 {
                selectors.insert(1);
                selectors.extend(1..=u16::from(count));
            }
        }
    }
    if successor_closure {
        close_construction_successors(&mut selectors)?;
    }
    Ok(selectors.into_iter().collect())
}

fn validate_construction_selectors(
    bytes: &[u8],
    authored: &[u8],
    legacy_selectors: &[u16],
    successor_closure: bool,
    reset_entities: Option<&[u8]>,
) -> Result<(), String> {
    let actual = decode_feature_construction_inputs(bytes)?;
    let version = read_u16(bytes, 4, "feature-input version")?;
    if successor_closure && !matches!(version, 2 | 3 | 4) {
        return Err("source-closed construction profile requires MCI3 v2, v3 or v4".to_owned());
    }
    if matches!(version, 3 | 4) && (!successor_closure || reset_entities.is_none()) {
        return Err(format!(
            "MCI3 v{version} requires reset houses and their source successor closure"
        ));
    }
    let expected = if version == 1 {
        legacy_selectors.to_vec()
    } else {
        construction_selectors_from_authored(
            authored,
            successor_closure,
            if matches!(version, 3 | 4) {
                reset_entities
            } else {
                None
            },
            version == 4,
        )?
    };
    if actual != expected {
        return Err(
            "feature construction selectors differ from versioned source closure".to_owned(),
        );
    }
    if successor_closure {
        for (selector, parameter) in crate::house::decode_campaign_house_parameters(bytes)? {
            if u16::from(parameter.destruction_successor)
                != source_building_successor(u16::from(selector))?
            {
                return Err("feature construction successor differs from pinned BLDGPRM".to_owned());
            }
        }
    }
    Ok(())
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

fn validate_construction_owner_parameters(inputs: &[u8], owner: &[u8]) -> Result<(), String> {
    let retained = crate::house::decode_campaign_house_parameters(inputs)?;
    let parameters = decode_building_parameters(owner)?;
    for (selector, parameter) in retained {
        if parameters.get(&selector) != Some(&parameter) {
            return Err(format!("BUILD {selector} parameter differs from owner BLDGPRM"));
        }
    }
    Ok(())
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
            || !(version == 1
                || (kind == SourceFactKind::FeatureConstructionInputs
                    && matches!(version, 2 | 3 | 4))
                || (kind == SourceFactKind::CollisionMask && matches!(version, 2 | 3 | 4)))
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
        // v1 retains the legacy Day provenance contract. v2 names the bank
        // Level.cpp actually loads for this map, while retaining MCM1 pixels.
        let expected_paths =
            if kind == SourceFactKind::FeatureConstructionInputs && matches!(version, 2 | 3 | 4) {
                &[
                    "CD_Files/DATA/BLDGPRM.DAT",
                    "CD_Files/DATA/BUILD0-0.DAT",
                    "CD_Files/DATA/BUILD0-0.TAB",
                    "CD_Files/DATA/SPELLS.DAT",
                    "CD_Files/DATA/TABLESN.DAT",
                ][..]
            } else if kind == SourceFactKind::CollisionMask && matches!(version, 2 | 3 | 4) {
                match context.map_type_id {
                    0 => &["CD_Files/DATA/MSPRD0-0.DAT", "CD_Files/DATA/MSPRD0-0.TAB"][..],
                    1 => &["CD_Files/DATA/MSPRN0-0.DAT", "CD_Files/DATA/MSPRN0-0.TAB"][..],
                    2 => &["CD_Files/DATA/MSPRC0-0.DAT", "CD_Files/DATA/MSPRC0-0.TAB"][..],
                    _ => return Err(format!("{label} has an unknown map bank")),
                }
            } else {
                expected.3
            };
        if actual_paths != expected_paths {
            return Err(format!("{label} owner artifact set/order differs"));
        }
        result.push(SourceFact {
            fact_id: fact_id.to_owned(),
            kind,
            version: version as u16,
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
    let authored_inputs = context
        .members
        .get("authored-level.bin")
        .ok_or_else(|| "authored construction input member is absent".to_owned())?;
    let construction_fact_version = result
        .iter()
        .find(|fact| fact.kind == SourceFactKind::FeatureConstructionInputs)
        .map_or(0, |fact| fact.version);
    let construction_version = read_u16(feature_inputs, 4, "feature-input version")?;
    if matches!(construction_version, 3 | 4) && !context.has_building_parameters {
        return Err("reset-house construction requires the owner BLDGPRM bank".to_owned());
    }
    if (matches!(construction_fact_version, 3 | 4) || matches!(construction_version, 3 | 4))
        && construction_fact_version != construction_version
    {
        return Err("reset-house SourceFact and construction member versions differ".to_owned());
    }
    let successor_closure = matches!(construction_fact_version, 2 | 3 | 4);
    let collision_version = result
        .iter()
        .find(|fact| fact.kind == SourceFactKind::CollisionMask)
        .map_or(0, |fact| fact.version);
    validate_construction_selectors(
        feature_inputs,
        authored_inputs,
        &expected_selectors,
        successor_closure,
        context.members.get("entities.bin").map(Vec::as_slice),
    )?;
    if let Some((collision_bank_bytes, collision_bank_sha256)) = context.collision_bank {
        let collision = context
            .members
            .get("collision-masks.bin")
            .ok_or_else(|| "collision-mask member is absent".to_owned())?;
        let collision_selectors = if collision_version == 3 {
            if !successor_closure
                || context.map_type_id != 0
                || collision.len() != 12_836
                || super::spec::sha256_hex(collision)
                    != "afb42f4c6d64cdd394c71eab88d3dea5f344853a1576e6314736d270de3010dd"
            {
                return Err(
                    "closed collision profile differs from qualified Level 2 source closure"
                        .to_owned(),
                );
            }
            decode_feature_construction_inputs(feature_inputs)?
        } else if collision_version == 4 {
            if construction_fact_version != 4
                || construction_version != 4
                || !context.has_building_parameters
            {
                return Err(
                    "generic model45 collision closure requires owner BLDGPRM and MCI3 v4"
                        .to_owned(),
                );
            }
            // Castle-only BUILD inputs are terrain constructors, not model45
            // collision selectors. Derive this set from source records rather
            // than accepting the complete (larger) MCI3 v4 bank.
            construction_selectors_from_authored(
                authored_inputs,
                true,
                Some(context.members.get("entities.bin").ok_or_else(|| {
                    "model45 collision closure requires reset entities".to_owned()
                })?),
                false,
            )?
        } else {
            expected_selectors.clone()
        };
        decode_collision_masks(collision, &collision_selectors)?;
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
        if matches!(construction_version, 3 | 4) {
            validate_construction_owner_parameters(feature_inputs, bytes)?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_successor_closure_is_transitive_and_stops_cycles() {
        let mut selectors = [18, 23, 37, 43].into_iter().collect();
        super::close_construction_successors(&mut selectors).unwrap();
        assert_eq!(
            selectors.into_iter().collect::<Vec<_>>(),
            [18, 21, 23, 24, 28, 37, 43, 44, 45, 54]
        );
        let mut invalid = [76].into_iter().collect();
        assert!(super::close_construction_successors(&mut invalid).is_err());
    }
    use super::{building_parameter_chain, decode_building_parameters, decode_collision_masks};

    #[test]
    fn full_construction_bank_is_closed_over_real_house_and_builder_inputs() {
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let full = std::env::var_os("MC_SETTLEMENT_FULL_PACKAGE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| generated.join("mc-1pb-16-level2-full-build-v2"));
        let legacy = generated.join("mc-cf02-campaign-package-v2-b/qualified-level2");
        let authored =
            std::fs::read(full.join("authored-level.bin")).expect("fresh owner-authored Level2");
        let bytes = std::fs::read(full.join("feature-construction-inputs.bin"))
            .expect("fresh full source BUILD bank");
        let old = std::fs::read(legacy.join("feature-construction-inputs.bin"))
            .expect("legacy qualified BUILD bank");
        let legacy_selectors = super::decode_feature_construction_inputs(&old).unwrap();
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 2);
        let selectors = super::decode_feature_construction_inputs(&bytes).unwrap();
        assert_eq!(
            selectors,
            [
                17, 19, 21, 22, 26, 29, 30, 31, 32, 33, 34, 35, 39, 40, 42, 44, 47, 50, 55, 59, 60,
                62, 64, 70, 74
            ],
            "actual all-group House selectors union the finite Builder closure"
        );
        assert!(selectors.contains(&19) && selectors.contains(&74));
        super::validate_construction_selectors(&bytes, &authored, &legacy_selectors, false, None)
            .unwrap();
        super::validate_construction_selectors(&old, &authored, &legacy_selectors, false, None)
            .unwrap();

        let mut missing = bytes.clone();
        let extent = u32::from_le_bytes(missing[24..28].try_into().unwrap()) as usize;
        missing.drain(16..28 + extent);
        let count = (selectors.len() as u16 - 1).to_le_bytes();
        missing[8..10].copy_from_slice(&count);
        assert!(
            super::validate_construction_selectors(
                &missing,
                &authored,
                &legacy_selectors,
                false,
                None
            )
            .is_err()
        );

        let mut extra = bytes.clone();
        extra[16..18].copy_from_slice(&0_u16.to_le_bytes());
        assert!(
            super::validate_construction_selectors(
                &extra,
                &authored,
                &legacy_selectors,
                false,
                None
            )
            .is_err()
        );

        let mut mislabeled = bytes;
        mislabeled[4..6].copy_from_slice(&1_u16.to_le_bytes());
        assert!(
            super::validate_construction_selectors(
                &mislabeled,
                &authored,
                &legacy_selectors,
                false,
                None
            )
            .is_err(),
            "v1 retains its original exact bank contract"
        );
    }

    #[test]
    fn reset_house_bank_v3_uses_real_live_selectors_and_rejects_a_missing_successor() {
        let package = std::env::var_os("MC_LEVEL23_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-ordinary-package-_pgn4jfo/level23")
            },
            std::path::PathBuf::from,
        );
        let authored = std::fs::read(package.join("authored-level.bin")).unwrap();
        let entities = std::fs::read(package.join("entities.bin")).unwrap();
        let bytes = std::fs::read(package.join("feature-construction-inputs.bin")).unwrap();
        let owner_parameters = std::fs::read(package.join("building-parameters.bin")).unwrap();
        assert_eq!(super::read_u16(&bytes, 4, "version").unwrap(), 3);
        super::validate_construction_selectors(&bytes, &authored, &[], true, Some(&entities))
            .unwrap();
        super::validate_construction_owner_parameters(&bytes, &owner_parameters).unwrap();
        assert!(
            super::validate_construction_selectors(&bytes, &authored, &[], true, None).is_err()
        );
        let selectors = super::decode_feature_construction_inputs(&bytes).unwrap();
        for required in [27, 41, 49, 16] {
            assert!(
                selectors.contains(&required),
                "real reset/successor row {required}"
            );
        }

        let mut cursor = 16;
        let mut removed = None;
        let mut altered_parameter = bytes.clone();
        let mut altered = false;
        for &selector in &selectors {
            let extent = super::read_u32(&bytes, cursor + 8, "extent").unwrap() as usize;
            if selector == 27 {
                altered_parameter[cursor + 2] ^= 1;
                altered = true;
            }
            if selector == 41 {
                removed = Some(cursor..cursor + 12 + extent);
                break;
            }
            cursor += 12 + extent;
        }
        assert!(altered);
        assert!(super::validate_construction_owner_parameters(&altered_parameter, &owner_parameters).is_err());
        let mut missing = bytes;
        missing.drain(removed.expect("real row27's source successor41 is retained"));
        missing[8..10].copy_from_slice(&(selectors.len() as u16 - 1).to_le_bytes());
        assert!(
            super::validate_construction_selectors(&missing, &authored, &[], true, Some(&entities))
                .is_err()
        );
    }

    #[test]
    fn initial_castle_bank_v4_requires_authored_tiers_and_preserves_v3_closure() {
        let generated =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_INITIAL_CASTLE_PACKAGE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                generated.join("ribbon-20261104-integration/group-zero-owner-package-v4/level9")
            });
        let legacy = generated.join("tremor-20261104-integration/current-owner-package/level9");
        let authored = std::fs::read(package.join("authored-level.bin")).unwrap();
        let entities = std::fs::read(package.join("entities.bin")).unwrap();
        let bytes = std::fs::read(package.join("feature-construction-inputs.bin")).unwrap();
        let owner_parameters = std::fs::read(package.join("building-parameters.bin")).unwrap();
        let old = std::fs::read(legacy.join("feature-construction-inputs.bin")).unwrap();
        assert_eq!(
            std::fs::read(legacy.join("authored-level.bin")).unwrap(),
            authored
        );
        assert_eq!(
            std::fs::read(legacy.join("entities.bin")).unwrap(),
            entities
        );
        assert_eq!(&authored[11..19], &[0, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(super::read_u16(&bytes, 4, "version").unwrap(), 4);
        assert_eq!(super::read_u16(&old, 4, "version").unwrap(), 3);
        super::validate_construction_selectors(&bytes, &authored, &[], true, Some(&entities))
            .unwrap();
        super::validate_construction_owner_parameters(&bytes, &owner_parameters).unwrap();
        super::validate_construction_selectors(&old, &authored, &[], true, Some(&entities))
            .expect("v3 retains its exact pre-Castle-input contract");
        let selectors = super::decode_feature_construction_inputs(&bytes).unwrap();
        let old_selectors = super::decode_feature_construction_inputs(&old).unwrap();
        let construction = crate::house::decode_house_construction_inputs(&bytes).unwrap();
        let parameters = crate::house::decode_campaign_house_parameters(&bytes).unwrap();
        assert!(
            !selectors.contains(&0),
            "source BUILD0 remains an empty no-op"
        );
        for required in [1, 2, 3] {
            assert!(selectors.contains(&required));
            assert!(!old_selectors.contains(&required));
            assert!(construction.contains_key(&(required as u8)));
            assert!(parameters.contains_key(&(required as u8)));
            let mut cursor = 16;
            let mut removed = None;
            for &selector in &selectors {
                let extent = super::read_u32(&bytes, cursor + 8, "extent").unwrap() as usize;
                if selector == required {
                    removed = Some(cursor..cursor + 12 + extent);
                    break;
                }
                cursor += 12 + extent;
            }
            let mut missing = bytes.clone();
            missing.drain(removed.expect("required real source BUILD row"));
            missing[8..10].copy_from_slice(&(selectors.len() as u16 - 1).to_le_bytes());
            assert!(
                super::validate_construction_selectors(
                    &missing,
                    &authored,
                    &[],
                    true,
                    Some(&entities)
                )
                .is_err(),
                "v4 requires BUILD{required}, including the first callback's next tier"
            );
        }
        let mut mislabeled = bytes.clone();
        mislabeled[4..6].copy_from_slice(&3_u16.to_le_bytes());
        assert!(
            super::validate_construction_selectors(
                &mislabeled,
                &authored,
                &[],
                true,
                Some(&entities)
            )
            .is_err(),
            "v3 cannot admit additional Castle inputs"
        );
        let mut missing_castle_bank = old;
        missing_castle_bank[4..6].copy_from_slice(&4_u16.to_le_bytes());
        assert!(
            super::validate_construction_selectors(
                &missing_castle_bank,
                &authored,
                &[],
                true,
                Some(&entities)
            )
            .is_err()
        );
        assert!(
            super::validate_construction_selectors(&bytes, &authored, &[], true, None).is_err()
        );
        assert!(
            super::validate_construction_selectors(&bytes, &authored, &[], false, Some(&entities))
                .is_err()
        );
    }

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
