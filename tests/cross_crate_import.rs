use schema::{ImportResolver, MacroContext, SchemaEngine, SchemaIdentity};
use schema_rust::RustEmitter;

mod support;

use support::FixtureSchemaDirectory;

fn emit_consumer() -> String {
    let marker_core = FixtureSchemaDirectory::new("marker-core");
    let import_consumer = FixtureSchemaDirectory::new("import-consumer");
    let resolver =
        ImportResolver::new().with_dependency("marker-core", marker_core.path(), "0.1.0");
    let engine = SchemaEngine::default();
    let source = import_consumer.schema("lib.schema").read();
    let schema = engine
        .lower_source_with_resolver(
            &source,
            SchemaIdentity::new("import-consumer:lib", "0.1.0"),
            &mut MacroContext::default(),
            &resolver,
        )
        .expect("consumer schema resolves its imports");
    RustEmitter::default()
        .emit_code_from_true_schema(&schema)
        .as_str()
        .to_owned()
}

#[test]
fn imported_type_is_referenced_through_a_use_not_redeclared() {
    let code = emit_consumer();

    // The consumer references the dependency crate's type under the
    // local alias — a `pub use`, not a fresh declaration.
    assert!(
        code.contains("pub use marker_core::schema::mail::DatabaseMarker as DatabaseMarker;"),
        "expected a cross-crate use alias, emitted:\n{code}"
    );

    // The consumer must NOT re-declare the imported type: no local
    // struct/enum definition for DatabaseMarker, and no re-emitted
    // rkyv/NOTA impl block for it.
    assert!(
        !code.contains("pub struct DatabaseMarker"),
        "imported type must not be re-declared as a struct, emitted:\n{code}"
    );
    assert!(
        !code.contains("impl DatabaseMarker {"),
        "imported type's impls belong to the dependency crate, emitted:\n{code}"
    );
}

#[test]
fn imported_type_is_used_by_a_local_variant() {
    let code = emit_consumer();

    // The local Output enum carries a variant whose payload is the
    // imported type — and that payload resolves through the alias, so
    // the consumer's Output uses the dependency crate's type identity.
    assert!(
        code.contains("Marked(DatabaseMarker)"),
        "local Output variant should carry the imported type, emitted:\n{code}"
    );
}
