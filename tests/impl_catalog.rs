//! The `{| … |}` impl-reference catalog, consumed on the schema-rust side.
//!
//! Report 703.3: schema-rust reads `TrueSchema::referenced_impls()`, DRIVES
//! standard-impl emission from it (replacing the `scalar_like()` flag trigger),
//! and verifies the recognized subset against the surface it actually emits.

use schema::{ImplFact, Name, RustSurface};
use schema_rust::{RustEmissionOptions, RustEmitter, RustModule};

mod support;

use support::FixtureSchema;

fn lower(fixture: &str) -> schema::TrueSchema {
    FixtureSchema::new(format!("impl-catalog/{fixture}.schema")).lower("impl-catalog:lib")
}

fn module(fixture: &str) -> RustModule {
    RustModule::from_true_schema(
        &lower(fixture),
        "schema-rust",
        RustEmissionOptions::binary_only(),
    )
}

fn emit(fixture: &str) -> String {
    RustEmitter::new(RustEmissionOptions::binary_only())
        .emit_code_from_true_schema(&lower(fixture))
        .as_str()
        .to_owned()
}

/// The catalog is no longer dropped on the floor: the lowered module carries
/// every `{| … |}` reference, paired with its target. (702 found
/// `grep ImplReference src/` returned 0 — this is the fix.)
#[test]
fn lowered_module_carries_the_referenced_impl_catalog() {
    let module = module("fused-markers");
    let references = module.referenced_impls();
    assert_eq!(references.len(), 2, "Display and Ord are both carried");
    assert!(
        references
            .iter()
            .all(|reference| reference.target().as_str() == "RecordIdentifier"),
        "both entries target RecordIdentifier"
    );
}

/// A recognized standard marker on a scalar newtype emits its generator-owned
/// body — the catalog SELECTS, the `*Tokens` library SPELLS. `Ord` is
/// derive-class, so it folds into the `#[derive(...)]` set rather than an impl.
#[test]
fn recognized_markers_emit_bodies_and_derives() {
    let code = emit("fused-markers");
    assert!(
        code.contains("impl std::fmt::Display for RecordIdentifier"),
        "Display marker drives the payload-delegating body:\n{code}"
    );
    assert!(
        code.contains("PartialOrd, Ord") && code.contains("pub struct RecordIdentifier(String)"),
        "Ord marker folds into the derive set:\n{code}"
    );
}

/// The body-optional block: `Display` (recognized) emits, but `word_count`
/// (an inherent method with no generator recipe) emits NO body — it is
/// recorded for the verify loop, trusted to the crate.
#[test]
fn body_optional_emits_recognized_only() {
    let code = emit("body-optional");
    assert!(
        code.contains("impl std::fmt::Display for StatementText"),
        "the recognized Display reference emits its body:\n{code}"
    );
    assert!(
        !code.contains("fn word_count"),
        "an inherent method with no recipe emits nothing (verify-only):\n{code}"
    );
    // The reference is still carried for verification.
    let module = module("body-optional");
    assert_eq!(module.referenced_impls().len(), 2);
}

/// The transitive-scalar blind spot the catalog closes: `scalar_like()` matched
/// `TypeReference::String` directly, so `Statement(StatementText(String))` was
/// invisible to it. The catalog has no such blind spot — the `{| Display |}`
/// reference is explicit, and the recipe RESOLVES the backing scalar through
/// the newtype chain, so `Display` emits regardless of nesting depth.
#[test]
fn transitive_scalar_emits_display() {
    let code = emit("transitive-scalar");
    assert!(
        code.contains("impl std::fmt::Display for Statement"),
        "Display emits over a transitive String-backed newtype:\n{code}"
    );
}

/// The verify boundary, now on a GENERATED surface: lowering `fused-markers`,
/// building the emitted surface, and verifying the recognized subset returns
/// `Ok`. The facts come from the module's own emission, not a hand-built test
/// vector — this is what makes `verify_catalog` meaningful outside tests.
#[test]
fn emitted_surface_verifies_recognized_subset() {
    let schema = lower("fused-markers");
    let module =
        RustModule::from_true_schema(&schema, "schema-rust", RustEmissionOptions::binary_only());
    module
        .verify_catalog(&schema)
        .expect("the recognized subset verifies against the generated surface");
}

/// The falsifiable half: a recognized reference whose body the generator did
/// NOT emit fails verification with the typed error naming the exact target —
/// mirror of schema/tests/impl_catalog.rs, now on a generated surface.
/// Here a hand-built surface drops the `Display` fact for `RecordIdentifier`,
/// so the recognized `Display` reference is unverified.
#[test]
fn absent_recognized_impl_fails_verification() {
    let schema = lower("fused-markers");
    // A surface that knows the derive-class `Ord` but is MISSING `Display`.
    let surface = RustSurface::new(vec![ImplFact::trait_impl(
        Name::new("RecordIdentifier"),
        Name::new("Ord"),
    )]);
    let error = surface
        .verify_catalog(&schema)
        .expect_err("a missing recognized impl must fail verification");
    let schema::SchemaError::UnverifiedImplReference {
        target, signature, ..
    } = &error
    else {
        panic!("expected UnverifiedImplReference, got: {error}");
    };
    assert_eq!(target, "RecordIdentifier");
    assert!(
        signature.contains("Display"),
        "the error names the unverified Display reference, got: {signature}"
    );
}
