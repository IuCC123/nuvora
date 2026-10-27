//! Nuvora for Mac and Windows. The window shows the server's own web app in
//! the system's web view (WebKit on macOS, WebView2 on Windows), so it is the
//! same app as in a browser, from the same server, always the server's version.
//! This side adds what a browser can't: finding servers on the network,
//! remembering one, and updating itself.
//!
//! The window opens on `connect.html` (built with the web app), which
//! reconnects to the remembered server or lets someone pick one, then
//! navigates to it. Links to anywhere else open in the default browser.

mod discovery;
mod server;

use std::sync::Mutex;

use discovery::{Discovered, FoundServer};
use serde::Serialize;
use server::{Current, SavedServer};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_plugin_window_state::StateFlags;

const MAIN: &str = "main";

/// Where the window goes to pick a server: the app's own `connect.html`.
struct ConnectPage(Url);

/// An update found by `check_app_update`, kept for `install_app_update`.
#[derive(Default)]
struct PendingUpdate(Mutex<Option<Update>>);

pub fn run() {
    tauri::Builder::default()
        // First, so a second launch only brings the open window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window(MAIN) {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // Quitting during a full-screen video shouldn't reopen full screen.
                .with_state_flags(StateFlags::all() & !StateFlags::FULLSCREEN)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Current::default())
        .manage(Discovered::default())
        .manage(PendingUpdate::default())
        .invoke_handler(tauri::generate_handler![
            saved_server,
            discovered_servers,
            connect,
            reconnect,
            switch_server,
            app_info,
            check_app_update,
            install_app_update,
        ])
        .setup(|app| {
            app.state::<Discovered>().start();
            let window = open_window(app.handle())?;
            app.manage(ConnectPage(window.url()?));
            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Nuvora failed to start");
}

fn open_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let navigating = app.clone();
    let opening = app.clone();
    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("connect.html".into()))
        .title("Nuvora")
        .inner_size(1280.0, 800.0)
        .min_inner_size(800.0, 560.0)
        // The web app's canvas, so nothing flashes white before the first paint.
        .background_color(tauri::window::Color(15, 16, 18, 255))
        .theme(Some(tauri::Theme::Dark))
        // The web app handles its own drag and drop.
        .disable_drag_drop_handler()
        .on_navigation(move |url| {
            if may_show(&navigating, url) {
                return true;
            }
            open_in_browser(&navigating, url);
            false
        })
        .on_new_window(move |url, _| {
            open_in_browser(&opening, &url);
            NewWindowResponse::Deny
        })
        .on_document_title_changed(|window, title| {
            let _ = window.set_title(&title);
        });
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Transparent);
    builder.build()
}

/// The app's own pages, and the server it is connected to.
fn may_show(app: &AppHandle, url: &Url) -> bool {
    if matches!(url.scheme(), "about" | "blob" | "data") {
        return true;
    }
    let own = app.try_state::<ConnectPage>().map(|page| {
        let page = &page.0;
        url.scheme() == page.scheme()
            && url.host_str() == page.host_str()
            && url.port() == page.port()
    });
    // Before `ConnectPage` is known, only the first load of connect.html happens.
    own.unwrap_or(true) || app.state::<Current>().allows(url)
}

fn open_in_browser(app: &AppHandle, url: &Url) {
    if matches!(url.scheme(), "http" | "https" | "mailto") {
        let _ = app.opener().open_url(url.as_str(), None::<&str>);
    }
}

#[tauri::command]
fn saved_server(app: AppHandle) -> Option<SavedServer> {
    server::load(&app)
}

#[tauri::command]
fn discovered_servers(discovered: State<'_, Discovered>) -> Vec<FoundServer> {
    discovered.list()
}

/// Finds the server at what someone typed (or picked), remembers it and lets
/// the window show it. The page then navigates there.
#[tauri::command]
async fn connect(app: AppHandle, address: String) -> Result<SavedServer, String> {
    let server = server::find(&address).await?;
    use_server(&app, &server);
    Ok(server)
}

/// The remembered server, at its address or, when it moved, wherever the
/// network says the same server is now.
#[tauri::command]
async fn reconnect(app: AppHandle) -> Result<SavedServer, String> {
    let saved = server::load(&app).ok_or("No server yet.")?;
    let mut found = server::probe(&saved.url).await.ok();
    if found.is_none() {
        let moved = saved
            .id
            .as_deref()
            .and_then(|id| app.state::<Discovered>().by_id(id));
        if let Some(moved) = moved {
            found = server::probe(&moved.url).await.ok();
        }
    }
    let server = found.ok_or_else(|| {
        format!(
            "Couldn't reach {} at {}. Check that it's running, then try again.",
            saved.name, saved.url
        )
    })?;
    use_server(&app, &server);
    Ok(server)
}

fn use_server(app: &AppHandle, server: &SavedServer) {
    server::save(app, server);
    app.state::<Current>().set(server);
}

/// Back to the connect page, to pick another server. Called by the web app's
/// account menu and Sign in page, and by the macOS menu.
#[tauri::command]
fn switch_server(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window(MAIN).ok_or("No window.")?;
    let mut page = app.state::<ConnectPage>().0.clone();
    page.set_query(Some("pick"));
    window.navigate(page).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct AppInfo {
    /// "Nuvora for Mac", "Nuvora for Windows".
    name: &'static str,
    version: String,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    let name = if cfg!(target_os = "macos") {
        "Nuvora for Mac"
    } else if cfg!(windows) {
        "Nuvora for Windows"
    } else {
        "Nuvora for Linux"
    };
    AppInfo {
        name,
        version: app.package_info().version.to_string(),
    }
}

#[derive(Serialize)]
struct AppUpdate {
    version: String,
    notes: Option<String>,
}

/// A newer app on GitHub releases, if there is one.
#[tauri::command]
async fn check_app_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<AppUpdate>, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let found = update.as_ref().map(|u| AppUpdate {
        version: u.version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap() = update;
    Ok(found)
}

/// Downloads and installs the update `check_app_update` found, then restarts.
#[tauri::command]
async fn install_app_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take();
    let update = match update {
        Some(update) => update,
        None => app
            .updater()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("This app is up to date.")?,
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

#[cfg(target_os = "macos")]
mod menu {
    //! The standard macOS menus, plus Switch Server, Reload, Back and Forward:
    //! what a browser window offers, without the address bar.

    use tauri::menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem};
    use tauri::{AppHandle, Manager};

    pub fn install(app: &AppHandle) -> tauri::Result<()> {
        let menu = Menu::default(app)?;
        for item in menu.items()? {
            let MenuItemKind::Submenu(submenu) = item else {
                continue;
            };
            match submenu.text()?.as_str() {
                "Nuvora" => {
                    let switch = MenuItem::with_id(
                        app,
                        "switch-server",
                        "Switch Server…",
                        true,
                        None::<&str>,
                    )?;
                    submenu.insert(&PredefinedMenuItem::separator(app)?, 1)?;
                    submenu.insert(&switch, 2)?;
                }
                "View" => {
                    let reload =
                        MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;
                    let back = MenuItem::with_id(app, "back", "Back", true, Some("CmdOrCtrl+["))?;
                    let forward =
                        MenuItem::with_id(app, "forward", "Forward", true, Some("CmdOrCtrl+]"))?;
                    submenu.insert_items(
                        &[
                            &reload,
                            &PredefinedMenuItem::separator(app)?,
                            &back,
                            &forward,
                            &PredefinedMenuItem::separator(app)?,
                        ],
                        0,
                    )?;
                }
                _ => {}
            }
        }
        app.set_menu(menu)?;
        app.on_menu_event(|app, event| {
            let Some(window) = app.get_webview_window(super::MAIN) else {
                return;
            };
            let _ = match event.id().as_ref() {
                "switch-server" => super::switch_server(app.clone())
                    .map_err(|_| tauri::Error::FailedToReceiveMessage),
                "reload" => window.eval("location.reload()"),
                "back" => window.eval("history.back()"),
                "forward" => window.eval("history.forward()"),
                _ => Ok(()),
            };
        });
        Ok(())
    }
}
