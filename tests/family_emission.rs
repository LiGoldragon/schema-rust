use schema_language::{SchemaEngine, SchemaIdentity};
use schema_rust::{RustEmissionOptions, RustEmitter, RustModule};

mod support;

use support::FixtureSchema;

/// Compile witness: the checked-in generated module — including the
/// record-family surface — compiles against the real `sema-engine`,
/// `signal-frame`, and `triad-runtime` crates, and the runtime tests
/// below drive the generated decode path through real descriptor and
/// identity values.
#[allow(dead_code)]
#[path = "fixtures/families_generated.rs"]
mod families_generated;

fn generated_fixture_path(file_name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file_name)
}

fn assert_generated_fixture(file_name: &str, generated: &str) {
    let path = generated_fixture_path(file_name);
    if std::env::var_os("SCHEMA_RUST_UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, generated).expect("write generated fixture");
    }
    let expected = std::fs::read_to_string(path).expect("read generated fixture");
    assert_eq!(generated, expected);
}

fn assert_code_contains(code: &str, expected: &str) {
    let compact = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace() && *character != ',')
            .collect::<String>()
    };
    assert!(
        compact(code).contains(&compact(expected)),
        "generated code must contain {expected:?}"
    );
}

fn family_schema() -> schema_language::TrueSchema {
    FixtureSchema::new("record-families.schema").lower("example:lib")
}

#[test]
fn family_declarations_emit_the_version_control_surface() {
    let schema = family_schema();
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);

    assert_code_contains(generated.as_str(), "pub mod family_identity");
    assert_code_contains(generated.as_str(), "pub const ENTRY_FAMILY: [u8; 32]");
    assert_code_contains(generated.as_str(), "pub const OBSERVATION_FAMILY: [u8; 32]");
    assert_code_contains(generated.as_str(), "pub enum RecordFamilyError");
    assert_code_contains(
        generated.as_str(),
        "pub enum RecordFamily { EntryFamily(Entry) ObservationFamily(Observation) }",
    );
    assert_code_contains(
        generated.as_str(),
        "pub const STORE_NAME: &'static str = \"example:lib\";",
    );
    assert_code_contains(
        generated.as_str(),
        "pub fn entry_family() -> sema_engine::TableDescriptor<Entry>",
    );
    assert_code_contains(
        generated.as_str(),
        "pub fn observation_family() -> sema_engine::IdentifiedTableDescriptor<Observation>",
    );
    assert_code_contains(
        generated.as_str(),
        "pub fn decode(identity: &sema_engine::FamilyIdentity, bytes: &[u8]) -> Result<Self, RecordFamilyError>",
    );

    assert_generated_fixture("families_generated.rs", generated.as_str());
}

#[test]
fn family_identity_emission_uses_true_schema_closure() {
    let schema = family_schema();
    let schema_module =
        RustModule::from_true_schema(&schema, "schema-rust", RustEmissionOptions::default());

    for family in schema_module.versioned_store().families() {
        let expected_hash = *schema
            .family_closure(family.record().as_str())
            .expect("family closure builds from TrueSchema")
            .content_hash()
            .expect("family closure hashes")
            .as_bytes();
        assert_eq!(family.schema_hash(), &expected_hash);
    }
}

#[test]
fn schema_without_families_emits_no_family_surface() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);

    assert!(!generated.as_str().contains("RecordFamily"));
    assert!(!generated.as_str().contains("family_identity"));
    assert!(!generated.as_str().contains("sema_engine"));
}

#[test]
fn schema_field_change_moves_the_emitted_family_constant() {
    let original = family_schema();
    let edited_source = FixtureSchema::new("record-families.schema").read().replace(
        "RecordIdentifier Integer\n  Entry { Topic Description }",
        "RecordIdentifier Integer\n  Revision Integer\n  Entry { Topic Description Revision }",
    );
    let edited = SchemaEngine::default()
        .lower_source(&edited_source, SchemaIdentity::new("example:lib", "0.1.0"))
        .expect("edited schema lowers");

    let original_module =
        RustModule::from_true_schema(&original, "schema-rust", RustEmissionOptions::default());
    let edited_module =
        RustModule::from_true_schema(&edited, "schema-rust", RustEmissionOptions::default());

    let family = |module: &RustModule, name: &str| {
        module
            .versioned_store()
            .families()
            .iter()
            .find(|family| family.name().as_str() == name)
            .expect("family is lowered")
            .schema_hash()
            .to_owned()
    };

    assert_ne!(
        family(&original_module, "EntryFamily"),
        family(&edited_module, "EntryFamily"),
        "an Entry field change moves the EntryFamily identity"
    );
    assert_eq!(
        family(&original_module, "ObservationFamily"),
        family(&edited_module, "ObservationFamily"),
        "an Entry field change does not move the ObservationFamily identity"
    );

    let constant_text = |hash: [u8; 32]| {
        let bytes = hash
            .iter()
            .map(|byte| byte.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        format!("pub const ENTRY_FAMILY: [u8; 32] = [{bytes}];")
    };
    assert_code_contains(
        RustEmitter::default()
            .emit_code_from_true_schema(&original)
            .as_str(),
        &constant_text(family(&original_module, "EntryFamily")),
    );
    assert_code_contains(
        RustEmitter::default()
            .emit_code_from_true_schema(&edited)
            .as_str(),
        &constant_text(family(&edited_module, "EntryFamily")),
    );
}

#[test]
fn generated_descriptors_carry_the_pinned_family_identity() {
    let entry_descriptor = families_generated::RecordFamily::entry_family();
    assert_eq!(entry_descriptor.name().as_str(), "entries");
    assert_eq!(entry_descriptor.family().as_str(), "EntryFamily");
    assert_eq!(
        entry_descriptor.schema_hash(),
        sema_engine::SchemaHash::new(families_generated::family_identity::ENTRY_FAMILY)
    );

    let observation_descriptor = families_generated::RecordFamily::observation_family();
    assert_eq!(observation_descriptor.name().as_str(), "observations");
    assert_eq!(
        observation_descriptor.family().as_str(),
        "ObservationFamily"
    );
    assert_eq!(
        observation_descriptor.schema_hash(),
        sema_engine::SchemaHash::new(families_generated::family_identity::OBSERVATION_FAMILY)
    );
}

#[test]
fn generated_decode_round_trips_both_families() {
    let entry = families_generated::Entry {
        topic: families_generated::Topic::new("storage".to_owned()),
        description: families_generated::Description::new("a stored entry".to_owned()),
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entry).expect("entry archives");
    let decoded = families_generated::RecordFamily::decode(
        &families_generated::RecordFamily::entry_family().family_identity(),
        &bytes,
    )
    .expect("entry family decodes");
    assert_eq!(
        decoded,
        families_generated::RecordFamily::EntryFamily(entry)
    );

    let observation = families_generated::Observation::new(families_generated::Description::new(
        "an observed note".to_owned(),
    ));
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&observation).expect("observation archives");
    let decoded = families_generated::RecordFamily::decode(
        &families_generated::RecordFamily::observation_family().family_identity(),
        &bytes,
    )
    .expect("observation family decodes");
    assert_eq!(
        decoded,
        families_generated::RecordFamily::ObservationFamily(observation)
    );
}

#[test]
fn generated_decode_rejects_an_unknown_family() {
    let identity = sema_engine::FamilyIdentity::new(
        sema_engine::FamilyName::new("GhostFamily"),
        sema_engine::SchemaHash::for_label("ghost"),
        sema_engine::TableName::new("ghosts"),
    );

    let error = families_generated::RecordFamily::decode(&identity, &[])
        .expect_err("an unknown family is a typed hard error");
    assert_eq!(
        error,
        families_generated::RecordFamilyError::UnknownFamily {
            family: sema_engine::FamilyName::new("GhostFamily"),
        }
    );
}

#[test]
fn generated_decode_rejects_schema_hash_drift() {
    let stale = sema_engine::SchemaHash::for_label("a previous schema version");
    let identity = sema_engine::FamilyIdentity::new(
        sema_engine::FamilyName::new("EntryFamily"),
        stale,
        sema_engine::TableName::new("entries"),
    );

    let error = families_generated::RecordFamily::decode(&identity, &[])
        .expect_err("schema hash drift is a typed hard error");
    assert_eq!(
        error,
        families_generated::RecordFamilyError::SchemaHashMismatch {
            family: sema_engine::FamilyName::new("EntryFamily"),
            stored: stale,
            generated: sema_engine::SchemaHash::new(
                families_generated::family_identity::ENTRY_FAMILY
            ),
        }
    );
}

#[test]
fn generated_versioning_policy_names_the_component_store() {
    assert_eq!(families_generated::RecordFamily::STORE_NAME, "example:lib");
    assert_eq!(
        families_generated::RecordFamily::versioning_policy()
            .store_name()
            .as_str(),
        "example:lib"
    );
}
