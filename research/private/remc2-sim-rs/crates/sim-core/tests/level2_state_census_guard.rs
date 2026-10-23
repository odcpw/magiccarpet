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
