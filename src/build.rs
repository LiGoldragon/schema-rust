//! Checked-artifact and Cargo metadata support for verified bootstrap generation.

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// Cargo build-script metadata for a component-owned Ethos source directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoSchemaMetadata {
    links_name: String,
}

impl CargoSchemaMetadata {
    /// Bind metadata to one Cargo `links` name.
    pub fn new(links_name: impl Into<String>) -> Self {
        Self {
            links_name: links_name.into(),
        }
    }

    /// Publish the owning component's `schema` directory to dependants.
    pub fn emit_schema_directory(&self, crate_root: &Path) {
        let schema_directory = crate_root.join("schema");
        println!("cargo::metadata=schema-dir={}", schema_directory.display());
    }

    /// Read the dependency-provided schema directory when present.
    pub fn schema_directory(&self) -> Option<PathBuf> {
        env::var_os(self.schema_directory_variable()).map(PathBuf::from)
    }

    /// Exact Cargo environment variable used for this dependency.
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

/// One canonical projection paired with its checked-in path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
    path: PathBuf,
    content: String,
}

impl GeneratedArtifact {
    /// Bind canonical content to the path that must carry it.
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }

    /// Checked-in artifact path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Canonical generated content.
    pub fn content(&self) -> &str {
        &self.content
    }

    pub(crate) fn check_with(&self, check: &FreshnessCheck) -> Result<(), BuildError> {
        if check.updates_files() {
            return self.write();
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
            Err(source) => Err(BuildError::ReadGeneratedArtifact {
                path: self.path.clone(),
                source,
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
pub(crate) enum FreshnessCheck {
    CheckOnly,
    UpdateWhenRequested {
        update_environment_variable: String,
        update_files: bool,
    },
}

impl FreshnessCheck {
    pub(crate) const fn check_only() -> Self {
        Self::CheckOnly
    }

    pub(crate) fn from_environment(update_environment_variable: impl Into<String>) -> Self {
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

/// Failure while comparing or updating one canonical projection.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Existing generated content could not be read.
    #[error("read generated artifact {path:?}: {source}")]
    ReadGeneratedArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Canonical generated content could not be written.
    #[error("write generated artifact {path:?}: {source}")]
    WriteGeneratedArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Checked-in content differs and this invocation cannot update it.
    #[error("generated artifact {path:?} is stale")]
    StaleGeneratedArtifact { path: PathBuf },
    /// Checked-in content differs and names the explicit update request.
    #[error(
        "generated artifact {path:?} is stale; set {update_environment_variable}=1 to update it"
    )]
    StaleGeneratedArtifactUpdateAvailable {
        path: PathBuf,
        update_environment_variable: String,
    },
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
    fn cargo_metadata_normalizes_the_links_name() {
        assert_eq!(
            CargoSchemaMetadata::new("signal-domain").schema_directory_variable(),
            "DEP_SIGNAL_DOMAIN_SCHEMA_DIR"
        );
    }
}
