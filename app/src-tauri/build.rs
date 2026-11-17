fn main() {
    // Only the commands listed here are exposed to the frontend, and each one
    // must be granted explicitly in a capability file (see `capabilities/`).
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(&["get_state"]));

    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}
