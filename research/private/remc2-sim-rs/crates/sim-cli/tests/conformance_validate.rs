use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(script: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sim-cli-conformance-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("validator.py");
    std::fs::write(&path, script).unwrap();
    path
}

#[test]
fn conformance_validate_returns_zero_only_for_closed_validator_report() {
    let passing = fixture("print('{\"ok\":true,\"closed\":true}')\n");
    let output = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args(["conformance-validate", "--validator"])
        .arg(&passing)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        r#"{"ok":true,"closed":true}"#
    );

    let failing = fixture(
        "print('{\"ok\":false,\"closed\":false,\"error\":\"mutation\"}')\nraise SystemExit(1)\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args(["conformance-validate", "--validator"])
        .arg(&failing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(r#""closed":false"#));
    assert!(!stdout.contains("PASS"));
}

#[test]
fn conformance_validate_rejects_missing_validator_as_invocation_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args([
            "conformance-validate",
            "--validator",
            "/definitely/missing/plan29-validator.py",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(r#""closed":false"#));
    assert!(!stdout.contains("PASS"));
}

#[test]
fn conformance_validate_forwards_final_arguments_without_an_oracle() {
    let validator = fixture(
        "import json,sys\nprint(json.dumps({'ok': True, 'closed': True, 'argv': sys.argv[1:]}))\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_sim-cli"))
        .args(["conformance-validate", "--validator"])
        .arg(&validator)
        .args([
            "--final-closure",
            "--closure-contract",
            "contract.json",
            "--receipts",
            "receipts.json",
            "--cache",
            "cache",
            "--expected-contract-sha256",
            "00",
            "matrix.json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--final-closure"));
    assert!(stdout.contains("--expected-contract-sha256"));
    assert!(!stdout.to_ascii_lowercase().contains("oracle"));
}
