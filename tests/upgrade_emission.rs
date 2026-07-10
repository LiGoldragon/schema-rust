//! Designer 481 — end-to-end witness for schema-daemon upgrade emission.
//!
//! Pipeline: an `UpgradeObject` describing v1 → v2 is constructed; the
//! `MigrationEmitter` produces Rust source that names `historical::T`,
//! `current::T`, and `impl From<historical::T> for current::T`. The
//! emitted source is inspected to confirm:
//!
//!  - The `WrapSingleton` migration produces `topic: vec![previous.topic]`.
//!  - `AddField` lands a default-filled line on the projection.
//!  - `AddVariant` extends the enum on the current side without making
//!    the historical projection unreachable.
//!  - Identity stamping appears in the header.
//!
//! Per `skills/architectural-truth-tests.md`, this is a Layer 2 witness
//! (runtime execution of the real emit method against real typed input).

use std::{fs, path::PathBuf};

use schema_language::{
    DefaultValue, FieldMigration, Name, SchemaEdit, SchemaIdentity, TypeReference, UpgradeObject,
};
use schema_rust::MigrationEmitter;

fn pilot_upgrade() -> UpgradeObject {
    UpgradeObject::new(
        SchemaIdentity::new("spirit-min", "0.1.0"),
        SchemaIdentity::new("spirit-min", "0.2.0"),
        vec![
            SchemaEdit::add_field(
                "Entry",
                "last_modified",
                TypeReference::Integer,
                DefaultValue::Integer(0),
            ),
            SchemaEdit::change_field_type(
                "Entry",
                "topic",
                TypeReference::vector(TypeReference::Plain(Name::new("Topic"))),
                FieldMigration::WrapSingleton,
            ),
            SchemaEdit::add_variant("Kind", "Reflection", None),
        ],
    )
}

#[test]
fn emitter_renders_header_with_identity_transition() {
    let upgrade = pilot_upgrade();
    let source = MigrationEmitter::new(&upgrade).emit();
    assert!(
        source.contains("spirit-min@0.1.0 -> spirit-min@0.2.0"),
        "identity transition appears in header, source:\n{source}"
    );
    assert!(
        source.contains("#[rustfmt::skip]"),
        "generated migration source should opt out of rustfmt, source:\n{source}"
    );
}

#[test]
fn emitter_produces_historical_and_current_modules() {
    let upgrade = pilot_upgrade();
    let source = MigrationEmitter::new(&upgrade).emit();
    assert!(
        source.contains("pub mod historical {"),
        "historical module emitted"
    );
    assert!(
        source.contains("pub mod current {"),
        "current module emitted"
    );
}

#[test]
fn emitter_lands_wrap_singleton_projection() {
    let upgrade = pilot_upgrade();
    let source = MigrationEmitter::new(&upgrade).emit();
    // The WrapSingleton migration on Entry.topic should appear as
    // `topic: vec![previous.topic],` in the projection.
    assert!(
        source.contains("topic: vec![previous.topic],"),
        "WrapSingleton projection appears, source:\n{source}"
    );
}

#[test]
fn emitter_lands_add_field_default_line() {
    let upgrade = pilot_upgrade();
    let source = MigrationEmitter::new(&upgrade).emit();
    // The AddField with DefaultValue::Integer(0) lands as
    // `last_modified: 0_i64,` in the projection.
    assert!(
        source.contains("last_modified: 0_i64,"),
        "AddField default projection appears, source:\n{source}"
    );
}

#[test]
fn emitter_lands_add_variant_on_current_enum() {
    let upgrade = pilot_upgrade();
    let source = MigrationEmitter::new(&upgrade).emit();
    // The current enum gets the new variant.
    assert!(
        source.contains("pub enum Kind {"),
        "current Kind enum present"
    );
    assert!(
        source.contains("Reflection,"),
        "Reflection variant emitted, source:\n{source}"
    );
}

#[test]
fn emitted_source_compiles_and_migrates_a_value() {
    // Layer-2 witness: emit the module to a temp file, syntax-check it
    // through rustc, then load + use it through `include!` in a sibling
    // file. This proves the emitted source is real Rust the compiler
    // accepts AND the projection method moves a value across versions.
    //
    // Implementation note: the emitted module references `Topic` and
    // other declared types only when the upgrade touches them. The
    // emitter renders `TypeReference::Plain(name)` as the plain name.
    // The pilot test keeps the upgrade to operations whose emission
    // references only built-in types so the harness compiles standalone.
    let upgrade = UpgradeObject::new(
        SchemaIdentity::new("spirit-min", "0.1.0"),
        SchemaIdentity::new("spirit-min", "0.2.0"),
        vec![
            SchemaEdit::add_field(
                "Reading",
                "last_modified",
                TypeReference::Integer,
                DefaultValue::Integer(0),
            ),
            SchemaEdit::change_field_type(
                "Reading",
                "score",
                TypeReference::vector(TypeReference::Integer),
                FieldMigration::WrapSingleton,
            ),
        ],
    );
    let emitted = MigrationEmitter::new(&upgrade).emit();

    // Write the emitted source to a temp file under target/, then run
    // rustc --emit=metadata against it to confirm syntactic + type
    // correctness. The harness file wraps the emitted module in a
    // `pub fn main()` that constructs a historical value and projects it
    // through the From impl.
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let temp_directory = manifest_directory.join("target").join("designer-481-pilot");
    fs::create_dir_all(&temp_directory).expect("temp dir created");
    let emission_path = temp_directory.join("emitted_migration.rs");
    let harness_path = temp_directory.join("emitted_migration_harness.rs");

    fs::write(&emission_path, &emitted).expect("emission written");

    // The harness file: include! the emission, build a historical value,
    // project it via the typed From impl, and assert the projection used
    // the expected default + WrapSingleton wrapping.
    let harness_source = format!(
        "include!({path:?});\n\
         \n\
         pub fn main() {{\n\
             let previous = historical::Reading {{ score: 7_i64 }};\n\
             let next: current::Reading = previous.into();\n\
             assert_eq!(next.last_modified, 0_i64);\n\
             assert_eq!(next.score, vec![7_i64]);\n\
         }}\n",
        path = emission_path
    );
    fs::write(&harness_path, &harness_source).expect("harness written");

    // Invoke rustc to syntax-check + type-check the harness. The
    // command runs the workspace's pinned rust toolchain via cargo's
    // RUSTC env so the witness uses the same compiler as the rest of
    // the suite.
    let rustc_path = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let output_path = temp_directory.join("emitted_migration_harness");
    let status = std::process::Command::new(rustc_path)
        .arg(&harness_path)
        .arg("--edition=2021")
        .arg("--emit=link")
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("rustc spawns");
    assert!(
        status.status.success(),
        "rustc rejected the emitted migration. stdout: {stdout}\nstderr: {stderr}\nemitted source:\n{emitted}",
        stdout = String::from_utf8_lossy(&status.stdout),
        stderr = String::from_utf8_lossy(&status.stderr),
    );

    // Run the harness binary; the assertions inside `main` are the
    // value-level witness that the migration runs correctly.
    let run = std::process::Command::new(&output_path)
        .output()
        .expect("compiled harness runs");
    assert!(
        run.status.success(),
        "compiled migration harness failed. stdout: {stdout}\nstderr: {stderr}",
        stdout = String::from_utf8_lossy(&run.stdout),
        stderr = String::from_utf8_lossy(&run.stderr),
    );
}
