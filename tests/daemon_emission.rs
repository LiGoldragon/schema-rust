use schema_rust::{
    DaemonModule, MetaListenerTier, NexusDaemonShape, SocketModeBits, TcpListenerTier,
    UpgradeListenerTier, WorkingListenerTier,
};

const OWNER_ONLY_SOCKET_MODE: u32 = 0o600;

fn assert_code_contains(code: &str, expected: &str) {
    let compact = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace() && *character != ',')
            .collect::<String>()
    };
    assert!(
        compact(code).contains(&compact(expected)),
        "generated daemon code must contain {expected:?}\n--- generated ---\n{code}"
    );
}

fn assert_code_excludes(code: &str, unexpected: &str) {
    assert!(
        !code.contains(unexpected),
        "generated daemon code must NOT contain {unexpected:?}"
    );
}

fn single_listener_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal"))
}

fn multi_listener_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal")).with_meta_tier(
        MetaListenerTier::new(SocketModeBits::new(OWNER_ONLY_SOCKET_MODE)),
    )
}

fn component_decoded_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::component_decoded()).with_meta_tier(
        MetaListenerTier::new(SocketModeBits::new(OWNER_ONLY_SOCKET_MODE)),
    )
}

fn upgrade_tier_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal"))
        .with_meta_tier(MetaListenerTier::new(SocketModeBits::new(
            OWNER_ONLY_SOCKET_MODE,
        )))
        .with_upgrade_tier(UpgradeListenerTier::new(SocketModeBits::new(
            OWNER_ONLY_SOCKET_MODE,
        )))
}

fn upgrade_only_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal")).with_upgrade_tier(
        UpgradeListenerTier::new(SocketModeBits::new(OWNER_ONLY_SOCKET_MODE)),
    )
}

fn tcp_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal"))
        .with_tcp_tier(TcpListenerTier::new())
}

fn meta_plus_tcp_shape() -> NexusDaemonShape {
    NexusDaemonShape::new("test-daemon", WorkingListenerTier::new("signal"))
        .with_meta_tier(MetaListenerTier::new(SocketModeBits::new(
            OWNER_ONLY_SOCKET_MODE,
        )))
        .with_tcp_tier(TcpListenerTier::new())
}

#[test]
fn daemon_module_emits_the_component_daemon_hook_trait() {
    let generated = DaemonModule::new(single_listener_shape(), "schema-rust").to_generated_file();

    assert_eq!(generated.path, "src/schema/daemon.rs");
    let code = generated.code.as_str();
    assert_code_contains(code, "#[rustfmt::skip]");
    assert_code_contains(code, "pub trait ComponentDaemon");
    assert_code_contains(code, "type Configuration: BindingSurface");
    assert_code_contains(code, "type Engine: Send + Sync + 'static;");
    assert_code_contains(code, "type Error:");
    assert_code_contains(code, "const PROCESS_NAME: &'static str;");
    assert_code_contains(
        code,
        "fn validate_configuration(configuration: &Self::Configuration) -> Result<(), Self::ConfigurationError>",
    );
    assert_code_contains(
        code,
        "fn build_runtime(configuration: &Self::Configuration) -> Result<Self::Engine, Self::Error>;",
    );
    // The non-stream, non-component-decoded tier is the actor tier: the engine
    // hook takes `&mut Self::Engine` (the actor handler holds `&mut self`).
    assert_code_contains(
        code,
        "fn handle_working_input<'connection>(engine: &'connection mut Self::Engine, input: Input, connection: &'connection triad_runtime::ConnectionContext) -> impl std::future::Future<Output = Result<Output, Self::Error>> + Send + 'connection;",
    );
}

#[test]
fn daemon_module_emits_the_command_and_exit_entry() {
    let generated = DaemonModule::new(single_listener_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "pub struct DaemonCommand<Daemon: ComponentDaemon>");
    assert_code_contains(code, "self.command.signal_file_argument()?");
    assert_code_contains(code, "Daemon::load_configuration(file.as_path())");
    assert_code_contains(code, "Daemon::validate_configuration(&configuration)");
    assert_code_contains(code, "tokio::runtime::Runtime::new()");
    assert_code_contains(code, "pub trait DaemonEntry: ComponentDaemon");
    assert_code_contains(code, "fn run_to_exit_code() -> std::process::ExitCode");
    assert_code_contains(code, "ExitReport::new(Self::PROCESS_NAME)");
}

#[test]
fn single_listener_daemon_emits_the_async_single_listener_spine() {
    let generated = DaemonModule::new(single_listener_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "AsyncSingleListenerDaemon::new(");
    assert_code_contains(
        code,
        ".with_concurrency_limit(configuration.request_concurrency_limit())",
    );
    assert_code_contains(code, "configuration.socket_mode()");
    assert_code_contains(code, "daemon.with_socket_mode(socket_mode)");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> AsyncConnectionRuntime for GeneratedDaemonRuntime<Daemon>",
    );
    assert_code_contains(code, "async fn handle_connection(");
    assert_code_contains(code, "self.handle_working_connection(connection).await");
    // The actor tier owns the engine in a kameo `EngineActor`; the runtime holds
    // an `ActorRef` and crosses the mailbox for each request.
    assert_code_contains(code, "pub struct EngineActor<Daemon: ComponentDaemon>");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> Actor for EngineActor<Daemon>",
    );
    assert_code_contains(code, "engine: ActorRef<EngineActor<Daemon>>");
    assert_code_contains(code, "EngineActor::<Daemon>::spawn(EngineActor { engine })");
    assert_code_contains(
        code,
        "Daemon::handle_working_input(&mut self.engine, message.input, &message.context).await",
    );
    assert_code_contains(
        code,
        "self.engine.ask(WorkingInput { input, context }).await",
    );
    assert_code_contains(code, "read_body_async(self.connection.stream_mut())");
    assert_code_contains(code, "write_body_async(");
    assert_code_contains(code, "ContractMarker::decode_single_request(&frame)");
    assert_code_contains(code, "output.encode_reply_frame(exchange)");
    assert_code_contains(code, "refusal.encode_bound_frame()");
    assert_code_excludes(code, "decode_signal_frame");
    assert_code_excludes(code, "encode_signal_frame");
    // The single-listener async daemon has no sync listener, no meta tier, and
    // no listener-tier enum.
    assert_code_excludes(
        code,
        "impl<Daemon: ComponentDaemon> DaemonRuntime for GeneratedDaemonRuntime<Daemon>",
    );
    assert_code_excludes(code, "UnixStream");
    assert_code_excludes(code, "MultiListenerRuntime");
    assert_code_excludes(code, "pub enum ListenerTier");
    assert_code_excludes(code, "MetaConnection");
}

#[test]
fn meta_listener_tier_emits_the_async_multi_listener_spine() {
    let generated = DaemonModule::new(multi_listener_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "pub enum ListenerTier");
    assert_code_contains(code, "Working");
    assert_code_contains(code, "Meta");
    assert_code_contains(code, "AsyncMultiListenerDaemon::new(");
    assert_code_contains(
        code,
        ".with_concurrency_limit(configuration.request_concurrency_limit())",
    );
    assert_code_contains(code, "AsyncListenerSocket::new(");
    assert_code_contains(code, "configuration.socket_mode()");
    assert_code_contains(code, "working_socket.with_socket_mode(socket_mode)");
    assert_code_contains(code, "SocketMode::new(0o600)");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> AsyncMultiConnectionRuntime for GeneratedDaemonRuntime<Daemon>",
    );
    assert_code_contains(code, "type Listener = ListenerTier;");
    assert_code_contains(
        code,
        "ListenerTier::Working => self.handle_working_connection(connection).await",
    );
    // The actor tier routes the meta connection through the runtime method,
    // which asks the engine actor (serialising meta with working state).
    assert_code_contains(
        code,
        "ListenerTier::Meta => self.handle_meta_connection(connection).await",
    );
    assert_code_contains(code, "pub struct EngineActor<Daemon: ComponentDaemon>");
    assert_code_contains(code, "pub struct MetaConnection");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> Message<MetaConnection> for EngineActor<Daemon>",
    );
    assert_code_contains(
        code,
        "Daemon::handle_meta_connection(&mut self.engine, message.connection).await",
    );
    assert_code_contains(code, "self.engine.ask(MetaConnection { connection }).await");
    assert_code_contains(code, "fn handle_meta_connection(");
    assert_code_contains(code, "MissingMetaSocket");
    assert_code_contains(code, "From<AsyncMultiListenerDaemonError<Daemon::Error>>");
    assert_code_excludes(code, "MultiListenerRuntime");
    assert_code_excludes(code, "AsyncSingleListenerDaemon::new(");
    assert_code_excludes(code, "AsyncConnectionRuntime for GeneratedDaemonRuntime");
    assert_code_excludes(code, "handle_meta_stream");
    assert_code_excludes(code, "UnixStream");
}

#[test]
fn tcp_listener_tier_emits_a_sibling_tcp_working_ingress() {
    let generated = DaemonModule::new(tcp_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "use triad_runtime::TcpListenerDaemon");
    assert_code_contains(code, "use tokio::net::TcpStream");
    assert_code_contains(code, "fn tcp_listen_address(");
    assert_code_contains(code, "MissingTcpSocket");
    assert_code_contains(code, "GeneratedSingleAndTcpDaemon<Self>");
    assert_code_contains(code, "TcpListenerDaemon::new(");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> AsyncConnectionRuntime<TcpStream> for GeneratedDaemonRuntime<Daemon>",
    );
    assert_code_contains(code, "self.handle_working_connection(connection).await");
    assert_code_contains(code, "runtime.clone()");
}

#[test]
fn tcp_listener_tier_composes_with_meta_and_keeps_meta_socket_mode() {
    let generated = DaemonModule::new(meta_plus_tcp_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "GeneratedMultiAndTcpDaemon<Self>");
    assert_code_contains(code, "AsyncMultiListenerDaemon::new(");
    assert_code_contains(code, "TcpListenerDaemon::new(");
    assert_code_contains(code, "ListenerTier::Meta");
    assert_code_contains(code, "SocketMode::new(0o600)");
    assert_code_contains(code, "configuration.meta_socket_path()");
    assert_code_contains(code, "configuration.socket_mode()");
}

#[test]
fn component_decoded_working_tier_delegates_frame_decode_to_component() {
    let generated = DaemonModule::new(component_decoded_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "pub enum ListenerTier");
    assert_code_contains(code, "AsyncMultiListenerDaemon::new(");
    assert_code_contains(code, "fn handle_working_connection(");
    assert_code_contains(
        code,
        "ListenerTier::Working => self.handle_working_connection(connection).await",
    );
    assert_code_contains(
        code,
        "Daemon::handle_working_connection(self.engine.as_ref(), connection).await",
    );
    assert_code_contains(
        code,
        "ListenerTier::Meta => { Daemon::handle_meta_connection(self.engine.as_ref(), connection).await }",
    );
    assert_code_excludes(
        code,
        "use crate::schema::signal::{Input, Output, SignalFrameError};",
    );
    assert_code_excludes(code, "fn handle_working_input");
    assert_code_excludes(code, "LengthPrefixedCodec");
    assert_code_excludes(code, "WorkingTransport");
}

#[test]
fn actor_tier_emits_the_staged_working_lane() {
    // The actor tier carries the staged three-phase working turn: a defaulted
    // lane classifier, the staged-advance object crossing the spine, the two
    // extra engine messages, and the first-in first-out advance gate on the
    // runtime. Components that never return `WorkingInputLane::Staged` keep
    // today's single-turn behavior — every new hook has a default.
    let generated = DaemonModule::new(single_listener_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "pub enum WorkingInputLane");
    assert_code_contains(
        code,
        "fn working_input_lane(input: &Input) -> WorkingInputLane { let _ = input; WorkingInputLane::Immediate }",
    );
    assert_code_contains(code, "pub enum StagedWorkingTurn<Daemon: ComponentDaemon>");
    assert_code_contains(
        code,
        "pub trait StagedAdvance<Daemon: ComponentDaemon>: Send",
    );
    assert_code_contains(code, "fn stage_working_input<'connection>(");
    assert_code_contains(
        code,
        "fn shared_advance_gate(engine: &Self::Engine) -> Option<std::sync::Arc<tokio::sync::Mutex<()>>>",
    );
    assert_code_contains(code, "pub enum StagedWorkingReply<Daemon: ComponentDaemon>");
    assert_code_contains(code, "pub struct StageWorkingInput");
    assert_code_contains(
        code,
        "pub struct ConcludeWorkingInput<Daemon: ComponentDaemon>",
    );
    assert_code_contains(code, "advance_gate: std::sync::Arc<tokio::sync::Mutex<()>>");
    assert_code_contains(
        code,
        "let advance_gate = Daemon::shared_advance_gate(&engine).unwrap_or_default();",
    );
    assert_code_contains(code, "let _advance_turn = self.advance_gate.lock().await;");
    assert_code_contains(code, "advance.resolve().await;");
    assert_code_contains(code, "self.engine.ask(ConcludeWorkingInput { advance })");
    assert_code_contains(code, "message.advance.conclude(&mut self.engine).await");
}

#[test]
fn component_decoded_tier_emits_no_staged_lane() {
    let generated = DaemonModule::new(component_decoded_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_excludes(code, "WorkingInputLane");
    assert_code_excludes(code, "StagedAdvance");
    assert_code_excludes(code, "advance_gate");
}

#[test]
fn schema_without_a_stream_emits_no_subscription_plumbing() {
    let generated = DaemonModule::new(single_listener_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_excludes(code, "EmittedSubscriptions");
    assert_code_excludes(code, "subscription_filter");
    assert_code_excludes(code, "SubscriptionRegistry");
    // The hook trait and spine are still emitted.
    assert_code_contains(code, "pub trait ComponentDaemon");
    assert_code_contains(code, "fn handle_working_input");
}

#[test]
fn upgrade_listener_tier_emits_the_third_listener_alongside_meta() {
    let generated = DaemonModule::new(upgrade_tier_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    // The listener-tier enum gains the third `Upgrade` identity alongside Meta.
    assert_code_contains(code, "pub enum ListenerTier");
    assert_code_contains(code, "Working");
    assert_code_contains(code, "Meta");
    assert_code_contains(code, "Upgrade");
    assert_code_contains(code, "Self::Upgrade => formatter.write_str(\"upgrade\")");

    // The component trait gains the component-decoded upgrade hook, defaulting to
    // a no-op exactly like the meta hook, taking the actor's `&mut Self::Engine`.
    assert_code_contains(
        code,
        "fn handle_upgrade_connection(engine: &mut Self::Engine, connection: AcceptedConnection)",
    );

    // The binder binds a third `AsyncListenerSocket` from the upgrade socket path,
    // owner-only at the declared mode.
    assert_code_contains(
        code,
        "let mut listener_sockets = std::vec![working_socket];",
    );
    assert_code_contains(
        code,
        "let upgrade_socket_path = configuration.upgrade_socket_path().ok_or(DaemonError::MissingUpgradeSocket)?.to_path_buf();",
    );
    assert_code_contains(
        code,
        "listener_sockets.push(AsyncListenerSocket::new(ListenerTier::Upgrade, upgrade_socket_path).with_socket_mode(SocketMode::new(0o600)))",
    );

    // The EngineActor gains an `UpgradeConnection` message routing to the hook,
    // mirroring the meta tier.
    assert_code_contains(code, "pub struct UpgradeConnection");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> Message<UpgradeConnection> for EngineActor<Daemon>",
    );
    assert_code_contains(
        code,
        "Daemon::handle_upgrade_connection(&mut self.engine, message.connection).await",
    );
    assert_code_contains(
        code,
        "self.engine.ask(UpgradeConnection { connection }).await",
    );

    // The multi-listener runtime routes all three tiers.
    assert_code_contains(
        code,
        "ListenerTier::Working => self.handle_working_connection(connection).await",
    );
    assert_code_contains(
        code,
        "ListenerTier::Meta => self.handle_meta_connection(connection).await",
    );
    assert_code_contains(
        code,
        "ListenerTier::Upgrade => self.handle_upgrade_connection(connection).await",
    );

    // The daemon error gains the missing-upgrade-socket variant.
    assert_code_contains(code, "MissingUpgradeSocket");
    assert_code_contains(code, "MissingMetaSocket");
    assert_code_contains(code, "From<AsyncMultiListenerDaemonError<Daemon::Error>>");
    assert_code_excludes(code, "UnixStream");
}

#[test]
fn upgrade_tier_without_meta_emits_a_two_listener_multi_daemon() {
    let generated = DaemonModule::new(upgrade_only_shape(), "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    // The enum carries Working + Upgrade only; the Meta tier is absent.
    assert_code_contains(code, "pub enum ListenerTier");
    assert_code_contains(code, "Upgrade");
    assert_code_contains(code, "fn handle_upgrade_connection(");
    assert_code_contains(code, "pub struct UpgradeConnection");
    assert_code_contains(code, "MissingUpgradeSocket");
    assert_code_contains(
        code,
        "ListenerTier::Upgrade => self.handle_upgrade_connection(connection).await",
    );
    // The upgrade-only daemon is still multi-listener (`AsyncMultiListenerDaemon`)
    // but emits NO meta tier: no Meta variant, no MetaConnection, no MissingMeta.
    assert_code_contains(code, "AsyncMultiListenerDaemon::new(");
    assert_code_excludes(code, "ListenerTier::Meta");
    assert_code_excludes(code, "MetaConnection");
    assert_code_excludes(code, "MissingMetaSocket");
    assert_code_excludes(code, "handle_meta_connection");
    assert_code_excludes(code, "AsyncSingleListenerDaemon::new(");
}
