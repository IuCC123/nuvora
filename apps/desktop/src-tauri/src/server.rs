//! The server this app shows: what someone typed, turned into an address that
//! answers like a Nuvora server, and remembered for the next launch.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};

/// Nuvora's port, tried first when an address has none.
const PORT: u16 = 8640;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct SavedServer {
    /// Origin only, no trailing slash: `http://192.168.1.20:8640`.
    pub url: String,
    pub name: String,
    /// Survives address changes, so a moved server is found again on the network.
    pub id: Option<String>,
}

/// The origin the window may show besides the app's own pages. Everything
/// else opens in the browser.
#[derive(Default)]
pub struct Current(Mutex<Option<String>>);

impl Current {
    pub fn set(&self, server: &SavedServer) {
        *self.0.lock().unwrap() = Some(server.url.clone());
    }

    pub fn allows(&self, url: &Url) -> bool {
        let origin = url.origin().ascii_serialization();
        self.0.lock().unwrap().as_deref() == Some(origin.as_str())
    }
}

/// The addresses to try for what someone typed, as the TV app does: people
/// type `192.168.1.20`, `192.168.1.20:8640`, `nuvora.example.com` or a full
/// URL; a bare host is tried on Nuvora's port first, then as a website.
pub fn candidates(typed: &str) -> Vec<String> {
    let input: String = typed.trim().trim_end_matches('/').replace(' ', "");
    if input.is_empty() {
        return Vec::new();
    }
    let lower = input.to_ascii_lowercase();
    let urls = if lower.starts_with("http://") || lower.starts_with("https://") {
        vec![input.clone()]
    } else {
        let host = input.split('/').next().unwrap_or_default();
        let has_port = host
            .rsplit_once(':')
            .is_some_and(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
        if has_port {
            vec![format!("http://{input}"), format!("https://{input}")]
        } else {
            vec![
                format!("http://{host}:{PORT}"),
                format!("https://{input}"),
                format!("http://{input}"),
            ]
        }
    };
    let mut origins: Vec<String> = Vec::new();
    for url in urls {
        let origin = Url::parse(&url)
            .ok()
            .filter(|u| u.has_host())
            .map(|u| u.origin().ascii_serialization());
        if let Some(origin) = origin
            && !origins.contains(&origin)
        {
            origins.push(origin);
        }
    }
    origins
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeError {
    Unreachable,
    NotNuvora,
    SetupRequired,
}

#[derive(Deserialize)]
struct Health {
    status: String,
}

#[derive(Deserialize)]
struct AuthState {
    setup_required: bool,
    server_name: String,
    server_id: String,
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .expect("HTTP client builds")
}

/// Asks `origin` whether it is a Nuvora server that people can sign in to.
pub async fn probe(origin: &str) -> Result<SavedServer, ProbeError> {
    let client = http();
    let health: Health = get(&client, &format!("{origin}/api/health")).await?;
    if health.status != "ok" {
        return Err(ProbeError::NotNuvora);
    }
    let state: AuthState = get(&client, &format!("{origin}/api/auth/state")).await?;
    if state.setup_required {
        return Err(ProbeError::SetupRequired);
    }
    Ok(SavedServer {
        url: origin.to_owned(),
        name: state.server_name,
        id: Some(state.server_id).filter(|id| !id.is_empty()),
    })
}

async fn get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, ProbeError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| ProbeError::Unreachable)?;
    if !response.status().is_success() {
        return Err(ProbeError::NotNuvora);
    }
    response.json().await.map_err(|_| ProbeError::NotNuvora)
}

/// The first of [`candidates`] that answers like Nuvora, or a sentence saying
/// what went wrong.
pub async fn find(typed: &str) -> Result<SavedServer, String> {
    let shown = typed.trim();
    let candidates = candidates(typed);
    if candidates.is_empty() {
        return Err("Type your server's address, like 192.168.1.20.".into());
    }
    let mut worst = ProbeError::Unreachable;
    for origin in candidates {
        match probe(&origin).await {
            Ok(server) => return Ok(server),
            Err(ProbeError::SetupRequired) => {
                return Err(format!(
                    "This server isn't set up yet. Open {origin} in a browser to create the first account."
                ));
            }
            Err(e @ ProbeError::NotNuvora) => worst = e,
            Err(ProbeError::Unreachable) => {}
        }
    }
    Err(match worst {
        ProbeError::NotNuvora => {
            format!("Something answered at {shown}, but it isn't a Nuvora server.")
        }
        _ => format!(
            "Couldn't reach a Nuvora server at {shown}. Check the address and that the server is running."
        ),
    })
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("server.json"))
}

pub fn load(app: &AppHandle) -> Option<SavedServer> {
    let text = std::fs::read_to_string(file(app)?).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(app: &AppHandle, server: &SavedServer) {
    let Some(path) = file(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, serde_json::to_vec_pretty(server).expect("serializes"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_tries_nuvoras_port_first() {
        assert_eq!(
            candidates(" 192.168.1.20 "),
            [
                "http://192.168.1.20:8640",
                "https://192.168.1.20",
                "http://192.168.1.20"
            ]
        );
    }

    #[test]
    fn a_port_or_scheme_is_taken_as_typed() {
        assert_eq!(
            candidates("nas.local:9000"),
            ["http://nas.local:9000", "https://nas.local:9000"]
        );
        assert_eq!(
            candidates("https://media.example.com/"),
            ["https://media.example.com"]
        );
        assert_eq!(candidates("HTTP://Box:8640/web"), ["http://box:8640"]);
        assert!(candidates("   ").is_empty());
    }
}
