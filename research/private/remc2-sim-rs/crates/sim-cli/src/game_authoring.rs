use crate::game_contract::{
    DefinitionSource, game_domain_error, game_error, game_request_error, game_success,
    load_definition,
};
use sim_core::game_definition::{
    ApplyStatus, CHANGE_SET_SCHEMA, ChangeSet, DefinitionOperation, Mc2OwnedInputs,
    PublicationFault, RequestedTiming, TransactionError, WorkspaceStore, default_catalog,
};
use std::{io::Read, path::PathBuf, process::ExitCode};

struct CommonWorkspaceRequest {
    workspace: PathBuf,
}

pub(crate) fn run(operation: &str, arguments: &[String]) -> ExitCode {
    match operation {
        "workspace-create" => workspace_create(arguments),
        "set" => stage_set(arguments),
        "validate" => validate(arguments, false),
        "diff" => validate(arguments, true),
        "preview" => preview(arguments),
        "apply" => apply(arguments),
        "apply-status" => apply_status(arguments),
        "rollback" => rollback(arguments),
        "inspect" => inspect(arguments),
        _ => game_request_error(
            operation,
            "UNKNOWN_OPERATION",
            "unknown authoring operation",
        ),
    }
}

fn workspace_create(arguments: &[String]) -> ExitCode {
    let mut workspace = None;
    let mut workspace_id = None;
    let mut package = None;
    let mut owner_export = None;
    let mut owner_closure = None;
    let mut owner_cache = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        "--workspace-id" => set_once(&mut workspace_id, value, flag),
        "--package" => set_once(&mut package, value, flag),
        "--owner-export" => set_once(&mut owner_export, value, flag),
        "--owner-closure" => set_once(&mut owner_closure, value, flag),
        "--owner-cache" => set_once(&mut owner_cache, value, flag),
        _ => Err(format!("unknown workspace-create argument: {flag}")),
    }) {
        return game_request_error("workspace-create", "INVALID_REQUEST", &error);
    }
    let Some(workspace) = workspace else {
        return game_request_error(
            "workspace-create",
            "MISSING_WORKSPACE",
            "--workspace is required",
        );
    };
    let Some(workspace_id) = workspace_id else {
        return game_request_error(
            "workspace-create",
            "MISSING_WORKSPACE_ID",
            "--workspace-id is required",
        );
    };
    let source = match (package, owner_export, owner_closure, owner_cache) {
        (Some(package), None, None, None) => DefinitionSource::Portable(PathBuf::from(package)),
        (None, Some(export), Some(closure), Some(cache)) => {
            DefinitionSource::Owner(Mc2OwnedInputs::new(
                PathBuf::from(export),
                PathBuf::from(closure),
                PathBuf::from(cache),
            ))
        }
        _ => {
            return game_request_error(
                "workspace-create",
                "AMBIGUOUS_DEFINITION_SOURCE",
                "provide --package or all three owner input paths",
            );
        }
    };
    let catalog = match default_catalog() {
        Ok(value) => value,
        Err(error) => return game_domain_error("workspace-create", &error),
    };
    let definition = match load_definition(source, &catalog) {
        Ok(value) => value,
        Err(error) => return game_domain_error("workspace-create", &error),
    };
    let store = match WorkspaceStore::create(&workspace, &workspace_id, definition) {
        Ok(value) => value,
        Err(error) => return transaction_error("workspace-create", &error),
    };
    workspace_success(
        "workspace-create",
        &store,
        serde_json::json!({"created": true}),
    )
}

fn stage_set(arguments: &[String]) -> ExitCode {
    let mut workspace = None;
    let mut idempotency_key = None;
    let mut timing = None;
    let mut changes = None;
    let mut intent = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        "--idempotency-key" => set_once(&mut idempotency_key, value, flag),
        "--timing" => set_once(&mut timing, value, flag),
        "--changes" => set_once(&mut changes, value, flag),
        "--intent" => set_once(&mut intent, value, flag),
        _ => Err(format!("unknown set argument: {flag}")),
    }) {
        return game_request_error("set", "INVALID_REQUEST", &error);
    }
    let Some(workspace) = workspace else {
        return game_request_error("set", "MISSING_WORKSPACE", "--workspace is required");
    };
    let Some(idempotency_key) = idempotency_key else {
        return game_request_error(
            "set",
            "MISSING_IDEMPOTENCY_KEY",
            "--idempotency-key is required",
        );
    };
    let Some(timing) = timing.and_then(|value| parse_timing(&value)) else {
        return game_request_error(
            "set",
            "INVALID_TIMING",
            "--timing must be session_reset or package_reload",
        );
    };
    let Some(changes) = changes else {
        return game_request_error(
            "set",
            "MISSING_CHANGES",
            "--changes <JSON-file-or-> is required",
        );
    };
    let operations: Vec<DefinitionOperation> = match read_json(&changes) {
        Ok(value) => value,
        Err(error) => return game_request_error("set", "INVALID_REQUEST", &error),
    };
    let store = WorkspaceStore::open(&workspace);
    let inspection = match store.inspect() {
        Ok(value) => value,
        Err(error) => return transaction_error("set", &error),
    };
    let change_set = ChangeSet {
        schema_version: CHANGE_SET_SCHEMA.to_owned(),
        target_workspace: inspection.workspace_id,
        expected_definition_hash: inspection.active_definition_hash,
        expected_definition_revision: inspection.active_definition_revision,
        idempotency_key,
        operations,
        requested_timing: timing,
        intent,
    };
    let change_set = match change_set.canonicalized() {
        Ok(value) => value,
        Err(error) => return transaction_error("set", &error),
    };
    workspace_success(
        "set",
        &store,
        serde_json::json!({
            "change_set": change_set,
            "change_set_hash": change_set.canonical_hash(),
            "published": false,
        }),
    )
}

fn validate(arguments: &[String], include_diff: bool) -> ExitCode {
    let operation = if include_diff { "diff" } else { "validate" };
    let (store, change_set) = match workspace_change_set(operation, arguments) {
        Ok(value) => value,
        Err(exit) => return exit,
    };
    let catalog = match default_catalog() {
        Ok(value) => value,
        Err(error) => return game_domain_error(operation, &error),
    };
    if include_diff {
        match store.diff(&catalog, change_set) {
            Ok((receipt, diff)) => workspace_success(
                operation,
                &store,
                serde_json::json!({"validation": receipt, "diff": diff}),
            ),
            Err(error) => transaction_error(operation, &error),
        }
    } else {
        match store.validate(&catalog, change_set) {
            Ok(receipt) => workspace_success(
                operation,
                &store,
                serde_json::json!({"validation": receipt}),
            ),
            Err(error) => transaction_error(operation, &error),
        }
    }
}

fn preview(arguments: &[String]) -> ExitCode {
    let (store, change_set) = match workspace_change_set("preview", arguments) {
        Ok(value) => value,
        Err(exit) => return exit,
    };
    let catalog = match default_catalog() {
        Ok(value) => value,
        Err(error) => return game_domain_error("preview", &error),
    };
    match store.diff(&catalog, change_set.clone()) {
        Ok((validation, diff)) => {
            let apply_request = validation.accepted.then(|| {
                serde_json::json!({
                    "workspace": store.root().display().to_string(),
                    "change_set": change_set,
                    "validation_receipt_hash": validation.receipt_hash,
                })
            });
            workspace_success(
                "preview",
                &store,
                serde_json::json!({
                    "affected_descriptors": validation.affected_ids,
                    "validation": validation,
                    "diff": diff,
                    "apply_request": apply_request,
                    "published": false,
                }),
            )
        }
        Err(error) => transaction_error("preview", &error),
    }
}

fn apply(arguments: &[String]) -> ExitCode {
    let mut validation_receipt_hash = None;
    let filtered = match take_pair(
        arguments,
        "--validation-receipt",
        &mut validation_receipt_hash,
    ) {
        Ok(value) => value,
        Err(error) => return game_request_error("apply", "INVALID_REQUEST", &error),
    };
    let Some(validation_receipt_hash) = validation_receipt_hash else {
        return game_request_error(
            "apply",
            "MISSING_VALIDATION_RECEIPT",
            "--validation-receipt is required",
        );
    };
    let (store, change_set) = match workspace_change_set("apply", &filtered) {
        Ok(value) => value,
        Err(exit) => return exit,
    };
    let catalog = match default_catalog() {
        Ok(value) => value,
        Err(error) => return game_domain_error("apply", &error),
    };
    match store.apply(
        &catalog,
        change_set,
        &validation_receipt_hash,
        publication_fault_from_test_environment(),
    ) {
        Ok(receipt) => workspace_success("apply", &store, serde_json::json!({"receipt": receipt})),
        Err(error) => transaction_error("apply", &error),
    }
}

fn apply_status(arguments: &[String]) -> ExitCode {
    let mut workspace = None;
    let mut idempotency_key = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        "--idempotency-key" => set_once(&mut idempotency_key, value, flag),
        _ => Err(format!("unknown apply-status argument: {flag}")),
    }) {
        return game_request_error("apply-status", "INVALID_REQUEST", &error);
    }
    let (Some(workspace), Some(idempotency_key)) = (workspace, idempotency_key) else {
        return game_request_error(
            "apply-status",
            "INVALID_REQUEST",
            "--workspace and --idempotency-key are required",
        );
    };
    let store = WorkspaceStore::open(workspace);
    match store.apply_status(&idempotency_key) {
        Ok(ApplyStatus::NotStarted) => workspace_success(
            "apply-status",
            &store,
            serde_json::json!({"status": "not_started", "receipt": null}),
        ),
        Ok(ApplyStatus::Committed(receipt)) => workspace_success(
            "apply-status",
            &store,
            serde_json::json!({"status": "committed", "receipt": receipt}),
        ),
        Err(error) => transaction_error("apply-status", &error),
    }
}

fn rollback(arguments: &[String]) -> ExitCode {
    let mut workspace = None;
    let mut expected_hash = None;
    let mut expected_revision = None;
    let mut source_transaction = None;
    let mut idempotency_key = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        "--expected-definition-hash" => set_once(&mut expected_hash, value, flag),
        "--expected-definition-revision" => set_once(&mut expected_revision, value, flag),
        "--source-transaction" => set_once(&mut source_transaction, value, flag),
        "--idempotency-key" => set_once(&mut idempotency_key, value, flag),
        _ => Err(format!("unknown rollback argument: {flag}")),
    }) {
        return game_request_error("rollback", "INVALID_REQUEST", &error);
    }
    let (
        Some(workspace),
        Some(expected_hash),
        Some(expected_revision),
        Some(source_transaction),
        Some(idempotency_key),
    ) = (
        workspace,
        expected_hash,
        expected_revision,
        source_transaction,
        idempotency_key,
    )
    else {
        return game_request_error(
            "rollback",
            "INVALID_REQUEST",
            "rollback requires workspace, expected hash/revision, source transaction, and idempotency key",
        );
    };
    let expected_revision = match expected_revision.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return game_request_error(
                "rollback",
                "INVALID_REQUEST",
                "expected revision must be u64",
            );
        }
    };
    let store = WorkspaceStore::open(workspace);
    match store.rollback(
        &expected_hash,
        expected_revision,
        &source_transaction,
        &idempotency_key,
        publication_fault_from_test_environment(),
    ) {
        Ok(receipt) => {
            workspace_success("rollback", &store, serde_json::json!({"receipt": receipt}))
        }
        Err(error) => transaction_error("rollback", &error),
    }
}

fn publication_fault_from_test_environment() -> PublicationFault {
    #[cfg(debug_assertions)]
    {
        match std::env::var("CARPET_FLIGHT_TEST_PUBLICATION_FAULT").as_deref() {
            Ok("pause_before_commit") => PublicationFault::PauseBeforeCommit,
            Ok("pause_after_commit") => PublicationFault::PauseAfterCommit,
            _ => PublicationFault::None,
        }
    }
    #[cfg(not(debug_assertions))]
    {
        PublicationFault::None
    }
}

fn inspect(arguments: &[String]) -> ExitCode {
    let request = match common_workspace_request("inspect", arguments) {
        Ok(value) => value,
        Err(exit) => return exit,
    };
    let store = WorkspaceStore::open(request.workspace);
    let catalog = match default_catalog() {
        Ok(value) => value,
        Err(error) => return game_domain_error("inspect", &error),
    };
    let writable_surface = catalog
        .descriptors()
        .filter(|descriptor| descriptor.access.as_str() == "writable")
        .map(|descriptor| descriptor.id.as_str())
        .collect::<Vec<_>>();
    match store.inspect() {
        Ok(inspection) => workspace_success(
            "inspect",
            &store,
            serde_json::json!({
                "workspace": inspection,
                "writable_surface": writable_surface,
                "no_claim": "definition-writable does not imply a live gameplay consumer uses the published value",
                "recommended_operations": ["describe", "set", "preview"]
            }),
        ),
        Err(error) => transaction_error("inspect", &error),
    }
}

fn workspace_change_set(
    operation: &str,
    arguments: &[String],
) -> Result<(WorkspaceStore, ChangeSet), ExitCode> {
    let mut workspace = None;
    let mut change_set_path = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        "--change-set" => set_once(&mut change_set_path, value, flag),
        _ => Err(format!("unknown {operation} argument: {flag}")),
    }) {
        return Err(game_request_error(operation, "INVALID_REQUEST", &error));
    }
    let (Some(workspace), Some(change_set_path)) = (workspace, change_set_path) else {
        return Err(game_request_error(
            operation,
            "INVALID_REQUEST",
            "--workspace and --change-set are required",
        ));
    };
    let change_set = read_json(&change_set_path)
        .map_err(|error| game_request_error(operation, "INVALID_REQUEST", &error))?;
    Ok((WorkspaceStore::open(workspace), change_set))
}

fn common_workspace_request(
    operation: &str,
    arguments: &[String],
) -> Result<CommonWorkspaceRequest, ExitCode> {
    let mut workspace = None;
    if let Err(error) = parse_pairs(arguments, |flag, value| match flag {
        "--workspace" => set_once(&mut workspace, value, flag),
        _ => Err(format!("unknown {operation} argument: {flag}")),
    }) {
        return Err(game_request_error(operation, "INVALID_REQUEST", &error));
    }
    workspace
        .map(|workspace| CommonWorkspaceRequest {
            workspace: PathBuf::from(workspace),
        })
        .ok_or_else(|| {
            game_request_error(operation, "MISSING_WORKSPACE", "--workspace is required")
        })
}

fn workspace_success(operation: &str, store: &WorkspaceStore, data: serde_json::Value) -> ExitCode {
    match store.active_definition() {
        Ok(definition) => game_success(
            operation,
            definition.definition_hash(),
            data,
            Some(&definition),
        ),
        Err(error) => transaction_error(operation, &error),
    }
}

fn transaction_error(operation: &str, error: &TransactionError) -> ExitCode {
    game_error(operation, error.to_json(operation));
    if error.code.as_str() == "INVALID_REQUEST" {
        ExitCode::from(2)
    } else {
        ExitCode::from(3)
    }
}

fn parse_pairs(
    arguments: &[String],
    mut accept: impl FnMut(&str, String) -> Result<(), String>,
) -> Result<(), String> {
    if !arguments.len().is_multiple_of(2) {
        return Err("every option requires one value".to_owned());
    }
    for pair in arguments.chunks_exact(2) {
        accept(&pair[0], pair[1].clone())?;
    }
    Ok(())
}

fn set_once(target: &mut Option<String>, value: String, flag: &str) -> Result<(), String> {
    if target.replace(value).is_some() {
        Err(format!("{flag} appears more than once"))
    } else {
        Ok(())
    }
}

fn parse_timing(value: &str) -> Option<RequestedTiming> {
    match value {
        "session_reset" => Some(RequestedTiming::SessionReset),
        "package_reload" => Some(RequestedTiming::PackageReload),
        _ => None,
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let mut bytes = Vec::new();
    if path == "-" {
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read JSON stdin: {error}"))?;
    } else {
        bytes = std::fs::read(path).map_err(|error| format!("read JSON file: {error}"))?;
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid JSON schema: {error}"))
}

fn take_pair(
    arguments: &[String],
    wanted: &str,
    target: &mut Option<String>,
) -> Result<Vec<String>, String> {
    let mut filtered = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if flag == wanted {
            set_once(target, value.clone(), wanted)?;
        } else {
            filtered.push(flag.clone());
            filtered.push(value.clone());
        }
        index += 2;
    }
    Ok(filtered)
}
