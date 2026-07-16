use schema_language::{ImportResolver, MacroContext, SchemaEngine, SchemaIdentity};
use schema_rust::{RustEmissionOptions, RustEmitter};

mod support;

use support::FixtureSchemaDirectory;

mod signal_lojix {
    #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
    pub enum Input {
        Send,
    }

    #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
    pub enum Output {
        Sent,
    }
}

mod meta_signal_lojix {
    #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
    pub enum Input {
        Authorize,
    }

    #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, PartialEq, Eq)]
    pub enum Output {
        Authorized,
    }
}

#[allow(dead_code, unused_imports)]
mod compiled_generated {
    use crate::{meta_signal_lojix, signal_lojix};

    include!("fixtures/qualified_contract_roots_generated.rs");
}

fn emit_consumer() -> String {
    let signal = FixtureSchemaDirectory::new("contract-roots/signal-lojix");
    let meta = FixtureSchemaDirectory::new("contract-roots/meta-signal-lojix");
    let consumer = FixtureSchemaDirectory::new("contract-roots/consumer");
    let resolver = ImportResolver::new()
        .with_dependency("signal-lojix", signal.path(), "1.2.3")
        .with_dependency("meta-signal-lojix", meta.path(), "4.5.6");
    let schema = SchemaEngine::default()
        .lower_source_with_resolver(
            &consumer.schema("lib.schema").read(),
            SchemaIdentity::new("qualified-consumer:lib", "0.3.0"),
            &mut MacroContext::default(),
            &resolver,
        )
        .expect("consumer resolves exact package-qualified contract roots");
    RustEmitter::new(RustEmissionOptions::binary_only())
        .emit_code_from_true_schema(&schema)
        .as_str()
        .to_owned()
}

#[test]
fn two_same_named_contract_roots_emit_as_distinct_package_paths() {
    let code = emit_consumer();
    assert!(code.contains("pub input: signal_lojix::Input,"), "{code}");
    assert!(
        code.contains("pub output: meta_signal_lojix::Output,"),
        "{code}"
    );
    assert!(
        !code.contains("as Input;"),
        "external roots must not be flattened: {code}"
    );
    assert!(
        !code.contains("as Output;"),
        "external roots must not be flattened: {code}"
    );
    syn::parse_file(&code).expect("package-qualified generated source parses as Rust");
}

#[test]
fn package_qualified_contract_root_emission_is_deterministic() {
    let emitted = emit_consumer();
    assert_eq!(emitted, emit_consumer());
    let fixture = std::fs::read_to_string("tests/fixtures/qualified_contract_roots_generated.rs")
        .expect("read qualified-root generated fixture");
    assert_eq!(
        emitted, fixture,
        "checked-in generated contract stays fresh"
    );
}

#[test]
fn generated_qualified_root_contract_compiles_and_round_trips_both_dependencies() {
    let bridge = compiled_generated::Bridge {
        input: signal_lojix::Input::Send,
        output: meta_signal_lojix::Output::Authorized,
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&bridge).expect("archive bridge");
    let recovered = rkyv::from_bytes::<compiled_generated::Bridge, rkyv::rancor::Error>(&bytes)
        .expect("decode bridge");
    assert_eq!(recovered, bridge);
}
