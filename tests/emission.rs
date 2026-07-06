use schema::{SchemaEngine, SchemaError, SchemaIdentity, SchemaSource, SchemaSourceArtifact};
use schema_rust::{
    LowerToRust, NotaSurface, RustEmissionOptions, RustEmissionTarget, RustEmitter,
    RustLoweringContext, RustTrueSchemaLowering, RustTypeDeclaration,
};
use std::path::PathBuf;

mod support;

use support::{FixtureNota, FixtureSchema};

fn generated_fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
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

fn enum_header<'source>(source: &'source str, name: &str) -> &'source str {
    let marker = format!("pub enum {name}");
    let enum_start = source.find(&marker).expect("enum exists");
    let header_start = source[..enum_start]
        .rfind("#[rustfmt::skip]")
        .expect("generated enum has rustfmt marker");
    &source[header_start..enum_start]
}

fn assert_code_contains(code: &str, expected: &str) {
    let compact_code = code
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    let compact_expected = expected
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    assert!(
        compact_code.contains(&compact_expected),
        "generated code must contain {expected:?}"
    );
}

fn assert_code_excludes(code: &str, unexpected: &str) {
    let compact_code = code
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    let compact_unexpected = unexpected
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    assert!(
        !compact_code.contains(&compact_unexpected),
        "generated code must exclude {unexpected:?}"
    );
}

/// Assert a `pattern => NexusAction::from(result)` projection arm is emitted,
/// tolerating prettyplease's optional `=> { ... }` block-wrapping of a long
/// arm body. The pattern and the `NexusAction::from(result)` mapping must
/// both appear, compacted, in declaration order.
fn assert_nexus_action_arm(code: &str, pattern: &str, result: &str) {
    let compact = |text: &str| {
        text.chars()
            .filter(|character| {
                !character.is_whitespace()
                    && *character != ','
                    && *character != '{'
                    && *character != '}'
            })
            .collect::<String>()
    };
    let needle = format!(
        "{}=>NexusAction::from({})",
        compact(pattern),
        compact(result)
    );
    assert!(
        compact(code).contains(&needle),
        "generated code must contain nexus action arm {pattern} => NexusAction::from({result})"
    );
}

#[allow(dead_code)]
#[path = "fixtures/spirit_generated.rs"]
mod generated;

#[allow(dead_code)]
#[path = "fixtures/collections_generated.rs"]
mod collections_generated;

#[allow(dead_code)]
#[path = "fixtures/runner_generated.rs"]
mod runner_generated;

#[test]
fn emits_rust_source_as_a_separate_artifact() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);

    assert_eq!(generated.path, "src/schema/lib.rs");
    assert!(generated.code.as_str().contains("pub enum Input"));
    assert!(
        generated
            .code
            .as_str()
            .contains("impl std::str::FromStr for Input")
    );
    assert!(generated.code.as_str().contains("rkyv::Archive"));
    assert!(generated.code.as_str().contains("pub enum Kind"));
    assert!(enum_header(generated.code.as_str(), "Kind").contains("Copy,"));
    assert!(!enum_header(generated.code.as_str(), "Input").contains("Copy,"));
    assert!(generated.code.as_str().contains("pub mod short_header"));
    assert!(generated.code.as_str().contains("pub enum InputRoute"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn encode_signal_frame")
    );
    assert!(!generated.code.as_str().contains("pub trait InputNexus"));
    assert!(!generated.code.as_str().contains("pub trait OutputNexus"));
    assert!(!generated.code.as_str().contains("dispatch_mail_with_nexus"));
    assert!(generated.code.as_str().contains("pub struct MessageSent"));
    assert!(generated.code.as_str().contains("pub struct OriginRoute"));
    assert!(generated.code.as_str().contains("pub mod signal"));
    assert!(generated.code.as_str().contains("pub enum Plane"));
    assert!(
        generated
            .code
            .as_str()
            .contains("Signal(super::Signal<SignalRoot>)")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub type Input = super::Input;")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub origin_route: OriginRoute")
    );
    assert!(generated.code.as_str().contains("impl OriginRoute"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub trait UpgradeFrom<Previous>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn record(payload: Entry) -> Self")
    );
    assert_generated_fixture("spirit_generated.rs", generated.code.as_str());
}

#[test]
fn emits_domain_scope_equivalence_expansion_from_relations() {
    let source = FixtureSchema::new("domain-relations.schema").read();
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:domain", "0.1.0"),
        )
        .expect("schema source lowers");
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);

    assert_code_contains(generated.as_str(), "pub enum DomainScope");
    assert_code_contains(generated.as_str(), "pub enum TechnologyScope");
    assert_code_contains(generated.as_str(), "pub enum HardwareScope");
    assert_code_contains(generated.as_str(), "pub enum SoftwareScope");
    assert_code_contains(generated.as_str(), "impl DomainScope");
    assert_code_contains(generated.as_str(), "impl From<Domain> for DomainScope");
    assert_code_contains(generated.as_str(), "All");
    assert_code_contains(
        generated.as_str(),
        "impl From<Technology> for TechnologyScope",
    );
    assert_code_contains(
        generated.as_str(),
        "pub fn contains_scope(&self, scope: &Self) -> bool",
    );
    assert_code_contains(
        generated.as_str(),
        "pub fn contains_domain(&self, domain: &Domain) -> bool",
    );
    assert_code_contains(generated.as_str(), "nota::NotaDecode");
    assert_code_contains(generated.as_str(), "nota::NotaEncode");
    assert_code_excludes(generated.as_str(), "fn to_nota(&self) -> String");
    assert_code_excludes(generated.as_str(), "from_nota_block");
    assert_code_excludes(generated.as_str(), "fn nota_path_from_block");
    assert_code_excludes(generated.as_str(), "impl NotaEncode for DomainScope");
    assert_code_excludes(generated.as_str(), "pub fn from_path");
    assert_code_excludes(generated.as_str(), "pub fn try_from_path");
    assert_code_excludes(generated.as_str(), "pub fn path_segments");
    assert_code_contains(generated.as_str(), "pub fn expand(&self) -> ScopeSet");
    assert_code_contains(
        generated.as_str(),
        "if relation.iter().any(|scope| scope == self)",
    );
    assert_code_contains(
        generated.as_str(),
        "DomainScope::Technology(TechnologyScope::Hardware(HardwareScope::Networking))",
    );
    assert_code_contains(
        generated.as_str(),
        "DomainScope::Technology(TechnologyScope::Software(SoftwareScope::Distributed(DistributedScope::Networking)))",
    );
}

#[test]
fn emits_terminal_value_domains_as_scope_all() {
    let source = FixtureSchema::new("domain-terminal-scope.schema").read();
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:domain", "0.1.0"),
        )
        .expect("schema source lowers");
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);

    // Strict positional NOTA: the variant payload is required, so it carries
    // the leaf directly with no `Option` wrapper and no bespoke codec.
    assert_code_contains(generated.as_str(), "Programming(ProgrammingLeaf),");
    assert_code_excludes(generated.as_str(), "Programming(Option<ProgrammingLeaf>)");
    // The value leaf enum names its whole-category case with an explicit `All`
    // member; the scope enum keeps its own catch-all `All` without duplication.
    assert_code_contains(generated.as_str(), "pub enum ProgrammingLeaf");
    assert_code_contains(generated.as_str(), "pub enum ProgrammingLeafScope");
    assert_code_contains(generated.as_str(), "All,");
    // Scope lowering is uniform: every payload variant maps `payload.into()`,
    // with no optional-specific `match payload { Some / None }` branch.
    assert_code_contains(
        generated.as_str(),
        "Domain::Technology(payload) => Self::Technology(payload.into())",
    );
    assert_code_contains(
        generated.as_str(),
        "Software::Programming(payload) => Self::Programming(payload.into())",
    );
    // The leaf's `All` member collapses into the scope catch-all.
    assert_code_contains(generated.as_str(), "ProgrammingLeaf::All => Self::All");
    assert_code_excludes(
        generated.as_str(),
        "None => Self::Programming(ProgrammingLeafScope::All)",
    );
}

#[test]
fn schema_object_lowers_itself_into_rust_through_emitter_policy() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let emitter = RustEmitter::default();

    let generated = schema.lower_to_rust_file(&emitter);
    let module = schema.lower_to_rust_module(&emitter);

    assert_eq!(generated, emitter.emit_file_from_true_schema(&schema));
    assert_eq!(module, emitter.emit_module_from_true_schema(&schema));
    assert_eq!(generated.path, "src/schema/lib.rs");
    assert!(generated.code.as_str().contains("pub enum Input"));
}

#[test]
fn rust_lowering_uses_true_schema_as_the_canonical_entrypoint() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let emitter = RustEmitter::default();

    assert_eq!(
        emitter.emit_code_from_true_schema(&schema),
        schema.lower_to_rust_code(&emitter)
    );
    assert_eq!(
        emitter.emit_module_from_true_schema(&schema),
        schema.lower_to_rust_module(&emitter)
    );
    assert_eq!(
        emitter.emit_file_from_true_schema(&schema),
        schema.lower_to_rust_file(&emitter)
    );
}

#[test]
fn true_schema_product_component_identity_controls_rust_fields() {
    let schema = SchemaEngine::default()
        .lower_source(
            "{}\n[]\n[]\n{\n  Time Integer\n  TimeRange { start.Time end.Time }\n}\n",
            SchemaIdentity::new("example:range", "0.1.0"),
        )
        .expect("repeated component disambiguators lower into TrueSchema");

    assert!(
        schema
            .to_schema_text()
            .contains("TimeRange { start.Time end.Time }"),
        "valid repeated-type disambiguators stay in canonical TrueSchema text"
    );
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);
    assert!(generated.as_str().contains("pub start: Time,"));
    assert!(generated.as_str().contains("pub end: Time,"));
}

#[test]
fn explicit_unique_product_component_identity_is_rejected_before_rust_emission() {
    let error = SchemaSource::from_schema_text(
        "{}\n[]\n[]\n{\n  Topic String\n  Entry { label.Topic }\n}\n",
    )
    .expect_err("explicit identity on a unique product component is invalid");

    let SchemaError::ExplicitFieldOnUniqueProductComponent { field, type_name } = error else {
        panic!("expected product component identity error, got {error}");
    };
    assert_eq!(field, "label");
    assert_eq!(type_name, "Topic");
}

#[test]
fn schema_subobjects_lower_themselves_into_rust_model_nouns() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let context = RustLoweringContext::from_emitter(&RustEmitter::default());

    let entry = schema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == "Entry")
        .expect("schema entry declaration");
    let rust_entry = entry.lower_to_rust(&context);
    let RustTypeDeclaration::Struct(entry_struct) = rust_entry.value() else {
        panic!("Entry lowers to a Rust struct noun");
    };
    assert_eq!(entry_struct.name().as_str(), "Entry");
    assert_eq!(entry_struct.fields()[0].name().as_str(), "topics");

    let input = schema
        .input_and_output()
        .into_iter()
        .find(|root| root.name().as_str() == "Input")
        .and_then(schema::Root::as_enum)
        .expect("schema input root enum");
    let rust_input = input.lower_to_rust(&context);
    assert_eq!(rust_input.name().as_str(), "Input");
    assert_eq!(rust_input.variants()[0].name().as_str(), "Record");
}

#[test]
fn emitter_builds_rust_module_data_before_rendering_text() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let emitter = RustEmitter::default();
    let module = emitter.emit_module_from_true_schema(&schema);

    assert_eq!(module.file_path(), "src/schema/lib.rs");
    assert_eq!(module.root_enums().len(), 2);
    assert_eq!(module.root_enums()[0].name().as_str(), "Input");
    assert_eq!(module.root_enums()[1].name().as_str(), "Output");
    assert!(
        module
            .scalar_aliases()
            .iter()
            .any(|alias| alias.name() == "Integer" && alias.rust_type() == "u64"),
        "scalar floor should be data on RustModule"
    );

    let topic = module
        .declaration_named("Topic")
        .expect("Topic declaration exists");
    assert!(matches!(topic.value(), RustTypeDeclaration::Newtype(_)));

    let summary = module
        .declaration_named("Summary")
        .expect("Summary declaration exists");
    assert!(matches!(summary.value(), RustTypeDeclaration::Newtype(_)));

    let entry = module
        .declaration_named("Entry")
        .expect("Entry declaration exists");
    let RustTypeDeclaration::Struct(entry_struct) = entry.value() else {
        panic!("Entry should model as a Rust struct declaration");
    };
    assert_eq!(entry_struct.fields()[0].name().as_str(), "topics");
    assert_eq!(entry_struct.fields()[1].name().as_str(), "kind");

    assert_eq!(module.render(), emitter.emit_code_from_true_schema(&schema));
}

#[test]
fn generated_objects_expose_named_constructors_and_newtype_payload_accessors() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let code = RustEmitter::default().emit_code_from_true_schema(&schema);
    let source = code.as_str();

    assert!(source.contains("pub struct Topic(String);"));
    assert!(source.contains("pub struct Topics(Vec<Topic>);"));
    assert!(source.contains("pub struct Description(String);"));
    assert!(source.contains("pub struct Summary(Description);"));
    assert!(source.contains("impl Summary {"));
    assert!(source.contains("pub fn new(payload: Description) -> Self"));
    assert!(source.contains("pub fn payload(&self) -> &Description"));
    assert!(source.contains("pub fn into_payload(self) -> Description"));
    assert!(source.contains("pub fn record(payload: Entry) -> Self"));
    assert!(source.contains("pub fn record_accepted(payload: Integer) -> Self"));
}

#[test]
fn emission_can_disable_nota_surface_for_binary_only_consumers() {
    // Binary-only shape — daemons and other binary-only consumers
    // ship zero NOTA derives and zero `nota::*` references.
    // The emitted source carries only the `rkyv` + signal-frame
    // surface, so the generated module compiles when the consumer
    // does not depend on `nota` at all.
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = RustEmitter::new(RustEmissionOptions {
        nota_surface: NotaSurface::Disabled,
        target: RustEmissionTarget::ComponentRuntime,
    })
    .emit_code_from_true_schema(&schema);
    let code = generated.as_str();

    assert!(code.contains("rkyv::Archive"));
    assert!(code.contains("pub fn encode_signal_frame"));
    assert!(code.contains("pub fn decode_signal_frame"));
    assert!(!code.contains("nota"));
    assert!(!code.contains("NotaDecode"));
    assert!(!code.contains("NotaEncode"));
    assert!(!code.contains("from_nota_block"));
    assert!(!code.contains("to_nota"));
    assert!(!code.contains("FromStr"));
    assert!(!code.contains("impl std::fmt::Display for Input"));
    assert!(!code.contains("impl std::fmt::Display for Output"));
    // No leftover `#[cfg(feature = ...)]` directives either —
    // `Disabled` removes the whole NOTA surface, it doesn't gate it.
    assert!(!code.contains("#[cfg(feature ="));
    assert!(!code.contains("#[cfg_attr(feature ="));
    // Lock in the exact emitted source so any future drift forces a
    // conscious fixture update.
    assert_generated_fixture("spirit_generated_binary_only.rs", code);
}

#[test]
fn emission_can_gate_nota_surface_behind_text_client_feature() {
    // Feature-gated shape (the default) — every NOTA surface lands
    // behind `#[cfg(feature = "nota-text")]` on impls / `use` items
    // and `#[cfg_attr(...)]` on derives, so a single emitted module
    // serves both text-facing CLIs (with the feature on) and
    // binary-only daemons (with the feature off).
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = RustEmitter::new(RustEmissionOptions {
        nota_surface: NotaSurface::FeatureGated {
            feature: "nota-text".to_owned(),
        },
        target: RustEmissionTarget::ComponentRuntime,
    })
    .emit_code_from_true_schema(&schema);
    let code = generated.as_str();

    assert!(code.contains("derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode)"));
    assert!(code.contains("#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize"));
    assert!(code.contains("#[cfg(feature = \"nota-text\")]\npub use nota::{"));
    assert!(code.contains("#[cfg(feature = \"nota-text\")]\nimpl std::str::FromStr for Input"));
    assert!(code.contains("#[cfg(feature = \"nota-text\")]\nimpl std::fmt::Display for Output"));
    assert!(code.contains("pub fn encode_signal_frame"));
    assert!(code.contains("pub fn decode_signal_frame"));
    // The default shape is identical to this explicit
    // construction — the checked-in `spirit_generated.rs` snapshot
    // is the binding form. Constructed-positional and default-
    // constructed emissions must stay byte-identical.
    let default_generated = RustEmitter::default().emit_code_from_true_schema(&schema);
    assert_eq!(code, default_generated.as_str());
}

#[test]
fn rust_emission_options_default_is_feature_gated_nota_text() {
    // `RustEmissionOptions::default()` and `RustEmitter::default()`
    // both pick the compatibility-oriented opt-in shape per the
    // codec opt-in design: rkyv is universal, NOTA is gated by the
    // `nota-text` feature.
    let options = RustEmissionOptions::default();
    assert_eq!(
        options.nota_surface,
        NotaSurface::FeatureGated {
            feature: "nota-text".to_owned(),
        },
    );
    assert_eq!(options.target, RustEmissionTarget::ComponentRuntime);
}

#[test]
fn emitted_path_mirrors_schema_module_identity() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit-next:signal:public");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);

    assert_eq!(generated.path, "src/schema/signal/public.rs");
}

#[test]
fn inline_private_schema_types_emit_crate_local_rust_boundary() {
    // A crate-private nested type emits a `pub(crate)` Rust boundary, and a
    // public type whose fields reference it borrows that boundary onto each
    // field. The crate-private declaration is `Receipt`, minted as a
    // `PrivateHelper` inline declaration inside a nested root-enum variant
    // payload (`Select [(Receipt { … })]`) — the currently-supported
    // private-declaration spelling now that the strict-positional grammar no
    // longer parses a Type-followed-by-brace as a struct field. `Entry` is a
    // public namespace struct that references `Receipt` by name, so both of
    // its fields downgrade to `pub(crate)` to keep the boundary closed.
    let schema = FixtureSchema::new("inline-private-type.schema").lower("example:inline");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub(crate) struct Receipt"));
    assert!(code.contains("pub struct Entry"));
    assert!(code.contains("pub(crate) selected: Receipt"));
    assert!(code.contains("pub(crate) later: Receipt"));
}

#[test]
fn emits_schema_plane_engine_traits_for_declared_signal_nexus_and_sema_languages() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("spirit:lib");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);

    assert!(generated.code.as_str().contains("pub trait SignalEngine"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum EngineStartFailure")
    );
    assert!(generated.code.as_str().contains("ResourceBusy(String)"));
    assert!(
        generated
            .code
            .as_str()
            .contains("ConfigurationInvalid(String)")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum EngineStopFailure")
    );
    assert!(generated.code.as_str().contains("ResourceLocked(String)"));
    assert!(
        generated
            .code
            .as_str()
            .contains("ChildStillRunning(String)")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("fn on_start(&mut self) -> Result<(), EngineStartFailure>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("fn on_stop(&mut self) -> Result<(), EngineStopFailure>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("fn trace_signal_activation(&self, _object_name: SignalObjectName) {}")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("self.trace_signal_activation(SignalObjectName::Triaged)")
    );
    assert!(generated.code.as_str().contains("pub enum ObjectName"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub struct TraceEvent(pub ObjectName);")
    );
    assert!(
        !generated
            .code
            .as_str()
            .contains("pub object_name: ObjectName")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum SignalObjectName")
    );
    assert!(generated.code.as_str().contains("pub enum NexusObjectName"));
    assert!(generated.code.as_str().contains("pub enum SemaObjectName"));
    assert!(generated.code.as_str().contains("Input(InputRoute)"));
    assert!(generated.code.as_str().contains("Started,"));
    assert!(generated.code.as_str().contains("Stopped,"));
    assert!(generated.code.as_str().contains("Triaged,"));
    assert!(generated.code.as_str().contains("Work(NexusWorkRoute)"));
    assert!(
        generated
            .code
            .as_str()
            .contains("ReadInput(SemaReadInputRoute)")
    );
    assert_code_contains(
        generated.code.as_str(),
        "fn triage_inner(&self, input: signal::Signal<signal::Input>) -> nexus::Nexus<nexus::Work>;",
    );
    assert_code_contains(
        generated.code.as_str(),
        "fn triage(&self, input: signal::Signal<signal::Input>) -> nexus::Nexus<nexus::Work> {",
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("self.trace_signal_replied();")
    );
    assert!(generated.code.as_str().contains("pub trait NexusEngine"));
    assert!(generated.code.as_str().contains("pub mod nexus"));
    assert!(generated.code.as_str().contains("pub mod sema"));
    assert!(generated.code.as_str().contains("pub enum NexusWorkRoute"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum NexusActionRoute")
    );
    assert_code_contains(
        generated.code.as_str(),
        "fn decide(&mut self, input: nexus::Nexus<nexus::Work>) -> nexus::Nexus<nexus::Action>;",
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("self.trace_nexus_activation(NexusObjectName::Entered)")
    );
    assert!(generated.code.as_str().contains("pub trait SemaEngine"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum SemaWriteInputRoute")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub enum SemaReadInputRoute")
    );
    assert_code_contains(
        generated.code.as_str(),
        "fn apply_inner(&mut self, input: sema::Sema<sema::WriteInput>) -> sema::Sema<sema::WriteOutput>;",
    );
    assert_code_contains(
        generated.code.as_str(),
        "fn observe_inner(&self, input: sema::Sema<sema::ReadInput>) -> sema::Sema<sema::ReadOutput>;",
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("self.trace_sema_activation(SemaObjectName::WriteApplied)")
    );
    assert!(!generated.code.as_str().contains("NexusMail<Payload>"));
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn into_nexus_action(self) -> nexus::Nexus<nexus::Action>")
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "Input::Record(payload)",
        "SemaWriteInput::Record(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "Input::Observe(payload)",
        "SemaReadInput::Observe(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "Input::Lookup(payload)",
        "SemaReadInput::Lookup(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "Input::Count(payload)",
        "SemaReadInput::Count(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "SemaWriteOutput::Recorded(payload)",
        "Output::RecordAccepted(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "SemaReadOutput::Observed(payload)",
        "Output::RecordsObserved(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "SemaReadOutput::Found(payload)",
        "Output::RecordFound(payload)",
    );
    assert_nexus_action_arm(
        generated.code.as_str(),
        "SemaReadOutput::Counted(payload)",
        "Output::RecordsCounted(payload)",
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn into_sema_write_input(self) -> sema::Sema<sema::WriteInput>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn into_sema_read_input(self) -> sema::Sema<sema::ReadInput>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("pub fn into_signal_output(self) -> signal::Signal<signal::Output>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("impl sema::Sema<sema::WriteOutput>")
    );
    assert!(
        generated
            .code
            .as_str()
            .contains("impl sema::Sema<sema::ReadOutput>")
    );
}

#[test]
fn nexus_runner_shape_emits_total_projection_and_generated_adapter() {
    let schema = FixtureSchema::new("runner-triad.schema").lower("spirit:lib");
    let generated = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::ComponentRuntime),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert_generated_fixture("runner_generated.rs", code);

    assert!(code.contains("pub type NexusRunnerNextStep = triad_runtime::NextStep<"));
    assert!(code.contains("    ReplyToSignal,"));
    assert!(code.contains("    CommandSemaWrite,"));
    assert!(code.contains("    CommandSemaRead,"));
    assert!(code.contains("    CommandEffect,"));
    assert!(code.contains("    NexusWork,"));
    assert!(code.contains("impl triad_runtime::NexusAction for NexusAction"));
    assert!(code.contains("type SemaWrite = SemaWriteInput;"));
    assert!(code.contains("type SemaRead = SemaReadInput;"));
    assert!(code.contains("type Effect = NexusEffectCommand;"));
    assert!(code.contains("type Work = NexusWork;"));
    assert!(code.contains("fn into_next_step(self) -> NexusRunnerNextStep"));
    assert!(code.contains("impl triad_runtime::NexusWork for NexusWork {}"));
    assert!(code.contains("impl triad_runtime::SemaWriteInput for SemaWriteInput {}"));
    assert!(code.contains("impl triad_runtime::SemaReadInput for SemaReadInput {}"));
    assert!(code.contains("impl triad_runtime::NexusEffectCommand for NexusEffectCommand {}"));
    assert!(code.contains("impl triad_runtime::NexusEffectResult for NexusEffectResult {}"));
    assert!(
        code.contains("Self::CommandSemaWrite(input) => triad_runtime::NextStep::SemaWrite(input)")
    );
    assert!(
        code.contains("Self::CommandSemaRead(input) => triad_runtime::NextStep::SemaRead(input)")
    );
    assert!(code.contains("Self::ReplyToSignal(output) => triad_runtime::NextStep::Reply(output)"));
    assert!(
        code.contains("Self::CommandEffect(effect) => triad_runtime::NextStep::RunEffect(effect)")
    );
    assert!(code.contains("Self::Continue(work) => triad_runtime::NextStep::Continue(work)"));
    assert!(code.contains("struct NexusRunnerAdapter<'engine, Engine>"));
    assert!(code.contains("impl<'engine, Engine> triad_runtime::RunnerEngines"));
    assert!(code.contains("for NexusRunnerAdapter<'engine, Engine>"));
    assert!(code.contains("triad_runtime::NexusAction::into_next_step(action)"));
    assert!(code.contains("fn continuation_limit(&self) -> triad_runtime::ContinuationLimit"));
    assert_code_contains(
        code,
        "fn apply_sema_write(&mut self, origin_route: OriginRoute, input: SemaWriteInput) -> impl std::future::Future<Output = SemaWriteOutput> + Send + '_;",
    );
    assert_code_contains(
        code,
        "fn observe_sema_read(&mut self, origin_route: OriginRoute, input: SemaReadInput) -> impl std::future::Future<Output = SemaReadOutput> + Send + '_;",
    );
    assert_code_contains(
        code,
        "fn run_effect(&mut self, input: NexusEffectCommand) -> impl std::future::Future<Output = NexusEffectResult> + Send + '_;",
    );
    assert_code_contains(
        code,
        "fn budget_exhausted_reply(&self, exhausted: triad_runtime::ContinuationExhausted) -> Output;",
    );
    assert!(code.contains("let runner = triad_runtime::Runner::new(self.continuation_limit());"));
    assert!(code.contains("let reply = runner.drive(&mut runner_adapter, first_work).await;"));
    assert_code_contains(
        code,
        "let output = NexusAction::reply_to_signal(reply).with_origin_route(origin_route);",
    );
    assert_code_contains(
        code,
        "fn execute(&mut self, input: nexus::Nexus<nexus::Work>) -> impl std::future::Future<Output = nexus::Nexus<nexus::Action>> + Send + '_",
    );
    assert_code_contains(
        code,
        "async fn run_effect(&mut self, effect: Self::Effect) -> Self::Work",
    );
    assert!(!code.contains("NexusAction::CommandEffect(effect) => panic!"));
}

#[test]
fn wire_contract_target_emits_wire_codecs_without_runtime_plane_support() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("spirit:lib");
    let generated = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::WireContract),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub enum Input"));
    assert!(code.contains("pub enum Output"));
    assert!(code.contains("pub enum NexusWork"));
    assert!(code.contains("pub enum SemaReadInput"));
    assert!(code.contains("rkyv::Archive"));
    assert!(code.contains("pub mod short_header"));

    // A separately-generated wire contract IS the wire framing: peers and
    // the owning daemon import it and call the basic frame codec on it, so
    // the contract carries route enums, the codec, and the frame error
    // even though it emits no daemon-side runtime planes.
    assert!(code.contains("pub enum InputRoute"));
    assert!(code.contains("pub enum OutputRoute"));
    assert!(code.contains("pub fn encode_signal_frame"));
    assert!(code.contains("pub fn decode_signal_frame"));
    assert!(code.contains("pub enum SignalFrameError"));

    // The published contract crate must be a real signal-frame contract,
    // not only a daemon-local short-header payload codec.
    assert!(code.contains("impl signal_frame::RequestPayload for Input"));
    assert!(code.contains("impl signal_frame::SignalOperationHeads for Input"));
    assert!(code.contains("pub type Frame = signal_frame::ExchangeFrame<Input, Output>;"));
    assert!(code.contains("pub type FrameBody = signal_frame::ExchangeFrameBody<Input, Output>;"));
    assert!(code.contains("pub type Request = signal_frame::Request<Input>;"));
    assert!(code.contains("pub type ReplyEnvelope = signal_frame::Reply<Output>;"));
    assert!(code.contains("pub type RequestBuilder = signal_frame::RequestBuilder<Input>;"));
    assert!(code.contains("pub fn into_frame"));
    assert!(code.contains("pub fn into_reply_frame"));

    // Subscription event push remains gated behind declared stream metadata.
    assert!(!code.contains("into_subscription_frame"));

    assert!(!code.contains("pub trait SignalEngine"));
    assert!(!code.contains("pub trait NexusEngine"));
    assert!(!code.contains("pub trait SemaEngine"));
    assert!(!code.contains("pub enum EngineStartFailure"));
    assert!(!code.contains("pub struct MessageSent"));
    assert!(!code.contains("pub struct OriginRoute"));
    assert!(!code.contains("pub enum Plane"));
    assert!(!code.contains("pub mod signal"));
    assert!(!code.contains("pub enum NexusWorkRoute"));
    assert!(!code.contains("pub struct TraceEvent"));
    assert!(!code.contains("pub fn into_nexus_action"));
    assert!(!code.contains("impl triad_runtime::NexusWork"));
    assert!(!code.contains("impl triad_runtime::SemaWriteInput"));
    assert!(!code.contains("impl triad_runtime::NexusAction for NexusAction"));
    assert!(!code.contains("pub trait UpgradeFrom<Previous>"));
}

/// Regression for the gb95 over-reach. gb95 correctly stopped the
/// internal `NexusRuntime` / `SemaRuntime` planes from receiving wire
/// frame code, but it gated ALL frame-codec emission behind
/// `emits_signal()`, which is false for `WireContract` — stripping the
/// codec from freshly-generated wire contracts. The split gate restores
/// the basic codec and signal-frame transport surface for every wire-facing
/// target while keeping subscription event push gated behind a declared stream.
#[test]
fn frame_codec_reaches_wire_contract_targets_but_not_internal_planes() {
    let schema = FixtureSchema::new("plane-triad.schema");

    let wire_contract = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::WireContract),
    )
    .emit_file_from_true_schema(&schema.lower("spirit:lib"));
    let wire_code = wire_contract.code.as_str();

    // (1) The wire contract carries the basic frame codec + route enums.
    assert!(wire_code.contains("pub enum InputRoute"));
    assert!(wire_code.contains("pub enum OutputRoute"));
    assert!(wire_code.contains("pub fn encode_signal_frame"));
    assert!(wire_code.contains("pub fn decode_signal_frame"));
    assert!(wire_code.contains("pub enum SignalFrameError"));

    // (2) Even without streams, a contract crate exposes the universal
    // signal-frame request/reply surface its consumers import.
    assert!(wire_code.contains("impl signal_frame::RequestPayload for Input"));
    assert!(wire_code.contains("impl signal_frame::SignalOperationHeads for Input"));
    assert!(wire_code.contains("pub type Frame = signal_frame::ExchangeFrame<Input, Output>;"));
    assert!(
        wire_code.contains("pub type FrameBody = signal_frame::ExchangeFrameBody<Input, Output>;")
    );
    assert!(wire_code.contains("pub type Request = signal_frame::Request<Input>;"));
    assert!(wire_code.contains("pub type ReplyEnvelope = signal_frame::Reply<Output>;"));
    assert!(wire_code.contains("pub type RequestBuilder = signal_frame::RequestBuilder<Input>;"));

    // (3) No declared stream, so no subscription event push surface.
    assert!(!wire_code.contains("into_subscription_frame"));

    // The internal Nexus plane is not wire-facing: it carries NEITHER the
    // frame codec NOR the transport surface.
    let nexus_runtime = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::NexusRuntime),
    )
    .emit_file_from_true_schema(&schema.lower("daemon:nexus"));
    let nexus_code = nexus_runtime.code.as_str();

    assert!(!nexus_code.contains("pub fn encode_signal_frame"));
    assert!(!nexus_code.contains("pub fn decode_signal_frame"));
    assert!(!nexus_code.contains("pub enum SignalFrameError"));
    assert!(!nexus_code.contains("pub type Frame ="));
    assert!(!nexus_code.contains("into_subscription_frame"));
    assert!(!nexus_code.contains("impl signal_frame::RequestPayload for Input"));
}

#[test]
fn signal_runtime_target_emits_signal_runtime_without_nexus_or_sema_support() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("spirit:lib");
    let generated = RustEmitter::new(
        RustEmissionOptions::feature_gated_nota("nota-text")
            .with_target(RustEmissionTarget::SignalRuntime),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub enum Input"));
    assert!(code.contains("pub enum Output"));
    assert!(code.contains("pub struct OriginRoute"));
    assert!(code.contains("pub struct MessageIdentifier"));
    assert!(code.contains("pub struct Signal<Root>"));
    assert!(code.contains("pub struct MessageSent"));
    assert!(code.contains("pub struct MessageProcessed<Reply>"));
    assert!(code.contains("#[allow(clippy::module_inception)]\npub mod signal"));
    assert!(code.contains("pub mod signal"));
    assert!(code.contains("pub enum SignalObjectName"));
    assert!(code.contains("pub trait SignalEngine"));
    assert!(code.contains("    type NexusInput;"));
    assert!(code.contains("    type NexusOutput;"));
    assert!(code.contains(
        "fn triage_inner(&self, input: signal::Signal<signal::Input>) -> Self::NexusInput;"
    ));
    assert!(code.contains(
        "fn reply_inner(&self, output: Self::NexusOutput) -> signal::Signal<signal::Output>;"
    ));
    assert!(code.contains("pub fn encode_signal_frame"));
    assert!(code.contains("pub fn decode_signal_frame"));

    assert!(!code.contains("pub trait NexusEngine"));
    assert!(!code.contains("pub trait SemaEngine"));
    assert!(!code.contains("pub struct Nexus<Root>"));
    assert!(!code.contains("pub struct Sema<Root>"));
    assert!(!code.contains("pub enum NexusObjectName"));
    assert!(!code.contains("pub enum SemaObjectName"));
    assert!(!code.contains("pub enum Plane"));
    assert!(!code.contains("pub type NexusRunnerNextStep"));
}

#[test]
fn nexus_runtime_target_emits_only_nexus_runtime_support_even_when_other_plane_names_exist() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("daemon:nexus");
    let generated = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::NexusRuntime),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub enum NexusWork"));
    assert!(code.contains("pub enum NexusAction"));
    assert!(code.contains("pub struct OriginRoute"));
    assert!(code.contains("pub struct Nexus<Root>"));
    assert!(code.contains("#[allow(clippy::module_inception)]\npub mod nexus"));
    assert!(code.contains("pub mod nexus"));
    assert!(code.contains("pub enum NexusWorkRoute"));
    assert!(code.contains("pub enum NexusObjectName"));
    assert!(code.contains("pub trait NexusEngine"));

    assert!(!code.contains("pub trait SignalEngine"));
    assert!(!code.contains("pub trait SemaEngine"));
    assert!(!code.contains("pub struct Signal<Root>"));
    assert!(!code.contains("pub struct Sema<Root>"));
    assert!(!code.contains("pub enum SignalObjectName"));
    assert!(!code.contains("pub enum SemaObjectName"));
    assert!(!code.contains("pub enum SemaWriteInputRoute"));
    assert!(!code.contains("pub enum SemaReadInputRoute"));
    assert!(!code.contains("pub struct MessageSent"));
    assert!(!code.contains("pub enum Plane"));
    assert!(!code.contains("pub mod short_header"));
    assert!(!code.contains("pub fn encode_signal_frame"));
    assert!(!code.contains("pub fn decode_signal_frame"));
    assert!(!code.contains("pub enum SignalFrameError"));
    assert!(!code.contains("pub fn into_nexus_action"));
    assert!(!code.contains("pub fn into_sema_write_input"));
}

#[test]
fn sema_runtime_target_emits_only_sema_runtime_support_even_when_other_plane_names_exist() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("daemon:sema");
    let generated = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::SemaRuntime),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub enum SemaWriteInput"));
    assert!(code.contains("pub enum SemaReadInput"));
    assert!(code.contains("pub struct OriginRoute"));
    assert!(code.contains("pub struct Sema<Root>"));
    assert!(code.contains("#[allow(clippy::module_inception)]\npub mod sema"));
    assert!(code.contains("pub mod sema"));
    assert!(code.contains("pub enum SemaWriteInputRoute"));
    assert!(code.contains("pub enum SemaReadInputRoute"));
    assert!(code.contains("pub enum SemaObjectName"));
    assert!(code.contains("pub trait SemaEngine"));
    assert!(code.contains("fn apply_inner("));
    assert!(code.contains("fn observe_inner("));

    assert!(!code.contains("pub trait SignalEngine"));
    assert!(!code.contains("pub trait NexusEngine"));
    assert!(!code.contains("pub struct Signal<Root>"));
    assert!(!code.contains("pub struct Nexus<Root>"));
    assert!(!code.contains("pub enum SignalObjectName"));
    assert!(!code.contains("pub enum NexusObjectName"));
    assert!(!code.contains("pub enum NexusWorkRoute"));
    assert!(!code.contains("pub struct MessageSent"));
    assert!(!code.contains("pub enum Plane"));
    assert!(!code.contains("pub mod short_header"));
    assert!(!code.contains("pub fn encode_signal_frame"));
    assert!(!code.contains("pub fn decode_signal_frame"));
    assert!(!code.contains("pub enum SignalFrameError"));
    assert!(!code.contains("pub fn into_nexus_action"));
    assert!(!code.contains("pub fn into_signal_output"));
}

#[test]
fn sema_runtime_target_accepts_plane_local_root_names() {
    let schema = FixtureSchema::new("sema-plane-local.schema").lower("daemon:sema");
    let generated = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::SemaRuntime),
    )
    .emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub enum WriteInput"));
    assert!(code.contains("pub enum ReadInput"));
    assert!(code.contains("pub enum WriteOutput"));
    assert!(code.contains("pub enum ReadOutput"));
    assert!(code.contains("pub struct Sema<Root>"));
    assert!(code.contains("#[allow(clippy::module_inception)]\npub mod sema"));
    assert!(code.contains("pub mod sema"));
    assert!(code.contains("pub type WriteInput = super::WriteInput;"));
    assert!(code.contains("pub type ReadInput = super::ReadInput;"));
    assert!(code.contains("impl WriteInput"));
    assert!(code.contains("impl ReadOutput"));
    assert!(code.contains("pub enum SemaObjectName"));
    assert!(code.contains("WriteApplied,"));
    assert!(code.contains("ReadObserved,"));
    assert!(code.contains("pub trait SemaEngine"));
    assert_code_contains(
        code,
        "fn apply_inner(&mut self, input: sema::Sema<sema::WriteInput>) -> sema::Sema<sema::WriteOutput>;",
    );
    assert_code_contains(
        code,
        "fn observe_inner(&self, input: sema::Sema<sema::ReadInput>) -> sema::Sema<sema::ReadOutput>;",
    );

    assert!(!code.contains("pub trait SignalEngine"));
    assert!(!code.contains("pub trait NexusEngine"));
    assert!(!code.contains("pub struct Signal<Root>"));
    assert!(!code.contains("pub struct Nexus<Root>"));
}

#[test]
fn runtime_target_emits_read_only_sema_engine_when_read_roots_exist() {
    let schema = FixtureSchema::new("sema-read-only.schema").lower("daemon:sema");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub trait SemaEngine"));
    assert!(code.contains("ReadObserved,"));
    assert!(code.contains("SemaObjectName::ReadObserved"));
    assert_code_contains(
        code,
        "fn observe_inner(&self, input: sema::Sema<sema::ReadInput>) -> sema::Sema<sema::ReadOutput>;",
    );
    assert_code_contains(
        code,
        "fn observe(&self, input: sema::Sema<sema::ReadInput>) -> sema::Sema<sema::ReadOutput> {",
    );

    assert!(!code.contains("WriteApplied,"));
    assert!(!code.contains("SemaObjectName::WriteApplied"));
    assert!(!code.contains("fn apply_inner("));
    assert!(!code.contains("fn apply(&mut self"));
}

#[test]
fn runtime_target_emits_write_only_sema_engine_when_write_roots_exist() {
    let schema = FixtureSchema::new("sema-write-only.schema").lower("daemon:sema");
    let generated = RustEmitter::default().emit_file_from_true_schema(&schema);
    let code = generated.code.as_str();

    assert!(code.contains("pub trait SemaEngine"));
    assert!(code.contains("WriteApplied,"));
    assert!(code.contains("SemaObjectName::WriteApplied"));
    assert_code_contains(
        code,
        "fn apply_inner(&mut self, input: sema::Sema<sema::WriteInput>) -> sema::Sema<sema::WriteOutput>;",
    );
    assert_code_contains(
        code,
        "fn apply(&mut self, input: sema::Sema<sema::WriteInput>) -> sema::Sema<sema::WriteOutput> {",
    );

    assert!(!code.contains("ReadObserved,"));
    assert!(!code.contains("SemaObjectName::ReadObserved"));
    assert!(!code.contains("fn observe_inner("));
    assert!(!code.contains("fn observe(&self"));
}

#[test]
fn generated_trace_identity_is_typed_from_interface_headers() {
    let signal_route = generated::ObjectName::Signal(generated::SignalObjectName::Input(
        generated::InputRoute::Record,
    ));

    assert_eq!(signal_route.name(), "SignalInputRecord");

    let event = generated::TraceEvent::new(signal_route);
    assert_eq!(event.object_name(), signal_route);
    assert_eq!(event.name(), "SignalInputRecord");
    assert_eq!(
        generated::NotaEncode::to_nota(&event),
        "(Signal (Input Record))"
    );

    let archive =
        rkyv::to_bytes::<rkyv::rancor::Error>(&event).expect("trace event archives as rkyv");
    let decoded = rkyv::from_bytes::<generated::TraceEvent, rkyv::rancor::Error>(&archive)
        .expect("trace event decodes from rkyv");
    assert_eq!(decoded, event);
}

#[test]
fn compiled_fixture_is_usable_rust() {
    let entry = generated::Entry {
        topics: generated::Topics::new(vec![generated::Topic::new("schema")]),
        kind: generated::Kind::Decision,
        description: generated::Description::new("schema drives rust"),
        magnitude: generated::Magnitude::Maximum,
    };
    let input = generated::Input::Record(entry);

    assert!(matches!(input, generated::Input::Record(_)));
    assert_eq!(generated::short_header::INPUT_RECORD, 0x0000_0000_0000_0000);
    assert_eq!(
        generated::short_header::INPUT_OBSERVE,
        0x0001_0000_0000_0000
    );
    assert_eq!(input.route(), generated::InputRoute::Record);
    assert_eq!(input.short_header(), generated::short_header::INPUT_RECORD);
}

#[test]
fn generated_roots_wrap_into_messages_with_automatic_origin_route() {
    let input = FixtureNota::new("nota/observe-schema-principle.nota")
        .read()
        .parse::<generated::Input>()
        .expect("parse observe input");
    let message: generated::signal::Signal<generated::signal::Input> =
        input.with_origin_route(generated::OriginRoute::new(19));

    assert_eq!(message.origin_route(), generated::OriginRoute::new(19));
    let plane = generated::schema::Plane::<generated::signal::Input, (), ()>::Signal(message);
    assert_eq!(plane.origin_route(), generated::OriginRoute::new(19));
    let generated::schema::Plane::Signal(message) = plane else {
        panic!("expected signal plane");
    };
    assert!(matches!(
        message.root(),
        generated::Input::Observe(generated::Query { .. })
    ));
}

#[test]
fn generated_input_parses_cli_nota_and_emits_nota() {
    let source = FixtureNota::new("nota/record-clarified-intent.nota").read();
    let input = source
        .parse::<generated::Input>()
        .expect("parse generated input");

    match &input {
        generated::Input::Record(entry) => {
            assert_eq!(entry.topics.payload()[0], generated::Topic::new("schema"));
            assert_eq!(entry.kind, generated::Kind::Constraint);
            assert_eq!(
                entry.description,
                generated::Description::new("agent's clarified intent")
            );
            assert_eq!(entry.magnitude, generated::Magnitude::Maximum);
        }
        generated::Input::Observe(_) => panic!("expected record"),
    }

    assert_eq!(input.to_string(), source);
}

#[test]
fn generated_signal_input_round_trips_from_nota_to_rkyv_bytes() {
    let input = FixtureNota::new("nota/record-component-rkyv.nota")
        .read()
        .parse::<generated::Input>()
        .expect("parse generated input");

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&input).expect("archive input");
    let decoded =
        rkyv::from_bytes::<generated::Input, rkyv::rancor::Error>(&bytes).expect("decode input");

    assert_eq!(decoded, input);
}

#[test]
fn generated_signal_frame_methods_round_trip_and_triage_route() {
    let input = FixtureNota::new("nota/record-schema-owns-frames.nota")
        .read()
        .parse::<generated::Input>()
        .expect("parse generated input");

    let frame = input.encode_signal_frame().expect("encode signal frame");
    let (route, decoded) =
        generated::Input::decode_signal_frame(&frame).expect("decode signal frame");

    assert_eq!(route, generated::InputRoute::Record);
    assert_eq!(decoded, input);
    assert_eq!(
        generated::Input::route_from_short_header(generated::short_header::INPUT_OBSERVE)
            .expect("observe route"),
        generated::InputRoute::Observe
    );
}

struct MailHook {
    sent_events: Vec<generated::MessageSent>,
    processed_events: Vec<generated::MessageProcessed<generated::Output>>,
}

impl MailHook {
    fn new() -> Self {
        Self {
            sent_events: Vec::new(),
            processed_events: Vec::new(),
        }
    }
}

impl generated::MessageSentHook for MailHook {
    type Error = RuntimeError;

    fn message_sent(&mut self, event: generated::MessageSent) -> Result<(), Self::Error> {
        self.sent_events.push(event);
        Ok(())
    }
}

#[test]
fn generated_signal_roots_emit_typed_message_sent_events() {
    let input = FixtureNota::new("nota/observe-schema-principle.nota")
        .read()
        .parse::<generated::Input>()
        .expect("parse observe input");
    let event = input
        .with_origin_route(generated::OriginRoute::new(900))
        .message_sent(generated::MessageIdentifier::new(42));
    let mut hook = MailHook::new();

    event.push_to(&mut hook).expect("message sent event pushes");

    assert_eq!(
        hook.sent_events,
        vec![generated::MessageSent {
            identifier: generated::MessageIdentifier::new(42),
            origin_route: generated::OriginRoute::new(900),
            root: generated::MessageRoot::Input,
            short_header: generated::short_header::INPUT_OBSERVE,
        }],
    );
    assert_eq!(event.origin_route(), generated::OriginRoute::new(900));
    assert_eq!(
        generated::NotaSource::new("900")
            .parse::<generated::OriginRoute>()
            .expect("origin route decodes through shared codec"),
        generated::OriginRoute::new(900)
    );
    assert_eq!(
        generated::NotaEncode::to_nota(&generated::OriginRoute::new(900)),
        "900"
    );
    assert_eq!(
        generated::NotaSource::new("42")
            .parse::<generated::MessageIdentifier>()
            .expect("message identifier decodes through shared codec"),
        generated::MessageIdentifier::new(42)
    );
    assert_eq!(
        generated::NotaEncode::to_nota(&generated::MessageIdentifier::new(42)),
        "42"
    );
    assert_ne!(
        event.origin_route(),
        generated::OriginRoute::new(event.identifier.payload()),
        "origin route is minted separately from the message identifier"
    );
}

#[derive(Debug, PartialEq, Eq)]
enum RuntimeError {
    StateRejected,
}

impl generated::MessageProcessedHook<generated::Output> for MailHook {
    type Error = RuntimeError;

    fn message_processed(
        &mut self,
        event: generated::MessageProcessed<generated::Output>,
    ) -> Result<(), Self::Error> {
        self.processed_events.push(event);
        Ok(())
    }
}

#[test]
fn generated_processed_mail_events_are_typed_without_root_dispatch_traits() {
    assert_eq!(RuntimeError::StateRejected, RuntimeError::StateRejected);
    let mut hook = MailHook::new();
    let reply = generated::Output::RecordsObserved(generated::RecordSet::new(vec![]));

    let processed = generated::MessageProcessed::new(
        generated::MessageIdentifier::new(77),
        generated::OriginRoute::new(701),
        reply,
    );
    processed
        .push_to(&mut hook)
        .expect("processed mail event pushes");

    assert_eq!(
        hook.processed_events,
        vec![generated::MessageProcessed {
            identifier: generated::MessageIdentifier::new(77),
            origin_route: generated::OriginRoute::new(701),
            reply: generated::Output::RecordsObserved(generated::RecordSet::new(vec![])),
        }],
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviousEntry {
    topic: String,
    description: String,
}

#[derive(Debug, PartialEq, Eq)]
struct UpgradeEvent {
    description: String,
}

impl generated::UpgradeFrom<PreviousEntry> for generated::Entry {
    type Error = RuntimeError;

    fn upgrade_from(previous: PreviousEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            topics: generated::Topics::new(vec![generated::Topic::new(previous.topic)]),
            kind: generated::Kind::Clarification,
            description: generated::Description::new(previous.description),
            magnitude: generated::Magnitude::High,
        })
    }
}

impl UpgradeEvent {
    fn from_previous_entry(previous: PreviousEntry) -> Result<Self, RuntimeError> {
        let entry =
            <generated::Entry as generated::AcceptPrevious<PreviousEntry>>::accept_previous(
                previous,
            )?;
        Ok(Self {
            description: format!(
                "accepted previous Entry as {}",
                generated::NotaEncode::to_nota(&entry)
            ),
        })
    }
}

#[test]
fn generated_upgrade_trait_accepts_previous_schema_objects_observably() {
    let event = UpgradeEvent::from_previous_entry(PreviousEntry {
        topic: "schema".to_owned(),
        description: "old client spoke previous entry".to_owned(),
    })
    .expect("previous entry upgrades");

    assert_eq!(
        event,
        UpgradeEvent {
            description: "accepted previous Entry as ([schema] Clarification [old client spoke previous entry] High)".to_owned(),
        },
    );
}

#[test]
fn emits_vec_map_and_option_collection_types_with_shared_codec_traits() {
    let schema = FixtureSchema::new("collections.schema").lower("collections:lib");
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);
    let code = generated.as_str();

    // The generated code imports the shared NOTA codec instead of
    // emitting a local collection-support runtime block. Under the
    // default `feature_gated_nota("nota-text")` shape, the `use
    // nota::*` and `cfg_attr(...)` derives sit behind the
    // `nota-text` feature.
    assert!(code.contains("#[cfg(feature = \"nota-text\")]\npub use nota::{"));
    assert!(!code.contains("pub struct NotaCollection"));
    assert!(code.contains("derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode)"));
    assert!(!code.contains("impl NotaDecode for Cluster"));
    assert!(!code.contains("impl NotaEncode for Cluster"));
    // Vec / KeyValue->BTreeMap / Option render at the field positions. The new
    // schema-source grammar derives a complex positional field's name from its
    // type (a custom name needs a named newtype), so the inline collection
    // fields carry their derived names.
    assert!(code.contains("pub service_vector: Vec<Service>,"));
    assert!(code.contains(
        "pub node_config_by_node_name: std::collections::BTreeMap<NodeName, NodeConfig>,"
    ));
    assert!(code.contains("pub optional_node_config: Option<NodeConfig>,"));
    assert!(code.contains("pub healthy: Healthy,"));
    assert!(code.contains("pub config_path: ConfigPath,"));
    assert!(code.contains("pub struct Healthy(Boolean);"));
    assert!(code.contains("pub struct ConfigPath(Path);"));
    assert!(code.contains("pub type Path = std::string::String;"));
    // Collection payloads in a root output variant.
    assert!(code.contains("Projected(std::collections::BTreeMap<NodeName, NodeConfig>),"));
    assert!(code.contains("Listed(Vec<NodeName>),"));
    // Bare bindings are distinct newtypes (no aliases); a newtype used as a
    // map key supplies its own ordering via the derived Ord.
    assert!(code.contains("pub struct NodeName(String);"));
    assert!(code.contains("pub struct NodeConfig(String);"));
    assert_generated_fixture("collections_generated.rs", code);
}

#[test]
fn collection_free_schema_keeps_checked_generated_source_stable() {
    // The regression safety net: a schema that uses no collection still
    // emits exactly the checked-in fixture, proving the collection work
    // is purely additive.
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = RustEmitter::default().emit_code_from_true_schema(&schema);

    assert_generated_fixture("spirit_generated.rs", generated.as_str());
}

#[test]
fn generated_collection_struct_round_trips_through_nota() {
    // Author a Cluster carrying all three collection kinds, encode it
    // to NOTA, parse it back, and confirm the value survives.
    let cluster = collections_generated::Cluster {
        service_vector: vec![
            collections_generated::Service::new("dns"),
            collections_generated::Service::new("mail"),
        ],
        node_config_by_node_name: {
            let mut nodes = std::collections::BTreeMap::new();
            nodes.insert(
                collections_generated::NodeName::new("alpha"),
                collections_generated::NodeConfig::new("primary"),
            );
            nodes.insert(
                collections_generated::NodeName::new("beta"),
                collections_generated::NodeConfig::new("replica"),
            );
            nodes
        },
        optional_node_config: Some(collections_generated::NodeConfig::new("warm")),
        healthy: collections_generated::Healthy::new(true),
        config_path: collections_generated::ConfigPath::new("/tmp/cluster.nota"),
        digest: collections_generated::Digest::new(collections_generated::Bytes::new(vec![
            0xde, 0xad, 0xbe, 0xef,
        ])),
        fingerprint: collections_generated::Fingerprint::new(
            collections_generated::FixedBytes::new([0x01, 0x02, 0x03, 0x04]),
        ),
    };

    let encoded = collections_generated::NotaEncode::to_nota(&cluster);
    let parsed = collections_generated::NotaSource::new(&encoded)
        .parse::<collections_generated::Cluster>()
        .expect("cluster decodes");

    assert_eq!(parsed, cluster);
    // The empty / None forms also round-trip.
    let empty = collections_generated::Cluster {
        service_vector: Vec::new(),
        node_config_by_node_name: std::collections::BTreeMap::new(),
        optional_node_config: None,
        healthy: collections_generated::Healthy::new(false),
        config_path: collections_generated::ConfigPath::new("/tmp/empty.nota"),
        digest: collections_generated::Digest::new(collections_generated::Bytes::new(Vec::new())),
        fingerprint: collections_generated::Fingerprint::new(
            collections_generated::FixedBytes::new([0u8; 4]),
        ),
    };
    let empty_encoded = collections_generated::NotaEncode::to_nota(&empty);
    let empty_parsed = collections_generated::NotaSource::new(&empty_encoded)
        .parse::<collections_generated::Cluster>()
        .expect("empty cluster decodes");
    assert_eq!(empty_parsed, empty);
}

#[test]
fn generated_collection_payload_root_variant_round_trips_to_nota_and_rkyv() {
    let mut projection = std::collections::BTreeMap::new();
    projection.insert(
        collections_generated::NodeName::new("alpha"),
        collections_generated::NodeConfig::new("primary"),
    );
    let output = collections_generated::Output::Projected(projection);

    // NOTA round-trip through the root enum codec.
    let encoded = collections_generated::NotaEncode::to_nota(&output);
    let parsed = encoded
        .parse::<collections_generated::Output>()
        .expect("projected output parses");
    assert_eq!(parsed, output);

    // rkyv round-trip — the map key's ordering derives let the archive
    // form compile and compare.
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&output).expect("archive output");
    let decoded = rkyv::from_bytes::<collections_generated::Output, rkyv::rancor::Error>(&bytes)
        .expect("decode output");
    assert_eq!(decoded, output);
}
