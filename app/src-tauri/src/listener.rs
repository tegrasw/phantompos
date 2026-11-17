//! Raw TCP listener for the virtual printer.
//!
//! Accepts any number of simultaneous connections and forwards every chunk of
//! bytes read from a socket, untouched, to an [`EventSink`]. Interpreting
//! ESC/POS is the job of `phantompos-core`; this module never looks inside the
//! data.
//!
//! The listener is independent of Tauri so it can be tested with a plain
//! channel as the sink. The Tauri glue lives in `lib.rs`.

use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

/// Port used by network thermal printers ("raw" / JetDirect printing).
pub const DEFAULT_PORT: u16 = 9100;

/// Size of the buffer for each socket read; one read produces one event.
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

/// Payload of the `bytes:received` event: one socket read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BytesReceived {
    /// Unique, increasing id of the connection the bytes came from.
    pub connection_id: u64,
    pub peer: String,
    pub bytes: Vec<u8>,
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
    Bytes(BytesReceived),
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

/// Removes the connection from the shared state when the connection task
/// ends, including when it is aborted.
struct ConnectionGuard {
    id: u64,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
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
    let _guard = ConnectionGuard {
        id,
        state,
        sink: sink.clone(),
    };
    let peer_label = peer.to_string();
    let mut buffer = vec![0_u8; READ_BUFFER_SIZE];

    loop {
        match stream.read(&mut buffer).await {
            // Orderly shutdown by the client.
            Ok(0) => break,
            Ok(read) => {
                let Some(chunk) = buffer.get(..read) else {
                    break;
                };
                sink.emit(ListenerEvent::Bytes(BytesReceived {
                    connection_id: id,
                    peer: peer_label.clone(),
                    bytes: chunk.to_vec(),
                }));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                eprintln!("phantompos: connection {id} from {peer_label} failed: {error}");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let bytes = BytesReceived {
            connection_id: 7,
            peer: "127.0.0.1:54321".into(),
            bytes: vec![0x1b, 0x40],
        };
        assert_eq!(
            json(&bytes),
            r#"{"connectionId":7,"peer":"127.0.0.1:54321","bytes":[27,64]}"#
        );
        let state = ListenerState::new(ListenerSettings::default());
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100},"connections":{"active":0,"peers":[]}}"#
        );
    }

    #[tokio::test]
    async fn forwards_bytes_and_tracks_the_connection() {
        let mut running = start().await;
        let mut client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        let client_addr = client.local_addr().expect("client address").to_string();

        let opened = next_connections(&mut running.events).await;
        assert_eq!(opened.active, 1);
        assert_eq!(opened.peers, vec![client_addr.clone()]);
        assert_eq!(running.state.snapshot().connections, opened);

        // Larger than one read buffer, so it must arrive in several chunks.
        let payload: Vec<u8> = (0..3 * READ_BUFFER_SIZE).map(|i| (i % 251) as u8).collect();
        client.write_all(&payload).await.expect("write");

        let mut received = Vec::new();
        let mut chunks = 0;
        let mut connection_id = None;
        while received.len() < payload.len() {
            match next_event(&mut running.events).await {
                ListenerEvent::Bytes(event) => {
                    assert_eq!(event.peer, client_addr);
                    assert!(!event.bytes.is_empty() && event.bytes.len() <= READ_BUFFER_SIZE);
                    assert_eq!(
                        *connection_id.get_or_insert(event.connection_id),
                        event.connection_id
                    );
                    received.extend_from_slice(&event.bytes);
                    chunks += 1;
                }
                other => panic!("expected bytes, got {other:?}"),
            }
        }
        assert_eq!(received, payload);
        assert!(chunks >= 3, "expected several chunks, got {chunks}");

        drop(client);
        let closed = next_connections(&mut running.events).await;
        assert_eq!(closed, ConnectionsSnapshot::default());
        assert_eq!(running.state.snapshot().connections, closed);
        running.task.abort();
    }

    #[tokio::test]
    async fn accepts_simultaneous_connections() {
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
        assert_eq!(
            both.peers,
            vec![
                first.local_addr().expect("first address").to_string(),
                second.local_addr().expect("second address").to_string(),
            ]
        );

        first.write_all(b"one").await.expect("write first");
        let ListenerEvent::Bytes(from_first) = next_event(&mut running.events).await else {
            panic!("expected bytes from the first connection");
        };
        second.write_all(b"two").await.expect("write second");
        let ListenerEvent::Bytes(from_second) = next_event(&mut running.events).await else {
            panic!("expected bytes from the second connection");
        };
        assert_eq!(from_first.bytes, b"one");
        assert_eq!(from_second.bytes, b"two");
        assert!(from_second.connection_id > from_first.connection_id);

        drop(first);
        let remaining = next_connections(&mut running.events).await;
        assert_eq!(remaining.active, 1);
        assert_eq!(
            remaining.peers,
            vec![second.local_addr().expect("second address").to_string()]
        );
        running.task.abort();
    }

    #[tokio::test]
    async fn aborting_the_listener_closes_connections() {
        let mut running = start().await;
        let _client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        assert_eq!(next_connections(&mut running.events).await.active, 1);

        running.task.abort();
        assert!(running.task.await.is_err_and(|error| error.is_cancelled()));
        // Connection tasks are dropped asynchronously after the abort.
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
