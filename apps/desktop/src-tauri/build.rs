//! Declares the app's commands, so each capability file can say which pages may
//! call which: the server's pages only get the four in `capabilities/server.json`.
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "saved_server",
            "discovered_servers",
            "connect",
            "reconnect",
            "switch_server",
            "app_info",
            "check_app_update",
            "install_app_update",
        ]),
    ))
    .expect("tauri-build");
}
