//! Daemon-module emission — the source-visible `src/schema/daemon.rs`.
//!
//! This is the `triad_main!` emitter from designer report 542: instead of a
//! literal macro, schema-rust emits a per-component, source-visible
//! `src/schema/daemon.rs` carrying the uniform daemon skeleton (the
//! `ComponentDaemon` hook trait, `DaemonCommand` argv parsing, the generated
//! runtime struct + its async decode -> execute -> encode connection spine, and
//! the `ExitReport`-based exit body). The component hand-writes only `impl
//! ComponentDaemon` (the `1488` escape hatches: `Configuration` / `Engine` /
//! `Error` / `PROCESS_NAME` + the required `build_runtime`, plus either the
//! typed working-input handler or an explicitly component-decoded working
//! connection hook) and a schema-side [`NexusDaemonShape`].
//!
//! The async task-backed slice emits the working listener and the optional meta
//! listener through `triad-runtime` async listener shells; the retired
//! synchronous multi-listener and raw `UnixStream` compatibility paths are not
//! emitted.
//!
//! Rust syntax is built as `proc_macro2` token streams through `quote!` and
//! pretty-printed once at the boundary, matching the token-first discipline of
//! the main emitter (`lib.rs`) and `migration.rs`. Each emitted section is its
//! own data-bearing `ToTokens` noun; the daemon emitter builds no Rust as
//! strings. The `// @generated` header is prepended as text because
//! `prettyplease` does not preserve non-doc comments through a parse/unparse
//! round-trip.

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};

use crate::{GeneratedFile, RustCode, RustfmtSkippedItems};

/// The schema-side declaration that turns the daemon emitter ON for a
/// component, sibling to the in-emitter `NexusRunnerShape`.
///
/// It carries the data the design says is *not* derivable from the wire
/// contract (fork 2 of report 542): the OS process name, the working listener
/// tier's contract module, and the optional owner-only meta tier with its
/// socket file mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NexusDaemonShape {
    process_name: String,
    working_tier: WorkingListenerTier,
    meta_tier: Option<MetaListenerTier>,
    upgrade_tier: Option<UpgradeListenerTier>,
    tcp_tier: Option<TcpListenerTier>,
}

impl NexusDaemonShape {
    pub fn new(process_name: impl Into<String>, working_tier: WorkingListenerTier) -> Self {
        Self {
            process_name: process_name.into(),
            working_tier,
            meta_tier: None,
            upgrade_tier: None,
            tcp_tier: None,
        }
    }

    pub fn with_meta_tier(mut self, meta_tier: MetaListenerTier) -> Self {
        self.meta_tier = Some(meta_tier);
        self
    }

    pub fn with_upgrade_tier(mut self, upgrade_tier: UpgradeListenerTier) -> Self {
        self.upgrade_tier = Some(upgrade_tier);
        self
    }

    pub fn with_tcp_tier(mut self, tcp_tier: TcpListenerTier) -> Self {
        self.tcp_tier = Some(tcp_tier);
        self
    }

    pub fn process_name(&self) -> &str {
        &self.process_name
    }

    pub fn working_tier(&self) -> &WorkingListenerTier {
        &self.working_tier
    }

    pub fn meta_tier(&self) -> Option<&MetaListenerTier> {
        self.meta_tier.as_ref()
    }

    pub fn upgrade_tier(&self) -> Option<&UpgradeListenerTier> {
        self.upgrade_tier.as_ref()
    }

    pub fn tcp_tier(&self) -> Option<&TcpListenerTier> {
        self.tcp_tier.as_ref()
    }

    fn has_meta_tier(&self) -> bool {
        self.meta_tier.is_some()
    }

    fn has_upgrade_tier(&self) -> bool {
        self.upgrade_tier.is_some()
    }

    fn has_tcp_tier(&self) -> bool {
        self.tcp_tier.is_some()
    }

    /// A daemon binds more than one listener whenever it declares any
    /// Unix owner-only tier beyond the working listener — meta, upgrade, or
    /// both. The optional TCP working ingress is emitted as a sibling
    /// `TcpListenerDaemon`, not as an `AsyncMultiListenerDaemon` Unix socket.
    fn is_multi_listener(&self) -> bool {
        self.meta_tier.is_some() || self.upgrade_tier.is_some()
    }
}

/// The peer-callable working listener tier.
///
/// Normal components name the contract whose emitted `Input` / `Output` roots
/// the decode -> execute -> encode spine drives. The contract is either emitted
/// locally into this crate's `src/schema` (the common case — spirit, message
/// emit their own `crate::schema::signal`), or consumed from a dependency crate
/// (cloud's triad keeps the working contract in `signal-cloud`, imported as
/// `signal_cloud::schema::lib`).
///
/// `component_decoded` is the narrow transitional escape hatch for daemons
/// whose ordinary socket intentionally accepts more than one legacy relation
/// contract. The generated daemon still owns argv, socket binding,
/// async task-backed accept, request gating, peer credentials, lifecycle, and exit
/// handling; only the per-connection wire dialect is component-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkingListenerTier {
    contract: WorkingContractPath,
}

impl WorkingListenerTier {
    /// A contract emitted locally into `crate::schema::<module>`.
    pub fn new(contract_module: impl Into<String>) -> Self {
        Self {
            contract: WorkingContractPath::Local(contract_module.into()),
        }
    }

    /// A contract consumed from a dependency crate, named by the full Rust path
    /// to the module holding the `Input` / `Output` roots, e.g.
    /// `signal_cloud::schema::lib`.
    pub fn dependency(contract_path: impl Into<String>) -> Self {
        Self {
            contract: WorkingContractPath::Dependency(contract_path.into()),
        }
    }

    /// A generated listener whose accepted working connection is decoded by the
    /// component. This is for relation-adapter components that must preserve
    /// multiple legacy public contracts on one ordinary socket while the
    /// contracts migrate to schema-derived roots.
    pub fn component_decoded() -> Self {
        Self {
            contract: WorkingContractPath::ComponentDecoded,
        }
    }

    /// The path tokens the emitted daemon imports the contract roots from —
    /// `crate::schema::<module>` for a local contract, the verbatim crate path
    /// for a dependency contract.
    pub fn contract_import_path(&self) -> Option<TokenStream> {
        self.contract.import_path()
    }

    pub fn is_component_decoded(&self) -> bool {
        self.contract.is_component_decoded()
    }
}

/// Where the working contract's `Input` / `Output` roots are imported from.
#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkingContractPath {
    /// A locally emitted contract module: `crate::schema::<module>`.
    Local(String),
    /// A dependency-crate contract path, e.g. `signal_cloud::schema::lib`.
    Dependency(String),
    /// The component owns relation-specific frame decoding for the working
    /// connection.
    ComponentDecoded,
}

impl WorkingContractPath {
    fn import_path(&self) -> Option<TokenStream> {
        match self {
            Self::Local(module) => {
                let module = syn::Ident::new(module, Span::call_site());
                Some(quote!(crate::schema::#module))
            }
            Self::Dependency(path) => {
                let path: syn::Path = syn::parse_str(path)
                    .expect("dependency working-contract path is a valid Rust path");
                Some(quote!(#path))
            }
            Self::ComponentDecoded => None,
        }
    }

    fn is_component_decoded(&self) -> bool {
        matches!(self, Self::ComponentDecoded)
    }
}

/// The owner-only meta listener tier: the owner-only socket file mode applied
/// at bind time. The meta wire codec is the component's escape hatch until the
/// meta contract path is represented in the daemon shape — the emitter routes
/// the meta socket to a component-provided `handle_meta_connection` future over
/// a runtime-owned `AcceptedConnection`, not to the retired synchronous
/// `UnixStream` path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetaListenerTier {
    socket_mode: SocketModeBits,
}

impl MetaListenerTier {
    pub fn new(socket_mode: SocketModeBits) -> Self {
        Self { socket_mode }
    }

    pub fn socket_mode(&self) -> SocketModeBits {
        self.socket_mode
    }
}

/// The owner-only upgrade listener tier: the third optional listener, mirroring
/// [`MetaListenerTier`]. It binds a third owner-only socket whose accepted
/// connection routes to a component-provided `handle_upgrade_connection` future
/// over a runtime-owned `AcceptedConnection` — the self-upgrade escape hatch
/// until the upgrade signal contract path is represented in the daemon shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeListenerTier {
    socket_mode: SocketModeBits,
}

impl UpgradeListenerTier {
    pub fn new(socket_mode: SocketModeBits) -> Self {
        Self { socket_mode }
    }

    pub fn socket_mode(&self) -> SocketModeBits {
        self.socket_mode
    }
}

/// An optional TCP working ingress for cross-host/tailnet traffic. TCP has no
/// socket file mode; deployment trust is the configured bind address plus the
/// runtime's typed `PeerIdentity::Tcp`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TcpListenerTier;

impl TcpListenerTier {
    pub const fn new() -> Self {
        Self
    }
}

/// A Unix socket file mode in octal-equivalent bits, e.g. `0o600` owner-only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketModeBits {
    bits: u32,
}

impl SocketModeBits {
    pub const fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub fn bits(self) -> u32 {
        self.bits
    }
}

/// Renders the full `src/schema/daemon.rs` source for a component from its
/// [`NexusDaemonShape`].
pub struct DaemonModule {
    shape: NexusDaemonShape,
    generator_name: String,
}

impl DaemonModule {
    pub fn new(shape: NexusDaemonShape, generator_name: impl Into<String>) -> Self {
        Self {
            shape,
            generator_name: generator_name.into(),
        }
    }

    pub fn to_generated_file(&self) -> GeneratedFile {
        GeneratedFile {
            path: "src/schema/daemon.rs".to_owned(),
            code: RustCode(self.render()),
        }
    }

    /// The generated-file header. Kept as text because `prettyplease` drops
    /// non-doc comments.
    fn header(&self) -> String {
        format!("// @generated by {}\n", self.generator_name)
    }

    /// Build the whole module as one `TokenStream`, then route it through the
    /// `syn::parse2` + `prettyplease` seam exactly like the main emitter's
    /// `emit_item_tokens` and `migration.rs`. Malformed emitted Rust fails
    /// here, at emission time, rather than in the consumer build (fix M2).
    fn render(&self) -> String {
        let body = DaemonModuleBody::new(&self.shape);
        let file = syn::parse2::<syn::File>(body.into_token_stream())
            .expect("generated daemon tokens parse");
        let mut source = self.header();
        source.push_str(&RustfmtSkippedItems::new(file).render());
        source
    }
}

/// The whole daemon-module body as a composition of per-section `ToTokens`
/// nouns. Owns the daemon shape it is rendering against.
struct DaemonModuleBody<'shape> {
    shape: &'shape NexusDaemonShape,
}

impl<'shape> DaemonModuleBody<'shape> {
    fn new(shape: &'shape NexusDaemonShape) -> Self {
        Self { shape }
    }
}

impl ToTokens for DaemonModuleBody<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let imports = DaemonImportsTokens::new(self.shape);
        let hook_trait = ComponentDaemonTraitTokens::new(self.shape);
        let command = DaemonCommandTokens::new();
        let listener_tier = ListenerTierTokens::new(self.shape);
        let binder = DaemonBinderTokens::new(self.shape);
        let transport = WorkingTransportTokens::new(self.shape);
        let runtime = GeneratedDaemonRuntimeTokens::new(self.shape);
        let error = DaemonErrorTokens::new(self.shape);
        let exit = DaemonEntryTokens::new();
        quote! {
            #imports
            #hook_trait
            #command
            #listener_tier
            #binder
            #transport
            #runtime
            #error
            #exit
        }
        .to_tokens(tokens);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DaemonSection {
    ComponentDaemonTrait,
    Command,
    ListenerTier,
    Binder,
    WorkingTransport,
    GeneratedRuntime,
    Error,
    Entry,
}

/// The `use` preamble: the always-present `std`/`thiserror` imports, the
/// async task-backed `triad_runtime` set, and the working contract
/// `Input`/`Output`/`SignalFrameError`.
struct DaemonImportsTokens<'shape> {
    shape: &'shape NexusDaemonShape,
}

impl<'shape> DaemonImportsTokens<'shape> {
    fn new(shape: &'shape NexusDaemonShape) -> Self {
        Self { shape }
    }
}

impl ToTokens for DaemonImportsTokens<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let component_decoded = self.shape.working_tier().is_component_decoded();
        let actor_engine = !component_decoded;
        let has_tcp_tier = self.shape.has_tcp_tier();
        let actor_imports = if actor_engine {
            quote! {
                use triad_runtime::EngineRequestError;
                use triad_runtime::kameo::Actor;
                use triad_runtime::kameo::actor::{ActorRef, Spawn, WeakActorRef};
                use triad_runtime::kameo::error::{ActorStopReason, HookError, SendError};
                use triad_runtime::kameo::message::{Context, Message};
            }
        } else {
            quote! {}
        };
        let working_import = match self.shape.working_tier().contract_import_path() {
            Some(working) => quote! { use #working::{EngineRefusal, Input, Output, SignalFrameError}; },
            None => quote! {},
        };
        let tcp_runtime_import = if has_tcp_tier {
            quote! { AsyncConnectionRuntime, }
        } else {
            quote! {}
        };
        let listener_imports = if self.shape.is_multi_listener() {
            quote! {
                AsyncListenerSocket, AsyncMultiConnectionRuntime,
                AsyncMultiListenerDaemon, AsyncMultiListenerDaemonError, SocketMode,
                #tcp_runtime_import
            }
        } else {
            quote! {
                AsyncConnectionRuntime, AsyncSingleListenerDaemon,
                AsyncSingleListenerDaemonError,
            }
        };
        let tcp_imports = if has_tcp_tier {
            quote! {
                use tokio::net::TcpStream;
                use triad_runtime::TcpListenerDaemon;
            }
        } else {
            quote! {}
        };
        let typed_transport_imports = if component_decoded {
            quote! {}
        } else {
            quote! {
                use tokio::io::AsyncWriteExt;
                use triad_runtime::{FrameBody, FrameError, LengthPrefixedCodec};
            }
        };
        quote! {
            use thiserror::Error;
            use triad_runtime::{
                AcceptedConnection, AsyncListenerError, #listener_imports ArgumentError,
                ComponentArgument, ComponentCommand, BindingSurface, ExitReport,
                RequestErrorLog,
            };

            #actor_imports
            #typed_transport_imports
            #working_import
            #tcp_imports
        }
        .to_tokens(tokens);
    }
}

/// The `ComponentDaemon` hook trait — the only daemon code the component
/// hand-writes (record 1488 escape hatches).
struct ComponentDaemonTraitTokens {
    section: DaemonSection,
    has_meta_tier: bool,
    has_upgrade_tier: bool,
    has_tcp_tier: bool,
    component_decoded: bool,
}

impl ComponentDaemonTraitTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::ComponentDaemonTrait,
            has_meta_tier: shape.has_meta_tier(),
            has_upgrade_tier: shape.has_upgrade_tier(),
            has_tcp_tier: shape.has_tcp_tier(),
            component_decoded: shape.working_tier().is_component_decoded(),
        }
    }
}

impl ToTokens for ComponentDaemonTraitTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::ComponentDaemonTrait);
        let actor_engine = !self.component_decoded;
        let owner_engine_parameter = if actor_engine {
            quote! { engine: &mut Self::Engine }
        } else {
            quote! { engine: &Self::Engine }
        };
        let meta_hook = if self.has_meta_tier {
            quote! {
                /// Run one accepted meta connection. The meta tier is async task-backed,
                /// but this hook remains the explicit component escape hatch until
                /// the daemon shape names the meta signal contract path.
                fn handle_meta_connection(
                    #owner_engine_parameter,
                    connection: AcceptedConnection,
                ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + '_ {
                    async move {
                        let _ = engine;
                        let _ = connection;
                        Ok(())
                    }
                }
            }
        } else {
            quote! {}
        };
        let upgrade_hook = if self.has_upgrade_tier {
            quote! {
                /// Run one accepted upgrade connection. The upgrade tier is async
                /// task-backed; this hook is the component escape hatch for the
                /// owner-only self-upgrade protocol until the daemon shape names the
                /// upgrade signal contract path. Defaults to a no-op like the meta tier.
                fn handle_upgrade_connection(
                    #owner_engine_parameter,
                    connection: AcceptedConnection,
                ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + '_ {
                    async move {
                        let _ = engine;
                        let _ = connection;
                        Ok(())
                    }
                }
            }
        } else {
            quote! {}
        };
        let tcp_address_hook = if self.has_tcp_tier {
            quote! {
                /// The optional TCP working ingress bind address. TCP has no socket
                /// mode; deployment trust is the configured bind address plus the
                /// runtime's typed `PeerIdentity::Tcp`.
                fn tcp_listen_address(
                    configuration: &Self::Configuration,
                ) -> Option<std::net::SocketAddr> {
                    let _ = configuration;
                    None
                }
            }
        } else {
            quote! {}
        };
        let error_bound = if self.component_decoded {
            quote! {
                std::fmt::Display + Send + Sync + 'static
            }
        } else {
            quote! {
                std::fmt::Debug
                    + std::fmt::Display
                    + From<FrameError>
                    + From<SignalFrameError>
                    + From<EngineRequestError>
                    + Send
                    + Sync
                    + 'static
            }
        };
        let staged_lane_types = if self.component_decoded {
            quote! {}
        } else {
            quote! {
                /// The lane one decoded working `Input` runs on. `Immediate` is the
                /// single-turn engine ask every component starts with; `Staged` runs
                /// the three-phase staged turn — stage under the daemon's advance
                /// gate, resolve on the connection task with no engine borrow, then
                /// conclude in one more engine turn. Components without a staged
                /// intake never return `Staged`.
                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub enum WorkingInputLane {
                    Immediate,
                    Staged,
                }

                /// The component-facing stage verdict: the stage turn either
                /// completed the input outright (a read, a refusal, a mode without
                /// staging) or parked a staged advance awaiting external resolution.
                pub enum StagedWorkingTurn<Daemon: ComponentDaemon> {
                    Completed(Output),
                    Awaiting(Box<dyn StagedAdvance<Daemon>>),
                }

                /// One staged advance crossing the daemon spine. `resolve` runs on
                /// the connection task with NO engine borrow — the external wait
                /// (for example a cluster authorization round) — storing its verdict
                /// internally; `conclude` then runs as one fast engine turn and
                /// produces the final `Output`.
                pub trait StagedAdvance<Daemon: ComponentDaemon>: Send {
                    fn resolve<'advance>(
                        &'advance mut self,
                    ) -> std::pin::Pin<
                        Box<dyn std::future::Future<Output = ()> + Send + 'advance>,
                    >;
                    fn conclude<'engine>(
                        self: Box<Self>,
                        engine: &'engine mut Daemon::Engine,
                    ) -> std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                Output = Result<Output, Daemon::Error>,
                            > + Send + 'engine,
                        >,
                    >;
                }
            }
        };
        let staged_lane_hooks = if self.component_decoded {
            quote! {}
        } else {
            quote! {
                /// The lane a decoded working `Input` runs on. The default keeps
                /// every input on the single-turn `Immediate` ask, so components
                /// without a staged intake are unaffected.
                fn working_input_lane(input: &Input) -> WorkingInputLane {
                    let _ = input;
                    WorkingInputLane::Immediate
                }

                /// Stage one working `Input` — the fast first engine turn of the
                /// staged lane. The default completes immediately through
                /// `handle_working_input`, so a component that never returns
                /// `WorkingInputLane::Staged` never stages.
                fn stage_working_input<'connection>(
                    engine: &'connection mut Self::Engine,
                    input: Input,
                    connection: &'connection triad_runtime::ConnectionContext,
                ) -> impl std::future::Future<
                    Output = Result<StagedWorkingTurn<Self>, Self::Error>,
                > + Send + 'connection {
                    async move {
                        Ok(StagedWorkingTurn::Completed(
                            Self::handle_working_input(engine, input, connection).await?,
                        ))
                    }
                }

                /// The component's shared advance gate, when it owns one: the
                /// daemon's staged lane serializes staged turns first-in first-out
                /// through this queue-fair lock, and a component can share the same
                /// gate with its own background passes. `None` lets the runtime own
                /// a private gate.
                fn shared_advance_gate(
                    engine: &Self::Engine,
                ) -> Option<std::sync::Arc<tokio::sync::Mutex<()>>> {
                    let _ = engine;
                    None
                }
            }
        };
        let working_hook = if self.component_decoded {
            quote! {
                /// Run one accepted working connection. Use this only for a daemon
                /// whose ordinary socket must preserve multiple relation-specific
                /// legacy contracts while the public contracts migrate to schema
                /// roots. The generated daemon owns listener mechanics; the
                /// component owns only relation-specific frame decode/encode.
                fn handle_working_connection(
                    engine: &Self::Engine,
                    connection: AcceptedConnection,
                ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + '_;
            }
        } else {
            let working_engine_parameter = if actor_engine {
                quote! { engine: &'connection mut Self::Engine }
            } else {
                quote! { engine: &'connection Self::Engine }
            };
            quote! {
                /// Run one decoded working `Input` through the engine and return the
                /// `Output` root to encode back to the caller.
                ///
                /// `connection` carries the accepted stream's kernel-vouched peer
                /// credentials (uid / gid / pid via `SO_PEERCRED`), so the component can
                /// mint an origin from the operating-system trust boundary rather than
                /// trusting a payload claim. Components that do not classify by origin
                /// take it as `_connection`.
                fn handle_working_input<'connection>(
                    #working_engine_parameter,
                    input: Input,
                    connection: &'connection triad_runtime::ConnectionContext,
                ) -> impl std::future::Future<Output = Result<Output, Self::Error>> + Send + 'connection;
            }
        };
        let tcp_working_hook = if self.component_decoded && self.has_tcp_tier {
            quote! {
                /// Run one accepted TCP working connection. Component-decoded daemons
                /// own their frame protocol, so they also own any transport-specific
                /// TCP handling the schema asks the daemon to expose.
                fn handle_tcp_working_connection(
                    engine: &Self::Engine,
                    connection: AcceptedConnection<TcpStream>,
                ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + '_;
            }
        } else {
            quote! {}
        };
        quote! {
            #staged_lane_types

            /// The component hook surface for the emitted daemon — the only daemon
            /// code the component hand-writes (record 1488 escape hatches).
            ///
            /// The component declares its `Configuration` / `Engine` / `Error` types
            /// and `PROCESS_NAME`, and provides the REQUIRED `build_runtime` (the
            /// emitter cannot know how to open the component's Store/Engine) plus the
            /// typed working-input handler.
            pub trait ComponentDaemon: Sized + 'static {
                type Configuration: BindingSurface;
                type ConfigurationError: std::error::Error;
                type Engine: Send + Sync + 'static;
                type Error: #error_bound;

                const PROCESS_NAME: &'static str;

                /// Load the binary rkyv `Configuration` from the daemon's single argument.
                fn load_configuration(path: &std::path::Path) -> Result<Self::Configuration, Self::ConfigurationError>;

                /// Validate the loaded configuration before any runtime, listener,
                /// or store is built. Components that carry only already-validated
                /// typed configuration keep the default no-op; components with decoded
                /// path records override this hook so bad startup shape fails before
                /// socket preparation or state mutation.
                fn validate_configuration(configuration: &Self::Configuration) -> Result<(), Self::ConfigurationError> {
                    let _ = configuration;
                    Ok(())
                }

                /// Open the component's durable Store and construct its Engine.
                fn build_runtime(configuration: &Self::Configuration) -> Result<Self::Engine, Self::Error>;

                #tcp_address_hook

                /// Lifecycle: called once before the listener serves, once after it stops.
                fn start(engine: &Self::Engine) -> Result<(), Self::Error> {
                    let _ = engine;
                    Ok(())
                }

                fn stop(engine: &Self::Engine) -> Result<(), Self::Error> {
                    let _ = engine;
                    Ok(())
                }

                #working_hook

                #staged_lane_hooks

                #tcp_working_hook

                #meta_hook

                #upgrade_hook
            }
        }
        .to_tokens(tokens);
    }
}

/// `DaemonCommand`: argv -> binary `Configuration` -> the bound daemon. The
/// single-argument rule: exactly one argument, a signal-encoded (rkyv)
/// configuration file. The section carries no per-component data.
struct DaemonCommandTokens {
    section: DaemonSection,
}

impl DaemonCommandTokens {
    fn new() -> Self {
        Self {
            section: DaemonSection::Command,
        }
    }
}

impl ToTokens for DaemonCommandTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::Command);
        quote! {
            /// argv -> binary `Configuration` -> the bound daemon. The single-argument
            /// rule: exactly one argument, a signal-encoded (rkyv) configuration file.
            pub struct DaemonCommand<Daemon: ComponentDaemon> {
                command: ComponentCommand,
                daemon: std::marker::PhantomData<fn() -> Daemon>,
            }

            impl<Daemon: ComponentDaemon> DaemonCommand<Daemon> {
                pub fn from_environment() -> Self {
                    Self {
                        command: ComponentCommand::from_environment(),
                        daemon: std::marker::PhantomData,
                    }
                }

                pub fn from_arguments<Arguments, Argument>(arguments: Arguments) -> Self
                where
                    Arguments: IntoIterator<Item = Argument>,
                    Argument: Into<String>,
                {
                    Self {
                        command: ComponentCommand::from_arguments(arguments),
                        daemon: std::marker::PhantomData,
                    }
                }

                pub fn configuration(&self) -> Result<Daemon::Configuration, DaemonError<Daemon>> {
                    match self.command.signal_file_argument()? {
                        ComponentArgument::SignalFile(file) => {
                            let configuration = Daemon::load_configuration(file.as_path())
                                .map_err(DaemonError::Configuration)?;
                            Daemon::validate_configuration(&configuration)
                                .map_err(DaemonError::Configuration)?;
                            Ok(configuration)
                        }
                        ComponentArgument::InlineNota(_) | ComponentArgument::NotaFile(_) => {
                            Err(DaemonError::Argument(ArgumentError::ExpectedSignalFile))
                        }
                    }
                }

                pub fn run(&self) -> Result<(), DaemonError<Daemon>> {
                    tokio::runtime::Runtime::new()
                        .map_err(DaemonError::Runtime)?
                        .block_on(async {
                            Daemon::bind(self.configuration()?)?
                                .run()
                                .await
                                .map_err(DaemonError::from)
                        })
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// The listener identity enum emitted only for multi-listener daemon shapes.
/// `Working` is always present; `Meta` and `Upgrade` ride their declared tiers.
struct ListenerTierTokens {
    section: DaemonSection,
    is_multi_listener: bool,
    has_meta_tier: bool,
    has_upgrade_tier: bool,
}

impl ListenerTierTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::ListenerTier,
            is_multi_listener: shape.is_multi_listener(),
            has_meta_tier: shape.has_meta_tier(),
            has_upgrade_tier: shape.has_upgrade_tier(),
        }
    }
}

impl ToTokens for ListenerTierTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::ListenerTier);
        if !self.is_multi_listener {
            return;
        }
        let meta_variant = if self.has_meta_tier {
            quote! { Meta, }
        } else {
            quote! {}
        };
        let upgrade_variant = if self.has_upgrade_tier {
            quote! { Upgrade, }
        } else {
            quote! {}
        };
        let meta_display = if self.has_meta_tier {
            quote! { Self::Meta => formatter.write_str("meta"), }
        } else {
            quote! {}
        };
        let upgrade_display = if self.has_upgrade_tier {
            quote! { Self::Upgrade => formatter.write_str("upgrade"), }
        } else {
            quote! {}
        };
        quote! {
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum ListenerTier {
                Working,
                #meta_variant
                #upgrade_variant
            }

            impl std::fmt::Display for ListenerTier {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        Self::Working => formatter.write_str("working"),
                        #meta_display
                        #upgrade_display
                    }
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// The `DaemonBinder` default-method trait: builds the engine and returns the
/// async task-backed listener shell the `DaemonCommand` drives.
struct DaemonBinderTokens {
    section: DaemonSection,
    is_multi_listener: bool,
    meta_tier: Option<MetaListenerTier>,
    upgrade_tier: Option<UpgradeListenerTier>,
    has_tcp_tier: bool,
}

impl DaemonBinderTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::Binder,
            is_multi_listener: shape.is_multi_listener(),
            meta_tier: shape.meta_tier().cloned(),
            upgrade_tier: shape.upgrade_tier().cloned(),
            has_tcp_tier: shape.has_tcp_tier(),
        }
    }
}

impl ToTokens for DaemonBinderTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::Binder);
        let bind_return = if self.is_multi_listener {
            if self.has_tcp_tier {
                quote! {
                    GeneratedMultiAndTcpDaemon<Self>
                }
            } else {
                quote! {
                    AsyncMultiListenerDaemon<GeneratedDaemonRuntime<Self>>
                }
            }
        } else {
            if self.has_tcp_tier {
                quote! {
                    GeneratedSingleAndTcpDaemon<Self>
                }
            } else {
                quote! {
                    AsyncSingleListenerDaemon<GeneratedDaemonRuntime<Self>>
                }
            }
        };
        let meta_socket_push = match self.meta_tier.as_ref() {
            Some(meta_tier) => {
                let bits = meta_tier.socket_mode().bits();
                let socket_mode = syn::LitInt::new(&format!("0o{bits:o}"), Span::call_site());
                quote! {
                    let meta_socket_path = configuration
                        .meta_socket_path()
                        .ok_or(DaemonError::MissingMetaSocket)?
                        .to_path_buf();
                    listener_sockets.push(
                        AsyncListenerSocket::new(ListenerTier::Meta, meta_socket_path)
                            .with_socket_mode(SocketMode::new(#socket_mode)),
                    );
                }
            }
            None => quote! {},
        };
        let upgrade_socket_push = match self.upgrade_tier.as_ref() {
            Some(upgrade_tier) => {
                let bits = upgrade_tier.socket_mode().bits();
                let socket_mode = syn::LitInt::new(&format!("0o{bits:o}"), Span::call_site());
                quote! {
                    let upgrade_socket_path = configuration
                        .upgrade_socket_path()
                        .ok_or(DaemonError::MissingUpgradeSocket)?
                        .to_path_buf();
                    listener_sockets.push(
                        AsyncListenerSocket::new(ListenerTier::Upgrade, upgrade_socket_path)
                            .with_socket_mode(SocketMode::new(#socket_mode)),
                    );
                }
            }
            None => quote! {},
        };
        let local_construction = if self.is_multi_listener {
            quote! {
                let working_socket = AsyncListenerSocket::new(
                    ListenerTier::Working,
                    configuration.socket_path().to_path_buf(),
                );
                let working_socket = match configuration.socket_mode() {
                    Some(socket_mode) => working_socket.with_socket_mode(socket_mode),
                    None => working_socket,
                };
                let mut listener_sockets = std::vec![working_socket];
                #meta_socket_push
                #upgrade_socket_push
                AsyncMultiListenerDaemon::new(
                    listener_sockets,
                    runtime.clone(),
                    RequestErrorLog::new(Self::PROCESS_NAME),
                )
                .with_concurrency_limit(configuration.request_concurrency_limit())
            }
        } else {
            quote! {
                let daemon = AsyncSingleListenerDaemon::new(
                    configuration.socket_path().to_path_buf(),
                    runtime.clone(),
                    RequestErrorLog::new(Self::PROCESS_NAME),
                )
                .with_concurrency_limit(configuration.request_concurrency_limit());
                match configuration.socket_mode() {
                    Some(socket_mode) => daemon.with_socket_mode(socket_mode),
                    None => daemon,
                }
            }
        };
        let construction = if self.has_tcp_tier {
            if self.is_multi_listener {
                quote! {
                    let local = { #local_construction };
                    let tcp_address = Self::tcp_listen_address(&configuration)
                        .ok_or(DaemonError::MissingTcpSocket)?;
                    let tcp = TcpListenerDaemon::new(
                        tcp_address,
                        runtime,
                        RequestErrorLog::new(Self::PROCESS_NAME),
                    )
                    .with_concurrency_limit(configuration.request_concurrency_limit());
                    Ok(GeneratedMultiAndTcpDaemon::new(local, tcp))
                }
            } else {
                quote! {
                    let local = { #local_construction };
                    let tcp_address = Self::tcp_listen_address(&configuration)
                        .ok_or(DaemonError::MissingTcpSocket)?;
                    let tcp = TcpListenerDaemon::new(
                        tcp_address,
                        runtime,
                        RequestErrorLog::new(Self::PROCESS_NAME),
                    )
                    .with_concurrency_limit(configuration.request_concurrency_limit());
                    Ok(GeneratedSingleAndTcpDaemon::new(local, tcp))
                }
            }
        } else {
            quote! {
                Ok({ #local_construction })
            }
        };
        let tcp_wrapper = if self.has_tcp_tier {
            quote! {
                pub struct GeneratedSingleAndTcpDaemon<Daemon: ComponentDaemon> {
                    local: AsyncSingleListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                    tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                }

                impl<Daemon: ComponentDaemon> GeneratedSingleAndTcpDaemon<Daemon> {
                    fn new(
                        local: AsyncSingleListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                        tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                    ) -> Self {
                        Self { local, tcp }
                    }

                    async fn run(self) -> Result<(), DaemonError<Daemon>> {
                        GeneratedTcpPair::new(self.local, self.tcp).run().await
                    }
                }

                pub struct GeneratedMultiAndTcpDaemon<Daemon: ComponentDaemon> {
                    local: AsyncMultiListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                    tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                }

                impl<Daemon: ComponentDaemon> GeneratedMultiAndTcpDaemon<Daemon> {
                    fn new(
                        local: AsyncMultiListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                        tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                    ) -> Self {
                        Self { local, tcp }
                    }

                    async fn run(self) -> Result<(), DaemonError<Daemon>> {
                        GeneratedTcpPair::new(self.local, self.tcp).run().await
                    }
                }

                struct GeneratedTcpPair<Local, Daemon: ComponentDaemon> {
                    local: Local,
                    tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                }

                impl<Local, Daemon> GeneratedTcpPair<Local, Daemon>
                where
                    Daemon: ComponentDaemon,
                    Local: GeneratedLocalDaemon<Daemon>,
                {
                    fn new(
                        local: Local,
                        tcp: TcpListenerDaemon<GeneratedDaemonRuntime<Daemon>>,
                    ) -> Self {
                        Self { local, tcp }
                    }

                    async fn run(self) -> Result<(), DaemonError<Daemon>> {
                        tokio::select! {
                            result = self.local.run_local() => result,
                            result = self.tcp.run() => result.map_err(DaemonError::from),
                        }
                    }
                }

                trait GeneratedLocalDaemon<Daemon: ComponentDaemon> {
                    fn run_local(
                        self,
                    ) -> impl std::future::Future<Output = Result<(), DaemonError<Daemon>>>;
                }

                impl<Daemon: ComponentDaemon> GeneratedLocalDaemon<Daemon>
                    for AsyncSingleListenerDaemon<GeneratedDaemonRuntime<Daemon>>
                {
                    async fn run_local(self) -> Result<(), DaemonError<Daemon>> {
                        self.run().await.map_err(DaemonError::from)
                    }
                }

                impl<Daemon: ComponentDaemon> GeneratedLocalDaemon<Daemon>
                    for AsyncMultiListenerDaemon<GeneratedDaemonRuntime<Daemon>>
                {
                    async fn run_local(self) -> Result<(), DaemonError<Daemon>> {
                        self.run().await.map_err(DaemonError::from)
                    }
                }
            }
        } else {
            quote! {}
        };
        quote! {
            #tcp_wrapper

            /// The bound daemon constructor on the component trait: builds the engine,
            /// wraps it in the generated actor connection runtime, and returns the
            /// async task-backed listener shell the `DaemonCommand` drives. The component
            /// never writes this by hand — it is emitted as a default method on
            /// `ComponentDaemon`.
            pub trait DaemonBinder: ComponentDaemon {
                fn bind(
                    configuration: Self::Configuration,
                ) -> Result<#bind_return, DaemonError<Self>> {
                    let engine = Self::build_runtime(&configuration).map_err(DaemonError::Component)?;
                    let runtime = GeneratedDaemonRuntime::<Self>::new(engine);
                    #construction
                }
            }

            impl<Daemon: ComponentDaemon> DaemonBinder for Daemon {}
        }
        .to_tokens(tokens);
    }
}

/// The working-tier wire transport over one accepted Tokio stream: a
/// length-prefixed envelope around the schema-emitted signal frame codec.
/// Emitted (not imported from a hand-written `transport.rs`) so the daemon
/// spine is self-contained. The section carries no per-component data.
struct WorkingTransportTokens {
    section: DaemonSection,
    component_decoded: bool,
}

impl WorkingTransportTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::WorkingTransport,
            component_decoded: shape.working_tier().is_component_decoded(),
        }
    }
}

impl ToTokens for WorkingTransportTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::WorkingTransport);
        if self.component_decoded {
            return;
        }
        quote! {
            /// The working-tier wire transport over one accepted stream: a
            /// length-prefixed envelope around the schema-emitted signal frame codec.
            struct WorkingTransport<'connection, Stream> {
                connection: &'connection mut AcceptedConnection<Stream>,
            }

            impl<'connection, Stream> WorkingTransport<'connection, Stream>
            where
                Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
            {
                fn new(connection: &'connection mut AcceptedConnection<Stream>) -> Self {
                    Self { connection }
                }

                fn context(&self) -> &triad_runtime::ConnectionContext {
                    self.connection.context()
                }

                async fn read_frame(&mut self) -> Result<Vec<u8>, FrameError> {
                    Ok(LengthPrefixedCodec::default()
                        .read_body_async(self.connection.stream_mut())
                        .await?
                        .into_bytes())
                }

                async fn write_frame(&mut self, frame: Vec<u8>) -> Result<(), FrameError> {
                    LengthPrefixedCodec::default()
                        .write_body_async(
                            self.connection.stream_mut(),
                            &FrameBody::new(frame),
                        )
                        .await?;
                    self.connection.stream_mut().flush().await?;
                    Ok(())
                }
            }
        }
        .to_tokens(tokens);
    }
}

/// The generated runtime struct that owns the engine. Its
/// `handle_connection` is the async decode -> execute -> encode spine.
struct GeneratedDaemonRuntimeTokens {
    section: DaemonSection,
    has_meta_tier: bool,
    has_upgrade_tier: bool,
    has_tcp_tier: bool,
    component_decoded: bool,
}

impl GeneratedDaemonRuntimeTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::GeneratedRuntime,
            has_meta_tier: shape.has_meta_tier(),
            has_upgrade_tier: shape.has_upgrade_tier(),
            has_tcp_tier: shape.has_tcp_tier(),
            component_decoded: shape.working_tier().is_component_decoded(),
        }
    }

    fn is_multi_listener(&self) -> bool {
        self.has_meta_tier || self.has_upgrade_tier
    }
}

impl ToTokens for GeneratedDaemonRuntimeTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::GeneratedRuntime);
        // The working tier routes the engine through a kameo `EngineActor`; only
        // the component-decoded tier keeps the engine shared behind `&self` and
        // owns its own per-connection frame decode.
        let actor_engine = !self.component_decoded;
        if actor_engine {
            self.emit_actor_runtime(tokens);
            return;
        }
        // The remaining path is the component-decoded working tier: the engine
        // stays shared and the component owns the connection hook.
        let working_connection_body = quote! {
            Daemon::handle_working_connection(self.engine.as_ref(), connection).await
        };
        let working_connection_parameter = quote! { connection };
        let tcp_connection_method = if self.has_tcp_tier {
            quote! {
                async fn handle_tcp_working_connection(
                    &self,
                    connection: AcceptedConnection<TcpStream>,
                ) -> Result<(), Daemon::Error> {
                    Daemon::handle_tcp_working_connection(self.engine.as_ref(), connection).await
                }
            }
        } else {
            quote! {}
        };
        let meta_connection_arm = if self.has_meta_tier {
            quote! {
                ListenerTier::Meta => {
                    Daemon::handle_meta_connection(self.engine.as_ref(), connection).await
                }
            }
        } else {
            quote! {}
        };
        let upgrade_connection_arm = if self.has_upgrade_tier {
            quote! {
                ListenerTier::Upgrade => {
                    Daemon::handle_upgrade_connection(self.engine.as_ref(), connection).await
                }
            }
        } else {
            quote! {}
        };
        let lifecycle_methods = quote! {
            async fn start(&self) -> Result<(), Self::Error> {
                Daemon::start(self.engine.as_ref())
            }

            async fn stop(&self) -> Result<(), Self::Error> {
                Daemon::stop(self.engine.as_ref())
            }
        };
        let runtime_impl = if self.is_multi_listener() {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncMultiConnectionRuntime for GeneratedDaemonRuntime<Daemon> {
                    type Listener = ListenerTier;
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        listener: Self::Listener,
                        connection: AcceptedConnection,
                    ) -> Result<(), Self::Error> {
                        match listener {
                            ListenerTier::Working => self.handle_working_connection(connection).await,
                            #meta_connection_arm
                            #upgrade_connection_arm
                        }
                    }
                }
            }
        } else {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncConnectionRuntime for GeneratedDaemonRuntime<Daemon> {
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        connection: AcceptedConnection,
                    ) -> Result<(), Self::Error> {
                        self.handle_working_connection(connection).await
                    }
                }
            }
        };
        let tcp_runtime_impl = if self.has_tcp_tier {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncConnectionRuntime<TcpStream> for GeneratedDaemonRuntime<Daemon> {
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        connection: AcceptedConnection<TcpStream>,
                    ) -> Result<(), Self::Error> {
                        self.handle_tcp_working_connection(connection).await
                    }
                }
            }
        } else {
            quote! {}
        };
        quote! {
            /// The generated runtime struct that owns the engine. Its
            /// `handle_connection` IS the async decode -> execute -> encode spine.
            pub struct GeneratedDaemonRuntime<Daemon: ComponentDaemon> {
                engine: std::sync::Arc<Daemon::Engine>,
            }

            impl<Daemon: ComponentDaemon> GeneratedDaemonRuntime<Daemon> {
                fn new(engine: Daemon::Engine) -> Self {
                    Self {
                        engine: std::sync::Arc::new(engine),
                    }
                }

                async fn handle_working_connection(
                    &self,
                    #working_connection_parameter: AcceptedConnection,
                ) -> Result<(), Daemon::Error> {
                    #working_connection_body
                }

                #tcp_connection_method
            }

            impl<Daemon: ComponentDaemon> Clone for GeneratedDaemonRuntime<Daemon> {
                fn clone(&self) -> Self {
                    Self {
                        engine: self.engine.clone(),
                    }
                }
            }

            #runtime_impl

            #tcp_runtime_impl
        }
        .to_tokens(tokens);
    }
}

impl GeneratedDaemonRuntimeTokens {
    /// Emit the actor-tier runtime: a kameo `EngineActor<Daemon>` owns the
    /// engine, the runtime holds an `ActorRef`, and every request crosses the
    /// mailbox — serialising writes the way a lock did, but without holding a
    /// guard across an `.await`, and handing the engine its `&mut self` for free.
    /// Emitted for every non-component-decoded tier — both non-stream and stream
    /// (the stream handler also returns the published event via `WorkingOutcome`).
    fn emit_actor_runtime(&self, tokens: &mut TokenStream) {
        // The owner-only `MetaConnection` / `UpgradeConnection` messages and the
        // runtime ask-methods share an identical `SendError` translation; emit
        // each through `owner_connection_message` / `owner_connection_method`.
        let meta_message = if self.has_meta_tier {
            Self::owner_connection_message(quote!(MetaConnection), quote!(handle_meta_connection))
        } else {
            quote! {}
        };
        let upgrade_message = if self.has_upgrade_tier {
            Self::owner_connection_message(
                quote!(UpgradeConnection),
                quote!(handle_upgrade_connection),
            )
        } else {
            quote! {}
        };
        let meta_connection_method = if self.has_meta_tier {
            Self::owner_connection_method(quote!(handle_meta_connection), quote!(MetaConnection))
        } else {
            quote! {}
        };
        let upgrade_connection_method = if self.has_upgrade_tier {
            Self::owner_connection_method(
                quote!(handle_upgrade_connection),
                quote!(UpgradeConnection),
            )
        } else {
            quote! {}
        };
        // The working-input message reply is the plain `Output` the engine actor
        // produces under its exclusive `&mut` handler.
        let working_input_reply = quote! { Result<Output, Daemon::Error> };
        let working_input_handler_body = quote! {
            Daemon::handle_working_input(&mut self.engine, message.input, &message.context).await
        };
        // The staged-lane actor messages: `StageWorkingInput` runs the fast first
        // engine turn and either completes the input or hands back the staged
        // advance; `ConcludeWorkingInput` runs the fast concluding turn after the
        // connection task resolved the advance outside the mailbox.
        let stage_completed_wrap = quote! {
            StagedWorkingTurn::Completed(output) => {
                Ok(StagedWorkingReply::Completed(output))
            }
        };
        let conclude_handler_body = quote! {
            message.advance.conclude(&mut self.engine).await
        };
        let staged_reply_ok = quote! { Output };
        let staged_messages = quote! {
            /// The engine actor's stage reply: `Completed` carries the finished
            /// outcome; `Awaiting` carries the staged advance the connection task
            /// resolves before the concluding engine turn.
            pub enum StagedWorkingReply<Daemon: ComponentDaemon> {
                Completed(#staged_reply_ok),
                Awaiting(Box<dyn StagedAdvance<Daemon>>),
            }

            pub struct StageWorkingInput {
                input: Input,
                context: triad_runtime::ConnectionContext,
            }

            impl<Daemon: ComponentDaemon> Message<StageWorkingInput> for EngineActor<Daemon> {
                type Reply = Result<StagedWorkingReply<Daemon>, Daemon::Error>;

                async fn handle(
                    &mut self,
                    message: StageWorkingInput,
                    _context: &mut Context<Self, Self::Reply>,
                ) -> Self::Reply {
                    match Daemon::stage_working_input(
                            &mut self.engine,
                            message.input,
                            &message.context,
                        )
                        .await?
                    {
                        #stage_completed_wrap
                        StagedWorkingTurn::Awaiting(advance) => {
                            Ok(StagedWorkingReply::Awaiting(advance))
                        }
                    }
                }
            }

            pub struct ConcludeWorkingInput<Daemon: ComponentDaemon> {
                advance: Box<dyn StagedAdvance<Daemon>>,
            }

            impl<Daemon: ComponentDaemon> Message<ConcludeWorkingInput<Daemon>> for EngineActor<Daemon> {
                type Reply = #working_input_reply;

                async fn handle(
                    &mut self,
                    message: ConcludeWorkingInput<Daemon>,
                    _context: &mut Context<Self, Self::Reply>,
                ) -> Self::Reply {
                    #conclude_handler_body
                }
            }
        };
        // The runtime's working-connection spine: decode the frame, ask the
        // engine actor over the immediate or staged lane, and answer EVERY
        // decoded request with a complete frame — the ordinary output, or the
        // typed refusal when the engine failed. A closed socket with no reply
        // is indistinguishable from daemon death on the caller side, so the
        // spine never returns an engine error without first writing the
        // refusal frame.
        let working_connection_body = quote! {
                async fn handle_working_connection<Stream>(
                    &self,
                    mut connection: AcceptedConnection<Stream>,
                ) -> Result<(), Daemon::Error>
                where
                    Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
                {
                    let mut transport = WorkingTransport::new(&mut connection);
                    let frame = transport.read_frame().await?;
                    let (_route, input) = Input::decode_signal_frame(&frame)?;
                    let context = *transport.context();
                    let turn = match Daemon::working_input_lane(&input) {
                        WorkingInputLane::Immediate => {
                            match self.engine.ask(WorkingInput { input, context }).await {
                                Ok(output) => Ok(output),
                                Err(error) => Err(Self::engine_ask_refused(error)),
                            }
                        }
                        WorkingInputLane::Staged => {
                            // Staged turns serialize first-in first-out through the
                            // advance gate across all three phases; the wait between
                            // the two engine turns runs HERE, holding no engine
                            // borrow, so the mailbox keeps serving other requests.
                            let _advance_turn = self.advance_gate.lock().await;
                            match self
                                .engine
                                .ask(StageWorkingInput { input, context })
                                .await
                            {
                                Ok(StagedWorkingReply::Completed(output)) => Ok(output),
                                Ok(StagedWorkingReply::Awaiting(mut advance)) => {
                                    advance.resolve().await;
                                    match self
                                        .engine
                                        .ask(ConcludeWorkingInput { advance })
                                        .await
                                    {
                                        Ok(output) => Ok(output),
                                        Err(error) => Err(Self::engine_ask_refused(error)),
                                    }
                                }
                                Err(error) => Err(Self::engine_ask_refused(error)),
                            }
                        }
                    };
                    match turn {
                        Ok(output) => match output.encode_signal_frame() {
                            Ok(reply) => {
                                transport.write_frame(reply).await?;
                                Ok(())
                            }
                            Err(error) => {
                                let refusal = EngineRefusal::unavailable(error.to_string());
                                Self::write_refusal_frame(&mut transport, refusal).await;
                                Err(error.into())
                            }
                        },
                        Err(refused) => {
                            Self::write_refusal_frame(&mut transport, refused.refusal).await;
                            Err(refused.error)
                        }
                    }
                }
        };
        let meta_connection_arm = if self.has_meta_tier {
            quote! { ListenerTier::Meta => self.handle_meta_connection(connection).await, }
        } else {
            quote! {}
        };
        let upgrade_connection_arm = if self.has_upgrade_tier {
            quote! { ListenerTier::Upgrade => self.handle_upgrade_connection(connection).await, }
        } else {
            quote! {}
        };
        let lifecycle_methods = quote! {
            async fn start(&self) -> Result<(), Daemon::Error> {
                // `wait_for_startup_result` needs `Error: Clone`; the
                // borrowing form does not, so the startup error is
                // surfaced through `EngineRequestError` carrying its text.
                self.engine
                    .wait_for_startup_with_result(|result| match result {
                        Ok(()) => Ok(()),
                        Err(HookError::Error(error)) => Err(EngineRequestError::new(
                            format!("engine actor failed to start: {error:?}"),
                        )
                        .into()),
                        Err(HookError::Panicked(_)) => Err(EngineRequestError::new(
                            "engine actor panicked during startup",
                        )
                        .into()),
                    })
                    .await
            }

            async fn stop(&self) -> Result<(), Daemon::Error> {
                let _ = self.engine.stop_gracefully().await;
                self.engine.wait_for_shutdown().await;
                Ok(())
            }
        };
        let runtime_impl = if self.is_multi_listener() {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncMultiConnectionRuntime for GeneratedDaemonRuntime<Daemon> {
                    type Listener = ListenerTier;
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        listener: Self::Listener,
                        connection: AcceptedConnection,
                    ) -> Result<(), Self::Error> {
                        match listener {
                            ListenerTier::Working => self.handle_working_connection(connection).await,
                            #meta_connection_arm
                            #upgrade_connection_arm
                        }
                    }
                }
            }
        } else {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncConnectionRuntime for GeneratedDaemonRuntime<Daemon> {
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        connection: AcceptedConnection,
                    ) -> Result<(), Self::Error> {
                        self.handle_working_connection(connection).await
                    }
                }
            }
        };
        let tcp_runtime_impl = if self.has_tcp_tier {
            quote! {
                impl<Daemon: ComponentDaemon> AsyncConnectionRuntime<TcpStream> for GeneratedDaemonRuntime<Daemon> {
                    type Error = Daemon::Error;

                    #lifecycle_methods

                    async fn handle_connection(
                        &self,
                        connection: AcceptedConnection<TcpStream>,
                    ) -> Result<(), Self::Error> {
                        self.handle_working_connection(connection).await
                    }
                }
            }
        } else {
            quote! {}
        };
        let clone_impl = quote! {
            impl<Daemon: ComponentDaemon> Clone for GeneratedDaemonRuntime<Daemon> {
                fn clone(&self) -> Self {
                    Self {
                        engine: self.engine.clone(),
                        advance_gate: self.advance_gate.clone(),
                    }
                }
            }
        };
        quote! {
            /// The kameo actor that owns the component engine. The mailbox
            /// serialises every request, giving each handler exclusive `&mut`
            /// access to the engine without a component-internal lock.
            pub struct EngineActor<Daemon: ComponentDaemon> {
                engine: Daemon::Engine,
            }

            impl<Daemon: ComponentDaemon> Actor for EngineActor<Daemon> {
                type Args = Self;
                type Error = Daemon::Error;

                async fn on_start(
                    actor: Self::Args,
                    _actor_reference: ActorRef<Self>,
                ) -> Result<Self, Self::Error> {
                    Daemon::start(&actor.engine)?;
                    Ok(actor)
                }

                async fn on_stop(
                    &mut self,
                    _actor_reference: WeakActorRef<Self>,
                    _reason: ActorStopReason,
                ) -> Result<(), Self::Error> {
                    Daemon::stop(&self.engine)
                }
            }

            #[derive(Debug)]
            pub struct WorkingInput {
                input: Input,
                context: triad_runtime::ConnectionContext,
            }

            impl<Daemon: ComponentDaemon> Message<WorkingInput> for EngineActor<Daemon> {
                type Reply = #working_input_reply;

                async fn handle(
                    &mut self,
                    message: WorkingInput,
                    _context: &mut Context<Self, Self::Reply>,
                ) -> Self::Reply {
                    #working_input_handler_body
                }
            }

            #staged_messages

            #meta_message

            #upgrade_message

            /// One refused working turn: the component's typed error for the
            /// daemon-side log plus the wire refusal written to the caller.
            struct RefusedWorkingTurn<DaemonError> {
                error: DaemonError,
                refusal: EngineRefusal,
            }

            /// The generated runtime struct holds an `ActorRef` to the engine
            /// actor. Its `handle_connection` IS the async decode -> ask -> encode
            /// spine; the engine state lives behind the actor mailbox. The advance
            /// gate serializes staged working turns first-in first-out across
            /// their stage, resolve, and conclude phases.
            pub struct GeneratedDaemonRuntime<Daemon: ComponentDaemon> {
                engine: ActorRef<EngineActor<Daemon>>,
                advance_gate: std::sync::Arc<tokio::sync::Mutex<()>>,
            }

            impl<Daemon: ComponentDaemon> GeneratedDaemonRuntime<Daemon> {
                fn new(engine: Daemon::Engine) -> Self {
                    let advance_gate = Daemon::shared_advance_gate(&engine).unwrap_or_default();
                    Self {
                        engine: EngineActor::<Daemon>::spawn(EngineActor { engine }),
                        advance_gate,
                    }
                }

                /// Translate a kameo `SendError` from an engine `ask` into the
                /// refused turn the spine answers with: the component's typed
                /// `Error` for the daemon side, plus the wire refusal the
                /// caller receives. A handler error is the engine rejecting
                /// the request; every mailbox-layer failure is the engine
                /// being unavailable.
                fn engine_ask_refused<Request>(
                    error: SendError<Request, Daemon::Error>,
                ) -> RefusedWorkingTurn<Daemon::Error> {
                    match error {
                        SendError::HandlerError(error) => {
                            let refusal = EngineRefusal::rejected(error.to_string());
                            RefusedWorkingTurn { error, refusal }
                        }
                        SendError::ActorNotRunning(_) => {
                            Self::engine_unavailable("engine actor is not running")
                        }
                        SendError::ActorStopped => {
                            Self::engine_unavailable("engine actor stopped before replying")
                        }
                        SendError::MailboxFull(_) => {
                            Self::engine_unavailable("engine actor mailbox is full")
                        }
                        SendError::Timeout(_) => {
                            Self::engine_unavailable("engine actor request timed out")
                        }
                    }
                }

                fn engine_unavailable(detail: &str) -> RefusedWorkingTurn<Daemon::Error> {
                    RefusedWorkingTurn {
                        error: EngineRequestError::new(detail).into(),
                        refusal: EngineRefusal::unavailable(detail.to_string()),
                    }
                }

                /// Best-effort refusal write: the engine failure outranks any
                /// secondary transport failure, which the caller experiences
                /// as the closed exchange it already handles today.
                async fn write_refusal_frame<Stream>(
                    transport: &mut WorkingTransport<'_, Stream>,
                    refusal: EngineRefusal,
                ) where
                    Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
                {
                    if let Ok(frame) = refusal.encode_signal_frame() {
                        let _ = transport.write_frame(frame).await;
                    }
                }

                #working_connection_body

                #meta_connection_method

                #upgrade_connection_method
            }

            #clone_impl

            #runtime_impl

            #tcp_runtime_impl
        }
        .to_tokens(tokens);
    }

    /// Emit one owner-only connection `Message<T>` impl on `EngineActor` routing
    /// to the named component hook — the shared shape for the meta and upgrade
    /// tiers.
    fn owner_connection_message(message_type: TokenStream, hook: TokenStream) -> TokenStream {
        quote! {
            pub struct #message_type {
                connection: AcceptedConnection,
            }

            impl<Daemon: ComponentDaemon> Message<#message_type> for EngineActor<Daemon> {
                type Reply = Result<(), Daemon::Error>;

                async fn handle(
                    &mut self,
                    message: #message_type,
                    _context: &mut Context<Self, Self::Reply>,
                ) -> Self::Reply {
                    Daemon::#hook(&mut self.engine, message.connection).await
                }
            }
        }
    }

    /// Emit one runtime ask-method that forwards an accepted owner-only
    /// connection to the engine actor and translates the `SendError` — the
    /// shared shape for the meta and upgrade tiers.
    fn owner_connection_method(method: TokenStream, message_type: TokenStream) -> TokenStream {
        quote! {
            async fn #method(
                &self,
                connection: AcceptedConnection,
            ) -> Result<(), Daemon::Error> {
                match self.engine.ask(#message_type { connection }).await {
                    Ok(()) => Ok(()),
                    Err(SendError::HandlerError(error)) => Err(error),
                    Err(SendError::ActorNotRunning(_)) => {
                        Err(EngineRequestError::new("engine actor is not running").into())
                    }
                    Err(SendError::ActorStopped) => {
                        Err(EngineRequestError::new("engine actor stopped before replying").into())
                    }
                    Err(SendError::MailboxFull(_)) => {
                        Err(EngineRequestError::new("engine actor mailbox is full").into())
                    }
                    Err(SendError::Timeout(_)) => {
                        Err(EngineRequestError::new("engine actor request timed out").into())
                    }
                }
            }
        }
    }
}

/// The emitted `DaemonError`: argv, configuration, Tokio runtime, listener,
/// and the component error, plus the `From` conversions.
struct DaemonErrorTokens {
    section: DaemonSection,
    is_multi_listener: bool,
    has_meta_tier: bool,
    has_upgrade_tier: bool,
    has_tcp_tier: bool,
}

impl DaemonErrorTokens {
    fn new(shape: &NexusDaemonShape) -> Self {
        Self {
            section: DaemonSection::Error,
            is_multi_listener: shape.is_multi_listener(),
            has_meta_tier: shape.has_meta_tier(),
            has_upgrade_tier: shape.has_upgrade_tier(),
            has_tcp_tier: shape.has_tcp_tier(),
        }
    }
}

impl ToTokens for DaemonErrorTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::Error);
        let missing_meta_variant = if self.has_meta_tier {
            quote! {
                #[error("daemon meta socket path missing from configuration")]
                MissingMetaSocket,
            }
        } else {
            quote! {}
        };
        let missing_upgrade_variant = if self.has_upgrade_tier {
            quote! {
                #[error("daemon upgrade socket path missing from configuration")]
                MissingUpgradeSocket,
            }
        } else {
            quote! {}
        };
        let missing_tcp_variant = if self.has_tcp_tier {
            quote! {
                #[error("daemon TCP listen address missing from configuration")]
                MissingTcpSocket,
            }
        } else {
            quote! {}
        };
        let multi_listener_error_conversion = if self.is_multi_listener {
            quote! {
                impl<Daemon: ComponentDaemon> From<AsyncMultiListenerDaemonError<Daemon::Error>>
                    for DaemonError<Daemon>
                {
                    fn from(error: AsyncMultiListenerDaemonError<Daemon::Error>) -> Self {
                        match error {
                            AsyncMultiListenerDaemonError::Listener(error) => Self::Listener(error),
                            AsyncMultiListenerDaemonError::Start(error)
                            | AsyncMultiListenerDaemonError::Stop(error) => Self::Component(error),
                        }
                    }
                }
            }
        } else {
            quote! {}
        };
        let single_listener_error_conversion = if !self.is_multi_listener || self.has_tcp_tier {
            quote! {
                impl<Daemon: ComponentDaemon> From<AsyncSingleListenerDaemonError<Daemon::Error>>
                    for DaemonError<Daemon>
                {
                    fn from(error: AsyncSingleListenerDaemonError<Daemon::Error>) -> Self {
                        match error {
                            AsyncSingleListenerDaemonError::Listener(error) => Self::Listener(error),
                            AsyncSingleListenerDaemonError::Start(error)
                            | AsyncSingleListenerDaemonError::Stop(error) => Self::Component(error),
                        }
                    }
                }
            }
        } else {
            quote! {}
        };
        quote! {
            /// The emitted daemon error: argv, configuration, listener, and the
            /// component error. The component's own error rides the `Component` arm.
            #[derive(Debug, Error)]
            pub enum DaemonError<Daemon: ComponentDaemon> {
                #[error("daemon argument error: {0}")]
                Argument(ArgumentError),

                #[error("daemon configuration error: {0}")]
                Configuration(Daemon::ConfigurationError),

                #[error("daemon runtime error: {0}")]
                Runtime(std::io::Error),

                #[error("daemon listener error: {0}")]
                Listener(AsyncListenerError),

                #missing_meta_variant

                #missing_upgrade_variant

                #missing_tcp_variant

                #[error("component error: {0}")]
                Component(Daemon::Error),
            }

            impl<Daemon: ComponentDaemon> From<ArgumentError> for DaemonError<Daemon> {
                fn from(error: ArgumentError) -> Self {
                    Self::Argument(error)
                }
            }

            #multi_listener_error_conversion

            #single_listener_error_conversion
        }
        .to_tokens(tokens);
    }
}

/// The component-agnostic exit body: `DaemonEntry::run_to_exit_code`, called
/// from the component binary's `fn main`. Carries no per-component data.
struct DaemonEntryTokens {
    section: DaemonSection,
}

impl DaemonEntryTokens {
    fn new() -> Self {
        Self {
            section: DaemonSection::Entry,
        }
    }
}

impl ToTokens for DaemonEntryTokens {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        debug_assert_eq!(self.section, DaemonSection::Entry);
        quote! {
            /// The component-agnostic exit body. The component's binary calls
            /// `<SpiritDaemon as DaemonEntry>::run_to_exit_code()` from `fn main`.
            pub trait DaemonEntry: ComponentDaemon {
                fn run_to_exit_code() -> std::process::ExitCode {
                    ExitReport::new(Self::PROCESS_NAME)
                        .from_result(DaemonCommand::<Self>::from_environment().run())
                }
            }

            impl<Daemon: ComponentDaemon> DaemonEntry for Daemon {}
        }
        .to_tokens(tokens);
    }
}
