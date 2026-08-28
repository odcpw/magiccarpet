use sim_core::game_definition::{
    CHANGE_SET_SCHEMA, DefinitionError, DefinitionOperation, GAME_DEFINITION_SCHEMA,
    GameDefinition, GamePackage, Mc2OwnedInputs, SemanticCatalog, SemanticId, default_catalog,
    engine_capabilities, load_mc2_owned,
};
use std::process::ExitCode;

pub(crate) enum DefinitionSource {
    Portable(std::path::PathBuf),
    Campaign(std::path::PathBuf),
    Owner(Mc2OwnedInputs),
    Workspace(std::path::PathBuf),
}

struct DefinitionRequest {
    source: DefinitionSource,
    id: Option<SemanticId>,
    limit: usize,
    cursor: Option<String>,
}

pub(crate) fn run() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    let Some(operation) = arguments.get(2).map(String::as_str) else {
        return game_request_error(
            "game",
            "MISSING_OPERATION",
            "expected capabilities, describe, entities, objectives, get, workspace-create, set, validate, diff, apply, apply-status, rollback, inspect, or preview",
        );
    };
    match operation {
        "capabilities" if arguments.len() == 3 => game_capabilities(),
        "describe" => game_describe(&arguments[3..]),
        "entities" => game_entities(&arguments[3..]),
        "objectives" => game_objectives(&arguments[3..]),
        "get" => game_get(&arguments[3..]),
        "workspace-create" | "set" | "validate" | "diff" | "apply" | "apply-status"
        | "rollback" | "inspect" | "preview" => {
            crate::game_authoring::run(operation, &arguments[3..])
        }
        "reset" => game_request_error(
            operation,
            "SESSION_CONTRACT_NOT_READY",
            "reset belongs to F4 session binding and is not implemented by definition transactions",
        ),
        "capabilities" => game_request_error(
            operation,
            "UNEXPECTED_ARGUMENT",
            "capabilities accepts no arguments",
        ),
        _ => game_request_error(
            operation,
            "UNKNOWN_OPERATION",
            "expected capabilities, describe, entities, objectives, get, workspace-create, set, validate, diff, apply, apply-status, rollback, inspect, or preview",
        ),
    }
}

fn game_capabilities() -> ExitCode {
    let catalog = match default_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return game_domain_error("capabilities", &error),
    };
    let registry = match engine_capabilities() {
        Ok(registry) => registry,
        Err(error) => return game_domain_error("capabilities", &error),
    };
    let data = serde_json::json!({
        "catalog_hash": catalog.catalog_hash(),
        "read_only": catalog.descriptors().all(|descriptor| descriptor.access.as_str() == "read_only"),
        "authoring_transaction_available": true,
        "authoring": {
            "change_set_schema": CHANGE_SET_SCHEMA,
            "game_definition_schema": GAME_DEFINITION_SCHEMA,
            "operation_schema": DefinitionOperation::schema_json(),
        },
        "writable_descriptor_count": catalog.descriptors().filter(|descriptor| descriptor.access.as_str() == "writable").count(),
        "limits": {"default_page_size": 100, "maximum_page_size": 100},
        "operations": {
            "capabilities": {"mutation": "none"},
            "describe": {"mutation": "none", "arguments": ["definition_source", "id", "limit", "cursor"]},
            "entities": {"mutation": "none", "arguments": ["definition_source", "id", "limit", "cursor"]},
            "objectives": {"mutation": "none", "arguments": ["definition_source", "id", "limit", "cursor"]},
            "get": {"mutation": "none", "arguments": ["definition_source", "id"]},
            "workspace-create": {"mutation": "create_workspace"},
            "set": {"mutation": "stage_only"},
            "validate": {"mutation": "none"},
            "diff": {"mutation": "none"},
            "apply": {"mutation": "atomic_commit"},
            "apply-status": {"mutation": "none"},
            "rollback": {"mutation": "atomic_commit"},
            "inspect": {"mutation": "none"},
            "preview": {"mutation": "none"}
        },
        "compiled_capabilities": registry.iter().map(|capability| capability.to_json()).collect::<Vec<_>>(),
        "semantic_ids": catalog.descriptors().map(|descriptor| descriptor.id.as_str()).collect::<Vec<_>>(),
    });
    game_success("capabilities", catalog.catalog_hash(), data, None)
}

fn game_entities(arguments: &[String]) -> ExitCode {
    let request = match parse_definition_request(arguments, false) {
        Ok(request) => request,
        Err((code, message)) => return game_request_error("entities", code, &message),
    };
    let catalog = match default_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return game_domain_error("entities", &error),
    };
    let definition = match load_definition(request.source, &catalog) {
        Ok(definition) => definition,
        Err(error) => return game_domain_error("entities", &error),
    };
    let Some(level) = definition.level_definition() else {
        return game_request_error(
            "entities",
            "NO_LEVEL_DEFINITION",
            "definition contains no semantic level entities",
        );
    };
    let entities = if let Some(id) = request.id {
        let Ok(id) = sim_core::level_definition::LevelEntityId::parse(id.as_str()) else {
            return game_unknown_id("entities", id.as_str());
        };
        let Some(entity) = level.entity(&id) else {
            return game_unknown_id("entities", id.as_str());
        };
        vec![public_level_entity_json(entity)]
    } else {
        level
            .entities()
            .iter()
            .map(public_level_entity_json)
            .collect()
    };
    let offset = match parse_cursor(
        request.cursor.as_deref(),
        catalog.catalog_hash(),
        definition.definition_hash(),
    ) {
        Ok(offset) => offset,
        Err(message) => return game_request_error("entities", "INVALID_CURSOR", &message),
    };
    let total = entities.len();
    if offset > total {
        return game_request_error(
            "entities",
            "INVALID_CURSOR",
            "cursor offset exceeds result set",
        );
    }
    let end = offset.saturating_add(request.limit).min(total);
    let entities = entities[offset..end].to_vec();
    let next_cursor = (end < total).then(|| {
        format!(
            "v1.{}.{}.{}",
            catalog.catalog_hash().trim_start_matches("sha256:"),
            definition.definition_hash().trim_start_matches("sha256:"),
            end
        )
    });
    let data = serde_json::json!({
        "entities": entities,
        "page": {
            "total": total,
            "limit": request.limit,
            "next_cursor": next_cursor,
            "complete": end == total,
        },
    });
    game_success(
        "entities",
        definition.definition_hash(),
        data,
        Some(&definition),
    )
}

fn game_objectives(arguments: &[String]) -> ExitCode {
    let request = match parse_definition_request(arguments, false) {
        Ok(request) => request,
        Err((code, message)) => return game_request_error("objectives", code, &message),
    };
    let catalog = match default_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return game_domain_error("objectives", &error),
    };
    let definition = match load_definition(request.source, &catalog) {
        Ok(definition) => definition,
        Err(error) => return game_domain_error("objectives", &error),
    };
    let Some(level) = definition.level_definition() else {
        return game_request_error(
            "objectives",
            "NO_LEVEL_DEFINITION",
            "definition contains no semantic level objectives",
        );
    };
    let objectives = if let Some(id) = request.id {
        let Ok(id) = sim_core::level_definition::LevelObjectiveId::parse(id.as_str()) else {
            return game_unknown_id("objectives", id.as_str());
        };
        let Some(objective) = level.objective(&id) else {
            return game_unknown_id("objectives", id.as_str());
        };
        vec![public_level_objective_json(objective)]
    } else {
        level
            .objectives()
            .iter()
            .map(public_level_objective_json)
            .collect()
    };
    let offset = match parse_cursor(
        request.cursor.as_deref(),
        catalog.catalog_hash(),
        definition.definition_hash(),
    ) {
        Ok(offset) => offset,
        Err(message) => return game_request_error("objectives", "INVALID_CURSOR", &message),
    };
    let total = objectives.len();
    if offset > total {
        return game_request_error(
            "objectives",
            "INVALID_CURSOR",
            "cursor offset exceeds result set",
        );
    }
    let end = offset.saturating_add(request.limit).min(total);
    let objectives = objectives[offset..end].to_vec();
    let next_cursor = (end < total).then(|| {
        format!(
            "v1.{}.{}.{}",
            catalog.catalog_hash().trim_start_matches("sha256:"),
            definition.definition_hash().trim_start_matches("sha256:"),
            end
        )
    });
    game_success(
        "objectives",
        definition.definition_hash(),
        serde_json::json!({
            "objectives": objectives,
            "page": {
                "total": total,
                "limit": request.limit,
                "next_cursor": next_cursor,
                "complete": end == total,
            },
        }),
        Some(&definition),
    )
}

fn game_describe(arguments: &[String]) -> ExitCode {
    let request = match parse_definition_request(arguments, false) {
        Ok(request) => request,
        Err((code, message)) => return game_request_error("describe", code, &message),
    };
    let catalog = match default_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return game_domain_error("describe", &error),
    };
    let definition = match load_definition(request.source, &catalog) {
        Ok(definition) => definition,
        Err(error) => return game_domain_error("describe", &error),
    };
    let descriptors = match request.id {
        Some(id) => {
            if let Some(descriptor) = catalog.descriptor(&id) {
                vec![descriptor.to_json(definition.value(&id))]
            } else if let Some(descriptor) = definition.level_entity_position_descriptor(&id) {
                let effective = definition.semantic_target_value(&id);
                vec![descriptor.to_json(effective.as_ref())]
            } else if let Some(descriptor) = definition.level_countdown_arming_ticks_descriptor(&id)
            {
                let effective = definition.semantic_target_value(&id);
                vec![descriptor.to_json(effective.as_ref())]
            } else if let Some(descriptor) = definition.level_objective_condition_descriptor(&id) {
                let effective = definition.semantic_target_value(&id);
                vec![descriptor.to_json(effective.as_ref())]
            } else {
                return game_unknown_id("describe", id.as_str());
            }
        }
        None => {
            let mut descriptors = catalog
                .descriptors()
                .map(|descriptor| descriptor.to_json(definition.value(&descriptor.id)))
                .collect::<Vec<_>>();
            descriptors.extend(
                definition
                    .level_entity_position_descriptors()
                    .into_iter()
                    .map(|descriptor| {
                        let effective = definition.semantic_target_value(&descriptor.id);
                        descriptor.to_json(effective.as_ref())
                    }),
            );
            descriptors.extend(
                definition
                    .level_countdown_arming_ticks_descriptors()
                    .into_iter()
                    .map(|descriptor| {
                        let effective = definition.semantic_target_value(&descriptor.id);
                        descriptor.to_json(effective.as_ref())
                    }),
            );
            descriptors.extend(
                definition
                    .level_objective_condition_descriptors()
                    .into_iter()
                    .map(|descriptor| {
                        let effective = definition.semantic_target_value(&descriptor.id);
                        descriptor.to_json(effective.as_ref())
                    }),
            );
            descriptors
        }
    };
    let offset = match parse_cursor(
        request.cursor.as_deref(),
        catalog.catalog_hash(),
        definition.definition_hash(),
    ) {
        Ok(offset) => offset,
        Err(message) => return game_request_error("describe", "INVALID_CURSOR", &message),
    };
    let total = descriptors.len();
    if offset > total {
        return game_request_error(
            "describe",
            "INVALID_CURSOR",
            "cursor offset exceeds result set",
        );
    }
    let end = offset.saturating_add(request.limit).min(total);
    let descriptors = descriptors[offset..end].to_vec();
    let next_cursor = (end < total).then(|| {
        format!(
            "v1.{}.{}.{}",
            catalog.catalog_hash().trim_start_matches("sha256:"),
            definition.definition_hash().trim_start_matches("sha256:"),
            end
        )
    });
    let data = serde_json::json!({
        "catalog_hash": catalog.catalog_hash(),
        "descriptors": descriptors,
        "page": {
            "total": total,
            "limit": request.limit,
            "next_cursor": next_cursor,
            "complete": end == total,
        },
    });
    game_success(
        "describe",
        definition.definition_hash(),
        data,
        Some(&definition),
    )
}

fn game_get(arguments: &[String]) -> ExitCode {
    let request = match parse_definition_request(arguments, true) {
        Ok(request) => request,
        Err((code, message)) => return game_request_error("get", code, &message),
    };
    let catalog = match default_catalog() {
        Ok(catalog) => catalog,
        Err(error) => return game_domain_error("get", &error),
    };
    let definition = match load_definition(request.source, &catalog) {
        Ok(definition) => definition,
        Err(error) => return game_domain_error("get", &error),
    };
    let id = request.id.expect("get request parser requires an ID");
    let Some(descriptor) = catalog.descriptor(&id) else {
        if let Some(descriptor) = definition.level_entity_position_descriptor(&id) {
            let value = definition
                .semantic_target_value(&id)
                .expect("position descriptor has semantic readback");
            return game_success(
                "get",
                definition.definition_hash(),
                serde_json::json!({
                    "descriptor": descriptor.to_json(Some(&value)),
                    "value": value.to_json(),
                }),
                Some(&definition),
            );
        }
        if let Some(descriptor) = definition.level_objective_condition_descriptor(&id) {
            let value = definition
                .semantic_target_value(&id)
                .expect("objective descriptor has semantic readback");
            return game_success(
                "get",
                definition.definition_hash(),
                serde_json::json!({
                    "descriptor": descriptor.to_json(Some(&value)),
                    "value": value.to_json(),
                }),
                Some(&definition),
            );
        }
        if let Some(descriptor) = definition.level_countdown_arming_ticks_descriptor(&id) {
            let value = definition
                .semantic_target_value(&id)
                .expect("countdown descriptor has semantic readback");
            return game_success(
                "get",
                definition.definition_hash(),
                serde_json::json!({
                    "descriptor": descriptor.to_json(Some(&value)),
                    "value": value.to_json(),
                }),
                Some(&definition),
            );
        }
        let entity = sim_core::level_definition::LevelEntityId::parse(id.as_str())
            .ok()
            .and_then(|entity_id| {
                definition
                    .level_definition()
                    .and_then(|level| level.entity(&entity_id))
            });
        let Some(entity) = entity else {
            return game_unknown_id("get", id.as_str());
        };
        return game_success(
            "get",
            definition.definition_hash(),
            serde_json::json!({"entity": public_level_entity_json(entity)}),
            Some(&definition),
        );
    };
    let value = match catalog.read(&definition, &id) {
        Ok(value) => value,
        Err(error) => return game_domain_error("get", &error),
    };
    let data = serde_json::json!({
        "descriptor": descriptor.to_json(Some(&value)),
        "value": value.to_json(),
    });
    game_success("get", definition.definition_hash(), data, Some(&definition))
}

fn public_level_entity_json(
    entity: &sim_core::level_definition::LevelEntityDefinition,
) -> serde_json::Value {
    serde_json::json!({
        "id": entity.id(),
        "kind": entity.kind(),
        "position": entity.position(),
        "position_access": entity.position_access(),
    })
}

fn public_level_objective_json(
    objective: &sim_core::level_definition::LevelObjectiveDefinition,
) -> serde_json::Value {
    serde_json::json!({
        "id": objective.id(),
        "condition": objective.condition(),
        "condition_access": objective.condition_access(),
    })
}

fn parse_definition_request(
    arguments: &[String],
    require_id: bool,
) -> Result<DefinitionRequest, (&'static str, String)> {
    let mut package = None;
    let mut campaign_package = None;
    let mut workspace = None;
    let mut owner_export = None;
    let mut owner_closure = None;
    let mut owner_cache = None;
    let mut id = None;
    let mut limit = 100_usize;
    let mut cursor = None;
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let Some(value) = arguments.get(index + 1) else {
            return Err(("MISSING_VALUE", format!("{flag} requires a value")));
        };
        let target = match flag {
            "--package" => &mut package,
            "--campaign-package" => &mut campaign_package,
            "--workspace" => &mut workspace,
            "--owner-export" => &mut owner_export,
            "--owner-closure" => &mut owner_closure,
            "--owner-cache" => &mut owner_cache,
            "--id" => {
                if id.is_some() {
                    return Err((
                        "DUPLICATE_ARGUMENT",
                        "--id appears more than once".to_owned(),
                    ));
                }
                id = Some(
                    SemanticId::parse(value)
                        .map_err(|error| ("INVALID_SEMANTIC_ID", error.message))?,
                );
                index += 2;
                continue;
            }
            "--limit" => {
                limit = value
                    .parse::<usize>()
                    .ok()
                    .filter(|value| (1..=100).contains(value))
                    .ok_or_else(|| {
                        (
                            "INVALID_LIMIT",
                            "--limit must be an integer from 1 through 100".to_owned(),
                        )
                    })?;
                index += 2;
                continue;
            }
            "--cursor" => {
                if cursor.replace(value.clone()).is_some() {
                    return Err((
                        "DUPLICATE_ARGUMENT",
                        "--cursor appears more than once".to_owned(),
                    ));
                }
                index += 2;
                continue;
            }
            _ => {
                return Err((
                    "UNKNOWN_ARGUMENT",
                    format!("unknown game-contract argument: {flag}"),
                ));
            }
        };
        if target.is_some() {
            return Err((
                "DUPLICATE_ARGUMENT",
                format!("{flag} appears more than once"),
            ));
        }
        *target = Some(std::path::PathBuf::from(value));
        index += 2;
    }
    if require_id && id.is_none() {
        return Err(("MISSING_ID", "get requires --id <semantic-id>".to_owned()));
    }
    let source = match (
        workspace,
        package,
        campaign_package,
        owner_export,
        owner_closure,
        owner_cache,
    ) {
        (Some(path), None, None, None, None, None) => DefinitionSource::Workspace(path),
        (None, Some(path), None, None, None, None) => DefinitionSource::Portable(path),
        (None, None, Some(path), None, None, None) => DefinitionSource::Campaign(path),
        (None, None, None, Some(export), Some(closure), Some(cache)) => {
            DefinitionSource::Owner(Mc2OwnedInputs::new(export, closure, cache))
        }
        (None, None, None, None, None, None) => {
            return Err((
                "MISSING_DEFINITION_SOURCE",
                "provide --workspace, --package, --campaign-package, or all three --owner-export/--owner-closure/--owner-cache paths"
                    .to_owned(),
            ));
        }
        _ => {
            return Err((
                "AMBIGUOUS_DEFINITION_SOURCE",
                "workspace, portable, campaign, and owner inputs are mutually exclusive; owner input requires all three paths"
                    .to_owned(),
            ));
        }
    };
    if require_id && cursor.is_some() {
        return Err((
            "UNEXPECTED_ARGUMENT",
            "get does not accept --cursor".to_owned(),
        ));
    }
    Ok(DefinitionRequest {
        source,
        id,
        limit,
        cursor,
    })
}

fn parse_cursor(
    cursor: Option<&str>,
    catalog_hash: &str,
    definition_hash: &str,
) -> Result<usize, String> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let parts = cursor.split('.').collect::<Vec<_>>();
    if parts.len() != 4
        || parts[0] != "v1"
        || parts[1] != catalog_hash.trim_start_matches("sha256:")
        || parts[2] != definition_hash.trim_start_matches("sha256:")
    {
        return Err("cursor is malformed or belongs to a different catalog/definition".to_owned());
    }
    parts[3]
        .parse::<usize>()
        .map_err(|_| "cursor offset is malformed".to_owned())
}

pub(crate) fn load_definition(
    source: DefinitionSource,
    catalog: &SemanticCatalog,
) -> sim_core::game_definition::Result<GameDefinition> {
    match source {
        DefinitionSource::Portable(path) => {
            GameDefinition::from_package(GamePackage::load(path)?, &engine_capabilities()?, catalog)
        }
        DefinitionSource::Campaign(path) => {
            sim_core::campaign::backend::load_mc2_campaign(path, catalog)
        }
        DefinitionSource::Owner(inputs) => load_mc2_owned(&inputs, catalog),
        DefinitionSource::Workspace(path) => sim_core::game_definition::WorkspaceStore::open(path)
            .active_definition()
            .map_err(|error| DefinitionError {
                code: sim_core::game_definition::DefinitionErrorCode::Io,
                path_or_id: error.path_or_id,
                message: error.message,
            }),
    }
}

fn definition_identity(definition: &GameDefinition) -> serde_json::Value {
    serde_json::json!({
        "schema": definition.schema(),
        "revision": definition.revision(),
        "definition_hash": definition.definition_hash(),
        "package_id": definition.package_id(),
        "package_version": definition.package_version(),
        "package_graph_hash": definition.package_graph_hash(),
        "artifact_graph_hash": definition.artifact_graph_hash(),
        "provenance": {
            "kind": definition.provenance().kind.as_str(),
            "distribution": definition.provenance().distribution,
            "source_sha256": definition.provenance().source_sha256,
        },
    })
}

pub(crate) fn game_success(
    operation: &str,
    request_identity: &str,
    data: serde_json::Value,
    definition: Option<&GameDefinition>,
) -> ExitCode {
    let identity = definition.map_or_else(
        || {
            serde_json::json!({
                "package_graph_hash": null,
                "definition_hash": null,
                "session_id": null,
                "generation": null,
                "tick": null,
            })
        },
        definition_identity,
    );
    let value = serde_json::json!({
        "ok": true,
        "contract_version": sim_core::game_definition::SEMANTIC_CONTRACT_VERSION,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "request_id": format!("{operation}:{}", request_identity.trim_start_matches("sha256:")),
        "operation": operation,
        "data": data,
        "identity": identity,
        "warnings": [],
        "errors": [],
        "recommended_operations": [],
    });
    println!(
        "{}",
        serde_json::to_string(&value).expect("game-contract output is serializable")
    );
    ExitCode::SUCCESS
}

pub(crate) fn game_domain_error(operation: &str, error: &DefinitionError) -> ExitCode {
    game_error(operation, error.to_json(operation));
    ExitCode::from(3)
}

fn game_unknown_id(operation: &str, id: &str) -> ExitCode {
    game_error(
        operation,
        serde_json::json!({
            "code": "UNKNOWN_ID",
            "message": "semantic ID is not present in the catalog",
            "path_or_id": id,
            "retryable": false,
            "state_changed": false,
            "safe_alternative": "game capabilities",
            "expected_identity": null,
            "actual_identity": null,
        }),
    );
    ExitCode::from(3)
}

pub(crate) fn game_request_error(operation: &str, code: &str, message: &str) -> ExitCode {
    game_error(
        operation,
        serde_json::json!({
            "code": code,
            "message": message,
            "path_or_id": null,
            "retryable": false,
            "state_changed": false,
            "remediation": "repair the request and retry the same read-only operation",
            "safe_alternative": "sim-cli game capabilities",
            "expected_identity": null,
            "actual_identity": null,
        }),
    );
    ExitCode::from(2)
}

pub(crate) fn game_error(operation: &str, error: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "ok": false,
            "contract_version": sim_core::game_definition::SEMANTIC_CONTRACT_VERSION,
            "engine_version": env!("CARGO_PKG_VERSION"),
            "request_id": format!("{operation}:rejected"),
            "operation": operation,
            "data": null,
            "identity": {
                "package_graph_hash": null,
                "definition_hash": null,
                "session_id": null,
                "generation": null,
                "tick": null,
            },
            "warnings": [],
            "errors": [error],
            "recommended_operations": ["capabilities"],
        })
    );
}
