//! Malformed-name boundary (srn-1): NOTA accepts symbol atoms (`Foo-Bar`,
//! `A/B`, `2Things`) far broader than Rust identifiers, so a malformed schema
//! name reached `Ident::new` and PANICKED. The emission boundary now turns that
//! into a typed `SchemaError::MalformedSchemaNode` via `RustModule::verify_names`.

use schema::{SchemaEngine, SchemaError, SchemaIdentity, TrueSchema};
use schema_rust::{RustEmissionOptions, RustModule};

fn lower_source(source: &str) -> TrueSchema {
    SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("name-validation:lib", "0.1.0"))
        .expect("NOTA accepts the symbol atom as a schema name")
}

fn module(source: &str) -> RustModule {
    RustModule::from_true_schema(
        &lower_source(source),
        "schema-rust",
        RustEmissionOptions::binary_only(),
    )
}

/// A hyphenated type name lowers fine through NOTA (it is a legal symbol atom)
/// but is not a legal Rust identifier — `verify_names` rejects it with a typed
/// error naming the offending name, instead of panicking at `Ident::new`.
#[test]
fn hyphenated_type_name_yields_typed_error_not_panic() {
    let module = module("{}\n[]\n[]\n{\n  Foo-Bar String\n}\n");
    let error = module
        .verify_names()
        .expect_err("a hyphenated name is not a legal Rust identifier");
    let SchemaError::MalformedSchemaNode { found } = &error else {
        panic!("expected MalformedSchemaNode, got: {error}");
    };
    assert!(
        found.contains("Foo-Bar") && found.contains("legal Rust identifier"),
        "the error names the offending identifier: {found}"
    );
}

/// A name starting with a digit is likewise a NOTA-legal atom but an illegal
/// Rust identifier.
#[test]
fn leading_digit_type_name_yields_typed_error() {
    let module = module("{}\n[]\n[]\n{\n  2Things String\n}\n");
    let error = module
        .verify_names()
        .expect_err("a leading-digit name is not a legal Rust identifier");
    assert!(matches!(error, SchemaError::MalformedSchemaNode { .. }));
}

/// A well-formed schema passes name validation. The boundary is a gate on
/// malformed names, not a blanket rejection.
#[test]
fn well_formed_names_pass_validation() {
    let module = module("{}\n[]\n[]\n{\n  RecordIdentifier String\n}\n");
    module
        .verify_names()
        .expect("a PascalCase name is a legal Rust identifier");
}

/// The source-lowering emission path (the one the build driver uses) runs the
/// name boundary, so a malformed name surfaces as a typed `Err`, never a panic,
/// all the way through `emit_module_from_schema_source`.
#[test]
fn source_emission_path_returns_err_for_malformed_name() {
    use schema::{ImportResolver, SchemaSource};
    use schema_rust::RustEmitter;

    let engine = SchemaEngine::default();
    let resolver = ImportResolver::default();
    let source = SchemaSource::from_schema_text("{}\n[]\n[]\n{\n  Bad/Name String\n}\n")
        .expect("NOTA parses the symbol atom name");
    let emitter = RustEmitter::new(RustEmissionOptions::binary_only());
    let result = emitter.emit_module_from_schema_source(
        &source,
        SchemaIdentity::new("name-validation:lib", "0.1.0"),
        &engine,
        &resolver,
    );
    let error = result.expect_err("a malformed name fails the source emission path");
    assert!(matches!(error, SchemaError::MalformedSchemaNode { .. }));
}
