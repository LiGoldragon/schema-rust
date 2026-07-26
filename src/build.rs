use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use schema_language::{
    ImportResolver, Name, SchemaEngine, SchemaEnvironmentManifest, SchemaEnvironmentModule,
    SchemaEnvironmentResult, SchemaError, SchemaPackage, SchemaSourceArtifact,
};

use crate::{
    DaemonModule, GeneratedFile, NexusDaemonShape, RustEmissionOptions, RustEmissionTarget,
    RustEmitter, RustGenerationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationPlan {
    package: SchemaPackage,
    modules: Vec<ModuleEmission>,
    dependencies: Vec<DependencySchema>,
}

impl GenerationPlan {
    pub fn new(
        crate_root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            package: SchemaPackage::new(crate_root, crate_name, version),
            modules: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn wire_contract(
        crate_root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        version: impl Into<String>,
        family: protos::WireContractFamily,
    ) -> Self {
        Self::new(crate_root, crate_name, version)
            .with_module(ModuleEmission::wire_contract(family))
    }

    pub fn daemon_runtime(
        crate_root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self::new(crate_root, crate_name, version)
            .with_module(ModuleEmission::nexus_runtime())
            .with_module(ModuleEmission::sema_runtime())
    }

    pub fn component_runtime_compatibility(
        crate_root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self::new(crate_root, crate_name, version)
            .with_module(ModuleEmission::lib_component_runtime())
    }

    pub fn with_module(mut self, module: ModuleEmission) -> Self {
        self.modules.push(module);
        self
    }

    pub fn with_dependency_schema(mut self, dependency: DependencySchema) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn with_optional_dependency_schema(mut self, dependency: Option<DependencySchema>) -> Self {
        if let Some(dependency) = dependency {
            self.dependencies.push(dependency);
        }
        self
    }

    pub fn with_dependency_schema_directory(
        self,
        crate_name: impl Into<String>,
        schema_directory: impl Into<PathBuf>,
        version: impl Into<String>,
    ) -> Self {
        self.with_dependency_schema(DependencySchema::new(crate_name, schema_directory, version))
    }

    pub fn package(&self) -> &SchemaPackage {
        &self.package
    }

    pub fn modules(&self) -> &[ModuleEmission] {
        &self.modules
    }

    pub fn dependencies(&self) -> &[DependencySchema] {
        &self.dependencies
    }

    fn import_resolver(&self) -> ImportResolver {
        self.dependencies.iter().fold(
            ImportResolver::new().with_package(self.package.clone()),
            |resolver, dependency| dependency.register(resolver),
        )
    }

    pub fn environment_manifest(&self) -> SchemaEnvironmentManifest {
        SchemaEnvironmentManifest::new(
            self.modules
                .iter()
                .map(|module| module.module().clone())
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleEmission {
    module: Name,
    options: RustEmissionOptions,
    daemon_shape: Option<NexusDaemonShape>,
}

impl ModuleEmission {
    pub fn new(module: impl Into<String>, options: RustEmissionOptions) -> Self {
        Self {
            module: Name::new(module),
            options,
            daemon_shape: None,
        }
    }

    /// The daemon-module emission (`triad_main!`): off by default, on only when
    /// a component declares a [`NexusDaemonShape`]. It reads the working signal
    /// `module`'s schema for the stream declarations that drive the option-B
    /// publish/subscribe wiring, and emits `src/schema/daemon.rs`.
    pub fn daemon_module(module: impl Into<String>, daemon_shape: NexusDaemonShape) -> Self {
        Self {
            module: Name::new(module),
            options: RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::SignalRuntime),
            daemon_shape: Some(daemon_shape),
        }
    }

    pub fn daemon_shape(&self) -> Option<&NexusDaemonShape> {
        self.daemon_shape.as_ref()
    }

    pub fn wire_contract(family: protos::WireContractFamily) -> Self {
        Self::wire_contract_module("lib", family)
    }

    pub fn declaration_module(module: impl Into<String>) -> Self {
        Self::new(
            module,
            RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::DeclarationModule),
        )
    }

    pub fn wire_contract_module(
        module: impl Into<String>,
        family: protos::WireContractFamily,
    ) -> Self {
        Self::new(module, RustEmissionOptions::wire_contract(family))
    }

    pub fn nexus_runtime() -> Self {
        Self::new(
            "nexus",
            RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::NexusRuntime),
        )
    }

    pub fn signal_runtime() -> Self {
        Self::signal_runtime_module("signal")
    }

    pub fn signal_runtime_module(module: impl Into<String>) -> Self {
        Self::new(
            module,
            RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::SignalRuntime),
        )
    }

    pub fn sema_runtime() -> Self {
        Self::new(
            "sema",
            RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::SemaRuntime),
        )
    }

    pub fn lib_component_runtime() -> Self {
        Self::new(
            "lib",
            RustEmissionOptions::feature_gated_nota("nota-text")
                .with_target(RustEmissionTarget::ComponentRuntime),
        )
    }

    pub fn module(&self) -> &Name {
        &self.module
    }

    pub fn options(&self) -> &RustEmissionOptions {
        &self.options
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySchema {
    crate_name: String,
    schema_directory: PathBuf,
    version: String,
}

impl DependencySchema {
    pub fn new(
        crate_name: impl Into<String>,
        schema_directory: impl Into<PathBuf>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            crate_name: crate_name.into(),
            schema_directory: schema_directory.into(),
            version: version.into(),
        }
    }

    pub fn from_cargo_metadata(
        crate_name: impl Into<String>,
        links_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Option<Self>, BuildError> {
        let crate_name = crate_name.into();
        let version = version.into();
        let metadata = CargoSchemaMetadata::new(links_name);
        let Some(schema_directory) = metadata.schema_directory()? else {
            return Ok(None);
        };
        Ok(Some(Self::new(crate_name, schema_directory, version)))
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn schema_directory(&self) -> &Path {
        &self.schema_directory
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    fn register(&self, resolver: ImportResolver) -> ImportResolver {
        resolver.with_dependency(
            self.crate_name.clone(),
            self.schema_directory.clone(),
            self.version.clone(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoSchemaMetadata {
    links_name: String,
}

impl CargoSchemaMetadata {
    pub fn new(links_name: impl Into<String>) -> Self {
        Self {
            links_name: links_name.into(),
        }
    }

    pub fn emit_schema_directory(&self, crate_root: &Path) {
        let schema_directory = crate_root.join("schema");
        println!("cargo::metadata=schema-dir={}", schema_directory.display());
    }

    pub fn schema_directory(&self) -> Result<Option<PathBuf>, BuildError> {
        let variable = self.schema_directory_variable();
        match env::var_os(&variable) {
            Some(value) => Ok(Some(PathBuf::from(value))),
            None => Ok(None),
        }
    }

    pub fn schema_directory_variable(&self) -> String {
        format!("DEP_{}_SCHEMA_DIR", self.normalized_links_name())
    }

    fn normalized_links_name(&self) -> String {
        self.links_name
            .chars()
            .map(|character| match character {
                '-' => '_',
                other => other.to_ascii_uppercase(),
            })
            .collect()
    }
}

pub struct GenerationDriver {
    plan: GenerationPlan,
    engine: SchemaEngine,
}

impl GenerationDriver {
    pub fn new(plan: GenerationPlan) -> Self {
        Self {
            plan,
            engine: SchemaEngine::default(),
        }
    }

    pub fn with_engine(mut self, engine: SchemaEngine) -> Self {
        self.engine = engine;
        self
    }

    pub fn generate(&self) -> Result<GeneratedPackage, BuildError> {
        let resolver = self.plan.import_resolver();
        let mut modules = Vec::new();
        for emission in self.plan.modules() {
            modules.push(GeneratedModule::from_emission(
                self.plan.package(),
                emission,
                &self.engine,
                &resolver,
            )?);
        }
        Ok(GeneratedPackage::new(
            self.plan.package().root().to_path_buf(),
            modules,
        ))
    }

    pub fn generate_from_environment(
        &self,
        environment: &SchemaEnvironmentResult,
    ) -> Result<GeneratedPackage, BuildError> {
        let mut modules = Vec::new();
        for emission in self.plan.modules() {
            let environment_module = environment
                .modules()
                .iter()
                .find(|module| self.environment_module_matches_emission(module, emission))
                .ok_or_else(|| BuildError::MissingEnvironmentModule {
                    module: emission.module().as_str().to_owned(),
                })?;
            modules.push(GeneratedModule::from_environment_module(
                environment_module,
                emission,
            )?);
        }
        Ok(GeneratedPackage::new(
            self.plan.package().root().to_path_buf(),
            modules,
        ))
    }

    fn environment_module_matches_emission(
        &self,
        module: &SchemaEnvironmentModule,
        emission: &ModuleEmission,
    ) -> bool {
        let prefix = format!("{}:", self.plan.package().crate_name().as_str());
        module
            .source()
            .identity()
            .component()
            .as_str()
            .strip_prefix(&prefix)
            .is_some_and(|selected| selected == emission.module().as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCrateBuild {
    crate_root: PathBuf,
    crate_name: String,
    schema_version: String,
    links_name: String,
    module: String,
    update_environment_variable: String,
    wire_contract_family: protos::WireContractFamily,
}

impl ContractCrateBuild {
    pub fn new(
        crate_root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        schema_version: impl Into<String>,
        update_environment_variable: impl Into<String>,
        wire_contract_family: protos::WireContractFamily,
    ) -> Self {
        let crate_name = crate_name.into();
        Self {
            crate_root: crate_root.into(),
            links_name: crate_name.clone(),
            crate_name,
            schema_version: schema_version.into(),
            module: "lib".to_owned(),
            update_environment_variable: update_environment_variable.into(),
            wire_contract_family,
        }
    }

    pub fn from_environment(
        crate_name: impl Into<String>,
        schema_version: impl Into<String>,
        update_environment_variable: impl Into<String>,
        wire_contract_family: protos::WireContractFamily,
    ) -> Self {
        Self::new(
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir set")),
            crate_name,
            schema_version,
            update_environment_variable,
            wire_contract_family,
        )
    }

    pub fn with_links_name(mut self, links_name: impl Into<String>) -> Self {
        self.links_name = links_name.into();
        self
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = module.into();
        self
    }

    pub fn crate_root(&self) -> &Path {
        &self.crate_root
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    pub fn links_name(&self) -> &str {
        &self.links_name
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn update_environment_variable(&self) -> &str {
        &self.update_environment_variable
    }

    pub fn wire_contract_family(&self) -> protos::WireContractFamily {
        self.wire_contract_family
    }

    pub fn generation_plan(&self) -> GenerationPlan {
        GenerationPlan::new(&self.crate_root, &self.crate_name, &self.schema_version).with_module(
            ModuleEmission::wire_contract_module(&self.module, self.wire_contract_family),
        )
    }

    pub fn generated_package(&self) -> Result<GeneratedPackage, BuildError> {
        GenerationDriver::new(self.generation_plan()).generate()
    }

    pub fn run(&self) -> Result<(), BuildError> {
        self.print_cargo_directives();
        self.generated_package()?
            .write_or_check(&self.update_environment_variable)
    }

    pub fn expect_fresh(&self) {
        self.run()
            .expect("checked-in wire contract schema artifacts are fresh");
    }

    fn print_cargo_directives(&self) {
        println!("cargo:rerun-if-changed=schema/{}.schema", self.module);
        println!(
            "cargo:rerun-if-changed=src/schema/{}.rs",
            Name::new(self.module.as_str()).field_name()
        );
        CargoSchemaMetadata::new(&self.links_name).emit_schema_directory(&self.crate_root);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedPackage {
    crate_root: PathBuf,
    modules: Vec<GeneratedModule>,
}

impl GeneratedPackage {
    pub fn new(crate_root: impl Into<PathBuf>, modules: Vec<GeneratedModule>) -> Self {
        Self {
            crate_root: crate_root.into(),
            modules,
        }
    }

    pub fn modules(&self) -> &[GeneratedModule] {
        &self.modules
    }

    pub fn rust_files(&self) -> Vec<&GeneratedFile> {
        self.modules
            .iter()
            .map(GeneratedModule::rust_file)
            .collect()
    }

    pub fn rust_file_named(&self, path: &str) -> Option<&GeneratedFile> {
        self.modules
            .iter()
            .map(GeneratedModule::rust_file)
            .find(|file| file.path == path)
    }

    pub fn feedback(&self) -> GenerationFeedback {
        GenerationFeedback::from_package(self)
    }

    pub fn assert_checked_in(&self) -> Result<(), BuildError> {
        self.check_with(FreshnessCheck::check_only())
    }

    pub fn write_or_check(
        self,
        update_environment_variable: impl Into<String>,
    ) -> Result<(), BuildError> {
        self.check_with(FreshnessCheck::from_environment(
            update_environment_variable,
        ))
    }

    fn check_with(&self, check: FreshnessCheck) -> Result<(), BuildError> {
        for module in &self.modules {
            module.check_generated_artifacts(&self.crate_root, &check)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationFeedback {
    crate_root: PathBuf,
    modules: Vec<ModuleFeedback>,
}

impl GenerationFeedback {
    pub fn crate_root(&self) -> &Path {
        &self.crate_root
    }

    pub fn modules(&self) -> &[ModuleFeedback] {
        &self.modules
    }

    fn from_package(package: &GeneratedPackage) -> Self {
        Self {
            crate_root: package.crate_root.clone(),
            modules: package
                .modules
                .iter()
                .map(ModuleFeedback::from_module)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFeedback {
    module: Name,
    source_path: PathBuf,
    source_text: String,
    rust_path: String,
    rust_byte_count: usize,
}

impl ModuleFeedback {
    pub fn module(&self) -> &Name {
        &self.module
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    pub fn rust_path(&self) -> &str {
        &self.rust_path
    }

    pub fn rust_byte_count(&self) -> usize {
        self.rust_byte_count
    }

    fn from_module(module: &GeneratedModule) -> Self {
        Self {
            module: module.module.clone(),
            source_path: module.source_artifact.path.clone(),
            source_text: module.source_artifact.content.clone(),
            rust_path: module.rust_file.path.clone(),
            rust_byte_count: module.rust_file.code.as_str().len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedModule {
    module: Name,
    source_artifact: GeneratedArtifact,
    rust_file: GeneratedFile,
}

impl GeneratedModule {
    pub fn module(&self) -> &Name {
        &self.module
    }

    pub fn source_artifact(&self) -> &GeneratedArtifact {
        &self.source_artifact
    }

    pub fn rust_file(&self) -> &GeneratedFile {
        &self.rust_file
    }

    fn from_emission(
        package: &SchemaPackage,
        emission: &ModuleEmission,
        engine: &SchemaEngine,
        resolver: &ImportResolver,
    ) -> Result<Self, BuildError> {
        let source = package.load_module(emission.module().clone())?;
        let schema_source = source.to_schema_source()?;
        let source_artifact = SourceArtifactRoundTrip::new(
            source.path().to_path_buf(),
            SchemaSourceArtifact::new(schema_source.clone()),
        )
        .validate()?;
        let rust_file = match emission.daemon_shape() {
            Some(daemon_shape) => {
                DaemonModule::new(daemon_shape.clone(), "schema-rust").to_generated_file()
            }
            None => RustEmitter::new(emission.options().clone()).emit_file_from_schema_source(
                &schema_source,
                source.identity().clone(),
                engine,
                resolver,
            )?,
        };
        Ok(Self {
            module: emission.module().clone(),
            source_artifact,
            rust_file,
        })
    }

    fn from_environment_module(
        environment: &SchemaEnvironmentModule,
        emission: &ModuleEmission,
    ) -> Result<Self, BuildError> {
        let source_artifact = SourceArtifactRoundTrip::new(
            environment.source().path().to_path_buf(),
            environment.artifact().clone(),
        )
        .validate()?;
        let rust_file = match emission.daemon_shape() {
            Some(daemon_shape) => {
                DaemonModule::new(daemon_shape.clone(), "schema-rust").to_generated_file()
            }
            None => {
                let module = RustEmitter::new(emission.options().clone())
                    .emit_module_from_true_schema(environment.true_schema());
                module.verify_names()?;
                module.verify_catalog(environment.true_schema())?;
                GeneratedFile {
                    path: module.file_path().to_owned(),
                    code: module.render()?,
                }
            }
        };
        Ok(Self {
            module: emission.module().clone(),
            source_artifact,
            rust_file,
        })
    }

    fn check_generated_artifacts(
        &self,
        crate_root: &Path,
        check: &FreshnessCheck,
    ) -> Result<(), BuildError> {
        self.rust_artifact(crate_root).check_with(check)?;
        Ok(())
    }

    fn rust_artifact(&self, crate_root: &Path) -> GeneratedArtifact {
        GeneratedArtifact::new(
            crate_root.join(&self.rust_file.path),
            self.rust_file.code.as_str().to_owned(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceArtifactRoundTrip {
    path: PathBuf,
    artifact: SchemaSourceArtifact,
}

impl SourceArtifactRoundTrip {
    fn new(path: PathBuf, artifact: SchemaSourceArtifact) -> Self {
        Self { path, artifact }
    }

    fn validate(self) -> Result<GeneratedArtifact, BuildError> {
        let source_text = self.artifact.to_schema_text();
        let recovered = SchemaSourceArtifact::from_schema_text(&source_text)?;
        if recovered != self.artifact {
            return Err(BuildError::SchemaSourceRoundTrip { path: self.path });
        }
        let source_binary = recovered.to_binary_bytes()?;
        let recovered_from_binary = SchemaSourceArtifact::from_binary_bytes(&source_binary)?;
        if recovered_from_binary != recovered {
            return Err(BuildError::SchemaSourceArchiveRoundTrip { path: self.path });
        }
        Ok(GeneratedArtifact::new(self.path, source_text))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
    path: PathBuf,
    content: String,
}

impl GeneratedArtifact {
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    fn check_with(&self, check: &FreshnessCheck) -> Result<(), BuildError> {
        if check.updates_files() {
            self.write()?;
            return Ok(());
        }
        if self.matches_existing()? {
            return Ok(());
        }
        Err(check.stale_generated_artifact_error(self.path.clone()))
    }

    fn matches_existing(&self) -> Result<bool, BuildError> {
        match fs::read_to_string(&self.path) {
            Ok(existing) => Ok(existing == self.content),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(BuildError::ReadGeneratedArtifact {
                path: self.path.clone(),
                source: error,
            }),
        }
    }

    fn write(&self) -> Result<(), BuildError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| BuildError::WriteGeneratedArtifact {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&self.path, &self.content).map_err(|source| BuildError::WriteGeneratedArtifact {
            path: self.path.clone(),
            source,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FreshnessCheck {
    CheckOnly,
    UpdateWhenRequested {
        update_environment_variable: String,
        update_files: bool,
    },
}

impl FreshnessCheck {
    fn check_only() -> Self {
        Self::CheckOnly
    }

    fn from_environment(update_environment_variable: impl Into<String>) -> Self {
        let update_environment_variable = update_environment_variable.into();
        let update_files = env::var_os(&update_environment_variable).is_some();
        Self::UpdateWhenRequested {
            update_environment_variable,
            update_files,
        }
    }

    fn updates_files(&self) -> bool {
        matches!(
            self,
            Self::UpdateWhenRequested {
                update_files: true,
                ..
            }
        )
    }

    fn stale_generated_artifact_error(&self, path: PathBuf) -> BuildError {
        match self {
            Self::CheckOnly => BuildError::StaleGeneratedArtifact { path },
            Self::UpdateWhenRequested {
                update_environment_variable,
                ..
            } => BuildError::StaleGeneratedArtifactUpdateAvailable {
                path,
                update_environment_variable: update_environment_variable.clone(),
            },
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error(transparent)]
    Generation(#[from] RustGenerationError),
    #[error("read generated artifact {path:?}: {source}")]
    ReadGeneratedArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("write generated artifact {path:?}: {source}")]
    WriteGeneratedArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("generated artifact {path:?} is stale")]
    StaleGeneratedArtifact { path: PathBuf },
    #[error(
        "generated artifact {path:?} is stale; set {update_environment_variable}=1 to update it"
    )]
    StaleGeneratedArtifactUpdateAvailable {
        path: PathBuf,
        update_environment_variable: String,
    },
    #[error("schema source artifact did not round-trip through generated text at {path:?}")]
    SchemaSourceRoundTrip { path: PathBuf },
    #[error(
        "schema source artifact did not round-trip through generated binary archive at {path:?}"
    )]
    SchemaSourceArchiveRoundTrip { path: PathBuf },
    #[error("environment result did not include requested module {module}")]
    MissingEnvironmentModule { module: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_only_staleness_has_no_update_instruction() {
        let path = PathBuf::from("schema-rust-freshness-check-only-missing.rs");
        let error = GeneratedArtifact::new(&path, "generated")
            .check_with(&FreshnessCheck::check_only())
            .expect_err("missing artifact is stale");

        assert!(
            matches!(&error, BuildError::StaleGeneratedArtifact { path: found } if found == &path)
        );
        assert!(!error.to_string().contains("set "));
    }

    #[test]
    fn update_capable_staleness_carries_its_update_variable() {
        let path = PathBuf::from("schema-rust-freshness-update-missing.rs");
        let error = FreshnessCheck::from_environment("UPDATE_SCHEMA_RUST_FIXTURE")
            .stale_generated_artifact_error(path.clone());

        assert!(matches!(
            error,
            BuildError::StaleGeneratedArtifactUpdateAvailable {
                path: found,
                update_environment_variable,
            } if found == path && update_environment_variable == "UPDATE_SCHEMA_RUST_FIXTURE"
        ));
    }
}
