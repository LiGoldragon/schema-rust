# schema-rust

`schema-rust` is the verified bootstrap generation boundary for Interface and
Sema Ethos documents.

A caller starts an empty Sema bootstrap authority and provides only source text
and its placement. The resulting opaque authority transaction is revalidated
and lowered by Core Nomos into Whole Logos. Rust Logos then projects it through
the authority's sealed read-only name view. This crate creates no identities,
metadata, canonical bytes, or Rust spelling from an Ethos name; callers may
still provide explicit external Rust type paths.

The generated Ethos and Rust artifacts are checked together. Build scripts may
update them only through an explicit component-owned environment variable.

The durable gate is:

```sh
nix flake check -L
```
