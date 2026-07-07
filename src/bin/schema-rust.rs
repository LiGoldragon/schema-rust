use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use nota::{NotaDecode, NotaDecodeError, NotaEncode, NotaSource};
use schema_language::{ImportResolver, SchemaEnvironment, SchemaEnvironmentResult};
use schema_rust::{
    RustEmissionOptions, RustEmissionTarget,
    build::{
        BuildError, DependencySchema, GenerationDriver, GenerationFeedback, GenerationPlan,
        ModuleEmission, ModuleFeedback,
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
}

impl SchemaRustCli {
    fn from_environment() -> Self {
        Self {
            command: ComponentCommand::from_environment(),
        }
    }

    fn run(&self) -> Result<(), SchemaRustCliError> {
        let input = RequestText::from_argument(self.command.nota_argument()?)?.parse()?;
        let output = input.execute()?;
        println!("{}", output.to_nota());
        Ok(())
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
            ComponentArgument::InlineNota(argument) => Ok(Self::new(argument.into_string())),
            ComponentArgument::NotaFile(file) => RequestFile::new(file.into_path()).read(),
            ComponentArgument::SignalFile(file) => RequestFile::new(file.into_path()).read(),
        }
    }

    fn parse(&self) -> Result<Input, SchemaRustCliError> {
        NotaSource::new(&self.text)
            .parse::<Input>()
            .map_err(SchemaRustCliError::NotaDecode)
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
            .map_err(|source| SchemaRustCliError::ReadNotaFile {
                path: self.path,
                source,
            })
    }
}

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
enum ModuleRequest {
    WireContract(ModuleName),
    Declaration(ModuleName),
    SignalRuntime(ModuleName),
    NexusRuntime(ModuleName),
    SemaRuntime(ModuleName),
    ComponentRuntime(ModuleName),
}

impl ModuleRequest {
    fn emission(&self) -> ModuleEmission {
        match self {
            Self::WireContract(module) => ModuleEmission::wire_contract_module(module.as_str()),
            Self::Declaration(module) => ModuleEmission::declaration_module(module.as_str()),
            Self::SignalRuntime(module) => ModuleEmission::signal_runtime_module(module.as_str()),
            Self::NexusRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_nota("nota-text")
                    .with_target(RustEmissionTarget::NexusRuntime),
            ),
            Self::SemaRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_nota("nota-text")
                    .with_target(RustEmissionTarget::SemaRuntime),
            ),
            Self::ComponentRuntime(module) => ModuleEmission::new(
                module.as_str(),
                RustEmissionOptions::feature_gated_nota("nota-text")
                    .with_target(RustEmissionTarget::ComponentRuntime),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
enum Output {
    Generated(GenerationFeedbackOutput),
}

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaDecode, NotaEncode, PartialEq)]
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

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
struct CrateName(String);

impl CrateName {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
struct SchemaVersion(String);

impl SchemaVersion {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, NotaDecode, NotaEncode, PartialEq)]
struct ModuleName(String);

impl ModuleName {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, NotaDecode, PartialEq)]
struct SchemaDirectory(String);

impl SchemaDirectory {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
struct SourcePath(String);

impl From<&Path> for SourcePath {
    fn from(path: &Path) -> Self {
        Self(path.display().to_string())
    }
}

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
struct SourceText(String);

impl SourceText {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
struct RustPath(String);

impl RustPath {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, NotaEncode, PartialEq)]
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

    #[error("failed to read NOTA file {}: {source}", path.display())]
    ReadNotaFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid schema-rust request NOTA: {0}")]
    NotaDecode(NotaDecodeError),

    #[error("generation failed: {0}")]
    Build(#[from] BuildError),
}
