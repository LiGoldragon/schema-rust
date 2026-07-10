use std::{path::PathBuf, process::Command};

#[test]
fn schema_rust_cli_generates_environment_backed_feedback() {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime_root = manifest_directory.join("tests/fixtures/driver-runtime");
    let contract_schema_directory =
        manifest_directory.join("tests/fixtures/driver-contract/schema");
    let request = format!(
        "(Generate ({} driver-runtime 0.1.0 [(NexusRuntime nexus) (SemaRuntime sema)] [(driver-contract {} 0.1.0)]))",
        runtime_root.display(),
        contract_schema_directory.display()
    );

    let output = Command::new(env!("CARGO_BIN_EXE_schema-rust"))
        .arg(request)
        .output()
        .expect("run schema-rust CLI");

    assert!(
        output.status.success(),
        "schema-rust CLI failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("(Generated ("));
    assert!(stdout.contains("nexus"));
    assert!(stdout.contains("src/schema/nexus.rs"));
    assert!(stdout.contains("driver-contract.lib.[DriverInput DriverOutput]"));
    assert!(stdout.contains("sema"));
    assert!(stdout.contains("src/schema/sema.rs"));
}

#[test]
fn schema_rust_cli_enforces_single_argument_rule() {
    let output = Command::new(env!("CARGO_BIN_EXE_schema-rust"))
        .args(["one", "two"])
        .output()
        .expect("run schema-rust CLI");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("expected exactly one component argument"));
}
