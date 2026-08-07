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

    /// Require this artifact's checked-in content to equal its canonical
    /// projection.
    pub fn assert_matches_existing(&self) -> Result<(), BuildError> {
        if self.matches_existing()? {
            Ok(())
        } else {
            Err(BuildError::StaleGeneratedArtifact {
                path: self.path.clone(),
            })
        }
    }

    /// Write this artifact's canonical content to an explicit target path,
    /// creating parent directories as needed.
    pub fn write_to(&self, target: &Path) -> Result<(), BuildError> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| BuildError::WriteGeneratedArtifact {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(target, &self.content).map_err(|source| BuildError::WriteGeneratedArtifact {
            path: target.to_path_buf(),
            source,
        })
    }

    /// The sibling `.pending` path used during atomic installation.
    pub fn pending_path(&self) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .expect("artifact path has a file name")
            .to_os_string();
        name.push(".pending");
        self.path.with_file_name(name)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_artifact_is_stale() {
        let path = PathBuf::from("schema-rust-freshness-missing.rs");
        let error = GeneratedArtifact::new(&path, "generated")
            .assert_matches_existing()
            .expect_err("missing artifact is stale");

        assert!(
            matches!(&error, BuildError::StaleGeneratedArtifact { path: found } if found == &path)
        );
    }

    #[test]
    fn ethos_source_metadata_normalizes_the_links_name() {
        assert_eq!(
            CargoEthosSourceMetadata::new("signal-domain").dependency_source_directory_variable(),
            "DEP_SIGNAL_DOMAIN_ETHOS_SOURCE_DIR"
        );
    }
}
