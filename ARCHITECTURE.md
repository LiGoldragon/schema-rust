# Architecture

**Frozen donor, dead under its old name.** This repo was ruled dead under its
old name 2026-07-27 (S1R entry 7): the language is Ethos, and legacy schema,
schema-language, and schema-rust die under their old names. It gains no new
consumers. Everything below describes what this frozen donor currently
implements, not the approved architecture going forward under the Ethos
name.

`schema-rust` emits Rust interface source from typed schema data and owns the
shared build-driver orchestrator for generated schema modules. This repository
owns the Rust code-generation step and that orchestrator; it does not define
schema semantics, which now come from the build-time `schema-language` crate. Rust emission is a separate step
from Rust macros: schema generates Rust code first, and macros are a later or
separate consumption surface. Generated Rust is emitted into the consumer crate
source tree under `src/schema/`, not hidden in `OUT_DIR`, so source-visible
generated interfaces stay reviewable and can become committed or
freshness-checked build artifacts.

Its source-facing input is `schema_language::SchemaSource`, the schema-in-Rust value
produced when authored `.schema` deserializes into Rust datatypes that fully
define the schema and serialize through rkyv. Its semantic emission input is
`schema_language::TrueSchema`, the canonical decoded semantic layer; there is no older
assembled-schema artifact or parallel semantic model beside that typed
source/true-schema pipeline.

## Interfaces

- `RustEmitter` is the code-generation engine.
- `RustTrueSchemaLowering` is the entry trait implemented for
  `schema_language::TrueSchema`. The deserialized semantic schema object owns the
  lowering call; the emitter supplies policy such as target, NOTA surface, and
  generator name. The trait lives in this repository rather than in `schema`
  because `schema` owns schema semantics and must not depend on Rust emission.
- `RustSchemaSourceLowering` is the trait implemented for
  `schema_language::SchemaSource`. It lowers typed source through `SchemaEngine` into
  `TrueSchema` and then through `RustTrueSchemaLowering`.
- `LowerToRust<Target>` is the recursive projection trait implemented for the
  schema subobjects that own each piece of the Rust model: imports,
  declarations, type declarations, aliases, newtypes, structs, fields, enums,
  variants, and support metadata. The top-level lowering traits are facades
  over these per-noun projections, not a place where schema meaning is
  reassembled centrally.
- The Rust renderer uses `proc_macro2::TokenStream` and `quote` through
  context-carrying token wrappers over Rust-model nouns. Plain `ToTokens` has no
  context parameter, so generation-wide switches such as the NOTA feature gate
  and private-type visibility flow through `RustRenderContext` wrappers instead
  of being copied into every noun.
- The string-to-token migration is complete. Every emitted section — the
  declaration surface (aliases, newtypes, structs, fields, enums, variants),
  the NOTA bridges, the enum payload `From` impls and variant constructors, the
  signal-frame binary codec, the route enums/impls, the short-header module, the
  cross-plane `into_*` projections, the runtime role-trait impls, the upgrade
  trait surface, and the runtime/plane/runner support — renders itself through a
  `ToTokens` wrapper noun (e.g. `RustScalarAliasTokens`, `NewtypeInherentImplTokens`,
  `NotaInherentBridgeTokens`, `SignalFrameImplTokens`, `RouteEnumTokens`,
  `RouteImplTokens`, `ShortHeaderModuleTokens`, `EnumVariantConstructorsTokens`).
  These are routed through one `prettyplease` pass at `emit_item_tokens`. No
  emitter code builds Rust source with `format!`/`self.line`; the only direct
  text written is the leading `// @generated` header comment, which cannot pass
  through `prettyplease` (it drops non-doc comments).
- Cross-object emission logic is named rather than smeared across a god-struct.
  `ShortHeader` owns the per-root-per-variant route-triage constant (name +
  packed value), shared by the `short_header` module and the frame codec so the
  constant names always agree. `RouteName` owns the `<Name>Route` identifier.
  `ScreamingName` owns the SCREAMING_SNAKE constant projection. The split-plane
  Nexus projection builds its match arms through `split_signal_arrived_arms` and
  `split_output_arms`, which carry the exact-then-fallback variant-matching
  logic as named arm builders.
- Runtime plane naming has a three-tier boundary. `Plane` owns only
  plane-intrinsic names such as the module, wrapper, export aliases, engine
  trait, and trace enum names. `RuntimePlaneSet` / `RustEmissionTarget` own
  which planes a target emits. Schema-presence and construct-presence checks
  stay on the emitter nouns that inspect declarations and roots. Do not move
  target selection or schema logic into `Plane`.
- `RustModule` is the data model between semantic schema data and rendered
  Rust text. It carries scalar aliases, cross-crate imports, generated Rust
  declarations, root enums, and support metadata before anything is rendered.
  `RustModule::render` drives `RustModuleRenderer`, which owns the emission
  context and the schema-analysis predicates and accumulates the pretty-printed
  source as each section's `ToTokens` noun renders itself. The renderer builds
  no Rust as strings.
- `RustCode` is the generated source text.
- `GeneratedFile` names a generated path plus source text.
- `RustModulePath` maps single-colon schema identities to crate-local generated
  module paths. The crate namespace segment is dropped; `lib` becomes
  `src/schema/lib.rs`, and nested modules become files under `src/schema/`.
- `build::GenerationDriver` is the shared build-script orchestrator for
  source-visible generated schema modules. It owns the per-crate
  load/lower/emit/freshness sequence so component `build.rs` files do not
  hand-roll it.
- `src/bin/schema-rust.rs` is the thin command surface over the same driver.
  It accepts exactly one NOTA request, loads the selected modules through
  `SchemaEnvironment`, calls
  `GenerationDriver::generate_from_environment`, and prints typed generation
  feedback over the selected canonical source and generated Rust artifacts.
- `build::GenerationPlan` names the crate package, target modules, and
  dependency schema directories. `build::ModuleEmission` selects the
  Rust-emission target for each schema module.
- `build::ContractCrateBuild` is the published `signal-*` /
  `meta-signal-*` crate driver. It owns the standard contract-crate
  build-script facts — crate root, crate name, schema version, Cargo `links`
  metadata name, module name, and freshness environment variable — then emits
  the standalone `WireContract` module and publishes the crate's `schema/`
  directory for daemon/runtime imports.
- `daemon_emit::DaemonModule` is the `triad_main!` emitter (designer report
  542): a per-component, source-visible `src/schema/daemon.rs`. It is OFF by
  default and turns ON only when a component declares a
  `daemon_emit::NexusDaemonShape` carrying the OS process name, working
  listener tier, and optional meta listener tier. The async task-backed slice emits
  the uniform daemon skeleton:
  the `ComponentDaemon` hook trait (the 1488 escape hatches),
  `DaemonCommand` argv parsing, the `GeneratedDaemonRuntime`
  async decode->execute->encode working spine, the generated listener-identity
  enum for multi-listener shapes, `AsyncConnectionRuntime` over
  `triad_runtime::AsyncSingleListenerDaemon` for working-only daemons,
  `AsyncMultiConnectionRuntime` over
  `triad_runtime::AsyncMultiListenerDaemon` for working + meta daemons,
  `DaemonError`, and the `ExitReport`-based
  `DaemonEntry::run_to_exit_code`. The old synchronous
  `{Single,Multi}ListenerDaemon` selection and raw `UnixStream` meta hook are
  gone from daemon emission. The current meta tier remains an async task-backed
  `AcceptedConnection` escape hatch because the shape does not yet name a
  meta-signal contract path; once the shape carries that path, the meta branch
  should lower to the same typed frame spine as working traffic. If the schema
  declares streams, the working branch stays async task-backed and adds emitted
  subscription support: `AcceptedConnection::into_parts`, Tokio stream
  splitting, retained owned writer halves, `triad_runtime::SubscriptionRegistry`,
  and `SubscriptionEventPublisher` for length-prefixed pushed events. Each
  emitted section renders itself as a `ToTokens` noun through
  `RustModuleRenderer`, matching the rest of the crate — the daemon emitter
  builds no Rust as strings.
  `WorkingListenerTier::component_decoded()` is the narrow relation-adapter
  form for daemons whose ordinary socket must preserve more than one legacy
  public contract while those contracts migrate to schema roots. In that form
  the generated daemon still owns process argv, async task-backed socket binding,
  per-listener request admission, peer credentials, lifecycle, and exit
  handling; only the accepted working connection's frame dialect is handed to a
  component hook. Generated binders apply `BindingSurface::socket_mode()`
  to the ordinary socket and the configured meta mode to the meta socket, so
  security-sensitive chmod behavior stays in the shared daemon path.

## Source Input And True Schema

The public source-facing contract is typed schema source data decoded through
structural macro node codecs: authored `.schema` deserializes into
schema-defining Rust datatypes, those datatypes are rkyv-serializable, and this
emitter lowers that schema-in-Rust value into Rust interface code.
`RustEmitter::emit_file_from_schema_source` and
`emit_module_from_schema_source` are the build-driver path. They ask
`SchemaEngine` to lower source into `TrueSchema`, then render `RustModule`.
Callers that already hold the semantic value use `TrueSchema`'s
`RustTrueSchemaLowering` trait methods (`lower_to_rust_file`,
`lower_to_rust_code`, `lower_to_rust_module`) or the emitter's
`emit_*_from_true_schema` methods. Those methods build a
`RustLoweringContext` and recursively ask schema subobjects to lower
themselves into Rust-model nouns. No generated component path reads or writes a
separate assembled-schema text file.

The rendered source is `RustModule::render()`, so tests can inspect the module
data shape before comparing generated source text. Each namespace entry carries
a typed `Visibility` on its declaration, and the emitter must project that
boundary into Rust instead of flattening every type into the same public
surface. The model holds visibility as typed data, not the retired
`(Public Name Value)` / `(Private Name Value)` parenthesized tag. Authored
type-privacy — a schema author declaring a shared private type — is DEFERRED by
psyche pending a future programmable-macros discussion, so the checked-in
fixtures are all public; the `pub(crate)` projection path below is retained for
when that authoring surface returns.

The active fixtures use the strict positional dotted schema shape: an enum body
is a square-bracket vector of variants, unit variants are bare capitalized
symbols, and data-carrying variants are dotted `Variant.Payload`. There is no
same-named self-tag shortcut: a variant whose payload type shares its name is
written explicitly (`Record.Record`). The legacy same-named `(Record)` self-tag
and the parenthesized `(Variant PayloadType)` form are retired from the source
language (see `schema-language`'s `ARCHITECTURE.md`), and the checked-in fixtures
carry only the dotted forms. Root headers should prefer exported operation names
or shallow inline operation payloads. This emitter sees the typed source/schema
data from `schema-language` and must not grow a second parser for the authored
form.

## Constraints

- No dependency on the old signal macro.
- No `macro_rules!` or proc-macro surface in `src/`.
- No authored-schema macro syntax is accepted as an emitter input. Tests lower
  real `.schema` fixtures into typed `TrueSchema` values before comparing
  generated Rust; no assembled-schema text fixture is accepted.
- Public schema declarations emit public Rust types and fields. Private schema
  declarations emit `pub(crate)` types and fields, preserving inline
  PascalCase schema declarations as module-local implementation nouns.
- `TypeDeclaration::Alias` and `TypeDeclaration::Newtype` are distinct.
  Bare source bindings such as `Topic.String`, `Topics.Vector.Topic`, or
  `Rejected.SignalRejection` emit as Rust `type` aliases. A brace-body
  declaration with exactly one field emits as a tuple newtype.
  `TypeDeclaration::Struct` is the named-field map shape. A newtype in
  `TrueSchema` is a single-element brace carrying just the wrapped type and no
  field name; the current authoring surface is the dotted `Topic.{ String }` —
  a brace body holding one bare type — as the checked-in fixtures write it (for
  example `DecisionReceipt.{ Integer }`). The retired `@`-suffixed `Topic@String`
  form and the parenthesized `(Public Topic { String })` form are no longer
  authored. The generated tuple newtype carries a NOTA-transparent shape so
  `Topic([foo])` and `topic.0` round-trip without a wrapper field name.
- Generated Rust is source-visible under `src/schema/`; consumers include or
  compile that source rather than hiding the interface in `OUT_DIR` or behind a
  compiler macro expansion. The emitter builds every section as `proc_macro2`
  tokens through `quote!` and pretty-prints them into checked source files. The
  each top-level generated item carries `#[rustfmt::skip]`; the `prettyplease`
  projection is the stable generated artifact, and `cargo fmt` is kept focused
  on handwritten Rust instead of rewriting freshness-checked generated files.
  The string-based runtime emitter named as a migration target in Spirit record
  `0bw0` is gone: emission is token-based end to end (see the Interfaces section
  above).
- Emission is tested by source fixture comparison and by compiling the fixture
  as Rust code.
- Same-shape sibling variants consolidate by semantic family. When
  several variants of a closed-sum enum carry identical payload shapes
  (the common case for rkyv-wrapped closed sums), the emitter groups
  them under one family-named variant whose body carries the original
  variant name as data, rather than repeating one identical-shape
  variant per name.
- No `Interact` / `InteractionActor` trait surface is emitted. A method
  call on an actor *is* the interaction, so the emitter produces no
  mediator-interaction trait scaffolding; the mediator pattern survives
  only as ordinary actor-method topology where the method body decides.
  What stays is the two-languages distinction (internal effect vocabulary
  vs external wire vocabulary), effect-table match-driven dispatch, and
  actor fan-out expressed as method returns.
- `RustEmissionTarget::WireContract` emits an external signal contract
  wire surface: schema nouns, derives, NOTA/rkyv codecs, short-header route
  constants, and the universal `signal-frame` request/reply surface (`Frame`,
  `FrameBody`, `Request`, `ReplyEnvelope`, `RequestBuilder`,
  `RequestPayload`, `SignalOperationHeads`, `Input::into_frame`, and
  `Output::into_reply_frame`). It does not emit runtime envelopes, engine
  traits, mail support, trace support, or subscription-event push unless the
  schema declares a stream.
- `RustEmissionTarget::SignalRuntime` emits daemon-local Signal runtime
  support over signal roots: the Signal envelope, origin route, mail lifecycle
  nouns, Signal trace object names, and `SignalEngine`. The engine bridge uses
  associated Nexus input/output types so the Signal schema does not import
  daemon-internal Nexus vocabulary.
- `RustEmissionTarget::NexusRuntime` emits daemon-side Nexus support only:
  Nexus envelope, Nexus route/trace vocabulary, and `NexusEngine`. Generated
  `NexusEngine::execute` returns a future, generated SEMA/effect hooks return
  futures, and the generated runner adapter awaits
  `triad_runtime::Runner::drive`; components do not wrap generated Nexus
  execution in blocking pools.
- `RustEmissionTarget::SemaRuntime` emits daemon-side SEMA support only: SEMA
  envelope, SEMA route/trace vocabulary, and `SemaEngine`. SEMA write and read
  halves emit independently, so a read-only SEMA schema still gets
  `observe`. Split SEMA schemas may use plane-local operation root names
  (`WriteInput`, `WriteOutput`, `ReadInput`, `ReadOutput`) instead of the old
  all-in-one backing names (`SemaWriteInput`, `SemaWriteOutput`,
  `SemaReadInput`, `SemaReadOutput`); the generated public namespace remains
  `sema::WriteInput`, `sema::WriteOutput`, `sema::ReadInput`, and
  `sema::ReadOutput`.
- Runtime plane schemas live as schema files inside the daemon crate, such as
  `cloud/schema/nexus.schema` and `cloud/schema/sema.schema`, and may import
  contract roots when daemon logic needs the external wire vocabulary. Runtime
  code implements generated Nexus and SEMA traits on data-bearing engine
  objects.
- `RustEmissionTarget::ComponentRuntime` is the compatibility/bootstrap target
  for unsplit all-in-one schemas. It emits the old combined Signal/Nexus/SEMA
  runtime support, including the generic plane enum and cross-plane
  projections. New daemon schemas use the per-plane targets instead.
- Build scripts use the shared driver rather than local emit loops. An external
  signal contract crate uses `build::ContractCrateBuild`, which emits
  `schema/lib.schema` through `RustEmissionTarget::WireContract`, writes/checks
  `src/schema/lib.rs`, and publishes `cargo::metadata=schema-dir=...` for
  importers. A daemon crate uses `GenerationPlan::daemon_runtime`, which emits
  `schema/nexus.schema` through `RustEmissionTarget::NexusRuntime` and
  `schema/sema.schema` through `RustEmissionTarget::SemaRuntime`; daemon crates
  that carry a local Signal runtime module add
  `ModuleEmission::signal_runtime_module("signal")` explicitly. The shared
  runtime module builders use the same feature-gated `nota-text` surface as
  contracts: normal binary daemon builds keep `nota` absent, while
  all-feature trace/testing builds can round-trip generated runtime support
  nouns such as `NexusObjectName` and `SemaObjectName`. An unsplit bootstrap
  schema uses
  `GenerationPlan::component_runtime_compatibility`, keeping
  `RustEmissionTarget::ComponentRuntime` explicit until the schema is split.
- Cross-crate imports in daemon runtime schemas come from Cargo-exposed
  dependency schema directories. Dependency crates publish their `schema/`
  directory as build metadata, and consumers register those paths as
  `build::DependencySchema` entries before lowering runtime modules.
- `build::CargoSchemaMetadata` owns both sides of that Cargo seam. Contract
  crate build scripts call `emit_schema_directory` after a successful
  freshness check; daemon build scripts read the corresponding
  `DEP_<LINKS>_SCHEMA_DIR` value through `DependencySchema::from_cargo_metadata`.
- Driver freshness is source-visible and committed. Authored `.schema` input
  is parsed into `SchemaSourceArtifact` and round-tripped through generated
  schema text plus rkyv archive bytes as internal codec witnesses, but it is
  not treated as generated output. Generated Rust files are checked against the
  working tree; a component-specific update environment variable rewrites them
  when the schema intentionally changes. The shared component driver does not
  write or freshness-check any intermediate schema artifact.
- Signal, Nexus, and SEMA roots are emitted from the same schema shape:
  imports/exports, input, output, and namespace. Emission may attach different
  support traits per plane, but the generated Rust mirrors the same authored
  schema structure.
- Plane namespaces are emitted only for the selected runtime plane.
  `nexus::Work`, `nexus::Action`, `sema::WriteInput`, and `sema::ReadInput`
  are the public shape for plane-local payloads; the current flat backing
  names are a bootstrap detail until schema files split fully by plane.
- Single-colon schema namespaces map to generated Rust module paths. The
  schema path `spirit-next:nexus:Mail` becomes a module/type path under
  `src/schema/` without inventing a second naming system.
- Cross-crate schema imports are emitted as Rust aliases, not local
  re-declarations. If `schema-language` resolves `DatabaseMarker` from
  `marker-core:mail:DatabaseMarker`, this emitter writes a `pub use
  marker_core::schema::mail::DatabaseMarker as DatabaseMarker;` line and local
  fields or variants reference that alias. The dependency crate owns the
  imported type's rkyv/NOTA implementations; the consumer only bridges imported
  decode errors into its own generated error type.
- Generated schema objects emit `UpgradeFrom<Previous>` and
  `AcceptPrevious<Previous>` trait surfaces. A changed type gets hand-written
  upgrade behavior on the generated noun; unchanged types do not need upgrade
  logic.
- A schema that declares record families emits the version-control surface;
  a schema without families emits nothing new. Lowering builds a
  `RustVersionedStore` (store name from the schema identity's component name,
  one `RustRecordFamily` per declaration) and computes each family's identity
  at generation time as the blake3 content hash of
  `TrueSchema::family_closure(record)`. Emission renders the `family_identity`
  module (one SCREAMING_SNAKE 32-byte constant per family, on the
  `short_header` module precedent), the typed `RecordFamilyError` enum, and
  the closed `RecordFamily` sum — one variant per family carrying the record
  type — whose impl owns `STORE_NAME`, `versioning_policy()`, one snake_case
  per-family constructor returning `sema_engine::TableDescriptor` (Domain key)
  or `sema_engine::IdentifiedTableDescriptor` (Identified key), and
  `decode(&sema_engine::FamilyIdentity, &[u8])`. Decode dispatches on the
  family name, rejects schema-hash drift and unknown families with typed
  errors, and rkyv-decodes the record payload. The constructors attach to the
  `RecordFamily` sum rather than to each record type so imported record types
  (Rust alias re-exports, which cannot take inherent impls) participate
  identically. Generated paths reference the real `sema_engine` crate on the
  signal-frame precedent, so declaring families adds `sema-engine` to the
  consumer crate's dependencies. Freshness needs no new machinery: the pinned
  constants are part of the generated artifact the build driver already
  checks, so a schema edit that moves a family closure fails freshness until
  regeneration.
- The emitted frame codec carries a refusal surface beside the per-root
  codecs: `EngineRefusal` (reason plus rendered detail text) encodes under the
  reserved all-ones short header — a value no declared variant can occupy
  because real headers pack indices into the top sixteen bits — and
  `decode_signal_frame` returns it as the typed
  `SignalFrameError::EngineRefused`. The emitted working spine answers every
  decoded request with a complete frame: the ordinary output, or that refusal
  when the engine fails, so a caller never has to read a closed socket as
  possible daemon death.
- Generated signal roots emit rkyv-derived data types, NOTA text conversion,
  short-header route triage, binary signal-frame encode/decode methods, and
  the universal `signal-frame` request/reply aliases and builders. Non-streaming
  contracts emit `Frame = signal_frame::ExchangeFrame<Input, Output>`.
  Stream-aware signal schemas instead emit
  `Frame = signal_frame::StreamingFrame<Input, Output, Event>` when semantic
  `TrueSchema::streams()` is non-empty and the stream event type is the payload of
  `Output.Event`; only then does the event payload get
  `into_subscription_frame`. This path reads schema stream metadata; it
  does not infer streaming from names alone and it does not route through the
  retired `signal_channel!` macro.
- Bootstrap all-in-one runtime emission emits mail-event nouns.
  `signal::Signal<Root>`, `nexus::Nexus<Root>`, and `sema::Sema<Root>` are the
  automatic envelopes for root objects in each plane; each has an
  `origin_route` field plus the root object.
  `schema::Plane::{Signal,Nexus,Sema}` is the data-carrying match surface for
  code that needs to branch across planes. Per-plane runtime targets emit only
  their own envelope and do not emit the generic three-plane enum.
  `MessageSent` records the message identifier, origin route, root schema type,
  and short header, and pushes through `MessageSentHook` so routers, UI layers,
  or introspection subscribers can react without polling. `MessageProcessed`
  carries the same origin route with the processed reply after Nexus receives
  the SEMA or execution outcome.
- Generated objects are the hand-written behavior surfaces. The emitter must
  not compensate for missing runtime nouns by producing free helper functions.
  If dispatch, upgrade, mail acceptance, or SEMA application needs behavior,
  the generated type exposes a trait or method target and the consumer
  implements it on a data-bearing actor or store object.
- Generated construction is also an object method surface. Tuple newtypes emit
  `new`, `payload`, `into_payload`, and `From<Payload>`. Aliases emit no
  inherent impls because they have no distinct Rust type identity. Enums emit
  variant-named associated constructors (`Input::record(entry)`,
  `SemaWriteOutput::recorded(receipt)`, `Output::rejected(rejection)`) so
  component code does not hand-write nested wrapper constructors. When a
  variant stores a generated newtype wrapper, the constructor accepts the
  wrapper's inner payload and creates the wrapper internally; when a variant
  stores an alias, the constructor accepts the alias target directly.
- The next runner target is generated/programmatic component wiring. The
  emitter should grow a component-runner surface so a daemon binary can reduce
  to a tiny generated call while domain behavior still lives in non-default
  implementations of generated Signal, Nexus, and SEMA engine traits. The
  runner does not move algorithms into `main`; it gives the component a
  schema-defined place to instantiate Signal, Nexus, SEMA, transport, trace,
  and binary configuration surfaces.
- Nexus runner glue is generated when the Nexus action/work vocabulary has an
  exhaustive runner shape: `ReplyToSignal` plus any of `CommandSemaWrite`,
  `CommandSemaRead`, `CommandEffect`, and `Continue`, with the matching
  completion work variants present for storage and effects. The generated code
  emits a total `NexusAction` to `triad_runtime::NextStep` projection, a
  data-bearing `NexusRunnerAdapter`, typed hooks on `NexusEngine` for storage,
  effects, and budget exhaustion, and a runner-backed `execute` wrapper that
  keeps the trace hooks at one entered/decided pair per external request.
  Unknown action variants reject runner emission rather than falling through a
  wildcard. The shared loop itself stays in `triad-runtime`.
- Generated engine traits carry minimal lifecycle hooks. `NexusEngine` and
  `SemaEngine` each emit default no-op `on_start` and `on_stop` methods
  returning typed `EngineStartFailure` and `EngineStopFailure` results. The
  bootstrap all-in-one `ComponentRuntime` target still emits the historical
  `SignalEngine` trait while unsplit schemas exist. These hooks give the
  generated runner and persona supervision a graceful-start/stop address
  without introducing full actor mailbox or runtime-control traits before those
  behaviors are needed.
- The `NexusRuntime` target emits a mutable `NexusEngine` trait for the heavier
  execution and decision plane. The `SemaRuntime` target emits
  `SemaEngine`. Schemas that declare a `SemaWriteInput` / `SemaWriteOutput`
  pair emit the mutable `SemaEngine::apply` path; schemas that declare a
  `SemaReadInput` / `SemaReadOutput` pair emit the shared-reference
  `SemaEngine::observe` path. Tests and runtime code use those generated plane
  traits so Nexus and SEMA take and return routed root messages for their own
  planes.
- Cross-plane projections prefer exact operation names before falling back to a
  unique payload type. That lets a realistic interface carry both
  `Lookup(RecordIdentifier)` and `Remove(RecordIdentifier)` without routing the
  read operation to the write plane, while still allowing semantic output
  bridges such as `Recorded(SemaReceipt)` to become
  `RecordAccepted(SemaReceipt)` when the payload type is unique.
- The engine traits also own testing trace hooks. Per-plane implementors
  provide `decide`, `apply_inner`, and `observe_inner`; the bootstrap
  all-in-one target also provides `triage_inner` and `reply_inner`. Generated
  default wrappers keep the public method names `execute`/`apply`/`observe`
  (plus `triage`/`reply` in the bootstrap target) and call default no-op trace
  hooks around the inner behavior. Those hooks activate typed generated object
  names, not strings: Nexus receives `NexusObjectName`, and SEMA receives
  `SemaObjectName`; the bootstrap target also emits `SignalObjectName`.
  Interface/header names use route enums such as `NexusInputRoute` and
  `SemaReadInputRoute`; actor-boundary names live beside the plane that owns
  them (`NexusObjectName::Entered`, `SemaObjectName::WriteApplied`,
  `SemaObjectName::Stopped`). The generated `ObjectName` enum wraps the emitted
  per-plane names for `TraceEvent` transport. A non-trace consumer gets the
  no-op defaults without linking a parallel instrumentation API.
- Trace remains typed data until the client display boundary. The generated
  `TraceEvent` is the component-specific event noun. Its current emitted shape
  is a transparent tuple newtype over `ObjectName`, so `TraceEvent` serializes
  to the generated object-name NOTA shape instead of a double-wrapped
  one-field struct. The shared
  `triad-runtime` trace client/log/socket objects are generic over that noun.
  The next emitter target is generating the small component adapters that are
  still mechanical today: `TraceEventFrame` for rkyv trace archives,
  `Display for TraceEvent` that renders the generated NOTA value at the
  client edge, and aliases for `TraceLog<TraceEvent>` /
  `TraceClient<TraceEvent>` when a trace surface is emitted. The emitter must
  not generate string-log substrates or a
  trace-on-trace path by default.
- Help/documentation emission comes from typed schema description data. The
  target is a mirror description namespace keyed by fully qualified schema
  symbols, with generated defaults for symbols that have no explicit
  description. Generated help actions or client help output render that typed
  description data at the client edge; they are not hand-written CLI string
  tables. The model is hybrid: `schema-language` owns the rkyv-encodable help
  catalog as schema data, this emitter emits that catalog alongside the
  generated contract with type-attached help accessors so a local text
  client resolves Help without daemon transport, and `schema-daemon`
  indexes the same catalogs by schema identity and content hash as the
  reusable registry and query surface for `mentci`, language-service, and
  other schema-aware clients. Each Help request exposes one structural
  level through schema-emitted nouns and newtypes; primitive scalar
  backing types appear only at scalar-backed leaf boundaries, and
  containers keep their constructor with named element references.
- Mail identifiers, origin routes, and short headers use the generated scalar
  floor (`Integer`) rather than bespoke primitive widths. This keeps the runtime
  mail support closer to schema-authored nouns while the core mail schema is
  still emitted by the support surface.
- Scalar references are explicit schema data. `TypeReference::String`,
  `TypeReference::Integer`, `TypeReference::Boolean`, and `TypeReference::Path`
  emit the scalar aliases (`String = std::string::String`, `Integer = u64`,
  `Boolean = bool`). Binary `rkyv` support is emitted for every consumer; NOTA
  codec derives are an optional text-client surface.
  `Plain(Name)` no longer carries scalar special cases; it names an emitted or
  imported schema type.
- Collection references emit standard Rust collections. Authored schemas use
  dotted positional application: `Vector.Topic`, `Map.(Topic RecordIdentifier)`,
  and `Optional.Topic`. Authored datatype declarations are strict positional
  dotted namespace entries: `Topic.String`, `Entry { topic.Topic }`, and
  `Kind.[Decision Correction]`. Square brackets declare enum bodies at enum
  positions; they are not application syntax. The emitter's Rust type projection
  recurses a `TypeReference` by generic kind, never by string name:
  `Vector` → `Vec<inner>`, `Map` →
  `std::collections::BTreeMap<key, value>` (fully qualified, so no `use` and a
  deterministic key order for rkyv + NOTA), `Optional` → `Option<inner>`. The
  target `TypeReference` mirrors the closed set of generic kinds (single-type,
  multi-type, const, template) rather than one uniform application variant or a
  per-name variant set, so lowering dispatches on kind (see `schema-language`'s
  `ARCHITECTURE.md`).
- Generated code can import `nota`'s shared codec surface and derive
  `nota::NotaDecode` / `nota::NotaEncode` for generated nouns, but
  that surface is selected by `RustEmissionOptions`: always enabled,
  feature-gated for text clients, or disabled for binary-only consumers. When
  NOTA is selected, small inherent bridge methods (`from_nota_block`,
  `to_nota`) stay on the owning noun, but the emitter does not hand-write
  per-type codec trait implementations. It does not emit private `NotaSource`,
  `NotaBlock`, or `NotaCollection` helper types. Its NOTA value shapes stay the
  shared codec shapes: a `Vec` is a square-bracket block `[e1 e2 ...]`, a
  `BTreeMap` is a brace block of `key value` pairs `{k1 v1 ...}`, and an
  `Option` is the atom `None` or the paren `(Some inner)`.
- `RustEmissionOptions` carries `nota_surface` and `target`. The named
  constructors (`::binary_only`, `::feature_gated_nota("...")`,
  `::always_enabled_nota`) set the compatibility target
  `RustEmissionTarget::ComponentRuntime`; callers use `with_target` to select
  `RustEmissionTarget::WireContract` for external signal contract generation,
  `RustEmissionTarget::NexusRuntime` for daemon Nexus
  schemas, and `RustEmissionTarget::SemaRuntime` for daemon SEMA schemas.
  `RustEmissionOptions::default()` and `RustEmitter::default()` both pick
  `NotaSurface::FeatureGated { feature: "nota-text" }` plus
  `ComponentRuntime`. `NotaSurface::Disabled` removes the
  NOTA surface entirely: no derives, no `use nota::*` items, no
  `from_nota_block` / `to_nota` bridges, no root `FromStr` / `Display`
  impls. `NotaSurface::AlwaysEnabled` keeps the older unconditional emission
  for callers (mostly tests) that always want NOTA on.
- NOTA owns those value shapes. Schema owns the type-name keywords that select
  scalar and composite type references in `.schema` files.
- A type used anywhere as a `BTreeMap` key earns the ordering derives
  (`PartialOrd, Ord` plus the archived `#[rkyv(derive(...))]`); value-only and
  non-collection types keep the original derive set. `CollectionScan` decides
  both the collection-codec emission and the map-key derive set, so a
  collection-free schema stays byte-identical to the current scalar-floor
  fixture when collection support changes.
- Integration tests load substantive `.schema` and `.nota` language examples
  from `tests/fixtures/` through `tests/support::FixtureSchema` and
  `FixtureNota`. Inline Rust strings remain for short expected generated-code
  fragments; the actual schema/NOTA input surfaces stay visible as files.
- `MigrationEmitter` in `src/migration.rs` derives upgrade/compatibility code
  from a `schema_language::UpgradeObject`. It emits a Rust module containing `mod
  historical` (the previous-version field shapes needed to read archived records)
  and `mod current` (the next-version field shapes) and `impl From<historical::T>
  for current::T` per changed type, materializing the `FieldMigration` directive.
  Correctness is proved by a Layer-2 rustc-subprocess witness that compiles the
  emitted code, not by a grep test. In the target evolution model, structural
  edits (`AddField`, `ChangeFieldType`, `AddVariant`) change core-ethos bytes
  and emit these historical-to-current `From` implementations, while a `Rename`
  edit touches only the `NameTable` and emits no migration code.
