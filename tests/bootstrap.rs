use std::{fs, process::Command};

use core_ethos::bootstrap::EthosKind;
use core_nomos::InterfaceRoleTraitIdentities;
use rust_logos::{RustLogos, RustTypePath, RustTypePathResolver};
use schema_rust::{
    bootstrap::{
        BOOTSTRAP_GENERATED_MARKER, ArtifactSet, BootstrapGeneration, BootstrapGenerationError,
        CommitBootstrap,
    },
    build::BuildError,
};
use sema_translator::bootstrap::{SemaBootstrapAuthority, SourcePlacement};

const INTERFACE_SOURCE: &str = "Interface.{1 0 0}\n[]\n{[] [] [] [] []}";
const SEMA_SOURCE: &str = "Sema.{1 0 0}\n[]\n{[] []}";
const NEXUS_SOURCE: &str = "Nexus.{1 0 0}\n[]\n{[] []}";
const MANIFEST: &str = include_str!("../Cargo.toml");

fn placement() -> SourcePlacement {
    SourcePlacement::new(
        vec!["app".to_owned()],
        vec!["app".to_owned(), "bootstrap.ethos".to_owned()],
    )
}

fn authorize(source: &str) -> sema_translator::bootstrap::AuthorizedBootstrap {
    let mut authority = SemaBootstrapAuthority::new().expect("empty authority owns its seed");
    authority
        .authorize(source, placement())
        .expect("authority mints and seals source-local declarations")
}

struct NoTypePaths;

impl RustTypePathResolver for NoTypePaths {
    fn resolve_type_path(&self, _: &name_table::EncodedName) -> Option<&RustTypePath> {
        None
    }
}

fn generate_interface() -> ArtifactSet {
    let assembly = authorize(INTERFACE_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("domain.ethos");
    let rust_path = directory.path().join("domain.rs");
    BootstrapGeneration::new(&assembly, &rust, &paths, &[], &source_path, &rust_path)
        .generate()
        .expect("authority-approved Interface lowers and projects")
}

fn generate_sema() -> ArtifactSet {
    let assembly = authorize(SEMA_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("storage.ethos");
    let rust_path = directory.path().join("storage.rs");
    BootstrapGeneration::new(&assembly, &rust, &paths, &[], &source_path, &rust_path)
        .generate()
        .expect("authority-approved empty Sema lowers and projects")
}

#[test]
fn unified_pipeline_generates_interface_with_canonical_source_and_rust() {
    let generated = generate_interface();
    assert!(
        generated
            .source()
            .content()
            .starts_with("Interface.{1 0 0}\n[]\n")
    );
    assert!(
        generated
            .rust()
            .content()
            .starts_with(BOOTSTRAP_GENERATED_MARKER)
    );
    syn::parse_file(generated.rust().content()).expect("canonical projection is Rust syntax");
}

#[test]
fn unified_pipeline_generates_sema_with_canonical_source_and_rust() {
    let generated = generate_sema();
    assert!(
        generated
            .source()
            .content()
            .starts_with("Sema.{1 0 0}\n[]\n")
    );
    assert!(
        generated
            .rust()
            .content()
            .starts_with(BOOTSTRAP_GENERATED_MARKER)
    );
    syn::parse_file(generated.rust().content()).expect("canonical projection is Rust syntax");
}

#[test]
fn commit_bootstrap_asserts_freshness_after_atomic_install() {
    let assembly = authorize(INTERFACE_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("domain.ethos");
    let rust_path = directory.path().join("domain.rs");
    let generated =
        BootstrapGeneration::new(&assembly, &rust, &paths, &[], &source_path, &rust_path)
            .generate()
            .expect("authority-approved Interface lowers and projects");

    // Manually write both files to simulate a prior commit.
    fs::write(&source_path, generated.source().content()).expect("seat canonical source");
    fs::write(&rust_path, generated.rust().content()).expect("seat canonical Rust");

    let commit = CommitBootstrap::single(generated.clone());
    commit
        .assert_checked_in()
        .expect("both artifacts are fresh after seating");

    // Corrupt one file; the commit must refuse.
    fs::write(&source_path, "stale source").expect("make source stale");
    assert!(matches!(
        commit.assert_checked_in(),
        Err(BuildError::StaleGeneratedArtifact { path }) if path == source_path
    ));
}

#[test]
fn commit_bootstrap_atomic_write_leaves_no_partial_state() {
    let assembly = authorize(INTERFACE_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("domain.ethos");
    let rust_path = directory.path().join("domain.rs");
    let generated =
        BootstrapGeneration::new(&assembly, &rust, &paths, &[], &source_path, &rust_path)
            .generate()
            .expect("authority-approved Interface lowers and projects");

    // Neither file exists yet.
    assert!(!source_path.exists());
    assert!(!rust_path.exists());

    // Atomic commit installs both artifacts.
    let commit = CommitBootstrap::single(generated.clone());
    commit
        .commit()
        .expect("atomic commit installs both artifacts");

    // Both files now exist with canonical content.
    assert_eq!(
        fs::read_to_string(&source_path).expect("read source"),
        generated.source().content()
    );
    assert_eq!(
        fs::read_to_string(&rust_path).expect("read Rust"),
        generated.rust().content()
    );

    // No pending files remain.
    assert!(!source_path.with_file_name("domain.ethos.pending").exists());
    assert!(!rust_path.with_file_name("domain.rs.pending").exists());
}

#[test]
fn generation_refuses_nexus_file_kind() {
    let assembly = authorize(NEXUS_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    assert!(matches!(
        BootstrapGeneration::new(&assembly, &rust, &paths, &[], "nexus.ethos", "nexus.rs")
            .generate(),
        Err(BootstrapGenerationError::UnsupportedFileKind {
            found: EthosKind::Nexus
        })
    ));
}

#[test]
fn empty_sema_needs_no_caller_storage_provenance() {
    let assembly = authorize(SEMA_SOURCE);
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    BootstrapGeneration::new(&assembly, &rust, &paths, &[], "sema.ethos", "sema.rs")
        .generate()
        .expect("an empty source needs no caller-created storage provenance");
}

#[test]
fn strict_bootstrap_lane_pins_the_current_verified_producer_train() {
    for exact_dependency in [
        "core-ethos = { git = \"https://github.com/LiGoldragon/core-ethos.git\", rev = \"aa83187\" }",
        "core-nomos = { git = \"https://github.com/LiGoldragon/core-nomos.git\", rev = \"e57728e\" }",
        "rust-logos = { git = \"https://github.com/LiGoldragon/rust-logos.git\", rev = \"0687449\" }",
        "sema-translator = { git = \"https://github.com/LiGoldragon/sema-translator.git\", rev = \"118d7a3\", default-features = false, features = [\"bootstrap\"] }",
    ] {
        assert!(
            MANIFEST.contains(exact_dependency),
            "strict bootstrap manifest omitted exact producer {exact_dependency}"
        );
    }
}

#[test]
fn strict_bootstrap_normal_graph_excludes_the_sema_engine_runtime() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            "schema-rust",
            "--edges",
            "normal",
            "--no-default-features",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("cargo tree output is UTF-8");
    assert!(
        !tree.contains("sema-engine"),
        "strict bootstrap generation must not close through its runtime consumer:\n{tree}"
    );
}

#[test]
fn interface_with_role_entries_emits_trait_implementations() {
    let source = "Interface.{1 0 0}\n[]\n{[Command.String] [Event.String] [] [Observation.String] []}";
    let mut authority = SemaBootstrapAuthority::new().expect("empty authority");
    let priors = authority.prior_identities().clone();
    let assembly = authority
        .authorize(source, placement())
        .expect("authorize Interface with role entries");
    let role_traits = InterfaceRoleTraitIdentities::new(
        priors.input_role,
        priors.output_role,
        priors.refusal_role,
        priors.stream_role,
    );
    let rust = RustLogos::new();
    let paths = NoTypePaths;
    let directory = tempfile::tempdir().expect("temporary directory");
    let source_path = directory.path().join("observer.ethos");
    let rust_path = directory.path().join("observer.rs");
    let generated = BootstrapGeneration::new(
        &assembly,
        &rust,
        &paths,
        &[],
        &source_path,
        &rust_path,
    )
    .with_role_traits(&role_traits)
    .generate()
    .expect("Interface with role traits lowers and projects");

    let rust_content = generated.rust().content();
    syn::parse_file(rust_content).expect("generated role-trait Rust is valid syntax");
    assert!(
        rust_content.contains("impl Input for Command"),
        "generated Rust must implement Input for Input-section entry:\n{rust_content}"
    );
    assert!(
        rust_content.contains("impl Output for Event"),
        "generated Rust must implement Output for Output-section entry:\n{rust_content}"
    );
    assert!(
        rust_content.contains("impl Stream for Observation"),
        "generated Rust must implement Stream for Stream-section entry:\n{rust_content}"
    );
}
