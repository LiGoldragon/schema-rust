#![allow(dead_code)]

use std::path::{Path, PathBuf};

use schema::{ImportResolver, MacroContext, SchemaEngine, SchemaIdentity, TrueSchema};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureSchema {
    relative_path: PathBuf,
}

impl FixtureSchema {
    pub fn new(relative_path: impl AsRef<Path>) -> Self {
        Self {
            relative_path: relative_path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> PathBuf {
        Self::fixture_root().join(&self.relative_path)
    }

    pub fn read(&self) -> String {
        std::fs::read_to_string(self.path()).expect("read schema fixture")
    }

    pub fn lower(&self, identity: &str) -> TrueSchema {
        SchemaEngine::default()
            .lower_source(&self.read(), SchemaIdentity::new(identity, "0.1.0"))
            .expect("schema fixture lowers")
    }

    pub fn lower_with_context(&self, identity: &str, context: &mut MacroContext) -> TrueSchema {
        SchemaEngine::default()
            .lower_source_with_context(
                &self.read(),
                SchemaIdentity::new(identity, "0.1.0"),
                context,
            )
            .expect("schema fixture lowers")
    }

    pub fn lower_with_resolver(
        &self,
        identity: &str,
        context: &mut MacroContext,
        resolver: &ImportResolver,
    ) -> TrueSchema {
        SchemaEngine::default()
            .lower_source_with_resolver(
                &self.read(),
                SchemaIdentity::new(identity, "0.1.0"),
                context,
                resolver,
            )
            .expect("schema fixture with imports lowers")
    }

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureNota {
    relative_path: PathBuf,
}

impl FixtureNota {
    pub fn new(relative_path: impl AsRef<Path>) -> Self {
        Self {
            relative_path: relative_path.as_ref().to_path_buf(),
        }
    }

    pub fn read(&self) -> String {
        std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(&self.relative_path),
        )
        .expect("read nota fixture")
        .trim()
        .to_owned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureSchemaDirectory {
    crate_directory: PathBuf,
}

impl FixtureSchemaDirectory {
    pub fn new(crate_directory: impl AsRef<Path>) -> Self {
        Self {
            crate_directory: crate_directory.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(&self.crate_directory)
            .join("schema")
    }

    pub fn crate_root(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(&self.crate_directory)
    }

    pub fn schema(&self, module_path: impl AsRef<Path>) -> FixtureSchema {
        FixtureSchema::new(
            self.crate_directory
                .join("schema")
                .join(module_path.as_ref()),
        )
    }
}
