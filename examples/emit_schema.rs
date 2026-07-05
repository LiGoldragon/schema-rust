use std::{env, fs};

use schema::{ImportResolver, MacroContext, SchemaEngine, SchemaIdentity};
use schema_rust::RustEmitter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let schema_path = arguments
        .next()
        .ok_or("usage: emit_schema <schema-path> <schema-identity> [version]")?;
    let identity = arguments
        .next()
        .ok_or("usage: emit_schema <schema-path> <schema-identity> [version]")?;
    let version = arguments.next().unwrap_or_else(|| "0.1.0".to_owned());
    let mut resolver = ImportResolver::new();
    let dependencies = arguments.collect::<Vec<_>>();
    for dependency in dependencies.chunks_exact(3) {
        resolver = resolver.with_dependency(&dependency[0], &dependency[1], &dependency[2]);
    }
    if !dependencies.chunks_exact(3).remainder().is_empty() {
        return Err("dependency arguments must be triples: <crate> <schema-dir> <version>".into());
    }

    let source = fs::read_to_string(schema_path)?;
    let mut context = MacroContext::default();
    let schema = SchemaEngine::default().lower_source_with_resolver(
        &source,
        SchemaIdentity::new(identity, version),
        &mut context,
        &resolver,
    )?;
    print!(
        "{}",
        RustEmitter::default()
            .emit_code_from_true_schema(&schema)
            .as_str()
    );
    Ok(())
}
