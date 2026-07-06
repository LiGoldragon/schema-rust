use schema::{ImportResolver, SchemaEnvironment};
use schema_rust::build::{
    ContractCrateBuild, DependencySchema, GenerationDriver, GenerationPlan, ModuleEmission,
};

mod support;

use support::FixtureSchemaDirectory;

fn source_contains_signature(source: &str, signature: &str) -> bool {
    let compact_source = source
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    let compact_signature = signature
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>();
    compact_source.contains(&compact_signature)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DriverFixture {
    contract: FixtureSchemaDirectory,
    runtime: FixtureSchemaDirectory,
}

impl DriverFixture {
    fn new() -> Self {
        Self {
            contract: FixtureSchemaDirectory::new("driver-contract"),
            runtime: FixtureSchemaDirectory::new("driver-runtime"),
        }
    }

    fn runtime_plan(&self) -> GenerationPlan {
        GenerationPlan::daemon_runtime(self.runtime.crate_root(), "driver-runtime", "0.1.0")
            .with_dependency_schema(DependencySchema::new(
                "driver-contract",
                self.contract.path(),
                "0.1.0",
            ))
    }

    fn generated_runtime(&self) -> schema_rust::build::GeneratedPackage {
        GenerationDriver::new(self.runtime_plan())
            .generate()
            .expect("driver emits runtime package")
    }

    fn environment(&self) -> schema::SchemaEnvironmentResult {
        let plan = self.runtime_plan();
        let resolver =
            ImportResolver::new().with_dependency("driver-contract", self.contract.path(), "0.1.0");
        SchemaEnvironment::new(plan.package().clone())
            .with_resolver(resolver)
            .load(&plan.environment_manifest())
            .expect("driver environment loads")
    }
}

#[test]
fn daemon_runtime_driver_emits_nexus_and_sema_files_with_plane_targets() {
    let generated = DriverFixture::new().generated_runtime();
    let nexus = generated
        .rust_file_named("src/schema/nexus.rs")
        .expect("nexus runtime file")
        .code
        .as_str();
    let sema = generated
        .rust_file_named("src/schema/sema.rs")
        .expect("sema runtime file")
        .code
        .as_str();

    assert!(
        nexus.contains("pub use driver_contract::schema::lib::Input as ContractInput;"),
        "nexus should import contract wire root through a Rust alias:\n{nexus}"
    );
    assert!(
        nexus.contains("pub use crate::schema::sema::SemaReadInput as SemaReadInput;"),
        "nexus should import same-package SEMA roots through crate::, not the crate name:\n{nexus}"
    );
    assert!(
        nexus.contains("pub trait NexusEngine"),
        "nexus runtime target should emit NexusEngine:\n{nexus}"
    );
    assert!(
        nexus.contains("#[cfg(feature = \"nota-text\")]\npub use nota::{"),
        "nexus runtime target should keep its NOTA surface feature-gated:\n{nexus}"
    );
    assert!(
        nexus.contains("derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode)"),
        "nexus runtime support nouns should derive NOTA only behind the feature:\n{nexus}"
    );
    assert!(
        nexus.contains("pub type NexusRunnerNextStep = triad_runtime::NextStep<")
            && nexus.contains("    ContractOutput,")
            && nexus.contains("    SemaWriteInput,")
            && nexus.contains("    SemaReadInput,")
            && nexus.contains("    std::convert::Infallible,")
            && nexus.contains("    NexusWork,"),
        "nexus runtime target should emit runner glue over imported contract output:\n{nexus}"
    );
    assert!(
        nexus.contains("impl triad_runtime::NexusWork for NexusWork {}"),
        "nexus runtime target should mark the local work enum with the runtime role trait:\n{nexus}"
    );
    assert!(
        !nexus.contains("impl triad_runtime::SemaWriteInput for SemaWriteInput {}"),
        "nexus runtime target must not re-implement role traits for imported SEMA roots:\n{nexus}"
    );
    assert!(
        !nexus.contains("impl triad_runtime::SemaReadInput for SemaReadInput {}"),
        "nexus runtime target must not re-implement role traits for imported SEMA roots:\n{nexus}"
    );
    assert!(
        source_contains_signature(
            nexus,
            "fn budget_exhausted_reply(&self, exhausted: triad_runtime::ContinuationExhausted) -> ContractOutput;"
        ),
        "nexus runtime target should ask the component for a typed exhaustion reply:\n{nexus}"
    );
    assert!(
        !nexus.contains("fn run_effect(&mut self, input"),
        "nexus runtime target should not require an effect hook without CommandEffect:\n{nexus}"
    );
    assert!(
        !nexus.contains("pub trait SemaEngine"),
        "nexus runtime target must not emit SemaEngine:\n{nexus}"
    );
    assert!(
        sema.contains("pub trait SemaEngine"),
        "sema runtime target should emit SemaEngine:\n{sema}"
    );
    assert!(
        sema.contains("impl triad_runtime::SemaWriteInput for SemaWriteInput {}"),
        "sema runtime target should mark its local write input root with the runtime role trait:\n{sema}"
    );
    assert!(
        sema.contains("impl triad_runtime::SemaReadInput for SemaReadInput {}"),
        "sema runtime target should mark its local read input root with the runtime role trait:\n{sema}"
    );
    assert!(
        sema.contains("#[cfg(feature = \"nota-text\")]\npub use nota::{"),
        "sema runtime target should keep its NOTA surface feature-gated:\n{sema}"
    );
    assert!(
        sema.contains("derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode)"),
        "sema runtime support nouns should derive NOTA only behind the feature:\n{sema}"
    );
    assert!(
        !sema.contains("pub trait NexusEngine"),
        "sema runtime target must not emit NexusEngine:\n{sema}"
    );
}

#[test]
fn generated_package_carries_source_and_rust_artifacts() {
    let generated = DriverFixture::new().generated_runtime();
    let module = generated
        .modules()
        .iter()
        .find(|module| module.module().as_str() == "nexus")
        .expect("nexus module");

    assert_eq!(
        module.source_artifact().path(),
        DriverFixture::new().runtime.path().join("nexus.schema")
    );
    assert_eq!(
        module.source_artifact().content(),
        "{\n  ContractInput driver-contract:lib:Input\n  ContractOutput driver-contract:lib:Output\n  SemaReadInput driver-runtime:sema:SemaReadInput\n  SemaReadOutput driver-runtime:sema:SemaReadOutput\n  SemaWriteInput driver-runtime:sema:SemaWriteInput\n  SemaWriteOutput driver-runtime:sema:SemaWriteOutput\n}\n[(SignalArrived ContractInput)]\n[(CommandSemaRead SemaReadInput) (CommandSemaWrite SemaWriteInput) (ReplyToSignal ContractOutput)]\n{\n  NexusWork [(SignalArrived ContractInput) (SemaReadCompleted SemaReadOutput) (SemaWriteCompleted SemaWriteOutput)]\n  NexusAction [(CommandSemaRead SemaReadInput) (CommandSemaWrite SemaWriteInput) (ReplyToSignal ContractOutput)]\n  DecisionReceipt { Integer }\n}"
    );
    assert_eq!(module.rust_file().path, "src/schema/nexus.rs");
    assert!(
        module
            .rust_file()
            .code
            .as_str()
            .contains("#[rustfmt::skip]"),
        "generated source-visible files should opt out of rustfmt so fixture freshness and cargo fmt do not fight"
    );
    assert!(
        module
            .rust_file()
            .code
            .as_str()
            .contains("pub trait NexusEngine"),
        "driver should emit Rust from the typed schema source value"
    );
}

#[test]
fn generated_package_feedback_summarizes_source_and_rust_outputs() {
    let fixture = DriverFixture::new();
    let generated = fixture.generated_runtime();
    let feedback = generated.feedback();

    assert_eq!(feedback.crate_root(), fixture.runtime.crate_root());
    let nexus = feedback
        .modules()
        .iter()
        .find(|module| module.module().as_str() == "nexus")
        .expect("nexus feedback");

    assert_eq!(
        nexus.source_path(),
        fixture.runtime.path().join("nexus.schema")
    );
    assert!(
        nexus
            .source_text()
            .contains("ContractInput driver-contract:lib:Input"),
        "feedback should expose canonical source text for fast inspection"
    );
    assert_eq!(nexus.rust_path(), "src/schema/nexus.rs");
    assert!(
        nexus.rust_byte_count() > nexus.source_text().len(),
        "feedback should expose the generated Rust artifact size"
    );
}

#[test]
fn generation_plan_derives_environment_manifest_from_selected_modules() {
    let manifest = DriverFixture::new().runtime_plan().environment_manifest();

    assert_eq!(
        manifest
            .module_names()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        vec!["nexus", "sema"]
    );
}

#[test]
fn driver_regenerates_rust_from_environment_true_schemas() {
    let fixture = DriverFixture::new();
    let driver = GenerationDriver::new(fixture.runtime_plan());
    let generated_from_environment = driver
        .generate_from_environment(&fixture.environment())
        .expect("driver emits from environment result");
    let generated_from_package = driver.generate().expect("driver emits from package");

    let environment_nexus = generated_from_environment
        .rust_file_named("src/schema/nexus.rs")
        .expect("environment nexus file");
    let package_nexus = generated_from_package
        .rust_file_named("src/schema/nexus.rs")
        .expect("package nexus file");

    assert_eq!(environment_nexus.code, package_nexus.code);

    let feedback = generated_from_environment.feedback();
    let nexus = feedback
        .modules()
        .iter()
        .find(|module| module.module().as_str() == "nexus")
        .expect("nexus feedback");
    assert!(
        nexus
            .source_text()
            .contains("ContractOutput driver-contract:lib:Output"),
        "environment-backed feedback keeps canonical selected source"
    );
}

#[test]
fn component_runtime_compatibility_keeps_lib_component_runtime_explicit() {
    let plan = GenerationPlan::component_runtime_compatibility(
        FixtureSchemaDirectory::new("driver-contract").crate_root(),
        "driver-contract",
        "0.1.0",
    );
    assert_eq!(plan.modules(), &[ModuleEmission::lib_component_runtime()]);
}

#[test]
fn signal_runtime_module_selects_the_signal_runtime_target() {
    let plan = GenerationPlan::new(
        FixtureSchemaDirectory::new("driver-contract").crate_root(),
        "driver-contract",
        "0.1.0",
    )
    .with_module(ModuleEmission::signal_runtime_module("signal"));

    assert_eq!(
        plan.modules(),
        &[ModuleEmission::signal_runtime_module("signal")]
    );
}

#[test]
fn contract_crate_build_is_the_standalone_wire_contract_driver() {
    let fixture = FixtureSchemaDirectory::new("driver-contract");
    let build = ContractCrateBuild::new(
        fixture.crate_root(),
        "driver-contract",
        "0.1.0",
        "DRIVER_CONTRACT_UPDATE_SCHEMA_ARTIFACTS",
    );

    assert_eq!(build.crate_name(), "driver-contract");
    assert_eq!(build.links_name(), "driver-contract");
    assert_eq!(build.module(), "lib");
    assert_eq!(
        build.update_environment_variable(),
        "DRIVER_CONTRACT_UPDATE_SCHEMA_ARTIFACTS"
    );
    assert_eq!(
        build.generation_plan(),
        GenerationPlan::wire_contract(fixture.crate_root(), "driver-contract", "0.1.0")
    );
}

#[test]
fn contract_crate_build_emits_standalone_schema_file() {
    let generated = ContractCrateBuild::new(
        FixtureSchemaDirectory::new("driver-contract").crate_root(),
        "driver-contract",
        "0.1.0",
        "DRIVER_CONTRACT_UPDATE_SCHEMA_ARTIFACTS",
    )
    .generated_package()
    .expect("contract crate generates");
    let source = generated
        .rust_file_named("src/schema/lib.rs")
        .expect("wire contract file")
        .code
        .as_str();

    assert!(
        source.contains("pub enum Input"),
        "standalone contract crate should emit public wire input root:\n{source}"
    );
    assert!(
        source.contains("pub type Frame = signal_frame::ExchangeFrame<Input, Output>;")
            && source.contains("impl signal_frame::RequestPayload for Input {}")
            && source.contains("pub fn encode_signal_frame(&self)"),
        "standalone contract crate should own signal-frame aliases and codecs:\n{source}"
    );
    assert!(
        !source.contains("pub trait NexusEngine") && !source.contains("pub trait SemaEngine"),
        "standalone contract crate must not emit daemon-internal engine traits:\n{source}"
    );
}
