# schema-rust

`schema-rust` is the verified bootstrap generation boundary for Interface and
Sema Ethos documents.

An authority-sealed transaction is revalidated and lowered by Core Nomos into
Whole Logos. Rust Logos then owns the sole structural projection into Rust
text. The caller supplies the sealed Rust vocabulary and every external Rust
type path explicitly; this crate infers no Rust spelling from an Ethos name.

The generated Ethos and Rust artifacts are checked together. Build scripts may
update them only through an explicit component-owned environment variable.

The durable gate is:

```sh
nix flake check -L
```
