fn main() {
    // Only the commands listed here are exposed to the frontend, and each one
    // must be granted explicitly in a capability file (see `capabilities/`).
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "get_state",
            "update_settings",
            "retry_listener",
            "set_printer_status",
            "clear_receipts",
            "export_receipt",
            "open_link",
        ]));

    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}
