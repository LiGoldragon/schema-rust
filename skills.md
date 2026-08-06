# Skills — schema-rust

Read the workspace Rust and schema skills before editing this repo:

- `skills/rust-discipline.md`
- `skills/rust/methods.md`
- `skills/rust/errors.md`
- `skills/rust/storage-and-wire.md`
- `skills/rust/crate-layout.md`
- `skills/abstractions.md`
- `skills/actor-systems.md`

This repository owns only verified bootstrap projection for authority-sealed
Interface and Sema Ethos transactions. `BootstrapInterfaceGeneration` and
`BootstrapSemaGeneration` retain the reader/transaction pairing through Core
Nomos validation, then delegate the only structural Rust text projection to
Rust Logos with explicit type paths.

Component repositories own their canonical Ethos source, authority manifest,
checked Rust projection, and explicit update request. Use the paired generated
artifacts in `schema_rust::bootstrap`; do not introduce another parser,
lowering route, naming table, inferred Rust spelling, or unchecked emission
path.

`schema_rust::build` owns checked-artifact freshness and one Cargo discovery
contract. A component with a Cargo `links` name may publish an explicitly
chosen Ethos source directory through `CargoEthosSourceMetadata`; the API must
never infer a directory spelling from the crate root. A dependent build may
resolve that directory and emit its exact rerun instruction before verifying
its own authority-sealed transaction. The metadata carries only the canonical
textual-source location—never schema family, version, parsing, lowering,
generation, runtime, or wire policy.
