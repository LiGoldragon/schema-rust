//! Proof for the shared generic reaction frame emission (designer slice).
//!
//! Part 1: `reaction.schema` — the parameterized frame — emits the two
//! generic data enums `Work<Event, Write, Read, Effect>` and
//! `Action<Reply, Write, Read, Effect, Continuation>` with the proven derive
//! stack, and nothing else (no runtime planes, no wire codec, no constructor
//! impls). The emitted module is written to `tests/fixtures/reaction_frame_generated.rs`,
//! `include!`d below, and a `Work<concrete>` value round-trips through rkyv.

use schema_rust::{RustEmissionOptions, RustEmissionTarget, RustEmitter};

mod support;

use support::FixtureSchema;

fn emit_reaction_frame() -> String {
    let schema = FixtureSchema::new("reaction/schema/reaction.schema").lower("reaction:reaction");
    let options = RustEmissionOptions::feature_gated_nota("nota-text")
        .with_target(RustEmissionTarget::DeclarationModule);
    RustEmitter::new(options)
        .emit_code_from_true_schema(&schema)
        .expect("schema emits")
        .as_str()
        .to_owned()
}

fn assert_contains(code: &str, expected: &str) {
    let compact = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace() && *character != ',')
            .collect::<String>()
    };
    assert!(
        compact(code).contains(&compact(expected)),
        "generated frame must contain {expected:?}\n--- generated ---\n{code}"
    );
}

#[test]
fn reaction_frame_emits_the_two_generic_data_enums() {
    let code = emit_reaction_frame();

    // The Work enum: four direct type parameters, the proven derive stack.
    assert_contains(
        &code,
        "derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode)",
    );
    assert_contains(
        &code,
        "#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]",
    );
    assert_contains(
        &code,
        "pub enum Work<Event, WriteDone, ReadDone, EffectDone>",
    );
    assert_contains(&code, "SignalArrived(Event)");
    assert_contains(&code, "SemaWriteCompleted(WriteDone)");
    assert_contains(&code, "SemaReadCompleted(ReadDone)");
    assert_contains(&code, "EffectCompleted(EffectDone)");

    // The Action enum: five direct type parameters, the same derive stack.
    assert_contains(
        &code,
        "pub enum Action<Reply, Write, Read, Effect, Continuation>",
    );
    assert_contains(&code, "ReplyToSignal(Reply)");
    assert_contains(&code, "CommandSemaWrite(Write)");
    assert_contains(&code, "CommandSemaRead(Read)");
    assert_contains(&code, "CommandEffect(Effect)");
    assert_contains(&code, "Continue(Continuation)");
}

#[test]
fn reaction_frame_emits_no_bound_attributes_or_runtime_support() {
    let code = emit_reaction_frame();

    // rkyv 0.8 + nota auto-synthesise per-parameter bounds: no
    // omit_bounds, no archive bound attributes, no explicit where.
    assert!(
        !code.contains("omit_bounds"),
        "frame needs no omit_bounds:\n{code}"
    );
    assert!(
        !code.contains("#[rkyv(") && !code.contains("#[archive("),
        "frame needs no rkyv/archive bound attributes:\n{code}"
    );
    // No empty wire root enums (the disproven zero-variant shape), no
    // runtime planes, no signal-frame codec, no hand-emitted constructors.
    assert!(
        !code.contains("pub enum Input") && !code.contains("pub enum Output"),
        "declaration-only frame emits no wire root enums:\n{code}"
    );
    assert!(
        !code.contains("impl Work") && !code.contains("impl Action"),
        "frame emits the DATA enums only — no inherent impls:\n{code}"
    );
    assert!(
        !code.contains("encode_signal_frame") && !code.contains("pub mod signal"),
        "declaration-only frame emits no wire codec / runtime planes:\n{code}"
    );
}

#[test]
fn write_reaction_frame_fixture() {
    // Materialise the emitted frame as the compiled fixture below. Guarded so
    // an accidental run does not silently rewrite the checked-in artifact; the
    // committed fixture is what `reaction_frame_generated` compiles.
    if std::env::var_os("SCHEMA_RUST_UPDATE_FIXTURES").is_none() {
        return;
    }
    let code = emit_reaction_frame();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reaction_frame_generated.rs");
    std::fs::write(path, code).expect("write reaction frame fixture");
}

#[allow(dead_code, unused_imports)]
mod reaction_frame_generated {
    include!("fixtures/reaction_frame_generated.rs");
}

#[test]
fn generated_work_round_trips_through_rkyv() {
    use reaction_frame_generated::Work;

    // A fully concrete Work instantiation — the four legs bound to plain
    // scalars — archives and reads back, proving the bare derive stack
    // composes over the multi-parameter generic enum (the gate's headline).
    type ConcreteWork = Work<String, u64, bool, i32>;
    let value: ConcreteWork = Work::SignalArrived("arrived".to_owned());

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).expect("work archives");
    let restored = rkyv::from_bytes::<ConcreteWork, rkyv::rancor::Error>(&bytes)
        .expect("work round-trips from rkyv bytes");
    assert_eq!(value, restored);

    let completed: ConcreteWork = Work::SemaWriteCompleted(7);
    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&completed).expect("write-completed archives");
    let restored = rkyv::from_bytes::<ConcreteWork, rkyv::rancor::Error>(&bytes)
        .expect("write-completed round-trips");
    assert_eq!(completed, restored);
}
