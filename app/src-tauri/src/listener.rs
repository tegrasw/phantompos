//! Raw TCP listener for the virtual printer.
//!
//! Accepts any number of simultaneous connections. Each connection owns a
//! `phantompos-core` [`Parser`]: every chunk read from the socket is fed to it
//! and the decoded commands go to an [`EventSink`]. Interpreting ESC/POS is
//! the job of `phantompos-core`; this module never looks inside the data.
//!
//! The listener is independent of Tauri so it can be tested with a plain
//! channel as the sink. The Tauri glue lives in `lib.rs`.

use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use phantompos_core::{DecodedCommand, Parser, ParserConfig};
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

/// Port used by network thermal printers ("raw" / JetDirect printing).
pub const DEFAULT_PORT: u16 = 9100;

/// Size of the buffer for each socket read; one read produces at most one
/// `commands:received` event.
const READ_BUFFER_SIZE: usize = 4096;

/// Pause after a failed `accept` so a persistent error (e.g. too many open
/// files) does not turn the accept loop into a busy loop.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

/// Where the listener binds. Kept in managed state so it can become
/// configurable later; for now it is always the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListenerSettings {
    pub address: IpAddr,
    pub port: u16,
}

impl Default for ListenerSettings {
    fn default() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: DEFAULT_PORT,
        }
    }
}

/// Payload of the `listener:status` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ListenerStatus {
    /// Not started yet.
    Stopped { address: String, port: u16 },
    /// Bound and accepting connections.
    Listening { address: String, port: u16 },
    /// Could not bind; `message` is meant to be shown to the user.
    Error {
        address: String,
        port: u16,
        message: String,
    },
}

/// Payload of the `connection:changed` event.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConnectionsSnapshot {
    /// Number of open connections.
    pub active: usize,
    /// Remote address of each open connection, ordered by connection id.
    pub peers: Vec<String>,
}

/// Payload of the `commands:received` event: the commands completed by one
/// socket read, or the tail flushed when the connection ends. Never empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandsReceived {
    /// Unique, increasing id of the connection the commands came from.
    pub connection_id: u64,
    pub peer: String,
    /// Serialized by `phantompos-core`; offsets are relative to this
    /// connection's stream.
    pub commands: Vec<DecodedCommand>,
}

/// Return value of the `get_state` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    pub listener: ListenerStatus,
    pub connections: ConnectionsSnapshot,
}

/// Something that happened in the listener and must reach the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerEvent {
    Status(ListenerStatus),
    Connections(ConnectionsSnapshot),
    Commands(CommandsReceived),
}

/// Receives listener events. Implementations must not block for long: status
/// and connection events are emitted while the state lock is held, so that
/// the order of events always matches the order of state changes.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: ListenerEvent);
}

#[derive(Debug, thiserror::Error)]
pub enum ListenerError {
    #[error("Port {port} on {address} is already in use by another program.")]
    AddrInUse { address: IpAddr, port: u16 },
    #[error("Could not listen on {address}:{port}: {source}")]
    Bind {
        address: IpAddr,
        port: u16,
        #[source]
        source: io::Error,
    },
}

/// Shared listener state, read by the `get_state` command and updated by the
/// listener tasks.
#[derive(Debug)]
pub struct ListenerState {
    settings: ListenerSettings,
    inner: Mutex<Inner>,
    next_connection_id: AtomicU64,
}

#[derive(Debug)]
struct Inner {
    status: ListenerStatus,
    peers: BTreeMap<u64, SocketAddr>,
}

impl Inner {
    fn connections(&self) -> ConnectionsSnapshot {
        ConnectionsSnapshot {
            active: self.peers.len(),
            peers: self.peers.values().map(ToString::to_string).collect(),
        }
    }
}

impl ListenerState {
    pub fn new(settings: ListenerSettings) -> Self {
        Self {
            settings,
            inner: Mutex::new(Inner {
                status: ListenerStatus::Stopped {
                    address: settings.address.to_string(),
                    port: settings.port,
                },
                peers: BTreeMap::new(),
            }),
            next_connection_id: AtomicU64::new(1),
        }
    }

    pub fn settings(&self) -> ListenerSettings {
        self.settings
    }

    pub fn snapshot(&self) -> StateSnapshot {
        let inner = self.lock();
        StateSnapshot {
            listener: inner.status.clone(),
            connections: inner.connections(),
        }
    }

    /// The state stays consistent even if a holder panicked, so a poisoned
    /// lock is recovered instead of propagating the panic.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_status(&self, status: ListenerStatus, sink: &dyn EventSink) {
        let mut inner = self.lock();
        inner.status = status.clone();
        sink.emit(ListenerEvent::Status(status));
    }

    fn add_connection(&self, peer: SocketAddr, sink: &dyn EventSink) -> u64 {
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        let mut inner = self.lock();
        inner.peers.insert(id, peer);
        sink.emit(ListenerEvent::Connections(inner.connections()));
        id
    }

    fn remove_connection(&self, id: u64, sink: &dyn EventSink) {
        let mut inner = self.lock();
        if inner.peers.remove(&id).is_some() {
            sink.emit(ListenerEvent::Connections(inner.connections()));
        }
    }
}

/// Binds the TCP listener described by `settings`.
pub async fn bind(settings: ListenerSettings) -> Result<TcpListener, ListenerError> {
    let ListenerSettings { address, port } = settings;
    TcpListener::bind(SocketAddr::new(address, port))
        .await
        .map_err(|source| match source.kind() {
            io::ErrorKind::AddrInUse => ListenerError::AddrInUse { address, port },
            _ => ListenerError::Bind {
                address,
                port,
                source,
            },
        })
}

/// Binds and serves until the task is dropped or aborted.
///
/// A bind failure is recorded as an error status and emitted; the function
/// then returns without affecting the rest of the app. Aborting the task
/// closes every open connection (they live in a [`JoinSet`]).
pub async fn run(state: Arc<ListenerState>, sink: Arc<dyn EventSink>) {
    let settings = state.settings();
    let address = settings.address.to_string();

    let listener = match bind(settings).await {
        Ok(listener) => listener,
        Err(error) => {
            state.set_status(
                ListenerStatus::Error {
                    address,
                    port: settings.port,
                    message: error.to_string(),
                },
                sink.as_ref(),
            );
            return;
        }
    };

    // Report the port actually bound (differs from the setting only for port 0).
    let port = listener
        .local_addr()
        .map_or(settings.port, |local| local.port());
    state.set_status(ListenerStatus::Listening { address, port }, sink.as_ref());

    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    connections.spawn(serve_connection(stream, peer, state.clone(), sink.clone()));
                }
                Err(error) => {
                    eprintln!("phantompos: failed to accept a connection: {error}");
                    tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                }
            },
            // Reap finished connection tasks so the set does not keep growing.
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

/// Per-connection state: the parser and what is needed to report its output.
///
/// Dropping it ends the connection, whether the task returned (EOF, read
/// error) or was aborted (listener shutdown): the parser is finished and its
/// remaining commands are emitted first, then the connection is removed from
/// the shared state. So the last `commands:received` of a connection always
/// precedes the `connection:changed` that reports it gone.
struct Connection {
    id: u64,
    peer: String,
    parser: Parser,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
}

impl Connection {
    fn emit_commands(&self, commands: Vec<DecodedCommand>) {
        if commands.is_empty() {
            return;
        }
        self.sink.emit(ListenerEvent::Commands(CommandsReceived {
            connection_id: self.id,
            peer: self.peer.clone(),
            commands,
        }));
    }

    fn feed(&mut self, chunk: &[u8]) {
        let commands = self.parser.feed(chunk);
        self.emit_commands(commands);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let tail = self.parser.finish();
        self.emit_commands(tail);
        self.state.remove_connection(self.id, self.sink.as_ref());
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
) {
    let id = state.add_connection(peer, sink.as_ref());
    let mut connection = Connection {
        id,
        peer: peer.to_string(),
        parser: Parser::new(ParserConfig::default()),
        state,
        sink,
    };
    let mut buffer = vec![0_u8; READ_BUFFER_SIZE];

    loop {
        match stream.read(&mut buffer).await {
            // Orderly shutdown by the client.
            Ok(0) => break,
            Ok(read) => {
                let Some(chunk) = buffer.get(..read) else {
                    break;
                };
                connection.feed(chunk);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                eprintln!(
                    "phantompos: connection {id} from {} failed: {error}",
                    connection.peer
                );
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantompos_core::{Command, UnknownReason, parse};
    use tokio::io::AsyncWriteExt;
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    use tokio::task::JoinHandle;
    use tokio::time::timeout;

    const TIMEOUT: Duration = Duration::from_secs(5);

    impl EventSink for UnboundedSender<ListenerEvent> {
        fn emit(&self, event: ListenerEvent) {
            // The test may have stopped listening; that is fine.
            let _ = self.send(event);
        }
    }

    fn localhost(port: u16) -> ListenerSettings {
        ListenerSettings {
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }

    async fn next_event(events: &mut UnboundedReceiver<ListenerEvent>) -> ListenerEvent {
        timeout(TIMEOUT, events.recv())
            .await
            .expect("timed out waiting for a listener event")
            .expect("event channel closed")
    }

    async fn next_connections(
        events: &mut UnboundedReceiver<ListenerEvent>,
    ) -> ConnectionsSnapshot {
        match next_event(events).await {
            ListenerEvent::Connections(snapshot) => snapshot,
            other => panic!("expected a connections event, got {other:?}"),
        }
    }

    async fn next_commands(events: &mut UnboundedReceiver<ListenerEvent>) -> CommandsReceived {
        match next_event(events).await {
            ListenerEvent::Commands(received) => received,
            other => panic!("expected a commands event, got {other:?}"),
        }
    }

    fn parse_default(bytes: &[u8]) -> Vec<DecodedCommand> {
        parse(bytes, ParserConfig::default())
    }

    struct Running {
        state: Arc<ListenerState>,
        events: UnboundedReceiver<ListenerEvent>,
        task: JoinHandle<()>,
        port: u16,
    }

    /// Starts a listener on an ephemeral port and waits until it is bound.
    async fn start() -> Running {
        let state = Arc::new(ListenerState::new(localhost(0)));
        let (sender, mut events) = unbounded_channel();
        let task = tokio::spawn(run(state.clone(), Arc::new(sender)));
        let port = match next_event(&mut events).await {
            ListenerEvent::Status(ListenerStatus::Listening { address, port }) => {
                assert_eq!(address, "127.0.0.1");
                port
            }
            other => panic!("expected a listening status, got {other:?}"),
        };
        assert_ne!(port, 0);
        Running {
            state,
            events,
            task,
            port,
        }
    }

    #[test]
    fn default_settings_are_local_port_9100() {
        let settings = ListenerSettings::default();
        assert_eq!(settings.address.to_string(), "127.0.0.1");
        assert_eq!(settings.port, 9100);
    }

    fn json<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).expect("payload serializes")
    }

    #[test]
    fn payloads_match_the_frontend_contract() {
        let status = ListenerStatus::Error {
            address: "127.0.0.1".into(),
            port: 9100,
            message: "boom".into(),
        };
        assert_eq!(
            json(&status),
            r#"{"state":"error","address":"127.0.0.1","port":9100,"message":"boom"}"#
        );
        let listening = ListenerStatus::Listening {
            address: "127.0.0.1".into(),
            port: 9100,
        };
        assert_eq!(
            json(&listening),
            r#"{"state":"listening","address":"127.0.0.1","port":9100}"#
        );
        let received = CommandsReceived {
            connection_id: 7,
            peer: "127.0.0.1:54321".into(),
            commands: parse_default(b"\x1bE\x01Hi\n\x1ba\x09"),
        };
        assert_eq!(
            json(&received),
            concat!(
                r#"{"connectionId":7,"peer":"127.0.0.1:54321","commands":["#,
                r#"{"offset":0,"bytes":[27,69,1],"command":{"type":"setBold","enabled":true}},"#,
                r#"{"offset":3,"bytes":[72,105],"command":{"type":"text","text":"Hi"}},"#,
                r#"{"offset":5,"bytes":[10],"command":{"type":"lineFeed"}},"#,
                r#"{"offset":6,"bytes":[27,97,9],"command":{"type":"setAlignment","alignment":null}}"#,
                r#"]}"#
            )
        );
        let state = ListenerState::new(ListenerSettings::default());
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100},"connections":{"active":0,"peers":[]}}"#
        );
    }

    #[tokio::test]
    async fn split_stream_yields_the_same_commands_as_one_shot_parse() {
        let mut running = start().await;
        let mut client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        let client_addr = client.local_addr().expect("client address").to_string();

        let opened = next_connections(&mut running.events).await;
        assert_eq!(opened.active, 1);
        assert_eq!(opened.peers, vec![client_addr.clone()]);
        assert_eq!(running.state.snapshot().connections, opened);

        // Writes that split commands and text runs mid-way. Each one completes
        // at least one command, so waiting for an event between writes forces
        // them into separate reads.
        let parts: [&[u8]; 4] = [
            b"\x1b@Caf\x82 ",
            b"total\n\x1b",
            b"E\x01bold\x1b",
            b"E\x00\x1da\x1d!\x11big\n",
        ];
        let mut stream = Vec::new();
        let mut batches = Vec::new();
        for part in parts {
            client.write_all(part).await.expect("write");
            stream.extend_from_slice(part);
            batches.push(next_commands(&mut running.events).await);
        }
        // Larger than one read buffer, so it must arrive in several reads.
        let bulk: Vec<u8> = (0..3 * READ_BUFFER_SIZE).map(|i| (i % 251) as u8).collect();
        client.write_all(&bulk).await.expect("write bulk");
        stream.extend_from_slice(&bulk);
        // Ends inside a command: only `finish` can report it.
        client.write_all(b"tail\x1bd").await.expect("write tail");
        stream.extend_from_slice(b"tail\x1bd");
        drop(client);

        let closed = loop {
            match next_event(&mut running.events).await {
                ListenerEvent::Commands(batch) => batches.push(batch),
                ListenerEvent::Connections(snapshot) => break snapshot,
                other => panic!("unexpected event {other:?}"),
            }
        };
        assert_eq!(closed, ConnectionsSnapshot::default());
        assert_eq!(running.state.snapshot().connections, closed);
        assert!(
            batches.len() > parts.len() + 1,
            "got {} batches",
            batches.len()
        );

        let id = batches.first().expect("batches").connection_id;
        let mut next_offset = 0_u64;
        let mut commands = Vec::new();
        for batch in batches {
            assert_eq!(batch.connection_id, id);
            assert_eq!(batch.peer, client_addr);
            assert!(!batch.commands.is_empty());
            // Offsets continue across batches with no gap or overlap.
            for command in &batch.commands {
                assert_eq!(command.offset, next_offset);
                next_offset += command.bytes.len() as u64;
            }
            commands.extend(batch.commands);
        }
        assert_eq!(next_offset, stream.len() as u64);
        assert_eq!(commands, parse_default(&stream));
        assert_eq!(
            commands.last().map(|last| &last.command),
            Some(&Command::Unknown {
                reason: UnknownReason::Incomplete
            })
        );
        running.task.abort();
    }

    #[tokio::test]
    async fn trailing_text_is_emitted_before_the_disconnect() {
        let mut running = start().await;
        let mut client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        assert_eq!(next_connections(&mut running.events).await.active, 1);

        client.write_all(b"Hello\nTail").await.expect("write");
        let first = next_commands(&mut running.events).await;
        assert_eq!(first.commands, parse_default(b"Hello\n"));

        drop(client);
        let tail = next_commands(&mut running.events).await;
        assert_eq!(tail.connection_id, first.connection_id);
        assert_eq!(
            tail.commands,
            vec![DecodedCommand {
                offset: 6,
                bytes: b"Tail".to_vec(),
                command: Command::Text {
                    text: "Tail".into()
                },
            }]
        );
        assert_eq!(
            next_connections(&mut running.events).await,
            ConnectionsSnapshot::default()
        );
        running.task.abort();
    }

    #[tokio::test]
    async fn simultaneous_connections_have_independent_parsers() {
        let mut running = start().await;
        let mut first = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect first");
        assert_eq!(next_connections(&mut running.events).await.active, 1);
        let mut second = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect second");
        let both = next_connections(&mut running.events).await;
        assert_eq!(both.active, 2);
        let second_addr = second.local_addr().expect("second address").to_string();
        assert_eq!(
            both.peers,
            vec![
                first.local_addr().expect("first address").to_string(),
                second_addr.clone(),
            ]
        );

        // The first connection leaves an unfinished command behind; it must
        // not leak into the second one.
        first.write_all(b"one\n\x1b").await.expect("write first");
        let from_first = next_commands(&mut running.events).await;
        second.write_all(b"E\x01two\n").await.expect("write second");
        let from_second = next_commands(&mut running.events).await;
        assert_eq!(from_first.commands, parse_default(b"one\n"));
        assert_eq!(from_second.commands, parse_default(b"E\x01two\n"));
        assert_eq!(from_second.peer, second_addr);
        assert!(from_second.connection_id > from_first.connection_id);

        first.write_all(b"E\x01").await.expect("write first again");
        let continued = next_commands(&mut running.events).await;
        assert_eq!(continued.connection_id, from_first.connection_id);
        assert_eq!(
            continued.commands,
            vec![DecodedCommand {
                offset: 4,
                bytes: vec![0x1b, b'E', 0x01],
                command: Command::SetBold { enabled: true },
            }]
        );

        // Nothing pending: the disconnect produces no commands event.
        drop(first);
        let remaining = next_connections(&mut running.events).await;
        assert_eq!(remaining.active, 1);
        assert_eq!(remaining.peers, vec![second_addr]);
        running.task.abort();
    }

    #[tokio::test]
    async fn aborting_the_listener_flushes_and_closes_connections() {
        let mut running = start().await;
        let mut client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        assert_eq!(next_connections(&mut running.events).await.active, 1);

        client.write_all(b"A\n\x1bE").await.expect("write");
        let first = next_commands(&mut running.events).await;
        assert_eq!(first.commands, parse_default(b"A\n"));

        running.task.abort();
        assert!(running.task.await.is_err_and(|error| error.is_cancelled()));
        // Connection tasks are dropped asynchronously after the abort.
        let tail = next_commands(&mut running.events).await;
        assert_eq!(
            tail.commands,
            vec![DecodedCommand {
                offset: 2,
                bytes: vec![0x1b, b'E'],
                command: Command::Unknown {
                    reason: UnknownReason::Incomplete
                },
            }]
        );
        assert_eq!(
            next_connections(&mut running.events).await,
            ConnectionsSnapshot::default()
        );
        assert_eq!(running.state.snapshot().connections.active, 0);
    }

    #[tokio::test]
    async fn bind_conflict_reports_an_error_without_panicking() {
        let occupied = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = occupied.local_addr().expect("address").port();

        let error = bind(localhost(port)).await.expect_err("port is taken");
        assert!(matches!(error, ListenerError::AddrInUse { port: p, .. } if p == port));

        let state = Arc::new(ListenerState::new(localhost(port)));
        let (sender, mut events) = unbounded_channel();
        timeout(TIMEOUT, run(state.clone(), Arc::new(sender)))
            .await
            .expect("run returns after a bind failure");

        let expected = ListenerStatus::Error {
            address: "127.0.0.1".into(),
            port,
            message: error.to_string(),
        };
        assert_eq!(
            next_event(&mut events).await,
            ListenerEvent::Status(expected.clone())
        );
        assert_eq!(state.snapshot().listener, expected);
    }
}
