use std::{collections::BTreeMap, fs, process::Command};

use core_ethos::bootstrap::{
    BootstrapCatalog, BootstrapGrammarIdentities, BootstrapPriorIdentities,
    BootstrapPriorVocabulary, BootstrapVersionPolicy, CanonicalIdentityOrder, EthosVersion,
    IdentitySchema, IdentitySchemaCatalog, InterfaceRole, NomosSchema, SchemaRole,
    TextualMetadataRecord, TextualMetadataSnapshot, TextualProjectionAddress,
};
use core_nomos::{
    BootstrapSliceOneLoweringError, ExternalStorageProvenance, StorageProvenanceOwner,
};
use name_table::LocalEncodedId;
use rust_logos::{RustEncodedIdCodec, RustLogos, RustTypePath, RustTypePathResolver};
use schema_rust::{
    bootstrap::{
        BOOTSTRAP_INTERFACE_GENERATED_MARKER, BOOTSTRAP_SEMA_GENERATED_MARKER,
        BootstrapInterfaceGeneration, BootstrapInterfaceGenerationError, BootstrapSemaGeneration,
        BootstrapSemaGenerationError,
    },
    build::BuildError,
};
use sema_translator::bootstrap::{
    AuthorizedBootstrap, AuthorizedBootstrapTransition, BootstrapAuthorityIdentity,
    BootstrapAuthorityRevision, BootstrapTransactionAssembler, SealedRustVocabulary,
};
use signal_sema_translator::{VocabularyEncodedId, VocabularyRoot};

const INTERFACE_SOURCE: &str =
    "Interface.{1 0 0}\n[]\n{[] [] [] [Thing.String Choice.[None Pair.{String Integer}]]}";
const SEMA_SOURCE: &str = "Sema.{1 0 0}\n[]\n{[Domain.String StoredRecord.{Domain Integer}] [records.{StoredRecord Domain}]}";
const MANIFEST: &str = include_str!("../Cargo.toml");

fn id(local: u16) -> VocabularyEncodedId {
    VocabularyEncodedId::new(VocabularyRoot::Universal, vec![LocalEncodedId::new(local)])
        .expect("test authority identity is nonempty")
}

fn record(
    module: &[&str],
    owner: Option<VocabularyEncodedId>,
    spelling: &str,
    identity: VocabularyEncodedId,
) -> TextualMetadataRecord {
    TextualMetadataRecord {
        address: TextualProjectionAddress {
            module_path: module.iter().map(|part| (*part).to_owned()).collect(),
            lexical_owner: owner,
            visible_name: spelling.to_owned(),
        },
        encoded_name: identity,
    }
}

fn prior_identities() -> BootstrapPriorIdentities {
    BootstrapPriorIdentities {
        interface_kind: id(1),
        nexus_kind: id(2),
        sema_kind: id(3),
        input_role: id(4),
        output_role: id(5),
        refusal_role: id(6),
        string_type: id(7),
        integer_type: id(8),
        boolean_type: id(9),
        unit_type: id(10),
        vector_shape: id(11),
        option_shape: id(12),
        map_shape: id(13),
        result_shape: id(14),
        stream_nomos: id(15),
        stream_shape: id(15),
        stream_identity_shape: id(16),
    }
}

fn base_catalog() -> BootstrapCatalog {
    let specifications = [
        (
            1,
            "Interface",
            vec![SchemaRole::FileKind(
                core_ethos::bootstrap::EthosKind::Interface,
            )],
        ),
        (
            2,
            "Nexus",
            vec![SchemaRole::FileKind(
                core_ethos::bootstrap::EthosKind::Nexus,
            )],
        ),
        (
            3,
            "Sema",
            vec![SchemaRole::FileKind(core_ethos::bootstrap::EthosKind::Sema)],
        ),
        (
            4,
            "Input",
            vec![SchemaRole::InterfaceRole(InterfaceRole::Input)],
        ),
        (
            5,
            "Output",
            vec![SchemaRole::InterfaceRole(InterfaceRole::Output)],
        ),
        (
            6,
            "Refusal",
            vec![SchemaRole::InterfaceRole(InterfaceRole::Refusal)],
        ),
        (7, "String", vec![SchemaRole::Nominal { persistent: true }]),
        (8, "Integer", vec![SchemaRole::Nominal { persistent: true }]),
        (9, "Boolean", vec![SchemaRole::Nominal { persistent: true }]),
        (10, "Unit", vec![SchemaRole::Nominal { persistent: true }]),
        (11, "Vector", vec![SchemaRole::Shape { arity: 1 }]),
        (12, "Option", vec![SchemaRole::Shape { arity: 1 }]),
        (13, "Map", vec![SchemaRole::Shape { arity: 2 }]),
        (14, "Result", vec![SchemaRole::Shape { arity: 2 }]),
        (
            15,
            "Stream",
            vec![
                SchemaRole::Shape { arity: 1 },
                SchemaRole::Nomos(NomosSchema::StreamInitiation { arity: 2 }),
            ],
        ),
        (16, "StreamIdentity", vec![SchemaRole::Shape { arity: 1 }]),
    ];
    let metadata = TextualMetadataSnapshot::new(
        specifications
            .iter()
            .map(|(local, spelling, _)| record(&["builtin"], None, spelling, id(*local)))
            .collect(),
    )
    .expect("valid prior metadata");
    let schemas = IdentitySchemaCatalog::new(
        specifications
            .iter()
            .map(|(local, _, roles)| IdentitySchema::new(id(*local), roles.clone()).unwrap())
            .collect(),
    )
    .expect("valid prior schemas");
    let priors = BootstrapPriorVocabulary::new(prior_identities(), &schemas, &metadata)
        .expect("valid prior relationships");
    let order = CanonicalIdentityOrder::new(
        specifications
            .iter()
            .map(|(local, _, _)| (id(*local), vec![0x10, *local as u8])),
    )
    .expect("unique prior order");
    BootstrapCatalog::new(
        vec!["app".to_owned()],
        metadata,
        schemas,
        priors,
        BootstrapVersionPolicy::exact(EthosVersion::new(1, 0, 0)),
        order,
    )
    .expect("valid bootstrap catalog")
}

fn assembler(catalog: BootstrapCatalog) -> BootstrapTransactionAssembler {
    BootstrapTransactionAssembler::new(
        BootstrapAuthorityIdentity::new([0x68; 32]),
        BootstrapAuthorityRevision::new(5),
        BootstrapGrammarIdentities {
            document: id(900),
            syntax: id(901),
        },
        catalog,
    )
}

fn approval(
    catalog: &BootstrapCatalog,
    records: impl IntoIterator<Item = TextualMetadataRecord>,
    identities: impl IntoIterator<Item = VocabularyEncodedId>,
) -> AuthorizedBootstrapTransition {
    let mut after = catalog.metadata().records().to_vec();
    after.extend(records);
    AuthorizedBootstrapTransition::new(
        TextualMetadataSnapshot::new(after).expect("authority projection is exact"),
        identities
            .into_iter()
            .map(|identity| {
                let local = identity.chain()[0].value();
                (identity, vec![0x80, (local >> 8) as u8, local as u8])
            })
            .collect(),
        BTreeMap::new(),
    )
}

fn interface_assembly() -> AuthorizedBootstrap {
    let catalog = base_catalog();
    let choice = id(101);
    let approved = approval(
        &catalog,
        [
            record(&["app"], None, "Thing", id(100)),
            record(&["app"], None, "Choice", choice.clone()),
            record(&["app"], Some(choice.clone()), "None", id(102)),
            record(&["app"], Some(choice), "Pair", id(103)),
        ],
        [id(100), id(101), id(102), id(103)],
    );
    assembler(catalog)
        .assemble(INTERFACE_SOURCE, approved)
        .expect("authority-approved Interface transaction")
}

fn sema_assembly() -> AuthorizedBootstrap {
    let catalog = base_catalog();
    let approved = approval(
        &catalog,
        [
            record(&["app"], None, "Domain", id(100)),
            record(&["app"], None, "StoredRecord", id(101)),
            record(&["app"], None, "records", id(102)),
        ],
        [id(100), id(101), id(102)],
    );
    assembler(catalog)
        .assemble(SEMA_SOURCE, approved)
        .expect("authority-approved Sema transaction")
}

fn external_storage(local: u16, fingerprint: u8) -> ExternalStorageProvenance {
    ExternalStorageProvenance::new(
        id(local),
        [fingerprint; 32],
        StorageProvenanceOwner::new(
            "https://github.com/LiGoldragon/bootstrap-storage-fixture".to_owned(),
            format!("revision-{fingerprint}"),
        )
        .expect("explicit revision-bearing storage owner"),
    )
    .expect("Universal external storage identity")
}

fn rust_logos() -> RustLogos {
    RustLogos::from_authority(&SealedRustVocabulary::bootstrap())
        .expect("authority releases the bootstrap Rust vocabulary")
}

#[derive(Default)]
struct TypePaths(BTreeMap<VocabularyEncodedId, RustTypePath>);

impl TypePaths {
    fn with(mut self, identity: VocabularyEncodedId, segments: &[&str]) -> Self {
        self.0.insert(
            identity,
            RustTypePath::try_new(
                segments
                    .iter()
                    .map(|segment| (*segment).to_owned())
                    .collect(),
            )
            .expect("explicit Rust path is valid"),
        );
        self
    }
}

impl RustTypePathResolver for TypePaths {
    fn resolve_type_path(&self, encoded_id: &VocabularyEncodedId) -> Option<&RustTypePath> {
        self.0.get(encoded_id)
    }
}

#[test]
fn verified_interface_transaction_projects_canonical_source_and_rust_with_exact_freshness() {
    let assembly = interface_assembly();
    let rust = rust_logos();
    let paths = TypePaths::default()
        .with(id(7), &["fixture", "Text"])
        .with(id(8), &["u64"]);
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("domain.schema");
    let rust_path = directory.path().join("domain.rs");
    let generated =
        BootstrapInterfaceGeneration::new(&assembly, &rust, &paths, &source_path, &rust_path)
            .generate()
            .expect("strict transaction lowers and projects");

    assert_eq!(generated.source().content(), assembly.canonical_source());
    assert!(
        generated
            .source()
            .content()
            .starts_with("Interface.{1 0 0}\n[]\n")
    );
    assert!(
        generated
            .rust()
            .content()
            .starts_with(BOOTSTRAP_INTERFACE_GENERATED_MARKER)
    );
    assert!(
        generated.rust().content().contains("fixture::Text"),
        "explicit external type path was not projected:\n{}",
        generated.rust().content()
    );
    assert!(
        !generated.rust().content().contains("Thing")
            && !generated.rust().content().contains("Choice")
            && !generated.rust().content().contains("Pair"),
        "Rust projection must encode complete identities instead of reversing through spellings"
    );
    syn::parse_file(generated.rust().content()).expect("canonical projection is Rust syntax");

    let repeated =
        BootstrapInterfaceGeneration::new(&assembly, &rust, &paths, &source_path, &rust_path)
            .generate()
            .expect("same exact inputs project again");
    assert_eq!(generated, repeated);

    fs::write(&source_path, generated.source().content()).expect("seat canonical source");
    fs::write(&rust_path, generated.rust().content()).expect("seat canonical Rust");
    generated
        .assert_checked_in()
        .expect("both artifacts are fresh");

    fs::write(&source_path, "stale source").expect("make source stale");
    assert!(matches!(
        generated.assert_checked_in(),
        Err(BuildError::StaleGeneratedArtifact { path }) if path == source_path
    ));
    fs::write(&source_path, generated.source().content()).expect("restore source");
    fs::write(&rust_path, "stale Rust").expect("make Rust stale");
    assert!(matches!(
        generated.assert_checked_in(),
        Err(BuildError::StaleGeneratedArtifact { path }) if path == rust_path
    ));
}

#[test]
fn verified_sema_transaction_projects_stored_rust_and_table_with_paired_freshness() {
    let assembly = sema_assembly();
    let rust = rust_logos();
    let paths = TypePaths::default()
        .with(id(7), &["fixture", "Text"])
        .with(id(8), &["u64"]);
    let external = [external_storage(7, 7), external_storage(8, 8)];
    let directory = tempfile::tempdir().expect("temporary checked-artifact directory");
    let source_path = directory.path().join("storage.schema");
    let rust_path = directory.path().join("storage.rs");
    let generated = BootstrapSemaGeneration::new(
        &assembly,
        &rust,
        &paths,
        &external,
        &source_path,
        &rust_path,
    )
    .generate()
    .expect("strict Sema transaction lowers and projects");

    assert_eq!(generated.source().content(), assembly.canonical_source());
    assert!(
        generated
            .source()
            .content()
            .starts_with("Sema.{1 0 0}\n[]\n")
    );
    assert!(
        generated
            .rust()
            .content()
            .starts_with(BOOTSTRAP_SEMA_GENERATED_MARKER)
    );
    assert_eq!(
        generated.rust().content().matches("rkyv::Archive").count(),
        2,
        "{}",
        generated.rust().content()
    );
    for required in [
        "impl sema_engine::TableSpecification",
        "type Record =",
        "type Key =",
        "key.payload().to_string()",
    ] {
        assert!(
            generated.rust().content().contains(required),
            "strict Sema projection omitted {required:?}:\n{}",
            generated.rust().content()
        );
    }
    let table_coordinate = RustEncodedIdCodec::encode(&id(102));
    assert!(
        generated.rust().content().contains(&format!(
            "sema_engine::TableName::new(\"{table_coordinate}\")"
        )),
        "table coordinate must be the stable encoded identity:\n{}",
        generated.rust().content()
    );
    assert!(
        !generated.rust().content().contains("records"),
        "the operational table spelling belongs to canonical Sema, not persisted Rust coordinates"
    );
    syn::parse_file(generated.rust().content()).expect("canonical projection is Rust syntax");

    let repeated = BootstrapSemaGeneration::new(
        &assembly,
        &rust,
        &paths,
        &external,
        &source_path,
        &rust_path,
    )
    .generate()
    .expect("same exact Sema inputs project again");
    assert_eq!(generated, repeated);

    fs::write(&source_path, generated.source().content()).expect("seat canonical Sema source");
    fs::write(&rust_path, generated.rust().content()).expect("seat canonical Sema Rust");
    generated
        .assert_checked_in()
        .expect("both Sema artifacts are fresh");

    fs::write(&source_path, "stale Sema source").expect("make Sema source stale");
    assert!(matches!(
        generated.assert_checked_in(),
        Err(BuildError::StaleGeneratedArtifact { path }) if path == source_path
    ));
    fs::write(&source_path, generated.source().content()).expect("restore Sema source");
    fs::write(&rust_path, "stale Sema Rust").expect("make Sema Rust stale");
    assert!(matches!(
        generated.assert_checked_in(),
        Err(BuildError::StaleGeneratedArtifact { path }) if path == rust_path
    ));
}

#[test]
fn strict_sema_generation_retains_kind_storage_and_key_refusals() {
    let interface = interface_assembly();
    let sema = sema_assembly();
    let rust = rust_logos();
    let paths = TypePaths::default()
        .with(id(7), &["fixture", "Text"])
        .with(id(8), &["u64"]);
    assert!(matches!(
        BootstrapSemaGeneration::new(&interface, &rust, &paths, &[], "wrong.schema", "wrong.rs")
            .generate(),
        Err(BootstrapSemaGenerationError::WrongFileKind {
            found: core_ethos::bootstrap::EthosKind::Interface
        })
    ));
    assert!(matches!(
        BootstrapSemaGeneration::new(
            &sema,
            &rust,
            &paths,
            &[external_storage(8, 8)],
            "missing.schema",
            "missing.rs"
        )
        .generate(),
        Err(BootstrapSemaGenerationError::Lowering(
            BootstrapSliceOneLoweringError::MissingExternalStorageProvenance { identity }
        )) if identity == id(7)
    ));

    let catalog = base_catalog();
    let product_key = assembler(catalog.clone())
        .assemble(
            "Sema.{1 0 0}\n[]\n{[Key.{String Integer} StoredRecord.String] [records.{StoredRecord Key}]}",
            approval(
                &catalog,
                [
                    record(&["app"], None, "Key", id(100)),
                    record(&["app"], None, "StoredRecord", id(101)),
                    record(&["app"], None, "records", id(102)),
                ],
                [id(100), id(101), id(102)],
            ),
        )
        .expect("authority-approved Sema product key");
    let external = [external_storage(7, 7), external_storage(8, 8)];
    assert!(matches!(
        BootstrapSemaGeneration::new(
            &product_key,
            &rust,
            &paths,
            &external,
            "product.schema",
            "product.rs"
        )
        .generate(),
        Err(BootstrapSemaGenerationError::Lowering(
            BootstrapSliceOneLoweringError::SemaTableKeyNotNewtype { key, .. }
        )) if key == id(100)
    ));
}

#[test]
fn nexus_transaction_never_enters_interface_generation() {
    let catalog = base_catalog();
    let nexus = assembler(catalog.clone())
        .assemble(
            "Nexus.{1 0 0}\n[]\n{[] [Thing.String]}",
            approval(
                &catalog,
                [record(&["app"], None, "Thing", id(100))],
                [id(100)],
            ),
        )
        .expect("authority may seal a Nexus transaction");
    let rust = rust_logos();
    let paths = TypePaths::default();
    assert!(matches!(
        BootstrapInterfaceGeneration::new(&nexus, &rust, &paths, "nexus.schema", "nexus.rs")
            .generate(),
        Err(BootstrapInterfaceGenerationError::WrongFileKind {
            found: core_ethos::bootstrap::EthosKind::Nexus
        })
    ));
}

#[test]
fn nonempty_interface_roles_are_exact_nomos_refusals() {
    let catalog = base_catalog();
    let assembly = assembler(catalog.clone())
        .assemble(
            "Interface.{1 0 0}\n[]\n{[InputValue.String] [] [] []}",
            approval(
                &catalog,
                [record(&["app"], None, "InputValue", id(100))],
                [id(100)],
            ),
        )
        .expect("authority-approved Interface role transaction");
    let rust = rust_logos();
    let paths = TypePaths::default();
    assert!(matches!(
        BootstrapInterfaceGeneration::new(
            &assembly,
            &rust,
            &paths,
            "interface.schema",
            "interface.rs"
        )
        .generate(),
        Err(BootstrapInterfaceGenerationError::Lowering(
            BootstrapSliceOneLoweringError::InterfaceRole {
                role: InterfaceRole::Input,
                target,
            }
        )) if target == id(100)
    ));
}

#[test]
fn strict_bootstrap_lane_pins_one_exact_verified_producer_train() {
    for exact_dependency in [
        "core-ethos = { git = \"https://github.com/LiGoldragon/core-ethos.git\", rev = \"43b48c779c54ee9f05cbcc111d5d88074b162461\" }",
        "core-nomos = { git = \"https://github.com/LiGoldragon/core-nomos.git\", rev = \"7b60721d199551b648d42a49934a2f0ef950c595\" }",
        "rust-logos = { git = \"https://github.com/LiGoldragon/rust-logos.git\", rev = \"82e0e5c10f6efb2d53330d72ba78dd3ac695f38a\" }",
        "sema-translator = { git = \"https://github.com/LiGoldragon/sema-translator.git\", rev = \"4bd6e8fa0c3139be94a83c7ca7975bd8153eb9f5\", default-features = false, features = [\"bootstrap\"] }",
        "signal-sema-translator = { git = \"https://github.com/LiGoldragon/signal-sema-translator.git\", rev = \"3f41813dd63904c7e2b3da4382eff64ed1bf12fe\" }",
    ] {
        assert!(
            MANIFEST.contains(exact_dependency),
            "strict bootstrap manifest omitted exact producer {exact_dependency}"
        );
    }
}

#[test]
fn strict_bootstrap_normal_graph_excludes_the_sema_engine_runtime() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            "schema-rust",
            "--edges",
            "normal",
            "--no-default-features",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("cargo tree output is UTF-8");
    assert!(
        !tree.contains("sema-engine"),
        "strict bootstrap generation must not close through its runtime consumer:\n{tree}"
    );
}
