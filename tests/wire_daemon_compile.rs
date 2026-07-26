use schema_rust::{DaemonModule, NexusDaemonShape, WorkingListenerTier};

#[allow(dead_code)]
mod schema {
    pub mod signal {
        include!("fixtures/spirit_wire_generated.rs");
    }

    pub mod daemon {
        include!("fixtures/spirit_daemon_generated.rs");
    }
}

#[test]
fn checked_bound_wire_daemon_fixture_is_fresh_and_compiles() {
    let generated = DaemonModule::new(
        NexusDaemonShape::new("spirit", WorkingListenerTier::new("signal")),
        "schema-rust",
    )
    .to_generated_file();
    assert_eq!(
        generated.code.as_str(),
        include_str!("fixtures/spirit_daemon_generated.rs")
    );
}

#[test]
fn daemon_fixture_uses_only_bound_contract_operations() {
    let source = include_str!("fixtures/spirit_daemon_generated.rs");
    assert!(source.contains("ContractMarker::decode_single_request(&frame)"));
    assert!(source.contains("output.encode_reply_frame(exchange)"));
    assert!(source.contains("refusal.encode_bound_frame()"));
    assert!(!source.contains("decode_signal_frame"));
    assert!(!source.contains("encode_signal_frame"));
    assert!(!source.contains("signal_frame::ExchangeFrame"));
}
