# Architecture

The authority-approved encoded transaction is the semantic center. Source text
and generated Rust are projections checked against that transaction.

```text
VerifiedBootstrapAssembly
          │
          ▼
BootstrapSliceOneLowering
          │
          ▼
      WholeLogos
          │
          ▼
RustLogos + explicit external type paths
          │
          ▼
checked structural Rust projection
```

`BootstrapInterfaceGeneration` admits only an authority-sealed Interface.
`BootstrapSemaGeneration` admits only an authority-sealed Sema and requires
explicit storage provenance for every nonlocal stored type. Both retain the
reader/transaction pairing through Nomos validation before Rust projection.

Rust naming policy exists only inside Rust Logos. This crate performs no other
generation.

`build` contains only checked-artifact freshness and Cargo schema-directory
metadata. Component repositories own their Ethos sources, authority manifests,
generated Rust, and the explicit update request for those checked artifacts.
