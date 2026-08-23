use std::collections::BTreeSet;

const NATIVE_WORLD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/native_world.rs"));
const CAMPAIGN_STATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/campaign/state.rs"
));
const SOURCE_ARENA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/campaign/source_arena.rs"
));
const MISSION_CONTENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/mission_content.rs"
));
const CENSUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../docs/campaign-level2-state-census.md"
));

fn source_fields(source: &str, struct_name: &str) -> Result<BTreeSet<String>, String> {
    let marker = format!("struct {struct_name} {{");
    let Some((_, declaration)) = source.split_once(&marker) else {
        return Err(format!("missing {struct_name} declaration"));
    };
    let Some((body, _)) = declaration.split_once("\n}") else {
        return Err(format!("unterminated {struct_name} declaration"));
    };
    Ok(body
        .lines()
        .filter_map(|line| {
            let (prefix, _) = line.trim().split_once(':')?;
            let name = prefix.split_whitespace().last()?;
            (!name.is_empty()
                && name
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric()))
            .then(|| name.to_owned())
        })
        .collect())
}

fn census_fields(census: &str, struct_name: &str) -> Result<BTreeSet<String>, String> {
    let struct_cell = format!("`{struct_name}`");
    Ok(census
        .lines()
        .filter_map(|line| {
            let columns = line
                .strip_prefix('|')?
                .strip_suffix('|')?
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            (columns.first().copied() == Some(struct_cell.as_str()) && columns.len() > 1)
                .then_some(columns)
        })
        .flat_map(|columns| match columns.get(1) {
            Some(field_cell) => field_cell
                .split('`')
                .enumerate()
                .filter_map(|(index, value)| (index % 2 == 1).then_some(value.to_owned()))
                .collect::<Vec<_>>(),
            None => Vec::new(),
        })
        .collect())
}

fn compare_fields(source: &str, census: &str, struct_name: &str) -> Result<(), String> {
    let source_fields = source_fields(source, struct_name)?;
    let census_fields = census_fields(census, struct_name)?;
    let missing = source_fields
        .difference(&census_fields)
        .cloned()
        .collect::<Vec<_>>();
    let extra = census_fields
        .difference(&source_fields)
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() && extra.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{struct_name}: missing={missing:?}, extra={extra:?}"
        ))
    }
}

#[test]
fn census_table_matches_canonical_carriers() -> Result<(), String> {
    for (source, struct_name) in [
        (CAMPAIGN_STATE, "CampaignDefinition"),
        (CAMPAIGN_STATE, "CanonicalRuntimeState"),
        (CAMPAIGN_STATE, "ScratchState"),
        (CAMPAIGN_STATE, "CampaignState"),
        (CAMPAIGN_STATE, "CampaignSession"),
        (NATIVE_WORLD, "Level2LiveState"),
        (NATIVE_WORLD, "Level2ResetSession"),
    ] {
        compare_fields(source, CENSUS, struct_name)?;
    }
    Ok(())
}

#[test]
fn exactly_one_session_baseline_and_no_obsolete_twins() -> Result<(), String> {
    let session_fields = source_fields(CAMPAIGN_STATE, "CampaignSession")?;
    let baseline_owners = session_fields
        .iter()
        .filter(|field| field.as_str() == "baseline")
        .count();
    if baseline_owners != 1 {
        return Err(format!(
            "CampaignSession must own exactly one baseline, found {baseline_owners}"
        ));
    }

    for (source, struct_name) in [
        (NATIVE_WORLD, "Level2LiveState"),
        (NATIVE_WORLD, "Level2ResetSession"),
        (NATIVE_WORLD, "Level2EconomyState"),
        (NATIVE_WORLD, "Level2SharedSpellResetSession"),
        (NATIVE_WORLD, "Level2OrdinaryResetSession"),
        (CAMPAIGN_STATE, "CanonicalRuntimeState"),
        (CAMPAIGN_STATE, "ScratchState"),
        (CAMPAIGN_STATE, "CampaignState"),
        (SOURCE_ARENA, "SourceArena"),
        (MISSION_CONTENT, "MissionContentSession"),
    ] {
        let obsolete = source_fields(source, struct_name)?
            .into_iter()
            .filter(|field| field.starts_with("baseline_"))
            .collect::<Vec<_>>();
        if !obsolete.is_empty() {
            return Err(format!("{struct_name} has obsolete twins: {obsolete:?}"));
        }
    }
    Ok(())
}

#[test]
fn canonical_shell_shapes_are_exact() -> Result<(), String> {
    let expected_session = BTreeSet::from([
        "definition".to_owned(),
        "baseline".to_owned(),
        "baseline_hash".to_owned(),
        "state".to_owned(),
        "generation".to_owned(),
    ]);
    let expected_state = BTreeSet::from([
        "persistent".to_owned(),
        "live".to_owned(),
        "scratch".to_owned(),
    ]);
    let expected_facade = BTreeSet::from(["session".to_owned()]);
    for (actual, expected, name) in [
        (
            source_fields(CAMPAIGN_STATE, "CampaignSession")?,
            expected_session,
            "CampaignSession",
        ),
        (
            source_fields(CAMPAIGN_STATE, "CampaignState")?,
            expected_state,
            "CampaignState",
        ),
        (
            source_fields(NATIVE_WORLD, "Level2ResetSession")?,
            expected_facade,
            "Level2ResetSession",
        ),
    ] {
        if actual != expected {
            return Err(format!("{name}: expected={expected:?}, actual={actual:?}"));
        }
    }
    Ok(())
}

#[test]
fn planted_missing_field_is_rejected() -> Result<(), String> {
    let source = "pub struct Example {\n    first: u8,\n    second: u8,\n}";
    let census = "| `Example` | `first` | resettable live |";
    let error = match compare_fields(source, census, "Example") {
        Ok(()) => return Err("omitted field must fail".to_owned()),
        Err(error) => error,
    };
    if !error.contains("second") {
        return Err(format!("diagnostic should name the omitted field: {error}"));
    }
    Ok(())
}
