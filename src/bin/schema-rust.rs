use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use dotos::{DotosDecode, DotosDecodeError, DotosEncode, DotosError, DotosSource, PrettyLayout};
use schema_language::{ImportResolver, SchemaEnvironment, SchemaEnvironmentResult};
use schema_rust::{
    RustEmissionOptions, RustEmissionTarget,
    build::{
        BuildError, CrateName, DependencySchema, GenerationDriver, GenerationFeedback,
        GenerationPlan, ModuleEmission, ModuleFeedback, SchemaVersion,
    },
};
use thiserror::Error;
use triad_runtime::{ArgumentError, ComponentArgument, ComponentCommand};

fn main() -> ExitCode {
    match SchemaRustCli::from_environment().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("schema-rust: {error}");
            ExitCode::FAILURE
        }
    }
}

struct SchemaRustCli {
    command: ComponentCommand,
    output_form: OutputForm,
}

impl SchemaRustCli {
    fn from_environment() -> Self {
        let mut output_form = OutputForm::Canonical;
        let mut operands = Vec::new();
        for argument in std::env::args().skip(1) {
            if argument == OutputForm::PRETTY_FLAG {
                output_form = OutputForm::Pretty;
            } else {
                operands.push(argument);
            }
        }
        Self {
            command: ComponentCommand::from_arguments(operands),
            output_form,
        }
    }

    fn run(&self) -> Result<(), SchemaRustCliError> {
        let input = RequestText::from_argument(self.command.dotos_argument()?)?.parse()?;
        let output = input.execute()?;
        println!("{}", self.output_form.render(&output)?);
        Ok(())
    }
}

/// How the generated DOTOS document is written to stdout.
///
/// The default `Canonical` form is the single-line encoder output every
/// consumer and golden depends on. `Pretty`, requested with `--pretty`, reflows
/// that same document across indented lines for reading; it is a pure
/// readability projection that re-parses to the identical document.
enum OutputForm {
    Canonical,
    Pretty,
}

impl OutputForm {
    const PRETTY_FLAG: &'static str = "--pretty";

    fn render(&self, output: &Output) -> Result<String, SchemaRustCliError> {
        let canonical = output.to_dotos();
        match self {
            Self::Canonical => Ok(canonical),
            Self::Pretty => PrettyLayout::standard()
                .render_dotos(&canonical)
                .map_err(SchemaRustCliError::Pretty),
        }
    }
}

struct RequestText {
    text: String,
}

impl RequestText {
    fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    fn from_argument(argument: ComponentArgument) -> Result<Self, SchemaRustCliError> {
        match argument {
            ComponentArgument::InlineDotos(argument) => Ok(Self::new(argument.into_string())),
            ComponentArgument::DotosFile(file) => RequestFile::new(file.into_path()).read(),
            ComponentArgument::SignalFile(file) => RequestFile::new(file.into_path()).read(),
        }
    }

    fn parse(&self) -> Result<Input, SchemaRustCliError> {
        DotosSource::new(&self.text)
            .parse::<Input>()
            .map_err(SchemaRustCliError::DotosDecode)
    }
}

struct RequestFile {
    path: PathBuf,
}

impl RequestFile {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(self) -> Result<RequestText, SchemaRustCliError> {
        fs::read_to_string(&self.path)
            .map(RequestText::new)
            .map_err(|source| SchemaRustCliError::ReadDotosFile {
                path: self.path,
                source,
            })
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
enum Input {
    Generate(GenerationRequest),
}

impl Input {
    fn execute(self) -> Result<Output, SchemaRustCliError> {
        match self {
            Self::Generate(request) => request.generate().map(Output::Generated),
        }
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
struct GenerationRequest {
    crate_root: CrateRoot,
    crate_name: CrateName,
    version: SchemaVersion,
    modules: Vec<ModuleRequest>,
    dependencies: Vec<DependencyRequest>,
}

impl GenerationRequest {
    fn generate(&self) -> Result<GenerationFeedbackOutput, SchemaRustCliError> {
        let plan = self.plan();
        let environment = self.environment(&plan)?;
        let generated = GenerationDriver::new(plan).generate_from_environment(&environment)?;
        let feedback = generated.feedback();
        Ok(GenerationFeedbackOutput::from(&feedback))
    }

    fn plan(&self) -> GenerationPlan {
        let plan = GenerationPlan::new(
            self.crate_root.as_str(),
            self.crate_name.as_str(),
            self.version.as_str(),
        );
        let plan = self
            .modules
            .iter()
            .fold(plan, |plan, module| plan.with_module(module.emission()));
        self.dependencies.iter().fold(plan, |plan, dependency| {
            plan.with_dependency_schema(dependency.schema())
        })
    }

    fn environment(
        &self,
        plan: &GenerationPlan,
    ) -> Result<SchemaEnvironmentResult, SchemaRustCliError> {
        SchemaEnvironment::new(plan.package().clone())
            .with_resolver(self.resolver())
            .load(&plan.environment_manifest())
            .map_err(BuildError::from)
            .map_err(SchemaRustCliError::Build)
    }

    fn resolver(&self) -> ImportResolver {
        self.dependencies
            .iter()
            .fold(ImportResolver::new(), |resolver, dependency| {
                dependency.register(resolver)
            })
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
enum ModuleRequest {
    WireContract(WireContractRequest),
    Declaration(ModuleName),
    SignalRuntime(ModuleName),
    NexusRuntime(ModuleName),
    SemaRuntime(ModuleName),
    ComponentRuntime(ModuleName),
}

impl ModuleRequest {
    fn emission(&self) -> ModuleEmission {
        match self {
            Self::WireContract(request) => ModuleEmission::wire_contract_module(
                request.module.as_str(),
                request.family.registry_family(),
            ),
            Self::Declaration(module) => ModuleEmission::declaration_module(module.as_str()),
            Self::SignalRuntime(module) => ModuleEmission::signal_runtime_module(module.as_str()),
            Self::NexusRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_dotos("dotos-text")
                    .with_target(RustEmissionTarget::NexusRuntime),
            ),
            Self::SemaRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_dotos("dotos-text")
                    .with_target(RustEmissionTarget::SemaRuntime),
            ),
            Self::ComponentRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_dotos("dotos-text")
                    .with_target(RustEmissionTarget::ComponentRuntime),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
struct WireContractRequest {
    family: WireContractFamily,
    module: ModuleName,
}

#[derive(Clone, Copy, Debug, Eq, DotosDecode, PartialEq)]
enum WireContractFamily {
    SignalSpirit,
    MetaSignalSpirit,
    SignalSpiritJudge,
}

impl WireContractFamily {
    fn registry_family(self) -> protos::WireContractFamily {
        match self {
            Self::SignalSpirit => protos::WireContractFamily::SignalSpirit,
            Self::MetaSignalSpirit => protos::WireContractFamily::MetaSignalSpirit,
            Self::SignalSpiritJudge => protos::WireContractFamily::SignalSpiritJudge,
        }
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
struct DependencyRequest {
    crate_name: CrateName,
    schema_directory: SchemaDirectory,
    version: SchemaVersion,
}

impl DependencyRequest {
    fn register(&self, resolver: ImportResolver) -> ImportResolver {
        resolver.with_dependency(
            self.crate_name.as_str(),
            self.schema_directory.as_str(),
            self.version.as_str(),
        )
    }

    fn schema(&self) -> DependencySchema {
        DependencySchema::new(
            self.crate_name.as_str(),
            self.schema_directory.as_str(),
            self.version.as_str(),
        )
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
enum Output {
    Generated(GenerationFeedbackOutput),
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct GenerationFeedbackOutput {
    crate_root: CrateRoot,
    modules: Vec<ModuleFeedbackOutput>,
}

impl From<&GenerationFeedback> for GenerationFeedbackOutput {
    fn from(feedback: &GenerationFeedback) -> Self {
        Self {
            crate_root: CrateRoot::from(feedback.crate_root()),
            modules: feedback
                .modules()
                .iter()
                .map(ModuleFeedbackOutput::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct ModuleFeedbackOutput {
    module: ModuleName,
    source_path: SourcePath,
    source_text: SourceText,
    rust_path: RustPath,
    rust_byte_count: RustByteCount,
}

impl From<&ModuleFeedback> for ModuleFeedbackOutput {
    fn from(feedback: &ModuleFeedback) -> Self {
        Self {
            module: ModuleName::new(feedback.module().as_str()),
            source_path: SourcePath::from(feedback.source_path()),
            source_text: SourceText::new(feedback.source_text()),
            rust_path: RustPath::new(feedback.rust_path()),
            rust_byte_count: RustByteCount::from(feedback.rust_byte_count()),
        }
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, DotosEncode, PartialEq)]
struct CrateRoot(String);

impl CrateRoot {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&Path> for CrateRoot {
    fn from(path: &Path) -> Self {
        Self(path.display().to_string())
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, DotosEncode, PartialEq)]
struct ModuleName(String);

impl ModuleName {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, DotosDecode, PartialEq)]
struct SchemaDirectory(String);

impl SchemaDirectory {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct SourcePath(String);

impl From<&Path> for SourcePath {
    fn from(path: &Path) -> Self {
        Self(path.display().to_string())
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct SourceText(String);

impl SourceText {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct RustPath(String);

impl RustPath {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, DotosEncode, PartialEq)]
struct RustByteCount(u64);

impl From<usize> for RustByteCount {
    fn from(value: usize) -> Self {
        Self(value as u64)
    }
}

#[derive(Debug, Error)]
enum SchemaRustCliError {
    #[error("component argument error: {0}")]
    Argument(#[from] ArgumentError),

    #[error("failed to read DOTOS file {}: {source}", path.display())]
    ReadDotosFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid schema-rust request DOTOS: {0}")]
    DotosDecode(DotosDecodeError),

    #[error("generation failed: {0}")]
    Build(#[from] BuildError),

    #[error("failed to lay out DOTOS for reading: {0}")]
    Pretty(DotosError),
}
