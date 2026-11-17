//! PhantomPOS desktop app: Tauri glue around the TCP listener.

mod listener;

use std::sync::Arc;

use phantompos_core::{CoreError, PaperProfile};
use serde::{Serialize, Serializer};
use tauri::{AppHandle, Emitter, Manager, State};

use listener::{EventSink, ListenerEvent, ListenerSettings, ListenerState, StateSnapshot};

/// Event names shared with the frontend.
mod events {
    pub const LISTENER_STATUS: &str = "listener:status";
    pub const CONNECTION_CHANGED: &str = "connection:changed";
    pub const COMMANDS_RECEIVED: &str = "commands:received";
    pub const RECEIPT_UPDATED: &str = "receipt:updated";
    pub const RECEIPT_COMPLETED: &str = "receipt:completed";
    pub const DRAWER_KICKED: &str = "drawer:kicked";
}

/// Error returned by commands to the UI. Serialized as its message (the
/// promise returned by `invoke` rejects with that string).
#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl Serialize for CommandError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// Forwards listener events to the frontend as global Tauri events.
struct AppEventSink(AppHandle);

impl EventSink for AppEventSink {
    fn emit(&self, event: ListenerEvent) {
        let result = match &event {
            ListenerEvent::Status(status) => self.0.emit(events::LISTENER_STATUS, status),
            ListenerEvent::Connections(connections) => {
                self.0.emit(events::CONNECTION_CHANGED, connections)
            }
            ListenerEvent::Commands(commands) => self.0.emit(events::COMMANDS_RECEIVED, commands),
            ListenerEvent::ReceiptUpdated(updated) => self.0.emit(events::RECEIPT_UPDATED, updated),
            ListenerEvent::ReceiptCompleted(completed) => {
                self.0.emit(events::RECEIPT_COMPLETED, completed)
            }
            ListenerEvent::DrawerKicked(kicked) => self.0.emit(events::DRAWER_KICKED, kicked),
        };
        if let Err(error) = result {
            eprintln!("phantompos: failed to emit event: {error}");
        }
    }
}

/// Current listener status, connections, paper and drawer state, so the UI
/// can sync on mount regardless of which events it missed.
#[tauri::command]
fn get_state(state: State<'_, Arc<ListenerState>>) -> StateSnapshot {
    state.snapshot()
}

/// Selects the paper (`"58mm"` or `"80mm"`) for receipts started from now on
/// and returns it. Kept in memory only.
#[tauri::command]
fn set_paper(
    state: State<'_, Arc<ListenerState>>,
    paper: String,
) -> Result<PaperProfile, CommandError> {
    let paper: PaperProfile = paper.parse()?;
    state.set_paper(paper);
    Ok(paper)
}

/// Marks the cash drawer as closed.
#[tauri::command]
fn close_drawer(state: State<'_, Arc<ListenerState>>) {
    state.close_drawer();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let state = Arc::new(ListenerState::new(ListenerSettings::default()));
            app.manage(state.clone());
            let sink = Arc::new(AppEventSink(app.handle().clone()));
            tauri::async_runtime::spawn(listener::run(state, sink));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_state, set_paper, close_drawer])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("phantompos: error while running the application: {error}");
        std::process::exit(1);
    }
}
