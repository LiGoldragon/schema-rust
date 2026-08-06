//! Checked-artifact and Cargo metadata support for verified bootstrap generation.

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// Cargo's discovery contract for component-owned Ethos source directories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoEthosSourceMetadata {
    links_name: String,
}

impl CargoEthosSourceMetadata {
    /// Bind dependency discovery to one Cargo `links` name.
    pub fn new(links_name: impl Into<String>) -> Self {
        Self {
            links_name: links_name.into(),
        }
    }

    /// Publish an explicit directory owned by the component running this build script.
    pub fn publish_owned_source_directory(&self, source_directory: impl AsRef<Path>) {
        println!(
            "cargo::metadata=ethos-source-dir={}",
            source_directory.as_ref().display()
        );
    }

    /// Read one dependency's published Ethos source directory when present.
    pub fn dependency_source_directory(&self) -> Option<PathBuf> {
        env::var_os(self.dependency_source_directory_variable()).map(PathBuf::from)
    }

    /// Exact Cargo environment variable for one dependency's Ethos source directory.
    pub fn dependency_source_directory_variable(&self) -> String {
        format!(
            "DEP_{}_ETHOS_SOURCE_DIR",
            Self::normalized_links_name(&self.links_name)
        )
    }

    /// Rebuild when Cargo reseats the dependency's published Ethos directory.
    pub fn emit_dependency_rerun_instruction(&self) {
        println!(
            "cargo::rerun-if-env-changed={}",
            self.dependency_source_directory_variable()
        );
    }

    fn normalized_links_name(links_name: &str) -> String {
        links_name
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
    fn ethos_source_metadata_normalizes_the_links_name() {
        assert_eq!(
            CargoEthosSourceMetadata::new("signal-domain").dependency_source_directory_variable(),
            "DEP_SIGNAL_DOMAIN_ETHOS_SOURCE_DIR"
        );
    }
}
