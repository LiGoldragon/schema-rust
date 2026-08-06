use std::{collections::BTreeMap, fs};

use core_ethos::bootstrap::{
    BootstrapCatalog, BootstrapGrammarIdentities, BootstrapPriorIdentities,
    BootstrapPriorVocabulary, BootstrapVersionPolicy, CanonicalIdentityOrder, EthosVersion,
    IdentitySchema, IdentitySchemaCatalog, InterfaceRole, NomosSchema, SchemaRole,
    TextualMetadataRecord, TextualMetadataSnapshot, TextualProjectionAddress,
};
use core_nomos::BootstrapSliceOneLoweringError;
use name_table::{LocalEncodedId, Name};
use rust_logos::{
    FixtureRustVocabulary, FixtureRustVocabularyIds, RustLogos, RustTypePath, RustTypePathResolver,
};
use schema_rust::{
    bootstrap::{
        BOOTSTRAP_INTERFACE_GENERATED_MARKER, BootstrapInterfaceGeneration,
        BootstrapInterfaceGenerationError,
    },
    build::BuildError,
};
use sema_translator::bootstrap::{
    AuthorizedBootstrapTransition, BootstrapAssemblyError, BootstrapAuthorityIdentity,
    BootstrapAuthorityRevision, BootstrapTransactionAssembler, VerifiedBootstrapAssembly,
};
use signal_sema_translator::{VocabularyEncodedId, VocabularyRoot};
use structural_codec::EncodedNameResolver;

const INTERFACE_SOURCE: &str =
    "Interface.{1 0 0}\n[]\n{[] [] [] [Thing.String Choice.[None Pair.{String Integer}]]}";

fn id(local: u16) -> VocabularyEncodedId {
    VocabularyEncodedId::new(VocabularyRoot::Universal, vec![LocalEncodedId::new(local)])
        .expect("test authority identity is nonempty")
}

fn rust_id(local: u16) -> VocabularyEncodedId {
    VocabularyEncodedId::new(VocabularyRoot::Rust, vec![LocalEncodedId::new(local)])
        .expect("test Rust vocabulary identity is nonempty")
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

fn interface_assembly() -> VerifiedBootstrapAssembly {
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

#[derive(Default)]
struct Names(BTreeMap<VocabularyEncodedId, Name>);

impl Names {
    fn insert(&mut self, identity: VocabularyEncodedId, spelling: &str) {
        self.0.insert(identity, Name::new(spelling));
    }
}

impl EncodedNameResolver<VocabularyRoot> for Names {
    fn resolve(&self, encoded_id: &VocabularyEncodedId) -> Option<&Name> {
        self.0.get(encoded_id)
    }
}

fn rust_logos() -> RustLogos {
    let ids = FixtureRustVocabularyIds::new(
        rust_id(10),
        rust_id(11),
        rust_id(12),
        rust_id(13),
        rust_id(14),
        rust_id(1),
        rust_id(2),
        rust_id(3),
        rust_id(4),
        rust_id(5),
    );
    let mut names = Names::default();
    for (identity, spelling) in [
        (rust_id(10), "NewtypeItemRecord"),
        (rust_id(11), "EnumerationItemRecord"),
        (rust_id(12), "VariantRecord"),
        (rust_id(13), "TupleFieldRecord"),
        (rust_id(14), "TypeReferenceRecord"),
        (rust_id(1), "struct"),
        (rust_id(2), "enum"),
        (rust_id(3), "pub"),
        (rust_id(4), ","),
        (rust_id(5), ";"),
    ] {
        names.insert(identity, spelling);
    }
    RustLogos::new(
        FixtureRustVocabulary::seal(ids, &names).expect("sealed caller-owned Rust vocabulary"),
    )
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
        .with(id(7), &["std", "string", "String"])
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
        generated.rust().content().contains("std::string::String"),
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
fn obsolete_six_slot_source_and_non_interface_transactions_never_enter_generation() {
    let catalog = base_catalog();
    let approved = approval(
        &catalog,
        [record(&["app"], None, "Thing", id(100))],
        [id(100)],
    );
    assert!(matches!(
        assembler(catalog.clone()).assemble("{}\n[]\n[]\n{}\n{}\n{}", approved),
        Err(BootstrapAssemblyError::Read(_))
    ));

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
fn production_bootstrap_lane_has_no_legacy_schema_reconstruction_or_identity_invention() {
    let source = include_str!("../src/bootstrap.rs");
    for forbidden in [
        "schema_language::",
        "SchemaEngine::",
        "SchemaSource::",
        "TrueSchema::",
        "LocalEncodedId",
    ] {
        assert!(
            !source.contains(forbidden),
            "strict bootstrap generation must not contain {forbidden}"
        );
    }
}
