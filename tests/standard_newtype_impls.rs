use schema_rust::{RustEmissionOptions, RustEmitter};

mod support;

use support::FixtureSchema;

fn emit_standard_newtype_impls() -> String {
    // No flag: the standard payload-delegating impls are now DRIVEN by the
    // `{| … |}` catalog the fixture carries, not by an emission flag. The
    // catalog rides inside the lowered module, so even the infallible
    // `emit_code_from_true_schema` path emits them.
    let schema = FixtureSchema::new("standard-newtype-impls.schema").lower("standard:impls");
    let options = RustEmissionOptions::binary_only();
    RustEmitter::new(options)
        .emit_code_from_true_schema(&schema)
        .expect("schema emits")
        .as_str()
        .to_owned()
}

#[test]
fn write_standard_newtype_impl_fixture() {
    if std::env::var_os("SCHEMA_RUST_UPDATE_FIXTURES").is_none() {
        return;
    }
    let code = emit_standard_newtype_impls();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/standard_newtype_impls_generated.rs");
    std::fs::write(path, code).expect("write standard newtype impl fixture");
}

#[test]
fn scalar_newtypes_emit_standard_impls() {
    let code = emit_standard_newtype_impls();
    assert!(
        code.contains("impl std::fmt::Display for NameText"),
        "string-backed Display emits:\n{code}"
    );
    assert!(
        code.contains("impl AsRef<str> for NameText"),
        "string-backed AsRef<str> emits:\n{code}"
    );
    assert!(
        code.contains("impl PartialEq<&str> for NameText"),
        "string-backed PartialEq<&str> emits:\n{code}"
    );
    assert!(
        code.contains("impl std::fmt::Display for Count"),
        "integer-backed Display emits:\n{code}"
    );
    assert!(
        code.contains("impl PartialEq<u64> for Count"),
        "integer-backed PartialEq<u64> emits:\n{code}"
    );
    assert!(
        code.contains("impl PartialOrd<u64> for Count"),
        "integer-backed PartialOrd<u64> emits:\n{code}"
    );
    assert!(
        code.contains("impl PartialEq<bool> for Enabled"),
        "boolean-backed PartialEq<bool> emits:\n{code}"
    );
    assert!(
        !code.contains("impl std::fmt::Display for WrappedName"),
        "a newtype with NO catalog entry gets no standard impls:\n{code}"
    );
}

#[allow(dead_code)]
mod generated {
    include!("fixtures/standard_newtype_impls_generated.rs");
}

#[test]
fn generated_standard_impls_compile_and_delegate_to_payloads() {
    let name = generated::NameText::new("schema");
    assert_eq!(name.to_string(), "schema");
    assert_eq!(name.as_ref(), "schema");
    assert_eq!(name, "schema");

    let path = generated::FilePath::new("/tmp/schema.dotos");
    assert_eq!(path.to_string(), "/tmp/schema.dotos");
    assert_eq!(path.as_ref(), "/tmp/schema.dotos");
    assert_eq!(path, "/tmp/schema.dotos");

    let count = generated::Count::new(42);
    assert_eq!(count.to_string(), "42");
    assert_eq!(count, 42);
    assert!(count > 10);

    let enabled = generated::Enabled::new(true);
    assert_eq!(enabled.to_string(), "true");
    assert_eq!(enabled, true);
}
