{
  description = "schema-rust — Rust source emitter for TrueSchema";

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
        schemaFilter = path: type:
          type == "regular" && (
            pkgs.lib.hasSuffix ".schema" path
            || pkgs.lib.hasSuffix ".nota" path
          );
        src = rust.cleanSource {
          root = ./.;
          extraFilters = [ schemaFilter ];
        };
        cargoVendorDirectory = craneLib.vendorCargoDeps { inherit src; };
        commonArguments = {
          inherit src cargoVendorDirectory;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArguments;
      in
      {
        packages.default = craneLib.buildPackage (commonArguments // { inherit cargoArtifacts; });
        checks = {
          build = craneLib.cargoBuild (commonArguments // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (commonArguments // { inherit cargoArtifacts; });
          no-old-signal-macro = pkgs.runCommand "schema-rust-no-old-signal-macro" { } ''
            if grep -R "signal_channel!" ${src}; then
              echo "schema-rust must not use the old signal_channel macro" >&2
              exit 1
            fi
            touch $out
          '';
          no-rust-macro-surface = pkgs.runCommand "schema-rust-no-rust-macro-surface" { } ''
            if grep -R -n -E 'macro_rules!|#\[(proc_macro|proc_macro_derive|proc_macro_attribute)\]|proc_macro::' ${src}/src; then
              echo "Rust emission must stay separate from Rust macros in src/" >&2
              exit 1
            fi
            touch $out
          '';
          generated-rkyv-boundary = pkgs.runCommand "schema-rust-generated-rkyv-boundary" { } ''
            grep -R "encode_signal_frame" ${src}/tests/emission.rs >/dev/null
            grep -R "decode_signal_frame" ${src}/tests/emission.rs >/dev/null
            grep -R "rkyv::Archive" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub enum InputRoute" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub enum OutputRoute" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub fn encode_signal_frame" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            touch $out
          '';
          generated-nexus-traits = pkgs.runCommand "schema-rust-generated-nexus-traits" { } ''
            grep -R "emits_schema_plane_engine_traits_for_declared_signal_nexus_and_sema_languages" ${src}/tests/emission.rs >/dev/null
            grep -R "fn on_start(&mut self) -> Result<(), EngineStartFailure>" ${src}/tests/emission.rs >/dev/null
            grep -R "fn on_stop(&mut self) -> Result<(), EngineStopFailure>" ${src}/tests/emission.rs >/dev/null
            grep -R "fn apply_inner(&mut self, input: sema::Sema<sema::WriteInput>)" ${src}/tests/emission.rs >/dev/null
            grep -R "fn observe_inner(&self, input: sema::Sema<sema::ReadInput>)" ${src}/tests/emission.rs >/dev/null
            ! grep -R "pub trait InputNexus" ${src}/tests/fixtures/spirit_generated.rs
            ! grep -R "pub trait OutputNexus" ${src}/tests/fixtures/spirit_generated.rs
            ! grep -R "dispatch_mail_with_nexus" ${src}/tests/fixtures/spirit_generated.rs
            ! grep -R "generated::InputNexus for SpiritNexus" ${src}/tests/emission.rs
            touch $out
          '';
          generated-mail-events = pkgs.runCommand "schema-rust-generated-mail-events" { } ''
            grep -R "pub struct MessageIdentifier(Integer)" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "impl MessageIdentifier" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub struct OriginRoute(Integer)" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "impl OriginRoute" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub mod schema" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub enum Plane" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "Sema(super::Sema<SemaRoot>)" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub mod signal" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub enum MessageRoot" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub struct MessageSent" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub origin_route: OriginRoute" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub short_header: Integer" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub struct MessageProcessed<Reply>" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub trait MessageSentHook" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub trait MessageProcessedHook<Reply>" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            ! grep -R "pub struct NexusMail<Payload>" ${src}/tests/fixtures/spirit_generated.rs
            grep -R "fn generated_signal_roots_emit_typed_message_sent_events" ${src}/tests/emission.rs >/dev/null
            grep -R "event.push_to" ${src}/tests/emission.rs >/dev/null
            touch $out
          '';
          generated-upgrade-traits = pkgs.runCommand "schema-rust-generated-upgrade-traits" { } ''
            grep -R "pub trait UpgradeFrom<Previous>" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "pub trait AcceptPrevious<Previous>" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "UpgradeFrom<PreviousEntry> for generated::Entry" ${src}/tests/emission.rs >/dev/null
            grep -R "accepted previous Entry as" ${src}/tests/emission.rs >/dev/null
            touch $out
          '';
          generated-nota-boundary = pkgs.runCommand "schema-rust-generated-nota-boundary" { } ''
            grep -R "parse::<generated::Input>" ${src}/tests/emission.rs >/dev/null
            grep -R "impl std::str::FromStr for Input" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            grep -R "impl std::fmt::Display for Input" ${src}/tests/fixtures/spirit_generated.rs >/dev/null
            touch $out
          '';
          generated-schema-module-path = pkgs.runCommand "schema-rust-generated-schema-module-path" { } ''
            grep -R "src/schema/lib.rs" ${src}/tests/emission.rs >/dev/null
            grep -R "src/schema/signal/public.rs" ${src}/tests/emission.rs >/dev/null
            grep -R "struct RustModulePath" ${src}/src/lib.rs >/dev/null
            touch $out
          '';
          generated-cross-crate-imports = pkgs.runCommand "schema-rust-generated-cross-crate-imports" { } ''
            grep -R "imported_type_is_referenced_through_a_use_not_redeclared" ${src}/tests/cross_crate_import.rs >/dev/null
            grep -R "pub use marker_core::schema::mail::DatabaseMarker as DatabaseMarker" ${src}/tests/cross_crate_import.rs >/dev/null
            grep -R "Marked(DatabaseMarker)" ${src}/tests/cross_crate_import.rs >/dev/null
            touch $out
          '';
          no-obsolete-asschema-fixtures = pkgs.runCommand "schema-rust-no-obsolete-asschema-fixtures" { } ''
            grep -R "assert_lowers_to_typed_schema_data" ${src}/tests/big_emission.rs >/dev/null
            if find ${src} -name '*.asschema' -print -quit | grep .; then
              echo "obsolete .asschema syntax fixtures must not remain in schema-rust" >&2
              exit 1
            fi
            if find ${src} -name '*.witness.txt' -print -quit | grep .; then
              echo "line-format .witness.txt goldens must not remain in schema-rust" >&2
              exit 1
            fi
            if grep -R -n -E '\[Input \[|\[Output \[|\(Struct \[|\(Enum \[|\(Newtype \[|\(Map \[\(Plain|\(Carries \(Plain' ${src}/tests ${src}/src; then
              echo "obsolete ASSchema vector-record syntax must not remain in active Rust-emission code or fixtures" >&2
              exit 1
            fi
            touch $out
          '';
          no-nested-root-enum-examples = pkgs.runCommand "schema-rust-no-nested-root-enum-examples" { } ''
            if grep -R -n -E '\((Input|Output) \(\(' ${src}/tests; then
              echo "root Input/Output examples must use direct variants, not nested enum bodies" >&2
              exit 1
            fi
            if grep -R -n -E '@(Vec|Option|KeyValue|Bag|HashSet)' ${src}/tests ${src}/src; then
              echo "schema-rust examples must not reintroduce the old @ macro sigil" >&2
              exit 1
            fi
            if grep -R -n -E '\[\[[A-Z]|\((records|kinds|services|Listed) \[[A-Z]|\((byTopic|Projected|nodes) \{[A-Z]' ${src}/tests/fixtures; then
              echo "schema-rust examples must use dotted generic references such as List.T, Maybe.T, Dictionary.(K V)" >&2
              exit 1
            fi
            if grep -R -n -E --include='*.schema' '\((Vector|Optional|ScopeOf|Map|Bytes|FixedBytes|Vec|Option|KeyValue)[[:space:]]' ${src}/tests/fixtures; then
              echo "schema-rust examples must not reintroduce parenthesized generic application" >&2
              exit 1
            fi
            if grep -R -n -E --include='*.schema' '\([A-Z][A-Za-z0-9_]*\)' ${src}/tests/fixtures; then
              echo "schema-rust examples must not reintroduce same-named self-tag enum payloads" >&2
              exit 1
            fi
            if grep -R -n -E --include='*.schema' 'Map\.[A-Za-z][A-Za-z0-9_]*\.[A-Za-z]' ${src}/tests/fixtures; then
              echo "schema-rust multi-argument generics must stay structural after the dot" >&2
              exit 1
            fi
            if grep -R -n -E '\((Vec|Option|KeyValue|Map) \[' ${src}/tests; then
              echo "schema-rust examples must not put raw vectors inside composite type constructors" >&2
              exit 1
            fi
            if grep -R -n -E '[A-Za-z][A-Za-z0-9]*\*' ${src}/tests/fixtures; then
              echo "schema-rust examples must not reintroduce star-suffix same-name payload sugar" >&2
              exit 1
            fi
            touch $out
          '';
          no-production-free-functions = pkgs.runCommand "schema-rust-no-production-free-functions" { } ''
            if find ${src}/src -path ${src}/src/bin -prune -o -type f -name '*.rs' -print0 \
              | xargs -0 grep -n -E '^(pub(\([^)]*\))? )?fn '; then
              echo "production Rust must not use module-level free functions outside binary main entrypoints" >&2
              exit 1
            fi
            touch $out
          '';
          no-production-unit-structs = pkgs.runCommand "schema-rust-no-production-unit-structs" { } ''
            if grep -R -n -E '^struct [A-Za-z][A-Za-z0-9_]*;' ${src}/src; then
              echo "production Rust must not use unit structs as namespace/method holders" >&2
              exit 1
            fi
            touch $out
          '';
          generated-no-free-functions = pkgs.runCommand "schema-rust-generated-no-free-functions" { } ''
            if grep -R -n --include='*.generated.rs' -E '^(pub(\([^)]*\))? )?fn ' ${src}/tests/fixtures; then
              echo "generated Rust fixture must not use module-level free functions" >&2
              exit 1
            fi
            touch $out
          '';
          generated-no-legacy-helper-surface = pkgs.runCommand "schema-rust-generated-no-legacy-helper-surface" { } ''
            ! grep -R --include='*.generated.rs' "parse_nota_root" ${src}/tests/fixtures
            ! grep -R --include='*.generated.rs' "UnknownHeader { surface" ${src}/tests/fixtures
            ! grep -R "pub struct RustEmitter;" ${src}/src
            ! grep -R --include='*.generated.rs' "pub struct NotaSource" ${src}/tests/fixtures
            ! grep -R --include='*.generated.rs' "pub struct NotaBlock" ${src}/tests/fixtures
            ! grep -R --include='*.generated.rs' "pub struct NotaCollection" ${src}/tests/fixtures
            grep -R --include='*.generated.rs' "pub use nota" ${src}/tests/fixtures >/dev/null
            grep -R --include='*.generated.rs' "derive(nota::NotaDecode, nota::NotaDecodeTraced, nota::NotaEncode" ${src}/tests/fixtures >/dev/null
            ! grep -R --include='*.generated.rs' "impl NotaDecode for Input" ${src}/tests/fixtures
            ! grep -R --include='*.generated.rs' "impl NotaEncode for Input" ${src}/tests/fixtures
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
          packages = [ pkgs.jujutsu pkgs.pkg-config toolchain ];
        };
      });
}
