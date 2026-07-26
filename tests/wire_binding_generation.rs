use protos::WireContractFamily;
use schema_rust::{
    RustEmissionOptions, RustEmissionTarget, RustEmitter, RustGenerationError,
    build::{GenerationPlan, ModuleEmission},
};

mod support;

use support::{FixtureSchema, FixtureSchemaDirectory};

#[allow(dead_code)]
#[path = "fixtures/spirit_wire_generated.rs"]
mod generated;

fn wire_source(family: WireContractFamily) -> String {
    RustEmitter::new(RustEmissionOptions::wire_contract(family))
        .emit_code_from_true_schema(&FixtureSchema::new("plane-triad.schema").lower("spirit:lib"))
        .expect("bound wire contract emits")
        .as_str()
        .to_owned()
}

fn spirit_wire_source() -> String {
    RustEmitter::new(RustEmissionOptions::wire_contract(
        WireContractFamily::SignalSpirit,
    ))
    .emit_code_from_true_schema(&FixtureSchema::new("spirit-min.schema").lower("spirit:lib"))
    .expect("bound Spirit fixture emits")
    .as_str()
    .to_owned()
}

fn exchange() -> signal_frame::ExchangeIdentifier {
    signal_frame::ExchangeIdentifier::new(
        signal_frame::SessionEpoch::new(9),
        signal_frame::ExchangeLane::Connector,
        signal_frame::LaneSequence::new(3),
    )
}

fn record_input() -> generated::Input {
    generated::Input::record(generated::Entry {
        topics: generated::Topics::new(vec![generated::Topic::new("schema")]),
        kind: generated::Kind::Decision,
        description: generated::Description::new("bound request"),
        magnitude: generated::Magnitude::High,
    })
}

#[test]
fn canonical_families_emit_exact_distinct_marker_bindings() {
    let ordinary = wire_source(WireContractFamily::SignalSpirit);
    let meta = wire_source(WireContractFamily::MetaSignalSpirit);
    let judge = wire_source(WireContractFamily::SignalSpiritJudge);

    for (source, contract_id) in [(&ordinary, 1_u32), (&meta, 2_u32), (&judge, 3_u32)] {
        assert!(source.contains("pub enum ContractMarker {}"));
        assert!(source.contains("impl signal_frame::WireContract for ContractMarker"));
        assert!(source.contains(&format!(
            "signal_frame::ContractId::try_new({contract_id}u32)"
        )));
        assert!(source.contains("signal_frame::WireRevision::try_new(1u16)"));
        assert!(!source.contains("protos::"));
        assert!(!source.contains("ExchangeFrame<Input, Output>"));
        assert!(!source.contains("pub fn encode_signal_frame"));
    }
    assert_ne!(ordinary, meta);
    assert_ne!(ordinary, judge);
    assert_ne!(meta, judge);
    assert!(judge.contains("pub const INPUT_RECORD: u64 = 0x0000000100000003"));
    assert!(judge.contains("pub const OUTPUT_RECORD_ACCEPTED: u64 = 0x0100000100000003"));
    assert!(judge.contains("pub const HANDSHAKE_REQUEST: u64 = 0xFF00000100000003"));
    assert!(judge.contains("pub const HANDSHAKE_REPLY: u64 = 0xFF01000100000003"));
    assert!(judge.contains("pub const ENGINE_REFUSAL: u64 = 0xFF02000100000003"));
    assert!(judge.contains(
        "pub type Frame = signal_frame::BoundExchangeFrame<ContractMarker, Input, Output>;"
    ));
}

#[test]
fn every_wire_route_packs_binding_and_root_variant_in_the_exact_bits() {
    let source = wire_source(WireContractFamily::SignalSpirit);

    assert!(source.contains("pub const INPUT_RECORD: u64 = 0x0000000100000001"));
    assert!(source.contains("pub const OUTPUT_RECORD_ACCEPTED: u64 = 0x0100000100000001"));
    assert!(source.contains("pub const HANDSHAKE_REQUEST: u64 = 0xFF00000100000001"));
    assert!(source.contains("pub const HANDSHAKE_REPLY: u64 = 0xFF01000100000001"));
    assert!(source.contains("pub const ENGINE_REFUSAL: u64 = 0xFF02000100000001"));
    assert!(source.contains(
        "pub type Frame = signal_frame::BoundExchangeFrame<ContractMarker, Input, Output>;"
    ));
    assert!(
        source.contains("pub type FrameBody = signal_frame::ExchangeFrameBody<Input, Output>;")
    );
}

#[test]
fn generated_decoder_orders_binding_route_and_archive_validation() {
    let source = wire_source(WireContractFamily::SignalSpirit);
    let contract_check = source.find("ContractMismatch").expect("contract mismatch");
    let revision_check = source
        .find("UnsupportedWireRevision")
        .expect("revision mismatch");
    let route_check = source.find("UnknownRoute").expect("route mismatch");
    let archive_decode = source
        .find("Frame::decode(bytes)")
        .expect("bound archive decode");

    assert!(contract_check < route_check);
    assert!(revision_check < route_check);
    assert!(route_check < archive_decode);
    assert!(source.contains("LegacyHeader"));
    assert!(source.contains("RouteBodyMismatch"));
    assert!(source.contains("ArchiveDecode"));
    assert!(source.contains("EngineRefused"));
}

#[test]
fn checked_wire_contract_fixture_is_fresh_and_compiles() {
    assert_eq!(
        spirit_wire_source(),
        include_str!("fixtures/spirit_wire_generated.rs")
    );
}

#[test]
fn generated_bound_request_and_reply_round_trip_with_exchange_identity() {
    let input = record_input();
    let encoded = input
        .clone()
        .encode_request_frame(exchange())
        .expect("bound request encodes");
    let (decoded_exchange, decoded_input) =
        generated::ContractMarker::decode_single_request(&encoded).expect("bound request decodes");
    assert_eq!(decoded_exchange, exchange());
    assert_eq!(decoded_input, input);
    assert_eq!(
        u64::from_le_bytes(encoded[..8].try_into().expect("short header")),
        generated::short_header::INPUT_RECORD
    );

    let output = generated::Output::record_accepted(17);
    let encoded = output
        .clone()
        .encode_reply_frame(exchange())
        .expect("bound reply encodes");
    let decoded = generated::ContractMarker::decode_frame(&encoded).expect("bound reply decodes");
    assert_eq!(
        decoded.into_body(),
        generated::FrameBody::Reply {
            exchange: exchange(),
            reply: signal_frame::Reply::committed(signal_frame::NonEmpty::single(
                signal_frame::SubReply::Ok(output),
            )),
        }
    );
}

#[test]
fn generated_decoder_rejects_binding_and_route_before_archive_then_checks_body_route() {
    let encoded = record_input()
        .encode_request_frame(exchange())
        .expect("bound request encodes");

    let mut wrong_contract = encoded.clone();
    wrong_contract[..4].copy_from_slice(&2_u32.to_le_bytes());
    wrong_contract.truncate(8);
    assert!(matches!(
        generated::ContractMarker::decode_frame(&wrong_contract),
        Err(generated::SignalFrameError::ContractMismatch {
            expected: 1,
            found: 2,
        })
    ));

    let mut wrong_revision = encoded.clone();
    wrong_revision[4..6].copy_from_slice(&2_u16.to_le_bytes());
    wrong_revision.truncate(8);
    assert!(matches!(
        generated::ContractMarker::decode_frame(&wrong_revision),
        Err(generated::SignalFrameError::UnsupportedWireRevision {
            contract_id: 1,
            expected: 1,
            found: 2,
        })
    ));

    let mut unknown_route = encoded.clone();
    unknown_route[6] = 99;
    unknown_route.truncate(8);
    assert!(matches!(
        generated::ContractMarker::decode_frame(&unknown_route),
        Err(generated::SignalFrameError::UnknownRoute {
            root: 0,
            variant: 99,
        })
    ));

    let mut mismatched_route = encoded;
    mismatched_route[6] = 1;
    assert!(matches!(
        generated::ContractMarker::decode_frame(&mismatched_route),
        Err(generated::SignalFrameError::RouteBodyMismatch {
            root: 0,
            variant: 1,
            body: "request",
        })
    ));
}

#[test]
fn generated_single_request_boundary_rejects_handshakes_replies_and_batches() {
    let handshake = generated::ContractMarker::handshake_request_frame(
        signal_frame::HandshakeRequest::current(),
    )
    .encode()
    .expect("handshake encodes");
    assert!(matches!(
        generated::ContractMarker::decode_single_request(&handshake),
        Err(generated::SignalFrameError::UnexpectedFrameBody {
            found: "handshake request",
        })
    ));

    let handshake = generated::ContractMarker::handshake_reply_frame(
        signal_frame::HandshakeReply::Accepted(signal_frame::SIGNAL_FRAME_PROTOCOL_VERSION),
    )
    .encode()
    .expect("handshake reply encodes");
    assert!(matches!(
        generated::ContractMarker::decode_single_request(&handshake),
        Err(generated::SignalFrameError::UnexpectedFrameBody {
            found: "handshake reply",
        })
    ));

    let reply = generated::Output::record_accepted(17)
        .encode_reply_frame(exchange())
        .expect("reply encodes");
    assert!(matches!(
        generated::ContractMarker::decode_single_request(&reply),
        Err(generated::SignalFrameError::UnexpectedFrameBody { found: "reply" })
    ));

    let mut builder = generated::RequestBuilder::new();
    builder.push(record_input());
    builder.push(generated::Input::observe(generated::Query {
        topic: generated::Topic::new("schema"),
        kind: generated::Kind::Decision,
    }));
    let request = builder.build().expect("two-operation request");
    let frame = generated::Frame::new(
        signal_frame::WireRoute::new(
            signal_frame::RootCode::new(0),
            signal_frame::VariantCode::new(0),
        ),
        generated::FrameBody::Request {
            exchange: exchange(),
            request,
        },
    )
    .encode()
    .expect("batch frame encodes");
    assert!(matches!(
        generated::ContractMarker::decode_single_request(&frame),
        Err(generated::SignalFrameError::OperationCount { found: 2 })
    ));
}

#[test]
fn generated_engine_refusal_keeps_the_bound_contract_header() {
    let refusal = generated::EngineRefusal::unavailable("engine stopped".to_owned());
    let frame = refusal.encode_bound_frame().expect("refusal encodes");
    assert_eq!(
        u64::from_le_bytes(frame[..8].try_into().expect("short header")),
        generated::short_header::ENGINE_REFUSAL
    );
    assert!(matches!(
        generated::ContractMarker::decode_frame(&frame),
        Err(generated::SignalFrameError::EngineRefused { refusal: found })
            if found == refusal
    ));
}

#[test]
fn missing_binding_is_a_typed_error_only_for_wire_codec_targets() {
    let schema = FixtureSchema::new("plane-triad.schema").lower("spirit:lib");
    let missing = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::WireContract),
    )
    .emit_code_from_true_schema(&schema);
    assert_eq!(missing, Err(RustGenerationError::MissingWireContractFamily));

    RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::NexusRuntime),
    )
    .emit_code_from_true_schema(&schema)
    .expect("non-wire schema needs no binding");
}

#[test]
fn plan_and_module_carry_typed_family_in_both_declaration_orders() {
    let fixture = FixtureSchemaDirectory::new("driver-contract");
    let ordinary = ModuleEmission::wire_contract(WireContractFamily::SignalSpirit);
    let meta = ModuleEmission::wire_contract(WireContractFamily::MetaSignalSpirit);
    let judge = ModuleEmission::wire_contract(WireContractFamily::SignalSpiritJudge);
    let plan = GenerationPlan::new(fixture.crate_root(), "driver-contract", "0.1.0")
        .with_module(meta.clone())
        .with_module(judge.clone())
        .with_module(ordinary.clone());
    assert_eq!(plan.modules(), &[meta, judge, ordinary]);
}

#[test]
fn schema_without_wire_codec_is_unchanged_by_family_capability() {
    let source = RustEmitter::new(
        RustEmissionOptions::binary_only().with_target(RustEmissionTarget::NexusRuntime),
    )
    .emit_code_from_true_schema(
        &FixtureSchema::new("plane-triad.schema").lower("driver-runtime:nexus"),
    )
    .expect("internal schema generates without a family");
    assert!(!source.as_str().contains("ContractMarker"));
}

#[test]
fn caller_cannot_construct_an_arbitrary_wire_binding() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/wire_binding/arbitrary_numeric_ids.rs");
}
