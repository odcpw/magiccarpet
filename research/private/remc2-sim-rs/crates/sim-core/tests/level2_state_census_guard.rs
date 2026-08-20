use std::collections::BTreeSet;

const SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/native_world.rs"));
const CENSUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../docs/campaign-level2-state-census.md"
));

fn source_fields(source: &str, struct_name: &str) -> Result<BTreeSet<String>, String> {
    let marker = format!("pub struct {struct_name} {{");
    let Some((_, declaration)) = source.split_once(&marker) else {
        return Err(format!("missing {struct_name} declaration"));
    };
    let Some((body, _)) = declaration.split_once("\n}") else {
        return Err(format!("unterminated {struct_name} declaration"));
    };
    Ok(body
        .lines()
        .filter_map(|line| {
            let field = line.trim().strip_prefix("pub ").unwrap_or(line.trim());
            field.split_once(':').and_then(|(name, _)| {
                let name = name.trim();
                (!name.is_empty()
                    && name
                        .chars()
                        .all(|character| character == '_' || character.is_ascii_alphanumeric()))
                .then(|| name.to_owned())
            })
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
        .collect::<BTreeSet<_>>())
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
fn census_table_matches_current_struct_fields() -> Result<(), String> {
    for struct_name in [
        "Level2EconomyState",
        "Level2SharedSpellResetSession",
        "Level2ResetSession",
    ] {
        compare_fields(SOURCE, CENSUS, struct_name)?;
    }
    Ok(())
}

#[test]
fn planted_missing_field_is_rejected() -> Result<(), String> {
    let source = "pub struct Example {\n    first: u8,\n    second: u8,\n}";
    let census = "| `Example` | `first` | resettable live | evidence | reset |";
    let error = match compare_fields(source, census, "Example") {
        Ok(()) => return Err("omitted field must fail".to_owned()),
        Err(error) => error,
    };
    if !error.contains("second") {
        return Err(format!("diagnostic should name the omitted field: {error}"));
    }
    Ok(())
}
