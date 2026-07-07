use schema_language::{SchemaEngine, SchemaIdentity};
use schema_rust::RustEmitter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = include_str!("../tests/fixtures/spirit-min.schema");
    let schema =
        SchemaEngine::default().lower_source(source, SchemaIdentity::new("spirit:lib", "0.1.0"))?;
    print!(
        "{}",
        RustEmitter::default()
            .emit_code_from_true_schema(&schema)
            .as_str()
    );
    Ok(())
}
