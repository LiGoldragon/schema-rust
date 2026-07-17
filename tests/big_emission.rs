use std::path::{Path, PathBuf};

use schema_language::{
    Declaration, EnumDeclaration, ImportResolver, MacroContext, SchemaEngine, SchemaIdentity,
    SchemaSourceArtifact, TrueSchema, TypeDeclaration,
};
use schema_rust::RustEmitter;

mod support;

#[cfg(feature = "nota-text")]
use support::FixtureNota;

#[allow(dead_code)]
#[path = "fixtures/big-schemas/spirit-reactive-large.generated.rs"]
mod spirit_large_generated;

struct BigRustFixture<'fixture> {
    name: &'fixture str,
    identity: &'fixture str,
    source_path: PathBuf,
    rust_path: PathBuf,
    resolver: Option<ImportResolver>,
}

impl<'fixture> BigRustFixture<'fixture> {
    fn local(name: &'fixture str, identity: &'fixture str) -> Self {
        Self {
            name,
            identity,
            source_path: fixture_path(name, "schema"),
            rust_path: fixture_path(name, "generated.rs"),
            resolver: None,
        }
    }

    fn imported(name: &'fixture str, identity: &'fixture str) -> Self {
        let schema_dir = manifest_dir()
            .join("tests")
            .join("fixtures")
            .join("marker-core")
            .join("schema");
        Self {
            name,
            identity,
            source_path: fixture_path(name, "schema"),
            rust_path: fixture_path(name, "generated.rs"),
            resolver: Some(ImportResolver::new().with_dependency(
                "marker-core",
                schema_dir,
                "0.1.0",
            )),
        }
    }

    fn lower(&self) -> (TrueSchema, MacroContext) {
        let source = std::fs::read_to_string(&self.source_path).expect("read schema fixture");
        let engine = SchemaEngine::default();
        let mut context = MacroContext::default();
        let schema = match &self.resolver {
            Some(resolver) => engine
                .lower_source_with_resolver(
                    &source,
                    SchemaIdentity::new(self.identity, "0.1.0"),
                    &mut context,
                    resolver,
                )
                .expect("schema with imports lowers"),
            None => engine
                .lower_source_with_context(
                    &source,
                    SchemaIdentity::new(self.identity, "0.1.0"),
                    &mut context,
                )
                .expect("schema lowers"),
        };
        (schema, context)
    }

    fn generate_rust(&self) -> String {
        let (schema, _) = self.lower();
        RustEmitter::default()
            .emit_code_from_true_schema(&schema)
            .as_str()
            .to_owned()
    }

    fn generate_rust_after_schema_source_artifact_round_trip(&self) -> String {
        let source = std::fs::read_to_string(&self.source_path).expect("read schema fixture");
        let artifact = SchemaSourceArtifact::from_schema_text(&source)
            .expect("schema source decodes into typed artifact");
        let source_text = artifact.to_schema_text();
        let from_text = SchemaSourceArtifact::from_schema_text(&source_text)
            .expect("canonical schema source text decodes");
        let source_binary = from_text
            .to_binary_bytes()
            .expect("schema source artifact serializes through rkyv");
        let from_binary = SchemaSourceArtifact::from_binary_bytes(&source_binary)
            .expect("schema source archive decodes");
        assert_eq!(
            from_text, from_binary,
            "text and rkyv schema source artifacts must recover the same typed source for {}",
            self.name
        );

        RustEmitter::default()
            .emit_file_from_schema_source(
                from_binary.source(),
                SchemaIdentity::new(self.identity, "0.1.0"),
                &SchemaEngine::default(),
                &self.resolver.clone().unwrap_or_default(),
            )
            .expect("emit Rust from schema source artifact")
            .code
            .as_str()
            .to_owned()
    }

    fn assert_lowers_to_typed_schema_data(&self) {
        let (schema, _) = self.lower();
        self.assert_schema_data_shape(&schema);
    }

    fn assert_schema_data_shape(&self, schema: &TrueSchema) {
        assert_eq!(schema.identity().component().as_str(), self.identity);
        assert_eq!(schema.identity().version(), "0.1.0");
        assert!(
            !schema.namespace().is_empty(),
            "{} must lower into typed namespace data",
            self.name
        );
        assert!(
            !Self::root_enum(&schema.input()).variants.is_empty(),
            "{} must lower typed input variants",
            self.name
        );
        assert!(
            !Self::root_enum(&schema.output()).variants.is_empty(),
            "{} must lower typed output variants",
            self.name
        );
        assert!(
            schema.root_named("Input").is_some(),
            "{} must expose Input as a direct root enum",
            self.name
        );
        assert!(
            schema.root_named("Output").is_some(),
            "{} must expose Output as a direct root enum",
            self.name
        );

        match self.name {
            "spirit-reactive-large" => {
                Self::assert_has_type(&schema.namespace(), "Entry");
                Self::assert_has_type(&schema.namespace(), "RecordSet");
                Self::assert_has_variant(Self::root_enum(&schema.input()), "Record");
                Self::assert_has_variant(Self::root_enum(&schema.output()), "Recorded");
            }
            "imported-mail-consumer" => {
                assert!(!schema.imports().is_empty());
                assert!(!schema.resolved_imports().is_empty());
                Self::assert_has_variant(Self::root_enum(&schema.output()), "Marked");
            }
            _ => panic!("unhandled big fixture {}", self.name),
        }
    }

    fn assert_has_type(declarations: &[Declaration], name: &str) {
        let found = declarations
            .iter()
            .any(|declaration| match declaration.value() {
                TypeDeclaration::Struct(declaration) => declaration.name.as_str() == name,
                TypeDeclaration::Newtype(declaration) => declaration.name.as_str() == name,
                TypeDeclaration::Enum(declaration) => declaration.name.as_str() == name,
            });
        assert!(found, "missing namespace type {name}");
    }

    fn root_enum(root: &schema_language::Root) -> &EnumDeclaration {
        root.as_enum().expect("root lowers to an enum body")
    }

    fn assert_has_variant(declaration: &EnumDeclaration, name: &str) {
        assert!(
            declaration
                .variants
                .iter()
                .any(|variant| variant.name.as_str() == name),
            "missing variant {name} on {}",
            declaration.name.as_str()
        );
    }

    fn assert_matches_checked_in_rust(&self) {
        let generated = self.generate_rust();
        if std::env::var_os("SCHEMA_RUST_UPDATE_BIG_EXAMPLES").is_some() {
            std::fs::write(&self.rust_path, &generated).expect("write generated Rust fixture");
        }
        let expected = std::fs::read_to_string(&self.rust_path).expect("read generated Rust");
        assert_eq!(
            generated, expected,
            "generated Rust drifted for {}",
            self.name
        );
    }

    fn assert_emission_uses_schema_source_artifact(&self) {
        assert_eq!(
            self.generate_rust_after_schema_source_artifact_round_trip(),
            self.generate_rust(),
            "emission for {} must be driven by readable schema source artifacts",
            self.name
        );
    }
}

#[test]
fn large_spirit_schema_lowers_to_typed_schema_data() {
    BigRustFixture::local("spirit-reactive-large", "example:spirit-reactive-large")
        .assert_lowers_to_typed_schema_data();
}

#[test]
fn large_spirit_schema_emits_checked_rust_snapshot() {
    BigRustFixture::local("spirit-reactive-large", "example:spirit-reactive-large")
        .assert_matches_checked_in_rust();
}

#[test]
fn large_imported_schema_lowers_to_typed_schema_data() {
    BigRustFixture::imported("imported-mail-consumer", "example:imported-mail-consumer")
        .assert_lowers_to_typed_schema_data();
}

#[test]
fn large_imported_schema_emits_checked_cross_crate_rust_snapshot() {
    BigRustFixture::imported("imported-mail-consumer", "example:imported-mail-consumer")
        .assert_matches_checked_in_rust();
}

#[test]
fn rust_emission_is_stable_after_schema_source_artifact_round_trip() {
    BigRustFixture::local("spirit-reactive-large", "example:spirit-reactive-large")
        .assert_emission_uses_schema_source_artifact();
    BigRustFixture::imported("imported-mail-consumer", "example:imported-mail-consumer")
        .assert_emission_uses_schema_source_artifact();
}

#[test]
fn generated_big_rust_contains_the_current_schema_stack_surfaces() {
    let spirit = BigRustFixture::local("spirit-reactive-large", "example:spirit-reactive-large")
        .generate_rust();
    let imported =
        BigRustFixture::imported("imported-mail-consumer", "example:imported-mail-consumer")
            .generate_rust();

    assert!(spirit.contains("pub enum Input"));
    assert!(spirit.contains("pub enum Output"));
    assert!(spirit.contains("pub fn encode_signal_frame"));
    assert!(spirit.contains("pub struct OriginRoute"));
    assert!(!spirit.contains("pub struct NexusMail<Payload>"));
    assert!(!spirit.contains("pub trait InputNexus"));
    assert!(!spirit.contains("dispatch_mail_with_nexus"));
    assert!(spirit.contains("pub topics: Topics,"));
    // The body-optional/inline-complex struct fields now route through named
    // newtypes (Entries / ByTopic) — the new schema-source grammar expresses a
    // custom-named complex field by declaring it as a newtype, not inline.
    assert!(spirit.contains("pub entries: Entries,"));
    assert!(spirit.contains("pub by_topic: ByTopic,"));
    assert!(!spirit.contains("pub type Frame = signal_frame::StreamingFrame"));

    assert!(
        imported.contains("pub use marker_core::schema::mail::DatabaseMarker as DatabaseMarker;")
    );
    assert!(
        imported.contains("pub use marker_core::schema::mail::CommitSequence as CommitSequence;")
    );
    assert!(imported.contains("pub database_marker: DatabaseMarker,"));
    assert!(imported.contains("Marked(DatabaseMarker)"));
    assert!(
        !imported.contains("pub struct DatabaseMarker"),
        "imported data types must be used by alias, not re-emitted"
    );
}

#[cfg(feature = "nota-text")]
#[test]
fn compiled_large_spirit_generated_rust_parses_frames_and_emits_mail_events() {
    let input = FixtureNota::new("nota/large-record-schema-rust.nota")
        .read()
        .parse::<spirit_large_generated::Input>()
        .expect("large spirit input parses");
    let frame = input.encode_signal_frame().expect("signal frame encodes");
    let (route, decoded) =
        spirit_large_generated::Input::decode_signal_frame(&frame).expect("signal frame decodes");
    let event = decoded
        .with_origin_route(spirit_large_generated::OriginRoute::new(7001))
        .message_sent(spirit_large_generated::MessageIdentifier::new(99));

    assert_eq!(route, spirit_large_generated::InputRoute::Record);
    assert_eq!(
        event.identifier,
        spirit_large_generated::MessageIdentifier::new(99)
    );
    assert_eq!(
        event.origin_route,
        spirit_large_generated::OriginRoute::new(7001)
    );
    assert_eq!(event.root, spirit_large_generated::MessageRoot::Input);
}

fn fixture_path(name: &str, extension: &str) -> PathBuf {
    manifest_dir()
        .join("tests")
        .join("fixtures")
        .join("big-schemas")
        .join(format!("{name}.{extension}"))
}

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
