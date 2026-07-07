//! Proof for spirit's frame application (designer slice, Part 2a) — UPDATED to
//! the EXPANSION pipeline.
//!
//! `spirit-nexus.schema` imports the shared `Work`/`Action` frame and applies
//! it at the root positions. The emitter now MONOMORPHIZES each applied root:
//! it expands the applied frame head (binder -> argument substitution over the
//! frame's variants) into a CONCRETE Rust enum named by the root position, so
//! `Input` and `Output` are real `pub enum` bodies — `Input` carries the four
//! Work legs bound to spirit's payloads, `Output` the five Action legs with the
//! recursive `Continue(Input)` leg re-aimed at the sibling Input enum. The
//! concrete enums flow through the existing concrete-enum emitters, so they
//! gain auto-emitted constructors and `From` impls. No `pub type Input =
//! Work<…>` alias, no `into_next_step` shim. The generated module is `include!`d
//! below over a local stand-in `reaction` frame module (which still defines the
//! generic frame enums the `pub use` import lines re-export), compiled, and the
//! concrete `Input`/`Output` enums round-trip through rkyv and NOTA — including
//! the recursive `Continue(Input)` leg — through the EMITTED constructors.

use schema_language::{ImportResolver, MacroContext, SchemaEngine, SchemaIdentity};
use schema_rust::{RustEmissionOptions, RustEmissionTarget, RustEmitter};

mod support;

use support::FixtureSchema;

fn emit_spirit_nexus() -> String {
    let reaction_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reaction/schema");
    let resolver = ImportResolver::new().with_dependency(
        "reaction",
        reaction_dir
            .to_str()
            .expect("reaction fixture path is utf-8"),
        "0.1.0",
    );
    let source = FixtureSchema::new("reaction/schema/spirit-nexus.schema").read();
    let mut context = MacroContext::default();
    let schema = SchemaEngine::default()
        .lower_source_with_resolver(
            &source,
            SchemaIdentity::new("spirit:nexus", "0.1.0"),
            &mut context,
            &resolver,
        )
        .expect("spirit-nexus lowers through import + root-application");
    let options = RustEmissionOptions::feature_gated_nota("nota-text")
        .with_target(RustEmissionTarget::NexusRuntime);
    RustEmitter::new(options)
        .emit_code_from_true_schema(&schema)
        .as_str()
        .to_owned()
}

fn emit_named_spirit_nexus() -> String {
    let source = FixtureSchema::new("reaction/schema/spirit-nexus-named.schema").read();
    let schema = SchemaEngine::default()
        .lower_source(&source, SchemaIdentity::new("spirit:nexus", "0.1.0"))
        .expect("spirit-nexus named frame applications lower");
    let options = RustEmissionOptions::feature_gated_nota("nota-text")
        .with_target(RustEmissionTarget::NexusRuntime);
    RustEmitter::new(options)
        .emit_code_from_true_schema(&schema)
        .as_str()
        .to_owned()
}

#[test]
fn spirit_expands_applied_roots_to_concrete_enums() {
    let code = emit_spirit_nexus();

    // The Input root expands to a CONCRETE enum carrying the four Work legs
    // bound to spirit's payload vocabulary — NOT a `pub type Input = Work<…>`
    // alias.
    assert!(
        !code.contains("pub type Input ="),
        "Input root is no longer a frame-application alias:\n{code}"
    );
    assert!(
        !code.contains("pub type Output ="),
        "Output root is no longer a frame-application alias:\n{code}"
    );
    assert!(
        code.contains("pub enum Input {"),
        "spirit Input root is a concrete enum body:\n{code}"
    );
    for leg in [
        "SignalArrived(SignalInput)",
        "SemaWriteCompleted(SemaWriteOutput)",
        "SemaReadCompleted(SemaReadOutput)",
        "EffectCompleted(EffectOutcome)",
    ] {
        assert!(code.contains(leg), "Input carries leg {leg}:\n{code}");
    }

    // The Output root expands to a concrete enum; the Continuation leg re-aims
    // at the SIBLING Input enum by name rather than re-expanding inline.
    assert!(
        code.contains("pub enum Output {"),
        "spirit Output root is a concrete enum body:\n{code}"
    );
    for leg in [
        "ReplyToSignal(SignalOutput)",
        "CommandSemaWrite(SemaWriteSet)",
        "CommandSemaRead(SemaReadInput)",
        "CommandEffect(EffectCommand)",
        "Continue(Input)",
    ] {
        assert!(code.contains(leg), "Output carries leg {leg}:\n{code}");
    }

    // The concrete enums flow through the concrete-enum emitters: auto-emitted
    // constructors and `From` impls appear.
    assert!(
        code.contains("impl Input {") && code.contains("pub fn signal_arrived(payload"),
        "Input gains schema-emitted constructors:\n{code}"
    );
    assert!(
        code.contains("impl From<SignalInput> for Input"),
        "Input gains payload From impls:\n{code}"
    );
    assert!(
        code.contains("impl From<Input> for Output"),
        "Output's Continue leg gains a From<Input> impl:\n{code}"
    );

    // Spirit's payload enums still emit.
    assert!(code.contains("pub enum SemaWriteSet"));
    assert!(code.contains("pub enum EffectCommand"));
    assert!(code.contains("pub enum EffectOutcome"));

    // No into_next_step shim, no per-component concrete frame copy.
    assert!(
        !code.contains("pub enum NexusWork") && !code.contains("pub enum NexusAction"),
        "no concrete NexusWork / NexusAction enum body:\n{code}"
    );
    assert!(
        !code.contains("into_next_step"),
        "no into_next_step shim:\n{code}"
    );
}

#[test]
fn spirit_expands_named_nexus_frame_applications_to_concrete_enums() {
    let code = emit_named_spirit_nexus();

    assert!(
        code.contains("pub enum NexusWork {"),
        "named NexusWork frame application expands to a concrete enum:\n{code}"
    );
    for leg in [
        "SignalArrived(SignalInput)",
        "SemaWriteCompleted(SemaWriteOutput)",
        "SemaReadCompleted(SemaReadOutput)",
        "EffectCompleted(NexusEffectResult)",
    ] {
        assert!(code.contains(leg), "NexusWork carries leg {leg}:\n{code}");
    }
    assert!(
        code.contains("pub enum NexusAction {"),
        "named NexusAction frame application expands to a concrete enum:\n{code}"
    );
    for leg in [
        "ReplyToSignal(SignalOutput)",
        "CommandSemaWrite(SemaWriteInput)",
        "CommandSemaRead(SemaReadInput)",
        "CommandEffect(NexusEffectCommand)",
        "Continue(NexusWork)",
    ] {
        assert!(code.contains(leg), "NexusAction carries leg {leg}:\n{code}");
    }
    assert!(
        !code.contains("pub struct NexusWork") && !code.contains("pub struct NexusAction"),
        "named frame applications must not emit wrapper newtypes:\n{code}"
    );
    assert!(
        code.contains("impl triad_runtime::NexusWork for NexusWork {}"),
        "expanded NexusWork remains the runtime work type:\n{code}"
    );
    assert!(
        code.contains("impl triad_runtime::NexusAction for NexusAction"),
        "expanded NexusAction remains the runtime action type:\n{code}"
    );
}

#[test]
fn write_spirit_nexus_fixture() {
    if std::env::var_os("SCHEMA_RUST_UPDATE_FIXTURES").is_none() {
        return;
    }
    let code = emit_spirit_nexus();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/spirit_nexus_generated.rs");
    std::fs::write(path, code).expect("write spirit nexus fixture");
}

/// A local stand-in for the `reaction` frame crate at the exact module path
/// the spirit emission's `pub use reaction::schema::reaction::{Work, Action}`
/// import lines reference. With the roots now expanded into concrete enums the
/// frame enums are no longer load-bearing for the root types, but the emitter
/// still re-exports the resolved imports, so the stand-in keeps them defined.
#[allow(dead_code, unused_imports)]
mod reaction {
    pub mod schema {
        pub mod reaction {
            #[cfg_attr(feature = "nota-text", derive(nota::NotaDecode, nota::NotaEncode))]
            #[derive(
                rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq,
            )]
            pub enum Work<Event, Write, Read, Effect> {
                SignalArrived(Event),
                SemaWriteCompleted(Write),
                SemaReadCompleted(Read),
                EffectCompleted(Effect),
            }

            #[cfg_attr(feature = "nota-text", derive(nota::NotaDecode, nota::NotaEncode))]
            #[derive(
                rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq,
            )]
            pub enum Action<Reply, Write, Read, Effect, Continuation> {
                ReplyToSignal(Reply),
                CommandSemaWrite(Write),
                CommandSemaRead(Read),
                CommandEffect(Effect),
                Continue(Continuation),
            }
        }
    }
}

#[allow(dead_code, unused_imports)]
mod spirit_nexus_generated {
    use crate::reaction;
    include!("fixtures/spirit_nexus_generated.rs");
}

#[test]
fn generated_input_round_trips_through_rkyv_via_emitted_constructor() {
    use spirit_nexus_generated::{Input, SignalInput};

    // Construct through the EMITTED constructor (signal_arrived), proving the
    // concrete expanded enum gained its schema-emitted construction surface.
    let arrived: Input = Input::signal_arrived("intent recorded".to_owned());
    assert_eq!(
        arrived,
        Input::SignalArrived(SignalInput::new("intent recorded"))
    );

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&arrived).expect("input archives");
    let restored = rkyv::from_bytes::<Input, rkyv::rancor::Error>(&bytes)
        .expect("input round-trips from rkyv bytes");
    assert_eq!(arrived, restored);

    let write_done: Input = Input::sema_write_completed(true);
    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&write_done).expect("write-completed archives");
    let restored = rkyv::from_bytes::<Input, rkyv::rancor::Error>(&bytes)
        .expect("write-completed round-trips");
    assert_eq!(write_done, restored);
}

#[test]
fn generated_output_recursive_continue_round_trips_through_rkyv() {
    use spirit_nexus_generated::{Input, Output, SignalInput};

    // The recursive `Continue(Input)` leg: Output embeds the sibling Input
    // enum. Construct through the emitted `r#continue` constructor and the
    // `From<Input> for Output` impl, then rkyv round-trip the nested value.
    let inner: Input = Input::SignalArrived(SignalInput::new("continue here"));
    let next: Output = Output::r#continue(inner.clone());
    assert_eq!(next, Output::Continue(inner.clone()));
    assert_eq!(Output::from(inner), next);

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&next).expect("output continue archives");
    let restored = rkyv::from_bytes::<Output, rkyv::rancor::Error>(&bytes)
        .expect("output continue round-trips from rkyv bytes");
    assert_eq!(next, restored);
}

#[cfg(feature = "nota-text")]
#[test]
fn generated_input_output_round_trip_through_nota() {
    use nota::{NotaEncode, NotaSource};
    use spirit_nexus_generated::{Input, Output, SignalInput};

    // Input round-trips through NOTA text via the derived NotaEncode/NotaDecode
    // the concrete expanded enum carries.
    let arrived: Input = Input::signal_arrived("recorded".to_owned());
    let rendered = arrived.to_nota();
    let parsed = NotaSource::new(&rendered)
        .parse::<Input>()
        .expect("input parses back from NOTA");
    assert_eq!(arrived, parsed);

    // The recursive Output::Continue(Input) leg round-trips through NOTA too.
    let next: Output = Output::Continue(Input::SignalArrived(SignalInput::new("again")));
    let rendered = next.to_nota();
    let parsed = NotaSource::new(&rendered)
        .parse::<Output>()
        .expect("output continue parses back from NOTA");
    assert_eq!(next, parsed);
}
