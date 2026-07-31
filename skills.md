# Skills — schema-rust

Read the workspace Rust and schema skills before editing this repo:

- `skills/rust-discipline.md`
- `skills/rust/methods.md`
- `skills/rust/errors.md`
- `skills/rust/storage-and-wire.md`
- `skills/rust/crate-layout.md`
- `skills/abstractions.md`
- `skills/actor-systems.md`

This repo emits Rust nouns from `schema_language::TrueSchema` data. Generated Signal,
Nexus, and SEMA traits are the runtime method surface; component crates supply
non-default algorithms by implementing those traits on data-bearing runtime
objects. Do not add parser-side shortcuts or hand-written helper APIs beside
the generated trait path.

Use `RustEmissionTarget::WireContract` for signal and meta-signal contract
repos: that target emits wire nouns, rkyv/DOTOS codecs, and the universal
`signal-frame` request/reply aliases and traits (`Frame`, `FrameBody`,
`Request`, `ReplyEnvelope`, `RequestBuilder`, `RequestPayload`,
`SignalOperationHeads`). Do not hand-write those aliases in contract crates.
Use
`RustEmissionTarget::NexusRuntime` for daemon-crate `schema/nexus.schema` files
and `RustEmissionTarget::SemaRuntime` for `schema/sema.schema` files. Those
per-plane runtime schemas import contract roots where needed.
`RustEmissionTarget::ComponentRuntime` is the compatibility/bootstrap target
for unsplit all-in-one schemas, not the canonical daemon shape.

Component `build.rs` files should use `schema_rust::build` rather than
hand-rolling package loading, lowering, emission, or checked-in freshness
logic. Use `GenerationPlan::wire_contract` for contract crates,
`GenerationPlan::daemon_runtime` for daemon `nexus.schema` + `sema.schema`,
and `GenerationPlan::component_runtime_compatibility` only for current
all-in-one bootstrap schemas. Register imported contract schemas through
`DependencySchema` entries sourced from Cargo build metadata, not hard-coded
local checkout paths.

Contract crates that declare a Cargo `links` name should publish their
`schema/` directory with `CargoSchemaMetadata::emit_schema_directory` after
the schema freshness check. Daemon crates consume that same convention with
`DependencySchema::from_cargo_metadata`.
