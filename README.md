# schema-rust

`schema-rust` emits source-visible Rust interface code from `schema-language`'s typed
schema data.

This repository is deliberately not a Rust macro crate. The active path is:
authored `.schema` source deserializes into `schema_language::SchemaSource`, lowers into
semantic `schema_language::TrueSchema`, then emits Rust source under `src/schema/`.
`TrueSchema` is the canonical decoded semantic layer; generated Rust is only a
projection from it.

The shared build driver is the public orchestration surface. Component
`build.rs` files use `schema_rust::build::GenerationDriver`,
`GenerationPlan`, and `ModuleEmission` to load selected schema modules through
`schema_language::SchemaEnvironment`, generate Rust from the environment-carried
`TrueSchema`, and freshness-check the checked-in Rust files.

Emission is still two-step inside the crate: true schema data lowers into a
`RustModule`, and `RustModule::render()` produces `RustCode`. The module object
carries scalar aliases, imports, declarations, roots, and support metadata, so
tests can inspect the code-generation model before comparing rendered source
snapshots.

Generated paths mirror crate-local schema modules. A schema identity such as
`spirit:lib` emits to `src/schema/lib.rs`; an identity such as
`spirit:signal:public` emits to `src/schema/signal/public.rs`. The first
namespace segment is the crate boundary and is not repeated inside the crate's
generated module tree.

The emitted source includes the data types, `nota` codec derives, small
inherent NOTA bridge methods, rkyv derives, short-header signal frames, Nexus
traits, Nexus mail lifecycle objects, mail-event hooks, family identity
surfaces, and upgrade/accept traits that runtime crates implement against.
Public schema declarations emit `pub` Rust nouns; private schema declarations
emit `pub(crate)` module-local nouns so inline PascalCase schema sugar does not
become an exported API by accident.

Terminal contract roots from dependencies remain package-qualified through emission. A schema reference such as `signal-lojix.Input` lowers through the resolved package identity and renders directly as `signal_lojix::Input`; it is never emitted as a local `use ... as Input`. This lets signal and meta-signal contracts both export `Input`/`Output` while the consuming schema retains its own local roots.

Composite type references come from typed datatype objects in the authored
schema, written as dotted positional application: `Vector.Topic`,
`Map.(Topic RecordIdentifier)`, and `Optional.Topic`. Product components are
positional and typed. A unique component normally uses the bare type name;
explicit `field.Type` disambiguation is reserved for repeated same-type
components that need stable field names.

Tests keep meaningful schema and NOTA examples in fixture files under
`tests/fixtures/`. Rust tests load those fixtures through the support helpers
instead of hiding the language examples inside Rust string literals.

The `schema-rust` binary is a thin one-argument NOTA client over the shared
driver. It accepts a `Generate` request, loads the selected modules through
`SchemaEnvironment`, regenerates from `TrueSchema`, and prints typed feedback
with the selected canonical source and generated Rust artifact sizes:

```sh
cargo run --bin schema-rust -- "(Generate (<crate-root> <crate-name> <version> [(NexusRuntime nexus) (SemaRuntime sema)] [(dependency-crate <dependency-schema-dir> <version>)]))"
```

The older `emit_schema` example remains a low-level local debugging tool for a
single schema file. The binary is the command surface that follows the current
build-driver path.
