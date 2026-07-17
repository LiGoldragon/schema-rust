use std::{path::PathBuf, process::Command};

#[test]
fn schema_rust_cli_generates_environment_backed_feedback() {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime_root = manifest_directory.join("tests/fixtures/driver-runtime");
    let contract_schema_directory =
        manifest_directory.join("tests/fixtures/driver-contract/schema");
    let request = format!(
        "Generate.{{ {} driver-runtime 0.1.0 [NexusRuntime.nexus SemaRuntime.sema] [{{ driver-contract {} 0.1.0 }}] }}",
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
    assert!(stdout.contains("Generated.{"));
    assert!(stdout.contains("nexus"));
    assert!(stdout.contains("src/schema/nexus.rs"));
    assert!(stdout.contains("driver-contract.lib.[DriverInput DriverOutput]"));
    assert!(stdout.contains("sema"));
    assert!(stdout.contains("src/schema/sema.rs"));
}

#[test]
fn schema_rust_cli_pretty_flag_reflows_the_same_document() {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime_root = manifest_directory.join("tests/fixtures/driver-runtime");
    let contract_schema_directory =
        manifest_directory.join("tests/fixtures/driver-contract/schema");
    let request = format!(
        "Generate.{{ {} driver-runtime 0.1.0 [NexusRuntime.nexus SemaRuntime.sema] [{{ driver-contract {} 0.1.0 }}] }}",
        runtime_root.display(),
        contract_schema_directory.display()
    );

    let canonical = Command::new(env!("CARGO_BIN_EXE_schema-rust"))
        .arg(&request)
        .output()
        .expect("run schema-rust CLI");
    let pretty = Command::new(env!("CARGO_BIN_EXE_schema-rust"))
        .args(["--pretty", &request])
        .output()
        .expect("run schema-rust CLI with --pretty");

    assert!(canonical.status.success());
    assert!(
        pretty.status.success(),
        "schema-rust --pretty failed:\n{}",
        String::from_utf8_lossy(&pretty.stderr)
    );

    let canonical_stdout = String::from_utf8(canonical.stdout).expect("stdout is UTF-8");
    let pretty_stdout = String::from_utf8(pretty.stdout).expect("stdout is UTF-8");

    // The pretty form is genuinely reflowed across lines, not the single-line
    // default.
    assert!(pretty_stdout.contains('\n'));
    assert!(pretty_stdout.matches('\n').count() > 1);

    // The pretty form re-parses to the same document: collapsing it back to a
    // single line reproduces the canonical output byte-for-byte.
    let reparsed =
        nota::Document::parse(pretty_stdout.trim()).expect("pretty output is valid nota");
    let collapsed = reparsed
        .root_objects()
        .iter()
        .map(|block| block.render_inline(reparsed.source()))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(collapsed, canonical_stdout.trim());
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
