//! Raw TCP listener for the virtual printer.
//!
//! Accepts any number of simultaneous connections. Each connection owns a
//! `phantompos-core` [`Session`] (parser, receipt builder, status replies):
//! every chunk read from the socket is fed to it with the current simulated
//! [`PrinterStatus`], and the decoded commands, receipts, drawer kicks and
//! replies go to an [`EventSink`] (replies are first written back to the
//! socket). Interpreting ESC/POS is the job of `phantompos-core`; this module
//! only routes the session's [`SessionEvent`]s.
//!
//! Event order per read: `commands:received`, `print:suppressed` (if data
//! was discarded in an error state), then `receipt:completed` /
//! `drawer:kicked` (+ `printer:status` when the drawer opened) in stream
//! order, then one `status:sent` per reply written, then `receipt:updated`
//! if the receipt in progress changed (at most one per read). On
//! disconnect: the flushed commands, the pending receipt (if any) and
//! `connection:changed`.
//!
//! The simulated state is global ([`ListenerState::set_printer_status`], drawer
//! kicks). Every change is published on a [`watch`] channel; each connection
//! task wakes up and sends its Automatic Status Back if the client enabled
//! it for an item that changed. Replies are written in the connection task,
//! each bounded by [`REPLY_WRITE_TIMEOUT`]: a client that never reads its
//! replies is disconnected instead of stalling its task forever (other
//! connections have their own tasks and are never affected).
//!
//! `receipt:updated` carries the whole receipt in progress, images included
//! (base64 bitmaps), so it is throttled per connection: the first change is
//! sent at once, later ones at most every [`UPDATE_INTERVAL`] (the latest
//! snapshot is sent when the interval ends). A snapshot still pending when
//! its receipt is completed (cut or disconnect) is dropped: the completed
//! receipt supersedes it.
//!
//! **Settings and restarts.** The bind address and port come from the
//! [`Settings`] (local mode `127.0.0.1`, network mode `0.0.0.0`). The accept
//! loop runs in its own task; [`ListenerState::start`] (startup, the
//! `retry_listener` command, and a change of port or network mode) stops the
//! running loop, if any, and binds again. Stopping is graceful: the loop
//! drops its socket (the port is free once `start` goes on) and then aborts
//! its connection tasks and waits for them, so every old connection is
//! flushed and closed like any disconnect (`commands:received`,
//! `receipt:completed`, `connection:changed`) before `listener:status`
//! reports `stopped` and then `listening` or `error`. Starts are serialized
//! by an async mutex, so concurrent commands never interleave. The app never
//! changes the port on its own: on "port in use" it only suggests the next
//! free one (owner decision).
//!
//! **Raw bytes.** Each connection keeps the bytes of its receipt in progress;
//! when a receipt completes, its own bytes (the range reported by core, see
//! `phantompos_core::SessionEvent::ReceiptCompleted`) are retained under the
//! receipt id for `export_receipt`, for the last [`MAX_RETAINED_RECEIPTS`]
//! receipts and at most [`MAX_RETAINED_BYTES`] in total (oldest evicted
//! first). A receipt whose bytes exceed [`MAX_RETAINED_BYTES`] is not kept.
//!
//! The listener is independent of Tauri so it can be tested with a plain
//! channel as the sink. The Tauri glue lives in `lib.rs`.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use phantompos_core::{
    DecodedCommand, DrawerKick, ParserConfig, PrinterStatus, PrinterStatusPatch, Receipt,
    ReceiptConfig, Reply, Session, SessionEvent, SessionOutput, StatusReply, SuppressReason,
};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::Instant;

use crate::settings::{Settings, SettingsError, SettingsPatch, SettingsStore};

/// Size of the buffer for each socket read; one read produces at most one
/// `commands:received` event.
const READ_BUFFER_SIZE: usize = 4096;

/// Minimum time between two `receipt:updated` events of one connection.
const UPDATE_INTERVAL: Duration = Duration::from_millis(50);

/// Longest time writing one reply may take. Replies are 1-4 bytes, so a write
/// only waits when the client has not read for so long that both socket
/// buffers are full; the connection is then closed.
const REPLY_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// Pause after a failed `accept` so a persistent error (e.g. too many open
/// files) does not turn the accept loop into a busy loop.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

/// How many ports above the configured one are probed for `suggestedPort`.
const SUGGEST_PORT_RANGE: u16 = 100;

/// Completed receipts whose raw bytes are kept for export (as many as the
/// roll shows).
pub const MAX_RETAINED_RECEIPTS: usize = 200;

/// Total raw bytes kept for export; also the most one receipt may have.
pub const MAX_RETAINED_BYTES: usize = 32 * 1024 * 1024;

/// `state` of the `listener:status` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ListenerPhase {
    /// Not started yet, or stopped for a restart.
    Stopped,
    /// Bound and accepting connections.
    Listening,
    /// Could not bind.
    Error,
}

/// Why binding failed, so the UI can show a translated message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BindErrorCode {
    PortInUse,
    PermissionDenied,
    Other,
}

/// Payload of the `listener:status` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenerStatus {
    pub state: ListenerPhase,
    /// Bind address: `127.0.0.1` (local mode) or `0.0.0.0` (network mode).
    pub address: String,
    pub port: u16,
    /// Addresses to show the user: the bind address in local mode; in
    /// network mode every non-loopback, non-link-local IPv4 address of this
    /// machine, sorted (may be empty).
    pub addresses: Vec<String>,
    /// English description of the error, for logs (only on error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Only on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<BindErrorCode>,
    /// Only on `portInUse`: the next port above the configured one that
    /// could be bound on the same address when the error happened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_port: Option<u16>,
}

impl ListenerStatus {
    fn new(state: ListenerPhase, address: IpAddr, port: u16) -> Self {
        Self {
            state,
            address: address.to_string(),
            port,
            addresses: display_addresses(address),
            message: None,
            code: None,
            suggested_port: None,
        }
    }

    pub fn stopped(settings: &Settings) -> Self {
        Self::new(
            ListenerPhase::Stopped,
            settings.network_mode.bind_address(),
            settings.port,
        )
    }

    fn listening(address: IpAddr, port: u16) -> Self {
        Self::new(ListenerPhase::Listening, address, port)
    }

    fn error(error: &ListenerError, suggested_port: Option<u16>) -> Self {
        let (address, port) = error.address();
        Self {
            message: Some(error.to_string()),
            code: Some(error.code()),
            suggested_port,
            ..Self::new(ListenerPhase::Error, address, port)
        }
    }

    /// The same status with the interface addresses read again.
    fn refreshed(&self) -> Self {
        let addresses = self
            .address
            .parse()
            .map_or_else(|_| self.addresses.clone(), display_addresses);
        Self {
            addresses,
            ..self.clone()
        }
    }
}

/// What to show the user for a bind address: the address itself, or for
/// the unspecified address (network mode) every IPv4 address another device
/// could use: loopback (127/8) and link-local (169.254/16, self-assigned when
/// there is no DHCP) are left out. Virtual interfaces (VPN, Docker, VM
/// bridges) are kept: they are real ways to reach this machine and there is
/// no portable way to tell them apart. Sorted, without duplicates.
pub fn display_addresses(bind: IpAddr) -> Vec<String> {
    if !bind.is_unspecified() {
        return vec![bind.to_string()];
    }
    let interfaces = match if_addrs::get_if_addrs() {
        Ok(interfaces) => interfaces,
        Err(error) => {
            eprintln!("phantompos: could not list the network interfaces: {error}");
            return Vec::new();
        }
    };
    let mut addresses: Vec<Ipv4Addr> = interfaces
        .iter()
        .filter_map(|interface| match interface.ip() {
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() => {
                Some(ip)
            }
            _ => None,
        })
        .collect();
    addresses.sort_unstable();
    addresses.dedup();
    addresses.iter().map(ToString::to_string).collect()
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

/// Payload of the `status:sent` event: a reply written to a socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSent {
    pub connection_id: u64,
    pub peer: String,
    /// The request answered (`"DLE EOT 1"`, `"GS r 2"`), or `"ASB"` for
    /// Automatic Status Back.
    pub request: String,
    pub reply: StatusReply,
}

/// Payload of the `print:suppressed` event: bytes of one read (or of the
/// disconnect flush) that were not printed because of an error state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintSuppressed {
    pub connection_id: u64,
    pub peer: String,
    /// `coverOpen`, then `paperOut`, then `offline` when several are active.
    pub reason: SuppressReason,
    pub bytes: u64,
}

/// Return value of the `get_state` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    pub listener: ListenerStatus,
    pub connections: ConnectionsSnapshot,
    pub settings: Settings,
    /// Simulated printer state.
    pub status: PrinterStatus,
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
    PrinterStatus(PrinterStatus),
    StatusSent(StatusSent),
    PrintSuppressed(PrintSuppressed),
    SettingsChanged(Settings),
}

/// Receives listener events. Implementations must not block for long: status,
/// connection, receipt-completed, drawer and printer-status events are emitted
/// while the state lock is held, so that the order of events always matches the order of
/// state changes (and receipt ids reach the UI in increasing order).
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: ListenerEvent);
}

#[derive(Debug, thiserror::Error)]
pub enum ListenerError {
    #[error("Port {port} on {address} is already in use by another program.")]
    AddrInUse { address: IpAddr, port: u16 },
    #[error("Permission denied to listen on {address}:{port}: {source}")]
    PermissionDenied {
        address: IpAddr,
        port: u16,
        #[source]
        source: io::Error,
    },
    #[error("Could not listen on {address}:{port}: {source}")]
    Bind {
        address: IpAddr,
        port: u16,
        #[source]
        source: io::Error,
    },
}

impl ListenerError {
    pub fn code(&self) -> BindErrorCode {
        match self {
            Self::AddrInUse { .. } => BindErrorCode::PortInUse,
            Self::PermissionDenied { .. } => BindErrorCode::PermissionDenied,
            Self::Bind { .. } => BindErrorCode::Other,
        }
    }

    fn address(&self) -> (IpAddr, u16) {
        match self {
            Self::AddrInUse { address, port }
            | Self::PermissionDenied { address, port, .. }
            | Self::Bind { address, port, .. } => (*address, *port),
        }
    }
}

/// Why a receipt cannot be exported.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RetainedError {
    #[error("There is no receipt {0}.")]
    Unknown(u64),
    #[error(
        "The raw bytes of receipt {0} are no longer available: the app keeps those of the last \
         200 receipts (32 MiB at most) until the roll is cleared."
    )]
    Expired(u64),
}

/// Raw bytes of completed receipts, by id, oldest first.
#[derive(Debug)]
struct Retained {
    entries: VecDeque<(u64, Arc<[u8]>)>,
    total: usize,
    max_count: usize,
    max_bytes: usize,
}

impl Retained {
    fn new(max_count: usize, max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            total: 0,
            max_count,
            max_bytes,
        }
    }

    /// Keeps `bytes` for `id` (ids are inserted in increasing order) and
    /// evicts the oldest entries beyond the caps.
    fn insert(&mut self, id: u64, bytes: Vec<u8>) {
        if bytes.len() > self.max_bytes {
            return;
        }
        self.total = self.total.saturating_add(bytes.len());
        self.entries.push_back((id, bytes.into()));
        while self.entries.len() > self.max_count || self.total > self.max_bytes {
            match self.entries.pop_front() {
                Some((_, evicted)) => self.total = self.total.saturating_sub(evicted.len()),
                None => break,
            }
        }
    }

    fn get(&self, id: u64) -> Option<Arc<[u8]>> {
        self.entries
            .iter()
            .find(|(entry, _)| *entry == id)
            .map(|(_, bytes)| bytes.clone())
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total = 0;
    }
}

/// The running accept loop.
#[derive(Debug)]
struct Running {
    /// Dropping or sending stops the loop.
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

/// Shared listener state, read by the commands and updated by the listener
/// tasks.
#[derive(Debug)]
pub struct ListenerState {
    inner: Mutex<Inner>,
    next_connection_id: AtomicU64,
    /// Publishes every change of the simulated state to the connections.
    status_tx: watch::Sender<PrinterStatus>,
    /// The accept loop; held across awaits so starts never interleave.
    running: tokio::sync::Mutex<Option<Running>>,
    /// Where settings are saved; `None` keeps them in memory only.
    store: Option<SettingsStore>,
}

#[derive(Debug)]
struct Inner {
    status: ListenerStatus,
    peers: BTreeMap<u64, SocketAddr>,
    settings: Settings,
    /// Simulated printer state.
    printer: PrinterStatus,
    /// Id of the next completed receipt.
    next_receipt_id: u64,
    retained: Retained,
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
    /// The state for `settings`, saved to `store` when they change. The
    /// listener is stopped until [`start`](Self::start).
    pub fn new(settings: Settings, store: Option<SettingsStore>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: ListenerStatus::stopped(&settings),
                peers: BTreeMap::new(),
                settings,
                printer: PrinterStatus::default(),
                next_receipt_id: 1,
                retained: Retained::new(MAX_RETAINED_RECEIPTS, MAX_RETAINED_BYTES),
            }),
            next_connection_id: AtomicU64::new(1),
            status_tx: watch::Sender::new(PrinterStatus::default()),
            running: tokio::sync::Mutex::new(None),
            store,
        }
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings
    }

    /// Everything the UI needs to sync; the network addresses are read
    /// again (interfaces come and go).
    pub fn snapshot(&self) -> StateSnapshot {
        let inner = self.lock();
        StateSnapshot {
            listener: inner.status.refreshed(),
            connections: inner.connections(),
            settings: inner.settings,
            status: inner.printer,
        }
    }

    /// (Re)starts the listener with the current settings: stops the running
    /// one (closing its connections), then binds. The result is reported as
    /// `listener:status` (`stopped` first if a listener was running).
    pub async fn start(self: &Arc<Self>, sink: Arc<dyn EventSink>) {
        let mut running = self.running.lock().await;
        self.restart_locked(&mut running, sink).await;
    }

    /// Stops the listener, if running, and closes its connections.
    #[cfg(test)]
    pub async fn stop(&self, sink: &dyn EventSink) {
        let mut running = self.running.lock().await;
        if let Some(current) = running.take() {
            Self::shut_down(current).await;
            self.set_status(ListenerStatus::stopped(&self.settings()), sink);
        }
    }

    async fn restart_locked(
        self: &Arc<Self>,
        running: &mut Option<Running>,
        sink: Arc<dyn EventSink>,
    ) {
        if let Some(current) = running.take() {
            Self::shut_down(current).await;
            self.set_status(ListenerStatus::stopped(&self.settings()), sink.as_ref());
        }
        let settings = self.settings();
        let address = settings.network_mode.bind_address();
        match bind(address, settings.port).await {
            Ok(listener) => {
                // The port actually bound (differs only for port 0, in tests).
                let port = listener
                    .local_addr()
                    .map_or(settings.port, |local| local.port());
                self.set_status(ListenerStatus::listening(address, port), sink.as_ref());
                let (shutdown, shutdown_rx) = oneshot::channel();
                let task = tokio::spawn(serve(listener, self.clone(), sink, shutdown_rx));
                *running = Some(Running { shutdown, task });
            }
            Err(error) => {
                let suggested = match error {
                    ListenerError::AddrInUse { .. } => suggest_port(address, settings.port).await,
                    _ => None,
                };
                self.set_status(ListenerStatus::error(&error, suggested), sink.as_ref());
            }
        }
    }

    async fn shut_down(running: Running) {
        let Running { shutdown, task } = running;
        // The loop may already be gone (it never ends on its own, but a
        // panic would end it); either way it is finished after the await.
        let _ = shutdown.send(());
        if let Err(error) = task.await
            && error.is_panic()
        {
            eprintln!("phantompos: the listener task panicked: {error}");
        }
    }

    /// Merges `patch` into the settings, saves them and emits
    /// `settings:changed` if they changed; restarts the listener if the port
    /// or the network mode changed. Returns the new settings.
    pub async fn update_settings(
        self: &Arc<Self>,
        patch: SettingsPatch,
        sink: Arc<dyn EventSink>,
    ) -> Result<Settings, SettingsError> {
        let mut running = self.running.lock().await;
        let (old, new) = {
            let mut inner = self.lock();
            let old = inner.settings;
            let new = old.merged(patch)?;
            if new != old {
                inner.settings = new;
                sink.emit(ListenerEvent::SettingsChanged(new));
            }
            (old, new)
        };
        if new == old {
            return Ok(new);
        }
        if let Some(store) = &self.store
            && let Err(error) = store.save(&new)
        {
            // Still applied for this run.
            eprintln!(
                "phantompos: could not save the settings to {}: {error}",
                store.path().display()
            );
        }
        if new.port != old.port || new.network_mode != old.network_mode {
            self.restart_locked(&mut running, sink).await;
        }
        Ok(new)
    }

    /// Current simulated printer state.
    pub fn printer_status(&self) -> PrinterStatus {
        self.lock().printer
    }

    /// Merges `patch` into the simulated state and returns the new state.
    /// A change is published to the connections (ASB) and emitted as
    /// `printer:status`.
    pub fn set_printer_status(
        &self,
        patch: PrinterStatusPatch,
        sink: &dyn EventSink,
    ) -> PrinterStatus {
        let mut inner = self.lock();
        let status = inner.printer.merged(patch);
        Self::store_printer_status(&mut inner, status, &self.status_tx, sink);
        status
    }

    fn store_printer_status(
        inner: &mut Inner,
        status: PrinterStatus,
        status_tx: &watch::Sender<PrinterStatus>,
        sink: &dyn EventSink,
    ) {
        if inner.printer == status {
            return;
        }
        inner.printer = status;
        status_tx.send_replace(status);
        sink.emit(ListenerEvent::PrinterStatus(status));
    }

    /// Receives every change of the simulated state.
    fn subscribe(&self) -> watch::Receiver<PrinterStatus> {
        self.status_tx.subscribe()
    }

    /// The raw bytes of completed receipt `id`.
    pub fn receipt_bytes(&self, id: u64) -> Result<Arc<[u8]>, RetainedError> {
        let inner = self.lock();
        Self::check_id(&inner, id)?;
        inner.retained.get(id).ok_or(RetainedError::Expired(id))
    }

    /// Ok if `id` is a receipt that was completed in this run.
    pub fn check_receipt_id(&self, id: u64) -> Result<(), RetainedError> {
        Self::check_id(&self.lock(), id)
    }

    fn check_id(inner: &Inner, id: u64) -> Result<(), RetainedError> {
        if id == 0 || id >= inner.next_receipt_id {
            return Err(RetainedError::Unknown(id));
        }
        Ok(())
    }

    /// Forgets the raw bytes of every completed receipt ("Clear roll").
    pub fn clear_receipts(&self) {
        self.lock().retained.clear();
    }

    fn complete_receipt(
        &self,
        connection_id: u64,
        peer: &str,
        receipt: Receipt,
        raw: Option<Vec<u8>>,
        sink: &dyn EventSink,
    ) {
        let mut inner = self.lock();
        let id = inner.next_receipt_id;
        inner.next_receipt_id = id.saturating_add(1);
        if let Some(raw) = raw {
            inner.retained.insert(id, raw);
        }
        sink.emit(ListenerEvent::ReceiptCompleted(ReceiptCompleted {
            id,
            connection_id,
            peer: peer.to_owned(),
            receipt,
        }));
    }

    fn kick_drawer(&self, connection_id: u64, peer: &str, kick: DrawerKick, sink: &dyn EventSink) {
        let mut inner = self.lock();
        sink.emit(ListenerEvent::DrawerKicked(DrawerKicked {
            connection_id,
            peer: peer.to_owned(),
            pin: kick.pin,
            on_ms: kick.on_ms,
            off_ms: kick.off_ms,
        }));
        let opened = PrinterStatus {
            drawer_open: true,
            ..inner.printer
        };
        Self::store_printer_status(&mut inner, opened, &self.status_tx, sink);
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

/// Binds a TCP listener on `address:port`.
pub async fn bind(address: IpAddr, port: u16) -> Result<TcpListener, ListenerError> {
    TcpListener::bind(SocketAddr::new(address, port))
        .await
        .map_err(|source| match source.kind() {
            io::ErrorKind::AddrInUse => ListenerError::AddrInUse { address, port },
            // Unix ports below 1024 without privileges; Windows ports in an
            // excluded range (WSAEACCES).
            io::ErrorKind::PermissionDenied => ListenerError::PermissionDenied {
                address,
                port,
                source,
            },
            _ => ListenerError::Bind {
                address,
                port,
                source,
            },
        })
}

/// The first port above `port` (at most [`SUGGEST_PORT_RANGE`] above) that
/// can be bound on `address` right now, found by binding it and letting it
/// go. Another program may take it before the user accepts the suggestion;
/// that attempt then reports its own error.
pub async fn suggest_port(address: IpAddr, port: u16) -> Option<u16> {
    let last = port.saturating_add(SUGGEST_PORT_RANGE);
    let mut candidate = port;
    while candidate < last {
        candidate = candidate.saturating_add(1);
        if TcpListener::bind(SocketAddr::new(address, candidate))
            .await
            .is_ok()
        {
            return Some(candidate);
        }
    }
    None
}

/// Accepts connections until `shutdown` fires (or its sender is dropped),
/// then closes the socket and every connection it accepted, waiting until
/// they are flushed (see [`Connection`]'s `Drop`).
async fn serve(
    listener: TcpListener,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
    mut shutdown: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
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
    // Free the port first, then end the connections.
    drop(listener);
    connections.shutdown().await;
}

/// Bytes of a connection's receipt in progress (and of what follows it in
/// the stream so far), for [`Retained`].
#[derive(Debug, Default)]
struct RawBuffer {
    bytes: Vec<u8>,
    /// Stream offset of `bytes[0]`.
    start: u64,
    /// Stream offset where the receipt in progress starts.
    receipt_start: u64,
}

impl RawBuffer {
    fn push(&mut self, chunk: &[u8]) {
        let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        if self.bytes.len().saturating_add(chunk.len()) > MAX_RETAINED_BYTES {
            // Too big to keep: this receipt (and the start of the next one,
            // if it began in what is dropped) cannot be exported.
            let held = u64::try_from(self.bytes.len()).unwrap_or(u64::MAX);
            self.start = self.start.saturating_add(held).saturating_add(len);
            self.bytes.clear();
        } else {
            self.bytes.extend_from_slice(chunk);
        }
    }

    /// The bytes of the receipt that ends at stream offset `end`, or `None`
    /// if some were not kept. The next receipt starts at `end`.
    fn take(&mut self, end: u64) -> Option<Vec<u8>> {
        let complete = self.receipt_start >= self.start;
        self.receipt_start = end;
        let held = end.saturating_sub(self.start);
        let held = usize::try_from(held)
            .unwrap_or(usize::MAX)
            .min(self.bytes.len());
        let taken: Vec<u8> = self.bytes.drain(..held).collect();
        self.start = self
            .start
            .saturating_add(u64::try_from(held).unwrap_or(u64::MAX));
        complete.then_some(taken)
    }
}

/// Per-connection state: the core session and what is needed to report its
/// output.
///
/// Dropping it ends the connection, whether the task returned (EOF, read
/// error, failed reply) or was aborted (listener shutdown): the session is
/// finished and its remaining commands are emitted first, then the pending
/// receipt is completed, then the connection is removed from the shared
/// state. So the last `commands:received` and `receipt:completed` of a
/// connection always precede the `connection:changed` that reports it gone.
struct Connection {
    id: u64,
    peer: String,
    session: Session,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
    /// Snapshot waiting for the end of the update interval.
    pending_update: Option<Receipt>,
    /// When the last `receipt:updated` was sent.
    last_update: Option<Instant>,
    /// Bytes of the receipt in progress, for export.
    raw: RawBuffer,
}

impl Connection {
    fn new(id: u64, peer: String, state: Arc<ListenerState>, sink: Arc<dyn EventSink>) -> Self {
        // The default code page applies to connections accepted from now
        // on; the paper is read again on every read (see `feed`).
        let settings = state.settings();
        let session = Session::new(
            ParserConfig {
                default_code_page: settings.default_code_page,
            },
            ReceiptConfig {
                paper: settings.paper,
                ..ReceiptConfig::default()
            },
            state.printer_status(),
        );
        Self {
            id,
            peer,
            session,
            state,
            sink,
            pending_update: None,
            last_update: None,
            raw: RawBuffer::default(),
        }
    }

    /// Emits a batch's commands and events, in order, and returns the
    /// replies to write to the socket.
    fn process(&mut self, output: SessionOutput) -> Vec<Reply> {
        let SessionOutput {
            commands,
            events,
            suppressed,
        } = output;
        if !commands.is_empty() {
            self.sink.emit(ListenerEvent::Commands(CommandsReceived {
                connection_id: self.id,
                peer: self.peer.clone(),
                commands,
            }));
        }
        if let Some(suppressed) = suppressed {
            self.sink
                .emit(ListenerEvent::PrintSuppressed(PrintSuppressed {
                    connection_id: self.id,
                    peer: self.peer.clone(),
                    reason: suppressed.reason,
                    bytes: suppressed.bytes,
                }));
        }
        let mut replies = Vec::new();
        for event in events {
            match event {
                SessionEvent::ReceiptCompleted { receipt, end } => {
                    // A snapshot of this receipt is obsolete now.
                    self.pending_update = None;
                    let raw = self.raw.take(end);
                    self.state.complete_receipt(
                        self.id,
                        &self.peer,
                        receipt,
                        raw,
                        self.sink.as_ref(),
                    );
                }
                SessionEvent::DrawerKick(kick) => {
                    self.state
                        .kick_drawer(self.id, &self.peer, kick, self.sink.as_ref());
                }
                SessionEvent::Reply(reply) => replies.push(reply),
            }
        }
        replies
    }

    /// Parses one chunk, emits its commands and effects, and returns the
    /// replies to write. The receipt in progress is reported separately by
    /// [`queue_update`](Self::queue_update).
    fn feed(&mut self, chunk: &[u8]) -> Vec<Reply> {
        // The paper setting is read here; the builder applies it only to
        // receipts that have not started yet.
        self.session.set_paper(self.state.settings().paper);
        self.raw.push(chunk);
        let output = self.session.feed(chunk, self.state.printer_status());
        self.process(output)
    }

    /// The shared state changed: the ASB status to send, if any.
    fn status_changed(&mut self, status: PrinterStatus) -> Option<Reply> {
        self.session.status_changed(status)
    }

    /// Reports a reply that was written to the socket.
    fn reply_sent(&self, reply: Reply) {
        self.sink.emit(ListenerEvent::StatusSent(StatusSent {
            connection_id: self.id,
            peer: self.peer.clone(),
            request: reply.request,
            reply: reply.reply,
        }));
    }

    /// Takes the in-progress receipt if it changed, and sends it now or when
    /// the update interval ends.
    fn queue_update(&mut self, now: Instant) {
        if let Some(receipt) = self.session.take_update() {
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
        let output = self.session.finish(self.state.printer_status());
        // Replies produced now (an ASB catching up with the state) cannot be
        // written any more; they are dropped without a `status:sent`.
        let _ = self.process(output);
        self.state.remove_connection(self.id, self.sink.as_ref());
    }
}

/// Writes `replies` in order, reporting each one after it was written.
/// Returns false when the socket failed or the client stopped reading for
/// [`REPLY_WRITE_TIMEOUT`]; the connection must then be closed.
async fn write_replies(
    stream: &mut TcpStream,
    connection: &Connection,
    replies: Vec<Reply>,
) -> bool {
    for reply in replies {
        let written =
            tokio::time::timeout(REPLY_WRITE_TIMEOUT, stream.write_all(&reply.reply.bytes)).await;
        match written {
            Ok(Ok(())) => connection.reply_sent(reply),
            Ok(Err(error)) => {
                eprintln!(
                    "phantompos: connection {} from {}: reply not sent: {error}",
                    connection.id, connection.peer
                );
                return false;
            }
            Err(_) => {
                eprintln!(
                    "phantompos: connection {} from {}: client is not reading replies; closing",
                    connection.id, connection.peer
                );
                return false;
            }
        }
    }
    true
}

async fn serve_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ListenerState>,
    sink: Arc<dyn EventSink>,
) {
    let id = state.add_connection(peer, sink.as_ref());
    let mut status_rx = state.subscribe();
    let mut connection = Connection::new(id, peer.to_string(), state, sink);
    let mut buffer = vec![0_u8; READ_BUFFER_SIZE];
    let mut watching = true;

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
                    let replies = connection.feed(chunk);
                    if !write_replies(&mut stream, &connection, replies).await {
                        break;
                    }
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
            // The simulated state changed (UI or a drawer kick anywhere).
            changed = status_rx.changed(), if watching => match changed {
                Ok(()) => {
                    let status = *status_rx.borrow_and_update();
                    if let Some(reply) = connection.status_changed(status)
                        && !write_replies(&mut stream, &connection, vec![reply]).await
                    {
                        break;
                    }
                }
                // The state is gone (app shutting down): stop watching.
                Err(_) => watching = false,
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
    use crate::export;
    use phantompos_core::{
        CodePage, Command, PaperProfile, ReceiptBuilder, UnknownReason, parse, status_reply,
    };
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
    use tokio::time::timeout;

    const TIMEOUT: Duration = Duration::from_secs(5);

    impl EventSink for UnboundedSender<ListenerEvent> {
        fn emit(&self, event: ListenerEvent) {
            // The test may have stopped listening; that is fine.
            let _ = self.send(event);
        }
    }

    fn localhost(port: u16) -> Settings {
        Settings {
            port,
            ..Settings::default()
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
                | ListenerEvent::DrawerKicked(_)
                | ListenerEvent::PrinterStatus(_)
                | ListenerEvent::StatusSent(_)
                | ListenerEvent::PrintSuppressed(_) => {}
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
        sink: Arc<UnboundedSender<ListenerEvent>>,
        port: u16,
    }

    /// Starts a listener with `settings` and waits until it is bound.
    async fn start_with(settings: Settings) -> Running {
        let state = Arc::new(ListenerState::new(settings, None));
        let (sender, mut events) = unbounded_channel();
        let sink = Arc::new(sender);
        state.start(sink.clone()).await;
        let port = match next_event(&mut events).await {
            ListenerEvent::Status(ListenerStatus {
                state: ListenerPhase::Listening,
                port,
                ..
            }) => port,
            other => panic!("expected a listening status, got {other:?}"),
        };
        assert_ne!(port, 0);
        Running {
            state,
            events,
            sink,
            port,
        }
    }

    /// Starts a local listener on an ephemeral port.
    async fn start() -> Running {
        let running = start_with(localhost(0)).await;
        assert_eq!(
            running.state.snapshot().listener.address,
            "127.0.0.1".to_owned()
        );
        running
    }

    #[test]
    fn default_settings_are_local_port_9100() {
        let status = ListenerStatus::stopped(&Settings::default());
        assert_eq!(status.address, "127.0.0.1");
        assert_eq!(status.port, 9100);
        assert_eq!(status.addresses, ["127.0.0.1"]);
    }

    fn json<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).expect("payload serializes")
    }

    #[test]
    fn payloads_match_the_frontend_contract() {
        let in_use = ListenerStatus::error(
            &ListenerError::AddrInUse {
                address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 9100,
            },
            Some(9101),
        );
        assert_eq!(
            json(&in_use),
            r#"{"state":"error","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"],"message":"Port 9100 on 127.0.0.1 is already in use by another program.","code":"portInUse","suggestedPort":9101}"#
        );
        let no_suggestion = ListenerStatus::error(
            &ListenerError::AddrInUse {
                address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 9100,
            },
            None,
        );
        assert!(!json(&no_suggestion).contains("suggestedPort"));
        let denied = ListenerStatus::error(
            &ListenerError::PermissionDenied {
                address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 80,
                source: io::Error::from(io::ErrorKind::PermissionDenied),
            },
            None,
        );
        let denied: serde_json::Value = serde_json::from_str(&json(&denied)).expect("JSON");
        assert_eq!(denied["code"], "permissionDenied");
        assert_eq!(denied["port"], 80);
        let other = ListenerStatus::error(
            &ListenerError::Bind {
                address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                port: 9100,
                source: io::Error::from(io::ErrorKind::AddrNotAvailable),
            },
            None,
        );
        let other: serde_json::Value = serde_json::from_str(&json(&other)).expect("JSON");
        assert_eq!(other["code"], "other");
        assert_eq!(other["address"], "0.0.0.0");
        assert!(other["addresses"].is_array());
        assert!(other.get("suggestedPort").is_none());
        let listening = ListenerStatus::listening(IpAddr::V4(Ipv4Addr::LOCALHOST), 9100);
        assert_eq!(
            json(&listening),
            r#"{"state":"listening","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"]}"#
        );
        assert_eq!(
            json(&ListenerStatus::stopped(&Settings::default())),
            r#"{"state":"stopped","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"]}"#
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
        let state = ListenerState::new(Settings::default(), None);
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"]},"connections":{"active":0,"peers":[]},"settings":{"port":9100,"networkMode":"local","paper":"80mm","defaultCodePage":"cp437"},"status":{"paperOut":false,"coverOpen":false,"offline":false,"drawerOpen":false}}"#
        );
        state.lock().settings.paper = PaperProfile::Mm58;
        state.lock().printer.drawer_open = true;
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"]},"connections":{"active":0,"peers":[]},"settings":{"port":9100,"networkMode":"local","paper":"58mm","defaultCodePage":"cp437"},"status":{"paperOut":false,"coverOpen":false,"offline":false,"drawerOpen":true}}"#
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
        let sent = StatusSent {
            connection_id: 3,
            peer: "127.0.0.1:5000".into(),
            request: "DLE EOT 1".into(),
            reply: phantompos_core::status_reply(
                &parse_default(b"\x10\x04\x01")[0].command,
                &PrinterStatus {
                    cover_open: true,
                    ..PrinterStatus::default()
                },
            )
            .expect("DLE EOT 1 is answered"),
        };
        assert_eq!(
            json(&sent),
            r#"{"connectionId":3,"peer":"127.0.0.1:5000","request":"DLE EOT 1","reply":{"kind":"realTime","bytes":[26],"flags":["offline","drawerLow"]}}"#
        );
        let suppressed = PrintSuppressed {
            connection_id: 3,
            peer: "127.0.0.1:5000".into(),
            reason: SuppressReason::PaperOut,
            bytes: 12,
        };
        assert_eq!(
            json(&suppressed),
            r#"{"connectionId":3,"peer":"127.0.0.1:5000","reason":"paperOut","bytes":12}"#
        );
        assert_eq!(
            json(&PrinterStatus::default()),
            r#"{"paperOut":false,"coverOpen":false,"offline":false,"drawerOpen":false}"#
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
            ListenerEvent::PrinterStatus(_) => "printer",
            ListenerEvent::StatusSent(_) => "sent",
            ListenerEvent::PrintSuppressed(_) => "suppressed",
            ListenerEvent::SettingsChanged(_) => "settings",
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
    }

    #[tokio::test]
    async fn drawer_kick_opens_the_drawer() {
        let mut running = start().await;
        let (mut client, addr) = connect(&mut running).await;
        assert!(!running.state.snapshot().status.drawer_open);

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
        match next_event(&mut running.events).await {
            ListenerEvent::PrinterStatus(status) => assert!(status.drawer_open),
            other => panic!("expected printer:status, got {other:?}"),
        }
        assert!(running.state.snapshot().status.drawer_open);

        // GS r 2 reports the drawer kick-out connector pin 3 HIGH.
        client.write_all(b"\x1dr\x02").await.expect("write");
        assert_eq!(read_reply(&mut client, 1).await, [0x01]);
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        match next_event(&mut running.events).await {
            ListenerEvent::StatusSent(sent) => {
                assert_eq!(sent.request, "GS r 2");
                assert_eq!(sent.reply.bytes, [0x01]);
            }
            other => panic!("expected status:sent, got {other:?}"),
        }

        // The user closes the drawer: GS r 2 reports LOW again.
        let (ui, mut ui_events) = unbounded_channel();
        running.state.set_printer_status(
            PrinterStatusPatch {
                drawer_open: Some(false),
                ..PrinterStatusPatch::default()
            },
            &ui,
        );
        assert!(matches!(
            ui_events.try_recv(),
            Ok(ListenerEvent::PrinterStatus(PrinterStatus {
                drawer_open: false,
                ..
            }))
        ));
        client.write_all(b"\x1dr\x02").await.expect("write");
        assert_eq!(read_reply(&mut client, 1).await, [0x00]);
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "sent");

        // The invalid pin produced nothing; the disconnect has no receipt.
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(tail.iter().map(kind).collect::<Vec<_>>(), ["connections"]);
    }

    #[tokio::test]
    async fn paper_change_applies_to_the_next_receipt_only() {
        let mut running = start().await;
        assert_eq!(running.state.snapshot().settings.paper, PaperProfile::Mm80);
        let (mut client, _) = connect(&mut running).await;

        client
            .write_all(b"started on 80 mm\n")
            .await
            .expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");

        let paper = serde_json::from_str(r#"{"paper":"58mm"}"#).expect("patch");
        running
            .state
            .update_settings(paper, running.sink.clone())
            .await
            .expect("valid settings");
        assert!(matches!(
            next_event(&mut running.events).await,
            ListenerEvent::SettingsChanged(Settings {
                paper: PaperProfile::Mm58,
                ..
            })
        ));
        assert_eq!(running.state.snapshot().settings.paper, PaperProfile::Mm58);
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
            b"E\x00\x1dZ\x1d!\x11big\n",
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

        running.state.stop(running.sink.as_ref()).await;
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

        let error = bind(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
            .await
            .expect_err("port is taken");
        assert!(matches!(error, ListenerError::AddrInUse { port: p, .. } if p == port));

        let state = Arc::new(ListenerState::new(localhost(port), None));
        let (sender, mut events) = unbounded_channel();
        timeout(TIMEOUT, state.start(Arc::new(sender)))
            .await
            .expect("start returns after a bind failure");

        let ListenerEvent::Status(status) = next_event(&mut events).await else {
            panic!("expected a status");
        };
        assert_eq!(status.state, ListenerPhase::Error);
        assert_eq!(status.message, Some(error.to_string()));
        assert_eq!(status.code, Some(BindErrorCode::PortInUse));
        assert_eq!(state.snapshot().listener, status);
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
    }

    // ---- Stage 5: status ------------------------------------------------

    /// Reads exactly `len` reply bytes from the client socket.
    async fn read_reply(client: &mut TcpStream, len: usize) -> Vec<u8> {
        let mut buf = vec![0_u8; len];
        timeout(TIMEOUT, client.read_exact(&mut buf))
            .await
            .expect("timed out waiting for a reply")
            .expect("reply read");
        buf
    }

    /// Asserts that nothing arrives on the client socket for a while.
    async fn assert_no_reply(client: &mut TcpStream) {
        let mut buf = [0_u8; 16];
        match timeout(Duration::from_millis(250), client.read(&mut buf)).await {
            Err(_) => {}
            Ok(result) => panic!("unexpected data from the printer: {result:?} {buf:?}"),
        }
    }

    /// Sets the simulated state as the UI does (events to a throwaway sink).
    fn set_state(state: &ListenerState, status: PrinterStatus) -> PrinterStatus {
        let (ui, _events) = unbounded_channel();
        state.set_printer_status(
            PrinterStatusPatch {
                paper_out: Some(status.paper_out),
                cover_open: Some(status.cover_open),
                offline: Some(status.offline),
                drawer_open: Some(status.drawer_open),
            },
            &ui,
        )
    }

    /// Next event of the given kind, skipping the others.
    async fn next_of(events: &mut UnboundedReceiver<ListenerEvent>, wanted: &str) -> ListenerEvent {
        loop {
            let event = next_event(events).await;
            if kind(&event) == wanted {
                return event;
            }
        }
    }

    fn all_states() -> Vec<PrinterStatus> {
        (0..16_u8)
            .map(|bits| PrinterStatus {
                paper_out: bits & 1 != 0,
                cover_open: bits & 2 != 0,
                offline: bits & 4 != 0,
                drawer_open: bits & 8 != 0,
            })
            .collect()
    }

    #[tokio::test]
    async fn status_replies_match_core_in_every_state() {
        let mut running = start().await;
        let (mut client, addr) = connect(&mut running).await;
        // Every answered request; GS r 1/49 and 2/50, DLE DC4 7 1 (4 bytes).
        let requests: &[u8] = b"\x10\x04\x01\x10\x04\x02\x10\x04\x03\x10\x04\x04\
            \x1dr\x01\x1dr1\x1dr\x02\x1dr2\x10\x14\x07\x01";
        let commands = parse_default(requests);
        for status in all_states() {
            assert_eq!(set_state(&running.state, status), status);
            let expected: Vec<StatusReply> = commands
                .iter()
                .map(|c| status_reply(&c.command, &status).expect("answered"))
                .collect();
            let expected_bytes: Vec<u8> = expected.iter().flat_map(|r| r.bytes.clone()).collect();
            client.write_all(requests).await.expect("write");
            assert_eq!(
                read_reply(&mut client, expected_bytes.len()).await,
                expected_bytes,
                "{status:?}"
            );
            assert_eq!(kind(&next_event(&mut running.events).await), "commands");
            for reply in expected {
                match next_event(&mut running.events).await {
                    ListenerEvent::StatusSent(sent) => {
                        assert_eq!(sent.peer, addr);
                        assert_eq!(sent.reply, reply);
                    }
                    other => panic!("expected status:sent, got {other:?}"),
                }
            }
        }
    }

    #[tokio::test]
    async fn asb_reaches_only_the_connection_that_enabled_it() {
        let mut running = start().await;
        let (mut asb, asb_addr) = connect(&mut running).await;
        let (mut other, _) = connect(&mut running).await;

        // Enabling sends the current status at once.
        asb.write_all(b"\x1da\x02").await.expect("write");
        assert_eq!(read_reply(&mut asb, 4).await, [0x10, 0, 0, 0]);
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        match next_event(&mut running.events).await {
            ListenerEvent::StatusSent(sent) => {
                assert_eq!(sent.request, "ASB");
                assert_eq!(sent.peer, asb_addr);
                assert_eq!(sent.reply.kind, phantompos_core::ReplyKind::Automatic);
            }
            other => panic!("expected status:sent, got {other:?}"),
        }

        // A change of an enabled item: 4 bytes to that connection only.
        set_state(
            &running.state,
            PrinterStatus {
                cover_open: true,
                ..PrinterStatus::default()
            },
        );
        assert_eq!(read_reply(&mut asb, 4).await, [0x38, 0, 0, 0]);
        assert!(matches!(
            next_of(&mut running.events, "sent").await,
            ListenerEvent::StatusSent(StatusSent { request, .. }) if request == "ASB"
        ));
        assert_no_reply(&mut other).await;

        // The drawer item is not enabled: no ASB for a drawer change.
        set_state(
            &running.state,
            PrinterStatus {
                cover_open: true,
                drawer_open: true,
                ..PrinterStatus::default()
            },
        );
        assert_no_reply(&mut asb).await;

        // Disabled: nothing more.
        asb.write_all(b"\x1da\x00").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        set_state(&running.state, PrinterStatus::default());
        assert_no_reply(&mut asb).await;
        assert_no_reply(&mut other).await;
    }

    #[tokio::test]
    async fn drawer_kick_sends_asb_to_every_connection_that_enabled_the_drawer() {
        let mut running = start().await;
        let (mut kicker, _) = connect(&mut running).await;
        let (mut watcher, _) = connect(&mut running).await;
        for client in [&mut kicker, &mut watcher] {
            client.write_all(b"\x1da\x01").await.expect("write");
            assert_eq!(read_reply(client, 4).await, [0x10, 0, 0, 0]);
        }
        kicker
            .write_all(b"\x10\x14\x01\x00\x01")
            .await
            .expect("write");
        // Exactly one ASB each (the kicker's own session saw it first).
        assert_eq!(read_reply(&mut kicker, 4).await, [0x14, 0, 0, 0]);
        assert_eq!(read_reply(&mut watcher, 4).await, [0x14, 0, 0, 0]);
        assert_no_reply(&mut kicker).await;
        assert_no_reply(&mut watcher).await;
        assert!(running.state.snapshot().status.drawer_open);
    }

    #[tokio::test]
    async fn error_state_discards_data_and_printing_resumes_after_recovery() {
        let mut running = start().await;
        let (mut client, addr) = connect(&mut running).await;

        client.write_all(b"before\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");

        set_state(
            &running.state,
            PrinterStatus {
                paper_out: true,
                cover_open: true,
                ..PrinterStatus::default()
            },
        );
        // Text, a cut and a drawer kick: logged, suppressed, nothing else;
        // the status request in the same read is still answered.
        client
            .write_all(b"lost\n\x1dV\x00\x1bp\x00\x01\x01\x10\x04\x02")
            .await
            .expect("write");
        assert_eq!(read_reply(&mut client, 1).await, [0x36]);
        let events: Vec<_> = [
            next_event(&mut running.events).await,
            next_event(&mut running.events).await,
            next_event(&mut running.events).await,
        ]
        .into();
        assert_eq!(
            events.iter().map(kind).collect::<Vec<_>>(),
            ["commands", "suppressed", "sent"]
        );
        let ListenerEvent::PrintSuppressed(suppressed) = &events[1] else {
            panic!("expected print:suppressed");
        };
        assert_eq!(suppressed.peer, addr);
        assert_eq!(suppressed.reason, SuppressReason::CoverOpen);
        assert_eq!(suppressed.bytes, 5 + 3 + 5);
        assert!(!running.state.snapshot().status.drawer_open);

        // Recovery: the receipt started before the error goes on.
        set_state(&running.state, PrinterStatus::default());
        client.write_all(b"after\n\x1dV\x00").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        match next_event(&mut running.events).await {
            ListenerEvent::ReceiptCompleted(completed) => {
                assert_eq!(completed.receipt.elements.len(), 2, "before + after");
            }
            other => panic!("expected receipt:completed, got {other:?}"),
        }
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(tail.iter().map(kind).collect::<Vec<_>>(), ["connections"]);
    }

    #[tokio::test]
    async fn disconnect_in_an_error_state_still_completes_the_receipt() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;
        client.write_all(b"kept\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");
        set_state(
            &running.state,
            PrinterStatus {
                offline: true,
                ..PrinterStatus::default()
            },
        );
        client.write_all(b"dropped").await.expect("write");
        drop(client);
        let tail = events_until_disconnect(&mut running.events).await;
        assert_eq!(
            tail.iter().map(kind).collect::<Vec<_>>(),
            ["commands", "suppressed", "completed", "connections"]
        );
        let ListenerEvent::ReceiptCompleted(completed) = &tail[2] else {
            panic!("expected receipt:completed");
        };
        assert_eq!(completed.receipt.elements.len(), 1);
    }

    #[test]
    fn set_printer_status_merges_and_emits_only_changes() {
        let state = ListenerState::new(Settings::default(), None);
        let (ui, mut events) = unbounded_channel();
        let cover = state.set_printer_status(
            PrinterStatusPatch {
                cover_open: Some(true),
                ..PrinterStatusPatch::default()
            },
            &ui,
        );
        assert_eq!(
            cover,
            PrinterStatus {
                cover_open: true,
                ..PrinterStatus::default()
            }
        );
        let both = state.set_printer_status(
            serde_json::from_str(r#"{"paperOut":true}"#).expect("patch"),
            &ui,
        );
        assert!(both.cover_open && both.paper_out && !both.offline && !both.drawer_open);
        // No change: no event.
        state.set_printer_status(
            serde_json::from_str(r#"{"paperOut":true}"#).expect("patch"),
            &ui,
        );
        let emitted: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
        assert_eq!(
            emitted,
            vec![
                ListenerEvent::PrinterStatus(cover),
                ListenerEvent::PrinterStatus(both)
            ]
        );
        assert_eq!(state.snapshot().status, both);
        assert_eq!(
            json(&state.snapshot()),
            r#"{"listener":{"state":"stopped","address":"127.0.0.1","port":9100,"addresses":["127.0.0.1"]},"connections":{"active":0,"peers":[]},"settings":{"port":9100,"networkMode":"local","paper":"80mm","defaultCodePage":"cp437"},"status":{"paperOut":true,"coverOpen":true,"offline":false,"drawerOpen":false}}"#
        );
    }

    #[tokio::test]
    async fn a_client_that_never_reads_does_not_stall_others() {
        let mut running = start().await;
        // Small receive buffer so the replies back up quickly.
        let socket = tokio::net::TcpSocket::new_v4().expect("socket");
        socket.set_recv_buffer_size(4096).expect("buffer size");
        let mut flooder = socket
            .connect(SocketAddr::from((Ipv4Addr::LOCALHOST, running.port)))
            .await
            .expect("connect");
        let flooder_addr = flooder.local_addr().expect("address").to_string();
        assert_eq!(next_connections(&mut running.events).await.active, 1);
        let (mut printer, printer_addr) = connect(&mut running).await;

        // 4-byte replies to 4-byte requests, never read. The writer stops
        // when the printer stops reading; the error after the close ends it.
        let flood = tokio::spawn(async move {
            let chunk = b"\x10\x14\x07\x01".repeat(16 * 1024);
            for _ in 0..64 {
                if flooder.write_all(&chunk).await.is_err() {
                    break;
                }
            }
            flooder
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        printer.write_all(b"hello\n\x1dV\x00").await.expect("write");
        let mut printed = false;
        loop {
            match next_event(&mut running.events).await {
                ListenerEvent::ReceiptCompleted(completed) if completed.peer == printer_addr => {
                    printed = true;
                }
                ListenerEvent::Connections(snapshot) => {
                    // The flooder was disconnected; the printer is still there.
                    assert_eq!(snapshot.peers, vec![printer_addr.clone()]);
                    assert!(!snapshot.peers.contains(&flooder_addr));
                    break;
                }
                _ => {}
            }
        }
        assert!(
            printed,
            "the receipt was not printed while the flooder was stalled"
        );
        drop(timeout(TIMEOUT, flood).await.expect("flood writer ends"));
    }

    // ---- Stage 6: settings, restarts, raw bytes, export ----------------

    async fn update(running: &mut Running, patch: &str) -> Settings {
        let patch = serde_json::from_str(patch).expect("patch");
        running
            .state
            .update_settings(patch, running.sink.clone())
            .await
            .expect("valid settings")
    }

    async fn next_status(events: &mut UnboundedReceiver<ListenerEvent>) -> ListenerStatus {
        match next_of(events, "status").await {
            ListenerEvent::Status(status) => status,
            _ => unreachable!(),
        }
    }

    /// A port that is free right now (bound and released).
    async fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        listener.local_addr().expect("address").port()
    }

    #[tokio::test]
    async fn changing_the_port_moves_the_listener_and_closes_old_connections() {
        let mut running = start().await;
        let old_port = running.port;
        set_state(
            &running.state,
            PrinterStatus {
                drawer_open: true,
                ..PrinterStatus::default()
            },
        );
        let (mut client, addr) = connect(&mut running).await;
        client.write_all(b"in progress\n").await.expect("write");
        assert_eq!(kind(&next_event(&mut running.events).await), "commands");
        assert_eq!(kind(&next_event(&mut running.events).await), "updated");

        let new_port = free_port().await;
        let settings = update(&mut running, &format!(r#"{{"port":{new_port}}}"#)).await;
        assert_eq!(settings.port, new_port);

        // Settings first, then the old connection ends like any disconnect,
        // then the listener reports stopped and listening on the new port.
        let mut events = Vec::new();
        loop {
            let event = next_event(&mut running.events).await;
            let done = matches!(
                &event,
                ListenerEvent::Status(ListenerStatus {
                    state: ListenerPhase::Listening,
                    ..
                })
            );
            events.push(event);
            if done {
                break;
            }
        }
        assert_eq!(
            events.iter().map(kind).collect::<Vec<_>>(),
            ["settings", "completed", "connections", "status", "status"]
        );
        let ListenerEvent::ReceiptCompleted(completed) = &events[1] else {
            unreachable!()
        };
        assert_eq!(completed.peer, addr);
        assert_eq!(completed.receipt.cut, None);
        let ListenerEvent::Status(stopped) = &events[3] else {
            unreachable!()
        };
        assert_eq!(stopped.state, ListenerPhase::Stopped);
        let ListenerEvent::Status(listening) = &events[4] else {
            unreachable!()
        };
        assert_eq!(listening.port, new_port);
        assert_eq!(running.state.snapshot().listener, *listening);

        // The old client sees an orderly close.
        let mut buf = [0_u8; 8];
        let read = timeout(TIMEOUT, client.read(&mut buf))
            .await
            .expect("closed in time");
        assert!(matches!(read, Ok(0)), "{read:?}");
        // The old port refuses, the new one accepts.
        assert!(TcpStream::connect(("127.0.0.1", old_port)).await.is_err());
        running.port = new_port;
        let (mut client, _) = connect(&mut running).await;
        // Nothing else was reset: the printer state and the receipt ids go on.
        assert!(running.state.snapshot().status.drawer_open);
        client.write_all(b"next\n\x1dV\x00").await.expect("write");
        match next_of(&mut running.events, "completed").await {
            ListenerEvent::ReceiptCompleted(next) => assert!(next.id > completed.id),
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn network_mode_binds_every_interface() {
        let mut running = start().await;
        update(&mut running, r#"{"networkMode":"network"}"#).await;
        assert_eq!(kind(&next_event(&mut running.events).await), "settings");
        assert_eq!(
            next_status(&mut running.events).await.state,
            ListenerPhase::Stopped
        );
        let status = next_status(&mut running.events).await;
        assert_eq!(status.state, ListenerPhase::Listening);
        assert_eq!(status.address, "0.0.0.0");
        // Non-loopback, non-link-local IPv4 addresses (maybe none), sorted.
        for address in &status.addresses {
            let ip: Ipv4Addr = address.parse().expect("IPv4");
            assert!(!ip.is_loopback() && !ip.is_link_local(), "{ip}");
        }
        let parsed: Vec<Ipv4Addr> = status
            .addresses
            .iter()
            .map(|a| a.parse().expect("IPv4"))
            .collect();
        assert!(parsed.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(
            running.state.snapshot().listener.addresses,
            status.addresses
        );
        // Reachable through loopback too.
        running.port = status.port;
        connect(&mut running).await;

        update(&mut running, r#"{"networkMode":"local"}"#).await;
        let status = loop {
            let status = next_status(&mut running.events).await;
            if status.state == ListenerPhase::Listening {
                break status;
            }
        };
        assert_eq!(status.address, "127.0.0.1");
        assert_eq!(status.addresses, ["127.0.0.1"]);
    }

    #[tokio::test]
    async fn port_in_use_suggests_a_free_port_and_retry_succeeds() {
        // An occupied port with room above it for the suggestion.
        let mut occupied = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        for _ in 0..20 {
            if occupied.local_addr().expect("address").port() < 65000 {
                break;
            }
            occupied = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        }
        let port = occupied.local_addr().expect("address").port();

        let state = Arc::new(ListenerState::new(localhost(port), None));
        let (sender, mut events) = unbounded_channel();
        let sink = Arc::new(sender);
        state.start(sink.clone()).await;
        let status = next_status(&mut events).await;
        assert_eq!(status.state, ListenerPhase::Error);
        assert_eq!(status.code, Some(BindErrorCode::PortInUse));
        assert_eq!(status.port, port);
        let suggested = status.suggested_port.expect("a free port nearby");
        assert!(suggested > port && suggested <= port + SUGGEST_PORT_RANGE);
        // It really is free (the app never switches to it on its own).
        drop(
            bind(IpAddr::V4(Ipv4Addr::LOCALHOST), suggested)
                .await
                .expect("suggested port is free"),
        );
        assert_eq!(state.settings().port, port);

        // "Try again" while still taken: the same error, nothing stopped.
        state.start(sink.clone()).await;
        assert_eq!(next_status(&mut events).await.state, ListenerPhase::Error);

        drop(occupied);
        state.start(sink.clone()).await;
        let status = next_status(&mut events).await;
        assert_eq!(status.state, ListenerPhase::Listening);
        assert_eq!(status.port, port);
        assert!(status.code.is_none() && status.suggested_port.is_none());
        TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("listening after retry");
        state.stop(sink.as_ref()).await;
    }

    #[tokio::test]
    async fn default_code_page_applies_to_the_next_connection() {
        let mut running = start().await;
        let (mut before, _) = connect(&mut running).await;
        update(&mut running, r#"{"defaultCodePage":"windows1252"}"#).await;
        assert!(matches!(
            next_event(&mut running.events).await,
            ListenerEvent::SettingsChanged(Settings {
                default_code_page: CodePage::Windows1252,
                ..
            })
        ));
        let (mut after, _) = connect(&mut running).await;
        for (client, expected) in [(&mut before, "\u{255F}"), (&mut after, "\u{C7}")] {
            client.write_all(b"\xC7\n").await.expect("write");
            let commands = next_commands(&mut running.events).await;
            assert_eq!(
                commands.commands[0].command,
                Command::Text {
                    text: expected.into()
                }
            );
        }
    }

    #[tokio::test]
    async fn update_settings_validates_and_persists() {
        let dir = crate::settings::tests::TempDir::new("listener-update");
        let store = SettingsStore::new(&dir.0);
        let state = Arc::new(ListenerState::new(Settings::default(), Some(store.clone())));
        let (sender, mut events) = unbounded_channel();
        let sink: Arc<dyn EventSink> = Arc::new(sender);

        let invalid = serde_json::from_str(r#"{"port":0}"#).expect("patch");
        assert_eq!(
            state.update_settings(invalid, sink.clone()).await,
            Err(SettingsError::InvalidPort)
        );
        assert!(!store.path().exists());

        let paper = serde_json::from_str(r#"{"paper":"58mm"}"#).expect("patch");
        let settings = state
            .update_settings(paper, sink.clone())
            .await
            .expect("valid");
        assert_eq!(settings.paper, PaperProfile::Mm58);
        assert_eq!(store.load(), settings);
        // Paper only: no restart (no listener:status), one settings:changed.
        assert_eq!(
            events.try_recv(),
            Ok(ListenerEvent::SettingsChanged(settings))
        );
        assert!(events.try_recv().is_err());
        // No change: nothing emitted.
        let same = serde_json::from_str(r#"{"paper":"58mm"}"#).expect("patch");
        state.update_settings(same, sink).await.expect("valid");
        assert!(events.try_recv().is_err());
    }

    /// Ids and receipts of the `receipt:completed` events until the
    /// connection closes.
    fn completed_receipts(events: &[ListenerEvent]) -> Vec<ReceiptCompleted> {
        events
            .iter()
            .filter_map(|event| match event {
                ListenerEvent::ReceiptCompleted(completed) => Some(completed.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn raw_bytes_of_each_receipt_are_the_exact_slice_sent() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;

        let first: &[u8] = b"\x1b@a\n\x10\x04\x01\x1dV\x00";
        let second: &[u8] = b"\x1dV\x00b\n\x1bi";
        // A raster image split across many reads, then text; closed by the
        // disconnect.
        let mut third = b"\x1dv0\x00\x48\x00\x20\x00".to_vec();
        third.extend((0..72 * 32).map(|i| (i % 256) as u8));
        third.extend_from_slice(b"c\n");

        let mut all = first.to_vec();
        all.extend_from_slice(second);
        all.extend_from_slice(&third);
        for piece in all.chunks(50) {
            client.write_all(piece).await.expect("write");
            client.flush().await.expect("flush");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        drop(client);
        let events = events_until_disconnect(&mut running.events).await;
        let completed = completed_receipts(&events);
        assert_eq!(completed.len(), 3);
        for (receipt, expected) in completed.iter().zip([first, second, &third[..]]) {
            assert_eq!(
                &*running.state.receipt_bytes(receipt.id).expect("retained"),
                expected
            );
        }
        assert_eq!(completed[2].receipt.cut, None);
    }

    #[tokio::test]
    async fn content_after_the_last_cut_without_print_is_not_retained() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;
        client
            .write_all(b"a\n\x1dV\x00\x10\x04\x01\x1b@")
            .await
            .expect("write");
        drop(client);
        let events = events_until_disconnect(&mut running.events).await;
        let completed = completed_receipts(&events);
        assert_eq!(completed.len(), 1);
        assert_eq!(
            &*running.state.receipt_bytes(completed[0].id).expect("kept"),
            b"a\n\x1dV\x00"
        );
        assert_eq!(
            running.state.receipt_bytes(completed[0].id + 1),
            Err(RetainedError::Unknown(completed[0].id + 1))
        );
    }

    #[test]
    fn retained_bytes_are_evicted_by_count_and_by_size() {
        let mut by_count = Retained::new(MAX_RETAINED_RECEIPTS, MAX_RETAINED_BYTES);
        for id in 1..=205 {
            by_count.insert(id, vec![0; 10]);
        }
        assert_eq!(by_count.entries.len(), MAX_RETAINED_RECEIPTS);
        assert!(by_count.get(5).is_none());
        assert!(by_count.get(6).is_some() && by_count.get(205).is_some());
        assert_eq!(by_count.total, 2000);

        let mut by_size = Retained::new(200, 10);
        by_size.insert(1, vec![1; 4]);
        by_size.insert(2, vec![2; 4]);
        by_size.insert(3, vec![3; 4]);
        assert!(by_size.get(1).is_none());
        assert_eq!(by_size.get(3).as_deref(), Some(&[3_u8; 4][..]));
        assert_eq!(by_size.total, 8);
        // Larger than the whole cap: not kept, nothing else evicted.
        by_size.insert(4, vec![4; 11]);
        assert!(by_size.get(4).is_none());
        assert_eq!(by_size.entries.len(), 2);
        by_size.clear();
        assert!(by_size.get(3).is_none());
        assert_eq!(by_size.total, 0);
    }

    #[test]
    fn a_receipt_too_large_to_keep_is_not_retained() {
        let mut raw = RawBuffer::default();
        raw.push(b"small\n");
        assert_eq!(raw.take(6).as_deref(), Some(&b"small\n"[..]));
        let chunk = vec![0_u8; READ_BUFFER_SIZE];
        let mut sent = 6_u64;
        while sent < 6 + MAX_RETAINED_BYTES as u64 + 10_000 {
            raw.push(&chunk);
            sent += chunk.len() as u64;
        }
        assert!(raw.bytes.len() < MAX_RETAINED_BYTES);
        // The huge receipt is lost; the next one starts in bytes that were
        // kept after the overflow, so it is complete.
        assert_eq!(raw.take(sent - 100), None);
        raw.push(b"x");
        let mut next = vec![0_u8; 100];
        next.push(b'x');
        assert_eq!(raw.take(sent + 1), Some(next));
        raw.push(b"next\n");
        assert_eq!(raw.take(sent + 6).as_deref(), Some(&b"next\n"[..]));
    }

    #[tokio::test]
    async fn export_writes_the_bin_and_png_and_the_bin_replays() {
        let mut running = start().await;
        let (mut client, _) = connect(&mut running).await;
        let bytes: &[u8] = b"\x1b@\x1ba\x01\x1bE\x01Total\x1bE\x00 9,90\n\x1dw\x02\x1dh\x20\x1dk\x02789123456789\x00\x1dVB\x05";
        client.write_all(bytes).await.expect("write");
        let completed = match next_of(&mut running.events, "completed").await {
            ListenerEvent::ReceiptCompleted(completed) => completed,
            _ => unreachable!(),
        };
        let dir = crate::settings::tests::TempDir::new("export");
        std::fs::create_dir_all(&dir.0).expect("temp dir");
        let state = running.state.as_ref();

        // .bin: the extension is added; replaying it reproduces the receipt.
        let result = export::export_to(
            state,
            completed.id,
            export::ExportFormat::Bin,
            None,
            dir.0.join("receipt"),
        )
        .expect("exported");
        let path = dir.0.join("receipt.bin");
        assert_eq!(result, export::ExportResult::saved(&path));
        let saved = std::fs::read(&path).expect("written");
        assert_eq!(saved, bytes);
        let mut session = Session::new(
            ParserConfig::default(),
            ReceiptConfig::default(),
            PrinterStatus::default(),
        );
        let mut out = session.feed(&saved, PrinterStatus::default()).events;
        out.extend(session.finish(PrinterStatus::default()).events);
        let replayed: Vec<Receipt> = out
            .into_iter()
            .filter_map(|event| match event {
                SessionEvent::ReceiptCompleted { receipt, .. } => Some(receipt),
                _ => None,
            })
            .collect();
        assert_eq!(replayed, vec![completed.receipt.clone()]);

        // .png: written as received.
        let png = b"\x89PNG\r\n\x1a\nnot really an image".to_vec();
        export::export_to(
            state,
            completed.id,
            export::ExportFormat::Png,
            Some(png.clone()),
            dir.0.join("receipt.png"),
        )
        .expect("exported");
        assert_eq!(
            std::fs::read(dir.0.join("receipt.png")).expect("written"),
            png
        );

        let fail = |id, format, png: Option<Vec<u8>>| {
            export::export_to(state, id, format, png, dir.0.join("x"))
                .expect_err("rejected")
                .to_string()
        };
        assert!(fail(completed.id, export::ExportFormat::Png, None).contains("png"));
        assert!(
            fail(
                completed.id,
                export::ExportFormat::Png,
                Some(b"GIF89a".to_vec())
            )
            .contains("not a PNG")
        );
        assert!(
            fail(
                completed.id,
                export::ExportFormat::Png,
                Some(vec![0; export::MAX_PNG_BYTES + 1])
            )
            .contains("32 MiB")
        );
        assert!(fail(999, export::ExportFormat::Bin, None).contains("no receipt 999"));
        assert!(fail(0, export::ExportFormat::Png, Some(png.clone())).contains("no receipt 0"));
        // A missing directory is a write error, reported with the path.
        let write_error = export::export_to(
            state,
            completed.id,
            export::ExportFormat::Bin,
            None,
            dir.0.join("missing").join("r.bin"),
        )
        .expect_err("no such directory");
        assert!(write_error.to_string().contains("Could not write"));

        // "Clear roll": the bytes are gone, the PNG export still works.
        state.clear_receipts();
        assert!(
            fail(completed.id, export::ExportFormat::Bin, None).contains("no longer available")
        );
        export::export_to(
            state,
            completed.id,
            export::ExportFormat::Png,
            Some(png),
            dir.0.join("again.png"),
        )
        .expect("PNG needs no raw bytes");
    }
}
