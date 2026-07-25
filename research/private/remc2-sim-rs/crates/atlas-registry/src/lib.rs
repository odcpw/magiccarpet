//! Complete private mapping mirror for the remc2 semantic atlas.
//!
//! The registry intentionally retains rows which are unknown, disabled, or
//! structural. A small implementation may therefore never masquerade as a
//! complete simulator.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImplementationStatus {
    MappedOnly,
    StructuralOnly,
    Partial,
    Implemented,
    Verified,
}

impl ImplementationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MappedOnly => "MappedOnly",
            Self::StructuralOnly => "StructuralOnly",
            Self::Partial => "Partial",
            Self::Implemented => "Implemented",
            Self::Verified => "Verified",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappingRow {
    pub table: &'static str,
    pub key: String,
    pub status: ImplementationStatus,
    pub test_id: Option<&'static str>,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct AtlasTable {
    pub id: &'static str,
    pub path: &'static str,
    pub expected_sha256: &'static str,
    pub actual_sha256: String,
    pub header: Vec<String>,
    pub rows: Vec<MappingRow>,
}

impl AtlasTable {
    #[must_use]
    pub fn field<'a>(&'a self, row: &'a MappingRow, name: &str) -> Option<&'a str> {
        self.header
            .iter()
            .position(|value| value == name)
            .and_then(|index| row.fields.get(index))
            .map(String::as_str)
    }
}

#[derive(Clone, Debug)]
pub struct AtlasRegistry {
    tables: BTreeMap<&'static str, AtlasTable>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageSummary {
    pub tables: usize,
    pub mappings: usize,
    pub by_status: BTreeMap<&'static str, usize>,
}

#[derive(Clone, Copy)]
struct SourceSpec {
    id: &'static str,
    path: &'static str,
    contents: &'static str,
    expected_rows: usize,
    expected_sha256: &'static str,
    key_fields: &'static [&'static str],
}

macro_rules! source {
    ($id:literal, $file:literal, $rows:literal, $sha:literal, [$($key:literal),+ $(,)?]) => {
        SourceSpec {
            id: $id,
            path: concat!("research/private/remc2-atlas/semantics/generated/", $file),
            contents: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../remc2-atlas/semantics/generated/",
                $file
            )),
            expected_rows: $rows,
            expected_sha256: $sha,
            key_fields: &[$($key),+],
        }
    };
}

const SOURCES: &[SourceSpec] = &[
    source!(
        "functions",
        "function-semantic-map.csv",
        3_060,
        "8eec56b7ace2eefd856aa9fb799e7b480df422b13905d971433e1eaef5c6201a",
        ["function_usr"]
    ),
    source!(
        "state",
        "state-semantic-map.csv",
        3_246,
        "8411490b8ac03c823a14073352eac8c359014d07e076a06ed8785affad98e257",
        ["target_usr"]
    ),
    source!(
        "claims",
        "claims.csv",
        337,
        "718927067d5d1f6454037dddf3c20a6bb915a09e8d9be56055d7c66afded4464",
        ["claim_id"]
    ),
    source!(
        "dispatch",
        "runtime-dispatch-rows.csv",
        845,
        "ac3ad1a9ca3d5708887145a299f65700dab191c52c9460122236cb270e31ad22",
        ["runtime_class", "table_kind", "physical_slot"]
    ),
    source!(
        "actions",
        "runtime-action-annotations.csv",
        546,
        "eccd5766786a6d767d9c2f600523b5acd49e5f9d9e6c3c3433affbe1b8d8374d",
        ["runtime_class", "action_physical_slot"]
    ),
    source!(
        "constructors",
        "runtime-constructor-registry.csv",
        284,
        "0ffccac4e74a289975372bcd9a0fbfbcee637c16f5479bb0afbdd50e53e23d06",
        ["runtime_class", "constructor_physical_slot"]
    ),
    source!(
        "classes",
        "runtime-class-families.csv",
        16,
        "f80b1f8790021b47c0d6817549ac5d88312b0b1e6afd0b8b7752afefe09049ee",
        ["runtime_class"]
    ),
    source!(
        "creature_models",
        "creature-models.csv",
        29,
        "7295b7909da396608ba5d1c557fc5baaa4e83ad9ab0623d5c9300ecae3d06476",
        ["runtime_class", "runtime_model"]
    ),
    source!(
        "creature_action_exceptions",
        "creature-action-exceptions.csv",
        3,
        "0b5b8d965e2406e39bf98a3dde6e9252765a5047a38fb8f2aa8db8acde5890da",
        ["action_index"]
    ),
    source!(
        "model_annotations",
        "runtime-model-annotations.csv",
        250,
        "12af2dd956a0e35af388a75544b80e6a89cba6da78d70005ae8440db221c3aad",
        ["runtime_class", "constructor_physical_slot"]
    ),
    source!(
        "tick_pipeline",
        "simulation-tick-pipeline.csv",
        14,
        "f2ae8ee87619364a6e0d61afb85b37b0d64a69412b5283ef5c1a1a873ec929ce",
        ["tick_order", "function_name"]
    ),
    source!(
        "system_surfaces",
        "system-surface-registry.csv",
        47,
        "c04f3fac910da5fdd6005c181ea55b54e3587c1268ffa310acaa2352885fd17f",
        ["execution_order_hint", "function_name"]
    ),
    source!(
        "claim_function_links",
        "claim-function-links.csv",
        1_845,
        "c45338a022eab683e87a1b36550b29c1bc816deaa9cb21befc6893f774dafe6a",
        [
            "claim_id",
            "function_usr",
            "anchor_start_line",
            "anchor_end_line",
            "link_method"
        ]
    ),
    source!(
        "core_behavior_claims",
        "core-behavior-claims.csv",
        227,
        "a3f616b10d1dc41286b320e4a7343151035a5379af37a68182ddd0093b7ca9bf",
        ["claim_id"]
    ),
    source!(
        "owned_sprite_atlas",
        "owned-sprite-atlas.csv",
        1_572,
        "a988273548e012fa90bffa5edcc51d509fa07acf640e31139a0219b0e7f04fb3",
        ["namespace", "raw_key"]
    ),
    source!(
        "semantic_gap_queue",
        "semantic-gap-queue.csv",
        920,
        "690743235f111034effe11fa290a561ef58bcf5bc77427dcae85681f06c091ca",
        ["function_usr"]
    ),
    source!(
        "spell_families",
        "spell-families.csv",
        26,
        "e77a7d4622cb84b4bb5a5cfa2e5919c3816545a6379fd2db9cf11502dbedc2af",
        ["family_id"]
    ),
    source!(
        "spell_icon_assets",
        "spell-icon-assets.csv",
        390,
        "faf24c19ec74152e4bffc7e25796ae209f82c79eeefb67d41071b80978f2899b",
        ["namespace", "raw_key"]
    ),
    source!(
        "spell_seed_differences",
        "spell-seed-vs-owned.csv",
        269,
        "053dec2cd933cfcd58554472bafc4cab99e3cbadeb9fe32feceaa0e90c54e52e",
        ["family_id", "tier", "field"]
    ),
    source!(
        "spell_tiers",
        "spell-tiers.csv",
        78,
        "844b59720761244d74905faa9cbfa07ad065666507ddbcfdec524a4468816240",
        ["family_id", "tier"]
    ),
    source!(
        "state_gap_queue",
        "state-gap-queue.csv",
        456,
        "b93c814c5821be91923d8c3c932de785f94573ddcc02cdbe998a2d1e76e26fd9",
        ["target_usr"]
    ),
    source!(
        "subsystem_coverage",
        "subsystem-semantic-coverage.csv",
        23,
        "a47dbf71a6b8038b4943b19e2d45644d33f2f2cb8d91144349b48ee0ab7624ba",
        ["subsystem"]
    ),
    source!(
        "unresolved",
        "unresolved.csv",
        11,
        "26db04cc08805d6ec77f95105b3b22e08fbd4ad31b3faabcdb6170e7325b232c",
        ["issue_id"]
    ),
    source!(
        "visual_evidence",
        "visual-evidence.csv",
        2,
        "3e399b9e34ae434dc08ee10a5acccb5dbfd976762a32e15b962dbb3d59b17616",
        ["claim_id", "raw_key"]
    ),
];

impl AtlasRegistry {
    /// Parse every embedded source and apply the separate implementation
    /// overlay. The CSV provenance fields are never replaced by the overlay.
    ///
    /// # Errors
    ///
    /// Returns an error on source-hash or denominator drift, malformed CSV,
    /// missing or duplicate physical keys, or an incomplete cross-table join.
    pub fn load_embedded() -> Result<Self, String> {
        let mut tables = BTreeMap::new();
        for source in SOURCES {
            let records = parse_csv(source.contents)?;
            let (header, data) = records
                .split_first()
                .ok_or_else(|| format!("{} is empty", source.path))?;
            if data.len() != source.expected_rows {
                return Err(format!(
                    "{} row drift: expected {}, found {}",
                    source.path,
                    source.expected_rows,
                    data.len()
                ));
            }
            let key_indices = source
                .key_fields
                .iter()
                .map(|name| {
                    header
                        .iter()
                        .position(|field| field == name)
                        .ok_or_else(|| format!("{} is missing key column {name}", source.path))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut keys = BTreeSet::new();
            let mut rows = Vec::with_capacity(data.len());
            for (row_index, fields) in data.iter().enumerate() {
                if fields.len() != header.len() {
                    return Err(format!(
                        "{} has {} fields but header has {}",
                        source.path,
                        fields.len(),
                        header.len()
                    ));
                }
                let mut key = key_indices
                    .iter()
                    .map(|index| fields[*index].as_str())
                    .collect::<Vec<_>>()
                    .join("|");
                // This file is an edge multiset and intentionally retains
                // repeated evidence edges. The physical CSV row is therefore
                // part of its stable private identity.
                if source.id == "claim_function_links" {
                    write!(key, "|row={}", row_index + 1).expect("write to String cannot fail");
                }
                if !keys.insert(key.clone()) {
                    return Err(format!("{} has duplicate physical key {key}", source.path));
                }
                let (status, test_id) = disposition(source.id, header, fields, &key);
                rows.push(MappingRow {
                    table: source.id,
                    key,
                    status,
                    test_id,
                    fields: fields.clone(),
                });
            }
            let actual_sha256 = sha256_hex(source.contents.as_bytes());
            if actual_sha256 != source.expected_sha256 {
                return Err(format!(
                    "{} content drift: expected {}, found {}",
                    source.path, source.expected_sha256, actual_sha256
                ));
            }
            tables.insert(
                source.id,
                AtlasTable {
                    id: source.id,
                    path: source.path,
                    expected_sha256: source.expected_sha256,
                    actual_sha256,
                    header: header.clone(),
                    rows,
                },
            );
        }
        let registry = Self { tables };
        registry.validate_crosswalks()?;
        Ok(registry)
    }

    #[must_use]
    pub fn table(&self, id: &str) -> Option<&AtlasTable> {
        self.tables.get(id)
    }

    pub fn tables(&self) -> impl Iterator<Item = &AtlasTable> {
        self.tables.values()
    }

    #[must_use]
    pub fn summary(&self) -> CoverageSummary {
        let mut by_status = BTreeMap::new();
        let mappings = self
            .tables()
            .flat_map(|table| table.rows.iter())
            .inspect(|row| {
                *by_status.entry(row.status.as_str()).or_insert(0) += 1;
            })
            .count();
        CoverageSummary {
            tables: self.tables.len(),
            mappings,
            by_status,
        }
    }

    // This is intentionally one flat audit ledger: every join is visible in
    // the same bounded validation path and fails before a registry is exposed.
    #[allow(clippy::too_many_lines)]
    fn validate_crosswalks(&self) -> Result<(), String> {
        let actions = self.table("actions").expect("required table");
        let individual = actions
            .rows
            .iter()
            .filter(|row| actions.field(row, "annotation_level") == Some("INDIVIDUAL_BEHAVIOR"))
            .count();
        let structural = actions
            .rows
            .iter()
            .filter(|row| actions.field(row, "annotation_level") == Some("STRUCTURAL_STATUS"))
            .count();
        if (individual, structural) != (429, 117) {
            return Err(format!(
                "action taxonomy drift: expected 429/117, found {individual}/{structural}"
            ));
        }

        let dispatch = self.table("dispatch").expect("required table");
        let action_keys: BTreeSet<_> = actions.rows.iter().map(|row| row.key.as_str()).collect();
        let mut joined = 0;
        for row in &dispatch.rows {
            if dispatch.field(row, "table_kind") == Some("action")
                && dispatch.field(row, "is_null_sentinel") != Some("True")
            {
                let key = format!(
                    "{}|{}",
                    dispatch.field(row, "runtime_class").unwrap_or_default(),
                    dispatch.field(row, "physical_slot").unwrap_or_default()
                );
                if !action_keys.contains(key.as_str()) {
                    return Err(format!(
                        "non-sentinel dispatch action lacks annotation: {key}"
                    ));
                }
                joined += 1;
            }
        }
        if joined != 546 {
            return Err(format!(
                "non-sentinel action join drift: expected 546, found {joined}"
            ));
        }

        let constructors = self.table("constructors").expect("required table");
        let constructor_keys: BTreeSet<_> = constructors
            .rows
            .iter()
            .map(|row| row.key.as_str())
            .collect();
        for row in &self
            .table("model_annotations")
            .expect("required table")
            .rows
        {
            if !constructor_keys.contains(row.key.as_str()) {
                return Err(format!(
                    "model annotation lacks constructor physical key: {}",
                    row.key
                ));
            }
        }

        let functions = self.table("functions").expect("required table");
        let function_keys: BTreeSet<_> =
            functions.rows.iter().map(|row| row.key.as_str()).collect();
        let claims = self.table("claims").expect("required table");
        let claim_keys: BTreeSet<_> = claims.rows.iter().map(|row| row.key.as_str()).collect();
        let links = self.table("claim_function_links").expect("required table");
        for row in &links.rows {
            let claim = links.field(row, "claim_id").unwrap_or_default();
            let function = links.field(row, "function_usr").unwrap_or_default();
            if !claim_keys.contains(claim) {
                return Err(format!("claim/function edge has unknown claim: {claim}"));
            }
            if !function_keys.contains(function) {
                return Err(format!(
                    "claim/function edge has unknown function: {function}"
                ));
            }
        }
        let semantic_gaps = self.table("semantic_gap_queue").expect("required table");
        for row in &semantic_gaps.rows {
            let function = semantic_gaps.field(row, "function_usr").unwrap_or_default();
            if !function_keys.contains(function) {
                return Err(format!("semantic gap has unknown function: {function}"));
            }
        }
        for row in &self
            .table("core_behavior_claims")
            .expect("required table")
            .rows
        {
            if !claim_keys.contains(row.key.as_str()) {
                return Err(format!("core behavior row has unknown claim: {}", row.key));
            }
        }

        let state = self.table("state").expect("required table");
        let state_keys: BTreeSet<_> = state.rows.iter().map(|row| row.key.as_str()).collect();
        for row in &self.table("state_gap_queue").expect("required table").rows {
            if !state_keys.contains(row.key.as_str()) {
                return Err(format!("state gap has unknown target: {}", row.key));
            }
        }

        let classes = self.table("classes").expect("required table");
        let class_keys: BTreeSet<_> = classes.rows.iter().map(|row| row.key.as_str()).collect();
        let creature_models = self.table("creature_models").expect("required table");
        for row in &creature_models.rows {
            let class = creature_models
                .field(row, "runtime_class")
                .unwrap_or_default();
            if !class_keys.contains(class) {
                return Err(format!("creature model has unknown class: {class}"));
            }
        }

        let spell_families = self.table("spell_families").expect("required table");
        let family_keys: BTreeSet<_> = spell_families
            .rows
            .iter()
            .map(|row| row.key.as_str())
            .collect();
        for table_id in ["spell_icon_assets", "spell_seed_differences", "spell_tiers"] {
            let table = self.table(table_id).expect("required table");
            for row in &table.rows {
                let family = table.field(row, "family_id").unwrap_or_default();
                if !family_keys.contains(family) {
                    return Err(format!("{table_id} row has unknown family: {family}"));
                }
            }
        }

        let visual = self.table("visual_evidence").expect("required table");
        for row in &visual.rows {
            let claim = visual.field(row, "claim_id").unwrap_or_default();
            if !claim_keys.contains(claim) {
                return Err(format!("visual evidence has unknown claim: {claim}"));
            }
        }
        Ok(())
    }
}

fn field<'a>(header: &[String], row: &'a [String], name: &str) -> Option<&'a str> {
    header
        .iter()
        .position(|field| field == name)
        .and_then(|index| row.get(index))
        .map(String::as_str)
}

fn disposition(
    table: &str,
    header: &[String],
    row: &[String],
    key: &str,
) -> (ImplementationStatus, Option<&'static str>) {
    if let Some(test_id) = partial_disposition(table, header, row, key) {
        return (ImplementationStatus::Partial, Some(test_id));
    }
    let source_name = field(header, row, "name");
    if (table == "dispatch" && key == "3|action|0")
        || (table == "actions" && key == "3|0")
        || (table == "functions" && source_name == Some("AddPlayer03_00_5E010"))
    {
        return (
            ImplementationStatus::Partial,
            Some("flight.class3_action0.partial"),
        );
    }
    if table == "tick_pipeline"
        && matches!(
            field(header, row, "function_name"),
            Some("PlayerEvents_51BB0" | "UpdateEntities_57730")
        )
    {
        return (
            ImplementationStatus::Partial,
            Some("scheduler.flight_spine.partial"),
        );
    }

    let structural = match table {
        "actions" => field(header, row, "annotation_level") == Some("STRUCTURAL_STATUS"),
        "dispatch" => {
            field(header, row, "is_null_sentinel") == Some("True")
                || field(header, row, "semantic_scope") == Some("STRUCTURAL_STATUS")
                || matches!(
                    field(header, row, "resolution_status"),
                    Some(
                        "NULL_SENTINEL"
                            | "DISABLED_NULL_ADDRESS"
                            | "DISABLED_WITH_STORED_ADDRESS"
                            | "ENABLED_NULL_ADDRESS"
                            | "ENABLED_DISPATCH_CASE_WITHOUT_CALL"
                    )
                )
        }
        "constructors" => {
            field(header, row, "is_null_sentinel") == Some("True")
                || matches!(
                    field(header, row, "resolution_status"),
                    Some(
                        "NULL_SENTINEL"
                            | "DISABLED_NULL_ADDRESS"
                            | "DISABLED_WITH_STORED_ADDRESS"
                            | "ENABLED_NULL_ADDRESS"
                            | "ENABLED_DISPATCH_CASE_WITHOUT_CALL"
                    )
                )
        }
        "creature_action_exceptions" => {
            field(header, row, "status").is_some_and(|value| value.contains("disabled"))
        }
        _ => false,
    };
    (
        if structural {
            ImplementationStatus::StructuralOnly
        } else {
            ImplementationStatus::MappedOnly
        },
        None,
    )
}

fn partial_disposition(
    table: &str,
    header: &[String],
    row: &[String],
    key: &str,
) -> Option<&'static str> {
    let source_name = field(header, row, "name");
    if table == "functions" {
        let test_id = match source_name {
            Some("moveTest_5D0A0") => Some("movement.move_test.partial"),
            Some("sub_5D530") => Some("flight.terrain_collision.partial"),
            Some("sub_11E20") => Some("movement.cave_overlap.partial"),
            _ => None,
        };
        if let Some(test_id) = test_id {
            return Some(test_id);
        }
    }
    if matches!(table, "claims" | "core_behavior_claims")
        && matches!(
            key,
            "flight-terrain-relative-pitch"
                | "flight-ground-clamp"
                | "flight-cave-clamp"
                | "flight-speed-modifier"
                | "flight-immobilize"
                | "terrain-ground-height"
                | "terrain-cave-ceiling"
                | "terrain-water"
                | "terrain-angle-water-grid"
        )
    {
        return Some("flight.terrain_provider.partial");
    }
    if table == "system_surfaces" && key == "120|sub_5D530" {
        return Some("flight.terrain_collision.partial");
    }
    if table == "state"
        && matches!(
            key,
            "c:@S@_str_0x6E8E@FI@position_0x4C_76"
                | "c:@S@_str_0x6E8E@FI@yaw_0x1C_28"
                | "c:@S@Type_str_164@FI@speed_0xc_12"
                | "c:@S@Type_str_164@FI@pitch_0x157_343"
                | "c:@S@Type_str_164@FI@pitch_0x24_36"
                | "c:@S@_str_0x6E8E@FI@pitch_0x1E_30"
                | "c:@S@Type_str_164@FI@waterCounter_0x262_610"
                | "c:@S@Type_str_164@FI@moveSpeed_0x14C_332"
                | "c:@S@Type_str_164@FI@moveSpeedCounter_0x14D_333"
                | "c:@S@Type_str_164@FI@mobilizeCounter_0x14E_334"
                | "c:@S@Type_str_164@FI@mobilizeCounter2_0x150_336"
                | "c:@predictedAxis_EB398ar"
                | "c:@isCaveLevel_D41B6"
                | "c:@SA@type_str_160@FI@word_160_0xa_10"
                | "c:@SA@type_str_160@FI@word_160_0xc_12"
                | "c:@SA@type_str_160@FI@word_160_0xe_14"
        )
    {
        return Some("flight.state_fields.partial");
    }
    None
}

fn sha256_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(output, "{byte:02x}").expect("write to String cannot fail");
    }
    output
}

/// RFC-4180-shaped parser for the generated atlas files. It supports quoted
/// commas, doubled quotes, CRLF, and quoted newlines.
fn parse_csv(input: &str) -> Result<Vec<Vec<String>>, String> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;

    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => quoted = true,
            ',' => record.push(std::mem::take(&mut field)),
            '\n' => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err("unterminated quoted CSV field".to_owned());
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parser_preserves_quoted_cells() {
        let rows = parse_csv("a,b\n1,\"two, and \"\"quoted\"\"\"\n").unwrap();
        assert_eq!(rows[1], ["1", "two, and \"quoted\""]);
    }

    #[test]
    fn complete_denominators_and_hashes_are_locked() {
        let registry = AtlasRegistry::load_embedded().unwrap();
        let expected = [
            ("functions", 3_060),
            ("state", 3_246),
            ("claims", 337),
            ("dispatch", 845),
            ("actions", 546),
            ("constructors", 284),
            ("classes", 16),
            ("creature_models", 29),
            ("creature_action_exceptions", 3),
            ("model_annotations", 250),
            ("tick_pipeline", 14),
            ("system_surfaces", 47),
            ("claim_function_links", 1_845),
            ("core_behavior_claims", 227),
            ("owned_sprite_atlas", 1_572),
            ("semantic_gap_queue", 920),
            ("spell_families", 26),
            ("spell_icon_assets", 390),
            ("spell_seed_differences", 269),
            ("spell_tiers", 78),
            ("state_gap_queue", 456),
            ("subsystem_coverage", 23),
            ("unresolved", 11),
            ("visual_evidence", 2),
        ];
        for (id, count) in expected {
            let table = registry.table(id).unwrap();
            assert_eq!(table.rows.len(), count, "{id}");
            assert_eq!(table.actual_sha256, table.expected_sha256, "{id}");
        }
    }

    #[test]
    fn physical_slot_is_not_selector_or_semantic_identity() {
        let registry = AtlasRegistry::load_embedded().unwrap();
        let dispatch = registry.table("dispatch").unwrap();
        let mismatched = dispatch
            .rows
            .iter()
            .find(|row| row.key == "10|action|63")
            .unwrap();
        assert_eq!(dispatch.field(mismatched, "selector_word_4"), Some("41"));
        assert_eq!(mismatched.status, ImplementationStatus::StructuralOnly);

        let player = dispatch
            .rows
            .iter()
            .find(|row| row.key == "3|action|0")
            .unwrap();
        assert_eq!(
            dispatch.field(player, "resolved_function"),
            Some("AddPlayer03_00_5E010")
        );
        assert_eq!(player.status, ImplementationStatus::Partial);
    }

    #[test]
    fn implementation_overlay_never_drops_a_mapping() {
        let registry = AtlasRegistry::load_embedded().unwrap();
        let summary = registry.summary();
        assert_eq!(summary.tables, 24);
        assert_eq!(summary.mappings, 14_496);
        assert_eq!(summary.by_status.get("Partial"), Some(&43));
        assert_eq!(summary.by_status.values().sum::<usize>(), summary.mappings);
    }
}
