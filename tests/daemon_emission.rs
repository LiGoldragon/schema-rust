use schema_rust::{
    DaemonModule, MetaListenerTier, NexusDaemonShape, SocketModeBits, TcpListenerTier,
    UpgradeListenerTier, WorkingListenerTier,
};

mod support;

use support::FixtureSchema;

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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(single_listener_shape(), &schema, "schema-rust").to_generated_file();

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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(single_listener_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(single_listener_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(multi_listener_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated = DaemonModule::new(tcp_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(meta_plus_tcp_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(component_decoded_shape(), &schema, "schema-rust").to_generated_file();
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
fn declared_stream_emits_async_subscription_support() {
    let schema = FixtureSchema::new("daemon-stream.schema").lower("test:signal");
    let generated =
        DaemonModule::new(single_listener_shape(), &schema, "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "type SubscriptionToken:");
    assert_code_contains(code, "type SubscriptionFilter:");
    assert_code_contains(code, "type StreamEvent:");
    assert_code_contains(code, "fn subscription_filter(input: &Input)");
    assert_code_contains(code, "fn subscription_token(output: &Output)");
    assert_code_contains(
        code,
        "fn published_event<'event>(engine: &'event Self::Engine, output: &'event Output)",
    );
    assert_code_contains(code, "fn event_matches_filter(");
    assert_code_contains(code, "fn subscription_event_short_header() -> u64");
    assert_code_contains(
        code,
        "pub struct EmittedSubscriptions<Daemon: ComponentDaemon>",
    );
    assert_code_contains(code, "tokio::sync::Mutex");
    assert_code_contains(code, "SubscriptionEventPublisher::acceptor(");
    assert_code_contains(code, "let (stream, context) = connection.into_parts();");
    assert_code_contains(code, "let (reader, writer) = tokio::io::split(stream);");
    assert_code_contains(
        code,
        "type SubscriptionWriter = Box<dyn tokio::io::AsyncWrite + Unpin + Send>;",
    );
    assert_code_contains(code, "transport.into_writer()");
    assert_code_contains(code, "frame.encode()?");
    assert_code_contains(code, "write_body_async(writer, &FrameBody::new(bytes))");
    assert_code_excludes(code, "compile_error!");
    assert_code_excludes(code, "std::sync::Mutex");
    assert_code_excludes(code, "try_clone_stream");
    assert_code_excludes(code, "OwnedWriteHalf");
    assert_code_excludes(code, "UnixStream");

    // The stream tier now also routes the engine through the kameo `EngineActor`.
    // The engine hook becomes `&mut Self::Engine` (the actor handler's exclusive
    // borrow), and the `WorkingInput` handler computes both the output and the
    // published event together, returning them as a `WorkingOutcome`.
    assert_code_contains(
        code,
        "fn handle_working_input<'connection>(engine: &'connection mut Self::Engine, input: Input, connection: &'connection triad_runtime::ConnectionContext)",
    );
    assert_code_contains(code, "pub struct EngineActor<Daemon: ComponentDaemon>");
    assert_code_contains(code, "engine: ActorRef<EngineActor<Daemon>>");
    assert_code_contains(code, "EngineActor::<Daemon>::spawn(EngineActor { engine })");
    assert_code_contains(
        code,
        "subscriptions: std::sync::Arc<EmittedSubscriptions<Daemon>>",
    );
    assert_code_contains(code, "pub struct WorkingOutcome<Daemon: ComponentDaemon>");
    assert_code_contains(code, "output: Output");
    assert_code_contains(code, "event: Option<Daemon::StreamEvent>");
    assert_code_contains(
        code,
        "type Reply = Result<WorkingOutcome<Daemon>, Daemon::Error>;",
    );
    // The actor handler computes the output under `&mut self.engine`, then the
    // published event under `&self.engine`, returning both together.
    assert_code_contains(
        code,
        "let output = Daemon::handle_working_input(&mut self.engine, message.input, &message.context).await?;",
    );
    assert_code_contains(
        code,
        "let event = Daemon::published_event(&self.engine, &output).await?;",
    );
    assert_code_contains(code, "Ok(WorkingOutcome { output, event })");
    // The runtime computes the subscription filter BEFORE the ask, then registers
    // the writer and publishes the returned event after the ask resolves.
    assert_code_contains(code, "let filter = Daemon::subscription_filter(&input);");
    assert_code_contains(
        code,
        "let outcome = match self.engine.ask(WorkingInput { input, context }).await",
    );
    assert_code_contains(
        code,
        "transport.write_frame(outcome.output.encode_signal_frame()?).await?;",
    );
    assert_code_contains(
        code,
        "if let (Some(filter), Some(token)) = (filter, Daemon::subscription_token(&outcome.output))",
    );
    assert_code_contains(code, "if let Some(event) = outcome.event {");
    assert_code_contains(code, "self.subscriptions.publish(event).await?");
    // The stream tier shares the actor lifecycle: graceful stop crosses the
    // mailbox, not a direct `Daemon::stop` on a shared engine.
    assert_code_contains(code, "self.engine.stop_gracefully().await");
    assert_code_contains(code, "self.engine.wait_for_shutdown().await");
}

#[test]
fn tcp_listener_tier_composes_with_stream_subscriptions() {
    let schema = FixtureSchema::new("daemon-stream.schema").lower("test:signal");
    let generated = DaemonModule::new(tcp_shape(), &schema, "schema-rust").to_generated_file();
    let code = generated.code.as_str();

    assert_code_contains(code, "GeneratedSingleAndTcpDaemon<Self>");
    assert_code_contains(
        code,
        "impl<Daemon: ComponentDaemon> AsyncConnectionRuntime<TcpStream> for GeneratedDaemonRuntime<Daemon>",
    );
    assert_code_contains(
        code,
        "async fn handle_working_connection<Stream>(&self, connection: AcceptedConnection<Stream>)",
    );
    assert_code_contains(
        code,
        "Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static",
    );
    assert_code_contains(code, "WorkingTransport::from_connection(connection)");
    assert_code_contains(
        code,
        "type SubscriptionWriter = Box<dyn tokio::io::AsyncWrite + Unpin + Send>;",
    );
    assert_code_contains(
        code,
        "writers: std::collections::HashMap<SubscriptionTokenInner, SubscriptionWriter>",
    );
    assert_code_contains(code, "self.handle_working_connection(connection).await");
    assert_code_excludes(code, "OwnedWriteHalf");
    assert_code_excludes(code, "UnixStream");
}

#[test]
fn schema_without_a_stream_emits_no_subscription_plumbing() {
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(single_listener_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(upgrade_tier_shape(), &schema, "schema-rust").to_generated_file();
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
    let schema = FixtureSchema::new("spirit-min.schema").lower("spirit:lib");
    let generated =
        DaemonModule::new(upgrade_only_shape(), &schema, "schema-rust").to_generated_file();
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
