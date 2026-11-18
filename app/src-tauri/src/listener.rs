//! Raw TCP listener for the virtual printer.
//!
//! Accepts any number of simultaneous connections. Each connection owns a
//! `phantompos-core` [`Parser`] and [`ReceiptBuilder`]: every chunk read from
//! the socket is fed to them, and the decoded commands, receipts and drawer
//! kicks go to an [`EventSink`]. Interpreting ESC/POS is the job of
//! `phantompos-core`; this module only routes the builder's [`Effect`]s.
//!
//! Event order per read: `commands:received`, then `receipt:completed` /
//! `drawer:kicked` in stream order, then `receipt:updated` if the receipt in
//! progress changed (at most one per read). On disconnect: the flushed
//! commands, the pending receipt (if any) and `connection:changed`.
//!
//! `receipt:updated` carries the whole receipt in progress, images included
//! (base64 bitmaps), so it is throttled per connection: the first change is
//! sent at once, later ones at most every [`UPDATE_INTERVAL`] (the latest
//! snapshot is sent when the interval ends). A snapshot still pending when
//! its receipt is completed (cut or disconnect) is dropped: the completed
//! receipt supersedes it.
//!
//! The listener is independent of Tauri so it can be tested with a plain
//! channel as the sink. The Tauri glue lives in `lib.rs`.

use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use phantompos_core::{
    DecodedCommand, DrawerKick, Effect, PaperProfile, Parser, ParserConfig, Receipt,
    ReceiptBuilder, ReceiptConfig,
};
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio::time::Instant;

/// Port used by network thermal printers ("raw" / JetDirect printing).
pub const DEFAULT_PORT: u16 = 9100;

/// Size of the buffer for each socket read; one read produces at most one
/// `commands:received` event.
const READ_BUFFER_SIZE: usize = 4096;

/// Minimum time between two `receipt:updated` events of one connection.
const UPDATE_INTERVAL: Duration = Duration::from_millis(50);

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

/// Payload of the `receipt:updated` event: the receipt in progress on a
/// connection (`cut` is always `null`), sent when it changed and is not empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptUpdated {
    pub connection_id: u64,
    pub peer: String,
    pub receipt: Receipt,
}

/// Payload of the `receipt:completed` event: a receipt closed by a cut, or by
/// the end of the connection (`cut: null`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptCompleted {
    /// Unique and increasing across the app run, in closing order.
    pub id: u64,
    pub connection_id: u64,
    pub peer: String,
    pub receipt: Receipt,
}

/// Payload of the `drawer:kicked` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawerKicked {
    pub connection_id: u64,
    pub peer: String,
    /// Drawer kick-out connector pin: 2 or 5.
    pub pin: u8,
    pub on_ms: u32,
    pub off_ms: u32,
}

/// Return value of the `get_state` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    pub listener: ListenerStatus,
    pub connections: ConnectionsSnapshot,
    /// Paper for receipts started from now on.
    pub paper: PaperProfile,
    /// Set by a drawer kick, cleared by the `close_drawer` command.
    pub drawer_open: bool,
}

/// Something that happened in the listener and must reach the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerEvent {
    Status(ListenerStatus),
    Connections(ConnectionsSnapshot),
    Commands(CommandsReceived),
    ReceiptUpdated(ReceiptUpdated),
    ReceiptCompleted(ReceiptCompleted),
    DrawerKicked(DrawerKicked),
}

/// Receives listener events. Implementations must not block for long: status,
/// connection, receipt-completed and drawer events are emitted while the state
/// lock is held, so that the order of events always matches the order of
/// state changes (and receipt ids reach the UI in increasing order).
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
    paper: PaperProfile,
    drawer_open: bool,
    /// Id of the next completed receipt.
    next_receipt_id: u64,
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
                paper: PaperProfile::default(),
                drawer_open: false,
                next_receipt_id: 1,
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
            paper: inner.paper,
            drawer_open: inner.drawer_open,
        }
    }

    /// Paper for receipts started from now on.
    pub fn paper(&self) -> PaperProfile {
        self.lock().paper
    }

    /// Selects the paper for receipts started from now on. Receipts already
    /// in progress keep theirs.
    pub fn set_paper(&self, paper: PaperProfile) {
        self.lock().paper = paper;
    }

    /// Marks the cash drawer as closed (the user closed it in the UI).
    pub fn close_drawer(&self) {
        self.lock().drawer_open = false;
    }

    fn complete_receipt(
        &self,
        connection_id: u64,
        peer: &str,
        receipt: Receipt,
        sink: &dyn EventSink,
    ) {
        let mut inner = self.lock();
        let id = inner.next_receipt_id;
        inner.next_receipt_id = id.saturating_add(1);
        sink.emit(ListenerEvent::ReceiptCompleted(ReceiptCompleted {
            id,
            connection_id,
            peer: peer.to_owned(),
            receipt,
        }));
    }

    fn kick_drawer(&self, connection_id: u64, peer: &str, kick: DrawerKick, sink: &dyn EventSink) {
        let mut inner = self.lock();
        inner.drawer_open = true;
        sink.emit(ListenerEvent::DrawerKicked(DrawerKicked {
            connection_id,
            peer: peer.to_owned(),
            pin: kick.pin,
            on_ms: kick.on_ms,
            off_ms: kick.off_ms,
        }));
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

/// Per-connection state: the parser, the receipt builder and what is needed
/// to report their output.
///
/// Dropping it ends the connection, whether the task returned (EOF, read
/// error) or was aborted (listener shutdown): the parser is finished and its
/// remaining commands are emitted first, then the pending receipt is
/// completed, then the connection is removed from the shared state. So the
/// last `commands:received` and `receipt:completed` of a connection always
/// precede the `connection:changed` that reports it gone.
struct Connection {
    id: u64,
    peer: String,
    parser: Parser,
    builder: ReceiptBuilder,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
    /// Snapshot waiting for the end of the update interval.
    pending_update: Option<Receipt>,
    /// When the last `receipt:updated` was sent.
    last_update: Option<Instant>,
}

impl Connection {
    fn new(id: u64, peer: String, state: Arc<ListenerState>, sink: Arc<dyn EventSink>) -> Self {
        let builder = ReceiptBuilder::new(ReceiptConfig {
            paper: state.paper(),
            ..ReceiptConfig::default()
        });
        Self {
            id,
            peer,
            parser: Parser::new(ParserConfig::default()),
            builder,
            state,
            sink,
            pending_update: None,
            last_update: None,
        }
    }

    /// Applies a batch of commands to the receipt builder and emits, in
    /// order: the commands, the effects, and the in-progress receipt if it
    /// changed.
    fn process(&mut self, commands: Vec<DecodedCommand>) {
        if commands.is_empty() {
            return;
        }
        // The paper setting is read here; the builder applies it only to
        // receipts that have not started yet.
        self.builder.set_paper(self.state.paper());
        let effects = self.builder.apply_all(&commands);
        self.sink.emit(ListenerEvent::Commands(CommandsReceived {
            connection_id: self.id,
            peer: self.peer.clone(),
            commands,
        }));
        for effect in effects {
            match effect {
                Effect::ReceiptCompleted(receipt) => {
                    // A snapshot of this receipt is obsolete now.
                    self.pending_update = None;
                    self.state
                        .complete_receipt(self.id, &self.peer, receipt, self.sink.as_ref());
                }
                Effect::DrawerKick(kick) => {
                    self.state
                        .kick_drawer(self.id, &self.peer, kick, self.sink.as_ref());
                }
            }
        }
    }

    /// Parses one chunk and emits its commands and effects. The receipt in
    /// progress is reported separately by [`queue_update`](Self::queue_update).
    fn feed(&mut self, chunk: &[u8]) {
        let commands = self.parser.feed(chunk);
        self.process(commands);
    }

    /// Takes the in-progress receipt if it changed, and sends it now or when
    /// the update interval ends.
    fn queue_update(&mut self, now: Instant) {
        if let Some(receipt) = self.builder.take_update() {
            self.pending_update = Some(receipt);
        }
        if self.update_due(now).is_some_and(|due| due <= now) {
            self.flush_update(now);
        }
    }

    /// When the pending snapshot may be sent (`now` if no update was sent
    /// yet), or `None` if nothing is pending.
    fn update_due(&self, now: Instant) -> Option<Instant> {
        self.pending_update.as_ref()?;
        Some(
            self.last_update
                .and_then(|last| last.checked_add(UPDATE_INTERVAL))
                .unwrap_or(now),
        )
    }

    /// Sends the pending snapshot, if any.
    fn flush_update(&mut self, now: Instant) {
        if let Some(receipt) = self.pending_update.take() {
            self.last_update = Some(now);
            self.sink
                .emit(ListenerEvent::ReceiptUpdated(ReceiptUpdated {
                    connection_id: self.id,
                    peer: self.peer.clone(),
                    receipt,
                }));
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let tail = self.parser.finish();
        self.process(tail);
        if let Some(receipt) = self.builder.finish() {
            self.state
                .complete_receipt(self.id, &self.peer, receipt, self.sink.as_ref());
        }
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
    let mut connection = Connection::new(id, peer.to_string(), state, sink);
    let mut buffer = vec![0_u8; READ_BUFFER_SIZE];

    loop {
        let due = connection.update_due(Instant::now());
        tokio::select! {
            result = stream.read(&mut buffer) => match result {
                // Orderly shutdown by the client.
                Ok(0) => break,
                Ok(read) => {
                    let Some(chunk) = buffer.get(..read) else {
                        break;
                    };
                    connection.feed(chunk);
                    connection.queue_update(Instant::now());
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    eprintln!(
                        "phantompos: connection {id} from {} failed: {error}",
                        connection.peer
                    );
                    break;
                }
            },
            // `read` is cancel safe, so racing it with the timer loses nothing.
            () = tokio::time::sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                connection.flush_update(Instant::now());
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

    /// Next event that is not about receipts or the drawer, for tests that
    /// only look at the command log and connections.
    async fn next_log_event(events: &mut UnboundedReceiver<ListenerEvent>) -> ListenerEvent {
        loop {
            match next_event(events).await {
                ListenerEvent::ReceiptUpdated(_)
                | ListenerEvent::ReceiptCompleted(_)
                | ListenerEvent::DrawerKicked(_) => {}
                other => return other,
            }
        }
    }

    async fn next_connections(
        events: &mut UnboundedReceiver<ListenerEvent>,
    ) -> ConnectionsSnapshot {
        match next_log_event(events).await {
            ListenerEvent::Connections(snapshot) => snapshot,
            other => panic!("expected a connections event, got {other:?}"),
        }
    }

    async fn next_commands(events: &mut UnboundedReceiver<ListenerEvent>) -> CommandsReceived {
        match next_log_event(events).await {
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
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100},"connections":{"active":0,"peers":[]},"paper":"80mm","drawerOpen":false}"#
        );
        state.set_paper(PaperProfile::Mm58);
        state.lock().drawer_open = true;
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100},"connections":{"active":0,"peers":[]},"paper":"58mm","drawerOpen":true}"#
        );
    }

    /// The receipt of `bytes` rendered on 80 mm, closed by a cut or the end.
    fn one_receipt(bytes: &[u8]) -> Receipt {
        let mut builder = ReceiptBuilder::new(ReceiptConfig::default());
        builder.apply_all(&parse_default(bytes));
        builder.finish().expect("non-empty receipt")
    }

    #[test]
    fn receipt_and_drawer_payloads_match_the_frontend_contract() {
        let receipt_json = concat!(
            r#"{"paper":"80mm","width":576,"height":30,"cut":null,"elements":[{"type":"line","y":0,"height":24,"baseline":21,"advance":30,"alignment":"left","contentWidth":24,"spans":["#,
            r#"{"text":"Hi","x":0,"y":0,"width":24,"columns":2,"charWidth":12,"charHeight":24,"style":{"font":"a","bold":false,"underline":"off","reverse":false,"widthMultiplier":1,"heightMultiplier":1}}"#,
            r#"]}]}"#
        );
        let updated = ReceiptUpdated {
            connection_id: 3,
            peer: "127.0.0.1:5000".into(),
            receipt: one_receipt(b"Hi\n"),
        };
        assert_eq!(
            json(&updated),
            format!(r#"{{"connectionId":3,"peer":"127.0.0.1:5000","receipt":{receipt_json}}}"#)
        );
        let mut cut = one_receipt(b"Hi\n");
        cut.cut = Some(phantompos_core::CutMode::Partial);
        let completed = ReceiptCompleted {
            id: 9,
            connection_id: 3,
            peer: "127.0.0.1:5000".into(),
            receipt: cut,
        };
        assert_eq!(
            json(&completed),
            format!(
                r#"{{"id":9,"connectionId":3,"peer":"127.0.0.1:5000","receipt":{}}}"#,
                receipt_json.replace(r#""cut":null"#, r#""cut":"partial""#)
            )
        );
        let kicked = DrawerKicked {
            connection_id: 3,
            peer: "127.0.0.1:5000".into(),
            pin: 5,
            on_ms: 100,
            off_ms: 200,
        };
        assert_eq!(
            json(&kicked),
            r#"{"connectionId":3,"peer":"127.0.0.1:5000","pin":5,"onMs":100,"offMs":200}"#
        );
    }

    async fn connect(running: &mut Running) -> (TcpStream, String) {
        let client = TcpStream::connect(("127.0.0.1", running.port))
            .await
            .expect("connect");
        let addr = client.local_addr().expect("client address").to_string();
        assert!(matches!(
            next_event(&mut running.events).await,
            ListenerEvent::Connections(_)
        ));
        (client, addr)
    }

    /// Short names of events, for order checks.
    fn kind(event: &ListenerEvent) -> &'static str {
        match event {
            ListenerEvent::Status(_) => "status",
            ListenerEvent::Connections(_) => "connections",
            ListenerEvent::Commands(_) => "commands",
            ListenerEvent::ReceiptUpdated(_) => "updated",
            ListenerEvent::ReceiptCompleted(_) => "completed",
            ListenerEvent::DrawerKicked(_) => "drawer",
        }
    }

    /// Events until (and including) the next `connection:changed`.
    async fn events_until_disconnect(
        events: &mut UnboundedReceiver<ListenerEvent>,
    ) -> Vec<ListenerEvent> {
        let mut out = Vec::new();
        loop {
            let event = next_event(events).await;
            let done = matches!(event, ListenerEvent::Connections(_));
            out.push(event);
            if done {
                return out;
            }
        }
    }

    #[tokio::test]
    async fn cut_completes_the_receipt_and_events_are_ordered() {
        let mut running = start().await;
        let (mut client, addr) = connect(&mut running).await;

        // Nothing on paper yet: commands only, no receipt:updated.
        client.write_all(b"\x1bE\x01").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");

        client
            .write_all(b"first\n\x1dV\x01second\n")
            .await
            .expect("write");
        let commands = next_commands(&mut running.events).await;
        assert_eq!(commands.peer, addr);
        let completed = match next_event(&mut running.events).await {
            ListenerEvent::ReceiptCompleted(completed) => completed,
            other => panic!("expected receipt:completed, got {other:?}"),
        };
        assert_eq!(completed.connection_id, commands.connection_id);
        assert_eq!(completed.peer, addr);
        assert_eq!(
            completed.receipt.cut,
            Some(phantompos_core::CutMode::Partial)
        );
        assert_eq!(completed.receipt.height, 30);
        let updated = match next_event(&mut running.events).await {
            ListenerEvent::ReceiptUpdated(updated) => updated,
            other => panic!("expected receipt:updated, got {other:?}"),
        };
        assert_eq!(updated.connection_id, commands.connection_id);
        assert_eq!(updated.receipt.cut, None);
        assert_eq!(updated.receipt.elements.len(), 1);

        // A full cut with nothing after it: completed, and no update.
        client.write_all(b"third\n\x1dV\x00").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        let second = match next_event(&mut running.events).await {
            ListenerEvent::ReceiptCompleted(completed) => completed,
            other => panic!("expected receipt:completed, got {other:?}"),
        };
        assert!(second.id > completed.id);
        assert_eq!(second.receipt.cut, Some(phantompos_core::CutMode::Full));
        assert_eq!(second.receipt.elements.len(), 2, "second + third");

        // Nothing pending: the disconnect emits only connection:changed.
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(tail.iter().map(kind).collect::<Vec<_>>(), ["connections"]);
        running.task.abort();
    }

    #[tokio::test]
    async fn pending_content_is_completed_before_the_disconnect() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;

        client.write_all(b"line\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");
        client.write_all(b"no LF").await.expect("write");
        drop(client);

        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(
            tail.iter().map(kind).collect::<Vec<_>>(),
            ["commands", "completed", "connections"]
        );
        let ListenerEvent::ReceiptCompleted(completed) = &tail[1] else {
            panic!("expected receipt:completed");
        };
        assert_eq!(completed.receipt.cut, None);
        assert_eq!(completed.receipt.elements.len(), 2);
        running.task.abort();
    }

    #[tokio::test]
    async fn receipts_from_several_connections_get_increasing_ids() {
        let mut running = start().await;
        let (first, _) = connect(&mut running).await;
        let (second, _) = connect(&mut running).await;
        let mut clients = [first, second];
        let mut ids = Vec::new();
        for (index, text) in [(0, "a"), (1, "b"), (0, "c")] {
            clients[index]
                .write_all(format!("{text}\n\x1dV\x00").as_bytes())
                .await
                .expect("write");
            assert_eq!(kind(&next_event(&mut running.events).await), "commands");
            match next_event(&mut running.events).await {
                ListenerEvent::ReceiptCompleted(completed) => ids.push(completed.id),
                other => panic!("expected receipt:completed, got {other:?}"),
            }
        }
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
        running.task.abort();
    }

    #[tokio::test]
    async fn drawer_kick_opens_the_drawer() {
        let mut running = start().await;
        let (mut client, addr) = connect(&mut running).await;
        assert!(!running.state.snapshot().drawer_open);

        // Pin 5, t1 = 50 (100 ms), t2 = 100 (200 ms); then an invalid pin.
        client
            .write_all(b"\x1bp\x01\x32\x64\x1bp\x07\x01\x01")
            .await
            .expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        match next_event(&mut running.events).await {
            ListenerEvent::DrawerKicked(kicked) => {
                assert_eq!(kicked.peer, addr);
                assert_eq!((kicked.pin, kicked.on_ms, kicked.off_ms), (5, 100, 200));
            }
            other => panic!("expected drawer:kicked, got {other:?}"),
        }
        assert!(running.state.snapshot().drawer_open);
        running.state.close_drawer();
        assert!(!running.state.snapshot().drawer_open);

        // The invalid pin produced nothing; the disconnect has no receipt.
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(tail.iter().map(kind).collect::<Vec<_>>(), ["connections"]);
        running.task.abort();
    }

    #[tokio::test]
    async fn paper_change_applies_to_the_next_receipt_only() {
        let mut running = start().await;
        assert_eq!(running.state.snapshot().paper, PaperProfile::Mm80);
        let (mut client, _) = connect(&mut running).await;

        client
            .write_all(b"started on 80 mm\n")
            .await
            .expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");

        running.state.set_paper(PaperProfile::Mm58);
        assert_eq!(running.state.snapshot().paper, PaperProfile::Mm58);
        client
            .write_all(b"more\n\x1dV\x00next\n")
            .await
            .expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        match next_event(&mut running.events).await {
            ListenerEvent::ReceiptCompleted(completed) => {
                assert_eq!(completed.receipt.paper, PaperProfile::Mm80);
                assert_eq!(completed.receipt.width, 576);
            }
            other => panic!("expected receipt:completed, got {other:?}"),
        }
        match next_event(&mut running.events).await {
            ListenerEvent::ReceiptUpdated(updated) => {
                assert_eq!(updated.receipt.paper, PaperProfile::Mm58);
                assert_eq!(updated.receipt.width, 384);
            }
            other => panic!("expected receipt:updated, got {other:?}"),
        }
        running.task.abort();
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
            match next_log_event(&mut running.events).await {
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

    /// Element kinds and sizes of a receipt, for compact assertions.
    fn layout(receipt: &Receipt) -> Vec<String> {
        receipt
            .elements
            .iter()
            .map(|element| match element {
                phantompos_core::Element::Line(line) => format!("line {}", line.y),
                phantompos_core::Element::Image(image) => format!(
                    "{:?} {},{} {}x{}",
                    image.kind, image.x, image.y, image.width, image.height
                ),
            })
            .collect()
    }

    #[tokio::test]
    async fn images_qr_codes_and_barcodes_arrive_in_the_completed_receipt() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;

        let mut bytes = b"\x1ba\x01".to_vec();
        // 16 x 2 raster, quadruple: 32 x 4 dots, centred.
        bytes.extend_from_slice(b"\x1dv0\x03\x02\x00\x02\x00\xf0\x0f\x0f\xf0");
        // QR Code "PhantomPOS", module 4: 21 x 4 = 84 dots.
        bytes
            .extend_from_slice(b"\x1d(k\x03\x001C\x04\x1d(k\x0d\x001P0PhantomPOS\x1d(k\x03\x001Q0");
        // EAN-13, module 2, 50 dots, HRI below: 95 x 2 = 190 dots.
        bytes.extend_from_slice(b"\x1dw\x02\x1dh\x32\x1dH\x02\x1dk\x02789123456789\x00");
        bytes.extend_from_slice(b"\x1dV\x00");
        client.write_all(&bytes).await.expect("write");

        let commands = next_commands(&mut running.events).await;
        assert_eq!(commands.commands.len(), 10);
        let completed = match next_event(&mut running.events).await {
            ListenerEvent::ReceiptCompleted(completed) => completed,
            other => panic!("expected receipt:completed, got {other:?}"),
        };
        assert_eq!(
            layout(&completed.receipt),
            [
                "Raster 272,0 32x4",
                "QrCode 246,4 84x84",
                "Barcode 193,88 190x50",
                "line 138",
            ]
        );
        assert_eq!(completed.receipt.height, 138 + 24);
        // The payload is the documented JSON shape.
        let value: serde_json::Value = serde_json::from_str(&json(&completed)).expect("valid JSON");
        let raster = &value["receipt"]["elements"][0];
        assert_eq!(raster["type"], "image");
        assert_eq!(raster["kind"], "raster");
        assert_eq!(raster["advance"], 4);
        // Source rows F00F / 0FF0, each dot doubled in both directions:
        // FF0000FF twice, then 00FFFF00 twice.
        assert_eq!(raster["data"], "/wAA//8AAP8A//8AAP//AA==");
        drop(client);
        running.task.abort();
    }

    #[tokio::test]
    async fn raster_image_split_across_many_writes_is_one_command() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;

        // 72 bytes x 100 rows (576 x 100 dots), sent 50 bytes at a time.
        let mut bytes = b"\x1dv0\x00\x48\x00\x64\x00".to_vec();
        bytes.extend((0..72 * 100).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }));
        bytes.extend_from_slice(b"\n");
        for piece in bytes.chunks(50) {
            client.write_all(piece).await.expect("write");
            client.flush().await.expect("flush");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        drop(client);

        let events = events_until_disconnect(&mut running.events).await;
        let commands: Vec<DecodedCommand> = events
            .iter()
            .filter_map(|event| match event {
                ListenerEvent::Commands(batch) => Some(batch.commands.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        let rasters: Vec<_> = commands
            .iter()
            .filter(|c| matches!(c.command, Command::RasterImage { .. }))
            .collect();
        assert_eq!(rasters.len(), 1);
        assert_eq!(rasters[0].bytes.len(), 8 + 72 * 100);
        assert_eq!(commands, parse_default(&bytes));
        let completed: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                ListenerEvent::ReceiptCompleted(completed) => Some(completed),
                _ => None,
            })
            .collect();
        assert_eq!(completed.len(), 1);
        assert_eq!(
            layout(&completed[0].receipt),
            ["Raster 0,0 576x100", "line 100"]
        );
        running.task.abort();
    }

    #[tokio::test]
    async fn updates_are_throttled_and_dropped_when_the_receipt_completes() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;

        // The first change is reported at once.
        client.write_all(b"first\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");

        // A quick series of lines: one read each, far fewer snapshots, and
        // the last snapshot has every line.
        let writes = 20;
        for i in 0..writes {
            client
                .write_all(format!("line {i}\n").as_bytes())
                .await
                .expect("write");
            assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        }
        let mut updates = Vec::new();
        while updates
            .last()
            .is_none_or(|r: &Receipt| r.elements.len() < writes + 1)
        {
            match next_event(&mut running.events).await {
                ListenerEvent::ReceiptUpdated(updated) => updates.push(updated.receipt),
                other => panic!("expected receipt:updated, got {other:?}"),
            }
        }
        assert!(updates.len() < writes, "{} updates", updates.len());

        // A snapshot still pending when the cut arrives is dropped: nothing
        // follows the completed receipt. (The interval may end before the cut
        // is read, so an update may come first.)
        client.write_all(b"last\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        client.write_all(b"\x1dV\x00").await.expect("write");
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        let kinds: Vec<_> = tail
            .iter()
            .map(kind)
            .skip_while(|&k| k == "updated")
            .collect();
        assert_eq!(kinds, ["commands", "completed", "connections"]);
        running.task.abort();
    }
}
