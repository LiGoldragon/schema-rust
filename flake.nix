{
  description = "schema-rust — verified bootstrap Ethos generation through Rust Logos";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-build = {
      url = "github:LiGoldragon/rust-build";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-build }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rust = rust-build.lib.${system}.fromPkgs pkgs;
        inherit (rust) craneLib toolchain;
        src = rust.cleanSource { root = ./.; };
        commonArguments = { inherit src; strictDeps = true; };
        cargoArtifacts = craneLib.buildDepsOnly commonArguments;
      in
      {
        packages.default = craneLib.buildPackage (commonArguments // { inherit cargoArtifacts; });
        checks = {
          build = craneLib.cargoBuild (commonArguments // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (commonArguments // { inherit cargoArtifacts; });
          sole-bootstrap-surface = pkgs.runCommand "schema-rust-sole-bootstrap-surface" { } ''
            test "$(find ${src}/src -maxdepth 1 -type f -name '*.rs' -printf '%f\n' | sort | tr '\n' ' ')" = "bootstrap.rs build.rs lib.rs "
            test "$(find ${src}/tests -maxdepth 1 -type f -name '*.rs' -printf '%f\n')" = "bootstrap.rs"
            test "$(grep -R -l 'RUST_NAMING_TRANSLATIONS' ${src}/src ${src}/tests | wc -l)" -eq 0
            retired_type="Cargo""SchemaMetadata"
            retired_key="schema""-dir"
            retired_variable="SCHEMA""_DIR"
            ! grep -R -E "$retired_type|$retired_key|$retired_variable" ${src}/src ${src}/tests ${./ARCHITECTURE.md} ${./skills.md}
            grep -q 'pub struct CargoEthosSourceMetadata' ${src}/src/build.rs
            grep -q 'cargo::metadata=ethos-source-dir=' ${src}/src/build.rs
            grep -q 'DEP_{}_ETHOS_SOURCE_DIR' ${src}/src/build.rs
            grep -q 'cargo::rerun-if-env-changed={}' ${src}/src/build.rs
            touch $out
          '';
          doc = craneLib.cargoDoc (commonArguments // {
            inherit cargoArtifacts;
            RUSTDOCFLAGS = "-D warnings";
          });
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (commonArguments // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
        };
        devShells.default = pkgs.mkShell {
          name = "schema-rust";
          packages = [ pkgs.jujutsu toolchain ];
        };
      });
}
