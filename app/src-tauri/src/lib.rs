//! PhantomPOS desktop app: Tauri glue around the TCP listener.

mod export;
mod links;
mod listener;
mod settings;

use std::sync::Arc;

use phantompos_core::{PrinterStatus, PrinterStatusPatch};
use serde::{Serialize, Serializer};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use export::{ExportError, ExportFormat, ExportResult};
use links::Link;
use listener::{EventSink, ListenerEvent, ListenerState, StateSnapshot};
use settings::{Settings, SettingsError, SettingsPatch, SettingsStore};

/// Event names shared with the frontend.
mod events {
    pub const LISTENER_STATUS: &str = "listener:status";
    pub const CONNECTION_CHANGED: &str = "connection:changed";
    pub const COMMANDS_RECEIVED: &str = "commands:received";
    pub const RECEIPT_UPDATED: &str = "receipt:updated";
    pub const RECEIPT_COMPLETED: &str = "receipt:completed";
    pub const DRAWER_KICKED: &str = "drawer:kicked";
    pub const PRINTER_STATUS: &str = "printer:status";
    pub const STATUS_SENT: &str = "status:sent";
    pub const PRINT_SUPPRESSED: &str = "print:suppressed";
    pub const SETTINGS_CHANGED: &str = "settings:changed";
}

/// Error returned by commands to the UI. Serialized as its message (the
/// promise returned by `invoke` rejects with that string).
#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("The export was interrupted: {0}")]
    Interrupted(String),
    #[error("Couldn't open {url}: {source}")]
    OpenLink {
        url: &'static str,
        source: tauri_plugin_opener::Error,
    },
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
            ListenerEvent::PrinterStatus(status) => self.0.emit(events::PRINTER_STATUS, status),
            ListenerEvent::StatusSent(sent) => self.0.emit(events::STATUS_SENT, sent),
            ListenerEvent::PrintSuppressed(suppressed) => {
                self.0.emit(events::PRINT_SUPPRESSED, suppressed)
            }
            ListenerEvent::SettingsChanged(settings) => {
                self.0.emit(events::SETTINGS_CHANGED, settings)
            }
        };
        if let Err(error) = result {
            eprintln!("phantompos: failed to emit event: {error}");
        }
    }
}

fn sink(app: AppHandle) -> Arc<dyn EventSink> {
    Arc::new(AppEventSink(app))
}

/// Current listener status (network addresses read again), connections,
/// settings and simulated printer state, so the UI can sync on mount
/// regardless of which events it missed.
#[tauri::command]
fn get_state(state: State<'_, Arc<ListenerState>>) -> StateSnapshot {
    state.snapshot()
}

/// Merges `settings` (a partial `Settings`; unknown fields or invalid values
/// reject) into the settings, saves them, emits `settings:changed`, and
/// restarts the listener if the port or the network mode changed. Returns
/// the whole new settings.
#[tauri::command]
async fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<ListenerState>>,
    settings: SettingsPatch,
) -> Result<Settings, CommandError> {
    Ok(state.update_settings(settings, sink(app)).await?)
}

/// Binds again with the current settings (the "try again" action); the
/// result arrives as `listener:status`.
#[tauri::command]
async fn retry_listener(
    app: AppHandle,
    state: State<'_, Arc<ListenerState>>,
) -> Result<(), CommandError> {
    state.start(sink(app)).await;
    Ok(())
}

/// Changes the simulated printer state. `status` is a partial object: any of
/// `paperOut`, `coverOpen`, `offline` and `drawerOpen` (booleans) replaces
/// the current value; an unknown field rejects the call. Returns the whole
/// new state; a change is also emitted as `printer:status`.
#[tauri::command]
fn set_printer_status(
    app: AppHandle,
    state: State<'_, Arc<ListenerState>>,
    status: PrinterStatusPatch,
) -> PrinterStatus {
    state.set_printer_status(status, &AppEventSink(app))
}

/// Forgets the raw bytes of every completed receipt ("Clear roll").
#[tauri::command]
fn clear_receipts(state: State<'_, Arc<ListenerState>>) {
    state.clear_receipts();
}

/// Saves completed receipt `id` as PNG (`png`: the file bytes rendered by
/// the UI) or as its raw bytes (`format: "bin"`), asking for the location
/// with the native save dialog.
///
/// The dialog is opened with the plugin's callback API: it runs on the main
/// thread and calls back from another thread, so neither the async runtime
/// nor the listener waits while it is open. The file is written on a
/// blocking thread.
#[tauri::command]
async fn export_receipt(
    app: AppHandle,
    state: State<'_, Arc<ListenerState>>,
    id: u64,
    format: ExportFormat,
    png: Option<Vec<u8>>,
) -> Result<ExportResult, CommandError> {
    let contents = export::prepare(&state, id, format, png)?;

    let (chosen_tx, chosen_rx) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_file_name(export::suggested_file_name(id, format))
        .add_filter(format.filter_name(), &[format.extension()]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog.save_file(move |path| {
        // The command may have been dropped; nothing to report then.
        let _ = chosen_tx.send(path);
    });
    let chosen = chosen_rx
        .await
        .map_err(|_| CommandError::Interrupted("the save dialog was closed".into()))?;
    let Some(chosen) = chosen else {
        return Ok(ExportResult::cancelled());
    };
    let path = chosen.into_path().map_err(|_| ExportError::InvalidPath)?;
    let path = export::with_extension(path, format);

    let written = path.clone();
    tokio::task::spawn_blocking(move || export::write(&written, &contents))
        .await
        .map_err(|error| CommandError::Interrupted(error.to_string()))??;
    Ok(ExportResult::saved(&path))
}

/// Opens one of the About links (`website`, `repository`, `issues`,
/// `license`) in the system browser. The webview passes an id, never a URL;
/// an unknown id rejects before this runs.
#[tauri::command]
fn open_link(app: AppHandle, link: Link) -> Result<(), CommandError> {
    let url = link.url();
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|source| CommandError::OpenLink { url, source })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        // Used from Rust only (`export_receipt`); the webview is granted no
        // dialog permission.
        .plugin(tauri_plugin_dialog::init())
        // Used from Rust only (`open_link`, fixed URLs); the webview is
        // granted no opener permission, so the script that routes `<a
        // target="_blank">` clicks through the plugin is not injected.
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .setup(|app| {
            let store = match app.path().app_config_dir() {
                Ok(dir) => Some(SettingsStore::new(dir)),
                Err(error) => {
                    eprintln!(
                        "phantompos: no config directory ({error}); settings will not be saved"
                    );
                    None
                }
            };
            let settings = store.as_ref().map(SettingsStore::load).unwrap_or_default();
            let state = Arc::new(ListenerState::new(settings, store));
            app.manage(state.clone());
            let sink = sink(app.handle().clone());
            tauri::async_runtime::spawn(async move { state.start(sink).await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            update_settings,
            retry_listener,
            set_printer_status,
            clear_receipts,
            export_receipt,
            open_link
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("phantompos: error while running the application: {error}");
        std::process::exit(1);
    }
}
