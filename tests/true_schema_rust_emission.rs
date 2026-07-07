use nota::{Document, NotaDecode, NotaEncode};
use schema_language::{ImportResolver, SchemaEngine, SchemaIdentity, SchemaSource, TrueSchema};
use schema_rust::{RustEmissionOptions, RustEmissionTarget, RustEmitter};

mod support;

use support::FixtureSchema;

const SPIRIT_SCHEMA_VERSION: &str = "0.10.0";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SpiritSignalSchemaFixture {
    signal_source_text: String,
    domain_source_text: String,
}

impl SpiritSignalSchemaFixture {
    fn current() -> Self {
        Self {
            signal_source_text: FixtureSchema::new("spirit-signal/signal.schema").read(),
            domain_source_text: FixtureSchema::new("spirit-signal/domain.schema").read(),
        }
    }

    fn signal_source(&self) -> SchemaSource {
        SchemaSource::from_schema_text(&self.signal_source_text)
            .expect("Spirit signal schema source decodes")
    }

    fn signal_identity(&self) -> SchemaIdentity {
        SchemaIdentity::new("signal-spirit:signal", SPIRIT_SCHEMA_VERSION)
    }

    fn resolver(&self) -> ImportResolver {
        ImportResolver::new().with_module_source(
            "signal-spirit",
            "domain",
            SPIRIT_SCHEMA_VERSION,
            self.domain_source_text.clone(),
        )
    }

    fn lower_to_true_schema(&self) -> TrueSchema {
        SchemaEngine::default()
            .lower_schema_source_with_resolver(
                &self.signal_source(),
                self.signal_identity(),
                &self.resolver(),
            )
            .expect("Spirit signal schema lowers into TrueSchema")
    }

    fn round_trip_through_structured_nota(&self, schema: &TrueSchema) -> TrueSchema {
        let rendered = schema.to_nota();
        let expected_prefix = format!("((signal-spirit:signal {SPIRIT_SCHEMA_VERSION}) [");
        assert!(
            rendered.starts_with(&expected_prefix),
            "structured TrueSchema NOTA should begin with the Spirit signal identity"
        );
        assert!(
            rendered.contains(
                "(SubscribeIntent (Some (Plain SubscribeIntent)) (Some (Opens IntentEventStream)))"
            ),
            "structured TrueSchema NOTA should preserve the Spirit stream-opening root variant"
        );
        assert!(
            rendered.contains(
                "(Public Entry [] (Struct (Entry {domains (Plain Domains) kind (Plain Kind) description (Plain Description) certainty (Plain Certainty) importance (Plain Importance) privacy (Plain Privacy) referents (Plain Referents)})) ([]))"
            ),
            "structured TrueSchema NOTA should include the decoded Spirit Entry declaration"
        );

        let document = Document::parse(&rendered).expect("structured TrueSchema NOTA parses");
        assert_eq!(
            document.holds_root_objects(),
            1,
            "TrueSchema projection should contain one NOTA root object"
        );
        let decoded = TrueSchema::from_nota_block(&document.root_objects()[0])
            .expect("structured TrueSchema NOTA decodes");
        assert_eq!(
            decoded, *schema,
            "structured NOTA projection should decode back to the same TrueSchema"
        );
        decoded
    }
}

fn assert_same_rust_source(left: &str, right: &str, context: &str) {
    if left == right {
        return;
    }

    let differing_byte = left
        .bytes()
        .zip(right.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| left.len().min(right.len()));
    let left_excerpt = TextExcerpt::new(left, differing_byte).render();
    let right_excerpt = TextExcerpt::new(right, differing_byte).render();
    panic!(
        "{context} diverged at byte {differing_byte}\nleft: {left_excerpt}\nright: {right_excerpt}"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextExcerpt<'text> {
    text: &'text str,
    byte_index: usize,
}

impl<'text> TextExcerpt<'text> {
    fn new(text: &'text str, byte_index: usize) -> Self {
        Self { text, byte_index }
    }

    fn render(self) -> String {
        let start = self.text[..self.byte_index]
            .char_indices()
            .rev()
            .nth(80)
            .map(|(index, _)| index)
            .unwrap_or(0);
        let end = self.text[self.byte_index..]
            .char_indices()
            .nth(80)
            .map(|(index, _)| self.byte_index + index)
            .unwrap_or(self.text.len());
        self.text[start..end].replace('\n', "\\n")
    }
}

#[test]
fn spirit_signal_rust_emission_uses_true_schema_projection() {
    let fixture = SpiritSignalSchemaFixture::current();
    let true_schema = fixture.lower_to_true_schema();
    let recovered_true_schema = fixture.round_trip_through_structured_nota(&true_schema);
    let emitter = RustEmitter::new(
        RustEmissionOptions::default().with_target(RustEmissionTarget::WireContract),
    );
    let engine = SchemaEngine::default();
    let signal_source = fixture.signal_source();
    let resolver = fixture.resolver();

    let source_module = emitter
        .emit_module_from_schema_source(
            &signal_source,
            fixture.signal_identity(),
            &engine,
            &resolver,
        )
        .expect("source-lowering emission produces a Rust module");
    let true_schema_module = emitter.emit_module_from_true_schema(&recovered_true_schema);
    assert!(
        source_module == true_schema_module,
        "source-lowering emission must delegate to the same RustModule value emitted from TrueSchema"
    );

    let source_file = emitter
        .emit_file_from_schema_source(
            &signal_source,
            fixture.signal_identity(),
            &engine,
            &resolver,
        )
        .expect("source-lowering emission produces a Rust file");
    let true_schema_file = emitter.emit_file_from_true_schema(&recovered_true_schema);
    assert_eq!(
        source_file.path, true_schema_file.path,
        "source and TrueSchema emission should choose the same generated module path"
    );
    assert_same_rust_source(
        source_file.code.as_str(),
        true_schema_file.code.as_str(),
        "Spirit signal source-lowering Rust and TrueSchema Rust",
    );

    let generated = true_schema_file.code.as_str();
    assert!(
        generated.contains("pub use crate::schema::domain::Domain as Domain;"),
        "real Spirit same-crate imports should lower to Rust module aliases"
    );
    assert!(
        generated.contains("pub enum Input"),
        "real Spirit signal input root should be emitted"
    );
    assert!(
        generated.contains("SubscribeIntent(SubscribeIntent)"),
        "real Spirit stream-opening operation should be emitted"
    );
    assert!(
        generated.contains("pub enum IntentEvent"),
        "real Spirit stream event enum should be emitted"
    );
    assert!(
        generated
            .contains("pub type Frame = signal_frame::StreamingFrame<Input, Output, IntentEvent>;"),
        "real Spirit stream metadata should choose the streaming wire frame"
    );
    assert!(
        generated.contains("pub fn into_subscription_frame("),
        "stream event payloads should receive subscription-frame emission"
    );
    assert!(
        !generated.contains("pub enum Plane"),
        "wire-contract emission must not include component runtime plane support"
    );
}
